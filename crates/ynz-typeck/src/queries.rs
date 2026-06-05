use std::{collections::HashSet, sync::Arc};

use ynz_ast::nodes::{Expr, ImportDecl, ImportKind, Item, Module, Stmt};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, DiagnosticKind};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    check::{check, TypedModule},
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
        },
        mono_table: crate::generics::MonomorphizationTable {
            entries: std::collections::HashMap::new(),
        },
        diagnostics: DiagnosticBucket::new(),
        suspends_set: std::collections::HashSet::new(),
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
        .filter_map(|(name, sig)| if sig.suspends { Some(name.clone()) } else { None })
        .collect();
    let may_block_result =
        may_block::analyze(&parse.module, &imported_fn_names, &imported_suspending_names);

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

    // M3e guard: universally loud-reject ALL calls to imported suspending functions.
    //
    // The scalar `composed_frame_size` shortcut (carrying a single u64 across the
    // module boundary) is provably insufficient for composing the caller's state-machine
    // frame: the importing module needs the callee's FULL FrameLayout (exact slot offsets
    // for crossing locals, the complete sub-frame tree, EC staging-slot position) to embed
    // the foreign sub-frame correctly. The scalar value cannot reconstruct any of that.
    //
    // The predictive `composed_frame_simple` guard (rounds 1-2 of M3b) tried to predict
    // WHICH cases were safe, but escaped 5 times (number crossing-local, transitive×caller-
    // frame, shape, EC×transitive, and others) because the typeck-side analysis is a
    // different and shallower algorithm than codegen's real `build_frame_layouts`. A guard
    // that allows NOTHING cannot be wrong.
    //
    // The correct fix — serializing the full FrameLayout into the export table — ships in
    // `v0-3-m3e-cross-module-frame-serialization`. This universal reject is the provably
    // sound floor until M3e lands.
    {
        // Every imported function with suspends=true is loud-rejected when called.
        let suspending_imported_names: HashSet<&str> = sig_output
            .imported_fns
            .iter()
            .filter(|(_, sig)| sig.suspends)
            .map(|(name, _)| name.as_str())
            .collect();

        if !suspending_imported_names.is_empty() {
            for item in &parse.module.items {
                let Item::Function(fn_decl) = item else { continue };
                emit_loud_reject_for_imported_suspending_calls(
                    &fn_decl.body.stmts,
                    &suspending_imported_names,
                    &mut all_diags,
                );
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
    })
}

/// Walk a function body and emit a loud-reject compile error for every direct call
/// to any imported suspending function.
///
/// Composing the caller's state-machine frame requires the callee's full FrameLayout
/// (exact crossing-local slot offsets, sub-frame tree, EC staging-slot position).
/// A scalar `composed_frame_size` value cannot reconstruct any of that — every
/// cross-module suspending call is unsafe until M3e serializes the full layout.
///
/// Called from `check_query` after the may-block fixpoint resolves which imported
/// functions suspend. Only fires when at least one imported suspending function
/// exists in the module's import set.
fn emit_loud_reject_for_imported_suspending_calls(
    stmts: &[Stmt],
    suspending_imported_names: &HashSet<&str>,
    diags: &mut DiagnosticBucket,
) {
    for stmt in stmts {
        emit_loud_reject_in_stmt(stmt, suspending_imported_names, diags);
    }
}

fn emit_loud_reject_in_stmt(
    stmt: &Stmt,
    suspending_imported_names: &HashSet<&str>,
    diags: &mut DiagnosticBucket,
) {
    match stmt {
        Stmt::Expr(e) | Stmt::Return { value: Some(e), .. } => {
            emit_loud_reject_in_expr(e, suspending_imported_names, diags);
        }
        Stmt::Return { value: None, .. } => {}
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            emit_loud_reject_in_expr(value, suspending_imported_names, diags);
        }
        Stmt::FieldAssign { target, value, .. } => {
            emit_loud_reject_in_expr(target, suspending_imported_names, diags);
            emit_loud_reject_in_expr(value, suspending_imported_names, diags);
        }
        Stmt::IndexAssign { receiver, index, value, .. } => {
            emit_loud_reject_in_expr(receiver, suspending_imported_names, diags);
            emit_loud_reject_in_expr(index, suspending_imported_names, diags);
            emit_loud_reject_in_expr(value, suspending_imported_names, diags);
        }
        Stmt::If { cond, body, .. } => {
            emit_loud_reject_in_expr(cond, suspending_imported_names, diags);
            emit_loud_reject_for_imported_suspending_calls(&body.stmts, suspending_imported_names, diags);
        }
        Stmt::While { cond, body, .. } => {
            emit_loud_reject_in_expr(cond, suspending_imported_names, diags);
            emit_loud_reject_for_imported_suspending_calls(&body.stmts, suspending_imported_names, diags);
        }
        Stmt::For { iter, body, .. } => {
            emit_loud_reject_in_expr(iter, suspending_imported_names, diags);
            emit_loud_reject_for_imported_suspending_calls(&body.stmts, suspending_imported_names, diags);
        }
        Stmt::Match { scrutinee, arms, else_arm, .. } => {
            emit_loud_reject_in_expr(scrutinee, suspending_imported_names, diags);
            for arm in arms {
                emit_loud_reject_for_imported_suspending_calls(
                    &arm.body.stmts, suspending_imported_names, diags,
                );
            }
            if let Some(eb) = else_arm {
                emit_loud_reject_for_imported_suspending_calls(
                    &eb.stmts, suspending_imported_names, diags,
                );
            }
        }
    }
}

