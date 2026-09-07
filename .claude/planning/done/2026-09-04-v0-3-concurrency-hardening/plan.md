---
name: "v0-3-concurrency-hardening"
plan-id: "2026-09-04-v0-3-concurrency-hardening"
status: "done"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["hardening-p1-20260905-a1", "hardening-p2a-20260905-a1", "hardening-p2b-20260905-a1", "hardening-p3.0-20260906-a1", "hardening-p3.1-20260906-a1", "hardening-p3.1-fix1-20260906-a1", "hardening-p3.2-20260906-a1", "hardening-p3.2-fix1-20260907-a1", "hardening-p3.345-20260907-a1", "hardening-p4-sizing-20260907-a1", "hardening-p4-defer-20260907-a1"]
tier: "hasty"
tier-reason: "Concurrency is a blocking gate on using Yinz at all; every known blocker is traced to a named producer and fixed at that producer, not patched per symptom. Scope is fixed (four phases, non-negotiable), deferral is forbidden, ambiguity is decided upstream. Small committed work riding Patrick's settled order."
created_at: "2026-09-04"
updated_at: "2026-09-07"
metadata:
  type: "plan"
---

# HASTY PLAN: v0.3 Concurrency Hardening

> ## ✅ PLAN CLOSED — 2026-09-07. Phases 1-3 delivered; Phase 4 carved out to v0.3-M9.
>
> **Phases 1, 2 and 3 are COMPLETE. Phase 4 was SIZED, found to be a milestone, and moved out of
> this plan under Patrick's signed override — the decision and its evidence are in `#### Phase 4`
> and `## Future Requirements / Revisit` below. The scope-exit release pass is now v0.3-M9's
> mission, and every section of this banner still applies to whoever picks that milestone up.**
> Branch
> `feat/v0-3-m8-concurrency-completion`, tree clean, PR #91 open into `main` (that PR carries
> v0.3-M8; this plan's commits ride the same branch behind it and are not in that PR's body).
>
> ### Verified state — a full gate ran at Phase 3 close
>
> `fmt --check` clean · `clippy --workspace --all-targets -D warnings` clean · `test --workspace
> --no-fail-fast` **135 test targets observed, zero failures** · `build --workspace --release`
> clean. **There are NO expected test conditions on this branch. No planned-RED files, no ignored
> pins. Anything red is real.**
>
> ### What Phases 1–3 established and fixed
>
> - **FRAGO 001** — no heap local is EVER released at scope exit. Verified structurally (three
>   free-emission sites in codegen, none a scope exit) and empirically (2,000 iterations → 4,000
>   allocations, zero frees, exactly linear). An unbounded leak **inside a single run**, not a
>   long-lived-server problem. **This is Phase 4's target and nothing else's.**
> - **FRAGO 002** — the six reported defects were three clusters and one singleton, and FRAGO 001's
>   leak is the ancestor of NONE of them. A missing free is a leak; those were reads of wrong bytes.
> - **FRAGO 003** — the `fr23` red was a stale test, not a live use-after-free. That file is now a
>   green regression lock at 17/17 despite its name.
> - **Phase 3 fixed, at producers:** the crossing scan now scans a suspending statement's own
>   operands (both M8 fuzzer defects, one fix — two 256-seed sweeps return zero findings); the
>   language has ONE authoritative owned-copy operation (`owned_copy_plan` / `emit_owned_copy`,
>   `AliasNoOp` deleted); the spawn path reads ownership from `effective_ownership::provenance`
>   instead of matching syntax; `errors` checked-ness follows binding identity; and the
>   `errors`-field list is one table both typeck and codegen consume.
>
> ### ⚠️ Phase 4 — SIZED AND MOVED OUT; these three are now v0.3-M9's inheritance
>
> 1. **It may not be a phase.** M8 Phase 7's evidence called the scope-exit release pass "a
>    milestone of its own, not a phase," and this plan's own Phase 4 section carries that as a
>    signed risk gate. **Size it before entering it**, and if it is a milestone, say so rather than
>    absorbing it.
> 2. **Two deferrals trigger ON Phase 4** — parked 67–71 record them. A deep array copy's items are
>    not released, and a spawn-cloned map is not released. Both are strictly better than the alias
>    they replaced, but **if Phase 4 splits into its own milestone those deferrals outlive "next
>    phase" by a lot**, and the map one grows heap per iteration in a spawn-in-a-loop.
> 3. **The ordering constraint is SATISFIED.** C2 (the owned-copy work) closed before Phase 4
>    opens, which was required: one of its members was a premature free, and a release pass landing
>    first would have turned a dangling read into a double-free.
>
> ### Open, deliberately, with no FRAGO authorising a fix
>
> **Parked 32 is LIVE.** Repeated `.failed()` checks on one binding inside an errors-capable
> function both evaluate. Confirmed on base `d0c46b3` AND on HEAD. Its producer is now named for
> the first time: `resolve_ident` auto-narrows an `ErrorsCapable` binding on ANY read — including
> the read that is itself a `.failed()` receiver — and codegen caches the extracted success value
> and hands it to the second `.failed()`, which dereferences a string's own bytes as an error
> pointer. It is NOT the sibling of the defect Phase 3 fixed; different producer entirely. The
> exact repro is in parked 32. It was deliberately not fixed: 3.5's charter was read-only and no
> FRAGO authorised it.
>
> ### Standing traps — each of these cost a round to learn
>
> - **`cargo test --workspace` needs `--no-fail-fast`** (parked 52). Without it cargo stops at the
>   first failing TARGET and reports on a prefix while reading like a full-suite verdict. That is
>   how a five-phase-old defect survived nine gates.
> - **A `registry/features.toml` edit puts THREE consumers in the lane** — `jargon_audit`,
>   `ynz-registry` and `ynz-tmgrammar` (parked 51). The third is the one every earlier lane rule
>   omitted, and its omission left a committed artifact stale for five phases.
> - **A zero corpus delta is evidence about the corpus, not proof about the language.** Recorded
>   because the conductor reported one as the latter, shipped a false rejection behind it, and a
>   reviewer caught it.
> - **Two contention flakes are known and recorded** (parked 50, 52):
>   `timed_out_program_leaves_no_descendant_process_running` and
>   `v0_3_m4_p3_cross_give_generic_not_over_rejected`. Both pass in isolation; both passed in the
>   Phase 3 closing gate. If one fails, rerun it alone before calling it a regression.
> - **`~/.claude/tools/plan-lifecycle.py` does not exist in this environment**, so
>   `.claude/planning/_index.md` is not regenerated by anything despite CLAUDE.md saying it is.
>   Edit it by hand when a plan changes status, or the index rots.

## 1. Situation

