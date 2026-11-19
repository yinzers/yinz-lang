//! The Replacement-category surface for the two auto-promotion hints
//! (`array_to_fixed_promotion`, `let_to_const_promotion`).
//!
//! The compiler already chose the stricter form (`fixed`, `const`); the editor
//! shows that choice on the token the user wrote, not as extra text:
//!
//! - a `promotable` semantic-token modifier marks the `array` / `let` token
//!   (`semantic_tokens`),
//! - hovering the marked token teaches the decision (`hover::promotion_hover`),
//! - a `refactor.rewrite` code action writes the stricter form into the source
//!   (`code_action`).
//!
//! The marks come from the typeck promotion passes — the same analysis the
//! compiler promotes from — and are never re-derived here.

use lsp_types::{Position, Range, TextEdit};
use ynz_parser::{SourceFile, SourceFileRegistry};
use ynz_typeck::{array_to_fixed_promotion_hints, let_to_const_promotion_hints, PromotionKind};

use crate::{capabilities::PositionEncoding, position::LineTable};

/// One token the compiler promoted: the written `array` or `let` plus the
/// stricter keyword that replaces it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromotionMark {
    /// Registry `[[muted_hint_domain]]` name; selects the hover text.
    pub domain: &'static str,
    /// Byte offset of the first byte of the marked token.
    pub start: usize,
    /// Byte offset one past the last byte of the marked token.
    pub end: usize,
    /// Keyword the token is rewritten to (`fixed` or `const`).
    pub replacement: &'static str,
    /// Code-action title.
    pub action_title: &'static str,
}

const ARRAY_TO_FIXED_DOMAIN: &str = "array_to_fixed_promotion";
const LET_TO_CONST_DOMAIN: &str = "let_to_const_promotion";

/// Every promoted `array` / `let` token in `sf`, in no particular order.
///
/// Time: O(P) over the typeck promotion hints (salsa-cached passes).
/// Space: O(P).
pub fn promotion_marks(db: &dyn SourceFileRegistry, sf: SourceFile) -> Vec<PromotionMark> {
    let array_hints = array_to_fixed_promotion_hints(db, sf);
    let let_hints = let_to_const_promotion_hints(db, sf);
    array_hints
        .iter()
        .chain(let_hints.iter())
        .filter_map(|hint| {
            let span = hint.keyword_span.as_ref()?;
            let (domain, replacement, action_title) = match hint.kind {
                PromotionKind::ArrayToFixed => (
                    ARRAY_TO_FIXED_DOMAIN,
                    "fixed",
                    "Write `fixed` instead of `array` (this never grows)",
                ),
                PromotionKind::LetToConst => (
                    LET_TO_CONST_DOMAIN,
                    "const",
                    "Write `const` instead of `let` (this is never reassigned)",
                ),
            };
            Some(PromotionMark {
                domain,
                start: span.start,
                end: span.end,
                replacement,
                action_title,
            })
        })
        .collect()
}

/// The mark whose token the cursor is on. A cursor at the token's end byte only
/// counts when no other token starts there (whitespace or end of file follows),
/// matching `hover::token_at_offset`, which resolves such an offset to the
/// following token: hovering the `<` of `array<int>` must not show the `array`
/// hover.
///
/// Time: O(M).  Space: O(1).
pub fn mark_at_offset<'marks>(
    marks: &'marks [PromotionMark],
    text: &str,
    byte_offset: usize,
) -> Option<&'marks PromotionMark> {
    marks.iter().find(|mark| {
        if mark.start > byte_offset || byte_offset > mark.end {
            return false;
        }
        byte_offset < mark.end
            || text
                .get(mark.end..)
                .and_then(|after_token| after_token.chars().next())
                .is_none_or(char::is_whitespace)
    })
}

/// Build the `TextEdit` that rewrites the marked token (`array` → `fixed`,
/// `let` → `const`). The edit covers exactly the token, never the `<int>`
/// type arguments or the initializer.
///
/// Time: O(1) given the line table.  Space: O(1).
pub fn promotion_edit(
    text: &str,
    mark: &PromotionMark,
    table: &LineTable,
    encoding: PositionEncoding,
) -> TextEdit {
    let position_of = |byte: usize| -> Position {
        table.byte_offset_to_position(text, byte.min(text.len()), encoding)
    };
    TextEdit {
        range: Range {
            start: position_of(mark.start),
            end: position_of(mark.end),
        },
        new_text: mark.replacement.to_string(),
    }
}
