// WHY: `ynz build --json` tests verify the NDJSON output schema is correct
// and that the exit code agrees with the process exit code. Bugs here break
// CI scripts that pipe `ynz build --json | jq '.exit_code'` and every tooling
// consumer that relies on structured diagnostics instead of regex-parsing ariadne.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn run_build_json(source_path: &Path) -> Output {
    Command::new(ynz_binary())
        .args([
            "build",
            "--json",
            source_path.to_str().expect("path is UTF-8"),
        ])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture(name: &str) -> PathBuf {
    fixtures_dir().join(name)
}

/// Parse every line of `stdout` as a JSON object; panic on any malformed line.
fn parse_ndjson(stdout: &str) -> Vec<serde_json::Value> {
    stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| panic!("invalid JSON line: {e}\nline: {l}"))
        })
        .collect()
}

// ─── Clean file: zero diagnostics, one summary line ──────────────────────────

// bug: `ynz build --json` on a clean file emits a diagnostic line, or a summary with errors or a non-zero exit code | tier: Medium
#[test]
fn build_json_clean_file_emits_only_summary() {
    let out = run_build_json(&fixture("hello.ynz"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let events = parse_ndjson(&stdout);
    assert_eq!(
        events.len(),
        1,
        "expected exactly 1 line (summary) for clean file; got:\n{stdout}"
    );
    let summary = &events[0];
    assert_eq!(summary["type"], "summary");
    assert_eq!(summary["errors"], 0);
    assert_eq!(summary["exit_code"], 0);
    assert_eq!(out.status.code(), Some(0), "clean file must exit 0");
}

// ─── File with errors: diagnostic lines + summary ────────────────────────────

// bug: `ynz build --json` on a broken file emits a malformed or null-bearing line, omits a required field, or its exit code disagrees with `summary.exit_code` | tier: Medium
#[test]
fn build_json_error_file_emits_diagnostic_and_summary() {
    let out = run_build_json(&fixture("broken_main.ynz"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // parse_ndjson panics on any line that is not one JSON object (embedded newlines escaped).
    let events = parse_ndjson(&stdout);
    assert!(
        events.len() >= 2,
        "expected at least 1 diagnostic + 1 summary; got {} events:\n{stdout}",
        events.len()
    );
    assert!(
        !stdout.contains(":null"),
        "absent optional fields must be omitted, not null; got:\n{stdout}"
    );
    for event in &events {
        assert!(
            event.get("schema_version").is_some(),
            "every event must have schema_version; got: {event}"
        );
        if event["type"] == "diagnostic" {
            for field in ["severity", "span", "message", "data"] {
                assert!(event.get(field).is_some(), "diagnostic missing {field}");
            }
            for field in ["what", "what_instead", "why"] {
                assert!(
                    event["data"].get(field).is_some(),
                    "diagnostic data.{field} missing"
                );
            }
        }
    }
    let summary = &events[events.len() - 1];
    assert_eq!(summary["type"], "summary", "last event must be summary");
    assert!(
        summary["errors"].as_u64().unwrap_or(0) > 0,
        "broken_main.ynz must have at least 1 error in summary"
    );
    let summary_exit = summary["exit_code"].as_i64().unwrap_or(-1) as i32;
    assert_eq!(
        out.status.code(),
        Some(summary_exit),
        "process exit code must match summary.exit_code"
    );
    assert_ne!(out.status.code(), Some(0), "error file must exit non-zero");
}

// ─── Default build output unchanged ──────────────────────────────────────────

// bug: plain `ynz build` (no --json) prints NDJSON, breaking scripts that parse its human-readable output | tier: Medium
#[test]
fn build_without_json_flag_has_no_json_output() {
    let out = Command::new(ynz_binary())
        .args([
            "build",
            fixture("hello.ynz").to_str().expect("path is UTF-8"),
        ])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("schema_version"),
        "default build output must not contain NDJSON; got:\n{stdout}"
    );
}
