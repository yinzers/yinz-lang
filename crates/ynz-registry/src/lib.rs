pub mod lsp_adapter;
mod schema;
pub use lsp_adapter::{
    lsp_completion_items, lsp_hover_for_token, CompletionContext, CompletionKind, HoverContent,
    HoverKind, RegistryCompletionItem,
};
pub use schema::*;

// Static arrays baked at compile time from registry/features.toml.
include!(concat!(env!("OUT_DIR"), "/registry.rs"));

// ---------------------------------------------------------------------------
// Adapter functions — follow the *Table convention from ynz-typeck/intrinsics.rs
// ---------------------------------------------------------------------------

pub fn keywords() -> impl Iterator<Item = &'static KeywordEntry> {
    KEYWORDS.iter()
}

pub fn keyword_lookup(name: &str) -> Option<&'static KeywordEntry> {
    KEYWORDS.iter().find(|e| e.name == name)
}

pub fn banned_declaration_keywords() -> impl Iterator<Item = &'static BannedDeclarationKeywordEntry>
{
    BANNED_DECLARATION_KEYWORDS.iter()
}

pub fn banned_declaration_keyword_lookup(
    name: &str,
) -> Option<&'static BannedDeclarationKeywordEntry> {
    BANNED_DECLARATION_KEYWORDS.iter().find(|e| e.name == name)
}

pub fn banned_jargon() -> impl Iterator<Item = &'static BannedJargonEntry> {
    BANNED_JARGON.iter()
}

pub fn banned_jargon_lookup(word: &str) -> Option<&'static BannedJargonEntry> {
    BANNED_JARGON.iter().find(|e| e.name == word)
}

pub fn primitive_intrinsics() -> impl Iterator<Item = &'static PrimitiveIntrinsicEntry> {
    PRIMITIVE_INTRINSICS.iter()
}

/// Look up a method by receiver type and name (returns all matching overloads).
pub fn primitive_intrinsic_methods<'a>(
    receiver_type: &'a str,
    name: &'a str,
) -> impl Iterator<Item = &'static PrimitiveIntrinsicEntry> + 'a {
    PRIMITIVE_INTRINSICS
        .iter()
        .filter(move |e| e.receiver_type == Some(receiver_type) && e.name == name)
}

/// Look up a free function by name (returns all matching overloads).
pub fn primitive_free_fns(
    name: &str,
) -> impl Iterator<Item = &'static PrimitiveIntrinsicEntry> + '_ {
    PRIMITIVE_INTRINSICS
        .iter()
        .filter(move |e| e.kind == "free_fn" && e.name == name)
}

pub fn type_attached_constants() -> impl Iterator<Item = &'static TypeAttachedConstantEntry> {
    TYPE_ATTACHED_CONSTANTS.iter()
}

pub fn type_attached_constant_lookup(
    type_name: &str,
    const_name: &str,
) -> Option<&'static TypeAttachedConstantEntry> {
    TYPE_ATTACHED_CONSTANTS
        .iter()
        .find(|e| e.type_name == type_name && e.const_name == const_name)
}

pub fn deferred_language_features() -> impl Iterator<Item = &'static DeferredLanguageFeatureEntry> {
    DEFERRED_LANGUAGE_FEATURES.iter()
}

pub fn deferred_language_feature_lookup(
    name: &str,
) -> Option<&'static DeferredLanguageFeatureEntry> {
    DEFERRED_LANGUAGE_FEATURES.iter().find(|e| e.name == name)
}

pub fn deferred_tooling_features() -> impl Iterator<Item = &'static DeferredToolingFeatureEntry> {
    DEFERRED_TOOLING_FEATURES.iter()
}

pub fn diagnostic_templates() -> impl Iterator<Item = &'static DiagnosticTemplateEntry> {
    DIAGNOSTIC_TEMPLATES.iter()
}

pub fn diagnostic_template_lookup(kind_name: &str) -> Option<&'static DiagnosticTemplateEntry> {
    DIAGNOSTIC_TEMPLATES
        .iter()
        .find(|e| e.kind_name == kind_name)
}

pub fn lint_rules() -> impl Iterator<Item = &'static LintRuleEntry> {
    LINT_RULES.iter()
}

pub fn lint_rule_lookup(name: &str) -> Option<&'static LintRuleEntry> {
    LINT_RULES.iter().find(|e| e.name == name)
}

