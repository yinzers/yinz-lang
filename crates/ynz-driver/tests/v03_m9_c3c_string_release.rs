//! v0.3-M9 C3c: the strings a container owns are released with the container.
//!
//! The one release helper (C6) frees a container's items, entries and fields; this card is the
//! string half. Until the scope-exit pass (C7) calls the helper for locals, the one place it runs
//! is a `background` task's drop ladder, which releases the task's own value: so each fixture
//! hands one container to a task and counts the heap strings (`string_alloc` / `string_free`,
//! C2's counters) around it.
//!
//! All High tier (memory safety and data integrity: a string released twice, or not at all,
//! is silent at exit 0). One fixture per container kind, three tests each: the count balance
//! (also asserts stdout), and AddressSanitizer at both optimization levels. A static string is
//! never freed (C3a's static mark): `string_free_bytes` of a fixture whose strings are literals
//! is covered by the byte balance.

mod support;

use support::{assert_asan_clean_and_prints, build, run_counted};

/// Build the fixture, run it with the counters on, and assert stdout, the heap strings allocated
/// and the heap strings released (and that the released bytes equal the allocated bytes when
/// every string is released).
fn assert_releases(fixture: &str, expected_stdout: &str, allocated: u64, released: u64) {
    let built = build(fixture, false);
    let (output, counters) = run_counted(&built);
    assert_eq!(output.exit_code, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, expected_stdout, "{fixture}: wrong output");
    assert_eq!(
        (counters.string_alloc, counters.string_free),
        (allocated, released),
        "{fixture}: expected {allocated} heap strings allocated and {released} released \
         (counters: {counters:?})"
    );
    if allocated == released {
        assert_eq!(
            counters.string_alloc_bytes, counters.string_free_bytes,
            "{fixture}: every string was released, so the bytes must balance too"
        );
    }
}

macro_rules! release_case {
    ($case:ident, $fixture:literal, $expected:literal, $allocated:literal, $released:literal, $bug:literal) => {
        #[doc = $bug]
        mod $case {
            use crate::{assert_asan_clean_and_prints, assert_releases};

            #[test]
            fn strings_released() {
                assert_releases($fixture, $expected, $allocated, $released);
            }

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

release_case!(
    array_items,
    "v0_3_m9_c3c_array_items.ynz",
    "first-1\n",
    2,
    2,
    "releasing an `array<string>` frees the buffer and header but not the strings its cells point at (High)"
);

release_case!(
    map_entries_and_raw_pointer_keys,
    "v0_3_m9_c3c_map_entries.ynz",
    "2\n",
    4,
    4,
    "releasing a `map<string, string>` frees its buffers but not its values, nor its keys, which sit in the map as raw pointers (High)"
);

release_case!(
    shape_string_fields,
    "v0_3_m9_c3c_shape_fields.ynz",
    "tag-1\ntag-1\n",
    2,
    1,
    "releasing a task's shape copy frees the struct but not the `string` field the copy cloned (High)"
);

release_case!(
    map_overwrite_frees_the_replaced_value_and_the_duplicate_key,
    "v0_3_m9_c3c_map_overwrite.ynz",
    "1\n",
    4,
    4,
    "`set` on a present key stores the new value over the old one without freeing it, and drops the key copy the set was handed, so a string-keyed map leaks both on every overwrite (High)"
);
