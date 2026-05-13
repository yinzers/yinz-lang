use ynz_ast::nodes::{
    BinOpKind, CallExpr, Expr, FunctionDecl, Item, Module, Stmt, Type as AstType, UnaryOpKind,
};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{
    intrinsics::PrimitiveIntrinsicTable,
    scope::{Scope, ScopeEntry},
    types::{type_name, Type},
};

/// The type-annotated view of a module.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedModule {
    pub module: Module,
    /// Per-expression types keyed by `(span.start, span.end)`.
    ///
    /// Keying by start alone causes collisions when a BinOp's span.start
    /// equals its leftmost child's span.start — the parent overwrites the child.
    /// The full `(start, end)` pair is unique per expression node.
    pub expr_types: std::collections::HashMap<(usize, usize), Type>,
}

/// Run the M2 type checker.
pub fn check(
    module: &Module,
    intrinsics: &PrimitiveIntrinsicTable,
) -> (TypedModule, DiagnosticBucket) {
    let mut checker = Checker {
        intrinsics,
        expr_types: std::collections::HashMap::new(),
        diags: DiagnosticBucket::new(),
        scope: Scope::new(),
    };
    checker.check_module(module);
    let typed = TypedModule {
        module: module.clone(),
        expr_types: checker.expr_types,
    };
    (typed, checker.diags)
}

struct Checker<'b> {
    intrinsics: &'b PrimitiveIntrinsicTable,
    expr_types: std::collections::HashMap<(usize, usize), Type>,
    diags: DiagnosticBucket,
    scope: Scope,
}

impl<'b> Checker<'b> {

