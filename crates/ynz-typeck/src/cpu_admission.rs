//! CPU auto-parallelization admission gate — the single decision authority for which
//! statement groups the compiler actually spawns onto separate cores.
//!
//! Two different group sets exist for a promoted function:
//!   1. The broad set of *parallelizable* groups found by [`crate::independence::partition_groups_classified`]
//!      — every adjacent run of independent CPU calls. This drives promotion candidacy.
//!   2. The narrow set this module admits — the groups that pass every frame-layout safety
//!      gate and therefore get a real spawn+join in codegen.
//!
//! Set 2 is a subset of set 1: a function can own a parallelizable group that the admission
//! gate then declines (two groups in one function, a parameter read after the join, a nested
//! group sharing the host with another suspension point, etc.). The frame-slot reasoning behind
//! each decline is documented inline.
//!
//! This gate lives in `ynz-typeck` (not `ynz-codegen`) because BOTH consumers must read the same
//! decision: codegen spawns exactly the admitted groups, and the `parallel_groups` inlay-hint
//! pass renders the "runs at the same time" hint on exactly the admitted groups. Crate direction
//! is codegen → typeck, so a shared predicate must live here for codegen to call up into it. If
//! the gate lived in codegen the hint pass could not reach it (typeck cannot depend on codegen),
//! and the hint would render against the broad set — telling the user a group runs in parallel
//! that codegen never spawned. The registry promise on `parallel_groups`/`background_routing`
//! ("the same set that drives codegen routing, so the hint and the binary always agree") is what
//! this single home enforces.

use std::collections::HashSet;

use ynz_ast::nodes::{Block, Expr, FunctionDecl, Stmt};

use crate::independence::collect_ident_names;
use crate::intrinsics::M2_MAY_BLOCK_INTRINSICS;

/// The transitive suspending-function name set (same shape codegen uses).
pub type SuspendSet = HashSet<String>;

/// The admitted CPU group for one function: the member statements' indices within the block that
/// directly contains them, plus the path of nested-block descent to reach that block.
///
/// `block_path` is the sequence of indices selecting nested branch blocks from the function body
/// to the block that holds the group (empty = the top-level body). `member_indices` are positions
/// WITHIN that block. A function admits at most one group (the single-group constraint), so an
/// admitting function yields exactly one `AdmittedCpuGroup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedCpuGroup {
    /// Indices selecting nested branch blocks from the function body down to the group's block.
    pub block_path: Vec<BlockStep>,
    /// Member statement indices within the group's block (≥2, contiguous).
    pub member_indices: Vec<usize>,
}

/// One step of nested-block descent: which statement, and which sub-block of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStep {
    /// Statement index in the parent block.
    pub stmt_index: usize,
    /// Sub-block index within that statement (`if` body = 0; match arm/else = arm order).
    pub block_index: usize,
}

