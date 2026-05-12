/// Hand-written recursive-descent parser for M2 (Pratt precedence climber for expressions).
///
/// Error recovery strategy:
///   - In statement position: on an unexpected token, emit a diagnostic, consume the
///     offending token, and return `Some(Stmt::Expr(Expr::Error(...)))` so the block
///     parser keeps going.
///   - In expression atom position: if the unexpected token is `}`, `let`, `const`,
///     `function`, or EOF, do NOT consume it — return `Expr::Error` and let the
///     enclosing statement parser see the boundary. Otherwise consume and return Error.
///   - In the Pratt loop: on a missing RHS after a binary operator, the atom parser
///     returns `Expr::Error` without consuming the boundary token. The result is a
///     `BinOp` with an Error RHS, which the type-checker gates on (skips the body).
///   - `parse_stmt` always returns `Some(_)`, never `None`. `None` is reserved for
///     `parse_function_decl` which has no meaningful partial result.
///
/// Every AST node carries a SourceSpan so downstream stages can point
/// diagnostics at exact source locations.
use ynz_ast::nodes::{
    BinOpKind, Block, CallExpr, Expr, FunctionDecl, Item, Module, Stmt, Type, UnaryOpKind,
};
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

    fn peek_ahead(&self, offset: usize) -> &Token {
        self.tokens
            .get(self.pos + offset)
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

    /// True when the current token is a statement-boundary sentinel.
    /// The atom parser uses this to avoid consuming delimiters that belong
    /// to the enclosing block or function.
    fn is_stmt_boundary(&self) -> bool {
        matches!(
            self.peek(),
            Token::RBrace
                | Token::Eof
                | Token::Let
                | Token::Const
                | Token::Function
        )
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

        // M1/M2: no parameters. Immediately expect `)`.
        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("`{name}` does not take parameters yet."),
                format!("Use `function {name}()` with empty parentheses."),
                "Parameters are added in Milestone 3.",
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

    // ── types ─────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Type {
        match self.peek().clone() {
            Token::Nothing => {
                self.advance();
                Type::Nothing
            }
            Token::Identifier(name) => {
                let span = self.current_span();
                self.advance();
                match name.as_str() {
                    "int" => Type::Int,
                    "float" => Type::Float,
                    "bool" => Type::Bool,
                    "number" => self.parse_number_type(),
                    _ => Type::Named(name, span),
                }
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a type here.",
                    "Use `nothing`, `int`, `float`, `number`, `bool`, or a type name.",
                    "Return types tell Yinz (and the next developer reading your code) what the function produces.",
                ));
                Type::Error
            }
        }
    }

    /// Parse `number` or `number[N]`.
    ///
    /// Called after `number` has already been consumed. If `[N]` follows,
    /// consumes the brackets and validates the precision. Only N=34 is
    /// supported in M2; any other value produces a deferral diagnostic.
    fn parse_number_type(&mut self) -> Type {
        if !matches!(self.peek(), Token::LBracket) {
            return Type::Number { precision: 34 };
        }

        self.advance(); // consume `[`

        let precision = match self.peek().clone() {
            Token::IntLit(n) if n > 0 && n <= 4096 => {
                let p = n as u32;
                self.advance();
                p
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a precision value between 1 and 4096 inside `number[N]`.",
                    "Write `number[34]` for the default 34-digit decimal.",
                    "The number in `number[N]` sets how many decimal digits of precision you get.",
                ));
                // Consume to `]` to recover
                while !matches!(self.peek(), Token::RBracket | Token::RBrace | Token::Eof) {
                    self.advance();
                }
                let _ = self.expect(&Token::RBracket);
                return Type::Error;
            }
        };

        if self.expect(&Token::RBracket).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `]` to close `number[N]`.",
                "Write `number[34]` — close the bracket after the precision.",
                "The bracket must be closed to complete the type.",
            ));
            return Type::Error;
        }

        if precision != 34 {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("`number[{precision}]` is not available yet."),
                "Use `number` (34 decimal digits) for now.",
                "`number[N]` for N other than 34 (bignum support) arrives in v0.8.",
            ));
            return Type::Error;
        }

        Type::Number { precision: 34 }
    }

    // ── blocks and statements ─────────────────────────────────────────────

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
        match self.peek() {
            Token::Let | Token::Const => Some(self.parse_let_or_const()),
            Token::Identifier(_) if *self.peek_ahead(1) == Token::Eq => {
                Some(self.parse_assign())
            }
            _ => {
                let expr = self.parse_expr(0);
                Some(Stmt::Expr(expr))
            }
        }
    }

    fn parse_let_or_const(&mut self) -> Stmt {
        let start = self.current_span();
        let is_const = matches!(self.peek(), Token::Const);
        let kw = if is_const { "const" } else { "let" };
        self.advance(); // consume `let` or `const`

        // Name
        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                let span = self.current_span();
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Expected a variable name after `{kw}`."),
                    format!("Write: `{kw} name = value`"),
                    "Variable declarations need a name to bind the value to.",
                ));
                // Don't consume — let the enclosing block see the boundary token.
                return Stmt::Expr(Expr::Error(span));
            }
        };

        // Optional `: type`
        let ty = if matches!(self.peek(), Token::Colon) {
            self.advance(); // consume `:`
            Some(self.parse_type())
        } else {
            None
        };

        // `=`
        if self.expect(&Token::Eq).is_none() {
            let span = self.current_span();
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("`{name}` declaration is missing `=`."),
                format!("Write: `{kw} {name} = value`"),
                "Variable declarations must assign an initial value.",
            ));
            return Stmt::Let {
                is_const,
                name,
                name_span: name_span.clone(),
                ty,
                value: Expr::Error(span),
                span: name_span,
            };
        }

        let value = self.parse_expr(0);
        let end = value.span().end;
        Stmt::Let {
            is_const,
            name,
            name_span,
            ty,
            value,
            span: SourceSpan::new(self.file, start.start, end),
        }
    }

    fn parse_assign(&mut self) -> Stmt {
        let (target, target_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => unreachable!("parse_assign called without Identifier token"),
        };
        let start = target_span.start;
        self.advance(); // consume `=`
        let value = self.parse_expr(0);
        let end = value.span().end;
        Stmt::Assign {
            target,
            target_span,
            value,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    // ── Pratt expression parser ───────────────────────────────────────────

    /// Parse an expression with the given minimum binding power.
    ///
    /// Binding powers follow the operator precedence table in `spec/operators.md`:
    ///   `||`=2  `&&`=4  `|`=6  `^`=8  `&`=10  `==`/`!=`=12
    ///   `<`/`>`/`<=`/`>=`=14  `<<`/`>>`=16  `+`/`-`=18  `*`/`/`/`%`=20
    ///
    /// Call (postfix `(`) and method-call (postfix `.`) have the highest
    /// effective priority: they are consumed greedily before any infix check.
    ///
    /// Unary prefix operators (`-`, `!`, `~`) use right-BP=21 (higher than `*`/`/`).
    pub fn parse_expr(&mut self, min_bp: u8) -> Expr {
        let mut lhs = self.parse_prefix();

        loop {
            // Postfix (highest precedence): call `(` or method `.`
            match self.peek() {
                Token::LParen => {
                    lhs = self.parse_call(lhs);
                    continue;
                }
                Token::Dot => {
                    lhs = self.parse_method_call(lhs);
                    continue;
                }
                _ => {}
            }

            // Binary infix
            let Some((lbp, rbp)) = infix_bp(self.peek()) else {
                break;
            };
            if lbp <= min_bp {
                break;
            }
            let op_span = self.current_span();
            let op = self.consume_bin_op();
            let rhs = self.parse_expr(rbp);
            let span =
                SourceSpan::new(self.file, lhs.span().start, rhs.span().end);
            let _ = op_span;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                span,
            };
        }

        lhs
    }

    fn parse_prefix(&mut self) -> Expr {
        match self.peek() {
            Token::Minus => {
                let span = self.current_span();
                self.advance();
                let operand = self.parse_expr(21); // right-BP for unary (above *)
                let end = operand.span().end;
                Expr::UnaryOp {
                    op: UnaryOpKind::Neg,
                    operand: Box::new(operand),
                    span: SourceSpan::new(self.file, span.start, end),
                }
            }
            Token::Bang => {
                let span = self.current_span();
                self.advance();
                let operand = self.parse_expr(21);
                let end = operand.span().end;
                Expr::UnaryOp {
                    op: UnaryOpKind::Not,
                    operand: Box::new(operand),
                    span: SourceSpan::new(self.file, span.start, end),
                }
            }
            Token::Tilde => {
                let span = self.current_span();
                self.advance();
                let operand = self.parse_expr(21);
                let end = operand.span().end;
                Expr::UnaryOp {
                    op: UnaryOpKind::BitNot,
                    operand: Box::new(operand),
                    span: SourceSpan::new(self.file, span.start, end),
                }
            }
            _ => self.parse_atom(),
        }
    }

    fn parse_atom(&mut self) -> Expr {
        match self.peek().clone() {
            Token::IntLit(n) => {
                let span = self.current_span();
                self.advance();
                Expr::IntLit(n, span)
            }
            Token::NumberLit(s) => {
                let span = self.current_span();
                self.advance();
                Expr::NumberLit(s, span)
            }
            Token::True => {
                let span = self.current_span();
                self.advance();
                Expr::BoolLit(true, span)
            }
            Token::False => {
                let span = self.current_span();
                self.advance();
                Expr::BoolLit(false, span)
            }
            Token::StringLit(bytes) => {
                let span = self.current_span();
                self.advance();
                Expr::StringLit(bytes, span)
            }
            Token::Identifier(name) => {
                let span = self.current_span();
                self.advance();
                Expr::Ident(name, span)
            }
            Token::LParen => {
                self.advance(); // consume `(`
                let inner = self.parse_expr(0);
                if self.expect(&Token::RParen).is_none() {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Missing `)` to close this expression.",
                        "Add `)` after the expression.",
                        "Every `(` must be matched with a `)`.",
                    ));
                }
                inner
            }
            _ => {
                let span = self.current_span();
                if self.is_stmt_boundary() {
                    // Don't consume — let the block/statement parser handle the boundary.
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!(
                            "Expected a value here, but found `{}`.",
                            token_display(self.peek())
                        ),
                        "Write a value, variable name, or expression.",
                        "Expressions need something to evaluate — a number, name, or calculation.",
                    ));
                } else {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!(
                            "`{}` cannot appear here.",
                            token_display(self.peek())
                        ),
                        "Write a value or expression instead.",
                        "Expressions need something to evaluate — a number, name, or calculation.",
                    ));
                    self.advance();
                }
                Expr::Error(span)
            }
        }
    }

    // ── call and method call ──────────────────────────────────────────────

    fn parse_call(&mut self, callee: Expr) -> Expr {
        let start = callee.span().start;
        self.advance(); // consume `(`

        let mut args = Vec::new();
        loop {
            match self.peek() {
                Token::RParen => {
                    let end = self.current_span();
                    self.advance();
                    return Expr::Call(Box::new(CallExpr {
                        callee,
                        args,
                        span: SourceSpan::new(self.file, start, end.end),
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
                Token::Comma => {
                    self.advance(); // consume `,` between args
                }
                _ => {
                    args.push(self.parse_expr(0));
                }
            }
        }
    }

    fn parse_method_call(&mut self, receiver: Expr) -> Expr {
        let start = receiver.span().start;
        self.advance(); // consume `.`

        // Method name
        let (method, method_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                let span = self.current_span();
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!(
                        "Expected a method name after `.`, but found `{}`.",
                        token_display(self.peek())
                    ),
                    "Write the method name: `value.toString()`",
                    "The `.` is a method-call separator — it must be followed by a method name.",
                ));
                return Expr::Error(span);
            }
        };

        // `(`
        if !matches!(self.peek(), Token::LParen) {
            let span = self.current_span();
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("Expected `(` after method name `{method}`."),
                format!("Write: `value.{method}()`"),
                "Method calls require parentheses — write the `()` even when there are no arguments.",
            ));
            return Expr::Error(span);
        }

        // Parse the argument list (reuses parse_call's loop logic)
        self.advance(); // consume `(`
        let mut args = Vec::new();
        let end_span = loop {
            match self.peek() {
                Token::RParen => {
                    let s = self.current_span();
                    self.advance();
                    break s;
                }
                Token::RBrace | Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Missing `)` to close the method argument list.",
                        "Add `)` after the last argument.",
                        "Every `(` in a method call must be matched with a `)`.",
                    ));
                    break self.current_span();
                }
                Token::Comma => {
                    self.advance();
                }
                _ => {
                    args.push(self.parse_expr(0));
                }
            }
        };

        Expr::MethodCall {
            receiver: Box::new(receiver),
            method,
            method_span,
            args,
            span: SourceSpan::new(self.file, start, end_span.end),
        }
    }

    // ── binary operator helpers ───────────────────────────────────────────

    fn consume_bin_op(&mut self) -> BinOpKind {
        let op = match self.peek() {
            Token::Plus => BinOpKind::Add,
            Token::Minus => BinOpKind::Sub,
            Token::Star => BinOpKind::Mul,
            Token::Slash => BinOpKind::Div,
            Token::Percent => BinOpKind::Rem,
            Token::Lt => BinOpKind::Lt,
            Token::LtEq => BinOpKind::LtEq,
            Token::Gt => BinOpKind::Gt,
            Token::GtEq => BinOpKind::GtEq,
            Token::EqEq => BinOpKind::EqEq,
            Token::NotEq => BinOpKind::NotEq,
            Token::AmpAmp => BinOpKind::And,
            Token::PipePipe => BinOpKind::Or,
            Token::Amp => BinOpKind::BitAnd,
            Token::Pipe => BinOpKind::BitOr,
            Token::Caret => BinOpKind::BitXor,
            Token::LtLt => BinOpKind::Shl,
            Token::GtGt => BinOpKind::Shr,
            _ => unreachable!("consume_bin_op called on non-operator token"),
        };
        self.advance();
        op
    }
}

