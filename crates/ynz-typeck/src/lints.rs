//! THE generic Tier 3 lint firing helper — the one compiler-side seam of the
//! `[[lint_rule]]` registry mechanism (v0.3-M4 Phase 4).
//!
//! A lint rule's teaching text (WHAT / WHAT-INSTEAD / WHY templates), severity, and
//! LSP code all live in exactly ONE home: its `[[lint_rule]]` entry in
//! `registry/features.toml`. A compiler pass that detects a lint condition calls
//! [`lint_diagnostic`] with the rule name plus the per-site `{placeholder}` vars and
//! pushes the returned diagnostic — nothing rule-specific lives here.
//!
//! Adding a new rule (e.g. v0.3-M5's `array-using-soa-layout`) is therefore a TOML
//! entry plus a firing site that calls this helper; the mechanism (registry schema,
//! parser, baked constants, this builder, the LSP `Diagnostic.code` seam) needs zero
//! changes.

use std::collections::HashMap;

use ynz_diagnostics::{Diagnostic, DiagnosticKind, Severity, SourceSpan};

/// Build the diagnostic for a fired lint rule.
///
/// - Text comes from the rule's registry templates, `{placeholder}`-substituted with `vars`.
/// - Severity comes from the rule's registry `severity` field (`"warning"` maps to
///   [`Severity::Warning`]; everything else — the Tier 3 default `"suggestion"` — maps to
///   [`Severity::Suggestion`]; `"error"` cannot occur, `build.rs` rejects it at parse time).
/// - The diagnostic carries [`DiagnosticKind::LintRule`] so the LSP `Diagnostic.code` is the
///   kebab-case rule name (group/dismiss-by-rule in editors).
///
/// Returns `None` when no `[[lint_rule]]` entry exists for `rule_name` — a firing site for
/// an unregistered rule is a compiler bug; callers `expect` with the rule name so the gap
/// is loud, never a silently-skipped teaching surface.
pub fn lint_diagnostic(
    rule_name: &str,
    span: SourceSpan,
    vars: &HashMap<&str, &str>,
) -> Option<Diagnostic> {
    let entry = ynz_registry::lint_rule_lookup(rule_name)?;
    let (what, what_instead, why) = ynz_registry::lint_rule_diagnostic_parts(rule_name, vars)?;
    let severity = match entry.severity {
        "warning" => Severity::Warning,
        _ => Severity::Suggestion,
    };
    Some(
        Diagnostic::new(severity, span, what, what_instead, why)
            .with_kind(DiagnosticKind::LintRule { rule: entry.name }),
    )
}
