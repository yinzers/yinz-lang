use crate::capabilities::PositionEncoding;
use lsp_types::Position;

/// Pre-computed line-start byte offsets for a source text.
/// Built once per `didOpen` / `didChange` so N span conversions are O(log lines) each.
///
/// BOM handling: a UTF-8 BOM (`\u{FEFF}`, bytes 0xEF 0xBB 0xBF) at offset 0 is
/// treated as 1 character per the LSP spec regardless of encoding, matching the
/// plan's locked spec: byte offset 3 (first byte after the BOM) maps to character 1.
#[derive(Clone, Debug)]
pub struct LineTable {
    /// `starts[i]` is the byte offset of the first character on line `i`.
    starts: Vec<usize>,
    total_bytes: usize,
    /// True when text starts with a UTF-8 BOM (3 bytes that count as 1 character).
    has_bom: bool,
}

impl LineTable {
    pub fn new(text: &str) -> Self {
        let has_bom = text.starts_with('\u{FEFF}');
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self {
            starts,
            total_bytes: text.len(),
            has_bom,
        }
    }

    /// Convert a byte offset to an LSP Position.
    ///
    /// BOM handling: a UTF-8 BOM (`\u{FEFF}`, bytes 0xEF 0xBB 0xBF) at offset 0 counts
    /// as 1 character per LSP spec — byte offset 3 (first byte after BOM) → character 1.
    pub fn byte_offset_to_position(
        &self,
        text: &str,
        byte_offset: usize,
        encoding: PositionEncoding,
    ) -> Position {
        let line = self
            .starts
            .partition_point(|&s| s <= byte_offset)
            .saturating_sub(1);
        let line_start = self.starts[line];
        let line_slice = &text[line_start..byte_offset.min(text.len())];

        let mut character = match encoding {
            PositionEncoding::Utf8 => line_slice.len() as u32,
            PositionEncoding::Utf16 => line_slice.encode_utf16().count() as u32,
        };

        // BOM on line 0: the 3-byte BOM sequence was counted as 3 chars above;
        // per LSP spec it is 1 character. Subtract 2 (3 bytes - 1 char = 2 bytes extra).
        if self.has_bom && line == 0 && byte_offset > 0 {
            character = character.saturating_sub(2);
        }

        Position {
            line: line as u32,
            character,
        }
    }

    /// Convert an LSP Position to a byte offset. Returns `None` if out of bounds.
    pub fn position_to_byte_offset(
        &self,
        text: &str,
        position: Position,
        encoding: PositionEncoding,
    ) -> Option<usize> {
        let line = position.line as usize;
        if line >= self.starts.len() {
            return None;
        }
        let line_start = self.starts[line];
        let line_end = self
            .starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.total_bytes);
        let line_text = &text[line_start..line_end];

        // BOM on line 0: LSP character 1 corresponds to byte 3 (after the 3-byte BOM).
        // Adjust the target character count up by 2 to account for the 2 extra bytes.
        let target_char = if self.has_bom && line == 0 && position.character > 0 {
            position.character + 2
        } else {
            position.character
        };

        let byte_in_line = match encoding {
            PositionEncoding::Utf8 => {
                let ch = target_char as usize;
                if ch > line_text.len() {
                    return None;
                }
                // A `positionEncoding: utf-8` client is trusted to send a byte count,
                // but a buggy or adversarial client can send a mid-character byte
                // offset. The utf-16 branch below can never land off a char boundary
                // (it walks `chars()`), so this is the one place a non-boundary offset
                // can escape — and this offset flows into consumers that slice `&str`
                // (formatting.rs, completion.rs, semantic_tokens.rs), where a mid-char
                // slice panics — the per-request isolation wrapper would catch it as an
                // InternalError, but reject it here so the failure mode stays explicit
                // and no request is burned on an avoidable panic.
                if !line_text.is_char_boundary(ch) {
                    return None;
                }
                ch
            }
            PositionEncoding::Utf16 => {
                let target_units = target_char as usize;
                let mut units = 0usize;
                let mut bytes = 0usize;
                for ch in line_text.chars() {
                    if units >= target_units {
                        break;
                    }
                    units += ch.len_utf16();
                    bytes += ch.len_utf8();
                }
                if units < target_units {
                    return None;
                }
                bytes
            }
        };

