//! v0.3-M9 C3b: `ynz_string_clone` and `ynz_map_clone_strings` give a copy bytes of its own.
//!
//! Tier: High (memory safety: a copy that shares a string with its source frees it twice once
//! the scope-exit release lands). Run under AddressSanitizer (the `sanitizers` CI lane): the
//! releases below are the ones a release pass makes, and a shared string frees twice. Without
//! the sanitizer the same tests still check the pointers are distinct and the bytes survive.

use std::ffi::{CStr, CString};

use ynz_runtime::{
    ynz_map_clone, ynz_map_clone_strings, ynz_map_count, ynz_map_drop, ynz_map_get,
    ynz_map_get_str, ynz_map_iter_get_str, ynz_map_new, ynz_map_set, ynz_map_set_str,
    ynz_string_clone, ynz_string_concat, ynz_string_release, StaticString,
};

/// A literal as codegen emits it: header word 0, then the bytes and a NUL.
static LITERAL: StaticString<6> = StaticString::new(*b"hello\0");

fn text<'string>(string: *const u8) -> &'string str {
    // SAFETY: every caller passes a live NUL-terminated string.
    unsafe { CStr::from_ptr(string as *const i8) }
        .to_str()
        .expect("test strings are UTF-8")
}

/// A heap string with the text `left` + `right`, owned by the caller.
fn heap_string(left: &str, right: &str) -> *const u8 {
    let left = CString::new(left).unwrap();
    let right = CString::new(right).unwrap();
    unsafe { ynz_string_concat(left.as_ptr().cast(), right.as_ptr().cast()) }
}

/// Bug caught: a clone that returns its argument, so a store of a borrowed string aliases the
/// source and releasing both frees one string twice. A static source must come back as a heap
/// string too (a release of static bytes is an invalid free), and a heap source must survive its
/// own release. (High)
#[test]
fn a_clone_of_a_static_or_heap_string_has_bytes_of_its_own() {
    let literal = LITERAL.string_pointer();
    let from_literal = unsafe { ynz_string_clone(literal) };
    assert_ne!(
        from_literal, literal,
        "a clone of a literal is a new string"
    );
    assert_eq!(text(from_literal), "hello");

    let heap = heap_string("wor", "ld");
    let from_heap = unsafe { ynz_string_clone(heap) };
    assert_ne!(from_heap, heap, "a clone of a heap string is a new string");
    unsafe { ynz_string_release(heap) };
    assert_eq!(text(from_heap), "world", "the clone outlives its source");

    unsafe {
        ynz_string_release(from_literal);
        ynz_string_release(from_heap);
    }
}

/// Bug caught: a clone that dereferences null, or turns the `none` of a `T | nothing` union into
/// an empty string. (High)
#[test]
fn a_clone_of_null_stays_null() {
    assert!(unsafe { ynz_string_clone(std::ptr::null()) }.is_null());
}

/// Every `(key, value)` of a `map<string, string>` in insertion order.
fn string_entries(map: *const ynz_runtime::YnzMap) -> Vec<(*const u8, *const u8)> {
    let mut entries = Vec::new();
    for position in 0..unsafe { ynz_map_count(map) } {
        let mut key: i64 = 0;
        let mut value: *const u8 = std::ptr::null();
        let found = unsafe {
            ynz_map_iter_get_str(
                map,
                position,
                &mut key,
                &mut value as *mut *const u8 as *mut u8,
            )
        };
        assert_eq!(found, 1, "position {position} is in range");
        entries.push((key as *const u8, value));
    }
    entries
}

