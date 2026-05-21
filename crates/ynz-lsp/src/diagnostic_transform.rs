use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range,
    Url,
};
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

    // code is the PascalCase DiagnosticKind name — used by code-action handler to
    // identify which quick-fix to offer. Phase 9 adds data + codeDescription.
    let code = d
        .kind
        .as_ref()
        .map(|k| NumberOrString::String(k.kind_name().to_string()));

    Diagnostic {
        range,
        severity: Some(severity),
        source: Some("ynz".to_string()),
        message,
        related_information,
        code,
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
    use lsp_types::Position;
    use ynz_diagnostics::{Diagnostic as YnzDiag, Severity, SourceSpan};

    fn make_diag(severity: Severity, start: usize, end: usize) -> YnzDiag {
        YnzDiag {
            severity,
            span: SourceSpan::new("test.ynz", start, end),
            what: "WHAT content".to_string(),
            what_instead: "WHAT INSTEAD content".to_string(),
            why: "WHY content".to_string(),
            related: vec![],
            kind: None,
        }
    }

    #[test]
    fn severity_mapping() {
        let text = "let x = 5";
        let table = LineTable::new(text);
        let d = make_diag(Severity::Error, 0, 3);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::ERROR));

        let d = make_diag(Severity::Warning, 0, 3);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::WARNING));

        let d = make_diag(Severity::Suggestion, 0, 3);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        assert_eq!(lsp.severity, Some(DiagnosticSeverity::HINT));
    }

    #[test]
    fn message_preserves_what_what_instead_why() {
        let text = "let x = 5";
        let table = LineTable::new(text);
        let d = make_diag(Severity::Error, 0, 3);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        assert!(lsp.message.contains("WHAT content"));
        assert!(lsp.message.contains("WHAT INSTEAD: WHAT INSTEAD content"));
        assert!(lsp.message.contains("WHY: WHY content"));
    }

    #[test]
    fn source_is_ynz() {
        let text = "let x = 5";
        let table = LineTable::new(text);
        let d = make_diag(Severity::Error, 0, 3);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        assert_eq!(lsp.source, Some("ynz".to_string()));
    }

    #[test]
    fn range_computed_correctly() {
        let text = "let x = 5\nlet y = 10";
        let table = LineTable::new(text);
        // "let" on line 0, bytes 0-3
        let d = make_diag(Severity::Error, 0, 3);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        assert_eq!(
            lsp.range.start,
            Position {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            lsp.range.end,
            Position {
                line: 0,
                character: 3
            }
        );
    }

    #[test]
    fn zero_offset_span_is_valid() {
        let text = "let x = 5";
        let table = LineTable::new(text);
        let d = make_diag(Severity::Error, 0, 0);
        let lsp = to_lsp_diagnostic(&d, text, &table, PositionEncoding::Utf8);
        // Zero-width span at offset 0 is valid; start == end
        assert_eq!(lsp.range.start, lsp.range.end);
    }
}
