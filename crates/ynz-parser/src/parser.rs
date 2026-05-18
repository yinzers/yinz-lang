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
    BinOpKind, Block, CallExpr, ConstDecl, ContractSig, Expr, FieldDecl, FunctionDecl,
    GenericParam, ImportDecl, ImportItem, ImportKind, Item, MatchArm, MatchPattern,
    MatchPatternKind, Module, OptionsDecl, OwnershipModifier, Param, PostfixOpKind, ReExport,
    ReExportItem, ReceiverKind, ShapeDecl, Stmt, StructLitField, Type, TypePath, UnaryOpKind,
};
// TypePath is also used for Expr::Is
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
                | Token::Shape
                | Token::Base
                | Token::Options
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
                Token::Import => {
                    if let Some(decl) = self.parse_import_decl() {
                        items.push(Item::ImportDecl(decl));
                    }
                }
                Token::Export => {
                    self.parse_export_item(&mut items);
                }
                Token::Function => {
                    if let Some(decl) = self.parse_function_decl() {
                        items.push(Item::Function(decl));
                    }
                }
                Token::Shape => {
                    if let Some(decl) = self.parse_shape_decl(false) {
                        items.push(Item::ShapeDecl(decl));
                    }
                }
                Token::Options => {
                    if let Some(decl) = self.parse_options_decl() {
                        items.push(Item::OptionsDecl(decl));
                    }
                }
                Token::Base => {
                    // `base shape Name { ... }` — consume `base`, then expect `shape`
                    let base_span = self.current_span();
                    self.advance(); // consume `base`
                    if !matches!(self.peek(), Token::Shape) {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected `shape` after `base`.",
                            "Write `base shape Name { ... }` to declare a shape that cannot be instantiated directly.",
                            "`base` is a modifier on `shape` — it must be immediately followed by the `shape` keyword.",
                        ));
                        let _ = base_span;
                        self.advance();
                    } else if let Some(decl) = self.parse_shape_decl(true) {
                        items.push(Item::ShapeDecl(decl));
                    }
                }
                Token::DocComment { .. } => {
                    // Doc-comment tokens are attached to items by P3. For now, skip them.
                    self.advance();
                }
                _ => {
                    let span = self.current_span();
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!(
                            "Unexpected `{}` at the top level.",
                            token_display(self.peek())
                        ),
                        "Top-level code must be inside a `function`, `shape`, `import`, or `export` declaration.",
                        "Yinz programs are made of functions and shapes. Only declarations are allowed at the top level.",
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

    /// Parse `export function/shape/options/const/base/{ ... }` after `export` is consumed.
    fn parse_export_item(&mut self, items: &mut Vec<Item>) {
        let export_start = self.current_span();
        self.advance(); // consume `export`

        match self.peek().clone() {
            Token::Function => {
                if let Some(mut decl) = self.parse_function_decl() {
                    decl.is_exported = true;
                    items.push(Item::Function(decl));
                }
            }
            Token::Shape => {
                if let Some(mut decl) = self.parse_shape_decl(false) {
                    decl.is_exported = true;
                    items.push(Item::ShapeDecl(decl));
                }
            }
            Token::Options => {
                if let Some(mut decl) = self.parse_options_decl() {
                    decl.is_exported = true;
                    items.push(Item::OptionsDecl(decl));
                }
            }
            Token::Base => {
                self.advance(); // consume `base`
                if !matches!(self.peek(), Token::Shape) {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected `shape` after `export base`.",
                        "Write `export base shape Name { ... }`.",
                        "`base` must be immediately followed by the `shape` keyword.",
                    ));
                    self.advance();
                } else if let Some(mut decl) = self.parse_shape_decl(true) {
                    decl.is_exported = true;
                    items.push(Item::ShapeDecl(decl));
                }
            }
            Token::Const => {
                if let Some(mut decl) = self.parse_const_decl() {
                    decl.is_exported = true;
                    items.push(Item::ConstDecl(decl));
                }
            }
            Token::LBrace => {
                // Re-export: `export { foo, bar } from "path"`
                if let Some(re) = self.parse_reexport(export_start.start) {
                    items.push(Item::ReExport(re));
                }
            }
            Token::Identifier(kw) if kw == "default" => {
                let span = self.current_span();
                self.advance(); // consume `default`
                self.diags.push(Diagnostic::error(
                    span,
                    "Yinz does not have default exports.",
                    "Name the export instead: `export function foo()` or `export const PI = 3.14`.",
                    "Yinz uses named exports only — every exported item is referred to by its name in `import { name }` statements.",
                ));
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    format!("Expected a declaration after `export`, not `{}`.", token_display(self.peek())),
                    "Write `export function foo()`, `export shape Foo { ... }`, `export options Status { ... }`, or `export { name } from \"path\"`.",
                    "`export` makes a declaration visible to other modules.",
                ));
                self.advance();
                let _ = export_start;
            }
        }
    }

    /// Parse `import { name1, name2 as alias } from "path"` or `import ns from "path"`.
    fn parse_import_decl(&mut self) -> Option<ImportDecl> {
        let start = self.current_span().start;
        self.advance(); // consume `import`

        // Banned form: `import * as X from "..."` or `import *`
        if matches!(self.peek(), Token::Star) {
            let span = self.current_span();
            self.advance();
            // Try to consume the rest for error recovery
            while !matches!(self.peek(), Token::Eof | Token::Function | Token::Shape) {
                self.advance();
            }
            self.diags.push(Diagnostic::error(
                span,
                "Wildcard imports are not allowed in Yinz.",
                "Use a namespace import instead: `import utils from \"utils\"`.",
                "Wildcard imports make it unclear where names come from. Namespace imports (`import ns from \"path\"`) give the same access with a clear prefix.",
            ));
            return None;
        }

        // Named import: `import { name1, name2 as alias } from "..."`
        if matches!(self.peek(), Token::LBrace) {
            return self.parse_named_import(start);
        }

        // Namespace import: `import ns from "..."` or `import ns as alias from "..."`
        let (local_name, local_name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a name or `{` after `import`.",
                    "Write `import utils from \"utils\"` or `import { fetchUser } from \"services/users\"`.",
                    "`import` brings names from another module into scope.",
                ));
                return None;
            }
        };

        // Check for banned `import "path"` (side-effect import — must bind)
        if matches!(self.peek(), Token::BacktickString(_)) {
            let span = self.current_span();
            self.diags.push(Diagnostic::error(
                span,
                "Import statements must bind a name.",
                "Write `import utils from \"utils\"` to bind the module as a namespace.",
                "Yinz does not support side-effect-only imports. Every import must name what it brings into scope.",
            ));
            self.advance();
            return None;
        }

        // `as alias` (optional)
        let final_local_name = if matches!(self.peek(), Token::Identifier(n) if n == "as") {
            self.advance(); // consume `as`
            match self.peek().clone() {
                Token::Identifier(alias) => {
                    let alias_span = self.current_span();
                    self.advance();
                    ImportItem {
                        exported_name: local_name.clone(),
                        exported_name_span: local_name_span.clone(),
                        local_name: alias,
                        local_name_span: alias_span,
                    }
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected an alias name after `as`.",
                        "Write `import ns as alias from \"path\"`.",
                        "`as` renames the namespace import to a different local name.",
                    ));
                    return None;
                }
            }
        } else {
            // No alias — namespace name is the local name
            ImportItem {
                exported_name: local_name.clone(),
                exported_name_span: local_name_span.clone(),
                local_name: local_name.clone(),
                local_name_span: local_name_span.clone(),
            }
        };

        // Expect `from`
        if !matches!(self.peek(), Token::Identifier(n) if n == "from") {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `from` after the import name.",
                "Write `import utils from \"services/utils\"`.",
                "`from` specifies which module to import from.",
            ));
            return None;
        }
        self.advance(); // consume `from`

        let (source, source_span) = self.parse_import_path()?;

        let end = self.current_span().end;
        Some(ImportDecl {
            kind: ImportKind::Namespace {
                local_name: final_local_name.local_name,
                local_name_span: final_local_name.local_name_span,
            },
            source,
            source_span,
            span: SourceSpan::new(self.file, start, end),
        })
    }

    /// Parse `{ name1, name2 as alias } from "path"` — called after `import` consumed.
    fn parse_named_import(&mut self, start: usize) -> Option<ImportDecl> {
        self.advance(); // consume `{`
        let mut import_items = Vec::new();

        loop {
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            // Check for wildcard (banned)
            if matches!(self.peek(), Token::Star) {
                let span = self.current_span();
                self.advance();
                self.diags.push(Diagnostic::error(
                    span,
                    "Wildcard imports inside `{` are not allowed.",
                    "List names explicitly: `import { fetchUser, createUser } from \"path\"`.",
                    "Wildcard imports make it unclear where names come from. List what you need.",
                ));
                // Recover to `}`
                while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                    self.advance();
                }
                break;
            }

            let (exported_name, exported_name_span) = match self.peek().clone() {
                Token::Identifier(n) => {
                    let span = self.current_span();
                    self.advance();
                    (n, span)
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected a name inside `import { ... }`.",
                        "Write `import { fetchUser, createUser } from \"path\"`.",
                        "Named imports list the specific names to bring into scope.",
                    ));
                    while !matches!(self.peek(), Token::RBrace | Token::Comma | Token::Eof) {
                        self.advance();
                    }
                    if matches!(self.peek(), Token::Comma) {
                        self.advance();
                    }
                    continue;
                }
            };

            // Optional `as alias`
            let (local_name, local_name_span) = if matches!(self.peek(), Token::Identifier(n) if n == "as")
            {
                self.advance(); // consume `as`
                match self.peek().clone() {
                    Token::Identifier(alias) => {
                        let span = self.current_span();
                        self.advance();
                        (alias, span)
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected an alias name after `as`.",
                            "Write `import { fetchUser as getUser } from \"path\"`.",
                            "`as` renames the imported name to a different local name.",
                        ));
                        (exported_name.clone(), exported_name_span.clone())
                    }
                }
            } else {
                (exported_name.clone(), exported_name_span.clone())
            };

            import_items.push(ImportItem {
                exported_name,
                exported_name_span,
                local_name,
                local_name_span,
            });

            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(&Token::RBrace);

        // Expect `from`
        if !matches!(self.peek(), Token::Identifier(n) if n == "from") {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `from` after `import { ... }`.",
                "Write `import { fetchUser } from \"services/users\"`.",
                "`from` specifies which module to import from.",
            ));
            return None;
        }
        self.advance(); // consume `from`

        let (source, source_span) = self.parse_import_path()?;

        let end = self.current_span().end;
        Some(ImportDecl {
            kind: ImportKind::Named(import_items),
            source,
            source_span,
            span: SourceSpan::new(self.file, start, end),
        })
    }

    /// Parse the path string after `from`. Validates: no relative paths, backtick form.
    fn parse_import_path(&mut self) -> Option<(String, SourceSpan)> {
        // Yinz import paths use backtick strings: `import { foo } from `services/users``
        // Accept either backtick or a simple identifier-like string.
        match self.peek().clone() {
            Token::BacktickString(bytes) => {
                let span = self.current_span();
                self.advance();
                let path = String::from_utf8_lossy(&bytes).to_string();
                // Reject relative paths
                if path.starts_with("./") || path.starts_with("../") {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "Import paths must be project-root relative, not relative paths.",
                        format!(
                            "Write `{}` instead of `{}`.",
                            path.trim_start_matches("./").trim_start_matches("../"),
                            path
                        ),
                        "Yinz uses project-root paths so moving a file never breaks imports.",
                    ));
                    return None;
                }
                Some((path, span))
            }
            Token::StringLit(bytes) => {
                let span = self.current_span();
                self.advance();
                let path = String::from_utf8_lossy(&bytes).to_string();
                if path.starts_with("./") || path.starts_with("../") {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "Import paths must be project-root relative, not relative paths.",
                        format!(
                            "Write `{}` instead of `{}`.",
                            path.trim_start_matches("./").trim_start_matches("../"),
                            path
                        ),
                        "Yinz uses project-root paths so moving a file never breaks imports.",
                    ));
                    return None;
                }
                Some((path, span))
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a path string after `from`.",
                    "Write the module path: `import { foo } from `services/users``.",
                    "The path is the project-root-relative location of the module file (without `.ynz`).",
                ));
                None
            }
        }
    }

    /// Parse `export { foo, bar as baz } from "path"`.
    fn parse_reexport(&mut self, start: usize) -> Option<ReExport> {
        self.advance(); // consume `{`
        let mut re_items = Vec::new();

        loop {
            if matches!(self.peek(), Token::RBrace | Token::Eof) {
                break;
            }
            let (name, name_span) = match self.peek().clone() {
                Token::Identifier(n) => {
                    let span = self.current_span();
                    self.advance();
                    (n, span)
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected a name in `export { ... }`.",
                        "Write `export { fetchUser, createUser } from \"services/users\"`.",
                        "Re-exports list the names to expose from a source module.",
                    ));
                    while !matches!(self.peek(), Token::RBrace | Token::Comma | Token::Eof) {
                        self.advance();
                    }
                    if matches!(self.peek(), Token::Comma) {
                        self.advance();
                    }
                    continue;
                }
            };
            let alias = if matches!(self.peek(), Token::Identifier(n) if n == "as") {
                self.advance();
                match self.peek().clone() {
                    Token::Identifier(a) => {
                        let s = self.current_span();
                        self.advance();
                        Some((a, s))
                    }
                    _ => None,
                }
            } else {
                None
            };
            re_items.push(ReExportItem {
                name,
                name_span,
                alias,
            });
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        self.expect(&Token::RBrace);

        if !matches!(self.peek(), Token::Identifier(n) if n == "from") {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `from` after `export { ... }`.",
                "Write `export { fetchUser } from \"services/users\"`.",
                "`from` specifies which module to re-export names from.",
            ));
            return None;
        }
        self.advance(); // consume `from`

        let (source, source_span) = self.parse_import_path()?;
        let end = self.current_span().end;
        Some(ReExport {
            items: re_items,
            source,
            source_span,
            span: SourceSpan::new(self.file, start, end),
        })
    }

    /// Parse `const NAME [: Type] = value` (top-level, no `let`).
    fn parse_const_decl(&mut self) -> Option<ConstDecl> {
        let start = self.current_span().start;
        self.advance(); // consume `const`

        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a constant name after `const`.",
                    "Write `const MAX_HEALTH = 100` or `export const MAX_HEALTH = 100`.",
                    "Top-level constants must have a name.",
                ));
                return None;
            }
        };

        let ty = if matches!(self.peek(), Token::Colon) {
            self.advance(); // consume `:`
            Some(self.parse_type())
        } else {
            None
        };

        if self.expect(&Token::Eq).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `=` after the constant name.",
                "Write `const MAX_HEALTH = 100`.",
                "Constants must have a value assigned with `=`.",
            ));
            return None;
        }

        let value = self.parse_expr(0);
        let end = self.current_span().end;
        Some(ConstDecl {
            name,
            name_span,
            ty,
            value,
            is_exported: false, // set to true by parse_export_item
            span: SourceSpan::new(self.file, start, end),
        })
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

        // Optional `<T, U>` type-param list
        let generics = self.parse_generic_params();

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

        let params = self.parse_params(&name)?;

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

        let return_type_start = self.current_span().start;
        let inner_ty = self.parse_type();

        // M7: `-> T errors` — the `errors` keyword after the return type marks
        // this function as fallible. Wrap the inner type in Type::ErrorCapable.
        let (return_type, errors_capable) = if matches!(self.peek(), Token::Errors) {
            let errors_tok_span = self.current_span();
            self.advance(); // consume `errors`
            let full_span = SourceSpan::new(self.file, return_type_start, errors_tok_span.end);
            (
                Type::ErrorCapable {
                    inner: Box::new(inner_ty),
                    span: full_span,
                },
                true,
            )
        } else {
            (inner_ty, false)
        };

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
            generics,
            params,
            return_type,
            body,
            span: SourceSpan::new(self.file, start_span.start, end.end),
            name_span,
            errors_capable,
            is_exported: false, // set to true by parse_module when `export` precedes
        })
    }

    fn parse_type(&mut self) -> Type {
        let start = self.current_span().start;
        let first = self.parse_type_with_depth(0);

        // `|` in type position = union type (not bitwise-OR which is expr-position only).
        // Collect remaining variants until no more `|`.
        if !matches!(self.peek(), Token::Pipe) {
            return first;
        }
        let mut variants = vec![first];
        while matches!(self.peek(), Token::Pipe) {
            self.advance(); // consume `|`
            let next = self.parse_type_with_depth(0);
            variants.push(next);
        }
        let end = self.current_span().start;
        Type::Union {
            variants,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    fn parse_type_with_depth(&mut self, depth: u8) -> Type {
        if depth > 16 {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Generic type nesting exceeds 16 levels.",
                "Simplify the type — for example, break the cycle with `maybe<Node<T>>` instead of nesting further.",
                "Deeply nested generic types are almost always caused by an accidental cycle. 16 levels is far more than any real type should need.",
            ));
            return Type::Error;
        }

        match self.peek().clone() {
            Token::Nothing => {
                self.advance();
                Type::Nothing
            }
            Token::Dynamic => {
                self.advance(); // consume `dynamic`
                let span = self.current_span();
                match self.peek().clone() {
                    Token::Identifier(contract) => {
                        let contract_span = span;
                        self.advance();
                        Type::Dynamic {
                            contract,
                            span: contract_span,
                        }
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected a contract name after `dynamic`.",
                            "Write `dynamic Foo` where `Foo` is a shape that declares contract signatures.",
                            "`dynamic` enables runtime dispatch — it must name the contract the value follows.",
                        ));
                        Type::Error
                    }
                }
            }
            Token::SelfType => {
                let span = self.current_span();
                self.advance();
                Type::SelfType { span }
            }
            Token::Identifier(name) => {
                let span = self.current_span();
                self.advance();
                match name.as_str() {
                    "int" => Type::Int,
                    "float" => Type::Float,
                    "bool" => Type::Bool,
                    "number" => self.parse_number_type(),
                    "maybe" => self.parse_maybe_type(span, depth),
                    _ => {
                        if matches!(self.peek(), Token::Lt) {
                            self.parse_generic_type(name, span, depth)
                        } else {
                            Type::Named(name, span)
                        }
                    }
                }
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a type here.",
                    "Use `nothing`, `int`, `float`, `number`, `bool`, `maybe<T>`, `array<T>`, `dynamic Foo`, `Self`, or a type name.",
                    "Types tell Yinz what kind of value a variable holds or a function produces.",
                ));
                Type::Error
            }
        }
    }

    /// Parse `maybe<T>` — called after `maybe` identifier has been consumed.
    fn parse_maybe_type(&mut self, maybe_start_span: SourceSpan, depth: u8) -> Type {
        if self.expect(&Token::Lt).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "`maybe` requires a type argument.",
                "Write `maybe<T>` — for example, `maybe<int>` or `maybe<Player>`.",
                "`maybe<T>` is the Yinz optional type. It must have exactly one type argument.",
            ));
            return Type::Error;
        }

        let inner = self.parse_type_with_depth(depth + 1);

        if self.expect(&Token::Gt).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Missing `>` to close `maybe<T>`.",
                "Write `maybe<T>` — close the angle bracket after the type.",
                "Every `<` in a `maybe<T>` must be matched with a `>`.",
            ));
            return Type::Error;
        }

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(maybe_start_span.start);
        Type::Maybe {
            inner: Box::new(inner),
            span: SourceSpan::new(self.file, maybe_start_span.start, end),
        }
    }

    /// Parse a generic type instantiation `Name<T, U, ...>` — called after the name
    /// has been consumed and `<` is the current token.
    fn parse_generic_type(&mut self, name: String, name_span: SourceSpan, depth: u8) -> Type {
        self.advance(); // consume `<`

        let mut args = Vec::new();

        loop {
            match self.peek() {
                Token::Gt | Token::Eof => break,
                Token::Comma => {
                    self.advance();
                }
                _ => {
                    args.push(self.parse_type_with_depth(depth + 1));
                    if !matches!(self.peek(), Token::Gt | Token::Comma) {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected `,` or `>` in generic type argument list.",
                            "Separate type arguments with `,`: `Pair<int, string>`. Close with `>`.",
                            "Generic type arguments are comma-separated and the list ends with `>`.",
                        ));
                        // Recover to `>` or something sane
                        while !matches!(
                            self.peek(),
                            Token::Gt | Token::RBrace | Token::Eof | Token::LParen
                        ) {
                            self.advance();
                        }
                        break;
                    }
                }
            }
        }

        if self.expect(&Token::Gt).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Missing `>` to close `{name}<...>`.",),
                format!("Write `{name}<T>` — close the angle bracket after the type arguments."),
                "Every `<` in a generic type must be matched with a `>`.",
            ));
            return Type::Error;
        }

        let type_start = name_span.start;
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(type_start);
        Type::Generic {
            name,
            name_span,
            args,
            span: SourceSpan::new(self.file, type_start, end),
        }
    }

    /// Parse `number`, `number<N>`, or legacy `number[N]` (migration redirect).
    ///
    /// Called after `number` has already been consumed.
    /// - Bare `number` → 34-digit precision (default).
    /// - `number<N>` → validated precision N ∈ 1..=4096; non-34 deferred until M8 bignum.
    /// - `number[N]` → three-part migration diagnostic; returns `Type::Error`.
    fn parse_number_type(&mut self) -> Type {
        match self.peek() {
            Token::LBracket => {
                // Legacy number[N] syntax — teach the migration.
                let bracket_span = self.current_span();
                self.advance(); // consume `[`
                                // Consume the body up to `]` for error recovery.
                while !matches!(self.peek(), Token::RBracket | Token::RBrace | Token::Eof) {
                    self.advance();
                }
                let _ = self.expect(&Token::RBracket);
                self.diags.push(Diagnostic::error(
                    bracket_span,
                    "Use angle brackets for `number<N>`, not square brackets.",
                    "Write `number<100>` instead of `number[100]`.",
                    "M5 unified all generic syntax — array<T>, map<K,V>, number<N> — on `<>`. Square brackets are now index access only.",
                ));
                Type::Error
            }
            Token::Lt => {
                self.advance(); // consume `<`

                let precision = match self.peek().clone() {
                    Token::IntLit(n) if n > 0 && n <= 4096 => {
                        let p = n as u32;
                        self.advance();
                        p
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected a precision value between 1 and 4096 inside `number<N>`.",
                            "Write `number<34>` for the default 34-digit decimal.",
                            "The number in `number<N>` sets how many decimal digits of precision you get.",
                        ));
                        // Consume to `>` to recover.
                        while !matches!(self.peek(), Token::Gt | Token::RBrace | Token::Eof) {
                            self.advance();
                        }
                        let _ = self.expect(&Token::Gt);
                        return Type::Error;
                    }
                };

                if self.expect(&Token::Gt).is_none() {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected `>` to close `number<N>`.",
                        "Write `number<34>` — close the angle bracket after the precision.",
                        "The angle bracket must be closed to complete the type.",
                    ));
                    return Type::Error;
                }

                if precision != 34 {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        format!("`number<{precision}>` is not available yet."),
                        "Use `number` (34 decimal digits) for now.",
                        "`number<N>` for N other than 34 (bignum support) arrives in M8.",
                    ));
                    return Type::Error;
                }

                Type::Number { precision: 34 }
            }
            _ => Type::Number { precision: 34 },
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
        match self.peek() {
            Token::Let | Token::Const => Some(self.parse_let_or_const()),
            Token::Identifier(_) if *self.peek_ahead(1) == Token::Eq => Some(self.parse_assign()),
            Token::If => Some(self.parse_if()),
            Token::While => Some(self.parse_while()),
            Token::For => Some(self.parse_for()),
            Token::Return => Some(self.parse_return()),
            _ => {
                // Parse the expression; if it resolves to a FieldAccess followed by
                // `=`, it becomes a FieldAssign. Otherwise it's a bare expression stmt.
                let expr = self.parse_expr(0);
                if matches!(self.peek(), Token::Eq) {
                    if matches!(expr, Expr::FieldAccess { .. }) {
                        let eq_span = self.current_span();
                        self.advance(); // consume `=`
                        let value = self.parse_expr(0);
                        let span = SourceSpan::new(self.file, expr.span().start, value.span().end);
                        let _ = eq_span;
                        return Some(Stmt::FieldAssign {
                            target: Box::new(expr),
                            value,
                            span,
                        });
                    }
                    if let Expr::IndexAccess {
                        receiver, index, ..
                    } = expr
                    {
                        self.advance(); // consume `=`
                        let value = self.parse_expr(0);
                        let span =
                            SourceSpan::new(self.file, receiver.span().start, value.span().end);
                        return Some(Stmt::IndexAssign {
                            receiver,
                            index,
                            value,
                            span,
                        });
                    }
                    // Something else followed by `=` — error
                    let span = self.current_span();
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "The left side of `=` must be a variable name, a field access, or an index expression.",
                        "Write `name = value`, `value.field = newValue`, or `arr[i] = newValue`.",
                        "Only named locations (variables, fields, and indexed positions) can be assigned to.",
                    ));
                    self.advance(); // consume `=`
                    let _ = self.parse_expr(0); // consume RHS for recovery
                    return Some(Stmt::Expr(Expr::Error(span)));
                }
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

            // Optional ownership modifier: `share`, `lend`, or `give` before the param name.
            // Also handles `share self`, `lend self`, `give self` receiver parameters.
            let ownership = if let Token::Identifier(kw) = self.peek().clone() {
                match kw.as_str() {
                    "share" => {
                        self.advance();
                        Some(OwnershipModifier::Share)
                    }
                    "lend" => {
                        self.advance();
                        Some(OwnershipModifier::Lend)
                    }
                    "give" => {
                        self.advance();
                        Some(OwnershipModifier::Give)
                    }
                    _ => None,
                }
            } else {
                None
            };

            // Parse `name: Type` — `self` (SelfValue token) is valid as the receiver name.
            let param_start = self.current_span().start;
            let (param_name, name_span) = match self.peek().clone() {
                Token::Identifier(n) => {
                    let span = self.current_span();
                    self.advance();
                    (n, span)
                }
                Token::SelfValue => {
                    let span = self.current_span();
                    self.advance();
                    ("self".to_string(), span)
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        format!("Expected a parameter name in `{fn_name}`'s parameter list."),
                        format!("Write `name: Type`, e.g. `function {fn_name}(x: int, y: string)`"),
                        "Each parameter needs a name and a type so the function body can use it.",
                    ));
                    // Recover to `)` or `}`
                    while !matches!(
                        self.peek(),
                        Token::RParen | Token::Comma | Token::RBrace | Token::Eof
                    ) {
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
                while !matches!(
                    self.peek(),
                    Token::RParen | Token::Comma | Token::RBrace | Token::Eof
                ) {
                    self.advance();
                }
                let _ = self.expect(&Token::Comma);
                continue;
            }

            let ty_start = self.current_span().start;
            let ty = self.parse_type();
            let ty_span = SourceSpan::new(
                self.file,
                ty_start,
                self.tokens
                    .get(self.pos.saturating_sub(1))
                    .map(|s| s.span.end)
                    .unwrap_or(ty_start),
            );
            let param_end = ty_span.end;

            // Duplicate name check
            if !seen_names.insert(param_name.clone()) {
                self.diags.push(Diagnostic::error(
                    name_span.clone(),
                    format!("Duplicate parameter name `{param_name}` in `{fn_name}`."),
                    "Each parameter in a function must have a unique name.",
                    "Two parameters with the same name would make it impossible to tell them apart inside the function body.",
                ));
            }

            params.push(Param {
                name: param_name,
                name_span,
                ownership,
                ty,
                ty_span,
                span: SourceSpan::new(self.file, param_start, param_end),
            });

            // Consume optional `,` — trailing comma is allowed
            if !matches!(self.peek(), Token::RParen) && self.expect(&Token::Comma).is_none() {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected `,` or `)` after parameter.",
                    "Separate parameters with `,`: `function foo(a: int, b: string)`",
                    "Each parameter must be separated by a comma.",
                ));
                // Recover to `)` or next identifier (another param attempt)
                while !matches!(
                    self.peek(),
                    Token::RParen | Token::RBrace | Token::Eof | Token::Identifier(_)
                ) {
                    self.advance();
                }
            }
        }

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Missing `)` to close `{fn_name}`'s parameter list."),
                "Add `)` after the last parameter.",
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

        let mut cond = self.parse_expr(0);

        // M6: `if (x is TypeName)` condition form — wrap as Expr::Is after condition parse.
        if matches!(self.peek(), Token::Is) {
            let is_start = cond.span().start;
            self.advance(); // consume `is`
            let ty_span = self.current_span();
            let ty_name = if let Token::Identifier(n) = self.peek().clone() {
                let n2 = n.clone();
                self.advance();
                n2
            } else {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a type name after `is`.",
                    "Write `if (x is Circle)` where `Circle` is a union variant type.",
                    "`is` checks which variant of a union type a value is at runtime.",
                ));
                String::new()
            };
            let is_end = self.current_span().start;
            cond = Expr::Is {
                expr: Box::new(cond),
                ty: TypePath {
                    name: ty_name,
                    span: ty_span,
                },
                span: SourceSpan::new(self.file, is_start, is_end),
            };
        }

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
            return Stmt::If {
                cond,
                body: Block {
                    stmts: vec![],
                    span: span.clone(),
                },
                span,
            };
        }

        let block_open = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.start)
            .unwrap_or(start);
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
                // test-ratchet: M7 P1 adds BacktickString as a valid match arm pattern
                | Token::BacktickString(_)
                | Token::True
                | Token::False
                | Token::Identifier(_)
                | Token::Else
        ) && *self.peek_ahead(1) == Token::FatArrow;

        // `is Type =>` form: Token::Is followed by a type-name identifier (M6 keyword)
        let is_type_arm =
            matches!(self.peek(), Token::Is) && matches!(self.peek_ahead(1), Token::Identifier(_));

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
                            "Write `else => { ... }` for the default arm.",
                            "The `else` default arm must be followed by `=>` and a block.",
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
                            "The `else` default arm matches anything — having more arms after it would be unreachable.",
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

        // Check for the `is TypeName =>` arm form (M6 — Token::Is is now a real keyword).
        // P2 replaces this deferral with a real implementation; for P1 it preserves the M3 diagnostic.
        // M6 P3b: `is TypeName =>` arm form — full implementation.
        // No longer deferred; parser produces Is(TypePath) and typeck validates.
        if matches!(self.peek(), Token::Is) {
            let is_span = self.current_span();
            self.advance(); // consume `is`
            let type_name = if let Token::Identifier(n) = self.peek().clone() {
                let n2 = n.clone();
                self.advance();
                n2
            } else {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a type name after `is` in multi-case arm.",
                    "Write `is Circle =>` where `Circle` is a union variant type.",
                    "`is` in a multi-case arm checks which union variant a value is.",
                ));
                String::new()
            };
            let arrow_span = self.current_span();
            let _ = self.expect(&Token::FatArrow);
            let body = self.parse_arm_body();
            let pat_span = SourceSpan::new(self.file, pat_start, arrow_span.start);
            return MatchArm {
                pattern: MatchPattern {
                    kind: MatchPatternKind::Is(TypePath {
                        name: type_name,
                        span: is_span,
                    }),
                    span: pat_span,
                },
                body,
                arrow_span,
            };
        }

        // Options-variant arm: a bare identifier directly followed by `=>`.
        // Typeck disambiguates at resolution time (options variant vs value expression).
        // `is TypeName =>` arms are handled above; this catches `variantName =>`.
        if let Token::Identifier(v) = self.peek().clone() {
            if *self.peek_ahead(1) == Token::FatArrow {
                let v_span = self.current_span();
                let v_name = v.clone();
                self.advance(); // consume identifier
                let arrow_span = self.current_span();
                let _ = self.expect(&Token::FatArrow);
                let body = self.parse_arm_body();
                let _pat_span = SourceSpan::new(self.file, pat_start, arrow_span.start);
                return MatchArm {
                    pattern: MatchPattern {
                        kind: MatchPatternKind::OptionName(v_name),
                        span: SourceSpan::new(self.file, v_span.start, v_span.end),
                    },
                    body,
                    arrow_span,
                };
            }
        }

        // Value pattern: literal expression followed by `=>`
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
            pattern: MatchPattern {
                kind: MatchPatternKind::Value(pattern_expr),
                span: pat_span,
            },
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
                Block {
                    stmts: vec![],
                    span,
                }
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
            return Stmt::While {
                cond,
                body: Block {
                    stmts: vec![],
                    span: span.clone(),
                },
                span,
            };
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

        // M7: `for ((k, v) in m)` tuple-destructure form for map iteration.
        // Detect `(` immediately after the outer `(` — that's the destructure pair.
        if matches!(self.peek(), Token::LParen) {
            return self.parse_for_destructure(start);
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
            return Stmt::For {
                var,
                var_span,
                iter,
                body: Block {
                    stmts: vec![],
                    span: span.clone(),
                },
                span,
            };
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

    /// Parse the `for ((k, v) in m) { body }` tuple-destructure form.
    ///
    /// Called after `for (` has been consumed and `(` is the current token (the
    /// inner pair). Desugars to:
    ///   `for (entry in m) { let k = entry.key; let v = entry.value; [body] }`
    ///
    /// The synthetic `entry` binding is exposed to the user's body stmts; k and v
    /// are introduced as `let` bindings at the top of the desugared body.
    fn parse_for_destructure(&mut self, start: usize) -> Stmt {
        self.advance(); // consume inner `(`

        // Parse `k`
        let (key_name, key_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let s = self.current_span();
                self.advance();
                (n, s)
            }
            _ => {
                let s = self.current_span();
                self.diags.push(Diagnostic::error(
                    s.clone(),
                    "Expected the key variable name in `for ((k, v) in m)`.",
                    "Write `for ((key, value) in m)` with two names separated by `,`.",
                    "The first name receives the map key on each iteration.",
                ));
                ("_k".to_string(), s)
            }
        };

        if self.expect(&Token::Comma).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `,` between names in `for ((k, v) in m)`.",
                "Write `for ((key, value) in m)` — separate the two variable names with a comma.",
                "The tuple-destructure form needs exactly two variable names.",
            ));
        }

        // Parse `v`
        let (val_name, val_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let s = self.current_span();
                self.advance();
                (n, s)
            }
            _ => {
                let s = self.current_span();
                self.diags.push(Diagnostic::error(
                    s.clone(),
                    "Expected the value variable name in `for ((k, v) in m)`.",
                    "Write `for ((key, value) in m)` with two names separated by `,`.",
                    "The second name receives the map value on each iteration.",
                ));
                ("_v".to_string(), s)
            }
        };

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `)` to close the destructure pair in `for ((k, v) in m)`.",
                "Write `for ((key, value) in m)` — close the inner `(` with `)`.",
                "The inner pair needs a closing `)`.",
            ));
        }

        if self.expect(&Token::In).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `in` after `(k, v)` in map for-loop.",
                "Write `for ((key, value) in m)` — put the collection after `in`.",
                "The `in` keyword separates the destructure pattern from the collection.",
            ));
        }

        let iter = self.parse_expr(0);

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `)` to close the `for` loop header.",
                "Write `for ((k, v) in m) { ... }` — close the outer `(` with `)`.",
                "Every `(` in a `for` header must be matched with a `)`.",
            ));
        }

        if self.expect(&Token::LBrace).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Expected `{` to open the `for` body.",
                "Write `for ((k, v) in m) { ... }` with curly braces around the body.",
                "Curly braces are always required in Yinz.",
            ));
            let span = SourceSpan::new(self.file, start, self.current_span().end);
            // Synthetic entry binding for error recovery
            let entry_span = key_span.clone();
            return Stmt::For {
                var: "__entry".to_string(),
                var_span: entry_span.clone(),
                iter,
                body: Block {
                    stmts: vec![],
                    span: span.clone(),
                },
                span,
            };
        }

        let user_body = self.parse_block();
        let body_end = user_body.span.end;
        let body_span = user_body.span.clone();

        // Desugar: inject `let k = __entry.key` and `let v = __entry.value` at the
        // top of the body, then append the user's statements.
        let entry_span = key_span.clone();
        let key_binding = Stmt::Let {
            is_const: false,
            name: key_name.clone(),
            name_span: key_span.clone(),
            ty: None,
            value: Expr::FieldAccess {
                receiver: Box::new(Expr::Ident("__entry".to_string(), entry_span.clone())),
                field: "key".to_string(),
                field_span: entry_span.clone(),
                span: entry_span.clone(),
            },
            span: key_span.clone(),
        };
        let val_binding = Stmt::Let {
            is_const: false,
            name: val_name.clone(),
            name_span: val_span.clone(),
            ty: None,
            value: Expr::FieldAccess {
                receiver: Box::new(Expr::Ident("__entry".to_string(), entry_span.clone())),
                field: "value".to_string(),
                field_span: entry_span.clone(),
                span: entry_span.clone(),
            },
            span: val_span.clone(),
        };

        let mut desugared_stmts = vec![key_binding, val_binding];
        desugared_stmts.extend(user_body.stmts);

        let full_span = SourceSpan::new(self.file, start, body_end);
        Stmt::For {
            var: "__entry".to_string(),
            var_span: entry_span,
            iter,
            body: Block {
                stmts: desugared_stmts,
                span: body_span,
            },
            span: full_span,
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
            // Postfix (highest precedence): call `(`, method `.`, index `[`, generic call `<`
            match self.peek() {
                Token::LParen => {
                    lhs = self.parse_call(lhs, None);
                    continue;
                }
                Token::Dot => {
                    lhs = self.parse_dot_postfix(lhs);
                    continue;
                }
                Token::LBracket => {
                    lhs = self.parse_index_access(lhs);
                    continue;
                }
                Token::Lt => {
                    // Speculatively try `<TypeArgs>(` for a generic call.
                    // If backtrack succeeds without `(` following, fall through
                    // to the infix handler which treats `<` as comparison.
                    if let Some(type_args) = self.try_parse_type_args() {
                        lhs = self.parse_call(lhs, Some(type_args));
                        continue;
                    }
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
            let span = SourceSpan::new(self.file, lhs.span().start, rhs.span().end);
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
            Token::None => {
                let span = self.current_span();
                self.advance();
                Expr::NoneLit { span }
            }
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
            // M7 P1: backtick string literal — may contain ${...} interpolations.
            Token::BacktickString(_) => self.parse_backtick_string(),
            Token::Identifier(name) => {
                let span = self.current_span();
                self.advance();
                // Prefix-form struct literal `Player { ... }` — banned; teaching diagnostic.
                if matches!(self.peek(), Token::LBrace) && self.peek_is_struct_lit_start(0) {
                    let brace_span = self.current_span();
                    self.diags.push(Diagnostic::error(
                        brace_span,
                        format!("`{name} {{ ... }}` is not valid Yinz shape value syntax."),
                        format!("Use an annotation-driven literal: `let p: {name} = {{ ... }}`"),
                        "In Yinz, shape values are written without a type prefix — the type comes from the `let`/`const` annotation.",
                    ));
                    // Parse and discard the body so the parser recovers cleanly.
                    self.advance(); // consume `{`
                    let _ = self.parse_struct_lit_fields();
                    return Expr::Error(span);
                }
                Expr::Ident(name, span)
            }
            Token::SelfValue => {
                let span = self.current_span();
                self.advance();
                Expr::SelfValue { span }
            }
            Token::LBrace if self.peek_is_map_lit_start() => self.parse_map_lit(),
            Token::LBrace if self.peek_is_struct_lit_start(0) => self.parse_struct_lit(),
            // Empty `{}` — parse as a struct literal with no fields; typeck resolves
            // based on the annotation (empty shape literal vs empty map literal).
            Token::LBrace if matches!(self.peek_ahead(1), Token::RBrace) => self.parse_struct_lit(),
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
            Token::LBracket => self.parse_array_lit(),
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
                        format!("`{}` cannot appear here.", token_display(self.peek())),
                        "Write a value or expression instead.",
                        "Expressions need something to evaluate — a number, name, or calculation.",
                    ));
                    self.advance();
                }
                Expr::Error(span)
            }
        }
    }

    /// Parse a backtick-quoted string literal into `Expr::InterpolatedString`.
    ///
    /// Called when the current token is `BacktickString(...)`. Collects all
    /// consecutive literal segments and `${...}` interpolations until the string
    /// is complete (no more `BacktickString` / `InterpolationStart` follow).
    ///
    /// The token stream for `` `hello ${name}!` `` looks like:
    ///   BacktickString("hello ") · InterpolationStart · Identifier("name") · InterpolationEnd · BacktickString("!")
    fn parse_backtick_string(&mut self) -> Expr {
        use ynz_ast::nodes::StringPart;

        let start_span = self.current_span();
        let mut parts: Vec<StringPart> = Vec::new();

        loop {
            match self.peek().clone() {
                Token::BacktickString(bytes) => {
                    let seg_span = self.current_span();
                    self.advance();
                    parts.push(StringPart::Lit(bytes, seg_span));
                    // After a Lit segment the string is either done or continues with
                    // another BacktickString (adjacent segments — shouldn't happen from
                    // the lexer, but handle defensively) or we're done.
                    // The next token is NOT BacktickString or InterpolationStart → done.
                    if !matches!(
                        self.peek(),
                        Token::BacktickString(_) | Token::InterpolationStart
                    ) {
                        break;
                    }
                }
                Token::InterpolationStart => {
                    let _start = self.current_span();
                    self.advance(); // consume `${`
                    let expr_span_start = self.current_span();
                    let inner = self.parse_expr(0);
                    // The lexer emits InterpolationEnd when it sees the matching `}`.
                    // Consume it here.
                    let expr_span_end = self.current_span();
                    if !matches!(self.peek(), Token::InterpolationEnd) {
                        self.diags.push(Diagnostic::error(
                            expr_span_end.clone(),
                            "Missing `}` to close this string interpolation.",
                            "Add `}` after the expression: `` `Hello ${name}` ``",
                            "Every `${` inside a backtick string must be closed with `}`.",
                        ));
                    } else {
                        self.advance(); // consume `}`
                    }
                    let span = SourceSpan::new(
                        &expr_span_start.file,
                        expr_span_start.start,
                        self.current_span().start,
                    );
                    parts.push(StringPart::Expr(Box::new(inner), span));
                    // After InterpolationEnd the lexer emits the next BacktickString.
                }
                _ => break,
            }
        }

        let end_span = if parts.is_empty() {
            start_span.clone()
        } else {
            self.current_span()
        };
        let full_span = SourceSpan::new(
            &start_span.file,
            start_span.start,
            end_span.start.max(start_span.end),
        );
        Expr::InterpolatedString(parts, full_span)
    }

    fn parse_call(&mut self, callee: Expr, type_args: Option<Vec<Type>>) -> Expr {
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
                        type_args,
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
                        type_args,
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

    /// Parse `receiver[index]` into `Expr::IndexAccess`.
    fn parse_index_access(&mut self, receiver: Expr) -> Expr {
        let start = receiver.span().start;
        self.advance(); // consume `[`

        let index = self.parse_expr(0);

        if self.expect(&Token::RBracket).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Missing `]` to close the index expression.",
                "Write `value[index]` — close the bracket after the index.",
                "Every `[` in an index expression must be matched with a `]`.",
            ));
        }

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(start);
        Expr::IndexAccess {
            receiver: Box::new(receiver),
            index: Box::new(index),
            span: SourceSpan::new(self.file, start, end),
        }
    }

    /// Parse `[expr, expr, ...]` into `Expr::ArrayLit`.
    fn parse_array_lit(&mut self) -> Expr {
        let start = self.current_span().start;
        self.advance(); // consume `[`
        let mut elements = Vec::new();
        loop {
            match self.peek() {
                Token::RBracket => {
                    let end = self.current_span().end;
                    self.advance();
                    return Expr::ArrayLit {
                        elements,
                        span: SourceSpan::new(self.file, start, end),
                    };
                }
                Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.eof_span(),
                        "Missing `]` to close this array literal.",
                        "Add `]` after the last element.",
                        "Every `[` in an array literal must be matched with a `]`.",
                    ));
                    break;
                }
                Token::Comma => {
                    self.advance();
                }
                _ => {
                    elements.push(self.parse_expr(0));
                }
            }
        }
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(start);
        Expr::ArrayLit {
            elements,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    /// Speculatively parse a generic type-argument list `<T, U>` followed by `(`.
    ///
    /// Returns `Some(type_args)` and leaves the parser positioned at `(` if the
    /// sequence `< type [, type]* > (` is found within 32 tokens.
    /// Returns `None` and restores parser state (position + diagnostics) on any
    /// failure — the `<` is treated as a comparison operator by the caller.
    fn try_parse_type_args(&mut self) -> Option<Vec<Type>> {
        debug_assert!(matches!(self.peek(), Token::Lt));

        let saved_pos = self.pos;
        let saved_diags_len = self.diags.len();

        self.advance(); // consume `<`
        let mut tokens_consumed: usize = 1;
        let mut args = Vec::new();

        loop {
            tokens_consumed += 1;
            if tokens_consumed > 32 {
                self.pos = saved_pos;
                self.diags.truncate(saved_diags_len);
                return None;
            }

            match self.peek() {
                Token::Gt => {
                    self.advance();
                    break;
                }
                Token::Comma => {
                    self.advance();
                }
                Token::Eof | Token::RBrace | Token::RParen => {
                    self.pos = saved_pos;
                    self.diags.truncate(saved_diags_len);
                    return None;
                }
                _ => {
                    let pre = self.pos;
                    let ty = self.parse_type_with_depth(0);
                    let used = self.pos.saturating_sub(pre);
                    tokens_consumed += used;
                    if matches!(ty, Type::Error) {
                        self.pos = saved_pos;
                        self.diags.truncate(saved_diags_len);
                        return None;
                    }
                    args.push(ty);
                }
            }
        }

        // Succeed only when `(` immediately follows — this is a generic call.
        if !matches!(self.peek(), Token::LParen) {
            self.pos = saved_pos;
            self.diags.truncate(saved_diags_len);
            return None;
        }

        Some(args)
    }

    /// Parse a `<T>` type-parameter list in a generic function or shape declaration.
    ///
    /// Called when the parser is positioned immediately after the declaration name.
    /// Returns an empty Vec when `<` is not present (non-generic declaration).
    fn parse_generic_params(&mut self) -> Vec<GenericParam> {
        if !matches!(self.peek(), Token::Lt) {
            return Vec::new();
        }
        self.advance(); // consume `<`

        let mut params = Vec::new();

        loop {
            match self.peek() {
                // Natural list terminators and hard boundaries — stop collecting params.
                Token::Gt | Token::Eof | Token::LBrace | Token::LParen | Token::Arrow => break,
                Token::Comma => {
                    self.advance();
                    continue;
                }
                _ => {}
            }

            let param_start = self.current_span().start;
            let (name, name_span) = match self.peek().clone() {
                Token::Identifier(n) => {
                    let span = self.current_span();
                    self.advance();
                    (n, span)
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected a type parameter name here.",
                        "Write `<T>` or `<T follows Comparable>` for the type parameter list.",
                        "Type parameters are names — usually a short uppercase letter like `T`, `U`, or `Key`.",
                    ));
                    // Consume to a safe boundary; stop at hard delimiters so we don't loop.
                    while !matches!(
                        self.peek(),
                        Token::Gt
                            | Token::Comma
                            | Token::Eof
                            | Token::LBrace
                            | Token::LParen
                            | Token::Arrow
                    ) {
                        self.advance();
                    }
                    if matches!(self.peek(), Token::Comma) {
                        self.advance(); // skip comma to try the next param
                    }
                    continue;
                }
            };

            // Optional `follows Contract` — one contract per type parameter.
            let mut constraints = Vec::new();
            if matches!(self.peek(), Token::Follows) {
                self.advance(); // consume `follows`
                match self.peek().clone() {
                    Token::Identifier(contract) => {
                        let cspan = self.current_span();
                        self.advance();
                        constraints.push((contract, cspan));
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected a contract name after `follows` in the type parameter list.",
                            "Write `<T follows Comparable>` where `Comparable` is a shape with contract signatures.",
                            "The `follows` constraint restricts which concrete types can fill in the type parameter.",
                        ));
                    }
                }
            }

            let end = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map(|s| s.span.end)
                .unwrap_or(param_start);
            params.push(GenericParam {
                name,
                name_span,
                constraints,
                span: SourceSpan::new(self.file, param_start, end),
            });

            if !matches!(self.peek(), Token::Gt | Token::Eof)
                && self.expect(&Token::Comma).is_none()
            {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected `,` or `>` after type parameter.",
                    "Separate type parameters with `,`: `<T, U>`. Close the list with `>`.",
                    "The type parameter list must end with `>`.",
                ));
                // Recover to something the parser can continue from
                while !matches!(
                    self.peek(),
                    Token::Gt | Token::LParen | Token::LBrace | Token::Eof
                ) {
                    self.advance();
                }
            }
        }

        if self.expect(&Token::Gt).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                "Missing `>` to close the type parameter list.",
                "Write `<T>` — close the angle bracket after the last type parameter.",
                "Every `<` in a type parameter list must be matched with a `>`.",
            ));
        }

        params
    }

    /// Parse a dot-postfix expression: field access, method call, or body operation.
    ///
    /// Dispatches on whether `(` follows the name:
    ///   - No `(` → `Expr::FieldAccess` (pure read, per dot-postfix rule)
    ///   - `(` after `copy` or `freeze` → `Expr::PostfixOp`
    ///   - `(` after anything else → `Expr::MethodCall`
    fn parse_dot_postfix(&mut self, receiver: Expr) -> Expr {
        let start = receiver.span().start;
        self.advance(); // consume `.`

        // Name after the dot
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
                    format!(
                        "Expected a field or method name after `.`, but found `{}`.",
                        token_display(self.peek())
                    ),
                    "Write a field name (`value.fieldName`) or method call (`value.method()`).",
                    "The `.` is used for field access and method calls — it must be followed by a name.",
                ));
                return Expr::Error(span);
            }
        };

        // No `(` → field access (pure read, no side effects)
        if !matches!(self.peek(), Token::LParen) {
            let end = name_span.end;
            return Expr::FieldAccess {
                receiver: Box::new(receiver),
                field: name,
                field_span: name_span,
                span: SourceSpan::new(self.file, start, end),
            };
        }

        // `.copy()` and `.freeze()` — body operations, produce PostfixOp
        if let Some(op) = match name.as_str() {
            "copy" => Some(PostfixOpKind::Copy),
            "freeze" => Some(PostfixOpKind::Freeze),
            _ => None,
        } {
            self.advance(); // consume `(`
            if self.expect(&Token::RParen).is_none() {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    format!("`.{name}()` takes no arguments."),
                    format!("Write `value.{name}()` with empty parentheses."),
                    format!("`{name}` is a dot-postfix body operation — it acts on the receiver value with no extra arguments."),
                ));
            }
            let end = self
                .tokens
                .get(self.pos.saturating_sub(1))
                .map(|s| s.span.end)
                .unwrap_or(start);
            return Expr::PostfixOp {
                receiver: Box::new(receiver),
                op,
                span: SourceSpan::new(self.file, start, end),
            };
        }

        // Regular method call — `(` is guaranteed here (FieldAccess returned above otherwise).
        let method = name;
        let method_span = name_span;
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

    // ── M4: struct literal helpers ──────────────────────────────────────────

    /// True when current position is the start of a map literal:
    /// `{` followed by a string/int/number literal (or backtick string) and then `:`.
    fn peek_is_map_lit_start(&self) -> bool {
        matches!(
            self.tokens.get(self.pos + 1).map(|s| &s.value),
            Some(Token::StringLit(_))
            | Some(Token::IntLit(_))
            | Some(Token::NumberLit(_))
            // test-ratchet: M7 P1 adds BacktickString as a valid map literal key
            | Some(Token::BacktickString(_))
        ) && matches!(
            self.tokens.get(self.pos + 2).map(|s| &s.value),
            Some(Token::Colon)
        )
    }

    /// True when position `offset` from current is the start of a struct literal:
    /// - offset=0: peek at `{`; returns true if the token AFTER `{` is `Identifier Colon`.
    /// - offset=1: peek is already `{`; checks offset+1 and offset+2.
    fn peek_is_struct_lit_start(&self, offset: usize) -> bool {
        // Token at `offset` should be `{`, and the two tokens after that
        // must be `Identifier` then `Colon`.
        let brace_pos = self.pos + offset;
        let ident_pos = brace_pos + 1;
        let colon_pos = brace_pos + 2;
        matches!(
            self.tokens.get(brace_pos).map(|s| &s.value),
            Some(Token::LBrace)
        ) && matches!(
            self.tokens.get(ident_pos).map(|s| &s.value),
            Some(Token::Identifier(_))
        ) && matches!(
            self.tokens.get(colon_pos).map(|s| &s.value),
            Some(Token::Colon)
        )
    }

    /// Parse an anonymous struct literal `{ field: value, ... }`.
    ///
    /// Called when we know `{` is followed by `Identifier Colon` (struct literal,
    /// not a block). Consumes the opening `{`.
    fn parse_struct_lit(&mut self) -> Expr {
        let start = self.current_span().start;
        self.advance(); // consume `{`
        let fields = self.parse_struct_lit_fields();
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(start);
        Expr::StructLit {
            fields,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    /// Parse the field list of a struct literal, including the closing `}`.
    fn parse_struct_lit_fields(&mut self) -> Vec<StructLitField> {
        let mut fields = Vec::new();
        loop {
            match self.peek() {
                Token::RBrace => {
                    self.advance();
                    break;
                }
                Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.eof_span(),
                        "Missing `}` to close this shape value.",
                        "Add `}` after the last field.",
                        "Every `{` in a shape value must be matched with a `}`.",
                    ));
                    break;
                }
                Token::Comma => {
                    self.advance();
                    continue;
                }
                _ => {}
            }

            // Parse `name: value`
            let (field_name, name_span) = match self.peek().clone() {
                Token::Identifier(n) => {
                    let span = self.current_span();
                    self.advance();
                    (n, span)
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected a field name here.",
                        "Write `{ fieldName: value, ... }` to initialize a shape value.",
                        "Shape values list each field by name followed by `:` and a value.",
                    ));
                    // Recover to `}` or `,`
                    while !matches!(self.peek(), Token::RBrace | Token::Comma | Token::Eof) {
                        self.advance();
                    }
                    continue;
                }
            };

            if self.expect(&Token::Colon).is_none() {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    format!("Expected `:` after field name `{field_name}`."),
                    format!("Write `{{ {field_name}: value }}`"),
                    "The `:` separates the field name from its value in a shape value.",
                ));
                while !matches!(self.peek(), Token::RBrace | Token::Comma | Token::Eof) {
                    self.advance();
                }
                continue;
            }

            let value = self.parse_expr(0);
            fields.push(StructLitField {
                name: field_name,
                name_span,
                value,
            });

            // Optional trailing comma
            if !matches!(self.peek(), Token::RBrace) {
                let _ = self.expect(&Token::Comma);
            }
        }
        fields
    }

    // ── M5 P3c: map literal ──────────────────────────────────────────────────

    /// Parse `{ "key": value, ... }` into `Expr::MapLit`.
    ///
    /// Called when we know `{` is followed by a non-identifier literal key then `:`.
    fn parse_map_lit(&mut self) -> Expr {
        let start = self.current_span().start;
        self.advance(); // consume `{`
        let mut entries: Vec<(Expr, Expr)> = Vec::new();
        loop {
            match self.peek() {
                Token::RBrace => {
                    let end = self.current_span().end;
                    self.advance();
                    return Expr::MapLit {
                        entries,
                        span: SourceSpan::new(self.file, start, end),
                    };
                }
                Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.eof_span(),
                        "Missing `}` to close this map literal.",
                        "Add `}` after the last entry.",
                        "Every `{` in a map literal must be matched with a `}`.",
                    ));
                    break;
                }
                Token::Comma => {
                    self.advance();
                    continue;
                }
                _ => {
                    let key = self.parse_expr(0);
                    if self.expect(&Token::Colon).is_none() {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected `:` between map key and value.",
                            "Write `{ key: value, ... }` for a map literal.",
                            "Map literals list key-value pairs separated by `:`.",
                        ));
                    }
                    let value = self.parse_expr(0);
                    entries.push((key, value));
                    if !matches!(self.peek(), Token::RBrace | Token::Comma | Token::Eof) {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected `,` or `}` after map entry.",
                            "Separate entries with `,`: `{ \"a\": 1, \"b\": 2 }`",
                            "Map literal entries must be separated by commas.",
                        ));
                    }
                }
            }
        }
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(start);
        Expr::MapLit {
            entries,
            span: SourceSpan::new(self.file, start, end),
        }
    }

    // ── M6: options declaration ──────────────────────────────────────────────

    /// Parse `options Name { variant1, variant2, ... }`.
    ///
    /// Consumes the `options` token. Tolerant: empty body parses (typeck rejects).
    fn parse_options_decl(&mut self) -> Option<OptionsDecl> {
        let start = self.current_span().start;
        self.advance(); // consume `options`

        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let sp = self.current_span();
                let name = n.clone();
                self.advance();
                (name, sp)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a name for the options type.",
                    "Write `options Status { active, inactive }` with a PascalCase name.",
                    "The name identifies the options type in the type system.",
                ));
                return None;
            }
        };

        if self.expect(&Token::LBrace).is_none() {
            return None;
        }

        let mut variants = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            match self.peek().clone() {
                Token::Identifier(v) => {
                    let v_span = self.current_span();
                    let v_name = v.clone();
                    self.advance();
                    variants.push((v_name, v_span));
                    // Optional trailing comma
                    if matches!(self.peek(), Token::Comma) {
                        self.advance();
                    }
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        format!("Expected a variant name inside `options {name} {{ ... }}`."),
                        "Write variant names as lowercase identifiers separated by commas.",
                        "Each variant is a named label: `options Status { active, inactive, banned }`.",
                    ));
                    self.advance();
                }
            }
        }

        let end = self.current_span().end;
        self.expect(&Token::RBrace);

        Some(OptionsDecl {
            name,
            name_span,
            variants,
            span: SourceSpan::new(self.file, start, end),
            is_exported: false, // set to true by parse_module when `export` precedes
        })
    }

    // ── M4: shape declaration ────────────────────────────────────────────────

    /// Parse a `shape Name [extends Parent] [follows C1, C2] { body }` declaration.
    ///
    /// `is_base` is true when the caller already consumed `base` before calling here.
    /// Consumes the `shape` token itself.
    fn parse_shape_decl(&mut self, is_base: bool) -> Option<ShapeDecl> {
        let start = self.current_span().start;
        self.advance(); // consume `shape`

        // Shape name
        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a shape name after `shape`.",
                    "Write `shape Name { ... }` — the name must start with a capital letter.",
                    "Shape names follow Yinz's capital-letter-equals-type rule (Golden Rule 13).",
                ));
                self.recover_to_rbrace();
                let _ = self.expect(&Token::RBrace);
                return None;
            }
        };

        // Optional `<T, U>` type-param list
        let generics = self.parse_generic_params();

        // M6: `shape Name = TypeExpr` — type alias form (union type declarations, etc.)
        if matches!(self.peek(), Token::Eq) {
            self.advance(); // consume `=`
            let alias_ty = self.parse_type();
            let end = self.current_span().start;
            return Some(ShapeDecl {
                name,
                name_span,
                is_base,
                generics,
                extends: None,
                follows: vec![],
                fields: vec![],
                contract_sigs: vec![],
                alias_ty: Some(alias_ty),
                span: SourceSpan::new(self.file, start, end),
                is_exported: false, // set to true by parse_module when `export` precedes
            });
        }

        // Optional `extends Parent`
        let extends = if matches!(self.peek(), Token::Extends) {
            self.advance(); // consume `extends`
            match self.peek().clone() {
                Token::Identifier(parent) => {
                    let span = self.current_span();
                    self.advance();
                    Some((parent, span))
                }
                _ => {
                    self.diags.push(Diagnostic::error(
                        self.current_span(),
                        "Expected a parent shape name after `extends`.",
                        "Write `shape Child extends Parent { ... }`.",
                        "`extends` is data-only inheritance — the child shape inherits all of the parent's fields.",
                    ));
                    None
                }
            }
        } else {
            None
        };

        // Optional `follows Contract1, Contract2, ...`
        let mut follows = Vec::new();
        if matches!(self.peek(), Token::Follows) {
            self.advance(); // consume `follows`
            loop {
                match self.peek().clone() {
                    Token::Identifier(contract) => {
                        let span = self.current_span();
                        self.advance();
                        follows.push((contract, span));
                        if !matches!(self.peek(), Token::Comma) {
                            break;
                        }
                        self.advance(); // consume `,`
                    }
                    _ => {
                        self.diags.push(Diagnostic::error(
                            self.current_span(),
                            "Expected a contract name after `follows`.",
                            "Write `shape Player follows Damageable { ... }`.",
                            "`follows` lists contract shapes whose bare-signature declarations this shape satisfies.",
                        ));
                        break;
                    }
                }
            }
        }

        // `{`
        if self.expect(&Token::LBrace).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Expected `{{` to open `shape {name}`'s body."),
                format!("Write `shape {name} {{ ... }}`"),
                "Shape bodies start with `{` and end with `}`.",
            ));
            return None;
        }

        // Shape body: fields and contract signatures
        let mut fields = Vec::new();
        let mut contract_sigs = Vec::new();

        loop {
            match self.peek() {
                Token::RBrace => {
                    self.advance();
                    break;
                }
                Token::Eof => {
                    self.diags.push(Diagnostic::error(
                        self.eof_span(),
                        format!("Missing `}}` to close `shape {name}`."),
                        "Add `}` at the end of the shape body.",
                        "Every `{` must be matched with a `}`.",
                    ));
                    break;
                }
                Token::Function => {
                    // `function` inside a shape body is a compile error in Yinz.
                    let span = self.current_span();
                    self.diags.push(Diagnostic::error(
                        span,
                        "Method bodies cannot be declared inside a shape.",
                        "Move this function outside the shape, then call it with dot syntax: `value.method()`.",
                        "Yinz is not object-oriented — shapes hold data and bare contract signatures only. Methods are standalone functions.",
                    ));
                    // Skip the whole function decl body for recovery
                    while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                        if matches!(self.peek(), Token::LBrace) {
                            self.advance();
                            self.recover_to_rbrace();
                            let _ = self.expect(&Token::RBrace);
                        } else {
                            self.advance();
                        }
                    }
                }
                Token::Hidden => {
                    // `hidden fieldName: Type = default`
                    let field_start = self.current_span().start;
                    self.advance(); // consume `hidden`
                    if let Some(field) = self.parse_field_decl(true, field_start) {
                        fields.push(field);
                    }
                }
                Token::Identifier(_) => {
                    // Could be a field `name: Type` or a contract sig `name(...)  -> Type`.
                    // Lookahead: Identifier Colon → field; Identifier LParen → contract sig.
                    let field_start = self.current_span().start;
                    match self.peek_ahead(1) {
                        Token::Colon => {
                            if let Some(field) = self.parse_field_decl(false, field_start) {
                                fields.push(field);
                            }
                        }
                        Token::LParen => {
                            if let Some(sig) = self.parse_contract_sig() {
                                contract_sigs.push(sig);
                            }
                        }
                        _ => {
                            let span = self.current_span();
                            self.diags.push(Diagnostic::error(
                                span,
                                "Expected a field declaration (`name: Type`) or a contract signature (`name(...)  -> Type`) here.",
                                "Shape bodies contain fields and optional bare method signatures.",
                                "Only data fields and contract method signatures are allowed inside a shape — no implementations.",
                            ));
                            self.advance();
                        }
                    }
                }
                _ => {
                    let span = self.current_span();
                    self.diags.push(Diagnostic::error(
                        span,
                        format!("Unexpected `{}` inside shape body.", token_display(self.peek())),
                        "Shape bodies contain fields (`name: Type`) and optional contract signatures.",
                        "Only data fields and bare method signatures are allowed inside a shape.",
                    ));
                    self.advance();
                }
            }
        }

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(start);
        Some(ShapeDecl {
            name,
            name_span,
            is_base,
            generics,
            extends,
            follows,
            fields,
            contract_sigs,
            alias_ty: None,
            span: SourceSpan::new(self.file, start, end),
            is_exported: false, // set to true by parse_module when `export` precedes
        })
    }

    /// Parse a field declaration `[hidden] name: Type [= default]`.
    fn parse_field_decl(&mut self, is_hidden: bool, field_start: usize) -> Option<FieldDecl> {
        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    self.current_span(),
                    "Expected a field name.",
                    "Write `fieldName: Type` to declare a field.",
                    "Fields need a name and a type.",
                ));
                return None;
            }
        };

        if self.expect(&Token::Colon).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Expected `:` after field name `{name}`."),
                format!("Write `{name}: Type`"),
                "The `:` separates the field name from its type.",
            ));
            return None;
        }

        let ty_start = self.current_span().start;
        let ty = self.parse_type();
        let ty_end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(ty_start);
        let ty_span = SourceSpan::new(self.file, ty_start, ty_end);

        // Optional `= default` (only valid for hidden fields; typeck enforces this)
        let default = if matches!(self.peek(), Token::Eq) {
            self.advance(); // consume `=`
            Some(self.parse_expr(0))
        } else {
            None
        };

        let end = default.as_ref().map(|e| e.span().end).unwrap_or(ty_end);
        Some(FieldDecl {
            name,
            name_span,
            ty,
            ty_span,
            is_hidden,
            default,
            span: SourceSpan::new(self.file, field_start, end),
        })
    }

    /// Parse a bare contract method signature `name([share|lend|give] self[, params]) -> Type`.
    ///
    /// No `function` keyword; no body. Example: `greet(share self) -> string`
    fn parse_contract_sig(&mut self) -> Option<ContractSig> {
        let start = self.current_span().start;
        let (name, name_span) = match self.peek().clone() {
            Token::Identifier(n) => {
                let span = self.current_span();
                self.advance();
                (n, span)
            }
            _ => return None,
        };

        self.expect(&Token::LParen)?;

        // Optional receiver: `share self` / `lend self` / `give self`
        let mut receiver = None;
        let mut params = Vec::new();

        if !matches!(self.peek(), Token::RParen | Token::Eof) {
            // Check for ownership modifier + `self`
            let ownership_kw = if let Token::Identifier(kw) = self.peek().clone() {
                match kw.as_str() {
                    "share" | "lend" | "give" => {
                        let k = kw.clone();
                        self.advance();
                        Some(k)
                    }
                    _ => None,
                }
            } else {
                None
            };

            if matches!(self.peek(), Token::SelfValue) {
                self.advance(); // consume `self`
                receiver = ownership_kw.as_deref().map(|kw| match kw {
                    "share" => ReceiverKind::Share,
                    "lend" => ReceiverKind::Lend,
                    "give" => ReceiverKind::Give,
                    _ => ReceiverKind::Share,
                });
                // Consume optional `,` after self
                let _ = self.expect(&Token::Comma);
            } else {
                // Not `self` — put back the ownership keyword as an error note
                // and parse as a normal param list.
                // (Edge case: user wrote `name(int)` without self — typeck catches it)
            }

            // Remaining params
            while !matches!(self.peek(), Token::RParen | Token::Eof) {
                let param_start = self.current_span().start;
                let own = if let Token::Identifier(kw) = self.peek().clone() {
                    match kw.as_str() {
                        "share" => {
                            self.advance();
                            Some(OwnershipModifier::Share)
                        }
                        "lend" => {
                            self.advance();
                            Some(OwnershipModifier::Lend)
                        }
                        "give" => {
                            self.advance();
                            Some(OwnershipModifier::Give)
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                let (pname, pname_span) = match self.peek().clone() {
                    Token::Identifier(n) => {
                        let s = self.current_span();
                        self.advance();
                        (n, s)
                    }
                    _ => break,
                };
                if self.expect(&Token::Colon).is_none() {
                    break;
                }
                let ty_s = self.current_span().start;
                let ty = self.parse_type();
                let ty_e = self
                    .tokens
                    .get(self.pos.saturating_sub(1))
                    .map(|s| s.span.end)
                    .unwrap_or(ty_s);
                params.push(Param {
                    name: pname,
                    name_span: pname_span,
                    ownership: own,
                    ty,
                    ty_span: SourceSpan::new(self.file, ty_s, ty_e),
                    span: SourceSpan::new(self.file, param_start, ty_e),
                });
                if !matches!(self.peek(), Token::RParen) {
                    let _ = self.expect(&Token::Comma);
                }
            }
        }

        if self.expect(&Token::RParen).is_none() {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Missing `)` to close contract signature `{name}`."),
                "Add `)` after the last parameter.",
                "Every `(` in a contract signature must be matched with a `)`.",
            ));
        }

        // `-> ReturnType`
        let return_type = if matches!(self.peek(), Token::Arrow) {
            self.advance(); // consume `->`
            self.parse_type()
        } else {
            self.diags.push(Diagnostic::error(
                self.current_span(),
                format!("Contract signature `{name}` is missing a return type."),
                format!("Add `-> nothing` or `-> ReturnType`: `{name}(share self) -> nothing`"),
                "Contract signatures must declare their return type — the compiler needs it to verify implementing functions.",
            ));
            Type::Error
        };

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|s| s.span.end)
            .unwrap_or(start);
        Some(ContractSig {
            name,
            name_span,
            receiver,
            params,
            return_type,
            span: SourceSpan::new(self.file, start, end),
        })
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
        Token::Shape => "shape",
        Token::Follows => "follows",
        Token::Extends => "extends",
        Token::Base => "base",
        Token::Hidden => "hidden",
        Token::Dynamic => "dynamic",
        Token::SelfType => "Self",
        Token::SelfValue => "self",
        Token::None => "none",
        Token::Options => "options",
        Token::Is => "is",
        // M7 P1: backtick strings + errors keyword
        Token::BacktickString(_) => "string literal",
        Token::InterpolationStart => "${",
        Token::InterpolationEnd => "}",
        Token::Errors => "errors",
        // M8 P1: modules + sensitive + concurrency keywords
        Token::Import => "import",
        Token::Export => "export",
        Token::Sensitive => "sensitive",
        Token::Wait => "wait",
        Token::Background => "background",
        Token::DocComment { .. } => "doc comment",
    }
}
