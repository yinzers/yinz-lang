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

---

## Phase 3 step 3.0 — RED pins committed, nothing fixed

**Dispatch** `hardening-p3.0-20260906-a1`, 2026-09-06. Committed the five probes named above (A,
D, G, J, N) as fixtures under `crates/ynz-driver/tests/fixtures/` plus one new `#[ignore]`d test
file, `crates/ynz-driver/tests/frago002_c1_c2_planned_red.rs`, mirroring the
`fr23_uaf_planned_red.rs` / `d5_frame_slot_collision_planned_red.rs` planned-RED convention
exactly (`#[ignore = "planned-RED: ..."]`, WHY comments naming the producer and cluster, run
explicitly with `-- --ignored`). One file for both C1 and C2, since FRAGO 002 itself is the shared
filing all five probes came out of. No fix landed; the compiler is untouched.

All five confirmed to FAIL on today's tree (verbatim output in the dispatch report). G and J
(non-deterministic at `-O0`, ~1/3–1/2 corruption rate observed) are pinned by running the built
`-O0` binary 30 times and asserting every run matches the correct value — false today with
overwhelming probability, true with certainty once the real fix lands, so neither direction is a
coin-flip. A and D are pinned at BOTH tiers because the failure SHAPE differs by tier (SIGABRT at
one, silent wrong value at the other) rather than being the same symptom read twice.

`fuzz_grammar/mod.rs` and `.github/workflows/ci.yml` were not touched, per the brief. A plain
`cargo test -p ynz-driver --test frago002_c1_c2_planned_red` (no `--ignored`) reports `5 ignored,
0 failed` — the new target is invisible to a normal or `--no-fail-fast` workspace run, so this
diff changes nothing about what a workspace run currently reports as failing.

---

## FRAGO 003 — the `fr23` red is a STALE TEST, not a live use-after-free; and the conductor's own hypothesis named the wrong culprit

**Dispatch** `hardening-fr23-20260906-a1` (executor-medium/opus), 2026-09-06. Read-only; tree clean.

### Verdict

**STALE TEST.** All three failures are **compile-time rejections**. No binary is produced, no
bytes are read, no runtime behaviour is involved. The language deliberately made these three
fixture programs illegal and nobody updated the tests.

Fresh run: `cargo test -p ynz-driver --test fr23_uaf_planned_red` → **15 passed, 3 failed**. Every
failure panics at `assert_tier_prints_correct_haul`'s `build_out.status.success()` assertion, on
the first tier attempted, before a binary exists.

### The culprit — and the conductor's brief was wrong about it

I briefed this dispatch to suspect **FRAGO 022/023's default-deny redesign**. Wrong. FRAGO 023's
own verification record shows the suite at **15/15 green** when it landed; that change did not
break these.

The actual producer is **v0.3-M8 Phase 4's transfer rule**, three weeks later. Conductor-verified
independently: `git log -S "TransferNeedsCopy" -- registry/features.toml` returns exactly one
commit, `2be2244` — "M8 Phase 4 round 1 — channel close, `maybe<T>` receive, **the transfer rule**,
fr12 cells, refuse_closed".

It was deliberate and signed. `IMP-ownership.md`'s "Transfer — Who Else Holds This Value" records
the rule as designed in M8 Phase 2, **signed off 2026-09-03**, shipped in Phase 4. Each failure
maps onto `check_transfer`'s documented contract:

- `first.value` — a `FieldAccess` → `Provenance::Reaches` → `TransferNeedsCopy`.
- `makeCargo().reroute()` — a call not returning fresh → same arm; the diagnostic's enumerated
  `{reason}` form appears verbatim in the design doc.
- `m` — originates from `ships[0]`, an item inside a container → `Reaches` → same arm.

The sink in all three is the callee's own declared `give` — the design doc's sink 2, where
`TransferNeedsCopy` is explicitly correct "because the callee's author wrote the word." Each
fixture declares `function identity<T>(give value: T) -> T`.

**Nobody diagnosed it.** M8's own audit logged these three as "give/copy-inference failures,
unrelated to background-handle code" — correct as far as it went, never followed up.

### The memory-safety question, answered independently of the tests

