// WHY: opens every import-free example program through the full LSP pipeline
// and asserts it publishes zero diagnostics. A parser or typeck regression that
// breaks valid user code surfaces here end to end.
//
// Excluded: examples/pirates-roster/entrypoint.ynz, whose import statements
// produce "module not registered" diagnostics in single-file context (no
// project root / yinz.toml).

mod harness;
use harness::InProcessHarness;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Liveness bound for the first publishDiagnostics after didOpen (about 10x observed).
const NOTIF_TIMEOUT: Duration = Duration::from_secs(2);

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates_dir| crates_dir.parent())
        .expect("crates/ynz-lsp has a workspace root two levels up")
        .to_path_buf()
}

fn ynz_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("{} must be readable: {err}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("ynz"))
        .collect();
    files.sort();
    files
}

// bug: the language server publishes a diagnostic for a valid, import-free example program | tier: Medium
#[test]
fn import_free_example_programs_publish_no_diagnostics() {
    let examples = workspace_root().join("examples/pirates-roster");
    let mut programs = ynz_files_in(&examples.join("services"));
    programs.extend(ynz_files_in(&examples.join("utils")));
    assert!(
        !programs.is_empty(),
        "the example folders hold no .ynz file"
    );

    let failures: Vec<String> = programs
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            let client = InProcessHarness::new().start_server();
            client.initialize();
            client.did_open(&format!("file://{}", path.display()), &text);
            let notification = client
                .try_recv_with_params(NOTIF_TIMEOUT)
                .unwrap_or_else(|| panic!("no publishDiagnostics for {}", path.display()));
            let diagnostics = &notification["params"]["diagnostics"];
            (diagnostics.as_array().map(Vec::len) != Some(0))
                .then(|| format!("{}: {diagnostics}", path.display()))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "diagnostics on valid programs:\n{}",
        failures.join("\n")
    );
}
