use std::{collections::HashMap, fmt, io::Write as _};

use ariadne::{Config, Label, Report, ReportKind, Source};

use crate::{
    bucket::DiagnosticBucket,
    diagnostic::{Diagnostic, Severity},
};

/// Lazily-parsed source cache: Source objects are only built for files that
/// actually appear in a diagnostic span.
// Perf: avoids cloning + building line tables for files with no diagnostics in
// this render pass. For a 100-file project, only the 1–2 files mentioned in
// diagnostics pay the line-table-build cost.
struct SourceCache<'a> {
    sources: &'a HashMap<String, String>,
    parsed: HashMap<String, Source>,
}

impl<'a> SourceCache<'a> {
    fn new(sources: &'a HashMap<String, String>) -> Self {
        Self {
            sources,
            parsed: HashMap::new(),
        }
    }
}

impl<'a> ariadne::Cache<str> for SourceCache<'a> {
    type Storage = String;

    #[allow(refining_impl_trait)]
    fn fetch(&mut self, id: &str) -> Result<&Source, String> {
        // Insert on first access; only files referenced by a span pay the parse cost.
        if !self.parsed.contains_key(id) {
            let text = self
                .sources
                .get(id)
                .ok_or_else(|| format!("unknown source file: {id}"))?;
            self.parsed
                .insert(id.to_string(), Source::from(text.clone()));
        }
        Ok(self.parsed.get(id).unwrap())
    }

    fn display<'b>(&self, id: &'b str) -> Option<impl fmt::Display + 'b> {
        Some(id)
    }
}

fn severity_to_kind(severity: Severity, colors: bool) -> ReportKind<'static> {
    match severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
        // Custom kinds embed a color directly in the ReportKind — bypass that
        // when color is disabled so no ANSI codes appear in colorless output.
        Severity::Suggestion => ReportKind::Custom(
            "SUGGESTION",
            if colors {
                ariadne::Color::Blue
            } else {
                ariadne::Color::Primary
            },
        ),
    }
}

/// Render all diagnostics in `bucket` using `sources` as the backing text.
///
/// `colors` controls ANSI escape codes — pass `false` in tests for deterministic snapshots.
///
/// Render order: errors first (by source position), then warnings, then suggestions.
/// If `bucket.hidden_count() > 0`, appends:
/// `... and N more errors hidden`
pub fn render(
    bucket: &DiagnosticBucket,
    sources: &HashMap<String, String>,
    colors: bool,
) -> String {
    let mut cache = SourceCache::new(sources);

    let config = Config::default().with_color(colors);

    let mut sorted: Vec<&Diagnostic> = bucket.iter().collect();
    sorted.sort_by_key(|d| (d.severity as u8, d.span.start));

    let mut out: Vec<u8> = Vec::new();

    for diag in &sorted {
        let report_kind = severity_to_kind(diag.severity, colors);

        // When a DiagnosticKind is present: use its terse tag as the caret label
        // and move the full what_instead prose into the note alongside why.
        // When absent: fall back to what_instead as the caret label (legacy path).
        let (caret_label, note_text) = if let Some(dk) = &diag.kind {
            let note = format!("{} — {}", diag.what_instead, diag.why);
            (dk.tag(), note)
        } else {
            (diag.what_instead.clone(), diag.why.clone())
        };

        let mut builder = Report::build(report_kind, diag.span.clone())
            .with_config(config)
            .with_message(&diag.what)
            .with_label(Label::new(diag.span.clone()).with_message(caret_label))
            .with_note(note_text);

        for rel in &diag.related {
            builder = builder.with_label(Label::new(rel.span.clone()).with_message(&rel.label));
        }

        if let Err(e) = builder.finish().write(&mut cache, &mut out) {
            let _ = writeln!(
                out,
                "<unable to render diagnostic for {}: {e}>",
                diag.span.file,
            );
        }
    }

    if bucket.hidden_count() > 0 {
        writeln!(out, "... and {} more errors hidden", bucket.hidden_count())
            .expect("write footer failed");
    }

    let has_errors = sorted.iter().any(|d| matches!(d.severity, Severity::Error));
    if has_errors {
        let url = "https://github.com/yinzers/yinz-lang/issues";
        if colors {
            // Bold + underline the URL with ANSI codes.
            writeln!(
                out,
                "\nIf any of these errors are confusing or unhelpful, please open an issue:\
                 \n  \x1b[1;4m{url}\x1b[0m"
            )
            .expect("write feedback footer failed");
        } else {
            writeln!(
                out,
                "\nIf any of these errors are confusing or unhelpful, please open an issue:\
                 \n  {url}"
            )
            .expect("write feedback footer failed");
        }
    }

    String::from_utf8(out).expect("ariadne output is valid UTF-8")
}
