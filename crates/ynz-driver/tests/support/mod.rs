//! Shared harness for tests that build ONE fixture and then observe the compiled program:
//! its heap alloc/free counters, or its behavior under AddressSanitizer.
//!
//! Each call builds into its own temp directory and runs its own process, so concurrent tests
//! never share a counter, a binary or a file (`.claude/rules/test-parallelism.md`).
//!
//! ## AddressSanitizer on Yinz programs
//!
//! `ynz build --emit-ir` writes the LLVM IR the object was lowered from. [`build_asan`] adds the
//! `sanitize_address` attribute to every function the compiler defined, then lets
//! `clang -fsanitize=address` compile that IR and link it with the bundled runtime archive.
//! Result: the generated code's loads/stores are instrumented (a use-after-free of a Yinz heap
//! value is caught at the access), and every `malloc`/`free` the runtime makes goes through
//! ASan (double free, invalid free). The runtime's own Rust code is not instrumented.
//!
//! Requires `clang` (`clang-18` first, as the driver's linker search does) WITH the ASan runtime
//! (`libclang-rt-18-dev` on Ubuntu/Debian). A missing runtime fails the test with the linker's
//! message; it never skips.

#![allow(dead_code)] // each test binary uses a different subset of this shared module

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Liveness ceiling for one compiled fixture run. A trip is a hang, never a slow test:
/// generous by an order of magnitude over the observed run so a loaded parallel lane cannot
/// false-trip it.
const RUN_WATCHDOG: Duration = Duration::from_secs(120);

/// `ASAN_OPTIONS` for the Yinz fixtures:
/// - `detect_leaks=0`: the safety fixtures guard against freeing early or twice. They pass
///   today BECAUSE nothing is freed, so leak detection would be red until the release pass
///   lands; leaks are asserted by the counter tests instead.
/// - `free_fill_byte=0` / `max_free_fill_size`: freed memory reads back as zero, so a read of
///   a freed header by the (uninstrumented) runtime shows up as a wrong count in the output
///   rather than going unnoticed.
const ASAN_OPTIONS: &str = "detect_leaks=0:max_free_fill_size=65536:free_fill_byte=0";

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// What a compiled fixture printed and how it exited.
pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    /// Exit code; `-1` when the process was killed by a signal.
    pub exit_code: i32,
}

/// The `YNZ_ALLOC_COUNTER_OUTPUT` file of a finished run.
#[derive(Debug)]
pub struct Counters {
    pub alloc: u64,
    pub free: u64,
    pub string_alloc: u64,
    pub string_free: u64,
    pub string_alloc_bytes: u64,
    pub string_free_bytes: u64,
}

/// A fixture built into a private temp directory. The directory is removed on drop.
pub struct Built {
    _dir: tempfile::TempDir,
    pub binary: PathBuf,
    pub ir: PathBuf,
}

/// `ynz build --emit-ir [--no-optimize] <copy of fixture>` into a fresh temp directory.
/// Panics with the compiler's output when the fixture does not compile (a fixture that fails
/// to compile is a test bug, never a skip).
pub fn build(fixture_name: &str, no_optimize: bool) -> Built {
    let dir = tempfile::tempdir().expect("create temp dir");
    let source = dir.path().join("program.ynz");
    std::fs::copy(fixture_path(fixture_name), &source)
        .unwrap_or_else(|err| panic!("copy fixture {fixture_name}: {err}"));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ynz"));
    cmd.arg("build").arg(&source).arg("--emit-ir");
    if no_optimize {
        cmd.arg("--no-optimize");
    }
    let output = cmd
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    assert!(
        output.status.success(),
        "{fixture_name}: `ynz build` failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Built {
        binary: dir.path().join("program"),
        ir: dir.path().join("program.ll"),
        _dir: dir,
    }
}

/// Run `binary` to completion under the liveness watchdog.
fn run_with_watchdog(mut cmd: Command) -> RunOutput {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn compiled fixture");
    let pid = child.id();
    let (sender, receiver) = mpsc::channel::<std::io::Result<Output>>();
    // `wait_with_output` drains both pipes while waiting, so a chatty child cannot deadlock.
    std::thread::spawn(move || {
        let _ = sender.send(child.wait_with_output());
    });
    match receiver.recv_timeout(RUN_WATCHDOG) {
        Ok(result) => {
            let output = result.expect("collect compiled fixture output");
            RunOutput {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_code: output.status.code().unwrap_or(-1),
            }
        }
        Err(_) => {
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
            panic!(
                "WATCHDOG TRIP: compiled fixture did not exit within {RUN_WATCHDOG:?} — a hang, \
                 never a slow test; fix the program, never widen this timeout."
            );
        }
    }
}

/// Run a built fixture with the runtime's alloc counters on and return what it printed plus the
/// counters. Panics when the counter file is missing or a key is absent: a silent `0` would turn
/// "the counter did not report" into "the program balanced".
pub fn run_counted(built: &Built) -> (RunOutput, Counters) {
    let counter_file = built.binary.with_extension("counters");
    let mut cmd = Command::new(&built.binary);
    cmd.env("YNZ_ALLOC_COUNTER", "1")
        .env("YNZ_ALLOC_COUNTER_OUTPUT", &counter_file);
    let output = run_with_watchdog(cmd);
    let text = std::fs::read_to_string(&counter_file).unwrap_or_else(|err| {
        panic!(
            "counter file {} not written ({err}); exit {}, stderr:\n{}",
            counter_file.display(),
            output.exit_code,
            output.stderr
        )
    });
    let counters = Counters {
        alloc: counter_value(&text, "alloc"),
        free: counter_value(&text, "free"),
        string_alloc: counter_value(&text, "string_alloc"),
        string_free: counter_value(&text, "string_free"),
        string_alloc_bytes: counter_value(&text, "string_alloc_bytes"),
        string_free_bytes: counter_value(&text, "string_free_bytes"),
    };
    (output, counters)
}

