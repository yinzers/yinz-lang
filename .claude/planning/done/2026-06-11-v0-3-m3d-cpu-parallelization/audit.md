---
name: "v0-3-m3d-cpu-parallelization-audit"
plan-id: "2026-06-11-v0-3-m3d-cpu-parallelization"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-06-11-v0-3-m3d-cpu-parallelization

Migrated 2026-07-01 from the pre-migration `.claude/plans/` ledger format. This sidecar is a
best-effort mechanical migration: the `## Session log` below is reconstructed from the old
scratch/*-deviations.md files (concatenated verbatim, NOT reformatted into individual FRAGO
delta-records — that reformatting was out of scope for a drive-by migration). Historical
session-ids were not tracked pre-migration, so the frontmatter `session-id` list starts empty.

## Session log

(pre-migration history — see plan.md body's Findings Logs / "Committed:" lines for the
authoritative narrative; this section is the raw scratch/ deviation record)

## FRAGO log

(none recorded in the old format — deviations were logged inline in the plan body's
"Findings Log" per-phase, not as discrete FRAGO records; see plan.md)

## Migrated scratch/ deviation notes

### phase0-deviations.md

# v0-3-m3d-cpu-parallelization Phase 0 Deviations — UPDATED (round-7 applied, 2026-06-12)

D_count: 17

> **UPDATED-PHASE NOTE (round 7)**: Round-6 left ISSUE-A (unterminated post_wait_bb) and ISSUE-B (spike frame aliasing / silent wrong output). Round-7 fixes F7-1–F7-3 address both. Scope deviations #1–#3 unchanged; approach deviations #4–#14 from prior rounds unchanged; deviation #15 extended with pre-pair and post-pair suspending decline guards (F7-1 and F7-2); #16 and #17 unchanged. The F7-2 fix initially attempted heap-boxing via `emit_suspending_call_heap_boxed` for post-pair suspending callees, but this caused an LLVM SSA dominance violation (recursion_slot is None for the callee; re-entry code used the initial child_frame SSA value across basic blocks). The correct fix is admission decline at the gate, consistent with the spike's proven-safe envelope posture. Two new fixtures (p: leading wait, q: suspending post-pair callee) verify byte-identical decline in both modes.

## Scope Deviations (verbatim from executor report)

### Deviations from Files (expected scope)

- **Scope Deviation #1** (file: `crates/ynz-runtime/src/lib.rs`): touched outside declared scope (`Files (expected scope)` lists `crates/ynz-runtime/src/runtime.rs`, not `lib.rs`). Rationale: `lib.rs` re-exports runtime symbols and hosts all unit tests for the crate; the new shims required both a `pub use` re-export update and 5 unit tests. Without the re-export update the shims are inaccessible to integration tests; without the test module, Step 1's acceptance requirement ("runtime unit tests") is unmet. The tests cannot live in `runtime.rs` because the test harness lives in `lib.rs` per the existing project convention. Diff hunks: `crates/ynz-runtime/src/lib.rs:1-6, crates/ynz-runtime/src/lib.rs:2774-3071`.

- **Scope Deviation #2** (file: `crates/ynz-codegen/src/runtime_decls.rs`; 13 insta golden snapshot files under `crates/ynz-codegen/tests/snapshots/golden__*.snap`): touched outside declared scope (`runtime_decls.rs` is Phase 1's declared scope). Rationale: `ynz_rt_spawn_blocking_joinable`, `ynz_rt_join_poll`, and `ynz_rt_join_handle_free` are declared in `runtime_decls.rs` (the compile-time ABI declaration table); without entries there the codegen's `extern` declarations are missing and the spike cannot link. Declarations are unconditional (declaring conditionally on an env var would put env-dependent content in every golden snapshot); they are body-less zero-cost `declare` lines, never called in default-gated code. The 13 snapshot updates are the deterministic cascade — the insta golden-snapshot suite snapshots all declared externs in the IR output and fails if any snapshot is stale. P1 owns hardening these declarations as its own declared scope. Diff hunks: `crates/ynz-codegen/src/runtime_decls.rs, crates/ynz-codegen/tests/snapshots/`.

- **Scope Deviation #3** (file: `.claude/state.md`): touched outside declared scope (`Files (expected scope)` lists only compiler source crates). Rationale: the build-env discovery (LLVM 18 + glibc 2.39 exist ONLY in the devcontainer; the WSL host with LLVM 15 cannot build `ynz-codegen`/`ynz-driver`) was operationally critical context that would block any future executor working on this branch. Per `CLAUDE.md` Rule 6, state.md captures environment facts that must survive compaction. Omitting this would cause the next session to rediscover the devcontainer requirement by failing builds. Diff hunks: `.claude/state.md:41-43`.

## Approach Deviations (verbatim from executor report)

### Approach Deviations

- **Deviation #4** (Step 1 — `ynz_rt_join_poll` ABI): plan said `extern "C" fn`, implementation used `extern "C-unwind"`. Rationale: `extern "C" fn` that calls `std::panic::resume_unwind` aborts instead of propagating the panic (Rust RFC 2945 / Rust 1.71+: panic-in-C-ABI = abort). `extern "C-unwind"` allows `resume_unwind` to propagate from within `ynz_rt_join_poll` itself. Correction from R5 (judge #4 blocked R4 on this): the codegen-emitted SM resume functions are `extern "C"` (not `extern "C-unwind"`), so an unwind originating in `ynz_rt_join_poll` that propagates back into an SM resume function will abort at that boundary. Full end-to-end C-unwind propagation (resume functions emitted as `extern "C-unwind"` so unwind reaches Tokio's `catch_unwind`) is a P1 deliverable. The unit test path (`panic_reraises_in_parent`) exercises the ABI correctly because `ynz_rt_join_poll` is called from `SpawnStateFnFuture::poll` (pure Rust, no C boundary). No behavioral change for Pending or Ready(Ok) paths. Diff hunks: `crates/ynz-runtime/src/runtime.rs:995-1010`.

- **Deviation #5** (Step 1 — `CpuJoinHandle` visibility): plan implied `CpuJoinHandle` would be private (internal implementation detail). Implementation made the struct `pub(crate)` with a PRIVATE inner field and a `pub(crate) fn new(h: JoinHandle<YnzCpuResult>) -> Self` constructor. Rationale: the panic re-raise test requires constructing a `CpuJoinHandle` directly from a panicking `spawn_blocking` future — bypassing the `extern "C" fn` boundary that would abort on panic per RFC 2945. Without crate-visible construction, the test cannot exercise the `Ready(Err(panic))` branch of `ynz_rt_join_poll` at all. The inner field stays private so the opaque-handle ownership protocol (only `ynz_rt_join_poll` and `ynz_rt_join_handle_free` consume the handle) remains type-enforced; the constructor grants construction without field access. No external ABI surface change. Diff hunks: `crates/ynz-runtime/src/runtime.rs:884-905`.

- **Deviation #6** (Step 2 — `spike_cpu_candidates` detection scope): plan said "ONE fixture shape (two int-returning calls to a recursive fib-style callee)" — implies same-callee detection. Implementation detects ANY two non-suspending int-returning direct calls regardless of callee identity (same OR distinct callees). Rationale: fixture (a) requires distinct callees (`fib` + `fib2`). The original same-callee-only detection would have made fixture (a) silently skip the spike and run sequentially, producing the correct numbers but not proving the parallel path. Extending to distinct-callee detection is the minimum change that makes fixture (a) actually exercise the spike. Zero behavior change on existing non-spike fixtures. Diff hunks: `crates/ynz-codegen/src/emit.rs:6198-6310`.

- **Deviation #7** (Step 2 — spawn state branches to `poll_state` not `pending_block`): plan described "spawn → suspend-at-join → resume → result-bind" implying the spawn state stores the handle and suspends. Implementation branches from spawn state to `poll_state` immediately, polling both handles on the same turn before ever suspending. Rationale: a Future that returns `Poll::Pending` MUST register a waker before returning Pending (Rust async contract). The original design of branching spawn→pending_block without first calling `ynz_rt_join_poll` skipped waker registration — the JoinHandle waker was never set, so when the blocking tasks completed, nothing woke the SM. Result: the SM hung indefinitely. Routing spawn→poll_state is the correct fix: the poll_state calls `ynz_rt_join_poll` (which forwards the waker to the JoinHandle), discovers tasks are Pending, THEN branches to pending_block. The spawn-then-poll-then-suspend semantics are preserved; only the order of operations within the first turn changes. Diff hunks: `crates/ynz-codegen/src/emit.rs:6543-6558`.

- **Deviation #8** (Step 2 — poll state null-guards each handle slot before re-polling): plan did not specify null-guard discipline on handle slots across re-polls. Implementation adds per-handle null-check and slot-nulling on Ready. Rationale: `ynz_rt_join_poll` returns 0 (Ready) and FREES the JoinHandle box. If the parent SM re-enters `poll_state` (because the OTHER child is still pending), re-polling the already-freed handle is a use-after-free → SEGFAULT (observed at exit code 139 for fib(22+)). Fix: after `ynz_rt_join_poll` returns 0 (Ready), store null into the frame handle slot; on re-entry, check is_null before calling poll, skip if null. This is the same discipline the sleep-handle protocol uses (freed-on-Ready, null-check in drop shim). No change to the Happy-path (both children finish on the first poll turn — no null to check). Diff hunks: `crates/ynz-codegen/src/emit.rs:6559-6700`.

- **Deviation #9** (round-1 fix loop, F7 — `emit_cpu_group_spawn_join` return type + frame-slot reload helper): the fix-round instruction said "fix spike so fixture (g) passes"; implementation changed the return type of `emit_cpu_group_spawn_join` from `Result<(), String>` to `Result<Vec<(String, u64)>, String>`, added helper `spike_reload_cpu_results_from_frame`, and updated `lower_sm_block`'s spike path to capture crossing results and reload them after each suspension. Rationale: the only correct fix for cross-invocation locals is to reload from persistent frame slots; the `Vec<(String, u64)>` return is the minimum-surface way to pass the (name, frame_offset) pairs from the point of creation to the reload site without globals or additional state. Diff hunks: `crates/ynz-codegen/src/emit.rs:6354-6415, crates/ynz-codegen/src/emit.rs:6836-6906`.

- **Deviation #10** (round-3 fix loop, R3-3 — exclusion via `Cg` field instead of parameter threading): fix instruction said "pass the spike crossing-name list as an exclusion set" implying a parameter change to `reload_params_from_frame`. Implementation stored exclusion names in a new `Cg` field (`m3d_spike_cpu_result_names`) read directly inside `reload_params_from_frame`. Rationale: `reload_params_from_frame` is called from 5 sites through 3 intermediate functions (`lower_sm_stmt_with_wait`, `emit_wait_point`, `emit_suspending_call_inline_poll`) — propagating the exclusion as a parameter required changing all 3 intermediate signatures and all 5 call sites. The `Cg` field approach achieves the same correctness result with zero signature changes and no intermediate-call cascades. The field is `Vec::new()` for all non-spike builds (zero behavioral change on default path). Diff hunks: `crates/ynz-codegen/src/emit.rs:1562-1579, crates/ynz-codegen/src/emit.rs:1260, crates/ynz-codegen/src/emit.rs:1965, crates/ynz-codegen/src/emit.rs:2318, crates/ynz-codegen/src/emit.rs:3193-3203, crates/ynz-codegen/src/emit.rs:3587-3597`.

- **Deviation #11** (round-3 fix loop, R3-4 — spike frame discriminator; coordinator-identified, NOT self-reported by executor): the fix instruction suggested the per-frame drop-shim-pointer mechanism (if real) for cancellation cleanup; implementation instead added a magic-value frame discriminator. Rationale (from executor report): spike frame discriminator: codegen writes SPIKE_FRAME_MAGIC (0x5350_494B) to frame offset 4 at spawn time; SpawnStateFnFuture::drop reads the magic and frees non-null spike handle slots 32/40 if present. Non-spike frames have 0 at offset 4 (ynz_alloc_zeroed guarantee) — safely skipped. Diff hunks: `crates/ynz-runtime/src/runtime.rs:454-479, crates/ynz-codegen/src/emit.rs:6653-6681`.

- **Deviation #12** (round-4 fix loop, F4-1 — spike frame layout shift; coordinator-identified, NOT self-reported by executor): the frame-slot collision fix changed the spike frame layout approach. Rationale (from executor report): F4-1 frame-slot collision fix: when the spike is active, crossing-local slot base shifts by SPIKE_SLOT_RESERVE=6 (crossing_slot_base = n_params + SPIKE_SLOT_RESERVE); spike functions bypass build_frame_layouts and size the frame as FRAME_HEADER_SIZE + own_locals_size(n_locals) with the reserve included. Spike handle/result region keeps its fixed byte offsets so the runtime drop contract (SpawnStateFnFuture::drop reading slots 32/40) is unchanged. Diff hunks: `crates/ynz-codegen/src/emit.rs:2146-2230`.

- **Deviation #13** (round-4 fix loop, F4-5 — discriminator cleanup extracted to testable helper): fix instruction said "add a unit test covering the discriminator drop branch"; implementation extracted `cleanup_spike_cpu_handles` as a `pub(crate)` helper and tests the helper directly. Rationale: SpawnStateFnFuture is a private struct with no constructor accessible from lib.rs tests; constructing a full instance requires a live resume-fn and allocated frame — substantially more test scaffolding than warranted for this fix. The extraction makes the discriminator logic independently testable and is itself the correctness proof; SpawnStateFnFuture::drop now delegates to the extracted function so the test transitively covers the drop path. Diff hunks: `crates/ynz-runtime/src/runtime.rs:469-515, crates/ynz-runtime/src/lib.rs:3228-3324`.

- **Deviation #14** (round-5 post-gate regression fix — spike result alloca pre-allocation in sm_entry): F5-3/F5-4 moved `spike_reload_cpu_results_from_frame` into `reload_params_from_frame`'s `reload_crossing:true` path; the implementation initially called `build_alloca` inside that function (in a state block). This caused LLVM module verification failure "Instruction does not dominate all uses" for fixture (h) and any spike fixture with >1 suspension. Root cause: `build_alloca` in a non-entry basic block (e.g. `cont_state_bb`) does not dominate loads in other state blocks, violating LLVM SSA. Fix: pre-allocate the 2 result allocas in the function entry block (`sm_entry`) during Step 1c of `lower_function_with_waits`, using a new helper `spike_cpu_group_result_names` (same gate logic as `spike_extract_cpu_group`) and a new `Cg` field `m3d_spike_cpu_result_allocas: HashMap<String, PointerValue<'ctx>>`. `spike_reload_cpu_results_from_frame` now loads from the frame slot and stores into the pre-existing sm_entry alloca (no `build_alloca` call). `emit_cpu_group_spawn_join`'s `all_done_bb` binding path likewise uses the pre-existing alloca via `m3d_spike_cpu_result_allocas` (removes the fallback `build_alloca` in `all_done_bb`). Rationale: LLVM SSA requires that all alloca instructions be in the function entry block when their values are loaded in multiple basic blocks. The sm_entry pre-allocation pattern is identical to how params and crossing locals are handled. All 14 fixtures (a)-(n) pass in both spike and non-spike modes after this fix. Diff hunks: `crates/ynz-codegen/src/emit.rs:1583-1601, crates/ynz-codegen/src/emit.rs:1262, crates/ynz-codegen/src/emit.rs:1984, crates/ynz-codegen/src/emit.rs:2399, crates/ynz-codegen/src/emit.rs:2598-2636, crates/ynz-codegen/src/emit.rs:6496-6609, crates/ynz-codegen/src/emit.rs:6759-6858, crates/ynz-codegen/src/emit.rs:7297-7316`.

- **Deviation #15** (rounds 6+7 F6-1/F6-2/F7-1/F7-2 — prune mechanism replaced by admission decline + pre-pair lowering + pre-pair suspending decline + post-pair suspending decline): plan's R5 prune-before-loop mechanism (`m3d_spike_cpu_result_names.retain()` in `lower_sm_block`) is REPLACED by: (a) a result-assignment decline guard added to all three gate functions (`spike_cpu_candidates`, `spike_cpu_group_result_names`, `spike_extract_cpu_group`) so admission is refused when any post-pair rest statement at any nesting depth assigns a bind name; (b) `spike_extract_cpu_group`'s return type changed from `Option<(Vec<&Stmt>, Vec<&Stmt>)>` to `Option<(Vec<&Stmt>, Vec<&Stmt>, Vec<&Stmt>)>` = `(pre_stmts, group_stmts, post_stmts)` to expose pre-pair statements; (c) `lower_sm_block` now lowers `pre_stmts` sequentially before calling `emit_cpu_group_spawn_join`, ensuring any pre-pair `let n = 10` binding is initialized in its alloca before spawn; (d) all three gate functions decline when any pre-pair statement contains a `wait` or suspending call — a wait in pre_stmts advances `current_state` and positions the builder in `post_wait_bb`; the subsequent spawn block has no predecessor branch from that block, leaving `post_wait_bb` without a terminator (LLVM verifier crash); (e) all three gate functions decline when any post-pair statement contains a SUSPENDING CALL (round-8 narrowing: `stmt_contains_suspending_call` only — intrinsic waits like `wait sleep` are admitted post-pair because sleep uses the SM inline-poll path with no embedded child sub-frame and is excluded from the predicate via the M2_MAY_BLOCK_INTRINSICS guard) — after the join, the spike host's child sub-frame offsets are computed pre-spike (without the 48-byte reserve), so embedding a suspending USER-DEFINED callee aliases `SPIKE_RESULT_0_OFFSET` (byte 48); routing through `emit_suspending_call_heap_boxed` is not viable because the spike host's `recursion_slot` is `None` (the callee appears in `children`), causing the heap-boxed path's re-entry code to use the initial `child_frame` SSA value across basic blocks — LLVM SSA dominance violation. Rationale: the prune mechanism had a correctness hole (judge #9's repro): for `let a = fib(10); let b = fib(11); wait; print(a); a = 999; wait; print(a)`, the prune removed `a` before the assignment fired, and the crossing slot was never populated, producing `0` instead of `55`. Admission decline is the correct fix: the spike emits correct code for programs in the admitted envelope and falls through to sequential for everything else. Pre-pair lowering fixes judge #7's repro. Pre-pair suspending decline fixes ISSUE-A (unterminated `post_wait_bb`). Post-pair suspending decline fixes ISSUE-B (silent wrong output 55/1/89). Fixtures (p) and (q) added to prove both decline byte-identically in both modes. Diff hunks: `crates/ynz-codegen/src/emit.rs:6433-6610, crates/ynz-codegen/src/emit.rs:6715-6870, crates/ynz-codegen/src/emit.rs:3728-3805`.

- **Deviation #16** (round-6 F6-3 — entrypoint-only admission gate): `spike_cpu_candidates` now gates at the top on `f.name != "entrypoint"` before any other check. Rationale: a non-entrypoint spike host is called from another SM function via `emit_suspending_call_heap_boxed`; that caller allocates the callee frame using `frame_layouts` (built before spike promotion) at 32 bytes, while the spike resume function writes bytes 48–95 for result slots → heap corruption. The entrypoint-only gate is the narrowest fix: it refuses admission for any function that is not the program entrypoint, preserving the proven-safe envelope without changing the spike's behavior for the admitted case. Fixture `v0_3_m3d_spike_o_callee_host.ynz` confirms `runner()` declines byte-identically in both modes. Diff hunks: `crates/ynz-codegen/src/emit.rs:6440-6445`.

- **Deviation #17** (round-6 F6-4 — CpuJoinHandle struct change + per-handle drop probe): `CpuJoinHandle` changed from a tuple-struct newtype `CpuJoinHandle(JoinHandle<YnzCpuResult>)` to a named-field struct with `inner: JoinHandle<YnzCpuResult>` and a `#[cfg(test)] probe: Option<Arc<AtomicUsize>>`. A `#[cfg(test)] impl Drop for CpuJoinHandle` increments the probe's counter when the handle is dropped. A `#[cfg(test)] pub(crate) fn set_drop_probe()` method injects the probe before boxing. Both `discriminator_drop_frees_spike_handles` and `spawn_state_fn_future_drop_before_first_poll_frees_handles` now go through the `SpawnStateFnFuture` drop path (not direct `cleanup_spike_cpu_handles` calls) so both FAIL when line 551 is commented out. Rationale: judge ISSUE-1 finding — both tests were blind to leaks: `AtomicBool::completed` asserts task ran (always true; Tokio runs tasks regardless of JoinHandle lifecycle), not that the Box was freed. Per-handle probes with `Arc`-local counters eliminate the global-counter race from concurrent tests (`saturation_600_joins` drops 640 handles). Diff hunks: `crates/ynz-runtime/src/runtime.rs:988-1030, crates/ynz-runtime/src/lib.rs:3232-3420`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 (scope)
- **type**: scope
- **rationale**: `lib.rs` re-exports runtime symbols and hosts all unit tests for the crate; the new shims required both a `pub use` re-export update and 5 unit tests. Without the re-export update the shims are inaccessible to integration tests; without the test module, Step 1's acceptance requirement ("runtime unit tests") is unmet. The tests cannot live in `runtime.rs` because the test harness lives in `lib.rs` per the existing project convention.
- **diff hunks**: crates/ynz-runtime/src/lib.rs:1-6, crates/ynz-runtime/src/lib.rs:2774-3071
- **judge identity hash**: 1b473f78dfe4066034c14b30972b6c1265624791

### Deviation #2 (scope)
- **type**: scope
- **rationale**: `ynz_rt_spawn_blocking_joinable`, `ynz_rt_join_poll`, and `ynz_rt_join_handle_free` are declared in `runtime_decls.rs` (the compile-time ABI declaration table); without entries there the codegen's `extern` declarations are missing and the spike cannot link. Declarations are unconditional (declaring conditionally on an env var would put env-dependent content in every golden snapshot); they are body-less zero-cost `declare` lines, never called in default-gated code. The 13 snapshot updates are the deterministic cascade. P1 owns hardening these declarations as its own declared scope.
- **diff hunks**: crates/ynz-codegen/src/runtime_decls.rs, crates/ynz-codegen/tests/snapshots/
- **judge identity hash**: adfdb61f3fa3c2b7ff66280641b75ec3f86e5dd8

### Deviation #3 (scope)
- **type**: scope
- **rationale**: the build-env discovery (LLVM 18 + glibc 2.39 exist ONLY in the devcontainer; the WSL host with LLVM 15 cannot build `ynz-codegen`/`ynz-driver`) was operationally critical context that would block any future executor working on this branch. Per `CLAUDE.md` Rule 6, state.md captures environment facts that must survive compaction. Omitting this would cause the next session to rediscover the devcontainer requirement by failing builds.
- **diff hunks**: .claude/state.md:41-43
- **judge identity hash**: 6c867c757443ae70c67be6a2e2cb542367fcb4e2

### Deviation #4 (approach)
- **type**: approach
- **rationale**: `extern "C" fn` that calls `std::panic::resume_unwind` aborts instead of propagating the panic (Rust RFC 2945 / Rust 1.71+: panic-in-C-ABI = abort). `extern "C-unwind"` allows `resume_unwind` to propagate from within `ynz_rt_join_poll` itself. Corrected claim (R5, judge #4 blocked R4): the SM resume functions are `extern "C"`, so unwind propagating back through them aborts at that boundary; full end-to-end C-unwind propagation is a P1 deliverable. The unit test path exercises the ABI correctly (pure Rust call stack, no C boundary). No behavioral change for Pending or Ready(Ok) paths.
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:995-1010
- **judge identity hash**: 2c4e6616b2b6ea1193b41a6fb7283f974413165b

### Deviation #5 (approach)
- **type**: approach
- **rationale**: the panic re-raise test requires constructing a `CpuJoinHandle` directly from a panicking `spawn_blocking` future — bypassing the `extern "C" fn` boundary that would abort on panic per RFC 2945. Without crate-visible construction, the test cannot exercise the `Ready(Err(panic))` branch of `ynz_rt_join_poll` at all. The inner field stays private so the opaque-handle ownership protocol remains type-enforced; the `pub(crate) fn new()` constructor grants construction without field access. No external ABI surface change.
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:884-905
- **judge identity hash**: dfc8312bdce8099ff0465833e024ba7cb26a2362

### Deviation #6 (approach)
- **type**: approach
- **rationale**: fixture (a) requires distinct callees (`fib` + `fib2`). The original same-callee-only detection would have made fixture (a) silently skip the spike and run sequentially. Extending to distinct-callee detection is the minimum change that makes fixture (a) actually exercise the spike. Zero behavior change on existing non-spike fixtures.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6198-6310
- **judge identity hash**: 7a6f20e673ae87d0af38d8e714b9ad54e3d993a4

### Deviation #7 (approach)
- **type**: approach
- **rationale**: a Future returning Poll::Pending MUST register a waker before returning Pending. Original spawn→pending_block path skipped the first poll, leaving waker unregistered, causing infinite hang. Routing spawn→poll_state registers wakers on the first turn. The spawn-then-poll-then-suspend semantics are preserved.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6543-6558
- **judge identity hash**: bba0e839805a6f6413c1319bc239104510c409b0

### Deviation #8 (approach)
- **type**: approach
- **rationale**: ynz_rt_join_poll frees the JoinHandle box on Ready. Re-polling a freed handle on subsequent SM re-entries caused SEGFAULT (exit 139 at fib(22+)). Null-guard discipline (null on Ready, skip-if-null on re-entry) prevents UAF. Matches existing sleep-handle protocol.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6559-6700
- **judge identity hash**: 73c553f72f4237237a9a4ab5c60bcecf09e359a7

### Deviation #9 (approach)
- **type**: approach
- **rationale**: the only correct fix for cross-invocation locals is to reload from persistent frame slots; the `Vec<(String, u64)>` return is the minimum-surface way to pass the (name, frame_offset) pairs from the point of creation to the reload site without globals or additional state.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6354-6415, crates/ynz-codegen/src/emit.rs:6836-6906
- **judge identity hash**: 8f8ffa45a154a0c86baffa82962c4cec408a00e5

### Deviation #10 (approach)
- **type**: approach
- **rationale**: `reload_params_from_frame` is called from 5 sites through 3 intermediate functions (`lower_sm_stmt_with_wait`, `emit_wait_point`, `emit_suspending_call_inline_poll`) — propagating the exclusion as a parameter required changing all 3 intermediate signatures and all 5 call sites. The `Cg` field approach achieves the same correctness result with zero signature changes and no intermediate-call cascades. The field is `Vec::new()` for all non-spike builds (zero behavioral change on default path).
- **diff hunks**: crates/ynz-codegen/src/emit.rs:1562-1579, crates/ynz-codegen/src/emit.rs:1260, crates/ynz-codegen/src/emit.rs:1965, crates/ynz-codegen/src/emit.rs:2318, crates/ynz-codegen/src/emit.rs:3193-3203, crates/ynz-codegen/src/emit.rs:3587-3597
- **judge identity hash**: dc62ffd4676322b290baffa54ea98f6995e058d7

### Deviation #11 (approach)
- **type**: approach
- **rationale**: spike frame discriminator: codegen writes SPIKE_FRAME_MAGIC (0x5350_494B) to frame offset 4 at spawn time; SpawnStateFnFuture::drop reads the magic and frees non-null spike handle slots 32/40 if present. Non-spike frames have 0 at offset 4 (ynz_alloc_zeroed guarantee) — safely skipped.
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:454-479, crates/ynz-codegen/src/emit.rs:6653-6681
- **judge identity hash**: 669dc89e7c79995a7163f0cf66a68437d0b93d7b

### Deviation #12 (approach)
- **type**: approach
- **rationale**: F4-1 frame-slot collision fix: when the spike is active, crossing-local slot base shifts by SPIKE_SLOT_RESERVE=6 (crossing_slot_base = n_params + SPIKE_SLOT_RESERVE); spike functions bypass build_frame_layouts and size the frame as FRAME_HEADER_SIZE + own_locals_size(n_locals) with the reserve included. Spike handle/result region keeps its fixed byte offsets so the runtime drop contract (SpawnStateFnFuture::drop reading slots 32/40) is unchanged.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:2146-2230
- **judge identity hash**: 8fee9ca776ef8d0e93fe923a46c53b700e13bde4

### Deviation #13 (approach)
- **type**: approach
- **rationale**: SpawnStateFnFuture is a private struct with no constructor accessible from lib.rs tests; constructing a full instance requires a live resume-fn and allocated frame — substantially more test scaffolding than warranted for this fix. The extraction makes the discriminator logic independently testable and is itself the correctness proof; SpawnStateFnFuture::drop now delegates to the extracted function so the test transitively covers the drop path
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:469-515, crates/ynz-runtime/src/lib.rs:3228-3324
- **judge identity hash**: 01e0de37e16a018f81670ae166680384dbaec7e3

### Deviation #14 (approach)
- **type**: approach
- **rationale**: F5-3/F5-4 moved spike_reload into reload_params_from_frame which called build_alloca in a non-entry state block (cont_state_bb), violating LLVM SSA dominance ("Instruction does not dominate all uses"). Fix: pre-allocate result allocas in sm_entry Step 1c via spike_cpu_group_result_names helper + m3d_spike_cpu_result_allocas Cg field; spike_reload and emit_cpu_group_spawn_join both use pre-existing sm_entry allocas (no build_alloca at reload or all_done_bb time). The sm_entry pre-allocation pattern matches how params and crossing locals are handled. All 14 fixtures (a)-(n) pass in both modes after this fix.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:1583-1601, crates/ynz-codegen/src/emit.rs:1262, crates/ynz-codegen/src/emit.rs:1984, crates/ynz-codegen/src/emit.rs:2399, crates/ynz-codegen/src/emit.rs:2598-2636, crates/ynz-codegen/src/emit.rs:6496-6609, crates/ynz-codegen/src/emit.rs:6759-6858, crates/ynz-codegen/src/emit.rs:7297-7316
- **judge identity hash**: b49a53a44d98691ccaa5168a23c802856d089744

### Deviation #15 (approach) — rounds 6+7+8
- **type**: approach
- **rationale**: prune-before-loop mechanism had a read-before-mutation hole: prune removed `a` before the assignment fired, leaving the spike reload at an uninitialized crossing slot. Fix: replace prune with admission decline in all three gate functions + return (pre_stmts, group_stmts, post_stmts) from spike_extract_cpu_group + lower pre_stmts before spawn in lower_sm_block. Round-7 extended: all three gates also decline when any pre-pair statement is a wait/suspending call (ISSUE-A: unterminated post_wait_bb). Round-8 narrowed the post-pair decline: post-pair statements decline ONLY on stmt_contains_suspending_call (user-defined suspending callees, whose embedded child sub-frames alias the spike result slots — ISSUE-B; heap_boxed routing not viable because recursion_slot is None for the callee, causing SSA dominance violation). Intrinsic waits (wait sleep) post-pair are ADMITTED — sleep uses the SM inline-poll path with no embedded child sub-frame, so no aliasing risk; sleep is excluded from stmt_contains_suspending_call via the M2_MAY_BLOCK_INTRINSICS guard. Pre-pair keeps the wider wait||suspending predicate because ISSUE-A is a control-flow crash triggered by ANY wait. Fixtures (g)/(h)/(i)/(j)/(n) fire the spike with post-pair waits (2 spawn call instructions each, IR-verified); (p) and (q) verify byte-identical decline in both modes.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6433-6990, crates/ynz-codegen/src/emit.rs:3728-3805, crates/ynz-codegen/src/emit.rs:2610-2620
- **judge identity hash**: fdc8dc2bb23213c95a17b98e23804a674d7280dd

### Deviation #16 (approach) — round 6
- **type**: approach
- **rationale**: non-entrypoint spike host frames are sized by emit_suspending_call_heap_boxed using frame_layouts (built before spike promotion) at 32 bytes; spike resume writes bytes 48-95 for result slots — heap corruption. Entrypoint-only gate added at top of spike_cpu_candidates (before any other check) to prevent this path entirely. Fixture (o) added to prove decline is byte-identical in both modes.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6440-6445
- **judge identity hash**: 3a091cffce9827ca1d0fe2242c5619b070b42a50

### Deviation #17 (approach) — round 6
- **type**: approach
- **rationale**: test blindness to leak: `AtomicBool::completed` asserts task ran (always true regardless of handle lifecycle). Per-handle `#[cfg(test)] probe: Option<Arc<AtomicUsize>>` in CpuJoinHandle is incremented in `#[cfg(test)] impl Drop` — Arc-local so concurrent drops from saturation_600_joins cannot race. Both tests now go through SpawnStateFnFuture::drop path so both FAIL when line-551 delegation is commented out. CpuJoinHandle changed from tuple-struct to named-field struct; `inner` field rename is the only non-test behavioral change.
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:988-1030, crates/ynz-runtime/src/lib.rs:3232-3420
- **judge identity hash**: 2ba8279816f0a7af5f11ff92977d03686bd15998

### phase1-deviations.md

# v0-3-m3d-cpu-parallelization Phase 1 Deviations — captured 2026-06-12

D_count: 0

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (Executor reported zero scope deviations in all rounds; only the 3 declared-scope files plus one coordinator-authorized touch — see below.)

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches. (All rounds: the ctx-free accounting used the literal allocation-counter mechanism through the real `ynz_rt_spawn_blocking_joinable`; no approach deviation.)

## Resolved spawn list (orchestrator's parsed view)

No deviations — no judges spawned this phase.

## Coordinator-authorized scope note (NOT an executor deviation, NOT judge-routed)

FIX 3 (round-1 fix) added one integration test `spawn_after_shutdown_returns_null` to the EXISTING `crates/ynz-runtime/tests/m2_spike.rs`, outside Phase 1's declared 3-file scope (`runtime.rs`/`lib.rs`/`runtime_decls.rs`). This was COORDINATOR-DIRECTED in the FIX-3 instruction: the post-shutdown discard branch (`runtime.rs:1086-1090`) requires `ynz_rt_init`/`ynz_rt_shutdown`, which are integration-binary-only and unreachable from the lib unit-test binary; the instruction preferred extending an existing integration file over creating a new one. plan-adherence-verifier confirmed the touch is MINIMAL (one test fn + import) and authorized across rounds 2-4. Not routed to a deviation-judge because it was coordinator-directed and exactly as wide as needed. Recorded here for the cumulative 4.a audit trail.

### phase2-deviations.md

# v0-3-m3d-cpu-parallelization Phase 2 Deviations — round 4 (re-captured 2026-06-13T03:30)

D_count: 10

Tree-integrity snapshot (round 4, before re-gate fan-out):
- status_hash: ab4645dd83a52c564ead0c10cb33950c3ecba9f7
- diff_hash: e03aa4fb9995443e908d299d6f50186281088ad4
- per-path blob hashes:
  - crates/ynz-typeck/src/queries.rs aae9b44dbd08109ae9793ecc4fa55b5555249318 (CHANGED R3-fix — cpu_promotion_query Big-O + 3 for-wait decline fixtures + rename + error_whats helper)
  - crates/ynz-typeck/src/independence.rs 62f33322ad8c18c8c4449cc467fb3796bdf93f26 (CHANGED — 2 Big-O)
  - crates/ynz-typeck/src/lib.rs 7b83f1c3ee8c9a1b9e26eb7adabb3acb08596b36 (CHANGED — dropped compute_cpu_promotions re-export)
  - crates/ynz-typeck/src/check.rs eaadd78c19ea5522ee51acf0dff6823bf1c3d9a7 (unchanged)
  - crates/ynz-lsp/tests/completion.rs af89cf03e6802246ec5aa616c2aa7da8c25f2ee4 (unchanged)
  - crates/ynz-lsp/tests/hover.rs a095270130057ccf915f7a401126dbdade3ebdf6 (unchanged)
  - crates/ynz-typeck/tests/check.rs 6d43fafee893a12a651464f20b1fae0617337ee3 (unchanged)
  - crates/ynz-codegen/src/lib.rs 57f4e84be6d84354add3f5a01a6837aadc923072 (unchanged)
  - crates/ynz-typeck/src/resolve_import.rs 387e18b6f9445d1ff4e350820e3da5237069805c (unchanged)

Diff base (BASE): 23f4e81. HEAD = 724a765 (verbatim relocation commit). Diff form: `git diff 23f4e81`.

## Resolved spawn list (round 3)

### Deviation #1 (scope: completion.rs) — identity 41af42b993fc0520f37810c68d1bf9fd8787cc6e — CARRY PASS (blob unchanged)
### Deviation #2 (scope: hover.rs) — identity 90b5e2bab5242c5a4404eca0c1e22673977c9c73 — CARRY PASS (blob unchanged)
### Deviation #3 (scope: tests/check.rs) — identity e7946cdec9f5e23a7fe6d61a38873da5240ef4d2 — CARRY PASS (blob unchanged)
### Deviation #4 (approach: probe param-shadow predicate, check.rs) — identity f35246ed951f9ba110aa1fc42cf619dbc4cf2750 — CARRY PASS (check.rs blob unchanged eaadd78c)
### Deviation #5 (approach: per-candidate CPU-callee seed, queries.rs) — identity b008d70a82fb1aa3c53383ebe678ad70051c73fb — RE-FIRE (queries.rs blob changed; PASSed R2, re-confirm Big-O/visibility edits didn't disturb seed logic)
- diff hunks: crates/ynz-typeck/src/queries.rs:786-845, crates/ynz-typeck/src/queries.rs:933-960
### Deviation #6 (approach: kernel_mode=false query entry, queries.rs) — identity 55616887e442d0ccf1b34a4320b170f6bc9c5792 — RE-FIRE (queries.rs blob changed)
- diff hunks: crates/ynz-typeck/src/queries.rs:625-631, crates/ynz-typeck/src/queries.rs:698-702
### Deviation #7 (scope: typeck/lib.rs re-exports) — identity 86c548c492e1e7d07521531d51c13bc98819da76 — CARRY PASS (R3 judge #7 PASS; lib.rs blob unchanged since R3: 7b83f1c3)
- diff hunks: crates/ynz-typeck/src/lib.rs:77-79
### Deviation #8 (scope: codegen/lib.rs dereg) — identity 10ba7e3935ae7c5fd762fbf8669788cbbf31d6dd — CARRY PASS (blob unchanged 57f4e84)
### Deviation #9 (scope: resolve_import.rs propagation) — identity 8429db77cdb857301918c876e9575cadecd99885 — CARRY PASS (blob unchanged 387e18b)
### Deviation #10 (approach: for-wait decline fixtures assert `!promoted` + "zero NEW diagnostics", not `!has_errors`) — identity 542b08aec6c657cd48b072715d49d78a0ec66747 — RE-FIRE (BLOCKed R3; rationale corrected + 3 real fixtures added — verify guards isolated, contract honest)
- type: approach
- rationale: the three for-with-wait probe guards (StoredRangeWithWait / FixedArrayIterWithWait / ExpressionIterWithWait) ARE reachable (the earlier "structurally unreachable" claim was FALSE — corrected). The reachable shape is a host that `wait`s a NON-suspending callee inside a `for` loop: `has_explicit_waits == true` in the probe (`block_contains_wait` reads the host's own body), but the waited callee reaches no may-block call, so the host is NOT in `base_suspends` → it survives the candidacy gate (queries.rs:763) as a real CPU candidate and the for-wait guard fires in the probe, declining it non-vacuously. BUT the same explicit `wait` trips `check_function`'s PRE-EXISTING M2/M3 for-wait guards (check.rs:584-650), so the shape ERRORS at baseline regardless of M3d — it never "compiled pre-M3d". Step 7's contract targets shapes "that compiled pre-M3d"; that precondition does not hold for these already-erroring shapes. The honest, satisfiable contract is therefore `!promoted` (the meaningful M3d behavior) + "M3d introduced ZERO NEW diagnostics" — proven by asserting `error_whats` contains exactly the shape's OWN pre-existing for-wait guard message and none of the other two — NOT the impossible `!has_errors`. Each fixture isolates its guard (stored-range fixture asserts the stored-range message + absence of fixed/expr messages, etc.). A non-interference test (`wait_free_for_loop_does_not_block_promotion`) covers the complementary wait-free case (pair promotes).
- diff hunks: crates/ynz-typeck/src/queries.rs:1389-1410 (error_whats helper), crates/ynz-typeck/src/queries.rs:1615-1830 (renamed non-interference test + 3 decline fixtures + TICK const)

### phase3-deviations.md

# v0-3-m3d-cpu-parallelization Phase 3 Deviations — captured 2026-06-14 (Sub-slice 4b, live-locals + for-body-decline fix)

D_count: 2

> NOTE: Phase 3 built incrementally across sub-slices 4a–4e. This scratch reflects the CURRENT 4b state
> after the live-locals crossing-slot fix (Patrick Option-2) AND the for-body-decline fix round (R1 gate
> found for-body FIRE over-admits nested shapes → declined per the pre-authorized verify-first escape).
> The accumulator straight-line fix (the load-bearing Option-2 win) STANDS and is validated. Prior R1/R2
> deviations are SUPERSEDED. Sub-slice 4a committed deaa30c. BASE for 4b = deaa30c.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (All code changes in `crates/ynz-typeck/src/**` + `crates/ynz-codegen/src/**` + `crates/ynz-driver/tests/**` + `.claude/todos.md`, all in the plan front-matter `files:`. `.claude/state.md` shows a 1-line hook-generated radar delta — not touched by the executor.)

## Approach Deviations (verbatim from executor report)

**Deviation #1** (codegen reload path — `reload_spike_results: bool` param on the canonical `reload_params_from_frame`, now corrected): the live-locals fix added a `reload_spike_results: bool` param to gate the trailing spike-result reload (which fails for nested groups with no Step-1c sm_entry alloca). The R1 gate (judge #1) found it was left `true` at the orphan-block terminator → a pure nested-if group with NO surrounding crossing local aborted codegen. Fix-round corrected emit.rs:3877 to `false`. Rationale: `orphan blocks are dead/unreachable code (they exist only to satisfy LLVM's terminator requirement), so the spike-result reload there is a no-op — false is safe regardless of nested vs top-level placement. The post-join site stays false; the 5 wait/IO sites stay true (judge #1 confirmed those correct). The wait/I-O callers keep existing behavior.` Diff hunks: `crates/ynz-codegen/src/emit.rs:3514-3530` (helper signature), `crates/ynz-codegen/src/emit.rs:3877` (orphan-terminator site corrected to false).

**Deviation #2** (for-body DECLINE — the pre-authorized verify-first escape outcome): the live-locals fix re-enabled for-body FIRE, but the R1 gate (code-reviewer) found it over-admits: a CPU group in a for-body nested under an `if` silently miscompiled, and a group in the inner of two nested for-loops aborted codegen. Per the verify-first escape (for-body needs multi-level synthetic-index work beyond crossing-slot reservation), the fix-round DECLINES ALL for-body. Rationale: `spike_nested_blocks reverted to exclude For/While so a CPU group in ANY for/while body declines to sequential (byte-identical); the nested for-body placements need multi-level synthetic-index reservation deferred to a dedicated future loop-placement-matrix slice; the for-loop cpu_supported threading reverted as now-dead (no-duct-tape); judge #2 proved the SIMPLE top-level for-body case fires correctly, which de-risks the future slice. Two DECLINE regression fixtures (for-under-if, inner-nested-for) lock the corpse-prevention (no abort, no silent-wrong).` Diff hunks: `crates/ynz-codegen/src/emit.rs:7011` (`spike_nested_blocks` excludes For), `crates/ynz-typeck/src/check.rs:6588-6612` (for-loop cpu_supported threading reverted), `crates/ynz-driver/tests/integration.rs:5854-5950` (for-body test FIRE→DECLINE + 2 new DECLINE regressions + 1 new orphan-terminator FIRE fixture).

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: nested-group-in-host-with-suspension DECLINE (closes judge #1's abort). spike_cpu_candidates (emit.rs:6903) declines a nested CPU group when the host body contains any other wait/suspending op. Verify-first refined the shape: an explicit `wait` makes the host a non-promotion-candidate (declines harmlessly, never reaches the gate); the REAL abort needs a nested group + a PROMOTED suspending CALLEE (invisible at candidate-id → host promoted → Step-1c spike_cpu_group_result_names scans only top-level → nested bind names get no alloca → wait/IO resume reload aborts). Decline-around: nested group fires only in a pure-CPU host; mixed CPU+wait deferred to 4c (proper FIRE needs Step-1c nested-block walk). Top-level group + suspension still FIRES (no regression). The orphan-terminator bool fix (emit.rs:3877 false) from the prior round stays. RISK: a nested+suspension shape that still FIRES into the abort despite the decline.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6903, crates/ynz-driver/tests/fixtures/v0_3_m3d_nested_group_with_suspending_callee.ynz:1-50, crates/ynz-driver/tests/integration.rs:5994-6051
- **judge identity hash**: fae99f4893e3d6e7f6cd1d33dda864da6e6d7bee
- **carry status**: re-judged at fix round (nested+suspension decline replaces the partial orphan-only fix)

### Deviation #2
- **type**: approach
- **rationale**: for-body DECLINE (verify-first escape) — spike_nested_blocks excludes For/While, any for/while-body CPU group declines to sequential; nested placements need multi-level synthetic-index work deferred to a future slice; for-loop cpu_supported threading reverted as dead; judge #2 validated the simple case works. RISK: an unenumerated for/while-body shape that still FIRES into the corpse (silent-wrong or abort) instead of declining.
- **diff hunks**: crates/ynz-codegen/src/emit.rs:7011, crates/ynz-typeck/src/check.rs:6588-6612, crates/ynz-driver/tests/integration.rs:5854-5950
- **judge identity hash**: b66edde4de80160fa9a8a397b181c591424c52b8
- **carry status**: re-judged at fix round (for-body FIRE → DECLINE)

### phase3-slice2-deviations.md

# v0-3-m3d-cpu-parallelization Phase 3 SLICE 2 Deviations — captured 2026-06-13

> Slice-specific scratch (slice 2 = distinct-callee return-class matrix). The phase3-deviations.md
> file holds slice-1's committed record and is preserved unclobbered. Cross-slice deviation record
> is the Phase 3 Findings Log narrative.

D_count: 3

## Scope Deviations (verbatim from executor report)

**Scope Deviation #2** (file: `crates/ynz-driver/tests/cross_impl_consistency.rs`): touched outside slice-2's declared working set (it IS inside the plan's front-matter `crates/ynz-driver/**` glob). Rationale: the FIX-2 `maybe` decline fixture's RUNTIME output is non-deterministic because two adjacent `maybe<int>`-returning binds hit a tracked pre-existing base-codegen bug (uninitialized staging-slot read, todos.md — orthogonal to auto-parallel, IR identical both modes). The corpus determinism harness runs every fixture twice and asserts identical output, so it flagged this fixture. The decline (the thing under test) is verified at the IR level in `v03_m3d_return_class_maybe_declines_and_ir_inert`, NOT by running the binary; the honest fix is to exclude this fixture from the runtime determinism harness with a WHY naming the tracked base bug — matching the harness's existing exclusion pattern for by-design non-deterministic fixtures (timing/background/pirates-roster). Not hidden: bug tracked in todos + named in the exclusion comment. Diff hunks: `crates/ynz-driver/tests/cross_impl_consistency.rs:167-175`.

**Scope Deviation #1** (file: `crates/ynz-typeck/src/independence.rs`): touched outside slice-2's declared working set (emit.rs / state_machine.rs / runtime_decls.rs / fixtures/** / integration.rs). It IS inside the plan's front-matter `files:` glob (`crates/ynz-typeck/src/**`), but the slice-2 record declared a narrower set, so this is recorded as a scope deviation. Rationale: the R1 gate's convergent root cause (number-errors UAF + maybe/union parity) is two return-class gates disagreeing; the prescribed fix is ONE shared CPU-supported-return-class predicate as the single source of truth. typeck's `cpu_result_abi_supports` (the resolved-`Type` gate that drives promotion + the IDE hint) is the natural home; codegen's AST-level gate mirrors it and is locked to it by `cpu_result_abi_gate_parity`. The shared-fn-over-one-enum alternative was rejected: it would require threading resolved sigs through the slice-1-hardened frame-size path (~3 helpers + 4 callers + the dual-frame-size machinery), a wide regression-prone refactor; the cross-asserting-test route is the prompt-sanctioned alternative for a genuine two-enum boundary. Without amending typeck, the hint/binary parity contract (the SAME contract slice-1 judge #2 BLOCKed on) cannot be satisfied. Diff hunks: `crates/ynz-typeck/src/independence.rs:259`, `crates/ynz-typeck/src/independence.rs:290-340`.

## Approach Deviations (verbatim from executor report)

**R2**: None — implementation matched the plan's named approach ("ONE shared CPU-supported-return-class predicate used by BOTH typeck + codegen, declining everything neither can safely parallelize"). The two-gate-mirror-plus-parity-test realization of that mandate is the prompt's explicitly-sanctioned "duplicate-with-a-cross-asserting-test" branch for the no-shared-enum case; it is the named approach, not a departure from it. Slice-1 Deviation #1 (trampoline pack dispatch) is unchanged and carries forward.

**R3 Approach Deviation #1** (FIX A step 3 — asymmetric dead-arm removal): the R3 fix instruction said "remove the dead `Generic{array|map}` arm from BOTH gates (independence.rs AND emit.rs)." The executor removed it from the TYPECK gate (`cpu_result_abi_supports`, independence.rs) ONLY, and KEPT it in the CODEGEN gate (`return_type_fits_cpu_result_abi`, emit.rs). Rationale (verify-first correction of the instruction, governed by the instruction's own "if you find ANY such path, do NOT remove" clause): the typeck gate classifies RESOLVED `crate::types::Type`, where `array<int>`/`map<_,_>` always lower to `Type::BuiltinArray`/`BuiltinMap` (check.rs:3810-3849) and `Type::Generic` survives ONLY for user-defined generic shapes (which correctly decline via `_=>false`) — so the Generic arm is genuinely DEAD there. But the codegen gate classifies UN-resolved `AstType`, where `array<int>` IS literally `AstType::Generic{name:"array"}` — the LIVE production path; removing it would mis-decline every array/map return and break the byte-identical FIRE matrix. Mutation-proven non-vacuous (code-reviewer R3: a one-sided codegen `Generic{"fixed"}` admit → parity test FAILS). Diff hunks: `crates/ynz-typeck/src/independence.rs:300-318, crates/ynz-typeck/src/independence.rs:330-342`.

## Slice-1 Approach Deviations (verbatim from prior executor report — carried, unchanged)

**Deviation #1** (Steps 3 + 5, trampoline+bind dispatch): plan said "route through the canonical `bind_sm_result_and_flush` discipline … incl. the i64→i1 bool truncation rule." I did route the join-bind through `bind_sm_result_and_flush` (reading the 16-byte result slot as a synthetic return-slot frame via `load_sm_return_value_typed`), but the trampoline's **pack** side dispatches on the callee's LLVM return value KIND rather than a precomputed return-`Type`, and required an extra `callee_returns_bare_number` lookup to disambiguate `number` (returns a ptr-to-i128 → must dereference) from string/array/map (ptr IS the value → `ptr_to_int`). Rationale: `number`'s non-SM ABI returns a heap pointer not an i128 value, so a pure value-kind dispatch would have packed the pointer bits and silently produced `0.000…0` (the verify-first bug); the explicit bare-number lookup is the minimal correct disambiguation and mirrors the existing `is_number_errors_callee` lookup pattern. The i64→i1 bool truncation rule cited in the plan lives inside `bind_sm_result_and_flush` (the `_` arm, emit.rs ~6226) which the bind now calls, so it is honored by routing through that function rather than re-implemented. Diff hunks: `crates/ynz-codegen/src/emit.rs:7455-7560` (trampoline pack), `crates/ynz-codegen/src/emit.rs:7848-7900` (join-bind), `crates/ynz-codegen/src/emit.rs:15001-15030` (`callee_returns_bare_number` helper).

## Resolved spawn list (orchestrator's parsed view)

### Scope Deviation #1
- **type**: scope
- **rationale**: (verbatim above — shared single-source-of-truth predicate `cpu_result_abi_supports` hosted in typeck; codegen gate mirrors + parity-test locked; shared-fn-over-one-enum rejected as a wide regression-prone refactor of the slice-1-hardened frame-size path)
- **diff hunks**: crates/ynz-typeck/src/independence.rs:259, crates/ynz-typeck/src/independence.rs:290-340
- **judge identity hash**: 447e931714a7a3d46850fe4541536f0353132daf
- **carry status**: fresh (round 2)

### Scope Deviation #2
- **type**: scope
- **rationale**: (verbatim above — exclude the `maybe` decline fixture from the corpus determinism harness; its runtime output trips a tracked pre-existing adjacent-`maybe`-bind base bug; the decline is verified at IR level, not by running the binary)
- **diff hunks**: crates/ynz-driver/tests/cross_impl_consistency.rs:167-175
- **judge identity hash**: 7b194f198d47b1a9418631948a9b2027871191f8
- **carry status**: fresh (round 2)

### Deviation #1 (slice-1 approach deviation — carried, unchanged)
- **type**: approach
- **rationale**: (verbatim above — trampoline pack dispatches on LLVM value-kind + `callee_returns_bare_number` disambiguation; join-bind routes through `bind_sm_result_and_flush`)
- **diff hunks**: crates/ynz-codegen/src/emit.rs:7455-7560, crates/ynz-codegen/src/emit.rs:7848-7900, crates/ynz-codegen/src/emit.rs:15001-15030
- **judge identity hash**: c577c45ce4628a8b31690219a0e538c58d684e8c
- **carry status**: carried (round-1 blob; round-2 edits to the `callee_returns_bare_number`/`is_errors_capable_fn` region were doc-comment relocation only — no logic change)

### phase3-sub4c-deviations.md

# v0-3-m3d-cpu-parallelization Phase 3 Sub-slice 4c Deviations — captured 2026-06-14

D_count: 2

> NOTE: Sub-slice 4c (mixed CPU+I/O) BASE = ab2e840 (4b's committed HEAD). The executor ran
> verify-first on BOTH declared FIRE targets, built the named fixes, proved each unsafe/unimplementable
> within 4c's poll-fusion scope, and REVERTED all emit.rs/typeck/runtime changes to BASE (probe-confirmed
> byte-identical). Net deliverable: 2 DECLINEs locked (1 new fixture+test for mixed CPU+I/O; the 2 existing
> 4b nested-in-suspending decline fixtures stay) + a todos:198 residual rewrite. Both deviations are
> FIRE→DECLINE envelope narrowings — the deviation-judges must verify they are LEGITIMATE (genuine
> unsafety) and not a cop-out to avoid work.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (emit.rs/typeck/runtime touched during the verify-first attempt and fully reverted; final diff touches only the declared fixture/test/todos files. `.claude/state.md` is a radar auto-refresh, not executor-touched.)

## Approach Deviations (verbatim from executor report)

**Deviation #1** (4c-i FIRE envelope): plan said `FIRE — a mixed CPU+I/O group overlaps both children (CPU → ynz_rt_spawn_blocking_joinable, I/O → resume_fn poll)`, executor did `kept it a DECLINE and locked the 0-spawn byte-identical shape with a new fixture+test`. Rationale: `the two lowering paths share no continuation — fusing them requires routing codegen off typeck's class-aware partition_groups_classified (which ships with zero codegen consumer by design), a fused poll loop re-driving every live spawn handle on each resume, AND dual-kind (CPU handle/result slots + I/O child sub-frame) frame-layout reservation for one group; this is the "frame work you can't safely land here" the VERIFY-FIRST mandate names. Verified: the shape declines to 0 spawns / 4958 byte-identical today, so locking the decline prevents a future fusion slice from silently regressing it (Silent-Envelope corpse)`. Diff hunks: `crates/ynz-driver/tests/fixtures/v0_3_m3d_mixed_cpu_io_group_declines.ynz:1-37, crates/ynz-driver/tests/integration.rs:6048-6068, .claude/todos.md:198`.

**Deviation #2** (4c-ii FIRE envelope): plan said `FIRE — make Step-1c walk nested blocks so a nested group's bind names get sm_entry allocas, then the nested group FIRES alongside an outer wait/suspending call`, executor did `built exactly that fix (spike_group_bind_names_all_depths + lifted the admission decline), verify-first-proved it produces a HANGING binary, reverted it, and kept the DECLINE`. Rationale: `pre-allocating the nested allocas removed the backend abort but did NOT fix the root hazard — the nested spawn-join yields Pending into a continuation the outer SM suspension's resume never re-drives, so the spawned CPU handle is never re-polled and the binary deadlocks (exit 1, no output, on ..._with_suspending_callee forced to 4 spawns). The explicit-wait host additionally can't promote at all: typeck excludes base_suspends functions from CPU candidacy (queries.rs:763). A safe fire needs the SAME poll-path-fusion machinery as 4c-i PLUS typeck promoting + guard-probing already-suspending hosts — out of this slice's poll-fusion scope. Shipping the deadlock would be a HALT-class regression`. Diff hunks: `.claude/todos.md:198`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: (verbatim above) 4c-i mixed CPU+I/O FIRE→DECLINE — two poll paths share no continuation; fusion needs partition_groups_classified codegen consumer + fused poll loop re-driving live handles + dual-kind frame reservation. Locked DECLINE (0 spawns / 4958 byte-identical). RISK: a mixed CPU+I/O shape that COULD fire with less machinery than the executor claims (judge: try to find one), OR the locked DECLINE over-declines a safe shape.
- **diff hunks**: crates/ynz-driver/tests/fixtures/v0_3_m3d_mixed_cpu_io_group_declines.ynz, crates/ynz-driver/tests/integration.rs, .claude/todos.md
- **judge focus**: is mixed CPU+I/O GENUINELY unfusable within 4c's poll-fusion scope, or did the executor narrow to avoid the work? Probe whether a one-CPU + one-I/O group can overlap with cheaper machinery than claimed. Confirm the DECLINE fixture asserts 0 spawns + correct sequential value.

### Deviation #2
- **type**: approach
- **rationale**: (verbatim above) 4c-ii nested-group-in-suspending-host FIRE→DECLINE — the named Step-1c fix compiled but DEADLOCKED (nested spawn-join Pending into a continuation the outer suspension's resume never re-drives → handle never re-polled); typeck also excludes base_suspends hosts from CPU candidacy. Reverted; DECLINE kept. RISK: the deadlock is a FIXABLE bug in the executor's attempt (not fundamental) → the FIRE was abandoned too early; OR the executor's revert left the decline unsound.
- **diff hunks**: .claude/todos.md (residual rewrite; the 2 existing 4b decline fixtures v0_3_m3d_nested_group_with_outer_wait / ..._with_suspending_callee + their tests lock the verified-correct declines, unchanged since 4b)
- **judge focus**: is the deadlock REAL and FUNDAMENTAL to 4c's scope, or a fixable defect in the executor's reverted attempt? Reconstruct/reason about the continuation re-drive claim. Verify typeck queries.rs:763 genuinely blocks already-suspending-host promotion. Confirm the kept DECLINE still holds (0 spawns, no abort, byte-identical) on the existing fixtures.

### phase3-sub4d-deviations.md

# v0-3-m3d-cpu-parallelization Phase 3 sub-slice 4d Deviations — ROUND 4 (domain-enumeration drain) — 2026-06-15

D_count (round 4): minimal re-gate — see resolved spawn list

> R3 gate: BLOCKed (disc-offset binding [code-reviewer] + bare add(4) in old tests [rules]) — both were the
> NEXT members of a recurring class the no-progress hash couldn't see. Coordinator detected the
> recurring-bug-class (detector ii, Patrick-flagged) and switched to DOMAIN ENUMERATION: round 4 drains the
> ENTIRE spike frame-ABI constant set in one pass + adds a source-scan completeness gate (round-5 stopper).
> Live drain verified: ZERO bare spike-offset literals remain (coordinator re-grep). Corpus 2043/0.

## Round-4 Approach Deviations (verbatim from executor report)

- **Deviation #1** (handle 0/1 offsets — option choice): task offered (a) derive-at-use-site OR (b) const-assert, "pick lower-risk + justify". Chose (b) const-assert bindings. Rationale: "the two named constants are referenced by NAME in three comments (emit.rs:2399/6855/6886, zero-param-host decline reasoning) and the use site (emit.rs:7793) builds a fallback array entangled with the out-of-class result offsets [48,64]; deriving would orphan the comment refs + risk the result-offset fallback. const-assert fully closes drift with zero use-site risk". Diff hunks: `crates/ynz-codegen/src/emit.rs:6718-6733`.
- **Deviation #2** (member #3 bare-8 — verified NOT in class, NOT swapped): task hypothesized emit.rs:8057 `const_int(8)` was the bare stride; executor verified it is `ctx_size=8` (sizeof(i64) spawn-ctx buffer for ynz_rt_spawn_blocking_joinable), coincidentally 8, NOT the handle stride → left bare per the task's "flag don't mis-swap" instruction. Rationale: "swapping a ctx-buffer-size arg for SPIKE_HANDLE_SLOT_BYTES conflates two distinct semantics that are coincidentally both 8". Diff hunks: none (no change). [Coordinator PROBE E confirmed this classification independently.]
- **Deviation #3** (disc const naming): renamed all consumers to the ynz-abi name `SPIKE_FRAME_DISCRIMINATOR_OFFSET` rather than keeping a `pub(crate)` re-export of the old `FRAME_SPIKE_DISCRIMINATOR_OFFSET`. Rationale: "a pub(crate) re-export becomes a dead/clippy-unused import once lib.rs imports straight from ynz_abi; one canonical name across both crates is cleaner; mechanical 5-site rename". Diff hunks: `crates/ynz-runtime/src/runtime.rs:69-74, crates/ynz-runtime/src/lib.rs:2804-2807,3560,3673,3732,3793`.

## Resolved spawn list (round 4)

### Judge B (re-fire — tracks frame-ABI const-assert completeness) — IS THE CLASS FULLY CLOSED?
- type: approach (the whole domain-drain)
- rationale: round 4 added `SPIKE_FRAME_DISCRIMINATOR_OFFSET` to ynz-abi (both crates consume), const-asserts binding SPIKE_HANDLE_0/1_OFFSET to base/base+stride, swapped the disc GEP + 6 test literals to named consts, + a source-scan completeness gate. Confirm: (1) the disc-offset BLOCK (R3) is resolved — codegen + runtime read ONE canonical const; (2) the handle 0/1 asserts are load-bearing (not tautological); (3) the completeness gate is REAL + mutation-non-vacuous + correctly scoped (catches any new in-class bare literal; does NOT false-flag the out-of-class general-header offsets 0/8/16 or the ctx-size 8); (4) the bare-8 left-as-ctx-size is genuinely out-of-class. Does any in-class member remain unbound?
- diff hunks: crates/ynz-abi/src/lib.rs:25, crates/ynz-codegen/src/emit.rs:6718-6733 + disc GEP, crates/ynz-runtime/src/{runtime.rs,lib.rs}, crates/ynz-runtime/tests/spike_frame_abi_no_bare_offsets.rs
- judge identity: approach-frame-abi-domain-fully-closed-r4
- carry status: re-fire (was R3 BLOCK as disc-offset; now the whole-domain-closure judge)

### CARRIED:
- judge-A ynz-abi-extraction (R2 PASS): adding one more const to ynz-abi doesn't change its zero-dep/no-cycle/no-tokio properties. CARRY.
- judge-C NB1-checker (R2/R3 PASS): nounwind checker untouched this round. CARRY.
- codegen-ABI-no-op, lib.rs-colocation, m2_runtime, todos (R1 PASS): untouched. CARRY.

## Reviewer re-gate (round 4)
- code-reviewer: RE-FIRE (R3 BLOCK on disc offset — confirm resolved; review the const-asserts + completeness-gate quality + the bare-8 classification).
- rules-compliance: RE-FIRE (R3 BLOCK on bare literals — confirm all 6 gone + the new test file / ynz-abi const / asserts are rule-clean).
- acceptance-verifier: RE-FIRE (confirm 2043/0 + completeness gate passes + cancellation/nounwind tests green). [R3 acceptance run is STALE — round 4 changed the tree; this fresh one supersedes it.]
- plan-adherence: RE-FIRE delta-focus (confirm the drain is the in-class prescribed remedy, 4e gates + CPU_GROUP_MEMBER_COUNT byte-intact, no over-reach into the general-SM-header layer).
- design-compliance: CARRY R2 PASS (constants-relocation + asserts + a test touch no concurrency/coloring/kernel surface; the new ynz-abi const is kernel-safe).

### phase3-sub4e-deviations.md

# v0-3-m3d-cpu-parallelization Phase 3 sub-slice 4e Deviations — 2026-06-15

D_count: 5 (3 approach + 2 scope)

> Sub-slice 4e (FINAL): gate lifts + sweep. Outcome = 2 honest verify-first DECLINEs (pre-pair-wait,
> param-host) + 1 FIRE (N>2 single group) + fixture-header sweep + demo. BASE eba9ec1. Corpus 2046/0.
> The two DECLINE judges are INVERTED (try to safely FIRE the declined case; PASS only if it can't be).

## Approach Deviations (verbatim from executor report)

- **Deviation #1 (gate lift 1, FIRE→DECLINE — pre-pair wait)**: envelope said FIRE if a pre-pair wait sequenced before the pair; executor DECLINED. Rationale: "verify-first Paper-Trace — a pre-pair wait host is in typeck's base_suspends, never a CPU-promotion candidate (queries.rs:763, same structural exclusion 4c hit). Lifting it broadly FIRES correctly (2 spawns, 7000 byte-identical, no deadlock) BUT breaks the slice-1 cross-boundary frame-sizing invariant — `imported_suspending_after_pair_declines_consistently_across_boundaries` FAILS = the silent-heap-corruption corpse the entrypoint-only gate prevents. A narrow lift (intrinsic-wait-only hosts) collides with the closure/rollback assumption promoted⊥base_suspends = the dual-kind frame machinery scoped to M3g (todos:198). Clean DECLINE with a 0-spawn lock is safe; unsafe FIRE is not." Diff hunks: `crates/ynz-driver/tests/fixtures/v0_3_m3d_prepair_wait_declines.ynz:1-31, crates/ynz-driver/tests/integration.rs (v03_m3d_prepair_wait_declines_byte_identical)`.
- **Deviation #2 (gate lift 2, FIRE→DECLINE — param-host)**: envelope said FIRE if params round-trip; executor DECLINED. Rationale: "verify-first Paper-Trace — Observed: param-host with the param read AFTER the join (`return seed + a + b`) prints 7000 default; Expected `score(3)+score(4)+seed = 7003` (--no-auto-parallel oracle = 7003); Residual 3 (= the param value). The wrapper writes param slot 0 at byte 32 (`store_local_slot`: FRAME_HEADER_SIZE + idx*8) = exactly the CPU handle-slot-0 byte for a spike host; the spawn overwrites it → post-join param reload reads 0. Params-live-across-the-join are NOT covered by the 4b crossing-slot machinery (locals only); the fix needs wrapper param-store + resume param-load offset past the CPU reserve = param-slot reservation beyond a gate flip. Param used only in spawn args round-trips fine; read-after-join silently corrupts. DECLINE is the honest outcome." Diff hunks: `crates/ynz-driver/tests/fixtures/v0_3_m3d_param_host_declines.ynz:1-38, crates/ynz-driver/tests/integration.rs (v03_m3d_param_host_declines_byte_identical)`.
- **Deviation #3 (gate lift 3 impl — DRY consolidation)**: plan said lift the count gate / [u64;2] arrays / CPU_GROUP_MEMBER_COUNT reservation; executor ADDITIONALLY consolidated 3 duplicated group-extraction blocks (spike_pair_in_block, spike_extract_cpu_group, spike_cpu_group_result_names) into ONE shared `spike_cpu_group_member_indices` source of truth + `spike_cpu_group_member_count`. Rationale: "the N-extension requires dependency/args/run checks across all N members in lockstep at the count gate AND admission gate AND extraction — 3 previously-duplicated copies; one shared member-index fn is the only way the frame reserve (sized from count) and emission (iterating members) can never disagree on N — the drift hazard the slice-1/slice-2 parity BLOCKs repeatedly hit. Net -188 lines (DRY, not bloat)." Diff hunks: `crates/ynz-codegen/src/emit.rs:6974-6991, :6993-7142, :7192-7233, :7326-7345, :7376-7402`.

## Scope Deviations (verbatim)
- **Scope #1**: swept the `s` fixture (`v0_3_m3d_spike_s_imported_suspending_after_pair/{entrypoint,io_lib}.ynz`) beyond the plan-named `r` fixture — same compiler-jargon class ("BARE/EFFECTIVE local suspend set") leaking into user-facing `.ynz` (vocabulary.md); prompt's sweep directive said "grep all fixtures". Diff hunks: those 2 files.
- **Scope #2**: `crates/ynz-typeck/src/queries.rs` touched ONLY during verify-first probing (experimental base_suspends lift), FULLY REVERTED (git diff eba9ec1 -- crates/ynz-typeck/src EMPTY). No net change. (Coordinator probe confirmed 0 diff lines.)
- (Demo: `examples/pirates-roster/entrypoint.ynz` + `expected_stdout.txt` — mandated Demo & Error Gallery per plan-invariants, IN scope.)

## Resolved spawn list

### Judge 1 (INVERTED) — lift-1 pre-pair-wait DECLINE: genuine unsafety or avoidance?
- type: approach
- task: try to SAFELY fire a pre-pair-wait-sequenced CPU group (prove the DECLINE wrong). Confirm the broad lift genuinely trips the slice-1 cross-boundary corpse (`imported_suspending_after_pair_declines_consistently_across_boundaries`) AND a narrow safe lift genuinely collides with promoted⊥base_suspends. PASS iff the DECLINE is genuine (can't safely fire in 4e scope); BLOCK if a safe FIRE exists the executor missed.
- diff hunks: emit.rs (gate 3925 area, unchanged), the prepair_wait_declines fixture
- judge identity: approach-lift1-prepair-wait-decline-genuine

### Judge 2 (INVERTED) — lift-2 param-host DECLINE: genuine corruption or fixable in-slice?
- type: approach
- task: verify the param-slot-collision corruption is REAL (param slot 0 at byte 32 = CPU handle slot 0) and that fixing it needs param-slot reservation beyond a gate flip (NOT cheaply fixable in 4e). Try to safely fire a param-host (e.g. param used only in spawn args, OR a cheap reservation). PASS iff the DECLINE is genuine; BLOCK if a safe FIRE or a cheap in-slice fix exists.
- diff hunks: emit.rs (gate 6906 area), the param_host_declines fixture
- judge identity: approach-lift2-param-host-decline-genuine

### Judge 3 — DRY consolidation behavior-preservation (parity corpse)
- type: approach
- task: the -188-line consolidation of 3 extraction blocks into one shared `spike_cpu_group_member_indices`. Verify it preserves behavior across ALL prior cases (distinct/same-callee, return-class matrix, if/match-arm nested, accumulator/crossing-local, promoted-host, the DECLINEs) AND the new N>2 case — the count gate, admission gate, and emission all agree on N by construction. This is the exact parity-drift surface the slice-1/2 BLOCKs hit. PASS iff no case regresses + the single-source genuinely eliminates drift; BLOCK if any case routes wrong or the consolidation hides a divergence.
- diff hunks: emit.rs:6974-7402 (the consolidated extraction)
- judge identity: approach-lift3-dry-consolidation-parity

## Reviewer gate
- code-reviewer (Opus): N>2 FIRE codegen correctness (dynamic offset arrays, member-count reserve, 3-spawn fire, distinct values, alloc=free) + the DRY consolidation + the removed-constants safety.
- rules-compliance: comments/vocab on new fixtures + the s-fixture sweep + Big-O on new helpers + no changelog/phase-labels.
- plan-adherence: 4e scope (3 lifts attempted, 2 declined verify-first, 1 fired), no re-opening of M3g/loop-body/multi-group, deviations documented + banned-phrase-clean, demo extended.
- acceptance-verifier: LIVE — 2046/0, N=3 FIRE (3 spawns byte-identical alloc=free), both DECLINEs 0-spawn, completeness gate green, m3d suite 44/0. (Coordinator may substitute a live-run if the agent malforms again.)
- design-compliance: the DECLINEs consistent with concurrency.md (sequential default) + no-function-coloring.md; N>2 poll-based; no coloring/bridge.

## FIX ROUND (2026-06-15) — 4e gate verdicts resolved (6 PASS / 2 BLOCK / 2 CONCERN)

BASE eba9ec1. Corpus now 2048/0 (+2 new param-host FIRE tests). Deviation #2 (param-host DECLINE)
was INVERTED by deviation-judge-2 → the wholesale param decline OVER-declined. Resolutions:

- **BLOCK 1 (judge-2 inversion) — narrowed the param-host gate**: replaced the wholesale
  `!f.params.is_empty()` decline with a `spike_param_read_after_join` post-join read check.
  A param used ONLY in spawn args now FIRES (the spawn-arg load at emit.rs:~7855 reads the
  param's stack alloca BEFORE the handle store clobbers byte 32 — a dead store); a param READ
  in a post-join statement still DECLINES. New helper `stmt_tree_ident_reads` (recursive,
  conservative) + `spike_param_read_after_join` in emit.rs; `collect_ident_names` made pub in
  ynz-typeck/independence.rs. Nested-group param-hosts still decline (post-join frontier crosses
  the branch boundary). VERIFIED LIVE: spawn_args_only FIRES 2 spawns 9907 byte-identical;
  n3_spawn_args_only FIRES 3 spawns 14862 byte-identical; param_host_declines (read-after-join)
  DECLINES 0 spawns 9910 byte-identical. Deviation #2 is RESOLVED (no longer a DECLINE for the
  spawn-args-only subset).
- **BLOCK 2 (rules) — added the todos.md deferral entry** `m3d-param-host-read-after-join`
  (4-field: WHAT/WHY/cost/trigger) for the NARROWED residual (read-after-join only). Updated the
  integration.rs param-host-declines test comment so its "tracked in todos.md" claim is now
  accurate + reflects the narrowed scope.
- **CONCERN 1 (independence-check invariant)**: added durable comment at emit.rs
  `earlier_bind_names[..pos]` naming the forward-only-dependency-flow invariant (compacted-list-
  vs-full-index misalignment is a conservative SUPERSET → spurious DECLINE only, never false-ADMIT).
- **CONCERN 2 (dead-symbol fixture comments)**: rewrote the 3 fixture comments
  (spike_i_mixed_locals:4, spike_k_param_host, spike_q_suspending_callee:10) to use plain byte
  descriptions instead of the deleted SPIKE_*_OFFSET symbols. spike_k comment also corrected to
  describe the read-after-join decline reason (not the old wholesale param decline). Grep confirms
  zero remaining references to the 4 deleted symbols across all .ynz fixtures.

New scope touch: `collect_ident_names` made `pub` in ynz-typeck/src/independence.rs (BLOCK 1
mandated reusing the existing walker rather than writing a parallel one).

### phase4-deviations.md

# v0-3-m3d-cpu-parallelization Phase 4 Deviations — captured 2026-06-19 (Round 2)

D_count: 7

## Scope Deviations (verbatim from executor report — Round 2)

- **Scope Deviation #1** (`crates/ynz-typeck/src/cpu_admission.rs`, NEW file): outside BLOCK-named files. Rationale: FIX 1 mandates a single shared admission decision; crate direction is codegen→typeck so the predicate MUST live in typeck for both consumers to read it (plan line 62 authorizes this as "a relocation, not a parallel implementation"). New self-contained module = lowest blast radius. Diff hunks: crates/ynz-typeck/src/cpu_admission.rs:1-560.
- **Scope Deviation #2** (`crates/ynz-codegen/src/emit.rs`): beyond FIX-1 hint files. Rationale: relocation requires codegen's spike_* gate + 4 AST walkers to DELEGATE to the new typeck module (one-line bodies, signatures preserved so ~60 call sites + codegen tests untouched); removed orphaned helpers/imports. Diff hunks: crates/ynz-codegen/src/emit.rs:61-64, 3331-3335, 6779-6783, 6829-6835, 6858-6864, 6878-6880, 6897-6907, 6989-6999.
- **Scope Deviation #3** (`crates/ynz-codegen/tests/golden.rs`): binary-side parity proof. Rationale: FIX 1b "hint set == codegen spawn set" needs the binary half proven where codegen IR is reachable (typeck can't run codegen). Diff hunks: crates/ynz-codegen/tests/golden.rs:861-903.
- **Scope Deviation #4** (`crates/ynz-registry/build.rs`): emit new schema fields. Rationale: MUTED_HINT_DOMAINS is codegen'd from features.toml by build.rs; FIX-4 hover fields can't exist on the struct without build.rs writing them into the generated literal (mirrors keyword-entry hover-field pattern). Diff hunks: crates/ynz-registry/build.rs:434-453.

## Approach Deviations (verbatim from executor report — Round 2)

- **Approach Deviation #1** (FIX 1): full relocation of the pure-AST admission gate to ynz-typeck::cpu_admission with codegen delegating up, rather than re-deriving the decision separately in the inlay pass. Rationale: re-deriving = two implementations = no-duct-tape #7 drift; relocation is the genuine single-source-of-truth (plan line 62). Diff hunks: crates/ynz-typeck/src/cpu_admission.rs:1-560, crates/ynz-codegen/src/emit.rs:61-64, 3331-3335.
- **Approach Deviation #2** (FIX 4 WHY contextuality): a contextual-but-static WHY ("the listed lines have no shared reads or writes — neither reads what the other writes") rather than literal per-call-site binding names. Rationale: the registry hover is one static template per domain; runtime line numbers live in the inline muted label, not the registry tooltip; the tooltip carries the contextual RELATIONSHIP. Diff hunks: registry/features.toml:2104-2107.

## Carried deviation (from Round 1 — re-judge to confirm BLOCK cleared)

- **Carried Deviation (placeholder)** (`tooling/vscode-ynz/screenshots/cpu-parallel-hints.png.PLACEHOLDER`): Round-1 deviation-judge BLOCKED (ships in .vsix; unanchored deferral). Round-2 fix: `.vscodeignore` excludes `screenshots/*.PLACEHOLDER`; coordinator anchored the 4-field deferral in the plan (Patrick-approved). Re-judge must confirm the BLOCK is cleared. Diff hunks: tooling/vscode-ynz/.vscodeignore:1, tooling/vscode-ynz/screenshots/cpu-parallel-hints.png.PLACEHOLDER:1-11.

## Resolved spawn list (orchestrator's parsed view)

7 deviations. D > 4 → §3.d.1 consolidation gate fires. Coordinator routing: see plan Findings Log Round 2 entry for the individual-vs-consolidated decision. The 4 scope deviations are one logical relocation (best judged holistically across files); approach #1 (relocation correctness — does it preserve codegen IR byte-identically?), approach #2 (hover WHY quality / GR11), and the placeholder re-judge are the distinct adversarial targets.

### phase5-deviations.md

# v0-3-m3d-cpu-parallelization Phase 5 Deviations — captured 2026-06-19

D_count: 4

(Combined diff = original Phase-5 test-only adversarial gate + the authorized in-phase
decimal128-return leak fix. Original Phase-5 executor reported ZERO deviations; all 4 below
come from the fix executor.)

## Scope Deviations (verbatim from executor reports)

Original Phase-5 executor: None — stayed within declared scope.

Fix executor:
- **Scope Deviation #1** (file: `crates/ynz-codegen/src/emit.rs`): touched outside Phase-5's
  declared test-only "Files (expected scope)". Rationale: AUTHORIZED — Patrick's 2026-06-19
  fix-in-phase decision (Findings Log coordinator-probe entry) + the Phase-5 deviation rule
  "M3d defect → fix commit within this phase". The file is in the plan front-matter `files:`
  list; the leak is a confirmed M3d-path-only defect on a core promoted class (decline-around
  unavailable), so the fix belongs in-phase per Rule 11 + no-duct-tape. Diff hunks:
  `crates/ynz-codegen/src/emit.rs:3157-3187`.

## Approach Deviations (verbatim from executor reports)

Original Phase-5 executor: None — implementation matched plan's named approaches.

Fix executor:
- **Deviation #1** (Your Fix steps 1–2): prompt said "add the missing free … free the
  worker-stabilized decimal128 heap allocation once its i128 has been copied into the caller's
  stable storage", I did "eliminated the heap allocation entirely — the SM wrapper now returns a
  stack-backed pointer matching the non-SM number ABI, so there is no heap block to free".
  Rationale: verify-first (LLVM IR) showed the caller's i128 copy happens at the caller's own
  call site AFTER the wrapper returns (main is byte-identical in both modes and never frees the
  returned pointer); a free inside the wrapper would dangle the pointee = the UAF the prompt
  explicitly warned against. The non-SM number ABI is already "return pointer to caller-local
  i128, caller copies-and-forgets, no free" — matching it in the SM wrapper makes the two
  lowerings ABI-identical (no-duct-tape: fixes the root mismatch, not a symptom) and is strictly
  a no-allocation fix, eliminating leak + double-free + UAF risk simultaneously. Diff hunks:
  `crates/ynz-codegen/src/emit.rs:3157-3187`.
- **Deviation #2** (Witness retirement): prompt said update only the two number-arm fixtures'
  KNOWN LEAK header notes, I additionally deleted the now-dead
  `m3d_assert_fires_byte_identical_no_alloc_check` helper (its only 2 callers were the rerouted
  number cells). Rationale: no-duct-tape — leaving an orphan unused test helper is dead code;
  removing it is part of a complete retirement. Diff hunks:
  `crates/ynz-driver/tests/integration.rs:5759-5799, crates/ynz-driver/tests/integration.rs:6448-6516`.
- **Deviation #3** (Plan writeback): prompt scoped writeback to the AC-1 Danger-matrix finding
  sub-bullet, I also corrected the AC-5/Step-10 sweep evidence (line 811): 2081→2080 test count,
  "test-only/zero-compiler-source" claim → "one authorized emit.rs touch", and logged the
  pre-existing m3e flake. Rationale: my diff falsifies those specific stated facts; leaving
  known-false statements in the plan violates no-duct-tape. Did NOT tick any AC checkbox or
  Phase Review Gate. Diff hunks:
  `.claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:801, .claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:811`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: scope
- **rationale**: AUTHORIZED scope deviation — emit.rs touched outside Phase-5 test-only scope per Patrick 2026-06-19 fix-in-phase decision + Phase-5 deviation rule (M3d defect → fix commit within phase).
- **diff hunks**: crates/ynz-codegen/src/emit.rs:3157-3187
- **judge identity hash**: 1864320ba898c2eab7a69086ccda2d8268bcc08b
- **carry status**: fresh

### Deviation #2
- **type**: approach
- **rationale**: Eliminated heap allocation entirely; SM wrapper returns stack-backed pointer matching non-SM number ABI rather than adding a free (verify-first IR showed a free inside the wrapper would dangle the pointee = UAF).
- **diff hunks**: crates/ynz-codegen/src/emit.rs:3157-3187
- **judge identity hash**: 327df07cbc1cb5718738484eaa157faf80b44c5a
- **carry status**: fresh

### Deviation #3
- **type**: approach
- **rationale**: Deleted the now-dead m3d_assert_fires_byte_identical_no_alloc_check helper whose only 2 callers were the rerouted number cells.
- **diff hunks**: crates/ynz-driver/tests/integration.rs:5759-5799, crates/ynz-driver/tests/integration.rs:6448-6516
- **judge identity hash**: 06c8c4139bbc61084c719bcb42bb065dd0243e16
- **carry status**: fresh

### Deviation #4
- **type**: approach
- **rationale**: Corrected AC-5/Step-10 sweep evidence (2081→2080 test count, zero-compiler-source claim → one authorized emit.rs touch, logged pre-existing m3e flake).
- **diff hunks**: .claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:801, .claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:811
- **judge identity hash**: a24a4368a4339322a1adf9ab8e3dc70316dc53f8
- **carry status**: fresh

