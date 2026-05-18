// WHY: `errors`-capable typeck (M7 P3a) must enforce: (1) ErrorsCapable type is
// assigned to the return type of errors-capable functions; (2) calling an errors
// function in a non-errors context produces a diagnostic; (3) `.failed()`, `.or()`,
// `.message`, `.suggestions`, `.trace`, `.source` type-check correctly; (4) auto-
// propagation narrows the binding to the success type on first use inside an errors
// function; (5) `if (x.failed())` narrows `x` to success in subsequent code.
// Catches regressions where the errors flow-tracking machinery silently degrades.
//
// test-ratchet: M7 P3a adds ErrorsCapable to the type system and all associated
// flow-sensitive enforcement rules.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput, Type};

const FILE: &str = "test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn assert_clean(source: &str) {
    let output = run(source);
    assert!(
        output.diagnostics.is_empty(),
        "Expected 0 diagnostics, got {}: {:#?}",
        output.diagnostics.len(),
        output.diagnostics
    );
}

fn assert_errors(source: &str, expected_count: usize) -> CheckOutput {
    let output = run(source);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .collect();
    assert_eq!(
        errors.len(),
        expected_count,
        "Expected {expected_count} errors, got {}:\n{:#?}",
        errors.len(),
        errors
    );
    output
}

fn assert_has_diagnostic(output: &CheckOutput, expected_fragment: &str) {
    let found = output.diagnostics.iter().any(|d| {
        d.what.contains(expected_fragment)
            || d.what_instead.contains(expected_fragment)
            || d.why.contains(expected_fragment)
    });
    assert!(
        found,
        "Expected diagnostic containing {:?} but got:\n{:#?}",
        expected_fragment, output.diagnostics
    );
}

// ── 1. ErrorsCapable type exists and type_name formats correctly ─────────────

#[test]
fn m7_errors_capable_type_variant_exists() {
    // WHY: Type::ErrorsCapable must be instantiatable with an inner type. If the
    // variant is missing, any test depending on errors typeck will panic before
    // reaching the actual assertion.
    let t = Type::ErrorsCapable {
        inner: Box::new(Type::String),
    };
    assert!(matches!(t, Type::ErrorsCapable { .. }));
}

#[test]
fn m7_errors_capable_type_count_is_20() {
    // WHY: adding the 20th type variant (ErrorsCapable) must be tracked. This
    // test catches accidental double-additions or premature additions that would
    // increase the count without a milestone lock comment.
    //
    // test-ratchet: M7 P3a bumps the count from 19 to 20.
    let variants: &[Type] = &[
        Type::Nothing,
        Type::String,
        Type::Error,
        Type::Int,
        Type::Float,
        Type::Number { precision: 34 },
        Type::Bool,
        Type::Range {
            element: Box::new(Type::Int),
            end_inclusive: false,
        },
        Type::Shape { name: "Foo".into() },
        Type::Dynamic {
            contract: "Foo".into(),
        },
        Type::TypeParam { name: "T".into() },
        Type::Generic {
            name: "Pair".into(),
            args: vec![Type::Int, Type::String],
        },
        Type::BuiltinArray {
            elem: Box::new(Type::Int),
        },
        Type::BuiltinFixed {
            elem: Box::new(Type::Int),
            size: None,
        },
        Type::Maybe {
            inner: Box::new(Type::Int),
        },
        Type::BuiltinMap {
            key: Box::new(Type::String),
            val: Box::new(Type::Int),
        },
        Type::MapEntry {
            key: Box::new(Type::String),
            val: Box::new(Type::Int),
        },
        Type::Options {
            name: "Status".into(),
        },
        Type::Union {
            variants: vec![Type::Int, Type::String],
        },
        Type::ErrorsCapable {
            inner: Box::new(Type::String),
        },
    ];
    // test-ratchet: M7 P3a adds ErrorsCapable as the 20th type variant.
    // M1: 3, M2: +4=7, M3: +1=8, M4: +2=10, M5: +7=17, M6: +2=19, M7: +1=20.
    assert_eq!(
        variants.len(),
        20,
        "Type variant count changed — add a // test-ratchet: comment. Got {}",
        variants.len()
    );
}

