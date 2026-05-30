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
use std::sync::OnceLock;

use ynz_ast::nodes::{Block, Expr, Item, OwnershipModifier, Stmt};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    generics::GenericFnTable,
    intrinsics::PrimitiveIntrinsicTable,
    queries::{check_query, module_signatures_query},
    signatures::{FunctionSig, SignatureTable},
    types::{type_name, Type},
};

// ─────────────────────────────────────────────────────────────────────────────
// Builtin callee ownership helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` when `name` is a builtin free-function whose every parameter
/// is read-only (share).
///
/// All v0.1/v0.2 builtin free-fns are read-only by contract — they inspect
/// their arguments and never take ownership or mutate through a borrow.
/// `range` and `sleepMs` are in `registry/features.toml` as `kind = "free_fn"`;
/// `print` and `sensitive` are special-cased in `check.rs` and not in the
/// free-fn registry table, so they are listed here explicitly.
///
/// When the registry gains an ownership field for free-fn entries (cross-file
/// sig data), replace the explicit list with a registry-sourced lookup.
///
/// Time: O(n) where n = number of registry free-fn names (small, <10).  Space: O(1).
fn builtin_free_fn_is_readonly(name: &str) -> bool {
    // Builtins special-cased in check.rs (not in PrimitiveIntrinsicTable.free_fns).
    if matches!(name, "print" | "sensitive") {
        return true;
    }
    // Registry free-fn builtins (range overloads, sleepMs, and any future additions).
    // PrimitiveIntrinsicTable::free_fn_names() is the SSOT for this list.
    static REGISTRY_FREE_FNS: OnceLock<Vec<&'static str>> = OnceLock::new();
    let registry_names =
        REGISTRY_FREE_FNS.get_or_init(|| PrimitiveIntrinsicTable::m6().free_fn_names());
    registry_names.contains(&name)
}

/// Returns `true` when `method` is a scalar primitive intrinsic method (e.g.
/// `.toString()`, `.toFloat()`, `.byteAt()`, `.contains()`) and therefore always
/// takes a read-only (share) receiver.
///
/// The set is sourced from `PrimitiveIntrinsicTable::all_scalar_intrinsic_method_names()`
/// which covers both zero-arg and one-arg scalar intrinsics.  Collection methods
/// (`add`, `remove`, `set`, etc.) are excluded because `build_table` already
/// skips `array`, `fixed`, `maybe`, and `map` receiver types.
///
/// Time: O(1) amortised (HashSet lookup; set built once via OnceLock).
/// Space: O(m) where m = number of scalar intrinsic method names (<40).
fn primitive_intrinsic_method_is_readonly(method: &str) -> bool {
    static INTRINSIC_METHODS: OnceLock<HashSet<String>> = OnceLock::new();
    let set = INTRINSIC_METHODS.get_or_init(|| {
        PrimitiveIntrinsicTable::m6()
            .all_scalar_intrinsic_method_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    });
    set.contains(method)
}

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

/// Walk a chained field/index access expression to the root `Expr::Ident`, returning
/// its name.  Returns `None` when the chain does not bottom out at an identifier (e.g.
/// a method-call result used as the base — rare, and not a named binding the mutation
/// collector tracks).
///
/// Time: O(d)  Space: O(1)  where d = receiver-chain depth
fn root_ident(mut expr: &Expr) -> Option<&str> {
    loop {
        match expr {
            Expr::Ident(name, _) => return Some(name.as_str()),
            Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. } => {
                expr = receiver.as_ref();
            }
            _ => return None,
        }
    }
}

