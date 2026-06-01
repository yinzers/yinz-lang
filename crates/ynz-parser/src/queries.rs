use std::sync::Arc;

use ynz_ast::nodes::{Item, Module};
use ynz_diagnostics::{DiagnosticBucket, DiagnosticKind, SourceSpan};

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

/// Collect source spans where a banned-declaration keyword is used as a plain
/// identifier. Currently covers shape field names and function parameter names.
/// BannedKeyword diagnostics at these positions are suppressed in `parse_query`.
fn valid_identifier_positions(module: &Module) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    for item in &module.items {
        match item {
            Item::ShapeDecl(s) => {
                for field in &s.fields {
                    spans.push(field.name_span.clone());
                }
            }
            Item::Function(f) => {
                for param in &f.params {
                    spans.push(param.name_span.clone());
                }
            }
            _ => {}
        }
    }
    spans
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

    // Collect positions where banned-declaration-keyword tokens are valid plain
    // identifiers (shape field names, function parameter names). A BannedKeyword
    // diagnostic at one of these positions is suppressed: `class: string` as a
    // field name is correct Yinz — `class` is only banned as a declaration keyword.
    let suppress_spans = valid_identifier_positions(&module);

    let mut diagnostics = DiagnosticBucket::new();
    for d in lex.diagnostics.iter() {
        let suppressed = matches!(&d.kind, Some(DiagnosticKind::BannedKeyword { .. }))
            && suppress_spans
                .iter()
                .any(|s| s.start == d.span.start && s.end == d.span.end);
        if !suppressed {
            diagnostics.push(d.clone());
        }
    }
    for d in parser.diags.into_iter() {
        diagnostics.push(d);
    }

    Arc::new(ParseOutput {
        module,
        diagnostics,
    })
}
