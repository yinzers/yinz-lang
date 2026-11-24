// WHY: The lexer's keyword match statement and the registry keyword list must stay in sync.
// If a keyword is added to the lexer but not the registry, the LSP autocomplete will be
// incomplete. If a keyword is in the registry but not the lexer, it will tokenize as an
// identifier. This test catches both directions of drift.
//
// Counts are pinned. When you add a keyword: add it to registry/features.toml AND bump the
// count here. The compile error from the count mismatch is the nudge that prevents forgetting.
//
// Protected keep: these tests were kept as they are (only their shape changed). Each
// keyword is its own named test so a failure names the missing keyword, and the two
// sweeps over every registry entry list every offender instead of stopping at the first.

// ---------------------------------------------------------------------------
// Expected counts — pin these; update both when adding keywords
// ---------------------------------------------------------------------------

// test-ratchet: `Nothing` is an ordinary type name, not a keyword: 29 - 1 = 28.
const EXPECTED_KEYWORD_COUNT: usize = 28;
const EXPECTED_BANNED_DECL_KEYWORD_COUNT: usize = 17;

/// One named test per keyword: the registry must list it among its keywords.
macro_rules! keyword_in_registry_cases {
    ($($case:ident: $keyword:expr;)+) => {$(
        // bug: a keyword the lexer recognises is missing from the registry, so LSP autocomplete omits it | tier: Medium
        #[test]
        fn $case() {
            let is_registered = ynz_registry::keywords().any(|entry| entry.name == $keyword);
            assert!(is_registered, "keyword {:?} missing from registry", $keyword);
        }
    )+};
}

/// One named test per banned declaration keyword: the registry must list it.
macro_rules! banned_keyword_in_registry_cases {
    ($($case:ident: $keyword:expr;)+) => {$(
        // bug: a banned declaration keyword is missing from the registry, so the parser cannot teach the user the Yinz alternative | tier: Medium
        #[test]
        fn $case() {
            let is_registered = ynz_registry::banned_declaration_keywords()
                .any(|entry| entry.name == $keyword);
            assert!(
                is_registered,
                "banned declaration keyword {:?} missing from registry",
                $keyword
            );
        }
    )+};
}

// ---------------------------------------------------------------------------
// Keyword registry completeness checks
// ---------------------------------------------------------------------------

// bug: a keyword was added to the lexer without being added to registry/features.toml (or the reverse) | tier: Medium
#[test]
fn keyword_count_matches_lexer() {
    let count = ynz_registry::keywords().count();
    assert_eq!(
        count, EXPECTED_KEYWORD_COUNT,
        "registry has {count} keywords but expected {EXPECTED_KEYWORD_COUNT}. \
         Did you add a keyword to lexer.rs without adding it to registry/features.toml? \
         Update both files and bump EXPECTED_KEYWORD_COUNT."
    );
}

keyword_in_registry_cases! {
    m1_keyword_function_in_registry: "function";
    m1_keyword_let_in_registry: "let";
    m1_keyword_const_in_registry: "const";
    m1_keyword_true_in_registry: "true";
    m1_keyword_false_in_registry: "false";
    m1_keyword_if_in_registry: "if";
    m1_keyword_else_in_registry: "else";
    m1_keyword_while_in_registry: "while";
    m1_keyword_for_in_registry: "for";
    m1_keyword_in_in_registry: "in";
    m1_keyword_return_in_registry: "return";
    m4_keyword_shape_in_registry: "shape";
    m4_keyword_follows_in_registry: "follows";
    m4_keyword_extends_in_registry: "extends";
    m4_keyword_base_in_registry: "base";
    m4_keyword_hidden_in_registry: "hidden";
    m4_keyword_dynamic_in_registry: "dynamic";
    m4_keyword_self_type_in_registry: "Self";
    m4_keyword_self_value_in_registry: "self";
    m5_through_m8_keyword_none_in_registry: "none";
    m5_through_m8_keyword_options_in_registry: "options";
    m5_through_m8_keyword_is_in_registry: "is";
    m5_through_m8_keyword_errors_in_registry: "errors";
    m5_through_m8_keyword_import_in_registry: "import";
    m5_through_m8_keyword_export_in_registry: "export";
    m5_through_m8_keyword_sensitive_in_registry: "sensitive";
    m5_through_m8_keyword_wait_in_registry: "wait";
    m5_through_m8_keyword_background_in_registry: "background";
}

// bug: a keyword entry has an empty token or since field, so LSP autocomplete shows a blank entry | tier: Medium
#[test]
fn keyword_token_fields_are_populated() {
    let offenders: Vec<String> = ynz_registry::keywords()
        .flat_map(|entry| {
            let empty_token = entry
                .token
                .is_empty()
                .then(|| format!("keyword {:?} has empty token field", entry.name));
            let empty_since = entry
                .since
                .is_empty()
                .then(|| format!("keyword {:?} has empty since field", entry.name));
            empty_token.into_iter().chain(empty_since)
        })
        .collect();
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

// ---------------------------------------------------------------------------
// Banned declaration keyword registry completeness checks
// ---------------------------------------------------------------------------

// bug: a banned declaration keyword was added or removed in the registry without updating the pinned count | tier: Medium
#[test]
fn banned_decl_keyword_count_matches_lexer() {
    let count = ynz_registry::banned_declaration_keywords().count();
    assert_eq!(
        count, EXPECTED_BANNED_DECL_KEYWORD_COUNT,
        "registry has {count} banned_declaration_keywords but expected \
         {EXPECTED_BANNED_DECL_KEYWORD_COUNT}. Update both registry and this count."
    );
}

banned_keyword_in_registry_cases! {
    oop_banned_keyword_type_in_registry: "type";
    oop_banned_keyword_struct_in_registry: "struct";
    oop_banned_keyword_class_in_registry: "class";
    oop_banned_keyword_interface_in_registry: "interface";
    oop_banned_keyword_enum_in_registry: "enum";
    oop_banned_keyword_abstract_in_registry: "abstract";
    concurrency_banned_keyword_async_in_registry: "async";
    concurrency_banned_keyword_await_in_registry: "await";
    concurrency_banned_keyword_promise_in_registry: "promise";
    concurrency_banned_keyword_future_in_registry: "future";
    concurrency_banned_keyword_goroutine_in_registry: "goroutine";
    visibility_banned_keyword_pub_in_registry: "pub";
    visibility_banned_keyword_private_in_registry: "private";
    visibility_banned_keyword_protected_in_registry: "protected";
    visibility_banned_keyword_public_in_registry: "public";
}

// bug: a banned declaration keyword has no what_instead or why text, so the diagnostic cannot teach the user | tier: Medium
#[test]
fn banned_decl_keywords_have_teaching_text() {
    let offenders: Vec<String> = ynz_registry::banned_declaration_keywords()
        .flat_map(|entry| {
            let missing_what_instead = entry.what_instead.is_empty().then(|| {
                format!(
                    "banned_declaration_keyword {:?} has empty what_instead — must tell the user what to do",
                    entry.name
                )
            });
            let missing_why = entry.why.is_empty().then(|| {
                format!(
                    "banned_declaration_keyword {:?} has empty why — must explain why Yinz chose differently",
                    entry.name
                )
            });
            missing_what_instead.into_iter().chain(missing_why)
        })
        .collect();
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}
