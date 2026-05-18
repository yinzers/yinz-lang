// WHY: String method typechecking (M7 P3b) must enforce: (1) each string method
// returns the correct type; (2) InterpolatedString accepts primitive types;
// (3) InterpolatedString rejects shapes without a standalone `toString`;
// (4) unknown string methods produce a diagnostic with the available-methods list;
// (5) string bracket access returns maybe<string>.
// Regressions here would silently break codegen which lowers string operations.
//
// test-ratchet: M7 P3b adds string method dispatch and interpolation typechecking.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput, Type};

const FILE: &str = "test.ynz";

fn check(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn check_no_diags(source: &str) -> CheckOutput {
    let out = check(source);
    assert_eq!(
        out.diagnostics.len(),
        0,
        "Expected no diagnostics, got: {:#?}",
        out.diagnostics
    );
    out
}

fn check_diag_count(source: &str, expected: usize) -> CheckOutput {
    let out = check(source);
    let error_count = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .count();
    assert_eq!(
        error_count, expected,
        "Expected {expected} error(s), got: {:#?}",
        out.diagnostics
    );
    out
}

fn check_has_diag(source: &str, fragment: &str) {
    let out = check(source);
    let found = out.diagnostics.iter().any(|d| {
        d.what.contains(fragment) || d.what_instead.contains(fragment) || d.why.contains(fragment)
    });
    assert!(
        found,
        "Expected a diagnostic containing {:?}, got: {:#?}",
        fragment, out.diagnostics
    );
}

// ── 1. Zero-arg string methods return correct types ──────────────────────────

#[test]
fn m7_string_toUpperCase_returns_string() {
    // WHY: .toUpperCase() must return string; if the return type were wrong, any
    // downstream assignment or use would produce a spurious type mismatch.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let t: string = s.toUpperCase()
}"#,
    );
}

#[test]
fn m7_string_toLowerCase_returns_string() {
    // WHY: same as toUpperCase — lowercase transform returns string.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `HELLO`
    let t: string = s.toLowerCase()
}"#,
    );
}

#[test]
fn m7_string_trim_returns_string() {
    // WHY: .trim() is a zero-arg string method returning string. Regression
    // here would break any code that chains or assigns the trimmed value.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `  hello  `
    let t: string = s.trim()
}"#,
    );
}

#[test]
fn m7_string_count_returns_int() {
    // WHY: .count() returns the code-point count as int. Used in loop bounds.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let n: int = s.count()
}"#,
    );
}

#[test]
fn m7_string_byteCount_returns_int() {
    // WHY: .byteCount() returns int. Distinct from .count() (code-point count)
    // and .graphemeCount() (grapheme cluster count). Must not be confused.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `café`
    let n: int = s.byteCount()
}"#,
    );
}

#[test]
fn m7_string_graphemeCount_returns_int() {
    // WHY: .graphemeCount() returns int for grapheme-cluster-aware length.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `café`
    let n: int = s.graphemeCount()
}"#,
    );
}

// ── 2. One-arg string predicate methods return bool ──────────────────────────

#[test]
fn m7_string_contains_returns_bool() {
    // WHY: .contains(substr) returns bool. If it returns something else,
    // conditional expressions using it would get a type mismatch.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello world`
    let b: bool = s.contains(`world`)
}"#,
    );
}

#[test]
fn m7_string_startsWith_returns_bool() {
    // WHY: .startsWith(prefix) returns bool.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let b: bool = s.startsWith(`he`)
}"#,
    );
}

#[test]
fn m7_string_endsWith_returns_bool() {
    // WHY: .endsWith(suffix) returns bool.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let b: bool = s.endsWith(`lo`)
}"#,
    );
}

// ── 3. Indexed access methods return maybe types ─────────────────────────────

#[test]
fn m7_string_indexOf_returns_maybe_int() {
    // WHY: .indexOf() returns maybe<int> because the substring may not be found.
    // If it returned plain int, callers would skip the none-check and crash.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let idx: maybe<int> = s.indexOf(`ll`)
}"#,
    );
}

#[test]
fn m7_string_byteAt_returns_maybe_int() {
    // WHY: .byteAt(n) returns maybe<int> — out-of-bounds returns none.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let b: maybe<int> = s.byteAt(2)
}"#,
    );
}

#[test]
fn m7_string_graphemeAt_returns_maybe_string() {
    // WHY: .graphemeAt(n) returns maybe<string> — grapheme-cluster-aware access.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `café`
    let g: maybe<string> = s.graphemeAt(0)
}"#,
    );
}

#[test]
fn m7_string_get_returns_maybe_string() {
    // WHY: .get(n) returns maybe<string> (one code point). This is also what
    // bracket access s[n] desugars to, so the type must be consistent.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let c: maybe<string> = s.get(0)
}"#,
    );
}

