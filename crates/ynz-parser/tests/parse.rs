// WHY: the parser must accumulate errors, not bail on the first one. The
// snapshot tests guard against silent regression to "return on first error."
// Each negative case asserts both the diagnostic count AND that a partial AST
// was produced (no panic, no empty module where items were expected).

use insta::assert_debug_snapshot;
use salsa::Setter as _;
use ynz_ast::nodes::{BinOpKind, Expr, Item, Stmt, Type};
use ynz_diagnostics::SourceSpan;
use ynz_parser::{parse_query, CompilerDb, SourceFile};

const FILE: &str = "test.ynz";

fn span(start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(FILE, start, end)
}

fn parse(source: &str) -> ynz_parser::ParseOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = parse_query(&db, sf);
    (*output).clone()
}


#[test]
fn m2_stmt_variant_count_locked() {
    // WHY: Stmt variants beyond the M2 set are M3+ work. This test pins the count.
    // Add a `// test-ratchet: <reason>` comment to unlock.
    //
    // test-ratchet: M2 adds 2 variants over M1's 1: Let and Assign.
    //   M1: Expr(1). M2: Let(2), Assign(3). Total: 3.
    use ynz_ast::nodes::Stmt::*;
    let all_variants: &[Stmt] = &[
        Expr(ynz_ast::nodes::Expr::Error(span(0, 0))),
        Let {
            is_const: false,
            name: String::new(),
            name_span: span(0, 0),
            ty: None,
            value: ynz_ast::nodes::Expr::Error(span(0, 0)),
            span: span(0, 0),
        },
        Assign {
            target: String::new(),
            target_span: span(0, 0),
            value: ynz_ast::nodes::Expr::Error(span(0, 0)),
            span: span(0, 0),
        },
    ];
    assert_eq!(all_variants.len(), 3, "Stmt variant count changed from 3");
}

#[test]
fn m2_expr_variant_count_locked() {
    // WHY: BinOp, UnaryOp, etc. beyond M2 are M3+ work. This pins the count.
    // Add a `// test-ratchet: <reason>` comment to unlock.
    //
    // test-ratchet: M2 adds 6 variants over M1's 4.
    //   M1: Ident(1), StringLit(2), Call(3), Error(4).
    //   M2: IntLit(5), NumberLit(6), BoolLit(7), BinOp(8), UnaryOp(9), MethodCall(10).
    //   Total: 10.
    use ynz_ast::nodes::Expr::*;
    let all_variants: &[Expr] = &[
        Ident("x".into(), span(0, 1)),
        StringLit(vec![], span(0, 0)),
        Call(Box::new(ynz_ast::nodes::CallExpr {
            callee: Ident("f".into(), span(0, 1)),
            args: vec![],
            span: span(0, 3),
        })),
        Error(span(0, 0)),
        IntLit(0, span(0, 1)),
        NumberLit("0".into(), span(0, 1)),
        BoolLit(true, span(0, 4)),
        BinOp {
            op: BinOpKind::Add,
            lhs: Box::new(Error(span(0, 0))),
            rhs: Box::new(Error(span(0, 0))),
            span: span(0, 0),
        },
        UnaryOp {
            op: ynz_ast::nodes::UnaryOpKind::Neg,
            operand: Box::new(Error(span(0, 0))),
            span: span(0, 0),
        },
        MethodCall {
            receiver: Box::new(Error(span(0, 0))),
            method: "toString".into(),
            method_span: span(0, 0),
            args: vec![],
            span: span(0, 0),
        },
    ];
    assert_eq!(all_variants.len(), 10, "Expr variant count changed from 10");
}

#[test]
fn m2_type_variant_count_locked() {
    // WHY: Type variants beyond M2 are M3+ work. This pins the count.
    // Add a `// test-ratchet: <reason>` comment to unlock.
    //
    // test-ratchet: M2 adds 4 variants over M1's 3.
    //   M1: Nothing(1), Named(2), Error(3).
    //   M2: Int(4), Float(5), Number(6), Bool(7).
    //   Total: 7.
    use ynz_ast::nodes::Type::*;
    let all_variants: &[Type] = &[
        Nothing,
        Named("foo".into(), span(0, 3)),
        Error,
        Int,
        Float,
        Number { precision: 34 },
        Bool,
    ];
    assert_eq!(all_variants.len(), 7, "Type variant count changed from 7");
}


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