**Conductor's hazard sweep** (facts grounded in M8 Phase 7 completion, Phase 8 fuzzer findings, and roadmap FR audit):

- **Data / heap-value corruption**: crossing locals sent into channels after suspension read back corrupted; capacity-forced-blocking sends read garbage; no scope-exit release pass exists. **Verified**: `crates/ynz-codegen/src/emit.rs` emits free calls at only three places — the background-arg glue, the channel element-glue table, and the spike trampoline — and `ynz_handle_free` has zero call sites. **VERIFIED 2026-09-05 (FRAGO 001)**: an ordinary `array`/`map`/`string`/promoted-`maybe` local is NEVER released at scope exit — not at function return, not at block exit, not per loop iteration. 2,000 iterations produced 4,000 allocations and zero frees, growing exactly linearly. It is an unbounded leak inside a single run, not a long-lived-server problem. **And FRAGO 002 established it is the ancestor of NONE of the corruption defects** — a missing free is a leak; those are reads of wrong bytes. It is Phase 4's target, alone. **Evidence**: M8 FR #11(a), FR #11(b), and `.claude/plans/parked.md` entry 49. **Neither defect is RED-pinned — there is no committed fixture for either**; `crates/ynz-driver/tests/fuzz_grammar/` holds only `README.md` and `mod.rs`. They reproduce by removing that file's two generator guards, and its doc comments there are the committed record of what was measured.

- **Money / irreversibility**: live defects in the released compiler that corrupt user data on send/receive. **Evidence**: M8 Phase 8 fuzzer, FRAGO 015 findings 1–2.

- **Prod-state**: `let h = background work()` creates an immortal task when `h` goes out of scope; no language-level stop mechanism, only a workaround (send a signal down a channel the task receives on). **Evidence**: M8 FR #3 (re-deferral), v0.3-M8 Phase 7's guard (Tier 3 lint), the design doc `IMP-no-function-coloring.md` "Task Cancellation."

- **Security / injection / PII**: not directly applicable (concurrency bugs are memory-safety, not injection). **N/A**.

- **Reversibility**: Phase 2's diagnosis and FRAGOs are non-reversible writes to this plan's audit.md. Phase 3's fixes are live in the compiler. No rollback path per design — rollback is a deferral to the hotfix branch if a FRAGO introduces a regression. **Not a floor on risk**, only an acknowledgment that this plan's discovery work is immutable once landed.

- **Concurrency / races**: the entire plan's domain. **Covered in all four phases**.

- **Auth**: N/A — Yinz has no auth surface.

- **External deps**: Tokio preemption model is already committed (M1 Architectural Decision locked). No new external dependencies introduced. **N/A**.

---

## 2. Mission

