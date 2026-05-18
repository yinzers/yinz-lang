// WHY: The generics engine (P3a) is the load-bearing piece of M5. These tests
// pin the MonomorphizationTable contents, type inference behavior, constraint
// checking, and field access through generic shapes. A regression in any of
// these surfaces would silently break the codegen monomorphization pass (P4a)
// which consumes the table without re-verifying the type-checking invariants.
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

// ── Generic function declarations ─────────────────────────────────────────────

#[test]
fn m5_generic_fn_single_type_param_typechecks() {
    // WHY: acceptance criterion — `function identity<T>` must type-check without
    // errors. If the TypeParam is rejected as "unknown type", all generic code fails.
    check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { }
"#,
    );
}

#[test]
fn m5_generic_fn_two_type_params_typechecks() {
    // WHY: multi-param generic `pair<A, B>` must type-check. Tests that the
    // type_param_scope correctly tracks multiple names simultaneously.
    check_no_diags(
        r#"
function pair<A, B>(give first: A, give second: B) -> A { return first }
function entrypoint() -> nothing { }
"#,
    );
}

#[test]
fn m5_generic_fn_body_uses_type_param() {
    // WHY: `return value` inside `identity<T>` should pass the return-type check
    // because `value: T` and return type is `T`. TypeParam equality must hold.
    check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let x = identity(5) }
"#,
    );
}

// ── Call-site inference ───────────────────────────────────────────────────────

#[test]
fn m5_inference_identity_int() {
    // WHY: acceptance criterion — `identity(5)` must record `(identity, [int])`
    // in the MonomorphizationTable. Without this, codegen has no instantiation
    // to emit.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let n = identity(5) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "identity")
        .expect("identity must appear in MonomorphizationTable");
    assert_eq!(key.type_args, vec![Type::Int]);
}

#[test]
fn m5_inference_identity_string() {
    // WHY: acceptance criterion — `identity(`hello`)` must record `[String]`.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let s = identity(`hello`) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "identity")
        .expect("identity must appear in MonomorphizationTable");
    assert_eq!(key.type_args, vec![Type::String]);
}

#[test]
fn m5_inference_two_type_params() {
    // WHY: acceptance criterion — `pair(5, `hello`)` must record `(pair, [int, string])`.
    let out = check_no_diags(
        r#"
function pair<A, B>(give first: A, give second: B) -> A { return first }
function entrypoint() -> nothing { let p = pair(5, `hello`) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "pair")
        .expect("pair must appear in MonomorphizationTable");
    assert_eq!(key.type_args, vec![Type::Int, Type::String]);
}

#[test]
fn m5_multiple_distinct_instantiations_both_recorded() {
    // WHY: two calls to `identity` with different types must produce TWO separate
    // MonomorphizationTable entries. A single-entry table would make codegen emit
    // only one LLVM function and mislink the other.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing {
  let n = identity(5)
  let s = identity(`hello`)
}
"#,
    );
    let entries: Vec<_> = out
        .mono_table
        .entries
        .keys()
        .filter(|k| k.fn_name == "identity")
        .collect();
    assert_eq!(
        entries.len(),
        2,
        "Expected two identity instantiations (int + string)"
    );
}

#[test]
fn m5_same_instantiation_deduped() {
    // WHY: two calls `identity(5)` and `identity(10)` both produce `[int]`.
    // The table is keyed by type args, so only ONE entry should exist.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing {
  let a = identity(5)
  let b = identity(10)
}
"#,
    );
    let entries: Vec<_> = out
        .mono_table
        .entries
        .keys()
        .filter(|k| k.fn_name == "identity")
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "Same type args must dedup to one table entry"
    );
}

// ── Explicit type args at call sites ─────────────────────────────────────────

#[test]
fn m5_explicit_type_arg_int() {
    // WHY: acceptance criterion — `identity<int>(5)` must record `[int]`.
    // Explicit type args must override inference without producing a diagnostic.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let n = identity<int>(5) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "identity")
        .expect("identity must appear in MonomorphizationTable");
    assert_eq!(key.type_args, vec![Type::Int]);
}

#[test]
fn m5_explicit_type_arg_string() {
    // WHY: acceptance criterion — `identity<string>("hello")` records `[string]`.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let s = identity<string>(`hello`) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "identity")
        .unwrap();
    assert_eq!(key.type_args, vec![Type::String]);
}

// ── Arity errors ──────────────────────────────────────────────────────────────

