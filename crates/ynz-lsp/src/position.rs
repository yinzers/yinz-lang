use crate::capabilities::PositionEncoding;
use lsp_types::Position;

/// Pre-computed line-start byte offsets for a source text.
/// Built once per `didOpen` / `didChange` so N span conversions are O(log lines) each.
#[derive(Clone, Debug)]
pub struct LineTable {
    /// `starts[i]` is the byte offset of the first character on line `i`.
    starts: Vec<usize>,
    total_bytes: usize,
}

impl LineTable {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Self { starts, total_bytes: text.len() }
    }

    /// Convert a byte offset to an LSP Position.
    ///
    /// BOM handling: a UTF-8 BOM (0xEF 0xBB 0xBF, bytes 0-2) at offset 0 counts as
    /// 3 UTF-8 bytes but 1 character per LSP spec.  The line table treats BOM bytes
    /// as ordinary bytes — callers that need to handle BOM should strip it before
    /// constructing the LineTable.
    pub fn byte_offset_to_position(
        &self,
        text: &str,
        byte_offset: usize,
        encoding: PositionEncoding,
    ) -> Position {
        let line = self.starts.partition_point(|&s| s <= byte_offset).saturating_sub(1);
        let line_start = self.starts[line];
        let line_slice = &text[line_start..byte_offset.min(text.len())];

        let character = match encoding {
            PositionEncoding::Utf8 => line_slice.len() as u32,
            PositionEncoding::Utf16 => line_slice.encode_utf16().count() as u32,
        };

        Position { line: line as u32, character }
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
        let line_end = self.starts.get(line + 1).copied().unwrap_or(self.total_bytes);
        let line_text = &text[line_start..line_end];

        let byte_in_line = match encoding {
            PositionEncoding::Utf8 => {
                let ch = position.character as usize;
                if ch > line_text.len() {
                    return None;
                }
                ch
            }
            PositionEncoding::Utf16 => {
                let target_units = position.character as usize;
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

    // GIVEN text="abc\nde", utf-8
    // WHEN byte_offset_to_position(4)
    // THEN Position{line:1, character:0}  (newline at byte 3 ends line 0; 'd' starts line 1)
    #[test]
    fn lf_basic() {
        let t = "abc\nde";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 4, PositionEncoding::Utf8), pos(1, 0));
        assert_eq!(tbl.position_to_byte_offset(t, pos(1, 0), PositionEncoding::Utf8), Some(4));
    }

    // GIVEN text="ab\r\ncd", utf-8
    // WHEN byte_offset_to_position(4)
    // THEN Position{line:1, character:0}  (\r\n is bytes 2-3; 'c' is byte 4 = line 1 char 0)
    #[test]
    fn crlf_basic() {
        let t = "ab\r\ncd";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 4, PositionEncoding::Utf8), pos(1, 0));
        assert_eq!(tbl.position_to_byte_offset(t, pos(1, 0), PositionEncoding::Utf8), Some(4));
    }

    // GIVEN text="a\r\nb", utf-8
    // WHEN byte_offset_to_position(2)
    // THEN Position{line:0, character:2}  (offset 2 is \n — stays on line 0 char 2)
    #[test]
    fn crlf_boundary() {
        let t = "a\r\nb";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 2, PositionEncoding::Utf8), pos(0, 2));
    }

    // GIVEN text="\u{FEFF}ab" (BOM-prefixed), utf-8
    // WHEN byte_offset_to_position(3)
    // THEN Position{line:0, character:3}  (BOM is 3 bytes; 'a' is at byte 3 = char 3 in utf-8)
    #[test]
    fn bom_utf8() {
        let t = "\u{FEFF}ab";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 3, PositionEncoding::Utf8), pos(0, 3));
    }

    // ✓ (U+2713, 3 UTF-8 bytes / 1 UTF-16 code unit)
    // utf-8: byte 3 → char 3; utf-16: byte 3 → char 1
    #[test]
    fn three_byte_char_utf8_vs_utf16() {
        let t = "✓✓";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 3, PositionEncoding::Utf8), pos(0, 3));
        assert_eq!(tbl.byte_offset_to_position(t, 3, PositionEncoding::Utf16), pos(0, 1));
    }

    // 😀 (U+1F600, 4 UTF-8 bytes / 2 UTF-16 code units)
    // text = "a😀b"  bytes: 0=a, 1-4=😀, 5=b
    // utf-8: byte 5 → char 5; utf-16: byte 5 → char 3 (1 for 'a' + 2 for surrogate pair)
    #[test]
    fn emoji_utf8_vs_utf16() {
        let t = "a\u{1F600}b";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 5, PositionEncoding::Utf8), pos(0, 5));
        assert_eq!(tbl.byte_offset_to_position(t, 5, PositionEncoding::Utf16), pos(0, 3));
    }

    // Round-trip: position_to_byte_offset(byte_offset_to_position(N)) == Some(N)
    #[test]
    fn round_trip() {
        let texts = ["abc\nde", "ab\r\ncd", "a\r\nb", "✓✓", "a\u{1F600}b", ""];
        for t in &texts {
            let tbl = LineTable::new(t);
            for offset in 0..=t.len() {
                // Only round-trip at valid char boundaries
                if !t.is_char_boundary(offset) {
                    continue;
                }
                for enc in [PositionEncoding::Utf8, PositionEncoding::Utf16] {
                    let pos = tbl.byte_offset_to_position(t, offset, enc);
                    let back = tbl.position_to_byte_offset(t, pos, enc);
                    assert_eq!(back, Some(offset), "round-trip failed: text={t:?} offset={offset} enc={enc:?}");
                }
            }
        }
    }

    // Empty text edge cases
    #[test]
    fn empty_text() {
        let t = "";
        let tbl = LineTable::new(t);
        assert_eq!(tbl.byte_offset_to_position(t, 0, PositionEncoding::Utf8), pos(0, 0));
        assert_eq!(tbl.position_to_byte_offset(t, pos(0, 0), PositionEncoding::Utf8), Some(0));
        assert_eq!(tbl.position_to_byte_offset(t, pos(0, 1), PositionEncoding::Utf8), None);
    }
}
