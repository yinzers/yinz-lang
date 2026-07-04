# Concurrency Pre-Release Audit — 2026-07-04

**Status**: IN PROGRESS — findings land here as each dimension completes, priority order.
**Scope**: the concurrency flagship — `ynz-codegen/{state_machine,emit}.rs` suspension/state-machine paths, `ynz-runtime/{arc,channel,handle,runtime}.rs`.
**Goal**: release-blocking bug/leak/race findings + perf posture vs "as fast or faster than Rust." Known issue going in: whole pipeline compiles at `OptimizationLevel::None` (emit.rs:879, state_machine.rs:755) — spike running in parallel.
**Method**: Sonnet-5 finder agents per dimension; every finding adversarially verified by Fable (main loop) before it's recorded here as CONFIRMED. Unverified agent claims are marked CLAIMED.
**Next step after audit**: `/plan` for the fix milestone.

## Priority 1 — Correctness bugs + twin-derivation drift

### P1-1 — CONFIRMED BLOCKER — UFCS method calls invisible to the entire suspension machinery
**Verified by Fable directly at all four sites.** Every "does this suspend" predicate recurses into `Expr::MethodCall`'s receiver/args but never checks whether `method` itself names a suspending function:
- `crates/ynz-typeck/src/may_block.rs:1296-1318` — `collect_calls_in_expr` MethodCall arm adds no call-graph edge for `method` → transitive SuspendSet never includes callers-via-UFCS.
- `crates/ynz-typeck/src/cpu_admission.rs:823-828` — `expr_contains_suspending_call` same gap.
- `crates/ynz-codegen/src/emit.rs:653-658` — `collect_callees_in_expr` same gap → frame layouts never embed the UFCS callee's sub-frame.
- `crates/ynz-codegen/src/emit.rs:8433-8440` — `is_direct_suspending_call` only matches `Expr::Call` with `Ident` callee; any MethodCall → `false` → `wait x.suspendingFn()` falls through to the no-op wait arm and lowers as a plain synchronous call.
Typeck DOES resolve `sig.suspends` for UFCS (check.rs UFCS kernel-mode guard exists) — so `wait player.longJob()` typechecks clean, then silently compiles wrong.
**Failure mode**: the synchronous wrapper for a suspending callee drives it via `ynz_rt_run_entrypoint` → `Handle::block_on` (runtime.rs:966-1007). Called from inside an SM resume fn on a Tokio worker thread this either blocks the worker (starvation) or panics ("cannot block from within a runtime") — the driver's own doc comment (runtime.rs:921-925) declares it unreachable from resume fns; UFCS breaks that invariant. Exact mode (block vs panic) to be pinned by a repro fixture during the fix milestone.
**Fix shape**: thread `method`-resolution through all 4 sites from the ONE authoritative UFCS resolution typeck already computes (authoritative-derivation rule — do NOT re-derive method→fn mapping per site). Add `wait x.method()` fixtures (none exist today per finder sweep).

### P1-2 — THEORY (LOW, needs one more read) — twin tree-walkers for crossing-local type lookup
`find_let_typeck_type_in_stmts` (emit.rs:8276, unsubstituted, feeds slot-count sizing) vs `find_let_type_in_stmts` via `cg.expr_type` (emit.rs:8364, generic-substituted, feeds type classification). Divergence trigger would be a crossing local whose raw type is an unsubstituted generic resolving to decimal128 (2 slots vs 1 → OOB frame write). Both frame-layout call sites filter `generics.is_empty()`, so likely dormant. Confirm whether `Cg.type_params` can be non-empty in SM-resume context; if dormant, still a derivation-drift corpse candidate — unify the walkers in the fix milestone.

### P1-3 — CONFIRMED (staleness, perf/starvation) — `ynz_rt_check_preempt` still the M1 no-op stub
runtime.rs:296-299: doc says full preemption "lands in v0.3-M2"; it's v0.3-M5 and it's still a pure no-op. Hot CPU loops inside a state machine never yield the worker thread. Either implement or update the design story + doc.

