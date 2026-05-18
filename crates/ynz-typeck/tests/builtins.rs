// WHY: P3b adds built-in collection types (array<T>, fixed<T>, maybe<T>) and
// ArrayLit parsing. These tests pin the typeck behavior for every acceptance
// criterion: type inference, method dispatch, bracket access desugar,
// flow-sensitive `.value`, for-loop element binding, and error paths.
// Regressions here would silently break P4a codegen which lowers these types.
//
// test-ratchet: M7 P1 — all double-quoted string literals in Yinz source strings
// replaced with backtick strings. Double-quotes now produce an error diagnostic.

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
    assert_eq!(
        out.diagnostics.len(),
        expected,
        "Expected {expected} diagnostic(s), got: {:#?}",
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

// ── array<T> type-checks ─────────────────────────────────────────────────────

#[test]
fn m5p3b_array_int_literal_typechecks() {
    // WHY: acceptance criterion — `let arr: array<int> = [1, 2, 3]` must type-check
    // without diagnostics. If array<T> annotation → ArrayLit inference is broken,
    // all collection code fails.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
}
"#);
}

#[test]
fn m5p3b_array_string_literal_typechecks() {
    // WHY: array<string> must work the same as array<int>.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<string> = [`hello`, `world`]
}
"#);
}

#[test]
fn m5p3b_array_bool_literal_typechecks() {
    // WHY: array<bool> must work.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<bool> = [true, false, true]
}
"#);
}

#[test]
fn m5p3b_array_empty_with_annotation_typechecks() {
    // WHY: `let arr: array<int> = []` must be valid — empty arrays are legal.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = []
}
"#);
}

#[test]
fn m5p3b_array_empty_without_annotation_produces_error() {
    // WHY: `let arr = []` without annotation has no type information — must error.
    check_diag_count(r#"
function main() -> nothing {
    let arr = []
}
"#, 1);
}

#[test]
fn m5p3b_array_mismatched_element_types_errors() {
    // WHY: `[1, `hello`]` is a type mismatch — element 2 is string, array is int.
    // Without this check, mistyped arrays compile silently.
    check_diag_count(r#"
function main() -> nothing {
    let arr: array<int> = [1, `hello`]
}
"#, 1);
}

// ── array<T> method calls ────────────────────────────────────────────────────

#[test]
fn m5p3b_array_count_returns_int() {
    // WHY: `.count()` on array<T> must return int.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let n: int = arr.count()
}
"#);
}

#[test]
fn m5p3b_array_contains_returns_bool() {
    // WHY: `.contains()` on array<int> must return bool.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let found: bool = arr.contains(2)
}
"#);
}

#[test]
fn m5p3b_array_get_returns_maybe() {
    // WHY: `.get()` on array<int> must return maybe<int>, not int.
    // This is the core safety guarantee — bracket access can be out-of-bounds.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let m: maybe<int> = arr.get(0)
}
"#);
}

#[test]
fn m5p3b_array_first_returns_maybe() {
    // WHY: `.first()` on array<int> must return maybe<int>.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let m: maybe<int> = arr.first()
}
"#);
}

#[test]
fn m5p3b_array_sort_returns_array() {
    // WHY: `.sort()` on array<int> must return array<int>.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [3, 1, 2]
    let sorted: array<int> = arr.sort()
}
"#);
}

#[test]
fn m5p3b_array_unknown_method_errors() {
    // WHY: calling `.frobnicate()` on array<int> must produce a diagnostic.
    // Without method validation, typos silently produce Error types.
    check_diag_count(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr.frobnicate()
}
"#, 1);
}

#[test]
fn m5p3b_array_unknown_method_error_has_correct_message() {
    // WHY: the error message must mention `array<int>` and the unknown method name.
    check_has_diag(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr.frobnicate()
}
"#, "array<int>");
}

// ── array<T> bracket access ──────────────────────────────────────────────────

#[test]
fn m5p3b_array_bracket_access_returns_maybe() {
    // WHY: acceptance criterion — `arr[0]` on array<int> must produce maybe<int>.
    // Bracket access desugars to `.get(index)` which is always maybe<T>.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let m: maybe<int> = arr[0]
}
"#);
}

#[test]
fn m5p3b_array_bracket_assign_typechecks() {
    // WHY: acceptance criterion — `arr[0] = 5` on array<int> must type-check.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr[0] = 5
}
"#);
}

#[test]
fn m5p3b_array_bracket_assign_wrong_type_errors() {
    // WHY: `arr[0] = `hello`` on array<int> must error — wrong element type.
    check_diag_count(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr[0] = `hello`
}
"#, 1);
}

// ── fixed<T> type-checks ─────────────────────────────────────────────────────

