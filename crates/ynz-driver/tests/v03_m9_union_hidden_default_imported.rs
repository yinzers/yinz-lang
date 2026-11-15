//! v0.3-M9 C1e rework (#421): a shape IMPORTED from another module keeps its hidden defaults.
//! The importing module used to look for the shape's declaration only in its own source, found
//! none and skipped every default: an `int` default read 0 and a union default was a null
//! envelope pointer (SIGSEGV on read). One initializer per shape, defined by the declaring
//! module and called by every construction site, writes them now. Tier: High (data integrity /
//! memory safety).
//!
//! Count: 11 tests = 2 fixtures x 4 runs + 1 structural check + 2 runs of the same-name fixture,
//! past the ~8 a High-tier unit gets without a written reason. The same-name fixture (two modules
//! each declaring a different shape `Thing`) is a bug of its own, the initializer symbol colliding
//! at link time, so it gets one plain and one ASan run rather than all four. `imported` is the plain cross-module construction;
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
    // The declaring module must DEFINE `ynz_defaults<shape:Holder>@<declaring file>`, the importer must only
    // DECLARE and call it, and no module may store a default inline. Tier: High.
    let built = support::build("v0_3_m9_union_hidden_default_imported", true);
    let ir = std::fs::read_to_string(&built.ir).expect("read emitted IR");
    // A section's first line is its file path. Match on that line alone: the importer's section
    // names `holders.ynz` too, inside the initializer's symbol.
    let sections = ir.split("; file: ").filter(|section| !section.is_empty());
    let section_of = |file: &str| {
        sections
            .clone()
            .find(|section| {
                section
                    .lines()
                    .next()
                    .is_some_and(|path| path.ends_with(file))
            })
            .unwrap_or_else(|| panic!("no module section for {file}"))
    };
    let entry_module = section_of("entrypoint.ynz");
    let holders_module = section_of("holders.ynz");
    // The symbol is `ynz_defaults<shape:Holder>@<declaring file>`: read it off the definition, then
    // require the importer to name that exact symbol.
    let definition_prefix = "define void @\"ynz_defaults<shape:Holder>@";
    let definition_start = holders_module.find(definition_prefix).unwrap_or_else(|| {
        panic!("the declaring module must define the initializer:\n{holders_module}")
    });
    let symbol_start = definition_start + "define void ".len();
    let symbol_length = holders_module[symbol_start + 2..]
        .find('"')
        .expect("the initializer symbol is quoted")
        + 3;
    let initializer = &holders_module[symbol_start..symbol_start + symbol_length];
    assert!(
        initializer.ends_with("holders.ynz\""),
        "the symbol must carry the declaring file: {initializer}"
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

// Two modules each declare a DIFFERENT shape named `Thing` with hidden defaults: the initializer
// symbol must tell them apart, both to link and so each construction runs its own shape's.
#[test]
fn v03_m9_union_hidden_default_same_name_shapes_optimized() {
    // Bug: both modules defined `ynz_defaults<shape:Thing>`, a duplicate-symbol link failure on a
    // program that built before the initializer existed. Tier: High.
    assert_prints(
        "v0_3_m9_union_hidden_default_same_name_shapes",
        false,
        "11a\n11a\n22b\n",
        "same-named shapes of two modules must each keep their own hidden defaults",
    );
}

#[test]
fn v03_m9_union_hidden_default_same_name_shapes_asan_no_optimize() {
    // Bug: the importer calling the other module's same-named initializer would write that
    // shape's layout into this one; ASan at -O0 is the memory verdict. Tier: High.
    assert_asan_clean(
        "v0_3_m9_union_hidden_default_same_name_shapes",
        true,
        "11a\n11a\n22b\n",
    );
}