    fn check_module(&mut self, module: &Module) {
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
        if f.name == "main" {
            match &f.return_type {
                AstType::Nothing => {}
                AstType::Error => {}
                other => {
                    self.diags.push(Diagnostic::error(
                        f.span.clone(),
                        format!(
                            "`main` must return `nothing`, but this says it returns `{}`.",
                            ast_type_display(other)
                        ),
                        "Change the return type to `nothing`: `function main() -> nothing`",
                        "`main` is the entry point of the program. It does not return a value — \
                         it runs and then the program ends.",
                    ));
                }
            }
        }

        if f.return_type == AstType::Error || body_has_error_node(&f.body.stmts) {
            return;
        }

        self.scope.push();
        self.check_stmts(&f.body.stmts);
        self.scope.pop();
    }


    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(expr) => {
                    self.infer_expr(expr, None);
                }
                Stmt::Let {
                    is_const,
                    name,
                    name_span,
                    ty,
                    value,
                    span: _,
                } => {
                    self.check_let(*is_const, name, name_span, ty.as_ref(), value);
                }
                Stmt::Assign {
                    target,
                    target_span,
                    value,
                    span: _,
                } => {
                    self.check_assign(target, target_span, value);
                }
                // M3 control flow — full typeck implemented in Phase 3.
                Stmt::If { .. }
                | Stmt::Match { .. }
                | Stmt::While { .. }
                | Stmt::For { .. }
                | Stmt::Return { .. } => {}
            }
        }
    }

    fn check_let(
        &mut self,
        is_const: bool,
        name: &str,
        name_span: &SourceSpan,
        annotation: Option<&AstType>,
        value: &Expr,
    ) {
        let annotated_ty = annotation.map(|t| self.ast_type_to_type(t));

        // Infer value type, passing the annotation as a literal-retyping hint.
        let value_ty = self.infer_expr(value, annotated_ty.as_ref());

        let binding_ty = if let Some(ann_ty) = &annotated_ty {
            if value_ty == Type::Error || *ann_ty == Type::Error {
                Type::Error
            } else if *ann_ty != value_ty {
                self.diags.push(Diagnostic::error(
                    value.span().clone(),
                    format!(
                        "This value is `{}`, but `{}` is declared as `{}`.",
                        type_name(&value_ty),
                        name,
                        type_name(ann_ty)
                    ),
                    format!(
                        "Either change the annotation to `{}`, or use a different value.",
                        type_name(&value_ty)
                    ),
                    "The value on the right side must match the type annotation on the left.",
                ));
                Type::Error
            } else {
                ann_ty.clone()
            }
        } else {
            value_ty
        };

        self.scope.insert(
            name.to_string(),
            ScopeEntry {
                ty: binding_ty,
                is_const,
                defined_at: name_span.clone(),
            },
        );
    }

    fn check_assign(&mut self, target: &str, target_span: &SourceSpan, value: &Expr) {
        let value_ty = self.infer_expr(value, None);

        match self.scope.lookup(target) {
            None => {
                let names = self.scope.all_names();
                let suggestion = find_closest_name(target, &names);
                let what_instead = match suggestion {
                    Some(close) => format!("Did you mean `{close}`?"),
                    None => format!("Declare it first: `let {target} = ...`"),
                };
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!("`{target}` is not defined."),
                    what_instead,
                    "You can only assign to variables that have been declared with `let`.",
                ));
            }
            Some(entry) if entry.is_const => {
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!(
                        "`{target}` cannot be changed — it was declared with `const`."
                    ),
                    format!(
                        "Change `const {target} = ...` to `let {target} = ...` if you need to reassign it."
                    ),
                    "`const` declares a value that never changes. Use `let` when the value needs to be updated.",
                ));
            }
            Some(entry) => {
                let bound_ty = entry.ty.clone();
                if value_ty != Type::Error && value_ty != bound_ty {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "Cannot store a `{}` in `{}` — it holds `{}`.",
                            type_name(&value_ty),
                            target,
                            type_name(&bound_ty)
                        ),
                        format!("The value must be a `{}`.", type_name(&bound_ty)),
                        format!(
                            "`{target}` was declared as `{}`. Storing a `{}` in it would change its type.",
                            type_name(&bound_ty),
                            type_name(&value_ty)
                        ),
                    ));
                }
            }
        }
    }


    /// Infer the type of `expr`.
    ///
    /// `hint` is an optional expected type passed from a `let` annotation.
    /// It only affects literal expressions (`IntLit`, `NumberLit`) — it does
    /// not change how compound expressions like `BinOp` are inferred.
    fn infer_expr(&mut self, expr: &Expr, hint: Option<&Type>) -> Type {
        let ty = match expr {
            Expr::StringLit(_, _) => Type::String,
            Expr::Ident(name, span) => self.resolve_ident(name, span),
            Expr::Call(call) => self.check_call(call),
            Expr::Error(span) => {
                self.expr_types.insert((span.start, span.end), Type::Error);
                return Type::Error;
            }


            // `IntLit` infers as `int` unless the binding context says `number` or `float`.
            Expr::IntLit(_, _) => match hint {
                Some(Type::Number { precision: 34 }) => Type::Number { precision: 34 },
                Some(Type::Float) => Type::Float,
                _ => Type::Int,
            },

            // `NumberLit` infers as `number` unless the binding context says `float`.
            Expr::NumberLit(_, _) => match hint {
                Some(Type::Float) => Type::Float,
                _ => Type::Number { precision: 34 },
            },

            Expr::BoolLit(_, _) => Type::Bool,


            Expr::BinOp { op, lhs, rhs, span } => {
                let lhs_ty = self.infer_expr(lhs, None);
                let rhs_ty = self.infer_expr(rhs, None);
                self.check_binop(op, &lhs_ty, &rhs_ty, span)
            }

            Expr::UnaryOp { op, operand, span } => {
                let operand_ty = self.infer_expr(operand, None);
                self.check_unaryop(op, &operand_ty, span)
            }


            Expr::MethodCall {
                receiver,
                method,
                method_span,
                args,
                ..
            } => {
                let receiver_ty = self.infer_expr(receiver, None);
                for arg in args.iter() {
                    self.infer_expr(arg, None);
                }
                self.check_method_call(&receiver_ty, method, method_span)
            }
        };

        self.expr_types.insert((expr.span().start, expr.span().end), ty.clone());
        ty
    }


    fn resolve_ident(&mut self, name: &str, span: &SourceSpan) -> Type {
        if let Some(entry) = self.scope.lookup(name) {
            return entry.ty.clone();
        }

        let names = self.scope.all_names();
        let suggestion = find_closest_name(name, &names);
        let what_instead = match suggestion {
            Some(close) => format!("Did you mean `{close}`?"),
            None => format!("Check the spelling, or declare it: `let {name} = ...`"),
        };

        self.diags.push(Diagnostic::error(
            span.clone(),
            format!("`{name}` is not defined."),
            what_instead,
            "Every name must be declared before it can be used.",
        ));
        Type::Error
    }


    fn check_call(&mut self, call: &CallExpr) -> Type {
        let callee_name = match &call.callee {
            Expr::Ident(name, _) => name.as_str(),
            _ => {
                self.infer_expr(&call.callee, None);
                return Type::Error;
            }
        };

        // Test-only functions (only compiled in test builds).
        #[cfg(test)]
        if let Some(sig) = self.intrinsics.lookup_test_fn(callee_name) {
            let sig = sig.clone();
            return self.check_test_fn_call(call, callee_name, &sig);
        }

        match callee_name {
            "print" => self.check_print_call(call),
            _ => {
                self.diags.push(Diagnostic::error(
                    call.callee.span().clone(),
                    format!("`{callee_name}` is not defined."),
                    format!(
                        "Check the spelling, or define `{callee_name}` as a function."
                    ),
                    "The compiler looks up every name you use. If a name doesn't exist, the program can't run.",
                ));
                Type::Error
            }
        }
    }

    fn check_print_call(&mut self, call: &CallExpr) -> Type {
        if call.args.len() != 1 {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`print` takes 1 argument, but {} were given.",
                    call.args.len()
                ),
                "Call it with one value: `print(value)`",
                "To display multiple values, use multiple `print` calls on separate lines.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Error;
        }

        let arg_ty = self.infer_expr(&call.args[0], None);
        if arg_ty != Type::Error && !self.intrinsics.is_print_type(&arg_ty) {
            self.diags.push(Diagnostic::error(
                call.args[0].span().clone(),
                format!("`print` cannot display a `{}` value directly.", type_name(&arg_ty)),
                "Convert it to a string first with `.toString()`.",
                "`print` works with: int, float, number, bool, and string.",
            ));
            return Type::Error;
        }

        Type::Nothing
    }


    fn check_binop(
        &mut self,
        op: &BinOpKind,
        lhs: &Type,
        rhs: &Type,
        span: &SourceSpan,
    ) -> Type {
        if *lhs == Type::Error || *rhs == Type::Error {
            return Type::Error;
        }

        use BinOpKind::*;
        match op {
            Add | Sub | Mul | Div => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                (Type::Number { .. }, Type::Number { .. }) => {
                    Type::Number { precision: 34 }
                }
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            Rem => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                (Type::Number { .. }, Type::Number { .. }) => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "`%` is not available for `number`.",
                        "Use the `math` module's `.rem()` method (arriving in v0.7).",
                        "Remainder on decimal numbers requires careful rounding semantics \
                         that the `math` module provides.",
                    ));
                    Type::Error
                }
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            Lt | LtEq | Gt | GtEq => match (lhs, rhs) {
                (Type::Int, Type::Int)
                | (Type::Float, Type::Float)
                | (Type::Number { .. }, Type::Number { .. }) => Type::Bool,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            EqEq | NotEq => match (lhs, rhs) {
                (Type::Int, Type::Int)
                | (Type::Float, Type::Float)
                | (Type::Number { .. }, Type::Number { .. })
                | (Type::Bool, Type::Bool)
                | (Type::String, Type::String) => Type::Bool,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            And | Or => match (lhs, rhs) {
                (Type::Bool, Type::Bool) => Type::Bool,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },

            BitAnd | BitOr | BitXor | Shl | Shr => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                _ => {
                    self.emit_binop_mismatch(op, lhs, rhs, span);
                    Type::Error
                }
            },
        }
    }

    fn check_unaryop(
        &mut self,
        op: &UnaryOpKind,
        operand: &Type,
        span: &SourceSpan,
    ) -> Type {
        if *operand == Type::Error {
            return Type::Error;
        }

        match op {
            UnaryOpKind::Neg => match operand {
                Type::Int | Type::Float | Type::Number { .. } => operand.clone(),
                other => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("Unary `-` cannot be used on a `{}` value.", type_name(other)),
                        "Unary `-` only works on `int`, `float`, and `number`.",
                        "Negation flips the sign of a number — it doesn't apply to other types.",
                    ));
                    Type::Error
                }
            },
            UnaryOpKind::Not => match operand {
                Type::Bool => Type::Bool,
                other => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`!` cannot be used on a `{}` value.", type_name(other)),
                        "Use `!` only with `bool` expressions.",
                        "`!` is the boolean NOT operator — it flips `true` to `false` and vice versa.",
                    ));
                    Type::Error
                }
            },
            UnaryOpKind::BitNot => match operand {
                Type::Int => Type::Int,
                other => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`~` cannot be used on a `{}` value.", type_name(other)),
                        "Use `~` only with `int` values.",
                        "`~` flips every bit in the integer — it only makes sense for `int`.",
                    ));
                    Type::Error
                }
            },
        }
    }

    fn check_method_call(
        &mut self,
        receiver_ty: &Type,
        method: &str,
        method_span: &SourceSpan,
    ) -> Type {
        if *receiver_ty == Type::Error {
            return Type::Error;
        }

        match self.intrinsics.lookup_method(receiver_ty, method) {
            Some(ret_ty) => ret_ty,
            None => {
                let available = self.intrinsics.methods_for_type(receiver_ty);
                let what_instead = if available.is_empty() {
                    format!("`{}` has no methods in M2.", type_name(receiver_ty))
                } else {
                    format!(
                        "Available on `{}`: {}",
                        type_name(receiver_ty),
                        available.join(", ")
                    )
                };
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!(
                        "`{}` does not have a method called `{method}`.",
                        type_name(receiver_ty)
                    ),
                    what_instead,
                    "Method calls are checked at compile time. Only the listed methods exist on this type.",
                ));
                Type::Error
            }
        }
    }


    /// Convert a syntactic AST type to the typeck type.
    fn ast_type_to_type(&mut self, ast_ty: &AstType) -> Type {
        match ast_ty {
            AstType::Nothing => Type::Nothing,
            AstType::Int => Type::Int,
            AstType::Float => Type::Float,
            AstType::Number { precision } => Type::Number {
                precision: *precision,
            },
            AstType::Bool => Type::Bool,
            AstType::Error => Type::Error,
            AstType::Named(n, _) if n == "string" => Type::String,
            AstType::Named(n, span) => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{n}` is not a known type."),
                    "Use a built-in type: `int`, `float`, `number`, `bool`, or `string`.",
                    "Custom types are defined with the `type` keyword, available in M4.",
                ));
                Type::Error
            }
            // Range is an internal typeck type — the parser never produces it as an annotation.
            AstType::Range { .. } => Type::Error,
        }
    }

    fn emit_binop_mismatch(
        &mut self,
        op: &BinOpKind,
        lhs: &Type,
        rhs: &Type,
        span: &SourceSpan,
    ) {
        let what = format!(
            "`{}` cannot be used with `{}` and `{}`.",
            binop_display(op),
            type_name(lhs),
            type_name(rhs)
        );
        let what_instead = suggest_conversion(lhs, rhs);
        self.diags.push(Diagnostic::error(
            span.clone(),
            what,
            what_instead,
            "Yinz does not convert between types automatically. Both sides of an expression must have the same type.",
        ));
    }


    #[cfg(test)]
    fn check_test_fn_call(
        &mut self,
        call: &CallExpr,
        name: &str,
        sig: &crate::intrinsics::FreeFnSig,
    ) -> Type {
        if call.args.len() != sig.params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{name}` takes {} argument(s), but {} were given.",
                    sig.params.len(),
                    call.args.len()
                ),
                format!(
                    "Call it with {} argument(s).",
                    sig.params.len()
                ),
                "Every function call must match the number of arguments the function expects.",
            ));
            return Type::Error;
        }
        for (i, (arg, expected)) in call.args.iter().zip(&sig.params).enumerate() {
            let actual = self.infer_expr(arg, None);
            if actual != *expected && actual != Type::Error {
                self.diags.push(Diagnostic::error(
                    arg.span().clone(),
                    format!(
                        "Argument {} to `{name}` should be `{}`, but got `{}`.",
                        i + 1,
                        type_name(expected),
                        type_name(&actual)
                    ),
                    format!("Pass a `{}` here.", type_name(expected)),
                    format!(
                        "`{name}` expects `{}` in this position.",
                        type_name(expected)
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
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_has_error(value),
        // M3 control flow — conservative: don't skip bodies that contain these
        Stmt::If { .. } | Stmt::Match { .. } | Stmt::While { .. } | Stmt::For { .. } | Stmt::Return { .. } => false,
    })
}

fn expr_has_error(expr: &Expr) -> bool {
    match expr {
        Expr::Error(_) => true,
        Expr::Call(c) => expr_has_error(&c.callee) || c.args.iter().any(expr_has_error),
        Expr::BinOp { lhs, rhs, .. } => expr_has_error(lhs) || expr_has_error(rhs),
        Expr::UnaryOp { operand, .. } => expr_has_error(operand),
        Expr::MethodCall { receiver, args, .. } => {
            expr_has_error(receiver) || args.iter().any(expr_has_error)
        }
        Expr::Ident(_, _)
        | Expr::StringLit(_, _)
        | Expr::IntLit(_, _)
        | Expr::NumberLit(_, _)
        | Expr::BoolLit(_, _) => false,
    }
}

fn ast_type_display(t: &AstType) -> &'static str {
    match t {
        AstType::Nothing => "nothing",
        AstType::Named(_, _) => "named type",
        AstType::Error => "unknown",
        AstType::Int => "int",
        AstType::Float => "float",
        AstType::Number { .. } => "number",
        AstType::Bool => "bool",
        AstType::Range { .. } => "range",
    }
}

fn binop_display(op: &BinOpKind) -> &'static str {
    use BinOpKind::*;
    match op {
        Add => "+", Sub => "-", Mul => "*", Div => "/", Rem => "%",
        Lt => "<", LtEq => "<=", Gt => ">", GtEq => ">=",
        EqEq => "==", NotEq => "!=",
        And => "&&", Or => "||",
        BitAnd => "&", BitOr => "|", BitXor => "^",
        Shl => "<<", Shr => ">>",
    }
}

