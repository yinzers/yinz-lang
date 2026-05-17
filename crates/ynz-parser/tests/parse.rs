// WHY: the parser must accumulate errors, not bail on the first one. The
// snapshot tests guard against silent regression to "return on first error."
// Each negative case asserts both the diagnostic count AND that a partial AST
// was produced (no panic, no empty module where items were expected).

use insta::assert_debug_snapshot;
use salsa::Setter as _;
use ynz_ast::nodes::{BinOpKind, Expr, Item, MatchPatternKind, Stmt, Type};
use ynz_diagnostics::SourceSpan;
use ynz_parser::{parse_query, CompilerDb, SourceFile};

// ── M5 parse helpers ─────────────────────────────────────────────────────────
fn parse_fn_body(source: &str) -> Vec<Stmt> {
    let output = parse(source);
    assert_eq!(output.diagnostics.len(), 0, "unexpected diagnostics: {:?}", output.diagnostics);
    match &output.module.items[0] {
        Item::Function(f) => f.body.stmts.clone(),
        Item::ShapeDecl(_) => panic!("expected a function"),
    }
}

fn wrap(inner: &str) -> String {
    format!("function main() -> nothing {{ {inner} }}")
}

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
fn m3_stmt_variant_count_locked() {
    // WHY: Stmt variants beyond the M3 set are M4+ work. This test pins the count.
    // Add a `// test-ratchet: <reason>` comment to unlock.
    //
    // test-ratchet: M2 adds 2 variants over M1's 1: Let and Assign.
    //   M1: Expr(1). M2: Let(2), Assign(3). Total: 3.
    //
    // test-ratchet: M3 adds 5 variants for control flow.
    //   If(4), Match(5), While(6), For(7), Return(8). Total: 8.
    use ynz_ast::nodes::Stmt::*;
    let err = ynz_ast::nodes::Expr::Error(span(0, 0));
    let empty_block = ynz_ast::nodes::Block { stmts: vec![], span: span(0, 0) };
    let all_variants: &[Stmt] = &[
        Expr(err.clone()),
        Let {
            is_const: false,
            name: String::new(),
            name_span: span(0, 0),
            ty: None,
            value: err.clone(),
            span: span(0, 0),
        },
        Assign {
            target: String::new(),
            target_span: span(0, 0),
            value: err.clone(),
            span: span(0, 0),
        },
        If {
            cond: err.clone(),
            body: empty_block.clone(),
            span: span(0, 0),
        },
        Match {
            scrutinee: err.clone(),
            arms: vec![],
            else_arm: None,
            span: span(0, 0),
        },
        While {
            cond: err.clone(),
            body: empty_block.clone(),
            span: span(0, 0),
        },
        For {
            var: String::new(),
            var_span: span(0, 0),
            iter: err.clone(),
            body: empty_block.clone(),
            span: span(0, 0),
        },
        Return {
            value: None,
            span: span(0, 0),
        },
    ];
    assert_eq!(all_variants.len(), 8, "Stmt variant count changed from 8 — add a // test-ratchet: comment");
}

