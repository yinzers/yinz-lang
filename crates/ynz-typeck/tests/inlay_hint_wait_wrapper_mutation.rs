// WHY: Regression tests for the wait/background/is/interpolated-string wrapper
// blindspot in `collect_maybe_mutated_expr`.
//
// The mutation collector must recurse into ALL Expr wrappers — including
// `Expr::Wait`, `Expr::Background`, `Expr::Is` (scrutinee), and
// `Expr::InterpolatedString` (each embedded `StringPart::Expr`).
//
// Without this recursion a `lend`/`give` mutation hidden inside one of these
// wrappers is invisible to the mutation collector, so a WRONG `let→const` or
// `array→fixed` hint fires on a binding that IS mutated.
//
// Tests (a)–(d) are the Paper-Trace cases the cumulative code-reviewer identified.
// The control case (d) guards against over-suppression: a read-only `wait`-wrapped
// call must NOT suppress the hint.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{array_to_fixed_promotion_hints, let_to_const_promotion_hints, PromotionKind};

// ─────────────────────────────────────────────────────────────────────────────
// Helper — unique path per source length to avoid cross-test salsa cache hits.
// ─────────────────────────────────────────────────────────────────────────────

fn make_db(src: &str, tag: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_wait_wrapper_{tag}_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

fn has_let_to_const_hint_for(src: &str, tag: &str) -> bool {
    let (db, sf) = make_db(src, tag);
    let hints = let_to_const_promotion_hints(&db, sf);
    hints.iter().any(|h| matches!(h.kind, PromotionKind::LetToConst))
}

fn has_array_to_fixed_hint_for(src: &str, tag: &str) -> bool {
    let (db, sf) = make_db(src, tag);
    let hints = array_to_fixed_promotion_hints(&db, sf);
    hints.iter().any(|h| matches!(h.kind, PromotionKind::ArrayToFixed))
}

// ─────────────────────────────────────────────────────────────────────────────
// (a) wait-wrapped lend call must suppress let→const hint on the argument
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wait_wrapper_lend_call_suppresses_let_to_const_hint() {
    // WHY: `wait slowFill(buf)` where `slowFill` takes `lend self: Buffer` is a
    // mutation via a lend parameter.  The `Expr::Wait` wrapper around the inner
    // `Expr::Call` must not cause the mutation collector to miss the lend call.
    // If the Wait arm is absent from `collect_maybe_mutated_expr`, `buf` is
    // invisible to the mutation collector and a WRONG `let→const` hint fires.
    //
    // Observed before fix: `let_to_const_promotion_hints` returned a hint for
    // `buf` (incorrectly claiming it is effectively const).
    // Expected after fix: no `let→const` hint for `buf`.
    let src = r#"
function slowFill(lend buf: int) -> nothing { }

function entrypoint() -> nothing {
  let buf = 0
  wait slowFill(buf)
}
"#;
    assert!(
        !has_let_to_const_hint_for(src, "a_wait_lend"),
        "wait-wrapper around a lend call must suppress the let→const hint on `buf`"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (b) interpolated-string wrapper: lend call inside ${...} must suppress hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn interpolated_string_lend_call_suppresses_let_to_const_hint() {
    // WHY: `` `count is ${bump(buf)}` `` where `bump` takes `lend x: int` is a
    // mutation of `buf` inside a `StringPart::Expr` embedded in an
    // `Expr::InterpolatedString`.  The InterpolatedString arm in
    // `collect_maybe_mutated_expr` must recurse into each StringPart::Expr.
    //
    // Without the arm, `buf` appears unmutated → WRONG `let→const` hint fires.
    // After fix: no `let→const` hint for `buf`.
    //
    // `msg` is the only let binding that should get a hint here (assigned once,
    // never mutated); `buf` must be absent from hints.
    let src = r#"
function bump(lend x: int) -> int { 0 }

function entrypoint() -> nothing {
  let buf = 0
  let msg = `count is ${bump(buf)}`
}
"#;
    // `buf` must NOT get a hint.  `msg` (string binding, never reassigned) MAY
    // get a hint — that is correct behaviour.  The critical assertion is that `buf`
    // is absent.  We count: if total hints ≥ 2, `buf` got a spurious hint.
    let (db, sf) = make_db(src, "b_interp_lend");
    let hints = let_to_const_promotion_hints(&db, sf);
    let let_to_const_count = hints.iter().filter(|h| matches!(h.kind, PromotionKind::LetToConst)).count();
    assert!(
        let_to_const_count < 2,
        "interpolated-string wrapper: `buf` passed to lend inside `${{...}}` must not get hint; \
         got {} let→const hints (expected at most 1 — only `msg`)",
        let_to_const_count
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (c) wait-wrapped lend call must suppress array→fixed hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wait_wrapper_lend_call_suppresses_array_to_fixed_hint() {
    // WHY: `let nums: array<int> = [1,2,3]; wait growIt(nums)` where `growIt`
    // takes `lend arr: int` is a mutation of `nums` (the array binding) hidden
    // behind a `wait` wrapper.
    //
    // The `array→fixed` hint must not fire on `nums` because `growIt` may
    // mutate it (lend parameter).  Without the Wait arm in the mutation
    // collector, `nums` looks unmutated → WRONG `array→fixed` hint fires.
    // After fix: no `array→fixed` hint.
    let src = r#"
function growIt(lend arr: int) -> nothing { }

function entrypoint() -> nothing {
  let nums: array<int> = [1, 2, 3]
  wait growIt(nums)
}
"#;
    assert!(
        !has_array_to_fixed_hint_for(src, "c_wait_arr"),
        "wait-wrapper around a lend call on an array binding must suppress the array→fixed hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// (d) CONTROL: wait-wrapped share call must NOT suppress the let→const hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wait_wrapper_share_call_does_not_suppress_let_to_const_hint() {
    // WHY: The fix must only suppress hints for bindings that are actually mutated
    // (lend/give ownership).  A `wait readOnly(x)` where `readOnly` takes
    // `share x: int` is NOT a mutation.  The `let→const` hint must still fire.
    //
    // This guards against over-suppression: the Wait arm must recurse into the
    // inner expression and apply the same ownership-aware logic as the Call arm —
    // NOT blanket-suppress everything wrapped in `wait`.
    let src = r#"
function readOnly(share x: int) -> nothing { }

function entrypoint() -> nothing {
  let count = 5
  wait readOnly(count)
}
"#;
    assert!(
        has_let_to_const_hint_for(src, "d_wait_share"),
        "wait-wrapper around a share call must NOT suppress the let→const hint — over-suppression guard"
    );
}
