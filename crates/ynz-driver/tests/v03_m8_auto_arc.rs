// v0.3-M8 Phase 5 — Auto-Arc codegen emission (topology (B)), locked end-to-end by `.ynz`
// fixtures: the beneficial-emission condition admits exactly the groups the design names,
// every decline leaves the shipped copy path byte-for-byte, the refcount protocol balances
// under real concurrent load, and the single-reader program emits no `ynz_arc_*` call at all.
//
// Plan: `.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/plan.md` Phase 5
// (steps 2, 3, 5, 6). Design: `IMP-ownership.md` "Auto-Arc — Sharing Topology Across
// `background` Boundaries" (signed 2026-09-03, `audit.md`'s Phase 2 SIGN-OFF record).
//
// RED observed before the emission landed (the spike fixture, step 2): the program printed the
// correct lines, but its IR carried ZERO `ynz_arc_new` calls — two independent `ynz_alloc`
// shape copies — so `two_spawn_group_emits_one_block_two_clones_and_balances` failed on the IR
// assertion. Every decline fixture was GREEN before AND after (the point: nothing changes when
// a condition fails).
//
// Three gates per Arc fixture, none vacuous: (1) the IR count of `ynz_arc_new`/`clone`/`free`
// CALLS (declarations excluded) matches the group shape exactly; (2) the runtime alloc counter
// reports alloc == free with the Arc block genuinely counted (`arc.rs` allocates through the
// counted `ynz_alloc`, and the IR gate proves the block exists); (3) stdout is the expected
// multiset (order relaxed only where tasks print concurrently).
//
// One named test per fixture (no fixture loop — `.claude/rules/test-parallelism.md`).

use std::{
    path::PathBuf,
    process::{Command, Output},
    time::Duration,
};
use tempfile::NamedTempFile;

/// Liveness ceiling for one compiled fixture run — a trip is a hang or a deadlocked receive,
/// never a slow test. Generous by an order of magnitude over the observed run.
const RUN_WATCHDOG: Duration = Duration::from_secs(120);

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
                if start.elapsed() >= RUN_WATCHDOG {
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

/// `ynz run <fixture>` with the runtime alloc counter on → (stdout, stderr, exit code,
/// (alloc, free)). The counter env vars are read by the RUNTIME at `ynz_rt_init`, so they reach
/// the compiled program through `ynz run` unchanged.
///
/// Uses `tempfile::NamedTempFile` to ensure a unique counter file per call, preventing races
/// between parallel tests running the same fixture.
fn ynz_run_counted(name: &str) -> (String, String, i32, (u64, u64)) {
    let count_file = NamedTempFile::new().expect("failed to create temp counter file");
    let count_path = count_file
        .path()
        .to_str()
        .expect("utf-8 count path")
        .to_string();

    let mut cmd = Command::new(ynz_binary());
    cmd.args(["run", fixture(name).to_str().expect("utf-8 fixture path")])
        .env("CLICOLOR", "0")
        .env("YNZ_ALLOC_COUNTER", "1")
        .env("YNZ_ALLOC_COUNTER_OUTPUT", &count_path);
    let out = run_with_watchdog(cmd);

    let content = std::fs::read_to_string(&count_path).unwrap_or_else(|err| {
        panic!(
            "alloc counter file must exist and be readable at {}: {}",
            count_path, err
        )
    });

    let parse = |prefix: &str| -> u64 {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or_else(|| {
                panic!(
                    "counter file at {} must contain '{prefix}=N' line; got:\n{}",
                    count_path, content
                )
            })
    };
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
        (parse("alloc"), parse("free")),
    )
}

/// Build a fixture with `--emit-ir --no-optimize` (isolated in a tmpdir so parallel tests never
/// race on a shared `.ll` path) and return the IR text. `--no-optimize` pins what CODEGEN EMITS
/// — the O2 pipeline may inline or fold the call shape this file counts.
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

/// (ynz_arc_new, ynz_arc_clone, ynz_arc_free) call counts.
fn arc_call_counts(ir: &str) -> (usize, usize, usize) {
    (
        ir_call_count(ir, "ynz_arc_new"),
        ir_call_count(ir, "ynz_arc_clone"),
        ir_call_count(ir, "ynz_arc_free"),
    )
}

fn sorted_lines(s: &str) -> Vec<&str> {
    let mut v: Vec<&str> = s.lines().collect();
    v.sort_unstable();
    v
}