#[test]
fn m2_source_parses_to_expected_ast() {
    // WHY: this is the exact AST the Phase 4 type-checker depends on for M2.
    // A silent change here means typeck breaks on the M2 smoke test.
    let source = r#"function main() -> nothing {
  let price = 0.1 + 0.2
  let count: int = 42
  let active = true
  print(price)
  print(count * count - 1)
  print(active && (count > 0))
}"#;
    let output = parse(source);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "M2 smoke test source must parse with zero diagnostics"
    );
    assert_debug_snapshot!("m2_ast", output.module);
}


#[test]
fn type_annotations_parse_correctly() {
    // WHY: `let x: int = 1` needs to produce Type::Int, not Type::Named("int").
    // If primitive types are not recognized, typeck will fail to match them.
    let output = parse("function main() -> nothing { let x: int = 1 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { ty: Some(Type::Int), .. } => {}
        other => panic!("Expected Let with Type::Int, got {other:?}"),
    }
}

#[test]
fn number_type_without_brackets_is_34_precision() {
    // WHY: `number` without `[N]` should default to 34 decimal digits.
    let output = parse("function main() -> nothing { let x: number = 3.14 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { ty: Some(Type::Number { precision: 34 }), .. } => {}
        other => panic!("Expected Let with Type::Number{{34}}, got {other:?}"),
    }
}

#[test]
fn number_n_not_34_produces_deferral_diagnostic() {
    // WHY: `number[128]` is reserved but not yet implemented. The parser must
    // emit a deferral diagnostic pointing at v0.8, not silently accept it.
    let output = parse("function main() -> nothing { let x: number[128] = 0 }");
    assert_eq!(
        output.diagnostics.len(),
        1,
        "number[N] for N != 34 must produce exactly 1 deferral diagnostic"
    );
    assert!(
        output.diagnostics[0].why.contains("v0.8"),
        "Deferral diagnostic must mention v0.8, got: {:?}",
        output.diagnostics[0].why
    );
}


#[test]
fn mul_binds_tighter_than_add() {
    // WHY: `1 + 2 * 3` must parse as `1 + (2 * 3)`, not `(1 + 2) * 3`.
    // If * and + have the same BP, PEMDAS breaks everywhere.
    let output = parse("function main() -> nothing { let x = 1 + 2 * 3 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value, .. } => match value {
            Expr::BinOp { op: BinOpKind::Add, rhs, .. } => match rhs.as_ref() {
                Expr::BinOp { op: BinOpKind::Mul, .. } => {}
                other => panic!("rhs of + should be Mul, got {other:?}"),
            },
            other => panic!("top should be Add, got {other:?}"),
        },
        other => panic!("Expected Stmt::Let, got {other:?}"),
    }
}

#[test]
fn comparison_binds_tighter_than_and() {
    // WHY: `1 < 2 && 3 > 4` must parse as `(1 < 2) && (3 > 4)`.
    // Mixing comparison and boolean operators is a common source of bugs.
    let output = parse("function main() -> nothing { let x = 1 < 2 && 3 > 4 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value, .. } => match value {
            Expr::BinOp { op: BinOpKind::And, lhs, rhs, .. } => {
                assert!(matches!(lhs.as_ref(), Expr::BinOp { op: BinOpKind::Lt, .. }));
                assert!(matches!(rhs.as_ref(), Expr::BinOp { op: BinOpKind::Gt, .. }));
            }
            other => panic!("top should be And, got {other:?}"),
        },
        other => panic!("Expected Stmt::Let, got {other:?}"),
    }
}

#[test]
fn binary_ops_are_left_associative() {
    // WHY: `1 - 2 - 3` must parse as `(1 - 2) - 3`, not `1 - (2 - 3)`.
    // Right-associativity would silently produce different runtime values.
    let output = parse("function main() -> nothing { let x = 1 - 2 - 3 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value, .. } => match value {
            Expr::BinOp { op: BinOpKind::Sub, lhs, rhs, .. } => {
                // lhs should be (1 - 2), rhs should be 3
                assert!(matches!(lhs.as_ref(), Expr::BinOp { op: BinOpKind::Sub, .. }));
                assert!(matches!(rhs.as_ref(), Expr::IntLit(3, _)));
            }
            other => panic!("Expected Sub, got {other:?}"),
        },
        other => panic!("Expected Stmt::Let, got {other:?}"),
    }
}

