//! v0.3-M9 C1e rework (#421): a shape IMPORTED from another module keeps its hidden defaults.
//! The importing module used to look for the shape's declaration only in its own source, found
//! none and skipped every default: an `int` default read 0 and a union default was a null
//! envelope pointer (SIGSEGV on read). One initializer per shape, defined by the declaring
//! module and called by every construction site, writes them now. Tier: High (data integrity /
//! memory safety).
//!
//! Count: 9 tests = 2 fixtures x 4 runs + 1 structural check, past the ~8 a High-tier unit gets
//! without a written reason. `imported` is the plain cross-module construction;
//! `inherited_imported` adds a parent's defaults applied through the child's initializer, a path of
//! its own (`extends` cannot cross files, so it cannot be folded into the first). The 4 runs per
//! fixture are plain vs ASan (a null envelope read can print plausible output) and -O0 vs -O2 (the
//! optimizer can mask a misread); all 8 go red without the one-initializer fix.
//!
//! Each fixture is a multi-module project (a directory) run at two optimization levels,
//! plainly (the VALUE) and under AddressSanitizer (the MEMORY ACCESS, every module
//! instrumented), kept apart so a failure says which is wrong.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

const WHAT: &str = "an imported shape must keep its hidden defaults (a plain `int` and a plain \
                    value inside a union field's envelope)";

macro_rules! imported_default_tests {
    ($fixture:expr, $stdout:expr, $optimized:ident, $no_optimize:ident, $asan_optimized:ident, $asan_no_optimize:ident) => {
        #[test]
        fn $optimized() {
            // Bug: imported shape's hidden defaults skipped (int read 0, union a null envelope).
            // Tier: High.
            assert_prints($fixture, false, $stdout, WHAT);
        }

        #[test]
        fn $no_optimize() {
            // Bug: same skipped defaults at -O0, where no optimizer can mask the misread.
            // Tier: High.
            assert_prints($fixture, true, $stdout, WHAT);
        }

        #[test]
        fn $asan_optimized() {
            // Bug: a null union envelope read is a memory fault; plausible output can hide a
            // stray read, so the sanitizer verdict stands alone. Tier: High.
            assert_asan_clean($fixture, false, $stdout);
        }

        #[test]
        fn $asan_no_optimize() {
            // Bug: same null-envelope read at -O0 under the sanitizer. Tier: High.
            assert_asan_clean($fixture, true, $stdout);
        }
    };
}

imported_default_tests!(
    "v0_3_m9_union_hidden_default_imported",
    "5\nother\nend\n",
    v03_m9_union_hidden_default_imported_optimized,
    v03_m9_union_hidden_default_imported_no_optimize,
    v03_m9_union_hidden_default_imported_asan_optimized,
    v03_m9_union_hidden_default_imported_asan_no_optimize
);

// The child's initializer applies its parent's defaults too (union one included). `extends`
// cannot cross files today, so parent and child share the declaring module.
imported_default_tests!(
    "v0_3_m9_union_hidden_default_inherited_imported",
    "5\n7\nother\nend\n",
    v03_m9_union_hidden_default_inherited_imported_optimized,
    v03_m9_union_hidden_default_inherited_imported_no_optimize,
    v03_m9_union_hidden_default_inherited_imported_asan_optimized,
    v03_m9_union_hidden_default_inherited_imported_asan_no_optimize
);

#[test]
fn v03_m9_union_hidden_default_imported_is_written_by_the_one_initializer() {
    // Bug: defaults lowered inline in each constructing module (and skipped when imported).
    // The declaring module must DEFINE `ynz_defaults<shape:Holder>`, the importer must only
    // DECLARE and call it, and no module may store a default inline. Tier: High.
    let built = support::build("v0_3_m9_union_hidden_default_imported", true);
    let ir = std::fs::read_to_string(&built.ir).expect("read emitted IR");
    let mut sections = ir.split("; file: ").filter(|section| !section.is_empty());
    let entry_module = sections
        .clone()
        .find(|section| section.contains("entrypoint.ynz"))
        .expect("entry module section");
    let holders_module = sections
        .find(|section| section.contains("holders.ynz"))
        .expect("holders module section");
    let initializer = "@\"ynz_defaults<shape:Holder>\"";
    assert!(
        holders_module.contains(&format!("define void {initializer}")),
        "the declaring module must define the initializer:\n{holders_module}"
    );
    assert!(
        entry_module.contains(&format!("declare void {initializer}"))
            && entry_module.contains(&format!("call void {initializer}")),
        "the importing module must declare and call the initializer:\n{entry_module}"
    );
    assert!(
        !entry_module.contains(".count_default"),
        "the importing module must store no default inline:\n{entry_module}"
    );
}
