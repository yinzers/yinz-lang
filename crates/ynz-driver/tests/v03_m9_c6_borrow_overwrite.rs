//! v0.3-M9 C6 security round 1: the borrow-overwrite class (a reader of a replaced or released
//! place) stays closed.
//!
//! The release helper frees what an overwrite replaces and what a task's argument owns. A binding,
//! loop variable, `share` argument or `return` that still points at the replaced or released
//! value then reads freed memory. Each fixture prints its expected output under ASan only while
//! typeck judges the reader a copy (`non_owned_reads`). All High tier: memory safety.

mod support;

use support::assert_asan_clean_and_prints;

macro_rules! borrow_case {
    ($case:ident, $fixture:literal, $expected:literal) => {
        mod $case {
            use crate::assert_asan_clean_and_prints;

            #[test]
            fn asan_optimized() {
                assert_asan_clean_and_prints($fixture, false, $expected);
            }

            #[test]
            fn asan_no_optimize() {
                assert_asan_clean_and_prints($fixture, true, $expected);
            }
        }
    };
}

borrow_case!(
    loop_elem_overwrite,
    "v0_3_m9_c6_borrow_loop_elem_overwrite.ynz",
    "a-1\nb-2\n"
);
borrow_case!(
    loop_lend_overwrite,
    "v0_3_m9_c6_borrow_loop_lend_overwrite.ynz",
    "a-1\nb-2\n"
);
borrow_case!(
    loop_map_overwrite,
    "v0_3_m9_c6_borrow_loop_map_overwrite.ynz",
    "v-1\n"
);
borrow_case!(
    loop_shape_overwrite,
    "v0_3_m9_c6_borrow_loop_shape_overwrite.ynz",
    "a-1\n"
);
borrow_case!(
    share_arg_overwrite,
    "v0_3_m9_c6_borrow_share_arg_overwrite.ynz",
    "a-1\n"
);
borrow_case!(
    let_field_then_assign,
    "v0_3_m9_c6_borrow_let_field_then_assign.ynz",
    "3\n10\n"
);
borrow_case!(
    let_or_then_set,
    "v0_3_m9_c6_borrow_let_or_then_set.ynz",
    "3\n10\n"
);
borrow_case!(
    assign_field_then_assign,
    "v0_3_m9_c6_borrow_assign_field_then_assign.ynz",
    "3\n"
);
borrow_case!(
    let_map_of_map,
    "v0_3_m9_c6_borrow_let_map_of_map.ynz",
    "v-1\n"
);
borrow_case!(
    return_field_sync,
    "v0_3_m9_c6_borrow_return_field_sync.ynz",
    "3\n10\n"
);
borrow_case!(
    return_param_echo,
    "v0_3_m9_c6_borrow_return_param_echo.ynz",
    "3\n"
);
borrow_case!(
    task_return_field,
    "v0_3_m9_c6_borrow_task_return_field.ynz",
    "3\n10\n"
);
borrow_case!(
    task_return_map_value,
    "v0_3_m9_c6_borrow_task_return_map_value.ynz",
    "3\n1\n"
);
borrow_case!(
    task_return_map_field,
    "v0_3_m9_c6_borrow_task_return_map_field.ynz",
    "v-1\n"
);

// Round 2 probes of the same class, each one a High-tier memory-safety case.
borrow_case!(
    loop_union_overwrite,
    "v0_3_m9_c6_borrow_loop_union_overwrite.ynz",
    "big-1\n"
);
borrow_case!(
    let_maybe_array_payload,
    "v0_3_m9_c6_borrow_let_maybe_array_payload.ynz",
    "3\n"
);
borrow_case!(
    expr_interpolation_order,
    "v0_3_m9_c6_borrow_expr_interpolation_order.ynz",
    "a-1-r\n"
);
borrow_case!(
    map_growth_overwrite,
    "v0_3_m9_c6_borrow_map_growth_overwrite.ynz",
    "v0\nw0\n41\n"
);
borrow_case!(
    soa_scatter_reader,
    "v0_3_m9_c6_borrow_soa_scatter_reader.ynz",
    "n-0\n2656\n"
);

// Cross-module glue: the same `array<Tag>` is overwritten by the module that declares `Tag` and by
// the one that imports it, each releasing with the glue its own module emitted.
borrow_case!(
    cross_module_glue,
    "v0_3_m9_c6_cross_module_glue",
    "lib-3\nmain-4\n"
);

// State-machine loops (a `wait` in the body) copy the same way.
borrow_case!(
    sm_loop_elem_overwrite,
    "v0_3_m9_c6_borrow_sm_loop_elem_overwrite.ynz",
    "a-1\nb-2\n"
);
borrow_case!(
    sm_loop_shape_overwrite,
    "v0_3_m9_c6_borrow_sm_loop_shape_overwrite.ynz",
    "a-1\n"
);
borrow_case!(
    sm_loop_map_overwrite,
    "v0_3_m9_c6_borrow_sm_loop_map_overwrite.ynz",
    "v-1\n"
);