#[test]
fn m5p3b_fixed_int_literal_typechecks() {
    // WHY: acceptance criterion — `let f: fixed<string> = [`a`, `b`, `c`]` must
    // type-check. Fixed literals use the same ArrayLit AST node as array.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<string> = [`a`, `b`, `c`]
}
"#);
}

#[test]
fn m5p3b_fixed_empty_with_annotation_typechecks() {
    // WHY: empty fixed array with annotation is valid.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = []
}
"#);
}

#[test]
fn m5p3b_fixed_mismatched_element_types_errors() {
    // WHY: element type mismatch in fixed literal must error.
    check_diag_count(r#"
function main() -> nothing {
    let f: fixed<int> = [1, `bad`]
}
"#, 1);
}

// ── fixed<T> method calls ────────────────────────────────────────────────────

#[test]
fn m5p3b_fixed_count_returns_int() {
    // WHY: `.count()` on fixed<T> must return int.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    let n: int = f.count()
}
"#);
}

#[test]
fn m5p3b_fixed_get_returns_maybe() {
    // WHY: `.get()` on fixed<int> must return maybe<int>.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    let m: maybe<int> = f.get(0)
}
"#);
}

#[test]
fn m5p3b_fixed_contains_returns_bool() {
    // WHY: `.contains()` on fixed<int> must return bool.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    let found: bool = f.contains(2)
}
"#);
}

#[test]
fn m5p3b_fixed_add_method_errors() {
    // WHY: `fixed<T>` does not have `.add()` — it's size-locked. The error message
    // must distinguish fixed from array and explain the limitation.
    check_diag_count(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    f.add(4)
}
"#, 1);
}

#[test]
fn m5p3b_fixed_add_error_message_mentions_array() {
    // WHY: the `.add()` error on fixed must hint to use array<T> instead.
    check_has_diag(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    f.add(4)
}
"#, "array<T>");
}

#[test]
fn m5p3b_fixed_unknown_method_errors() {
    // WHY: unknown method on fixed must produce exactly one diagnostic.
    check_diag_count(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2]
    f.badMethod()
}
"#, 1);
}

// ── fixed<T> bracket access ──────────────────────────────────────────────────

#[test]
fn m5p3b_fixed_bracket_access_returns_maybe() {
    // WHY: `f[0]` on fixed<int> must produce maybe<int>.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    let m: maybe<int> = f[0]
}
"#);
}

#[test]
fn m5p3b_fixed_bracket_assign_typechecks() {
    // WHY: `f[0] = 5` on fixed<int> must type-check.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    f[0] = 5
}
"#);
}

#[test]
fn m5p3b_fixed_bracket_assign_wrong_type_errors() {
    // WHY: `f[0] = "bad"` on fixed<int> must error.
    check_diag_count(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2, 3]
    f[0] = `bad`
}
"#, 1);
}

// ── maybe<T> type-checks ─────────────────────────────────────────────────────

#[test]
fn m5p3b_maybe_none_literal_typechecks() {
    // WHY: acceptance criterion — `let m: maybe<int> = none` must type-check.
    // This is the fundamental use of the none literal.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<int> = none
}
"#);
}

#[test]
fn m5p3b_maybe_none_without_annotation_errors() {
    // WHY: `let m = none` without annotation has no type to infer — must error.
    check_diag_count(r#"
function main() -> nothing {
    let m = none
}
"#, 1);
}

#[test]
fn m5p3b_none_in_wrong_context_errors() {
    // WHY: `let m: int = none` — none cannot be an int — must error.
    check_diag_count(r#"
function main() -> nothing {
    let m: int = none
}
"#, 1);
}

// ── maybe<T> methods ─────────────────────────────────────────────────────────

#[test]
fn m5p3b_maybe_exists_returns_bool() {
    // WHY: `.exists()` on maybe<int> must return bool. This is the gate for
    // flow-sensitive `.value` access.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let b: bool = m.exists()
}
"#);
}

#[test]
fn m5p3b_maybe_or_returns_inner_type() {
    // WHY: acceptance criterion — `m.or(0)` on maybe<int> must produce int.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let n: int = m.or(0)
}
"#);
}

#[test]
fn m5p3b_maybe_or_returns_inner_type_string() {
    // WHY: `.or()` must return the inner type across different element types.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<string> = none
    let s: string = m.or(`default`)
}
"#);
}

