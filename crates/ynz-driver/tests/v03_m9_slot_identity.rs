//! v0.3-M9 C4: per-binding slot identity. Two LIVE bindings that share a NAME must never share
//! a state-machine frame slot / crossing alloca. Tier: High (data integrity — a shared slot
//! returns the wrong value, or reads a value at the wrong width, with exit 0 and no diagnostic).
//!
//! Spike S2 (probes p02..p29, q02..q06): every program here is ACCEPTED by typeck (the
//! `ShadowsCrossingLocal` guard only examines a top-level outer `let` or a param) and then
//! mis-compiled, because crossing slots, crossing allocas and `cg.locals` were keyed by NAME and
//! a slot's width came from the FIRST `let` of that name. The `p01` / `p11` rows pin that the
//! fix does not disturb the sibling cases that were already right.
//!
//! Each case runs at the default optimization level and under `YNZ_NO_OPTIMIZE=1` (what
//! `ynz build --no-optimize` sets). One generated module per case, one named test per
//! optimization level, so the runner parallelizes them and names the failing case. Each
//! `ynz run` is its own process (`.claude/rules/test-parallelism.md`).

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Run `ynz run <fixture>`; return (stdout, stderr, exit code; -1 when signalled).
fn run_fixture(name: &str, no_optimize: bool) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ynz"));
    cmd.args(["run", fixture(name).to_str().unwrap()])
        .env("CLICOLOR", "0");
    if no_optimize {
        cmd.env("YNZ_NO_OPTIMIZE", "1");
    }
    let output = cmd.output().expect("failed to spawn ynz binary");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

fn assert_fixture_prints(name: &str, no_optimize: bool, expected_stdout: &str) {
    let (stdout, stderr, code) = run_fixture(name, no_optimize);
    assert_eq!(
        code, 0,
        "{name} (no_optimize={no_optimize}) must run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, expected_stdout,
        "{name} (no_optimize={no_optimize}): each binding must keep its own slot; stderr:\n{stderr}"
    );
}

/// `slot_case!(case_name, fixture, expected_stdout, "bug it catches (Tier: High)")`; the bug
/// line becomes the module doc.
macro_rules! slot_case {
    ($case:ident, $fixture:literal, $expected:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use super::assert_fixture_prints;

            #[test]
            fn optimized() {
                assert_fixture_prints($fixture, false, $expected);
            }

            #[test]
            fn no_optimize() {
                // WHY: same defect at -O0, where no optimizer can mask the shared slot.
                assert_fixture_prints($fixture, true, $expected);
            }
        }
    };
}

slot_case!(
    p01_sibling_scopes,
    "v0_3_m9_slot_p01_sibling_scopes.ynz",
    "first\nsecond\n",
    "guard: disjoint sibling `let x` must stay correct once names are made unique (High)"
);
slot_case!(
    p02_nested_let_both_cross,
    "v0_3_m9_slot_p02_nested_let_both_cross.ynz",
    "inner\nouter\n",
    "nested `let x` (outer not top-level), both crossing: one slot printed `inner, inner` (High)"
);
slot_case!(
    p08_param_vs_for_var,
    "v0_3_m9_slot_p08_param_vs_for_var.ynz",
    "amy\nbob\nparam\n",
    "`for (n in ..)` over a param `n`: the param read after the loop returned the loop var (High)"
);
slot_case!(
    p09_local_vs_for_var,
    "v0_3_m9_slot_p09_local_vs_for_var.ynz",
    "amy\nbob\nouter\n",
    "`for (n in ..)` over a crossing local `n`: the local read after the loop returned `bob` (High)"
);
slot_case!(
    p11_sequential_for_same_var,
    "v0_3_m9_slot_p11_sequential_for_same_var.ynz",
    "a1\na2\nb1\nb2\n",
    "guard: sequential loops with one var name must stay correct once names are made unique (High)"
);
slot_case!(
    p12_nested_for_same_var,
    "v0_3_m9_slot_p12_nested_for_same_var.ynz",
    "b1\nb2\na1\nb1\nb2\na2\n",
    "nested `for (v ..)` with the same var: outer element lost to the inner (High)"
);
slot_case!(
    p17_outer_read_after_inner_scope,
    "v0_3_m9_slot_p17_outer_read_after_inner_scope.ynz",
    "outer\ninner\nouter\n",
    "non-crossing outer `x` read after a crossing inner `x` scope ends: read the inner value (High)"
);
slot_case!(
    p23_noncrossing_shadow_of_crossing,
    "v0_3_m9_slot_p23_noncrossing_shadow_of_crossing.ynz",
    "inner\nouter\n",
    "NON-crossing inner `s` reused the crossing outer `s` alloca: printed `inner, inner` (High)"
);
slot_case!(
    p26_sibling_int_then_number,
    "v0_3_m9_slot_p26_sibling_int_then_number.ynz",
    "7\n1.5\n",
    "sibling `x: int` then `x: number`: slot width came from the first `let`, `1.5` printed as 0.000.. (High)"
);
slot_case!(
    p28_sibling_shape_then_int,
    "v0_3_m9_slot_p28_sibling_shape_then_int.ynz",
    "1\n99\n",
    "sibling `x: shape` then `x: int`: the int was read through the shape's slot layout, garbage (High)"
);
slot_case!(
    p29_sibling_int_then_shape,
    "v0_3_m9_slot_p29_sibling_int_then_shape.ynz",
    "99\n2\n",
    "sibling `x: int` then `x: shape`: a shape was stored in an int-width slot, garbage (High)"
);
slot_case!(
    q02_nested_destructure_alias,
    "v0_3_m9_slot_q02_nested_destructure_alias.ynz",
    "d1\nd2\na1\nd1\nd2\na2\n",
    "nested `for ({ a as v } ..) { for ({ b as v } ..)`: the desugared alias lets share a slot (High)"
);
slot_case!(
    q05_errors_value_shadow,
    "v0_3_m9_slot_q05_errors_value_shadow.ynz",
    "99.9\n0.0\ntrue\n",
    "`errors` values `r` shadowed (outer failed, inner ok): the outer read the inner's success (High)"
);
slot_case!(
    q06_background_arg_shadow,
    "v0_3_m9_slot_q06_background_arg_shadow.ynz",
    "out\nin\nin\nout\n",
    "`background work(tag)` with a shadowed crossing `tag`: the outer task/`print(tag)` saw `in` (High)"
);
