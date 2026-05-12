// WHY: object-file bytes are the reproducibility contract — IR text drifts on
// LLVM patch versions, object bytes do not. If this golden SHA-256 changes
// without an intentional compiler change, something in the codegen or LLVM
// backend changed silently.

use std::path::PathBuf;

use ynz_codegen::{codegen_query, sha256, CompiledArtifact};
use ynz_parser::{CompilerDb, SourceFile};

const FILE: &str = "hello.ynz";
const M1_SOURCE: &str = r#"function main() -> nothing { print("hello, yinz") }"#;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/__golden__")
}

fn triple_slug() -> String {
    // Use the target triple compiled into this binary.
    // For Linux x86_64: "x86_64-unknown-linux-gnu"
    // For macOS ARM:    "aarch64-apple-darwin"
    // We sanitise to a filesystem-safe name.
    std::env::consts::ARCH.to_string() + "-" + std::env::consts::OS
}

/// Load or update the golden SHA-256 for the current host triple.
fn load_golden(slug: &str) -> Option<[u8; 32]> {
    let path = golden_dir().join(format!("hello.{slug}.sha256"));
    let hex = std::fs::read_to_string(&path).ok()?;
    let hex = hex.trim();
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for i in 0..32 {
        bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

fn save_golden(slug: &str, hash: &[u8; 32]) {
    let path = golden_dir().join(format!("hello.{slug}.sha256"));
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(&path, hex).expect("failed to write golden hash");
}

fn run_codegen() -> CompiledArtifact {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), M1_SOURCE.to_string());
    let output = codegen_query(&db, sf);
    assert!(
        output.diagnostics.is_empty(),
        "Codegen must have no diagnostics for valid M1 source: {:#?}",
        output.diagnostics
    );
    output.artifact.clone()
}

// ─── LLVM version assertion ──────────────────────────────────────────────────

#[test]
fn llvm_version_is_18() {
    // WHY: the golden SHA-256 is valid only for a specific LLVM version.
    // If the LLVM version on the runner doesn't match 18, the hash will differ
    // and the test below will give confusing output.
    let (major, _minor, _patch) = inkwell::support::get_llvm_version();
    assert_eq!(
        major, 18,
        "LLVM version must be 18 for the golden hash to match. Found LLVM {major}. \
         See README.md for LLVM 18 install instructions."
    );
}

// ─── Object-file determinism (golden SHA-256) ────────────────────────────────

#[test]
fn object_file_sha256_matches_golden() {
    // WHY: this is the reproducibility contract. If the hash changes without
    // an intentional compiler change, codegen became non-deterministic.
    let artifact = run_codegen();
    let slug = triple_slug();

    match load_golden(&slug) {
        Some(expected) => {
            assert_eq!(
                artifact.sha256, expected,
                "SHA-256 of object file changed. If this is intentional, delete \
                 tests/__golden__/hello.{slug}.sha256 and re-run to regenerate it."
            );
        }
        None => {
            // First run on this host: write the golden.
            save_golden(&slug, &artifact.sha256);
            println!("INFO: wrote new golden hash for triple={slug}");
        }
    }
}

// ─── Cross-run determinism ───────────────────────────────────────────────────

#[test]
fn codegen_is_deterministic_across_two_runs() {
    // WHY: a second run with the same source must produce byte-identical output.
    // If it doesn't, the SHA-256 golden is useless as a stability signal.
    let db1 = CompilerDb::default();
    let sf1 = SourceFile::new(&db1, FILE.to_string(), M1_SOURCE.to_string());
    let artifact1 = codegen_query(&db1, sf1).artifact.clone();

    let db2 = CompilerDb::default();
    let sf2 = SourceFile::new(&db2, FILE.to_string(), M1_SOURCE.to_string());
    let artifact2 = codegen_query(&db2, sf2).artifact.clone();

    assert_eq!(
        artifact1.sha256, artifact2.sha256,
        "Two runs produced different SHA-256 hashes — codegen is not deterministic"
    );
    assert_eq!(
        artifact1.object_bytes, artifact2.object_bytes,
        "Two runs produced different object bytes — codegen is not deterministic"
    );
}

// ─── IR text snapshot (informational) ────────────────────────────────────────

#[test]
fn ir_text_snapshot() {
    // WHY: IR text diffs help spot codegen regressions during development.
    // This test is informational — a change here means "something about the IR
    // changed." The SHA-256 test above is the gate, not this one.
    let artifact = run_codegen();
    insta::assert_snapshot!("hello_ir", artifact.ir_text);
}

// ─── Object file is non-empty and module verify passed ──────────────────────

#[test]
fn object_file_is_non_empty() {
    // WHY: an empty object file means LLVM silently failed to emit anything.
    let artifact = run_codegen();
    assert!(
        !artifact.object_bytes.is_empty(),
        "Object file must be non-empty"
    );
}

// ─── SHA-256 implementation sanity check ─────────────────────────────────────

#[test]
fn sha256_of_empty_input_matches_known_value() {
    // WHY: if the SHA-256 implementation is broken, all golden hashes are
    // meaningless. The hash of empty input is an FIPS-published test vector.
    let expected = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(
        sha256(b""),
        expected,
        "SHA-256 of empty input must match FIPS test vector"
    );
}

#[test]
fn sha256_of_abc_matches_known_value() {
    // WHY: second FIPS test vector for "abc".
    let expected = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    assert_eq!(
        sha256(b"abc"),
        expected,
        "SHA-256 of \"abc\" must match FIPS test vector"
    );
}

// ─── Salsa: inkwell types don't leak past codegen ───────────────────────────

#[test]
fn codegen_query_returns_owned_bytes_not_inkwell_types() {
    // WHY: inkwell's Context is not Send+Sync. If it escapes the query, the
    // salsa cache becomes unsound under multithreading. Verify the output
    // type contains only owned data.
    let artifact = run_codegen();
    // If this compiles, the output type is Send + Sync.
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&artifact);
}