/// Return the left and right binding powers for an infix binary operator token.
///
/// The BP values encode the precedence table from `spec/operators.md`.
/// Higher values bind more tightly. Left-associativity is achieved by
/// making rbp = lbp + 1 so the same-level operator on the right stops recursion.
///
/// `spec/operators.md` level → BP mapping (level 12 = loosest, level 3 = tightest):
///   level 12 `||`         → lbp=2,  rbp=3
///   level 11 `&&`         → lbp=4,  rbp=5
///   level 10 `|`          → lbp=6,  rbp=7
///   level  9 `^`          → lbp=8,  rbp=9
///   level  8 `&`          → lbp=10, rbp=11
///   level  7 `==` `!=`    → lbp=12, rbp=13
///   level  6 `<` `>` etc. → lbp=14, rbp=15
///   level  5 `<<` `>>`    → lbp=16, rbp=17
///   level  4 `+` `-`      → lbp=18, rbp=19
///   level  3 `*` `/` `%`  → lbp=20, rbp=21
pub fn infix_bp(tok: &Token) -> Option<(u8, u8)> {
    match tok {
        Token::PipePipe => Some((2, 3)),
        Token::AmpAmp => Some((4, 5)),
        Token::Pipe => Some((6, 7)),
        Token::Caret => Some((8, 9)),
        Token::Amp => Some((10, 11)),
        Token::EqEq | Token::NotEq => Some((12, 13)),
        Token::Lt | Token::Gt | Token::LtEq | Token::GtEq => Some((14, 15)),
        Token::LtLt | Token::GtGt => Some((16, 17)),
        Token::Plus | Token::Minus => Some((18, 19)),
        Token::Star | Token::Slash | Token::Percent => Some((20, 21)),
        _ => None,
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
        Token::Let => "let",
        Token::Const => "const",
        Token::True => "true",
        Token::False => "false",
        Token::IntLit(_) => "integer literal",
        Token::NumberLit(_) => "number literal",
        Token::Plus => "+",
        Token::Minus => "-",
        Token::Star => "*",
        Token::Slash => "/",
        Token::Percent => "%",
        Token::EqEq => "==",
        Token::NotEq => "!=",
        Token::Lt => "<",
        Token::LtEq => "<=",
        Token::Gt => ">",
        Token::GtEq => ">=",
        Token::AmpAmp => "&&",
        Token::PipePipe => "||",
        Token::Bang => "!",
        Token::Amp => "&",
        Token::Pipe => "|",
        Token::Caret => "^",
        Token::Tilde => "~",
        Token::LtLt => "<<",
        Token::GtGt => ">>",
        Token::Eq => "=",
        Token::Colon => ":",
        Token::Dot => ".",
        Token::LBracket => "[",
        Token::RBracket => "]",
        Token::Comma => ",",
    }
}