/// The full Arc-fixture gate: exact IR call shape, exit 0, expected stdout (as a sorted
/// multiset), and non-vacuous alloc == free. `expected_arc` is `(new, clone, free)` CALL counts
/// — `free` counts the caller-side transient releases plus, for CPU-arm spawns, the closure-body
/// releases (the SM ladder's release is in the runtime, not the IR).
fn assert_arc_fixture(name: &str, expected_arc: (usize, usize, usize), expected_stdout: &str) {
    let ir = emit_ir_no_optimize(name);
    let counts = arc_call_counts(&ir);
    assert_eq!(
        counts, expected_arc,
        "{name}: IR (ynz_arc_new, ynz_arc_clone, ynz_arc_free) call counts must match the \
         group shape exactly — an extra `new` is a copy that should have been shared, a missing \
         `clone` or `free` is a refcount imbalance"
    );
    let (stdout, stderr, code, (alloc, free)) = ynz_run_counted(name);
    assert_eq!(
        code, 0,
        "{name}: must exit 0 (a signal here is a use-after-free or double free on the shared \
         block); stderr:\n{stderr}"
    );
    assert_eq!(
        sorted_lines(&stdout),
        sorted_lines(expected_stdout),
        "{name}: stdout multiset mismatch (a wrong total is a torn read or a task reading a \
         value it should not see); stderr:\n{stderr}"
    );
    assert!(
        alloc > 0,
        "{name}: vacuous parity run — the alloc counter saw nothing"
    );
    assert!(
        expected_arc.0 > 0,
        "{name}: assert_arc_fixture is for programs that emit at least one Arc block"
    );
    assert_eq!(
        alloc, free,
        "{name}: alloc/free parity broken with {} Arc block(s) live in the program — a leaked \
         count or a skipped release",
        expected_arc.0
    );
}

/// The decline gate: NO `ynz_arc_*` call in the IR (the shipped copy path runs unchanged), exit
/// 0, expected stdout, alloc == free.
fn assert_declined_fixture(name: &str, expected_stdout: &str) {
    let ir = emit_ir_no_optimize(name);
    assert_eq!(
        arc_call_counts(&ir),
        (0, 0, 0),
        "{name}: a declined group must emit NO ynz_arc_* call — the copy path runs unchanged"
    );
    assert!(
        !ir.contains("@ynz_arc_"),
        "{name}: a program with no admitted group must not even DECLARE ynz_arc_* (the \
         declarations are lazy so the pre-emission IR is byte-identical)"
    );
    let (stdout, stderr, code, (alloc, free)) = ynz_run_counted(name);
    assert_eq!(code, 0, "{name}: must exit 0; stderr:\n{stderr}");
    assert_eq!(
        sorted_lines(&stdout),
        sorted_lines(expected_stdout),
        "{name}: stdout mismatch; stderr:\n{stderr}"
    );
    assert!(alloc > 0, "{name}: vacuous parity run");
    assert_eq!(
        alloc, free,
        "{name}: alloc/free parity broken on the copy path"
    );
}

// ── Step 2 spike fixture (promoted to the first real fixture) ──────────────────────────────

#[test]
fn two_spawn_group_emits_one_block_two_clones_and_balances() {
    // WHY: the spike verdict, kept as a permanent gate. Two spawns of the same read-only shape
    // → ONE `ynz_arc_new`, TWO `ynz_arc_clone` (one per task), ONE `ynz_arc_free` in the IR
    // (the caller's transient, released right after the second spawn; the tasks' two releases
    // run in the runtime's drop ladder). RED pre-emission: (0, 0, 0).
    assert_arc_fixture(
        "m8_arc_two_spawn_group.ynz",
        (1, 2, 1),
        "tasks saw 84\ncaller keeps Three Rivers 6x7\n",
    );
}

#[test]
fn two_spawn_group_shares_exactly_one_allocation_fewer_than_the_copy_path() {
    // WHY: the non-vacuous half of the parity gate, stated as a DIFFERENCE the copy path
    // cannot produce: the two-spawn group allocates exactly ONE more counted block than the
    // one-spawn program plus one more task frame and one more descriptor array — i.e. the
    // second task's shape copy is GONE. Under the copy path the delta would be +3 (frame,
    // descriptors, copy); with the shared block it is +2 (frame, descriptors) — the group's one
    // Arc block replaces BOTH per-task copies, at a net cost of zero extra allocations.
    let (_, stderr_one, code_one, (alloc_one, free_one)) =
        ynz_run_counted("m8_arc_one_spawn_noop.ynz");
    assert_eq!(code_one, 0, "one-spawn must exit 0; stderr:\n{stderr_one}");
    assert_eq!(alloc_one, free_one, "one-spawn parity");
    let (_, stderr_two, code_two, (alloc_two, free_two)) =
        ynz_run_counted("m8_arc_two_spawn_group.ynz");
    assert_eq!(code_two, 0, "two-spawn must exit 0; stderr:\n{stderr_two}");
    assert_eq!(alloc_two, free_two, "two-spawn parity");
    assert_eq!(
        alloc_two,
        alloc_one + 2,
        "two-spawn group must allocate exactly one task frame + one descriptor array more than \
         one-spawn (the Arc block replaced both per-task copies); observed one={alloc_one} \
         two={alloc_two} — a +3 means the second task still took its own copy"
    );
}

// ── Step 3: the shipped copy path is UNCHANGED for the single-reader case ───────────────────

