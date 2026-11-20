// WHY: P3b adds built-in collection types (Array<T>, Fixed<T>, Maybe<T>) and
// ArrayLit parsing. These tests pin the typeck behavior for every acceptance
// criterion: type inference, method dispatch, bracket access desugar,
// flow-sensitive `.value`, for-loop element binding, and error paths.
// Regressions here would silently break P4a codegen which lowers these types.
//
// test-ratchet: M7 P1 — all double-quoted string literals in Yinz source strings
// replaced with backtick strings. Double-quotes now produce an error diagnostic.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

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

// ── Array<T> type-checks ─────────────────────────────────────────────────────

#[test]
fn m5p3b_array_int_literal_typechecks() {
    // WHY: acceptance criterion — `let arr: Array<Int> = [1, 2, 3]` must type-check
    // without diagnostics. If Array<T> annotation → ArrayLit inference is broken,
    // all collection code fails.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
}
"#,
    );
}

#[test]
fn m5p3b_array_string_literal_typechecks() {
    // WHY: Array<String> must work the same as Array<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<String> = [`hello`, `world`]
}
"#,
    );
}

#[test]
fn m5p3b_array_bool_literal_typechecks() {
    // WHY: Array<Boolean> must work.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Boolean> = [true, false, true]
}
"#,
    );
}

#[test]
fn m5p3b_array_empty_with_annotation_typechecks() {
    // WHY: `let arr: Array<Int> = []` must be valid — empty arrays are legal.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = []
}
"#,
    );
}

#[test]
fn m5p3b_array_empty_without_annotation_produces_error() {
    // WHY: `let arr = []` without annotation has no type information — must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let arr = []
}
"#,
        1,
    );
}

#[test]
fn m5p3b_array_mismatched_element_types_errors() {
    // WHY: `[1, `hello`]` is a type mismatch — element 2 is string, array is int.
    // Without this check, mistyped arrays compile silently.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, `hello`]
}
"#,
        1,
    );
}

// ── Array<T> method calls ────────────────────────────────────────────────────

#[test]
fn m5p3b_array_count_returns_int() {
    // WHY: `.count()` on Array<T> must return int.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let n: Int = arr.count()
}
"#,
    );
}

#[test]
fn m5p3b_array_contains_returns_bool() {
    // WHY: `.contains()` on Array<Int> must return bool.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let found: Boolean = arr.contains(2)
}
"#,
    );
}

#[test]
fn m5p3b_array_get_returns_maybe() {
    // WHY: `.get()` on Array<Int> must return Maybe<Int>, not int.
    // This is the core safety guarantee — bracket access can be out-of-bounds.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr.get(0)
}
"#,
    );
}

#[test]
fn m5p3b_array_first_returns_maybe() {
    // WHY: `.first()` on Array<Int> must return Maybe<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr.first()
}
"#,
    );
}

#[test]
fn m5p3b_array_sort_returns_array() {
    // WHY: `.sort()` on Array<Int> must return Array<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [3, 1, 2]
    let sorted: Array<Int> = arr.sort()
}
"#,
    );
}

#[test]
fn m5p3b_array_unknown_method_errors() {
    // WHY: calling `.frobnicate()` on Array<Int> must produce a diagnostic.
    // Without method validation, typos silently produce Error types.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.frobnicate()
}
"#,
        1,
    );
}

#[test]
fn m5p3b_array_unknown_method_error_has_correct_message() {
    // WHY: the error message must mention `Array<Int>` and the unknown method name.
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.frobnicate()
}
"#,
        "Array<Int>",
    );
}

// ── Array<T> bracket access ──────────────────────────────────────────────────

#[test]
fn m5p3b_array_bracket_access_returns_maybe() {
    // WHY: acceptance criterion — `arr[0]` on Array<Int> must produce Maybe<Int>.
    // Bracket access desugars to `.get(index)` which is always Maybe<T>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr[0]
}
"#,
    );
}

#[test]
fn m5p3b_array_bracket_assign_typechecks() {
    // WHY: acceptance criterion — `arr[0] = 5` on Array<Int> must type-check.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr[0] = 5
}
"#,
    );
}

#[test]
fn m5p3b_array_bracket_assign_wrong_type_errors() {
    // WHY: `arr[0] = `hello`` on Array<Int> must error — wrong element type.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr[0] = `hello`
}
"#,
        1,
    );
}

// ── Fixed<T> type-checks ─────────────────────────────────────────────────────

