//! v0.3-M9 C6 security round 1: RED until the borrow-overwrite class is fixed.
//!
//! The release helper frees what an overwrite replaces and what a task's argument owns. A binding,
//! loop variable, `share` argument or `return` that still points at the replaced or released
//! value then reads freed memory. Base `cf5a42dd` prints each expected output under ASan;
//! head reads freed memory. All High tier: memory safety.

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
