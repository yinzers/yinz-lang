// v0.3-M7 Phase 7 — O0-vs-optimized A/B benchmark harness for the LLVM pass
// pipeline (Key Outcome 5's evidence base).
//
// Extends soa_calibration.rs's harness pattern (criterion, compiled-.ynz-binary
// driving, pre-bench gates) rather than inventing a new one. Drives each
// workload binary at the two pipeline tiers via the harness-only YNZ_OPT_FORCE
// override (v0.3-M7 Phase 3): "o0" is byte-for-byte the `--no-optimize` tier
// (PipelineConfig::o0()); "default" is what a plain `ynz build` ships
// (default<O2> mid-end + Aggressive backend).
//
// Every workload binary passes three gates BEFORE it is benched:
//   1. checksum — one run must print exactly the closed-form checksum
//      (correctness tripwire: an optimized binary computing a different answer
//      is a miscompile, not a speedup);
//   2. byte-identical stdout across the two tiers (the dual-mode oracle —
//      optimization must never change behavior);
//   3. IR gate — the default-tier .ll must DIFFER from the o0-tier .ll. Valid
//      because `--emit-ir` prints the module AFTER run_passes (emit.rs: "the IR
//      the object was actually lowered from, never a pre-pipeline draft") and
//      builds are proven deterministic (Phase 5), so byte-identical .ll across
//      tiers could only mean the mid-end pipeline silently did not run — the
//      M3d silent-decline tripwire class (this harness would then measure
//      O0 vs O0 and report a fake ~1.0x with green checksums).
//
// Visit-budget re-assessment (Phase 7 Step 1 obligation — do NOT blindly copy
// soa_calibration's 131,072 cap): that cap was re-evaluated at Phase 4 as a
// bench-runtime budget ONLY — the old O0 per-iteration alloca stack-growth
// crash envelope (ledger row 439) is eliminated by loop_stack_save/
// loop_stack_restore and permanently locked at 67,108,864 visits per tier by
// crates/ynz-driver/tests/hot_loop_stack_stress.rs. This harness therefore
// raises the per-workload budget to 8.4M-16.8M visits (64x-128x the old cap):
// an A/B wall-clock ratio is only honest when per-run time (~150-560ms
// measured) dominates process-spawn overhead (~5-15ms, measured explicitly by
// the `overhead` group below); at 131,072 visits every run would be
// spawn-dominated and the reported ratio would be an artifact of exec cost.
// The ceiling stays a bench-runtime decision: ~7 bench points x ~11s keeps the
// full suite under ~2 minutes.
//
// Results land in target/criterion/**/new/estimates.json; recorded medians +
// provenance: crates/ynz-driver/benches/opt-pipeline-raw-2026-07-17.md (A/B)
// and rust-equiv-raw-2026-07-17.md (Rust comparison).
//
// Rust-equivalent comparison (Phase 7 Step 3): the `rust_equiv` group benches
// the hand-authored idiomatic Rust programs in benches/rust-equivalents/ (a
// deliberately NON-workspace-member package — see its Cargo.toml header for the
// placement + overflow-semantics decisions), built by this harness at run time
// via `cargo build --manifest-path` at two profiles: `release` (idiomatic
// defaults, overflow checks off — the primary comparison) and
// `release-checked` (overflow-checks=true — the Yinz-semantics-matched
// secondary). Each Rust binary must print the same closed-form checksum as its
// Yinz twin (the shared oracle); the dual-mode and IR gates are Yinz-tier
// gates and do not apply cross-language. The comparable Yinz side is the same
// bench run's opt_pipeline default-tier points (same session, same host, both
// sides net of the measured spawn overhead).
//
// Time: O(workloads · modes · sample_size) process spawns.
// Space: O(SOA_N) generated source.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode};

#[path = "bench_common.rs"]
mod bench_common;
use bench_common::{compile_workload, run_once_checked, scratch_dir};

/// The knob-not-to-reach-for tail of run_once_checked's crash panic: this
/// harness's budget axis is per-workload size (the raised visit budget), not a
/// shared visit cap.
const CRASH_HINT: &str = "shrink the workload";

/// Pipeline tiers under comparison (YNZ_OPT_FORCE accepted values,
/// state_machine.rs pipeline_config_from_env): "o0" == the `--no-optimize`
/// escape hatch; "default" == the shipped `ynz build` default (default<O2>).
const MODES: &[&str] = &["o0", "default"];

/// CPU-bound scalar loop: 2^24 iterations of add + rem (overflow-checked int
/// arithmetic, no allocation, no array traffic).
const CPU_LOOP_REPS: i64 = 16_777_216;

/// Shape-heavy allocation loop: 2^23 iterations, each constructing a 3-field
/// shape literal in the loop body (per-iteration allocas — the row-439
/// stacksave/stackrestore path) and reading all three fields back.
const SHAPE_ALLOC_REPS: i64 = 8_388_608;

