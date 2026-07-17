// WHY: v0.3-M7 R10 — builds were nondeterministic: per-process-seeded HashMap iteration
// reached LLVM emission order (monomorphization table entries → function definition order
// in the object; imported-fn declaration loop → declaration order in the IR text; vtable
// globals → global emission order, FRAGO 013), so repeated `ynz build` of the same project
// flapped the output binary between orderings run-to-run (the pirates-roster two-hash
// flap, audit.md F3; the 8/8 vtable-order flap, FRAGO 013).
//
// These tests lock the reproducible-build Safety invariant: N INDEPENDENT builds (separate
// process spawns — the nondeterminism was per-process hash seeding, so in-process
// repetition inside one test binary would prove nothing) of the same project must produce
// a byte-identical binary AND byte-identical `--emit-ir` IR text.
//
// Two legs:
// - `examples/pirates-roster` (default pipeline) — the canonical multi-file demo project
//   and the exact reproducer the R10 flap was proven on. Single-file builds were already
//   deterministic for THAT ordering class (audit.md F3 residual); multi-file is the
//   surface it guards.
// - `fixtures/v0_3_m7_r10_multi_vtable.ynz` under `--no-optimize` — a ≥2-vtable module
//   where the vtable-global emission order (vtable.rs, FRAGO 013) is NOT masked by DCE.
//   Optimized builds strip the unreferenced-by-optimized-code vtable globals, which is
//   exactly how the vtable instance of this class evaded the first leg.

use std::path::Path;
use std::process::Command;

/// Number of independent build processes per leg. The operative guard is the JOINT
/// binary∧IR byte-equality bound: a false pass requires every one of the RUNS-1
/// comparison runs to match run 1 on BOTH axes at once. The honest single-axis figure,
/// derived from the observed pre-fix flap distribution (two orderings at roughly 60/40,
/// audit.md F3): P(all 6 runs land one ordering) = 0.6^6 + 0.4^6 ≈ 5.1%. The joint
/// binary∧IR check is strictly tighter (the IR text exposes more ordering sources than
/// the linked binary), so 5.1% is the conservative per-leg ceiling, not the operative
/// probability.
const RUNS: usize = 6;

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)?.flatten() {
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Run `ynz <args>` RUNS independent times in `dir`, asserting the produced binary and
/// IR text are byte-identical across every run. `context` names the leg in failure text.
///
/// Time: O(RUNS) build invocations; Space: O(binary + IR text) held per run for comparison.
fn assert_independent_builds_identical(
    dir: &Path,
    args: &[&str],
    bin_path: &Path,
    ir_path: &Path,
    context: &str,
) {
    // Drop any stale artifacts — each run below must produce its own from scratch.
    let _ = std::fs::remove_file(bin_path);
    let _ = std::fs::remove_file(ir_path);

    let mut first: Option<(Vec<u8>, Vec<u8>)> = None;
    for run in 1..=RUNS {
        let out = Command::new(env!("CARGO_BIN_EXE_ynz"))
            .args(args)
            .current_dir(dir)
            .env("CLICOLOR", "0")
            .output()
            .unwrap_or_else(|e| panic!("{context}: run {run}: failed to spawn ynz: {e}"));
        assert!(
            out.status.success(),
            "{context}: run {run}: build failed (exit {:?}); stderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );

        let binary = std::fs::read(bin_path)
            .unwrap_or_else(|e| panic!("{context}: run {run}: could not read built binary: {e}"));
        let ir = std::fs::read(ir_path)
            .unwrap_or_else(|e| panic!("{context}: run {run}: could not read emitted IR: {e}"));

        match &first {
            None => first = Some((binary, ir)),
            Some((first_binary, first_ir)) => {
                assert!(
                    first_binary == &binary,
                    "{context}: run {run}: binary bytes differ from run 1 ({} vs {}) — \
                     build nondeterminism regressed (v0.3-M7 R10; an unordered \
                     collection's iteration order is reaching emission order again)",
                    fingerprint_hex(first_binary),
                    fingerprint_hex(&binary),
                );
                assert!(
                    first_ir == &ir,
                    "{context}: run {run}: --emit-ir text differs from run 1 ({} vs {}) — \
                     build nondeterminism regressed (v0.3-M7 R10; an unordered \
                     collection's iteration order is reaching emission order again)",
                    fingerprint_hex(first_ir),
                    fingerprint_hex(&ir),
                );
            }
        }

        // Remove artifacts so the next run rebuilds from scratch rather than any
        // incremental path short-circuiting the comparison.
        std::fs::remove_file(bin_path)
            .unwrap_or_else(|e| panic!("{context}: remove binary between runs: {e}"));
        std::fs::remove_file(ir_path)
            .unwrap_or_else(|e| panic!("{context}: remove IR between runs: {e}"));
    }
}

#[test]
fn multi_file_build_is_deterministic_across_independent_processes() {
    let project_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/pirates-roster");
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let project = tmp.path().join("pirates-roster");
    std::fs::create_dir_all(&project).expect("create project dir");
    copy_dir(&project_src, &project).expect("copy pirates-roster into tempdir");

    assert_independent_builds_identical(
        &project,
        &["build", "entrypoint.ynz", "--emit-ir"],
        &project.join("bin"),
        &project.join("bin.ll"),
        "pirates-roster (default pipeline)",
    );
}

// WHY: locks the vtable-global emission order (vtable.rs sorted iteration, FRAGO 013)
// under `--no-optimize`, where DCE cannot strip the vtable globals and mask an ordering
// flap. Three (shape, contract) pairs → six possible emission orderings pre-fix.
#[test]
fn multi_vtable_no_optimize_build_is_deterministic_across_independent_processes() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v0_3_m7_r10_multi_vtable.ynz");
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    std::fs::copy(&fixture, tmp.path().join("vtables.ynz")).expect("copy fixture into tempdir");

    assert_independent_builds_identical(
        tmp.path(),
        &["build", "vtables.ynz", "--emit-ir", "--no-optimize"],
        &tmp.path().join("vtables"),
        &tmp.path().join("vtables.ll"),
        "multi-vtable fixture (--no-optimize)",
    );
}

/// Compact identity for a byte blob in failure messages. Full byte equality is asserted
/// above; this only labels a mismatch without dumping megabytes into the panic text.
fn fingerprint_hex(bytes: &[u8]) -> String {
    // FNV-1a 128-bit fold — deterministic fingerprint, not cryptographic.
    let mut hi: u64 = 0xcbf2_9ce4_8422_2325;
    let mut lo: u64 = 0x6c62_272e_07bb_0142;
    for &b in bytes {
        lo ^= u64::from(b);
        lo = lo.wrapping_mul(0x0000_0100_0000_01b3);
        hi ^= lo.rotate_left(29);
        hi = hi.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hi:016x}{lo:016x}-len{}", bytes.len())
}
