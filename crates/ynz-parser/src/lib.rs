//! Yinz lexer and parser.
//!
//! # Pipeline
//!
//! Source text → [`lex_query`] → token stream → [`parse_query`] → AST [`Module`].
//!
//! Both queries are Salsa-tracked: re-running on unchanged input returns a cached result
//! without re-lexing or re-parsing.
//!
//! # Entry points
//!
//! - [`lex_query`] — tokenise a [`SourceFile`]; returns [`LexOutput`] with tokens +
//!   any lex diagnostics.
//! - [`parse_query`] — parse a [`SourceFile`]; calls `lex_query` internally and returns
//!   [`ParseOutput`] with the root [`ynz_ast::nodes::Module`] and diagnostics.
//! - [`infix_bp`] — exported for use by the type-checker's operator-precedence tests.
//!
//! # Key types
//!
//! - [`CompilerDb`] / [`SourceFileRegistry`] — the Salsa database trait and its default
//!   implementation; callers create one `CompilerDb` per compilation session.
//! - [`SourceFile`] — a Salsa input holding a file path and source text. Create with
//!   [`SourceFile::new`] and register via [`CompilerDb::register_source`] before running
//!   cross-file queries.
//! - [`Token`] / [`Spanned`] — the token type and its source-span wrapper.

pub mod db;
pub mod lexer;
pub mod parser;
pub mod queries;
pub mod token;
pub mod trivia;
pub use parser::infix_bp;

pub use db::{CompilerDb, SourceFile, SourceFileRegistry};
pub use lexer::lex_with_trivia;
pub use queries::{lex_query, parse_query, LexOutput, ParseOutput};
pub use token::{Spanned, Token};
pub use trivia::{Comment, CommentKind};