/// The M5-characterized SoA physics-update workload: 64-byte 8-int-field
/// Player, 2-of-8 hot fields (x, y), read-accumulate scan — identical shape to
/// soa_calibration.rs's settled workload, at the default layout admission
/// (no YNZ_SOA_FORCE: this harness A/Bs the pipeline tier, nothing else).
const SOA_N: i64 = 64;
const SOA_REPS: i64 = 262_144; // 16.8M element visits

/// sum_{i=0}^{reps-1} ((i % 7) + 1): full cycles of 1..=7 sum to 28.
fn cpu_loop_checksum(reps: i64) -> i64 {
    let rem = reps % 7;
    (reps / 7) * 28 + rem * (rem + 1) / 2
}

fn cpu_loop_source(reps: i64) -> String {
    format!(
        "function entrypoint() -> nothing {{\n  let reps: int = {reps}\n  let i: int = 0\n  let acc: int = 0\n  while (i < reps) {{\n    acc = acc + (i % 7) + 1\n    i = i + 1\n  }}\n  print(acc.toString())\n}}\n"
    )
}

/// sum_{i=1}^{reps} (i + 2i + 3) = 3·reps(reps+1)/2 + 3·reps.
fn shape_alloc_checksum(reps: i64) -> i64 {
    3 * reps * (reps + 1) / 2 + 3 * reps
}

fn shape_alloc_source(reps: i64) -> String {
    format!(
        "shape Point {{\n  x: int\n  y: int\n  z: int\n}}\n\nfunction entrypoint() -> nothing {{\n  let reps: int = {reps}\n  let i: int = 1\n  let acc: int = 0\n  while (i <= reps) {{\n    let p: Point = {{ x: i, y: i + i, z: 3 }}\n    acc = acc + p.x + p.y + p.z\n    i = i + 1\n  }}\n  print(acc.toString())\n}}\n"
    )
}

/// x_i = i+1 pattern from soa_calibration: checksum = reps·3·n(n+1)/2.
fn soa_physics_checksum(n: i64, reps: i64) -> i64 {
    reps * 3 * n * (n + 1) / 2
}

fn soa_physics_source(n: i64, reps: i64) -> String {
    let mut src = String::with_capacity(96 * n as usize + 512);
    src.push_str(
        "shape Player {\n  x: int\n  y: int\n  dx: int\n  dy: int\n  health: int\n  stamina: int\n  gold: int\n  level: int\n}\n\nfunction entrypoint() -> nothing {\n  let players: array<Player> = [\n",
    );
    for i in 1..=n {
        let sep = if i == n { "" } else { "," };
        writeln!(
            src,
            "    {{ x: {i}, y: {}, dx: 1, dy: 1, health: 100, stamina: 50, gold: 7, level: 3 }}{sep}",
            2 * i
        )
        .expect("string write");
    }
    write!(
        src,
        "  ]\n  let reps: int = {reps}\n  let r: int = 0\n  let acc: int = 0\n  while (r < reps) {{\n    for (p in players) {{\n      acc = acc + p.x + p.y\n    }}\n    r = r + 1\n  }}\n  print(acc.toString())\n}}\n"
    )
    .expect("string write");
    src
}

/// Compile one workload source under one forced pipeline tier (harness-only
/// YNZ_OPT_FORCE override); returns the binary path. Spawn-and-assert plumbing
/// is shared — see bench_common::compile_workload.
fn compile(dir: &Path, workload: &str, source: &str, mode: &str) -> PathBuf {
    let stem = format!("{workload}_{mode}");
    compile_workload(dir, &stem, source, "YNZ_OPT_FORCE", mode)
}

/// IR gate: the default-tier .ll must differ from the o0-tier .ll (see header —
/// post-pipeline IR + deterministic builds make byte-equality prove exactly
/// "the mid-end pipeline did not run").
fn assert_ir_gate(workload: &str, o0_bin: &Path, default_bin: &Path) {
    let read = |bin: &Path| -> String {
        let ll = bin.with_extension("ll");
        let ir = fs::read_to_string(&ll)
            .unwrap_or_else(|e| panic!("--emit-ir output missing at {}: {e}", ll.display()));
        assert!(!ir.trim().is_empty(), "empty .ll at {}", ll.display());
        ir
    };
    let (o0_ir, default_ir) = (read(o0_bin), read(default_bin));
    assert_ne!(
        o0_ir, default_ir,
        "IR gate: {workload}'s default-tier .ll is byte-identical to its o0 .ll — \
         the mid-end pipeline silently did not run (M3d silent-decline class); \
         this harness would measure O0 vs O0 and report a fake ~1.0x"
    );
}

