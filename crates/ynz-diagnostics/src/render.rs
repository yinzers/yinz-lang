use std::{collections::HashMap, fmt, io::Write as _};

use ariadne::{Config, Label, Report, ReportKind, Source};

use crate::{
    bucket::DiagnosticBucket,
    diagnostic::{Diagnostic, Severity},
};

struct SourceCache(HashMap<String, Source>);

impl ariadne::Cache<str> for SourceCache {
    type Storage = String;

    fn fetch(&mut self, id: &str) -> Result<&Source, impl fmt::Debug> {
        self.0
            .get(id)
            .ok_or_else(|| format!("unknown source file: {id}"))
    }

    fn display<'a>(&self, id: &'a str) -> Option<impl fmt::Display + 'a> {
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
    let mut cache = SourceCache(
        sources
            .iter()
            .map(|(k, v)| (k.clone(), Source::from(v.clone())))
            .collect(),
    );

    let config = Config::default().with_color(colors);

    let mut sorted: Vec<&Diagnostic> = bucket.iter().collect();
    sorted.sort_by_key(|d| (d.severity as u8, d.span.start));

    let mut out: Vec<u8> = Vec::new();

    for diag in &sorted {
        let kind = severity_to_kind(diag.severity, colors);

        let mut builder = Report::build(kind, diag.span.clone())
            .with_config(config)
            .with_message(&diag.what)
            .with_label(Label::new(diag.span.clone()).with_message(&diag.what_instead))
            .with_note(format!("Why: {}", diag.why));

        for rel in &diag.related {
            builder = builder.with_label(Label::new(rel.span.clone()).with_message(&rel.label));
        }

        builder
            .finish()
            .write(&mut cache, &mut out)
            .expect("ariadne render failed");
    }

    if bucket.hidden_count() > 0 {
        writeln!(out, "... and {} more errors hidden", bucket.hidden_count())
            .expect("write footer failed");
    }

    String::from_utf8(out).expect("ariadne output is valid UTF-8")
}
