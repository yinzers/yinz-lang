//! v0.3-M9 C4: per-binding slot identity. Two LIVE bindings that share a NAME must never share
//! a state-machine frame slot / crossing alloca (a shared slot returns the wrong value, or reads a
//! value at the wrong width, with exit 0 and no diagnostic).
//!
//! Spike S2 (probes p02..p29, q02..q06, m01..m03): every program here is ACCEPTED by typeck (the
//! `ShadowsCrossingLocal` guard only examines a top-level outer `let` or a param) and then
//! mis-compiled, because crossing slots, crossing allocas and `cg.locals` were keyed by NAME and
//! a slot's width came from the FIRST `let` of that name. The `p01` / `p11` rows pin that the
//! fix does not disturb the sibling cases that were already right.
//!
//! Each case runs at the default optimization level and under `--no-optimize` at once, in its own
//! process under a liveness watchdog. Each row is a distinct binding shape (param / local / `for`
//! var / destructure alias / `errors` value / background argument; crossing vs non-crossing; int /
//! number / shape / string widths) that reached the mis-compile through a different keyed-by-name
//! site (one S2 probe each). With the fix reverted the 15 bug rows are red at both levels; the
//! second level stays as the guard against a fix that holds at one level only.

mod support;

use support::{assert_fixture_prints, at_both_optimization_levels};

/// The check every case here shares, at -O2 and at -O0 at once: each binding keeps its own slot.
fn assert_each_binding_keeps_its_slot(fixture: &str, expected_stdout: &str) {
    at_both_optimization_levels(|no_optimize| {
        assert_fixture_prints(
            fixture,
            no_optimize,
            expected_stdout,
            "each binding must keep its own slot",
        );
    });
}

/// One named test per binding shape of the `per-binding slot identity (feature run, integrity)` run
/// unit.
macro_rules! slot_identity_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: two live bindings that share a name share a state-machine frame slot or crossing alloca, so a read returns the other binding's value or reads at the wrong width, silently at exit 0 | tier: High
        #[test]
        fn $case_name() {
            assert_each_binding_keeps_its_slot($fixture, $expected_stdout);
        }
    )+ };
}

slot_identity_high_case! {
    // case: guard: disjoint sibling `let x` must stay correct once names are made unique
    p01_sibling_scopes, "v0_3_m9_slot_p01_sibling_scopes.ynz", "first\nsecond\n";
    // case: nested `let x` (outer not top-level), both crossing: one slot printed `inner, inner`
    p02_nested_let_both_cross, "v0_3_m9_slot_p02_nested_let_both_cross.ynz", "inner\nouter\n";
    // case: `for (n in ..)` over a param `n`: the param read after the loop returned the loop var
    p08_param_vs_for_var, "v0_3_m9_slot_p08_param_vs_for_var.ynz", "amy\nbob\nparam\n";
    // case: `for (n in ..)` over a crossing local `n`: the local read after the loop returned `bob`
    p09_local_vs_for_var, "v0_3_m9_slot_p09_local_vs_for_var.ynz", "amy\nbob\nouter\n";
    // case: guard: sequential loops with one var name must stay correct once names are made unique
    p11_sequential_for_same_var, "v0_3_m9_slot_p11_sequential_for_same_var.ynz", "a1\na2\nb1\nb2\n";
    // case: nested `for (v ..)` with the same var: outer element lost to the inner
    p12_nested_for_same_var, "v0_3_m9_slot_p12_nested_for_same_var.ynz", "b1\nb2\na1\nb1\nb2\na2\n";
    // case: non-crossing outer `x` read after a crossing inner `x` scope ends: read the inner value
    p17_outer_read_after_inner_scope, "v0_3_m9_slot_p17_outer_read_after_inner_scope.ynz", "outer\ninner\nouter\n";
    // case: NON-crossing inner `s` reused the crossing outer `s` alloca: printed `inner, inner`
    p23_noncrossing_shadow_of_crossing, "v0_3_m9_slot_p23_noncrossing_shadow_of_crossing.ynz", "inner\nouter\n";
    // case: sibling `x: Int` then `x: Number`: slot width came from the first `let`, `1.5` printed as 0.000..
    p26_sibling_int_then_number, "v0_3_m9_slot_p26_sibling_int_then_number.ynz", "7\n1.5\n";
    // case: sibling `x: shape` then `x: Int`: the int was read through the shape's slot layout, garbage
    p28_sibling_shape_then_int, "v0_3_m9_slot_p28_sibling_shape_then_int.ynz", "1\n99\n";
    // case: sibling `x: Int` then `x: shape`: a shape was stored in an int-width slot, garbage
    p29_sibling_int_then_shape, "v0_3_m9_slot_p29_sibling_int_then_shape.ynz", "99\n2\n";
    // case: nested `for ({ a as v } ..) { for ({ b as v } ..)`: the desugared alias lets share a slot
    q02_nested_destructure_alias, "v0_3_m9_slot_q02_nested_destructure_alias.ynz", "d1\nd2\na1\nd1\nd2\na2\n";
    // case: `errors` values `r` shadowed (outer failed, inner ok): the outer read the inner's success
    q05_errors_value_shadow, "v0_3_m9_slot_q05_errors_value_shadow.ynz", "99.9\n0.0\ntrue\n";
    // case: `background work(tag)` with a shadowed crossing `tag`: the outer task/`print(tag)` saw `in`
    q06_background_arg_shadow, "v0_3_m9_slot_q06_background_arg_shadow.ynz", "out\nin\nin\nout\n";
    // case: NON-crossing `for (n ..)` shadowing a crossing param `n`: shared the param's alloca/slot
    m01_param_vs_noncrossing_for_var, "v0_3_m9_slot_m01_param_vs_noncrossing_for_var.ynz", "amy\nbob\nparam\n";
    // case: NON-crossing `for (n ..)` shadowing a crossing local `n`: shared the local's alloca/slot
    m02_local_vs_noncrossing_for_var, "v0_3_m9_slot_m02_local_vs_noncrossing_for_var.ynz", "amy\nbob\nouter\n";
    // case: crossing param `n: Int` vs crossing `for (n in strings)`: slot width/type came from one binding
    m03_param_int_vs_for_var_string, "v0_3_m9_slot_m03_param_int_vs_for_var_string.ynz", "amy\nbob\n41\n";
}