/// Suggest the specific conversion method for a numeric type mismatch.
///
/// Picks the "widening" direction: int → number/float, float ← number.
/// When both directions lose precision (`number` + `float`), lists both options
/// and explains the tradeoff — this is a teaching opportunity.
fn suggest_conversion(lhs: &Type, rhs: &Type) -> String {
    match (lhs, rhs) {
        // int ↔ number: widen the int (no precision loss)
        (Type::Int, Type::Number { .. }) => {
            "Convert the `int` to `number`: `myInt.toNumber() + myNumber`".to_string()
        }
        (Type::Number { .. }, Type::Int) => {
            "Convert the `int` to `number`: `myNumber + myInt.toNumber()`".to_string()
        }
        // int ↔ float: widen the int (no precision loss)
        (Type::Int, Type::Float) => {
            "Convert the `int` to `float`: `myInt.toFloat() + myFloat`".to_string()
        }
        (Type::Float, Type::Int) => {
            "Convert the `int` to `float`: `myFloat + myInt.toFloat()`".to_string()
        }
        // number ↔ float: both lose precision — show both options
        (Type::Number { .. }, Type::Float) => {
            "Option A: `myNumber.toFloat() + myFloat` (converts decimal to binary — may change the value). \
             Option B: `myNumber + myFloat.toNumber()` (converts binary to decimal — may change the value). \
             Pick based on which type is most precise for your use case."
                .to_string()
        }
        (Type::Float, Type::Number { .. }) => {
            "Option A: `myFloat + myNumber.toFloat()` (converts decimal to binary — may change the value). \
             Option B: `myFloat.toNumber() + myNumber` (converts binary to decimal — may change the value). \
             Pick based on which type is most precise for your use case."
                .to_string()
        }
        // Non-numeric mismatch: generic suggestion
        _ => "Make both sides the same type before combining them.".to_string(),
    }
}

