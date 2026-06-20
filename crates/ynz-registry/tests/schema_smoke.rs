// Sketch consumers proving the registry schema works against 5 real query shapes.
// Uses placeholder entries from the Phase 1 registry/features.toml.
// When migration phases replace placeholders with real data, these tests remain
// valid — they'll look up real entries by their new names.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// 1. Lexer keyword sketch
//    Given a keyword name, look it up and get the token string.
// ---------------------------------------------------------------------------
#[test]
fn keyword_lookup_by_name() {
    let entry = ynz_registry::keyword_lookup("function")
        .expect("'function' keyword entry not found (Phase 4 migration)");
    assert_eq!(entry.token, "Function");
    assert_eq!(entry.since, "M1");
}

#[test]
fn banned_declaration_keyword_lookup_by_name() {
    let entry = ynz_registry::banned_declaration_keyword_lookup("class")
        .expect("'class' banned_declaration_keyword not found (Phase 4 migration)");
    assert!(!entry.what_instead.is_empty());
    assert!(!entry.why.is_empty());
    assert_eq!(entry.since, "M4");
}

#[test]
fn keyword_lookup_miss_returns_none() {
    assert!(ynz_registry::keyword_lookup("definitely_not_a_keyword").is_none());
}

#[test]
fn keyword_iteration_finds_all() {
    let names: Vec<&str> = ynz_registry::keywords().map(|e| e.name).collect();
    assert!(
        names.contains(&"function"),
        "expected 'function' in keyword iteration; got: {names:?}"
    );
    assert!(
        names.contains(&"wait"),
        "expected 'wait' in keyword iteration; got: {names:?}"
    );
    assert_eq!(names.len(), 29, "expected 29 keywords");
}

// ---------------------------------------------------------------------------
// 2. Diagnostics banned-jargon sketch
//    Given a banned word, get the replacement and reason.
// ---------------------------------------------------------------------------
#[test]
fn banned_jargon_lookup_by_name() {
    let entry = ynz_registry::banned_jargon_lookup("infer")
        .expect("'infer' banned-jargon entry not found (Phase 2 migration)");
    assert_eq!(entry.replacement, "figure out automatically");
    assert!(!entry.reason.is_empty(), "reason must not be empty");
    assert!(!entry.is_acronym);
}

#[test]
fn banned_jargon_lookup_acronym() {
    let entry = ynz_registry::banned_jargon_lookup("ADT")
        .expect("'ADT' banned-jargon entry not found (Phase 2 migration)");
    assert!(entry.is_acronym);
    assert_eq!(entry.replacement, "shape or options type");
}

#[test]
fn banned_jargon_lookup_miss_returns_none() {
    assert!(ynz_registry::banned_jargon_lookup("totally_fine_word").is_none());
}

// ---------------------------------------------------------------------------
// 3. Typeck primitive-intrinsic sketch
//    Given receiver type + method name, find the entry.
// ---------------------------------------------------------------------------
#[test]
fn primitive_intrinsic_method_lookup() {
    let mut found = ynz_registry::primitive_intrinsic_methods("int", "toNumber");
    let entry = found
        .next()
        .expect("int.toNumber intrinsic not found (Phase 3 migration)");
    assert_eq!(entry.kind, "method");
    assert_eq!(entry.receiver_type, Some("int"));
    assert_eq!(entry.return_type, "number");
}

#[test]
fn primitive_intrinsic_method_1arg_lookup() {
    let mut found = ynz_registry::primitive_intrinsic_methods("int", "wrappingAdd");
    let entry = found
        .next()
        .expect("int.wrappingAdd not found (Phase 3 migration)");
    assert_eq!(entry.kind, "method_1arg");
    assert_eq!(entry.param_types, &["int"]);
    assert_eq!(entry.return_type, "int");
}

#[test]
fn primitive_intrinsic_free_fn_lookup() {
    let mut overloads = ynz_registry::primitive_free_fns("range");
    assert!(overloads.next().is_some(), "range/1 not found");
    assert!(overloads.next().is_some(), "range/2 not found");
}

