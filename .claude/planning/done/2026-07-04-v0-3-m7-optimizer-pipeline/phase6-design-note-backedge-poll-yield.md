# Phase 6 Step 1 — Design Note: SM Back-Edge Poll-Yield Codegen Transform

- **Plan:** `2026-07-04-v0-3-m7-optimizer-pipeline`, Phase 6 (plan.md §3.3). Gates Steps 2–7.
- **Session:** `executor-2026-07-17-phase6-designnote`
- **Governing doc:** `docs/internal/implementation/IMP-no-function-coloring.md` "Scheduler
  Preemption Model" (lines 218–262 at HEAD `a05aced`) — the compile-time-assisted safe-point
  model this note implements the loop-back-edge half of.
- **Governing risk:** R8 (plan ¶1, signed RISK OVERRIDE; conductor re-score 2026-07-17
  addendum 3 — UNCHANGED HIGH, Step 2 authorized once this note exists). Frame-layout /
  crossing-local questions are R8's family: maximum rigor, Paper-Trace everything.
- **Governing rule:** `.claude/rules/authoritative-derivation.md` — one authoritative producer
  per derived question; extending the existing walk is right, forking a twin is the bug.

All `file:line` anchors below were verified against the working tree at HEAD `a05aced`
(2026-07-17), not taken from the plan's pre-drift citations. The plan's cited anchor
`emit.rs:12356-12365` has drifted; current anchors are listed in §(a).

---

## (a) Which loops qualify

**Qualifying: every loop back edge inside a state-machine (wait-containing) function.**
Non-SM functions get nothing (see §(d)).

### Ground truth about today's loop-emission routing (verified, not assumed)

The plan's Step-2 warning ("verify which loop-emission path SM loops actually take") resolves
to a **two-path reality**, and this is the design's central composition fact:

1. **Loops whose body contains a suspension** route through the SM walker's dedicated loop
   arms, selected by `stmt_needs_sm_walker` (`emit.rs:5806-5810` — true iff the statement
   contains a `wait`, a suspending call, or a conduit suspend). The SM arms and their
   back-edge `emit_loop_preempt` calls (current anchors):
   - SM `while` — back edge at `emit.rs:7304`
   - SM `for` over range — back edge at `emit.rs:7594` (frame-resident idx, flushed pre-back-edge)
   - SM `for` over array — back edge at `emit.rs:7739`
   - SM `for` over fixed array — back edge at `emit.rs:7840`
   - SM `for` (remaining iterable arm) — back edge at `emit.rs:7976`
2. **A wait-free loop inside an SM function** — the EXACT starvation shape fixture (a)
   targets (hot CPU-bound no-call loop) — makes `stmt_needs_sm_walker` return **false** and
   is lowered by plain `lower_stmt` inside `lower_sm_block`'s else-branch, i.e. through the
   **plain** loop arms (`emit.rs:14003/14100/14186/14291/14378` + siblings), which carry
   Phase 4's `loop_stack_save`/restore (row 439) and a void `emit_loop_preempt` call.
   Plain `lower_stmt` has no access to `frame_ptr`/`state_blocks`/`pending_block` and
   **cannot emit a suspension**.

### Consequence — the transform widens SM routing; it does not merely edit the SM arms

To make a wait-free loop inside an SM function preemptible, that loop must itself become a
suspension-point-bearing statement and route through the SM walker. The design decision:

> **Inside an SM function, every `while`/`for` statement routes through the SM loop arms**
> (i.e. the loop-routing predicate becomes "is a loop AND we are emitting an SM function",
> OR the existing `stmt_needs_sm_walker` conditions), **and every such loop's back edge is
> one poll-yield suspension point.**