/// Simple Levenshtein distance. Returns the edit distance between `a` and `b`.
#[allow(clippy::needless_range_loop)]
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j - 1].min(dp[i - 1][j]).min(dp[i][j - 1])
            };
        }
    }
    dp[m][n]
}

/// Find the closest name in `candidates` to `target`, within a distance of 2.
fn find_closest_name<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .filter_map(|&c| {
            let d = levenshtein(target, c);
            if d <= 2 { Some((d, c)) } else { None }
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}


#[cfg(test)]
mod tests {
    use ynz_ast::nodes::{Block, CallExpr, Expr, FunctionDecl, Item, Module, Stmt, Type as AstType};
    use ynz_diagnostics::SourceSpan;

    use super::*;

    fn span(start: usize, end: usize) -> SourceSpan {
        SourceSpan::new("test.ynz", start, end)
    }

    #[test]
    fn type_mismatch_produces_three_part_diagnostic() {
        // WHY: this is the load-bearing test for the type-mismatch code path.
        // The test uses a test-only intrinsic to avoid needing full M2 types.
        let module = Module {
            items: vec![Item::Function(FunctionDecl {
                name: "main".into(),
                params: vec![],
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

        let intrinsics = PrimitiveIntrinsicTable::m2().with_test_intrinsic(
            "_test_takes_nothing",
            vec![Type::Nothing],
            Type::Nothing,
        );

        let (_, diag_bucket) = check(&module, &intrinsics);
        let diags: Vec<_> = diag_bucket.into_iter().collect();

        assert!(!diags.is_empty(), "Type mismatch must produce a diagnostic");
        assert!(!diags[0].what.is_empty(), "what must not be empty");
        assert!(!diags[0].what_instead.is_empty(), "what_instead must not be empty");
        assert!(!diags[0].why.is_empty(), "why must not be empty");
        assert!(
            diags[0].what.contains("nothing") || diags[0].what.contains("string"),
            "Type-mismatch diagnostic must mention the types, got: {}",
            diags[0].what
        );
    }
}
