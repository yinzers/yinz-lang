// v0.3-M7 planned-RED gate — the name-keyed loop-var frame-slot collision UNDERLYING
// Phase 6's D5 admission decline (plan `2026-07-04-v0-3-m7-optimizer-pipeline` Future
// Requirements entry "name-keyed loop-var frame-slot collision", ELEVATED priority).
//
// What D5 did and did NOT do: `for_var_elem_type_conflict` (crates/ynz-typeck/src/
// check.rs) DECLINES back-edge-yield admission when two `for` loops share a loop-var
// name across differently-typed elements — that stops Phase 6 from WIDENING the
// exposure, but the hazard itself PRE-EXISTS Phase 6 for loops whose bodies SUSPEND
// (they were already routed through the SM arms): the crossing frame slot is keyed by
// NAME and classified once from the FIRST loop's element type, so the second loop's
// differently-typed variable flushes/reloads through a mis-classified, mis-sized slot.
//
// Two live manifestations (both 2026-07-17):
//   - this gate's minimized fixture (Point loop then string loop, both suspending):
//     the string element reloads as raw garbage bytes — silent corruption, exit 0;
//   - m5_p5_soa_copy_wait_bg.ynz's original three-loop + background shape (pre-D5
//     widened routing): heap corruption, SIGABRT "corrupted size vs. prev_size".
//
// These tests are the no-duct-tape planned-RED inverse: they assert the CORRECT
// contract (each loop's variable reloads ITS OWN element values at both tiers). They were
// `#[ignore]`d planned-RED locks until v0.3-M9 C4 (per-binding slot identity: typeck gives
// every binding a unique codegen name, so two same-named loop variables no longer share a
// frame slot) fixed the hazard; the `#[ignore]` marks are gone and they run in the normal
// suite. Do NOT weaken an assertion or assert garbage to force green: a red here is silent
// frame corruption in the suspension machinery (the R8 / M3a-family class).

mod support;

use support::assert_fixture_prints;

const FIXTURE: &str = "v0_3_m7_d5_suspending_loop_var_slot_collision.ynz";

/// The correct contract: each loop's variable carries ITS OWN loop's element values
/// through every suspension — never another loop's slot classification.
const EXPECTED_STDOUT: &str = "111\nanchor\nharbor\ndone\n";

/// The fixture runs through the shared harness (`support`): own temp directory, own process,
/// liveness watchdog that kills a hang — the same build-and-run this file used to carry a copy of.
const WHAT_IT_GUARDS: &str = "a loop variable reloaded through ANOTHER loop's name-keyed frame \
     slot (or the collision escalated to a crash, m5_p5's SIGABRT class)";

#[test]
fn d5_red_suspending_loops_sharing_var_name_reload_their_own_values_default() {
    // WHY: locks the correct contract on the shipped default tier — the second loop's
    // string variable must reload `anchor`/`harbor`, not raw bytes through the first
    // loop's Point-classified slot.
    assert_fixture_prints(FIXTURE, false, EXPECTED_STDOUT, WHAT_IT_GUARDS);
}

#[test]
fn d5_red_suspending_loops_sharing_var_name_reload_their_own_values_o0() {
    // WHY: the --no-optimize escape hatch shares the identical name-keyed slot
    // machinery and must hold the same contract independently.
    assert_fixture_prints(FIXTURE, true, EXPECTED_STDOUT, WHAT_IT_GUARDS);
}