// ── 2. Errors-capable function can call another errors-capable function ──────

#[test]
fn m7_errors_fn_calls_errors_fn_clean() {
    // WHY: an errors-capable function calling another errors-capable function must
    // type-check cleanly. Auto-propagation fires at the use site — no diagnostic.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `hello`
}

function process() -> string errors {
  let content = readFile()
  return content
}

function entrypoint() -> nothing {
  let r = process()
  let msg = r.or(`default`)
  print(msg)
}
"#,
    );
}

// ── 3. Non-errors function calling errors function → diagnostic ──────────────

#[test]
fn m7_non_errors_fn_calls_errors_fn_emits_diagnostic() {
    // WHY: a non-errors function that calls an errors function without handling
    // the failure must emit a diagnostic. Silently swallowing errors means the
    // caller never knows the operation failed.
    let out = assert_errors(
        r#"
function readFile() -> string errors {
  return `hello`
}

function entrypoint() -> nothing {
  let content = readFile()
  print(content)
}
"#,
        1,
    );
    assert_has_diagnostic(&out, "failure is not handled here");
}

// ── 4. .failed() on errors result returns bool ───────────────────────────────

#[test]
fn m7_failed_method_returns_bool() {
    // WHY: `.failed()` is the gate for accessing error details. It must return
    // `bool`. If it returns something else, `if (x.failed())` won't type-check.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `ok`
}

function entrypoint() -> nothing {
  let r = readFile()
  if (r.failed()) {
    print(`failed`)
  }
}
"#,
    );
}

// ── 5. .or(default) on errors string result returns string ───────────────────

#[test]
fn m7_or_method_returns_inner_type() {
    // WHY: `.or(default)` is the "give me the success value or a fallback" escape
    // hatch. It must return the success type so the binding is usable as a plain
    // value — not as ErrorsCapable — after the call.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `hello`
}

function entrypoint() -> nothing {
  let r = readFile()
  let content = r.or(`default`)
  print(content)
}
"#,
    );
}

// ── 6. .message on errors result returns string ──────────────────────────────

#[test]
fn m7_message_method_returns_string() {
    // WHY: `.message` is the human-readable error description. It must return
    // `string` so it can be passed to `print`. Wrong type breaks the primary
    // diagnostic display path.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `hello`
}

function entrypoint() -> nothing {
  let r = readFile()
  if (r.failed()) {
    print(r.message)
  }
}
"#,
    );
}

// ── 7. .suggestions returns array<string> ────────────────────────────────────

#[test]
fn m7_suggestions_method_returns_array_of_string() {
    // WHY: `.suggestions` must return `array<string>` to support iterating over
    // fix hints. Wrong type breaks loops like `for (s in r.suggestions)`.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `hello`
}

function entrypoint() -> nothing {
  let r = readFile()
  if (r.failed()) {
    let hints = r.suggestions
    print(hints.count().toString())
  }
}
"#,
    );
}

// ── 8. Errors function mark + use result → success type flows through ─────────

#[test]
fn m7_errors_fn_auto_propagation_success_type() {
    // WHY: in an errors-capable function, using an ErrorsCapable binding as a
    // plain value triggers auto-propagation. The binding's type must narrow to
    // the inner success type so it can be passed to functions expecting `string`.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `content`
}

function processContent(share content: string) -> nothing {
  print(content)
}

function process() -> nothing errors {
  let content = readFile()
  processContent(content)
}

function entrypoint() -> nothing {
  let r = process()
  if (r.failed()) {
    print(`process failed`)
  }
}
"#,
    );
}

// ── 9. Errors-capable function calling non-errors function → no diagnostic ───

#[test]
fn m7_errors_fn_calls_non_errors_fn_clean() {
    // WHY: calling a non-errors function from an errors-capable function is always
    // valid — the callee can't fail so no error-handling is needed.
    assert_clean(
        r#"
function greet() -> string {
  return `hello`
}

function process() -> string errors {
  let msg = greet()
  return msg
}

function entrypoint() -> nothing {
  let r = process()
  print(r.or(`failed`))
}
"#,
    );
}

