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

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Liveness ceiling for one compiled fixture run. A trip is a hang, never a slow test:
/// generous by an order of magnitude over the observed run so a loaded parallel lane cannot
/// false-trip it.
const RUN_WATCHDOG: Duration = Duration::from_secs(120);

/// The runtime's exit drain budget (`YNZ_SHUTDOWN_TIMEOUT_MS`) for the `*_until_tasks_finish`
/// runs: below [`RUN_WATCHDOG`], so a task that never finishes shows as a missing task in the
/// assertions before the watchdog calls it a hang.
const TASK_DRAIN_BUDGET_MS: &str = "100000";

/// `ASAN_OPTIONS` for the Yinz fixtures:
/// - `detect_leaks=0`: the safety fixtures guard against freeing early or twice. They pass
///   today BECAUSE nothing is freed, so leak detection would be red until the release pass
///   lands; leaks are asserted by the counter tests instead.
/// - `free_fill_byte=0` / `max_free_fill_size`: freed memory reads back as zero, so a read of
///   a freed header by the (uninstrumented) runtime shows up as a wrong count in the output
///   rather than going unnoticed.
const ASAN_OPTIONS: &str = "detect_leaks=0:max_free_fill_size=65536:free_fill_byte=0";

/// What `ynz build` said about one fixture: whether it compiled, and every diagnostic it printed.
pub struct Compiled {
    pub success: bool,
    pub diagnostics: String,
}

/// Build `fixture_name` with `ynz build` in its own temp directory and keep the compiler's
/// verdict and text. For the suites that assert a program is REFUSED with a teaching diagnostic
/// (no program is run). `CLICOLOR=0` keeps the text free of escape codes. A project fixture (a
/// directory) is built as a project, as in [`build`].
pub fn compile(fixture_name: &str) -> Compiled {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = stage_fixture(directory.path(), fixture_name);
    let output = Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(&source)
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    Compiled {
        success: output.status.success(),
        diagnostics: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// The shared check of the refusal suites: `fixture_name` must NOT compile, the failure must be the
/// program's (typeck's teaching diagnostic) and not the compiler's own (`compiler bug`,
/// `internal error`, `codegen:`), and the diagnostic must hold every phrase (one per distinct
/// rejection the fixture holds).
pub fn assert_refused_with(fixture_name: &str, phrases: &[&str]) {
    let compiled = compile(fixture_name);
    assert!(
        !compiled.success,
        "{fixture_name}: must be refused, but compiled:\n{}",
        compiled.diagnostics
    );
    for internal in ["codegen:", "internal error", "compiler bug"] {
        assert!(
            !compiled.diagnostics.contains(internal),
            "{fixture_name}: a user error must come from typeck, not the compiler ({internal:?}):\n{}",
            compiled.diagnostics
        );
    }
    for phrase in phrases {
        assert!(
            compiled.diagnostics.contains(phrase),
            "{fixture_name}: diagnostic must contain {phrase:?}; got:\n{}",
            compiled.diagnostics
        );
    }
}

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
    _directory: tempfile::TempDir,
    pub binary: PathBuf,
    pub ir: PathBuf,
}

/// Copy `fixture_name` into `directory` and return what `ynz build` takes: `program.ynz` for a
/// single file, or the `project` directory (copied whole) for a multi-module project.
fn stage_fixture(directory: &Path, fixture_name: &str) -> PathBuf {
    let fixture = fixture_path(fixture_name);
    if !fixture.is_dir() {
        let source = directory.join("program.ynz");
        std::fs::copy(&fixture, &source)
            .unwrap_or_else(|err| panic!("copy fixture {fixture_name}: {err}"));
        return source;
    }
    let project = directory.join("project");
    std::fs::create_dir(&project).expect("create project directory");
    for entry in std::fs::read_dir(&fixture)
        .unwrap_or_else(|err| panic!("read fixture project {fixture_name}: {err}"))
    {
        let entry = entry.expect("read fixture project entry");
        std::fs::copy(entry.path(), project.join(entry.file_name()))
            .unwrap_or_else(|err| panic!("copy fixture project {fixture_name}: {err}"));
    }
    project
}

/// `ynz build --emit-ir [--no-optimize] <copy of fixture>` into a fresh temp directory.
/// Panics with the compiler's output when the fixture does not compile (a fixture that fails
/// to compile is a test bug, never a skip).
///
/// A `fixture_name` that names a DIRECTORY is a multi-module project (a `yinz.toml` and its
/// `.ynz` files): it is copied whole and built as a project, whose binary is `bin` and whose IR
/// is `bin.ll`, one `; file: <path>` section per module.
pub fn build(fixture_name: &str, no_optimize: bool) -> Built {
    let directory = tempfile::tempdir().expect("create temp directory");
    let is_project = fixture_path(fixture_name).is_dir();
    let source = stage_fixture(directory.path(), fixture_name);
    build_staged(directory, source, is_project, fixture_name, no_optimize)
}

/// [`build`] for a program given as text: for the suites that generate a family of programs from
/// a template (one per spawn count, say) rather than keeping a file for each.
pub fn build_source(program: &str, label: &str, no_optimize: bool) -> Built {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("program.ynz");
    std::fs::write(&source, program).expect("write generated program");
    build_staged(directory, source, false, label, no_optimize)
}

fn build_staged(
    directory: tempfile::TempDir,
    source: PathBuf,
    is_project: bool,
    fixture_name: &str,
    no_optimize: bool,
) -> Built {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ynz"));
    command.arg("build").arg(&source).arg("--emit-ir");
    if no_optimize {
        command.arg("--no-optimize");
    }
    let output = command
        .env("CLICOLOR", "0")
        .output()
        .expect("spawn `ynz build`");
    assert!(
        output.status.success(),
        "{fixture_name}: `ynz build` failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let (binary, ir) = if is_project {
        (source.join("bin"), source.join("bin.ll"))
    } else {
        (
            directory.path().join("program"),
            directory.path().join("program.ll"),
        )
    };
    Built {
        binary,
        ir,
        _directory: directory,
    }
}

/// How often the watchdog polls the child. Only bounds how late a finished run is noticed; it
/// is not a performance budget.
const WATCHDOG_POLL: Duration = Duration::from_millis(5);

/// Read `stream` to its end on a thread of its own, so a chatty child cannot fill a pipe and
/// block while the watchdog polls for its exit.
fn drain<R: Read + Send + 'static>(stream: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut stream = stream;
        // A read error (child killed mid-write) still returns what arrived before it.
        let _ = stream.read_to_end(&mut bytes);
        bytes
    })
}

