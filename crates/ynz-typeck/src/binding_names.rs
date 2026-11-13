//! Per-binding codegen names — the ONE answer to "which declaration is this identifier?".
//!
//! Typeck's scope walk (`Checker::resolve_ident` and the scope code) already resolves every
//! identifier use to the declaration it names. This module records that answer and gives every
//! declaration a codegen name that is unique inside its function, so the name-keyed structures
//! in the backend (state-machine frame slots, crossing allocas, `cg.locals`, SoA bindings)
//! hold exactly one binding per key (v0.3-M9 C4, spike S2; `authoritative-derivation.md`:
//! codegen applies this map and never re-resolves scoping).
//!
//! * [`BindingNameRecorder`] — filled by the checker while it walks a function: each declaration
//!   (`let`, `for` variable, desugared destructure alias, parameter) and each identifier use.
//! * [`BindingNames`] — the finished record, carried on `TypedModule`.
//! * [`BindingNames::apply`] — returns a clone of the module with every declaration and use
//!   renamed; the checker's diagnostics, lints and the LSP never see it (they keep user names).
//!
//! # Naming
//!
//! The first declaration of a name in a function keeps it. Every later declaration of that name
//! becomes `name__N`, with N the smallest value for which `name__N` is not the name of any
//! declaration in the function. Declarations are keyed by `(span, declared name)`: the parser's
//! desugared bindings reuse a neighbouring span (`for ((k, v) in m)` gives the synthetic
//! `__entry` variable and `k` the same span), so the span alone is not unique.
//!
//! # Release-pass keying (card C5)
//!
//! Anything that later looks a binding up by name — the scope-exit release list above all —
//! must use the unique codegen name (the name in the cloned AST) or the declaration key
//! (`DeclKey`) from this map, never the name the user wrote: two live bindings can share that.

use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{
    Block, CallExpr, Expr, FunctionDecl, Item, MatchArm, MatchPatternKind, Module, Stmt, StringPart,
};
use ynz_diagnostics::SourceSpan;

/// A declaration: `(span.start, span.end, declared name)`.
pub type DeclKey = (usize, usize, String);

/// An identifier use: `(span.start, span.end, name as written)`.
pub type UseKey = (usize, usize, String);

/// The declaration key of a binding declared with `name` at `span`.
pub fn decl_key(span: &SourceSpan, name: &str) -> DeclKey {
    (span.start, span.end, name.to_string())
}

/// The use key of an identifier `name` written at `span`.
pub fn use_key(span: &SourceSpan, name: &str) -> UseKey {
    (span.start, span.end, name.to_string())
}

/// The finished record of a module's bindings (see the module docs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BindingNames {
    /// Declaration -> its codegen name (unique inside the declaring function).
    decl_names: HashMap<DeclKey, String>,
    /// Identifier use -> the declaration it resolves to.
    uses: HashMap<UseKey, DeclKey>,
    /// Identifier uses typeck resolved to two different declarations (never expected: the
    /// same node is walked in the same scope). Reported by [`BindingNames::apply`].
    conflicting_uses: Vec<UseKey>,
    /// Per function (keyed by the function's span): every name any of its bindings was
    /// declared with. An identifier with that name and no resolution entry is a hole in the
    /// record, so `apply` rejects it instead of leaving the raw name.
    function_binding_names: HashMap<(usize, usize), HashSet<String>>,
}

impl BindingNames {
    /// The codegen name of the declaration `key`, if typeck recorded it.
    pub fn codegen_name(&self, key: &DeclKey) -> Option<&str> {
        self.decl_names.get(key).map(String::as_str)
    }

    /// The declaration the identifier `name` written at `span` resolves to, if recorded.
    pub fn declaration_of_use(&self, span: &SourceSpan, name: &str) -> Option<&DeclKey> {
        self.uses.get(&use_key(span, name))
    }