fn emit_loud_reject_in_expr(
    expr: &Expr,
    suspending_imported_names: &HashSet<&str>,
    diags: &mut DiagnosticBucket,
) {
    match expr {
        Expr::Call(call_expr) => {
            // Emit the reject diagnostic for any direct call to an imported suspending fn.
            if let Expr::Ident(name, span) = &call_expr.callee {
                if suspending_imported_names.contains(name.as_str()) {
                    diags.push(Diagnostic::error(
                        span.clone(),
                        format!(
                            "Calling `{name}` across a module boundary isn't supported yet — \
                             it (or a function it calls) suspends, and composing the caller's \
                             saved state needs the callee's exact memory layout, which isn't \
                             carried across modules yet."
                        ),
                        format!(
                            "Move `{name}` into this module, or avoid suspending it across the \
                             boundary, until v0.3-M3e ships cross-module frame-layout serialization."
                        ),
                        "The caller's state machine needs the callee's full slot map — exact \
                         positions for every variable that survives a pause, plus any nested \
                         calls — to embed the callee's saved-state block at the right offset. \
                         A scalar size alone cannot reconstruct that. Calling with a wrong \
                         layout silently corrupts memory; a clean error here is the safe floor. \
                         See design/future/cross-module-frame-serialization.md for the M3e plan.",
                    ));
                }
            }
            // Recurse into call arguments and callee expression.
            for arg in &call_expr.args {
                emit_loud_reject_in_expr(arg, suspending_imported_names, diags);
            }
            emit_loud_reject_in_expr(&call_expr.callee, suspending_imported_names, diags);
        }
        Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            emit_loud_reject_in_expr(inner, suspending_imported_names, diags);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            emit_loud_reject_in_expr(lhs, suspending_imported_names, diags);
            emit_loud_reject_in_expr(rhs, suspending_imported_names, diags);
        }
        Expr::UnaryOp { operand, .. } => {
            emit_loud_reject_in_expr(operand, suspending_imported_names, diags);
        }
        Expr::MethodCall { receiver, args, .. } => {
            emit_loud_reject_in_expr(receiver, suspending_imported_names, diags);
            for arg in args {
                emit_loud_reject_in_expr(arg, suspending_imported_names, diags);
            }
        }
        Expr::IndexAccess { receiver, index, .. } => {
            emit_loud_reject_in_expr(receiver, suspending_imported_names, diags);
            emit_loud_reject_in_expr(index, suspending_imported_names, diags);
        }
        Expr::FieldAccess { receiver, .. } => {
            emit_loud_reject_in_expr(receiver, suspending_imported_names, diags);
        }
        Expr::StructLit { fields, .. } => {
            for field in fields {
                emit_loud_reject_in_expr(&field.value, suspending_imported_names, diags);
            }
        }
        Expr::ArrayLit { elements, .. } => {
            for el in elements {
                emit_loud_reject_in_expr(el, suspending_imported_names, diags);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                emit_loud_reject_in_expr(k, suspending_imported_names, diags);
                emit_loud_reject_in_expr(v, suspending_imported_names, diags);
            }
        }
        Expr::Is { expr: inner, .. } | Expr::PostfixOp { receiver: inner, .. } => {
            emit_loud_reject_in_expr(inner, suspending_imported_names, diags);
        }
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    emit_loud_reject_in_expr(e, suspending_imported_names, diags);
                }
            }
        }
        // Leaf nodes — no sub-expressions that could be calls.
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
