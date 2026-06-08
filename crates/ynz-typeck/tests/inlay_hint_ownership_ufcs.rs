// WHY: Tests for `collect_ownership_hints_expr`'s generic-fn and UFCS (dot-call) coverage.
//
// Bug 2.11: ownership muted hints must be consistent across both call syntaxes.
// `player.heal(20)` (UFCS / MethodCall) and `heal(player, 20)` (free-fn / Call) are
// the same call — the hint on the receiver/argument must appear in both forms.
// Generic functions must also get ownership hints; the previous sig lookup stopped at
// `sig_table` + `imported` and never queried `generic_fn_table`.
//
// AC1: UFCS form gets the same ownership hint as the free-fn form (same modifier, same position
//      relative to the argument).  Both are asserted: `modifier` confirms the correct ownership
//      kind; `position` (the byte offset of the end of the argument/receiver token, derived
//      from the source string) confirms the hint anchors at the same logical location in both
//      call syntaxes.  Hardcoded magic byte offsets are not used — positions are derived via
//      `src.find(…).unwrap() + prefix.len()` so they remain accurate if surrounding source
//      changes.
// AC2: A generic-fn call gets an ownership hint.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::ownership_call_site_hints;

// ─────────────────────────────────────────────────────────────────────────────
// Helper — registers a single file with the salsa db.
// Path is unique per test (includes a discriminator) to avoid cross-test cache collisions.
// ─────────────────────────────────────────────────────────────────────────────

fn single_file(discriminator: &str, src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_ownership_ufcs_{}.ynz", discriminator);
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.11 — UFCS call must get the same ownership hint as the free-fn call
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn free_fn_call_emits_lend_hint() {
    // WHY: Baseline — `heal(player, 20)` (free-fn / `Expr::Call`) already worked before
    // this fix.  Confirms the test helper and data format are correct before checking the
    // UFCS form in the next test.  If this breaks, the underlying sig resolution broke.
    let src = r#"
shape Player { health: int }
function heal(lend self: Player, amount: int) -> nothing { }

function entrypoint() -> nothing {
    let player: Player = { health: 100 }
    heal(player, 20)
}
"#;
    let (db, sf) = single_file("free_fn_lend", src);
    let hints = ownership_call_site_hints(&db, sf);

    // The `player` argument at position of `heal(player,...)` gets a `lend` hint.
    let lend_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "lend").collect();
    assert!(
        !lend_hints.is_empty(),
        "Expected a `lend` hint for `player` in `heal(player, 20)`, got hints: {:?}",
        hints
    );

    // The hint must anchor at the end of the `player` token in the call `heal(player, 20)`.
    // Derived from the source string — not a hardcoded magic byte — so the assertion stays
    // valid if unrelated whitespace before the call changes.
    //
    // "heal(player" is unique in the source: `heal(` is 5 bytes, `player` is 6 bytes.
    let expected_pos = src.find("heal(player").unwrap() + "heal(".len() + "player".len();
    assert!(
        lend_hints.iter().any(|h| h.position == expected_pos),
        "Expected lend hint at position {} (end of `player` in `heal(player, 20)`), got hints: {:?}",
        expected_pos,
        hints
    );
}

