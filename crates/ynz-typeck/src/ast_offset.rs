//! AST node location utilities — find the identifier at a given byte offset.
//!
//! Used by the LSP to map a cursor position to a named symbol reference so
//! `resolve_symbol_at` can determine what the user is pointing at.
use ynz_ast::nodes::{
    Block, ConstDecl, Expr, FieldDecl, FunctionDecl, ImportDecl, ImportKind, Item,
    MatchPatternKind, Module, OptionsDecl, ShapeDecl, Stmt, StringPart, Type as AstType,
};
use ynz_diagnostics::SourceSpan;

/// Returns `(name, span)` if the byte offset falls inside any identifier-use-site
/// in the module.  An identifier-use-site is any position in source where a name
/// could be looked up: variable references, type annotations, function calls, field
/// accesses, import names, top-level const values.
///
/// Returns `None` for keywords, punctuation, literals, and whitespace.
///
/// Time: O(n) where n = AST nodes in the module.  Space: O(depth) call stack.
pub fn identifier_use_site_at_offset(
    module: &Module,
    byte_offset: usize,
) -> Option<(String, SourceSpan)> {
    for item in &module.items {
        match item {
            Item::Function(f) => {
                if let Some(r) = ident_in_function(f, byte_offset) {
                    return Some(r);
                }
            }
            Item::ShapeDecl(s) => {
                if let Some(r) = ident_in_shape(s, byte_offset) {
                    return Some(r);
                }
            }
            Item::OptionsDecl(o) => {
                if let Some(r) = ident_in_options(o, byte_offset) {
                    return Some(r);
                }
            }
            Item::ImportDecl(d) => {
                if let Some(r) = ident_in_import(d, byte_offset) {
                    return Some(r);
                }
            }
            Item::ConstDecl(c) => {
                if let Some(r) = ident_in_const(c, byte_offset) {
                    return Some(r);
                }
            }
            Item::ReExport(_) => {}
        }
    }
    None
}

fn ident_in_function(f: &FunctionDecl, offset: usize) -> Option<(String, SourceSpan)> {
    if span_contains(&f.name_span, offset) {
        return Some((f.name.clone(), f.name_span.clone()));
    }
    for p in &f.params {
        if span_contains(&p.name_span, offset) {
            return Some((p.name.clone(), p.name_span.clone()));
        }
        if let Some(r) = ident_in_ast_type(&p.ty, offset) {
            return Some(r);
        }
    }
    ident_in_block(&f.body, offset)
}

fn ident_in_shape(s: &ShapeDecl, offset: usize) -> Option<(String, SourceSpan)> {
    if span_contains(&s.name_span, offset) {
        return Some((s.name.clone(), s.name_span.clone()));
    }
    for field in &s.fields {
        if let Some(r) = ident_in_field(field, offset) {
            return Some(r);
        }
    }
    if let Some(alias) = &s.alias_ty {
        if let Some(r) = ident_in_ast_type(alias, offset) {
            return Some(r);
        }
    }
    None
}

fn ident_in_field(field: &FieldDecl, offset: usize) -> Option<(String, SourceSpan)> {
    if let Some(r) = ident_in_ast_type(&field.ty, offset) {
        return Some(r);
    }
    if let Some(expr) = &field.default {
        ident_in_expr(expr, offset)
    } else {
        None
    }
}

fn ident_in_options(o: &OptionsDecl, offset: usize) -> Option<(String, SourceSpan)> {
    if span_contains(&o.name_span, offset) {
        return Some((o.name.clone(), o.name_span.clone()));
    }
    None
}

fn ident_in_import(d: &ImportDecl, offset: usize) -> Option<(String, SourceSpan)> {
    match &d.kind {
        ImportKind::Named(items) => {
            for item in items {
                if span_contains(&item.local_name_span, offset) {
                    return Some((item.local_name.clone(), item.local_name_span.clone()));
                }
                if span_contains(&item.exported_name_span, offset) {
                    return Some((item.exported_name.clone(), item.exported_name_span.clone()));
                }
            }
        }
        ImportKind::Namespace {
            local_name,
            local_name_span,
        } => {
            if span_contains(local_name_span, offset) {
                return Some((local_name.clone(), local_name_span.clone()));
            }
        }
    }
    None
}

