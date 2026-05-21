use lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensOptions,
    SemanticTokensServerCapabilities,
};
use ynz_parser::token::Token;

use crate::{capabilities::PositionEncoding, position::LineTable, state::ServerState};

// ─── Legend ──────────────────────────────────────────────────────────────────

/// The ordered list of token types this server emits.
///
/// The index of each entry in this array is the `tokenType` value sent to the
/// client in `SemanticToken.token_type`. Clients map index → theme color using
/// the legend advertised during `initialize`. This array MUST stay in sync with
/// the `IDX_*` constants below.
pub const SEMANTIC_TOKEN_LEGEND: &[SemanticTokenType] = &[
    SemanticTokenType::KEYWORD,     // 0
    SemanticTokenType::TYPE,        // 1
    SemanticTokenType::FUNCTION,    // 2
    SemanticTokenType::VARIABLE,    // 3
    SemanticTokenType::PARAMETER,   // 4 — unused today; reserved in legend for future AST-walk upgrade
    SemanticTokenType::PROPERTY,    // 5 — unused today; reserved for field access
    SemanticTokenType::ENUM_MEMBER, // 6
    SemanticTokenType::NUMBER,      // 7
    SemanticTokenType::STRING,      // 8
    SemanticTokenType::COMMENT,     // 9
];

const IDX_KEYWORD: u32 = 0;
const IDX_TYPE: u32 = 1;
const IDX_FUNCTION: u32 = 2;
const IDX_VARIABLE: u32 = 3;
const IDX_ENUM_MEMBER: u32 = 6;
const IDX_NUMBER: u32 = 7;
const IDX_STRING: u32 = 8;
const IDX_COMMENT: u32 = 9;

/// Build the `semanticTokensProvider` capability advertisement.
pub fn semantic_tokens_options() -> SemanticTokensServerCapabilities {
    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
        legend: lsp_types::SemanticTokensLegend {
            token_types: SEMANTIC_TOKEN_LEGEND.to_vec(),
            token_modifiers: vec![],
        },
        full: Some(lsp_types::SemanticTokensFullOptions::Bool(true)),
        range: Some(true),
        work_done_progress_options: Default::default(),
    })
}

// ─── Full-file response ───────────────────────────────────────────────────────

/// Emit semantic tokens for the entire document.
///
/// Walks the lex output linearly — no AST traversal needed. Identifier
/// classification uses a two-level lookup:
///   1. Name starts with ASCII uppercase → `TYPE` (Golden Rule 13: capital = type).
///   2. Name appears in `sig_table` → `FUNCTION`.
///   3. Name appears in `options` `variants` list → `ENUM_MEMBER`.
///   4. Fallback → `VARIABLE`.
///
/// Time: O(T + I·log I) where T = token count, I = identifier count (hash lookups).
pub fn semantic_tokens_full_response(
    state: &ServerState,
    uri: &lsp_types::Url,
) -> Option<SemanticTokens> {
    let text = state.text_for(uri)?;
    let table = LineTable::new(text);
    let sf = state.source_file_for(uri)?;

    let lex = ynz_parser::queries::lex_query(&state.db, sf);
    let sig = ynz_typeck::queries::module_signatures_query(&state.db, sf);

    let raw = collect_tokens(&lex.tokens, text, &table, &sig, state.encoding, None);
    Some(SemanticTokens {
        result_id: None,
        data: encode_delta(raw),
    })
}

/// Emit semantic tokens for the visible viewport range only.
///
/// Time: O(T) where T = tokens in the viewport (range filter is applied
/// before classification, so out-of-range tokens skip the identifier lookup).
pub fn semantic_tokens_range_response(
    state: &ServerState,
    uri: &lsp_types::Url,
    range: lsp_types::Range,
) -> Option<SemanticTokens> {
    let text = state.text_for(uri)?;
    let table = LineTable::new(text);
    let sf = state.source_file_for(uri)?;

    let lex = ynz_parser::queries::lex_query(&state.db, sf);
    let sig = ynz_typeck::queries::module_signatures_query(&state.db, sf);

    let start_byte = table.position_to_byte_offset(text, range.start, state.encoding)?;
    let end_byte = table.position_to_byte_offset(text, range.end, state.encoding)?;

    let raw = collect_tokens(
        &lex.tokens,
        text,
        &table,
        &sig,
        state.encoding,
        Some(start_byte..end_byte),
    );
    Some(SemanticTokens {
        result_id: None,
        data: encode_delta(raw),
    })
}

// ─── Internal ─────────────────────────────────────────────────────────────────

/// A single absolute-position token (before delta encoding).
struct RawToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
}

