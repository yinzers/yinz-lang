//! Inlay-hint detection passes for the v0.2-M5 + v0.3-M3b LSP teaching surfaces.
//!
//! Each pass is a salsa-tracked function so repeated LSP requests at the same
//! file version reuse the computed hints.  Cache is invalidated when source changes.
//!
//! # Firing domains (7 of 9)
//!
//! - `variable_type_hints`            — `: TypeName` after un-annotated `let` bindings
//! - `ownership_call_site_hints`      — `share`/`lend`/`give` after call arguments
//! - `copy_point_hints`               — `.copy (N bytes)` for trivially-copyable passes
//! - `array_to_fixed_promotion_hints` — decoration on never-grown `array<T>` bindings
//! - `let_to_const_promotion_hints`   — decoration on never-mutated `let` bindings
//! - `wait_points_hints`              — muted `wait` before suspending call sites (Addition)
//! - `background_routing_hints`       — routing comment at `background` spawn sites (Informational)
//!
//! # Protocol-only domains (2 of 9)
//!
//! `function_param_type` and `lifetimes` are handled by the LSP layer but return empty
//! hint lists.  `allocators` is also registered in the registry but not yet firing —
//! it ships when arena allocation lands (v0.2+).  `lifetimes` remain protocol-only.

use std::collections::HashSet;
use std::sync::OnceLock;

use ynz_ast::nodes::{Block, Expr, Item, OwnershipModifier, Stmt};
use ynz_diagnostics::SourceSpan;
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    generics::GenericFnTable,
    intrinsics::PrimitiveIntrinsicTable,
    queries::{check_query, module_signatures_query},
    signatures::{build_effective_suspend_set, FunctionSig, SignatureTable},
    types::{is_trivially_copyable, type_name, Type},
};

// ─────────────────────────────────────────────────────────────────────────────
// Builtin callee ownership helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` when `name` is a builtin free-function whose every parameter
/// is read-only (share).
///
/// All v0.1/v0.2 builtin free-fns are read-only by contract — they inspect
/// their arguments and never take ownership or mutate through a borrow.
/// `range` and `sleepBlocking` are in `registry/features.toml` as `kind = "free_fn"`;
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
    // Registry free-fn builtins (range overloads, sleepBlocking, and any future additions).
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
    /// For `ArrayToFixed` hints: byte range of the `array` keyword in the type
    /// annotation (e.g. the `array` in `array<int>`), used to build the
    /// click-to-make-explicit TextEdit that rewrites `array` → `fixed`.
    ///
    /// `None` is the graceful no-edit path (the hint renders as a decoration but
    /// has nothing to click-replace). Currently `None` is unreachable for
    /// `ArrayToFixed`: the hint only fires on `array<T>`-annotated bindings, whose
    /// `Type::Generic` always carries a `name_span`. The Option is kept defensive
    /// for a future inferred-array promotion (`let nums = [1,2,3]` with no
    /// annotation), which would have no `array` keyword to locate.
    /// Always `None` for `LetToConst` hints (they use the `let`-keyword span
    /// stored in `position` instead — see `let_to_const_edit` in `inlay_hint.rs`).
    pub type_keyword_span: Option<SourceSpan>,
}

/// Suspension-point hint at a call site whose callee transitively `suspends`.
///
/// Rendered as muted `wait` BEFORE the call (Addition placement per
/// `.claude/rules/inference.md`).  Suppressed when the user already wrote `wait`.
#[derive(Clone, Debug, PartialEq)]
pub struct WaitPointHint {
    /// Byte offset just before the call expression — where the muted `wait` renders.
    pub position: usize,
    /// The callee name, used to produce the contextual WHY: e.g. `"sleep"`.
    pub callee_name: String,
}

/// Thread-pool routing hint at a `background` spawn site.
///
/// Rendered as a muted comment AFTER the `background` statement (Informational
/// placement per `.claude/rules/inference.md`).  Reads `suspends_set` — the same
/// SSOT that drives the codegen routing decision at `emit.rs:9335` — so the hint
/// and the actual binary routing always agree.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundRoutingHint {
    /// Byte offset at the end of the `background` expression — where the routing
    /// comment renders.
    pub position: usize,
    /// The rendered muted comment label, e.g.
    /// `"// routed to I/O pool — sleep suspends here"` or
    /// `"// routed to CPU pool — no may-block calls in call graph"`.
    pub label: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

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

