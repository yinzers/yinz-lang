/// A valid Yinz keyword recognized by the lexer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct KeywordEntry {
    pub name: &'static str,
    /// The `Token::Xxx` variant emitted (e.g. `"Function"`).
    pub token: &'static str,
    pub since: &'static str,
    /// WHAT this keyword does (Rule 11 first clause). `None` = no hover doc yet.
    pub hover_what: Option<&'static str>,
    /// WHAT INSTEAD the developer should write to use it correctly (Rule 11 second clause).
    pub hover_what_instead: Option<&'static str>,
    /// WHY the keyword works the way it does — contextual, not generic (Rule 11 third clause).
    pub hover_why: Option<&'static str>,
}

/// A keyword from another language that Yinz rejects at lex time with a teaching error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BannedDeclarationKeywordEntry {
    pub name: &'static str,
    pub what_instead: &'static str,
    pub why: &'static str,
    pub since: &'static str,
}

/// A word banned from user-facing compiler diagnostic prose.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BannedJargonEntry {
    pub name: &'static str,
    pub replacement: &'static str,
    pub reason: &'static str,
    /// True for multi-character acronyms (ADT, AST, UTF-16).
    pub is_acronym: bool,
}

/// A built-in function or method on a primitive type.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrimitiveIntrinsicEntry {
    pub name: &'static str,
    /// `"print_type"` | `"free_fn"` | `"method"` | `"method_1arg"`
    pub kind: &'static str,
    /// `Some("int")` for method/method_1arg kinds; `None` for print_type/free_fn.
    pub receiver_type: Option<&'static str>,
    pub param_types: &'static [&'static str],
    pub return_type: &'static str,
    pub since: &'static str,
    /// Optional teaching note shown in IDE completion documentation.
    pub doc: Option<&'static str>,
}

/// A constant accessible as `Type.name` (e.g. `int.max`, `number.epsilon`).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TypeAttachedConstantEntry {
    pub type_name: &'static str,
    pub const_name: &'static str,
    pub value_type: &'static str,
    /// Numeric value as a string to preserve full precision (e.g. `"9223372036854775807"`).
    pub value_literal: &'static str,
    pub since: &'static str,
}

/// A language feature reserved but not yet implemented.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeferredLanguageFeatureEntry {
    /// The token or syntax that triggers this error.
    pub name: &'static str,
    /// What to use instead right now. Empty string if no current substitute.
    pub substitute: &'static str,
    pub why: &'static str,
    pub ships_in: &'static str,
    pub design_doc: &'static str,
    /// What user code makes this error fire.
    pub triggers: &'static str,
}

/// A tooling feature reserved but not yet implemented.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeferredToolingFeatureEntry {
    pub name: &'static str,
    pub substitute: &'static str,
    pub why: &'static str,
    pub ships_in: &'static str,
    pub design_doc: &'static str,
    pub triggers: &'static str,
}

/// Canonical WHAT/WHAT-INSTEAD/WHY text for a reusable diagnostic kind.
/// Supports `{placeholder}` substitution; `{{` and `}}` are literal braces.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiagnosticTemplateEntry {
    /// Matches the `DiagnosticKind` variant name.
    pub kind_name: &'static str,
    pub what_template: &'static str,
    pub what_instead_template: &'static str,
    pub why_template: &'static str,
}

/// An IDE inference domain per `.claude/rules/inference.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MutedHintDomainEntry {
    pub domain: &'static str,
    /// `"Addition"` | `"Replacement"` | `"Informational"`
    pub placement_category: &'static str,
    pub description: &'static str,
    pub example_source: &'static str,
    pub example_hint_rendered: &'static str,
}
