//! Shared proptest strategies for `proptest_idempotency` and `proptest_smoke`.
//!
//! Both test files import this module via `mod common;`.  The smoke test's
//! strategy-quality gate runs against the SAME `arb_module` as the idempotency
//! test so the gate actually guards the strategy it claims to guard.

#![allow(dead_code)]

use proptest::prelude::*;
use proptest::strategy::BoxedStrategy;
use ynz_ast::nodes::{
    BinOpKind, Block, CallExpr, Expr, FieldDecl, FunctionDecl, ImportDecl, ImportItem, ImportKind,
    Item, Module, Param, ShapeDecl, Stmt, StringPart, StructLitField, Type, UnaryOpKind,
};
use ynz_diagnostics::SourceSpan;

pub fn zs() -> SourceSpan {
    SourceSpan::new("", 0, 0)
}

pub fn arb_ident() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("foo".to_string()),
        Just("bar".to_string()),
        Just("baz".to_string()),
        Just("val".to_string()),
        Just("result".to_string()),
        Just("num".to_string()),
        Just("flag".to_string()),
        Just("item".to_string()),
        Just("data".to_string()),
        Just("out".to_string()),
    ]
}

pub fn arb_simple_type() -> impl Strategy<Value = Type> {
    prop_oneof![
        Just(Type::Int),
        Just(Type::Float),
        Just(Type::Bool),
        Just(Type::Nothing),
        Just(Type::Named("string".to_string(), zs())),
    ]
}

pub fn arb_binop() -> impl Strategy<Value = BinOpKind> {
    prop_oneof![
        Just(BinOpKind::Add),
        Just(BinOpKind::Sub),
        Just(BinOpKind::Mul),
        Just(BinOpKind::EqEq),
        Just(BinOpKind::NotEq),
        Just(BinOpKind::Lt),
        Just(BinOpKind::Gt),
        Just(BinOpKind::And),
        Just(BinOpKind::Or),
    ]
}

pub fn arb_unary_op() -> impl Strategy<Value = UnaryOpKind> {
    prop_oneof![Just(UnaryOpKind::Neg), Just(UnaryOpKind::Not),]
}

pub fn arb_expr() -> BoxedStrategy<Expr> {
    prop_oneof![
        any::<i64>().prop_map(|v| Expr::IntLit(v, zs())),
        (any::<i32>(), 1u32..=99u32).prop_map(|(i, d)| Expr::NumberLit(format!("{i}.{d}"), zs())),
        any::<bool>().prop_map(|v| Expr::BoolLit(v, zs())),
        "[a-zA-Z0-9 ]{0,15}".prop_map(|s| Expr::StringLit(s.into_bytes(), zs())),
        arb_ident().prop_map(|n| Expr::Ident(n, zs())),
        // Non-empty backtick string to avoid adjacent-token merge (see WHY in
        // proptest_idempotency.rs Stmt::Expr CARVE-OUT comment)
        "[a-z]{1,10}".prop_map(|s| {
            Expr::InterpolatedString(vec![StringPart::Lit(s.into_bytes(), zs())], zs())
        }),
        arb_ident().prop_map(|name| {
            Expr::InterpolatedString(
                vec![
                    StringPart::Lit(b"v=".to_vec(), zs()),
                    StringPart::Expr(Box::new(Expr::Ident(name, zs())), zs()),
                ],
                zs(),
            )
        }),
        (any::<i64>(), arb_binop(), any::<i64>()).prop_map(|(l, op, r)| Expr::BinOp {
            op,
            lhs: Box::new(Expr::IntLit(l, zs())),
            rhs: Box::new(Expr::IntLit(r, zs())),
            span: zs(),
        }),
        (arb_unary_op(), any::<i64>()).prop_map(|(op, v)| Expr::UnaryOp {
            op,
            operand: Box::new(Expr::IntLit(v, zs())),
            span: zs(),
        }),
        (arb_ident(), arb_ident()).prop_map(|(callee, arg)| Expr::Call(Box::new(CallExpr {
            callee: Expr::Ident(callee, zs()),
            type_args: None,
            args: vec![Expr::Ident(arg, zs())],
            span: zs(),
        }))),
        (arb_ident(), arb_ident(), arb_ident()).prop_map(|(recv, method, arg)| {
            Expr::MethodCall {
                receiver: Box::new(Expr::Ident(recv, zs())),
                method,
                method_span: zs(),
                args: vec![Expr::Ident(arg, zs())],
                span: zs(),
            }
        }),
        prop::collection::vec(any::<i64>(), 1..=3).prop_map(|vals| Expr::ArrayLit {
            elements: vals.into_iter().map(|v| Expr::IntLit(v, zs())).collect(),
            span: zs(),
        }),
        (arb_ident(), any::<i64>()).prop_map(|(fname, v)| Expr::StructLit {
            fields: vec![StructLitField {
                name: fname,
                name_span: zs(),
                value: Expr::IntLit(v, zs()),
            }],
            span: zs(),
        }),
    ]
    .boxed()
}

