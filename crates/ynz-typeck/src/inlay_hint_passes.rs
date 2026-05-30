//! Inlay-hint detection passes for the v0.2-M5 LSP teaching surfaces.
//!
//! Each pass is a salsa-tracked function so repeated LSP requests at the same
//! file version reuse the computed hints.  Cache is invalidated when source changes.
//!
//! # Firing domains (5 of 9)
//!
//! - `variable_type_hints`            — `: TypeName` after un-annotated `let` bindings
//! - `ownership_call_site_hints`      — `share`/`lend`/`give` after call arguments
//! - `copy_point_hints`               — `.copy (N bytes)` for trivially-copyable passes
//! - `array_to_fixed_promotion_hints` — decoration on never-grown `array<T>` bindings
//! - `let_to_const_promotion_hints`   — decoration on never-mutated `let` bindings
//!
//! # Protocol-only domains (4 of 9)
//!
//! `function_param_type`, `wait_points`, `lifetimes`, `allocators` are handled
//! by the LSP layer but return empty hint lists until v0.3+ data exists.

use std::collections::HashSet;

use ynz_ast::nodes::{Block, Expr, Item, OwnershipModifier, Stmt};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    queries::{check_query, module_signatures_query},
    types::{type_name, Type},
};

// ─────────────────────────────────────────────────────────────────────────────
// Public hint structs
// ─────────────────────────────────────────────────────────────────────────────

/// Inferred type annotation for an un-annotated `let` binding.
///
/// Rendered as `: TypeName` at `position` (byte offset after the binding name).
#[derive(Clone, Debug, PartialEq)]
pub struct TypeHint {
    pub position: usize,
    /// The inferred type as a Yinz type-name string, e.g. `"int"` or `"Player"`.
    pub type_text: String,
}

/// Ownership modifier hint at a function-call argument.
///
/// Rendered as `share`, `lend`, or `give` at `position` (after the argument).
#[derive(Clone, Debug, PartialEq)]
pub struct OwnershipHint {
    pub position: usize,
    /// `"share"`, `"lend"`, or `"give"`.
    pub modifier: String,
}

/// Trivially-copyable copy-point hint at a function-call argument.
///
/// Rendered as `.copy (N bytes, trivially copyable)` at `position`.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyHint {
    pub position: usize,
    /// Human-readable size annotation, e.g. `"8 bytes"`.
    pub size_text: String,
}

/// Auto-promotion hint kind.
#[derive(Clone, Debug, PartialEq)]
pub enum PromotionKind {
    /// `array<T>` → `fixed<T>` (binding never grown).
    ArrayToFixed,
    /// `let` → `const` (binding never reassigned or mutated).
    LetToConst,
}

/// Decoration hint for an auto-promotion opportunity.
#[derive(Clone, Debug, PartialEq)]
pub struct PromotionHint {
    /// Byte offset of the `let` keyword (the token to decorate).
    pub position: usize,
    pub kind: PromotionKind,
    /// Short label shown in the hint, e.g. `"// effectively const — never reassigned"`.
    pub label: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn is_trivially_copyable(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::Bool | Type::Number { .. })
}

fn copy_size_text(ty: &Type) -> &'static str {
    match ty {
        Type::Bool => "1 byte",
        Type::Int | Type::Float => "8 bytes",
        Type::Number { .. } => "16 bytes",
        _ => "N bytes",
    }
}

/// Check whether the AST type annotation (`ynz_ast::nodes::Type`) is `array<T>`.
fn ast_ty_is_array(ty: &ynz_ast::nodes::Type) -> bool {
    matches!(ty, ynz_ast::nodes::Type::Generic { name, .. } if name == "array")
}

/// Conservative mutation-name collector: walks a block and records every identifier
/// that appears in a position that could indicate mutation (assignment target,
/// receiver of a method call, argument to any function — conservative).
fn collect_maybe_mutated(block: &Block, out: &mut HashSet<String>) {
    for stmt in &block.stmts {
        collect_maybe_mutated_stmt(stmt, out);
    }
}

