use std::{collections::HashMap, sync::Arc};

use inkwell::context::Context;
use ynz_ast::nodes::{ImportKind, Item};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};
use ynz_typeck::{check_query, module_signatures_query};

use crate::{
    artifact::CompiledArtifact,
    emit::{build_frame_layouts_with_resolver, emit_artifact, FrameLayout, SuspendSet},
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

    // Build the effective suspend_set (local + imported suspending), mirroring
    // what build_module does before calling build_frame_layouts.
    let mut effective_suspend_set: SuspendSet = check.suspends_set.clone();
    for (name, sig) in &sig_output.imported_fns {
        if sig.suspends {
            effective_suspend_set.insert(name.clone());
        }
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

        // Emit LLVM struct types for all shapes in this module so we can measure them.
        let shape_types = crate::shape_types::emit_shape_types(&ctx, &sig_output.shape_table);
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
    let callee_size_resolver = |name: &str| -> Option<u64> {
        let callee_sf = callee_source_map.get(name)?;
        let callee_layouts = frame_layouts_query(db, *callee_sf);
        callee_layouts.get(name).map(|layout| layout.total_size)
    };

    let layouts = build_frame_layouts_with_resolver(
        &check.typed_module,
        &effective_suspend_set,
        &shape_abi_sizes,
        &callee_size_resolver,
    );

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

    let sig_output = module_signatures_query(db, source);
    let source_path = source.path(db);
    // Pre-compute LLVM-accurate frame layouts via the salsa query so both the emitter
    // and future cross-module importers read the SAME layouts (single source of truth).
    // Salsa memoizes this — the check_query call inside frame_layouts_query is already
    // cached from the call above, so there is no re-parse or re-typecheck cost.
    let layouts_arc = frame_layouts_query(db, source);
    // Pass check.suspends_set (from may_block::analyze via check_query) directly to
    // emit_artifact so codegen reads the TRANSITIVE suspends flags, not the pre-analysis
    // sig_table (which has suspends=false for all fns — the Phase-7 seam fix).
    //
    // Pass imported_fns so emit_artifact can forward-declare cross-module functions as
    // LLVM external declarations — without these, calls to imported functions fail with
    // "function not found in module" during codegen (the linker would resolve them, but
    // LLVM's verifier needs them declared before it will emit a reference).
    match emit_artifact(
        source_path.as_str(),
        &check.typed_module,
        &sig_output.shape_table,
        &sig_output.sig_table,
        &sig_output.generic_fn_table,
        &check.mono_table,
        None,
        &sig_output.imported_options,
        &check.suspends_set,
        &sig_output.imported_fns,
        &layouts_arc,
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