**The UAF is genuinely fixed, not merely unreachable.** `bg_arg_is_provably_safe` is intact as
FRAGO 022's default-deny (safe set, then a trailing `_ => false`), `SelfValue` still removed per
FRAGO 024, and `prepare_bg_arg_for_ctx`'s `is_heap_arg` gate still reads *presence* in
`background_arg_inferred_ownership` rather than its variant — one authoritative record, no
codegen-side twin. Fifteen live green locks exercise it today.

Empirically confirmed rather than code-read alone: rewriting the two lost shapes with the `.copy()`
the diagnostic asks for compiles and prints correct values at **both** tiers, and these genuinely
exercise the gate rather than sidestepping it (`bg_expr_resolved_type` returns `None` for
`Expr::PostfixOp`, so `T` stays unresolved, the `Call` arm reads fail-closed, the record is
written, codegen heap-upgrades).

**One honest caveat, named rather than buried.** Test 3's shape — a `maybe<Shape>` transferred
through a generic `give` — is currently **unreachable-by-construction**, which is a weaker
guarantee than fixed. `m.copy()` is refused because provenance classifies it `Unknown`, exactly the
documented FR#10 deferral. Three separate attempts to produce an owned `maybe<Cargo>` all failed
(direct return: type error; `give` accessor: `returns_fresh` correctly propagates `Reaches`;
`channel<Cargo>.receive()`: element type unsupported). A documented deferral with a named trigger,
not a silent hole — and the B′ class it belonged to stays reachable and green via two other locks.

### What must change (remediation dispatched separately; nothing changed here)

