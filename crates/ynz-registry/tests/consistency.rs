// WHY: Catches registry drift — malformed entries, missing required fields,
// broken cross-references, and invalid enum values. Each test enforces one
// schema invariant. If a test fails after editing registry/features.toml,
// the entry is malformed — fix the TOML, not the test.

use ynz_registry;

// ---------------------------------------------------------------------------
// [[banned_jargon]] invariants
// ---------------------------------------------------------------------------

#[test]
fn banned_jargon_all_have_replacement_and_reason() {
    let mut violations = Vec::new();
    for entry in ynz_registry::banned_jargon() {
        if entry.replacement.is_empty() {
            violations.push(format!("[[banned_jargon]] '{name}': replacement is empty", name = entry.name));
        }
        if entry.reason.is_empty() {
            violations.push(format!("[[banned_jargon]] '{name}': reason is empty", name = entry.name));
        }
    }
    assert!(violations.is_empty(), "banned_jargon invariant violations:\n{}", violations.join("\n"));
}

#[test]
fn banned_jargon_names_are_unique() {
    let mut names: Vec<&str> = ynz_registry::banned_jargon().map(|e| e.name).collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(), total,
        "banned_jargon has duplicate names — each name must be unique"
    );
}

// ---------------------------------------------------------------------------
// [[deferred_language_feature]] and [[deferred_tooling_feature]] invariants
// ---------------------------------------------------------------------------

#[test]
fn deferred_language_features_have_required_fields() {
    let mut violations = Vec::new();
    for entry in ynz_registry::deferred_language_features() {
        if entry.why.is_empty() {
            violations.push(format!("[[deferred_language_feature]] '{name}': why is empty", name = entry.name));
        }
        if entry.ships_in.is_empty() {
            violations.push(format!("[[deferred_language_feature]] '{name}': ships_in is empty", name = entry.name));
        }
        if entry.design_doc.is_empty() {
            violations.push(format!("[[deferred_language_feature]] '{name}': design_doc is empty", name = entry.name));
        }
        if entry.triggers.is_empty() {
            violations.push(format!("[[deferred_language_feature]] '{name}': triggers is empty", name = entry.name));
        }
    }
    assert!(violations.is_empty(), "deferred_language_feature invariant violations:\n{}", violations.join("\n"));
}

#[test]
fn deferred_tooling_features_have_required_fields() {
    let mut violations = Vec::new();
    for entry in ynz_registry::deferred_tooling_features() {
        if entry.why.is_empty() {
            violations.push(format!("[[deferred_tooling_feature]] '{name}': why is empty", name = entry.name));
        }
        if entry.ships_in.is_empty() {
            violations.push(format!("[[deferred_tooling_feature]] '{name}': ships_in is empty", name = entry.name));
        }
        if entry.design_doc.is_empty() {
            violations.push(format!("[[deferred_tooling_feature]] '{name}': design_doc is empty", name = entry.name));
        }
    }
    assert!(violations.is_empty(), "deferred_tooling_feature invariant violations:\n{}", violations.join("\n"));
}

// ---------------------------------------------------------------------------
// [[type_attached_constant]] invariants
// ---------------------------------------------------------------------------

#[test]
fn type_attached_constants_have_parseable_value_literals() {
    let mut violations = Vec::new();
    for entry in ynz_registry::type_attached_constants() {
        match entry.value_type {
            "int" => {
                if entry.value_literal.parse::<i64>().is_err() {
                    violations.push(format!(
                        "[[type_attached_constant]] {type_name}.{const_name}: \
                         value_literal {lit:?} is not a valid i64",
                        type_name = entry.type_name,
                        const_name = entry.const_name,
                        lit = entry.value_literal,
                    ));
                }
            }
            "float" => {
                if entry.value_literal.parse::<f64>().is_err() {
                    violations.push(format!(
                        "[[type_attached_constant]] {type_name}.{const_name}: \
                         value_literal {lit:?} is not a valid f64",
                        type_name = entry.type_name,
                        const_name = entry.const_name,
                        lit = entry.value_literal,
                    ));
                }
            }
            "number" => {
                // Decimal128 literals are opaque strings; just check non-empty.
                if entry.value_literal.is_empty() {
                    violations.push(format!(
                        "[[type_attached_constant]] {type_name}.{const_name}: value_literal is empty",
                        type_name = entry.type_name,
                        const_name = entry.const_name,
                    ));
                }
            }
            other => {
                violations.push(format!(
                    "[[type_attached_constant]] {type_name}.{const_name}: \
                     unknown value_type {other:?} — must be 'int', 'float', or 'number'",
                    type_name = entry.type_name,
                    const_name = entry.const_name,
                ));
            }
        }
    }
    assert!(violations.is_empty(), "type_attached_constant invariant violations:\n{}", violations.join("\n"));
}

