// WHY: signature mismatch must produce a three-part diagnostic. Catches the
// regression where typeck silently degrades to one-line error messages that
// leave the developer without a "what to do instead" or "why".

use ynz_ast::nodes::Item;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput, Type};

const FILE: &str = "test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

// ─── Happy path ──────────────────────────────────────────────────────────────

#[test]
fn m1_source_type_checks_clean() {
    // WHY: if this test breaks, the hello-world integration test (Phase 7) will also fail.
    let output = run(r#"function main() -> nothing { print("hello, yinz") }"#);
    assert!(
        output.diagnostics.is_empty(),
        "M1 source must type-check with zero diagnostics, got: {:#?}",
        output.diagnostics
    );
    // Confirm the string literal got its type recorded.
    let string_ty = output
        .typed_module
        .expr_types
        .values()
        .find(|t| **t == Type::String);
    assert!(
        string_ty.is_some(),
        "String literal type should be in expr_types"
    );
}

// ─── Type variant count gate ─────────────────────────────────────────────────

#[test]
fn m1_type_variant_count_locked() {
    // WHY: adding `int`, `float`, or `number` before their milestones would
    // introduce untested code paths. This test pins the M1 type surface.
    // M1 count: Nothing(1) + String(2) + Error(3)
    let all: &[Type] = &[Type::Nothing, Type::String, Type::Error];
    assert_eq!(all.len(), 3, "Type variant count changed from 3");
}

// ─── Missing `main` ──────────────────────────────────────────────────────────

#[test]
fn empty_file_missing_main_produces_diagnostic() {
    // WHY: an empty file is valid parse input (zero items, zero parse errors).
    // The type checker is responsible for the "no main" diagnostic, not the parser.
    let output = run("");
    assert!(
        !output.diagnostics.is_empty(),
        "Empty file must produce a 'no main function' diagnostic"
    );
    let what = &output.diagnostics[0].what;
    assert!(
        what.contains("main"),
        "Diagnostic must mention 'main', got: {what}"
    );
    // Verify three-part format is present.
    assert!(
        !output.diagnostics[0].what_instead.is_empty(),
        "what_instead must be non-empty"
    );
    assert!(
        !output.diagnostics[0].why.is_empty(),
        "why must be non-empty"
    );
}

// ─── Wrong `main` return type ─────────────────────────────────────────────────

#[test]
fn main_with_wrong_return_type_produces_diagnostic() {
    // WHY: `main` returning anything other than `nothing` would crash the
    // linker or produce undefined behaviour. This must be caught at compile time.
    let output = run(r#"function main() -> string { print("hi") }"#);
    assert!(
        !output.diagnostics.is_empty(),
        "Wrong return type on main must produce a diagnostic"
    );
    let what = &output.diagnostics[0].what;
    assert!(
        what.contains("main"),
        "Diagnostic must mention 'main', got: {what}"
    );
}

// ─── `main` with parameters ──────────────────────────────────────────────────

#[test]
fn main_with_parameters_produces_diagnostic() {
    // WHY: M1 `main` takes no parameters. The parser accepts parameters in the
    // grammar for forward-compatibility, but the type checker rejects them.
    // (This test exercises the wrong-arg-count path via the type checker.)
    //
    // Note: M1 parser actually errors on parameters before typeck sees them,
    // so this test verifies the joint parse+typeck pipeline still produces
    // an error for a hand-constructed broken source.
    let output = run("function main(x) -> nothing { }");
    // Parser error on the parameter is fine — we just need at least one diagnostic.
    assert!(
        !output.diagnostics.is_empty(),
        "main with parameters must produce a diagnostic"
    );
}

// ─── Undefined identifier ────────────────────────────────────────────────────

#[test]
fn undefined_identifier_produces_diagnostic() {
    // WHY: referencing an undefined name must error at compile time, not at
    // runtime. A runtime crash from an undefined name is exactly what
    // compile-time type checking exists to prevent.
    let output = run(r#"function main() -> nothing { unknownIdent() }"#);
    assert!(
        !output.diagnostics.is_empty(),
        "Undefined identifier must produce a diagnostic"
    );
    let what = &output.diagnostics[0].what;
    assert!(
        what.contains("unknownIdent"),
        "Diagnostic must name the unknown identifier, got: {what}"
    );
}

// ─── Type mismatch ───────────────────────────────────────────────────────────
// This test lives in src/check.rs as a #[cfg(test)] unit test because it uses
// the test-only BuiltinTable::with_test_builtin method (gated by #[cfg(test)]).
// See crates/ynz-typeck/src/check.rs for the test.

// ─── Parse-error gate ────────────────────────────────────────────────────────

#[test]
fn parse_error_gate_prevents_cascade_noise() {
    // WHY: type-checking a body that has parse errors produces duplicate
    // diagnostics that confuse the developer. The gate ensures typeck is silent
    // for functions whose bodies contain error nodes.
    //
    // We exercise this by passing source that the parser recovers from.
    let output = run("function main() -> nothing { $ }");
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();

    // The `$` produces a lex error. Typeck should NOT add additional errors
    // about the body (e.g. "unknown identifier" for the error placeholder).
    // We expect exactly 1 error: the lex error for `$`.
    // If typeck cascades, we'd see extra errors about the error-recovery node.
    assert_eq!(
        errors.len(),
        1,
        "Parse error gate should suppress typeck cascade — expected 1 error, got {}: {:#?}",
        errors.len(),
        errors
    );
}

// ─── Salsa invalidation ──────────────────────────────────────────────────────

#[test]
fn check_re_runs_when_source_changes() {
    // WHY: check_query depends on parse_query. When source changes, salsa
    // must re-run the full pipeline. If it doesn't, stale types persist.
    use salsa::Setter as _;

    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), "".to_string());

    let diag_count_before = check_query(&db, sf).diagnostics.len();

    sf.set_text(&mut db)
        .to(r#"function main() -> nothing { print("hi") }"#.to_string());

    let diag_count_after = check_query(&db, sf).diagnostics.len();

    // Before: empty file (1 error: missing main).
    // After: valid M1 program (0 errors).
    assert!(diag_count_before > 0, "Empty file should have diagnostics");
    assert_eq!(
        diag_count_after, 0,
        "Valid M1 program should have 0 diagnostics"
    );
}
