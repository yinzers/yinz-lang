use std::time::{Duration, Instant};

fn parse_within_timeout(source: &str) {
    let start = Instant::now();
    let db = ynz_parser::CompilerDb::default();
    let sf = ynz_parser::SourceFile::new(&db, "test.ynz".into(), source.to_string().into());
    let _ = ynz_parser::parse_query(&db, sf);
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "Parser hung (> 5s) on input: {:?}",
        source
    );
}

// WHY: Regression test for the parse_block infinite-loop bug. Token::Function and
// Token::Options inside a function body caused parse_atom to return Expr::Error
// without consuming the token; parse_block re-entered the loop on the same token
// forever. Fixed by a forward-progress guarantee in parse_block's default arm.
// Without this test a future regression would hang CI silently.
#[test]
fn stmt_boundary_token_inside_block_does_not_hang() {
    // Token::Function inside a body — the original reproducer from v0.2-M1/v0.3-M1.
    parse_within_timeout(
        "function outer() -> nothing {\n  function inner() -> nothing { }\n}",
    );
    // Token::Options inside a body — same mechanism.
    parse_within_timeout("function test() -> nothing {\n  options Foo { a, b }\n}");
    // Nested function-inside-block inside another block.
    parse_within_timeout(
        "function test() -> nothing {\n  if (true) {\n    function nested() -> nothing { }\n  }\n}",
    );
    // The async-banned-keyword trigger from v0_2_m1_errors.ynz gallery.
    parse_within_timeout(
        "function asyncExample() -> nothing {\n  async function inner() -> nothing { }\n}",
    );
}

// WHY: Confirm the parser terminates on both known-hanging gallery files.
// These were skipped in idempotency / semantic_roundtrip / mass_rewrite tests
// because the hang reproducibly fired. This test re-adds them as active
// regression coverage so they can never silently re-hang.
#[test]
fn gallery_files_do_not_hang() {
    let gallery_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .join("examples/primantis-orders");

    for name in &["m1_errors.ynz", "v0_2_m1_errors.ynz"] {
        let path = gallery_root.join(name);
        if !path.exists() {
            continue; // file may not be present in all worktrees
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("could not read {name}"));
        let start = Instant::now();
        let db = ynz_parser::CompilerDb::default();
        let sf = ynz_parser::SourceFile::new(&db, name.to_string().into(), source.into());
        let _ = ynz_parser::parse_query(&db, sf);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Parser hung (> 5s) on gallery file: {name}"
        );
    }
}
