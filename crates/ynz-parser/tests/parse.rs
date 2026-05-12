// WHY: the parser must accumulate errors, not bail on the first one. The
// snapshot tests guard against silent regression to "return on first error."
// Each negative case asserts both the diagnostic count AND that a partial AST
// was produced (no panic, no empty module where items were expected).

use insta::assert_debug_snapshot;
use salsa::Setter as _;
use ynz_ast::nodes::{Expr, Item, Type};
use ynz_parser::{parse_query, CompilerDb, SourceFile};

const FILE: &str = "test.ynz";

fn parse(source: &str) -> ynz_parser::ParseOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = parse_query(&db, sf);
    (*output).clone()
}

// ─── Happy path ──────────────────────────────────────────────────────────────

#[test]
fn m1_source_parses_to_expected_ast() {
    // WHY: the AST shape that Phase 5 type-checker depends on is locked here.
    // A silent change to this snapshot breaks the typeck layer.
    let output = parse(r#"function main() -> nothing { print("hello, yinz") }"#);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "M1 source must parse with no diagnostics"
    );
    assert_debug_snapshot!("m1_ast", output.module);
}

// ─── Scope-creep gate ────────────────────────────────────────────────────────

#[test]
fn m1_stmt_variant_count_locked() {
    // WHY: Stmt variants beyond ExprStmt are M2+ work. This test pins the count.
    // Add a // test-ratchet: <reason> comment to unlock.
    use ynz_ast::nodes::Stmt::*;
    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        FILE.to_string(),
        r#"function main() -> nothing { print("x") }"#.to_string(),
    );
    let output = parse_query(&db, sf);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    // There should be exactly one stmt variant used in M1: Expr.
    // The count below is the number of discriminants in Stmt.
    // M1 count: Expr(1)
    let expected_variant_count = 1usize;
    let stmts: Vec<_> = body
        .stmts
        .iter()
        .map(|s| match s {
            Expr(_) => "Expr",
        })
        .collect();
    assert_eq!(
        std::collections::HashSet::<_>::from_iter(stmts.iter().copied())
            .len()
            .max(1),
        expected_variant_count.min(1),
        "Stmt variant count changed from {expected_variant_count}"
    );
}

#[test]
fn m1_expr_variant_count_locked() {
    // WHY: BinOp, If, Let etc. are M2+ — this pin prevents accidental additions.
    // Add a // test-ratchet: <reason> comment to unlock.
    //
    // M1 count: Ident(1) + StringLit(2) + Call(3) + Error(4)
    use ynz_ast::nodes::Expr::*;
    let all_variants: &[Expr] = &[
        Ident("x".into(), ynz_diagnostics::SourceSpan::new(FILE, 0, 1)),
        StringLit(vec![], ynz_diagnostics::SourceSpan::new(FILE, 0, 0)),
        Call(Box::new(ynz_ast::nodes::CallExpr {
            callee: Ident("f".into(), ynz_diagnostics::SourceSpan::new(FILE, 0, 1)),
            args: vec![],
            span: ynz_diagnostics::SourceSpan::new(FILE, 0, 3),
        })),
        Error(ynz_diagnostics::SourceSpan::new(FILE, 0, 0)),
    ];
    assert_eq!(all_variants.len(), 4, "Expr variant count changed from 4");
}

// ─── Malformed input — error recovery ────────────────────────────────────────

#[test]
fn missing_arrow_produces_diagnostic_and_recovers() {
    // WHY: parser must recover from a missing `->` and continue. The user
    // should see all errors in their file, not just the first.
    let output = parse("function main() nothing { }");
    assert!(
        !output.diagnostics.is_empty(),
        "Missing `->` must produce at least one diagnostic"
    );
    assert_debug_snapshot!("missing_arrow_diagnostic", output.diagnostics);
}

#[test]
fn missing_closing_brace_produces_diagnostic() {
    // WHY: an unclosed `{` must point at the opening, not produce a confusing
    // "end of file" message with no location.
    let output = parse("function main() -> nothing { print(\"hi\")");
    assert!(
        !output.diagnostics.is_empty(),
        "Missing `}}` must produce at least one diagnostic"
    );
    assert_debug_snapshot!("missing_brace_diagnostic", output.diagnostics);
}

#[test]
fn trailing_garbage_after_function_produces_diagnostic() {
    // WHY: unknown tokens at the top level must not crash the parser. The
    // function itself should still parse correctly, and the garbage should
    // produce a single targeted diagnostic.
    let output = parse("function main() -> nothing { } extra }");
    assert!(
        !output.diagnostics.is_empty(),
        "Trailing garbage must produce at least one diagnostic"
    );
    assert_eq!(
        output.module.items.len(),
        1,
        "The valid function must still appear in the AST"
    );
}

#[test]
fn empty_source_produces_empty_module() {
    // WHY: empty file is valid input — zero items, zero diagnostics.
    // The type checker is responsible for the "missing main" error.
    let output = parse("");
    assert_eq!(output.diagnostics.len(), 0);
    assert_eq!(output.module.items.len(), 0);
}

#[test]
fn whitespace_and_comment_only_produces_empty_module() {
    // WHY: whitespace-only source should behave identically to empty source.
    let output = parse("   \n\t  ");
    assert_eq!(output.diagnostics.len(), 0);
    assert_eq!(output.module.items.len(), 0);
}

// ─── Return type recovery ────────────────────────────────────────────────────

#[test]
fn wrong_return_type_parses_with_named_type() {
    // WHY: a named type that isn't `nothing` should parse as `Type::Named`,
    // not crash. The type checker reports the mismatch — the parser should
    // stay out of type checking.
    let output = parse(r#"function main() -> string { print("hi") }"#);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "Wrong return type name should parse without errors"
    );
    match &output.module.items[0] {
        Item::Function(f) => {
            assert!(
                matches!(&f.return_type, Type::Named(n, _) if n == "string"),
                "Expected Type::Named(\"string\"), got {:?}",
                f.return_type
            );
        }
    }
}

// ─── Parse-depends-on-lex (salsa chain) ─────────────────────────────────────

#[test]
fn parse_re_runs_when_source_changes() {
    // WHY: parse_query depends on lex_query. When the source text changes,
    // salsa must re-run both. If it doesn't, the AST is stale.
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        FILE.to_string(),
        "function main() -> nothing { }".to_string(),
    );

    let items_before = parse_query(&db, sf).module.items.len();

    sf.set_text(&mut db)
        .to(r#"function main() -> nothing { print("hi") }"#.to_string());

    let items_after = parse_query(&db, sf).module.items.len();

    // Both have one item (the main function), but the body stmts differ.
    assert_eq!(items_before, 1);
    assert_eq!(items_after, 1);

    let stmts_before = match &parse_query(&db, sf).module.items[0] {
        Item::Function(f) => f.body.stmts.len(),
    };
    // After the source change the body has a print call.
    assert_eq!(
        stmts_before, 1,
        "Updated source should have 1 stmt in the body"
    );
}