/// Compute the byte offset where a Replacement-category promotion hint should be
/// placed for a `let` statement — just before any trailing `//` line comment, or
/// at end-of-line when no trailing comment exists.
///
/// Scans the source line that contains `stmt_start` (the `let` keyword offset),
/// tracking whether each byte is inside a backtick string (`` ` ``) or a
/// double-quoted string (`"`).  The first `//` that falls outside all open
/// string literals is the trailing-comment boundary; the hint is placed at that
/// offset.  If no such `//` exists before end-of-line, returns `line_end`.
///
/// A `//` that appears INSIDE a string literal is not a comment — it must not
/// affect hint placement.  This guards the common case of URL literals
/// (`"http://..."`, `` `https://...` ``) appearing as initializers.
///
/// # Arguments
/// - `text`: full source file text.
/// - `stmt_start`: byte offset of the `let` keyword (`span.start`), used to locate the source line.
///
/// Single-line constraint: this scans only the line that `stmt_start` sits on. For a
/// `let` whose initializer spans multiple lines (e.g. a multi-line array literal), the
/// returned position is the end of the FIRST line, not the true end of the statement —
/// so the Replacement decoration anchors mid-expression for such bindings.
/// Single-line-only by design: multi-line initializers on promotable (never-grown /
/// never-reassigned) bindings are rare, single-line is correct, and there is no crash.
/// Locating the true statement end requires reliable end-of-statement detection across
/// lines — `span.end` points past trailing trivia (comments) rather than the closing
/// token, so it can't be used directly. Narrow the scan to the full statement when that
/// trivia handling is sorted, or when a user reports the multi-line mis-anchor.
///
/// Time: O(line_len)  Space: O(1)  where line_len = length of the containing line.
fn hint_position_end_of_stmt_or_before_comment(text: &str, stmt_start: usize) -> usize {
    // Locate the start of the line containing the `let` keyword.
    let line_start = text[..stmt_start].rfind('\n').map_or(0, |p| p + 1);
    // Locate end-of-line (exclusive, stopping just before the '\n').
    let line_end = text[line_start..]
        .find('\n')
        .map_or(text.len(), |p| line_start + p);

    // Scan the full line to determine string-literal state at each byte.
    // Yinz uses backtick strings (`...`) and double-quoted strings ("...").
    // Escape sequences (`\`` or `\"`) prevent a delimiter from closing the literal.
    let line = &text[line_start..line_end];
    let line_byte_base = line_start;

    let mut in_backtick = false;
    let mut in_dquote = false;
    let mut i = 0;
    while i < line.len() {
        let b = line.as_bytes()[i];
        match b {
            b'\\' if in_backtick || in_dquote => {
                // Skip the escaped character — it cannot be a string delimiter.
                i += 2;
                continue;
            }
            b'`' if !in_dquote => {
                in_backtick = !in_backtick;
            }
            b'"' if !in_backtick => {
                in_dquote = !in_dquote;
            }
            // Check for `//` outside string literals — a real line comment.
            b'/' if !in_backtick && !in_dquote && line.as_bytes().get(i + 1) == Some(&b'/') => {
                // Found an unquoted `//` on this line — place the hint here.
                return line_byte_base + i;
            }
            _ => {}
        }
        i += 1;
    }
    // No trailing comment found — hint goes at end-of-line.
    line_end
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
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
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
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
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
            // (print, range, sleepBlocking, sensitive, and any registry free-fns).  Builtin
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
                                Some(Some(OwnershipModifier::Lend | OwnershipModifier::Give)) => {
                                    true
                                }
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
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
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
        Expr::IndexAccess {
            receiver, index, ..
        } => {
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
        Expr::PostfixOp {
            receiver: operand, ..
        } => {
            collect_maybe_mutated_expr(operand, sig_table, imported, generic_fn_table, out);
        }
        // Recurse into concurrency wrappers and type-narrowing predicates so that
        // a `lend`/`give` call nested inside `wait`, `background`, or an `is`
        // scrutinee is not invisible to the mutation collector.
        // Mirrors the handling in `collect_referenced_names_in_expr` (check.rs).
        Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            collect_maybe_mutated_expr(inner, sig_table, imported, generic_fn_table, out);
        }
        Expr::Is { expr, .. } => {
            collect_maybe_mutated_expr(expr, sig_table, imported, generic_fn_table, out);
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_maybe_mutated_expr(e, sig_table, imported, generic_fn_table, out);
                }
            }
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
            Stmt::Let {
                ty: None,
                name_span,
                value,
                ..
            } => {
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

/// Emit `share`/`lend`/`give`/`copy` hints after call-site arguments.
///
/// Fires for free-function calls, generic-function calls, and UFCS method calls
/// (`player.heal(20)` is equivalent to `heal(player, 20)` — both get the same hint).
/// Also emits inferred `give` or `copy` hints for plain-ident arguments at
/// `background` call sites (sourced from the check pass's use-after-spawn analysis).
/// Suppressed when the callee cannot be resolved (unresolvable → no hint, not a crash).
///
/// Time: O(n × signature-lookup).  Space: O(hints).
#[salsa::tracked]
pub fn ownership_call_site_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<OwnershipHint> {
    let parse = parse_query(db, source);
    let sigs = module_signatures_query(db, source);
    let check = check_query(db, source);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            collect_ownership_hints_block(
                &f.body,
                &sigs.sig_table,
                &sigs.imported_fns,
                &sigs.generic_fn_table,
                &mut hints,
            );
        }
    }

    // Emit inferred `give`/`copy` hints for plain-ident arguments at `background`
    // call sites.  The check pass records which ident-span maps to which inferred
    // modifier in `background_arg_inferred_ownership`.
    //
    // Walk every statement in every function body; when we find a
    // `Stmt::Expr(Expr::Background(Expr::Call(...)))`, check each plain-ident arg
    // against the map and emit a hint at `arg.span().end`.
    let bg_inferred = &check.typed_module.background_arg_inferred_ownership;
    if !bg_inferred.is_empty() {
        for item in &parse.module.items {
            if let Item::Function(f) = item {
                collect_background_ownership_hints_block(&f.body, bg_inferred, &mut hints);
            }
        }
    }

    hints
}

