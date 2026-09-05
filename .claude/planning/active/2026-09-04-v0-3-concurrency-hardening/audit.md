# Audit Trail — v0.3 Concurrency Hardening

Phase 1–4 findings, FRAGOs, and decisions will be appended here as each phase completes.

Sections will be added:
- **Phase 1 findings** — blocker list audit results, cross-reference reconciliation
- **Phase 2 findings** — Question (a) & (b) probe results, FRAGO definitions
- **Phase 3 findings** — RED pins, fix commits, regression checks
- **Phase 4 findings** — release codegen paths added, test results

---

*Plan file created 2026-09-04. Audit initiated.*

---

## FRAGO 001 — Phase 2 question (a) ANSWERED: no heap local is ever released at scope exit

**Dispatch** `hardening-p2a-20260905-a1` (executor-medium), 2026-09-05. Diagnosis only; nothing
fixed, nothing shipped.

### The answer

**No.** An ordinary heap-backed local — `array<T>`, `map<K,V>`, a runtime-built `string`, a
heap-promoted `maybe<T>` cell — is **never** freed when its scope ends. Not at function return,
not at nested-block exit, not at loop-iteration end. Not for any type. This is a genuine
unbounded leak *within a single run*, not memory reclaimed by some mechanism a naive reading of
codegen would miss.

This upgrades M8 Phase 7's conclusion from a plausible premise to a settled finding. That
conclusion was drawn while investigating handles, and its supporting probes showed balanced
alloc/free counts that came from the background-arg glue rather than from scope exit — so it was
never actually tested for ordinary values. It is now.

### Structural proof (conductor-verified independently, not relayed)

`crates/ynz-codegen/src/emit.rs` contains exactly **7** free-emission call sites, reached through
the `cg.rt.*` builder handles (an earlier grep for the bare symbol names as string literals
under-matched and found nothing — the call sites go through `cg.rt.ynz_array_drop` and
siblings). All 7 live in exactly **3** functions:

- `build_cpu_trampoline` — the spike trampoline's one staged decimal128 cell
- `channel_drop_glue` — the channel element-glue table
- `emit_bg_arg_frees` — the background-arg copy glue

`ynz_string_free` and `ynz_handle_free` have **zero** call sites. There is no scope-exit dispatch
anywhere in codegen. The three emitters are narrow, special-cased, and none of them is a general
release mechanism.

IR read of the simplest case confirms it directly: `makeAndUse` allocates an `array<int>`, pushes
five elements, loops over it, and exits on a plain `ret void` — no `ynz_array_drop`, no
`ynz_free`, nothing in the `for_after` exit block. The array pointer lived only in a stack
`alloca`; the alloca dies with the frame and the heap buffer it addressed is never touched again.

### Empirical numbers

Reused M8 Phase 7's harness (`YNZ_ALLOC_COUNTER=1`, latched at `ynz_rt_init`, dumped by
`ynz_rt_shutdown`, counting the one authoritative choke point `ynz_alloc`/`ynz_free`).

| Probe | Iterations | alloc | free | per-iter |
|---|---|---|---|---|
| `array<int>` local, single call | 1 | 2 | 0 | 2 |
| `array<int>` local, loop | 2,000 | 4,000 | 0 | 2 |
| same, doubled | 4,000 | 8,000 | 0 | 2 (**exact linear doubling**) |
| `map<string,int>` local, loop | 2,000 | 10,000 | 0 | 5 |
| `array<int>` in a nested `if` block, loop | 2,000 | 4,000 | 0 | 2 (**block exit leaks identically to function exit**) |
| promoted `maybe<int>` cell in a map, loop | 2,000 | 16,000 | 0 | 8 |

Exact linear growth, flat zero frees, confirmed by doubling iterations. Nothing reclaims it:
`ynz_rt_shutdown` drains Tokio and dumps counters — it holds no table of live pointers to sweep,
and the IR shows the pointer was never captured anywhere durable to begin with.

### Instrument validity — the part that makes the zeros trustworthy

A flat zero is exactly what a broken counter also produces, so validity was established two ways
rather than assumed:

- **Positive control, run live rather than trusted:** the existing fixture
  `bg_arg_channel_send_array.ynz` under the same counter gives `alloc=7, free=3` — a gap of 4,
  matching that test's own documented `expected_gap=4` in
  `integration.rs::assert_bg_arg_handoff_fixture`. When a real free mechanism exists
  (`emit_bg_arg_frees` releasing the spawn ladder's arg-copy after the task retires), the counter
  registers it. Against that, probes 1–5's literal zero across 4,000–16,000 allocations is a
  finding, not an artifact.
- **Negative control (transfer):** 50 arrays `give`n into a channel gives `alloc=101, free=1`,
  and the single free is send-count-independent — a fixed 16-byte boxing cell on the channel send
  path, not a release of any payload. `ynz_channel_close` contains no free at all; it only
  detaches the sender half. A purely synchronous program never reaches `ynz_channel_free`, which
  is called only from `emit_bg_arg_frees`.

**Instrument blind spot, exposed rather than routed around:** `ynz_string_concat` and
`ynz_string_builder_finalize` call libc `malloc` directly, bypassing `ynz_alloc`, so the counter
reads 0/0 for strings regardless of truth. The executor did not report that as "no allocation" —
it read the IR and confirmed the string leak structurally. Separately, `ynz_int_to_string` is
**not** a leak: it returns a pointer into a reused thread-local buffer.

### Corroboration found in the tree, not invented here

`integration.rs`'s own comment on the bg-arg handoff test already states that the spawner's
channel local "is itself never released today ... no scope-exit `ynz_channel_free` is emitted."
Pre-existing, independent agreement with this finding, written by someone who noticed the same
thing from a different direction and recorded it where it would be found.

### Consequence for this plan

The shared-producer hypothesis in the plan's Situation section is now a settled premise rather
than a hypothesis: **the absence of any scope-exit release mechanism is a real, verified
producer**, and it is Phase 4's target. Every heap-backed local type leaks identically and
unconditionally, at a rate exactly linear in call frequency — a hot loop building one small array
and one small map per iteration bleeds 7 `ynz_alloc` calls per iteration with zero frees, and it
reproduces inside a single 2,000-iteration run in milliseconds. No long-running-server assumption
is required, which was the framing this plan started with and which understated the problem.

**This does NOT yet establish that it is the ancestor of the M8 defect cluster** (FR #11(a)/(b),
FR #9, FR #10, parked 32–34). That is Phase 2 question (b), still open, and it must be answered
before Phase 3 clusters anything. A settled producer is not the same as a proven common ancestor,
and collapsing the two would be the exact reasoning error `root-cause.md` warns about.

### Process note

The dispatch flagged a mid-task system-reminder (preferring Bash over Read/Edit under bypass
permissions) as a likely injection and declined it. That reminder is genuine harness
configuration, not an injection — the conductor received the same one. This is the second
consecutive dispatch to make that call. The instinct is correct and worth keeping; the resolution
is simply that the repo's `tooling.md` already governs the choice and reaches the same answer.
