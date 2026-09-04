//! Hotfix `bgarg-number` (parked item 40 / M8 FRAGO 012): a shape with a `number`
//! (decimal128) field passed to a `background` spawn — RED→GREEN pin.
//!
//! Authored BEFORE the fix (RED-repro-before-fix). Pre-fix signature on released
//! v0.3.3 / `main` at ec014d8 (default optimized build): the compiled program dies with
//! signal 11 (SIGSEGV) — `ynz run` reports the signal, no stdout. The identical
//! program without the `number` field runs clean, `YNZ_NO_OPTIMIZE=1` runs clean, and a
//! `number` PARAMETER (not a field) already crosses the same spawn boundary correctly
//! (v03_m6_number_spawn_boundary.rs).
//!
//! Root cause (IR + asm evidence in the commit, `git log --grep=bgarg-number`): the
//! caller keeps reading `scene` after the spawn, so `scene` is a crossing local
//! frame-EMBEDDED in the spawner's state-machine frame at an 8-byte slot offset
//! (`base + 40`). Its LLVM struct carries an i128 field, so the struct's ABI alignment
//! is 16, and every access through the region pointer — the spawn-site whole-struct
//! copy (`bg_shape_src` load) and the caller's own `scene.scale` field read — is
//! emitted at `align 16`. Optimized X86 ISel honors that claim with `movaps`; on an
//! address that is 8 mod 16 it faults. The fix lives at the one producer of the region
//! pointer (`state_machine::shape_frame_region_ptr`): the region is rounded up to
//! `FRAME_SHAPE_REGION_ALIGN` and the layout reserves the slack for it.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The RED pin fixture (direct `number` field, channel spawn form, inferred copy).
const FIXTURE: &str = "bg_arg_shape_number_field.ynz";
/// Sweep pin: the handle spawn form (`let h = background f(scene)`).
const FIXTURE_HANDLE: &str = "bg_arg_shape_number_field_handle.ynz";
/// Sweep pin: the `number` field one level down, inside a nested shape.
const FIXTURE_NESTED: &str = "bg_arg_shape_number_field_nested.ynz";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Liveness budget, not a performance assertion (test-parallelism.md): an order of
/// magnitude over the observed run so a contended nextest lane never flips it.
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