/// Walk a block collecting inferred `give`/`copy` hints for `background` call sites.
fn collect_background_ownership_hints_block(
    block: &ynz_ast::nodes::Block,
    bg_inferred: &std::collections::HashMap<(usize, usize), crate::check::BgOwnership>,
    out: &mut Vec<OwnershipHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(Expr::Background(inner, _)) => {
                if let Expr::Call(call) = inner.as_ref() {
                    for arg in &call.args {
                        if let Expr::Ident(_, span) = arg {
                            let key = (span.start, span.end);
                            if let Some(own) = bg_inferred.get(&key) {
                                out.push(OwnershipHint {
                                    position: span.end,
                                    modifier: bg_ownership_modifier_str(own).to_string(),
                                });
                            }
                        }
                    }
                }
            }
            // Recurse into blocks so nested `background` statements are also covered.
            Stmt::If { body, .. } => {
                collect_background_ownership_hints_block(body, bg_inferred, out);
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_background_ownership_hints_block(body, bg_inferred, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_background_ownership_hints_block(&arm.body, bg_inferred, out);
                }
                if let Some(eb) = else_arm {
                    collect_background_ownership_hints_block(eb, bg_inferred, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_ownership_hints_block(
    block: &Block,
    sig_table: &crate::signatures::SignatureTable,
    imported: &std::collections::HashMap<String, crate::signatures::FunctionSig>,
    generic_fn_table: &GenericFnTable,
    out: &mut Vec<OwnershipHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(e) | Stmt::Let { value: e, .. } => {
                collect_ownership_hints_expr(e, sig_table, imported, generic_fn_table, out);
            }
            Stmt::Assign { value, .. } | Stmt::FieldAssign { value, .. } => {
                collect_ownership_hints_expr(value, sig_table, imported, generic_fn_table, out);
            }
            Stmt::IndexAssign { index, value, .. } => {
                collect_ownership_hints_expr(index, sig_table, imported, generic_fn_table, out);
                collect_ownership_hints_expr(value, sig_table, imported, generic_fn_table, out);
            }
            Stmt::If { cond, body, .. } => {
                collect_ownership_hints_expr(cond, sig_table, imported, generic_fn_table, out);
                collect_ownership_hints_block(body, sig_table, imported, generic_fn_table, out);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                collect_ownership_hints_expr(scrutinee, sig_table, imported, generic_fn_table, out);
                for arm in arms {
                    collect_ownership_hints_block(
                        &arm.body,
                        sig_table,
                        imported,
                        generic_fn_table,
                        out,
                    );
                }
                if let Some(eb) = else_arm {
                    collect_ownership_hints_block(eb, sig_table, imported, generic_fn_table, out);
                }
            }
            Stmt::While { cond, body, .. } => {
                collect_ownership_hints_expr(cond, sig_table, imported, generic_fn_table, out);
                collect_ownership_hints_block(body, sig_table, imported, generic_fn_table, out);
            }
            Stmt::For { iter, body, .. } => {
                collect_ownership_hints_expr(iter, sig_table, imported, generic_fn_table, out);
                collect_ownership_hints_block(body, sig_table, imported, generic_fn_table, out);
            }
            _ => {}
        }
    }
}

/// Map an `OwnershipModifier` to the Yinz keyword string shown in the hint.
fn ownership_modifier_str(own: &OwnershipModifier) -> &'static str {
    match own {
        OwnershipModifier::Share => "share",
        OwnershipModifier::Lend => "lend",
        OwnershipModifier::Give => "give",
    }
}

/// Map a `BgOwnership` (inferred modifier for `background` args) to the hint string.
fn bg_ownership_modifier_str(own: &crate::check::BgOwnership) -> &'static str {
    match own {
        crate::check::BgOwnership::Give => "give",
        crate::check::BgOwnership::Copy => "copy",
    }
}