fn counter_value(file_text: &str, key: &str) -> u64 {
    let prefix = format!("{key}=");
    file_text
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("counter key `{key}` missing from:\n{file_text}"))
        .trim()
        .parse()
        .unwrap_or_else(|err| {
            panic!("counter key `{key}` is not a number ({err}) in:\n{file_text}")
        })
}

/// Add `sanitize_address` to every function definition in `ir`. O(n) time, O(n) memory.
/// Panics when no definition is found: an uninstrumented binary would pass every check.
fn instrument_for_asan(ir: &str) -> String {
    let mut instrumented_definitions = 0;
    let mut out = String::with_capacity(ir.len() + 4096);
    for line in ir.lines() {
        match line.strip_suffix(" {") {
            Some(header) if line.starts_with("define ") => {
                out.push_str(header);
                out.push_str(" sanitize_address {");
                instrumented_definitions += 1;
            }
            _ => out.push_str(line),
        }
        out.push('\n');
    }
    assert!(
        instrumented_definitions > 0,
        "no `define ... {{` line found in the emitted IR; cannot instrument for ASan"
    );
    out
}

/// First of the driver's linker candidates that runs.
fn find_clang() -> &'static str {
    ["clang-18", "clang"]
        .into_iter()
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        })
        .expect("ASan fixtures need `clang-18` or `clang` on PATH (with libclang-rt-18-dev)")
}

/// Instrument `ir` (LLVM IR text) for ASan, compile it with `clang -fsanitize=address` and link
/// it against the bundled runtime archive. The files are written into `dir`; returns the path
/// of the sanitized binary.
pub fn build_asan_from_ir(dir: &Path, ir: &str) -> PathBuf {
    let sanitized_ir = dir.join("program.asan.ll");
    std::fs::write(&sanitized_ir, instrument_for_asan(ir)).expect("write instrumented IR");
    let sanitized_binary = dir.join("program.asan");
    let output = Command::new(find_clang())
        .args(["-O0", "-g", "-fsanitize=address", "-no-pie"])
        .arg(&sanitized_ir)
        .arg(env!("YNZ_RT_LIB_PATH"))
        // The same system libraries the driver's own link line adds for the Tokio runtime.
        .args(["-lpthread", "-ldl", "-lm", "-lrt"])
        .arg("-o")
        .arg(&sanitized_binary)
        .output()
        .expect("spawn clang");
    assert!(
        output.status.success(),
        "clang -fsanitize=address failed (is libclang-rt-18-dev installed?):\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    sanitized_binary
}

/// Compile `built`'s emitted IR with the ASan instrumentation pass and link it against the
/// bundled runtime archive. Returns the path of the sanitized binary.
pub fn build_asan(built: &Built) -> PathBuf {
    let ir = std::fs::read_to_string(&built.ir).expect("read emitted IR");
    let dir = built
        .ir
        .parent()
        .expect("emitted IR has a parent directory");
    build_asan_from_ir(dir, &ir)
}

/// Run an ASan-built binary under the fixtures' `ASAN_OPTIONS`, with the liveness watchdog.
pub fn run_asan_binary(sanitized_binary: &Path) -> RunOutput {
    let mut cmd = Command::new(sanitized_binary);
    cmd.env("ASAN_OPTIONS", ASAN_OPTIONS);
    run_with_watchdog(cmd)
}

/// Build `fixture_name`, instrument it, run it under AddressSanitizer.
pub fn run_under_asan(fixture_name: &str, no_optimize: bool) -> RunOutput {
    let built = build(fixture_name, no_optimize);
    run_asan_binary(&build_asan(&built))
}

/// Build `fixture_name` with `ynz build` and run the binary (no sanitizer, no counters) under
/// the liveness watchdog. The binary is the watchdog's direct child, so a hang is killed with
/// the program itself rather than leaving it orphaned behind a `ynz run` parent.
pub fn run_plain(fixture_name: &str, no_optimize: bool) -> RunOutput {
    let built = build(fixture_name, no_optimize);
    run_with_watchdog(Command::new(&built.binary))
}

/// The shared check of the output-assertion suites: `fixture_name` exits 0 and prints exactly
/// `expected_stdout`. `what_it_guards` completes the failure message.
pub fn assert_fixture_prints(
    fixture_name: &str,
    no_optimize: bool,
    expected_stdout: &str,
    what_it_guards: &str,
) {
    let run = run_plain(fixture_name, no_optimize);
    assert_eq!(
        run.exit_code, 0,
        "{fixture_name} (no_optimize={no_optimize}) must run clean; stderr:\n{}",
        run.stderr
    );
    assert_eq!(
        run.stdout, expected_stdout,
        "{fixture_name} (no_optimize={no_optimize}): {what_it_guards}; stderr:\n{}",
        run.stderr
    );
}
