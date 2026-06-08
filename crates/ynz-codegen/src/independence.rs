//! Dependency-graph independence analysis for auto-parallelization (v0.3-M3b Phase 4).
//!
//! # What this computes
//!
//! A maximal partition of a straight-line statement sequence into groups of mutually
//! independent suspending calls. Two statements are independent ONLY IF the analysis
//! can PROVE all of:
//!
//! 1. **No data dependency** — neither uses a binding the other defines.
//! 2. **No shared-write conflict** — neither writes a resource the other reads or writes.
//!    A call argument is a potential write iff its parameter TYPE is a mutable heap reference
//!    (`shape`/`array`/`map`/...): the conservative floor (see `write_positions_for_sig`).
//!    Trivially-copyable scalars and immutable strings are never write hazards. The analysis
//!    deliberately does NOT consult a per-form ownership classification to narrow this — that
//!    cannot be made sound without a full type+alias-aware borrow checker (see below).
//! 3. **Both are suspending calls** to DISTINCT callee names — same-callee concurrent
//!    calls are lowered sequentially because the composed frame allocates one sub-frame
//!    slot per unique callee name; two simultaneous invocations of the same callee would
//!    require two sub-frame slots, which is a separate future concern.
//!
//! # Conservative-correct
//!
//! When the analysis cannot PROVE independence (e.g., a callee has no known signature,
//! or the call involves method-call syntax rather than a direct ident call), the
//! statements are placed in separate singleton groups — the existing sequential path
//! handles them.
//!
//! # Scope boundary
//!
//! `partition_independent_groups` scans ONE straight-line statement list. A nested compound
//! statement (`if`/`match`/`for`/`while`) is one OPAQUE unit in that scan — this function never
//! flattens a nested block's statements into the current sequence, so a parallel group never
//! spans a control-flow boundary. emit.rs (`lower_sm_block`/`lower_sm_for`) recurses into each
//! nested straight-line block and re-runs this partition there, so parallel groups CAN appear
//! inside an `if`/`match` branch body or a single loop-iteration body — they just never merge
//! across the boundary. Two hard limits hold regardless:
//! - Loop ITERATIONS stay sequential — the loop structure runs one iteration at a time
//!   (`design/concurrency.md` "Loop Iterations — Sequential by Default"); independence only
//!   ever overlaps statements WITHIN a single iteration body, never across iterations.
//! - An explicit `wait` barrier resets the group — a `wait`-prefixed call is a sequencing
//!   point; statements on opposite sides of it never share a parallel group.
//!
//! # Corpse guard (no-duct-tape.md + graveyard 2026-06-04)
//!
//! This module CONSUMES the authoritative producers:
//! - The mutable-heap test reuses `ynz_typeck::is_trivially_copyable` — never a local
//!   re-implementation of the same `matches!`.
//! - Crossing membership is NOT re-derived here; that is `crossing_local_names`'s job.
//!
//! # Soundness floor (why the write-effect is type-based, not classification-based)
//!
//! `design/scope.md` PROHIBITS mutable module-level/global state — file-level `let` is a
//! compile error; only `const` is allowed at file level. So every mutable resource is either
//! a local binding or a parameter visible at the call site; there is no hidden global state.
//!
//! Write-effect detection uses a CONSERVATIVE FLOOR: a call argument is a potential aliased
//! write iff its parameter TYPE is a mutable heap reference (`shape`/`array`/`map`/maybe/
//! union/dynamic). It does NOT try to classify per-call-site whether the callee actually
//! mutates the argument. A name-based AST classification that attempted that narrowing was
//! built and removed: across three gate rounds it missed five distinct write forms (`share
//! self` receivers, mutating methods, mutation through a field/index of the param, a builtin
//! mutator shadowed by a same-named user function, and a local `let`-alias of the argument),
//! each a silent miscompile. The root cause is structural — proving a heap argument is
//! read-only requires receiver types AND alias tracking, i.e. a full borrow checker, which
//! this AST pass is not. The floor sidesteps the entire class: treating every mutable-heap
//! argument as a potential write is sound regardless of how the callee uses it.
//!
//! Golden Rule 5 (compile-time soundness — never a silent runtime miscompile) outranks Rule
//! 10 (efficiency-first); when they conflict, soundness wins. Trivially-copyable scalars are
//! passed by copy and immutable strings cannot be mutated through a reference, so neither is
//! a write hazard — the I/O-overlap headline (no-arg, primitive-, and string-argument
//! suspending calls) still parallelizes.
//!
//! # Design divergences (documented costs)
//!
//! - **Same-callee concurrency**: two independent calls to the same suspending function
//!   are lowered sequentially. Cost: no parallelism when the same I/O function is called
//!   twice in a row. Fix in a future phase extending `build_frame_layouts` to allocate
//!   N sub-frame slots per callee name (one per concurrent invocation).
//!
//! - **Mutable-heap arguments never parallelize**: any suspending call taking a mutable-heap
//!   argument (`shape`/`array`/`map`/...) is sequenced against every other statement, even
//!   when the callee only reads it. Cost: forfeits the read-only-mutable-heap-argument
//!   overlap (e.g. two suspending reads of distinct shapes). Reversal path: a real
//!   type+alias-aware ownership analysis (M4 borrow-checker completion) that can PROVE a
//!   heap argument read-only AND non-aliased re-enables narrowing the floor. Documented in
//!   `design/concurrency.md` "Design Divergences".

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Expr, Stmt, StringPart};
use ynz_typeck::{is_trivially_copyable, signatures::FunctionSig, SignatureTable};

