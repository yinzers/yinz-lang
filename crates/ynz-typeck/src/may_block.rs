//! Transitive may-block analysis for v0.3-M2.
//!
//! # What this computes
//!
//! A function `suspends` if it can reach a suspension point: either it directly
//! calls a may-block intrinsic (`sleep`, `__testFallibleAsync`) or it calls
//! another function that `suspends`. The property propagates up the call graph to
//! a fixpoint.
//!
//! # Scope boundary
//!
//! Cross-module calls are resolved via `check_query` on the imported module —
//! the caller passes `imported_suspending_names` (the subset of imported fns
//! whose `check_query.suspends_set` includes them). These are treated as
//! suspension seeds: a local function that calls an imported-suspending fn is
//! itself seeded as suspending in the fixpoint, exactly like a direct intrinsic
//! call. Imported fns NOT in `imported_suspending_names` are non-suspending
//! leaves — no edge, no error.
//!
//! Dynamic dispatch through a `dynamic Contract` vtable remains
//! `UnresolvableEdge::DynamicDispatch` — static analysis cannot determine the
//! concrete callee at compile time. These still emit the conservative-correct
//! can't-infer diagnostic at the check.rs level.
//!
//! # Background edges are call-graph CUTS
//!
//! `background bar()` decouples the edge: the calling function fires-and-forgets
//! `bar` without awaiting it. A function whose only path to a may-block call is
//! through a `background` spawn never awaits a suspension point itself and is
//! therefore NOT marked `suspends`. The analysis treats `background`-wrapped calls
//! as opaque leaves (same as cross-module edges for propagation purposes).
//!
//! # Algorithm
//!
//! 1. Seed the `suspends` set with every function that directly calls a may-block
//!    intrinsic (classified via `suspension_source::is_base_suspension_intrinsic`).
//! 2. Iteratively mark a function `suspends` if it calls (non-background) any
//!    function already in the set.
//! 3. Repeat until no new entries are added (Kleene fixpoint over a finite set).
//!
//! Cycles / mutual recursion converge naturally: a cycle member is marked
//! `suspends` iff any member of the cycle reaches a may-block call. The fixpoint
//! terminates because the set only grows and is bounded by the function count.
//!
//! Time: O(F² · E) where F = function count, E = edges per function.
//! Space: O(F + E).

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Expr, Item, Module, Stmt};

use crate::suspension_source::is_base_suspension_intrinsic;

// ── Public types ──────────────────────────────────────────────────────────────

/// A call edge the analysis cannot resolve within this compilation unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvableEdge {
    /// Callee is defined in another module (cross-file, pre-M8 cross-unit analysis).
    CrossModule {
        /// The name as written at the call site.
        callee_name: String,
        /// Import source (e.g. `"./other"`) if we can tell, otherwise empty.
        import_src: String,
    },
    /// Call dispatched through a `dynamic Contract` vtable — static analysis cannot
    /// determine the concrete callee at compile time.
    DynamicDispatch {
        /// The receiver expression text (best-effort).
        receiver_hint: String,
    },
}

/// Result of the intra-unit transitive may-block analysis.
pub struct MayBlockAnalysis {
    /// Functions (by name) that transitively reach a suspension point.
    pub suspends: HashSet<String>,
    /// Call edges the analysis could not resolve.
    pub unresolvable: Vec<(String, UnresolvableEdge)>,
}

