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
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: SourceSpan,
    pub what: String,
    pub what_instead: String,
    pub why: String,
    pub related: Vec<RelatedSpan>,
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
}