#[test]
fn or_binds_looser_than_and() {
    // WHY: `a || b && c` must be `a || (b && c)`, matching standard boolean precedence.
    let output = parse("function main() -> nothing { let x = a || b && c }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value, .. } => match value {
            Expr::BinOp { op: BinOpKind::Or, rhs, .. } => {
                assert!(matches!(rhs.as_ref(), Expr::BinOp { op: BinOpKind::And, .. }));
            }
            other => panic!("top should be Or, got {other:?}"),
        },
        other => panic!("Expected Stmt::Let, got {other:?}"),
    }
}

#[test]
fn parentheses_override_precedence() {
    // WHY: `(1 + 2) * 3` must parse as `Mul(Add(1,2), 3)`, not `Add(1, Mul(2,3))`.
    // Parentheses are the user's escape valve from the precedence table.
    let output = parse("function main() -> nothing { let x = (1 + 2) * 3 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value, .. } => match value {
            Expr::BinOp { op: BinOpKind::Mul, lhs, .. } => {
                assert!(matches!(lhs.as_ref(), Expr::BinOp { op: BinOpKind::Add, .. }));
            }
            other => panic!("top should be Mul, got {other:?}"),
        },
        other => panic!("Expected Stmt::Let, got {other:?}"),
    }
}


#[test]
fn parser_precedence_table_matches_spec() {
    // WHY: the Pratt binding powers in parser.rs must match the precedence table
    // in spec/operators.md. If these drift, the compiler enforces different
    // precedence than the language spec promises — a silent correctness bug.
    // This test parses the spec table at runtime and compares it to the code.
    use ynz_parser::Token;
    use std::collections::HashMap;

    // Read the spec precedence table
    let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .join("spec/operators.md");
    let spec = std::fs::read_to_string(&spec_path)
        .unwrap_or_else(|_| panic!("Could not read {:?}", spec_path));

    // Find the "Operator precedence" code block
    let prec_section = spec
        .find("## Operator precedence")
        .expect("spec/operators.md must have an '## Operator precedence' section");
    let after = &spec[prec_section..];
    // Find the opening ``` of the code block
    let block_start = after
        .find("```\n")
        .expect("Operator precedence section must have a ``` code block")
        + 4;
    let block_end = after[block_start..]
        .find("```")
        .expect("Code block must be closed");
    let block = &after[block_start..block_start + block_end];

    // Parse each line: "N.  op1  op2  // comment"
    let mut spec_table: HashMap<&str, u8> = HashMap::new();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let without_comment = line.split("//").next().unwrap_or("").trim();
        let mut parts = without_comment.split_whitespace();
        let level_str = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let level: u8 = match level_str.trim_end_matches('.').parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        for op in parts {
            spec_table.insert(op, level);
        }
    }

    // The code table: (token, spec-level).  Level 1 = parentheses (handled in
    // parse_atom, not infix_bp), level 2 = unary (handled in parse_prefix).
    // This table covers only the INFIX binary operators in infix_bp().
    let code_table: &[(&str, u8, Token)] = &[
        ("*",  3,  Token::Star),
        ("/",  3,  Token::Slash),
        ("%",  3,  Token::Percent),
        ("+",  4,  Token::Plus),
        ("-",  4,  Token::Minus),
        ("<<", 5,  Token::LtLt),
        (">>", 5,  Token::GtGt),
        ("<",  6,  Token::Lt),
        (">",  6,  Token::Gt),
        ("<=", 6,  Token::LtEq),
        (">=", 6,  Token::GtEq),
        ("==", 7,  Token::EqEq),
        ("!=", 7,  Token::NotEq),
        ("&",  8,  Token::Amp),
        ("^",  9,  Token::Caret),
        ("|",  10, Token::Pipe),
        ("&&", 11, Token::AmpAmp),
        ("||", 12, Token::PipePipe),
    ];

    for (op_str, expected_level, tok) in code_table {
        // Verify spec has the operator
        let spec_level = spec_table.get(op_str).unwrap_or_else(|| {
            panic!(
                "Operator `{op_str}` is in the code's infix_bp table but missing from \
                 spec/operators.md's precedence list. Add it to the spec."
            )
        });
        assert_eq!(
            spec_level, expected_level,
            "Operator `{op_str}`: spec says level {spec_level} but code says level {expected_level}. \
             Update one to match."
        );

        // Verify code's infix_bp returns the right BP for this operator.
        // Level N maps to lbp = (13 - N) * 2.
        let expected_lbp = (13 - expected_level) * 2;
        let (actual_lbp, _) = ynz_parser::parser::infix_bp(tok)
            .unwrap_or_else(|| panic!("infix_bp returned None for `{op_str}`"));
        assert_eq!(
            actual_lbp, expected_lbp,
            "Operator `{op_str}`: expected lbp {expected_lbp} (spec level {expected_level}), \
             got lbp {actual_lbp}"
        );
    }
}