/// Resolve a callee name to its `param_ownerships` vector.
///
/// Lookup order: user sig_table → imported → generic_fn_table.
/// Returns `None` when the name cannot be resolved through any path — callers
/// treat this as "no hint" rather than an error (graceful, no panic).
///
/// Mirrors the lookup order in `collect_maybe_mutated_expr` so both passes stay
/// consistent about which callees are "known" to the file.
fn resolve_param_ownerships<'a>(
    name: &str,
    sig_table: &'a crate::signatures::SignatureTable,
    imported: &'a std::collections::HashMap<String, crate::signatures::FunctionSig>,
    generic_fn_table: &'a GenericFnTable,
) -> Option<&'a Vec<Option<OwnershipModifier>>> {
    sig_table
        .fns
        .get(name)
        .map(|s| &s.param_ownerships)
        .or_else(|| imported.get(name).map(|s| &s.param_ownerships))
        .or_else(|| generic_fn_table.fns.get(name).map(|s| &s.param_ownerships))
}

fn collect_ownership_hints_expr(
    expr: &Expr,
    sig_table: &crate::signatures::SignatureTable,
    imported: &std::collections::HashMap<String, crate::signatures::FunctionSig>,
    generic_fn_table: &GenericFnTable,
    out: &mut Vec<OwnershipHint>,
) {
    match expr {
        Expr::Call(c) => {
            // Resolve the callee's parameter ownership list via the shared helper.
            //
            // Lookup order: user sig_table → imported → generic_fn_table.
            // Builtin free-fns (print, range, sleepBlocking, sensitive) are not in any of these
            // tables and get no ownership hint — correct, because their params carry no
            // explicit Yinz-source ownership modifier.
            //
            // Graceful fallback: unresolvable callee → recurse into args without hints.
            if let Expr::Ident(name, _) = &c.callee {
                if let Some(ownerships) =
                    resolve_param_ownerships(name, sig_table, imported, generic_fn_table)
                {
                    for (i, arg) in c.args.iter().enumerate() {
                        if let Some(Some(own)) = ownerships.get(i) {
                            out.push(OwnershipHint {
                                position: arg.span().end,
                                modifier: ownership_modifier_str(own).to_string(),
                            });
                        }
                        collect_ownership_hints_expr(
                            arg,
                            sig_table,
                            imported,
                            generic_fn_table,
                            out,
                        );
                    }
                    return;
                }
            }
            // Fallback: recurse into args without emitting hints (unresolvable callee).
            for arg in &c.args {
                collect_ownership_hints_expr(arg, sig_table, imported, generic_fn_table, out);
            }
            collect_ownership_hints_expr(&c.callee, sig_table, imported, generic_fn_table, out);
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            // UFCS: `player.heal(20)` desugars to `heal(player, 20)`.
            //
            // Look up the method name with the same resolver as free-fn calls — receiver is
            // param 0, additional args are params 1..  Scalar primitive intrinsic methods
            // (toString, toFloat, etc.) are not in sig_table/imported/generic_fn_table, so
            // they fall through to the unresolvable branch (no hint — correct, they carry no
            // explicit ownership modifier).
            //
            // Graceful fallback: unresolvable method → recurse without hints (no panic).
            if let Some(ownerships) =
                resolve_param_ownerships(method, sig_table, imported, generic_fn_table)
            {
                // Receiver → param 0.
                if let Some(Some(own)) = ownerships.first() {
                    out.push(OwnershipHint {
                        position: receiver.span().end,
                        modifier: ownership_modifier_str(own).to_string(),
                    });
                }
                collect_ownership_hints_expr(receiver, sig_table, imported, generic_fn_table, out);
                // Additional args → params 1..
                for (i, arg) in args.iter().enumerate() {
                    if let Some(Some(own)) = ownerships.get(i + 1) {
                        out.push(OwnershipHint {
                            position: arg.span().end,
                            modifier: ownership_modifier_str(own).to_string(),
                        });
                    }
                    collect_ownership_hints_expr(arg, sig_table, imported, generic_fn_table, out);
                }
            } else {
                // Unresolvable method — recurse without hints.
                collect_ownership_hints_expr(receiver, sig_table, imported, generic_fn_table, out);
                for arg in args {
                    collect_ownership_hints_expr(arg, sig_table, imported, generic_fn_table, out);
                }
            }
        }
        // Recurse into concurrency wrappers, type-narrowing predicates, and
        // interpolated strings so that call sites nested inside these wrappers
        // are not invisible to the ownership-hint pass.
        // Mirrors `collect_maybe_mutated_expr` and `collect_referenced_names_in_expr`.
        Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            collect_ownership_hints_expr(inner, sig_table, imported, generic_fn_table, out);
        }
        Expr::Is { expr, .. } => {
            collect_ownership_hints_expr(expr, sig_table, imported, generic_fn_table, out);
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_ownership_hints_expr(e, sig_table, imported, generic_fn_table, out);
                }
            }
        }
        _ => {}
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
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
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
    match expr {
        Expr::Call(c) => {
            for arg in &c.args {
                // Emit a copy hint when this arg's type is trivially copyable.  The
                // type-check runs before the recursion, so a plain `Expr::Ident` arg
                // that is copyable gets exactly one hint here; the recursion below
                // finds no nested `Expr::Call` inside an ident and emits nothing.
                if let Some(ty) = expr_types.get(&expr_span_key(arg)) {
                    if is_trivially_copyable(ty) {
                        out.push(CopyHint {
                            position: arg.span().end,
                            size_text: copy_size_text(ty).to_string(),
                        });
                    }
                }
                // Recurse into the arg so that copyable values nested inside inner
                // calls (e.g. `outer(inner(n))` → `n`) are reached at any depth.
                // Mirrors `collect_ownership_hints_expr`'s recursion shape.
                collect_copy_hints_expr(arg, expr_types, out);
            }
        }
        // Recurse into concurrency wrappers, type-narrowing predicates, and
        // interpolated strings so that call sites nested inside these wrappers
        // produce copy hints.  Mirrors `collect_maybe_mutated_expr` and
        // `collect_ownership_hints_expr` for sibling-walker consistency.
        Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            collect_copy_hints_expr(inner, expr_types, out);
        }
        Expr::Is { expr: inner, .. } => {
            collect_copy_hints_expr(inner, expr_types, out);
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_copy_hints_expr(e, expr_types, out);
                }
            }
        }
        _ => {}
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

    let text = source.text(db);
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
            collect_array_hints_block(&f.body, &mutated, &text, &mut hints);
        }
    }
    hints
}

