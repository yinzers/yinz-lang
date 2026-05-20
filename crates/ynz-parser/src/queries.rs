use std::sync::Arc;

use ynz_ast::nodes::Module;
use ynz_diagnostics::DiagnosticBucket;

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
    pub diagnostics: DiagnosticBucket,
}

/// Lex a source file.
///
/// This is a salsa-tracked query: when `source.text` or `source.path` changes,
/// salsa automatically re-runs the lex and invalidates all downstream queries
/// that depend on the result.
// lru = 128: lex is cheap to recompute; keep more results in the salsa cache.
#[salsa::tracked(lru = 128)]
pub fn lex_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<LexOutput> {
    let path = source.path(db);
    let text = source.text(db);
    let (tokens, diagnostics) = lexer::lex(path.as_str(), text.as_str());
    Arc::new(LexOutput {
        tokens,
        diagnostics,
    })
}

/// Convenience: lex a source and return the diagnostics bucket directly.
pub fn lex_to_bucket(db: &dyn salsa::Database, source: SourceFile) -> DiagnosticBucket {
    lex_query(db, source).diagnostics.clone()
}

/// The output of the parse pass: an AST module + any parse-level diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct ParseOutput {
    pub module: Module,
    pub diagnostics: DiagnosticBucket,
}

/// Parse a source file.
///
/// This is a salsa-tracked query that depends on `lex_query`. Changing the source
/// text will re-run both the lexer and the parser automatically.
// lru = 128: parse is cheap; keep more results in the salsa cache.
#[salsa::tracked(lru = 128)]
pub fn parse_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<ParseOutput> {
    let lex = lex_query(db, source);
    let path = source.path(db);

    let mut parser = Parser::new(path.as_str(), &lex.tokens);
    let module = parser.parse_module();

    let mut diagnostics = lex.diagnostics.clone();
    for d in parser.diags.into_iter() {
        diagnostics.push(d);
    }

    Arc::new(ParseOutput {
        module,
        diagnostics,
    })
}
