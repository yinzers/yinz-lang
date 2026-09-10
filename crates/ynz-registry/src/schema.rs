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
    /// The ownership word of each parameter, aligned with `param_types` (`"share"` |
    /// `"lend"` | `"give"`), or EMPTY for today's by-value/shared semantics. v0.3-M8 Phase
    /// 4 (FRAGO 003): `send` carries `["give"]` so the LSP hover says "send gives its value"
    /// from the registry rather than from a typeck constant. `build.rs` validates every
    /// value and the alignment, so a typo or a misaligned list fails the build, never
    /// ships as hover text (parked item 5).
    pub param_ownership: &'static [&'static str],
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

/// A Tier 3 lint rule per `docs/internal/implementation/IMP-linting.md`.
///
/// Lint rules are TEACHING surfaces, never errors: `severity` is `"suggestion"`
/// (dismissable, the Tier 3 default) or `"warning"` — `build.rs` rejects `"error"`
/// at registry-parse time so a lint can never block a compile.
///
/// The three `*_template` fields carry the canonical WHAT / WHAT-INSTEAD / WHY text
/// (Golden Rule 11) with `{placeholder}` substitution via [`crate::render_template`] —
/// the firing site in the compiler supplies the per-site vars, so the rule's teaching
/// text has exactly ONE home. Adding a new lint rule is a TOML entry plus its firing
/// site; no mechanism change (schema, parser, constants, LSP seam) is required.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct LintRuleEntry {
    /// The kebab-case rule id (e.g. `"prefer-yielding-sleep"`). Used as the LSP
    /// `Diagnostic.code` so editors can group/dismiss by rule.
    pub name: &'static str,
    /// `"suggestion"` (Tier 3 default, dismissable) | `"warning"`. Never `"error"`.
    pub severity: &'static str,
    /// One-sentence description of what the rule teaches (LSP hover/documentation).
    pub description: &'static str,
    /// WHAT happened — `{placeholder}`-substituted at the firing site.
    pub what_template: &'static str,
    /// WHAT to write instead — `{placeholder}`-substituted at the firing site.
    pub what_instead_template: &'static str,
    /// WHY — contextual reason, `{placeholder}`-substituted at the firing site.
    pub why_template: &'static str,
    pub since: &'static str,
    /// The design doc that locked this rule (repo-relative path).
    pub design_doc: &'static str,
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
    /// Explicit hover WHAT, used verbatim when set. When `None`, the hover path derives WHAT from
    /// `description`. Needed for domains where the rendered hint is not itself the explicit form
    /// (Informational comment-style hints have no typeable equivalent — see `parallel_groups`).
    pub hover_what: Option<&'static str>,
    /// Explicit hover WHAT-INSTEAD, used verbatim when set. When `None`, the hover path uses
    /// `example_hint_rendered` (correct only for Addition/Replacement domains whose rendered hint
    /// IS the explicit source form). Informational domains must set this to an actionable sentence
    /// rather than ask the user to type a comment.
    pub hover_what_instead: Option<&'static str>,
    /// Explicit hover WHY, used verbatim when set. When `None`, the hover path derives a generic
    /// WHY from `placement_category`. Set this to a contextual reason naming what the decision
    /// depends on.
    pub hover_why: Option<&'static str>,
}
