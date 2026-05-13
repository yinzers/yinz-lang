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
    BinOpKind, Block, CallExpr, Expr, FunctionDecl, Item, MatchArm, MatchPattern, MatchPatternKind,
    Module, Param, Stmt, Type, UnaryOpKind,
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
                | Token::If
                | Token::While
                | Token::For
                | Token::Return
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

        let params = self.parse_params(&name);
        if params.is_none() {
            return None;
        }
        let params = params.unwrap();

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
            params,
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
            Token::If => Some(self.parse_if()),
            Token::While => Some(self.parse_while()),
            Token::For => Some(self.parse_for()),
            Token::Return => Some(self.parse_return()),
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


    /// Parse a parameter list `(p1: T1, p2: T2, ...)` for a function declaration.
    ///
    /// Returns `None` (fatal, recovery attempted) only if `)` cannot be found
    /// at all. Otherwise always returns `Some(params)`, which may be empty.
    fn parse_params(&mut self, fn_name: &str) -> Option<Vec<Param>> {
        if self.expect(&Token::RParen).is_some() {
            return Some(Vec::new());
        }

        let mut params: Vec<Param> = Vec::new();
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        loop {
            if matches!(self.peek(), Token::RParen | Token::Eof | Token::RBrace) {
                break;
            }

            // Ownership annotations: `share`/`lend`/`give` are M4 — skip with deferral.
            if let Token::Identifier(kw) = self.peek().clone() {
                if matches!(kw.as_str(), "share" | "lend" | "give")
                    && matches!(self.peek_ahead(1), Token::Identifier(_))
                {
                    let kw_span = self.current_span();
                    self.advance(); // skip ownership keyword
                    self.diags.push(Diagnostic::error(
                        kw_span,
                        format!("`{kw}` ownership annotations are not available yet."),
                        format!("Declare the parameter without an annotation: `name: Type`"),
                        "Yinz ownership modifiers (`share`, `lend`, `give`) land in v0.1 milestone 4. Until then, parameters are read-only.",
                    ));
                }
            }

            // Parse `name: Type`
            let param_start = self.current_span().start;
            let (param_name, name_span) = match self.peek().clone() {
                Token::Identifier(n) => {
                    let span = self.current_span();
                    self.advance();
                    (n, span)
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        format!("Expected a parameter name in `{fn_name}`'s parameter list."),
                        format!("Write `name: Type`, e.g. `function {fn_name}(x: int, y: string)`"),
                        "Each parameter needs a name and a type so the function body can use it.",
                    ));
                    // Recover to `)` or `}`
                    while !matches!(self.peek(), Token::RParen | Token::Comma | Token::RBrace | Token::Eof) {
                        self.advance();
                    }
                    let _ = self.expect(&Token::Comma);
                    continue;
                }
            };

            if self.expect(&Token::Colon).is_none() {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    format!("Expected `:` after parameter name `{param_name}`."),
                    format!("Write `{param_name}: Type`, e.g. `{param_name}: int`"),
                    "The `:` separates the parameter name from its type.",
                ));
                while !matches!(self.peek(), Token::RParen | Token::Comma | Token::RBrace | Token::Eof) {
                    self.advance();
                }
                let _ = self.expect(&Token::Comma);
                continue;
            }

            let ty_start = self.current_span().start;
            let ty = self.parse_type();
            let ty_span = SourceSpan::new(self.file, ty_start, self.tokens.get(self.pos.saturating_sub(1)).map(|s| s.span.end).unwrap_or(ty_start));
            let param_end = ty_span.end;

            // Duplicate name check
            if !seen_names.insert(param_name.clone()) {
                self.diags.push(Diagnostic::error(
                    name_span.clone(),
                    format!("Duplicate parameter name `{param_name}` in `{fn_name}`."),
                    format!("Each parameter in a function must have a unique name."),
                    "Two parameters with the same name would make it impossible to tell them apart inside the function body.",
                ));
            }

            params.push(Param {
                name: param_name,
                name_span,
                ty,
                ty_span,
                span: SourceSpan::new(self.file, param_start, param_end),
            });

            // Consume optional `,` — trailing comma is allowed
            if !matches!(self.peek(), Token::RParen) {
                if self.expect(&Token::Comma).is_none() {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected `,` or `)` after parameter.",
                        "Separate parameters with `,`: `function foo(a: int, b: string)`",
                        "Each parameter must be separated by a comma.",
                    ));
                    // Recover to `)` or next identifier (another param attempt)
                    while !matches!(self.peek(), Token::RParen | Token::RBrace | Token::Eof | Token::Identifier(_)) {
                        self.advance();
                    }
                }
            }
        }

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Missing `)` to close `{fn_name}`'s parameter list."),
                format!("Add `)` after the last parameter."),
                "Every `(` in a function declaration must be matched with a `)`.",
            ));
            self.recover_to_rbrace();
            return None;
        }

        Some(params)
    }

    /// Parse `if (cond) { body }` (simple) or `if (scrutinee) { arms }` (multi-case).
    ///
    /// Disambiguation: after consuming `if (cond) {`, peek at the first token:
    /// - `}`: empty simple if
    /// - literal or `else` followed by `=>`: multi-case
    /// - anything else: simple if (parse statements until `}`)
    fn parse_if(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.advance(); // consume `if`

        if self.expect(&Token::LParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `(` after `if`.",
                "Write `if (condition) { ... }` with the condition in parentheses.",
                "The condition of an `if` must be wrapped in parentheses.",
            ));
        }

        let cond = self.parse_expr(0);

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `)` to close the `if` condition.",
                "Write `if (condition) { ... }` — close the parentheses before the `{`.",
                "Every `(` in an `if` condition must be matched with a `)`.",
            ));
        }

        if self.expect(&Token::LBrace).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `{` to open the `if` body.",
                "Write `if (condition) { ... }` with curly braces around the body.",
                "Curly braces are always required in Yinz — `if (cond) stmt` without braces is not valid.",
            ));
            let span = SourceSpan::new(self.file, start, self.current_span().end);
            return Stmt::If { cond, body: Block { stmts: vec![], span: span.clone() }, span };
        }

        let block_open = self.tokens.get(self.pos.saturating_sub(1)).map(|s| s.span.start).unwrap_or(start);
        let _ = block_open;

        // Disambiguate: is this simple-if or multi-case-if?
        let is_multi_case = self.peek_is_match_arm_start();

        if is_multi_case {
            self.parse_match_body(cond, start)
        } else {
            let body = self.parse_block();
            let end = body.span.end;
            Stmt::If {
                cond,
                body,
                span: SourceSpan::new(self.file, start, end),
            }
        }
    }

    /// True if the current position looks like the start of a multi-case arm:
    /// a literal/`else` token followed by `=>`, OR an `is TypeName =>` triple.
    fn peek_is_match_arm_start(&self) -> bool {
        // Value arm: literal or identifier directly followed by `=>`
        let value_arm = matches!(
            self.peek(),
            Token::IntLit(_)
                | Token::NumberLit(_)
                | Token::StringLit(_)
                | Token::True
                | Token::False
                | Token::Identifier(_)
                | Token::Else
        ) && *self.peek_ahead(1) == Token::FatArrow;

        // `is Type =>` form (three tokens): `is` identifier, type-name identifier, `=>`
        let is_type_arm = matches!(self.peek(), Token::Identifier(kw) if kw == "is")
            && matches!(self.peek_ahead(1), Token::Identifier(_));

        value_arm || is_type_arm
    }

    /// Parse the body of a multi-case `if`: `arms [else_arm] }`.
    ///
    /// Called after `if (scrutinee) {` has been consumed and we've determined
    /// the block is multi-case (first token + `=>` pattern).
    fn parse_match_body(&mut self, scrutinee: Expr, start: usize) -> Stmt {
        let mut arms: Vec<MatchArm> = Vec::new();
        let mut else_arm: Option<Block> = None;

        loop {
            match self.peek() {
                Token::RBrace | Token::Eof => break,
                Token::Else => {
                    // `else => { ... }` or `else => stmt`
                    self.advance(); // consume `else`
                    let arrow_span = self.current_span();
                    if self.expect(&Token::FatArrow).is_none() {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected `=>` after `else` in multi-case `if`.",
                            "Write `else => { ... }` for the catch-all arm.",
                            "The `else` catch-all must be followed by `=>` and a block.",
                        ));
                    }
                    let body = self.parse_arm_body();
                    let _ = arrow_span;
                    else_arm = Some(body);
                    // After else_arm, only `}` is valid
                    if !matches!(self.peek(), Token::RBrace | Token::Eof) {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "The `else =>` arm must be the last arm in a multi-case `if`.",
                            "Move `else => ...` to the end of the multi-case block.",
                            "The `else` catch-all matches anything — having more arms after it would be unreachable.",
                        ));
                        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                            self.advance();
                        }
                    }
                    break;
                }
                _ if self.peek_is_match_arm_start() => {
                    arms.push(self.parse_match_arm());
                }
                _ => {
                    // Non-arm statement inside a committed multi-case block.
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        format!(
                            "Inside a multi-case `if`, every entry must be a `pattern => body` arm, but found `{}`.",
                            token_display(self.peek())
                        ),
                        "Write `value => { ... }` or `else => { ... }` for each case.",
                        "Once the first `=>` arm is seen, all entries in the block are treated as arms.",
                    ));
                    // Recover: scan to next `=>` or `}`
                    while !matches!(self.peek(), Token::FatArrow | Token::RBrace | Token::Eof) {
                        self.advance();
                    }
                    if matches!(self.peek(), Token::FatArrow) {
                        self.advance(); // consume `=>`
                        let _ = self.parse_arm_body(); // discard recovered body
                    }
                }
            }
        }

        let end = self.current_span().end;
        if self.expect(&Token::RBrace).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Missing `}` to close the multi-case `if` block.",
                "Add `}` at the end of the multi-case block.",
                "Every `{` must be matched with a `}`.",
            ));
        }

        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    /// Parse a single multi-case arm: `pattern => body`.
    fn parse_match_arm(&mut self) -> MatchArm {
        let pat_start = self.current_span().start;

        // Check for deferred M6 forms: `is TypeName =>` or bare identifier-not-followed-by-`=>`
        // `is Type =>` form
        if let Token::Identifier(kw) = self.peek().clone() {
            if kw == "is" {
                let is_span = self.current_span();
                self.advance(); // consume `is`
                let type_name = if let Token::Identifier(n) = self.peek().clone() {
                    let n2 = n.clone();
                    self.advance();
                    n2
                } else {
                    String::new()
                };
                let arrow_span = self.current_span();
                let _ = self.expect(&Token::FatArrow);
                self.diags.push(Diagnostic::error(
                    is_span,
                    format!("`is {type_name} =>` pattern matching is not available yet."),
                    "Use value matching `1 => ...` or `else => ...` for now.",
                    "Matching on a value's type in multi-case `if` arms (`is Circle => ...`) arrives in v0.1 milestone 6 when union types land.",
                ));
                let body = self.parse_arm_body();
                let pat_span = SourceSpan::new(self.file, pat_start, arrow_span.start);
                return MatchArm {
                    pattern: MatchPattern { kind: MatchPatternKind::IsType(type_name), span: pat_span },
                    body,
                    arrow_span,
                };
            }
        }

        // Value pattern: literal or identifier followed by `=>`
        let pat_span_start = self.current_span().start;
        let pattern_expr = self.parse_expr(0);
        let pat_span_end = pattern_expr.span().end;
        let pat_span = SourceSpan::new(self.file, pat_span_start, pat_span_end);

        let arrow_span = self.current_span();
        if self.expect(&Token::FatArrow).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `=>` after pattern in multi-case `if`.",
                "Write `pattern => { ... }` for each arm.",
                "The `=>` separates the pattern from the body in a multi-case arm.",
            ));
        }

        let body = self.parse_arm_body();
        MatchArm {
            pattern: MatchPattern { kind: MatchPatternKind::Value(pattern_expr), span: pat_span },
            body,
            arrow_span,
        }
    }

    /// Parse the body of a multi-case arm: either `{ stmts }` or a single statement.
    ///
    /// The spec allows both `1 => { print(x) }` and `1 => print(x)` (single stmt).
    fn parse_arm_body(&mut self) -> Block {
        if matches!(self.peek(), Token::LBrace) {
            self.advance(); // consume `{`
            let body = self.parse_block();
            // parse_block already consumed the `}`
            body
        } else {
            // Single statement (no braces)
            let start = self.current_span().start;
            if let Some(stmt) = self.parse_stmt() {
                let end = self.current_span().start;
                Block {
                    stmts: vec![stmt],
                    span: SourceSpan::new(self.file, start, end),
                }
            } else {
                let span = self.current_span();
                Block { stmts: vec![], span }
            }
        }
    }

    fn parse_while(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.advance(); // consume `while`

        if self.expect(&Token::LParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `(` after `while`.",
                "Write `while (condition) { ... }` with the condition in parentheses.",
                "The condition of a `while` loop must be wrapped in parentheses.",
            ));
        }

        let cond = self.parse_expr(0);

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `)` to close the `while` condition.",
                "Write `while (condition) { ... }` — close the parentheses before the `{`.",
                "Every `(` in a `while` condition must be matched with a `)`.",
            ));
        }

        if self.expect(&Token::LBrace).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `{` to open the `while` body.",
                "Write `while (condition) { ... }` with curly braces around the body.",
                "Curly braces are always required in Yinz — `while (cond) stmt` without braces is not valid.",
            ));
            let span = SourceSpan::new(self.file, start, self.current_span().end);
            return Stmt::While { cond, body: Block { stmts: vec![], span: span.clone() }, span };
        }

        let body = self.parse_block();
        let end = body.span.end;
        Stmt::While {
            cond,
            body,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    fn parse_for(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.advance(); // consume `for`

        if self.expect(&Token::LParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `(` after `for`.",
                "Write `for (x in collection) { ... }` with the loop variable in parentheses.",
                "The loop header of a `for` must be wrapped in parentheses.",
            ));
        }

        let (var, var_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                let span = self.current_span();
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    "Expected a loop variable name after `for (`.",
                    "Write `for (i in range(0, 10)) { ... }` with a variable name before `in`.",
                    "The loop variable holds the current item on each iteration.",
                ));
                ("_".to_string(), span)
            }
        };

        if self.expect(&Token::In).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Expected `in` after loop variable `{var}`."),
                format!("Write `for ({var} in collection) {{ ... }}`"),
                "The `in` keyword separates the loop variable from the collection being iterated.",
            ));
        }

        let iter = self.parse_expr(0);

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `)` to close the `for` loop header.",
                "Write `for (x in collection) { ... }` — close the parentheses before the `{`.",
                "Every `(` in a `for` header must be matched with a `)`.",
            ));
        }

        if self.expect(&Token::LBrace).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `{` to open the `for` body.",
                "Write `for (x in collection) { ... }` with curly braces around the body.",
                "Curly braces are always required in Yinz.",
            ));
            let span = SourceSpan::new(self.file, start, self.current_span().end);
            return Stmt::For { var, var_span, iter, body: Block { stmts: vec![], span: span.clone() }, span };
        }

        let body = self.parse_block();
        let end = body.span.end;
        Stmt::For {
            var,
            var_span,
            iter,
            body,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    fn parse_return(&mut self) -> Stmt {
        let start = self.current_span().start;
        self.advance(); // consume `return`

        // `return` alone (nothing follows that's an expression start) → return None
        let value = if self.is_stmt_boundary() || matches!(self.peek(), Token::Eof) {
            None
        } else {
            Some(self.parse_expr(0))
        };

        let end = value.as_ref().map(|e| e.span().end).unwrap_or(start + 6); // len("return")
        Stmt::Return {
            value,
            span: SourceSpan::new(self.file, start, end),
        }
    }

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
        Token::If => "if",
        Token::Else => "else",
        Token::While => "while",
        Token::For => "for",
        Token::In => "in",
        Token::Return => "return",
        Token::FatArrow => "=>",
    }
}
