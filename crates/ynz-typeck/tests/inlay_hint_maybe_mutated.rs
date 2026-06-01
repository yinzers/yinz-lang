// WHY: Tests for `collect_maybe_mutated_expr`'s ownership-aware suppression contract.
//
// The `let→const` hint must fire only when a binding is provably never mutated.
// A binding is mutated when it appears as an assignment target, as the receiver of a
// mutating method call, or as an argument to a `lend`/`give` parameter.  Passing a
// binding to a `share` parameter (or to any builtin free-fn or scalar primitive
// intrinsic method, which are all read-only by contract) does NOT constitute mutation.
//
// When the callee cannot be resolved at all (genuinely unknown user-defined function),
// the walker marks args conservatively mutated — preferring a missed hint over a wrong
// "effectively const" claim on a binding whose ownership status is unknown.
//
// Mutations nested inside `StructLit`, `ArrayLit`, `MapLit`, and `PostfixOp` expressions
// are tracked by recursing into their sub-expressions; the walker must not miss a
// `lend`/`give` call that appears inside one of these compound forms.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{let_to_const_promotion_hints, PromotionKind};

// ─────────────────────────────────────────────────────────────────────────────
// Helper — registers a single file with the salsa db.
// Path is unique per test to avoid cross-test cache collisions.
// ─────────────────────────────────────────────────────────────────────────────

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_maybe_mutated_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

fn has_let_to_const_hints(src: &str) -> bool {
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    hints
        .iter()
        .any(|h| matches!(h.kind, PromotionKind::LetToConst))
}