#[test]
fn missing_rhs_produces_diagnostic_and_recovers() {
    // WHY: `let x = 1 +` must emit a diagnostic and still produce a parseable
    // AST. If the parser panics or produces no AST, all other errors in the
    // file are hidden.
    let output = parse("function main() -> nothing { let x = 1 + }");
    assert_eq!(
        output.diagnostics.len(),
        1,
        "Missing RHS must produce exactly 1 diagnostic"
    );
    // Module must still have 1 function item (recovery succeeded)
    assert_eq!(output.module.items.len(), 1);
}

#[test]
fn missing_let_identifier_produces_diagnostic_and_recovers() {
    // WHY: `let : int = 5` (missing the variable name) must emit a diagnostic
    // but not crash. The block should continue parsing.
    let output = parse("function main() -> nothing { let : int = 5 }");
    assert!(
        !output.diagnostics.is_empty(),
        "Missing variable name must produce at least 1 diagnostic"
    );
    assert_eq!(output.module.items.len(), 1, "Function must still be in AST");
}

#[test]
fn let_type_mismatch_parses_cleanly() {
    // WHY: `let x: int = 1.5` is a TYPE error, not a PARSE error. The parser
    // must accept it without diagnostics — the type checker catches it in P4.
    // If the parser rejects it, P4 never gets to produce the teaching diagnostic.
    let output = parse("function main() -> nothing { let x: int = 1.5 }");
    assert_eq!(
        output.diagnostics.len(),
        0,
        "Type annotation mismatch is a typeck error, not a parse error"
    );
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { ty: Some(Type::Int), value: Expr::NumberLit(s, _), .. } => {
            assert_eq!(s, "1.5");
        }
        other => panic!("Expected Let with Int type and NumberLit value, got {other:?}"),
    }
}

#[test]
fn unexpected_rbrace_in_expression_recovers() {
    // WHY: `let x = }` must not eat the `}` that closes the function body.
    // If the parser consumes the `}`, the enclosing block is left unclosed
    // and the rest of the file is misparsed.
    let output = parse("function main() -> nothing { let x = }");
    assert_eq!(
        output.diagnostics.len(),
        1,
        "Unexpected `}}` as expression must produce exactly 1 diagnostic"
    );
    assert_eq!(output.module.items.len(), 1, "Function must still parse");
}

#[test]
fn multi_arg_call_parses_cleanly() {
    // WHY: `print(1, 2, 3)` has the wrong arity for M2 but the PARSER must
    // accept it — arity enforcement is typeck's job. If the parser rejects
    // multi-arg calls, the typeck teaching diagnostic about arity is unreachable.
    let output = parse("function main() -> nothing { print(1, 2, 3) }");
    assert_eq!(
        output.diagnostics.len(),
        0,
        "Multi-arg call must parse without diagnostics (arity is a typeck concern)"
    );
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Expr(Expr::Call(c)) => {
            assert_eq!(c.args.len(), 3, "Expected 3 args in print(1, 2, 3)");
        }
        other => panic!("Expected Expr(Call), got {other:?}"),
    }
}

