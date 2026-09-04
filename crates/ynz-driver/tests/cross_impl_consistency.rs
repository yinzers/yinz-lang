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
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

/// The v0.3-M8 Phase 8 structured-fuzzing generator. It lives in a subdirectory module, not a
/// second test target, precisely so the generated-corpus sweep at the bottom of this file can
/// reuse THIS file's oracle (`outputs_match`, `output_order_is_scheduler_dependent`,
/// `run_ynz_mode`, `parallel_sweep`) instead of growing a second copy of it — the
/// `authoritative-derivation.md` constraint applied to the test tree.
mod fuzz_grammar;

/// Run `f` over every corpus entry across all available cores, concatenating the
/// per-entry findings.
///
/// WHY parallel: each corpus entry is fully independent. `f` spawns `ynz run` as a child
/// process, compares the captured strings, and returns findings — nothing is shared and
/// nothing is ordered. Concurrent invocations cannot collide on disk either: `ynz run`
/// builds into its OWN per-invocation temp directory (random name, mode 0o700 — see the
/// contract on `crates/ynz-driver/src/run.rs::run`).
///
/// WHY it matters: run serially, this sweep was the single most expensive thing in the
/// workspace — 2291s of a 3108s full-suite run (74% of total wall clock) from just two
/// tests, holding one core of sixteen while every other test binary queued behind it.
///
/// WHY an atomic cursor instead of pre-chunked ranges: per-fixture cost varies by more
/// than an order of magnitude (a two-line program vs. a suspension-heavy state machine),
/// so fixed chunks would strand cores waiting on whichever chunk drew the slow tail.
/// Workers claim the next index as they free up.
fn parallel_sweep<F>(corpus: &[PathBuf], f: F) -> Vec<String>
where
    F: Fn(&Path) -> Vec<String> + Sync,
{
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(corpus.len().max(1));
    let cursor = AtomicUsize::new(0);
    let collected = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let idx = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(path) = corpus.get(idx) else { break };
                let mut found = f(path);
                if !found.is_empty() {
                    collected
                        .lock()
                        .expect("findings mutex poisoned")
                        .append(&mut found);
                }
            });
        }
    });

    let mut out = collected.into_inner().expect("findings mutex poisoned");
    // Completion order is nondeterministic; sort so a failing run reports the same text
    // every time. (A determinism harness with nondeterministic failure output would be a
    // poor joke at the next reader's expense.)
    out.sort();
    out
}

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
    let out = ynz_cmd(path, no_auto_parallel, no_optimize)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn ynz: {e}"));
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

/// The single place the `ynz run` invocation (args + both mode env vars) is built. Both the
/// blocking `run_ynz_mode` above and the bounded `run_ynz_mode_bounded` below go through it, so
/// the two runners cannot drift on how a mode is selected.
fn ynz_cmd(path: &Path, no_auto_parallel: bool, no_optimize: bool) -> Command {
    let mut cmd = Command::new(ynz_binary());
    cmd.args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0");
    if no_auto_parallel {
        cmd.env("YNZ_NO_AUTO_PARALLEL", "1");
    }
    if no_optimize {
        cmd.env("YNZ_NO_OPTIMIZE", "1");
    }
    cmd
}

