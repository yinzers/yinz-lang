//! v0.3-M9 C7 slice 4: the release of a nested-shape cell checks for null only where a `Take` can
//! null it, and a union of heap variants carries a tag so a scope exit can release the variant it
//! holds (`IMP-ownership.md` "Scope-exit release on the plain path", `IMP-unions.md`).
//!
//! Counter cases run at both optimization levels; the counters say every allocation was freed and
//! AddressSanitizer with `free_fill_byte=0` says nothing was freed early or twice. All are High
//! tier: a wrong release is a leak, a double free or a use-after-free, silent at exit 0.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    build_source, run_balanced,
};

// ── Item 0: the nested-shape null check ─────────────────────────────────────────────────────

const NESTED_SHAPE_NEVER_TAKEN: &str = "
shape Tag {
  name: string
  id: int
}
shape Wrap {
  inner: Tag
  count: int
}
function entrypoint() -> nothing {
  let tag: Tag = { name: `t`, id: 4 }
  let wrap: Wrap = { inner: tag, count: 3 }
  print(wrap.inner.id + wrap.count)
}
";

const NESTED_SHAPE_TAKEN: &str = "
shape Tag {
  name: string
  id: int
}
shape Wrap {
  inner: Tag
  count: int
}
function make(index: int) -> Wrap {
  let tag: Tag = { name: `t${index}`, id: index }
  return { inner: tag, count: 3 }
}
function entrypoint() -> nothing {
  let taken: Tag = make(5).inner
  print(taken.id)
}
";

fn ir_of(program: &str, label: &str, no_optimize: bool) -> String {
    let built = build_source(program, label, no_optimize);
    std::fs::read_to_string(&built.ir).expect("read emitted IR")
}

#[test]
fn a_nested_shape_no_take_reads_is_released_without_a_null_check() {
    // Bug (cost, Golden Rule 8; High: the check is what keeps a taken cell from a double free, so
    // it must stay where a take exists): a cell release that checks for null where nothing can
    // null the cell costs 0.1-0.25 ns per release over Rust's `Box` drop.
    let ir = ir_of(NESTED_SHAPE_NEVER_TAKEN, "never_taken", true);
    assert!(
        ir.contains("ynz_release_shape:Tag_cell") && !ir.contains("release_cell_taken"),
        "no `Take` reads a `Tag`: its cell release must not test for null:\n{ir}"
    );
}

#[test]
fn a_nested_shape_a_take_reads_is_released_behind_a_null_check_and_balances() {
    // Bug (High: double free): the take nulls the cell in the temporary; the temporary's release
    // of that shape must skip it.
    let ir = ir_of(NESTED_SHAPE_TAKEN, "taken", true);
    assert!(
        ir.contains("release_cell_taken"),
        "a `Take` moves a `Tag` out of a temporary: its cell release must test for null:\n{ir}"
    );
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("taken", NESTED_SHAPE_TAKEN, "5\n", no_optimize);
        assert_heap_balanced("taken", &counters, no_optimize);
        assert_strings_balanced("taken", &counters, no_optimize);
        assert_asan_clean("taken", NESTED_SHAPE_TAKEN, "5\n", no_optimize);
    });
}
