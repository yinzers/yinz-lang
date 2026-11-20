//! Where the parser read a built-in-looking name as a TYPE, for tools that rewrite source text.
//!
//! The compiler never turns this on: [`Parser`](crate::parser::Parser) records nothing unless
//! [`Parser::record_type_positions`](crate::parser::Parser::record_type_positions) was called,
//! so a normal compile pays one `Option` check per type name. The rename tool
//! (`ynz-pascalcase-codemod`) calls [`find_type_positions`], which needs the parser's own
//! decision about what is a type: `map` is a type in `map<String, Int>` and a method in
//! `rows.map(...)`, and only the parser knows which.

use ynz_diagnostics::{DiagnosticBucket, SourceSpan};

use crate::lexer;
use crate::queries::parse_lexed;
use crate::LexOutput;

/// Source spans the parser read as types. Each span covers exactly the identifier.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypePositions {
    /// Every identifier consumed as a type name: annotations, return types, shape fields,
    /// generic arguments, union members and `is` targets.
    pub type_names: Vec<SourceSpan>,
    /// The callee of every call whose callee is a bare identifier (`channel<Int>(8)`,
    /// `range(0, 3)`). The callee may be a built-in type in call position or a function; the
    /// caller decides by name.
    pub call_callees: Vec<SourceSpan>,
    /// `receiver.field` reads (no call) whose receiver is a bare identifier: the shape of a
    /// type-attached constant read such as `Int.max`. The receiver may equally be a variable;
    /// the caller decides.
    pub constant_read_receivers: Vec<ConstantReadReceiver>,
}

/// One `receiver.field` read whose receiver is a bare identifier.
#[derive(Clone, Debug, PartialEq)]
pub struct ConstantReadReceiver {
    pub receiver_name: String,
    pub receiver_span: SourceSpan,
    pub field_name: String,
}

/// What [`find_type_positions`] saw in one source text.
#[derive(Debug)]
pub struct TypePositionsOutput {
    pub positions: TypePositions,
    /// Lexer and parser diagnostics, in the order they were raised. An old lowercase built-in
    /// spelling in a type position is among them (`OldBuiltinTypeName`); that is expected input
    /// for a rename tool, not a parse failure.
    pub diagnostics: DiagnosticBucket,
}

/// Lex and parse `source` with position recording on. O(n) time and memory in the source
/// length. Never panics on its own; the parser may on pathological input, so callers that
/// read untrusted files should catch unwinds.
pub fn find_type_positions(file_label: &str, source: &str) -> TypePositionsOutput {
    let (tokens, diagnostics) = lexer::lex(file_label, source);
    let lexed = LexOutput {
        tokens,
        diagnostics,
    };
    let (parsed, positions) = parse_lexed(file_label, &lexed, true);
    TypePositionsOutput {
        positions: positions.unwrap_or_default(),
        diagnostics: parsed.diagnostics,
    }
}