/// Classify each lexed token and return absolute-position records.
///
/// `range_filter`: when `Some(start..end)`, only tokens whose byte offsets
/// intersect the range are included. Matches the viewport-filter semantic
/// used by inlay hints — inclusion is span-based so partial tokens at
/// the viewport edge ARE included.
fn collect_tokens(
    tokens: &[ynz_parser::token::Spanned<Token>],
    text: &str,
    table: &LineTable,
    sig: &ynz_typeck::queries::SignatureOutput,
    encoding: PositionEncoding,
    range_filter: Option<std::ops::Range<usize>>,
) -> Vec<RawToken> {
    let mut out = Vec::with_capacity(tokens.len());

    for spanned in tokens {
        let start = spanned.span.start;
        let end = spanned.span.end;

        // Viewport filter: include the token if its span overlaps the range.
        if let Some(ref r) = range_filter {
            if end <= r.start || start >= r.end {
                continue;
            }
        }

        let token_type = classify_token(&spanned.value, sig);
        let Some(token_type) = token_type else {
            continue; // structural tokens (parens, arrows, etc.) — no semantic type
        };

        let pos = table.byte_offset_to_position(text, start, encoding);
        let length = char_len(text, start, end, encoding);
        if length == 0 {
            continue;
        }

        out.push(RawToken {
            line: pos.line,
            start: pos.character,
            length,
            token_type,
        });
    }

    out
}

/// Classify a single token to an `IDX_*` value (or `None` for structural tokens).
fn classify_token(tok: &Token, sig: &ynz_typeck::queries::SignatureOutput) -> Option<u32> {
    match tok {
        // ── Keywords ──────────────────────────────────────────────────────────
        Token::Function
        | Token::Let
        | Token::Const
        | Token::Return
        | Token::If
        | Token::Else
        | Token::While
        | Token::For
        | Token::In
        | Token::Shape
        | Token::Follows
        | Token::Extends
        | Token::Base
        | Token::Hidden
        | Token::Dynamic
        | Token::Options
        | Token::Is
        | Token::Import
        | Token::Export
        | Token::Wait
        | Token::Background
        | Token::Sensitive
        | Token::Errors
        | Token::True
        | Token::False
        | Token::Nothing
        | Token::None
        | Token::SelfValue
        | Token::SelfType => Some(IDX_KEYWORD),

        // ── Literals ──────────────────────────────────────────────────────────
        Token::IntLit(_) | Token::NumberLit(_) => Some(IDX_NUMBER),
        Token::StringLit(_) | Token::BacktickString(_) | Token::InterpolationStart
        | Token::InterpolationEnd => Some(IDX_STRING),

        // ── Doc comment ───────────────────────────────────────────────────────
        Token::DocComment { .. } => Some(IDX_COMMENT),

        // ── Identifiers ───────────────────────────────────────────────────────
        Token::Identifier(name) => Some(classify_identifier(name, sig)),

        // ── Structural (no semantic type) ─────────────────────────────────────
        Token::LParen
        | Token::RParen
        | Token::LBrace
        | Token::RBrace
        | Token::LBracket
        | Token::RBracket
        | Token::Dot
        | Token::Comma
        | Token::Colon
        | Token::Eq
        | Token::Arrow
        | Token::FatArrow
        | Token::Plus
        | Token::Minus
        | Token::Star
        | Token::Slash
        | Token::Percent
        | Token::EqEq
        | Token::NotEq
        | Token::Lt
        | Token::LtEq
        | Token::Gt
        | Token::GtEq
        | Token::AmpAmp
        | Token::PipePipe
        | Token::Bang
        | Token::Amp
        | Token::Pipe
        | Token::Caret
        | Token::Tilde
        | Token::LtLt
        | Token::GtGt
        | Token::Eof => None,
    }
}

/// Classify an identifier using Golden Rule 13 and the signature tables.
///
/// Decision order (first match wins):
///   1. Starts with ASCII uppercase → TYPE (Golden Rule 13: capital letter = type)
///   2. Found in `sig_table.fns` → FUNCTION
///   3. Found as an options variant (checked from imported_options) → ENUM_MEMBER
///   4. Fallback → VARIABLE
fn classify_identifier(name: &str, sig: &ynz_typeck::queries::SignatureOutput) -> u32 {
    // Golden Rule 13: capital letter = type.
    if name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return IDX_TYPE;
    }

    // Known function name from the current file's signatures.
    if sig.sig_table.fns.contains_key(name) {
        return IDX_FUNCTION;
    }

    // Options variant name — check if `name` appears in any known options type's variants.
    for entry in sig.imported_options.values() {
        if entry.variants.iter().any(|v| v == name) {
            return IDX_ENUM_MEMBER;
        }
    }

    IDX_VARIABLE
}

/// Encode absolute-position records to the LSP delta-encoded wire format.
///
/// The LSP wire format requires tokens in document order (ascending line, then
/// start character). Each token is encoded as 5 u32 values:
///   [deltaLine, deltaStart, length, tokenType, tokenModifiers]
/// where deltaLine = line - prev_line and deltaStart = start - prev_start (when
/// on the same line), or start (when on a different line).
///
/// Time: O(T) where T = token count.  Space: O(T).
fn encode_delta(mut raw: Vec<RawToken>) -> Vec<SemanticToken> {
    // Tokens must be in document order.
    raw.sort_unstable_by(|a, b| a.line.cmp(&b.line).then(a.start.cmp(&b.start)));

    let mut out = Vec::with_capacity(raw.len());
    let (mut prev_line, mut prev_start) = (0u32, 0u32);

    for tok in &raw {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            tok.start - prev_start
        } else {
            tok.start
        };

        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length,
            token_type: tok.token_type,
            token_modifiers_bitset: 0,
        });

        prev_line = tok.line;
        prev_start = tok.start;
    }

    out
}