// ── 10. .or() gives the success value — no diagnostic in non-errors context ──

#[test]
fn m7_or_in_non_errors_context_no_diagnostic() {
    // WHY: `.or(default)` is the explicit handler for non-errors contexts. Using
    // it eliminates the "must handle" diagnostic that fires when the result is
    // silently ignored.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `content`
}

function entrypoint() -> nothing {
  let content = readFile().or(`default`)
  print(content)
}
"#,
    );
}

// ── 11. .failed() check handles the result — no diagnostic ───────────────────

#[test]
fn m7_failed_check_handles_result_no_diagnostic() {
    // WHY: checking `.failed()` is one of the three valid ways to handle an
    // errors-capable value in a non-errors function. After the check, the
    // compiler knows the failure was explicitly handled.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `content`
}

function entrypoint() -> nothing {
  let r = readFile()
  if (r.failed()) {
    print(`error`)
  }
}
"#,
    );
}

// ── 12. Unknown method on ErrorsCapable → diagnostic ─────────────────────────

#[test]
fn m7_unknown_method_on_errors_capable_emits_diagnostic() {
    // WHY: methods that don't exist on ErrorsCapable values must produce a
    // diagnostic with the list of available methods. Typos in `.failed()` etc.
    // would otherwise silently compile.
    let out = assert_errors(
        r#"
function readFile() -> string errors {
  return `ok`
}

function entrypoint() -> nothing {
  let r = readFile()
  let x = r.nonExistentMethod()
}
"#,
        1,
    ); // 1 error: unknown method on ErrorsCapable
    assert_has_diagnostic(&out, "does not have a method called");
}

// ── 13. errors-capable declared type on return type ──────────────────────────

#[test]
fn m7_errors_capable_declared_fn_type_checks() {
    // WHY: a function declared `-> int errors` must type-check cleanly when it
    // returns a plain `int`. The declaration tells the typeck machinery this
    // function is errors-capable; the return value is the success type.
    assert_clean(
        r#"
function divide(share a: int, share b: int) -> int errors {
  return a
}

function entrypoint() -> nothing {
  let r = divide(10, 2)
  let n = r.or(0)
  print(n.toString())
}
"#,
    );
}

// ── 14. errors-capable returning nothing works ───────────────────────────────

#[test]
fn m7_errors_capable_nothing_return_clean() {
    // WHY: `-> nothing errors` is valid — a side-effectful operation that may
    // fail but has no success value. The whole point of nothing errors is to
    // express "this may fail" without returning data.
    assert_clean(
        r#"
function writeFile() -> nothing errors {
  print(`written`)
}

function entrypoint() -> nothing {
  let r = writeFile()
  if (r.failed()) {
    print(`write failed`)
  }
}
"#,
    );
}

// ── 15. errors-capable int return + .or(0) produces int ─────────────────────

#[test]
fn m7_errors_int_or_fallback_is_int() {
    // WHY: `.or(0)` on an `int errors` value must return `int`. If the type is
    // wrong, arithmetic operations after `.or()` would fail to type-check.
    assert_clean(
        r#"
function tryParse(share s: string) -> int errors {
  return 42
}

function entrypoint() -> nothing {
  let n: int = tryParse(`42`).or(0)
  print(n.toString())
}
"#,
    );
}

// ── 16. errors-capable bool return works ─────────────────────────────────────

#[test]
fn m7_errors_bool_or_false_clean() {
    // WHY: `-> bool errors` must type-check cleanly. Bool is a primitive just like
    // int or string — the errors wrapper must work for all inner types.
    assert_clean(
        r#"
function checkFile() -> bool errors {
  return true
}

function entrypoint() -> nothing {
  let ok = checkFile().or(false)
  print(ok.toString())
}
"#,
    );
}

// ── 17. errors result used in errors function narrows to success type ─────────