fn ident_in_const(c: &ConstDecl, offset: usize) -> Option<(String, SourceSpan)> {
    if span_contains(&c.name_span, offset) {
        return Some((c.name.clone(), c.name_span.clone()));
    }
    ident_in_expr(&c.value, offset)
}

fn ident_in_block(block: &Block, offset: usize) -> Option<(String, SourceSpan)> {
    for stmt in &block.stmts {
        if let Some(r) = ident_in_stmt(stmt, offset) {
            return Some(r);
        }
    }
    None
}

fn ident_in_stmt(stmt: &Stmt, offset: usize) -> Option<(String, SourceSpan)> {
    match stmt {
        Stmt::Let {
            name,
            name_span,
            ty,
            value,
            ..
        } => {
            if span_contains(name_span, offset) {
                return Some((name.clone(), name_span.clone()));
            }
            if let Some(t) = ty {
                if let Some(r) = ident_in_ast_type(t, offset) {
                    return Some(r);
                }
            }
            ident_in_expr(value, offset)
        }
        Stmt::Assign {
            target,
            target_span,
            value,
            ..
        } => {
            if span_contains(target_span, offset) {
                return Some((target.clone(), target_span.clone()));
            }
            ident_in_expr(value, offset)
        }
        Stmt::Expr(e) => ident_in_expr(e, offset),
        Stmt::Return { value, .. } => value.as_ref().and_then(|e| ident_in_expr(e, offset)),
        Stmt::If { cond, body, .. } => {
            ident_in_expr(cond, offset).or_else(|| ident_in_block(body, offset))
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            if let Some(r) = ident_in_expr(scrutinee, offset) {
                return Some(r);
            }
            for arm in arms {
                match &arm.pattern.kind {
                    MatchPatternKind::Value(e) => {
                        if let Some(r) = ident_in_expr(e, offset) {
                            return Some(r);
                        }
                    }
                    MatchPatternKind::Is(tp) => {
                        if span_contains(&tp.span, offset) {
                            return Some((tp.name.clone(), tp.span.clone()));
                        }
                    }
                    MatchPatternKind::OptionName(_) => {}
                }
                if let Some(r) = ident_in_block(&arm.body, offset) {
                    return Some(r);
                }
            }
            else_arm.as_ref().and_then(|b| ident_in_block(b, offset))
        }
        Stmt::While { cond, body, .. } => {
            ident_in_expr(cond, offset).or_else(|| ident_in_block(body, offset))
        }
        Stmt::For { iter, body, .. } => {
            ident_in_expr(iter, offset).or_else(|| ident_in_block(body, offset))
        }
        Stmt::FieldAssign { target, value, .. } => {
            ident_in_expr(target, offset).or_else(|| ident_in_expr(value, offset))
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => ident_in_expr(receiver, offset)
            .or_else(|| ident_in_expr(index, offset))
            .or_else(|| ident_in_expr(value, offset)),
    }
}

fn ident_in_expr(expr: &Expr, offset: usize) -> Option<(String, SourceSpan)> {
    match expr {
        Expr::Ident(name, span) => {
            if span_contains(span, offset) {
                Some((name.clone(), span.clone()))
            } else {
                None
            }
        }
        Expr::Call(call) => {
            if let Some(r) = ident_in_expr(&call.callee, offset) {
                return Some(r);
            }
            for arg in &call.args {
                if let Some(r) = ident_in_expr(arg, offset) {
                    return Some(r);
                }
            }
            None
        }
        Expr::MethodCall {
            receiver,
            method,
            method_span,
            args,
            ..
        } => {
            if span_contains(method_span, offset) {
                return Some((method.clone(), method_span.clone()));
            }
            if let Some(r) = ident_in_expr(receiver, offset) {
                return Some(r);
            }
            for arg in args {
                if let Some(r) = ident_in_expr(arg, offset) {
                    return Some(r);
                }
            }
            None
        }
        Expr::FieldAccess {
            receiver,
            field,
            field_span,
            ..
        } => {
            if span_contains(field_span, offset) {
                return Some((field.clone(), field_span.clone()));
            }
            ident_in_expr(receiver, offset)
        }
        Expr::BinOp { lhs, rhs, .. } => {
            ident_in_expr(lhs, offset).or_else(|| ident_in_expr(rhs, offset))
        }
        Expr::UnaryOp { operand, .. } => ident_in_expr(operand, offset),
        Expr::StructLit { fields, .. } => {
            for f in fields {
                if let Some(r) = ident_in_expr(&f.value, offset) {
                    return Some(r);
                }
            }
            None
        }
        Expr::PostfixOp { receiver, .. } => ident_in_expr(receiver, offset),
        Expr::IndexAccess {
            receiver, index, ..
        } => ident_in_expr(receiver, offset).or_else(|| ident_in_expr(index, offset)),
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                if let Some(r) = ident_in_expr(e, offset) {
                    return Some(r);
                }
            }
            None
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                if let Some(r) = ident_in_expr(k, offset) {
                    return Some(r);
                }
                if let Some(r) = ident_in_expr(v, offset) {
                    return Some(r);
                }
            }
            None
        }
        Expr::Is { expr, ty, .. } => {
            if span_contains(&ty.span, offset) {
                return Some((ty.name.clone(), ty.span.clone()));
            }
            ident_in_expr(expr, offset)
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let StringPart::Expr(e, _) = part {
                    if let Some(r) = ident_in_expr(e, offset) {
                        return Some(r);
                    }
                }
            }
            None
        }
        Expr::Wait(inner, _) | Expr::Background(inner, _) => ident_in_expr(inner, offset),
        // Leaves with no nested identifiers.
        Expr::StringLit(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::NoneLit { .. }
        | Expr::SelfValue { .. }
        | Expr::Error(..) => None,
    }
}