    /// Clone `module` with every binding declaration and use renamed to its codegen name.
    ///
    /// Hard error (never a fallback to the raw name) when a declaration has no codegen name,
    /// or an identifier that names one of its function's bindings has no resolution entry.
    ///
    /// O(n) time and memory in the size of the module.
    pub fn apply(&self, module: &Module) -> Result<Module, String> {
        if let Some((start, end, name)) = self.conflicting_uses.first() {
            return Err(format!(
                "internal: identifier `{name}` at bytes {start}..{end} resolved to two different \
                 declarations during type checking"
            ));
        }
        let mut renamed = module.clone();
        for item in &mut renamed.items {
            if let Item::Function(function) = item {
                self.apply_to_function(function)?;
            }
        }
        Ok(renamed)
    }

    fn apply_to_function(&self, function: &mut FunctionDecl) -> Result<(), String> {
        let empty = HashSet::new();
        let binding_names = self
            .function_binding_names
            .get(&(function.span.start, function.span.end))
            .unwrap_or(&empty);
        let mut renamer = Renamer {
            names: self,
            function_name: function.name.clone(),
            binding_names,
        };
        for param in &mut function.params {
            renamer.rename_declaration(&mut param.name, &param.name_span)?;
        }
        renamer.block(&mut function.body)
    }
}

/// Fills a [`BindingNames`] while the checker walks the module, one function at a time.
#[derive(Debug, Default)]
pub struct BindingNameRecorder {
    names: BindingNames,
    /// The function currently being walked (its span) and its declarations in walk order.
    current_function: Option<(usize, usize)>,
    current_declarations: Vec<DeclKey>,
    declared_anywhere: HashSet<DeclKey>,
}

impl BindingNameRecorder {
    /// Start walking a function. Names the previous function's bindings first.
    pub fn begin_function(&mut self, function_span: &SourceSpan) {
        self.finish_function();
        self.current_function = Some((function_span.start, function_span.end));
    }

    /// Record a declaration. Idempotent: a node walked twice is declared once.
    pub fn declare(&mut self, key: DeclKey) {
        if self.declared_anywhere.insert(key.clone()) {
            self.current_declarations.push(key);
        }
    }

    /// Record that the identifier `use_at` resolved to the declaration `declaration`.
    pub fn note_use(&mut self, use_at: UseKey, declaration: DeclKey) {
        match self.names.uses.get(&use_at) {
            Some(existing) if *existing != declaration => {
                self.names.conflicting_uses.push(use_at);
            }
            Some(_) => {}
            None => {
                self.names.uses.insert(use_at, declaration);
            }
        }
    }

    /// Name the current function's declarations and close it.
    ///
    /// O(d) time and memory for d declarations in the function (the suffix search is bounded
    /// by the number of names already taken).
    pub fn finish_function(&mut self) {
        let Some(function) = self.current_function.take() else {
            return;
        };
        let declarations = std::mem::take(&mut self.current_declarations);
        let declared_names: HashSet<&str> = declarations
            .iter()
            .map(|declaration| declaration.2.as_str())
            .collect();
        let mut taken: HashSet<String> = HashSet::new();
        let mut next_suffix: HashMap<&str, u32> = HashMap::new();
        let mut named: Vec<(DeclKey, String)> = Vec::with_capacity(declarations.len());
        for key in &declarations {
            let base = key.2.as_str();
            let unique = if taken.insert(base.to_string()) {
                base.to_string()
            } else {
                loop {
                    let counter = next_suffix.entry(base).or_insert(0);
                    *counter += 1;
                    let candidate = format!("{base}__{counter}");
                    if !declared_names.contains(candidate.as_str())
                        && taken.insert(candidate.clone())
                    {
                        break candidate;
                    }
                }
            };
            named.push((key.clone(), unique));
        }
        self.names.decl_names.extend(named);
        self.names
            .function_binding_names
            .entry(function)
            .or_default()
            .extend(declarations.into_iter().map(|declaration| declaration.2));
    }