fn collect_maybe_mutated_stmt(stmt: &Stmt, out: &mut HashSet<String>) {
    match stmt {
        Stmt::Assign { target, value, .. } => {
            out.insert(target.clone());
            collect_maybe_mutated_expr(value, out);
        }
        Stmt::FieldAssign { target, value, .. } => {
            // Mutates the receiver of the field access.
            if let Expr::FieldAccess { receiver, .. } = target.as_ref() {
                if let Expr::Ident(name, _) = receiver.as_ref() {
                    out.insert(name.clone());
                }
            }
            collect_maybe_mutated_expr(value, out);
        }
        Stmt::IndexAssign { receiver, index, value, .. } => {
            if let Expr::Ident(name, _) = receiver.as_ref() {
                out.insert(name.clone());
            }
            collect_maybe_mutated_expr(index, out);
            collect_maybe_mutated_expr(value, out);
        }
        Stmt::Let { value, .. } => collect_maybe_mutated_expr(value, out),
        Stmt::Expr(e) => collect_maybe_mutated_expr(e, out),
        Stmt::Return { value: Some(e), .. } => collect_maybe_mutated_expr(e, out),
        Stmt::If { cond, body, .. } => {
            collect_maybe_mutated_expr(cond, out);
            collect_maybe_mutated(body, out);
        }
        Stmt::Match { scrutinee, arms, else_arm, .. } => {
            collect_maybe_mutated_expr(scrutinee, out);
            for arm in arms {
                collect_maybe_mutated(&arm.body, out);
            }
            if let Some(eb) = else_arm {
                collect_maybe_mutated(eb, out);
            }
        }
        Stmt::While { cond, body, .. } => {
            collect_maybe_mutated_expr(cond, out);
            collect_maybe_mutated(body, out);
        }
        Stmt::For { iter, body, .. } => {
            collect_maybe_mutated_expr(iter, out);
            collect_maybe_mutated(body, out);
        }
        _ => {}
    }
}

fn collect_maybe_mutated_expr(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        // Any call-site: conservatively mark all ident arguments as "possibly mutated"
        // because we don't know the callee's parameter ownership here.
        Expr::Call(c) => {
            for arg in &c.args {
                if let Expr::Ident(name, _) = arg {
                    out.insert(name.clone());
                }
                collect_maybe_mutated_expr(arg, out);
            }
            collect_maybe_mutated_expr(&c.callee, out);
        }
        Expr::MethodCall { receiver, args, .. } => {
            // Any method call on a binding potentially mutates it.
            if let Expr::Ident(name, _) = receiver.as_ref() {
                out.insert(name.clone());
            }
            collect_maybe_mutated_expr(receiver, out);
            for arg in args {
                if let Expr::Ident(name, _) = arg {
                    out.insert(name.clone());
                }
                collect_maybe_mutated_expr(arg, out);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_maybe_mutated_expr(lhs, out);
            collect_maybe_mutated_expr(rhs, out);
        }
        Expr::UnaryOp { operand, .. } => collect_maybe_mutated_expr(operand, out),
        Expr::FieldAccess { receiver, .. } => collect_maybe_mutated_expr(receiver, out),
        Expr::IndexAccess { receiver, index, .. } => {
            collect_maybe_mutated_expr(receiver, out);
            collect_maybe_mutated_expr(index, out);
        }
        _ => {}
    }
}

fn expr_span_key(expr: &Expr) -> (usize, usize) {
    let span = expr.span();
    (span.start, span.end)
}

// ─────────────────────────────────────────────────────────────────────────────
// Salsa-tracked detection passes
// ─────────────────────────────────────────────────────────────────────────────

/// Emit `: TypeName` hints after each un-annotated `let` binding.
///
/// Suppressed when `ty: Some(_)` (explicit annotation already present) or when
/// the inferred type is `Error`/`Nothing`/`Infer` (incomplete inference).
///
/// Time: O(n) AST walk.  Space: O(hints).
#[salsa::tracked]
pub fn variable_type_hints(db: &dyn SourceFileRegistry, source: SourceFile) -> Vec<TypeHint> {
    let parse = parse_query(db, source);
    let check = check_query(db, source);
    let expr_types = &check.typed_module.expr_types;

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            collect_type_hints_block(&f.body, expr_types, &mut hints);
        }
    }
    hints
}

fn collect_type_hints_block(
    block: &Block,
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
    out: &mut Vec<TypeHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { ty: None, name_span, value, .. } => {
                if let Some(t) = expr_types.get(&expr_span_key(value)) {
                    if !matches!(t, Type::Error | Type::Nothing) {
                        out.push(TypeHint {
                            position: name_span.end,
                            type_text: type_name(t),
                        });
                    }
                }
            }
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_type_hints_block(body, expr_types, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_type_hints_block(&arm.body, expr_types, out);
                }
                if let Some(eb) = else_arm {
                    collect_type_hints_block(eb, expr_types, out);
                }
            }
            _ => {}
        }
    }
}

