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
        format!("import {{ x }} from `{import_path}`\nfunction entrypoint() -> Nothing {{}}");
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
        format!("import {{ x }} from `{import_path}`\nfunction entrypoint() -> Nothing {{}}");
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

/// The errors a legitimate import must not produce: the resolver refusing the path itself.
fn path_errors(output: &CheckOutput) -> Vec<&ynz_diagnostics::Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(d.severity, ynz_diagnostics::Severity::Error)
                && (d.what.contains("not found")
                    || d.what.contains("segments")
                    || d.what.contains("boundary")
                    || d.what.contains("registered"))
        })
        .collect()
}

/// The import must produce at least one error: it must NOT resolve cleanly.
fn assert_import_blocked(suffix: &str, import_path: &str) {
    let dir = make_temp_project(suffix);
    let output = check_file_with_import(&dir, import_path);
    assert!(
        has_any_error(&output),
        "import `{import_path}` must produce an error; got 0 errors. \
         Check resolve_imports layer-1 segment check. Diagnostics: {:#?}",
        output.diagnostics
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The module sits at `<project>/<directory>/<file_name>.ynz` and `import_path` must resolve to
/// it with no path error: a false positive here means the check is over-aggressive.
fn assert_import_resolves(suffix: &str, directory: &str, file_name: &str, import_path: &str) {
    let dir = make_temp_project(suffix);
    let module_dir = dir.join(directory);
    std::fs::create_dir_all(&module_dir).expect("create module dir");
    let target = module_dir.join(format!("{file_name}.ynz"));
    let target_src = "export function x() -> Int { return 1 }";
    std::fs::write(&target, target_src).expect("write module file");

    let output = check_file_with_registered_import(&dir, import_path, &target, target_src);
    let errors = path_errors(&output);
    assert!(
        errors.is_empty(),
        "Legitimate import `{import_path}` must NOT produce path errors. \
         If this fails, the traversal check is over-aggressive. Fix the check, \
         not these tests. Errors: {errors:#?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

macro_rules! blocked_import_cases {
    ($($case:ident: $import_path:literal;)+) => {
        $(
            // bug: typeck resolves an import whose path holds an exact `.` or `..` segment, so a malicious .ynz escapes the project root | tier: High
            #[test]
            fn $case() {
                assert_import_blocked(stringify!($case), $import_path);
            }
        )+
    };
}

// Cases are the input classes the segment check decides: a `..` in the middle of the path, a
// leading `.` before a `..`, and a `.` between two `..`. Deeper or longer `..` chains are the same
// class: layer 1 rejects every exact `..` segment.
blocked_import_cases! {
    import_with_dotdot_in_middle_blocked: "foo/../../escape";
    import_with_dot_slash_then_dotdot_blocked: "./inner/../../escape";
    import_with_mixed_dot_and_dotdot_blocked: "subdir/./../../escape";
}

macro_rules! resolving_import_cases {
    ($($case:ident: $directory:literal, $file_name:literal, $import_path:literal;)+) => {
        $(
            // bug: typeck refuses a legitimate import whose directory or file name merely contains dots (substring instead of exact-segment match) | tier: High
            #[test]
            fn $case() {
                assert_import_resolves(stringify!($case), $directory, $file_name, $import_path);
            }
        )+
    };
}

resolving_import_cases! {
    flat_import_in_project_root_resolves: "", "file", "file";
    normal_nested_import_resolves: "subdir", "file", "subdir/file";
    directory_name_with_dots_resolves: "dir.with.dots", "file", "dir.with.dots/file";
    file_name_with_dots_resolves: "subdir", "file.with.dots", "subdir/file.with.dots";
}

// bug: typeck resolves an import through a symlink inside the project that points outside the project root | tier: High
#[cfg(unix)]
#[test]
fn import_via_symlink_escape_blocked() {
    // Layer 2 (canonicalize + starts_with): the segment check cannot see a symlink. Own test
    // because the case needs a symlink to a directory outside the project, not a path string.
    //   <proj>/main.ynz imports `escape_link/file`; <proj>/escape_link -> <elsewhere>;
    //   <elsewhere>/file.ynz exists, so only the boundary check can refuse it.
    let dir = make_temp_project("symlink_escape");
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
        "export function x() -> Int { return 1 }",
    )
    .expect("write outside file");
    std::os::unix::fs::symlink(&elsewhere, dir.join("escape_link")).expect("create symlink");

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
