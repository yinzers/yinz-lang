// WHY: Regression tests for Bug 2.7 — `collect_maybe_mutated_stmt` only unwrapped
// one level of `Expr::FieldAccess` / `Expr::IndexAccess` to find the root binding.
// `player.address.street = "x"` has a two-level FieldAccess chain; the old code
// found `address` (an intermediate receiver), not `player` (the root), so `player`
// was not recorded in the mutated set and received a false `let→const` hint.
// Similarly, `arr[i][j] = v` has a nested IndexAccess receiver; the old code found
// `arr[i]` (IndexAccess), not `arr` (Ident), leaving `arr` untracked.
//
// The fix introduces `root_ident` — a helper that follows the
// FieldAccess/IndexAccess chain all the way to the `Expr::Ident` leaf — and uses it
// for both `FieldAssign.target` and `IndexAssign.receiver`.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{let_to_const_promotion_hints, PromotionKind};

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_nested_assign_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

// ─────────────────────────────────────────────────────────────────────────────
// Nested FieldAssign: player.address.street = "x" must mark `player` mutated
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn nested_field_assign_marks_root_binding_mutated_no_let_to_const_hint() {
    // WHY: Before the fix, `player.address.street = "x"` only recorded `address`
    // (the intermediate FieldAccess receiver), not `player` (the root Ident).
    // The false `let→const` hint fired on `player`.  After the fix, `root_ident`
    // follows the chain to `player`, records it mutated, and the hint is suppressed.
    let src = r#"
function entrypoint() -> nothing {
  let player = { name: "Carey", address: { street: "Federal St", city: "Pittsburgh" } }
  player.address.street = "Mon Wharf"
}
"#;
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    let const_hints_for_player: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();
    assert!(
        const_hints_for_player.is_empty(),
        "binding mutated via nested field path must NOT get a let→const hint; got: {:?}",
        const_hints_for_player
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Nested IndexAssign: arr[i][j] = v must mark `arr` mutated
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn nested_index_assign_marks_root_binding_mutated_no_let_to_const_hint() {
    // WHY: Before the fix, `arr[i][j] = v` had receiver = `IndexAccess { receiver:
    // Ident("arr"), index: Ident("i") }`.  The old code matched only `Expr::Ident`
    // at the top level of `receiver`, which failed (got IndexAccess), so `arr` was
    // never recorded as mutated and the false hint fired.  The fix follows the
    // IndexAccess chain to the root Ident("arr") and records it.
    let src = r#"
function entrypoint() -> nothing {
  let arr: array<array<int>> = [[1, 2], [3, 4]]
  let i = 0
  let j = 1
  arr[i][j] = 99
}
"#;
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    // `arr` is mutated via nested index assign — must have NO let→const hint.
    // `i` and `j` are never mutated — they SHOULD get hints, which proves the
    // fix didn't over-suppress.
    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();
    // `arr` must not appear — we detect its hint by position; easier: just
    // confirm at least one hint fires (for `i` or `j`) and none for `arr`.
    // We cannot easily filter by name without position info, so assert the
    // hint count is < 3 (arr, i, j are the three lets; arr must be absent).
    assert!(
        !const_hints.is_empty(),
        "`i` and `j` are never mutated and SHOULD get hints (proves no over-suppression)"
    );
    // The full fixture has exactly 3 let bindings (arr, i, j).
    // If arr were wrongly included, count would be 3; correct is 2 (i and j).
    assert_eq!(
        const_hints.len(),
        2,
        "exactly `i` and `j` should get hints (arr is mutated via arr[i][j] = 99); got: {:?}",
        const_hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Regression guard: single-level field assign still works after refactor
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn single_level_field_assign_still_marks_binding_mutated() {
    // WHY: Ensures the refactor from `if let Expr::Ident` to `root_ident` didn't
    // break the existing one-level case.  `player.health = 5` must still record
    // `player` as mutated (no let→const hint on `player`).
    let src = r#"
function entrypoint() -> nothing {
  let player = { name: "Carey", health: 100 }
  player.health = 50
}
"#;
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();
    assert!(
        const_hints.is_empty(),
        "binding mutated via single-level field assign must NOT get a let→const hint; got: {:?}",
        const_hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Nested MethodCall receiver: player.address.heal(5) must mark `player` mutated
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn nested_method_call_receiver_marks_root_binding_mutated() {
    // WHY: `player.address.heal(5)` is an `Expr::MethodCall` whose receiver is
    // `Expr::FieldAccess { receiver: Ident("player"), field: "address" }`.  The
    // old `if let Expr::Ident = receiver` pattern failed on the FieldAccess level,
    // leaving `player` unrecorded in the mutated set and triggering a spurious
    // let→const hint.  `root_ident` follows the chain to `player` and records it.
    //
    // `addr` is genuinely never reassigned after construction and legitimately
    // receives a LetToConst hint — the assertion targets only `player`.
    let src = r#"
shape Address { street: string }
shape Player { name: string, address: Address, health: int }
function heal(lend self: Player, amount: int) -> nothing {
  self.health = self.health + amount
}
function entrypoint() -> nothing {
  let addr = { street: "Federal St" }
  let player = { name: "Carey", address: addr, health: 80 }
  player.address.heal(5)
}
"#;
    // Byte offsets of `let` keywords in the source above:
    //   `let addr`   starts at 230
    //   `let player` starts at 268
    // (verified: python3 re.finditer on the raw string above)
    let player_let_position: usize = 268;

    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    let player_const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst) && h.position == player_let_position)
        .collect();
    assert!(
        player_const_hints.is_empty(),
        "binding mutated via chained method-call receiver must NOT get a let→const hint; got: {:?}",
        player_const_hints
    );
}