/// Run `command` to completion under the liveness watchdog. The child is killed through its own
/// handle (`Child::kill`), never an external `kill` binary that a minimal image may lack: a
/// hung fixture must not survive the failed test as an orphan.
fn run_with_watchdog(mut command: Command) -> RunOutput {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn compiled fixture");
    let stdout_reader = drain(child.stdout.take().expect("piped stdout"));
    let stderr_reader = drain(child.stderr.take().expect("piped stderr"));
    let deadline = Instant::now() + RUN_WATCHDOG;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll compiled fixture") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "WATCHDOG TRIP: compiled fixture did not exit within {RUN_WATCHDOG:?} — a hang, \
                 never a slow test; fix the program, never widen this timeout."
            );
        }
        std::thread::sleep(WATCHDOG_POLL);
    };
    RunOutput {
        stdout: String::from_utf8_lossy(&stdout_reader.join().expect("join stdout reader"))
            .into_owned(),
        stderr: String::from_utf8_lossy(&stderr_reader.join().expect("join stderr reader"))
            .into_owned(),
        exit_code: status.code().unwrap_or(-1),
    }
}

/// Run a built fixture with the runtime's alloc counters on and return what it printed plus the
/// counters. Panics when the counter file is missing or a key is absent: a silent `0` would turn
/// "the counter did not report" into "the program balanced".
pub fn run_counted(built: &Built) -> (RunOutput, Counters) {
    run_counted_with_drain_budget(built, None)
}

