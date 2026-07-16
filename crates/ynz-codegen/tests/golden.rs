// WHY: object-file bytes are the reproducibility contract — IR text drifts on
// LLVM patch versions, object bytes do not. If this golden SHA-256 changes
// without an intentional compiler change, something in the codegen or LLVM
// backend changed silently.

use std::path::PathBuf;

use ynz_codegen::{codegen_query, sha256, CompiledArtifact};
use ynz_parser::{CompilerDb, SourceFile};

/// Diagnostics that gate a clean compile: everything EXCEPT Suggestion severity.
/// Tier 3 lint suggestions (v0.3-M4 `[[lint_rule]]`, e.g. `prefer-yielding-sleep` on
/// the deliberate sleepBlocking keepalives several fixtures here use) are
/// informational teaching surfaces — they never block compilation and must not fail
/// a "compiles clean" gate. Mirrors the assert_clean ratchet in ynz-typeck/tests.
fn gating_diags(bucket: &ynz_diagnostics::DiagnosticBucket) -> Vec<&ynz_diagnostics::Diagnostic> {
    bucket
        .iter()
        .filter(|d| d.severity != ynz_diagnostics::Severity::Suggestion)
        .collect()
}

const NON_CROSSING_LOCAL_FILE: &str = "v0_3_m3a_p1_non_crossing_local_not_slotted.ynz";

/// Read the non-crossing-local fixture source from disk.
///
/// The fixture file is the single source of truth for this program. Reading it directly
/// avoids maintaining a parallel inline copy that can silently diverge if the fixture
/// changes — one definition, one place to update.
fn non_crossing_local_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ynz-driver/tests/fixtures")
        .join(NON_CROSSING_LOCAL_FILE);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

const FILE: &str = "hello.ynz";
// test-ratchet: M7 P1 migrates double-quoted string to backtick syntax — double-quotes now
// produce an error diagnostic, so any test source must use backticks. The golden SHA-256
// files will change and must be regenerated (they are auto-regenerated on first run).
const M1_SOURCE: &str = "function entrypoint() -> nothing { print(`hello, yinz`) }";

const M2_SMOKE_FILE: &str = "m2_smoke.ynz";
const M2_SMOKE_SOURCE: &str = r#"function entrypoint() -> nothing {
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
        gating_diags(&output.diagnostics).is_empty(),
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
fn non_crossing_local_not_frame_slotted_ir_inspection() {
    // WHY: AC#8 — a local that is read only BEFORE a `wait` must NOT receive a frame slot.
    // The frame-slot system pre-creates allocas only for locals in `crossing_local_names`
    // (declared before a suspension AND read after it). `setup=99` here is read before the
    // wait and never referenced after, so it must not be in that set.
    //
    // The IR signal: frame-slot stores use the GEP name `ls_{slot_index}` (from
    // `store_local_slot`). A function with zero crossing locals emits no `ls_0`, `ls_1`, etc.
    // If `setup` were spuriously slotted it would appear as `ls_0` in the IR. Asserting
    // its ABSENCE proves non-crossing locals stay in SSA registers — no frame overhead.
    //
    // This test would FAIL if `flush_crossing_local_if_needed` were changed to slot every
    // local instead of only crossing locals — making it mutation-resistant.
    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        NON_CROSSING_LOCAL_FILE.to_string(),
        non_crossing_local_source(),
    );
    let output = codegen_query(&db, sf);
    assert!(
        !output.diagnostics.has_errors(),
        "Non-crossing-local source must compile clean; has errors: {:#?}",
        output.diagnostics
    );
    let ir = &output.artifact.ir_text;
    // Frame-slot store GEP names follow the `ls_{idx}` pattern (state_machine.rs store_local_slot).
    // A function with no crossing locals emits none of these names.
    assert!(
        !ir.contains("ls_0"),
        "Non-crossing local `setup` must NOT be frame-slotted; found `ls_0` in IR.\n\
         This means `flush_crossing_local_if_needed` slotted a non-crossing local — \
         fix `crossing_local_names` to exclude locals that are never read after a wait."
    );
    assert!(
        !ir.contains("ls_1"),
        "Non-crossing local `setup` must NOT be frame-slotted; found `ls_1` in IR."
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
    if !gating_diags(&output.diagnostics).is_empty() {
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
    // execution is the driver integration step which actually runs the binary.
    let source = "function entrypoint() -> nothing { let x = 0.1 + 0.2\nprint(x) }";
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "decimal_test.ynz".to_string(), source.to_string());
    let output = codegen_query(&db, sf);
    assert!(
        gating_diags(&output.diagnostics).is_empty(),
        "Decimal exactness source must compile without errors: {:#?}",
        output.diagnostics
    );
    assert!(
        !output.artifact.object_bytes.is_empty(),
        "Must produce an object file"
    );
}