#[test]
fn m5p3b_fixed_int_literal_typechecks() {
    // WHY: acceptance criterion — `let f: Fixed<String> = [`a`, `b`, `c`]` must
    // type-check. Fixed literals use the same ArrayLit AST node as array.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<String> = [`a`, `b`, `c`]
}
"#,
    );
}

#[test]
fn m5p3b_fixed_empty_with_annotation_typechecks() {
    // WHY: empty fixed array with annotation is valid.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = []
}
"#,
    );
}

#[test]
fn m5p3b_fixed_mismatched_element_types_errors() {
    // WHY: element type mismatch in fixed literal must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, `bad`]
}
"#,
        1,
    );
}

// ── Fixed<T> method calls ────────────────────────────────────────────────────

#[test]
fn m5p3b_fixed_count_returns_int() {
    // WHY: `.count()` on Fixed<T> must return int.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let n: Int = f.count()
}
"#,
    );
}

#[test]
fn m5p3b_fixed_get_returns_maybe() {
    // WHY: `.get()` on Fixed<Int> must return Maybe<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let m: Maybe<Int> = f.get(0)
}
"#,
    );
}

#[test]
fn m5p3b_fixed_contains_returns_bool() {
    // WHY: `.contains()` on Fixed<Int> must return bool.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let found: Boolean = f.contains(2)
}
"#,
    );
}

#[test]
fn m5p3b_fixed_add_method_errors() {
    // WHY: `Fixed<T>` does not have `.add()` — it's size-locked. The error message
    // must distinguish fixed from array and explain the limitation.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f.add(4)
}
"#,
        1,
    );
}

#[test]
fn m5p3b_fixed_add_error_message_mentions_array() {
    // WHY: the `.add()` error on fixed must hint to use Array<T> instead.
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f.add(4)
}
"#,
        "Array<T>",
    );
}

#[test]
fn m5p3b_fixed_unknown_method_errors() {
    // WHY: unknown method on fixed must produce exactly one diagnostic.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2]
    f.badMethod()
}
"#,
        1,
    );
}

// ── Fixed<T> bracket access ──────────────────────────────────────────────────

#[test]
fn m5p3b_fixed_bracket_access_returns_maybe() {
    // WHY: `f[0]` on Fixed<Int> must produce Maybe<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    let m: Maybe<Int> = f[0]
}
"#,
    );
}

#[test]
fn m5p3b_fixed_bracket_assign_typechecks() {
    // WHY: `f[0] = 5` on Fixed<Int> must type-check.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f[0] = 5
}
"#,
    );
}

#[test]
fn m5p3b_fixed_bracket_assign_wrong_type_errors() {
    // WHY: `f[0] = "bad"` on Fixed<Int> must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2, 3]
    f[0] = `bad`
}
"#,
        1,
    );
}

// ── Maybe<T> type-checks ─────────────────────────────────────────────────────

#[test]
fn m5p3b_maybe_none_literal_typechecks() {
    // WHY: acceptance criterion — `let m: Maybe<Int> = none` must type-check.
    // This is the fundamental use of the none literal.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
}
"#,
    );
}

#[test]
fn m5p3b_maybe_none_without_annotation_errors() {
    // WHY: `let m = none` without annotation has no type to infer — must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m = none
}
"#,
        1,
    );
}

#[test]
fn m5p3b_none_in_wrong_context_errors() {
    // WHY: `let m: Int = none` — none cannot be an int — must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m: Int = none
}
"#,
        1,
    );
}

// ── Maybe<T> methods ─────────────────────────────────────────────────────────

#[test]
fn m5p3b_maybe_exists_returns_bool() {
    // WHY: `.exists()` on Maybe<Int> must return bool. This is the gate for
    // flow-sensitive `.value` access.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let b: Boolean = m.exists()
}
"#,
    );
}

#[test]
fn m5p3b_maybe_or_returns_inner_type() {
    // WHY: acceptance criterion — `m.or(0)` on Maybe<Int> must produce int.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let n: Int = m.or(0)
}
"#,
    );
}

#[test]
fn m5p3b_maybe_or_returns_inner_type_string() {
    // WHY: `.or()` must return the inner type across different element types.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<String> = none
    let s: String = m.or(`default`)
}
"#,
    );
}

#[test]
fn m5p3b_maybe_unknown_method_errors() {
    // WHY: unknown method on Maybe<T> must produce a diagnostic.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    m.badMethod()
}
"#,
        1,
    );
}

// ── Maybe<T>.value flow-sensitive access ─────────────────────────────────────

