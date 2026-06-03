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
    targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
    },
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, GlobalValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use ynz_ast::nodes::{
    BinOpKind, Expr, FunctionDecl, Item, MatchPatternKind, OwnershipModifier, Stmt, UnaryOpKind,
};
use ynz_numerics; // parse(s: &str) -> Option<u128>
use ynz_typeck::{
    crossing_local_names, type_attached_const_type, GenericFnTable, MonomorphizationTable,
    ShapeTable, SignatureTable, Type, TypedModule,
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

/// Composed-frame layout for one suspending function.
///
/// A composed frame embeds the sub-frames of all directly-called suspending children
/// at compile-time-fixed byte offsets, so the entire intra-function call tree shares
/// ONE `ynz_alloc` per spawned task.
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
}

/// True when `f` is a suspending function that returns `-> number errors` (decimal128 EC).
///
/// These functions need a 16-byte staging slot in their composed frame: the SM EC-return
/// path stores the i128 decimal there and points the EC `ok` word at it. The slot is freed
/// automatically when the frame drops (one `ynz_alloc` invariant, alloc=1/free=1).
/// True when `f` returns `-> number errors` (decimal128 EC).
///
/// The AST stores `-> number errors` as `return_type = ErrorCapable { inner = Number { .. } }`.
/// These functions need a 16-byte staging slot in their composed frame so the EC ok-word
/// points at a frame-stable region rather than a resume-stack alloca.
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
/// 1. Walk each function's AST to collect its direct suspending callees in call order
///    (deduplicating per callee name).
/// 2. Detect recursion edges via a simple ancestor-set DFS (no topological sort needed
///    for the recursion-detection goal).
/// 3. Compute total_size bottom-up: leaf functions have size = header + own_locals;
///    internal nodes add the sizes of all non-recursive children.
fn build_frame_layouts(
    typed: &TypedModule,
    suspend_set: &SuspendSet,
    shape_abi_sizes: &HashMap<String, u64>,
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

    // Step 2 + 3: compute frame sizes recursively with cycle detection.
    // Use a memo map and a visiting set.
    let mut sizes: HashMap<String, u64> = HashMap::new();
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

    // Step 4: build FrameLayout for each fn using the computed sizes.
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
            let crossing = crossing_local_names(&f.body.stmts, &param_names_ref, &suspending_refs);
            let crossing_slots = crossing_local_total_slots(f, &crossing, typed, shape_abi_sizes);
            let n_locals = f.params.len() + crossing_slots;
            let own_base =
                state_machine::FRAME_HEADER_SIZE + state_machine::own_locals_size(n_locals);
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
            layouts.insert(
                f.name.clone(),
                FrameLayout {
                    total_size,
                    n_locals,
                    children,
                    recursion_slot,
                    number_errors_staging_offset,
                },
            );
        }
    }
    layouts
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

    // Find n_locals and staging requirements for this fn.
    let (n_locals, needs_number_errors_staging) = typed
        .module
        .items
        .iter()
        .find_map(|item| {
            if let Item::Function(f) = item {
                if f.name == fn_name {
                    let param_names: Vec<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
                    let suspending_refs: HashSet<&str> =
                        suspend_set.iter().map(|s| s.as_str()).collect();
                    let crossing =
                        crossing_local_names(&f.body.stmts, &param_names, &suspending_refs);
                    let crossing_slots =
                        crossing_local_total_slots(f, &crossing, typed, shape_abi_sizes);
                    return Some((f.params.len() + crossing_slots, is_number_errors_return(f)));
                }
            }
            None
        })
        .unwrap_or((0, false));

    let own_base = state_machine::FRAME_HEADER_SIZE + state_machine::own_locals_size(n_locals);
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
) -> Result<CompiledArtifact, String> {
    Target::initialize_x86(&InitializationConfig::default());

    let context = Context::create();
    let module_id = module_identifier(source_path);
    let module = context.create_module(&module_id);

    let triple = match target_triple {
        Some(t) => TargetTriple::create(t),
        None => TargetMachine::get_default_triple(),
    };
    module.set_triple(&triple);

    let target = Target::from_triple(&triple)
        .map_err(|e| format!("LLVM: no target for triple {:?}: {e}", triple.as_str()))?;
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| "LLVM: failed to create target machine".to_string())?;
    module.set_data_layout(&machine.get_target_data().get_data_layout());

    // Use the suspends_set passed in from check_query (computed by may_block::analyze).
    // This is the Phase-7 seam fix: the pre-analysis sig_table (from module_signatures_query)
    // has suspends=false for all fns; the real transitive set comes from check_query.
    let suspend_set: SuspendSet = suspends_set_arg.clone();
    let _ = sig_table; // sig_table kept in signature for API compatibility

    build_module(
        &context,
        &module,
        source_path,
        typed_module,
        shape_table,
        generic_fn_table,
        mono_table,
        imported_options,
        &suspend_set,
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
/// # Flow (5 passes — order is mandatory)
///
/// | Pass | What | Requires from prior passes |
/// |------|------|---------------------------|
/// | 0 | Emit LLVM struct types for all user-defined shapes | nothing |
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
    generic_fn_table: &'g GenericFnTable,
    mono_table: &'g MonomorphizationTable,
    imported_options: &std::collections::HashMap<String, ynz_typeck::options_table::OptionsEntry>,
    suspend_set: &'g SuspendSet,
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

    // Pass 0.5 — build the wait cache (kept for backward-compat with generic lowering +
    // background routing) AND compute frame layouts for all suspending functions.
    //
    // The wait_cache still serves the local-syntactic check used by non-SM call sites.
    // frame_layouts encodes the composed structure (embedded child sub-frames) used by
    // lower_function_with_waits to allocate ONE frame per task tree.
    let wait_cache = build_wait_cache(typed);
    let frame_layouts = build_frame_layouts(typed, suspend_set, &shape_abi_sizes);

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
                &frame_layouts,
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
        sm_frame_ptr: None,
        sm_yinz_ret_ty: None,
        sm_crossing_names: None,
        sm_crossing_scalar_set: HashSet::new(),
        sm_crossing_bool_set: HashSet::new(),
        sm_crossing_slot_indices: Vec::new(),
        sm_crossing_decimal128_set: HashSet::new(),
        sm_crossing_float_set: HashSet::new(),
        sm_crossing_errors_capable_set: HashSet::new(),
        sm_crossing_shape_embed_set: HashSet::new(),
        sm_crossing_ec_struct_allocas: HashMap::new(),
        sm_crossing_shape_names: HashMap::new(),
        sm_crossing_shape_allocas: HashMap::new(),
        sm_scope_depth: 0,
        sm_number_errors_staging_offset: None,
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
    // Byte offset of the 16-byte `number errors` staging slot within the composed frame,
    // when the current SM function returns `-> number errors`. None for all other functions.
    // Used by lower_stmt_return to write the i128 decimal to a frame-stable location so
    // the EC ok-pointer survives the resume function returning.
    sm_number_errors_staging_offset: Option<u64>,
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
        sm_frame_ptr: None,
        sm_yinz_ret_ty: None,
        sm_crossing_names: None,
        sm_crossing_scalar_set: HashSet::new(),
        sm_crossing_bool_set: HashSet::new(),
        sm_crossing_slot_indices: Vec::new(),
        sm_crossing_decimal128_set: HashSet::new(),
        sm_crossing_float_set: HashSet::new(),
        sm_crossing_errors_capable_set: HashSet::new(),
        sm_crossing_shape_embed_set: HashSet::new(),
        sm_crossing_ec_struct_allocas: HashMap::new(),
        sm_crossing_shape_names: HashMap::new(),
        sm_crossing_shape_allocas: HashMap::new(),
        sm_scope_depth: 0,
        sm_number_errors_staging_offset: None,
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
) -> Result<(), String> {
    // Collect the names of parameters. ALL parameters are live across any wait.
    let param_names: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();

    // Compute the set of locals that cross a suspension boundary (declared before
    // a wait, read after it). These must live in the heap frame instead of SSA
    // registers so their values survive across resume calls.
    let param_name_refs: Vec<&str> = param_names.iter().map(|s| s.as_str()).collect();
    let suspending_refs: HashSet<&str> = suspend_set.iter().map(|s| s.as_str()).collect();
    let crossing_names: Vec<String> =
        crossing_local_names(&f.body.stmts, &param_name_refs, &suspending_refs);

    // Slot index layout: params occupy slots [0..n_params), crossing locals occupy
    // slots starting at n_params. decimal128 + EC use 2 consecutive slots; shapes use
    // ceil(N/8) consecutive slots (frame-embedded); all others use 1.
    // This matches the slot counting in build_frame_layouts.
    let n_params = param_names.len();
    // Compute per-crossing-local slot indices using typeck types (catches inferred number).
    let crossing_slot_indices: Vec<usize> = {
        let mut indices = Vec::with_capacity(crossing_names.len());
        let mut cursor = n_params;
        for cname in &crossing_names {
            indices.push(cursor);
            let ty = find_let_typeck_type_in_stmts(&f.body.stmts, cname.as_str(), typed);
            let slots = match ty {
                Some(Type::Number { precision }) if precision <= 34 => 2,
                Some(Type::ErrorsCapable { .. }) => 2,
                Some(Type::Shape { name: ref sname }) => shape_frame_slots(sname, shape_abi_sizes),
                _ => 1,
            };
            cursor += slots;
        }
        indices
    };
    let n_locals =
        n_params + crossing_local_total_slots(f, &crossing_names, typed, shape_abi_sizes);

    // Look up the composed frame layout for this function. The total_size covers
    // header(32) + own_locals + optional 16-byte number-errors staging slot + embedded child
    // sub-frames = ONE allocation per task tree.
    let frame_layout = frame_layouts.get(&f.name);
    let frame_bytes = frame_layout.map(|l| l.total_size).unwrap_or_else(|| {
        state_machine::FRAME_HEADER_SIZE + state_machine::own_locals_size(n_locals)
    });
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
    let n_waits = count_suspension_points(&f.body, suspend_set);
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
        sm_scope_depth: 0,
        sm_number_errors_staging_offset: number_errors_staging_offset,
        // Carry errors-capable flag separately so lower_stmt_return can handle it.
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
            }
        }
        cg_resume.sm_crossing_scalar_set = scalar_set;
        cg_resume.sm_crossing_bool_set = bool_set;
        cg_resume.sm_crossing_decimal128_set = decimal128_set;
        cg_resume.sm_crossing_float_set = float_set;
        cg_resume.sm_crossing_errors_capable_set = errors_capable_set;
        cg_resume.sm_crossing_shape_embed_set = shape_embed_set;
        cg_resume.sm_crossing_ec_struct_allocas = ec_struct_allocas;
        cg_resume.sm_crossing_shape_names = shape_names_map;
        cg_resume.sm_crossing_shape_allocas = shape_allocas_map;
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
    // This eliminates the separate ynz_alloc per shape crossing local (the old bug).
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
        // The ok-word may reference frame-resident storage (e.g. the 16-byte `-> number errors`
        // staging slot inside the composed frame). That storage is freed by free_frame below,
        // so the returned EC struct's ok-pointer becomes invalid after this function returns.
        //
        // This is safe for the only reachable caller today: `background` (fire-and-forget)
        // DISCARDS the EC result entirely — the returned struct is never dereferenced. A caller
        // that COLLECTS the result must read and copy the ok-value BEFORE free_frame; that
        // read-before-free + copy path is deferred to M3b (`background` result-collection).
        // See `registry/features.toml` entry `ec-wrapper-collect-on-completion` and
        // `design/concurrency.md` M3a Scope Boundaries for the full rationale.
        let (err_i64, ok_i64) = state_machine::load_return_value_errors(ctx, &builder, frame_ptr)?;
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
            let shape_embed_set = cg.sm_crossing_shape_embed_set.clone();
            let ec_struct_allocas = cg.sm_crossing_ec_struct_allocas.clone();
            for (i, cname) in crossing_names.iter().enumerate() {
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
                        // ErrorsCapable: reload 2 frame slots into the companion sm_entry
                        // struct alloca, then ensure the ptr alloca points at it.
                        let hi_bits = state_machine::load_local_slot(
                            ctx,
                            &cg.builder,
                            frame_ptr,
                            slot_idx + 1,
                            &format!("{cname}_ec_hi"),
                        )?;
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
                        cg.builder
                            .build_store(f0_ptr, bits)
                            .map_err(|e| format!("reload ec f0 store {cname}: {e}"))?;
                        cg.builder
                            .build_store(f1_ptr, hi_bits)
                            .map_err(|e| format!("reload ec f1 store {cname}: {e}"))?;
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
            // Shape-typed crossing locals at a `let` definition site: `lower_struct_lit`
            // creates a fresh stack alloca and stores its pointer in cg.locals[name].
            // The stack alloca dies when this resume call returns. Copy the struct bytes
            // into the pre-existing sm_entry struct alloca (the stable working copy that
            // `flush_crossing_local_if_needed` then memcpy's into the frame slots).
            anchor_shape_crossing_locals_to_frame_alloca(cg, stmt)?;
            // After lowering a non-wait statement, flush any crossing local it defined or
            // mutated back to the frame slot so the value survives the next suspension.
            flush_crossing_local_if_needed(cg, stmt, frame_ptr)?;
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
    let ctx = cg.ctx;

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
        {
            let slot_idx = cg.sm_crossing_slot_indices[slot_pos];
            let is_int = cg.sm_crossing_scalar_set.contains(name);
            let is_bool = cg.sm_crossing_bool_set.contains(name);
            let is_float = cg.sm_crossing_float_set.contains(name);
            let is_decimal128 = cg.sm_crossing_decimal128_set.contains(name);
            let is_errors_capable = cg.sm_crossing_errors_capable_set.contains(name);
            let is_shape_embed = cg.sm_crossing_shape_embed_set.contains(name);
            // Load current value from alloca, convert to i64 bits, store to frame slot(s).
            if let Some(&alloca) = cg.locals.get(name) {
                if is_int {
                    // Int: i64 alloca — raw i64 load, 1 slot.
                    let bits = cg
                        .builder
                        .build_load(ctx.i64_type(), alloca, &format!("{name}_flush_load"))
                        .map_err(|e| format!("crossing flush load {name}: {e}"))?
                        .into_int_value();
                    state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
                } else if is_bool {
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
                } else if is_float {
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
                } else if is_decimal128 {
                    // Decimal128: i128 alloca — split into 2 i64 halves, 2 slots.
                    // Frame holds the value directly (not a pointer to stack).
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
                } else if is_errors_capable {
                    // ErrorsCapable {i64,i64}: load the ptr from the ptr alloca, then load
                    // both i64 fields from the companion struct and store to 2 frame slots.
                    // The companion struct alloca lives in sm_entry so it is always valid here.
                    let ec_struct_ty =
                        ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false);
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
                    let f1_ptr = cg
                        .builder
                        .build_struct_gep(ec_struct_ty, struct_ptr, 1, &format!("{name}_f1_gep"))
                        .map_err(|e| format!("crossing flush ec f1 gep {name}: {e}"))?;
                    let f0 = cg
                        .builder
                        .build_load(ctx.i64_type(), f0_ptr, &format!("{name}_f0"))
                        .map_err(|e| format!("crossing flush ec f0 {name}: {e}"))?
                        .into_int_value();
                    let f1 = cg
                        .builder
                        .build_load(ctx.i64_type(), f1_ptr, &format!("{name}_f1"))
                        .map_err(|e| format!("crossing flush ec f1 {name}: {e}"))?
                        .into_int_value();
                    state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx, f0)?;
                    state_machine::store_local_slot(ctx, &cg.builder, frame_ptr, slot_idx + 1, f1)?;
                } else if is_shape_embed {
                    // Shape crossing local: frame-embedded.
                    // The ptr alloca points directly into the composed frame's slot region
                    // (wired in Step 1b of lower_function_with_waits). All field writes
                    // through the alloca go directly to the frame — no flush needed.
                    // The `flush` call is a no-op for shape crossing locals.
                } else {
                    // Pointer alloca (string/array/map/etc.): load the heap pointer, ptr_to_int.
                    // These types already live on the heap so the pointer is stable.
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
            }
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
            let alloca = bind_sm_result_and_flush(cg, name, return_val, frame_ptr)?;
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
            let alloca = bind_sm_result_and_flush(cg, name, return_val, frame_ptr)?;
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

        _ => {
            if stmt_contains_wait(stmt) && !stmt_contains_suspending_call(stmt, cg.suspend_set) {
                panic!(
                    "BUG: SM codegen reached a wait-bearing statement that typeck should have \
                     rejected. The WaitInsideLoop guard (for/match) regressed. \
                     Statement: {stmt:?}"
                );
            }
            lower_stmt(cg, stmt)?;
        }
    }
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
/// - Anything else: fall back to i64 load.
fn load_sm_return_value_typed<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    ctx: &'ctx inkwell::context::Context,
    frame_ptr: PointerValue<'ctx>,
    callee_name: &str,
    tag: &str,
) -> Result<inkwell::values::BasicValueEnum<'ctx>, String> {
    // Look up the callee's declared return type.
    let callee_ret_ty = cg.typed.module.items.iter().find_map(|item| {
        if let ynz_ast::nodes::Item::Function(f) = item {
            if f.name == callee_name {
                return Some(ast_type_to_typeck_type(&f.return_type, cg.shape_table));
            }
        }
        None
    });

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
/// After this call the alloca is registered in `cg.locals` under `name`.
fn bind_sm_result_and_flush<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    name: &str,
    return_val: inkwell::values::BasicValueEnum<'ctx>,
    frame_ptr: PointerValue<'ctx>,
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
                // ErrorsCapable {i64, i64}: extract both fields, store into the companion
                // sm_entry struct alloca (pre-created for this crossing local), and flush
                // both words to the 2 consecutive frame slots. Mirrors the EC flush path.
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
                // The alloca is a ptr alloca pointing at the companion struct alloca.
                let struct_ptr = *cg.sm_crossing_ec_struct_allocas.get(name).ok_or_else(|| {
                    format!("bind_sm_result_and_flush: EC companion alloca for `{name}` missing")
                })?;
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
                cg.builder
                    .build_store(f1_ptr, f1)
                    .map_err(|e| format!("bind_sm_result ec store f1 {name}: {e}"))?;
                // Ensure ptr alloca points at companion struct.
                cg.builder
                    .build_store(alloca, struct_ptr)
                    .map_err(|e| format!("bind_sm_result ec ptr init {name}: {e}"))?;
                // Flush both words to frame slots.
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, f0)?;
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx + 1, f1)?;
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
                cg.builder
                    .build_store(alloca, bits)
                    .map_err(|e| format!("bind_sm_result store {name}: {e}"))?;
                state_machine::store_local_slot(cg.ctx, &cg.builder, frame_ptr, slot_idx, bits)?;
            }
        }
        Ok(alloca)
    } else {
        // Non-crossing: use the standard bind path.
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
            // ErrorsCapable {i64,i64}: 2 frame slots for the two fields directly.
            Some(Type::ErrorsCapable { .. }) => 2,
            // Shape: frame-embed the struct bytes in ceil(N/8) consecutive slots.
            Some(Type::Shape { name: ref sname }) => shape_frame_slots(sname, shape_abi_sizes),
            _ => 1,
        };
        total += slots;
    }
    total
}

