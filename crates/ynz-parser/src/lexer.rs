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
            self.skip_whitespace();
            if self.pos >= self.src.len() {
                self.push_token(Token::Eof, self.pos, self.pos);
                break;
            }
            self.lex_one();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.src.len() && self.src[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
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

            b'-' => {
                if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'>' {
                    self.pos += 2;
                    self.push_token(Token::Arrow, start, self.pos);
                } else {
                    self.pos += 1;
                    self.emit_unknown_byte(start, byte);
                }
            }

            b'"' => self.lex_string(start),

            b if b.is_ascii_alphabetic() || b == b'_' => self.lex_identifier_or_keyword(start),

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
            other => Token::Identifier(other.to_string()),
        };
        self.push_token(tok, start, self.pos);
    }

    fn lex_string(&mut self, start: usize) {
        self.pos += 1; // skip opening `"`
        let content_start = self.pos;

        loop {
            if self.pos >= self.src.len() {
                // Unterminated string — emit diagnostic and recover.
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, start + 1),
                    "A string literal is missing its closing quote.",
                    "Add `\"` at the end of the string.",
                    "String literals must start and end with double-quote characters.",
                ));
                // Emit what we have so far so the parser sees a StringLit token.
                let bytes = self.src[content_start..self.pos].to_vec();
                self.push_token(Token::StringLit(bytes), start, self.pos);
                return;
            }

            match self.src[self.pos] {
                b'"' => {
                    // Closing quote found.
                    let bytes = self.src[content_start..self.pos].to_vec();
                    self.pos += 1; // skip closing `"`
                    self.push_token(Token::StringLit(bytes), start, self.pos);
                    return;
                }
                b'\n' => {
                    // Newline before closing quote — recover at newline boundary.
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

    fn emit_unknown_byte(&mut self, pos: usize, byte: u8) {
        self.diags.push(Diagnostic::error(
            SourceSpan::new(self.file, pos, pos + 1),
            format!("The character `{}` is not valid here.", byte as char),
            "Remove or replace this character.",
            "Yinz source files may only contain ASCII text and UTF-8 string content.",
        ));
    }

    fn push_token(&mut self, tok: Token, start: usize, end: usize) {
        self.tokens.push(Spanned::new(tok, self.file, start, end));
    }
}