// ── Public types ──────────────────────────────────────────────────────────────

/// A group of statements to lower together.
///
/// `Singleton` groups lower via the existing sequential path.
/// `Parallel` groups contain ≥2 independent suspending calls that may be
/// interleaved in a single join poll.
#[derive(Debug)]
pub enum IndependentGroup<'a> {
    /// A single statement — lowered via the existing sequential path.
    Singleton(&'a Stmt),
    /// ≥2 independent suspending `Let` or `Expr` statements with distinct callee names.
    /// All members satisfy the independence criteria in this module.
    Parallel(Vec<&'a Stmt>),
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Partition `stmts` (a straight-line top-level statement list, NOT inside a loop) into
/// independence groups.
///
/// Returns a list of groups in source order. Each `Parallel` group contains ≥2 statements
/// that may be interleaved. Each `Singleton` contains one statement lowered sequentially.
///
/// `suspend_set` is the transitive suspending-function name set (from typeck).
/// `sig_table` is used for `params` (the parameter types, for the mutable-heap test).
/// `imported_fns` covers cross-module callees.
///
/// A call argument is a potential aliased write iff its parameter TYPE is a mutable heap
/// reference (`shape`/`array`/`map`/...) — the conservative floor in `write_positions_for_sig`.
/// This is sound by construction without consulting any per-form effective-ownership
/// classification (see that function's doc for why).
pub fn partition_independent_groups<'a>(
    stmts: &'a [Stmt],
    suspend_set: &HashSet<String>,
    sig_table: &SignatureTable,
    imported_fns: &HashMap<String, FunctionSig>,
) -> Vec<IndependentGroup<'a>> {
    let write_effects = build_write_effect_summary(sig_table, imported_fns);

    let mut groups: Vec<IndependentGroup<'a>> = Vec::new();
    let mut pending: Vec<&Stmt> = Vec::new();

    for stmt in stmts {
        // Explicit `wait` prefix resets the group — `wait` is a sequential ordering barrier.
        if stmt_has_explicit_wait(stmt) {
            flush_pending(&mut pending, &mut groups);
            groups.push(IndependentGroup::Singleton(stmt));
            continue;
        }

        // Only suspending DIRECT ident calls are candidates for parallelization.
        let Some(callee_name) = stmt_direct_suspending_callee(stmt, suspend_set) else {
            flush_pending(&mut pending, &mut groups);
            groups.push(IndependentGroup::Singleton(stmt));
            continue;
        };

        // Same-callee constraint: sequential (one sub-frame slot per unique callee name).
        let callee_already_in_group = pending.iter().any(|s| {
            stmt_direct_suspending_callee(s, suspend_set).as_deref() == Some(&callee_name)
        });
        if callee_already_in_group {
            flush_pending(&mut pending, &mut groups);
            pending.push(stmt);
            continue;
        }

        // Check independence against every pending group member.
        let independent_of_all = pending
            .iter()
            .all(|&prev| stmts_are_independent(prev, stmt, &write_effects));

        if independent_of_all {
            pending.push(stmt);
        } else {
            flush_pending(&mut pending, &mut groups);
            pending.push(stmt);
        }
    }

    flush_pending(&mut pending, &mut groups);
    groups
}

/// Emit the pending group as a `Parallel` (≥2 members) or `Singleton` (1 member).
fn flush_pending<'a>(pending: &mut Vec<&'a Stmt>, groups: &mut Vec<IndependentGroup<'a>>) {
    match pending.len() {
        0 => {}
        1 => {
            groups.push(IndependentGroup::Singleton(pending.remove(0)));
        }
        _ => {
            groups.push(IndependentGroup::Parallel(std::mem::take(pending)));
        }
    }
}

// ── Independence predicate ────────────────────────────────────────────────────