/// Run the transitive may-block fixpoint over the intra-unit module.
///
/// `imported_fn_names` is the set of all function names imported from other
/// files visible in this module — used to classify calls as "cross-module
/// non-suspending leaf" rather than "unknown name" (which typeck reports as
/// "not defined").
///
/// `imported_suspending_names` is the subset of `imported_fn_names` that are
/// known to transitively reach a suspension point (derived from each imported
/// module's `check_query.suspends_set`). A local function that calls one of
/// these is seeded as suspending in the fixpoint — same as calling an
/// intrinsic directly.
///
/// Time: O(F² · E)  Space: O(F + E)
pub fn analyze(
    module: &Module,
    imported_fn_names: &HashSet<String>,
    imported_suspending_names: &HashSet<String>,
) -> MayBlockAnalysis {
    // Step 1 — collect the intra-unit call graph.
    let graph = build_call_graph(module, imported_fn_names, imported_suspending_names);

    // Step 2 — seed from direct may-block intrinsic calls AND from calls to
    // imported-suspending functions (both are known suspension sources).
    let mut suspends: HashSet<String> = HashSet::new();
    for (fn_name, edges) in &graph.edges {
        if edges.calls_may_block_intrinsic {
            suspends.insert(fn_name.clone());
        }
    }

    // Step 3 — Kleene fixpoint: propagate `suspends` up the call graph.
    loop {
        let mut changed = false;
        for (fn_name, edges) in &graph.edges {
            if suspends.contains(fn_name) {
                continue;
            }
            // Non-background callee that is already in the suspends set.
            if edges.direct.iter().any(|callee| suspends.contains(callee)) {
                suspends.insert(fn_name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    MayBlockAnalysis {
        suspends,
        unresolvable: graph.unresolvable,
    }
}

/// Returns only the `suspends` set from the analysis.
///
/// Used by test code that wants to assert exact fixpoint membership without
/// building the full [`MayBlockAnalysis`].
pub fn suspends_set_for_test(
    module: &Module,
    imported_fn_names: &HashSet<String>,
) -> HashSet<String> {
    analyze(module, imported_fn_names, &HashSet::new()).suspends
}

/// Run the suspends fixpoint with an EXTRA set of suspension seeds beyond the
/// intrinsic-rooted seeds.
///
/// The v0.3-M3d CPU promotion pass uses this to compute the transitive
/// state-machine closure of a candidate promotion set: a promoted CPU function
/// becomes a state machine, so every (non-`background`) caller of it becomes a
/// state machine too — exactly the propagation a real `wait` would cause. Passing
/// the promoted set as `extra_seeds` yields `base_suspends ∪ closure(promoted)`,
/// which is the set of functions the guard-probe must check for decline.
///
/// `extra_seeds` are added to the seed set alongside the intrinsic-rooted seeds and
/// imported-suspending seeds; the same Kleene fixpoint then propagates all of them
/// up the call graph.
///
/// Time: O(F² · E)  Space: O(F + E)
pub fn suspends_with_extra_seeds(
    module: &Module,
    imported_fn_names: &HashSet<String>,
    imported_suspending_names: &HashSet<String>,
    extra_seeds: &HashSet<String>,
) -> HashSet<String> {
    let graph = build_call_graph(module, imported_fn_names, imported_suspending_names);

    let mut suspends: HashSet<String> = HashSet::new();
    for (fn_name, edges) in &graph.edges {
        if edges.calls_may_block_intrinsic || extra_seeds.contains(fn_name) {
            suspends.insert(fn_name.clone());
        }
    }

    loop {
        let mut changed = false;
        for (fn_name, edges) in &graph.edges {
            if suspends.contains(fn_name) {
                continue;
            }
            if edges.direct.iter().any(|callee| suspends.contains(callee)) {
                suspends.insert(fn_name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    suspends
}

/// Expose the direct (non-`background`) callee adjacency of the intra-unit call
/// graph: `fn_name → its direct callees`.
///
/// Used by the v0.3-M3d promotion rollback to walk which promoted candidates a
/// guard-violating function transitively reaches. Reuses the same `build_call_graph`
/// the suspends fixpoint uses, so the edge set is identical (background edges cut).
///
/// Time: O(F · E)  Space: O(F + E)
pub fn direct_callee_adjacency(
    module: &Module,
    imported_fn_names: &HashSet<String>,
) -> HashMap<String, Vec<String>> {
    let graph = build_call_graph(module, imported_fn_names, &HashSet::new());
    graph
        .edges
        .into_iter()
        .map(|(name, edges)| (name, edges.direct))
        .collect()
}

// The worth-it ("does real work") fixpoint is the auto-parallel proxy (Research Finding 9):
// spawning a CPU task onto
// the blocking pool is only worth its overhead when the callee transitively does
// non-trivial work. A trivial/leaf/arithmetic callee runs inline. The proxy for
// "does real work" is: the callee's call graph reaches a loop (`while`/`for`) or a
// recursion (self or mutual). The promotion query (v0.3-M3d Phase 3) consults this
// to gate which independent CPU statement pairs actually spawn.
//
// This is a SEPARATE property from `suspends`: a function can do real work without
// suspending (a pure compute loop) and can suspend without a loop (a single
// `wait sleep`). They ride the same call-graph walk but seed and propagate
// independently.

/// Run the transitive "does real work" fixpoint over the intra-unit module.
///
/// A function does real work iff it:
/// - directly contains a loop (`while`/`for`) anywhere in its own body, OR
/// - participates in recursion (self-recursion `f → f`, or a mutual cycle of size
///   ≥ 2 in the call graph), OR
/// - transitively calls a function that does real work (local OR an imported fn in
///   `imported_real_work_names`).
///
/// `imported_real_work_names` is the subset of imported fns known to do real work
/// (derived from each imported module's `check_query` result), seeded exactly like
/// `imported_suspending_names` in [`analyze`]. `background`-cut edges are NOT
/// followed (a fire-and-forget child's work is not the parent's work) — the same
/// graph-cut discipline `analyze` uses.
///
/// Time: O(F² · E)  Space: O(F + E)
pub fn does_real_work_set(
    module: &Module,
    imported_fn_names: &HashSet<String>,
    imported_real_work_names: &HashSet<String>,
) -> HashSet<String> {
    // The call graph already excludes `background`-cut edges and records direct
    // intra-unit callees — exactly the adjacency the propagation needs. Imported
    // suspending names are irrelevant to this property, so pass an empty set; the
    // edges built here carry only the call structure, not suspension seeds.
    let graph = build_call_graph(module, imported_fn_names, &HashSet::new());

    // Seed 1 — a function whose own body directly contains a loop.
    let mut does_real_work: HashSet<String> = HashSet::new();
    for item in &module.items {
        let Item::Function(f) = item else { continue };
        if block_contains_loop(&f.body.stmts) {
            does_real_work.insert(f.name.clone());
        }
    }

    // Seed 2 — a function that participates in recursion. Self-recursion (`f → f`)
    // and any strongly-connected component of size ≥ 2 both qualify: a cycle means
    // the work is unbounded-at-compile-time, which is exactly the heavy-work proxy.
    for member in recursive_members(&graph) {
        does_real_work.insert(member);
    }

    // Seed 3 — a function that directly calls an imported fn known to do real work.
    // Imported callees are NOT in `graph.edges[*].direct` (that holds only intra-unit
    // callees), so scan each function body for a direct ident call to an imported
    // real-work name. `background`-cut calls do not count (the parent doesn't perform
    // the child's work).
    if !imported_real_work_names.is_empty() {
        for item in &module.items {
            let Item::Function(f) = item else { continue };
            if block_calls_imported_name(&f.body.stmts, imported_real_work_names) {
                does_real_work.insert(f.name.clone());
            }
        }
    }

    // Kleene fixpoint — propagate `does_real_work` up the call graph: a function that
    // (non-background) calls a real-work function does real work too. Monotone over a
    // finite set → terminates.
    loop {
        let mut changed = false;
        for (fn_name, edges) in &graph.edges {
            if does_real_work.contains(fn_name) {
                continue;
            }
            if edges
                .direct
                .iter()
                .any(|callee| does_real_work.contains(callee))
            {
                does_real_work.insert(fn_name.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    does_real_work
}

/// True when any statement in `stmts` (recursively, through nested blocks and
/// expressions) makes a direct ident call to a name in `targets`, NOT through a
/// `background` cut. Used by the does-real-work Seed 3 to find local callers of an
/// imported real-work function (imported callees are not in the intra-unit edge set).
///
/// Cluster entry for the `block`/`stmt`/`expr_calls_imported_name` mutual recursion
/// over the AST.
///
/// Time: O(N) where N = AST nodes in `stmts`.  Space: O(D) where D = nesting depth.
fn block_calls_imported_name(stmts: &[Stmt], targets: &HashSet<String>) -> bool {
    stmts.iter().any(|s| stmt_calls_imported_name(s, targets))
}

/// Recursive sibling of [`block_calls_imported_name`] over a single statement.
///
/// Time: O(N) where N = AST nodes under `stmt`.  Space: O(D) where D = nesting depth.
fn stmt_calls_imported_name(stmt: &Stmt, targets: &HashSet<String>) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_calls_imported_name(e, targets),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            expr_calls_imported_name(value, targets)
        }
        Stmt::Return { value, .. } => value
            .as_ref()
            .map(|v| expr_calls_imported_name(v, targets))
            .unwrap_or(false),
        Stmt::FieldAssign { target, value, .. } => {
            expr_calls_imported_name(target, targets) || expr_calls_imported_name(value, targets)
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            expr_calls_imported_name(receiver, targets)
                || expr_calls_imported_name(index, targets)
                || expr_calls_imported_name(value, targets)
        }
        Stmt::If { cond, body, .. } => {
            expr_calls_imported_name(cond, targets)
                || block_calls_imported_name(&body.stmts, targets)
        }
        Stmt::While { cond, body, .. } => {
            expr_calls_imported_name(cond, targets)
                || block_calls_imported_name(&body.stmts, targets)
        }
        Stmt::For { iter, body, .. } => {
            expr_calls_imported_name(iter, targets)
                || block_calls_imported_name(&body.stmts, targets)
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_calls_imported_name(scrutinee, targets)
                || arms
                    .iter()
                    .any(|arm| block_calls_imported_name(&arm.body.stmts, targets))
                || else_arm
                    .as_ref()
                    .map(|b| block_calls_imported_name(&b.stmts, targets))
                    .unwrap_or(false)
        }
    }
}

/// Deepest sibling of the `block`/`stmt`/`expr_calls_imported_name` cluster —
/// walks an expression tree for any call to an imported target name.
///
/// Time: O(N) where N = expression nodes under `expr`.  Space: O(D) where D = nesting depth.
fn expr_calls_imported_name(expr: &Expr, targets: &HashSet<String>) -> bool {
    match expr {
        // `background f()` is a graph cut — the parent does not do the child's work.
        // Still recurse into the args (evaluated in the calling context).
        Expr::Background(inner, _) => {
            if let Expr::Call(c) = inner.as_ref() {
                c.args.iter().any(|a| expr_calls_imported_name(a, targets))
            } else {
                expr_calls_imported_name(inner, targets)
            }
        }
        Expr::Call(c) => {
            let direct_hit = matches!(&c.callee, Expr::Ident(name, _) if targets.contains(name));
            direct_hit
                || expr_calls_imported_name(&c.callee, targets)
                || c.args.iter().any(|a| expr_calls_imported_name(a, targets))
        }
        Expr::Wait(inner, _) => expr_calls_imported_name(inner, targets),
        Expr::BinOp { lhs, rhs, .. } => {
            expr_calls_imported_name(lhs, targets) || expr_calls_imported_name(rhs, targets)
        }
        Expr::UnaryOp { operand, .. } => expr_calls_imported_name(operand, targets),
        Expr::MethodCall { receiver, args, .. } => {
            expr_calls_imported_name(receiver, targets)
                || args.iter().any(|a| expr_calls_imported_name(a, targets))
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            expr_calls_imported_name(receiver, targets) || expr_calls_imported_name(index, targets)
        }
        Expr::FieldAccess { receiver, .. } => expr_calls_imported_name(receiver, targets),
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|f| expr_calls_imported_name(&f.value, targets)),
        Expr::ArrayLit { elements, .. } => elements
            .iter()
            .any(|e| expr_calls_imported_name(e, targets)),
        Expr::MapLit { entries, .. } => entries.iter().any(|(k, v)| {
            expr_calls_imported_name(k, targets) || expr_calls_imported_name(v, targets)
        }),
        Expr::Is { expr: inner, .. } => expr_calls_imported_name(inner, targets),
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| {
            if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                expr_calls_imported_name(e, targets)
            } else {
                false
            }
        }),
        Expr::PostfixOp { receiver, .. } => expr_calls_imported_name(receiver, targets),
        Expr::Ident(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::StringLit(..)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. }
        | Expr::Error(..) => false,
    }
}

/// True when a statement list directly contains a `while`/`for` loop, scanning
/// recursively through nested control-flow blocks (`if`/`match`/loop bodies) but
/// NOT into other function declarations (those have their own analysis entry).
///
/// Time: O(N) where N = AST nodes in `stmts`.  Space: O(D) where D = nesting depth.
fn block_contains_loop(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_contains_loop)
}

/// Recursive sibling of [`block_contains_loop`] over a single statement.
///
/// Time: O(N) where N = AST nodes under `stmt`.  Space: O(D) where D = nesting depth.
fn stmt_contains_loop(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::While { .. } | Stmt::For { .. } => true,
        Stmt::If { body, .. } => block_contains_loop(&body.stmts),
        Stmt::Match { arms, else_arm, .. } => {
            arms.iter().any(|arm| block_contains_loop(&arm.body.stmts))
                || else_arm
                    .as_ref()
                    .map(|b| block_contains_loop(&b.stmts))
                    .unwrap_or(false)
        }
        Stmt::Expr(_)
        | Stmt::Let { .. }
        | Stmt::Assign { .. }
        | Stmt::Return { .. }
        | Stmt::FieldAssign { .. }
        | Stmt::IndexAssign { .. } => false,
    }
}

/// Names of all functions that participate in recursion: self-recursive functions
/// (`f` appears in its own direct-callee set) and every member of a call-graph SCC
/// of size ≥ 2 (mutual recursion). Uses Kosaraju's two-pass iterative SCC algorithm
/// over the full intra-unit call graph.
///
/// Time: O(V + E)  Space: O(V + E)  where V = functions, E = call-graph edges
/// (Kosaraju: two linear DFS passes over the graph and its transpose).
fn recursive_members(graph: &CallGraph) -> HashSet<String> {
    let fns: Vec<&str> = graph.edges.keys().map(String::as_str).collect();
    let fn_index: HashMap<&str, usize> = fns.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let n = fns.len();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut radj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut self_recursive: HashSet<String> = HashSet::new();
    for (fn_name, edges) in &graph.edges {
        let Some(&fi) = fn_index.get(fn_name.as_str()) else {
            continue;
        };
        for callee in &edges.direct {
            if callee == fn_name {
                self_recursive.insert(fn_name.clone());
            }
            if let Some(&ci) = fn_index.get(callee.as_str()) {
                adj[fi].push(ci);
                radj[ci].push(fi);
            }
        }
    }

    // Kosaraju pass 1: forward DFS, collect finish order (iterative to avoid deep
    // recursion on large programs).
    let mut visited = vec![false; n];
    let mut finish_order: Vec<usize> = Vec::with_capacity(n);
    for start in 0..n {
        if visited[start] {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        visited[start] = true;
        while let Some((node, child_idx)) = stack.last_mut() {
            let node = *node;
            if *child_idx < adj[node].len() {
                let next = adj[node][*child_idx];
                *child_idx += 1;
                if !visited[next] {
                    visited[next] = true;
                    stack.push((next, 0));
                }
            } else {
                finish_order.push(node);
                stack.pop();
            }
        }
    }

    // Kosaraju pass 2: reverse DFS in reverse finish order → SCCs.
    let mut visited2 = vec![false; n];
    let mut members: HashSet<String> = self_recursive;
    for &start in finish_order.iter().rev() {
        if visited2[start] {
            continue;
        }
        let mut component: Vec<usize> = Vec::new();
        let mut stack: Vec<usize> = vec![start];
        visited2[start] = true;
        while let Some(node) = stack.pop() {
            component.push(node);
            for &nb in &radj[node] {
                if !visited2[nb] {
                    visited2[nb] = true;
                    stack.push(nb);
                }
            }
        }
        if component.len() >= 2 {
            for &i in &component {
                members.insert(fns[i].to_string());
            }
        }
    }
    members
}

// ── v0.3-M4: syntactic conduit-binding resolver (R6 sibling-arm consumer) ─────
//
// The may-block fixpoint runs BEFORE expression checking, so it has no `expr_types`. Conduit-ness
// (is this receiver a `channel<T>` or a background task handle?) is resolved here syntactically.
// The CLASSIFICATION (which method names suspend on a conduit) stays in the one authoritative
// home, `suspension_source::channel_method_suspends` — this resolver only answers "is this
// identifier a conduit binding?", which typeck's receiver restriction keeps equivalent to the
// exact type-keyed view (see suspension_source.rs docs).

/// True when the DECLARED AST type is `channel<T>` (any element type).
///
/// Shared with typeck's conduit-receiver origin discipline (`check_conduit_method_call`) so
/// both views seed channel-typed parameters from the same predicate.
pub(crate) fn ast_type_is_channel(ast_ty: &ynz_ast::nodes::Type) -> bool {
    matches!(ast_ty, ynz_ast::nodes::Type::Generic { name, .. } if name == "channel")
}

/// THE authoritative collection of local functions whose DECLARED return type is `channel<T>`.
///
/// Consumed by BOTH the syntactic conduit-binding resolver (via [`build_call_graph`]) and
/// typeck's conduit-receiver origin discipline — one collection, threaded to both, so the two
/// views can never disagree about which local calls produce a derivable channel binding
/// (authoritative-derivation.md).
pub(crate) fn collect_channel_returning_fns(module: &Module) -> HashSet<String> {
    module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(f) = item {
                if ast_type_is_channel(&f.return_type) {
                    return Some(f.name.clone());
                }
            }
            None
        })
        .collect()
}

/// THE authoritative binding-origin predicate: does `let <name>[: ty] = value` create a
/// syntactically-derivable conduit binding, given the conduit bindings already accumulated
/// in statement order?
///
/// Derivable origins (the complete list — anything else is NOT derivable):
/// - a `channel<T>` type annotation on the `let` (works for ANY right-hand side — a shape
///   field, a collection element, a cross-module call — because the annotation itself is the
///   syntactic evidence);
/// - `channel<T>(...)` construction;
/// - a call to a LOCAL function whose declared return type is `channel<T>`;
/// - `background f(...)` — a background task handle;
/// - a direct ident alias of an existing conduit binding.
///
/// Consumed by BOTH the resolver's `Stmt::Let` arm below and typeck's
/// `check_conduit_method_call` origin discipline. Typeck accepts a suspending conduit-method
/// call ONLY on receivers this predicate derived, which is what makes the resolver's view
/// equivalent-by-construction to typeck's acceptance (see `suspension_source.rs` docs) — a
/// second, hand-kept-in-sync copy of this match is the banned twin-derivation pattern
/// (authoritative-derivation.md).
pub(crate) fn let_binds_derivable_conduit(
    ty: Option<&ynz_ast::nodes::Type>,
    value: &Expr,
    conduits: &HashSet<String>,
    channel_returning_fns: &HashSet<String>,
) -> bool {
    ty.is_some_and(ast_type_is_channel)
        || match value {
            // `channel<T>(...)` construction.
            Expr::Call(c) => match &c.callee {
                Expr::Ident(n, _) if n == "channel" => true,
                // Local fn with a declared `channel<T>` return type.
                Expr::Ident(n, _) => channel_returning_fns.contains(n),
                _ => false,
            },
            // `let h = background f(...)` — a background task handle.
            Expr::Background(_, _) => true,
            // Direct ident alias of an existing conduit binding.
            Expr::Ident(n, _) => conduits.contains(n),
            _ => false,
        }
}

/// True when `f`'s body contains a suspending conduit-method call (`ch.send(v)`,
/// `ch.receive()`, `h.send(v)`, `h.receive()`) on a syntactically-derivable conduit binding.
///
/// Conduit bindings are exactly what [`let_binds_derivable_conduit`] derives (annotation,
/// construction, local channel-returning fn, `background` spawn, direct ident alias) plus
/// channel-annotated parameters.
///
/// Over-approximation is safe (an extra state machine is correct-but-slower).
/// Under-approximation is impossible BY CONSTRUCTION: typeck's receiver origin discipline
/// (`check_conduit_method_call`) accepts a suspending conduit-method call ONLY on a
/// plain-ident receiver whose binding THIS SAME predicate derived — a channel that reaches a
/// binding through a shape field, a collection element, a loop variable, or a cross-module
/// call is a teaching error at typeck unless the binding carries a `channel<T>` annotation
/// (which makes it derivable here too).
///
/// Time: O(N) where N = AST nodes in `f`  Space: O(B) conduit bindings + O(D) recursion depth
fn fn_contains_conduit_suspension(
    f: &ynz_ast::nodes::FunctionDecl,
    channel_returning_fns: &HashSet<String>,
) -> bool {
    let mut conduits: HashSet<String> = f
        .params
        .iter()
        .filter(|p| ast_type_is_channel(&p.ty))
        .map(|p| p.name.clone())
        .collect();
    stmts_contain_conduit_suspension(&f.body.stmts, &mut conduits, channel_returning_fns)
}

/// Recursive kernel for [`fn_contains_conduit_suspension`]: accumulates conduit bindings in
/// statement order and reports whether any suspending conduit-method call appears.
fn stmts_contain_conduit_suspension(
    stmts: &[Stmt],
    conduits: &mut HashSet<String>,
    channel_returning_fns: &HashSet<String>,
) -> bool {
    for stmt in stmts {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                if expr_has_conduit_suspension(value, conduits) {
                    return true;
                }
                // THE shared origin predicate — typeck's receiver origin discipline calls the
                // same function, so the two views accumulate identical conduit sets.
                let is_conduit = let_binds_derivable_conduit(
                    ty.as_ref(),
                    value,
                    conduits,
                    channel_returning_fns,
                );
                if is_conduit {
                    conduits.insert(name.clone());
                }
            }
            Stmt::Expr(e) | Stmt::Assign { value: e, .. } => {
                if expr_has_conduit_suspension(e, conduits) {
                    return true;
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    if expr_has_conduit_suspension(v, conduits) {
                        return true;
                    }
                }
            }
            Stmt::FieldAssign { target, value, .. } => {
                if expr_has_conduit_suspension(target, conduits)
                    || expr_has_conduit_suspension(value, conduits)
                {
                    return true;
                }
            }
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => {
                if expr_has_conduit_suspension(receiver, conduits)
                    || expr_has_conduit_suspension(index, conduits)
                    || expr_has_conduit_suspension(value, conduits)
                {
                    return true;
                }
            }
            Stmt::If { cond, body, .. } => {
                if expr_has_conduit_suspension(cond, conduits)
                    || stmts_contain_conduit_suspension(
                        &body.stmts,
                        conduits,
                        channel_returning_fns,
                    )
                {
                    return true;
                }
            }
            Stmt::While { cond, body, .. } => {
                if expr_has_conduit_suspension(cond, conduits)
                    || stmts_contain_conduit_suspension(
                        &body.stmts,
                        conduits,
                        channel_returning_fns,
                    )
                {
                    return true;
                }
            }
            Stmt::For { iter, body, .. } => {
                if expr_has_conduit_suspension(iter, conduits)
                    || stmts_contain_conduit_suspension(
                        &body.stmts,
                        conduits,
                        channel_returning_fns,
                    )
                {
                    return true;
                }
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                if expr_has_conduit_suspension(scrutinee, conduits) {
                    return true;
                }
                for arm in arms {
                    if stmts_contain_conduit_suspension(
                        &arm.body.stmts,
                        conduits,
                        channel_returning_fns,
                    ) {
                        return true;
                    }
                }
                if let Some(eb) = else_arm {
                    if stmts_contain_conduit_suspension(&eb.stmts, conduits, channel_returning_fns)
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// True when `expr` contains a suspending conduit-method call at any depth.
fn expr_has_conduit_suspension(expr: &Expr, conduits: &HashSet<String>) -> bool {
    match expr {
        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let receiver_is_conduit =
                matches!(receiver.as_ref(), Expr::Ident(n, _) if conduits.contains(n));
            if crate::suspension_source::channel_method_suspends(receiver_is_conduit, method) {
                return true;
            }
            expr_has_conduit_suspension(receiver, conduits)
                || args
                    .iter()
                    .any(|a| expr_has_conduit_suspension(a, conduits))
        }
        Expr::Wait(inner, _) => expr_has_conduit_suspension(inner, conduits),
        // `background f(args)`: the callee's body is a graph cut, but the ARGS are evaluated in
        // the calling context — recurse into them only.
        Expr::Background(inner, _) => match inner.as_ref() {
            Expr::Call(c) => c
                .args
                .iter()
                .any(|a| expr_has_conduit_suspension(a, conduits)),
            other => expr_has_conduit_suspension(other, conduits),
        },
        Expr::Call(c) => {
            expr_has_conduit_suspension(&c.callee, conduits)
                || c.args
                    .iter()
                    .any(|a| expr_has_conduit_suspension(a, conduits))
        }
        Expr::BinOp { lhs, rhs, .. } => {
            expr_has_conduit_suspension(lhs, conduits) || expr_has_conduit_suspension(rhs, conduits)
        }
        Expr::UnaryOp { operand, .. } => expr_has_conduit_suspension(operand, conduits),
        Expr::FieldAccess { receiver, .. } => expr_has_conduit_suspension(receiver, conduits),
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            expr_has_conduit_suspension(receiver, conduits)
                || expr_has_conduit_suspension(index, conduits)
        }
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|f| expr_has_conduit_suspension(&f.value, conduits)),
        Expr::ArrayLit { elements, .. } => elements
            .iter()
            .any(|e| expr_has_conduit_suspension(e, conduits)),
        Expr::MapLit { entries, .. } => entries.iter().any(|(k, v)| {
            expr_has_conduit_suspension(k, conduits) || expr_has_conduit_suspension(v, conduits)
        }),
        Expr::PostfixOp { receiver, .. } => expr_has_conduit_suspension(receiver, conduits),
        Expr::Is { expr: inner, .. } => expr_has_conduit_suspension(inner, conduits),
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| {
            if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                expr_has_conduit_suspension(e, conduits)
            } else {
                false
            }
        }),
        _ => false,
    }
}

// ── Internal call-graph types ─────────────────────────────────────────────────

struct CallGraph {
    /// Adjacency: fn_name → direct non-background callees within the module.
    edges: HashMap<String, FnEdges>,
    /// Edges that could not be resolved.
    unresolvable: Vec<(String, UnresolvableEdge)>,
}

struct FnEdges {
    /// Non-background callees visible in this compilation unit.
    ///
    /// Contains only user-defined function names (intra-unit callees). May-block
    /// intrinsics are tracked separately via `calls_may_block_intrinsic` so that
    /// the seed step is a boolean check, not a name scan. Keeping them separate
    /// also prevents a user function named `sleep` from false-positiving
    /// into the seed set via name collision.
    direct: Vec<String>,
    /// True when this function makes a non-background call to a may-block
    /// intrinsic (`sleep`, `__testFallibleAsync`).
    calls_may_block_intrinsic: bool,
}

fn build_call_graph(
    module: &Module,
    imported_fn_names: &HashSet<String>,
    imported_suspending_names: &HashSet<String>,
) -> CallGraph {
    // Collect all local function names so we know which calls are cross-module.
    let local_fns: HashSet<String> = module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(f) = item {
                Some(f.name.clone())
            } else {
                None
            }
        })
        .collect();

    let mut edges: HashMap<String, FnEdges> = HashMap::new();
    let mut unresolvable: Vec<(String, UnresolvableEdge)> = Vec::new();

    // v0.3-M4 (R6 sibling arm): local functions whose DECLARED return type is `channel<T>`.
    // Used by the syntactic conduit-binding resolver so `let ch = makeChannel()` marks `ch`
    // as a channel binding. Shared collection with typeck's origin discipline.
    let channel_returning_fns = collect_channel_returning_fns(module);

    for item in &module.items {
        let Item::Function(f) = item else { continue };

        let mut fn_edges = FnEdges {
            direct: Vec::new(),
            calls_may_block_intrinsic: false,
        };
        collect_calls_in_block(
            &f.body.stmts,
            &local_fns,
            imported_fn_names,
            imported_suspending_names,
            &f.name,
            &mut fn_edges,
            &mut unresolvable,
        );
        // v0.3-M4 (R6 sibling arm): a channel/handle suspending method call (`ch.send(v)`,
        // `ch.receive()`, `h.receive()`, …) is a base suspension source, classified by the ONE
        // authoritative `suspension_source::channel_method_suspends`. This fixpoint runs before
        // expression checking (no `expr_types`), so conduit-ness of the receiver is resolved
        // syntactically here; typeck's receiver restriction (plain-ident, syntactically-derivable
        // conduit binding) keeps the two views equivalent — see the classifier's module docs.
        if !fn_edges.calls_may_block_intrinsic
            && fn_contains_conduit_suspension(f, &channel_returning_fns)
        {
            fn_edges.calls_may_block_intrinsic = true;
        }
        edges.insert(f.name.clone(), fn_edges);
    }

    CallGraph {
        edges,
        unresolvable,
    }
}

