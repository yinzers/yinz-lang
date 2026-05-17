// WHY: object-file bytes are the reproducibility contract — IR text drifts on
// LLVM patch versions, object bytes do not. If this golden SHA-256 changes
// without an intentional compiler change, something in the codegen or LLVM
// backend changed silently.

use std::path::PathBuf;

use ynz_codegen::{codegen_query, sha256, CompiledArtifact};
use ynz_parser::{CompilerDb, SourceFile};

const FILE: &str = "hello.ynz";
const M1_SOURCE: &str = r#"function main() -> nothing { print("hello, yinz") }"#;

const M2_SMOKE_FILE: &str = "m2_smoke.ynz";
const M2_SMOKE_SOURCE: &str = r#"function main() -> nothing {
  let price = 0.1 + 0.2
  let count: int = 42
  let active = true
  print(price)
  print(count * count - 1)
  print(active && (count > 0))
}"#;

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
/// Load a golden SHA-256 by full filename (e.g. `"hello.x86_64-linux.sha256"`).
fn load_golden(filename: &str) -> Option<[u8; 32]> {
    let path = golden_dir().join(filename);
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

fn save_golden(filename: &str, hash: &[u8; 32]) {
    let path = golden_dir().join(filename);
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


#[test]
fn object_file_sha256_matches_golden() {
    // WHY: this is the reproducibility contract. If the hash changes without
    // an intentional compiler change, codegen became non-deterministic.
    let artifact = run_codegen();
    let slug = triple_slug();

    match load_golden(&format!("hello.{slug}.sha256")) {
        Some(expected) => {
            assert_eq!(
                artifact.sha256, expected,
                "SHA-256 of object file changed. If this is intentional, delete \
                 tests/__golden__/hello.{slug}.sha256 and re-run to regenerate it."
            );
        }
        None => {
            save_golden(&format!("hello.{slug}.sha256"), &artifact.sha256);
            println!("INFO: wrote new golden hash for triple={slug}");
        }
    }
}


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


#[test]
fn ir_text_snapshot() {
    // WHY: IR text diffs help spot codegen regressions during development.
    // This test is informational — a change here means "something about the IR
    // changed." The SHA-256 test above is the gate, not this one.
    let artifact = run_codegen();
    insta::assert_snapshot!("hello_ir", artifact.ir_text);
}


#[test]
fn object_file_is_non_empty() {
    // WHY: an empty object file means LLVM silently failed to emit anything.
    let artifact = run_codegen();
    assert!(
        !artifact.object_bytes.is_empty(),
        "Object file must be non-empty"
    );
}


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


fn run_m2_codegen() -> Option<CompiledArtifact> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, M2_SMOKE_FILE.to_string(), M2_SMOKE_SOURCE.to_string());
    let output = codegen_query(&db, sf);
    if !output.diagnostics.is_empty() {
        eprintln!("M2 codegen diagnostics: {:#?}", output.diagnostics);
        return None;
    }
    Some(output.artifact.clone())
}

#[test]
fn m2_smoke_codegen_produces_non_empty_object() {
    // WHY: if this returns None, print the diagnostics — a type or codegen
    // error is present and the M2 smoke test won't pass end-to-end.
    let artifact = run_m2_codegen().expect("M2 smoke codegen must succeed");
    assert!(
        !artifact.object_bytes.is_empty(),
        "M2 smoke object file must be non-empty"
    );
}

#[test]
fn m2_smoke_codegen_is_deterministic() {
    // WHY: two independent runs must produce byte-identical output.
    // Non-determinism here means the SHA-256 golden is meaningless.
    let db1 = CompilerDb::default();
    let sf1 = SourceFile::new(&db1, M2_SMOKE_FILE.to_string(), M2_SMOKE_SOURCE.to_string());
    let a1 = codegen_query(&db1, sf1).artifact.clone();

    let db2 = CompilerDb::default();
    let sf2 = SourceFile::new(&db2, M2_SMOKE_FILE.to_string(), M2_SMOKE_SOURCE.to_string());
    let a2 = codegen_query(&db2, sf2).artifact.clone();

    assert_eq!(
        a1.sha256, a2.sha256,
        "M2 smoke codegen is not deterministic"
    );
}

