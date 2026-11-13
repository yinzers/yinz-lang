//! v0.3-M9 C2: controls for the AddressSanitizer harness in `support`.
//!
//! Tier: High (the 18 `v03_m9_release_safety` ASan tests are green BY DESIGN today, so they only
//! mean something if this harness really reports an early or a double free in generated code).
//! Bug caught: a harness that silently detects nothing (the `sanitize_address` attribute not
//! applied, the sanitizer runtime not linked, `ASAN_OPTIONS` masking reports) turns every ASan
//! safety fixture into a vacuous pass, and the release pass could then free early or twice with
//! the suite staying green.
//!
//! Each control is a hand-written LLVM module that calls the same runtime entry points the
//! release pass will (`ynz_alloc`, `ynz_free`). It goes through the same instrument + link + run
//! path as the fixtures, and must be reported with the right ASan error class: a use-after-free
//! READ in the instrumented module, and a double free through the runtime's `free`. A clean
//! program (alloc, use, free once) must NOT be reported, so a harness that flags everything
//! cannot pass either. Three tests: one per outcome the harness must tell apart.

mod support;

use support::{build_asan_from_ir, run_asan_binary, RunOutput};

const DECLARATIONS: &str = "declare ptr @ynz_alloc(i64)\ndeclare void @ynz_free(ptr, i64)\n";

fn run_module(main_body: &str) -> RunOutput {
    let ir = format!("{DECLARATIONS}define i32 @main() {{\n{main_body}\n}}\n");
    let dir = tempfile::tempdir().expect("create temp dir");
    let binary = build_asan_from_ir(dir.path(), &ir);
    run_asan_binary(&binary)
}

#[test]
fn control_free_once_then_no_access_is_not_reported() {
    // The harness must not flag a correct program.
    let run = run_module(
        "  %p = call ptr @ynz_alloc(i64 16)\n  store i64 7, ptr %p\n  %v = load i64, ptr %p\n  \
         call void @ynz_free(ptr %p, i64 16)\n  ret i32 0",
    );
    assert!(
        !run.stderr.contains("AddressSanitizer"),
        "false positive on a correct program:\n{}",
        run.stderr
    );
    assert_eq!(run.exit_code, 0, "stderr:\n{}", run.stderr);
}

#[test]
fn control_read_after_free_in_generated_code_is_reported() {
    // A release that runs before a later read of the value: the instrumented load must trip.
    let run = run_module(
        "  %p = call ptr @ynz_alloc(i64 16)\n  store i64 7, ptr %p\n  \
         call void @ynz_free(ptr %p, i64 16)\n  %v = load i64, ptr %p\n  ret i32 0",
    );
    assert!(
        run.stderr.contains("heap-use-after-free"),
        "use-after-free in generated code went undetected (exit {}):\n{}",
        run.exit_code,
        run.stderr
    );
    assert_ne!(run.exit_code, 0, "ASan must fail the run");
}

#[test]
fn control_double_free_through_the_runtime_is_reported() {
    // Two releases of one allocation (callee and caller both free): caught at the runtime's
    // `free`, which ASan intercepts although the runtime itself is not instrumented.
    let run = run_module(
        "  %p = call ptr @ynz_alloc(i64 16)\n  call void @ynz_free(ptr %p, i64 16)\n  \
         call void @ynz_free(ptr %p, i64 16)\n  ret i32 0",
    );
    assert!(
        run.stderr.contains("attempting double-free"),
        "double free went undetected (exit {}):\n{}",
        run.exit_code,
        run.stderr
    );
    assert_ne!(run.exit_code, 0, "ASan must fail the run");
}