Loops already routed through SM arms (wait-containing) get the same back-edge poll-yield in
addition to their existing wait-site suspensions. The SM walker's fallback iteration forms
(string/shape iteration, `emit.rs` ~7830-7833 fallback comment) are today only reachable
wait-free (typeck's WaitInsideLoop guard rejects suspension bodies there); under the widened
routing these forms either (i) gain an SM arm too, or (ii) are explicitly excluded and named
as a residual sub-case in Step 7's doc rewrite. Step 2 resolves (i)-vs-(ii) by reading the
fallback's reachable forms; the default is (i) unless the arm's cost is disproportionate,
in which case the exclusion is surfaced, never silent.

**Nesting:** each loop level's back edge is its own suspension point (own continuation
state). Inner-loop back edges fire most often; outer back edges are cheap redundancy.

### The three authoritative walks that must agree (R8's core hazard)

A "qualifying loop back edge = suspension point" definition is consumed by THREE
pre-existing derivations. Per `authoritative-derivation.md`, the qualifying predicate is
defined **once** (one shared function, e.g. in `ynz_typeck`, since both crates consume it)
and threaded into all three — never re-derived per consumer:

1. **`count_suspension_points`** (`emit.rs:5671`) — sizes the `state_blocks` vector
   (`emit.rs:4855-4880`). Must count one extra state per qualifying back edge, or the
   continuation index overruns the vector (the exact M3d/M3e envelope-narrowing family —
   the `spike_extra_states` comment at `emit.rs:4859-4871` documents the identical
   prior-milestone bug shape).
2. **`crossing_local_names`** (`ynz-typeck/src/check.rs:8100`, with its
   `_with_cpu_spike`/`_with_provenance` variants and `locals_crossing_wait` at
   `check.rs:9061`) — decides which locals get frame slots. A back-edge yield makes every
   local **live across the back edge** (loop-carried values, the loop variable, anything
   read after the loop) a crossing local; if the crossing-set walk is not extended to treat
   qualifying back edges as suspension points, those locals resume from fresh, uninitialized
   allocas — the M3a silent-garbage miscompile, verbatim. **Extend the one existing walk in
   `check.rs`; do not compute a second "back-edge-crossing" set in codegen.**
3. **The SM walker's emission** (`lower_sm_block`/`lower_sm_stmt` routing + the loop arms) —
   must route and emit exactly the loops the counter counted and the crossing set assumed.

