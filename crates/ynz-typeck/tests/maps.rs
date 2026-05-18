// WHY: P3c adds map<K,V> typeck: MapLit parsing, bracket access returning maybe<V>,
// method dispatch (get/set/has/remove/count/keys/values/entries), for-loop MapEntry
// binding, duplicate-key detection, and all error paths.
// Regressions here would silently break P4b codegen which lowers map operations.
//
// test-ratchet: M7 P1 — all double-quoted string literals in Yinz source strings
// replaced with backtick strings. Double-quotes now produce an error diagnostic
// in the lexer; every `check_no_diags` test would fail with unexpected diagnostics.

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

// ── map literal type-checks ──────────────────────────────────────────────────

#[test]
fn m5p3c_map_string_int_literal_typechecks() {
    // WHY: acceptance criterion — `let m: map<string, int> = { `alice`: 90, `bob`: 85 }`
    // must type-check without diagnostics.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90, `bob`: 85 }
}
"#);
}

#[test]
fn m5p3c_map_string_string_literal_typechecks() {
    // WHY: map<string, string> must work — both key and value types can be string.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, string> = { `key`: `value`, `name`: `alice` }
}
"#);
}

#[test]
fn m5p3c_map_int_bool_literal_typechecks() {
    // WHY: int keys must be valid — map<int, bool> with int literals.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<int, bool> = { 1: true, 2: false }
}
"#);
}

#[test]
fn m5p3c_map_empty_with_annotation_typechecks() {
    // WHY: `let m: map<string, int> = {}` must be valid — empty map literal
    // via the BuiltinMap annotation path in check_struct_lit.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = {}
}
"#);
}

#[test]
fn m5p3c_map_single_entry_typechecks() {
    // WHY: single-entry map must work (no trailing comma needed).
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `only`: 1 }
}
"#);
}

// ── map bracket access ────────────────────────────────────────────────────────

#[test]
fn m5p3c_map_bracket_access_returns_maybe() {
    // WHY: `m["key"]` must return `maybe<int>` — bracket access on maps is fallible.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v: maybe<int> = m[`alice`]
}
"#);
}

#[test]
fn m5p3c_map_bracket_access_with_or_returns_value() {
    // WHY: `m[`alice`].or(0)` must resolve to int — chaining `.or()` on a maybe<int>
    // from bracket access.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v: int = m[`alice`].or(0)
}
"#);
}

#[test]
fn m5p3c_map_bracket_assignment_typechecks() {
    // WHY: `m[`bob`] = 95` must type-check when m: map<string, int>.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90, `bob`: 85 }
    m[`bob`] = 95
}
"#);
}

#[test]
fn m5p3c_map_bracket_assignment_wrong_value_type_errors() {
    // WHY: `m["key"] = "wrong"` must error when the map holds int values.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    m[`alice`] = `not an int`
}
"#, 1);
}

// ── map method calls ──────────────────────────────────────────────────────────

#[test]
fn m5p3c_map_get_returns_maybe() {
    // WHY: `.get("alice")` on map<string, int> must return maybe<int>.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v: maybe<int> = m.get(`alice`)
}
"#);
}

#[test]
fn m5p3c_map_has_returns_bool() {
    // WHY: `.has()` on map<string, int> must return bool.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let exists: bool = m.has(`alice`)
}
"#);
}

#[test]
fn m5p3c_map_set_returns_nothing() {
    // WHY: `.set()` must return nothing — mutation that returns no value.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    m.set(`bob`, 85)
}
"#);
}

#[test]
fn m5p3c_map_remove_returns_nothing() {
    // WHY: `.remove()` must return nothing.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90, `bob`: 85 }
    m.remove(`alice`)
}
"#);
}

#[test]
fn m5p3c_map_count_returns_int() {
    // WHY: `.count()` on map<string, int> must return int.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let n: int = m.count()
}
"#);
}

#[test]
fn m5p3c_map_keys_returns_array_of_keys() {
    // WHY: `.keys()` must return array<string> for map<string, int>.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let k: array<string> = m.keys()
}
"#);
}

#[test]
fn m5p3c_map_values_returns_array_of_values() {
    // WHY: `.values()` must return array<int> for map<string, int>.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v: array<int> = m.values()
}
"#);
}

#[test]
fn m5p3c_map_entries_returns_array_of_mapentry() {
    // WHY: `.entries()` must return an array of MapEntry values — verified by
    // iterating and accessing .key / .value, which only work on MapEntry.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let entries = m.entries()
    for (e in entries) {
        print(e.key)
        print(e.value.toString())
    }
}
"#);
}

