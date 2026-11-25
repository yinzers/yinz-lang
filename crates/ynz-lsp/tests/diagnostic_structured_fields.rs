// WHY: every LSP diagnostic emitted for a BannedKeyword error must have `code`
// and `data` populated. If either is absent, the code-action handler and
// `ynz build --json` tooling consumers lose the kind and structured WHY content
// they depend on to surface quick-fixes and machine-readable diagnostics.

mod harness;
use harness::InProcessHarness;
use serde_json::Value;

/// Opens `text` on a fresh server and returns the diagnostics of the first
/// notification. Panics when no publishDiagnostics arrives within the liveness
/// timeout (2 s, about 10x the observed latency).
fn diagnostics_published_on_open(uri: &str, text: &str) -> Vec<Value> {
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open(uri, text);
    let notification = client
        .try_recv_with_params(std::time::Duration::from_secs(2))
        .expect("didOpen must publish diagnostics");
    notification["params"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("notification carries no diagnostics array: {notification}"))
        .clone()
}

macro_rules! open_diagnostics_case {
    ($name:ident, $uri:expr, $source:expr, $expect_banned_keyword:expr) => {
        // bug: didOpen publishes a BannedKeyword diagnostic without its `code` and `data.what_instead` fields, or publishes diagnostics for a clean file | tier: Medium
        #[test]
        fn $name() {
            let diagnostics = diagnostics_published_on_open($uri, $source);
            if $expect_banned_keyword {
                let structured = diagnostics.iter().any(|diagnostic| {
                    diagnostic["code"] == "BannedKeyword"
                        && diagnostic["data"].get("what_instead").is_some()
                });
                assert!(
                    structured,
                    "a diagnostic must have code=BannedKeyword and data.what_instead; got: {diagnostics:?}"
                );
            } else {
                assert!(
                    diagnostics.is_empty(),
                    "a clean file must publish 0 diagnostics; got: {diagnostics:?}"
                );
            }
        }
    };
}

open_diagnostics_case!(
    banned_keyword_diagnostic_carries_code_and_what_instead_data,
    "file:///test_code.ynz",
    "class Foo { }\n",
    true
);
open_diagnostics_case!(
    clean_file_publishes_no_diagnostics,
    "file:///test_clean.ynz",
    "function entrypoint() -> Nothing { print(`ok`) }\n",
    false
);
