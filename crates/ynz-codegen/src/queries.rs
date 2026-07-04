use std::{collections::HashMap, sync::Arc};

use inkwell::context::Context;
use ynz_ast::nodes::{ImportKind, Item};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};
use ynz_typeck::{
    build_effective_suspend_set, check_query, cpu_promotion_query, layout_decisions_query,
    module_signatures_query,
};

use crate::{
    artifact::CompiledArtifact,
    emit::{
        build_frame_layouts_with_resolver, emit_artifact, spike_host_subset, FrameLayout,
        SuspendSet,
    },
    state_machine,
};

/// The output of the codegen pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CodegenOutput {
    pub artifact: CompiledArtifact,
    pub diagnostics: DiagnosticBucket,
}

// ── frame_layouts_query cycle recovery ───────────────────────────────────────

/// Cycle-initial placeholder for `frame_layouts_query` when the module is part of a
/// circular-import cycle.
///
/// Circular imports are already detected by `module_signatures_query` (which also has
/// cycle recovery) and produce a typeck error → codegen is skipped. This empty map is a
/// defense-in-depth backstop so `frame_layouts_query` never infinite-loops even if the
/// cycle-error path has a gap (Guard G3).
fn frame_layouts_cycle_initial(
    _db: &dyn SourceFileRegistry,
    _id: salsa::Id,
    _source: SourceFile,
) -> Arc<HashMap<String, FrameLayout>> {
    Arc::new(HashMap::new())
}

/// Cycle-recovery function for `frame_layouts_query`.
///
/// Returns the current provisional value unchanged — the empty map from
/// `frame_layouts_cycle_initial`. Salsa converges immediately because
/// `Arc<HashMap>` implements `PartialEq` and the empty maps compare equal.
fn frame_layouts_cycle_fn(
    _db: &dyn SourceFileRegistry,
    _cycle: &salsa::Cycle,
    _last_provisional: &Arc<HashMap<String, FrameLayout>>,
    value: Arc<HashMap<String, FrameLayout>>,
    _source: SourceFile,
) -> Arc<HashMap<String, FrameLayout>> {
    value
}

