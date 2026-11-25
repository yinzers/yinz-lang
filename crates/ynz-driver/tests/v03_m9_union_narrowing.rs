//! v0.3-M9 C1: `is Variant =>` union narrowing reads the variant PAYLOAD, not the
//! `{i64 tag, i64 data}` envelope (data integrity: a wrong read is silent garbage or an
//! out-of-bounds read).
//!
//! RED-first: all four fixtures fail on the pre-fix tree. Measured: with
//! `narrow_union_local_to_variant` disabled, every case below is red; with only the post-resume
//! re-derivation (`renarrow_union_locals`) reverted, only `sm_host` is red.
//!   - `field_read`: the arm typed `fig` as the variant but codegen left the binding on the
//!     16-byte envelope, so `fig.radius` returned the tag word (a zero value, not `5.0`).
//!   - `payload_copy`: `let inner: Circle = fig` memcpy'd `sizeof(Circle)` bytes out of the
//!     16-byte envelope (AddressSanitizer: stack-buffer-overflow READ in memcpy) and the
//!     string field then dereferenced garbage (SIGSEGV in the optimized build).
//!   - `sm_host`: in a suspending function (`lower_sm_match`, a second `is`-arm lowering site)
//!     the resume reloaded the envelope into the narrowed payload slot, so a read after a
//!     `wait` inside the arm saw the envelope again.
//!   - `union_slot`: the narrowed binding passed or returned where the UNION is expected (union
//!     parameter of a plain, `wait`ed, UFCS or generic call; union return) is a separate
//!     lowering decision (envelope vs payload per use); the pre-fix output was wrong only some of
//!     the time, the read past the 1-byte variant every time, so the sanitizer verdict is the
//!     check.
//!
//! The `payload_copy` and `sm_host` cases run the program as shipped (the VALUE) and under
//! AddressSanitizer (the MEMORY ACCESS, which needs `clang` with the ASan runtime, as the C2
//! safety fixtures do) at -O0 and -O2: the output assertions alone would not name an
//! out-of-bounds read that happens to return plausible bytes.

mod support;

use support::{
    assert_asan_clean_and_prints_at_both_levels, assert_fixture_prints,
    assert_prints_and_asan_clean_at_both_levels, at_both_optimization_levels,
};

const NARROWED_ARM_READS_THE_PAYLOAD: &str = "the narrowed arm must read the variant payload";

/// One named test per lowering path of the `union narrowing reads the variant payload (feature
/// run, integrity)` unit.
macro_rules! union_narrowing_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: an `is Variant =>` arm reads the 16-byte union envelope instead of the variant payload, so a field read returns the tag word, a copy reads past the envelope (ASan stack-buffer-overflow, SIGSEGV on a string field) or a read after a `wait` sees the envelope again | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

union_narrowing_high_case! {
    // case: `fig.radius` in `is Circle =>` returned the envelope tag word instead of 5.0 (plain at both levels: -O0 has no optimizer to mask the envelope read)
    a_narrowed_field_read_returns_the_payload_field => {
        at_both_optimization_levels(|no_optimize| {
            assert_fixture_prints(
                "v0_3_m9_union_narrow_field_read.ynz",
                no_optimize,
                "5.0\n3.0\n",
                NARROWED_ARM_READS_THE_PAYLOAD,
            );
        });
    }
    // case: `let inner: Circle = fig` read sizeof(Circle) bytes out of a 16-byte envelope (OOB read; SIGSEGV on the string field in the optimized build)
    a_narrowed_binding_copied_out_of_the_arm_is_the_payload => {
        assert_prints_and_asan_clean_at_both_levels(
            "v0_3_m9_union_narrow_payload_copy.ynz",
            "round\n5.0\n78.5\nt\n",
            NARROWED_ARM_READS_THE_PAYLOAD,
        );
    }
    // case: `lower_sm_match` is a second, separate `is`-arm lowering site; fixing only the plain-match site would leave narrowing in any suspending function reading the envelope. The post-`wait` reads catch a resume that reloads the envelope into the payload slot (`let after: Circle = fig` then copied sizeof(Circle) bytes out of the envelope)
    a_narrowed_binding_in_a_suspending_function_survives_the_resume => {
        assert_prints_and_asan_clean_at_both_levels(
            "v0_3_m9_union_narrow_sm_host.ynz",
            "round\n5.0\nround\nround\n5.0\nround\nresumed\n5.0\nround\nround\nend\n",
            NARROWED_ARM_READS_THE_PAYLOAD,
        );
    }
    // case: the callee read a 16-byte envelope out of the 1-byte `Tiny` payload (buffer overflow READ, wrong variant `big`)
    a_narrowed_binding_passed_where_the_union_is_expected_is_the_envelope => {
        assert_asan_clean_and_prints_at_both_levels(
            "v0_3_m9_union_narrow_union_slot.ynz",
            "tiny\ntrue\ntiny\ntiny\ntiny\ntiny\ntrue\nend\n",
        );
    }
}
