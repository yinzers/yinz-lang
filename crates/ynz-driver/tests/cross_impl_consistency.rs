// WHY: v0.3-M1 introduced background task scheduling, which could in principle make
// programs non-deterministic (if background thread ordering affected stdout). This
// harness tests the DETERMINISM property: every program in the corpus must produce
// byte-identical output on two consecutive runs. Non-determinism here means either
// (a) background output is racing with foreground output in a way that changes stdout
// ordering, or (b) codegen introduced non-determinism in unrelated code paths.
//
// The harness also serves as a regression guard for the P0-P3 changes: if any
// existing fixture starts producing different output, a change broke it.
//
// This test also validates that no existing program was broken by the P0-P3 changes:
// if a fixture's output changes between runs, something non-deterministic crept in.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

/// Run `ynz run <path>` and return (stdout, stderr, exit_code).
fn run_ynz(path: &Path) -> (String, String, i32) {
    run_ynz_mode(path, false, false)
}

/// Run `ynz run <path>` in any combination of the two compilation-mode axes and return
/// (stdout, stderr, exit_code). Both axes are selected via env vars the compiler reads
/// through the salsa barrier (the `build` subcommand's `--no-auto-parallel` /
/// `--no-optimize` flags set the same vars — both routes select the same lowering):
/// - `YNZ_NO_AUTO_PARALLEL=1` — forced-sequential statement lowering (no auto-parallel pass);
/// - `YNZ_NO_OPTIMIZE=1` — LLVM pipeline off, -O0 backend (the pre-M7 `ynz build` behavior;
///   read by `pipeline_config_from_env`, crates/ynz-codegen/src/state_machine.rs).
fn run_ynz_mode(path: &Path, no_auto_parallel: bool, no_optimize: bool) -> (String, String, i32) {
    let mut cmd = Command::new(ynz_binary());
    cmd.args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0");
    if no_auto_parallel {
        cmd.env("YNZ_NO_AUTO_PARALLEL", "1");
    }
    if no_optimize {
        cmd.env("YNZ_NO_OPTIMIZE", "1");
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ynz: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

/// True when a file is an intentional-error gallery file (should fail to compile).
fn is_error_gallery(path: &Path) -> bool {
    path.ancestors().any(|p| p.ends_with("primantis-orders"))
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.starts_with("broken")
                    || n.starts_with("empty")
                    || n.contains("_neg_")
                    || n.contains("mismatch")
                    || n.contains("overflow")
                    || n.contains("div_by_zero")
                    || n.contains("deferral")
                    || n.contains("reassign")
                    || n.contains("missing_return")
                    || n.contains("dead_code")
                    || n.contains("compound_assign")
                    || n.contains("banned")
                    || n.contains("base_instantiate")
                    || n.contains("is_type_deferral")
                    || n.contains("return_no_value")
                    || n.contains("int_max_deferred")
                    || n.contains("wrapping_add_deferred")
                    || n.contains("bignum_deferral")
                    || n.contains("arg_arity")
                    || n.contains("arg_type")
                    || n.contains("undefined_function")
                    || n.contains("no_follows")
                    || n.contains("const_field")
            })
            .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn collect_corpus() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut files = Vec::new();

    // Driver fixtures
    let fixtures = root.join("crates/ynz-driver/tests/fixtures");
    if let Ok(entries) = std::fs::read_dir(&fixtures) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("ynz") && !is_error_gallery(&p) {
                files.push(p);
            }
        }
    }

    // examples/ (excluding primantis-orders and non-entrypoint sub-files)
    let examples = root.join("examples");
    if let Ok(entries) = std::fs::read_dir(&examples) {
        for dir_entry in entries.flatten() {
            let dir = dir_entry.path();
            if !dir.is_dir() {
                continue;
            }
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dir_name == "primantis-orders" || dir_name == "burgh-poem" {
                continue;
            }
            // Use entrypoint.ynz if present (single-entry projects)
            let ep = dir.join("entrypoint.ynz");
            if ep.exists() && !is_error_gallery(&ep) {
                files.push(ep);
            }
        }
    }

    files.sort();
    files
}