    /// Close the last function and hand over the finished record.
    pub fn finish(mut self) -> BindingNames {
        self.finish_function();
        self.names
    }
}

/// Walks a function applying the map.
struct Renamer<'a> {
    names: &'a BindingNames,
    function_name: String,
    binding_names: &'a HashSet<String>,
}

impl Renamer<'_> {
    fn rename_declaration(&mut self, name: &mut String, span: &SourceSpan) -> Result<(), String> {
        let key = decl_key(span, name);
        match self.names.decl_names.get(&key) {
            Some(codegen_name) => {
                *name = codegen_name.clone();
                Ok(())
            }
            None => Err(format!(
                "internal: the declaration of `{name}` at bytes {}..{} in `{}` has no codegen \
                 name from type checking",
                span.start, span.end, self.function_name
            )),
        }
    }

    fn rename_use(&mut self, name: &mut String, span: &SourceSpan) -> Result<(), String> {
        match self.names.uses.get(&use_key(span, name)) {
            Some(declaration) => match self.names.decl_names.get(declaration) {
                Some(codegen_name) => {
                    *name = codegen_name.clone();
                    Ok(())
                }
                None => Err(format!(
                    "internal: `{name}` at bytes {}..{} in `{}` resolves to a declaration with \
                     no codegen name",
                    span.start, span.end, self.function_name
                )),
            },
            // Not a binding use (a function, a type, a module): nothing to rename. But a name
            // that one of this function's bindings was declared with, and that type checking
            // never resolved, is a hole in the record — codegen must not guess.
            None if self.binding_names.contains(name.as_str()) => Err(format!(
                "internal: identifier `{name}` at bytes {}..{} in `{}` has no resolution entry \
                 from type checking, but `{name}` is also a local binding of this function",
                span.start, span.end, self.function_name
            )),
            None => Ok(()),
        }
    }

    fn block(&mut self, block: &mut Block) -> Result<(), String> {
        for stmt in &mut block.stmts {
            self.stmt(stmt)?;
        }
        Ok(())
    }

    fn stmt(&mut self, stmt: &mut Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Expr(expr) => self.expr(expr),
            Stmt::Let {
                name,
                name_span,
                value,
                ..
            } => {
                // The initializer is resolved before the new binding exists.
                self.expr(value)?;
                self.rename_declaration(name, name_span)
            }
            Stmt::Assign {
                target,
                target_span,
                value,
                ..
            } => {
                self.expr(value)?;
                self.rename_use(target, target_span)
            }
            Stmt::If { cond, body, .. } | Stmt::While { cond, body, .. } => {
                self.expr(cond)?;
                self.block(body)
            }
            Stmt::Match {
                scrutinee,
                arms,
                else_arm,
                ..
            } => {
                self.expr(scrutinee)?;
                for arm in arms {
                    self.match_arm(arm)?;
                }
                if let Some(else_block) = else_arm {
                    self.block(else_block)?;
                }
                Ok(())
            }
            Stmt::For {
                var,
                var_span,
                iter,
                body,
                destructure_pattern,
                map_destructure_pattern,
                ..
            } => {
                self.expr(iter)?;
                self.rename_declaration(var, var_span)?;
                self.block(body)?;
                // The parser desugars `{ field as alias }` / `(key, value)` into the leading
                // `let` statements of the body. The pattern the backend's name-keyed analyses
                // read must name the same bindings as those lets.
                if let Some(bindings) = destructure_pattern {
                    for (index, binding) in bindings.iter_mut().enumerate() {
                        if let Some(Stmt::Let { name, .. }) = body.stmts.get(index) {
                            if *name != binding.field {
                                binding.alias = Some(name.clone());
                            }
                        }
                    }
                }
                if let Some((key_name, value_name)) = map_destructure_pattern {
                    if let Some(Stmt::Let { name, .. }) = body.stmts.first() {
                        *key_name = name.clone();
                    }
                    if let Some(Stmt::Let { name, .. }) = body.stmts.get(1) {
                        *value_name = name.clone();
                    }
                }
                Ok(())
            }
            Stmt::Return { value, .. } => match value {
                Some(expr) => self.expr(expr),
                None => Ok(()),
            },
            Stmt::FieldAssign { target, value, .. } => {
                self.expr(target)?;
                self.expr(value)
            }
            Stmt::IndexAssign {
                receiver,
                index,
                value,
                ..
            } => {
                self.expr(receiver)?;
                self.expr(index)?;
                self.expr(value)
            }
        }
    }

    fn match_arm(&mut self, arm: &mut MatchArm) -> Result<(), String> {
        if let MatchPatternKind::Value(expr) = &mut arm.pattern.kind {
            self.expr(expr)?;
        }
        self.block(&mut arm.body)
    }

    fn call(&mut self, call: &mut CallExpr) -> Result<(), String> {
        // A call's callee names a function, never a local binding.
        if !matches!(call.callee, Expr::Ident(..)) {
            self.expr(&mut call.callee)?;
        }
        for arg in &mut call.args {
            self.expr(arg)?;
        }
        Ok(())
    }

    fn expr(&mut self, expr: &mut Expr) -> Result<(), String> {
        match expr {
            Expr::Ident(name, span) => {
                let span = span.clone();
                self.rename_use(name, &span)
            }
            Expr::Call(call) => self.call(call),
            Expr::StringLit(..)
            | Expr::Error(..)
            | Expr::IntLit(..)
            | Expr::NumberLit(..)
            | Expr::BoolLit(..)
            | Expr::SelfValue { .. }
            | Expr::NoneLit { .. } => Ok(()),
            Expr::BinOp { lhs, rhs, .. } => {
                self.expr(lhs)?;
                self.expr(rhs)
            }
            Expr::UnaryOp { operand, .. } => self.expr(operand),
            Expr::MethodCall { receiver, args, .. } => {
                self.expr(receiver)?;
                for arg in args {
                    self.expr(arg)?;
                }
                Ok(())
            }
            Expr::FieldAccess { receiver, .. } | Expr::PostfixOp { receiver, .. } => {
                self.expr(receiver)
            }
            Expr::StructLit { fields, .. } => {
                for field in fields {
                    self.expr(&mut field.value)?;
                }
                Ok(())
            }
            Expr::IndexAccess {
                receiver, index, ..
            } => {
                self.expr(receiver)?;
                self.expr(index)
            }
            Expr::ArrayLit { elements, .. } => {
                for element in elements {
                    self.expr(element)?;
                }
                Ok(())
            }
            Expr::MapLit { entries, .. } => {
                for (key, value) in entries {
                    self.expr(key)?;
                    self.expr(value)?;
                }
                Ok(())
            }
            Expr::Is { expr, .. } => self.expr(expr),
            Expr::InterpolatedString(parts, _) => {
                for part in parts {
                    if let StringPart::Expr(inner, _) = part {
                        self.expr(inner)?;
                    }
                }
                Ok(())
            }
            Expr::Wait(inner, _) | Expr::Background(inner, _) => self.expr(inner),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tier: High (data integrity — the names decide which binding owns a frame slot).

    use ynz_ast::nodes::{Expr, Item, Module, Stmt};
    use ynz_parser::{CompilerDb, SourceFile};

    use crate::queries::check_query;

    use super::BindingNames;

    fn recorded(source: &str) -> (Module, BindingNames) {
        let db = CompilerDb::default();
        let file = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
        let output = check_query(&db, file);
        assert!(
            !output.diagnostics.has_errors(),
            "fixture must type-check: {:?}",
            output.diagnostics
        );
        (
            output.typed_module.module.clone(),
            output.typed_module.binding_names.clone(),
        )
    }

    fn let_names(module: &Module) -> Vec<String> {
        fn walk(stmts: &[Stmt], out: &mut Vec<String>) {
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, .. } => out.push(name.clone()),
                    Stmt::If { body, .. } | Stmt::While { body, .. } => walk(&body.stmts, out),
                    Stmt::For { var, body, .. } => {
                        out.push(var.clone());
                        walk(&body.stmts, out);
                    }
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for item in &module.items {
            if let Item::Function(function) = item {
                walk(&function.body.stmts, &mut out);
            }
        }
        out
    }

    // WHY: two live bindings named `x` must get two codegen names, and a name the user wrote
    // (`x__1`) must never be handed out for a different binding — a clash would put two
    // bindings back in one slot, the exact bug class this map exists to close.
    #[test]
    fn shadowing_declarations_get_unique_names_that_avoid_user_names() {
        let source = "function entrypoint() -> nothing {\n  let flag = true\n  let x = 1\n  \
                      let x__1 = 2\n  if (flag) {\n    let x = 3\n    print(x.toString())\n  }\n  \
                      print(x__1.toString())\n}\n";
        let (module, names) = recorded(source);
        let renamed = names.apply(&module).expect("a complete record applies");
        let renamed_lets = let_names(&renamed);
        assert_eq!(
            renamed_lets,
            vec!["flag", "x", "x__1", "x__2"],
            "first `x` keeps its name, the user's `x__1` is untouched, the nested `x` skips it"
        );
        assert_eq!(
            let_names(&module),
            vec!["flag", "x", "x__1", "x"],
            "the checker's own module keeps the user's names"
        );
    }

    // WHY: a use must follow its own declaration, not the name — the nested `print(x)` reads
    // the inner `x`, the outer one reads the outer `x`.
    #[test]
    fn uses_follow_their_declaration_not_the_name() {
        let source = "function entrypoint() -> nothing {\n  let flag = true\n  let x = 1\n  \
                      if (flag) {\n    let x = 2\n    print(x.toString())\n  }\n  \
                      print(x.toString())\n}\n";
        let (module, names) = recorded(source);
        let renamed = names.apply(&module).expect("a complete record applies");
        let Item::Function(function) = &renamed.items[0] else {
            panic!("expected the entrypoint function");
        };
        let receiver_name = |stmt: &Stmt| -> String {
            let Stmt::Expr(Expr::Call(call)) = stmt else {
                panic!("expected a print call");
            };
            let Expr::MethodCall { receiver, .. } = &call.args[0] else {
                panic!("expected x.toString()");
            };
            let Expr::Ident(name, _) = receiver.as_ref() else {
                panic!("expected an identifier receiver");
            };
            name.clone()
        };
        let Stmt::If { body, .. } = &function.body.stmts[2] else {
            panic!("expected the if");
        };
        assert_eq!(receiver_name(&body.stmts[1]), "x__1", "inner read");
        assert_eq!(receiver_name(&function.body.stmts[3]), "x", "outer read");
    }

    // WHY: Fail Fast — a declaration typeck never recorded must be a hard error, never the raw
    // name (which would silently share a slot with another binding).
    #[test]
    fn declaration_without_a_codegen_name_is_a_hard_error() {
        let (module, _) =
            recorded("function entrypoint() -> nothing {\n  let x = 1\n  print(x.toString())\n}\n");
        let error = BindingNames::default()
            .apply(&module)
            .expect_err("an empty record must not apply");
        assert!(error.contains("no codegen name"), "{error}");
    }

    // WHY: the parity assertion — an identifier that names a local binding but has no
    // resolution entry is a hole in the record; codegen must refuse it, not keep the raw name.
    #[test]
    fn local_identifier_without_a_resolution_entry_is_a_hard_error() {
        let (module, mut names) =
            recorded("function entrypoint() -> nothing {\n  let x = 1\n  print(x.toString())\n}\n");
        names.uses.clear();
        let error = names
            .apply(&module)
            .expect_err("a use with no entry must not apply");
        assert!(error.contains("no resolution entry"), "{error}");
    }
}
