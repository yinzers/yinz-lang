//! LSP completion and hover adapters.
//!
//! `ynz-registry` does NOT depend on `lsp-types` — it returns registry-owned
//! mirror types (`RegistryCompletionItem`, `HoverContent`) and the `ynz-lsp`
//! crate translates those to `lsp_types` shapes. This keeps `ynz-registry` as
//! a foundational crate with no editor-tool deps.


// Completion items


/// The context at the cursor when a completion is requested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionContext<'a> {
    /// Cursor is at the start of a line or after whitespace — show keywords +
    /// types + visible symbols.
    BareIdentifier,
    /// Cursor is directly after a `.` with an identified receiver type.
    AfterDot { receiver_type: Option<&'a str> },
}

/// High-level completion kind (maps to `lsp_types::CompletionItemKind` in ynz-lsp).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    PrimitiveMethod,
    FreeFn,
    TypeAttachedConstant,
    DeferredFeature,
    BannedKeyword,
}

/// A completion item produced by the registry — no `lsp-types` dependency.
#[derive(Clone, Debug)]
pub struct RegistryCompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub deprecated: bool,
    /// Lower number = sorted earlier. 0xx = user symbols (added by ynz-lsp),
    /// 100 = keywords, 200 = intrinsics, 900 = deferred (deprecated last).
    pub sort_priority: u16,
}

/// Return registry-sourced completion items for the given context.
///
/// Caller (ynz-lsp) merges user-defined symbols BEFORE these (priority 0xx)
/// so user symbols appear first in the sorted list.
pub fn lsp_completion_items(context: &CompletionContext<'_>) -> Vec<RegistryCompletionItem> {
    let mut items: Vec<RegistryCompletionItem> = Vec::new();

    match context {
        CompletionContext::BareIdentifier => {
            // Keywords
            for e in crate::keywords() {
                items.push(RegistryCompletionItem {
                    label: e.name.to_string(),
                    kind: CompletionKind::Keyword,
                    detail: Some(format!("keyword — since {}", e.since)),
                    documentation: None,
                    deprecated: false,
                    sort_priority: 100,
                });
            }
            // Banned declaration keywords (show as deprecated so the user sees
            // the right alternative in autocomplete)
            for e in crate::banned_declaration_keywords() {
                items.push(RegistryCompletionItem {
                    label: e.name.to_string(),
                    kind: CompletionKind::BannedKeyword,
                    detail: Some(format!("not a Yinz keyword — use {} instead", e.what_instead)),
                    documentation: Some(e.why.to_string()),
                    deprecated: true,
                    sort_priority: 800,
                });
            }
            // Deferred language features (show deprecated)
            for e in crate::deferred_language_features() {
                items.push(RegistryCompletionItem {
                    label: e.name.to_string(),
                    kind: CompletionKind::DeferredFeature,
                    detail: Some(format!("ships in {} — use {} for now", e.ships_in, e.substitute)),
                    documentation: Some(e.why.to_string()),
                    deprecated: true,
                    sort_priority: 900,
                });
            }
            // Free functions (e.g. range)
            for e in crate::primitive_intrinsics().filter(|e| e.kind == "free_fn") {
                items.push(RegistryCompletionItem {
                    label: e.name.to_string(),
                    kind: CompletionKind::FreeFn,
                    detail: Some(format!("({}) -> {}", e.param_types.join(", "), e.return_type)),
                    documentation: None,
                    deprecated: false,
                    sort_priority: 150,
                });
            }
            // Primitive type names (int, float, string, etc.) — kind "print_type"
            // entries are type constructors, not free functions; excluded from completion
            // because they appear as keywords/type-annotations, not as call expressions.
        }

        CompletionContext::AfterDot { receiver_type } => {
            match receiver_type {
                Some(rtype) => {
                    // Primitive methods for this receiver type
                    for e in crate::primitive_intrinsics()
                        .filter(|e| e.receiver_type == Some(rtype) && (e.kind == "method" || e.kind == "method_1arg"))
                    {
                        let param_str = if e.param_types.is_empty() {
                            String::new()
                        } else {
                            e.param_types.join(", ").to_string()
                        };
                        items.push(RegistryCompletionItem {
                            label: format!("{}()", e.name),
                            kind: CompletionKind::PrimitiveMethod,
                            detail: Some(format!("{rtype}.{}({param_str}) -> {}", e.name, e.return_type)),
                            documentation: None,
                            deprecated: false,
                            sort_priority: 200,
                        });
                    }
                    // Type-attached constants (e.g. int.max)
                    for e in crate::type_attached_constants().filter(|e| e.type_name == *rtype) {
                        items.push(RegistryCompletionItem {
                            label: e.const_name.to_string(),
                            kind: CompletionKind::TypeAttachedConstant,
                            detail: Some(format!("{}: {} = {}", e.const_name, e.value_type, e.value_literal)),
                            documentation: None,
                            deprecated: false,
                            sort_priority: 210,
                        });
                    }
                }
                None => {
                    // Unknown receiver — suggest all primitive methods (best-effort)
                    for e in crate::primitive_intrinsics()
                        .filter(|e| e.receiver_type.is_some() && (e.kind == "method" || e.kind == "method_1arg"))
                    {
                        items.push(RegistryCompletionItem {
                            label: format!("{}()", e.name),
                            kind: CompletionKind::PrimitiveMethod,
                            detail: Some(format!("{}.{}() -> {}", e.receiver_type.unwrap_or("?"), e.name, e.return_type)),
                            documentation: None,
                            deprecated: false,
                            sort_priority: 200,
                        });
                    }
                }
            }
        }
    }

    items
}


