use std::sync::Arc;

use ynz_ast::nodes::Module;
use ynz_diagnostics::{Diagnostic, DiagnosticBucket};

use crate::{
    db::SourceFile,
    lexer,
    parser::Parser,
    token::{Spanned, Token},
};

/// The output of a successful lex pass: token stream + any lexer-level diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct LexOutput {
    pub tokens: Vec<Spanned<Token>>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lex a source file.
///
/// This is a salsa-tracked query: when `source.text` or `source.path` changes,
/// salsa automatically re-runs the lex and invalidates all downstream queries
/// that depend on the result.
#[salsa::tracked]
pub fn lex_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<LexOutput> {
    let path = source.path(db);
    let text = source.text(db);
    let (tokens, bucket) = lexer::lex(path.as_str(), text.as_str());
    let diagnostics: Vec<Diagnostic> = bucket.into_iter().collect();
    Arc::new(LexOutput {
        tokens,
        diagnostics,
    })
}

/// Convenience: lex a source and return the diagnostics bucket directly.
pub fn lex_to_bucket(db: &dyn salsa::Database, source: SourceFile) -> DiagnosticBucket {
    let output = lex_query(db, source);
    let mut bucket = DiagnosticBucket::new();
    for diag in &output.diagnostics {
        bucket.push(diag.clone());
    }
    bucket
}

/// The output of the parse pass: an AST module + any parse-level diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseOutput {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a source file.
///
/// This is a salsa-tracked query that depends on `lex_query`. Changing the source
/// text will re-run both the lexer and the parser automatically.
#[salsa::tracked]
pub fn parse_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<ParseOutput> {
    let lex = lex_query(db, source);
    let path = source.path(db);

    let mut parser = Parser::new(path.as_str(), &lex.tokens);
    let module = parser.parse_module();

    let mut diagnostics: Vec<Diagnostic> = lex.diagnostics.clone();
    diagnostics.extend(parser.diags.into_iter());

    Arc::new(ParseOutput {
        module,
        diagnostics,
    })
}