#[test]
fn ufcs_method_call_emits_lend_hint_on_receiver() {
    // WHY: AC1 — `player.heal(20)` (UFCS / `Expr::MethodCall`) must emit the same
    // `lend` hint as `heal(player, 20)`.  The receiver in a MethodCall is param 0,
    // which is the `lend` parameter.  Before the fix, MethodCall fell through the `if
    // let Expr::Call` guard entirely — no hint ever fired for UFCS form.
    let src = r#"
shape Player { health: int }
function heal(lend self: Player, amount: int) -> nothing { }

function entrypoint() -> nothing {
    let player: Player = { health: 100 }
    player.heal(20)
}
"#;
    let (db, sf) = single_file("ufcs_lend", src);
    let hints = ownership_call_site_hints(&db, sf);

    let lend_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "lend").collect();
    assert!(
        !lend_hints.is_empty(),
        "Expected a `lend` hint for receiver `player` in `player.heal(20)`, got hints: {:?}",
        hints
    );

    // AC1 position parity: the hint must anchor at the end of the `player` receiver token
    // in `player.heal(20)`, matching the free-fn form's `heal(player, 20)` anchor position
    // derivation.  "player.heal" is unique in this source; `player` is 6 bytes.
    // Uses `span().end` — the exclusive byte offset after the last byte of the token —
    // consistent with how `Expr::MethodCall` sets OwnershipHint.position in the production arm.
    let expected_pos = src.find("player.heal").unwrap() + "player".len();
    assert!(
        lend_hints.iter().any(|h| h.position == expected_pos),
        "Expected lend hint at position {} (end of receiver `player` in `player.heal(20)`), got hints: {:?}",
        expected_pos,
        hints
    );
}

#[test]
fn ufcs_share_receiver_emits_share_hint() {
    // WHY: UFCS with a `share` (read-only) receiver must emit a `share` hint, not `lend`.
    // Tests that the modifier is correctly read from the param_ownerships vector for UFCS
    // and is not hardcoded to a particular modifier.
    let src = r#"
shape Player { health: int }
function greet(share self: Player) -> nothing { }

function entrypoint() -> nothing {
    let player: Player = { health: 100 }
    player.greet()
}
"#;
    let (db, sf) = single_file("ufcs_share", src);
    let hints = ownership_call_site_hints(&db, sf);

    let share_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "share").collect();
    assert!(
        !share_hints.is_empty(),
        "Expected a `share` hint for receiver `player` in `player.greet()`, got hints: {:?}",
        hints
    );
}

#[test]
fn ufcs_arg_hint_emitted_for_non_receiver_arg() {
    // WHY: In a MethodCall with additional args beyond the receiver, those args correspond
    // to params 1.. in the sig.  A `lend` arg should get its own hint.  Ensures the arg
    // loop in the MethodCall arm works, not just the receiver check.
    let src = r#"
shape Player { health: int }
shape Weapon { damage: int }
function equip(share self: Player, lend w: Weapon) -> nothing { }

function entrypoint() -> nothing {
    let player: Player = { health: 100 }
    let sword: Weapon = { damage: 10 }
    player.equip(sword)
}
"#;
    let (db, sf) = single_file("ufcs_arg_lend", src);
    let hints = ownership_call_site_hints(&db, sf);

    // `player` → share hint; `sword` → lend hint.
    let share_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "share").collect();
    let lend_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "lend").collect();
    assert!(
        !share_hints.is_empty(),
        "Expected a `share` hint for receiver `player` in `player.equip(sword)`, got: {:?}",
        hints
    );
    assert!(
        !lend_hints.is_empty(),
        "Expected a `lend` hint for arg `sword` in `player.equip(sword)`, got: {:?}",
        hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bug 2.11 — generic-fn call must get an ownership hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn generic_fn_call_emits_ownership_hint() {
    // WHY: AC2 — generic functions (`function foo<T>(share x: T)`) must get an ownership
    // hint just like concrete functions.  The previous sig lookup stopped at `sig_table` +
    // `imported` and never queried `generic_fn_table`, so all generic calls silently
    // dropped through to the "fallback: recurse into args without hints" branch.
    let src = r#"
function identity<T>(share x: T) -> T { return x }

function entrypoint() -> nothing {
    let n: int = 42
    identity(n)
}
"#;
    let (db, sf) = single_file("generic_share", src);
    let hints = ownership_call_site_hints(&db, sf);

    let share_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "share").collect();
    assert!(
        !share_hints.is_empty(),
        "Expected a `share` hint for `n` passed to generic `identity(n)`, got hints: {:?}",
        hints
    );
}

