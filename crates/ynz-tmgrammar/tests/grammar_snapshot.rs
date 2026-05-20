// WHY: Verifies that the committed ynz.tmLanguage.json matches what the generator
// produces from the current registry. If a keyword is added to registry/features.toml
// without regenerating the grammar, this test fails — enforcing the "registry-derived
// grammar, no drift" invariant.

use std::path::Path;

#[test]
fn committed_grammar_matches_generator_output() {
    let committed_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json");

    assert!(
        committed_path.exists(),
        "tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json must exist. \
         Run `cargo run -p ynz-tmgrammar` to generate it."
    );

    let committed_raw = std::fs::read_to_string(&committed_path).expect("read committed grammar");
    let committed: serde_json::Value =
        serde_json::from_str(&committed_raw).expect("parse committed grammar JSON");

    let generated = ynz_tmgrammar::build_grammar();

    // Compare canonical JSON values (key ordering doesn't matter)
    assert_eq!(
        canonical(&committed),
        canonical(&generated),
        "ynz.tmLanguage.json drifted from registry state. \
         Run `cargo run -p ynz-tmgrammar` from the repo root and commit the updated file."
    );
}

/// Recursively sort object keys to produce a canonical representation for
/// value-equality comparison. This avoids false negatives from key-order differences
/// between serde_json::to_string_pretty and the committed file's formatting.
fn canonical(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(k, _)| k.as_str());
            let sorted: serde_json::Map<_, _> = entries
                .into_iter()
                .map(|(k, v)| (k.clone(), canonical(v)))
                .collect();
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonical).collect())
        }
        other => other.clone(),
    }
}