fn ident_in_ast_type(ty: &AstType, offset: usize) -> Option<(String, SourceSpan)> {
    match ty {
        AstType::Named(name, span) => {
            if span_contains(span, offset) {
                Some((name.clone(), span.clone()))
            } else {
                None
            }
        }
        AstType::Dynamic { contract, span } => {
            if span_contains(span, offset) {
                Some((contract.clone(), span.clone()))
            } else {
                None
            }
        }
        AstType::TypeParam { name, span } => {
            if span_contains(span, offset) {
                Some((name.clone(), span.clone()))
            } else {
                None
            }
        }
        AstType::Generic {
            name,
            name_span,
            args,
            ..
        } => {
            if span_contains(name_span, offset) {
                return Some((name.clone(), name_span.clone()));
            }
            for arg in args {
                if let Some(r) = ident_in_ast_type(arg, offset) {
                    return Some(r);
                }
            }
            None
        }
        AstType::Maybe { inner, .. }
        | AstType::ErrorCapable { inner, .. }
        | AstType::Sensitive(inner) => ident_in_ast_type(inner, offset),
        AstType::Union { variants, .. } => {
            for v in variants {
                if let Some(r) = ident_in_ast_type(v, offset) {
                    return Some(r);
                }
            }
            None
        }
        AstType::AnonShape { fields, .. } => {
            for f in fields {
                if let Some(r) = ident_in_ast_type(&f.ty, offset) {
                    return Some(r);
                }
            }
            None
        }
        AstType::Range { element, .. } => ident_in_ast_type(element, offset),
        // Primitive types — no nested named types.
        AstType::Int
        | AstType::Float
        | AstType::Number { .. }
        | AstType::Bool
        | AstType::Nothing
        | AstType::SelfType { .. }
        | AstType::Error => None,
    }
}

/// Returns `true` if `offset` is within `[span.start, span.end)`.
pub fn span_contains(span: &SourceSpan, offset: usize) -> bool {
    offset >= span.start && offset < span.end
}
