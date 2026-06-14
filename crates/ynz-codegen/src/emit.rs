/// LLVM IR emission via inkwell.
///
/// # LLVM context lifetime
///
/// `inkwell::Context` is NOT `Send + Sync` and must NOT outlive this module.
/// `emit_artifact` creates a `Context`, builds the module, emits object bytes,
/// and drops the context before returning. Salsa never sees inkwell types.
///
/// # Decimal128 representation
///
/// `number` values are stored as `i128` on the stack (16 bytes = BID encoding).
/// `BasicValueEnum::PointerValue` carries decimal128 values through the lowering
/// pass; the runtime arithmetic functions receive `ptr` arguments.
use std::collections::{HashMap, HashSet};

use inkwell::{
    attributes::{Attribute, AttributeLoc},
    basic_block::BasicBlock,
    context::Context,
    module::Module,
    targets::{CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetTriple},
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, GlobalValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use ynz_ast::nodes::{
    BinOpKind, Expr, FunctionDecl, Item, MatchPatternKind, OwnershipModifier, Stmt, UnaryOpKind,
};
use ynz_numerics; // parse(s: &str) -> Option<u128>
use ynz_typeck::{
    build_effective_suspend_set, crossing_local_names,
    independence::{partition_independent_groups, IndependentGroup},
    type_attached_const_type, GenericFnTable, MonomorphizationTable, ShapeTable, SignatureTable,
    Type, TypedModule,
};

use crate::{
    artifact::{sha256, CompiledArtifact},
    runtime_decls::RuntimeDecls,
    shape_types::{emit_shape_types, ShapeLlvmTypes},
    state_machine,
    vtable::emit_vtable_globals,
};

// ── v0.3-M2: function-contains-wait analysis ──────────────────────────────────

/// True when any `Expr::Wait` appears anywhere in `body` (recursive, depth-first).
///
/// This is the path-selection predicate for the state-machine codegen path.
/// Functions whose body contains `Expr::Wait` are lowered via
/// `lower_function_with_waits`; all others use the standard `lower_function` path
/// with zero added overhead.
///
/// Recurses into all nested blocks (if/while/for/match arms). Does NOT cross
/// function-call boundaries — `wait` in a callee is NOT transitive in M2 (M3
/// ships the transitive predicate via call-graph analysis).
pub fn function_contains_wait(body: &ynz_ast::nodes::Block) -> bool {
    body.stmts.iter().any(stmt_contains_wait)
}

fn stmt_contains_wait(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_contains_wait(e),
        Stmt::Let { value, .. } => expr_contains_wait(value),
        Stmt::Assign { value, .. } => expr_contains_wait(value),
        Stmt::If { cond, body, .. } => expr_contains_wait(cond) || function_contains_wait(body),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_contains_wait(scrutinee)
                || arms.iter().any(|a| function_contains_wait(&a.body))
                || else_arm.as_ref().is_some_and(function_contains_wait)
        }
        Stmt::While { cond, body, .. } => expr_contains_wait(cond) || function_contains_wait(body),
        Stmt::For { iter, body, .. } => expr_contains_wait(iter) || function_contains_wait(body),
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
        // Leaf nodes (literals, idents, NoneLit, SelfValue, Error) — no nested waits.
        _ => false,
    }
}

/// Cache of `function_contains_wait` results keyed by Yinz function name.
///
/// Built once per module during `build_module` before Pass 2. Prevents re-walking
/// the AST on every call-site check during background routing and call-site dispatch.
pub type WaitCache = HashMap<String, bool>;

/// Empty WaitCache used by generic-function lowering (generics cannot contain `wait` in M2).
///
/// Process-global OnceLock avoids allocating a new empty HashMap per generic instantiation.
static EMPTY_WAIT_CACHE: std::sync::OnceLock<WaitCache> = std::sync::OnceLock::new();

fn empty_wait_cache() -> &'static WaitCache {
    EMPTY_WAIT_CACHE.get_or_init(WaitCache::new)
}

static EMPTY_SUSPEND_SET: std::sync::OnceLock<SuspendSet> = std::sync::OnceLock::new();
fn empty_suspend_set() -> &'static SuspendSet {
    EMPTY_SUSPEND_SET.get_or_init(SuspendSet::new)
}

static EMPTY_FRAME_LAYOUTS: std::sync::OnceLock<HashMap<String, FrameLayout>> =
    std::sync::OnceLock::new();
fn empty_frame_layouts() -> &'static HashMap<String, FrameLayout> {
    EMPTY_FRAME_LAYOUTS.get_or_init(HashMap::new)
}

static EMPTY_IMPORTED_FNS: std::sync::OnceLock<
    std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
> = std::sync::OnceLock::new();
fn empty_imported_fns(
) -> &'static std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig> {
    EMPTY_IMPORTED_FNS.get_or_init(std::collections::HashMap::new)
}

// Generic functions cannot reach lower_sm_block (no `wait` in generics), so an empty
// SignatureTable satisfies the struct field without allocating per-instantiation.
static EMPTY_SIG_TABLE: std::sync::OnceLock<SignatureTable> = std::sync::OnceLock::new();
fn empty_sig_table() -> &'static SignatureTable {
    EMPTY_SIG_TABLE.get_or_init(SignatureTable::empty)
}

/// Build the `WaitCache` for all non-generic functions in the module.
///
/// Generic functions are excluded because their concrete instantiations are lowered
/// separately and generics cannot contain user-written `wait` in M2 (they have no
/// concrete return type that could be awaitable).
fn build_wait_cache(typed: &TypedModule) -> WaitCache {
    let mut cache = WaitCache::new();
    for item in &typed.module.items {
        if let Item::Function(f) = item {
            if f.generics.is_empty() {
                cache.insert(f.name.clone(), function_contains_wait(&f.body));
            }
        }
    }
    cache
}

// ── v0.3-M2 Phase 7: Composed-frame layout + SuspendSet ─────────────────────

/// The set of user-defined function names that transitively reach a suspension point
/// (computed by `ynz_typeck::may_block::analyze` and stored in `FunctionSig.suspends`).
///
/// Functions in this set are compiled as state machines. Functions NOT in this set
/// compile to straight-line code with zero suspension overhead.
pub type SuspendSet = HashSet<String>;

/// Per-member slot reservation for one member of a CPU-parallel group (v0.3-M3d).
///
/// Each spawned CPU child needs two frame regions: an 8-byte handle slot (holds the
/// `*mut CpuJoinHandle` between spawn and join-Ready) and a 16-byte result slot (holds
/// the `YnzCpuResult = [i64;2]` the join writes on Ready). Both live inside the parent's
/// composed frame so the whole group shares ONE allocation — alloc-once per task tree.
///
/// Slots are keyed by `(group_id, member_index)` — NEVER by callee name. A same-callee
/// CPU pair (`let a = fib(40); let b = fib(41)`) gets two distinct slots because identity
/// is the statement position in the group, not the function called.
#[derive(Clone, Debug, PartialEq)]
pub struct CpuGroupSlot {
    /// Which parallel group within the function (0-based, source order).
    pub group_id: usize,
    /// Which member within the group (0-based, source order).
    pub member_index: usize,
    /// Byte offset of the 8-byte `*mut CpuJoinHandle` slot from the frame base.
    pub handle_offset: u64,
    /// Byte offset of the 16-byte `YnzCpuResult` slot from the frame base.
    pub result_offset: u64,
}

/// Composed-frame layout for one suspending function.
///
/// A composed frame embeds the sub-frames of all directly-called suspending children
/// at compile-time-fixed byte offsets, so the entire intra-function call tree shares
/// ONE `ynz_alloc` per spawned task.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameLayout {
    /// Total frame size: per-frame header (32 bytes) + own locals + all child sub-frames.
    pub total_size: u64,
    /// Number of own local slots (params in M2).
    pub n_locals: usize,
    /// Unique directly-called suspending callees with their byte offsets within this frame.
    ///
    /// Multiple calls to the same callee share ONE embedded slot (sequential calls
    /// never overlap; they reuse the same sub-frame). Ordered by first appearance.
    pub children: Vec<(String, u64)>,
    /// Byte offset of the recursion heap-pointer slot, when this function has a
    /// recursive edge in its call graph. The slot stores a `*mut u8` to a heap-boxed
    /// child frame (`ynz_alloc`), freed after the recursive call returns Ready.
    pub recursion_slot: Option<u64>,
    /// Byte offset of the 16-byte `number errors` staging slot, when this function returns
    /// `-> number errors` and is suspending.
    ///
    /// The slot stores the raw decimal128 i128 between the SM return-store and the wrapper
    /// read. It is placed after all own-local slots and before child sub-frames so it lives
    /// inside the single composed frame allocation (zero extra `ynz_alloc`).
    pub number_errors_staging_offset: Option<u64>,
    /// CPU-parallel-group member slots (v0.3-M3d), keyed by `(group_id, member_index)`.
    ///
    /// Empty for every function that does not contain a promoted CPU group — non-promoted
    /// functions carry an empty Vec and their layout is byte-identical to pre-M3d. The
    /// handle/result regions sit immediately after the frame header (handles first, then
    /// results) so the byte offsets are computed ONCE here instead of via the Phase-0
    /// hardcoded `SPIKE_*_OFFSET` constants at the emission site.
    pub cpu_group_slots: Vec<CpuGroupSlot>,
}

/// True when `f` is a suspending function that returns `-> number errors` (decimal128 EC).
///
/// The AST encodes `-> number errors` as `ErrorCapable { inner = Number { precision ≤ 34 } }`.
/// These functions need a 16-byte staging slot in their composed frame: the SM EC-return path
/// stores the raw i128 decimal there and points the EC `ok` word at the slot. Placing the
/// slot inside the heap frame (after own-local slots, before child sub-frames) keeps
/// alloc=1/free=1 — no separate `ynz_alloc` for the staging region.
fn is_number_errors_return(f: &FunctionDecl) -> bool {
    match &f.return_type {
        ynz_ast::nodes::Type::ErrorCapable { inner, .. } => {
            matches!(inner.as_ref(), ynz_ast::nodes::Type::Number { precision } if *precision <= 34)
        }
        _ => false,
    }
}

/// Build `FrameLayout` for every suspending function in the module.
///
/// # Algorithm (O(N²) where N = number of suspending fns — bounded in practice)
///
/// 1. Walk each local function's AST to collect its direct suspending callees in call order
///    (deduplicating per callee name).
/// 2. Pre-seed `sizes` for every imported suspending callee (in `suspend_set` but not a local
///    function) by calling `callee_size_resolver`. This must happen BEFORE step 3 so that
///    when the recursive descent encounters an imported callee, the cache entry is already
///    present and the real resolver value is used instead of the local-fn fallback path.
/// 3. Compute total_size bottom-up for local fns: leaf functions have size = header + own_locals;
///    internal nodes add the sizes of all non-recursive children (imported children now read
///    their pre-seeded resolver values from the cache).
/// 4. Build the final `FrameLayout` records for each local suspending function.
///
/// `callee_size_resolver` is called for each imported suspending callee (step 2). It returns
/// the callee's total composed frame size in bytes, or `None` to fall back to `FRAME_HEADER_SIZE`.
/// The resolver is pluggable so the same computation can be used by `frame_layouts_query`
/// (resolving recursively via `frame_layouts_query(callee_file)` for cross-module accuracy —
/// Guard G2). For a local-only module the pre-seed loop (step 2) is empty and behavior is
/// byte-identical to pre-M3e.
pub fn build_frame_layouts_with_resolver(
    typed: &TypedModule,
    suspend_set: &SuspendSet,
    shape_abi_sizes: &HashMap<String, u64>,
    callee_size_resolver: &dyn Fn(&str) -> Option<u64>,
) -> HashMap<String, FrameLayout> {
    // Step 1: collect direct suspending callees for each suspending fn.
    let mut direct_children: HashMap<String, Vec<String>> = HashMap::new();
    for item in &typed.module.items {
        let Item::Function(f) = item else { continue };
        if f.generics.is_empty() && suspend_set.contains(&f.name) {
            let callees = collect_suspending_callees(&f.body, suspend_set);
            direct_children.insert(f.name.clone(), callees);
        }
    }

    // Step 2: pre-seed imported suspending callees BEFORE compute_frame_size runs.
    //
    // Imported callees are in suspend_set but are NOT local functions (i.e., not in
    // direct_children). They must be seeded in `sizes` before the recursive descent
    // below so that when compute_frame_size("doWork") recurses into "getValue" (an
    // imported callee), it hits the cache entry on the first lookup and uses the
    // resolver's real value instead of falling through to the local-fn path (which
    // computes n_locals=0 + no children = FRAME_HEADER_SIZE and caches that stale 32).
    //
    // The sole caller for cross-module work is `frame_layouts_query`, which passes
    // `frame_layouts_query(callee_file)[name].total_size` as the resolver (Guard G2) —
    // this is what makes re-export chains (A→B→C) compose B's total_size including A's
    // real sub-frame. Falls back to FRAME_HEADER_SIZE when the resolver returns None
    // (unresolvable import; codegen is skipped on errors anyway).
    //
    // For a LOCAL-ONLY module (no imported suspending callees), every name in suspend_set
    // IS in direct_children, so this loop is empty and compute_frame_size behaves exactly
    // as before — byte-identity for intra-module codegen is preserved.
    let local_fn_names: HashSet<&str> = direct_children.keys().map(|s| s.as_str()).collect();
    let mut sizes: HashMap<String, u64> = HashMap::new();
    for name in suspend_set.iter() {
        if !local_fn_names.contains(name.as_str()) {
            let resolved =
                callee_size_resolver(name.as_str()).unwrap_or(state_machine::FRAME_HEADER_SIZE);
            sizes.insert(name.clone(), resolved);
        }
    }

    // Step 3: compute frame sizes for local fns recursively with cycle detection.
    // Imported callees are already in `sizes` (Step 2), so compute_frame_size reads
    // the resolver value from the cache on the first recursive lookup.
    let fn_names: Vec<String> = direct_children.keys().cloned().collect();
    for name in &fn_names {
        let mut visiting = HashSet::new();
        compute_frame_size(
            name,
            &direct_children,
            typed,
            suspend_set,
            shape_abi_sizes,
            &mut sizes,
            &mut visiting,
        );
    }

    // Step 4: build FrameLayout for each local fn using the fully-populated sizes map.
    let mut layouts: HashMap<String, FrameLayout> = HashMap::new();
    for item in &typed.module.items {
        let Item::Function(f) = item else { continue };
        if f.generics.is_empty() && suspend_set.contains(&f.name) {
            // Total local slots = params + crossing-local slots. Crossing locals are those
            // declared before a suspension and read after it — they must survive across
            // the resume boundary by living in the heap frame rather than SSA registers.
            // decimal128 crossing locals use 2 slots (16 bytes); all others use 1.
            let param_names_ref: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
            let suspending_refs: HashSet<&str> = suspend_set.iter().map(|s| s.as_str()).collect();
            let crossing = crossing_local_names(
                &f.body.stmts,
                &param_names_ref,
                &suspending_refs,
                &typed.expr_types,
            );
            let crossing_slots = crossing_local_total_slots(f, &crossing, typed, shape_abi_sizes);

            // v0.3-M3d CPU-group slots. A promoted host (admitted by spike_cpu_candidates)
            // reserves a handle+result region immediately after the frame header, BEFORE its
            // own-local slots. Handles come first (8 bytes each), then results (16 bytes each),
            // matching the runtime drop-shim contract (`cleanup_spike_cpu_handles` reads handle
            // slots at frame offsets 32/40). The offsets are computed here ONCE; the emission
            // site reads them from `cpu_group_slots` instead of hardcoded SPIKE_*_OFFSET consts.
            //
            // The Phase-0 envelope admits exactly one group of two members on a zero-param
            // entrypoint, so this produces handles @ 32/40 and results @ 48/64. The general
            // multi-group / N-member layout is a later slice; the keying (group_id,
            // member_index) is already shape-correct for it.
            let (cpu_group_slots, cpu_reserve) = cpu_group_slots_and_reserve(f, typed, suspend_set);

            let n_locals = f.params.len() + crossing_slots;
            // own_base (start of own-local slots) is pushed past the CPU-slot reserve so
            // crossing locals never alias a handle/result slot.
            let own_base = state_machine::FRAME_HEADER_SIZE
                + cpu_reserve
                + state_machine::own_locals_size(n_locals);
            let children_raw = direct_children.get(&f.name).cloned().unwrap_or_default();

            // Reserve a 16-byte staging slot after own-local slots when the function returns
            // `-> number errors` (decimal128 EC). The slot is placed before child sub-frames
            // so it remains part of the single composed frame allocation (alloc=1/free=1).
            let number_errors_staging_offset = if is_number_errors_return(f) {
                Some(own_base)
            } else {
                None
            };
            // Child sub-frames start after the optional staging slot.
            let children_start = own_base + number_errors_staging_offset.map_or(0, |_| 16);

            // Build child offset list, detecting recursion edges.
            let mut children: Vec<(String, u64)> = Vec::new();
            let mut recursion_slot: Option<u64> = None;
            let mut cursor = children_start;

            // Detect which children are recursive (cycle in the call graph).
            // Simple heuristic: a child is "recursive" if its name == the current fn OR
            // if there's no size computed for it (size would be infinite if truly recursive).
            for callee in &children_raw {
                if callee == &f.name || !sizes.contains_key(callee.as_str()) {
                    // Recursion edge: store an 8-byte heap-pointer slot instead of embedding.
                    if recursion_slot.is_none() {
                        recursion_slot = Some(cursor);
                        cursor += 8; // one pointer slot
                    }
                    // Don't embed the recursive child.
                } else {
                    let child_size = *sizes
                        .get(callee.as_str())
                        .unwrap_or(&state_machine::FRAME_HEADER_SIZE);
                    children.push((callee.clone(), cursor));
                    cursor += child_size;
                }
            }

            let total_size = cursor;

            // Lock the two frame-size computations together: `total_size` here MUST equal
            // the value `compute_frame_size` cached in `sizes` for this fn (read as
            // `child_frame_size` when a parent embeds this fn as a composed callee). Both
            // now route their CPU reserve through `cpu_group_slots_and_reserve`, so a spike
            // fn's own-base — and therefore its total — agrees on both paths. If a future
            // edit re-diverges them, this fires in debug builds before a parent can
            // under-allocate an embedded spike child and alias its next sub-frame.
            debug_assert_eq!(
                sizes.get(&f.name).copied(),
                Some(total_size),
                "frame-size divergence for {}: build_frame_layouts={total_size}, \
                 compute_frame_size memo={:?} — the CPU reserve must match on both paths",
                f.name,
                sizes.get(&f.name).copied(),
            );

            layouts.insert(
                f.name.clone(),
                FrameLayout {
                    total_size,
                    n_locals,
                    children,
                    recursion_slot,
                    number_errors_staging_offset,
                    cpu_group_slots,
                },
            );
        }
    }
    layouts
}

/// Number of members in the Phase-0 CPU group (one adjacent pair). The general N-member
/// group is a later slice; the per-member slot layout below is already keyed for it.
const CPU_GROUP_MEMBER_COUNT: usize = 2;
/// Bytes per CPU-child handle slot (`*mut CpuJoinHandle`).
const CPU_HANDLE_SLOT_BYTES: u64 = 8;
/// Bytes per CPU-child result slot (`YnzCpuResult = [i64; 2]`).
const CPU_RESULT_SLOT_BYTES: u64 = 16;

/// Compute the per-member CPU-group slot offsets for `member_count` members of group 0.
///
/// Layout (immediately after the 32-byte frame header): all handle slots first (8 bytes
/// each), then all result slots (16 bytes each). For the Phase-0 two-member case this is
/// handles @ 32/40 and results @ 48/64 — byte-identical to the hardcoded SPIKE_*_OFFSET
/// constants. Returns an empty Vec when `member_count == 0` (non-promoted function).
///
/// Time: O(m)  Space: O(m) where m = member_count.
fn build_cpu_group_slots(member_count: usize) -> Vec<CpuGroupSlot> {
    if member_count == 0 {
        return Vec::new();
    }
    let handles_base = state_machine::FRAME_HEADER_SIZE;
    let results_base = handles_base + (member_count as u64) * CPU_HANDLE_SLOT_BYTES;
    (0..member_count)
        .map(|m| CpuGroupSlot {
            group_id: 0,
            member_index: m,
            handle_offset: handles_base + (m as u64) * CPU_HANDLE_SLOT_BYTES,
            result_offset: results_base + (m as u64) * CPU_RESULT_SLOT_BYTES,
        })
        .collect()
}

/// Byte span (header → end of the last result slot) that a CPU group's handle/result
/// region occupies, for the given slot list. Zero when the slice is empty. This is the
/// single source of truth both frame-size computations use to push own-local slots and
/// composed-child sub-frames past the CPU reserve.
///
/// Time: O(m)  Space: O(1) where m = slots.len().
fn cpu_reserve_bytes(slots: &[CpuGroupSlot]) -> u64 {
    slots
        .iter()
        .map(|s| {
            (s.result_offset + CPU_RESULT_SLOT_BYTES)
                .saturating_sub(state_machine::FRAME_HEADER_SIZE)
        })
        .max()
        .unwrap_or(0)
}

/// CPU-group slots + reserve bytes for `f`, the single computation both frame-size paths
/// share. A function `spike_cpu_candidates` admits reserves a handle+result region after
/// the frame header (per `CPU_GROUP_MEMBER_COUNT`); all others reserve nothing. Binding
/// `build_frame_layouts_with_resolver` and `compute_frame_size` to ONE helper is what
/// keeps the `sizes` memo (read as a composed child's `child_frame_size`) in lockstep with
/// the host's own `total_size` — a divergence here under-allocates an embedded spike child
/// by the reserve and aliases the parent's next sub-frame (silent heap corruption).
///
/// Time: O(k)  Space: O(m) where k = AST nodes scanned by `spike_cpu_candidates`,
/// m = group members.
fn cpu_group_slots_and_reserve(
    f: &FunctionDecl,
    typed: &TypedModule,
    suspend_set: &SuspendSet,
) -> (Vec<CpuGroupSlot>, u64) {
    let member_count = spike_cpu_candidates(f, typed, suspend_set)
        .map(|_| CPU_GROUP_MEMBER_COUNT)
        .unwrap_or(0);
    let slots = build_cpu_group_slots(member_count);
    let reserve = cpu_reserve_bytes(&slots);
    (slots, reserve)
}

/// Number of 8-byte slots the CPU-group handle/result region occupies after the frame
/// header, derived from a layout's `cpu_group_slots`. Zero when the function has no CPU
/// group. Used by `lower_function_with_waits` to place crossing-local slots past the
/// reserve without re-deriving the byte math.
///
/// Time: O(m)  Space: O(1) where m = group members.
fn cpu_slot_reserve_slots(layout: &FrameLayout) -> usize {
    let max_end = layout
        .cpu_group_slots
        .iter()
        .map(|s| s.result_offset + CPU_RESULT_SLOT_BYTES)
        .max();
    match max_end {
        Some(end) => ((end - state_machine::FRAME_HEADER_SIZE) / CPU_HANDLE_SLOT_BYTES) as usize,
        None => 0,
    }
}

/// Collect the unique suspending callee names called directly in `block` (in call order).
///
/// Deduplicates by callee name — multiple calls to the same callee share one embedded
/// sub-frame slot (sequential calls never overlap). This is O(N) where N = AST nodes.
fn collect_suspending_callees(
    block: &ynz_ast::nodes::Block,
    suspend_set: &SuspendSet,
) -> Vec<String> {
    let mut callees: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    collect_callees_in_block(block, suspend_set, &mut callees, &mut seen);
    callees
}

fn collect_callees_in_block(
    block: &ynz_ast::nodes::Block,
    suspend_set: &SuspendSet,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for stmt in &block.stmts {
        collect_callees_in_stmt(stmt, suspend_set, out, seen);
    }
}

fn collect_callees_in_stmt(
    stmt: &Stmt,
    suspend_set: &SuspendSet,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match stmt {
        Stmt::Expr(e) | Stmt::Return { value: Some(e), .. } => {
            collect_callees_in_expr(e, suspend_set, out, seen)
        }
        Stmt::Let { value: e, .. } | Stmt::Assign { value: e, .. } => {
            collect_callees_in_expr(e, suspend_set, out, seen)
        }
        Stmt::FieldAssign { target, value, .. } => {
            collect_callees_in_expr(target, suspend_set, out, seen);
            collect_callees_in_expr(value, suspend_set, out, seen);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_callees_in_expr(receiver, suspend_set, out, seen);
            collect_callees_in_expr(index, suspend_set, out, seen);
            collect_callees_in_expr(value, suspend_set, out, seen);
        }
        Stmt::If { cond, body, .. } => {
            collect_callees_in_expr(cond, suspend_set, out, seen);
            collect_callees_in_block(body, suspend_set, out, seen);
        }
        Stmt::While { cond, body, .. } => {
            collect_callees_in_expr(cond, suspend_set, out, seen);
            collect_callees_in_block(body, suspend_set, out, seen);
        }
        Stmt::For { iter, body, .. } => {
            collect_callees_in_expr(iter, suspend_set, out, seen);
            collect_callees_in_block(body, suspend_set, out, seen);
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_callees_in_expr(scrutinee, suspend_set, out, seen);
            for arm in arms {
                collect_callees_in_block(&arm.body, suspend_set, out, seen);
            }
            if let Some(b) = else_arm {
                collect_callees_in_block(b, suspend_set, out, seen);
            }
        }
        Stmt::Return { value: None, .. } => {}
    }
}

fn collect_callees_in_expr(
    expr: &Expr,
    suspend_set: &SuspendSet,
    out: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match expr {
        Expr::Call(c) => {
            if let Expr::Ident(name, _) = &c.callee {
                if suspend_set.contains(name.as_str())
                    && !M2_MAY_BLOCK_INTRINSICS.contains(&name.as_str())
                    && seen.insert(name.clone())
                {
                    out.push(name.clone());
                }
            }
            // Recurse into args
            for arg in &c.args {
                collect_callees_in_expr(arg, suspend_set, out, seen);
            }
        }
        Expr::Wait(inner, _) => collect_callees_in_expr(inner, suspend_set, out, seen),
        Expr::Background(inner, _) => {
            // background calls don't embed — they get their own alloc via ynz_rt_spawn.
            let _ = inner;
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_callees_in_expr(lhs, suspend_set, out, seen);
            collect_callees_in_expr(rhs, suspend_set, out, seen);
        }
        Expr::UnaryOp { operand, .. } => collect_callees_in_expr(operand, suspend_set, out, seen),
        Expr::MethodCall { receiver, args, .. } => {
            collect_callees_in_expr(receiver, suspend_set, out, seen);
            for a in args {
                collect_callees_in_expr(a, suspend_set, out, seen);
            }
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_callees_in_expr(receiver, suspend_set, out, seen)
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_callees_in_expr(receiver, suspend_set, out, seen);
            collect_callees_in_expr(index, suspend_set, out, seen);
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_callees_in_expr(&f.value, suspend_set, out, seen);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_callees_in_expr(e, suspend_set, out, seen);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_callees_in_expr(k, suspend_set, out, seen);
                collect_callees_in_expr(v, suspend_set, out, seen);
            }
        }
        Expr::InterpolatedString(parts, _) => {
            for p in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = p {
                    collect_callees_in_expr(e, suspend_set, out, seen);
                }
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_callees_in_expr(receiver, suspend_set, out, seen)
        }
        Expr::Is { expr: inner, .. } => collect_callees_in_expr(inner, suspend_set, out, seen),
        // Leaf nodes
        _ => {}
    }
}

/// Recursively compute the total frame size for `fn_name`, memoizing in `sizes`.
///
/// `visiting` tracks the ancestor path to detect recursion cycles.
fn compute_frame_size(
    fn_name: &str,
    direct_children: &HashMap<String, Vec<String>>,
    typed: &TypedModule,
    suspend_set: &SuspendSet,
    shape_abi_sizes: &HashMap<String, u64>,
    sizes: &mut HashMap<String, u64>,
    visiting: &mut HashSet<String>,
) -> u64 {
    if let Some(&cached) = sizes.get(fn_name) {
        return cached;
    }
    if visiting.contains(fn_name) {
        // Recursion — return 0 as sentinel; the caller will use a heap-pointer slot instead.
        return 0;
    }
    visiting.insert(fn_name.to_string());

    // Find n_locals, staging requirements, and the CPU-group reserve for this fn.
    //
    // The CPU reserve MUST match `build_frame_layouts_with_resolver`'s own-base computation
    // for the same fn — this `sizes` memo is read as `child_frame_size` when a parent embeds
    // this fn as a composed callee. A reserve omitted here under-allocates an embedded spike
    // child by the reserve, aliasing the parent's next sub-frame (silent heap corruption).
    // Both paths call `cpu_group_slots_and_reserve`, so they can never diverge.
    let (n_locals, needs_number_errors_staging, cpu_reserve) = typed
        .module
        .items
        .iter()
        .find_map(|item| {
            if let Item::Function(f) = item {
                if f.name == fn_name {
                    let param_names: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
                    let suspending_refs: HashSet<&str> =
                        suspend_set.iter().map(|s| s.as_str()).collect();
                    let crossing = crossing_local_names(
                        &f.body.stmts,
                        &param_names,
                        &suspending_refs,
                        &typed.expr_types,
                    );
                    let crossing_slots =
                        crossing_local_total_slots(f, &crossing, typed, shape_abi_sizes);
                    let (_, reserve) = cpu_group_slots_and_reserve(f, typed, suspend_set);
                    return Some((
                        f.params.len() + crossing_slots,
                        is_number_errors_return(f),
                        reserve,
                    ));
                }
            }
            None
        })
        .unwrap_or((0, false, 0));

    let own_base =
        state_machine::FRAME_HEADER_SIZE + cpu_reserve + state_machine::own_locals_size(n_locals);
    // Include the 16-byte `number errors` staging slot in the own-locals region when needed.
    let staging_size = if needs_number_errors_staging { 16 } else { 0 };
    let mut total = own_base + staging_size;

    if let Some(children) = direct_children.get(fn_name) {
        let mut seen_recursive: HashSet<String> = HashSet::new();
        for child in children {
            if child == fn_name || visiting.contains(child.as_str()) {
                // Recursive edge: add 8 bytes for the heap-pointer slot (once).
                if seen_recursive.insert(child.clone()) {
                    total += 8;
                }
            } else {
                let child_size = compute_frame_size(
                    child,
                    direct_children,
                    typed,
                    suspend_set,
                    shape_abi_sizes,
                    sizes,
                    visiting,
                );
                if child_size == 0 {
                    // Child had a cycle; treat as recursive pointer.
                    if seen_recursive.insert(child.clone()) {
                        total += 8;
                    }
                } else {
                    total += child_size;
                }
            }
        }
    }

    visiting.remove(fn_name);
    sizes.insert(fn_name.to_string(), total);
    total
}

/// The set of free-function intrinsic names that are may-block (can contain suspension points).
///
/// M2 uses a fixed 2-element set. M3 will replace this with a transitive call-graph analysis
/// pass, at which point this constant can be removed.
///
/// The cost of keeping this as an in-code constant (rather than a registry field) is explicit:
/// every new may-block intrinsic added before M3 must edit this list. Caught at code review.
// CARVE-OUT: compiler-internal constant — predicate for M2 sleep dispatch in codegen.
// Not a user-facing feature. M3's transitive analysis replaces this entirely.
const M2_MAY_BLOCK_INTRINSICS: &[&str] = &["sleep", "__testFallibleAsync"];

// is_may_block_callee (local-syntactic predicate) removed in P7.
// The SM-selection predicate is now SuspendSet (transitive, from typeck).
// Kept only as a dead-code stub; M3 will remove it entirely.

/// The file ID embedded in the LLVM module for deterministic IR and object output.
pub fn module_identifier(source_path: &str) -> String {
    format!("ynz-{source_path}")
}

/// Emit a relocatable object file for an M5 program.
///
/// `frame_layouts` must come from `frame_layouts_query` (built with the same target machine
/// as `emit_artifact` — Guard G1) so the emitter uses the identical layout the importer
/// reads when it calls `frame_layouts_query(callee_file)`. This is the single-source-of-truth
/// guarantee: one computation, consumed by both the emitter and future cross-module importers.
#[allow(clippy::too_many_arguments)]
pub fn emit_artifact(
    source_path: &str,
    typed_module: &TypedModule,
    shape_table: &ShapeTable,
    sig_table: &SignatureTable,
    generic_fn_table: &GenericFnTable,
    mono_table: &MonomorphizationTable,
    target_triple: Option<&str>,
    imported_options: &std::collections::HashMap<String, ynz_typeck::options_table::OptionsEntry>,
    suspends_set_arg: &HashSet<String>,
    imported_fns: &std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    frame_layouts: &HashMap<String, FrameLayout>,
    cpu_promoted: &HashSet<String>,
) -> Result<CompiledArtifact, String> {
    let context = Context::create();
    let module_id = module_identifier(source_path);
    let module = context.create_module(&module_id);

    // Use the shared target-machine constructor for the default triple (Guard G1: same
    // triple/CPU/data-layout as frame_layouts_query — byte-identical shape ABI sizes
    // between the emitter and the query). For explicit target_triple overrides (cross-
    // compilation and tests), construct the machine from the supplied triple directly.
    let machine = match target_triple {
        None => crate::state_machine::default_target_machine()?,
        Some(t) => {
            // Override-branch init: default_target_machine() handles it for the None branch.
            Target::initialize_x86(&InitializationConfig::default());
            let triple = TargetTriple::create(t);
            module.set_triple(&triple);
            let target = Target::from_triple(&triple)
                .map_err(|e| format!("LLVM: no target for triple {:?}: {e}", triple.as_str()))?;
            target
                .create_target_machine(
                    &triple,
                    "generic",
                    "",
                    OptimizationLevel::None,
                    RelocMode::Default,
                    CodeModel::Default,
                )
                .ok_or_else(|| "LLVM: failed to create target machine".to_string())?
        }
    };
    // Always set triple and data-layout from the machine (the shared constructor uses the
    // default triple; the override branch already set the triple above).
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());

    // Use the suspends_set passed in from check_query (computed by may_block::analyze).
    // This is the Phase-7 seam fix: the pre-analysis sig_table (from module_signatures_query)
    // has suspends=false for all fns; the real transitive set comes from check_query.
    let suspend_set: SuspendSet = suspends_set_arg.clone();
    let _ = sig_table; // sig_table kept in signature for API compatibility

    // Read the auto-parallel kill switch set by main.rs before the salsa dispatch.
    // The env var is set once before the first salsa call in the CLI path, so the
    // process-per-build model keeps the memo valid for the lifetime of the process.
    //
    // Latent hazard: `ynz watch` (long-lived) and LSP (incremental) would NOT invalidate
    // the memoized codegen_query when this env var changes between rebuilds — salsa has
    // no visibility into env vars. The correct fix is to thread `no_auto_parallel` as an
    // explicit salsa input parameter. Deferred until `ynz watch --no-auto-parallel` or
    // LSP codegen integration lands. Tracked: .claude/todos.md "no-auto-parallel env-var".
    let no_auto_parallel = ynz_typeck::no_auto_parallel_env();
    // v0.3-M3d CPU-statement parallelization trigger. `cpu_promoted` is the typeck
    // promotion set (`cpu_promotion_query`) — the single source of truth that drives the
    // suspend-set extension, the inlay hints, and codegen routing per the registry's
    // "hint and binary always agree" contract. A non-empty set means at least one function
    // contains a CPU-parallelizable group, so the per-function spike-candidacy probe in
    // `lower_function_with_waits` must run; when empty, no program lowers a CPU group.
    //
    // The promoted functions are ALREADY in `suspend_set` (unioned at the codegen_query
    // boundary), so they route through `lower_function_with_waits` automatically via the
    // existing `suspend_set.contains` dispatch — no separate suspend-set extension here.
    let m3d_spike = !cpu_promoted.is_empty();

    build_module(
        &context,
        &module,
        source_path,
        typed_module,
        shape_table,
        sig_table,
        generic_fn_table,
        mono_table,
        imported_options,
        &suspend_set,
        imported_fns,
        frame_layouts,
        no_auto_parallel,
        m3d_spike,
    )?;

    module
        .verify()
        .map_err(|e| format!("LLVM module verify failed: {}", e.to_string()))?;

    let ir_text = module.print_to_string().to_string();
    let obj_buf = machine
        .write_to_memory_buffer(&module, FileType::Object)
        .map_err(|e| format!("LLVM: failed to write object: {}", e.to_string()))?;
    let object_bytes = obj_buf.as_slice().to_vec();
    let hash = sha256(&object_bytes);

    Ok(CompiledArtifact {
        object_bytes,
        ir_text,
        sha256: hash,
    })
}

struct ModuleGlobals<'ctx> {
    str_true: GlobalValue<'ctx>,
    str_false: GlobalValue<'ctx>,
    dec_zero: GlobalValue<'ctx>,
    panic_int_add: GlobalValue<'ctx>,
    panic_int_sub: GlobalValue<'ctx>,
    panic_int_mul: GlobalValue<'ctx>,
    panic_int_div: GlobalValue<'ctx>,
    panic_int_rem: GlobalValue<'ctx>,
    panic_dec_div: GlobalValue<'ctx>,
    panic_dec_rem: GlobalValue<'ctx>,
    /// Source file path embedded as a C string for runtime panic messages.
    source_file: GlobalValue<'ctx>,
}

/// Emit LLVM IR for one Yinz source module.
///
/// # Flow (6 passes — order is mandatory)
///
/// | Pass | What | Requires from prior passes |
/// |------|------|---------------------------|
/// | 0 | Emit LLVM struct types for all user-defined shapes | nothing |
/// | 0.25 | Forward-declare imported (cross-module) functions as external LLVM declarations | Pass 0 (shape types for param/return type mapping) |
/// | 1 | Forward-declare every non-generic function | Pass 0 (shape types used in param/return types) |
/// | 1.5 | Forward-declare monomorphized generic functions | Pass 0 + Pass 1 (non-generic functions may be called from generic bodies) |
/// | 1.6 | Emit vtable globals for `dynamic Foo` dispatch | Pass 1 (vtable entries point to forward-declared function values) |
/// | 2 | Emit non-generic function bodies (and lower generic instances) | All prior passes (bodies call functions, construct shapes, dispatch via vtables) |
///
/// Generic functions are lowered during Pass 2 by iterating `mono_table` entries, not
/// by walking the AST's `Item::Function` list — by Pass 2 every generic call site has
/// already been collected into `mono_table` by the typeck pass.
#[allow(clippy::too_many_arguments)]
fn build_module<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    source_path: &str,
    typed: &'g TypedModule,
    shape_table: &'g ShapeTable,
    sig_table: &'g SignatureTable,
    generic_fn_table: &'g GenericFnTable,
    mono_table: &'g MonomorphizationTable,
    imported_options: &std::collections::HashMap<String, ynz_typeck::options_table::OptionsEntry>,
    suspend_set: &'g SuspendSet,
    imported_fns: &std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    frame_layouts_arg: &HashMap<String, FrameLayout>,
    no_auto_parallel: bool,
    m3d_spike: bool,
) -> Result<(), String> {
    let rt = RuntimeDecls::declare(ctx, module);

    // M6: collect options table for variant tag lookups during codegen.
    let mut options_diags = ynz_diagnostics::DiagnosticBucket::new();
    let mut options_table =
        ynz_typeck::options_table::collect_options(&typed.module, &mut options_diags);
    // Merge imported options so cross-file options types work in codegen
    // (e.g. `Timeframe.daily` where Timeframe is imported from another file).
    for (name, entry) in imported_options {
        options_table
            .options
            .entry(name.clone())
            .or_insert_with(|| entry.clone());
    }

    let zero_bits = ynz_numerics::parse("0").expect("decimal zero parse");
    let globals = ModuleGlobals {
        str_true: build_string_global(ctx, module, "true", ".str.true"),
        str_false: build_string_global(ctx, module, "false", ".str.false"),
        dec_zero: build_decimal_global(ctx, module, zero_bits, ".dec.zero"),
        panic_int_add: build_string_global(ctx, module, "int overflow in '+'", ".panic.iadd"),
        panic_int_sub: build_string_global(ctx, module, "int overflow in '-'", ".panic.isub"),
        panic_int_mul: build_string_global(ctx, module, "int overflow in '*'", ".panic.imul"),
        panic_int_div: build_string_global(ctx, module, "division by zero (int)", ".panic.idiv"),
        panic_int_rem: build_string_global(ctx, module, "remainder by zero (int)", ".panic.irem"),
        panic_dec_div: build_string_global(ctx, module, "division by zero (number)", ".panic.ddiv"),
        panic_dec_rem: build_string_global(
            ctx,
            module,
            "remainder by zero (number)",
            ".panic.drem",
        ),
        source_file: build_string_global(ctx, module, source_path, ".source.file"),
    };

    // Pass 0 — emit LLVM struct types for all user-defined shapes.
    let shape_types = emit_shape_types(ctx, shape_table);

    // Pass 0.25 — forward-declare imported (cross-module) functions as LLVM external declarations.
    //
    // Each imported function lives in another translation unit (another .o file). The linker
    // resolves the reference at link time. Without this pass, `get_function(name)` in Pass 2
    // returns None for cross-module calls, producing a codegen error.
    //
    // For suspending imported functions: the SM inline-poll mechanism calls the callee's
    // RESUME FUNCTION (`ynz_sm_<name>_resume`), not the outer wrapper. Both declarations
    // are emitted here — the wrapper for non-SM callers, the resume fn for SM callers.
    for (local_name, sig) in imported_fns {
        // Use the exported symbol name for the LLVM declaration. When the import is aliased
        // (`import { getValue as fetchVal }`), the exporting module compiled and exported
        // `getValue` — the LLVM external declaration must use that name so the linker
        // resolves the reference to the exporting module's object file. The local alias
        // name is used only by the call-site name lookup (sig_table key, frame_layouts key).
        let llvm_name = sig.original_name.as_deref().unwrap_or(local_name.as_str());
        if module.get_function(llvm_name).is_none() {
            let ptr = ctx.ptr_type(AddressSpace::default());
            let param_types: Vec<BasicMetadataTypeEnum<'ctx>> = sig
                .params
                .iter()
                .map(|(_, ty)| match ty {
                    Type::Int => ctx.i64_type().into(),
                    Type::Float => ctx.f64_type().into(),
                    Type::Bool => ctx.bool_type().into(),
                    Type::Number { precision } if *precision <= 34 => ctx.i128_type().into(),
                    _ => ptr.into(),
                })
                .collect();
            let fn_ty = match &sig.ret {
                Type::Nothing => ctx.void_type().fn_type(&param_types, false),
                Type::Int => ctx.i64_type().fn_type(&param_types, false),
                Type::Float => ctx.f64_type().fn_type(&param_types, false),
                Type::Bool => ctx.bool_type().fn_type(&param_types, false),
                Type::Number { precision } if *precision <= 34 => {
                    ctx.i128_type().fn_type(&param_types, false)
                }
                // Errors-capable functions return `{i64, i64}` — the same ABI as the
                // errors_result_type struct. Using ptr here produces an ABI mismatch:
                // the importer reads an i64 where the callee returns a {i64,i64} struct,
                // silently returning 0 instead of the real value.
                Type::ErrorsCapable { .. } => errors_result_type(ctx).fn_type(&param_types, false),
                _ => ptr.fn_type(&param_types, false),
            };
            module.add_function(llvm_name, fn_ty, None);
        }

        // For suspending imported functions, also declare the resume function.
        // SM callers call `ynz_sm_<name>_resume` for inline poll-yield rather than
        // the wrapper — without this declaration, `emit_suspending_call` panics with
        // "resume fn not declared". The resume function uses the original exported name
        // (not the alias) because state_machine::resume_fn_name derives from the LLVM
        // symbol, which is the original name in the exporting module's object file.
        if sig.suspends {
            let resume_name = state_machine::resume_fn_name(llvm_name);
            if module.get_function(&resume_name).is_none() {
                state_machine::declare_resume_fn(ctx, module, &resume_name);
            }
        }
    }

    // Compute ABI byte sizes for shapes using LLVM TargetData (the authoritative layout
    // source, same as the memcpy size used in lower_function_with_waits). Stored as a
    // plain HashMap<name, bytes> so frame-layout computation (no LLVM context) and codegen
    // (has LLVM context) share one source of truth.
    //
    // Prior impl used `struct_ty.size_of().get_zero_extended_constant()`, which fails
    // for GEP-based size constants (returns None for non-trivial structs) — causing the
    // frame-layout fallback to 1 slot per shape, then an out-of-bounds frame write on
    // shapes with 2+ slots (e.g. Point{x,y} = 2 slots = 16 bytes). Fixed by using
    // TargetData::get_abi_size which always returns the real byte count.
    let shape_abi_sizes: HashMap<String, u64> = {
        let dl_owned = module.get_data_layout();
        let dl_str = dl_owned.as_str().to_str().unwrap_or("");
        let target_data = inkwell::targets::TargetData::create(dl_str);
        shape_types
            .named
            .iter()
            .map(|(name, &struct_ty)| {
                let bytes = target_data.get_abi_size(&struct_ty);
                (name.clone(), bytes)
            })
            .collect()
    };

    // WHY: single SSOT for the effective suspend set — local + imported suspending
    // names.  `build_effective_suspend_set` is the canonical computation consumed by
    // the frame-layout query, this routing gate, and the IDE hint passes, ensuring they
    // all agree on which callees are suspending.
    //
    // Without the imported names:
    //   1. `build_frame_layouts` skips cross-module child sub-frames → heap-boxed
    //      fallback paths with SSA dominator bugs.
    //   2. `is_direct_suspending_call` misses imported suspending fns → calls the SM
    //      wrapper directly inside a resume body → "Cannot start a runtime from within
    //      a runtime" panic at runtime.
    let effective_suspend_set = build_effective_suspend_set(suspend_set, imported_fns);
    let suspend_set = &effective_suspend_set;

    // Pass 0.5 — build the wait cache (kept for backward-compat with generic lowering +
    // background routing) AND bind the pre-computed frame layouts.
    //
    // The wait_cache still serves the local-syntactic check used by non-SM call sites.
    // frame_layouts comes from frame_layouts_query (pre-computed, LLVM-accurate): it
    // encodes the composed structure (embedded child sub-frames) used by
    // lower_function_with_waits to allocate ONE frame per task tree. Using the query
    // result here ensures the emitter and any future cross-module importers read the
    // identical layout (single source of truth — Guard G1 + G2 integrity).
    let wait_cache = build_wait_cache(typed);
    let frame_layouts = frame_layouts_arg;

    // Pass 0.6 — forward-declare resume functions for ALL SUSPENDING functions.
    // Phase 7: use suspend_set (transitive) instead of wait_cache (local) so fns that
    // reach `sleep` transitively (without explicit `wait`) get a resume fn declared.
    for item in &typed.module.items {
        if let Item::Function(f) = item {
            if f.generics.is_empty() && suspend_set.contains(&f.name) {
                let resume_name = state_machine::resume_fn_name(&f.name);
                state_machine::declare_resume_fn(ctx, module, &resume_name);
            }
        }
    }

    // Pass 1 — forward-declare every non-generic function so vtables and bodies can reference them.
    for item in &typed.module.items {
        match item {
            Item::Function(f) if f.generics.is_empty() => {
                declare_function(ctx, module, f, shape_table)?
            }
            Item::Function(_)
            | Item::ShapeDecl(_)
            | Item::OptionsDecl(_)
            | Item::ImportDecl(_)
            | Item::ConstDecl(_)
            | Item::ReExport(_) => {}
        }
    }

    // Pass 1.5 — forward-declare monomorphized generic functions.
    for (key, mono_sig) in &mono_table.entries {
        let mangled = mangle_mono_name(&key.fn_name, &key.type_args);
        let param_llvms: Vec<BasicMetadataTypeEnum<'ctx>> = mono_sig
            .param_types
            .iter()
            .map(|t| {
                llvm_type_for_ctx(ctx, t)
                    .map(BasicMetadataTypeEnum::from)
                    .unwrap_or_else(|| ctx.i64_type().into())
            })
            .collect();
        let fn_ty = match llvm_type_for_ctx(ctx, &mono_sig.ret_type) {
            Some(ret) => ret.fn_type(&param_llvms, false),
            None => ctx.void_type().fn_type(&param_llvms, false),
        };
        if module.get_function(&mangled).is_none() {
            module.add_function(&mangled, fn_ty, None);
        }
    }

    // Pass 1.6 — emit vtable globals for `dynamic Foo` dispatch.
    let _vtables = emit_vtable_globals(module, shape_table);

    // Pass 2 — emit non-generic function bodies.
    for item in &typed.module.items {
        match item {
            Item::Function(f) if f.generics.is_empty() => lower_function(
                ctx,
                module,
                &rt,
                &globals,
                typed,
                f,
                shape_table,
                &shape_types,
                &shape_abi_sizes,
                mono_table,
                &options_table,
                &wait_cache,
                suspend_set,
                frame_layouts,
                imported_fns,
                sig_table,
                no_auto_parallel,
                m3d_spike,
            )?,
            Item::Function(_)
            | Item::ShapeDecl(_)
            | Item::OptionsDecl(_)
            | Item::ImportDecl(_)
            | Item::ConstDecl(_)
            | Item::ReExport(_) => {}
        }
    }

    // Pass 2.5 — emit monomorphized generic function bodies.
    for (key, mono_sig) in &mono_table.entries {
        let Some(fn_decl) = typed.module.items.iter().find_map(|item| {
            if let Item::Function(f) = item {
                if f.name == key.fn_name && !f.generics.is_empty() {
                    return Some(f);
                }
            }
            None
        }) else {
            continue;
        };

        let type_subst: HashMap<String, Type> = fn_decl
            .generics
            .iter()
            .zip(key.type_args.iter())
            .map(|(gp, ty)| (gp.name.clone(), ty.clone()))
            .collect();

        let mangled = mangle_mono_name(&key.fn_name, &key.type_args);
        let Some(fn_val) = module.get_function(&mangled) else {
            continue;
        };

        lower_generic_function(
            ctx,
            module,
            &rt,
            &globals,
            typed,
            fn_decl,
            shape_table,
            &shape_types,
            fn_val,
            type_subst,
            mono_sig,
            mono_table,
            &options_table,
        )?;
    }

    let _ = generic_fn_table; // consumed via mono_table; kept in signature for future use
    Ok(())
}

/// Mangle a generic function name + type args into a unique LLVM symbol name.
fn mangle_mono_name(fn_name: &str, type_args: &[Type]) -> String {
    let mut name = fn_name.to_string();
    for ty in type_args {
        name.push('_');
        name.push_str(&mangle_type(ty));
    }
    name
}

fn mangle_type(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "boolean".to_string(),
        Type::String => "string".to_string(),
        Type::Nothing => "nothing".to_string(),
        Type::Shape { name } => format!("shape_{name}"),
        Type::Dynamic { contract } => format!("dyn_{contract}"),
        Type::BuiltinArray { elem } => format!("array_{}", mangle_type(elem)),
        Type::BuiltinFixed { elem, size } => {
            format!("fixed_{}_{}", size.unwrap_or(0), mangle_type(elem))
        }
        Type::Maybe { inner } => format!("maybe_{}", mangle_type(inner)),
        Type::Generic { name, args } => {
            let arg_str = args.iter().map(mangle_type).collect::<Vec<_>>().join("_");
            format!("{name}_{arg_str}")
        }
        Type::TypeParam { name } => format!("tparam_{name}"),
        Type::Number { precision } => format!("number_{precision}"),
        Type::Range {
            element,
            end_inclusive,
        } => {
            format!(
                "range_{}{}",
                mangle_type(element),
                if *end_inclusive { "_inc" } else { "" }
            )
        }
        Type::BuiltinMap { key, val } => {
            format!("map_{}_{}", mangle_type(key), mangle_type(val))
        }
        Type::MapEntry { key, val } => {
            format!("mapentry_{}_{}", mangle_type(key), mangle_type(val))
        }
        Type::Options { name } => format!("options_{name}"),
        Type::Union { variants } => {
            let parts: Vec<_> = variants.iter().map(mangle_type).collect();
            format!("union_{}", parts.join("_or_"))
        }
        Type::ErrorsCapable { inner } => format!("errors_{}", mangle_type(inner)),
        Type::Sensitive { inner } => format!("sensitive_{}", mangle_type(inner)),
        // Type::Error should never reach codegen — an earlier phase should have
        // caught all type errors and stopped compilation. Panic here so it's
        // visible immediately if it ever does (instead of silently emitting a
        // mangled name that causes a mysterious linker error).
        Type::Error => {
            panic!("Type::Error reached mangle_type — compilation should have stopped at typeck")
        }
    }
}

/// Map a typeck `Type` to an LLVM basic type, given only a `Context`.
///
/// Used during Pass 1.5 (forward declarations) where `Cg` doesn't exist yet.
fn llvm_type_for_ctx<'ctx>(ctx: &'ctx Context, ty: &Type) -> Option<BasicTypeEnum<'ctx>> {
    match ty {
        Type::Int => Some(ctx.i64_type().into()),
        Type::Float => Some(ctx.f64_type().into()),
        Type::Bool => Some(ctx.bool_type().into()),
        // N ≤ 34: hardware decimal128 path (i128 pair).
        // N > 34: bignum path — pointer to heap-allocated decimal string.
        Type::Number { precision } if *precision <= 34 => Some(ctx.i128_type().into()),
        Type::Nothing => None,
        _ => Some(ctx.ptr_type(AddressSpace::default()).into()),
    }
}

/// Lower a single monomorphized instantiation of a generic function.
#[allow(clippy::too_many_arguments)]
fn lower_generic_function<'ctx>(
    ctx: &'ctx Context,
    module: &'_ Module<'ctx>,
    rt: &'_ RuntimeDecls<'ctx>,
    globals: &'_ ModuleGlobals<'ctx>,
    typed: &'_ TypedModule,
    f: &FunctionDecl,
    shape_table: &'_ ShapeTable,
    shape_types: &'_ ShapeLlvmTypes<'ctx>,
    fn_val: FunctionValue<'ctx>,
    type_subst: HashMap<String, Type>,
    mono_sig: &ynz_typeck::generics::MonoSignature,
    mono_table: &'_ MonomorphizationTable,
    options_table: &'_ ynz_typeck::options_table::OptionsTable,
) -> Result<(), String> {
    let ret_ty = mono_sig.ret_type.clone();
    let ret_is_nothing = matches!(ret_ty, Type::Nothing);

    let mut cg = Cg {
        ctx,
        module,
        builder: ctx.create_builder(),
        rt,
        globals,
        typed,
        current_fn: fn_val,
        is_main: false,
        _current_fn_ret_ty: ret_ty,
        locals: HashMap::new(),
        shape_table,
        shape_types,
        type_subst,
        mono_table,
        options_table,
        // Generic functions are not errors-capable in M7 (no `-> T errors` on generics yet).
        is_errors_capable: false,
        errors_capable_locals: std::collections::HashSet::new(),
        bg_uid: 0,
        // Generic functions cannot contain `wait` in M2. Use empty caches.
        wait_cache: empty_wait_cache(),
        suspend_set: empty_suspend_set(),
        frame_layouts: empty_frame_layouts(),
        imported_fns: empty_imported_fns(),
        sm_frame_ptr: None,
        sm_yinz_ret_ty: None,
        sm_crossing_names: None,
        sm_crossing_scalar_set: HashSet::new(),
        sm_crossing_bool_set: HashSet::new(),
        sm_crossing_slot_indices: Vec::new(),
        sm_crossing_decimal128_set: HashSet::new(),
        sm_crossing_float_set: HashSet::new(),
        sm_crossing_errors_capable_set: HashSet::new(),
        sm_crossing_ec_number_set: HashSet::new(),
        sm_crossing_ec_number_i128_allocas: HashMap::new(),
        sm_crossing_shape_embed_set: HashSet::new(),
        sm_crossing_ec_struct_allocas: HashMap::new(),
        sm_crossing_shape_names: HashMap::new(),
        sm_crossing_shape_allocas: HashMap::new(),
        sm_scope_depth: 0,
        sm_for_loop_counter: 0,
        sm_number_errors_staging_offset: None,
        // Generic functions cannot contain `wait` — auto-parallel is never applicable.
        no_auto_parallel: false,
        // Generic functions never reach lower_sm_block; the empty table satisfies
        // the struct field without the caller needing a real SignatureTable.
        sig_table: empty_sig_table(),
        // Generic functions never enter the M3d spike path.
        m3d_spike: false,
        m3d_spike_cpu_result_names: Vec::new(),
        m3d_spike_cpu_result_allocas: HashMap::new(),
    };

    let entry = ctx.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);

    for (i, (param, concrete_ty)) in f.params.iter().zip(mono_sig.param_types.iter()).enumerate() {
        let llvm_param = fn_val
            .get_nth_param(i as u32)
            .ok_or_else(|| format!("missing param {} for generic `{}`", i, f.name))?;
        materialize_param(&mut cg, &param.name, llvm_param, concrete_ty)?;
    }

    for stmt in &f.body.stmts {
        if is_block_terminated(&cg) {
            break;
        }
        lower_stmt(&mut cg, stmt)?;
    }

    if !is_block_terminated(&cg) {
        if ret_is_nothing {
            cg.builder.build_return(None).map_err(|e| format!("{e}"))?;
        } else {
            cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

/// Compute the LLVM parameter types for a function declaration.
///
/// Primitive scalars pass by value. Everything else (string, number, shape, dynamic)
/// passes as an opaque pointer — the callee accesses the actual data via loads/GEPs.
fn llvm_param_types<'ctx>(
    ctx: &'ctx Context,
    f: &FunctionDecl,
    shape_table: &ShapeTable,
) -> Vec<BasicMetadataTypeEnum<'ctx>> {
    let ptr = ctx.ptr_type(AddressSpace::default());
    f.params
        .iter()
        .map(|p| {
            match &p.ty {
                ynz_ast::nodes::Type::Int => ctx.i64_type().into(),
                ynz_ast::nodes::Type::Float => ctx.f64_type().into(),
                ynz_ast::nodes::Type::Bool => ctx.bool_type().into(),
                // Named shape type or self — all shapes pass as ptr
                ynz_ast::nodes::Type::Named(n, _) if shape_table.contains(n) || n == "self" => {
                    ptr.into()
                }
                ynz_ast::nodes::Type::SelfType { .. } => ptr.into(),
                ynz_ast::nodes::Type::Dynamic { .. } => ptr.into(),
                // String, Number, and everything else: ptr
                _ => ptr.into(),
            }
        })
        .collect()
}

/// Forward-declare a function in the LLVM module (signature only, no body).
///
/// Also attaches LLVM `readonly` and `noalias` attributes to pointer parameters
/// based on the declared ownership modifier:
/// - `share` / inferred (None) → `readonly` + `noalias`
/// - `lend` → `noalias` only
/// - `give` → no attributes (callee owns the data, may mutate)
fn declare_function<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    f: &FunctionDecl,
    shape_table: &ShapeTable,
) -> Result<(), String> {
    let params = llvm_param_types(ctx, f, shape_table);
    let fn_ty = if f.name == "entrypoint" {
        ctx.i32_type().fn_type(&params, false)
    } else if f.errors_capable {
        // M7 P4a: errors-capable functions return `{i64 error_ptr, i64 success_val}`.
        // field 0 = error pointer (0 = success, non-zero = *YnzError)
        // field 1 = success value as i64 (valid only when field 0 = 0)
        let result_ty = errors_result_type(ctx);
        result_ty.fn_type(&params, false)
    } else {
        match &f.return_type {
            ynz_ast::nodes::Type::Nothing => ctx.void_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Int => ctx.i64_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Float => ctx.f64_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Bool => ctx.bool_type().fn_type(&params, false),
            _ => ctx
                .ptr_type(AddressSpace::default())
                .fn_type(&params, false),
        }
    };
    // `entrypoint` is the Yinz name; the C ABI entry point must be `main` for the linker.
    let llvm_name = if f.name == "entrypoint" {
        "main"
    } else {
        &f.name
    };
    let fn_val = module.add_function(llvm_name, fn_ty, None);

    // Emit LLVM ownership attributes on pointer-typed parameters.
    let readonly_kind = Attribute::get_named_enum_kind_id("readonly");
    let noalias_kind = Attribute::get_named_enum_kind_id("noalias");

    for (i, param) in f.params.iter().enumerate() {
        if !is_ptr_param(&param.ty, shape_table) {
            continue;
        }
        let ownership = param
            .ownership
            .as_ref()
            .unwrap_or(&OwnershipModifier::Share);
        match ownership {
            OwnershipModifier::Share => {
                fn_val.add_attribute(
                    AttributeLoc::Param(i as u32),
                    ctx.create_enum_attribute(readonly_kind, 0),
                );
                fn_val.add_attribute(
                    AttributeLoc::Param(i as u32),
                    ctx.create_enum_attribute(noalias_kind, 0),
                );
            }
            OwnershipModifier::Lend => {
                fn_val.add_attribute(
                    AttributeLoc::Param(i as u32),
                    ctx.create_enum_attribute(noalias_kind, 0),
                );
            }
            OwnershipModifier::Give => {}
        }
    }

    Ok(())
}

/// LLVM struct type for errors-capable return values: `{i64, i64}`.
///
/// LLVM struct type for the `errors`-keyword return encoding: `{i64, i64}`.
///
/// # ABI contract
///
/// - `field 0` (i64): error pointer — `0` means success; non-zero is a `*YnzError`
///   heap pointer cast to i64.
/// - `field 1` (i64): success value bits — valid ONLY when `field 0 == 0`.
///   - Scalar types (int, bool, float): stored directly as i64.
///   - Pointer-typed success values (string, shape, array, …): the heap pointer is
///     cast to i64.  Callers must cast `field 1` back to the appropriate pointer
///     type before dereferencing.
fn errors_result_type(ctx: &Context) -> inkwell::types::StructType<'_> {
    ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false)
}

/// True when the AST type will be passed as a pointer in LLVM (not a scalar value).
fn is_ptr_param(ty: &ynz_ast::nodes::Type, shape_table: &ShapeTable) -> bool {
    match ty {
        ynz_ast::nodes::Type::Int | ynz_ast::nodes::Type::Float | ynz_ast::nodes::Type::Bool => {
            false
        }
        ynz_ast::nodes::Type::Named(n, _)
            if !shape_table.contains(n) && n != "self" && n != "string" =>
        {
            false
        }
        _ => true,
    }
}

struct Cg<'ctx, 'g> {
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    builder: inkwell::builder::Builder<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    current_fn: FunctionValue<'ctx>,
    /// True when this function is `main` (affects return type and implicit ret).
    is_main: bool,
    /// Return type of the current function.
    _current_fn_ret_ty: Type,
    locals: HashMap<String, PointerValue<'ctx>>,
    // M4 additions:
    shape_table: &'g ShapeTable,
    shape_types: &'g ShapeLlvmTypes<'ctx>,
    // M5 P4a: type-param substitution for the current monomorphized instance.
    // Empty for non-generic functions.
    type_subst: HashMap<String, Type>,
    mono_table: &'g MonomorphizationTable,
    // M6: options table for variant tag lookup.
    options_table: &'g ynz_typeck::options_table::OptionsTable,
    // M7 P4a: true when the current function declared `-> T errors`.
    // Affects return-type wrapping and call-site auto-propagation.
    is_errors_capable: bool,
    // M7 P4a: set of local names that hold errors-capable results (pointer to {i64, i64}).
    // When one of these is first used in an errors-capable function, auto-propagation fires.
    errors_capable_locals: std::collections::HashSet<String>,
    // v0.3-M1: per-compilation counter for unique `background` closure LLVM function names.
    // Per-Cg (not global static) so identical source always produces identical IR even
    // when multiple compilations run in the same process (LSP, test harness).
    bg_uid: u64,
    // v0.3-M2 P6: local contains-wait cache (kept for generic lowering backward-compat).
    // Dead in P7 for non-generic code; remove in M3 when generic functions can suspend.
    #[allow(dead_code)]
    wait_cache: &'g WaitCache,
    // v0.3-M2 P7: transitive suspend set — the "is state machine" predicate.
    suspend_set: &'g SuspendSet,
    // v0.3-M2 P7: composed-frame layouts for all suspending functions.
    frame_layouts: &'g HashMap<String, FrameLayout>,
    // v0.3-M3b: imported function signatures — needed so helpers that look up
    // callee return types and errors-capable flags can find cross-module callees.
    imported_fns: &'g std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    // v0.3-M2 P7: when non-None, this Cg is inside a state-machine resume function.
    // The frame pointer and Yinz return type are needed so Stmt::Return stores the
    // typed value in frame[FRAME_OFFSET_RETURN_SLOT] and returns `i32 0` (Ready)
    // instead of the usual LLVM `ret <value>`.
    sm_frame_ptr: Option<PointerValue<'ctx>>,
    sm_yinz_ret_ty: Option<Type>,
    // v0.3-M3a P1: crossing-local metadata for frame-backed locals.
    // `sm_crossing_names` is the sorted list of local names that cross a suspension
    // boundary. `sm_crossing_scalar_set` contains the subset of crossing locals that
    // use a raw i64 alloca (int only — NOT bool, which uses i1 alloca + zext/trunc).
    // Non-SM Cg contexts use None/empty.
    sm_crossing_names: Option<Vec<String>>,
    sm_crossing_scalar_set: HashSet<String>,
    // Set of crossing local names whose type is bool. These use an i1 alloca (matching
    // the rest of codegen); flush zexts i1→i64 for the frame slot and reload truncates
    // the i64 frame slot back to i1 before storing. One frame slot (8 bytes) per bool.
    sm_crossing_bool_set: HashSet<String>,
    // Slot index for each crossing local (parallel to sm_crossing_names).
    // int/bool/float/ptr types use 1 slot; decimal128 and ErrorsCapable use 2 slots
    // (16 bytes stored directly in the frame — not a pointer to a stack buffer).
    sm_crossing_slot_indices: Vec<usize>,
    // Set of crossing local names whose type is decimal128 (number with precision ≤ 34).
    // These use 2 frame slots and i128 alloca (not ptr alloca).
    sm_crossing_decimal128_set: HashSet<String>,
    // Set of crossing local names whose type is float (f64).
    // These use a bitcast (f64 ↔ i64) rather than a raw integer load.
    sm_crossing_float_set: HashSet<String>,
    // Set of crossing local names whose type is ErrorsCapable {i64, i64}.
    // These use 2 frame slots (the two i64 fields stored directly); a companion
    // sm_entry struct alloca is pre-created and refreshed on every reload.
    sm_crossing_errors_capable_set: HashSet<String>,
    // Subset of sm_crossing_errors_capable_set for `-> number errors` (decimal128 EC) locals.
    // These use 3 frame slots: {f0, i128_lo, i128_hi}. The i128 bits are copied from the
    // callee's staging slot at bind time (guarded on f0==0) so the value does not alias
    // subsequent same-callee calls that reuse the same staging slot. A sm_entry i128 alloca
    // (keyed here) holds the reloaded decimal bits; f1 is repointed at it on every reload.
    sm_crossing_ec_number_set: HashSet<String>,
    // sm_entry i128 alloca for each EC<Number> crossing local. Holds the decimal bits
    // between suspend and resume. f1 of the companion EC struct is always set to point
    // at this alloca so `.or()` / `.failed()` can dereference it correctly after reload.
    sm_crossing_ec_number_i128_allocas: HashMap<String, PointerValue<'ctx>>,
    // Set of crossing local names whose type is a Shape (frame-embedded struct).
    // The struct bytes are stored directly in consecutive frame slots (no separate
    // heap allocation). The frame slot region is the persistent storage; the
    // sm_entry struct alloca is the working copy valid within one resume call.
    sm_crossing_shape_embed_set: HashSet<String>,
    // Companion alloca for each ErrorsCapable crossing local: a sm_entry {i64,i64}
    // struct alloca whose contents are refreshed from the frame slots on every reload.
    // Keyed by local name.
    sm_crossing_ec_struct_allocas: HashMap<String, PointerValue<'ctx>>,
    // Shape name for each shape-typed crossing local (used by frame-embed codegen to
    // look up the LLVM struct type for memcpy size computation).
    sm_crossing_shape_names: HashMap<String, String>,
    // sm_entry struct alloca for each shape-typed crossing local.
    // The alloca has the shape's LLVM struct type (not ptr). On each resume call:
    // - reload: memcpy frame slot region → this alloca
    // - flush:  memcpy this alloca → frame slot region
    // cg.locals[name] points to this alloca so field access GEPs work correctly.
    sm_crossing_shape_allocas: HashMap<String, PointerValue<'ctx>>,
    // Nesting depth inside if/while/for/match bodies in a SM resume function.
    // Used for the snapshot/restore protocol that prevents non-crossing locals introduced
    // inside a nested scope from leaking into cg.locals after the scope exits. Shadow
    // bindings (a `let x` where outer `x` crosses a wait) are rejected at typeck
    // (ShadowsCrossingLocal), so at codegen time depth > 0 never signals a shadow.
    sm_scope_depth: usize,
    // Counter tracking how many suspending for-loops have been emitted in this SM function.
    // Generates the matching synthetic crossing-local name `__ynz_for_idx_N` that
    // typeck pre-allocates a frame slot for. Must increment in the same order as
    // `collect_for_loop_synthetic_crossings_inner` in check.rs.
    sm_for_loop_counter: usize,
    // Byte offset of the 16-byte `number errors` staging slot within the composed frame,
    // when the current SM function returns `-> number errors`. None for all other functions.
    // Used by lower_stmt_return to write the i128 decimal to a frame-stable location so
    // the EC ok-pointer survives the resume function returning.
    sm_number_errors_staging_offset: Option<u64>,
    // v0.3-M3b Phase 4: when true, the auto-parallelize pass is disabled.
    // All suspending statements lower in pure source order via the existing
    // single inline-poll path. This is the TRUE dumb-sequential baseline used
    // as the cross-impl consistency oracle.
    //
    // When false (the default), `lower_sm_block` runs `partition_independent_groups`
    // and routes independent groups through `emit_independent_group_poll`.
    no_auto_parallel: bool,
    // v0.3-M3b Phase 4: signature table for write-effect classification in the
    // independence analysis. Used ONLY by `lower_sm_block` → `partition_independent_groups`.
    // When `no_auto_parallel` is true, this is never accessed.
    //
    // Stored here so the independence analysis can consume `param_ownerships` from
    // function signatures without re-deriving them (corpse b compliance).
    sig_table: &'g SignatureTable,
    // v0.3-M3d: when true, at least one function was promoted for CPU-statement
    // parallelization (the typeck `cpu_promotion_query` set is non-empty), so the
    // CPU-parallel join path may fire for any function `spike_cpu_candidates` admits.
    // False for every module that promotes nothing — zero behavior change there.
    m3d_spike: bool,
    // CPU-result (name, frame_offset) pairs owned by the spike reload mechanism.
    //
    // Each pair names a local bound by the CPU group's all_done_bb AND records the
    // SPIKE_RESULT_N_OFFSET where its value lives persistently across suspensions.
    //
    // Two invariants:
    //   1. reload_params_from_frame SKIPS these names in its crossing-local loop (they
    //      live at SPIKE_RESULT_N_OFFSET, not SM crossing slot indices) AND calls
    //      spike_reload_cpu_results_from_frame for the pairs at reload_crossing:true time
    //      — this fires at ANY sm_scope_depth, fixing the depth-asymmetry bug (#10).
    //   2. When rest_stmts processes a Stmt::Assign whose target matches a name here,
    //      the pair is REMOVED before the assign is lowered — the crossing machinery
    //      then owns that name (flush + reload from crossing slot), so the mutation is
    //      visible after the next suspension (#9 fix).
    //
    // Only consulted when m3d_spike is true (always empty for non-spike builds).
    m3d_spike_cpu_result_names: Vec<(String, u64)>,
    // sm_entry allocas for CPU-group result bindings.
    //
    // Pre-allocated in the function entry block (sm_entry) so they dominate all state
    // blocks. LLVM SSA requires allocas to be in the entry block when their values are
    // loaded in multiple basic blocks (e.g., after each suspension the reload path reads
    // from the same alloca). Creating allocas in a non-entry state block would produce
    // "Instruction does not dominate all uses" for any load that follows a different
    // control-flow path (e.g., a second suspension state block).
    //
    // emit_cpu_group_spawn_join stores results into these allocas (found via cg.locals).
    // spike_reload_cpu_results_from_frame reloads frame bytes into these same allocas
    // instead of creating fresh ones — no alloca needed at reload time.
    //
    // Always empty when m3d_spike is false.
    m3d_spike_cpu_result_allocas: HashMap<String, PointerValue<'ctx>>,
}

impl<'ctx, 'g> Cg<'ctx, 'g> {
    fn i64(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i64_type()
    }
    fn i128(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i128_type()
    }
    fn i32(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i32_type()
    }
    fn i8(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i8_type()
    }
    fn f64(&self) -> inkwell::types::FloatType<'ctx> {
        self.ctx.f64_type()
    }
    fn bool(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.bool_type()
    }
    fn ptr(&self) -> inkwell::types::PointerType<'ctx> {
        self.ctx.ptr_type(AddressSpace::default())
    }

    /// Apply the current type-param substitution to `ty`, returning a concrete type.
    fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::TypeParam { name } => self
                .type_subst
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Type::Generic { name, args } => Type::Generic {
                name: name.clone(),
                args: args.iter().map(|a| self.resolve_type(a)).collect(),
            },
            Type::BuiltinArray { elem } => Type::BuiltinArray {
                elem: Box::new(self.resolve_type(elem)),
            },
            Type::BuiltinFixed { elem, size } => Type::BuiltinFixed {
                elem: Box::new(self.resolve_type(elem)),
                size: *size,
            },
            Type::Maybe { inner } => Type::Maybe {
                inner: Box::new(self.resolve_type(inner)),
            },
            other => other.clone(),
        }
    }

    /// Look up the typeck type for `expr` and apply the current type-param substitution.
    fn expr_type(&self, expr: &Expr) -> Type {
        let key = (expr.span().start, expr.span().end);
        let raw = self
            .typed
            .expr_types
            .get(&key)
            .cloned()
            .unwrap_or(Type::Error);
        self.resolve_type(&raw)
    }

    fn llvm_type_for(&self, ty: &Type) -> Option<BasicTypeEnum<'ctx>> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            Type::Int => Some(self.i64().into()),
            Type::Float => Some(self.f64().into()),
            Type::Bool => Some(self.bool().into()),
            // N ≤ 34: hardware decimal128 (i128). N > 34: bignum (ptr to decimal string).
            Type::Number { precision } if *precision <= 34 => Some(self.i128().into()),
            Type::Number { .. } => Some(self.ptr().into()),
            Type::String => Some(self.ptr().into()),
            // Shape and dynamic values are always passed/stored as opaque pointers.
            Type::Shape { .. } => Some(self.ptr().into()),
            Type::Dynamic { .. } => Some(self.ptr().into()),
            // Collections and maybe are all represented as opaque pointers (heap or alloca).
            Type::BuiltinArray { .. } => Some(self.ptr().into()),
            Type::BuiltinFixed { .. } => Some(self.ptr().into()),
            Type::Maybe { .. } => Some(self.ptr().into()),
            Type::MapEntry { .. } => Some(self.ptr().into()),
            Type::BuiltinMap { .. } => Some(self.ptr().into()),
            // M6: options values are i8 tags; union values are opaque pointers (heap tagged-struct).
            Type::Options { .. } => Some(self.ctx.i8_type().into()),
            Type::Union { .. } => Some(self.ptr().into()),
            // M7 P4c: Range values are {i64 start, i64 end} stored via a stack-alloca pointer.
            Type::Range { .. } => Some(self.ptr().into()),
            // M8 P4: sensitive values have the same ABI as their inner type (string ptr).
            Type::Sensitive { .. } => Some(self.ptr().into()),
            _ => None,
        }
    }

    /// LLVM struct type for `maybe<T>`: `{i64, i64}` where slot 0 = has_value, slot 1 = bits.
    fn maybe_type(&self) -> inkwell::types::StructType<'ctx> {
        self.ctx
            .struct_type(&[self.i64().into(), self.i64().into()], false)
    }

    fn alloca(&self, ty: &Type, name: &str) -> Result<PointerValue<'ctx>, String> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            // Collections and maybe: the variable slot holds an opaque pointer to the
            // actual data structure (heap YnzArray, stack [N x i64], or stack {i64,i64}).
            // Loading the slot returns the pointer; the caller works through that pointer.
            Type::BuiltinArray { .. }
            | Type::BuiltinFixed { .. }
            | Type::Maybe { .. }
            | Type::BuiltinMap { .. }
            | Type::MapEntry { .. }
            | Type::Union { .. } => self
                .builder
                .build_alloca(self.ptr(), name)
                .map_err(|e| format!("{e}")),
            // M7 P4a: ErrorsCapable stores a pointer to the {i64, i64} result struct alloca.
            Type::ErrorsCapable { .. } => self
                .builder
                .build_alloca(self.ptr(), name)
                .map_err(|e| format!("{e}")),
            // M7 P4c: Range stores a pointer to the {i64 start, i64 end} alloca.
            Type::Range { .. } => self
                .builder
                .build_alloca(self.ptr(), name)
                .map_err(|e| format!("{e}")),
            _ => {
                let llvm_ty = self
                    .llvm_type_for(&resolved)
                    .ok_or_else(|| format!("cannot alloca for type {:?}", resolved))?;
                self.builder
                    .build_alloca(llvm_ty, name)
                    .map_err(|e| format!("{e}"))
            }
        }
    }

    /// Build an alloca in the function's ENTRY block, regardless of where the builder
    /// currently points. LLVM SSA requires allocas to dominate every use; placing them
    /// in the entry block is the canonical way to satisfy this for values that may be
    /// used across multiple successor blocks (e.g., inside if/while bodies that are
    /// separate basic blocks). This matches what `materialize_param` does for params.
    ///
    /// Yinz allows variable shadowing (spec/linting.md `shadowed-variables` lint).
    /// When a shadow `let x` appears inside a nested scope that is a separate LLVM basic
    /// block, its alloca must be in the entry block so it dominates its uses inside that
    /// block. The outer binding is restored to `cg.locals` on scope exit (restore-all
    /// protocol), so the shadow has no effect on the outer name after the scope closes.
    fn alloca_in_entry(&self, ty: &Type, name: &str) -> Result<PointerValue<'ctx>, String> {
        let entry_bb = self
            .current_fn
            .get_first_basic_block()
            .ok_or_else(|| format!("alloca_in_entry: no entry block for `{name}`"))?;
        // Position at the end of the entry block (before its terminator, if any).
        // We save and restore the builder's current insertion point so the caller's
        // ongoing block emission is unaffected.
        let saved_bb = self.builder.get_insert_block();
        if let Some(term) = entry_bb.get_terminator() {
            self.builder.position_before(&term);
        } else {
            self.builder.position_at_end(entry_bb);
        }
        let slot = self.alloca(ty, name)?;
        // Restore the builder to wherever it was before we moved it.
        if let Some(bb) = saved_bb {
            self.builder.position_at_end(bb);
        }
        Ok(slot)
    }

    fn append_block(&self, name: &str) -> BasicBlock<'ctx> {
        self.ctx.append_basic_block(self.current_fn, name)
    }

    /// Build an alloca of a raw LLVM basic type in the function's ENTRY block.
    /// Used by SM for-loop codegen for internal index/pointer slots that have no
    /// corresponding Yinz type (e.g., collection pointers, entry-struct slots).
    fn alloca_in_entry_llvm(
        &self,
        llvm_ty: impl inkwell::types::BasicType<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, String> {
        let entry_bb = self
            .current_fn
            .get_first_basic_block()
            .ok_or_else(|| format!("alloca_in_entry_llvm: no entry block for `{name}`"))?;
        let saved_bb = self.builder.get_insert_block();
        if let Some(term) = entry_bb.get_terminator() {
            self.builder.position_before(&term);
        } else {
            self.builder.position_at_end(entry_bb);
        }
        let slot = self
            .builder
            .build_alloca(llvm_ty, name)
            .map_err(|e| format!("{e}"))?;
        if let Some(bb) = saved_bb {
            self.builder.position_at_end(bb);
        }
        Ok(slot)
    }

    /// Build an alloca holding a `maybe<T>` with `has_value = 0`.
    fn build_maybe_none(&self) -> Result<PointerValue<'ctx>, String> {
        let slot = self
            .builder
            .build_alloca(self.maybe_type(), "maybe_none")
            .map_err(|e| format!("{e}"))?;
        self.builder
            .build_store(slot, self.maybe_type().const_zero())
            .map_err(|e| format!("{e}"))?;
        Ok(slot)
    }

    /// Build an alloca holding a `maybe<T>` with `has_value = 1` and `bits = value_i64`.
    #[allow(dead_code)]
    fn build_maybe_some(
        &self,
        value_i64: inkwell::values::IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
        let slot = self
            .builder
            .build_alloca(self.maybe_type(), "maybe_some")
            .map_err(|e| format!("{e}"))?;
        let one = self.i64().const_int(1, false);
        let has_gep = self
            .builder
            .build_struct_gep(self.maybe_type(), slot, 0, "has_gep")
            .map_err(|e| format!("{e}"))?;
        self.builder
            .build_store(has_gep, one)
            .map_err(|e| format!("{e}"))?;
        let val_gep = self
            .builder
            .build_struct_gep(self.maybe_type(), slot, 1, "val_gep")
            .map_err(|e| format!("{e}"))?;
        self.builder
            .build_store(val_gep, value_i64)
            .map_err(|e| format!("{e}"))?;
        Ok(slot)
    }

    /// Convert any BasicValueEnum to its i64-bit representation for uniform array/map/frame
    /// storage. Delegates to the shared `value_to_i64_bits` marshaller after resolving generics.
    fn to_i64_bits(
        &self,
        val: BasicValueEnum<'ctx>,
        ty: &Type,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        let resolved = self.resolve_type(ty);
        value_to_i64_bits(&self.builder, self.i64(), val, &resolved)
    }

    /// Convert an i64-bit pattern back to the concrete type representation.
    fn i64_bits_to(
        &self,
        val: inkwell::values::IntValue<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            Type::Int => Ok(val.into()),
            Type::Bool => self
                .builder
                .build_int_truncate(val, self.bool(), "i_to_b")
                .map(|v| v.into())
                .map_err(|e| format!("{e}")),
            Type::Float => self
                .builder
                .build_bit_cast(val, self.f64(), "i_to_f")
                .map_err(|e| format!("{e}")),
            Type::String
            | Type::Number { .. }
            | Type::Shape { .. }
            | Type::Dynamic { .. }
            | Type::BuiltinArray { .. }
            | Type::BuiltinFixed { .. }
            | Type::Maybe { .. }
            | Type::BuiltinMap { .. }
            | Type::MapEntry { .. }
            | Type::Union { .. }
            | Type::Sensitive { .. } => self
                .builder
                .build_int_to_ptr(val, self.ptr(), "i_to_ptr")
                .map(|v| v.into())
                .map_err(|e| format!("{e}")),
            _ => Err(format!("cannot convert i64 bits to {:?}", resolved)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_function<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    shape_types: &'g ShapeLlvmTypes<'ctx>,
    shape_abi_sizes: &'g HashMap<String, u64>,
    mono_table: &'g MonomorphizationTable,
    options_table: &'g ynz_typeck::options_table::OptionsTable,
    wait_cache: &'g WaitCache,
    suspend_set: &'g SuspendSet,
    frame_layouts: &'g HashMap<String, FrameLayout>,
    imported_fns: &'g std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    sig_table: &'g SignatureTable,
    no_auto_parallel: bool,
    m3d_spike: bool,
) -> Result<(), String> {
    // v0.3-M2 P7 path selection: functions in the transitive suspend_set get the
    // state-machine path. Uses suspend_set (from typeck FunctionSig.suspends) instead
    // of the local wait_cache, so fns that reach `sleep` transitively — without an
    // explicit `wait` — are now correctly compiled as state machines.
    if suspend_set.contains(&f.name) {
        return lower_function_with_waits(
            ctx,
            module,
            rt,
            globals,
            typed,
            f,
            shape_table,
            shape_types,
            shape_abi_sizes,
            mono_table,
            options_table,
            wait_cache,
            suspend_set,
            frame_layouts,
            imported_fns,
            sig_table,
            no_auto_parallel,
            m3d_spike,
        );
    }

    let llvm_name = if f.name == "entrypoint" {
        "main"
    } else {
        f.name.as_str()
    };
    let fn_val = module
        .get_function(llvm_name)
        .ok_or_else(|| format!("function `{}` was not forward-declared", f.name))?;

    let ret_ty = ast_type_to_typeck_type(&f.return_type, shape_table);
    let is_main = f.name == "entrypoint";
    let ret_is_nothing = matches!(ret_ty, Type::Nothing);
    let is_errors_capable = f.errors_capable;

    let mut cg = Cg {
        ctx,
        module,
        builder: ctx.create_builder(),
        rt,
        globals,
        typed,
        current_fn: fn_val,
        is_main,
        _current_fn_ret_ty: ret_ty,
        locals: HashMap::new(),
        shape_table,
        shape_types,
        type_subst: HashMap::new(),
        mono_table,
        options_table,
        is_errors_capable,
        errors_capable_locals: std::collections::HashSet::new(),
        bg_uid: 0,
        wait_cache,
        suspend_set,
        frame_layouts,
        imported_fns,
        sm_frame_ptr: None,
        sm_yinz_ret_ty: None,
        sm_crossing_names: None,
        sm_crossing_scalar_set: HashSet::new(),
        sm_crossing_bool_set: HashSet::new(),
        sm_crossing_slot_indices: Vec::new(),
        sm_crossing_decimal128_set: HashSet::new(),
        sm_crossing_float_set: HashSet::new(),
        sm_crossing_errors_capable_set: HashSet::new(),
        sm_crossing_ec_number_set: HashSet::new(),
        sm_crossing_ec_number_i128_allocas: HashMap::new(),
        sm_crossing_shape_embed_set: HashSet::new(),
        sm_crossing_ec_struct_allocas: HashMap::new(),
        sm_crossing_shape_names: HashMap::new(),
        sm_crossing_shape_allocas: HashMap::new(),
        sm_scope_depth: 0,
        sm_for_loop_counter: 0,
        sm_number_errors_staging_offset: None,
        // Non-SM functions cannot contain independent suspending groups.
        no_auto_parallel: false,
        // sig_table unused for non-SM functions — independence analysis runs only in
        // lower_sm_block which is never reached from the non-SM path.
        sig_table,
        // Non-SM functions are never in the spike suspend_set extension, so m3d_spike
        // only matters for SM functions. Forward the flag so future callers are consistent.
        m3d_spike,
        m3d_spike_cpu_result_names: Vec::new(),
        m3d_spike_cpu_result_allocas: HashMap::new(),
    };

    let entry = ctx.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);

    // Initialize SipHash key from OS entropy before any map operations.
    // Also initialise the Tokio runtime (v0.3-M1: spawns the blocking thread pool).
    if is_main {
        cg.builder
            .build_call(cg.rt.ynz_siphash_init, &[], "siphash_init")
            .map_err(|e| format!("siphash_init: {e}"))?;
        cg.builder
            .build_call(cg.rt.ynz_rt_init, &[], "rt_init")
            .map_err(|e| format!("rt_init: {e}"))?;
    }

    // M7 P4a: errors-capable functions push a frame on entry for the call trace.
    if is_errors_capable {
        let file_g = build_string_global(ctx, module, f.name.as_str(), ".ec.file");
        let fn_g = build_string_global(ctx, module, f.name.as_str(), ".ec.fn");
        cg.builder
            .build_call(
                cg.rt.ynz_frame_push,
                &[
                    file_g.as_pointer_value().into(),
                    cg.i64().const_int(0, false).into(),
                    fn_g.as_pointer_value().into(),
                ],
                "ec_push",
            )
            .map_err(|e| format!("frame_push: {e}"))?;
    }

    // Materialize each parameter as an alloca so the "every name = alloca" invariant holds.
    for (i, param) in f.params.iter().enumerate() {
        let llvm_param = fn_val
            .get_nth_param(i as u32)
            .ok_or_else(|| format!("missing LLVM param {} for `{}`", i, f.name))?;
        let param_ty = ast_type_to_typeck_type(&param.ty, shape_table);
        materialize_param(&mut cg, &param.name, llvm_param, &param_ty)?;
    }

    for stmt in &f.body.stmts {
        if is_block_terminated(&cg) {
            break;
        }
        lower_stmt(&mut cg, stmt)?;
    }

    // Implicit terminator if the current block has no terminator yet.
    //
    // - entrypoint: always ret i32 0 (C ABI entry point).
    // - nothing-returning functions: ret void (legitimate fall-off-the-end).
    // - errors-capable: implicit success return of zero value (typeck ensures all
    //   paths have explicit returns, so this block should be dead code; emit a
    //   safe zeroed success result instead of unreachable to help debugging).
    // - non-nothing functions: unreachable — typeck confirmed exhaustive paths.
    if !is_block_terminated(&cg) {
        if is_main {
            // Drain in-flight background tasks before exiting.
            cg.builder
                .build_call(cg.rt.ynz_rt_shutdown, &[], "rt_shutdown_implicit")
                .map_err(|e| format!("rt_shutdown: {e}"))?;
            cg.builder
                .build_return(Some(&ctx.i32_type().const_int(0, false)))
                .map_err(|e| format!("implicit main ret: {e}"))?;
        } else if is_errors_capable {
            // Implicit success with zero-valued success field.
            cg.builder
                .build_call(cg.rt.ynz_frame_pop, &[], "ec_pop_implicit")
                .map_err(|e| format!("frame_pop: {e}"))?;
            let zero_result = errors_result_type(ctx).const_zero();
            cg.builder
                .build_return(Some(&zero_result))
                .map_err(|e| format!("implicit ec ret: {e}"))?;
        } else if ret_is_nothing {
            cg.builder
                .build_return(None)
                .map_err(|e| format!("implicit void ret: {e}"))?;
        } else {
            cg.builder
                .build_unreachable()
                .map_err(|e| format!("implicit unreachable: {e}"))?;
        }
    }
    Ok(())
}

// ── v0.3-M2: state-machine function codegen ───────────────────────────────────

/// Lower a `wait`-containing function as an LLVM state machine.
///
/// # Generated components
///
/// 1. **Resume function** (`ynz_sm_<name>_resume`) — the actual state-machine logic,
///    split at each `Expr::Wait`. Pre-declared in Pass 0.6; body emitted here.
///    Signature: `i32 (ptr frame, ptr waker_ctx)` where 0=Ready, 1=Pending.
///
/// 2. **Wrapper function** (`<name>` LLVM function, or `main` for `entrypoint`) —
///    allocates the frame, copies args to frame slots, calls
///    `ynz_rt_run_entrypoint`, reads the typed return value from frame[16], frees
///    the frame, and returns. This is the function called from non-SM contexts and
///    by `main` entry.
///
/// # Frame allocation
///
/// Frame is heap-allocated via `ynz_alloc(frame_size)`. For the `background` path,
/// the frame is allocated at the call site (in `lower_expr_background`) and passed
/// directly to `ynz_rt_spawn` — the wrapper function is not invoked.
///
/// # Wait dispatch strategy
///
/// Each `Expr::Wait(Call { callee: "sleep", args: [ms] }, _)` in the body
/// generates:
/// - **State N (before wait)**: create the sleep handle, store in `frame[SLEEP_HANDLE]`,
///   set `resume_point = N+1`, return Pending.
/// - **State N+1 (continuation)**: poll the sleep handle via `ynz_rt_async_sleep_poll`.
///   If Ready: clear handle slot, continue. If Pending: return 1.
///
/// # `main` wrap
///
/// When `entrypoint` contains `wait`, the LLVM `main` function:
/// 1. Calls `ynz_rt_init` (first non-allocation instruction — see AC #5).
/// 2. Allocates the frame.
/// 3. Calls `ynz_rt_run_entrypoint(resume_fn, frame, size)`.
/// 4. Reads exit code from `frame[0]`.
/// 5. Calls `ynz_rt_shutdown`.
/// 6. Returns exit code.
///
/// # Failure modes
///
/// Propagates `Err` from any inkwell builder call. State-machine resume panics are
/// caught by Tokio's task wrapper and do not propagate to the calling scope.
#[allow(clippy::too_many_arguments)]
fn lower_function_with_waits<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    f: &'g FunctionDecl,
    shape_table: &'g ShapeTable,
    shape_types: &'g ShapeLlvmTypes<'ctx>,
    shape_abi_sizes: &'g HashMap<String, u64>,
    mono_table: &'g MonomorphizationTable,
    options_table: &'g ynz_typeck::options_table::OptionsTable,
    wait_cache: &'g WaitCache,
    suspend_set: &'g SuspendSet,
    frame_layouts: &'g HashMap<String, FrameLayout>,
    imported_fns: &'g std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    sig_table: &'g SignatureTable,
    no_auto_parallel: bool,
    m3d_spike: bool,
) -> Result<(), String> {
    // Collect the names of parameters. ALL parameters are live across any wait.
    let param_names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();

    // Cache the spike-candidacy probe once. spike_cpu_candidates walks the function body
    // to identify an adjacent pure-CPU pair; calling it three times per function would
    // triple that scan cost. The result is stable (pure read of f + typed + suspend_set).
    let spike_candidates = if m3d_spike {
        spike_cpu_candidates(f, typed, suspend_set)
    } else {
        None
    };

    // Compute the set of locals that cross a suspension boundary (declared before
    // a wait, read after it). These must live in the heap frame instead of SSA
    // registers so their values survive across resume calls.
    let param_name_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();
    let suspending_refs: HashSet<&str> = suspend_set.iter().map(|s| s.as_str()).collect();
    let crossing_names: Vec<String> = crossing_local_names(
        &f.body.stmts,
        &param_name_refs,
        &suspending_refs,
        &typed.expr_types,
    );

    // Slot index layout: params occupy slots [0..n_params), crossing locals occupy
    // slots starting at n_params (or n_params+SPIKE_SLOT_RESERVE when spike is active —
    // see below). decimal128 + EC use 2 consecutive slots; shapes use ceil(N/8) consecutive
    // slots (frame-embedded); all others use 1.
    // This matches the slot counting in build_frame_layouts.
    let n_params = param_names.len();

    // Spike functions reserve 48 bytes (6 × 8-byte slots) immediately after the frame
    // header for CPU handle and result slots.
    //
    // Spike frame byte layout (zero-param hosts only — spike_cpu_candidates declines
    // any host with ≥1 params to avoid the collision below):
    //   bytes  0..31 : frame header (resume_point, discriminator, sleep_handle, return_slot)
    //   bytes 32..39 : SPIKE_HANDLE_0 — *mut CpuJoinHandle (runtime contract, must stay at 32)
    //   bytes 40..47 : SPIKE_HANDLE_1 — *mut CpuJoinHandle (runtime contract, must stay at 40)
    //   bytes 48..63 : SPIKE_RESULT_0 — YnzCpuResult = [i64;2] (16 bytes)
    //   bytes 64..79 : SPIKE_RESULT_1 — YnzCpuResult = [i64;2] (16 bytes)
    //   bytes 80+    : crossing locals (no params for spike hosts, so no param slots here)
    //
    // Non-spike frame:
    //   bytes  0..31 : frame header
    //   bytes 32+    : params (0..n_params slots) then crossing locals
    //
    // Zero-param invariant: admitting a param'd host would place param slot 0 at byte 32
    // (= SPIKE_HANDLE_0_OFFSET), colliding with the handle pointer. spike_cpu_candidates
    // guards this: returns None for any host with params. This is why `n_params == 0` here
    // when spike_active_here is true.
    let spike_active_here = spike_candidates.is_some();
    // Number of 8-byte slots reserved for the spike handle+result region after the frame header.
    //
    // Derivation (each slot = 8 bytes; frame header = 32 bytes = 4 slots):
    //   slot 0 (byte 32): SPIKE_HANDLE_0  — *mut CpuJoinHandle (8 bytes)
    //   slot 1 (byte 40): SPIKE_HANDLE_1  — *mut CpuJoinHandle (8 bytes)
    //   slot 2 (byte 48): SPIKE_RESULT_0 lo — first i64 of YnzCpuResult (8 bytes)
    //   slot 3 (byte 56): SPIKE_RESULT_0 hi — second i64 of YnzCpuResult (8 bytes)
    //   slot 4 (byte 64): SPIKE_RESULT_1 lo — first i64 of YnzCpuResult (8 bytes)
    //   slot 5 (byte 72): SPIKE_RESULT_1 hi — second i64 of YnzCpuResult (8 bytes)
    //   ─────────────────────────────────────────────────────────────────
    //   Total: 2 handles × 8B + 2 results × 16B = 16 + 32 = 48 bytes = 6 slots
    //
    // Crossing locals for spike functions start past the CPU-slot reserve (byte 80+ for the
    // Phase-0 two-member group), so they never overlap the handle/result region. Non-spike
    // functions use n_params+0 (no reserve).
    //
    // The reserve is read from the composed frame layout's `cpu_group_slots` (the SSOT
    // computed in `build_frame_layouts`) rather than a hardcoded constant, so the size math
    // here and the offsets the join emission uses cannot drift. Falls back to the 6-slot
    // (48-byte) Phase-0 reserve only if the layout entry is missing, which cannot happen for
    // a spike-active (in-suspend-set) function.
    let spike_slot_reserve: usize = if spike_active_here {
        frame_layouts
            .get(&f.name)
            .map(cpu_slot_reserve_slots)
            .filter(|&s| s > 0)
            .unwrap_or(6)
    } else {
        0
    };
    let crossing_slot_base = n_params + spike_slot_reserve;

    // Compute per-crossing-local slot indices using typeck types (catches inferred number).
    let crossing_slot_indices: Vec<usize> = {
        let mut indices = Vec::with_capacity(crossing_names.len());
        let mut cursor = crossing_slot_base;
        for cname in &crossing_names {
            indices.push(cursor);
            let ty = find_let_typeck_type_in_stmts(&f.body.stmts, cname.as_str(), typed);
            let slots = match ty {
                Some(Type::Number { precision }) if precision <= 34 => 2,
                // EC<Number> (-> number errors): 3 frame slots {f0, i128_lo, i128_hi}.
                Some(Type::ErrorsCapable { ref inner }) if matches!(inner.as_ref(), Type::Number { precision } if *precision <= 34) => {
                    3
                }
                // All other ErrorsCapable {i64,i64}: 2 frame slots for the two fields.
                Some(Type::ErrorsCapable { .. }) => 2,
                Some(Type::Shape { name: ref sname }) => shape_frame_slots(sname, shape_abi_sizes),
                _ => 1,
            };
            cursor += slots;
        }
        indices
    };
    // n_locals counts the slots the frame must accommodate for params + crossing locals.
    // For spike functions, the CPU-slot reserve is included so the fallback frame-size
    // formula (FRAME_HEADER_SIZE + own_locals_size(n_locals)) produces the right total
    // without double-counting spike_extra_frame_bytes.
    let n_locals = if spike_active_here {
        n_params
            + spike_slot_reserve
            + crossing_local_total_slots(f, &crossing_names, typed, shape_abi_sizes)
    } else {
        n_params + crossing_local_total_slots(f, &crossing_names, typed, shape_abi_sizes)
    };

    // Look up the composed frame layout for this function. The total_size covers
    // header(32) + own_locals + optional 16-byte number-errors staging slot + embedded child
    // sub-frames = ONE allocation per task tree.
    let frame_layout = frame_layouts.get(&f.name);
    // Spike frame layout (byte offsets from frame base):
    //   +0..31  : standard SM frame header (resume_point, padding/discriminator, sleep_handle, return_slot)
    //   +32..39 : spike handle slot 0 (*mut CpuJoinHandle, 8 bytes) — runtime contract, MUST stay at 32
    //   +40..47 : spike handle slot 1 (*mut CpuJoinHandle, 8 bytes) — runtime contract, MUST stay at 40
    //   +48..63 : spike result slot 0 (YnzCpuResult = [i64;2], 16 bytes)
    //   +64..79 : spike result slot 1 (YnzCpuResult = [i64;2], 16 bytes)
    //   +80..   : params (n_params × 8 bytes) then crossing locals
    //
    // For spike functions, `build_frame_layouts` (invoked before spike detection) does not
    // know about the 48-byte handle/result region — it computes total_size = 32 + n_params*8 +
    // crossing_bytes. Using that stale size as frame_bytes_base would under-allocate by 48 bytes
    // and overlap the spike region with param/crossing slots. We therefore always use the
    // fallback formula for spike functions (FRAME_HEADER_SIZE + own_locals_size(n_locals)), where
    // n_locals now includes SPIKE_SLOT_RESERVE (see above), giving the correct 80 + param*8 +
    // crossing_bytes total. Non-spike functions continue to use the frame_layout path unchanged.
    let frame_bytes: u64 = if spike_active_here {
        state_machine::FRAME_HEADER_SIZE + state_machine::own_locals_size(n_locals)
    } else {
        frame_layout.map(|l| l.total_size).unwrap_or_else(|| {
            state_machine::FRAME_HEADER_SIZE + state_machine::own_locals_size(n_locals)
        })
    };
    let number_errors_staging_offset = frame_layout.and_then(|l| l.number_errors_staging_offset);

    let is_main = f.name == "entrypoint";
    let llvm_name = if is_main { "main" } else { f.name.as_str() };

    // ── Part 1: Generate the resume function body ──────────────────────────────

    let resume_name = state_machine::resume_fn_name(&f.name);
    let resume_fn = module
        .get_function(&resume_name)
        .ok_or_else(|| format!("resume fn `{resume_name}` not pre-declared"))?;

    // Resume fn params: (frame: ptr, waker_ctx: ptr)
    let frame_param = resume_fn
        .get_nth_param(0)
        .ok_or("resume: missing frame param")?
        .into_pointer_value();
    let waker_param = resume_fn
        .get_nth_param(1)
        .ok_or("resume: missing waker param")?
        .into_pointer_value();

    // Build the entry block: switch on resume_point.
    let resume_entry = ctx.append_basic_block(resume_fn, "sm_entry");

    // Pre-create state blocks.
    // Count ALL suspension points: explicit `wait` nodes + calls to suspending callees.
    // Each suspension point needs a poll-loop continuation state.
    let n_waits_base = count_suspension_points(&f.body, suspend_set);
    // Spike: a CPU-parallel group occupies two states — the spawn state (state 0, which is
    // the initial dispatch state, always present) and the poll state (state 1, the extra one).
    // After emit_cpu_group_spawn_join, *current_state is advanced by 2. Any subsequent
    // wait-bearing statement uses *current_state as its "current" slot and
    // *current_state + 1 as its "continuation" slot, so we need 2 extra state blocks
    // beyond what the base wait count provides. Without the +2 here, a mixed body
    // (CPU group + at least one `wait`) would request a continuation index that exceeds
    // the pre-allocated state_blocks vector.
    let spike_extra_states = if spike_candidates.is_some() {
        2usize
    } else {
        0
    };
    let n_waits = n_waits_base + spike_extra_states;
    // States: 0 = before first wait, 1..n_waits = each continuation, plus a dead/error block.
    let state_blocks: Vec<inkwell::basic_block::BasicBlock<'ctx>> = (0..=n_waits)
        .map(|i| ctx.append_basic_block(resume_fn, &format!("sm_s{i}")))
        .collect();
    let dead_block = ctx.append_basic_block(resume_fn, "sm_dead");

    // Pending return block: returns 1 (Pending) to the outer driver.
    let pending_block = ctx.append_basic_block(resume_fn, "sm_pending");

    // Build a Cg context for the resume function body. Allocas must be placed in the
    // function entry block (sm_entry) so they dominate all successor blocks per LLVM SSA rules.
    // Compute the Yinz return type so lower_stmt_return can store typed values
    // in the return slot instead of emitting a bare LLVM ret instruction.
    //
    // For errors-capable functions: ast_type_to_typeck_type strips the ErrorCapable
    // wrapper and returns the inner type (e.g., Int for `-> int errors`). We need
    // the full ErrorsCapable type so lower_stmt_return dispatches to the correct arm.
    let fn_is_errors_capable = f.errors_capable;
    let yinz_ret_ty = if fn_is_errors_capable {
        let inner = ast_type_to_typeck_type(&f.return_type, shape_table);
        Type::ErrorsCapable {
            inner: Box::new(inner),
        }
    } else {
        ast_type_to_typeck_type(&f.return_type, shape_table)
    };

    // frame_ptr_for_resume is not available until Part 2 — we'll thread it through
    // after allocation. For Part 1 (resume body), we need to pass it via cg_resume.sm_frame_ptr.
    // Since frame_param IS the frame pointer for the resume fn, we set it here.
    // (frame_param was already bound above from resume_fn.get_nth_param(0).)

    let mut cg_resume = Cg {
        ctx,
        module,
        builder: ctx.create_builder(),
        rt,
        globals,
        typed,
        current_fn: resume_fn,
        is_main: false,
        _current_fn_ret_ty: Type::Nothing, // resume fn returns i32, not the Yinz type
        locals: HashMap::new(),
        shape_table,
        shape_types,
        type_subst: HashMap::new(),
        mono_table,
        options_table,
        // is_errors_capable = false inside the resume fn: the SM return path handles
        // errors-capable results explicitly (stores {i64,i64} in return slot). Keeping
        // is_errors_capable=true would trigger auto-propagation via `ret {i64,i64}` which
        // conflicts with the resume fn's `i32` return type.
        is_errors_capable: false,
        errors_capable_locals: std::collections::HashSet::new(),
        bg_uid: 0,
        wait_cache,
        suspend_set,
        frame_layouts,
        imported_fns,
        sm_frame_ptr: Some(frame_param),
        sm_yinz_ret_ty: Some(yinz_ret_ty.clone()),
        sm_crossing_names: Some(crossing_names.clone()),
        sm_crossing_scalar_set: HashSet::new(), // populated during alloca creation below
        sm_crossing_bool_set: HashSet::new(),   // populated during alloca creation below
        sm_crossing_slot_indices: crossing_slot_indices.clone(),
        sm_crossing_decimal128_set: HashSet::new(), // populated during alloca creation below
        sm_crossing_float_set: HashSet::new(),      // populated during alloca creation below
        sm_crossing_errors_capable_set: HashSet::new(), // populated during alloca creation below
        sm_crossing_shape_embed_set: HashSet::new(), // populated during alloca creation below
        sm_crossing_ec_struct_allocas: HashMap::new(), // populated during alloca creation below
        sm_crossing_shape_names: HashMap::new(),    // populated during alloca creation below
        sm_crossing_shape_allocas: HashMap::new(),  // populated during alloca creation below
        sm_crossing_ec_number_set: HashSet::new(),  // populated during alloca creation below
        sm_crossing_ec_number_i128_allocas: HashMap::new(), // populated during alloca creation below
        sm_scope_depth: 0,
        sm_for_loop_counter: 0,
        sm_number_errors_staging_offset: number_errors_staging_offset,
        // When set, lower_sm_block skips independence analysis and lowers all stmts
        // sequentially — the TRUE dumb-sequential baseline (not a shared-analysis no-op).
        no_auto_parallel,
        // sig_table forwarded so independence analysis can read param_ownerships
        // without re-deriving write effects from scratch (corpse b compliance).
        sig_table,
        // Gate gate-1 / gate-2 coherence: lower_sm_block's spike path (gate 2) only fires
        // when cg.m3d_spike is true. If spike_candidates is None — because this host was
        // declined (e.g. has params) — set m3d_spike to false so gate 2 cannot emit a CPU
        // group that gate 1 never allocated state slots or frame bytes for.
        m3d_spike: spike_candidates.is_some(),
        m3d_spike_cpu_result_names: Vec::new(),
        m3d_spike_cpu_result_allocas: HashMap::new(), // populated in Step 1c below
    };

    // Step 1 — Emit allocas in the entry block (sm_entry). LLVM SSA requires all allocas
    // to be in the function entry block so they dominate every use across all state blocks.
    // Both parameters AND crossing locals get i64 allocas here; each is loaded from its
    // frame slot at the start of every continuation state block.
    cg_resume.builder.position_at_end(resume_entry);
    for pname in &param_names {
        // Alloca sized to i64 (the frame slot width). The actual LLVM type is i64.
        let alloca = cg_resume
            .builder
            .build_alloca(cg_resume.i64(), &format!("{pname}_alloca"))
            .map_err(|e| format!("sm param alloca: {e}"))?;
        // Register in locals map — state blocks load from these allocas.
        cg_resume.locals.insert(pname.clone(), alloca);
    }
    // Crossing locals also get allocas in sm_entry so lower_stmt can reuse them.
    // LLVM SSA requires allocas to dominate all uses — sm_entry dominates all state
    // blocks, so every crossing local's alloca is visible in every resume state.
    //
    // Per-type alloca strategy (types wider than 8 bytes cannot use a stack pointer
    // stored in the frame — the resume fn's stack is destroyed between calls):
    //   int / bool         → i64 alloca; raw i64 load/store in frame slot
    //   float              → f64 alloca; bitcast f64↔i64 for frame slot
    //   decimal128         → i128 alloca; 2 consecutive frame slots (lo + hi)
    //   ErrorsCapable      → ptr alloca (holds ptr to companion {i64,i64} alloca);
    //                        2 frame slots for the two i64 fields; companion struct
    //                        alloca is also in sm_entry so it dominates all states
    //   Shape              → ptr alloca; pre-wired to the composed frame's slot region
    //                        (frame-embed, not heap-promotion); bytes live directly in
    //                        the frame — no separate allocation needed
    //   string/array/map   → ptr alloca; pointer already lives on the heap (stable)
    {
        let mut scalar_set: HashSet<String> = HashSet::new();
        let mut bool_set: HashSet<String> = HashSet::new();
        let mut decimal128_set: HashSet<String> = HashSet::new();
        let mut float_set: HashSet<String> = HashSet::new();
        let mut errors_capable_set: HashSet<String> = HashSet::new();
        let mut ec_number_set: HashSet<String> = HashSet::new();
        let mut ec_number_i128_allocas: HashMap<String, PointerValue<'ctx>> = HashMap::new();
        let mut shape_embed_set: HashSet<String> = HashSet::new();
        let mut ec_struct_allocas: HashMap<String, PointerValue<'ctx>> = HashMap::new();
        let mut shape_names_map: HashMap<String, String> = HashMap::new();
        let mut shape_allocas_map: HashMap<String, PointerValue<'ctx>> = HashMap::new();

        for cname in &crossing_names {
            // Resolve the typeck type (catches inferred types like number).
            let crossing_ty = crossing_local_type_from_body(&f.body, cname.as_str(), &cg_resume);
            // Cross-check against typeck expr_types for decimal128 (annotation may miss inferred).
            let crossing_ty = {
                let typeck_ty = find_let_typeck_type_in_stmts(&f.body.stmts, cname.as_str(), typed);
                match typeck_ty {
                    Some(ty @ Type::Number { .. }) => ty,
                    _ => crossing_ty,
                }
            };

            // Classify the crossing local so flush/reload know which strategy to use.
            // Bool is separate from Int: both get 1 frame slot, but Bool's alloca is i1
            // (matching the rest of codegen) while Int's is i64. Flush zexts i1→i64;
            // reload truncates the i64 frame slot back to i1.
            let is_int = matches!(&crossing_ty, Type::Int);
            let is_bool = matches!(&crossing_ty, Type::Bool);
            let is_float = matches!(&crossing_ty, Type::Float);
            let is_decimal128 =
                matches!(&crossing_ty, Type::Number { precision } if *precision <= 34);
            let is_errors_capable = matches!(&crossing_ty, Type::ErrorsCapable { .. });
            // EC<Number>: ErrorsCapable wrapping decimal128. Uses 3 frame slots
            // ({f0, i128_lo, i128_hi}) so the decimal bits are stable across resumes
            // even when the callee's staging slot is reused by a second same-callee call.
            let is_ec_number = matches!(&crossing_ty,
                Type::ErrorsCapable { inner }
                    if matches!(inner.as_ref(), Type::Number { precision } if *precision <= 34));
            let is_shape = matches!(&crossing_ty, Type::Shape { .. });

            if is_int {
                scalar_set.insert(cname.clone());
            }
            if is_bool {
                bool_set.insert(cname.clone());
            }
            if is_float {
                float_set.insert(cname.clone());
            }
            if is_decimal128 {
                decimal128_set.insert(cname.clone());
            }
            if is_errors_capable {
                errors_capable_set.insert(cname.clone());
            }
            if is_ec_number {
                ec_number_set.insert(cname.clone());
            }
            if is_shape {
                shape_embed_set.insert(cname.clone());
                if let Type::Shape { name: ref sn } = crossing_ty {
                    shape_names_map.insert(cname.clone(), sn.clone());
                }
            }

            // Create the sm_entry alloca for this crossing local.
            //
            // Per-type alloca strategy (types wider than 8 bytes cannot use a stack pointer
            // stored in the frame — the resume fn's stack is destroyed between calls):
            //   int              → i64 alloca; raw i64 load/store in frame slot
            //   bool             → i1 alloca (matches rest of codegen); flush zexts i1→i64,
            //                      reload truncates i64→i1; 1 frame slot
            //   float            → f64 alloca; bitcast f64↔i64 for frame slot
            //   decimal128       → i128 alloca; 2 consecutive frame slots (lo + hi)
            //   ErrorsCapable    → ptr alloca (holds ptr to companion {i64,i64} alloca);
            //                      2 frame slots for the two i64 fields
            //   Shape            → ptr alloca; pre-initialized to point into the composed
            //                      frame's slot region (see Step 1b below); bytes are
            //                      stored directly in the frame — no separate heap alloc
            //   string/array/map → ptr alloca; pointer already lives on the heap (stable)
            let llvm_ty: inkwell::types::BasicTypeEnum<'ctx> = match &crossing_ty {
                Type::Int => cg_resume.i64().into(),
                // Bool keeps its natural i1 alloca; flush/reload convert at the frame boundary.
                Type::Bool => cg_resume.ctx.bool_type().into(),
                Type::Float => cg_resume.ctx.f64_type().into(),
                // decimal128: i128 alloca; 2 consecutive frame slots hold the bits directly.
                Type::Number { precision } if *precision <= 34 => cg_resume.ctx.i128_type().into(),
                // All pointer-backed types (Shape, ErrorsCapable, string, array):
                // ptr alloca holds the pointer.
                _ => cg_resume.ctx.ptr_type(AddressSpace::default()).into(),
            };
            let alloca = cg_resume
                .builder
                .build_alloca(llvm_ty, &format!("{cname}_alloca"))
                .map_err(|e| format!("sm crossing alloca {cname}: {e}"))?;
            cg_resume.locals.insert(cname.clone(), alloca);
            if is_shape {
                shape_allocas_map.insert(cname.clone(), alloca);
            }

            // ErrorsCapable: also create a companion {i64,i64} struct alloca in sm_entry.
            // Its contents are refreshed from the two frame slots on every reload.
            // The ptr alloca above holds the address of this struct alloca — stable
            // across resumes because sm_entry allocas dominate all state blocks.
            if is_errors_capable {
                let ec_struct_ty = cg_resume
                    .ctx
                    .struct_type(&[cg_resume.i64().into(), cg_resume.i64().into()], false);
                let ec_struct_alloca = cg_resume
                    .builder
                    .build_alloca(ec_struct_ty, &format!("{cname}_ec_struct"))
                    .map_err(|e| format!("sm ec struct alloca {cname}: {e}"))?;
                // Wire the ptr alloca to point at the companion struct.
                cg_resume
                    .builder
                    .build_store(alloca, ec_struct_alloca)
                    .map_err(|e| format!("sm ec ptr init {cname}: {e}"))?;
                ec_struct_allocas.insert(cname.clone(), ec_struct_alloca);

                // EC<Number>: also create a per-binding i128 alloca to hold the decimal
                // bits between suspend and resume. On every reload, the 3 frame slots
                // (f0, i128_lo, i128_hi) are reconstructed here and f1 is set to point
                // at this alloca. The alloca is in sm_entry so it dominates all states.
                if is_ec_number {
                    let i128_alloca = cg_resume
                        .builder
                        .build_alloca(cg_resume.ctx.i128_type(), &format!("{cname}_ec_num_i128"))
                        .map_err(|e| format!("sm ec num i128 alloca {cname}: {e}"))?;
                    ec_number_i128_allocas.insert(cname.clone(), i128_alloca);
                }
            }
        }
        cg_resume.sm_crossing_scalar_set = scalar_set;
        cg_resume.sm_crossing_bool_set = bool_set;
        cg_resume.sm_crossing_decimal128_set = decimal128_set;
        cg_resume.sm_crossing_float_set = float_set;
        cg_resume.sm_crossing_errors_capable_set = errors_capable_set;
        cg_resume.sm_crossing_ec_number_set = ec_number_set;
        cg_resume.sm_crossing_ec_number_i128_allocas = ec_number_i128_allocas;
        cg_resume.sm_crossing_shape_embed_set = shape_embed_set;
        cg_resume.sm_crossing_ec_struct_allocas = ec_struct_allocas;
        cg_resume.sm_crossing_shape_names = shape_names_map;
        cg_resume.sm_crossing_shape_allocas = shape_allocas_map;
    }

    // Step 1c — Pre-allocate CPU-group result allocas in sm_entry (spike only).
    //
    // emit_cpu_group_spawn_join stores results into these allocas after the join completes
    // (all_done_bb). spike_reload_cpu_results_from_frame reloads frame bytes into these
    // same allocas after every subsequent suspension. Both use the alloca via cg.locals.
    //
    // Pre-allocation here — while the builder is still in sm_entry (resume_entry) — is
    // required so that the allocas dominate all state blocks. Creating allocas later (e.g.
    // in a state block like cont_state_bb) would violate LLVM SSA dominance: a load in a
    // different state block would not be dominated by the alloca's defining block.
    //
    // The candidate names are extracted by the same adjacency + arg-lowering + dependency
    // scan as spike_extract_cpu_group so the sets always agree. When spike_cpu_group_result_names
    // declines a group (suspending callee or mutation in rest stmts), it returns an empty set,
    // so no allocas are pre-allocated for that group. This function is invoked only when
    // cg_resume.m3d_spike is true, meaning spike_cpu_candidates already admitted the group;
    // all three gates use identical predicates and therefore agree on admission.
    if cg_resume.m3d_spike {
        let candidate_names = spike_cpu_group_result_names(&f.body.stmts, suspend_set, typed);
        let mut result_allocas: HashMap<String, PointerValue> = HashMap::new();
        for name in &candidate_names {
            let alloca = cg_resume
                .builder
                .build_alloca(cg_resume.i64(), &format!("{name}_result_alloca"))
                .map_err(|e| format!("spike result pre-alloc `{name}`: {e}"))?;
            cg_resume.locals.insert(name.clone(), alloca);
            result_allocas.insert(name.clone(), alloca);
        }
        cg_resume.m3d_spike_cpu_result_allocas = result_allocas;
    }

    // Step 1b — Wire shape crossing-local ptr allocas to point into the composed frame.
    //
    // Shape crossing locals use frame-embedding: their struct bytes live directly in the
    // composed heap frame's slot region (consecutive i64 slots). The ptr alloca (created
    // in Step 1 above) is pre-initialized here to hold a pointer to that slot region.
    // This means:
    //   - Field accesses (load ptr → GEP into struct) now GEP into the frame directly
    //   - Writes to shape fields go directly to the frame — no flush needed
    //   - Across suspension boundaries, the frame already holds the current bytes
    //   - At reload, the ptr alloca is re-initialized to the same frame offset — no reload needed
    // One ynz_alloc backs the whole task tree; shape crossing locals live inline
    // in the composed frame's slot region — no per-shape allocation is needed.
    {
        let shape_names = cg_resume.sm_crossing_shape_names.clone();
        let shape_alloca_map = cg_resume.sm_crossing_shape_allocas.clone();
        for (cname, alloca) in &shape_alloca_map {
            // Find this crossing local's slot index.
            let pos = crossing_names
                .iter()
                .position(|n| n == cname)
                .ok_or_else(|| {
                    format!("sm shape wire: crossing local `{cname}` not found in crossing_names")
                })?;
            let slot_idx = crossing_slot_indices[pos];
            let shape_name = shape_names
                .get(cname.as_str())
                .ok_or_else(|| format!("sm shape wire: shape name for `{cname}` not found"))?;
            // Compute the GEP into the frame's slot region for this shape.
            let frame_slot_byte_offset = state_machine::FRAME_OFFSET_LOCALS_START
                + (slot_idx as u64) * state_machine::FRAME_LOCAL_SLOT_SIZE;
            let shape_region_ptr = unsafe {
                cg_resume
                    .builder
                    .build_gep(
                        ctx.i8_type(),
                        frame_param,
                        &[ctx.i64_type().const_int(frame_slot_byte_offset, false)],
                        &format!("{cname}_frame_region"),
                    )
                    .map_err(|e| format!("sm shape frame GEP {cname}: {e}"))?
            };
            // Verify the struct type is known (for documentation; GEP is byte-level).
            let _ = cg_resume
                .shape_types
                .get(shape_name.as_str())
                .ok_or_else(|| format!("sm shape wire: LLVM type for `{shape_name}` not found"))?;
            // Store the frame region ptr into the ptr alloca so field access GEPs land
            // directly in the frame. This is the sole persistent source of truth — no
            // separate allocation, no flush/reload for shape bytes.
            cg_resume
                .builder
                .build_store(*alloca, shape_region_ptr)
                .map_err(|e| format!("sm shape wire store {cname}: {e}"))?;
        }
    }

    // Step 2 — Emit the switch on resume_point (still in sm_entry, after allocas).
    {
        let rp = state_machine::load_resume_point(ctx, &cg_resume.builder, frame_param)?;
        let cases: Vec<(
            inkwell::values::IntValue<'ctx>,
            inkwell::basic_block::BasicBlock<'ctx>,
        )> = state_blocks
            .iter()
            .enumerate()
            .map(|(i, &bb)| (ctx.i32_type().const_int(i as u64, false), bb))
            .collect();
        cg_resume
            .builder
            .build_switch(rp, dead_block, &cases)
            .map_err(|e| format!("sm switch: {e}"))?;
    }

    // Step 3 — Build the pending return block.
    {
        let builder = ctx.create_builder();
        builder.position_at_end(pending_block);
        builder
            .build_return(Some(&ctx.i32_type().const_int(1, false)))
            .map_err(|e| format!("pending ret: {e}"))?;
    }

    // Step 4 — Build the dead block (unreachable in correct codegen).
    {
        let builder = ctx.create_builder();
        builder.position_at_end(dead_block);
        builder
            .build_return(Some(&ctx.i32_type().const_int(0, false)))
            .map_err(|e| format!("dead ret: {e}"))?;
    }

    // Step 5 — Emit state_blocks[0] (initial state). Load params from frame into allocas.
    // Crossing locals are NOT reloaded at state 0 — they are defined inline later in the
    // function body and stored to the frame at their definition site. Reloading them here
    // would read uninitialized frame bytes (the frame was zeroed by ynz_alloc_zeroed, so
    // it's safe memory-wise, but the value would be 0 rather than the actual value).
    cg_resume.builder.position_at_end(state_blocks[0]);

    for (slot_idx, pname) in param_names.iter().enumerate() {
        let bits =
            state_machine::load_local_slot(ctx, &cg_resume.builder, frame_param, slot_idx, pname)?;
        let param_decl = &f.params[slot_idx];
        let param_ty = ast_type_to_typeck_type(&param_decl.ty, shape_table);
        // Reconstruct the LLVM value from i64 bits, then store into the alloca.
        let val = cg_resume
            .i64_bits_to(bits, &param_ty)
            .map_err(|e| format!("sm param reconstruct: {e}"))?;
        let alloca = *cg_resume
            .locals
            .get(pname)
            .ok_or_else(|| format!("sm: alloca for {pname} not found"))?;
        // Store directly regardless of type (the alloca is i64; store the raw bits).
        let bits_for_store = cg_resume
            .to_i64_bits(val, &param_ty)
            .map_err(|e| format!("sm param to_bits: {e}"))?;
        cg_resume
            .builder
            .build_store(alloca, bits_for_store)
            .map_err(|e| format!("sm param store: {e}"))?;
    }

    // Emit the function body statements, intercepting waits at the right state boundaries.
    // crossing_names is threaded through so lower_sm_body can flush/reload crossing locals
    // at suspension boundaries.
    lower_sm_body(
        &mut cg_resume,
        &f.body,
        &state_blocks,
        pending_block,
        frame_param,
        waker_param,
        &param_names,
        f,
        shape_table,
    )?;

    // ── Part 2: Generate the wrapper function ──────────────────────────────────

    let wrapper_fn = module
        .get_function(llvm_name)
        .ok_or_else(|| format!("wrapper fn `{llvm_name}` not forward-declared"))?;

    let builder = ctx.create_builder();
    let entry_bb = ctx.append_basic_block(wrapper_fn, "entry");
    builder.position_at_end(entry_bb);

    // Main-specific: emit rt_init (FIRST instruction per AC #5) and siphash_init.
    if is_main {
        builder
            .build_call(rt.ynz_siphash_init, &[], "siphash_init")
            .map_err(|e| format!("siphash_init: {e}"))?;
        // ynz_rt_init is the FIRST non-allocation instruction (AC: main_rt_init_is_first_instruction).
        builder
            .build_call(rt.ynz_rt_init, &[], "rt_init")
            .map_err(|e| format!("rt_init: {e}"))?;
    }

    // Allocate the COMPOSED frame (covers header + own locals + all embedded child sub-frames).
    // ONE allocation per spawned task tree — Rust-async-model performance.
    let frame_ptr = state_machine::alloc_frame(ctx, &builder, rt, frame_bytes)?;

    // Write each parameter to its frame slot (locals start at offset 32).
    for (slot_idx, param) in f.params.iter().enumerate() {
        let llvm_param = wrapper_fn
            .get_nth_param(slot_idx as u32)
            .ok_or_else(|| format!("wrapper: missing param {slot_idx}"))?;
        let param_ty = ast_type_to_typeck_type(&param.ty, shape_table);
        let bits = value_to_i64_bits(&builder, ctx.i64_type(), llvm_param, &param_ty)
            .map_err(|e| format!("param to bits: {e}"))?;
        state_machine::store_local_slot(ctx, &builder, frame_ptr, slot_idx, bits)?;
    }

    // Drive the state machine to completion via ynz_rt_run_entrypoint — the program-entry
    // driver (tokio::main-equivalent). This is the ONLY call site: the wrapper→resume
    // handoff at the top level of a suspending function's wrapper. The resume function
    // itself never calls this driver; it inline-poll-yields into embedded child sub-frames.
    let resume_fn_ptr = resume_fn.as_global_value().as_pointer_value();
    let frame_size_val = ctx.i64_type().const_int(frame_bytes, false);

    builder
        .build_call(
            rt.ynz_rt_run_entrypoint,
            &[
                resume_fn_ptr.into(),
                frame_ptr.into(),
                frame_size_val.into(),
            ],
            "sm_drive",
        )
        .map_err(|e| format!("sm_drive call: {e}"))?;

    // Read the typed return value from the return slot at offset 16.
    // The resume function stored the typed value there (instead of the old i32-truncation
    // into frame[0] which was the i32-truncation defect fixed in Phase 7).
    let ret_ty = ast_type_to_typeck_type(&f.return_type, shape_table);
    let is_errors_capable = f.errors_capable;

    // Free the frame before constructing the return value — the frame slots were already
    // read above; no further access to frame_ptr after free.
    // EXCEPTION: for errors-capable returns we must read the errors struct BEFORE freeing,
    // then free, then return. We defer the free to after the read below.

    if is_main {
        // For main: read return value from typed return slot, free frame, shutdown, return exit code.
        // The return value for main (-> nothing or -> int) is an i64 in the return slot.
        let exit_i64 = state_machine::load_return_value_i64(ctx, &builder, frame_ptr, "exit_i64")?;
        state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
        builder
            .build_call(rt.ynz_rt_shutdown, &[], "rt_shutdown")
            .map_err(|e| format!("rt_shutdown: {e}"))?;
        // Truncate i64 → i32 for C main's exit code (AC: entrypoint -> int → exit $?).
        let exit_i32 = builder
            .build_int_truncate(exit_i64, ctx.i32_type(), "exit_i32")
            .map_err(|e| format!("exit truncate: {e}"))?;
        builder
            .build_return(Some(&exit_i32))
            .map_err(|e| format!("main ret: {e}"))?;
    } else if is_errors_capable {
        // Errors-capable wrapper: read {i64, i64} from return slot, free frame, return struct.
        //
        // For `-> number errors` (inner type = decimal128), the ok-word is a pointer into the
        // composed heap frame's 16-byte staging slot. Freeing the frame makes that pointer
        // dangling. Fix: on the success path (err_i64 == 0), copy the i128 from the staging
        // slot into a heap allocation BEFORE freeing the frame, and return the heap pointer as
        // the new ok-word. The heap allocation ownership transfers to the caller; it is freed
        // when the caller calls `.or()` / `.orError()` (the error runtime handles EC lifetime).
        //
        // For other EC inner types (int, bool, ptr — all fit in the i64 ok-word directly),
        // no staging slot is involved, so the two-step read-then-free is safe as-is.
        let (err_i64, mut ok_i64) =
            state_machine::load_return_value_errors(ctx, &builder, frame_ptr)?;

        // `-> number errors`: copy the i128 out of the staging slot before freeing the frame.
        let is_number_errors_inner =
            matches!(&ret_ty, Type::Number { precision } if *precision <= 34);
        if is_number_errors_inner {
            // Guard the staging-slot deref: only valid on the success path (err_i64 == 0).
            // On the error path ok_i64 == 0 — dereferencing it is a null crash.
            let is_ok = builder
                .build_int_compare(
                    IntPredicate::EQ,
                    err_i64,
                    ctx.i64_type().const_int(0, false),
                    "wrap_ec_isok",
                )
                .map_err(|e| format!("ec wrapper isok cmp: {e}"))?;
            let wrap_copy_bb = ctx.append_basic_block(wrapper_fn, "wrap_ec_copy");
            let wrap_merge_bb = ctx.append_basic_block(wrapper_fn, "wrap_ec_merge");
            builder
                .build_conditional_branch(is_ok, wrap_copy_bb, wrap_merge_bb)
                .map_err(|e| format!("ec wrapper cob branch: {e}"))?;

            // Success path: load i128 from staging slot (still live in frame), copy to heap.
            builder.position_at_end(wrap_copy_bb);
            let staging_ptr = builder
                .build_int_to_ptr(
                    ok_i64,
                    ctx.ptr_type(AddressSpace::default()),
                    "wrap_ec_sptr",
                )
                .map_err(|e| format!("ec wrapper int_to_ptr: {e}"))?;
            let i128_val = builder
                .build_load(ctx.i128_type(), staging_ptr, "wrap_ec_i128")
                .map_err(|e| format!("ec wrapper load i128: {e}"))?
                .into_int_value();
            let heap_ptr = builder
                .build_call(
                    rt.ynz_alloc,
                    &[ctx.i64_type().const_int(16, false).into()],
                    "wrap_ec_heap",
                )
                .map_err(|e| format!("ec wrapper heap alloc: {e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ec wrapper heap alloc: expected ptr")?
                .into_pointer_value();
            builder
                .build_store(heap_ptr, i128_val)
                .map_err(|e| format!("ec wrapper store i128: {e}"))?;
            let heap_i64 = builder
                .build_ptr_to_int(heap_ptr, ctx.i64_type(), "wrap_ec_hptr_i64")
                .map_err(|e| format!("ec wrapper ptr_to_int: {e}"))?;
            builder
                .build_unconditional_branch(wrap_merge_bb)
                .map_err(|e| format!("ec wrapper cob->merge: {e}"))?;

            // Merge: ok_i64 is the heap pointer on success, 0 (untouched) on error.
            builder.position_at_end(wrap_merge_bb);
            let phi = builder
                .build_phi(ctx.i64_type(), "wrap_ec_ok")
                .map_err(|e| format!("ec wrapper ok phi: {e}"))?;
            // Entry BB of the is_errors_capable branch falls through to the isok branch,
            // then to wrap_copy_bb or wrap_merge_bb. The predecessor of wrap_merge_bb
            // that did NOT take the copy path is the BB before wrap_copy_bb.
            let pre_copy_bb = wrap_copy_bb
                .get_previous_basic_block()
                .ok_or("ec wrapper: no predecessor of wrap_copy_bb")?;
            phi.add_incoming(&[(&heap_i64, wrap_copy_bb), (&ok_i64, pre_copy_bb)]);
            ok_i64 = phi.as_basic_value().into_int_value();
        }

        state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
        // Reconstruct the {i64, i64} struct value for the caller.
        let struct_ty = ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false);
        let mut result = struct_ty.const_zero();
        result = builder
            .build_insert_value(result, err_i64, 0, "ec_err")
            .map_err(|e| format!("ec_err insert: {e}"))?
            .into_struct_value();
        result = builder
            .build_insert_value(result, ok_i64, 1, "ec_ok")
            .map_err(|e| format!("ec_ok insert: {e}"))?
            .into_struct_value();
        builder
            .build_return(Some(&result))
            .map_err(|e| format!("ec wrapper ret: {e}"))?;
    } else {
        // Non-main, non-errors wrapper: read return slot and return typed value.
        match &ret_ty {
            Type::Nothing => {
                state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
                builder
                    .build_return(None)
                    .map_err(|e| format!("wrapper void ret: {e}"))?;
            }
            Type::Int => {
                let val =
                    state_machine::load_return_value_i64(ctx, &builder, frame_ptr, "ret_i64")?;
                state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
                builder
                    .build_return(Some(&val))
                    .map_err(|e| format!("wrapper int ret: {e}"))?;
            }
            Type::Bool => {
                // The frame return slot stores bool as i64 (zext on write). The wrapper
                // function is declared with i1 return type (per declare_function). Truncate
                // i64→i1 here to match; without the trunc LLVM rejects "ret i64 ... i1".
                let as_i64 =
                    state_machine::load_return_value_i64(ctx, &builder, frame_ptr, "ret_bool_i64")?;
                state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
                let as_i1 = builder
                    .build_int_truncate(as_i64, ctx.bool_type(), "ret_bool_i1")
                    .map_err(|e| format!("wrapper bool trunc: {e}"))?;
                builder
                    .build_return(Some(&as_i1))
                    .map_err(|e| format!("wrapper bool ret: {e}"))?;
            }
            Type::Float => {
                // Float: the resume fn stored the f64 as i64 bits (bitcast) in the slot.
                // Load the i64 and bitcast back to f64 before returning — the declared
                // wrapper return type is f64, so returning the raw i64 would cause LLVM
                // to emit "ret i64 ... double" and fail module verification.
                let f_val =
                    state_machine::load_return_value_f64(ctx, &builder, frame_ptr, "ret_f64")?;
                state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
                builder
                    .build_return(Some(&f_val))
                    .map_err(|e| format!("wrapper float ret: {e}"))?;
            }
            Type::Number { precision } if *precision <= 34 => {
                // Decimal128 (i128): the resume fn stored the full 16-byte i128 value
                // directly in the 16-byte return slot. The wrapper is declared as ptr-returning
                // (matching the non-SM number ABI: callers expect a pointer to a heap i128).
                // Allocate 16 bytes, copy the i128 from the return slot, free the frame, then
                // return the heap pointer — the caller owns the allocation and may read from it.
                let i128_val =
                    state_machine::load_return_value_i128(ctx, &builder, frame_ptr, "ret_i128")?;
                // Allocate 16 bytes for the i128 return value on the heap.
                let heap_ptr = builder
                    .build_call(
                        rt.ynz_alloc,
                        &[ctx.i64_type().const_int(16, false).into()],
                        "ret_dec_alloc",
                    )
                    .map_err(|e| format!("ret_dec_alloc: {e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ret_dec_alloc: expected ptr")?
                    .into_pointer_value();
                builder
                    .build_store(heap_ptr, i128_val)
                    .map_err(|e| format!("ret_dec_store: {e}"))?;
                state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
                builder
                    .build_return(Some(&heap_ptr))
                    .map_err(|e| format!("wrapper number ret: {e}"))?;
            }
            Type::String
            | Type::Shape { .. }
            | Type::BuiltinArray { .. }
            | Type::BuiltinFixed { .. }
            | Type::Maybe { .. }
            | Type::BuiltinMap { .. }
            | Type::Union { .. }
            | Type::Sensitive { .. } => {
                let as_i64 =
                    state_machine::load_return_value_i64(ctx, &builder, frame_ptr, "ret_ptr_i64")?;
                state_machine::free_frame(ctx, &builder, rt, frame_ptr, frame_bytes)?;
                // Convert i64 back to pointer.
                let ptr_val = builder
                    .build_int_to_ptr(as_i64, ctx.ptr_type(AddressSpace::default()), "ret_ptr")
                    .map_err(|e| format!("ret_ptr cast: {e}"))?;
                builder
                    .build_return(Some(&ptr_val))
                    .map_err(|e| format!("wrapper ptr ret: {e}"))?;
            }
            _ => {
                // Any Yinz type not handled above (e.g., a new type added to the AST without
                // a corresponding wrapper-return arm) must fail loud at compile time — not
                // silently emit `ret void` and crash the LLVM backend. The fallback void
                // return was the original source of the Float/Number module-verification bug.
                return Err(format!(
                    "BUG: SM wrapper-return has no arm for return type {ret_ty:?}. \
                     Add an explicit arm in lower_function_with_waits wrapper-return match. \
                     Emitting `ret void` would produce an LLVM module verification failure."
                ));
            }
        }
    }

    Ok(())
}

/// Count suspension points in a block: `Expr::Wait` nodes PLUS calls to suspending callees.
///
/// Used by `lower_function_with_waits` to pre-allocate state blocks. Every explicit `wait` AND
/// every call to a suspending callee (regardless of whether `wait` was written) needs one
/// continuation state for the poll-loop re-entry.
fn count_suspension_points(block: &ynz_ast::nodes::Block, suspend_set: &SuspendSet) -> usize {
    block
        .stmts
        .iter()
        .map(|s| count_suspension_stmt(s, suspend_set))
        .sum()
}

fn count_suspension_stmt(stmt: &Stmt, suspend_set: &SuspendSet) -> usize {
    match stmt {
        Stmt::Expr(e) => count_suspension_expr(e, suspend_set),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            count_suspension_expr(value, suspend_set)
        }
        Stmt::Return { value, .. } => value
            .as_ref()
            .map_or(0, |e| count_suspension_expr(e, suspend_set)),
        Stmt::FieldAssign { target, value, .. } => {
            count_suspension_expr(target, suspend_set) + count_suspension_expr(value, suspend_set)
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            count_suspension_expr(receiver, suspend_set)
                + count_suspension_expr(index, suspend_set)
                + count_suspension_expr(value, suspend_set)
        }
        Stmt::If { cond, body, .. } => {
            count_suspension_expr(cond, suspend_set) + count_suspension_points(body, suspend_set)
        }
        Stmt::While { cond, body, .. } => {
            count_suspension_expr(cond, suspend_set) + count_suspension_points(body, suspend_set)
        }
        Stmt::For { iter, body, .. } => {
            count_suspension_expr(iter, suspend_set) + count_suspension_points(body, suspend_set)
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            count_suspension_expr(scrutinee, suspend_set)
                + arms
                    .iter()
                    .map(|a| count_suspension_points(&a.body, suspend_set))
                    .sum::<usize>()
                + else_arm
                    .as_ref()
                    .map_or(0, |b| count_suspension_points(b, suspend_set))
        }
    }
}

fn count_suspension_expr(expr: &Expr, suspend_set: &SuspendSet) -> usize {
    match expr {
        // Explicit `wait` of any kind = 1 suspension point.
        Expr::Wait(..) => 1,
        // Direct call to a suspending user-defined fn (without explicit `wait`) = 1 suspension point.
        Expr::Call(c) => {
            if let Expr::Ident(name, _) = &c.callee {
                if suspend_set.contains(name.as_str())
                    && !M2_MAY_BLOCK_INTRINSICS.contains(&name.as_str())
                {
                    // This suspending call needs a poll-loop state.
                    return 1 + c
                        .args
                        .iter()
                        .map(|a| count_suspension_expr(a, suspend_set))
                        .sum::<usize>();
                }
            }
            c.args
                .iter()
                .map(|a| count_suspension_expr(a, suspend_set))
                .sum()
        }
        Expr::BinOp { lhs, rhs, .. } => {
            count_suspension_expr(lhs, suspend_set) + count_suspension_expr(rhs, suspend_set)
        }
        Expr::UnaryOp { operand, .. } => count_suspension_expr(operand, suspend_set),
        Expr::MethodCall { receiver, args, .. } => {
            count_suspension_expr(receiver, suspend_set)
                + args
                    .iter()
                    .map(|a| count_suspension_expr(a, suspend_set))
                    .sum::<usize>()
        }
        Expr::Background(inner, _) => {
            // background calls spawn separately — NOT a suspension point in the parent.
            let _ = inner;
            0
        }
        _ => 0,
    }
}

/// True if the statement contains a direct call to a suspending user-defined function.
///
/// Used alongside `stmt_contains_wait` to detect suspension points that don't have an
/// explicit `wait` token (transitive case). Background expressions are excluded because
/// they spawn independently and don't suspend the current function.
fn stmt_contains_suspending_call(stmt: &Stmt, suspend_set: &SuspendSet) -> bool {
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
            expr_contains_suspending_call(cond, suspend_set)
                || body
                    .stmts
                    .iter()
                    .any(|s| stmt_contains_suspending_call(s, suspend_set))
        }
        Stmt::While { cond, body, .. } => {
            expr_contains_suspending_call(cond, suspend_set)
                || body
                    .stmts
                    .iter()
                    .any(|s| stmt_contains_suspending_call(s, suspend_set))
        }
        Stmt::For { iter, body, .. } => {
            expr_contains_suspending_call(iter, suspend_set)
                || body
                    .stmts
                    .iter()
                    .any(|s| stmt_contains_suspending_call(s, suspend_set))
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_contains_suspending_call(scrutinee, suspend_set)
                || arms.iter().any(|a| {
                    a.body
                        .stmts
                        .iter()
                        .any(|s| stmt_contains_suspending_call(s, suspend_set))
                })
                || else_arm.as_ref().is_some_and(|b| {
                    b.stmts
                        .iter()
                        .any(|s| stmt_contains_suspending_call(s, suspend_set))
                })
        }
    }
}

/// True if `stmt` or any statement nested inside it is a `Stmt::Assign` whose target
/// equals `name`.
///
/// Used by the spike static admission gates (`spike_cpu_candidates`, `spike_extract_cpu_group`,
/// `spike_cpu_group_result_names`) to detect assignments to CPU-result locals at any nesting
/// depth. When any rest statement assigns a CPU-result bind name, the gates decline the whole
/// group so that sequential lowering (which is always correct) handles the program instead.
/// This ensures the spike only emits code for programs in the proven-safe admission envelope.
///
/// Time: O(n) where n = total AST nodes in stmt  Space: O(d) call-stack depth where d = nesting
fn stmt_assigns_name(stmt: &Stmt, name: &str) -> bool {
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
        Expr::Background(_inner, _) => false, // background spawns separately, doesn't suspend current fn
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

/// Reload parameters and crossing locals from their frame slots into allocas.
///
/// Called at the start of EVERY continuation state block (state 1+). Each call to the
/// resume function creates fresh allocas (the stack frame is new); without reloading,
/// continuation states see uninitialized allocas for values that lived across the
/// previous suspension.
///
/// Parameters occupy slots [0..n_params); crossing locals occupy slots
/// [n_params..n_params+n_crossing). Both sets are reloaded here — parameters because
/// they are always live, crossing locals because they were stored to the frame just
/// before the suspension and must be restored for post-wait code.
///
/// State 0 does NOT call this for crossing locals (they are defined inline and stored
/// to the frame at their definition site — no prior stored value exists to reload).
/// `reload_crossing` controls whether frame-backed crossing locals are also reloaded.
/// Pass `false` for state 0 (locals not yet stored) and `true` for all continuation states.
fn reload_params_from_frame<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    frame_ptr: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &ShapeTable,
    reload_crossing: bool,
) -> Result<(), String> {
    let ctx = cg.ctx;
    // Reload parameters.
    for (slot_idx, pname) in param_names.iter().enumerate() {
        let bits = state_machine::load_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, pname)?;
        let param_decl = &f.params[slot_idx];
        let param_ty = ast_type_to_typeck_type(&param_decl.ty, shape_table);
        let alloca = *cg
            .locals
            .get(pname)
            .ok_or_else(|| format!("sm reload: alloca for {pname} missing"))?;
        let bits_to_store = cg.to_i64_bits(cg.i64_bits_to(bits, &param_ty)?, &param_ty)?;
        cg.builder
            .build_store(alloca, bits_to_store)
            .map_err(|e| format!("sm reload store {pname}: {e}"))?;
    }
    // Reload crossing locals from their frame slots (defined after state 0, live at state 1+).
    // The crossing names and slot-start are stored on the Cg when inside a SM resume fn.
    if reload_crossing {
        if let Some(ref crossing_names) = cg.sm_crossing_names.clone() {
            let slot_indices = cg.sm_crossing_slot_indices.clone();
            let scalar_set = cg.sm_crossing_scalar_set.clone();
            let bool_set = cg.sm_crossing_bool_set.clone();
            let float_set = cg.sm_crossing_float_set.clone();
            let decimal128_set = cg.sm_crossing_decimal128_set.clone();
            let errors_capable_set = cg.sm_crossing_errors_capable_set.clone();
            let ec_number_set = cg.sm_crossing_ec_number_set.clone();
            let ec_number_i128_allocas = cg.sm_crossing_ec_number_i128_allocas.clone();
            let shape_embed_set = cg.sm_crossing_shape_embed_set.clone();
            let ec_struct_allocas = cg.sm_crossing_ec_struct_allocas.clone();
            // Spike CPU-result names are reloaded exclusively by
            // spike_reload_cpu_results_from_frame (called below, after the crossing loop).
            // That function plants fresh allocas from SPIKE_RESULT_N_OFFSET — the fixed
            // per-result frame slots. Letting the crossing loop handle these names would use
            // sm_crossing_slot_indices (SM slot indices, NOT the fixed result offsets) and
            // would clobber the freshly-planted allocas.
            let spike_pairs = cg.m3d_spike_cpu_result_names.clone();
            for (i, cname) in crossing_names.iter().enumerate() {
                if !spike_pairs.is_empty() && spike_pairs.iter().any(|(n, _)| n == cname) {
                    continue;
                }
                let slot_idx = slot_indices[i];
                let bits =
                    state_machine::load_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, cname)?;
                if let Some(&alloca) = cg.locals.get(cname) {
                    let is_int = scalar_set.contains(cname.as_str());
                    let is_bool = bool_set.contains(cname.as_str());
                    let is_float = float_set.contains(cname.as_str());
                    let is_decimal128 = decimal128_set.contains(cname.as_str());
                    let is_errors_capable = errors_capable_set.contains(cname.as_str());
                    let is_shape_embed = shape_embed_set.contains(cname.as_str());
                    if is_int {
                        // Int: i64 alloca — store i64 frame bits directly.
                        cg.builder
                            .build_store(alloca, bits)
                            .map_err(|e| format!("sm crossing reload store {cname}: {e}"))?;
                    } else if is_bool {
                        // Bool: i1 alloca — truncate the i64 frame slot back to i1 before storing.
                        // The frame slot holds the zero-extended bit; the alloca expects i1.
                        let bit = cg
                            .builder
                            .build_int_truncate(
                                bits,
                                ctx.bool_type(),
                                &format!("{cname}_reload_trunc"),
                            )
                            .map_err(|e| format!("sm crossing reload bool trunc {cname}: {e}"))?;
                        cg.builder
                            .build_store(alloca, bit)
                            .map_err(|e| format!("sm crossing reload bool store {cname}: {e}"))?;
                    } else if is_float {
                        // Float: bitcast i64 frame bits back to f64, store into f64 alloca.
                        let f_val = cg
                            .builder
                            .build_bit_cast(bits, ctx.f64_type(), &format!("{cname}_reload_i_to_f"))
                            .map_err(|e| format!("sm crossing reload f64 bitcast {cname}: {e}"))?;
                        cg.builder
                            .build_store(alloca, f_val)
                            .map_err(|e| format!("sm crossing reload f64 store {cname}: {e}"))?;
                    } else if is_decimal128 {
                        // Decimal128: load 2 slots (lo + hi), reconstruct i128, store.
                        let hi_bits = state_machine::load_local_slot(
                            ctx,
                            &cg.builder,
                            frame_ptr,
                            slot_idx + 1,
                            &format!("{cname}_hi"),
                        )?;
                        let lo_128 = cg
                            .builder
                            .build_int_z_extend(bits, ctx.i128_type(), &format!("{cname}_lo_128"))
                            .map_err(|e| format!("reload i128 lo zext {cname}: {e}"))?;
                        let hi_128 = cg
                            .builder
                            .build_int_z_extend(
                                hi_bits,
                                ctx.i128_type(),
                                &format!("{cname}_hi_128"),
                            )
                            .map_err(|e| format!("reload i128 hi zext {cname}: {e}"))?;
                        let shift_amt = ctx.i128_type().const_int(64, false);
                        let hi_shifted = cg
                            .builder
                            .build_left_shift(hi_128, shift_amt, &format!("{cname}_hi_shift"))
                            .map_err(|e| format!("reload i128 hi shift {cname}: {e}"))?;
                        let i128_val = cg
                            .builder
                            .build_or(lo_128, hi_shifted, &format!("{cname}_i128_or"))
                            .map_err(|e| format!("reload i128 or {cname}: {e}"))?;
                        cg.builder
                            .build_store(alloca, i128_val)
                            .map_err(|e| format!("sm crossing reload i128 store {cname}: {e}"))?;
                    } else if is_errors_capable {
                        let ec_struct_ty =
                            ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false);
                        // Use the pre-created companion alloca (lives in sm_entry — stable).
                        let struct_alloca =
                            *ec_struct_allocas.get(cname.as_str()).ok_or_else(|| {
                                format!("sm reload ec: companion alloca for `{cname}` missing")
                            })?;
                        let f0_ptr = cg
                            .builder
                            .build_struct_gep(
                                ec_struct_ty,
                                struct_alloca,
                                0,
                                &format!("{cname}_r_f0_gep"),
                            )
                            .map_err(|e| format!("reload ec f0 gep {cname}: {e}"))?;
                        let f1_ptr = cg
                            .builder
                            .build_struct_gep(
                                ec_struct_ty,
                                struct_alloca,
                                1,
                                &format!("{cname}_r_f1_gep"),
                            )
                            .map_err(|e| format!("reload ec f1 gep {cname}: {e}"))?;
                        // Store f0 (slot N) — always safe, never a pointer.
                        cg.builder
                            .build_store(f0_ptr, bits)
                            .map_err(|e| format!("reload ec f0 store {cname}: {e}"))?;

                        if ec_number_set.contains(cname.as_str()) {
                            // EC<Number>: slots N+1 and N+2 hold the i128 decimal bits (lo/hi).
                            // Reconstruct the i128 in the per-binding alloca and point f1 at it.
                            // The caller (.or() / .failed()) checks f0 first; on error f1 is
                            // never dereferenced, so a stale f1 value on the error path is safe.
                            let lo_bits = state_machine::load_local_slot(
                                ctx,
                                &cg.builder,
                                frame_ptr,
                                slot_idx + 1,
                                &format!("{cname}_ec_num_lo"),
                            )?;
                            let hi_bits = state_machine::load_local_slot(
                                ctx,
                                &cg.builder,
                                frame_ptr,
                                slot_idx + 2,
                                &format!("{cname}_ec_num_hi"),
                            )?;
                            let lo_128 = cg
                                .builder
                                .build_int_z_extend(
                                    lo_bits,
                                    ctx.i128_type(),
                                    &format!("{cname}_ec_num_lo128"),
                                )
                                .map_err(|e| format!("reload ec num lo zext {cname}: {e}"))?;
                            let hi_128 = cg
                                .builder
                                .build_int_z_extend(
                                    hi_bits,
                                    ctx.i128_type(),
                                    &format!("{cname}_ec_num_hi128"),
                                )
                                .map_err(|e| format!("reload ec num hi zext {cname}: {e}"))?;
                            let shift_amt = ctx.i128_type().const_int(64, false);
                            let hi_shifted = cg
                                .builder
                                .build_left_shift(
                                    hi_128,
                                    shift_amt,
                                    &format!("{cname}_ec_num_hishift"),
                                )
                                .map_err(|e| format!("reload ec num hi shift {cname}: {e}"))?;
                            let i128_val = cg
                                .builder
                                .build_or(lo_128, hi_shifted, &format!("{cname}_ec_num_i128"))
                                .map_err(|e| format!("reload ec num or {cname}: {e}"))?;
                            // Store the reconstructed bits into the per-binding i128 alloca.
                            let i128_alloca =
                                *ec_number_i128_allocas.get(cname.as_str()).ok_or_else(|| {
                                    format!("sm reload ec_num: i128 alloca for `{cname}` missing")
                                })?;
                            cg.builder
                                .build_store(i128_alloca, i128_val)
                                .map_err(|e| format!("reload ec num i128 store {cname}: {e}"))?;
                            // Point f1 at the per-binding i128 alloca (stable across the
                            // current resume call; the alloca is in sm_entry).
                            let f1_as_i64 = cg
                                .builder
                                .build_ptr_to_int(
                                    i128_alloca,
                                    ctx.i64_type(),
                                    &format!("{cname}_ec_num_f1"),
                                )
                                .map_err(|e| format!("reload ec num ptr_to_int {cname}: {e}"))?;
                            cg.builder
                                .build_store(f1_ptr, f1_as_i64)
                                .map_err(|e| format!("reload ec num f1 store {cname}: {e}"))?;
                        } else {
                            // All other ErrorsCapable: slot N+1 holds f1 (ok-word as i64).
                            let hi_bits = state_machine::load_local_slot(
                                ctx,
                                &cg.builder,
                                frame_ptr,
                                slot_idx + 1,
                                &format!("{cname}_ec_hi"),
                            )?;
                            cg.builder
                                .build_store(f1_ptr, hi_bits)
                                .map_err(|e| format!("reload ec f1 store {cname}: {e}"))?;
                        }
                        // Ensure the ptr alloca (the local's alloca) points at the struct.
                        cg.builder
                            .build_store(alloca, struct_alloca)
                            .map_err(|e| format!("reload ec ptr store {cname}: {e}"))?;
                    } else if is_shape_embed {
                        // Shape crossing local: frame-embedded. The ptr alloca is re-wired
                        // to the frame's slot region in Step 1b on every resume call — no
                        // reload needed here. The alloca already holds the correct ptr.
                        // This is a no-op for shape crossing locals.
                    } else {
                        // Pointer alloca (string/array/map/etc.): reconstruct ptr from i64.
                        let ptr_val = cg
                            .builder
                            .build_int_to_ptr(
                                bits,
                                ctx.ptr_type(AddressSpace::default()),
                                &format!("{cname}_reload_i2p"),
                            )
                            .map_err(|e| format!("sm crossing reload int_to_ptr {cname}: {e}"))?;
                        cg.builder
                            .build_store(alloca, ptr_val)
                            .map_err(|e| format!("sm crossing reload ptr store {cname}: {e}"))?;
                    }
                }
                // If the alloca is not registered (local defined only in a conditional branch
                // that was never entered before the first suspension), the frame slot holds zeroed
                // bytes — safe to skip since the local is not yet in scope.
            }
            // Reload any spike CPU-result pairs that remain after the join. The admission
            // gate already declined groups where any post-pair statement assigns a CPU-result
            // bind name, so every name in spike_pairs is stable across the entire post-pair
            // region. The SM crossing loop above skips these names (their frame offsets are
            // SPIKE_RESULT_N_OFFSET, not SM crossing slot indices), so this call provides
            // the exclusive reload for spike-owned results.
            if !spike_pairs.is_empty() {
                spike_reload_cpu_results_from_frame(cg, &spike_pairs, frame_ptr)?;
            }
        }
    } // end reload_crossing guard
    Ok(())
}

/// Lower the body of a state-machine function.
///
/// Emits statements sequentially. When an `Expr::Wait` is encountered:
/// - Emits the sleep-create + first-poll sequence.
/// - If Pending: stores the handle, sets `resume_point = continuation_state_idx`, branches to `pending_block`.
/// - If Ready (zero-ms sleep): continues inline.
/// - Continues emission in the post-wait block.
///
/// All statement types that don't contain `wait` are lowered normally via `lower_stmt`.
///
/// # Flow
///
/// States are allocated linearly: state 0 → stmts up to first wait → state 1 (continuation
/// of first wait, re-entered on wakeup) → stmts up to second wait → state 2 → ... → terminal.
///
/// # Failure modes
///
/// Any `lower_stmt` or `lower_expr` error propagates immediately.
#[allow(clippy::too_many_arguments)]
fn lower_sm_body<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    body: &ynz_ast::nodes::Block,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
) -> Result<(), String> {
    // State 0 already has the builder positioned by the caller. Reload params from frame.
    // Crossing locals are NOT reloaded here — they are defined inline later in this state
    // and stored to the frame at their definition site (no prior stored value to load).
    reload_params_from_frame(cg, frame_ptr, param_names, f, shape_table, false)?;

    // Track which state block index we are currently emitting into.
    let mut current_state: usize = 0;

    // Walk the body, recursing through control flow (if/while/for) so a `wait` at ANY
    // nesting depth becomes a real suspend point. The flat top-level walk this replaces
    // silently no-op'd nested waits (created a timer, discarded it).
    lower_sm_block(
        cg,
        body,
        state_blocks,
        pending_block,
        frame_ptr,
        waker_ctx,
        param_names,
        f,
        shape_table,
        &mut current_state,
    )?;

    // Emit the terminal transition: write typed return value to return_slot@16, return Ready.
    if !is_block_terminated(cg) {
        // For nothing-returning fns: store 0 as the return value (harmless to read).
        state_machine::store_return_value_i64(
            cg.ctx,
            &cg.builder,
            frame_ptr,
            cg.ctx.i64_type().const_int(0, false),
        )?;
        cg.builder
            .build_return(Some(&cg.ctx.i32_type().const_int(0, false)))
            .map_err(|e| format!("sm terminal ret: {e}"))?;
    }

    // Terminate any unreached state blocks. LLVM requires every basic block to have a terminator.
    for &bb in state_blocks.iter() {
        if bb.get_terminator().is_none() {
            cg.builder.position_at_end(bb);
            reload_params_from_frame(cg, frame_ptr, param_names, f, shape_table, false)?;
            state_machine::store_return_value_i64(
                cg.ctx,
                &cg.builder,
                frame_ptr,
                cg.ctx.i64_type().const_int(0, false),
            )?;
            cg.builder
                .build_return(Some(&cg.ctx.i32_type().const_int(0, false)))
                .map_err(|e| format!("sm orphan state ret: {e}"))?;
        }
    }

    Ok(())
}

/// Walk a block of statements in state-machine context, recursing through control flow.
///
/// For each statement: if it (transitively) contains a `wait`, dispatch to
/// `lower_sm_stmt_with_wait` (which handles bare/let waits AND recurses into `if`/loop
/// bodies); otherwise lower it normally. The `current_state` counter threads through the
/// recursion so each `wait` — regardless of nesting depth — consumes the next pre-allocated
/// continuation state, matching the `count_suspension_points` pre-count that sized `state_blocks`.
///
/// Extracted from `lower_sm_body` so control-flow handlers (`Stmt::If`, future loops) can
/// recurse into their branch bodies with the same walk.
#[allow(clippy::too_many_arguments)]
fn lower_sm_block<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    block: &ynz_ast::nodes::Block,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<(), String> {
    if cg.no_auto_parallel {
        // TRUE dumb-sequential baseline: lower every statement in source order with zero
        // consultation of the independence analysis. This is the `--no-auto-parallel` path
        // and the cross-impl consistency oracle — a bug in independence analysis makes
        // default mode diverge from this oracle, turning the consistency gate RED.
        for stmt in &block.stmts {
            if is_block_terminated(cg) {
                break;
            }
            if stmt_contains_wait(stmt) || stmt_contains_suspending_call(stmt, cg.suspend_set) {
                lower_sm_stmt_with_wait(
                    cg,
                    stmt,
                    state_blocks,
                    pending_block,
                    frame_ptr,
                    waker_ctx,
                    param_names,
                    f,
                    shape_table,
                    current_state,
                )?;
            } else {
                lower_stmt(cg, stmt)?;
                anchor_shape_crossing_locals_to_frame_alloca(cg, stmt)?;
                flush_crossing_local_if_needed(cg, stmt, frame_ptr)?;
            }
        }
        return Ok(());
    }

    // Spike CPU-parallel path: if the spike flag is active AND we are at the top-level
    // SM body (sm_scope_depth == 0), scan for a CPU candidate group and emit it via
    // spawn+poll join before falling through to the normal partition for remaining
    // statements.
    //
    // The depth guard is critical for correctness: spike_cpu_candidates (gate 1, in
    // lower_function_with_waits) scans ONLY the top-level function body non-recursively,
    // so it allocates frame bytes and state slots for exactly one CPU group at depth 0.
    // lower_sm_block is called recursively for if/while/for bodies (sm_scope_depth > 0);
    // allowing the spike to fire inside a nested body would spawn handles into frame slots
    // that gate 1 never reserved — producing state-index collisions and OOB frame GEPs.
    if cg.m3d_spike && cg.sm_scope_depth == 0 {
        let cpu_group = spike_extract_cpu_group(&block.stmts, cg.suspend_set, cg.typed);
        if let Some((pre_stmts, cpu_stmts, post_stmts)) = cpu_group {
            // Lower pre-pair statements sequentially before spawning. Any locals they
            // produce (e.g. `let n = 10` before the CPU pair) are allocated to their
            // own allocas here so callee arguments that reference those names resolve
            // to a properly initialized alloca when emit_cpu_group_spawn_join loads them.
            for stmt in &pre_stmts {
                if is_block_terminated(cg) {
                    break;
                }
                if stmt_contains_wait(stmt) || stmt_contains_suspending_call(stmt, cg.suspend_set) {
                    lower_sm_stmt_with_wait(
                        cg,
                        stmt,
                        state_blocks,
                        pending_block,
                        frame_ptr,
                        waker_ctx,
                        param_names,
                        f,
                        shape_table,
                        current_state,
                    )?;
                } else {
                    lower_stmt(cg, stmt)?;
                    flush_crossing_local_if_needed(cg, stmt, frame_ptr)?;
                }
            }

            let spike_crossing = emit_cpu_group_spawn_join(
                cg,
                &cpu_stmts,
                f,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                current_state,
            )?;
            // Store (name, frame_offset) pairs so reload_params_from_frame can call
            // spike_reload at ANY suspension depth (fixes depth-asymmetry: spike_reload
            // now fires inside reload_params_from_frame's reload_crossing:true path, which
            // is reached from lower_sm_stmt_with_wait at all sm_scope_depths — depth-0
            // post_stmts loop AND nested if/while/for bodies).
            cg.m3d_spike_cpu_result_names = spike_crossing.clone();

            // Lower the post-pair statements via the normal sequential path.
            // spike_extract_cpu_group already declined if any post stmt assigns a CPU-result
            // bind name, so no prune is needed here.
            for stmt in &post_stmts {
                if is_block_terminated(cg) {
                    break;
                }
                if stmt_contains_wait(stmt) || stmt_contains_suspending_call(stmt, cg.suspend_set) {
                    // lower_sm_stmt_with_wait calls reload_params_from_frame internally after
                    // the suspension. reload_params_from_frame now calls spike_reload for
                    // any remaining pairs in cg.m3d_spike_cpu_result_names — no explicit
                    // post-suspension spike_reload call needed here.
                    lower_sm_stmt_with_wait(
                        cg,
                        stmt,
                        state_blocks,
                        pending_block,
                        frame_ptr,
                        waker_ctx,
                        param_names,
                        f,
                        shape_table,
                        current_state,
                    )?;
                } else {
                    lower_stmt(cg, stmt)?;
                    flush_crossing_local_if_needed(cg, stmt, frame_ptr)?;
                }
            }
            return Ok(());
        }
    }

    // Auto-parallel path: partition the block into independent groups.
    // The suspend_set is the effective suspend set (including imported fns) passed
    // into this codegen context — `cg.suspend_set` is authoritative (corpse b).
    let groups =
        partition_independent_groups(&block.stmts, cg.suspend_set, cg.sig_table, cg.imported_fns);

    for group in &groups {
        if is_block_terminated(cg) {
            break;
        }
        match group {
            IndependentGroup::Singleton(stmt) => {
                // Existing sequential path unchanged for single statements.
                if stmt_contains_wait(stmt) || stmt_contains_suspending_call(stmt, cg.suspend_set) {
                    lower_sm_stmt_with_wait(
                        cg,
                        stmt,
                        state_blocks,
                        pending_block,
                        frame_ptr,
                        waker_ctx,
                        param_names,
                        f,
                        shape_table,
                        current_state,
                    )?;
                } else {
                    lower_stmt(cg, stmt)?;
                    anchor_shape_crossing_locals_to_frame_alloca(cg, stmt)?;
                    flush_crossing_local_if_needed(cg, stmt, frame_ptr)?;
                }
            }
            IndependentGroup::Parallel(stmts) => {
                // Interleaved inline poll for N ≥ 2 independent suspending statements.
                // Each callee's embedded sub-frame is polled in declaration order;
                // we yield Pending only when ALL are still Pending.
                emit_independent_group_poll(
                    cg,
                    stmts,
                    state_blocks,
                    pending_block,
                    frame_ptr,
                    waker_ctx,
                    param_names,
                    f,
                    shape_table,
                    current_state,
                )?;
                // Flush any crossing locals defined by the parallel group's let-bindings.
                for &stmt in stmts.iter() {
                    flush_crossing_local_if_needed(cg, stmt, frame_ptr)?;
                }
            }
        }
    }
    Ok(())
}

/// No-op stub: shape crossing locals now write directly to the composed frame via
/// the pre-wired ptr alloca (Step 1b in lower_function_with_waits). The definition
/// site memcpy is handled directly in `lower_stmt`'s SM shape special case.
fn anchor_shape_crossing_locals_to_frame_alloca<'ctx>(
    _cg: &mut Cg<'ctx, '_>,
    _stmt: &Stmt,
) -> Result<(), String> {
    Ok(())
}

/// Single per-type flush: read from `alloca` and write to the frame slot(s) for `name`.
///
/// This is the canonical per-type dispatch for every frame flush operation. Both
/// `flush_crossing_local_if_needed` (written crossing locals after a statement) and
/// `flush_for_loop_var` (for-loop element after binding) delegate here so each type is
/// handled in exactly one place. Adding a new type requires updating only this function.
///
/// Type strategies (matching the reload path in `reload_params_from_frame`):
///   int            → i64 alloca, raw 1-slot load/store
///   bool           → i1 alloca, zero-extend to i64 (frame slot is always 8 bytes)
///   float          → f64 alloca, bitcast f64↔i64, 1 slot
///   decimal128     → i128 alloca, split lo/hi, 2 consecutive slots
///   ErrorsCapable  → ptr alloca → companion {i64,i64} struct → 2 frame slots
///   shape-embed    → no-op: ptr alloca points into frame region (Step 1b wiring);
///                    all field writes go directly to frame — bytes are already live
///   pointer types  → ptr alloca, ptr_to_int, 1 slot (string/array/map/dynamic)
fn flush_var_slot_to_frame<'ctx>(
    cg: &Cg<'ctx, '_>,
    name: &str,
    alloca: PointerValue<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    slot_idx: usize,
) -> Result<(), String> {
    let ctx = cg.ctx;
    if cg.sm_crossing_scalar_set.contains(name) {
        // Int: i64 alloca — raw i64 load, 1 slot.
        let bits = cg
            .builder
            .build_load(ctx.i64_type(), alloca, &format!("{name}_flush_load"))
            .map_err(|e| format!("crossing flush load {name}: {e}"))?
            .into_int_value();
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
    } else if cg.sm_crossing_bool_set.contains(name) {
        // Bool: i1 alloca — load i1, zero-extend to i64 for the frame slot.
        // The frame slot is always 8 bytes; storing a raw i1 would write 1 byte
        // into an 8-byte region (UB and SIGSEGV on reload).
        let bit = cg
            .builder
            .build_load(ctx.bool_type(), alloca, &format!("{name}_flush_bool"))
            .map_err(|e| format!("crossing flush bool load {name}: {e}"))?
            .into_int_value();
        let bits = cg
            .builder
            .build_int_z_extend(bit, ctx.i64_type(), &format!("{name}_flush_zext"))
            .map_err(|e| format!("crossing flush bool zext {name}: {e}"))?;
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
    } else if cg.sm_crossing_float_set.contains(name) {
        // Float: f64 alloca — load f64, bitcast to i64, 1 slot.
        let f_val = cg
            .builder
            .build_load(ctx.f64_type(), alloca, &format!("{name}_flush_f64"))
            .map_err(|e| format!("crossing flush f64 {name}: {e}"))?
            .into_float_value();
        let bits = cg
            .builder
            .build_bit_cast(f_val, ctx.i64_type(), &format!("{name}_f_to_i"))
            .map_err(|e| format!("crossing flush f64 bitcast {name}: {e}"))?
            .into_int_value();
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
    } else if cg.sm_crossing_decimal128_set.contains(name) {
        // Decimal128: i128 alloca — split into 2 i64 halves, 2 slots.
        // Frame holds the value directly (not a pointer to stack) so the bits survive
        // suspension even though the resume function's stack frame is freed between calls.
        let i128_val = cg
            .builder
            .build_load(ctx.i128_type(), alloca, &format!("{name}_flush_i128"))
            .map_err(|e| format!("crossing flush i128 {name}: {e}"))?
            .into_int_value();
        let lo = cg
            .builder
            .build_int_truncate(i128_val, ctx.i64_type(), &format!("{name}_lo"))
            .map_err(|e| format!("crossing flush i128 lo {name}: {e}"))?;
        let shift_amt = ctx.i128_type().const_int(64, false);
        let shifted = cg
            .builder
            .build_right_shift(i128_val, shift_amt, false, &format!("{name}_sh"))
            .map_err(|e| format!("crossing flush i128 shift {name}: {e}"))?;
        let hi = cg
            .builder
            .build_int_truncate(shifted, ctx.i64_type(), &format!("{name}_hi"))
            .map_err(|e| format!("crossing flush i128 hi {name}: {e}"))?;
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, lo)?;
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx + 1, hi)?;
    } else if cg.sm_crossing_errors_capable_set.contains(name) {
        let ec_struct_ty = ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false);
        let struct_ptr = cg
            .builder
            .build_load(
                ctx.ptr_type(AddressSpace::default()),
                alloca,
                &format!("{name}_flush_ec_ptr"),
            )
            .map_err(|e| format!("crossing flush ec ptr {name}: {e}"))?
            .into_pointer_value();
        let f0_ptr = cg
            .builder
            .build_struct_gep(ec_struct_ty, struct_ptr, 0, &format!("{name}_f0_gep"))
            .map_err(|e| format!("crossing flush ec f0 gep {name}: {e}"))?;
        let f0 = cg
            .builder
            .build_load(ctx.i64_type(), f0_ptr, &format!("{name}_f0"))
            .map_err(|e| format!("crossing flush ec f0 {name}: {e}"))?
            .into_int_value();
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, f0)?;

        if cg.sm_crossing_ec_number_set.contains(name) {
            // EC<Number>: f1 is a pointer into the per-binding i128 alloca (set at bind
            // time). Read the i128 bits from that alloca and flush as lo/hi in slots N+1
            // and N+2. This stores the value itself (not a pointer), so the bits survive
            // even when the callee's staging slot is reused by a subsequent same-callee call.
            let f1_ptr = cg
                .builder
                .build_struct_gep(ec_struct_ty, struct_ptr, 1, &format!("{name}_f1_gep"))
                .map_err(|e| format!("crossing flush ec_num f1 gep {name}: {e}"))?;
            let f1_bits = cg
                .builder
                .build_load(ctx.i64_type(), f1_ptr, &format!("{name}_f1_bits"))
                .map_err(|e| format!("crossing flush ec_num f1 load {name}: {e}"))?
                .into_int_value();
            // On the error path f0 != 0 and f1 == 0; guard the deref so we don't
            // load from a null pointer. On error we store zero lo/hi (don't care — the
            // error discriminant f0 tells callers to ignore f1).
            let is_ok = cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    f0,
                    ctx.i64_type().const_int(0, false),
                    &format!("{name}_flush_isok"),
                )
                .map_err(|e| format!("crossing flush ec_num isok cmp {name}: {e}"))?;
            let flush_copy_bb = cg.append_block(&format!("{name}_flush_copy"));
            let flush_merge_bb = cg.append_block(&format!("{name}_flush_merge"));
            cg.builder
                .build_conditional_branch(is_ok, flush_copy_bb, flush_merge_bb)
                .map_err(|e| format!("crossing flush ec_num branch {name}: {e}"))?;
            // Success path: deref f1 as a pointer to the i128 decimal bits.
            cg.builder.position_at_end(flush_copy_bb);
            let dec_ptr = cg
                .builder
                .build_int_to_ptr(
                    f1_bits,
                    ctx.ptr_type(inkwell::AddressSpace::default()),
                    &format!("{name}_flush_decptr"),
                )
                .map_err(|e| format!("crossing flush ec_num int_to_ptr {name}: {e}"))?;
            let i128_val = cg
                .builder
                .build_load(ctx.i128_type(), dec_ptr, &format!("{name}_flush_i128"))
                .map_err(|e| format!("crossing flush ec_num load i128 {name}: {e}"))?
                .into_int_value();
            let lo = cg
                .builder
                .build_int_truncate(i128_val, ctx.i64_type(), &format!("{name}_flush_lo"))
                .map_err(|e| format!("crossing flush ec_num lo {name}: {e}"))?;
            let shift_amt = ctx.i128_type().const_int(64, false);
            let shifted = cg
                .builder
                .build_right_shift(i128_val, shift_amt, false, &format!("{name}_flush_sh"))
                .map_err(|e| format!("crossing flush ec_num shift {name}: {e}"))?;
            let hi = cg
                .builder
                .build_int_truncate(shifted, ctx.i64_type(), &format!("{name}_flush_hi"))
                .map_err(|e| format!("crossing flush ec_num hi {name}: {e}"))?;
            state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx + 1, lo)?;
            state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx + 2, hi)?;
            cg.builder
                .build_unconditional_branch(flush_merge_bb)
                .map_err(|e| format!("crossing flush ec_num copy->merge {name}: {e}"))?;
            // Error path (f0 != 0): slots N+1/N+2 retain their zero-init from
            // ynz_alloc_zeroed — no stores are emitted here. Callers check f0 first
            // and never read the ok-value slots on the error path.
            cg.builder.position_at_end(flush_merge_bb);
        } else {
            // All other ErrorsCapable {i64,i64}: f1 is the ok-word (a heap pointer or
            // an int-as-i64). Store f1 directly in slot N+1 — it is always valid.
            let f1_ptr = cg
                .builder
                .build_struct_gep(ec_struct_ty, struct_ptr, 1, &format!("{name}_f1_gep"))
                .map_err(|e| format!("crossing flush ec f1 gep {name}: {e}"))?;
            let f1 = cg
                .builder
                .build_load(ctx.i64_type(), f1_ptr, &format!("{name}_f1"))
                .map_err(|e| format!("crossing flush ec f1 {name}: {e}"))?
                .into_int_value();
            state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx + 1, f1)?;
        }
    } else if cg.sm_crossing_shape_embed_set.contains(name) {
        // Shape crossing local: frame-embedded.
        // The ptr alloca points directly into the composed frame's slot region
        // (wired in Step 1b of lower_function_with_waits). All field writes through
        // the alloca go directly to the frame — no flush needed.
        // No-op: the frame bytes are already live at the correct location.
    } else {
        // Pointer alloca (string/array/map/dynamic/etc.): load the heap pointer, ptr_to_int.
        // These types already live on the heap so the pointer is stable across suspension.
        let ptr_val = cg
            .builder
            .build_load(
                ctx.ptr_type(AddressSpace::default()),
                alloca,
                &format!("{name}_flush_ptr_load"),
            )
            .map_err(|e| format!("crossing flush ptr load {name}: {e}"))?
            .into_pointer_value();
        let bits = cg
            .builder
            .build_ptr_to_int(ptr_val, ctx.i64_type(), &format!("{name}_flush_p2i"))
            .map_err(|e| format!("crossing flush ptr_to_int {name}: {e}"))?;
        state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
    }
    Ok(())
}

/// If `stmt` is a `let` or `assign` targeting a crossing local, store the current
/// alloca value to the local's frame slot.
///
/// Called after every non-wait statement in the SM walk. Crossing locals not yet
/// defined (local declared in a branch not yet entered) are skipped because their
/// alloca is not in `cg.locals`.
fn flush_crossing_local_if_needed<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    stmt: &Stmt,
    frame_ptr: PointerValue<'ctx>,
) -> Result<(), String> {
    let Some(ref crossing_names) = cg.sm_crossing_names.clone() else {
        return Ok(());
    };

    // Collect all crossing-local names written by this statement (recursively for if/while).
    // `Stmt::If` and `Stmt::While` can contain assigns to crossing locals in their bodies.
    let mut written_names: Vec<String> = Vec::new();
    collect_crossing_writes(
        stmt,
        cg.sm_crossing_names.as_deref().unwrap_or(&[]),
        &mut written_names,
    );

    for name in &written_names {
        let name = name.as_str();
        let Some(slot_pos) = crossing_names
            .iter()
            .position(|n| n == name.to_string().as_str())
        else {
            continue;
        };
        let slot_idx = cg.sm_crossing_slot_indices[slot_pos];
        if let Some(&alloca) = cg.locals.get(name) {
            flush_var_slot_to_frame(cg, name, alloca, frame_ptr, slot_idx)?;
        }
    } // end for name in written_names
    Ok(())
}

/// Recursively collect names of crossing locals that are written (defined at their
/// top-level `Stmt::Let` site, or mutated by `Stmt::Assign`/`FieldAssign`/`IndexAssign`)
/// anywhere in `stmt` or any nested block body, filtering against `crossing_names`.
///
/// Recurses into: `If`, `While`, `For`, `Match` (arms + else arm) bodies so that
/// mutations inside any control-flow construct trigger a flush to the frame slot.
///
/// Shadowing: a `Stmt::Let` inside a NESTED scope (if/while/for body) with the same
/// name as a crossing local introduces a new inner binding — it is NOT a write to
/// the outer crossing local's frame slot. Only `Stmt::Assign` in nested scopes
/// counts as a mutation. This prevents Bug 6: `let x` inside an if arm from
/// clobbering the outer x frame slot with the shadow's value.
fn collect_crossing_writes(stmt: &Stmt, crossing_names: &[String], out: &mut Vec<String>) {
    collect_crossing_writes_impl(stmt, crossing_names, out, false);
}

fn collect_crossing_writes_impl(
    stmt: &Stmt,
    crossing_names: &[String],
    out: &mut Vec<String>,
    nested_scope: bool,
) {
    let push_unique = |out: &mut Vec<String>, name: &String| {
        if !out.contains(name) {
            out.push(name.clone());
        }
    };
    match stmt {
        // Definition site of a crossing local (only at the outer scope, not in a nested
        // scope where `let x` creates a shadow binding instead of writing the outer slot).
        Stmt::Let { name, .. } if !nested_scope && crossing_names.iter().any(|n| n == name) => {
            push_unique(out, name);
        }
        // Mutation of a crossing local (re-assignment, valid at any nesting depth).
        Stmt::Assign { target, .. } if crossing_names.iter().any(|n| n == target) => {
            push_unique(out, target);
        }
        // FieldAssign: struct is mutated in place through the crossing-local pointer.
        // The frame slot value (the pointer) is unchanged — no slot update needed.
        // Include so callers that check for "was this local mutated" see it.
        Stmt::FieldAssign { target, .. } => {
            let root = root_ident(target);
            if let Some(r) = root {
                if crossing_names.iter().any(|n| n == r) {
                    push_unique(out, &r.to_string());
                }
            }
        }
        // IndexAssign: same as FieldAssign — pointer is stable.
        Stmt::IndexAssign { receiver, .. } => {
            let root = root_ident(receiver);
            if let Some(r) = root {
                if crossing_names.iter().any(|n| n == r) {
                    push_unique(out, &r.to_string());
                }
            }
        }
        // Recurse into all control-flow bodies. nested_scope=true so inner `let x` is
        // not treated as a write to the outer crossing local (shadowing guard).
        Stmt::If { body, .. } => {
            for s in &body.stmts {
                collect_crossing_writes_impl(s, crossing_names, out, true);
            }
        }
        Stmt::While { body, .. } => {
            for s in &body.stmts {
                collect_crossing_writes_impl(s, crossing_names, out, true);
            }
        }
        Stmt::For { body, .. } => {
            for s in &body.stmts {
                collect_crossing_writes_impl(s, crossing_names, out, true);
            }
        }
        Stmt::Match { arms, else_arm, .. } => {
            for arm in arms {
                for s in &arm.body.stmts {
                    collect_crossing_writes_impl(s, crossing_names, out, true);
                }
            }
            if let Some(else_body) = else_arm {
                for s in &else_body.stmts {
                    collect_crossing_writes_impl(s, crossing_names, out, true);
                }
            }
        }
        _ => {}
    }
}

/// Extract the root identifier from a field-access or index-access expression chain.
///
/// Used by `collect_crossing_writes` to find the base local name being mutated.
fn root_ident(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name.as_str()),
        Expr::FieldAccess { receiver, .. } => root_ident(receiver),
        Expr::IndexAccess { receiver, .. } => root_ident(receiver),
        _ => None,
    }
}

/// Lower a statement that contains at least one `Expr::Wait`.
///
/// Handles:
/// - `Stmt::Expr(Expr::Wait(...))` — a bare `wait sleep(ms)` statement
/// - `Stmt::Let { value: Expr::Wait(...), ... }` — `let x = wait sleep(ms)`
/// - `Stmt::If { cond, body, .. }` whose body contains a wait — recurses into the branch
///   so the nested wait suspends correctly (the branch and its continuation converge at a
///   merge block; Yinz `if` has no else — multi-case uses `Stmt::Match`).
///
/// For each wait encountered, the codegen:
/// 1. Evaluates the sleep duration.
/// 2. Calls `ynz_rt_async_sleep_create(ms)` + first poll (registers waker).
/// 3. If Pending: stores handle, sets `resume_point = continuation_state_idx`, branches to `pending_block`.
/// 4. If Ready: continues inline.
/// 5. Post-wait block: reloads params from frame, continues emitting post-wait code.
#[allow(clippy::too_many_arguments)]
fn lower_sm_stmt_with_wait<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    stmt: &Stmt,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<(), String> {
    match stmt {
        // `wait sleep(ms)` as a bare expression statement.
        Stmt::Expr(Expr::Wait(inner, _)) if is_sleep_call(inner) => {
            emit_wait_point(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
        }

        // `wait suspendingCallee(args)` — explicit wait on a user SM call.
        Stmt::Expr(Expr::Wait(inner, _)) if is_direct_suspending_call(inner, cg.suspend_set) => {
            let return_val = emit_suspending_call_inline_poll(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            let _ = return_val; // bare statement — discard return value
        }

        // Direct call to suspending callee without explicit `wait` (transitive case).
        // No explicit `wait` token — the inference model drives inline-poll-and-yield.
        Stmt::Expr(inner) if is_direct_suspending_call(inner, cg.suspend_set) => {
            let return_val = emit_suspending_call_inline_poll(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            let _ = return_val;
        }

        // `wait sleep(ms)` as a bare expression statement (non-ident call — fallback).
        Stmt::Expr(Expr::Wait(inner, _)) => {
            emit_wait_point(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
        }

        // `let name = wait sleep(ms)` — sleep is nothing-returning; bind zero.
        Stmt::Let {
            name,
            value: Expr::Wait(inner, _),
            ..
        } if is_sleep_call(inner) => {
            emit_wait_point(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            // sleep returns nothing; bind zero to name.
            let alloca = cg
                .builder
                .build_alloca(cg.i64(), &format!("{name}_alloca"))
                .map_err(|e| format!("wait let alloca: {e}"))?;
            cg.builder
                .build_store(alloca, cg.i64().const_int(0, false))
                .map_err(|e| format!("wait let store: {e}"))?;
            cg.locals.insert(name.clone(), alloca);
        }

        // `let name = wait suspendingCallee(args)` — bind the return value.
        Stmt::Let {
            name,
            value: Expr::Wait(inner, _),
            ..
        } if is_direct_suspending_call(inner, cg.suspend_set) => {
            let callee_name_str = callee_name_from_call_expr(inner).unwrap_or("");
            let return_val = emit_suspending_call_inline_poll(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            // Bind the return value to `name`, flushing to frame slot if it is a
            // crossing local (needed before any subsequent suspension point).
            let alloca =
                bind_sm_result_and_flush(cg, name, return_val, frame_ptr, callee_name_str)?;
            cg.locals.insert(name.clone(), alloca);
            // EC crossing locals must be tracked in errors_capable_locals so that
            // lower_expr's ident handler can extract the success value (or propagate the
            // error) when `name` is used AFTER a subsequent suspension. Without this,
            // `return x` where x is an EC crossing local sends the companion-struct
            // pointer as the ok-word instead of the actual success bits.
            if cg.sm_crossing_errors_capable_set.contains(name.as_str()) {
                cg.errors_capable_locals.insert(name.clone());
            }
        }

        // `let name = suspendingCallee(args)` — no explicit `wait`, bind the return value.
        Stmt::Let {
            name, value: inner, ..
        } if is_direct_suspending_call(inner, cg.suspend_set) => {
            let callee_name_str = callee_name_from_call_expr(inner).unwrap_or("");
            let return_val = emit_suspending_call_inline_poll(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            let alloca =
                bind_sm_result_and_flush(cg, name, return_val, frame_ptr, callee_name_str)?;
            cg.locals.insert(name.clone(), alloca);
            // Same EC-crossing-local registration as the wait-let arm above.
            if cg.sm_crossing_errors_capable_set.contains(name.as_str()) {
                cg.errors_capable_locals.insert(name.clone());
            }
        }

        // `let name = wait non_sleep_async_call(...)` — non-suspension wait; lower normally.
        Stmt::Let {
            name: _,
            value: Expr::Wait(..),
            ..
        } => {
            lower_stmt(cg, stmt)?;
        }

        // `if (cond) { ...wait/suspending_call... }` — recurse the SM walker into the branch.
        Stmt::If { cond, body, .. } => {
            let cond_val = lower_expr(cg, cond)?.into_int_value();
            let then_bb = cg.append_block("sm_if_then");
            let merge_bb = cg.append_block("sm_if_merge");
            cg.builder
                .build_conditional_branch(cond_val, then_bb, merge_bb)
                .map_err(|e| format!("sm if branch: {e}"))?;

            cg.builder.position_at_end(then_bb);
            // Save locals snapshot + bump scope depth so shadow bindings inside the
            // wait-bearing if body don't clobber the outer crossing local's sm_entry alloca.
            // This mirrors the same guard in lower_stmt's Stmt::If arm.
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            lower_sm_block(
                cg,
                body,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as lower_stmt's
            // Stmt::If arm. Crossing sm_entry allocas stay active; shadow bindings are unwound.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
            if !is_block_terminated(cg) {
                cg.builder
                    .build_unconditional_branch(merge_bb)
                    .map_err(|e| format!("sm if then->merge: {e}"))?;
            }
            cg.builder.position_at_end(merge_bb);
        }

        // `return suspendingCallee(args)` from a SM function — inline-poll + store return + Ready.
        Stmt::Return {
            value: Some(inner), ..
        } if is_direct_suspending_call(inner, cg.suspend_set) => {
            let return_val = emit_suspending_call_inline_poll(
                cg,
                inner,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            // Store the return value in our own return slot and emit Ready.
            // When the callee is errors-capable, emit_suspending_call_inline_poll returns a
            // StructValue ({i64, i64}). Both words must reach the return slot — extracting a
            // single i64 via to_i64_bits would silently drop the error_ptr (field0), causing
            // the wrapper to reconstruct {0, success} and treat an error as success.
            if let Some(own_frame) = cg.sm_frame_ptr {
                match return_val {
                    inkwell::values::BasicValueEnum::StructValue(sv) => {
                        // errors-capable callee: extract error_ptr (field0) + success (field1).
                        let err_i64 = cg
                            .builder
                            .build_extract_value(sv, 0, "sm_fwd_err")
                            .map_err(|e| format!("sm fwd err extract: {e}"))?
                            .into_int_value();
                        let ok_i64 = cg
                            .builder
                            .build_extract_value(sv, 1, "sm_fwd_ok")
                            .map_err(|e| format!("sm fwd ok extract: {e}"))?
                            .into_int_value();
                        state_machine::store_return_value_errors(
                            cg.ctx,
                            &cg.builder,
                            own_frame,
                            err_i64,
                            ok_i64,
                        )?;
                    }
                    // Float callee: load_sm_return_value_typed returns FloatValue.
                    inkwell::values::BasicValueEnum::FloatValue(fv) => {
                        state_machine::store_return_value_f64(cg.ctx, &cg.builder, own_frame, fv)?;
                    }
                    // Decimal128 callee: load_sm_return_value_typed returns i128 IntValue.
                    inkwell::values::BasicValueEnum::IntValue(iv)
                        if iv.get_type() == cg.ctx.i128_type() =>
                    {
                        state_machine::store_return_value_i128(cg.ctx, &cg.builder, own_frame, iv)?;
                    }
                    _ => {
                        // Non-errors callee: store the i64/ptr return value.
                        let val_ty = cg.expr_type(inner);
                        let bits = cg
                            .to_i64_bits(return_val, &val_ty)
                            .unwrap_or_else(|_| cg.i64().const_int(0, false));
                        state_machine::store_return_value_i64(
                            cg.ctx,
                            &cg.builder,
                            own_frame,
                            bits,
                        )?;
                    }
                }
            }
            cg.builder
                .build_return(Some(&cg.ctx.i32_type().const_int(0, false)))
                .map_err(|e| format!("sm return after inline-poll: {e}"))?;
        }

        // `while (cond) { ...wait/suspending_call... }` — emit a frame-backed loop.
        //
        // Control flow mirrors the non-SM lower_stmt_while but walks the body with
        // lower_sm_block so each wait inside consumes a pre-allocated continuation
        // state. Crossing loop-carried locals (declared before the while or accumulated
        // inside it) are frame-backed by P1's slot machinery — no separate alloc.
        //
        // Resume behaviour: after a suspension inside the body, the continuation state
        // reloads params/crossing-locals and branches to post_wait_bb (inside the body).
        // The rest of the body statements run, then execution falls through to the branch
        // back to while_header_bb, which re-checks the condition. If still true, the body
        // runs again (potentially suspending again). Each iteration is therefore a distinct
        // poll cycle: sequential by construction (the runtime never resumes the same task
        // twice concurrently), satisfying the "loop iterations sequential by default" design.
        Stmt::While { cond, body, .. } => {
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;

            let while_header_bb = cg.append_block("sm_while_header");
            let while_body_bb = cg.append_block("sm_while_body");
            let while_exit_bb = cg.append_block("sm_while_exit");

            // Branch from current position (whatever state block we are in) to header.
            cg.builder
                .build_unconditional_branch(while_header_bb)
                .map_err(|e| format!("sm while entry branch: {e}"))?;

            // Header: evaluate condition; branch to body or exit.
            cg.builder.position_at_end(while_header_bb);
            let cond_val = lower_expr(cg, cond)?.into_int_value();
            cg.builder
                .build_conditional_branch(cond_val, while_body_bb, while_exit_bb)
                .map_err(|e| format!("sm while cond branch: {e}"))?;

            // Body: lower via SM block walker so nested waits consume continuation states.
            cg.builder.position_at_end(while_body_bb);
            lower_sm_block(
                cg,
                body,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            if !is_block_terminated(cg) {
                // Body didn't contain a return; flush preempt hook then loop back to header.
                emit_loop_preempt(cg)?;
                cg.builder
                    .build_unconditional_branch(while_header_bb)
                    .map_err(|e| format!("sm while body->header branch: {e}"))?;
            }

            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as lower_stmt's
            // Stmt::While arm. sm_entry allocas for crossing locals stay active; shadow
            // bindings introduced inside the body are unwound so the outer scope is clean.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }

            cg.builder.position_at_end(while_exit_bb);
        }

        // `for (var in iter) { ...wait/suspending_call... }` — frame-backed for-loop.
        //
        // Control flow mirrors the non-SM lower_stmt_for but walks the body with
        // lower_sm_block so each wait inside consumes a pre-allocated continuation state.
        // The iteration index (for array/range) or entry cursor (for map) is stored in a
        // stack alloca, which the SM resume-switch lands inside the body on each resume.
        // Loop-carried outer locals are frame-backed by P1's slot machinery — no extra alloc.
        //
        // Resume behaviour: after a suspension inside the body, the continuation state
        // reloads params/crossing-locals and branches to post_wait_bb (inside the body).
        // The remaining body statements run, then execution increments the index and falls
        // through to the back-edge branch that re-checks the condition. Each iteration is
        // therefore a distinct poll cycle: sequential by construction (the runtime never
        // resumes the same task twice concurrently), satisfying "loop iterations sequential
        // by default" in design/concurrency.md.
        //
        // The alloca for the loop index is placed in the current (state) block, not sm_entry,
        // because it is re-initialized at the start of the for-loop, not carried across
        // resumes at the function level. Crossing OUTER locals (declared before the for-loop
        // and read after it, or read inside the body) DO get sm_entry allocas via P1.
        Stmt::For {
            var, iter, body, ..
        } => {
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            lower_sm_for(
                cg,
                var,
                iter,
                body,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as Stmt::While arm.
            // sm_entry allocas for crossing locals stay active; shadow bindings introduced
            // inside the for body are unwound so the outer scope is clean.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
        }

        // `match (scrutinee) { pat => { ...wait/suspending_call... } }` — SM match.
        //
        // Control flow mirrors the non-SM lower_stmt_match but walks each arm body that
        // contains a suspension with lower_sm_block. Arms without a suspension use the
        // regular lower_stmt path. The scrutinee is evaluated once before the arm dispatch;
        // the matching arm's body is then walked by the SM block walker.
        //
        // Resume behaviour: after a suspension inside an arm body, the continuation state
        // reloads params/crossing-locals and branches to post_wait_bb (inside that arm body).
        // The remaining arm-body statements run, then execution falls through to the merge block.
        //
        // Note: `match` in Yinz is the multi-case form of `if` (see parser.rs). Arms without
        // a suspension lower normally; only arms containing a wait go through lower_sm_block.
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            lower_sm_match(
                cg,
                scrutinee,
                arms,
                else_arm.as_ref(),
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as Stmt::If arm.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
        }

        _ => {
            if stmt_contains_wait(stmt) || stmt_contains_suspending_call(stmt, cg.suspend_set) {
                panic!(
                    "BUG: SM codegen reached a wait-bearing statement with no handler. \
                     This is a compiler bug — all suspendable statement forms should have \
                     an explicit arm above. Statement: {stmt:?}"
                );
            }
            lower_stmt(cg, stmt)?;
        }
    }
    Ok(())
}

/// Flush a for-loop variable to its crossing-local frame slot after binding.
///
/// The for-loop variable is bound by the iteration mechanism (not via `Stmt::Let`),
/// so `flush_crossing_local_if_needed` does not see it. We flush manually so the
/// continuation state's reload path can restore the value after a suspension.
///
/// Delegates to `flush_var_slot_to_frame` for the per-type conversion so both flush
/// paths share a single dispatch — adding a new type requires updating only that helper.
fn flush_for_loop_var<'ctx>(
    cg: &Cg<'ctx, '_>,
    var: &str,
    var_slot: PointerValue<'ctx>,
    frame_ptr: PointerValue<'ctx>,
) -> Result<(), String> {
    // Find the slot index for `var` in the crossing-names/slot-indices parallel arrays.
    let Some(pos) = cg
        .sm_crossing_names
        .as_deref()
        .and_then(|names| names.iter().position(|n| n == var))
    else {
        // `var` is not a crossing local — no frame slot to flush.
        return Ok(());
    };
    let slot_idx = cg.sm_crossing_slot_indices[pos];
    flush_var_slot_to_frame(cg, var, var_slot, frame_ptr, slot_idx)
}

/// Emit state-machine codegen for `for (var in iter) { ...body with waits... }`.
///
/// The iteration index is a synthetic crossing local (`__ynz_for_idx_N`) whose frame slot
/// was pre-allocated by `crossing_local_names`. This lets the index survive suspension the
/// same way user-declared crossing locals do — no extra alloc, no separate mechanism.
///
/// Control flow mirrors `lower_stmt_for` but uses `lower_sm_block` for the body so each
/// wait inside allocates a continuation state. After the body, the index is incremented
/// and explicitly flushed to its frame slot before the back-edge branch.
///
/// All for-loop variants (array, map, range, fixed, string, shape-iter) are handled.
/// Non-SM iteration (no wait in body) falls through to `lower_stmt_for` via the non-SM
/// path in `lower_sm_block`.
#[allow(clippy::too_many_arguments)]
fn lower_sm_for<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    var: &str,
    iter: &Expr,
    body: &ynz_ast::nodes::Block,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<(), String> {
    // Claim the synthetic index slot for this for-loop.
    let loop_idx = cg.sm_for_loop_counter;
    cg.sm_for_loop_counter += 1;
    let syn_name = format!("__ynz_for_idx_{loop_idx}");

    // Find the frame slot index for the synthetic crossing local.
    // `crossing_local_names` pre-allocated a slot for every suspending for-loop under
    // the same counter — the slot must exist or the frame-layout invariant is violated.
    let slot_pos = cg
        .sm_crossing_names
        .as_deref()
        .and_then(|names| names.iter().position(|n| n == &syn_name))
        .ok_or_else(|| {
            format!(
                "SM for-loop: synthetic crossing local `{syn_name}` not found in frame. \
                 This is a compiler bug — crossing_local_names must pre-allocate this slot."
            )
        })?;
    let slot_idx = cg.sm_crossing_slot_indices[slot_pos];

    let iter_ty = cg.expr_type(iter);

    // Create an sm_entry alloca for the working-copy of the index. The frame slot
    // (idx_slot) holds the durable value across resume calls; the alloca holds the
    // in-progress value within one resume call.
    let idx_alloca = cg.alloca_in_entry_llvm(cg.i64(), &syn_name)?;

    // ── Range-based: `for (i in range(start, end))` ──────────────────────────────
    // The synthetic slot holds the loop counter (index). The range end is re-evaluated
    // at the header on each iteration — range expressions are pure (no side effects) and
    // always produce the same end value. This avoids the need for a second frame slot.
    if matches!(iter_ty, Type::Range { .. }) {
        let (start_val, _) = extract_range_bounds(cg, iter)?;

        // Init index and flush to frame slot.
        cg.builder
            .build_store(idx_alloca, start_val)
            .map_err(|e| format!("sm range idx init: {e}"))?;
        state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, start_val)
            .map_err(|e| format!("sm range flush idx: {e}"))?;

        let header_bb = cg.append_block("sm_for_r_header");
        let body_bb = cg.append_block("sm_for_r_body");
        let exit_bb = cg.append_block("sm_for_r_exit");
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("sm for range entry: {e}"))?;

        // Header: reload idx from frame slot; re-evaluate end; check idx < end.
        // Re-evaluating end at the header is correct because range bounds are pure
        // expressions (literals or reads of frame-backed crossing locals) — same value
        // on every iteration.
        cg.builder.position_at_end(header_bb);
        let idx_cur =
            state_machine::load_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, "sm_r_idx")?;
        cg.builder
            .build_store(idx_alloca, idx_cur)
            .map_err(|e| format!("{e}"))?;
        let (_, end_cur_val) = extract_range_bounds(cg, iter)?;
        let in_range = cg
            .builder
            .build_int_compare(IntPredicate::SLT, idx_cur, end_cur_val, "sm_r_cond")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(in_range, body_bb, exit_bb)
            .map_err(|e| format!("sm for range cond: {e}"))?;

        // Body: bind loop variable (= index for range loops), run SM body.
        // Use existing crossing-local alloca if present (avoids creating a duplicate that
        // the frame reload would miss on subsequent resumes).
        cg.builder.position_at_end(body_bb);
        let var_slot = if let Some(&existing) = cg.locals.get(var) {
            existing
        } else {
            let s = cg.alloca_in_entry(&Type::Int, var)?;
            cg.locals.insert(var.to_string(), s);
            s
        };
        cg.builder
            .build_store(var_slot, idx_cur)
            .map_err(|e| format!("sm range var bind: {e}"))?;
        // Flush the loop variable to its frame slot so the continuation state can reload
        // it after a suspension. The for-loop binding is not via Stmt::Let, so
        // flush_crossing_local_if_needed does not see it — flush manually here.
        flush_for_loop_var(cg, var, var_slot, frame_ptr)?;

        lower_sm_block(
            cg,
            body,
            state_blocks,
            pending_block,
            frame_ptr,
            waker_ctx,
            param_names,
            f,
            shape_table,
            current_state,
        )?;

        if !is_block_terminated(cg) {
            let idx_after = state_machine::load_local_slot(
                cg.ctx,
                &cg.builder,
                frame_ptr,
                slot_idx,
                "sm_r_idx_after",
            )?;
            let one = cg.i64().const_int(1, false);
            let idx_next = cg
                .builder
                .build_int_add(idx_after, one, "sm_r_idx_next")
                .map_err(|e| format!("{e}"))?;
            state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, idx_next)
                .map_err(|e| format!("sm range flush next: {e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(header_bb)
                .map_err(|e| format!("sm for range back-edge: {e}"))?;
        }

        cg.builder.position_at_end(exit_bb);
        cg.locals.remove(var);
        return Ok(());
    }

    // ── Array-based: `for (x in arr)` where arr: array<T> ────────────────────────
    // The array pointer and count are re-evaluated at the header on each iteration.
    // `lower_expr(iter)` for a variable identifier loads from `cg.locals["arr"]`, which
    // is a crossing local alloca whose value is reloaded from the frame on each resume.
    // The count is stable (arrays don't grow during suspension). No extra frame slots needed.
    if let Type::BuiltinArray { elem } = &iter_ty {
        let elem = elem.as_ref().clone();

        // Init index to 0 and flush to frame slot.
        let zero = cg.i64().const_zero();
        cg.builder
            .build_store(idx_alloca, zero)
            .map_err(|e| format!("{e}"))?;
        state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, zero)
            .map_err(|e| format!("sm array flush zero: {e}"))?;

        let header_bb = cg.append_block("sm_for_a_header");
        let body_bb = cg.append_block("sm_for_a_body");
        let exit_bb = cg.append_block("sm_for_a_exit");
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(header_bb);
        let idx_cur =
            state_machine::load_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, "sm_a_idx")?;
        cg.builder
            .build_store(idx_alloca, idx_cur)
            .map_err(|e| format!("{e}"))?;
        // Re-evaluate array pointer and count at header — reloads from frame-backed local.
        let arr_ptr_h = lower_expr(cg, iter)?.into_pointer_value();
        let cnt_cur = cg
            .builder
            .build_call(cg.rt.ynz_array_count, &[arr_ptr_h.into()], "sm_a_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("array_count void")?
            .into_int_value();
        let in_range = cg
            .builder
            .build_int_compare(IntPredicate::SLT, idx_cur, cnt_cur, "sm_a_cond")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(in_range, body_bb, exit_bb)
            .map_err(|e| format!("sm for array cond: {e}"))?;

        cg.builder.position_at_end(body_bb);
        let arr_cur = lower_expr(cg, iter)?.into_pointer_value();
        let out = cg
            .builder
            .build_alloca(cg.maybe_type(), "sm_a_get")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_call(
                cg.rt.ynz_array_get,
                &[arr_cur.into(), idx_cur.into(), out.into()],
                "sm_a_get_call",
            )
            .map_err(|e| format!("{e}"))?;
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), out, 1, "sm_a_val")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "sm_a_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let elem_val = cg.i64_bits_to(bits, &elem)?;
        // Use the existing crossing-local alloca if `var` is already frame-backed; otherwise
        // create a new sm_entry alloca. Creating a duplicate alloca would make cg.locals[var]
        // point to a different alloca than the one the frame reload writes into, causing the
        // reload to update the stale alloca while the body reads from the new (uninitialized) one.
        let var_slot = if let Some(&existing) = cg.locals.get(var) {
            existing
        } else {
            let s = cg.alloca_in_entry(&elem, var)?;
            cg.locals.insert(var.to_string(), s);
            s
        };
        // Shape-embed loop var: `var_slot` is the ptr alloca pre-wired to the frame region
        // (Step 1b in lower_function_with_waits). `elem_val` is a pointer to the heap element
        // (from ynz_array_get via i64_bits_to). Copy the element bytes into the frame region
        // directly — this is the frame-persistent copy. A regular store would overwrite the
        // frame region pointer in the ptr alloca with the heap element pointer, breaking the
        // frame-embed invariant and causing every field read after suspension to GEP into the
        // heap element (whose lifetime is not controlled by the task frame).
        if cg.sm_crossing_shape_embed_set.contains(var) {
            if let Some(shape_name) = cg.sm_crossing_shape_names.get(var).cloned() {
                let struct_ty = cg.shape_types.get(&shape_name).ok_or_else(|| {
                    format!("sm for array shape-embed: LLVM type for `{shape_name}` missing")
                })?;
                let size_val = struct_ty.size_of().ok_or_else(|| {
                    format!("sm for array shape-embed: size_of `{shape_name}` unavailable")
                })?;
                let size_i64 = cg
                    .builder
                    .build_int_z_extend(size_val, cg.ctx.i64_type(), &format!("{var}_arr_shape_sz"))
                    .map_err(|e| format!("sm for array shape size extend {var}: {e}"))?;
                // `elem_val` is the heap element pointer (int_to_ptr from ynz_array_get bits).
                let src_ptr = elem_val.into_pointer_value();
                // Load the frame region ptr from the ptr alloca (pre-wired in Step 1b).
                let dest_ptr = cg
                    .builder
                    .build_load(
                        cg.ctx.ptr_type(inkwell::AddressSpace::default()),
                        var_slot,
                        &format!("{var}_arr_shape_frame_ptr"),
                    )
                    .map_err(|e| format!("sm for array shape frame ptr load {var}: {e}"))?
                    .into_pointer_value();
                cg.builder
                    .build_memcpy(dest_ptr, 1, src_ptr, 1, size_i64)
                    .map_err(|e| format!("sm for array shape memcpy {var}: {e}"))?;
                // No flush needed — frame bytes are already live at the correct location.
            }
        } else {
            store(cg, elem_val, &elem, var_slot)?;
            flush_for_loop_var(cg, var, var_slot, frame_ptr)?;
        }

        lower_sm_block(
            cg,
            body,
            state_blocks,
            pending_block,
            frame_ptr,
            waker_ctx,
            param_names,
            f,
            shape_table,
            current_state,
        )?;

        if !is_block_terminated(cg) {
            let idx_after = state_machine::load_local_slot(
                cg.ctx,
                &cg.builder,
                frame_ptr,
                slot_idx,
                "sm_a_idx_after",
            )?;
            let one = cg.i64().const_int(1, false);
            let idx_next = cg
                .builder
                .build_int_add(idx_after, one, "sm_a_idx_next")
                .map_err(|e| format!("{e}"))?;
            state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, idx_next)
                .map_err(|e| format!("sm array flush next: {e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(header_bb)
                .map_err(|e| format!("sm for array back: {e}"))?;
        }

        cg.builder.position_at_end(exit_bb);
        cg.locals.remove(var);
        return Ok(());
    }

    // ── Fixed array: `for (x in arr)` where arr: fixed<T> ────────────────────────
    // Array pointer and compile-time size are re-evaluated at the header. Size is a
    // compile-time constant; pointer comes from a frame-backed crossing local.
    if let Type::BuiltinFixed { elem, size } = &iter_ty {
        let elem = elem.as_ref().clone();
        let n = match size {
            Some(n) => *n as u64,
            None => return Err("SM for-loop: fixed array with unknown size".to_string()),
        };
        let size_val = cg.i64().const_int(n, false);

        let zero = cg.i64().const_zero();
        cg.builder
            .build_store(idx_alloca, zero)
            .map_err(|e| format!("{e}"))?;
        state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, zero)
            .map_err(|e| format!("sm fixed flush zero: {e}"))?;

        let header_bb = cg.append_block("sm_for_ff_header");
        let body_bb = cg.append_block("sm_for_ff_body");
        let exit_bb = cg.append_block("sm_for_ff_exit");
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(header_bb);
        let idx_cur =
            state_machine::load_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, "sm_ff_idx")?;
        cg.builder
            .build_store(idx_alloca, idx_cur)
            .map_err(|e| format!("{e}"))?;
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, idx_cur, size_val, "sm_ff_cond")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, exit_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let arr_cur = lower_expr(cg, iter)?.into_pointer_value();
        let gep = unsafe {
            cg.builder
                .build_gep(cg.i64(), arr_cur, &[idx_cur], "sm_ff_gep")
                .map_err(|e| format!("{e}"))?
        };
        let bits = cg
            .builder
            .build_load(cg.i64(), gep, "sm_ff_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let elem_val = cg.i64_bits_to(bits, &elem)?;
        let var_slot = if let Some(&existing) = cg.locals.get(var) {
            existing
        } else {
            let s = cg.alloca_in_entry(&elem, var)?;
            cg.locals.insert(var.to_string(), s);
            s
        };
        store(cg, elem_val, &elem, var_slot)?;
        flush_for_loop_var(cg, var, var_slot, frame_ptr)?;

        lower_sm_block(
            cg,
            body,
            state_blocks,
            pending_block,
            frame_ptr,
            waker_ctx,
            param_names,
            f,
            shape_table,
            current_state,
        )?;

        if !is_block_terminated(cg) {
            let idx_after = state_machine::load_local_slot(
                cg.ctx,
                &cg.builder,
                frame_ptr,
                slot_idx,
                "sm_ff_idx_after",
            )?;
            let one = cg.i64().const_int(1, false);
            let idx_next = cg
                .builder
                .build_int_add(idx_after, one, "sm_ff_idx_next")
                .map_err(|e| format!("{e}"))?;
            state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, idx_next)
                .map_err(|e| format!("sm fixed flush next: {e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(header_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(exit_bb);
        cg.locals.remove(var);
        return Ok(());
    }

    // ── Map-based: `for ((k, v) in m)` where m: map<K,V> ────────────────────────
    // Map pointer and count are re-evaluated at the header on each iteration.
    // The map pointer comes from a frame-backed crossing local (always fresh on resume).
    if let Type::BuiltinMap { key, val } = &iter_ty {
        let key_ty = key.as_ref().clone();
        let _val_ty = val.as_ref().clone();

        let zero = cg.i64().const_zero();
        cg.builder
            .build_store(idx_alloca, zero)
            .map_err(|e| format!("{e}"))?;
        state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, zero)
            .map_err(|e| format!("sm map flush zero: {e}"))?;

        let header_bb = cg.append_block("sm_mfor_header");
        let body_bb = cg.append_block("sm_mfor_body");
        let exit_bb = cg.append_block("sm_mfor_exit");
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(header_bb);
        let idx_cur =
            state_machine::load_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, "sm_mf_idx")?;
        cg.builder
            .build_store(idx_alloca, idx_cur)
            .map_err(|e| format!("{e}"))?;
        // Re-evaluate map pointer and count at header — reloads from frame-backed local.
        let map_ptr_h = lower_expr(cg, iter)?.into_pointer_value();
        let cnt_cur = cg
            .builder
            .build_call(cg.rt.ynz_map_count, &[map_ptr_h.into()], "sm_mf_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("map_count void")?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, idx_cur, cnt_cur, "sm_mf_cond")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, exit_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let map_cur = lower_expr(cg, iter)?.into_pointer_value();
        let entry_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into()], false);
        // Entry slot uses alloca_in_entry_llvm since MapEntry has an LLVM struct type.
        let entry_slot = cg.alloca_in_entry_llvm(entry_ty, var)?;
        cg.locals.insert(var.to_string(), entry_slot);

        let triple_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into(), cg.i64().into()], false);
        let triple_slot = cg
            .builder
            .build_alloca(triple_ty, "sm_mf_triple")
            .map_err(|e| format!("{e}"))?;
        if key_is_string(&key_ty) {
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get_str,
                    &[map_cur.into(), idx_cur.into(), triple_slot.into()],
                    "sm_mf_iter_s",
                )
                .map_err(|e| format!("{e}"))?;
        } else {
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get,
                    &[map_cur.into(), idx_cur.into(), triple_slot.into()],
                    "sm_mf_iter",
                )
                .map_err(|e| format!("{e}"))?;
        }
        let k_src = cg
            .builder
            .build_struct_gep(triple_ty, triple_slot, 1, "sm_mf_ks")
            .map_err(|e| format!("{e}"))?;
        let v_src = cg
            .builder
            .build_struct_gep(triple_ty, triple_slot, 2, "sm_mf_vs")
            .map_err(|e| format!("{e}"))?;
        let k_dst = cg
            .builder
            .build_struct_gep(entry_ty, entry_slot, 0, "sm_mf_kd")
            .map_err(|e| format!("{e}"))?;
        let v_dst = cg
            .builder
            .build_struct_gep(entry_ty, entry_slot, 1, "sm_mf_vd")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(
                k_dst,
                cg.builder
                    .build_load(cg.i64(), k_src, "sm_mf_kv")
                    .map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(
                v_dst,
                cg.builder
                    .build_load(cg.i64(), v_src, "sm_mf_vv")
                    .map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?;
        // No loop-var flush for map entries: the entry struct has two i64 fields and
        // uses a struct alloca (not a scalar slot). The body must not read the loop variable
        // after a `wait` inside the body — crossing-local analysis handles this correctly
        // because map entry destructure bindings are not yet tracked as scalar crossing locals.

        lower_sm_block(
            cg,
            body,
            state_blocks,
            pending_block,
            frame_ptr,
            waker_ctx,
            param_names,
            f,
            shape_table,
            current_state,
        )?;

        if !is_block_terminated(cg) {
            let idx_after = state_machine::load_local_slot(
                cg.ctx,
                &cg.builder,
                frame_ptr,
                slot_idx,
                "sm_mf_idx_after",
            )?;
            let one = cg.i64().const_int(1, false);
            let idx_next = cg
                .builder
                .build_int_add(idx_after, one, "sm_mf_idx_next")
                .map_err(|e| format!("{e}"))?;
            state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, idx_next)
                .map_err(|e| format!("sm map flush next: {e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(header_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(exit_bb);
        cg.locals.remove(var);
        return Ok(());
    }

    // Fallback: string iteration, shape iteration, stored-range variables.
    // These forms with a suspension body are unsupported — typeck should have caught them
    // via the WaitInsideLoop guard before this point. Fall back to non-SM lowering
    // (which will panic if it hits a wait, surfacing the bug cleanly).
    lower_stmt_for(cg, var, iter, body)
}

/// Emit state-machine codegen for a multi-case `if` (`Stmt::Match`) with a `wait`
/// in one or more arms.
///
/// The scrutinee is evaluated once before arm dispatch. Each arm whose body contains
/// a suspension is walked by `lower_sm_block` so its waits get continuation states.
/// Arms without a suspension use the regular `lower_stmt` path. After each arm (SM or
/// non-SM), control merges to `match_merge_bb`.
///
/// Resume behaviour: after a suspension inside an arm body, the continuation state
/// reloads params/crossing-locals and resumes inside that arm. Execution reaches the
/// merge block after the remaining arm stmts complete.
#[allow(clippy::too_many_arguments)]
fn lower_sm_match<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    scrutinee: &Expr,
    arms: &[ynz_ast::nodes::MatchArm],
    else_arm: Option<&ynz_ast::nodes::Block>,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<(), String> {
    let scrutinee_ty = cg.expr_type(scrutinee);
    let scrutinee_val = lower_expr(cg, scrutinee)?;

    let merge_bb = cg.append_block("sm_match_merge");
    let final_fallthrough_bb = if else_arm.is_some() {
        cg.append_block("sm_match_else")
    } else {
        merge_bb
    };

    for (i, arm) in arms.iter().enumerate() {
        let arm_body_bb = cg.append_block(&format!("sm_match_arm{i}"));
        let next_check_bb = if i + 1 < arms.len() {
            cg.append_block(&format!("sm_match_check{}", i + 1))
        } else {
            final_fallthrough_bb
        };

        let pat_cond = match &arm.pattern.kind {
            MatchPatternKind::Value(pat_expr) => {
                let pat_val = lower_expr(cg, pat_expr)?;
                match_cmp(cg, &scrutinee_ty, scrutinee_val, pat_val)?
            }
            MatchPatternKind::OptionName(variant_name) => {
                if let Type::Options { name: opts_name } = &scrutinee_ty {
                    if let Some(entry) = cg.options_table.options.get(opts_name.as_str()) {
                        let tag = entry.variants.iter().position(|v| v == variant_name)
                            .ok_or_else(|| format!("SM match: unknown variant `{variant_name}` in options `{opts_name}`"))? as u64;
                        let tag_val = cg.ctx.i8_type().const_int(tag, false);
                        cg.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                scrutinee_val.into_int_value(),
                                tag_val,
                                "sm_opt_arm_cmp",
                            )
                            .map_err(|e| format!("{e}"))?
                    } else {
                        return Err(format!("SM match: unknown options type `{opts_name}`"));
                    }
                } else {
                    return Err(format!(
                        "SM match: OptionName arm on non-options type {:?}",
                        scrutinee_ty
                    ));
                }
            }
            MatchPatternKind::Is(type_path) => {
                if let Type::Union { variants } = &scrutinee_ty {
                    let tag = variants
                        .iter()
                        .position(|v| {
                            if let Type::Shape { name } = v {
                                name == &type_path.name
                            } else {
                                false
                            }
                        })
                        .ok_or_else(|| {
                            format!("SM match: union variant `{}` not found", type_path.name)
                        })? as u64;
                    let tag_const = cg.i64().const_int(tag, false);
                    let union_st = cg
                        .ctx
                        .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                    let tag_gep = cg
                        .builder
                        .build_struct_gep(
                            union_st,
                            scrutinee_val.into_pointer_value(),
                            0,
                            "sm_union_tag_gep",
                        )
                        .map_err(|e| format!("SM match union tag gep: {e}"))?;
                    let tag_loaded = cg
                        .builder
                        .build_load(cg.i64(), tag_gep, "sm_union_tag")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_loaded.into_int_value(),
                            tag_const,
                            "sm_union_arm_cmp",
                        )
                        .map_err(|e| format!("{e}"))?
                } else {
                    return Err(format!(
                        "SM match: Is arm on non-union type {:?}",
                        scrutinee_ty
                    ));
                }
            }
        };

        cg.builder
            .build_conditional_branch(pat_cond, arm_body_bb, next_check_bb)
            .map_err(|e| format!("SM match arm cond: {e}"))?;

        cg.builder.position_at_end(arm_body_bb);
        let arm_has_wait = function_contains_wait(&arm.body)
            || arm
                .body
                .stmts
                .iter()
                .any(|s| stmt_contains_suspending_call(s, cg.suspend_set));
        let locals_snapshot = cg.locals.clone();
        cg.sm_scope_depth += 1;
        if arm_has_wait {
            lower_sm_block(
                cg,
                &arm.body,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
        } else {
            for stmt in &arm.body.stmts {
                if is_block_terminated(cg) {
                    break;
                }
                lower_stmt(cg, stmt)?;
            }
        }
        cg.sm_scope_depth -= 1;
        for (name, &outer_alloca) in &locals_snapshot {
            cg.locals.insert(name.clone(), outer_alloca);
        }
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("SM match arm to merge: {e}"))?;
        }

        if i + 1 < arms.len() {
            cg.builder.position_at_end(next_check_bb);
        }
    }

    // Else arm (if present).
    if let Some(else_body) = else_arm {
        cg.builder.position_at_end(final_fallthrough_bb);
        let else_has_wait = function_contains_wait(else_body)
            || else_body
                .stmts
                .iter()
                .any(|s| stmt_contains_suspending_call(s, cg.suspend_set));
        let locals_snapshot = cg.locals.clone();
        cg.sm_scope_depth += 1;
        if else_has_wait {
            lower_sm_block(
                cg,
                else_body,
                state_blocks,
                pending_block,
                frame_ptr,
                waker_ctx,
                param_names,
                f,
                shape_table,
                current_state,
            )?;
        } else {
            for stmt in &else_body.stmts {
                if is_block_terminated(cg) {
                    break;
                }
                lower_stmt(cg, stmt)?;
            }
        }
        cg.sm_scope_depth -= 1;
        for (name, &outer_alloca) in &locals_snapshot {
            cg.locals.insert(name.clone(), outer_alloca);
        }
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;
        }
    }

    cg.builder.position_at_end(merge_bb);
    Ok(())
}

/// Load a suspending callee's return value from its return slot, using the callee's
/// declared return type to pick the correct load primitive.
///
/// Returns the value in the same representation `lower_expr` would produce for a
/// non-SM call to the same callee — so `bind_sm_return_value` and the SM `Stmt::Return`
/// forwarder can handle it without knowing the specific type.
///
/// - `Int` / `Bool` / `String` / `Shape` / `Array` / pointer-family: load i64, return IntValue.
/// - `Float`: load f64 (via i64 bitcast), return FloatValue.
/// - `Number { precision ≤ 34 }`: load i128 from 16-byte slot, return as i128 IntValue.
///   `bind_sm_return_value`'s I128Value arm allocates an i128 alloca and stores the value
///   so that `load(slot, &Type::Number, ...)` can read the full i128 from it.
/// - errors-capable (`-> T errors`, detected via `is_errors_capable_fn`): load the 2-slot
///   `{i64, i64}` errors struct from the return slot and rebuild it as a StructValue, so
///   `bind_sm_return_value`'s StructValue arm registers the binding in `errors_capable_locals`.
///   Without this an errors-capable callee in a parallel group falls into the i64 catch-all,
///   collapsing the `{err, ok}` struct to one word and dereferencing garbage (exit 139).
/// - Anything else: fall back to i64 load.
fn load_sm_return_value_typed<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    ctx: &'ctx inkwell::context::Context,
    frame_ptr: PointerValue<'ctx>,
    callee_name: &str,
    tag: &str,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    // `-> T errors`: the return slot holds the 2-slot `{i64, i64}` errors ABI result
    // (field0 = error pointer, field1 = success value). Load both halves and rebuild the
    // StructValue so `bind_sm_return_value`'s StructValue arm fires and registers the binding
    // in `errors_capable_locals`. This must be checked via `is_errors_capable_fn` — NOT via
    // the declared return type below — because `ast_type_to_typeck_type` strips the errors
    // wrapper for LOCAL callees (returns the bare inner type), so a local errors-capable
    // callee would otherwise fall into the i64 catch-all, collapse the 2-slot struct to one
    // word, and dereference garbage (exit 139) in a parallel group. Mirrors the single/
    // recursive EC-return load path (the `is_errors_capable_fn` branch in the non-parallel
    // callers).
    if is_errors_capable_fn(cg.typed, cg.imported_fns, callee_name) {
        let (err_i64, ok_i64) =
            state_machine::load_return_value_errors(ctx, &cg.builder, frame_ptr)?;
        let struct_ty = errors_result_type(ctx);
        let mut result = struct_ty.const_zero();
        result = cg
            .builder
            .build_insert_value(result, err_i64, 0, &format!("{tag}_err"))
            .map_err(|e| format!("{tag}_err insert: {e}"))?
            .into_struct_value();
        result = cg
            .builder
            .build_insert_value(result, ok_i64, 1, &format!("{tag}_ok"))
            .map_err(|e| format!("{tag}_ok insert: {e}"))?
            .into_struct_value();
        return Ok(result.into());
    }

    // Look up the callee's declared return type — check local items first, then
    // imported functions for cross-module callees.
    let callee_ret_ty = cg
        .typed
        .module
        .items
        .iter()
        .find_map(|item| {
            if let ynz_ast::nodes::Item::Function(f) = item {
                if f.name == callee_name {
                    return Some(ast_type_to_typeck_type(&f.return_type, cg.shape_table));
                }
            }
            None
        })
        .or_else(|| cg.imported_fns.get(callee_name).map(|sig| sig.ret.clone()));

    match callee_ret_ty {
        Some(Type::Float) => {
            // Float: return slot holds f64-as-i64 bits. Load as f64 via bitcast.
            let f_val = state_machine::load_return_value_f64(ctx, &cg.builder, frame_ptr, tag)?;
            Ok(f_val.into())
        }
        Some(Type::Number { precision }) if precision <= 34 => {
            // Decimal128: return slot holds the full i128. Load and return as IntValue.
            // bind_sm_return_value's I128Value arm will create an i128 alloca for it.
            let i128_val = state_machine::load_return_value_i128(ctx, &cg.builder, frame_ptr, tag)?;
            Ok(i128_val.into())
        }
        _ => {
            // All other types (int, bool, string, shape, array, etc.): load the i64
            // from the return slot (ptr-as-i64 for pointer-family; raw i64 for scalars).
            let ret_i64 = state_machine::load_return_value_i64(ctx, &cg.builder, frame_ptr, tag)?;
            Ok(ret_i64.into())
        }
    }
}

/// Extract the callee function name from a `Expr::Call(c)` expression, returning `None`
/// if the callee is not a simple identifier (e.g., a method call or computed callee).
///
/// Used to pass the callee name to `bind_sm_result_and_flush` so it can detect
/// `-> number errors` wide-EC returns and apply copy-on-bind.
fn callee_name_from_call_expr(expr: &Expr) -> Option<&str> {
    if let Expr::Call(c) = expr {
        if let Expr::Ident(name, _) = &c.callee {
            return Some(name.as_str());
        }
    }
    None
}

/// Bind a suspending call's return value to a named local alloca.
///
/// For i64 (int, bool, pointer-as-i64) return values: allocate an i64 alloca and store.
/// For {i64, i64} (errors-capable) return values: allocate a pointer to a stack-alloca struct.
/// Returns the alloca pointer for registration in `cg.locals`.
fn bind_sm_return_value<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    name: &str,
    return_val: inkwell::values::BasicValueEnum<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    match return_val {
        inkwell::values::BasicValueEnum::IntValue(iv) => {
            // load_sm_return_value_typed returns an i128 IntValue for decimal128 callees.
            // Use an i128 alloca so `load(slot, &Type::Number, ...)` reads the full value.
            // For all other types the value is i64 and an i64 alloca is correct.
            let alloca_ty: inkwell::types::BasicTypeEnum = if iv.get_type() == cg.ctx.i128_type() {
                cg.ctx.i128_type().into()
            } else {
                cg.i64().into()
            };
            let alloca = cg
                .builder
                .build_alloca(alloca_ty, &format!("{name}_alloca"))
                .map_err(|e| format!("sm let alloca {name}: {e}"))?;
            cg.builder
                .build_store(alloca, iv)
                .map_err(|e| format!("sm let store {name}: {e}"))?;
            Ok(alloca)
        }
        // Float: load_sm_return_value_typed returns a FloatValue for float-returning callees.
        // Allocate an f64 alloca and store the value so subsequent reads via lower_expr work.
        inkwell::values::BasicValueEnum::FloatValue(fv) => {
            let alloca = cg
                .builder
                .build_alloca(cg.ctx.f64_type(), &format!("{name}_f_alloca"))
                .map_err(|e| format!("sm let f alloca {name}: {e}"))?;
            cg.builder
                .build_store(alloca, fv)
                .map_err(|e| format!("sm let f store {name}: {e}"))?;
            Ok(alloca)
        }
        inkwell::values::BasicValueEnum::StructValue(sv) => {
            // errors-capable return: create the standard pointer-to-struct representation
            // (matches how lower_errors_capable_call_result works in the non-SM path).
            // 1. Alloca the struct on the stack.
            // 2. Store the struct value in the struct alloca.
            // 3. Create a ptr alloca to hold the pointer to the struct alloca.
            // 4. Store the struct pointer in the ptr alloca.
            // Note: caller must register `name` in errors_capable_locals after this.
            let struct_ty = errors_result_type(cg.ctx);
            let struct_alloca = cg
                .builder
                .build_alloca(struct_ty, &format!("{name}_ec_struct"))
                .map_err(|e| format!("sm let ec_struct {name}: {e}"))?;
            cg.builder
                .build_store(struct_alloca, sv)
                .map_err(|e| format!("sm let ec_struct store {name}: {e}"))?;
            // Create a ptr alloca that holds the pointer to the struct alloca.
            let ptr_alloca = cg
                .builder
                .build_alloca(cg.ptr(), &format!("{name}_ec_ptr"))
                .map_err(|e| format!("sm let ec_ptr {name}: {e}"))?;
            cg.builder
                .build_store(ptr_alloca, struct_alloca)
                .map_err(|e| format!("sm let ec_ptr store {name}: {e}"))?;
            cg.errors_capable_locals.insert(name.to_string());
            Ok(ptr_alloca)
        }
        inkwell::values::BasicValueEnum::PointerValue(pv) => {
            // pointer-valued return (string, shape, etc.) — store pointer as i64
            let as_i64 = cg
                .builder
                .build_ptr_to_int(pv, cg.ctx.i64_type(), &format!("{name}_ptr_i64"))
                .map_err(|e| format!("{e}"))?;
            let alloca = cg
                .builder
                .build_alloca(cg.i64(), &format!("{name}_alloca"))
                .map_err(|e| format!("sm let alloca {name}: {e}"))?;
            cg.builder
                .build_store(alloca, as_i64)
                .map_err(|e| format!("sm let store {name}: {e}"))?;
            Ok(alloca)
        }
        other => Err(format!(
            "bind_sm_return_value: unexpected variant {:?}",
            other
        )),
    }
}

/// Bind a state-machine callee's return value to `name`, then flush to frame if crossing.
///
/// If `name` was pre-allocated as a crossing local (frame-backed), reuse that alloca
/// and store the return value bits into it, then flush to the frame slot(s).
/// Otherwise, create a fresh alloca via `bind_sm_return_value` (non-crossing result binding).
///
/// Handles three value shapes:
/// - `IntValue` — int/bool/nothing: z-extend to i64, store, flush 1 slot.
/// - `PointerValue` — string/shape/array/map: ptr_to_int, store, flush 1 slot.
/// - `StructValue` — ErrorsCapable `{i64,i64}`: extract both fields, store into companion
///   struct alloca, flush 2 slots. This mirrors the EC flush path in
///   `flush_crossing_local_if_needed` and is the fix for wait-ecFn crossing a 2nd wait.
///
/// `callee_name` is used to detect `-> number errors` returns (wide-EC) so copy-on-bind
/// can fire for non-crossing bindings (see `bind_sm_return_value`).
///
/// After this call the alloca is registered in `cg.locals` under `name`.
fn bind_sm_result_and_flush<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    name: &str,
    mut return_val: inkwell::values::BasicValueEnum<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    callee_name: &str,
) -> Result<PointerValue<'ctx>, String> {
    let is_crossing = cg
        .sm_crossing_names
        .as_ref()
        .is_some_and(|v| v.iter().any(|n| n == name));

    if is_crossing {
        // The alloca was pre-created in sm_entry. Store the return value bits into it
        // and flush to the frame slot(s) so the value survives the next suspension.
        let alloca = *cg.locals.get(name).ok_or_else(|| {
            format!("bind_sm_result_and_flush: alloca for crossing local `{name}` missing")
        })?;
        // Look up slot index once (needed for both single and dual-slot paths).
        let slot_idx = {
            let crossing_names = cg
                .sm_crossing_names
                .as_ref()
                .ok_or_else(|| "bind_sm_result_and_flush: sm_crossing_names is None".to_string())?;
            let pos = crossing_names
                .iter()
                .position(|n| n == name)
                .ok_or_else(|| {
                    format!("bind_sm_result_and_flush: crossing local `{name}` not in slot index")
                })?;
            cg.sm_crossing_slot_indices[pos]
        };
        match return_val {
            inkwell::values::BasicValueEnum::StructValue(sv)
                if cg.sm_crossing_errors_capable_set.contains(name) =>
            {
                let ec_struct_ty = cg
                    .ctx
                    .struct_type(&[cg.ctx.i64_type().into(), cg.ctx.i64_type().into()], false);
                let f0 = cg
                    .builder
                    .build_extract_value(sv, 0, &format!("{name}_ec_f0"))
                    .map_err(|e| format!("bind_sm_result ec extract f0 {name}: {e}"))?
                    .into_int_value();
                let f1 = cg
                    .builder
                    .build_extract_value(sv, 1, &format!("{name}_ec_f1"))
                    .map_err(|e| format!("bind_sm_result ec extract f1 {name}: {e}"))?
                    .into_int_value();
                let struct_ptr = *cg.sm_crossing_ec_struct_allocas.get(name).ok_or_else(|| {
                    format!("bind_sm_result_and_flush: EC companion alloca for `{name}` missing")
                })?;
                // Always flush f0 (error discriminant) to slot N.
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, f0)?;
                let f0_ptr = cg
                    .builder
                    .build_struct_gep(ec_struct_ty, struct_ptr, 0, &format!("{name}_bind_f0_gep"))
                    .map_err(|e| format!("bind_sm_result ec gep f0 {name}: {e}"))?;
                let f1_ptr = cg
                    .builder
                    .build_struct_gep(ec_struct_ty, struct_ptr, 1, &format!("{name}_bind_f1_gep"))
                    .map_err(|e| format!("bind_sm_result ec gep f1 {name}: {e}"))?;
                cg.builder
                    .build_store(f0_ptr, f0)
                    .map_err(|e| format!("bind_sm_result ec store f0 {name}: {e}"))?;

                if cg.sm_crossing_ec_number_set.contains(name) {
                    // EC<Number>: f1 is a staging-slot pointer. Copy the i128 decimal bits
                    // into the per-binding alloca immediately so a subsequent same-callee call
                    // that reuses the staging slot cannot clobber this binding's value.
                    // Guard the deref: on the error path f0 != 0 and f1 == 0 (null deref).
                    let i128_alloca =
                        *cg.sm_crossing_ec_number_i128_allocas
                            .get(name)
                            .ok_or_else(|| {
                                format!(
                                "bind_sm_result_and_flush: EC num i128 alloca for `{name}` missing"
                            )
                            })?;
                    let is_ok = cg
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            f0,
                            cg.ctx.i64_type().const_int(0, false),
                            &format!("{name}_cob_isok"),
                        )
                        .map_err(|e| format!("bind_sm_result ec_num isok cmp {name}: {e}"))?;
                    let cob_copy_bb = cg.append_block(&format!("{name}_cob_copy"));
                    let cob_merge_bb = cg.append_block(&format!("{name}_cob_merge"));
                    cg.builder
                        .build_conditional_branch(is_ok, cob_copy_bb, cob_merge_bb)
                        .map_err(|e| format!("bind_sm_result ec_num branch {name}: {e}"))?;
                    // Success path: copy i128 from staging slot into per-binding alloca.
                    cg.builder.position_at_end(cob_copy_bb);
                    let staging_ptr = cg
                        .builder
                        .build_int_to_ptr(
                            f1,
                            cg.ctx.ptr_type(inkwell::AddressSpace::default()),
                            &format!("{name}_cob_sptr"),
                        )
                        .map_err(|e| format!("bind_sm_result ec_num int_to_ptr {name}: {e}"))?;
                    let i128_val = cg
                        .builder
                        .build_load(cg.ctx.i128_type(), staging_ptr, &format!("{name}_cob_i128"))
                        .map_err(|e| format!("bind_sm_result ec_num load i128 {name}: {e}"))?
                        .into_int_value();
                    cg.builder
                        .build_store(i128_alloca, i128_val)
                        .map_err(|e| format!("bind_sm_result ec_num store i128 {name}: {e}"))?;
                    cg.builder
                        .build_unconditional_branch(cob_merge_bb)
                        .map_err(|e| format!("bind_sm_result ec_num copy->merge {name}: {e}"))?;
                    cg.builder.position_at_end(cob_merge_bb);
                    // Repoint f1 at the per-binding alloca (stable for this resume call).
                    let new_f1 = cg
                        .builder
                        .build_ptr_to_int(
                            i128_alloca,
                            cg.ctx.i64_type(),
                            &format!("{name}_cob_newf1"),
                        )
                        .map_err(|e| format!("bind_sm_result ec_num ptr_to_int {name}: {e}"))?;
                    cg.builder
                        .build_store(f1_ptr, new_f1)
                        .map_err(|e| format!("bind_sm_result ec_num f1 store {name}: {e}"))?;
                    // Flush i128 bits as lo/hi into frame slots N+1, N+2.
                    // Load from i128_alloca here (not from i128_val which lives in cob_copy_bb)
                    // so the instruction dominates all successors of cob_merge_bb.
                    let i128_for_flush = cg
                        .builder
                        .build_load(
                            cg.ctx.i128_type(),
                            i128_alloca,
                            &format!("{name}_cob_reload"),
                        )
                        .map_err(|e| format!("bind_sm_result ec_num reload {name}: {e}"))?
                        .into_int_value();
                    let lo = cg
                        .builder
                        .build_int_truncate(
                            i128_for_flush,
                            cg.ctx.i64_type(),
                            &format!("{name}_cob_lo"),
                        )
                        .map_err(|e| format!("bind_sm_result ec_num lo {name}: {e}"))?;
                    let shift_amt = cg.ctx.i128_type().const_int(64, false);
                    let shifted = cg
                        .builder
                        .build_right_shift(
                            i128_for_flush,
                            shift_amt,
                            false,
                            &format!("{name}_cob_sh"),
                        )
                        .map_err(|e| format!("bind_sm_result ec_num shift {name}: {e}"))?;
                    let hi = cg
                        .builder
                        .build_int_truncate(shifted, cg.ctx.i64_type(), &format!("{name}_cob_hi"))
                        .map_err(|e| format!("bind_sm_result ec_num hi {name}: {e}"))?;
                    state_machine::store_local_slot(
                        cg.ctx,
                        &cg.builder,
                        frame_ptr,
                        slot_idx + 1,
                        lo,
                    )?;
                    state_machine::store_local_slot(
                        cg.ctx,
                        &cg.builder,
                        frame_ptr,
                        slot_idx + 2,
                        hi,
                    )?;
                } else {
                    // All other ErrorsCapable: f1 is the ok-word (a heap pointer or int-as-i64).
                    // Store f1 directly in slot N+1 and the companion struct.
                    cg.builder
                        .build_store(f1_ptr, f1)
                        .map_err(|e| format!("bind_sm_result ec store f1 {name}: {e}"))?;
                    state_machine::store_local_slot(
                        cg.ctx,
                        &cg.builder,
                        frame_ptr,
                        slot_idx + 1,
                        f1,
                    )?;
                }
                // Ensure ptr alloca points at companion struct.
                cg.builder
                    .build_store(alloca, struct_ptr)
                    .map_err(|e| format!("bind_sm_result ec ptr init {name}: {e}"))?;
            }
            // Shape crossing local: the callee stored the struct pointer as i64 bits in the
            // return slot (lower_stmt_return: `ptr_to_int → store_return_value_i64`). The
            // alloca is a ptr alloca pre-wired to the composed frame's slot region (Step 1b).
            // Convert the i64 bits back to a pointer, then memcpy the struct bytes into the
            // frame region — the frame IS the persistent storage for the shape.
            inkwell::values::BasicValueEnum::IntValue(bits_iv)
                if cg.sm_crossing_shape_embed_set.contains(name) =>
            {
                let shape_name = cg
                    .sm_crossing_shape_names
                    .get(name)
                    .ok_or_else(|| {
                        format!("bind_sm_result_and_flush: shape name for `{name}` missing")
                    })?
                    .clone();
                let struct_ty = cg.shape_types.get(&shape_name).ok_or_else(|| {
                    format!("bind_sm_result_and_flush: LLVM type for `{shape_name}` missing")
                })?;
                let size_val = struct_ty.size_of().ok_or_else(|| {
                    format!("bind_sm_result_and_flush: size_of `{shape_name}` unavailable")
                })?;
                let size_i64 = cg
                    .builder
                    .build_int_z_extend(size_val, cg.ctx.i64_type(), &format!("{name}_bind_sz"))
                    .map_err(|e| format!("bind_sm_result shape size extend {name}: {e}"))?;
                // Reconstruct the pointer from the i64 bits (reverses lower_stmt_return's ptr_to_int).
                let src_ptr = cg
                    .builder
                    .build_int_to_ptr(
                        bits_iv,
                        cg.ctx.ptr_type(inkwell::AddressSpace::default()),
                        &format!("{name}_bind_i2p"),
                    )
                    .map_err(|e| format!("bind_sm_result shape int_to_ptr {name}: {e}"))?;
                // Load the frame region ptr from the ptr alloca (pre-wired to frame slot region).
                let dest_ptr = cg
                    .builder
                    .build_load(
                        cg.ctx.ptr_type(inkwell::AddressSpace::default()),
                        alloca,
                        &format!("{name}_bind_frame_ptr"),
                    )
                    .map_err(|e| format!("bind_sm_result shape load frame ptr {name}: {e}"))?
                    .into_pointer_value();
                cg.builder
                    .build_memcpy(dest_ptr, 1, src_ptr, 1, size_i64)
                    .map_err(|e| format!("bind_sm_result shape memcpy {name}: {e}"))?;
                // No frame slot store needed — the bytes are already in the frame region.
                // The alloca holds the frame region ptr (set in Step 1b); it is stable.
            }
            // Decimal128 crossing local: load_sm_return_value_typed returns i128 IntValue.
            // Split i128 into lo/hi i64 halves and store in 2 consecutive frame slots —
            // matching the decimal128 flush/reload scheme used for let-defined crossing locals.
            inkwell::values::BasicValueEnum::IntValue(iv)
                if iv.get_type() == cg.ctx.i128_type() =>
            {
                cg.builder
                    .build_store(alloca, iv)
                    .map_err(|e| format!("bind_sm_result dec store {name}: {e}"))?;
                let lo = cg
                    .builder
                    .build_int_truncate(iv, cg.ctx.i64_type(), &format!("{name}_bind_lo"))
                    .map_err(|e| format!("bind_sm_result dec lo {name}: {e}"))?;
                let shift_amt = cg.ctx.i128_type().const_int(64, false);
                let shifted = cg
                    .builder
                    .build_right_shift(iv, shift_amt, false, &format!("{name}_bind_sh"))
                    .map_err(|e| format!("bind_sm_result dec shift {name}: {e}"))?;
                let hi = cg
                    .builder
                    .build_int_truncate(shifted, cg.ctx.i64_type(), &format!("{name}_bind_hi"))
                    .map_err(|e| format!("bind_sm_result dec hi {name}: {e}"))?;
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, lo)?;
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx + 1, hi)?;
            }
            // Float crossing local: load_sm_return_value_typed returns FloatValue.
            // Bitcast f64 → i64 and flush 1 slot — matching the float flush scheme.
            inkwell::values::BasicValueEnum::FloatValue(fv) => {
                let as_i64 = cg
                    .builder
                    .build_bit_cast(fv, cg.ctx.i64_type(), &format!("{name}_bind_f_to_i"))
                    .map_err(|e| format!("bind_sm_result float bitcast {name}: {e}"))?
                    .into_int_value();
                cg.builder
                    .build_store(alloca, fv)
                    .map_err(|e| format!("bind_sm_result float store {name}: {e}"))?;
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, as_i64)?;
            }
            _ => {
                // Int/bool/ptr (non-shape): convert to i64 bits, store into alloca, flush 1 slot.
                let bits = match return_val {
                    inkwell::values::BasicValueEnum::IntValue(iv) => {
                        if iv.get_type() != cg.ctx.i64_type() {
                            cg.builder
                                .build_int_z_extend(iv, cg.ctx.i64_type(), &format!("{name}_widen"))
                                .map_err(|e| format!("bind_sm_result widen {name}: {e}"))?
                        } else {
                            iv
                        }
                    }
                    inkwell::values::BasicValueEnum::PointerValue(pv) => cg
                        .builder
                        .build_ptr_to_int(pv, cg.ctx.i64_type(), &format!("{name}_ptr_i64"))
                        .map_err(|e| format!("bind_sm_result ptr_to_int {name}: {e}"))?,
                    other => {
                        return Err(format!(
                    "bind_sm_result_and_flush: unexpected return type for crossing local `{name}`: {other:?}"
                ))
                    }
                };
                // Bool crossing locals have an i1 alloca (matching the rest of codegen);
                // the return slot always holds the value as i64 (1-bit zero-extended). Store
                // i1 into the alloca (truncate from i64) to prevent an i64-into-i1-alloca
                // type mismatch that overwrites 7 bytes past the alloca, corrupting adjacent
                // group-member allocas (the parallel-group bool-sibling slot-corruption bug).
                let store_val: inkwell::values::BasicValueEnum = if cg
                    .sm_crossing_bool_set
                    .contains(name)
                {
                    cg.builder
                        .build_int_truncate(bits, cg.ctx.bool_type(), &format!("{name}_bind_trunc"))
                        .map_err(|e| format!("bind_sm_result bool trunc {name}: {e}"))?
                        .into()
                } else {
                    bits.into()
                };
                cg.builder
                    .build_store(alloca, store_val)
                    .map_err(|e| format!("bind_sm_result store {name}: {e}"))?;
                // Frame slot always holds i64 (zext of the bit); slot_idx is already correct.
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
            }
        }
        Ok(alloca)
    } else {
        // Non-crossing: copy-on-bind for wide-EC (-> number errors) before delegating to
        // bind_sm_return_value. The EC ok-word for a `-> number errors` callee is a pointer
        // into the callee's 16-byte decimal128 staging slot, which lives inside the shared
        // child sub-frame embedded in the caller's composed heap frame. A second call to
        // the same callee reuses that sub-frame (and its staging slot), overwriting the i128
        // before this binding's value is read. Copying the i128 into a per-binding stack
        // alloca now gives this binding its own stable storage that no subsequent call can
        // clobber. Stack alloca is correct here: non-crossing bindings never survive a
        // suspension, so the alloca's lifetime covers all uses within the current resume state.
        if is_number_errors_callee(cg.typed, cg.imported_fns, callee_name) {
            if let inkwell::values::BasicValueEnum::StructValue(sv) = return_val {
                let ec_struct_ty = cg
                    .ctx
                    .struct_type(&[cg.ctx.i64_type().into(), cg.ctx.i64_type().into()], false);
                // Extract the error word (f0) and ok word (f1 = staging slot ptr as i64).
                let err_bits = cg
                    .builder
                    .build_extract_value(sv, 0, &format!("{name}_cob_err"))
                    .map_err(|e| format!("copy-on-bind extract err {name}: {e}"))?
                    .into_int_value();
                let ok_bits = cg
                    .builder
                    .build_extract_value(sv, 1, &format!("{name}_cob_ok"))
                    .map_err(|e| format!("copy-on-bind extract ok {name}: {e}"))?
                    .into_int_value();
                // Per-binding stable storage allocated unconditionally so the pointer is
                // always valid for the EC struct's f1 field. Error path: f0 != 0 → `.or()`
                // reads f0 first and branches to the fallback; ok_bits (f1) is extracted
                // above but never dereferenced as a pointer on the error path.
                let binding_alloca = cg
                    .builder
                    .build_alloca(cg.ctx.i128_type(), &format!("{name}_dec_own"))
                    .map_err(|e| format!("copy-on-bind alloca {name}: {e}"))?;
                // Guard the staging-slot load: only dereference ok_bits (staging slot ptr)
                // on the success path (f0 == 0). On the error path f1 == 0 — a null deref.
                let is_ok = cg
                    .builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        err_bits,
                        cg.ctx.i64_type().const_int(0, false),
                        &format!("{name}_cob_isok"),
                    )
                    .map_err(|e| format!("copy-on-bind isok cmp {name}: {e}"))?;
                let cob_copy_bb = cg.append_block(&format!("{name}_cob_copy"));
                let cob_merge_bb = cg.append_block(&format!("{name}_cob_merge"));
                cg.builder
                    .build_conditional_branch(is_ok, cob_copy_bb, cob_merge_bb)
                    .map_err(|e| format!("copy-on-bind branch {name}: {e}"))?;
                // Success path: copy the i128 from the staging slot into the binding alloca.
                cg.builder.position_at_end(cob_copy_bb);
                let staging_ptr = cg
                    .builder
                    .build_int_to_ptr(
                        ok_bits,
                        cg.ctx.ptr_type(inkwell::AddressSpace::default()),
                        &format!("{name}_cob_sptr"),
                    )
                    .map_err(|e| format!("copy-on-bind int_to_ptr {name}: {e}"))?;
                let i128_val = cg
                    .builder
                    .build_load(cg.ctx.i128_type(), staging_ptr, &format!("{name}_cob_i128"))
                    .map_err(|e| format!("copy-on-bind load i128 {name}: {e}"))?
                    .into_int_value();
                cg.builder
                    .build_store(binding_alloca, i128_val)
                    .map_err(|e| format!("copy-on-bind store {name}: {e}"))?;
                cg.builder
                    .build_unconditional_branch(cob_merge_bb)
                    .map_err(|e| format!("copy-on-bind copy->merge {name}: {e}"))?;
                cg.builder.position_at_end(cob_merge_bb);
                // Repoint ok-word at the per-binding alloca. The EC struct's callers (`.or()`,
                // `.failed()`) always check f0 first; on error, f1 is never loaded.
                let new_ok_bits = cg
                    .builder
                    .build_ptr_to_int(
                        binding_alloca,
                        cg.ctx.i64_type(),
                        &format!("{name}_cob_newok"),
                    )
                    .map_err(|e| format!("copy-on-bind ptr_to_int {name}: {e}"))?;
                // Rebuild the EC struct with the stable ok-word. err_bits is unchanged.
                let mut new_sv = ec_struct_ty.const_zero();
                new_sv = cg
                    .builder
                    .build_insert_value(new_sv, err_bits, 0, &format!("{name}_cob_sv0"))
                    .map_err(|e| format!("copy-on-bind insert err {name}: {e}"))?
                    .into_struct_value();
                new_sv = cg
                    .builder
                    .build_insert_value(new_sv, new_ok_bits, 1, &format!("{name}_cob_sv1"))
                    .map_err(|e| format!("copy-on-bind insert ok {name}: {e}"))?
                    .into_struct_value();
                return_val = new_sv.into();
            }
        }
        let alloca = bind_sm_return_value(cg, name, return_val)?;
        Ok(alloca)
    }
}

/// Number of i64 frame slots needed to store a shape's bytes inline in the composed frame.
///
/// Uses the pre-computed ABI byte size from LLVM (`struct_ty.size_of()`) — the same
/// source as the memcpy in the shape-let codegen — so slot-count and memcpy-size never
/// diverge. `ceil(byte_size / 8)` rounds up to the next 8-byte slot boundary.
fn shape_frame_slots(shape_name: &str, shape_abi_sizes: &HashMap<String, u64>) -> usize {
    // Fallback to 1 slot (8 bytes) if the shape is not in the precomputed map. This
    // can only happen for shapes not seen during emit_shape_types (compiler bug).
    let byte_size = shape_abi_sizes.get(shape_name).copied().unwrap_or(8);
    // At minimum 1 slot even for a zero-byte struct (degenerate; avoids zero-size alloca).
    (byte_size.max(8) as usize).div_ceil(8)
}

/// Most types fit in 1 slot (8 bytes). Decimal128 (number with precision ≤ 34) is
/// 16 bytes and stored directly in the frame using 2 consecutive i64 slots.
/// ErrorsCapable `{i64,i64}` similarly uses 2 consecutive slots.
/// Shape crossing locals are frame-embedded: their bytes occupy `ceil(N/8)` consecutive
/// slots (no separate heap allocation — avoids the leak + re-promotion bugs).
///
/// Uses LLVM ABI sizes (via `shape_abi_sizes`) so slot-count and the memcpy in
/// lower_function_with_waits read from the same source of truth.
/// Uses typeck expr_types (from `typed`) to detect decimal128 locals including
/// inferred ones (no annotation).
fn crossing_local_total_slots(
    f: &ynz_ast::nodes::FunctionDecl,
    crossing_names: &[String],
    typed: &TypedModule,
    shape_abi_sizes: &HashMap<String, u64>,
) -> usize {
    let mut total = 0usize;
    for cname in crossing_names {
        let ty = find_let_typeck_type_in_stmts(&f.body.stmts, cname.as_str(), typed);
        let slots = match ty {
            // decimal128: 16-byte value stored in 2 consecutive i64 frame slots.
            Some(Type::Number { precision }) if precision <= 34 => 2,
            // EC<Number> (-> number errors): 3 frame slots {f0, i128_lo, i128_hi}.
            // The i128 decimal bits are stored directly so same-callee calls that reuse
            // the callee's staging slot cannot clobber a live crossing binding.
            Some(Type::ErrorsCapable { ref inner }) if matches!(inner.as_ref(), Type::Number { precision } if *precision <= 34) => {
                3
            }
            // All other ErrorsCapable {i64,i64}: 2 frame slots for the two fields.
            Some(Type::ErrorsCapable { .. }) => 2,
            // Shape: frame-embed the struct bytes in ceil(N/8) consecutive slots.
            Some(Type::Shape { name: ref sname }) => shape_frame_slots(sname, shape_abi_sizes),
            _ => 1,
        };
        total += slots;
    }
    total
}

/// Look up the typeck-inferred `Type` for a let binding or for-loop variable by scanning
/// the function body.
///
/// Handles both `Stmt::Let` bindings (returns the RHS expression type from `expr_types`)
/// and `Stmt::For` loop variables (returns the element type derived from the iterator's
/// collection type). The for-loop case is required so that decimal128 loop vars (which
/// have no Stmt::Let) get their correct 2-slot width in crossing_local_total_slots and
/// crossing_slot_indices — without it, a `number` loop var is assigned 1 slot and the
/// flush/reload writes out of bounds into the next local's slot region.
fn find_let_typeck_type_in_stmts(
    stmts: &[Stmt],
    target: &str,
    typed: &TypedModule,
) -> Option<Type> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value, .. } if name == target => {
                let key = (value.span().start, value.span().end);
                return typed.expr_types.get(&key).cloned();
            }
            // For-loop variables have no Stmt::Let — derive the element type from the
            // iterator expression's collection type as recorded in expr_types.
            Stmt::For {
                var, iter, body, ..
            } if var == target => {
                let key = (iter.span().start, iter.span().end);
                if let Some(iter_ty) = typed.expr_types.get(&key).cloned() {
                    let elem_ty = match iter_ty {
                        Type::BuiltinArray { elem } | Type::BuiltinFixed { elem, .. } => {
                            Some(*elem)
                        }
                        Type::Range { .. } => Some(Type::Int),
                        Type::BuiltinMap { key: k, val: v } => {
                            Some(Type::MapEntry { key: k, val: v })
                        }
                        _ => None,
                    };
                    if let Some(t) = elem_ty {
                        return Some(t);
                    }
                }
                // Recurse into body for declarations sharing the var name (unlikely but safe).
                if let Some(t) = find_let_typeck_type_in_stmts(&body.stmts, target, typed) {
                    return Some(t);
                }
            }
            Stmt::If { body, .. } => {
                if let Some(t) = find_let_typeck_type_in_stmts(&body.stmts, target, typed) {
                    return Some(t);
                }
            }
            // Recurse into loop/match bodies so a crossing local declared inside one of
            // these constructs is found by slot-width classification. Without this, a
            // decimal128 or EC local declared in a while/for/match body would default to
            // 1 slot and silently truncate (Tier-A silent-wrong-output).
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(t) = find_let_typeck_type_in_stmts(&body.stmts, target, typed) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) = find_let_typeck_type_in_stmts(&arm.body.stmts, target, typed) {
                        return Some(t);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(t) = find_let_typeck_type_in_stmts(&eb.stmts, target, typed) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Look up the typeck `Type` of a crossing local's value expression.
///
/// Scans the function body for the first `Stmt::Let { name }` or `Stmt::For { var }`
/// matching `target` and returns its type. Falls back to `Type::Int` when no match is
/// found, which is safe for two reasons:
///   1. Synthetic loop-index locals (`__ynz_for_idx_N`) have no Stmt::Let and ARE Int.
///   2. Unsupported types (MapEntry, BuiltinFixed, union, maybe, dynamic) are blocked
///      by UnsupportedCrossingLocalType at typeck before any codegen runs, so they
///      never reach this function on valid input.
///
/// The Int fallback is intentional-and-documented, not a silent-wrong classification.
fn crossing_local_type_from_body<'ctx>(
    body: &ynz_ast::nodes::Block,
    target: &str,
    cg: &Cg<'ctx, '_>,
) -> Type {
    find_let_type_in_stmts(&body.stmts, target, cg).unwrap_or(Type::Int)
}

fn find_let_type_in_stmts<'ctx>(stmts: &[Stmt], target: &str, cg: &Cg<'ctx, '_>) -> Option<Type> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value, .. } if name == target => {
                return Some(cg.expr_type(value));
            }
            // For-loop variable: the loop var is bound by the iteration mechanism, not via
            // Stmt::Let, so the Stmt::Let arm above never fires. Derive the element type
            // directly from the iterator expression's collection type so the type classifier
            // picks the correct alloca (i1 for bool, f64 for float, i64 for int, i128 for
            // decimal128, {i64,i64} struct for MapEntry).
            Stmt::For {
                var, iter, body, ..
            } if var == target => {
                let iter_ty = cg.expr_type(iter);
                let elem_ty = match iter_ty {
                    Type::BuiltinArray { elem } | Type::BuiltinFixed { elem, .. } => Some(*elem),
                    // Range element is always Int.
                    Type::Range { .. } => Some(Type::Int),
                    // Map iteration: the loop var is a MapEntry<K,V> struct. Returning
                    // the real type here ensures UnsupportedCrossingLocalType is triggered
                    // by codegen's classifier for names that reach flush_for_loop_var.
                    Type::BuiltinMap { key, val } => Some(Type::MapEntry { key, val }),
                    _ => None,
                };
                if let Some(t) = elem_ty {
                    return Some(t);
                }
                // Recurse into body for declarations with the same name.
                if let Some(t) = find_let_type_in_stmts(&body.stmts, target, cg) {
                    return Some(t);
                }
            }
            Stmt::If { body, .. } => {
                if let Some(t) = find_let_type_in_stmts(&body.stmts, target, cg) {
                    return Some(t);
                }
            }
            // Mirror the same recursion as find_let_typeck_type_in_stmts: crossing locals
            // declared inside loop/match bodies must be found for correct type classification.
            Stmt::While { body, .. } | Stmt::For { body, .. } => {
                if let Some(t) = find_let_type_in_stmts(&body.stmts, target, cg) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) = find_let_type_in_stmts(&arm.body.stmts, target, cg) {
                        return Some(t);
                    }
                }
                if let Some(eb) = else_arm {
                    if let Some(t) = find_let_type_in_stmts(&eb.stmts, target, cg) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// True when `expr` is a `sleep(...)` call (the yielding non-blocking sleep intrinsic).
fn is_sleep_call(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(c) if matches!(&c.callee, Expr::Ident(n, _) if n == "sleep"))
}

/// True when `expr` is a direct call to a user-defined suspending function (not a may-block intrinsic).
fn is_direct_suspending_call(expr: &Expr, suspend_set: &SuspendSet) -> bool {
    if let Expr::Call(c) = expr {
        if let Expr::Ident(name, _) = &c.callee {
            return suspend_set.contains(name.as_str())
                && !M2_MAY_BLOCK_INTRINSICS.contains(&name.as_str());
        }
    }
    false
}

// ── v0.3-M3d: CPU-parallel join helpers ──────────────────────────────────────
//
// Code in this section fires for any function the typeck `cpu_promotion_query` promotes
// (the production trigger). The poll-based CPU join mechanism it emits was proven
// end-to-end through the real compiler in Phase 0; later slices generalize the layout
// from the fixed Phase-0 two-member group to N-member / multi-group bodies.

/// Fallback byte offset of CPU handle slot 0 within the parent SM frame.
///
/// The live offsets come from `FrameLayout::cpu_group_slots` (computed in
/// `build_frame_layouts` — the single source of truth). These constants are the
/// defensive fallback the join emission uses only if a layout entry is somehow absent,
/// which cannot happen for a promoted (in-suspend-set) function. They encode the Phase-0
/// two-member layout: two handle slots then two result slots immediately after the
/// 32-byte frame header.
const SPIKE_HANDLE_0_OFFSET: u64 = 32;
const SPIKE_HANDLE_1_OFFSET: u64 = 40;
const SPIKE_RESULT_0_OFFSET: u64 = 48;
const SPIKE_RESULT_1_OFFSET: u64 = 64;

/// Codegen's AST-level mirror of typeck's `cpu_result_abi_supports`
/// (`ynz-typeck/src/independence.rs`) — THE single source of truth for which return classes a
/// CPU-parallel group member may have. This function classifies the *un-resolved* AST `Type`
/// codegen has at its candidacy gates; the typeck predicate classifies the resolved `Type`.
/// Both MUST admit/decline the IDENTICAL set so the IDE `parallel_groups` hint (driven by
/// typeck promotion) and the emitted binary (driven by this gate) never disagree on which
/// calls overlap. That equivalence is not left to inspection — it is locked by the
/// `cpu_result_abi_gate_parity` test below, which resolves a representative spread of types
/// and asserts both gates agree.
///
/// **ADMITS** (value or heap-stable pointer fits the 16-byte `YnzCpuResult` slot and outlives
/// the worker frame that produced it):
/// - `int` / `bool` / `float` — scalar word; bare `number` (decimal128) — heap-stable ABI ptr.
/// - `string` / `array<_>` / `map<_,_>` — a single owning heap pointer.
/// - `T errors` ONLY when `T` is a safe-to-carry class (int/bool/float/string/array/map).
///
/// **DECLINES** (sequential lowering is always correct — declining is a first-class
/// auto-promotion outcome, never an error):
/// - bare `Shape` / `Shape errors` — by-value shape needs variable-size staging
///   (WideValueSuspendingReturn). At the AST level a shape is a `Named` that is not `string`.
/// - `number errors` (and any wide-value-EC inner) — the 24-byte `{i64 err + i128 ok}` pair
///   overflows the 16-byte slot, so the ABI smuggles a pointer into the worker thread's dead
///   stack; join-bind would dereference it (use-after-free). Wide-EC is the parallel-path
///   manifestation of the staging-slot family tracked in `.claude/todos.md`; declined per the
///   plan's "Hotfix that isn't" rule and fixed on its own track, NOT in M3d.
/// - `fixed<_>` — the non-suspending `fixed`-return path returns a count of 0 (a separate
///   pre-existing base-codegen bug, byte-identical in both modes so not a parallel divergence);
///   admitting it would be dead anyway (it never fires), so it declines here to keep both
///   gates honest. Tracked with the `fixed`-return bug, not fixed in M3d.
/// - `maybe` / `union` / `dynamic` / `range` / any non-(array|map) `Generic` / a `Named` that
///   is not `string` (shape/options/union-alias) — outside this milestone's carried set.
/// - `nothing` — no value to join-bind; `Error` — an earlier type error poisoned the sig.
fn return_type_fits_cpu_result_abi(ret: &ynz_ast::nodes::Type) -> bool {
    use ynz_ast::nodes::Type as AstType;
    match ret {
        AstType::Int | AstType::Float | AstType::Bool | AstType::Number { .. } => true,
        // `string` is the only `Named` that fits — every other `Named` is a user shape, an
        // options type, or a union alias, all declined.
        AstType::Named(n, _) => n == "string",
        // Only growable/keyed heap collections return a single owning heap pointer. `fixed`
        // declines (see doc above).
        AstType::Generic { name, .. } => matches!(name.as_str(), "array" | "map"),
        // `T errors` admits only when the success word is a safe-to-carry class. A wide inner
        // (number → dead-worker-stack pointer; shape → variable-size staging) is declined.
        AstType::ErrorCapable { inner, .. } => ec_inner_fits_cpu_result_abi(inner),
        _ => false,
    }
}

/// True when `T` in an AST `T errors` return is safe to carry in the 8-byte ok-word of the
/// `{i64, i64}` errors ABI. Mirrors typeck's `cpu_result_ec_inner_is_safe`. `number` is
/// EXCLUDED even though bare `number` is admitted by [`return_type_fits_cpu_result_abi`]: bare
/// `number` returns a heap-stable ABI pointer, but `number errors` packs the i128 into the
/// callee's worker-thread staging slot and the ok-word points into it — that pointer dangles
/// the instant the worker frame dies. Shapes (`Named` ≠ `string`) and `fixed` are excluded for
/// the same wide/unstable-value family of reasons.
fn ec_inner_fits_cpu_result_abi(inner: &ynz_ast::nodes::Type) -> bool {
    use ynz_ast::nodes::Type as AstType;
    match inner {
        AstType::Int | AstType::Float | AstType::Bool => true,
        AstType::Named(n, _) => n == "string",
        AstType::Generic { name, .. } => matches!(name.as_str(), "array" | "map"),
        _ => false,
    }
}

/// Detect a 2-member CPU-parallel candidate group in `f`.
///
/// Returns `Some(callee_name)` when `f` contains an adjacent pair of `let x = callee(...)`
/// statements whose callees:
/// - are NOT in the suspend_set (pure CPU — not a state machine)
/// - return a class that fits the `YnzCpuResult` ABI (`return_type_fits_cpu_result_abi`)
///
/// **Admission envelope** (all must hold; sequential fallback is always correct):
/// - Host must be named `"entrypoint"` — non-entrypoint spike hosts require the
///   caller to pre-allocate a correctly-sized frame (via `frame_layouts`) before this
///   spike bypass path runs. Callers that go through the normal wrapper emit the frame
///   size from `frame_layouts` (pre-spike), so the resume function writes bytes 48–95
///   into a heap block that was only allocated 32 bytes. Declining non-entrypoint hosts
///   eliminates the corruption without requiring frame_layouts integration (P3's job).
/// - Zero params — param slots start at byte 32 (FRAME_HEADER_SIZE), colliding with
///   SPIKE_HANDLE_0_OFFSET. P3's canonical slot system lifts this.
/// - No rest-statement assigns a CPU-result bind name (at any depth). When a result name
///   is assigned after the group, the SM crossing machinery must own it from the start —
///   the upfront-prune approach left the initial read sourcing from a never-populated
///   crossing slot, returning zeroed bytes instead of the computed value.
///
/// Used in both `emit_artifact` (suspend_set extension) and `lower_function_with_waits`
/// (frame-size + state-count injection).
///
/// Time: O(n) where n = items in typed module  Space: O(k) where k = CPU-ABI-returning fns
fn spike_cpu_candidates(
    f: &FunctionDecl,
    typed: &TypedModule,
    suspend_set: &SuspendSet,
) -> Option<String> {
    // Entrypoint-only: non-entrypoint spike hosts have their frame allocated by the
    // caller's emit_suspending_call_heap_boxed fallback, which reads from frame_layouts
    // (computed before spike promotion). That pre-spike layout is too small: the spike
    // resume function writes handle/result bytes at offsets 32–79 into a frame sized
    // without the 48-byte spike reserve — heap corruption. Sequential is always correct.
    if f.name != "entrypoint" {
        return None;
    }

    // Zero-param hosts only. Param slots start at byte 32 (FRAME_HEADER_SIZE),
    // which is the same byte SPIKE_HANDLE_0_OFFSET occupies. A host with ≥1 params would
    // put the param reload into the handle slot — silent corruption or invalid-free.
    if !f.params.is_empty() {
        return None;
    }

    // Callees whose return class the CPU result ABI can safely carry — the exact set
    // `return_type_fits_cpu_result_abi` admits (its doc enumerates the admit/decline set and
    // the shared-truth invariant with typeck's `cpu_result_abi_supports`). Declined classes
    // run sequentially, byte-identical to `--no-auto-parallel`.
    let cpu_supported_callees: std::collections::HashSet<String> = typed
        .module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(func) = item {
                if return_type_fits_cpu_result_abi(&func.return_type) {
                    Some(func.name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Collect indices for all non-suspending direct-call statements whose callee returns a
    // CPU-result-ABI class.
    let mut call_indices: Vec<usize> = Vec::new();
    for (i, stmt) in f.body.stmts.iter().enumerate() {
        let is_eligible = match stmt {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => {
                if let Expr::Ident(name, _) = &c.callee {
                    !suspend_set.contains(name.as_str()) && cpu_supported_callees.contains(name)
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

    // Need an adjacent pair.
    let adjacent_pair = call_indices
        .windows(2)
        .find(|w| w[1] == w[0] + 1)
        .map(|w| (w[0], w[1]));
    let (first_idx, second_idx) = adjacent_pair?;

    // Collect bind names for the pair.
    let bind_names: Vec<&str> = [first_idx, second_idx]
        .iter()
        .filter_map(|&i| match &f.body.stmts[i] {
            Stmt::Let { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();

    // Decline when any pre-pair statement contains a wait or suspending call.
    // A wait-bearing pre-pair statement is lowered via lower_sm_stmt_with_wait which
    // advances current_state and positions the builder in a new post_wait_bb. The
    // subsequent emit_cpu_group_spawn_join then emits into a spawn_bb that has no
    // predecessor branch from that post_wait_bb — the post_wait_bb gets no terminator,
    // triggering "Basic Block does not have terminator!" in the LLVM verifier.
    // Declining routes the whole body through sequential lowering, which is always correct.
    let pre_stmts: Vec<&Stmt> = f.body.stmts.iter().take(first_idx).collect();
    if pre_stmts
        .iter()
        .any(|s| stmt_contains_wait(s) || stmt_contains_suspending_call(s, suspend_set))
    {
        return None;
    }

    // Decline when any rest statement (at any depth) assigns a CPU-result bind name.
    // Declining routes the whole group through sequential lowering which is always correct.
    let rest_stmts: Vec<&Stmt> = f
        .body
        .stmts
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != first_idx && *i != second_idx)
        .map(|(_, s)| s)
        .collect();
    for bind_name in &bind_names {
        if rest_stmts.iter().any(|s| stmt_assigns_name(s, bind_name)) {
            return None;
        }
    }

    // Decline when any post-pair statement contains a suspending user-defined callee.
    // A user-defined suspending callee embeds a child sub-frame inside the parent frame.
    // The spike host frame's child sub-frame offset is computed pre-spike (without the
    // 48-byte spike reserve), so the embedded child frame aliases the spike result region
    // (SPIKE_RESULT_{0,1}_OFFSET). Intrinsic waits (e.g. `wait sleep(...)`) use the
    // standard SM inline-poll path with no embedded child sub-frame, posing no aliasing
    // risk and therefore not matching stmt_contains_suspending_call. Declining routes the
    // whole body through sequential lowering, which is always correct and avoids aliasing.
    let post_stmts: Vec<&Stmt> = f.body.stmts.iter().skip(second_idx + 1).collect();
    if post_stmts
        .iter()
        .any(|s| stmt_contains_suspending_call(s, suspend_set))
    {
        return None;
    }

    // Report the first eligible callee as representative.
    match &f.body.stmts[first_idx] {
        Stmt::Let {
            value: Expr::Call(c),
            ..
        }
        | Stmt::Expr(Expr::Call(c)) => {
            if let Expr::Ident(name, _) = &c.callee {
                return Some(name.clone());
            }
        }
        _ => {}
    }
    None
}

/// Of the functions typeck promoted, the subset codegen will actually spike-HOST in this
/// slice — probed against the EFFECTIVE suspend set (local suspending names ∪
/// imported-suspending names), the SAME set both query boundaries size the frame with.
///
/// This reconciliation is the fix for the union-poisoning hazard: typeck's
/// `compute_cpu_promotions` promotes EVERY function that owns a CPU group (e.g. both an
/// inner `work` and an outer `entrypoint` in a nested program), but slice-1 codegen can
/// only host `entrypoint` (the entrypoint-only gate in `spike_cpu_candidates`). Unioning
/// the FULL promotion set into the SM suspend-set would (a) make a promoted-but-unhosted
/// callee like `work` an SM whose callers still call it as a plain int-returning fn
/// (trampoline mismatch), and (b) place `work` IN the suspend set `spike_cpu_candidates`
/// reads, so the host's own callee-eligibility filter (`!suspend_set.contains(callee)`)
/// would exclude `work` and the host's group would silently DECLINE — defeating the
/// parallelism typeck approved. Reconciling down to the actual host subset keeps a promoted
/// non-host callee out of the union: it neither becomes an SM nor poisons the host's
/// admission.
///
/// WHY the EFFECTIVE set, not the bare local set: `spike_cpu_candidates`'s post-pair
/// decline gate (`stmt_contains_suspending_call`) must see imported-suspending callees so
/// that BOTH query boundaries — `frame_layouts_query` (which SIZES the frame) and
/// `codegen_query` (which LAYS IT OUT) — reach the SAME spike-host admission decision and
/// size the host's frame identically. An imported-suspending name is absent from the bare
/// local set: probing with the bare set at one boundary would ADMIT a host that the other
/// boundary, probing with the effective set, DECLINED — frame_layouts then sizes the frame
/// sequentially (with the imported callee's child sub-frame) while codegen lays it out as a
/// spike host (omitting that sub-frame), under-allocating the heap block by exactly the
/// imported callee's frame size and corrupting it when the child writes at its offset. Both
/// callers MUST pass the effective set; a future caller reverted to the bare set walks back
/// into that under-allocation. The `imported_suspending_after_pair_*` tests are the tripwire.
///
/// Slice-2 carry-forward: a promoted inner host (`work`) is NOT spike-hosted here, so its
/// own CPU group runs sequentially this slice. That is the intended residual — codegen
/// catches up to typeck's full promotion set as the entrypoint-only gate is relaxed in
/// later slices. Output stays correct (sequential is always correct); only the inner
/// overlap is deferred.
///
/// Slice-2 carry-forward (benign over-allocation, logged in the plan Findings Log): an
/// `entrypoint` that calls ITSELF in a post-pair statement gets `"entrypoint"` into the
/// emit-time `suspends_with_promotions` host union but NOT into this probe's `promoted`
/// input at probe time, so the emit-time re-probe declines while this probe admitted →
/// a 48-byte OVER-allocation (dead spike reserve, NOT under-allocation — no corruption).
/// Slice 2 should align this probe's input set with the emit-time host set to drop even
/// that benign waste.
///
/// Time: O(p · k) where p = promoted fns, k = AST nodes scanned per candidate.
/// Space: O(p).
pub fn spike_host_subset(
    typed: &TypedModule,
    suspend_set: &SuspendSet,
    promoted: &HashSet<String>,
) -> HashSet<String> {
    let mut hosts: HashSet<String> = HashSet::new();
    for item in &typed.module.items {
        let Item::Function(f) = item else { continue };
        if promoted.contains(&f.name) && spike_cpu_candidates(f, typed, suspend_set).is_some() {
            hosts.insert(f.name.clone());
        }
    }
    hosts
}

/// Return the bind names of the CPU group that `spike_extract_cpu_group` would extract.
///
/// Applies the same adjacency, arg-lowering, data-dependency, and result-assignment-decline
/// checks as `spike_extract_cpu_group` so the two functions always agree on which statements
/// form the group. Returns the `let`-binding names for each group member (up to 2); returns
/// an empty Vec when no eligible adjacent pair exists or when any gate declines the group.
///
/// The entrypoint-only and zero-param gates are enforced by the caller (`lower_function_with_waits`
/// step 1c) which only calls this function when `spike_candidates.is_some()`. This function
/// applies the remaining gates that operate on the statement list.
///
/// Used during sm_entry pre-allocation (Step 1c) to create allocas in the function entry
/// block before the state machine blocks exist. Pre-allocating here ensures the allocas
/// dominate every state block, satisfying LLVM SSA dominance. When the group is declined,
/// the empty return means no allocas are created — correct because gate-2 will also decline.
///
/// Time: O(n) where n = stmts length  Space: O(k) where k = CPU-ABI-returning fns in module
fn spike_cpu_group_result_names(
    stmts: &[Stmt],
    suspend_set: &SuspendSet,
    typed: &TypedModule,
) -> Vec<String> {
    let cpu_supported_callees: std::collections::HashSet<String> = typed
        .module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(func) = item {
                if return_type_fits_cpu_result_abi(&func.return_type) {
                    Some(func.name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Collect indices of non-suspending direct-call statements whose callee returns a
    // CPU-result-ABI class. Must match `spike_cpu_candidates` / `spike_extract_cpu_group`
    // exactly so the Step-1c pre-alloc set agrees with the extraction set.
    let mut call_indices: Vec<usize> = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        let is_eligible = match stmt {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => {
                if let Expr::Ident(name, _) = &c.callee {
                    !suspend_set.contains(name.as_str())
                        && cpu_supported_callees.contains(name.as_str())
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

    // Find first adjacent pair.
    let adjacent_pair = call_indices
        .windows(2)
        .find(|w| w[1] == w[0] + 1)
        .map(|w| (w[0], w[1]));
    let (first_idx, second_idx) = match adjacent_pair {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Decline when any pre-pair statement contains a wait or suspending call.
    // Mirrors the guard in spike_cpu_candidates and spike_extract_cpu_group so all three
    // gates agree: when a wait-bearing pre-pair statement exists the group is declined and
    // no result allocas are pre-allocated (which would leave unreachable alloca instructions).
    let has_suspending_pre = stmts[..first_idx]
        .iter()
        .any(|s| stmt_contains_wait(s) || stmt_contains_suspending_call(s, suspend_set));
    if has_suspending_pre {
        return Vec::new();
    }

    // Arg-lowering gate: exactly one arg, IntLit or Ident.
    let args_lowerable = |stmt: &Stmt| -> bool {
        let call = match stmt {
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
    if !args_lowerable(&stmts[first_idx]) || !args_lowerable(&stmts[second_idx]) {
        return Vec::new();
    }

    // Data-dependency gate: second call must not use first call's bind name.
    let first_bind_name: Option<&str> = match &stmts[first_idx] {
        Stmt::Let { name, .. } => Some(name.as_str()),
        _ => None,
    };
    if let Some(bind_name) = first_bind_name {
        let second_uses_first = match &stmts[second_idx] {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => c
                .args
                .iter()
                .any(|arg| matches!(arg, Expr::Ident(n, _) if n.as_str() == bind_name)),
            _ => false,
        };
        if second_uses_first {
            return Vec::new();
        }
    }

    // Collect bind names for the group members.
    let bind_names: Vec<String> = [first_idx, second_idx]
        .iter()
        .filter_map(|&i| match &stmts[i] {
            Stmt::Let { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    // Result-assignment decline gate (mirrors spike_cpu_candidates and spike_extract_cpu_group):
    // when any rest statement assigns a CPU-result bind name at any depth, the SM crossing
    // machinery must own that name from the start. Declining ensures gate-1, gate-2, and
    // this pre-allocation function always agree on the group.
    let rest_stmts: Vec<&Stmt> = stmts
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != first_idx && *i != second_idx)
        .map(|(_, s)| s)
        .collect();
    for bind_name in &bind_names {
        if rest_stmts
            .iter()
            .any(|s| stmt_assigns_name(s, bind_name.as_str()))
        {
            return Vec::new();
        }
    }

    // Decline when any post-pair statement contains a suspending user-defined callee.
    // Mirrors the guard in spike_cpu_candidates and spike_extract_cpu_group: when a
    // user-defined suspending callee is present in post-pair stmts, no result allocas are
    // pre-allocated (leaving unreachable alloca instructions for declined groups). Intrinsic
    // waits (e.g. `wait sleep(...)`) embed no child sub-frame and pose no aliasing risk;
    // stmt_contains_suspending_call excludes them via the M2_MAY_BLOCK_INTRINSICS guard.
    let has_suspending_post = stmts[(second_idx + 1)..]
        .iter()
        .any(|s| stmt_contains_suspending_call(s, suspend_set));
    if has_suspending_post {
        return Vec::new();
    }

    bind_names
}

/// Extract a 2-member CPU group from `stmts`, returning `(pre_stmts, group_stmts, post_stmts)`.
///
/// Scans for the first two ADJACENT non-suspending int-returning direct calls in the statement
/// list. "Adjacent" means `second_idx == first_idx + 1` — the two calls have no intervening
/// statements between them. This is required to avoid a data-dependency hole: if there is a
/// `let b = a + 1` between `let a = f()` and `let c = g(b)`, extracting calls 0 and 2 would
/// spawn `g(b)` before `b` is computed (because the intervening let is in `rest`, lowered
/// after spawn). Adjacency ensures the group's arguments can only reference locals that are
/// already available at spawn time.
///
/// The same `return_type_fits_cpu_result_abi` membership filter as `spike_cpu_candidates` is
/// applied so the gate-1 check (frame sizing) and gate-2 check (group extraction) always agree:
/// a callee that gate-1 would not count for frame-slot allocation is never extracted by gate-2
/// either. The trampoline + join-bind serialize each admitted class into the 16-byte result
/// slot, so a class mismatch between the two gates can never surface a wrong-typed bind.
///
/// **Edge cases**: calls whose `args` has arity ≠ 1 are skipped by `args_lowerable` — the
/// trampoline ABI passes exactly one i64 argument; zero-arg and multi-arg calls are declined.
/// A call with a literal-only argument (e.g. `fib(10)`) is included; a call whose argument
/// is an expression other than `Ident` or `IntLit` is excluded.
///
/// `pre_stmts`: statements before the pair (indices 0..first_idx). These must be lowered
/// sequentially before spawning the pair, so any locals they produce are in scope at spawn time.
///
/// `post_stmts`: statements after the pair (indices first_idx+2..). These are lowered after join.
///
/// Returns `None` when no eligible adjacent pair exists, or when any post statement assigns
/// a CPU-result bind name (mirrors the decline gate in spike_cpu_candidates).
///
/// Time: O(n) where n = stmts length  Space: O(k) where k = CPU-ABI-returning fns in module
fn spike_extract_cpu_group<'s>(
    stmts: &'s [Stmt],
    suspend_set: &SuspendSet,
    typed: &TypedModule,
) -> Option<(Vec<&'s Stmt>, Vec<&'s Stmt>, Vec<&'s Stmt>)> {
    // Build the same CPU-result-ABI callee set used by spike_cpu_candidates so the
    // two gates cannot disagree on callee eligibility.
    let cpu_supported_callees: std::collections::HashSet<String> = typed
        .module
        .items
        .iter()
        .filter_map(|item| {
            if let Item::Function(func) = item {
                if return_type_fits_cpu_result_abi(&func.return_type) {
                    Some(func.name.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Collect indices for all non-suspending CPU-result-ABI direct-call statements.
    let mut call_indices: Vec<usize> = Vec::new();
    for (i, stmt) in stmts.iter().enumerate() {
        let is_eligible = match stmt {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => {
                if let Expr::Ident(name, _) = &c.callee {
                    !suspend_set.contains(name.as_str())
                        && cpu_supported_callees.contains(name.as_str())
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

    // Need at least 2 eligible calls; find the first adjacent pair.
    // Non-adjacent pairs are skipped: extracting non-adjacent calls could spawn the
    // second call's trampoline before the intervening statements (which may produce
    // values the second callee's argument depends on) have been lowered.
    let adjacent_pair = call_indices
        .windows(2)
        .find(|w| w[1] == w[0] + 1)
        .map(|w| (w[0], w[1]));

    let (first_idx, second_idx) = adjacent_pair?;

    // Decline when any pre-pair statement contains a wait or suspending call.
    // A wait-bearing pre-pair statement advances current_state and positions the builder
    // in a new post_wait_bb. The subsequent emit_cpu_group_spawn_join emits into a spawn_bb
    // with no predecessor branch from that post_wait_bb — the post_wait_bb gets no
    // terminator, causing "Basic Block does not have terminator!" in the LLVM verifier.
    // Sequential lowering is always correct; declining is always safe.
    if stmts[..first_idx]
        .iter()
        .any(|s| stmt_contains_wait(s) || stmt_contains_suspending_call(s, suspend_set))
    {
        return None;
    }

    // Arg-lowering eligibility: the spike arg-evaluator in emit_cpu_group_spawn_join can
    // only lower exactly one IntLit or Ident argument. The trampoline ctx holds a single
    // i64 — multi-arg or zero-arg callees cannot be packed into it without a wider ctx
    // layout (P3's job). Decline rather than emitting an LLVM verifier abort or Err on
    // a syntactically valid program.
    let args_lowerable = |stmt: &Stmt| -> bool {
        let call = match stmt {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => c,
            _ => return false,
        };
        // Exactly one arg, and it must be a literal or a simple ident.
        // Zero args: no value to pack into ctx. Two+ args: ctx is only 8 bytes (one i64).
        call.args.len() == 1
            && call
                .args
                .iter()
                .all(|arg| matches!(arg, Expr::IntLit(_, _) | Expr::Ident(_, _)))
    };
    if !args_lowerable(&stmts[first_idx]) || !args_lowerable(&stmts[second_idx]) {
        return None;
    }

    // Data-dependency check: if the first statement binds a name (let a = f(...))
    // and the second call's argument list references that name, the two calls are not
    // independent — the second depends on the result of the first. Spawning them in
    // parallel would evaluate the second's arg at spawn time before `a` is bound.
    // In that case, decline the group and run both sequentially.
    let first_bind_name: Option<&str> = match &stmts[first_idx] {
        Stmt::Let { name, .. } => Some(name.as_str()),
        _ => None,
    };
    if let Some(bind_name) = first_bind_name {
        let second_uses_first = match &stmts[second_idx] {
            Stmt::Let {
                value: Expr::Call(c),
                ..
            }
            | Stmt::Expr(Expr::Call(c)) => c
                .args
                .iter()
                .any(|arg| matches!(arg, Expr::Ident(n, _) if n.as_str() == bind_name)),
            _ => false,
        };
        if second_uses_first {
            return None;
        }
    }

    // Collect bind names for the pair to check result-assignment in post stmts.
    let mut bind_names: Vec<String> = Vec::new();
    for idx in [first_idx, second_idx] {
        if let Stmt::Let { name, .. } = &stmts[idx] {
            bind_names.push(name.clone());
        }
    }

    // Decline when any post statement assigns a CPU-result bind name at any depth.
    // The same guard lives in spike_cpu_candidates and spike_cpu_group_result_names;
    // all three gates must agree so frame-slot allocation and extraction are consistent.
    let post_stmts: Vec<&Stmt> = stmts[(second_idx + 1)..].iter().collect();
    for bind_name in &bind_names {
        if post_stmts
            .iter()
            .any(|s| stmt_assigns_name(s, bind_name.as_str()))
        {
            return None;
        }
    }

    // Decline when any post-pair statement contains a suspending user-defined callee.
    // A user-defined suspending callee embeds its child sub-frame at the pre-spike frame
    // offset (computed before the 48-byte spike reserve was added). For a spike host with
    // 0 own locals that offset equals SPIKE_RESULT_0_OFFSET (byte 48), so the callee's
    // resume_point write aliases the joined result. Intrinsic waits (e.g. `wait sleep(...)`)
    // use the SM inline-poll path with no embedded child sub-frame and therefore do not
    // alias — stmt_contains_suspending_call excludes them via the M2_MAY_BLOCK_INTRINSICS
    // guard. Declining routes the whole body through sequential lowering, which is always
    // correct.
    if post_stmts
        .iter()
        .any(|s| stmt_contains_suspending_call(s, suspend_set))
    {
        return None;
    }

    let group = vec![&stmts[first_idx], &stmts[second_idx]];
    let pre_stmts: Vec<&Stmt> = stmts[..first_idx].iter().collect();
    Some((pre_stmts, group, post_stmts))
}

/// Reload CPU-group result locals from their persistent frame slots into pre-allocated sm_entry allocas.
///
/// After `emit_cpu_group_spawn_join` completes all joins, each CPU result is stored in
/// both a local alloca (for the current resume-fn invocation) AND in the persistent frame
/// slot at `SPIKE_RESULT_{0,1}_OFFSET`. When a subsequent suspension causes Tokio to call
/// resume_fn again with a fresh stack frame, the frame slot still holds the value. This
/// function reloads each remaining bound CPU result from its persistent frame slot into the
/// pre-allocated sm_entry alloca for that name, refreshing the alloca's content for the
/// current resume invocation.
///
/// **Pre-allocated allocas**: result allocas are created in the function entry block
/// (sm_entry) during Step 1c of `lower_function_with_waits`. This ensures they dominate
/// all state blocks, satisfying LLVM SSA dominance. This function NEVER calls `build_alloca`
/// — doing so in a non-entry state block would place the alloca after a conditional branch,
/// violating "Instruction does not dominate all uses" for any load in a different state.
///
/// **Called from**: `reload_params_from_frame`'s `reload_crossing: true` path (fires at
/// every suspension depth, not just the outermost one). The crossing-local loop in that
/// function skips names present in `cg.m3d_spike_cpu_result_names` because their frame
/// offsets are `SPIKE_RESULT_N_OFFSET` — distinct from SM crossing slot indices. Loading
/// from a crossing slot index for a spike name would read the wrong frame bytes.
///
/// **Edge case — mutated result**: if any post-pair statement assigns a CPU-result bind
/// name, the static admission gate declines the group entirely so that sequential
/// lowering handles the whole body. This means every name in `crossing_results` is
/// stable (never reassigned in rest_stmts), and this function is only called for groups
/// that the gate admitted.
///
/// Time: O(k) where k = crossing_results length  Space: O(1) per call
fn spike_reload_cpu_results_from_frame<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    crossing_results: &[(String, u64)],
    frame_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<(), String> {
    if crossing_results.is_empty() {
        return Ok(());
    }
    let ctx = cg.ctx;
    let i64_ty = ctx.i64_type();
    let i8_ty = ctx.i8_type();
    for (name, frame_offset) in crossing_results {
        // GEP to the byte at frame_ptr + frame_offset (SPIKE_RESULT_N_OFFSET).
        // The frame slot holds YnzCpuResult = {i64, i64}; the first i64 is the value.
        let slot_ptr = if *frame_offset == 0 {
            frame_ptr
        } else {
            unsafe {
                cg.builder
                    .build_gep(
                        i8_ty,
                        frame_ptr,
                        &[i64_ty.const_int(*frame_offset, false)],
                        &format!("{name}_reload_slot"),
                    )
                    .map_err(|e| format!("spike reload GEP `{name}`: {e}"))?
            }
        };
        // Load the i64 value from the slot (same layout as result_slot in all_done_bb).
        let reloaded = cg
            .builder
            .build_load(i64_ty, slot_ptr, &format!("{name}_reloaded"))
            .map_err(|e| format!("spike reload load `{name}`: {e}"))?;
        // Use the sm_entry alloca pre-allocated in Step 1c. LLVM SSA requires that the
        // alloca dominate all uses — a build_alloca here (in a state block) would not
        // dominate loads in other state blocks, producing "does not dominate all uses".
        let pre_alloc = cg
            .m3d_spike_cpu_result_allocas
            .get(name.as_str())
            .copied()
            .ok_or_else(|| {
                format!(
                    "spike reload: no sm_entry alloca for `{name}` — \
                     Step 1c pre-allocation and gate-2 must agree on group membership"
                )
            })?;
        cg.builder
            .build_store(pre_alloc, reloaded)
            .map_err(|e| format!("spike reload store `{name}`: {e}"))?;
        // cg.locals already points to pre_alloc from Step 1c; refreshing the store above
        // makes the alloca's content valid for this resume invocation without re-inserting.
        cg.locals.insert(name.clone(), pre_alloc);
    }
    Ok(())
}

/// Emit a CPU-parallel spawn+join group via the spike polling protocol.
///
/// # What this does
///
/// For a 2-member group `[let a = callee(arg_a), let b = callee(arg_b)]`:
///
/// 1. **Spawn state** (state `*current_state`, which is 0 for a pure-spike function):
///    - Build a trampoline function per callee invocation (`ptr → YnzCpuResult`)
///      that unpacks the i64 arg from a ctx copy, calls the compiled callee, packs result.
///    - `ynz_rt_spawn_blocking_joinable(trampoline, ctx_ptr, ctx_size)` → handle ptr
///    - Store each handle ptr in a dedicated frame slot (SPIKE_HANDLE_0/1_OFFSET).
///    - Store resume_point = next_state (the poll state), then branch to pending_block.
///
/// 2. **Poll state** (state `*current_state + 1`, entered on re-entry):
///    - `ynz_rt_join_poll(handle_0, waker_ctx, &result_0)` → 0=Ready, 1=Pending
///    - `ynz_rt_join_poll(handle_1, waker_ctx, &result_1)` → 0=Ready, 1=Pending
///    - If any Pending: store resume_point = poll_state again, branch to pending_block.
///    - If all Ready: load result i64 from each result slot, store into local allocas.
///
/// # Frame slot usage (spike-only, not part of the canonical frame layout system)
///
/// - `SPIKE_HANDLE_0/1_OFFSET`: stores the `*mut u8` handle pointer (8 bytes each)
/// - `SPIKE_RESULT_0/1_OFFSET`: stores the `YnzCpuResult = [i64;2]` (16 bytes each)
///
/// # Edge cases
///
/// - **Null handles**: `ynz_rt_join_poll` panics on a null handle (codegen bug guard). In the
///   spike this cannot happen: handles are stored unconditionally at spawn time and polled
///   exactly once in the poll state.
/// - **Cancellation**: if the parent SM is cancelled mid-poll (Tokio drops the future before
///   the all-Ready branch fires), `SpawnStateFnFuture::drop` reads `SPIKE_FRAME_MAGIC` at
///   frame offset 4 and frees any non-null handle slots — no resource leak.
/// - **Same-callee vs distinct-callee**: the trampoline is built per invocation (using
///   `call_idx` for name disambiguation), so two `fib(10)` + `fib(11)` calls work correctly
///   even though they have the same callee.
///
/// # Non-goals
///
/// This is spike code. It hardwires 2-member groups, uses fixed frame offsets, and bypasses
/// the canonical frame-layout system. Production codegen (P3) will use proper frame slots
/// and support N members.
///
/// Time: O(1) LLVM instructions emitted (fixed 2-member protocol)  Space: O(1)
#[allow(clippy::too_many_arguments)]
fn emit_cpu_group_spawn_join<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    stmts: &[&Stmt],
    f: &FunctionDecl,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    current_state: &mut usize,
) -> Result<Vec<(String, u64)>, String> {
    // Returns: Vec<(bind_name, frame_result_offset)> for each CPU-group member that bound
    // its result to a local name. The caller uses this to reload the results from their
    // persistent frame slots after any subsequent suspension (where the local allocas
    // created in this invocation's all_done_bb would otherwise be unreachable).
    let ctx = cg.ctx;

    // --- Collect call info for the two group members ---

    struct CpuChild {
        /// Name to bind the result to (from `let name = callee(arg)`), or None for bare call.
        bind_name: Option<String>,
        /// Callee function name.
        callee: String,
        /// Argument expressions (should be exactly one integer literal or ident).
        args: Vec<Expr>,
    }

    let mut children: Vec<CpuChild> = Vec::with_capacity(stmts.len());
    for &stmt in stmts {
        match stmt {
            Stmt::Let {
                name,
                value: Expr::Call(c),
                ..
            } => {
                if let Expr::Ident(callee_name, _) = &c.callee {
                    children.push(CpuChild {
                        bind_name: Some(name.clone()),
                        callee: callee_name.clone(),
                        args: c.args.clone(),
                    });
                } else {
                    return Err("spike cpu group: non-ident callee in let stmt".to_string());
                }
            }
            Stmt::Expr(Expr::Call(c)) => {
                if let Expr::Ident(callee_name, _) = &c.callee {
                    children.push(CpuChild {
                        bind_name: None,
                        callee: callee_name.clone(),
                        args: c.args.clone(),
                    });
                } else {
                    return Err("spike cpu group: non-ident callee in expr stmt".to_string());
                }
            }
            _ => return Err("spike cpu group: stmt is not a direct call".to_string()),
        }
    }

    if children.len() != 2 {
        return Err(format!(
            "spike cpu group: expected 2 members, got {}",
            children.len()
        ));
    }

    // Read per-member handle/result byte offsets from the composed frame layout
    // (`build_frame_layouts` computed them, keyed by member index — group 0). The layout is
    // the single source of truth: the reserve, the size math, and these offsets all derive
    // from `build_cpu_group_slots`, so they cannot drift apart. The fallback to the
    // SPIKE_*_OFFSET constants only fires if no layout entry exists, which cannot happen for
    // a promoted (in-suspend-set) function — kept defensive so a future refactor that drops
    // the layout entry fails loud at the join, not silently mis-offset.
    let (handle_offsets, result_offsets): ([u64; 2], [u64; 2]) = cg
        .frame_layouts
        .get(&f.name)
        .filter(|l| l.cpu_group_slots.len() >= 2)
        .map(|l| {
            let s = &l.cpu_group_slots;
            (
                [s[0].handle_offset, s[1].handle_offset],
                [s[0].result_offset, s[1].result_offset],
            )
        })
        .unwrap_or((
            [SPIKE_HANDLE_0_OFFSET, SPIKE_HANDLE_1_OFFSET],
            [SPIKE_RESULT_0_OFFSET, SPIKE_RESULT_1_OFFSET],
        ));

    // --- Build trampolines ---
    //
    // Each trampoline has signature `ptr → [i64 × 2]` (i.e. `{i64,i64}` struct
    // returned by value, matching YnzCpuResult's C repr as two i64 fields).
    // The ctx layout is simply 8 bytes holding the i64 argument value.
    let i64_ty = ctx.i64_type();
    let cpu_result_ty = ctx.struct_type(&[i64_ty.into(), i64_ty.into()], false);
    let trampoline_ty =
        cpu_result_ty.fn_type(&[ctx.ptr_type(AddressSpace::default()).into()], false);

    // Helper: build one trampoline for child[idx].
    //
    // The trampoline loads the i64 arg from ctx[0..8], calls the compiled callee, and packs
    // the callee's return value into the 16-byte `YnzCpuResult` ({i64, i64}) using the SAME
    // serialization the canonical SM return slot uses (`state_machine::store_return_value_*`).
    // The join-side bind then reads the slot back through `load_sm_return_value_typed` +
    // `bind_sm_result_and_flush`, so a CPU group binds every return class exactly as a
    // sequential call would. Packing dispatches on the callee's LLVM return value kind:
    //   - i64           (int/bool)      → field0 = value,           field1 = 0
    //   - i128          (number)        → field0 = lo, field1 = hi  (the 16 bytes ARE the i128)
    //   - f64           (float)         → field0 = bitcast→i64,      field1 = 0
    //   - ptr           (string/array/map) → field0 = ptr→i64,      field1 = 0
    //   - {i64, i64}    (`T errors`)    → field0 = error word,       field1 = success word
    let i128_ty = ctx.i128_type();
    let mut trampoline_fns: Vec<FunctionValue<'ctx>> = Vec::with_capacity(2);
    for (idx, child) in children.iter().enumerate() {
        let trampoline_name = format!("__ynz_spike_trampoline_{}_{}_{}", f.name, child.callee, idx);
        let trampoline_fn = cg
            .module
            .add_function(&trampoline_name, trampoline_ty, None);
        let tramp_entry = ctx.append_basic_block(trampoline_fn, "entry");
        let tramp_builder = ctx.create_builder();
        tramp_builder.position_at_end(tramp_entry);

        // Load the i64 arg from ctx (offset 0).
        let ctx_param = trampoline_fn
            .get_nth_param(0)
            .ok_or("trampoline: missing ctx param")?
            .into_pointer_value();
        let arg_val = tramp_builder
            .build_load(i64_ty, ctx_param, "spike_arg")
            .map_err(|e| format!("trampoline load arg {idx}: {e}"))?
            .into_int_value();

        // Call the compiled callee(arg_val).
        let callee_fn = cg
            .module
            .get_function(child.callee.as_str())
            .ok_or_else(|| format!("spike: callee `{}` not declared", child.callee))?;
        let call_result = tramp_builder
            .build_call(callee_fn, &[arg_val.into()], "spike_call")
            .map_err(|e| format!("trampoline call {idx}: {e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| format!("spike callee `{}` returned void", child.callee))?;

        // Pack the callee's return value into the 16-byte {i64, i64} result.
        let (word0, word1): (
            inkwell::values::IntValue<'ctx>,
            inkwell::values::IntValue<'ctx>,
        ) = match call_result {
            inkwell::values::BasicValueEnum::IntValue(iv) if iv.get_type() == i128_ty => {
                // number (decimal128): the 16-byte slot holds the full i128 as lo/hi.
                let lo = tramp_builder
                    .build_int_truncate(iv, i64_ty, "spike_num_lo")
                    .map_err(|e| format!("trampoline num lo {idx}: {e}"))?;
                let hi_shift = tramp_builder
                    .build_right_shift(iv, i128_ty.const_int(64, false), false, "spike_num_sh")
                    .map_err(|e| format!("trampoline num shift {idx}: {e}"))?;
                let hi = tramp_builder
                    .build_int_truncate(hi_shift, i64_ty, "spike_num_hi")
                    .map_err(|e| format!("trampoline num hi {idx}: {e}"))?;
                (lo, hi)
            }
            inkwell::values::BasicValueEnum::IntValue(iv) => {
                // int/bool: zero-extend a narrow bool to i64; an i64 passes through.
                let v = if iv.get_type() == i64_ty {
                    iv
                } else {
                    tramp_builder
                        .build_int_z_extend(iv, i64_ty, "spike_int_widen")
                        .map_err(|e| format!("trampoline int widen {idx}: {e}"))?
                };
                (v, i64_ty.const_int(0, false))
            }
            inkwell::values::BasicValueEnum::FloatValue(fv) => {
                // float: store the raw IEEE-754 bits (bitcast f64 → i64), matching
                // `store_return_value_f64` so the bind's f64-bitcast load reverses it.
                let bits = tramp_builder
                    .build_bit_cast(fv, i64_ty, "spike_f_to_i")
                    .map_err(|e| format!("trampoline float bitcast {idx}: {e}"))?
                    .into_int_value();
                (bits, i64_ty.const_int(0, false))
            }
            inkwell::values::BasicValueEnum::PointerValue(pv) => {
                if callee_returns_bare_number(cg.typed, cg.imported_fns, &child.callee) {
                    // number (decimal128): the non-SM ABI returns a POINTER to a
                    // heap-stable 16-byte i128. Dereference it and pack lo/hi so the
                    // result slot holds the raw i128 the join-side i128 load expects.
                    let i128_val = tramp_builder
                        .build_load(i128_ty, pv, "spike_num_load")
                        .map_err(|e| format!("trampoline num load {idx}: {e}"))?
                        .into_int_value();
                    let lo = tramp_builder
                        .build_int_truncate(i128_val, i64_ty, "spike_num_lo")
                        .map_err(|e| format!("trampoline num lo {idx}: {e}"))?;
                    let hi_shift = tramp_builder
                        .build_right_shift(
                            i128_val,
                            i128_ty.const_int(64, false),
                            false,
                            "spike_num_sh",
                        )
                        .map_err(|e| format!("trampoline num shift {idx}: {e}"))?;
                    let hi = tramp_builder
                        .build_int_truncate(hi_shift, i64_ty, "spike_num_hi")
                        .map_err(|e| format!("trampoline num hi {idx}: {e}"))?;
                    (lo, hi)
                } else {
                    // string/array/map: the returned heap pointer IS the value. Store it
                    // as i64 (ptr_to_int); the heap block outlives the blocking-pool task,
                    // so the parent reads it post-join.
                    let bits = tramp_builder
                        .build_ptr_to_int(pv, i64_ty, "spike_ptr_to_i")
                        .map_err(|e| format!("trampoline ptr_to_int {idx}: {e}"))?;
                    (bits, i64_ty.const_int(0, false))
                }
            }
            inkwell::values::BasicValueEnum::StructValue(sv) => {
                // `T errors`: {i64 error word, i64 success word}. Both words must reach
                // the result slot — dropping field0 would turn an error into a success.
                let err = tramp_builder
                    .build_extract_value(sv, 0, "spike_ec_err")
                    .map_err(|e| format!("trampoline ec err {idx}: {e}"))?
                    .into_int_value();
                let ok = tramp_builder
                    .build_extract_value(sv, 1, "spike_ec_ok")
                    .map_err(|e| format!("trampoline ec ok {idx}: {e}"))?
                    .into_int_value();
                (err, ok)
            }
            other => {
                return Err(format!(
                    "spike trampoline: unsupported callee `{}` return value {other:?}",
                    child.callee
                ))
            }
        };

        let packed = cpu_result_ty.const_zero();
        let packed = tramp_builder
            .build_insert_value(packed, word0, 0, "spike_pack_w0")
            .map_err(|e| format!("trampoline insert w0 {idx}: {e}"))?
            .into_struct_value();
        let packed = tramp_builder
            .build_insert_value(packed, word1, 1, "spike_pack_w1")
            .map_err(|e| format!("trampoline insert w1 {idx}: {e}"))?
            .into_struct_value();

        tramp_builder
            .build_return(Some(&packed))
            .map_err(|e| format!("trampoline ret {idx}: {e}"))?;

        trampoline_fns.push(trampoline_fn);
    }

    // --- Spawn state (state_blocks[*current_state]) ---

    let spawn_state = state_blocks[*current_state];
    // Poll state is the next pre-allocated state block.
    let poll_state_idx = *current_state + 1;
    let poll_state = state_blocks
        .get(poll_state_idx)
        .copied()
        .ok_or_else(|| format!("spike: no poll state block at index {poll_state_idx}"))?;

    cg.builder.position_at_end(spawn_state);

    // Write spike frame discriminator to bytes 4-7 of the parent frame.
    // Normal (non-spike) SM frames leave bytes 4-7 as zero (ynz_alloc_zeroed guarantee).
    // SpawnStateFnFuture::drop reads this magic to decide whether to free the CPU handle
    // slots at offsets 32/40 on cancellation. The write happens once at spawn time; it
    // survives across resume_fn invocations because it is in the persistent heap frame.
    {
        let i8_ty = ctx.i8_type();
        let i32_ty = ctx.i32_type();
        // SPIKE_FRAME_MAGIC = 0x5350_494B ("SPIK")
        const SPIKE_FRAME_MAGIC: u32 = 0x5350_494B;
        // Offset 4 is within the frame header; use a byte GEP then cast to i32*.
        let disc_byte_ptr = unsafe {
            cg.builder
                .build_gep(
                    i8_ty,
                    frame_ptr,
                    &[i64_ty.const_int(4, false)],
                    "spike_disc_ptr",
                )
                .map_err(|e| format!("spike disc GEP: {e}"))?
        };
        cg.builder
            .build_store(
                disc_byte_ptr,
                i32_ty.const_int(SPIKE_FRAME_MAGIC as u64, false),
            )
            .map_err(|e| format!("spike disc store: {e}"))?;
    }

    // Helper: get a byte ptr into the frame at a fixed offset.
    let frame_byte_ptr = |offset: u64, name: &str| -> Result<PointerValue<'ctx>, String> {
        if offset == 0 {
            return Ok(frame_ptr);
        }
        unsafe {
            cg.builder
                .build_gep(
                    ctx.i8_type(),
                    frame_ptr,
                    &[i64_ty.const_int(offset, false)],
                    name,
                )
                .map_err(|e| format!("spike frame GEP {name}: {e}"))
        }
    };

    // For each child: allocate a 8-byte ctx on the stack, write the arg, spawn.
    // `handle_offsets` is read from the composed frame layout (computed above).
    for (idx, child) in children.iter().enumerate() {
        // Evaluate the argument expression (must be a simple integer — ident or literal).
        let arg_llvm = match child.args.first() {
            Some(Expr::IntLit(n, _)) => i64_ty.const_int(*n as u64, true),
            Some(Expr::Ident(name, span)) => {
                // Load from local alloca.
                let local_ptr = cg
                    .locals
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| format!("spike: local `{name}` not found at {:?}", span))?;
                cg.builder
                    .build_load(i64_ty, local_ptr, &format!("spike_arg_{idx}"))
                    .map_err(|e| format!("spike load arg {idx}: {e}"))?
                    .into_int_value()
            }
            Some(other) => {
                return Err(format!(
                    "spike: unsupported arg expr for child {idx}: {other:?}"
                ))
            }
            None => return Err(format!("spike: child {idx} has no arguments")),
        };

        // Allocate an 8-byte ctx on the stack.
        let ctx_alloca = cg
            .builder
            .build_alloca(i64_ty, &format!("spike_ctx_{idx}"))
            .map_err(|e| format!("spike ctx alloca {idx}: {e}"))?;
        cg.builder
            .build_store(ctx_alloca, arg_llvm)
            .map_err(|e| format!("spike ctx store {idx}: {e}"))?;

        // Call ynz_rt_spawn_blocking_joinable(trampoline_ptr, ctx_ptr, ctx_size=8).
        let handle = cg
            .builder
            .build_call(
                cg.rt.ynz_rt_spawn_blocking_joinable,
                &[
                    trampoline_fns[idx]
                        .as_global_value()
                        .as_pointer_value()
                        .into(),
                    ctx_alloca.into(),
                    i64_ty.const_int(8, false).into(),
                ],
                &format!("spike_handle_{idx}"),
            )
            .map_err(|e| format!("spike spawn {idx}: {e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| format!("spike spawn {idx}: expected ptr return"))?
            .into_pointer_value();

        // Store handle into frame slot.
        let handle_slot = frame_byte_ptr(handle_offsets[idx], &format!("spike_hslot_{idx}"))?;
        cg.builder
            .build_store(handle_slot, handle)
            .map_err(|e| format!("spike handle store {idx}: {e}"))?;
    }

    // Save resume_point = poll_state_idx so that on any subsequent re-entry the SM
    // dispatch switch lands in poll_state (not back here in spawn_state).
    state_machine::store_resume_point(ctx, &cg.builder, frame_ptr, poll_state_idx as u64)?;
    // Branch to poll_state immediately — the first poll of both handles registers
    // the waker with each JoinHandle so the runtime can wake the SM when tasks finish.
    // Branching directly to pending_block here would skip the first poll, leaving
    // the waker unregistered and hanging the SM forever (the bug this fixes).
    cg.builder
        .build_unconditional_branch(poll_state)
        .map_err(|e| format!("spike spawn-to-poll branch: {e}"))?;

    // --- Poll state (state_blocks[poll_state_idx]) ---

    cg.builder.position_at_end(poll_state);

    // Reset "any still pending?" accumulator at the top of every poll-state entry.
    // The alloca sits in poll_state (a non-entry block), which is valid because each call
    // to the resume_fn gets a fresh stack frame — poll_state is only ever entered from the
    // SM dispatch switch in sm_entry, so the alloca always dominates its uses in this
    // invocation. OptimizationLevel::None means mem2reg does not run; the alloca stays
    // as a stack slot, not an SSA value, so LLVM does not require entry-block placement
    // for correctness here.
    let any_pending = cg
        .builder
        .build_alloca(ctx.i32_type(), "spike_any_pending")
        .map_err(|e| format!("spike any_pending alloca: {e}"))?;
    cg.builder
        .build_store(any_pending, ctx.i32_type().const_int(0, false))
        .map_err(|e| format!("spike any_pending init: {e}"))?;

    let ptr_ty = ctx.ptr_type(AddressSpace::default());
    // `result_offsets` is read from the composed frame layout (computed above).
    for idx in 0..2usize {
        // Load handle from frame slot.
        let handle_slot = frame_byte_ptr(handle_offsets[idx], &format!("spike_hslot_re_{idx}"))?;
        let handle = cg
            .builder
            .build_load(ptr_ty, handle_slot, &format!("spike_handle_re_{idx}"))
            .map_err(|e| format!("spike handle load {idx}: {e}"))?
            .into_pointer_value();

        // If this child was already Ready on a prior re-poll, the handle slot was nulled.
        // Skip polling it again to avoid a UAF on the already-freed JoinHandle box.
        let is_null = cg
            .builder
            .build_is_null(handle, &format!("spike_is_null_{idx}"))
            .map_err(|e| format!("spike null check {idx}: {e}"))?;

        let skip_bb = ctx.append_basic_block(cg.current_fn, &format!("spike_skip_{idx}"));
        let poll_bb = ctx.append_basic_block(cg.current_fn, &format!("spike_dopoll_{idx}"));
        let next_bb = ctx.append_basic_block(cg.current_fn, &format!("spike_next_{idx}"));

        cg.builder
            .build_conditional_branch(is_null, skip_bb, poll_bb)
            .map_err(|e| format!("spike null branch {idx}: {e}"))?;

        // poll_bb: call ynz_rt_join_poll.
        cg.builder.position_at_end(poll_bb);
        let result_slot = frame_byte_ptr(result_offsets[idx], &format!("spike_rslot_{idx}"))?;
        let poll_result = cg
            .builder
            .build_call(
                cg.rt.ynz_rt_join_poll,
                &[handle.into(), waker_ctx.into(), result_slot.into()],
                &format!("spike_poll_{idx}"),
            )
            .map_err(|e| format!("spike poll {idx}: {e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| format!("spike poll {idx}: expected i32 return"))?
            .into_int_value();

        // is_pending = (poll_result != 0)
        let is_pending = cg
            .builder
            .build_int_compare(
                IntPredicate::NE,
                poll_result,
                ctx.i32_type().const_int(0, false),
                &format!("spike_ispend_{idx}"),
            )
            .map_err(|e| format!("spike poll cmp {idx}: {e}"))?;

        let pend_bb = ctx.append_basic_block(cg.current_fn, &format!("spike_pend_{idx}"));
        let ready_bb = ctx.append_basic_block(cg.current_fn, &format!("spike_ready_{idx}"));
        cg.builder
            .build_conditional_branch(is_pending, pend_bb, ready_bb)
            .map_err(|e| format!("spike poll branch {idx}: {e}"))?;

        // pend_bb: set accumulator, continue to next_bb.
        cg.builder.position_at_end(pend_bb);
        cg.builder
            .build_store(any_pending, ctx.i32_type().const_int(1, false))
            .map_err(|e| format!("spike pend store {idx}: {e}"))?;
        cg.builder
            .build_unconditional_branch(next_bb)
            .map_err(|e| format!("spike pend cont {idx}: {e}"))?;

        // ready_bb: null the handle slot (ynz_rt_join_poll already freed the box).
        // Nulling prevents a UAF if this child finishes before its sibling and
        // poll_state is re-entered for the still-pending sibling.
        cg.builder.position_at_end(ready_bb);
        cg.builder
            .build_store(handle_slot, ptr_ty.const_null())
            .map_err(|e| format!("spike null slot {idx}: {e}"))?;
        cg.builder
            .build_unconditional_branch(next_bb)
            .map_err(|e| format!("spike ready cont {idx}: {e}"))?;

        // skip_bb: handle was already null (child done on a prior re-poll), nothing to do.
        cg.builder.position_at_end(skip_bb);
        cg.builder
            .build_unconditional_branch(next_bb)
            .map_err(|e| format!("spike skip cont {idx}: {e}"))?;

        cg.builder.position_at_end(next_bb);
    }

    // Check accumulator: if any pending, re-save state and yield.
    let any_val = cg
        .builder
        .build_load(ctx.i32_type(), any_pending, "spike_any_val")
        .map_err(|e| format!("spike any_val load: {e}"))?
        .into_int_value();
    let had_pending = cg
        .builder
        .build_int_compare(
            IntPredicate::NE,
            any_val,
            ctx.i32_type().const_int(0, false),
            "spike_had_pending",
        )
        .map_err(|e| format!("spike had_pending cmp: {e}"))?;

    let all_done_bb = ctx.append_basic_block(cg.current_fn, "spike_all_done");
    cg.builder
        .build_conditional_branch(had_pending, pending_block, all_done_bb)
        .map_err(|e| format!("spike final branch: {e}"))?;

    // resume_point was set to poll_state_idx at spawn time and is left unchanged on
    // Pending — on every re-entry the SM dispatch switch sees poll_state_idx and lands
    // here to re-poll. Branching directly to pending_block is correct: the frame already
    // holds the right state index.

    // all_done_bb: all children are Ready; read results and bind locals.
    cg.builder.position_at_end(all_done_bb);

    for (idx, child) in children.iter().enumerate() {
        let Some(bind_name) = &child.bind_name else {
            continue;
        };

        // The 16-byte result slot is laid out exactly like a canonical SM return slot (which
        // lives at `FRAME_OFFSET_RETURN_SLOT` = 16 within its frame). Synthesize a frame
        // pointer = `result_slot - 16` so `load_sm_return_value_typed` (which reads at +16)
        // reads the result slot, decoding the callee's return class the same way the
        // sequential SM path does — int/bool i64, float f64-bits, number i128 lo/hi, pointer
        // classes, and the `{i64,i64}` errors pair. `result_offset` is always ≥ 48 (handle
        // region precedes it), so subtracting the 16-byte return-slot offset stays inside the
        // frame header region and never GEPs below the frame base (negative offset).
        let synth_offset = result_offsets[idx] - state_machine::FRAME_OFFSET_RETURN_SLOT;
        // Inline GEP (not the `frame_byte_ptr` closure) so no immutable `cg` borrow lingers
        // into the `&mut cg` binder calls below. synth_offset ≥ 32, so it is never 0.
        let synth_frame = unsafe {
            cg.builder
                .build_gep(
                    ctx.i8_type(),
                    frame_ptr,
                    &[i64_ty.const_int(synth_offset, false)],
                    &format!("spike_synth_frame_{idx}"),
                )
                .map_err(|e| format!("spike synth frame GEP {idx}: {e}"))?
        };
        let ret_val = load_sm_return_value_typed(
            cg,
            ctx,
            synth_frame,
            &child.callee,
            &format!("spike_ret_{idx}"),
        )?;

        // Bind through the same unified binder the sequential and I/O-parallel SM let-paths
        // use (`bind_sm_result_and_flush`) — no CPU-only store that could drift from the
        // canonical per-class discipline (corpse guard (a)). For a non-crossing result (read
        // before any subsequent suspension — every spike-firing fixture in this slice) it
        // creates a fresh, correctly-typed alloca; for a crossing result it reuses the
        // dominating entry-block alloca and flushes to its frame slot(s).
        let alloca = bind_sm_result_and_flush(cg, bind_name, ret_val, frame_ptr, &child.callee)?;
        cg.locals.insert(bind_name.clone(), alloca);

        // `T errors` results must be tracked so a later `.or(...)` / propagation extracts the
        // success word instead of dereferencing the companion-struct pointer — mirrors the
        // sequential SM let-arms and the I/O-parallel join bind.
        if cg
            .sm_crossing_errors_capable_set
            .contains(bind_name.as_str())
            || is_errors_capable_fn(cg.typed, cg.imported_fns, &child.callee)
        {
            cg.errors_capable_locals.insert(bind_name.clone());
        }
    }

    // Build the caller-visible list of (name, frame_offset) for result-crossing reload.
    // The frame result slots are persistent across suspension — the callee uses them to
    // reload bound names into fresh allocas after any subsequent suspension in rest_stmts.
    let crossing_results: Vec<(String, u64)> = children
        .iter()
        .enumerate()
        .filter_map(|(idx, child)| {
            child
                .bind_name
                .as_ref()
                .map(|name| (name.clone(), result_offsets[idx]))
        })
        .collect();

    *current_state += 2;
    Ok(crossing_results)
}

/// Emit interleaved inline-poll for a parallel group of ≥2 independent suspending statements.
///
/// # Mechanism (no spawn — corpse (a) compliance)
///
/// Each member's child frame is already embedded in the composed parent frame (allocated at
/// `lower_function_with_waits` time). This function:
/// 1. Initializes all child frame headers (resume_point=0, sleep_handle=null).
/// 2. Writes call arguments to each child frame's local slots.
/// 3. Polls all child frames in order. Any Pending child suspends; we track which are done.
/// 4. If any child is Pending, save parent resume_point = continuation_state, yield Pending.
/// 5. On re-entry (continuation_state): re-poll any child that was not yet Ready.
/// 6. When ALL children are Ready, fall through to post_call_bb.
/// 7. Reads results from each child frame and binds to the let-target if applicable.
///
/// # State budget
///
/// A parallel group of N stmts uses 1 shared continuation state. The other N-1 states
/// pre-allocated by `count_suspension_points` are left unterminated; `lower_sm_body`'s
/// trailing loop adds an unreachable terminator to any unterminated block so LLVM is valid.
///
/// # Corpse guards
///
/// - (a) No forked frame dispatch: all frame slot I/O routes through `flush_var_slot_to_frame`
///   and `reload_params_from_frame`. Return-value reads use `load_sm_return_value_typed`.
/// - (b) No flat-scan re-derivation: this function receives the pre-partitioned group from
///   `partition_independent_groups`; it does not re-examine statement order.
///
/// # EC-returning parallel calls
///
/// `-> T errors` (EC) calls DO parallelize through this function: the `{i64,i64}`
/// companion-struct result is read via `load_sm_return_value_typed`'s `ErrorsCapable` arm
/// and bound through the unified `bind_sm_result_and_flush` (StructValue arm), so an EC
/// return survives a later `wait` barrier byte-identically. The still-deferred piece is
/// collecting a `background`-spawned EC task's result via a handle
/// (`ec-wrapper-collect-on-completion`, gated on `background-handle-form`, v0.3-M4) — a
/// separate path that does not flow through this inline-poll function.
///
/// # Failure modes
///
/// Returns `Err` propagated from any LLVM builder call or a missing child frame/resume fn.
/// A missing child frame layout or missing resume fn is always an `Err` (codegen bug).
#[allow(clippy::too_many_arguments)]
fn emit_independent_group_poll<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    stmts: &[&Stmt],
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    parent_frame: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<(), String> {
    let ctx = cg.ctx;
    // N ≥ 2 by construction (independence.rs only emits Parallel groups with 2+ members).

    // --- Step 1 — collect callee info and child frame pointers ---

    struct Child<'ctx> {
        callee_name: String,
        child_frame: PointerValue<'ctx>,
        resume_fn: inkwell::values::FunctionValue<'ctx>,
    }

    let mut children: Vec<Child<'ctx>> = Vec::with_capacity(stmts.len());

    for &stmt in stmts {
        // Extract (callee_name, call_args) from the statement.
        let (callee_name, call_args) = extract_call_and_args(stmt).ok_or_else(|| {
            "emit_independent_group_poll: stmt is not a direct ident call".to_string()
        })?;

        // Find child frame offset from parent's layout.
        let child_offset = cg
            .frame_layouts
            .get(&f.name)
            .and_then(|layout| {
                layout
                    .children
                    .iter()
                    .find(|(n, _)| n == &callee_name)
                    .map(|(_, off)| *off)
            })
            .ok_or_else(|| {
                format!(
                    "emit_independent_group_poll: no child frame slot for `{callee_name}` in `{}`",
                    f.name
                )
            })?;

        let child_frame = state_machine::child_frame_ptr(
            ctx,
            &cg.builder,
            parent_frame,
            child_offset,
            &format!("par_cf_{callee_name}"),
        )?;

        // Resolve resume function (same alias logic as emit_suspending_call_inline_poll).
        let callee_llvm_name = cg
            .imported_fns
            .get(callee_name.as_str())
            .and_then(|sig| sig.original_name.as_deref())
            .unwrap_or(callee_name.as_str());
        let resume_name = state_machine::resume_fn_name(callee_llvm_name);
        let resume_fn = cg.module.get_function(&resume_name).ok_or_else(|| {
            format!(
                "emit_independent_group_poll: resume fn `{resume_name}` not declared for `{callee_name}`"
            )
        })?;

        // Initialize child frame header: resume_point=0, sleep_handle=null.
        state_machine::store_resume_point(ctx, &cg.builder, child_frame, 0)?;
        let null_ptr = ctx.ptr_type(AddressSpace::default()).const_null();
        state_machine::store_sleep_handle(ctx, &cg.builder, child_frame, null_ptr)?;

        // Evaluate args and write to child frame local slots BEFORE any poll.
        let child_frame_layout = cg.frame_layouts.get(&callee_name);
        let child_n_locals = child_frame_layout
            .map(|l| l.n_locals)
            .unwrap_or(call_args.len());
        for (idx, arg) in call_args.iter().enumerate().take(child_n_locals) {
            let arg_val = lower_expr(cg, arg)?;
            let arg_ty = cg.expr_type(arg);
            let bits = cg
                .to_i64_bits(arg_val, &arg_ty)
                .map_err(|e| format!("par group arg bits: {e}"))?;
            state_machine::store_local_slot(ctx, &cg.builder, child_frame, idx, bits)?;
        }

        children.push(Child {
            callee_name,
            child_frame,
            resume_fn,
        });
    }

    // --- Step 2 — allocate the shared continuation state ---

    let continuation_state = *current_state + 1;
    let cont_state_bb = state_blocks
        .get(continuation_state)
        .copied()
        .ok_or_else(|| {
            format!(
                "parallel group cont state {continuation_state} out of range (n_states={})",
                state_blocks.len()
            )
        })?;
    let post_call_bb = ctx.append_basic_block(cg.current_fn, "par_post");
    // Consumed N slots from state_blocks; only 1 is used. The rest (N-1) will be terminated
    // as unreachable by lower_sm_body's trailing loop.
    *current_state = continuation_state + stmts.len() - 1;

    // --- Step 3 — first-poll pass: poll ALL children before deciding to yield ---
    //
    // All N children are polled in declaration order on the first pass. This is the
    // fan-out step: every child's I/O operation (e.g. sleep timer) starts before we
    // yield. Without this, a child polled after the first Pending result would never
    // start its I/O and the operations would run sequentially instead of overlapping.
    //
    // After all polls:
    //   - Any child that returned Ready gets the sentinel (0x7FFFFFFF) stored into its
    //     resume_point, so re-poll passes route to sm_dead and return 0 safely.
    //   - If ANY child was Pending, we yield with resume_point = continuation_state.
    //   - If ALL were Ready, we jump directly to post_call_bb.
    //
    // An alloca (`par_any_pending`) acts as the "was any child Pending?" accumulator.
    // It is initialized to 0; each child that is still Pending stores 1 into it.

    let any_pending_alloca = cg
        .builder
        .build_alloca(ctx.i32_type(), "par_any_pending")
        .map_err(|e| format!("par any_pending alloca: {e}"))?;
    cg.builder
        .build_store(any_pending_alloca, ctx.i32_type().const_int(0, false))
        .map_err(|e| format!("par any_pending init: {e}"))?;

    let suspend_bb = ctx.append_basic_block(cg.current_fn, "par_suspend");

    for child in &children {
        let first_poll = cg
            .builder
            .build_call(
                child.resume_fn,
                &[child.child_frame.into(), waker_ctx.into()],
                &format!("par_poll1_{}", child.callee_name),
            )
            .map_err(|e| format!("par first poll {}: {e}", child.callee_name))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| format!("par resume fn {} returned void", child.callee_name))?
            .into_int_value();

        let is_pending = cg
            .builder
            .build_int_compare(
                IntPredicate::NE,
                first_poll,
                ctx.i32_type().const_int(0, false),
                &format!("par_pend1_{}", child.callee_name),
            )
            .map_err(|e| format!("par first cmp {}: {e}", child.callee_name))?;

        // Two paths: Pending → set accumulator flag and continue; Ready → mark sentinel and continue.
        // Both paths continue to the next child poll (no early exit).
        let pend_bb =
            ctx.append_basic_block(cg.current_fn, &format!("par_pend1_{}", child.callee_name));
        let ready_bb =
            ctx.append_basic_block(cg.current_fn, &format!("par_next1_{}", child.callee_name));
        let after_bb =
            ctx.append_basic_block(cg.current_fn, &format!("par_after1_{}", child.callee_name));

        cg.builder
            .build_conditional_branch(is_pending, pend_bb, ready_bb)
            .map_err(|e| format!("par first branch {}: {e}", child.callee_name))?;

        // pend_bb: set the accumulator; continue to after_bb.
        cg.builder.position_at_end(pend_bb);
        cg.builder
            .build_store(any_pending_alloca, ctx.i32_type().const_int(1, false))
            .map_err(|e| format!("par pend store {}: {e}", child.callee_name))?;
        cg.builder
            .build_unconditional_branch(after_bb)
            .map_err(|e| format!("par pend cont {}: {e}", child.callee_name))?;

        // ready_bb: mark child done (sentinel) so re-poll passes skip it safely; continue.
        cg.builder.position_at_end(ready_bb);
        // Sentinel routes to sm_dead (default switch arm) on any subsequent re-poll,
        // preventing a null sleep_handle dereference after the handle was freed on Ready.
        state_machine::store_resume_point(ctx, &cg.builder, child.child_frame, 0x7FFF_FFFFu64)?;
        cg.builder
            .build_unconditional_branch(after_bb)
            .map_err(|e| format!("par ready cont {}: {e}", child.callee_name))?;

        cg.builder.position_at_end(after_bb);
    }

    // After all first polls: check the accumulator.
    let any_pending_val = cg
        .builder
        .build_load(ctx.i32_type(), any_pending_alloca, "par_any_pending_val")
        .map_err(|e| format!("par any_pending load: {e}"))?
        .into_int_value();
    let had_pending = cg
        .builder
        .build_int_compare(
            IntPredicate::NE,
            any_pending_val,
            ctx.i32_type().const_int(0, false),
            "par_had_pending",
        )
        .map_err(|e| format!("par had_pending cmp: {e}"))?;
    cg.builder
        .build_conditional_branch(had_pending, suspend_bb, post_call_bb)
        .map_err(|e| format!("par first final branch: {e}"))?;

    // --- suspend_bb: at least one child was Pending — save state, yield ---
    cg.builder.position_at_end(suspend_bb);
    state_machine::store_resume_point(ctx, &cg.builder, parent_frame, continuation_state as u64)?;
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("par suspend branch: {e}"))?;

    // --- cont_state_bb: re-entry — re-poll ALL children (fan-out before yield) ---
    //
    // Same fan-out discipline as the first-poll pass: poll every child and accumulate
    // "any still Pending?" before deciding to yield or proceed. Children whose first-poll
    // stored a sentinel return 0 from sm_dead immediately (safe, correct). Children that
    // still have a live sleep timer return 1 and re-register the parent waker for the
    // next wake-up. After polling all children, yield if any still Pending; otherwise post.
    cg.builder.position_at_end(cont_state_bb);
    reload_params_from_frame(cg, parent_frame, param_names, f, shape_table, true)?;

    let re_suspend_bb = ctx.append_basic_block(cg.current_fn, "par_re_suspend");
    let re_post_bb = ctx.append_basic_block(cg.current_fn, "par_re_post");

    // Accumulator: 0 = all ready so far, 1 = at least one still Pending.
    let re_any_pending_alloca = cg
        .builder
        .build_alloca(ctx.i32_type(), "par_re_any_pending")
        .map_err(|e| format!("par re_any_pending alloca: {e}"))?;
    cg.builder
        .build_store(re_any_pending_alloca, ctx.i32_type().const_int(0, false))
        .map_err(|e| format!("par re_any_pending init: {e}"))?;

    for child in &children {
        // Recompute child frame pointer (GEP must be recomputed in each basic block).
        let child_offset = cg
            .frame_layouts
            .get(&f.name)
            .and_then(|layout| {
                layout
                    .children
                    .iter()
                    .find(|(n, _)| n == &child.callee_name)
                    .map(|(_, off)| *off)
            })
            .ok_or_else(|| {
                format!(
                    "emit_independent_group_poll re: no child frame slot for `{}`",
                    child.callee_name
                )
            })?;

        let child_frame_re = state_machine::child_frame_ptr(
            ctx,
            &cg.builder,
            parent_frame,
            child_offset,
            &format!("par_cf_{}_re", child.callee_name),
        )?;

        let re_poll = cg
            .builder
            .build_call(
                child.resume_fn,
                &[child_frame_re.into(), waker_ctx.into()],
                &format!("par_poll_re_{}", child.callee_name),
            )
            .map_err(|e| format!("par re-poll {}: {e}", child.callee_name))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| format!("par resume fn {} (re) returned void", child.callee_name))?
            .into_int_value();

        let is_pending_re = cg
            .builder
            .build_int_compare(
                IntPredicate::NE,
                re_poll,
                ctx.i32_type().const_int(0, false),
                &format!("par_pend_re_{}", child.callee_name),
            )
            .map_err(|e| format!("par re cmp {}: {e}", child.callee_name))?;

        // Fan-out: both paths continue to the next child poll.
        let re_pend_bb =
            ctx.append_basic_block(cg.current_fn, &format!("par_pend_re_{}", child.callee_name));
        let re_ready_bb =
            ctx.append_basic_block(cg.current_fn, &format!("par_next_re_{}", child.callee_name));
        let re_after_bb = ctx.append_basic_block(
            cg.current_fn,
            &format!("par_after_re_{}", child.callee_name),
        );

        cg.builder
            .build_conditional_branch(is_pending_re, re_pend_bb, re_ready_bb)
            .map_err(|e| format!("par re branch {}: {e}", child.callee_name))?;

        // re_pend_bb: still Pending — set accumulator, continue.
        cg.builder.position_at_end(re_pend_bb);
        cg.builder
            .build_store(re_any_pending_alloca, ctx.i32_type().const_int(1, false))
            .map_err(|e| format!("par re pend store {}: {e}", child.callee_name))?;
        cg.builder
            .build_unconditional_branch(re_after_bb)
            .map_err(|e| format!("par re pend cont {}: {e}", child.callee_name))?;

        // re_ready_bb: Ready — mark sentinel so future re-polls skip this child safely.
        cg.builder.position_at_end(re_ready_bb);
        // Sentinel routes to sm_dead on any subsequent re-poll — null sleep_handle safe.
        state_machine::store_resume_point(ctx, &cg.builder, child_frame_re, 0x7FFF_FFFFu64)?;
        cg.builder
            .build_unconditional_branch(re_after_bb)
            .map_err(|e| format!("par re ready cont {}: {e}", child.callee_name))?;

        cg.builder.position_at_end(re_after_bb);
    }

    // After all re-polls: check accumulator.
    let re_any_pending_val = cg
        .builder
        .build_load(
            ctx.i32_type(),
            re_any_pending_alloca,
            "par_re_any_pending_val",
        )
        .map_err(|e| format!("par re_any_pending load: {e}"))?
        .into_int_value();
    let re_had_pending = cg
        .builder
        .build_int_compare(
            IntPredicate::NE,
            re_any_pending_val,
            ctx.i32_type().const_int(0, false),
            "par_re_had_pending",
        )
        .map_err(|e| format!("par re had_pending cmp: {e}"))?;
    cg.builder
        .build_conditional_branch(re_had_pending, re_suspend_bb, re_post_bb)
        .map_err(|e| format!("par re final branch: {e}"))?;

    // --- re_suspend_bb: still Pending — yield with same continuation state ---
    cg.builder.position_at_end(re_suspend_bb);
    state_machine::store_resume_point(ctx, &cg.builder, parent_frame, continuation_state as u64)?;
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("par re_suspend branch: {e}"))?;

    // --- re_post_bb: all Ready on re-poll path — merge into post_call_bb ---
    cg.builder.position_at_end(re_post_bb);
    cg.builder
        .build_unconditional_branch(post_call_bb)
        .map_err(|e| format!("par re_post merge: {e}"))?;

    // --- post_call_bb: all children Ready; read results and bind let-targets ---
    cg.builder.position_at_end(post_call_bb);

    // Read each child's return value and bind to the let-target name if applicable.
    // Results are read in declaration order; each child frame is still valid (embedded,
    // never freed for inline-poll groups — the composed frame owns the memory).
    for (child, &stmt) in children.iter().zip(stmts.iter()) {
        let child_offset = cg
            .frame_layouts
            .get(&f.name)
            .and_then(|layout| {
                layout
                    .children
                    .iter()
                    .find(|(n, _)| n == &child.callee_name)
                    .map(|(_, off)| *off)
            })
            .ok_or_else(|| {
                format!(
                    "emit_independent_group_poll post: no child frame slot for `{}`",
                    child.callee_name
                )
            })?;

        let child_frame_post = state_machine::child_frame_ptr(
            ctx,
            &cg.builder,
            parent_frame,
            child_offset,
            &format!("par_cf_{}_post", child.callee_name),
        )?;

        let ret_val =
            load_sm_return_value_typed(cg, ctx, child_frame_post, &child.callee_name, "par_ret")?;

        // Bind to the let target when the stmt is `let name = calleeF(...)`.
        if let Stmt::Let { name, .. } = stmt {
            // Route through the SAME unified binder the sequential SM let-path uses
            // (`bind_sm_result_and_flush`, emit.rs let-arms at ~3778/3806). For a
            // crossing local — one whose value must survive a SUBSEQUENT `wait` — the
            // value MUST be stored into the entry-block (sm_entry) alloca pre-created
            // at the crossing-local setup (~2344, plus the EC companion struct at ~2357)
            // and flushed to its frame slot(s). That entry-block alloca dominates every
            // resume block; a fresh alloca built here in the parallel-join block does NOT,
            // so `reload_params_from_frame` would read it from a block it doesn't dominate
            // (LLVM "instruction does not dominate all uses"). `bind_sm_result_and_flush`
            // reuses the dominating alloca for crossing locals and only fresh-allocas for
            // non-crossing bindings (which never survive a suspension, so a join-block
            // alloca is correct there). This keeps the parallel path byte-for-byte aligned
            // with the sequential store-into-existing-alloca contract — no parallel-only
            // EC/number store that could drift (corpse-(a)).
            let alloca =
                bind_sm_result_and_flush(cg, name, ret_val, parent_frame, &child.callee_name)?;
            cg.locals.insert(name.clone(), alloca);

            // EC crossing locals must be tracked in errors_capable_locals so a later use
            // (after a subsequent suspension) extracts the success value / propagates the
            // error instead of reading the companion-struct pointer. Mirrors the sequential
            // let-arms (emit.rs ~3785/3809).
            if cg.sm_crossing_errors_capable_set.contains(name.as_str()) {
                cg.errors_capable_locals.insert(name.clone());
            }
        }
        // For Stmt::Expr, return value is discarded (the existing sequential path does the same).
    }

    Ok(())
}

/// Extract the callee name and args from a statement that is a direct ident call.
///
/// Handles `let _ = calleeF(args)` and `calleeF(args)` (bare expression).
/// Returns `None` for anything else.
fn extract_call_and_args(stmt: &Stmt) -> Option<(String, &[Expr])> {
    match stmt {
        Stmt::Let {
            value: Expr::Call(c),
            ..
        }
        | Stmt::Expr(Expr::Call(c)) => {
            if let Expr::Ident(name, _) = &c.callee {
                return Some((name.clone(), &c.args));
            }
            None
        }
        _ => None,
    }
}

/// Emit inline-poll-and-yield for a call to a user-defined suspending function.
///
/// # Flow (O(1) per call site)
///
/// 1. Get the child frame pointer = GEP(parent_frame, child_offset) — fixed at compile time.
/// 2. Initialize the child frame header (resume_point=0, sleep_handle=null) on first entry.
///    Re-entry path (after Pending) skips init: the child's resume_point was already set.
/// 3. Write call arguments to child frame local slots (offset 32+).
/// 4. Call child_resume_fn(child_frame, waker_ctx) → poll_result.
///    - Ready (0): read return value from child_frame[RETURN_SLOT], continue inline.
///    - Pending (1): set parent resume_point = continuation_state, return 1 (Pending).
///
/// On re-entry (parent resumes at continuation_state):
/// 5. Get child_frame (same embedded offset — pointer is stable).
/// 6. Call child_resume_fn again (child handles its own internal state).
///    - Ready: read return value, continue.
///    - Pending: set parent resume_point = same state, return 1 again.
///
/// # Waker contract (P0 #11, ABI-locked)
///
/// `waker_ctx` is forwarded verbatim to the child resume fn. No fabricated wakers.
/// The child registers the waker with its own sub-future (sleep handle etc.);
/// the parent merely forwards the outer context.
///
/// # Return value
///
/// Returns the return value read from the child's return slot after the child signals Ready.
#[allow(clippy::too_many_arguments)]
fn emit_suspending_call_inline_poll<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    call_expr: &Expr,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    parent_frame: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    let ctx = cg.ctx;

    // Extract callee name and args from the call expression.
    let Expr::Call(c) = call_expr else {
        return Err("emit_suspending_call_inline_poll: not a Call expression".to_string());
    };
    let callee_name = if let Expr::Ident(name, _) = &c.callee {
        name.clone()
    } else {
        return Err("emit_suspending_call_inline_poll: callee is not an Ident".to_string());
    };

    // Find the child frame offset in the parent's frame layout.
    let child_offset_opt = cg
        .frame_layouts
        .get(&f.name)
        .and_then(|layout| {
            layout
                .children
                .iter()
                .find(|(name, _)| name == &callee_name)
        })
        .map(|(_, offset)| *offset);

    // Recursion edge: the callee is the same function as the caller (or causes a cycle).
    // We cannot embed the frame (infinite size). Use a heap-allocated child frame instead.
    // The heap pointer is stored in the parent frame's recursion slot (if present).
    // This is the only per-call alloc/free in non-background SM code.
    if child_offset_opt.is_none() {
        // Heap-box path for recursive/cyclic SM calls.
        return emit_suspending_call_heap_boxed(
            cg,
            c,
            &callee_name,
            state_blocks,
            pending_block,
            parent_frame,
            waker_ctx,
            param_names,
            f,
            shape_table,
            current_state,
        );
    }

    let child_offset = child_offset_opt.unwrap();

    // Get the child frame pointer (embedded sub-frame at fixed offset).
    let child_frame = state_machine::child_frame_ptr(
        ctx,
        &cg.builder,
        parent_frame,
        child_offset,
        &format!("cf_{callee_name}"),
    )?;

    // Find the child's resume function. When the callee was imported under an alias
    // (`import { getValue as fetchVal }`), the LLVM symbol was forward-declared using the
    // original exported name (`ynz_sm_getValue_resume`), not the alias. Look up the
    // effective name via FunctionSig.original_name so the module lookup succeeds.
    let callee_llvm_name = cg
        .imported_fns
        .get(callee_name.as_str())
        .and_then(|sig| sig.original_name.as_deref())
        .unwrap_or(callee_name.as_str());
    let resume_name = state_machine::resume_fn_name(callee_llvm_name);
    let child_resume_fn = cg
        .module
        .get_function(&resume_name)
        .ok_or_else(|| format!("emit_suspending_call: resume fn `{resume_name}` not declared"))?;

    // Initialize the child frame: resume_point=0 (start state), sleep_handle=null.
    // This runs in the FIRST-ENTRY basic block (state_blocks[current_state] before
    // the continuation_state is appended). The CONTINUATION basic block (re-entry after
    // Pending) skips straight past this init — the child's resume_point is already
    // non-zero from the previous poll, so re-initialization would corrupt its state.
    // Two separate basic blocks (post_call_bb for first-Ready and cont_state_bb for
    // re-entry) enforce which path reaches the re-poll, never the init.
    state_machine::store_resume_point(ctx, &cg.builder, child_frame, 0)?;
    let null_ptr = ctx.ptr_type(AddressSpace::default()).const_null();
    state_machine::store_sleep_handle(ctx, &cg.builder, child_frame, null_ptr)?;

    // Write call arguments to the child frame's local slots.
    let child_frame_layout = cg.frame_layouts.get(&callee_name);
    let child_n_locals = child_frame_layout
        .map(|l| l.n_locals)
        .unwrap_or(c.args.len());
    for (idx, arg) in c.args.iter().enumerate().take(child_n_locals) {
        let arg_val = lower_expr(cg, arg)?;
        let arg_ty = cg.expr_type(arg);
        let bits = cg
            .to_i64_bits(arg_val, &arg_ty)
            .map_err(|e| format!("sm inline-poll arg bits: {e}"))?;
        state_machine::store_local_slot(ctx, &cg.builder, child_frame, idx, bits)?;
    }

    // Continuation state for the poll-loop (re-entered when child is Pending).
    let continuation_state = *current_state + 1;
    let cont_state_bb = state_blocks
        .get(continuation_state)
        .copied()
        .ok_or_else(|| format!("inline-poll cont state {continuation_state} out of range"))?;
    let post_call_bb = ctx.append_basic_block(cg.current_fn, "sm_post_call");
    let suspend_bb = ctx.append_basic_block(cg.current_fn, "sm_call_suspend");

    // First poll: call child_resume_fn(child_frame, waker_ctx) → poll_result.
    let first_poll = cg
        .builder
        .build_call(
            child_resume_fn,
            &[child_frame.into(), waker_ctx.into()],
            "child_poll_first",
        )
        .map_err(|e| format!("child_poll_first call: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("child_resume_fn returned void")?
        .into_int_value();

    let is_ready = cg
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            first_poll,
            ctx.i32_type().const_int(0, false),
            "child_first_ready",
        )
        .map_err(|e| format!("first_poll cmp: {e}"))?;
    cg.builder
        .build_conditional_branch(is_ready, post_call_bb, suspend_bb)
        .map_err(|e| format!("first_poll branch: {e}"))?;

    // suspend_bb: child is Pending — save parent's resume_point, yield.
    cg.builder.position_at_end(suspend_bb);
    state_machine::store_resume_point(ctx, &cg.builder, parent_frame, continuation_state as u64)?;
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("sm call suspend branch: {e}"))?;

    // cont_state_bb: parent resumed after child suspended. Re-poll the child.
    // GEPs must be recomputed in each basic block (LLVM SSA dominance requirement).
    *current_state = continuation_state;
    cg.builder.position_at_end(cont_state_bb);
    reload_params_from_frame(cg, parent_frame, param_names, f, shape_table, true)?;

    // Recompute child frame pointer (same offset, but new instruction in this BB).
    let child_frame_re = state_machine::child_frame_ptr(
        ctx,
        &cg.builder,
        parent_frame,
        child_offset,
        &format!("cf_{callee_name}_re"),
    )?;

    // Re-poll child (may return Pending again; waker already registered by child's own poll).
    let re_poll = cg
        .builder
        .build_call(
            child_resume_fn,
            &[child_frame_re.into(), waker_ctx.into()],
            "child_poll_re",
        )
        .map_err(|e| format!("child_poll_re call: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("child_resume_fn (re) returned void")?
        .into_int_value();

    let is_ready_re = cg
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            re_poll,
            ctx.i32_type().const_int(0, false),
            "child_re_ready",
        )
        .map_err(|e| format!("re_poll cmp: {e}"))?;
    let still_pending_bb = ctx.append_basic_block(cg.current_fn, "sm_still_pending");
    cg.builder
        .build_conditional_branch(is_ready_re, post_call_bb, still_pending_bb)
        .map_err(|e| format!("re_poll branch: {e}"))?;

    // still_pending_bb: still Pending — keep same resume_point, yield again.
    cg.builder.position_at_end(still_pending_bb);
    state_machine::store_resume_point(ctx, &cg.builder, parent_frame, continuation_state as u64)?;
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("sm still_pending branch: {e}"))?;

    // post_call_bb: child returned Ready. Read return value from child's return slot.
    // Recompute child frame pointer again (new BB).
    cg.builder.position_at_end(post_call_bb);
    let child_frame_post = state_machine::child_frame_ptr(
        ctx,
        &cg.builder,
        parent_frame,
        child_offset,
        &format!("cf_{callee_name}_post"),
    )?;

    // For errors-capable callees: reconstruct the {i64, i64} struct from the return slot.
    // For float/number callees: load the typed value from the return slot using the
    // appropriate helper (the slot stores f64-as-i64 for float, full i128 for number).
    // For all other non-errors callees: load the i64 from the return slot.
    if is_errors_capable_fn(cg.typed, cg.imported_fns, &callee_name) {
        let (err_i64, ok_i64) =
            state_machine::load_return_value_errors(ctx, &cg.builder, child_frame_post)?;
        let struct_ty = errors_result_type(ctx);
        let mut result = struct_ty.const_zero();
        result = cg
            .builder
            .build_insert_value(result, err_i64, 0, "child_ret_err")
            .map_err(|e| format!("child_ret_err insert: {e}"))?
            .into_struct_value();
        result = cg
            .builder
            .build_insert_value(result, ok_i64, 1, "child_ret_ok")
            .map_err(|e| format!("child_ret_ok insert: {e}"))?
            .into_struct_value();
        Ok(result.into())
    } else {
        load_sm_return_value_typed(cg, ctx, child_frame_post, &callee_name, "child_ret")
    }
}

/// Emit inline-poll-and-yield for a RECURSIVE suspending call (recursion/cycle edge).
///
/// For recursive calls, the child frame cannot be embedded (infinite size). Instead:
/// 1. Heap-allocate the child frame via `ynz_alloc`.
/// 2. Store the heap pointer in the parent frame's recursion_slot so `SpawnStateFnFuture::Drop`
///    can free it on cancellation.
/// 3. Drive the child via inline poll-and-yield same as non-recursive.
/// 4. On Ready: `ynz_free` the heap child frame.
///
/// # Failure modes
///
/// Emit an inline-poll-and-yield for a suspending callee using a heap-allocated child frame.
///
/// Used for self-recursive suspending functions: the child frame cannot be embedded in the
/// parent frame (unknown size at layout time), so it is heap-allocated via `ynz_alloc_zeroed`
/// and its pointer stored in the parent's recursion slot for Drop to free on cancellation.
///
/// A missing resume function is a codegen bug — typeck rejected all forms that would
/// produce a suspending callee without a resume fn (sub-expression positions, mutual
/// recursion). Returning a wrapper-call fallback here would emit a `block_on` call from
/// inside a resume function, which panics on Tokio worker threads and contradicts AC6.
#[allow(clippy::too_many_arguments)]
fn emit_suspending_call_heap_boxed<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    c: &ynz_ast::nodes::CallExpr,
    callee_name: &str,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    parent_frame: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'_ ShapeTable,
    current_state: &mut usize,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    let ctx = cg.ctx;

    // When the callee was imported under an alias, the LLVM resume fn uses the original
    // exported symbol name. Resolve via FunctionSig.original_name so the lookup succeeds.
    let callee_llvm_name = cg
        .imported_fns
        .get(callee_name)
        .and_then(|sig| sig.original_name.as_deref())
        .unwrap_or(callee_name);
    let resume_name = state_machine::resume_fn_name(callee_llvm_name);
    let child_resume_fn = match cg.module.get_function(&resume_name) {
        Some(rf) => rf,
        None => {
            // A suspending callee without a resume fn is a codegen bug. Typeck rejects
            // every source form that would reach this path (sub-expression suspension →
            // typeck error; mutual recursion → typeck error). Emitting a wrapper-call here
            // would invoke block_on from inside a resume fn → Tokio worker thread panic.
            return Err(format!(
                "codegen bug: suspending callee `{callee_name}` has no resume fn `{resume_name}` \
                 — cannot emit inline-poll from a resume context. \
                 Typeck should have rejected this before codegen."
            ));
        }
    };

    // Compute child frame size.
    let child_frame_size = cg
        .frame_layouts
        .get(callee_name)
        .map(|l| l.total_size)
        .unwrap_or(state_machine::FRAME_HEADER_SIZE);
    let child_n_locals = cg
        .frame_layouts
        .get(callee_name)
        .map(|l| l.n_locals)
        .unwrap_or(c.args.len());

    // Heap-allocate the child frame.
    let child_frame = state_machine::alloc_frame(ctx, &cg.builder, cg.rt, child_frame_size)?;

    // Store the heap pointer in the parent frame's recursion_slot (for Drop to free).
    if let Some(rec_offset) = cg.frame_layouts.get(&f.name).and_then(|l| l.recursion_slot) {
        let rec_slot = unsafe {
            cg.builder
                .build_gep(
                    ctx.i8_type(),
                    parent_frame,
                    &[ctx.i64_type().const_int(rec_offset, false)],
                    "rec_slot_ptr",
                )
                .map_err(|e| format!("rec_slot gep: {e}"))?
        };
        cg.builder
            .build_store(rec_slot, child_frame)
            .map_err(|e| format!("rec_slot store: {e}"))?;
    }

    // Write call arguments.
    for (idx, arg) in c.args.iter().enumerate().take(child_n_locals) {
        let arg_val = lower_expr(cg, arg)?;
        let arg_ty = cg.expr_type(arg);
        let bits = cg
            .to_i64_bits(arg_val, &arg_ty)
            .map_err(|e| format!("rec arg bits: {e}"))?;
        state_machine::store_local_slot(ctx, &cg.builder, child_frame, idx, bits)?;
    }

    // Inline poll-and-yield (same mechanism as non-recursive; heap frame replaces embedded frame).
    let continuation_state = *current_state + 1;
    let cont_state_bb = state_blocks
        .get(continuation_state)
        .copied()
        .ok_or_else(|| format!("rec inline-poll cont state {continuation_state} out of range"))?;
    let post_call_bb = ctx.append_basic_block(cg.current_fn, "sm_rec_post");
    let suspend_bb = ctx.append_basic_block(cg.current_fn, "sm_rec_suspend");

    // First poll.
    let first_poll = cg
        .builder
        .build_call(
            child_resume_fn,
            &[child_frame.into(), waker_ctx.into()],
            "rec_poll_first",
        )
        .map_err(|e| format!("rec_poll_first: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("rec resume returned void")?
        .into_int_value();
    let is_ready = cg
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            first_poll,
            ctx.i32_type().const_int(0, false),
            "rec_first_ready",
        )
        .map_err(|e| format!("rec first_ready: {e}"))?;
    cg.builder
        .build_conditional_branch(is_ready, post_call_bb, suspend_bb)
        .map_err(|e| format!("rec first branch: {e}"))?;

    // suspend_bb.
    cg.builder.position_at_end(suspend_bb);
    state_machine::store_resume_point(ctx, &cg.builder, parent_frame, continuation_state as u64)?;
    // Store the heap frame pointer in the recursion slot for re-entry access.
    // (Already stored above if recursion_slot is Some.)
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("rec suspend branch: {e}"))?;

    // cont_state_bb: re-poll.
    *current_state = continuation_state;
    cg.builder.position_at_end(cont_state_bb);
    reload_params_from_frame(cg, parent_frame, param_names, f, shape_table, true)?;

    // Reload the heap child frame pointer from the recursion slot.
    let rec_frame =
        if let Some(rec_offset) = cg.frame_layouts.get(&f.name).and_then(|l| l.recursion_slot) {
            let rec_slot = unsafe {
                cg.builder
                    .build_gep(
                        ctx.i8_type(),
                        parent_frame,
                        &[ctx.i64_type().const_int(rec_offset, false)],
                        "rec_slot_ptr_re",
                    )
                    .map_err(|e| format!("rec_slot_re gep: {e}"))?
            };
            cg.builder
                .build_load(
                    ctx.ptr_type(AddressSpace::default()),
                    rec_slot,
                    "rec_frame_re",
                )
                .map_err(|e| format!("rec_frame_re load: {e}"))?
                .into_pointer_value()
        } else {
            child_frame // use the originally-allocated frame (SSA value from state 0)
        };

    let re_poll = cg
        .builder
        .build_call(
            child_resume_fn,
            &[rec_frame.into(), waker_ctx.into()],
            "rec_poll_re",
        )
        .map_err(|e| format!("rec_poll_re: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("rec resume re returned void")?
        .into_int_value();
    let is_ready_re = cg
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            re_poll,
            ctx.i32_type().const_int(0, false),
            "rec_re_ready",
        )
        .map_err(|e| format!("rec re_ready: {e}"))?;
    let still_pending = ctx.append_basic_block(cg.current_fn, "sm_rec_still_pending");
    cg.builder
        .build_conditional_branch(is_ready_re, post_call_bb, still_pending)
        .map_err(|e| format!("rec re branch: {e}"))?;

    // still_pending_bb.
    cg.builder.position_at_end(still_pending);
    state_machine::store_resume_point(ctx, &cg.builder, parent_frame, continuation_state as u64)?;
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("rec still_pending: {e}"))?;

    // post_call_bb: child Ready. Read return value, free heap frame.
    cg.builder.position_at_end(post_call_bb);
    let rec_frame_post =
        if let Some(rec_offset) = cg.frame_layouts.get(&f.name).and_then(|l| l.recursion_slot) {
            let rec_slot = unsafe {
                cg.builder
                    .build_gep(
                        ctx.i8_type(),
                        parent_frame,
                        &[ctx.i64_type().const_int(rec_offset, false)],
                        "rec_slot_ptr_post",
                    )
                    .map_err(|e| format!("rec_slot_post gep: {e}"))?
            };
            cg.builder
                .build_load(
                    ctx.ptr_type(AddressSpace::default()),
                    rec_slot,
                    "rec_frame_post",
                )
                .map_err(|e| format!("rec_frame_post load: {e}"))?
                .into_pointer_value()
        } else {
            child_frame
        };

    let ret_val = if is_errors_capable_fn(cg.typed, cg.imported_fns, callee_name) {
        let (err_i64, ok_i64) =
            state_machine::load_return_value_errors(ctx, &cg.builder, rec_frame_post)?;
        let struct_ty = errors_result_type(ctx);
        let mut r = struct_ty.const_zero();
        r = cg
            .builder
            .build_insert_value(r, err_i64, 0, "rec_ret_err")
            .map_err(|e| format!("rec_ret_err: {e}"))?
            .into_struct_value();
        r = cg
            .builder
            .build_insert_value(r, ok_i64, 1, "rec_ret_ok")
            .map_err(|e| format!("rec_ret_ok: {e}"))?
            .into_struct_value();
        r.into()
    } else {
        load_sm_return_value_typed(cg, ctx, rec_frame_post, callee_name, "rec_ret")?
    };

    // Free the heap child frame (normal completion path).
    state_machine::free_frame(ctx, &cg.builder, cg.rt, rec_frame_post, child_frame_size)?;

    // Clear the recursion slot so Drop knows it's been freed.
    if let Some(rec_offset) = cg.frame_layouts.get(&f.name).and_then(|l| l.recursion_slot) {
        let rec_slot = unsafe {
            cg.builder
                .build_gep(
                    ctx.i8_type(),
                    parent_frame,
                    &[ctx.i64_type().const_int(rec_offset, false)],
                    "rec_slot_ptr_clear",
                )
                .map_err(|e| format!("rec_slot_clear gep: {e}"))?
        };
        let null = ctx.ptr_type(AddressSpace::default()).const_null();
        cg.builder
            .build_store(rec_slot, null)
            .map_err(|e| format!("rec_slot clear: {e}"))?;
    }

    Ok(ret_val)
}

/// Emit the IR for a single `wait sleep(ms)` point within a state-machine body.
///
/// # Flow
///
/// 1. Evaluate the sleep duration `ms` as an i64.
/// 2. Call `ynz_rt_async_sleep_create(ms)` → opaque handle pointer.
/// 3. Poll immediately via `ynz_rt_async_sleep_poll` — this registers the waker with Tokio's
///    timer reactor (without this poll, the reactor never knows to wake the task).
/// 4. If Ready (0ms sleep): branch directly to `post_wait_bb` (no suspension needed).
/// 5. If Pending: save handle to frame, set `resume_point = continuation_state_idx`,
///    branch to `pending_block` (returns 1 = Pending to Tokio's executor).
/// 6. `cont_state_bb` (entered on Tokio wakeup): reload params from frame (fresh stack frame
///    each resume_fn call), re-poll handle to confirm Ready, branch to `post_wait_bb`.
/// 7. `post_wait_bb`: clear handle slot, reload params, continue with post-wait statements.
///
/// # Failure modes
///
/// - Inner expression is not a `sleep` call: falls back to evaluating the inner
///   expression normally (no suspension). Kept as safe fallback — typeck warns first.
#[allow(clippy::too_many_arguments)]
fn emit_wait_point<'ctx, 'g>(
    cg: &mut Cg<'ctx, 'g>,
    inner: &Expr,
    state_blocks: &[inkwell::basic_block::BasicBlock<'ctx>],
    pending_block: inkwell::basic_block::BasicBlock<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    param_names: &[String],
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    current_state: &mut usize,
) -> Result<(), String> {
    let ctx = cg.ctx;

    // Determine if the inner call is `sleep(ms)` (the yielding non-blocking sleep intrinsic).
    let is_sleep_async = matches!(
        inner,
        Expr::Call(c) if matches!(&c.callee, Expr::Ident(n, _) if n == "sleep")
    );

    if !is_sleep_async {
        // Non-may-block wait: typeck should warn (wait_on_non_may_block_warning). Evaluate
        // the inner expression normally (no suspension). Still transition through the state
        // so sm_sN gets a terminator — the "wait" is a no-op at runtime.
        lower_expr(cg, inner)?;
        let continuation_state = *current_state + 1;
        let cont_state_bb = state_blocks
            .get(continuation_state)
            .copied()
            .ok_or_else(|| format!("continuation state {continuation_state} out of range"))?;
        let post_wait_bb = cg.ctx.append_basic_block(cg.current_fn, "sm_noop_wait");
        // Direct branch to continuation state (no suspension).
        cg.builder
            .build_unconditional_branch(cont_state_bb)
            .map_err(|e| format!("noop wait branch: {e}"))?;
        *current_state = continuation_state;
        // Fill the continuation state with a direct branch to post_wait_bb.
        cg.builder.position_at_end(cont_state_bb);
        reload_params_from_frame(cg, frame_ptr, param_names, f, shape_table, true)?;
        cg.builder
            .build_unconditional_branch(post_wait_bb)
            .map_err(|e| format!("cont noop branch: {e}"))?;
        cg.builder.position_at_end(post_wait_bb);
        return Ok(());
    }

    // Extract the ms argument from sleep(ms).
    let ms_val = if let Expr::Call(c) = inner {
        if c.args.is_empty() {
            ctx.i64_type().const_int(0, false)
        } else {
            lower_expr(cg, &c.args[0])?.into_int_value()
        }
    } else {
        ctx.i64_type().const_int(0, false)
    };

    // Step 1: Call ynz_rt_async_sleep_create(ms) → handle pointer.
    let handle = cg
        .builder
        .build_call(
            cg.rt.ynz_rt_async_sleep_create,
            &[ms_val.into()],
            "sleep_handle",
        )
        .map_err(|e| format!("sleep_create call: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("ynz_rt_async_sleep_create returned void")?
        .into_pointer_value();

    // Step 2: Poll the sleep handle immediately — this registers the waker with Tokio's
    // reactor so the task is woken when the timer fires.
    //
    // WHY poll here (not just on re-entry): `ynz_rt_async_sleep_create` allocates the
    // Sleep future but does NOT register any waker. Only calling `ynz_rt_async_sleep_poll`
    // registers the waker with the timer reactor. Without this first poll, the task
    // returns Pending with no waker, and Tokio's reactor never knows to wake it.
    // P0 Contract #11 locked this protocol: the waker_ctx must be forwarded from the
    // enclosing SpawnStateFnFuture::poll's cx argument.
    let first_poll_result = cg
        .builder
        .build_call(
            cg.rt.ynz_rt_async_sleep_poll,
            &[handle.into(), waker_ctx.into()],
            "first_poll",
        )
        .map_err(|e| format!("first_poll call: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("ynz_rt_async_sleep_poll returned void")?
        .into_int_value();

    // Continuation state index and the state/post_wait blocks.
    let continuation_state = *current_state + 1;
    let cont_state_bb = state_blocks
        .get(continuation_state)
        .copied()
        .ok_or_else(|| format!("continuation state {continuation_state} out of range"))?;
    let post_wait_bb = ctx.append_basic_block(cg.current_fn, "sm_post_wait");
    let suspend_bb = ctx.append_basic_block(cg.current_fn, "sm_suspend");

    // Branch on the first poll result:
    // - Ready (0): the sleep already completed (very short or zero duration); skip state machine
    //   suspension and go directly to post_wait_bb.
    // - Pending (1): save the handle to the frame, set resume_point = continuation_state,
    //   branch to pending_block (returns 1 to Tokio's executor).
    let is_ready = cg
        .builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            first_poll_result,
            ctx.i32_type().const_int(0, false),
            "first_is_ready",
        )
        .map_err(|e| format!("first_poll cmp: {e}"))?;
    cg.builder
        .build_conditional_branch(is_ready, post_wait_bb, suspend_bb)
        .map_err(|e| format!("first_poll branch: {e}"))?;

    // suspend_bb: the sleep is pending — save state and return Pending to Tokio.
    cg.builder.position_at_end(suspend_bb);
    state_machine::store_sleep_handle(ctx, &cg.builder, frame_ptr, handle)?;
    state_machine::store_resume_point(ctx, &cg.builder, frame_ptr, continuation_state as u64)?;
    cg.builder
        .build_unconditional_branch(pending_block)
        .map_err(|e| format!("sm branch to pending: {e}"))?;

    // cont_state_bb: Tokio woke us up after the timer fired. Reload params from frame
    // (each resume_fn call has fresh allocas — the old allocas from state_0's call are gone).
    // Then re-poll the handle to confirm Ready and transition to post_wait_bb.
    *current_state = continuation_state;
    cg.builder.position_at_end(cont_state_bb);
    reload_params_from_frame(cg, frame_ptr, param_names, f, shape_table, true)?;
    state_machine::emit_sleep_poll_branch(
        ctx,
        &cg.builder,
        cg.rt,
        frame_ptr,
        waker_ctx,
        post_wait_bb,
        pending_block,
    )?;

    // post_wait_bb: the sleep completed (either immediately on first poll or after wakeup).
    // Clear the handle slot (the runtime freed the Sleep box on Ready return).
    // Reload params in case we arrived here from the first-poll-Ready path (state_0).
    cg.builder.position_at_end(post_wait_bb);
    let null_ptr = ctx.ptr_type(inkwell::AddressSpace::default()).const_null();
    state_machine::store_sleep_handle(ctx, &cg.builder, frame_ptr, null_ptr)?;
    // Params are already loaded in both paths (state_0 via sm_entry reload, state_N via cont_state_bb reload).

    // Builder is now positioned at post_wait_bb for subsequent statement emission.
    Ok(())
}

/// Convert a Yinz parameter LLVM value to i64 bits for storage in a state-machine frame slot.
///
/// Mirrors `Cg::to_i64_bits` but operates on `BasicValueEnum` directly (parameter values
/// are not yet in allocas at the wrapper-function entry point).
/// Convert a value to its i64-bit slot representation — the SINGLE marshaller shared by
/// array/map storage (`Cg::to_i64_bits`) AND state-machine wrapper param storage.
///
/// `resolved` must already have generic substitution applied (callers that hold a `Cg`
/// pass `self.resolve_type(ty)`; the state-machine wrapper passes a concrete param type).
///
/// Bool returns the i1 unchanged; the `store_local_slot` / array-store helpers zero-extend
/// to the i64 slot width, so callers never need to widen here.
fn value_to_i64_bits<'ctx>(
    builder: &inkwell::builder::Builder<'ctx>,
    i64_ty: inkwell::types::IntType<'ctx>,
    val: BasicValueEnum<'ctx>,
    resolved: &Type,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    match resolved {
        Type::Int | Type::Bool => Ok(val.into_int_value()),
        Type::Float => builder
            .build_bit_cast(val.into_float_value(), i64_ty, "f_to_i")
            .map(|v| v.into_int_value())
            .map_err(|e| format!("{e}")),
        Type::String
        | Type::Number { .. }
        | Type::Shape { .. }
        | Type::Dynamic { .. }
        | Type::BuiltinArray { .. }
        | Type::BuiltinFixed { .. }
        | Type::Maybe { .. }
        | Type::BuiltinMap { .. }
        | Type::MapEntry { .. }
        | Type::Union { .. }
        | Type::Sensitive { .. } => builder
            .build_ptr_to_int(val.into_pointer_value(), i64_ty, "ptr_to_i")
            .map_err(|e| format!("{e}")),
        _ => Err(format!("cannot convert {:?} to i64 bits", resolved)),
    }
}

/// Map an AST type annotation to the typeck `Type` for use in codegen decisions.
fn ast_type_to_typeck_type(ast_ty: &ynz_ast::nodes::Type, shape_table: &ShapeTable) -> Type {
    match ast_ty {
        ynz_ast::nodes::Type::Nothing => Type::Nothing,
        ynz_ast::nodes::Type::Int => Type::Int,
        ynz_ast::nodes::Type::Float => Type::Float,
        ynz_ast::nodes::Type::Bool => Type::Bool,
        ynz_ast::nodes::Type::Named(n, _) if n == "string" => Type::String,
        // M6: union type aliases.
        ynz_ast::nodes::Type::Named(n, _) if shape_table.union_aliases.contains_key(n) => {
            shape_table.union_aliases[n].clone()
        }
        ynz_ast::nodes::Type::Named(n, _) if shape_table.contains(n) => {
            Type::Shape { name: n.clone() }
        }
        ynz_ast::nodes::Type::Dynamic { contract, .. } => Type::Dynamic {
            contract: contract.clone(),
        },
        // `self` parameter and SelfType: the typeck stores the concrete Shape type in
        // expr_types; for parameter materialization we just need to know it's a ptr.
        ynz_ast::nodes::Type::Named(n, _) if n == "self" => Type::Shape {
            name: String::new(),
        },
        ynz_ast::nodes::Type::SelfType { .. } => Type::Shape {
            name: String::new(),
        },
        // M5 generic types — delegate to shape_table.resolve_ast_type for proper resolution.
        ynz_ast::nodes::Type::Generic { .. }
        | ynz_ast::nodes::Type::Maybe { .. }
        | ynz_ast::nodes::Type::TypeParam { .. } => shape_table.resolve_ast_type(ast_ty),
        // M6: union types in parameter position.
        ynz_ast::nodes::Type::Union { variants, .. } => {
            let resolved: Vec<Type> = variants
                .iter()
                .map(|v| ast_type_to_typeck_type(v, shape_table))
                .collect();
            if resolved.len() < 2 {
                Type::Error
            } else {
                Type::Union { variants: resolved }
            }
        }
        // M7 P1: `-> T errors` — defer to the inner type for codegen purposes.
        ynz_ast::nodes::Type::ErrorCapable { inner, .. } => {
            ast_type_to_typeck_type(inner, shape_table)
        }
        // M8 P4: `sensitive T` — preserve the Sensitive wrapper (ABI = ptr, same as string).
        ynz_ast::nodes::Type::Sensitive(inner) => {
            let inner_ty = ast_type_to_typeck_type(inner, shape_table);
            Type::Sensitive {
                inner: Box::new(inner_ty),
            }
        }
        // M8 P6: `number<N>` with explicit precision in parameter position.
        ynz_ast::nodes::Type::Number { precision } => Type::Number {
            precision: *precision,
        },
        _ => Type::Error,
    }
}

/// Materialize an LLVM function parameter as a named alloca.
///
/// Scalars (int, float, bool) are stored directly. Pointer params (string, number,
/// shape, dynamic) get a `ptr`-sized alloca that holds the incoming pointer, so the
/// "every local = alloca" invariant holds. Loading from that slot gives back the ptr.
fn materialize_param<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    name: &str,
    llvm_val: inkwell::values::BasicValueEnum<'ctx>,
    param_ty: &Type,
) -> Result<(), String> {
    match param_ty {
        // Pointer-typed params: incoming LLVM value is a ptr; store the ptr in a ptr-slot.
        Type::Shape { .. } | Type::Dynamic { .. }
        | Type::BuiltinArray { .. } | Type::BuiltinFixed { .. } | Type::Maybe { .. }
        | Type::BuiltinMap { .. } | Type::MapEntry { .. }
        // M6: union values are heap tagged-structs, passed/stored as opaque pointers.
        | Type::Union { .. } => {
            let slot = cg.builder.build_alloca(cg.ptr(), name)
                .map_err(|e| format!("param alloca {name}: {e}"))?;
            cg.builder.build_store(slot, llvm_val)
                .map_err(|e| format!("param store {name}: {e}"))?;
            cg.locals.insert(name.to_string(), slot);
        }
        _ => {
            let slot = cg.alloca(param_ty, name)?;
            store(cg, llvm_val, param_ty, slot)?;
            cg.locals.insert(name.to_string(), slot);
        }
    }
    Ok(())
}

/// True when the current basic block already has a terminator instruction.
///
/// Used to avoid emitting dead instructions after `ret`, `br`, or `unreachable`.
fn is_block_terminated(cg: &Cg) -> bool {
    cg.builder
        .get_insert_block()
        .map(|bb| bb.get_terminator().is_some())
        .unwrap_or(true)
}

/// Emit a cooperative preemption checkpoint at a loop back-edge.
///
/// The v0.3-M1 stub is a no-op (single `ret`); it correctly positions call sites
/// for v0.3-M2 state-machine suspension. Call-site preempt (at every `build_call`
/// for user functions) deferred to M2 per P1 GATE measurement (1190% overhead).
#[inline]
fn emit_loop_preempt<'ctx>(cg: &mut Cg<'ctx, '_>) -> Result<(), String> {
    cg.builder
        .build_call(cg.rt.ynz_rt_check_preempt, &[], "preempt")
        .map_err(|e| format!("preempt: {e}"))?;
    Ok(())
}

fn lower_stmt<'ctx>(cg: &mut Cg<'ctx, '_>, stmt: &Stmt) -> Result<(), String> {
    match stmt {
        Stmt::Expr(expr) => {
            lower_expr(cg, expr)?;
        }

        Stmt::Let {
            name,
            ty: ann_ty,
            value,
            ..
        } => {
            let val_ty = cg.expr_type(value);
            let val = lower_expr(cg, value)?;

            // M6: when annotation is a union type and value is a concrete shape variant,
            // construct a tagged-struct { i64 tag, i64 data } on the stack.
            let union_constructed = 'union_ctor: {
                if let Some(ast_ann) = ann_ty.as_ref() {
                    let resolved_ann = ast_type_to_typeck_type(ast_ann, cg.shape_table);
                    if let Type::Union { ref variants } = resolved_ann {
                        if let Type::Shape {
                            name: ref shape_name,
                        } = val_ty
                        {
                            let tag = variants.iter().position(|v| {
                                if let Type::Shape { name } = v {
                                    name == shape_name
                                } else {
                                    false
                                }
                            });
                            if let Some(tag_idx) = tag {
                                let union_st = cg
                                    .ctx
                                    .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                                let union_slot = cg
                                    .builder
                                    .build_alloca(union_st, &format!("{name}_union"))
                                    .map_err(|e| format!("{e}"))?;
                                let tag_gep = cg
                                    .builder
                                    .build_struct_gep(union_st, union_slot, 0, "u_tag")
                                    .map_err(|e| format!("{e}"))?;
                                cg.builder
                                    .build_store(tag_gep, cg.i64().const_int(tag_idx as u64, false))
                                    .map_err(|e| format!("{e}"))?;
                                let data_gep = cg
                                    .builder
                                    .build_struct_gep(union_st, union_slot, 1, "u_data")
                                    .map_err(|e| format!("{e}"))?;
                                let ptr_as_i64 = cg
                                    .builder
                                    .build_ptr_to_int(val.into_pointer_value(), cg.i64(), "ptr2i")
                                    .map_err(|e| format!("{e}"))?;
                                cg.builder
                                    .build_store(data_gep, ptr_as_i64)
                                    .map_err(|e| format!("{e}"))?;
                                let outer_slot = cg
                                    .builder
                                    .build_alloca(cg.ptr(), name)
                                    .map_err(|e| format!("{e}"))?;
                                cg.builder
                                    .build_store(outer_slot, union_slot)
                                    .map_err(|e| format!("{e}"))?;
                                cg.locals.insert(name.clone(), outer_slot);
                                break 'union_ctor true;
                            }
                        }
                    }
                }
                false
            };
            if !union_constructed {
                // When inside a SM resume function and this name is a crossing local,
                // the alloca was pre-created in the sm_entry block (LLVM SSA dominance
                // requires allocas to be in the entry block). Reuse that alloca instead
                // of creating a new one in the current state block.
                //
                // Exception: if we are inside a nested scope (sm_scope_depth > 0), this
                // `let name = ...` is a SHADOW binding that creates a new inner local —
                // not a write to the outer crossing local. Create a fresh alloca so the
                // outer crossing alloca is not clobbered by the shadow's value.
                let is_sm_crossing = cg
                    .sm_crossing_names
                    .as_ref()
                    .is_some_and(|v| v.iter().any(|n| n == name.as_str()));
                let slot = if is_sm_crossing {
                    // Crossing local: reuse the pre-created sm_entry alloca regardless of
                    // nesting depth. LLVM SSA dominance requires allocas to be in the entry
                    // block; the sm_entry alloca dominates all state blocks. Shadow bindings
                    // (a nested `let x` where outer `x` crosses a wait) are rejected at
                    // typeck (ShadowsCrossingLocal), so any `let name` here is the first
                    // and only definition of that crossing local.
                    *cg.locals.get(name.as_str()).ok_or_else(|| {
                        format!("sm crossing alloca for `{name}` missing in entry")
                    })?
                } else {
                    // Not a crossing local: alloca in the ENTRY block so it dominates all
                    // successor blocks. Yinz allows variable shadowing (design/linting.md
                    // `shadowed-variables` lint); a shadow `let x` inside an if/while body
                    // is a separate LLVM basic block — its alloca must be in the entry block
                    // or LLVM rejects the IR with "Instruction does not dominate all uses!".
                    // The outer binding is restored to cg.locals on scope exit (restore-all
                    // protocol), so the shadow has no lasting effect outside its own scope.
                    let s = cg.alloca_in_entry(&val_ty, name)?;
                    cg.locals.insert(name.clone(), s);
                    s
                };
                // Shape crossing local: val is a PointerValue to a temp stack struct (from
                // lower_struct_lit). The slot is a ptr alloca pre-wired to the frame's slot
                // region (Step 1b). Memcpy from the temp into the frame region by loading the
                // frame region ptr from the ptr alloca and memcpy-ing into it. Works at any
                // nesting depth — the ptr alloca is always the sm_entry one.
                let stored = if is_sm_crossing
                    && matches!(val_ty, Type::Shape { .. })
                    && cg.sm_crossing_shape_embed_set.contains(name.as_str())
                {
                    let shape_name = match &val_ty {
                        Type::Shape { name: sn } => sn.clone(),
                        _ => unreachable!(),
                    };
                    if let Some(struct_ty) = cg.shape_types.get(&shape_name) {
                        let src_ptr = val.into_pointer_value();
                        let size_val = struct_ty.size_of().ok_or_else(|| {
                            format!("sm shape let: size_of `{shape_name}` unavailable")
                        })?;
                        let size_i64 = cg
                            .builder
                            .build_int_z_extend(
                                size_val,
                                cg.ctx.i64_type(),
                                &format!("{name}_let_sz"),
                            )
                            .map_err(|e| format!("sm shape let size extend {name}: {e}"))?;
                        // Load the frame region ptr from the ptr alloca (pre-wired to frame
                        // in Step 1b). Memcpy from the temp stack alloca into the frame region.
                        let dest_ptr = cg
                            .builder
                            .build_load(
                                cg.ctx.ptr_type(AddressSpace::default()),
                                slot,
                                &format!("{name}_frame_ptr"),
                            )
                            .map_err(|e| format!("sm shape let load frame ptr {name}: {e}"))?
                            .into_pointer_value();
                        cg.builder
                            .build_memcpy(dest_ptr, 1, src_ptr, 1, size_i64)
                            .map_err(|e| format!("sm shape let memcpy {name}: {e}"))?;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !stored {
                    store(cg, val, &val_ty, slot)?;
                }
                // M7 P4a: track bindings that hold errors-capable results.
                if matches!(val_ty, Type::ErrorsCapable { .. }) {
                    cg.errors_capable_locals.insert(name.clone());
                }
            }
        }

        Stmt::Assign { target, value, .. } => {
            let slot = *cg
                .locals
                .get(target.as_str())
                .ok_or_else(|| format!("undefined `{target}` in codegen"))?;
            let ty = cg.expr_type(value);
            let val = lower_expr(cg, value)?;
            store(cg, val, &ty, slot)?;
        }

        Stmt::If { cond, body, .. } => {
            // Snapshot before entering the if body so we can restore every name on exit.
            // This preserves lexical scoping: crossing-local sm_entry allocas stay active
            // after the block (not replaced by a shadow binding's fresh alloca), and
            // non-crossing names don't leak into the outer scope after the block closes.
            // Yinz allows variable shadowing (design/linting.md `shadowed-variables` lint);
            // restoring all snapshot entries makes shadowing safe at the codegen level.
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            let r = lower_stmt_if(cg, cond, body);
            cg.sm_scope_depth -= 1;
            // Restore ALL names that were present before the scope — not just crossing names.
            // Crossing locals get back their sm_entry allocas; shadow bindings are unwound.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
            r?;
        }

        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            let r = lower_stmt_match(cg, scrutinee, arms, else_arm.as_ref());
            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as Stmt::If.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
            r?;
        }

        Stmt::While { cond, body, .. } => {
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            let r = lower_stmt_while(cg, cond, body);
            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as Stmt::If.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
            r?;
        }

        Stmt::For {
            var, iter, body, ..
        } => {
            let locals_snapshot = cg.locals.clone();
            cg.sm_scope_depth += 1;
            let r = lower_stmt_for(cg, var, iter, body);
            cg.sm_scope_depth -= 1;
            // Restore ALL snapshot entries — same lexical-scoping rationale as Stmt::If.
            for (name, &outer_alloca) in &locals_snapshot {
                cg.locals.insert(name.clone(), outer_alloca);
            }
            r?;
        }

        Stmt::Return { value, .. } => {
            lower_stmt_return(cg, value.as_ref())?;
        }

        Stmt::FieldAssign { target, value, .. } => {
            lower_stmt_field_assign(cg, target, value)?;
        }

        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;
            let elem_val = lower_expr(cg, value)?;
            let elem_ty = cg.expr_type(value);
            let bits = cg.to_i64_bits(elem_val, &elem_ty)?;

            if let Type::BuiltinMap { key, .. } = &recv_ty {
                let key_ty = key.as_ref().clone();
                let key_val = lower_expr(cg, index)?;
                if key_is_string(&key_ty) {
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set_str,
                            &[recv_val.into(), key_val.into(), bits.into()],
                            "mia_s",
                        )
                        .map_err(|e| format!("{e}"))?;
                } else {
                    let kt = cg.expr_type(index);
                    let kb = cg.to_i64_bits(key_val, &kt)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set,
                            &[recv_val.into(), kb.into(), bits.into()],
                            "mia",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
            } else {
                let idx_val = lower_expr(cg, index)?.into_int_value();
                match &recv_ty {
                    Type::BuiltinArray { .. } => {
                        cg.builder
                            .build_call(
                                cg.rt.ynz_array_set,
                                &[recv_val.into(), idx_val.into(), bits.into()],
                                "arr_set",
                            )
                            .map_err(|e| format!("array_set: {e}"))?;
                    }
                    Type::BuiltinFixed { size, .. } => {
                        let arr_ptr = recv_val.into_pointer_value();
                        let gep = unsafe {
                            cg.builder
                                .build_gep(cg.i64(), arr_ptr, &[idx_val], "fixed_set_elem")
                                .map_err(|e| format!("fixed_set gep: {e}"))?
                        };
                        cg.builder
                            .build_store(gep, bits)
                            .map_err(|e| format!("{e}"))?;
                        let _ = size;
                    }
                    _ => return Err(format!("IndexAssign on unsupported type: {:?}", recv_ty)),
                }
            }
        }
    }
    Ok(())
}

fn lower_stmt_if<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    cond: &Expr,
    body: &ynz_ast::nodes::Block,
) -> Result<(), String> {
    let cond_val = lower_expr(cg, cond)?.into_int_value();
    let then_bb = cg.append_block("if_then");
    let merge_bb = cg.append_block("if_merge");

    cg.builder
        .build_conditional_branch(cond_val, then_bb, merge_bb)
        .map_err(|e| format!("if branch: {e}"))?;

    cg.builder.position_at_end(then_bb);
    for stmt in &body.stmts {
        if is_block_terminated(cg) {
            break;
        }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        cg.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(merge_bb);
    Ok(())
}

fn lower_stmt_match<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    scrutinee: &Expr,
    arms: &[ynz_ast::nodes::MatchArm],
    else_arm: Option<&ynz_ast::nodes::Block>,
) -> Result<(), String> {
    let scrutinee_ty = cg.expr_type(scrutinee);
    let scrutinee_val = lower_expr(cg, scrutinee)?;

    let merge_bb = cg.append_block("match_merge");
    let final_fallthrough_bb = if else_arm.is_some() {
        cg.append_block("match_else")
    } else {
        merge_bb
    };

    for (i, arm) in arms.iter().enumerate() {
        let arm_body_bb = cg.append_block(&format!("match_arm{i}"));
        let next_check_bb = if i + 1 < arms.len() {
            cg.append_block(&format!("match_check{}", i + 1))
        } else {
            final_fallthrough_bb
        };

        let pat_cond = match &arm.pattern.kind {
            MatchPatternKind::Value(pat_expr) => {
                let pat_val = lower_expr(cg, pat_expr)?;
                match_cmp(cg, &scrutinee_ty, scrutinee_val, pat_val)?
            }
            // M6: options variant arm — compare i8 tag
            MatchPatternKind::OptionName(variant_name) => {
                if let Type::Options { name: opts_name } = &scrutinee_ty {
                    if let Some(entry) = cg.options_table.options.get(opts_name.as_str()) {
                        let tag = entry.variants.iter().position(|v| v == variant_name)
                            .ok_or_else(|| format!("codegen: unknown variant `{variant_name}` in options `{opts_name}`"))? as u64;
                        let tag_val = cg.ctx.i8_type().const_int(tag, false);
                        cg.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                scrutinee_val.into_int_value(),
                                tag_val,
                                "opt_arm_cmp",
                            )
                            .map_err(|e| format!("{e}"))?
                    } else {
                        return Err(format!("codegen: unknown options type `{opts_name}`"));
                    }
                } else {
                    return Err(format!(
                        "codegen: OptionName arm on non-options type {:?}",
                        scrutinee_ty
                    ));
                }
            }
            // M6: union type-narrowing arm — compare tag in { i64 tag, i64 data } struct
            MatchPatternKind::Is(type_path) => {
                if let Type::Union { variants } = &scrutinee_ty {
                    let tag = variants
                        .iter()
                        .position(|v| {
                            if let Type::Shape { name } = v {
                                name == &type_path.name
                            } else {
                                false
                            }
                        })
                        .ok_or_else(|| {
                            format!("codegen: union variant `{}` not found", type_path.name)
                        })? as u64;
                    let tag_const = cg.i64().const_int(tag, false);
                    // Union layout: { i64 tag, i64 data }. scrutinee_val is ptr-to-struct.
                    // (It was loaded from the slot by lower_expr, giving us the struct ptr directly.)
                    let union_st = cg
                        .ctx
                        .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                    let tag_gep = cg
                        .builder
                        .build_struct_gep(
                            union_st,
                            scrutinee_val.into_pointer_value(),
                            0,
                            "union_tag_gep",
                        )
                        .map_err(|e| format!("union tag gep: {e}"))?;
                    let tag_loaded = cg
                        .builder
                        .build_load(cg.i64(), tag_gep, "union_tag")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_loaded.into_int_value(),
                            tag_const,
                            "union_arm_cmp",
                        )
                        .map_err(|e| format!("{e}"))?
                } else {
                    return Err(format!(
                        "codegen: Is arm on non-union type {:?}",
                        scrutinee_ty
                    ));
                }
            }
        };

        cg.builder
            .build_conditional_branch(pat_cond, arm_body_bb, next_check_bb)
            .map_err(|e| format!("match branch: {e}"))?;

        cg.builder.position_at_end(arm_body_bb);
        for stmt in &arm.body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(next_check_bb);
    }

    // Emit else body (current position is final_fallthrough_bb or merge_bb).
    if let Some(else_body) = else_arm {
        for stmt in &else_body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;
        }
        cg.builder.position_at_end(merge_bb);
    }
    // If no else_arm: current position is already merge_bb (final_fallthrough_bb == merge_bb).

    Ok(())
}

/// Compare `scrutinee_val` against `pattern_val` for equality, returning `i1`.
fn match_cmp<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    scrutinee_ty: &Type,
    scrutinee_val: BasicValueEnum<'ctx>,
    pattern_val: BasicValueEnum<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    match scrutinee_ty {
        Type::Int | Type::Bool => cg
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                scrutinee_val.into_int_value(),
                pattern_val.into_int_value(),
                "match_eq",
            )
            .map_err(|e| format!("{e}")),
        Type::Float => cg
            .builder
            .build_float_compare(
                inkwell::FloatPredicate::OEQ,
                scrutinee_val.into_float_value(),
                pattern_val.into_float_value(),
                "fmatch",
            )
            .map_err(|e| format!("{e}")),
        Type::String => {
            let call = cg
                .builder
                .build_call(
                    cg.rt.string_eq,
                    &[scrutinee_val.into(), pattern_val.into()],
                    "str_eq",
                )
                .map_err(|e| format!("{e}"))?;
            let result = call
                .try_as_basic_value()
                .basic()
                .ok_or("string_eq void")?
                .into_int_value();
            cg.builder
                .build_int_compare(
                    IntPredicate::NE,
                    result,
                    cg.i32().const_int(0, false),
                    "str_eq_bool",
                )
                .map_err(|e| format!("{e}"))
        }
        Type::Number { .. } => {
            let c = cg
                .builder
                .build_call(
                    cg.rt.decimal_compare,
                    &[scrutinee_val.into(), pattern_val.into()],
                    "dcmp",
                )
                .map_err(|e| format!("{e}"))?;
            let ci = c
                .try_as_basic_value()
                .basic()
                .ok_or("dcmp void")?
                .into_int_value();
            cg.builder
                .build_int_compare(IntPredicate::EQ, ci, cg.i32().const_int(0, false), "deq")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!("codegen: match on unsupported type {:?}", other)),
    }
}

fn lower_stmt_while<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    cond: &Expr,
    body: &ynz_ast::nodes::Block,
) -> Result<(), String> {
    let header_bb = cg.append_block("while_header");
    let body_bb = cg.append_block("while_body");
    let exit_bb = cg.append_block("while_exit");

    cg.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(header_bb);
    let cond_val = lower_expr(cg, cond)?.into_int_value();
    cg.builder
        .build_conditional_branch(cond_val, body_bb, exit_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(body_bb);
    for stmt in &body.stmts {
        if is_block_terminated(cg) {
            break;
        }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        emit_loop_preempt(cg)?;
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(exit_bb);
    Ok(())
}

fn lower_stmt_for<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    var: &str,
    iter: &Expr,
    body: &ynz_ast::nodes::Block,
) -> Result<(), String> {
    let iter_ty = cg.expr_type(iter);

    // M7 P4c: string iteration — `for (c in s)` where s: string.
    // Index-based: count code points, then loop 0..count calling ynz_string_codepoint_at.
    if matches!(iter_ty, Type::String) {
        let s_ptr = lower_expr(cg, iter)?.into_pointer_value();
        let cnt = cg
            .builder
            .build_call(cg.rt.ynz_string_count, &[s_ptr.into()], "si_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("string_count void")?
            .into_int_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "si_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("si_cond");
        let body_bb = cg.append_block("si_body");
        let after_bb = cg.append_block("si_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;
        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "si_i_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, cnt, "si_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let ch_ptr = cg
            .builder
            .build_call(
                cg.rt.ynz_string_codepoint_at,
                &[s_ptr.into(), i.into()],
                "si_cp",
            )
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("codepoint_at void")?;
        let var_slot = cg.alloca(&Type::String, var)?;
        cg.builder
            .build_store(var_slot, ch_ptr)
            .map_err(|e| format!("{e}"))?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "si_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.locals.remove(var);
        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // M7 P4c: user shape iteration — `for (x in obj)` where obj: Shape.
    // Calls the standalone `next(lend self: Shape) -> maybe<T>` function.
    if let Type::Shape { name: shape_name } = &iter_ty {
        let shape_name = shape_name.clone();
        let obj_ptr = lower_expr(cg, iter)?;

        let next_fn = cg.module.get_function("next").ok_or_else(|| {
            format!("codegen: shape `{shape_name}` follows Iterable<T> but `next` is not compiled")
        })?;

        let cond_bb = cg.append_block("uf_cond");
        let body_bb = cg.append_block("uf_body");
        let after_bb = cg.append_block("uf_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;
        cg.builder.position_at_end(cond_bb);

        // Call next(&obj) → maybe<T> (stored in a fresh alloca returned as ptr).
        let maybe_slot_ptr = cg
            .builder
            .build_call(next_fn, &[obj_ptr.into()], "uf_next")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("next() returned void")?
            .into_pointer_value();

        // Check has_value (slot 0).
        let tag_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), maybe_slot_ptr, 0, "uf_tag")
            .map_err(|e| format!("{e}"))?;
        let tag = cg
            .builder
            .build_load(cg.i64(), tag_gep, "uf_tag_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let has = cg
            .builder
            .build_int_compare(IntPredicate::NE, tag, cg.i64().const_zero(), "uf_has")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(has, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        // Extract the value (slot 1) and determine its type by looking at the typeck.
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), maybe_slot_ptr, 1, "uf_val")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "uf_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        // The element type is Int (as the most common case in our fixtures).
        // For P4c we emit the loop var as an Int slot; the compiler already typechecked the type.
        let var_slot = cg.alloca(&Type::Int, var)?;
        cg.builder
            .build_store(var_slot, bits)
            .map_err(|e| format!("{e}"))?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        cg.locals.remove(var);
        if !is_block_terminated(cg) {
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Array iteration: `for (x in arr)` where arr: array<T>.
    if let Type::BuiltinArray { elem } = &iter_ty {
        let elem = elem.as_ref().clone();
        let arr_ptr = lower_expr(cg, iter)?.into_pointer_value();
        let cnt = cg
            .builder
            .build_call(cg.rt.ynz_array_count, &[arr_ptr.into()], "for_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("array_count void")?
            .into_int_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "for_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("for_cond");
        let body_bb = cg.append_block("for_body");
        let after_bb = cg.append_block("for_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "i")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let cmp = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, cnt, "for_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(cmp, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let out = cg
            .builder
            .build_alloca(cg.maybe_type(), "for_get")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_call(
                cg.rt.ynz_array_get,
                &[arr_ptr.into(), i.into(), out.into()],
                "for_get_call",
            )
            .map_err(|e| format!("{e}"))?;
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), out, 1, "for_val")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "for_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let elem_val = cg.i64_bits_to(bits, &elem)?;

        let var_slot = cg.alloca(&elem, var)?;
        store(cg, elem_val, &elem, var_slot)?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "next_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.locals.remove(var);
        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Fixed array iteration: `for (x in arr)` where arr: fixed<T>.
    // Identical to BuiltinArray but uses compile-time size + direct GEP instead of runtime calls.
    if let Type::BuiltinFixed { elem, size } = &iter_ty {
        let elem = elem.as_ref().clone();
        let n = match size {
            Some(n) => *n as u64,
            None => return Err("codegen: cannot iterate fixed array with unknown size".to_string()),
        };
        let size_val = cg.i64().const_int(n, false);
        let arr_ptr = lower_expr(cg, iter)?.into_pointer_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "ff_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("ff_cond");
        let body_bb = cg.append_block("ff_body");
        let after_bb = cg.append_block("ff_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "ff_i_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, size_val, "ff_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let elem_gep = unsafe {
            cg.builder
                .build_gep(cg.i64(), arr_ptr, &[i], "ff_gep")
                .map_err(|e| format!("{e}"))?
        };
        let bits = cg
            .builder
            .build_load(cg.i64(), elem_gep, "ff_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let elem_val = cg.i64_bits_to(bits, &elem)?;
        let var_slot = cg.alloca(&elem, var)?;
        store(cg, elem_val, &elem, var_slot)?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "ff_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.locals.remove(var);
        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Map iteration: `for (entry in m)` where m: map<K,V>.
    // Iterates by insertion order; loop var has type MapEntry {key_bits, val_bits}.
    if let Type::BuiltinMap { key, val } = &iter_ty {
        let key_ty = key.as_ref().clone();
        let _val_ty = val.as_ref().clone();
        let map_ptr = lower_expr(cg, iter)?.into_pointer_value();

        let cnt = cg
            .builder
            .build_call(cg.rt.ynz_map_count, &[map_ptr.into()], "mf_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("map_count void")?
            .into_int_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "mf_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        // Loop var slot: {i64 key_bits, i64 val_bits}
        let entry_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into()], false);
        let entry_slot = cg
            .builder
            .build_alloca(entry_ty, var)
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("mfor_cond");
        let body_bb = cg.append_block("mfor_body");
        let after_bb = cg.append_block("mfor_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "mf_i_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, cnt, "mf_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let triple_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into(), cg.i64().into()], false);
        let triple_slot = cg
            .builder
            .build_alloca(triple_ty, "mf_triple")
            .map_err(|e| format!("{e}"))?;
        if key_is_string(&key_ty) {
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get_str,
                    &[map_ptr.into(), i.into(), triple_slot.into()],
                    "mf_iter_s",
                )
                .map_err(|e| format!("{e}"))?;
        } else {
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get,
                    &[map_ptr.into(), i.into(), triple_slot.into()],
                    "mf_iter",
                )
                .map_err(|e| format!("{e}"))?;
        }
        // triple = {has, key, val} — copy key[1] and val[2] into entry_slot
        let k_src = cg
            .builder
            .build_struct_gep(triple_ty, triple_slot, 1, "mf_ks")
            .map_err(|e| format!("{e}"))?;
        let v_src = cg
            .builder
            .build_struct_gep(triple_ty, triple_slot, 2, "mf_vs")
            .map_err(|e| format!("{e}"))?;
        let k_dst = cg
            .builder
            .build_struct_gep(entry_ty, entry_slot, 0, "mf_kd")
            .map_err(|e| format!("{e}"))?;
        let v_dst = cg
            .builder
            .build_struct_gep(entry_ty, entry_slot, 1, "mf_vd")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(
                k_dst,
                cg.builder
                    .build_load(cg.i64(), k_src, "mf_kv")
                    .map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(
                v_dst,
                cg.builder
                    .build_load(cg.i64(), v_src, "mf_vv")
                    .map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?;

        cg.locals.insert(var.to_string(), entry_slot);

        for s in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, s)?;
        }

        cg.locals.remove(var);

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "mf_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            emit_loop_preempt(cg)?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Range iteration: `for (x in range(n))` — original inline-range path,
    // OR `for (x in r)` where r is a stored Range value (M7 P4c first-class range).
    let (start_val, end_val) =
        if matches!(iter_ty, Type::Range { .. }) && !matches!(iter, Expr::Call(_)) {
            // Stored range variable: load the {i64, i64} struct pointer and extract fields.
            let range_ptr_val = lower_expr(cg, iter)?;
            let range_struct_ptr = range_ptr_val.into_pointer_value();
            let range_ty = cg
                .ctx
                .struct_type(&[cg.i64().into(), cg.i64().into()], false);
            let s_gep = cg
                .builder
                .build_struct_gep(range_ty, range_struct_ptr, 0, "r_s")
                .map_err(|e| format!("{e}"))?;
            let e_gep = cg
                .builder
                .build_struct_gep(range_ty, range_struct_ptr, 1, "r_e")
                .map_err(|e| format!("{e}"))?;
            let s = cg
                .builder
                .build_load(cg.i64(), s_gep, "r_sv")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let e = cg
                .builder
                .build_load(cg.i64(), e_gep, "r_ev")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            (s, e)
        } else {
            extract_range_bounds(cg, iter)?
        };

    let counter_slot = cg
        .builder
        .build_alloca(cg.i64(), "for_ctr")
        .map_err(|e| format!("{e}"))?;
    let end_slot = cg
        .builder
        .build_alloca(cg.i64(), "for_end")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(counter_slot, start_val)
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(end_slot, end_val)
        .map_err(|e| format!("{e}"))?;

    // Loop variable alloca — loop var is typed as int.
    let var_slot = cg
        .builder
        .build_alloca(cg.i64(), var)
        .map_err(|e| format!("{e}"))?;
    cg.locals.insert(var.to_string(), var_slot);

    let header_bb = cg.append_block("for_header");
    let body_bb = cg.append_block("for_body");
    let exit_bb = cg.append_block("for_exit");

    cg.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| format!("{e}"))?;

    // Header: check counter < end.
    cg.builder.position_at_end(header_bb);
    let ctr = cg
        .builder
        .build_load(cg.i64(), counter_slot, "ctr")
        .map_err(|e| format!("{e}"))?
        .into_int_value();
    let end = cg
        .builder
        .build_load(cg.i64(), end_slot, "end")
        .map_err(|e| format!("{e}"))?
        .into_int_value();
    let in_range = cg
        .builder
        .build_int_compare(IntPredicate::SLT, ctr, end, "for_cond")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(in_range, body_bb, exit_bb)
        .map_err(|e| format!("{e}"))?;

    // Body: bind loop var, emit stmts, increment, back-edge.
    cg.builder.position_at_end(body_bb);
    let ctr_bind = cg
        .builder
        .build_load(cg.i64(), counter_slot, "ctr_bind")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(var_slot, ctr_bind)
        .map_err(|e| format!("{e}"))?;

    for stmt in &body.stmts {
        if is_block_terminated(cg) {
            break;
        }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        let ctr_cur = cg
            .builder
            .build_load(cg.i64(), counter_slot, "ctr_cur")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let one = cg.i64().const_int(1, false);
        let ctr_next = cg
            .builder
            .build_int_add(ctr_cur, one, "ctr_next")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(counter_slot, ctr_next)
            .map_err(|e| format!("{e}"))?;
        emit_loop_preempt(cg)?;
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(exit_bb);
    cg.locals.remove(var);
    Ok(())
}

fn lower_stmt_return<'ctx>(cg: &mut Cg<'ctx, '_>, value: Option<&Expr>) -> Result<(), String> {
    // v0.3-M2 P7: inside a state-machine resume function, explicit `return expr` must
    // store the typed value in the frame's return_slot@16 and emit `ret i32 0` (Ready).
    // This is the fix for Bug A (value-returning SM) — the resume fn returns i32, not the Yinz type.
    if let (Some(frame_ptr), Some(ret_ty)) = (cg.sm_frame_ptr, cg.sm_yinz_ret_ty.clone()) {
        match value {
            None => {
                // `return` with no value: store 0 and signal Ready.
                state_machine::store_return_value_i64(
                    cg.ctx,
                    &cg.builder,
                    frame_ptr,
                    cg.ctx.i64_type().const_int(0, false),
                )?;
            }
            Some(expr) => {
                let val = lower_expr(cg, expr)?;
                let val_ty = cg.expr_type(expr);
                // Detect errors-capable return: the Yinz return type is ErrorsCapable, OR
                // the actual value is a struct (from __testFallibleAsync or a nested SM errors call).
                let is_ec_return = matches!(val, inkwell::values::BasicValueEnum::StructValue(_));
                if is_ec_return {
                    // errors-capable return: val may be {i64, i64} struct (unauto-propagated),
                    // OR an IntValue (auto-propagated success value after `.failed()` narrowing).
                    match val {
                        inkwell::values::BasicValueEnum::StructValue(sv) => {
                            let err_i64 = cg
                                .builder
                                .build_extract_value(sv, 0, "sm_ret_err")
                                .map_err(|e| format!("sm ret err extract: {e}"))?
                                .into_int_value();
                            let ok_i64 = cg
                                .builder
                                .build_extract_value(sv, 1, "sm_ret_ok")
                                .map_err(|e| format!("sm ret ok extract: {e}"))?
                                .into_int_value();
                            state_machine::store_return_value_errors(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                err_i64,
                                ok_i64,
                            )?;
                        }
                        _ => {
                            // Auto-propagated: val is the success value (no error).
                            let success_bits = cg
                                .to_i64_bits(val, &val_ty)
                                .unwrap_or_else(|_| cg.i64().const_int(0, false));
                            state_machine::store_return_value_errors(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                cg.i64().const_int(0, false), // no error (ptr=null)
                                success_bits,
                            )?;
                        }
                    }
                } else {
                    match &ret_ty {
                        // ErrorsCapable: val_ty tells us whether the PointerValue is the EC
                        // struct itself (val_ty == ErrorsCapable) or the success value
                        // (val_ty == inner type). The two cases require different handling:
                        // - ErrorsCapable val_ty: ptr → {i64,i64} EC struct; load and extract.
                        // - Non-ErrorsCapable val_ty: ptr IS the success value; wrap as {0, ok}.
                        //   Wide inner types (Number/Shape) need heap-stable staging because the
                        //   pointer may be into the resume fn's stack (dies when resume returns).
                        //   String/Array/Map are already heap-allocated — safe to ptr_to_int.
                        Type::ErrorsCapable { inner } => {
                            match val {
                                inkwell::values::BasicValueEnum::PointerValue(ptr) => {
                                    if matches!(val_ty, Type::ErrorsCapable { .. }) {
                                        // val_ty == ErrorsCapable: ptr points to the {i64,i64}
                                        // EC result struct (an errors-capable local or call result).
                                        // Load and extract the two fields directly.
                                        let result_ty = errors_result_type(cg.ctx);
                                        let ec_struct = cg
                                            .builder
                                            .build_load(result_ty, ptr, "sm_ret_ec_struct")
                                            .map_err(|e| format!("sm_ret_ec_struct: {e}"))?
                                            .into_struct_value();
                                        let err_i64 = cg
                                            .builder
                                            .build_extract_value(ec_struct, 0, "sm_ret_ec_err")
                                            .map_err(|e| format!("sm_ret_ec_err: {e}"))?
                                            .into_int_value();
                                        let ok_i64 = cg
                                            .builder
                                            .build_extract_value(ec_struct, 1, "sm_ret_ec_ok")
                                            .map_err(|e| format!("sm_ret_ec_ok: {e}"))?
                                            .into_int_value();
                                        state_machine::store_return_value_errors(
                                            cg.ctx,
                                            &cg.builder,
                                            frame_ptr,
                                            err_i64,
                                            ok_i64,
                                        )?;
                                    } else {
                                        // val_ty is the inner success type (Number, Shape, String,
                                        // Array, Map, etc.): ptr IS the success value.
                                        // Wrap as {err=null, ok=ptr_to_int(stable_ptr)}.
                                        // Wide inner types (Number/Shape) need heap-stable staging.
                                        // Number: write the i128 into the dedicated 16-byte staging
                                        // slot in the composed frame, then point ok at that slot.
                                        // Shape: still rejected by WideValueSuspendingReturn at typeck.
                                        let ok_i64 = match inner.as_ref() {
                                            Type::Number { precision } if *precision <= 34 => {
                                                // The decimal128 value is a pointer to an i128 alloca
                                                // on the resume fn's stack. That stack dies when the
                                                // resume fn returns. Store the i128 in the dedicated
                                                // 16-byte staging slot in the composed frame so the
                                                // ok-pointer remains valid when the wrapper reads it.
                                                let staging_offset = cg
                                                    .sm_number_errors_staging_offset
                                                    .ok_or_else(|| {
                                                        "ICE: `-> number errors` SM return: \
                                                         sm_number_errors_staging_offset is None — \
                                                         build_frame_layouts must have omitted the \
                                                         staging slot for this function"
                                                            .to_string()
                                                    })?;
                                                let staging_ptr =
                                                    state_machine::number_errors_staging_ptr(
                                                        cg.ctx,
                                                        &cg.builder,
                                                        frame_ptr,
                                                        staging_offset,
                                                    )?;
                                                // Load the i128 from the stack-alloca pointer and
                                                // store it into the frame-stable staging slot.
                                                let i128_val = cg
                                                    .builder
                                                    .build_load(
                                                        cg.ctx.i128_type(),
                                                        ptr,
                                                        "num_err_i128",
                                                    )
                                                    .map_err(|e| format!("num err i128 load: {e}"))?
                                                    .into_int_value();
                                                cg.builder
                                                    .build_store(staging_ptr, i128_val)
                                                    .map_err(|e| {
                                                        format!("num err staging store: {e}")
                                                    })?;
                                                // The EC ok-word is the staging slot address as i64.
                                                cg.builder
                                                    .build_ptr_to_int(
                                                        staging_ptr,
                                                        cg.ctx.i64_type(),
                                                        "num_err_ok_i64",
                                                    )
                                                    .map_err(|e| {
                                                        format!("num err ok ptr_to_int: {e}")
                                                    })?
                                            }
                                            Type::Shape { .. } => {
                                                // WideValueSuspendingReturn at typeck rejects every
                                                // `-> Shape errors` suspending return before codegen.
                                                // Reaching here means the guard was bypassed — fail
                                                // loud so the ICE is visible immediately.
                                                return Err(
                                                    "ICE: `Shape errors` return from a suspending \
                                                     function reached codegen — \
                                                     WideValueSuspendingReturn typeck guard should \
                                                     have rejected it"
                                                        .to_string(),
                                                );
                                            }
                                            // String, Array, Map, Maybe, Union: all heap-allocated
                                            // (global literals or ynz_alloc). The pointer survives
                                            // resume fn return — ptr_to_int is safe.
                                            _ => cg
                                                .builder
                                                .build_ptr_to_int(
                                                    ptr,
                                                    cg.ctx.i64_type(),
                                                    "sm_ec_ptr_ok",
                                                )
                                                .map_err(|e| format!("sm_ec_ptr_ok p2i: {e}"))?,
                                        };
                                        state_machine::store_return_value_errors(
                                            cg.ctx,
                                            &cg.builder,
                                            frame_ptr,
                                            cg.ctx.i64_type().const_int(0, false),
                                            ok_i64,
                                        )?;
                                    }
                                }
                                _ => {
                                    // Non-pointer success value (Int/Bool/Float): wrap as {0, bits}.
                                    let success_bits = cg
                                        .to_i64_bits(val, inner)
                                        .unwrap_or_else(|_| cg.i64().const_int(0, false));
                                    state_machine::store_return_value_errors(
                                        cg.ctx,
                                        &cg.builder,
                                        frame_ptr,
                                        cg.i64().const_int(0, false),
                                        success_bits,
                                    )?;
                                }
                            }
                        }
                        Type::Int | Type::Bool => {
                            let i64v = val.into_int_value();
                            let wide = if i64v.get_type() != cg.ctx.i64_type() {
                                cg.builder
                                    .build_int_z_extend(i64v, cg.ctx.i64_type(), "sm_ret_widen")
                                    .map_err(|e| format!("sm ret widen: {e}"))?
                            } else {
                                i64v
                            };
                            state_machine::store_return_value_i64(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                wide,
                            )?;
                        }
                        // Bare `-> Shape` from a suspending function.
                        //
                        // WideValueSuspendingReturn at typeck rejects every such return before
                        // codegen reaches here. Reaching this arm means the guard was bypassed —
                        // fail loud so the ICE is visible immediately rather than producing a
                        // silent SIGSEGV from staging at FRAME_OFFSET_LOCALS_START.
                        Type::Shape { .. } => {
                            return Err(
                                "ICE: bare `Shape` return from a suspending function reached \
                                 codegen — WideValueSuspendingReturn typeck guard should have \
                                 rejected it"
                                    .to_string(),
                            );
                        }
                        Type::Float => {
                            // Float: bitcast f64 → i64, store in the first 8 bytes of the
                            // 16-byte return slot. The wrapper-return load does the inverse
                            // bitcast. Storing the raw f64 directly into an i64-typed slot
                            // pointer would produce an LLVM type mismatch at verification.
                            let f_val = val.into_float_value();
                            state_machine::store_return_value_f64(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                f_val,
                            )?;
                        }
                        Type::Number { precision } if *precision <= 34 => {
                            // Decimal128 (i128): load the full 16-byte value through the
                            // pointer (lower_expr returns a ptr-to-i128 alloca for number
                            // values), then store the i128 directly into the 16-byte return
                            // slot. Storing the pointer itself (as to_i64_bits would do via
                            // ptr_to_int) would write a stack address that becomes invalid
                            // once this resume function returns — Tier-A silent-wrong-value bug.
                            let ptr_v = val.into_pointer_value();
                            let i128_val = cg
                                .builder
                                .build_load(cg.ctx.i128_type(), ptr_v, "sm_ret_dec_load")
                                .map_err(|e| format!("sm ret number i128 load: {e}"))?
                                .into_int_value();
                            state_machine::store_return_value_i128(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                i128_val,
                            )?;
                        }
                        Type::String
                        | Type::BuiltinArray { .. }
                        | Type::BuiltinFixed { .. }
                        | Type::Maybe { .. }
                        | Type::BuiltinMap { .. }
                        | Type::Union { .. }
                        | Type::Sensitive { .. } => {
                            let ptr_v = val.into_pointer_value();
                            let as_i64 = cg
                                .builder
                                .build_ptr_to_int(ptr_v, cg.ctx.i64_type(), "sm_ret_ptr_i64")
                                .map_err(|e| format!("sm ret ptr_to_int: {e}"))?;
                            state_machine::store_return_value_i64(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                as_i64,
                            )?;
                        }
                        _ => {
                            // Fallback: try to_i64_bits.
                            let bits = cg
                                .to_i64_bits(val, &val_ty)
                                .unwrap_or_else(|_| cg.ctx.i64_type().const_int(0, false));
                            state_machine::store_return_value_i64(
                                cg.ctx,
                                &cg.builder,
                                frame_ptr,
                                bits,
                            )?;
                        }
                    }
                }
            }
        }
        cg.builder
            .build_return(Some(&cg.ctx.i32_type().const_int(0, false)))
            .map_err(|e| format!("sm resume ready ret: {e}"))?;
        return Ok(());
    }

    if cg.is_main {
        // Drain in-flight background tasks before exiting.
        cg.builder
            .build_call(cg.rt.ynz_rt_shutdown, &[], "rt_shutdown")
            .map_err(|e| format!("rt_shutdown: {e}"))?;
        cg.builder
            .build_return(Some(&cg.i32().const_int(0, false)))
            .map_err(|e| format!("main ret: {e}"))?;
        return Ok(());
    }
    if cg.is_errors_capable {
        // M7 P4a: errors-capable return wraps the success value in {0, success_bits}.
        // Always pop the frame before returning.
        cg.builder
            .build_call(cg.rt.ynz_frame_pop, &[], "ec_pop")
            .map_err(|e| format!("frame_pop on return: {e}"))?;
        match value {
            None => {
                // `-> nothing errors` function returning without a value.
                let zero_result = errors_result_type(cg.ctx).const_zero();
                cg.builder
                    .build_return(Some(&zero_result))
                    .map_err(|e| format!("ec void ret: {e}"))?;
            }
            Some(expr) => {
                let val = lower_expr(cg, expr)?;
                let val_ty = cg.expr_type(expr);
                let success_bits = cg
                    .to_i64_bits(val, &val_ty)
                    .unwrap_or_else(|_| cg.i64().const_int(0, false));
                let result_ty = errors_result_type(cg.ctx);
                let mut result = result_ty.const_zero();
                result = cg
                    .builder
                    .build_insert_value(result, cg.i64().const_int(0, false), 0, "ec_err0")
                    .map_err(|e| format!("ec insert err: {e}"))?
                    .into_struct_value();
                result = cg
                    .builder
                    .build_insert_value(result, success_bits, 1, "ec_val")
                    .map_err(|e| format!("ec insert val: {e}"))?
                    .into_struct_value();
                cg.builder
                    .build_return(Some(&result))
                    .map_err(|e| format!("ec success ret: {e}"))?;
            }
        }
        return Ok(());
    }
    match value {
        None => {
            cg.builder
                .build_return(None)
                .map_err(|e| format!("void ret: {e}"))?;
        }
        Some(expr) => {
            let val = lower_expr(cg, expr)?;
            cg.builder
                .build_return(Some(&val))
                .map_err(|e| format!("ret: {e}"))?;
        }
    }
    Ok(())
}

/// Extract the start and end `i64` values from a `range(end)` or `range(start, end)` call.
fn extract_range_bounds<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    iter: &Expr,
) -> Result<
    (
        inkwell::values::IntValue<'ctx>,
        inkwell::values::IntValue<'ctx>,
    ),
    String,
> {
    let Expr::Call(call) = iter else {
        return Err("for-loop iter is not a call expression".to_string());
    };
    let Expr::Ident(name, _) = &call.callee else {
        return Err("for-loop iter callee is not an identifier".to_string());
    };
    if name != "range" {
        return Err(format!("for-loop iter calls `{name}`, expected `range`"));
    }
    match call.args.len() {
        1 => {
            let end = lower_expr(cg, &call.args[0])?.into_int_value();
            Ok((cg.i64().const_int(0, false), end))
        }
        2 => {
            let start = lower_expr(cg, &call.args[0])?.into_int_value();
            let end = lower_expr(cg, &call.args[1])?.into_int_value();
            Ok((start, end))
        }
        n => Err(format!("range takes 1 or 2 args, got {n}")),
    }
}

fn lower_expr<'ctx>(cg: &mut Cg<'ctx, '_>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, String> {
    match expr {
        Expr::IntLit(n, _) => Ok(cg.i64().const_int(*n as u64, true).into()),

        Expr::NumberLit(s, _) => {
            let ty = cg.expr_type(expr);
            // M8 P6: bignum path for N > 34.
            if let Type::Number { precision } = &ty {
                if *precision > 34 {
                    let prec16 = (*precision).min(4096) as u16;
                    let bn = ynz_numerics::decimal_n::parse_bignum(s, prec16)
                        .unwrap_or_else(|| ynz_numerics::BigNum::zero(prec16));
                    let formatted = ynz_numerics::decimal_n::format_bignum(&bn);
                    let global = build_string_global(
                        cg.ctx,
                        cg.module,
                        &formatted,
                        &format!(".bignum.lit.{}", &s[..s.len().min(8)]),
                    );
                    return Ok(global.as_pointer_value().into());
                }
            }
            // Hardware decimal128 path (N ≤ 34): stack alloca holds i128 bits.
            let bits: u128 =
                ynz_numerics::parse(s).ok_or_else(|| format!("bad decimal literal `{s}`"))?;
            let slot = cg
                .builder
                .build_alloca(cg.i128(), "dec_lit")
                .map_err(|e| format!("{e}"))?;
            let const_val = cg.i128().const_int_arbitrary_precision(&[
                (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
                (bits >> 64) as u64,
            ]);
            cg.builder
                .build_store(slot, const_val)
                .map_err(|e| format!("{e}"))?;
            Ok(slot.into())
        }

        Expr::BoolLit(b, _) => Ok(cg.bool().const_int(*b as u64, false).into()),

        Expr::StringLit(bytes, _) => {
            let mut null = bytes.clone();
            push_c_string_terminator(&mut null);
            let i8t = cg.i8();
            let arr_ty = i8t.array_type(null.len() as u32);
            let arr = i8t.const_array(
                &null
                    .iter()
                    .map(|&b| i8t.const_int(b as u64, false))
                    .collect::<Vec<_>>(),
            );
            let g = cg
                .module
                .add_global(arr_ty, Some(AddressSpace::default()), "str");
            g.set_initializer(&arr);
            g.set_constant(true);
            g.set_linkage(inkwell::module::Linkage::Private);
            g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
            Ok(g.as_pointer_value().into())
        }

        Expr::Ident(name, _) => {
            let slot = *cg
                .locals
                .get(name.as_str())
                .ok_or_else(|| format!("undefined `{name}` in codegen"))?;
            let ty = cg.expr_type(expr);

            // M7 P4a: handle errors-capable locals.
            //
            // A local in `errors_capable_locals` holds a pointer to {i64, i64} result struct.
            // The typeck's expr_type may be:
            //   - `ErrorsCapable { inner }` — still unhandled; load ptr for .or()/.failed()
            //   - inner type (String, int, etc.) — typeck narrowed it; extract success value
            //
            // Auto-propagation fires here when the caller IS errors-capable AND the type
            // is still ErrorsCapable (typeck hasn't narrowed it yet).
            if cg.errors_capable_locals.contains(name.as_str()) {
                if matches!(ty, Type::ErrorsCapable { .. }) && cg.is_errors_capable {
                    // Auto-propagation: load struct, check error, early-return or yield success.
                    let ec_ptr = cg
                        .builder
                        .build_load(cg.ptr(), slot, "ec_id_ptr")
                        .map_err(|e| format!("ec ident load: {e}"))?
                        .into_pointer_value();
                    let result_ty = errors_result_type(cg.ctx);
                    let result_struct = cg
                        .builder
                        .build_load(result_ty, ec_ptr, "ec_id_struct")
                        .map_err(|e| format!("ec ident struct load: {e}"))?
                        .into_struct_value();
                    cg.errors_capable_locals.remove(name.as_str());
                    let inner_ty = if let Type::ErrorsCapable { inner } = &ty {
                        *inner.clone()
                    } else {
                        ty.clone()
                    };
                    let success_val = lower_ec_auto_propagate(cg, result_struct, &inner_ty)?;
                    let new_slot = cg.alloca(&inner_ty, &format!("{name}_ec_inner"))?;
                    store(cg, success_val, &inner_ty, new_slot)?;
                    cg.locals.insert(name.to_string(), new_slot);
                    return Ok(success_val);
                } else if !matches!(ty, Type::ErrorsCapable { .. }) {
                    // Typeck narrowed the binding to the inner type (after a .failed() check or
                    // after auto-propagation in an errors-capable caller).
                    //
                    // SM resume context: is_errors_capable=false in the resume fn so the normal
                    // auto-propagation arm above doesn't fire. Use the SM return mechanism instead
                    // (store to frame return slot + ret i32 0 on the error path, yield the success
                    // Int on the success path). This covers both `v + 10` (arithmetic caller needs
                    // the Int) and `return v` (lower_stmt_return gets the Int and wraps it as
                    // {0, success} — correct because the error was already propagated here).
                    // No ynz_frame_pop: the resume fn never called ynz_frame_push.
                    if cg.sm_frame_ptr.is_some() {
                        let frame_ptr = cg.sm_frame_ptr.expect("sm_frame_ptr confirmed Some above");
                        let ec_ptr = cg
                            .builder
                            .build_load(cg.ptr(), slot, "sm_ec_narrow_ptr")
                            .map_err(|e| format!("sm ec narrow ptr load: {e}"))?
                            .into_pointer_value();
                        let result_ty = errors_result_type(cg.ctx);
                        let result_struct = cg
                            .builder
                            .build_load(result_ty, ec_ptr, "sm_ec_narrow_struct")
                            .map_err(|e| format!("sm ec narrow struct load: {e}"))?
                            .into_struct_value();
                        let err_ptr_i64 = cg
                            .builder
                            .build_extract_value(result_struct, 0, "sm_nap_err")
                            .map_err(|e| format!("sm nap extract err: {e}"))?
                            .into_int_value();
                        let is_err = cg
                            .builder
                            .build_int_compare(
                                IntPredicate::NE,
                                err_ptr_i64,
                                cg.i64().const_int(0, false),
                                "sm_nap_is_err",
                            )
                            .map_err(|e| format!("sm nap icmp: {e}"))?;
                        let propagate_bb = cg.append_block("sm_nap_propagate");
                        let success_bb = cg.append_block("sm_nap_success");
                        cg.builder
                            .build_conditional_branch(is_err, propagate_bb, success_bb)
                            .map_err(|e| format!("sm nap branch: {e}"))?;
                        // Error path: store {error_ptr, 0} to frame return slot, signal Ready.
                        cg.builder.position_at_end(propagate_bb);
                        state_machine::store_return_value_errors(
                            cg.ctx,
                            &cg.builder,
                            frame_ptr,
                            err_ptr_i64,
                            cg.i64().const_int(0, false),
                        )?;
                        cg.builder
                            .build_return(Some(&cg.ctx.i32_type().const_int(0, false)))
                            .map_err(|e| format!("sm nap error ret: {e}"))?;
                        // Success path: extract field1 (success bits), update slot to inner type.
                        cg.builder.position_at_end(success_bb);
                        let success_bits = cg
                            .builder
                            .build_extract_value(result_struct, 1, "sm_nap_val")
                            .map_err(|e| format!("sm nap extract val: {e}"))?
                            .into_int_value();
                        let success_val = cg.i64_bits_to(success_bits, &ty)?;
                        cg.errors_capable_locals.remove(name.as_str());
                        let new_slot = cg.alloca(&ty, &format!("{name}_ec_narrowed"))?;
                        store(cg, success_val, &ty, new_slot)?;
                        cg.locals.insert(name.to_string(), new_slot);
                        return Ok(success_val);
                    }

                    let ec_ptr = cg
                        .builder
                        .build_load(cg.ptr(), slot, "ec_id_ptr")
                        .map_err(|e| format!("ec ident narrow load: {e}"))?
                        .into_pointer_value();
                    let result_ty = errors_result_type(cg.ctx);
                    let success_gep = cg
                        .builder
                        .build_struct_gep(result_ty, ec_ptr, 1, "ec_narrow_val")
                        .map_err(|e| format!("ec narrow gep: {e}"))?;
                    let bits = cg
                        .builder
                        .build_load(cg.i64(), success_gep, "ec_narrow_bits")
                        .map_err(|e| format!("ec narrow bits: {e}"))?
                        .into_int_value();
                    let success_val = cg.i64_bits_to(bits, &ty)?;
                    // Update the slot so future uses don't do this extraction again.
                    cg.errors_capable_locals.remove(name.as_str());
                    let new_slot = cg.alloca(&ty, &format!("{name}_ec_narrowed"))?;
                    store(cg, success_val, &ty, new_slot)?;
                    cg.locals.insert(name.to_string(), new_slot);
                    return Ok(success_val);
                }
                // If ErrorsCapable AND not errors-capable caller: fall through to load
                // which returns the ptr for .or()/.failed() method dispatch.
            }

            load(cg, slot, &ty, name)
        }

        Expr::BinOp { op, lhs, rhs, .. } => {
            let lhs_ty = cg.expr_type(lhs);
            let rhs_ty = cg.expr_type(rhs);
            lower_binop(cg, op, lhs, rhs, &lhs_ty, &rhs_ty)
        }

        Expr::UnaryOp { op, operand, .. } => {
            let ty = cg.expr_type(operand);
            let val = lower_expr(cg, operand)?;
            lower_unary(cg, op, val, &ty)
        }

        Expr::Call(call) => {
            let Expr::Ident(fn_name, _) = &call.callee else {
                return Err("codegen: call to non-identifier callee".to_string());
            };
            let fn_name = fn_name.clone();
            match fn_name.as_str() {
                "print" if call.args.len() == 1 => {
                    let ty = cg.expr_type(&call.args[0]);
                    let val = lower_expr(cg, &call.args[0])?;
                    lower_print(cg, val, &ty)?;
                    Ok(cg.i32().const_int(0, false).into())
                }
                // M8 P4: `sensitive(value)` — identity at the ABI level (string ptr passes through).
                // The type system already marked the return type as Sensitive; codegen just returns
                // the underlying value unchanged.
                "sensitive" if call.args.len() == 1 => {
                    let val = lower_expr(cg, &call.args[0])?;
                    Ok(val)
                }
                // v0.3-M1: sleepBlocking(ms: int) — synchronous blocking sleep; lowers to ynz_thread_sleep_ms.
                "sleepBlocking" if call.args.len() == 1 => {
                    let ms = lower_expr(cg, &call.args[0])?.into_int_value();
                    cg.builder
                        .build_call(cg.rt.ynz_thread_sleep_ms, &[ms.into()], "sleepBlocking")
                        .map_err(|e| format!("{e}"))?;
                    Ok(cg.i32().const_int(0, false).into())
                }
                "range" => {
                    // M7 P4c: range() as a first-class value — produces a {i64 start, i64 end}
                    // alloca on the stack.  The pointer to that alloca is returned so the range
                    // can be stored in a variable binding and later used as a for-loop iter.
                    let (start_v, end_v) = match call.args.len() {
                        1 => {
                            let e = lower_expr(cg, &call.args[0])?.into_int_value();
                            (cg.i64().const_zero(), e)
                        }
                        2 => {
                            let s = lower_expr(cg, &call.args[0])?.into_int_value();
                            let e = lower_expr(cg, &call.args[1])?.into_int_value();
                            (s, e)
                        }
                        n => return Err(format!("range takes 1 or 2 args, got {n}")),
                    };
                    let range_ty = cg
                        .ctx
                        .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                    let slot = cg
                        .builder
                        .build_alloca(range_ty, "range_val")
                        .map_err(|e| format!("{e}"))?;
                    let s_ptr = cg
                        .builder
                        .build_struct_gep(range_ty, slot, 0, "rng_s")
                        .map_err(|e| format!("{e}"))?;
                    let e_ptr = cg
                        .builder
                        .build_struct_gep(range_ty, slot, 1, "rng_e")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(s_ptr, start_v)
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(e_ptr, end_v)
                        .map_err(|e| format!("{e}"))?;
                    Ok(slot.into())
                }
                // v0.3-M2: sleep(ms: int) — non-blocking sleep; lowers to state-machine wait point.
                // Only reaches here when called WITHOUT `wait` wrapping (e.g., bare `sleep(100)`).
                // With `wait`, the call is handled by emit_wait_point / lower_sm_body.
                // Without `wait`, evaluate to a no-op (discards the sleep handle immediately).
                // WHY: typeck emits `unawaited_sleep` warning for this case; codegen still
                // needs to produce valid IR without crashing.
                "sleep" if call.args.len() == 1 => {
                    // Evaluate the ms argument for side effects but discard the sleep handle.
                    let ms = lower_expr(cg, &call.args[0])?.into_int_value();
                    let handle = cg
                        .builder
                        .build_call(
                            cg.rt.ynz_rt_async_sleep_create,
                            &[ms.into()],
                            "sleep_handle",
                        )
                        .map_err(|e| format!("sleep: {e}"))?
                        .try_as_basic_value()
                        .basic()
                        .ok_or("ynz_rt_async_sleep_create void")?;
                    // Immediately discard the handle — typeck warned the user; this is intentional.
                    let _ = handle;
                    Ok(cg.i32().const_int(0, false).into())
                }
                // v0.3-M2: __testFallibleAsync(should_fail: bool) -> int errors
                // Internal test intrinsic for validating errors-cascade through suspension.
                // Returns {0, 42} (success, value=42) when should_fail=false,
                // {error_ptr, 0} (error) when should_fail=true.
                // This is a codegen-only simulation of a fallible async intrinsic.
                "__testFallibleAsync" if call.args.len() == 1 => {
                    let should_fail = lower_expr(cg, &call.args[0])?.into_int_value();
                    // Build {i64, i64} result: field0=error_ptr, field1=success_value
                    let result_ty = errors_result_type(cg.ctx);
                    // Success path: {0, 42}
                    let success_result = {
                        let mut r = result_ty.const_zero();
                        r = cg
                            .builder
                            .build_insert_value(r, cg.i64().const_int(0, false), 0, "tf_ok_err")
                            .map_err(|e| format!("tf ok err: {e}"))?
                            .into_struct_value();
                        r = cg
                            .builder
                            .build_insert_value(r, cg.i64().const_int(42, false), 1, "tf_ok_val")
                            .map_err(|e| format!("tf ok val: {e}"))?
                            .into_struct_value();
                        r
                    };
                    // Failure path: allocate an error string
                    let err_msg = build_string_global(
                        cg.ctx,
                        cg.module,
                        "testFallibleAsync error",
                        ".tfa.err",
                    );
                    let err_ptr = cg
                        .builder
                        .build_call(
                            cg.rt.ynz_error_new,
                            &[err_msg.as_pointer_value().into()],
                            "tfa_err",
                        )
                        .map_err(|e| format!("tfa err new: {e}"))?
                        .try_as_basic_value()
                        .basic()
                        .ok_or("ynz_error_new returned void")?;
                    let err_as_i64 = cg
                        .builder
                        .build_ptr_to_int(err_ptr.into_pointer_value(), cg.i64(), "tfa_err_i64")
                        .map_err(|e| format!("tfa ptr_to_int: {e}"))?;
                    let error_result = {
                        let mut r = result_ty.const_zero();
                        r = cg
                            .builder
                            .build_insert_value(r, err_as_i64, 0, "tf_err_ptr")
                            .map_err(|e| format!("tf err_ptr: {e}"))?
                            .into_struct_value();
                        r
                    };
                    // Select between success and error based on should_fail.
                    // For each field, select the appropriate value.
                    let tf_true = cg.ctx.bool_type().const_int(1, false);
                    // Compare against zero of the same type as should_fail.
                    let zero_same_ty = should_fail.get_type().const_zero();
                    let should_fail_i1 = cg
                        .builder
                        .build_int_compare(IntPredicate::NE, should_fail, zero_same_ty, "tf_cmp")
                        .map_err(|e| format!("tf cmp: {e}"))?;
                    let field0_success = cg
                        .builder
                        .build_extract_value(success_result, 0, "tf_s0")
                        .map_err(|e| format!("tf s0: {e}"))?
                        .into_int_value();
                    let field0_error = cg
                        .builder
                        .build_extract_value(error_result, 0, "tf_e0")
                        .map_err(|e| format!("tf e0: {e}"))?
                        .into_int_value();
                    let field1_success = cg
                        .builder
                        .build_extract_value(success_result, 1, "tf_s1")
                        .map_err(|e| format!("tf s1: {e}"))?
                        .into_int_value();
                    let field0 = cg
                        .builder
                        .build_select(should_fail_i1, field0_error, field0_success, "tf_f0")
                        .map_err(|e| format!("tf_f0 sel: {e}"))?
                        .into_int_value();
                    let field1 = cg
                        .builder
                        .build_select(
                            should_fail_i1,
                            cg.i64().const_zero(),
                            field1_success,
                            "tf_f1",
                        )
                        .map_err(|e| format!("tf_f1 sel: {e}"))?
                        .into_int_value();
                    let _ = tf_true; // suppress unused warning
                    let mut final_result = result_ty.const_zero();
                    final_result = cg
                        .builder
                        .build_insert_value(final_result, field0, 0, "tf_r0")
                        .map_err(|e| format!("tf_r0: {e}"))?
                        .into_struct_value();
                    final_result = cg
                        .builder
                        .build_insert_value(final_result, field1, 1, "tf_r1")
                        .map_err(|e| format!("tf_r1: {e}"))?
                        .into_struct_value();
                    // Route through lower_errors_capable_call_result to get a pointer
                    // representation consistent with how all other errors-capable calls work.
                    lower_errors_capable_call_result(cg, final_result, "__testFallibleAsync")
                }
                name => {
                    // Prefer the direct name. If not found, check for an aliased import
                    // (`import { getValue as fetchVal }`) — the LLVM module declares the
                    // function under the original exported name, not the local alias.
                    // Fall back to monomorphization lookup, then the name itself.
                    let effective_name = if cg.module.get_function(name).is_some() {
                        name.to_string()
                    } else if let Some(orig) = cg
                        .imported_fns
                        .get(name)
                        .and_then(|sig| sig.original_name.as_deref())
                    {
                        orig.to_string()
                    } else {
                        // Infer concrete arg types from the call site to pick the right mono.
                        let arg_types: Vec<Type> =
                            call.args.iter().map(|a| cg.expr_type(a)).collect();
                        find_mono_name_by_args(cg.mono_table, name, &arg_types)
                            .unwrap_or_else(|| name.to_string())
                    };

                    // v0.3-M2 P7: suspending calls inside SM resume bodies are handled by
                    // lower_sm_stmt_with_wait (inline-poll-and-yield) BEFORE reaching lower_expr.
                    // If a suspending call reaches here, the caller is a non-SM function calling
                    // a suspending wrapper — invoke the wrapper fn directly (it drives the SM
                    // internally via RUNTIME.block_on). This path should only be reached from
                    // non-SM callers; SM callers route through lower_sm_stmt_with_wait instead.
                    //
                    // Per AC 9: no ynz_rt_run_entrypoint inside any ynz_sm_*_resume.
                    // The program-entry driver lives in the WRAPPER function only; this path
                    // calls the wrapper fn.

                    let fn_val = cg.module.get_function(&effective_name).ok_or_else(|| {
                        format!("codegen: function `{effective_name}` not found in module")
                    })?;
                    let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
                    for arg in &call.args {
                        let val = lower_expr(cg, arg)?;
                        call_args.push(val.into());
                    }
                    let call_site = cg
                        .builder
                        .build_call(fn_val, &call_args, "call")
                        .map_err(|e| format!("call {effective_name}: {e}"))?;

                    // M7 P4a: if callee is errors-capable, handle the {i64, i64} result.
                    let callee_is_ec =
                        is_errors_capable_fn(cg.typed, cg.imported_fns, &effective_name);
                    if callee_is_ec {
                        let result_struct = call_site
                            .try_as_basic_value()
                            .basic()
                            .ok_or_else(|| {
                                format!("errors-capable call `{effective_name}` returned void")
                            })?
                            .into_struct_value();
                        return lower_errors_capable_call_result(
                            cg,
                            result_struct,
                            &effective_name,
                        );
                    }

                    match call_site.try_as_basic_value().basic() {
                        Some(val) => Ok(val),
                        None => Ok(cg.i32().const_int(0, false).into()),
                    }
                }
            }
        }

        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;
            match &recv_ty {
                Type::Shape { name } => {
                    let name = name.clone();
                    lower_ufcs_call(cg, recv_val, &name, method, args)
                }
                Type::Dynamic { .. } => {
                    // Dynamic dispatch via vtable — deferred post-P5.
                    Err("codegen: dynamic dispatch call sites not yet lowered in M4 P4".to_string())
                }
                Type::BuiltinArray { elem } => {
                    let elem = elem.as_ref().clone();
                    lower_array_method(cg, recv_val.into_pointer_value(), &elem, method, args)
                }
                Type::BuiltinFixed { elem, size } => {
                    let elem = elem.as_ref().clone();
                    let sz = *size;
                    lower_fixed_method(cg, recv_val.into_pointer_value(), &elem, sz, method, args)
                }
                Type::Maybe { inner } => {
                    let inner = inner.as_ref().clone();
                    lower_maybe_method(cg, recv_val.into_pointer_value(), &inner, method, args)
                }
                Type::BuiltinMap { key, val } => {
                    let key_ty = key.as_ref().clone();
                    let val_ty = val.as_ref().clone();
                    lower_map_method(
                        cg,
                        recv_val.into_pointer_value(),
                        &key_ty,
                        &val_ty,
                        method,
                        args,
                    )
                }
                // M6: options .toString()
                Type::Options { name: opts_name } => {
                    if method == "toString" {
                        lower_options_to_string(cg, recv_val, opts_name.as_str())
                    } else {
                        Err(format!(
                            "codegen: unknown method `{method}` on options type `{opts_name}`"
                        ))
                    }
                }
                // M7 P4a: ErrorsCapable — .failed() and .or(default).
                // The receiver is a pointer to a heap-allocated {i64 error_ptr, i64 success_val}
                // stored as a pointer in a local alloca.
                Type::ErrorsCapable { inner } => {
                    let inner = inner.as_ref().clone();
                    lower_errors_capable_method(cg, recv_val, &inner, method, args)
                }
                // M7 P4b: string methods — dispatched through the string runtime.
                Type::String => {
                    lower_string_method(cg, recv_val.into_pointer_value(), method, args)
                }
                // M8 P4: sensitive methods.
                // `.reveal()` → return the underlying string pointer (identity).
                // All other methods delegate to the string runtime (same ABI).
                Type::Sensitive { .. } => {
                    let ptr = recv_val.into_pointer_value();
                    if method == "reveal" {
                        // reveal() strips the sensitive wrapper — just return the pointer.
                        Ok(ptr.into())
                    } else {
                        // All string methods work on the underlying pointer.
                        lower_string_method(cg, ptr, method, args)
                    }
                }
                _ => {
                    // M4 P5: one-arg primitive intrinsics (wrapping/saturating arithmetic).
                    if args.len() == 1 && is_1arg_intrinsic(&recv_ty, method) {
                        let arg_val = lower_expr(cg, &args[0])?;
                        lower_method_call_1arg(cg, recv_val, &recv_ty, method, arg_val)
                    } else {
                        lower_method_call(cg, recv_val, &recv_ty, method)
                    }
                }
            }
        }

        Expr::FieldAccess {
            receiver, field, ..
        } => {
            // M4 P5: type-attached constants (int.max, number.epsilon, etc.)
            if let Expr::Ident(type_name_str, _) = receiver.as_ref() {
                if type_attached_const_type(type_name_str, field).is_some() {
                    return emit_type_const(cg, type_name_str, field);
                }
                // M6: options value access: `Status.active` → i8 tag constant.
                if let Some(entry) = cg.options_table.options.get(type_name_str.as_str()) {
                    if let Some(tag) = entry.variants.iter().position(|v| v == field) {
                        return Ok(cg.ctx.i8_type().const_int(tag as u64, false).into());
                    }
                }
            }
            lower_field_access(cg, receiver, field)
        }

        Expr::StructLit { fields, .. } => {
            // When typeck resolved this to a BuiltinMap, the user wrote `{ key: value }` syntax
            // with a map annotation — lower identifier names as string literal keys.
            if let Type::BuiltinMap { val, .. } = cg.expr_type(expr) {
                let map_ptr = cg
                    .builder
                    .build_call(cg.rt.ynz_map_new, &[], "map_new")
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_map_new returned void")?
                    .into_pointer_value();
                for f in fields {
                    let key_global = build_string_global(cg.ctx, cg.module, &f.name, "imap_key");
                    let key_ptr = key_global.as_pointer_value();
                    let val_val = lower_expr(cg, &f.value)?;
                    let val_bits = cg.to_i64_bits(val_val, &val)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set_str,
                            &[map_ptr.into(), key_ptr.into(), val_bits.into()],
                            "imap_set",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
                Ok(map_ptr.into())
            } else {
                lower_struct_lit(cg, expr, fields)
            }
        }

        Expr::PostfixOp { receiver, op, .. } => lower_postfix_op(cg, receiver, op),

        Expr::SelfValue { .. } => {
            let slot = *cg
                .locals
                .get("self")
                .ok_or("codegen: `self` used outside method scope")?;
            let ty = cg.expr_type(expr);
            load(cg, slot, &ty, "self")
        }

        Expr::NoneLit { .. } => {
            let slot = cg.build_maybe_none()?;
            Ok(slot.into())
        }

        Expr::ArrayLit { elements, .. } => {
            let arr_ty = cg.expr_type(expr);
            match &arr_ty {
                Type::BuiltinFixed { elem, size } => {
                    let n = size.unwrap_or(elements.len()) as u32;
                    let arr_t = cg.i64().array_type(n.max(1));
                    let slot = cg
                        .builder
                        .build_alloca(arr_t, "fixed_arr")
                        .map_err(|e| format!("{e}"))?;
                    for (i, elem_expr) in elements.iter().enumerate() {
                        let elem_val = lower_expr(cg, elem_expr)?;
                        let elem_ty2 = cg.expr_type(elem_expr);
                        let bits = cg.to_i64_bits(elem_val, &elem_ty2)?;
                        let idx = cg.i64().const_int(i as u64, false);
                        let gep = unsafe {
                            cg.builder
                                .build_gep(cg.i64(), slot, &[idx], "fixed_elem")
                                .map_err(|e| format!("fixed gep: {e}"))?
                        };
                        cg.builder
                            .build_store(gep, bits)
                            .map_err(|e| format!("{e}"))?;
                    }
                    let _ = elem;
                    Ok(slot.into())
                }
                _ => {
                    // BuiltinArray: heap-allocate via runtime.
                    let arr_ptr = cg
                        .builder
                        .build_call(cg.rt.ynz_array_new, &[], "arr_new")
                        .map_err(|e| format!("array_new: {e}"))?
                        .try_as_basic_value()
                        .basic()
                        .ok_or("ynz_array_new returned void")?
                        .into_pointer_value();
                    let in_sm = cg.sm_frame_ptr.is_some();
                    for (elem_idx, elem_expr) in elements.iter().enumerate() {
                        let elem_ty2 = cg.expr_type(elem_expr);
                        // NumberLit decimal128 elements in SM functions: emit a module-level
                        // constant global instead of a stack alloca so the pointer survives
                        // across suspension boundaries. Module globals have static lifetime —
                        // no ynz_alloc, no ynz_free, zero per-element heap overhead. This
                        // mirrors the non-SM array<number> path (also alloc=0/free=0) and
                        // the existing string-global pattern in NumberLit lowering.
                        //
                        // Shape struct-literal elements in SM functions: same global approach.
                        // Stack allocas created by lower_struct_lit are freed when the resume
                        // function returns after a suspension. The heap array retains the
                        // (now dangling) stack pointer as an i64 — reading it on the next
                        // resume is UB. Emitting a module-level global for all-literal-field
                        // struct elements gives the array a stable, statically-allocated source
                        // pointer that survives across any number of resume calls.
                        let elem_val = if in_sm
                            && matches!(elem_ty2, Type::Number { precision } if precision <= 34)
                        {
                            if let Expr::NumberLit(s, _) = elem_expr {
                                let bits: u128 = ynz_numerics::parse(s)
                                    .ok_or_else(|| format!("bad decimal literal `{s}`"))?;
                                let gname =
                                    format!(".arr.dec.{}.{}", elem_idx, &s[..s.len().min(8)]);
                                let g = build_decimal_global(cg.ctx, cg.module, bits, &gname);
                                g.as_pointer_value().into()
                            } else {
                                lower_expr(cg, elem_expr)?
                            }
                        } else if in_sm {
                            if let (
                                Type::Shape { name: ref sname },
                                Expr::StructLit { ref fields, .. },
                            ) = (&elem_ty2, elem_expr)
                            {
                                let sname = sname.clone();
                                let struct_ty_opt = cg.shape_types.get(&sname);
                                let shape_def_opt = cg.shape_table.get(&sname).cloned();
                                if let (Some(struct_ty), Some(shape_def)) =
                                    (struct_ty_opt, shape_def_opt)
                                {
                                    let gname = format!(".arr.shape.{}.{}", elem_idx, sname);
                                    if let Some(g) = try_build_shape_global(
                                        cg.ctx,
                                        cg.module,
                                        struct_ty,
                                        fields,
                                        &shape_def.fields,
                                        &gname,
                                    ) {
                                        g.as_pointer_value().into()
                                    } else {
                                        lower_expr(cg, elem_expr)?
                                    }
                                } else {
                                    lower_expr(cg, elem_expr)?
                                }
                            } else {
                                lower_expr(cg, elem_expr)?
                            }
                        } else {
                            lower_expr(cg, elem_expr)?
                        };
                        let bits = cg.to_i64_bits(elem_val, &elem_ty2)?;
                        cg.builder
                            .build_call(
                                cg.rt.ynz_array_push,
                                &[arr_ptr.into(), bits.into()],
                                "arr_push",
                            )
                            .map_err(|e| format!("array_push: {e}"))?;
                    }
                    Ok(arr_ptr.into())
                }
            }
        }

        Expr::IndexAccess {
            receiver, index, ..
        } => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;

            // Map index access is handled separately — key may be a string (pointer).
            if let Type::BuiltinMap { key, .. } = &recv_ty {
                let key_ty = key.as_ref().clone();
                let idx_val = lower_expr(cg, index)?;
                let pair_ty = cg
                    .ctx
                    .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                let out = cg
                    .builder
                    .build_alloca(pair_ty, "mi_out")
                    .map_err(|e| format!("{e}"))?;
                if key_is_string(&key_ty) {
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_get_str,
                            &[recv_val.into(), idx_val.into(), out.into()],
                            "mi_gs",
                        )
                        .map_err(|e| format!("{e}"))?;
                } else {
                    let kt = cg.expr_type(index);
                    let key_bits = cg.to_i64_bits(idx_val, &kt)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_get,
                            &[recv_val.into(), key_bits.into(), out.into()],
                            "mi_g",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
                let maybe_slot = cg
                    .builder
                    .build_alloca(cg.maybe_type(), "mi_m")
                    .map_err(|e| format!("{e}"))?;
                let h0 = cg
                    .builder
                    .build_struct_gep(pair_ty, out, 0, "h0")
                    .map_err(|e| format!("{e}"))?;
                let v0 = cg
                    .builder
                    .build_struct_gep(pair_ty, out, 1, "v0")
                    .map_err(|e| format!("{e}"))?;
                let h1 = cg
                    .builder
                    .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "h1")
                    .map_err(|e| format!("{e}"))?;
                let v1 = cg
                    .builder
                    .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "v1")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_store(
                        h1,
                        cg.builder
                            .build_load(cg.i64(), h0, "hv")
                            .map_err(|e| format!("{e}"))?,
                    )
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_store(
                        v1,
                        cg.builder
                            .build_load(cg.i64(), v0, "vv")
                            .map_err(|e| format!("{e}"))?,
                    )
                    .map_err(|e| format!("{e}"))?;
                return Ok(maybe_slot.into());
            }

            let idx_val = lower_expr(cg, index)?;
            let idx = idx_val.into_int_value();

            match &recv_ty {
                Type::BuiltinArray { .. } => {
                    let out_slot = cg
                        .builder
                        .build_alloca(cg.maybe_type(), "arr_get_out")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_array_get,
                            &[recv_val.into(), idx.into(), out_slot.into()],
                            "arr_get",
                        )
                        .map_err(|e| format!("array_get: {e}"))?;
                    Ok(out_slot.into())
                }
                Type::BuiltinFixed { size, .. } => {
                    let n = size.unwrap_or(0) as u64;
                    let maybe_slot = cg
                        .builder
                        .build_alloca(cg.maybe_type(), "fixed_get")
                        .map_err(|e| format!("{e}"))?;

                    let in_bounds = if n > 0 {
                        let idx_ext = cg
                            .builder
                            .build_int_z_extend_or_bit_cast(idx, cg.i64(), "ie")
                            .map_err(|e| format!("{e}"))?;
                        cg.builder
                            .build_int_compare(
                                IntPredicate::ULT,
                                idx_ext,
                                cg.i64().const_int(n, false),
                                "in_bounds",
                            )
                            .map_err(|e| format!("{e}"))?
                    } else {
                        cg.bool().const_int(0, false)
                    };

                    let ok_bb = cg.append_block("fixed_ok");
                    let oob_bb = cg.append_block("fixed_oob");
                    let merge_bb = cg.append_block("fixed_merge");
                    cg.builder
                        .build_conditional_branch(in_bounds, ok_bb, oob_bb)
                        .map_err(|e| format!("{e}"))?;

                    // ok: load element and write maybe_some
                    cg.builder.position_at_end(ok_bb);
                    let arr_ptr = recv_val.into_pointer_value();
                    let gep = unsafe {
                        cg.builder
                            .build_gep(cg.i64(), arr_ptr, &[idx], "fixed_elem_ok")
                            .map_err(|e| format!("fixed gep ok: {e}"))?
                    };
                    let elem_bits = cg
                        .builder
                        .build_load(cg.i64(), gep, "elem_bits")
                        .map_err(|e| format!("{e}"))?
                        .into_int_value();
                    let has_ok = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_ok")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(has_ok, cg.i64().const_int(1, false))
                        .map_err(|e| format!("{e}"))?;
                    let val_ok = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_ok")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(val_ok, elem_bits)
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_unconditional_branch(merge_bb)
                        .map_err(|e| format!("{e}"))?;

                    // oob: write maybe_none
                    cg.builder.position_at_end(oob_bb);
                    let has_oob = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_oob")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(has_oob, cg.i64().const_zero())
                        .map_err(|e| format!("{e}"))?;
                    let val_oob = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_oob")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(val_oob, cg.i64().const_zero())
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_unconditional_branch(merge_bb)
                        .map_err(|e| format!("{e}"))?;

                    cg.builder.position_at_end(merge_bb);
                    Ok(maybe_slot.into())
                }
                _ => Err(format!("IndexAccess on unsupported type: {:?}", recv_ty)),
            }
        }

        Expr::MapLit { entries, .. } => {
            let map_ty = cg.expr_type(expr);
            let key_ty = match &map_ty {
                Type::BuiltinMap { key, .. } => key.as_ref().clone(),
                _ => return Err("MapLit with non-BuiltinMap type".to_string()),
            };

            let map_ptr = cg
                .builder
                .build_call(cg.rt.ynz_map_new, &[], "map_new")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_map_new returned void")?
                .into_pointer_value();

            for (key_expr, val_expr) in entries {
                let key_val = lower_expr(cg, key_expr)?;
                let val_val = lower_expr(cg, val_expr)?;
                let vt = cg.expr_type(val_expr);
                let val_bits = cg.to_i64_bits(val_val, &vt)?;
                if key_is_string(&key_ty) {
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set_str,
                            &[map_ptr.into(), key_val.into(), val_bits.into()],
                            "map_set_str",
                        )
                        .map_err(|e| format!("{e}"))?;
                } else {
                    let kt = cg.expr_type(key_expr);
                    let key_bits = cg.to_i64_bits(key_val, &kt)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set,
                            &[map_ptr.into(), key_bits.into(), val_bits.into()],
                            "map_set",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
            }
            Ok(map_ptr.into())
        }

        Expr::Error(_) => Err("codegen: error node".to_string()),

        // M6: `x is Foo` — P4 implements; typeck would have rejected programs with unimplemented forms.
        Expr::Is { .. } => Err("codegen: Expr::Is not yet lowered (P4 work)".to_string()),

        // M7 P4b: interpolated string codegen.
        //
        // Pure-literal backtick strings (no `${}` parts): emit as a null-terminated
        // global byte array (same as the prior M7 P1 path).
        //
        // Strings with `${}` expressions: use the string builder API. Each part is
        // appended in sequence; the builder is finalized to a single heap string.
        // One allocation per interpolated string expression, regardless of part count.
        Expr::InterpolatedString(parts, _) => {
            let is_pure_lit = parts
                .iter()
                .all(|p| matches!(p, ynz_ast::nodes::StringPart::Lit(_, _)));
            if is_pure_lit {
                let mut bytes: Vec<u8> = parts
                    .iter()
                    .flat_map(|p| match p {
                        ynz_ast::nodes::StringPart::Lit(b, _) => b.clone(),
                        ynz_ast::nodes::StringPart::Expr(_, _) => unreachable!(),
                    })
                    .collect();
                push_c_string_terminator(&mut bytes);
                let i8t = cg.i8();
                let arr_ty = i8t.array_type(bytes.len() as u32);
                let arr = i8t.const_array(
                    &bytes
                        .iter()
                        .map(|&b| i8t.const_int(b as u64, false))
                        .collect::<Vec<_>>(),
                );
                let g = cg
                    .module
                    .add_global(arr_ty, Some(AddressSpace::default()), "bts");
                g.set_initializer(&arr);
                g.set_constant(true);
                g.set_linkage(inkwell::module::Linkage::Private);
                g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
                Ok(g.as_pointer_value().into())
            } else {
                // Interpolated string: build via ynz_string_builder_*.
                let builder = cg
                    .builder
                    .build_call(cg.rt.ynz_string_builder_new, &[], "istr_bld")
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_builder_new void")?
                    .into_pointer_value();

                for part in parts.iter() {
                    match part {
                        ynz_ast::nodes::StringPart::Lit(bytes, _) => {
                            // Emit the literal bytes as a global and append.
                            let mut b = bytes.clone();
                            push_c_string_terminator(&mut b);
                            let i8t = cg.i8();
                            let arr_ty = i8t.array_type(b.len() as u32);
                            let arr = i8t.const_array(
                                &b.iter()
                                    .map(|&x| i8t.const_int(x as u64, false))
                                    .collect::<Vec<_>>(),
                            );
                            let g = cg.module.add_global(
                                arr_ty,
                                Some(AddressSpace::default()),
                                "istr_lit",
                            );
                            g.set_initializer(&arr);
                            g.set_constant(true);
                            g.set_linkage(inkwell::module::Linkage::Private);
                            g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
                            cg.builder
                                .build_call(
                                    cg.rt.ynz_string_builder_append,
                                    &[builder.into(), g.as_pointer_value().into()],
                                    "",
                                )
                                .map_err(|e| format!("{e}"))?;
                        }
                        ynz_ast::nodes::StringPart::Expr(sub_expr, _) => {
                            // Evaluate the expression and convert to string via toString().
                            let val = lower_expr(cg, sub_expr)?;
                            let sub_ty = cg.expr_type(sub_expr);
                            // Produce a null-terminated string from the expression value.
                            let s_ptr = expr_to_cstring(cg, val, &sub_ty)?;
                            cg.builder
                                .build_call(
                                    cg.rt.ynz_string_builder_append,
                                    &[builder.into(), s_ptr.into()],
                                    "",
                                )
                                .map_err(|e| format!("{e}"))?;
                        }
                    }
                }

                let result = cg
                    .builder
                    .build_call(
                        cg.rt.ynz_string_builder_finalize,
                        &[builder.into()],
                        "istr_fin",
                    )
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_builder_finalize void")?;
                Ok(result)
            }
        }
        // v0.3-M2: Expr::Wait lowering.
        //
        // Inside a state-machine resume function (cg.is_state_machine=true), `wait` is
        // handled by lower_sm_body / emit_wait_point — not by lower_expr. The is_state_machine
        // branch here is a safety fallback for waits encountered during generic expression
        // evaluation (e.g., waits in argument positions). State-machine callers reach a
        // suspending callee via inline poll-and-yield (lower_sm_stmt_with_wait) into the
        // callee's embedded sub-frame — never via a runtime driver. The non-SM path below
        // drives a suspending callee through its wrapper (which runs the program-entry driver
        // to completion); the callee function itself is generated as a state machine by
        // lower_function_with_waits.
        //
        // WHY the non-SM arm evaluates the inner expression: if a non-SM function contains
        // wait (e.g., a wait-free function calling a wait-containing function) the call
        // falls through to the normal call lowering path which then emits ynz_rt_run_entrypoint
        // at the call site (Step 6 dispatch). The `wait` keyword itself has no IR equivalent
        // outside of a state-machine context — it is pure syntax that drives path selection.
        Expr::Wait(inner, _) => {
            // Safety fallback: if we somehow reach here inside a state machine, evaluate normally.
            // The SM body lowering (lower_sm_body) handles waits before they reach lower_expr.
            lower_expr(cg, inner)
        }
        Expr::Background(inner, _) => lower_expr_background(cg, inner),
    }
}

/// What the background task closure must do with a heap-copied arg after the call returns.
///
/// Each `background` arg that was heap-copied (to survive the spawner's frame return) needs
/// exactly one matching free call inside the closure. Primitives and strings need no free:
/// primitives are i64 by-value, strings are immutable heap pointers that outlive the frame.
///
/// This enum is produced by `prepare_bg_arg_for_ctx` at the spawn site and consumed by the
/// closure body emitter to generate the matching `ynz_free` / `ynz_array_drop` calls.
#[derive(Clone, Debug)]
enum BgArgFreeKind {
    /// No heap allocation to free — primitives (int/float/bool) and strings.
    None,
    /// Shape heap copy: call `ynz_free(ptr, byte_size)` after the fn call.
    HeapShape { byte_size: u64 },
    /// Primitive array clone: call `ynz_array_drop(ptr)` after the fn call.
    HeapArrayPrimitive,
}

/// Prepare one `background` argument for storage in the task ctx.
///
/// Heap types whose pointer would alias the spawner's stack frame are upgraded to
/// independent heap allocations here — their pointed-to data is `ynz_alloc`'d and
/// the returned value is the heap pointer (safe to pass to the task via the ctx).
/// The returned `BgArgFreeKind` tells the closure body what to free after the call.
///
/// Two kinds of bg args reach this function:
/// - Plain-ident args where typeck chose Copy (inferred from use-after-spawn).
/// - Explicit `.copy()` args whose inner `lower_expr` produced a Shape alloca pointer.
///   Those also point into spawner stack memory and need the same heap-upgrade.
///
/// In both cases the heap allocation outlives the spawner's frame (ynz_rt_spawn_blocking
/// copies the ctx bytes — the i64 pointer value — before returning; the pointed-to
/// heap data is what must survive).
///
/// Per-type decisions:
/// - `Shape`: `ynz_alloc(struct_bytes)` + memcpy. BgArgFreeKind::HeapShape.
/// - `String`: immutable heap bytes, already outlive the spawner frame. BgArgFreeKind::None.
/// - `array<Int|Float|Bool>`: `ynz_array_clone_primitive`. BgArgFreeKind::HeapArrayPrimitive.
/// - Primitives (Int/Bool/Float): by-value i64, no pointer. BgArgFreeKind::None.
/// - Other heap types (array<heap_elem>, map, maybe, union): not yet supported here;
///   these fall through unchanged (same pointer-alias behavior as today — the caller
///   is responsible for not mutating these after the spawn, which the typeck enforces
///   by consuming give bindings and producing a copy warning for inferred-copy cases).
fn prepare_bg_arg_for_ctx<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    arg: &ynz_ast::nodes::Expr,
    val: inkwell::values::BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<(inkwell::values::BasicValueEnum<'ctx>, BgArgFreeKind), String> {
    // Determine whether this arg is a heap type that needs the spawn-lifetime fix.
    //
    // Both Give and Copy paths need heap-upgrade for Shape/array<primitive> args:
    // - Copy: the caller keeps the original; the task needs an independent heap copy.
    // - Give: the caller no longer uses the value, but the Stack alloca holding the
    //   struct data is still on the spawner's frame — it is freed when the spawner
    //   returns. If the spawner returns before the task reads, the task has a dangling
    //   pointer into freed stack memory (UAF). Heap-upgrading the Give path produces
    //   a heap copy the task owns; the spawner's alloca is freed harmlessly by the
    //   normal stack unwind.
    //
    // For both cases, the closure body must call ynz_free after the original fn
    // returns, matching the ynz_alloc emitted here.
    //
    // Primitives (Int/Bool/Float) are i64 by-value and need no heap-upgrade.
    // Strings are immutable heap bytes; the pointer itself survives any frame.
    let is_heap_arg = match arg {
        ynz_ast::nodes::Expr::Ident(_, s) => {
            // Plain ident: any inferred Give or Copy ownership gets the heap fix.
            let key = (s.start, s.end);
            cg.typed
                .background_arg_inferred_ownership
                .contains_key(&key)
        }
        // Explicit .copy() postfix — always heap-upgrade for heap types.
        ynz_ast::nodes::Expr::PostfixOp {
            op: ynz_ast::nodes::PostfixOpKind::Copy,
            ..
        } => true,
        _ => false,
    };

    if !is_heap_arg {
        return Ok((val, BgArgFreeKind::None));
    }

    let resolved = cg.resolve_type(ty);
    match &resolved {
        Type::Shape { name } => {
            // Shape: the val is a pointer to struct data on the spawner's stack (whether the
            // copy came from an alloca+memcpy in inferred-copy or explicit .copy() codegen,
            // or from the original shape allocation in a give path). Heap-allocate the struct
            // bytes so the task's pointer survives the spawner's frame return.
            let name = name.clone();
            let struct_ty = cg
                .shape_types
                .get(&name)
                .ok_or_else(|| format!("bg heap copy: LLVM type for `{}` not found", name))?;
            // Byte size of the struct according to LLVM's target data layout.
            let byte_size_val = struct_ty
                .size_of()
                .ok_or_else(|| format!("bg heap copy: size_of unavailable for `{}`", name))?;
            let byte_size_i64 = cg
                .builder
                .build_int_z_extend(byte_size_val, cg.i64(), "shape_size_i64")
                .map_err(|e| format!("bg heap copy: size zext: {e}"))?;
            let heap_ptr = cg
                .builder
                .build_call(cg.rt.ynz_alloc, &[byte_size_i64.into()], "bg_shape_heap")
                .map_err(|e| format!("bg heap copy: ynz_alloc call: {e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or_else(|| "bg heap copy: ynz_alloc returned void".to_string())?
                .into_pointer_value();
            let struct_val = cg
                .builder
                .build_load(struct_ty, val.into_pointer_value(), "bg_shape_src")
                .map_err(|e| format!("bg heap copy: load src: {e}"))?;
            cg.builder
                .build_store(heap_ptr, struct_val)
                .map_err(|e| format!("bg heap copy: store to heap: {e}"))?;
            // Byte size for the BgArgFreeKind free call. LLVM `size_of()` is a constant
            // EXPRESSION (`ptrtoint getelementptr`), NOT a literal ConstantInt, so
            // `get_zero_extended_constant()` returns None here — the fallback is taken in
            // practice, not as an error path.
            //
            // @design-decision Fall back to 0 when the constant can't be extracted.
            // @rationale `ynz_free` ignores its size argument today (it wraps libc `free`,
            //   which tracks allocation size internally), so 0 is observably correct now.
            //   The authoritative size lives in `shape_abi_sizes` (TargetData::get_abi_size)
            //   but is not threaded into this helper; wiring it for a currently-ignored value
            //   would be gold-plating (YAGNI).
            // @follow-up When kernel-mode sized-dealloc lands (a custom allocator whose free
            //   DOES use the size), thread `shape_abi_sizes` into `prepare_bg_arg_for_ctx` and
            //   look the size up by shape name instead of this fallback.
            // @triggers `--kernel` sized-dealloc support (design/no-runtime-mode.md).
            let byte_size = byte_size_val.get_zero_extended_constant().unwrap_or(0);
            Ok((heap_ptr.into(), BgArgFreeKind::HeapShape { byte_size }))
        }
        Type::BuiltinArray { elem } => {
            // Clone a primitive-element array so the task gets an independent copy.
            // For heap-element arrays (shapes, strings, etc.) we cannot recursively
            // deep-copy without knowing element copy semantics — that is the m3c array-by-value
            // ABI work. Those fall through unchanged (same behavior as today's explicit
            // `.copy()` path).
            let is_primitive_elem = matches!(elem.as_ref(), Type::Int | Type::Bool | Type::Float);
            if is_primitive_elem {
                let clone_ptr = cg
                    .builder
                    .build_call(
                        cg.rt.ynz_array_clone_primitive,
                        &[val.into_pointer_value().into()],
                        "bg_arr_clone",
                    )
                    .map_err(|e| format!("bg arr clone: {e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or_else(|| "bg arr clone: returned void".to_string())?;
                Ok((clone_ptr, BgArgFreeKind::HeapArrayPrimitive))
            } else {
                // array<heap_elem>: recursive deep-copy is m3c ABI work.
                // Pass as-is — same pointer-alias behavior as today's explicit `.copy()` on
                // these types. The binding is already consumed (give path) or copied-shallow
                // (explicit .copy() path); the task should not mutate elements.
                Ok((val, BgArgFreeKind::None))
            }
        }
        Type::String => {
            // String bytes are heap-allocated and immutable — the pointer itself survives the
            // spawner's frame independently of the stack. No heap copy needed.
            Ok((val, BgArgFreeKind::None))
        }
        _ => {
            // Primitives (Int/Bool/Float) are i64 by-value — no pointer involved.
            // All other heap types (map, maybe, union) alias today on explicit .copy() too;
            // that is the m3c scope, not changed here.
            Ok((val, BgArgFreeKind::None))
        }
    }
}

/// Emit the free calls for heap-copied `background` args inside the closure body.
///
/// After the original function call, each arg that was heap-allocated for the task must be
/// freed exactly once. Called from inside the closure (`ynz_bg_<name>_<uid>`) after
/// the original fn call and before the closure returns.
///
/// The `ctx_arg` pointer and `arg_types` give the slot layout; `free_kinds` is parallel to
/// `arg_types` and was recorded at the spawn site.
fn emit_bg_arg_frees<'ctx>(
    cg_builder: &inkwell::builder::Builder<'ctx>,
    rt: &RuntimeDecls<'ctx>,
    i64_ty: inkwell::types::IntType<'ctx>,
    ptr_ty: inkwell::types::PointerType<'ctx>,
    ctx_arg: inkwell::values::PointerValue<'ctx>,
    free_kinds: &[BgArgFreeKind],
) -> Result<(), String> {
    for (i, kind) in free_kinds.iter().enumerate() {
        match kind {
            BgArgFreeKind::None => {}
            BgArgFreeKind::HeapShape { byte_size } => {
                let slot = unsafe {
                    cg_builder
                        .build_gep(
                            i64_ty,
                            ctx_arg,
                            &[i64_ty.const_int(i as u64, false)],
                            "free_slot",
                        )
                        .map_err(|e| format!("bg free gep: {e}"))?
                };
                let bits = cg_builder
                    .build_load(i64_ty, slot, "free_bits")
                    .map_err(|e| format!("bg free load: {e}"))?
                    .into_int_value();
                let heap_ptr = cg_builder
                    .build_int_to_ptr(bits, ptr_ty, "free_ptr")
                    .map_err(|e| format!("bg free inttoptr: {e}"))?;
                let size_val = i64_ty.const_int(*byte_size, false);
                cg_builder
                    .build_call(
                        rt.ynz_free,
                        &[heap_ptr.into(), size_val.into()],
                        "bg_shape_free",
                    )
                    .map_err(|e| format!("bg shape free call: {e}"))?;
            }
            BgArgFreeKind::HeapArrayPrimitive => {
                let slot = unsafe {
                    cg_builder
                        .build_gep(
                            i64_ty,
                            ctx_arg,
                            &[i64_ty.const_int(i as u64, false)],
                            "free_arr_slot",
                        )
                        .map_err(|e| format!("bg arr free gep: {e}"))?
                };
                let bits = cg_builder
                    .build_load(i64_ty, slot, "free_arr_bits")
                    .map_err(|e| format!("bg arr free load: {e}"))?
                    .into_int_value();
                let heap_ptr = cg_builder
                    .build_int_to_ptr(bits, ptr_ty, "free_arr_ptr")
                    .map_err(|e| format!("bg arr free inttoptr: {e}"))?;
                cg_builder
                    .build_call(rt.ynz_array_drop, &[heap_ptr.into()], "bg_arr_drop")
                    .map_err(|e| format!("bg arr drop call: {e}"))?;
            }
        }
    }
    Ok(())
}

/// Lower `background fn(args)` to `ynz_rt_spawn_blocking`.
///
/// # Approach
///
/// 1. Evaluate all arguments on the calling thread; convert each to an i64 bit pattern.
/// 2. Alloca a context struct on the caller's stack (`n_args * 8` bytes); store each i64.
///    Stack alloca is safe because `ynz_rt_spawn_blocking` copies the bytes synchronously
///    before returning — the callee owns its heap copy; the caller's alloca is freed
///    automatically at end-of-function without any explicit `ynz_free`.
/// 3. Create a new LLVM function `ynz_bg_<name>_<uid>` with signature `void (ptr)`.
///    Its body: for each ctx slot, load i64 → reconstruct original type → call original fn.
///    The `uid` is per-Cg (incremented on `cg.bg_uid`) — deterministic across compilations.
/// 4. Emit `ynz_rt_spawn_blocking(closure, ctx, size)` and return i32(0).
///
/// # Ownership
///
/// The runtime (`ynz_rt_spawn_blocking`) copies the ctx bytes into a `Box<[u8]>` owned by
/// `CtxDropGuard`. The `CtxDropGuard` frees the COPY on both normal return and panic path.
/// The caller-side alloca is reclaimed at function exit — no explicit free needed.
///
/// Call-site preempt insertion deferred to M2 per P1 GATE measurement.
fn lower_expr_background<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    inner: &ynz_ast::nodes::Expr,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    use inkwell::values::BasicMetadataValueEnum;

    // background must wrap a Call — typeck already enforces this.
    let call = match inner {
        ynz_ast::nodes::Expr::Call(c) => c,
        _ => {
            // Typeck already emitted an error. Fall back to sequential execution.
            let _ = lower_expr(cg, inner)?;
            return Ok(cg.i32().const_int(0, false).into());
        }
    };

    let callee_name = match &call.callee {
        ynz_ast::nodes::Expr::Ident(name, _) => name.clone(),
        _ => {
            // Complex callee (method call desugared to non-ident) — run synchronously.
            let _ = lower_expr(cg, inner)?;
            return Ok(cg.i32().const_int(0, false).into());
        }
    };

    // v0.3-M2: if the callee is a state-machine function, route to ynz_rt_spawn (I/O pool)
    // instead of ynz_rt_spawn_blocking (CPU blocking pool).
    //
    // WHY: state-machine functions yield during I/O via poll-and-suspend; routing them to
    // the blocking pool would hold a dedicated OS thread captive during the wait, defeating
    // the entire point of the state machine. The I/O pool shares threads cooperatively.
    // v0.3-M2 P7: use suspend_set (transitive) instead of wait_cache (local) for routing.
    // Any suspending fn (transitively reaches `sleep`) routes to the I/O pool via ynz_rt_spawn.
    if cg.suspend_set.contains(&callee_name) {
        return lower_expr_background_state_machine(cg, call, &callee_name);
    }

    // Step 1: evaluate arguments on the calling thread.
    // Heap types whose pointer would alias the spawner's stack frame are heap-upgraded via
    // `prepare_bg_arg_for_ctx`: the pointed-to data is ynz_alloc'd so the task's pointer
    // survives the spawner's frame return. The returned BgArgFreeKind records what the closure
    // body must free after calling the original fn.
    let mut arg_vals_i64: Vec<inkwell::values::IntValue<'ctx>> = Vec::new();
    let mut arg_types: Vec<Type> = Vec::new();
    let mut free_kinds: Vec<BgArgFreeKind> = Vec::new();
    for arg in &call.args {
        let val = lower_expr(cg, arg)?;
        let ty = cg.expr_type(arg);
        let (val, kind) = prepare_bg_arg_for_ctx(cg, arg, val, &ty)?;
        let bits = cg
            .to_i64_bits(val, &ty)
            .map_err(|e| format!("background arg to_i64_bits: {e}"))?;
        arg_vals_i64.push(bits);
        arg_types.push(ty);
        free_kinds.push(kind);
    }

    let n_args = arg_vals_i64.len();
    let ctx_size: u64 = (n_args as u64) * 8; // each arg is i64 = 8 bytes

    // Step 2: alloca context on the caller's stack and store args.
    // Stack alloca is safe because ynz_rt_spawn_blocking copies the bytes synchronously
    // before returning; the runtime's heap copy is owned by CtxDropGuard.
    // No ynz_free needed — the alloca is reclaimed automatically at function exit.
    let ctx_ptr = if ctx_size > 0 {
        let ctx_ty = cg.i64().array_type(n_args as u32);
        let alloca = cg
            .builder
            .build_alloca(ctx_ty, "bg_ctx")
            .map_err(|e| format!("bg ctx alloca: {e}"))?;
        for (i, bits) in arg_vals_i64.iter().enumerate() {
            // SAFETY: GEP into the alloca; all offsets are within [0, n_args * 8).
            let slot = unsafe {
                cg.builder
                    .build_gep(
                        cg.i64(),
                        alloca,
                        &[cg.i64().const_int(i as u64, false)],
                        "bg_slot",
                    )
                    .map_err(|e| format!("bg ctx gep: {e}"))?
            };
            cg.builder
                .build_store(slot, *bits)
                .map_err(|e| format!("bg store: {e}"))?;
        }
        alloca
    } else {
        cg.ctx
            .ptr_type(inkwell::AddressSpace::default())
            .const_null()
    };

    // Step 3: create the closure LLVM function.
    // Per-Cg counter: deterministic across multiple compilations in the same process.
    let uid = cg.bg_uid;
    cg.bg_uid += 1;
    let closure_name = format!("ynz_bg_{}_{}", callee_name, uid);
    let closure_ty = cg.ctx.void_type().fn_type(
        &[cg.ctx.ptr_type(inkwell::AddressSpace::default()).into()],
        false,
    );
    let closure_fn = cg.module.add_function(&closure_name, closure_ty, None);

    // Save current insert block so we can restore it after building the closure body.
    let return_bb = cg
        .builder
        .get_insert_block()
        .ok_or_else(|| "no insert block at background site".to_string())?;

    let closure_entry = cg.ctx.append_basic_block(closure_fn, "entry");
    cg.builder.position_at_end(closure_entry);

    // Unpack args from ctx and reconstruct original types.
    let ctx_arg = closure_fn
        .get_nth_param(0)
        .ok_or_else(|| "closure has no param".to_string())?
        .into_pointer_value();

    let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
    for (i, ty) in arg_types.iter().enumerate() {
        let slot = unsafe {
            cg.builder
                .build_gep(
                    cg.i64(),
                    ctx_arg,
                    &[cg.i64().const_int(i as u64, false)],
                    "cl_slot",
                )
                .map_err(|e| format!("closure gep: {e}"))?
        };
        let bits = cg
            .builder
            .build_load(cg.i64(), slot, "cl_arg")
            .map_err(|e| format!("closure load: {e}"))?
            .into_int_value();
        let val = cg
            .i64_bits_to(bits, ty)
            .map_err(|e| format!("closure i64_bits_to: {e}"))?;
        call_args.push(val.into());
    }

    // Call the original function from within the closure.
    // Use the same mangled-name resolution as the regular Call path (for generics).
    // Import-alias check comes FIRST: when a local function has the same name as an
    // import alias (e.g., local `function doWork()` and `import { compute as doWork }`),
    // the caller wrote `background doWork()` intending the imported callee — the import
    // alias must win. Checking module.get_function first would silently dispatch to the
    // local definition instead, emitting a call to the wrong callee.
    let effective_name = if let Some(orig) = cg
        .imported_fns
        .get(callee_name.as_str())
        .and_then(|sig| sig.original_name.as_deref())
    {
        orig.to_string()
    } else if cg.module.get_function(&callee_name).is_some() {
        callee_name.clone()
    } else {
        find_mono_name_by_args(cg.mono_table, &callee_name, &arg_types)
            .unwrap_or_else(|| callee_name.clone())
    };
    let target_fn = cg
        .module
        .get_function(&effective_name)
        .ok_or_else(|| format!("background callee `{effective_name}` not in module"))?;
    cg.builder
        .build_call(target_fn, &call_args, "bg_call")
        .map_err(|e| format!("closure call: {e}"))?;

    // Free any heap-copied args now that the original fn has returned.
    // Each BgArgFreeKind::HeapShape/HeapArrayPrimitive slot holds a heap pointer that was
    // ynz_alloc'd at spawn time and must be freed exactly once here.
    let ptr_ty = cg.ctx.ptr_type(inkwell::AddressSpace::default());
    emit_bg_arg_frees(&cg.builder, cg.rt, cg.i64(), ptr_ty, ctx_arg, &free_kinds)
        .map_err(|e| format!("bg arg free: {e}"))?;

    cg.builder
        .build_return(None)
        .map_err(|e| format!("closure ret: {e}"))?;

    // Step 4: restore builder and emit spawn call.
    cg.builder.position_at_end(return_bb);

    // Cast closure function pointer to ptr (the C-ABI fn pointer type).
    let closure_ptr = closure_fn.as_global_value().as_pointer_value();
    let ctx_i64 = cg
        .builder
        .build_ptr_to_int(ctx_ptr, cg.i64(), "ctx_i64")
        .map_err(|e| format!("ctx ptrtoint: {e}"))?;
    let ctx_as_ptr = cg
        .builder
        .build_int_to_ptr(
            ctx_i64,
            cg.ctx.ptr_type(inkwell::AddressSpace::default()),
            "ctx_ptr",
        )
        .map_err(|e| format!("ctx inttoptr: {e}"))?;

    cg.builder
        .build_call(
            cg.rt.ynz_rt_spawn_blocking,
            &[
                closure_ptr.into(),
                ctx_as_ptr.into(),
                cg.i64().const_int(ctx_size, false).into(),
            ],
            "bg_spawn",
        )
        .map_err(|e| format!("spawn_blocking: {e}"))?;

    Ok(cg.i32().const_int(0, false).into())
}

/// Lower `background sm_fn(args)` for a state-machine callee (routes to `ynz_rt_spawn`).
///
/// # Flow
///
/// 1. Evaluate all arguments and convert to i64 bit patterns.
/// 2. Heap-allocate the state-machine frame via `ynz_alloc`.
///    The frame is heap-allocated (not stack) because it outlives this call — the spawned
///    future owns it until completion or cancellation.
/// 3. Write resume_point=0 and arg slots to the frame.
/// 4. Call `ynz_rt_spawn(resume_fn_ptr, frame_ptr, frame_size)` — fire-and-forget.
///    The spawned future owns the frame; no ynz_free here. The frame is freed by the
///    codegen-emitted resume function when it reaches its terminal state (returns Ready).
///    Note: in M2, frame deallocation on spawn path is handled by resume_fn's terminal
///    transition; the RAII drop guard in SpawnStateFnFuture covers cancellation.
///
/// # Failure modes
///
/// - `ynz_rt_init` not called: `ynz_rt_spawn` logs a warning and discards the task.
/// - Resume function panics: Tokio task wrapper catches it; error is logged and discarded.
fn lower_expr_background_state_machine<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    call: &ynz_ast::nodes::CallExpr,
    callee_name: &str,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    // Step 1: evaluate arguments and convert to i64 bits.
    // Heap-upgrade copied args so task pointers survive the spawner's frame return.
    // The resulting heap allocations (Shape via ynz_alloc / array<primitive> via
    // ynz_array_clone_primitive) are stored in the frame's local slots as i64 bit-patterns.
    // SpawnStateFnFuture::drop frees them (via arg_drop_ptr/arg_drop_count) after the
    // callee has read them, keeping alloc/free balanced on every task exit path.
    let mut arg_vals_i64: Vec<inkwell::values::IntValue<'ctx>> = Vec::new();
    let mut free_kinds: Vec<BgArgFreeKind> = Vec::new();
    for arg in &call.args {
        let val = lower_expr(cg, arg)?;
        let ty = cg.expr_type(arg);
        let (val, kind) = prepare_bg_arg_for_ctx(cg, arg, val, &ty)?;
        let bits = cg
            .to_i64_bits(val, &ty)
            .map_err(|e| format!("sm bg arg bits: {e}"))?;
        arg_vals_i64.push(bits);
        free_kinds.push(kind);
    }
    let n_locals = arg_vals_i64.len();

    // Step 2: heap-allocate the COMPOSED frame (callee's total composed size covers the
    // entire spawned task tree — one alloc per background spawn per the design doc model).
    let total_frame_size = cg
        .frame_layouts
        .get(callee_name)
        .map(|l| l.total_size)
        .unwrap_or_else(|| {
            state_machine::FRAME_HEADER_SIZE + state_machine::own_locals_size(n_locals)
        });
    let frame_ptr = state_machine::alloc_frame(cg.ctx, &cg.builder, cg.rt, total_frame_size)?;

    // Step 3: write parameter values to frame local slots (at offset 32+).
    for (idx, bits) in arg_vals_i64.iter().enumerate() {
        state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, idx, *bits)?;
    }

    // Step 4: find the resume function.
    // When the callee was imported under an alias (`import { getValue as fetchVal }`),
    // the LLVM resume fn uses the original exported symbol name (`ynz_sm_getValue_resume`),
    // not the alias. The module forward-declared the resume fn under the original name
    // (see Pass 0.6 / the imported-fn declaration loop). Using the alias here would
    // produce a lookup failure at runtime: "ynz_sm_fetchVal_resume not found".
    let callee_exported = cg
        .imported_fns
        .get(callee_name)
        .and_then(|sig| sig.original_name.as_deref())
        .unwrap_or(callee_name);
    let resume_name = state_machine::resume_fn_name(callee_exported);
    let resume_fn = cg
        .module
        .get_function(&resume_name)
        .ok_or_else(|| format!("sm bg: resume fn `{resume_name}` not found"))?;

    let resume_ptr = resume_fn.as_global_value().as_pointer_value();
    let frame_size_val = cg.ctx.i64_type().const_int(total_frame_size, false);

    // Pass the recursion_slot_offset so SpawnStateFnFuture::Drop can walk and free
    // any heap-boxed recursive child frames on cancellation. -1 = no recursion slot.
    let rec_slot_offset = cg
        .frame_layouts
        .get(callee_name)
        .and_then(|l| l.recursion_slot)
        .map(|off| off as i64)
        .unwrap_or(-1_i64);
    let rec_slot_offset_val = cg.ctx.i64_type().const_int(
        // i64::from_le_bytes(rec_slot_offset.to_le_bytes()) — cast negative as unsigned for LLVM
        rec_slot_offset as u64,
        true, // sign-extended constant
    );

    // Step 5: build the arg-drop descriptor array for SpawnStateFnFuture::drop.
    //
    // Each BgArgDropEntry has three i64 fields (24 bytes total):
    //   byte_offset: u64 — byte offset in the frame to the i64 slot holding the heap pointer
    //   kind: u64        — 0=HeapShape (ynz_free), 1=HeapArrayPrimitive (ynz_array_drop)
    //   size: u64        — byte count for ynz_free (HeapShape); 0 for HeapArrayPrimitive
    //
    // Build the descriptor list only for args that were actually heap-copied; skip None.
    // If no args need freeing, pass null pointer + count=0 (no allocation needed).
    let heap_args: Vec<(usize, u64, u64)> = free_kinds
        .iter()
        .enumerate()
        .filter_map(|(slot_idx, kind)| match kind {
            BgArgFreeKind::HeapShape { byte_size } => {
                let byte_offset = state_machine::FRAME_OFFSET_LOCALS_START + (slot_idx as u64) * 8;
                Some((slot_idx, byte_offset, *byte_size))
            }
            BgArgFreeKind::HeapArrayPrimitive => {
                let byte_offset = state_machine::FRAME_OFFSET_LOCALS_START + (slot_idx as u64) * 8;
                // Triple is (slot_idx, byte_offset, size). size=0 for arrays —
                // `ynz_array_drop` knows its own buffer size, so the descriptor's size
                // field is unused for this kind. The kind (1=array) is re-derived from
                // `free_kinds[slot_idx]` when the descriptor is written below; slot_idx is
                // preserved precisely so that re-derivation indexes the original, unfiltered slot.
                Some((slot_idx, byte_offset, 0_u64))
            }
            BgArgFreeKind::None => None,
        })
        .collect();

    let (arg_drop_ptr_val, arg_drop_count_val) = if heap_args.is_empty() {
        // No heap arg-copies — pass null/0. SpawnStateFnFuture::drop skips the loop.
        let null_ptr = cg
            .ctx
            .ptr_type(inkwell::AddressSpace::default())
            .const_null();
        (null_ptr.into(), cg.ctx.i64_type().const_int(0, false))
    } else {
        // Allocate the descriptor array: heap_args.len() entries × 24 bytes each.
        let desc_entry_size: u64 = 24; // 3 × u64
        let desc_total = desc_entry_size * heap_args.len() as u64;
        let desc_total_val = cg.ctx.i64_type().const_int(desc_total, false);
        let desc_ptr = cg
            .builder
            .build_call(cg.rt.ynz_alloc, &[desc_total_val.into()], "arg_drop_alloc")
            .map_err(|e| format!("arg drop alloc: {e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| "arg drop alloc: returned void".to_string())?
            .into_pointer_value();

        // Fill each descriptor: { byte_offset: u64, kind: u64, size: u64 }.
        let i64_ty = cg.ctx.i64_type();
        let i8_ty = cg.ctx.i8_type();
        for (entry_idx, (slot_idx, byte_offset, size)) in heap_args.iter().enumerate() {
            let entry_byte_base = entry_idx as u64 * desc_entry_size;

            // field 0: byte_offset
            let off0 = unsafe {
                cg.builder
                    .build_gep(
                        i8_ty,
                        desc_ptr,
                        &[i64_ty.const_int(entry_byte_base, false)],
                        "desc_off0",
                    )
                    .map_err(|e| format!("desc gep 0: {e}"))?
            };
            cg.builder
                .build_store(off0, i64_ty.const_int(*byte_offset, false))
                .map_err(|e| format!("desc store 0: {e}"))?;

            // field 1: kind (0=HeapShape, 1=HeapArrayPrimitive)
            let kind_val = match &free_kinds[*slot_idx] {
                BgArgFreeKind::HeapShape { .. } => 0_u64,
                BgArgFreeKind::HeapArrayPrimitive => 1_u64,
                BgArgFreeKind::None => unreachable!("filtered above"),
            };
            let off1 = unsafe {
                cg.builder
                    .build_gep(
                        i8_ty,
                        desc_ptr,
                        &[i64_ty.const_int(entry_byte_base + 8, false)],
                        "desc_off1",
                    )
                    .map_err(|e| format!("desc gep 1: {e}"))?
            };
            cg.builder
                .build_store(off1, i64_ty.const_int(kind_val, false))
                .map_err(|e| format!("desc store 1: {e}"))?;

            // field 2: size (byte count for ynz_free; 0 for ynz_array_drop)
            let off2 = unsafe {
                cg.builder
                    .build_gep(
                        i8_ty,
                        desc_ptr,
                        &[i64_ty.const_int(entry_byte_base + 16, false)],
                        "desc_off2",
                    )
                    .map_err(|e| format!("desc gep 2: {e}"))?
            };
            cg.builder
                .build_store(off2, i64_ty.const_int(*size, false))
                .map_err(|e| format!("desc store 2: {e}"))?;
        }

        (
            desc_ptr.into(),
            i64_ty.const_int(heap_args.len() as u64, false),
        )
    };

    // Step 6: call ynz_rt_spawn with the frame + arg-drop descriptor.
    cg.builder
        .build_call(
            cg.rt.ynz_rt_spawn,
            &[
                resume_ptr.into(),
                frame_ptr.into(),
                frame_size_val.into(),
                rec_slot_offset_val.into(),
                arg_drop_ptr_val,
                arg_drop_count_val.into(),
            ],
            "sm_spawn",
        )
        .map_err(|e| format!("ynz_rt_spawn: {e}"))?;

    Ok(cg.i32().const_int(0, false).into())
}

/// Coerce a decimal128 (N≤34) operand to a bignum C-string when the other side is
/// bignum (N>34). Returns (lhs, rhs) with both as bignum string pointers when needed.
fn coerce_to_bignum_pair<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    lhs_ty: &Type,
    rhs_ty: &Type,
) -> Result<(BasicValueEnum<'ctx>, BasicValueEnum<'ctx>), String> {
    let lhs_big = matches!(lhs_ty, Type::Number { precision } if *precision > 34);
    let rhs_big = matches!(rhs_ty, Type::Number { precision } if *precision > 34);
    if !lhs_big && !rhs_big {
        return Ok((lhs, rhs));
    }
    let lhs2 = if matches!(lhs_ty, Type::Number { precision } if *precision <= 34) {
        let s = cg
            .builder
            .build_call(cg.rt.decimal_to_string, &[lhs.into()], "lhs_dec2str")
            .map_err(|e| format!("{e}"))?;
        s.try_as_basic_value()
            .basic()
            .ok_or_else(|| "decimal_to_string returned void".to_string())?
    } else {
        lhs
    };
    let rhs2 = if matches!(rhs_ty, Type::Number { precision } if *precision <= 34) {
        let s = cg
            .builder
            .build_call(cg.rt.decimal_to_string, &[rhs.into()], "rhs_dec2str")
            .map_err(|e| format!("{e}"))?;
        s.try_as_basic_value()
            .basic()
            .ok_or_else(|| "decimal_to_string returned void".to_string())?
    } else {
        rhs
    };
    Ok((lhs2, rhs2))
}

fn lower_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    op: &BinOpKind,
    lhs_e: &Expr,
    rhs_e: &Expr,
    lhs_ty: &Type,
    rhs_ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    use BinOpKind::*;
    if matches!(op, And | Or) {
        return lower_short_circuit(cg, matches!(op, And), lhs_e, rhs_e);
    }
    let lhs = lower_expr(cg, lhs_e)?;
    let rhs = lower_expr(cg, rhs_e)?;

    // M8 P6: when one operand is bignum (N>34) and the other is decimal128 (N≤34),
    // coerce the N≤34 side to a C string so both operands are bignum-compatible.
    let (lhs, rhs) = coerce_to_bignum_pair(cg, lhs, rhs, lhs_ty, rhs_ty)?;

    match (op, lhs_ty) {
        (Add, Type::Int) => int_arith_overflow(
            cg,
            lhs.into_int_value(),
            rhs.into_int_value(),
            op,
            lhs_e.span().start as u32,
        ),
        (Sub, Type::Int) => int_arith_overflow(
            cg,
            lhs.into_int_value(),
            rhs.into_int_value(),
            op,
            lhs_e.span().start as u32,
        ),
        (Mul, Type::Int) => int_arith_overflow(
            cg,
            lhs.into_int_value(),
            rhs.into_int_value(),
            op,
            lhs_e.span().start as u32,
        ),
        (Div, Type::Int) => int_divrem(
            cg,
            lhs.into_int_value(),
            rhs.into_int_value(),
            false,
            lhs_e.span().start as u32,
        ),
        (Rem, Type::Int) => int_divrem(
            cg,
            lhs.into_int_value(),
            rhs.into_int_value(),
            true,
            lhs_e.span().start as u32,
        ),

        (Add, Type::Float) => cg
            .builder
            .build_float_add(lhs.into_float_value(), rhs.into_float_value(), "fadd")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Sub, Type::Float) => cg
            .builder
            .build_float_sub(lhs.into_float_value(), rhs.into_float_value(), "fsub")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Mul, Type::Float) => cg
            .builder
            .build_float_mul(lhs.into_float_value(), rhs.into_float_value(), "fmul")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Div, Type::Float) => cg
            .builder
            .build_float_div(lhs.into_float_value(), rhs.into_float_value(), "fdiv")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Rem, Type::Float) => cg
            .builder
            .build_float_rem(lhs.into_float_value(), rhs.into_float_value(), "frem")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),

        (Add, Type::Number { precision }) if *precision <= 34 => decimal_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            cg.rt.decimal_add,
            "dadd",
        ),
        (Add, Type::Number { precision }) => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_add,
            "bnadd",
        ),
        (Sub, Type::Number { precision }) if *precision <= 34 => decimal_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            cg.rt.decimal_sub,
            "dsub",
        ),
        (Sub, Type::Number { precision }) => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_sub,
            "bnsub",
        ),
        (Mul, Type::Number { precision }) if *precision <= 34 => decimal_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            cg.rt.decimal_mul,
            "dmul",
        ),
        (Mul, Type::Number { precision }) => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_mul,
            "bnmul",
        ),
        (Div, Type::Number { precision }) if *precision > 34 => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_div,
            "bndiv",
        ),
        (Div, Type::Number { .. }) => {
            decimal_div(cg, lhs.into_pointer_value(), rhs.into_pointer_value())
        }
        (Rem, Type::Number { .. }) => {
            // typeck already rejected this; emit unreachable panic
            let file_ptr = cg.globals.source_file.as_pointer_value();
            let zero32 = cg.i32().const_int(0, false);
            cg.builder
                .build_call(
                    cg.rt.panic_div_by_zero,
                    &[
                        cg.globals.panic_dec_rem.as_pointer_value().into(),
                        file_ptr.into(),
                        zero32.into(),
                        zero32.into(),
                    ],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
            let out = cg
                .builder
                .build_alloca(cg.i128(), "rem_dead")
                .map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }

        (Lt, Type::Int) => icmp(cg, IntPredicate::SLT, lhs, rhs, "ilt"),
        (LtEq, Type::Int) => icmp(cg, IntPredicate::SLE, lhs, rhs, "ile"),
        (Gt, Type::Int) => icmp(cg, IntPredicate::SGT, lhs, rhs, "igt"),
        (GtEq, Type::Int) => icmp(cg, IntPredicate::SGE, lhs, rhs, "ige"),
        (EqEq, Type::Int) => icmp(cg, IntPredicate::EQ, lhs, rhs, "ieq"),
        (NotEq, Type::Int) => icmp(cg, IntPredicate::NE, lhs, rhs, "ine"),

        (Lt, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OLT, lhs, rhs, "flt"),
        (LtEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OLE, lhs, rhs, "fle"),
        (Gt, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OGT, lhs, rhs, "fgt"),
        (GtEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OGE, lhs, rhs, "fge"),
        (EqEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OEQ, lhs, rhs, "feq"),
        (NotEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::ONE, lhs, rhs, "fne"),

        (EqEq | NotEq | Lt | LtEq | Gt | GtEq, Type::Number { .. }) => {
            decimal_compare(cg, lhs.into_pointer_value(), rhs.into_pointer_value(), op)
        }

        (EqEq, Type::Bool) => icmp(cg, IntPredicate::EQ, lhs, rhs, "beq"),
        (NotEq, Type::Bool) => icmp(cg, IntPredicate::NE, lhs, rhs, "bne"),

        // M6: options equality — `icmp eq i8`
        (EqEq, Type::Options { .. }) => icmp(cg, IntPredicate::EQ, lhs, rhs, "oeq"),
        (NotEq, Type::Options { .. }) => icmp(cg, IntPredicate::NE, lhs, rhs, "one"),

        // M7 P4b: string equality/inequality via ynz_string_eq (NFC normalized).
        (EqEq, Type::String) => {
            let call = cg
                .builder
                .build_call(cg.rt.string_eq, &[lhs.into(), rhs.into()], "s_eq")
                .map_err(|e| format!("{e}"))?;
            let r = call
                .try_as_basic_value()
                .basic()
                .ok_or("string_eq void")?
                .into_int_value();
            // ynz_string_eq returns i32; convert to i1 for bool.
            cg.builder
                .build_int_compare(IntPredicate::NE, r, cg.i32().const_int(0, false), "s_eq_b")
                .map(|v| v.into())
                .map_err(|e| format!("{e}"))
        }
        (NotEq, Type::String) => {
            let call = cg
                .builder
                .build_call(cg.rt.string_eq, &[lhs.into(), rhs.into()], "s_ne")
                .map_err(|e| format!("{e}"))?;
            let r = call
                .try_as_basic_value()
                .basic()
                .ok_or("string_eq void")?
                .into_int_value();
            // Invert: EQ → 0 means not-equal.
            cg.builder
                .build_int_compare(IntPredicate::EQ, r, cg.i32().const_int(0, false), "s_ne_b")
                .map(|v| v.into())
                .map_err(|e| format!("{e}"))
        }

        (BitAnd, Type::Int) => cg
            .builder
            .build_and(lhs.into_int_value(), rhs.into_int_value(), "band")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (BitOr, Type::Int) => cg
            .builder
            .build_or(lhs.into_int_value(), rhs.into_int_value(), "bor")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (BitXor, Type::Int) => cg
            .builder
            .build_xor(lhs.into_int_value(), rhs.into_int_value(), "bxor")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Shl, Type::Int) => cg
            .builder
            .build_left_shift(lhs.into_int_value(), rhs.into_int_value(), "shl")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Shr, Type::Int) => cg
            .builder
            .build_right_shift(lhs.into_int_value(), rhs.into_int_value(), true, "shr")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),

        _ => Err(format!("codegen: unsupported binop {:?} {:?}", op, lhs_ty)),
    }
}

fn icmp<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    pred: IntPredicate,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    cg.builder
        .build_int_compare(pred, lhs.into_int_value(), rhs.into_int_value(), name)
        .map(|v| v.into())
        .map_err(|e| format!("{e}"))
}

fn fcmp<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    pred: inkwell::FloatPredicate,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    cg.builder
        .build_float_compare(pred, lhs.into_float_value(), rhs.into_float_value(), name)
        .map(|v| v.into())
        .map_err(|e| format!("{e}"))
}

fn int_arith_overflow<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
    op: &BinOpKind,
    span_start: u32,
) -> Result<BasicValueEnum<'ctx>, String> {
    let (intrinsic, msg_g) = match op {
        BinOpKind::Add => (cg.rt.sadd_overflow, cg.globals.panic_int_add),
        BinOpKind::Sub => (cg.rt.ssub_overflow, cg.globals.panic_int_sub),
        BinOpKind::Mul => (cg.rt.smul_overflow, cg.globals.panic_int_mul),
        _ => unreachable!(),
    };
    let call = cg
        .builder
        .build_call(intrinsic, &[lhs.into(), rhs.into()], "ov_res")
        .map_err(|e| format!("{e}"))?;
    let s = call
        .try_as_basic_value()
        .basic()
        .ok_or("overflow intrinsic void")?
        .into_struct_value();
    let sum = cg
        .builder
        .build_extract_value(s, 0, "sum")
        .map_err(|e| format!("{e}"))?
        .into_int_value();
    let ov = cg
        .builder
        .build_extract_value(s, 1, "ov")
        .map_err(|e| format!("{e}"))?
        .into_int_value();

    let ok_bb = cg.append_block("ov_ok");
    let panic_bb = cg.append_block("ov_panic");
    cg.builder
        .build_conditional_branch(ov, panic_bb, ok_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(panic_bb);
    let file_ptr = cg.globals.source_file.as_pointer_value();
    let offset_val = cg.i32().const_int(span_start as u64, false);
    let zero_col = cg.i32().const_int(0, false);
    cg.builder
        .build_call(
            cg.rt.panic_overflow,
            &[
                msg_g.as_pointer_value().into(),
                file_ptr.into(),
                offset_val.into(),
                zero_col.into(),
            ],
            "",
        )
        .map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ok_bb);
    Ok(sum.into())
}

fn int_divrem<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
    is_rem: bool,
    span_start: u32,
) -> Result<BasicValueEnum<'ctx>, String> {
    let zero = cg.i64().const_int(0, false);
    let is_z = cg
        .builder
        .build_int_compare(IntPredicate::EQ, rhs, zero, "div_zero")
        .map_err(|e| format!("{e}"))?;
    let ok_bb = cg.append_block("div_ok");
    let pbb = cg.append_block("div_panic");
    cg.builder
        .build_conditional_branch(is_z, pbb, ok_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(pbb);
    let msg = if is_rem {
        cg.globals.panic_int_rem
    } else {
        cg.globals.panic_int_div
    };
    let file_ptr = cg.globals.source_file.as_pointer_value();
    let offset_val = cg.i32().const_int(span_start as u64, false);
    let zero_col = cg.i32().const_int(0, false);
    cg.builder
        .build_call(
            cg.rt.panic_div_by_zero,
            &[
                msg.as_pointer_value().into(),
                file_ptr.into(),
                offset_val.into(),
                zero_col.into(),
            ],
            "",
        )
        .map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ok_bb);
    if is_rem {
        cg.builder
            .build_int_signed_rem(lhs, rhs, "srem")
            .map(|v| v.into())
            .map_err(|e| format!("{e}"))
    } else {
        cg.builder
            .build_int_signed_div(lhs, rhs, "sdiv")
            .map(|v| v.into())
            .map_err(|e| format!("{e}"))
    }
}

fn decimal_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    rt_fn: FunctionValue<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let out = cg
        .builder
        .build_alloca(cg.i128(), name)
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_call(rt_fn, &[lhs.into(), rhs.into(), out.into()], "")
        .map_err(|e| format!("{e}"))?;
    Ok(out.into())
}

/// M8 P6: bignum arithmetic call — (a: *i8, b: *i8, precision: i32) → *i8.
fn bignum_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    precision: u32,
    rt_fn: FunctionValue<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let prec_val = cg.ctx.i32_type().const_int(precision as u64, false);
    let result = cg
        .builder
        .build_call(rt_fn, &[lhs.into(), rhs.into(), prec_val.into()], name)
        .map_err(|e| format!("{e}"))?;
    Ok(result
        .try_as_basic_value()
        .basic()
        .ok_or("bignum op returned void")?)
}

fn decimal_div<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let zero = cg.globals.dec_zero.as_pointer_value();
    let cmp_call = cg
        .builder
        .build_call(
            cg.rt.decimal_compare,
            &[rhs.into(), zero.into()],
            "ddiv_cmp",
        )
        .map_err(|e| format!("{e}"))?;
    let cmp_i32 = cmp_call
        .try_as_basic_value()
        .basic()
        .ok_or("decimal_compare void")?
        .into_int_value();
    let is_z = cg
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            cmp_i32,
            cg.i32().const_int(0, false),
            "ddiv_zero",
        )
        .map_err(|e| format!("{e}"))?;
    let ok_bb = cg.append_block("ddiv_ok");
    let pbb = cg.append_block("ddiv_panic");
    cg.builder
        .build_conditional_branch(is_z, pbb, ok_bb)
        .map_err(|e| format!("{e}"))?;
    cg.builder.position_at_end(pbb);
    let file_ptr = cg.globals.source_file.as_pointer_value();
    let zero32 = cg.i32().const_int(0, false);
    cg.builder
        .build_call(
            cg.rt.panic_div_by_zero,
            &[
                cg.globals.panic_dec_div.as_pointer_value().into(),
                file_ptr.into(),
                zero32.into(),
                zero32.into(),
            ],
            "",
        )
        .map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
    cg.builder.position_at_end(ok_bb);
    decimal_binop(cg, lhs, rhs, cg.rt.decimal_div, "ddiv")
}

fn decimal_compare<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    op: &BinOpKind,
) -> Result<BasicValueEnum<'ctx>, String> {
    let c = cg
        .builder
        .build_call(cg.rt.decimal_compare, &[lhs.into(), rhs.into()], "dcmp")
        .map_err(|e| format!("{e}"))?;
    let ci = c
        .try_as_basic_value()
        .basic()
        .ok_or("cmp void")?
        .into_int_value();
    let z = cg.i32().const_int(0, false);
    let pred = match op {
        BinOpKind::Lt => IntPredicate::SLT,
        BinOpKind::LtEq => IntPredicate::SLE,
        BinOpKind::Gt => IntPredicate::SGT,
        BinOpKind::GtEq => IntPredicate::SGE,
        BinOpKind::EqEq => IntPredicate::EQ,
        BinOpKind::NotEq => IntPredicate::NE,
        _ => unreachable!(),
    };
    cg.builder
        .build_int_compare(pred, ci, z, "dcmp_b")
        .map(|v| v.into())
        .map_err(|e| format!("{e}"))
}

fn lower_short_circuit<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    is_and: bool,
    lhs_e: &Expr,
    rhs_e: &Expr,
) -> Result<BasicValueEnum<'ctx>, String> {
    let bool_ty = cg.bool();
    let lhs = lower_expr(cg, lhs_e)?.into_int_value();
    let lhs_bb = cg.builder.get_insert_block().ok_or("no lhs block")?;

    let rhs_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_rhs" } else { "or_rhs" });
    let short_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_short" } else { "or_short" });
    let merge_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_merge" } else { "or_merge" });

    let _ = lhs_bb;
    if is_and {
        cg.builder.build_conditional_branch(lhs, rhs_bb, short_bb)
    } else {
        cg.builder.build_conditional_branch(lhs, short_bb, rhs_bb)
    }
    .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(rhs_bb);
    let rhs = lower_expr(cg, rhs_e)?.into_int_value();
    let rhs_bb_end = cg.builder.get_insert_block().ok_or("no rhs end block")?;
    cg.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(short_bb);
    cg.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(merge_bb);
    let phi = cg
        .builder
        .build_phi(bool_ty, if is_and { "and_r" } else { "or_r" })
        .map_err(|e| format!("{e}"))?;
    let short_val = bool_ty.const_int(if is_and { 0 } else { 1 }, false);
    phi.add_incoming(&[(&rhs, rhs_bb_end), (&short_val, short_bb)]);
    Ok(phi.as_basic_value())
}

fn lower_unary<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    op: &UnaryOpKind,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    match (op, ty) {
        (UnaryOpKind::Neg, Type::Int) => cg
            .builder
            .build_int_neg(val.into_int_value(), "neg")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (UnaryOpKind::Neg, Type::Float) => cg
            .builder
            .build_float_neg(val.into_float_value(), "fneg")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (UnaryOpKind::Neg, Type::Number { .. }) => {
            let zero = cg.globals.dec_zero.as_pointer_value();
            decimal_binop(
                cg,
                zero,
                val.into_pointer_value(),
                cg.rt.decimal_sub,
                "dec_neg",
            )
        }
        (UnaryOpKind::Not, Type::Bool) => cg
            .builder
            .build_xor(val.into_int_value(), cg.bool().const_int(1, false), "not")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (UnaryOpKind::BitNot, Type::Int) => cg
            .builder
            .build_not(val.into_int_value(), "bitnot")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        _ => Err(format!("codegen: unsupported unary {:?} {:?}", op, ty)),
    }
}

fn lower_print<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<(), String> {
    let p = to_c_string(cg, val, ty)?;
    cg.builder
        .build_call(cg.rt.puts, &[p.into()], "puts")
        .map_err(|e| format!("{e}"))?;
    Ok(())
}

/// Terminate a byte vector as a C string, aborting with an ICE message if any
/// embedded NUL is found.
///
/// The lexer rejects `\0` in string literals (Batch 5a.6), so this path should
/// be unreachable in v0.1. If reached, an earlier compiler phase silently
/// introduced a NUL — that is a compiler bug, not a user error.
fn push_c_string_terminator(bytes: &mut Vec<u8>) {
    if bytes.contains(&0u8) {
        eprintln!(
            "INTERNAL COMPILER ERROR: string literal contains an embedded NUL byte at codegen \
             time. The lexer should have rejected this. Please file an issue at \
             https://github.com/yinzers/yinz-lang/issues with the source file."
        );
        std::process::abort();
    }
    bytes.push(0);
}

fn to_c_string<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<PointerValue<'ctx>, String> {
    match ty {
        Type::String => Ok(val.into_pointer_value()),

        // M8 P4: sensitive string — delegate to the runtime, which checks
        // YNZ_REVEAL_SENSITIVE at first call and returns either the raw pointer
        // or a static "[REDACTED]" string accordingly.
        Type::Sensitive { .. } => {
            let call = cg
                .builder
                .build_call(cg.rt.ynz_sensitive_to_string, &[val.into()], "sens_str")
                .map_err(|e| format!("{e}"))?;
            let ptr = call
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_sensitive_to_string returned void")?
                .into_pointer_value();
            Ok(ptr)
        }

        Type::Bool => cg
            .builder
            .build_select(
                val.into_int_value(),
                cg.globals.str_true.as_pointer_value(),
                cg.globals.str_false.as_pointer_value(),
                "bstr",
            )
            .map(|v| v.into_pointer_value())
            .map_err(|e| format!("{e}")),

        // Runtime format shims return a ptr into a thread-local static buffer.
        Type::Int => {
            let c = cg
                .builder
                .build_call(cg.rt.int_to_string, &[val.into()], "int_str")
                .map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value()
                .basic()
                .ok_or("int_to_string returned void")?
                .into_pointer_value())
        }
        Type::Float => {
            let c = cg
                .builder
                .build_call(cg.rt.float_to_string, &[val.into()], "flt_str")
                .map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value()
                .basic()
                .ok_or("float_to_string returned void")?
                .into_pointer_value())
        }
        Type::Number { precision } if *precision <= 34 => {
            let c = cg
                .builder
                .build_call(cg.rt.decimal_to_string, &[val.into()], "dec_str")
                .map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value()
                .basic()
                .ok_or("decimal_to_string returned void")?
                .into_pointer_value())
        }
        // M8 P6: bignum (N > 34) is stored as a pointer to a decimal string — pass directly.
        Type::Number { .. } => Ok(val.into_pointer_value()),
        // Default debug representation for user-defined shapes: "ShapeName { field: val, ... }"
        // Visible fields only. Nested shapes are printed recursively.
        Type::Shape { name } => {
            let shape_ptr = val.into_pointer_value();

            // Collect visible field names + types before borrowing cg mutably.
            let (visible_fields, struct_ty) = {
                let shape_def = cg
                    .shape_table
                    .get(name)
                    .ok_or_else(|| format!("to_c_string: no shape `{name}`"))?;
                let visible: Vec<(String, usize, Type)> = shape_def
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| !f.is_hidden)
                    .map(|(idx, f)| (f.name.clone(), idx, f.ty.clone()))
                    .collect();
                let st = cg
                    .shape_types
                    .get(name)
                    .ok_or_else(|| format!("to_c_string: no LLVM type for `{name}`"))?;
                (visible, st)
            };

            let builder_val = cg
                .builder
                .build_call(cg.rt.ynz_string_builder_new, &[], "dbg_bld")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_new void")?
                .into_pointer_value();

            // Helper closure: append a static string literal to the builder.
            let append_static = |cg: &mut Cg<'ctx, '_>, bld: PointerValue<'ctx>, s: &str| {
                let g = build_string_global(cg.ctx, cg.module, s, "dbg_lit");
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[bld.into(), g.as_pointer_value().into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))
                    .map(|_| ())
            };

            // Anonymous inline shapes use synthesized `__anon_*` names — omit the name prefix.
            let open = if name.starts_with("__anon_") {
                "{ ".to_string()
            } else {
                format!("{name} {{ ")
            };
            append_static(cg, builder_val, &open)?;

            for (i, (field_name, field_idx, field_ty)) in visible_fields.iter().enumerate() {
                if i > 0 {
                    append_static(cg, builder_val, ", ")?;
                }
                append_static(cg, builder_val, &format!("{field_name}: "))?;

                let gep = cg
                    .builder
                    .build_struct_gep(struct_ty, shape_ptr, *field_idx as u32, field_name)
                    .map_err(|e| format!("GEP {field_name}: {e}"))?;
                // Fields use different LLVM types depending on the Yinz type:
                //   Number (decimal128) → i128 → pass as pointer
                //   Options            → i8   → zero-extend to i64, use as tag
                //   everything else    → i64  → i64_bits_to
                let field_val: BasicValueEnum = if matches!(field_ty, Type::Number { .. }) {
                    let i128t = cg.ctx.i128_type();
                    let raw = cg
                        .builder
                        .build_load(i128t, gep, "dbg_dec_raw")
                        .map_err(|e| format!("{e}"))?;
                    let slot = cg
                        .builder
                        .build_alloca(i128t, "dbg_dec_slot")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(slot, raw)
                        .map_err(|e| format!("{e}"))?;
                    slot.into()
                } else if matches!(field_ty, Type::Options { .. }) {
                    // Options stored as i8 tag — load as i8, zero-extend to i64.
                    let raw = cg
                        .builder
                        .build_load(cg.ctx.i8_type(), gep, "dbg_opt_raw")
                        .map_err(|e| format!("{e}"))?
                        .into_int_value();
                    let extended = cg
                        .builder
                        .build_int_z_extend(raw, cg.i64(), "dbg_opt_ext")
                        .map_err(|e| format!("{e}"))?;
                    extended.into()
                } else {
                    let bits = cg
                        .builder
                        .build_load(cg.i64(), gep, "dbg_bits")
                        .map_err(|e| format!("{e}"))?
                        .into_int_value();
                    cg.i64_bits_to(bits, field_ty)?
                };
                let field_str = to_c_string(cg, field_val, field_ty)?;

                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[builder_val.into(), field_str.into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))?;
            }

            append_static(cg, builder_val, " }")?;

            let result = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_builder_finalize,
                    &[builder_val.into()],
                    "dbg_str",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_finalize void")?
                .into_pointer_value();

            Ok(result)
        }

        // Array debug representation: "[elem1, elem2, ..., elem20, ... and N more]"
        // Capped at 20 elements — arrays of 10k elements should not flood output.
        Type::BuiltinArray { elem } => {
            const PRINT_CAP: u64 = 20;
            let arr_ptr = val.into_pointer_value();
            let elem = elem.as_ref().clone();

            let builder_val = cg
                .builder
                .build_call(cg.rt.ynz_string_builder_new, &[], "arr_bld")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_new void")?
                .into_pointer_value();

            let append_static = |cg: &mut Cg<'ctx, '_>, bld: PointerValue<'ctx>, s: &str| {
                let g = build_string_global(cg.ctx, cg.module, s, "arr_lit");
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[bld.into(), g.as_pointer_value().into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))
                    .map(|_| ())
            };

            append_static(cg, builder_val, "[")?;

            let count = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr_ptr.into()], "arr_cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_array_count void")?
                .into_int_value();

            let cap_const = cg.i64().const_int(PRINT_CAP, false);
            let use_full = cg
                .builder
                .build_int_compare(IntPredicate::SLE, count, cap_const, "arr_use_full")
                .map_err(|e| format!("{e}"))?;
            let cap = cg
                .builder
                .build_select(use_full, count, cap_const, "arr_cap")
                .map_err(|e| format!("{e}"))?
                .into_int_value();

            // Loop 0..cap, appending each element.
            let i_slot = cg
                .builder
                .build_alloca(cg.i64(), "arr_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;

            let cond_bb = cg.append_block("arr_cond");
            let body_bb = cg.append_block("arr_body");
            let after_bb = cg.append_block("arr_after");

            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(cond_bb);

            let i = cg
                .builder
                .build_load(cg.i64(), i_slot, "arr_i_val")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let cmp = cg
                .builder
                .build_int_compare(IntPredicate::SLT, i, cap, "arr_lt")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(cmp, body_bb, after_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(body_bb);

            // Separator: ", " before all elements except the first.
            let is_first = cg
                .builder
                .build_int_compare(IntPredicate::EQ, i, cg.i64().const_zero(), "arr_first")
                .map_err(|e| format!("{e}"))?;
            let sep_bb = cg.append_block("arr_sep");
            let elem_bb = cg.append_block("arr_elem");
            cg.builder
                .build_conditional_branch(is_first, elem_bb, sep_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(sep_bb);
            append_static(cg, builder_val, ", ")?;
            cg.builder
                .build_unconditional_branch(elem_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(elem_bb);

            // Load element via ynz_array_get into a maybe_type() slot.
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "arr_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr_ptr.into(), i.into(), out.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            let val_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), out, 1, "arr_vgep")
                .map_err(|e| format!("{e}"))?;
            let bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "arr_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let elem_val = cg.i64_bits_to(bits, &elem)?;
            let elem_str = to_c_string(cg, elem_val, &elem)?;

            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), elem_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;

            // i += 1
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "arr_next_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(after_bb);

            // If count > cap, append "... and N more"
            let truncated_bb = cg.append_block("arr_trunc");
            let finish_bb = cg.append_block("arr_finish");
            let was_truncated = cg
                .builder
                .build_int_compare(IntPredicate::SGT, count, cap_const, "arr_trunc_cmp")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(was_truncated, truncated_bb, finish_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(truncated_bb);
            let remaining = cg
                .builder
                .build_int_sub(count, cap_const, "arr_rem")
                .map_err(|e| format!("{e}"))?;
            // Convert remaining count to string and build the "... and N more" suffix.
            let rem_str = cg
                .builder
                .build_call(cg.rt.int_to_string, &[remaining.into()], "arr_rem_str")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("int_to_string void")?
                .into_pointer_value();
            append_static(cg, builder_val, ", ... and ")?;
            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), rem_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            append_static(cg, builder_val, " more")?;
            cg.builder
                .build_unconditional_branch(finish_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(finish_bb);
            append_static(cg, builder_val, "]")?;

            let result = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_builder_finalize,
                    &[builder_val.into()],
                    "arr_str",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_finalize void")?
                .into_pointer_value();

            Ok(result)
        }

        // Map debug representation: "{ key: val, key2: val2 }" using insertion order.
        Type::BuiltinMap {
            key,
            val: map_val_ty,
        } if key_is_string(key) => {
            let map_ptr = val.into_pointer_value();
            let val_ty = map_val_ty.as_ref().clone();

            let builder_val = cg
                .builder
                .build_call(cg.rt.ynz_string_builder_new, &[], "mdbg_bld")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("builder_new void")?
                .into_pointer_value();

            let append_static = |cg: &mut Cg<'ctx, '_>, bld: PointerValue<'ctx>, s: &str| {
                let g = build_string_global(cg.ctx, cg.module, s, "mdbg_lit");
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[bld.into(), g.as_pointer_value().into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))
                    .map(|_| ())
            };

            append_static(cg, builder_val, "{ ")?;

            let count = cg
                .builder
                .build_call(cg.rt.ynz_map_count, &[map_ptr.into()], "mdbg_cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("map_count void")?
                .into_int_value();

            let i_slot = cg
                .builder
                .build_alloca(cg.i64(), "mdbg_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;

            let cond_bb = cg.append_block("mdbg_cond");
            let body_bb = cg.append_block("mdbg_body");
            let after_bb = cg.append_block("mdbg_after");
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(cond_bb);
            let i = cg
                .builder
                .build_load(cg.i64(), i_slot, "mdbg_iv")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let lt = cg
                .builder
                .build_int_compare(IntPredicate::SLT, i, count, "mdbg_lt")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(lt, body_bb, after_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(body_bb);

            let is_first = cg
                .builder
                .build_int_compare(IntPredicate::EQ, i, cg.i64().const_zero(), "mdbg_first")
                .map_err(|e| format!("{e}"))?;
            let sep_bb = cg.append_block("mdbg_sep");
            let entry_bb = cg.append_block("mdbg_entry");
            cg.builder
                .build_conditional_branch(is_first, entry_bb, sep_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(sep_bb);
            append_static(cg, builder_val, ", ")?;
            cg.builder
                .build_unconditional_branch(entry_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(entry_bb);

            let triple_ty = cg
                .ctx
                .struct_type(&[cg.i64().into(), cg.i64().into(), cg.i64().into()], false);
            let triple_slot = cg
                .builder
                .build_alloca(triple_ty, "mdbg_triple")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get_str,
                    &[map_ptr.into(), i.into(), triple_slot.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;

            let key_gep = cg
                .builder
                .build_struct_gep(triple_ty, triple_slot, 1, "mdbg_kgep")
                .map_err(|e| format!("{e}"))?;
            let key_ptr = cg
                .builder
                .build_load(cg.i64(), key_gep, "mdbg_kbits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let key_str = cg
                .builder
                .build_int_to_ptr(key_ptr, cg.ptr(), "mdbg_kptr")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), key_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            append_static(cg, builder_val, ": ")?;

            let val_gep = cg
                .builder
                .build_struct_gep(triple_ty, triple_slot, 2, "mdbg_vgep")
                .map_err(|e| format!("{e}"))?;
            let val_bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "mdbg_vbits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let val_val = cg.i64_bits_to(val_bits, &val_ty)?;
            let val_str = to_c_string(cg, val_val, &val_ty)?;
            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), val_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;

            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "mdbg_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(after_bb);
            append_static(cg, builder_val, " }")?;

            let result = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_builder_finalize,
                    &[builder_val.into()],
                    "mdbg_str",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("finalize void")?
                .into_pointer_value();
            Ok(result)
        }
        Type::BuiltinMap { .. } => {
            let g = build_string_global(cg.ctx, cg.module, "{ <map> }", "map_ph");
            Ok(g.as_pointer_value())
        }

        // Union printing: for `T | nothing`, check null → "none", else print as T.
        // For other union shapes, show a placeholder — full union introspection is future work.
        Type::Union { variants } => {
            let non_nothing: Vec<&Type> = variants
                .iter()
                .filter(|v| !matches!(v, Type::Nothing))
                .collect();

            if non_nothing.len() == 1 {
                let inner = non_nothing[0].clone();
                let ptr = val.into_pointer_value();
                let is_null = cg
                    .builder
                    .build_is_null(ptr, "union_null")
                    .map_err(|e| format!("{e}"))?;

                let none_bb = cg.append_block("union_none");
                let some_bb = cg.append_block("union_some");
                let merge_bb = cg.append_block("union_merge");

                cg.builder
                    .build_conditional_branch(is_null, none_bb, some_bb)
                    .map_err(|e| format!("{e}"))?;

                cg.builder.position_at_end(none_bb);
                let none_str = build_string_global(cg.ctx, cg.module, "none", "union_none_str");
                cg.builder
                    .build_unconditional_branch(merge_bb)
                    .map_err(|e| format!("{e}"))?;

                cg.builder.position_at_end(some_bb);
                let some_str = to_c_string(cg, val, &inner)?;
                cg.builder
                    .build_unconditional_branch(merge_bb)
                    .map_err(|e| format!("{e}"))?;
                let some_bb_end = cg.builder.get_insert_block().unwrap();

                cg.builder.position_at_end(merge_bb);
                let phi = cg
                    .builder
                    .build_phi(cg.ptr(), "union_str")
                    .map_err(|e| format!("{e}"))?;
                phi.add_incoming(&[
                    (&none_str.as_pointer_value(), none_bb),
                    (&some_str, some_bb_end),
                ]);
                Ok(phi.as_basic_value().into_pointer_value())
            } else {
                let g = build_string_global(cg.ctx, cg.module, "<union>", "union_placeholder");
                Ok(g.as_pointer_value())
            }
        }

        // Options type — val is the i64-extended i8 tag; delegate to options toString.
        Type::Options { name } => {
            // Cast i64 back to i8 for lower_options_to_string which expects an i8.
            let tag_i64 = val.into_int_value();
            let tag_i8 = cg
                .builder
                .build_int_truncate(tag_i64, cg.ctx.i8_type(), "opt_tag_i8")
                .map_err(|e| format!("{e}"))?;
            lower_options_to_string(cg, tag_i8.into(), name).map(|v| v.into_pointer_value())
        }

        _ => Err(format!("codegen: cannot convert {:?} to string", ty)),
    }
}

/// Convert any Yinz value to a null-terminated C string pointer for interpolation.
///
/// Delegates to `to_c_string` for the types it knows. For Options types, emits a call
/// to the options toString runtime helper. For all other types, defers to `to_c_string`.
fn expr_to_cstring<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<inkwell::values::PointerValue<'ctx>, String> {
    match ty {
        Type::Options { name } => {
            lower_options_to_string(cg, val, name).map(|v| v.into_pointer_value())
        }
        _ => to_c_string(cg, val, ty),
    }
}

/// UFCS dispatch: `receiver.method(args)` → call standalone function `method(receiver, args)`.
fn lower_ufcs_call<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv_val: BasicValueEnum<'ctx>,
    _shape_name: &str,
    method: &str,
    extra_args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    let fn_val = cg
        .module
        .get_function(method)
        .ok_or_else(|| format!("codegen UFCS: function `{}` not found in module", method))?;

    let mut args: Vec<BasicMetadataValueEnum<'ctx>> = vec![recv_val.into()];
    for arg in extra_args {
        let arg_val = lower_expr(cg, arg)?;
        // If the function param expects dynamic but arg is a concrete shape, coerce.
        args.push(arg_val.into());
    }

    let call = cg
        .builder
        .build_call(fn_val, &args, "ufcs")
        .map_err(|e| format!("UFCS call `{method}`: {e}"))?;
    match call.try_as_basic_value().basic() {
        Some(v) => Ok(v),
        None => Ok(cg.i32().const_int(0, false).into()),
    }
}

/// Field access: lower `receiver.field` to the field's value.
///
/// Handles `maybe<T>.value` (extract the inner value from the {i64,i64} slot) and
/// shape field GEP for user-defined shapes.
fn lower_field_access<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    field_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let recv_ty = cg.expr_type(receiver);

    // MapEntry field access: entry.key → field 0, entry.value → field 1.
    if let Type::MapEntry { key, val } = &recv_ty {
        let entry_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into()], false);
        let entry_ptr = lower_expr(cg, receiver)?.into_pointer_value();
        let (field_idx, field_ty) = match field_name {
            "key" => (0u32, key.as_ref().clone()),
            "value" => (1u32, val.as_ref().clone()),
            f => return Err(format!("MapEntry has no field `{f}`")),
        };
        let gep = cg
            .builder
            .build_struct_gep(entry_ty, entry_ptr, field_idx, field_name)
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), gep, "me_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        return cg.i64_bits_to(bits, &field_ty);
    }

    // maybe<T>.value — extract value bits from the {i64,i64} alloca.
    if let Type::Maybe { inner } = &recv_ty {
        let inner = inner.as_ref().clone();
        let ptr = lower_expr(cg, receiver)?.into_pointer_value();
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), ptr, 1, "mv_gep")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "mv_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        return cg.i64_bits_to(bits, &inner);
    }

    let (field_ptr, field_ty) = field_gep(cg, receiver, field_name)?;
    load(cg, field_ptr, &field_ty, field_name)
}

/// Get a GEP pointer to a field inside a shape value (for reads AND writes).
fn field_gep<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    field_name: &str,
) -> Result<(PointerValue<'ctx>, Type), String> {
    let recv_ty = cg.expr_type(receiver);
    let shape_name = match &recv_ty {
        Type::Shape { name } => name.clone(),
        _ => {
            return Err(format!(
                "field_gep: receiver is not a Shape, got {:?}",
                recv_ty
            ))
        }
    };

    let shape_def = cg
        .shape_table
        .get(&shape_name)
        .ok_or_else(|| format!("field_gep: shape `{}` not in table", shape_name))?;
    let field_idx = shape_def
        .fields
        .iter()
        .position(|f| f.name == field_name)
        .ok_or_else(|| {
            format!(
                "field_gep: field `{}` not found in shape `{}`",
                field_name, shape_name
            )
        })?;
    let field_ty = shape_def.fields[field_idx].ty.clone();

    let struct_ty = cg
        .shape_types
        .get(&shape_name)
        .ok_or_else(|| format!("field_gep: LLVM type for shape `{}` not found", shape_name))?;

    // Lower the receiver to get a pointer to the struct.
    let recv_ptr = lower_expr(cg, receiver)?;

    let gep = cg
        .builder
        .build_struct_gep(
            struct_ty,
            recv_ptr.into_pointer_value(),
            field_idx as u32,
            field_name,
        )
        .map_err(|e| format!("GEP field `{}`: {e}", field_name))?;

    Ok((gep, field_ty))
}

/// Lower a struct literal to a stack-allocated value; return pointer to it.
fn lower_struct_lit<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    expr: &Expr,
    fields: &[ynz_ast::nodes::StructLitField],
) -> Result<BasicValueEnum<'ctx>, String> {
    let ty = cg.expr_type(expr);
    let Type::Shape { name: shape_name } = &ty else {
        return Err(format!("struct literal with non-shape type {:?}", ty));
    };
    let shape_name = shape_name.clone();

    let struct_ty = cg
        .shape_types
        .get(&shape_name)
        .ok_or_else(|| format!("struct_lit: LLVM type for `{}` not found", shape_name))?;

    // Allocate the struct on the stack.
    let slot = cg
        .builder
        .build_alloca(struct_ty, &shape_name)
        .map_err(|e| format!("alloca {}: {e}", shape_name))?;

    // Zero-initialize — covers hidden fields that aren't provided.
    let zero = struct_ty.const_zero();
    cg.builder
        .build_store(slot, zero)
        .map_err(|e| format!("zero-init {}: {e}", shape_name))?;

    let shape_def = cg
        .shape_table
        .get(&shape_name)
        .ok_or_else(|| format!("struct_lit: shape `{}` not in table", shape_name))?
        .clone();

    // Evaluate and store each provided field.
    for lit_field in fields {
        let field_idx = shape_def
            .fields
            .iter()
            .position(|f| f.name == lit_field.name)
            .ok_or_else(|| format!("struct_lit: unknown field `{}`", lit_field.name))?;
        let field_ty = shape_def.fields[field_idx].ty.clone();

        let gep = cg
            .builder
            .build_struct_gep(struct_ty, slot, field_idx as u32, &lit_field.name)
            .map_err(|e| format!("struct_lit GEP `{}`: {e}", lit_field.name))?;

        let val = lower_expr(cg, &lit_field.value)?;
        store_field(cg, val, &field_ty, gep)?;
    }

    // Evaluate and store non-zero hidden-field defaults.
    //
    // Walk all AST ShapeDecls in the inheritance chain (current shape + every ancestor
    // via `extends`) to find hidden fields with explicit default expressions.
    // Zero-init above already covers zero-default fields; only emit a GEP+store when
    // the default expression evaluates to something non-zero.
    //
    // Inherited hidden fields live in the parent's AST ShapeDecl, not the child's —
    // so we must walk the chain.  The GEP index is always looked up from the resolved
    // ShapeDef.fields (which includes inherited fields in layout order) to produce
    // the correct LLVM struct offset.
    let ast_shape_decl_for = |name: &str| -> Option<&ynz_ast::nodes::ShapeDecl> {
        cg.typed.module.items.iter().find_map(|item| {
            if let ynz_ast::nodes::Item::ShapeDecl(s) = item {
                if s.name == name {
                    Some(s)
                } else {
                    None
                }
            } else {
                None
            }
        })
    };

    // Collect the shape names to visit: walk up the extends chain.
    let mut chain: Vec<String> = vec![shape_name.clone()];
    {
        let mut cur = shape_name.clone();
        while let Some(parent) = cg.shape_table.get(&cur).and_then(|s| s.extends.clone()) {
            chain.push(parent.clone());
            cur = parent;
        }
    }

    for decl_name in &chain {
        let Some(ast_shape) = ast_shape_decl_for(decl_name) else {
            continue;
        };
        for ast_field in &ast_shape.fields {
            if !ast_field.is_hidden {
                continue;
            }
            let Some(default_expr) = &ast_field.default else {
                continue;
            };
            // Find this field's index in the resolved ShapeDef (which includes
            // inherited fields; the LLVM struct layout follows ShapeDef.fields order).
            let field_idx = match shape_def
                .fields
                .iter()
                .position(|f| f.name == ast_field.name)
            {
                Some(idx) => idx,
                None => continue, // field not in resolved def — skip safely
            };
            let field_ty = shape_def.fields[field_idx].ty.clone();

            let gep = cg
                .builder
                .build_struct_gep(
                    struct_ty,
                    slot,
                    field_idx as u32,
                    &format!("{}.{}_default", shape_name, ast_field.name),
                )
                .map_err(|e| format!("hidden default GEP `{}`: {e}", ast_field.name))?;

            let val = lower_expr(cg, default_expr)?;
            store_field(cg, val, &field_ty, gep)?;
        }
    }

    Ok(slot.into())
}

/// Field assignment helper: `target.field = value`.
fn lower_stmt_field_assign<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    target: &Expr,
    value: &Expr,
) -> Result<(), String> {
    // Expect target to be Expr::FieldAccess
    let Expr::FieldAccess {
        receiver, field, ..
    } = target
    else {
        return Err(format!(
            "codegen: FieldAssign target is not FieldAccess: {:?}",
            target
        ));
    };

    let (field_ptr, field_ty) = field_gep(cg, receiver, field)?;
    let val = lower_expr(cg, value)?;
    store_field(cg, val, &field_ty, field_ptr)
}

/// PostfixOp lowering: `.copy()` and `.freeze()`.
fn lower_postfix_op<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    op: &ynz_ast::nodes::PostfixOpKind,
) -> Result<BasicValueEnum<'ctx>, String> {
    use ynz_ast::nodes::PostfixOpKind;
    match op {
        PostfixOpKind::Freeze => {
            // `.freeze()` is a typeck-only operation; no codegen change.
            lower_expr(cg, receiver)
        }
        PostfixOpKind::Copy => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;
            match &recv_ty {
                Type::Shape { name } => {
                    // Trivially-copyable shape: memcpy into a fresh alloca.
                    let name = name.clone();
                    let struct_ty = cg
                        .shape_types
                        .get(&name)
                        .ok_or_else(|| format!(".copy(): LLVM type for `{}` not found", name))?;
                    let new_slot = cg
                        .builder
                        .build_alloca(struct_ty, &format!("{}_copy", name))
                        .map_err(|e| format!(".copy alloca: {e}"))?;
                    // Load the struct value and store into the new slot.
                    let val = cg
                        .builder
                        .build_load(struct_ty, recv_val.into_pointer_value(), "copy_src")
                        .map_err(|e| format!(".copy load: {e}"))?;
                    cg.builder
                        .build_store(new_slot, val)
                        .map_err(|e| format!(".copy store: {e}"))?;
                    Ok(new_slot.into())
                }
                // For primitives, the value is already by-value — just return it.
                _ => Ok(recv_val),
            }
        }
    }
}

/// Emit a type-attached constant such as `int.max` or `number.epsilon`.
///
/// All data (value_type and value_literal) lives in registry/features.toml.
fn emit_type_const<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    type_name: &str,
    const_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let entry = ynz_registry::type_attached_constant_lookup(type_name, const_name)
        .ok_or_else(|| format!("no registry entry for {type_name}.{const_name}"))?;

    match entry.value_type {
        "int" => {
            let v: i64 = entry.value_literal.parse()
                .map_err(|e| format!("int literal parse error for {type_name}.{const_name}: {e}"))?;
            Ok(cg.i64().const_int(v as u64, v < 0).into())
        }
        "float" => {
            let v: f64 = entry.value_literal.parse()
                .map_err(|e| format!("float literal parse error for {type_name}.{const_name}: {e}"))?;
            Ok(cg.f64().const_float(v).into())
        }
        "number" => {
            let s = entry.value_literal;
            let bits = ynz_numerics::parse(s)
                .ok_or_else(|| format!("failed to parse decimal128 constant `{s}`"))?;
            let slot = cg
                .builder
                .build_alloca(cg.i128(), const_name)
                .map_err(|e| format!("{e}"))?;
            let val = cg.i128().const_int_arbitrary_precision(&[
                (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
                (bits >> 64) as u64,
            ]);
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
            Ok(slot.into())
        }
        other => Err(format!(
            "codegen: unknown value_type {other:?} for type-attached constant `{type_name}.{const_name}`"
        )),
    }
}

/// True for method calls that take one primitive argument (dispatched via `lower_method_call_1arg`).
fn is_1arg_intrinsic(recv_ty: &Type, method: &str) -> bool {
    matches!(
        (recv_ty, method),
        (Type::Int, "wrappingAdd")
            | (Type::Int, "wrappingSub")
            | (Type::Int, "wrappingMul")
            | (Type::Int, "saturatingAdd")
            | (Type::Int, "saturatingSub")
            | (Type::Int, "saturatingMul")
    )
}

/// Lower a one-arg primitive intrinsic method call.
fn lower_method_call_1arg<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    recv_ty: &Type,
    method: &str,
    arg: BasicValueEnum<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let a = recv.into_int_value();
    let b = arg.into_int_value();
    match (recv_ty, method) {
        // Wrapping: plain LLVM add/sub/mul — two's complement overflow is the default.
        (Type::Int, "wrappingAdd") => cg
            .builder
            .build_int_add(a, b, "wadd")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Type::Int, "wrappingSub") => cg
            .builder
            .build_int_sub(a, b, "wsub")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Type::Int, "wrappingMul") => cg
            .builder
            .build_int_mul(a, b, "wmul")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        // Saturating: LLVM `sadd.sat` / `ssub.sat` / `smul.fix.sat` (scale=0) intrinsics.
        (Type::Int, "saturatingAdd") => {
            let f = declare_sat_intrinsic(
                cg.module,
                cg.ctx,
                "llvm.sadd.sat.i64",
                cg.i64().fn_type(&[cg.i64().into(), cg.i64().into()], false),
            );
            let r = cg
                .builder
                .build_call(f, &[a.into(), b.into()], "sadd_sat")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("sadd.sat void")?)
        }
        (Type::Int, "saturatingSub") => {
            let f = declare_sat_intrinsic(
                cg.module,
                cg.ctx,
                "llvm.ssub.sat.i64",
                cg.i64().fn_type(&[cg.i64().into(), cg.i64().into()], false),
            );
            let r = cg
                .builder
                .build_call(f, &[a.into(), b.into()], "ssub_sat")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("ssub.sat void")?)
        }
        (Type::Int, "saturatingMul") => {
            // `llvm.smul.fix.sat.i64(a, b, scale=0)` gives saturating signed integer mul.
            let fn_ty = cg
                .i64()
                .fn_type(&[cg.i64().into(), cg.i64().into(), cg.i32().into()], false);
            let f = declare_sat_intrinsic(cg.module, cg.ctx, "llvm.smul.fix.sat.i64", fn_ty);
            let scale = cg.i32().const_int(0, false);
            let r = cg
                .builder
                .build_call(f, &[a.into(), b.into(), scale.into()], "smul_sat")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("smul.fix.sat void")?)
        }
        _ => Err(format!(
            "codegen: no 1-arg intrinsic for {recv_ty:?}.{method}"
        )),
    }
}

fn declare_sat_intrinsic<'ctx>(
    module: &inkwell::module::Module<'ctx>,
    ctx: &'ctx inkwell::context::Context,
    name: &str,
    fn_ty: inkwell::types::FunctionType<'ctx>,
) -> inkwell::values::FunctionValue<'ctx> {
    let _ = ctx; // ctx not needed — type already constructed
    module
        .get_function(name)
        .unwrap_or_else(|| module.add_function(name, fn_ty, None))
}

/// Find the mangled name for a generic function call by matching the base name in the mono table.
///
/// When there are multiple instantiations of the same generic (e.g. `identity<int>` and
/// `identity<string>`), this picks the first match. For P4a the typical case is a single
/// instantiation per call site — a precise match by argument types is a P4b refinement.
fn find_mono_name_by_args(
    mono_table: &MonomorphizationTable,
    fn_name: &str,
    arg_types: &[Type],
) -> Option<String> {
    // Try exact match on param types first.
    for (key, sig) in &mono_table.entries {
        if key.fn_name != fn_name {
            continue;
        }
        if sig.param_types.len() == arg_types.len()
            && sig.param_types.iter().zip(arg_types).all(|(a, b)| a == b)
        {
            return Some(mangle_mono_name(&key.fn_name, &key.type_args));
        }
    }
    // Fall back: first entry with matching name (single-instantiation case).
    mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == fn_name)
        .map(|k| mangle_mono_name(&k.fn_name, &k.type_args))
}

// ── BuiltinArray method dispatch ─────────────────────────────────────────────

fn lower_array_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    arr: PointerValue<'ctx>,
    elem: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        "count" => {
            let n = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr.into()], "arr_count")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_array_count returned void")?;
            Ok(n)
        }
        "add" => {
            let val = lower_expr(cg, &args[0])?;
            let elem_ty = cg.expr_type(&args[0]);
            let bits = cg.to_i64_bits(val, &elem_ty)?;
            cg.builder
                .build_call(cg.rt.ynz_array_push, &[arr.into(), bits.into()], "arr_add")
                .map_err(|e| format!("{e}"))?;
            Ok(cg.i64().const_zero().into())
        }
        "get" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "get_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), idx.into(), out.into()],
                    "arr_get_m",
                )
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(out.into())
        }
        "first" => {
            let idx = cg.i64().const_zero();
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "first_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), idx.into(), out.into()],
                    "arr_first",
                )
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(out.into())
        }
        "last" => {
            let cnt = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr.into()], "cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("count void")?
                .into_int_value();
            let idx = cg
                .builder
                .build_int_sub(cnt, cg.i64().const_int(1, false), "last_idx")
                .map_err(|e| format!("{e}"))?;
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "last_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), idx.into(), out.into()],
                    "arr_last",
                )
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(out.into())
        }
        "set" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let val = lower_expr(cg, &args[1])?;
            let elem_ty = cg.expr_type(&args[1]);
            let bits = cg.to_i64_bits(val, &elem_ty)?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_set,
                    &[arr.into(), idx.into(), bits.into()],
                    "arr_set_m",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(cg.i64().const_zero().into())
        }
        "contains" => {
            // Linear scan: bool result.
            let cnt = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr.into()], "cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("count void")?
                .into_int_value();
            let target_val = lower_expr(cg, &args[0])?;
            let target_ty = cg.expr_type(&args[0]);
            let target_bits = cg.to_i64_bits(target_val, &target_ty)?;

            let result_slot = cg
                .builder
                .build_alloca(cg.bool(), "contains_res")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(result_slot, cg.bool().const_int(0, false))
                .map_err(|e| format!("{e}"))?;
            let i_slot = cg
                .builder
                .build_alloca(cg.i64(), "ci")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;

            let loop_bb = cg.append_block("contains_loop");
            let body_bb = cg.append_block("contains_body");
            let found_bb = cg.append_block("c_found");
            let cont_bb = cg.append_block("c_cont");
            let exit_bb = cg.append_block("contains_exit");

            cg.builder
                .build_unconditional_branch(loop_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(loop_bb);
            let i = cg
                .builder
                .build_load(cg.i64(), i_slot, "i")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let cmp = cg
                .builder
                .build_int_compare(IntPredicate::SLT, i, cnt, "lt")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(cmp, body_bb, exit_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(body_bb);
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "c_get")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), i.into(), out.into()],
                    "c_get_call",
                )
                .map_err(|e| format!("{e}"))?;
            let val_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), out, 1, "c_val")
                .map_err(|e| format!("{e}"))?;
            let elem_bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "c_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let eq = cg
                .builder
                .build_int_compare(IntPredicate::EQ, elem_bits, target_bits, "eq")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(eq, found_bb, cont_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(found_bb);
            cg.builder
                .build_store(result_slot, cg.bool().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(exit_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(cont_bb);
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "next_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(loop_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(exit_bb);
            let res = cg
                .builder
                .build_load(cg.bool(), result_slot, "res")
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(res)
        }
        other => Err(format!("array method `{other}` not yet lowered in P4a")),
    }
}

// ── BuiltinFixed method dispatch ──────────────────────────────────────────────

fn lower_fixed_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    arr: PointerValue<'ctx>,
    elem: &Type,
    size: Option<usize>,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        "count" => {
            let n = size.unwrap_or(0) as u64;
            Ok(cg.i64().const_int(n, false).into())
        }
        "get" | "first" | "last" => {
            let n = size.unwrap_or(0) as u64;
            let idx = if method == "first" {
                cg.i64().const_zero()
            } else if method == "last" && n > 0 {
                cg.i64().const_int(n - 1, false)
            } else {
                lower_expr(cg, &args[0])?.into_int_value()
            };
            let maybe_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "fg")
                .map_err(|e| format!("{e}"))?;
            let in_bounds = if n > 0 {
                let idx_ext = cg
                    .builder
                    .build_int_z_extend_or_bit_cast(idx, cg.i64(), "ie")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_int_compare(
                        IntPredicate::ULT,
                        idx_ext,
                        cg.i64().const_int(n, false),
                        "ib",
                    )
                    .map_err(|e| format!("{e}"))?
            } else {
                cg.bool().const_int(0, false)
            };
            let ok_bb = cg.append_block("fg_ok");
            let oob_bb = cg.append_block("fg_oob");
            let merge_bb = cg.append_block("fg_merge");
            cg.builder
                .build_conditional_branch(in_bounds, ok_bb, oob_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(ok_bb);
            let gep = unsafe {
                cg.builder
                    .build_gep(cg.i64(), arr, &[idx], "fg_elem")
                    .map_err(|e| format!("{e}"))?
            };
            let bits = cg
                .builder
                .build_load(cg.i64(), gep, "bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let has0 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_ok")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(has0, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let val0 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_ok")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val0, bits)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(oob_bb);
            let has1 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_oob")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(has1, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            let val1 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_oob")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val1, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(merge_bb);
            let _ = elem;
            Ok(maybe_slot.into())
        }
        "set" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let val = lower_expr(cg, &args[1])?;
            let vty = cg.expr_type(&args[1]);
            let bits = cg.to_i64_bits(val, &vty)?;
            let gep = unsafe {
                cg.builder
                    .build_gep(cg.i64(), arr, &[idx], "fs_elem")
                    .map_err(|e| format!("{e}"))?
            };
            cg.builder
                .build_store(gep, bits)
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(cg.i64().const_zero().into())
        }
        other => Err(format!("fixed method `{other}` not yet lowered in P4a")),
    }
}

// ── Maybe method dispatch ─────────────────────────────────────────────────────

fn lower_maybe_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    maybe_ptr: PointerValue<'ctx>,
    inner: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        "exists" => {
            let has_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_ptr, 0, "has")
                .map_err(|e| format!("{e}"))?;
            let has = cg
                .builder
                .build_load(cg.i64(), has_gep, "has_val")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let b = cg
                .builder
                .build_int_truncate(has, cg.bool(), "exists_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "or" => {
            let has_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_ptr, 0, "or_has")
                .map_err(|e| format!("{e}"))?;
            let has = cg
                .builder
                .build_load(cg.i64(), has_gep, "or_hv")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let cond = cg
                .builder
                .build_int_compare(IntPredicate::NE, has, cg.i64().const_zero(), "or_cond")
                .map_err(|e| format!("{e}"))?;

            let val_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_ptr, 1, "or_val_gep")
                .map_err(|e| format!("{e}"))?;
            let bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "or_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let inner_val = cg.i64_bits_to(bits, inner)?;
            let default_val = lower_expr(cg, &args[0])?;

            cg.builder
                .build_select(cond, inner_val, default_val, "or_res")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!("maybe method `{other}` not yet lowered in P4a")),
    }
}

// ── M7 P4a: errors-capable helpers ───────────────────────────────────────────

/// True when the named function returns a bare `number` (decimal128, precision ≤ 34) —
/// NOT `number errors`. The non-SM `number` ABI returns a POINTER to a heap-stable 16-byte
/// i128 (see the wrapper at the `Type::Number` arm of the SM wrapper), so a CPU trampoline
/// must DEREFERENCE that pointer to recover the i128 value before packing it into the result
/// slot — unlike string/array/map, where the returned pointer IS the value (`ptr_to_int`).
///
/// Time: O(n) where n = items in the typed module  Space: O(1)
fn callee_returns_bare_number(
    typed: &TypedModule,
    imported_fns: &std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    fn_name: &str,
) -> bool {
    let local = typed.module.items.iter().any(|item| {
        if let ynz_ast::nodes::Item::Function(f) = item {
            f.name == fn_name
                && matches!(
                    f.return_type,
                    ynz_ast::nodes::Type::Number { precision } if precision <= 34
                )
        } else {
            false
        }
    });
    if local {
        return true;
    }
    imported_fns.get(fn_name).is_some_and(
        |sig| matches!(sig.ret, ynz_typeck::types::Type::Number { precision } if precision <= 34),
    )
}

/// True when the named function has `errors_capable = true`, checking both local
/// module items and the imported function table for cross-module callees.
fn is_errors_capable_fn(
    typed: &TypedModule,
    imported_fns: &std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    fn_name: &str,
) -> bool {
    // Check local functions first.
    let local = typed.module.items.iter().any(|item| {
        if let ynz_ast::nodes::Item::Function(f) = item {
            f.name == fn_name && f.errors_capable
        } else {
            false
        }
    });
    if local {
        return true;
    }
    // Check imported functions: ErrorsCapable return type means the function is
    // errors-capable even when the importer's AST has no FunctionDecl for it.
    imported_fns
        .get(fn_name)
        .is_some_and(|sig| matches!(sig.ret, ynz_typeck::types::Type::ErrorsCapable { .. }))
}

/// True when the named function returns `-> number errors` (decimal128 EC).
///
/// The EC ok-word for such functions is a pointer into the callee's 16-byte staging slot
/// (embedded in the shared child sub-frame). Distinct bindings of the same callee share
/// that staging slot — a second call overwrites the slot before the first binding is read.
/// Copy-on-bind must fire for every wide-EC (number errors) binding to give each binding
/// its own per-binding i128 storage that the callee cannot later overwrite.
fn is_number_errors_callee(
    typed: &TypedModule,
    imported_fns: &std::collections::HashMap<String, ynz_typeck::signatures::FunctionSig>,
    fn_name: &str,
) -> bool {
    // Local function: check the AST return type annotation.
    let local_match = typed.module.items.iter().any(|item| {
        if let ynz_ast::nodes::Item::Function(f) = item {
            f.name == fn_name && is_number_errors_return(f)
        } else {
            false
        }
    });
    if local_match {
        return true;
    }
    // Imported function: check the typeck signature's return type.
    imported_fns.get(fn_name).is_some_and(|sig| {
        matches!(
            &sig.ret,
            ynz_typeck::types::Type::ErrorsCapable { inner }
                if matches!(
                    inner.as_ref(),
                    ynz_typeck::types::Type::Number { precision } if *precision <= 34
                )
        )
    })
}

/// Emit auto-propagation for an errors-capable result struct.
///
/// Emits IR that:
/// 1. Extracts the error pointer from `result_struct`.
/// 2. If non-null → pops the current frame, returns the error wrapped in
///    `{error_ptr, 0}` (early-exit path).
/// 3. If null → falls through to a "success" block and returns the success
///    value converted to the given inner type.
///
/// Must only be called when `cg.is_errors_capable = true`.
fn lower_ec_auto_propagate<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    result_struct: inkwell::values::StructValue<'ctx>,
    inner_ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_ty = errors_result_type(cg.ctx);

    let err_ptr_i64 = cg
        .builder
        .build_extract_value(result_struct, 0, "ap_err")
        .map_err(|e| format!("ap extract err: {e}"))?
        .into_int_value();
    let is_err = cg
        .builder
        .build_int_compare(
            IntPredicate::NE,
            err_ptr_i64,
            cg.i64().const_int(0, false),
            "ap_is_err",
        )
        .map_err(|e| format!("ap icmp: {e}"))?;

    let propagate_bb = cg.append_block("ap_propagate");
    let success_bb = cg.append_block("ap_success");
    cg.builder
        .build_conditional_branch(is_err, propagate_bb, success_bb)
        .map_err(|e| format!("ap branch: {e}"))?;

    // Propagation block: pop frame, wrap error, early-return.
    cg.builder.position_at_end(propagate_bb);
    cg.builder
        .build_call(cg.rt.ynz_frame_pop, &[], "ap_pop")
        .map_err(|e| format!("ap pop: {e}"))?;
    let mut prop_result = result_ty.const_zero();
    prop_result = cg
        .builder
        .build_insert_value(prop_result, err_ptr_i64, 0, "prop_err")
        .map_err(|e| format!("prop ins err: {e}"))?
        .into_struct_value();
    prop_result = cg
        .builder
        .build_insert_value(prop_result, cg.i64().const_int(0, false), 1, "prop_val")
        .map_err(|e| format!("prop ins val: {e}"))?
        .into_struct_value();
    cg.builder
        .build_return(Some(&prop_result))
        .map_err(|e| format!("ap early return: {e}"))?;

    // Success block: extract the success bits and convert to the inner type.
    cg.builder.position_at_end(success_bb);
    let success_bits = cg
        .builder
        .build_extract_value(result_struct, 1, "ap_val")
        .map_err(|e| format!("ap extract val: {e}"))?
        .into_int_value();
    cg.i64_bits_to(success_bits, inner_ty)
}

/// Handle the `{i64 error_ptr, i64 success_val}` struct returned from an
/// errors-capable function call.
///
/// Stores the result struct in a stack alloca.
///
/// - **In an errors-capable caller**: also emits an auto-propagation check
///   immediately. If error: pop frame + early-return. Returns a flag in
///   `errors_capable_locals` so the subsequent `Ident` use site knows it needs
///   to load the success value from the struct.
/// - **Outside an errors-capable caller**: just stores the struct; the caller
///   handles it via `.or()` / `.failed()`.
///
/// For `-> number errors` callees the ok-word is a pointer into the callee's decimal128
/// stack alloca. Non-SM (non-suspending) callees' stack allocas become invalid the
/// moment the callee returns; calling the same callee a second time allocates a new
/// frame at the same stack slot, clobbering the first binding's ok-pointer. Copy-on-bind
/// copies the i128 into a per-binding alloca so each binding owns stable storage.
fn lower_errors_capable_call_result<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    mut result_struct: inkwell::values::StructValue<'ctx>,
    callee_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    // Wide-EC copy-on-bind: for `-> number errors` callees the ok-word holds a pointer
    // to an i128 alloca inside the callee's (now-returned) stack frame. Copy the i128
    // into a per-binding alloca so this binding's ok-pointer remains valid regardless
    // of subsequent calls to the same callee. Stack alloca is correct here: non-SM
    // EC bindings are used within the same non-suspending scope.
    if is_number_errors_callee(cg.typed, cg.imported_fns, callee_name) {
        let ok_bits = cg
            .builder
            .build_extract_value(result_struct, 1, "ec_cob_ok")
            .map_err(|e| format!("ec_result cob extract ok {callee_name}: {e}"))?
            .into_int_value();
        let err_bits = cg
            .builder
            .build_extract_value(result_struct, 0, "ec_cob_err")
            .map_err(|e| format!("ec_result cob extract err {callee_name}: {e}"))?
            .into_int_value();
        // Per-binding stable storage allocated unconditionally so f1 always points to
        // valid memory. Error path: f0 != 0 → `.or()` reads f0 first and branches to
        // the fallback; ok_bits (f1) is extracted above but never dereferenced as a
        // pointer on the error path.
        let binding_alloca = cg
            .builder
            .build_alloca(cg.ctx.i128_type(), "ec_cob_dec_own")
            .map_err(|e| format!("ec_result cob alloca {callee_name}: {e}"))?;
        // Guard the staging-slot load: only dereference ok_bits on the success path
        // (err_bits == 0). On the error path ok_bits == 0 — a null deref without this guard.
        let is_ok = cg
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                err_bits,
                cg.ctx.i64_type().const_int(0, false),
                "ec_cob_isok",
            )
            .map_err(|e| format!("ec_result cob isok cmp {callee_name}: {e}"))?;
        let cob_copy_bb = cg.append_block("ec_cob_copy");
        let cob_merge_bb = cg.append_block("ec_cob_merge");
        cg.builder
            .build_conditional_branch(is_ok, cob_copy_bb, cob_merge_bb)
            .map_err(|e| format!("ec_result cob branch {callee_name}: {e}"))?;
        // Success path: copy i128 from the callee's staging slot into the binding alloca.
        cg.builder.position_at_end(cob_copy_bb);
        let dec_ptr = cg
            .builder
            .build_int_to_ptr(
                ok_bits,
                cg.ctx.ptr_type(inkwell::AddressSpace::default()),
                "ec_cob_dec_ptr",
            )
            .map_err(|e| format!("ec_result cob int_to_ptr {callee_name}: {e}"))?;
        let i128_val = cg
            .builder
            .build_load(cg.ctx.i128_type(), dec_ptr, "ec_cob_i128")
            .map_err(|e| format!("ec_result cob load i128 {callee_name}: {e}"))?
            .into_int_value();
        cg.builder
            .build_store(binding_alloca, i128_val)
            .map_err(|e| format!("ec_result cob store {callee_name}: {e}"))?;
        cg.builder
            .build_unconditional_branch(cob_merge_bb)
            .map_err(|e| format!("ec_result cob copy->merge {callee_name}: {e}"))?;
        cg.builder.position_at_end(cob_merge_bb);
        let new_ok_bits = cg
            .builder
            .build_ptr_to_int(binding_alloca, cg.ctx.i64_type(), "ec_cob_newok")
            .map_err(|e| format!("ec_result cob ptr_to_int {callee_name}: {e}"))?;
        let ec_struct_ty = cg
            .ctx
            .struct_type(&[cg.ctx.i64_type().into(), cg.ctx.i64_type().into()], false);
        let mut new_sv = ec_struct_ty.const_zero();
        new_sv = cg
            .builder
            .build_insert_value(new_sv, err_bits, 0, "ec_cob_sv0")
            .map_err(|e| format!("ec_result cob insert err {callee_name}: {e}"))?
            .into_struct_value();
        new_sv = cg
            .builder
            .build_insert_value(new_sv, new_ok_bits, 1, "ec_cob_sv1")
            .map_err(|e| format!("ec_result cob insert ok {callee_name}: {e}"))?
            .into_struct_value();
        result_struct = new_sv;
    }
    let result_ty = errors_result_type(cg.ctx);
    let slot = cg
        .builder
        .build_alloca(result_ty, "ec_result")
        .map_err(|e| format!("ec_result alloca: {e}"))?;
    cg.builder
        .build_store(slot, result_struct)
        .map_err(|e| format!("ec_result store: {e}"))?;
    Ok(slot.into())
}

/// Dispatch `.failed()` and `.or(default)` on an `ErrorsCapable` value.
///
/// The receiver is a pointer to a stack-allocated `{i64 error_ptr, i64 success_val}`.
fn lower_errors_capable_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv_val: BasicValueEnum<'ctx>,
    inner: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_ty = errors_result_type(cg.ctx);
    let recv_ptr = recv_val.into_pointer_value();

    match method {
        "failed" => {
            // .failed() → bool: true when error_ptr != 0.
            let err_gep = cg
                .builder
                .build_struct_gep(result_ty, recv_ptr, 0, "ec_err_gep")
                .map_err(|e| format!("{e}"))?;
            let err_ptr = cg
                .builder
                .build_load(cg.i64(), err_gep, "ec_err_ptr")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let is_failed = cg
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    err_ptr,
                    cg.i64().const_int(0, false),
                    "ec_failed",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(is_failed.into())
        }
        "or" => {
            // .or(default) → inner_type: error_ptr == 0 ? success_val : default.
            let err_gep = cg
                .builder
                .build_struct_gep(result_ty, recv_ptr, 0, "ec_or_err_gep")
                .map_err(|e| format!("{e}"))?;
            let err_ptr = cg
                .builder
                .build_load(cg.i64(), err_gep, "ec_or_err")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let is_ok = cg
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    err_ptr,
                    cg.i64().const_int(0, false),
                    "ec_is_ok",
                )
                .map_err(|e| format!("{e}"))?;

            let val_gep = cg
                .builder
                .build_struct_gep(result_ty, recv_ptr, 1, "ec_or_val_gep")
                .map_err(|e| format!("{e}"))?;
            let bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "ec_or_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let success_val = cg.i64_bits_to(bits, inner)?;
            let default_val = lower_expr(cg, &args[0])?;

            cg.builder
                .build_select(is_ok, success_val, default_val, "ec_or_res")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!(
            "errors-capable method `{other}` not yet implemented in M7 P4a"
        )),
    }
}

// ── Helper: test if a map key type is String ──────────────────────────────────

fn key_is_string(key_ty: &Type) -> bool {
    matches!(key_ty, Type::String)
}

// ── Map method dispatch (M5 P4b) ──────────────────────────────────────────────

fn lower_map_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    map: PointerValue<'ctx>,
    key_ty: &Type,
    val_ty: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    let is_str = key_is_string(key_ty);
    match method {
        "count" => {
            let n = cg
                .builder
                .build_call(cg.rt.ynz_map_count, &[map.into()], "mc")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("map_count void")?;
            Ok(n)
        }
        "has" => {
            let key_val = lower_expr(cg, &args[0])?;
            let n = if is_str {
                let pair_ty = cg
                    .ctx
                    .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                let out = cg
                    .builder
                    .build_alloca(pair_ty, "has_out")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_get_str,
                        &[map.into(), key_val.into(), out.into()],
                        "hg",
                    )
                    .map_err(|e| format!("{e}"))?;
                let has_gep = cg
                    .builder
                    .build_struct_gep(pair_ty, out, 0, "has0")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_load(cg.i64(), has_gep, "has_v")
                    .map_err(|e| format!("{e}"))?
            } else {
                let kt = cg.expr_type(&args[0]);
                let key_bits = cg.to_i64_bits(key_val, &kt)?;
                cg.builder
                    .build_call(cg.rt.ynz_map_has, &[map.into(), key_bits.into()], "mhas")
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("map_has void")?
            };
            let b = cg
                .builder
                .build_int_truncate(n.into_int_value(), cg.bool(), "has_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "get" => {
            let key_val = lower_expr(cg, &args[0])?;
            let pair_ty = cg
                .ctx
                .struct_type(&[cg.i64().into(), cg.i64().into()], false);
            let out = cg
                .builder
                .build_alloca(pair_ty, "mget_out")
                .map_err(|e| format!("{e}"))?;
            if is_str {
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_get_str,
                        &[map.into(), key_val.into(), out.into()],
                        "mg_s",
                    )
                    .map_err(|e| format!("{e}"))?;
            } else {
                let kt = cg.expr_type(&args[0]);
                let key_bits = cg.to_i64_bits(key_val, &kt)?;
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_get,
                        &[map.into(), key_bits.into(), out.into()],
                        "mg",
                    )
                    .map_err(|e| format!("{e}"))?;
            }
            // Copy pair {i64 has, i64 bits} into a maybe<V> slot — identical layout.
            let maybe_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "mg_maybe")
                .map_err(|e| format!("{e}"))?;
            let has_src = cg
                .builder
                .build_struct_gep(pair_ty, out, 0, "hs")
                .map_err(|e| format!("{e}"))?;
            let val_src = cg
                .builder
                .build_struct_gep(pair_ty, out, 1, "vs")
                .map_err(|e| format!("{e}"))?;
            let has_v = cg
                .builder
                .build_load(cg.i64(), has_src, "hv")
                .map_err(|e| format!("{e}"))?;
            let val_v = cg
                .builder
                .build_load(cg.i64(), val_src, "vv")
                .map_err(|e| format!("{e}"))?;
            let has_dst = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "hd")
                .map_err(|e| format!("{e}"))?;
            let val_dst = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "vd")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(has_dst, has_v)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val_dst, val_v)
                .map_err(|e| format!("{e}"))?;
            let _ = val_ty;
            Ok(maybe_slot.into())
        }
        "set" => {
            let key_val = lower_expr(cg, &args[0])?;
            let val_val = lower_expr(cg, &args[1])?;
            let vt = cg.expr_type(&args[1]);
            let val_bits = cg.to_i64_bits(val_val, &vt)?;
            if is_str {
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_set_str,
                        &[map.into(), key_val.into(), val_bits.into()],
                        "ms_s",
                    )
                    .map_err(|e| format!("{e}"))?;
            } else {
                let kt = cg.expr_type(&args[0]);
                let key_bits = cg.to_i64_bits(key_val, &kt)?;
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_set,
                        &[map.into(), key_bits.into(), val_bits.into()],
                        "ms",
                    )
                    .map_err(|e| format!("{e}"))?;
            }
            Ok(cg.i64().const_zero().into())
        }
        other => Err(format!("map method `{other}` not yet lowered in P4b")),
    }
}

// ── M6: options + fallible-conversion codegen helpers ────────────────────────

/// Lower `options_value.toString()` → call `ynz_string_from_static` with the variant name.
///
/// The options_table maps variant tags to names; we use a runtime switch to call
/// the right string. For simplicity, we build an LLVM switch with one case per variant.
fn lower_options_to_string<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    opts_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let entry = cg
        .options_table
        .options
        .get(opts_name)
        .ok_or_else(|| format!("codegen: unknown options type `{opts_name}`"))?
        .clone();

    // Allocate a pointer slot to hold the result string pointer.
    let result_slot = cg
        .builder
        .build_alloca(cg.ptr(), "opts_str_slot")
        .map_err(|e| format!("{e}"))?;
    // Default: empty string
    let empty_g = build_string_global(cg.ctx, cg.module, "", ".opts.str.empty");
    cg.builder
        .build_store(result_slot, empty_g.as_pointer_value())
        .map_err(|e| format!("{e}"))?;

    let merge_bb = cg.append_block("opts_str_merge");
    let tag_val = recv.into_int_value();

    // For each variant, emit a conditional branch: if tag == i, store variant-name string.
    let mut current_bb = cg
        .builder
        .get_insert_block()
        .ok_or("opts_to_string: no insert block")?;

    for (i, variant) in entry.variants.iter().enumerate() {
        // Use the display string if provided, otherwise fall back to the variant name.
        let display = entry
            .display_strings
            .get(i)
            .and_then(|d| d.as_deref())
            .unwrap_or(variant.as_str());
        let tag_const = cg.ctx.i8_type().const_int(i as u64, false);
        let is_this_variant = cg
            .builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag_val, tag_const, "ots_cmp")
            .map_err(|e| format!("{e}"))?;

        let arm_bb = cg.append_block(&format!("opts_str_{i}"));
        let next_bb = if i + 1 < entry.variants.len() {
            cg.append_block(&format!("opts_str_check{}", i + 1))
        } else {
            merge_bb
        };

        cg.builder
            .build_conditional_branch(is_this_variant, arm_bb, next_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(arm_bb);
        let g = build_string_global(
            cg.ctx,
            cg.module,
            display,
            &format!(".opts.{opts_name}.{variant}"),
        );
        let len = cg.i64().const_int(display.len() as u64, false);
        let ptr = cg
            .builder
            .build_call(
                cg.rt.ynz_string_from_static,
                &[g.as_pointer_value().into(), len.into()],
                "opt_str",
            )
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("ynz_string_from_static returned void")?;
        cg.builder
            .build_store(result_slot, ptr)
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| format!("{e}"))?;

        // Only position at next_bb if it's different from merge_bb.
        // If next_bb IS merge_bb (last iteration), don't reposition — the builder
        // is already in a terminated arm body and will fall into the next part.
        if next_bb != merge_bb {
            cg.builder.position_at_end(next_bb);
        }
        current_bb = next_bb;
    }
    let _ = current_bb;

    // Ensure the current block (the last check/default block, or merge_bb itself)
    // branches to merge_bb if not already terminated.
    if !is_block_terminated(cg) {
        cg.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| format!("{e}"))?;
    }
    cg.builder.position_at_end(merge_bb);

    let result = cg
        .builder
        .build_load(cg.ptr(), result_slot, "opts_str")
        .map_err(|e| format!("{e}"))?;
    Ok(result)
}

/// Lower `(float).toInt()` → `maybe<int>` with NaN + range checks.
/// Locked codegen sequence per design/narrowing.md P4 step 9.
fn lower_float_to_int<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    x: inkwell::values::FloatValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_slot = cg
        .builder
        .build_alloca(cg.maybe_type(), "f2i_slot")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(result_slot, cg.maybe_type().const_zero())
        .map_err(|e| format!("{e}"))?;

    let nan_bb = cg.append_block("f2i_nan");
    let range_bb = cg.append_block("f2i_range");
    let low_bb = cg.append_block("f2i_low");
    let ok_bb = cg.append_block("f2i_ok");
    let ret_bb = cg.append_block("f2i_ret");

    // NaN check: `x != x` (unordered comparison)
    let is_nan = cg
        .builder
        .build_float_compare(inkwell::FloatPredicate::UNO, x, x, "is_nan")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(is_nan, nan_bb, range_bb)
        .map_err(|e| format!("{e}"))?;

    // NaN → return none (slot is already zeroed)
    cg.builder.position_at_end(nan_bb);
    cg.builder
        .build_unconditional_branch(ret_bb)
        .map_err(|e| format!("{e}"))?;

    // Upper range check: x >= 2^63 (9.223372036854776e18)
    cg.builder.position_at_end(range_bb);
    let i64_max_f64 = cg.f64().const_float(9.223372036854776e18_f64);
    let too_big = cg
        .builder
        .build_float_compare(inkwell::FloatPredicate::OGE, x, i64_max_f64, "too_big")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(too_big, nan_bb, low_bb)
        .map_err(|e| format!("{e}"))?;

    // Lower range check: x < -2^63
    cg.builder.position_at_end(low_bb);
    let i64_min_f64 = cg.f64().const_float(-9.223372036854776e18_f64);
    let too_small = cg
        .builder
        .build_float_compare(inkwell::FloatPredicate::OLT, x, i64_min_f64, "too_small")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(too_small, nan_bb, ok_bb)
        .map_err(|e| format!("{e}"))?;

    // Convert: fptosi (safe — in-range proven)
    cg.builder.position_at_end(ok_bb);
    let int_val = cg
        .builder
        .build_float_to_signed_int(x, cg.i64(), "fptosi")
        .map_err(|e| format!("{e}"))?;
    let has_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), result_slot, 0, "has_gep")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(has_gep, cg.i64().const_int(1, false))
        .map_err(|e| format!("{e}"))?;
    let val_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), result_slot, 1, "val_gep")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(val_gep, int_val)
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_unconditional_branch(ret_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ret_bb);
    Ok(result_slot.into())
}

/// Lower `(number).toInt()` → `maybe<int>`.
fn lower_number_to_int<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    num_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    // Use ynz_decimal_to_float then float_to_int.
    let f64t = cg.f64();
    let prt = cg.ptr();
    let fn_ty = f64t.fn_type(&[prt.into()], false);
    let f = cg
        .module
        .get_function("ynz_decimal_to_float")
        .unwrap_or_else(|| cg.module.add_function("ynz_decimal_to_float", fn_ty, None));
    let f_val = cg
        .builder
        .build_call(f, &[num_ptr.into()], "d2f")
        .map_err(|e| format!("{e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("decimal_to_float void")?
        .into_float_value();
    lower_float_to_int(cg, f_val)
}

/// Lower `(string).toInt()` → `maybe<int>` via `ynz_string_to_int` runtime.
fn lower_string_to_int<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    lower_string_to_maybe_primitive(cg, str_ptr, "ynz_string_to_int", false)
}

/// Lower `(string).toFloat()` → `maybe<float>` via `ynz_string_to_float` runtime.
fn lower_string_to_float<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    lower_string_to_maybe_primitive(cg, str_ptr, "ynz_string_to_float", false)
}

/// Lower `(string).toNumber()` → `maybe<number>` via `ynz_string_to_number` runtime.
fn lower_string_to_number<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    lower_string_to_maybe_primitive(cg, str_ptr, "ynz_string_to_number", true)
}

/// Shared implementation: call a `(ptr, len, out) -> void` runtime function;
/// the `out` buffer is `[i64; 2]` (has_value + value) or `[i64; 3]` for number.
/// Returns a pointer to a `maybe<T>` struct.
fn lower_string_to_maybe_primitive<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
    rt_fn_name: &str,
    is_number: bool,
) -> Result<BasicValueEnum<'ctx>, String> {
    // Allocate output buffer: [3 x i64] (enough for number, fine for int/float).
    let arr_ty = cg.i64().array_type(3);
    let out = cg
        .builder
        .build_alloca(arr_ty, "str_conv_out")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(out, arr_ty.const_zero())
        .map_err(|e| format!("{e}"))?;

    // Get string pointer and length from the Yinz string (null-terminated).
    // The runtime functions take (ptr, len) where len is the byte count.
    // For null-terminated strings, compute the length with strlen.
    let strlen_ty = cg.i64().fn_type(&[cg.ptr().into()], false);
    let strlen = cg
        .module
        .get_function("strlen")
        .unwrap_or_else(|| cg.module.add_function("strlen", strlen_ty, None));
    let len_val = cg
        .builder
        .build_call(strlen, &[str_ptr.into()], "str_len")
        .map_err(|e| format!("{e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("strlen void")?;

    // Look up or declare the runtime function.
    let fn_val = match rt_fn_name {
        "ynz_string_to_int" => cg.rt.ynz_string_to_int,
        "ynz_string_to_float" => cg.rt.ynz_string_to_float,
        "ynz_string_to_number" => cg.rt.ynz_string_to_number,
        _ => return Err(format!("unknown str conv fn: {rt_fn_name}")),
    };

    cg.builder
        .build_call(
            fn_val,
            &[str_ptr.into(), len_val.into(), out.into()],
            "str_conv",
        )
        .map_err(|e| format!("{e}"))?;

    // Build a `maybe<T>` struct from the output buffer.
    let slot = cg
        .builder
        .build_alloca(cg.maybe_type(), "str_conv_maybe")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(slot, cg.maybe_type().const_zero())
        .map_err(|e| format!("{e}"))?;

    let has_gep = unsafe {
        cg.builder
            .build_gep(cg.i64(), out, &[cg.i64().const_int(0, false)], "has_v")
    }
    .map_err(|e| format!("{e}"))?;
    let has_val = cg
        .builder
        .build_load(cg.i64(), has_gep, "has_val")
        .map_err(|e| format!("{e}"))?;

    // Store has_value
    let has_out_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), slot, 0, "has_out")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(has_out_gep, has_val)
        .map_err(|e| format!("{e}"))?;

    // Store value bits (index 1 for int/float, or for number we store the pointer-as-bits)
    let val_gep_src = unsafe {
        cg.builder
            .build_gep(cg.i64(), out, &[cg.i64().const_int(1, false)], "val_v")
    }
    .map_err(|e| format!("{e}"))?;

    let val_out_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), slot, 1, "val_out")
        .map_err(|e| format!("{e}"))?;

    if is_number {
        // For number, store the pointer to the out buffer's number data as bits.
        let ptr_bits = cg
            .builder
            .build_ptr_to_int(val_gep_src, cg.i64(), "ptr2i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(val_out_gep, ptr_bits)
            .map_err(|e| format!("{e}"))?;
    } else {
        let val = cg
            .builder
            .build_load(cg.i64(), val_gep_src, "val")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(val_out_gep, val)
            .map_err(|e| format!("{e}"))?;
    }

    Ok(slot.into())
}

// ── String method dispatch (M7 P4b) ──────────────────────────────────────────

fn lower_string_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    s_ptr: inkwell::values::PointerValue<'ctx>,
    method: &str,
    args: &[ynz_ast::nodes::Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        // Zero-arg methods returning a string.
        "toUpperCase" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_to_upper, &[s_ptr.into()], "s_upper")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_to_upper void")?)
        }
        "toLowerCase" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_to_lower, &[s_ptr.into()], "s_lower")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_to_lower void")?)
        }
        "trim" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_trim, &[s_ptr.into()], "s_trim")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_trim void")?)
        }
        // Zero-arg methods returning int.
        "count" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_count, &[s_ptr.into()], "s_count")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_count void")?)
        }
        "byteCount" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_byte_count, &[s_ptr.into()], "s_bcount")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_byte_count void")?)
        }
        "graphemeCount" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_grapheme_count, &[s_ptr.into()], "s_gcount")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_grapheme_count void")?)
        }
        // One-arg predicate methods returning bool.
        "contains" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_contains,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_contains",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_contains void")?
                .into_int_value();
            // Convert i32 → i1 (bool).
            let b = cg
                .builder
                .build_int_truncate(r, cg.bool(), "s_contains_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "startsWith" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_starts_with,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_sw",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_starts_with void")?
                .into_int_value();
            let b = cg
                .builder
                .build_int_truncate(r, cg.bool(), "s_sw_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "endsWith" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_ends_with,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_ew",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_ends_with void")?
                .into_int_value();
            let b = cg
                .builder
                .build_int_truncate(r, cg.bool(), "s_ew_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        // One-arg indexed access returning maybe<string>.
        "get" | "graphemeAt" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let raw_ptr = if method == "graphemeAt" {
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_grapheme_at,
                        &[s_ptr.into(), idx.into()],
                        "s_gat",
                    )
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_grapheme_at void")?
                    .into_pointer_value()
            } else {
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_codepoint_at,
                        &[s_ptr.into(), idx.into()],
                        "s_cpat",
                    )
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_codepoint_at void")?
                    .into_pointer_value()
            };
            // Null pointer → none (tag=0); non-null → some (tag=1, value=ptr as i64).
            let result_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "s_get_res")
                .map_err(|e| format!("{e}"))?;
            let is_null = cg
                .builder
                .build_is_null(raw_ptr, "is_null")
                .map_err(|e| format!("{e}"))?;
            let some_bb = cg.append_block("s_get_some");
            let none_bb = cg.append_block("s_get_none");
            let done_bb = cg.append_block("s_get_done");
            cg.builder
                .build_conditional_branch(is_null, none_bb, some_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(none_bb);
            let tag_ptr = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "s_tag_n")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tag_ptr, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(some_bb);
            let tag_ptr2 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "s_tag_s")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tag_ptr2, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let val_ptr = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 1, "s_val")
                .map_err(|e| format!("{e}"))?;
            let ptr_as_i64 = cg
                .builder
                .build_ptr_to_int(raw_ptr, cg.i64(), "ptr_i64")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val_ptr, ptr_as_i64)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(done_bb);
            Ok(result_slot.into())
        }
        // One-arg: byteAt returns maybe<int>.
        "byteAt" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let raw = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_byte_at,
                    &[s_ptr.into(), idx.into()],
                    "s_bat",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_byte_at void")?
                .into_int_value();
            // -1 means OOB → none; else → some(value).
            let result_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "s_bat_res")
                .map_err(|e| format!("{e}"))?;
            let minus_one = cg.i64().const_int(u64::MAX, true); // -1 as i64
            let is_oob = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, raw, minus_one, "bat_oob")
                .map_err(|e| format!("{e}"))?;
            let some_bb = cg.append_block("bat_some");
            let none_bb = cg.append_block("bat_none");
            let done_bb = cg.append_block("bat_done");
            cg.builder
                .build_conditional_branch(is_oob, none_bb, some_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(none_bb);
            let tp_n = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "bat_tn")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_n, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(some_bb);
            let tp_s = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "bat_ts")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_s, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let vp = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 1, "bat_v")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(vp, raw)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(done_bb);
            Ok(result_slot.into())
        }
        // One-arg: indexOf returns maybe<int>.
        "indexOf" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let raw = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_index_of,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_iof",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_index_of void")?
                .into_int_value();
            let result_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "s_iof_res")
                .map_err(|e| format!("{e}"))?;
            let minus_one = cg.i64().const_int(u64::MAX, true);
            let is_missing = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, raw, minus_one, "iof_miss")
                .map_err(|e| format!("{e}"))?;
            let some_bb = cg.append_block("iof_some");
            let none_bb = cg.append_block("iof_none");
            let done_bb = cg.append_block("iof_done");
            cg.builder
                .build_conditional_branch(is_missing, none_bb, some_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(none_bb);
            let tp_n = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "iof_tn")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_n, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(some_bb);
            let tp_s = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "iof_ts")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_s, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let vp = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 1, "iof_v")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(vp, raw)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(done_bb);
            Ok(result_slot.into())
        }
        // Two-arg: substring(start: int, end: int) → string.
        "substring" => {
            let start = lower_expr(cg, &args[0])?.into_int_value();
            let end = lower_expr(cg, &args[1])?.into_int_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_substring,
                    &[s_ptr.into(), start.into(), end.into()],
                    "s_sub",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_substring void")?)
        }
        // One-arg: split(sep: string) → array<string>.
        "split" => {
            let sep_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_split,
                    &[s_ptr.into(), sep_ptr.into()],
                    "s_split",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_split void")?)
        }
        // Two-arg: replace(from: string, to: string) → string.
        "replace" => {
            let from_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let to_ptr = lower_expr(cg, &args[1])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_replace,
                    &[s_ptr.into(), from_ptr.into(), to_ptr.into()],
                    "s_rep",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_replace void")?)
        }
        // toString() on string is identity.
        "toString" => Ok(s_ptr.into()),
        // Fallible conversions — forwarded to lower_method_call.
        "toInt" => lower_string_to_int(cg, s_ptr),
        "toFloat" => lower_string_to_float(cg, s_ptr),
        "toNumber" => lower_string_to_number(cg, s_ptr),
        other => Err(format!("codegen: unknown string method `{other}`")),
    }
}

fn lower_method_call<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    recv_ty: &Type,
    method: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    match (recv_ty, method) {
        (Type::Int, "toNumber") => {
            let out = cg
                .builder
                .build_alloca(cg.i128(), "i2n")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(cg.rt.decimal_from_int, &[recv.into(), out.into()], "")
                .map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }
        (Type::Int, "toFloat") => cg
            .builder
            .build_signed_int_to_float(recv.into_int_value(), cg.f64(), "i2f")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Type::Int, "toString") => {
            to_c_string(cg, recv, &Type::Int).map(|p: PointerValue<'ctx>| p.into())
        }
        (Type::Float, "toNumber") => {
            let out = cg
                .builder
                .build_alloca(cg.i128(), "f2n")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(cg.rt.decimal_from_float, &[recv.into(), out.into()], "")
                .map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }
        (Type::Float, "toString") => to_c_string(cg, recv, &Type::Float).map(|p| p.into()),
        (Type::Number { .. }, "toFloat") => {
            let f64t = cg.f64();
            let prt = cg.ptr();
            let fn_ty = f64t.fn_type(&[prt.into()], false);
            let f = cg
                .module
                .get_function("ynz_decimal_to_float")
                .unwrap_or_else(|| cg.module.add_function("ynz_decimal_to_float", fn_ty, None));
            let r = cg
                .builder
                .build_call(f, &[recv.into()], "d2f")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("decimal_to_float void")?)
        }
        (Type::Number { .. }, "toString") => {
            to_c_string(cg, recv, &Type::Number { precision: 34 }).map(|p| p.into())
        }
        (Type::Bool, "toString") => to_c_string(cg, recv, &Type::Bool).map(|p| p.into()),

        // M6: fallible conversions — return maybe<T>
        (Type::Int, "toInt") => {
            // (int).toInt() is identity — return the int value directly (infallible).
            Ok(recv)
        }
        (Type::Float, "toInt") => lower_float_to_int(cg, recv.into_float_value()),
        (Type::Number { .. }, "toInt") => lower_number_to_int(cg, recv.into_pointer_value()),
        (Type::String, "toInt") => lower_string_to_int(cg, recv.into_pointer_value()),
        (Type::String, "toFloat") => lower_string_to_float(cg, recv.into_pointer_value()),
        (Type::String, "toNumber") => lower_string_to_number(cg, recv.into_pointer_value()),

        _ => Err(format!(
            "codegen: unknown method `{method}` on {:?}",
            recv_ty
        )),
    }
}

fn store<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
    slot: PointerValue<'ctx>,
) -> Result<(), String> {
    match ty {
        Type::Number { precision } if *precision <= 34 => {
            // Hardware decimal128: val is a ptr to i128; load the bits then store into slot.
            let bits = cg
                .builder
                .build_load(cg.i128(), val.into_pointer_value(), "dec_bits")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(slot, bits)
                .map_err(|e| format!("{e}"))?;
        }
        // M8 P6: bignum (N > 34) — val is a ptr to the string; store the pointer.
        Type::Number { .. } => {
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
        }
        // BuiltinArray is a pointer to the heap YnzArray; store the pointer.
        // BuiltinFixed is already stored as an alloca pointer; store the pointer.
        // Maybe is an alloca pointer to {i64,i64}; store the pointer.
        // ErrorsCapable is an alloca pointer to {i64,i64}; store the pointer.
        // Range is a pointer to the {i64,i64} range alloca; store the pointer.
        Type::BuiltinArray { .. }
        | Type::BuiltinFixed { .. }
        | Type::Maybe { .. }
        | Type::BuiltinMap { .. }
        | Type::MapEntry { .. }
        | Type::ErrorsCapable { .. }
        | Type::Range { .. } => {
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
        }
        _ => {
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

/// Store a value into a STRUCT FIELD pointer.
///
/// Shape fields use `llvm_field_type` layout (number = i128 inline; everything else
/// by value or by pointer). This differs from the variable-slot layout where number
/// is stored as i128 in the slot but "value" is a ptr-to-i128.
fn store_field<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
    field_ptr: PointerValue<'ctx>,
) -> Result<(), String> {
    match ty {
        Type::Number { precision } if *precision <= 34 => {
            // N ≤ 34: field stores i128 bits inline; val is ptr-to-i128.
            let bits = cg
                .builder
                .build_load(cg.i128(), val.into_pointer_value(), "dec_field_bits")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(field_ptr, bits)
                .map_err(|e| format!("{e}"))?;
        }
        Type::Number { .. } => {
            // N > 34: field stores a pointer to the decimal string; val is already a ptr.
            cg.builder
                .build_store(field_ptr, val)
                .map_err(|e| format!("{e}"))?;
        }
        _ => {
            cg.builder
                .build_store(field_ptr, val)
                .map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

fn load<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    slot: PointerValue<'ctx>,
    ty: &Type,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    match ty {
        Type::Number { precision } if *precision <= 34 => {
            // Hardware decimal128: load i128 bits from slot, copy into fresh alloca, return ptr.
            let bits = cg
                .builder
                .build_load(cg.i128(), slot, "dec_ld")
                .map_err(|e| format!("{e}"))?;
            let tmp = cg
                .builder
                .build_alloca(cg.i128(), name)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tmp, bits)
                .map_err(|e| format!("{e}"))?;
            Ok(tmp.into())
        }
        // M8 P6: bignum (N > 34) — slot stores a pointer to the decimal string.
        Type::Number { .. } => cg
            .builder
            .build_load(cg.ptr(), slot, name)
            .map_err(|e| format!("{e}")),
        // Shapes and dynamic values: the slot holds a pointer to the struct data.
        // Load that pointer and return it — the caller gets a ptr to the struct.
        Type::Shape { .. } | Type::Dynamic { .. } => {
            let lt = cg.ptr();
            cg.builder
                .build_load(lt, slot, name)
                .map_err(|e| format!("{e}"))
        }
        // BuiltinArray: slot stores the *mut YnzArray pointer — load and return it.
        // BuiltinFixed: slot stores a pointer to the [N x i64] alloca — load and return.
        // Maybe: slot stores a pointer to the {i64,i64} alloca — load and return.
        // Union: slot stores a pointer to the tagged-struct alloca — load and return.
        // ErrorsCapable: slot stores a pointer to the {i64,i64} result alloca — load and return.
        // Range: slot stores a pointer to the {i64 start, i64 end} range alloca — load and return.
        Type::BuiltinArray { .. }
        | Type::BuiltinFixed { .. }
        | Type::Maybe { .. }
        | Type::BuiltinMap { .. }
        | Type::MapEntry { .. }
        | Type::Union { .. }
        | Type::ErrorsCapable { .. }
        | Type::Range { .. } => cg
            .builder
            .build_load(cg.ptr(), slot, name)
            .map_err(|e| format!("{e}")),
        ty => {
            let lt = cg
                .llvm_type_for(ty)
                .ok_or_else(|| format!("load: unknown type {:?}", ty))?;
            cg.builder
                .build_load(lt, slot, name)
                .map_err(|e| format!("{e}"))
        }
    }
}

fn build_string_global<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    s: &str,
    name: &str,
) -> GlobalValue<'ctx> {
    let i8t = ctx.i8_type();
    let mut bytes: Vec<u8> = s.bytes().collect();
    bytes.push(0);
    let arr_ty = i8t.array_type(bytes.len() as u32);
    let arr = i8t.const_array(
        &bytes
            .iter()
            .map(|&b| i8t.const_int(b as u64, false))
            .collect::<Vec<_>>(),
    );
    let g = module.add_global(arr_ty, Some(AddressSpace::default()), name);
    g.set_initializer(&arr);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Private);
    g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
    g
}

fn build_decimal_global<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    bits: u128,
    name: &str,
) -> GlobalValue<'ctx> {
    let i128t = ctx.i128_type();
    let val = i128t.const_int_arbitrary_precision(&[
        (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
        (bits >> 64) as u64,
    ]);
    let g = module.add_global(i128t, Some(AddressSpace::default()), name);
    g.set_initializer(&val);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Private);
    g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
    g
}

/// Attempt to build a module-level global for a shape struct literal whose fields are
/// all compile-time integer/boolean literals.
///
/// Returns `Some(global_ptr)` when all fields of the struct literal are int or bool
/// literals that can be folded into LLVM constant values. Returns `None` when any field
/// is a non-literal expression (runtime value) — the caller falls back to the stack-alloca path.
///
/// Module-level globals have static lifetime and survive across suspension boundaries.
/// Used for `array<Shape>` literals in SM functions so the element pointers stored in
/// the array remain valid between resume calls — stack allocas from one resume call are
/// freed when that call returns, making those pointers dangle on the next resume.
fn try_build_shape_global<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    struct_ty: inkwell::types::StructType<'ctx>,
    fields_lit: &[ynz_ast::nodes::StructLitField],
    shape_def_fields: &[ynz_typeck::shapes::FieldDef],
    global_name: &str,
) -> Option<inkwell::values::GlobalValue<'ctx>> {
    let i64t = ctx.i64_type();
    // Build one i64 constant per shape field in layout order.
    // Only int and bool literals produce folded constants; other types fall through to None.
    let mut field_vals: Vec<inkwell::values::IntValue<'ctx>> =
        Vec::with_capacity(shape_def_fields.len());
    for def_field in shape_def_fields {
        let lit_field = fields_lit.iter().find(|f| f.name == def_field.name)?;
        let const_val = match &lit_field.value {
            ynz_ast::nodes::Expr::IntLit(n, _) => {
                i64t.const_int(*n as u64, true) // bit-reinterpret signed int literal as u64
            }
            ynz_ast::nodes::Expr::BoolLit(b, _) => i64t.const_int(u64::from(*b), false),
            _ => return None, // non-literal field — cannot fold to a global
        };
        field_vals.push(const_val);
    }
    let init_vals: Vec<inkwell::values::BasicValueEnum<'ctx>> =
        field_vals.iter().map(|v| (*v).into()).collect();
    let init = struct_ty.const_named_struct(&init_vals);
    let g = module.add_global(
        struct_ty,
        Some(inkwell::AddressSpace::default()),
        global_name,
    );
    g.set_initializer(&init);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Private);
    g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
    Some(g)
}

#[cfg(test)]
mod tests {
    use super::{
        build_cpu_group_slots, cpu_slot_reserve_slots, function_contains_wait,
        return_type_fits_cpu_result_abi, CpuGroupSlot, FrameLayout,
    };
    use ynz_ast::nodes::{Block, Expr, Stmt};
    use ynz_diagnostics::SourceSpan;
    use ynz_typeck::independence::cpu_result_abi_supports;

    fn dummy_span() -> SourceSpan {
        SourceSpan::new("test.ynz", 0, 1)
    }

    /// A bare FrameLayout carrying only the CPU slots under test. The other fields are
    /// irrelevant to `cpu_slot_reserve_slots`, which reads `cpu_group_slots` exclusively.
    fn layout_with_cpu_slots(cpu_group_slots: Vec<CpuGroupSlot>) -> FrameLayout {
        FrameLayout {
            total_size: 0,
            n_locals: 0,
            children: Vec::new(),
            recursion_slot: None,
            number_errors_staging_offset: None,
            cpu_group_slots,
        }
    }

    /// One return-type variant's classification for the parity test.
    ///
    /// A `Reachable` case carries a representative AST form, the EXACT resolved form the
    /// resolver emits for it, and the expected shared verdict in BOTH the bare and the
    /// `errors`-wrapped (EC-inner) positions. An `UnreachableAsReturnInner` case names a
    /// resolved variant that no function-return annotation can ever produce, with the WHY —
    /// it is counted by the exhaustive classifier (so the compiler still forces it to be
    /// addressed) but has no gate run.
    enum ParityCase {
        Reachable {
            label: &'static str,
            ast: ynz_ast::nodes::Type,
            resolved: ynz_typeck::types::Type,
            /// Expected gate verdict for the bare `-> T` form.
            bare_expected: bool,
            /// Expected gate verdict for the `-> T errors` (EC-inner) form.
            ec_expected: bool,
        },
        UnreachableAsReturnInner(&'static str),
    }

    /// Classify ONE resolved `Type` variant for the CPU-result-ABI parity invariant.
    ///
    /// This `match` has NO `_` arm BY DESIGN: it is what makes `cpu_result_abi_gate_parity`
    /// compile-forced exhaustive. Adding a variant to `ynz_typeck::types::Type` makes this
    /// function fail to compile (`non-exhaustive patterns: ... not covered`) until someone
    /// classifies the new variant here — so no return class can ever be silently left
    /// unpinned by the parity test again. Each `Reachable` row pairs the AST form with the
    /// EXACT resolved form the typeck resolver emits (verified against `check.rs`'s
    /// `ast_type_to_type` + `signatures.rs`), so the rows drive the live production paths.
    fn parity_case(variant: &ynz_typeck::types::Type) -> ParityCase {
        use ynz_ast::nodes::Type as Ast;
        use ynz_typeck::types::Type as Resolved;
        let span = dummy_span();
        match variant {
            // ── Admitted classes: value or single owning heap pointer fits the 16-byte slot ──
            Resolved::Int => ParityCase::Reachable {
                label: "int",
                ast: Ast::Int,
                resolved: Resolved::Int,
                bare_expected: true,
                ec_expected: true,
            },
            Resolved::Float => ParityCase::Reachable {
                label: "float",
                ast: Ast::Float,
                resolved: Resolved::Float,
                bare_expected: true,
                ec_expected: true,
            },
            Resolved::Bool => ParityCase::Reachable {
                label: "bool",
                ast: Ast::Bool,
                resolved: Resolved::Bool,
                bare_expected: true,
                ec_expected: true,
            },
            // `string` is the only `Named` that fits at the AST level; resolves to `String`.
            Resolved::String => ParityCase::Reachable {
                label: "string",
                ast: Ast::Named("string".to_string(), span.clone()),
                resolved: Resolved::String,
                bare_expected: true,
                ec_expected: true,
            },
            // bare `number` admits (heap-stable ABI ptr); `number errors` declines — the i128
            // ok-word points into the worker thread's dead staging slot (wide-EC UAF).
            Resolved::Number { .. } => ParityCase::Reachable {
                label: "number",
                ast: Ast::Number { precision: 34 },
                resolved: Resolved::Number { precision: 34 },
                bare_expected: true,
                ec_expected: false,
            },
            // `array<_>`/`map<_,_>` resolve to `BuiltinArray`/`BuiltinMap` (never `Generic`);
            // both admit bare and EC (the ok-word is the collection's owning heap pointer).
            Resolved::BuiltinArray { .. } => ParityCase::Reachable {
                label: "array<int>",
                ast: Ast::Generic {
                    name: "array".to_string(),
                    name_span: span.clone(),
                    args: vec![Ast::Int],
                    span: span.clone(),
                },
                resolved: Resolved::BuiltinArray {
                    elem: Box::new(Resolved::Int),
                },
                bare_expected: true,
                ec_expected: true,
            },
            Resolved::BuiltinMap { .. } => ParityCase::Reachable {
                label: "map<int,int>",
                ast: Ast::Generic {
                    name: "map".to_string(),
                    name_span: span.clone(),
                    args: vec![Ast::Int, Ast::Int],
                    span: span.clone(),
                },
                resolved: Resolved::BuiltinMap {
                    key: Box::new(Resolved::Int),
                    val: Box::new(Resolved::Int),
                },
                bare_expected: true,
                ec_expected: true,
            },
            // ── Declined classes: sequential lowering is always correct, never an error ──
            // `fixed` lowers to `BuiltinFixed`; its non-suspending return path is a pre-existing
            // base bug, so it declines in both positions (admitting it would be dead anyway).
            Resolved::BuiltinFixed { .. } => ParityCase::Reachable {
                label: "fixed<int>",
                ast: Ast::Generic {
                    name: "fixed".to_string(),
                    name_span: span.clone(),
                    args: vec![Ast::Int],
                    span: span.clone(),
                },
                resolved: Resolved::BuiltinFixed {
                    elem: Box::new(Resolved::Int),
                    size: None,
                },
                bare_expected: false,
                ec_expected: false,
            },
            // A by-value shape needs variable-size frame staging (WideValueSuspendingReturn).
            Resolved::Shape { .. } => ParityCase::Reachable {
                label: "Shape",
                ast: Ast::Named("Player".to_string(), span.clone()),
                resolved: Resolved::Shape {
                    name: "Player".to_string(),
                },
                bare_expected: false,
                ec_expected: false,
            },
            Resolved::Dynamic { .. } => ParityCase::Reachable {
                label: "dynamic Damageable",
                ast: Ast::Dynamic {
                    contract: "Damageable".to_string(),
                    span: span.clone(),
                },
                resolved: Resolved::Dynamic {
                    contract: "Damageable".to_string(),
                },
                bare_expected: false,
                ec_expected: false,
            },
            // A user-defined generic shape resolves to `Resolved::Generic` (distinct from the
            // built-in `array`/`map` paths above) — declines in both positions.
            Resolved::Generic { .. } => ParityCase::Reachable {
                label: "user generic Pair<int,int>",
                ast: Ast::Generic {
                    name: "Pair".to_string(),
                    name_span: span.clone(),
                    args: vec![Ast::Int, Ast::Int],
                    span: span.clone(),
                },
                resolved: Resolved::Generic {
                    name: "Pair".to_string(),
                    args: vec![Resolved::Int, Resolved::Int],
                },
                bare_expected: false,
                ec_expected: false,
            },
            Resolved::Maybe { .. } => ParityCase::Reachable {
                label: "maybe<int>",
                ast: Ast::Maybe {
                    inner: Box::new(Ast::Int),
                    span: span.clone(),
                },
                resolved: Resolved::Maybe {
                    inner: Box::new(Resolved::Int),
                },
                bare_expected: false,
                ec_expected: false,
            },
            Resolved::Union { .. } => ParityCase::Reachable {
                label: "union",
                ast: Ast::Union {
                    variants: vec![
                        Ast::Named("Circle".to_string(), span.clone()),
                        Ast::Named("Square".to_string(), span.clone()),
                    ],
                    span: span.clone(),
                },
                resolved: Resolved::Union {
                    variants: vec![
                        Resolved::Shape {
                            name: "Circle".to_string(),
                        },
                        Resolved::Shape {
                            name: "Square".to_string(),
                        },
                    ],
                },
                bare_expected: false,
                ec_expected: false,
            },
            // `range` as a return annotation parses to `Named("range")` (no dedicated AST range
            // type) and resolves to `Resolved::Range`. The bare AST form is the `Named`, not
            // `Ast::Range` (which resolves to `Error` and is never written in type position).
            Resolved::Range { .. } => ParityCase::Reachable {
                label: "range",
                ast: Ast::Named("range".to_string(), span.clone()),
                resolved: Resolved::Range {
                    element: Box::new(Resolved::Int),
                    end_inclusive: false,
                },
                bare_expected: false,
                ec_expected: false,
            },
            // An `options` type name resolves to `Resolved::Options` (distinct from `Shape`).
            Resolved::Options { .. } => ParityCase::Reachable {
                label: "options",
                ast: Ast::Named("Status".to_string(), span.clone()),
                resolved: Resolved::Options {
                    name: "Status".to_string(),
                },
                bare_expected: false,
                ec_expected: false,
            },
            // `sensitive string` — wraps `string`; resolves to `Resolved::Sensitive`. Declines.
            Resolved::Sensitive { .. } => ParityCase::Reachable {
                label: "sensitive string",
                ast: Ast::Sensitive(Box::new(Ast::Named("string".to_string(), span.clone()))),
                resolved: Resolved::Sensitive {
                    inner: Box::new(Resolved::String),
                },
                bare_expected: false,
                ec_expected: false,
            },
            // `-> nothing` and `-> nothing errors` are BOTH production-handled (check.rs:1074
            // skips return-path analysis for `ErrorsCapable { inner: Nothing }`); there is no
            // value to join-bind, so both decline. NOT unreachable — the R5 mis-classification
            // (marking `Nothing` unreachable) is exactly what this row prevents recurring.
            Resolved::Nothing => ParityCase::Reachable {
                label: "nothing",
                ast: Ast::Nothing,
                resolved: Resolved::Nothing,
                bare_expected: false,
                ec_expected: false,
            },
            // A function whose return annotation fails to resolve carries `sig.ret = Error`
            // (the function already emitted a type error and never compiles to a binary, but
            // the gate is a pure classifier and must decline it). The AST counterpart is the
            // error-recovery placeholder `Ast::Error`, which codegen's gate also declines.
            // EC over `Error` is the same poisoned-sig story — declines.
            Resolved::Error => ParityCase::Reachable {
                label: "Error (poisoned sig)",
                ast: Ast::Error,
                resolved: Resolved::Error,
                bare_expected: false,
                ec_expected: false,
            },
            // `MapEntry<K,V>` IS writable as a return annotation (the `Generic { name: "MapEntry" }`
            // resolver arm at check.rs:3841 produces it), so `sig.ret` can be `MapEntry` — it is
            // reachable, not synthetic-only at the gate. Declines in both positions.
            Resolved::MapEntry { .. } => ParityCase::Reachable {
                label: "MapEntry<int,int>",
                ast: Ast::Generic {
                    name: "MapEntry".to_string(),
                    name_span: span.clone(),
                    args: vec![Ast::Int, Ast::Int],
                    span: span.clone(),
                },
                resolved: Resolved::MapEntry {
                    key: Box::new(Resolved::Int),
                    val: Box::new(Resolved::Int),
                },
                bare_expected: false,
                ec_expected: false,
            },
            // `TypeParam` is the ONLY genuinely-unreachable return class at the gate: generic
            // functions are routed to the `GenericFnTable`, NOT the `SignatureTable.fns` that the
            // CPU candidacy gate looks callees up in (signatures.rs:132 skips `!generics.is_empty()`).
            // A gated callee is therefore always non-generic, and its `sig.ret` is concrete — a
            // bare `TypeParam` never reaches `cpu_result_abi_supports`. Counted by the exhaustive
            // match (so a future variant still forces a decision) but no gate run.
            Resolved::TypeParam { .. } => ParityCase::UnreachableAsReturnInner(
                "generic-fn return types live in GenericFnTable, never in the CPU-gated SignatureTable.fns",
            ),
            // The `ErrorsCapable` wrapper itself is never a `-> T` inner of ANOTHER
            // `ErrorsCapable` (no `-> T errors errors` syntax); it is exercised via the
            // `ec_expected` arm of every Reachable bare class above, not as its own bare row.
            Resolved::ErrorsCapable { .. } => ParityCase::UnreachableAsReturnInner(
                "errors-wrapping is tested via the ec_expected arm of every bare class; \
                 a doubly-wrapped `T errors errors` is not expressible",
            ),
        }
    }

    // WHY: the CPU-parallel candidacy gate lives in two places that classify return types over
    // two DIFFERENT enums — codegen's `return_type_fits_cpu_result_abi` (un-resolved AST `Type`)
    // and typeck's `cpu_result_abi_supports` (resolved `Type`). Invariant: for every return
    // class, both gates must reach the IDENTICAL admit/decline verdict, in BOTH the bare and the
    // `errors`-wrapped form. If they diverge, the IDE `parallel_groups` hint (driven by typeck)
    // marks a call parallel while the emitted binary runs it sequentially — or worse, codegen
    // admits a class typeck declined (the number-errors UAF class).
    //
    // Coverage is COMPILE-FORCED exhaustive: `parity_case` is a `match` over every resolved
    // `Type` variant with no `_` arm, so a future-added variant is a BUILD ERROR until it is
    // classified here. Three prior rounds (R3 map, R4 maybe, R5 nothing) each found one more
    // variant the old hand-listed `cases` vec had silently omitted; this structure makes that
    // class of gap impossible. If you change one gate, change the other; do NOT relax this test.
    #[test]
    fn cpu_result_abi_gate_parity() {
        use ynz_ast::nodes::Type as Ast;
        use ynz_typeck::types::Type as Resolved;

        let span = dummy_span();
        // One representative per resolved `Type` variant. The classifier (`parity_case`) is the
        // exhaustiveness driver — every variant present here is matched by an arm that has no
        // `_` fallback, so the compiler rejects any future variant that is not classified.
        let all_variants: Vec<Resolved> = vec![
            Resolved::Nothing,
            Resolved::String,
            Resolved::Error,
            Resolved::Int,
            Resolved::Float,
            Resolved::Number { precision: 34 },
            Resolved::Bool,
            Resolved::Range {
                element: Box::new(Resolved::Int),
                end_inclusive: false,
            },
            Resolved::Shape {
                name: "Player".to_string(),
            },
            Resolved::Dynamic {
                contract: "Damageable".to_string(),
            },
            Resolved::TypeParam {
                name: "T".to_string(),
            },
            Resolved::Generic {
                name: "Pair".to_string(),
                args: vec![Resolved::Int, Resolved::Int],
            },
            Resolved::BuiltinArray {
                elem: Box::new(Resolved::Int),
            },
            Resolved::BuiltinFixed {
                elem: Box::new(Resolved::Int),
                size: None,
            },
            Resolved::Maybe {
                inner: Box::new(Resolved::Int),
            },
            Resolved::BuiltinMap {
                key: Box::new(Resolved::Int),
                val: Box::new(Resolved::Int),
            },
            Resolved::MapEntry {
                key: Box::new(Resolved::Int),
                val: Box::new(Resolved::Int),
            },
            Resolved::Options {
                name: "Status".to_string(),
            },
            Resolved::Union {
                variants: vec![
                    Resolved::Shape {
                        name: "Circle".to_string(),
                    },
                    Resolved::Shape {
                        name: "Square".to_string(),
                    },
                ],
            },
            Resolved::ErrorsCapable {
                inner: Box::new(Resolved::Int),
            },
            Resolved::Sensitive {
                inner: Box::new(Resolved::String),
            },
        ];

        // Assert the representative set is itself complete: if a variant were missing from
        // `all_variants`, this loop would never drive `parity_case` for it, so the compile-time
        // exhaustiveness guard would not protect it. `parity_case` covering every variant is the
        // compile-time half; iterating every variant here is the runtime half — together they
        // guarantee each variant is BOTH classified and exercised.
        for variant in &all_variants {
            match parity_case(variant) {
                ParityCase::Reachable {
                    label,
                    ast,
                    resolved,
                    bare_expected,
                    ec_expected,
                } => {
                    // Bare `-> T` form: both gates classify the un-wrapped type.
                    let bare_codegen = return_type_fits_cpu_result_abi(&ast);
                    let bare_typeck = cpu_result_abi_supports(&resolved);
                    assert_eq!(
                        bare_codegen, bare_typeck,
                        "gate divergence for bare `{label}`: codegen={bare_codegen}, \
                         typeck={bare_typeck} — both gates must agree (hint/binary parity)"
                    );
                    assert_eq!(
                        bare_codegen, bare_expected,
                        "wrong verdict for bare `{label}`: got {bare_codegen}, \
                         expected {bare_expected}"
                    );

                    // `-> T errors` form: wrap both forms and re-run. This drives codegen's
                    // `ec_inner_fits_cpu_result_abi` and typeck's `cpu_result_ec_inner_is_safe`.
                    let ec_ast = Ast::ErrorCapable {
                        inner: Box::new(ast),
                        span: span.clone(),
                    };
                    let ec_resolved = Resolved::ErrorsCapable {
                        inner: Box::new(resolved),
                    };
                    let ec_codegen = return_type_fits_cpu_result_abi(&ec_ast);
                    let ec_typeck = cpu_result_abi_supports(&ec_resolved);
                    assert_eq!(
                        ec_codegen, ec_typeck,
                        "gate divergence for `{label} errors`: codegen={ec_codegen}, \
                         typeck={ec_typeck} — both gates must agree (hint/binary parity)"
                    );
                    assert_eq!(
                        ec_codegen, ec_expected,
                        "wrong verdict for `{label} errors`: got {ec_codegen}, \
                         expected {ec_expected}"
                    );
                }
                // No gate run — the variant cannot appear as a function-return type. It is still
                // classified by the exhaustive `parity_case` match (the compiler counted it), so
                // a future variant cannot slip in unclassified.
                ParityCase::UnreachableAsReturnInner(_why) => {}
            }
        }
    }

    // WHY: pins the exact byte offsets the two-member CPU group occupies. These offsets
    // (handles @ 32/40, results @ 48/64) are an ABI contract with the runtime drop-shim
    // `cleanup_spike_cpu_handles`, which reads handle slots at 32/40. A regression here
    // silently mis-offsets the join result or the freed handle — corpse-class corruption.
    #[test]
    fn build_cpu_group_slots_two_members_pins_abi_offsets() {
        let slots = build_cpu_group_slots(2);
        assert_eq!(
            slots,
            vec![
                CpuGroupSlot {
                    group_id: 0,
                    member_index: 0,
                    handle_offset: 32,
                    result_offset: 48,
                },
                CpuGroupSlot {
                    group_id: 0,
                    member_index: 1,
                    handle_offset: 40,
                    result_offset: 64,
                },
            ],
            "two-member CPU group must place handles @ 32/40 and results @ 48/64"
        );
    }

    // WHY: the (group_id, member_index) keying must generalize before slice 2 relies on
    // N>2 members. Three members put handles contiguously (32/40/48 — one 8-byte slot
    // each) then results contiguously (56/72/88 — one 16-byte slot each). If the handle
    // and result regions ever interleave instead of grouping, this catches it.
    #[test]
    fn build_cpu_group_slots_three_members_generalizes() {
        let slots = build_cpu_group_slots(3);
        let handle_offsets: Vec<u64> = slots.iter().map(|s| s.handle_offset).collect();
        let result_offsets: Vec<u64> = slots.iter().map(|s| s.result_offset).collect();
        assert_eq!(
            handle_offsets,
            vec![32, 40, 48],
            "three handle slots are contiguous 8-byte slots after the 32-byte header"
        );
        assert_eq!(
            result_offsets,
            vec![56, 72, 88],
            "three result slots are contiguous 16-byte slots after the handle region"
        );
    }

    // WHY: a zero-member group (every non-promoted function) must reserve nothing — its
    // frame stays byte-identical to pre-M3d. A non-empty Vec here would inflate every
    // frame, breaking the zero-cost-for-non-promoted invariant.
    #[test]
    fn build_cpu_group_slots_zero_members_is_empty() {
        assert!(
            build_cpu_group_slots(0).is_empty(),
            "a non-promoted function reserves no CPU slots"
        );
    }

    // WHY: `cpu_slot_reserve_slots` returns the count of 8-byte slots own-locals must skip
    // to clear the CPU region. For the two-member group the region spans header(32) → 80
    // (result 64 + 16), i.e. 48 bytes = 6 eight-byte slots. An off-by-one here aliases a
    // crossing-local slot onto a result slot — silent wrong join value after a suspension.
    #[test]
    fn cpu_slot_reserve_slots_two_member_group_is_six() {
        let layout = layout_with_cpu_slots(build_cpu_group_slots(2));
        assert_eq!(
            cpu_slot_reserve_slots(&layout),
            6,
            "two-member CPU region (header→80) is 6 eight-byte slots"
        );
    }

    // WHY: an empty CPU group reserves zero slots — own-locals start immediately after the
    // header, preserving pre-M3d frame layout for non-promoted functions.
    #[test]
    fn cpu_slot_reserve_slots_empty_group_is_zero() {
        let layout = layout_with_cpu_slots(Vec::new());
        assert_eq!(
            cpu_slot_reserve_slots(&layout),
            0,
            "no CPU group reserves zero slots"
        );
    }

    fn wait_expr() -> Expr {
        // A bare `wait` wrapping an integer literal — the wrapped expr type does not
        // matter for the contains-wait walk; only the Wait variant is tested here.
        Expr::Wait(Box::new(Expr::IntLit(0, dummy_span())), dummy_span())
    }

    // WHY: guards against the routing decision regressing — if function_contains_wait
    // returns false for a wait-containing function, that function gets lowered on the
    // straight-line path and the wait becomes a silent no-op (M1 regression).
    #[test]
    fn block_with_top_level_wait_returns_true() {
        let block = Block {
            stmts: vec![Stmt::Expr(wait_expr())],
            span: dummy_span(),
        };
        assert!(
            function_contains_wait(&block),
            "block containing a bare Expr::Wait must return true"
        );
    }

    // WHY: a wait-free function must take the standard lowering path with zero
    // added overhead. False positives here would route every simple function through
    // the state-machine path, bloating codegen for all programs.
    #[test]
    fn block_without_wait_returns_false() {
        let block = Block {
            stmts: vec![Stmt::Expr(Expr::IntLit(42, dummy_span()))],
            span: dummy_span(),
        };
        assert!(
            !function_contains_wait(&block),
            "block with no Wait must return false"
        );
    }

    // WHY: Increment A routes `if`-nested waits to the SM lowering path. If this
    // returns false, an `if`-nested wait silently regresses to the M1 no-op behavior.
    #[test]
    fn block_with_if_nested_wait_returns_true() {
        let if_body = Block {
            stmts: vec![Stmt::Expr(wait_expr())],
            span: dummy_span(),
        };
        let if_stmt = Stmt::If {
            cond: Expr::BoolLit(true, dummy_span()),
            body: if_body,
            span: dummy_span(),
        };
        let block = Block {
            stmts: vec![if_stmt],
            span: dummy_span(),
        };
        assert!(
            function_contains_wait(&block),
            "block containing an if-nested Expr::Wait must return true"
        );
    }
}