/// [`run_counted`] for a program whose `background` CPU tasks outlast its `entrypoint`. At exit the
/// runtime waits for the running tasks only up to `YNZ_SHUTDOWN_TIMEOUT_MS`, 5 s by default, and
/// cancels the rest, so a task still spinning at that cut-off prints nothing: with the default,
/// such a test passes or fails by how loaded the machine is. Here the exit itself is the
/// completion signal, and only [`RUN_WATCHDOG`] bounds the wait. Decided (search history for
/// "shutdown budget"): this is not a loosened timeout, and only the C6 spawn tests call it.
pub fn run_counted_until_tasks_finish(built: &Built) -> (RunOutput, Counters) {
    run_counted_with_drain_budget(built, Some(TASK_DRAIN_BUDGET_MS))
}

/// [`run_counted`] with the runtime's exit drain budget (`YNZ_SHUTDOWN_TIMEOUT_MS`, in ms) set when
/// `drain_budget_ms` is `Some`. For a test whose own timing decides what the shutdown cancels
/// (the recursion-cancellation fixtures pass a short budget so the cancel lands mid-sleep).
pub fn run_counted_with_drain_budget(
    built: &Built,
    drain_budget_ms: Option<&str>,
) -> (RunOutput, Counters) {
    let counter_file = built.binary.with_extension("counters");
    let mut command = Command::new(&built.binary);
    command
        .env("YNZ_ALLOC_COUNTER", "1")
        .env("YNZ_ALLOC_COUNTER_OUTPUT", &counter_file);
    if let Some(budget) = drain_budget_ms {
        command.env("YNZ_SHUTDOWN_TIMEOUT_MS", budget);
    }
    let output = run_with_watchdog(command);
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

/// Split the IR text of a multi-module build into one text per module. `ynz build` on a project
/// joins the modules, each behind a `; file: <path>` line; single-file IR has no such line and
/// comes back as the one module it is. O(n) time and space.
fn split_module_sections(ir: &str) -> Vec<String> {
    let mut sections: Vec<String> = Vec::new();
    for line in ir.lines() {
        if line.starts_with("; file: ") || sections.is_empty() {
            sections.push(String::new());
        }
        let current = sections.last_mut().expect("a section was just pushed");
        current.push_str(line);
        current.push('\n');
    }
    sections
}

/// Instrument `ir` (LLVM IR text) for ASan, compile it with `clang -fsanitize=address` and link
/// it against the bundled runtime archive. A multi-module build's IR is split per module and
/// every module is instrumented and linked, so a default lowered in one module and called from
/// another is checked too. The files are written into `directory`; returns the path of the
/// sanitized binary.
pub fn build_asan_from_ir(directory: &Path, ir: &str) -> PathBuf {
    let sanitized_irs: Vec<PathBuf> = split_module_sections(ir)
        .iter()
        .enumerate()
        .map(|(position, section)| {
            let sanitized_ir = directory.join(format!("program.asan.{position}.ll"));
            std::fs::write(&sanitized_ir, instrument_for_asan(section))
                .expect("write instrumented IR");
            sanitized_ir
        })
        .collect();
    let sanitized_binary = directory.join("program.asan");
    let output = Command::new(find_clang())
        .args(["-O0", "-g", "-fsanitize=address", "-no-pie"])
        .args(&sanitized_irs)
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
    let directory = built
        .ir
        .parent()
        .expect("emitted IR has a parent directory");
    build_asan_from_ir(directory, &ir)
}

/// [`run_asan_binary`] with the runtime's alloc counters on: what the sanitized program printed
/// plus the counters it wrote. Panics when the counter file is missing, as [`run_counted`] does.
pub fn run_asan_binary_counted(sanitized_binary: &Path) -> (RunOutput, Counters) {
    let counter_file = sanitized_binary.with_extension("counters");
    let mut command = Command::new(sanitized_binary);
    command
        .env("ASAN_OPTIONS", ASAN_OPTIONS)
        .env("YNZ_ALLOC_COUNTER", "1")
        .env("YNZ_ALLOC_COUNTER_OUTPUT", &counter_file);
    let output = run_with_watchdog(command);
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

/// Run an ASan-built binary under the fixtures' `ASAN_OPTIONS`, with the liveness watchdog.
pub fn run_asan_binary(sanitized_binary: &Path) -> RunOutput {
    let mut command = Command::new(sanitized_binary);
    command.env("ASAN_OPTIONS", ASAN_OPTIONS);
    run_with_watchdog(command)
}

/// [`run_asan_binary`] with the exit waiting for every running task, as
/// [`run_counted_until_tasks_finish`] does.
pub fn run_asan_binary_until_tasks_finish(sanitized_binary: &Path) -> RunOutput {
    let mut command = Command::new(sanitized_binary);
    command
        .env("ASAN_OPTIONS", ASAN_OPTIONS)
        .env("YNZ_SHUTDOWN_TIMEOUT_MS", TASK_DRAIN_BUDGET_MS);
    run_with_watchdog(command)
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
    run_plain_with_env(fixture_name, no_optimize, &[])
}

/// [`run_plain`] with extra environment variables for the compiled program (for example
/// `YNZ_WORKER_THREADS=1`, which makes a scheduling order deterministic).
pub fn run_plain_with_env(
    fixture_name: &str,
    no_optimize: bool,
    environment: &[(&str, &str)],
) -> RunOutput {
    let built = build(fixture_name, no_optimize);
    let mut command = Command::new(&built.binary);
    command.envs(environment.iter().copied());
    run_with_watchdog(command)
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

/// The shared check of the AddressSanitizer suites: `fixture_name` runs clean under ASan (no
/// report, exit 0) and prints exactly `expected_stdout`. A bad read that returns plausible bytes
/// leaves the output right, so the sanitizer verdict is asserted on its own.
pub fn assert_asan_clean_and_prints(fixture_name: &str, no_optimize: bool, expected_stdout: &str) {
    let run = run_under_asan(fixture_name, no_optimize);
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "{fixture_name} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
        run.stderr
    );
    assert_eq!(
        run.exit_code, 0,
        "{fixture_name} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
        run.stderr
    );
    assert_eq!(
        run.stdout, expected_stdout,
        "{fixture_name} (no_optimize={no_optimize}): wrong values; stderr:\n{}",
        run.stderr
    );
}

/// Run two independent checks at once and return both results. Each call builds into its own temp
/// directory and runs its own process (module header), so two checks share nothing; running them
/// in turn made one test as slow as their sum (`.claude/rules/test-parallelism.md`). A panic in
/// either is re-raised with its own message once both have finished.
pub fn concurrently<A: Send, B: Send>(
    first: impl FnOnce() -> A + Send,
    second: impl FnOnce() -> B + Send,
) -> (A, B) {
    std::thread::scope(|scope| {
        let first_handle = scope.spawn(first);
        let second_handle = scope.spawn(second);
        let second_result = second_handle.join();
        let first_result = first_handle.join();
        match (first_result, second_result) {
            (Ok(first_value), Ok(second_value)) => (first_value, second_value),
            (Err(payload), _) | (_, Err(payload)) => std::panic::resume_unwind(payload),
        }
    })
}

/// Run `check(no_optimize)` at -O2 and at -O0 at once. `check` puts `no_optimize` in its own
/// failure messages, so a failure names its level.
pub fn at_both_optimization_levels(check: impl Fn(bool) + Sync) {
    concurrently(|| check(false), || check(true));
}

/// Build `program`, run it with the alloc counters on, and check it exits 0 and prints `expected`.
/// Returns the counters. For the release suites (`v03_m9_c7_*`).
pub fn run_balanced(label: &str, program: &str, expected: &str, no_optimize: bool) -> Counters {
    let built = build_source(program, label, no_optimize);
    let (run, counters) = run_counted(&built);
    assert_eq!(
        run.exit_code, 0,
        "{label} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
        run.stderr
    );
    assert_eq!(run.stdout, expected, "{label} (no_optimize={no_optimize})");
    counters
}

/// Every heap allocation of the run was freed, and there was at least one (a program that
/// allocates nothing would balance trivially).
pub fn assert_heap_balanced(label: &str, counters: &Counters, no_optimize: bool) {
    assert!(
        counters.alloc > 0,
        "{label}: non-vacuous: the program must allocate: {counters:?}"
    );
    assert_eq!(
        counters.free, counters.alloc,
        "{label} (no_optimize={no_optimize}): every heap allocation must be freed: {counters:?}"
    );
}

/// Every string of the run was freed, and there was at least one.
pub fn assert_strings_balanced(label: &str, counters: &Counters, no_optimize: bool) {
    assert!(
        counters.string_alloc > 0,
        "{label}: non-vacuous: the program must build strings: {counters:?}"
    );
    assert_eq!(
        counters.string_free, counters.string_alloc,
        "{label} (no_optimize={no_optimize}): every string must be freed: {counters:?}"
    );
}

/// Build `program` under AddressSanitizer and check it exits 0, reports nothing and prints
/// `expected`.
pub fn assert_asan_clean(label: &str, program: &str, expected: &str, no_optimize: bool) {
    let built = build_source(program, label, no_optimize);
    let run = run_asan_binary(&build_asan(&built));
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "{label} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
        run.stderr
    );
    assert_eq!(
        run.exit_code, 0,
        "{label} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
        run.stderr
    );
    assert_eq!(run.stdout, expected, "{label} (no_optimize={no_optimize})");
}

/// Run `built` and check it exits 0, prints `expected`, allocated something (a program that allocates
/// nothing would balance trivially), freed every heap allocation and every string, and is
/// clean under AddressSanitizer. `label` names the program in a failure.
pub fn assert_built_balanced_and_asan_clean(
    label: &str,
    built: &Built,
    expected: &str,
    no_optimize: bool,
) {
    assert_built_balanced_and_asan_clean_printing_one_of(label, built, &[expected], no_optimize);
}

/// [`assert_built_balanced_and_asan_clean`] for a program whose output order the language does not
/// fix: it passes when the output is any one of `accepted`. A fire-and-forget task is never waited
/// for, so its prints can land before or after the spawner's own, and a loaded machine shows both.
pub fn assert_built_balanced_and_asan_clean_printing_one_of(
    label: &str,
    built: &Built,
    accepted: &[&str],
    no_optimize: bool,
) {
    let (run, counters) = run_counted(built);
    assert_eq!(
        run.exit_code, 0,
        "{label} (no_optimize={no_optimize}) must exit 0; stderr:\n{}",
        run.stderr
    );
    assert!(
        accepted.contains(&run.stdout.as_str()),
        "{label} (no_optimize={no_optimize}): output {:?} is none of {accepted:?}",
        run.stdout
    );
    assert!(
        counters.alloc + counters.string_alloc > 0,
        "{label} (no_optimize={no_optimize}): non-vacuous: the program must allocate: {counters:?}"
    );
    assert_eq!(
        counters.free, counters.alloc,
        "{label} (no_optimize={no_optimize}): every heap allocation must be freed: {counters:?}"
    );
    assert_eq!(
        counters.string_free, counters.string_alloc,
        "{label} (no_optimize={no_optimize}): every string must be freed: {counters:?}"
    );
    assert_built_asan_clean(label, built, no_optimize);
}

/// Run `built` under AddressSanitizer and check it exits 0 and reports nothing.
pub fn assert_built_asan_clean(label: &str, built: &Built, no_optimize: bool) {
    let sanitized = run_asan_binary(&build_asan(built));
    assert!(
        !sanitized.stderr.contains("AddressSanitizer") && sanitized.exit_code == 0,
        "{label} (no_optimize={no_optimize}): AddressSanitizer reported an error:\n{}",
        sanitized.stderr
    );
}

/// Build the fixture at both optimization levels and run [`assert_built_balanced_and_asan_clean`].
pub fn assert_fixture_balanced_and_asan_clean(fixture: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean(fixture, &built, expected, no_optimize);
    });
}

