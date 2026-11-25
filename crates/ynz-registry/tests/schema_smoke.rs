// Sketch consumers proving the registry schema works against real query shapes: each lookup
// the compiler, LSP and docs make finds the entry build.rs generated, with its fields mapped
// to the right slots, and misses cleanly.

use std::collections::HashMap;

// bug: keyword_lookup returns the wrong entry for a real keyword, or an entry for a non-keyword, so the lexer misclassifies tokens | tier: Medium
#[test]
fn keyword_lookup_by_name() {
    let entry = ynz_registry::keyword_lookup("function")
        .expect("'function' keyword entry not found (Phase 4 migration)");
    assert_eq!(entry.token, "Function");
    assert_eq!(entry.since, "M1");

    assert!(ynz_registry::keyword_lookup("definitely_not_a_keyword").is_none());
}

// bug: banned_declaration_keyword_lookup misses a banned keyword (`class`), so users get no teaching rejection | tier: Medium
#[test]
fn banned_declaration_keyword_lookup_by_name() {
    let entry = ynz_registry::banned_declaration_keyword_lookup("class")
        .expect("'class' banned_declaration_keyword not found (Phase 4 migration)");
    assert!(!entry.what_instead.is_empty());
    assert!(!entry.why.is_empty());
    assert_eq!(entry.since, "M4");
}

// bug: banned_jargon_lookup returns the wrong replacement, drops the is_acronym flag, or invents an entry for a fine word | tier: Medium
#[test]
fn banned_jargon_lookup_by_name() {
    let entry = ynz_registry::banned_jargon_lookup("infer")
        .expect("'infer' banned-jargon entry not found (Phase 2 migration)");
    assert_eq!(entry.replacement, "figure out automatically");
    assert!(!entry.reason.is_empty(), "reason must not be empty");
    assert!(!entry.is_acronym);

    let acronym = ynz_registry::banned_jargon_lookup("ADT")
        .expect("'ADT' banned-jargon entry not found (Phase 2 migration)");
    assert!(acronym.is_acronym);
    assert_eq!(acronym.replacement, "shape or options type");

    assert!(ynz_registry::banned_jargon_lookup("totally_fine_word").is_none());
}

// bug: primitive_intrinsic_methods maps kind, receiver, parameter or return types to the wrong slots, or returns entries for an unknown method, so typeck resolves the wrong intrinsic | tier: Medium
#[test]
fn primitive_intrinsic_methods_lookup() {
    let entry = ynz_registry::primitive_intrinsic_methods("Int", "toNumber")
        .next()
        .expect("Int.toNumber intrinsic not found (Phase 3 migration)");
    assert_eq!(entry.kind, "method");
    assert_eq!(entry.receiver_type, Some("Int"));
    assert_eq!(entry.return_type, "Number");

    let one_argument = ynz_registry::primitive_intrinsic_methods("Int", "wrappingAdd")
        .next()
        .expect("Int.wrappingAdd not found (Phase 3 migration)");
    assert_eq!(one_argument.kind, "method_1arg");
    assert_eq!(one_argument.param_types, &["Int"]);
    assert_eq!(one_argument.return_type, "Int");

    assert_eq!(
        ynz_registry::primitive_intrinsic_methods("Int", "nonexistent_method").count(),
        0
    );
}

// bug: primitive_free_fns drops an overload of a free function (`range` has 1- and 2-argument forms), so typeck rejects a valid call | tier: Medium
#[test]
fn primitive_intrinsic_free_fn_lookup() {
    let mut overloads = ynz_registry::primitive_free_fns("range");
    assert!(overloads.next().is_some(), "range/1 not found");
    assert!(overloads.next().is_some(), "range/2 not found");
}

// bug: type_attached_constant_lookup returns the wrong literal or value type for a constant (Int.max, Number.epsilon), so codegen emits the wrong value | tier: Medium
#[test]
fn type_attached_constant_lookup() {
    let entry = ynz_registry::type_attached_constant_lookup("Int", "max")
        .expect("Int.max not found (Phase 3 migration)");
    assert_eq!(entry.type_name, "Int");
    assert_eq!(entry.value_type, "Int");
    assert_eq!(entry.value_literal, "9223372036854775807");

    let epsilon = ynz_registry::type_attached_constant_lookup("Number", "epsilon")
        .expect("Number.epsilon not found (Phase 3 migration)");
    assert_eq!(epsilon.value_type, "Number");
    assert_eq!(epsilon.value_literal, "1e-33");
}

// bug: render_template leaves a placeholder unsubstituted or mishandles an escaped brace, so diagnostics show raw `{name}` text or lose literal braces | tier: Medium
#[test]
fn render_template_substitutes_placeholders_and_unescapes_braces() {
    let mut vars = HashMap::new();
    vars.insert("binding", "count");
    assert_eq!(
        ynz_registry::render_template(
            "`{binding}` is declared `const` and cannot be changed.",
            &vars
        ),
        "`count` is declared `const` and cannot be changed."
    );

    assert_eq!(
        ynz_registry::render_template("use {{braces}} like this", &HashMap::new()),
        "use {braces} like this"
    );
}

