//! Rust input: rewrites string literals (plain with escapes, raw, byte-skipped) whose decoded
//! content is a complete Yinz source file. Offsets inside the decoded text are mapped back to
//! source offsets, so escapes elsewhere in the literal are preserved byte for byte.

use crate::yinz_source::{
    apply_renames, line_of_offset, rewrite_yinz_source, ParseDiagnostics, Rename, RENAMES,
};

#[derive(Debug, Default)]
pub struct RustRewrite {
    pub new_text: String,
    pub renames: usize,
    pub strings_rewritten: usize,
    /// Parsed as Yinz, nothing to rename.
    pub strings_unchanged: usize,
    /// Contained an old built-in spelling but did not parse as a complete Yinz file.
    pub strings_skipped: usize,
    /// Skipped strings that look like Yinz source (contain `function`/`shape`/`let`): (line, reason).
    pub suspect_skipped: Vec<(usize, String)>,
    pub ambiguous: Vec<(usize, String)>,
}

struct StringLiteral {
    /// Source range of the whole literal including quotes and prefix.
    literal_start: usize,
    /// Source range of the content between the quotes.
    content_start: usize,
    content_end: usize,
    is_raw: bool,
    is_byte_string: bool,
}

/// O(n) time and memory in the file length plus the parse cost of each candidate string.
pub fn rewrite_rust_file(file_label: &str, source: &str) -> RustRewrite {
    let mut result = RustRewrite::default();
    let literals = find_string_literals(source);
    let mut source_renames: Vec<Rename> = Vec::new();

    for literal in &literals {
        if literal.is_byte_string {
            continue;
        }
        let content_source = &source[literal.content_start..literal.content_end];
        if !RENAMES.iter().any(|(old, _)| content_source.contains(old)) {
            continue;
        }
        let decoded = if literal.is_raw {
            DecodedString::identity(content_source)
        } else {
            match DecodedString::from_escaped(content_source) {
                Some(decoded) => decoded,
                None => {
                    result.strings_skipped += 1;
                    continue;
                }
            }
        };
        let line = line_of_offset(source, literal.literal_start);
        match rewrite_yinz_source(file_label, &decoded.text, ParseDiagnostics::Reject) {
            Err(unparseable) => {
                result.strings_skipped += 1;
                let looks_like_yinz = ["function ", "shape ", "let ", "const "]
                    .iter()
                    .any(|marker| decoded.text.contains(marker));
                if looks_like_yinz {
                    result.suspect_skipped.push((line, unparseable.reason));
                }
            }
            Ok(rewrite) => {
                for (line_in_string, name) in &rewrite.ambiguous_constant_receivers {
                    result.ambiguous.push((line + line_in_string - 1, name.clone()));
                }
                if rewrite.renames.is_empty() {
                    result.strings_unchanged += 1;
                    continue;
                }
                match decoded.map_renames_to_source(
                    &rewrite.renames,
                    source,
                    literal.content_start,
                ) {
                    Some(mapped) => {
                        result.strings_rewritten += 1;
                        result.renames += mapped.len();
                        source_renames.extend(mapped);
                    }
                    None => {
                        result.strings_skipped += 1;
                        result.suspect_skipped.push((
                            line,
                            "a type name contains an escape sequence; rename by hand".to_string(),
                        ));
                    }
                }
            }
        }
    }

    source_renames.sort_by_key(|rename| rename.start);
    result.new_text = apply_renames(source, &source_renames);
    result
}

/// Decoded string content plus, for every decoded byte, where it came from in the source.
struct DecodedString {
    text: String,
    /// `start_in_source[i]`: offset (relative to the content start) of the segment that produced byte i.
    start_in_source: Vec<usize>,
    /// `end_in_source[i]`: offset (relative to the content start) just past that segment.
    end_in_source: Vec<usize>,
}

