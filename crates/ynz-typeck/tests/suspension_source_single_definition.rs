//! R6 build-blocking tripwire — the may-block / suspension-source classification must have
//! exactly ONE authoritative definition in the workspace.
//!
//! # Why this test exists
//!
//! The base suspension-source intrinsic set was historically DEFINED TWICE (once in
//! `ynz-typeck`, once byte-for-byte copied in `ynz-codegen`) with no compile-time link — the exact
//! twin-computation drift class that shipped silent miscompiles across M3a/M3d/M3e/M3g (see
//! `.claude/rules/authoritative-derivation.md`). v0.3-M4 Phase 0 unified the two into a single
//! authoritative source (`ynz_typeck::suspension_source`). This test is the permanent tripwire that
//! fails the build if a SECOND hardcoded copy of the leaf-intrinsic list is ever reintroduced into
//! `ynz-typeck` or `ynz-codegen` source — defense-in-depth guarding every FUTURE consumer, not just
//! the ones threaded at unification time.
//!
//! It is deliberately a source-scanning drift tripwire (not a semantic equality assertion): after
//! unification there is only ONE definition, so a semantic "do the two copies agree?" check would be
//! tautological. The real failure mode this class produces is a NEW second copy appearing — which is
//! exactly what a source scan catches.

use std::fs;
use std::path::{Path, PathBuf};

/// The two crates whose source may consume — but must never re-define — the base suspension set.
const SCANNED_CRATE_SRC: &[&str] = &["../ynz-typeck/src", "../ynz-codegen/src"];

/// The ONE file allowed to contain the authoritative literal definition.
const AUTHORITATIVE_HOME: &str = "suspension_source.rs";

/// Max line-distance within which the two quoted leaf literals co-occurring counts as a re-derived
/// twin list. Same-line (distance 0) catches a single-line array literal AND a
/// `"sleep" | "__testFallibleAsync" => ...` match-arm re-derivation; the window catches a multi-line
/// array literal (one name per line) — the shape a routine `cargo fmt --all` reformat of a long
/// single-line literal produces, which the old same-line-with-`[` check silently missed. Kept well
/// below the ~20-line separation of the legitimate per-intrinsic *dispatch* arms in `check.rs`
/// (2462/2482) and `emit.rs` (11955/11978), which lower `sleep` and `__testFallibleAsync`
/// DIFFERENTLY (divergent codegen, not a membership list); verified false-positive-free against the
/// clean tree.
const TWIN_PROXIMITY_WINDOW: usize = 5;

/// The drift signature: the two quoted leaf intrinsic names co-occurring within
/// `TWIN_PROXIMITY_WINDOW` lines of each other — the twin-definition shape, whether written as a
/// single-line array literal (`&["sleep", "__testFallibleAsync"]`), a `cargo fmt`-reformatted
/// multi-line literal (one name per line), or a `"a" | "b" => ...` match arm. Keys ONLY on both
/// quoted string literals being present and close — no `[` co-occurrence required, so a reformat that
/// moves the bracket off the name lines can no longer defeat the tripwire. The literals are built
/// from fragments so this test's own source cannot itself match the pattern it scans for. Returns
/// the 1-based (sleep-line, test-line) pair on the first co-occurrence found.
fn find_twin_definition(contents: &str) -> Option<(usize, usize)> {
    let q = '"';
    let sleep_lit = format!("{q}sleep{q}");
    let test_lit = format!("{q}__testFallibleAsync{q}");
    let sleep_lines: Vec<usize> = contents
        .lines()
        .enumerate()
        .filter_map(|(i, l)| l.contains(&sleep_lit).then_some(i))
        .collect();
    for (t_idx, line) in contents.lines().enumerate() {
        if !line.contains(&test_lit) {
            continue;
        }
        if let Some(&s_idx) = sleep_lines
            .iter()
            .find(|&&s| s.abs_diff(t_idx) <= TWIN_PROXIMITY_WINDOW)
        {
            return Some((s_idx + 1, t_idx + 1));
        }
    }
    None
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn base_suspension_set_has_exactly_one_definition() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut offenders: Vec<String> = Vec::new();

    for src_rel in SCANNED_CRATE_SRC {
        let src_dir = Path::new(manifest_dir).join(src_rel);
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);

        for file in &files {
            if file.file_name().is_some_and(|n| n == AUTHORITATIVE_HOME) {
                continue;
            }
            let contents = fs::read_to_string(file).unwrap_or_default();
            if let Some((sleep_line, test_line)) = find_twin_definition(&contents) {
                offenders.push(format!(
                    "{} → \"sleep\"@L{} + \"__testFallibleAsync\"@L{} (co-occur within {} lines)",
                    file.display(),
                    sleep_line,
                    test_line,
                    TWIN_PROXIMITY_WINDOW,
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "R6 tripwire: a SECOND hardcoded copy of the base suspension-source intrinsic list was \
         found outside the authoritative home (`{AUTHORITATIVE_HOME}` — \
         `ynz_typeck::suspension_source::BASE_SUSPENSION_INTRINSICS`). This is the \
         twin-computation drift class that shipped silent miscompiles across M3a/M3d/M3e/M3g. \
         Thread `ynz_typeck::is_base_suspension_intrinsic` instead of re-deriving the list. \
         See `.claude/rules/authoritative-derivation.md`. Offending site(s):\n{}",
        offenders.join("\n")
    );
}

/// The authoritative home is actually present and non-empty — guards against the scan silently
/// passing because the source moved and the tripwire now scans nothing.
#[test]
fn authoritative_home_exists() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let home = Path::new(manifest_dir).join("src").join(AUTHORITATIVE_HOME);
    let contents = fs::read_to_string(&home)
        .unwrap_or_else(|_| panic!("authoritative home {} must exist", home.display()));
    assert!(
        contents.contains("BASE_SUSPENSION_INTRINSICS"),
        "authoritative home must define BASE_SUSPENSION_INTRINSICS"
    );
    // The classifier is reachable through the crate's public API — a compile-time guarantee that
    // consumers have a single exported home to thread.
    assert!(ynz_typeck::is_base_suspension_intrinsic("sleep"));
    assert!(!ynz_typeck::is_base_suspension_intrinsic("notAnIntrinsic"));
}