const M3_FIB_FILE: &str = "m3_fib.ynz";
const M3_FIB_SOURCE: &str = r#"function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}
function entrypoint() -> nothing {
  let result = fib(10)
  print(result)
}"#;

fn run_m3_fib_codegen() -> Option<CompiledArtifact> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, M3_FIB_FILE.to_string(), M3_FIB_SOURCE.to_string());
    let output = codegen_query(&db, sf);
    if !gating_diags(&output.diagnostics).is_empty() {
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
    assert!(
        !artifact.object_bytes.is_empty(),
        "M3 fib object file must be non-empty"
    );
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
    assert_eq!(
        a1.object_bytes, a2.object_bytes,
        "M3 fib object bytes not deterministic"
    );
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
// test-ratchet: M7 P1 migrates double-quoted string to backtick syntax
const M4_PLAYER_SOURCE: &str = "shape Player {\n  name: string\n  health: int\n}\nfunction greet(share self: Player) -> nothing {\n  print(self.name)\n}\nfunction heal(lend self: Player, amount: int) -> nothing {\n  self.health = self.health + amount\n}\nfunction consume(give p: Player) -> nothing {\n  print(p.name)\n}\nfunction entrypoint() -> nothing {\n  let p: Player = { name: `Patrick`, health: 100 }\n  p.greet()\n  p.heal(20)\n  print(p.health.toString())\n  consume(p)\n}";

fn run_m4_player_codegen() -> Option<CompiledArtifact> {
    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        M4_PLAYER_FILE.to_string(),
        M4_PLAYER_SOURCE.to_string(),
    );
    let output = codegen_query(&db, sf);
    if !gating_diags(&output.diagnostics).is_empty() {
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
    assert!(
        !artifact.object_bytes.is_empty(),
        "M4 Player object file must be non-empty"
    );
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
        ("m3_for.ynz", "function entrypoint() -> nothing { for (i in range(0, 3)) { print(i) } }"),
        ("m3_while.ynz", "function entrypoint() -> nothing { let x: int = 3\nwhile (x > 0) { x = x - 1 }\nprint(x) }"),
        ("m3_early_ret.ynz", "function sign(x: int) -> int { if (x > 0) { return 1 } return 0 }\nfunction entrypoint() -> nothing { print(sign(5)) }"),
        ("m3_multicase.ynz", "function entrypoint() -> nothing { let v: int = 2\nif (v) { 1 => print(1)\n2 => print(2)\nelse => print(0) } }"),
        ("m3_mutual.ynz", "function a(n: int) -> int { if (n <= 0) { return 0 } return b(n - 1) }\nfunction b(n: int) -> int { if (n <= 0) { return 0 } return a(n - 1) }\nfunction entrypoint() -> nothing { print(a(4)) }"),
    ];
    for (file, source) in &sources {
        let db = CompilerDb::default();
        let sf = SourceFile::new(&db, file.to_string(), source.to_string());
        let output = codegen_query(&db, sf);
        assert!(
            gating_diags(&output.diagnostics).is_empty(),
            "M3 source `{file}` must compile without diagnostics:\n{:#?}",
            output.diagnostics
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M1: background + loop preempt IR snapshots
// ─────────────────────────────────────────────────────────────────────────────

const V03_M1_BACKGROUND_SOURCE: &str = r#"
function worker() -> nothing {
  sleepBlocking(1)
}

function entrypoint() -> nothing {
  background worker()
  print(`done`)
}
"#;

const V03_M1_WHILE_PREEMPT_SOURCE: &str = r#"
function entrypoint() -> nothing {
  let x: int = 3
  while (x > 0) {
    x = x - 1
  }
  print(x)
}
"#;

#[test]
fn v03_m1_background_ir_snapshot() {
    // WHY: locks the exact ynz_rt_spawn_blocking call sequence and closure shape.
    // If the Background lowering drifts (wrong ctx size, wrong closure name, missing
    // shutdown call), this snapshot fails and catches it immediately.
    let db = ynz_parser::CompilerDb::default();
    let sf = ynz_parser::SourceFile::new(
        &db,
        "v03m1bg.ynz".to_string(),
        V03_M1_BACKGROUND_SOURCE.to_string(),
    );
    let output = codegen_query(&db, sf);
    assert!(
        gating_diags(&output.diagnostics).is_empty(),
        "v0.3-M1 background source must compile without diagnostics:\n{:#?}",
        output.diagnostics
    );
    insta::assert_snapshot!("v03_m1_background_ir", output.artifact.ir_text);
}

#[test]
fn v03_m1_while_loop_preempt_ir_snapshot() {
    // WHY: locks that ynz_rt_check_preempt is inserted at the while-loop back-edge.
    // If someone removes emit_loop_preempt from the while-loop lowering, this snapshot
    // fails (preempt call disappears from the IR).
    let db = ynz_parser::CompilerDb::default();
    let sf = ynz_parser::SourceFile::new(
        &db,
        "v03m1while.ynz".to_string(),
        V03_M1_WHILE_PREEMPT_SOURCE.to_string(),
    );
    let output = codegen_query(&db, sf);
    assert!(
        gating_diags(&output.diagnostics).is_empty(),
        "v0.3-M1 while-loop source must compile without diagnostics:\n{:#?}",
        output.diagnostics
    );
    let ir = &output.artifact.ir_text;
    assert!(
        ir.contains("ynz_rt_check_preempt"),
        "while-loop IR must contain ynz_rt_check_preempt call; got:\n{ir}"
    );
    insta::assert_snapshot!("v03_m1_while_preempt_ir", ir);
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M2: state-machine codegen IR snapshots
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: run codegen on a source string, return the IR text.
/// Panics if codegen fails (diagnostics present or empty object file).
fn run_m2_sm_codegen(file: &str, source: &str) -> String {
    let db = ynz_parser::CompilerDb::default();
    let sf = ynz_parser::SourceFile::new(&db, file.to_string(), source.to_string());
    let output = codegen_query(&db, sf);
    assert!(
        gating_diags(&output.diagnostics).is_empty(),
        "v0.3-M2 SM source `{file}` must compile without diagnostics:\n{:#?}",
        output.diagnostics
    );
    assert!(
        !output.artifact.object_bytes.is_empty(),
        "v0.3-M2 SM source `{file}` must produce a non-empty object"
    );
    output.artifact.ir_text.clone()
}

#[test]
fn v03_m2_single_wait_ir_snapshot() {
    // WHY: locks the state-machine IR shape for a function with exactly one wait point.
    // If the resume function, switch dispatch, or suspend/resume blocks change structure,
    // this snapshot fails and the reviewer can audit the diff.
    let source = r#"
function pause() -> nothing {
  wait sleep(100)
}
function entrypoint() -> nothing {
  background pause()
  sleepBlocking(200)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_single_wait.ynz", source);
    assert!(
        ir.contains("ynz_sm_pause_resume"),
        "single-wait IR must contain resume function; got:\n{}",
        ir.lines().take(80).collect::<Vec<_>>().join("\n")
    );
    assert!(
        ir.contains("ynz_rt_async_sleep_create"),
        "single-wait IR must emit sleep create; got:\n{}",
        ir.lines().take(80).collect::<Vec<_>>().join("\n")
    );
    assert!(
        ir.contains("ynz_rt_async_sleep_poll"),
        "single-wait IR must emit sleep poll; got:\n{}",
        ir.lines().take(80).collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("v03_m2_single_wait_ir", ir);
}

#[test]
fn v03_m2_multi_wait_ir_snapshot() {
    // WHY: locks the multi-state state machine IR shape (two sequential waits).
    // A regression in state-block numbering or resume_point tracking will appear here.
    let source = r#"
function chain() -> nothing {
  wait sleep(50)
  wait sleep(50)
}
function entrypoint() -> nothing {
  background chain()
  sleepBlocking(200)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_multi_wait.ynz", source);
    assert!(
        ir.contains("ynz_sm_chain_resume"),
        "multi-wait IR must contain resume function"
    );
    // Two waits → two sleep-create calls.
    let sleep_create_count = ir.matches("ynz_rt_async_sleep_create").count();
    assert!(
        sleep_create_count >= 2,
        "multi-wait IR must have >= 2 sleep_create calls; got {sleep_create_count}"
    );
    insta::assert_snapshot!("v03_m2_multi_wait_ir", ir);
}

#[test]
fn v03_m2_wait_in_if_ir_snapshot() {
    // WHY: locks the IR shape for a wait inside a conditional (wait-in-if branching).
    // State machines with wait inside an `if` must emit correct poll-and-yield in the
    // conditional branch while the non-waiting path goes straight to the terminal.
    let source = r#"
function maybeWait(b: boolean) -> nothing {
  if (b) {
    wait sleep(100)
  }
}
function entrypoint() -> nothing {
  background maybeWait(true)
  sleepBlocking(200)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_wait_in_if.ynz", source);
    assert!(
        ir.contains("ynz_sm_maybeWait_resume"),
        "wait-in-if IR must contain resume function"
    );
    // Behavioral assertions: the `if` branch that contains the wait must reach
    // the suspend/poll path, and sm_pending must have live predecessors (is NOT dead).
    assert!(
        ir.contains("ynz_rt_async_sleep_poll"),
        "wait-in-if IR must call async_sleep_poll inside the if branch; \
         if this fails the wait was silently no-oped"
    );
    // sm_pending must have a predecessor comment showing live control flow to it.
    // A dead sm_pending would show "No predecessors!" — that was the pre-fix bug.
    let sm_pending_line = ir.lines().find(|l| l.contains("sm_pending:")).unwrap_or("");
    assert!(
        !sm_pending_line.contains("No predecessors!"),
        "sm_pending must have live predecessors after wait-in-if fix; \
         'No predecessors!' means the suspend path is dead code"
    );
    insta::assert_snapshot!("v03_m2_wait_in_if_ir", ir);
}

#[test]
fn v03_m2_non_sm_caller_block_on_ir_snapshot() {
    // WHY: locks that a wrapper function calling a state-machine function emits ynz_rt_run_entrypoint.
    // If the program-entry driver symbol disappears from the IR, this test catches it before
    // the program silently returns wrong values.
    let source = r#"
function sleeper() -> nothing {
  wait sleep(100)
}
function entrypoint() -> nothing {
  sleeper()
}
"#;
    let ir = run_m2_sm_codegen("v03m2_non_sm_caller.ynz", source);
    assert!(
        ir.contains("ynz_rt_run_entrypoint"),
        "wrapper IR must emit program-entry driver (ynz_rt_run_entrypoint); got:\n{}",
        ir.lines().take(60).collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("v03_m2_non_sm_caller_block_on_ir", ir);
}

#[test]
fn v03_m2_main_with_wait_ir_snapshot() {
    // WHY: locks that main emits ynz_rt_run_entrypoint when entrypoint contains wait, and that
    // ynz_rt_init is the FIRST non-allocation instruction in main's entry block.
    // If ynz_rt_init placement regresses, programs using wait will panic at runtime
    // with "ynz_rt_init not called before ynz_rt_run_entrypoint call".
    let source = r#"
function entrypoint() -> nothing {
  wait sleep(1)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_main_with_wait.ynz", source);
    assert!(
        ir.contains("ynz_rt_run_entrypoint"),
        "main-with-wait IR must emit program-entry driver (ynz_rt_run_entrypoint)"
    );
    assert!(
        ir.contains("ynz_rt_init"),
        "main-with-wait IR must call ynz_rt_init"
    );
    insta::assert_snapshot!("v03_m2_main_with_wait_ir", ir);
}

#[test]
fn v03_m2_background_spawn_sm_fn_ir_snapshot() {
    // WHY: locks that `background sm_fn()` emits ynz_rt_spawn (I/O pool) and NOT
    // ynz_rt_spawn_blocking (blocking pool). If the routing regresses, background
    // state machines would tie up OS threads during their wait, defeating M2.
    let source = r#"
function worker() -> nothing {
  wait sleep(100)
}
function entrypoint() -> nothing {
  background worker()
  sleepBlocking(200)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_bg_spawn_sm.ynz", source);
    let spawn_calls = ir
        .lines()
        .filter(|l| {
            l.contains("call") && l.contains("ynz_rt_spawn") && !l.contains("ynz_rt_spawn_blocking")
        })
        .count();
    assert!(
        spawn_calls > 0,
        "background SM call must emit ynz_rt_spawn (I/O pool); got:\n{}",
        ir.lines().take(80).collect::<Vec<_>>().join("\n")
    );
    // Must NOT use ynz_rt_spawn_blocking for state-machine callees.
    // Check for actual call instructions (not just declare statements which always appear).
    let spawn_blocking_calls = ir
        .lines()
        .filter(|l| l.contains("call") && l.contains("ynz_rt_spawn_blocking"))
        .count();
    assert_eq!(
        spawn_blocking_calls,
        0,
        "background SM call must NOT emit ynz_rt_spawn_blocking call instructions; got:\n{}",
        ir.lines().take(60).collect::<Vec<_>>().join("\n")
    );
    insta::assert_snapshot!("v03_m2_background_spawn_sm_fn_ir", ir);
}

#[test]
fn v03_m2_background_spawn_regular_fn_ir_snapshot() {
    // WHY: verifies that M1's background behavior is preserved — a wait-free function
    // spawned via `background` still routes to ynz_rt_spawn_blocking (not ynz_rt_spawn).
    // This is the M1 behavior that must not regress when M2 routing is added.
    let source = r#"
function worker() -> nothing {
  sleepBlocking(100)
}
function entrypoint() -> nothing {
  background worker()
  sleepBlocking(200)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_bg_regular.ynz", source);
    let spawn_blocking_calls = ir
        .lines()
        .filter(|l| l.contains("call") && l.contains("ynz_rt_spawn_blocking"))
        .count();
    assert!(
        spawn_blocking_calls > 0,
        "background wait-free call must preserve M1 ynz_rt_spawn_blocking behavior"
    );
    insta::assert_snapshot!("v03_m2_background_spawn_regular_fn_ir", ir);
}

#[test]
fn main_rt_init_is_first_instruction() {
    // WHY: AC #5 — P0 Contract #12 deferred to this test. ynz_rt_init MUST be the
    // first non-alloca instruction in main's entry block whenever any function in the
    // compilation unit contains `wait` or `background`. Without this ordering guarantee,
    // ynz_rt_run_entrypoint panics with "ynz_rt_init not called".
    //
    // This test asserts: the IR of main contains `call void @ynz_rt_init()` and it
    // appears before any `call void @ynz_rt_run_entrypoint` or
    // `call void @ynz_rt_spawn` instruction in main's text.
    let source = r#"
function entrypoint() -> nothing {
  wait sleep(1)
}
"#;
    let ir = run_m2_sm_codegen("v03m2_rt_init_first.ynz", source);

    // Find the main function definition and check instruction order.
    // Scan line by line: once we enter the `main` function, ynz_rt_init must appear
    // before ynz_rt_run_entrypoint.
    let mut in_main = false;
    let mut rt_init_seen = false;
    let mut sm_sync_before_init = false;

    for line in ir.lines() {
        if line.contains("define") && (line.contains("@main") || line.contains("i32 @main")) {
            in_main = true;
        }
        if in_main {
            if line.contains("@ynz_rt_init") {
                rt_init_seen = true;
            }
            if line.contains("@ynz_rt_run_entrypoint") && !rt_init_seen {
                sm_sync_before_init = true;
            }
            // Exit main function definition at closing brace.
            if line == "}" && in_main {
                break;
            }
        }
    }

    assert!(
        rt_init_seen,
        "main must call ynz_rt_init when compilation unit contains wait. IR snippet:\n{}",
        ir.lines().take(60).collect::<Vec<_>>().join("\n")
    );
    assert!(
        !sm_sync_before_init,
        "ynz_rt_run_entrypoint must not appear before ynz_rt_init in main"
    );
    insta::assert_snapshot!("v03_m2_main_rt_init_first", ir);
}

const TWO_CPU_GROUPS_SOURCE: &str = r#"
function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function entrypoint() -> nothing {
  let a = fib(10)
  let b = fib(11)
  print(a)
  print(b)
  if (a > 0) {
    let c = fib(12)
    let d = fib(13)
    print(c)
    print(d)
  }
}
"#;

#[test]
fn two_cpu_groups_decline_emits_no_spawn() {
    // WHY: a function with TWO CPU groups (a top-level pair + a nested-arm pair) is declined by the
    // single-group admission constraint, so codegen must emit ZERO `ynz_rt_spawn_blocking_joinable`
    // calls — the whole function lowers sequentially. This is the binary half of the hint↔binary
    // agreement invariant: the typeck `parallel_group_hints` pass emits no separate-core hint for
    // this shape (see ynz-typeck/tests/parallel_group_hint_parity.rs), and the IR proves the
    // binary spawns nothing. If a future change spawns one of the two groups, this catches the
    // hint/binary divergence (the hint would also have to start firing — both ends move together).
    let ir = run_m2_sm_codegen("two_cpu_groups.ynz", TWO_CPU_GROUPS_SOURCE);
    let joinable_spawns = ir
        .lines()
        .filter(|l| l.contains("call") && l.contains("ynz_rt_spawn_blocking_joinable"))
        .count();
    assert_eq!(
        joinable_spawns, 0,
        "two-CPU-group function must spawn nothing (sequential lowering); got {joinable_spawns} \
         joinable-spawn call instructions:\n{ir}"
    );
}

/// Number of `ynz_rt_spawn_blocking_joinable` call instructions in `ir` — one per spawned CPU
/// group member.
fn joinable_spawn_count(ir: &str) -> usize {
    ir.lines()
        .filter(|l| l.contains("call") && l.contains("ynz_rt_spawn_blocking_joinable"))
        .count()
}

/// The admitted CPU group for the named function in `source`, computed exactly the way the
/// inlay-hint pass and codegen compute it (effective suspend set + CPU-ABI supported callees).
/// Returns `None` when the authority declines to spike-host the function.
fn admitted_group_for(
    source: &str,
    fn_name: &str,
) -> Option<ynz_typeck::cpu_admission::AdmittedCpuGroup> {
    use ynz_typeck::cpu_admission::admitted_cpu_group;
    use ynz_typeck::queries::{check_query, module_signatures_query};
    use ynz_typeck::signatures::build_effective_suspend_set;

    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        format!("/tmp/ynz_xpin_{fn_name}.ynz"),
        source.to_string(),
    );

    let sig_output = module_signatures_query(&db, sf);
    let check_out = check_query(&db, sf);
    let effective_suspends =
        build_effective_suspend_set(&check_out.suspends_set, &sig_output.imported_fns);
    let supported: std::collections::HashSet<String> = sig_output
        .sig_table
        .fns
        .iter()
        .filter(|(_, sig)| ynz_typeck::independence::cpu_result_abi_supports(&sig.ret))
        .map(|(name, _)| name.clone())
        .collect();

    let parse = ynz_parser::parse_query(&db, sf);
    let f = parse.module.items.iter().find_map(|it| match it {
        ynz_ast::nodes::Item::Function(f) if f.name == fn_name => Some(f),
        _ => None,
    })?;
    admitted_cpu_group(
        f,
        &effective_suspends,
        &supported,
        &check_out.typed_module.expr_types,
    )
}

const XPIN_FIRE_SOURCE: &str = r#"
function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function entrypoint() -> nothing {
  let a = fib(20)
  let b = fib(21)
  print(a)
  print(b)
}
"#;

#[test]
fn cross_pin_fire_spawn_count_equals_admitted_member_count() {
    // WHY: pins the BINARY's emitted spawn count directly to the single admission AUTHORITY
    //      (`admitted_cpu_group`), not to a hardcoded literal. The hint side is already pinned to
    //      the authority (ynz-typeck/tests/parallel_group_hint_parity.rs); without this the codegen
    //      side only asserted hand-written `expected_spawns` numbers, so a unification bug that
    //      silently changed which members spawn could go green. The invariant: for an admitted
    //      function, codegen emits exactly one `ynz_rt_spawn_blocking_joinable` per admitted group
    //      member. If you're tempted to relax `== members.len()` to a tolerance or a literal, the
    //      bug is a spike-admission/codegen divergence — fix that, not this assertion.
    let ir = run_m2_sm_codegen("xpin_fire.ynz", XPIN_FIRE_SOURCE);
    let spawns = joinable_spawn_count(&ir);

    let group =
        admitted_group_for(XPIN_FIRE_SOURCE, "entrypoint").expect("entrypoint must be admitted");
    assert_eq!(
        spawns,
        group.member_indices.len(),
        "emitted joinable-spawn count ({spawns}) must equal the authority's admitted member count \
         ({}); a mismatch is a spike-admission ⇄ codegen divergence (the exact hint↔binary \
         silent-drift class). IR:\n{ir}",
        group.member_indices.len()
    );
    // Sanity: a clean 2-member group fires 2 spawns. If this changes, the fixture changed, not the
    // invariant — the assertion above is the real guard.
    assert_eq!(spawns, 2, "clean 2-member group must spawn exactly 2");
}

#[test]
fn cross_pin_decline_means_zero_spawns() {
    // WHY: the DECLINE half of the cross-pin invariant — `admitted_cpu_group(...) == None` MUST
    //      coincide with ZERO emitted spawns. Together with the FIRE test this makes the binary's
    //      spawn behavior a direct function of the admission authority in BOTH directions, closing
    //      the gap the prior hardcoded-`expected_spawns` golden tests left open. A two-CPU-group
    //      function is the canonical decline (single-group constraint). If this fails, an admission
    //      change started spawning for a declined function — fix the divergence, not this test.
    let ir = run_m2_sm_codegen("xpin_decline.ynz", TWO_CPU_GROUPS_SOURCE);
    let spawns = joinable_spawn_count(&ir);

    let group = admitted_group_for(TWO_CPU_GROUPS_SOURCE, "entrypoint");
    assert!(
        group.is_none(),
        "two-CPU-group function must be DECLINED by the authority; got {group:?}"
    );
    assert_eq!(
        spawns, 0,
        "a declined function (admitted_cpu_group == None) must emit ZERO spawns; got {spawns}. \
         IR:\n{ir}"
    );
}
