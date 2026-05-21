// WHY: Phase 9 structured fields test — every LSP diagnostic emitted for a
// BannedKeyword error must have `code`, `data` populated. If either is absent,
// the code-action handler (Phase 7) and tooling consumers (Phase 9 --json) lose
// the kind and structured WHY content they depend on.

mod harness;
use harness::InProcessHarness;

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

#[test]
fn banned_keyword_diagnostic_has_code_field() {
    // WHY: the `code` field drives the code-action handler — if it's absent,
    // quick-fix lightbulbs never appear for banned-keyword diagnostics.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    let src = "class Foo { }\n";
    let diags = open_and_get_diagnostics(&client, "file:///test_code.ynz", src);
    assert!(!diags.is_empty(), "class Foo must produce at least one diagnostic");
    let has_code = diags.iter().any(|d| {
        d.get("code")
            .map(|c| c.as_str().unwrap_or("") == "BannedKeyword")
            .unwrap_or(false)
    });
    assert!(
        has_code,
        "at least one diagnostic must have code='BannedKeyword'; got: {:?}",
        diags
    );
}

#[test]
fn banned_keyword_diagnostic_has_data_field() {
    // WHY: the `data.what_instead` field enables custom UI rendering without
    // re-parsing `message`. If absent, consumers must fall back to regex on
    // the human-readable text — fragile against message wording changes.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    let src = "class Foo { }\n";
    let diags = open_and_get_diagnostics(&client, "file:///test_data.ynz", src);
    assert!(!diags.is_empty(), "class Foo must produce at least one diagnostic");
    let has_data = diags.iter().any(|d| {
        d.get("data")
            .and_then(|data| data.get("what_instead"))
            .is_some()
    });
    assert!(
        has_data,
        "at least one diagnostic must have data.what_instead; got: {:?}",
        diags
    );
}

#[test]
fn clean_file_has_no_diagnostic_code() {
    // WHY: a clean file must emit zero diagnostics. If the code/data fields
    // accidentally create diagnostics for clean code, this is a regression.
    let client = InProcessHarness::new().start_server();
    client.initialize();
    let src = "function entrypoint() -> nothing { print(`ok`) }\n";
    let diags = open_and_get_diagnostics(&client, "file:///test_clean.ynz", src);
    assert!(
        diags.is_empty(),
        "clean file must have 0 diagnostics after adding structured fields; got: {:?}",
        diags
    );
}
