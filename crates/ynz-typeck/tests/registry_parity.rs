// WHY: builtins.rs and registry/features.toml are two parallel sources of truth for
// what methods exist on built-in types. Without a mechanical check they drift silently —
// methods typecheck correctly but never appear in IDE autocomplete or hover. This test
// reads builtins.rs SOURCE CODE at test time (not a hardcoded list) and asserts every
// match-arm method name is in the registry.
//
// Hard-coded lists go stale. Reading the source doesn't.
//
// If this test fails: add the missing method to registry/features.toml under the
// appropriate [[primitive_intrinsic]] block. Do NOT add it to CARVE_OUTS to silence a
// real gap — fix the registry.
//
// Adding a NEW method to a builtins.rs match arm: the test will fail until you add the
// registry entry. That's the point — failure is the signal to update the registry.

use std::collections::{HashMap, HashSet};

/// Parse a `fn foo_method_return(...)` function body from `builtins.rs` and extract
/// every string literal that appears as a match arm pattern.
///
/// We read the ACTUAL SOURCE FILE so adding a method to the match arm in builtins.rs
/// automatically makes this test fail until the registry entry is added. No hardcoded lists.
fn extract_match_arm_strings(src: &str, fn_name: &str) -> HashSet<String> {
    // Find function start
    let marker = format!("fn {}(", fn_name);
    let Some(fn_start) = src.find(&marker) else {
        return HashSet::new();
    };

    // Find function end: first top-level `\n}` after fn_start
    let body_src = &src[fn_start..];
    let fn_end = body_src
        .find("\n}")
        .map(|i| i + 2)
        .unwrap_or(body_src.len());
    let body = &body_src[..fn_end];

    let mut results = HashSet::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Find opening quote
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        // Scan to closing quote, respecting backslash escapes
        while j < len && bytes[j] != b'"' {
            if bytes[j] == b'\\' {
                j += 1; // skip escaped char
            }
            j += 1;
        }
        if j >= len {
            break;
        }
        let literal = &body[start..j];
        i = j + 1;

        // Only keep plausible method names: non-empty, pure alphanumeric/underscore,
        // camelCase identifiers — filter out diagnostic message strings.
        if literal.is_empty()
            || literal.len() > 40
            || literal.contains(' ')
            || literal.contains('\n')
            || literal.contains('`')
            || literal.contains('<')
        {
            continue;
        }
        if !literal.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }

        // Confirm this literal is in a match-arm position: the text immediately following
        // the closing quote (after optional whitespace, `|`, more quoted strings) must
        // contain `=>`.  Walk forward up to 120 chars.
        let after = &body[i..];
        let window: &str = &after[..after.len().min(120)];
        // Strip whitespace, pipes, more quoted alternatives
        let mut k = 0;
        let wbytes = window.as_bytes();
        while k < wbytes.len() {
            match wbytes[k] {
                b' ' | b'\t' | b'\n' | b'|' => k += 1,
                b'"' => {
                    // skip another alternative string
                    k += 1;
                    while k < wbytes.len() && wbytes[k] != b'"' {
                        if wbytes[k] == b'\\' { k += 1; }
                        k += 1;
                    }
                    k += 1;
                }
                _ => break,
            }
        }
        if window[k..].starts_with("=>") {
            results.insert(literal.to_string());
        }
    }

    results
}

fn registry_methods_by_receiver() -> HashMap<String, HashSet<String>> {
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    for e in ynz_registry::primitive_intrinsics() {
        if !matches!(e.kind, "method" | "method_1arg") {
            continue;
        }
        let Some(recv) = e.receiver_type else { continue };
        map.entry(recv.to_string())
            .or_default()
            .insert(e.name.to_string());
    }
    map
}

/// Methods in builtins.rs match arms that are intentionally NOT in the registry.
/// Each must have a comment. This list should be tiny — the normal path is to fix the
/// registry, not add carve-outs.
fn carve_outs() -> HashMap<&'static str, HashSet<&'static str>> {
    let mut m: HashMap<&'static str, HashSet<&'static str>> = HashMap::new();

    // sensitive_method_return: sensitive<T> is a wrapper that re-uses the inner type's
    // methods (all string methods) at the type-checker level. There is no separate
    // `receiver_type = "sensitive"` registry block because the methods are identical to
    // string's. A future dedicated block can be added if the IDE needs to show them
    // separately. Until then, carve them out to avoid false failures.
    m.entry("sensitive_method_return").or_default().extend([
        "reveal", "contains", "startsWith", "endsWith", "indexOf",
        "byteAt", "get", "count", "byteCount", "graphemeCount",
        "toUpperCase", "toLowerCase", "trim", "substring", "replace",
        "graphemeAt", "split",
    ]);

    m
}

#[test]
// WHY: Every method accepted by the type checker must also appear in the registry
// so the IDE can surface it in autocomplete and hover. A method missing from the
// registry compiles fine but is invisible to the developer — they discover it only
// by reading Rust source, which defeats the entire teaching mission.
//
// This test reads builtins.rs source at test time so it self-updates when new match
// arms are added. No hardcoded lists.
fn all_builtin_methods_are_registered() {
    let builtins_src = include_str!("../src/builtins.rs");
    let registry = registry_methods_by_receiver();
    let carve_out_map = carve_outs();

    // (builtins.rs fn name, registry receiver_type base)
    let cases: &[(&str, &str)] = &[
        ("array_method_return",      "array"),
        ("fixed_method_return",      "fixed"),
        ("maybe_method_return",      "maybe"),
        ("map_method_return",        "map"),
        ("string_method_return",     "string"),
        ("sensitive_method_return",  "sensitive"),
    ];

    let mut failures: Vec<String> = Vec::new();

    for &(fn_name, receiver) in cases {
        let builtin_methods = extract_match_arm_strings(builtins_src, fn_name);
        if builtin_methods.is_empty() {
            failures.push(format!(
                "  Could not extract any methods from `{fn_name}` — \
                 check that the function name matches exactly"
            ));
            continue;
        }

        let registered = registry.get(receiver).cloned().unwrap_or_default();
        let carved = carve_out_map.get(fn_name).cloned().unwrap_or_default();

        for method in &builtin_methods {
            if carved.contains(method.as_str()) {
                continue;
            }
            if !registered.contains(method) {
                failures.push(format!(
                    "  [{fn_name}] `{method}` — add to registry: \
                     receiver_type = \"{receiver}\", name = \"{method}\""
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "Registry parity failure — builtins.rs methods missing from registry/features.toml:\n\
         {}\n\n\
         Fix: add [[primitive_intrinsic]] entries. \
         Only add to carve_outs() for methods that are genuinely intentional omissions.",
        failures.join("\n")
    );
}

#[test]
// WHY: Confirms the skip guard in intrinsics.rs correctly excludes generic collection
// receiver types from the scalar intrinsic dispatch table. Without the guard, adding
// a generic receiver entry to the registry panics at startup.
fn generic_collection_receivers_dont_panic_intrinsic_table() {
    // Panics at construction if the guard is absent.
    let _table = ynz_typeck::PrimitiveIntrinsicTable::m6();
}