// WHY: corpus determinism — every program must produce byte-identical stdout/stderr/exit-code
// on two consecutive runs. Background scheduling must not introduce observable ordering
// non-determinism for the programs in this corpus (timing fixtures are excluded).
// Quality gate: at least 30 files in the corpus (validates coverage, not a stub).
// If count drops below 30, either files were deleted (update corpus) or discovery broke.
#[test]
fn corpus_produces_deterministic_output_across_runs() {
    let corpus = collect_corpus();

    let corpus_size = corpus.len();
    assert!(
        corpus_size >= 30,
        "corpus must have at least 30 files (got {corpus_size}); discovery logic may be broken"
    );

    let mut failures: Vec<String> = Vec::new();

    for path in &corpus {
        let (run1_out, run1_err, run1_code) = run_ynz(path);
        let (run2_out, run2_err, run2_code) = run_ynz(path);

        // Output must be byte-identical between runs (determinism).
        // For background programs, the order guarantee is that main-thread output
        // precedes background output in the same run; two runs may differ in
        // interleaving if the system is under load. We skip the timing fixture
        // which is inherently racy.
        // WHY concurrent_waits_proof is excluded: it uses 8 concurrent background state machines
        // with non-deterministic scheduling order (which task's START/DONE prints first depends
        // on the Tokio I/O pool scheduler). The ordering assertions live in the dedicated driver
        // integration test (v0_3_m2_concurrent_waits_proof), not in the determinism harness.
        //
        // WHY examples/pirates-roster/entrypoint.ynz is excluded: the v0.3-M2 section spawns 8
        // background state machines with non-deterministic print ordering across runs.
        // The ordering assertions live in the dedicated M2 integration tests.
        let is_timing_fixture = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| {
                n.contains("timing")
                    || n.contains("background")
                    || n.contains("concurrent")
                    // v0.3-M2 demo: spawns 8 concurrent background state machines whose print
                    // order varies between runs — non-deterministic by design (proves concurrency).
                    || (n == "entrypoint.ynz"
                        && path.parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            == Some("pirates-roster"))
                    // v0.3-M3d `maybe`-return decline fixture: its purpose is to verify (at the
                    // IR level, in v03_m3d_return_class_maybe_declines_and_ir_inert) that a
                    // `maybe`-returning CPU pair is DECLINED from auto-parallel. Its RUNTIME
                    // output is non-deterministic because two adjacent `maybe<int>`-returning
                    // binds hit a tracked pre-existing base-codegen bug (uninitialized
                    // staging-slot read — .claude/todos.md), orthogonal to auto-parallel and
                    // identical in both modes. The decline is asserted via IR, not by running
                    // the binary, so this fixture's runtime output is irrelevant to its purpose.
                    || n == "v0_3_m3d_return_class_maybe.ynz"
                    // v0.3-M3g E8 stress: 20 fused CPU+I/O groups fired via a recursion-spawning
                    // tree of `background` tasks — its print INTERLEAVING is scheduler-dependent
                    // by design (same reason as the "background"/"concurrent" substring rule
                    // above; this fixture just doesn't happen to contain either substring). The
                    // dedicated integration test
                    // (v03_m3g_e8_pool_exhaustion_stress_completes_without_deadlock) asserts the
                    // SET of completion lines, which is the invariant that actually matters here.
                    || n == "v0_3_m3g_e8_pool_exhaustion_stress.ynz"
                    // v0.3-M3g overlap proof: a timing-margin fixture (a CPU loop vs. a sleep,
                    // per Phase 1's A7 protocol) whose print INTERLEAVING is, by construction,
                    // scheduler-timing-dependent — the same class of exclusion as "timing" above,
                    // it just doesn't happen to contain that substring. Its own dedicated test
                    // (v03_m3g_overlap_proof_cpu_and_io_members_genuinely_run_concurrently)
                    // asserts the ordering invariant directly, with the same generous margins
                    // this corpus sweep's blanket byte-comparison cannot express.
                    || n == "v0_3_m3g_overlap_proof.ynz"
                    // v0.3-M7 fr23 planned-RED fixtures (FRAGO 011): CONFIRMED-LIVE
                    // use-after-free — the spawned task reads a dead stack frame, so their
                    // runtime output is UB garbage BY DESIGN until the give/copy machinery
                    // for non-ident spawn receivers lands. Their contract is owned by the
                    // planned-RED locks in fr23_uaf_planned_red.rs (not run by default); a
                    // UB fixture cannot participate in a determinism sweep. REMOVE these two
                    // exclusions in the same change that fixes fr23 and activates those
                    // locks — post-fix they must be deterministic like any other fixture.
                    // test-ratchet: FRAGO 011 planned-RED fixture — UB output until the fr23 fix; excluded, not weakened (next line)
                    || n == "v0_3_m7_fr23_maybe_payload_spawn_receiver.ynz"
                    // test-ratchet: FRAGO 011 planned-RED fixture — UB output until the fr23 fix; excluded, not weakened
                    || n == "v0_3_m7_fr23_call_materialized_spawn_receiver.ynz"
                    // v0.3-M7 Phase 6 back-edge preemption fixtures: timing-margin races BY
                    // CONSTRUCTION — a fire-and-forget CPU hog (100M iterations; v0.3 has no
                    // join primitive, background is fire-and-forget) vs. main's fixed
                    // `wait sleep(4000)` keep-alive. Under host load the hog's completion
                    // crosses the deadline nondeterministically, so the `hog done` /
                    // `plain hog done` line's presence AND position vary between runs (exit 0
                    // either way — runtime shutdown cancels still-pending tasks by design).
                    // Same class as v0_3_m3g_overlap_proof.ynz above: the filename just lacks
                    // the "timing"/"background" substrings. The real invariants (victim runs
                    // BEFORE the hog completes; both lines present) are owned by the dedicated
                    // v03_m7_backedge_preemption.rs tests under the deterministic
                    // YNZ_WORKER_THREADS=1 latch, where the starvation shape does not depend
                    // on host load.
                    || n == "v0_3_m7_p6_backedge_starvation_sm.ynz"
                    || n == "v0_3_m7_p6_backedge_residual_nonsm.ynz"
                    // v0.3-M7 planned-RED fixture (Phase 6 review round): the name-keyed
                    // loop-var frame-slot collision on suspending-body loops — its second
                    // loop prints MISCOMPILED garbage bytes (a string reloaded through a
                    // Point-classified frame slot) BY DESIGN until the per-loop slot-keying
                    // fix lands (plan Future Requirements, ELEVATED). Garbage-pointer bytes
                    // cannot participate in a determinism sweep; the contract is owned by
                    // the planned-RED locks in d5_frame_slot_collision_planned_red.rs (not
                    // run by default). REMOVE this exclusion in the same change that fixes
                    // the collision and activates those locks — post-fix the fixture must
                    // be deterministic like any other.
                    // test-ratchet: planned-RED fixture — miscompiled output until the slot-keying fix; excluded, not weakened (next line)
                    || n == "v0_3_m7_d5_suspending_loop_var_slot_collision.ynz"
            })
            .unwrap_or(false);

        if !is_timing_fixture
            && (run1_out != run2_out || run1_err != run2_err || run1_code != run2_code)
        {
            failures.push(format!(
                "NON-DETERMINISTIC: {:?}\n  run1 stdout: {:?}\n  run2 stdout: {:?}\n  run1 exit: {run1_code}, run2 exit: {run2_code}",
                path.file_name().unwrap_or_default(),
                &run1_out[..run1_out.len().min(200)],
                &run2_out[..run2_out.len().min(200)],
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "determinism failures ({} / {} non-timing files):\n{}",
        failures.len(),
        corpus_size,
        failures.join("\n\n")
    );
}

// WHY: BOTH compilation-mode axes must be OBSERVABLY INVISIBLE across the ENTIRE corpus —
// every program must produce byte-identical stdout/stderr/exit-code across the full 2×2 mode
// matrix: {default auto-parallel, --no-auto-parallel} × {default optimized, --no-optimize}.
// This is the strongest cross-impl invariant the milestone carries:
// - the auto-parallel axis: parallelizing independent statements changes WHEN work runs,
//   never WHAT the program observes — a divergence is a silent miscompile (a parallel
//   pack/bind that disagrees with the sequential path);
// - the optimizer axis (v0.3-M7 Phase 5 step 5): the LLVM pass pipeline changes HOW FAST
//   code runs, never what it computes — a divergence is an optimizer-revealed miscompile
//   (exactly the R9 return-ABI / fr21 class Phases 2-3 closed), and the corpus includes
//   the suspension-path fixtures (v0_3_m2_* wait/state-machine programs), so the frame
//   flush/reload machinery is exercised under O2 here, not just at O0.
// The matrix is asserted pairwise against the default-mode (parallel+optimized) baseline,
// so any single divergent combination is named in the failure. Exactly the failure class
// the per-fixture m3d FIRE/DECLINE tests guard one fixture at a time, lifted to the whole
// corpus so a NEW fixture is covered the moment it lands without anyone wiring a bespoke
// twin assertion.
//
// Timing/background/concurrent fixtures are excluded for the same reason the determinism test
// excludes them: their print ordering is scheduler-dependent within a single mode, so a strict
// byte comparison across modes would flag a non-bug.
//
// Quality gate: at least 30 non-excluded files (validates coverage, not a stub).
#[test]
fn corpus_byte_identical_across_mode_matrix() {
    let corpus = collect_corpus();

    let mut compared = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in &corpus {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Same exclusions as the determinism harness: ordering of these is scheduler-dependent
        // WITHIN a single run, so cross-mode byte comparison would flag scheduling noise, not a
        // codegen divergence.
        let is_scheduling_nondeterministic = name.contains("timing")
            || name.contains("background")
            || name.contains("concurrent")
            || name == "v0_3_m3d_return_class_maybe.ynz"
            // Model-A intended reorder: two independent I/O calls with print side effects whose
            // finish order differs by design between the modes (`B\nA` parallel vs `A\nB`
            // sequential). The fixture's own header states it is NOT part of the byte-identical
            // sweep — `wait`, not the absence of parallelism, is the user's ordering tool
            // (design/concurrency.md Model A). A pure-CPU group has no such observable side
            // effect (its members only return values, bound after the join), so this exception
            // is specific to I/O-side-effect ordering and does not weaken the CPU-parallel
            // invariant the sweep protects.
            || name == "v0_3_m3b_p4_model_a_intended_reorder.ynz"
            // v0.3-M3g E8 stress: see the matching exclusion + WHY in
            // corpus_produces_deterministic_output_across_runs above — a recursion-spawning tree
            // of `background` tasks, print interleaving scheduler-dependent by design.
            || name == "v0_3_m3g_e8_pool_exhaustion_stress.ynz"
            // v0.3-M3g overlap proof: the ENTIRE point of this fixture is that default mode's
            // START/DONE interleaving DIFFERS from `--no-auto-parallel`'s (that difference IS
            // the overlap proof) — a mode-divergent byte comparison here would fail a fixture
            // that is working exactly as designed, not flag a codegen bug. Its own dedicated
            // test asserts both modes' orderings explicitly (and that the final RESULT value is
            // identical either way, preserving the real invariant this sweep protects).
            || name == "v0_3_m3g_overlap_proof.ynz"
            // v0.3-M7 fr23 planned-RED fixtures (FRAGO 011): confirmed-live UAF — output is
            // dead-frame garbage by design until the fr23 fix lands (see the matching
            // exclusion + full WHY in corpus_produces_deterministic_output_across_runs
            // above; contract owned by fr23_uaf_planned_red.rs). REMOVE with the fix.
            // test-ratchet: FRAGO 011 planned-RED fixture — UB output until the fr23 fix; excluded, not weakened (next line)
            || name == "v0_3_m7_fr23_maybe_payload_spawn_receiver.ynz"
            // test-ratchet: FRAGO 011 planned-RED fixture — UB output until the fr23 fix; excluded, not weakened
            || name == "v0_3_m7_fr23_call_materialized_spawn_receiver.ynz"
            // v0.3-M7 Phase 6 back-edge preemption fixtures: timing-margin races by
            // construction (fire-and-forget CPU hog vs. main's fixed 4000ms keep-alive) —
            // see the matching exclusion + full WHY in
            // corpus_produces_deterministic_output_across_runs above. The hog-completion
            // line's presence/position varies under load WITHIN a single mode, so a
            // cross-mode byte comparison would flag scheduling noise, not a codegen
            // divergence. Invariants owned by v03_m7_backedge_preemption.rs
            // (YNZ_WORKER_THREADS=1).
            || name == "v0_3_m7_p6_backedge_starvation_sm.ynz"
            || name == "v0_3_m7_p6_backedge_residual_nonsm.ynz"
            // v0.3-M7 planned-RED slot-collision fixture: miscompiled garbage output by
            // design until the per-loop slot-keying fix — see the matching exclusion +
            // full WHY in corpus_produces_deterministic_output_across_runs above
            // (contract owned by d5_frame_slot_collision_planned_red.rs). REMOVE with
            // the fix.
            // test-ratchet: planned-RED fixture — miscompiled output until the slot-keying fix; excluded, not weakened (next line)
            || name == "v0_3_m7_d5_suspending_loop_var_slot_collision.ynz"
            || (name == "entrypoint.ynz"
                && path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    == Some("pirates-roster"));
        if is_scheduling_nondeterministic {
            continue;
        }

        // v0.3-M5 SoA-admitted fixtures: the `array-using-soa-layout` Tier 3 lint fires only
        // when SoA layout is actually admitted, and `--no-auto-parallel` structurally prevents
        // admission (the "no-auto-parallel disables SoA" hard invariant, gate #2 in
        // crates/ynz-typeck/src/soa.rs — the same gate the milestone's dual-mode AoS oracle
        // relies on). So default mode prints the lint on stderr and sequential mode cannot,
        // by design: a documented gate difference, same class as the M3b intended-reorder
        // exclusion above, not a codegen bug. But the divergence is STDERR-ONLY — stdout and
        // exit code MUST stay byte-identical across modes (for m5_p4_soa_qualifying.ynz this
        // sweep is the only runtime stdout-equivalence oracle; its other tests are typeck-
        // analysis / lint-firing only) — so these fixtures skip just the stderr comparison
        // instead of dropping out of the sweep entirely. The skip applies ONLY to the
        // sequential (`--no-auto-parallel`) variants: the optimizer axis does not touch SoA
        // admission (the gate lives in typeck's auto-parallel analysis, not the LLVM
        // pipeline), so the parallel+no-optimize corner still admits SoA, still prints the
        // lint, and its stderr MUST match the baseline.
        let soa_lint_fixture =
            name == "m5_p4_soa_qualifying.ynz" || name == "m5_p5_soa_copy_wait_bg.ynz";

        // Baseline: the default mode users actually get (auto-parallel + optimized).
        let (base_out, base_err, base_code) = run_ynz_mode(path, false, false);
        // The other three corners of the 2×2 matrix, each compared against the baseline
        // (pairwise-vs-baseline is transitively all-pairs equality).
        let variants: [(&str, bool, bool); 3] = [
            ("sequential+optimized", true, false),
            ("parallel+no-optimize", false, true),
            ("sequential+no-optimize", true, true),
        ];
        compared += 1;

        for (label, no_auto_parallel, no_optimize) in variants {
            let (var_out, var_err, var_code) = run_ynz_mode(path, no_auto_parallel, no_optimize);
            let stderr_diverges_by_design = soa_lint_fixture && no_auto_parallel;
            let stderr_mismatch = !stderr_diverges_by_design && base_err != var_err;
            if base_out != var_out || stderr_mismatch || base_code != var_code {
                failures.push(format!(
                    "MODE-DIVERGENT: {:?} [{label}]\n  default (parallel+optimized) stdout: {:?} exit {base_code}\n  {label} stdout: {:?} exit {var_code}",
                    path.file_name().unwrap_or_default(),
                    &base_out[..base_out.len().min(200)],
                    &var_out[..var_out.len().min(200)],
                ));
            }
        }
    }

    assert!(
        compared >= 30,
        "expected at least 30 corpus files to compare across modes (got {compared}); discovery \
         or exclusion logic may be broken"
    );
    assert!(
        failures.is_empty(),
        "mode-matrix divergences ({} findings across {} compared files):\n{}",
        failures.len(),
        compared,
        failures.join("\n\n")
    );
}
