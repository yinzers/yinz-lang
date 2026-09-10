// WHY: The lexer's keyword match statement and the registry keyword list must stay in sync.
// If a keyword is added to the lexer but not the registry, the LSP autocomplete will be
// incomplete. If a keyword is in the registry but not the lexer, it will tokenize as an
// identifier. This test catches both directions of drift.
//
// Counts are pinned. When you add a keyword: add it to registry/features.toml AND bump the
// count here. The compile error from the count mismatch is the nudge that prevents forgetting.

// ---------------------------------------------------------------------------
// Expected counts — pin these; update both when adding keywords
// ---------------------------------------------------------------------------

const EXPECTED_KEYWORD_COUNT: usize = 29;
const EXPECTED_BANNED_DECL_KEYWORD_COUNT: usize = 17;

// ---------------------------------------------------------------------------
// Keyword registry completeness checks
// ---------------------------------------------------------------------------

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

#[test]
fn all_m1_keywords_in_registry() {
    let names: Vec<&str> = ynz_registry::keywords().map(|e| e.name).collect();
    for kw in &[
        "function", "nothing", "let", "const", "true", "false", "if", "else", "while", "for", "in",
        "return",
    ] {
        assert!(
            names.contains(kw),
            "M1 keyword {kw:?} missing from registry"
        );
    }
}

#[test]
fn m4_keywords_in_registry() {
    let names: Vec<&str> = ynz_registry::keywords().map(|e| e.name).collect();
    for kw in &[
        "shape", "follows", "extends", "base", "hidden", "dynamic", "Self", "self",
    ] {
        assert!(
            names.contains(kw),
            "M4 keyword {kw:?} missing from registry"
        );
    }
}

#[test]
fn m5_through_m8_keywords_in_registry() {
    let names: Vec<&str> = ynz_registry::keywords().map(|e| e.name).collect();
    for kw in &[
        "none",
        "options",
        "is",
        "errors",
        "import",
        "export",
        "sensitive",
        "wait",
        "background",
    ] {
        assert!(names.contains(kw), "keyword {kw:?} missing from registry");
    }
}

#[test]
fn keyword_token_fields_are_populated() {
    for entry in ynz_registry::keywords() {
        assert!(
            !entry.token.is_empty(),
            "keyword {:?} has empty token field",
            entry.name
        );
        assert!(
            !entry.since.is_empty(),
            "keyword {:?} has empty since field",
            entry.name
        );
    }
}

// ---------------------------------------------------------------------------
// Banned declaration keyword registry completeness checks
// ---------------------------------------------------------------------------

#[test]
fn banned_decl_keyword_count_matches_lexer() {
    let count = ynz_registry::banned_declaration_keywords().count();
    assert_eq!(
        count, EXPECTED_BANNED_DECL_KEYWORD_COUNT,
        "registry has {count} banned_declaration_keywords but expected \
         {EXPECTED_BANNED_DECL_KEYWORD_COUNT}. Update both registry and this count."
    );
}

#[test]
fn oop_banned_keywords_in_registry() {
    let names: Vec<&str> = ynz_registry::banned_declaration_keywords()
        .map(|e| e.name)
        .collect();
    for kw in &["type", "struct", "class", "interface", "enum", "abstract"] {
        assert!(
            names.contains(kw),
            "OOP banned keyword {kw:?} missing from registry"
        );
    }
}

#[test]
fn concurrency_banned_keywords_in_registry() {
    let names: Vec<&str> = ynz_registry::banned_declaration_keywords()
        .map(|e| e.name)
        .collect();
    for kw in &["async", "await", "promise", "future", "goroutine"] {
        assert!(
            names.contains(kw),
            "concurrency banned keyword {kw:?} missing from registry"
        );
    }
}

#[test]
fn visibility_banned_keywords_in_registry() {
    let names: Vec<&str> = ynz_registry::banned_declaration_keywords()
        .map(|e| e.name)
        .collect();
    for kw in &["pub", "private", "protected", "public"] {
        assert!(
            names.contains(kw),
            "visibility banned keyword {kw:?} missing from registry"
        );
    }
}

#[test]
fn banned_decl_keywords_have_teaching_text() {
    for entry in ynz_registry::banned_declaration_keywords() {
        assert!(
            !entry.what_instead.is_empty(),
            "banned_declaration_keyword {:?} has empty what_instead — must tell the user what to do",
            entry.name
        );
        assert!(
            !entry.why.is_empty(),
            "banned_declaration_keyword {:?} has empty why — must explain why Yinz chose differently",
            entry.name
        );
    }
}