/// Look up the typeck-inferred `Type` for a let binding by scanning the function body.
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
/// Scans the function body for the first `Stmt::Let { name }` matching `target`
/// and returns the type of its RHS expression from `typed.expr_types`. Falls back
/// to `Type::Int` (i64, safe default) if the binding cannot be found — this can only
/// happen if the crossing local was produced by a suspending call (in which case
/// the bind_sm_result_and_flush path handles the alloca separately and the pre-created
/// alloca is never used for the definition store).
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

    // Find the child's resume function.
    let resume_name = state_machine::resume_fn_name(&callee_name);
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
    if is_errors_capable_fn(cg.typed, &callee_name) {
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

    let resume_name = state_machine::resume_fn_name(callee_name);
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

    let ret_val = if is_errors_capable_fn(cg.typed, callee_name) {
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
            // Hardware decimal128 path (N ≤ 34).
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
                    // Prefer the direct name. If not found, find the correct monomorphized
                    // variant by matching argument types against MonomorphizationTable entries.
                    let effective_name = if cg.module.get_function(name).is_some() {
                        name.to_string()
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
                    let callee_is_ec = is_errors_capable_fn(cg.typed, &effective_name);
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
                    for elem_expr in elements {
                        let elem_ty2 = cg.expr_type(elem_expr);
                        let elem_val = lower_expr(cg, elem_expr)?;
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
    let mut arg_vals_i64: Vec<inkwell::values::IntValue<'ctx>> = Vec::new();
    let mut arg_types: Vec<Type> = Vec::new();
    for arg in &call.args {
        let val = lower_expr(cg, arg)?;
        let ty = cg.expr_type(arg);
        let bits = cg
            .to_i64_bits(val, &ty)
            .map_err(|e| format!("background arg to_i64_bits: {e}"))?;
        arg_vals_i64.push(bits);
        arg_types.push(ty);
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
    let effective_name = if cg.module.get_function(&callee_name).is_some() {
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
    let mut arg_vals_i64: Vec<inkwell::values::IntValue<'ctx>> = Vec::new();
    let mut arg_types: Vec<Type> = Vec::new();
    for arg in &call.args {
        let val = lower_expr(cg, arg)?;
        let ty = cg.expr_type(arg);
        let bits = cg
            .to_i64_bits(val, &ty)
            .map_err(|e| format!("sm bg arg bits: {e}"))?;
        arg_vals_i64.push(bits);
        arg_types.push(ty);
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

    // Step 4: find the resume function and call ynz_rt_spawn.
    let resume_name = state_machine::resume_fn_name(callee_name);
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

    cg.builder
        .build_call(
            cg.rt.ynz_rt_spawn,
            &[
                resume_ptr.into(),
                frame_ptr.into(),
                frame_size_val.into(),
                rec_slot_offset_val.into(),
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

/// True when the named function in `typed_module` has `errors_capable = true`.
fn is_errors_capable_fn(typed: &TypedModule, fn_name: &str) -> bool {
    typed.module.items.iter().any(|item| {
        if let ynz_ast::nodes::Item::Function(f) = item {
            f.name == fn_name && f.errors_capable
        } else {
            false
        }
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
fn lower_errors_capable_call_result<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    result_struct: inkwell::values::StructValue<'ctx>,
    _callee_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
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

#[cfg(test)]
mod tests {
    use super::function_contains_wait;
    use ynz_ast::nodes::{Block, Expr, Stmt};
    use ynz_diagnostics::SourceSpan;

    fn dummy_span() -> SourceSpan {
        SourceSpan::new("test.ynz", 0, 1)
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
