use ynz_ast::nodes::{Expr, FunctionDecl, Item, Module, Stmt, Type as AstType};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{builtins::BuiltinTable, types::Type};

/// The type-annotated view of a module. Each expression maps to its resolved type.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedModule {
    pub module: Module,
    /// Per-expression types keyed by the expression's span start byte.
    /// `Type::Error` means the expression's type could not be determined.
    pub expr_types: std::collections::HashMap<usize, Type>,
}

/// Run the M1 type checker.
///
/// Returns a `TypedModule` and any type-level diagnostics.
///
/// **Parse-error gate**: if a function's body contains any `Expr::Error` or
/// `Type::Error` node (inserted during parse-error recovery), that function's
/// body is skipped entirely. Top-level checks (e.g. "is `main` defined?") still run.
pub fn check(module: &Module, builtins: &BuiltinTable) -> (TypedModule, DiagnosticBucket) {
    let mut checker = Checker {
        builtins,
        expr_types: std::collections::HashMap::new(),
        diags: DiagnosticBucket::new(),
    };
    checker.check_module(module);
    let typed = TypedModule {
        module: module.clone(),
        expr_types: checker.expr_types,
    };
    (typed, checker.diags)
}

struct Checker<'b> {
    builtins: &'b BuiltinTable,
    expr_types: std::collections::HashMap<usize, Type>,
    diags: DiagnosticBucket,
}

impl<'b> Checker<'b> {
    fn check_module(&mut self, module: &Module) {
        // Top-level: verify `main` exists.
        let main_decl = module.items.iter().find_map(|item| match item {
            Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        });

        if main_decl.is_none() {
            self.diags.push(Diagnostic::error(
                module.span.clone(),
                "This file has no `main` function.",
                "Add a main function:\n  function main() -> nothing {\n    ...\n  }",
                "Every Yinz program needs a `main` function — that is where execution starts.",
            ));
        }

        for item in &module.items {
            match item {
                Item::Function(f) => self.check_function(f),
            }
        }
    }

    fn check_function(&mut self, f: &FunctionDecl) {
        // Validate `main` signature.
        if f.name == "main" {
            match &f.return_type {
                AstType::Nothing => {} // correct
                AstType::Error => {}   // already has a parse error, skip
                other => {
                    self.diags.push(Diagnostic::error(
                        f.span.clone(),
                        format!(
                            "`main` must return `nothing`, but this says it returns `{}`.",
                            ast_type_name(other)
                        ),
                        "Change the return type to `nothing`: `function main() -> nothing`",
                        "`main` is the entry point of the program. It does not return a value — \
                         it runs and then the program ends.",
                    ));
                }
            }
        }

        // Parse-error gate: if the return type is Error or the body has error
        // nodes, skip body type-checking for this function.
        if f.return_type == AstType::Error || body_has_error_node(&f.body.stmts) {
            return;
        }

        self.check_block_stmts(&f.body.stmts);
    }

