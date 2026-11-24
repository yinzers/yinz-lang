// `errors`-capable typeck (M7 P3a): which programs that declare, call, handle or inspect an
// `errors` function the checker accepts or rejects (an unhandled result in a plain function, an
// unknown method, the refused `.suggestions` field). One table of named cases: a case is clean (no
// diagnostic), exactly N errors, or exactly one error of a named class (`[Class]`). Wording is not asserted.

use ynz_diagnostics::{DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, source_file)).clone()
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

fn error_diagnostics(output: &CheckOutput) -> Vec<&ynz_diagnostics::Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .collect()
}

fn assert_error_count(source: &str, expected: usize) {
    let output = run(source);
    let errors = error_diagnostics(&output);
    assert_eq!(
        errors.len(),
        expected,
        "Expected {expected} errors, got {}:\n{errors:#?}",
        errors.len()
    );
}

fn assert_one_error_of_kind(source: &str, expected: DiagnosticKind) {
    let output = run(source);
    let errors = error_diagnostics(&output);
    assert_eq!(errors.len(), 1, "Expected one error, got: {errors:#?}");
    assert_eq!(errors[0].kind, Some(expected), "got: {errors:#?}");
}

macro_rules! errors_cases {
    ($($case:ident: $source:expr => $expected:tt;)+) => {
        $(
            // bug: typeck accepts an `errors` result that no one handles in a plain function (a failure silently swallowed), or refuses a program that declares, calls, `.or()`s, `.failed()`-checks or propagates an `errors` function correctly | tier: High
            #[test]
            fn $case() {
                errors_case!($source, $expected);
            }
        )+
    };
}

macro_rules! errors_case {
    ($source:expr, clean) => {
        assert_clean($source)
    };
    ($source:expr, [$kind:ident]) => {
        assert_one_error_of_kind($source, DiagnosticKind::$kind)
    };
    ($source:expr, $errors:literal) => {
        assert_error_count($source, $errors)
    };
}

errors_cases! {
    an_errors_function_calling_an_errors_function_is_clean:
        r#"
function readFile() -> String errors {
  return `hello`
}

function process() -> String errors {
  let content = readFile()
  return content
}

function entrypoint() -> Nothing {
  let r = process()
  let msg = r.or(`default`)
  print(msg)
}
"#
        => clean;
    a_plain_function_using_an_errors_result_unhandled_is_one_error:
        r#"
function readFile() -> String errors {
  return `hello`
}

function entrypoint() -> Nothing {
  let content = readFile()
  print(content)
}
"#
        => 1;
    a_failed_check_in_a_plain_function_is_clean:
        r#"
function readFile() -> String errors {
  return `ok`
}

function entrypoint() -> Nothing {
  let r = readFile()
  if (r.failed()) {
    print(`failed`)
  }
}
"#
        => clean;
    or_on_an_errors_binding_is_clean:
        r#"
function readFile() -> String errors {
  return `hello`
}

function entrypoint() -> Nothing {
  let r = readFile()
  let content = r.or(`default`)
  print(content)
}
"#
        => clean;
    message_after_a_failed_check_is_clean:
        r#"
function readFile() -> String errors {
  return `hello`
}

function entrypoint() -> Nothing {
  let r = readFile()
  if (r.failed()) {
    print(r.message)
  }
}
"#
        => clean;
    suggestions_after_a_failed_check_is_the_not_yet_available_error:
        r#"
function readFile() -> String errors {
  return `hello`
}

function entrypoint() -> Nothing {
  let r = readFile()
  if (r.failed()) {
    let hints = r.suggestions
    print(hints.count().toString())
  }
}
"#
        => [EcFieldNotYetAvailable];
    an_errors_binding_passed_on_inside_an_errors_function_is_clean:
        r#"
function readFile() -> String errors {
  return `content`
}

function processContent(share content: String) -> Nothing {
  print(content)
}

function process() -> Nothing errors {
  let content = readFile()
  processContent(content)
}

function entrypoint() -> Nothing {
  let r = process()
  if (r.failed()) {
    print(`process failed`)
  }
}
"#
        => clean;
    an_errors_function_calling_a_plain_function_is_clean:
        r#"
function greet() -> String {
  return `hello`
}

function process() -> String errors {
  let msg = greet()
  return msg
}

function entrypoint() -> Nothing {
  let r = process()
  print(r.or(`failed`))
}
"#
        => clean;
    or_directly_on_a_call_in_a_plain_function_is_clean:
        r#"
function readFile() -> String errors {
  return `content`
}

function entrypoint() -> Nothing {
  let content = readFile().or(`default`)
  print(content)
}
"#
        => clean;
    an_unknown_method_on_an_errors_binding_is_one_error:
        r#"
function readFile() -> String errors {
  return `ok`
}

function entrypoint() -> Nothing {
  let r = readFile()
  let x = r.nonExistentMethod()
}
"#
        => 1;
    an_int_errors_function_returning_a_plain_int_is_clean:
        r#"
function divide(share a: Int, share b: Int) -> Int errors {
  return a
}

function entrypoint() -> Nothing {
  let r = divide(10, 2)
  let n = r.or(0)
  print(n.toString())
}
"#
        => clean;
    a_nothing_errors_function_is_clean:
        r#"
function writeFile() -> Nothing errors {
  print(`written`)
}

function entrypoint() -> Nothing {
  let r = writeFile()
  if (r.failed()) {
    print(`write failed`)
  }
}
"#
        => clean;
    or_on_an_int_errors_call_is_an_int:
        r#"
function tryParse(share s: String) -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let n: Int = tryParse(`42`).or(0)
  print(n.toString())
}
"#
        => clean;
    or_directly_on_a_call_argument_is_clean:
        r#"
function readFile() -> String errors {
  return `ok`
}

function entrypoint() -> Nothing {
  print(readFile().or(`default`))
}
"#
        => clean;
    two_errors_calls_in_one_errors_function_are_clean:
        r#"
function step1() -> String errors {
  return `a`
}

function step2() -> Int errors {
  return 42
}

function pipeline() -> Nothing errors {
  let a = step1()
  let b = step2()
  print(a)
  print(b.toString())
}

function entrypoint() -> Nothing {
  let r = pipeline()
  if (r.failed()) {
    print(`pipeline failed`)
  }
}
"#
        => clean;
}
