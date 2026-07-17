//! Transitive effective-ownership fixpoint for v0.3-M3b Phase 4.
//!
//! # What this computes
//!
//! For every function parameter, the analysis answers: "across every path through
//! this function's body, is the parameter only READ, definitely WRITTEN, or does it
//! flow somewhere UNANALYZABLE?" The answer is the parameter's *effective* ownership —
//! distinct from its *declared* modifier (`share`/`lend`/`give`/bare). A bare parameter
//! whose body mutates it has declared modifier `None` but effective ownership `Writes`;
//! the compiler infers `lend` for it (`design/ownership.md` line 41). A `share` parameter
//! whose body writes it transitively is a soundness violation (`design/concurrency.md`
//! line 651) — Part 2 of this fix-round rejects exactly that.
//!
//! # The lattice
//!
//! ```text
//!   Reads  ⊏  Unknown  ⊏  Writes
//! ```
//!
//! Three points, totally ordered. `Writes` dominates `Unknown` dominates `Reads`. The
//! JOIN of two values is the higher one. This ordering encodes the conservative bias:
//! a parameter that is `Reads` on one path and `Unknown` on another is `Unknown` (we
//! cannot prove read-only); a parameter that is `Unknown` on one path and `Writes` on
//! another is `Writes` (a definite write subsumes uncertainty).
//!
//! # Why `Reads ⊏ Unknown ⊏ Writes` and not the reverse
//!
//! Two consumers read this report, and the ordering serves both:
//!
//! - **Typeck enforcement (Part 2)** errors ONLY on `share` parameters that reach
//!   `Writes` — the proven-write case. `Unknown` is given the benefit of the doubt (no
//!   error), because we cannot prove a violation; the independence side keeps soundness.
//! - **Independence analysis (Part 3)** sequentializes any position whose effective
//!   ownership is `Writes` OR `Unknown` — both are "might write," so both forfeit
//!   parallelism. Only proven `Reads` positions parallelize.
//!
//! Putting `Unknown` between `Reads` and `Writes` makes BOTH consumers correct with one
//! lattice: typeck treats `Unknown` as "not a proven write" (no error), independence
//! treats it as "not a proven read" (sequentialize). The conservative answer in each
//! direction falls out of the ordering — there is no separate special-case for `Unknown`.
//!
//! # Termination
//!
//! Kleene fixpoint over a finite lattice. Each parameter's value starts at the bottom
//! (`Reads`) and only ever ascends (`Reads` → `Unknown` → `Writes`) as the iteration
//! discovers escalations. The JOIN is monotone and the lattice height is 3, so each
//! parameter can change at most twice. The total number of parameters is finite, so the
//! `changed` flag goes false after at most `2 · (total params)` iterations. Recursion and
//! mutual recursion converge for the same reason the may-block fixpoint does: a recursive
//! call reads the callee's CURRENT effective ownership, which only rises across passes —
//! the cycle stabilizes once no member rises further. Self-recursion (`f` calling `f`)
//! is a single-node cycle and terminates identically. This mirrors `may_block::analyze`'s
//! transitive `suspends` propagation.
//!
//! # The conservative-on-`Unknown` invariant (soundness-critical)
//!
//! Every path the analysis cannot classify resolves to `Unknown`, never `Reads`. A
//! silent `Reads` on an unanalyzable path is the one failure mode that makes the whole
//! analysis unsound — it would let typeck miss a real violation AND let independence
//! parallelize a real write. The `classify_*` functions below return `Unknown` from
//! every fall-through arm; there is no path that yields `Reads` by default. `Reads` is
//! ONLY ever the starting bottom value, raised away the instant any non-read use appears.
//!
//! # Scope boundary
//!
//! Intra-module: the fixpoint is built FULLY across the local call graph. Cross-module:
//! a call to an imported function whose effective ownership is not propagated into this
//! unit resolves to `Unknown` for any parameter that flows into it — sound-conservative
//! (independence sequentializes; typeck does not error). Full cross-module effective-
//! ownership propagation is a tracked follow-on; the `Unknown` fallback keeps the
//! cross-module boundary sound in the meantime.
//!
//! Time: O(P² · S) where P = total parameters, S = statements per body (each pass walks
//! every body; the height-3 lattice bounds the pass count at O(P)).
//! Space: O(P).

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{
    Block, ContractSig, Expr, FunctionDecl, Item, Module, OwnershipModifier, PostfixOpKind,
    ReceiverKind, Stmt,
};

use crate::builtins::{array_method_is_mutating, fixed_method_is_mutating, map_method_is_mutating};

// ── Public types ────────────────────────────────────────────────────────────────

/// The effective ownership of a single parameter across every path of its function body.
///
/// Lattice order: `Reads ⊏ Unknown ⊏ Writes` (see module doc). `Writes` dominates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveOwnership {
    /// Provably only read on every path. The bottom of the lattice and the only value
    /// that permits parallelization / passes the `share`-read-only check.
    Reads,
    /// At least one path is UNANALYZABLE (e.g. the parameter flows into an imported
    /// function whose effective ownership is not available in this unit). Conservative
    /// middle: typeck gives it the benefit of the doubt (no error), independence
    /// sequentializes it (treats it as a possible write).
    Unknown,
    /// The body DEFINITELY mutates / moves the parameter on some path (direct field or
    /// element write, pass to a write-capable callee position, `.give`/consume, or a
    /// method whose receiver is write-capable). The top of the lattice.
    Writes,
}

impl EffectiveOwnership {
    /// Numeric height for lattice comparison: `Reads` = 0, `Unknown` = 1, `Writes` = 2.
    fn rank(self) -> u8 {
        match self {
            EffectiveOwnership::Reads => 0,
            EffectiveOwnership::Unknown => 1,
            EffectiveOwnership::Writes => 2,
        }
    }

    /// JOIN (least upper bound): the higher of the two lattice points.
    ///
    /// Monotone — repeated joins only ascend, which is what guarantees the fixpoint
    /// terminates (module doc "Termination").
    fn join(self, other: EffectiveOwnership) -> EffectiveOwnership {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}

/// The result of the effective-ownership fixpoint.
///
/// `per_fn[name][i]` is the effective ownership of the `i`-th parameter of function
/// `name`. Functions absent from the map (e.g. imported with no available body) are
/// treated as all-`Unknown` by consumers — see [`EffectiveOwnershipReport::ownership_of`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveOwnershipReport {
    /// Per function name → per-parameter effective ownership, indexed by position.
    pub per_fn: HashMap<String, Vec<EffectiveOwnership>>,
}

impl EffectiveOwnershipReport {
    /// An empty report: every lookup resolves to `Unknown` (conservative).
    ///
    /// For call sites that do not run the fixpoint (test harnesses that only inspect
    /// diagnostics, codegen paths with no auto-parallel candidate). A position the
    /// analysis never saw is never treated as a proven read.
    pub fn empty() -> Self {
        EffectiveOwnershipReport {
            per_fn: HashMap::new(),
        }
    }