/// Returns `true` ONLY when the analysis can PROVE `a` and `b` are independent:
/// no data dependency AND no shared-write conflict.
///
/// Conservative: when in doubt, returns `false` (sequential, never silent-wrong).
fn stmts_are_independent(a: &Stmt, b: &Stmt, write_effects: &WriteEffectMap) -> bool {
    let defs_a = stmt_defines(a);
    let reads_a = stmt_reads(a);
    let defs_b = stmt_defines(b);
    let reads_b = stmt_reads(b);

    // Data dependency: a defines something b reads, or vice versa.
    if defs_a.iter().any(|d| reads_b.contains(d.as_str())) {
        return false;
    }
    if defs_b.iter().any(|d| reads_a.contains(d.as_str())) {
        return false;
    }
    // Define-define: both define the same name — conflict (shadowing would break the join).
    if defs_a.iter().any(|d| defs_b.contains(d)) {
        return false;
    }

    // Write-effect conflict: must not write to a resource the other reads or writes.
    let call_a = stmt_call_args(a);
    let call_b = stmt_call_args(b);
    if let (Some((callee_a, args_a)), Some((callee_b, args_b))) = (call_a, call_b) {
        let write_a = write_effects
            .get(callee_a)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let write_b = write_effects
            .get(callee_b)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        // Aliasing-soundness guard (conservative floor): a write-capable argument is any
        // argument whose TYPE is mutable-heap (`shape`/`array`/`map`/...), per
        // `write_positions_for_sig`. A mutable-heap argument can alias ANY other heap binding
        // (`let b = a` copies the pointer — heap references share the object), and the callee
        // may mutate through it in a way this AST pass cannot disprove without a full
        // type+alias-aware borrow checker. A concurrent read or write of an alias while the
        // callee mutates reads the post-mutation object, producing a wrong value. So we
        // sequence any pair where EITHER statement carries a mutable-heap argument. Only pairs
        // where NEITHER does — every argument a copied scalar or an immutable string — are
        // eligible for parallelization. See the module-level "Soundness floor" doc and the
        // unit tests in this module.
        let a_has_write_arg = args_a.iter().enumerate().any(|(i, _)| write_a.contains(&i));
        let b_has_write_arg = args_b.iter().enumerate().any(|(i, _)| write_b.contains(&i));
        if a_has_write_arg || b_has_write_arg {
            return false;
        }

        let writes_a = call_site_write_set(args_a, write_a);
        let writes_b = call_site_write_set(args_b, write_b);

        // Collect all names read or written by each call for the conflict check.
        let mut all_a: HashSet<String> = writes_a.clone();
        let mut arg_reads_a: HashSet<String> = HashSet::new();
        for (i, arg) in args_a.iter().enumerate() {
            if !write_a.contains(&i) {
                collect_ident_names(arg, &mut arg_reads_a);
            }
        }
        all_a.extend(arg_reads_a.iter().cloned());

        let mut all_b: HashSet<String> = writes_b.clone();
        let mut arg_reads_b: HashSet<String> = HashSet::new();
        for (i, arg) in args_b.iter().enumerate() {
            if !write_b.contains(&i) {
                collect_ident_names(arg, &mut arg_reads_b);
            }
        }
        all_b.extend(arg_reads_b.iter().cloned());

        // Write(A) ∩ (Read(B) ∪ Write(B)) or Write(B) ∩ (Read(A) ∪ Write(A)) — conflict.
        if writes_a.iter().any(|w| all_b.contains(w)) {
            return false;
        }
        if writes_b.iter().any(|w| all_a.contains(w)) {
            return false;
        }
    } else {
        // Cannot extract call args — conservative: not independent.
        return false;
    }

    true
}

// ── Define/Read sets ──────────────────────────────────────────────────────────

/// Collect the set of binding NAMES defined (introduced) by a statement at the top level.
fn stmt_defines(stmt: &Stmt) -> HashSet<String> {
    match stmt {
        Stmt::Let { name, .. } => {
            let mut s = HashSet::new();
            s.insert(name.clone());
            s
        }
        _ => HashSet::new(),
    }
}

/// Collect the set of identifier names READ by a statement's expression.
fn stmt_reads(stmt: &Stmt) -> HashSet<String> {
    let mut refs: HashSet<&str> = HashSet::new();
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_ident_refs(value, &mut refs)
        }
        Stmt::Expr(e) => collect_ident_refs(e, &mut refs),
        Stmt::Return { value: Some(v), .. } => collect_ident_refs(v, &mut refs),
        Stmt::Return { value: None, .. } => {}
        Stmt::FieldAssign { target, value, .. } => {
            collect_ident_refs(target, &mut refs);
            collect_ident_refs(value, &mut refs);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_ident_refs(receiver, &mut refs);
            collect_ident_refs(index, &mut refs);
            collect_ident_refs(value, &mut refs);
        }
        _ => {}
    }
    refs.into_iter().map(|s| s.to_string()).collect()
}

/// Collect ident name references from an expression (conservative — includes all reads).
fn collect_ident_refs<'a>(expr: &'a Expr, out: &mut HashSet<&'a str>) {
    match expr {
        Expr::Ident(name, _) => {
            out.insert(name.as_str());
        }
        Expr::Call(c) => {
            // Skip the callee ident itself (it's a function name, not a data binding).
            if !matches!(&c.callee, Expr::Ident(..)) {
                collect_ident_refs(&c.callee, out);
            }
            for arg in &c.args {
                collect_ident_refs(arg, out);
            }
        }
        Expr::Wait(inner, _) | Expr::Background(inner, _) => collect_ident_refs(inner, out),
        Expr::BinOp { lhs, rhs, .. } => {
            collect_ident_refs(lhs, out);
            collect_ident_refs(rhs, out);
        }
        Expr::UnaryOp { operand, .. } => collect_ident_refs(operand, out),
        Expr::FieldAccess { receiver, .. } => collect_ident_refs(receiver, out),
        Expr::MethodCall { receiver, args, .. } => {
            collect_ident_refs(receiver, out);
            for arg in args {
                collect_ident_refs(arg, out);
            }
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_ident_refs(receiver, out);
            collect_ident_refs(index, out);
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_ident_refs(&f.value, out);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_ident_refs(e, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_ident_refs(k, out);
                collect_ident_refs(v, out);
            }
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let StringPart::Expr(e, _) = part {
                    collect_ident_refs(e, out);
                }
            }
        }
        Expr::PostfixOp { receiver, .. } => collect_ident_refs(receiver, out),
        Expr::Is { expr, .. } => collect_ident_refs(expr, out),
        // Literals and SelfValue don't reference data bindings.
        Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::StringLit(..)
        | Expr::NoneLit { .. }
        | Expr::SelfValue { .. }
        | Expr::Error(..) => {} // TypeAttachedConst does not appear in ynz-ast Expr; covered by the catch-all.
    }
}