/// Return the single admitted CPU group for `f`, or `None` when codegen declines to spawn.
///
/// This is the function both codegen and the inlay-hint pass call. A `Some` result means codegen
/// emits a spawn+join for these exact members; a `None` result means the whole function lowers
/// sequentially (byte-identical to `--no-auto-parallel`).
///
/// Mirrors the codegen admission decision exactly: single-group constraint, no post-join param
/// read, no nested group sharing the host with another suspension, no nested param-host.
///
/// Time: O(N) where N = AST nodes in `f`  Space: O(D) recursion depth + O(m) members.
pub fn admitted_cpu_group(
    f: &FunctionDecl,
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> Option<AdmittedCpuGroup> {
    // Single-group constraint: a function spike-hosts IFF it has EXACTLY ONE CPU group across all
    // depths. Two-or-more groups would alias the single group-0 slot region — a silent wrong
    // answer — so decline ALL spiking and lower sequentially. Zero groups: nothing to host.
    if count_cpu_groups_all_depths(&f.body.stmts, suspend_set, supported_callees) != 1 {
        return None;
    }

    // Top-level group: hosted by the depth-0 path. Decline only when a parameter is READ in a
    // post-join statement — the wrapper writes param slot 0 at the byte the first CPU handle
    // occupies, so a post-join reload returns the handle pointer's bytes instead of the value.
    // A param used only in spawn args round-trips safely (the load precedes the handle store).
    if let Some(members) = cpu_group_member_indices(&f.body.stmts, suspend_set, supported_callees) {
        let last_idx = *members.last().expect("group has ≥2 members");
        let post_stmts = &f.body.stmts[(last_idx + 1)..];
        if param_read_after_join(f, post_stmts) {
            return None;
        }
        return Some(AdmittedCpuGroup {
            block_path: Vec::new(),
            member_indices: members,
        });
    }

    // The sole group is one level inside a branch arm. It declines when the host also contains any
    // OTHER suspension point (an explicit `wait` or a suspending callee): a second suspension
    // resumes through the reload path, which looks up the nested group's bind names in the
    // (empty, because Step 1c scans only the top level) alloca map and aborts. Pure-CPU IFF the
    // only suspension is the nested join itself.
    if f.body
        .stmts
        .iter()
        .any(|s| stmt_contains_wait_deep(s) || stmt_contains_suspending_call_deep(s, suspend_set))
    {
        return None;
    }

    // A nested group in a function that takes ANY parameter declines: the post-join frontier
    // crosses the branch boundary, which needs param-slot reservation the spike path does not yet
    // provide. A zero-param nested host is unaffected.
    if !f.params.is_empty() {
        return None;
    }

    nested_group_member_path(&f.body.stmts, suspend_set, supported_callees)
}

/// The member statement indices of the single admissible CPU group in one straight-line block, or
/// `None` if no group is admissible there. A group is a maximal run of N ≥ 2 adjacent eligible
/// CPU calls that passes every admission gate. SINGLE source of truth for "which statements are
/// this block's CPU group" — every count/extract/representative path derives from it.
///
/// Admission gates (all must hold; sequential fallback is always correct):
/// - run length ≥ 2,
/// - every member has exactly one literal/ident argument (the 8-byte trampoline ctx),
/// - no member's argument references an EARLIER member's bind name (independence),
/// - no pre-group statement suspends (a pre-group wait leaves a terminator-less block),
/// - no post-group statement assigns a member's bind name (would source a never-populated slot),
/// - no post-group statement calls a user-defined suspending callee (its child sub-frame would
///   alias a joined result slot; intrinsic `wait sleep` is exempt).
///
/// Time: O(n) where n = stmts length  Space: O(m) where m = group members.
pub fn cpu_group_member_indices(
    stmts: &[Stmt],
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> Option<Vec<usize>> {
    let mut call_indices: Vec<usize> = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        let is_eligible = match stmt {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => {
                if let Expr::Ident(name, _) = &c.callee {
                    !suspend_set.contains(name.as_str()) && supported_callees.contains(name)
                } else {
                    false
                }
            }
            _ => false,
        };
        if is_eligible {
            call_indices.push(i);
        }
    }

    // First adjacent pair anchors the group; extend forward over every further adjacent call.
    let first_idx = call_indices
        .windows(2)
        .find(|w| w[1] == w[0] + 1)
        .map(|w| w[0])?;
    let mut last_idx = first_idx + 1;
    while call_indices.contains(&(last_idx + 1)) {
        last_idx += 1;
    }
    let member_indices: Vec<usize> = (first_idx..=last_idx).collect();

    let args_lowerable = |i: usize| -> bool {
        let call = match &stmts[i] {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => c,
            _ => return false,
        };
        call.args.len() == 1
            && call
                .args
                .iter()
                .all(|arg| matches!(arg, Expr::IntLit(_, _) | Expr::Ident(_, _)))
    };
    if member_indices.iter().any(|&i| !args_lowerable(i)) {
        return None;
    }

    let member_bind_name = |i: usize| -> Option<&str> {
        match &stmts[i] {
            Stmt::Let { name, .. } => Some(name.as_str()),
            _ => None,
        }
    };
    let member_arg_idents = |i: usize| -> Vec<&str> {
        match &stmts[i] {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => c
                .args
                .iter()
                .filter_map(|arg| match arg {
                    Expr::Ident(n, _) => Some(n.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    };
    // `earlier_bind_names` is COMPACTED (drops non-`Let` members, which have no bind name) but
    // `pos` indexes the FULL `member_indices`. Because data dependencies flow forward-only, the
    // resulting superset can only trigger a spurious DECLINE (over-conservative sequential
    // fallback), never a false-ADMIT of a genuinely dependent member.
    let earlier_bind_names: Vec<&str> = member_indices
        .iter()
        .filter_map(|&i| member_bind_name(i))
        .collect();
    for (pos, &i) in member_indices.iter().enumerate() {
        let prior = &earlier_bind_names[..pos.min(earlier_bind_names.len())];
        if member_arg_idents(i).iter().any(|a| prior.contains(a)) {
            return None;
        }
    }

    // No pre-group statement may suspend.
    if stmts[..first_idx]
        .iter()
        .any(|s| stmt_contains_wait_deep(s) || stmt_contains_suspending_call_deep(s, suspend_set))
    {
        return None;
    }

    // No post-group statement may assign a member's bind name.
    let bind_names: Vec<&str> = member_indices
        .iter()
        .filter_map(|&i| member_bind_name(i))
        .collect();
    let post_stmts = &stmts[(last_idx + 1)..];
    for bind_name in &bind_names {
        if post_stmts.iter().any(|s| stmt_assigns_name(s, bind_name)) {
            return None;
        }
    }

    // No post-group statement may call a user-defined suspending callee. Intrinsic waits exempt.
    if post_stmts
        .iter()
        .any(|s| stmt_contains_suspending_call_deep(s, suspend_set))
    {
        return None;
    }

    Some(member_indices)
}

/// The representative (first) callee name of the admissible CPU group in one straight-line block.
///
/// Time: O(n) where n = stmts length  Space: O(1).
pub fn pair_representative_callee(
    stmts: &[Stmt],
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> Option<String> {
    let members = cpu_group_member_indices(stmts, suspend_set, supported_callees)?;
    let first_idx = members[0];
    match &stmts[first_idx] {
        Stmt::Let {
            value: Expr::Call(c),
            ..
        }
        | Stmt::Expr(Expr::Call(c)) => match &c.callee {
            Expr::Ident(name, _) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The nested straight-line blocks (`if`/`match` arm bodies) reachable directly inside `stmts`,
/// in lowering order. `for`/`while` bodies are excluded — a CPU group in a loop body needs
/// loop-index frame backing the spike path does not provide, so it never fires there.
///
/// Time: O(A) where A = match arms  Space: O(A)
pub fn nested_blocks(stmt: &Stmt) -> Vec<&[Stmt]> {
    match stmt {
        Stmt::If { body, .. } => vec![body.stmts.as_slice()],
        Stmt::Match { arms, else_arm, .. } => {
            let mut v: Vec<&[Stmt]> = arms.iter().map(|a| a.body.stmts.as_slice()).collect();
            if let Some(eb) = else_arm {
                v.push(eb.stmts.as_slice());
            }
            v
        }
        _ => Vec::new(),
    }
}

/// Count admissible CPU groups across `stmts` and every nested block reachable from it. Enforces
/// the single-group constraint so the group-0 frame slots are never aliased by a second group.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn count_cpu_groups_all_depths(
    stmts: &[Stmt],
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> usize {
    let mut count = 0;
    if cpu_group_member_indices(stmts, suspend_set, supported_callees).is_some() {
        count += 1;
    }
    for stmt in stmts {
        for block in nested_blocks(stmt) {
            count += count_cpu_groups_all_depths(block, suspend_set, supported_callees);
        }
    }
    count
}

/// The representative callee of the single CPU group nested inside `stmts` (one level into a
/// branch arm), searched in lowering order.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn nested_group_representative_callee(
    stmts: &[Stmt],
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> Option<String> {
    for stmt in stmts {
        for block in nested_blocks(stmt) {
            if let Some(callee) = pair_representative_callee(block, suspend_set, supported_callees)
            {
                return Some(callee);
            }
            if let Some(callee) =
                nested_group_representative_callee(block, suspend_set, supported_callees)
            {
                return Some(callee);
            }
        }
    }
    None
}

/// The `block_path` + `member_indices` of the single nested CPU group inside `stmts`, in lowering
/// order. The caller has established (via `count_cpu_groups_all_depths == 1`) that exactly one
/// group exists, so the first nested group found is THE group.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
fn nested_group_member_path(
    stmts: &[Stmt],
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> Option<AdmittedCpuGroup> {
    for (stmt_index, stmt) in stmts.iter().enumerate() {
        for (block_index, block) in nested_blocks(stmt).into_iter().enumerate() {
            if let Some(members) = cpu_group_member_indices(block, suspend_set, supported_callees) {
                return Some(AdmittedCpuGroup {
                    block_path: vec![BlockStep {
                        stmt_index,
                        block_index,
                    }],
                    member_indices: members,
                });
            }
            if let Some(mut deeper) =
                nested_group_member_path(block, suspend_set, supported_callees)
            {
                deeper.block_path.insert(
                    0,
                    BlockStep {
                        stmt_index,
                        block_index,
                    },
                );
                return Some(deeper);
            }
        }
    }
    None
}

/// True when any of `f`'s parameters is READ in a post-join statement — the one shape the spike
/// frame layout cannot serve (the spawn overwrites param slot 0, so a post-join reload returns
/// the handle pointer's bytes instead of the parameter value).
///
/// Time: O(N) where N = AST nodes across `post_stmts`  Space: O(P) param-name set.
pub fn param_read_after_join(f: &FunctionDecl, post_stmts: &[Stmt]) -> bool {
    if f.params.is_empty() {
        return false;
    }
    let mut reads: HashSet<String> = HashSet::new();
    for s in post_stmts {
        stmt_tree_ident_reads(s, &mut reads);
    }
    f.params.iter().any(|p| reads.contains(p.name.as_str()))
}

// AST walkers (shared with codegen via delegation).

/// True when `stmt` contains an explicit `wait` anywhere in its subtree (including nested blocks).
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn stmt_contains_wait_deep(stmt: &Stmt) -> bool {
    let block_has = |b: &Block| b.stmts.iter().any(stmt_contains_wait_deep);
    match stmt {
        Stmt::Expr(e) => expr_contains_wait(e),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_contains_wait(value),
        Stmt::If { cond, body, .. } => expr_contains_wait(cond) || block_has(body),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_contains_wait(scrutinee)
                || arms.iter().any(|a| block_has(&a.body))
                || else_arm.as_ref().is_some_and(block_has)
        }
        Stmt::While { cond, body, .. } => expr_contains_wait(cond) || block_has(body),
        Stmt::For { iter, body, .. } => expr_contains_wait(iter) || block_has(body),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_contains_wait),
        Stmt::FieldAssign { target, value, .. } => {
            expr_contains_wait(target) || expr_contains_wait(value)
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => expr_contains_wait(receiver) || expr_contains_wait(index) || expr_contains_wait(value),
    }
}

fn expr_contains_wait(expr: &Expr) -> bool {
    match expr {
        Expr::Wait(..) => true,
        Expr::Background(inner, _) => expr_contains_wait(inner),
        Expr::Call(c) => c.args.iter().any(expr_contains_wait),
        Expr::BinOp { lhs, rhs, .. } => expr_contains_wait(lhs) || expr_contains_wait(rhs),
        Expr::UnaryOp { operand, .. } => expr_contains_wait(operand),
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_wait(receiver) || args.iter().any(expr_contains_wait)
        }
        Expr::FieldAccess { receiver, .. } => expr_contains_wait(receiver),
        Expr::IndexAccess {
            receiver, index, ..
        } => expr_contains_wait(receiver) || expr_contains_wait(index),
        Expr::StructLit { fields, .. } => fields.iter().any(|f| expr_contains_wait(&f.value)),
        Expr::ArrayLit { elements, .. } => elements.iter().any(expr_contains_wait),
        Expr::MapLit { entries, .. } => entries
            .iter()
            .any(|(k, v)| expr_contains_wait(k) || expr_contains_wait(v)),
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| match p {
            ynz_ast::nodes::StringPart::Expr(e, _) => expr_contains_wait(e),
            ynz_ast::nodes::StringPart::Lit(..) => false,
        }),
        Expr::PostfixOp { receiver, .. } => expr_contains_wait(receiver),
        Expr::Is { expr: inner, .. } => expr_contains_wait(inner),
        _ => false,
    }
}

/// True when `stmt` contains a call to a user-defined suspending callee anywhere in its subtree.
/// May-block intrinsics (`sleep`, `__testFallibleAsync`) are exempt — they have no embedded child
/// sub-frame to alias a joined result slot.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn stmt_contains_suspending_call_deep(stmt: &Stmt, suspend_set: &SuspendSet) -> bool {
    let block_has = |b: &Block| {
        b.stmts
            .iter()
            .any(|s| stmt_contains_suspending_call_deep(s, suspend_set))
    };
    match stmt {
        Stmt::Expr(e) => expr_contains_suspending_call(e, suspend_set),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            expr_contains_suspending_call(value, suspend_set)
        }
        Stmt::Return { value, .. } => value
            .as_ref()
            .is_some_and(|e| expr_contains_suspending_call(e, suspend_set)),
        Stmt::FieldAssign { target, value, .. } => {
            expr_contains_suspending_call(target, suspend_set)
                || expr_contains_suspending_call(value, suspend_set)
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            expr_contains_suspending_call(receiver, suspend_set)
                || expr_contains_suspending_call(index, suspend_set)
                || expr_contains_suspending_call(value, suspend_set)
        }
        Stmt::If { cond, body, .. } => {
            expr_contains_suspending_call(cond, suspend_set) || block_has(body)
        }
        Stmt::While { cond, body, .. } => {
            expr_contains_suspending_call(cond, suspend_set) || block_has(body)
        }
        Stmt::For { iter, body, .. } => {
            expr_contains_suspending_call(iter, suspend_set) || block_has(body)
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_contains_suspending_call(scrutinee, suspend_set)
                || arms.iter().any(|a| block_has(&a.body))
                || else_arm.as_ref().is_some_and(block_has)
        }
    }
}

fn expr_contains_suspending_call(expr: &Expr, suspend_set: &SuspendSet) -> bool {
    match expr {
        Expr::Call(c) => {
            if let Expr::Ident(name, _) = &c.callee {
                if suspend_set.contains(name.as_str())
                    && !M2_MAY_BLOCK_INTRINSICS.contains(&name.as_str())
                {
                    return true;
                }
            }
            c.args
                .iter()
                .any(|a| expr_contains_suspending_call(a, suspend_set))
        }
        Expr::Wait(inner, _) => expr_contains_suspending_call(inner, suspend_set),
        // A `background` spawn schedules separately — it does not suspend the current function.
        Expr::Background(_inner, _) => false,
        Expr::BinOp { lhs, rhs, .. } => {
            expr_contains_suspending_call(lhs, suspend_set)
                || expr_contains_suspending_call(rhs, suspend_set)
        }
        Expr::UnaryOp { operand, .. } => expr_contains_suspending_call(operand, suspend_set),
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_suspending_call(receiver, suspend_set)
                || args
                    .iter()
                    .any(|a| expr_contains_suspending_call(a, suspend_set))
        }
        _ => false,
    }
}

/// True when `stmt` (at any nesting depth) reassigns the binding `name` via `=`.
///
/// Time: O(N) where N = AST nodes  Space: O(D) recursion depth.
pub fn stmt_assigns_name(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::Assign { target, .. } => target.as_str() == name,
        Stmt::If { body, .. } => body.stmts.iter().any(|s| stmt_assigns_name(s, name)),
        Stmt::While { body, .. } | Stmt::For { body, .. } => {
            body.stmts.iter().any(|s| stmt_assigns_name(s, name))
        }
        Stmt::Match { arms, else_arm, .. } => {
            arms.iter()
                .any(|a| a.body.stmts.iter().any(|s| stmt_assigns_name(s, name)))
                || else_arm
                    .as_ref()
                    .is_some_and(|b| b.stmts.iter().any(|s| stmt_assigns_name(s, name)))
        }
        _ => false,
    }
}

/// Collect every identifier name READ anywhere in `stmt`, descending into nested control flow.
/// Loop-variable and destructure-bound names shadow outer names only inside the body, so they are
/// stripped from a loop-local read set before union — never removed from the shared accumulator
/// (which could erase a legitimate earlier post-join param read).
///
/// Time: O(N) where N = AST nodes  Space: O(R) read-name set + O(D) recursion depth.
pub fn stmt_tree_ident_reads(stmt: &Stmt, out: &mut HashSet<String>) {
    let block_reads = |b: &Block, out: &mut HashSet<String>| {
        for s in &b.stmts {
            stmt_tree_ident_reads(s, out);
        }
    };
    match stmt {
        Stmt::Expr(e) | Stmt::Let { value: e, .. } | Stmt::Assign { value: e, .. } => {
            collect_ident_names(e, out)
        }
        Stmt::Return { value: Some(v), .. } => collect_ident_names(v, out),
        Stmt::Return { value: None, .. } => {}
        Stmt::FieldAssign { target, value, .. } => {
            collect_ident_names(target, out);
            collect_ident_names(value, out);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_ident_names(receiver, out);
            collect_ident_names(index, out);
            collect_ident_names(value, out);
        }
        Stmt::If { cond, body, .. } => {
            collect_ident_names(cond, out);
            block_reads(body, out);
        }
        Stmt::While { cond, body, .. } => {
            collect_ident_names(cond, out);
            block_reads(body, out);
        }
        Stmt::For {
            iter,
            body,
            var,
            destructure_pattern,
            map_destructure_pattern,
            ..
        } => {
            collect_ident_names(iter, out);
            let mut body_reads = HashSet::new();
            block_reads(body, &mut body_reads);
            body_reads.remove(var.as_str());
            if let Some(bindings) = destructure_pattern {
                for b in bindings {
                    let bound = b.alias.as_deref().unwrap_or(b.field.as_str());
                    body_reads.remove(bound);
                }
            }
            if let Some((key, value)) = map_destructure_pattern {
                body_reads.remove(key.as_str());
                body_reads.remove(value.as_str());
            }
            out.extend(body_reads);
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_ident_names(scrutinee, out);
            for arm in arms {
                block_reads(&arm.body, out);
            }
            if let Some(eb) = else_arm {
                block_reads(eb, out);
            }
        }
    }
}
