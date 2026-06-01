// WHY: Regression tests for Bug 2.6 — the `else =>` catch-all arm of `Stmt::Match`
// was silently dropped by all six inlay-hint walkers.  A binding mutated or an
// array grown exclusively inside the `else =>` arm received a false promotion hint
// because the mutation was never recorded.  These tests assert the corrected behavior:
// no promotion hint fires when the mutation evidence lives in the `else =>` arm.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{
    array_to_fixed_promotion_hints, let_to_const_promotion_hints, PromotionKind,
};

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_else_arm_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

// ─────────────────────────────────────────────────────────────────────────────
// let → const hints must be suppressed when mutation is in the else => arm
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn let_mutated_only_in_else_arm_does_not_get_let_to_const_hint() {
    // WHY: `count` is reassigned inside the `else =>` arm.  Before the fix,
    // collect_maybe_mutated_stmt dropped `else_arm` via `..`, so the reassignment
    // was invisible and the false "effectively const" hint fired.  After the fix,
    // the walker visits the else arm and records the mutation — no hint.
    let src = r#"
function entrypoint() -> nothing {
  let count = 0
  if (count) {
    1 => {}
    else => {
      count = 99
    }
  }
}
"#;
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    let const_hints_for_count: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();
    assert!(
        const_hints_for_count.is_empty(),
        "let binding mutated in else => arm must NOT get a let→const hint; got: {:?}",
        const_hints_for_count
    );
}

#[test]
fn let_not_mutated_in_any_arm_still_gets_let_to_const_hint() {
    // WHY: When the else => arm exists but does NOT mutate the binding, the
    // promotion hint should still fire.  Verifies the fix didn't over-suppress.
    let src = r#"
function entrypoint() -> nothing {
  let count = 0
  if (count) {
    1 => {}
    else => {
      let other = 5
    }
  }
}
"#;
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();
    assert!(
        !const_hints.is_empty(),
        "let binding NOT mutated in any arm should still get let→const hint; got nothing"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// array → fixed hints must be suppressed when the array is grown in else => arm
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn array_grown_in_else_arm_does_not_get_array_to_fixed_hint() {
    // WHY: `nums` has `.add()` called on it inside the `else =>` arm.  Before the
    // fix, collect_maybe_mutated_stmt's Stmt::Match arm dropped else_arm via `..`,
    // so the method call on `nums` was invisible.  The false array→fixed hint fired.
    // After the fix the walker visits the else arm and records `nums` as mutated.
    let src = r#"
function entrypoint() -> nothing {
  let nums: array<int> = [1, 2, 3]
  if (nums) {
    1 => {}
    else => {
      nums.add(4)
    }
  }
}
"#;
    let (db, sf) = single_file(src);
    let hints = array_to_fixed_promotion_hints(&db, sf);
    let fixed_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::ArrayToFixed))
        .collect();
    assert!(
        fixed_hints.is_empty(),
        "array grown via .add() in else => arm must NOT get array→fixed hint; got: {:?}",
        fixed_hints
    );
}

#[test]
fn array_not_grown_in_any_arm_still_gets_array_to_fixed_hint() {
    // WHY: Ensures the fix doesn't suppress correct promotion hints when the else
    // arm exists but does not grow the array.
    let src = r#"
function entrypoint() -> nothing {
  let nums: array<int> = [1, 2, 3]
  if (nums) {
    1 => {}
    else => {
      let x = 5
    }
  }
}
"#;
    let (db, sf) = single_file(src);
    let hints = array_to_fixed_promotion_hints(&db, sf);
    let fixed_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::ArrayToFixed))
        .collect();
    assert!(
        !fixed_hints.is_empty(),
        "array not grown in any arm should still get array→fixed hint; got nothing"
    );
}