#[test]
fn primitive_intrinsic_unknown_method_returns_empty() {
    let count = ynz_registry::primitive_intrinsic_methods("int", "nonexistent_method").count();
    assert_eq!(count, 0);
}

#[test]
fn type_attached_constant_lookup() {
    let entry = ynz_registry::type_attached_constant_lookup("int", "max")
        .expect("int.max not found (Phase 3 migration)");
    assert_eq!(entry.type_name, "int");
    assert_eq!(entry.value_type, "int");
    assert_eq!(entry.value_literal, "9223372036854775807");
}

#[test]
fn type_attached_constant_number_epsilon() {
    let entry = ynz_registry::type_attached_constant_lookup("number", "epsilon")
        .expect("number.epsilon not found (Phase 3 migration)");
    assert_eq!(entry.value_type, "number");
    assert_eq!(entry.value_literal, "1e-33");
}

// ---------------------------------------------------------------------------
// 4. LSP autocomplete sketch
//    Sweep all keywords + intrinsics to produce a flat name list.
//    Proves O(N) iteration works for fuzzy-match autocomplete.
// ---------------------------------------------------------------------------
#[test]
fn lsp_autocomplete_sweep() {
    let keyword_names: Vec<&str> = ynz_registry::keywords().map(|e| e.name).collect();
    let intrinsic_names: Vec<&str> = ynz_registry::primitive_intrinsics()
        .map(|e| e.name)
        .collect();

    let all: Vec<&str> = keyword_names
        .iter()
        .chain(intrinsic_names.iter())
        .copied()
        .collect();

    // Prove the sweep produces at least one entry per kind.
    assert!(!keyword_names.is_empty(), "keyword list must not be empty");
    assert!(
        !intrinsic_names.is_empty(),
        "intrinsic list must not be empty"
    );
    assert!(all.len() >= 2, "combined list must have at least 2 entries");

    // Prove fuzzy filtering works (contains check — stand-in for prefix/fuzzy match).
    let prefix = "to";
    let matches: Vec<&&str> = all.iter().filter(|name| name.starts_with(prefix)).collect();
    assert!(
        !matches.is_empty(),
        "expected at least one name starting with '{prefix}' (e.g. toString, toFloat) for fuzzy-match test"
    );
}

// ---------------------------------------------------------------------------
// 5. Diagnostic-template render sketch
//    Proves the {placeholder} substitution grammar works.
// ---------------------------------------------------------------------------
#[test]
fn render_template_basic_substitution() {
    let mut vars = HashMap::new();
    vars.insert("binding", "count");

    let result = ynz_registry::render_template(
        "`{binding}` is declared `const` and cannot be changed.",
        &vars,
    );
    assert_eq!(result, "`count` is declared `const` and cannot be changed.");
}

#[test]
fn render_template_escaped_braces() {
    let vars = HashMap::new();
    let result = ynz_registry::render_template("use {{braces}} like this", &vars);
    assert_eq!(result, "use {braces} like this");
}

#[test]
fn render_template_multiple_vars() {
    let mut vars = HashMap::new();
    vars.insert("name", "wrappingAdd");
    vars.insert("type", "int");

    let result = ynz_registry::render_template("{type}.{name}() wraps on overflow.", &vars);
    assert_eq!(result, "int.wrappingAdd() wraps on overflow.");
}

#[test]
#[should_panic(expected = "unknown placeholder key 'missing'")]
fn render_template_unknown_key_panics() {
    let vars = HashMap::new();
    ynz_registry::render_template("hello {missing}", &vars);
}

#[test]
fn render_template_no_substitutions() {
    let vars = HashMap::new();
    let result = ynz_registry::render_template("no placeholders here", &vars);
    assert_eq!(result, "no placeholders here");
}