fn collect_calls_in_block(
    stmts: &[Stmt],
    local_fns: &HashSet<String>,
    imported_fns: &HashSet<String>,
    imported_suspending: &HashSet<String>,
    enclosing_fn: &str,
    edges: &mut FnEdges,
    unresolvable: &mut Vec<(String, UnresolvableEdge)>,
) {
    for stmt in stmts {
        collect_calls_in_stmt(
            stmt,
            local_fns,
            imported_fns,
            imported_suspending,
            enclosing_fn,
            edges,
            unresolvable,
        );
    }
}

fn collect_calls_in_stmt(
    stmt: &Stmt,
    local_fns: &HashSet<String>,
    imported_fns: &HashSet<String>,
    imported_suspending: &HashSet<String>,
    enclosing_fn: &str,
    edges: &mut FnEdges,
    unresolvable: &mut Vec<(String, UnresolvableEdge)>,
) {
    match stmt {
        Stmt::Expr(e) => {
            collect_calls_in_expr(
                e,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_calls_in_expr(
                value,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                collect_calls_in_expr(
                    v,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
            }
        }
        Stmt::FieldAssign { target, value, .. } => {
            collect_calls_in_expr(
                target,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                value,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_calls_in_expr(
                receiver,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                index,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                value,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Stmt::If { cond, body, .. } => {
            collect_calls_in_expr(
                cond,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_block(
                &body.stmts,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
            );
        }
        Stmt::While { cond, body, .. } => {
            collect_calls_in_expr(
                cond,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_block(
                &body.stmts,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
            );
        }
        Stmt::For { iter, body, .. } => {
            collect_calls_in_expr(
                iter,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_block(
                &body.stmts,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
            );
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_calls_in_expr(
                scrutinee,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            for arm in arms {
                collect_calls_in_block(
                    &arm.body.stmts,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                );
            }
            if let Some(eb) = else_arm {
                collect_calls_in_block(
                    &eb.stmts,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                );
            }
        }
    }
}

// All 8 params are genuinely independent state for a recursive AST tree-walk;
// bundling into a struct would add a type used nowhere else in the crate.
// `enclosing_fn` and `unresolvable` are thread-through params in the recursive
// protocol — used by the callee's recursive arms and reserved for future arms.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::only_used_in_recursion)]
fn collect_calls_in_expr(
    expr: &Expr,
    local_fns: &HashSet<String>,
    imported_fns: &HashSet<String>,
    imported_suspending: &HashSet<String>,
    enclosing_fn: &str,
    edges: &mut FnEdges,
    unresolvable: &mut Vec<(String, UnresolvableEdge)>,
    // True when this expression is the immediate inner of `background` — these
    // calls are graph cuts (the calling function does not await them).
    is_background_call: bool,
) {
    match expr {
        Expr::Background(inner, _) => {
            // The direct callee of `background` is a graph cut. Recurse into its
            // arguments normally (they are evaluated in the calling context).
            collect_calls_in_expr(
                inner,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                true,
            );
        }
        Expr::Call(call) => {
            // Determine callee name.
            let callee_name = match &call.callee {
                Expr::Ident(name, _) => Some(name.clone()),
                _ => None,
            };

            if let Some(name) = callee_name {
                if !is_background_call {
                    if local_fns.contains(&name) {
                        // Intra-unit user-defined function — add a propagation edge.
                        // User-defined fns shadow intrinsics of the same name, so this branch
                        // must come BEFORE the intrinsic check. A function named `sleep`
                        // that does not itself call the intrinsic is NOT a suspension source.
                        if !edges.direct.contains(&name) {
                            edges.direct.push(name.clone());
                        }
                    } else if is_base_suspension_intrinsic(&name) {
                        // May-block intrinsic (not shadowed by a local fn of the same name) —
                        // record the boolean flag. This avoids putting intrinsic names into
                        // `direct`, which would cause a user function named `sleep` to
                        // false-positive via name collision in the seed step.
                        edges.calls_may_block_intrinsic = true;
                    } else if imported_fns.contains(&name) && imported_suspending.contains(&name) {
                        // Imported fn with known suspension status: suspends.
                        // Treat it like a direct intrinsic call — the enclosing
                        // function transitively reaches a suspension point.
                        // Non-suspending imported fns: non-suspending leaf, no edge needed.
                        edges.calls_may_block_intrinsic = true;
                    }
                    // else: unknown name — normal typeck will emit "not defined" diagnostic
                }
                // Recurse into arguments regardless (args are evaluated in calling context).
                for arg in &call.args {
                    collect_calls_in_expr(
                        arg,
                        local_fns,
                        imported_fns,
                        imported_suspending,
                        enclosing_fn,
                        edges,
                        unresolvable,
                        false,
                    );
                }
                // Callee expression (e.g. for function-value calls, rare in current Yinz)
                collect_calls_in_expr(
                    &call.callee,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
            } else {
                // Non-ident callee expression — recurse.
                collect_calls_in_expr(
                    &call.callee,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
                for arg in &call.args {
                    collect_calls_in_expr(
                        arg,
                        local_fns,
                        imported_fns,
                        imported_suspending,
                        enclosing_fn,
                        edges,
                        unresolvable,
                        false,
                    );
                }
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            // MethodCall — dynamic dispatch detection. If the receiver has been
            // typed as a `dynamic Contract`, we would need type information not
            // available here. At the AST-walk level we conservatively recurse —
            // can't-infer errors for `dynamic` dispatch are emitted at the typeck
            // level (check.rs) where receiver type is known.
            collect_calls_in_expr(
                receiver,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            for arg in args {
                collect_calls_in_expr(
                    arg,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
            }
        }
        Expr::Wait(inner, _) => {
            collect_calls_in_expr(
                inner,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_calls_in_expr(
                lhs,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                rhs,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Expr::UnaryOp { operand, .. } => {
            collect_calls_in_expr(
                operand,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_calls_in_expr(
                receiver,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                index,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_calls_in_expr(
                receiver,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_calls_in_expr(
                    &f.value,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_calls_in_expr(
                    e,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_calls_in_expr(
                    k,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
                collect_calls_in_expr(
                    v,
                    local_fns,
                    imported_fns,
                    imported_suspending,
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
            }
        }
        Expr::Is { expr: inner, .. } => {
            collect_calls_in_expr(
                inner,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_calls_in_expr(
                        e,
                        local_fns,
                        imported_fns,
                        imported_suspending,
                        enclosing_fn,
                        edges,
                        unresolvable,
                        false,
                    );
                }
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_calls_in_expr(
                receiver,
                local_fns,
                imported_fns,
                imported_suspending,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
        }
        // Leaf nodes — no calls inside.
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

// ── Mutual-suspension cycle detection ────────────────────────────────────────
//
// v0.3-M2 supports SELF-recursion among suspending functions (f → f). The Drop
// impl on SpawnStateFnFuture walks the recursion chain assuming all frames have
// the same layout (same function = same frame size + recursion slot offset). A
// cycle with ≥2 DISTINCT suspending functions has mixed layouts → heap corruption
// on cancellation. Detecting and rejecting this at typeck prevents the corruption.
//
// Self-recursion (f → f, SCC of size 1) is NOT a cycle in this sense — it
// remains supported. Only SCCs of size ≥ 2 in the `suspends` subgraph are errors.
//
// v0.3-M3 lifts this restriction by adding per-frame size+offset metadata so the
// Drop walk can handle mixed layouts.

/// A non-self mutual suspension cycle: ≥2 distinct suspending functions that
/// form a call cycle. The Drop walk in `SpawnStateFnFuture` assumes uniform
/// frame layout (self-recursion), so mixed-layout mutual cycles corrupt memory.
pub struct MutualSuspensionCycle {
    /// The participating function names (sorted for deterministic diagnostics).
    pub members: Vec<String>,
}

/// Find all SCCs of size ≥ 2 in the call graph restricted to the `suspends` set.
///
/// Returns one entry per strongly-connected component that has ≥2 distinct
/// members, all of which are suspending functions. Each entry's `members` list
/// is sorted alphabetically for deterministic diagnostic output.
///
/// Uses iterative DFS (Kosaraju's two-pass algorithm) to avoid stack overflow
/// on large programs.
pub fn find_mutual_suspension_cycles(
    module: &Module,
    imported_fn_names: &HashSet<String>,
    suspends: &HashSet<String>,
) -> Vec<MutualSuspensionCycle> {
    if suspends.len() < 2 {
        return Vec::new();
    }

    // Build the restricted call graph: edges only among suspending functions.
    // Imported suspending names are not needed here — the mutual-cycle check only
    // considers local fns (cross-module cycle detection is out of scope for M3b).
    let graph = build_call_graph(module, imported_fn_names, &HashSet::new());
    let fns: Vec<&str> = suspends.iter().map(|s| s.as_str()).collect();
    let fn_index: HashMap<&str, usize> = fns.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let n = fns.len();

    // Adjacency lists restricted to the suspends set.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut radj: Vec<Vec<usize>> = vec![Vec::new(); n]; // reverse graph
    for (fn_name, edges) in &graph.edges {
        let Some(&fi) = fn_index.get(fn_name.as_str()) else {
            continue;
        };
        for callee in &edges.direct {
            if let Some(&ci) = fn_index.get(callee.as_str()) {
                adj[fi].push(ci);
                radj[ci].push(fi);
            }
        }
    }

    // Kosaraju pass 1: DFS on forward graph, collect finish order.
    let mut visited = vec![false; n];
    let mut finish_order: Vec<usize> = Vec::with_capacity(n);
    for start in 0..n {
        if !visited[start] {
            // Iterative DFS with explicit stack to avoid recursion depth limits.
            let mut stack: Vec<(usize, usize)> = vec![(start, 0)]; // (node, next_child_idx)
            visited[start] = true;
            while let Some((node, child_idx)) = stack.last_mut() {
                let node = *node;
                if *child_idx < adj[node].len() {
                    let next = adj[node][*child_idx];
                    *child_idx += 1;
                    if !visited[next] {
                        visited[next] = true;
                        stack.push((next, 0));
                    }
                } else {
                    finish_order.push(node);
                    stack.pop();
                }
            }
        }
    }

    // Kosaraju pass 2: DFS on reverse graph in reverse finish order → SCCs.
    let mut visited2 = vec![false; n];
    let mut cycles = Vec::new();
    for &start in finish_order.iter().rev() {
        if visited2[start] {
            continue;
        }
        let mut component: Vec<usize> = Vec::new();
        let mut stack: Vec<usize> = vec![start];
        visited2[start] = true;
        while let Some(node) = stack.pop() {
            component.push(node);
            for &nb in &radj[node] {
                if !visited2[nb] {
                    visited2[nb] = true;
                    stack.push(nb);
                }
            }
        }
        if component.len() >= 2 {
            let mut members: Vec<String> = component.iter().map(|&i| fns[i].to_string()).collect();
            members.sort(); // deterministic output
            cycles.push(MutualSuspensionCycle { members });
        }
    }
    cycles
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE: &str = "test.ynz";

    fn parse_module(src: &str) -> ynz_ast::nodes::Module {
        let db = ynz_parser::CompilerDb::default();
        let sf = ynz_parser::SourceFile::new(&db, TEST_FILE.to_string(), src.to_string());
        ynz_parser::parse_query(&db, sf).module.clone()
    }

    fn suspends_set(src: &str) -> HashSet<String> {
        let module = parse_module(src);
        analyze(&module, &HashSet::new(), &HashSet::new()).suspends
    }

    #[test]
    fn pure_cpu_fn_not_suspends() {
        let suspends = suspends_set(
            r#"
function entrypoint() -> nothing {
    print("hello")
}
"#,
        );
        assert!(
            !suspends.contains("entrypoint"),
            "pure CPU fn must NOT be suspends"
        );
    }

    #[test]
    fn direct_sleep_async_call_suspends() {
        let suspends = suspends_set(
            r#"
function pause() -> nothing {
    wait sleep(100)
}
function entrypoint() -> nothing { }
"#,
        );
        assert!(
            suspends.contains("pause"),
            "direct sleep caller must be suspends"
        );
        assert!(
            !suspends.contains("entrypoint"),
            "unrelated fn must NOT be suspends"
        );
    }

    #[test]
    fn transitive_outer_inner_sleep_async_all_suspends() {
        let suspends = suspends_set(
            r#"
function inner() -> nothing {
    wait sleep(50)
}
function outer() -> nothing {
    inner()
}
function entrypoint() -> nothing {
    outer()
}
"#,
        );
        // All three are in the transitive chain.
        assert!(
            suspends.contains("inner"),
            "inner (direct) must be suspends"
        );
        assert!(
            suspends.contains("outer"),
            "outer (transitive) must be suspends"
        );
        assert!(
            suspends.contains("entrypoint"),
            "entrypoint (transitive) must be suspends"
        );
    }

    #[test]
    fn background_decouples_propagation() {
        // A function whose ONLY path to `sleep` is through `background`
        // does not itself suspend — it fires and forgets.
        let suspends = suspends_set(
            r#"
function worker() -> nothing {
    wait sleep(100)
}
function launcher() -> nothing {
    background worker()
}
function entrypoint() -> nothing { }
"#,
        );
        assert!(
            suspends.contains("worker"),
            "worker (direct) must be suspends"
        );
        assert!(
            !suspends.contains("launcher"),
            "launcher (background-only) must NOT be suspends"
        );
        assert!(!suspends.contains("entrypoint"));
    }

    #[test]
    fn cyclic_call_graph_converges() {
        // Mutual recursion: a() calls b(), b() calls a(). Neither reaches `sleep`.
        let suspends = suspends_set(
            r#"
function a() -> nothing { b() }
function b() -> nothing { a() }
function entrypoint() -> nothing { }
"#,
        );
        assert!(
            !suspends.contains("a"),
            "a (cycle, no may-block) must NOT be suspends"
        );
        assert!(
            !suspends.contains("b"),
            "b (cycle, no may-block) must NOT be suspends"
        );

        // Mutual recursion where one reaches `sleep` — both should be suspends.
        let suspends2 = suspends_set(
            r#"
function a() -> nothing { b() }
function b() -> nothing { wait sleep(10); a() }
function entrypoint() -> nothing { }
"#,
        );
        assert!(suspends2.contains("b"), "b (direct) must be suspends");
        assert!(
            suspends2.contains("a"),
            "a (cycle-member via b) must be suspends"
        );
    }

    #[test]
    fn imported_non_suspending_fn_produces_no_unresolvable_and_no_suspension() {
        // WHY: an imported fn with known-non-suspending status (in imported_fn_names but
        // NOT in imported_suspending_names) must be treated as a non-suspending leaf.
        // No CrossModule unresolvable edge, and the caller must NOT be in suspends_set.
        // Guards regressions where non-suspending cross-module calls incorrectly poison
        // the caller's suspension analysis (false-positive state machine classification).
        let module = parse_module(
            r#"
function entrypoint() -> nothing {
    remoteOp()
}
"#,
        );
        let imported: HashSet<String> = ["remoteOp"].iter().map(|s| s.to_string()).collect();
        let result = analyze(&module, &imported, &HashSet::new());
        assert!(
            result.unresolvable.is_empty(),
            "known-non-suspending imported callee must NOT produce any unresolvable edge; got: {:#?}",
            result.unresolvable
        );
        assert!(
            !result.suspends.contains("entrypoint"),
            "caller of non-suspending imported fn must NOT be in suspends_set; got: {:?}",
            result.suspends
        );
    }

    #[test]
    fn imported_suspending_fn_seeds_caller_suspension() {
        // WHY: an imported fn with known-suspending status (in BOTH imported_fn_names
        // AND imported_suspending_names) must seed the caller as suspending — same effect
        // as a direct sleep call. Guards regressions where cross-module suspension fails
        // to propagate (caller left non-suspending, compiles as straight-line code, panics
        // at runtime trying to run a state machine outer wrapper from inside a Tokio runtime).
        let module = parse_module(
            r#"
function entrypoint() -> nothing {
    slowOp()
}
"#,
        );
        let imported: HashSet<String> = ["slowOp"].iter().map(|s| s.to_string()).collect();
        let imported_suspending: HashSet<String> =
            ["slowOp"].iter().map(|s| s.to_string()).collect();
        let result = analyze(&module, &imported, &imported_suspending);
        assert!(
            result.suspends.contains("entrypoint"),
            "caller of imported suspending fn must be in suspends_set; got: {:?}",
            result.suspends
        );
        assert!(
            result.unresolvable.is_empty(),
            "known-suspending imported fn must NOT produce an unresolvable edge; got: {:#?}",
            result.unresolvable
        );
    }

    #[test]
    fn background_args_evaluated_in_calling_context() {
        // Even though the background call is a graph cut, its ARGUMENTS are evaluated
        // in the calling context. If an argument itself calls a suspending fn, the
        // caller IS suspends (the arg evaluation happens before the background launch).
        let suspends = suspends_set(
            r#"
function worker(n: int) -> nothing {
    wait sleep(n)
}
function compute() -> int {
    return 5
}
function launcher() -> nothing {
    background worker(compute())
}
function entrypoint() -> nothing { }
"#,
        );
        // compute() is pure CPU; worker() suspends; launcher() backgrounds worker —
        // but compute() in the arg is intra-context. launcher() itself does NOT suspend
        // because compute() is pure.
        assert!(suspends.contains("worker"), "worker must be suspends");
        assert!(
            !suspends.contains("compute"),
            "compute must NOT be suspends (pure CPU)"
        );
        assert!(
            !suspends.contains("launcher"),
            "launcher must NOT be suspends (background-only path)"
        );
    }

    #[test]
    fn wait_inside_if_is_still_transitive() {
        // A function with `wait` nested inside an `if` is still suspends.
        let suspends = suspends_set(
            r#"
function maybePause(flag: bool) -> nothing {
    if (flag) {
        wait sleep(50)
    }
}
function caller() -> nothing {
    maybePause(true)
}
function entrypoint() -> nothing { }
"#,
        );
        assert!(
            suspends.contains("maybePause"),
            "wait inside if must still be suspends"
        );
        assert!(
            suspends.contains("caller"),
            "transitive via maybePause must be suspends"
        );
    }

    #[test]
    fn test_fallible_async_seeds_suspends() {
        let suspends = suspends_set(
            r#"
function fetchResult() -> int {
    return 0
}
function entrypoint() -> nothing {
    wait __testFallibleAsync(true)
}
"#,
        );
        assert!(
            suspends.contains("entrypoint"),
            "__testFallibleAsync must seed suspends"
        );
    }

    #[test]
    fn user_fn_named_sleep_is_not_suspends_without_intrinsic_call() {
        // WHY: guards against the name-collision class where a user function named
        // exactly `sleep` would false-positive into the seed set because the
        // old implementation pushed intrinsic names into `direct` and the seed step
        // scanned `direct` for intrinsic names. With `calls_may_block_intrinsic: bool`,
        // only a real intrinsic CALL sets the flag — a function whose NAME matches an
        // intrinsic but whose body calls nothing does not.
        let suspends = suspends_set(
            r#"
function sleep(ms: int) -> nothing {
    print(`this is a user fn, not the intrinsic`)
}
function entrypoint() -> nothing {
    sleep(100)
}
"#,
        );
        assert!(
            !suspends.contains("sleep"),
            "user fn named sleep without an intrinsic call must NOT be suspends"
        );
        assert!(
            !suspends.contains("entrypoint"),
            "caller of user-fn-named-sleep without intrinsic must NOT be suspends"
        );
    }

    fn real_work_set(src: &str) -> HashSet<String> {
        let module = parse_module(src);
        does_real_work_set(&module, &HashSet::new(), &HashSet::new())
    }

    #[test]
    fn leaf_arithmetic_fn_does_no_real_work() {
        // WHY: a trivial leaf with no loop and no recursion is NOT worth spawning —
        // the worth-it proxy must classify it as does-no-real-work so the promotion
        // query never parallelizes calls to it.
        let work = real_work_set(
            r#"
function add(x: int) -> int { return x + 1 }
function entrypoint() -> nothing { print("hi") }
"#,
        );
        assert!(
            !work.contains("add"),
            "leaf arithmetic fn must NOT do real work"
        );
        assert!(!work.contains("entrypoint"));
    }

    #[test]
    fn loop_containing_fn_does_real_work() {
        // WHY: a `while`/`for` loop is the worth-it proxy's primary signal.
        let work = real_work_set(
            r#"
function sumTo(n: int) -> int {
    let total = 0
    let i = 0
    while (i < n) {
        total = total + i
        i = i + 1
    }
    return total
}
function entrypoint() -> nothing { print("hi") }
"#,
        );
        assert!(
            work.contains("sumTo"),
            "loop-containing fn must do real work"
        );
    }

    #[test]
    fn self_recursive_fn_does_real_work() {
        // WHY: self-recursion (fib-style) is unbounded-at-compile-time work — the
        // headline CPU-parallel callee shape.
        let work = real_work_set(
            r#"
function fib(n: int) -> int {
    if (n < 2) { return n }
    let a = fib(n - 1)
    let b = fib(n - 2)
    return a + b
}
function entrypoint() -> nothing { print("hi") }
"#,
        );
        assert!(work.contains("fib"), "self-recursive fn must do real work");
    }

    #[test]
    fn mutual_recursion_does_real_work() {
        // WHY: a mutual cycle (isEven/isOdd) is recursion too — both members do work.
        let work = real_work_set(
            r#"
function isEven(n: int) -> bool {
    if (n == 0) { return true }
    return isOdd(n - 1)
}
function isOdd(n: int) -> bool {
    if (n == 0) { return false }
    return isEven(n - 1)
}
function entrypoint() -> nothing { print("hi") }
"#,
        );
        assert!(
            work.contains("isEven"),
            "mutual-cycle member must do real work"
        );
        assert!(
            work.contains("isOdd"),
            "mutual-cycle member must do real work"
        );
    }

    #[test]
    fn real_work_propagates_transitively_up_callers() {
        // WHY: a caller of a real-work function does real work too (the work happens
        // inside the call) — needed so a thin wrapper around `fib` is still worth it.
        let work = real_work_set(
            r#"
function fib(n: int) -> int {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}
function wrapper(n: int) -> int { return fib(n) }
function entrypoint() -> nothing { print("hi") }
"#,
        );
        assert!(work.contains("fib"));
        assert!(
            work.contains("wrapper"),
            "caller of real-work fn must do real work"
        );
    }

    #[test]
    fn imported_real_work_seeds_local_caller() {
        // WHY: cross-module worth-it propagation — a local fn that calls an imported
        // real-work callee does real work, exactly like the suspends seed path.
        let module = parse_module(
            r#"
function entrypoint() -> nothing {
    heavyImported()
}
"#,
        );
        let imported: HashSet<String> = ["heavyImported"].iter().map(|s| s.to_string()).collect();
        let imported_real_work: HashSet<String> =
            ["heavyImported"].iter().map(|s| s.to_string()).collect();
        let work = does_real_work_set(&module, &imported, &imported_real_work);
        assert!(
            work.contains("entrypoint"),
            "caller of imported real-work fn must do real work; got: {work:?}"
        );
    }

    #[test]
    fn imported_trivial_callee_does_not_seed_real_work() {
        // WHY: an imported callee NOT in the real-work set is a trivial leaf — the
        // caller must not be marked.
        let module = parse_module(
            r#"
function entrypoint() -> nothing {
    trivialImported()
}
"#,
        );
        let imported: HashSet<String> = ["trivialImported"].iter().map(|s| s.to_string()).collect();
        let work = does_real_work_set(&module, &imported, &HashSet::new());
        assert!(
            !work.contains("entrypoint"),
            "caller of imported trivial fn must NOT do real work; got: {work:?}"
        );
    }

    #[test]
    fn does_real_work_is_deterministic_across_runs() {
        // WHY: the fixpoint must be order-independent (no nondeterministic builds).
        let src = r#"
function fib(n: int) -> int {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}
function sumTo(n: int) -> int {
    let t = 0
    let i = 0
    while (i < n) { t = t + i; i = i + 1 }
    return t
}
function leaf(x: int) -> int { return x }
function entrypoint() -> nothing { print("hi") }
"#;
        let a = real_work_set(src);
        let b = real_work_set(src);
        assert_eq!(a, b, "does_real_work_set must be deterministic across runs");
        assert!(a.contains("fib") && a.contains("sumTo"));
        assert!(!a.contains("leaf") && !a.contains("entrypoint"));
    }
}