/// Bug caught: `ynz_map_clone` copies the cells, so without `ynz_map_clone_strings` (or with it
/// rewriting only `keys`, not `insert_order`) the clone's keys or values point at the source's
/// strings: releasing both maps' strings frees each twice, and an iteration reads a freed key. (High)
#[test]
fn a_cloned_string_map_owns_every_key_and_value() {
    unsafe {
        let source = ynz_map_new(8);
        let source_strings = [
            (heap_string("k", "1"), heap_string("v", "1")),
            (heap_string("k", "2"), heap_string("v", "2")),
            (heap_string("k", "3"), heap_string("v", "3")),
        ];
        for (key, value) in &source_strings {
            ynz_map_set_str(source, *key, value as *const *const u8 as *const u8);
        }
        let clone = ynz_map_clone(source);
        ynz_map_clone_strings(clone, 1, 1);

        let source_entries = string_entries(source);
        let clone_entries = string_entries(clone);
        assert_eq!(clone_entries.len(), 3);
        for (position, ((source_key, source_value), (clone_key, clone_value))) in
            source_entries.iter().zip(&clone_entries).enumerate()
        {
            assert_ne!(clone_key, source_key, "entry {position}: key is shared");
            assert_ne!(
                clone_value, source_value,
                "entry {position}: value is shared"
            );
            assert_eq!(
                text(*clone_key),
                text(*source_key),
                "entry {position}: key text"
            );
            assert_eq!(
                text(*clone_value),
                text(*source_value),
                "entry {position}: value text"
            );
        }

        // Release the source's strings and map, as the source's scope exit will; the clone must
        // still be whole, and a lookup by the text the clone's keys hold must still hit.
        for (key, value) in source_strings {
            ynz_string_release(key);
            ynz_string_release(value);
        }
        ynz_map_drop(source);
        for (key, value) in &clone_entries {
            let mut found: *const u8 = std::ptr::null();
            let hit = ynz_map_get_str(clone, *key, &mut found as *mut *const u8 as *mut u8);
            assert_eq!(hit, 1, "the clone's own key is found");
            assert_eq!(found, *value, "the lookup returns the clone's own value");
        }
        let texts: Vec<&str> = clone_entries.iter().map(|(key, _)| text(*key)).collect();
        assert_eq!(
            texts,
            ["k1", "k2", "k3"],
            "insertion order survives the rewrite"
        );

        for (key, value) in clone_entries {
            ynz_string_release(key);
            ynz_string_release(value);
        }
        ynz_map_drop(clone);
    }
}

/// Bug caught: the flags are ignored, so a `map<string, int>` has its `int` values read as string
/// pointers (a crash) or a `map<int, string>` has its `int` keys read as string pointers. The
/// untouched side must keep the source's bits. (High)
#[test]
fn only_the_string_side_of_a_map_is_copied() {
    unsafe {
        // map<string, int>: keys only.
        let counts = ynz_map_new(8);
        let key = heap_string("tal", "ly");
        let count: i64 = 7;
        ynz_map_set_str(counts, key, &count as *const i64 as *const u8);
        let counts_clone = ynz_map_clone(counts);
        ynz_map_clone_strings(counts_clone, 1, 0);
        let counts_entries = {
            let mut key_out: i64 = 0;
            let mut value_out: i64 = 0;
            let found = ynz_map_iter_get_str(
                counts_clone,
                0,
                &mut key_out,
                &mut value_out as *mut i64 as *mut u8,
            );
            assert_eq!(found, 1);
            (key_out as *const u8, value_out)
        };
        assert_ne!(counts_entries.0, key, "the string key is copied");
        assert_eq!(text(counts_entries.0), "tally");
        assert_eq!(counts_entries.1, 7, "the int value is untouched");
        ynz_string_release(counts_entries.0);
        ynz_string_release(key);
        ynz_map_drop(counts_clone);
        ynz_map_drop(counts);

        // map<int, string>: values only.
        let names = ynz_map_new(8);
        let value = heap_string("nam", "e");
        ynz_map_set(names, 42, &value as *const *const u8 as *const u8);
        let names_clone = ynz_map_clone(names);
        ynz_map_clone_strings(names_clone, 0, 1);
        let mut value_out: *const u8 = std::ptr::null();
        let hit = ynz_map_get(names_clone, 42, &mut value_out as *mut *const u8 as *mut u8);
        assert_eq!(hit, 1, "the int key is untouched");
        assert_ne!(value_out, value, "the string value is copied");
        assert_eq!(text(value_out), "name");
        ynz_string_release(value_out);
        ynz_string_release(value);
        ynz_map_drop(names_clone);
        ynz_map_drop(names);
    }
}
