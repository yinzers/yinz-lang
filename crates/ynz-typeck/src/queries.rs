use std::sync::Arc;

use ynz_diagnostics::Diagnostic;
use ynz_parser::{parse_query, SourceFile};

use crate::{
    check::{check, TypedModule},
    generics::{GenericFnTable, GenericShapeTable, MonomorphizationTable},
    intrinsics::PrimitiveIntrinsicTable,
    shapes::{collect_generic_shapes, collect_shapes, ShapeTable},
    signatures::{collect_generic_signatures, collect_signatures, SignatureTable},
};

/// Output of the signature pre-pass.
#[derive(Clone, Debug, PartialEq)]
pub struct SignatureOutput {
    pub sig_table: SignatureTable,
    pub shape_table: ShapeTable,
    pub generic_fn_table: GenericFnTable,
    pub generic_shape_table: GenericShapeTable,
    pub diagnostics: Vec<Diagnostic>,
}

impl SignatureOutput {
    pub fn sig_table(&self) -> &SignatureTable {
        &self.sig_table
    }
}

/// Allow SignatureTable to derive PartialEq for salsa.
impl PartialEq for SignatureTable {
    fn eq(&self, other: &Self) -> bool {
        // Two signature tables are equal if they have the same set of function names.
        // Fine-grained per-body salsa incrementality (v0.2 LSP work) would compare
        // individual sigs; for now, same-key-set is the coarse check.
        self.fns.len() == other.fns.len()
            && self.fns.keys().all(|k| other.fns.contains_key(k))
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

/// Pass 1: collect all shape declarations and function signatures from the module.
///
/// Validates:
/// - Duplicate shape names, duplicate field names, field type cycles
/// - Duplicate function names
/// - `main` exists with `() -> nothing` signature
///
/// Shapes are collected before signatures so function signatures can reference
/// shape types (e.g. `function greet(share self: Player) -> string`).
#[salsa::tracked]
pub fn module_signatures_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<SignatureOutput> {
    let parse = parse_query(db, source);
    let mut diag_bucket = ynz_diagnostics::DiagnosticBucket::new();
    // Shapes first — function signatures need them for type resolution.
    let shape_table = collect_shapes(&parse.module, &mut diag_bucket);
    let generic_shape_table = collect_generic_shapes(&parse.module, &mut diag_bucket);
    let sig_table = collect_signatures(&parse.module, &mut diag_bucket, &shape_table);
    let generic_fn_table = collect_generic_signatures(&parse.module, &mut diag_bucket, &shape_table);
    Arc::new(SignatureOutput {
        sig_table,
        shape_table,
        generic_fn_table,
        generic_shape_table,
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

    let (typed, mono_table, check_diags) = check(
        &parse.module,
        &sig_output.sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &PrimitiveIntrinsicTable::m3(),
    );
    all_diags.extend(check_diags.into_iter());

    Arc::new(CheckOutput {
        typed_module: typed,
        mono_table,
        diagnostics: all_diags,
    })
}