fn collect_array_hints_block(
    block: &Block,
    mutated: &HashSet<String>,
    text: &str,
    out: &mut Vec<PromotionHint>,
) {
    for stmt in &block.stmts {
        if let Stmt::Let {
            ty: Some(ty_ann),
            name,
            span,
            ..
        } = stmt
        {
            if ast_ty_is_array(ty_ann) && !mutated.contains(name) {
                // Extract the span of `array` keyword from the type annotation.
                // `Type::Generic { name_span, .. }` carries the span of the type
                // constructor name (e.g. `array` in `array<int>`), which is exactly
                // the byte range the TextEdit must replace with `fixed`.
                let type_keyword_span =
                    if let ynz_ast::nodes::Type::Generic { name_span, .. } = ty_ann {
                        Some(name_span.clone())
                    } else {
                        None
                    };
                out.push(PromotionHint {
                    position: hint_position_end_of_stmt_or_before_comment(text, span.start),
                    kind: PromotionKind::ArrayToFixed,
                    label: "// promoted to fixed — never grown".to_string(),
                    type_keyword_span,
                });
            }
        }
        match stmt {
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_array_hints_block(body, mutated, text, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_array_hints_block(&arm.body, mutated, text, out);
                }
                if let Some(eb) = else_arm {
                    collect_array_hints_block(eb, mutated, text, out);
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

    let text = source.text(db);
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
            collect_const_hints_block(&f.body, &mutated, &text, &mut hints);
        }
    }
    hints
}

fn collect_const_hints_block(
    block: &Block,
    mutated: &HashSet<String>,
    text: &str,
    out: &mut Vec<PromotionHint>,
) {
    for stmt in &block.stmts {
        if let Stmt::Let {
            is_const: false,
            name,
            span,
            ..
        } = stmt
        {
            if !mutated.contains(name) {
                out.push(PromotionHint {
                    position: hint_position_end_of_stmt_or_before_comment(text, span.start),
                    kind: PromotionKind::LetToConst,
                    label: "// effectively const — never reassigned".to_string(),
                    // LetToConst uses the `let`-keyword span stored in `position`
                    // via `let_to_const_edit` in `inlay_hint.rs`; no type annotation involved.
                    type_keyword_span: None,
                });
            }
        }
        match stmt {
            Stmt::If { body, .. } | Stmt::While { body, .. } | Stmt::For { body, .. } => {
                collect_const_hints_block(body, mutated, text, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_const_hints_block(&arm.body, mutated, text, out);
                }
                if let Some(eb) = else_arm {
                    collect_const_hints_block(eb, mutated, text, out);
                }
            }
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// wait_points pass (Addition placement)
// ─────────────────────────────────────────────────────────────────────────────

/// Emit a muted `wait` hint before each call site whose callee transitively suspends.
///
/// Suppressed at call sites where the user already wrote `wait` — the hint is
/// informational, not redundant.  Per `.claude/rules/inference.md`, this is an
/// Addition-category hint: the muted text appears in-position before the call,
/// and click-to-make-explicit inserts `wait ` before the expression.
///
/// Uses the EFFECTIVE suspend set (local-suspending names PLUS imported-suspending
/// names from `module_signatures_query`), mirroring exactly what codegen builds at
/// `crates/ynz-codegen/src/queries.rs:90-94`.  Without this, a call to an imported
/// suspending function is correctly routed by codegen but the hint never fires —
/// the hint and the binary diverge for cross-module callee sites.
///
/// Time: O(n) AST walk.  Space: O(hints).
#[salsa::tracked]
pub fn wait_points_hints(db: &dyn SourceFileRegistry, source: SourceFile) -> Vec<WaitPointHint> {
    let parse = parse_query(db, source);
    let check = check_query(db, source);
    let sig_output = module_signatures_query(db, source);
    // WHY: single SSOT — `build_effective_suspend_set` is the same computation that
    // feeds codegen frame-layout and routing, so the hint can never drift from the
    // binary's actual suspension decisions.
    let effective_suspends =
        build_effective_suspend_set(&check.suspends_set, &sig_output.imported_fns);

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            collect_wait_point_hints_block(&f.body, &effective_suspends, &mut hints);
        }
    }
    hints
}

/// Walk a block collecting `WaitPointHint`s for suspending call sites.
///
/// `inside_wait` is `true` when the current expression is the inner expr of an
/// `Expr::Wait` wrapper — those sites already have an explicit `wait` and are
/// suppressed.
fn collect_wait_point_hints_block(
    block: &Block,
    suspends_set: &HashSet<String>,
    out: &mut Vec<WaitPointHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(e) | Stmt::Let { value: e, .. } => {
                collect_wait_point_hints_expr(e, suspends_set, out);
            }
            Stmt::Assign { value, .. } | Stmt::FieldAssign { value, .. } => {
                collect_wait_point_hints_expr(value, suspends_set, out);
            }
            Stmt::IndexAssign { index, value, .. } => {
                collect_wait_point_hints_expr(index, suspends_set, out);
                collect_wait_point_hints_expr(value, suspends_set, out);
            }
            Stmt::Return { value: Some(e), .. } => {
                collect_wait_point_hints_expr(e, suspends_set, out);
            }
            Stmt::If { cond, body, .. } => {
                collect_wait_point_hints_expr(cond, suspends_set, out);
                collect_wait_point_hints_block(body, suspends_set, out);
            }
            Stmt::While { cond, body, .. } => {
                collect_wait_point_hints_expr(cond, suspends_set, out);
                collect_wait_point_hints_block(body, suspends_set, out);
            }
            Stmt::For { iter, body, .. } => {
                collect_wait_point_hints_expr(iter, suspends_set, out);
                collect_wait_point_hints_block(body, suspends_set, out);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                collect_wait_point_hints_expr(scrutinee, suspends_set, out);
                for arm in arms {
                    collect_wait_point_hints_block(&arm.body, suspends_set, out);
                }
                if let Some(eb) = else_arm {
                    collect_wait_point_hints_block(eb, suspends_set, out);
                }
            }
            _ => {}
        }
    }
}

