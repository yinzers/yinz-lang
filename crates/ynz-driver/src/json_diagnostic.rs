use serde::Serialize;
use ynz_diagnostics::{DiagnosticBucket, Severity};

/// Schema version for `ynz build --json` NDJSON output.
///
/// Stable as of v0.2.0. Pre-release builds use an `-unstable` suffix; tooling
/// consumers MUST check this field and reject schemas they have not validated.
pub const SCHEMA_VERSION: &str = "v0.2.0";

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
        let line = serde_json::to_string(&event).unwrap_or_else(|_| {
            r#"{"type":"diagnostic","error":"serialization failed"}"#.to_string()
        });
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
        // The `array-using-soa-layout` Tier 3 lint lives OUTSIDE check_query (its
        // inputs depend on check_query — salsa cycle), so the --json path merges it
        // here; tooling consumers see the same lint stream the stderr path renders.
        for lint in ynz_typeck::queries::soa_layout_lints(db, sf) {
            bucket.push(lint);
        }
    }
    bucket
}