pub fn arb_stmt() -> BoxedStrategy<Stmt> {
    prop_oneof![
        arb_expr().prop_map(|e| Stmt::Return {
            value: Some(e),
            span: zs(),
        }),
        (arb_ident(), arb_expr()).prop_map(|(name, value)| Stmt::Let {
            is_const: false,
            name,
            name_span: zs(),
            ty: None,
            value,
            span: zs(),
        }),
        (arb_ident(), arb_expr()).prop_map(|(name, value)| Stmt::Let {
            is_const: true,
            name,
            name_span: zs(),
            ty: None,
            value,
            span: zs(),
        }),
        // expression statement: call expressions only — backtick strings excluded
        // (see CARVE-OUT in proptest_idempotency.rs)
        (arb_ident(), arb_ident()).prop_map(|(callee, arg)| {
            Stmt::Expr(Expr::Call(Box::new(CallExpr {
                callee: Expr::Ident(callee, zs()),
                type_args: None,
                args: vec![Expr::Ident(arg, zs())],
                span: zs(),
            })))
        }),
        any::<bool>().prop_map(|v| Stmt::If {
            cond: Expr::BoolLit(v, zs()),
            body: Block {
                stmts: vec![Stmt::Return {
                    value: Some(Expr::IntLit(1, zs())),
                    span: zs(),
                }],
                span: zs(),
            },
            span: zs(),
        }),
        Just(Stmt::While {
            cond: Expr::BoolLit(false, zs()),
            body: Block {
                stmts: vec![],
                span: zs(),
            },
            span: zs(),
        }),
    ]
    .boxed()
}

pub fn arb_block() -> BoxedStrategy<Block> {
    prop::collection::vec(arb_stmt(), 0..=3)
        .prop_map(|stmts| Block { stmts, span: zs() })
        .boxed()
}

pub fn arb_param() -> BoxedStrategy<Param> {
    (arb_ident(), arb_simple_type())
        .prop_map(|(name, ty)| Param {
            name,
            name_span: zs(),
            ownership: None,
            ty,
            ty_span: zs(),
            span: zs(),
        })
        .boxed()
}

pub fn arb_doc() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        "[a-zA-Z0-9 ]{1,40}".prop_map(Some),
        ("[a-zA-Z0-9 ]{1,20}", "[a-zA-Z0-9 ]{1,20}")
            .prop_map(|(l1, l2)| Some(format!("{l1}\n{l2}"))),
    ]
}

pub fn arb_function() -> BoxedStrategy<FunctionDecl> {
    (
        arb_ident(),
        prop::collection::vec(arb_param(), 0..=3),
        arb_simple_type(),
        arb_block(),
        arb_doc(),
    )
        .prop_map(|(name, params, return_type, body, doc)| FunctionDecl {
            name,
            generics: vec![],
            params,
            return_type,
            body,
            span: zs(),
            name_span: zs(),
            errors_capable: false,
            is_exported: false,
            doc,
        })
        .boxed()
}

pub fn arb_shape() -> BoxedStrategy<ShapeDecl> {
    let field_strat = (arb_ident(), arb_simple_type())
        .prop_map(|(name, ty)| FieldDecl {
            name,
            name_span: zs(),
            ty,
            ty_span: zs(),
            is_hidden: false,
            default: None,
            span: zs(),
            doc: None,
        })
        .boxed();

    (
        arb_ident(),
        prop::collection::vec(field_strat, 1..=3),
        arb_doc(),
    )
        .prop_map(|(name, fields, doc)| ShapeDecl {
            name,
            name_span: zs(),
            is_base: false,
            generics: vec![],
            extends: None,
            follows: vec![],
            fields,
            contract_sigs: vec![],
            alias_ty: None,
            span: zs(),
            is_exported: false,
            doc,
        })
        .boxed()
}

pub fn arb_import() -> BoxedStrategy<ImportDecl> {
    (
        arb_ident(),
        "[a-z]{3,8}/[a-z]{3,8}".prop_filter("safe source path", |s| !s.contains("--")),
    )
        .prop_map(|(name, source)| ImportDecl {
            kind: ImportKind::Named(vec![ImportItem {
                exported_name: name.clone(),
                exported_name_span: zs(),
                local_name: name,
                local_name_span: zs(),
            }]),
            source,
            source_span: zs(),
            span: zs(),
        })
        .boxed()
}

pub fn arb_item() -> BoxedStrategy<Item> {
    prop_oneof![
        arb_function().prop_map(Item::Function),
        arb_shape().prop_map(Item::ShapeDecl),
        arb_import().prop_map(Item::ImportDecl),
    ]
    .boxed()
}

pub fn arb_module() -> BoxedStrategy<Module> {
    prop::collection::vec(arb_item(), 1..=3)
        .prop_map(|items| Module { items, span: zs() })
        .boxed()
}
