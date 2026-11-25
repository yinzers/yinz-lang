//! v0.3-M9 C6: the borrow-overwrite class (a reader of a replaced or released
//! place) stays closed.
//!
//! The release helper frees what an overwrite replaces and what a task's argument owns. A binding,
//! loop variable, `share` argument or `return` that still points at the replaced or released
//! value then reads freed memory. Each fixture prints its expected output under ASan only while
//! typeck judges the reader a copy (`non_owned_reads`).

mod support;

use support::{assert_asan_clean_and_prints, at_both_optimization_levels};

/// One named test per reader position of the `borrow overwrite` run unit; each case runs at both
/// optimization levels under AddressSanitizer.
macro_rules! borrow_overwrite_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal;)+) => { $(
        // bug: a binding, loop variable, share argument or return still reads a place the release helper freed on overwrite, silently at exit 0 | tier: High
        #[test]
        fn $case_name() {
            at_both_optimization_levels(|no_optimize| {
                assert_asan_clean_and_prints($fixture, no_optimize, $expected_stdout);
            });
        }
    )+ };
}

borrow_overwrite_high_case! {
    loop_elem_overwrite, "v0_3_m9_c6_borrow_loop_elem_overwrite.ynz", "a-1\nb-2\n";
    loop_lend_overwrite, "v0_3_m9_c6_borrow_loop_lend_overwrite.ynz", "a-1\nb-2\n";
    loop_map_overwrite, "v0_3_m9_c6_borrow_loop_map_overwrite.ynz", "v-1\n";
    loop_shape_overwrite, "v0_3_m9_c6_borrow_loop_shape_overwrite.ynz", "a-1\n";
    share_arg_overwrite, "v0_3_m9_c6_borrow_share_arg_overwrite.ynz", "a-1\n";
    ufcs_receiver_field_overwrite, "v0_3_m9_c6_cost_ufcs_receiver_write.ynz", "3\n";
    let_field_then_assign, "v0_3_m9_c6_borrow_let_field_then_assign.ynz", "3\n10\n";
    let_or_then_set, "v0_3_m9_c6_borrow_let_or_then_set.ynz", "3\n10\n";
    assign_field_then_assign, "v0_3_m9_c6_borrow_assign_field_then_assign.ynz", "3\n";
    let_map_of_map, "v0_3_m9_c6_borrow_let_map_of_map.ynz", "v-1\n";
    return_field_sync, "v0_3_m9_c6_borrow_return_field_sync.ynz", "3\n10\n";
    return_param_echo, "v0_3_m9_c6_borrow_return_param_echo.ynz", "3\n";
    task_return_field, "v0_3_m9_c6_borrow_task_return_field.ynz", "3\n10\n";
    task_return_map_value, "v0_3_m9_c6_borrow_task_return_map_value.ynz", "3\n1\n";
    task_return_map_field, "v0_3_m9_c6_borrow_task_return_map_field.ynz", "v-1\n";
    loop_union_overwrite, "v0_3_m9_c6_borrow_loop_union_overwrite.ynz", "big-1\n";
    let_maybe_array_payload, "v0_3_m9_c6_borrow_let_maybe_array_payload.ynz", "3\n";
    expr_interpolation_order, "v0_3_m9_c6_borrow_expr_interpolation_order.ynz", "a-1-r\n";
    map_growth_overwrite, "v0_3_m9_c6_borrow_map_growth_overwrite.ynz", "v0\nw0\n41\n";
    soa_scatter_reader, "v0_3_m9_c6_borrow_soa_scatter_reader.ynz", "n-0\n2656\n";
    cross_module_glue, "v0_3_m9_c6_cross_module_glue", "lib-3\nmain-4\n";
    sm_loop_elem_overwrite, "v0_3_m9_c6_borrow_sm_loop_elem_overwrite.ynz", "a-1\nb-2\n";
    sm_loop_shape_overwrite, "v0_3_m9_c6_borrow_sm_loop_shape_overwrite.ynz", "a-1\n";
    sm_loop_map_overwrite, "v0_3_m9_c6_borrow_sm_loop_map_overwrite.ynz", "v-1\n";
}
