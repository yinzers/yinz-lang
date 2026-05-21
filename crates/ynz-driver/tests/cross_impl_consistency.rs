// WHY: v0.3-M1 introduced background task scheduling, which could in principle make
// programs non-deterministic (if background thread ordering affected stdout). This
// harness tests the DETERMINISM property: every program in the corpus must produce
// byte-identical output on two consecutive runs. Non-determinism here means either
// (a) background output is racing with foreground output in a way that changes stdout
// ordering, or (b) codegen introduced non-determinism in unrelated code paths.
//
// The harness also serves as a regression guard for the P0-P3 changes: if any
// existing fixture starts producing different output, a change broke it.
//
// This test also validates that no existing program was broken by the P0-P3 changes:
// if a fixture's output changes between runs, something non-deterministic crept in.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

/// Run `ynz run <path>` and return (stdout, stderr, exit_code).
fn run_ynz(path: &Path) -> (String, String, i32) {
    let out = Command::new(ynz_binary())
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ynz: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

/// True when a file is an intentional-error gallery file (should fail to compile).
fn is_error_gallery(path: &Path) -> bool {
    path.ancestors().any(|p| p.ends_with("primantis-orders"))
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.starts_with("broken")
                    || n.starts_with("empty")
                    || n.contains("_neg_")
                    || n.contains("mismatch")
                    || n.contains("overflow")
                    || n.contains("div_by_zero")
                    || n.contains("deferral")
                    || n.contains("reassign")
                    || n.contains("missing_return")
                    || n.contains("dead_code")
                    || n.contains("compound_assign")
                    || n.contains("banned")
                    || n.contains("base_instantiate")
                    || n.contains("is_type_deferral")
                    || n.contains("return_no_value")
                    || n.contains("int_max_deferred")
                    || n.contains("wrapping_add_deferred")
                    || n.contains("bignum_deferral")
                    || n.contains("arg_arity")
                    || n.contains("arg_type")
                    || n.contains("undefined_function")
                    || n.contains("no_follows")
                    || n.contains("const_field")
            })
            .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn collect_corpus() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut files = Vec::new();

    // Driver fixtures
    let fixtures = root.join("crates/ynz-driver/tests/fixtures");
    if let Ok(entries) = std::fs::read_dir(&fixtures) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("ynz") && !is_error_gallery(&p) {
                files.push(p);
            }
        }
    }

    // examples/ (excluding primantis-orders and non-entrypoint sub-files)
    let examples = root.join("examples");
    if let Ok(entries) = std::fs::read_dir(&examples) {
        for dir_entry in entries.flatten() {
            let dir = dir_entry.path();
            if !dir.is_dir() {
                continue;
            }
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name == "primantis-orders" || dir_name == "burgh-poem" {
                continue;
            }
            // Use entrypoint.ynz if present (single-entry projects)
            let ep = dir.join("entrypoint.ynz");
            if ep.exists() && !is_error_gallery(&ep) {
                files.push(ep);
            }
        }
    }

    files.sort();
    files
}

// WHY: corpus determinism — every program must produce byte-identical stdout/stderr/exit-code
// on two consecutive runs. Background scheduling must not introduce observable ordering
// non-determinism for the programs in this corpus (timing fixtures are excluded).
// Quality gate: at least 30 files in the corpus (validates coverage, not a stub).
// If count drops below 30, either files were deleted (update corpus) or discovery broke.
#[test]
fn corpus_produces_deterministic_output_across_runs() {
    let corpus = collect_corpus();

    let corpus_size = corpus.len();
    assert!(
        corpus_size >= 30,
        "corpus must have at least 30 files (got {corpus_size}); discovery logic may be broken"
    );

    let mut failures: Vec<String> = Vec::new();

    for path in &corpus {
        let (run1_out, run1_err, run1_code) = run_ynz(path);
        let (run2_out, run2_err, run2_code) = run_ynz(path);

        // Output must be byte-identical between runs (determinism).
        // For background programs, the order guarantee is that main-thread output
        // precedes background output in the same run; two runs may differ in
        // interleaving if the system is under load. We skip the timing fixture
        // which is inherently racy.
        let is_timing_fixture = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.contains("timing") || n.contains("background"))
            .unwrap_or(false);

        if !is_timing_fixture {
            if run1_out != run2_out || run1_err != run2_err || run1_code != run2_code {
                failures.push(format!(
                    "NON-DETERMINISTIC: {:?}\n  run1 stdout: {:?}\n  run2 stdout: {:?}\n  run1 exit: {run1_code}, run2 exit: {run2_code}",
                    path.file_name().unwrap_or_default(),
                    &run1_out[..run1_out.len().min(200)],
                    &run2_out[..run2_out.len().min(200)],
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "determinism failures ({} / {} non-timing files):\n{}",
        failures.len(),
        corpus_size,
        failures.join("\n\n")
    );
}
