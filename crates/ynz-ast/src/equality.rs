//! Structural equality for AST nodes, ignoring span/position information.
//!
//! `ast_eq_modulo_trivia(a, b)` returns `true` when the two modules are
//! semantically identical — same structure, same names, same values — regardless
//! of where nodes appear in the source file.
//!
//! # Contract
//!
//! - `SourceSpan` fields: **IGNORED**.  Two ASTs that differ only in byte-offsets
//!   compare equal.
//! - `doc` fields: compared after `trim_end()` per line.  The formatter
//!   normalises trailing whitespace inside doc-comment lines; this comparator
//!   applies the same normalisation so re-parsing a formatted file still
//!   compares equal to the original.
//! - All other fields: **structural equality** (recursive).

use ynz_diagnostics::SourceSpan;

use crate::nodes::{
    Block, CallExpr, ConstDecl, ContractSig, Expr, FieldDecl, ForDestructureBinding, FunctionDecl,
    GenericParam, ImportDecl, ImportItem, ImportKind, Item, MatchArm, MatchPattern,
    MatchPatternKind, Module, OptionsDecl, OwnershipModifier, Param, PostfixOpKind, ReExport,
    ReExportItem, ReceiverKind, ShapeDecl, Stmt, StringPart, StructLitField, Type, TypePath,
};

