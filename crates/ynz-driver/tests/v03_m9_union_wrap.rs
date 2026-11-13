//! v0.3-M9 C1c: a PLAIN shape passed to (or returned as) a UNION must reach the callee (or the
//! caller) as a correctly tagged envelope. Tier: High (data integrity / memory safety — the
//! pre-fix lowering handed over the bare shape pointer: the `is` arm picked the wrong variant
//! and a variant smaller than the 16-byte envelope was read out of bounds).
//!
//! RED-first: both fixtures fail on the pre-fix tree.
//!   - `arg`: a plain call, a UFCS extra argument, a generic callee and a `wait`ed callee each
//!     got the bare shape pointer; output was `none` / the wrong variant and ASan reported a
//!     stack-buffer-overflow READ past the 1-byte `Tiny`.
//!   - `return`: `-> Thing` returned the bare shape pointer, and a union local the callee
//!     built itself came back as a pointer into the callee's dead stack frame (ASan:
//!     stack-use-after-return).
//!
//! 8 tests, the High-tier budget: two lowering families (argument slot, return slot), each at
//! two optimization levels, each run both plainly (the VALUE) and under AddressSanitizer (the
//! MEMORY ACCESS) — kept apart so a failure says which one is wrong.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a plain shape reaching a union slot must carry its own variant tag";

const ARG: &str = "v0_3_m9_union_wrap_arg.ynz";
const ARG_STDOUT: &str =
    "big hello\ntiny\ntrue / big hello\ntiny\nbig hello\ntiny\nbig hello\nend\n";

#[test]
fn v03_m9_union_wrap_arg_optimized() {
    // WHY: `describe(big)` / `describe(tiny)` picked the wrong variant (the shape's bytes were
    // read as `{tag, data}`).
    assert_prints(ARG, false, ARG_STDOUT, WHAT);
}

#[test]
fn v03_m9_union_wrap_arg_no_optimize() {
    // WHY: same defect at -O0, where no optimizer can mask the misread.
    assert_prints(ARG, true, ARG_STDOUT, WHAT);
}

#[test]
fn v03_m9_union_wrap_arg_asan_optimized() {
    // WHY: the callee read a 16-byte envelope out of the 1-byte `Tiny` (stack-buffer-overflow
    // READ) — plausible-looking output can hide it, so the sanitizer verdict stands alone.
    assert_asan_clean(ARG, false, ARG_STDOUT);
}

#[test]
fn v03_m9_union_wrap_arg_asan_no_optimize() {
    // WHY: same out-of-bounds envelope read at -O0.
    assert_asan_clean(ARG, true, ARG_STDOUT);
}

const RETURN: &str = "v0_3_m9_union_wrap_return.ynz";
const RETURN_STDOUT: &str = "big hello\ntiny\nbig local\ntiny\nbig wrapped\ntiny\nend\n";

#[test]
fn v03_m9_union_wrap_return_optimized() {
    // WHY: `-> Thing` returned the bare shape pointer, so the caller's `is` arm picked the
    // wrong variant; a callee-built envelope came back pointing into the dead callee frame.
    assert_prints(RETURN, false, RETURN_STDOUT, WHAT);
}

#[test]
fn v03_m9_union_wrap_return_no_optimize() {
    // WHY: same defect at -O0.
    assert_prints(RETURN, true, RETURN_STDOUT, WHAT);
}

#[test]
fn v03_m9_union_wrap_return_asan_optimized() {
    // WHY: stack-use-after-return — the returned envelope lived in the callee's frame.
    assert_asan_clean(RETURN, false, RETURN_STDOUT);
}

#[test]
fn v03_m9_union_wrap_return_asan_no_optimize() {
    // WHY: same use-after-return at -O0.
    assert_asan_clean(RETURN, true, RETURN_STDOUT);
}