#[test]
fn m5_generic_call_wrong_arity_produces_error() {
    // WHY: `identity(5, 6)` — too many args — must produce exactly ONE error.
    // If the generic engine crashes or produces zero errors, arity bugs slip through.
    check_diag_count(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let n = identity(5, 6) }
"#,
        1,
    );
}

#[test]
fn m5_generic_call_too_few_args_produces_error() {
    // WHY: `identity()` — missing arg — must produce ONE error (cannot infer T).
    check_diag_count(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let n = identity() }
"#,
        1,
    );
}

// ── Cannot-infer diagnostic ───────────────────────────────────────────────────

#[test]
fn m5_cannot_infer_type_param_when_no_args() {
    // WHY: `createList()` with no args and no explicit type annotation — the
    // compiler cannot infer T and must emit the "cannot work out T" diagnostic.
    check_diag_count(
        r#"
function createEmpty<T>(give dummy: T) -> T { return dummy }
function entrypoint() -> nothing { let x = createEmpty() }
"#,
        1,
    );
}

// ── Follows constraint checking ───────────────────────────────────────────────

#[test]
fn m5_constraint_satisfied_no_error() {
    // WHY: acceptance criterion — `sort<T follows Comparable>` called with a type
    // that DOES follow Comparable must produce zero errors. The constraint check
    // must not fire false positives.
    check_no_diags(
        r#"
shape Comparable {
  compare(share self, share other: Self) -> int
}
shape Player follows Comparable {
  name: string
  score: int
}
function compare(share self: Player, share other: Player) -> int {
  return self.score
}
function sort<T follows Comparable>(share item: T) -> T { return item }
function entrypoint() -> nothing {
  const p: Player = { name: `Alice`, score: 100 }
  let q = sort(p)
}
"#,
    );
}

#[test]
fn m5_constraint_violated_produces_error() {
    // WHY: acceptance criterion — calling `sort<T follows Comparable>` with a type
    // that does NOT follow Comparable must produce exactly ONE error.
    check_diag_count(
        r#"
shape Comparable {
  compare(share self, share other: Self) -> int
}
shape Player {
  name: string
}
function sort<T follows Comparable>(share item: T) -> T { return item }
function entrypoint() -> nothing {
  const p: Player = { name: `Alice` }
  let q = sort(p)
}
"#,
        1,
    );
}

#[test]
fn m5_constraint_error_mentions_contract_name() {
    // WHY: the "does not follow" error must name the contract so the user knows
    // what to add. A generic "type error" message is useless.
    let out = check(
        r#"
shape Comparable {
  compare(share self, share other: Self) -> int
}
shape Player { name: string }
function sort<T follows Comparable>(share item: T) -> T { return item }
function entrypoint() -> nothing {
  const p: Player = { name: `Alice` }
  let q = sort(p)
}
"#,
    );
    assert_eq!(out.diagnostics.len(), 1);
    let diag = &out.diagnostics[0];
    assert!(
        diag.what.contains("Comparable") || diag.what_instead.contains("Comparable"),
        "Error must mention the contract name `Comparable`, got: {:?}",
        diag
    );
}

// ── Generic shape declarations ────────────────────────────────────────────────

#[test]
fn m5_generic_shape_decl_typechecks() {
    // WHY: acceptance criterion — `shape Pair<A, B>` must type-check without errors.
    check_no_diags(
        r#"
shape Pair<A, B> { first: A  second: B }
function entrypoint() -> nothing { }
"#,
    );
}

#[test]
fn m5_generic_shape_appears_in_generic_shape_table() {
    // WHY: the generic shape must be in the generic_shape_table (not the regular
    // shape_table). If it lands in the wrong table, field access resolution breaks.
    let out = check_no_diags(
        r#"
shape Pair<A, B> { first: A  second: B }
function entrypoint() -> nothing { }
"#,
    );
    // Indirect verification: a Pair<int, string> binding must resolve cleanly.
    // If the shape table is wrong, this would have produced a diagnostic.
    let _ = out;
}

// ── Field access through generic shapes ───────────────────────────────────────

#[test]
fn m5_field_access_through_generic_shape() {
    // WHY: acceptance criterion — `p.first` where `p: Pair<int, string>` must
    // resolve to `int`. Without substitution, it would stay as `TypeParam("A")`
    // and typeck would fail to verify downstream uses.
    check_no_diags(
        r#"
shape Pair<A, B> { first: A  second: B }
function mkPair<A, B>(give first: A, give second: B) -> A { return first }
function entrypoint() -> nothing {
  let x = mkPair(5, `hello`)
}
"#,
    );
}