        Some(line_start + byte_in_line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Position;

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    macro_rules! byte_offset_to_position_case {
        ($name:ident, $text:expr, $offset:expr, $encoding:expr, $expected:expr) => {
            // bug: a byte offset maps to the wrong line or column (LF/CRLF, BOM, multi-byte characters, UTF-8 vs UTF-16), so edits and ranges land on the wrong text | tier: Medium
            #[test]
            fn $name() {
                let text = $text;
                let table = LineTable::new(text);
                assert_eq!(
                    table.byte_offset_to_position(text, $offset, $encoding),
                    $expected
                );
            }
        };
    }

    byte_offset_to_position_case!(
        lf_starts_a_new_line,
        "abc\nde",
        4,
        PositionEncoding::Utf8,
        pos(1, 0)
    );
    byte_offset_to_position_case!(
        crlf_starts_a_new_line,
        "ab\r\ncd",
        4,
        PositionEncoding::Utf8,
        pos(1, 0)
    );
    // offset 2 is the `\n` of `\r\n`: it stays on line 0
    byte_offset_to_position_case!(
        crlf_newline_byte_stays_on_its_line,
        "a\r\nb",
        2,
        PositionEncoding::Utf8,
        pos(0, 2)
    );
    // the BOM is 3 bytes but one character: `a` at byte 3 is character 1
    byte_offset_to_position_case!(
        bom_counts_as_one_character,
        "\u{FEFF}ab",
        3,
        PositionEncoding::Utf8,
        pos(0, 1)
    );
    // U+2713 is 3 UTF-8 bytes and 1 UTF-16 code unit
    byte_offset_to_position_case!(
        three_byte_char_counts_bytes_in_utf8,
        "✓✓",
        3,
        PositionEncoding::Utf8,
        pos(0, 3)
    );
    byte_offset_to_position_case!(
        three_byte_char_counts_one_unit_in_utf16,
        "✓✓",
        3,
        PositionEncoding::Utf16,
        pos(0, 1)
    );
    // U+1F600 is 4 UTF-8 bytes and a UTF-16 surrogate pair (2 units)
    byte_offset_to_position_case!(
        emoji_counts_bytes_in_utf8,
        "a\u{1F600}b",
        5,
        PositionEncoding::Utf8,
        pos(0, 5)
    );
    byte_offset_to_position_case!(
        emoji_counts_a_surrogate_pair_in_utf16,
        "a\u{1F600}b",
        5,
        PositionEncoding::Utf16,
        pos(0, 3)
    );
    byte_offset_to_position_case!(
        empty_text_has_position_zero,
        "",
        0,
        PositionEncoding::Utf8,
        pos(0, 0)
    );

    macro_rules! position_to_byte_offset_case {
        ($name:ident, $text:expr, $position:expr, $expected:expr) => {
            // bug: a client position inside a multi-byte character, past the end or after a BOM resolves to a wrong byte (a mid-character one would panic a later string slice) | tier: Medium
            #[test]
            fn $name() {
                let text = $text;
                let table = LineTable::new(text);
                assert_eq!(
                    table.position_to_byte_offset(text, $position, PositionEncoding::Utf8),
                    $expected
                );
            }
        };
    }

    // `✓` is bytes 0..3: character 1 is inside it and must be rejected, not returned as byte 1
    position_to_byte_offset_case!(
        position_inside_a_multibyte_char_is_rejected,
        "✓b",
        pos(0, 1),
        None
    );
    position_to_byte_offset_case!(
        position_after_a_multibyte_char_resolves,
        "✓b",
        pos(0, 3),
        Some(3)
    );
    position_to_byte_offset_case!(
        position_after_a_bom_resolves,
        "\u{FEFF}ab",
        pos(0, 1),
        Some(3)
    );
    position_to_byte_offset_case!(
        position_past_the_end_of_empty_text_is_rejected,
        "",
        pos(0, 1),
        None
    );
    position_to_byte_offset_case!(
        position_on_a_crlf_second_line_resolves,
        "ab\r\ncd",
        pos(1, 0),
        Some(4)
    );

    // position_to_byte_offset(byte_offset_to_position(N)) == Some(N) at every char boundary
    macro_rules! round_trip_case {
        ($name:ident, $text:expr) => {
            // bug: converting a byte offset to a position and back does not return the same byte for some text shape or encoding | tier: Medium
            #[test]
            fn $name() {
                let text = $text;
                let table = LineTable::new(text);
                let failures: Vec<String> = (0..=text.len())
                    .filter(|offset| text.is_char_boundary(*offset))
                    .flat_map(|offset| {
                        [PositionEncoding::Utf8, PositionEncoding::Utf16]
                            .map(|encoding| (offset, encoding))
                    })
                    .filter_map(|(offset, encoding)| {
                        let position = table.byte_offset_to_position(text, offset, encoding);
                        let back = table.position_to_byte_offset(text, position, encoding);
                        (back != Some(offset))
                            .then(|| format!("offset {offset} {encoding:?}: back {back:?}"))
                    })
                    .collect();
                assert!(failures.is_empty(), "round-trip failed: {failures:?}");
            }
        };
    }

    round_trip_case!(lf_text_round_trips, "abc\nde");
    round_trip_case!(crlf_text_round_trips, "ab\r\ncd");
    round_trip_case!(three_byte_chars_round_trip, "✓✓");
    round_trip_case!(emoji_text_round_trips, "a\u{1F600}b");
    round_trip_case!(empty_text_round_trips, "");
}
