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
                    detail: Some(format!(
                        "not a Yinz keyword — use {} instead",
                        e.what_instead
                    )),
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
                    detail: Some(format!(
                        "ships in {} — use {} for now",
                        e.ships_in, e.substitute
                    )),
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
                    detail: Some(format!(
                        "({}) -> {}",
                        e.param_types.join(", "),
                        e.return_type
                    )),
                    documentation: None,
                    deprecated: false,
                    sort_priority: 150,
                });
            }
            // Primitive type names (Int, Float, String, etc.) — kind "print_type"
            // entries are type constructors, not free functions; excluded from completion
            // because they appear as keywords/type-annotations, not as call expressions.
        }

        CompletionContext::AfterDot { receiver_type } => {
            match receiver_type {
                Some(rtype) => {
                    // Strip generic params for matching: "Array<Int>" → "Array".
                    // Registry entries use the base type name (e.g. "Array", "Fixed")
                    // so they match any concrete instantiation.
                    let base_type = rtype.split('<').next().unwrap_or(rtype);

                    // Primitive methods for this receiver type
                    for e in crate::primitive_intrinsics().filter(|e| {
                        e.receiver_type == Some(base_type)
                            && (e.kind == "method" || e.kind == "method_1arg")
                    }) {
                        // For generic collections, replace placeholder "T" with the
                        // concrete element type extracted from the receiver (e.g. "Int").
                        let elem = rtype
                            .strip_prefix(&format!("{base_type}<"))
                            .and_then(|s| s.strip_suffix('>'))
                            .unwrap_or("T");
                        let return_display = e.return_type.replace('T', elem);
                        let param_str = e
                            .param_types
                            .iter()
                            .map(|p| p.replace('T', elem))
                            .collect::<Vec<_>>()
                            .join(", ");
                        items.push(RegistryCompletionItem {
                            label: format!("{}()", e.name),
                            kind: CompletionKind::PrimitiveMethod,
                            detail: Some(format!(
                                "{rtype}.{}({param_str}) -> {return_display}",
                                e.name,
                            )),
                            documentation: e.doc.map(|d| d.to_string()),
                            deprecated: false,
                            sort_priority: 200,
                        });
                    }
                    // Type-attached constants (e.g. int.max)
                    for e in crate::type_attached_constants().filter(|e| e.type_name == *rtype) {
                        items.push(RegistryCompletionItem {
                            label: e.const_name.to_string(),
                            kind: CompletionKind::TypeAttachedConstant,
                            detail: Some(format!(
                                "{}: {} = {}",
                                e.const_name, e.value_type, e.value_literal
                            )),
                            documentation: None,
                            deprecated: false,
                            sort_priority: 210,
                        });
                    }
                }
                None => {
                    // Unknown receiver — suggest all primitive methods (best-effort)
                    for e in crate::primitive_intrinsics().filter(|e| {
                        e.receiver_type.is_some() && (e.kind == "method" || e.kind == "method_1arg")
                    }) {
                        items.push(RegistryCompletionItem {
                            label: format!("{}()", e.name),
                            kind: CompletionKind::PrimitiveMethod,
                            detail: Some(format!(
                                "{}.{}() -> {}",
                                e.receiver_type.unwrap_or("?"),
                                e.name,
                                e.return_type
                            )),
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
        let markdown_body = if let (Some(what), Some(what_instead), Some(why)) =
            (e.hover_what, e.hover_what_instead, e.hover_why)
        {
            format!(
                "## Keyword: `{}`\n\n**WHAT:** {what}\n\n**WHAT INSTEAD:** {what_instead}\n\n**WHY:** {why}\n\nIntroduced in {}.",
                e.name, e.since
            )
        } else {
            format!("## Keyword: `{}`\n\nIntroduced in {}.", e.name, e.since)
        };
        return Some(HoverContent {
            markdown_body,
            kind: HoverKind::Keyword,
        });
    }

    // Primitive intrinsics (methods + free fns) — match by name, return first overload
    if let Some(e) = crate::primitive_intrinsics().find(|e| e.name == name) {
        let mut detail = if let Some(rt) = e.receiver_type {
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
        if let Some(doc) = e.doc {
            detail.push_str("\n\n");
            detail.push_str(doc);
        }
        // `param_ownership` (v0.3-M8 Phase 4, FRAGO 003): the hover states a `give`
        // parameter FROM the registry field — never from a typeck constant — so "send gives
        // its value" is data the build validated, not prose that can drift.
        if e.param_ownership.contains(&"give") {
            let words = e
                .param_ownership
                .iter()
                .zip(e.param_types.iter())
                .map(|(w, t)| format!("`{w} {t}`"))
                .collect::<Vec<_>>()
                .join(", ");
            detail.push_str(&format!(
                "\n\nOwnership: {words} — a `give` parameter is given away by the call, so the \
                 argument's binding cannot be used afterward (send `.copy()` to keep it)."
            ));
        }
        return Some(HoverContent {
            markdown_body: detail,
            kind: if e.receiver_type.is_some() {
                HoverKind::PrimitiveMethod
            } else {
                HoverKind::FreeFn
            },
        });
    }

    // Type-attached constants — token may be "Int.max" or just "max"
    // Try both forms.
    let type_const_match = crate::type_attached_constants()
        .find(|e| e.const_name == name || format!("{}.{}", e.type_name, e.const_name) == name);
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
        return Some(HoverContent {
            markdown_body: body,
            kind: HoverKind::DeferredFeature,
        });
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
                "## Not a Yinz term: `{}`\n\n**What to use instead:** `{}`\n\n**Why:** {}",
                e.name, e.replacement, e.reason
            ),
            kind: HoverKind::BannedJargon,
        });
    }

    None
}
