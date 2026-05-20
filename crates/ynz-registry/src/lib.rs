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

pub fn muted_hint_domains() -> impl Iterator<Item = &'static MutedHintDomainEntry> {
    MUTED_HINT_DOMAINS.iter()
}

pub fn muted_hint_domain_lookup(domain: &str) -> Option<&'static MutedHintDomainEntry> {
    MUTED_HINT_DOMAINS.iter().find(|e| e.domain == domain)
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
