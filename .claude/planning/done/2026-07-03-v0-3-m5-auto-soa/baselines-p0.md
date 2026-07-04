# P0 baselines — pre-M5 reference numbers (E8 + E11)

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 5
- **Tree:** worktree `feat/v0-3-m5-auto-soa` @ fork commit `1ac52fd`, source pristine (only
  plan-dir files differ). All runs in the worktree Docker `dev` container.

## (a) E8 pre-migration alloc=free baseline — array fixture suite

**Mechanism:** compiled program run with `YNZ_ALLOC_COUNTER=1` +
`YNZ_ALLOC_COUNTER_OUTPUT=<file>`; the runtime writes `alloc=N` / `free=M` at shutdown
(`crates/ynz-runtime/src/runtime.rs:345-352`).

**⚠ SCOPE FACT (load-bearing for the Phase 3 E8 gate):** the counter instruments ONLY
`ynz_alloc` / `ynz_free` (the shape heap allocator — `crates/ynz-runtime/src/lib.rs:295-342`).
The mallocs inside `ynz_array_new` / `ynz_array_push` / `ynz_map_new` are **invisible** to it
today — that is why array-heavy fixtures below read `alloc=0`. **For the E8 parity gate to have
teeth on the NEW by-value ABI, Phase 2/3 must either route the new element-buffer allocations
through counted entry points or extend the counter to the array/map allocator** — otherwise
"alloc=free" vacuously passes while element buffers leak. Surfaced as a Phase 2/3 design
obligation.

**Selection rule:** every driver fixture whose filename contains "array"
(`crates/ynz-driver/tests/fixtures/`) + the P0 spike fixtures (the actual `array<Shape>`
migration surface). Rejection fixtures (compile errors by design) recorded as SKIP.
Command per fixture: `YNZ_ALLOC_COUNTER=1 YNZ_ALLOC_COUNTER_OUTPUT=/tmp/cnt.txt
./target/debug/ynz run <fixture>` (debug profile — counts are profile-independent).

| Fixture | alloc | free |
|---|---|---|
| m5_array.ynz | 0 | 0 |
| v0_3_m3a_p3_array_shape_literal_crossing_still_works.ynz | 1 | 1 |
| v0_3_m3a_p3_array_shape_runtime_field_rejected.ynz | SKIP (compile-reject, exit 1) | — |
| v0_3_m3a_p3_audit_array_crossing.ynz | 1 | 1 |
| v0_3_m3a_p3_audit_array_number_no_leak.ynz | 1 | 1 |
| v0_3_m3a_p3_for_array_wait.ynz | 1 | 1 |
| v0_3_m3a_p3_for_int_array_loop_var.ynz | 1 | 1 |
| v0_3_m3a_p3_for_string_array_wait.ynz | 1 | 1 |
| v0_3_m3a_r7_array_shape_between_waits_rejected.ynz | SKIP (compile-reject, exit 1) | — |
| v0_3_m3a_r7_array_shape_nested_if_rejected.ynz | SKIP (compile-reject, exit 1) | — |
| v0_3_m3b_p2_bg_array_real_copy.ynz | 0 | 0 |
| v0_3_m3d_danger_array_match_arm.ynz | 1 | 1 |
| v0_3_m3d_danger_same_callee_array_if_arm.ynz | 1 | 1 |
| v0_3_m3d_return_class_array.ynz | 1 | 1 |
| v0_3_m3d_same_callee_array.ynz | 1 | 1 |
| spike-notes/s1_byval_fixture.ynz | 0 | 0 |
| spike-notes/s2_fix1_qualifying_2field.ynz | 0 | 0 |
| spike-notes/s2_fix2_threefield_loop.ynz | 0 | 0 |
| spike-notes/s2_fix3_escaping.ynz | 0 | 0 |
| spike-notes/s2_fix4_runtime_length.ynz | 0 | 0 |

**Invariant this baseline pins:** alloc == free on every runnable fixture (12/12 runnable = parity;
two of them 0=0 because nothing routes through the counted allocator). Post-migration, parity must
hold AND (per the scope fact above) the gate must be made to actually see the new buffers.

## (b) E11 compile-time baseline — pirates-roster wall-clock

**Command:** `./target/release/ynz build examples/pirates-roster/entrypoint.ynz` (release-profile
compiler; built `cargo build -p ynz-runtime --release && cargo build -p ynz-driver --release`
per the embedded-archive build order). Timed via `date +%s%N` around the whole process,
7 back-to-back reps, worktree dev container.

**Note on the plan's phrasing:** ¶3.3 step 5 says "`ynz build --release`" — the ynz CLI has NO
`--release` flag yet (`crates/ynz-driver/src/main.rs:94-95` marks it future). The honest
equivalent measured here: release-profile COMPILER binary running `ynz build`. Recorded as minor
recon drift; Phase 8's comparison must use the identical setup.

| rep | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| ms | 222 | 206 | 209 | 208 | 205 | 210 | 212 |

**Baseline: mean ≈ 210 ms, min 205 ms, max 222 ms (spread 8.3%).** Phase 8's <10% cost gate
compares against the MEAN of ≥7 reps in this same environment; a delta below the ~8% spread is
noise, so the gate should re-measure BOTH old and new compilers on the same day (same host state)
rather than trust this absolute number across weeks.

**Output-nondeterminism caveat (known, surfaced by segment 1):** `ynz run` on pirates-roster
interleaves the background "done" lines nondeterministically vs `expected_stdout.txt`. This
baseline therefore times the BUILD only and never diffs run output; anyone extending it to run
output must first check how `integration.rs` normalizes ordering.