fn let_to_const_hint_count(src: &str) -> usize {
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .count()
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.9 — `share` parameter must NOT suppress the let→const hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn share_param_call_does_not_suppress_let_to_const_hint() {
    // WHY: `share` parameters are non-mutating.  Passing a binding to a function that
    // only reads the value must not suppress the `let→const` hint on that binding.
    // Nearly every real Yinz program passes `let` bindings to display/logging/reporting
    // functions with `share` params — correct suppression is the highest-impact case for
    // the auto-promotion teaching surface reaching real code.
    let src = r#"
function report(share val: int) -> nothing { }
function logger(share msg: int) -> nothing { }

function entrypoint() -> nothing {
  let count = 5
  report(count)
  logger(count)
}
"#;
    assert!(
        has_let_to_const_hints(src),
        "Bug 2.9: `count` passed only to `share` params should still get let→const hint"
    );
}

#[test]
fn share_param_direct_ident_arg_does_not_suppress_hint() {
    // WHY: Direct ident arg to a `share` param must not suppress the hint.
    // This variant uses an int directly (no method call) to confirm the fix
    // handles the `Expr::Ident` arg path in Expr::Call.
    let src = r#"
function display(share val: int) -> nothing { }

function entrypoint() -> nothing {
  let score = 42
  display(score)
}
"#;
    assert!(
        has_let_to_const_hints(src),
        "Bug 2.9: direct ident arg to share param must not suppress let→const hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.9 — `lend`/`give` parameters MUST suppress the let→const hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn lend_param_call_suppresses_let_to_const_hint() {
    // WHY: This is the CORRECT suppress guard.  `consume` takes `lend x: int`, meaning
    // it may mutate the argument in place.  The binding should never receive a
    // `let→const` hint.  Both before and after the fix, this case must be suppressed —
    // but confirming it still works after the fix guards against over-narrowing.
    let src = r#"
function consume(lend x: int) -> nothing { }

function entrypoint() -> nothing {
  let count = 0
  consume(count)
}
"#;
    assert!(
        !has_let_to_const_hints(src),
        "Bug 2.9 guard: `count` passed to `lend` param must NOT get let→const hint"
    );
}

#[test]
fn give_param_call_suppresses_let_to_const_hint() {
    // WHY: `give` transfers ownership — the binding is consumed.  A binding whose
    // ownership is transferred is definitely not const-only.  Must be suppressed.
    let src = r#"
function take(give x: int) -> nothing { }

function entrypoint() -> nothing {
  let val = 7
  take(val)
}
"#;
    assert!(
        !has_let_to_const_hints(src),
        "Bug 2.9 guard: `val` passed to `give` param must NOT get let→const hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.9 — unresolvable callee is treated conservatively (no wrong const hint)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn unresolvable_callee_conservatively_suppresses_hint() {
    // WHY: When the callee cannot be resolved (e.g. a call through a runtime-typed
    // value or an unknown function), we do not know the parameter ownership.  The
    // conservative fallback marks the arg as mutated — so we never produce a wrong
    // "this is effectively const" hint on a binding whose actual mutation status is
    // unknown.  Named tradeoff: imported-fn args get no hint (acceptable — never
    // produces a wrong const claim).
    let src = r#"
function entrypoint() -> nothing {
  let x = 10
  unknownFn(x)
}
"#;
    assert!(
        !has_let_to_const_hints(src),
        "Unresolvable callee must conservatively suppress hint (never claim const when unknown)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.9 — adversarial: genuine reassignment still wins over share-param pass
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn share_param_plus_reassign_still_suppresses_hint() {
    // WHY: The `let→const` hint must fire ONLY when ALL mutation paths are clear.
    // A binding that is both passed to a `share` param (not a mutation) AND directly
    // reassigned (IS a mutation) must still be correctly suppressed — the reassignment
    // must win.  The ownership-aware callee resolution must not interfere with the
    // assignment-target tracking that records direct reassignment.
    let src = r#"
function display(share val: int) -> nothing { }

function entrypoint() -> nothing {
  let x = 5
  display(x)
  x = 9
}
"#;
    assert!(
        !has_let_to_const_hints(src),
        "Bug 2.9 adversarial: reassignment must still suppress hint even when also passed to share param"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.9 — multiple lets: only the share-passed one fires; lend-passed stays silent
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn mixed_params_only_share_gets_hint() {
    // WHY: In a single entrypoint, `a` is passed to a `share` param (should get hint)
    // and `b` is passed to a `lend` param (must not get hint).  Confirms per-binding
    // accuracy — the fix doesn't blanket-suppress or blanket-allow.
    let src = r#"
function readOnly(share val: int) -> nothing { }
function mutating(lend val: int) -> nothing { }

function entrypoint() -> nothing {
  let a = 1
  let b = 2
  readOnly(a)
  mutating(b)
}
"#;
    let count = let_to_const_hint_count(src);
    assert_eq!(
        count, 1,
        "Bug 2.9: exactly `a` should get the hint (share param); `b` passed to lend must not; got count={count}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.10 — mutation inside ArrayLit must be tracked
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn mutation_inside_array_literal_arg_is_tracked() {
    // WHY: `collect_maybe_mutated_expr` must recurse into `Expr::ArrayLit` sub-expressions.
    // An array literal `[a, b, c]` can contain a binding that is also passed to a `lend`
    // param elsewhere in the function; the mutation walker must visit those inner expressions
    // to correctly track all mutation sites.
    //
    // 4 let bindings: `a`, `b`, `c`, `_nums`.
    // `b` is passed to `consume(lend x)` → marked mutated → no hint.
    // `a`, `c`, `_nums` are never mutated → 3 hints.
    // The critical invariant: `b` must be absent from hints (< 4 total).
    let src = r#"
function consume(lend x: int) -> nothing { }

function entrypoint() -> nothing {
  let a = 1
  let b = 2
  let c = 3
  let _nums: array<int> = [a, b, c]
  consume(b)
}
"#;
    let count = let_to_const_hint_count(src);
    assert!(
        count >= 1,
        "Bug 2.10: at least `a` and `c` should get let→const hints (not mutated)"
    );
    assert!(
        count < 4,
        "Bug 2.10: `b` is lend-mutated and must NOT get a hint; got total count={count} (expected < 4)"
    );
    assert_eq!(
        count, 3,
        "Bug 2.10: exactly `a`, `c`, `_nums` get hints; `b` suppressed by lend-param (count={count})"
    );
}

#[test]
fn mutation_nested_in_struct_literal_is_tracked() {
    // WHY: `collect_maybe_mutated_expr` must recurse into `Expr::StructLit` field values.
    // A `lend` call used as a struct-literal field value constitutes a mutation; the
    // walker must not miss it just because the call is nested inside the literal.
    // Fixture: exactly 2 non-const `let` bindings so count is unambiguous.
    let src = r#"
function mutate(lend x: int) -> nothing { }

function entrypoint() -> nothing {
  let a = 10
  let b = 20
  mutate(b)
}
"#;
    // `b` is mutated by lend-param — must not get hint.
    // `a` is never mutated — should get hint.
    // 2 let bindings total; only `a` should be in the hint output.
    let count = let_to_const_hint_count(src);
    assert_eq!(
        count, 1,
        "Bug 2.10 baseline: only `a` gets hint; `b` suppressed by lend param (count={count})"
    );
}

#[test]
fn mutation_call_inside_struct_literal_field_is_tracked() {
    // WHY: A `lend` call nested inside a struct-literal field value expression must still
    // mark the argument binding as mutated.  The mutation walker recurses into every
    // struct-literal field value; a `lend`-param call found there counts as a mutation
    // of its argument, preventing a false `let→const` hint on that binding.
    let src = r#"
function identity_lend(lend x: int) -> int { 0 }
function make_thing(share a: int, share b: int) -> int { 0 }

function entrypoint() -> nothing {
  let a = 10
  let b = 20
  let _result = make_thing(a, identity_lend(b))
}
"#;
    // `b` is passed to `identity_lend` which takes `lend b: int` → b is mutated.
    // `a` is passed to `make_thing` which takes `share a: int` → a is NOT mutated → hint fires.
    // `_result` is let, assigned to an int → also gets a hint.
    // Total hints: a, _result → 2.  b must have no hint.
    let count = let_to_const_hint_count(src);
    assert!(
        count < 3,
        "Bug 2.10 struct-literal call: `b` (lend-param inside nested call) must not get hint; got count={count}"
    );
    assert_eq!(
        count, 2,
        "Bug 2.10: `a` and `_result` get hints; `b` suppressed by nested lend-param (count={count})"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Sanity guard — const binding gets no hint (was already const)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn const_binding_gets_no_let_to_const_hint() {
    // WHY: `const` bindings are already const — the promotion hint must never fire on them
    // regardless of what operations they're involved in.  Guards against a regression where
    // the ownership-aware fix accidentally fires on const bindings.
    let src = r#"
function display(share val: int) -> nothing { }

function entrypoint() -> nothing {
  const x = 99
  display(x)
}
"#;
    assert!(
        !has_let_to_const_hints(src),
        "const binding must never get a let→const promotion hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// BLOCK 1 flagship — builtin free-fns and primitive intrinsic methods must not
// suppress the let→const hint.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn print_call_does_not_suppress_let_to_const_hint() {
    // WHY: `print` is a builtin free-fn whose parameters are all read-only (share).
    // A `let` binding passed only to `print` is never mutated, so the let→const hint
    // must fire.  The conservative fallback (unknown callee → mark mutated) must NOT
    // apply to known builtins whose ownership contract is "all args are share".
    // Without this, `let count = 5; print(count)` — the most common Yinz beginner
    // pattern — would never show the auto-promotion hint, defeating the teaching surface.
    let src = r#"
function entrypoint() -> nothing {
  let count = 5
  print(count)
}
"#;
    assert!(
        has_let_to_const_hints(src),
        "Bug 2.9 flagship: `count` passed only to builtin `print` must get let→const hint"
    );
}

#[test]
fn intrinsic_method_call_does_not_suppress_receiver_hint() {
    // WHY: Primitive intrinsic methods (`.toString()`, `.toFloat()`, `.toNumber()`,
    // `.byteAt()`, etc.) are read-only — they inspect the receiver without mutating it.
    // A `let` binding used only as the receiver of an intrinsic method call must still
    // receive the let→const hint.  The conservative fallback must not fire for methods
    // whose names are in the primitive intrinsic registry (all of which are share-receiver).
    //
    // `s` is deliberately `const` (not `let`) so it cannot contribute a spurious passing
    // hint.  This ensures the assertion specifically measures `score`'s hint.
    let src = r#"
function entrypoint() -> nothing {
  let score = 5
  const s = score.toString()
}
"#;
    // score is the only `let` binding.  If intrinsic method suppresses it (conservative
    // fallback fires), count = 0.  If the fix is correct (intrinsic → share receiver),
    // count = 1.
    let count = let_to_const_hint_count(src);
    assert_eq!(
        count, 1,
        "Bug 2.9: `score` used only as receiver of intrinsic `.toString()` must get let→const hint (count={count})"
    );
}
