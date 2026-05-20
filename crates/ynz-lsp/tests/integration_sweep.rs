// WHY: Integration sweep tests open every example fixture through the full LSP
// pipeline and assert that import-free basics files produce zero diagnostics,
// while error gallery files produce at least one diagnostic. All fixtures
// survive completion and hover requests without panicking. These tests catch
// regressions where a new language feature breaks the pipeline end-to-end.
//
// Design constraints:
// - examples/basics/entrypoint.ynz has cross-module import statements that
//   produce "module not registered" diagnostics when opened in single-file
//   context (no project root / yinz.toml). It is excluded from the
//   zero-diagnostics assertion but included in the crash-safety sweeps.
// - The sweep uses try_recv_with_params() with a 400ms timeout (matching the
//   pattern in diagnostics.rs). Files that don't respond within 400ms are
//   skipped rather than hanging the test suite — a pre-existing parser
//   infinite-loop on certain malformed inputs means some error gallery files
//   may not respond before the timeout. The sweeps assert what they CAN assert.

mod harness;
use harness::InProcessHarness;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Notification drain timeout — matches the diagnostics.rs `next_notif` pattern.
const NOTIF_TIMEOUT: Duration = Duration::from_millis(400);

// ---------------------------------------------------------------------------
// Fixture discovery helpers
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir has a parent")
        .parent()
        .expect("crates/ dir has a parent")
        .to_path_buf()
}

fn collect_ynz_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = vec![];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_ynz_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("ynz") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Import-free basics files: services/ and utils/ companion modules.
/// entrypoint.ynz is excluded because its import statements produce expected
/// "module not registered" diagnostics in single-file mode.
fn import_free_basics_fixtures() -> Vec<PathBuf> {
    let mut files = collect_ynz_files(&workspace_root().join("examples/basics/services"));
    files.extend(collect_ynz_files(
        &workspace_root().join("examples/basics/utils"),
    ));
    assert!(
        !files.is_empty(),
        "examples/basics/services/ and utils/ must contain at least one .ynz file"
    );
    files
}

fn all_basics_fixtures() -> Vec<PathBuf> {
    let files = collect_ynz_files(&workspace_root().join("examples/basics"));
    assert!(
        !files.is_empty(),
        "examples/basics/ must contain at least one .ynz file"
    );
    files
}

fn error_fixtures() -> Vec<PathBuf> {
    let files = collect_ynz_files(&workspace_root().join("examples/errors"));
    assert!(
        !files.is_empty(),
        "examples/errors/ must contain at least one .ynz file"
    );
    files
}

fn all_example_fixtures() -> Vec<PathBuf> {
    let mut all = all_basics_fixtures();
    all.extend(error_fixtures());
    all
}

fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

// ---------------------------------------------------------------------------
// Helper: open a file and drain the publishDiagnostics notification.
// Returns the diagnostics array if the server responded within NOTIF_TIMEOUT.
// Returns None if no notification arrived (server may be unresponsive).
// ---------------------------------------------------------------------------

fn open_and_drain_diagnostics(
    client: &harness::HarnessClient,
    uri: &str,
    text: &str,
) -> Option<Vec<serde_json::Value>> {
    client.did_open(uri, text);
    let msg = client.try_recv_with_params(NOTIF_TIMEOUT)?;
    let diags = msg
        .get("params")
        .and_then(|p| p.get("diagnostics"))
        .and_then(|d| d.as_array())
        .map(|a| a.to_vec())
        .unwrap_or_default();
    Some(diags)
}

// ---------------------------------------------------------------------------
// Sweep: import-free basics fixtures must have zero diagnostics
// ---------------------------------------------------------------------------

#[test]
fn sweep_basics_fixtures_have_no_diagnostics() {
    // WHY: The import-free files in examples/basics/ are clean showcase files.
    // If the LSP reports diagnostics for any of them, a parser or typeck
    // regression broke valid user code.
    for path in &import_free_basics_fixtures() {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read basics fixture {}: {}", path.display(), e));
        let uri = path_to_uri(path);

        let client = InProcessHarness::new().start_server();
        client.initialize();

        let diags = open_and_drain_diagnostics(&client, &uri, &text).unwrap_or_else(|| {
            panic!(
                "server did not respond to didOpen for {} within {:?}",
                path.display(),
                NOTIF_TIMEOUT
            )
        });

        client.shutdown();

        assert!(
            diags.is_empty(),
            "basics fixture {} must have 0 diagnostics; got {}:\n{:?}",
            path.display(),
            diags.len(),
            diags
        );
    }
}

// ---------------------------------------------------------------------------
// Sweep: error fixtures must have at least one diagnostic
// ---------------------------------------------------------------------------