#[test]
fn m5p3b_maybe_value_without_guard_errors() {
    // WHY: acceptance criterion — `m.value` without a prior `.exists()` check
    // must produce a compile error. This is the core safety guarantee of Maybe<T>.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let n: Int = m.value
}
"#,
        1,
    );
}

#[test]
fn m5p3b_maybe_value_without_guard_error_message() {
    // WHY: the error must explain that `.exists()` is required.
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let n: Int = m.value
}
"#,
        "exists",
    );
}

#[test]
fn m5p3b_maybe_value_with_exists_guard_typechecks() {
    // WHY: acceptance criterion — `if (m.exists()) { print(m.value) }` must
    // type-check without diagnostics. Flow-sensitive narrowing enables .value.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    if (m.exists()) {
        let n: Int = m.value
    }
}
"#,
    );
}

#[test]
fn m5p3b_maybe_value_type_is_inner() {
    // WHY: inside the `.exists()` guard, `m.value` must type-check as int (not Maybe<Int>).
    // Using it where int is expected must succeed.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    if (m.exists()) {
        let n: Int = m.value
        let doubled: Int = n + 1
    }
}
"#,
    );
}

#[test]
fn m5p3b_maybe_value_outside_guard_still_errors() {
    // WHY: narrowing must be scoped to the if body. After the block exits, `.value`
    // should require a new guard.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    if (m.exists()) {
        let n: Int = m.value
    }
    let outside: Int = m.value
}
"#,
        1,
    );
}

#[test]
fn m5p3b_maybe_bad_field_access_errors() {
    // WHY: `m.foo` on Maybe<Int> (where foo is not `value`) must error with a
    // helpful message pointing to `.value`, `.exists()`, and `.or()`.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let x = m.foo
}
"#,
        1,
    );
}

#[test]
fn m5p3b_maybe_bad_field_message_suggests_value() {
    // WHY: the error must mention .value as the correct field name.
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let x = m.badField
}
"#,
        ".value",
    );
}

// ── for-loop iteration over collections ──────────────────────────────────────

#[test]
fn m5p3b_for_loop_over_array_typechecks() {
    // WHY: acceptance criterion — `for (x in arr)` where arr is Array<Int>
    // must bind x as int and type-check without diagnostics.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    for (x in arr) {
        let n: Int = x
    }
}
"#,
    );
}

#[test]
fn m5p3b_for_loop_over_fixed_typechecks() {
    // WHY: acceptance criterion — `for (x in f)` where f is Fixed<String>
    // must bind x as string.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<String> = [`a`, `b`, `c`]
    for (x in f) {
        let s: String = x
    }
}
"#,
    );
}

#[test]
fn m5p3b_for_loop_over_array_wrong_elem_type_errors() {
    // WHY: `let n: String = x` where x is int (from Array<Int>) must error.
    // The loop variable must have the correct element type.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    for (x in arr) {
        let s: String = x
    }
}
"#,
        1,
    );
}

// ── bare type name without args errors ───────────────────────────────────────

#[test]
fn m5p3b_bare_array_type_without_arg_errors() {
    // WHY: `let arr: Array = []` — bare `Array` without type arg must error with
    // a message pointing to `Array<T>`.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let arr: Array = []
}
"#,
        1,
    );
}

#[test]
fn m5p3b_bare_fixed_type_without_arg_errors() {
    // WHY: `let f: Fixed = []` — bare `Fixed` without type arg must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed = []
}
"#,
        1,
    );
}

#[test]
fn m5p3b_bare_maybe_type_without_arg_errors() {
    // WHY: `let m: Maybe = none` — bare `Maybe` without type arg must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe = none
}
"#,
        1,
    );
}

#[test]
fn m5p3b_bare_array_error_mentions_type_arg() {
    // WHY: the error must tell the user to write `Array<T>`.
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let arr: Array = []
}
"#,
        "type argument",
    );
}

// ── bracket access on non-collection types errors ────────────────────────────

#[test]
fn m5p3b_bracket_access_on_int_errors() {
    // WHY: `x[0]` where x is int must produce a diagnostic explaining bracket
    // access is only for collections.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let x: Int = 42
    let y = x[0]
}
"#,
        1,
    );
}

#[test]
fn m5p3b_bracket_assign_on_non_collection_errors() {
    // WHY: `x[0] = 5` where x is bool must error.
    check_diag_count(
        r#"
function entrypoint() -> Nothing {
    let x: Boolean = true
    x[0] = 5
}
"#,
        1,
    );
}

