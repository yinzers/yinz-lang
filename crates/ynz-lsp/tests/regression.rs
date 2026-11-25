// WHY: The LSP diagnostic pipeline (ynz-lsp via salsa queries) and the CLI
// pipeline (`ynz build --json` via ynz-driver) must report the same error
// count for every error-gallery example. A divergence means one pipeline is
// silently suppressing or fabricating diagnostics.

mod harness;
use harness::InProcessHarness;

/// Error diagnostics (LSP severity 1) published by the first notification after
/// didOpen. Panics when none arrives within the liveness timeout.
fn lsp_error_count(uri: &str, text: &str) -> usize {
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open(uri, text);
    let notification = client
        .try_recv_with_params(std::time::Duration::from_secs(5))
        .unwrap_or_else(|| panic!("no publishDiagnostics for {uri}"));
    notification["params"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("no diagnostics array for {uri}: {notification}"))
        .iter()
        .filter(|diagnostic| diagnostic["severity"] == 1)
        .count()
}

/// Error count from the summary line of `ynz build --json <path>`.
fn cli_error_count(ynz_binary: &std::path::Path, path: &std::path::Path) -> usize {
    let output = std::process::Command::new(ynz_binary)
        .args([
            "build",
            "--json",
            path.to_str().expect("path is valid UTF-8"),
        ])
        .output()
        .unwrap_or_else(|err| panic!("could not run {}: {err}", ynz_binary.display()));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary_line = stdout
        .lines()
        .rfind(|line| !line.is_empty())
        .unwrap_or_else(|| panic!("no --json output for {}", path.display()));
    let summary: serde_json::Value = serde_json::from_str(summary_line)
        .unwrap_or_else(|err| panic!("unparseable summary for {}: {err}", path.display()));
    assert_eq!(summary["type"], "summary", "last line is the summary");
    summary["errors"]
        .as_u64()
        .expect("summary has an error count") as usize
}

// bug: the language server reports a different error count than `ynz build --json` for an error-gallery example | tier: Medium
#[test]
fn regression_lsp_vs_cli_divergence() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates_dir| crates_dir.parent())
        .expect("crates/ynz-lsp has a workspace root two levels up");
    // `ynz` belongs to ynz-driver, so cargo gives this test no CARGO_BIN_EXE for it. It is built
    // into the same folder as `ynz-lsp`, whose path cargo does give (valid whatever the layout
    // of the test executables, and after `nextest archive` + `--workspace-remap`, which restores
    // the workspace's `target/<profile>/` folder). The test executable's own path is not usable:
    // under the archive it sits in a build folder, not beside the binaries.
    let ynz_binary = std::path::Path::new(env!("CARGO_BIN_EXE_ynz-lsp")).with_file_name("ynz");
    assert!(
        ynz_binary.exists(),
        "ynz binary not found at {}; run `cargo build -p ynz-driver` first",
        ynz_binary.display()
    );

    let mut example_paths: Vec<_> =
        std::fs::read_dir(workspace_root.join("examples/primantis-orders"))
            .expect("examples/primantis-orders must be readable")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("ynz"))
            .collect();
    example_paths.sort();

    let disagreements: Vec<String> = example_paths
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            let lsp_errors = lsp_error_count(&format!("file://{}", path.display()), &text);
            let cli_errors = cli_error_count(&ynz_binary, path);
            (lsp_errors != cli_errors).then(|| {
                format!(
                    "{}: LSP {lsp_errors} errors, CLI {cli_errors} errors",
                    path.display()
                )
            })
        })
        .collect();
    assert!(
        disagreements.is_empty(),
        "LSP and CLI disagree:\n{}",
        disagreements.join("\n")
    );
}
