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
    /// Brace depth per open interpolation. Pushed 0 on `${`, popped on matching `}`.
    /// If non-empty, any `{` increments the top; any `}` either decrements the top
    /// (inner block) or pops + emits InterpolationEnd (when top == 0).
    interp_depth_stack: Vec<u32>,
    /// When true, `run()` must call `lex_backtick_segment()` before the next
    /// normal lex cycle — we just closed an interpolation and are back inside
    /// the surrounding backtick string literal.
    after_interp_end: bool,
}

impl<'src> Lexer<'src> {
    fn new(file: &'src str, source: &'src str) -> Self {
        Self {
            file,
            src: source.as_bytes(),
            pos: 0,
            tokens: Vec::new(),
            diags: DiagnosticBucket::new(),
            interp_depth_stack: Vec::new(),
            after_interp_end: false,
        }
    }

    fn run(&mut self) {
        loop {
            // After closing an interpolation, re-enter backtick string mode.
            // The opening backtick was already consumed; we're continuing
            // inside the same string without a fresh `\`` at the start.
            if self.after_interp_end {
                self.after_interp_end = false;
                self.lex_backtick_content(); // does NOT consume an opening backtick
                continue;
            }
            let before_skip = self.tokens.len();
            self.skip_whitespace_and_comments();
            // If skip_whitespace_and_comments emitted a token (a DocComment), loop
            // back so the NEXT call to skip_whitespace_and_comments processes what
            // follows — do NOT call lex_one() on the next character.
            if self.tokens.len() > before_skip {
                continue;
            }
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
            // `//` — either a regular comment or a `///` doc-comment.
            if self.pos + 1 < self.src.len()
                && self.src[self.pos] == b'/'
                && self.src[self.pos + 1] == b'/'
            {
                // Check for `///` — doc-comment trivia.
                if self.pos + 2 < self.src.len() && self.src[self.pos + 2] == b'/' {
                    self.lex_doc_comment();
                    return; // DocComment token emitted; let run() call us again.
                }
                // Regular `//` comment — skip to end of line.
                while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    /// Lex a `///` doc-comment line and emit `Token::DocComment`.
    ///
    /// Called from `skip_whitespace_and_comments` when `///` is detected.
    /// Consumes the entire `///...` line including its terminating `\n`.
    /// Determines `break_after` via read-only lookahead (does not advance past
    /// the lookahead content).
    fn lex_doc_comment(&mut self) {
        let start = self.pos;

        // Consume the `///` prefix.
        self.pos += 3; // skip `///`

        // Strip one optional leading space.
        if self.pos < self.src.len() && self.src[self.pos] == b' ' {
            self.pos += 1;
        }

        // Collect content up to `\n` or EOF.
        let content_start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
            self.pos += 1;
        }
        let content_bytes = &self.src[content_start..self.pos];
        let content = std::str::from_utf8(content_bytes)
            .unwrap_or("")
            .trim_end()
            .to_string();

        let line_end = self.pos; // position of the `\n` (or EOF)

        // Consume the `\n` if present.
        if self.pos < self.src.len() && self.src[self.pos] == b'\n' {
            self.pos += 1;
        }

        // Read-only lookahead to determine `break_after`.
        let break_after = self.doc_comment_break_after(self.pos);

        self.push_token(
            Token::DocComment {
                content,
                break_after,
            },
            start,
            line_end,
        );
    }

    /// Read-only lookahead from `scan_start` to determine whether a blank line
    /// separates the doc-comment from the next non-trivia content.
    ///
    /// A blank line = a line containing only whitespace (spaces/tabs) ending with `\n`.
    /// Regular `//` comment lines are NOT blank lines (they have content).
    fn doc_comment_break_after(&self, scan_start: usize) -> bool {
        let mut scan = scan_start;
        let mut line_is_blank = true;

        while scan < self.src.len() {
            match self.src[scan] {
                b'\n' => {
                    if line_is_blank {
                        return true; // Found a blank line.
                    }
                    scan += 1;
                    line_is_blank = true; // Start of next line.
                }
                b' ' | b'\t' | b'\r' => {
                    scan += 1; // Whitespace doesn't change blank-line status.
                }
                b'/' if scan + 1 < self.src.len() && self.src[scan + 1] == b'/' => {
                    if scan + 2 < self.src.len() && self.src[scan + 2] == b'/' {
                        return false; // Next doc-comment — no blank line between.
                    }
                    // Regular `//` comment — not a blank line; skip to `\n`.
                    line_is_blank = false;
                    while scan < self.src.len() && self.src[scan] != b'\n' {
                        scan += 1;
                    }
                }
                _ => return false, // Non-blank content reached — no blank line.
            }
        }
        false // EOF — no blank line found.
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
                // Track nesting inside active interpolations so inner `{` blocks
                // (e.g. `if` bodies) don't accidentally close the interpolation.
                if !self.interp_depth_stack.is_empty() {
                    *self.interp_depth_stack.last_mut().unwrap() += 1;
                }
                self.push_token(Token::LBrace, start, self.pos);
            }
            b'}' => {
                self.pos += 1;
                match self.interp_depth_stack.last_mut() {
                    Some(depth) if *depth > 0 => {
                        // Inner block close — decrement nesting counter.
                        *depth -= 1;
                        self.push_token(Token::RBrace, start, self.pos);
                    }
                    Some(_) => {
                        // depth == 0: this `}` closes the interpolation.
                        self.interp_depth_stack.pop();
                        self.push_token(Token::InterpolationEnd, start, self.pos);
                        self.after_interp_end = true;
                    }
                    None => {
                        // Normal block close — not inside any interpolation.
                        self.push_token(Token::RBrace, start, self.pos);
                    }
                }
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
                } else if self.peek(1) == Some(b'>') {
                    self.pos += 2;
                    self.push_token(Token::FatArrow, start, self.pos);
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

            b'"' => self.lex_double_quote_error(start),
            b'\'' => self.lex_single_quote_error(start),

            // `#` is a comment marker in Python/shell. Consume to end of line so the
            // rest of the line doesn't cascade into identifier/keyword parse errors.
            b'#' => {
                while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                    self.pos += 1;
                }
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, self.pos),
                    "Yinz uses `//` for comments, not `#`.",
                    "Change `#` to `//`: `// this is a comment`",
                    "Yinz follows the C/Rust/Java comment style. \
                     `#` is not a valid comment or operator in Yinz.",
                ));
            }

            // `;` is a statement terminator in JS/Rust/C. Yinz uses newlines instead.
            b';' => {
                self.pos += 1;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, self.pos),
                    "Semicolons are not used in Yinz.",
                    "Remove the `;` — Yinz uses newlines to end statements.",
                    "In JavaScript, Rust, and C, semicolons end statements. \
                     In Yinz, a newline is enough. Your code is correct without the `;`.",
                ));
            }

            // `$var` is PHP/shell syntax. Variables in Yinz have no prefix.
            b'$' => {
                self.pos += 1;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, self.pos),
                    "Variables in Yinz don't use a `$` prefix.",
                    "Remove the `$`: write `x` instead of `$x`.",
                    "In PHP and shell scripts, `$` marks a variable. \
                     In Yinz, variable names start with a letter, no prefix needed.",
                ));
            }

            // `T?` nullable syntax (Swift/Kotlin/TypeScript). Yinz uses `maybe<T>`.
            b'?' => {
                self.pos += 1;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, self.pos),
                    "The `?` optional suffix is not valid in Yinz.",
                    "Use `maybe<T>` instead: `maybe<int>` not `int?`.",
                    "Swift, Kotlin, and TypeScript use `T?` for optional types. \
                     Yinz uses `maybe<T>` — it reads the same way but is unambiguous.",
                ));
            }

            b'`' => self.lex_backtick_segment(),

            b'0'..=b'9' => self.lex_number(start),

            b if b.is_ascii_alphabetic() || b == b'_' => self.lex_identifier_or_keyword(start),

            b => {
                let codepoint_len = utf8_codepoint_len(b);
                let end = (self.pos + codepoint_len).min(self.src.len());
                self.pos = end;
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, start, end),
                    format!(
                        "The character `{}` is not valid here.",
                        byte_as_char_display(self.src, start, end)
                    ),
                    "Remove or replace this character.",
                    "Yinz source files may only contain ASCII text and UTF-8 string content.",
                ));
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
        if self.pos - start > 1024 {
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, start, self.pos),
                "Identifier too long (max 1024 characters).",
                "Split it into a short binding name and a longer descriptive comment, or trim the name.",
                "Identifiers this long usually indicate a typo where a closing quote or boundary character is missing, or an attempt to exhaust memory. The limit is set far above realistic usage.",
            ));
            self.push_token(Token::Identifier(String::new()), start, self.pos);
            return;
        }
        // Invariant: load.rs validates UTF-8 before constructing a Lexer; every
        // byte in self.src is valid UTF-8. Identifier bytes are ASCII (alphanumeric
        // + '_'), which are always valid single-byte UTF-8 sequences, so this slice
        // is guaranteed to be valid.
        let text = std::str::from_utf8(&self.src[start..self.pos])
            .expect("identifier slice is valid UTF-8 — source was validated at load time");
        let tok = match text {
            "function" => Token::Function,
            "nothing" => Token::Nothing,
            "let" => Token::Let,
            "const" => Token::Const,
            "true" => Token::True,
            "false" => Token::False,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "return" => Token::Return,
            // M3 banned keywords — redirect to Yinz equivalents
            "match" | "switch" => {
                self.emit_banned_keyword(start, self.pos, text);
                Token::Identifier(text.to_string())
            }
            // M4 keywords
            "shape" => Token::Shape,
            "follows" => Token::Follows,
            "extends" => Token::Extends,
            "base" => Token::Base,
            "hidden" => Token::Hidden,
            "dynamic" => Token::Dynamic,
            "Self" => Token::SelfType,
            "self" => Token::SelfValue,
            // M5 keywords
            "none" => Token::None,
            // M6 keywords
            "options" => Token::Options,
            "is" => Token::Is,
            // M7 keywords
            "errors" => Token::Errors,
            // M8 keywords
            "import" => Token::Import,
            "export" => Token::Export,
            "sensitive" => Token::Sensitive,
            "wait" => Token::Wait,
            "background" => Token::Background,
            // M8 banned concurrency keywords — other languages' concurrency vocabulary
            "async" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "async",
                    "Write `wait foo()` at the call site — no declaration-level annotation needed.",
                    "Yinz determines concurrency from the call graph automatically. Functions don't need a special declaration.",
                );
                Token::Identifier(text.to_string())
            }
            "await" => {
                self.emit_banned_declaration_keyword(
                    start,
                    self.pos,
                    "await",
                    "Write `wait foo()` at the call site.",
                    "Yinz uses `wait` at the call site — no function-level annotation needed.",
                );
                Token::Identifier(text.to_string())
            }
            "promise" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "promise",
                    "Yinz has `wait` (force completion) and `background` (run after this function returns). Two keywords cover what other languages use a dozen for.",
                    "A `background` task runs when called; use `wait` to force completion. No promise objects needed.",
                );
                Token::Identifier(text.to_string())
            }
            "future" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "future",
                    "Yinz has `wait` (force completion) and `background` (run after this function returns). Two keywords cover what other languages use a dozen for.",
                    "A `background` task runs when called; use `wait` to force completion. No future objects needed.",
                );
                Token::Identifier(text.to_string())
            }
            "goroutine" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "goroutine",
                    "Yinz has `background` for running a function after the current function returns: `background process(data)`.",
                    "Use `background foo(arg)` instead. Two keywords (`wait`, `background`) cover what other languages use a dozen for.",
                );
                Token::Identifier(text.to_string())
            }
            // `test` is reserved for the built-in test framework shipping in v0.14.
            // Error text is registry-driven via render_deferred_feature().
            "test" => {
                let entry = ynz_registry::deferred_language_feature_lookup("test")
                    .expect("registry missing deferred_language_feature 'test'");
                let (what_instead, why) =
                    ynz_diagnostics::deferred_feature::render_deferred_feature(entry);
                self.emit_banned_declaration_keyword(start, self.pos, "test", what_instead, why);
                Token::Identifier(text.to_string())
            }
            // M8 banned visibility keywords — Yinz uses `export` or private-by-default
            "pub" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "pub",
                    "Yinz has two visibility states — exported (`export`) or private (default). No `pub`, no `protected`, no modifiers.",
                    "Write `export function foo()` to make it visible to other modules. Without `export`, it's private automatically.",
                );
                Token::Identifier(text.to_string())
            }
            "private" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "private",
                    "Yinz has two visibility states — exported (`export`) or private (default). Declarations are private unless marked `export`.",
                    "Remove `private` — in Yinz, anything without `export` is already private.",
                );
                Token::Identifier(text.to_string())
            }
            "protected" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "protected",
                    "Yinz has two visibility states — exported (`export`) or private (default). No `pub`, no `protected`, no modifiers.",
                    "Remove `protected` — in Yinz, visibility is binary: `export` or private.",
                );
                Token::Identifier(text.to_string())
            }
            "public" => {
                self.emit_banned_declaration_keyword(
                    start, self.pos, "public",
                    "Yinz has two visibility states — exported (`export`) or private (default). Use `export` to make a declaration visible.",
                    "Write `export function foo()` instead of `public function foo()`.",
                );
                Token::Identifier(text.to_string())
            }
            // M4 banned declaration keywords — redirect to Yinz equivalents
            "type" => {
                self.emit_banned_declaration_keyword(start, self.pos, "type",
                    "Use `shape` to declare a data type: `shape Player { name: string, health: int }`",
                    "`type` is common in TypeScript, but Yinz uses `shape` — one unambiguous keyword \
                     for all data type declarations.",
                );
                Token::Identifier(text.to_string())
            }
            "struct" => {
                self.emit_banned_declaration_keyword(start, self.pos, "struct",
                    "Use `shape` to declare a data type: `shape Player { name: string, health: int }`",
                    "Yinz uses `shape` for all data type declarations, whether they would be called \
                     structs, classes, or interfaces in other languages.",
                );
                Token::Identifier(text.to_string())
            }
            "class" => {
                self.emit_banned_declaration_keyword(start, self.pos, "class",
                    "Use `shape` for data and standalone functions for behavior: \
                     `shape Player { name: string }` then `function greet(share self: Player) -> string`",
                    "Yinz is not object-oriented — there are no classes. Data lives in shapes; \
                     behavior lives in standalone functions called via dot-call syntax.",
                );
                Token::Identifier(text.to_string())
            }
            "interface" => {
                self.emit_banned_declaration_keyword(start, self.pos, "interface",
                    "For contracts, declare a `shape` with bare method signatures and use `follows`: \
                     `shape Damageable { takeDamage(lend self, amount: int) -> nothing }` \
                     then `shape Player follows Damageable { ... }`",
                    "Yinz uses `shape` for both data types and contracts. A contract shape holds \
                     bare-signature declarations; implementing shapes use the `follows` keyword.",
                );
                Token::Identifier(text.to_string())
            }
            "enum" => {
                self.emit_banned_declaration_keyword(start, self.pos, "enum",
                    "Use `options` to declare a set of named values: \
                     `options Status { active, inactive, banned }`",
                    "Yinz uses human-readable keywords. `options` immediately signals a set of choices.",
                );
                Token::Identifier(text.to_string())
            }
            "abstract" => {
                self.emit_banned_declaration_keyword(start, self.pos, "abstract",
                    "Use `base shape` for a shape that cannot be instantiated directly: \
                     `base shape Entity { name: string }`",
                    "`base shape` names what the type is before saying it cannot be instantiated — \
                     the constraint is part of the declaration, not a separate modifier.",
                );
                Token::Identifier(text.to_string())
            }
            // Sized numeric types from C/Rust/Go — reserved, ship in v2+ with kernel/embedded targets.
            // Error text is registry-driven via render_deferred_feature().
            "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => {
                let entry =
                    ynz_registry::deferred_language_feature_lookup(text).unwrap_or_else(|| {
                        panic!("registry missing deferred_language_feature {text:?}")
                    });
                let (what_instead, why) =
                    ynz_diagnostics::deferred_feature::render_deferred_feature(entry);
                self.emit_banned_declaration_keyword(start, self.pos, text, what_instead, why);
                Token::Identifier(text.to_string())
            }

            other => Token::Identifier(other.to_string()),
        };
        self.push_token(tok, start, self.pos);
    }

    /// M7: double-quoted strings are not valid Yinz syntax. Redirect to backticks.
    ///
    /// Advances past the entire `"..."` content for error recovery, emits ONE
    /// teaching diagnostic pointing at the opening quote, then produces an empty
    /// `BacktickString` token so downstream parsing can continue without cascading errors.
    ///
    /// Recovery consumes up to the matching closing `"`, or a blank line (two
    /// consecutive newlines — an unambiguous block boundary), or EOF — whichever
    /// comes first. This avoids the cascade of "word is undefined" errors that
    /// result from stopping at the first `\n` and re-lexing the rest as code.
    fn lex_double_quote_error(&mut self, start: usize) {
        self.pos += 1; // skip opening `"`
        let mut last_was_newline = false;
        loop {
            if self.pos >= self.src.len() {
                break;
            }
            let b = self.src[self.pos];
            if b == b'"' {
                self.pos += 1; // skip closing `"`
                break;
            }
            if b == b'\n' {
                if last_was_newline {
                    // Two consecutive newlines = blank line = block boundary; stop here.
                    break;
                }
                last_was_newline = true;
            } else {
                last_was_newline = false;
            }
            self.pos += 1;
        }
        self.diags.push(Diagnostic::error(
            SourceSpan::new(self.file, start, start + 1),
            "Double-quoted strings don't exist in Yinz.",
            "Use backtick strings instead: `` `hello` `` or `` `Hello ${name}` ``",
            "Yinz has one string form — backtick-quoted strings. They always support \
             interpolation and span multiple lines without extra syntax.",
        ));
        // Emit an empty BacktickString so the parser sees a string token and can recover.
        self.push_token(Token::BacktickString(vec![]), start, self.pos);
    }

    /// Single-quoted strings are not valid Yinz syntax. Mirrors lex_double_quote_error:
    /// consumes the whole `'...'` content for recovery, emits one teaching diagnostic,
    /// produces an empty BacktickString token so the parser sees a string and doesn't cascade.
    fn lex_single_quote_error(&mut self, start: usize) {
        self.pos += 1; // skip opening `'`
        while self.pos < self.src.len()
            && self.src[self.pos] != b'\''
            && self.src[self.pos] != b'\n'
        {
            self.pos += 1;
        }
        if self.pos < self.src.len() && self.src[self.pos] == b'\'' {
            self.pos += 1; // skip closing `'`
        }
        self.diags.push(Diagnostic::error(
            SourceSpan::new(self.file, start, self.pos),
            "Single-quoted strings don't exist in Yinz.",
            "Use backtick strings instead: `` `hello world` ``",
            "Yinz has one string form — backtick strings. \
             They support interpolation and span multiple lines without extra syntax.",
        ));
        self.push_token(Token::BacktickString(vec![]), start, self.pos);
    }

    /// Entry point from `lex_one` — current position is at the opening `` ` ``.
    /// Consumes the backtick, then delegates to `lex_backtick_content`.
    fn lex_backtick_segment(&mut self) {
        // Consume the opening backtick.
        debug_assert!(
            self.src.get(self.pos) == Some(&b'`'),
            "lex_backtick_segment called without leading backtick"
        );
        self.pos += 1;
        self.lex_backtick_content();
    }

    /// Lex one contiguous literal segment of a backtick string starting from
    /// the current position (which is INSIDE the string, after the opening backtick).
    ///
    /// Called either from `lex_backtick_segment` (after consuming the opening `` ` ``)
    /// or from `run()` after an interpolation closes (`after_interp_end` path).
    ///
    /// Exits on:
    ///   - closing `` ` `` (end of string) — consumes it and emits `BacktickString`
    ///   - `${` (start of interpolation) — emits `BacktickString` + `InterpolationStart`, pushes depth 0
    ///   - EOF (unterminated string) — emits diagnostic + `BacktickString`
    fn lex_backtick_content(&mut self) {
        let seg_start = self.pos;
        let mut bytes: Vec<u8> = Vec::new();

        loop {
            if self.pos >= self.src.len() {
                self.diags.push(Diagnostic::error(
                    SourceSpan::new(self.file, seg_start, self.pos),
                    "A backtick string is missing its closing backtick.",
                    "Add a closing `` ` `` at the end of the string.",
                    "Backtick strings start and end with `` ` ``. Multi-line strings \
                     are fine — just make sure the string has a closing backtick.",
                ));
                self.push_token(Token::BacktickString(bytes), seg_start, self.pos);
                return;
            }

            match self.src[self.pos] {
                b'`' => {
                    // End of string — emit the final literal segment and consume the backtick.
                    self.push_token(Token::BacktickString(bytes), seg_start, self.pos);
                    self.pos += 1;
                    return;
                }
                b'$' if self.peek(1) == Some(b'{') => {
                    if self.interp_depth_stack.len() >= 64 {
                        self.diags.push(Diagnostic::error(
                            SourceSpan::new(self.file, self.pos, self.pos + 2),
                            "String interpolation nested too deeply (max 64 levels).",
                            "Build the string with named `let` bindings instead of inline interpolation.",
                            "Each `${...}` opens a new nesting level. Beyond 64 the lexer can't recover meaningfully; the limit catches malformed input and adversarial nesting.",
                        ));
                        // Consume `${` and continue so the lexer doesn't spin.
                        self.pos += 2;
                        continue;
                    }
                    // Start of interpolation — emit what we have so far, then the marker.
                    self.push_token(Token::BacktickString(bytes), seg_start, self.pos);
                    let interp_start = self.pos;
                    self.pos += 2; // skip `${`
                    self.push_token(Token::InterpolationStart, interp_start, self.pos);
                    self.interp_depth_stack.push(0);
                    return; // return to normal lex mode for the interpolated expression
                }
                b'\\' => {
                    self.pos += 1;
                    if self.pos >= self.src.len() {
                        self.diags.push(Diagnostic::error(
                            SourceSpan::new(self.file, self.pos - 1, self.pos),
                            "A string escape sequence is incomplete.",
                            "Add the character after `\\`, e.g. `\\n` for newline or `\\\\` for a literal backslash.",
                            "Every `\\` in a string must be followed by an escape character: \
                             `\\n`, `\\t`, `\\\\`, `` \\` ``, `\\${{`, `\\r`, or `\\0`.",
                        ));
                        break;
                    }
                    match self.src[self.pos] {
                        b'`' => {
                            bytes.push(b'`');
                            self.pos += 1;
                        }
                        b'n' => {
                            bytes.push(b'\n');
                            self.pos += 1;
                        }
                        b't' => {
                            bytes.push(b'\t');
                            self.pos += 1;
                        }
                        b'\\' => {
                            bytes.push(b'\\');
                            self.pos += 1;
                        }
                        b'r' => {
                            bytes.push(b'\r');
                            self.pos += 1;
                        }
                        b'0' => {
                            self.diags.push(Diagnostic::error(
                                SourceSpan::new(self.file, self.pos - 1, self.pos + 1),
                                "`\\0` (NUL byte) is not a valid escape in Yinz strings.",
                                "Use a non-NUL placeholder like `*` or split the string into pieces if you need a sentinel.",
                                "Yinz strings are passed to the runtime as C strings (length-prefixed string slices ship in a future version). Embedding a NUL byte would silently truncate the string at print/concatenate time. The compiler refuses up-front to avoid the silent-truncation footgun.",
                            ));
                            self.pos += 1;
                        }
                        b'$' if self.peek(1) == Some(b'{') => {
                            // `\${` — literal `${` (not an interpolation)
                            bytes.push(b'$');
                            bytes.push(b'{');
                            self.pos += 2;
                        }
                        other => {
                            self.diags.push(Diagnostic::error(
                                SourceSpan::new(self.file, self.pos - 1, self.pos + 1),
                                format!("Unknown escape sequence `\\{}`.", other as char),
                                "Valid escape sequences: `\\n`, `\\t`, `\\\\`, `` \\` ``, `\\${{`, `\\r`, `\\0`.",
                                "Each `\\` must be followed by one of the listed characters.",
                            ));
                            bytes.push(other);
                            self.pos += 1;
                        }
                    }
                }
                b => {
                    bytes.push(b);
                    self.pos += 1;
                }
            }
        }
        self.push_token(Token::BacktickString(bytes), seg_start, self.pos);
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

        if self.pos < self.src.len() && self.src[self.pos] == b'_' {
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, self.pos, self.pos + 1),
                "Numeric literals cannot start with `_`.",
                "Use a single `_` between groups of digits: `0xDEAD_BEEF`.",
                "The `_` separator is a visual aid — it must sit between digit groups, not at the start.",
            ));
            self.skip_to_whitespace();
            return;
        }

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

        // Invariant: lex_hex_int only advances self.pos over bytes matching
        // b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' | b'_', all of which are
        // ASCII — valid single-byte UTF-8.
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

        if self.pos < self.src.len() && self.src[self.pos] == b'_' {
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, self.pos, self.pos + 1),
                "Numeric literals cannot start with `_`.",
                "Use a single `_` between digit groups: `0b1111_0000`.",
                "The `_` separator is a visual aid — it must sit between digit groups, not at the start.",
            ));
            self.skip_to_whitespace();
            return;
        }

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

        // Invariant: lex_binary_int only advances over b'0', b'1', and b'_',
        // all ASCII — valid single-byte UTF-8.
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
        while self.pos < self.src.len() && matches!(self.src[self.pos], b'0'..=b'9' | b'_') {
            self.pos += 1;
        }
        // Invariant: lex_decimal_number only advances over b'0'..=b'9' and b'_',
        // all ASCII — valid single-byte UTF-8.
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

        // Phase 2 — optional fractional part (only if `.` is followed by a digit).
        // `3.` followed by EOF, whitespace, or an operator is an error — ambiguous intent.
        // `42.toString()` is fine — `.` followed by a letter is a method-call dot, not decimal.
        let next_after_dot = self
            .pos
            .checked_add(1)
            .and_then(|i| self.src.get(i))
            .copied();
        if self.pos < self.src.len()
            && self.src[self.pos] == b'.'
            && !matches!(next_after_dot, Some(b'0'..=b'9'))
            && !matches!(next_after_dot, Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        {
            let dot_pos = self.pos;
            self.diags.push(Diagnostic::error(
                SourceSpan::new(self.file, dot_pos, dot_pos + 1),
                "Decimal point without fractional digits.",
                "Write `3.0` if you mean three, or `3` if you mean integer three.",
                "`3.` is ambiguous — could be a float literal or a typo. Yinz requires the digit so the intent is clear at the point of writing.",
            ));
            self.skip_to_whitespace();
            return;
        }

        let has_dot = self.pos < self.src.len()
            && self.src[self.pos] == b'.'
            && self.pos + 1 < self.src.len()
            && self.src[self.pos + 1].is_ascii_digit();

        if has_dot {
            self.pos += 1; // consume `.`
            let frac_start = self.pos;
            while self.pos < self.src.len() && matches!(self.src[self.pos], b'0'..=b'9' | b'_') {
                self.pos += 1;
            }
            // Invariant: fractional digits are b'0'..=b'9' | b'_', all ASCII.
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
        let has_exp = self.pos < self.src.len() && matches!(self.src[self.pos], b'e' | b'E');

        if has_exp {
            let e_pos = self.pos;
            self.pos += 1; // consume `e` or `E`
            if self.pos < self.src.len() && matches!(self.src[self.pos], b'+' | b'-') {
                self.pos += 1; // consume optional sign
            }
            let exp_digits_start = self.pos;
            while self.pos < self.src.len() && matches!(self.src[self.pos], b'0'..=b'9' | b'_') {
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
            // Invariant: exponent digits are b'0'..=b'9' | b'_', all ASCII.
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
        // Invariant: the full decimal literal span (int part + optional dot +
        // frac + optional exponent) consists only of b'0'..=b'9', b'.',
        // b'e'/'E', b'+'/b'-', and b'_', all ASCII — valid UTF-8.
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

    fn emit_banned_compound_op(&mut self, start: usize, end: usize, op: &str, suggestion: &str) {
        self.diags.push(Diagnostic::error(
            SourceSpan::new(self.file, start, end),
            format!("`{}` is not supported in Yinz.", op),
            format!("Use `{}` instead.", suggestion),
            "Compound assignment and increment operators are not part of the Yinz language. \
             Step-by-step assignment on its own line makes the intent explicit.",
        ));
    }

    fn emit_banned_keyword(&mut self, start: usize, end: usize, keyword: &str) {
        self.diags.push(Diagnostic::warning(
            SourceSpan::new(self.file, start, end),
            format!("`{keyword}` is not a keyword in Yinz."),
            "Use multi-case `if` with `=>` arms: `if (value) { 1 => ...; 2 => ...; else => ... }`",
            "Yinz uses one `if` keyword for simple branches, value matching, and type narrowing. \
             One concept, one keyword.",
        ));
    }

    fn emit_banned_declaration_keyword(
        &mut self,
        start: usize,
        end: usize,
        keyword: &str,
        what_instead: &str,
        why: &str,
    ) {
        self.diags.push(Diagnostic::warning(
            SourceSpan::new(self.file, start, end),
            format!("`{keyword}` is not a keyword in Yinz."),
            what_instead,
            why,
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

/// Return the byte length of the UTF-8 codepoint starting with `first_byte`.
/// Defaults to 1 for any invalid / continuation byte.
fn utf8_codepoint_len(first_byte: u8) -> usize {
    match first_byte {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

/// Produce a display string for the codepoint at `src[start..end]`.
/// Falls back to a hex escape if the bytes are not valid UTF-8.
fn byte_as_char_display(src: &[u8], start: usize, end: usize) -> String {
    match std::str::from_utf8(&src[start..end]) {
        Ok(s) => s.to_string(),
        Err(_) => format!("\\x{:02X}", src[start]),
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