/// `run_ynz_mode` with a LIVENESS bound: returns `None` when the child outlived `budget`
/// (after killing it), `Some((stdout, stderr, exit_code))` otherwise.
///
/// WHY it exists only for the generated corpus: a hand-written fixture that hangs is a bug
/// someone will notice within one `cargo test`. A GENERATED program that hangs would wedge a CI
/// job with no fixture name to blame, so the fuzzing sweep must be able to report "timed out"
/// as a finding rather than becoming one. The hand-written sweeps keep the unbounded runner —
/// this adds a bound where it is needed and changes nothing where it is not.
///
/// WHY files instead of pipes: reading `Child::stdout` while polling `try_wait` risks filling
/// the pipe buffer and deadlocking the very thing the budget is meant to bound. Redirecting to
/// files sidesteps it. Per `~/.claude/rules/testing.md` the budget is a LIVENESS timeout, not a
/// performance assertion — it is set an order of magnitude above the observed per-program cost.
fn run_ynz_mode_bounded(
    path: &Path,
    no_auto_parallel: bool,
    no_optimize: bool,
    budget: Duration,
    scratch: &Path,
) -> Option<(String, String, i32)> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("prog");
    let tag = format!("{stem}-{}{}", no_auto_parallel as u8, no_optimize as u8);
    let out_path = scratch.join(format!("{tag}.out"));
    let err_path = scratch.join(format!("{tag}.err"));
    let out_file = std::fs::File::create(&out_path).expect("create stdout capture file");
    let err_file = std::fs::File::create(&err_path).expect("create stderr capture file");

    let mut child = ynz_cmd(path, no_auto_parallel, no_optimize)
        .stdout(out_file)
        .stderr(err_file)
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn ynz: {e}"));

    let started = Instant::now();
    let status = loop {
        match child.try_wait().expect("try_wait on ynz child") {
            Some(status) => break status,
            None if started.elapsed() >= budget => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };

    Some((
        std::fs::read_to_string(&out_path).unwrap_or_default(),
        std::fs::read_to_string(&err_path).unwrap_or_default(),
        status.code().unwrap_or(-1),
    ))
}

/// True when a program's OUTPUT ORDERING is scheduler-dependent — i.e. it spawns `background`
/// work, so the interleaving of its prints is not a property the language guarantees.
///
/// AUTHORITATIVE SOURCE, not a name proxy. Both sweeps below previously decided this by testing
/// the FILE NAME for the substrings "timing"/"background"/"concurrent". That is the
/// `authoritative-derivation.md` twin: the real property lives in the source text, and the proxy
/// drifted from it — 91 corpus fixtures use `background`, only 16 are NAMED for it, leaving 75
/// asserting a byte-identical ordering the language never promised. They passed only because a
/// serially-run sweep left the machine idle enough for the interleaving to repeat by luck;
/// parallelizing the sweep (this same commit series) started flipping them. The first to fall was
/// `v0_3_m4_p3_cross_copy.ynz`, which printed `q3\n42\n...` on one run and `42\n...\nq3` on the
/// next — same lines, same exit code, different order.
///
/// Reading the source is the fix, and it is cheap: each corpus file is read once per sweep,
/// against ~4 compile+link+execute cycles for the same file.
fn output_order_is_scheduler_dependent(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|src| src.contains("background"))
        .unwrap_or(false)
}

/// Compare two program outputs, relaxing ORDER (and only order) when the program's ordering is
/// scheduler-dependent.
///
/// WHY NOT simply exclude those fixtures: this harness exists BECAUSE v0.3-M1 introduced
/// background scheduling (see the module header) — background programs are its whole subject.
/// Dropping all 91 would leave a "determinism harness" that skips every concurrent program, which
/// is `no-duct-tape.md`'s six-month-contributor test failing on the spot.
///
/// WHAT IS STILL STRICT, for every fixture without exception: the exit code, the stderr text, and
/// the complete multiset of stdout lines. A dropped line, a duplicated line, a wrong VALUE, or a
/// changed exit code all still fail. The ONLY thing relaxed is the sequence in which concurrently
/// produced lines appear — which per `IMP-concurrency.md`'s Model A is not a codegen property at
/// all (`wait` is the user's ordering tool), so asserting it was asserting a guarantee Yinz does
/// not make.
fn outputs_match(a: &str, b: &str, order_sensitive: bool) -> bool {
    if order_sensitive {
        return a == b;
    }
    let mut a_lines: Vec<&str> = a.lines().collect();
    let mut b_lines: Vec<&str> = b.lines().collect();
    a_lines.sort_unstable();
    b_lines.sort_unstable();
    a_lines == b_lines
}

