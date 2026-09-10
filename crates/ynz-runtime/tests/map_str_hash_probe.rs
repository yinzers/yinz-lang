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
    ynz_map_drop, ynz_map_get, ynz_map_get_str, ynz_map_iter_get_str, ynz_map_new, ynz_map_set,
    ynz_map_set_str,
};

/// Build the owned key strings ("key-<i>") whose pointers the map will store.
/// The CStrings must outlive the map — the map stores pointers, not copies.
fn make_keys(n: usize) -> Vec<CString> {
    (0..n)
        .map(|i| CString::new(format!("key-{i}")).unwrap())
        .collect()
}

#[test]
fn string_map_insert_get_overwrite_and_miss_behave_correctly_across_growth() {
    // WHY: the F1 fix replaces the linear scan with a hash probe; this test locks
    // functional equivalence (hit/miss/overwrite/iteration-order) across many
    // growth cycles (16 → 8192 capacity), so the probe rewrite cannot silently
    // change map semantics.
    const N: usize = 5_000;
    let keys = make_keys(N);
    unsafe {
        let map = ynz_map_new(8);

        // Arrange + Act: insert N distinct keys.
        for (i, k) in keys.iter().enumerate() {
            let v = i as i64;
            ynz_map_set_str(map, k.as_ptr() as *const u8, &v as *const i64 as *const u8);
        }

        // Assert: every key hits with its own value.
        for (i, k) in keys.iter().enumerate() {
            let mut out: i64 = -1;
            let flag = ynz_map_get_str(
                map,
                k.as_ptr() as *const u8,
                &mut out as *mut i64 as *mut u8,
            );
            assert_eq!(flag, 1, "key-{i} should be present");
            assert_eq!(out, i as i64, "key-{i} value mismatch");
        }

        // Assert: overwrite updates in place (no duplicate entry, count stable).
        let count_before = ynz_runtime::ynz_map_count(map);
        let v: i64 = 999_999;
        ynz_map_set_str(
            map,
            keys[42].as_ptr() as *const u8,
            &v as *const i64 as *const u8,
        );
        assert_eq!(ynz_runtime::ynz_map_count(map), count_before);
        let mut out: i64 = 0;
        let flag = ynz_map_get_str(
            map,
            keys[42].as_ptr() as *const u8,
            &mut out as *mut i64 as *mut u8,
        );
        assert_eq!((flag, out), (1, 999_999));

        // Assert: a DIFFERENT pointer with equal content still hits (content, not
        // identity, equality).
        let alias = CString::new("key-42").unwrap();
        let mut out2: i64 = 0;
        let flag2 = ynz_map_get_str(
            map,
            alias.as_ptr() as *const u8,
            &mut out2 as *mut i64 as *mut u8,
        );
        assert_eq!((flag2, out2), (1, 999_999));

        // Assert: miss returns 0 and zeroes the out slot.
        let missing = CString::new("no-such-key").unwrap();
        let mut out3: i64 = -1;
        let flag3 = ynz_map_get_str(
            map,
            missing.as_ptr() as *const u8,
            &mut out3 as *mut i64 as *mut u8,
        );
        assert_eq!((flag3, out3), (0, 0));

        // Assert: insertion-order iteration returns every entry, in order.
        for i in [0usize, 1, 42, N - 1] {
            let mut key_out: i64 = 0;
            let mut val_out: i64 = -1;
            let flag = ynz_map_iter_get_str(
                map,
                i as i64,
                &mut key_out as *mut i64,
                &mut val_out as *mut i64 as *mut u8,
            );
            assert_eq!(flag, 1, "iter pos {i} should hit");
            assert_eq!(key_out, keys[i].as_ptr() as i64, "iter pos {i} key pointer");
            let expected = if i == 42 { 999_999 } else { i as i64 };
            assert_eq!(val_out, expected, "iter pos {i} value");
        }

        ynz_map_drop(map);
    }
}

/// Time one full build + full-lookup pass over an n-key STRING map.
unsafe fn time_str_map(keys: &[CString]) -> Duration {
    let start = Instant::now();
    let map = ynz_map_new(8);
    for (i, k) in keys.iter().enumerate() {
        let v = i as i64;
        ynz_map_set_str(map, k.as_ptr() as *const u8, &v as *const i64 as *const u8);
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
        ynz_map_set(map, i as i64, &v as *const i64 as *const u8);
    }
    let mut out: i64 = 0;
    for i in 0..n {
        ynz_map_get(map, i as i64, &mut out as *mut i64 as *mut u8);
    }
    let elapsed = start.elapsed();
    ynz_map_drop(map);
    elapsed
}

#[test]
fn string_map_build_plus_lookup_stays_within_constant_factor_of_int_map() {
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
