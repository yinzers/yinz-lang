# v0-3-m2-wait-and-state-machines Phase 7 Deviations — captured 2026-06-01 (round 3, post-fix)

D_count: 1

## Scope Deviations (verbatim from executor reports across rounds)

Round 2 (codegen): Scope Dev — `crates/ynz-typeck/src/queries.rs` + `tests/check.rs` (add `suspends_set: HashSet<String>` to `CheckOutput` to thread the transitive `suspends` set from typeck → codegen `emit_artifact` — the P6/P7 seam plumbing); `crates/ynz-runtime/tests/m2_runtime.rs` (ynz_rt_spawn gained a 4th arg `recursion_slot_offset`; `SyncValueSm`/`SignalSm` updated to P7 frame layout + `#[repr(C)]` null@8 to prevent Drop chasing garbage); 12 golden IR snapshots (ynz_alloc→ynz_alloc_zeroed + ynz_rt_spawn 3→4 args). Round 3: test file + recursive_cancel fixture only.
Coordinator note: all the above are necessary seam/ABI follow-ons, NOT scope creep. The 5 `integration.rs` env-path snapshots were NOT touched (round-2 poisoning reverted to base by coordinator). The compiled-binary blobs that `ynz run` dropped into fixtures/ were removed + a durable `.gitignore` pattern added.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Round 3, Fix 1 root-cause): coordinator's diagnosis said the cancellation-test flakiness was "timing-raced"; executor identified TWO specific mechanisms instead: (a) `shutdown_timeout(0ms)` thread-join race — with 0ms, `Runtime::shutdown_timeout` returns before worker threads are joined, so `ynz_free` calls inside `SpawnStateFnFuture::Drop` (on worker threads) aren't visible to the main-thread counter read in `ynz_rt_shutdown`; fix = 50ms (joins threads, still cancels before countdown's natural 600ms completion); (b) entrypoint-wrapper-frame counting — the positive-control threshold was `>= 3` but the entrypoint wrapper always allocs its own frame so actual alloc at cancel is 4; fix = threshold `>= 4`. Plus a `CANCEL_TEST_LOCK` static mutex serializing the 3 `recursion_cancellation*` tests. Diff hunks: crates/ynz-driver/tests/m2_state_machine_integration.rs:417-424, crates/ynz-driver/tests/m2_state_machine_integration.rs:452-510.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: cancellation-test determinism — fixed via shutdown_timeout 0→50ms (thread-join-race fix, the real correctness bug), positive-control threshold 3→4 (entrypoint frame), + serialization mutex. Coordinator independently verified 10/10 deterministic.
- **diff hunks**: crates/ynz-driver/tests/m2_state_machine_integration.rs:417-424, crates/ynz-driver/tests/m2_state_machine_integration.rs:452-510