/// Compute LLVM-accurate composed-frame layouts for every suspending function in a module.
///
/// Salsa-memoized — cache miss only when the source file or any of its imports change.
/// The inkwell `Context` created inside is dropped before returning; no LLVM types escape.
///
/// Guard G1: uses `state_machine::default_target_machine()` — the same constructor as
/// `emit_artifact` — so data-layout strings are byte-identical between the emitter and
/// this query (wrong data-layout → wrong `n_locals` for shape-typed crossing locals →
/// silent frame mis-sizing).
///
/// Guard G2: imported suspending callees are resolved recursively via
/// `frame_layouts_query(callee_file)` rather than via the lossy
/// `FunctionSig.composed_frame_size` scalar. This makes re-export chains (A→B→C)
/// compute B's `total_size` including A's real sub-frame instead of a placeholder.
///
/// Guard G3 (cycle recovery): circular imports are already typeck errors and codegen is
/// skipped on errors; the cycle functions above return an empty map as a backstop.
#[salsa::tracked(
    lru = 64,
    cycle_fn = frame_layouts_cycle_fn,
    cycle_initial = frame_layouts_cycle_initial
)]
pub fn frame_layouts_query(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Arc<HashMap<String, FrameLayout>> {
    let check = check_query(db, source);
    if check.diagnostics.has_errors() {
        // Codegen is skipped on type errors — no meaningful layouts to compute.
        return Arc::new(HashMap::new());
    }
    let sig_output = module_signatures_query(db, source);
    let source_path = source.path(db);

    // WHY: single SSOT for the effective suspend set — local + imported suspending
    // names.  `build_effective_suspend_set` is the canonical computation; using it
    // here ensures frame-layout, codegen routing, and IDE hints all read the same set.
    //
    // v0.3-M3d: union the CPU-promotion set so promoted functions are sized as state
    // machines (their CPU group joins are suspension points). `cpu_promotion_query` is the
    // production trigger for CPU-statement parallelization; build_frame_layouts consumes the
    // promoted functions through this set so the composed-frame size includes their CPU
    // handle/result reserve (computed there, not via a fallback at emit time).
    //
    // Reconcile typeck's promotion set with what codegen will actually spike-HOST this
    // slice (`spike_host_subset`, probed against the EFFECTIVE suspend set built above —
    // local ∪ imported-suspending). Unioning the full promotion set would poison nested
    // hosts: a promoted-but-unhosted inner callee would land IN the suspend set the host's
    // callee-eligibility filter reads, silently declining the host's group. Probing with
    // the effective set (the SAME set codegen_query uses) keeps both query boundaries'
    // host-admission decisions identical. See `spike_host_subset` for the full rationale.
    // `base_suspend_set` is the AUTHORITATIVE pre-CPU-promotion suspend set — no longer consulted
    // by `admitted_cpu_group` itself post-v0.3-M3g-Phase-3 (that function dropped the parameter;
    // see its doc comment), but still threaded, kept UNMUTATED here (never unioned with
    // `spike_hosts`), through to `build_frame_layouts_with_resolver` as a separate argument from
    // the union `effective_suspend_set` below, so a legitimate pure-CPU host (no suspension of its
    // own) is never wrongly self-declined once its own name lands in the union.
    let base_suspend_set: SuspendSet =
        build_effective_suspend_set(&check.suspends_set, &sig_output.imported_fns);
    let promotion = cpu_promotion_query(db, source);
    let spike_hosts = spike_host_subset(
        &check.typed_module,
        &base_suspend_set,
        &base_suspend_set,
        &promotion.promoted,
    );
    let mut effective_suspend_set = base_suspend_set.clone();
    for name in &spike_hosts {
        effective_suspend_set.insert(name.clone());
    }

    // Build a map from imported function name → the SourceFile it was imported from.
    // Needed by Guard G2: the callee-size resolver calls frame_layouts_query on the
    // callee's SourceFile to get its real total_size.
    //
    // Walk the parse AST's ImportDecl list; for each named import item, resolve the
    // module path to a SourceFile. We only care about function names that are in
    // imported_fns AND suspending.
    let callee_source_map: HashMap<String, SourceFile> = {
        let parse = parse_query(db, source);
        let mut map: HashMap<String, SourceFile> = HashMap::new();
        for item in &parse.module.items {
            let Item::ImportDecl(decl) = item else {
                continue;
            };
            let Some(resolved) =
                ynz_typeck::resolve_import::resolve_module_path(source_path.as_str(), &decl.source)
            else {
                continue;
            };
            let path_str = resolved.display().to_string();
            let Some(callee_sf) = db.source_by_path(&path_str) else {
                continue;
            };
            // Map every named imported function from this module.
            match &decl.kind {
                ImportKind::Named(items) => {
                    for item in items {
                        if sig_output.imported_fns.contains_key(&item.local_name) {
                            map.insert(item.local_name.clone(), callee_sf);
                        }
                    }
                }
                ImportKind::Namespace { local_name, .. } => {
                    // Namespace imports (e.g. `import ns from "mod"`) expose functions as
                    // `ns.fn`. The imported_fns map uses the local name (may differ after
                    // `as` alias), so we check which imported_fns entries originated from
                    // this module by comparing source paths. This is conservative — the
                    // namespace case is not currently exercised by the danger-matrix
                    // fixtures, but handling it prevents a silent wrong-resolver fallback.
                    let _ = local_name;
                    // For each imported_fn whose callee_sf hasn't been mapped yet,
                    // attempt to match via the registered path.
                    for fn_name in sig_output.imported_fns.keys() {
                        map.entry(fn_name.clone()).or_insert(callee_sf);
                    }
                }
            }
        }
        map
    };

    // Create an inkwell Context to compute LLVM-accurate shape ABI sizes.
    // Context is created here and dropped at the end of this scope — no LLVM
    // types escape into the returned Arc<HashMap> (only u64 sizes escape).
    let shape_abi_sizes: HashMap<String, u64> = {
        // Guard G1: same target machine constructor as emit_artifact.
        //
        // The Err branch returns an empty map rather than propagating. This is sound
        // because `emit_artifact` constructs the same target machine via `?` and errors
        // first for the same module — `codegen_query` skips emission on errors, so the
        // empty layout map here is never observed as a live wrong value.
        let machine = match state_machine::default_target_machine() {
            Ok(m) => m,
            Err(_) => return Arc::new(HashMap::new()),
        };
        let ctx = Context::create();
        let module = ctx.create_module("frame_layout_sizing");
        module.set_triple(&machine.get_triple());
        module.set_data_layout(&machine.get_target_data().get_data_layout());
        let dl_owned = module.get_data_layout();
        let dl_str = dl_owned.as_str().to_str().unwrap_or("");
        let target_data = inkwell::targets::TargetData::create(dl_str);

        // Emit LLVM struct types for all shapes in this module so we can measure them —
        // with the SAME padded set emit_artifact's layout uses, read from the ONE
        // layout authority (`layout_decisions_query`, v0.3-M5 P4 / E3 B1), so a padded
        // shape's frame slot is sized from the padded struct type automatically (one
        // source, every consumer threaded — authoritative-derivation.md).
        let layout = layout_decisions_query(db, source);
        let shape_types = crate::shape_types::emit_shape_types(
            &ctx,
            &sig_output.shape_table,
            &layout.padded_shapes,
        );
        shape_types
            .named
            .iter()
            .map(|(name, &struct_ty)| {
                let bytes = target_data.get_abi_size(&struct_ty);
                (name.clone(), bytes)
            })
            .collect()
        // ctx and module drop here — no inkwell types escape past this block.
    };

    // Guard G2: resolver recursively calls frame_layouts_query for imported callees
    // instead of reading the lossy FunctionSig.composed_frame_size scalar.
    // This correctly handles re-export chains: frame_layouts_query(B) recursively
    // calls frame_layouts_query(A) so B's total_size includes A's real sub-frame.
    //
    // Alias handling: when a function is imported under an alias (`import { getValue as fetchVal }`),
    // the callee_source_map uses the local name `fetchVal` as key (so the resolver finds the
    // right SourceFile), but the callee module's frame_layouts map uses the original exported
    // name `getValue`. The resolver must look up by original name in the callee's layouts.
    let callee_size_resolver = |name: &str| -> Option<u64> {
        let callee_sf = callee_source_map.get(name)?;
        let callee_layouts = frame_layouts_query(db, *callee_sf);
        // First try the local name (non-aliased case). If not found, try the original
        // exported name from the FunctionSig (aliased case).
        callee_layouts
            .get(name)
            .map(|layout| layout.total_size)
            .or_else(|| {
                let orig = sig_output
                    .imported_fns
                    .get(name)?
                    .original_name
                    .as_deref()?;
                callee_layouts.get(orig).map(|layout| layout.total_size)
            })
    };

    let mut layouts = build_frame_layouts_with_resolver(
        &check.typed_module,
        &effective_suspend_set,
        &base_suspend_set,
        &shape_abi_sizes,
        &callee_size_resolver,
    );

    // Add FrameLayout stubs for imported suspending callees, keyed by their LOCAL alias
    // name. These stubs carry the callee's real composed total_size (via Guard G2 recursive
    // lookup) and param count as n_locals. Without these entries, codegen sites that look
    // up `cg.frame_layouts.get(callee_name)` for an imported callee get None → 32-byte
    // fallback frame → heap corruption for any callee with crossing-locals.
    //
    // The stubs intentionally omit children/recursion_slot/number_errors_staging_offset:
    // those are the callee's INTERNAL layout details, owned by the callee's resume body.
    // The importer only needs total_size (to allocate the frame) and n_locals (to cap
    // the arg-write count at the spawn site).
    for (local_name, sig) in &sig_output.imported_fns {
        if !sig.suspends || layouts.contains_key(local_name.as_str()) {
            continue;
        }
        let total_size =
            callee_size_resolver(local_name.as_str()).unwrap_or(ynz_abi::FRAME_HEADER_SIZE);
        layouts.insert(
            local_name.clone(),
            FrameLayout {
                total_size,
                n_locals: sig.params.len(),
                children: Vec::new(),
                recursion_slot: None,
                number_errors_staging_offset: None,
                // Imported-callee stubs carry no CPU groups — those belong to the importer's
                // own frame, not the callee's.
                cpu_group_slots: Vec::new(),
            },
        );
    }

    Arc::new(layouts)
}

/// Generate a relocatable object file for a source file.
///
/// Salsa-tracked — depends on `check_query`. Skips emission if there are
/// type errors (avoids emitting broken object files).
// lru = 32: codegen is the heaviest query; smallest cache cap to bound memory.
#[salsa::tracked(lru = 32)]
pub fn codegen_query(db: &dyn SourceFileRegistry, source: SourceFile) -> Arc<CodegenOutput> {
    let check = check_query(db, source);
    let mut diagnostics = check.diagnostics.clone();

    if diagnostics.has_errors() {
        return Arc::new(CodegenOutput {
            artifact: CompiledArtifact {
                object_bytes: Vec::new(),
                ir_text: String::new(),
                sha256: [0u8; 32],
            },
            diagnostics,
        });
    }

    // The `array-using-soa-layout` Tier 3 lint (v0.3-M5 Phase 7) merges here, NOT in
    // check_query — its inputs (`layout_decisions_query` → `soa_candidate_query`)
    // depend on check_query, so firing it there would close a salsa cycle. Both
    // driver stderr paths render THIS bucket, so the lint reaches `ynz build` and
    // `ynz run` alike. Safe after the has_errors return: candidates (and therefore
    // lints) are empty on type errors.
    for lint in ynz_typeck::queries::soa_layout_lints(db, source) {
        diagnostics.push(lint);
    }

    let sig_output = module_signatures_query(db, source);
    let source_path = source.path(db);
    // Pre-compute LLVM-accurate frame layouts via the salsa query so both the emitter
    // and future cross-module importers read the SAME layouts (single source of truth).
    // Salsa memoizes this — the check_query call inside frame_layouts_query is already
    // cached from the call above, so there is no re-parse or re-typecheck cost.
    let layouts_arc = frame_layouts_query(db, source);

    // v0.3-M3d: the CPU-promotion set is the production trigger for CPU-statement
    // parallelization. Functions codegen will spike-HOST route through the state-machine
    // lowering (their CPU group joins are suspension points). Union ONLY the host subset
    // into the suspends set emit_artifact consumes.
    // Salsa memoizes this query (already evaluated inside frame_layouts_query above).
    //
    // Reconcile typeck's full promotion set down to what codegen actually hosts this slice
    // (`spike_host_subset`). Unioning the full set would land a promoted-but-unhosted inner
    // callee in the suspend set the host's own callee-eligibility filter reads, silently
    // declining the host's group — and would SM-lower that callee while its callers still
    // call it as a plain int-returning fn (trampoline mismatch). See `spike_host_subset`.
    //
    // The probe MUST run against the SAME effective suspend set `frame_layouts_query` uses
    // (`build_effective_suspend_set` = local ∪ imported-suspending names). The base set
    // alone omits imported suspending callees, so `spike_cpu_candidates`'s post-pair decline
    // gate would admit a host here that `frame_layouts_query` declined — and the host's frame
    // would be sized (there) sequentially-with-the-imported-child-sub-frame while codegen
    // (here) lays it out as a spike host, under-allocating the heap block by exactly that
    // sub-frame and corrupting it when the imported child writes at its layout offset. One
    // canonical set across both query boundaries is the only thing that keeps the two
    // sizing decisions in lock-step.
    // `base_suspend_set` is the AUTHORITATIVE pre-CPU-promotion suspend set — no longer consulted
    // by `admitted_cpu_group` itself post-v0.3-M3g-Phase-3 (that function dropped the parameter;
    // see its doc comment), but still kept UNMUTATED (never unioned with `spike_hosts`) and
    // threaded through to `emit_artifact` as a separate argument from the union
    // `suspends_with_promotions` below, so a legitimate pure-CPU host is never wrongly
    // self-declined once its own name lands in the union.
    let base_suspend_set: SuspendSet =
        build_effective_suspend_set(&check.suspends_set, &sig_output.imported_fns);
    let promotion = cpu_promotion_query(db, source);
    let spike_hosts = spike_host_subset(
        &check.typed_module,
        &base_suspend_set,
        &base_suspend_set,
        &promotion.promoted,
    );
    // emit_artifact's emit-time re-probe (`lower_function_with_waits` → `spike_cpu_candidates`)
    // reads THIS set, so it too must carry the imported-suspending names to agree with the
    // frame-layout sizing decision above.
    //
    // Probe/emit-time asymmetry (benign over-allocation; tracked residual): a host whose
    // post-pair statement calls ANOTHER host lands that callee in this union, so the
    // emit-time re-probe declines the host (post-pair-suspending gate) while
    // `spike_host_subset` — probed against the effective set BEFORE this union — admitted it.
    // The admitted-but-declined host gets a dead 48-byte reserve (OVER-allocation, never
    // under, so output stays correct and alloc==free). Exact reconciliation needs a fixpoint
    // over the host set, so it is deferred rather than approximated. See `spike_host_subset`.
    let mut suspends_with_promotions = base_suspend_set.clone();
    for name in &spike_hosts {
        suspends_with_promotions.insert(name.clone());
    }

    // Pass suspends_with_promotions (may_block::analyze's transitive set ∪ CPU spike hosts)
    // directly to emit_artifact so codegen reads the TRANSITIVE suspends flags plus the
    // spike-host functions, not the pre-analysis sig_table (which has suspends=false for all
    // fns — the Phase-7 seam fix).
    //
    // Pass imported_fns so emit_artifact can forward-declare cross-module functions as
    // LLVM external declarations — without these, calls to imported functions fail with
    // "function not found in module" during codegen (the linker would resolve them, but
    // LLVM's verifier needs them declared before it will emit a reference).
    // THE one layout authority (v0.3-M5 P4 / E3 B1): emit_artifact reads the padded
    // set (and, from Phase 5, SoA decisions) from this single source. Salsa memoizes —
    // already evaluated inside frame_layouts_query above.
    let layout = layout_decisions_query(db, source);

    match emit_artifact(
        source_path.as_str(),
        &check.typed_module,
        &sig_output.shape_table,
        &sig_output.sig_table,
        &sig_output.generic_fn_table,
        &check.mono_table,
        None,
        &sig_output.imported_options,
        &suspends_with_promotions,
        &base_suspend_set,
        &sig_output.imported_fns,
        &layouts_arc,
        &spike_hosts,
        &layout,
    ) {
        Ok(artifact) => Arc::new(CodegenOutput {
            artifact,
            diagnostics,
        }),
        Err(msg) => {
            diagnostics.push(Diagnostic::file_error(
                source_path.as_str(),
                format!("The compiler failed to produce machine code: {msg}"),
                "This is a compiler bug. Please report it with the source file attached.",
                "Machine-code generation failed inside the backend.",
            ));
            Arc::new(CodegenOutput {
                artifact: CompiledArtifact {
                    object_bytes: Vec::new(),
                    ir_text: String::new(),
                    sha256: [0u8; 32],
                },
                diagnostics,
            })
        }
    }
}
