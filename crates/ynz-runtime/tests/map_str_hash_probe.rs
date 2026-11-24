/// String-keyed map hash-probe tests (audit F1, 2026-07-11 non-concurrency audit).
///
/// F1 confirmed that `ynz_map_get_str` / `ynz_map_set_str` did a full O(capacity)
/// linear scan on every operation and never consulted the SipHash the Swiss-table
/// design is built on — making an n-entry string-keyed map O(n²) to build while the
/// int-keyed path is O(n). These tests lock the fix: string-map ops must probe from
/// the hash bucket like the int path's `find_slot`.
use std::ffi::CString;
use std::time::{Duration, Instant};

use ynz_runtime::{
    ynz_map_count, ynz_map_drop, ynz_map_get, ynz_map_get_str, ynz_map_iter_get_str, ynz_map_new,
    ynz_map_set, ynz_map_set_str, YnzMap,
};

/// Build the owned key strings ("key-<i>") whose pointers the map will store.
/// The CStrings must outlive the map — the map stores pointers, not copies.
fn make_keys(n: usize) -> Vec<CString> {
    (0..n)
        .map(|i| CString::new(format!("key-{i}")).unwrap())
        .collect()
}

/// Enough keys to grow the map through several capacity doublings (16 → 8192).
const KEY_COUNT: usize = 5_000;

/// Index of the key the overwrite and iteration cases change.
const OVERWRITTEN_KEY_INDEX: usize = 42;

/// A string-keyed map holding `keys[i] -> i`.
unsafe fn map_of_keys(keys: &[CString]) -> *mut YnzMap {
    let map = ynz_map_new(8);
    for (position, key) in keys.iter().enumerate() {
        let value = position as i64;
        ynz_map_set_str(
            map,
            key.as_ptr() as *const u8,
            &value as *const i64 as *const u8,
            None,
            None,
            0,
        );
    }
    map
}

/// `(found flag, value)` for `key`, with the out slot pre-filled with `sentinel`.
unsafe fn lookup(map: *mut YnzMap, key: &CString, sentinel: i64) -> (i64, i64) {
    let mut value: i64 = sentinel;
    let found = ynz_map_get_str(
        map,
        key.as_ptr() as *const u8,
        &mut value as *mut i64 as *mut u8,
    );
    (found, value)
}

// bug: ynz_map_get_str misses a key, or returns another key's value, after the string map grew | tier: Medium
#[test]
fn every_inserted_string_key_is_found_with_its_own_value_across_growth() {
    let keys = make_keys(KEY_COUNT);
    unsafe {
        let map = map_of_keys(&keys);
        for (position, key) in keys.iter().enumerate() {
            assert_eq!(lookup(map, key, -1), (1, position as i64), "key-{position}");
        }
        ynz_map_drop(map);
    }
}

// bug: ynz_map_get_str compares key pointers instead of key text, so an equal string at another address misses | tier: Medium
#[test]
fn a_key_with_equal_text_at_another_address_is_found() {
    let keys = make_keys(KEY_COUNT);
    unsafe {
        let map = map_of_keys(&keys);
        let alias = CString::new("key-42").unwrap();
        assert_eq!(lookup(map, &alias, 0), (1, 42));
        ynz_map_drop(map);
    }
}

// bug: ynz_map_get_str reports a hit for an absent key, or leaves a stale value in the out slot | tier: Medium
#[test]
fn a_missing_key_reports_a_miss_and_zeroes_the_out_slot() {
    let keys = make_keys(KEY_COUNT);
    unsafe {
        let map = map_of_keys(&keys);
        let missing = CString::new("no-such-key").unwrap();
        assert_eq!(lookup(map, &missing, -1), (0, 0));
        ynz_map_drop(map);
    }
}

// bug: ynz_map_set_str adds a second entry for an existing key instead of updating it in place | tier: Medium
#[test]
fn overwriting_a_key_updates_its_value_and_keeps_the_count() {
    let keys = make_keys(KEY_COUNT);
    unsafe {
        let map = map_of_keys(&keys);
        let count_before = ynz_map_count(map);
        let value: i64 = 999_999;
        ynz_map_set_str(
            map,
            keys[OVERWRITTEN_KEY_INDEX].as_ptr() as *const u8,
            &value as *const i64 as *const u8,
            None,
            None,
            0,
        );
        assert_eq!(ynz_map_count(map), count_before);
        assert_eq!(lookup(map, &keys[OVERWRITTEN_KEY_INDEX], 0), (1, 999_999));
        ynz_map_drop(map);
    }
}

