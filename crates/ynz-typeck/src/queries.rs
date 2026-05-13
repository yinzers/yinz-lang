use std::sync::Arc;

use ynz_diagnostics::Diagnostic;
use ynz_parser::{parse_query, SourceFile};

use crate::{
    check::{check, TypedModule},
    intrinsics::PrimitiveIntrinsicTable,
    signatures::{collect_signatures, SignatureTable},
};

/// Output of the signature pre-pass.
#[derive(Clone, Debug, PartialEq)]
pub struct SignatureOutput {
    pub sig_table: SignatureTable,
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

/// The output of the type-check pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckOutput {
    pub typed_module: TypedModule,
    pub diagnostics: Vec<Diagnostic>,
}

/// Pass 1: collect all function signatures from the module.
///
/// Validates:
/// - Duplicate function names
/// - `main` exists with `() -> nothing` signature
///
/// This is a separate salsa query so body-only changes don't re-run the
/// signature pass, and signature changes correctly cascade to body checks
/// of all callers.
#[salsa::tracked]
pub fn module_signatures_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<SignatureOutput> {
    let parse = parse_query(db, source);
    let mut diag_bucket = ynz_diagnostics::DiagnosticBucket::new();
    let sig_table = collect_signatures(&parse.module, &mut diag_bucket);
    Arc::new(SignatureOutput {
        sig_table,
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

    let (typed, check_diags) = check(
        &parse.module,
        &sig_output.sig_table,
        &PrimitiveIntrinsicTable::m3(),
    );
    all_diags.extend(check_diags.into_iter());

    Arc::new(CheckOutput {
        typed_module: typed,
        diagnostics: all_diags,
    })
}
