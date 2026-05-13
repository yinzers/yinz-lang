use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::token::{Spanned, Token};

/// Lex a single source file.
///
/// Produces a list of spanned tokens and a bucket of any lexer-level diagnostics.
/// The lexer never panics — errors are recorded and lexing continues so the caller
/// sees all problems at once.
///
/// The source bytes MUST be valid UTF-8 (the driver verifies this before calling).
pub fn lex(file: &str, source: &str) -> (Vec<Spanned<Token>>, DiagnosticBucket) {
    let mut lex = Lexer::new(file, source);
    lex.run();
    (lex.tokens, lex.diags)
}

struct Lexer<'src> {
    file: &'src str,
    src: &'src [u8],
    pos: usize,
    tokens: Vec<Spanned<Token>>,
    diags: DiagnosticBucket,
}

impl<'src> Lexer<'src> {
    fn new(file: &'src str, source: &'src str) -> Self {
        Self {
            file,
            src: source.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
            diags: DiagnosticBucket::new(),
        }
    }

    fn run(&mut self) {
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.src.len() {
                self.push_token(Token::Eof, self.pos, self.pos);
                break;
            }
            self.lex_one();
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            // Line comment: `// ...` — skip to end of line
            if self.pos + 1 < self.src.len()
                && self.src[self.pos] == b'/'
                && self.src[self.pos + 1] == b'/'
            {
                while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    fn peek(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    fn lex_one(&mut self) {
        let start = self.pos;
        let byte = self.src[self.pos];

        match byte {

            b'(' => {
                self.pos += 1;
                self.push_token(Token::LParen, start, self.pos);
            }
            b')' => {
                self.pos += 1;
                self.push_token(Token::RParen, start, self.pos);
            }
            b'{' => {
                self.pos += 1;
                self.push_token(Token::LBrace, start, self.pos);
            }
            b'}' => {
                self.pos += 1;
                self.push_token(Token::RBrace, start, self.pos);
            }
            b'[' => {
                self.pos += 1;
                self.push_token(Token::LBracket, start, self.pos);
            }
            b']' => {
                self.pos += 1;
                self.push_token(Token::RBracket, start, self.pos);
            }
            b',' => {
                self.pos += 1;
                self.push_token(Token::Comma, start, self.pos);
            }
            b'.' => {
                self.pos += 1;
                self.push_token(Token::Dot, start, self.pos);
            }
            b':' => {
                self.pos += 1;
                self.push_token(Token::Colon, start, self.pos);
            }
            b'~' => {
                self.pos += 1;
                self.push_token(Token::Tilde, start, self.pos);
            }
            b'^' => {
                self.pos += 1;
                self.push_token(Token::Caret, start, self.pos);
            }


            b'+' => {
                if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "+=", "x = x + n");
                } else if self.peek(1) == Some(b'+') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "++", "x = x + 1");
                } else {
                    self.pos += 1;
                    self.push_token(Token::Plus, start, self.pos);
                }
            }

            b'-' => {
                if self.peek(1) == Some(b'>') {
                    self.pos += 2;
                    self.push_token(Token::Arrow, start, self.pos);
                } else if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "-=", "x = x - n");
                } else if self.peek(1) == Some(b'-') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "--", "x = x - 1");
                } else {
                    self.pos += 1;
                    self.push_token(Token::Minus, start, self.pos);
                }
            }

            b'*' => {
                if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "*=", "x = x * n");
                } else {
                    self.pos += 1;
                    self.push_token(Token::Star, start, self.pos);
                }
            }

            b'/' => {
                // Note: `//` comments are consumed by skip_whitespace_and_comments
                // before lex_one is called. A bare `/` here is always division.
                if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "/=", "x = x / n");
                } else {
                    self.pos += 1;
                    self.push_token(Token::Slash, start, self.pos);
                }
            }

            b'%' => {
                if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.emit_banned_compound_op(start, self.pos, "%=", "x = x % n");
                } else {
                    self.pos += 1;
                    self.push_token(Token::Percent, start, self.pos);
                }
            }


            b'=' => {
                if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.push_token(Token::EqEq, start, self.pos);
                } else {
                    self.pos += 1;
                    self.push_token(Token::Eq, start, self.pos);
                }
            }

            b'!' => {
                if self.peek(1) == Some(b'=') {
                    self.pos += 2;
                    self.push_token(Token::NotEq, start, self.pos);
                } else {
                    self.pos += 1;
                    self.push_token(Token::Bang, start, self.pos);
                }
            }

            b'<' => match self.peek(1) {
                Some(b'=') => {
                    self.pos += 2;
                    self.push_token(Token::LtEq, start, self.pos);
                }
                Some(b'<') => {
                    self.pos += 2;
                    self.push_token(Token::LtLt, start, self.pos);
                }
                _ => {
                    self.pos += 1;
                    self.push_token(Token::Lt, start, self.pos);
                }
            },

            b'>' => match self.peek(1) {
                Some(b'=') => {
                    self.pos += 2;
                    self.push_token(Token::GtEq, start, self.pos);
                }
                Some(b'>') => {
                    self.pos += 2;
                    self.push_token(Token::GtGt, start, self.pos);
                }
                _ => {
                    self.pos += 1;
                    self.push_token(Token::Gt, start, self.pos);
                }
            },


            b'&' => {
                if self.peek(1) == Some(b'&') {
                    self.pos += 2;
                    self.push_token(Token::AmpAmp, start, self.pos);
                } else {
                    self.pos += 1;
                    self.push_token(Token::Amp, start, self.pos);
                }
            }

            b'|' => {
                if self.peek(1) == Some(b'|') {
                    self.pos += 2;
                    self.push_token(Token::PipePipe, start, self.pos);
                } else {
                    self.pos += 1;
                    self.push_token(Token::Pipe, start, self.pos);
                }
            }


            b'"' => self.lex_string(start),


            b'0'..=b'9' => self.lex_number(start),


            b if b.is_ascii_alphabetic() || b == b'_' => {
                self.lex_identifier_or_keyword(start)
            }

            b => {
                self.pos += 1;
                self.emit_unknown_byte(start, b);
            }
        }
    }


    fn lex_identifier_or_keyword(&mut self, start: usize) {
        while self.pos < self.src.len() {
            let b = self.src[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos])
            .expect("identifier slice is valid UTF-8 — source was validated at load time");
        let tok = match text {
            "function" => Token::Function,
            "nothing" => Token::Nothing,
            "let" => Token::Let,
            "const" => Token::Const,
            "true" => Token::True,
            "false" => Token::False,
            other => Token::Identifier(other.to_string()),
        };
        self.push_token(tok, start, self.pos);
    }


    fn lex_string(&mut self, start: usize) {
        self.pos += 1; // skip opening `"`
        let content_start = self.pos;

        loop {
            if self.pos >= self.src.len() {
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, start + 1),
                    "A string literal is missing its closing quote.",
                    "Add `\"` at the end of the string.",
                    "String literals must start and end with double-quote characters.",
                ));
                let bytes = self.src[content_start..self.pos].to_vec();
                self.push_token(Token::StringLit(bytes), start, self.pos);
                return;
            }

            match self.src[self.pos] {
                b'"' => {
                    let bytes = self.src[content_start..self.pos].to_vec();
                    self.pos += 1; // skip closing `"`
                    self.push_token(Token::StringLit(bytes), start, self.pos);
                    return;
                }
                b'\n' => {
                    self.diags.push(Diagnostic::error(
                        SourceSpan::new(self.file, start, start + 1),
                        "A string literal is missing its closing quote.",
                        "Add `\"` before the end of the line.",
                        "String literals cannot span multiple lines.",
                    ));
                    let bytes = self.src[content_start..self.pos].to_vec();
                    self.push_token(Token::StringLit(bytes), start, self.pos);
                    return;
                }
                _ => {
                    self.pos += 1;
                }
            }
        }
    }


    fn lex_number(&mut self, start: usize) {
        // Dispatch on prefix: `0x` = hex, `0b` = binary, else decimal
        if self.src[self.pos] == b'0' && self.pos + 1 < self.src.len() {
            match self.src[self.pos + 1] {
                b'x' | b'X' => return self.lex_hex_int(start),
                b'b' | b'B' => return self.lex_binary_int(start),
                _ => {}
            }
        }
        self.lex_decimal_number(start)
    }

    fn lex_hex_int(&mut self, start: usize) {
        self.pos += 2; // skip "0x" or "0X"
        let digits_start = self.pos;
        let mut has_invalid = false;

        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' | b'_' => self.pos += 1,
                b => {
                    if b.is_ascii_alphanumeric() {
                        let bad = self.pos;
                        self.pos += 1;
                        self.diags.push(Diagnostic::error(
                            SourceSpan::new(self.file, bad, bad + 1),
                            format!("`{}` is not a valid hex digit.", b as char),
                            "Hex literals use digits 0–9 and letters A–F (uppercase or lowercase).",
                            "Hex integer literals look like `0xFF` or `0xDEAD_BEEF`.",
                        ));
                        has_invalid = true;
                    } else {
                        break;
                    }
                }
            }
        }

        if self.pos == digits_start {
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, start, self.pos),
                "A hex literal must have at least one digit after `0x`.",
                "Add hex digits after the `0x` prefix, e.g. `0xFF`.",
                "Hex integer literals look like `0x2A` or `0xFF`.",
            ));
            return;
        }

        if has_invalid {
            return; // diagnostic already emitted; skip the token
        }

        let raw = std::str::from_utf8(&self.src[digits_start..self.pos])
            .expect("hex digit bytes are ASCII");

        if let Some(err_offset) = validate_underscores(raw) {
            let err_pos = digits_start + err_offset;
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, err_pos, err_pos + 1),
                "Numeric literals cannot have adjacent `_` characters or a trailing `_`.",
                "Use a single `_` between groups of digits: `0xDEAD_BEEF`.",
                "The `_` separator is a visual aid — it must sit between digit groups, not next to another `_`.",
            ));
            return;
        }

        let stripped: String = raw.chars().filter(|&c| c != '_').collect();
        // Parse as u64 first — bit-reinterpret to i64 so the full 64-bit range is valid
        match u64::from_str_radix(&stripped, 16) {
            Ok(v) => self.push_token(Token::IntLit(v as i64), start, self.pos),
            Err(_) => {
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, self.pos),
                    "This hex literal is too large to fit in an `int`.",
                    "Use `number` for values beyond ±9.2×10¹⁸.",
                    "`int` holds 64-bit signed integers — hex values above 0xFFFF_FFFF_FFFF_FFFF overflow.",
                ));
            }
        }
    }

    fn lex_binary_int(&mut self, start: usize) {
        self.pos += 2; // skip "0b" or "0B"
        let digits_start = self.pos;
        let mut has_invalid = false;

        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b'0' | b'1' | b'_' => self.pos += 1,
                b => {
                    if b.is_ascii_alphanumeric() {
                        let bad = self.pos;
                        self.pos += 1;
                        self.diags.push(Diagnostic::error(
                            SourceSpan::new(self.file, bad, bad + 1),
                            format!("`{}` is not a valid binary digit.", b as char),
                            "Binary literals use only `0` and `1`.",
                            "Binary integer literals look like `0b1010` or `0b1111_0000`.",
                        ));
                        has_invalid = true;
                    } else {
                        break;
                    }
                }
            }
        }

        if self.pos == digits_start {
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, start, self.pos),
                "A binary literal must have at least one digit after `0b`.",
                "Add `0` or `1` digits after the `0b` prefix, e.g. `0b1010`.",
                "Binary integer literals look like `0b1010` or `0b1111_0000`.",
            ));
            return;
        }

        if has_invalid {
            return;
        }

        let raw = std::str::from_utf8(&self.src[digits_start..self.pos])
            .expect("binary digit bytes are ASCII");

        if let Some(err_offset) = validate_underscores(raw) {
            let err_pos = digits_start + err_offset;
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, err_pos, err_pos + 1),
                "Numeric literals cannot have adjacent `_` characters or a trailing `_`.",
                "Use a single `_` between digit groups: `0b1111_0000`.",
                "The `_` separator is a visual aid — it must sit between digit groups, not next to another `_`.",
            ));
            return;
        }

        let stripped: String = raw.chars().filter(|&c| c != '_').collect();
        match u64::from_str_radix(&stripped, 2) {
            Ok(v) => self.push_token(Token::IntLit(v as i64), start, self.pos),
            Err(_) => {
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, self.pos),
                    "This binary literal is too large to fit in an `int`.",
                    "Use `number` for values beyond ±9.2×10¹⁸.",
                    "`int` holds 64-bit signed integers — this binary literal has too many bits.",
                ));
            }
        }
    }

    /// Lex a decimal number literal.
    ///
    /// A literal containing `.` or `e`/`E` becomes `NumberLit(String)` (underscores
    /// stripped). One with neither becomes `IntLit(i64)`.
    ///
    /// The `.` is only consumed as part of a number when immediately followed by a digit,
    /// so `42.toString()` lexes as `IntLit(42)` + `Dot` + … rather than `NumberLit("42.")`.
    fn lex_decimal_number(&mut self, start: usize) {
        // Phase 1 — integer part: digits and underscores
        let int_start = self.pos;
        while self.pos < self.src.len()
            && matches!(self.src[self.pos], b'0'..=b'9' | b'_')
        {
            self.pos += 1;
        }
        let int_raw =
            std::str::from_utf8(&self.src[int_start..self.pos]).expect("decimal digits are ASCII");

        if let Some(err_offset) = validate_underscores(int_raw) {
            let err_pos = int_start + err_offset;
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, err_pos, err_pos + 1),
                "Numeric literals cannot have adjacent `_` characters or a trailing `_`.",
                "Use a single `_` between groups of digits: `1_000_000`.",
                "The `_` separator is a visual aid — it must sit between digit groups, not next to another `_`.",
            ));
            self.skip_to_whitespace();
            return;
        }

        // Phase 2 — optional fractional part (only if `.` is followed by a digit)
        let has_dot = self.pos < self.src.len()
            && self.src[self.pos] == b'.'
            && self.pos + 1 < self.src.len()
            && self.src[self.pos + 1].is_ascii_digit();

        if has_dot {
            self.pos += 1; // consume `.`
            let frac_start = self.pos;
            while self.pos < self.src.len()
                && matches!(self.src[self.pos], b'0'..=b'9' | b'_')
            {
                self.pos += 1;
            }
            let frac_raw = std::str::from_utf8(&self.src[frac_start..self.pos])
                .expect("fractional digits are ASCII");
            if let Some(err_offset) = validate_underscores(frac_raw) {
                let err_pos = frac_start + err_offset;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, err_pos, err_pos + 1),
                    "Numeric literals cannot have adjacent `_` characters or a trailing `_`.",
                    "Use a single `_` between groups of digits: `3.141_592`.",
                    "The `_` separator is a visual aid — it must sit between digit groups, not next to another `_`.",
                ));
                self.skip_to_whitespace();
                return;
            }

            // A second `.` followed by a digit is an error (`1.2.3`)
            if self.pos < self.src.len()
                && self.src[self.pos] == b'.'
                && self.pos + 1 < self.src.len()
                && self.src[self.pos + 1].is_ascii_digit()
            {
                let dot_pos = self.pos;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, dot_pos, dot_pos + 1),
                    "A decimal literal can only have one decimal point.",
                    "Remove the extra `.` — numbers look like `3.14`, not `3.1.4`.",
                    "Each number literal represents a single value with at most one decimal point.",
                ));
                self.skip_to_whitespace();
                return;
            }
        }

        // Phase 3 — optional exponent part
        let has_exp =
            self.pos < self.src.len() && matches!(self.src[self.pos], b'e' | b'E');

        if has_exp {
            let e_pos = self.pos;
            self.pos += 1; // consume `e` or `E`
            if self.pos < self.src.len() && matches!(self.src[self.pos], b'+' | b'-') {
                self.pos += 1; // consume optional sign
            }
            let exp_digits_start = self.pos;
            while self.pos < self.src.len()
                && matches!(self.src[self.pos], b'0'..=b'9' | b'_')
            {
                self.pos += 1;
            }
            if self.pos == exp_digits_start {
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, e_pos, self.pos),
                    "The exponent in this number literal has no digits.",
                    "Add digits after `e`, e.g. `1e5` or `2.5e-3`.",
                    "`e` notation means ×10^N — the N must be a number.",
                ));
                self.skip_to_whitespace();
                return;
            }
            let exp_raw = std::str::from_utf8(&self.src[exp_digits_start..self.pos])
                .expect("exponent digits are ASCII");
            if let Some(err_offset) = validate_underscores(exp_raw) {
                let err_pos = exp_digits_start + err_offset;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, err_pos, err_pos + 1),
                    "Numeric literals cannot have adjacent `_` characters or a trailing `_`.",
                    "Use a single `_` between exponent digits.",
                    "The `_` separator is a visual aid — it must sit between digit groups.",
                ));
                self.skip_to_whitespace();
                return;
            }
        }

        // Produce the token
        let raw = std::str::from_utf8(&self.src[start..self.pos]).expect("decimal chars are ASCII");
        if has_dot || has_exp {
            let normalized: String = raw.chars().filter(|&c| c != '_').collect();
            self.push_token(Token::NumberLit(normalized), start, self.pos);
        } else {
            let stripped: String = raw.chars().filter(|&c| c != '_').collect();
            match stripped.parse::<i64>() {
                Ok(n) => self.push_token(Token::IntLit(n), start, self.pos),
                Err(_) => {
                    self.diags.push(Diagnostic::error(
                        SourceSpan::new(self.file, start, self.pos),
                        "This integer literal is too large to fit in an `int`.",
                        "Use `number` for values beyond ±9.2×10¹⁸: `let x: number = 99_999_999_999_999_999_999`.",
                        "`int` holds 64-bit signed integers, in the range −9,223,372,036,854,775,808 to 9,223,372,036,854,775,807.",
                    ));
                }
            }
        }
    }


    fn emit_banned_compound_op(
        &mut self,
        start: usize,
        end: usize,
        op: &str,
        suggestion: &str,
    ) {
        self.diags.push(Diagnostic::error(
            SourceSpan::new(self.file, start, end),
            format!("`{}` is not supported in Yinz.", op),
            format!("Use `{}` instead.", suggestion),
            "Compound assignment and increment operators are not part of the Yinz language. \
             Step-by-step assignment on its own line makes the intent explicit.",
        ));
    }

    fn emit_unknown_byte(&mut self, pos: usize, byte: u8) {
        self.diags.push(Diagnostic::error(
            SourceSpan::new(self.file, pos, pos + 1),
            format!("The character `{}` is not valid here.", byte as char),
            "Remove or replace this character.",
            "Yinz source files may only contain ASCII text and UTF-8 string content.",
        ));
    }

    /// Advance past non-whitespace bytes to recover from a malformed literal.
    fn skip_to_whitespace(&mut self) {
        while self.pos < self.src.len() && !self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn push_token(&mut self, tok: Token, start: usize, end: usize) {
        self.tokens.push(Spanned::new(tok, self.file, start, end));
    }
}

/// Check a slice of digit characters (and `_`) for underscore placement errors.
///
/// Returns the byte offset within `s` of the first error, or `None` if valid.
/// Errors: adjacent underscores (`1__0`) or a trailing underscore (`1_`).
///
/// `s` must contain only ASCII digits and underscores — callers are responsible
/// for passing only the digit-group portion of a literal (no `.`, `e`, `+`, `-`).
fn validate_underscores(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'_' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'_' {
                return Some(i + 1); // position of the second `_`
            }
            if i + 1 == bytes.len() {
                return Some(i); // trailing `_`
            }
        }
        i += 1;
    }
    None
}
