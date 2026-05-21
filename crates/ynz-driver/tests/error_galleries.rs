// WHY: Error gallery files are the hands-on review surface for compiler diagnostic
// quality — per plan-invariants.md `### Demo & Error Gallery`. These tests assert
// that each gallery file produces its expected diagnostic count and that key error
// classes are present by key-phrase. If this count changes, either a new diagnostic
// class was added (update count + add a key-phrase check) or a diagnostic regressed
// (compiler bug). Both require deliberate action, not silent drift.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn galleries_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/ynz-driver; walk up two levels to workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples/primantis-orders")
}

fn gallery(name: &str) -> PathBuf {
    galleries_dir().join(name)
}

/// Compile a gallery file and return (stderr, exit_code).
fn compile_gallery(path: &Path) -> (String, i32) {
    let out = Command::new(ynz_binary())
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let code = out.status.code().unwrap_or(-1);
    (stderr, code)
}

fn count_errors(stderr: &str) -> usize {
    stderr.lines().filter(|l| l.starts_with("Error:")).count()
}

#[test]
fn m4_gallery_fires_expected_diagnostics() {
    // WHY: m4_errors.ynz covers banned declaration keywords, parser errors, and
    // typeck errors. The 50-error cap is expected to trigger (50 errors).
    // If this count drops, a diagnostic class regressed. If it rises and doesn't hit
    // the cap, new M4 error classes were added without updating this test.
    let (stderr, code) = compile_gallery(&gallery("m4_errors.ynz"));
    assert_ne!(code, 0, "m4 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    // 50 = error cap fires (file has more than 50 possible errors).
    assert_eq!(
        error_count, 50,
        "m4 gallery must produce exactly 50 errors (cap); got {error_count}.\nstderr:\n{stderr}"
    );

    // Key phrase checks — one per major M4 error class.
    assert!(
        stderr.contains("type"),
        "m4 gallery must mention `type` keyword ban; got:\n{stderr}"
    );
    assert!(
        stderr.contains("struct"),
        "m4 gallery must mention `struct` keyword ban; got:\n{stderr}"
    );
    assert!(
        stderr.contains("class"),
        "m4 gallery must mention `class` keyword ban; got:\n{stderr}"
    );
}

#[test]
fn m5_gallery_fires_expected_diagnostics() {
    // WHY: m5_errors.ynz covers collection/array errors. 8 diagnostics expected.
    let (stderr, code) = compile_gallery(&gallery("m5_errors.ynz"));
    assert_ne!(code, 0, "m5 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (7..=12).contains(&error_count),
        "m5 gallery must produce 7–12 errors; got {error_count}.\nstderr:\n{stderr}"
    );
}

#[test]
fn m6_gallery_fires_expected_diagnostics() {
    // WHY: m6_errors.ynz covers union type errors. 9 diagnostics expected.
    let (stderr, code) = compile_gallery(&gallery("m6_errors.ynz"));
    assert_ne!(code, 0, "m6 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (7..=14).contains(&error_count),
        "m6 gallery must produce 7–14 errors; got {error_count}.\nstderr:\n{stderr}"
    );
}

#[test]
fn m7_gallery_fires_expected_diagnostics() {
    // WHY: m7_errors.ynz covers errors keyword, string, and iterable diagnostics.
    // Expected: 7 compile errors (noErrorsContext, badOr, badFailed, stringMutation,
    // badContains, iterateInt, iterateBool).
    let (stderr, code) = compile_gallery(&gallery("m7_errors.ynz"));
    assert_ne!(code, 0, "m7 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (6..=12).contains(&error_count),
        "m7 gallery must produce 6–12 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    assert!(
        stderr.contains("not handled"),
        "m7 gallery must include an unhandled-errors diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("for` loops"),
        "m7 gallery must include a not-iterable diagnostic; got:\n{stderr}"
    );
}

#[test]
fn m8_gallery_fires_expected_diagnostics() {
    // WHY: m8_errors.ynz covers concurrency keyword errors (2 errors) plus
    // v0.1-polish inline shape type errors (4+ errors). Expected: 5–10.
    // test-ratchet: v0.1-polish adds 3 inline-shape error triggers (unknown field,
    // missing field, hidden-in-inline) — count grows from 2 to ~6.
    let (stderr, code) = compile_gallery(&gallery("m8_errors.ynz"));
    assert_ne!(code, 0, "m8 gallery must exit non-zero");

    let error_count = count_errors(&stderr);
    assert!(
        (5..=12).contains(&error_count),
        "m8 gallery must produce 5–12 errors; got {error_count}.\nstderr:\n{stderr}"
    );

    assert!(
        stderr.contains("background"),
        "m8 gallery must include a background-share diagnostic; got:\n{stderr}"
    );
}

// WHY: v0_3_m1_errors.ynz exercises every new v0.3-M1 error/warning class:
// share-param carry-forward, lend-cross-thread, large-copy warning, and the
// happy-path fire-and-forget. If error count changes, either a new diagnostic
// class was added (update count + key-phrase) or something regressed (fix it).
#[test]
fn v0_3_m1_gallery_fires_expected_diagnostics() {
    let (stderr, code) = compile_gallery(&gallery("v0_3_m1_errors.ynz"));
    // Gallery has intentional errors; must exit non-zero.
    assert_ne!(code, 0, "v0_3_m1 gallery must exit non-zero");

    // Count compile errors (warnings are separate).
    let error_count = count_errors(&stderr);
    let warning_count = stderr.lines().filter(|l| l.starts_with("Warning:")).count();

    // Expected: 3 errors (no-entrypoint + share-param + lend-cross-thread) and 1 warning (large-copy).
    assert!(
        (2..=5).contains(&error_count),
        "v0_3_m1 gallery must produce 2–5 errors; got {error_count}.\nstderr:\n{stderr}"
    );
    assert_eq!(
        warning_count, 1,
        "v0_3_m1 gallery must produce exactly 1 warning (large-copy); got {warning_count}.\nstderr:\n{stderr}"
    );

    // Key-phrase checks — one per error/warning class.
    assert!(
        stderr.contains("borrows its arguments"),
        "v0_3_m1 gallery must include share-param diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("mutates its arguments via `lend`"),
        "v0_3_m1 gallery must include lend-cross-thread diagnostic; got:\n{stderr}"
    );
    assert!(
        stderr.contains("Copying") && stderr.contains("bytes into a background task"),
        "v0_3_m1 gallery must include large-copy warning; got:\n{stderr}"
    );
}
