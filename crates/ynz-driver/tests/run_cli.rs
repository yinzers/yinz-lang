//! CLI integration tests for `ynz run`.
//!
//! Each test invokes the real `ynz` binary via `assert_cmd` and verifies exit codes and
//! stderr content.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

fn ynz() -> Command {
    Command::cargo_bin("ynz").expect("ynz binary not found")
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

// M6 Phase 6 regression fixture (P3-3 sibling fix — `ynz run` signal masking).
//
// WHY: a signal-terminated child process (here: unbounded non-tail recursion driving a
// real, deterministic native stack overflow — see the fixture's own header comment for
// why this needs no compiler bug to reproduce) previously fell through to
// `status.code().unwrap_or(1)` in `crates/ynz-driver/src/run.rs`, silently reporting
// exit code 1 — indistinguishable from the compiled Yinz program calling `exit(1)` on
// purpose. This test proves the fix: `ynz run` must report the signal by name on
// stderr, in the three-part WHAT/WHAT-INSTEAD/WHY format `docs/reference/REF-compiler-errors.md`
// requires for a `RUNTIME ERROR:`-severity diagnostic, and return the POSIX-shell
// `128 + signal` exit code (139 for SIGSEGV), not the masked `1`.
#[test]
#[cfg(unix)]
fn signal_terminated_child_reports_signal_not_bare_exit_1() {
    let path = fixture("v0_3_m6_signal_terminated_stack_overflow.ynz");

    ynz()
        .arg("run")
        .arg(&path)
        .assert()
        .failure()
        .code(139) // 128 + 11 (SIGSEGV) — the POSIX-shell convention, not the masked `1`.
        .stderr(predicate::str::contains("RUNTIME ERROR:"))
        .stderr(predicate::str::contains("killed by signal 11 (SIGSEGV)"))
        .stderr(predicate::str::contains("compiler miscompile"));
}
