//! v0.3-M9 C2: the heap-string alloc/free counter sees EVERY string allocator, byte-exact.
//!
//! Tier: Medium (test instrumentation that every later string-leak assertion stands on).
//! Bug caught: a string allocator that bypasses `heap_string_buffer` (or a future sixth one
//! that mallocs directly) is invisible to the counter, so a leak assertion built on
//! `string_alloc == string_free` passes vacuously for that allocator. Each allocator is also
//! pinned to its exact byte cost (length + 1 NUL), so a miscounted size cannot hide a leak.
//!
//! This is the ONLY test in this file, on purpose: the counters are process-global statics,
//! and cargo runs each integration-test file as its own process, so a single test here is
//! the one-program-per-process isolation `.claude/rules/test-parallelism.md` asks for — a
//! second test in this file would race it on the same counters. The steps below share that
//! state by nature (each reads a before/after delta), which is why they run in sequence.

use std::ffi::CStr;

use ynz_runtime::runtime::{ynz_rt_init, ynz_rt_shutdown};
use ynz_runtime::{
    ynz_bignum_add, ynz_string_alloc_bytes, ynz_string_alloc_count, ynz_string_builder_append,
    ynz_string_builder_finalize, ynz_string_builder_new, ynz_string_codepoint_at,
    ynz_string_concat, ynz_string_free, ynz_string_free_bytes, ynz_string_free_count,
    ynz_string_from_static, ynz_string_to_upper,
};

/// Run one allocator, check it moved the alloc counters by exactly (1, expected_bytes) and
/// that the returned string has the expected content, then free it through `ynz_string_free`
/// and check the free counters moved by the same amount.
fn check_allocator(
    allocator_name: &str,
    expected_content: &str,
    allocate: impl FnOnce() -> *const u8,
) {
    let alloc_count_before = ynz_string_alloc_count();
    let alloc_bytes_before = ynz_string_alloc_bytes();
    let free_count_before = ynz_string_free_count();
    let free_bytes_before = ynz_string_free_bytes();

    let heap_string = allocate();

    let expected_bytes = expected_content.len() as u64 + 1;
    assert_eq!(
        ynz_string_alloc_count() - alloc_count_before,
        1,
        "{allocator_name}: must count exactly one string allocation"
    );
    assert_eq!(
        ynz_string_alloc_bytes() - alloc_bytes_before,
        expected_bytes,
        "{allocator_name}: must count length + 1 NUL bytes"
    );
    let content = unsafe { CStr::from_ptr(heap_string as *const i8) };
    assert_eq!(
        content.to_str().unwrap(),
        expected_content,
        "{allocator_name}"
    );

    unsafe { ynz_string_free(heap_string) };
    assert_eq!(
        ynz_string_free_count() - free_count_before,
        1,
        "{allocator_name}: ynz_string_free must count exactly one free"
    );
    assert_eq!(
        ynz_string_free_bytes() - free_bytes_before,
        expected_bytes,
        "{allocator_name}: ynz_string_free must count the same bytes the allocation did"
    );
}

#[test]
fn every_string_allocator_is_counted_and_balanced_by_ynz_string_free() {
    // Latched once by `ynz_rt_init`, exactly as a compiled program does (`YNZ_ALLOC_COUNTER`).
    std::env::set_var("YNZ_ALLOC_COUNTER", "1");
    ynz_rt_init();

    check_allocator("ynz_string_from_static", "round", || unsafe {
        ynz_string_from_static(b"round".as_ptr(), 5)
    });
    check_allocator("ynz_string_concat", "foobar", || unsafe {
        ynz_string_concat(c"foo".as_ptr().cast(), c"bar".as_ptr().cast())
    });
    check_allocator("ynz_string_codepoint_at", "é", || unsafe {
        ynz_string_codepoint_at(c"aé".as_ptr().cast(), 1)
    });
    check_allocator("ynz_string_builder_finalize", "ab", || unsafe {
        let builder = ynz_string_builder_new();
        ynz_string_builder_append(builder, c"a".as_ptr().cast());
        ynz_string_builder_append(builder, c"b".as_ptr().cast());
        ynz_string_builder_finalize(builder)
    });
    // `heap_string_from_str` is the shared helper behind to_upper/to_lower/substring/trim/
    // grapheme_at/split/replace; one caller proves the helper is counted.
    check_allocator(
        "heap_string_from_str (via ynz_string_to_upper)",
        "ABC",
        || unsafe { ynz_string_to_upper(c"abc".as_ptr().cast()) },
    );

    // High-precision `number` arithmetic returns its result as a heap string too; it used to
    // come from `CString::into_raw` (invisible to the counter, wrong allocator for the free).
    check_allocator("ynz_bignum_add", "3", || unsafe {
        ynz_bignum_add(c"1".as_ptr().cast(), c"2".as_ptr().cast(), 40) as *const u8
    });

    // Freeing null must be a no-op that moves nothing (a null `string` slot is a legal state).
    let free_count_before = ynz_string_free_count();
    unsafe { ynz_string_free(std::ptr::null()) };
    assert_eq!(ynz_string_free_count(), free_count_before);

    // `ynz_rt_init` starts Tokio worker threads; Miri fails a process that exits with live
    // threads ("main thread terminated without waiting for all remaining threads"), so join
    // them the way a compiled program does at exit.
    ynz_rt_shutdown();
}
