// Registry lsp_adapter functions must produce correct completion/hover data without
// depending on lsp-types. These tests verify the adapter independently from the LSP server,
// so regressions in registry data are caught before the LSP integration tests surface them.

use ynz_registry::{
    lsp_completion_items, lsp_hover_for_token, CompletionContext, CompletionKind, HoverKind,
};

// One named test per completion context a caller handles differently.
macro_rules! completion_case {
    ($name:ident, $case_bug:literal, $context:expr, $verdict:expr) => {
        // bug: lsp_completion_items offers the wrong set for a completion context (missing keywords, live deferred features, methods of the wrong receiver), so the editor completes invalid code | tier: Medium
        #[doc = $case_bug]
        #[test]
        fn $name() {
            let items = lsp_completion_items(&$context);
            let verdict: fn(&[ynz_registry::RegistryCompletionItem]) = $verdict;
            verdict(&items);
        }
    };
}

completion_case!(
    bare_identifier_offers_every_keyword,
    "A keyword missing from the bare-identifier list cannot be completed.",
    CompletionContext::BareIdentifier,
    |items| {
        let offered = items
            .iter()
            .filter(|item| item.kind == CompletionKind::Keyword)
            .count();
        assert_eq!(
            offered,
            ynz_registry::keywords().count(),
            "BareIdentifier must offer exactly the registry's keywords"
        );
    }
);

completion_case!(
    bare_identifier_deferred_features_are_deprecated_and_sort_after_keywords,
    "A deferred feature offered as live (or sorted above a keyword) invites unusable code.",
    CompletionContext::BareIdentifier,
    |items| {
        let deferred: Vec<_> = items
            .iter()
            .filter(|item| item.kind == CompletionKind::DeferredFeature)
            .collect();
        assert!(
            !deferred.is_empty(),
            "deferred features must appear in BareIdentifier"
        );
        let keyword_max_priority = items
            .iter()
            .filter(|item| item.kind == CompletionKind::Keyword)
            .map(|item| item.sort_priority)
            .max()
            .unwrap();
        for item in deferred {
            assert!(
                item.deprecated,
                "deferred feature '{}' must be deprecated",
                item.label
            );
            assert!(
                item.sort_priority >= 900 && item.sort_priority > keyword_max_priority,
                "deferred feature '{}' must sort after every keyword",
                item.label
            );
        }
    }
);

completion_case!(
    after_dot_known_type_offers_only_its_methods_and_constants,
    "Completing after `Int.` offers something that is not an Int method or constant.",
    CompletionContext::AfterDot {
        receiver_type: Some("Int")
    },
    |items| {
        assert!(!items.is_empty(), "AfterDot{{Int}} must return methods");
        for item in items {
            assert!(
                item.kind == CompletionKind::PrimitiveMethod
                    || item.kind == CompletionKind::TypeAttachedConstant,
                "unexpected item kind {:?} for Int receiver: {}",
                item.kind,
                item.label
            );
        }
    }
);

completion_case!(
    after_dot_unknown_type_offers_no_methods,
    "Completing after an unknown type offers methods of other types.",
    CompletionContext::AfterDot {
        receiver_type: Some("nonexistent_xyz")
    },
    |items| {
        let method_count = items
            .iter()
            .filter(|item| item.kind == CompletionKind::PrimitiveMethod)
            .count();
        assert_eq!(
            method_count, 0,
            "unknown receiver type should return no methods"
        );
    }
);

completion_case!(
    after_dot_without_receiver_type_offers_all_methods_best_effort,
    "Completing after a dot whose type is unknown offers nothing instead of the best-effort list.",
    CompletionContext::AfterDot {
        receiver_type: None
    },
    |items| assert!(
        !items.is_empty(),
        "best-effort: all methods when receiver is unknown"
    )
);

// One named test per registry list that must have a hover of the right kind for every entry.
// Runtime lists over the registry: kept as loops because `parallel_sweep` lives in ynz-driver
// (rubric 11.9).
macro_rules! hover_sweep_case {
    ($name:ident, $case_bug:literal, $entries:expr, $verdict:expr) => {
        // bug: a registry entry has no hover of its kind (or a hover missing its key content), so the editor shows nothing for a real token | tier: Medium
        #[doc = $case_bug]
        #[test]
        fn $name() {
            for entry in $entries {
                let hover = lsp_hover_for_token(entry.name)
                    .unwrap_or_else(|| panic!("registry token '{}' must have hover", entry.name));
                let verdict = $verdict;
                verdict(entry, hover);
            }
        }
    };
}

hover_sweep_case!(
    every_keyword_has_a_keyword_hover,
    "A keyword without hover content leaves the editor blank on it.",
    ynz_registry::keywords(),
    |_entry: &ynz_registry::KeywordEntry, hover: ynz_registry::HoverContent| assert_eq!(
        hover.kind,
        HoverKind::Keyword
    )
);

hover_sweep_case!(
    every_deferred_feature_hover_says_when_it_ships,
    "A deferred feature hover without its ship version hides when the feature arrives.",
    ynz_registry::deferred_language_features(),
    |entry: &ynz_registry::DeferredLanguageFeatureEntry, hover: ynz_registry::HoverContent| {
        assert!(
            hover.markdown_body.contains("Ships in"),
            "deferred '{}' hover must mention 'Ships in'",
            entry.name
        );
    }
);

hover_sweep_case!(
    every_banned_keyword_hover_names_what_instead,
    "A banned keyword hover without its replacement leaves the user with a refusal and no alternative.",
    ynz_registry::banned_declaration_keywords(),
    |entry: &ynz_registry::BannedDeclarationKeywordEntry, hover: ynz_registry::HoverContent| {
        assert!(
            hover.markdown_body.contains(entry.what_instead),
            "hover for '{}' must mention what_instead",
            entry.name
        );
    }
);

// One named test per token class whose hover kind is not covered by the registry sweeps above.
macro_rules! hover_token_case {
    ($name:ident, $case_bug:literal, $token:literal, $verdict:expr) => {
        // bug: lsp_hover_for_token classifies a token of this class under the wrong kind or returns content for an unregistered token | tier: Medium
        #[doc = $case_bug]
        #[test]
        fn $name() {
            let verdict: fn(Option<ynz_registry::HoverContent>) = $verdict;
            verdict(lsp_hover_for_token($token));
        }
    };
}

hover_token_case!(
    hover_banned_jargon_names_its_replacement,
    "A banned jargon term hovers without its replacement.",
    "propagate",
    |hover| {
        let hover = hover.expect("banned jargon 'propagate' must have hover content");
        assert_eq!(hover.kind, HoverKind::BannedJargon);
        assert!(
            hover.markdown_body.contains("auto-propagate"),
            "hover must include the replacement text"
        );
    }
);

hover_token_case!(
    hover_type_attached_constant_shows_its_value,
    "A type-attached constant (Int.max) hovers without its value.",
    "max",
    |hover| {
        let hover = hover.expect("hover for 'max' (Int.max) should return Some");
        assert_eq!(hover.kind, HoverKind::TypeAttachedConstant);
        assert!(
            hover.markdown_body.contains("9223372036854775807"),
            "Int.max value must appear in hover"
        );
    }
);

hover_token_case!(
    hover_unregistered_token_returns_none,
    "A token that is not in the registry hovers with invented content.",
    "_totally_unregistered_xyz_",
    |hover| assert!(hover.is_none())
);