#[test]
fn m2_smoke_ir_snapshot() {
    // WHY: IR text diffs reveal codegen regressions during development.
    // Informational — the SHA-256 test is the gate.
    if let Some(artifact) = run_m2_codegen() {
        insta::assert_snapshot!("m2_smoke_ir", artifact.ir_text);
    }
}

#[test]
fn m2_smoke_sha256_golden() {
    // WHY: reproducibility contract for M2. If this changes without
    // an intentional compiler change, codegen or LLVM changed silently.
    let artifact = match run_m2_codegen() {
        Some(a) => a,
        None => {
            eprintln!("SKIP: M2 codegen had errors — fix them before the golden matters");
            return;
        }
    };
    let slug = triple_slug();
    let golden_name = format!("m2_smoke.{slug}.sha256");
    match load_golden(&golden_name) {
        Some(expected) => {
            assert_eq!(
                artifact.sha256, expected,
                "M2 smoke SHA-256 changed. If intentional, delete tests/__golden__/{golden_name} and re-run."
            );
        }
        None => {
            save_golden(&golden_name, &artifact.sha256);
            println!("INFO: wrote new M2 smoke golden for triple={slug}");
        }
    }
}

#[test]
fn m2_decimal_exactness() {
    // WHY: the whole point of decimal128 is that 0.1 + 0.2 == 0.3 exactly.
    // This test compiles just the number computation (not the full smoke test)
    // and verifies the IR is produced without codegen errors. End-to-end
    // execution is Phase 6 (driver integration) which actually runs the binary.
    let source = "function main() -> nothing { let x = 0.1 + 0.2\nprint(x) }";
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "decimal_test.ynz".to_string(), source.to_string());
    let output = codegen_query(&db, sf);
    assert!(
        output.diagnostics.is_empty(),
        "Decimal exactness source must compile without errors: {:#?}",
        output.diagnostics
    );
    assert!(!output.artifact.object_bytes.is_empty(), "Must produce an object file");
}



const M3_FIB_FILE: &str = "m3_fib.ynz";
const M3_FIB_SOURCE: &str = r#"function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}
function main() -> nothing {
  let result = fib(10)
  print(result)
}"#;

fn run_m3_fib_codegen() -> Option<CompiledArtifact> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, M3_FIB_FILE.to_string(), M3_FIB_SOURCE.to_string());
    let output = codegen_query(&db, sf);
    if !output.diagnostics.is_empty() {
        eprintln!("M3 fib codegen diagnostics: {:#?}", output.diagnostics);
        return None;
    }
    Some(output.artifact.clone())
}

#[test]
fn m3_fib_codegen_produces_non_empty_object() {
    // WHY: M3 success criterion — fibonacci must compile to a non-empty object.
    // A None result means typeck or codegen failed; check the diagnostics.
    let artifact = run_m3_fib_codegen().expect("M3 fibonacci codegen must succeed");
    assert!(!artifact.object_bytes.is_empty(), "M3 fib object file must be non-empty");
}

#[test]
fn m3_fib_codegen_is_deterministic() {
    // WHY: two runs of fib codegen must produce identical object bytes.
    // Non-determinism here means the SHA-256 golden is useless.
    let db1 = CompilerDb::default();
    let sf1 = SourceFile::new(&db1, M3_FIB_FILE.to_string(), M3_FIB_SOURCE.to_string());
    let a1 = codegen_query(&db1, sf1).artifact.clone();

    let db2 = CompilerDb::default();
    let sf2 = SourceFile::new(&db2, M3_FIB_FILE.to_string(), M3_FIB_SOURCE.to_string());
    let a2 = codegen_query(&db2, sf2).artifact.clone();

    assert_eq!(a1.sha256, a2.sha256, "M3 fib codegen is not deterministic");
    assert_eq!(a1.object_bytes, a2.object_bytes, "M3 fib object bytes not deterministic");
}

#[test]
fn m3_fib_sha256_golden() {
    // WHY: reproducibility contract for M3 fibonacci. Any unintentional change
    // to codegen or LLVM will flip this hash.
    let artifact = match run_m3_fib_codegen() {
        Some(a) => a,
        None => {
            panic!("M3 fib codegen failed — see diagnostics above");
        }
    };
    let slug = triple_slug();
    let filename = format!("m3_fib.{slug}.sha256");
    match load_golden(&filename) {
        Some(expected) => {
            assert_eq!(
                artifact.sha256, expected,
                "SHA-256 of M3 fib object changed. If intentional, delete \
                 tests/__golden__/{filename} and re-run to regenerate."
            );
        }
        None => {
            save_golden(&filename, &artifact.sha256);
            println!("INFO: wrote new M3 fib golden hash for triple={slug}");
        }
    }
}

