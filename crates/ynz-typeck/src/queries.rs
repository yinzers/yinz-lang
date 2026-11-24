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
    /// The places each local function writes through each parameter
    /// (`walked_place::analyze_written_param_places`), by function name and parameter position.
    ///
    /// Computed once here, before the bodies are checked: the loop check reads it through
    /// `FunctionSig::written_param_places`, and the export table copies it onto the exported
    /// signatures so an importer's loop sees the same answer.
    pub written_param_places:
        std::collections::HashMap<String, Vec<Vec<crate::walked_place::WrittenPlace>>>,
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
    let mut shape_table = collect_shapes(
        &parse.module,
        &imported.shapes,
        &imported.options,
        &mut diag_bucket,
    );
    // An imported shape's union-field variants the importer never named, kept only so codegen
    // can size the imported shape (a layout-only entry is not resolvable by name). A name that
    // already means ANOTHER shape here (this module's own, or another import) is refused: the
    // imported shape would be sized by the wrong variant (`union_variant_name_clash`).
    for (name, def) in imported.layout_shapes {
        match shape_table.get(&name) {
            None => {
                shape_table.layout_only.insert(name, def);
            }
            Some(named) if *named == def => {}
            Some(named) => {
                let span = named.defined_at.clone();
                diag_bucket.push(crate::resolve_import::union_variant_name_clash(&name, span));
            }
        }
    }
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
            back_edge_yield_admitted: std::collections::HashSet::new(),
            background_arg_inferred_ownership: std::collections::HashMap::new(),
            background_arg_sole_holder: std::collections::HashMap::new(),
            stores_handed_over: std::collections::HashMap::new(),
            union_coercions: std::collections::HashMap::new(),
            cross_thread_padded_shapes: std::collections::HashSet::new(),
            binding_names: crate::binding_names::BindingNames::default(),
            transfers: Default::default(),
            release_plan: Default::default(),
            generic_call_instances: Default::default(),
            string_moves: Default::default(),
            heap_moves: Default::default(),
            loop_row_handovers: Default::default(),
            non_owned_reads: Default::default(),
            temporaries: Default::default(),
            callable_fns: Default::default(),
            fixed_instances: Default::default(),
            inferred_return_types: Default::default(),
        },
        mono_table: crate::generics::MonomorphizationTable {
            entries: std::collections::BTreeMap::new(),
        },
        diagnostics: DiagnosticBucket::new(),
        suspends_set: std::collections::HashSet::new(),
        does_real_work_set: std::collections::HashSet::new(),
        written_param_places: std::collections::HashMap::new(),
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

    // The places each local function writes through its parameters: read by the loop check
    // (`Checker::check_walked_list_kept`) when a loop passes the list it walks to a callee.
    let written_param_places =
        crate::walked_place::analyze_written_param_places(&parse.module, &sig_output.imported_fns);
    for (name, sig) in merged_sig_table.fns.iter_mut() {
        if let Some(places) = written_param_places.get(name.as_str()) {
            sig.written_param_places = places.clone();
        }
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

    // Transitive effective-ownership fixpoint (v0.3-M3b Phase 4) — HOISTED above the body
    // check in v0.3-M8 Phase 4: it depends only on the parse and the signature tables, both
    // in hand here, and the body check now CONSUMES it (the transfer decision reads
    // `consumed[fn][i]` so a relay chain missing `give` is reported at every frame in one
    // compile, and `returns_fresh[fn]` so a call result bound by `let` has a provenance —
    // `IMP-ownership.md` "Where the authority lives"). Classifies every parameter as
    // `Reads`/`Unknown`/`Writes` across the whole local call graph. Consumers:
    //   - `check(...)` below: `check_transfer` at every transfer sink;
    //   - Part 2 further below: reject `share` params that are written transitively (the
    //     `design/concurrency.md` line 651 no-escalation rule, extended past the direct
    //     cases `check.rs` already catches).
    //
    // Codegen does NOT consume this fixpoint. The auto-parallel write-effect decision uses
    // the type-based conservative floor in `ynz-codegen/src/independence.rs` (any mutable-heap
    // arg is a potential write, Golden Rule 2 > Rule 7) — sound by construction, with no
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
        effective_ownership::analyze(&parse.module, &declared_writes, &imported_fn_names, &|t| {
            sig_output.shape_table.resolve_ast_type(t)
        });

    let (typed, mono_table, check_diags, referenced_names) = check(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &PrimitiveIntrinsicTable::m6().with_m2_internals(),
        &sig_output.imported_options,
        &effective_ownership_report,
        &imported_fn_names,
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
    // for the duration of the call (Golden Rule 2: wrong = caught at compile time).
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
        written_param_places,
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
        &check_out.typed_module.release_plan,
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

/// The names of the functions that do real work (a loop or recursion, transitively): the
/// worth-it proxy CPU candidacy reads.
///
/// O(F) time and space for F functions.
fn does_real_work_names(sig_table: &SignatureTable) -> std::collections::HashSet<String> {
    sig_table
        .fns
        .iter()
        .filter(|(_, sig)| sig.does_real_work)
        .map(|(name, _)| name.clone())
        .collect()
}

/// The module's functions, sorted by name: the deterministic order candidacy is judged in.
///
/// O(F log F) time.
fn sorted_function_decls(module: &Module) -> Vec<&ynz_ast::nodes::FunctionDecl> {
    let mut fn_decls: Vec<&ynz_ast::nodes::FunctionDecl> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    fn_decls.sort_by(|a, b| a.name.cmp(&b.name));
    fn_decls
}

/// THE producer of CPU-promotion candidates: the non-cyclic functions whose body holds a
/// CPU-parallelizable group. `compute_cpu_promotions` rolls this set back to the functions whose
/// CPU joins pass the suspension guards; the checker reads the unrolled set through
/// [`possibly_cpu_promoted`] to know which functions may become state machines.
///
/// A function in a call cycle is never a candidate (mutual-suspension prevention: self-recursion
/// is allowed by the spawn ABI, but a self-recursive call is never a CPU group member).
/// `find_mutual_suspension_cycles` over the FULL call graph (not restricted to suspends) gives
/// the mutually-recursive members. A `base_suspends` host (already a state machine for its OWN
/// wait or suspending callee) is a candidate too when it hosts its own CPU group.
///
/// O(F · N · S²) time for F functions of N statements (one `walk_cpu_groups` descent each).
pub(crate) fn cpu_promotion_candidates(
    module: &Module,
    sig_table: &SignatureTable,
    imported_fns: &std::collections::HashMap<String, FunctionSig>,
    base_suspends: &std::collections::HashSet<String>,
    imported_fn_names: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let does_real_work = does_real_work_names(sig_table);
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
    let mut candidates: HashSet<String> = HashSet::new();
    for f in sorted_function_decls(module) {
        if cyclic_members.contains(&f.name) {
            continue;
        }
        let cpu_here = crate::independence::CpuCandidacy {
            does_real_work: &does_real_work,
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
    candidates
}

/// The functions that MAY be state machines once CPU promotion has run: the candidates and every
/// caller of one (`suspends_with_extra_seeds`), added to the functions that already suspend.
/// The guard-probe rollback in `compute_cpu_promotions` only ever shrinks the candidate set, so
/// the real promoted set and its closure lie inside this one. The checker records the release
/// plan's suspension edges for these functions, and for no other that does not suspend.
///
/// O(F · N · S² + call graph) time.
pub(crate) fn possibly_cpu_promoted(
    module: &Module,
    sig_table: &SignatureTable,
    imported_fn_names: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let base_suspends: HashSet<String> = sig_table
        .fns
        .iter()
        .filter(|(name, sig)| sig.suspends && !imported_fn_names.contains(*name))
        .map(|(name, _)| name.clone())
        .collect();
    let imported_suspending_names: HashSet<String> = imported_fn_names
        .iter()
        .filter(|name| sig_table.fns.get(*name).is_some_and(|sig| sig.suspends))
        .cloned()
        .collect();
    let candidates = cpu_promotion_candidates(
        module,
        sig_table,
        &sig_table.fns,
        &base_suspends,
        imported_fn_names,
    );
    if candidates.is_empty() {
        return candidates;
    }
    crate::may_block::suspends_with_extra_seeds(
        module,
        imported_fn_names,
        &imported_suspending_names,
        &candidates,
    )
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
    release_plan: &crate::release_plan::ReleasePlan,
) -> std::collections::HashSet<String> {
    use std::collections::HashSet;

    // Step 6 gating — promotion declines entirely under either flag.
    if no_auto_parallel || kernel_mode {
        return HashSet::new();
    }

    let does_real_work = does_real_work_names(sig_table);
    let fn_decls = sorted_function_decls(module);

    // Step 1 — candidate identification (`cpu_promotion_candidates`, shared with the checker).
    let mut candidates = cpu_promotion_candidates(
        module,
        sig_table,
        imported_fns,
        base_suspends,
        imported_fn_names,
    );

    // Steps 2 + 3 — transitive-SM closure + guard-probe rollback to fixpoint.
    let union_aliases = shape_table.union_aliases.clone();
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
                // M3d decline probe: baseline crossing set, never the Phase 6 widening
                // (only back_edge_yield_admission probes with true).
                false,
                // The release owners cross where their release is (a read at its edge).
                &release_plan.reads(&f.name, crate::release_plan::Names::User),
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

    /// Serializes every test that reads or writes the process-global `YNZ_NO_AUTO_PARALLEL` env
    /// var (`cpu_promotion_query` reads it at its entry); under plain `cargo test` the tests of
    /// this module share one process, and one test's `set_var` would race another's query.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const FIB: &str = r#"
function fib(n: Int) -> Int {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}
"#;

    fn source_file(db: &mut CompilerDb, file_name: &str, source: &str) -> SourceFile {
        let source_file = SourceFile::new(db, file_name.to_string(), source.to_string());
        db.register_source(source_file);
        source_file
    }

    /// The functions `cpu_promotion_query` promotes, with the kill switch off.
    fn promoted_functions(source: &str) -> std::collections::HashSet<String> {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        let mut db = CompilerDb::default();
        let source_file = source_file(&mut db, "test.ynz", source);
        cpu_promotion_query(&db, source_file).promoted.clone()
    }

    /// The `what` text of every error-severity diagnostic the check pass emits.
    fn error_whats(source: &str) -> Vec<String> {
        let mut db = CompilerDb::default();
        let source_file = source_file(&mut db, "guard.ynz", source);
        check_query(&db, source_file)
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == ynz_diagnostics::Severity::Error)
            .map(|diagnostic| diagnostic.what.clone())
            .collect()
    }

    /// One named test per case so the runner names the failing case. Each case lists the functions
    /// that must be promoted and the ones that must not, and the program must still compile with
    /// no error (promotion never introduces one).
    macro_rules! promotion_case {
        ($test_name:ident, source: $source:expr, promoted: [$($promoted:expr),*], declined: [$($declined:expr),*]) => {
            // bug: cpu_promotion_query promotes a host that cannot become a state machine (the checker then rejects a program that compiled before: a silent compile break) or declines one that can (the lost parallel speed-up) | tier: Medium
            #[test]
            fn $test_name() {
                let source: String = $source;
                let promoted = promoted_functions(&source);
                $(assert!(promoted.contains($promoted), "{} must be promoted; got {promoted:?}", $promoted);)*
                $(assert!(!promoted.contains($declined), "{} must NOT be promoted; got {promoted:?}", $declined);)*
                assert_eq!(error_whats(&source), Vec::<String>::new(), "promotion must not introduce an error");
            }
        };
    }

    // Thirteen cases here, three for-with-wait cases and two query cases below (18 for one query), over
    // the Medium guide of three, justified in the lane report: each is a
    // different decline guard or seed of the promotion probe (a distinct code path with its own
    // shipped bug), not a variation of one input.

    // Case bug: the headline pair of independent heavy calls does not promote its host (the callee `fib` is not a host).
    promotion_case!(
        a_clean_two_member_cpu_group_promotes_its_host,
        source: [FIB, "function entrypoint() -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\n"].concat(),
        promoted: ["entrypoint"],
        declined: ["fib"]
    );
    // Case bug: two calls of the SAME callee are refused the way suspending same-callee calls are (research finding 6).
    promotion_case!(
        a_same_callee_cpu_pair_promotes,
        source: [FIB, "function entrypoint() -> Nothing {\n    let a = fib(20)\n    let b = fib(20)\n    print(a)\n    print(b)\n}\n"].concat(),
        promoted: ["entrypoint"],
        declined: []
    );
    // Case bug: calls to a trivial leaf are spawned (the worth-it proxy is ignored).
    promotion_case!(
        calls_to_a_trivial_leaf_do_not_promote,
        source: "function add(n: Int) -> Int { return n + 1 }\nfunction entrypoint() -> Nothing {\n    let a = add(20)\n    let b = add(21)\n    print(a)\n    print(b)\n}\n".to_string(),
        promoted: [],
        declined: ["entrypoint"]
    );
    // Case bug: a data dependency between the calls still forms a parallel group.
    promotion_case!(
        data_dependent_calls_do_not_promote,
        source: [FIB, "function entrypoint() -> Nothing {\n    let a = fib(20)\n    let b = fib(a)\n    print(a)\n    print(b)\n}\n"].concat(),
        promoted: [],
        declined: ["entrypoint"]
    );
    // Case bug: a member of a mutual call cycle is promoted (mixed-layout mutual suspension).
    promotion_case!(
        a_mutual_cycle_member_never_promotes,
        source: [FIB, "function pingHost() -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n    pongHost()\n}\nfunction pongHost() -> Nothing {\n    pingHost()\n}\nfunction entrypoint() -> Nothing { pingHost() }\n"].concat(),
        promoted: [],
        declined: ["pingHost", "pongHost"]
    );
    // Case bug (WideValueSuspendingReturn): a host returning a bare shape is promoted into a suspending function the wide-return guard rejects.
    promotion_case!(
        a_host_returning_a_wide_shape_declines,
        source: [FIB, "shape Point {\n    x: Int\n    y: Int\n}\nfunction makePoint() -> Point {\n    let a = fib(20)\n    let b = fib(21)\n    return { x: a, y: b }\n}\nfunction entrypoint() -> Nothing {\n    let p = makePoint()\n    print(p.x)\n}\n"].concat(),
        promoted: [],
        declined: ["makePoint"]
    );
    // Case bug: a nested-shape local that crosses the CPU join is promoted (it cannot be frame-embedded yet).
    promotion_case!(
        a_host_with_a_nested_shape_local_across_the_join_declines,
        source: [FIB, "shape Inner {\n    v: Int\n}\nshape Outer {\n    inner: Inner\n    tag: Int\n}\nfunction makeOuter() -> Outer {\n    let i: Inner = { v: 1 }\n    return { inner: i, tag: 2 }\n}\nfunction entrypoint() -> Nothing {\n    let w = makeOuter()\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n    print(w.tag)\n}\n"].concat(),
        promoted: [],
        declined: ["entrypoint"]
    );
    // Case bug: the guard probe seeds a CPU callee globally, so a NON-candidate caller that uses it in a sub-expression rolls back a sound candidate.
    promotion_case!(
        a_noncandidate_callers_subexpression_call_does_not_roll_back_a_sound_candidate,
        source: [FIB, "function caller() -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\nfunction wrapper() -> Nothing {\n    let x = fib(10) + 1\n    print(x)\n    caller()\n}\nfunction entrypoint() -> Nothing {\n    wrapper()\n}\n"].concat(),
        promoted: ["caller"],
        declined: []
    );
    // Case bug (ShadowsCrossingLocal): a host whose parameter is re-declared by a nested `let` is promoted and the checker then rejects it.
    promotion_case!(
        a_host_with_a_nested_parameter_shadow_declines,
        source: [FIB, "function host(seed: Int) -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    if (a > 0) {\n        let seed = a + b\n        print(seed)\n    }\n    print(a)\n    print(b)\n}\nfunction entrypoint() -> Nothing {\n    host(1)\n}\n"].concat(),
        promoted: [],
        declined: ["host"]
    );
    // Case bug: a lifted guard (v0.3-M5): an Array<Shape> with runtime fields crossing the join is still declined.
    promotion_case!(
        a_host_with_a_runtime_field_array_of_shapes_across_the_join_promotes,
        source: [FIB, "shape Item {\n    id: Int\n    qty: Int\n}\nfunction entrypoint() -> Nothing {\n    let qval = 10\n    let items: Array<Item> = [{ id: 1, qty: qval }, { id: 2, qty: qval }]\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n    let total = 0\n    for (it in items) {\n        total = total + it.qty\n    }\n    print(total)\n}\n"].concat(),
        promoted: ["entrypoint"],
        declined: []
    );
    // Case bug: the three wait-gated for-loop guards fire on loops with no `wait` and block a valid pair.
    promotion_case!(
        wait_free_for_loops_do_not_block_promotion,
        source: [FIB, "function makeArray() -> Array<Int> {\n    return [1, 2, 3]\n}\nfunction entrypoint() -> Nothing {\n    let sum = 0\n    for (i in range(0, 3)) {\n        sum = sum + i\n    }\n    let nums: Fixed<Int> = [10, 20, 30]\n    for (n in nums) {\n        sum = sum + n\n    }\n    for (m in makeArray()) {\n        sum = sum + m\n    }\n    print(sum)\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\n"].concat(),
        promoted: ["entrypoint"],
        declined: []
    );
    // Case bug (base_suspends lifted): a suspending host with a clean top-level CPU group no longer promotes once the skip is lifted.
    promotion_case!(
        a_suspending_host_with_a_clean_cpu_group_promotes,
        source: [FIB, "function entrypoint() -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    wait sleep(0)\n    print(a)\n    print(b)\n}\n"].concat(),
        promoted: ["entrypoint"],
        declined: []
    );
    // Case bug (E6 guard-probe extension): a suspending host whose own CPU callee is also called in a sub-expression (`fib(22) + 1`) stays promoted; the hazard shows only when the probe seeds this candidate's callee.
    promotion_case!(
        a_suspending_host_with_a_subexpression_call_to_its_cpu_callee_declines,
        source: [FIB, "function entrypoint() -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    let c = fib(22) + 1\n    wait sleep(0)\n    print(a + b + c)\n}\n"].concat(),
        promoted: [],
        declined: ["entrypoint"]
    );

    // A trivial non-suspending callee: waiting on it gives the host an explicit `wait` (so the
    // for-with-wait guard reaches it) without making it I/O-suspending, so it stays a CPU candidate.
    const TICK: &str = r#"
function tick() -> Int {
    return 1
}
"#;

    /// For-with-wait hosts: the shapes never compiled (the existing for-with-wait guard errors), so
    /// the contract is: not promoted, and the one pre-existing guard named by `guard_text` fires.
    macro_rules! for_with_wait_case {
        ($test_name:ident, source: $source:expr, guard_text: $guard_text:expr) => {
            // bug: a host whose `for` body waits on a stored range / fixed list / call-expression iterator is promoted into a state machine that cannot recover the iterator across the suspension, or promotion changes which guard error the user sees | tier: Medium
            #[test]
            fn $test_name() {
                let source: String = $source;
                let promoted = promoted_functions(&source);
                assert!(!promoted.contains("entrypoint"), "got {promoted:?}");
                let whats = error_whats(&source);
                assert!(
                    whats.iter().any(|what| what.contains($guard_text)),
                    "must hit the guard {:?}; got errors {whats:?}",
                    $guard_text
                );
            }
        };
    }

    // Case bug: the StoredRangeWithWait guard (iterator is a stored `range` variable).
    for_with_wait_case!(
        a_stored_range_for_loop_that_waits_declines_promotion,
        source: [FIB, TICK, "function entrypoint() -> Nothing {\n    let r = range(0, 3)\n    for (i in r) {\n        wait tick()\n    }\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\n"].concat(),
        guard_text: "stored range variable cannot yet be the iterator"
    );
    // Case bug: the FixedArrayIterWithWait guard (iterator is a `Fixed<T>` array).
    for_with_wait_case!(
        a_fixed_array_for_loop_that_waits_declines_promotion,
        source: [FIB, TICK, "function entrypoint() -> Nothing {\n    let nums: Fixed<Int> = [10, 20, 30]\n    for (n in nums) {\n        wait tick()\n    }\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\n"].concat(),
        guard_text: "`Fixed<T>` array cannot be the iterator"
    );
    // Case bug: the ExpressionIterWithWait guard (iterator is a non-`range` call expression).
    for_with_wait_case!(
        a_call_expression_for_loop_that_waits_declines_promotion,
        source: [FIB, TICK, "function makeArray() -> Array<Int> {\n    return [1, 2, 3]\n}\nfunction entrypoint() -> Nothing {\n    for (m in makeArray()) {\n        wait tick()\n    }\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\n"].concat(),
        guard_text: "call-expression iterator cannot yet be used"
    );

    // bug: cpu_promotion_query ignores the `--no-auto-parallel` kill switch (YNZ_NO_AUTO_PARALLEL), so the user's off switch does not turn promotion off | tier: Medium
    #[test]
    fn the_no_auto_parallel_switch_declines_every_promotion() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let source = [
            FIB,
            "function entrypoint() -> Nothing {\n    let a = fib(20)\n    let b = fib(21)\n    print(a)\n    print(b)\n}\n",
        ]
        .concat();

        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        let mut db_without_switch = CompilerDb::default();
        let file_without_switch = source_file(&mut db_without_switch, "envtest.ynz", &source);
        let promoted_without_switch = cpu_promotion_query(&db_without_switch, file_without_switch);
        assert!(
            promoted_without_switch.promoted.contains("entrypoint"),
            "without the switch the host should promote; got: {:?}",
            promoted_without_switch.promoted
        );

        // A fresh db, so the salsa cache re-evaluates with the env var visible.
        std::env::set_var("YNZ_NO_AUTO_PARALLEL", "1");
        let mut db_with_switch = CompilerDb::default();
        let file_with_switch = source_file(&mut db_with_switch, "envtest2.ynz", &source);
        let promoted_with_switch = cpu_promotion_query(&db_with_switch, file_with_switch);
        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        assert!(
            promoted_with_switch.promoted.is_empty(),
            "with the switch the query must decline; got: {:?}",
            promoted_with_switch.promoted
        );
    }

    // bug: an importer's call to an imported loop/recursion callee is not worth-it, because the callee's `does_real_work` flag does not ride the export table (cross-module propagation) | tier: Medium
    #[test]
    fn an_imported_real_work_callee_promotes_its_importer() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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

        let dependency_source = r#"
export function heavy(n: Int) -> Int {
    let total = 0
    let i = 0
    while (i < n) {
        total = total + i
        i = i + 1
    }
    return total
}
"#;
        let main_source = r#"
import { heavy } from `lib`
function entrypoint() -> Nothing {
    let a = heavy(20)
    let b = heavy(21)
    print(a)
    print(b)
}
"#;
        let dependency_path = dir.join("lib.ynz");
        let main_path = dir.join("entrypoint.ynz");
        std::fs::write(&dependency_path, dependency_source).expect("write dep");
        std::fs::write(&main_path, main_source).expect("write main");

        let mut db = CompilerDb::default();
        source_file(
            &mut db,
            &dependency_path.display().to_string(),
            dependency_source,
        );
        let main_file = source_file(&mut db, &main_path.display().to_string(), main_source);

        std::env::remove_var("YNZ_NO_AUTO_PARALLEL");
        let promoted = cpu_promotion_query(&db, main_file);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            promoted.promoted.contains("entrypoint"),
            "importer must promote calls to an imported real-work callee; got: {:?}",
            promoted.promoted
        );
    }
}
