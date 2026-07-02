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
use crate::suspension_source::is_base_suspension_intrinsic;

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
/// read, no nested group sharing the host with a call to another CPU-group-capable function,
/// no nested param-host.
///
/// **History (v0.3-M3g Phases 2–3):** Phase 2 temporarily declined a top-level group whenever the
/// host `f` was itself a member of the pre-promotion suspend set (co-resident-suspension decline),
/// keeping the promotion lift behavior-neutral. Phase 3 built the real fused CPU+I/O continuation
/// and removed that decline (a host that also suspends elsewhere is now admitted — the state
/// machine's ordinary multi-suspension-point dispatch already shares one continuation across every
/// suspension state). The same phase also root-caused and removed a NESTED-branch residual decline
/// that had been guarding a co-resident call to another spike-capable host: the crash it guarded
/// against was a frame-size under-allocation bug in `crates/ynz-codegen/src/emit.rs` (a spike-active
/// host's `frame_bytes` computation silently dropped an embedded child sub-frame), not a runtime
/// handle-lifecycle defect — fixed directly in `emit.rs`, verified 20/20 clean runs. Both declines'
/// former parameters (`base_suspends`, `spike_capable_names`) are gone with them; see the plan's
/// audit trail for the full repro + fix history.
///
/// Time: O(1) amortized (`HashSet::contains`) plus the O(N) AST work shared with the other gates.
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
    //
    // **v0.3-M3g Phase 3 admission flip**: the Phase-2 TEMPORARY co-resident-suspension decline
    // (keyed on `base_suspends.contains(&f.name)`) that lived here is REMOVED in this phase. A
    // host that also suspends elsewhere (an explicit `wait`, a suspending callee, or a bare
    // may-block-intrinsic call) is now admitted: the state machine's ordinary multi-suspension-
    // point dispatch already shares ONE continuation across every suspension state in the
    // function (the CPU join's poll state is just one more `resume_point` value alongside any
    // `wait`-derived state), so a group co-resident with another suspension is safe once its
    // result names are correctly frame-backed — which `spike_cpu_group_result_names`'s
    // depth-aware pre-allocation (Step 1c) now guarantees for both top-level and nested groups.
    if let Some(members) = cpu_group_member_indices(&f.body.stmts, suspend_set, supported_callees) {
        let last_idx = *members.last().expect("group has ≥2 members");
        let post_stmts = &f.body.stmts[(last_idx + 1)..];
        if param_read_after_join(f, post_stmts) {
            return None;
        }
        // **RESOLVED (was: a HALT-and-surface residual decline for a POST-GROUP control-flow
        // statement whose body suspends):** a prior session root-caused but did not fix the real
        // bug — `crate::check`'s crossing-local analysis (`collect_crossings_in_stmts`) flushed a
        // CPU group's pending result-binding names into `declared` only when it recognized the
        // FOLLOWING statement as itself suspending, and its `this_stmt_suspends` check matched
        // only a DIRECT top-level `wait`/suspending-call statement, never a control-flow statement
        // whose BODY suspends. Fixed directly in `check.rs` this session: `this_stmt_suspends` now
        // also recognizes an `if`/`while`/`for`/`match` whose body suspends as a suspension point
        // for the surrounding statement sequence, flushing pending result-bindings and recursing
        // into the suspending sub-block exactly like the not-yet-suspended case already did. With
        // the real crossing-analysis gap fixed, this decline is unnecessary and removed — a
        // POST-GROUP control-flow statement whose body suspends no longer needs special handling
        // here.
        return Some(AdmittedCpuGroup {
            block_path: Vec::new(),
            member_indices: members,
        });
    }

    // The sole group is one level inside a branch arm.
    //
    // **v0.3-M3g Phase 3 admission flip**: this used to decline whenever the host ALSO contained
    // any OTHER suspension point (an explicit `wait` or a suspending callee) — because a second
    // suspension resumed through the reload path, which looked up the nested group's bind names
    // in an (empty, pre-Phase-3) alloca map and aborted ("no sm_entry alloca for `<name>`"; see
    // the M3d DECLINE fixtures this un-declines, `integration.rs`). `spike_cpu_group_result_names`
    // now walks nested blocks (mirroring `spike_cpu_group_member_count`'s traversal), so Step 1c
    // pre-allocates the nested group's result names regardless of depth, and the reload succeeds.
    // A nested group co-resident with another suspension is therefore admitted here too — the
    // decline that used to guard this exact crash is gone because its root cause is fixed, not
    // papered over.
    //
    // **RESOLVED (was: "the one case that STAYS declined" — a call anywhere in `f`'s body to a
    // function that is ITSELF spike-capable):** this residual decline is REMOVED. See the doc
    // comment above `admitted_cpu_group` for the full root-cause correction — the crash it used
    // to guard against was a frame-size under-allocation bug (fixed in `emit.rs`), not a runtime
    // handle-lifecycle defect. A nested group co-resident with a call into another spike-capable
    // host is admitted here too, exactly like any other co-resident suspension.

    // A nested group in a function that takes ANY parameter declines: the post-join frontier
    // crosses the branch boundary, which needs param-slot reservation the spike path does not yet
    // provide. A zero-param nested host is unaffected.
    if !f.params.is_empty() {
        return None;
    }

    nested_group_member_path(&f.body.stmts, suspend_set, supported_callees)
}