#[test]
fn m5p3b_maybe_unknown_method_errors() {
    // WHY: unknown method on maybe<T> must produce a diagnostic.
    check_diag_count(r#"
function main() -> nothing {
    let m: maybe<int> = none
    m.badMethod()
}
"#, 1);
}

// ── maybe<T>.value flow-sensitive access ─────────────────────────────────────

#[test]
fn m5p3b_maybe_value_without_guard_errors() {
    // WHY: acceptance criterion — `m.value` without a prior `.exists()` check
    // must produce a compile error. This is the core safety guarantee of maybe<T>.
    check_diag_count(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let n: int = m.value
}
"#, 1);
}

#[test]
fn m5p3b_maybe_value_without_guard_error_message() {
    // WHY: the error must explain that `.exists()` is required.
    check_has_diag(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let n: int = m.value
}
"#, "exists");
}

#[test]
fn m5p3b_maybe_value_with_exists_guard_typechecks() {
    // WHY: acceptance criterion — `if (m.exists()) { print(m.value) }` must
    // type-check without diagnostics. Flow-sensitive narrowing enables .value.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<int> = none
    if (m.exists()) {
        let n: int = m.value
    }
}
"#);
}

#[test]
fn m5p3b_maybe_value_type_is_inner() {
    // WHY: inside the `.exists()` guard, `m.value` must type-check as int (not maybe<int>).
    // Using it where int is expected must succeed.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<int> = none
    if (m.exists()) {
        let n: int = m.value
        let doubled: int = n + 1
    }
}
"#);
}

#[test]
fn m5p3b_maybe_value_outside_guard_still_errors() {
    // WHY: narrowing must be scoped to the if body. After the block exits, `.value`
    // should require a new guard.
    check_diag_count(r#"
function main() -> nothing {
    let m: maybe<int> = none
    if (m.exists()) {
        let n: int = m.value
    }
    let outside: int = m.value
}
"#, 1);
}

#[test]
fn m5p3b_maybe_bad_field_access_errors() {
    // WHY: `m.foo` on maybe<int> (where foo is not `value`) must error with a
    // helpful message pointing to `.value`, `.exists()`, and `.or()`.
    check_diag_count(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let x = m.foo
}
"#, 1);
}

#[test]
fn m5p3b_maybe_bad_field_message_suggests_value() {
    // WHY: the error must mention .value as the correct field name.
    check_has_diag(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let x = m.badField
}
"#, ".value");
}

// ── for-loop iteration over collections ──────────────────────────────────────

#[test]
fn m5p3b_for_loop_over_array_typechecks() {
    // WHY: acceptance criterion — `for (x in arr)` where arr is array<int>
    // must bind x as int and type-check without diagnostics.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    for (x in arr) {
        let n: int = x
    }
}
"#);
}

#[test]
fn m5p3b_for_loop_over_fixed_typechecks() {
    // WHY: acceptance criterion — `for (x in f)` where f is fixed<string>
    // must bind x as string.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<string> = [`a`, `b`, `c`]
    for (x in f) {
        let s: string = x
    }
}
"#);
}

#[test]
fn m5p3b_for_loop_over_array_wrong_elem_type_errors() {
    // WHY: `let n: string = x` where x is int (from array<int>) must error.
    // The loop variable must have the correct element type.
    check_diag_count(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    for (x in arr) {
        let s: string = x
    }
}
"#, 1);
}

// ── bare type name without args errors ───────────────────────────────────────