/// One test per iteration position class: first entry, an overwritten entry, last entry:
/// `iteration_case!(case, position, expected_value)`.
macro_rules! iteration_case {
    ($case:ident, $position:expr, $expected_value:expr) => {
        // bug: ynz_map_iter_get_str returns the wrong key pointer or value at an insertion-order position of a string map | tier: Medium
        #[test]
        fn $case() {
            let keys = make_keys(KEY_COUNT);
            unsafe {
                let map = map_of_keys(&keys);
                let overwritten_value: i64 = 999_999;
                ynz_map_set_str(
                    map,
                    keys[OVERWRITTEN_KEY_INDEX].as_ptr() as *const u8,
                    &overwritten_value as *const i64 as *const u8,
                    None,
                    None,
                    0,
                );
                let mut key_out: i64 = 0;
                let mut value_out: i64 = -1;
                let found = ynz_map_iter_get_str(
                    map,
                    $position as i64,
                    &mut key_out as *mut i64,
                    &mut value_out as *mut i64 as *mut u8,
                );
                assert_eq!(found, 1, "iteration position {} should hit", $position);
                assert_eq!(key_out, keys[$position].as_ptr() as i64, "key pointer");
                assert_eq!(value_out, $expected_value, "value");
                ynz_map_drop(map);
            }
        }
    };
}

iteration_case!(iteration_returns_the_first_inserted_entry_first, 0, 0);
iteration_case!(
    iteration_keeps_an_overwritten_entry_at_its_original_position,
    OVERWRITTEN_KEY_INDEX,
    999_999
);
iteration_case!(
    iteration_returns_the_last_inserted_entry_last,
    KEY_COUNT - 1,
    (KEY_COUNT - 1) as i64
);

/// Time one full build + full-lookup pass over an n-key STRING map.
unsafe fn time_str_map(keys: &[CString]) -> Duration {
    let start = Instant::now();
    let map = ynz_map_new(8);
    for (i, k) in keys.iter().enumerate() {
        let v = i as i64;
        ynz_map_set_str(
            map,
            k.as_ptr() as *const u8,
            &v as *const i64 as *const u8,
            None,
            None,
            0,
        );
    }
    let mut out: i64 = 0;
    for k in keys {
        ynz_map_get_str(
            map,
            k.as_ptr() as *const u8,
            &mut out as *mut i64 as *mut u8,
        );
    }
    let elapsed = start.elapsed();
    ynz_map_drop(map);
    elapsed
}

/// Time one full build + full-lookup pass over an n-key INT map (the O(n)
/// reference implementation whose `find_slot` probe the string path must mirror).
unsafe fn time_int_map(n: usize) -> Duration {
    let start = Instant::now();
    let map = ynz_map_new(8);
    for i in 0..n {
        let v = i as i64;
        ynz_map_set(map, i as i64, &v as *const i64 as *const u8, None, 0);
    }
    let mut out: i64 = 0;
    for i in 0..n {
        ynz_map_get(map, i as i64, &mut out as *mut i64 as *mut u8);
    }
    let elapsed = start.elapsed();
    ynz_map_drop(map);
    elapsed
}

// bug: ynz_map_set_str and ynz_map_get_str scan every slot instead of probing from the hash bucket, so an n-entry string map costs O(n²) to build and read | tier: Medium
#[test]
fn nightly_string_map_build_plus_lookup_stays_within_constant_factor_of_int_map() {
    // WHY: F1's O(n²) bug means the string path's build+lookup cost diverges from
    // the int path's O(n) cost by a factor that GROWS with n (measured 1281× at
    // n=20k under the linear scan). A correct hash probe costs a small CONSTANT
    // factor over the int path (longer hash input + content compare at candidate
    // slots). Asserting str <= 25 × int at n=20k fails the quadratic scan by a
    // wide margin (51× safety margin over the 25× threshold) while leaving
    // generous headroom for machine noise.
    const N: usize = 20_000;
    const MAX_RATIO: u128 = 25;
    let keys = make_keys(N);
    unsafe {
        // Take the best of 3 runs on each side to damp scheduler noise.
        let str_time = (0..3).map(|_| time_str_map(&keys)).min().unwrap();
        let int_time = (0..3).map(|_| time_int_map(N)).min().unwrap();

        let str_ns = str_time.as_nanos().max(1);
        let int_ns = int_time.as_nanos().max(1);
        assert!(
            str_ns <= int_ns * MAX_RATIO,
            "string-map build+lookup at n={N} took {str_ns}ns vs int-map {int_ns}ns \
             (ratio {}×, limit {MAX_RATIO}×) — the string path is not using the hash probe",
            str_ns / int_ns,
        );
    }
}
