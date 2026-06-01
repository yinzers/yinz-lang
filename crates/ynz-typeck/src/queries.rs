use std::{collections::HashSet, sync::Arc};

use ynz_ast::nodes::{ImportDecl, ImportKind, Item};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, DiagnosticKind};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    check::{check, TypedModule},
    exports::{collect_exports, ExportTable},
    generics::{GenericFnTable, GenericShapeTable, MonomorphizationTable},
    may_block,
    intrinsics::PrimitiveIntrinsicTable,
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
}

/// Pass 1: collect all shape declarations and function signatures from the module,
/// including symbols imported from other files.
///
/// Cross-file import resolution happens here so shape field type annotations
/// can reference imported shapes and options types.
// lru = 128: signature computation is moderate cost; keep more results cached.
#[salsa::tracked(lru = 128)]
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

/// Pass 2: type-check all function bodies.
///
/// Depends on `module_signatures_query` for the signature table.
/// Depends on `parse_query` for the AST.
// lru = 64: typechecking is moderately expensive; smaller cap than parse/signatures.
#[salsa::tracked(lru = 64)]
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
    // The analysis sees only this compilation unit — cross-module calls produce
    // `UnresolvableEdge::CrossModule` entries recorded in the analysis result.
    // The body checker gates can't-infer diagnostics on `current_fn_suspends`
    // (set from sig.suspends): only callers that independently suspend AND make
    // an unanalyzable boundary call receive the error.
    let imported_fn_names: HashSet<String> = sig_output.imported_fns.keys().cloned().collect();
    let may_block_result = may_block::analyze(&parse.module, &imported_fn_names);
    for (name, sig) in merged_sig_table.fns.iter_mut() {
        sig.suspends = may_block_result.suspends.contains(name.as_str());
    }

    let (typed, mono_table, check_diags, referenced_names) = check(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &PrimitiveIntrinsicTable::m6().with_m2_internals(),
        &sig_output.imported_options,
        imported_fn_names,
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

    Arc::new(CheckOutput {
        typed_module: typed,
        mono_table,
        diagnostics: all_diags,
    })
}
