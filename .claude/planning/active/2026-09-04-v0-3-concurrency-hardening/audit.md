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

---

## FRAGO 002 — Phase 2 question (b) ANSWERED: three clusters, one singleton, and FRAGO 001 is the ancestor of none of them

**Dispatch** `hardening-p2b-20260905-a1` (executor-medium/opus), 2026-09-05. Diagnosis only; tree
left clean, nothing fixed, no fixture committed.

Six defects went in. The answer is **not one ancestor and not six**: three clusters plus a
singleton. Each cluster carries the observable-change test `root-cause.md` demands.

### The negative result first, because it was the tempting answer

**FRAGO 001's missing scope-exit release is the ancestor of NOTHING on this list.** A missing free
is a *leak*; every defect here is a *read of the wrong bytes*. C1 is an analysis gap in typeck —
the local is never frame-slotted, so the resume reads an uninitialised alloca; releasing memory at
scope exit changes nothing about it. C2's FR #9 is the *opposite* of a leak (a premature free
through an alias). C3 and S1 never touch the heap at all. FRAGO 001 remains a real, verified
producer of its own class. It is not this list's ancestor, and collapsing the two would have been
precisely the error `root-cause.md` names.

---

### C1 — FR #11(a) and FR #11(b) are ONE bug, and the class is wider than either filing

**Producer.** `collect_crossings_in_stmts` (`crates/ynz-typeck/src/check.rs`): once `past_wait` is
true, a statement that is *itself* a suspension point has its result-binding recorded but its own
operands are never scanned. The `If`/`While`/`For`/`Match` arms call
`collect_ident_refs_in_stmt`; the direct suspending forms — `Stmt::Expr(conduit send)`,
`Stmt::Expr(suspending call)`, `Stmt::Let{value: Call|Wait|MethodCall}` — fall through `_ => {}`.
A pre-suspension local read only by such a statement never enters the crossing set, gets no frame
slot, and is read from an uninitialised alloca in the resume invocation. The arg-escape collector
does not cover the hole: `collect_aggregate_args_in_expr` fires only on `is_suspending_call` /
`expr_is_ufcs_suspending_call` (a conduit `ch.send` is neither), and `mark_aggregate_arg` admits
only stack-backed types, never `array`/`map`.

*Conductor-verified independently:* the suspending forms are classified at the top of that
function and the `collect_ident_refs_in_stmt` calls appear only in the `If`/`While`/`For` arms; the
direct forms reach `_ => {}`.

**Members.** M8 FR #11(a), FR #11(b).

**Evidence — 6 probes, `target/p2b-probe/` (gitignored, left for Phase 3):**

| Probe | Shape | Result |
|---|---|---|
| A | `array<int>` before `wait sleep`, `wire.send(rows)` after | SIGABRT 3/3, **default mode** |
| B | same, declared *after* the suspension | correct 3/3 — control holds |
| C | **A plus one harmless `print(rows.count())` before the send** | correct 12/12 — **the read alone fixes it** |
| D | **no channel at all** — array arg to a suspending user function | prints **6** instead of 3, **exit 0, default mode** |
| E | the `let sent = wire.send(rows)` form | misaligned-pointer abort |
| G | `number` sent 3× into `channel<number>(1)` (sends 2–3 block) | `6.8` in 6/12 at `-O0`; clean 5/5 at default |
| H | **G plus one harmless `print(price)`** | correct **12/12** |
| J | int-**local** twin of G | prints heap addresses 5/12 |
| I | the fuzzer's literal-Int feeder, no local | clean 12/12 |

IR: probe A emits `rows.0` only; probe C emits `rows_slot`, `rows_flush_p2i`, `rows_reload_i2p`.
The only difference is one harmless read.

**Probe H is the argument.** The same one-line control that fixes (a) also fixes (b). Two
symptoms, one producer.

