// WHY: Every file in design/future/ that describes a user-facing feature must have a
// corresponding registry entry so the LSP, docs generator, and error messages can read it.
// If a new future-design doc is added without a registry entry, the compiler has no way
// to render a consistent deferred-feature error when users accidentally use that syntax.
// This test catches the missing-entry direction. The registry→filesystem direction is also
// checked: every entry's design_doc field must reference an existing file.

use std::path::PathBuf;

fn design_future_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = .../crates/ynz-registry
    // parent() x2 → workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()  // crates/
        .parent().unwrap()  // workspace root
        .join("design")
        .join("future")
}

/// Files in design/future/ that intentionally have no registry entry.
/// Rationale for each in the comment.
const SKIP: &[(&str, &str)] = &[
    ("index.md",               "index file, not a feature doc"),
    ("inline-shape-types.md",  "shipped in v0.1-polish — no deferred entry needed"),
    ("auto-soa.md",            "codegen-only optimization, no user-facing token"),
    ("concurrency.md",         "covered by M8 wait/background keywords, no deferred token"),
    ("http-framework.md",      "deferred_stdlib_api kind, zero M1 entries per schema"),
    ("panic-safety.md",        "covered by errors keyword + panic isolation, no deferred token"),
    ("string-ptr-len-overhaul.md", "compiler-internal representation change, no user token"),
    ("supervisor.md",          "deferred_stdlib_api kind, zero M1 entries per schema"),
];

fn is_skipped(filename: &str) -> bool {
    SKIP.iter().any(|(name, _)| *name == filename)
}

/// Collect all registry entries (both deferred_language and deferred_tooling) for lookup.
fn deferred_design_docs() -> Vec<&'static str> {
    ynz_registry::deferred_language_features()
        .map(|e| e.design_doc)
        .chain(ynz_registry::deferred_tooling_features().map(|e| e.design_doc))
        .collect()
}

#[test]
fn every_future_doc_has_a_registry_entry_or_is_skipped() {
    let future_dir = design_future_dir();
    let design_docs = deferred_design_docs();

    let entries = std::fs::read_dir(&future_dir)
        .unwrap_or_else(|e| panic!("cannot read design/future/: {e}"));

    let mut missing = Vec::new();

    for entry in entries.flatten() {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        if !filename_str.ends_with(".md") {
            continue;
        }

        if is_skipped(&filename_str) {
            continue;
        }

        let design_doc_path = format!("design/future/{filename_str}");
        if !design_docs.iter().any(|doc| *doc == design_doc_path) {
            missing.push(format!(
                "design/future/{filename_str} has no registry entry — \
                 add a [[deferred_language_feature]] or [[deferred_tooling_feature]] entry \
                 to registry/features.toml with design_doc = {design_doc_path:?}, \
                 OR add it to the SKIP list in this test with a rationale"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "Missing registry entries for future design docs:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_registry_entry_design_doc_exists() {
    let workspace_root = design_future_dir()
        .parent().unwrap()  // design/
        .parent().unwrap()  // workspace root
        .to_path_buf();

    let mut broken = Vec::new();

    for entry in ynz_registry::deferred_language_features() {
        let path = workspace_root.join(entry.design_doc);
        if !path.exists() {
            broken.push(format!(
                "[[deferred_language_feature]] '{name}': design_doc = {doc:?} does not exist",
                name = entry.name,
                doc = entry.design_doc,
            ));
        }
    }

    for entry in ynz_registry::deferred_tooling_features() {
        let path = workspace_root.join(entry.design_doc);
        if !path.exists() {
            broken.push(format!(
                "[[deferred_tooling_feature]] '{name}': design_doc = {doc:?} does not exist",
                name = entry.name,
                doc = entry.design_doc,
            ));
        }
    }

    assert!(
        broken.is_empty(),
        "Registry entries with broken design_doc paths:\n{}",
        broken.join("\n")
    );
}

#[test]
fn skip_list_has_rationale_for_every_entry() {
    // Every skipped file must have a non-empty rationale.
    for (filename, rationale) in SKIP {
        assert!(
            !rationale.is_empty(),
            "SKIP entry {filename:?} has no rationale — explain why it doesn't need a registry entry"
        );
    }
}
