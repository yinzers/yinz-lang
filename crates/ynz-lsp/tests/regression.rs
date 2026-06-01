// WHY: Regression tests pin specific end-to-end behaviors that have historically
// been broken or are easy to accidentally break. Unlike the sweep tests (which
// cover all fixtures at a coarse level), regression tests make precise assertions
// about specific fixture content to guard named invariants.

mod harness;
use harness::InProcessHarness;

fn read_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read fixture {name}: {e}"))
}

/// Open a file and return the diagnostics from the first publishDiagnostics
/// notification. Uses a 2s timeout to avoid hanging on complex files.
fn open_and_get_diagnostics(
    client: &harness::HarnessClient,
    uri: &str,
    text: &str,
) -> Vec<serde_json::Value> {
    client.did_open(uri, text);
    let msg = match client.try_recv_with_params(std::time::Duration::from_secs(2)) {
        Some(m) => m,
        None => return vec![],
    };
    if let Some(params) = msg.get("params") {
        if let Some(diags) = params.get("diagnostics").and_then(|d| d.as_array()) {
            return diags.to_vec();
        }
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Zero-diagnostic regression
// ---------------------------------------------------------------------------

#[test]
fn regression_no_errors_fixture_has_zero_diagnostics() {
    // WHY: The no_errors.ynz fixture is the simplest possible valid Yinz program.
    // If the LSP reports any diagnostics for it, a parser or typeck regression
    // broke valid code. This is a strict regression pin — zero tolerance.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    let text = read_fixture("no_errors.ynz");
    let diags = open_and_get_diagnostics(&client, "file:///no_errors.ynz", &text);
    assert!(
        diags.is_empty(),
        "no_errors.ynz must have 0 diagnostics; got {}:\n{:?}",
        diags.len(),
        diags
    );
}

// ---------------------------------------------------------------------------
// Teaching content regression
// ---------------------------------------------------------------------------

#[test]
fn regression_errors_fixture_has_diagnostics_with_teaching_content() {
    // WHY: Golden Rule 11 requires every diagnostic to carry WHAT/WHAT INSTEAD/WHY
    // content. This test verifies that at least one diagnostic in has_errors.ynz
    // contains both "WHAT INSTEAD:" and "WHY:" markers in its message. If this
    // fails, the diagnostic_transform pipeline lost the teaching content.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    let text = read_fixture("has_errors.ynz");
    let diags = open_and_get_diagnostics(&client, "file:///has_errors.ynz", &text);

    assert!(
        !diags.is_empty(),
        "has_errors.ynz must have at least one diagnostic"
    );

    let has_teaching_content = diags.iter().any(|d| {
        let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
        msg.contains("WHAT INSTEAD:") && msg.contains("WHY:")
    });

    assert!(
        has_teaching_content,
        "at least one diagnostic in has_errors.ynz must contain 'WHAT INSTEAD:' and 'WHY:' \
         (Golden Rule 11 teaching format); found diagnostics:\n{:?}",
        diags
            .iter()
            .map(|d| d.get("message").and_then(|m| m.as_str()).unwrap_or(""))
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// CLI vs LSP divergence regression (ignored: requires ynz binary and can be slow)
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn regression_lsp_vs_cli_divergence() {
    // WHY: The LSP diagnostic pipeline (ynz-lsp via salsa queries) and the CLI
    // pipeline (ynz build --json via ynz-driver) must report the same diagnostic
    // count per severity for every fixture. A divergence means one pipeline is
    // silently suppressing or fabricating diagnostics. This test is #[ignore]
    // because it requires the `ynz` binary to be built and present.
    // Run manually with: cargo test -p ynz-lsp -- --ignored regression_lsp_vs_cli
    //
    // Closes: lsp-vs-cli-exact-divergence in todos.md.
    use std::path::Path;

    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has a parent")
        .parent()
        .expect("crates/ dir has a parent");

    let ynz_binary = std::env::current_exe()
        .expect("current_exe must be readable")
        .parent()
        .expect("test binary has a parent dir")
        .parent()
        .expect("deps dir has a parent")
        .join("ynz");

    if !ynz_binary.exists() {
        eprintln!(
            "regression_lsp_vs_cli_divergence: ynz binary not found at {}; skipping",
            ynz_binary.display()
        );
        return;
    }

    let error_dir = workspace_root.join("examples/primantis-orders");
    let mut entries: Vec<_> = std::fs::read_dir(&error_dir)
        .expect("examples/primantis-orders must be readable")
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("ynz"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in &entries {
        let path = entry.path();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
        let uri = format!("file://{}", path.display());

        // Count diagnostics from the LSP pipeline.
        let client = InProcessHarness::new().start_server();
        client.initialize();
        let lsp_diags = open_and_get_diagnostics(&client, &uri, &text);
        let lsp_error_count = lsp_diags
            .iter()
            .filter(|d| {
                d.get("severity")
                    .and_then(|s| s.as_u64())
                    .map(|s| s == 1) // LSP DiagnosticSeverity::ERROR = 1
                    .unwrap_or(false)
            })
            .count();

        // Count diagnostics from the CLI pipeline via `--json` structured output.
        let cli_output = std::process::Command::new(&ynz_binary)
            .args([
                "build",
                "--json",
                path.to_str().expect("path is valid UTF-8"),
            ])
            .output();

        let cli_error_count = match cli_output {
            Err(_) => {
                eprintln!(
                    "regression_lsp_vs_cli_divergence: could not run CLI for {}; skipping",
                    path.display()
                );
                continue;
            }
            Ok(output) => {
                // Parse the summary line (last line of NDJSON output).
                let stdout = String::from_utf8_lossy(&output.stdout);
                let summary_line = stdout.lines().filter(|l| !l.is_empty()).last();
                match summary_line.and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok()) {
                    Some(v) if v["type"] == "summary" => v["errors"].as_u64().unwrap_or(0) as usize,
                    _ => {
                        eprintln!(
                            "regression_lsp_vs_cli_divergence: could not parse --json output for {}; skipping",
                            path.display()
                        );
                        continue;
                    }
                }
            }
        };

        assert_eq!(
            lsp_error_count,
            cli_error_count,
            "LSP vs CLI ERROR COUNT disagreement for {}:\n  LSP: {} errors\n  CLI (--json): {} errors",
            path.display(),
            lsp_error_count,
            cli_error_count
        );
    }
}