/// Compute the character-length of a span `[start, end)` in `text`, expressed
/// in the units the client negotiated (UTF-8 byte count or UTF-16 code units).
///
/// Empty spans (start >= end) return 0.  Out-of-bounds spans are clamped to
/// `text.len()` before computing the length.
fn char_len(text: &str, start: usize, end: usize, encoding: PositionEncoding) -> u32 {
    if start >= end || start >= text.len() {
        return 0;
    }
    let end = end.min(text.len());
    let slice = &text[start..end];
    match encoding {
        PositionEncoding::Utf8 => slice.len() as u32,
        PositionEncoding::Utf16 => slice.encode_utf16().count() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{capabilities::PositionEncoding, position::LineTable};

    // ─── Legend / options sanity ─────────────────────────────────────────────

    #[test]
    fn legend_idxs_map_to_correct_token_types() {
        // WHY: reordering SEMANTIC_TOKEN_LEGEND silently produces wrong colors in VSCode
        // without any compile error. This test verifies each IDX constant points to
        // the expected SemanticTokenType — not just that it's in-bounds.
        use lsp_types::SemanticTokenType;
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_KEYWORD as usize], SemanticTokenType::KEYWORD);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_TYPE as usize], SemanticTokenType::TYPE);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_FUNCTION as usize], SemanticTokenType::FUNCTION);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_VARIABLE as usize], SemanticTokenType::VARIABLE);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_ENUM_MEMBER as usize], SemanticTokenType::ENUM_MEMBER);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_NUMBER as usize], SemanticTokenType::NUMBER);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_STRING as usize], SemanticTokenType::STRING);
        assert_eq!(SEMANTIC_TOKEN_LEGEND[IDX_COMMENT as usize], SemanticTokenType::COMMENT);
    }

    // ─── Delta encoding ──────────────────────────────────────────────────────

    #[test]
    fn delta_encoding_single_token() {
        // WHY: delta-encoding off-by-one bugs are silent — emitter produces wrong
        // column positions in VSCode and no assertion catches them without this test.
        let raw = vec![RawToken { line: 2, start: 5, length: 3, token_type: IDX_KEYWORD }];
        let encoded = encode_delta(raw);
        assert_eq!(encoded.len(), 1);
        // First token: deltaLine = line (2 - 0), deltaStart = start (5 - 0 because new line)
        assert_eq!(encoded[0].delta_line, 2);
        assert_eq!(encoded[0].delta_start, 5);
        assert_eq!(encoded[0].length, 3);
        assert_eq!(encoded[0].token_type, IDX_KEYWORD);
    }

    #[test]
    fn delta_encoding_two_tokens_same_line() {
        // WHY: same-line tokens encode start relative to previous start.
        // Getting the condition wrong produces wrong column for the second token.
        let raw = vec![
            RawToken { line: 0, start: 4, length: 3, token_type: IDX_KEYWORD },
            RawToken { line: 0, start: 10, length: 5, token_type: IDX_VARIABLE },
        ];
        let encoded = encode_delta(raw);
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].delta_line, 0);
        assert_eq!(encoded[0].delta_start, 4);
        assert_eq!(encoded[1].delta_line, 0);
        assert_eq!(encoded[1].delta_start, 6); // 10 - 4
        assert_eq!(encoded[1].token_type, IDX_VARIABLE);
    }

    #[test]
    fn delta_encoding_two_tokens_different_lines() {
        // WHY: when delta_line > 0, delta_start resets to absolute character position.
        let raw = vec![
            RawToken { line: 1, start: 4, length: 3, token_type: IDX_KEYWORD },
            RawToken { line: 3, start: 7, length: 2, token_type: IDX_NUMBER },
        ];
        let encoded = encode_delta(raw);
        assert_eq!(encoded.len(), 2);
        assert_eq!(encoded[0].delta_line, 1);
        assert_eq!(encoded[0].delta_start, 4);
        assert_eq!(encoded[1].delta_line, 2); // 3 - 1
        assert_eq!(encoded[1].delta_start, 7); // absolute (different line)
    }

    // ─── char_len ────────────────────────────────────────────────────────────

    #[test]
    fn char_len_utf8_ascii() {
        assert_eq!(char_len("hello world", 0, 5, PositionEncoding::Utf8), 5);
    }

    #[test]
    fn char_len_utf8_emoji() {
        // 🦀 is 4 UTF-8 bytes, 2 UTF-16 code units.
        let s = "🦀 crab";
        assert_eq!(char_len(s, 0, 4, PositionEncoding::Utf8), 4);
        assert_eq!(char_len(s, 0, 4, PositionEncoding::Utf16), 2);
    }

    #[test]
    fn char_len_zero_width() {
        assert_eq!(char_len("hello", 3, 3, PositionEncoding::Utf8), 0);
        assert_eq!(char_len("hello", 10, 15, PositionEncoding::Utf8), 0); // out of bounds
    }
}