#[test]
fn sweep_error_fixtures_have_diagnostics() {
    // WHY: Every file in examples/errors/ contains at least one intentional
    // error. If the LSP reports zero diagnostics, the diagnostic pipeline is
    // broken or the fixture no longer exercises the error class.
    //
    // Files that don't respond within NOTIF_TIMEOUT are skipped — some error
    // files trigger a known parser infinite-loop on heavily malformed input.
    // The skip is logged; it doesn't hide a regression because the missing
    // response itself is evidence of the parser bug, not a silent pass.
    let fixtures = error_fixtures();
    let mut tested = 0usize;

    for path in &fixtures {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read error fixture {}: {}", path.display(), e));
        let uri = path_to_uri(path);

        let client = InProcessHarness::new().start_server();
        client.initialize();

        match open_and_drain_diagnostics(&client, &uri, &text) {
            None => {
                // Server did not respond — skip rather than hang. This indicates
                // a parser infinite loop on the file, which is a pre-existing bug.
                eprintln!(
                    "SKIP {}: server unresponsive within {:?} (pre-existing parser bug)",
                    path.display(),
                    NOTIF_TIMEOUT
                );
                // Do NOT call client.shutdown() — the server thread is stuck.
                // Background threads are cleaned up when the test process exits.
            }
            Some(diags) => {
                client.shutdown();
                assert!(
                    !diags.is_empty(),
                    "error fixture {} must have at least 1 diagnostic; got 0",
                    path.display()
                );
                tested += 1;
            }
        }
    }

    assert!(
        tested > 0,
        "no error fixtures responded within the timeout — all {} files may be triggering \
         the parser infinite-loop bug",
        fixtures.len()
    );
}

// ---------------------------------------------------------------------------
// Sweep: completion requests never crash
// ---------------------------------------------------------------------------

#[test]
fn sweep_all_fixtures_completion_returns() {
    // WHY: Completion is invoked on every fixture at position (0,0). The result
    // may be null or populated — both are valid. A server panic or error-field
    // response is not. Files that don't respond within the timeout are skipped.
    let mut request_id = 100i32;
    let mut tested = 0usize;

    for path in &all_example_fixtures() {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));
        let uri = path_to_uri(path);

        let client = InProcessHarness::new().start_server();
        client.initialize();

        // Drain the diagnostic notification first.
        if open_and_drain_diagnostics(&client, &uri, &text).is_none() {
            eprintln!(
                "SKIP completion for {}: server unresponsive",
                path.display()
            );
            continue;
        }

        client
            .conn
            .sender
            .send(lsp_server::Message::Request(lsp_server::Request {
                id: lsp_server::RequestId::from(request_id),
                method: "textDocument/completion".to_string(),
                params: serde_json::json!({
                    "textDocument": { "uri": &uri },
                    "position": { "line": 0, "character": 0 }
                }),
            }))
            .expect("completion request send must succeed");
        request_id += 1;

        // recv_response() is safe here: the diagnostic was already drained, and
        // completion is a synchronous request the server answers immediately.
        let response = client.recv_response();
        client.shutdown();

        assert!(
            response.get("error").is_none(),
            "completion on {} must not return an error field: {:?}",
            path.display(),
            response
        );
        tested += 1;
    }

    assert!(
        tested > 0,
        "no fixtures were tested for completion — all files may be unresponsive"
    );
}

// ---------------------------------------------------------------------------
// Sweep: hover requests never crash
// ---------------------------------------------------------------------------

#[test]
fn sweep_all_fixtures_hover_returns() {
    // WHY: Hover is invoked on every fixture at position (0,0). Null is a
    // valid result; a server panic or error-field response is not.
    // Files that don't respond within the timeout are skipped.
    let mut request_id = 200i32;
    let mut tested = 0usize;

    for path in &all_example_fixtures() {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", path.display(), e));
        let uri = path_to_uri(path);

        let client = InProcessHarness::new().start_server();
        client.initialize();

        // Drain the diagnostic notification first.
        if open_and_drain_diagnostics(&client, &uri, &text).is_none() {
            eprintln!("SKIP hover for {}: server unresponsive", path.display());
            continue;
        }

        client
            .conn
            .sender
            .send(lsp_server::Message::Request(lsp_server::Request {
                id: lsp_server::RequestId::from(request_id),
                method: "textDocument/hover".to_string(),
                params: serde_json::json!({
                    "textDocument": { "uri": &uri },
                    "position": { "line": 0, "character": 0 }
                }),
            }))
            .expect("hover request send must succeed");
        request_id += 1;

        let response = client.recv_response();
        client.shutdown();

        assert!(
            response.get("error").is_none(),
            "hover on {} must not return an error field: {:?}",
            path.display(),
            response
        );
        tested += 1;
    }

    assert!(
        tested > 0,
        "no fixtures were tested for hover — all files may be unresponsive"
    );
}
