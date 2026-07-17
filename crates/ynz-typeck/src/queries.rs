use std::{collections::HashSet, sync::Arc};

use ynz_ast::nodes::{ImportDecl, ImportKind, Item, Module};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, DiagnosticKind};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    check::{check, TypedModule},
    effective_ownership,
    exports::{collect_exports, ExportTable},
    generics::{GenericFnTable, GenericShapeTable, MonomorphizationTable},
    intrinsics::PrimitiveIntrinsicTable,
    may_block,
    options_table::collect_options,
    resolve_import::resolve_imports,
    shapes::{collect_generic_shapes, collect_shapes, ShapeTable},
    signatures::{collect_generic_signatures, collect_signatures, FunctionSig, SignatureTable},
};

/// Output of the signature pre-pass.
#[derive(Clone, Debug, PartialEq)]
pub struct SignatureOutput {
    pub sig_table: SignatureTable,
    pub shape_table: ShapeTable,
    pub generic_fn_table: GenericFnTable,
    pub generic_shape_table: GenericShapeTable,
    /// Imported function signatures visible in function bodies.
    pub imported_fns: std::collections::HashMap<String, FunctionSig>,
    /// Imported options types visible in function bodies (for options value expressions).
    pub imported_options: std::collections::HashMap<String, crate::options_table::OptionsEntry>,
    pub diagnostics: DiagnosticBucket,
}

impl SignatureOutput {
    pub fn sig_table(&self) -> &SignatureTable {
        &self.sig_table
    }
}

/// The output of the type-check pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckOutput {
    pub typed_module: TypedModule,
    pub mono_table: MonomorphizationTable,
    pub diagnostics: DiagnosticBucket,
    /// The set of function names that transitively reach a suspension point.
    ///
    /// Computed by `may_block::analyze` during `check_query`. Codegen reads this
    /// to determine which functions compile as state machines (Phase-7 seam).
    pub suspends_set: std::collections::HashSet<String>,
    /// The set of function names that transitively do real work (reach a loop or a
    /// recursion) — the v0.3-M3d auto-parallel worth-it proxy.
    ///
    /// Computed by `may_block::does_real_work_set` during `check_query`. The CPU
    /// promotion query reads it to gate which independent statement pairs spawn.
    /// Carried cross-module the same way `suspends_set` is (via the per-fn flag on
    /// the export table).
    pub does_real_work_set: std::collections::HashSet<String>,
    /// The transitive effective-ownership fixpoint (v0.3-M7 Phase 2, FRAGO 002).
    ///
    /// Codegen's `declare_function` consults this — never the raw AST ownership
    /// modifier — when emitting LLVM `readonly`/`noalias` parameter attributes
    /// (authoritative-derivation: consume the computed answer downstream). A bare
    /// parameter the body mutates is `Writes` here; defaulting it to `share` in
    /// codegen emitted a FALSE `readonly` the optimizer exploited into a silent
    /// miscompile (RED fixture `v0_3_m7_p1_bare_param_mutation.ynz`).
    pub effective_ownership: crate::effective_ownership::EffectiveOwnershipReport,
}

/// Cycle-initial placeholder returned by salsa when `module_signatures_query` is the
/// head of a circular-import cycle.
///
/// Salsa feeds this value back to other cycle participants as their provisional
/// view of the cycle-head module. An empty table is correct: the cycle cannot
/// resolve any real exports, and the `module_signatures_cycle_fn` will inject the
/// diagnostic on the next pass when salsa hands back the computed value.
fn module_signatures_cycle_initial(
    _db: &dyn SourceFileRegistry,
    _id: salsa::Id,
    _source: SourceFile,
) -> Arc<SignatureOutput> {
    Arc::new(SignatureOutput {
        sig_table: SignatureTable::empty(),
        shape_table: ShapeTable::empty(),
        generic_fn_table: GenericFnTable::default(),
        generic_shape_table: GenericShapeTable::default(),
        imported_fns: std::collections::HashMap::new(),
        imported_options: std::collections::HashMap::new(),
        diagnostics: DiagnosticBucket::new(),
    })
}

/// Cycle-recovery function called by salsa when `module_signatures_query` detects
/// that it is the head of a circular-import cycle.
///
/// Salsa has already run one provisional iteration; `value` is the result from that
/// pass. We inject a WHAT/WHAT-INSTEAD/WHY circular-import diagnostic into the
/// diagnostic bucket so `check_query` propagates it and the driver emits exit 1.
/// Returning `value` unchanged causes salsa to converge immediately (PartialEq
/// sees the same content on the next pass), so we avoid unbounded iteration.
fn module_signatures_cycle_fn(
    db: &dyn SourceFileRegistry,
    _cycle: &salsa::Cycle,
    _last_provisional: &Arc<SignatureOutput>,
    value: Arc<SignatureOutput>,
    source: SourceFile,
) -> Arc<SignatureOutput> {
    // Only inject the diagnostic on the first recovery call (iteration 0 of the
    // cycle-recovery phase). On subsequent iterations salsa checks PartialEq and
    // converges — but since we already injected the diagnostic in the first pass
    // the output is stable and this branch won't fire again.
    if value.diagnostics.is_empty() {
        let path = source.path(db);
        let span = ynz_diagnostics::SourceSpan::new(path.as_str(), 0, 0);
        let mut diags = value.diagnostics.clone();
        diags.push(Diagnostic::error(
            span,
            "Circular import: this module is part of a mutually-recursive import chain.",
            "Move the shared definitions to a third module that both can import without \
             creating a cycle.",
            "Circular imports cannot be resolved — the compiler cannot determine which \
             module to compile first. Extract the shared shapes, functions, or types into \
             a new file and import that file from both modules.",
        ));
        // Clone the output and replace the diagnostics bucket.
        let mut out = (*value).clone();
        out.diagnostics = diags;
        Arc::new(out)
    } else {
        value
    }
}