impl DecodedString {
    fn identity(content: &str) -> Self {
        DecodedString {
            text: content.to_string(),
            start_in_source: (0..content.len()).collect(),
            end_in_source: (1..=content.len()).collect(),
        }
    }

    /// Decodes Rust escapes. Returns `None` for a malformed escape (the literal is then skipped).
    fn from_escaped(content: &str) -> Option<Self> {
        let bytes = content.as_bytes();
        let mut text = String::with_capacity(content.len());
        let mut start_in_source = Vec::with_capacity(content.len());
        let mut end_in_source = Vec::with_capacity(content.len());
        let mut cursor = 0;

        let mut push_char = |character: char, from: usize, to: usize, text: &mut String| {
            let before = text.len();
            text.push(character);
            for _ in before..text.len() {
                start_in_source.push(from);
                end_in_source.push(to);
            }
        };

        while cursor < bytes.len() {
            if bytes[cursor] != b'\\' {
                let character = content[cursor..].chars().next()?;
                let width = character.len_utf8();
                push_char(character, cursor, cursor + width, &mut text);
                cursor += width;
                continue;
            }
            let escape_start = cursor;
            cursor += 1;
            let escape_letter = *bytes.get(cursor)?;
            cursor += 1;
            let decoded_char = match escape_letter {
                b'n' => '\n',
                b'r' => '\r',
                b't' => '\t',
                b'\\' => '\\',
                b'0' => '\0',
                b'\'' => '\'',
                b'"' => '"',
                b'x' => {
                    let hex = content.get(cursor..cursor + 2)?;
                    cursor += 2;
                    char::from(u8::from_str_radix(hex, 16).ok()?)
                }
                b'u' => {
                    if bytes.get(cursor) != Some(&b'{') {
                        return None;
                    }
                    let close = content[cursor..].find('}')? + cursor;
                    let digits: String = content[cursor + 1..close]
                        .chars()
                        .filter(|c| *c != '_')
                        .collect();
                    cursor = close + 1;
                    char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?
                }
                b'\n' | b'\r' => {
                    // Line continuation: the backslash, the newline and the next line's
                    // leading whitespace are all dropped.
                    while cursor < bytes.len() && matches!(bytes[cursor], b' ' | b'\t' | b'\n' | b'\r') {
                        cursor += 1;
                    }
                    continue;
                }
                _ => return None,
            };
            push_char(decoded_char, escape_start, cursor, &mut text);
        }
        Some(DecodedString {
            text,
            start_in_source,
            end_in_source,
        })
    }

    /// Maps renames from decoded offsets to source offsets. Returns `None` unless every renamed
    /// token is written verbatim in the source (no escape inside the identifier).
    fn map_renames_to_source(
        &self,
        renames: &[Rename],
        source: &str,
        content_start: usize,
    ) -> Option<Vec<Rename>> {
        let mut mapped = Vec::with_capacity(renames.len());
        for rename in renames {
            let source_start = content_start + self.start_in_source[rename.start];
            let source_end = content_start + self.end_in_source[rename.end - 1];
            let decoded_token = &self.text[rename.start..rename.end];
            if source.get(source_start..source_end) != Some(decoded_token) {
                return None;
            }
            mapped.push(Rename {
                start: source_start,
                end: source_end,
                new_name: rename.new_name,
            });
        }
        Some(mapped)
    }
}

/// Minimal Rust lexer: finds string literals while skipping comments, char literals and lifetimes.
fn find_string_literals(source: &str) -> Vec<StringLiteral> {
    let bytes = source.as_bytes();
    let mut literals = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        match byte {
            b'/' if bytes.get(cursor + 1) == Some(&b'/') => {
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                let mut depth = 1;
                cursor += 2;
                while cursor < bytes.len() && depth > 0 {
                    if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
                        depth += 1;
                        cursor += 2;
                    } else if bytes[cursor] == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
                        depth -= 1;
                        cursor += 2;
                    } else {
                        cursor += 1;
                    }
                }
            }
            b'"' => {
                cursor = scan_plain_string(bytes, cursor, cursor, false, &mut literals);
            }
            b'r' | b'b' if !is_identifier_continuation(bytes, cursor) => {
                cursor = scan_prefixed_literal(bytes, cursor, &mut literals);
            }
            b'\'' => {
                cursor = skip_char_literal_or_lifetime(bytes, cursor);
            }
            _ if is_identifier_byte(byte) => {
                while cursor < bytes.len() && is_identifier_byte(bytes[cursor]) {
                    cursor += 1;
                }
            }
            _ => cursor += 1,
        }
    }
    literals
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