#[test]
fn m5p3b_bare_array_type_without_arg_errors() {
    // WHY: `let arr: array = []` — bare `array` without type arg must error with
    // a message pointing to `array<T>`.
    check_diag_count(r#"
function main() -> nothing {
    let arr: array = []
}
"#, 1);
}

#[test]
fn m5p3b_bare_fixed_type_without_arg_errors() {
    // WHY: `let f: fixed = []` — bare `fixed` without type arg must error.
    check_diag_count(r#"
function main() -> nothing {
    let f: fixed = []
}
"#, 1);
}

#[test]
fn m5p3b_bare_maybe_type_without_arg_errors() {
    // WHY: `let m: maybe = none` — bare `maybe` without type arg must error.
    check_diag_count(r#"
function main() -> nothing {
    let m: maybe = none
}
"#, 1);
}

#[test]
fn m5p3b_bare_array_error_mentions_type_arg() {
    // WHY: the error must tell the user to write `array<T>`.
    check_has_diag(r#"
function main() -> nothing {
    let arr: array = []
}
"#, "type argument");
}

// ── bracket access on non-collection types errors ────────────────────────────

#[test]
fn m5p3b_bracket_access_on_int_errors() {
    // WHY: `x[0]` where x is int must produce a diagnostic explaining bracket
    // access is only for collections.
    check_diag_count(r#"
function main() -> nothing {
    let x: int = 42
    let y = x[0]
}
"#, 1);
}

#[test]
fn m5p3b_bracket_assign_on_non_collection_errors() {
    // WHY: `x[0] = 5` where x is bool must error.
    check_diag_count(r#"
function main() -> nothing {
    let x: bool = true
    x[0] = 5
}
"#, 1);
}

// ── array/fixed type-name appears in diagnostics ─────────────────────────────

#[test]
fn m5p3b_array_type_name_in_diagnostic() {
    // WHY: `type_name` for BuiltinArray must render as "array<int>", not some
    // internal representation. Diagnostics must use user-facing Yinz terms.
    check_has_diag(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr.badMethod()
}
"#, "array<int>");
}

#[test]
fn m5p3b_fixed_type_name_in_diagnostic() {
    // WHY: `type_name` for BuiltinFixed must render as "fixed<int>".
    check_has_diag(r#"
function main() -> nothing {
    let f: fixed<int> = [1, 2]
    f.badMethod()
}
"#, "fixed<int>");
}

#[test]
fn m5p3b_maybe_type_name_in_diagnostic() {
    // WHY: `type_name` for Maybe must render as "maybe<int>".
    check_has_diag(r#"
function main() -> nothing {
    let m: maybe<int> = none
    m.badMethod()
}
"#, "maybe<int>");
}

// ── combined acceptance criteria ─────────────────────────────────────────────

#[test]
fn m5p3b_acceptance_array_add_method_typechecks() {
    // WHY: acceptance criterion — `arr.add(4)` on array<int> must type-check.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr.add(4)
}
"#);
}

#[test]
fn m5p3b_acceptance_maybe_or_produces_int() {
    // WHY: acceptance criterion — `m.or(0)` produces int, usable as int.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<int> = none
    let n: int = m.or(0)
    let doubled: int = n + 1
}
"#);
}

#[test]
fn m5p3b_acceptance_array_bracket_desugar() {
    // WHY: acceptance criterion — `arr[0]` produces `maybe<int>`, matching
    // the `.get(index)` return type.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let m: maybe<int> = arr[0]
    if (m.exists()) {
        let n: int = m.value
    }
}
"#);
}

#[test]
fn m5p3b_acceptance_full_maybe_workflow() {
    // WHY: end-to-end test of the complete maybe<T> workflow:
    // create, check exists, access .value, use .or() — all in one function.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [10, 20, 30]
    let m: maybe<int> = arr.get(0)
    let safe: int = m.or(0)
    if (m.exists()) {
        let direct: int = m.value
    }
}
"#);
}

#[test]
fn m5p3b_acceptance_fixed_for_loop() {
    // WHY: end-to-end test of iterating over a fixed<int> — must compile
    // and bind loop variable as int.
    check_no_diags(r#"
function main() -> nothing {
    let scores: fixed<int> = [100, 200, 300]
    for (score in scores) {
        let doubled: int = score + score
    }
}
"#);
}

#[test]
fn m5p3b_acceptance_array_for_loop() {
    // WHY: end-to-end test of iterating over an array<int>.
    check_no_diags(r#"
function main() -> nothing {
    let nums: array<int> = [1, 2, 3]
    for (n in nums) {
        let x: int = n
    }
}
"#);
}

#[test]
fn m5p3b_array_single_element_inferred() {
    // WHY: single-element array literal `[42]` with annotation must type-check.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [42]
}
"#);
}

#[test]
fn m5p3b_fixed_sort_returns_fixed() {
    // WHY: `.sort()` on fixed<int> must return fixed<int>, not array<int>.
    check_no_diags(r#"
function main() -> nothing {
    let f: fixed<int> = [3, 1, 2]
    let sorted: fixed<int> = f.sort()
}
"#);
}

#[test]
fn m5p3b_array_freeze_is_postfix_op() {
    // WHY: `.freeze()` is parsed as PostfixOp (not a method call) and returns nothing
    // per the dot-postfix rule — it's a side-effecting body operation, not a
    // transformation. This pins the current behavior; full freeze semantics land in P3c.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    arr.freeze()
}
"#);
}

#[test]
fn m5p3b_maybe_string_exists_guard_works() {
    // WHY: flow-sensitive narrowing must work for maybe<string> as well as maybe<int>.
    check_no_diags(r#"
function main() -> nothing {
    let m: maybe<string> = none
    if (m.exists()) {
        let s: string = m.value
    }
}
"#);
}

#[test]
fn m5p3b_array_last_returns_maybe() {
    // WHY: `.last()` on array<string> must return maybe<string>.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<string> = [`a`, `b`]
    let m: maybe<string> = arr.last()
}
"#);
}

#[test]
fn m5p3b_array_filter_returns_array() {
    // WHY: `.filter()` on array<int> must return array<int>.
    check_no_diags(r#"
function main() -> nothing {
    let arr: array<int> = [1, 2, 3]
    let filtered: array<int> = arr.filter()
}
"#);
}
