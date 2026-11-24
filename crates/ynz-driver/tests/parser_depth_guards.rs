// WHY: F2 — `parse_block`/`parse_stmt` recursion had no depth cap, so deeply nested
// statements/blocks overflowed the stack and aborted the process with a raw core dump
// (no ICE banner, no exit-code-2 classification — it bypassed the driver's panic hook
// entirely). This test locks the fix: nesting past the cap must produce a clean
// compile-error diagnostic, never a process abort. Mirrors the audit's own live repro
// (20 000 nested `if (true) { ... }`).

use std::{io::Write, path::PathBuf, process::Command};

fn ynz_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ynz"))
}

/// Builds a `.ynz` source string with `depth` levels of nested `if (true) { ... }`
/// blocks inside a single function body.
fn deeply_nested_program(depth: usize) -> String {
    let mut src = String::from("function entrypoint() -> Nothing {\n");
    for _ in 0..depth {
        src.push_str("if (true) {\n");
    }
    src.push_str("print(`unreachable`)\n");
    for _ in 0..depth {
        src.push_str("}\n");
    }
    src.push_str("}\n");
    src
}

/// Builds a `.ynz` source string with `depth` levels of nested anonymous inline shape
/// TYPES (`{ a: { a: { a: ... } } }`) used as a `let` binding's type annotation.
fn deeply_nested_anon_shape_type_program(depth: usize) -> String {
    let mut src = String::from("function entrypoint() -> Nothing {\n");
    src.push_str("let x: ");
    for _ in 0..depth {
        src.push_str("{ a: ");
    }
    src.push_str("int");
    for _ in 0..depth {
        src.push_str(" }");
    }
    src.push_str(" = 0\n");
    src.push_str("}\n");
    src
}

/// Writes `src` to a temp `.ynz` file and runs `ynz run` on it. Returns the exit code
/// (`None` when a signal killed the process) and stderr.
fn run_source(src: &str) -> (Option<i32>, String) {
    let mut file = tempfile::Builder::new()
        .prefix("parser-depth-")
        .suffix(".ynz")
        .tempfile()
        .expect("failed to create temp fixture");
    file.write_all(src.as_bytes())
        .expect("failed to write fixture");
    let path = file.path().to_path_buf();

    let out = Command::new(ynz_binary())
        .args(["run", path.to_str().unwrap()])
        .env("CLICOLOR", "0")
        .output()
        .expect("failed to spawn ynz binary");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Nesting past the cap: a clean compile error (exit 1) carrying the cap's diagnostic.
/// Exit `None` is a raw stack-overflow abort (SIGABRT/SIGSEGV); exit 2 would mean the ICE
/// hook caught a panic, but the fix is a parser-level diagnostic, not a caught panic.
fn assert_rejected_with(src: &str, expected_phrase: &str) {
    let (code, stderr) = run_source(src);
    assert!(
        code.is_some(),
        "process was killed by a signal (raw stack overflow), not a clean diagnostic exit\nstderr:\n{stderr}"
    );
    assert_eq!(
        code,
        Some(1),
        "expected a clean compile-error exit (1), got {code:?}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains(expected_phrase),
        "expected the depth-cap diagnostic {expected_phrase:?} in stderr; got:\n{stderr}"
    );
}

/// Nesting under the cap: never killed by a signal and never trips the cap's diagnostic.
/// `must_exit_zero` is set where the program also type-checks cleanly.
fn assert_accepted(src: &str, cap_phrase: &str, must_exit_zero: bool) {
    let (code, stderr) = run_source(src);
    assert!(
        code.is_some(),
        "process was killed by a signal at a depth under the cap\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains(cap_phrase),
        "nesting under the cap must not trip {cap_phrase:?}; stderr:\n{stderr}"
    );
    if must_exit_zero {
        assert_eq!(
            code,
            Some(0),
            "nesting under the cap must compile cleanly; stderr:\n{stderr}"
        );
    }
}

/// One named test per input class the depth caps decide differently.
macro_rules! depth_guard_case {
    ($test_name:ident, $check:ident($source:expr $(, $argument:expr)*)) => {
        // bug: nesting past a parser depth cap aborts the process with a stack overflow instead of a diagnostic, or a legitimate depth under the cap is rejected | tier: High
        #[test]
        fn $test_name() {
            $check(&$source $(, $argument)*);
        }
    };
}

// 20 000 matches the audit's own live repro: well past any real program and past the
// 8 MB main-thread stack if recursion were unbounded.
depth_guard_case!(
    deeply_nested_blocks_are_rejected_not_aborted,
    assert_rejected_with(deeply_nested_program(20_000), "Nesting too deep")
);

// Anonymous shape TYPES (`{ a: { a: ... } }`): each nested field's type reset the depth to 0,
// bypassing both caps, so 5 000 levels (the audit's repro) overflowed the stack with no
// diagnostic. The fix threads depth through field-type parsing, so the existing 16-level
// generic-nesting cap's diagnostic fires.
depth_guard_case!(
    deeply_nested_anonymous_shape_types_are_rejected_not_aborted,
    assert_rejected_with(
        deeply_nested_anon_shape_type_program(5_000),
        "Generic type nesting exceeds 16 levels"
    )
);

// Guard against a cap set too low: 100 levels is well under the 256 block cap and must
// compile with zero diagnostics.
depth_guard_case!(
    moderately_nested_blocks_are_accepted,
    assert_accepted(deeply_nested_program(100), "Nesting too deep", true)
);

// 10 levels is under the 16-level type cap. The type is not expected to type-check (an
// int literal assigned to a shape), so only the depth-cap diagnostic's absence is asserted.
depth_guard_case!(
    moderately_nested_anonymous_shape_types_are_accepted,
    assert_accepted(
        deeply_nested_anon_shape_type_program(10),
        "Generic type nesting exceeds 16 levels",
        false
    )
);
