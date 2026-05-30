// WHY: Replacement-category promotion hints (array→fixed, let→const) must render
// at the end of the statement, not at span.start (the `let` keyword position).
// Per .claude/rules/inference.md "Replacement" category: the decorated token gets
// a visual decoration; the annotation lives in the hover tooltip; positioning the
// hint at span.start (mid-statement, on the keyword) is wrong.  These tests
// assert that the hint byte position is ≥ stmt_span.end (past the value expression)
// and ≤ end-of-line, and that a trailing `//` comment causes the hint to sit
// just before the comment rather than after it.  The adversarial test pins the
// string-literal-`//` quality gate: a `//` inside a string literal must NOT be
// mistaken for a comment start.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{array_to_fixed_promotion_hints, let_to_const_promotion_hints, PromotionKind};

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    // Path includes src.len() so parallel tests don't collide on the same salsa key.
    let path = format!("/tmp/ynz_ih_pos_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

fn offset_of(src: &str, needle: &str) -> usize {
    src.find(needle).unwrap_or_else(|| panic!("{needle:?} not found in source"))
}

// ─────────────────────────────────────────────────────────────────────────────
// let→const: position is end-of-statement, not span.start
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn let_to_const_hint_position_is_end_of_statement_not_span_start() {
    // WHY: The hint must sit at the end of the `let count = 0` statement so the
    // Replacement-category decoration appears where readers expect it (after the
    // initializer).  Position == span.start would put it on the `let` keyword —
    // wrong column, wrong UX.
    let src = "function entrypoint() -> nothing {\n  let count = 0\n}\n";
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);

    let let_kw_pos = offset_of(src, "let count");
    let stmt_end = offset_of(src, "let count = 0") + "let count = 0".len();

    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();

    assert!(
        !const_hints.is_empty(),
        "never-reassigned let must emit a LetToConst hint"
    );
    for h in &const_hints {
        assert!(
            h.position >= stmt_end,
            "hint position {} must be >= end-of-statement {} (not at let-keyword {})",
            h.position,
            stmt_end,
            let_kw_pos
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// let→const: trailing comment — hint sits before the `//`
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn let_to_const_hint_positions_before_trailing_comment() {
    // WHY: When the statement has a trailing comment, the hint must appear just
    // before the `//`.  Positioning it after the `//` would embed the hint inside
    // the comment text in the IDE view — invisible to the user.
    let src = "function entrypoint() -> nothing {\n  let count = 0  // trailing\n}\n";
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);

    let comment_pos = offset_of(src, "// trailing");

    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();

    assert!(
        !const_hints.is_empty(),
        "never-reassigned let with trailing comment must still emit a LetToConst hint"
    );
    for h in &const_hints {
        assert!(
            h.position <= comment_pos,
            "hint position {} must be <= trailing-comment position {}",
            h.position,
            comment_pos
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ADVERSARIAL: `//` inside string literal must NOT be treated as a comment
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn let_to_const_hint_does_not_treat_url_slash_slash_as_comment() {
    // WHY: A string literal containing `//` (e.g. a URL) must not cause the
    // positioning helper to stop at the in-string `//` and place the hint
    // mid-statement.  The hint must appear at end-of-statement (≥ the end of the
    // initializer expression) unless there is a REAL trailing comment after the
    // closing string delimiter.  This is the quality gate from the Phase 4 plan:
    // string-literal `//` is not a comment; only a `//` that appears after all
    // string literals have closed counts.
    //
    // Source: `let msg = ` + backtick-string `http://x` + `  // real comment`.
    // The `//` inside the backtick string literal must NOT be treated as the
    // start of a comment; only the `//` after the string closes is the real
    // trailing comment that the hint must sit before.
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let msg = `http://x`  // real comment\n",
        "}\n"
    );
    let (db, sf) = single_file(src);
    let hints = let_to_const_promotion_hints(&db, sf);

    // The in-string `//` is at position of "http://x" inside the backtick string.
    let in_string_slash_pos = offset_of(src, "http://x") + "http:".len();
    // The real comment is at the `// real comment` after the closing backtick.
    let real_comment_pos = offset_of(src, "// real comment");

    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::LetToConst))
        .collect();

    assert!(
        !const_hints.is_empty(),
        "let with URL string and trailing real comment must emit a LetToConst hint"
    );
    for h in &const_hints {
        assert!(
            h.position >= in_string_slash_pos + 2,
            "hint position {} must be PAST the in-string // at {} (must not stop at in-string //)",
            h.position,
            in_string_slash_pos
        );
        assert!(
            h.position <= real_comment_pos,
            "hint position {} must be <= the real trailing comment at {}",
            h.position,
            real_comment_pos
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// array→fixed: position is end-of-statement, not span.start
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn array_to_fixed_hint_position_is_end_of_statement_not_span_start() {
    // WHY: Same Replacement-category positioning requirement as let→const.
    // span.start for an array<int> binding is the `let` keyword — placing the
    // hint there is wrong; it must be at end-of-statement.
    let src = "function entrypoint() -> nothing {\n  let nums: array<int> = [1, 2, 3]\n}\n";
    let (db, sf) = single_file(src);
    let hints = array_to_fixed_promotion_hints(&db, sf);

    let let_kw_pos = offset_of(src, "let nums");
    let stmt_end = offset_of(src, "[1, 2, 3]") + "[1, 2, 3]".len();

    let fixed_hints: Vec<_> = hints
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::ArrayToFixed))
        .collect();

    assert!(
        !fixed_hints.is_empty(),
        "never-grown array<int> must emit an ArrayToFixed hint"
    );
    for h in &fixed_hints {
        assert!(
            h.position >= stmt_end,
            "hint position {} must be >= end-of-statement {} (not at let-keyword {})",
            h.position,
            stmt_end,
            let_kw_pos
        );
    }
}