Trace every concurrency blocker discovered in v0.3-M2 through M8 to its named producer; cluster findings by ancestor; fix each producer once per cluster, never patch per symptom; retire all live exposure (data corruption, immortal tasks) and close all channels (M8 Phase 1–2's remaining deferrals); end state: programs using the default auto-concurrency produce correct output, and spawned tasks can be stopped by the language not by manual workarounds.

---

## 3. Execution

### 3.1 Intent & End State

> **CLOSE-OUT CORRECTION, 2026-09-07.** Of the three behaviours the Purpose names, **the first was
> delivered and the second and third were NOT.** An ordinary local is still never released at scope
> exit, and a handle binding's scope end still does not stop its task. Both belong to the scope-exit
> release pass, which was sized at close, found to be a milestone rather than a phase, and carved out
> whole to **v0.3-M9**. Key task 4 below and the second half of "What done looks like" describe that
> milestone's mission, not this plan's delivered result. This plan closed at three phases, honestly
> and deliberately — see `## Future Requirements / Revisit`.

**Purpose**: Concurrency works. A program using `wait` and `background` with channels produces correct output; an ordinary local is released when its scope exits; a handle binding's scope end stops the task (or schedules cancellation at its next suspension). Every finding from M2–M8's deferral sections and audit is the producer of itself, not a downstream symptom being patched elsewhere.

**Key tasks**:
1. Audit every concurrency plan (M2–M8) into one consolidated blocker register with durable homes.
2. Diagnose each blocker to its producer with evidence-backed probes; output FRAGOs one per producer.
3. Execute each FRAGO (steps will be defined by Phase 2's diagnosis; cannot pre-specify).
4. ~~Land the scope-exit release pass as the general mechanism for cleaning up locals on all control-flow edges.~~ **CARVED OUT to v0.3-M9, 2026-09-07 — NOT delivered by this plan.**

**What done looks like**: M2–M8 deferrals are either fixed or re-deferred with a new trigger in the roadmap's own registry; M8 Phase 8's two fuzzer-surfaced defects (neither RED-pinned today — pinning them is Phase 2's own output) are diagnosed to their producers and RED-pinned (no finding remains undiagnosed); Phase 7's re-deferral stands with Patrick's signature and a durable record of its evidence; ~~scope-exit releases are emitted at every block exit, loop-iteration end, and function return for every local type (handles, arrays, maps, strings, channels, promoted maybe/union cells); background tasks can be stopped at language level, not by manual channel workarounds.~~ **CARVED OUT to v0.3-M9, 2026-09-07 — NOT delivered by this plan.**

---

### 3.2 Concept

**Phase 1** — read-only audit of every M2–M8 plan's `## Future Requirements / Revisit` section and the roadmap's own Capability Ledger, producing one consolidated blocker list with durable homes (roadmap Capability Ledger, `registry/features.toml` `[[deferred_*]]` entries, `.claude/plans/parked.md`). M8 has been audited; M2–M7 have not. Output is a cross-referenced index, not code.

**Phase 2** — diagnosis phase answering two questions (exact questions given in Phase 2 task block): does an ordinary heap local EVER get freed at scope exit, and do several Phase 8 findings share one ancestor producer. Two probes (alloc counter, IR read) plus two optional follow-up probes (call-chain verify, owner-type classification). Output: one FRAGO per confirmed root cause, appended to this plan's `audit.md`. Phases 3 and 4 are **blocked** on Phase 2's FRAGO list — neither phase executes until the producer list is settled.

**Phase 3** — execute the FRAGOs. Phase 2 has delivered (FRAGO 001, FRAGO 002), so Phase 3's steps are now **written out as a checklist in its own section below** — three clusters and one singleton, ordered by what a user experiences. Discipline: one RED pin per FRAGO before any fix; fix at the most upstream reachable point (the producer, not a symptom); one fix per ancestor, never one patch per symptom. One session minimum per FRAGO (diagnosis from Phase 2 already done).

**Phase 4** — the scope-exit release pass. Every local released at scope exit, with `background` handles as ONE ARM of the general mechanism — never a handle-only pass. Retires the Tier 3 lint from v0.3-M8 Phase 7's guard, flips the two pin tests in `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs`, and retires the `background-handle-cancel-injection` registry entry. **RISK GATE (HIGH, signed override required if Phase 4 overruns Phase 3)**: v0.3-M8 Phase 7 concluded this pass is "a milestone of its own, not a phase." Phase 4 may split into its own milestone once Phase 2 sizes it; decide at Phase 3 close before entering Phase 4.

**Handoff between phases**: Phase 1 output (blocker list index) is read into Phase 2. Phase 2 output (FRAGO list) is read into Phase 3 and Phase 4. No phase output loops back to an earlier phase.

---

### 3.3 Phases

#### Phase 1 — Complete the Blocker List

**Task & Purpose**: Audit every concurrency plan from v0.3-M2 through v0.3-M8 into one consolidated register. No code changes. Read-only.

**Rationale**: Fixing from an incomplete list is how a blocker gets discovered halfway through Phase 3. One source of truth (the roadmap's Capability Ledger) has been de-duplicated; two other homes (`.claude/plans/parked.md` and `registry/features.toml`'s `[[deferred_*]]` entries) may have diverged. M8 Phase 9 checked its own deferrals; M2–M7 have not.

**Steps**:
1. Read `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md` — search `Capability Ledger` section. Count rows, verify no `STALE` mark or duplicate entries.
2. For each M2–M7 plan in `.claude/planning/done/`, read the `## Future Requirements / Revisit` section. Record every entry by ID (FR #N) and plan (2026-XX-v0-3-mN).
3. Grep `.claude/plans/parked.md` for entries mentioning concurrency, channels, `background`, handles, `wait`, or scope-exit. Record by item number.
4. Grep `registry/features.toml` for `[[deferred_language_feature]]` and `[[deferred_tooling_feature]]` entries with `why` or `substitute` text mentioning concurrency. Record by name.
5. Cross-index the four sources (roadmap Ledger, M2–M8 FR sections, parked.md, registry). Flag every entry that appears in only one source (stale, needs home). Flag every entry that appears in two+ sources with different descriptions (reconcile).
6. Produce a consolidated list with four columns: (blocker ID, short what, durable home by source, FR cross-reference). State plainly any missing homes — do not invent one.

**Exit criteria**: No deferral in any M2–M8 plan lacks a durable home in the roadmap or registry. Every entry with multiple sources has identical descriptions, or explicit reconciliation is recorded. M2–M7 are audited by name; M8 is spot-checked (Phase 9 already audited it).

---

#### Phase 2 — Diagnosis

**Task & Purpose**: Answer two questions with evidence-backed probes. Output: one FRAGO per confirmed root cause. Read-only plus two targeted probes.

**Question (a)**: Does an ordinary heap local EVER get freed at scope exit? **Claim to verify**: `crates/ynz-codegen/src/emit.rs` emits free calls at only three places: the background-arg glue, the channel element-glue table, and the spike trampoline. `ynz_handle_free` has zero call sites. If true, an ordinary `array`/`map`/`string` local's heap buffer may never be freed when its function returns (the one producer of the scope-exit leak class).

**Probe (a)**: Grep `emit.rs` for `"ynz_array_drop"`, `"ynz_free"`, `"ynz_channel_free"`, `"ynz_map_drop"`, `"ynz_string_free"`, `"ynz_handle_free"`. Record every call site (file:line, function name, context). Verify (count, context match, zero for `ynz_handle_free`).

**Question (b)**: Do these share one ancestor: M8 FR #11(a) (crossing-local heap-channel-send corruption), M8 FR #11(b) (capacity-forced-blocking channel send garbage), M8 FR #9 (background-arg escape door #4), M8 FR #10 (`.copy()` catch-all aliasing), and parked items 32/33/34 (three `errors`-surface defects discovered in Phase 4)? **Suspicion**: several are one bug — heap values crossing task and channel boundaries, either aliased or freed prematurely.

**Probe (b1)**: Reproduce both defects from the fuzz harness, which is the only mechanism that produces them today. **There are no committed fixture programs for either — neither defect is RED-pinned anywhere in the tree** (`.claude/plans/parked.md` entry 49), and `crates/ynz-driver/tests/fuzz_grammar/` contains exactly two files, `README.md` and `mod.rs`. Do not go looking for a fixture; generate the programs. In `crates/ynz-driver/tests/fuzz_grammar/mod.rs`: remove `Builder::suspension_seen`'s reuse guard to surface (a) (a 256-seed sweep produced 35 findings, 28 of them silent wrong output at exit 0), and remove the `send_count`-versus-capacity floor to surface (b) (6–7 findings per 256 seeds). Read that file's doc-comment narrative at both guards FIRST — it is the committed record of what was already measured, including the corrected symptom rates from Phase 8 round 4. Then capture a minimal reproducing program for each, compile both with IR emit, and diff the send/receive paths against a passing baseline. Record: which codegen path each takes, and whether they share one. **Capturing those two minimal programs as committed RED pins is itself an output of this probe** — the absence of one is why this diagnosis costs a probe instead of a read.

**Probe (b2)**: Optional, if (b1) suggests a shared path. Read the shared path (e.g., channel-send lowering, crossing-local frame-slot machinery). Classify: does the path assume a local outlives a suspension, or does it assume the local is freed before entry? Is the assumption checked?

**Exit criteria**: Question (a) is settled with a verified count. Question (b) is settled with evidence-backed clustering. FRAGOs are written: one per confirmed producer, including (1) what the producer emits, (2) which Phase 1 blocker(s) it produces, (3) the evidence (probe results, code read, failed test), (4) any preconditions for Phase 3 fix (e.g., loom substrate must exist, must not run during a phase that also reverts code). **MET when FRAGOs are appended to this plan's `audit.md` and Patrick has reviewed them** (sign-off gate before Phase 3 starts; see Coordinating Instructions below).

---

#### Phase 3 — Execute the FRAGOs

**Task & Purpose**: Deliver one fix per FRAGO at its identified producer.

**Phase 2 has now delivered its diagnosis** (FRAGO 001 and FRAGO 002 in this plan's `audit.md`),
so the steps below are no longer unspecifiable. Six reported defects resolved into **three
clusters and one singleton**; the checklist is ordered by what a user actually experiences, which
is the ordering FRAGO 002 recommends and its reasoning is recorded there.

Everything NOT on this list stays parked and is explicitly out of Phase 3's scope — the ten
M2–M7 items recovered as `.claude/plans/parked.md` entries 53–65 cost parallelism, not
correctness, and no program breaks because of them. Patrick's ruling, 2026-09-05: as long as they
are in parked, they stay in parked.

**The fixes, in order:**

- [x] **3.0 — RED pins first, before any fix.** Commit five probe programs from
      `target/p2b-probe/` (gitignored) into `crates/ynz-driver/tests/fixtures/` as failing tests:
      **A** (array before `wait`, sent after → SIGABRT in the default mode), **D** (no channel at
      all — array arg to a suspending user function → prints 6 for 3, exit 0), **G** (`number`
      into a capacity-1 channel with blocking sends → wrong value at `-O0`), **J** (int-local twin
      of G → prints heap addresses), **N** (`.copy()` on `fixed<T>` then mutate the copy → the
      source changes). Each must FAIL on today's tree before its fix lands; a pin that passes
      before the fix is measuring nothing.

- [x] **3.1 — C1: the crossing scan skips a suspending statement's own operands.** THE priority
      and it is not close: silent wrong output, exit 0, **default optimized mode, ordinary code**
      (probe D — no channel, no `background`, no `.copy()`, no `errors`). Producer:
      `collect_crossings_in_stmts` in `crates/ynz-typeck/src/check.rs` — once `past_wait` is true,
      a statement that is itself a suspension point records its result-binding but never has its
      own operands scanned; the direct suspending forms fall through `_ => {}` while only the
      `If`/`While`/`For`/`Match` arms call `collect_ident_refs_in_stmt`. Closes **M8 FR #11(a) AND
      FR #11(b)** — one fix, both symptoms, proven by the shared control (one harmless read of the
      local before the suspending statement fixes both).
      - [x] **Precondition, measure before landing:** widening the crossing set pushes more locals
            through `suspension_guards_fire_for_fn`, and types that cannot be frame-backed
            (`fixed`, `maybe`, union, `dynamic`, nested shape) currently force a decline. The fix
            may convert today's silent miscompiles into new declines or compile errors on programs
            that build today. Measure the delta on the existing corpus; do not assume it is free.
      - [x] **After the fix:** delete both fuzz-generator suppression guards
            (`Builder::suspension_seen`'s reuse gate and the `send_count`-versus-capacity floor in
            `crates/ynz-driver/tests/fuzz_grammar/mod.rs`) and run `YNZ_FUZZ_PROGRAMS=256`.
            Findings should go to zero. This also settles FRAGO 002's open question 2.
      - [x] **Fix round (dispatch `hardening-p3.1-fix1-20260906-a1`, review of `eb0aa0c`):** the
            hoisted scan compared a suspending statement's operands against the WHOLE `declared`
            set, and nothing asked whether a suspension actually fell between a local's
            declaration and the read — so a local declared after a suspension and read only in
            operands evaluated before the next one was rejected
            (`a maybe<int> value cannot yet cross a wait`). Fixed at the same producer, which
            also kills the pre-existing instance of the same imprecision (no later suspension at
            all). Three fixtures + `tests/post_suspension_local_not_crossing.rs` pin the
            false-rejection direction; the fuzz generator's capacity draw regains the
            slack-buffer regime it lost while widening.
      - [x] **Correct the record while here:** `mod.rs::take_or_make_array`'s doc comment claims
            this is "specific to the channel-transfer path" — false (probe D). And `mod.rs`
            contradicts itself on Int; the cautious `FeedFn::send_count` comment was right and
            parked 49(b) relays the wrong one. The discriminator is not the element type, it is
            whether a local is read by a statement that suspends.

- [x] **3.2 — C2: no authoritative per-type owned-copy operation.** Two independent per-type
      dispatches answer "give me an independent copy of this heap value" and both default to
      returning the receiver's own pointer: `prepare_bg_arg_for_ctx`'s `array<pointer-elem>`
      branch and `_` arm, and `copy_lowering_arm`'s `AliasNoOp`. Closes **M8 FR #9** (a live UAF,
      RED-pinned) and **M8 FR #10** (live silent-wrong today — probe N).
      - [x] **DECIDED by Patrick, 2026-09-06 — no longer blocking.** The ruling, in one line:
            **`.copy()` returns a genuinely independent value, copied all the way down, for every
            type where independence is meaningful — and is a COMPILE ERROR where it is not.
            Nothing silently aliases, ever.**
            - **Deep, not shallow.** A copy that shares an inner value is the exact defect this
              cluster exists to remove; shipping a shallow copy would re-introduce it one level
              down and call it a design. Golden Rule 2 decides the tie against Golden Rule 10
              here: a junior developer must be able to predict what the line does without reading
              documentation, and "sometimes independent, sometimes not, depending on how nested
              your value is" fails that outright. If deep copying ever proves too slow on a real
              workload, the answer is an explicit cheaper operation with its own name — never a
              silent reinterpretation of `.copy()`.
            - **Refusal is a real answer, and it must be loud.** Where an independent copy is
              meaningless or unsafe — a `channel`, a task handle — `.copy()` is rejected at
              compile time with three-slot teaching text (WHAT / WHAT-INSTEAD / WHY) per
              `.claude/rules/teaching-surfaces.md`, naming what to do instead. A refusal the user
              can see beats an alias they cannot.
            - **`AliasNoOp` does not survive as a silent behavior.** Every type currently in that
              arm becomes either a real deep copy or a ratified compile-time refusal. There is no
              third bucket, and "returns the receiver's own pointer while claiming to copy" is not
              a design position — `.claude/rules/no-duct-tape.md` makes leaving it a deferral that
              would need all four fields, and it has none.
            - **One routine, two call sites** (unchanged from below): the same clone routine feeds
              `prepare_bg_arg_for_ctx`, so the background-argument path and `.copy()` can never
              again disagree about what an owned copy is.
      - [x] **One shared clone routine, two call sites rewired** — not two new per-type tables.
            Two fresh tables rebuild exactly the twin `.claude/rules/authoritative-derivation.md`
            forbids, and that is the reason these are one cluster rather than two items.
            **DONE** (dispatch `hardening-p3.2-20260906-a1`): the table is
            `ynz_typeck::owned_copy::owned_copy_plan` (exhaustive over `Type`, no `_` arm, two
            answers per type — a copy strategy or a `CopyRefusal` carrying its three teaching
            slots). The emitter is `ynz_codegen::emit::emit_owned_copy` (exhaustive over
            `OwnedCopy`, no `_` arm), consumed by BOTH `lower_postfix_op`'s `.copy()` arm and
            `prepare_bg_arg_for_ctx`. `copy_lowering_arm` / `CopyLowering` / `AliasNoOp` are
            deleted; `types::copy_is_independent` is now a `pub use` re-export of the derived
            predicate, not a body. `copy_parity_tests` kept and extended with the binding the
            compiler cannot give: every plan matches the `Type` shape its emitter destructures.
      - [x] **Fix round (dispatch `hardening-p3.2-fix1-20260907-a1`, review of `6be6773`):**
            three seats fired. The spawn path's de-dup guard was keyed to `Expr::PostfixOp{Copy}`
            plus `Type::BuiltinArray` while `map`, `maybe` and `fixed` had just started
            allocating, so each of those double-copied and leaked the first copy (measured: a
            `map` spawn went 11 → 16 allocs, a `maybe` spawn 2/2 → 3/2 — a leak from zero).
            Both facts the guard was guessing at now come from their producers: typeck records
            `background_arg_sole_holder` from `effective_ownership::provenance`, and the
            per-plan storage answer is one non-wildcard match over the same `OwnedCopy` the
            emitter destructures. The `give_needs_no_copy` arm's invariant ("`Give` means the
            binding was consumed") was FALSE on the default-deny route and let
            `background eat(b.items)` share the parent's map (99/99 → 99/1); it now reads the
            consuming route. `IMP-ownership.md` and `REF-ownership.md` both stated things this
            commit made false and were rewritten. Three RED-verified pins added; parked 67–71.
      - [x] **HARD ORDERING CONSTRAINT — C2 closes before Phase 4 opens.** HONOURED: no
            scope-exit release was implemented; C2 closed first. FR #9 is a *premature
            free*: the ladder frees a clone the parent still points at. If Phase 4's scope-exit
            release pass lands first it will emit frees on aliased pointers and upgrade a dangling
            read into a double-free.

- [x] **3.3 — C3: flow-sensitive `errors` state keyed by name, not by binding identity.**
      `errors_failed_true_branch` and its siblings key on a bare `String`; `check_stmt_if`
      push/pops `self.scope` around the body while the errors sets are not scope-aware, so a
      shadowing inner `let` inherits the outer binding's checked status. Closes **parked 33**.
      Compile-time hole, no memory-unsafety — codegen's `br`/`phi` defense makes the observable an
      empty string rather than a crash.

- [x] **3.4 — S1: the `errors`-field surface is two unbound lists.**
      `EC_FIELDS_REQUIRE_FAILED_CHECK` admits four fields; codegen's `Type::ErrorsCapable` field
      arm lowers one and hard-errors on the rest. Closes **parked 34**. Ranked last despite being
      the easiest because it is LOUD and self-identifying ("This is a compiler bug") — nobody is
      silently misled. Pair it with 3.3 in one session; same surface.
      - [x] **Fix upstream, not by adding three arms.** Make the field list ONE shared enumeration
            with a parity test mirroring `copy_parity_tests` (which already binds
            `copy_lowering_arm` to typeck's `copy_is_independent`), so a fifth
            admitted-but-unlowered field becomes a build failure instead of a user-facing ICE.

- [x] **3.5 — parked 32: an archival read BEFORE any session is budgeted.** Two shaped repro
      attempts in Phase 2 both produced correct output, and parked 32's own record says half was
      fixed in round 3 by `restore_ec_receiver_ty`. Recover the round-3 executor's exact repro
      from the M8 plan's `audit.md` (`m8-p4-fix3-20260904`, base `d0c46b3`) and re-run it on HEAD.
      **It may not exist.** Do not budget a fix session before this read.
      **RESULT (2026-09-07): it LIVES — reproduced on `d0c46b3` and again on HEAD (both markers
      print for an always-succeeding `errors` call). NOT C3's sibling — a distinct producer in
      `resolve_ident` + codegen's `Expr::Ident`/`errors_capable_locals` handling, named in full
      in `parked.md` entry 32. No fix budgeted here per the read-only charter; left for its own
      diagnosis-then-FRAGO cycle.**

**NOT in Phase 3, stated so it is not mistaken for dropped:** FRAGO 001's finding — that no heap
local is ever released at scope exit — is a real, verified producer, but it is **Phase 4's**
target, not Phase 3's. It is the ancestor of none of the defects above (a missing free is a leak;
these are reads of wrong bytes), and FRAGO 002 records why in full.


**Discipline**:
- One RED pin per FRAGO before any fix (a failing test that will pass after the fix, used to verify the fix is real and not a no-op).
- Fix at the most upstream reachable point (the producer, not a symptom patch).
- One fix per ancestor; if two FRAGOs share a producer, one fix handles both.
- When a fix touches the crossing-local or frame-slot machinery, re-read `authoritative-derivation.md` before writing code — parallel implementations diverge silently.

**Exit criteria**: Every FRAGO has a RED pin and a fix. All pins flip to GREEN. The two M8 Phase 8 fuzzer findings (the genuine defects FRAGO 015 names) have either been fixed or re-deferred with explicit evidence in the plan's Future Requirements section. Clippy/fmt/test suite clean. **MET when Phase 3's work is merged to this plan's branch** (code review happens in parallel; see Coordinating Instructions).

---

#### Phase 4 — The Scope-Exit Release Pass — **NOT EXECUTED HERE; CARVED OUT TO v0.3-M9**

> **Signed override, 2026-09-07 (Patrick).** The HIGH risk gate below fired. Phase 4 was sized
> before entry, per that gate's own instruction, and the sizing confirmed M8 Phase 7's original
> call: this is a milestone, not a phase. It is deferred WHOLE to **v0.3-M9 (the "drop-story"
> milestone)**, which opens immediately — this is a re-carve, not a park. The four deferral fields
> and the sizing evidence live in `## Future Requirements / Revisit` below and in this plan's
> close-out commit (`git log --grep=drop-story`). Everything from here down is v0.3-M9's inherited
> mission statement, kept verbatim and unexecuted.
>
> Two corrections v0.3-M9 must carry, both found by the sizing pass:
> - The two test names in **Key outputs** below **do not exist in this repo**. The real pins are
>   `v03_m8_handle_scope_pin.rs::handle_leaving_its_block_does_not_cancel_the_child_today` and
>   `::no_handle_free_is_emitted_at_a_handle_bindings_scope_exit_today`, both currently PASSING as
>   deliberate pins of today's behaviour. They go RED when the pass lands and are then REWRITTEN,
>   not flipped. Do not go hunting for XFAIL markers that were never there.
> - **`string` has no release symbol at all**, deliberately: its bytes are raw-`malloc`'d, invisible
>   to the alloc counter, and freeing them may be unsound as currently built. That decision exists
>   only as an inline code comment in `emit.rs` and is absent from
>   `docs/internal/implementation/IMP-strings.md`. It is a design question and is v0.3-M9's FIRST
>   decision, before any release code is written.


**Task & Purpose**: Emit free/release calls at every scope exit for every heap-backed local type (`array<T>`, `map<K,V>`, `string`, `channel<T>`, promoted `maybe<T>` and union cells, `background` handle bindings). Handles are one arm of the general mechanism.

**Key outputs**:
- Codegen emits free calls at block exit, loop-iteration end, function return (both normal `ret` and early `return`), and state-machine frame retirement (both free paths: caller's `free_frame` after Ready, and spawned parent's drop ladder).
- Transfer rule (sent / given away / returned) skips release on the transferred binding (ownership moved, no release needed).
- The Tier 3 lint from v0.3-M8 Phase 7's guard is retired (no longer needed; scope exit now actually stops the task).
- `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs::test_background_handle_stopped_when_scope_ends_after_child_starts` and `test_background_handle_stopped_when_scope_ends_before_child_runs` both flip from XPASS/XFAIL to PASS.
- `background-handle-cancel-injection` registry entry is retired.

**Exit criteria**: Every local has a corresponding release call in all exit paths. Tests pass. Fuzzer clean. M7's two handle-scope pins are green. **MET when Phase 4 is complete and merged** (code review in parallel).

**Risk gate (HIGH, signed override required if Phase 4 overruns Phase 3)**: M8 Phase 7 evidence concluded this pass is "a milestone of its own, not a phase — 1–2 sessions per the roadmap's never-drop-locals row." Phase 4 may exceed this plan's time budget and require being carved into its own milestone. Decision point: at Phase 3 close-out, size Phase 4 against the remaining budget. If Phase 4 > budget, escalate to Patrick for deferral approval (with a signed override, record the decision in this plan's Future Requirements, update the roadmap status, and defer the pass to its own v0.3-M9 milestone). If Phase 4 ≤ budget, proceed.

---

### 3.4 Coordinating Instructions

**Phase 2 is a gate**: No Phase 3 work starts until Phase 2's FRAGOs are written and Patrick has reviewed them. Phase 3 and 4 are both blocked on this gate.

**Phase 3 review happens in parallel with Phase 3 execution**: Each fix lands as a PR with its RED pin and evidence documented in the commit message and plan audit. The executor does not wait for review between FRAGOs; multiple FRAGOs can be in flight at once. Patrick reviews each FRAGO's fix before it merges to the plan branch.

**Phase 4 size gate**: Before Phase 4 starts, record the estimated time budget (sessions, tasks). If the pass appears to exceed the per-phase budget, escalate to Patrick immediately. Do not code blind. Do not discover scope-exit release is its own milestone mid-implementation.

**Loom substrate precondition**: If any FRAGO's fix depends on loom model-checking (e.g., a synchronization-primitive change), confirm M8 Phase 3's loom substrate is still live and the new logic is model-checkable before writing code. Do not add loom-requiring changes to a phase where loom work is not already in scope.

**Crossing-local and frame-slot fixes**: If Phase 3 produces a FRAGO touching crossing locals or frame slots, freeze all other tree-mutating work in that session. Two concurrent code-reading seats (one FRAGO fix + one design review) on the same choke point have led to reversed state observations in the past (`m8-p3` incident); single-threaded discipline is the guard.

---

## 4. Sustainment

**Dependencies**: Tokio runtime (already embedded in `libynz_rt.a`; no new external deps). Loom for model-checking (already in dev-time `Cargo.toml`; production-build no-op per M8 Phase 3 proof).

**Environment**: Dev container (`docker-compose.yml`). `/tmp` is tmpfs with exec, required by `ynz run` which links a binary there.

**Tooling**: Alloc counter (Phase 2 Probe (a), to be built as a temporary instrumentation). Fuzzer (Phase 8 proves it runs; reuse its corpus and generators).

**Credentials**: None.

---

## 5. Command & Signal

**Ownership**: Patrick Rizzardi (this plan is driven by his settled order; every FRAGO decision gates on his sign-off).

**Cold-resume pointer**: Read this file top-to-bottom. Then read `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md` Capability Ledger to understand what "one consolidated blocker register" means. Then read M8's plan `## Future Requirements` section for the exact deferrals this plan inherits. Then Phase 1 begins.

**Audit trail**: Commit messages follow `Co-Authored-By: Claude Opus 5 (1M context)` convention + `Claude-Session: https://claude.ai/code/session_...`. Phase 2 findings and FRAGOs are written to `.claude/planning/done/2026-09-04-v0-3-concurrency-hardening/audit.md` (this plan moved `active/` → `done/` at close-out, 2026-09-07) (created at Phase 2 close-out, appended through Phase 3). Each FRAGO landing is recorded by session ID and executor name (e.g., `m8-p2-signoff-20260903`). Risk decisions (Phase 4 size gate) are recorded with Patrick's signature if a deferral is chosen.

---

## Design-Doc Alignment

**Governing design docs**:

1. **`docs/internal/implementation/IMP-no-function-coloring.md`** — the no-function-coloring model and Task Cancellation section. **Specifies**: (a) whole-program may-block analysis (M2 scope completed); (b) auto-inserted suspension points at call sites (M2 completed); (c) preemption-check insertion at loop back-edges and function calls (M1 completed as per Architectural Decisions); (d) auto-Arc sharing topology across `background` boundaries with read-only proof (M8 Phase 2 specified, Phase 5 implemented the BENEFICIAL-EMISSION subset). **Silent on**: the scope-exit drop mechanism for handles (Phase 7 re-deferred its entire design; Task Cancellation section says "tasks stop at scope end" but names zero codegen path to implement it). This plan's Phase 2 diagnosis answers half the silence (the producer is named and proven). **The other half — the codegen path itself — is v0.3-M9's, not this plan's**: Phase 4 was carved out and never executed, so the design doc's silence on the drop MECHANISM still stands, unanswered, and is M9's to close.

2. **`docs/internal/implementation/IMP-concurrency.md`** — core concurrency semantics (Suspension vs. Ordering, Reads vs. Writes, Loop Iterations). **Specifies**: auto-parallelization for independent operations; data-dependency and ownership-based ordering; `wait` as explicit ordering only (not suspension). **Silent on**: scope-exit release (the authoritative-derivation class — one drop-insertion pass, never two parallel implementations). ~~This plan's Phase 4 is the implementation of the unspecified mechanism.~~ **CARVED OUT to v0.3-M9, 2026-09-07 — NOT delivered by this plan.** The mechanism remains unimplemented and unspecified; v0.3-M9 owns both.

3. **`docs/internal/implementation/IMP-ownership.md`** — call-site ownership inference (`share`/`lend`/`give`), `.copy()` semantics, auto-Arc sharing-topology section (M8 Phase 2 added it). **Specifies**: transfer rule (sent, given, returned); effective-ownership proof for read-only inference. **Silent on**: scope-exit transfer handling (when a binding is transferred via `send`, does the scope-exit release apply to the original binding or does ownership move? The transfer rule is silent on where transfer happens in the control-flow edges). ~~This plan's Phase 4 must define the transfer-rule intersection with scope-exit enumeration.~~ **CARVED OUT to v0.3-M9, 2026-09-07 — NOT delivered by this plan.** v0.3-M9 must define it — and the sizing pass found the raw material is already there but out of reach: `Scope::consumed_classes` in typeck answers the question transiently and is persisted into no report codegen can read. Per `authoritative-derivation.md` M9 threads that answer rather than re-deriving it.

**Verification of cited specifications**:

- `IMP-no-function-coloring.md`'s "Task Cancellation" section (cited by heading, not line number — anchors in this repo have drifted 500–650 lines): correctly specifies suspension correctness (auto-inserted, no function coloring), correctly specifies that Tokio will stop a task at its next suspension once it's dropped — but does NOT specify the compiler-side mechanism to call the drop (emit.rs writes zero `ynz_handle_free` today). **Finding**: specification is incomplete; the mechanism is the silence this plan's Phase 4 fills.

- `IMP-concurrency.md` entire "Suspension vs. Ordering" section: correctly specifies what suspension and ordering are. Does NOT specify when locals are freed relative to control-flow edges. **Finding**: specification gap; not inconsistent with this plan, only incomplete.

- `IMP-ownership.md` transfer rule: specifies which bindings are consumed by a send/give/return. Does NOT specify whether a binding that is transferred also skips scope-exit release on the original declaration site (it does, per Phase 4 discipline). **Finding**: discipline is consistent with the rule's intent (ownership moved = no release on the original site); the plan's Phase 4 must implement the interaction explicitly.

---

## Invariants This Milestone Must Preserve

### Safety

> **CLOSE-OUT CORRECTION, 2026-09-07.** Two of the four rows below were written as satisfied ON THE
> STRENGTH OF PHASE 4, which never ran. They are marked NOT ESTABLISHED and inherited by v0.3-M9.
> A closed plan must not hand its successor a safety floor it never poured.

- **NOT ESTABLISHED — inherited by v0.3-M9.** ~~No use-after-free on local bindings (Phase 4's scope-exit release catches this).~~ Phase 4 was carved out; nothing releases a local at scope exit today, so this plan neither establishes nor tests this invariant. What IS true is narrower and worth saying exactly: FRAGO 002 established the leak is the ancestor of none of the corruption defects, and Phase 3 fixed those at their producers — so today's failure mode is a leak (memory retained), not a use-after-free (memory read after release). The invariant becomes live to PROVE the moment v0.3-M9 starts emitting releases.
- **PARTIALLY ESTABLISHED.** No double-free on local bindings. The transfer-rule skip and parity test this row names belong to Phase 4 and do not exist. What Phase 3 did deliver is upstream of it and load-bearing for it: ONE authoritative owned-copy operation (`owned_copy_plan` / `emit_owned_copy`, `AliasNoOp` deleted), which removed the premature-free member that would have turned a release pass into a double-free. That is why C2 had to close before Phase 4 could open — see the HARD ORDERING CONSTRAINT in step 3.2.
- **ESTABLISHED.** Channel send does not corrupt the payload (Phase 3's FRAGO for crossing-local + blocked-send — the crossing scan now scans a suspending statement's own operands; two 256-seed sweeps return zero findings).
- **NOT ESTABLISHED — inherited by v0.3-M9.** ~~Handle scope exit does not cause use-after-free in the parent (Phase 4 proves this via the two pin tests).~~ The two pin tests in `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs` currently assert the OPPOSITE of this row: they pin today's behaviour (the child is NOT cancelled, no `ynz_handle_free` is emitted) and pass. They go RED when v0.3-M9 lands and are then rewritten.

### Performance

No auto-promotion candidates identified for Phase 2–4's scope. Scope-exit release is mandatory overhead (correctness, not optimization), not subject to auto-promotion. Performance impact: one codegen path per exit edge per local type (overhead is the released-memory guarantee, not a perf choice to make cheaper).

### Teaching

Phase 3's fixes must update three-slot diagnostics (WHAT/WHAT-INSTEAD/WHY) where new compiler errors are introduced. The guard for M8 Phase 7's re-deferral is **already decided by Patrick (2026-09-04): a Tier 3 lint, NOT a muted hint** — he prefers a known deferred defect to nag loudly rather than sit in passive gray text, and a Tier 3 lint is visible in the editor AND in compile output where a muted hint is neither. It fires only on the exposing shape (a bound handle whose scope ends with no `wait` on it), never on every spawn. Its text carries the three slots: WHAT — this task keeps running after `h` goes out of scope; WHAT-INSTEAD — `wait` on it, or send it a stop signal (close the channel it receives on); WHY — nothing in Yinz releases a local when its scope ends yet, so the receipt going away does not stop the work. That lint is small separate work landing BEFORE this plan; **this plan's Phase 4 retires it.** Phase 3's fixes must carry three-slot text for any new compile error they introduce.

### Runtime Dependencies

Phase 3's FRAGOs may add new runtime calls (e.g., rebalancing a synchronization primitive). Each new call is a new runtime dependency; record it here and in the Kernel-Mode Behavior section. Phase 4's scope-exit release calls `ynz_array_drop`, `ynz_free`, etc. — already exist, no new dependencies.

### Kernel-Mode Behavior

Phase 3's FRAGOs: each new runtime call is classified by whether it's allowed in `--kernel` mode. Likely answer for scope-exit release: **allowed** (local cleanup is non-blocking, per-scope). If a FRAGO introduces a may-block runtime call, `--kernel` mode must reject it at compile time with a WHAT/WHAT-INSTEAD/WHY diagnostic. Phase 4's release calls are allowed in `--kernel` mode.

### Demo & Error Gallery

Phase 3's new compile errors are added to `examples/primantis-orders/m8_errors.ynz` with `// WHY: <diagnostic-class>` comments (or higher milestone's gallery if Phase 3 slip causes it to ship in M9). ~~Phase 4's handle-cancellation behavior is demonstrated in `examples/pirates-roster/entrypoint.ynz` with a spawned task that prints before the handle scope ends, showing the task actually stops (vs. running to completion).~~ **CARVED OUT to v0.3-M9, 2026-09-07 — NOT delivered by this plan.** **This demo does NOT exist** — verified absent from `entrypoint.ynz` at close-out. It could not exist: the behaviour it would demonstrate is not implemented. Building it is v0.3-M9's obligation under `plan-invariants.md`'s `### Demo & Error Gallery` rule, not a debt this plan discharged.

### Feature Registry Entries

Phase 3 may retire registry entries (e.g., `background-handle-cancel-injection` if Phase 4 ships, else stays deferred with new trigger). Phase 4 retires `background-handle-cancel-injection` if it ships. Record all entries touched by phase:
- **Retiring**: (deferred_language_feature — verified against `registry/features.toml`) `background-handle-cancel-injection` — Phase 4 closes the underlying defect; the Tier 3 lint is no longer needed. **DID NOT HAPPEN — Phase 4 never shipped (carved out to v0.3-M9, 2026-09-07). The entry was RETAGGED, not retired: its `triggers` field now names v0.3-M9, every other field byte-identical. Retiring it while the capability is still deferred would assert a false fact. The retirement above is v0.3-M9's to perform, when the pass actually lands.**
- **Modifying**: (deferred_language_feature) entries named by Phase 1's blocker audit may be modified with corrected descriptions if Phase 2's diagnosis changes their trigger or scope. Record each modification.
- **No new entries** expected from Phases 1–2 (diagnosis, no language surface). Phase 3 may add entries if a FRAGO introduces new muted-hint domains or lint rules (record if it happens).
- **Added by step 3.2's fix round** (dispatch `hardening-p3.2-fix1-20260907-a1`): one `[[diagnostic_template]]` entry, `SpawnArgStorageDiesWithTheFrame` — the OTHER `background`-argument refusal, which needed its own sentence shell rather than a reuse of `SpawnArgNotIndependent`'s: that shell says "this line still reads it after the task starts" and tells the reader to hand the value over instead, and both are false when the problem is that the value is kept with the frame (handing it over does not move it). Fires under either ownership label for that reason, where `SpawnArgNotIndependent` is `Copy`-only. No new keyword, banned_jargon, primitive_intrinsic, type_attached_constant, deferred_* or muted_hint_domain entries.
- **Added by step 3.2** (dispatch `hardening-p3.2-20260906-a1`): two `[[diagnostic_template]]` entries, `CopyNotIndependent` and `SpawnArgNotIndependent` — the two sentence shells for the ruling's refusals (`.copy()` and a `background` argument the spawner keeps reading). Both take their per-type `{detail}` / `{fix}` / `{why}` fills from the ONE owned-copy table, so the registry carries the canonical shell and never a second copy of the per-type text. No new keyword, banned_jargon, primitive_intrinsic, type_attached_constant, deferred_* or muted_hint_domain entries.

---

## Future Requirements / Revisit

**Phase 2 gate (before Phase 3 starts)**: Patrick reviews the FRAGO list and signs off on the producer clustering. If Phase 2 discovers that a blocker is unfixable in this plan's scope (e.g., requires redesigning the ownership system), it is re-deferred with explicit evidence and a new trigger.

**Phase 4 size gate — FIRED AND RESOLVED, 2026-09-07. Outcome: DEFERRED to v0.3-M9, opening immediately.**

- **WHAT**: the entire scope-exit release pass — every heap-backed local (`array<T>`, `map<K,V>`, `string`, `channel<T>`, promoted `maybe<T>`/union cells, `background` handle bindings) released on every scope-exit edge, with handles as one arm of the general mechanism.
- **WHY** (the real tradeoff, not "saves time today"): a read-only sizing pass measured **6+ distinct insertion-site classes across two parallel codegen implementations** (plain-`alloca` and state-machine frame-slot), per-local transfer tracking that exists in typeck (`Scope::consumed_classes`) but is **never surfaced to codegen**, recursive per-element release machinery that **does not exist for any container type** (the copy side recurses; the free side is flat), and an **unresolved soundness question on `string`**. 3-5 sessions minimum. Forced into this plan's remaining budget it yields a PARTIAL pass — and a partial pass converts a bounded, safe, per-run leak into use-after-free and double-free, which is strictly worse than the leak and is the exact defect class Phase 3 just closed. Golden Rule 3 (ownership-based memory management, no GC) makes this load-bearing, not cosmetic: a compiler that releases nothing is not doing ownership-based management. Load-bearing plus 3-5 sessions is a milestone by definition.
- **COST to fix later**: FRAGO 001's leak stays live and unbounded within a single run (2,000 iterations → 4,000 allocations, zero frees, exactly linear); parked 67-71's two deferrals — a deep array copy's items unreleased, a spawn-cloned map unreleased — stay live, and the map one grows heap per iteration in a spawn-in-a-loop.
- **GUARD CLAUSE — considered and declined, deliberately.** `no-duct-tape.md` requires that when a deferred risk has live exposure before its trigger and a **cheap in-scope guard exists**, the guard is taken now, alongside the deferral. The exposure is real (the leak is live from this commit until v0.3-M9 lands), so the clause was evaluated rather than skipped. **No such guard exists here.** The obvious candidate — mirroring M8 Phase 7's `background-handle-not-waited` Tier 3 lint for the general case — fails on signal, not on cost: that lint works because handles are RARE, whereas a "this local is never released" lint fires on every `array`, `map` and `string` in every program ever written in Yinz. A diagnostic that fires on all code teaches nothing and trains the reader to ignore the channel it arrives on, which fails Golden Rule 11 (the compiler is a teacher; the WHY must be specific and contextual) and inverts the teaching mission the lint tier exists to serve. The leak is also **bounded per run and memory-safe** — FRAGO 002 established it is the ancestor of none of the corruption defects — so the exposure it carries is resource growth inside one process lifetime, not a wrong answer. **The honest guard is the milestone opening immediately, which is the trigger below.** Recorded rather than built, per this rule's own "a deferral names the future fix" discipline.
- **TRIGGER**: v0.3-M9 opens **now**, not on a future condition. Its first decision is the `string` soundness question above; its first commit is a RED fixture, since **no committed fixture pins the leak today**.

Deferral mechanics executed alongside this close-out: roadmap Capability Ledger status updated, and the `background-handle-cancel-injection` registry entry retagged to a v0.3-M9 trigger. M8's re-deferral of handle cancellation (Phase 7 FR #3) is unaffected — it has its own separate trigger.

**Parked 32 stays parked, deliberately.** It is LIVE and its producer is fully named (see the banner), but it is an `errors`-surface defect — a wrong branch, not a wrong pointer — already routed by Patrick to the `errors`-surface hotfix branch. Folding it into a memory-management milestone is the scope drift `no-duct-tape.md` exists to prevent. Its own trigger stands: the next `errors`-surface session, RED fixture first.

**Phase 3 regression gate (after Phase 3 closes)**: If any FRAGO fix introduces a regression (a test that was green goes red), the fix is reverted and the blocker is re-deferred with the regression documented and a new trigger (e.g., "fix the regression before retrying").

**Contingency — Phase 3 discovers a blocker is deferred to a future release**: Phase 2 may discover that one producer cannot be fixed in this plan's scope (e.g., requires the drop-story milestone). That blocker is explicitly deferred in this plan's audit.md with four fields: WHAT (the blocker), WHY (the technical reason it's out of scope), COST (effort to fix later), TRIGGER (what must happen first).