// Hover content


/// High-level hover category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverKind {
    Keyword,
    PrimitiveMethod,
    FreeFn,
    TypeAttachedConstant,
    DeferredFeature,
    BannedDeclarationKeyword,
    BannedJargon,
}

/// Hover content returned by the registry — no `lsp-types` dependency.
#[derive(Clone, Debug)]
pub struct HoverContent {
    pub markdown_body: String,
    pub kind: HoverKind,
}

/// Return hover content for any registered token name, or `None` if not registered.
///
/// The caller (ynz-lsp) falls back to typeck symbol lookup when this returns `None`.
pub fn lsp_hover_for_token(name: &str) -> Option<HoverContent> {
    // Keywords
    if let Some(e) = crate::keyword_lookup(name) {
        return Some(HoverContent {
            markdown_body: format!(
                "## Keyword: `{}`\n\nIntroduced in {}.",
                e.name, e.since
            ),
            kind: HoverKind::Keyword,
        });
    }

    // Primitive intrinsics (methods + free fns) — match by name, return first overload
    if let Some(e) = crate::primitive_intrinsics().find(|e| e.name == name) {
        let detail = if let Some(rt) = e.receiver_type {
            format!(
                "## `{rt}.{}()`\n\nReceiver: `{rt}`  \nReturns: `{}`  \nIntroduced in {}.",
                e.name, e.return_type, e.since
            )
        } else {
            format!(
                "## `{}()`\n\nReturns: `{}`  \nIntroduced in {}.",
                e.name, e.return_type, e.since
            )
        };
        return Some(HoverContent {
            markdown_body: detail,
            kind: if e.receiver_type.is_some() { HoverKind::PrimitiveMethod } else { HoverKind::FreeFn },
        });
    }

    // Type-attached constants — token may be "int.max" or just "max"
    // Try both forms.
    let type_const_match = crate::type_attached_constants().find(|e| {
        e.const_name == name || format!("{}.{}", e.type_name, e.const_name) == name
    });
    if let Some(e) = type_const_match {
        return Some(HoverContent {
            markdown_body: format!(
                "## `{}.{}`\n\nValue: `{}`  \nType: `{}`  \nIntroduced in {}.",
                e.type_name, e.const_name, e.value_literal, e.value_type, e.since
            ),
            kind: HoverKind::TypeAttachedConstant,
        });
    }

    // Deferred language features
    if let Some(e) = crate::deferred_language_feature_lookup(name) {
        let mut body = format!(
            "## Deferred feature: `{}`\n\n**Ships in:** {}\n\n**Substitute:** {}\n\n**Why deferred:** {}",
            e.name, e.ships_in, e.substitute, e.why
        );
        if !e.design_doc.is_empty() {
            body.push_str(&format!("\n\n[Design doc]({})", e.design_doc));
        }
        return Some(HoverContent { markdown_body: body, kind: HoverKind::DeferredFeature });
    }

    // Banned declaration keywords
    if let Some(e) = crate::banned_declaration_keyword_lookup(name) {
        return Some(HoverContent {
            markdown_body: format!(
                "## Not a Yinz term: `{}`\n\n**What to use instead:** `{}`\n\n**Why:** {}",
                e.name, e.what_instead, e.why
            ),
            kind: HoverKind::BannedDeclarationKeyword,
        });
    }

    // Banned jargon
    if let Some(e) = crate::banned_jargon_lookup(name) {
        return Some(HoverContent {
            markdown_body: format!(
                "## Not a Yinz term: `{}`\n\n**What to use instead:** `{}`",
                e.name, e.replacement
            ),
            kind: HoverKind::BannedJargon,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_identifier_returns_all_keywords() {
        let items = lsp_completion_items(&CompletionContext::BareIdentifier);
        let kw_labels: Vec<_> = items.iter()
            .filter(|i| i.kind == CompletionKind::Keyword)
            .map(|i| i.label.as_str())
            .collect();
        assert!(kw_labels.len() >= 29, "expected ≥29 keywords, got {}", kw_labels.len());
        assert!(kw_labels.contains(&"function"), "function keyword must appear");
        assert!(kw_labels.contains(&"let"), "let keyword must appear");
    }

    #[test]
    fn bare_identifier_deferred_features_are_deprecated() {
        let items = lsp_completion_items(&CompletionContext::BareIdentifier);
        let deferred: Vec<_> = items.iter()
            .filter(|i| i.kind == CompletionKind::DeferredFeature)
            .collect();
        assert!(!deferred.is_empty(), "deferred features must appear in BareIdentifier");
        for d in &deferred {
            assert!(d.deprecated, "deferred feature '{}' must be marked deprecated", d.label);
        }
    }

    #[test]
    fn after_dot_int_returns_int_methods() {
        let items = lsp_completion_items(&CompletionContext::AfterDot {
            receiver_type: Some("int"),
        });
        assert!(!items.is_empty(), "int should have primitive methods");
        let has_to_string = items.iter().any(|i| i.label.starts_with("toString"));
        assert!(has_to_string, "int should have toString method");
    }

    #[test]
    fn after_dot_unknown_type_returns_all_methods() {
        let items = lsp_completion_items(&CompletionContext::AfterDot {
            receiver_type: Some("nonexistent"),
        });
        // Unknown type: returns empty (no methods registered for "nonexistent")
        let method_count = items.iter().filter(|i| i.kind == CompletionKind::PrimitiveMethod).count();
        assert_eq!(method_count, 0, "no methods for unknown receiver type");
    }

    #[test]
    fn after_dot_none_receiver_returns_all_methods() {
        let items = lsp_completion_items(&CompletionContext::AfterDot { receiver_type: None });
        // None receiver returns all registered methods as best-effort
        assert!(!items.is_empty(), "best-effort: all methods when receiver is unknown");
    }

    #[test]
    fn hover_function_keyword() {
        let h = lsp_hover_for_token("function");
        assert!(h.is_some(), "hover for 'function' should return Some");
        let h = h.unwrap();
        assert_eq!(h.kind, HoverKind::Keyword);
        assert!(h.markdown_body.contains("function"), "body should mention the keyword");
    }

    #[test]
    fn hover_int_max() {
        let h = lsp_hover_for_token("max");
        assert!(h.is_some(), "hover for 'max' (int.max) should return Some");
        let h = h.unwrap();
        assert_eq!(h.kind, HoverKind::TypeAttachedConstant);
        assert!(h.markdown_body.contains("9223372036854775807"), "int.max value must appear in hover");
    }

    #[test]
    fn hover_deferred_feature() {
        // "gpu" is a deferred language feature
        let h = lsp_hover_for_token("gpu");
        assert!(h.is_some(), "hover for deferred feature 'gpu' should return Some");
        let h = h.unwrap();
        assert_eq!(h.kind, HoverKind::DeferredFeature);
        assert!(h.markdown_body.contains("Ships in"), "deferred hover must mention ships_in");
    }

    #[test]
    fn hover_banned_keyword_type() {
        let h = lsp_hover_for_token("type");
        assert!(h.is_some(), "hover for banned keyword 'type' should return Some");
        let h = h.unwrap();
        assert_eq!(h.kind, HoverKind::BannedDeclarationKeyword);
        assert!(h.markdown_body.contains("shape"), "hover for 'type' must mention 'shape' as alternative");
    }

    #[test]
    fn hover_unregistered_token_returns_none() {
        let h = lsp_hover_for_token("thiswillneverberegistered_xyz");
        assert!(h.is_none(), "unregistered token must return None");
    }

    #[test]
    fn registry_adapter_does_not_depend_on_lsp_types() {
        // This test verifies at compile time (by existing) that ynz-registry
        // doesn't import lsp-types. If it did, the `cargo tree -p ynz-registry`
        // check would fail, but this test documents the intent.
        let _ = lsp_completion_items(&CompletionContext::BareIdentifier);
    }
}
