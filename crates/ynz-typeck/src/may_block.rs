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
//! This is an **intra-compilation-unit** analysis. Cross-module calls (a callee
//! in another file) cannot be resolved here, so they are reported as
//! `UnresolvableEdge::CrossModule` — the caller of `analyze` is responsible for
//! emitting the appropriate can't-infer diagnostic. Same for dynamic dispatch
//! through a `dynamic Contract` vtable (`UnresolvableEdge::DynamicDispatch`).
//!
//! v0.3-M3 lifts the cross-module limit by propagating the `suspends` flag
//! through compiled-package metadata (design/future/packages.md) and wiring the
//! M8 multi-file query.
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
//!    intrinsic (inlining `M2_MAY_BLOCK_INTRINSICS`).
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

use crate::intrinsics::M2_MAY_BLOCK_INTRINSICS;

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
/// `imported_fn_names` is the set of function names imported from OTHER files
/// and visible in this module — used to distinguish cross-module calls (which
/// produce an `UnresolvableEdge::CrossModule`) from simple "unknown" names that
/// are probably user errors (which the normal typeck already handles).
///
/// Time: O(F² · E)  Space: O(F + E)
pub fn analyze(module: &Module, imported_fn_names: &HashSet<String>) -> MayBlockAnalysis {
    // Step 1 — collect the intra-unit call graph.
    let graph = build_call_graph(module, imported_fn_names);

    // Step 2 — seed from direct may-block intrinsic calls.
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
    analyze(module, imported_fn_names).suspends
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

fn build_call_graph(module: &Module, imported_fn_names: &HashSet<String>) -> CallGraph {
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
            &f.name,
            &mut fn_edges,
            &mut unresolvable,
        );
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
    enclosing_fn: &str,
    edges: &mut FnEdges,
    unresolvable: &mut Vec<(String, UnresolvableEdge)>,
) {
    for stmt in stmts {
        collect_calls_in_stmt(
            stmt,
            local_fns,
            imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                value,
                local_fns,
                imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                index,
                local_fns,
                imported_fns,
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                value,
                local_fns,
                imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_block(
                &body.stmts,
                local_fns,
                imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_block(
                &body.stmts,
                local_fns,
                imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_block(
                &body.stmts,
                local_fns,
                imported_fns,
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
                    enclosing_fn,
                    edges,
                    unresolvable,
                );
            }
        }
    }
}

fn collect_calls_in_expr(
    expr: &Expr,
    local_fns: &HashSet<String>,
    imported_fns: &HashSet<String>,
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
                    } else if M2_MAY_BLOCK_INTRINSICS.contains(&name.as_str()) {
                        // May-block intrinsic (not shadowed by a local fn of the same name) —
                        // record the boolean flag. This avoids putting intrinsic names into
                        // `direct`, which would cause a user function named `sleep` to
                        // false-positive via name collision in the seed step.
                        edges.calls_may_block_intrinsic = true;
                    } else if imported_fns.contains(&name) {
                        // Cross-module callee — we can't determine if it suspends.
                        unresolvable.push((
                            enclosing_fn.to_string(),
                            UnresolvableEdge::CrossModule {
                                callee_name: name.clone(),
                                import_src: String::new(),
                            },
                        ));
                    }
                    // else: unknown name — normal typeck will emit "not defined" diagnostic
                }
                // Recurse into arguments regardless (args are evaluated in calling context).
                for arg in &call.args {
                    collect_calls_in_expr(
                        arg,
                        local_fns,
                        imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                rhs,
                local_fns,
                imported_fns,
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
                enclosing_fn,
                edges,
                unresolvable,
                false,
            );
            collect_calls_in_expr(
                index,
                local_fns,
                imported_fns,
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
                    enclosing_fn,
                    edges,
                    unresolvable,
                    false,
                );
                collect_calls_in_expr(
                    v,
                    local_fns,
                    imported_fns,
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
    let graph = build_call_graph(module, imported_fn_names);
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
        analyze(&module, &HashSet::new()).suspends
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
    fn imported_fn_produces_cross_module_unresolvable() {
        let module = parse_module(
            r#"
function entrypoint() -> nothing {
    remoteOp()
}
"#,
        );
        let imported: HashSet<String> = ["remoteOp"].iter().map(|s| s.to_string()).collect();
        let result = analyze(&module, &imported);
        assert!(
            result.unresolvable.iter().any(|(_, e)| matches!(
                e,
                UnresolvableEdge::CrossModule { callee_name, .. } if callee_name == "remoteOp"
            )),
            "imported callee must produce CrossModule unresolvable edge"
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
}