// bug: render_template silently renders an unknown placeholder instead of failing, so a typo in features.toml ships as broken diagnostic text | tier: Medium
#[test]
#[should_panic(expected = "unknown placeholder key 'missing'")]
fn render_template_unknown_key_panics() {
    let vars = HashMap::new();
    ynz_registry::render_template("hello {missing}", &vars);
}

// bug: build.rs generates no entries for a registry kind (BOM/CRLF or parse regression), so every lookup of that kind misses | tier: Medium
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

// bug: deferred_language_feature_lookup returns the wrong ship version, design doc or substitute for a deferred feature, so the deferred-feature error points users at the wrong release | tier: Medium
#[test]
fn deferred_language_feature_lookup() {
    let entry = ynz_registry::deferred_language_feature_lookup("f32")
        .expect("f32 deferred_language_feature not found (Phase 5a migration)");
    assert_eq!(entry.ships_in, "v2+");
    assert!(!entry.why.is_empty());
    assert!(!entry.substitute.is_empty());
    assert_eq!(entry.design_doc, "docs/reference/REF-mvp-scope.md");
}

// bug: muted_hint_domain_lookup returns the wrong placement category or rendered example for a hint domain, so the inlay-hint docs misdescribe it | tier: Medium
#[test]
fn muted_hint_domain_lookup() {
    let entry = ynz_registry::muted_hint_domain_lookup("variable_type")
        .expect("variable_type muted-hint domain not found (Phase 6 migration)");
    assert_eq!(entry.placement_category, "Addition");
    assert_eq!(entry.example_hint_rendered, ": Int (from 42)");
}

// bug: diagnostic_template_lookup misses a template or maps its WHAT/WHY parts to the wrong slots, so the diagnostic renders the wrong text | tier: Medium
#[test]
fn diagnostic_template_lookup() {
    let entry = ynz_registry::diagnostic_template_lookup("MutationOfConst")
        .expect("MutationOfConst diagnostic template not found (Phase 6 migration)");
    assert!(entry.what_template.contains("{binding}"));
    assert!(entry.why_template.contains("const"));
}

// bug: lint_rule_lookup returns the wrong severity or since for a rule, or an entry for an unknown rule, so a lint fires at the wrong level | tier: Medium
#[test]
fn lint_rule_lookup_by_name() {
    let entry = ynz_registry::lint_rule_lookup("prefer-yielding-sleep")
        .expect("'prefer-yielding-sleep' lint_rule entry not found");
    assert_eq!(entry.severity, "suggestion");
    assert_eq!(entry.since, "v0.3-M4");
    assert!(!entry.description.is_empty());
    assert!(!entry.design_doc.is_empty());

    assert!(ynz_registry::lint_rule_lookup("definitely-not-a-rule").is_none());
}

// A lint is a teaching surface, never a gate: build.rs rejects "error" at parse time; this
// locks the invariant against a build.rs regression. Registry consistency keep (rubric 4); a
// runtime list over registry entries, kept as a loop because `parallel_sweep` lives in
// ynz-driver (rubric 11.9).
// bug: a [[lint_rule]] with severity error slips past build.rs, so a lint blocks builds | tier: Medium
#[test]
fn lint_rule_severity_is_never_error() {
    for entry in ynz_registry::lint_rules() {
        assert!(
            matches!(entry.severity, "suggestion" | "warning"),
            "lint_rule '{}' has severity '{}' — a lint may never be an error",
            entry.name,
            entry.severity
        );
    }
}

// This is the GENERIC firing-site contract: any rule fires through exactly this call with its
// own vars.
// bug: lint_rule_diagnostic_parts leaves a {placeholder} unsubstituted in WHAT or WHY, so the lint shows raw template text | tier: Medium
#[test]
fn lint_rule_diagnostic_parts_render_placeholders() {
    let vars: HashMap<&str, &str> =
        HashMap::from([("shape", "Tally"), ("reason", "it is exported")]);
    let (what, what_instead, why) =
        ynz_registry::lint_rule_diagnostic_parts("cross-thread-fields-not-padded", &vars)
            .expect("parts for cross-thread-fields-not-padded");
    assert!(
        what.contains("`Tally`"),
        "WHAT must substitute {{shape}}; got: {what}"
    );
    assert!(!what_instead.is_empty());
    assert!(
        why.contains("it is exported"),
        "WHY must substitute {{reason}}; got: {why}"
    );
}

// bug: lsp_lint_rule_hover_for returns no hover for a real lint rule or invents one for an unknown rule | tier: Medium
#[test]
fn lint_rule_lsp_hover_exists_for_rule_names_only() {
    let hover = ynz_registry::lsp_lint_rule_hover_for("prefer-yielding-sleep")
        .expect("hover for prefer-yielding-sleep");
    assert!(hover.contains("prefer-yielding-sleep"));
    assert!(ynz_registry::lsp_lint_rule_hover_for("not-a-rule").is_none());
}