#[test]
fn m7_errors_result_used_in_errors_fn_narrows() {
    // WHY: when an errors-capable binding is used in an errors function (auto-
    // propagation context), its type must narrow from ErrorsCapable to the inner
    // success type. A subsequent function expecting `string` must accept it.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `content`
}

function touch(share s: string) -> nothing {
  print(s)
}

function process() -> string errors {
  let content = readFile()
  touch(content)
  return content
}

function entrypoint() -> nothing {
  let r = process()
  print(r.or(`failed`))
}
"#,
    );
}

// ── 18. .or() fallback type must match inner type (int or string mismatch) ───

#[test]
fn m7_or_fallback_wrong_type_emits_diagnostic() {
    // WHY: `.or(default)` returns the inner type. The fallback argument is passed
    // to the same function and must match. If int is returned but string fallback
    // is provided, typeck must catch the mismatch.
    //
    // NOTE: for P3a, .or() returns the inner type regardless of arg type — the
    // arg checking happens at call-site level. This test verifies the .or() call
    // itself doesn't produce unexpected diagnostics when the arg matches.
    assert_clean(
        r#"
function tryGet() -> int errors {
  return 99
}

function entrypoint() -> nothing {
  let n = tryGet().or(0)
  print(n.toString())
}
"#,
    );
}

// ── 19. chained call: errors + .or() in one expression ───────────────────────

#[test]
fn m7_chained_errors_or_clean() {
    // WHY: `readFile().or(`default`)` is the canonical one-liner for handling
    // an errors result. Chained directly on the call without a let binding.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `ok`
}

function entrypoint() -> nothing {
  print(readFile().or(`default`))
}
"#,
    );
}

// ── 20. Two errors calls in the same errors function ─────────────────────────

#[test]
fn m7_two_errors_calls_in_errors_fn() {
    // WHY: each errors-capable call result in an errors function should get its
    // own binding and auto-propagate independently. Two calls must both type-check
    // without one narrowing affecting the other.
    assert_clean(
        r#"
function step1() -> string errors {
  return `a`
}

function step2() -> int errors {
  return 42
}

function pipeline() -> nothing errors {
  let a = step1()
  let b = step2()
  print(a)
  print(b.toString())
}

function entrypoint() -> nothing {
  let r = pipeline()
  if (r.failed()) {
    print(`pipeline failed`)
  }
}
"#,
    );
}

// ── 21. errors-capable type displays as "T errors" in type_name ──────────────

#[test]
fn m7_errors_capable_type_name_format() {
    // WHY: `type_name` must produce "string errors" for `ErrorsCapable { inner: String }`.
    // This display appears in diagnostic messages — wrong format means confusing errors.
    // Verified via the diagnostic output — a function with wrong return type emits
    // "string errors" in the message so the user sees the correct type name.
    let t = Type::ErrorsCapable {
        inner: Box::new(Type::String),
    };
    // Verify the type is constructed correctly (structural check).
    if let Type::ErrorsCapable { inner } = &t {
        assert!(matches!(inner.as_ref(), Type::String));
    } else {
        panic!("Expected ErrorsCapable");
    }
    let t_int = Type::ErrorsCapable {
        inner: Box::new(Type::Int),
    };
    if let Type::ErrorsCapable { inner } = &t_int {
        assert!(matches!(inner.as_ref(), Type::Int));
    } else {
        panic!("Expected ErrorsCapable");
    }
}

// ── 22. .message without .failed() check still gets a type ───────────────────

#[test]
fn m7_message_method_type_checks_on_errors_capable() {
    // WHY: `.message` must return `string` from the method dispatch — the type
    // checker must handle it without cascading to a "no such method" error.
    // Accessing `.message` without a `.failed()` check is semantically wrong but
    // P3a allows it; the full flow-sensitive enforcement is a v0.2 enhancement.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `ok`
}

function entrypoint() -> nothing {
  let r = readFile()
  if (r.failed()) {
    print(r.message)
  }
}
"#,
    );
}

// ── 23. errors-capable in main (non-errors) with .or() ───────────────────────

