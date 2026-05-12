use std::sync::Arc;

use ynz_diagnostics::Diagnostic;
use ynz_parser::{parse_query, SourceFile};

use crate::{
    builtins::BuiltinTable,
    check::{check, TypedModule},
};

/// The output of the type-check pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckOutput {
    pub typed_module: TypedModule,
    pub diagnostics: Vec<Diagnostic>,
}

/// Type-check a source file.
///
/// This salsa-tracked query depends on `parse_query`. Changing the source
/// text invalidates both the parse and type-check results.
#[salsa::tracked]
pub fn check_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<CheckOutput> {
    let parse = parse_query(db, source);

    // Collect all diagnostics from lex and parse.
    let mut all_diags: Vec<Diagnostic> = parse.diagnostics.clone();

    // Run the type checker.
    let (typed, check_diags) = check(&parse.module, &BuiltinTable::m1());
    all_diags.extend(check_diags.into_iter());

    Arc::new(CheckOutput {
        typed_module: typed,
        diagnostics: all_diags,
    })
}