/// Run `ynz run <fixture>` with a kill-after-timeout guard.
/// Returns (stdout, stderr, exit_code, timed_out). exit_code is -1 when killed/signalled.
fn ynz_run_with_timeout(
    name: &str,
    timeout: Duration,
    extra_env: &[(&str, &str)],
) -> (String, String, i32, bool) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ynz"));
    cmd.args(["run", fixture(name).to_str().unwrap()])
        .env("CLICOLOR", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("failed to spawn ynz binary");

    let start = Instant::now();
    let mut timed_out = false;
    loop {
        match child.try_wait().expect("try_wait failed") {
            Some(_) => break,
            None if start.elapsed() > timeout => {
                child.kill().ok();
                child.wait().ok();
                timed_out = true;
                break;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut stdout).ok();
    }
    if let Some(mut s) = child.stderr.take() {
        s.read_to_string(&mut stderr).ok();
    }
    let code = if timed_out {
        -1
    } else {
        child
            .try_wait()
            .ok()
            .flatten()
            .and_then(|s| s.code())
            .unwrap_or(-1)
    };
    (stdout, stderr, code, timed_out)
}

/// Shared assertion: the fixture runs clean and prints exactly `expected` (order is
/// deterministic in every fixture here — the caller blocks on the task's send/return
/// before its own print).
fn assert_runs_clean_with(name: &str, expected: &str, what: &str) {
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(name, RUN_TIMEOUT, &[]);
    assert!(
        !timed_out,
        "{what} fixture must not hang; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        code, 0,
        "{what} fixture must compile and run clean — pre-fix it dies with SIGSEGV \
         (exit reported as -1); stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout, expected,
        "{what}: every field must reach the task and the caller intact; stderr:\n{stderr}"
    );
}

const EXPECTED_STDOUT: &str = "Point 800 600 1.5\n1 1.5\n";

#[test]
fn bg_arg_shape_with_number_field_survives_spawn() {
    // WHY: the RED pin. `background render(scene, results)` where `Scene` carries a
    // `number` field, `render` suspends (SM spawn arm), and the caller reads `scene`
    // after the spawn (inferred copy → `scene` frame-embedded in the spawner's frame).
    // Both faulting sites are exercised: the spawn-site whole-struct copy and the
    // caller's post-resume `scene.scale` read. Pre-fix: signal 11, no stdout.
    assert_runs_clean_with(FIXTURE, EXPECTED_STDOUT, "shape-with-number-field bg");
}

#[test]
fn bg_arg_shape_with_number_field_is_deterministic_across_runs() {
    // WHY: the non-vacuous proof — a fix that merely dodged the fault by luck (a frame
    // base that happened to land the region on a 16-byte boundary, as the give-path
    // twin of this program does) could pass one run. N=5 fresh processes must all
    // print the identical staged values.
    for run in 0..5 {
        let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(FIXTURE, RUN_TIMEOUT, &[]);
        assert!(!timed_out, "run {run}: must not hang; stderr:\n{stderr}");
        assert_eq!(code, 0, "run {run}: must exit clean; stderr:\n{stderr}");
        assert_eq!(
            stdout, EXPECTED_STDOUT,
            "run {run}: every run must print the same staged values; stderr:\n{stderr}"
        );
    }
}

#[test]
fn bg_arg_shape_with_number_field_handle_form_survives_spawn() {
    // WHY: sweep pin (hotfix step 4) — the handle spawn form routes the same shape
    // through the same `prepare_bg_arg_for_ctx` copy from the same misaligned frame
    // region; pre-fix it faulted identically (probe-confirmed).
    assert_runs_clean_with(
        FIXTURE_HANDLE,
        "task sees 1.5\nhandle saw 6 Three Rivers\n",
        "handle-form shape-with-number-field bg",
    );
}

#[test]
fn bg_arg_shape_with_number_field_nested_survives_spawn() {
    // WHY: sweep pin (hotfix step 4) — a `number` field one level down, give path.
    // The copy-path twin is compile-gated (nested-shape crossing locals are refused
    // with a teaching error), so no frame-embedded nested region can exist; nested
    // shape fields are pointer-stored heap cells, so the i128 never lands inline in
    // the outer struct either. Ran clean pre-fix; locked so the nested form stays
    // covered next to the direct-field fix.
    assert_runs_clean_with(
        FIXTURE_NESTED,
        "task sees 2.5\ntasks saw 1\n",
        "nested shape-with-number-field bg",
    );
}

#[test]
fn bg_arg_shape_with_number_field_alloc_free_parity() {
    // WHY: the shape heap copy (`ynz_alloc` at the spawn site) must be freed exactly
    // once by the task's drop ladder (`BgArgFreeKind::HeapShape { byte_size }`) — the
    // fix changed where the SOURCE bytes live (frame slack), not the copy's size or
    // representation, and alloc == free must hold.
    let count_file = std::env::temp_dir().join(format!(
        "ynz_alloc_hotfix_bg_arg_number_field_{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&count_file);
    let (stdout, stderr, code, timed_out) = ynz_run_with_timeout(
        FIXTURE,
        RUN_TIMEOUT,
        &[
            ("YNZ_ALLOC_COUNTER", "1"),
            (
                "YNZ_ALLOC_COUNTER_OUTPUT",
                count_file.to_str().expect("valid path"),
            ),
        ],
    );
    let content = std::fs::read_to_string(&count_file).unwrap_or_default();
    let _ = std::fs::remove_file(&count_file);
    assert!(!timed_out, "must not hang; stderr:\n{stderr}");
    assert_eq!(
        code, 0,
        "must exit clean; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let parse_count = |prefix: &str| -> Option<u64> {
        content
            .lines()
            .find(|l| l.starts_with(prefix))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
    };
    let alloc = parse_count("alloc").expect("alloc counter line present");
    let free = parse_count("free").expect("free counter line present");
    assert!(
        alloc > 0,
        "the spawn-site shape heap copy must be counted (alloc=0 means the counter \
         never observed the copy); counter file:\n{content}"
    );
    assert_eq!(
        alloc, free,
        "every spawn-site allocation must be freed exactly once (alloc={alloc}, \
         free={free}); counter file:\n{content}"
    );
}

/// Build a fixture with `--emit-ir --no-optimize` (isolated in a tempdir so parallel
/// tests never race on a shared `.ll` path) and return the emitted IR text. The
/// codegen-emission anchor tier is pinned deliberately: the marker below is a claim
/// about what codegen EMITS, and `default<O2>` legally renames or folds local value
/// names (same move as v03_m6_number_spawn_boundary.rs).
fn emit_fixture_ir(name: &str) -> String {
    let src = fixture(name);
    let tmp = tempfile::TempDir::new().expect("failed to create tmpdir");
    let isolated_src = tmp.path().join(src.file_name().expect("fixture filename"));
    std::fs::copy(&src, &isolated_src).expect("failed to copy fixture to tmpdir");
    let build_out = Command::new(env!("CARGO_BIN_EXE_ynz"))
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

#[test]
fn bg_arg_shape_with_number_field_frame_region_is_aligned_in_ir() {
    // WHY: pins the MECHANISM, not just the symptom. The spawner's resume fn must wire
    // the frame-embedded `scene` through the one aligning producer
    // (`shape_frame_region_ptr` → `<name>_frame_region_aligned`), and the spawn-site
    // whole-struct copy must still read from THAT pointer (`bg_shape_src`). A future
    // edit that re-derives the region from the slot index at any consumer, or drops
    // the rounding, breaks this loudly while the runtime pin above might pass by a
    // lucky frame base.
    let ir = emit_fixture_ir(FIXTURE);
    assert!(
        ir.contains("%scene_frame_region_aligned"),
        "the spawner's frame-embedded `scene` region must be wired through the aligning \
         producer (`scene_frame_region_aligned`); IR:\n{ir}"
    );
    assert!(
        ir.contains("%scene_frame_region_mask = and i64"),
        "the region pointer must be rounded with the FRAME_SHAPE_REGION_ALIGN mask; IR:\n{ir}"
    );
    assert!(
        ir.contains("%bg_shape_src = load"),
        "the spawn-site whole-struct copy (`bg_shape_src`) must still be emitted — this \
         fixture must keep exercising the inferred-copy path; IR:\n{ir}"
    );
}
