//! Controls for the AddressSanitizer harness in `support`. The ASan safety fixtures are green by
//! design, so they only mean something if this harness really reports an early or a double free
//! in generated code, and stays quiet on a correct program.
//!
//! Each control is a hand-written LLVM module that calls the runtime entry points the release
//! pass uses (`ynz_alloc`, `ynz_free`) and goes through the same instrument + link + run path as
//! the fixtures.

mod support;

use support::{build_asan_from_ir, run_asan_binary, RunOutput};

const DECLARATIONS: &str = "declare ptr @ynz_alloc(i64)\ndeclare void @ynz_free(ptr, i64)\n";

fn run_module(main_body: &str) -> RunOutput {
    let ir = format!("{DECLARATIONS}define i32 @main() {{\n{main_body}\n}}\n");
    let directory = tempfile::tempdir().expect("create temp directory");
    let binary = build_asan_from_ir(directory.path(), &ir);
    run_asan_binary(&binary)
}

/// One named test per outcome the harness must tell apart; each case is a test the runner names.
macro_rules! asan_harness_control_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: the ASan harness detects nothing (or flags everything), so every ASan safety fixture passes vacuously and a release pass could free early or twice unseen | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

asan_harness_control_high_case! {
    // case: a correct program (alloc, use, free once) is not reported
    control_free_once_then_no_access_is_not_reported => {
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
    // case: a read after the release, in instrumented generated code, trips the sanitizer
    control_read_after_free_in_generated_code_is_reported => {
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
    // case: two releases of one allocation are caught at the runtime's `free`, which ASan intercepts
    control_double_free_through_the_runtime_is_reported => {
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
}
