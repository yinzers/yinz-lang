//! v0.3-M9 C7 slice 3: inline aggregate locals (nested shapes, `maybe`, unions, `fixed`,
//! errors-capable values, SoA arrays) are released at their scope exits through the one release
//! helper, and nothing is freed early or twice (`IMP-ownership.md` "Scope-exit release on the
//! plain path").
//!
//! Each case runs at both optimization levels. The counters say every allocation was freed (the
//! LEAK side) and AddressSanitizer with `free_fill_byte=0` says nothing was freed early or twice
//! (the SAFETY side). All are High tier: a wrong release is a use-after-free, a double free or a
//! leak, silent at exit 0.

mod support;

use support::{
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
};

/// What a case checks the counters of.
#[derive(Clone, Copy)]
enum Balance {
    Heap,
    HeapAndStrings,
    Strings,
}

fn check(label: &str, program: &str, expected: &str, balance: Balance) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        match balance {
            Balance::Heap => assert_heap_balanced(label, &counters, no_optimize),
            Balance::HeapAndStrings => {
                assert_heap_balanced(label, &counters, no_optimize);
                assert_strings_balanced(label, &counters, no_optimize);
            }
            Balance::Strings => assert_strings_balanced(label, &counters, no_optimize),
        }
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

/// `case!(name, PROGRAM, "stdout", Balance::X, "the bug it catches (High)")`.
macro_rules! case {
    ($case:ident, $program:expr, $expected:literal, $balance:expr, $bug:literal) => {
        #[doc = $bug]
        #[test]
        fn $case() {
            check(stringify!($case), $program, $expected, $balance);
        }
    };
}

// ── Item 1: nested shapes ───────────────────────────────────────────────────────────────────

const NESTED_SHAPE_LOCAL: &str = "
shape Tag {
  name: string
  id: int
}

shape Wrap {
  inner: Tag
  label: string
}

function make(index: int) -> Wrap {
  let tag: Tag = { name: `t`, id: index }
  return { inner: tag, label: `l` }
}

function entrypoint() -> nothing {
  let total: int = 0
  for (index in range(0, 100)) {
    let wrap: Wrap = make(index)
    total = total + wrap.inner.id
  }
  print(total)
}
";

case!(
    a_nested_shape_local_is_released_with_its_inner_cell,
    NESTED_SHAPE_LOCAL,
    "4950\n",
    Balance::HeapAndStrings,
    "a shape holding a shape (each shape-typed field is a heap cell) is never released at its scope exit: 100 cells and 200 strings leak (High)"
);
