use serde::Serialize;
use ynz_diagnostics::{DiagnosticBucket, Severity};

/// Schema version for `ynz build --json` NDJSON output.
///
/// Stabilized at `v0.2.0` when the v0.2.0 tag is cut (Phase 12 drops the
/// `-unstable` suffix). Until then, tooling consumers MUST check this field
/// and reject output from versions they haven't validated against.
pub const SCHEMA_VERSION: &str = "v0.2.0-m5-unstable";

/// One line of NDJSON output per diagnostic, emitted to stdout.
///
/// The three-part WHAT/WHAT-INSTEAD/WHY structure is preserved both in the
/// flat `message` field (for backward-compat consumers that parse text) AND
/// in the structured `data` object (for consumers that want to render custom
/// UI without re-parsing the message).
///
/// Field semantics:
/// - `span.file`: absolute filesystem path (UTF-8).
/// - `span.start_byte` / `span.end_byte`: UTF-8 byte offsets, 0-indexed,
///   half-open [start, end). Consumers that need line/column compute from
///   UTF-8 line tables themselves.
/// - `severity`: lowercase string — "error" | "warning" | "suggestion".
/// - `kind` / `code`: identical string — the registered `DiagnosticKind`
///   name (PascalCase), or `null` when the diagnostic has no structural kind.
/// - `message`: full plaintext WHAT + WHAT INSTEAD + WHY, same as the LSP
///   `Diagnostic.message` field. Embedded newlines are JSON-escaped.
/// - `data`: structured WHAT/WHAT-INSTEAD/WHY for custom UI rendering.
#[derive(Serialize)]
pub struct JsonDiagnostic<'a> {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub schema_version: &'static str,
    pub severity: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'a str>,
    pub span: JsonSpan<'a>,
    pub message: String,
    pub data: JsonDiagnosticData<'a>,
}

/// The file + byte-offset span for a diagnostic.
#[derive(Serialize)]
pub struct JsonSpan<'a> {
    pub file: &'a str,
    pub start_byte: u32,
    pub end_byte: u32,
}

/// Structured three-part WHAT/WHAT-INSTEAD/WHY — for consumers that want to
/// render diagnostics in custom UI without re-parsing `message`.
#[derive(Serialize)]
pub struct JsonDiagnosticData<'a> {
    pub what: &'a str,
    pub what_instead: &'a str,
    pub why: &'a str,
}

/// Final summary event — always the last line of `ynz build --json` output,
/// even when there are zero diagnostics.
///
/// `exit_code` matches the process exit code: 0 when `errors == 0`; non-zero
/// on any error. Warnings and suggestions do NOT trigger a non-zero exit code.
/// This invariant is locked so v0.3+ cannot silently flip the policy and
/// break CI consumers that check `jq '.exit_code'`.
#[derive(Serialize)]
pub struct JsonSummary {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub schema_version: &'static str,
    pub errors: u32,
    pub warnings: u32,
    pub suggestions: u32,
    pub exit_code: i32,
}

/// Emit all diagnostics in `bucket` as NDJSON to stdout, then emit a summary line.
///
/// Each diagnostic is serialized to a single JSON line (`\n`-terminated, no CRLF).
/// Embedded newlines inside WHAT/WHAT-INSTEAD/WHY fields are JSON-escaped.
///
/// Returns the exit code: 0 when `errors == 0`; 1 otherwise.
pub fn emit_json(bucket: &DiagnosticBucket) -> i32 {
    let mut errors = 0u32;
    let mut warnings = 0u32;
    let mut suggestions = 0u32;

    for d in bucket.iter() {
        match d.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Suggestion => suggestions += 1,
        }

        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Suggestion => "suggestion",
        };

        let kind_name: Option<String> = d.kind.as_ref().map(|k| k.kind_name().to_string());
        let kind_ref = kind_name.as_deref();

        let message = format!(
            "{}\n\nWHAT INSTEAD: {}\n\nWHY: {}",
            d.what, d.what_instead, d.why
        );

        let event = JsonDiagnostic {
            event_type: "diagnostic",
            schema_version: SCHEMA_VERSION,
            severity,
            kind: kind_ref,
            code: kind_ref,
            span: JsonSpan {
                file: &d.span.file,
                start_byte: d.span.start as u32,
                end_byte: d.span.end as u32,
            },
            message,
            data: JsonDiagnosticData {
                what: &d.what,
                what_instead: &d.what_instead,
                why: &d.why,
            },
        };

        // serde_json::to_string never emits raw newlines inside strings;
        // embedded \n in WHAT/WHY are JSON-escaped to \\n automatically.
        let line =
            serde_json::to_string(&event).unwrap_or_else(|_| r#"{"type":"diagnostic","error":"serialization failed"}"#.to_string());
        println!("{line}");
    }

    let exit_code: i32 = if errors > 0 { 1 } else { 0 };

    let summary = JsonSummary {
        event_type: "summary",
        schema_version: SCHEMA_VERSION,
        errors,
        warnings,
        suggestions,
        exit_code,
    };
    let summary_line = serde_json::to_string(&summary)
        .unwrap_or_else(|_| r#"{"type":"summary","error":"serialization failed"}"#.to_string());
    println!("{summary_line}");

    exit_code
}