**Probe D is the severity.** No channel, no `background`, no `.copy()`, no `errors` — an array
declared before an I/O call and passed to a function after it. Silent wrong output, exit 0,
default optimized mode. That is ordinary code, and it is worse than FR #11(a)'s own filing.

**Three committed claims overturned, each independently checkable:**
1. `fuzz_grammar/mod.rs::take_or_make_array`'s "specific to the channel-transfer path" — **false**
   (probe D has no channel).
2. `mod.rs` contradicts itself on Int: the `FeedFn::send_count` comment says "untested, not
   confirmed safe"; the `stmt_background_drain_loop` comment says "general to BOTH `int` and
   `number`". Parked 49(b) relays the second. Probes I and J show the discriminator is not the
   element type — it is **whether a local is read by a statement that suspends**. The cautious
   comment was the honest one.
3. Parked 48 understates FR #10 — see C2.

**What dies when fixed.** A prints 3; D prints 3; E runs; G prints 10.2 at `-O0`; J stops printing
addresses; **both fuzz-generator suppression guards become deletable.**

**Precondition for the fix, not a reason to defer.** Widening the crossing set pushes more locals
through `suspension_guards_fire_for_fn`, and types that cannot be frame-backed (`fixed`, `maybe`,
union, `dynamic`, nested shape) currently make a function decline. The fix may convert today's
silent miscompiles into new declines or compile errors on programs that build today. Measure the
delta on the existing corpus before landing.

---

### C2 — FR #9 and FR #10 share a design ancestor, not a code line

**Producer.** Two independent per-type dispatches answer "give me an independent copy of this heap
value," and both default to returning the receiver's own pointer:
`prepare_bg_arg_for_ctx`'s `array<pointer-elem>` branch and `_` arm, and `copy_lowering_arm`'s
`AliasNoOp`. Same type set. The bg-arg arms' own comments cite `.copy()` as justification; parked
47 and 48 name the identical trigger.

**Members.** FR #9 (bg-arg escape door #4), FR #10 (`.copy()` alias arm).

**Honest boundary.** Fixing one does not fix the other. This is **one decision and one shared
clone routine, two call sites rewired** — not one edit. Two fresh per-type tables would rebuild
exactly the twin `authoritative-derivation.md` forbids, which is *why* it is one cluster.