/// [`assert_fixture_balanced_and_asan_clean`] for a program whose output order the language does
/// not fix: it passes when the output is any one of `accepted`.
pub fn assert_fixture_balanced_and_asan_clean_printing_one_of(fixture: &str, accepted: &[&str]) {
    at_both_optimization_levels(|no_optimize| {
        let built = build(fixture, no_optimize);
        assert_built_balanced_and_asan_clean_printing_one_of(
            fixture,
            &built,
            accepted,
            no_optimize,
        );
    });
}

/// [`assert_fixture_balanced_and_asan_clean_printing_one_of`] for a fixture that ends
/// `wait sleep(N)` then `print(done)` while a fire-and-forget task it spawned prints
/// `task_output_then_done` minus the final `done` line. Nothing waits for such a task, so its
/// output may land before or after `done`; a loaded machine shows both. The bug check is the
/// balance and ASan, asserted in both orders.
pub fn assert_fixture_balanced_and_asan_clean_task_then_done(
    fixture: &str,
    task_output_then_done: &str,
) {
    let task_output = task_output_then_done
        .strip_suffix("done\n")
        .expect("expected output ends with the spawner's `done` line");
    let done_first = format!("done\n{task_output}");
    assert_fixture_balanced_and_asan_clean_printing_one_of(
        fixture,
        &[task_output_then_done, done_first.as_str()],
    );
}