    /// Effective ownership of parameter `index` of function `name`.
    ///
    /// Returns `Unknown` (conservative) when the function is not in the report or the
    /// index is out of range — a position the analysis never saw is never treated as a
    /// proven read.
    pub fn ownership_of(&self, name: &str, index: usize) -> EffectiveOwnership {
        self.per_fn
            .get(name)
            .and_then(|v| v.get(index).copied())
            .unwrap_or(EffectiveOwnership::Unknown)
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Run the transitive effective-ownership fixpoint over the intra-unit module.
///
/// `declared_writes` carries, for each function name visible in this unit (local or
/// imported), the set of parameter positions whose DECLARED modifier is `lend` or `give`
/// (explicit writes) — used as a seed and as the boundary value for callees whose body is
/// not available (imported functions). Local function bodies refine this via the body
/// walk; imported functions keep exactly their declared write positions, with any
/// parameter the caller flows into them but that is NOT declared `lend`/`give` resolving
/// to `Unknown` at the caller (see [`classify_call_position`]).
///
/// `imported_fn_names` is the set of names defined in OTHER modules — used to detect the
/// cross-module boundary so a flow into an imported callee at a non-declared-write
/// position resolves to `Unknown` rather than a spurious `Reads`.
///
/// Time: O(P² · S)  Space: O(P)  (see module doc).
pub fn analyze(
    module: &Module,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> EffectiveOwnershipReport {
    let fns = local_functions(module);

    // Map: fn name → parameter names (in order). The body walk matches a parameter by
    // name; this gives the name set + position index for each local function.
    let param_names: HashMap<String, Vec<String>> = fns
        .iter()
        .map(|f| {
            (
                f.name.clone(),
                f.params.iter().map(|p| p.name.clone()).collect(),
            )
        })
        .collect();

    // Initialize every LOCAL parameter to the bottom of the lattice (`Reads`). The body
    // walk and the fixpoint only ever raise these. Declared `lend`/`give` positions are
    // seeded to `Writes` directly (the author already committed to a write there).
    let mut report: HashMap<String, Vec<EffectiveOwnership>> = HashMap::new();
    for f in &fns {
        let declared = declared_writes.get(&f.name);
        let row: Vec<EffectiveOwnership> = (0..f.params.len())
            .map(|i| {
                if declared.is_some_and(|set| set.contains(&i)) {
                    EffectiveOwnership::Writes
                } else {
                    EffectiveOwnership::Reads
                }
            })
            .collect();
        report.insert(f.name.clone(), row);
    }

    // Kleene fixpoint: re-walk every body, joining in escalations discovered using the
    // CURRENT effective ownership of every callee (transitive). The height-3 lattice + the
    // monotone JOIN guarantee termination (module doc "Termination").
    loop {
        let mut changed = false;
        for f in &fns {
            let names = &param_names[&f.name];
            for (i, param_name) in names.iter().enumerate() {
                // A position already at the top cannot rise further — skip it.
                let current = report[&f.name][i];
                if current == EffectiveOwnership::Writes {
                    continue;
                }
                let discovered = classify_param_in_block(
                    param_name,
                    &f.body,
                    &report,
                    declared_writes,
                    imported_fn_names,
                );
                let joined = current.join(discovered);
                if joined != current {
                    // Safe: `report` was seeded with an entry for every local function name
                    // (and the `i`th slot for every param) before the fixpoint loop began.
                    report.get_mut(&f.name).unwrap()[i] = joined;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    EffectiveOwnershipReport { per_fn: report }
}

/// Test-only entry point: run the fixpoint and return the report for exact assertions.
///
/// Mirrors `may_block::suspends_set_for_test`. Computes `declared_writes` and
/// `imported_fn_names` from the module itself (no imports), so tests can pass raw source.
pub fn analyze_for_test(module: &Module) -> EffectiveOwnershipReport {
    let declared_writes = declared_write_positions(module);
    let imported: HashSet<String> = HashSet::new();
    analyze(module, &declared_writes, &imported)
}

/// Collect, for each LOCAL function, the parameter positions whose DECLARED modifier is
/// `lend` or `give`. This is the explicit-write seed for the fixpoint and the boundary
/// value for the test entry point.
pub fn declared_write_positions(module: &Module) -> HashMap<String, HashSet<usize>> {
    let mut out: HashMap<String, HashSet<usize>> = HashMap::new();
    for f in local_functions(module) {
        let mut set = HashSet::new();
        for (i, p) in f.params.iter().enumerate() {
            if matches!(
                p.ownership,
                Some(OwnershipModifier::Lend) | Some(OwnershipModifier::Give)
            ) {
                set.insert(i);
            }
        }
        out.insert(f.name.clone(), set);
    }
    out
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn local_functions(module: &Module) -> Vec<&FunctionDecl> {
    module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(f) = item {
                Some(f)
            } else {
                None
            }
        })
        .collect()
}

/// Classify how `param_name` is used across a whole block, joining the result of every
/// statement. Conservative: any unanalyzable use raises to at least `Unknown`.
fn classify_param_in_block(
    param_name: &str,
    block: &Block,
    report: &HashMap<String, Vec<EffectiveOwnership>>,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> EffectiveOwnership {
    let mut acc = EffectiveOwnership::Reads;
    for stmt in &block.stmts {
        acc = acc.join(classify_param_in_stmt(
            param_name,
            stmt,
            report,
            declared_writes,
            imported_fn_names,
        ));
        if acc == EffectiveOwnership::Writes {
            return acc; // top — no need to keep walking
        }
    }
    acc
}

/// Classify how `param_name` is used in a single statement.
///
/// Direct field-assign / index-assign whose root binding is `param_name` is the clearest
/// `Writes`. Reassigning the binding itself (`param_name = ...`) is a local rebinding, not
/// a write through the value — it does not escalate. Every expression position is then
/// walked for flow into a call/method/give.
fn classify_param_in_stmt(
    param_name: &str,
    stmt: &Stmt,
    report: &HashMap<String, Vec<EffectiveOwnership>>,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> EffectiveOwnership {
    let classify_expr = |e: &Expr| {
        classify_param_in_expr(param_name, e, report, declared_writes, imported_fn_names)
    };
    let classify_block = |b: &Block| {
        classify_param_in_block(param_name, b, report, declared_writes, imported_fn_names)
    };

    match stmt {
        // Direct mutation through the parameter: `param.field = v` or `param[i] = v`.
        // The root binding being the parameter means the write lands on the value the
        // caller passed — a definite write.
        Stmt::FieldAssign { target, value, .. } => {
            let mut acc = classify_expr(value);
            if let Some(root) = root_binding_name(target) {
                if root == param_name {
                    return EffectiveOwnership::Writes;
                }
            }
            // The target may still READ the parameter elsewhere (e.g. `a.field = param.x`
            // walks `value`); the receiver chain is a read, not a write.
            acc = acc.join(classify_expr(target));
            acc
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            if let Some(root) = root_binding_name(receiver) {
                if root == param_name {
                    return EffectiveOwnership::Writes;
                }
            }
            classify_expr(receiver)
                .join(classify_expr(index))
                .join(classify_expr(value))
        }
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => classify_expr(value),
        Stmt::Expr(e) => classify_expr(e),
        Stmt::Return { value, .. } => value
            .as_ref()
            .map(classify_expr)
            .unwrap_or(EffectiveOwnership::Reads),
        Stmt::If { cond, body, .. } => classify_expr(cond).join(classify_block(body)),
        Stmt::While { cond, body, .. } => classify_expr(cond).join(classify_block(body)),
        Stmt::For { iter, body, .. } => classify_expr(iter).join(classify_block(body)),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            let mut acc = classify_expr(scrutinee);
            for arm in arms {
                acc = acc.join(classify_block(&arm.body));
            }
            if let Some(eb) = else_arm {
                acc = acc.join(classify_block(eb));
            }
            acc
        }
    }
}

/// Classify how `param_name` is used in an expression.
///
/// The interesting cases are CALLS: passing the parameter at a write-capable position
/// (the callee's effective ownership is `Writes`/`Unknown`, or its declared modifier is
/// `lend`/`give`) escalates the parameter to that callee's effective ownership. Method
/// calls map the receiver to the callee's `self` position. A `.give` postfix consumes.
/// Every other use is a read of the parameter; the recursion walks sub-expressions and
/// joins. The fall-through arms return `Reads` for leaves that cannot reference a binding,
/// and the call/method arms return `Unknown` for any callee the analysis cannot classify.
fn classify_param_in_expr(
    param_name: &str,
    expr: &Expr,
    report: &HashMap<String, Vec<EffectiveOwnership>>,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> EffectiveOwnership {
    let recurse = |e: &Expr| {
        classify_param_in_expr(param_name, e, report, declared_writes, imported_fn_names)
    };

    match expr {
        Expr::Call(call) => {
            let mut acc = EffectiveOwnership::Reads;
            // Direct ident callee → look up its per-position effective ownership.
            let callee_name = match &call.callee {
                Expr::Ident(name, _) => Some(name.as_str()),
                other => {
                    // Non-ident callee (function-value call) is unanalyzable for write
                    // tracking; any argument that IS the parameter flows somewhere we
                    // cannot classify.
                    acc = acc.join(recurse(other));
                    None
                }
            };
            for (i, arg) in call.args.iter().enumerate() {
                if arg_is_binding(arg, param_name) {
                    acc = acc.join(classify_call_position(
                        callee_name,
                        i,
                        report,
                        declared_writes,
                        imported_fn_names,
                    ));
                } else {
                    // The parameter may still be read inside a sub-expression of the arg.
                    acc = acc.join(recurse(arg));
                }
            }
            acc
        }
        Expr::MethodCall {
            receiver,
            args,
            method,
            ..
        } => {
            let mut acc = EffectiveOwnership::Reads;
            // UFCS: `recv.method(args)` desugars to `method(recv, args)`. The receiver
            // occupies position 0 (the `self` slot). If the receiver IS the parameter,
            // its effective ownership is the callee's position-0 ownership.
            if arg_is_binding(receiver, param_name) {
                // A builtin in-place collection mutator (`array.add`/`map.set`/`fixed.set`/…)
                // writes its receiver. The name-based check is authoritative ONLY for builtins
                // — a user-defined function reachable by this name (a standalone `function set`
                // used via UFCS) must use its REAL resolved ownership instead, or a read-only
                // user method named like a mutator would be misclassified Writes. The
                // pure-named-method contract (stdlib-design.md Rule 1) guarantees that builtin
                // non-mutating methods (`.get`/`.length`/`.toString`) only read, so routing
                // those through `classify_call_position` correctly yields Reads.
                let is_user_fn = report.contains_key(method.as_str())
                    || imported_fn_names.contains(method.as_str());
                let recv_own = if !is_user_fn
                    && (array_method_is_mutating(method)
                        || map_method_is_mutating(method)
                        || fixed_method_is_mutating(method))
                {
                    EffectiveOwnership::Writes
                } else {
                    classify_call_position(
                        Some(method.as_str()),
                        0,
                        report,
                        declared_writes,
                        imported_fn_names,
                    )
                };
                acc = acc.join(recv_own);
            } else {
                acc = acc.join(recurse(receiver));
            }
            for (i, arg) in args.iter().enumerate() {
                if arg_is_binding(arg, param_name) {
                    // Method args occupy positions 1.. (after `self`).
                    acc = acc.join(classify_call_position(
                        Some(method.as_str()),
                        i + 1,
                        report,
                        declared_writes,
                        imported_fn_names,
                    ));
                } else {
                    acc = acc.join(recurse(arg));
                }
            }
            acc
        }
        // `.give` consumes the value — a definite write/move. `.copy`/`.freeze` are reads
        // of the receiver (`.copy` produces a fresh value; `.freeze` does not mutate).
        Expr::PostfixOp { receiver, op, .. } => match op {
            PostfixOpKind::Copy | PostfixOpKind::Freeze => recurse(receiver),
        },
        Expr::BinOp { lhs, rhs, .. } => recurse(lhs).join(recurse(rhs)),
        Expr::UnaryOp { operand, .. } => recurse(operand),
        Expr::FieldAccess { receiver, .. } => recurse(receiver),
        Expr::IndexAccess {
            receiver, index, ..
        } => recurse(receiver).join(recurse(index)),
        Expr::Wait(inner, _) | Expr::Background(inner, _) => recurse(inner),
        Expr::Is { expr: inner, .. } => recurse(inner),
        Expr::StructLit { fields, .. } => {
            let mut acc = EffectiveOwnership::Reads;
            for f in fields {
                acc = acc.join(recurse(&f.value));
            }
            acc
        }
        Expr::ArrayLit { elements, .. } => {
            let mut acc = EffectiveOwnership::Reads;
            for e in elements {
                acc = acc.join(recurse(e));
            }
            acc
        }
        Expr::MapLit { entries, .. } => {
            let mut acc = EffectiveOwnership::Reads;
            for (k, v) in entries {
                acc = acc.join(recurse(k)).join(recurse(v));
            }
            acc
        }
        Expr::InterpolatedString(parts, _) => {
            let mut acc = EffectiveOwnership::Reads;
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    acc = acc.join(recurse(e));
                }
            }
            acc
        }
        // Leaf nodes — a bare `Ident` reference to the parameter is a READ (a write would
        // appear as a FieldAssign/IndexAssign statement or a write-capable call position,
        // both handled above). No leaf escalates on its own.
        Expr::Ident(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::StringLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => EffectiveOwnership::Reads,
    }
}

/// The effective ownership contributed when the parameter is passed at position `index`
/// of a call to `callee_name`.
///
/// Resolution order (each step is conservative):
/// 1. **Unknown callee** (`None` — non-ident callee, or name not classifiable) → `Unknown`.
/// 2. **Local function with a body** (in `report`) → its CURRENT effective ownership at
///    that position (transitive — this is the fixpoint edge). Out-of-range index →
///    `Unknown`.
/// 3. **Imported function** (in `imported_fn_names`, body not in this unit) → `Writes` if
///    the position is a declared `lend`/`give`, else `Unknown`. A cross-module non-declared
///    position cannot be proven read-only, so it is `Unknown`, never `Reads`.
/// 4. **Intrinsic / unknown name** (e.g. `print`, `sleep`, `.toString`, `range`) → a
///    declared write position is `Writes`; otherwise `Reads`. Pure-named intrinsics take
///    their arguments by value / `share`, so a parameter passed to one is read-only
///    (`stdlib-design.md` Rule 1 guarantees a read-named intrinsic only reads). (Intrinsics
///    have no entry in `report` or `imported_fn_names`; they reach this arm.) The ONE
///    intrinsic class that does NOT obey this — an in-place collection mutator
///    (`array.add`/`map.set`/`fixed.set`/…) — never reaches this arm: the `MethodCall` arm of
///    `classify_param_in_expr` classifies a mutating collection method on the receiver as
///    `Writes` by name BEFORE delegating here, so the receiver is never under-classified as
///    `Reads`.
fn classify_call_position(
    callee_name: Option<&str>,
    index: usize,
    report: &HashMap<String, Vec<EffectiveOwnership>>,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> EffectiveOwnership {
    let Some(name) = callee_name else {
        // Non-ident callee — cannot classify the destination at all.
        return EffectiveOwnership::Unknown;
    };

    // Local function with an analyzed body: use its current effective ownership (the
    // transitive fixpoint edge). An out-of-range index (arity mismatch handled elsewhere
    // by typeck) is conservatively `Unknown`.
    if let Some(row) = report.get(name) {
        return row
            .get(index)
            .copied()
            .unwrap_or(EffectiveOwnership::Unknown);
    }

    // Declared explicit-write position (covers imported functions whose declared `lend`/
    // `give` positions are known even without a body).
    let declared_write = declared_writes
        .get(name)
        .is_some_and(|set| set.contains(&index));

    if imported_fn_names.contains(name) {
        // Imported function, body not in this unit. A declared write is a definite write;
        // any other position cannot be proven read-only across the boundary → Unknown.
        if declared_write {
            return EffectiveOwnership::Writes;
        }
        return EffectiveOwnership::Unknown;
    }

    // Unknown name = intrinsic (print/sleep/range/...) or a name typeck will reject. A
    // declared write (none, for intrinsics) is a write; intrinsics take args by value /
    // share, so a parameter passed to one is a read.
    if declared_write {
        EffectiveOwnership::Writes
    } else {
        EffectiveOwnership::Reads
    }
}

/// True when `arg` is exactly a bare reference to `param_name` (`Ident` or `self`-as-name).
/// A field/index sub-expression (`param.field`) is NOT the binding itself — passing
/// `param.field` passes the field's value, not the parameter.
///
/// The parser emits the `self` keyword in expression position as `Expr::SelfValue`, not
/// `Expr::Ident("self")`. A `share self` method that flows `self` into a callee must match
/// here, so `self` is recognized as the binding named `"self"`.
fn arg_is_binding(arg: &Expr, param_name: &str) -> bool {
    match arg {
        Expr::Ident(name, _) => name == param_name,
        Expr::SelfValue { .. } => param_name == "self",
        _ => false,
    }
}

/// The root binding name of an assignable place expression (`a`, `a.b`, `a.b.c`, `a[i]`).
/// Returns `None` for expressions with no single binding root.
///
/// `self.field = v` roots in `self`: the parser emits `self` as `Expr::SelfValue`, so a
/// direct field/element write through `share self` is rooted to the name `"self"` here, the
/// same way the named-parameter case roots to its `Expr::Ident`.
fn root_binding_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name.as_str()),
        Expr::SelfValue { .. } => Some("self"),
        Expr::FieldAccess { receiver, .. } => root_binding_name(receiver),
        Expr::IndexAccess { receiver, .. } => root_binding_name(receiver),
        _ => None,
    }
}

// ── Receiver-kind helpers (for Part 2 enforcement; shared with check_query) ────────

/// Map a contract receiver kind to the parameter index-0 effective ownership a method's
/// `self` carries when used through a contract. Exposed so the dynamic-dispatch boundary
/// (Part 2 enforcement) can classify a contract method's declared `self` modifier.
pub fn receiver_kind_ownership(kind: &ReceiverKind) -> EffectiveOwnership {
    match kind {
        ReceiverKind::Share => EffectiveOwnership::Reads,
        ReceiverKind::Lend | ReceiverKind::Give => EffectiveOwnership::Writes,
    }
}

/// Map a contract signature's receiver to its effective `self` ownership, or `None` when
/// the signature has no receiver (a static contract function with no `self`).
pub fn contract_self_ownership(sig: &ContractSig) -> Option<EffectiveOwnership> {
    sig.receiver.as_ref().map(receiver_kind_ownership)
}

// ── Part 2: transitive share→write violation detection ────────────────────────

use ynz_diagnostics::SourceSpan;

/// A `share`-declared parameter that is written transitively through a callee — the
/// `design/concurrency.md` line 651 violation Part 2 rejects.
///
/// Only the TRANSITIVE case is reported here (the parameter is passed to a callee whose
/// effective ownership at that position is `Writes`). The DIRECT cases — a `share` body
/// that field-assigns the param, or passes it to an EXPLICIT `lend`/`give` position — are
/// already rejected by `check.rs` (`reject_share_param_mutation` and `check_arg_ownership`)
/// with a precise span on the assignment / argument. Reporting only the transitive case
/// here prevents double-erroring the same parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitiveShareViolation {
    /// The offending parameter's name.
    pub param_name: String,
    /// The parameter's type, rendered for the `lend x: Type` suggestion.
    pub param_type: ynz_ast::nodes::Type,
    /// The callee through which the write happens (named in the WHY).
    pub callee_name: String,
    /// Span pointing at the call site where the parameter is lent onward.
    pub span: SourceSpan,
}

/// Find every `share`-declared parameter that is written TRANSITIVELY through a callee.
///
/// `report` is the completed effective-ownership fixpoint. For each local function and each
/// parameter declared explicit `share` (including `share self`), scan the body for the FIRST
/// call/method-call position the parameter flows into whose callee effective ownership is
/// `Writes`. That call site is the violation: a `share` (read-only) parameter cannot be lent
/// onward to a function that writes it.
///
/// `Unknown`-classified flows are NOT reported (benefit of the doubt — the independence side
/// keeps soundness by sequentializing them). Only PROVEN writes are violations.
pub fn find_transitive_share_violations(
    module: &Module,
    report: &EffectiveOwnershipReport,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> Vec<TransitiveShareViolation> {
    let mut out = Vec::new();
    for f in local_functions(module) {
        for p in &f.params {
            // Only explicit `share` parameters are subject to the no-escalation rule. Bare
            // params infer their modifier from the body; `lend`/`give` declared writes.
            if p.ownership != Some(OwnershipModifier::Share) {
                continue;
            }
            if let Some(site) = first_transitive_write_site(
                &p.name,
                &f.body,
                report,
                declared_writes,
                imported_fn_names,
            ) {
                out.push(TransitiveShareViolation {
                    param_name: p.name.clone(),
                    param_type: p.ty.clone(),
                    callee_name: site.callee_name,
                    span: site.span,
                });
            }
        }
    }
    out
}

struct WriteSite {
    callee_name: String,
    span: SourceSpan,
}

/// Locate the first call/method-call site in `block` where `param_name` is passed at a
/// position whose callee effective ownership is `Writes`. Returns `None` when there is no
/// such transitive write (the parameter is only read, or only flows to `Unknown` sinks).
fn first_transitive_write_site(
    param_name: &str,
    block: &Block,
    report: &EffectiveOwnershipReport,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> Option<WriteSite> {
    for stmt in &block.stmts {
        if let Some(site) =
            first_write_site_in_stmt(param_name, stmt, report, declared_writes, imported_fn_names)
        {
            return Some(site);
        }
    }
    None
}

fn first_write_site_in_stmt(
    param_name: &str,
    stmt: &Stmt,
    report: &EffectiveOwnershipReport,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> Option<WriteSite> {
    let in_expr = |e: &Expr| {
        first_write_site_in_expr(param_name, e, report, declared_writes, imported_fn_names)
    };
    let in_block = |b: &Block| {
        first_transitive_write_site(param_name, b, report, declared_writes, imported_fn_names)
    };
    match stmt {
        Stmt::Expr(e) => in_expr(e),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => in_expr(value),
        Stmt::Return { value, .. } => value.as_ref().and_then(&in_expr),
        Stmt::FieldAssign { target, value, .. } => in_expr(target).or_else(|| in_expr(value)),
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => in_expr(receiver)
            .or_else(|| in_expr(index))
            .or_else(|| in_expr(value)),
        Stmt::If { cond, body, .. } => in_expr(cond).or_else(|| in_block(body)),
        Stmt::While { cond, body, .. } => in_expr(cond).or_else(|| in_block(body)),
        Stmt::For { iter, body, .. } => in_expr(iter).or_else(|| in_block(body)),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            if let Some(s) = in_expr(scrutinee) {
                return Some(s);
            }
            for arm in arms {
                if let Some(s) = in_block(&arm.body) {
                    return Some(s);
                }
            }
            else_arm.as_ref().and_then(in_block)
        }
    }
}

fn first_write_site_in_expr(
    param_name: &str,
    expr: &Expr,
    report: &EffectiveOwnershipReport,
    declared_writes: &HashMap<String, HashSet<usize>>,
    imported_fn_names: &HashSet<String>,
) -> Option<WriteSite> {
    let recurse = |e: &Expr| {
        first_write_site_in_expr(param_name, e, report, declared_writes, imported_fn_names)
    };
    match expr {
        Expr::Call(call) => {
            let callee_name = match &call.callee {
                Expr::Ident(name, _) => Some(name.as_str()),
                _ => None,
            };
            for (i, arg) in call.args.iter().enumerate() {
                if arg_is_binding(arg, param_name)
                    && classify_call_position(
                        callee_name,
                        i,
                        &report.per_fn,
                        declared_writes,
                        imported_fn_names,
                    ) == EffectiveOwnership::Writes
                {
                    return Some(WriteSite {
                        callee_name: callee_name.unwrap_or("a function").to_string(),
                        span: call.span.clone(),
                    });
                }
            }
            // Not a direct flow at this call — recurse into args for nested calls.
            for arg in &call.args {
                if let Some(s) = recurse(arg) {
                    return Some(s);
                }
            }
            None
        }
        Expr::MethodCall {
            receiver,
            args,
            method,
            span,
            ..
        } => {
            if arg_is_binding(receiver, param_name)
                && classify_call_position(
                    Some(method.as_str()),
                    0,
                    &report.per_fn,
                    declared_writes,
                    imported_fn_names,
                ) == EffectiveOwnership::Writes
            {
                return Some(WriteSite {
                    callee_name: method.clone(),
                    span: span.clone(),
                });
            }
            for (i, arg) in args.iter().enumerate() {
                if arg_is_binding(arg, param_name)
                    && classify_call_position(
                        Some(method.as_str()),
                        i + 1,
                        &report.per_fn,
                        declared_writes,
                        imported_fn_names,
                    ) == EffectiveOwnership::Writes
                {
                    return Some(WriteSite {
                        callee_name: method.clone(),
                        span: span.clone(),
                    });
                }
            }
            recurse(receiver).or_else(|| args.iter().find_map(&recurse))
        }
        Expr::BinOp { lhs, rhs, .. } => recurse(lhs).or_else(|| recurse(rhs)),
        Expr::UnaryOp { operand, .. } => recurse(operand),
        Expr::FieldAccess { receiver, .. } => recurse(receiver),
        Expr::IndexAccess {
            receiver, index, ..
        } => recurse(receiver).or_else(|| recurse(index)),
        Expr::Wait(inner, _) | Expr::Background(inner, _) => recurse(inner),
        Expr::Is { expr: inner, .. } => recurse(inner),
        Expr::PostfixOp { receiver, .. } => recurse(receiver),
        Expr::StructLit { fields, .. } => fields.iter().find_map(|f| recurse(&f.value)),
        Expr::ArrayLit { elements, .. } => elements.iter().find_map(recurse),
        Expr::MapLit { entries, .. } => entries
            .iter()
            .find_map(|(k, v)| recurse(k).or_else(|| recurse(v))),
        Expr::InterpolatedString(parts, _) => parts.iter().find_map(|p| {
            if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                recurse(e)
            } else {
                None
            }
        }),
        Expr::Ident(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::StringLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => None,
    }
}

// ── Aliasing-call rejection (v0.3-M7 Phase 2, FRAGO 002) ─────────────────────
//
// A call that passes the SAME value (or two overlapping pieces of one value, e.g.
// `player` and `player.pet`) into two parameter positions where at least one position
// can modify the value is a genuine violation of the ownership contract: `lend` means
// exclusive mutable access for the duration of the call, so no other live view of the
// value may exist. Typeck previously ACCEPTED such calls while codegen claimed LLVM
// `noalias` on both parameters — a false claim the optimizer exploits into a silent
// miscompile (RED fixture `v0_3_m7_p1_share_lend_alias.ynz`). Patrick's decision
// (FRAGO 002, 2026-07-16): reject at compile time (Golden Rule 5), with a teaching
// diagnostic (Golden Rule 11) — rather than merely dropping the attribute, which would
// leave typeck's unsoundness in place.
//
// Write-capability convention: a position counts as write-capable when its declared
// modifier is `lend`/`give`, OR its effective ownership is `Writes` (a bare param the
// body provably mutates — the inferred `lend`), OR `Unknown` (cannot prove read-only).
// Treating `Unknown` as "might write" is the SAME conservative convention the
// independence analysis uses (module doc, "The lattice") — one lattice, one reading.
// Only proven-`Reads` positions may share a value in one call (read-read overlap is
// harmless and LLVM's `noalias` explicitly permits it).
//
// Scope: place-paths (an identifier or a field path rooted at one). In Yinz's ownership
// model these are the only source-expressible aliases at a call site — there are no
// reference bindings, so two DISTINCT roots never denote the same value. Index accesses
// (`a[i]`) produce element copies/handles through the collection API and are out of this
// check's scope. Scalar-typed parameters (int/float/bool) pass by value and cannot
// alias — they are skipped.

/// How one argument position of a flagged call relates to the shared value —
/// used to render the teaching diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasArgKind {
    /// Declared `share` — a read-only view.
    DeclaredShare,
    /// Declared `lend` — the function modifies the value.
    DeclaredLend,
    /// Declared `give` — ownership moves into the function.
    DeclaredGive,
    /// Bare parameter the body provably mutates (effective ownership `Writes`).
    InferredWrite,
    /// Bare parameter proven read-only on every path (effective ownership `Reads`).
    InferredRead,
    /// Bare parameter the analysis cannot prove read-only (effective ownership
    /// `Unknown`) — treated as write-capable, same as the independence analysis.
    Unverifiable,
}

impl AliasArgKind {
    /// True when this position can modify (or take over) the value during the call.
    pub fn write_capable(self) -> bool {
        !matches!(
            self,
            AliasArgKind::DeclaredShare | AliasArgKind::InferredRead
        )
    }

    /// Plain-English rendering for the diagnostic (no compiler jargon).
    pub fn describe(self) -> &'static str {
        match self {
            AliasArgKind::DeclaredShare => "`share` (a read-only view)",
            AliasArgKind::DeclaredLend => "`lend` (the function modifies it)",
            AliasArgKind::DeclaredGive => "`give` (ownership moves into the function)",
            AliasArgKind::InferredWrite => "a parameter the function modifies",
            AliasArgKind::InferredRead => "a read-only parameter",
            AliasArgKind::Unverifiable => "a parameter the compiler cannot prove stays read-only",
        }
    }
}

/// One rejected aliasing call: the same value reaches two parameters of one call and
/// at least one of the two positions can modify it.
#[derive(Debug, Clone)]
pub struct AliasingCallViolation {
    /// Rendered place-path of the FIRST overlapping argument (e.g. `player`).
    pub first_path: String,
    /// Rendered place-path of the SECOND overlapping argument (e.g. `player.pet`).
    pub second_path: String,
    /// The callee whose call is rejected.
    pub callee_name: String,
    /// How the first overlapping position treats the value.
    pub first_kind: AliasArgKind,
    /// How the second overlapping position treats the value.
    pub second_kind: AliasArgKind,
    /// Span of the second overlapping argument (the point of the conflict).
    pub span: SourceSpan,
}

/// Find every call in the module that passes overlapping place-paths into two
/// parameter positions of one call where at least one position is write-capable.
///
/// `sigs` is the LOCAL signature map (name → sig); `imported_sigs` the cross-module
/// one — both consulted so imported callees are held to the same rule their own unit's
/// codegen relies on. Callee names with no signature entry (built-in methods,
/// intrinsics) are skipped — built-in receiver aliasing is governed by the collection
/// API's own rules, not by user-function ownership modifiers.
///
/// Time: O(S · A²) where S = call sites, A = arguments per call (A is tiny).
pub fn find_aliasing_call_violations(
    module: &Module,
    report: &EffectiveOwnershipReport,
    sigs: &HashMap<String, crate::signatures::FunctionSig>,
    imported_sigs: &HashMap<String, crate::signatures::FunctionSig>,
) -> Vec<AliasingCallViolation> {
    let mut out = Vec::new();
    for f in local_functions(module) {
        collect_aliasing_in_block(&f.body, report, sigs, imported_sigs, &mut out);
    }
    out
}

/// A place-path: the root identifier plus any field chain (`player.pet.name` →
/// `["player", "pet", "name"]`). `None` for any expression that produces a fresh
/// value (literals, calls, operators) — fresh values cannot alias anything.
fn place_path(e: &Expr) -> Option<Vec<String>> {
    match e {
        Expr::Ident(n, _) => Some(vec![n.clone()]),
        Expr::SelfValue { .. } => Some(vec!["self".to_string()]),
        Expr::FieldAccess {
            receiver, field, ..
        } => {
            let mut p = place_path(receiver)?;
            p.push(field.clone());
            Some(p)
        }
        _ => None,
    }
}

/// Two place-paths overlap when one is a prefix of the other (same value, or one
/// argument is a piece of the other).
fn paths_overlap(a: &[String], b: &[String]) -> bool {
    let n = a.len().min(b.len());
    a[..n] == b[..n]
}

fn render_path(p: &[String]) -> String {
    p.join(".")
}

/// Classify how parameter `index` of callee `name` treats its value.
///
/// Returns `None` for scalar-typed parameters (int/float/bool pass by value — no
/// aliasing possible) and for callees with no known signature.
fn alias_arg_kind(
    name: &str,
    index: usize,
    report: &EffectiveOwnershipReport,
    sigs: &HashMap<String, crate::signatures::FunctionSig>,
    imported_sigs: &HashMap<String, crate::signatures::FunctionSig>,
) -> Option<AliasArgKind> {
    let sig = sigs.get(name).or_else(|| imported_sigs.get(name))?;
    let (_, param_ty) = sig.params.get(index)?;
    if matches!(
        param_ty,
        crate::types::Type::Int | crate::types::Type::Float | crate::types::Type::Bool
    ) {
        return None;
    }
    let declared = sig.param_ownerships.get(index).cloned().flatten();
    Some(match declared {
        Some(OwnershipModifier::Share) => AliasArgKind::DeclaredShare,
        Some(OwnershipModifier::Lend) => AliasArgKind::DeclaredLend,
        Some(OwnershipModifier::Give) => AliasArgKind::DeclaredGive,
        None => match report.ownership_of(name, index) {
            EffectiveOwnership::Reads => AliasArgKind::InferredRead,
            EffectiveOwnership::Writes => AliasArgKind::InferredWrite,
            EffectiveOwnership::Unknown => AliasArgKind::Unverifiable,
        },
    })
}

/// Check one call's argument list (already normalized: UFCS receiver prepended) for
/// overlapping place-paths with a write-capable side.
fn check_call_args_for_aliasing(
    callee_name: &str,
    args: &[&Expr],
    report: &EffectiveOwnershipReport,
    sigs: &HashMap<String, crate::signatures::FunctionSig>,
    imported_sigs: &HashMap<String, crate::signatures::FunctionSig>,
    out: &mut Vec<AliasingCallViolation>,
) {
    // Resolve each argument to (index, path, kind); skip fresh values and scalars.
    let mut places: Vec<(usize, Vec<String>, AliasArgKind)> = Vec::new();
    for (i, arg) in args.iter().enumerate() {
        let Some(path) = place_path(arg) else {
            continue;
        };
        let Some(kind) = alias_arg_kind(callee_name, i, report, sigs, imported_sigs) else {
            continue;
        };
        places.push((i, path, kind));
    }
    for a in 0..places.len() {
        for b in (a + 1)..places.len() {
            let (_, pa, ka) = &places[a];
            let (bi, pb, kb) = &places[b];
            if paths_overlap(pa, pb) && (ka.write_capable() || kb.write_capable()) {
                out.push(AliasingCallViolation {
                    first_path: render_path(pa),
                    second_path: render_path(pb),
                    callee_name: callee_name.to_string(),
                    first_kind: *ka,
                    second_kind: *kb,
                    span: args[*bi].span().clone(),
                });
                // One report per call is enough teaching; stop at the first pair.
                return;
            }
        }
    }
}

fn collect_aliasing_in_block(
    block: &Block,
    report: &EffectiveOwnershipReport,
    sigs: &HashMap<String, crate::signatures::FunctionSig>,
    imported_sigs: &HashMap<String, crate::signatures::FunctionSig>,
    out: &mut Vec<AliasingCallViolation>,
) {
    for stmt in &block.stmts {
        collect_aliasing_in_stmt(stmt, report, sigs, imported_sigs, out);
    }
}

fn collect_aliasing_in_stmt(
    stmt: &Stmt,
    report: &EffectiveOwnershipReport,
    sigs: &HashMap<String, crate::signatures::FunctionSig>,
    imported_sigs: &HashMap<String, crate::signatures::FunctionSig>,
    out: &mut Vec<AliasingCallViolation>,
) {
    match stmt {
        Stmt::Expr(e) => collect_aliasing_in_expr(e, report, sigs, imported_sigs, out),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_aliasing_in_expr(value, report, sigs, imported_sigs, out)
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                collect_aliasing_in_expr(v, report, sigs, imported_sigs, out);
            }
        }
        Stmt::FieldAssign { target, value, .. } => {
            collect_aliasing_in_expr(target, report, sigs, imported_sigs, out);
            collect_aliasing_in_expr(value, report, sigs, imported_sigs, out);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_aliasing_in_expr(receiver, report, sigs, imported_sigs, out);
            collect_aliasing_in_expr(index, report, sigs, imported_sigs, out);
            collect_aliasing_in_expr(value, report, sigs, imported_sigs, out);
        }
        Stmt::If { cond, body, .. } | Stmt::While { cond, body, .. } => {
            collect_aliasing_in_expr(cond, report, sigs, imported_sigs, out);
            collect_aliasing_in_block(body, report, sigs, imported_sigs, out);
        }
        Stmt::For { iter, body, .. } => {
            collect_aliasing_in_expr(iter, report, sigs, imported_sigs, out);
            collect_aliasing_in_block(body, report, sigs, imported_sigs, out);
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_aliasing_in_expr(scrutinee, report, sigs, imported_sigs, out);
            for arm in arms {
                collect_aliasing_in_block(&arm.body, report, sigs, imported_sigs, out);
            }
            if let Some(b) = else_arm {
                collect_aliasing_in_block(b, report, sigs, imported_sigs, out);
            }
        }
    }
}

fn collect_aliasing_in_expr(
    expr: &Expr,
    report: &EffectiveOwnershipReport,
    sigs: &HashMap<String, crate::signatures::FunctionSig>,
    imported_sigs: &HashMap<String, crate::signatures::FunctionSig>,
    out: &mut Vec<AliasingCallViolation>,
) {
    match expr {
        Expr::Call(c) => {
            if let Expr::Ident(name, _) = &c.callee {
                let arg_refs: Vec<&Expr> = c.args.iter().collect();
                check_call_args_for_aliasing(name, &arg_refs, report, sigs, imported_sigs, out);
            }
            collect_aliasing_in_expr(&c.callee, report, sigs, imported_sigs, out);
            for a in &c.args {
                collect_aliasing_in_expr(a, report, sigs, imported_sigs, out);
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            // UFCS: `value.f(a)` is `f(value, a)` — receiver is argument 0. Built-in
            // methods have no signature entry and are skipped inside the checker.
            let mut arg_refs: Vec<&Expr> = Vec::with_capacity(args.len() + 1);
            arg_refs.push(receiver);
            arg_refs.extend(args.iter());
            check_call_args_for_aliasing(method, &arg_refs, report, sigs, imported_sigs, out);
            collect_aliasing_in_expr(receiver, report, sigs, imported_sigs, out);
            for a in args {
                collect_aliasing_in_expr(a, report, sigs, imported_sigs, out);
            }
        }
        Expr::Background(inner, _) | Expr::Wait(inner, _) => {
            collect_aliasing_in_expr(inner, report, sigs, imported_sigs, out)
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_aliasing_in_expr(lhs, report, sigs, imported_sigs, out);
            collect_aliasing_in_expr(rhs, report, sigs, imported_sigs, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_aliasing_in_expr(operand, report, sigs, imported_sigs, out)
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_aliasing_in_expr(receiver, report, sigs, imported_sigs, out);
            collect_aliasing_in_expr(index, report, sigs, imported_sigs, out);
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_aliasing_in_expr(receiver, report, sigs, imported_sigs, out)
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_aliasing_in_expr(&f.value, report, sigs, imported_sigs, out);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_aliasing_in_expr(e, report, sigs, imported_sigs, out);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_aliasing_in_expr(k, report, sigs, imported_sigs, out);
                collect_aliasing_in_expr(v, report, sigs, imported_sigs, out);
            }
        }
        Expr::Is { expr: inner, .. } => {
            collect_aliasing_in_expr(inner, report, sigs, imported_sigs, out)
        }
        Expr::InterpolatedString(parts, _) => {
            for p in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                    collect_aliasing_in_expr(e, report, sigs, imported_sigs, out);
                }
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_aliasing_in_expr(receiver, report, sigs, imported_sigs, out)
        }
        Expr::Ident(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::StringLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => {}
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_module(src: &str) -> Module {
        let db = ynz_parser::CompilerDb::default();
        let sf = ynz_parser::SourceFile::new(&db, "test.ynz".to_string(), src.to_string());
        ynz_parser::parse_query(&db, sf).module.clone()
    }

    fn eo(src: &str, fn_name: &str, index: usize) -> EffectiveOwnership {
        let module = parse_module(src);
        analyze_for_test(&module).ownership_of(fn_name, index)
    }

    #[test]
    fn direct_field_write_is_writes() {
        // WHY: a bare parameter whose body directly assigns a field IS a write — the
        // compiler infers `lend` for it. This is the base case the fixpoint builds on.
        let src = r#"
shape Box { v: int }
function mutate(b: Box) -> nothing { b.v = 5 }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "mutate", 0), EffectiveOwnership::Writes);
    }

    #[test]
    fn only_read_is_reads() {
        // WHY: a parameter that is only read (field access, passed to print) must stay
        // Reads — this is what lets the independence side parallelize bare-read calls.
        let src = r#"
shape Box { v: int }
function look(b: Box) -> int { return b.v }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "look", 0), EffectiveOwnership::Reads);
    }