/// Compare two `Module` ASTs for semantic equality, ignoring span/position
/// information and normalising doc-comment trailing whitespace.
pub fn ast_eq_modulo_trivia(a: &Module, b: &Module) -> bool {
    normalize_module(a.clone()) == normalize_module(b.clone())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn zero_span() -> SourceSpan {
    SourceSpan::new("", 0, 0)
}

fn normalize_doc(doc: Option<String>) -> Option<String> {
    doc.map(|s| {
        s.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

// ── Module ────────────────────────────────────────────────────────────────────

fn normalize_module(mut m: Module) -> Module {
    m.span = zero_span();
    m.items = m.items.into_iter().map(normalize_item).collect();
    m
}

fn normalize_item(item: Item) -> Item {
    match item {
        Item::Function(f) => Item::Function(normalize_function(f)),
        Item::ShapeDecl(s) => Item::ShapeDecl(normalize_shape_decl(s)),
        Item::OptionsDecl(o) => Item::OptionsDecl(normalize_options_decl(o)),
        Item::ImportDecl(i) => Item::ImportDecl(normalize_import_decl(i)),
        Item::ConstDecl(c) => Item::ConstDecl(normalize_const_decl(c)),
        Item::ReExport(r) => Item::ReExport(normalize_reexport(r)),
    }
}

// ── Function ──────────────────────────────────────────────────────────────────

fn normalize_function(mut f: FunctionDecl) -> FunctionDecl {
    f.span = zero_span();
    f.name_span = zero_span();
    f.generics = f
        .generics
        .into_iter()
        .map(normalize_generic_param)
        .collect();
    f.params = f.params.into_iter().map(normalize_param).collect();
    f.return_type = normalize_type(f.return_type);
    f.body = normalize_block(f.body);
    f.doc = normalize_doc(f.doc);
    f
}

fn normalize_generic_param(mut g: GenericParam) -> GenericParam {
    g.span = zero_span();
    g.name_span = zero_span();
    g.constraints = g
        .constraints
        .into_iter()
        .map(|(name, _)| (name, zero_span()))
        .collect();
    g
}

fn normalize_param(mut p: Param) -> Param {
    p.span = zero_span();
    p.name_span = zero_span();
    p.ty_span = zero_span();
    p.ty = normalize_type(p.ty);
    p
}

// ── Block / Statements ────────────────────────────────────────────────────────

fn normalize_block(mut b: Block) -> Block {
    b.span = zero_span();
    b.stmts = b.stmts.into_iter().map(normalize_stmt).collect();
    b
}

fn normalize_stmt(stmt: Stmt) -> Stmt {
    match stmt {
        Stmt::Expr(e) => Stmt::Expr(normalize_expr(e)),
        Stmt::Let {
            is_const,
            name,
            ty,
            value,
            ..
        } => Stmt::Let {
            is_const,
            name,
            name_span: zero_span(),
            ty: ty.map(normalize_type),
            value: normalize_expr(value),
            span: zero_span(),
        },
        Stmt::Assign { target, value, .. } => Stmt::Assign {
            target,
            target_span: zero_span(),
            value: normalize_expr(value),
            span: zero_span(),
        },
        Stmt::If { cond, body, .. } => Stmt::If {
            cond: normalize_expr(cond),
            body: normalize_block(body),
            span: zero_span(),
        },
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => Stmt::Match {
            scrutinee: normalize_expr(scrutinee),
            arms: arms.into_iter().map(normalize_match_arm).collect(),
            else_arm: else_arm.map(normalize_block),
            span: zero_span(),
        },
        Stmt::While { cond, body, .. } => Stmt::While {
            cond: normalize_expr(cond),
            body: normalize_block(body),
            span: zero_span(),
        },
        Stmt::For {
            var,
            iter,
            body,
            destructure_pattern,
            ..
        } => Stmt::For {
            var,
            var_span: zero_span(),
            iter: normalize_expr(iter),
            body: normalize_block(body),
            span: zero_span(),
            destructure_pattern,
        },
        Stmt::Return { value, .. } => Stmt::Return {
            value: value.map(normalize_expr),
            span: zero_span(),
        },
        Stmt::FieldAssign { target, value, .. } => Stmt::FieldAssign {
            target: Box::new(normalize_expr(*target)),
            value: normalize_expr(value),
            span: zero_span(),
        },
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => Stmt::IndexAssign {
            receiver: Box::new(normalize_expr(*receiver)),
            index: Box::new(normalize_expr(*index)),
            value: normalize_expr(value),
            span: zero_span(),
        },
    }
}

fn normalize_match_arm(arm: MatchArm) -> MatchArm {
    MatchArm {
        pattern: normalize_match_pattern(arm.pattern),
        body: normalize_block(arm.body),
        arrow_span: zero_span(),
    }
}

fn normalize_match_pattern(p: MatchPattern) -> MatchPattern {
    MatchPattern {
        kind: normalize_match_pattern_kind(p.kind),
        span: zero_span(),
    }
}

fn normalize_match_pattern_kind(k: MatchPatternKind) -> MatchPatternKind {
    match k {
        MatchPatternKind::Value(e) => MatchPatternKind::Value(normalize_expr(e)),
        MatchPatternKind::Is(tp) => MatchPatternKind::Is(normalize_type_path(tp)),
        MatchPatternKind::OptionName(n) => MatchPatternKind::OptionName(n),
    }
}

fn normalize_type_path(tp: TypePath) -> TypePath {
    TypePath {
        name: tp.name,
        span: zero_span(),
    }
}

// ── Expressions ───────────────────────────────────────────────────────────────

fn normalize_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Ident(n, _) => Expr::Ident(n, zero_span()),
        Expr::StringLit(b, _) => Expr::StringLit(b, zero_span()),
        Expr::IntLit(v, _) => Expr::IntLit(v, zero_span()),
        Expr::NumberLit(s, _) => Expr::NumberLit(s, zero_span()),
        Expr::BoolLit(v, _) => Expr::BoolLit(v, zero_span()),
        Expr::Error(_) => Expr::Error(zero_span()),
        Expr::Call(c) => Expr::Call(Box::new(normalize_call_expr(*c))),
        Expr::BinOp { op, lhs, rhs, .. } => Expr::BinOp {
            op,
            lhs: Box::new(normalize_expr(*lhs)),
            rhs: Box::new(normalize_expr(*rhs)),
            span: zero_span(),
        },
        Expr::UnaryOp { op, operand, .. } => Expr::UnaryOp {
            op,
            operand: Box::new(normalize_expr(*operand)),
            span: zero_span(),
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => Expr::MethodCall {
            receiver: Box::new(normalize_expr(*receiver)),
            method,
            method_span: zero_span(),
            args: args.into_iter().map(normalize_expr).collect(),
            span: zero_span(),
        },
        Expr::FieldAccess {
            receiver, field, ..
        } => Expr::FieldAccess {
            receiver: Box::new(normalize_expr(*receiver)),
            field,
            field_span: zero_span(),
            span: zero_span(),
        },
        Expr::StructLit { fields, .. } => Expr::StructLit {
            fields: fields.into_iter().map(normalize_struct_lit_field).collect(),
            span: zero_span(),
        },
        Expr::PostfixOp { receiver, op, .. } => Expr::PostfixOp {
            receiver: Box::new(normalize_expr(*receiver)),
            op,
            span: zero_span(),
        },
        Expr::SelfValue { .. } => Expr::SelfValue { span: zero_span() },
        Expr::NoneLit { .. } => Expr::NoneLit { span: zero_span() },
        Expr::IndexAccess {
            receiver, index, ..
        } => Expr::IndexAccess {
            receiver: Box::new(normalize_expr(*receiver)),
            index: Box::new(normalize_expr(*index)),
            span: zero_span(),
        },
        Expr::ArrayLit { elements, .. } => Expr::ArrayLit {
            elements: elements.into_iter().map(normalize_expr).collect(),
            span: zero_span(),
        },
        Expr::MapLit { entries, .. } => Expr::MapLit {
            entries: entries
                .into_iter()
                .map(|(k, v)| (normalize_expr(k), normalize_expr(v)))
                .collect(),
            span: zero_span(),
        },
        Expr::Is { expr, ty, .. } => Expr::Is {
            expr: Box::new(normalize_expr(*expr)),
            ty: normalize_type_path(ty),
            span: zero_span(),
        },
        Expr::InterpolatedString(parts, _) => Expr::InterpolatedString(
            parts.into_iter().map(normalize_string_part).collect(),
            zero_span(),
        ),
        Expr::Wait(e, _) => Expr::Wait(Box::new(normalize_expr(*e)), zero_span()),
        Expr::Background(e, _) => Expr::Background(Box::new(normalize_expr(*e)), zero_span()),
    }
}

fn normalize_string_part(part: StringPart) -> StringPart {
    match part {
        StringPart::Lit(b, _) => StringPart::Lit(b, zero_span()),
        StringPart::Expr(e, _) => StringPart::Expr(Box::new(normalize_expr(*e)), zero_span()),
    }
}

fn normalize_call_expr(mut c: CallExpr) -> CallExpr {
    c.span = zero_span();
    c.callee = normalize_expr(c.callee);
    c.type_args = c
        .type_args
        .map(|args| args.into_iter().map(normalize_type).collect());
    c.args = c.args.into_iter().map(normalize_expr).collect();
    c
}

fn normalize_struct_lit_field(f: StructLitField) -> StructLitField {
    StructLitField {
        name: f.name,
        name_span: zero_span(),
        value: normalize_expr(f.value),
    }
}

// ── Types ─────────────────────────────────────────────────────────────────────

fn normalize_type(ty: Type) -> Type {
    match ty {
        // Span-free unit variants — pass through unchanged.
        Type::Nothing | Type::Int | Type::Float | Type::Bool | Type::Error => ty,
        Type::Number { precision } => Type::Number { precision },
        Type::Named(n, _) => Type::Named(n, zero_span()),
        Type::Range {
            element,
            end_inclusive,
        } => Type::Range {
            element: Box::new(normalize_type(*element)),
            end_inclusive,
        },
        Type::Dynamic { contract, .. } => Type::Dynamic {
            contract,
            span: zero_span(),
        },
        Type::SelfType { .. } => Type::SelfType { span: zero_span() },
        Type::TypeParam { name, .. } => Type::TypeParam {
            name,
            span: zero_span(),
        },
        Type::Generic { name, args, .. } => Type::Generic {
            name,
            name_span: zero_span(),
            args: args.into_iter().map(normalize_type).collect(),
            span: zero_span(),
        },
        Type::Maybe { inner, .. } => Type::Maybe {
            inner: Box::new(normalize_type(*inner)),
            span: zero_span(),
        },
        Type::Union { variants, .. } => Type::Union {
            variants: variants.into_iter().map(normalize_type).collect(),
            span: zero_span(),
        },
        Type::ErrorCapable { inner, .. } => Type::ErrorCapable {
            inner: Box::new(normalize_type(*inner)),
            span: zero_span(),
        },
        Type::Sensitive(inner) => Type::Sensitive(Box::new(normalize_type(*inner))),
        Type::AnonShape { fields, .. } => Type::AnonShape {
            fields: fields.into_iter().map(normalize_field_decl).collect(),
            span: zero_span(),
        },
    }
}

// ── Shape / Options ───────────────────────────────────────────────────────────

fn normalize_field_decl(mut f: FieldDecl) -> FieldDecl {
    f.span = zero_span();
    f.name_span = zero_span();
    f.ty_span = zero_span();
    f.ty = normalize_type(f.ty);
    f.default = f.default.map(normalize_expr);
    f.doc = normalize_doc(f.doc);
    f
}

fn normalize_contract_sig(mut sig: ContractSig) -> ContractSig {
    sig.span = zero_span();
    sig.name_span = zero_span();
    sig.params = sig.params.into_iter().map(normalize_param).collect();
    sig.return_type = normalize_type(sig.return_type);
    sig
}

fn normalize_shape_decl(mut s: ShapeDecl) -> ShapeDecl {
    s.span = zero_span();
    s.name_span = zero_span();
    s.generics = s
        .generics
        .into_iter()
        .map(normalize_generic_param)
        .collect();
    s.extends = s.extends.map(|(name, _)| (name, zero_span()));
    s.follows = s
        .follows
        .into_iter()
        .map(|(name, _)| (name, zero_span()))
        .collect();
    s.fields = s.fields.into_iter().map(normalize_field_decl).collect();
    s.contract_sigs = s
        .contract_sigs
        .into_iter()
        .map(normalize_contract_sig)
        .collect();
    s.alias_ty = s.alias_ty.map(normalize_type);
    s.doc = normalize_doc(s.doc);
    s
}

fn normalize_options_decl(mut o: OptionsDecl) -> OptionsDecl {
    o.span = zero_span();
    o.name_span = zero_span();
    o.variants = o
        .variants
        .into_iter()
        .map(|(name, _, display)| (name, zero_span(), display))
        .collect();
    o.doc = normalize_doc(o.doc);
    o
}

// ── Imports / Exports ─────────────────────────────────────────────────────────

fn normalize_import_decl(mut i: ImportDecl) -> ImportDecl {
    i.span = zero_span();
    i.source_span = zero_span();
    i.kind = normalize_import_kind(i.kind);
    i
}

fn normalize_import_kind(kind: ImportKind) -> ImportKind {
    match kind {
        ImportKind::Named(items) => {
            ImportKind::Named(items.into_iter().map(normalize_import_item).collect())
        }
        ImportKind::Namespace { local_name, .. } => ImportKind::Namespace {
            local_name,
            local_name_span: zero_span(),
        },
    }
}

fn normalize_import_item(mut item: ImportItem) -> ImportItem {
    item.exported_name_span = zero_span();
    item.local_name_span = zero_span();
    item
}

fn normalize_const_decl(mut c: ConstDecl) -> ConstDecl {
    c.span = zero_span();
    c.name_span = zero_span();
    c.ty = c.ty.map(normalize_type);
    c.value = normalize_expr(c.value);
    c
}

fn normalize_reexport(mut r: ReExport) -> ReExport {
    r.span = zero_span();
    r.source_span = zero_span();
    r.items = r.items.into_iter().map(normalize_reexport_item).collect();
    r
}

fn normalize_reexport_item(mut item: ReExportItem) -> ReExportItem {
    item.name_span = zero_span();
    item.alias = item.alias.map(|(name, _)| (name, zero_span()));
    item
}

// ── Suppress dead-code warnings for exhaustiveness helpers ────────────────────

// These impls exist only to ensure the normalize_* functions above compile
// with exhaustive pattern matches.  `OwnershipModifier` and `ReceiverKind` are
// only used inside `Param` / `ContractSig` respectively and do not carry spans,
// so they pass through unchanged.
const _: fn(OwnershipModifier) = |_| {};
const _: fn(ReceiverKind) = |_| {};
const _: fn(PostfixOpKind) = |_| {};
const _: fn(ForDestructureBinding) = |_| {};

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_diagnostics::SourceSpan;

    fn s(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new("file.ynz", start, end)
    }

    fn int_fn(name: &str, ret_val: i64) -> FunctionDecl {
        FunctionDecl {
            name: name.to_string(),
            generics: vec![],
            params: vec![],
            return_type: Type::Int,
            body: Block {
                stmts: vec![Stmt::Return {
                    value: Some(Expr::IntLit(ret_val, s(10, 12))),
                    span: s(5, 15),
                }],
                span: s(4, 16),
            },
            span: s(0, 16),
            name_span: s(9, 12),
            errors_capable: false,
            is_exported: false,
            doc: None,
        }
    }

    fn simple_module(items: Vec<Item>) -> Module {
        Module {
            items,
            span: s(0, 100),
        }
    }

    // ── Mutations that MUST produce `false` ───────────────────────────────────

    #[test]
    fn mutation_rename_identifier_returns_false() {
        let a = simple_module(vec![Item::Function(int_fn("foo", 1))]);
        let mut b_fn = int_fn("bar", 1); // renamed
        b_fn.name = "bar".to_string();
        let b = simple_module(vec![Item::Function(b_fn)]);
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn mutation_swap_two_args_returns_false() {
        let make = |params: Vec<Param>| -> Module {
            let f = FunctionDecl {
                name: "f".to_string(),
                generics: vec![],
                params,
                return_type: Type::Nothing,
                body: Block {
                    stmts: vec![],
                    span: s(0, 2),
                },
                span: s(0, 50),
                name_span: s(9, 10),
                errors_capable: false,
                is_exported: false,
                doc: None,
            };
            simple_module(vec![Item::Function(f)])
        };

        let p1 = Param {
            name: "x".to_string(),
            name_span: s(0, 1),
            ownership: None,
            ty: Type::Int,
            ty_span: s(3, 6),
            span: s(0, 6),
        };
        let p2 = Param {
            name: "y".to_string(),
            name_span: s(8, 9),
            ownership: None,
            ty: Type::Bool,
            ty_span: s(11, 15),
            span: s(8, 15),
        };

        let a = make(vec![p1.clone(), p2.clone()]);
        let b = make(vec![p2, p1]); // swapped
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn mutation_change_operator_returns_false() {
        use crate::nodes::BinOpKind;
        let make_binop = |op: BinOpKind| -> Module {
            let expr = Expr::BinOp {
                op,
                lhs: Box::new(Expr::IntLit(1, s(0, 1))),
                rhs: Box::new(Expr::IntLit(2, s(4, 5))),
                span: s(0, 5),
            };
            let f = FunctionDecl {
                name: "f".to_string(),
                generics: vec![],
                params: vec![],
                return_type: Type::Int,
                body: Block {
                    stmts: vec![Stmt::Return {
                        value: Some(expr),
                        span: s(0, 5),
                    }],
                    span: s(0, 10),
                },
                span: s(0, 20),
                name_span: s(9, 10),
                errors_capable: false,
                is_exported: false,
                doc: None,
            };
            simple_module(vec![Item::Function(f)])
        };

        let a = make_binop(BinOpKind::Add);
        let b = make_binop(BinOpKind::Sub); // different op
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn mutation_change_literal_value_returns_false() {
        let a = simple_module(vec![Item::Function(int_fn("f", 42))]);
        let b = simple_module(vec![Item::Function(int_fn("f", 99))]);
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn mutation_move_doc_to_different_decl_returns_false() {
        let make = |first_doc: Option<String>, second_doc: Option<String>| -> Module {
            let mut f1 = int_fn("f1", 1);
            f1.doc = first_doc;
            let mut f2 = int_fn("f2", 2);
            f2.doc = second_doc;
            simple_module(vec![Item::Function(f1), Item::Function(f2)])
        };

        let a = make(Some("doc for f1".to_string()), None);
        let b = make(None, Some("doc for f1".to_string())); // doc moved to sibling
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn mutation_alter_doc_content_beyond_trailing_whitespace_returns_false() {
        let make = |doc: Option<String>| -> Module {
            let mut f = int_fn("f", 1);
            f.doc = doc;
            simple_module(vec![Item::Function(f)])
        };

        let a = make(Some("Returns one.".to_string()));
        let b = make(Some("Returns two.".to_string())); // different content
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn mutation_swap_doc_comments_between_siblings_returns_false() {
        // WHY: order-among-siblings sensitivity — swapping doc A↔B when A != B
        // must not compare equal.
        let make = |first_doc: &str, second_doc: &str| -> Module {
            let mut f1 = int_fn("f1", 1);
            f1.doc = Some(first_doc.to_string());
            let mut f2 = int_fn("f2", 2);
            f2.doc = Some(second_doc.to_string());
            simple_module(vec![Item::Function(f1), Item::Function(f2)])
        };

        let a = make("doc for f1", "doc for f2");
        let b = make("doc for f2", "doc for f1"); // docs swapped between siblings
        assert!(!ast_eq_modulo_trivia(&a, &b));
    }

    // ── Trivia-only changes that MUST produce `true` ──────────────────────────

    #[test]
    fn trivia_rebump_all_spans_returns_true() {
        let a = simple_module(vec![Item::Function(int_fn("f", 1))]);
        let mut b = simple_module(vec![Item::Function(int_fn("f", 1))]);
        // Modify every span in b
        b.span = s(999, 9999);
        if let Item::Function(ref mut f) = b.items[0] {
            f.span = s(100, 200);
            f.name_span = s(109, 110);
            if let Stmt::Return { ref mut span, .. } = f.body.stmts[0] {
                *span = s(50, 60);
            }
        }
        assert!(ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn trivia_trailing_whitespace_in_doc_line_returns_true() {
        let make = |doc: &str| -> Module {
            let mut f = int_fn("f", 1);
            f.doc = Some(doc.to_string());
            simple_module(vec![Item::Function(f)])
        };

        // "line 1  " (trailing spaces) vs "line 1" — should be equal after trim_end.
        let a = make("line 1  ");
        let b = make("line 1");
        assert!(ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn trivia_different_file_name_in_spans_returns_true() {
        let make_in = |file: &str| -> Module {
            Module {
                items: vec![Item::Function(FunctionDecl {
                    name: "f".to_string(),
                    generics: vec![],
                    params: vec![],
                    return_type: Type::Int,
                    body: Block {
                        stmts: vec![Stmt::Return {
                            value: Some(Expr::IntLit(1, SourceSpan::new(file, 10, 12))),
                            span: SourceSpan::new(file, 5, 15),
                        }],
                        span: SourceSpan::new(file, 4, 16),
                    },
                    span: SourceSpan::new(file, 0, 16),
                    name_span: SourceSpan::new(file, 9, 12),
                    errors_capable: false,
                    is_exported: false,
                    doc: None,
                })],
                span: SourceSpan::new(file, 0, 100),
            }
        };

        let a = make_in("file_a.ynz");
        let b = make_in("file_b.ynz");
        assert!(ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn trivia_doc_none_vs_none_returns_true() {
        let a = simple_module(vec![Item::Function(int_fn("f", 1))]);
        let b = simple_module(vec![Item::Function(int_fn("f", 1))]);
        assert!(ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn trivia_same_doc_different_trailing_whitespace_multiline_returns_true() {
        let make = |doc: &str| -> Module {
            let mut f = int_fn("f", 1);
            f.doc = Some(doc.to_string());
            simple_module(vec![Item::Function(f)])
        };

        let a = make("line 1  \nline 2   ");
        let b = make("line 1\nline 2");
        assert!(ast_eq_modulo_trivia(&a, &b));
    }

    #[test]
    fn trivia_offset_shifted_all_spans_returns_true() {
        // Two identical programs except one has been shifted right by 100 bytes
        // (as if there were 100 bytes of header before it).
        let make_at = |offset: usize| -> Module {
            Module {
                items: vec![Item::Function(FunctionDecl {
                    name: "f".to_string(),
                    generics: vec![],
                    params: vec![],
                    return_type: Type::Int,
                    body: Block {
                        stmts: vec![Stmt::Return {
                            value: Some(Expr::IntLit(1, s(offset + 10, offset + 12))),
                            span: s(offset + 5, offset + 15),
                        }],
                        span: s(offset + 4, offset + 16),
                    },
                    span: s(offset, offset + 16),
                    name_span: s(offset + 9, offset + 12),
                    errors_capable: false,
                    is_exported: false,
                    doc: None,
                })],
                span: s(offset, offset + 100),
            }
        };

        let a = make_at(0);
        let b = make_at(100);
        assert!(ast_eq_modulo_trivia(&a, &b));
    }
}
