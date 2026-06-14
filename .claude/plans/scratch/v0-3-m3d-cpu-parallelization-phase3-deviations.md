# v0-3-m3d-cpu-parallelization Phase 3 Deviations — captured 2026-06-14 (Slice 3)

D_count: 0

> NOTE: Phase 3 is built INCREMENTALLY across live-verified slices (Patrick decision, 2026-06-13),
> one commit at the phase boundary. This scratch file reflects the CURRENT slice's deviation set.
> Slice 3 = same-callee distinctness across the return-class matrix + worth-it/trivial-leaf-not-spawned.
> Slice 1 (production trigger + FrameLayout CPU-slots, committed 0cc0ae0) and Slice 2 (distinct-callee
> return-class matrix, committed d8bef4e) deviation records + their deviation-judge PASS verdicts live
> durably in the plan-file Phase-3 Findings Log AND in the committed scratch snapshots in git history.
> Cumulative 4.a re-judges the full Phase-3 deviation set from those durable records.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (Touched only `crates/ynz-driver/tests/fixtures/` and `crates/ynz-driver/tests/integration.rs`. The plan file's one-line modification visible in `git status` is the pre-existing slice-2 checkpoint note that was already uncommitted in the working tree at session start — executor did not author or modify it, and does not tick Phase-3 ACs/gates.)

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches. The prompt's Concern A/B both anticipated a possible codegen fix ("if a class binds wrong … FIX it"); verify-first proved no class binds wrong and the worth-it gate is already wired, so the correct, plan-sanctioned outcome was test-only fixtures + CI gates with no source change. Each FIRE fixture asserts the mechanism FIRED (2 spawns via IR-grep) and each DECLINE asserts 0 spawns, per the gated-path-fire-assertions requirement.

## Resolved spawn list (orchestrator's parsed view)

No deviations — no judges spawned this slice.