    #[test]
    fn transitive_through_bare_callee_is_writes() {
        // WHY (the headline soundness case): `fa(x)` passes `x` to `helper`, a bare-param
        // callee that mutates → helper's effective ownership is Writes → fa's `x` is Writes
        // transitively. This is the residual hole the fixpoint closes. The DECLARED modifier
        // of helper's param is None (bare) — only the effective fixpoint catches this.
        let src = r#"
shape Box { v: int }
function helper(b: Box) -> nothing { b.v = 999 }
function fa(x: Box) -> nothing { helper(x) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "helper", 0), EffectiveOwnership::Writes);
        assert_eq!(
            eo(src, "fa", 0),
            EffectiveOwnership::Writes,
            "fa's x flows into helper's mutating bare param — must be Writes transitively"
        );
    }

    #[test]
    fn two_hop_transitive_is_writes() {
        // WHY: the fixpoint must propagate across MORE than one hop. `outer → mid → inner`
        // where inner writes — every hop's param is Writes. A one-hop-only analysis would
        // miss `outer`.
        let src = r#"
shape Box { v: int }
function inner(b: Box) -> nothing { b.v = 1 }
function mid(b: Box) -> nothing { inner(b) }
function outer(b: Box) -> nothing { mid(b) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "inner", 0), EffectiveOwnership::Writes);
        assert_eq!(eo(src, "mid", 0), EffectiveOwnership::Writes);
        assert_eq!(eo(src, "outer", 0), EffectiveOwnership::Writes);
    }

    #[test]
    fn explicit_give_is_writes() {
        // WHY: a parameter declared `give` is a definite write (ownership transfer). The
        // declared-write seed must classify it Writes without needing a body escalation.
        let src = r#"
shape Box { v: int }
function consume(give b: Box) -> nothing { }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "consume", 0), EffectiveOwnership::Writes);
    }

    #[test]
    fn explicit_lend_is_writes() {
        // WHY: a parameter declared `lend` is a declared write — seeded Writes.
        let src = r#"
shape Box { v: int }
function bump(lend b: Box) -> nothing { b.v = b.v + 1 }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "bump", 0), EffectiveOwnership::Writes);
    }

    #[test]
    fn give_postfix_is_writes() {
        // WHY: `.give` consumes the value — even on a bare param, passing it via give to a
        // give-position consume escalates to Writes.
        let src = r#"
shape Box { v: int }
function consume(give b: Box) -> nothing { }
function passOn(b: Box) -> nothing { consume(b) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "passOn", 0),
            EffectiveOwnership::Writes,
            "passOn gives b to a give-position consume — Writes"
        );
    }

    #[test]
    fn method_receiver_to_lend_is_writes() {
        // WHY: UFCS — `b.bump()` desugars to `bump(b)` where bump declares `lend b`. The
        // receiver flows into a write-capable self position → Writes.
        let src = r#"
shape Box { v: int }
function bump(lend self: Box) -> nothing { self.v = self.v + 1 }
function caller(b: Box) -> nothing { b.bump() }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "caller", 0),
            EffectiveOwnership::Writes,
            "caller's b flows into bump's lend self via method-call syntax — Writes"
        );
    }

    #[test]
    fn read_only_method_receiver_is_reads() {
        // WHY: a method whose self is share/read leaves the receiver Reads — proves the
        // method path does NOT over-escalate.
        let src = r#"
shape Box { v: int }
function look(share self: Box) -> int { return self.v }
function caller(b: Box) -> int { return b.look() }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "caller", 0), EffectiveOwnership::Reads);
    }

    #[test]
    fn self_recursion_converges_and_classifies_writes() {
        // WHY: a self-recursive function that mutates its param must (a) terminate the
        // fixpoint and (b) classify Writes. Termination is proven by the test completing;
        // classification by the assertion.
        let src = r#"
shape Box { v: int }
function recur(b: Box, n: int) -> nothing {
  if (n) {
    b.v = n
    recur(b, 0)
  }
}
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "recur", 0),
            EffectiveOwnership::Writes,
            "self-recursive mutating param converges to Writes"
        );
    }

    #[test]
    fn mutual_recursion_converges() {
        // WHY: mutual recursion (ping → pong → ping) where one writes must converge. Both
        // bare params flow through each other; the writing path makes both Writes.
        let src = r#"
shape Box { v: int }
function ping(b: Box) -> nothing { pong(b) }
function pong(b: Box) -> nothing { b.v = 7 ping(b) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "pong", 0), EffectiveOwnership::Writes);
        assert_eq!(
            eo(src, "ping", 0),
            EffectiveOwnership::Writes,
            "ping flows into pong which writes — Writes transitively"
        );
    }

    #[test]
    fn passed_to_intrinsic_is_reads() {
        // WHY: a parameter passed only to an intrinsic (print) is read-only. Intrinsics
        // take args by value/share — they cannot be a hidden write. Must stay Reads so the
        // independence side can parallelize.
        let src = r#"
shape Box { v: int }
function show(b: Box) -> nothing { print(`${b.v.toString()}`) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(eo(src, "show", 0), EffectiveOwnership::Reads);
    }

    #[test]
    fn flow_into_imported_nondeclared_is_unknown() {
        // WHY (cross-module conservatism): a parameter that flows into an imported function
        // at a position that is NOT a declared lend/give cannot be proven read-only — it
        // must be Unknown, never Reads. This is the soundness fallback for the cross-module
        // boundary: independence sequentializes Unknown, typeck does not error.
        let module = parse_module(
            r#"
shape Box { v: int }
function fa(b: Box) -> nothing { remoteOp(b) }
function entrypoint() -> nothing { }
"#,
        );
        // remoteOp is imported (in this unit only by name); it declares no write positions.
        let imported: HashSet<String> = ["remoteOp"].iter().map(|s| s.to_string()).collect();
        let declared = declared_write_positions(&module);
        let report = analyze(&module, &declared, &imported);
        assert_eq!(
            report.ownership_of("fa", 0),
            EffectiveOwnership::Unknown,
            "flow into an imported non-declared position must be Unknown (conservative)"
        );
    }

    #[test]
    fn flow_into_imported_declared_lend_is_writes() {
        // WHY: an imported function whose declared modifier at the flowed-into position IS
        // lend/give is a definite write even without a body — must classify Writes.
        let module = parse_module(
            r#"
shape Box { v: int }
function fa(b: Box) -> nothing { remoteWrite(b) }
function entrypoint() -> nothing { }
"#,
        );
        let imported: HashSet<String> = ["remoteWrite"].iter().map(|s| s.to_string()).collect();
        let mut declared = declared_write_positions(&module);
        declared.insert("remoteWrite".to_string(), [0].into_iter().collect());
        let report = analyze(&module, &declared, &imported);
        assert_eq!(
            report.ownership_of("fa", 0),
            EffectiveOwnership::Writes,
            "flow into an imported declared-lend position is a definite write"
        );
    }

    #[test]
    fn unknown_dominated_by_writes_on_join() {
        // WHY: a parameter that flows into BOTH an unknown (imported) sink AND a definite
        // local write must be Writes — `Writes` dominates `Unknown` in the join. Locks the
        // lattice ordering at the consumer level.
        let module = parse_module(
            r#"
shape Box { v: int }
function localWrite(b: Box) -> nothing { b.v = 1 }
function fa(b: Box) -> nothing { remoteOp(b) localWrite(b) }
function entrypoint() -> nothing { }
"#,
        );
        let imported: HashSet<String> = ["remoteOp"].iter().map(|s| s.to_string()).collect();
        let declared = declared_write_positions(&module);
        let report = analyze(&module, &declared, &imported);
        assert_eq!(
            report.ownership_of("fa", 0),
            EffectiveOwnership::Writes,
            "Writes must dominate Unknown when a param flows into both"
        );
    }

    #[test]
    fn second_param_independent_classification() {
        // WHY: per-position tracking — a function with one read param and one write param
        // must classify each independently, not collapse to a single per-function verdict.
        let src = r#"
shape Box { v: int }
function mixed(reader: Box, writer: Box) -> nothing {
  let x: int = reader.v
  writer.v = x
}
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "mixed", 0),
            EffectiveOwnership::Reads,
            "reader param"
        );
        assert_eq!(
            eo(src, "mixed", 1),
            EffectiveOwnership::Writes,
            "writer param"
        );
    }

    #[test]
    fn self_param_transitive_write_is_writes() {
        // WHY (HOLE 1 — the `self`-blindness soundness case): the parser emits the `self`
        // keyword as `Expr::SelfValue`, not `Expr::Ident("self")`. A `share self` method that
        // flows `self` into a mutating callee must classify `self` as Writes — the same as the
        // IDENTICAL named-parameter form. Before the `arg_is_binding` fix, `self` slipped past
        // the param-match and stayed Reads, reopening the transitive share-violation hole and
        // letting the independence side parallelize an aliased write. Reverting the SelfValue
        // arm makes `self` classify Reads here.
        let src = r#"
shape Box { v: int }
function helper(b: Box) -> nothing { b.v = 999 }
function wrapper(self: Box) -> nothing { helper(self) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "wrapper", 0),
            EffectiveOwnership::Writes,
            "self flows into a mutating callee — must be Writes transitively, same as the named form"
        );
    }

    #[test]
    fn self_param_direct_field_write_is_writes() {
        // WHY (HOLE 1 — direct case): `self.v = n` roots in `self` (an `Expr::SelfValue`). The
        // `root_binding_name` SelfValue arm must root a direct field write through `self` to the
        // name "self" so the fixpoint classifies it Writes, identical to a named `b.v = n`.
        let src = r#"
shape Box { v: int }
function bump(self: Box) -> nothing { self.v = 1 }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "bump", 0),
            EffectiveOwnership::Writes,
            "direct field write through self must classify Writes"
        );
    }

    #[test]
    fn mutating_collection_method_on_param_is_writes() {
        // WHY (HOLE 2 — the silent-miscompile case): a builtin in-place collection mutator
        // (`array.add`, `array.set`) called on a param receiver writes that param. The fixpoint
        // MethodCall arm must classify the receiver Writes by NAME — `classify_call_position`
        // returns Reads for a builtin method (step 4), which is the false premise this carve-out
        // corrects. Without it, two aliased `.set()` calls would auto-parallelize into concurrent
        // in-place writes.
        let add_src = r#"
function grow(xs: array<int>) -> nothing { xs.add(7) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(add_src, "grow", 0),
            EffectiveOwnership::Writes,
            "xs.add(7) writes xs in place — must be Writes"
        );
        let set_src = r#"
function put(xs: array<int>) -> nothing { xs.set(0, 9) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(set_src, "put", 0),
            EffectiveOwnership::Writes,
            "xs.set(0, 9) writes xs in place — must be Writes"
        );
    }

    #[test]
    fn read_collection_method_on_param_is_reads() {
        // WHY (HOLE 2 — the read-method guard): a pure-named builtin collection method
        // (`array.get`, `array.count`) only reads its receiver (stdlib-design.md Rule 1). The
        // carve-out must NOT over-escalate these — they stay Reads so the independence side can
        // parallelize a pair of read-only collection accesses.
        let get_src = r#"
shape Box { v: int }
function peek(xs: array<int>) -> maybe<int> { return xs.get(0) }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(get_src, "peek", 0),
            EffectiveOwnership::Reads,
            "xs.get(0) only reads xs — must stay Reads"
        );
        let count_src = r#"
