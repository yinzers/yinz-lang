// Thin adapter — all data lives in registry/features.toml.
// Schema reference: design/feature-registry.md #deferred_language_feature

pub use ynz_registry::{
    DeferredLanguageFeatureEntry, deferred_language_feature_lookup, deferred_language_features,
};

/// Render a deferred-feature entry into the (what_instead, why) pair that
/// `emit_banned_declaration_keyword` expects.
///
/// Called by the lexer whenever a user writes a reserved-but-not-yet-shipped
/// token (sized numerics, `test`, future keywords). Centralising the call
/// here means any future consumer (codegen, LSP) uses the same rendering
/// without re-implementing the registry lookup.
pub fn render_deferred_feature(entry: &DeferredLanguageFeatureEntry) -> (&'static str, &'static str) {
    (entry.substitute, entry.why)
}
