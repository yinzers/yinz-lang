use crate::span::SourceSpan;

/// How urgent a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Won't compile. Shown first.
    Error,
    /// Compiles but indicates a problem.
    Warning,
    /// IDE-style hint. Shown last.
    Suggestion,
}

/// Structural category of a diagnostic, used to generate a terse caret label.
///
/// A `DiagnosticKind` on a diagnostic means the caret label is derived from the
/// kind (e.g. `"expected: int"`, `"not const"`) rather than from `what_instead`.
/// The full `what_instead` prose moves to the note position alongside `why`.
///
/// `None` (the default) falls back to the first ~40 chars of `what` as the caret label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// Type mismatch — expected a specific type.
    TypeMismatch { expected: String },
    /// Binding is `const` so the operation is rejected.
    MutationOfConst,
    /// Name is not defined in this scope.
    NotDefined,
    /// Field is missing from a shape literal.
    MissingField { field: String },
    /// Field is hidden — not accessible from outside the declaring file.
    HiddenAccess,
    /// Import was not found.
    ImportNotFound,
    /// Value was already consumed (use-after-give).
    Consumed,
    /// Value is already borrowed.
    Borrowed,
    /// Function does not return on all paths.
    MissingReturn,
    /// A banned keyword was used.
    BannedKeyword { keyword: String },
    /// A banned jargon word was used as an identifier — the identifier name is
    /// carried so the LSP code-action handler can build a quick-fix without
    /// re-parsing the source text.
    BannedJargon { term: String },
    /// An imported name is never referenced in the file.
    UnusedImport { name: String },
    /// A Tier 3 lint rule fired. `rule` is the kebab-case `[[lint_rule]]` registry name
    /// (`&'static str` because it always comes from the registry's baked constants) —
    /// it becomes the LSP `Diagnostic.code` so editors can group/dismiss by rule, and
    /// `ynz_registry::lsp_lint_rule_hover_for(code)` renders the rule's teaching hover.
    LintRule { rule: &'static str },
}

impl DiagnosticKind {
    /// The registry name for this variant — used as the LSP `Diagnostic.code` value
    /// and in `--json` output. PascalCase for `[[diagnostic_template]]`-class kinds
    /// (matches `kind_name` in `registry/features.toml`); kebab-case for `LintRule`
    /// (the `[[lint_rule]]` entry name, e.g. `"prefer-yielding-sleep"`).
    pub fn kind_name(&self) -> &'static str {
        match self {
            DiagnosticKind::TypeMismatch { .. } => "TypeMismatch",
            DiagnosticKind::MutationOfConst => "MutationOfConst",
            DiagnosticKind::NotDefined => "NotDefined",
            DiagnosticKind::MissingField { .. } => "MissingField",
            DiagnosticKind::HiddenAccess => "HiddenAccess",
            DiagnosticKind::ImportNotFound => "ImportNotFound",
            DiagnosticKind::Consumed => "Consumed",
            DiagnosticKind::Borrowed => "Borrowed",
            DiagnosticKind::MissingReturn => "MissingReturn",
            DiagnosticKind::BannedKeyword { .. } => "BannedKeyword",
            DiagnosticKind::BannedJargon { .. } => "BannedJargon",
            DiagnosticKind::UnusedImport { .. } => "UnusedImport",
            DiagnosticKind::LintRule { rule } => rule,
        }
    }

    /// Generate the terse caret tag shown inline at the error site.
    pub fn tag(&self) -> String {
        match self {
            DiagnosticKind::TypeMismatch { expected } => format!("expected: {expected}"),
            DiagnosticKind::MutationOfConst => "not mutable".to_string(),
            DiagnosticKind::NotDefined => "not defined".to_string(),
            DiagnosticKind::MissingField { field } => format!("missing: {field}"),
            DiagnosticKind::HiddenAccess => "hidden".to_string(),
            DiagnosticKind::ImportNotFound => "not found".to_string(),
            DiagnosticKind::Consumed => "consumed".to_string(),
            DiagnosticKind::Borrowed => "borrowed".to_string(),
            DiagnosticKind::MissingReturn => "missing return".to_string(),
            DiagnosticKind::BannedKeyword { keyword } => format!("`{keyword}` not valid here"),
            DiagnosticKind::BannedJargon { term } => format!("`{term}` is programmer jargon"),
            DiagnosticKind::UnusedImport { name } => format!("`{name}` never used"),
            DiagnosticKind::LintRule { rule } => format!("lint: {rule}"),
        }
    }
}