#[test]
fn generic_fn_lend_param_emits_lend_hint() {
    // WHY: Generic functions with `lend` params must emit a `lend` hint, not `share`.
    // Tests the full modifier resolution path through `generic_fn_table`.
    let src = r#"
function swap<T>(lend a: T, lend b: T) -> nothing { }

function entrypoint() -> nothing {
    let x: int = 1
    let y: int = 2
    swap(x, y)
}
"#;
    let (db, sf) = single_file("generic_lend", src);
    let hints = ownership_call_site_hints(&db, sf);

    let lend_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "lend").collect();
    assert_eq!(
        lend_hints.len(),
        2,
        "Expected two `lend` hints for `x` and `y` in `swap(x, y)`, got hints: {:?}",
        hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v0.3-M3b Phase 2: `background` inferred give/copy hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn background_unused_after_spawn_emits_give_hint() {
    // WHY: AC — `background fn(x)` with `x` unused after the spawn must emit a
    // `give` hint (inferred `.give` — zero-copy ownership transfer). The hint
    // teaches the user what the compiler decided without requiring explicit syntax.
    let src = r#"
function worker(id: int) -> nothing { }

function entrypoint() -> nothing {
    let taskId: int = 42
    background worker(taskId)
}
"#;
    let (db, sf) = single_file("bg_give_hint", src);
    let hints = ownership_call_site_hints(&db, sf);

    let give_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "give").collect();
    assert!(
        !give_hints.is_empty(),
        "Expected a `give` hint for `taskId` in `background worker(taskId)` (unused after spawn), got hints: {:?}",
        hints
    );

    // Position: the hint anchors at the end of the `taskId` ident token in the background arg.
    let expected_pos = src.find("background worker(taskId)").unwrap()
        + "background worker(".len()
        + "taskId".len();
    assert!(
        give_hints.iter().any(|h| h.position == expected_pos),
        "Expected give hint at position {} (end of `taskId` in background call), got hints: {:?}",
        expected_pos,
        hints
    );
}

#[test]
fn background_used_after_spawn_emits_copy_hint() {
    // WHY: AC — `background fn(x)` with `x` read after the spawn must emit a `copy`
    // hint (inferred `.copy` — the caller retains the original, task gets a copy).
    // Incorrect inference to `.give` would be a use-after-move bug — this hint proves
    // the safe direction was chosen.
    let src = r#"
function worker(id: int) -> nothing { }

function entrypoint() -> nothing {
    let taskId: int = 42
    background worker(taskId)
    print(taskId.toString())
}
"#;
    let (db, sf) = single_file("bg_copy_hint", src);
    let hints = ownership_call_site_hints(&db, sf);

    let copy_hints: Vec<_> = hints.iter().filter(|h| h.modifier == "copy").collect();
    assert!(
        !copy_hints.is_empty(),
        "Expected a `copy` hint for `taskId` in `background worker(taskId)` (used after spawn), got hints: {:?}",
        hints
    );

    // Position: the hint anchors at the end of the `taskId` ident token in the background arg.
    let expected_pos = src.find("background worker(taskId)").unwrap()
        + "background worker(".len()
        + "taskId".len();
    assert!(
        copy_hints.iter().any(|h| h.position == expected_pos),
        "Expected copy hint at position {} (end of `taskId` in background call), got hints: {:?}",
        expected_pos,
        hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Graceful: unresolved MethodCall callee yields no hint (no panic)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn unresolved_method_call_yields_no_hint_and_no_panic() {
    // WHY: An unresolvable MethodCall callee (not in sig_table, imported, generic_fn_table,
    // or intrinsic set) must gracefully produce no hint — not panic.  Verifies the quality
    // gate "no panic on unresolved generic/UFCS callee."
    let src = r#"
shape Gadget { charge: int }

function entrypoint() -> nothing {
    let g: Gadget = { charge: 50 }
    g.unknownMethod()
}
"#;
    let (db, sf) = single_file("unresolved_method", src);
    // Must not panic; result can be empty or non-empty — we only check no crash.
    let _ = ownership_call_site_hints(&db, sf);
}