/// Ownership-aware mutation-name collector: walks a block and records every identifier
/// that appears in a position that indicates mutation — assignment target, receiver of a
/// mutating method call, or argument to a `lend`/`give` parameter.
///
/// `share` parameters (and all builtin free-fns and scalar primitive intrinsic methods)
/// do NOT count as mutations — bindings only passed to read-only callees remain
/// candidates for the `let→const` hint.
///
/// **Conservative fallback for unresolvable callees**: when a callee cannot be resolved
/// through sig_table, imported, generic_fn_table, or the builtin/intrinsic sets, every
/// ident argument is marked mutated.  The tradeoff: a missed hint on a binding whose
/// actual ownership is unknown is acceptable; a wrong "effectively const" hint on a
/// binding that IS mutated is not — it trains users to distrust the teaching surface.
/// **Cost**: imported functions (cross-file user code) suppress `let→const` hints on
/// their arguments even when those functions are genuinely read-only, because their
/// signatures are not yet resolved at hint-analysis time.
/// **Trigger to narrow the fallback**: when cross-file signature data is available at
/// the call site (imported-function ownership recorded in the registry or resolved via
/// the cross-file salsa query), replace the conservative fallback with a registry-sourced
/// ownership lookup — at that point, only truly-unknown callees (e.g. indirect calls
/// through a function-typed value) need the conservative path.
///
/// Time: O(n × sig-lookup)  Space: O(mutated-set)
fn collect_maybe_mutated(
    block: &Block,
    sig_table: &SignatureTable,
    imported: &std::collections::HashMap<String, FunctionSig>,
    generic_fn_table: &GenericFnTable,
    out: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        collect_maybe_mutated_stmt(stmt, sig_table, imported, generic_fn_table, out);
    }
}

fn collect_maybe_mutated_stmt(
    stmt: &Stmt,
    sig_table: &SignatureTable,
    imported: &std::collections::HashMap<String, FunctionSig>,
    generic_fn_table: &GenericFnTable,
    out: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Assign { target, value, .. } => {
            out.insert(target.clone());
            collect_maybe_mutated_expr(value, sig_table, imported, generic_fn_table, out);
        }
        Stmt::FieldAssign { target, value, .. } => {
            // Follow any chained field-access path to the root binding name.
            if let Some(name) = root_ident(target.as_ref()) {
                out.insert(name.to_string());
            }
            collect_maybe_mutated_expr(value, sig_table, imported, generic_fn_table, out);
        }
        Stmt::IndexAssign { receiver, index, value, .. } => {
            // Follow any chained index-access path to the root binding name.
            if let Some(name) = root_ident(receiver.as_ref()) {
                out.insert(name.to_string());
            }
            collect_maybe_mutated_expr(index, sig_table, imported, generic_fn_table, out);
            collect_maybe_mutated_expr(value, sig_table, imported, generic_fn_table, out);
        }
        Stmt::Let { value, .. } => {
            collect_maybe_mutated_expr(value, sig_table, imported, generic_fn_table, out);
        }
        Stmt::Expr(e) => {
            collect_maybe_mutated_expr(e, sig_table, imported, generic_fn_table, out);
        }
        Stmt::Return { value: Some(e), .. } => {
            collect_maybe_mutated_expr(e, sig_table, imported, generic_fn_table, out);
        }
        Stmt::If { cond, body, .. } => {
            collect_maybe_mutated_expr(cond, sig_table, imported, generic_fn_table, out);
            collect_maybe_mutated(body, sig_table, imported, generic_fn_table, out);
        }
        Stmt::Match { scrutinee, arms, else_arm, .. } => {
            collect_maybe_mutated_expr(scrutinee, sig_table, imported, generic_fn_table, out);
            for arm in arms {
                collect_maybe_mutated(&arm.body, sig_table, imported, generic_fn_table, out);
            }
            if let Some(eb) = else_arm {
                collect_maybe_mutated(eb, sig_table, imported, generic_fn_table, out);
            }
        }
        Stmt::While { cond, body, .. } => {
            collect_maybe_mutated_expr(cond, sig_table, imported, generic_fn_table, out);
            collect_maybe_mutated(body, sig_table, imported, generic_fn_table, out);
        }
        Stmt::For { iter, body, .. } => {
            collect_maybe_mutated_expr(iter, sig_table, imported, generic_fn_table, out);
            collect_maybe_mutated(body, sig_table, imported, generic_fn_table, out);
        }
        _ => {}
    }
}