#[test]
fn m5p3c_map_clear_returns_nothing() {
    // WHY: `.clear()` must return nothing.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    m.clear()
}
"#);
}

#[test]
fn m5p3c_map_find_returns_maybe() {
    // WHY: `.find()` is an alias for `.get()` — must return maybe<int>.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v: maybe<int> = m.find(`alice`)
}
"#);
}

// ── for-loop MapEntry binding ─────────────────────────────────────────────────

#[test]
fn m5p3c_map_for_loop_entry_key_typechecks() {
    // WHY: `for (entry in m)` must bind `entry` as MapEntry<string, int>,
    // and `entry.key` must resolve to string without diagnostics.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90, `bob`: 85 }
    for (entry in m) {
        print(entry.key)
    }
}
"#);
}

#[test]
fn m5p3c_map_for_loop_entry_value_typechecks() {
    // WHY: `entry.value` inside a map for-loop must resolve to int without diagnostics.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90, `bob`: 85 }
    for (entry in m) {
        print(entry.value.toString())
    }
}
"#);
}

#[test]
fn m5p3c_map_for_loop_entry_key_and_value_typechecks() {
    // WHY: using both entry.key and entry.value in the same loop body must work.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    for (entry in m) {
        print(entry.key)
        print(entry.value.toString())
    }
}
"#);
}

// ── duplicate key detection ───────────────────────────────────────────────────

#[test]
fn m5p3c_map_duplicate_string_key_produces_error() {
    // WHY: `{ `alice`: 1, `alice`: 2 }` is always a mistake — the compiler must reject it
    // rather than silently picking one value.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 1, `alice`: 2 }
}
"#, 1);
}

#[test]
fn m5p3c_map_duplicate_int_key_produces_error() {
    // WHY: duplicate int keys must also be caught.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<int, int> = { 1: 10, 2: 20, 1: 30 }
}
"#, 1);
}

#[test]
fn m5p3c_map_three_unique_keys_no_error() {
    // WHY: three distinct keys must produce no duplicate-key diagnostic.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `a`: 1, `b`: 2, `c`: 3 }
}
"#);
}

// ── type mismatch errors ──────────────────────────────────────────────────────

#[test]
fn m5p3c_map_wrong_value_type_in_literal_errors() {
    // WHY: `{ `alice`: `not an int` }` when annotated as map<string, int>
    // must produce a type-mismatch diagnostic.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: `not an int` }
}
"#, 1);
}

#[test]
fn m5p3c_map_wrong_key_type_in_literal_errors() {
    // WHY: a string key when the map expects int keys must be caught.
    // The first key sets the inferred key type; subsequent mismatches error.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<int, int> = { `wrong`: 1, 2: 20 }
}
"#, 1);
}

#[test]
fn m5p3c_map_empty_without_annotation_errors() {
    // WHY: `let m = {}` with no annotation and no entries cannot determine key/value types.
    // The parser routes `{}` to struct-lit (no identifier keys, no literal keys) —
    // which produces a "needs type annotation" error from check_struct_lit.
    check_diag_count(r#"
function main() -> nothing {
    let m = {}
}
"#, 1);
}

// ── dot access on map ─────────────────────────────────────────────────────────

#[test]
fn m5p3c_map_dot_access_errors() {
    // WHY: `m.alice` is not valid for a map — map keys are dynamic runtime values,
    // not shape fields. The compiler must reject dot access and suggest bracket syntax.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v = m.alice
}
"#, 1);
}

#[test]
fn m5p3c_map_dot_access_error_mentions_bracket() {
    // WHY: the diagnostic must tell the user to use bracket syntax — just rejecting
    // without guidance would leave them stuck.
    check_has_diag(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let v = m.alice
}
"#, "bracket");
}

// ── nonexistent method ────────────────────────────────────────────────────────

#[test]
fn m5p3c_map_nonexistent_method_errors() {
    // WHY: `m.nonexistent()` must produce exactly one diagnostic naming the method.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    m.nonexistent()
}
"#, 1);
}

#[test]
fn m5p3c_map_nonexistent_method_error_mentions_method_name() {
    // WHY: the diagnostic must mention "nonexistent" so the user can see which name failed.
    check_has_diag(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    m.nonexistent()
}
"#, "nonexistent");
}

// ── MapEntry error paths ──────────────────────────────────────────────────────