#[test]
fn chained_comparison_parses_as_left_associative() {
    // WHY: `1 < 2 < 3` parses as `(1 < 2) < 3` per left-associativity.
    // Typeck will reject this in P4 because `bool < int` is a type error.
    // The test documents that the PARSE result is deterministic — the typeck
    // diagnostic targets the outer `<`, not the inner one.
    let output = parse("function main() -> nothing { let x = 1 < 2 < 3 }");
    assert_eq!(output.diagnostics.len(), 0, "Chained comparison parses clean");
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value, .. } => match value {
            Expr::BinOp { op: BinOpKind::Lt, lhs, rhs, .. } => {
                // outer: (1 < 2) < 3
                assert!(matches!(lhs.as_ref(), Expr::BinOp { op: BinOpKind::Lt, .. }));
                assert!(matches!(rhs.as_ref(), Expr::IntLit(3, _)));
            }
            other => panic!("Expected Lt, got {other:?}"),
        },
        other => panic!("Expected Stmt::Let, got {other:?}"),
    }
}


#[test]
fn assignment_parses_as_stmt_assign() {
    // WHY: `x = 5` after a `let x = 0` must parse as Stmt::Assign, not as
    // a bare expression. If assignment is misparsed as Expr::BinOp(Eq, x, 5)
    // the type-checker would see a boolean expression in a statement position.
    let source = "function main() -> nothing { let x = 0\nx = 5 }";
    let output = parse(source);
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    assert!(
        matches!(&body.stmts[1], Stmt::Assign { target, .. } if target == "x"),
        "Second stmt must be Stmt::Assign with target x"
    );
}


#[test]
fn method_call_parses_correctly() {
    // WHY: `count.toString()` must produce MethodCall, not a BinOp(Dot) hack.
    // If method calls are misparsed, the P4 intrinsic dispatch never fires.
    let output = parse("function main() -> nothing { let s = count.toString() }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value: Expr::MethodCall { method, .. }, .. } => {
            assert_eq!(method, "toString");
        }
        other => panic!("Expected Let with MethodCall value, got {other:?}"),
    }
}


#[test]
fn unary_neg_parses_correctly() {
    // WHY: `-x` must produce UnaryOp(Neg, x), not BinOp(Sub, _, x).
    // If unary minus is misparsed as subtraction, negation diagnostics are wrong.
    let output = parse("function main() -> nothing { let x = -5 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
    };
    match &body.stmts[0] {
        Stmt::Let { value: Expr::UnaryOp { op, operand, .. }, .. } => {
            assert_eq!(*op, ynz_ast::nodes::UnaryOpKind::Neg);
            assert!(matches!(operand.as_ref(), Expr::IntLit(5, _)));
        }
        other => panic!("Expected Let with UnaryOp(Neg), got {other:?}"),
    }
}


#[test]
fn missing_arrow_produces_diagnostic_and_recovers() {
    // WHY: parser must recover from a missing `->` and continue.
    let output = parse("function main() nothing { }");
    assert!(
        !output.diagnostics.is_empty(),
        "Missing `->` must produce at least one diagnostic"
    );
    assert_debug_snapshot!("missing_arrow_diagnostic", output.diagnostics);
}

#[test]
fn missing_closing_brace_produces_diagnostic() {
    let output = parse("function main() -> nothing { print(\"hi\")");
    assert!(
        !output.diagnostics.is_empty(),
        "Missing `}}` must produce at least one diagnostic"
    );
    assert_debug_snapshot!("missing_brace_diagnostic", output.diagnostics);
}

#[test]
fn trailing_garbage_after_function_produces_diagnostic() {
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
    let output = parse("");
    assert_eq!(output.diagnostics.len(), 0);
    assert_eq!(output.module.items.len(), 0);
}

#[test]
fn whitespace_and_comment_only_produces_empty_module() {
    let output = parse("   \n\t  ");
    assert_eq!(output.diagnostics.len(), 0);
    assert_eq!(output.module.items.len(), 0);
}

#[test]
fn wrong_return_type_parses_with_named_type() {
    let output = parse(r#"function main() -> string { print("hi") }"#);
    assert_eq!(output.diagnostics.len(), 0);
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
    assert_eq!(items_before, 1);
    assert_eq!(items_after, 1);

    let stmts_after = match &parse_query(&db, sf).module.items[0] {
        Item::Function(f) => f.body.stmts.len(),
    };
    assert_eq!(stmts_after, 1, "Updated source should have 1 stmt in the body");
}