#[cfg(test)]
mod outputs_match_contract {
    use super::outputs_match;

    // WHY these exist: `outputs_match` RELAXES an assertion on the corpus sweep — this
    // workspace's strongest silent-miscompile guard. A relaxation that quietly stopped
    // catching real divergence would be strictly worse than the flaky strictness it replaced,
    // and it would fail silently (green forever). These lock the exact boundary: order is
    // forgiven, nothing else is.

    #[test]
    fn reordering_is_forgiven_only_when_order_insensitive() {
        assert!(outputs_match("q3\n42\ndone\n", "42\ndone\nq3\n", false));
        // The same pair MUST still fail under strict comparison — the relaxation has to be
        // opt-in per fixture, never the global default.
        assert!(!outputs_match("q3\n42\ndone\n", "42\ndone\nq3\n", true));
    }

    #[test]
    fn a_wrong_value_still_fails_even_when_order_insensitive() {
        // The miscompile shape this sweep exists to catch: same line count, same ordering,
        // one value silently different.
        assert!(!outputs_match("42\ndone\n", "43\ndone\n", false));
    }

    #[test]
    fn a_missing_or_extra_line_still_fails_even_when_order_insensitive() {
        assert!(!outputs_match("a\nb\n", "a\n", false));
        assert!(!outputs_match("a\n", "a\nb\n", false));
    }

    #[test]
    fn duplicate_lines_are_multiset_compared_not_set_compared() {
        // Sorted-Vec, not HashSet: a line emitted twice where it should appear once is a real
        // defect (a loop running an extra iteration, a double-flush) and must not be collapsed.
        assert!(!outputs_match("a\na\n", "a\n", false));
        assert!(outputs_match("a\na\nb\n", "b\na\na\n", false));
    }

    #[test]
    fn identical_output_matches_under_both_modes() {
        assert!(outputs_match("a\nb\n", "a\nb\n", true));
        assert!(outputs_match("a\nb\n", "a\nb\n", false));
    }
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

    let failures = parallel_sweep(&corpus, |path| {
        let mut failures: Vec<String> = Vec::new();

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
                    // KNOWN-DEFECT PIN, not a scheduling-ordering fixture: a `background` task's
                    // array argument is stored into an outer container it ALIASES
                    // (`bucket.add(rows)`); the ladder frees the clone at task retire while the
                    // parent's `bucket[0]` still holds the now-dangling pointer. Its output is a
                    // deterministic alloc-counter READING of that use-after-free, not scheduler
                    // interleaving — genuinely nondeterministic run-to-run (confirmed: green then
                    // red then red-with-two-failures across three back-to-back identical-tree
                    // runs), so it fails this determinism sweep by design, not by drift. The
                    // defect is deferred, not fixed: Future Requirements #8 in
                    // `.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/plan.md`.
                    // Its own dedicated RED-pin test
                    // (`bg_arg_alias_container_add_is_a_known_uaf_red_pin` in this crate's
                    // `integration.rs`) owns the contract and asserts today's wrong-but-stable
                    // (alloc-counter-observed) behavior — do not delete this exclusion or the
                    // fixture to "fix" this sweep; remove it only in the same change that closes
                    // the alias-fall-through producer and flips the RED-pin test to green-world.
                    // test-ratchet: known-defect pin — dangling-pointer-influenced allocator state read on purpose; excluded, not weakened (next line)
                    || n == "bg_arg_alias_container_add_red.ynz"
            })
            .unwrap_or(false);

        // Ordering is relaxed ONLY for programs that actually spawn background work — derived
        // from the source, never from the file name (see `output_order_is_scheduler_dependent`).
        // Values, line multiset, stderr and exit code stay strict for every fixture.
        let order_sensitive = !output_order_is_scheduler_dependent(path);