#[test]
fn one_spawn_emits_no_arc_symbol_at_all() {
    // WHY: "caller + 1 task" is NOT a sharing case under topology (B). The pre-emission IR
    // for this fixture was captured and sha256-compared against the post-emission IR at
    // introduction (identical: 4a812358…); this test pins the checkable half of that proof —
    // no `ynz_arc_*` call AND no `ynz_arc_*` declaration (declared lazily on first use).
    assert_declined_fixture(
        "m8_arc_one_spawn_noop.ynz",
        "task saw 42\ncaller keeps Three Rivers 6x7\n",
    );
}

// ── Step 3: every decline in the beneficial-emission condition ──────────────────────────────

#[test]
fn caller_side_write_between_spawns_declines_and_second_task_sees_the_update() {
    // WHY: condition 3 — `scene.width = 10` between the spawns is `Writes` in
    // `classify_binding_in_stmts`; each task copies the CURRENT value (42 + 70).
    assert_declined_fixture(
        "m8_arc_write_between_declines.ynz",
        "tasks total 112\ncaller keeps Stargell 10x7\n",
    );
}

#[test]
fn suspension_between_spawns_declines() {
    // WHY: condition 1 — a `wait` between the spawns would force the transient across a
    // frame boundary (the registry's residual); the group declines to the copy path.
    assert_declined_fixture(
        "m8_arc_suspend_between_declines.ynz",
        "tasks total 84\ncaller keeps Mazeroski 6x7\n",
    );
}

#[test]
fn early_return_between_spawns_declines() {
    // WHY: the implementation's one boundary beyond the signed text — an exit between the
    // spawns would skip the transient's straight-line release (a leaked count).
    assert_declined_fixture(
        "m8_arc_return_between_declines.ynz",
        "tasks total 84\ncaller keeps Bonds 6x7\n",
    );
}

#[test]
fn top_level_rebinding_between_spawns_is_a_group_boundary_not_a_decline() {
    // WHY: condition 3's boundary rule — spawns 1–2 share value A, spawns 3–4 share value B:
    // TWO groups (two `new`, four `clone`, two transient `free`), and the totals prove each
    // pair read its own value (2·42 + 2·100).
    assert_arc_fixture(
        "m8_arc_rebind_boundary.ynz",
        (2, 4, 2),
        "tasks total 284\ncaller keeps Kiner 10x10\n",
    );
}

#[test]
fn explicit_copy_at_one_spawn_opts_that_spawn_out_of_the_group() {
    // WHY: the override direction — `.copy()` is `Fresh`, not a whole-binding member; the
    // other two spawns still form a group of two (1 new, 2 clones, 1 transient free) and the
    // copied spawn takes the per-task copy path (3 × 42).
    assert_arc_fixture(
        "m8_arc_explicit_copy_opts_out.ynz",
        (1, 2, 1),
        "tasks total 126\ncaller keeps Beaumont 6x7\n",
    );
}

// ── Both spawn forms, both runtime arms ────────────────────────────────────────────────────

#[test]
fn handle_form_spawns_form_a_group_too() {
    // WHY: `let h = background f(v)` records through the SAME shared recording function as
    // the bare statement form (parked item 16's fix); two handle spawns share one block.
    assert_arc_fixture(
        "m8_arc_handle_form_group.ynz",
        (1, 2, 1),
        "handles total 84\ncaller keeps Clemente 6x7\n",
    );
}

#[test]
fn cpu_arm_group_releases_through_the_closure_body_free() {
    // WHY: non-suspending callees route to `ynz_rt_spawn_blocking`; the task's reference is
    // released by the closure-body `emit_bg_arg_frees` ArcShape arm, so the IR carries THREE
    // task releases + ONE transient release (4 `free`), against 1 `new` and 3 `clone`.
    assert_arc_fixture(
        "m8_arc_cpu_arm_group.ynz",
        (1, 3, 4),
        "cpu task saw 42\ncpu task saw 42\ncpu task saw 42\ncaller keeps Smithfield 6x7\n",
    );
}

// ── Step 5: the end-to-end hammer ──────────────────────────────────────────────────────────

#[test]
fn hammer_four_tasks_read_one_shared_block_under_concurrent_load() {
    // WHY: plan step 5 — the substrate hammer in `arc.rs` proves clone/free counting on one
    // thread pool; THIS proves the codegen-emitted call sites end-to-end: four suspending
    // tasks interleave 25 iterations each of reading every field of ONE shared block on the
    // I/O pool while the caller keeps reading its original, then all four references retire
    // through the ladder. 4 × 25 × 30·35 = 105000 — a torn read or a freed block changes it;
    // alloc == free proves the last release freed the block exactly once.
    assert_arc_fixture(
        "m8_arc_hammer_shared_shape.ynz",
        (1, 4, 1),
        "tasks total 105000\ncaller keeps Fort Pitt 30x35 depth 2\n",
    );
}