function size(xs: array<int>) -> int { return xs.count() }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(count_src, "size", 0),
            EffectiveOwnership::Reads,
            "xs.count() only reads xs — must stay Reads"
        );
    }

    #[test]
    fn user_method_named_like_mutator_uses_real_ownership() {
        // WHY (HOLE 2 — the false-positive guard): a USER-defined function named like a builtin
        // mutator (`function set(share self: MyShape)`) used via UFCS must use its REAL resolved
        // ownership, never the name-based mutating verdict. The `is_user_fn` ordering check in the
        // MethodCall arm makes the user function win: this read-only `set` keeps `caller`'s
        // receiver at Reads. If the name-based check fired unconditionally, this would wrongly
        // classify Writes and reject a legal read.
        let src = r#"
shape MyShape { v: int }
function set(share self: MyShape) -> int { return self.v }
function caller(s: MyShape) -> int { return s.set() }
function entrypoint() -> nothing { }
"#;
        assert_eq!(
            eo(src, "set", 0),
            EffectiveOwnership::Reads,
            "the user `set` only reads self — Reads"
        );
        assert_eq!(
            eo(src, "caller", 0),
            EffectiveOwnership::Reads,
            "caller's receiver flows into a read-only user `set` — must stay Reads, not the name-based Writes"
        );
    }

    #[test]
    fn dynamic_dispatch_contract_lend_is_writes() {
        // WHY (dynamic-dispatch boundary): a contract method declared `lend self` is a
        // write at the receiver position. The helper that maps a ReceiverKind to effective
        // ownership must report Writes for lend, Reads for share — this is how the
        // dynamic-dispatch path stays analyzable (the contract's DECLARED self modifier is
        // explicit per non-oop.md).
        assert_eq!(
            receiver_kind_ownership(&ReceiverKind::Lend),
            EffectiveOwnership::Writes
        );
        assert_eq!(
            receiver_kind_ownership(&ReceiverKind::Give),
            EffectiveOwnership::Writes
        );
        assert_eq!(
            receiver_kind_ownership(&ReceiverKind::Share),
            EffectiveOwnership::Reads
        );
    }
}
