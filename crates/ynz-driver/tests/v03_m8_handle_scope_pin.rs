// v0.3-M8 Phase 7 — Track 3 (source-level scope-drop cancellation) took Branch B: RE-DEFER.
//
// These two tests PIN the current state the deferral describes, so the deferral is checkable
// (`no-duct-tape.md`: a deferral names what observably changes when it lifts). They are the
// planned-RED inverse: when the drop-story milestone lands its scope-exit release pass and the
// handle arm calls `ynz_handle_free` at a handle binding's scope exit, BOTH flip — the IR gains
// a `ynz_handle_free` call site and the child stops printing `child: done` — and whoever lands
// that pass rewrites them into the Branch A fixtures (a handle dropped at scope exit cancels the
// child at its next suspension; alloc == free; the crossing-local and non-suspending-parent
// shapes) rather than deleting them.
//
// Plan: `.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/plan.md` Phase 7
// (Branch B), Future Requirements #3. Evidence record: that plan's `audit.md` entry
// `m8-p7-20260904-a1` (ten probe programs, alloc counter armed, IR read). Design:
// `IMP-no-function-coloring.md` "Task Cancellation" (current-state paragraph); registry
// `background-handle-cancel-injection`.
//
// Why these are NOT vacuous: the IR test also asserts exactly one `ynz_rt_spawn_handle` call
// (the handle form genuinely compiled), and the semantic test asserts the parent's three lines
// AND the child's two lines — a cancelled child would lose `child: done`.
//
// One named test per shape, no fixture loop (`.claude/rules/test-parallelism.md`).

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};

/// Liveness ceiling for one compiled fixture run — a trip is a hang, never a slow test.
/// Generous by an order of magnitude over the observed run (~0.3s of sleeps).
const RUN_WATCHDOG: Duration = Duration::from_secs(120);

const FIXTURE: &str = "v0_3_m8_p7_handle_scope_exit_pin.ynz";

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
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
            Some(_) => {
                return child
                    .wait_with_output()
                    .expect("watchdog: failed to collect child output");
            }
            None => {
                if start.elapsed() > RUN_WATCHDOG {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "WATCHDOG TRIP: fixture did not exit within {RUN_WATCHDOG:?} — a hang, \
                         never a slow test; fix the runtime, never widen this timeout."
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// `ynz run <fixture>` → (stdout, stderr, exit code).
fn ynz_run(name: &str) -> (String, String, i32) {
    let mut cmd = Command::new(ynz_binary());
    cmd.args(["run", fixture(name).to_str().expect("utf-8 fixture path")])
        .env("CLICOLOR", "0");
    let out = run_with_watchdog(cmd);
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

/// Build a fixture with `--emit-ir --no-optimize` (isolated in a tmpdir so parallel tests never
/// race on a shared `.ll` path) and return the IR text. `--no-optimize` pins what CODEGEN EMITS.
fn emit_ir_no_optimize(name: &str) -> String {
    let src = fixture(name);
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture to tmpdir");
    let build_out = Command::new(ynz_binary())
        .args([
            "build",
            "--no-optimize",
            isolated_src.to_str().unwrap(),
            "--emit-ir",
        ])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz build");
    assert!(
        build_out.status.success(),
        "fixture `{name}` must build clean; stderr:\n{}",
        String::from_utf8_lossy(&build_out.stderr)
    );
    std::fs::read_to_string(isolated_src.with_extension("ll"))
        .expect("emitted .ll must be readable")
}

/// Count CALLS of a runtime symbol in IR text — `call ... @sym(` — never its `declare` line.
fn ir_call_count(ir: &str, sym: &str) -> usize {
    let needle = format!("@{sym}(");
    ir.lines()
        .filter(|l| l.contains("call ") && l.contains(&needle))
        .count()
}

fn sorted_lines(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.lines().collect();
    v.sort_unstable();
    v
}

/// PIN (Branch B current state): a handle binding leaving its block does NOT cancel the child.
/// The child prints both of its post-suspension lines while the parent is still alive. The
/// parent's lines and the child's lines interleave on the multi-thread scheduler, so the
/// assertion is on the sorted multiset (the auto-Arc suite's convention), with `child: done`
/// additionally asserted by name — it is the line a scope-exit cancel would remove.
#[test]
fn handle_leaving_its_block_does_not_cancel_the_child_today() {
    let (stdout, stderr, code) = ynz_run(FIXTURE);
    assert_eq!(code, 0, "{FIXTURE}: must exit 0; stderr:\n{stderr}");
    assert!(
        stdout.contains("child: done\n"),
        "{FIXTURE}: the child must run to completion past the handle's scope exit — if this \
         line is missing, a scope-exit release now cancels the child: the drop pass has landed, \
         so rewrite this pin into the Branch A fixture (plan Future Requirement #3); stdout:\n{stdout}"
    );
    assert_eq!(
        sorted_lines(&stdout),
        sorted_lines(
            "parent: leaving block\nparent: block exited\nchild: after first sleep\n\
             child: done\nparent: done\n"
        ),
        "{FIXTURE}: exact line multiset; stderr:\n{stderr}"
    );
}

/// PIN (Branch B current state): codegen emits NO `ynz_handle_free` call for a handle binding
/// whose scope ends — the language half of cancel-via-drop is not wired. Non-vacuous: the same
/// IR carries exactly one `ynz_rt_spawn_handle` call, so the handle form genuinely compiled.
#[test]
fn no_handle_free_is_emitted_at_a_handle_bindings_scope_exit_today() {
    let ir = emit_ir_no_optimize(FIXTURE);
    assert_eq!(
        ir_call_count(&ir, "ynz_rt_spawn_handle"),
        1,
        "{FIXTURE}: the handle-form spawn must compile to exactly one ynz_rt_spawn_handle call"
    );
    assert_eq!(
        ir_call_count(&ir, "ynz_handle_free"),
        0,
        "{FIXTURE}: a ynz_handle_free call site appeared — the scope-exit release pass has \
         landed for handles; flip this pin into the Branch A IR gate (one call per handle \
         binding scope exit) and retire the `background-handle-cancel-injection` registry entry"
    );
}
