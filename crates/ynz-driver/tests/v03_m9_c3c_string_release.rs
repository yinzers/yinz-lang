//! C3c: the strings a container owns are released with the container. The one release helper (C6)
//! frees a container's items, entries and fields; this card is the string half. The place it
//! runs is a `background` task's drop ladder, which releases the task's own value, and a scope
//! exit: each fixture hands one container to a task or drops it and counts the heap strings
//! (`string_alloc` / `string_free`) around it. One case per container kind; each also runs under
//! AddressSanitizer at both optimization levels. A static string is never freed (C3a's static
//! mark): no Yinz program can store one in a container (every store copies), so that half runs in
//! `ynz-runtime/tests/container_string_release.rs`.

mod support;

use support::{assert_asan_clean_and_prints, at_both_optimization_levels, build, run_counted};

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

/// One named test per container kind of the `container string release` run unit; each case is a
/// test the runner names.
macro_rules! string_release_high_case {
    ($($case_name:ident, $fixture:literal, $expected_stdout:literal, $allocated:literal, $released:literal;)+) => { $(
        // bug: releasing a container leaks the strings it owns, frees one twice, or reads one freed, silently at exit 0 | tier: High
        #[test]
        fn $case_name() {
            assert_releases($fixture, $expected_stdout, $allocated, $released);
            at_both_optimization_levels(|no_optimize| {
                assert_asan_clean_and_prints($fixture, no_optimize, $expected_stdout);
            });
        }
    )+ };
}

string_release_high_case! {
    // case: releasing an `Array<String>` frees the buffer and header but not the strings its cells point at
    array_items, "v0_3_m9_c3c_array_items.ynz", "first-1\n", 2, 2;
    // case: releasing a `Map<String, String>` frees its buffers but not its values, nor its keys, which sit in the map as raw pointers
    map_entries_and_raw_pointer_keys, "v0_3_m9_c3c_map_entries.ynz", "2\n", 4, 4;
    // case: releasing a task's shape copy frees the struct and the `string` field the copy cloned, and a resume function now releases the spawner's own `tag` at its scope exit (C8 slice 4a)
    shape_string_fields, "v0_3_m9_c3c_shape_fields.ynz", "tag-1\ntag-1\n", 2, 2;
    // case: `set` on a present key stores the new value over the old one without freeing it, and drops the key copy the set was handed, so a string-keyed map leaks both on every overwrite
    map_overwrite_frees_the_replaced_value_and_the_duplicate_key, "v0_3_m9_c3c_map_overwrite.ynz", "1\n", 4, 4;
    // case: `set` and `items[i] = ...` store over a string item without freeing it, so every replacement leaks the old bytes
    array_set_and_index_assign_free_the_replaced_string, "v0_3_m9_c3c_array_overwrite.ynz", "1\nthird\n", 3, 3;
    // case: `tag.name = ...` stores over a string field without freeing it, so every assignment leaks the old bytes
    field_assign_frees_the_replaced_string, "v0_3_m9_c3c_field_assign.ynz", "renamed\n", 3, 3;
    // case: `tag.name = tag.name` releases the old string and only then copies it, reading freed bytes
    field_assign_from_the_same_field_copies_before_it_releases, "v0_3_m9_c3c_field_self_assign.ynz", "x-1\n", 2, 2;
    // case: `pts[0] = ...` on a SoA array writes each field segment over the old element's without freeing its strings, while the same assignment on an AoS array frees them the array itself is released at its scope exit by field segment (C7 slice 3), so every string is freed
    soa_scatter_frees_the_replaced_elements_strings, "v0_3_m9_c6_soa_scatter.ynz", "2656\n278\n", 73, 73;
}