#[test]
fn m5p3c_mapentry_nonexistent_field_errors() {
    // WHY: `entry.nonexistent` on a MapEntry must produce one diagnostic.
    // MapEntry only has .key and .value — any other field is an error.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    for (entry in m) {
        let bad = entry.nonexistent
    }
}
"#, 1);
}

#[test]
fn m5p3c_mapentry_method_call_errors() {
    // WHY: `entry.someMethod()` on a MapEntry must produce one diagnostic —
    // MapEntry has no methods, only the two fields.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    for (entry in m) {
        entry.someMethod()
    }
}
"#, 1);
}

// ── struct lit with map annotation ───────────────────────────────────────────

#[test]
fn m5p3c_struct_lit_with_map_annotation_and_identifier_keys_errors() {
    // WHY: `let m: map<string, int> = { alice: 90 }` — identifier keys in a
    // struct-lit-looking literal when the annotation is BuiltinMap must error.
    // Users must write `{ `alice`: 90 }` (string key) for map literals.
    check_diag_count(r#"
function main() -> nothing {
    let m: map<string, int> = { alice: 90 }
}
"#, 1);
}

// ── type_name rendering for map types ────────────────────────────────────────

#[test]
fn m5p3c_map_type_name_in_diagnostic_renders_correctly() {
    // WHY: when a map-typed receiver gets an unknown method call, the diagnostic
    // must render the type as "map<string, int>" — not an internal representation.
    check_has_diag(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    m.badMethod()
}
"#, "map<string, int>");
}

#[test]
fn m5p3c_mapentry_type_name_in_diagnostic_renders_correctly() {
    // WHY: when a MapEntry gets an unknown field access, the diagnostic must
    // mention "MapEntry" in the what message.
    check_has_diag(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    for (entry in m) {
        let bad = entry.nonexistent
    }
}
"#, "MapEntry");
}

// ── map variant in typeck Type count ─────────────────────────────────────────

#[test]
fn m5p3c_type_variant_count_locked() {
    // WHY: adding new types before their milestones introduces untested paths.
    //
    // test-ratchet: M5 P3b adds BuiltinArray(13), BuiltinFixed(14), Maybe(15).
    // test-ratchet: M5 P3c adds BuiltinMap(16), MapEntry(17). Total: 17.
    // (TypeParam(11) and Generic(12) added in P3a — see check.rs m4_type_variant_count_locked)
    use ynz_typeck::Type;
    let all: &[Type] = &[
        Type::Nothing,
        Type::String,
        Type::Error,
        Type::Int,
        Type::Float,
        Type::Number { precision: 34 },
        Type::Bool,
        Type::Range { element: Box::new(Type::Int), end_inclusive: false },
        Type::Shape { name: "Player".into() },
        Type::Dynamic { contract: "Foo".into() },
        Type::TypeParam { name: "T".into() },
        Type::Generic { name: "Pair".into(), args: vec![Type::Int, Type::String] },
        Type::BuiltinArray { elem: Box::new(Type::Int) },
        Type::BuiltinFixed { elem: Box::new(Type::Int), size: None },
        Type::Maybe { inner: Box::new(Type::Int) },
        Type::BuiltinMap { key: Box::new(Type::String), val: Box::new(Type::Int) },
        Type::MapEntry { key: Box::new(Type::String), val: Box::new(Type::Int) },
    ];
    assert_eq!(all.len(), 17, "Type variant count changed from 17 — add // test-ratchet: comment");
}

// ── map literal parsed correctly (parse-level smoke test) ────────────────────

#[test]
fn m5p3c_map_literal_with_trailing_comma_typechecks() {
    // WHY: trailing commas in map literals must be accepted.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90, `bob`: 85, }
}
"#);
}

#[test]
fn m5p3c_map_bracket_access_exists_check_and_value() {
    // WHY: bracket access produces maybe<T>, which can be checked with .exists()
    // and unwrapped with .value — the full usage pattern must work.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<string, int> = { `alice`: 90 }
    let val: maybe<int> = m[`alice`]
    if (val.exists()) {
        print(val.value.toString())
    }
}
"#);
}

#[test]
fn m5p3c_map_int_key_bracket_access_returns_maybe() {
    // WHY: map<int, string> bracket access with int key must return maybe<string>.
    check_no_diags(r#"
function main() -> nothing {
    let m: map<int, string> = { 1: `one`, 2: `two` }
    let v: maybe<string> = m[1]
}
"#);
}