/// Collect all ident names in an expression into an owned HashSet.
fn collect_ident_names(expr: &Expr, out: &mut HashSet<String>) {
    let mut tmp = HashSet::new();
    collect_ident_refs(expr, &mut tmp);
    out.extend(tmp.into_iter().map(|s| s.to_string()));
}

// ── Write-effect summary ──────────────────────────────────────────────────────

/// Per-function name: indices of write-capable (non-`share` heap) parameters.
type WriteEffectMap = HashMap<String, Vec<usize>>;

/// Compute the write-capable parameter positions for one function (the conservative floor).
///
/// A position is a potential aliased write iff its parameter TYPE is a mutable heap reference
/// (`shape`/`array`/`map`/maybe/union/`dynamic`). A trivially-copyable scalar
/// (`int`/`float`/`boolean`/`number`) is passed by value and an immutable `string` cannot be
/// mutated through a reference, so neither is ever a write-effect. See the function body and
/// the module-level "Soundness floor" doc for why this is type-based rather than a per-form
/// ownership classification.
fn write_positions_for_sig(sig: &FunctionSig) -> Vec<usize> {
    // CONSERVATIVE FLOOR (sound by construction): a position is a potential aliased write
    // iff its parameter TYPE is a mutable heap reference (`shape`/`array`/`map`/...). The
    // per-form effective-ownership classification is deliberately NOT consulted to narrow
    // this. A name-based AST analysis cannot prove a callee only reads a heap argument
    // without receiver types AND alias tracking — i.e., without being a full borrow checker.
    // Treating EVERY mutable-heap argument as a potential write makes auto-parallelization
    // sound regardless of how the callee uses the value (direct mutation, mutation through a
    // field/index, a mutating method, a `self`-receiver, a same-named user function, or a
    // local alias of the argument). The cost is that a genuinely read-only mutable-heap
    // argument does not parallelize — a forfeited perf capability documented in
    // `design/concurrency.md` "Design Divergences" with its reversal path (a real
    // type+alias-aware ownership analysis re-enables it). Golden Rule 5 (compile-time
    // soundness — no silent runtime miscompile) outranks Golden Rule 10 (efficiency-first):
    // when they conflict, soundness wins. Primitives are passed by copy and immutable strings
    // cannot be mutated through a reference, so neither is ever a write hazard (see
    // `is_mutable_heap_type`) — that preserves the I/O-overlap headline (no-arg, primitive-,
    // and string-argument suspending calls still parallelize).
    (0..sig.params.len())
        .filter(|&i| {
            sig.params
                .get(i)
                .map(|(_, ty)| ty)
                .is_some_and(is_mutable_heap_type)
        })
        .collect()
}

/// True when a value of this type is a MUTABLE heap reference — the only kind of argument
/// that can be a potential aliased write.
///
/// Excluded (never a write hazard regardless of effective ownership):
/// - Trivially-copyable scalars (`int`/`float`/`boolean`/`number`): passed by copy, so a
///   write to the copy cannot alias the caller's value (`is_trivially_copyable`).
/// - `string`: immutable (`spec/strings.md` line 99). A string reference cannot be used to
///   mutate the string, so passing one is never a write hazard even at a `Writes` position.
///
/// Everything else (`shape`, `array`, `map`, `maybe`, union, `dynamic`, and generics/maybe/
/// union OF mutable types) is a mutable-heap reference: passing it shares the underlying
/// object, so a write through it IS observable at the call site. maybe/union/dynamic are
/// treated as mutable-heap conservatively (they may wrap a mutable shape).
fn is_mutable_heap_type(ty: &ynz_typeck::Type) -> bool {
    use ynz_typeck::Type;
    if is_trivially_copyable(ty) {
        return false;
    }
    // Strings are immutable — never a write hazard.
    !matches!(ty, Type::String)
}

/// Build the write-effect summary from `sig_table` and `imported_fns`.
///
/// Per-position write-capability is the conservative floor: a position is write-capable iff
/// its parameter TYPE is a mutable heap reference (see `write_positions_for_sig`). Local
/// definitions take priority over imported ones of the same name.
fn build_write_effect_summary(
    sig_table: &SignatureTable,
    imported_fns: &HashMap<String, FunctionSig>,
) -> WriteEffectMap {
    let mut map: WriteEffectMap = HashMap::new();

    for (name, sig) in &sig_table.fns {
        map.insert(name.clone(), write_positions_for_sig(sig));
    }

    for (name, sig) in imported_fns {
        if map.contains_key(name) {
            continue; // Local definition takes priority.
        }
        map.insert(name.clone(), write_positions_for_sig(sig));
    }

    map
}

