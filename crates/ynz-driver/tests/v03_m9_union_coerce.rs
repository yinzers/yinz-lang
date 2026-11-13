//! v0.3-M9 C1e (#421): every plain value that reaches a UNION-typed slot is wrapped in the tagged
//! `{tag, data}` envelope at ONE coercion point. Tier: High (memory safety / data integrity: the
//! pre-fix lowering wrapped per call and return site only, so a union-typed shape field, a
//! non-shape variant and a `.or` fallback were stored or passed as the bare value — wrong
//! variant, an out-of-bounds read past the 1-byte `Tiny`, or a SIGSEGV on the raw int).
//!
//! RED-first on the base (f790a45): `field`, `let`, `non_shape` and `or` fail there; the
//! parameter and return slots of a SHAPE variant pass there (#410) and stay as the
//! `v03_m9_union_wrap` fixtures through the new point.
//!
//! One named test per case: each fixture runs at two optimization levels, plainly (the VALUE)
//! and under AddressSanitizer (the MEMORY ACCESS), kept apart so a failure says which is wrong.
//! Non-shape variants are matched with `else =>`; an `is int =>` arm does not lower (#418).

mod support;

use std::path::Path;
use std::process::Command;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "a plain value reaching a union slot must carry its own variant tag";

macro_rules! coercion_tests {
    ($fixture:expr, $stdout:expr, $optimized:ident, $no_optimize:ident, $asan_optimized:ident, $asan_no_optimize:ident) => {
        #[test]
        fn $optimized() {
            // WHY: the value read back through the slot must be the variant that was stored.
            assert_prints($fixture, false, $stdout, WHAT);
        }

        #[test]
        fn $no_optimize() {
            // WHY: same defect at -O0, where no optimizer can mask the misread.
            assert_prints($fixture, true, $stdout, WHAT);
        }

        #[test]
        fn $asan_optimized() {
            // WHY: a bare value read as a 16-byte envelope is out of bounds; plausible output
            // can hide it, so the sanitizer verdict stands alone.
            assert_asan_clean($fixture, false, $stdout);
        }

        #[test]
        fn $asan_no_optimize() {
            // WHY: same out-of-bounds envelope read at -O0.
            assert_asan_clean($fixture, true, $stdout);
        }
    };
}

// Bug: a plain shape stored in a union-typed shape FIELD (`holder: { inner: tiny }`) was stored as
// the bare shape pointer — wrong variant, ASan stack-buffer-overflow past `Tiny`. Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_field.ynz",
    "tiny\nbig hello\nbig hello\nbig world\nend\n",
    v03_m9_union_coerce_field_optimized,
    v03_m9_union_coerce_field_no_optimize,
    v03_m9_union_coerce_field_asan_optimized,
    v03_m9_union_coerce_field_asan_no_optimize
);

// Bug: `let n: Mixed = 5` kept the raw int as the envelope pointer — SIGSEGV on the first tag
// read; shape lets (call result, loop body) must stay right. Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_let.ynz",
    "circle\nother\ncircle\ncircle\nother\ncircle\nother\nend\n",
    v03_m9_union_coerce_let_optimized,
    v03_m9_union_coerce_let_no_optimize,
    v03_m9_union_coerce_let_asan_optimized,
    v03_m9_union_coerce_let_asan_no_optimize
);

// Bug: a NON-shape variant (`int`) passed, returned or stored in a field of a union with a shape
// variant was never wrapped (LLVM verifier failure / SIGSEGV). Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_non_shape.ynz",
    "other\nother\nother\ncircle\nend\n",
    v03_m9_union_coerce_non_shape_optimized,
    v03_m9_union_coerce_non_shape_no_optimize,
    v03_m9_union_coerce_non_shape_asan_optimized,
    v03_m9_union_coerce_non_shape_asan_no_optimize
);

// Bug: `.or(fallback)` with a plain shape fallback into a `maybe<union>` result was not wrapped
// (printed `none`, ASan overflow). Tier: High.
coercion_tests!(
    "v0_3_m9_union_coerce_or.ynz",
    "big hello\ntiny\nend\n",
    v03_m9_union_coerce_or_optimized,
    v03_m9_union_coerce_or_no_optimize,
    v03_m9_union_coerce_or_asan_optimized,
    v03_m9_union_coerce_or_asan_no_optimize
);

#[test]
fn v03_m9_union_coerce_channel_union_element_is_refused() {
    // Regression guard (no hole found): a channel never carries a union, so no plain value can
    // reach a union-typed channel slot. If channels start accepting union elements, a send must
    // wrap through the one coercion point and this test becomes a run fixture. Tier: High.
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("program.ynz");
    std::fs::copy(
        support::fixture_path("v0_3_m9_union_coerce_channel_refused.ynz"),
        &source,
    )
    .expect("copy fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(Path::new(&source))
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "a union-element channel compiled:\n{diagnostics}"
    );
    assert!(
        diagnostics.contains("`channel<Tiny | Big>` is not supported yet"),
        "refusal must name the union channel; got:\n{diagnostics}"
    );
}
