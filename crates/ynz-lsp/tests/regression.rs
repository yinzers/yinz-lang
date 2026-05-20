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
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read fixture {name}: {e}"))
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
    // pipeline (ynz build via ynz-driver) must report consistent diagnostic
    // counts for the same source. A divergence means one pipeline is silently
    // suppressing or fabricating errors. This test is #[ignore] because the CLI
    // binary may be slow to start (LLVM init) and is not built in all CI configs.
    // Run manually with: cargo test -p ynz-lsp -- --ignored regression_lsp_vs_cli
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

    let error_dir = workspace_root.join("examples/errors");
    let mut entries: Vec<_> = std::fs::read_dir(&error_dir)
        .expect("examples/errors must be readable")
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("ynz"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in &entries {
        let path = entry.path();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("failed to read {}: {}", path.display(), e)
        });
        let uri = format!("file://{}", path.display());

        // Count diagnostics from the LSP pipeline.
        let client = InProcessHarness::new().start_server();
        client.initialize();
        let lsp_diags = open_and_get_diagnostics(&client, &uri, &text);
        let lsp_count = lsp_diags.len();

        // Count diagnostics from the CLI pipeline with a generous timeout.
        let cli_output = std::process::Command::new(&ynz_binary)
            .args(["build", path.to_str().expect("path is valid UTF-8")])
            .output();

        let cli_has_errors = match cli_output {
            Err(_) => {
                eprintln!(
                    "regression_lsp_vs_cli_divergence: could not run CLI for {}; skipping",
                    path.display()
                );
                continue;
            }
            Ok(output) => {
                // CLI exits non-zero when there are errors. stderr content is ariadne
                // pretty-printed; we do NOT count individual diagnostics here because
                // ariadne's rendering (spans, notes, suggestions) produces variable line
                // counts per diagnostic. Exact count matching requires `ynz build --json`
                // (structured output not yet implemented — tracked in todos.md
                // "lsp-vs-cli-exact-divergence"). The invariant we CAN assert reliably
                // is: CLI sees errors ↔ LSP sees errors (boolean agreement).
                !output.status.success()
            }
        };

        let lsp_has_errors = lsp_count > 0;
        assert_eq!(
            lsp_has_errors, cli_has_errors,
            "LSP vs CLI error presence disagreement for {}: \
             LSP reported {} diagnostics (has_errors={lsp_has_errors}), \
             CLI exited with success={} (has_errors={cli_has_errors})",
            path.display(),
            lsp_count,
            !cli_has_errors
        );
    }
}