fn opt_pipeline_bench(c: &mut Criterion) {
    let dir = scratch_dir("p7-opt-calibration");

    let workloads: Vec<(&str, String, i64)> = vec![
        (
            "cpu_loop",
            cpu_loop_source(CPU_LOOP_REPS),
            cpu_loop_checksum(CPU_LOOP_REPS),
        ),
        (
            "shape_alloc",
            shape_alloc_source(SHAPE_ALLOC_REPS),
            shape_alloc_checksum(SHAPE_ALLOC_REPS),
        ),
        (
            "soa_physics",
            soa_physics_source(SOA_N, SOA_REPS),
            soa_physics_checksum(SOA_N, SOA_REPS),
        ),
    ];

    // Process-overhead baseline: reps = 0 (touches nothing, prints 0). Recorded
    // in the provenance file so per-workload totals can be read net of spawn
    // cost — the explicit justification for the raised visit budget (header).
    {
        let bin = compile(&dir, "cpu_loop", &cpu_loop_source(0), "o0");
        run_once_checked(&bin, 0, CRASH_HINT);
        let mut overhead = c.benchmark_group("overhead");
        overhead.sample_size(12);
        overhead.sampling_mode(SamplingMode::Flat);
        overhead.warm_up_time(Duration::from_secs(1));
        overhead.measurement_time(Duration::from_secs(4));
        overhead.bench_function("spawn_only", |b| {
            b.iter(|| {
                let out = Command::new(&bin).output().expect("spawn");
                assert!(out.status.success());
            });
        });
        overhead.finish();
    }

    let mut group = c.benchmark_group("opt_pipeline");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));

    for (workload, source, expected) in &workloads {
        // Compile + gate both tiers BEFORE benching either, so the IR gate and
        // the byte-identical cross-tier comparison happen up front.
        let mut outputs = Vec::new();
        let mut bins = Vec::new();
        for &mode in MODES {
            let bin = compile(&dir, workload, source, mode);
            outputs.push(run_once_checked(&bin, *expected, CRASH_HINT));
            bins.push((mode, bin));
        }
        assert_eq!(
            outputs[0], outputs[1],
            "dual-mode stdout diverged for {workload} — the pipeline tier must \
             never change behavior"
        );
        assert_ir_gate(workload, &bins[0].1, &bins[1].1);

        for (mode, bin) in bins {
            group.bench_with_input(BenchmarkId::new(*workload, mode), &bin, |b, bin| {
                b.iter(|| {
                    let out = Command::new(bin).output().expect("spawn workload");
                    assert!(out.status.success());
                });
            });
        }
    }
    group.finish();
}

/// The two cargo profiles the Rust equivalents are built at (see the
/// rust-equivalents Cargo.toml header for the overflow-semantics decision).
const RUST_PROFILES: &[&str] = &["release", "release-checked"];

/// Build the rust-equivalents package (NOT a workspace member) at one profile;
/// returns the profile's output dir. Uses the same `cargo` that is running this
/// bench (the CARGO env var cargo sets on its children) so toolchain choice is
/// inherited, and an isolated target dir so the workspace build is untouched.
fn build_rust_equivalents(target_dir: &Path, profile: &str) -> PathBuf {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/rust-equivalents/Cargo.toml");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--profile")
        .arg(profile)
        .arg("--target-dir")
        .arg(target_dir)
        .output()
        .expect("spawn cargo build for rust-equivalents");
    assert!(
        out.status.success(),
        "cargo build --profile {profile} failed for rust-equivalents: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    target_dir.join(profile)
}

fn rust_equiv_bench(c: &mut Criterion) {
    let target_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/p7-rust-equiv");

    // (workload, expected checksum) — the SAME closed-form oracles the Yinz
    // side is gated on, so both languages are checked against one answer.
    let workloads: &[(&str, i64)] = &[
        ("cpu_loop", cpu_loop_checksum(CPU_LOOP_REPS)),
        ("shape_alloc", shape_alloc_checksum(SHAPE_ALLOC_REPS)),
        ("soa_physics", soa_physics_checksum(SOA_N, SOA_REPS)),
    ];

    let mut group = c.benchmark_group("rust_equiv");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));

    for &profile in RUST_PROFILES {
        let bin_dir = build_rust_equivalents(&target_dir, profile);

        // Per-language spawn baseline (release profile only — the probe does no
        // arithmetic, so the overflow-checks profile split is irrelevant to
        // it): the Yinz `overhead` group's reps=0 binary includes Yinz runtime
        // init, so each language's workloads must be read net of its OWN
        // baseline; the difference between the two baselines is the measured
        // Yinz runtime-init cost.
        if profile == "release" {
            let probe = bin_dir.join("spawn_probe");
            run_once_checked(&probe, 0, CRASH_HINT);
            group.bench_function("spawn_only", |b| {
                b.iter(|| {
                    let out = Command::new(&probe).output().expect("spawn probe");
                    assert!(out.status.success());
                });
            });
        }

        for &(workload, expected) in workloads {
            let bin = bin_dir.join(workload);
            run_once_checked(&bin, expected, CRASH_HINT);
            group.bench_with_input(BenchmarkId::new(workload, profile), &bin, |b, bin| {
                b.iter(|| {
                    let out = Command::new(bin).output().expect("spawn rust workload");
                    assert!(out.status.success());
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, opt_pipeline_bench, rust_equiv_bench);
criterion_main!(benches);
