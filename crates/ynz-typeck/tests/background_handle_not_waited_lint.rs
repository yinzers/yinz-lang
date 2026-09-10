// The `background-handle-not-waited` Tier 3 lint (v0.3-M8 Phase 7 no-duct-tape guard).
//
// WHY: Patrick re-deferred the real fix (codegen calling `ynz_handle_free` when a handle
// binding's scope ends) on condition that the live exposure ships with a LOUD guard —
// nothing releases a local of any type at scope exit today, so a task whose handle goes
// out of scope keeps running unseen. This lint is the nag, not the cure; it retires the
// moment the real fix ships (registry entry `background-handle-cancel-injection`).
//
// Three shapes matter: it MUST fire on a bound handle that is never received anywhere in
// its scope, and it must NOT fire on the two shapes that would make it noise — an unbound
// (fire-and-forget) spawn with no handle to mislead anyone, and a bound handle that IS
// received before its scope ends.

use ynz_diagnostics::{DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

const FILE: &str = "background_handle_not_waited_test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn lint_diags<'a>(out: &'a CheckOutput, rule: &str) -> Vec<&'a ynz_diagnostics::Diagnostic> {
    out.diagnostics
        .iter()
        .filter(|d| matches!(&d.kind, Some(DiagnosticKind::LintRule { rule: r }) if *r == rule))
        .collect()
}

fn assert_no_errors(out: &CheckOutput) {
    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, Severity::Error))
        .collect();
    assert!(errors.is_empty(), "expected 0 errors, got: {errors:#?}");
}

const RULE: &str = "background-handle-not-waited";

const WORKER: &str = "function worker() -> int { wait sleep(5)\n return 7 }\n";

#[test]
fn bound_handle_never_received_fires_the_lint() {
    // The exposing shape: `let h = background worker()` with no `.receive()` anywhere
    // in the rest of `entrypoint`'s scope.
    let src = format!(
        "{WORKER}function entrypoint() -> nothing {{ let h = background worker()\n \
         print(`spawned`) }}"
    );
    let out = run(&src);
    assert_no_errors(&out);

    let lints = lint_diags(&out, RULE);
    assert_eq!(
        lints.len(),
        1,
        "one un-received handle binding -> one lint; got: {lints:#?}"
    );
    let lint = lints[0];
    assert_eq!(
        lint.severity,
        Severity::Suggestion,
        "background-handle-not-waited is a dismissable suggestion, NEVER an error \
         (a correct program must still compile)"
    );
    assert!(
        lint.what.contains('h'),
        "WHAT names the handle binding: {}",
        lint.what
    );
    assert!(
        lint.what_instead.contains("h.receive()"),
        "WHAT-INSTEAD names the fix in terms of this binding: {}",
        lint.what_instead
    );
    assert!(!lint.why.is_empty());
}

#[test]
fn unbound_fire_and_forget_spawn_does_not_fire() {
    // No handle exists to mislead anyone — `background worker()` with no `let` binding.
    let src = format!(
        "{WORKER}function entrypoint() -> nothing {{ background worker()\n \
         print(`spawned`) }}"
    );
    let out = run(&src);
    assert_no_errors(&out);
    assert!(
        lint_diags(&out, RULE).is_empty(),
        "an unbound fire-and-forget spawn has no handle to nag about"
    );
}

#[test]
fn handle_received_later_in_scope_does_not_fire() {
    // The handle IS waited on — `.receive()` appears later in the same scope.
    let src = format!(
        "{WORKER}function entrypoint() -> nothing {{ let h = background worker()\n \
         let done = h.receive()\n let value = done.or(0)\n print(value.toString()) }}"
    );
    let out = run(&src);
    assert_no_errors(&out);
    assert!(
        lint_diags(&out, RULE).is_empty(),
        "a handle received before scope end must not fire the nag"
    );
}

#[test]
fn handle_received_inside_a_later_nested_block_does_not_fire() {
    // The `.receive()` lives inside a nested `if` body that follows the spawn — still
    // "waited on anywhere in scope," so the lint must not fire (mirrors
    // `ident_read_in_stmt`'s existing nested-block recursion).
    let src = format!(
        "{WORKER}function entrypoint() -> nothing {{ let h = background worker()\n \
         if (true) {{ let done = h.receive()\n let value = done.or(0)\n \
         print(value.toString()) }} }}"
    );
    let out = run(&src);
    assert_no_errors(&out);
    assert!(
        lint_diags(&out, RULE).is_empty(),
        "a handle received inside a later nested block must not fire the nag"
    );
}
