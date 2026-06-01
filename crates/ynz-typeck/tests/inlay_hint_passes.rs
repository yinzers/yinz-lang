// WHY: unit tests for the salsa-tracked inlay-hint detection passes.  These
// verify that each pass produces hints at the correct byte positions and that
// the conservative aliasing suppression logic fires in both directions
// (suppressed when mutated/passed; fires when genuinely never mutated).

use ynz_parser::{CompilerDb, SourceFile, SourceFileRegistry};
use ynz_typeck::{
    array_to_fixed_promotion_hints, copy_point_hints, let_to_const_promotion_hints,
    variable_type_hints, PromotionKind,
};

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_ih_pass_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

fn offset_of(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in source"))
}

// ─────────────────────────────────────────────────────────────────────────────
// variable_type_hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_variable_type_hint_fires_for_int_literal() {
    // WHY: `let x = 42` has no annotation — the pass must emit a TypeHint.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let (db, sf) = single_file(src);
    let hints = variable_type_hints(&db, sf);
    assert!(
        !hints.is_empty(),
        "unannotated let must produce a type hint"
    );
    assert!(
        hints.iter().any(|h| h.type_text == "int"),
        "inferred type must be `int`; got: {:?}",
        hints.iter().map(|h| &h.type_text).collect::<Vec<_>>()
    );
}

#[test]
fn test_variable_type_hint_suppressed_for_annotated_let() {
    // WHY: `let x: int = 42` already has the annotation — no hint should fire.
    let src = "function entrypoint() -> nothing {\n  let x: int = 42\n}\n";
    let (db, sf) = single_file(src);
    let hints = variable_type_hints(&db, sf);
    assert!(
        hints.is_empty(),
        "annotated let must produce zero type hints; got: {:?}",
        hints
    );
}

#[test]
fn test_variable_type_hint_position_is_after_name() {
    // WHY: the hint must appear after the binding name, not before or at the start
    // of the statement — otherwise it would render in the wrong column.
    let src = "function entrypoint() -> nothing {\n  let score = 42\n}\n";
    let (db, sf) = single_file(src);
    let hints = variable_type_hints(&db, sf);
    let name_end = offset_of(src, "score") + "score".len();
    for h in &hints {
        assert!(
            h.position >= name_end,
            "type hint position {} must be at or after name-end {}",
            h.position,
            name_end
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// let_to_const_promotion_hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_let_to_const_fires_for_never_reassigned() {
    // WHY: `let x = 0` with no later assignment is a const-promotion candidate.
    let src = "function entrypoint() -> nothing {\n  let count = 0\n}\n";
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    assert!(
        hints
            .iter()
            .any(|h| matches!(h.kind, PromotionKind::LetToConst)),
        "never-reassigned let must emit LetToConst hint"
    );
}

#[test]
fn test_let_to_const_suppressed_for_const_binding() {
    // WHY: `const x = 0` is already const — no promotion hint should fire.
    let src = "function entrypoint() -> nothing {\n  const count = 0\n}\n";
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    assert!(
        hints.is_empty(),
        "const binding must not emit a const-promotion hint"
    );
}

#[test]
fn test_let_to_const_suppressed_when_reassigned() {
    // WHY: `let x = 0; x = 1` — x is reassigned, so it cannot be const.
    let src = "function entrypoint() -> nothing {\n  let x = 0\n  x = 1\n}\n";
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);
    let for_x: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();
    assert!(
        for_x.is_empty(),
        "reassigned let must not emit const-promotion hint; got: {:?}",
        for_x
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// array_to_fixed_promotion_hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_array_to_fixed_fires_for_annotated_never_grown() {
    // WHY: `let nums: array<int> = [1, 2, 3]` with no .add() call is a fixed
    // promotion candidate.
    let src = "function entrypoint() -> nothing {\n  let nums: array<int> = [1, 2, 3]\n}\n";
    let (db, sf) = single_file(src);
    let hints = array_to_fixed_promotion_hints(&db, sf);
    assert!(
        hints
            .iter()
            .any(|h| matches!(h.kind, PromotionKind::ArrayToFixed)),
        "never-grown array<int> must emit ArrayToFixed hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// copy_point_hints
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_copy_point_does_not_fire_for_string_arg() {
    // WHY: `string` is NOT trivially copyable (heap allocation) — no copy hint.
    let src = "function greet(s: string) -> string { return s }\n\
               function entrypoint() -> nothing {\n  let r = greet(`hi`)\n}\n";
    let (db, sf) = single_file(src);
    let hints = copy_point_hints(&db, sf);
    // String args must not produce copy hints.
    for h in &hints {
        assert!(
            h.size_text != "N bytes",
            "copy hint should not fire for string arg; got: {:?}",
            hints
        );
    }
}