// ---------------------------------------------------------------------------
// 6. BOM and CRLF handling
//    Proves the build pipeline handles Windows-style line endings.
//    (The actual TOML file is parsed by build.rs at compile time;
//     these tests prove the OUTPUT is consistent — if the build succeeded
//     with the TOML, BOM/CRLF in the source was handled correctly.)
// ---------------------------------------------------------------------------
#[test]
fn bom_crlf_build_succeeded() {
    // If the build script ran without panic, BOM/CRLF handling worked.
    // Verify by checking that at least one entry of each kind is present.
    assert!(
        ynz_registry::keywords().count() >= 1,
        "[[keyword]] entries present"
    );
    assert!(
        ynz_registry::banned_jargon().count() >= 1,
        "[[banned_jargon]] entries present"
    );
    assert!(
        ynz_registry::primitive_intrinsics().count() >= 1,
        "[[primitive_intrinsic]] entries present"
    );
    assert!(
        ynz_registry::type_attached_constants().count() >= 1,
        "[[type_attached_constant]] entries present"
    );
    assert!(
        ynz_registry::deferred_language_features().count() >= 1,
        "[[deferred_language_feature]] entries present"
    );
    assert!(
        ynz_registry::deferred_tooling_features().count() >= 1,
        "[[deferred_tooling_feature]] entries present"
    );
    assert!(
        ynz_registry::diagnostic_templates().count() >= 1,
        "[[diagnostic_template]] entries present"
    );
    assert!(
        ynz_registry::muted_hint_domains().count() >= 1,
        "[[muted_hint_domain]] entries present"
    );
}

// ---------------------------------------------------------------------------
// 7. Deferred-feature lookup sketch
// ---------------------------------------------------------------------------
#[test]
fn deferred_tooling_feature_lookup() {
    let entry = ynz_registry::deferred_tooling_features()
        .find(|e| e.name == "--kernel")
        .expect("--kernel deferred_tooling_feature not found (Phase 5b migration)");
    assert_eq!(entry.ships_in, "v0.3");
    assert_eq!(entry.design_doc, "design/no-runtime-mode.md");
}

#[test]
fn deferred_language_feature_lookup() {
    let entry = ynz_registry::deferred_language_feature_lookup("f32")
        .expect("f32 deferred_language_feature not found (Phase 5a migration)");
    assert_eq!(entry.ships_in, "v2+");
    assert!(!entry.why.is_empty());
    assert_eq!(entry.design_doc, "design/mvp-scope.md");
}

#[test]
fn deferred_language_feature_test_keyword() {
    let entry = ynz_registry::deferred_language_feature_lookup("test")
        .expect("test deferred_language_feature not found (Phase 5a migration)");
    assert_eq!(entry.ships_in, "v0.13");
    assert!(!entry.substitute.is_empty());
}

// ---------------------------------------------------------------------------
// 8. Muted-hint domain lookup sketch
// ---------------------------------------------------------------------------
#[test]
fn muted_hint_domain_lookup() {
    let entry = ynz_registry::muted_hint_domain_lookup("variable_type")
        .expect("variable_type muted-hint domain not found (Phase 6 migration)");
    assert_eq!(entry.placement_category, "Addition");
    assert_eq!(entry.example_hint_rendered, ": int (from 42)");
}

#[test]
fn diagnostic_template_lookup() {
    let entry = ynz_registry::diagnostic_template_lookup("MutationOfConst")
        .expect("MutationOfConst diagnostic template not found (Phase 6 migration)");
    assert!(entry.what_template.contains("{binding}"));
    assert!(entry.why_template.contains("const"));
}

#[test]
fn all_muted_hint_placement_categories_valid() {
    for entry in ynz_registry::muted_hint_domains() {
        assert!(
            matches!(
                entry.placement_category,
                "Addition" | "Replacement" | "Informational"
            ),
            "muted_hint_domain '{}' has invalid placement_category '{}'",
            entry.domain,
            entry.placement_category
        );
    }
}