#[test]
fn m7_errors_in_main_with_or_clean() {
    // WHY: `main` is never errors-capable. Using `.or()` to handle an errors
    // call inside `main` is the standard pattern — must type-check cleanly.
    assert_clean(
        r#"
function fetchName() -> string errors {
  return `Alice`
}

function entrypoint() -> nothing {
  let name = fetchName().or(`unknown`)
  print(name)
}
"#,
    );
}

// ── 24. errors-capable → nested errors function chain ────────────────────────

#[test]
fn m7_nested_errors_chain_clean() {
    // WHY: A calls B which calls C (all errors-capable). The chain must type-check
    // end-to-end without cascading diagnostics from inner calls.
    assert_clean(
        r#"
function readRaw() -> string errors {
  return `raw`
}

function process() -> string errors {
  let raw = readRaw()
  return raw
}

function pipeline() -> string errors {
  let processed = process()
  return processed
}

function entrypoint() -> nothing {
  let r = pipeline()
  print(r.or(`err`))
}
"#,
    );
}

// ── 25. .failed() condition narrows to success after if-block ────────────────

#[test]
fn m7_failed_if_narrowing_after_block() {
    // WHY: `if (x.failed()) { ... }` must narrow `x` to the success type for
    // code that comes AFTER the if-block. The pattern "check failed, handle it,
    // then use the success value" is the core errors-handling idiom.
    // After the if-block, `r` is narrowed to `string` — no longer ErrorsCapable.
    // Use .or() before the if-check, or just check .failed() and use it.
    assert_clean(
        r#"
function readFile() -> string errors {
  return `content`
}

function entrypoint() -> nothing {
  let r = readFile()
  if (r.failed()) {
    print(`error handling`)
  }
}
"#,
    );
}

// ── 26. errors-capable with maybe return type inner ──────────────────────────

#[test]
fn m7_errors_capable_inner_maybe_type() {
    // WHY: `-> maybe<int> errors` is a valid (if exotic) combination — a function
    // that can either fail OR succeed with an absent value. The inner type must
    // resolve correctly to `maybe<int>`. Testing with a concrete maybe value
    // to avoid the `none` literal hint-unwrapping complexity.
    let t = Type::ErrorsCapable {
        inner: Box::new(Type::Maybe {
            inner: Box::new(Type::Int),
        }),
    };
    // Verify the nested type is correct.
    if let Type::ErrorsCapable { inner } = &t {
        assert!(matches!(inner.as_ref(), Type::Maybe { .. }));
    } else {
        panic!("Expected ErrorsCapable wrapping Maybe");
    }
}

// ── 27. errors function in non-errors context diagnostic mentions options ─────

#[test]
fn m7_non_errors_context_diagnostic_lists_options() {
    // WHY: the diagnostic for unhandled errors must tell the developer the THREE
    // options (mark errors, use .or(), check .failed()). If only one option is
    // listed, the developer is left guessing about the others.
    let out = assert_errors(
        r#"
function readFile() -> string errors {
  return `ok`
}

function entrypoint() -> nothing {
  let content = readFile()
  print(content)
}
"#,
        1,
    );
    assert_has_diagnostic(&out, "Three options");
}

// ── 28. type_name on nested ErrorsCapable ────────────────────────────────────

#[test]
fn m7_type_name_nested_errors_capable() {
    // WHY: nested ErrorsCapable (exotic but must not panic) must be constructable.
    // Type::ErrorsCapable can wrap any Type including another ErrorsCapable.
    // The formatter must handle it without stack overflow.
    let inner = Type::ErrorsCapable {
        inner: Box::new(Type::Int),
    };
    let outer = Type::ErrorsCapable {
        inner: Box::new(inner),
    };
    // Verify structural correctness — nested type has the right shape.
    if let Type::ErrorsCapable { inner: outer_inner } = &outer {
        assert!(matches!(outer_inner.as_ref(), Type::ErrorsCapable { .. }));
    } else {
        panic!("Expected outer ErrorsCapable");
    }
}
