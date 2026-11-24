mod harness;
use harness::{HarnessClient, InProcessHarness};
use serde_json::Value;
use std::time::Duration;

const BANNED_KEYWORD_SOURCE: &str = "class Foo { }\n";
const CLEAN_SOURCE: &str = "function entrypoint() -> Nothing { print(`ok`) }\n";

/// Next publishDiagnostics notification's diagnostics; panics when none
/// arrives within the liveness timeout (2 s, about 10x the observed latency).
fn next_published_diagnostics(client: &HarnessClient) -> Vec<Value> {
    let notification = client
        .try_recv_with_params(Duration::from_secs(2))
        .expect("expected a publishDiagnostics notification");
    notification["params"]["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("notification carries no diagnostics array: {notification}"))
        .clone()
}

fn is_banned_keyword(diagnostic: &Value) -> bool {
    diagnostic["code"] == "BannedKeyword"
}

// bug: didChange publishes diagnostics for a stale text, or drops one of two back-to-back edits | tier: Medium
#[test]
fn did_change_publishes_diagnostics_for_each_edit_of_the_latest_text() {
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open("file:///edit.ynz", CLEAN_SOURCE);
    assert!(next_published_diagnostics(&client).is_empty());

    client.did_change("file:///edit.ynz", BANNED_KEYWORD_SOURCE, 2);
    client.did_change("file:///edit.ynz", CLEAN_SOURCE, 3);

    let after_first_edit = next_published_diagnostics(&client);
    assert!(
        after_first_edit.iter().any(is_banned_keyword),
        "edit 2 introduced `class`; got: {after_first_edit:?}"
    );
    let after_second_edit = next_published_diagnostics(&client);
    assert!(
        after_second_edit.is_empty(),
        "edit 3 removed the error; got: {after_second_edit:?}"
    );
}

// bug: didClose leaves the closed file's diagnostics showing in the editor | tier: Medium
#[test]
fn did_close_publishes_empty_diagnostics_for_the_closed_file() {
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open("file:///close.ynz", BANNED_KEYWORD_SOURCE);
    assert!(!next_published_diagnostics(&client).is_empty());

    client.did_close("file:///close.ynz");

    let after_close = next_published_diagnostics(&client);
    assert!(
        after_close.is_empty(),
        "closing the file must clear its diagnostics; got: {after_close:?}"
    );
}