/// Render the three diagnostic parts (WHAT, WHAT-INSTEAD, WHY) for a lint rule, with
/// `{placeholder}` substitution from `vars`.
///
/// This is THE generic firing helper: a compiler pass that detects a lint condition calls
/// this with the rule's registry name plus its per-site vars and gets the canonical teaching
/// text back — the text has exactly one home (the `[[lint_rule]]` entry), so adding a new
/// rule (e.g. M5's `array-using-soa-layout`) is a TOML entry + a firing site, with zero
/// change to this mechanism.
///
/// Returns `None` when no `[[lint_rule]]` entry exists for `rule_name`.
/// Panics (via [`render_template`]) on an unknown `{placeholder}` key — a template/firing-site
/// mismatch is a compiler bug, not a user error.
pub fn lint_rule_diagnostic_parts(
    rule_name: &str,
    vars: &std::collections::HashMap<&str, &str>,
) -> Option<(String, String, String)> {
    let entry = lint_rule_lookup(rule_name)?;
    Some((
        render_template(entry.what_template, vars),
        render_template(entry.what_instead_template, vars),
        render_template(entry.why_template, vars),
    ))
}

/// Render a WHAT / WHAT-INSTEAD / WHY hover for a lint rule's LSP `Diagnostic.code`.
///
/// Mirrors [`lsp_inlay_hint_hover_for`]: markdown suitable for an editor surface keyed by
/// the diagnostic code (the kebab-case rule name). Templates render with their placeholders
/// left intact (`{callee}` etc.) — the hover documents the RULE, not one firing site; the
/// fired diagnostic itself already carries the substituted per-site text.
///
/// Returns `None` when the rule is not in the registry.
pub fn lsp_lint_rule_hover_for(rule_name: &str) -> Option<String> {
    let entry = lint_rule_lookup(rule_name)?;
    Some(format!(
        "**{name}** ({severity}) — {description}\n\n**WHAT**: {what}\n\n**WHAT INSTEAD**: {what_instead}\n\n**WHY**: {why}",
        name = entry.name,
        severity = entry.severity,
        description = entry.description,
        what = entry.what_template,
        what_instead = entry.what_instead_template,
        why = entry.why_template,
    ))
}

pub fn muted_hint_domains() -> impl Iterator<Item = &'static MutedHintDomainEntry> {
    MUTED_HINT_DOMAINS.iter()
}

pub fn muted_hint_domain_lookup(domain: &str) -> Option<&'static MutedHintDomainEntry> {
    MUTED_HINT_DOMAINS.iter().find(|e| e.domain == domain)
}

/// Render a WHAT / WHAT-INSTEAD / WHY hover tooltip for an inlay hint domain.
///
/// Returns markdown suitable for an LSP `InlayHint.tooltip` field.
///
/// Each part prefers the entry's EXPLICIT hover field (`hover_what` / `hover_what_instead` /
/// `hover_why`) when set, and otherwise falls back to the generic derivation:
/// - **WHAT**: `hover_what`, else the entry's `description`.
/// - **WHAT INSTEAD**: `hover_what_instead`, else "write `{example_hint_rendered}` to make the
///   decision explicit". The fallback is correct only for Addition/Replacement domains whose
///   rendered hint IS a typeable source form; Informational comment-style domains (e.g.
///   `parallel_groups`) have no typeable equivalent and MUST set `hover_what_instead` to an
///   actionable sentence rather than asking the user to type a comment.
/// - **WHY**: `hover_why`, else a generic sentence keyed off `placement_category`.
///
/// Returns `None` when the domain is not in the registry (deferred/unknown).
///
/// Per Golden Rule 11, every teaching surface must answer WHAT / WHAT-INSTEAD / WHY. Inlay hints
/// without a hover tooltip are muted annotations the user cannot learn from.
pub fn lsp_inlay_hint_hover_for(domain: &str) -> Option<String> {
    let entry = muted_hint_domain_lookup(domain)?;

    let what = entry.hover_what.unwrap_or(entry.description).to_string();

    let what_instead = match entry.hover_what_instead {
        Some(s) => s.to_string(),
        None => format!(
            "`{}` — write this to make the decision explicit in source.",
            entry.example_hint_rendered
        ),
    };

    let why = match entry.hover_why {
        Some(s) => s.to_string(),
        None => match entry.placement_category {
            "Addition" => {
                "The compiler figured this out from context. Click to make it explicit in source."
            }
            "Replacement" => {
                "The compiler picked the stricter form automatically. Hover to see the alternative \
                 you could write explicitly."
            }
            "Informational" => {
                "This shows what the compiler decided at this call site. No source change is needed."
            }
            _ => "The compiler figured this out automatically.",
        }
        .to_string(),
    };

    Some(format!(
        "**WHAT**: {what}\n\n**WHAT INSTEAD**: {what_instead}\n\n**WHY**: {why}",
    ))
}