/// Emit `share`/`lend`/`give` hints after call-site arguments.
///
/// Only fires when the callee's signature is resolved and the parameter carries
/// an explicit ownership modifier.  Suppressed for generics, imports not in
/// scope, and method calls (UFCS dispatch is not yet tracked here).
///
/// Time: O(n × signature-lookup).  Space: O(hints).
#[salsa::tracked]
pub fn ownership_call_site_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<OwnershipHint> {
    let parse = parse_query(db, source);
    let sigs = module_signatures_query(db, source);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            collect_ownership_hints_block(
                &f.body,
                &sigs.sig_table,
                &sigs.imported_fns,
                &mut hints,
            );
        }
    }
    hints
}

fn collect_ownership_hints_block(
    block: &Block,
    sig_table: &crate::signatures::SignatureTable,
    imported: &std::collections::HashMap<String, crate::signatures::FunctionSig>,
    out: &mut Vec<OwnershipHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(e) | Stmt::Let { value: e, .. } => {
                collect_ownership_hints_expr(e, sig_table, imported, out);
            }
            Stmt::Assign { value, .. } | Stmt::FieldAssign { value, .. } => {
                collect_ownership_hints_expr(value, sig_table, imported, out);
            }
            Stmt::IndexAssign { index, value, .. } => {
                collect_ownership_hints_expr(index, sig_table, imported, out);
                collect_ownership_hints_expr(value, sig_table, imported, out);
            }
            Stmt::If { cond, body, .. } => {
                collect_ownership_hints_expr(cond, sig_table, imported, out);
                collect_ownership_hints_block(body, sig_table, imported, out);
            }
            Stmt::Match { scrutinee, arms, else_arm, .. } => {
                collect_ownership_hints_expr(scrutinee, sig_table, imported, out);
                for arm in arms {
                    collect_ownership_hints_block(&arm.body, sig_table, imported, out);
                }
                if let Some(eb) = else_arm {
                    collect_ownership_hints_block(eb, sig_table, imported, out);
                }
            }
            Stmt::While { cond, body, .. } => {
                collect_ownership_hints_expr(cond, sig_table, imported, out);
                collect_ownership_hints_block(body, sig_table, imported, out);
            }
            Stmt::For { iter, body, .. } => {
                collect_ownership_hints_expr(iter, sig_table, imported, out);
                collect_ownership_hints_block(body, sig_table, imported, out);
            }
            _ => {}
        }
    }
}

fn collect_ownership_hints_expr(
    expr: &Expr,
    sig_table: &crate::signatures::SignatureTable,
    imported: &std::collections::HashMap<String, crate::signatures::FunctionSig>,
    out: &mut Vec<OwnershipHint>,
) {
    if let Expr::Call(c) = expr {
        // Resolve callee name for free-function calls.
        if let Expr::Ident(name, _) = &c.callee {
            let sig_opt = sig_table.fns.get(name).or_else(|| imported.get(name));
            if let Some(sig) = sig_opt {
                for (i, arg) in c.args.iter().enumerate() {
                    if let Some(Some(own)) = sig.param_ownerships.get(i) {
                        let modifier = match own {
                            OwnershipModifier::Share => "share",
                            OwnershipModifier::Lend => "lend",
                            OwnershipModifier::Give => "give",
                        };
                        out.push(OwnershipHint {
                            position: arg.span().end,
                            modifier: modifier.to_string(),
                        });
                    }
                    collect_ownership_hints_expr(arg, sig_table, imported, out);
                }
                return;
            }
        }
        // Fallback: recurse into args without hints.
        for arg in &c.args {
            collect_ownership_hints_expr(arg, sig_table, imported, out);
        }
        collect_ownership_hints_expr(&c.callee, sig_table, imported, out);
    }
}

/// Emit `.copy (N bytes, trivially copyable)` hints at trivially-copyable pass sites.
///
/// `int`, `float`, `bool`, and `number` values always copy on pass — no heap
/// allocation, no ownership transfer.  The hint makes this explicit so the user
/// understands why there's no ownership modifier.
///
/// Time: O(n).  Space: O(hints).
#[salsa::tracked]
pub fn copy_point_hints(db: &dyn SourceFileRegistry, source: SourceFile) -> Vec<CopyHint> {
    let parse = parse_query(db, source);
    let check = check_query(db, source);
    let expr_types = &check.typed_module.expr_types;

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            collect_copy_hints_block(&f.body, expr_types, &mut hints);
        }
    }
    hints
}