// ── Fused (mixed CPU+I/O) top-level group — v0.3-M3g Phase 3 ───────────────────────────────

/// The execution class of one member of an [`AdmittedFusedGroup`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusedMemberClass {
    /// A pure-CPU call spawned onto the blocking pool (same eligibility as
    /// [`cpu_group_member_indices`]'s per-member checks).
    Cpu,
    /// A suspending (I/O) call inline-polled via its embedded child sub-frame.
    Suspending,
}

/// The admitted FUSED group for one function: a maximal adjacent run, at the TOP LEVEL of the
/// function body ONLY, mixing at least one [`FusedMemberClass::Cpu`] member and at least one
/// [`FusedMemberClass::Suspending`] member. `(stmt_index, class)` pairs, in source order.
///
/// **Scope (deliberate, recorded per the plan's disciplined-initiative rule 4 — "edge shapes
/// beyond the three non-negotiable flip fixtures MAY decline safely"):** top-level only (a NESTED
/// mixed group declines to sequential — the existing pure-CPU nested machinery's frame-slot
/// reasoning does not extend to a nested FUSED group without further work); every member's call
/// takes exactly one `IntLit`/`Ident` argument (mirrors [`cpu_group_member_indices`]'s existing
/// CPU-member restriction — the 8-byte spawn ctx cannot carry more, and restricting BOTH classes
/// to this shape sidesteps re-deriving the write-effect/alias soundness floor
/// `crate::independence`'s full analysis provides: a scalar-only argument set has no possible
/// aliased-write hazard between members, so independence holds by construction regardless of what
/// each callee does internally). A group with only ONE class present is NOT fused — the existing,
/// separately-tested pure-CPU (`cpu_group_member_indices`) or pure-I/O
/// (`crate::independence::partition_independent_groups`) paths already handle it and are left
/// untouched by this function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedFusedGroup {
    pub members: Vec<(usize, FusedMemberClass)>,
}

/// Extract `(callee_name, args)` from a direct ident call statement (`let x = f(a)` or `f(a)`),
/// or `None` for anything else (method calls, non-ident callees, non-call statements).
///
/// Time: O(1).
fn stmt_direct_call(stmt: &Stmt) -> Option<(&str, &[Expr])> {
    let c = match stmt {
        Stmt::Let {
            value: Expr::Call(c),
            ..
        }
        | Stmt::Expr(Expr::Call(c)) => c,
        _ => return None,
    };
    match &c.callee {
        Expr::Ident(name, _) => Some((name.as_str(), c.args.as_slice())),
        _ => None,
    }
}

