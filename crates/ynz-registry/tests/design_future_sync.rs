// WHY: Every "future design" doc that describes a user-facing feature must have a
// corresponding registry entry so the LSP, docs generator, and error messages can read it.
// If a new future-design doc is added without a registry entry, the compiler has no way
// to render a consistent deferred-feature error when users accidentally use that syntax.
// This test catches the missing-entry direction. The registry→filesystem direction is also
// checked: every entry's design_doc field must reference an existing file.
//
// Post-2026-07-01 docs/ taxonomy migration (see crates/ynz-registry/tests/schema_smoke.rs
// for the same migration's effect on registry design_doc values): the old design/future/
// directory (plus its nested design/future/gui/ subfolder) was flattened into
// docs/internal/scratchpad/ with every future-design file renamed to a SCRATCH-future-*
// prefix. That scratchpad directory is now SHARED with SCRATCH-stdlib-*.md and
// SCRATCH-open-questions.md, so this test filters to the SCRATCH-future- prefix to
// preserve the original "just the future/ docs" scan scope.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = .../crates/ynz-registry
    // parent() x2 → workspace root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .to_path_buf()
}

fn design_future_dir() -> PathBuf {
    workspace_root()
        .join("docs")
        .join("internal")
        .join("scratchpad")
}

const FUTURE_PREFIX: &str = "SCRATCH-future-";

/// Files in docs/internal/scratchpad/ (SCRATCH-future- prefixed) that intentionally have
/// no registry entry. Rationale for each in the comment.
const SKIP: &[(&str, &str)] = &[
    (
        "SCRATCH-future-designs-index.md",
        "index file, not a feature doc",
    ),
    (
        "SCRATCH-future-auto-soa.md",
        "codegen-only optimization, no user-facing token",
    ),
    (
        "SCRATCH-future-http-framework.md",
        "deferred_stdlib_api kind, zero M1 entries per schema",
    ),
    (
        "SCRATCH-future-panic-safety.md",
        "covered by errors keyword + panic isolation, no deferred token",
    ),
    (
        "SCRATCH-future-string-ptr-len-overhaul.md",
        "compiler-internal representation change, no user token",
    ),
    (
        "SCRATCH-future-supervisor.md",
        "deferred_stdlib_api kind, zero M1 entries per schema",
    ),
    (
        "SCRATCH-future-doc-generator.md",
        "parking-lot tooling direction (`ynz doc`), no concrete deferred user token yet",
    ),
    (
        "SCRATCH-future-macos-platform-support.md",
        "CI/infra deferral (macOS dropped from matrix), no user-facing token",
    ),
    (
        "SCRATCH-future-cross-module-frame-serialization.md",
        "shipped in v0.3-M3e — deferred_language_feature entry retired; file kept as design record",
    ),
    (
        "SCRATCH-future-array-by-value-element-storage.md",
        "shipped in v0.3-M5 — deferred_language_feature entry retired (ArrayShapeRuntimeFieldWithWait lifted); file kept as design record",
    ),
    (
        "SCRATCH-future-gui-index.md",
        "index file for the GUI design subfolder, not a feature doc (formerly design/future/gui/index.md)",
    ),
    (
        "SCRATCH-future-gui-architecture.md",
        "parking-lot far-future stdlib direction (webview-shell model, post-v0.5), no concrete deferred user token yet",
    ),
    (
        "SCRATCH-future-gui-build-targets.md",
        "parking-lot far-future stdlib direction (webview-shell model, post-v0.5), no concrete deferred user token yet",
    ),
    (
        "SCRATCH-future-gui-capabilities.md",
        "parking-lot far-future stdlib direction (webview-shell model, post-v0.5), no concrete deferred user token yet",
    ),
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
        .unwrap_or_else(|e| panic!("cannot read docs/internal/scratchpad/: {e}"));

    let mut missing = Vec::new();

    for entry in entries.flatten() {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        if !filename_str.ends_with(".md") {
            continue;
        }

        // Only the SCRATCH-future- prefixed files correspond to the old design/future/
        // scan scope — docs/internal/scratchpad/ is now shared with SCRATCH-stdlib-*.md
        // and SCRATCH-open-questions.md, which this test never covered.
        if !filename_str.starts_with(FUTURE_PREFIX) {
            continue;
        }

        if is_skipped(&filename_str) {
            continue;
        }

        let design_doc_path = format!("docs/internal/scratchpad/{filename_str}");
        if !design_docs.iter().any(|doc| *doc == design_doc_path) {
            missing.push(format!(
                "docs/internal/scratchpad/{filename_str} has no registry entry — \
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
    let workspace_root = workspace_root();

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