// ── M4 codegen tests ─────────────────────────────────────────────────────────

const M4_PLAYER_FILE: &str = "m4_player.ynz";
const M4_PLAYER_SOURCE: &str = r#"shape Player {
  name: string
  health: int
}
function greet(share self: Player) -> nothing {
  print(self.name)
}
function heal(lend self: Player, amount: int) -> nothing {
  self.health = self.health + amount
}
function consume(give p: Player) -> nothing {
  print(p.name)
}
function main() -> nothing {
  let p: Player = { name: "Patrick", health: 100 }
  p.greet()
  p.heal(20)
  print(p.health.toString())
  consume(p)
}"#;

fn run_m4_player_codegen() -> Option<CompiledArtifact> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, M4_PLAYER_FILE.to_string(), M4_PLAYER_SOURCE.to_string());
    let output = codegen_query(&db, sf);
    if !output.diagnostics.is_empty() {
        eprintln!("M4 Player codegen diagnostics: {:#?}", output.diagnostics);
        return None;
    }
    Some(output.artifact.clone())
}

#[test]
fn m4_player_codegen_produces_non_empty_object() {
    // WHY: M4 P4 success criterion. Shape struct, UFCS dispatch, ownership
    // modifiers must all lower to a valid, non-empty object file.
    let artifact = run_m4_player_codegen().expect("M4 Player codegen must succeed");
    assert!(!artifact.object_bytes.is_empty(), "M4 Player object file must be non-empty");
}

#[test]
fn m4_player_ir_has_readonly_on_share_param() {
    // WHY: LLVM `readonly` attribute on `share T` parameters is the LLVM contract
    // that unlocks noalias-based optimizations. If this attribute is missing,
    // the perf invariant from design/ownership.md:51-66 is violated silently.
    let artifact = run_m4_player_codegen().expect("M4 Player codegen must succeed");
    let ir = &artifact.ir_text;
    // greet(share self: Player) and consume(give p: Player) must be in the IR.
    // The share param must have `readonly` and `noalias` (in some form).
    assert!(
        ir.contains("readonly") && ir.contains("noalias"),
        "IR must contain `readonly` and `noalias` attributes for share params.\nIR snippet:\n{}",
        ir.lines().take(60).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn m4_player_ir_snapshot() {
    // WHY: IR text diffs expose M4 codegen regressions (struct layout, attribute
    // placement, UFCS call sites). Informational — the sha256 golden is the gate.
    if let Some(artifact) = run_m4_player_codegen() {
        insta::assert_snapshot!("m4_player_ir", artifact.ir_text);
    }
}

#[test]
fn m3_codegen_query_returns_no_diagnostics_on_valid_m3_source() {
    // WHY: all M3 happy-path fixtures must compile without diagnostics.
    // Any diagnostic here means typeck or codegen rejected valid M3 code.
    let sources = [
        ("m3_for.ynz", "function main() -> nothing { for (i in range(0, 3)) { print(i) } }"),
        ("m3_while.ynz", "function main() -> nothing { let x: int = 3\nwhile (x > 0) { x = x - 1 }\nprint(x) }"),
        ("m3_early_ret.ynz", "function sign(x: int) -> int { if (x > 0) { return 1 } return 0 }\nfunction main() -> nothing { print(sign(5)) }"),
        ("m3_multicase.ynz", "function main() -> nothing { let v: int = 2\nif (v) { 1 => print(1)\n2 => print(2)\nelse => print(0) } }"),
        ("m3_mutual.ynz", "function a(n: int) -> int { if (n <= 0) { return 0 } return b(n - 1) }\nfunction b(n: int) -> int { if (n <= 0) { return 0 } return a(n - 1) }\nfunction main() -> nothing { print(a(4)) }"),
    ];
    for (file, source) in &sources {
        let db = CompilerDb::default();
        let sf = SourceFile::new(&db, file.to_string(), source.to_string());
        let output = codegen_query(&db, sf);
        assert!(
            output.diagnostics.is_empty(),
            "M3 source `{file}` must compile without diagnostics:\n{:#?}",
            output.diagnostics
        );
    }
}