        if !is_timing_fixture
            && (!outputs_match(&run1_out, &run2_out, order_sensitive)
                || !outputs_match(&run1_err, &run2_err, order_sensitive)
                || run1_code != run2_code)
        {
            failures.push(format!(
                "NON-DETERMINISTIC{}: {:?}\n  run1 stdout: {:?}\n  run2 stdout: {:?}\n  run1 exit: {run1_code}, run2 exit: {run2_code}",
                if order_sensitive { "" } else { " (order-insensitive: program uses `background`; line multiset differs, not just their order)" },
                path.file_name().unwrap_or_default(),
                &run1_out[..run1_out.len().min(200)],
                &run2_out[..run2_out.len().min(200)],
            ));
        }

        failures
    });

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

    let compared = AtomicUsize::new(0);

    let failures = parallel_sweep(&corpus, |path| {
        let mut failures: Vec<String> = Vec::new();

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
            // KNOWN-DEFECT PIN, not scheduling nondeterminism: see the matching exclusion + full
            // WHY in corpus_produces_deterministic_output_across_runs above — a `background`
            // task's array argument aliased into an outer container, freed by the ladder while
            // the parent still holds the dangling pointer. The output is a deterministic
            // alloc-counter READING of that use-after-free, genuinely varying run-to-run, so a
            // cross-mode byte comparison here would flag the known, deferred defect (Future
            // Requirements #8, `.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/plan.md`)
            // rather than a real mode-divergence. Owned by the dedicated RED-pin test
            // (`bg_arg_alias_container_add_is_a_known_uaf_red_pin`). Remove only alongside the fix.
            // test-ratchet: known-defect pin — dangling-pointer-influenced allocator state read on purpose; excluded, not weakened (next line)
            || name == "bg_arg_alias_container_add_red.ynz"
            || (name == "entrypoint.ynz"
                && path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    == Some("pirates-roster"));
        if is_scheduling_nondeterministic {
            return failures;
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
        compared.fetch_add(1, Ordering::Relaxed);

        // Same authoritative source as the determinism sweep above — the CLUSTERED sibling of the
        // same defect (root-cause.md: two findings sharing an ancestor get ONE fix at the
        // ancestor). This sweep carried an identical name-substring filter with the identical
        // 75-fixture blind spot; both now consult `output_order_is_scheduler_dependent`.
        let order_sensitive = !output_order_is_scheduler_dependent(path);

        for (label, no_auto_parallel, no_optimize) in variants {
            let (var_out, var_err, var_code) = run_ynz_mode(path, no_auto_parallel, no_optimize);
            let stderr_diverges_by_design = soa_lint_fixture && no_auto_parallel;
            let stderr_mismatch =
                !stderr_diverges_by_design && !outputs_match(&base_err, &var_err, order_sensitive);
            if !outputs_match(&base_out, &var_out, order_sensitive)
                || stderr_mismatch
                || base_code != var_code
            {
                failures.push(format!(
                    "MODE-DIVERGENT{}: {:?} [{label}]\n  default (parallel+optimized) stdout: {:?} exit {base_code}\n  {label} stdout: {:?} exit {var_code}",
                    if order_sensitive { "" } else { " (order-insensitive: program uses `background`; line multiset differs, not just their order)" },
                    path.file_name().unwrap_or_default(),
                    &base_out[..base_out.len().min(200)],
                    &var_out[..var_out.len().min(200)],
                ));
            }
        }

        failures
    });

    let compared = compared.into_inner();
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

// ═════════════════════════════════════════════════════════════════════════════
// Structured fuzzing (v0.3-M8 Phase 8, Track 4b)
// ═════════════════════════════════════════════════════════════════════════════
//
// The generator (`fuzz_grammar`) supplies programs; THIS file's oracle judges them. Nothing
// about the judgment is re-derived here: the mode matrix, `outputs_match`'s order relaxation,
// and `output_order_is_scheduler_dependent`'s source-derived classification are all the same
// ones the hand-written corpus above runs under. A generated `background` program is
// auto-classified as scheduler-order-dependent with no exclusion list to remember, which is
// exactly why that classifier had to read the source rather than the file name.
//
// Full scope, budget and replay documentation: `tests/fuzz_grammar/README.md`.

/// Corpus size for a plain `cargo test` run. Deliberately small — the local/CI knob is
/// `YNZ_FUZZ_PROGRAMS`, and the default must not turn `cargo test --workspace` into a
/// fuzzing session.
const FUZZ_DEFAULT_PROGRAMS: usize = 24;

/// LIVENESS bound per (program × mode) invocation — compile, link and execute. Generated
/// programs sleep at most a few milliseconds; the observed worst case is a couple of seconds of
/// LLVM work under a fully loaded sweep. 90s is the order-of-magnitude headroom
/// `~/.claude/rules/testing.md` asks for: it catches a genuine hang and cannot fail a slow
/// machine.
const FUZZ_RUN_BUDGET: Duration = Duration::from_secs(90);

fn fuzz_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// WHY: the whole point of Track 4b. Every program the grammar can emit must observe the SAME
// stdout multiset, the same stderr and the same exit code across the full 2×2 compilation-mode
// matrix — a divergence is a silent miscompile in a shape nobody wrote a fixture for.
//
// Three failure kinds are reported, each by program (never as one "sweep failed"):
//   - GENERATOR BUG   — the program did not compile or exited non-zero. The generator claims
//                       type-validity by construction, so this is its defect, not the
//                       compiler's; the source is saved for the report.
//   - TIMED OUT       — the program outlived FUZZ_RUN_BUDGET in some mode.
//   - MODE-DIVERGENT  — the finding this harness exists for.
//
// Vacuity guard: a corpus of zero programs FAILS. A "passing" fuzz lane that generated nothing
// is the exact shape of green rot this milestone's loom lane also guards against.
#[test]
fn generated_corpus_byte_identical_across_mode_matrix() {
    let programs: usize = fuzz_env("YNZ_FUZZ_PROGRAMS", FUZZ_DEFAULT_PROGRAMS);
    let base_seed: u64 = fuzz_env("YNZ_FUZZ_SEED", 0u64);

    assert!(
        programs > 0,
        "vacuity guard: YNZ_FUZZ_PROGRAMS resolved to 0 — a fuzz lane that generates nothing \
         must fail, not pass"
    );

    let dir = tempfile::Builder::new()
        .prefix("ynz-fuzz-")
        .tempdir()
        .expect("create fuzz scratch dir");
    let scratch = dir.path().join("capture");
    std::fs::create_dir_all(&scratch).expect("create capture dir");

    let started = Instant::now();
    let mut corpus = Vec::with_capacity(programs);
    let mut sources = std::collections::BTreeSet::new();
    let mut concurrent = 0usize;
    for i in 0..programs {
        let seed = base_seed.wrapping_add(i as u64);
        let program = fuzz_grammar::generate(seed);
        if program.uses_background {
            concurrent += 1;
        }
        sources.insert(program.source.clone());
        // Named from the program's OWN recorded seed, not from the local `seed` that produced
        // it — so the replay key in the filename cannot drift from the one in the file's header
        // comment even if the generator's seeding ever changes.
        let path = dir.path().join(format!("gen_{:020}.ynz", program.seed));
        std::fs::write(&path, &program.source).expect("write generated program");
        corpus.push(path);
    }

    // Anti-triviality: a hit rate propped up by re-emitting one program measures nothing.
    let distinct = sources.len();
    assert!(
        distinct * 10 >= programs * 9,
        "only {distinct}/{programs} generated programs are distinct — the grammar has collapsed"
    );

    let ran_ok = AtomicUsize::new(0);

    let findings = parallel_sweep(&corpus, |path| {
        let mut findings: Vec<String> = Vec::new();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let Some((base_out, base_err, base_code)) =
            run_ynz_mode_bounded(path, false, false, FUZZ_RUN_BUDGET, &scratch)
        else {
            findings.push(format!(
                "TIMED OUT: {name} [default (parallel+optimized)] after {}s\n{}",
                FUZZ_RUN_BUDGET.as_secs(),
                indent_source(path)
            ));
            return findings;
        };

        if base_code != 0 {
            // The generator's own contract is "type-valid by construction". A rejection here
            // is a GENERATOR bug (the grammar drifted past what the compiler accepts), and it
            // is also exactly what the Phase 8 spike measures as its hit rate.
            findings.push(format!(
                "GENERATOR BUG (program did not compile or exited {base_code}): {name}\n  \
                 stderr: {}\n{}",
                truncate(&base_err, 1600),
                indent_source(path)
            ));
            return findings;
        }
        ran_ok.fetch_add(1, Ordering::Relaxed);

        // Same authoritative classifier the hand-written sweeps use — derived from the source
        // text, never from the file name.
        let order_sensitive = !output_order_is_scheduler_dependent(path);

        let variants: [(&str, bool, bool); 3] = [
            ("sequential+optimized", true, false),
            ("parallel+no-optimize", false, true),
            ("sequential+no-optimize", true, true),
        ];
        for (label, no_auto_parallel, no_optimize) in variants {
            let Some((var_out, var_err, var_code)) = run_ynz_mode_bounded(
                path,
                no_auto_parallel,
                no_optimize,
                FUZZ_RUN_BUDGET,
                &scratch,
            ) else {
                findings.push(format!(
                    "TIMED OUT: {name} [{label}] after {}s\n{}",
                    FUZZ_RUN_BUDGET.as_secs(),
                    indent_source(path)
                ));
                continue;
            };

            if !outputs_match(&base_out, &var_out, order_sensitive)
                || !outputs_match(&base_err, &var_err, order_sensitive)
                || base_code != var_code
            {
                findings.push(format!(
                    "MODE-DIVERGENT{}: {name} [{label}]\n  default stdout: {:?} exit {base_code}\n  \
                     {label} stdout: {:?} exit {var_code}\n  default stderr: {:?}\n  {label} \
                     stderr: {:?}\n{}",
                    if order_sensitive {
                        ""
                    } else {
                        " (order-insensitive: program uses `background`; line multiset differs, \
                          not just their order)"
                    },
                    truncate(&base_out, 600),
                    truncate(&var_out, 600),
                    truncate(&base_err, 600),
                    truncate(&var_err, 600),
                    indent_source(path),
                ));
            }
        }

        findings
    });

    let ran_ok = ran_ok.into_inner();
    let elapsed = started.elapsed();
    // Printed on every run (visible with --nocapture, and always on failure): the spike's three
    // numbers, so a CI log answers "did this lane actually do anything?" without a rerun.
    println!(
        "fuzz corpus: seed base {base_seed}, {programs} generated ({distinct} distinct, \
         {concurrent} spawning `background`), {ran_ok} compiled and ran to exit 0, \
         {} findings, {:.1}s wall clock",
        findings.len(),
        elapsed.as_secs_f64()
    );

    if !findings.is_empty() {
        // Keep the corpus on disk so a failing program can be replayed by hand. (The seed
        // alone reproduces it, but a saved file survives a generator revision.)
        let kept = dir.keep();
        panic!(
            "structured-fuzzing findings ({} across {programs} generated programs; corpus kept at \
             {}):\n\n{}",
            findings.len(),
            kept.display(),
            findings.join("\n\n")
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…<{} more bytes>", &s[..max], s.len() - max)
    }
}

/// The generated source, indented — a finding is only actionable with the program attached,
/// and a generated program has no name a reader can look up.
fn indent_source(path: &Path) -> String {
    let src = std::fs::read_to_string(path).unwrap_or_default();
    let body: String = src
        .lines()
        .map(|l| format!("    {l}\n"))
        .collect::<Vec<_>>()
        .concat();
    format!("  --- generated source ---\n{body}  --- end ---")
}