// CARVE-OUT: simple-replacement table for code-action label generation.
// Each entry maps a banned declaration keyword to its direct Yinz replacement token.
// Not in features.toml because the replacement token is already implied by the
// `what_instead` prose in each [[banned_declaration_keyword]] entry; duplicating it
// per-entry adds N redundant fields for the same label template ("Replace `X` with `Y`")
// across all keyword entries — a violation of DRY without benefit.
const SIMPLE_KEYWORD_REPLACEMENTS: &[(&str, &str)] = &[
    ("class", "shape"),
    ("struct", "shape"),
    ("type", "shape"),
    ("interface", "shape"),
    ("enum", "options"),
    ("fn", "function"),
    ("func", "function"),
    ("await", "wait"),
];

/// Return a quick-fix label for the given diagnostic kind and token, following the
/// "Replace `X` with `Y`" template used consistently across all code actions.
///
/// Returns `None` when the kind has no unambiguous single-token quick-fix
/// (e.g., `async` → multi-step transformation, not a one-word swap).
pub fn lsp_code_action_label_for(diagnostic_kind: &str, token: &str) -> Option<String> {
    let replacement = lsp_code_action_replacement_for(diagnostic_kind, token)?;
    Some(format!("Replace `{token}` with `{replacement}`"))
}

/// Return the replacement token for the given diagnostic kind and offending token.
///
/// Returns `None` when there is no straightforward single-token replacement
/// (complex cases like `async`/`abstract` require multi-token edits or context-
/// dependent rewrites — those are deferred to v0.3 per `lsp-complex-code-actions`
/// in todos.md).
pub fn lsp_code_action_replacement_for(diagnostic_kind: &str, token: &str) -> Option<&'static str> {
    match diagnostic_kind {
        "BannedKeyword" => SIMPLE_KEYWORD_REPLACEMENTS
            .iter()
            .find(|(banned, _)| *banned == token)
            .map(|(_, replacement)| *replacement),
        "BannedJargon" => banned_jargon_lookup(token).map(|e| e.replacement),
        _ => None,
    }
}

/// Return a quick-fix label for a `BannedJargon` diagnostic, embedding the registry
/// `reason` field so the user learns WHY the word is banned, not just what to write instead.
///
/// Returns `None` when no `[[banned_jargon]]` entry exists for the term.
pub fn lsp_code_action_label_for_jargon(term: &str) -> Option<String> {
    let entry = banned_jargon_lookup(term)?;
    Some(format!(
        "Replace `{term}` with `{replacement}` — {reason}",
        replacement = entry.replacement,
        reason = entry.reason,
    ))
}

/// Render a diagnostic template string by substituting `{key}` placeholders.
///
/// Grammar:
/// - `{name}` → replaced with the value for `"name"` from `vars`
/// - `{{` → literal `{`
/// - `}}` → literal `}`
/// - Unknown key → panic naming the template entry and the unknown key
pub fn render_template(template: &str, vars: &std::collections::HashMap<&str, &str>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    out.push('{');
                } else {
                    // Collect key until '}'
                    let mut key = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => key.push(c),
                            None => {
                                panic!("render_template: unclosed '{{' in template: {template:?}")
                            }
                        }
                    }
                    let value = vars.get(key.as_str()).unwrap_or_else(|| {
                        panic!(
                            "render_template: unknown placeholder key '{key}' in template: {template:?}"
                        )
                    });
                    out.push_str(value);
                }
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    out.push('}');
                } else {
                    panic!("render_template: unescaped '}}' in template: {template:?}");
                }
            }
            other => out.push(other),
        }
    }

    out
}
