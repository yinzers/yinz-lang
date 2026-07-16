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
    let mut src = String::from("function entrypoint() -> nothing {\n");
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

#[test]
fn deeply_nested_blocks_never_abort_the_process() {
    // 20 000 matches the audit's own live repro — comfortably past any real program
    // and past the 8 MB default main-thread stack if recursion were unbounded.
    let src = deeply_nested_program(20_000);

    let mut file = tempfile::Builder::new()
        .prefix("f2-deep-nest-")
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

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    // The process must exit normally (Some(code)), never be killed by a signal
    // (SIGABRT/SIGSEGV from a raw stack overflow surfaces as `status.code() == None`
    // on Unix). This is the load-bearing assertion — before the F2 fix, this fired
    // as a raw stack-overflow abort with no code at all.
    assert!(
        out.status.code().is_some(),
        "process was killed by a signal (raw stack overflow), not a clean diagnostic exit; \
         status: {:?}\nstderr:\n{stderr}",
        out.status
    );

    // Exit code 1 = clean compile error (see main.rs EXIT_COMPILE_ERROR). Exit code 2
    // would mean this routed through the ICE hook (a panic), which is also wrong — F2's
    // fix is a parser-level teaching diagnostic, not a caught panic.
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected a clean compile-error exit (1), got {:?}\nstderr:\n{stderr}",
        out.status.code()
    );

    assert!(
        stderr.contains("Nesting too deep"),
        "expected the F2 nesting-depth teaching diagnostic in stderr; got:\n{stderr}"
    );
}

/// Builds a `.ynz` source string with `depth` levels of nested anonymous inline shape
/// TYPES (`{ a: { a: { a: ... } } }`) used as a `let` binding's type annotation.
fn deeply_nested_anon_shape_type_program(depth: usize) -> String {
    let mut src = String::from("function entrypoint() -> nothing {\n");
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

#[test]
fn deeply_nested_anonymous_shape_types_never_abort_the_process() {
    // BLOCKER 1: `parse_type_with_depth`'s
    // `Token::LBrace` arm (anonymous inline shape type) parsed each field's type via a
    // bare `self.parse_field_decl(...)` → `self.parse_type()`, which resets depth to 0
    // on every nested field — bypassing BOTH the 16-level generic-nesting cap and the
    // 256-level block-depth cap. `{ a: { a: ... } }` nesting in TYPE position overflowed
    // the stack (real repro: SIGABRT around depth ~5000) with no diagnostic at all.
    // 5_000 mirrors the audit's own live repro depth.
    let src = deeply_nested_anon_shape_type_program(5_000);

    let mut file = tempfile::Builder::new()
        .prefix("f-anon-shape-depth-")
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

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    // Load-bearing assertion: before the fix, this fired as a raw stack-overflow abort
    // (status.code() == None on Unix — killed by SIGABRT/SIGSEGV), not a clean diagnostic.
    assert!(
        out.status.code().is_some(),
        "process was killed by a signal (raw stack overflow via anon-shape-type field \
         recursion bypassing the depth cap), not a clean diagnostic exit; status: {:?}\nstderr:\n{stderr}",
        out.status
    );

    assert_eq!(
        out.status.code(),
        Some(1),
        "expected a clean compile-error exit (1), got {:?}\nstderr:\n{stderr}",
        out.status.code()
    );

    // The fix threads depth through field-type parsing so this hits the EXISTING
    // 16-level generic-nesting cap's teaching diagnostic — no new diagnostic needed.
    assert!(
        stderr.contains("Generic type nesting exceeds 16 levels"),
        "expected the existing 16-level type-nesting cap diagnostic in stderr; got:\n{stderr}"
    );
}

#[test]
fn moderately_nested_anonymous_shape_types_still_compile_cleanly() {
    // WHY: guard against an off-by-one cap that rejects legitimate (if unusual) code.
    // 10 levels is well under the 16-level generic-nesting cap and must compile cleanly.
    let src = deeply_nested_anon_shape_type_program(10);

    let mut file = tempfile::Builder::new()
        .prefix("f-anon-shape-moderate-")
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

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    // Depth-only concern here — the type isn't expected to type-check meaningfully
    // (it's an anonymous shape assigned an int literal), so we only assert the process
    // wasn't killed by a signal and no depth-cap diagnostic fired. Typeck errors (if
    // any) are irrelevant to this test's WHY.
    assert!(
        out.status.code().is_some(),
        "process was killed by a signal at a depth well under the cap; status: {:?}\nstderr:\n{stderr}",
        out.status
    );
    assert!(
        !stderr.contains("Generic type nesting exceeds 16 levels"),
        "10 levels of anon-shape-type nesting is well under the 16-level cap and must not \
         trip it; stderr:\n{stderr}"
    );
}

#[test]
fn moderately_nested_blocks_still_compile_cleanly() {
    // WHY: guard against an off-by-one cap that rejects legitimate (if unusual) code.
    // 100 levels is well under the 256 cap and must compile with zero diagnostics.
    let src = deeply_nested_program(100);

    let mut file = tempfile::Builder::new()
        .prefix("f2-moderate-nest-")
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

    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(
        out.status.code(),
        Some(0),
        "100 levels of nesting is well under the 256 cap and must compile cleanly; stderr:\n{stderr}"
    );
}