/// Pass 1: collect all shape declarations and function signatures from the module,
/// including symbols imported from other files.
///
/// Cross-file import resolution happens here so shape field type annotations
/// can reference imported shapes and options types.
// lru = 128: signature computation is moderate cost; keep more results cached.
#[salsa::tracked(lru = 128, cycle_fn = module_signatures_cycle_fn, cycle_initial = module_signatures_cycle_initial)]
pub fn module_signatures_query(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Arc<SignatureOutput> {
    let parse = parse_query(db, source);
    let mut diag_bucket = ynz_diagnostics::DiagnosticBucket::new();

    // Collect import declarations from this module.
    let import_decls: Vec<&ImportDecl> = parse
        .module
        .items
        .iter()
        .filter_map(|i| {
            if let Item::ImportDecl(d) = i {
                Some(d)
            } else {
                None
            }
        })
        .collect();

    // Resolve imports to get cross-file shapes, options, and functions.
    // Uses a passed-in visiting set to detect circular imports.
    let mut visiting: HashSet<std::path::PathBuf> = HashSet::new();
    let importer_path = source.path(db);
    let importer_path_str: &str = &importer_path;

    // Import resolution reads imported files from disk using the SAME salsa db.
    // Using the same db avoids "Cannot change database mid-query" panics from salsa.
    let imported = resolve_imports(
        &import_decls,
        importer_path_str,
        db,
        &mut visiting,
        &mut diag_bucket,
    );

    // Shapes first — function signatures need them for type resolution.
    // Pass imported shapes and options so field type annotations can reference them.
    let shape_table = collect_shapes(
        &parse.module,
        &imported.shapes,
        &imported.options,
        &mut diag_bucket,
    );
    let generic_shape_table = collect_generic_shapes(&parse.module, &mut diag_bucket);
    let sig_table = collect_signatures(&parse.module, &mut diag_bucket, &shape_table);
    let generic_fn_table =
        collect_generic_signatures(&parse.module, &mut diag_bucket, &shape_table);

    Arc::new(SignatureOutput {
        sig_table,
        shape_table,
        generic_fn_table,
        generic_shape_table,
        imported_fns: imported.functions,
        imported_options: imported.options.clone(),
        diagnostics: diag_bucket,
    })
}

/// Returns the symbols this file exports — used by the LSP auto-import code action.
///
/// Salsa-tracked so the result is re-used across auto-import lookups on unchanged files.
// lru = 128: cheap to compute; keep a broad cache for cross-file auto-import sweeps.
#[salsa::tracked(lru = 128)]
pub fn exports_query(db: &dyn SourceFileRegistry, source: SourceFile) -> Arc<ExportTable> {
    let parse = parse_query(db, source);
    let sig_output = module_signatures_query(db, source);
    let mut dummy = DiagnosticBucket::new();
    let options_table = collect_options(&parse.module, &mut dummy);
    Arc::new(collect_exports(
        &parse.module,
        &sig_output.shape_table,
        &options_table,
        &sig_output.sig_table,
    ))
}

/// Cycle-initial placeholder for `check_query` when the module is part of a circular
/// import chain.
///
/// An empty `CheckOutput` (no suspends, no diagnostics) is returned so that cycle
/// participants that call `check_query` (e.g. `load_export_table` resolving suspension
/// flags) get a safe zero-value rather than a salsa panic. The circular-import
/// diagnostic is injected by `module_signatures_cycle_fn` on `SignatureOutput`, which
/// `check_query` propagates on its own (non-cycle) invocation after the cycle head
/// resolves.
fn check_query_cycle_initial(
    _db: &dyn SourceFileRegistry,
    _id: salsa::Id,
    _source: SourceFile,
) -> Arc<CheckOutput> {
    Arc::new(CheckOutput {
        typed_module: TypedModule {
            module: Module {
                items: vec![],
                span: ynz_diagnostics::SourceSpan::new("", 0, 0),
            },
            expr_types: std::collections::HashMap::new(),
            background_arg_inferred_ownership: std::collections::HashMap::new(),
            cross_thread_padded_shapes: std::collections::HashSet::new(),
        },
        mono_table: crate::generics::MonomorphizationTable {
            entries: std::collections::BTreeMap::new(),
        },
        diagnostics: DiagnosticBucket::new(),
        suspends_set: std::collections::HashSet::new(),
        does_real_work_set: std::collections::HashSet::new(),
        effective_ownership: crate::effective_ownership::EffectiveOwnershipReport::empty(),
    })
}

/// Cycle-recovery function for `check_query`.
///
/// Returns the provisional `value` unchanged so salsa converges immediately.
/// The circular-import error is already injected via `module_signatures_cycle_fn`
/// into `SignatureOutput.diagnostics`, which `check_query` propagates through
/// `sig_output.diagnostics` on its own pass.
fn check_query_cycle_fn(
    _db: &dyn SourceFileRegistry,
    _cycle: &salsa::Cycle,
    _last_provisional: &Arc<CheckOutput>,
    value: Arc<CheckOutput>,
    _source: SourceFile,
) -> Arc<CheckOutput> {
    value
}

/// Pass 2: type-check all function bodies.
///
/// Depends on `module_signatures_query` for the signature table.
/// Depends on `parse_query` for the AST.
// lru = 64: typechecking is moderately expensive; smaller cap than parse/signatures.
#[salsa::tracked(lru = 64, cycle_fn = check_query_cycle_fn, cycle_initial = check_query_cycle_initial)]
pub fn check_query(db: &dyn SourceFileRegistry, source: SourceFile) -> Arc<CheckOutput> {
    let parse = parse_query(db, source);
    let sig_output = module_signatures_query(db, source);

    let mut all_diags = parse.diagnostics.clone();
    for d in sig_output.diagnostics.iter() {
        all_diags.push(d.clone());
    }

    // Merge imported functions into the local signature table so function bodies
    // can call imported functions by name.
    let mut merged_sig_table = sig_output.sig_table.clone();
    for (name, sig) in &sig_output.imported_fns {
        // Local declarations take priority — don't override with imported.
        merged_sig_table
            .fns
            .entry(name.clone())
            .or_insert_with(|| sig.clone());
    }

    // Run the transitive may-block analysis to populate `FunctionSig.suspends`.
    //
    // `imported_fn_names` lets the analysis distinguish known cross-module calls
    // (non-suspending leaves or known-suspending seeds) from unknown names
    // (typeck will report "not defined").
    //
    // `imported_suspending_names` is the subset of imported fns whose
    // `check_query.suspends_set` flags them as suspending — derived from the
    // `suspends` field on each `FunctionSig` in `imported_fns`, which was set by
    // `load_export_table` calling `check_query` on the imported module. A local fn
    // that calls one of these is seeded as suspending in the fixpoint.
    let imported_fn_names: HashSet<String> = sig_output.imported_fns.keys().cloned().collect();
    let imported_suspending_names: HashSet<String> = sig_output
        .imported_fns
        .iter()
        .filter_map(|(name, sig)| {
            if sig.suspends {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    let may_block_result = may_block::analyze(
        &parse.module,
        &imported_fn_names,
        &imported_suspending_names,
    );

    // Update each merged sig's `suspends` flag using the UNION of:
    // (a) the imported fn's own preserved `suspends` (set by load_export_table),
    // (b) the local fixpoint result for locally-defined fns.
    //
    // Without the union, overwriting with the local-only fixpoint would clear
    // `suspends=true` on imported fns — those fns' bodies aren't in this unit so
    // the local analysis can never mark them.
    for (name, sig) in merged_sig_table.fns.iter_mut() {
        let local_suspends = may_block_result.suspends.contains(name.as_str());
        // Preserve the imported fn's own `suspends` flag (already set by
        // load_export_table via check_query on the imported module). For locally-
        // defined fns, sig.suspends starts false and is set here.
        sig.suspends = sig.suspends || local_suspends;
    }

    // v0.3-M3d worth-it proxy: the `does_real_work` fixpoint (loop or recursion,
    // transitively) drives which CPU statement pairs the promotion query spawns.
    // Seeds from local loop/recursion AND from imported fns already known to do real
    // work (their flag was set by load_export_table via check_query on the importee),
    // exactly mirroring the `imported_suspending_names` seed for `suspends`.
    let imported_real_work_names: HashSet<String> = sig_output
        .imported_fns
        .iter()
        .filter_map(|(name, sig)| {
            if sig.does_real_work {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    let does_real_work_result =
        may_block::does_real_work_set(&parse.module, &imported_fn_names, &imported_real_work_names);
    for (name, sig) in merged_sig_table.fns.iter_mut() {
        let local_real_work = does_real_work_result.contains(name.as_str());
        // Union with the imported fn's own flag (set by load_export_table). For
        // locally-defined fns, sig.does_real_work starts false and is set here.
        sig.does_real_work = sig.does_real_work || local_real_work;
    }

    // Reject non-self mutual recursion among suspending functions.
    //
    // `SpawnStateFnFuture::Drop` walks the recursion chain assuming every frame has
    // the same size and recursion-slot offset (self-recursion: all frames are the
    // same function's layout). A mutual cycle (ping → pong → ping) has mixed layouts
    // → heap corruption on cancellation. Detecting SCCs of size ≥ 2 here emits a
    // clean teaching error instead of corrupting heap at runtime.
    //
    // Self-recursion (SCC of size 1) is NOT a cycle in this sense — it stays supported.
    // Per-frame layout metadata to support mixed cycles ships in v0.3-M3.
    {
        // Build a name→span map for each local function (for error location).
        let fn_spans: std::collections::HashMap<String, ynz_diagnostics::SourceSpan> = parse
            .module
            .items
            .iter()
            .filter_map(|item| {
                if let Item::Function(f) = item {
                    Some((f.name.clone(), f.name_span.clone()))
                } else {
                    None
                }
            })
            .collect();

        let cycles = may_block::find_mutual_suspension_cycles(
            &parse.module,
            &imported_fn_names,
            &may_block_result.suspends,
        );
        for cycle in cycles {
            let members_display = cycle.members.join("`, `");
            // Emit one diagnostic per cycle member so each function's error points to
            // its own declaration (reduces hunting across the file).
            for member in &cycle.members {
                if let Some(span) = fn_spans.get(member) {
                    all_diags.push(Diagnostic::error(
                        span.clone(),
                        format!(
                            "`{member}` is part of a mutually-recursive suspending cycle \
                             with `{members_display}`."
                        ),
                        "Restructure so the recursion is self-recursive (a single function \
                         calling itself — self-recursion is supported), or remove the `wait` \
                         call from the cycle so it is no longer suspending."
                            .to_string(),
                        "Self-recursive suspending functions work correctly. Mutually-recursive \
                         suspending cycles — where two or more DIFFERENT functions call each other \
                         and all suspend — are rare in practice and can always be restructured: \
                         extract the shared logic into a helper, make one of the functions \
                         non-suspending, or use a self-recursive form instead.",
                    ));
                }
            }
        }
    }

    let (typed, mono_table, check_diags, referenced_names) = check(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &PrimitiveIntrinsicTable::m6().with_m2_internals(),
        &sig_output.imported_options,
    );
    for d in check_diags.into_iter() {
        all_diags.push(d);
    }

    // Emit UnusedImport warnings for named imports whose local name was never
    // resolved during the check pass. Namespace imports (`import ns from "..."`)
    // are excluded — their dotted references (`ns.Shape`) require separate tracking
    // that is not yet implemented.
    for item in &parse.module.items {
        let Item::ImportDecl(decl) = item else {
            continue;
        };
        let ImportKind::Named(import_items) = &decl.kind else {
            continue;
        };
        for import_item in import_items {
            if !referenced_names.contains(&import_item.local_name) {
                all_diags.push(
                    Diagnostic::warning(
                        import_item.local_name_span.clone(),
                        format!("`{}` is imported but never used.", import_item.local_name),
                        format!(
                            "Remove `{}` from the import list, or use it somewhere in this file.",
                            import_item.local_name
                        ),
                        "Unused imports add noise — every import signals to readers that this file depends on that symbol.",
                    )
                    .with_kind(DiagnosticKind::UnusedImport {
                        name: import_item.local_name.clone(),
                    }),
                );
            }
        }
    }

    // Transitive effective-ownership fixpoint (v0.3-M3b Phase 4). Classifies every
    // parameter as `Reads`/`Unknown`/`Writes` across the whole local call graph. One
    // consumer reads it:
    //   - Part 2 below: reject `share` params that are written transitively (the
    //     `design/concurrency.md` line 651 no-escalation rule, extended past the direct
    //     cases `check.rs` already catches).
    //
    // Codegen does NOT consume this fixpoint. The auto-parallel write-effect decision uses
    // the type-based conservative floor in `ynz-codegen/src/independence.rs` (any mutable-heap
    // arg is a potential write, Golden Rule 5 > Rule 10) — sound by construction, with no
    // name-based classifier to drift out of sync with this fixpoint.
    //
    // `declared_writes` seeds the fixpoint with each function's explicit `lend`/`give`
    // positions (local AND imported — an imported declared-write position is a definite
    // write even without a body). `imported_fn_names` marks the cross-module boundary so a
    // flow into an imported callee at a non-declared position resolves to `Unknown` (sound
    // conservative), never a spurious `Reads`.
    let mut declared_writes = effective_ownership::declared_write_positions(&parse.module);
    for (name, sig) in &sig_output.imported_fns {
        // Local definitions already have their declared writes from the module walk.
        if declared_writes.contains_key(name) {
            continue;
        }
        let mut set = std::collections::HashSet::new();
        for (i, ownership) in sig.param_ownerships.iter().enumerate() {
            if matches!(
                ownership,
                Some(ynz_ast::nodes::OwnershipModifier::Lend)
                    | Some(ynz_ast::nodes::OwnershipModifier::Give)
            ) {
                set.insert(i);
            }
        }
        declared_writes.insert(name.clone(), set);
    }
    let effective_ownership_report =
        effective_ownership::analyze(&parse.module, &declared_writes, &imported_fn_names);

    // Part 2 — reject `share` params written TRANSITIVELY through a callee (full
    // `design/concurrency.md` line 651 enforcement). The DIRECT cases (a `share` body that
    // field-assigns the param, or passes it to an explicit `lend`/`give` position) are
    // already rejected by `check.rs` with a precise span; this catches the transitive case
    // those checks miss (a bare callee that mutates → infers `lend` → the declared modifier
    // the direct check inspects is `None`, not `lend`). `Unknown` flows are NOT errors —
    // benefit of the doubt; the independence side keeps soundness by sequentializing them.
    let share_violations = effective_ownership::find_transitive_share_violations(
        &parse.module,
        &effective_ownership_report,
        &declared_writes,
        &imported_fn_names,
    );
    for v in share_violations {
        let type_name =
            crate::types::type_name(&sig_output.shape_table.resolve_ast_type(&v.param_type));
        all_diags.push(Diagnostic::error(
            v.span,
            format!(
                "`{param}` is declared `share` (read-only) but it is modified through `{callee}`.",
                param = v.param_name,
                callee = v.callee_name,
            ),
            format!(
                "Change the parameter to `lend {param}: {ty}` — the function (directly or through a call) writes to it.",
                param = v.param_name,
                ty = type_name,
            ),
            format!(
                "A `share` parameter is a read-only borrow; the caller trusts the value is unchanged. Because `{callee}` modifies `{param}`, this function lends it onward — declare `lend` so the write is visible at every call site.",
                callee = v.callee_name,
                param = v.param_name,
            ),
        ));
    }

    // v0.3-M7 Phase 2 (FRAGO 002, Patrick-directed 2026-07-16) — reject calls that pass
    // the same value (or overlapping pieces of one value) into two parameter positions
    // where at least one position can modify it. `lend` means exclusive mutable access
    // for the duration of the call (Golden Rule 5: wrong = caught at compile time).
    // Typeck previously accepted `relay(h, h)` against `(share, lend)` while codegen
    // claimed LLVM `noalias` on both parameters — a false claim the optimizer exploits
    // into a silent miscompile (RED fixture `v0_3_m7_p1_share_lend_alias.ynz`).
    let aliasing_violations = effective_ownership::find_aliasing_call_violations(
        &parse.module,
        &effective_ownership_report,
        &sig_output.sig_table.fns,
        &sig_output.imported_fns,
    );
    for v in aliasing_violations {
        let what = if v.first_path == v.second_path {
            format!(
                "`{path}` is passed to `{callee}` twice in the same call — once as {d1} and once as {d2}.",
                path = v.first_path,
                callee = v.callee_name,
                d1 = v.first_kind.describe(),
                d2 = v.second_kind.describe(),
            )
        } else {
            format!(
                "`{longer}` is part of `{shorter}`, and both are passed to `{callee}` in the same call — one as {d1}, the other as {d2}.",
                longer = if v.first_path.len() >= v.second_path.len() { &v.first_path } else { &v.second_path },
                shorter = if v.first_path.len() >= v.second_path.len() { &v.second_path } else { &v.first_path },
                callee = v.callee_name,
                d1 = v.first_kind.describe(),
                d2 = v.second_kind.describe(),
            )
        };
        all_diags.push(Diagnostic::error(
            v.span,
            what,
            format!(
                "Give one of the two arguments its own copy — call `.copy()` on it — or restructure so `{path}` reaches `{callee}` only once.",
                path = v.first_path,
                callee = v.callee_name,
            ),
            "A parameter that modifies or takes over a value needs to be the only way that value is reached during the call. Because both arguments lead to the same value, a change through one would show up through the other mid-call — the compiler builds fast code on the promise that this cannot happen, so it reports the conflict here instead of letting the program's results become unpredictable.",
        ));
    }

    // Export the suspends set so codegen can read it directly instead of deriving
    // it from sig_table (which is from module_signatures_query, pre-analysis).
    let suspends_set: std::collections::HashSet<String> = may_block_result
        .suspends
        .iter()
        .map(|s| s.to_string())
        .collect();

    Arc::new(CheckOutput {
        typed_module: typed,
        mono_table,
        diagnostics: all_diags,
        suspends_set,
        does_real_work_set: does_real_work_result,
        effective_ownership: effective_ownership_report,
    })
}

/// The result of the CPU-statement promotion analysis.
///
/// `promoted` is the set of NON-suspending functions that become state machines
/// because they contain at least one CPU-parallelizable statement group AND survive
/// the guard-probe transitive rollback. This is the extension to the effective
/// suspend set that v0.3-M3d adds.
///
/// **Phase 2 ships this computed but UNCONSUMED**: codegen still reads only
/// `CheckOutput::suspends_set` (the M3b I/O set), so the binary is byte-identical to
/// pre-M3d. Phase 3 wires codegen to union `promoted` into the SM-lowering set and
/// emit the CPU group spawn/join — the feature goes live there.
#[derive(Clone, Debug, PartialEq)]
pub struct PromotionOutput {
    /// Functions promoted to state machines for CPU parallelization (post-rollback).
    pub promoted: std::collections::HashSet<String>,
}

/// Cycle-initial placeholder for `cpu_promotion_query` in a circular-import chain.
fn cpu_promotion_cycle_initial(
    _db: &dyn SourceFileRegistry,
    _id: salsa::Id,
    _source: SourceFile,
) -> Arc<PromotionOutput> {
    Arc::new(PromotionOutput {
        promoted: std::collections::HashSet::new(),
    })
}

/// Cycle-recovery for `cpu_promotion_query` — return the provisional value so salsa
/// converges immediately (the circular-import diagnostic is raised by the signature
/// query; promotion is a non-error optimization that simply yields no promotions in
/// a broken cycle).
fn cpu_promotion_cycle_fn(
    _db: &dyn SourceFileRegistry,
    _cycle: &salsa::Cycle,
    _last_provisional: &Arc<PromotionOutput>,
    value: Arc<PromotionOutput>,
    _source: SourceFile,
) -> Arc<PromotionOutput> {
    value
}

/// The single predicate for the `--no-auto-parallel` kill switch.
///
/// `main.rs` sets `YNZ_NO_AUTO_PARALLEL="1"` when the user passes `--no-auto-parallel`,
/// threading the flag across the salsa barrier via the environment. BOTH the typeck
/// promotion query and the codegen lowering path read the switch through this one
/// function so the promotion decision and the binary can never disagree on what counts
/// as "off" (any value vs. exactly `"1"`). Codegen calls
/// `ynz_typeck::no_auto_parallel_env()` rather than re-reading the var itself.
pub fn no_auto_parallel_env() -> bool {
    std::env::var("YNZ_NO_AUTO_PARALLEL").is_ok_and(|v| v == "1")
}

/// Read the harness-only `YNZ_SOA_FORCE` layout override (recorded decision D8).
///
/// `"soa"` / `"aos"` parse to the matching [`crate::soa::SoaForce`]; any other value
/// (or unset) is ignored. NEVER user-facing — no CLI flag sets it; it exists solely
/// for the Phase 6 SIZE_THRESHOLD calibration harness to pin each compiled workload
/// to one layout. Read ONLY at `soa_candidate_query` entry (one read site, same
/// discipline as [`no_auto_parallel_env`]) and threaded explicitly into the pure
/// core. Precedence: kernel mode and `YNZ_NO_AUTO_PARALLEL` outrank it.
pub fn soa_force_env() -> Option<crate::soa::SoaForce> {
    match std::env::var("YNZ_SOA_FORCE").as_deref() {
        Ok("soa") => Some(crate::soa::SoaForce::Soa),
        Ok("aos") => Some(crate::soa::SoaForce::Aos),
        _ => None,
    }
}

/// Compute which functions get CPU-statement promotion for this module.
///
/// Salsa-tracked (incremental-safe; no global mutable state). Depends on
/// `module_signatures_query` (sig table + shape table + imported fns) and
/// `check_query` (the `does_real_work` flags, the suspends set, and the `expr_types`
/// the guard-probe reads).
///
/// Gating (Phase 2 Step 6) is at the query entry: `--no-auto-parallel`
/// (read via [`no_auto_parallel_env`], the SAME predicate codegen reads) and kernel mode
/// both disable promotion entirely (kernel mode has no scheduler to spawn onto; the
/// production check path is never kernel today, so this query passes `false`). The
/// pure core [`compute_cpu_promotions`] takes both flags explicitly so unit tests can
/// drive the decline paths deterministically.
///
/// Time: O(C·F²·E)  Space: O(F) where C = promotion candidates, F = function count,
/// E = call edges. The merge loops (sig-table flag application, imported-name
/// collection) are each O(F) and dominated by the delegated `compute_cpu_promotions`.
// lru = 64: same cost class as check_query; both ride the call-graph walk.
#[salsa::tracked(lru = 64, cycle_fn = cpu_promotion_cycle_fn, cycle_initial = cpu_promotion_cycle_initial)]
pub fn cpu_promotion_query(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Arc<PromotionOutput> {
    let no_auto_parallel = no_auto_parallel_env();
    // The production check path is never kernel mode today (`--kernel` is wired only
    // through the test-only `check_with_kernel_mode`); when it lands, this reads the
    // same kernel signal check_query uses. Promotion declines under kernel mode.
    let kernel_mode = false;

    let parse = parse_query(db, source);
    let sig_output = module_signatures_query(db, source);
    let check_out = check_query(db, source);

    // Build the merged sig table (local + imported) with the analysis-populated
    // `suspends` / `does_real_work` flags, exactly as the check pass sees it.
    let mut merged_sig_table = sig_output.sig_table.clone();
    for (name, sig) in &sig_output.imported_fns {
        merged_sig_table
            .fns
            .entry(name.clone())
            .or_insert_with(|| sig.clone());
    }
    for (name, sig) in merged_sig_table.fns.iter_mut() {
        if check_out.suspends_set.contains(name.as_str()) {
            sig.suspends = true;
        }
        if check_out.does_real_work_set.contains(name.as_str()) {
            sig.does_real_work = true;
        }
    }

    let imported_fn_names: std::collections::HashSet<String> =
        sig_output.imported_fns.keys().cloned().collect();
    let imported_suspending_names: std::collections::HashSet<String> = sig_output
        .imported_fns
        .iter()
        .filter(|(_, sig)| sig.suspends)
        .map(|(name, _)| name.clone())
        .collect();

    let promoted = compute_cpu_promotions(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.imported_fns,
        &check_out.suspends_set,
        &imported_fn_names,
        &imported_suspending_names,
        &check_out.typed_module.expr_types,
        kernel_mode,
        no_auto_parallel,
    );

    Arc::new(PromotionOutput { promoted })
}

/// Cycle-initial placeholder for `soa_candidate_query` on circular import chains:
/// the empty candidate list (declining is always safe; the circular-import error is
/// injected by `module_signatures_cycle_fn` and surfaces through `check_query`).
fn soa_candidate_cycle_initial(
    _db: &dyn SourceFileRegistry,
    _id: salsa::Id,
    _source: SourceFile,
) -> Arc<Vec<crate::soa::SoaCandidate>> {
    Arc::new(Vec::new())
}

fn soa_candidate_cycle_fn(
    _db: &dyn SourceFileRegistry,
    _cycle: &salsa::Cycle,
    _last_provisional: &Arc<Vec<crate::soa::SoaCandidate>>,
    value: Arc<Vec<crate::soa::SoaCandidate>>,
    _source: SourceFile,
) -> Arc<Vec<crate::soa::SoaCandidate>> {
    value
}

/// Compute the SoA candidate list for this module (v0.3-M5 Phase 4 step 1).
///
/// Salsa-tracked (incremental-safe; no global mutable state). Depends on
/// `check_query` (the `expr_types` type oracle + diagnostics) and
/// `module_signatures_query` (the shape table for param/lend-self resolution).
///
/// Gating is at the query entry: `--no-auto-parallel` (read via
/// [`no_auto_parallel_env`], the SAME predicate codegen reads — D1) and kernel mode
/// both disable SoA analysis entirely; the harness-only `YNZ_SOA_FORCE` override
/// (read via [`soa_force_env`], D8) is subordinate to both. The pure core
/// [`crate::soa::analyze`] takes the flags explicitly so unit tests can drive the
/// decline paths deterministically.
///
/// Time: O(F · B)  Space: O(bindings) — one body walk per function.
// lru = 64: same cost class as check_query; both ride the body walk.
#[salsa::tracked(lru = 64, cycle_fn = soa_candidate_cycle_fn, cycle_initial = soa_candidate_cycle_initial)]
pub fn soa_candidate_query(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Arc<Vec<crate::soa::SoaCandidate>> {
    let no_auto_parallel = no_auto_parallel_env();
    // The production check path is never kernel mode today (`--kernel` is wired only
    // through the test-only `check_with_kernel_mode`); when it lands, this reads the
    // same kernel signal check_query uses. SoA analysis declines under kernel mode.
    let kernel_mode = false;

    let check_out = check_query(db, source);
    if check_out.diagnostics.has_errors() {
        // Codegen is skipped on type errors — no meaningful layout to analyze
        // (the frame_layouts_query posture).
        return Arc::new(Vec::new());
    }
    let sig_output = module_signatures_query(db, source);

    Arc::new(crate::soa::analyze(
        &check_out.typed_module,
        &sig_output.shape_table,
        kernel_mode,
        no_auto_parallel,
        soa_force_env(),
    ))
}

/// Cycle-initial placeholder for `layout_decisions_query` on circular import
/// chains: an empty authority (no padded shapes, no arrays) — the circular-import
/// error is injected by `module_signatures_cycle_fn` and surfaces through
/// `check_query`.
fn layout_decisions_cycle_initial(
    _db: &dyn SourceFileRegistry,
    _id: salsa::Id,
    _source: SourceFile,
) -> Arc<crate::soa::LayoutDecisions> {
    Arc::new(crate::soa::LayoutDecisions {
        padded_shapes: std::collections::HashSet::new(),
        arrays: Vec::new(),
    })
}

fn layout_decisions_cycle_fn(
    _db: &dyn SourceFileRegistry,
    _cycle: &salsa::Cycle,
    _last_provisional: &Arc<crate::soa::LayoutDecisions>,
    value: Arc<crate::soa::LayoutDecisions>,
    _source: SourceFile,
) -> Arc<crate::soa::LayoutDecisions> {
    value
}

/// Resolve THE one authoritative layout-decision source (v0.3-M5 Phase 4 step 2,
/// E3 B1). Delegates to [`crate::soa::resolve_layout`] — D11 precedence: padding
/// wins; a cross-thread-padded shape's arrays are never SoA'd.
///
/// Every layout consumer — codegen's shape-type emission (Pass 0), the deep
/// padded-alloca alignment read, and `frame_layouts_query`'s sizing pass — reads
/// `LayoutDecisions` from THIS query; a direct read of
/// `TypedModule::cross_thread_padded_shapes` outside this authority is the E3
/// twin-derivation corpse (authoritative-derivation.md).
///
/// `padded_shapes` is cloned UNCONDITIONALLY — even when the module has type
/// errors — matching the pre-authority consumers' unconditional
/// `typed.cross_thread_padded_shapes` reads, so the re-threading stays
/// byte-identical. `arrays` rides `soa_candidate_query` (already empty on
/// errors and under the kernel / `--no-auto-parallel` gates).
///
/// Phase 4 exit criterion: codegen consumes ONLY `padded_shapes`; ZERO codegen
/// consumers of `arrays` exist until Phase 5's SoA lowering.
///
/// Time: O(C · F) resolve on top of its input queries.  Space: O(C · F + P).
// lru = 64: same cost class as its inputs; one resolve pass per module.
#[salsa::tracked(lru = 64, cycle_fn = layout_decisions_cycle_fn, cycle_initial = layout_decisions_cycle_initial)]
pub fn layout_decisions_query(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Arc<crate::soa::LayoutDecisions> {
    let check_out = check_query(db, source);
    let candidates = soa_candidate_query(db, source);
    let sig_output = module_signatures_query(db, source);

    Arc::new(crate::soa::resolve_layout(
        &check_out.typed_module.cross_thread_padded_shapes,
        &candidates,
        &sig_output.shape_table,
    ))
}

/// The `array-using-soa-layout` lint diagnostics for this module (v0.3-M5 Phase 7
/// step 2) — a thin NON-salsa wrapper over the two memoized inputs.
///
/// The lint cannot fire inside `check_query`: `layout_decisions_query` →
/// `soa_candidate_query` → `check_query`, so pushing it there would close a salsa
/// cycle — and re-running the SoA analysis inside `check_query` instead would be the
/// E3 twin-derivation corpse (authoritative-derivation.md). So the three diagnostic
/// consumers that today read only `check_query` merge THIS wrapper's output on top:
/// `ynz-codegen::codegen_query` (driver stderr — both `ynz build` paths render its
/// bucket), the LSP's `run_and_publish_diagnostics`, and the driver's `--json`
/// `collect_diagnostics`. Both inputs are salsa-memoized and the pairing walk is
/// trivial, so no tracked query (and no cycle_fn boilerplate) is needed here.
///
/// Empty on type errors and under the kernel / `--no-auto-parallel` gates —
/// `soa_candidate_query` (and therefore `arrays`) is already empty there.
///
/// Time: O(A · C) on memoized inputs.  Space: O(A).
pub fn soa_layout_lints(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
) -> Vec<ynz_diagnostics::Diagnostic> {
    let decisions = layout_decisions_query(db, source);
    let candidates = soa_candidate_query(db, source);
    crate::soa::layout_lints(&decisions, &candidates)
}

/// Pure core of the CPU-promotion analysis (Decision Record item 8b/8c).
///
/// 1. **Candidate identification** (bottom-up, deterministic): a non-cyclic function whose body
///    contains at least one CPU-parallelizable group (a `Parallel` group of ≥2 members with ≥1
///    CPU member) is a promotion candidate — **v0.3-M3g Phase 2:** this now includes hosts that
///    already suspend (`base_suspends`, e.g. a function with its own `wait` or a suspending
///    callee) so a co-resident CPU group can be identified as a candidate too. This ships
///    computed but STRUCTURALLY INERT for the codegen fire site: `admitted_cpu_group`'s Phase-2
///    temporary co-resident-suspension decline (`cpu_admission.rs`) still blocks a
///    `base_suspends` host's top-level (and, pre-existing, nested) group from ever firing until
///    Phase 3's admission flip — this phase only makes the TYPECK-side `promoted` set correctly
///    inclusive (a prerequisite for the Phase 3 fusion and the E6 guard-probe extension below).
/// 2. **Transitive-SM closure**: a promoted function becomes a state machine, so the
///    closure is `base_suspends ∪ closure(promoted)` — every caller of a promoted
///    function becomes SM too.
/// 3. **Guard-probe transitive rollback**: probe EVERY newly-SM function (the closure
///    minus the base suspends) — **plus, v0.3-M3g Phase 2 (E6), every `base_suspends` host that
///    is ITSELF a direct promotion candidate** (its own CPU join is a NEW suspension point the
///    pre-promotion `suspends_set` never accounted for) — against the suspension guards via
///    `suspension_guards_fire_for_fn`. If a guard fires on a function `F`, remove
///    from the candidate set every promotion in `F`'s call-closure (any of them being
///    SM forces `F` to be SM). Recompute and re-probe until no guard fires — monotone
///    downward over a finite set, so it converges.
///
/// Decline (empty result) when `--no-auto-parallel` or kernel mode is set.
///
/// Time: O(C · F² · E) worst case (C = candidates, each rollback iteration re-runs
/// the closure + probe). Space: O(F + E).
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_cpu_promotions(
    module: &Module,
    sig_table: &SignatureTable,
    shape_table: &ShapeTable,
    imported_fns: &std::collections::HashMap<String, FunctionSig>,
    base_suspends: &std::collections::HashSet<String>,
    imported_fn_names: &std::collections::HashSet<String>,
    imported_suspending_names: &std::collections::HashSet<String>,
    expr_types: &std::collections::HashMap<(usize, usize), crate::types::Type>,
    kernel_mode: bool,
    no_auto_parallel: bool,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    // Step 6 gating — promotion declines entirely under either flag.
    if no_auto_parallel || kernel_mode {
        return HashSet::new();
    }

    let does_real_work: HashSet<String> = sig_table
        .fns
        .iter()
        .filter(|(_, sig)| sig.does_real_work)
        .map(|(name, _)| name.clone())
        .collect();

    // Functions in call cycles are never promoted (mutual-suspension prevention;
    // self-recursion is allowed by the spawn ABI but a self-recursive call is never a
    // CPU group member, so a self-only-recursive function with no other CPU group is
    // simply not a candidate). `find_mutual_suspension_cycles` over the FULL call
    // graph (not restricted to suspends) gives the mutually-recursive members.
    let all_fn_names: HashSet<String> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(f) => Some(f.name.clone()),
            _ => None,
        })
        .collect();
    let cyclic_members: HashSet<String> =
        crate::may_block::find_mutual_suspension_cycles(module, imported_fn_names, &all_fn_names)
            .into_iter()
            .flat_map(|c| c.members)
            .collect();

    // Step 1 — candidate identification, in deterministic (sorted) order.
    let cpu = crate::independence::CpuCandidacy {
        does_real_work: &does_real_work,
        enclosing_fn: "", // set per-function below
    };
    let mut fn_decls: Vec<&ynz_ast::nodes::FunctionDecl> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    fn_decls.sort_by(|a, b| a.name.cmp(&b.name));

    let mut candidates: HashSet<String> = HashSet::new();
    for f in &fn_decls {
        // A cyclic function is excluded (mutual-suspension prevention — see below). A
        // `base_suspends` host (already a state machine for its OWN wait/suspending-callee
        // reasons) is v0.3-M3g Phase 2's extension: it can STILL be a candidate when it hosts
        // its own CPU group, because promoting it costs nothing extra at the typeck layer (it
        // was already going to be a state machine) and the guard-probe below (E6) still checks
        // its own CPU-join crossings before admitting it into `candidates`. This lift is LIVE
        // at codegen: the v0.3-M3g Phase 3 admission flip removed the co-resident-suspension
        // decline in BOTH of `admitted_cpu_group`'s arms (top-level and nested — see
        // cpu_admission.rs), so a `base_suspends` host's admitted group DOES fire.
        if cyclic_members.contains(&f.name) {
            continue;
        }
        let cpu_here = crate::independence::CpuCandidacy {
            does_real_work: cpu.does_real_work,
            enclosing_fn: &f.name,
        };
        if function_has_cpu_group(
            &f.body.stmts,
            base_suspends,
            sig_table,
            imported_fns,
            &cpu_here,
        ) {
            candidates.insert(f.name.clone());
        }
    }

    // Steps 2 + 3 — transitive-SM closure + guard-probe rollback to fixpoint.
    let union_aliases = crate::check::collect_union_aliases(module, shape_table);
    loop {
        if candidates.is_empty() {
            break;
        }

        // Transitive-SM closure of the current candidate set.
        let closure = crate::may_block::suspends_with_extra_seeds(
            module,
            imported_fn_names,
            imported_suspending_names,
            &candidates,
        );

        // The suspension points INSIDE a promoted function are its OWN CPU-group-member
        // calls (the spawn→join is where the SM suspends). The guard probe's crossing-
        // local detection keys off calls to names in `suspending_fns`, so a candidate's
        // CPU callees must be treated as suspending WHEN PROBING THAT CANDIDATE —
        // otherwise a local crossing the CPU join is invisible and a guard that SHOULD
        // fire (e.g. a nested-shape value held across the join) is missed.
        //
        // The seed is PER-CANDIDATE: a non-candidate caller in the SM closure that calls
        // the same CPU callee (e.g. `let x = fib(10) + 1`) is NOT promoted, so that call
        // is an ordinary call in its body, not a suspension point. Seeding it globally
        // would fire a guard on the non-candidate and roll back the sound candidate it
        // transitively calls — a false negative. So we key the callee set by candidate
        // name and only augment the probe's suspending set when probing that candidate.
        let mut cpu_callees_by_candidate: std::collections::HashMap<String, HashSet<String>> =
            std::collections::HashMap::new();
        for f in &fn_decls {
            if !candidates.contains(&f.name) {
                continue;
            }
            let cpu_here = crate::independence::CpuCandidacy {
                does_real_work: &does_real_work,
                enclosing_fn: &f.name,
            };
            let mut callees: HashSet<String> = HashSet::new();
            collect_cpu_callees(
                &f.body.stmts,
                base_suspends,
                sig_table,
                imported_fns,
                &cpu_here,
                &mut callees,
            );
            cpu_callees_by_candidate.insert(f.name.clone(), callees);
        }

        // Probe every NEWLY-SM function (closure minus base suspends) — PLUS (v0.3-M3g Phase 2,
        // E6) every `base_suspends` host that is ITSELF a direct promotion candidate. A guard
        // firing on `F` means `F` cannot be a state machine — roll back every candidate in `F`'s
        // call-closure (which, for a directly-candidate `base_suspends` host, includes `F`
        // itself, since `candidates_reachable_from` starts its DFS at `f.name`).
        //
        // WHY the extension: the standard "newly-SM" loop deliberately EXCLUDES `base_suspends`
        // hosts, because normally they were already state machines for reasons unrelated to CPU
        // promotion (their own `wait`/suspending callee), and nothing new needs re-checking.
        // That reasoning breaks once a `base_suspends` host can itself be a CPU-promotion
        // candidate (the `:761`-era skip lifted above): its OWN CPU join is a suspension point
        // the pre-promotion `suspends_set` never accounted for, so a guard-tripping local
        // crossing THAT join (a nested shape, an unsupported crossing type, …) must still be
        // caught here — the guard-probe is the only mechanism that keeps the TYPECK-side
        // `promoted`/`cpu_promoted` set (and everything that reads it: the `parallel_groups`
        // hint, the eventual Phase-3 fused-continuation admission) honest about which
        // `base_suspends` hosts are actually safe to treat as CPU-promoted.
        let closure_suspending: HashSet<&str> = closure.iter().map(String::as_str).collect();
        let mut to_remove: HashSet<String> = HashSet::new();
        for f in &fn_decls {
            let is_newly_sm = closure.contains(&f.name) && !base_suspends.contains(&f.name);
            let is_base_suspends_direct_candidate =
                base_suspends.contains(&f.name) && candidates.contains(&f.name);
            if !is_newly_sm && !is_base_suspends_direct_candidate {
                continue; // not newly-SM and not a base_suspends host with its own CPU join
            }
            // Augment the suspending set with this function's OWN CPU callees only when
            // it is itself a promotion candidate (its CPU joins are its suspension points).
            let mut suspending_fns = closure_suspending.clone();
            if let Some(callees) = cpu_callees_by_candidate.get(&f.name) {
                for name in callees {
                    suspending_fns.insert(name.as_str());
                }
            }
            let ret_ty = sig_table
                .fns
                .get(&f.name)
                .map(|s| s.ret.clone())
                .unwrap_or(crate::types::Type::Nothing);
            if crate::check::suspension_guards_fire_for_fn(
                f,
                &ret_ty,
                &suspending_fns,
                shape_table,
                &union_aliases,
                expr_types,
                kernel_mode,
            ) {
                // Every promoted candidate reachable from `f` is a cause; remove all.
                let reachable =
                    candidates_reachable_from(&f.name, module, imported_fn_names, &candidates);
                to_remove.extend(reachable);
            }
        }

        if to_remove.is_empty() {
            break; // fixpoint — no guard fires anywhere
        }
        for name in to_remove {
            candidates.remove(&name);
        }
    }

    candidates
}

/// Walk every CPU-parallelizable group in `stmts` (and the nested straight-line
/// blocks within them), invoking `visit` with each group's member statements and
/// their per-member classes.
///
/// A CPU-parallelizable group is a `Parallel` group of ≥2 members with ≥1 CPU member.
/// Recurses into nested control-flow blocks (`if`/`while`/`for`/`match` bodies) the
/// same way `lower_sm_block` does, re-running the partition per straight-line block —
/// a nested CPU group still makes the function a state machine. The single producer
/// for both candidacy (`function_has_cpu_group`) and CPU-callee collection.
///
/// Time: O(N · S²) where N = AST nodes visited (the full recursive descent) and
/// S = statements per straight-line block — `partition_groups_classified` is O(S²)
/// (pairwise data-dependency comparison) and runs once per block.
/// Space: O(D) where D = control-flow nesting depth (recursion stack only; `visit`
/// borrows, it does not accumulate).
fn walk_cpu_groups(
    stmts: &[ynz_ast::nodes::Stmt],
    suspend_set: &std::collections::HashSet<String>,
    sig_table: &SignatureTable,
    imported_fns: &std::collections::HashMap<String, FunctionSig>,
    cpu: &crate::independence::CpuCandidacy,
    visit: &mut dyn FnMut(&[&ynz_ast::nodes::Stmt], &[crate::independence::MemberClass]),
) {
    use crate::independence::ClassifiedGroup;
    use ynz_ast::nodes::Stmt;

    let groups = crate::independence::partition_groups_classified(
        stmts,
        suspend_set,
        sig_table,
        imported_fns,
        cpu,
    );
    for group in &groups {
        if let ClassifiedGroup::Parallel {
            stmts: members,
            classes,
        } = group
        {
            if members.len() >= 2 && classes.contains(&crate::independence::MemberClass::Cpu) {
                visit(members, classes);
            }
        }
    }

    for stmt in stmts {
        let nested: Vec<&[Stmt]> = match stmt {
            Stmt::If { body, .. } => vec![body.stmts.as_slice()],
            Stmt::While { body, .. } | Stmt::For { body, .. } => vec![body.stmts.as_slice()],
            Stmt::Match { arms, else_arm, .. } => {
                let mut v: Vec<&[Stmt]> = arms.iter().map(|a| a.body.stmts.as_slice()).collect();
                if let Some(eb) = else_arm {
                    v.push(eb.stmts.as_slice());
                }
                v
            }
            _ => Vec::new(),
        };
        for block in nested {
            walk_cpu_groups(block, suspend_set, sig_table, imported_fns, cpu, visit);
        }
    }
}

/// True when the function body contains at least one CPU-parallelizable group.
///
/// Time: O(N · S²) — a single `walk_cpu_groups` descent (same order as the walk).
/// Space: O(D) — the walk's recursion stack; the `found` flag is O(1).
fn function_has_cpu_group(
    stmts: &[ynz_ast::nodes::Stmt],
    suspend_set: &std::collections::HashSet<String>,
    sig_table: &SignatureTable,
    imported_fns: &std::collections::HashMap<String, FunctionSig>,
    cpu: &crate::independence::CpuCandidacy,
) -> bool {
    let mut found = false;
    walk_cpu_groups(
        stmts,
        suspend_set,
        sig_table,
        imported_fns,
        cpu,
        &mut |_, _| {
            found = true;
        },
    );
    found
}

/// Collect the callee names of every CPU member of every CPU-parallelizable group in
/// the function body. These are the function's suspension points once it is promoted —
/// the guard probe must treat them as suspending for crossing-local detection.
///
/// Time: O(N · S²) — a single `walk_cpu_groups` descent (same order as the walk).
/// Space: O(D + K) where D = nesting depth (the walk's recursion stack) and K = the
/// number of distinct CPU callee names accumulated into `out`.
fn collect_cpu_callees(
    stmts: &[ynz_ast::nodes::Stmt],
    suspend_set: &std::collections::HashSet<String>,
    sig_table: &SignatureTable,
    imported_fns: &std::collections::HashMap<String, FunctionSig>,
    cpu: &crate::independence::CpuCandidacy,
    out: &mut std::collections::HashSet<String>,
) {
    use crate::independence::MemberClass;
    walk_cpu_groups(
        stmts,
        suspend_set,
        sig_table,
        imported_fns,
        cpu,
        &mut |members, classes| {
            for (stmt, class) in members.iter().zip(classes.iter()) {
                if *class == MemberClass::Cpu {
                    if let Some(name) = crate::independence::stmt_direct_callee_name(stmt) {
                        out.insert(name);
                    }
                }
            }
        },
    );
}

/// The subset of `candidates` reachable from `start` through the (non-`background`)
/// call graph, including `start` itself if it is a candidate. Used by the rollback to
/// remove every promotion whose state-machine propagation reaches a guard-violating
/// function.
///
/// Time: O(V + E) — an explicit DFS over the call graph (V = functions reachable from
/// `start`, E = call edges between them); each node and edge is visited once via the
/// `seen` set.  Space: O(V) for the `seen`/`reachable` sets and the worklist stack.
fn candidates_reachable_from(
    start: &str,
    module: &Module,
    imported_fn_names: &std::collections::HashSet<String>,
    candidates: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    // Direct-callee adjacency from the call graph (background edges already cut).
    let adjacency = crate::may_block::direct_callee_adjacency(module, imported_fn_names);

    let mut reachable: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = vec![start.to_string()];
    let mut seen: HashSet<String> = HashSet::new();
    while let Some(node) = stack.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        if candidates.contains(&node) {
            reachable.insert(node.clone());
        }
        if let Some(callees) = adjacency.get(&node) {
            for callee in callees {
                if !seen.contains(callee) {
                    stack.push(callee.clone());
                }
            }
        }
    }
    reachable
}

#[cfg(test)]
mod promotion_tests {
    use super::*;
    use ynz_parser::{CompilerDb, SourceFile};

    /// Inputs `compute_cpu_promotions` consumes, assembled from a real sig + check
    /// pass on single-file source — exactly what `cpu_promotion_query` builds.
    struct PromoInputs {
        module: Module,
        sig_table: SignatureTable,
        shape_table: ShapeTable,
        imported_fns: std::collections::HashMap<String, FunctionSig>,
        base_suspends: std::collections::HashSet<String>,
        expr_types: std::collections::HashMap<(usize, usize), crate::types::Type>,
    }

    fn build_inputs(src: &str) -> PromoInputs {
        let mut db = CompilerDb::default();
        let sf = SourceFile::new(&db, "test.ynz".to_string(), src.to_string());
        db.register_source(sf);

        let parse = parse_query(&db, sf);
        let sig_output = module_signatures_query(&db, sf);
        let check_out = check_query(&db, sf);

        let mut sig_table = sig_output.sig_table.clone();
        for (name, sig) in &sig_output.imported_fns {
            sig_table
                .fns
                .entry(name.clone())
                .or_insert_with(|| sig.clone());
        }
        for (name, sig) in sig_table.fns.iter_mut() {
            if check_out.suspends_set.contains(name.as_str()) {
                sig.suspends = true;
            }
            if check_out.does_real_work_set.contains(name.as_str()) {
                sig.does_real_work = true;
            }
        }

        PromoInputs {
            module: parse.module.clone(),
            sig_table,
            shape_table: sig_output.shape_table.clone(),
            imported_fns: sig_output.imported_fns.clone(),
            base_suspends: check_out.suspends_set.clone(),
            expr_types: check_out.typed_module.expr_types.clone(),
        }
    }

    fn promote(
        inputs: &PromoInputs,
        kernel: bool,
        no_auto: bool,
    ) -> std::collections::HashSet<String> {
        let imported_fn_names: std::collections::HashSet<String> =
            inputs.imported_fns.keys().cloned().collect();
        let imported_suspending: std::collections::HashSet<String> = inputs
            .imported_fns
            .iter()
            .filter(|(_, s)| s.suspends)
            .map(|(n, _)| n.clone())
            .collect();
        compute_cpu_promotions(
            &inputs.module,
            &inputs.sig_table,
            &inputs.shape_table,
            &inputs.imported_fns,
            &inputs.base_suspends,
            &imported_fn_names,
            &imported_suspending,
            &inputs.expr_types,
            kernel,
            no_auto,
        )
    }

    const FIB: &str = r#"
function fib(n: int) -> int {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}
"#;

    #[test]
    fn clean_two_member_cpu_group_promotes() {
        // WHY: the headline pattern — two independent heavy CPU calls become a CPU
        // group, so the enclosing function promotes to a state machine.
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.contains("entrypoint"),
            "clean 2-member CPU group must promote the host; got: {promoted:?}"
        );
        // fib is self-recursive (a cycle of size 1 is NOT a mutual cycle) and is a
        // CALLEE, not a host with its own CPU group — it must not be promoted.
        assert!(
            !promoted.contains("fib"),
            "the callee fib must not be promoted"
        );
    }

    #[test]
    fn same_callee_cpu_pair_promotes() {
        // WHY: Research Finding 6 — same-callee CPU calls CAN parallelize (per-
        // invocation ctx), unlike the suspending same-callee restriction.
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(20)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.contains("entrypoint"),
            "same-callee CPU pair must promote the host; got: {promoted:?}"
        );
    }

    #[test]
    fn trivial_leaf_calls_do_not_promote() {
        // WHY: the worth-it proxy — two calls to a trivial leaf are not worth spawning.
        let src = r#"
function add(n: int) -> int { return n + 1 }
function entrypoint() -> nothing {
    let a = add(20)
    let b = add(21)
    print(a)
    print(b)
}
"#;
        let inputs = build_inputs(src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.is_empty(),
            "trivial-leaf calls must NOT promote; got: {promoted:?}"
        );
    }

    #[test]
    fn data_dependent_calls_do_not_promote() {
        // WHY: a data dependency breaks the group → no parallel group → no promotion.
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(a)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.is_empty(),
            "data-dependent calls must NOT form a CPU group; got: {promoted:?}"
        );
    }

    #[test]
    fn mutual_cycle_member_never_promotes() {
        // WHY: functions in a mutual call cycle are never promoted (mixed-layout
        // mutual-suspension prevention). Even with a CPU group, a cyclic host declines.
        let src = format!(
            r#"{FIB}
function pingHost() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
    pongHost()
}}
function pongHost() -> nothing {{
    pingHost()
}}
function entrypoint() -> nothing {{ pingHost() }}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("pingHost"),
            "a mutual-cycle member must never promote; got: {promoted:?}"
        );
        assert!(!promoted.contains("pongHost"));
        // The MutualSuspensionCycle decline (via the candidacy-gate cyclic-member
        // exclusion) must keep the program compiling clean — promotion never introduces
        // a new compile error.
        assert!(
            !has_errors(&src),
            "the mutual-cycle decline must keep the program compiling with no new errors"
        );
    }

    #[test]
    fn no_auto_parallel_flag_declines_all() {
        // WHY: the program-wide escape hatch — promotion declines entirely.
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, /*no_auto=*/ true);
        assert!(
            promoted.is_empty(),
            "--no-auto-parallel must decline all promotions; got: {promoted:?}"
        );
    }

    #[test]
    fn kernel_mode_declines_all() {
        // WHY: kernel mode has no scheduler to spawn onto — promotion declines
        // (inline-sequential fallback, never an error).
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, /*kernel=*/ true, false);
        assert!(
            promoted.is_empty(),
            "kernel mode must decline all promotions; got: {promoted:?}"
        );
    }

    #[test]
    fn promotion_is_deterministic_across_runs() {
        // WHY: the candidate + rollback fixpoint must be order-independent.
        let src = format!(
            r#"{FIB}
function sumTo(n: int) -> int {{
    let t = 0
    let i = 0
    while (i < n) {{ t = t + i; i = i + 1 }}
    return t
}}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = sumTo(21)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let a = promote(&inputs, false, false);
        let b = promote(&inputs, false, false);
        assert_eq!(a, b, "promotion set must be deterministic");
        assert!(a.contains("entrypoint"));
    }

    /// Serializes every test that reads or writes the process-global
    /// `YNZ_NO_AUTO_PARALLEL` env var (the salsa query reads it at its entry). Without
    /// this lock, one test's `set_var` races another test's query and both flake. Any
    /// test that calls `cpu_promotion_query` (which reads the env var) holds this lock.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn salsa_query_respects_no_auto_parallel_env() {
        // WHY: the salsa query reads YNZ_NO_AUTO_PARALLEL at its entry — verify the
        // gate wiring (separate from the pure-fn flag tests). The ENV_LOCK serializes
        // this against any other env-var-touching test; the var is restored after.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let mut db = CompilerDb::default();
        let sf = SourceFile::new(&db, "envtest.ynz".to_string(), src);
        db.register_source(sf);

        // Sanity: without the flag, the host promotes.
        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        let on = cpu_promotion_query(&db, sf);
        assert!(
            on.promoted.contains("entrypoint"),
            "without --no-auto-parallel the host should promote; got: {:?}",
            on.promoted
        );

        // With the flag set, the query declines entirely.
        std::env::set_var("YNZ_NO_AUTO_PARALLEL", "1");
        // A fresh db so the salsa cache re-evaluates with the env var visible.
        let mut db2 = CompilerDb::default();
        let src2 = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let sf2 = SourceFile::new(&db2, "envtest2.ynz".to_string(), src2);
        db2.register_source(sf2);
        let off = cpu_promotion_query(&db2, sf2);
        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        assert!(
            off.promoted.is_empty(),
            "with --no-auto-parallel the query must decline; got: {:?}",
            off.promoted
        );
    }

    // Guard-probe decline: a CPU group whose host would trip a suspension guard declines
    // silently and the program still compiles with no new errors.

    /// Run the full check pass and return whether it produced any ERROR diagnostic.
    fn has_errors(src: &str) -> bool {
        let mut db = CompilerDb::default();
        let sf = SourceFile::new(&db, "guard.ynz".to_string(), src.to_string());
        db.register_source(sf);
        let check_out = check_query(&db, sf);
        let has_err = check_out
            .diagnostics
            .iter()
            .any(|d| d.severity == ynz_diagnostics::Severity::Error);
        has_err
    }

    /// Collect the `what` text of every error-severity diagnostic the check pass emits.
    /// Used by the for-with-wait decline fixtures to prove the PRE-EXISTING for-wait guard
    /// (not anything M3d added) is what rejected the shape, and that the RIGHT guard fired.
    fn error_whats(src: &str) -> Vec<String> {
        let mut db = CompilerDb::default();
        let sf = SourceFile::new(&db, "guard.ynz".to_string(), src.to_string());
        db.register_source(sf);
        let check_out = check_query(&db, sf);
        check_out
            .diagnostics
            .iter()
            .filter(|d| d.severity == ynz_diagnostics::Severity::Error)
            .map(|d| d.what.clone())
            .collect()
    }

    #[test]
    fn wide_shape_return_host_declines_and_compiles_clean() {
        // WHY (WideValueSuspendingReturn decline): a host that returns a bare Shape
        // and ALSO has a CPU pair must NOT promote — promoting it would make it a
        // suspending function returning a Shape, which the wide-return guard rejects.
        // The decline keeps it sequential, so it must still compile with zero errors.
        let src = format!(
            r#"{FIB}
shape Point {{
    x: int
    y: int
}}
function makePoint() -> Point {{
    let a = fib(20)
    let b = fib(21)
    return {{ x: a, y: b }}
}}
function entrypoint() -> nothing {{
    let p = makePoint()
    print(p.x)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("makePoint"),
            "a wide-Shape-return host must decline promotion; got: {promoted:?}"
        );
        assert!(
            !has_errors(&src),
            "the wide-return decline must keep the program compiling with no new errors"
        );
    }

    #[test]
    fn nested_shape_crossing_local_host_declines_and_compiles_clean() {
        // WHY (nested-shape crossing decline): a shape value with a nested-shape field,
        // bound to a local that crosses the CPU join, cannot be frame-embedded yet.
        // Promoting the host trips the crossing-local guard, so promotion declines and
        // the program compiles clean sequentially (no new error).
        let src = format!(
            r#"{FIB}
shape Inner {{
    v: int
}}
shape Outer {{
    inner: Inner
    tag: int
}}
function makeOuter() -> Outer {{
    let i: Inner = {{ v: 1 }}
    return {{ inner: i, tag: 2 }}
}}
function entrypoint() -> nothing {{
    let w = makeOuter()
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
    print(w.tag)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("entrypoint"),
            "a nested-shape-crossing-local host must decline promotion; got: {promoted:?}"
        );
        assert!(
            !has_errors(&src),
            "the crossing-local decline must keep the program compiling with no new errors"
        );
    }

    #[test]
    fn cpu_callee_in_noncandidate_caller_subexpr_does_not_roll_back_sound_candidate() {
        // WHY (per-candidate CPU-callee seeding): the guard probe seeds a promoted
        // function's CPU callees into `suspending_fns` so a local crossing the CPU join
        // is visible. That seed must be PER-CANDIDATE — augmenting the probe's suspending
        // set ONLY when probing the candidate itself. A NON-candidate caller that happens
        // to call the same CPU callee in a sub-expression position (`let x = fib(10) + 1`)
        // must NOT be probed against `fib` as a suspending function: it isn't promoted, so
        // `fib` is an ordinary call there. A global seed would fire
        // `suspending_calls_in_subexpr_position` on the non-candidate caller and roll back
        // the SOUND candidate it transitively calls — a false negative.
        let src = format!(
            r#"{FIB}
function caller() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
function wrapper() -> nothing {{
    let x = fib(10) + 1
    print(x)
    caller()
}}
function entrypoint() -> nothing {{
    wrapper()
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.contains("caller"),
            "a sound CPU candidate must promote even when a non-candidate caller calls \
             the same CPU callee in a sub-expression; the seed must be per-candidate. \
             got: {promoted:?}"
        );
    }

    #[test]
    fn param_nested_let_shadow_host_declines_and_compiles_clean() {
        // WHY (ShadowsCrossingLocal — parameter nested-shadow decline): a host with a
        // CPU pair (making it a promotion candidate) AND a parameter re-declared by a
        // `let` inside a NESTED block. The real checker's parameter-shadow guard
        // (`param_has_nested_let_shadow`, Shape (a)) fires on ANY nested `let <param>`,
        // even one not gated behind a top-level `wait`. If promotion turned this host
        // into a state machine, `check_function` would reject it as `ShadowsCrossingLocal`
        // — a silent compile break. The promotion probe MUST therefore detect the same
        // nested param shadow and decline, keeping the host sequential and clean.
        let src = format!(
            r#"{FIB}
function host(seed: int) -> nothing {{
    let a = fib(20)
    let b = fib(21)
    if (a > 0) {{
        let seed = a + b
        print(seed)
    }}
    print(a)
    print(b)
}}
function entrypoint() -> nothing {{
    host(1)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("host"),
            "a host with a nested parameter shadow must decline promotion (else \
             check_function would later reject it as ShadowsCrossingLocal); got: {promoted:?}"
        );
        assert!(
            !has_errors(&src),
            "the parameter-shadow decline must keep the program compiling with no new errors"
        );
    }

    #[test]
    fn array_shape_runtime_field_crossing_host_promotes_and_compiles_clean() {
        // WHY (ArrayShapeRuntimeFieldWithWait LIFTED, v0.3-M5 Phase 3): an `array<Shape>`
        // whose elements have runtime-computed field values, bound to a local that crosses
        // the CPU join, is now SAFE to frame-embed — the by-value ABI cut stores element
        // bytes inline in the heap buffer (stable owner across any suspension), so
        // `check_function` no longer rejects the promoted SM host and the probe must no
        // longer decline it. This is the INVERSE of the pre-M5 decline test: the host
        // must now PROMOTE, and the program must still compile with no errors. If this
        // test starts failing with a decline, a rejection path for runtime-field
        // array<Shape> crossings has been reintroduced — that class was lifted, not moved.
        let src = format!(
            r#"{FIB}
shape Item {{
    id: int
    qty: int
}}
function entrypoint() -> nothing {{
    let qval = 10
    let items: array<Item> = [{{ id: 1, qty: qval }}, {{ id: 2, qty: qval }}]
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
    let total = 0
    for (it in items) {{
        total = total + it.qty
    }}
    print(total)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.contains("entrypoint"),
            "an array<Shape>-runtime-field crossing-local host must now PROMOTE (guard \
             lifted by v0.3-M5 by-value storage); got: {promoted:?}"
        );
        assert!(
            !has_errors(&src),
            "the promoted array<Shape>-runtime-field host must compile with no errors"
        );
    }

    #[test]
    fn wait_free_for_loop_does_not_block_promotion() {
        // WHY (non-interference): a wait-FREE for-loop sitting beside a valid CPU pair must
        // NOT spuriously block promotion. The three for-with-wait guards (StoredRangeWithWait
        // / FixedArrayIterWithWait / ExpressionIterWithWait) are gated behind
        // `has_explicit_waits` — they fire only on a `for` loop whose body contains an
        // explicit `wait`. None of these loops carries a `wait`, so none of the guards apply,
        // and the top-level CPU pair (`fib(20)` / `fib(21)`) promotes normally. This proves
        // the wait-gated guards impose no cost on ordinary wait-free loops.
        //
        // (Those guards CAN fire — see `*_wait_host_declines_promotion` below, which drive
        // each guard via a `for` body that waits on a non-suspending callee. This test only
        // covers the complementary case: a wait-free loop doesn't get caught in the crossfire.)
        let src = format!(
            r#"{FIB}
function makeArray() -> array<int> {{
    return [1, 2, 3]
}}
function entrypoint() -> nothing {{
    let sum = 0
    for (i in range(0, 3)) {{
        sum = sum + i
    }}
    let nums: fixed<int> = [10, 20, 30]
    for (n in nums) {{
        sum = sum + n
    }}
    for (m in makeArray()) {{
        sum = sum + m
    }}
    print(sum)
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        // The program is clean pre-promotion: no `wait`, so no for-with-wait guard fires.
        assert!(
            !has_errors(&src),
            "a for-loop over a stored range/fixed/expr WITHOUT a wait must compile clean"
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.contains("entrypoint"),
            "the CPU pair must promote; wait-free for-loops impose no decline. got: {promoted:?}"
        );
    }

    // A trivial non-suspending callee for the three decline fixtures below. Waiting on it
    // gives the host an explicit `wait` (so the for-with-wait guard reaches it) WITHOUT
    // making the host I/O-suspending (it reaches no may-block call), so the host stays a
    // CPU-promotion candidate. `wait tick()` only earns a "never suspends" WARNING — never
    // an error — so the ONLY error each fixture sees is its for-with-wait guard.
    const TICK: &str = r#"
function tick() -> int {
    return 1
}
"#;

    #[test]
    fn stored_range_wait_host_declines_promotion() {
        // WHY: these three guard shapes (this one + the fixed/expr fixtures below) ERROR
        // pre-M3d via the EXISTING for-with-wait guards (check.rs ~590) — a `for` whose body
        // waits cannot yet recover a stored-range / fixed / call-expr iterator across the
        // suspension. So, unlike the 5 valid-but-unpromotable guards (which compile clean),
        // the contract here is "NOT promoted + the pre-existing diagnostic is unchanged"
        // (M3d introduced ZERO NEW diagnostics) — NOT "compiles clean". Step 7's
        // "compiled pre-M3d" precondition does not apply: these shapes never compiled.
        //
        // This fixture isolates the StoredRangeWithWait guard: `r` is a stored `range(...)`
        // variable, so the for-loop iterator is an `Ident` of `Type::Range`. The top-level
        // `fib(20)` / `fib(21)` pair makes the host a genuine CPU-promotion candidate, so
        // `!promoted` is NON-vacuous: the probe reaches this candidate and the for-wait guard
        // declines it.
        let src = format!(
            r#"{FIB}{TICK}
function entrypoint() -> nothing {{
    let r = range(0, 3)
    for (i in r) {{
        wait tick()
    }}
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        // Non-vacuous decline: the host IS a CPU candidate (the fib pair), but the
        // StoredRangeWithWait guard fires in the probe and rolls it back.
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("entrypoint"),
            "a stored-range for-with-wait host must decline promotion; got: {promoted:?}"
        );
        // Zero NEW diagnostics: the only error is the PRE-EXISTING StoredRangeWithWait guard,
        // and it is THIS guard (not the fixed/expr one) — isolating the intended guard.
        let whats = error_whats(&src);
        assert!(
            whats
                .iter()
                .any(|w| w.contains("stored range variable cannot yet be the iterator")),
            "must hit the StoredRangeWithWait guard; got errors: {whats:?}"
        );
        assert!(
            !whats
                .iter()
                .any(|w| w.contains("`fixed<T>` array") || w.contains("call-expression iterator")),
            "must isolate StoredRangeWithWait — no fixed/expr guard should fire; got: {whats:?}"
        );
    }

    #[test]
    fn fixed_array_iter_wait_host_declines_promotion() {
        // WHY: see `stored_range_wait_host_declines_promotion` — these guard shapes error
        // pre-M3d, so the contract is "NOT promoted + pre-existing diagnostic unchanged".
        //
        // This fixture isolates the FixedArrayIterWithWait guard: `nums: fixed<int>` makes
        // the for-loop iterator an `Ident` of `Type::BuiltinFixed`. The fib pair makes the
        // host a real CPU candidate, so the `!promoted` decline is non-vacuous.
        let src = format!(
            r#"{FIB}{TICK}
function entrypoint() -> nothing {{
    let nums: fixed<int> = [10, 20, 30]
    for (n in nums) {{
        wait tick()
    }}
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("entrypoint"),
            "a fixed<T>-iter for-with-wait host must decline promotion; got: {promoted:?}"
        );
        let whats = error_whats(&src);
        assert!(
            whats
                .iter()
                .any(|w| w.contains("`fixed<T>` array cannot be the iterator")),
            "must hit the FixedArrayIterWithWait guard; got errors: {whats:?}"
        );
        assert!(
            !whats
                .iter()
                .any(|w| w.contains("stored range variable")
                    || w.contains("call-expression iterator")),
            "must isolate FixedArrayIterWithWait — no stored-range/expr guard; got: {whats:?}"
        );
    }

    #[test]
    fn expression_iter_wait_host_declines_promotion() {
        // WHY: see `stored_range_wait_host_declines_promotion` — these guard shapes error
        // pre-M3d, so the contract is "NOT promoted + pre-existing diagnostic unchanged".
        //
        // This fixture isolates the ExpressionIterWithWait guard: the for-loop iterates a
        // call expression (`makeArray()`, a non-`range` call), not a plain identifier. The
        // fib pair makes the host a real CPU candidate, so the `!promoted` decline is
        // non-vacuous.
        let src = format!(
            r#"{FIB}{TICK}
function makeArray() -> array<int> {{
    return [1, 2, 3]
}}
function entrypoint() -> nothing {{
    for (m in makeArray()) {{
        wait tick()
    }}
    let a = fib(20)
    let b = fib(21)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("entrypoint"),
            "a call-expr-iter for-with-wait host must decline promotion; got: {promoted:?}"
        );
        let whats = error_whats(&src);
        assert!(
            whats
                .iter()
                .any(|w| w.contains("call-expression iterator cannot yet be used")),
            "must hit the ExpressionIterWithWait guard; got errors: {whats:?}"
        );
        assert!(
            !whats
                .iter()
                .any(|w| w.contains("stored range variable") || w.contains("`fixed<T>` array")),
            "must isolate ExpressionIterWithWait — no stored-range/fixed guard; got: {whats:?}"
        );
    }

    #[test]
    fn imported_real_work_callee_promotes_importer() {
        // WHY (AC: does_real_work cross-module propagation): an importer that calls an
        // imported loop/recursion callee TWICE in independent statements must promote —
        // the imported callee's `does_real_work` flag rides the export table, so the
        // importer's CPU candidacy sees it as worth-it. A two-file project proves the
        // cross-module worth-it propagation end-to-end through the salsa queries.
        // Holds ENV_LOCK because this calls `cpu_promotion_query` (reads the env var).
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "ynz_m3d_promo_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("yinz.toml"), "[project]\nname = \"test\"\n")
            .expect("write yinz.toml");

        let dep_src = r#"
export function heavy(n: int) -> int {
    let total = 0
    let i = 0
    while (i < n) {
        total = total + i
        i = i + 1
    }
    return total
}
"#;
        let main_src = r#"
import { heavy } from `lib`
function entrypoint() -> nothing {
    let a = heavy(20)
    let b = heavy(21)
    print(a)
    print(b)
}
"#;
        let dep_path = dir.join("lib.ynz");
        let main_path = dir.join("entrypoint.ynz");
        std::fs::write(&dep_path, dep_src).expect("write dep");
        std::fs::write(&main_path, main_src).expect("write main");

        let mut db = CompilerDb::default();
        let sf_dep = SourceFile::new(&db, dep_path.display().to_string(), dep_src.to_string());
        let sf_main = SourceFile::new(&db, main_path.display().to_string(), main_src.to_string());
        db.register_source(sf_dep);
        db.register_source(sf_main);

        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        let promoted = cpu_promotion_query(&db, sf_main);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            promoted.promoted.contains("entrypoint"),
            "importer must promote calls to an imported real-work callee; got: {:?}",
            promoted.promoted
        );
    }

    #[test]
    fn guard_clean_host_still_compiles_clean() {
        // WHY (control): the SAME shapes WITHOUT a CPU group compile clean — proving
        // the decline cases above aren't masking a pre-existing error, and that the
        // guard corpus's diagnostics are identical pre/post the promotion logic
        // (here: zero errors in both, because P2 emits no new diagnostics at all).
        let src = r#"
shape Point {
    x: int
    y: int
}
function makePoint(seed: int) -> Point {
    return { x: seed, y: seed }
}
function entrypoint() -> nothing {
    let p = makePoint(3)
    print(p.x)
}
"#;
        assert!(
            !has_errors(src),
            "the guard-clean control must compile with no errors"
        );
    }

    // v0.3-M3g Phase 2: `base_suspends` host promotion + guard-probe extension (E6).
    // These probe `compute_cpu_promotions`'s `promoted` set DIRECTLY — the precise, internal
    // proof that the integration-level `v03_m3g_*_declines_byte_identical` fixtures cannot give
    // by themselves, because `admitted_cpu_group`'s Phase-2 temporary co-resident-suspension
    // decline ALSO forces every such fixture to 0 spawns regardless of whether this typeck-side
    // set is correct. Both proofs matter: the fixtures lock the observable (byte-identical)
    // behavior; these unit tests lock the underlying `promoted`-set correctness the fixtures
    // can't directly see.

    #[test]
    fn base_suspends_host_with_clean_cpu_group_promotes() {
        // WHY: a suspending host (`wait sleep(...)`) that ALSO owns a clean (non-guard-tripping)
        // top-level CPU group must now be a genuine `promoted` candidate — the `base_suspends`
        // skip this phase lifts from `compute_cpu_promotions`'s candidate identification. This
        // is the positive companion to the E6 decline test below: lifting the skip must not
        // ALSO make every base_suspends host universally decline via the guard-probe — a clean
        // one must still promote.
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    wait sleep(0)
    print(a)
    print(b)
}}
"#
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            promoted.contains("entrypoint"),
            "a base_suspends host with a clean top-level CPU group must promote once the \
             base_suspends skip is lifted; got: {promoted:?}"
        );
    }

    #[test]
    fn base_suspends_host_with_subexpr_position_guard_declines_promotion() {
        // WHY (E6 proof): `entrypoint` is a `base_suspends` host (`wait sleep(0)`) that ALSO
        // owns its own top-level CPU group (`fib(20)`, `fib(21)`) — a genuine candidate once
        // the base_suspends skip is lifted — PLUS a THIRD, separate call to the same CPU-group
        // callee used in a sub-expression position (`fib(22) + 1`). `fib` is NOT suspending
        // under the REAL suspend set (ordinary `check_query` never flags this — it compiles
        // clean), so the hazard is invisible to the plain compile pass. It becomes visible ONLY
        // when the guard-probe augments `suspending_fns` with THIS candidate's own CPU-group
        // callee (`fib`, per-candidate seeding), which is exactly the E6 extension: the
        // guard-probe must now probe `base_suspends` hosts that are themselves direct
        // candidates, not just skip them. If the E6 extension regresses (the guard-probe loop
        // reverts to skipping `base_suspends` hosts entirely), this candidate would wrongly stay
        // in `promoted` — this test would then fail to observe the decline.
        let src = format!(
            r#"{FIB}
function entrypoint() -> nothing {{
    let a = fib(20)
    let b = fib(21)
    let c = fib(22) + 1
    wait sleep(0)
    print(a + b + c)
}}
"#
        );
        // Non-vacuous control: the program compiles clean BEFORE promotion is even considered —
        // `fib(22) + 1` is not a suspending-call-in-subexpr-position violation under the real
        // suspend set, so this isn't accidentally testing a pre-existing hard compile error.
        assert!(
            !has_errors(&src),
            "the subexpr-position fib call must compile clean under the REAL suspend set \
             (fib never suspends) — this test must isolate the guard-probe decline, not a \
             pre-existing compile error"
        );
        let inputs = build_inputs(&src);
        let promoted = promote(&inputs, false, false);
        assert!(
            !promoted.contains("entrypoint"),
            "a base_suspends host whose own CPU-group callee ALSO appears in a sub-expression \
             position elsewhere must decline promotion (E6 guard-probe extension); got: {promoted:?}"
        );
    }
}