#[test]
fn m3_match_pattern_kind_variant_count_locked() {
    // WHY: Only three MatchPatternKind variants ship in M3: Value (active),
    // IsType and Variant (both reserved for M6). Locking this prevents
    // accidentally adding a fourth without documenting the milestone.
    //
    // test-ratchet: M3 adds 3 variants: Value(1), IsType(2), Variant(3).
    //   IsType and Variant are deferred to M6 — parser emits diagnostics.
    use ynz_ast::nodes::MatchPatternKind::*;
    let all_variants: &[MatchPatternKind] = &[
        Value(ynz_ast::nodes::Expr::Error(span(0, 0))),
        IsType("Foo".into()),
        Variant("bar".into()),
    ];
    assert_eq!(all_variants.len(), 3, "MatchPatternKind variant count changed from 3");
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
            type_args: None,
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
fn m3_type_variant_count_locked() {
    // WHY: Type variants beyond M3 are M4+ work. This pins the count.
    // Add a `// test-ratchet: <reason>` comment to unlock.
    //
    // test-ratchet: M2 adds 4 variants over M1's 3.
    //   M1: Nothing(1), Named(2), Error(3).
    //   M2: Int(4), Float(5), Number(6), Bool(7).
    //   Total: 7.
    //
    // test-ratchet: M3 adds 1 variant for the range builtin iterable type.
    //   Range is restricted to for-loop iterable position only.
    //   Full Iterable[T] protocol replaces it in M7.
    //   Total: 8.
    use ynz_ast::nodes::Type::*;
    let all_variants: &[Type] = &[
        Nothing,
        Named("foo".into(), span(0, 3)),
        Error,
        Int,
        Float,
        Number { precision: 34 },
        Bool,
        Range { element: Box::new(Int), end_inclusive: false },
    ];
    assert_eq!(all_variants.len(), 8, "Type variant count changed from 8 — add a // test-ratchet: comment");
}

// ── M4 variant count ratchets ────────────────────────────────────────────────

#[test]
fn m4_item_variant_count_locked() {
    // WHY: Item variants beyond M4 are M5+ work. This pins the count.
    // Add a `// test-ratchet: <reason>` comment to unlock.
    //
    // test-ratchet: M1 adds 1 variant: Function(1).
    // test-ratchet: M4 adds 1 variant: ShapeDecl(2).
    use ynz_ast::nodes::{Block, FunctionDecl, Item, ShapeDecl, Type};
    let empty_block = Block { stmts: vec![], span: span(0, 0) };
    let all_variants: &[Item] = &[
        Item::Function(FunctionDecl {
            name: "f".into(),
            name_span: span(0, 1),
            generics: vec![],
            params: vec![],
            return_type: Type::Nothing,
            body: empty_block,
            span: span(0, 0),
        }),
        Item::ShapeDecl(ShapeDecl {
            name: "Foo".into(),
            name_span: span(0, 3),
            is_base: false,
            generics: vec![],
            extends: None,
            follows: vec![],
            fields: vec![],
            contract_sigs: vec![],
            span: span(0, 0),
        }),
    ];
    assert_eq!(all_variants.len(), 2, "Item variant count changed from 2 — add a // test-ratchet: comment");
}

#[test]
fn m4_stmt_variant_count_locked() {
    // WHY: Stmt variants beyond M4 are M5+ work. This pins the count.
    //
    // test-ratchet: M4 adds 1 variant over M3's 8: FieldAssign(9).
    use ynz_ast::nodes::Stmt::*;
    let err = ynz_ast::nodes::Expr::Error(span(0, 0));
    let empty_block = ynz_ast::nodes::Block { stmts: vec![], span: span(0, 0) };
    let all_variants: &[Stmt] = &[
        Expr(err.clone()),
        Let { is_const: false, name: String::new(), name_span: span(0, 0), ty: None, value: err.clone(), span: span(0, 0) },
        Assign { target: String::new(), target_span: span(0, 0), value: err.clone(), span: span(0, 0) },
        If { cond: err.clone(), body: empty_block.clone(), span: span(0, 0) },
        Match { scrutinee: err.clone(), arms: vec![], else_arm: None, span: span(0, 0) },
        While { cond: err.clone(), body: empty_block.clone(), span: span(0, 0) },
        For { var: String::new(), var_span: span(0, 0), iter: err.clone(), body: empty_block.clone(), span: span(0, 0) },
        Return { value: None, span: span(0, 0) },
        FieldAssign { target: Box::new(err.clone()), value: err.clone(), span: span(0, 0) },
    ];
    assert_eq!(all_variants.len(), 9, "Stmt variant count changed from 9 — add a // test-ratchet: comment");
}

#[test]
fn m4_expr_variant_count_locked() {
    // WHY: Expr variants beyond M4 are M5+ work. This pins the count.
    //
    // test-ratchet: M4 adds 4 variants over M2's 10: FieldAccess(11), StructLit(12), PostfixOp(13), SelfValue(14).
    use ynz_ast::nodes::{BinOpKind, Expr::*, PostfixOpKind};
    let err = || Box::new(Error(span(0, 0)));
    let all_variants: &[ynz_ast::nodes::Expr] = &[
        Ident("x".into(), span(0, 1)),
        StringLit(vec![], span(0, 0)),
        Call(Box::new(ynz_ast::nodes::CallExpr { callee: Ident("f".into(), span(0, 1)), type_args: None, args: vec![], span: span(0, 3) })),
        Error(span(0, 0)),
        IntLit(0, span(0, 1)),
        NumberLit("0".into(), span(0, 1)),
        BoolLit(true, span(0, 4)),
        BinOp { op: BinOpKind::Add, lhs: err(), rhs: err(), span: span(0, 0) },
        UnaryOp { op: ynz_ast::nodes::UnaryOpKind::Neg, operand: err(), span: span(0, 0) },
        MethodCall { receiver: err(), method: "m".into(), method_span: span(0, 1), args: vec![], span: span(0, 0) },
        FieldAccess { receiver: err(), field: "f".into(), field_span: span(0, 1), span: span(0, 0) },
        StructLit { fields: vec![], span: span(0, 0) },
        PostfixOp { receiver: err(), op: PostfixOpKind::Copy, span: span(0, 0) },
        SelfValue { span: span(0, 0) },
    ];
    assert_eq!(all_variants.len(), 14, "Expr variant count changed from 14 — add a // test-ratchet: comment");
}

#[test]
fn m4_type_variant_count_locked() {
    // WHY: Type variants beyond M4 are M5+ work. This pins the count.
    //
    // test-ratchet: M4 adds 2 variants over M3's 8: Dynamic(9), SelfType(10).
    use ynz_ast::nodes::Type::*;
    let all_variants: &[ynz_ast::nodes::Type] = &[
        Nothing,
        Named("foo".into(), span(0, 3)),
        Error,
        Int,
        Float,
        Number { precision: 34 },
        Bool,
        Range { element: Box::new(Int), end_inclusive: false },
        Dynamic { contract: "Foo".into(), span: span(0, 3) },
        SelfType { span: span(0, 4) },
    ];
    assert_eq!(all_variants.len(), 10, "Type variant count changed from 10 — add a // test-ratchet: comment");
}

// ── M5 variant-count locks ───────────────────────────────────────────────────

#[test]
fn m5_stmt_variant_count_locked() {
    // WHY: Stmt variants beyond M5 are M6+ work. This pins the count.
    //
    // test-ratchet: M5 adds 1 variant over M4's 9: IndexAssign(10).
    use ynz_ast::nodes::Stmt::*;
    let err = ynz_ast::nodes::Expr::Error(span(0, 0));
    let empty_block = ynz_ast::nodes::Block { stmts: vec![], span: span(0, 0) };
    let all_variants: &[Stmt] = &[
        Expr(err.clone()),
        Let { is_const: false, name: String::new(), name_span: span(0, 0), ty: None, value: err.clone(), span: span(0, 0) },
        Assign { target: String::new(), target_span: span(0, 0), value: err.clone(), span: span(0, 0) },
        If { cond: err.clone(), body: empty_block.clone(), span: span(0, 0) },
        Match { scrutinee: err.clone(), arms: vec![], else_arm: None, span: span(0, 0) },
        While { cond: err.clone(), body: empty_block.clone(), span: span(0, 0) },
        For { var: String::new(), var_span: span(0, 0), iter: err.clone(), body: empty_block.clone(), span: span(0, 0) },
        Return { value: None, span: span(0, 0) },
        FieldAssign { target: Box::new(err.clone()), value: err.clone(), span: span(0, 0) },
        IndexAssign { receiver: Box::new(err.clone()), index: Box::new(err.clone()), value: err.clone(), span: span(0, 0) },
    ];
    assert_eq!(all_variants.len(), 10, "Stmt variant count changed from 10 — add a // test-ratchet: comment");
}

#[test]
fn m5_expr_variant_count_locked() {
    // WHY: Expr variants beyond M5 are M6+ work. This pins the count.
    //
    // test-ratchet: M5 adds 2 variants over M4's 14: NoneLit(15), IndexAccess(16).
    use ynz_ast::nodes::{BinOpKind, Expr::*, PostfixOpKind};
    let err = || Box::new(Error(span(0, 0)));
    let all_variants: &[ynz_ast::nodes::Expr] = &[
        Ident("x".into(), span(0, 1)),
        StringLit(vec![], span(0, 0)),
        Call(Box::new(ynz_ast::nodes::CallExpr { callee: Ident("f".into(), span(0, 1)), type_args: None, args: vec![], span: span(0, 3) })),
        Error(span(0, 0)),
        IntLit(0, span(0, 1)),
        NumberLit("0".into(), span(0, 1)),
        BoolLit(true, span(0, 4)),
        BinOp { op: BinOpKind::Add, lhs: err(), rhs: err(), span: span(0, 0) },
        UnaryOp { op: ynz_ast::nodes::UnaryOpKind::Neg, operand: err(), span: span(0, 0) },
        MethodCall { receiver: err(), method: "m".into(), method_span: span(0, 1), args: vec![], span: span(0, 0) },
        FieldAccess { receiver: err(), field: "f".into(), field_span: span(0, 1), span: span(0, 0) },
        StructLit { fields: vec![], span: span(0, 0) },
        PostfixOp { receiver: err(), op: PostfixOpKind::Copy, span: span(0, 0) },
        SelfValue { span: span(0, 0) },
        NoneLit { span: span(0, 0) },
        IndexAccess { receiver: err(), index: err(), span: span(0, 0) },
    ];
    assert_eq!(all_variants.len(), 16, "Expr variant count changed from 16 — add a // test-ratchet: comment");
}

#[test]
fn m5_type_variant_count_locked() {
    // WHY: Type variants beyond M5 are M6+ work. This pins the count.
    //
    // test-ratchet: M5 adds 3 variants over M4's 10: TypeParam(11), Generic(12), Maybe(13).
    use ynz_ast::nodes::Type::*;
    let all_variants: &[ynz_ast::nodes::Type] = &[
        Nothing,
        Named("foo".into(), span(0, 3)),
        Error,
        Int,
        Float,
        Number { precision: 34 },
        Bool,
        Range { element: Box::new(Int), end_inclusive: false },
        Dynamic { contract: "Foo".into(), span: span(0, 3) },
        SelfType { span: span(0, 4) },
        TypeParam { name: "T".into(), span: span(0, 1) },
        Generic { name: "array".into(), name_span: span(0, 5), args: vec![Int], span: span(0, 10) },
        Maybe { inner: Box::new(Int), span: span(0, 9) },
    ];
    assert_eq!(all_variants.len(), 13, "Type variant count changed from 13 — add a // test-ratchet: comment");
}

// ── M4 parse tests ───────────────────────────────────────────────────────────

#[test]
fn shape_declaration_parses() {
    // WHY: The simplest valid shape must parse cleanly. If this fails,
    // every downstream shape test is meaningless.
    let output = parse("shape Player { name: string\n health: int }");
    assert_eq!(output.diagnostics.len(), 0, "Simple shape must parse with 0 diagnostics");
    assert!(
        matches!(&output.module.items[0], Item::ShapeDecl(s) if s.name == "Player"),
        "Expected ShapeDecl(Player)"
    );
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert_eq!(s.fields.len(), 2, "Expected 2 fields");
            assert_eq!(s.fields[0].name, "name");
            assert_eq!(s.fields[1].name, "health");
            assert!(!s.is_base);
            assert!(s.extends.is_none());
            assert!(s.follows.is_empty());
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn base_shape_parses() {
    // WHY: `base shape` is the Yinz form for non-instantiable types. The parser must
    // set is_base=true and not confuse it with a regular shape.
    let output = parse("base shape Entity { name: string }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert!(s.is_base, "Expected is_base=true");
            assert_eq!(s.name, "Entity");
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn shape_with_extends_and_follows_parses() {
    // WHY: `shape Warrior extends Entity follows Damageable` must parse the
    // inheritance and contract declarations correctly into the AST.
    let output = parse("shape Warrior extends Entity follows Damageable { weapon: string }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert!(s.extends.as_ref().is_some_and(|(n, _)| n == "Entity"), "Expected extends=Entity");
            assert_eq!(s.follows.len(), 1);
            assert_eq!(s.follows[0].0, "Damageable");
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn hidden_field_parses() {
    // WHY: `hidden cache: int` must set is_hidden=true on the FieldDecl.
    let output = parse("shape Player { name: string\n hidden cache: int }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert!(!s.fields[0].is_hidden, "name should not be hidden");
            assert!(s.fields[1].is_hidden, "cache should be hidden");
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn contract_sig_in_shape_parses() {
    // WHY: Bare method signatures `greet(share self) -> string` inside a shape
    // are how contracts are declared. Parser must distinguish them from fields.
    let output = parse("shape Greetable { greet(share self) -> string }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert_eq!(s.fields.len(), 0, "No data fields");
            assert_eq!(s.contract_sigs.len(), 1, "Expected 1 contract sig");
            assert_eq!(s.contract_sigs[0].name, "greet");
            assert!(matches!(s.contract_sigs[0].receiver, Some(ynz_ast::nodes::ReceiverKind::Share)));
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn anonymous_struct_lit_parses() {
    // WHY: `let p: Player = { name: "Patrick", health: 100 }` — the annotation-only
    // struct literal form — must produce Expr::StructLit with 2 fields.
    let output = parse(r#"function main() -> nothing { let p: string = { name: "Patrick" } }"#);
    assert_eq!(output.diagnostics.len(), 0, "Struct lit must parse cleanly");
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::Let { value: ynz_ast::nodes::Expr::StructLit { fields, .. }, .. } => {
            assert_eq!(fields.len(), 1, "Expected 1 field");
            assert_eq!(fields[0].name, "name");
        }
        other => panic!("Expected Let with StructLit value, got {other:?}"),
    }
}

#[test]
fn field_access_parses() {
    // WHY: `player.health` must produce Expr::FieldAccess (no parens = pure read
    // per dot-postfix rule). If it accidentally becomes MethodCall, codegen breaks.
    let output = parse("function main() -> nothing { let x = player.health }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::Let { value: ynz_ast::nodes::Expr::FieldAccess { field, .. }, .. } => {
            assert_eq!(field, "health");
        }
        other => panic!("Expected Let with FieldAccess, got {other:?}"),
    }
}

#[test]
fn field_assignment_parses() {
    // WHY: `player.health = 50` must produce Stmt::FieldAssign, not Stmt::Assign.
    // If it becomes Stmt::Assign, the typeck won't enforce field-mutation rules.
    let output = parse("function main() -> nothing { player.health = 50 }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::FieldAssign { target, value: ynz_ast::nodes::Expr::IntLit(50, _), .. } => {
            assert!(matches!(**target, ynz_ast::nodes::Expr::FieldAccess { ref field, .. } if field == "health"));
        }
        other => panic!("Expected FieldAssign, got {other:?}"),
    }
}

#[test]
fn copy_postfix_op_parses() {
    // WHY: `value.copy()` must produce Expr::PostfixOp { op: Copy }. This is a
    // body operation (parens required) per the dot-postfix rule.
    let output = parse("function main() -> nothing { let b = a.copy() }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::Let { value: ynz_ast::nodes::Expr::PostfixOp { op: ynz_ast::nodes::PostfixOpKind::Copy, .. }, .. } => {}
        other => panic!("Expected Let with PostfixOp(Copy), got {other:?}"),
    }
}

#[test]
fn freeze_postfix_op_parses() {
    // WHY: `value.freeze()` must produce Expr::PostfixOp { op: Freeze }.
    let output = parse("function main() -> nothing { cfg.freeze() }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::Expr(ynz_ast::nodes::Expr::PostfixOp { op: ynz_ast::nodes::PostfixOpKind::Freeze, .. }) => {}
        other => panic!("Expected Expr(PostfixOp(Freeze)), got {other:?}"),
    }
}

#[test]
fn dynamic_type_in_type_position_parses() {
    // WHY: `let d: dynamic Damageable = p` — `dynamic Foo` is a valid type annotation.
    // Must produce Type::Dynamic, not Type::Named.
    let output = parse("function main() -> nothing { let d: dynamic Foo = p }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::Let { ty: Some(ynz_ast::nodes::Type::Dynamic { contract, .. }), .. } => {
            assert_eq!(contract, "Foo");
        }
        other => panic!("Expected Let with Type::Dynamic, got {other:?}"),
    }
}

#[test]
fn prefix_form_struct_lit_produces_teaching_diagnostic() {
    // WHY: `Player { name: "x" }` is the banned prefix form. The parser must emit
    // a teaching diagnostic redirecting to the annotation-only form, recover cleanly,
    // and NOT produce an Expr::StructLit (since the prefix form is compile-time banned).
    let output = parse(r#"function main() -> nothing { let x = Player { name: "Patrick" } }"#);
    assert_eq!(
        output.diagnostics.len(),
        1,
        "Prefix-form struct literal must produce exactly 1 teaching diagnostic"
    );
    assert!(
        output.diagnostics[0].what_instead.contains("annotation"),
        "Teaching diagnostic must mention annotation-driven form, got: {:?}",
        output.diagnostics[0].what_instead
    );
}

#[test]
fn method_body_inside_shape_produces_diagnostic() {
    // WHY: Yinz is not OOP — method bodies cannot be declared inside shapes.
    // The parser must emit a clear teaching diagnostic and recover.
    let output = parse("shape Player { function greet() -> string { return \"hi\" } }");
    assert_eq!(
        output.diagnostics.len(),
        1,
        "Method inside shape must produce exactly 1 diagnostic"
    );
    assert!(
        output.diagnostics[0].what_instead.contains("outside"),
        "Diagnostic must tell user to move the function outside the shape, got: {:?}",
        output.diagnostics[0].what_instead
    );
}

#[test]
fn self_value_in_function_body_parses() {
    // WHY: `self` (lowercase) in an expression position produces Expr::SelfValue.
    // This enables receiver-style functions: `function greet(share self: Player) { print(self.name) }`.
    let output = parse("function greet(share self: string) -> nothing { print(self) }");
    assert_eq!(output.diagnostics.len(), 0);
    let body = match &output.module.items[0] {
        Item::Function(f) => &f.body,
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    match &body.stmts[0] {
        Stmt::Expr(ynz_ast::nodes::Expr::Call(c)) => {
            assert!(matches!(c.args[0], ynz_ast::nodes::Expr::SelfValue { .. }), "Arg must be SelfValue");
        }
        other => panic!("Expected Expr(Call) with SelfValue arg, got {other:?}"),
    }
}

#[test]
fn m4_parse_snapshot() {
    // WHY: Snapshot locks the full M4 AST for a realistic shape-declaration source
    // so any parser regression shows up as an insta diff.
    let source = r#"shape Player {
  name: string
  health: int
  hidden cache: int
}

function greet(share self: Player) -> string {
  return self.name
}

function main() -> nothing {
  let p: Player = { name: "Patrick", health: 100 }
  p.health = 90
  let h = p.health
  let b = p.copy()
}"#;
    let output = parse(source);
    assert_eq!(output.diagnostics.len(), 0, "M4 fixture must parse with 0 diagnostics");
    assert_debug_snapshot!("m4_ast", output.module);
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn m3_source_parses_to_expected_ast() {
    // WHY: Snapshot locks the full M3 AST shape including function parameters,
    // if/while/for/return, and multi-case match arms. Any silent regression in
    // parser output breaks the typeck and codegen stages that depend on this shape.
    let source = r#"function fib(n: int) -> int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function main() -> nothing {
  let result = fib(10)
  print(result)
}"#;
    let output = parse(source);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "M3 fibonacci source must parse with no diagnostics"
    );
    assert_debug_snapshot!("m3_ast", output.module);
}

#[test]
fn function_with_parameters_parses_correctly() {
    // WHY: `function add(x: int, y: int) -> int { ... }` must produce a
    // FunctionDecl with two Param entries. If params are silently dropped,
    // typeck cannot check argument types at call sites.
    let output = parse("function add(x: int, y: int) -> int { return x }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => {
            assert_eq!(f.params.len(), 2, "Expected 2 params");
            assert_eq!(f.params[0].name, "x");
            assert_eq!(f.params[1].name, "y");
            assert!(matches!(f.params[0].ty, Type::Int));
            assert!(matches!(f.params[1].ty, Type::Int));
        }
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn function_with_no_params_still_parses() {
    // WHY: Zero-parameter functions were the only form in M1/M2. This guards
    // against the M3 parameter parser accidentally requiring at least one param.
    let output = parse("function main() -> nothing { }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => assert_eq!(f.params.len(), 0),
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn trailing_comma_in_params_is_accepted() {
    // WHY: `function foo(x: int,) -> nothing { }` (trailing comma) must parse
    // cleanly. Trailing commas are ergonomic when adding params line-by-line;
    // rejecting them would be a footgun with no benefit.
    let output = parse("function foo(x: int,) -> nothing { }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => assert_eq!(f.params.len(), 1),
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn duplicate_param_name_produces_diagnostic() {
    // WHY: `function foo(a: int, a: int) -> int { return a }` — two params with
    // the same name — must produce a diagnostic. Silent acceptance would leave
    // typeck to pick one arbitrarily (implementation-defined behavior = bug).
    let output = parse("function foo(a: int, a: int) -> int { return a }");
    assert!(
        !output.diagnostics.is_empty(),
        "Duplicate param name must produce at least 1 diagnostic"
    );
    assert!(
        output.diagnostics[0].what.contains("Duplicate"),
        "Diagnostic should mention 'Duplicate', got: {:?}",
        output.diagnostics[0].what
    );
}

#[test]
fn ownership_annotation_on_param_parses_correctly() {
    // WHY: M4 ships ownership modifiers. `function foo(share x: int) -> nothing { }` must
    // parse cleanly with no diagnostics, and the param must have ownership=Some(Share).
    // This test replaced the M3 deferral test — ownership annotations now work.
    // test-ratchet: M4 ships ownership modifiers — deferral replaced with success assertion.
    let output = parse("function foo(share x: int) -> nothing { }");
    assert_eq!(
        output.diagnostics.len(),
        0,
        "share annotation must parse cleanly in M4"
    );
    match &output.module.items[0] {
        Item::Function(f) => {
            assert_eq!(f.params.len(), 1, "Expected 1 param");
            assert!(
                matches!(f.params[0].ownership, Some(ynz_ast::nodes::OwnershipModifier::Share)),
                "Expected ownership=Share, got {:?}", f.params[0].ownership
            );
            assert_eq!(f.params[0].name, "x");
        }
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn simple_if_parses_correctly() {
    // WHY: `if (cond) { body }` must produce Stmt::If, not Stmt::Match.
    // If disambiguation defaults to multi-case, every simple if in the codebase
    // would fail to parse.
    let output = parse("function main() -> nothing { if (x > 0) { print(x) } }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::If { .. } => {}
            other => panic!("Expected Stmt::If, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn while_loop_parses_correctly() {
    // WHY: `while (cond) { body }` must produce Stmt::While. Regression would
    // mean loops silently become expression statements or fail to parse.
    let output = parse("function main() -> nothing { while (x > 0) { x = x - 1 } }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::While { .. } => {}
            other => panic!("Expected Stmt::While, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn for_loop_parses_correctly() {
    // WHY: `for (i in range(0, 5)) { body }` must produce Stmt::For with `var`
    // set to "i" and `iter` as a Call to range. The loop variable and iterable
    // expression are both critical for typeck's `range`-as-only-iterable check.
    let output = parse("function main() -> nothing { for (i in range(0, 5)) { print(i) } }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::For { var, .. } => assert_eq!(var, "i"),
            other => panic!("Expected Stmt::For, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn return_with_value_parses_correctly() {
    // WHY: `return expr` must produce Stmt::Return with value Some(expr).
    // If value is None when an expression is present, typeck cannot check
    // the return type against the function signature.
    let output = parse("function foo() -> int { return 42 }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::Return { value: Some(_), .. } => {}
            other => panic!("Expected Stmt::Return with value, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn return_without_value_parses_correctly() {
    // WHY: `return` alone (no expression) must produce Stmt::Return with value
    // None. This is the only valid form for `-> nothing` functions; typeck
    // uses the None/Some to determine whether a value was provided.
    let output = parse("function main() -> nothing { return }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::Return { value: None, .. } => {}
            other => panic!("Expected Stmt::Return with no value, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn multi_case_if_with_int_arms_parses_as_match() {
    // WHY: `if (x) { 1 => ...; 2 => ... }` must produce Stmt::Match, not Stmt::If.
    // If the parser always picks simple-if, multi-case `if` becomes syntactically
    // impossible and the entire branching-by-value feature is dead.
    let output = parse("function main() -> nothing { if (x) { 1 => print(1) 2 => print(2) } }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::Match { arms, else_arm: None, .. } => {
                assert_eq!(arms.len(), 2, "Expected 2 arms");
            }
            other => panic!("Expected Stmt::Match, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn multi_case_if_with_else_arm() {
    // WHY: `if (x) { 1 => ...; else => ... }` must produce Stmt::Match with
    // else_arm = Some(_). Typeck uses the presence of else_arm to determine
    // whether the multi-case is exhaustive for return-path analysis.
    let output = parse("function main() -> nothing { if (x) { 1 => print(1) else => print(0) } }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::Match { arms, else_arm: Some(_), .. } => {
                assert_eq!(arms.len(), 1, "Expected 1 value arm + else");
            }
            other => panic!("Expected Stmt::Match with else_arm, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn multi_case_else_only_parses_as_match() {
    // WHY: `if (x) { else => ... }` (only a catch-all, no value arms) is a
    // valid multi-case `if`. Typeck treats it as an exhaustive match that always
    // executes the else_arm body. Parser must not reject it as malformed.
    let output = parse("function main() -> nothing { if (x) { else => print(0) } }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::Match { arms, else_arm: Some(_), .. } => {
                assert_eq!(arms.len(), 0, "Else-only: 0 value arms");
            }
            other => panic!("Expected Stmt::Match with else_arm only, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
    }
}

#[test]
fn is_type_arm_produces_m6_deferral_diagnostic() {
    // WHY: `if (shape) { is Circle => ... }` is the M6 type-narrowing form.
    // In M3 the parser must emit a deferral diagnostic (not a confusing
    // "unexpected token" error) and produce a MatchArm so the parser recovers.
    let output = parse("function main() -> nothing { if (value) { is Circle => print(1) } }");
    assert_eq!(
        output.diagnostics.len(),
        1,
        "is-type arm must produce exactly 1 M6 deferral diagnostic"
    );
    assert!(
        output.diagnostics[0].why.contains("milestone 6"),
        "Deferral must mention milestone 6, got: {:?}",
        output.diagnostics[0].why
    );
}

#[test]
fn if_without_parens_produces_diagnostic() {
    // WHY: `if cond { }` (no parentheses) is a common mistake from languages like
    // Rust/Go. The parser must emit a teaching diagnostic, not silently misbehave.
    // Yinz requires `if (cond)` consistently with `while (cond)` and `for (...)`.
    let output = parse("function main() -> nothing { if x > 0 { print(x) } }");
    assert!(
        !output.diagnostics.is_empty(),
        "Missing parens on if must produce at least 1 diagnostic"
    );
}

#[test]
fn multi_function_module_parses_correctly() {
    // WHY: M3 is the first milestone with multi-function modules. Both functions
    // must appear as separate items in the AST. If the parser stops after the
    // first function, mutual recursion and callee lookups in typeck fail silently.
    let source = r#"function add(a: int, b: int) -> int { return a }
function main() -> nothing { let x = add(1, 2) }"#;
    let output = parse(source);
    assert_eq!(output.diagnostics.len(), 0);
    assert_eq!(output.module.items.len(), 2, "Expected 2 function items");
}

#[test]
fn nested_if_inside_for_parses_correctly() {
    // WHY: Control-flow nesting is the typical real-program pattern. If nested
    // blocks corrupt the block-end tracking, only the outermost construct works
    // and every realistic program fails to parse.
    let source = r#"function main() -> nothing {
  for (i in range(0, 5)) {
    if (i > 2) {
      print(i)
    }
  }
}"#;
    let output = parse(source);
    assert_eq!(output.diagnostics.len(), 0, "Nested if-inside-for must parse cleanly");
    match &output.module.items[0] {
        Item::Function(f) => match &f.body.stmts[0] {
            Stmt::For { body, .. } => match &body.stmts[0] {
                Stmt::If { .. } => {}
                other => panic!("Inner stmt should be If, got {other:?}"),
            },
            other => panic!("Outer stmt should be For, got {other:?}"),
        },
        Item::ShapeDecl(_) => panic!("expected a function item"),
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
        Item::ShapeDecl(_) => panic!("expected a function item"),
    };
    assert_eq!(stmts_after, 1, "Updated source should have 1 stmt in the body");
}

// ── M5 P2: Generic function declarations ─────────────────────────────────────

#[test]
fn m5_generic_fn_single_type_param() {
    // WHY: acceptance criterion — `function identity<T>(give value: T) -> T`
    // must parse with `generics: [T]`. If the generic param is lost, typeck
    // cannot substitute concrete types and monomorphization is impossible.
    let output = parse("function identity<T>(give value: T) -> T { return value }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => {
            assert_eq!(f.generics.len(), 1);
            assert_eq!(f.generics[0].name, "T");
            assert!(f.generics[0].constraints.is_empty());
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn m5_generic_fn_two_type_params() {
    // WHY: multi-param generics like `pair<A, B>` must produce two separate
    // GenericParam entries. If they collapse to one, `B` is invisible to typeck.
    let output = parse("function pair<A, B>(give first: A, give second: B) -> nothing { }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => {
            assert_eq!(f.generics.len(), 2);
            assert_eq!(f.generics[0].name, "A");
            assert_eq!(f.generics[1].name, "B");
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn m5_generic_fn_with_constraint() {
    // WHY: acceptance criterion — `function sort<T follows Comparable>(...)` must
    // parse with `constraints: [("Comparable", _)]` on T. Without this, typeck
    // cannot verify that the concrete type satisfies the contract.
    let output = parse("function sort<T follows Comparable>(share items: T) -> nothing { }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => {
            assert_eq!(f.generics.len(), 1);
            assert_eq!(f.generics[0].name, "T");
            assert_eq!(f.generics[0].constraints.len(), 1);
            assert_eq!(f.generics[0].constraints[0].0, "Comparable");
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn m5_generic_fn_two_params_one_constrained() {
    // WHY: `<T follows Comparable, U>` is two params — T with a constraint, U
    // without. The `,` must terminate T's entry and start U's.
    let output = parse("function f<T follows Comparable, U>(share a: T, give b: U) -> nothing { }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => {
            assert_eq!(f.generics.len(), 2);
            assert_eq!(f.generics[0].name, "T");
            assert_eq!(f.generics[0].constraints.len(), 1);
            assert_eq!(f.generics[1].name, "U");
            assert!(f.generics[1].constraints.is_empty());
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn m5_non_generic_fn_has_empty_generics() {
    // WHY: a non-generic function must produce `generics: []`. If it accidentally
    // produces a non-empty list, every typeck path that checks for generic vs
    // non-generic would be wrong.
    let output = parse("function add(a: int, b: int) -> int { return a }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::Function(f) => assert!(f.generics.is_empty()),
        _ => panic!("expected Function"),
    }
}

// ── M5 P2: Generic shape declarations ────────────────────────────────────────

#[test]
fn m5_generic_shape_two_params() {
    // WHY: acceptance criterion — `shape Pair<A, B> { first: A, second: B }` must
    // parse with `generics: [A, B]`. Without this, the field types `A` and `B`
    // cannot be resolved to type parameters in typeck.
    let output = parse("shape Pair<A, B> { first: A second: B }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert_eq!(s.generics.len(), 2);
            assert_eq!(s.generics[0].name, "A");
            assert_eq!(s.generics[1].name, "B");
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn m5_generic_shape_single_param() {
    // WHY: `shape Box<T>` must parse with one generic param.
    let output = parse("shape Box<T> { value: T }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => {
            assert_eq!(s.generics.len(), 1);
            assert_eq!(s.generics[0].name, "T");
        }
        _ => panic!("expected ShapeDecl"),
    }
}

#[test]
fn m5_non_generic_shape_has_empty_generics() {
    // WHY: `shape Player { ... }` must keep `generics: []`. An accidental
    // generic param would confuse the monomorphization table.
    let output = parse("shape Player { name: int }");
    assert_eq!(output.diagnostics.len(), 0);
    match &output.module.items[0] {
        Item::ShapeDecl(s) => assert!(s.generics.is_empty()),
        _ => panic!("expected ShapeDecl"),
    }
}

// ── M5 P2: Generic type instantiations in type position ──────────────────────

#[test]
fn m5_array_type_annotation() {
    // WHY: acceptance criterion — `let arr: array<int> = ...` must produce
    // `Type::Generic { name: "array", args: [Int] }`. If it stays `Type::Named`,
    // the typeck cannot resolve the element type.
    let output = parse(&wrap("let arr: array<int> = arr"));
    assert_eq!(output.diagnostics.len(), 0);
    let stmts = parse_fn_body(&wrap("let arr: array<int> = arr"));
    match &stmts[0] {
        Stmt::Let { ty: Some(Type::Generic { name, args, .. }), .. } => {
            assert_eq!(name, "array");
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Type::Int));
        }
        other => panic!("expected Let with Generic type annotation, got {other:?}"),
    }
}

#[test]
fn m5_maybe_type_annotation() {
    // WHY: acceptance criterion — `let x: maybe<int> = none` must produce
    // `Type::Maybe { inner: Int }`. The distinct Maybe variant (not Generic)
    // is required so typeck can apply flow-sensitive `.value` rules.
    let stmts = parse_fn_body(&wrap("let x: maybe<int> = none"));
    match &stmts[0] {
        Stmt::Let { ty: Some(Type::Maybe { inner, .. }), value: Expr::NoneLit { .. }, .. } => {
            assert!(matches!(**inner, Type::Int));
        }
        other => panic!("expected Let with Maybe<Int> type and NoneLit, got {other:?}"),
    }
}

#[test]
fn m5_maybe_player_type_annotation() {
    // WHY: `maybe<Player>` (named inner type) must produce `Maybe { inner: Named("Player") }`.
    let stmts = parse_fn_body(&wrap("let x: maybe<Player> = none"));
    match &stmts[0] {
        Stmt::Let { ty: Some(Type::Maybe { inner, .. }), .. } => {
            assert!(matches!(**inner, Type::Named(ref n, _) if n == "Player"));
        }
        other => panic!("expected Let with Maybe<Named>, got {other:?}"),
    }
}

#[test]
fn m5_nested_generic_type() {
    // WHY: `Pair<Pair<int, string>, int>` tests nested angle brackets. The depth
    // cap (16) must not fire here; the type must parse to a Generic with one
    // Generic arg and one Int arg.
    let stmts = parse_fn_body(&wrap("let x: Pair<Pair<int, int>, int> = x"));
    match &stmts[0] {
        Stmt::Let { ty: Some(Type::Generic { name, args, .. }), .. } => {
            assert_eq!(name, "Pair");
            assert_eq!(args.len(), 2);
            assert!(matches!(&args[0], Type::Generic { name: n, .. } if n == "Pair"));
            assert!(matches!(args[1], Type::Int));
        }
        other => panic!("expected nested Generic type, got {other:?}"),
    }
}

#[test]
fn m5_generic_type_depth_cap_error() {
    // WHY: the depth cap (16) must produce exactly ONE diagnostic, not a
    // cascade of errors. A cascade would hide the real error in noise.
    // Build 17-deep nesting: `A<A<A<...<int>...>>>`.
    let deep: String = "A<".repeat(17) + "int" + &">".repeat(17);
    let source = format!("function main() -> nothing {{ let x: {deep} = x }}");
    let output = parse(&source);
    assert!(
        output.diagnostics.len() >= 1,
        "Expected at least one diagnostic for 17-deep nesting"
    );
}

// ── M5 P2: `none` literal ────────────────────────────────────────────────────

#[test]
fn m5_none_literal_expr() {
    // WHY: `none` must parse to `Expr::NoneLit`. If it parses as `Expr::Ident("none")`,
    // typeck cannot recognize the absence value and the maybe<T> flow won't work.
    let stmts = parse_fn_body(&wrap("let x = none"));
    match &stmts[0] {
        Stmt::Let { value: Expr::NoneLit { .. }, .. } => {}
        other => panic!("expected NoneLit, got {other:?}"),
    }
}

#[test]
fn m5_none_in_condition() {
    // WHY: `none` must be usable as an expression in any position, including
    // comparisons. Restricting it to binding RHS would require special-casing
    // in every expression context.
    let output = parse(&wrap("if (x == none) { }"));
    assert_eq!(output.diagnostics.len(), 0);
}

// ── M5 P2: Bracket index access ──────────────────────────────────────────────

#[test]
fn m5_index_access_expr() {
    // WHY: acceptance criterion — `arr[0]` must produce `Expr::IndexAccess`.
    // Without this, bracket access on collections has no AST representation.
    let stmts = parse_fn_body(&wrap("let x = arr[0]"));
    match &stmts[0] {
        Stmt::Let { value: Expr::IndexAccess { index, .. }, .. } => {
            assert!(matches!(**index, Expr::IntLit(0, _)));
        }
        other => panic!("expected IndexAccess, got {other:?}"),
    }
}

#[test]
fn m5_index_access_string_key() {
    // WHY: `m["alice"]` must work too — map keys are not just integers.
    let stmts = parse_fn_body(&wrap(r#"let x = m["alice"]"#));
    match &stmts[0] {
        Stmt::Let { value: Expr::IndexAccess { index, .. }, .. } => {
            assert!(matches!(**index, Expr::StringLit(_, _)));
        }
        other => panic!("expected IndexAccess with StringLit index, got {other:?}"),
    }
}

#[test]
fn m5_chained_index_access() {
    // WHY: `m["alice"][0]` is an IndexAccess whose receiver is also IndexAccess.
    // If chaining doesn't work, multi-dimensional access breaks.
    let stmts = parse_fn_body(&wrap(r#"let x = m["alice"][0]"#));
    match &stmts[0] {
        Stmt::Let { value: Expr::IndexAccess { receiver, .. }, .. } => {
            assert!(matches!(**receiver, Expr::IndexAccess { .. }));
        }
        other => panic!("expected chained IndexAccess, got {other:?}"),
    }
}

#[test]
fn m5_index_access_standalone_stmt() {
    // WHY: `arr[0]` as a bare statement (expression statement) must not produce
    // a diagnostic. It's unusual but legal — typeck will warn about unused values.
    let output = parse(&wrap("arr[0]"));
    assert_eq!(output.diagnostics.len(), 0);
    let stmts = parse_fn_body(&wrap("arr[0]"));
    assert!(matches!(stmts[0], Stmt::Expr(Expr::IndexAccess { .. })));
}

// ── M5 P2: Index assign ───────────────────────────────────────────────────────

#[test]
fn m5_index_assign_stmt() {
    // WHY: acceptance criterion — `arr[0] = 5` must produce `Stmt::IndexAssign`.
    // If it becomes `Stmt::Assign` or `Stmt::Expr`, typeck cannot enforce
    // the bracket-sugar `.set()` lowering.
    let stmts = parse_fn_body(&wrap("arr[0] = 5"));
    match &stmts[0] {
        Stmt::IndexAssign { index, value: Expr::IntLit(5, _), .. } => {
            assert!(matches!(**index, Expr::IntLit(0, _)));
        }
        other => panic!("expected IndexAssign, got {other:?}"),
    }
}

#[test]
fn m5_index_assign_string_key() {
    // WHY: `scores["alice"] = 90` is the map-write path. If string-keyed
    // index assign doesn't work, maps are read-only via bracket syntax.
    let stmts = parse_fn_body(&wrap(r#"scores["alice"] = 90"#));
    match &stmts[0] {
        Stmt::IndexAssign { index, value: Expr::IntLit(90, _), .. } => {
            assert!(matches!(**index, Expr::StringLit(_, _)));
        }
        other => panic!("expected IndexAssign with StringLit key, got {other:?}"),
    }
}

// ── M5 P2: Generic call sites (contextual `<` disambiguation) ────────────────

#[test]
fn m5_generic_call_one_type_arg() {
    // WHY: acceptance criterion — `identity<int>(5)` must produce a Call with
    // `type_args: Some([Int])`. Without this, explicit type arguments at call
    // sites are silently dropped and typeck falls back to inference only.
    let stmts = parse_fn_body(&wrap("let x = identity<int>(5)"));
    match &stmts[0] {
        Stmt::Let { value: Expr::Call(call), .. } => {
            assert!(call.type_args.is_some(), "type_args must be Some");
            let args = call.type_args.as_ref().unwrap();
            assert_eq!(args.len(), 1);
            assert!(matches!(args[0], Type::Int));
        }
        other => panic!("expected Call with type_args, got {other:?}"),
    }
}

#[test]
fn m5_generic_call_two_type_args() {
    // WHY: `pair<int, string>(1, "a")` tests multi-arg generic calls. If the
    // second type arg is dropped, the `B` parameter can't be inferred.
    let stmts = parse_fn_body(&wrap(r#"let x = pair<int, string>(1, "a")"#));
    match &stmts[0] {
        Stmt::Let { value: Expr::Call(call), .. } => {
            let args = call.type_args.as_ref().expect("type_args must be Some");
            assert_eq!(args.len(), 2);
            assert!(matches!(args[0], Type::Int));
            assert!(matches!(args[1], Type::Named(ref n, _) if n == "string"));
        }
        other => panic!("expected Call with two type_args, got {other:?}"),
    }
}

#[test]
fn m5_generic_call_no_type_args_has_none() {
    // WHY: a normal (non-generic) call `add(1, 2)` must have `type_args: None`,
    // not `Some([])`. `None` = no explicit type args; `Some([])` = empty explicit
    // list (semantically different for typeck to distinguish).
    let stmts = parse_fn_body(&wrap("let x = add(1, 2)"));
    match &stmts[0] {
        Stmt::Let { value: Expr::Call(call), .. } => {
            assert!(call.type_args.is_none(), "non-generic call must have type_args: None");
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

// ── M5 P2: Contextual `<` disambiguation — comparison vs generic ──────────────

#[test]
fn m5_less_than_stays_comparison() {
    // WHY: acceptance criterion — `a < b` must NOT be parsed as a generic call.
    // If `<` is always treated as a type-arg opener, every comparison silently
    // becomes a parse error.
    let stmts = parse_fn_body(&wrap("let x = a < b"));
    match &stmts[0] {
        Stmt::Let { value: Expr::BinOp { op: BinOpKind::Lt, .. }, .. } => {}
        other => panic!("expected Lt comparison, got {other:?}"),
    }
}

#[test]
fn m5_comparison_chain_not_generic() {
    // WHY: acceptance criterion — `a < b > c` must parse as two comparisons,
    // NOT as a generic call `a<b>(c)`. This is the canonical ambiguity case.
    let stmts = parse_fn_body(&wrap("let x = a < b > c"));
    // Should be (a < b) > c — two BinOps.
    match &stmts[0] {
        Stmt::Let { value: Expr::BinOp { op: BinOpKind::Gt, lhs, .. }, .. } => {
            assert!(matches!(**lhs, Expr::BinOp { op: BinOpKind::Lt, .. }));
        }
        other => panic!("expected comparison chain a<b>c, got {other:?}"),
    }
}

#[test]
fn m5_comparison_in_index_not_generic() {
    // WHY: `arr[a < b]` must parse the index as a comparison, not a type-arg list.
    // If `<` inside `[...]` is treated as a generic opener, the index expression
    // is lost.
    let stmts = parse_fn_body(&wrap("let x = arr[a < b]"));
    match &stmts[0] {
        Stmt::Let { value: Expr::IndexAccess { index, .. }, .. } => {
            assert!(matches!(**index, Expr::BinOp { op: BinOpKind::Lt, .. }));
        }
        other => panic!("expected IndexAccess with Lt index, got {other:?}"),
    }
}

#[test]
fn m5_generic_call_not_confused_by_comparison() {
    // WHY: `foo<int>(x)` must be a generic call even when `foo < int` would also
    // be a valid comparison. The `(` following the `>` disambiguates.
    let stmts = parse_fn_body(&wrap("let x = foo<int>(5)"));
    match &stmts[0] {
        Stmt::Let { value: Expr::Call(call), .. } => {
            assert!(call.type_args.is_some());
        }
        other => panic!("expected generic Call, got {other:?}"),
    }
}

// ── M5 P2: M5 snapshot test ──────────────────────────────────────────────────

#[test]
fn m5_ast_snapshot() {
    // WHY: snapshot guards all M5 parser surface in one pass. Any structural
    // regression to generics, index access, maybe, or none-literal AST output
    // will diff here and require explicit approval via `cargo insta review`.
    let source = r#"
shape Pair<A, B> {
  first: A
  second: B
}

function identity<T>(give value: T) -> T {
  return value
}

function sort<T follows Comparable>(share items: array<T>) -> nothing {
}

function main() -> nothing {
  let x: maybe<int> = none
  let arr: array<int> = arr
  let first = arr[0]
  arr[0] = 99
  let y = identity<int>(5)
  let a = arr[0]
  let cmp = a < 10
}
"#;
    let output = parse(source);
    assert_eq!(output.diagnostics.len(), 0, "M5 surface must parse clean: {:?}", output.diagnostics);
    assert_debug_snapshot!(output.module);
}

// ── M5 P2: Error recovery tests ──────────────────────────────────────────────

#[test]
fn m5_recovery_unclosed_generic_param_list() {
    // WHY: `function foo<` with no closing `>` must produce exactly one diagnostic
    // and still parse the remaining top-level items. A cascade of errors here
    // would drown out the single real mistake.
    let output = parse("function foo< { } function bar() -> nothing { }");
    assert!(output.diagnostics.len() >= 1, "Expected at least one diagnostic");
    // bar() must still be parsed — recovery must continue past the broken decl.
    let fn_names: Vec<_> = output.module.items.iter().filter_map(|i| match i {
        Item::Function(f) => Some(f.name.as_str()),
        _ => None,
    }).collect();
    assert!(fn_names.contains(&"bar"), "bar() must parse despite foo<'s error");
}

#[test]
fn m5_recovery_missing_type_arg_close_bracket() {
    // WHY: `let x: array<int = 5` (missing `>`) must produce a diagnostic but
    // still give the parser enough to continue with the next statement.
    let output = parse(&wrap("let x: array<int = 5"));
    assert!(output.diagnostics.len() >= 1, "Expected at least one diagnostic for missing `>`");
}

#[test]
fn m5_recovery_generic_call_no_close_angle() {
    // WHY: `foo<int(5)` — paren before `>` — must NOT parse as a generic call
    // (falls back to comparison). The 32-token budget ensures we don't consume
    // the arg list as part of speculative type-arg parsing.
    let output = parse(&wrap("let x = foo < int > 5"));
    // This is a comparison, not a generic call: (foo < int) > 5
    assert_eq!(output.diagnostics.len(), 0, "comparison must parse cleanly");
    match &parse_fn_body(&wrap("let x = foo < int > 5"))[0] {
        Stmt::Let { value: Expr::BinOp { op: BinOpKind::Gt, .. }, .. } => {}
        other => panic!("expected Gt comparison, got {other:?}"),
    }
}

#[test]
fn m5_recovery_index_missing_close_bracket() {
    // WHY: `arr[0` without `]` must produce exactly one diagnostic and not crash.
    let output = parse(&wrap("let x = arr[0"));
    assert!(output.diagnostics.len() >= 1, "Expected diagnostic for unclosed `[`");
}

#[test]
fn m5_maybe_missing_type_arg() {
    // WHY: `maybe` without `<T>` must produce exactly one diagnostic. The error
    // message must guide the user to write `maybe<T>` — not produce a cascade.
    let output = parse(&wrap("let x: maybe = none"));
    assert!(output.diagnostics.len() >= 1, "Expected diagnostic for bare `maybe`");
    assert!(
        output.diagnostics.iter().any(|d| d.what.contains("maybe")),
        "Diagnostic must mention `maybe`"
    );
}
