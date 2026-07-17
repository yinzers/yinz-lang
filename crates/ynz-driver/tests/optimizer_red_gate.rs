// v0.3-M7 Phase 1 RED gate — the committed proof artifact for risk R1 (optimizer flip
// miscompiles). Every test in this file locks a CONFIRMED O0-reliant codegen path: a
// program whose behavior was correct under the pre-M7 default backend `-O0` but wrong
// (or crashing) once the IR/backend is optimized — the pipeline v0.3-M7 Phase 3 turned
// on by default (`--no-optimize` is the O0 escape hatch).
//
// Phase 1 committed this file as a planned RED set (`#[ignore]`d, failing — the
// no-duct-tape legitimate inverse). v0.3-M7 Phase 2 fixed both root-cause classes and
// removed the `#[ignore]` marks: the gate now runs in the default suite as a permanent
// regression lock. Do NOT weaken an assertion or widen a timeout to force green — a red
// here is a real miscompile.
//
// Two confirmed root-cause classes are locked (evidence: plan
// `2026-07-04-v0-3-m7-optimizer-pipeline`, audit.md Phase-1 session-log entries):
//
// 1. FALSE ALIGNMENT CLAIMS — `load i128`/`store i128` for decimal128 values through
//    8-aligned SM-frame/staging byte-offset pointers carry no explicit alignment, so LLVM
//    defaults to ABI `align 16`. Optimized X86 ISel honors the claim and selects `movaps`
//    (alignment-requiring) → SIGSEGV. Emitting sites: `ynz-codegen/src/emit.rs`
//    (`build_load(i128, staging_ptr, ..)` with no `set_alignment`) +
//    `state_machine.rs` (raw i8 GEP staging pointers).
//
// 2. FALSE OWNERSHIP ATTRIBUTES — `declare_function` (`ynz-codegen/src/emit.rs`) emits
//    `readonly`/`noalias` param attributes from the DECLARED ownership modifier, defaulting
//    bare params to `share`, and never consults typeck's `effective_ownership` analysis
//    (the authoritative answer — computed but unconsumed downstream). Two manifestations:
//    a bare param the body mutates gets a false `readonly` (the store is UB and gets
//    deleted), and an aliasing share+lend call — which typeck ACCEPTED at the time —
//    falsified `noalias` on both params (the read got hoisted past the aliased store).
//    Phase 2 closed the class from both ends: `declare_function` now consumes
//    `effective_ownership`, and typeck rejects the aliasing call outright (FRAGO 002).
//
// Harness: differential O0-vs-optimized. Each fixture is built at the O0 escape-hatch
// tier (`ynz build --no-optimize --emit-ir` — the default build optimizes since v0.3-M7
// Phase 3, so the anchor pins O0 explicitly) and its run output is the correctness
// anchor; the SAME emitted IR is then optimized (`opt-18 -passes=default<O2>` mid-end +
// `llc-18 -O2` backend — the exact pipeline shape the shipped default runs) and linked
// against the same runtime archive. The two runs must agree byte-for-byte. No golden
// outputs are duplicated here: the unoptimized build IS the authoritative expected
// behavior.
//
// Requires the LLVM 18 CLI tools (`opt-18`, `llc-18`, `clang-18`) — run inside the dev
// container (`docker compose run --rm dev cargo test -p ynz-driver --test
// optimizer_red_gate`).

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Watchdog ceiling for a compiled fixture run. Mirrors the integration-suite discipline:
/// a trip is a real deadlock/miscompile, never a slow test — fix the codegen, don't widen this.
const RUN_WATCHDOG: Duration = Duration::from_secs(60);

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The workspace-level runtime static library the optimized binary links against — the
/// same archive `ynz build` embeds and extracts for its own link step.
fn runtime_archive() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/libynz_runtime.a");
    assert!(
        p.exists(),
        "libynz_runtime.a not found at {} — build the workspace first \
         (`cargo build -p ynz-runtime`); this gate links the optimized object against it",
        p.display()
    );
    p
}