// ---------------------------------------------------------------------------
// [[primitive_intrinsic]] invariants
// ---------------------------------------------------------------------------

#[test]
fn primitive_intrinsics_have_valid_kinds() {
    let valid_kinds = ["print_type", "free_fn", "method", "method_1arg"];
    let mut violations = Vec::new();
    for entry in ynz_registry::primitive_intrinsics() {
        if !valid_kinds.contains(&entry.kind) {
            violations.push(format!(
                "[[primitive_intrinsic]] '{name}': invalid kind {kind:?} — must be one of {valid_kinds:?}",
                name = entry.name,
                kind = entry.kind,
            ));
        }
        // method and method_1arg must have receiver_type
        if matches!(entry.kind, "method" | "method_1arg") && entry.receiver_type.is_none() {
            violations.push(format!(
                "[[primitive_intrinsic]] '{name}' (kind={kind}): missing receiver_type",
                name = entry.name,
                kind = entry.kind,
            ));
        }
        // method_1arg must have exactly 1 param_type
        if entry.kind == "method_1arg" && entry.param_types.len() != 1 {
            violations.push(format!(
                "[[primitive_intrinsic]] '{name}' (method_1arg): expected 1 param_type, got {}",
                entry.param_types.len(),
                name = entry.name,
            ));
        }
    }
    assert!(violations.is_empty(), "primitive_intrinsic invariant violations:\n{}", violations.join("\n"));
}

// ---------------------------------------------------------------------------
// [[muted_hint_domain]] invariants
// ---------------------------------------------------------------------------

#[test]
fn muted_hint_domains_have_valid_placement_categories() {
    let valid = ["Addition", "Replacement", "Informational"];
    let mut violations = Vec::new();
    for entry in ynz_registry::muted_hint_domains() {
        if !valid.contains(&entry.placement_category) {
            violations.push(format!(
                "[[muted_hint_domain]] '{domain}': invalid placement_category {cat:?}",
                domain = entry.domain,
                cat = entry.placement_category,
            ));
        }
        if entry.description.is_empty() {
            violations.push(format!("[[muted_hint_domain]] '{domain}': description is empty", domain = entry.domain));
        }
        if entry.example_source.is_empty() {
            violations.push(format!("[[muted_hint_domain]] '{domain}': example_source is empty", domain = entry.domain));
        }
        if entry.example_hint_rendered.is_empty() {
            violations.push(format!("[[muted_hint_domain]] '{domain}': example_hint_rendered is empty", domain = entry.domain));
        }
    }
    assert!(violations.is_empty(), "muted_hint_domain invariant violations:\n{}", violations.join("\n"));
}

// ---------------------------------------------------------------------------
// [[diagnostic_template]] invariants
// ---------------------------------------------------------------------------

#[test]
fn diagnostic_templates_have_all_three_parts() {
    let mut violations = Vec::new();
    for entry in ynz_registry::diagnostic_templates() {
        if entry.what_template.is_empty() {
            violations.push(format!("[[diagnostic_template]] '{kind_name}': what_template is empty", kind_name = entry.kind_name));
        }
        if entry.what_instead_template.is_empty() {
            violations.push(format!("[[diagnostic_template]] '{kind_name}': what_instead_template is empty", kind_name = entry.kind_name));
        }
        if entry.why_template.is_empty() {
            violations.push(format!("[[diagnostic_template]] '{kind_name}': why_template is empty", kind_name = entry.kind_name));
        }
    }
    assert!(violations.is_empty(), "diagnostic_template invariant violations:\n{}", violations.join("\n"));
}

// ---------------------------------------------------------------------------
// [[keyword]] and [[banned_declaration_keyword]] invariants
// ---------------------------------------------------------------------------

#[test]
fn keywords_have_populated_fields() {
    let mut violations = Vec::new();
    for entry in ynz_registry::keywords() {
        if entry.token.is_empty() {
            violations.push(format!("[[keyword]] '{name}': token is empty", name = entry.name));
        }
        if entry.since.is_empty() {
            violations.push(format!("[[keyword]] '{name}': since is empty", name = entry.name));
        }
    }
    assert!(violations.is_empty(), "keyword invariant violations:\n{}", violations.join("\n"));
}

#[test]
fn banned_declaration_keywords_have_teaching_text() {
    let mut violations = Vec::new();
    for entry in ynz_registry::banned_declaration_keywords() {
        if entry.what_instead.is_empty() {
            violations.push(format!("[[banned_declaration_keyword]] '{name}': what_instead is empty", name = entry.name));
        }
        if entry.why.is_empty() {
            violations.push(format!("[[banned_declaration_keyword]] '{name}': why is empty", name = entry.name));
        }
    }
    assert!(violations.is_empty(), "banned_declaration_keyword invariant violations:\n{}", violations.join("\n"));
}