/// Build `source` (labelled `label`) at both optimization levels and run
/// [`assert_built_balanced_and_asan_clean`].
pub fn assert_source_balanced_and_asan_clean(label: &str, source: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        let built = build_source(source, label, no_optimize);
        assert_built_balanced_and_asan_clean(label, &built, expected, no_optimize);
    });
}

/// The fixture names under `tests/fixtures` that contain `filter` (all of them when it is empty),
/// sorted.
pub fn fixture_names(filter: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixture_path(""))
        .expect("read the fixtures directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(filter))
        .collect();
    names.sort();
    names
}

/// Run `measure` on every item at once, on up to four workers, and return each item with its
/// result, sorted by item. A worker claims the next item as it frees up (per-item cost varies by
/// more than an order of magnitude) and the result is sorted because completion order is not
/// deterministic (`.claude/rules/test-parallelism.md`). O(n) time over n items / 4 workers.
pub fn sweep_in_parallel<'items, Result: Send>(
    items: &'items [String],
    measure: impl Fn(&'items str) -> Result + Sync,
) -> Vec<(&'items str, Result)> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    let cursor = AtomicUsize::new(0);
    let results: Mutex<Vec<(&'items str, Result)>> = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .min(4);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let position = cursor.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(position) else {
                    break;
                };
                let result = measure(item);
                results.lock().expect("results mutex").push((item, result));
            });
        }
    });
    let mut results = results.into_inner().expect("results mutex");
    results.sort_by(|left, right| left.0.cmp(right.0));
    results
}