/// Collect wait-point hints for an expression.  A call whose callee is in
/// `suspends_set` gets a `WaitPointHint` placed before the call's span start.
///
/// The match is exhaustive — every `Expr` variant is named explicitly.  Genuine
/// leaves (literals, identifiers, `self`, `none`) have empty arms.  This forces a
/// conscious decision whenever a new `Expr` variant is added to the AST instead of
/// silently dropping it into the `_ => {}` catch-all and missing suspension hints.
fn collect_wait_point_hints_expr(
    expr: &Expr,
    suspends_set: &HashSet<String>,
    out: &mut Vec<WaitPointHint>,
) {
    match expr {
        Expr::Call(c) => {
            // Emit a hint if the callee is a known-suspending user-defined function.
            if let Expr::Ident(name, _) = &c.callee {
                if suspends_set.contains(name.as_str()) {
                    out.push(WaitPointHint {
                        position: expr.span().start,
                        callee_name: name.clone(),
                    });
                }
            }
            // Recurse into args and callee for nested suspending calls.
            for arg in &c.args {
                collect_wait_point_hints_expr(arg, suspends_set, out);
            }
            collect_wait_point_hints_expr(&c.callee, suspends_set, out);
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_wait_point_hints_expr(receiver, suspends_set, out);
            for arg in args {
                collect_wait_point_hints_expr(arg, suspends_set, out);
            }
        }
        // A user-written `wait foo()` — skip the top-level hint; recurse into
        // nested calls inside the inner expression (args to the waited call still
        // get their own hints if they also suspend).
        Expr::Wait(inner, _) => {
            collect_wait_point_hints_expr_no_top(inner, suspends_set, out);
        }
        // `background` spawn sites: recurse for any nested suspending calls in args.
        Expr::Background(inner, _) => {
            collect_wait_point_hints_expr(inner, suspends_set, out);
        }
        // Compound expressions — a suspending call buried inside any of these would
        // have been silently dropped before this fix.  Mirror the reference walker
        // `collect_maybe_mutated_expr` which handles all of these.
        Expr::BinOp { lhs, rhs, .. } => {
            collect_wait_point_hints_expr(lhs, suspends_set, out);
            collect_wait_point_hints_expr(rhs, suspends_set, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_wait_point_hints_expr(operand, suspends_set, out);
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_wait_point_hints_expr(receiver, suspends_set, out);
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_wait_point_hints_expr(receiver, suspends_set, out);
            collect_wait_point_hints_expr(index, suspends_set, out);
        }
        Expr::StructLit { fields, .. } => {
            for field in fields {
                collect_wait_point_hints_expr(&field.value, suspends_set, out);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for elem in elements {
                collect_wait_point_hints_expr(elem, suspends_set, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (key_expr, val_expr) in entries {
                collect_wait_point_hints_expr(key_expr, suspends_set, out);
                collect_wait_point_hints_expr(val_expr, suspends_set, out);
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_wait_point_hints_expr(receiver, suspends_set, out);
        }
        Expr::Is { expr: inner, .. } => {
            collect_wait_point_hints_expr(inner, suspends_set, out);
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_wait_point_hints_expr(e, suspends_set, out);
                }
            }
        }
        // Genuine leaves — no sub-expressions to recurse into.
        Expr::Ident(..)
        | Expr::StringLit(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => {}
    }
}

/// Walk an expression for nested suspending calls, but do NOT emit a hint for the
/// top-level call itself (because the parent is already an explicit `wait`).
///
/// Recurses into the call's arguments so a suspending call passed as an argument
/// to an already-waited call still gets its own hint.  Any non-Call top-level
/// expression delegates to the full walker — the suppression applies only to the
/// immediately-waited Call, not to arbitrary deeper expressions.
fn collect_wait_point_hints_expr_no_top(
    expr: &Expr,
    suspends_set: &HashSet<String>,
    out: &mut Vec<WaitPointHint>,
) {
    match expr {
        Expr::Call(c) => {
            // The top-level call is already `wait`'d — skip emitting a hint for it.
            // Its args are fair game: a suspending argument still needs its own hint.
            for arg in &c.args {
                collect_wait_point_hints_expr(arg, suspends_set, out);
            }
        }
        // Anything else falls through to the full walker — suppression is only for
        // the directly-waited Call node, not for every sub-expression underneath it.
        _ => collect_wait_point_hints_expr(expr, suspends_set, out),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// background_routing pass (Informational placement)
// ─────────────────────────────────────────────────────────────────────────────

/// Emit a muted routing comment at each `background` spawn site.
///
/// The comment says `// routed to I/O pool — <callee> suspends here` when the
/// callee is in the effective suspend set (state-machine, routes to
/// `ynz_rt_spawn`), or `// routed to CPU pool — no may-block calls in call
/// graph` otherwise (routes to `ynz_rt_spawn_blocking`).
///
/// Per `.claude/rules/inference.md`, this is an Informational-category hint: the
/// compiler made a decision with no typeable equivalent syntax, so the hint is a
/// muted comment annotation rather than an Addition/Replacement hint.
///
/// Uses the EFFECTIVE suspend set (local-suspending names PLUS imported-suspending
/// names from `module_signatures_query`), mirroring exactly what codegen builds at
/// `crates/ynz-codegen/src/queries.rs:90-94`.  Without this, `background
/// importedFn()` where `importedFn` suspends would show "CPU pool" in the hint
/// while the binary routes to the I/O pool — the hint lies and the binary contradicts
/// it (confirmed live, two-module test).
///
/// Time: O(n) AST walk.  Space: O(hints).
#[salsa::tracked]
pub fn background_routing_hints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<BackgroundRoutingHint> {
    let parse = parse_query(db, source);
    let check = check_query(db, source);
    let sig_output = module_signatures_query(db, source);
    // WHY: single SSOT — `build_effective_suspend_set` is the same computation that
    // feeds codegen frame-layout and routing, so the hint can never drift from the
    // binary's actual suspension decisions.
    let effective_suspends =
        build_effective_suspend_set(&check.suspends_set, &sig_output.imported_fns);
    let suspends_set = &effective_suspends;

    let mut hints = Vec::new();
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            collect_background_routing_hints_block(&f.body, suspends_set, &mut hints);
        }
    }
    hints
}

/// Walk a block collecting `BackgroundRoutingHint`s.
fn collect_background_routing_hints_block(
    block: &Block,
    suspends_set: &HashSet<String>,
    out: &mut Vec<BackgroundRoutingHint>,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(Expr::Background(inner, bg_span)) => {
                emit_background_routing_hint(inner, bg_span, suspends_set, out);
                // Recurse into the inner call's args for nested background/wait sites.
                collect_background_routing_hints_expr(inner, suspends_set, out);
            }
            Stmt::Expr(e) | Stmt::Let { value: e, .. } => {
                collect_background_routing_hints_expr(e, suspends_set, out);
            }
            Stmt::Assign { value, .. } | Stmt::FieldAssign { value, .. } => {
                collect_background_routing_hints_expr(value, suspends_set, out);
            }
            Stmt::IndexAssign { index, value, .. } => {
                collect_background_routing_hints_expr(index, suspends_set, out);
                collect_background_routing_hints_expr(value, suspends_set, out);
            }
            Stmt::Return { value: Some(e), .. } => {
                collect_background_routing_hints_expr(e, suspends_set, out);
            }
            Stmt::If { cond, body, .. } => {
                collect_background_routing_hints_expr(cond, suspends_set, out);
                collect_background_routing_hints_block(body, suspends_set, out);
            }
            Stmt::While { cond, body, .. } => {
                collect_background_routing_hints_expr(cond, suspends_set, out);
                collect_background_routing_hints_block(body, suspends_set, out);
            }
            Stmt::For { iter, body, .. } => {
                collect_background_routing_hints_expr(iter, suspends_set, out);
                collect_background_routing_hints_block(body, suspends_set, out);
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                collect_background_routing_hints_expr(scrutinee, suspends_set, out);
                for arm in arms {
                    collect_background_routing_hints_block(&arm.body, suspends_set, out);
                }
                if let Some(eb) = else_arm {
                    collect_background_routing_hints_block(eb, suspends_set, out);
                }
            }
            _ => {}
        }
    }
}

/// Emit a `BackgroundRoutingHint` for a `background inner` spawn site.
fn emit_background_routing_hint(
    inner: &Expr,
    bg_span: &SourceSpan,
    suspends_set: &HashSet<String>,
    out: &mut Vec<BackgroundRoutingHint>,
) {
    let callee_name = match inner {
        Expr::Call(c) => match &c.callee {
            Expr::Ident(name, _) => Some(name.as_str()),
            _ => None,
        },
        _ => None,
    };

    let label = if let Some(name) = callee_name {
        if suspends_set.contains(name) {
            format!("// routed to I/O pool — {} suspends here", name)
        } else {
            "// routed to CPU pool — no may-block calls in call graph".to_string()
        }
    } else {
        // Complex callee (non-ident, e.g. method call desugared) — emit CPU routing
        // as the conservative default (mirrors codegen's complex-callee branch).
        "// routed to CPU pool — no may-block calls in call graph".to_string()
    };

    out.push(BackgroundRoutingHint {
        position: bg_span.end,
        label,
    });
}

/// Recurse into an expression collecting `BackgroundRoutingHint`s for nested
/// `background` sites inside the expression.
///
/// The match is exhaustive — every `Expr` variant is named explicitly so that a
/// future AST variant forces a conscious decision instead of silently dropping
/// nested `background` spawns buried in compound expressions.
fn collect_background_routing_hints_expr(
    expr: &Expr,
    suspends_set: &HashSet<String>,
    out: &mut Vec<BackgroundRoutingHint>,
) {
    match expr {
        Expr::Background(inner, bg_span) => {
            emit_background_routing_hint(inner, bg_span, suspends_set, out);
            collect_background_routing_hints_expr(inner, suspends_set, out);
        }
        Expr::Wait(inner, _) => {
            collect_background_routing_hints_expr(inner, suspends_set, out);
        }
        Expr::Call(c) => {
            collect_background_routing_hints_expr(&c.callee, suspends_set, out);
            for arg in &c.args {
                collect_background_routing_hints_expr(arg, suspends_set, out);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_background_routing_hints_expr(receiver, suspends_set, out);
            for arg in args {
                collect_background_routing_hints_expr(arg, suspends_set, out);
            }
        }
        // Compound expressions — a nested `background` buried in any of these was
        // silently dropped before this fix.  Mirror the reference walker
        // `collect_maybe_mutated_expr` which handles all of these.
        Expr::BinOp { lhs, rhs, .. } => {
            collect_background_routing_hints_expr(lhs, suspends_set, out);
            collect_background_routing_hints_expr(rhs, suspends_set, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_background_routing_hints_expr(operand, suspends_set, out);
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_background_routing_hints_expr(receiver, suspends_set, out);
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_background_routing_hints_expr(receiver, suspends_set, out);
            collect_background_routing_hints_expr(index, suspends_set, out);
        }
        Expr::StructLit { fields, .. } => {
            for field in fields {
                collect_background_routing_hints_expr(&field.value, suspends_set, out);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for elem in elements {
                collect_background_routing_hints_expr(elem, suspends_set, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (key_expr, val_expr) in entries {
                collect_background_routing_hints_expr(key_expr, suspends_set, out);
                collect_background_routing_hints_expr(val_expr, suspends_set, out);
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_background_routing_hints_expr(receiver, suspends_set, out);
        }
        Expr::Is { expr: inner, .. } => {
            collect_background_routing_hints_expr(inner, suspends_set, out);
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_background_routing_hints_expr(e, suspends_set, out);
                }
            }
        }
        // Genuine leaves — no sub-expressions to recurse into.
        Expr::Ident(..)
        | Expr::StringLit(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => {}
    }
}
