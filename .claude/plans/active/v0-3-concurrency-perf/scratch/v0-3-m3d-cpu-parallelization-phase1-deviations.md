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
