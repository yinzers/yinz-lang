//! v0.3-M9 C3c: releasing an array or a map releases the heap strings it owns and never frees a
//! static one.
//!
//! Tier: High (memory safety: a walk that frees a literal's bytes is an invalid free, and one that
//! skips a heap string leaks it).
//!
//! No Yinz program can put a static string in a container before C7 (every store copies), so the
//! driver tests cannot observe this: the container walks run here with `ynz_string_release`, the
//! glue codegen passes for string items, string keys and string values, over containers that
//! hold a mix of static and heap strings.
//!
//! This is the ONLY test in this file, on purpose: the string counters are process-global
//! statics, and a second test would race it on them
//! (`.claude/rules/test-parallelism.md`, as `string_counter.rs`).

use std::ffi::CStr;

use ynz_runtime::runtime::{ynz_rt_init, ynz_rt_shutdown};
use ynz_runtime::{
    ynz_array_new, ynz_array_push, ynz_array_release, ynz_map_new, ynz_map_release,
    ynz_map_set_str, ynz_string_alloc_bytes, ynz_string_alloc_count, ynz_string_clone,
    ynz_string_free_bytes, ynz_string_free_count, ynz_string_release, StaticString,
};

static FIRST_LITERAL: StaticString<6> = StaticString::new(*b"first\0");
static SECOND_LITERAL: StaticString<7> = StaticString::new(*b"second\0");
static THIRD_LITERAL: StaticString<6> = StaticString::new(*b"third\0");

/// `ynz_string_release` with the signature of a container walk's glue.
unsafe extern "C" fn release_string_glue(string: *mut u8) {
    ynz_string_release(string as *const u8);
}

fn text<'string>(string: *const u8) -> &'string str {
    // SAFETY: every caller passes a live NUL-terminated string.
    unsafe { CStr::from_ptr(string as *const i8) }
        .to_str()
        .expect("test strings are UTF-8")
}

/// The heap string counters: `(allocated, freed, allocated bytes, freed bytes)`.
fn counters() -> (u64, u64, u64, u64) {
    (
        ynz_string_alloc_count(),
        ynz_string_free_count(),
        ynz_string_alloc_bytes(),
        ynz_string_free_bytes(),
    )
}

/// What `release` did to the string counters, for a container holding `heap_strings` heap strings
/// of `heap_bytes` bytes in all (each counted with its NUL) and some static ones: exactly the heap
/// strings are freed, byte for byte.
fn assert_frees_only_the_heap_strings(
    container: &str,
    before: (u64, u64, u64, u64),
    heap_strings: u64,
    heap_bytes: u64,
    release: impl FnOnce(),
) {
    release();
    let after = counters();
    assert_eq!(
        (after.1 - before.1, after.3 - before.3),
        (heap_strings, heap_bytes),
        "{container}: releasing it must free exactly its {heap_strings} heap strings \
         ({heap_bytes} bytes) and leave its static ones alone"
    );
}

// bug: releasing an array or map frees a static string's bytes (invalid free) or leaks a heap string it owns | tier: High
#[test]
fn a_container_release_frees_its_heap_strings_and_leaves_its_static_ones() {
    // Latched once by `ynz_rt_init`, exactly as a compiled program does.
    std::env::set_var("YNZ_ALLOC_COUNTER", "1");
    ynz_rt_init();
    let first = FIRST_LITERAL.string_pointer();
    let second = SECOND_LITERAL.string_pointer();
    let third = THIRD_LITERAL.string_pointer();

    // An array of string pointers: static, heap, static, heap. Heap strings: "heap-a" (7 bytes)
    // and "heap-b" (7 bytes).
    unsafe {
        let heap_a = ynz_string_clone(c"heap-a".as_ptr().cast());
        let heap_b = ynz_string_clone(c"heap-b".as_ptr().cast());
        let array = ynz_array_new(8);
        for item in [first, heap_a, second, heap_b] {
            ynz_array_push(array, &item as *const *const u8 as *const u8);
        }
        let before = counters();
        assert_frees_only_the_heap_strings("Array<String>", before, 2, 14, || {
            ynz_array_release(array, Some(release_string_glue), 0)
        });
        assert_eq!(text(first), "first", "a static item survives the release");
        assert_eq!(text(second), "second", "a static item survives the release");
    }

    // A `map<string, string>` whose keys are raw pointers the map owns: one entry of static key
    // and static value, one of heap key and heap value, and two mixed. Heap strings: "key-h"
    // (6 bytes), "val-h" (6), "key-m" (6) and "val-m" (6).
    unsafe {
        let heap_key = ynz_string_clone(c"key-h".as_ptr().cast());
        let heap_value = ynz_string_clone(c"val-h".as_ptr().cast());
        let mixed_key = ynz_string_clone(c"key-m".as_ptr().cast());
        let mixed_value = ynz_string_clone(c"val-m".as_ptr().cast());
        let map = ynz_map_new(8);
        for (key, value) in [
            (first, second),
            (heap_key, heap_value),
            (mixed_key, third),
            (second, mixed_value),
        ] {
            ynz_map_set_str(
                map,
                key,
                &value as *const *const u8 as *const u8,
                None,
                None,
                0,
            );
        }
        let before = counters();
        assert_frees_only_the_heap_strings("Map<String, String>", before, 4, 24, || {
            ynz_map_release(map, Some(release_string_glue), Some(release_string_glue), 0)
        });
        for literal in [first, second, third] {
            assert!(
                matches!(text(literal), "first" | "second" | "third"),
                "a static key or value survives the release"
            );
        }
    }

    let (allocated, freed, allocated_bytes, freed_bytes) = counters();
    assert_eq!(
        (allocated, allocated_bytes),
        (freed, freed_bytes),
        "every heap string the test made was released exactly once"
    );
    ynz_rt_shutdown();
}
