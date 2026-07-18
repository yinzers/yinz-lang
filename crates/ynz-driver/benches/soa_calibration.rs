// v0.3-M5 Phase 6 — SOA_SIZE_THRESHOLD calibration harness.
//
// Dev-only (criterion is a dev-dependency; nothing here links into libynz_rt.a or
// any shipped binary). Drives COMPILED .ynz workload binaries — the roadmap's
// physics-update loop over array<Player> with x/y access, realized as the
// S2-qualifying read-accumulate scan (field write-back through the for-in loop var
// does not exist in the language; whole-element IndexAssign touches all 8 segments,
// destroying the 2-hot-field pattern this measures) — at N in {8..4096} powers of
// two, once per layout mode via the harness-only YNZ_SOA_FORCE override (D8).
//
// Every workload binary passes three gates BEFORE it is benched:
//   1. checksum — one run must print exactly R·3·N(N+1)/2 (correctness tripwire;
//      historically also the E13 stack-growth-SIGSEGV canary — fixed, see below);
//   2. byte-identical stdout across the two layout modes (the dual-mode oracle);
//   3. IR gate — the soa-mode .ll must contain the SoA lowering symbols
//      (soa_ctor/soa_new) and the aos-mode .ll must contain ZERO (the M3d
//      silent-decline tripwire; without it this would measure AoS vs AoS).
//
// TOTAL_VISITS is held constant across N so per-point totals compare per-visit
// cost directly; R = TOTAL_VISITS / N.
//
// Cap re-evaluated 2026-07-17 (v0.3-M7 Phase 4): the underlying hot-loop O0
// stack-growth SIGSEGV (roadmap ledger row 439 / risk E13's crash envelope —
// historically N=8/R=65536 = 524,288 visits SIGSEGV'd at -O0) is FIXED: plain-loop
// back-edges now release each iteration's allocas via llvm.stacksave/stackrestore
// (ynz-codegen/src/emit.rs `loop_stack_save`/`loop_stack_restore`). Fresh evidence:
// N=8/R=8,388,608 (67.1M visits — 16x the old ~4.19M envelope, 128x the old N=8
// crash point) runs green with the exact checksum at BOTH tiers, and 33.5M visits
// completes under a 1 MB stack ulimit at -O0 (flat frame). Permanent lock:
// crates/ynz-driver/tests/hot_loop_stack_stress.rs. 131,072 therefore remains ONLY
// as a bench-runtime budget (it keeps each criterion process-spawn iteration fast
// across all 20 points) — no longer a safety cap on either axis; raising it is a
// bench-cost decision, not a crash-envelope one. (The earlier "SIGABRT at 262144
// visits at N=8" reading was the stale-runtime-archive bug, FRAGO 018 — not this
// stack-growth class.)
//
// Each criterion iteration spawns the compiled binary; spawn overhead is identical
// across modes so it cancels in the crossover comparison, and the `overhead`
// group point (reps = 0) measures it explicitly for the provenance record.
//
// Results land in target/criterion/**/new/estimates.json (medians read by the
// calibration step); provenance: crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md,
// "Step 3 calibration verdict" section.
//
// Time: O(points · sample_size) process spawns.  Space: O(N) generated source.

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
/// harness's budget axis is the visit cap (TOTAL_VISITS), not workload shape.
const CRASH_HINT: &str = "lower the cap";

/// Total element visits per process run — a bench-runtime budget since the v0.3-M7
/// Phase 4 stack-growth fix (re-evaluation record + evidence in the header).
const TOTAL_VISITS: i64 = 131_072;

/// Calibration points: powers of two bracketing SOA_SIZE_THRESHOLD = 64.
const SIZES: &[i64] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

/// YNZ_SOA_FORCE values (D8): "aos" empties the candidate set; "soa" skips only
/// the BelowSizeThreshold admission arm.
const MODES: &[&str] = &["aos", "soa"];

/// The settled workload: 64-byte 8-int-field Player, 2-of-8 hot fields (x, y) —
/// a 4x theoretical bandwidth edge for SoA at a cache-line granularity.
/// x_i = i+1, y_i = 2(i+1); checksum = R·3·N(N+1)/2.
fn workload_source(n: i64, reps: i64) -> String {
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

fn expected_checksum(n: i64, reps: i64) -> i64 {
    reps * 3 * n * (n + 1) / 2
}

/// Compile one workload under one forced layout (harness-only YNZ_SOA_FORCE
/// override); returns the binary path. Spawn-and-assert plumbing is shared —
/// see bench_common::compile_workload.
fn compile(dir: &Path, n: i64, reps: i64, mode: &str) -> PathBuf {
    let stem = format!("n{n}_{mode}");
    compile_workload(dir, &stem, &workload_source(n, reps), "YNZ_SOA_FORCE", mode)
}

/// IR gate: soa-mode IR must carry the SoA lowering symbols; aos-mode must not.
fn assert_ir_gate(bin: &Path, mode: &str) {
    let ll_path = bin.with_extension("ll");
    let ir = fs::read_to_string(&ll_path)
        .unwrap_or_else(|e| panic!("--emit-ir output missing at {}: {e}", ll_path.display()));
    let has_soa = ir.contains("soa_ctor") || ir.contains("soa_new");
    match mode {
        "soa" => assert!(
            has_soa,
            "IR gate: forced-soa build of {} contains NO SoA lowering symbols — \
             the admission silently declined and this point would measure AoS vs AoS",
            bin.display()
        ),
        "aos" => assert!(
            !has_soa,
            "IR gate: forced-aos build of {} contains SoA lowering symbols — \
             the force override failed",
            bin.display()
        ),
        other => panic!("unknown mode {other}"),
    }
}

fn soa_threshold_bench(c: &mut Criterion) {
    let dir = scratch_dir("p6-soa-calibration");

    // Process-overhead baseline: reps = 0 (touches nothing, prints 0). Recorded
    // in the provenance file so per-point totals can be read net of spawn cost.
    {
        let bin = compile(&dir, 8, 0, "aos");
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

    let mut group = c.benchmark_group("soa_threshold");
    group.sample_size(12);
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(4));

    for &n in SIZES {
        let reps = TOTAL_VISITS / n;
        let expected = expected_checksum(n, reps);

        // Compile + gate both modes BEFORE benching either, so the
        // byte-identical cross-mode comparison happens up front.
        let mut outputs = Vec::new();
        let mut bins = Vec::new();
        for &mode in MODES {
            let bin = compile(&dir, n, reps, mode);
            assert_ir_gate(&bin, mode);
            outputs.push(run_once_checked(&bin, expected, CRASH_HINT));
            bins.push((mode, bin));
        }
        assert_eq!(
            outputs[0], outputs[1],
            "dual-mode stdout diverged at N={n} — layout must never change behavior"
        );

        for (mode, bin) in bins {
            group.bench_with_input(BenchmarkId::new(mode, n), &bin, |b, bin| {
                b.iter(|| {
                    let out = Command::new(bin).output().expect("spawn workload");
                    assert!(out.status.success());
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, soa_threshold_bench);
criterion_main!(benches);