fn collect_copy_hints_block(
    block: &Block,
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
    out: &mut Vec<CopyHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(e) | Stmt::Let { value: e, .. } => {
                collect_copy_hints_expr(e, expr_types, out);
            }
            Stmt::If { cond, body, .. } => {
                collect_copy_hints_expr(cond, expr_types, out);
                collect_copy_hints_block(body, expr_types, out);
            }
            Stmt::Match { scrutinee, arms, else_arm, .. } => {
                collect_copy_hints_expr(scrutinee, expr_types, out);
                for arm in arms {
                    collect_copy_hints_block(&arm.body, expr_types, out);
                }
                if let Some(eb) = else_arm {
                    collect_copy_hints_block(eb, expr_types, out);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_copy_hints_block(body, expr_types, out);
            }
            _ => {}
        }
    }
}

fn collect_copy_hints_expr(
    expr: &Expr,
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
    out: &mut Vec<CopyHint>,
) {
    if let Expr::Call(c) = expr {
        for arg in &c.args {
            if let Some(ty) = expr_types.get(&expr_span_key(arg)) {
                if is_trivially_copyable(ty) {
                    out.push(CopyHint {
                        position: arg.span().end,
                        size_text: copy_size_text(ty).to_string(),
                    });
                }
            }
        }
    }
}

/// Emit `// promoted to fixed` decorations on `array<T>` bindings never grown.
///
/// Conservative: if the binding name appears in ANY call argument, method call,
/// or assignment target, the hint is suppressed (the call might mutate via lend).
///
/// Time: O(n × binding-count).  Space: O(hints).
#[salsa::tracked]
pub fn array_to_fixed_promotion_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<PromotionHint> {
    let parse = parse_query(db, source);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            let mut mutated = HashSet::new();
            collect_maybe_mutated(&f.body, &mut mutated);
            collect_array_hints_block(&f.body, &mutated, &mut hints);
        }
    }
    hints
}

fn collect_array_hints_block(block: &Block, mutated: &HashSet<String>, out: &mut Vec<PromotionHint>) {
    for stmt in &block.stmts {
        if let Stmt::Let { ty: Some(ty_ann), name, span, .. } = stmt {
            if ast_ty_is_array(ty_ann) && !mutated.contains(name) {
                out.push(PromotionHint {
                    position: span.start,
                    kind: PromotionKind::ArrayToFixed,
                    label: "// promoted to fixed — never grown".to_string(),
                });
            }
        }
        match stmt {
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_array_hints_block(body, mutated, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_array_hints_block(&arm.body, mutated, out);
                }
                if let Some(eb) = else_arm {
                    collect_array_hints_block(eb, mutated, out);
                }
            }
            _ => {}
        }
    }
}

/// Emit `// effectively const` decorations on `let` bindings never reassigned/mutated.
///
/// Conservative: any binding that appears in a call argument, method receiver,
/// or assignment target is excluded (the operation might mutate via lend).
///
/// `const` bindings are excluded (they already are const).
///
/// Time: O(n × binding-count).  Space: O(hints).
#[salsa::tracked]
pub fn let_to_const_promotion_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<PromotionHint> {
    let parse = parse_query(db, source);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            let mut mutated = HashSet::new();
            collect_maybe_mutated(&f.body, &mut mutated);
            collect_const_hints_block(&f.body, &mutated, &mut hints);
        }
    }
    hints
}

fn collect_const_hints_block(block: &Block, mutated: &HashSet<String>, out: &mut Vec<PromotionHint>) {
    for stmt in &block.stmts {
        if let Stmt::Let { is_const: false, name, span, .. } = stmt {
            if !mutated.contains(name) {
                out.push(PromotionHint {
                    position: span.start,
                    kind: PromotionKind::LetToConst,
                    label: "// effectively const — never reassigned".to_string(),
                });
            }
        }
        match stmt {
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_const_hints_block(body, mutated, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_const_hints_block(&arm.body, mutated, out);
                }
                if let Some(eb) = else_arm {
                    collect_const_hints_block(eb, mutated, out);
                }
            }
            _ => {}
        }
    }
}