fn run_with_watchdog(mut cmd: Command) -> Output {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("watchdog: failed to spawn child process");
    let start = std::time::Instant::now();
    loop {
        match child
            .try_wait()
            .expect("watchdog: failed to poll child status")
        {
            Some(_status) => {
                return child
                    .wait_with_output()
                    .expect("watchdog: failed to collect child output after exit");
            }
            None => {
                if start.elapsed() >= RUN_WATCHDOG {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "WATCHDOG TRIP: fixture did not exit within {RUN_WATCHDOG:?} — treat \
                         as a hang-class miscompile under optimization; fix the codegen, never \
                         widen this timeout"
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// Run `tool args` and panic with the tool's stderr on failure — every step of the
/// optimize-and-link pipeline is a hard precondition, not part of the assertion.
fn run_tool(tool: &str, args: &[&str]) {
    let out = Command::new(tool).args(args).output().unwrap_or_else(|e| {
        panic!(
            "failed to spawn `{tool}` — this gate requires the LLVM 18 CLI tools; \
                 run it inside the dev container: {e}"
        )
    });
    assert!(
        out.status.success(),
        "`{tool} {}` failed:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Which optimization configuration to apply to the emitted IR before the differential run.
///
/// Each RED class is locked under the configuration where its miscompile deterministically
/// manifests (per the Phase-1 bisection evidence) — a class tested under the wrong stage
/// combination can be incidentally masked and the gate would lie green:
/// - `BackendOnly` (`llc -O2` on the raw emitted IR): the false-alignment class — the
///   bisected failure is X86 DAG->DAG instruction selection honoring the false `align 16`;
///   the O2 mid-end incidentally reshapes the i128 accesses on some fixtures and masks it.
/// - `MidEndAndBackend` (`opt -passes=default<O2>` then `llc -O2`): the false-ownership-
///   attribute class — the miscompile is a mid-end exploit (store deletion / load hoisting
///   licensed by false `readonly`/`noalias`).
enum OptStage {
    BackendOnly,
    MidEndAndBackend,
}

/// Differential O0-vs-optimized equivalence check for one fixture.
///
/// Flow: copy fixture to a tmpdir → `ynz build --no-optimize --emit-ir` (the -O0
/// escape hatch — since v0.3-M7 Phase 3 the DEFAULT build optimizes, so the anchor
/// pins the O0 tier explicitly to keep this a true O0-vs-optimized differential)
/// → run the O0 binary (this output is the correctness anchor; the fixture must be
/// healthy at O0 or the gate itself is broken) → optimize the SAME IR per `stage` →
/// link against the runtime archive with the exact flag set `ynz build` uses → run →
/// assert exit code and stdout both match the O0 run byte-for-byte.
fn assert_o0_and_optimized_agree(fixture_name: &str, stage: OptStage) {
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture(fixture_name);
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture into tmpdir");

    // Build unoptimized (--no-optimize pins the O0 anchor tier; the default now
    // optimizes per v0.3-M7 Phase 3) + emit the IR alongside the binary.
    let build_out = Command::new(ynz_binary())
        .args([
            "build",
            "--no-optimize",
            "--emit-ir",
            isolated_src.to_str().unwrap(),
        ])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        build_out.status.success(),
        "ynz build failed for {fixture_name}:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );

    let o0_binary = isolated_src.with_extension("");
    let ir_path = o0_binary.with_extension("ll");
    assert!(
        ir_path.exists(),
        "--emit-ir produced no IR at {}",
        ir_path.display()
    );

    // Correctness anchor: the O0 run. A non-zero exit here means the fixture is broken
    // outright (not an optimizer question) — fail loudly.
    let mut o0_cmd = Command::new(&o0_binary);
    o0_cmd.env("CLICOLOR", "0");
    let o0 = run_with_watchdog(o0_cmd);
    let o0_code = o0.status.code().unwrap_or(-1);
    let o0_stdout = String::from_utf8_lossy(&o0.stdout).into_owned();
    assert_eq!(
        o0_code,
        0,
        "fixture {fixture_name} must be healthy at O0 (the anchor run); stderr:\n{}",
        String::from_utf8_lossy(&o0.stderr)
    );

    // Optimize the same IR per the class's manifesting stage combination.
    let opt_ll = tmp.path().join("optimized.ll");
    let obj = tmp.path().join("optimized.o");
    let opt_binary = tmp.path().join("optimized");
    let llc_input = match stage {
        OptStage::BackendOnly => ir_path.clone(),
        OptStage::MidEndAndBackend => {
            run_tool(
                "opt-18",
                &[
                    "-passes=default<O2>",
                    ir_path.to_str().unwrap(),
                    "-S",
                    "-o",
                    opt_ll.to_str().unwrap(),
                ],
            );
            opt_ll.clone()
        }
    };
    run_tool(
        "llc-18",
        &[
            "-O2",
            "-filetype=obj",
            llc_input.to_str().unwrap(),
            "-o",
            obj.to_str().unwrap(),
        ],
    );
    let archive = runtime_archive();
    run_tool(
        "clang-18",
        &[
            obj.to_str().unwrap(),
            archive.to_str().unwrap(),
            "-no-pie",
            "-o",
            opt_binary.to_str().unwrap(),
            "-lpthread",
            "-ldl",
            "-lm",
            "-lrt",
        ],
    );

    let mut opt_cmd = Command::new(&opt_binary);
    opt_cmd.env("CLICOLOR", "0");
    let optimized = run_with_watchdog(opt_cmd);
    let opt_code = optimized.status.code().unwrap_or(-1);
    let opt_stdout = String::from_utf8_lossy(&optimized.stdout).into_owned();

    assert_eq!(
        opt_code,
        o0_code,
        "{fixture_name}: optimized exit code diverges from O0 (miscompile); \
         optimized stderr:\n{}",
        String::from_utf8_lossy(&optimized.stderr)
    );
    assert_eq!(
        opt_stdout, o0_stdout,
        "{fixture_name}: optimized stdout diverges from O0 (miscompile)"
    );
}

// ── Class 1: false i128 alignment claims (decimal128 through SM-frame staging slots) ──
// The four deterministic single-file fixtures from the Phase-0 spike's failing set. (The
// spike's remaining failure, the pirates-roster demo project, fails on this same class;
// it stays locked by its normal-suite byte-exact golden test rather than here, because its
// background-section output is deliberately order-relaxed and the demo is a multi-file
// project the single-file differential harness does not cover.)

#[test]
fn red_opt_alignment_ec_crossing_local_propagated_number() {
    // WHY: the bisected primary repro — an EC-crossing decimal128 local propagated across
    // a wait; optimized ISel selects movaps against an 8-aligned frame slot and faults.
    assert_o0_and_optimized_agree(
        "v0_3_m3a_p1_ec_crossing_local_propagated_number.ynz",
        OptStage::BackendOnly,
    );
}

#[test]
fn red_opt_alignment_parallel_number_ec_inline_collect() {
    // WHY: the inline-poll parallel number+EC collection — same alignment class through
    // the parallel-group staging path.
    assert_o0_and_optimized_agree(
        "v0_3_m3b_p5_parallel_number_ec_inline_collect.ynz",
        OptStage::BackendOnly,
    );
}

#[test]
fn red_opt_alignment_danger_mixed_number_declines() {
    // WHY: the mixed CPU+I/O group with a decimal128 CPU child — same alignment class
    // through the CPU-trampoline i128 packing path.
    assert_o0_and_optimized_agree(
        "v0_3_m3d_danger_mixed_number_declines.ynz",
        OptStage::BackendOnly,
    );
}

#[test]
fn red_opt_alignment_ec_three_bindings() {
    // WHY: three live bindings of the same `-> number errors` callee — same alignment
    // class through the per-binding stable-storage copies.
    assert_o0_and_optimized_agree("v0_3_m3f_ec_three_bindings.ynz", OptStage::BackendOnly);
}

// ── Class 2: false ownership attributes from `declare_function` ──────────────────────

#[test]
fn red_opt_bare_param_mutation_dropped() {
    // WHY: a bare param the body mutates is declared `readonly` (codegen defaults
    // ownership None → Share and never reads typeck's effective-ownership answer); the
    // optimizer deletes the store through the readonly-claimed pointer — the mutation is
    // silently lost (O0 prints 1,42; optimized prints 1,1).
    assert_o0_and_optimized_agree(
        "v0_3_m7_p1_bare_param_mutation.ynz",
        OptStage::MidEndAndBackend,
    );
}

// ── Class 3: dangling-stack-return ABI (`ret ptr` to a callee alloca) ─────────────────
//
// Discovered at v0.3-M7 Phase 3's default flip (CCIR item 3 — a NEW O0-reliant path
// beyond the two Phase-1 classes). Plain and SM-wrapper functions returning `maybe<T>`
// (`build_maybe_none` / `%result_env_own`, emit.rs) — and the documented "stack-backed,
// copy-and-forget ABI" for `number` returns (emit.rs `ret ptr %resultN` comment) — return
// a pointer to the callee's OWN stack slot and rely on the caller copying out of the dead
// frame before reuse. That is UB the optimizer legally exploits: stores to the dying
// alloca are deleted, the caller reads garbage, and a `maybe`-terminated iterator loop
// never observes `none` (infinite-loop miscompile in examples/pirates-roster's
// InningClock demo). Manifestation is inlining-dependent, which is why the Phase 1 sweep's
// fixtures did not sample it.
//
// RESOLVED (v0.3-M7 Phase 3 Steps 4b/4c, FRAGO 005/R9): the return ABI is now BY VALUE
// for the whole class — `number` returns `i128`, `maybe<T>` returns its `{i64, i64}`
// envelope (wide shape/number payloads heap-promote flag-guarded), shapes return their LLVM struct,
// and errors-capable ok-words heap-promote maybe/shape/number success values — so
// callee-owned stack memory is never the returned storage (see `abi_return_type` in
// ynz-codegen/src/emit.rs). Un-ignored per the Phase 1 RED-set precedent; this gate is
// now a permanent regression lock.

#[test] // test-ratchet: #[ignore] removed by the fixing phase (v0.3-M7 P3 Step 4c) — the planned-RED contract this mark carried is fulfilled, not weakened
fn red_opt_dangling_stack_return_maybe_iterable() {
    // WHY: locks the dangling-stack-return class end-to-end — a user-iterable `next`
    // returning `maybe<int>` must terminate and yield 3,2,1 identically at O0 and
    // optimized. Under the current `ret ptr <alloca>` ABI the optimized run prints
    // garbage and never terminates (the watchdog trips).
    assert_o0_and_optimized_agree(
        "v0_3_m7_p3_dangling_stack_return.ynz",
        OptStage::MidEndAndBackend,
    );
}

#[test] // test-ratchet: NEW regression lock, born green with the Step-4b ABI fix — locks the sweep-confirmed sibling members of Class 3 (shape / nested-shape / number / EC ok-word returns), not just the maybe-iterable anchor the class was discovered on
fn red_opt_dangling_stack_return_sibling_sweep() {
    // WHY: locks the NON-SM (plain-return) members of the dangling-stack-return
    // class the Phase-3 Step-4b sibling sweep confirmed live (each member returned
    // garbage / SIGSEGV'd under default<O2> pre-fix while healthy at O0): plain
    // shape returns (member 1), nested-shape returns (member 2), plain number
    // returns (member 3), maybe<Shape> returns via array.get (member 4), and the
    // errors-capable ok-word carrying maybe/shape/number success values (member 4's
    // EC form). It does NOT cover the SM-tier members 5-7 (behind a `wait` —
    // `..._sm_tier` below) or the maybe<number> payload member 8
    // (`..._maybe_number_payload` below); the three tests together lock the class.
    // One differential fixture per test; O0 and optimized must agree byte-for-byte.
    assert_o0_and_optimized_agree(
        "v0_3_m7_p3_dangling_stack_return_siblings.ynz",
        OptStage::MidEndAndBackend,
    );
}

#[test] // test-ratchet: planned-RED lock authored BEFORE its fix (FRAGO 009 blocker, RED-then-green receipt) — the 8th member of Class 3
fn red_opt_dangling_stack_return_maybe_number_payload() {
    // WHY: locks the maybe<number> PAYLOAD member of the dangling-stack-return
    // class (FRAGO 009 blocker — member 8). The by-value envelope return carries
    // (flag, bits), but a number payload's bits are ptr_to_int of the callee's own
    // 16-byte i128 stack slot; `maybe_payload_stable_bits` must heap-promote it
    // (via the one authoritative `number_to_heap_cell`) or the optimized caller
    // reads zeros out of the dead frame (live repro: 3.50/10.75/99.25 at O0,
    // zeros optimized). Covers the non-SM `-> maybe<number>` return AND the
    // non-SM `-> maybe<number> errors` ok-word.
    assert_o0_and_optimized_agree(
        "v0_3_m7_p3_maybe_number_payload.ynz",
        OptStage::MidEndAndBackend,
    );
}

#[test] // test-ratchet: NEW regression lock (FRAGO 009 / test-quality SF1) — SM-tier members 5-7 of Class 3, previously unlocked while the sibling-sweep comment claimed full coverage
fn red_opt_dangling_stack_return_sm_tier() {
    // WHY: locks the SM-tier members of the dangling-stack-return class — the
    // same return shapes placed behind a `wait`, so every value crosses a
    // state-machine wrapper + resume fn instead of a plain `ret`: SM wrapper
    // `-> number` (member 5), SM `-> maybe<T>` for T ∈ {int, number} (member 6),
    // and the SM `-> maybe<T> errors` ok-word for T ∈ {int, number} (member 7).
    // A resume-stack pointer riding the 16-byte return slot or the ok word is
    // exactly the storage the optimizer legally kills; O0 and optimized must
    // agree byte-for-byte.
    assert_o0_and_optimized_agree(
        "v0_3_m7_p3_dangling_stack_return_sm_tier.ynz",
        OptStage::MidEndAndBackend,
    );
}

// test-ratchet: v0.3-M7 Phase 2 / FRAGO 002 — the anticipated resolution for this class
// (the original #[ignore] message named both options: "attribute fix or typeck rejection")
// was decided by Patrick as TYPECK REJECTION, so the differential O0-vs-optimized shape is
// structurally impossible now: the program no longer compiles. The test is reshaped (not
// weakened) to lock the rejection + its teaching diagnostic instead.
#[test]
fn red_opt_share_lend_alias_rejected_at_compile_time() {
    // WHY: the Phase-2 resolution for this class (FRAGO 002, Patrick-directed
    // 2026-07-16) is typeck REJECTION, not an attribute downgrade: a call passing the
    // same value as both `share` and `lend` violates the ownership contract (`lend` =
    // exclusive mutable access), so the miscompile this fixture originally exploited
    // (share-side read hoisted past the aliased lend-side store under false `noalias`)
    // is now unreachable — the program never compiles. This test locks the rejection
    // AND its teaching shape; if it starts compiling again, the gate is red again.
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let src = fixture("v0_3_m7_p1_share_lend_alias.ynz");
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture into tmpdir");

    let build_out = Command::new(ynz_binary())
        .args(["build", isolated_src.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        !build_out.status.success(),
        "aliasing share+lend call must be a compile error (FRAGO 002), but the build \
         succeeded — the typeck aliasing rejection has regressed"
    );
    let stderr = String::from_utf8_lossy(&build_out.stderr);
    for phrase in [
        "passed to `relay` twice in the same call",
        "`share` (a read-only view)",
        "`lend` (the function modifies it)",
        ".copy()",
    ] {
        assert!(
            stderr.contains(phrase),
            "aliasing rejection diagnostic must contain {phrase:?} (teaching shape, \
             Golden Rule 11); got:\n{stderr}"
        );
    }
}