// ── 4. Multi-arg string methods return correct types ─────────────────────────

#[test]
fn m7_string_substring_returns_string() {
    // WHY: .substring(start, end) returns a new string slice.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello world`
    let sub: string = s.substring(0, 5)
}"#,
    );
}

#[test]
fn m7_string_split_returns_array_of_string() {
    // WHY: .split(sep) returns array<string>. If the element type were wrong,
    // any for-loop or map over the split result would produce spurious errors.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `a,b,c`
    let parts: array<string> = s.split(`,`)
}"#,
    );
}

#[test]
fn m7_string_replace_returns_string() {
    // WHY: .replace(old, new) returns the modified string.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello world`
    let r: string = s.replace(`world`, `yinz`)
}"#,
    );
}

// ── 5. Interpolated string typechecking ─────────────────────────────────────

#[test]
fn m7_interpolated_string_with_int_is_clean() {
    // WHY: int is a primitive type — always stringifiable. Must not produce
    // a diagnostic. If it did, every `print(`count is ${n}`)` would fail.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let n = 42
    let s = `count is ${n}`
}"#,
    );
}

#[test]
fn m7_interpolated_string_with_bool_is_clean() {
    // WHY: bool is primitive — always stringifiable.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let b = true
    let s = `active: ${b}`
}"#,
    );
}

#[test]
fn m7_interpolated_string_with_float_is_clean() {
    // WHY: float is primitive — always stringifiable.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let f: float = 3.14
    let s = `pi is ${f}`
}"#,
    );
}

#[test]
fn m7_interpolated_string_with_string_is_clean() {
    // WHY: string inside interpolation is trivially stringifiable.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let name = `world`
    let s = `hello ${name}`
}"#,
    );
}

#[test]
fn m7_interpolated_string_with_shape_with_toString_is_clean() {
    // WHY: a shape with a standalone toString function must be accepted in
    // interpolation. If it's rejected, user shapes would be un-usable in templates.
    check_no_diags(
        r#"shape Player {
    name: string
}

function toString(share self: Player) -> string {
    return self.name
}

function entrypoint() -> nothing {
    let p: Player = { name: `Patrick` }
    let s = `player: ${p}`
}"#,
    );
}

#[test]
fn m7_interpolated_string_with_shape_without_toString_produces_error() {
    // WHY: a shape without toString must be rejected in interpolation with a
    // clear "add a toString function" message. Without this check, codegen
    // would try to call a non-existent function at runtime.
    check_diag_count(
        r#"shape Enemy {
    name: string
}

function entrypoint() -> nothing {
    let e: Enemy = { name: `Goblin` }
    let s = `enemy: ${e}`
}"#,
        1,
    );
}

#[test]
fn m7_interpolated_string_error_message_mentions_toString() {
    // WHY: the diagnostic message must guide the user to add the toString
    // function — without this, they have no hint on how to fix the error.
    check_has_diag(
        r#"shape Enemy {
    name: string
}

function entrypoint() -> nothing {
    let e: Enemy = { name: `Goblin` }
    let s = `enemy: ${e}`
}"#,
        "toString",
    );
}

// ── 6. Unknown string method produces diagnostic ─────────────────────────────

#[test]
fn m7_string_unknown_method_produces_error() {
    // WHY: calling a non-existent method on string must produce a compile error.
    // Without this, the codegen would try to emit a call to a non-existent function.
    check_diag_count(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let x = s.nonExistent()
}"#,
        1,
    );
}

#[test]
fn m7_string_unknown_method_mentions_available_methods() {
    // WHY: the error message must list available string methods so the user
    // can find the right one. Without this, they'd have to read the spec.
    check_has_diag(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let x = s.nonExistent()
}"#,
        "contains",
    );
}

// ── 7. Bracket access desugars to .get() → maybe<string> ────────────────────

#[test]
fn m7_string_bracket_access_returns_maybe_string() {
    // WHY: s[n] desugars to s.get(n) which returns maybe<string>. If this
    // were not checked, the type would be wrong for any downstream use.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `hello`
    let c: maybe<string> = s[0]
}"#,
    );
}

// ── 8. Existing M6 string conversions still work ─────────────────────────────

#[test]
fn m7_string_toInt_still_works() {
    // WHY: .toInt() was added in M6 and must still typecheck correctly alongside
    // the new M7 string methods. Non-regression guard.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `42`
    let n: maybe<int> = s.toInt()
}"#,
    );
}

#[test]
fn m7_string_toFloat_still_works() {
    // WHY: .toFloat() was added in M6 — same non-regression guard.
    check_no_diags(
        r#"function entrypoint() -> nothing {
    let s = `3.14`
    let f: maybe<float> = s.toFloat()
}"#,
    );
}