/// True when `args` is exactly one `IntLit`/`Ident` argument — the shape both
/// [`cpu_group_member_indices`] (CPU members) and [`admitted_fused_group`] (both classes) require.
///
/// Time: O(1).
fn single_scalar_arg(args: &[Expr]) -> bool {
    args.len() == 1 && matches!(args[0], Expr::IntLit(_, _) | Expr::Ident(_, _))
}

/// Return the single admitted FUSED (mixed CPU+I/O) group for `f`'s TOP-LEVEL body, or `None`.
///
/// Sequential fallback is always correct — a `None` here means the normal, unchanged pure-CPU
/// (`admitted_cpu_group`) and pure-I/O (`partition_independent_groups`-driven) paths handle the
/// function exactly as before this function existed.
///
/// Admission gates (all must hold):
/// - a maximal adjacent run (length ≥ 2) of eligible members, where a member is eligible when it
///   is a direct ident call with exactly one `IntLit`/`Ident` argument AND the callee is either
///   pure-CPU-eligible (not in `suspend_set`, in `supported_callees`, not self-recursive) or
///   suspending-eligible (in `suspend_set`),
/// - the run contains AT LEAST ONE member of EACH class (else it is not fused — see the doc
///   comment above),
/// - no member's argument references an EARLIER member's bind name (independence — mirrors
///   [`cpu_group_member_indices`]'s existing dependency check),
/// - no two Suspending members share the same callee name (mirrors the pre-existing M3b
///   same-callee-suspending restriction — one embedded child sub-frame per unique callee name;
///   out of this dispatch's scope to lift),
/// - no pre-group statement suspends,
/// - no post-group statement assigns a member's bind name or calls a user-defined suspending
///   callee (mirrors [`cpu_group_member_indices`]'s existing post-group gates).
///
/// Time: O(n) where n = stmts length  Space: O(m) where m = group members.
pub fn admitted_fused_group(
    f: &FunctionDecl,
    suspend_set: &SuspendSet,
    supported_callees: &HashSet<String>,
) -> Option<AdmittedFusedGroup> {
    // A host with ANY parameter declines. Params and the fused group's CPU handle/result
    // reserve share the SAME byte-32-relative slot addressing (`admitted_cpu_group`'s top-level
    // branch tolerates this ONLY behind `param_read_after_join`'s narrower "no post-join READ"
    // gate — see that function's doc comment for the exact overlap mechanism). A fused group
    // ALSO embeds I/O child sub-frames whose own byte layout depends on the same `own_base`
    // computation the reserve pushes past, so the safer, more conservative bar for this first
    // fused-group codegen consumer is "no params at all" — narrowing later to mirror
    // `param_read_after_join`'s precision is legitimate follow-on work (Future Requirements),
    // never a correctness requirement for this always-safe-to-decline gate.
    if !f.params.is_empty() {
        return None;
    }

    let stmts = &f.body.stmts;

    // Classify every top-level statement (eligible Cpu / eligible Suspending / ineligible).
    let classify = |i: usize| -> Option<FusedMemberClass> {
        let stmt = &stmts[i];
        if stmt_has_explicit_wait_stmt(stmt) {
            return None; // an explicit `wait` is an ordering barrier, never a group member.
        }
        let (callee, args) = stmt_direct_call(stmt)?;
        if !single_scalar_arg(args) {
            return None;
        }
        if suspend_set.contains(callee) {
            Some(FusedMemberClass::Suspending)
        } else if supported_callees.contains(callee) && callee != f.name {
            Some(FusedMemberClass::Cpu)
        } else {
            None
        }
    };

    let eligible: Vec<(usize, FusedMemberClass)> = (0..stmts.len())
        .filter_map(|i| classify(i).map(|c| (i, c)))
        .collect();

    // Anchor on the first adjacent pair (any class combination), then extend forward over every
    // further adjacent eligible statement — mirrors `cpu_group_member_indices`'s anchoring.
    let first_idx = eligible
        .windows(2)
        .find(|w| w[1].0 == w[0].0 + 1)
        .map(|w| w[0].0)?;
    let mut last_idx = first_idx + 1;
    while eligible.iter().any(|&(i, _)| i == last_idx + 1) {
        last_idx += 1;
    }
    let members: Vec<(usize, FusedMemberClass)> = eligible
        .into_iter()
        .filter(|&(i, _)| i >= first_idx && i <= last_idx)
        .collect();

    // Must be genuinely MIXED — at least one member of each class.
    let has_cpu = members.iter().any(|&(_, c)| c == FusedMemberClass::Cpu);
    let has_suspending = members
        .iter()
        .any(|&(_, c)| c == FusedMemberClass::Suspending);
    if !has_cpu || !has_suspending {
        return None;
    }

    // Same-callee restriction for Suspending members only (mirrors M3b).
    let mut seen_suspending_callees: HashSet<&str> = HashSet::new();
    for &(i, class) in &members {
        if class == FusedMemberClass::Suspending {
            let (callee, _) = stmt_direct_call(&stmts[i]).expect("classified as a direct call");
            if !seen_suspending_callees.insert(callee) {
                return None;
            }
        }
    }

    // Independence: no member's argument references an EARLIER member's bind name.
    let bind_name = |i: usize| -> Option<&str> {
        match &stmts[i] {
            Stmt::Let { name, .. } => Some(name.as_str()),
            _ => None,
        }
    };
    let arg_ident = |i: usize| -> Option<&str> {
        let (_, args) = stmt_direct_call(&stmts[i])?;
        match args.first() {
            Some(Expr::Ident(n, _)) => Some(n.as_str()),
            _ => None,
        }
    };
    let earlier_bind_names: Vec<&str> = members.iter().filter_map(|&(i, _)| bind_name(i)).collect();
    for (pos, &(i, _)) in members.iter().enumerate() {
        let prior = &earlier_bind_names[..pos.min(earlier_bind_names.len())];
        if let Some(a) = arg_ident(i) {
            if prior.contains(&a) {
                return None;
            }
        }
    }

    // No pre-group statement may suspend.
    if stmts[..first_idx]
        .iter()
        .any(|s| stmt_contains_wait_deep(s) || stmt_contains_suspending_call_deep(s, suspend_set))
    {
        return None;
    }

    // No post-group statement may assign a member's bind name or call a user-defined suspending
    // callee (mirrors `cpu_group_member_indices`'s existing post-group gates).
    let bind_names: Vec<&str> = members.iter().filter_map(|&(i, _)| bind_name(i)).collect();
    let post_stmts = &stmts[(last_idx + 1)..];
    for bn in &bind_names {
        if post_stmts.iter().any(|s| stmt_assigns_name(s, bn)) {
            return None;
        }
    }
    if post_stmts
        .iter()
        .any(|s| stmt_contains_suspending_call_deep(s, suspend_set))
    {
        return None;
    }
    // RESOLVED — see `admitted_cpu_group`'s top-level branch doc comment for the root-cause fix
    // (the `crate::check` crossing-analysis gap this decline used to guard against is fixed
    // directly in `check.rs`; this fused-group mirror decline is removed for the same reason).

    Some(AdmittedFusedGroup { members })
}

/// True when `stmt` is (or wraps, per the existing `wait`-expr shapes) an explicit `wait`. A
/// narrower helper than [`stmt_contains_wait_deep`] — this checks only the STATEMENT's own
/// top-level expression, matching `crate::independence::stmt_has_explicit_wait`'s contract
/// (an explicit `wait` resets grouping; only the wait-bearing statement itself is excluded, not
/// its descendants — this function never has descendants to check since a fused-group candidate
/// is a plain call statement).
///
/// Time: O(1).
fn stmt_has_explicit_wait_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(Expr::Wait(..))
            | Stmt::Let {
                value: Expr::Wait(..),
                ..
            }
    )
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
                    && !is_base_suspension_intrinsic(name.as_str())
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
