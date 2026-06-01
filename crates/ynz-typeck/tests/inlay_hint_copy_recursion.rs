// WHY: Tests for `collect_copy_hints_expr`'s recursion into nested call arguments.
//
// Bug 2.14: copy-point hints must fire for trivially-copyable values at every call
// depth, not just the outermost.  `outer(inner(n))` — `n` is passed to `inner`,
// which is itself passed as an arg to `outer`.  Before the fix, `collect_copy_hints_expr`
// only inspected top-level args of the outermost call, so `n` was never visited and
// got no copy hint.  After the fix, the walker recurses into each arg (mirroring
// `collect_ownership_hints_expr`), so `n` is found at any depth.
//
// AC1: A trivially-copyable value nested inside an inner call gets a copy hint.
// AC2: A trivially-copyable value at the outermost level gets EXACTLY ONE copy hint —
//      recursion must not double-emit for a top-level arg.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::copy_point_hints;

// ─────────────────────────────────────────────────────────────────────────────
// Helper — registers a single file with the salsa db.
// Path is unique per discriminator to avoid cross-test cache collisions.
// ─────────────────────────────────────────────────────────────────────────────

fn single_file(discriminator: &str, src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_copy_recursion_{}.ynz", discriminator);
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

// ─────────────────────────────────────────────────────────────────────────────
// AC1 — nested-call arg gets a copy hint (Bug 2.14)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn nested_call_arg_gets_copy_hint() {
    // WHY: `n` is passed to `inner`, which is itself the sole argument of `outer`.
    // The copy hint for `n` must fire even though `n` is not a direct arg of `outer`.
    // Before the fix, `collect_copy_hints_expr` only inspected top-level args of the
    // outermost call — `inner(n)` is the top-level arg and is NOT trivially copyable
    // (it's a call expression), so `n` was never reached and got no hint.
    let src = r#"
function inner(x: int) -> nothing { }
function outer(result: int) -> nothing { }

function entrypoint() -> nothing {
    let n: int = 5
    outer(inner(n))
}
"#;
    let (db, sf) = single_file("nested_arg", src);
    let hints = copy_point_hints(&db, sf);

    // `n` is trivially copyable (int, 8 bytes).  The recursion must find it.
    let n_pos = src.find("inner(n)").unwrap() + "inner(".len();
    let hint_at_n = hints.iter().find(|h| h.position == n_pos + "n".len());
    assert!(
        hint_at_n.is_some(),
        "Expected a copy hint at the position of `n` inside `inner(n)`, \
         but got hints: {:?} (looked for position {} = end of `n`)",
        hints,
        n_pos + "n".len(),
    );
    assert_eq!(
        hint_at_n.unwrap().size_text,
        "8 bytes",
        "`int` must render as `8 bytes`",
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// AC2 — top-level copyable arg gets exactly one copy hint (no duplicate from recursion)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn top_level_copyable_arg_gets_exactly_one_copy_hint() {
    // WHY: After adding recursion, `outer(n)` must still produce exactly ONE hint
    // for `n`.  The fix recurses into each arg after the top-level type-check; for a
    // plain `Expr::Ident`, the recursion body has no `Expr::Call` to match, so it
    // emits nothing.  This test proves the recursion does not double-emit.
    let src = r#"
function consume(x: int) -> nothing { }

function entrypoint() -> nothing {
    let n: int = 42
    consume(n)
}
"#;
    let (db, sf) = single_file("no_duplicate", src);
    let hints = copy_point_hints(&db, sf);

    // `n` is the only copyable arg.  Expect exactly one hint.
    let n_pos = src.find("consume(n)").unwrap() + "consume(".len();
    let hints_at_n: Vec<_> = hints
        .iter()
        .filter(|h| h.position == n_pos + "n".len())
        .collect();
    assert_eq!(
        hints_at_n.len(),
        1,
        "Top-level `n` must produce exactly one copy hint — \
         recursion must not emit a duplicate; got: {:?}",
        hints,
    );
}