/// Names of arguments passed at write-capable positions in a call.
fn call_site_write_set(args: &[Expr], write_positions: &[usize]) -> HashSet<String> {
    let mut writes = HashSet::new();
    for (i, arg) in args.iter().enumerate() {
        if write_positions.contains(&i) {
            collect_ident_names(arg, &mut writes);
        }
    }
    writes
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `(callee_name, args)` when `stmt` is a direct suspending call to a known callee.
/// Returns `None` for anything else (method calls, wait-prefixed, non-suspending, etc.).
fn stmt_call_args<'a>(stmt: &'a Stmt) -> Option<(&'a str, &'a [Expr])> {
    let call_expr: &'a Expr = match stmt {
        Stmt::Let { value, .. } | Stmt::Expr(value) => value,
        _ => return None,
    };
    // Unwrap Wait prefix if present — but wait-prefixed stmts are filtered upstream.
    let call = match call_expr {
        Expr::Call(c) => c,
        Expr::Wait(inner, _) => match inner.as_ref() {
            Expr::Call(c) => c,
            _ => return None,
        },
        _ => return None,
    };
    let callee_name = match &call.callee {
        Expr::Ident(name, _) => name.as_str(),
        _ => return None,
    };
    Some((callee_name, call.args.as_slice()))
}

/// Returns the callee name if `stmt` is a direct suspending call (no `wait` prefix),
/// `None` otherwise.
///
/// Candidates are:
/// - `let name = calleeF(args)` where `calleeF` is in `suspend_set`
/// - `calleeF(args)` (bare expression) where `calleeF` is in `suspend_set`
///
/// Explicitly `wait`-prefixed calls are excluded — those are ordering barriers.
pub fn stmt_direct_suspending_callee(stmt: &Stmt, suspend_set: &HashSet<String>) -> Option<String> {
    match stmt {
        Stmt::Let {
            value: Expr::Call(c),
            ..
        }
        | Stmt::Expr(Expr::Call(c)) => {
            if let Expr::Ident(name, _) = &c.callee {
                if suspend_set.contains(name.as_str()) {
                    return Some(name.clone());
                }
            }
            None
        }
        _ => None,
    }
}