Step 2's exit proof for this section: a grep receipt showing exactly one definition site for
the qualifying predicate, consumed by all three walks (plan exit criterion "grep-verified, no
parallel frame-flush" extends naturally to "no parallel qualifying-predicate").

---

## (b) What the yield emits

At each qualifying back edge, replace today's unconditional void call
(`emit_loop_preempt`, `emit.rs:13271`) with a **conditional poll-yield**:

```llvm
back_edge:                                        ; end of loop body
  %should = call i1 @ynz_rt_check_preempt()       ; cheap budget check, §(c)
  br i1 %should, label %yield_K, label %header

yield_K:
  ; crossing locals are already frame-resident here BY CONSTRUCTION:
  ;   - per-statement flush discipline: flush_crossing_local_if_needed (emit.rs:6786)
  ;     runs after every non-wait statement in the SM walk, delegating to
  ;     flush_var_slot_to_frame (emit.rs:6577) — THE one authoritative per-type dispatch
  ;   - loop variables: flush_for_loop_var (emit.rs:~7433) / the SM-range idx flush
  ;     ("sm range flush next", emit.rs:~7590) — both delegate to flush_var_slot_to_frame
  ; store the continuation index via THE existing helper:
  call store_resume_point(frame_ptr, K)           ; state_machine.rs:134
  br label %sm_pending                            ; existing Pending return block

; state_blocks[K] (continuation state, resume path):
sm_sK:
  ; reload params + crossing locals via THE existing helper:
  call reload_params_from_frame(..., reload_crossing = true)   ; emit.rs:5851
  br label %header                                ; re-evaluate loop condition and continue
```

Reuse contract (R8 / authoritative-derivation — these are the ONLY frame-touching calls the
transform may emit; introducing any new frame-slot read/write path is a BLOCK):

| Concern | Reused authoritative machinery |
|---|---|
| Resume-point store | `store_resume_point` (`state_machine.rs:134`) |
| Crossing-local flush | `flush_var_slot_to_frame` (`emit.rs:6577`) via the existing per-statement/loop-var flush discipline — **no new flush call at the yield site**; the design relies on the already-enforced invariant that crossing locals are frame-resident after every statement. If Step 2 finds any SM loop arm where that invariant does NOT hold at the back edge (e.g. a value flushed only lazily), the fix is to extend the existing flush discipline at that arm, not to add a parallel yield-site bulk flush. |
| Resume reload | `reload_params_from_frame` (`emit.rs:5851`) with `reload_crossing = true`, exactly as every wait continuation state |
| Pending return | the function's existing `sm_pending` block (`emit.rs:4884-4885`) |
| State allocation | the same `*current_state` advance protocol every wait site uses (each qualifying back edge claims the next continuation index during the walk, matching `count_suspension_points`'s pre-count) |

**Resume target = loop header, not loop body.** The header re-evaluates the condition from
reloaded state; this is correct for all loop forms (the range/array idx is frame-resident
and was flushed before the back edge, so the header's reload sees the post-increment value).

**Interaction with Phase 4's `loop_stack_save`/restore (row 439):** the plain arms carry
stacksave/restore; the SM arms do not (verified — no `loop_stack_save` calls in the SM arms
at 7304-7976). When wait-free loops move from the plain path to SM arms, Step 2 must
Paper-Trace the alloca-growth question for those migrated loops in the SM context: SM loop
bodies place allocas in `sm_entry` (dominating, one-time) rather than per-iteration where
the walker manages them, but plain statements lowered by `lower_stmt` INSIDE an SM loop body
may still `build_alloca` per iteration. If migrated loops reintroduce per-iteration alloca
growth (the row-439 class), the SM arms need the same stacksave/restore pair — with the
save/restore respecting suspension (a stacksave'd pointer must NOT be restored after a
resume, since the resume call has a fresh C stack; the save must be re-taken per resume
invocation, i.e. inside the loop's SM-side preheader within the current activation, or the
restore skipped on the resume edge). This is a named Step-2 verification item, not settled
here — it is R8-family (frame/stack correctness) and gets its own Paper-Trace.

**O2 interaction:** the poll-yield branch is a real conditional in the loop; LLVM may
unswitch/hoist parts but cannot remove the `ynz_rt_check_preempt` call (external, side-effecting
by declaration). The existing golden lock (`golden.rs:563-581`) asserts the call's presence in
while-loop IR; Step 2 updates goldens for the new conditional shape.

---

## (c) The budget mechanism

`ynz_rt_check_preempt` (`ynz-runtime/src/runtime.rs:479`) changes from the M1 no-op
`extern "C" fn()` to a **cheap synchronous budget check returning a bool** — it decides
WHETHER to yield; it never yields itself (a synchronous `extern "C"` callee cannot yield the
enclosing Tokio task — Design-Doc Alignment divergence 1). The YIELD is codegen's branch, §(b).

- **Signature:** `pub extern "C" fn ynz_rt_check_preempt() -> bool` (C ABI `i8`; codegen
  truncates to `i1` for the branch). One function, one signature: the codegen declaration
  (`runtime_decls.rs:221-227, 724`) changes to match; **plain-loop back edges keep their
  existing call and discard the result** (they cannot yield; the call still ticks the budget,
  which is harmless and keeps a single ABI — no second entry point).
- **Mechanism (cheap path first):** thread-local `Cell<u32>` countdown (e.g. start 1024).
  Each call decrements; while nonzero → return `false` (the ≤5ns-per-call bound the Phase 0
  spike measured for the stub-call shape, `ynz-runtime/tests/spike.rs:336`, is the budget
  for this fast path). On reaching zero → reset the counter and compare a thread-local
  "last yield" `Instant` against the quantum; if elapsed ≥ quantum → record now, return
  `true`; else return `false`.
- **Quantum:** ~10ms default, per the IMP doc's locked "Time quantum" paragraph
  (`IMP-no-function-coloring.md:246`). Constant in the runtime for now; per-runtime
  configuration is the doc's stated model and can remain a constant until a config surface
  exists (no new user-facing surface in this phase).
- **No Tokio internals:** the check does not consult Tokio's cooperative budget (unstable
  API surface); it is a self-contained counter+clock. Tokio involvement is only the normal
  effect of the resume function returning `Pending` — the task yields to the worker, which
  schedules other ready tasks (exactly what fixture (a) proves).
- **Threading:** thread-locals make the budget per-worker, which is the correct granularity
  (starvation is a per-worker phenomenon).

---

## (d) Non-SM functions: NOTHING new — and the named residual

Loops in plain (non-SM, non-wait-containing) functions get **no new mechanism**: no frame
exists, a synchronous function cannot return `Pending`, and there is nothing to resume. Their
existing and only protection is **CPU-admission routing to the blocking pool** (shipped;
`IMP-no-function-coloring.md:248` — tasks whose call graph contains zero may-block calls are
routed off the I/O workers). Their back edges keep the discarded-bool budget call (§(c)),
which is deliberately inert for them.

**The named residual (unchanged by this phase, stated for Step 7's doc rewrite and plan
Future-Requirement §1075):** CPU-heavy code inside a non-SM function that admission
misses — concretely, a task admitted to the I/O pool because its call graph CAN suspend,
which then spends unbounded time inside a non-SM callee's hot loop (or loop-free CPU-bound
recursion, the shape the IMP doc already names at line 224). That code runs on an I/O worker
with no yield mechanism until it returns. Fixture (b) documents this as expected behavior.
Call-site checks (Steps 4–6), if shipped, narrow but do not close this residual: a call-site
check is likewise a codegen-emitted poll-yield site and can only yield where a frame exists —
i.e. at call sites INSIDE SM functions. A non-SM function's interior remains non-preemptible
under every mechanism this phase can ship; the blocking-pool routing is, by design, the
protection for that shape.

---

## Step-3 fixture implications (recorded here so the RED-first ordering is settled)

- **Fixture (a)** (SM-positive starvation proof) is authorable **RED-first**: a `.ynz`
  program whose SM function runs a hot wait-free loop while a sibling `background` task on
  the same worker must get scheduled time (observable via output ordering / a shared-progress
  check within a bounded wall-clock window). Under today's no-op stub it hangs/starves → RED.
  It gates Step 2 per R8's committed mitigation: author and commit it BEFORE the transform.
- **Fixture (b)** (non-SM residual) is an expected-behavior PASS fixture from day one (it
  documents absence of preemption), so RED-first does not apply to it by shape — record
  as such, per the plan's "if the ordering genuinely can't be RED-first, record why."

## Decisions recorded (Safe-Default / on-the-record)

1. **Design-note placement:** this plan's own directory (sibling of `spike-o0-flip.patch`,
   same Facet-2 precedent) — it is phase work-product gating implementation, superseded by
   Step 7's IMP rewrite; not an `IMP-*.md` (that would duplicate the doc Step 7 rewrites)
   and not scratchpad (it is approved-plan work-product, not idea-stage).
2. **Single-signature ABI change** for `ynz_rt_check_preempt` (bool return, plain sites
   discard) over a second SM-only entry point — one API per capability, no parallel twin.
3. **Widened SM loop routing** (all loops in SM functions → SM arms) over a
   plain-path-with-frame hybrid — the hybrid would need `lower_stmt` to grow suspension
   plumbing, forking the suspension machinery into a second home (authoritative-derivation
   violation by construction).