fn collect_maybe_mutated_expr(
    expr: &Expr,
    sig_table: &SignatureTable,
    imported: &std::collections::HashMap<String, FunctionSig>,
    generic_fn_table: &GenericFnTable,
    out: &mut HashSet<String>,
) {
    match expr {
        Expr::Call(c) => {
            // Resolve the callee's ownership modifiers so we only mark args mutated when
            // the matched parameter is `lend` or `give`.  `share` params (non-mutating
            // read access) do not count — they let the `let→const` hint fire.
            //
            // Lookup order: user sig_table → imported → generic_fn_table → builtin free-fns
            // (print, range, sleepMs, sensitive, and any registry free-fns).  Builtin
            // free-fns are all read-only (share) in v0.1/v0.2 — none mutate their args.
            //
            // Conservative fallback: if the callee cannot be resolved through any path
            // above (genuinely unknown user-defined function), ALL ident args are marked
            // mutated.  This prevents a wrong "effectively const" hint when the actual
            // ownership is unknown.  Tradeoff: imported-function args that are truly
            // read-only also get suppressed — a missed hint is safer than a wrong one.
            // Narrows when cross-file signature data becomes available in the registry.
            let resolved_ownerships: Option<&Vec<Option<OwnershipModifier>>> =
                if let Expr::Ident(name, _) = &c.callee {
                    sig_table
                        .fns
                        .get(name.as_str())
                        .map(|s| &s.param_ownerships)
                        .or_else(|| imported.get(name.as_str()).map(|s| &s.param_ownerships))
                        .or_else(|| {
                            generic_fn_table
                                .fns
                                .get(name.as_str())
                                .map(|s| &s.param_ownerships)
                        })
                } else {
                    None
                };

            // Determine if this is a known-readonly builtin (all args are share).
            let callee_is_builtin_readonly = if let Expr::Ident(name, _) = &c.callee {
                builtin_free_fn_is_readonly(name.as_str())
            } else {
                false
            };

            for (i, arg) in c.args.iter().enumerate() {
                let is_mutating = if callee_is_builtin_readonly {
                    // Builtin free-fn — all parameters are share; never marks args mutated.
                    false
                } else {
                    match &resolved_ownerships {
                        Some(param_ownerships) => {
                            // Resolved user callee: only `lend`/`give` modifiers count as mutations.
                            match param_ownerships.get(i) {
                                Some(Some(OwnershipModifier::Lend | OwnershipModifier::Give)) => true,
                                // `share`, `None` (implicit share), or index out of range → not mutating.
                                _ => false,
                            }
                        }
                        // Unresolvable callee → conservative: mark every ident arg mutated.
                        None => true,
                    }
                };

                if is_mutating {
                    if let Expr::Ident(name, _) = arg {
                        out.insert(name.clone());
                    }
                }
                collect_maybe_mutated_expr(arg, sig_table, imported, generic_fn_table, out);
            }
            collect_maybe_mutated_expr(&c.callee, sig_table, imported, generic_fn_table, out);
        }
        Expr::MethodCall { receiver, method, args, .. } => {
            // Resolve the method via the same sig lookup as free-function calls (UFCS —
            // `player.heal(20)` desugars to `heal(player, 20)`; the receiver is param 0).
            // Lookup order: user sig_table → imported → generic_fn_table → primitive
            // intrinsic table.  All scalar primitive intrinsic methods (toString, toFloat,
            // byteAt, contains, etc.) are read-only (share receiver) by definition.
            // If the method still can't be resolved, conservatively mark the receiver mutated.
            let ownerships: Option<&Vec<Option<OwnershipModifier>>> = {
                sig_table
                    .fns
                    .get(method.as_str())
                    .map(|s| &s.param_ownerships)
                    .or_else(|| imported.get(method.as_str()).map(|s| &s.param_ownerships))
                    .or_else(|| {
                        generic_fn_table
                            .fns
                            .get(method.as_str())
                            .map(|s| &s.param_ownerships)
                    })
            };

            // Receiver corresponds to param 0 in the resolved signature.
            let receiver_is_mutating = if primitive_intrinsic_method_is_readonly(method.as_str()) {
                // Scalar intrinsic method — receiver is always share (read-only).
                false
            } else {
                match &ownerships {
                    Some(param_ownerships) => {
                        matches!(
                            param_ownerships.first(),
                            Some(Some(OwnershipModifier::Lend | OwnershipModifier::Give))
                        )
                    }
                    // Unresolvable user-defined method → conservative: assume the receiver is mutated.
                    None => true,
                }
            };
            if receiver_is_mutating {
                // Follow the full receiver chain to the root Ident so that
                // `a.b.heal(5)` marks `a` (not the intermediate `a.b`).
                if let Some(name) = root_ident(receiver.as_ref()) {
                    out.insert(name.to_string());
                }
            }
            collect_maybe_mutated_expr(receiver, sig_table, imported, generic_fn_table, out);

            // Additional args correspond to params 1.. in the resolved signature.
            for (i, arg) in args.iter().enumerate() {
                let arg_is_mutating = match &ownerships {
                    Some(param_ownerships) => {
                        matches!(
                            param_ownerships.get(i + 1),
                            Some(Some(OwnershipModifier::Lend | OwnershipModifier::Give))
                        )
                    }
                    None => true,
                };
                if arg_is_mutating {
                    if let Expr::Ident(name, _) = arg {
                        out.insert(name.clone());
                    }
                }
                collect_maybe_mutated_expr(arg, sig_table, imported, generic_fn_table, out);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_maybe_mutated_expr(lhs, sig_table, imported, generic_fn_table, out);
            collect_maybe_mutated_expr(rhs, sig_table, imported, generic_fn_table, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_maybe_mutated_expr(operand, sig_table, imported, generic_fn_table, out);
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_maybe_mutated_expr(receiver, sig_table, imported, generic_fn_table, out);
        }
        Expr::IndexAccess { receiver, index, .. } => {
            collect_maybe_mutated_expr(receiver, sig_table, imported, generic_fn_table, out);
            collect_maybe_mutated_expr(index, sig_table, imported, generic_fn_table, out);
        }
        // Recurse into compound literal expressions so mutations nested inside
        // struct/array/map literals and postfix operations are tracked.
        Expr::StructLit { fields, .. } => {
            for field in fields {
                collect_maybe_mutated_expr(
                    &field.value,
                    sig_table,
                    imported,
                    generic_fn_table,
                    out,
                );
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for element in elements {
                collect_maybe_mutated_expr(element, sig_table, imported, generic_fn_table, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (key_expr, val_expr) in entries {
                collect_maybe_mutated_expr(key_expr, sig_table, imported, generic_fn_table, out);
                collect_maybe_mutated_expr(val_expr, sig_table, imported, generic_fn_table, out);
            }
        }
        Expr::PostfixOp { receiver: operand, .. } => {
            collect_maybe_mutated_expr(operand, sig_table, imported, generic_fn_table, out);
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
/// A binding is considered mutated only when it appears in an assignment target, a
/// method-call receiver, or an argument to a `lend`/`give` parameter.  `share` params
/// do not count.  Unresolvable callees are treated conservatively (mark mutated).
///
/// Time: O(n × sig-lookup).  Space: O(hints).
#[salsa::tracked]
pub fn array_to_fixed_promotion_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<PromotionHint> {
    let parse = parse_query(db, source);
    let sigs = module_signatures_query(db, source);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            let mut mutated = HashSet::new();
            collect_maybe_mutated(
                &f.body,
                &sigs.sig_table,
                &sigs.imported_fns,
                &sigs.generic_fn_table,
                &mut mutated,
            );
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
/// A binding is considered mutated only when it appears in an assignment target, a
/// method-call receiver, or an argument to a `lend`/`give` parameter.  `share` params
/// do not count — passing a binding to a non-mutating function is consistent with
/// const semantics.  Unresolvable callees are treated conservatively (mark mutated).
///
/// `const` bindings are excluded (they already are const).
///
/// Time: O(n × sig-lookup).  Space: O(hints).
#[salsa::tracked]
pub fn let_to_const_promotion_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<PromotionHint> {
    let parse = parse_query(db, source);
    let sigs = module_signatures_query(db, source);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            let mut mutated = HashSet::new();
            collect_maybe_mutated(
                &f.body,
                &sigs.sig_table,
                &sigs.imported_fns,
                &sigs.generic_fn_table,
                &mut mutated,
            );
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