// ── array/fixed type-name appears in diagnostics ─────────────────────────────

#[test]
fn m5p3b_array_type_name_in_diagnostic() {
    // WHY: `type_name` for BuiltinArray must render as "Array<Int>", not some
    // internal representation. Diagnostics must use user-facing Yinz terms.
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.badMethod()
}
"#,
        "Array<Int>",
    );
}

#[test]
fn m5p3b_fixed_type_name_in_diagnostic() {
    // WHY: `type_name` for BuiltinFixed must render as "Fixed<Int>".
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [1, 2]
    f.badMethod()
}
"#,
        "Fixed<Int>",
    );
}

#[test]
fn m5p3b_maybe_type_name_in_diagnostic() {
    // WHY: `type_name` for Maybe must render as "Maybe<Int>".
    check_has_diag(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    m.badMethod()
}
"#,
        "Maybe<Int>",
    );
}

// ── combined acceptance criteria ─────────────────────────────────────────────

#[test]
fn m5p3b_acceptance_array_add_method_typechecks() {
    // WHY: acceptance criterion — `arr.add(4)` on Array<Int> must type-check.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.add(4)
}
"#,
    );
}

#[test]
fn m5p3b_acceptance_maybe_or_produces_int() {
    // WHY: acceptance criterion — `m.or(0)` produces int, usable as int.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<Int> = none
    let n: Int = m.or(0)
    let doubled: Int = n + 1
}
"#,
    );
}

#[test]
fn m5p3b_acceptance_array_bracket_desugar() {
    // WHY: acceptance criterion — `arr[0]` produces `Maybe<Int>`, matching
    // the `.get(index)` return type.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let m: Maybe<Int> = arr[0]
    if (m.exists()) {
        let n: Int = m.value
    }
}
"#,
    );
}

#[test]
fn m5p3b_acceptance_full_maybe_workflow() {
    // WHY: end-to-end test of the complete Maybe<T> workflow:
    // create, check exists, access .value, use .or() — all in one function.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [10, 20, 30]
    let m: Maybe<Int> = arr.get(0)
    let safe: Int = m.or(0)
    if (m.exists()) {
        let direct: Int = m.value
    }
}
"#,
    );
}

#[test]
fn m5p3b_acceptance_fixed_for_loop() {
    // WHY: end-to-end test of iterating over a Fixed<Int> — must compile
    // and bind loop variable as int.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let scores: Fixed<Int> = [100, 200, 300]
    for (score in scores) {
        let doubled: Int = score + score
    }
}
"#,
    );
}

#[test]
fn m5p3b_acceptance_array_for_loop() {
    // WHY: end-to-end test of iterating over an Array<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let nums: Array<Int> = [1, 2, 3]
    for (n in nums) {
        let x: Int = n
    }
}
"#,
    );
}

#[test]
fn m5p3b_array_single_element_inferred() {
    // WHY: single-element array literal `[42]` with annotation must type-check.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [42]
}
"#,
    );
}

#[test]
fn m5p3b_fixed_sort_returns_fixed() {
    // WHY: `.sort()` on Fixed<Int> must return Fixed<Int>, not Array<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let f: Fixed<Int> = [3, 1, 2]
    let sorted: Fixed<Int> = f.sort()
}
"#,
    );
}

#[test]
fn m5p3b_array_freeze_is_postfix_op() {
    // WHY: `.freeze()` is parsed as PostfixOp (not a method call) and returns nothing
    // per the dot-postfix rule — it's a side-effecting body operation, not a
    // transformation. This pins the current behavior; full freeze semantics land in P3c.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    arr.freeze()
}
"#,
    );
}

#[test]
fn m5p3b_maybe_string_exists_guard_works() {
    // WHY: flow-sensitive narrowing must work for Maybe<String> as well as Maybe<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let m: Maybe<String> = none
    if (m.exists()) {
        let s: String = m.value
    }
}
"#,
    );
}

#[test]
fn m5p3b_array_last_returns_maybe() {
    // WHY: `.last()` on Array<String> must return Maybe<String>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<String> = [`a`, `b`]
    let m: Maybe<String> = arr.last()
}
"#,
    );
}

#[test]
fn m5p3b_array_filter_returns_array() {
    // WHY: `.filter()` on Array<Int> must return Array<Int>.
    check_no_diags(
        r#"
function entrypoint() -> Nothing {
    let arr: Array<Int> = [1, 2, 3]
    let filtered: Array<Int> = arr.filter()
}
"#,
    );
}