    fn check_block_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(expr) => {
                    self.infer_expr(expr);
                }
            }
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> Type {
        let ty = match expr {
            Expr::StringLit(_, _) => Type::String,
            Expr::Ident(name, span) => self.resolve_ident(name, span),
            Expr::Call(call) => self.check_call(call),
            Expr::Error(span) => {
                self.expr_types.insert(span.start, Type::Error);
                return Type::Error;
            }
        };
        self.expr_types.insert(expr.span().start, ty.clone());
        ty
    }

    fn resolve_ident(&mut self, name: &str, span: &SourceSpan) -> Type {
        if let Some(sig) = self.builtins.lookup(name) {
            return sig.ret.clone();
        }
        self.diags.push(Diagnostic::error(
            span.clone(),
            format!("`{name}` is not defined."),
            format!("Check the spelling, or define `{name}` as a function."),
            "The compiler looks up every name you use. If a name doesn't exist in scope, the program can't run.",
        ));
        Type::Error
    }

    fn check_call(&mut self, call: &ynz_ast::nodes::CallExpr) -> Type {
        // Infer callee type to resolve it (needed for diagnostics).
        let callee_name = match &call.callee {
            Expr::Ident(name, _) => name.clone(),
            _ => {
                let callee_type = self.infer_expr(&call.callee);
                let _ = callee_type;
                // Complex callee expressions (e.g. method chains) come in M3+.
                return Type::Error;
            }
        };

        let sig = match self.builtins.lookup(&callee_name) {
            Some(s) => s.clone(),
            None => {
                self.diags.push(Diagnostic::error(
                    call.callee.span().clone(),
                    format!("`{callee_name}` is not defined."),
                    format!("Check the spelling, or define `{callee_name}` as a function."),
                    "The compiler looks up every name you use. If a name doesn't exist, the program can't run.",
                ));
                return Type::Error;
            }
        };

        // Check argument count.
        if call.args.len() != sig.params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{callee_name}` takes {} argument(s), but {} were given.",
                    sig.params.len(),
                    call.args.len()
                ),
                format!(
                    "Call it with {} argument(s): `{callee_name}({})`",
                    sig.params.len(),
                    sig.params
                        .iter()
                        .enumerate()
                        .map(|(i, t)| format!("arg{i}: {}", type_name(t)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                "Every function call must match the number of arguments the function expects.",
            ));
            return Type::Error;
        }

        // Check argument types.
        for (i, (arg, expected)) in call.args.iter().zip(&sig.params).enumerate() {
            let actual = self.infer_expr(arg);
            if actual != *expected && actual != Type::Error {
                self.diags.push(Diagnostic::error(
                    arg.span().clone(),
                    format!(
                        "Argument {} to `{callee_name}` should be a {}, but got a {}.",
                        i + 1,
                        type_name(expected),
                        type_name(&actual)
                    ),
                    format!("Pass a {} here.", type_name(expected)),
                    format!(
                        "`{callee_name}` expects a {} in this position. Passing a {} here would cause a type error.",
                        type_name(expected),
                        type_name(&actual)
                    ),
                ));
            }
        }

        sig.ret.clone()
    }
}

fn body_has_error_node(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Expr(e) => expr_has_error(e),
    })
}

fn expr_has_error(expr: &Expr) -> bool {
    match expr {
        Expr::Error(_) => true,
        Expr::Call(c) => expr_has_error(&c.callee) || c.args.iter().any(expr_has_error),
        Expr::Ident(_, _) | Expr::StringLit(_, _) => false,
    }
}

fn type_name(t: &Type) -> &'static str {
    match t {
        Type::Nothing => "nothing",
        Type::String => "string",
        Type::Error => "unknown",
    }
}

fn ast_type_name(t: &AstType) -> &'static str {
    match t {
        AstType::Nothing => "nothing",
        AstType::Named(_, _) => "named type",
        AstType::Error => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use ynz_ast::nodes::{
        Block, CallExpr, Expr, FunctionDecl, Item, Module, Stmt, Type as AstType,
    };
    use ynz_diagnostics::SourceSpan;

    use super::*;

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new("test.ynz", start, end)
    }

    #[test]
    fn type_mismatch_produces_three_part_diagnostic() {
        // WHY: this is the load-bearing test for the type-mismatch code path.
        // Without it, silent type errors can ship because the mismatch branch
        // was never exercised. We use a test-only builtin to avoid needing
        // `int` literals, which are M2 work.
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                name: "main".into(),
                return_type: AstType::Nothing,
                body: Block {
                    stmts: vec![Stmt::Expr(Expr::Call(Box::new(CallExpr {
                        callee: Expr::Ident("_test_takes_nothing".into(), span(29, 48)),
                        args: vec![Expr::StringLit(b"hi".to_vec(), span(49, 53))],
                        span: span(29, 54),
                    })))],
                    span: span(28, 56),
                },
                span: span(0, 57),
                name_span: span(9, 13),
            })],
            span: span(0, 57),
        };

        let builtins = BuiltinTable::m1().with_test_builtin(
            "_test_takes_nothing",
            vec![Type::Nothing],
            Type::Nothing,
        );

        let (_, diag_bucket) = check(&module, &builtins);
        let diags: Vec<_> = diag_bucket.into_iter().collect();

        assert!(!diags.is_empty(), "Type mismatch must produce a diagnostic");
        assert!(!diags[0].what.is_empty(), "what must not be empty");
        assert!(
            !diags[0].what_instead.is_empty(),
            "what_instead must not be empty"
        );
        assert!(!diags[0].why.is_empty(), "why must not be empty");
        assert!(
            diags[0].what.contains("nothing") || diags[0].what.contains("string"),
            "Type-mismatch diagnostic must mention the types, got: {}",
            diags[0].what
        );
    }
}
