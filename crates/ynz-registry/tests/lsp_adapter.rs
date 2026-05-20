// WHY: Registry lsp_adapter functions must produce correct completion/hover data
// without depending on lsp-types. These tests verify the adapter independently
// from the LSP server, so regressions in registry data can be caught before
// the LSP integration tests surface them.

use ynz_registry::{
    CompletionContext, CompletionKind, HoverKind,
    lsp_completion_items, lsp_hover_for_token,
};

#[test]
fn bare_identifier_contains_all_keywords() {
    let items = lsp_completion_items(&CompletionContext::BareIdentifier);
    let count = items.iter().filter(|i| i.kind == CompletionKind::Keyword).count();
    assert!(count >= 29, "BareIdentifier must include all {count} keywords (expected ≥29)");
}

#[test]
fn deferred_features_are_deprecated_in_bare_identifier() {
    let items = lsp_completion_items(&CompletionContext::BareIdentifier);
    for item in items.iter().filter(|i| i.kind == CompletionKind::DeferredFeature) {
        assert!(item.deprecated, "deferred feature '{}' must be deprecated", item.label);
        assert!(item.sort_priority >= 900, "deferred features must sort last (priority ≥900)");
    }
}

#[test]
fn after_dot_int_returns_int_methods() {
    let items = lsp_completion_items(&CompletionContext::AfterDot { receiver_type: Some("int") });
    assert!(!items.is_empty(), "AfterDot{{int}} must return methods");
    for item in &items {
        // Every item must be a method/constant for int
        assert!(
            item.kind == CompletionKind::PrimitiveMethod || item.kind == CompletionKind::TypeAttachedConstant,
            "unexpected item kind {:?} for int receiver: {}", item.kind, item.label
        );
    }
}

#[test]
fn after_dot_unknown_type_returns_empty() {
    let items = lsp_completion_items(&CompletionContext::AfterDot {
        receiver_type: Some("nonexistent_xyz"),
    });
    let method_count = items.iter().filter(|i| i.kind == CompletionKind::PrimitiveMethod).count();
    assert_eq!(method_count, 0, "unknown receiver type should return no methods");
}

#[test]
fn hover_all_keyword_entries_return_some() {
    for e in ynz_registry::keywords() {
        let h = lsp_hover_for_token(e.name);
        assert!(h.is_some(), "keyword '{}' must have hover content", e.name);
        assert_eq!(h.unwrap().kind, HoverKind::Keyword);
    }
}

#[test]
fn hover_deferred_features_show_ships_in() {
    for e in ynz_registry::deferred_language_features() {
        let h = lsp_hover_for_token(e.name);
        assert!(h.is_some(), "deferred feature '{}' must have hover", e.name);
        let body = h.unwrap().markdown_body;
        assert!(
            body.contains("Ships in"),
            "deferred '{}' hover must mention 'Ships in'", e.name
        );
    }
}

#[test]
fn hover_banned_keywords_show_what_instead() {
    for e in ynz_registry::banned_declaration_keywords() {
        let h = lsp_hover_for_token(e.name);
        assert!(h.is_some(), "banned keyword '{}' must have hover", e.name);
        let body = h.unwrap().markdown_body;
        assert!(
            body.contains(&e.what_instead.to_string()),
            "hover for '{}' must mention what_instead", e.name
        );
    }
}

#[test]
fn hover_unregistered_returns_none() {
    assert!(lsp_hover_for_token("_totally_unregistered_xyz_").is_none());
}

#[test]
fn sort_priority_deferred_greater_than_keywords() {
    let bare = lsp_completion_items(&CompletionContext::BareIdentifier);
    let kw_max_prio = bare.iter()
        .filter(|i| i.kind == CompletionKind::Keyword)
        .map(|i| i.sort_priority)
        .max()
        .unwrap_or(0);
    let deferred_min_prio = bare.iter()
        .filter(|i| i.kind == CompletionKind::DeferredFeature)
        .map(|i| i.sort_priority)
        .min()
        .unwrap_or(u16::MAX);
    assert!(
        deferred_min_prio > kw_max_prio,
        "deferred features (priority {deferred_min_prio}) must sort after keywords (max {kw_max_prio})"
    );
}