1. **Tests 1 and 2 — UPDATE, not delete.** Add the `.copy()` the diagnostic asks for. Proven to
   compile and print correctly at both tiers. The assertion strings stay byte-identical; only the
   fixtures become legal Yinz again. That is not weakening — the regression they lock (a nested
   `FieldAccess`/`MethodCall` inside a generic callee's argument must heap-upgrade) still routes
   through the default-deny wildcard and keeps its teeth.
2. **Test 3 — SUPERSEDE with its reasoning migrated.** Its purpose was to prove the B′ admission
   arm read `binding_ty_narrowed` rather than a function table; FRAGO 022 made that moot —
   `FieldAccess` now falls through the wildcard regardless of how its type was derived, so there
   is no table lookup left to regress. Its fixture is inexpressible until the `maybe-move-out`
   deferred feature lands. Delete **with** rationale folded into the surviving B′ locks, plus a
   parked entry tying restoration to that trigger.
3. **THE HEADER MUST BE CORRECTED, and this is the fix that stops the recurrence.** It currently
   asserts categorically that "a red here is a live use-after-free in the flagship concurrency
   surface." That sentence is what turned four gates into rubber stamps: it is true of a
   runtime-value red and false of a build-rejection red, and it does not distinguish them. It
   needs that clause.

### The conductor's error, on the record

Four gate dispatches were told to expect these three failures as intended planned REDs. That
instruction originated with me and was wrong — the file is not a planned-RED file, carries no
`#[ignore]`, and its own header says the opposite. Each gate did what it was told and confirmed the
diagnostic class; none was positioned to know the brief contradicted the file. The failure was not
in the gates. It was in briefing them from memory instead of from the file.

---

## Phase 3 step 3.1 — C1 FIXED at its producer; four pins live; both fuzz guards deleted; zero corpus regressions

**Dispatch** `hardening-p3.1-20260906-a1`, 2026-09-06. Tree left dirty for the conductor to seal;
three files touched (`crates/ynz-typeck/src/check.rs`,
`crates/ynz-driver/tests/frago002_c1_c2_planned_red.rs`,
`crates/ynz-driver/tests/fuzz_grammar/mod.rs`).

### The fix, in two parts — the second was NOT anticipated by FRAGO 002

**Part 1 — the producer.** In `collect_crossings_in_stmts`, the `collect_ident_refs_in_stmt` call
is HOISTED out of the `If`/`While`/`For`/`Match` arms and runs for EVERY statement classified as
`this_stmt_suspends`, immediately after the pending-result-binding flush. A suspending statement's
operands run strictly after the prior suspension in the sequence, so a read of a pre-suspension
local there is a real crossing. The three per-arm calls are deleted; there is now ONE scan for the
one question, so a fifth shape added to `this_stmt_suspends` cannot arrive without its scan — the
arm-by-arm alternative would have rebuilt the same omission the next time the classifier grew.

**Part 2 — the provenance split had to become span-based.** Part 1 alone regressed exactly two
corpus programs from *builds and runs correctly* to *compile error*:
`v0_3_m6_maybe_arg_pure_call.ynz` and `v0_3_m6_union_arg_pure_call.ynz`, both
`UnsupportedCrossingLocalType`. Diagnosed rather than accepted: `arg_escape_only` was computed by
COLLECTOR ORDER (`names[before_arg_escape..]`), which is a proxy that held only while the lexical
scan could not see an argument-position read. After Part 1 the two collectors describe ONE event at
ONE span for `pick(m)`, the order-based split misfiled it as a lexical crossing, and the
`maybe`/`union` skip that bind-time heap-cell promotion earns was silently withdrawn. Fix: the
arg-escape collector now records the exact `Ident` span of every argument position it qualifies
(new `ArgEscapeSink`), and a name is `arg_escape_only` when every crossing span the ONE lexical
scan recorded for it is one of those spans. Names with no lexical crossing qualify vacuously — the
original case, unchanged. The two facts can no longer drift apart, because one is now defined in
terms of the other rather than in terms of the order they were computed in.

Both parts live inside the single authoritative producer (`locals_crossing_wait` /
`crossing_local_names_with_provenance`), which typeck's Check 2/2b, the M3d decline probe
(`suspension_guards_fire_for_fn`) and codegen's `crossing_local_names_with_cpu_spike` all read. No
second derivation was added anywhere.

### The precondition, measured rather than assumed

626-fixture corpus, `ynz build` at the default tier, before and after, recorded as
`<fixture> <exit> <first-diagnostic>`:

| | builds clean | build error | exit 2 |
|---|---|---|---|
| baseline (pre-fix) | 504 | 121 | 1 |
| after Part 1 only | 502 | **123** | 1 |
| after Parts 1+2 (landed) | 504 | 121 | 1 |

**Landed delta: ZERO** — the two files are byte-identical. The two-program regression was real,
was caught by measuring instead of assuming, and was fixed upstream rather than absorbed. No
program that built before fails to build now; no new decline is user-observable in the corpus.

### Results

- **Pins A, D, G, J: GREEN**, `#[ignore]` removed, reasons rewritten as live regression locks.
  A and D print `3\nend` at both tiers (were SIGABRT / silent `6`). G is 30/30 `10.2` and J is
  30/30 `102` at `-O0` (were wrong in roughly half and a majority of runs).
- **Pin N (C2) stays ignored and stays RED** — verified with `-- --ignored`: still prints `99`.
  The clustering in FRAGO 002 holds; step 3.2 still has its job.
- **Two corpus tests that were RED on the baseline are now GREEN** — and nobody had noticed they
  were red: step 3.0's own committed fixtures are inside the sweep corpus, so
  `corpus_produces_deterministic_output_across_runs` and
  `corpus_byte_identical_across_mode_matrix` had been failing (1 determinism finding, 8
  mode-matrix divergences, all four C1 fixtures) since that commit.
- **FRAGO 002 open question 2 is SETTLED: the fix alone closes the findings.** Both generator
  suppression guards deleted (`Builder::suspension_seen` in its entirety — field, five
  assignments, and both reuse gates — and the `send_count`-versus-capacity floor). Two independent
  `YNZ_FUZZ_PROGRAMS=256` sweeps: **0 findings** at seed base 0 and **0 findings** at seed base
  777000, 256/256 compiled and ran to exit 0 in each.
- **Non-vacuity checked, not assumed** (throwaway probe over the same 256 seeds, deleted after
  reading): 166 programs fire the drain loop, 367 channels are drawn at a capacity below the
  maximum send count, and pool reuse fires 32/256 — against 13-17/**1024** measured under the
  deleted gate. The sweep genuinely exercises the shapes the guards used to forbid.

### The record corrections

- `take_or_make_array`'s "specific to the channel-transfer path" — replaced with the disproof
  (probe D: no channel anywhere) and the real producer, named.
- The `int`/`number` self-contradiction — `FeedFn::send_count`'s cautious "untested, not confirmed
  safe" was the honest one; both it and `stmt_background_drain_loop`'s "general to BOTH int and
  number" now say the discriminator is **whether a local is read by a statement that suspends**,
  not the element type. The `pool_reuse` floor's rationale and assertion message were also
  de-staled (they described the deleted gate).

### Residual for the conductor, NOT actioned here

`.claude/plans/parked.md` entry 49 is now closed by this fix — both (a) and (b). Its text still
routes a future reader to "read `mod.rs`'s guards first — they are the in-tree record," and those
guards no longer exist. Left untouched because parked.md is the conductor's ledger and outside
this dispatch's named files; it needs a close-out pass.

### Known-red, pre-existing, NOT this dispatch

`cross_impl_consistency::bounded_run_kills_the_whole_tree::timed_out_program_leaves_no_descendant_process_running`
failed in the PRE-FIX baseline run of the same target and fails intermittently after; it passes
3/3 in isolation and fails only when the target's own fuzz sweep saturates every core. That is the
wall-clock-budget-calibrated-on-an-idle-machine corpse class `.claude/rules/test-parallelism.md`
already names — a 3s poll window, not a value assertion. Unrelated to this diff.

---

## Phase 3 step 3.1 FIX ROUND — the widened scan was rejecting correct programs; fixed at the same producer

**Dispatch** `hardening-p3.1-fix1-20260906-a1`, 2026-09-06, answering a confirmed reviewer finding
against `eb0aa0c`. Tree left dirty for the conductor to seal.

### The regression, and why it is the same producer rather than a new one

```ynz
wait sleep(1)
let m: maybe<int> = `42`.toInt()
const v = wait consume(m.or(0))     // Error: a `maybe<int>` value cannot yet cross a `wait`.
```

Delete the leading `wait sleep(1)` and the identical read compiles. `m` is declared AFTER the
suspension and read only in operands evaluated BEFORE the next one — it crosses nothing. Same with
`fixed<int>`, since index access returns a `maybe`.

Producer, named: `collect_crossings_in_stmts` kept ONE set (`declared`) that accumulated every
local declared since the most recent suspension, and **nothing asked whether a suspension actually
fell between a local's declaration and the read being scanned** — which is the crossing
precondition. Step 3.1's hoisted `collect_ident_refs_in_stmt` did not create that imprecision, it
extended its reach: the pre-existing `else` branch had it too, and
`wait sleep(1)  let m: maybe<int> = ...  print(m.or(0))` was rejected on every tree since the
analysis was written. Both instances die at one fix, which is the check `root-cause.md` asks for.

### The fix

The mechanism that was already correct for result-bindings is generalized to every post-suspension
binding. `pending_result_bindings` becomes `declared_since_suspension` and now stages EVERY local
bound since the last suspension; `declared` holds exactly the names a suspension separates from the
current position. One flush point, at the next suspension.

Ordering is decided by WHERE the statement's suspension sits, derived once as a new
`SuspensionSite` enum returned by the single classifying `match` (was a bare `bool`) and read from
there — no second derivation of "is there a suspension between here and there":

- **`AtRoot`** (`f()`, `let x = wait f()`, `ch.send(v)`) — operands all run before the statement
  suspends, and typeck **Check 3** (`suspending_calls_in_subexpr_position`) rejects a nested
  suspending call inside them, so "before the root suspension" is "before every suspension in this
  statement". Scan against `declared`, THEN flush. Verified live: both `outer(inner(), m)` and
  `wait outer(wait inner(), m)` are compile errors today.
- **`InControlFlow`** (`if`/`while`/`for`/`match` with a suspending body) — a loop back edge
  re-evaluates the condition after the body suspends, and the scan walks the body. Flush FIRST,
  scan against the widened set: unchanged, deliberately conservative behavior.

### Proof it narrowed precisely rather than loosening the guard

| program | before | after |
|---|---|---|
| `wait` · `let m: maybe` · `wait consume(m.or(0))` | rejected | **prints 42** |
| `wait` · `let f: fixed` · `wait consume(f[2].or(0))` | rejected | **prints 3** |
| `wait` · `let m: maybe` · `print(m.or(0))` (pre-existing instance) | rejected | **prints 42** |
| `let m: maybe` · `wait` · `print(m.or(0))` | rejected | **still rejected** |
| `wait` · `let m: maybe` · `wait` · `print(m.or(0))` | rejected | **still rejected** (the flush) |
| `wait` · `let m: maybe` · `while { wait  print(m.or(0)) }` | rejected | **still rejected** |
| `wait` · `let m: maybe` · `if { wait  print(m.or(0)) }` | rejected | **still rejected** |

### The corpus delta, and what it is NOT evidence of

626 fixtures, `ynz build` at the default tier, `<fixture> <exit> <first-diagnostic>`, baseline
binary built from `eb0aa0c` versus the fixed tree: **504 clean / 121 build error / 1 exit-2 on
both, files byte-identical.**

Said plainly, because the number invites the wrong reading: **a zero delta is evidence about the
corpus, not proof about the language.** The corpus contains no program of this shape — that is
precisely why the rejection shipped through a 626-fixture sweep and two 256-seed fuzz sweeps. The
three programs that DO move are the three committed here as fixtures; all three were verified RED
against the `eb0aa0c` binary before the pins were written.

### New pins

`crates/ynz-driver/tests/post_suspension_local_not_crossing.rs` — three tests, one per fixture,
both tiers each. It is the deliberate mirror of `frago002_c1_c2_planned_red.rs`: that file locks
the direction where this producer reports too LITTLE (missed crossing → silent wrong output), this
one locks where it reports too MUCH (false `UnsupportedCrossingLocalType`). One producer answers
both, so both directions need a lock or the next fix trades one failure for the other.

### The other two review findings

- **Check 2 / Check 2b's safety rationale in `check_function` was false by construction** under
  Part 2's span rule (it claimed lexical crossings "never enter `arg_escape_only`"; under the span
  rule one whose span IS a qualified argument position does). Comments corrected to state the span
  rule and what actually disqualifies a name — a crossing read at any non-argument span. Code
  unchanged; it was right.
- **The fuzz generator's capacity draw had narrowed while widening.** `1 + below(send_count)` gives
  `cap ∈ [1, send_count]`: blocking well covered, slack buffer (`cap > send_count`) UNREACHABLE,
  where the pre-floor draw could reach 4. Now `1 + below(send_count + 1)`, one draw so seed streams
  stay comparable. Measured over 256 seeds (throwaway probe, deleted): 252 drain-loop channels —
  **28.2% blocking, 35.7% exact, 36.1% slack**, capacities 1–4 reachable.

### Verification

- Two `YNZ_FUZZ_PROGRAMS=256` sweeps after the capacity change: **0 findings** at seed base 0 and
  **0 findings** at 777000; 256/256 compiled and ran to exit 0 in each, 256 distinct.
- Pins **A, D, G, J green**; pin **N still `#[ignore]`d and still RED** (prints `99`, want `1`) —
  cluster C2 is step 3.2's job.
- `fr23_uaf_planned_red` **17/17**. `cargo test -p ynz-driver`: 779 passed, 0 failed, 4 ignored
  (pin N, two D5 planned-REDs, one replay tool). `-p ynz-typeck -p ynz-codegen`: all green.
  `clippy --workspace --all-targets -D warnings` clean; `fmt --all --check` clean.

---

## FRAGO 003 — Phase 3 step 3.2 (dispatch `hardening-p3.2-20260906-a1`): cluster C2 closed

Base `c3988ab`. Closes **M8 FR #9** (a live use-after-free) and **M8 FR #10** (a live silent wrong
answer) at one producer, under Patrick's 2026-09-06 ruling.

### The producer, and what replaced it

Two per-type dispatches answered "give me an independent copy of this heap value", agreeing only
by comment, and both DEFAULTED to handing back the receiver's own pointer:
`copy_lowering_arm`'s `AliasNoOp` and `prepare_bg_arg_for_ctx`'s `array<pointer-elem>` branch plus
its `_` arm.

One table now answers it: `ynz_typeck::owned_copy::owned_copy_plan`, exhaustive over `Type` with
no `_` arm, with exactly two answers per type — a copy strategy, or a `CopyRefusal` carrying its
own WHAT-detail / WHAT-INSTEAD / WHY. One emitter turns that answer into machine code:
`ynz_codegen::emit::emit_owned_copy`, exhaustive over `OwnedCopy` with no `_` arm, taking a
`CopyMode` (`Body` for `.copy()`, `SpawnArg` for a `background` argument) because the two consumers
share a plan but not a lifetime. `copy_lowering_arm`, `CopyLowering` and `AliasNoOp` are deleted;
`types::copy_is_independent` is a `pub use` re-export of the derived predicate, so it cannot grow a
body of its own again.

### The per-type ruling

| Type | Answer | Reason |
|---|---|---|
| `int` `float` `bool` `string` `options` | copy = the receiver | nothing can change the contents, so a second name cannot disagree with the first |
| `number` (≤34 digits) | copy = the receiver in a body; heap cell at a spawn | immutable, but in frame-owned storage |
| `range` | same, and REFUSED at a spawn | immutable; no spawn-side re-homing exists, so passing it would dangle |
| `shape` | struct bytes into fresh storage | unchanged; the pointer-field residual is named below |
| `fixed<T>` | N cells into fresh storage | the cells hold the items outright — this is FR #10 / pin N |
| `array<T>` | fresh header + buffer, items copied through the same emitter when the cells hold pointers | one level would leave both lists sharing their rows — this is FR #9 |
| `map<K, V>` | fresh header + four buffers | unchanged; the value-cell residual is named below |
| `maybe<T>` | fresh envelope cell | refused when the inner needs its own fresh allocation |
| `sensitive<T>` | the inner's answer | the copy KEEPS the label, so it is still redacted; refusing would push people to `.reveal()` just to get a copyable value — worse for the secret |
| `channel<T>` | REFUSED | a channel is shared on purpose; a copy is a line nobody is listening on |
| task handle | REFUSED | a handle names one running task; a second one names the same task |
| `dynamic C` | REFUSED | which shape is inside is only settled at run time, so there is no byte count to copy |
| union | REFUSED (both faces — `.copy()` and a `Copy` spawn argument) | which choice is live is only settled at run time, and they are different sizes |
| bignum `number` (>34) | REFUSED | unratified representation; an alias would be silent the moment it ships |
| `nothing` | REFUSED | there is no value |
| map entry | REFUSED | the loop rewrites the view next turn; copy `entry.key` / `entry.value` |
| `errors`-capable | REFUSED | until `.failed()` runs it is either the answer or a failure |
| generic instantiation | REFUSED | copying one is not supported; aliasing it silently is what this replaces |
| type parameter | not a type yet — no diagnostic at the body; the refusal fires at the call site where the real type is known |

### Residuals, named rather than hidden

- **`shape` copies are one level.** A pointer-valued field (nested shape, `array`, `map`, `maybe`)
  is copied as a pointer. Pre-existing, unchanged, and out of this cluster's scope; making it deep
  needs a per-shape recursive clone with a release story.
- **`map` copies are one level**, same shape of gap; iterating a map's occupied slots from
  generated code is the missing machinery.
- **A deep array copy's items are not released**, and a `map` cloned at a spawn is not released.
  Both are four-field deferrals written at their emitter arms, both triggered by Phase 4's release
  pass, and both are strictly better than the alias they replace.
- **`fixed<T>` parameters lose their length** — indexing a `fixed<int>` parameter always yields
  `none`. Confirmed PRE-EXISTING against the `c3988ab` binary (identical output), unrelated to this
  change, and the reason no `fixed<T>` spawn fixture ships here.

### The corpus delta, and what it is NOT evidence of

750 files (`crates/ynz-driver/tests/fixtures` + `examples`), `ynz build`,
`<file> <exit> <first-diagnostic>`, `c3988ab` binary versus the fixed tree: **587 clean / 162 build
error / 1 exit-2 on both, byte-identical, zero delta.** Re-measured after the spawn-side refusal
landed: **588 / 163 / 1 over 752 files** — the baseline plus exactly the two files this change
adds (one clean fixture, one gallery). That second reading is aggregate rather than per-file; the
per-file guarantee for it comes from `cross_impl_consistency`, which builds and runs the whole
fixture corpus and is green.

Said plainly: **a zero delta is evidence about the corpus, not proof about the language.** Nothing
in the corpus called `.copy()` on a channel, a handle, a union or a `dynamic` value, and nothing
copied a nested container — which is exactly why an alias sat in the default path across two
milestones without a single test noticing. The programs that DO move are the ones committed here as
fixtures, and each was verified against the `c3988ab` binary before its pin was written.

### Pins and fixtures

- **Pin N** (`frago002_c2_red_fixed_copy_aliases_source`) — `#[ignore]` removed, prints `1`. The
  file now has no ignored test at all.
- **FR #9's pin** — `bg_arg_alias_container_add_is_a_known_uaf_red_pin` became
  `bg_arg_alias_container_add_no_longer_aliases_the_parents_container`, and its fixture was rewritten
  from "observe the defect through the alloc counter, do NOT dereference" to three green-world
  readings: the parent still sees 1 row, the parent's row 0 still starts at 1 (a one-level clone
  fails here), and the parent DEREFERENCES `bucket[0]`, which is what the defect made unsafe. Both
  `cross_impl_consistency` exclusions for this fixture removed, as their own text instructed.
- **New** `v0_3_hardening_c2_deep_copy_independence.ynz` + `copy_of_a_container_owns_its_items_too`
  — the depth lock with no `background` in it. Verified RED against `c3988ab`: printed
  `original row 0 starts at 99` where `1` is correct.
- **New** `examples/primantis-orders/v0_3_hardening_errors.ynz` + gallery test — all seven refusals
  in one file (six `.copy()` sites plus the `background`-argument one), asserted by their own WHAT
  phrase AND their own WHAT-INSTEAD, so a refusal cannot regress into a generic message.
- `examples/pirates-roster/entrypoint.ynz` grew a `.copy()` section (nested manifest, `fixed` bell,
  and the channel refusal shown as a comment); golden regenerated, appended lines only.

### The `background` face of the ruling, found by making the alias loud

Closing the alias arm turned one previously-miscompiling program shape into a BACKEND error
string: a union `background` argument the spawner keeps reading. Before, it silently shared one
value between the task and the spawner; after, codegen hit "no independent copy exists … this is
a compiler bug", which is both ugly and a lie — the program is a user program, not a compiler
fault. The ruling says a refusal must be loud AND readable, so the refusal moved to typeck as
`SpawnArgNotIndependent`: same per-type `{fix}`/`{why}` fills, a spawn-shaped WHAT and WHY.

The first cut of that check was too wide and refused `MapEntry`, a shipped and working spawn
argument — because "can this be copied?" is not the spawn path's question. The spawn path
re-homes three types with mechanisms of its own (a `channel` is shared by design, a map-entry
loop view is stabilized, a decimal number goes to a heap cell), and those pre-gates were a match
in codegen that typeck knew nothing about. That list is now ONE function,
`owned_copy::spawn_rehoming`, which `prepare_bg_arg_for_ctx` dispatches its pre-gates off and
`spawn_arg_can_be_independent` reads — a twin removed on the way past rather than papered over
with an exemption.

### Two failures this change caused, and what they taught

- `hotfix_bg_arg_number_field` greps the IR for `%bg_shape_src`. The shared emitter had renamed it.
  IR value names are now mode-stable by construction (`copy_src` / `bg_shape_src`,
  `arr_copy_clone` / `bg_arr_clone`) — the two call sites' historic names, preserved.
- `v03_m8_channel_close::m8_p4_chan_map_roundtrip` went from gap 10 to gap 15: a `give` `map`
  argument was newly cloned, leaking the spawner's original. The fix is not an exemption but the
  authoritative fact — typeck's recorded `BgOwnership::Give` means the spawner's binding was
  consumed, so the task is the sole holder and there is nothing to be independent from. That gate
  is deliberately NOT extended to `array`, whose `give`-path clone is what the drop ladder owns.

### Verification

- `cargo test -p ynz-driver --no-fail-fast`: all targets green, including
  `cross_impl_consistency` 16/16 (1 ignored). `fr23_uaf_planned_red` **17/17**;
  `post_suspension_local_not_crossing` 3/3; `frago002_c1_c2_planned_red` **5/5, none ignored**.
- `-p ynz-typeck -p ynz-codegen -p ynz-registry -p ynz-diagnostics -p ynz-tmgrammar`: all green.
- `clippy --workspace --all-targets -- -D warnings` clean; `fmt --all -- --check` clean.
- One earlier run of the full driver suite tripped
  `bounded_run_kills_the_whole_tree::timed_out_program_leaves_no_descendant_process_running`;
  passes in isolation every time (verified twice) and passed one of three full runs — parked
  entry 50's known contention flake, not this diff.