/// Returns `true` if the statement has an explicit `wait` prefix.
pub fn stmt_has_explicit_wait(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(Expr::Wait(..))
            | Stmt::Let {
                value: Expr::Wait(..),
                ..
            }
    )
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_ast::nodes::CallExpr;
    use ynz_diagnostics::SourceSpan;

    fn src_span() -> SourceSpan {
        SourceSpan {
            file: String::new(),
            start: 0,
            end: 0,
        }
    }

    fn ident(name: &str) -> Expr {
        Expr::Ident(name.to_string(), src_span())
    }

    fn call_stmt(callee: &str, args: Vec<Expr>) -> Stmt {
        Stmt::Expr(Expr::Call(Box::new(CallExpr {
            callee: ident(callee),
            type_args: None,
            args,
            span: src_span(),
        })))
    }

    fn let_stmt(name: &str, callee: &str, args: Vec<Expr>) -> Stmt {
        Stmt::Let {
            name: name.to_string(),
            name_span: src_span(),
            ty: None,
            value: Expr::Call(Box::new(CallExpr {
                callee: ident(callee),
                type_args: None,
                args,
                span: src_span(),
            })),
            is_const: false,
            span: src_span(),
        }
    }

    /// A heap (non-trivially-copyable) parameter type for write-effect tests.
    fn heap_ty() -> ynz_typeck::Type {
        ynz_typeck::Type::Shape {
            name: "Box".to_string(),
        }
    }

    /// Build a `FunctionSig` whose `params` carry the given per-position types.
    ///
    /// The conservative-floor write-effect (`write_positions_for_sig`) reads ONLY the
    /// parameter TYPES (mutable-heap vs scalar/string) — never the declared modifier and
    /// never an effective-ownership classification. `param_ownerships` is filled with `None`.
    fn sig_typed(tys: Vec<ynz_typeck::Type>) -> FunctionSig {
        use ynz_typeck::Type;
        let params: Vec<(String, Type)> = tys
            .iter()
            .enumerate()
            .map(|(i, ty)| (format!("p{i}"), ty.clone()))
            .collect();
        FunctionSig {
            params,
            param_ownerships: vec![None; tys.len()],
            ret: Type::Nothing,
            decl_span: SourceSpan {
                file: String::new(),
                start: 0,
                end: 0,
            },
            contains_wait: false,
            suspends: true,
            original_name: None,
        }
    }

    /// Build a `SignatureTable` from a per-function declaration of `(name, [Type])`. The sig
    /// supplies parameter types for the mutable-heap test — the only input the conservative
    /// floor consumes. This is the single builder for the partition tests.
    fn table_only(defs: &[(&str, Vec<ynz_typeck::Type>)]) -> SignatureTable {
        let mut table = SignatureTable {
            fns: HashMap::new(),
        };
        for (name, tys) in defs {
            table
                .fns
                .insert((*name).to_string(), sig_typed(tys.clone()));
        }
        table
    }

    fn suspend_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // ── Data-dependency detection ─────────────────────────────────────────────

    #[test]
    fn data_dependent_stmts_are_not_independent() {
        // WHY: A statement that reads the binding another defines must be placed
        // sequentially. Wrong analysis would mark them as Parallel, corrupting output
        // and proving the cross-impl gate RED.
        let effects: WriteEffectMap = HashMap::new();
        let susp = suspend_set(&["fetcher"]);
        let a = let_stmt("x", "fetcher", vec![]);
        let b = call_stmt("fetcher", vec![ident("x")]);
        assert!(
            !stmts_are_independent(&a, &b, &effects),
            "b reads `x` which a defines — must NOT be independent"
        );
    }

    #[test]
    fn define_define_conflict_not_independent() {
        // WHY: Two stmts that both define the same name would shadow each other.
        // Conservative: not independent.
        let effects: WriteEffectMap = HashMap::new();
        let susp = suspend_set(&["fa", "fb"]);
        let a = let_stmt("x", "fa", vec![]);
        let b = let_stmt("x", "fb", vec![]);
        assert!(
            !stmts_are_independent(&a, &b, &effects),
            "both define `x` — must NOT be independent"
        );
    }

    #[test]
    fn truly_independent_stmts_are_independent() {
        // WHY: Stmts with no data dep and no write conflict must be marked independent
        // so the parallel pass can group them.
        let effects: WriteEffectMap = HashMap::new();
        let susp = suspend_set(&["fa", "fb"]);
        let a = let_stmt("x", "fa", vec![]);
        let b = let_stmt("y", "fb", vec![]);
        assert!(
            stmts_are_independent(&a, &b, &effects),
            "no shared defs/reads/write-effects — must be independent"
        );
    }

    #[test]
    fn lend_conflict_not_independent() {
        // WHY: If both calls write the same binding, they have a write conflict and must
        // be sequenced. This is the `writeA(x); writeB(x)` pattern in the plan ACs. The
        // write at position 0 comes from the conservative floor (heap-typed arg = potential
        // write).
        let table = table_only(&[("writer", vec![heap_ty()])]);
        let effects = build_write_effect_summary(&table, &HashMap::new());
        let susp = suspend_set(&["writer"]);
        let a = call_stmt("writer", vec![ident("counter")]);
        let b = call_stmt("writer", vec![ident("counter")]);
        assert!(
            !stmts_are_independent(&a, &b, &effects),
            "both write `counter` — write conflict, must NOT be independent"
        );
    }

    // ── Aliasing-soundness: heap-lend pairs are always sequenced ─────────────────

    #[test]
    fn aliased_lend_pair_produces_singletons_not_parallel() {
        // WHY: `let b = a` makes `b` an alias for `a` — both bindings point at the same
        // heap object (shapes/arrays/maps are heap references; assignment copies the pointer).
        // The independence analysis cannot distinguish aliased names from distinct objects,
        // so it conservatively sequences any pair where either call has a `lend` argument.
        //
        // The unit test proves `partition_independent_groups` produces two Singleton groups
        // (NOT a Parallel) for `slowBump(a, 10); slowBump(b, 100)` where both have lend pos 0.
        // This closes the soundness gap: the correct result is sequential regardless of whether
        // `b` was aliased from `a` or is a distinct object — alias analysis is required to
        // distinguish the two cases, and we don't have it.
        // slowBump: pos 0 is a heap arg (potential write under the floor), pos 1 is a scalar.
        let table = table_only(&[("slowBump", vec![heap_ty(), ynz_typeck::Type::Int])]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["slowBump"]);
        let a_stmt = call_stmt("slowBump", vec![ident("a"), ident("10")]);
        let b_stmt = call_stmt("slowBump", vec![ident("b"), ident("100")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            2,
            "aliased-lend pair: expected 2 Singleton groups, got {} groups",
            groups.len()
        );
        assert!(
            matches!(groups[0], IndependentGroup::Singleton(_)),
            "group[0] must be Singleton (aliasing-soundness: lend-lend always sequential)"
        );
        assert!(
            matches!(groups[1], IndependentGroup::Singleton(_)),
            "group[1] must be Singleton (aliasing-soundness: lend-lend always sequential)"
        );
    }

    #[test]
    fn aliased_share_read_vs_lend_write_produces_singletons() {
        // WHY: `let b = a; let r = slowRead(b); fastBump(a)` — slowRead takes a `share`-heap arg
        // (read-only) on b, fastBump takes a `lend`-heap arg (write) on a. Since `b = a` they
        // alias the same heap object. Parallelizing slowRead(b) with fastBump(a) means the read
        // may sample the object AFTER the write commits → wrong value (live-reproduced: r:110 vs
        // r:10 expected). The guard must fire on ANY pair where EITHER statement has a
        // write-capable arg, not just when BOTH do. This test locks that the share-heap read
        // does NOT count as a write while the lend-heap write does.
        // Both take a heap arg; under the conservative floor both are potential writes, so
        // the pair sequentializes regardless of which one actually mutates.
        let table = table_only(&[("slowRead", vec![heap_ty()]), ("fastBump", vec![heap_ty()])]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["slowRead", "fastBump"]);

        let read_stmt = let_stmt("r", "slowRead", vec![ident("b")]);
        let write_stmt = call_stmt("fastBump", vec![ident("a")]);
        let stmts = vec![read_stmt, write_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            2,
            "share-read vs lend-write pair: expected 2 Singleton groups (aliasing-soundness), got {}",
            groups.len()
        );
        assert!(
            matches!(groups[0], IndependentGroup::Singleton(_)),
            "group[0] must be Singleton — lend-write partner means pair is not independent"
        );
        assert!(
            matches!(groups[1], IndependentGroup::Singleton(_)),
            "group[1] must be Singleton — lend-write partner means pair is not independent"
        );
    }

    #[test]
    fn no_write_args_different_bindings_can_parallelize() {
        // WHY: Two calls where NEITHER has a write-capable argument (both read-only) have
        // no write-effect conflict. They are independent and eligible for parallelization.
        // This confirms the aliasing-soundness guard only blocks write-arg pairs, not
        // read-only pairs (the common case for parallel I/O reads).
        let effects: WriteEffectMap = HashMap::new(); // no write positions
        let susp = suspend_set(&["fa", "fb"]);
        let a = let_stmt("x", "fa", vec![ident("data_a")]);
        let b = let_stmt("y", "fb", vec![ident("data_b")]);
        assert!(
            stmts_are_independent(&a, &b, &effects),
            "calls with no write args and distinct defs — must be independent (parallelizable)"
        );
    }

    // ── HOLE A + soundness: give / bare-heap / share-heap / scalar classification ──

    #[test]
    fn give_arg_produces_singletons() {
        // WHY (HOLE A): `give` is ownership transfer = write+free. A callee that takes a
        // heap value by `give` writes (consumes/frees) it. Two such calls on potentially-
        // aliased heap bindings must be sequenced — the old lend-only summary missed `give`
        // entirely, letting a give+aliased-read pair parallelize → use-after-free / wrong read.
        // This test locks that a `give`-heap argument is classified write-capable.
        // consume: `give`-heap param. Under the floor a heap arg is a potential write, so the
        // give case is covered without distinguishing give from lend.
        let table = table_only(&[("consume", vec![heap_ty()])]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["consume"]);
        let a_stmt = call_stmt("consume", vec![ident("a")]);
        let b_stmt = call_stmt("consume", vec![ident("b")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            2,
            "give-heap pair: expected 2 Singleton groups (give is a write), got {}",
            groups.len()
        );
        assert!(matches!(groups[0], IndependentGroup::Singleton(_)));
        assert!(matches!(groups[1], IndependentGroup::Singleton(_)));
    }

    #[test]
    fn bare_write_heap_pair_sequentializes() {
        // WHY: a BARE heap parameter is a potential write under the conservative floor (the
        // floor does not try to distinguish a bare-mutating param from a bare-reading one —
        // either could mutate the shared heap object). Two such calls on potentially-aliased
        // heap bindings must be sequenced.
        let table = table_only(&[("bareMut", vec![heap_ty()])]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["bareMut"]);
        let a_stmt = call_stmt("bareMut", vec![ident("a")]);
        let b_stmt = call_stmt("bareMut", vec![ident("b")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            2,
            "bare-write-heap pair: expected 2 Singleton groups (bare-mutating heap is Writes), got {}",
            groups.len()
        );
        assert!(matches!(groups[0], IndependentGroup::Singleton(_)));
        assert!(matches!(groups[1], IndependentGroup::Singleton(_)));
    }

    #[test]
    fn bare_read_heap_pair_sequentializes_under_floor() {
        // test-ratchet: the conservative floor (Golden Rule 5 soundness > Rule 10 perf)
        // forfeits read-only-mutable-heap parallelism. The floor treats every mutable-heap arg
        // as a potential write — it does not trust any per-form ownership classifier (that
        // approach missed five distinct write forms across three gate rounds) — so two
        // read-only bare-heap calls SEQUENTIALIZE. The assertion locks that sound behavior;
        // reversal path (re-enable parallelism) is a real type+alias-aware ownership analysis.
        //
        // WHY: two bare-heap-arg calls sequentialize under the floor — a bare heap arg could
        // mutate the shared object (the floor does not prove otherwise), so the conservative
        // result is two Singleton groups.
        let table = table_only(&[("readA", vec![heap_ty()]), ("readB", vec![heap_ty()])]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["readA", "readB"]);
        let a_stmt = let_stmt("x", "readA", vec![ident("a")]);
        let b_stmt = let_stmt("y", "readB", vec![ident("b")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            2,
            "bare-read-heap pair: floor sequentializes mutable-heap args → 2 Singletons, got {}",
            groups.len()
        );
        assert!(matches!(groups[0], IndependentGroup::Singleton(_)));
        assert!(matches!(groups[1], IndependentGroup::Singleton(_)));
    }

    #[test]
    fn mutable_heap_share_read_pair_sequentializes_under_floor() {
        // test-ratchet: same forfeiture as `bare_read_heap_pair_sequentializes_under_floor`.
        // Under the conservative floor a mutable-heap arg is unconditionally a potential write
        // (the floor does not consult `share` enforcement, which proved incomplete), so two
        // `share`-heap reads sequentialize. The assertion locks that sound behavior.
        //
        // WHY: two share-heap-arg calls sequentialize under the floor — soundness over the
        // read-only-heap-overlap perf capability (Golden Rule 5 > Rule 10).
        let table = table_only(&[("readA", vec![heap_ty()]), ("readB", vec![heap_ty()])]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["readA", "readB"]);
        let a_stmt = let_stmt("x", "readA", vec![ident("a")]);
        let b_stmt = let_stmt("y", "readB", vec![ident("b")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            2,
            "share-heap read pair: floor sequentializes mutable-heap args → 2 Singletons, got {}",
            groups.len()
        );
        assert!(matches!(groups[0], IndependentGroup::Singleton(_)));
        assert!(matches!(groups[1], IndependentGroup::Singleton(_)));
    }

    #[test]
    fn trivially_copyable_args_can_parallelize() {
        // WHY: scalar arguments (`int`/`float`/`boolean`/`number`) are passed by copy — a
        // callee that "writes" a copied scalar parameter cannot affect the caller's value,
        // so there is no aliasing. Even at an effective-`Writes` position, a trivially-copyable
        // parameter is NOT write-capable (the heap-type test excludes it). Two such calls must
        // collapse into ONE Parallel group.
        let table = table_only(&[
            ("addA", vec![ynz_typeck::Type::Int]),
            ("addB", vec![ynz_typeck::Type::Int]),
        ]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["addA", "addB"]);
        let a_stmt = let_stmt("x", "addA", vec![ident("n")]);
        let b_stmt = let_stmt("y", "addB", vec![ident("m")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            1,
            "scalar-arg pair: expected 1 Parallel group, got {} groups",
            groups.len()
        );
        assert!(
            matches!(&groups[0], IndependentGroup::Parallel(v) if v.len() == 2),
            "group[0] must be a Parallel of 2 (scalar args are passed by copy — no aliasing)"
        );
    }

    #[test]
    fn string_args_parallelize() {
        // WHY: strings are immutable (`spec/strings.md` line 99) — a `string` argument is never
        // a write hazard regardless of its effective ownership. Two calls passing string args
        // must parallelize even when the position's effective ownership is `Writes`/`Unknown`,
        // because the value itself cannot be mutated through the reference.
        let table = table_only(&[
            ("emitA", vec![ynz_typeck::Type::String]),
            ("emitB", vec![ynz_typeck::Type::String]),
        ]);
        let imported: HashMap<String, _> = HashMap::new();
        let susp = suspend_set(&["emitA", "emitB"]);
        let a_stmt = let_stmt("x", "emitA", vec![ident("s")]);
        let b_stmt = let_stmt("y", "emitB", vec![ident("t")]);
        let stmts = vec![a_stmt, b_stmt];

        let groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        assert_eq!(
            groups.len(),
            1,
            "string-arg pair: expected 1 Parallel group (strings are immutable), got {} groups",
            groups.len()
        );
        assert!(
            matches!(&groups[0], IndependentGroup::Parallel(v) if v.len() == 2),
            "group[0] must be a Parallel of 2 (string args are immutable — never a write hazard)"
        );
    }

    // ── Gate discrimination: wrong analysis → wrong groups → cross-impl diverges ──

    #[test]
    fn gate_discrimination_wrong_independent_verdict_would_change_grouping() {
        // WHY: This is the planted-bug gate-discrimination test. It verifies that a
        // wrong independence verdict (marking dependent stmts as "independent") produces
        // a DIFFERENT partition than the correct analysis. When the default-parallel mode
        // uses the wrong partition and --no-auto-parallel uses source order, the outputs
        // diverge → the cross-impl consistency gate goes RED.
        //
        // The "bug" here is: bypassing the independence check and declaring everything
        // parallel. We verify the correct analysis says NOT independent, while the buggy
        // all-parallel partition differs.
        let susp = suspend_set(&["fetcher"]);
        let table = SignatureTable {
            fns: HashMap::new(),
        };
        let imported: HashMap<String, _> = HashMap::new();
        let a = let_stmt("x", "fetcher", vec![]);
        let b = call_stmt("fetcher", vec![ident("x")]); // data-dep on `a`
        let stmts = vec![a, b];

        let correct_groups = partition_independent_groups(&stmts, &susp, &table, &imported);
        // With correct analysis: a is Singleton (data-dep breaks grouping), b is Singleton.
        assert_eq!(
            correct_groups.len(),
            2,
            "correct analysis: two dependent stmts → two Singleton groups"
        );
        assert!(
            matches!(correct_groups[0], IndependentGroup::Singleton(_)),
            "correct analysis: group[0] must be Singleton"
        );
        assert!(
            matches!(correct_groups[1], IndependentGroup::Singleton(_)),
            "correct analysis: group[1] must be Singleton"
        );
        // If the bug existed (skip the independence check, treat all as parallel):
        // the partition would produce one Parallel(2) group instead of two Singletons.
        // Running that parallel group would poll both concurrently — but `b` reads `x`
        // which `a` hasn't produced yet → data race / wrong value.
        // The cross-impl oracle (--no-auto-parallel = sequential) would produce the
        // correct output; the buggy parallel mode would produce wrong output → gate RED.
        // This test locks the CORRECT partition shape so a bug regresses this test first.
    }
}