## Priority 2 — Memory safety / leaks (frames, cancel/drop, Arc)

### P2-1 — CONFIRMED (Fable direct read) — bare `channel<T>` closure is unreachable in production → receive() can hang forever
`YnzChannel` (channel.rs:109-123) holds BOTH mpsc endpoints in its own fields for its entire life. `poll_recv` returns `Ready(None)` only when all senders drop — but the object itself always holds one `Sender`. Ditto `try_send` never sees `Closed` (object holds the `Receiver`). The tests prove it: they must `std::mem::replace` the endpoints to simulate closure (channel.rs:536-539, 557-560). Net: the typed channel-closed error (Lock 8) is dead code for bare channels; a consumer that keeps calling `receive()` after producers finish parks FOREVER. Liveness/design gap: the language has no end-of-stream signal for bare channels. (Handle-outbox path is fine — its conduit closes properly via `outbox_tx.take()`.)

### P2-2 — CONFIRMED (Fable direct read) — cancelled suspended sender orphans its `pending_sends` entry, wedging closure semantics further
A task cancelled while suspended on `send` leaves its boxed send-future (holding a cloned `Sender`) in `pending_sends` (channel.rs:120, inserted :270) — nothing removes it on the arg-drop path (runtime.rs drop ladder only calls `ynz_channel_free`, which just decrements the Arc). The orphaned Sender clone lives until the channel object dies. Compounds P2-1; also the leak the module doc hand-waves as "bounded" (channel.rs:117-119) — bounded in memory, unbounded in liveness effect.

### P2-3 — VERIFIED but LATENT (downgraded from HIGH) — closed-channel `.send()` drops heap payloads with no drop-glue
emit.rs closed1/closed2 blocks (~:11833-11960) build the typed error and branch to post with no `ynz_array_drop`/`ynz_free` of `value_bits`; runtime side drops the captured i64 silently (no dtor). Every failed send of a heap element (string/array/map/shape) leaks. Mechanism independently consistent with Fable's channel.rs read (type-erased i64, zero drop-glue anywhere in runtime).

### P2-4 — CONFIRMED by mechanism (Fable channel.rs read corroborates finder) — buffered channel elements never freed at channel drop
`YnzChannel` has no `Drop` impl; buffer holds `i64` bit-patterns with no type info — heap elements resident in the buffer (or in orphaned pending_sends) leak when the last ref drops. No mechanism exists anywhere to free them (the runtime cannot know the element type). Fix requires design: either codegen-registered drop-glue fn ptr per channel, or restrict/drain semantics.

### P2-5 — CLAIMED by finder (THEORY) — recursion-chain cancellation skips `cleanup_spike_cpu_handles` for child frames
runtime.rs:591-693 drop ladder: root frame gets spike-handle cleanup (:605-607), recursion-chain children (:659-680) get sleep-handle + frame free only. Live only if a self-recursive SM fn can contain a CPU-parallel group — mutual-exclusion gate unverified. Check spike admission vs recursion_slot before fix.

### P2-6 — CONFIRMED by grep (finder) + design-relevant — auto-Arc substrate (`ynz_arc_*`) has ZERO codegen callers
arc.rs is well-tested, correct (acq-rel protocol verified by Fable read) — and completely unwired. `IMP-no-function-coloring.md` describes auto-Arc as the cross-thread shared-state mechanism. Either the milestone claiming it shipped is wrong, or docs overstate. Cross-check with design-doc alignment findings.

### P2-7 — Noted (LOW) — `ynz_handle_recv_poll` panic path returns Pending with possibly-unregistered waker
handle.rs:297-303: a panic inside the poll returns `CHANNEL_PENDING`; if the panic fired before waker registration the task may never be woken → hang instead of crash. Same pattern in channel poll shims (deliberate "don't corrupt the frame" choice, but the hang mode is unacknowledged).