**FR #10 has live user-visible exposure today.** Probe N: `let b = a.copy(); b.set(0, 99);
print(a[0])` prints **99**. No transfer, no diagnostic, silent wrong answer in the default mode.
Parked 48 says the residual is "auditing what `.copy()` SHOULD mean" on the grounds that
provenance refuses the transfer — that understates it.

**Record correction.** `.copy()`'s lowering is no longer `_ => Ok(recv_val)`. It is an exhaustive
`copy_lowering_arm` over `Type` with **no `_` arm**, a named `AliasNoOp` variant, and
`copy_parity_tests` binding it to typeck's `copy_is_independent`. Same defect, better fenced.

**CROSS-PHASE ORDERING CONSTRAINT — load-bearing.** FR #9's defect is a *premature free*: the
ladder frees a clone the parent still points at. **Phase 4's scope-exit release pass must NOT land
before C2 closes**, or it will emit frees on aliased pointers and convert a dangling read into a
double-free.

---

### C3 — parked 33 confirmed; parked 32 unsettled

**Producer.** Every flow-sensitive `errors` fact in `check.rs` keys on a bare name —
`errors_failed_true_branch`, `errors_consumed`, `errors_success_narrowed` — and
`check_errors_field_needs_failed_check` admits a read by matching `Expr::Ident(name)` against that
list. `check_stmt_if` push/pops `self.scope` around the body, but the errors sets are not
scope-aware, so a shadowing inner `let` inherits the outer binding's checked status.

**Reproduced** (probe L3): `let x = mayFail(1); if (x.failed()) { let x = mayFail(3);
print(x.message) }` compiles and runs. The inner `x` was never checked.

**parked 32 is a CANDIDATE, not a member.** Two shaped repro attempts both produced correct
output; parked 32's own record says half was fixed in round 3 by `restore_ec_receiver_ty`.
**Settling experiment, cheap and named:** recover the round-3 executor's exact repro from the M8
`audit.md` (`m8-p4-fix3-20260904`, base `d0c46b3`) and re-run on HEAD *before* budgeting a session
for it.

**What dies when fixed.** Probe L3 stops compiling; `MessageBeforeFailedCheck` fires on the inner
`x`.

---

### S1 (singleton) — parked 34

**Producer.** `EC_FIELDS_REQUIRE_FAILED_CHECK` admits `message`/`suggestions`/`trace`/`source`;
codegen's `Type::ErrorsCapable` field arm lowers `message` and hard-errors on the rest. Two lists,
nothing binding them.

**Not C3's**, against parked's own filing. Parked 33/34 are recorded as sharing one ancestor ("the
`errors`-value field surface was never finished end-to-end") — true as narrative, false under
`root-cause.md`'s test: fixing the name-keying kills nothing in 34, and adding codegen arms kills
nothing in 33.

**Reproduced** (probe M): `late.trace` inside a *correct* guard → "not lowered yet (only
.message) / This is a compiler bug."

**The upstream form, already invented one function away.** `copy_parity_tests` binds
`copy_lowering_arm` to typeck's `copy_is_independent` over an exhaustive sampler, so a type
codegen cannot copy fails to compile. The errors-field surface has no equivalent. Make the field
list one shared enumeration with a parity test, and a fifth admitted-but-unlowered field becomes a
build failure instead of a user-facing ICE.

---

### Recommended Phase 3 ordering

1. **C1 — first, and it is not close.** Silent wrong output in the default mode on ordinary code
   (probe D). Single function, well-understood place, and closing it deletes two fuzz-generator
   suppression guards currently hiding findings from every future round.
2. **C2 — decision starts now, in parallel with C1's code.** FR #9 is a live UAF, FR #10 live
   silent-wrong. Both block on one call Patrick owns: what an owned, independent copy of each heap
   type means. **Must close before Phase 4 opens.**
3. **C3 (33)** — compile-time hole, no memory-unsafety.
4. **S1 (34)** — easiest, but LOUD and self-identifying, so nobody is silently misled. Pair with 33.
5. **parked 32** — archival read before any session is budgeted. It may not exist.

### Not settled, with the settling evidence named

1. parked 32's live status — recover the round-3 repro, re-run on HEAD.
2. Whether C1's fix alone closes all 35 findings from the 256-seed sweep — belongs in Phase 3's
   RED-pin step: after the fix, remove both generator guards and run `YNZ_FUZZ_PROGRAMS=256`;
   findings should go to zero.
3. Why (b) is `-O0`-only — **labelled inference**: LLVM likely folds the poison load to something
   benign at `-O2` while an array pointer is fatal at any level. Optimized IR not read. Does not
   change the fix.
4. The exact size of C2's per-type decision — deliberately not pre-empted; it is Patrick's call.

### Dispatch deviations, flagged

- **Did not modify `fuzz_grammar/mod.rs`** as the brief prescribed. Read the guard doc comments,
  then reproduced both defects from first principles with 15-line hand-written programs —
  deterministic for (a), ~50% for (b), far tighter than a 256-seed sweep. The tree never went
  dirty, so the "confirm the revert" step is vacuous rather than skipped. **Better than the method
  I specified.**
- Did not commit the minimal programs as RED pins; the brief forbade tree writes. A, D, G, J and N
  are the five that belong in `crates/ynz-driver/tests/fixtures/` as Phase 3's first commit.
- Graded parked 32/33/34 as two producers rather than the one their own entry claims.
- Ran three probes the brief did not ask for (D, I/J, N); each overturned or upgraded a committed
  claim.