/// Build `program`, run it under AddressSanitizer with LeakSanitizer on and check it exits 0,
/// prints `expected` and reports nothing. For what the alloc counters cannot see: a box the Tokio
/// runtime owns (a `Sleep`), a counted `Arc` a channel shares (a reference nobody gives back is a
/// leak, one given back twice is a free).
pub fn assert_asan_and_leaks_clean(label: &str, program: &str, expected: &str, no_optimize: bool) {
    let built = build_source(program, label, no_optimize);
    let mut command = Command::new(build_asan(&built));
    command.env("ASAN_OPTIONS", "detect_leaks=1:free_fill_byte=0");
    let run = run_with_watchdog(command);
    assert!(
        run.exit_code == 0 && !run.stderr.contains("Sanitizer"),
        "{label} (no_optimize={no_optimize}): ASan or LeakSanitizer reported:\n{}",
        run.stderr
    );
    assert_eq!(run.stdout, expected, "{label} (no_optimize={no_optimize})");
}

/// [`assert_asan_and_leaks_clean`] at both optimization levels.
pub fn assert_source_asan_and_leaks_clean(label: &str, source: &str, expected: &str) {
    at_both_optimization_levels(|no_optimize| {
        assert_asan_and_leaks_clean(label, source, expected, no_optimize);
    });
}
