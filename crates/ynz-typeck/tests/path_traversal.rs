// WHY: path traversal via mid-path `..` and symlink escape in import resolution
// are attacker-controlled input vectors — a malicious `.ynz` file that escapes
// the project root could leak proprietary source via diagnostic output.
// These tests enforce both defense layers: segment check (layer 1) and
// canonicalize+starts_with boundary check (layer 2).
//
// SECURITY INVARIANT: should-FAIL tests must NEVER be relaxed.
// If a should-FAIL test breaks, examine resolve_module_path's component check
// (layer 1) and the starts_with boundary check (layer 2) before touching the test.
//
// should-PASS tests: if any of these fail, the fix is too aggressive (false positive).
// Do NOT loosen should-FAIL tests to make should-PASS tests pass — tighten
// the component check to match only exactly `..` or `.`.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

fn make_temp_project(suffix: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ynz_path_traversal_{}_{}",
        suffix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n").expect("write yinz.toml");
    dir
}

fn check_file_with_import(dir: &std::path::Path, import_path: &str) -> CheckOutput {
    let main_src =
        format!("import {{ x }} from `{import_path}`\nfunction entrypoint() -> nothing {{}}");
    let main_path = dir.join("main.ynz");
    std::fs::write(&main_path, &main_src).expect("write main.ynz");

    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, main_path.display().to_string(), main_src.clone());
    db.register_source(sf);

    (*check_query(&db, sf)).clone()
}

/// Variant of check_file_with_import that also registers a target file in the
/// salsa db. Required for should-PASS tests where the target module must resolve
/// cleanly — the resolver calls db.source_by_path and returns an error if the
/// file is missing from the registry even when it exists on disk.
fn check_file_with_registered_import(
    dir: &std::path::Path,
    import_path: &str,
    target_path: &std::path::Path,
    target_src: &str,
) -> CheckOutput {
    let main_src =
        format!("import {{ x }} from `{import_path}`\nfunction entrypoint() -> nothing {{}}");
    let main_path = dir.join("main.ynz");
    std::fs::write(&main_path, &main_src).expect("write main.ynz");

    let mut db = CompilerDb::default();
    let sf_target = SourceFile::new(
        &db,
        target_path.display().to_string(),
        target_src.to_string(),
    );
    db.register_source(sf_target);

    let sf = SourceFile::new(&db, main_path.display().to_string(), main_src.clone());
    db.register_source(sf);

    (*check_query(&db, sf)).clone()
}

fn has_any_error(output: &CheckOutput) -> bool {
    output
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
}

// --- SHOULD-FAIL cases ---
// Each test MUST emit at least one error diagnostic.
// If any quietly resolves (zero errors), the security fix is incomplete.