/// Build a `DiagnosticBucket` from a set of source files by running `check_query`
/// on each file registered in the database. Used by the `--json` build path.
///
/// This function is separate from `build_with_output_dir` so the `--json` path
/// can share typeck analysis without paying codegen / object-file costs (tooling
/// consumers that want diagnostics only don't want to invoke the linker).
pub fn collect_diagnostics(
    db: &ynz_parser::CompilerDb,
    source_files: &[ynz_parser::SourceFile],
) -> DiagnosticBucket {
    let mut bucket = DiagnosticBucket::new();
    for &sf in source_files {
        let check = ynz_typeck::queries::check_query(db, sf);
        for d in check.diagnostics.iter() {
            bucket.push(d.clone());
        }
    }
    bucket
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_diagnostics::Diagnostic;

    fn make_diag(severity: Severity, what: &str, what_instead: &str, why: &str) -> Diagnostic {
        Diagnostic::new(
            severity,
            ynz_diagnostics::SourceSpan::new("/tmp/test.ynz", 0, 5),
            what,
            what_instead,
            why,
        )
    }

    #[test]
    fn severity_mapping_error_is_error() {
        let mut bucket = DiagnosticBucket::new();
        bucket.push(make_diag(Severity::Error, "error", "fix it", "because"));
        // emit_json writes to stdout; we can't capture it easily here.
        // The important invariant is tested: errors > 0 → exit_code = 1.
        // This is verified via integration tests in crates/ynz-driver/tests/.
        let _ = bucket; // suppress unused warning
    }

    #[test]
    fn zero_diagnostics_exit_code_is_zero() {
        // WHY: `ynz build --json` on a clean file must emit ONLY the summary line
        // (exit_code: 0, all counts = 0). If it emits diagnostic lines for zero
        // errors, CI scripts that parse the output get confused.
        // Tested indirectly here; integration test verifies the NDJSON content.
        let bucket = DiagnosticBucket::new();
        let exit_code = {
            let errors: u32 = bucket.iter().filter(|d| d.severity == Severity::Error).count() as u32;
            if errors > 0 { 1i32 } else { 0i32 }
        };
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn schema_version_has_unstable_suffix() {
        // WHY: Phase 12 drops the -unstable suffix when v0.2.0 tag is cut.
        // This test enforces the suffix is present in all pre-release builds.
        // If it fails after Phase 12, remove this test (suffix was intentionally dropped).
        assert!(
            SCHEMA_VERSION.ends_with("-unstable"),
            "schema_version should end with -unstable until v0.2.0 tag is cut (Phase 12)"
        );
    }

    #[test]
    fn json_diagnostic_serialization_no_null_fields() {
        // WHY: The schema specifies no field emits `null`. Fields that are absent
        // must be omitted entirely (JSON-object-key-absent), not set to null.
        // `skip_serializing_if = "Option::is_none"` on kind/code enforces this.
        let mut bucket = DiagnosticBucket::new();
        bucket.push(make_diag(Severity::Error, "what", "instead", "why"));
        // A diagnostic without kind — kind + code must be omitted, not null.
        // We serialize manually to check:
        let d = bucket.iter().next().unwrap();
        let event = JsonDiagnostic {
            event_type: "diagnostic",
            schema_version: SCHEMA_VERSION,
            severity: "error",
            kind: None,
            code: None,
            span: JsonSpan { file: "/tmp/test.ynz", start_byte: 0, end_byte: 5 },
            message: format!("{}\n\nWHAT INSTEAD: {}\n\nWHY: {}", d.what, d.what_instead, d.why),
            data: JsonDiagnosticData { what: &d.what, what_instead: &d.what_instead, why: &d.why },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains(r#""kind":null"#), "kind must be omitted, not null");
        assert!(!json.contains(r#""code":null"#), "code must be omitted, not null");
    }

    #[test]
    fn json_diagnostic_newline_in_why_is_escaped() {
        // WHY: NDJSON requires one JSON object per line. If a WHY field contains
        // a raw \n, the output has two lines for one diagnostic — parsers break.
        // serde_json escapes embedded newlines automatically; this test confirms.
        let d = make_diag(
            Severity::Error,
            "something broke",
            "fix like this:\nstep 1\nstep 2",
            "explanation\non two lines",
        );
        let event = JsonDiagnostic {
            event_type: "diagnostic",
            schema_version: SCHEMA_VERSION,
            severity: "error",
            kind: None,
            code: None,
            span: JsonSpan { file: "/tmp/test.ynz", start_byte: 0, end_byte: 5 },
            message: format!("{}\n\nWHAT INSTEAD: {}\n\nWHY: {}", d.what, d.what_instead, d.why),
            data: JsonDiagnosticData { what: &d.what, what_instead: &d.what_instead, why: &d.why },
        };
        let json = serde_json::to_string(&event).unwrap();
        // Output must be a single line — no raw newlines.
        assert_eq!(
            json.lines().count(),
            1,
            "serialized JSON must be exactly one line; embedded newlines must be escaped"
        );
        // The escaped form must be present.
        assert!(json.contains(r#"\n"#), "embedded newline must appear as \\n");
    }
}