### P2-1 REFINEMENT (Fable, emit.rs:11834-11841) — bare-channel non-closure is KNOWN to the codegen
The closed-recv arm carries the comment "Structurally unreachable in v0.3-M4 (the channel object holds a sender)" and emits a loud abort, not a hang. So: bare channels never closing is a known v0.3-M4 design state, not a latent surprise. Remaining issues: (a) a consumer polling `receive()` after producers finish parks forever — a real UX/liveness footgun the docs must state loudly (end-of-stream signal is simply not a feature yet); (b) P2-3's leak and the closed-path error codegen are dead code until close semantics ship — but P2-4 (buffered heap elements leak when the channel drops) IS reachable today.

## Priority 3 — Scheduler races / deadlocks

### P3-1 — CONFIRMED-BY-MECHANISM, HIGH→BLOCKER-adjacent — `caller_token` ABA: frame-pointer reuse resurrects a dead task's suspended send
Token = raw frame pointer (`emit.rs:11651-11654` ptr_to_int), no generation salt. Cancelled sender's `pending_sends` entry never purged (Fable verified the full drop ladder — kind-2 arg-drop only calls `ynz_channel_free`). Allocator reuses the freed frame address for a new task → new task's `send(v2)` matches the stale entry → v2 silently discarded, DEAD task's v1 delivered under the new task's identity. Silent data corruption; trigger conditions (full channel backpressure + cancellation + malloc address reuse) are all routine. **Fix shape**: purge this caller's pending_sends entry on cancellation (the drop ladder already knows the channel ptr from the kind-2 entry; add a purge call keyed by frame ptr) AND/OR salt tokens with a monotonic generation counter. Fixing this also fixes P2-2 (same root cause).

### P3-1 ADDENDUM (Fable personal plan-audit, 2026-07-04) — SECOND caller_token producer: handle pointers
`handle.rs:326` passes `handle_ptr as u64` as the caller_token for `h.send()` — a second token-producer site with the identical ABA/orphan shape: `ynz_handle_free` (handle.rs:337-351) drops the handle and releases the channel ref but never purges the handle-keyed `pending_sends` entry; a recycled handle address inherits the dead handle's suspended send. Any P3-1 fix must enumerate ALL token producers (frame-ptr conduit tokens AND handle-ptr tokens) — purge at both cancellation paths (drop ladder AND ynz_handle_free, which already holds msg_chan) and salt both. Caught in Fable's personal M6-plan audit; missed by both scout and reviewer passes.

