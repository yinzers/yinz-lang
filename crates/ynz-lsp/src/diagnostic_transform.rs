use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range,
    Url,
};
use serde_json::json;
use ynz_diagnostics::{Diagnostic as YnzDiagnostic, Severity};

use crate::{capabilities::PositionEncoding, position::LineTable};

/// Transform a ynz `Diagnostic` to an LSP `Diagnostic`.
///
/// The WHAT/WHAT-INSTEAD/WHY structure is preserved as plaintext with
/// `\n\n`-separated sections — VSCode renders these as soft breaks in the
/// squiggle tooltip. Delimiter collision mitigation: a debug_assert checks
/// that WHAT and WHAT-INSTEAD don't contain the separator substrings.
pub fn to_lsp_diagnostic(
    d: &YnzDiagnostic,
    text: &str,
    table: &LineTable,
    encoding: PositionEncoding,
) -> Diagnostic {
    debug_assert!(
        !d.what.contains("\n\nWHAT INSTEAD:") && !d.what_instead.contains("\n\nWHY:"),
        "diagnostic field contains LSP message delimiter — would mis-split on client"
    );

    let range = span_to_range(text, table, d.span.start, d.span.end, encoding);

    let severity = match d.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Suggestion => DiagnosticSeverity::HINT,
    };

    let message = format!(
        "{}\n\nWHAT INSTEAD: {}\n\nWHY: {}",
        d.what, d.what_instead, d.why
    );

    let related_information = if d.related.is_empty() {
        None
    } else {
        // Thin slice: only emit related-info for same-file spans where `text`/`table` are valid.
        // Cross-file related-info requires per-file text lookup (deferred — no caller supplies
        // a multi-file resolver yet); cross-file entries are silently omitted rather than
        // emitting ranges computed against the wrong file's bytes.
        let info: Vec<_> = d
            .related
            .iter()
            .filter(|rs| rs.span.file == d.span.file)
            .filter_map(|rs| {
                let uri = path_to_uri(&rs.span.file)?;
                let rel_range = span_to_range(text, table, rs.span.start, rs.span.end, encoding);
                Some(DiagnosticRelatedInformation {
                    location: Location {
                        uri,
                        range: rel_range,
                    },
                    message: rs.label.clone(),
                })
            })
            .collect();
        if info.is_empty() {
            None
        } else {
            Some(info)
        }
    };

    // `code` = PascalCase DiagnosticKind name — used by code-action handler and
    // tooling consumers to identify the diagnostic class without parsing the message.
    let code = d
        .kind
        .as_ref()
        .map(|k| NumberOrString::String(k.kind_name().to_string()));

    // `data` = structured WHAT/WHAT-INSTEAD/WHY for consumers that want to render
    // custom UI without re-parsing `message`. `codeDescription` reserved for docs
    // site URL — currently `None` until the docs site ships in v0.3+.
    let data = Some(json!({
        "what": d.what,
        "what_instead": d.what_instead,
        "why": d.why,
    }));

    Diagnostic {
        range,
        severity: Some(severity),
        source: Some("ynz".to_string()),
        message,
        related_information,
        code,
        data,
        ..Default::default()
    }
}

fn span_to_range(
    text: &str,
    table: &LineTable,
    start: usize,
    end: usize,
    encoding: PositionEncoding,
) -> Range {
    let start_pos = table.byte_offset_to_position(text, start, encoding);
    let end_pos = table.byte_offset_to_position(text, end.min(text.len()), encoding);
    // Guard: end must not precede start (zero-width spans at offset 0 are valid)
    let end_pos = if end_pos.line < start_pos.line
        || (end_pos.line == start_pos.line && end_pos.character < start_pos.character)
    {
        start_pos
    } else {
        end_pos
    };
    Range {
        start: start_pos,
        end: end_pos,
    }
}

/// Convert a file path to a URI. Returns `None` if the path cannot be resolved.
/// Caller is responsible for skipping related-info items whose URI returns None.
pub fn path_to_uri(path: &str) -> Option<Url> {
    Url::from_file_path(path)
        .ok()
        .or_else(|| Url::parse(path).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_diagnostics::{Diagnostic as YnzDiag, Severity, SourceSpan};

    macro_rules! severity_case {
        ($name:ident, $severity:expr, $expected:expr) => {
            // bug: an error shows as a warning (or a warning as an error or hint) in the editor | tier: Medium
            #[test]
            fn $name() {
                let text = "let x = 5";
                let table = LineTable::new(text);
                let diagnostic = YnzDiag {
                    severity: $severity,
                    span: SourceSpan::new("test.ynz", 0, 3),
                    what: "WHAT content".to_string(),
                    what_instead: "WHAT INSTEAD content".to_string(),
                    why: "WHY content".to_string(),
                    related: vec![],
                    kind: None,
                };
                let lsp = to_lsp_diagnostic(&diagnostic, text, &table, PositionEncoding::Utf8);
                assert_eq!(lsp.severity, Some($expected));
            }
        };
    }

    severity_case!(
        error_maps_to_error,
        Severity::Error,
        DiagnosticSeverity::ERROR
    );
    severity_case!(
        warning_maps_to_warning,
        Severity::Warning,
        DiagnosticSeverity::WARNING
    );
    severity_case!(
        suggestion_maps_to_hint,
        Severity::Suggestion,
        DiagnosticSeverity::HINT
    );
}