/// True when the byte before `cursor` continues an identifier (so `cursor` is not a token start).
fn is_identifier_continuation(bytes: &[u8], cursor: usize) -> bool {
    cursor > 0 && is_identifier_byte(bytes[cursor - 1])
}

/// Handles `r"..."`, `r#"..."#`, `b"..."`, `br"..."`, `b'x'`; falls back to an identifier scan.
fn scan_prefixed_literal(bytes: &[u8], start: usize, literals: &mut Vec<StringLiteral>) -> usize {
    let mut cursor = start;
    let is_byte = bytes[cursor] == b'b';
    if is_byte {
        cursor += 1;
        if bytes.get(cursor) == Some(&b'\'') {
            return skip_char_literal_or_lifetime(bytes, cursor);
        }
        if bytes.get(cursor) == Some(&b'"') {
            return scan_plain_string(bytes, start, cursor, true, literals);
        }
    }
    if bytes.get(cursor) == Some(&b'r') {
        let mut hashes = 0;
        let mut probe = cursor + 1;
        while bytes.get(probe) == Some(&b'#') {
            hashes += 1;
            probe += 1;
        }
        if bytes.get(probe) == Some(&b'"') {
            let content_start = probe + 1;
            let mut closing = vec![b'"'];
            closing.extend(std::iter::repeat_n(b'#', hashes));
            let mut scan = content_start;
            while scan + closing.len() <= bytes.len() {
                if bytes[scan..scan + closing.len()] == closing[..] {
                    literals.push(StringLiteral {
                        literal_start: start,
                        content_start,
                        content_end: scan,
                        is_raw: true,
                        is_byte_string: is_byte,
                    });
                    return scan + closing.len();
                }
                scan += 1;
            }
            return bytes.len();
        }
    }
    // Plain identifier starting with `r` or `b`.
    while cursor < bytes.len() && is_identifier_byte(bytes[cursor]) {
        cursor += 1;
    }
    cursor.max(start + 1)
}

fn scan_plain_string(
    bytes: &[u8],
    literal_start: usize,
    quote_position: usize,
    is_byte_string: bool,
    literals: &mut Vec<StringLiteral>,
) -> usize {
    let content_start = quote_position + 1;
    let mut cursor = content_start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            b'"' => {
                literals.push(StringLiteral {
                    literal_start,
                    content_start,
                    content_end: cursor,
                    is_raw: false,
                    is_byte_string,
                });
                return cursor + 1;
            }
            _ => cursor += 1,
        }
    }
    bytes.len()
}

/// `'a'`, `'\n'`, `'\u{1F600}'` are char literals; `'a` followed by anything but `'` is a lifetime.
fn skip_char_literal_or_lifetime(bytes: &[u8], quote_position: usize) -> usize {
    let after_quote = quote_position + 1;
    if bytes.get(after_quote) == Some(&b'\\') {
        let mut cursor = after_quote + 2;
        while cursor < bytes.len() && bytes[cursor] != b'\'' {
            cursor += 1;
        }
        return cursor + 1;
    }
    // One (possibly multi-byte) character then `'`.
    let mut cursor = after_quote + 1;
    while cursor < bytes.len() && (bytes[cursor] & 0xC0) == 0x80 {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'\'') {
        cursor + 1
    } else {
        after_quote
    }
}