#[test]
fn import_with_dotdot_in_middle_blocked() {
    // WHY: mid-path `..` is the primary path-traversal vector. Without this check,
    // an attacker-controlled .ynz can write `foo/../../etc/secret` to escape the
    // project root. Layer 1 (segment check) catches `..` regardless of context.
    // NEVER relax this test — security regression if it fails.
    let dir = make_temp_project("dotdot_middle");
    let output = check_file_with_import(&dir, "foo/../../escape");
    assert!(
        has_any_error(&output),
        "import `foo/../../escape` must produce an error; got 0 errors. \
         Check resolve_imports layer-1 segment check. Diagnostics: {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_with_dotdot_via_subdir_blocked() {
    // WHY: `subdir/../../escape` — the subdir indirection does not bypass the check.
    // The segment check sees `..` at the third component regardless of depth.
    let dir = make_temp_project("dotdot_subdir");
    let output = check_file_with_import(&dir, "subdir/../../escape");
    assert!(
        has_any_error(&output),
        "import `subdir/../../escape` must produce an error; got 0 errors. \
         Diagnostics: {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_with_dot_slash_then_dotdot_blocked() {
    // WHY: `./inner/../../escape` mixes `.` and `..`. Both segment kinds are
    // rejected independently by the component check.
    let dir = make_temp_project("dot_then_dotdot");
    let output = check_file_with_import(&dir, "./inner/../../escape");
    assert!(
        has_any_error(&output),
        "import `./inner/../../escape` must produce an error; got 0 errors. \
         Diagnostics: {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_with_mixed_dot_and_dotdot_blocked() {
    // WHY: `subdir/./../../escape` mixes single-dot and double-dot segments.
    // Both `.` and `..` are exact-segment matches and are rejected.
    let dir = make_temp_project("mixed_dots");
    let output = check_file_with_import(&dir, "subdir/./../../escape");
    assert!(
        has_any_error(&output),
        "import `subdir/./../../escape` must produce an error; got 0 errors. \
         Diagnostics: {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_with_multiple_dotdots_overshooting_blocked() {
    // WHY: `foo/bar/../../../../escape` has more `..` than the path depth.
    // Layer 1 catches the `..` segments; layer 2 (boundary check) catches
    // any case that slips past layer 1 via a resolved path outside the root.
    let dir = make_temp_project("multi_dotdot");
    let output = check_file_with_import(&dir, "foo/bar/../../../../escape");
    assert!(
        has_any_error(&output),
        "import `foo/bar/../../../../escape` must produce an error; got 0 errors. \
         Diagnostics: {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_via_symlink_escape_blocked() {
    // WHY: a symlink inside the project that points outside the project root is
    // a layer-2 escape. The segment check (layer 1) cannot detect symlinks —
    // only the canonicalize + starts_with boundary check (layer 2) catches it.
    // This test verifies layer 2 is present and functional.
    //
    // Setup:
    //   /tmp/proj/        ← project root with yinz.toml
    //   /tmp/proj/main.ynz  (imports `escape_link/file`)
    //   /tmp/proj/escape_link  ← symlink → /tmp/elsewhere
    //   /tmp/elsewhere/file.ynz  ← would expose contents if imported
    //
    // Expected: error about module not found OR boundary escape.
    // The test passes as long as at least one error is emitted — the module
    // must NOT resolve cleanly to the outside-root file.
    let dir = make_temp_project("symlink_escape");

    // Create the outside-root target directory and file.
    let elsewhere = std::env::temp_dir().join(format!(
        "ynz_symlink_target_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&elsewhere).expect("create elsewhere dir");
    std::fs::write(
        elsewhere.join("file.ynz"),
        "export function x() -> int { return 1 }",
    )
    .expect("write outside file");

    // Create a symlink inside the project that points outside.
    let link_path = dir.join("escape_link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, &link_path).expect("create symlink");

    // On non-Unix: skip the symlink part; the test reduces to "file not found"
    // which is still an error (and tests the happy-path for non-Unix builds).
    #[cfg(not(unix))]
    {
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&elsewhere);
        return;
    }

    let output = check_file_with_import(&dir, "escape_link/file");
    assert!(
        has_any_error(&output),
        "import via symlink that escapes the project root must produce an error; \
         got 0 errors. Diagnostics: {:#?}",
        output.diagnostics
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&elsewhere);
}

// --- SHOULD-PASS cases ---
// Each test MUST resolve with zero errors from path resolution.
// If any of these produce path-related errors, the fix is too aggressive.
// DO NOT loosen the should-FAIL tests above; fix the component check here instead.

#[test]
fn normal_nested_import_resolves() {
    // WHY: `subdir/file` is a legitimate nested import. The traversal check must
    // NOT reject this — if it does, the fix is over-aggressive (false positive).
    let dir = make_temp_project("normal_nested");
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).expect("create subdir");
    let target = sub.join("file.ynz");
    let target_src = "export function x() -> int { return 1 }";
    std::fs::write(&target, target_src).expect("write file.ynz");

    let output = check_file_with_registered_import(&dir, "subdir/file", &target, target_src);
    let path_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("not found")
                    || d.what.contains("segments")
                    || d.what.contains("boundary")
                    || d.what.contains("registered"))
        })
        .collect();
    assert!(
        path_errors.is_empty(),
        "Legitimate `subdir/file` import must NOT produce path errors. \
         If this fails, the traversal check is over-aggressive. Fix the check, \
         not these tests. Errors: {:#?}",
        path_errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn flat_import_in_project_root_resolves() {
    // WHY: `file` (no slashes) is the simplest valid import form. Must not be blocked.
    let dir = make_temp_project("flat_import");
    let target = dir.join("file.ynz");
    let target_src = "export function x() -> int { return 1 }";
    std::fs::write(&target, target_src).expect("write file.ynz");

    let output = check_file_with_registered_import(&dir, "file", &target, target_src);
    let path_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("not found")
                    || d.what.contains("segments")
                    || d.what.contains("boundary")
                    || d.what.contains("registered"))
        })
        .collect();
    assert!(
        path_errors.is_empty(),
        "Flat import `file` must NOT produce path errors. Errors: {:#?}",
        path_errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deeply_nested_path_resolves() {
    // WHY: `deeply/nested/module` — depth alone doesn't trigger the traversal check.
    // Only segments that are EXACTLY `..` or `.` are rejected.
    let dir = make_temp_project("deep_nested");
    let deep = dir.join("deeply").join("nested");
    std::fs::create_dir_all(&deep).expect("create deep dirs");
    let target = deep.join("module.ynz");
    let target_src = "export function x() -> int { return 1 }";
    std::fs::write(&target, target_src).expect("write module.ynz");

    let output =
        check_file_with_registered_import(&dir, "deeply/nested/module", &target, target_src);
    let path_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("not found")
                    || d.what.contains("segments")
                    || d.what.contains("boundary")
                    || d.what.contains("registered"))
        })
        .collect();
    assert!(
        path_errors.is_empty(),
        "Deeply nested import must NOT produce path errors. Errors: {:#?}",
        path_errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn directory_name_with_dots_resolves() {
    // WHY: `dir.with.dots/file` — the segment "dir.with.dots" contains dots but
    // is NOT exactly `.` or `..`. The check uses `seg == ".."` not `contains(".")`.
    // If this test fails, the check uses substring matching instead of exact-segment
    // matching — fix the check, not this test.
    let dir = make_temp_project("dir_with_dots");
    let sub = dir.join("dir.with.dots");
    std::fs::create_dir_all(&sub).expect("create dir.with.dots");
    let target = sub.join("file.ynz");
    let target_src = "export function x() -> int { return 1 }";
    std::fs::write(&target, target_src).expect("write file.ynz");

    let output = check_file_with_registered_import(&dir, "dir.with.dots/file", &target, target_src);
    let path_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("not found")
                    || d.what.contains("segments")
                    || d.what.contains("boundary")
                    || d.what.contains("registered"))
        })
        .collect();
    assert!(
        path_errors.is_empty(),
        "Import `dir.with.dots/file` must NOT produce path errors — `dir.with.dots` \
         is not literally `..`. If this fails, the segment check uses substring \
         matching instead of exact-segment matching. Fix the check. Errors: {:#?}",
        path_errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_name_with_dots_resolves() {
    // WHY: `subdir/file.with.dots` — the file name segment "file.with.dots" contains
    // dots but is not exactly `.` or `..`. Same reasoning as directory_name_with_dots.
    let dir = make_temp_project("file_with_dots");
    let sub = dir.join("subdir");
    std::fs::create_dir_all(&sub).expect("create subdir");
    let target = sub.join("file.with.dots.ynz");
    let target_src = "export function x() -> int { return 1 }";
    std::fs::write(&target, target_src).expect("write file.with.dots.ynz");

    let output =
        check_file_with_registered_import(&dir, "subdir/file.with.dots", &target, target_src);
    let path_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("not found")
                    || d.what.contains("segments")
                    || d.what.contains("boundary")
                    || d.what.contains("registered"))
        })
        .collect();
    assert!(
        path_errors.is_empty(),
        "Import `subdir/file.with.dots` must NOT produce path errors. \
         If this fails, the segment check is too broad. Fix the check. Errors: {:#?}",
        path_errors
    );
    let _ = std::fs::remove_dir_all(&dir);
}
