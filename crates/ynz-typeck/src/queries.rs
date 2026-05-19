use std::{collections::HashSet, sync::Arc};

use ynz_ast::nodes::{ImportDecl, Item};
use ynz_diagnostics::Diagnostic;
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{
    check::{check, TypedModule},
    exports::collect_exports,
    generics::{GenericFnTable, GenericShapeTable, MonomorphizationTable},
    intrinsics::PrimitiveIntrinsicTable,
    options_table::collect_options,
    resolve_import::resolve_imports,
    shapes::{collect_generic_shapes, collect_shapes, ShapeTable},
    signatures::{collect_generic_signatures, collect_signatures, FunctionSig, SignatureTable},
};

/// Output of the signature pre-pass.
#[derive(Clone, Debug)]
pub struct SignatureOutput {
    pub sig_table: SignatureTable,
    pub shape_table: ShapeTable,
    pub generic_fn_table: GenericFnTable,
    pub generic_shape_table: GenericShapeTable,
    /// Imported function signatures visible in function bodies.
    pub imported_fns: std::collections::HashMap<String, FunctionSig>,
    /// Imported options types visible in function bodies (for options value expressions).
    pub imported_options: std::collections::HashMap<String, crate::options_table::OptionsEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

impl PartialEq for SignatureOutput {
    fn eq(&self, other: &Self) -> bool {
        self.sig_table == other.sig_table
            && self.shape_table == other.shape_table
            && self.generic_fn_table == other.generic_fn_table
            && self.generic_shape_table == other.generic_shape_table
            && self.imported_fns.len() == other.imported_fns.len()
            && self.imported_fns.keys().all(|k| other.imported_fns.contains_key(k))
    }
}

impl SignatureOutput {
    pub fn sig_table(&self) -> &SignatureTable {
        &self.sig_table
    }
}

/// Allow SignatureTable to derive PartialEq for salsa.
impl PartialEq for SignatureTable {
    fn eq(&self, other: &Self) -> bool {
        self.fns.len() == other.fns.len() && self.fns.keys().all(|k| other.fns.contains_key(k))
    }
}

/// Allow ShapeTable to derive PartialEq for salsa (coarse: same shape names).
impl PartialEq for ShapeTable {
    fn eq(&self, other: &Self) -> bool {
        self.shapes.len() == other.shapes.len()
            && self.shapes.keys().all(|k| other.shapes.contains_key(k))
    }
}

impl PartialEq for GenericFnTable {
    fn eq(&self, other: &Self) -> bool {
        self.fns.len() == other.fns.len() && self.fns.keys().all(|k| other.fns.contains_key(k))
    }
}

impl PartialEq for GenericShapeTable {
    fn eq(&self, other: &Self) -> bool {
        self.shapes.len() == other.shapes.len()
            && self.shapes.keys().all(|k| other.shapes.contains_key(k))
    }
}

/// The output of the type-check pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckOutput {
    pub typed_module: TypedModule,
    pub mono_table: MonomorphizationTable,
    pub diagnostics: Vec<Diagnostic>,
}

impl PartialEq for MonomorphizationTable {
    fn eq(&self, other: &Self) -> bool {
        self.entries.len() == other.entries.len()
    }
}

/// Pass 1: collect all shape declarations and function signatures from the module,
/// including symbols imported from other files.
///
/// Cross-file import resolution happens here so shape field type annotations
/// can reference imported shapes and options types.
#[salsa::tracked]
pub fn module_signatures_query(
    db: &dyn salsa::Database,
    source: SourceFile,
) -> Arc<SignatureOutput> {
    let parse = parse_query(db, source);
    let mut diag_bucket = ynz_diagnostics::DiagnosticBucket::new();

    // Collect import declarations from this module.
    let import_decls: Vec<&ImportDecl> = parse.module.items.iter().filter_map(|i| {
        if let Item::ImportDecl(d) = i { Some(d) } else { None }
    }).collect();

    // Resolve imports to get cross-file shapes, options, and functions.
    // Uses a passed-in visiting set to detect circular imports.
    let mut visiting: HashSet<std::path::PathBuf> = HashSet::new();
    let importer_path = source.path(db);
    let importer_path_str: &str = &importer_path;

    // Import resolution reads imported files from disk using the SAME salsa db.
    // Using the same db avoids "Cannot change database mid-query" panics from salsa.
    let imported = resolve_imports(&import_decls, importer_path_str, db, &mut visiting, &mut diag_bucket);

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
        diagnostics: diag_bucket.into_iter().collect(),
    })
}

/// Pass 2: type-check all function bodies.
///
/// Depends on `module_signatures_query` for the signature table.
/// Depends on `parse_query` for the AST.
#[salsa::tracked]
pub fn check_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<CheckOutput> {
    let parse = parse_query(db, source);
    let sig_output = module_signatures_query(db, source);

    let mut all_diags: Vec<Diagnostic> = parse.diagnostics.clone();
    all_diags.extend(sig_output.diagnostics.clone());

    // Merge imported functions into the local signature table so function bodies
    // can call imported functions by name.
    let mut merged_sig_table = sig_output.sig_table.clone();
    for (name, sig) in &sig_output.imported_fns {
        // Local declarations take priority — don't override with imported.
        merged_sig_table.fns.entry(name.clone()).or_insert_with(|| sig.clone());
    }

    let (typed, mono_table, check_diags) = check(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &PrimitiveIntrinsicTable::m6(),
        &sig_output.imported_options,
    );
    all_diags.extend(check_diags.into_iter());

    Arc::new(CheckOutput {
        typed_module: typed,
        mono_table,
        diagnostics: all_diags,
    })
}