### P3-2 — CONFIRMED race window, HIGH (not BLOCKER — narrower than finder claimed) — lost wakeup on multi-consumer receive
`ynz_channel_recv_poll` (channel.rs:311-339): poll_recv (registers with tokio's single-slot waker) and `record_recv_waiter` are two separate critical sections. Window: consumer A's tokio registration clobbered by consumer C's later poll, and a send fires before A reaches `record_recv_waiter` → A woken by neither mechanism. Fable narrowing: the Ready-recv path also wakes all recorded waiters (channel.rs:331), so a PERMANENT hang requires A's few-instruction gap to straddle the final send of the channel's life. Real but narrow. **Fix shape**: record the waiter BEFORE polling (registration-then-poll), or hold one channel-wide lock across poll+record.

### P3-3 — CONFIRMED (Fable read agrees) — `ynz_rt_shutdown` holds the RUNTIME mutex across the up-to-5s `shutdown_timeout`
runtime.rs:316-354: `lock` is function-scoped; the "lock drops here" comment is fiction. Any other native thread hitting the RUNTIME fallback path blocks for the full drain window. MEDIUM. Mechanical fix — scope the lock, extract the owned Runtime, drain outside (the file already does this correctly in `ynz_rt_run_entrypoint:995-1006`).

### P3-4 — Clean bill (both Fable + finder): no lock-ordering inversions, no lock held across a blocking poll, arc.rs ordering textbook-correct, SeqCst hits are test-only, blocking-pool/`spawn_on_runtime` mutex discipline correct.

## Priority 4 — Design-doc alignment (impl vs IMP-no-function-coloring / IMP-concurrency)

### P4-1 — HIGH — preemption is 100% theater; doc still states it as locked+shipped
`IMP-no-function-coloring.md:216` locks "check points at function call sites AND loop back-edges." Reality: codegen emits back-edge calls only (emit.rs:12356-12365) and they call `ynz_rt_check_preempt` — a documented no-op stub (runtime.rs:281-299). Net runtime preemption: zero. The relaxation was pre-authorized in the roadmap (call-site checks cost 1190% on fib(30) per spike bench) but never written back to the design doc, and there is NO `[[deferred_language_feature]]` registry entry — the exact undocumented-deferral shape no-duct-tape bans. Starvation risk the doc exists to prevent (Go's 8-year window) is live today for loop-free CPU-bound recursion.

### P4-2 — MEDIUM — `background.cpuBound` override specified (IMP-no-function-coloring.md:247) but absent, no registry entry, violating auto-promotion.md's own override-direction checklist.

### P4-3 — MEDIUM — emit.rs:15122-15137 retains an UNASSERTED fallback branch that emits a synchronous `ynz_rt_run_entrypoint` drive (block_on shape, the M2-HALT corpse class) whenever a non-SM-classified caller reaches a suspending call. The recursive-path sibling hard-errors (emit.rs:11162); this one silently emits. **Directly interacts with P1-1: the UFCS misclassification is exactly what routes traffic onto this unguarded branch.** Fix: assert it unreachable for non-main callers (and fix P1-1 so it actually is).

### P4-4 — LOW — doc staleness: FFI `may-block` presented present-tense (foreign kw is a registered v2+ deferral); `KernelModeRejectsWait` doc says unshipped but check.rs:2441-2449 implements it; auto-Arc unwired but properly registry-deferred to v0.4 (registry even self-diagnoses that IMP-ownership.md lacks the Arc topology spec).

### P4-5 — Positive: no M2-class contradiction. The no-bridge invariant is actively guarded (emit.rs:11162 hard error; corpse-reference comments), bounded channels + kernel gates + share/lend background rejection + cancel-via-drop all match the docs. IMP-concurrency.md's Design-Divergences self-documentation is genuinely good.

## Priority 5 — Perf (gated on spike verdict)
- Everything ships at O0 today: `OptimizationLevel::None` at emit.rs:879 + state_machine.rs:755 (Fable-verified). SoA bench showed ~3.3x left on the table under opt-18 -O2.
- **Fable caveat for interpreting the spike**: TargetMachine opt level ≠ IR pass pipeline. Flipping the enum alone does NOT run mem2reg/SROA (those need a PassManager/`run_passes` over the module). A GREEN spike verdict may be a false negative — meaning "nothing broke because nothing optimized." The real milestone needs an explicit pass pipeline + the frame-safety design work (allocas that must stay addressable across suspension need to be provably exempt from promotion — likely via the frame being a single heap allocation already, which mem2reg cannot promote; VERIFY: crossing locals are stored to the heap frame at suspension, so mem2reg promoting the between-suspension allocas may be SAFE BY CONSTRUCTION — the flush/reload discipline is the protection, not O0. The emit.rs:9961/10717 comments suggest at least one path relies on O0 semantics though; that path must be found and hardened first.)
- `ynz_rt_check_preempt` no-op (P1-3/P4-1) is also a perf-adjacent item: dead call at every loop back-edge (cheap but nonzero at O0 since nothing inlines it away).
- "As fast or faster than Rust" is unfalsifiable until the optimizer pipeline exists. Recommended plan shape: optimizer milestone FIRST (it invalidates all other perf measurement), then benchmark suite vs Rust equivalents.

## Phase-0 spike — O0 → Default optimization
**VERDICT: RED** (spike agent, baseline-verified via stash/rerun — the opt flip is the only variable).
- 6/470 ynz-driver integration failures; **all 6 are `number` (decimal128) crossing-local or EC-collect paths**; every structurally-identical int/bool/float/string/shape/array/map sibling test passes. Direct repro: SIGSEGV (exit 139) on `v0_3_m3a_p1_ec_crossing_local_propagated_number` binary.
- Failing set: examples_basics_runs_end_to_end, v03_m3a_p1_ec_crossing_local_propagated_number, v03_m3b_p5_parallel_number_ec_inline_collect ×2, v03_m3d_danger_mixed_number_declines, v0_3_m3f_ec_three_bindings_distinct_values.
- **Mechanism attribution is UNVERIFIED THEORY**: scout blames mem2reg per the emit.rs:9961/10717 comments, but TargetMachine opt level drives BACKEND passes (ISel/regalloc/scheduling), not the mid-end IR pipeline where mem2reg lives. M7 phase 1 must root-cause the actual pass (compare -O0/-O2 backend output on the failing fixture; bisect via llc/opt) before designing the fix. The narrow type-scope (16-byte decimal128 + EC staging) suggests wide-value stack-slot handling, possibly alloca coalescing/dead-store elimination in the backend against the manually-serialized frame.
- **Bonus finding (add to M6): `ynz run` masks signal-death as exit 1** — crates/ynz-driver/src/run.rs:75 `status.code().unwrap_or(1)` silently converts SIGSEGV into a diagnostic-free exit 1. Teaching-language UX violation (Golden Rule 11) and it actively hindered this spike. Fix: report the signal.
- Spike diff preserved in worktree `.claude/worktrees/agent-abba2c8babbd9ea21` (2-line change) for M7's reference.

## M7-plan addendum (Fable personal plan-audit, 2026-07-04) — back-edge yield is a CODEGEN mechanism, not a runtime body-swap
The M7 plan's Phase 6 framed real preemption as "implement cooperative-yield semantics inside `ynz_rt_check_preempt`." Architecturally wrong: `ynz_rt_check_preempt` is a synchronous extern-C callee — it cannot yield the enclosing Tokio task. True back-edge yield requires codegen to turn loop back-edges INSIDE SM functions into poll-yield suspension points (store resume_point, flush crossing locals, return Pending) — a frame-layout-affecting transformation in the same hazard family as R1. Non-SM (plain synchronous) functions can NEVER cooperatively yield — their protection is CPU-admission routing to the blocking pool, which already exists. The honest preemption architecture the doc update must state: (a) SM-function back edges = codegen poll-yield points (new, M7), (b) non-SM CPU-bound work = blocking-pool routing (shipped), (c) residual: CPU-heavy code inside non-SM fns that admission misses — named limitation. Caught in Fable's personal M7 read; missed by planner and both reviewer passes.

## Synthesis — fix-milestone priority order (for /plan)
1. **P1-1 UFCS suspension invisibility** — release blocker, flagship-feature × flagship-syntax. Fix all 4 sites from ONE authoritative resolution + fixtures.
2. **P3-1/P2-2 pending_sends ABA + orphan purge** — silent data corruption, reachable today.
3. **P3-2 lost-wakeup window** — register-before-poll.
4. **P4-3 unasserted block_on fallback** — hard-error guard (cheap, closes P1-1's escape hatch class).
5. **P2-4 buffered-element leak at channel drop** — needs small design decision (drop-glue fn ptr per channel).
6. **P4-1 preemption honesty** — either implement back-edge preemption for real or write the deferral (doc + registry entry). No-duct-tape demands the paper trail.
7. **P3-3 shutdown mutex scope** — mechanical.
8. **O0→optimizer pipeline** — own milestone, spike-informed, with the frame-safety analysis (find the paths relying on O0 semantics; prove flush/reload discipline covers them; wire run_passes).
9. Doc staleness sweep (P4-4, P2-1 footgun documentation, P2-6 wording).
Items NOT to fix (YAGNI/latent): P2-3 closed-send leak (unreachable until close semantics ship — record as deferral tied to that feature), P1-2 (verify dormancy, then unify walkers as cleanup), P2-5 (verify spike×recursion exclusion first).