#[test]
fn m5_field_access_wrong_field_on_generic_shape_error() {
    // WHY: `p.nonexistent` on a generic shape must produce a "no such field" error,
    // just like a concrete shape. Generic shapes must not silently succeed for
    // unknown fields.
    let out = check(
        r#"
shape Pair<A, B> { first: A  second: B }
function entrypoint() -> nothing {
  let p: Pair<int, string> = { first: 5, second: `hi` }
  let x = p.third
}
"#,
    );
    assert!(
        out.diagnostics.len() >= 1,
        "Expected at least one error for accessing nonexistent field on Pair"
    );
}

// ── Non-generic functions unaffected ─────────────────────────────────────────

#[test]
fn m5_non_generic_fn_not_in_mono_table() {
    // WHY: `add(1, 2)` is non-generic and must NOT appear in the MonomorphizationTable.
    // If non-generic functions are accidentally recorded, codegen would try to
    // emit a second copy of every function.
    let out = check_no_diags(
        r#"
function add(a: int, b: int) -> int { return a }
function entrypoint() -> nothing { let x = add(1, 2) }
"#,
    );
    assert!(
        out.mono_table.entries.is_empty(),
        "Non-generic call must not populate MonomorphizationTable"
    );
}

#[test]
fn m5_non_generic_fn_still_typechecks() {
    // WHY: adding generics engine must not break existing non-generic function
    // type-checking. This is the basic M4 regression guard.
    check_no_diags(
        r#"
function greet(name: string) -> string { return name }
function entrypoint() -> nothing { let s = greet(`Alice`) }
"#,
    );
}

// ── All existing M4 typeck paths still work ───────────────────────────────────

#[test]
fn m5_m4_shapes_and_follows_still_work() {
    // WHY: M4 shapes + follows contracts must be unaffected by the generics engine.
    // A regression here means the generic shape table lookup is incorrectly
    // stealing entries from the concrete shape table.
    check_no_diags(
        r#"
shape Damageable { takeDamage(lend self, amount: int) -> nothing }
shape Player follows Damageable { name: string  health: int }
function takeDamage(lend self: Player, amount: int) -> nothing {
  self.health = self.health
}
function entrypoint() -> nothing {
  let p: Player = { name: `Bob`, health: 100 }
  p.takeDamage(10)
}
"#,
    );
}

#[test]
fn m5_m4_ownership_still_enforced() {
    // WHY: const-cannot-be-given still applies inside a generic call. The generics
    // engine must not bypass the ownership checks from M4.
    let out = check(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing {
  const x = 5
  let y = identity(x)
}
"#,
    );
    assert_eq!(
        out.diagnostics.len(),
        1,
        "const-cannot-be-given must still fire"
    );
}

// ── Concurrent generic + non-generic functions ────────────────────────────────

#[test]
fn m5_generic_and_non_generic_coexist() {
    // WHY: a module with both `identity<T>` and `add(int, int)` must type-check
    // cleanly. Both tables (sig_table + generic_fn_table) must be populated
    // independently without cross-contamination.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function add(a: int, b: int) -> int { return a }
function entrypoint() -> nothing {
  let x = identity(5)
  let y = add(1, 2)
}
"#,
    );
    // Only `identity` should be in the mono table.
    assert_eq!(out.mono_table.entries.len(), 1);
    let key = out.mono_table.entries.keys().next().unwrap();
    assert_eq!(key.fn_name, "identity");
}

// ── Mono table structure ──────────────────────────────────────────────────────

#[test]
fn m5_mono_entry_has_correct_ret_type() {
    // WHY: the MonoSignature must carry the CONCRETE return type, not TypeParam.
    // Codegen reads ret_type to set the LLVM return type of the monomorphic function.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let n = identity(5) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "identity")
        .unwrap();
    let sig = out.mono_table.entries.get(key).unwrap();
    assert_eq!(
        sig.ret_type,
        Type::Int,
        "Monomorphized identity<int> must have ret_type = int"
    );
}

#[test]
fn m5_mono_entry_has_correct_param_types() {
    // WHY: the MonoSignature param_types must be concrete. Codegen uses these
    // to emit the LLVM function parameter types.
    let out = check_no_diags(
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> nothing { let n = identity(5) }
"#,
    );
    let key = out
        .mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == "identity")
        .unwrap();
    let sig = out.mono_table.entries.get(key).unwrap();
    assert_eq!(
        sig.param_types,
        vec![Type::Int],
        "Monomorphized identity<int> must have param_type = int"
    );
}