/// A secondary span attached to a diagnostic with its own label.
#[derive(Clone, Debug, PartialEq)]
pub struct RelatedSpan {
    pub span: SourceSpan,
    pub label: String,
}

/// A single compiler diagnostic following the mandatory three-part format:
///
/// - **what**: one sentence describing what went wrong (plain English, no jargon)
/// - **what_instead**: the corrected code or action, ready to copy
/// - **why**: the reason — correctness, performance, convention, or safety
///
/// All three fields are required. The constructor panics if any is empty — this encodes
/// Golden Rule 11 in the type system so a missing "Why:" can never reach a user.
///
/// When `kind` is `Some`, the renderer uses `kind.tag()` as the caret label and
/// places `what_instead` in the note alongside `why`. When `kind` is `None`, the
/// caret label falls back to a truncated form of `what`.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: SourceSpan,
    pub what: String,
    pub what_instead: String,
    pub why: String,
    pub related: Vec<RelatedSpan>,
    /// Optional structural kind — drives the caret label and note layout.
    pub kind: Option<DiagnosticKind>,
}

impl Diagnostic {
    pub fn new(
        severity: Severity,
        span: SourceSpan,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        let what = what.into();
        let what_instead = what_instead.into();
        let why = why.into();

        assert!(
            !what.is_empty(),
            "Diagnostic.what must not be empty — Golden Rule 11 requires all three message parts"
        );
        assert!(
            !what_instead.is_empty(),
            "Diagnostic.what_instead must not be empty — Golden Rule 11 requires all three message parts"
        );
        assert!(
            !why.is_empty(),
            "Diagnostic.why must not be empty — Golden Rule 11 requires all three message parts"
        );

        Self {
            severity,
            span,
            what,
            what_instead,
            why,
            related: Vec::new(),
            kind: None,
        }
    }

    pub fn error(
        span: SourceSpan,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Error, span, what, what_instead, why)
    }

    pub fn warning(
        span: SourceSpan,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Warning, span, what, what_instead, why)
    }

    pub fn suggestion(
        span: SourceSpan,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::new(Severity::Suggestion, span, what, what_instead, why)
    }

    /// Attach an additional span with a label (e.g. "defined here", "first use here").
    pub fn with_related(mut self, span: SourceSpan, label: impl Into<String>) -> Self {
        self.related.push(RelatedSpan {
            span,
            label: label.into(),
        });
        self
    }

    /// Attach a structural kind that drives the caret label and note layout.
    pub fn with_kind(mut self, kind: DiagnosticKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Convenience constructor for file-level errors with no meaningful span.
    ///
    /// Wraps the common pattern `Diagnostic::error(SourceSpan::new(path, 0, 0), ...)`.
    pub fn file_error(
        path: impl Into<String>,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::error(SourceSpan::new(path.into(), 0, 0), what, what_instead, why)
    }

    /// Convenience constructor for file-level warnings with no meaningful span.
    pub fn file_warning(
        path: impl Into<String>,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::warning(SourceSpan::new(path.into(), 0, 0), what, what_instead, why)
    }

    /// Convenience constructor for file-level suggestions with no meaningful span.
    pub fn file_suggestion(
        path: impl Into<String>,
        what: impl Into<String>,
        what_instead: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        Self::suggestion(SourceSpan::new(path.into(), 0, 0), what, what_instead, why)
    }
}
