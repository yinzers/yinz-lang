/// Hand-written recursive-descent parser for M1.
///
/// Error recovery strategy: on an unexpected token, emit a diagnostic,
/// then scan forward to the next `}` or end-of-file. The parser continues
/// from there so the caller sees as many errors as possible in one pass.
///
/// Every AST node carries a SourceSpan so downstream stages can point
/// diagnostics at exact source locations.
use ynz_ast::nodes::{Block, CallExpr, Expr, FunctionDecl, Item, Module, Stmt, Type};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::token::{Spanned, Token};

pub struct Parser<'a> {
    file: &'a str,
    tokens: &'a [Spanned<Token>],
    pos: usize,
    pub diags: DiagnosticBucket,
}

impl<'a> Parser<'a> {
    pub fn new(file: &'a str, tokens: &'a [Spanned<Token>]) -> Self {
        Self {
            file,
            tokens,
            pos: 0,
            diags: DiagnosticBucket::new(),
        }
    }

    // ── cursor helpers ────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|s| &s.value)
            .unwrap_or(&Token::Eof)
    }

    fn current_span(&self) -> SourceSpan {
        self.tokens
            .get(self.pos)
            .map(|s| s.span.clone())
            .unwrap_or_else(|| self.eof_span())
    }

    fn eof_span(&self) -> SourceSpan {
        // Point one byte past the last non-EOF token, or byte 0 if empty.
        let last = self.tokens.last();
        let offset = last.map(|s| s.span.end).unwrap_or(0);
        SourceSpan::new(self.file, offset, offset)
    }

    fn advance(&mut self) -> &Spanned<Token> {
        let tok = &self.tokens[self.pos];
        if !tok.value.is_eof() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Option<SourceSpan> {
        if self.peek() == expected {
            let span = self.current_span();
            self.advance();
            Some(span)
        } else {
            None
        }
    }

    /// Scan forward past all tokens until `}` or EOF to recover after a parse error.
    fn recover_to_rbrace(&mut self) {
        loop {
            match self.peek() {
                Token::RBrace | Token::Eof => break,
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn error_recovery_span(&self) -> SourceSpan {
        self.current_span()
    }

    // ── grammar ───────────────────────────────────────────────────────────

    pub fn parse_module(&mut self) -> Module {
        let start = self.current_span();
        let mut items = Vec::new();

        while !matches!(self.peek(), Token::Eof) {
            match self.peek() {
                Token::Function => {
                    if let Some(decl) = self.parse_function_decl() {
                        items.push(Item::Function(decl));
                    }
                }
                _ => {
                    let span = self.current_span();
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!(
                            "Unexpected `{}` at the top level.",
                            token_display(self.peek())
                        ),
                        "Top-level code must be inside a `function` declaration.",
                        "Yinz programs are made of functions. Only `function` declarations are allowed at the top level.",
                    ));
                    self.advance();
                }
            }
        }

        let end = self.current_span();
        Module {
            items,
            span: SourceSpan::new(self.file, start.start, end.end),
        }
    }

    fn parse_function_decl(&mut self) -> Option<FunctionDecl> {
        let start_span = self.current_span();
        self.advance(); // consume `function`

        // Name
        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a function name after `function`.",
                    "Write the function name: `function myFunction() -> nothing { }`",
                    "Every function needs a name so it can be called from other places.",
                ));
                self.recover_to_rbrace();
                return None;
            }
        };

        // `(`
        if self.expect(&Token::LParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Expected `(` after function name `{name}`."),
                format!("Write `function {name}()`."),
                "The parentheses are where parameters go. Even with no parameters, they are required.",
            ));
            self.recover_to_rbrace();
            return None;
        }

        // M1: no parameters. Immediately expect `)`.
        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("`{name}` does not take parameters yet."),
                format!("Use `function {name}()` with empty parentheses."),
                "M1 functions take no parameters. Parameters are added in Milestone 3.",
            ));
            self.recover_to_rbrace();
            return None;
        }

        // `->`
        if self.expect(&Token::Arrow).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Missing return type on `{name}`."),
                format!("Add `-> nothing` after the parentheses: `function {name}() -> nothing`"),
                "Every function in Yinz declares what it returns. Use `-> nothing` for functions that don't return a value.",
            ));
            self.recover_to_rbrace();
            return None;
        }

        // return type
        let return_type = self.parse_type();

        // `{`
        if self.expect(&Token::LBrace).is_none() {
            let span = self.current_span();
            self.diags.push(Diagnostic::error(
                span,
                format!("Expected `{{` to open the body of `{name}`."),
                format!("function {name}() -> nothing {{ ... }}"),
                "Function bodies start with `{` and end with `}`.",
            ));
            return None;
        }

        let body = self.parse_block();

        let end = self.current_span();
        Some(FunctionDecl {
            name,
            return_type,
            body,
            span: SourceSpan::new(self.file, start_span.start, end.end),
            name_span,
        })
    }

    fn parse_type(&mut self) -> Type {
        match self.peek().clone() {
            Token::Nothing => {
                self.advance();
                Type::Nothing
            }
            Token::Identifier(name) => {
                let span = self.current_span();
                self.advance();
                Type::Named(name, span)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a type here.",
                    "Use `nothing` for functions that don't return a value.",
                    "Return types tell Yinz (and the next developer reading your code) what the function produces.",
                ));
                Type::Error
            }
        }
    }

    fn parse_block(&mut self) -> Block {
        let start = self.current_span();
        let mut stmts = Vec::new();

        loop {
            match self.peek() {
                Token::RBrace => {
                    let end = self.current_span();
                    self.advance(); // consume `}`
                    return Block {
                        stmts,
                        span: SourceSpan::new(self.file, start.start, end.end),
                    };
                }
                Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.eof_span(),
                        "Missing closing `}` for this block.",
                        "Add `}` at the end of the function body.",
                        "Every `{` must be matched with a `}`. The compiler reached the end of the file before finding it.",
                    ));
                    return Block {
                        stmts,
                        span: SourceSpan::new(self.file, start.start, self.eof_span().end),
                    };
                }
                _ => {
                    if let Some(stmt) = self.parse_stmt() {
                        stmts.push(stmt);
                    }
                }
            }
        }
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        let expr = self.parse_expr();
        Some(Stmt::Expr(expr))
    }

    fn parse_expr(&mut self) -> Expr {
        match self.peek().clone() {
            Token::Identifier(name) => {
                let ident_span = self.current_span();
                self.advance();
                // Is this a call?
                if matches!(self.peek(), Token::LParen) {
                    self.parse_call(Expr::Ident(name, ident_span))
                } else {
                    Expr::Ident(name, ident_span)
                }
            }
            Token::StringLit(bytes) => {
                let span = self.current_span();
                self.advance();
                Expr::StringLit(bytes, span)
            }
            _ => {
                let span = self.error_recovery_span();
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!(
                        "Unexpected `{}` where an expression was expected.",
                        token_display(self.peek())
                    ),
                    "Write a function call or a value here.",
                    "Statements in a function body must be expressions.",
                ));
                // Skip the unexpected token and mark this as an error node.
                self.advance();
                Expr::Error(span)
            }
        }
    }

    fn parse_call(&mut self, callee: Expr) -> Expr {
        let start = callee.span().start;
        self.advance(); // consume `(`

        let mut args = Vec::new();

        loop {
            match self.peek() {
                Token::RParen => {
                    let end_span = self.current_span();
                    self.advance(); // consume `)`
                    return Expr::Call(Box::new(CallExpr {
                        callee,
                        args,
                        span: SourceSpan::new(self.file, start, end_span.end),
                    }));
                }
                Token::RBrace | Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Missing `)` to close the argument list.",
                        "Add `)` after the last argument.",
                        "Every `(` in a function call must be matched with a `)`.",
                    ));
                    let end = self.current_span().end;
                    return Expr::Call(Box::new(CallExpr {
                        callee,
                        args,
                        span: SourceSpan::new(self.file, start, end),
                    }));
                }
                _ => {
                    args.push(self.parse_expr());
                }
            }
        }
    }
}

fn token_display(tok: &Token) -> &str {
    match tok {
        Token::Function => "function",
        Token::Nothing => "nothing",
        Token::Identifier(_) => "identifier",
        Token::StringLit(_) => "string literal",
        Token::LParen => "(",
        Token::RParen => ")",
        Token::LBrace => "{",
        Token::RBrace => "}",
        Token::Arrow => "->",
        Token::Eof => "end of file",
    }
}
