use ynz_ast::nodes::{
    BinOpKind, Block, CallExpr, Expr, FunctionDecl, Item, MatchArm, MatchPatternKind, Module,
    PostfixOpKind, Stmt, StructLitField, Type as AstType, UnaryOpKind,
};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{
    intrinsics::PrimitiveIntrinsicTable,
    return_paths::analyze_return_paths,
    scope::{Scope, ScopeEntry},
    shapes::ShapeTable,
    signatures::SignatureTable,
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

/// Run the M4 type checker over all function bodies.
///
/// Depends on both `sig_table` (function signatures) and `shape_table`
/// (shape declarations and their field layouts) produced by the pre-passes.
pub fn check(
    module: &Module,
    sig_table: &SignatureTable,
    shape_table: &ShapeTable,
    intrinsics: &PrimitiveIntrinsicTable,
) -> (TypedModule, DiagnosticBucket) {
    let mut checker = Checker {
        intrinsics,
        sig_table,
        shape_table,
        expr_types: std::collections::HashMap::new(),
        diags: DiagnosticBucket::new(),
        scope: Scope::new(),
        current_fn_ret: Type::Nothing,
        current_shape: None,
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
    sig_table: &'b SignatureTable,
    shape_table: &'b ShapeTable,
    expr_types: std::collections::HashMap<(usize, usize), Type>,
    diags: DiagnosticBucket,
    scope: Scope,
    /// Return type of the function currently being checked.
    current_fn_ret: Type,
    /// Name of the shape whose method we're currently checking (for `self`/`Self` resolution
    /// and hidden-field visibility). `None` when checking a free function.
    current_shape: Option<String>,
}

impl<'b> Checker<'b> {

    fn check_module(&mut self, module: &Module) {
        // `main` existence and signature are validated in `collect_signatures`.
        // Body checking just iterates all functions with the signature table available.
        // P3b: verify follows contracts after both tables are available.
        self.check_follows_contracts();
        for item in &module.items {
            match item {
                Item::Function(f) => self.check_function(f),
                Item::ShapeDecl(_) => {}
            }
        }
    }

    fn check_function(&mut self, f: &FunctionDecl) {
        if f.return_type == AstType::Error || body_has_error_node(&f.body.stmts) {
            return;
        }

        let ret_ty = self.ast_type_to_type(&f.return_type);
        self.current_fn_ret = ret_ty.clone();

        self.scope.push();

        // Register parameters. If the first param is named `self` and has a Shape type,
        // record the enclosing shape for hidden-field visibility and Self resolution.
        self.current_shape = None;
        for (i, param) in f.params.iter().enumerate() {
            let param_ty = self.ast_type_to_type(&param.ty);
            if i == 0 && param.name == "self" {
                if let Type::Shape { name } = &param_ty {
                    self.current_shape = Some(name.clone());
                }
            }
            self.scope.insert(
                param.name.clone(),
                ScopeEntry {
                    ty: param_ty,
                    is_const: false,
                    is_param: true,
                    is_loop_var: false,
                    is_consumed: false,
                    defined_at: param.name_span.clone(),
                },
            );
        }

        self.check_stmts(&f.body.stmts);
        self.scope.pop();

        // Return-path analysis for non-nothing functions.
        if ret_ty != Type::Nothing && ret_ty != Type::Error {
            let analysis = analyze_return_paths(&f.body);
            if !analysis.all_paths_return {
                self.diags.push(Diagnostic::error(
                    f.span.clone(),
                    format!(
                        "`{}` must return a `{}` on every path, but some paths fall off the end without returning.",
                        f.name,
                        type_name(&ret_ty)
                    ),
                    "Add `return value` at the end of the function, or add an `else =>` catch-all to any multi-case `if` that needs to return.",
                    "Every path through the function must produce a value. A path that falls off the end produces no value, which is a bug.",
                ));
            }
            for dead_span in analysis.dead_code {
                self.diags.push(Diagnostic::warning(
                    dead_span,
                    "This code will never run.",
                    "Remove the unreachable code, or move the `return` statement after it.",
                    "A `return` statement ends the function immediately. Any code after it in the same block is never reached.",
                ));
            }
        }
    }


    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Expr(expr) => {
                    self.infer_expr(expr, None);
                }
                Stmt::Let { is_const, name, name_span, ty, value, span: _ } => {
                    self.check_let(*is_const, name, name_span, ty.as_ref(), value);
                }
                Stmt::Assign { target, target_span, value, span: _ } => {
                    self.check_assign(target, target_span, value);
                }
                Stmt::If { cond, body, .. } => {
                    self.check_stmt_if(cond, body);
                }
                Stmt::Match { scrutinee, arms, else_arm, .. } => {
                    self.check_stmt_match(scrutinee, arms, else_arm.as_ref());
                }
                Stmt::While { cond, body, .. } => {
                    self.check_stmt_while(cond, body);
                }
                Stmt::For { var, var_span, iter, body, .. } => {
                    self.check_stmt_for(var, var_span, iter, body);
                }
                Stmt::Return { value, span } => {
                    self.check_stmt_return(value.as_ref(), span);
                }
                Stmt::FieldAssign { target, value, span } => {
                    self.check_field_assign(target, value, span);
                }
                Stmt::IndexAssign { .. } => {
                    // M5 P3b implements bracket-sugar desugar to `.set(index, value)`.
                    // P1 ships only the AST shape; the parser does not construct this
                    // variant in M5 P1, so reaching here means an out-of-sequence
                    // change — fall through silently rather than emit a noisy stub.
                }
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
        let value_ty = self.infer_expr(value, annotated_ty.as_ref());

        // Range values cannot be stored in let bindings (M7 deferral).
        if matches!(value_ty, Type::Range { .. }) {
            self.diags.push(Diagnostic::error(
                value.span().clone(),
                "A `range(...)` value cannot be stored in a variable.",
                "Use `range(...)` directly in a `for` loop: `for (i in range(0, 10)) { ... }`",
                "Range values can only appear as the iterable in a `for` loop in M3. Storing, passing, or returning them arrives in v0.1 milestone 7.",
            ));
            self.scope.insert(name.to_string(), ScopeEntry {
                ty: Type::Error,
                is_const,
                is_param: false,
                is_loop_var: false,
                is_consumed: false,
                defined_at: name_span.clone(),
            });
            return;
        }

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
                is_param: false,
                is_loop_var: false,
                is_consumed: false,
                defined_at: name_span.clone(),
            },
        );
    }

    fn check_assign(&mut self, target: &str, target_span: &SourceSpan, value: &Expr) {
        let value_ty = self.infer_expr(value, None);

        match self.scope.lookup(target) {
            None => {
                let mut candidates: Vec<&str> = self.scope.all_names();
                candidates.extend(self.sig_table.all_names());
                let suggestion = find_closest_name(target, &candidates);
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
            Some(entry) if entry.is_param => {
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!("`{target}` is a parameter — parameters cannot be reassigned."),
                    format!("To work with a modified value, declare a new variable: `let my_{target} = {target}`"),
                    "Yinz ownership modifiers that allow parameter mutation (`lend`) arrive in v0.1 milestone 4. Until then, parameters are read-only.",
                ));
            }
            Some(entry) if entry.is_loop_var => {
                self.diags.push(Diagnostic::error(
                    target_span.clone(),
                    format!("`{target}` is the loop variable — it cannot be changed inside the loop body."),
                    format!("Declare a separate variable if you need a counter: `let count = {target}`"),
                    "The loop variable steps through values automatically each iteration. Changing it inside the body would cause confusing behavior.",
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

    fn check_stmt_if(&mut self, cond: &Expr, body: &Block) {
        let cond_ty = self.infer_expr(cond, None);
        if cond_ty != Type::Error && cond_ty != Type::Bool {
            self.diags.push(Diagnostic::error(
                cond.span().clone(),
                format!("The condition of an `if` must be `bool`, but this is `{}`.", type_name(&cond_ty)),
                "Write a comparison that produces `true` or `false`, e.g. `x > 0`.",
                "`if` branches on whether the condition is `true` or `false`. Any other type cannot be used as a condition.",
            ));
        }
        self.scope.push();
        self.check_stmts(&body.stmts);
        self.scope.pop();
    }

    fn check_stmt_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        else_arm: Option<&Block>,
    ) {
        let scrutinee_ty = self.infer_expr(scrutinee, None);

        for arm in arms {
            match &arm.pattern.kind {
                MatchPatternKind::Value(pat_expr) => {
                    let pat_ty = self.infer_expr(pat_expr, Some(&scrutinee_ty));
                    if pat_ty != Type::Error && scrutinee_ty != Type::Error && pat_ty != scrutinee_ty {
                        self.diags.push(Diagnostic::error(
                            pat_expr.span().clone(),
                            format!(
                                "This arm pattern is `{}`, but the matched value is `{}`.",
                                type_name(&pat_ty),
                                type_name(&scrutinee_ty)
                            ),
                            format!("Use a `{}` literal or expression as the pattern.", type_name(&scrutinee_ty)),
                            "Each arm pattern must have the same type as the value being matched.",
                        ));
                    }
                }
                // IsType and Variant: M6 deferrals already emitted by the parser.
                MatchPatternKind::IsType(_) | MatchPatternKind::Variant(_) => {}
            }
            self.scope.push();
            self.check_stmts(&arm.body.stmts);
            self.scope.pop();
        }

        if let Some(else_body) = else_arm {
            self.scope.push();
            self.check_stmts(&else_body.stmts);
            self.scope.pop();
        }
    }

    fn check_stmt_while(&mut self, cond: &Expr, body: &Block) {
        let cond_ty = self.infer_expr(cond, None);
        if cond_ty != Type::Error && cond_ty != Type::Bool {
            self.diags.push(Diagnostic::error(
                cond.span().clone(),
                format!("The condition of a `while` loop must be `bool`, but this is `{}`.", type_name(&cond_ty)),
                "Write a comparison that produces `true` or `false`, e.g. `x > 0`.",
                "`while` loops until the condition becomes `false`. Any other type cannot be used as a condition.",
            ));
        }
        self.scope.push();
        self.check_stmts(&body.stmts);
        self.scope.pop();
    }

    fn check_stmt_for(
        &mut self,
        var: &str,
        var_span: &SourceSpan,
        iter: &Expr,
        body: &Block,
    ) {
        let iter_ty = self.infer_expr(iter, None);

        let elem_ty = match &iter_ty {
            Type::Range { element, .. } => *element.clone(),
            Type::Error => Type::Error,
            other => {
                self.diags.push(Diagnostic::error(
                    iter.span().clone(),
                    format!("`for` loops require `range(start, end)` as the collection in M3, but got `{}`.", type_name(other)),
                    "Use `range(...)`: `for (i in range(0, 10)) { ... }`",
                    "Full iteration over arrays, maps, and custom types arrives in v0.1 milestone 7 with the `Iterable[T]` protocol.",
                ));
                Type::Error
            }
        };

        self.scope.push();
        self.scope.insert(
            var.to_string(),
            ScopeEntry {
                ty: elem_ty,
                is_const: false,
                is_param: false,
                is_loop_var: true,
                is_consumed: false,
                defined_at: var_span.clone(),
            },
        );
        self.check_stmts(&body.stmts);
        self.scope.pop();
    }

    fn check_stmt_return(&mut self, value: Option<&Expr>, span: &SourceSpan) {
        let expected = self.current_fn_ret.clone();
        match (value, &expected) {
            (None, Type::Nothing) => {}
            (None, Type::Error) => {}
            (None, ret) => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`return` without a value, but this function must return `{}`.", type_name(ret)),
                    "Add a return value: `return expr`",
                    "A non-`nothing` function must return a value on every path that exits.",
                ));
            }
            (Some(expr), Type::Nothing) => {
                self.infer_expr(expr, None);
                self.diags.push(Diagnostic::error(
                    expr.span().clone(),
                    "`return` with a value in a `-> nothing` function.",
                    "Remove the value: write `return` with no expression.",
                    "Functions declared `-> nothing` do not produce a value. A `return` inside them ends the function — no value allowed.",
                ));
            }
            (Some(expr), ret) => {
                let val_ty = self.infer_expr(expr, Some(ret));
                if val_ty != Type::Error && *ret != Type::Error && val_ty != *ret {
                    self.diags.push(Diagnostic::error(
                        expr.span().clone(),
                        format!(
                            "`return` produces `{}`, but this function must return `{}`.",
                            type_name(&val_ty),
                            type_name(ret)
                        ),
                        format!("Return a `{}` value instead.", type_name(ret)),
                        format!(
                            "The function's declared return type is `{}`. Every `return` must produce a value of that type.",
                            type_name(ret)
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

            Expr::IntLit(_, _) => match hint {
                Some(Type::Number { precision: 34 }) => Type::Number { precision: 34 },
                Some(Type::Float) => Type::Float,
                _ => Type::Int,
            },

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

            Expr::MethodCall { receiver, method, method_span, args, .. } => {
                let receiver_ty = self.infer_expr(receiver, None);
                // M4 P5: one-arg intrinsic methods (wrapping/saturating arithmetic).
                // Must NOT use `return` here — the match value feeds expr_types.insert below.
                if args.len() == 1 {
                    if let Some((expected_arg_ty, ret_ty)) = self.intrinsics.lookup_method_1arg(&receiver_ty, method) {
                        let expected = expected_arg_ty.clone();
                        let actual = self.infer_expr(&args[0], Some(&expected));
                        if actual != expected && actual != Type::Error {
                            self.diags.push(ynz_diagnostics::Diagnostic::error(
                                args[0].span().clone(),
                                format!("`.{method}()` expects `{}` but got `{}`.", crate::types::type_name(&expected), crate::types::type_name(&actual)),
                                format!("Pass an `{}` value.", crate::types::type_name(&expected)),
                                format!("`.{method}()` is a primitive arithmetic operation that only works on `{}`.", crate::types::type_name(&expected)),
                            ));
                        }
                        ret_ty
                    } else {
                        for arg in args.iter() { self.infer_expr(arg, None); }
                        self.check_method_call(&receiver_ty, method, method_span)
                    }
                } else {
                    for arg in args.iter() { self.infer_expr(arg, None); }
                    self.check_method_call(&receiver_ty, method, method_span)
                }
            }

            Expr::FieldAccess { receiver, field, field_span, .. } => {
                // M4 P5: type-attached constants (e.g. `int.max`, `number.epsilon`).
                // Intercept before inferring receiver type to avoid "undefined `int`" error.
                // Must NOT use `return` here — the match value feeds expr_types.insert below.
                if let Expr::Ident(type_name, _) = receiver.as_ref() {
                    if let Some(const_ty) = type_attached_const_type(type_name, field) {
                        const_ty
                    } else {
                        self.infer_field_access(receiver, field, field_span)
                    }
                } else {
                    self.infer_field_access(receiver, field, field_span)
                }
            }
            Expr::StructLit { fields, span } => {
                self.check_struct_lit(fields, hint, span)
            }
            Expr::PostfixOp { receiver, op, span } => {
                self.check_postfix_op(receiver, op, span)
            }
            Expr::SelfValue { span } => {
                match self.scope.lookup("self") {
                    Some(entry) => entry.ty.clone(),
                    None => {
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            "`self` can only be used inside a function whose first parameter is named `self`.",
                            "Add `share self: ShapeName` as the first parameter of this function.",
                            "`self` refers to the value the function was called on. It must be declared as the first parameter.",
                        ));
                        Type::Error
                    }
                }
            }
            Expr::NoneLit { .. } => {
                // M5 P3b implements `maybe<T>` and `none` typing. P1 ships the AST
                // variant only; the parser does not construct this in M5 P1, so
                // reaching this branch returns Type::Error harmlessly.
                Type::Error
            }
            Expr::IndexAccess { receiver, index, .. } => {
                // M5 P3b implements bracket-sugar desugar to `.get(index)`.
                // P1 ships AST only; still type-check sub-expressions so any
                // future construction-site bug doesn't silently skip them.
                let _ = self.infer_expr(receiver, None);
                let _ = self.infer_expr(index, None);
                Type::Error
            }
        };

        self.expr_types.insert((expr.span().start, expr.span().end), ty.clone());
        ty
    }


    fn resolve_ident(&mut self, name: &str, span: &SourceSpan) -> Type {
        if let Some(entry) = self.scope.lookup(name) {
            if entry.is_consumed {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{name}` was already given away and cannot be used here."),
                    "Create a new value or use `.copy()` before passing if you need it in both places.",
                    "When a function takes ownership of a value, the caller no longer holds it. Using it afterward would be a memory safety violation.",
                ));
                return Type::Error;
            }
            return entry.ty.clone();
        }

        let mut candidates: Vec<&str> = self.scope.all_names();
        candidates.extend(self.sig_table.all_names());
        candidates.extend(["print", "range"]);
        let suggestion = find_closest_name(name, &candidates);
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
            Expr::Ident(name, _) => name.clone(),
            _ => {
                self.infer_expr(&call.callee, None);
                return Type::Error;
            }
        };

        // Test-only functions (only compiled in test builds).
        #[cfg(test)]
        if let Some(sig) = self.intrinsics.lookup_test_fn(&callee_name) {
            let sig = sig.clone();
            return self.check_test_fn_call(call, &callee_name, &sig);
        }

        match callee_name.as_str() {
            "print" => self.check_print_call(call),
            "range" => self.check_range_call(call),
            name => {
                // User-defined function?
                if let Some(sig) = self.sig_table.fns.get(name) {
                    let params = sig.params.clone();
                    let ownerships = sig.param_ownerships.clone();
                    let ret = sig.ret.clone();
                    return self.check_user_fn_call(call, name, &params, &ownerships, ret);
                }
                // Unknown
                let mut candidates: Vec<&str> = self.sig_table.all_names();
                candidates.extend(["print", "range"]);
                let suggestion = find_closest_name(name, &candidates);
                let what_instead = match suggestion {
                    Some(close) => format!("Did you mean `{close}`?"),
                    None => format!("Define `{name}` as a function or check the spelling."),
                };
                self.diags.push(Diagnostic::error(
                    call.callee.span().clone(),
                    format!("`{name}` is not defined."),
                    what_instead,
                    "The compiler looks up every name you call. If a name doesn't exist, the program can't run.",
                ));
                for arg in &call.args { self.infer_expr(arg, None); }
                Type::Error
            }
        }
    }

    fn check_print_call(&mut self, call: &CallExpr) -> Type {
        if call.args.len() != 1 {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!("`print` takes 1 argument, but {} were given.", call.args.len()),
                "Call it with one value: `print(value)`",
                "To display multiple values, use multiple `print` calls on separate lines.",
            ));
            for arg in &call.args { self.infer_expr(arg, None); }
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

    fn check_range_call(&mut self, call: &CallExpr) -> Type {
        match call.args.len() {
            1 | 2 => {
                for (i, arg) in call.args.iter().enumerate() {
                    let ty = self.infer_expr(arg, Some(&Type::Int));
                    if ty != Type::Int && ty != Type::Error {
                        self.diags.push(Diagnostic::error(
                            arg.span().clone(),
                            format!("Argument {} of `range` must be `int`, but got `{}`.", i + 1, type_name(&ty)),
                            "Pass an `int` value, e.g. `range(0, 10)`.",
                            "`range` produces integer sequences — its start and end must both be `int`.",
                        ));
                    }
                }
                Type::Range { element: Box::new(Type::Int), end_inclusive: false }
            }
            n => {
                self.diags.push(Diagnostic::error(
                    call.span.clone(),
                    format!("`range` takes 1 or 2 arguments, but {} were given.", n),
                    "Use `range(end)` for 0..end or `range(start, end)` for start..end.",
                    "`range(end)` counts from 0 up to (but not including) end. `range(start, end)` starts at a specific value.",
                ));
                for arg in &call.args { self.infer_expr(arg, None); }
                Type::Error
            }
        }
    }

    fn check_user_fn_call(
        &mut self,
        call: &CallExpr,
        name: &str,
        params: &[(String, Type)],
        ownerships: &[Option<ynz_ast::nodes::OwnershipModifier>],
        ret: Type,
    ) -> Type {
        if call.args.len() != params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{name}` takes {} argument(s), but {} were given.",
                    params.len(),
                    call.args.len()
                ),
                format!("Call it with {} argument(s).", params.len()),
                "Every function call must match the number of arguments the function declares.",
            ));
            for arg in &call.args { self.infer_expr(arg, None); }
            return Type::Error;
        }
        for (i, (arg, (_, expected_ty))) in call.args.iter().zip(params.iter()).enumerate() {
            let ownership = ownerships.get(i).and_then(|o| o.as_ref());
            let actual_ty = self.infer_expr(arg, Some(expected_ty));

            // Ownership enforcement on direct identifier arguments.
            if let Some(binding_name) = simple_ident_name(arg) {
                match ownership {
                    Some(ynz_ast::nodes::OwnershipModifier::Give) => {
                        if let Some(entry) = self.scope.lookup(binding_name) {
                            if entry.is_const {
                                self.diags.push(Diagnostic::error(
                                    arg.span().clone(),
                                    format!("`{binding_name}` is `const` and cannot be given away."),
                                    format!("Declare `{binding_name}` with `let` if you need to transfer ownership."),
                                    "`const` bindings are fully read-only — the compiler cannot transfer ownership of a value that may not change.",
                                ));
                            } else if !entry.is_consumed {
                                self.scope.consume(binding_name);
                            }
                        }
                    }
                    Some(ynz_ast::nodes::OwnershipModifier::Lend) => {
                        if let Some(entry) = self.scope.lookup(binding_name) {
                            if entry.is_const {
                                self.diags.push(Diagnostic::error(
                                    arg.span().clone(),
                                    format!("`{binding_name}` is `const` — `{name}` needs to mutate it but `const` blocks mutation."),
                                    format!("Declare `{binding_name}` with `let` if you need `{name}` to modify it."),
                                    "`const` bindings cannot be lent for mutation. The `lend` modifier means the function will write to the value.",
                                ));
                            }
                        }
                    }
                    _ => {} // share or unspecified: no restrictions
                }
            }

            if matches!(actual_ty, Type::Range { .. }) {
                self.diags.push(Diagnostic::error(
                    arg.span().clone(),
                    "A `range(...)` value cannot be passed as a function argument.",
                    "Use `range(...)` directly in a `for` loop instead.",
                    "Range values can only appear as the iterable in a `for` loop in M3. Passing them arrives in v0.1 milestone 7.",
                ));
            } else if actual_ty != Type::Error && actual_ty != *expected_ty {
                self.diags.push(Diagnostic::error(
                    arg.span().clone(),
                    format!(
                        "This argument is `{}`, but `{name}` expects `{}` here.",
                        type_name(&actual_ty),
                        type_name(expected_ty)
                    ),
                    format!("Pass a `{}` value.", type_name(expected_ty)),
                    format!(
                        "`{name}` declared this parameter as `{}`. Passing a `{}` would be a type mismatch.",
                        type_name(expected_ty),
                        type_name(&actual_ty)
                    ),
                ));
            }
        }
        // Reject Range return values (shouldn't be in sig_table, but guard anyway)
        if matches!(ret, Type::Range { .. }) {
            return Type::Error;
        }
        ret
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
                (Type::Number { .. }, Type::Number { .. }) => Type::Number { precision: 34 },
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
                        "Remainder on decimal numbers requires careful rounding semantics that the `math` module provides.",
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

    fn check_unaryop(&mut self, op: &UnaryOpKind, operand: &Type, span: &SourceSpan) -> Type {
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

        // Primitive intrinsic methods (M2/M3 — toString, toFloat, etc.)
        if let Some(ret_ty) = self.intrinsics.lookup_method(receiver_ty, method) {
            return ret_ty;
        }

        // Shape or dynamic receiver — try UFCS.
        if let Type::Dynamic { contract } = receiver_ty {
            // Dynamic dispatch: look up the method on the contract shape's sigs.
            if let Some(shape_def) = self.shape_table.get(contract) {
                if let Some(sig) = shape_def.contract_sigs.iter().find(|s| s.name == method) {
                    return sig.ret_ty.clone();
                }
            }
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("Contract `{contract}` does not declare a method `{method}`."),
                format!("Add `{method}(share self) -> ...` to the `shape {contract}` body."),
                "Dynamic dispatch only routes calls to methods declared in the contract shape's body.",
            ));
            return Type::Error;
        }
        if let Type::Shape { name } = receiver_ty {
            if let Some(sig) = self.sig_table.fns.get(method) {
                // Check that first param type matches receiver
                if let Some((_, first_ty)) = sig.params.first() {
                    if first_ty == receiver_ty || *first_ty == Type::Error {
                        // Note: receiver ownership check for UFCS is limited here —
                        // full receiver tracking requires the call expression context.
                        // The call site's check_user_fn_call handles arg 0's ownership
                        // when called as a free function; UFCS receiver is checked
                        // via the MethodCall path which doesn't have the arg list here.
                        return sig.ret.clone();
                    }
                }
                // Function exists but first param doesn't match
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("No function `{method}` takes a `{name}` as its first argument."),
                    format!("Define `function {method}(share self: {name}) -> ...` to call it as `value.{method}()`."),
                    "In Yinz, `value.method()` is sugar for `method(value)` — the function's first parameter must match the receiver's type.",
                ));
                return Type::Error;
            }
            // No function named `method` at all
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("No function `{method}` is defined for `{name}` values."),
                format!("Define `function {method}(share self: {name}) -> ...` then call it as `value.{method}()`."),
                "In Yinz, `value.method()` is sugar for `method(value)` (UFCS). Both call forms work — define the function first.",
            ));
            return Type::Error;
        }

        // Primitive type with unknown method
        let available = self.intrinsics.methods_for_type(receiver_ty);
        let what_instead = if available.is_empty() {
            format!("`{}` has no built-in methods.", type_name(receiver_ty))
        } else {
            format!(
                "Available on `{}`: {}",
                type_name(receiver_ty),
                available.join(", ")
            )
        };
        self.diags.push(Diagnostic::error(
            method_span.clone(),
            format!("`{}` does not have a method called `{method}`.", type_name(receiver_ty)),
            what_instead,
            "Method calls are checked at compile time. Only the listed methods exist on this type.",
        ));
        Type::Error
    }


    fn ast_type_to_type(&mut self, ast_ty: &AstType) -> Type {
        match ast_ty {
            AstType::Nothing => Type::Nothing,
            AstType::Int => Type::Int,
            AstType::Float => Type::Float,
            AstType::Number { precision } => Type::Number { precision: *precision },
            AstType::Bool => Type::Bool,
            AstType::Error => Type::Error,
            AstType::Named(n, _) if n == "string" => Type::String,
            AstType::Named(n, _) if self.shape_table.contains(n) => {
                Type::Shape { name: n.clone() }
            }
            AstType::Named(n, span) => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{n}` is not a known type."),
                    "Use a built-in type (`int`, `float`, `number`, `bool`, `string`) or a `shape` name defined in this file.",
                    "Types must be declared before use. If `{n}` is a shape, make sure the `shape {n} {{ ... }}` declaration is in this file.",
                ));
                Type::Error
            }
            AstType::Range { .. } => Type::Error,
            AstType::SelfType { span } => {
                match &self.current_shape {
                    Some(name) => Type::Shape { name: name.clone() },
                    None => {
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            "`Self` can only be used inside a function that operates on a shape.",
                            "Use the concrete shape name instead, e.g. `Player`.",
                            "`Self` refers to the type of the enclosing shape — it only makes sense inside functions with a `self` receiver parameter.",
                        ));
                        Type::Error
                    }
                }
            }
            AstType::Dynamic { contract, span } => {
                if self.shape_table.contains(contract) {
                    Type::Dynamic { contract: contract.clone() }
                } else {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`{contract}` is not a known shape — cannot use it as a `dynamic` contract."),
                        format!("Declare `shape {contract} {{ ... }}` with bare method signatures first."),
                        "`dynamic Foo` requires `Foo` to be a contract shape with bare method signature declarations.",
                    ));
                    Type::Error
                }
            }
            // M5 P3a / P3b implement these (generics engine + built-in collections).
            // P1 ships AST scaffolding only; the parser does not construct these in
            // M5 P1, so reaching here returns Type::Error.
            AstType::TypeParam { .. }
            | AstType::Generic { .. }
            | AstType::Maybe { .. } => Type::Error,
        }
    }

    fn emit_binop_mismatch(&mut self, op: &BinOpKind, lhs: &Type, rhs: &Type, span: &SourceSpan) {
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
    fn check_test_fn_call(&mut self, call: &CallExpr, name: &str, sig: &crate::intrinsics::FreeFnSig) -> Type {
        if call.args.len() != sig.params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!("`{name}` takes {} argument(s), but {} were given.", sig.params.len(), call.args.len()),
                format!("Call it with {} argument(s).", sig.params.len()),
                "Every function call must match the number of arguments the function expects.",
            ));
            return Type::Error;
        }
        for (i, (arg, expected)) in call.args.iter().zip(&sig.params).enumerate() {
            let actual = self.infer_expr(arg, None);
            if actual != *expected && actual != Type::Error {
                self.diags.push(Diagnostic::error(
                    arg.span().clone(),
                    format!("Argument {} to `{name}` should be `{}`, but got `{}`.", i + 1, type_name(expected), type_name(&actual)),
                    format!("Pass a `{}` here.", type_name(expected)),
                    format!("`{name}` expects `{}` in this position.", type_name(expected)),
                ));
            }
        }
        sig.ret.clone()
    }

    // ── M4 P3b: inheritance + follows contract verification ──────────────────

    /// Verify every `shape X follows Y` declaration.
    ///
    /// For each contract sig `method(self, params...) -> Ret` in Y, there must be
    /// a standalone function `method` in `sig_table` whose first param is
    /// `Type::Shape { name: X }` and whose return type matches.
    fn check_follows_contracts(&mut self) {
        // Collect (shape_name, follows_list) to avoid borrow conflicts.
        let follows_list: Vec<(String, Vec<String>, Vec<crate::shapes::ContractSigDef>)> = self
            .shape_table
            .shapes
            .iter()
            .filter(|(_, def)| !def.follows.is_empty())
            .map(|(name, def)| (name.clone(), def.follows.clone(), def.contract_sigs.clone()))
            .collect();

        for (shape_name, contracts, _own_sigs) in &follows_list {
            let shape_ty = Type::Shape { name: shape_name.clone() };
            for contract_name in contracts {
                let Some(contract_def) = self.shape_table.get(contract_name) else {
                    continue; // already errored in collect_shapes
                };
                let contract_sigs = contract_def.contract_sigs.clone();
                let shape_def_span = self.shape_table.get(shape_name)
                    .map(|s| s.defined_at.clone())
                    .unwrap_or_else(|| SourceSpan::new("", 0, 0));

                for sig in &contract_sigs {
                    match self.sig_table.fns.get(&sig.name) {
                        None => {
                            self.diags.push(Diagnostic::error(
                                shape_def_span.clone(),
                                format!("`{shape_name}` follows `{contract_name}` but is missing function `{}`.", sig.name),
                                format!("Add `function {}(share self: {shape_name}) -> ...` to this file.", sig.name),
                                format!("`{contract_name}` requires a function named `{}` — define it as a standalone function whose first parameter is `self: {shape_name}`.", sig.name),
                            ));
                        }
                        Some(fn_sig) => {
                            // Check first param matches the implementing shape.
                            match fn_sig.params.first() {
                                Some((_, first_ty)) if *first_ty == shape_ty => {
                                    // Return type must match.
                                    if fn_sig.ret != sig.ret_ty
                                        && fn_sig.ret != Type::Error
                                        && sig.ret_ty != Type::Error
                                    {
                                        self.diags.push(Diagnostic::error(
                                            shape_def_span.clone(),
                                            format!("Function `{}` for `{shape_name}` returns `{}`, but `{contract_name}` requires `{}`.", sig.name, type_name(&fn_sig.ret), type_name(&sig.ret_ty)),
                                            format!("Change the return type to `{}` to satisfy `{contract_name}`.", type_name(&sig.ret_ty)),
                                            "Functions that satisfy a contract must return exactly the type the contract declares.",
                                        ));
                                    }
                                }
                                Some((_, first_ty)) => {
                                    self.diags.push(Diagnostic::error(
                                        shape_def_span.clone(),
                                        format!("Function `{}` cannot satisfy `{contract_name}` for `{shape_name}` — its first parameter is `{}`, not `{shape_name}`.", sig.name, type_name(first_ty)),
                                        format!("Change the first parameter to `share self: {shape_name}`."),
                                        "Contract satisfaction requires the function's first parameter to be the implementing shape.",
                                    ));
                                }
                                None => {
                                    self.diags.push(Diagnostic::error(
                                        shape_def_span.clone(),
                                        format!("Function `{}` has no parameters but `{contract_name}` requires a `self: {shape_name}` receiver.", sig.name),
                                        format!("Add `share self: {shape_name}` as the first parameter."),
                                        "Contract functions must have the implementing shape as their first parameter.",
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ── M4 P3a: shape type-checking ──────────────────────────────────────────

    /// Infer the type of a field access `receiver.field`.
    fn infer_field_access(&mut self, receiver: &Expr, field: &str, field_span: &SourceSpan) -> Type {
        let receiver_ty = self.infer_expr(receiver, None);
        // Dynamic dispatch: treat the contract shape as the lookup target.
        let shape_name = match &receiver_ty {
            Type::Shape { name } => name.clone(),
            Type::Dynamic { contract } => contract.clone(),
            Type::Error => return Type::Error,
            other => {
                self.diags.push(Diagnostic::error(
                    field_span.clone(),
                    format!("`{}` values do not have fields.", type_name(other)),
                    "Field access is only available on shape values.",
                    "Shapes are the only Yinz types with named fields. Primitive types like `int` and `string` use methods instead.",
                ));
                return Type::Error;
            }
        };
        let shape_name = shape_name.clone();
        let Some(shape_def) = self.shape_table.get(&shape_name) else {
            return Type::Error; // shape not in table — already errored at pre-pass
        };
        let Some(field_def) = shape_def.field(field) else {
            let available: Vec<&str> = shape_def.fields.iter().map(|f| f.name.as_str()).collect();
            let suggestion = find_closest_name(field, &available);
            let what_instead = match suggestion {
                Some(close) => format!("Did you mean `{close}`?"),
                None => format!("`{shape_name}` has these fields: {}", available.join(", ")),
            };
            self.diags.push(Diagnostic::error(
                field_span.clone(),
                format!("`{shape_name}` does not have a field called `{field}`.", ),
                what_instead,
                "Field names must match exactly what was declared in the `shape` body.",
            ));
            return Type::Error;
        };
        // Hidden field visibility: only accessible inside the declaring shape's functions.
        if field_def.is_hidden {
            let inside_shape = self.current_shape.as_deref() == Some(&shape_name);
            if !inside_shape {
                self.diags.push(Diagnostic::error(
                    field_span.clone(),
                    format!("`{field}` is a hidden field of `{shape_name}` and cannot be read here."),
                    format!("Move this access inside a function whose first parameter is `self: {shape_name}`."),
                    "Hidden fields are only accessible to functions that explicitly operate on that shape — they cannot be read by outside code.",
                ));
                return Type::Error;
            }
        }
        field_def.ty.clone()
    }

    /// Type-check a struct literal `{ name: "x", health: 100 }` against the hint type.
    fn check_struct_lit(&mut self, fields: &[StructLitField], hint: Option<&Type>, span: &SourceSpan) -> Type {
        let shape_name = match hint {
            Some(Type::Shape { name }) => name.clone(),
            Some(other) if *other != Type::Error => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("A struct literal `{{ ... }}` cannot produce a `{}` value.", type_name(other)),
                    "Annotate the binding with a `shape` name: `let p: Player = { ... }`",
                    "Struct literals create shape values — the annotation must name a shape, not a primitive type.",
                ));
                for f in fields { self.infer_expr(&f.value, None); }
                return Type::Error;
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    "This struct literal needs a type annotation to know which shape to create.",
                    "Add a type annotation: `let p: Player = { ... }`",
                    "Struct literals are anonymous — the `shape` type comes from the surrounding annotation. Without it, the compiler cannot check the field names or types.",
                ));
                for f in fields { self.infer_expr(&f.value, None); }
                return Type::Error;
            }
        };

        let Some(shape_def) = self.shape_table.get(&shape_name) else {
            for f in fields { self.infer_expr(&f.value, None); }
            return Type::Error;
        };

        // base shapes cannot be instantiated
        if shape_def.is_base {
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("`{shape_name}` is a `base shape` and cannot be constructed directly."),
                format!("Create a shape that extends `{shape_name}`, then construct that instead."),
                "`base shape` declarations are meant to be extended — they provide shared fields for child shapes but cannot be instantiated on their own.",
            ));
            for f in fields { self.infer_expr(&f.value, None); }
            return Type::Error;
        }

        // Check every required (non-hidden, no-default) field is present.
        for shape_field in &shape_def.fields.clone() {
            if shape_field.is_hidden {
                continue; // hidden fields are not provided at construction
            }
            let provided = fields.iter().any(|f| f.name == shape_field.name);
            if !provided {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Missing field `{}` in `{shape_name}` construction.", shape_field.name),
                    format!("Add `{}: value` to the struct literal.", shape_field.name),
                    "Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.",
                ));
            }
        }

        // Check each provided field: name must exist and value type must match.
        for lit_field in fields {
            let expected_ty = shape_def.fields.iter()
                .find(|f| f.name == lit_field.name)
                .map(|f| f.ty.clone());
            match expected_ty {
                None => {
                    let available: Vec<&str> = shape_def.fields.iter()
                        .filter(|f| !f.is_hidden)
                        .map(|f| f.name.as_str())
                        .collect();
                    let suggestion = find_closest_name(&lit_field.name, &available);
                    let what_instead = match suggestion {
                        Some(close) => format!("Did you mean `{close}`?"),
                        None => format!("`{shape_name}` has these fields: {}", available.join(", ")),
                    };
                    self.diags.push(Diagnostic::error(
                        lit_field.name_span.clone(),
                        format!("`{shape_name}` does not have a field called `{}`.", lit_field.name),
                        what_instead,
                        "Struct literals can only set fields that are declared on the shape.",
                    ));
                    self.infer_expr(&lit_field.value, None);
                }
                Some(expected) => {
                    let actual = self.infer_expr(&lit_field.value, Some(&expected));
                    if actual != Type::Error && expected != Type::Error && actual != expected {
                        self.diags.push(Diagnostic::error(
                            lit_field.name_span.clone(),
                            format!("Field `{}` expects `{}`, but got `{}`.", lit_field.name, type_name(&expected), type_name(&actual)),
                            format!("Pass a `{}` value for `{}`.", type_name(&expected), lit_field.name),
                            format!("`{shape_name}.{}` was declared as `{}`.", lit_field.name, type_name(&expected)),
                        ));
                    }
                }
            }
        }

        Type::Shape { name: shape_name }
    }

    /// Type-check a field assignment `target.field = value`.
    fn check_field_assign(&mut self, target: &Expr, value: &Expr, span: &SourceSpan) {
        let Expr::FieldAccess { receiver, field, field_span, .. } = target else {
            // Parser only produces FieldAssign when target is a FieldAccess, but be defensive.
            self.infer_expr(target, None);
            self.infer_expr(value, None);
            return;
        };

        // The receiver must be a mutable (let-bound, non-const) shape value.
        // Walk the receiver chain to find the root binding and check it.
        if let Some(root_name) = root_binding_name(receiver) {
            if let Some(entry) = self.scope.lookup(root_name) {
                if entry.is_const {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("`{root_name}` is `const` and its fields cannot be changed."),
                        format!("Declare it with `let` instead: `let {root_name}: ShapeType = {{ ... }}`"),
                        "`const` bindings are fully read-only — no reassignment, no field mutation. Use `let` for values that need to change.",
                    ));
                    self.infer_expr(value, None);
                    return;
                }
            }
        }

        // Resolve the field and check the value type.
        let field_ty = self.infer_field_access(receiver, field, field_span);
        let value_ty = self.infer_expr(value, Some(&field_ty));

        if field_ty != Type::Error && value_ty != Type::Error && field_ty != value_ty {
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("Cannot assign `{}` to field `{field}` which has type `{}`.", type_name(&value_ty), type_name(&field_ty)),
                format!("Pass a `{}` value.", type_name(&field_ty)),
                format!("The field `{field}` was declared as `{}`.", type_name(&field_ty)),
            ));
        }
    }

    /// Type-check a dot-postfix body operation (`.copy()` or `.freeze()`).
    fn check_postfix_op(&mut self, receiver: &Expr, op: &PostfixOpKind, span: &SourceSpan) -> Type {
        let receiver_ty = self.infer_expr(receiver, None);
        match op {
            PostfixOpKind::Copy => {
                // P3c will enforce trivially-copyable requirement.
                // P3a: just return the receiver type.
                if receiver_ty == Type::Error { return Type::Error; }
                receiver_ty
            }
            PostfixOpKind::Freeze => {
                // P3c will flip the binding's mutability.
                // P3a: no-op semantically, returns nothing.
                let _ = span;
                Type::Nothing
            }
        }
    }
}


/// Return the typeck `Type` for a type-attached constant like `int.max` or `number.epsilon`.
///
/// Returns `None` if the (type_name, const_name) pair is not a known constant.
pub fn type_attached_const_type(type_name: &str, const_name: &str) -> Option<Type> {
    match (type_name, const_name) {
        ("int",    "max") | ("int",    "min") => Some(Type::Int),
        ("float",  "max") | ("float",  "min") | ("float",  "epsilon") => Some(Type::Float),
        ("number", "max") | ("number", "min") | ("number", "epsilon") => Some(Type::Number { precision: 34 }),
        _ => None,
    }
}

fn body_has_error_node(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Expr(e) => expr_has_error(e),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_has_error(value),
        Stmt::If { cond, body, .. } => expr_has_error(cond) || body_has_error_node(&body.stmts),
        Stmt::Match { scrutinee, arms, else_arm, .. } => {
            expr_has_error(scrutinee)
                || arms.iter().any(|arm| body_has_error_node(&arm.body.stmts))
                || else_arm.as_ref().is_some_and(|b| body_has_error_node(&b.stmts))
        }
        Stmt::While { cond, body, .. } | Stmt::For { iter: cond, body, .. } => {
            expr_has_error(cond) || body_has_error_node(&body.stmts)
        }
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_has_error),
        // M4 P3a: field assignment — not yet type-checked.
        Stmt::FieldAssign { target, value, .. } => expr_has_error(target) || expr_has_error(value),
        // M5 P1: index assignment — parser does not construct in P1; reached only if
        // out-of-sequence change happens. Walk sub-expressions for safety.
        Stmt::IndexAssign { receiver, index, value, .. } => {
            expr_has_error(receiver) || expr_has_error(index) || expr_has_error(value)
        }
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
        | Expr::BoolLit(_, _)
        | Expr::SelfValue { .. }
        | Expr::NoneLit { .. } => false,
        // M4 P3a: shape expressions — propagate error check into sub-expressions.
        Expr::FieldAccess { receiver, .. } | Expr::PostfixOp { receiver, .. } => expr_has_error(receiver),
        Expr::StructLit { fields, .. } => fields.iter().any(|f| expr_has_error(&f.value)),
        // M5 P1: bracket-index — propagate error check into receiver + index.
        Expr::IndexAccess { receiver, index, .. } => expr_has_error(receiver) || expr_has_error(index),
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

fn suggest_conversion(lhs: &Type, rhs: &Type) -> String {
    match (lhs, rhs) {
        (Type::Int, Type::Number { .. }) => {
            "Convert the `int` to `number`: `myInt.toNumber() + myNumber`".to_string()
        }
        (Type::Number { .. }, Type::Int) => {
            "Convert the `int` to `number`: `myNumber + myInt.toNumber()`".to_string()
        }
        (Type::Int, Type::Float) => {
            "Convert the `int` to `float`: `myInt.toFloat() + myFloat`".to_string()
        }
        (Type::Float, Type::Int) => {
            "Convert the `int` to `float`: `myFloat + myInt.toFloat()`".to_string()
        }
        (Type::Number { .. }, Type::Float) | (Type::Float, Type::Number { .. }) => {
            "Converting between `number` and `float` can lose precision either way. \
             Use `.toFloat()` to convert `number` → `float` (loses decimal precision), \
             or `.toNumber()` to convert `float` → `number` (may lose binary precision)."
                .to_string()
        }
        _ => "Check that both sides have the same type.".to_string(),
    }
}

/// Find the closest name using Levenshtein distance — for "did you mean?" suggestions.
pub fn find_closest_name<'a>(target: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let threshold = match target.len() {
        0..=2 => 0,
        3..=4 => 1,
        _ => 2,
    };
    candidates
        .iter()
        .filter_map(|&c| {
            let dist = levenshtein(target, c);
            if dist <= threshold { Some((dist, c)) } else { None }
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    (0..=m).for_each(|i| dp[i][0] = i);
    (0..=n).for_each(|j| dp[0][j] = j);
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

/// Return the identifier name if `expr` is a bare `Ident` or `SelfValue`.
///
/// Used for ownership checking: ownership enforcement only applies to direct
/// binding references, not to computed expressions like `foo()` or `a + b`.
fn simple_ident_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name.as_str()),
        Expr::SelfValue { .. } => Some("self"),
        _ => None,
    }
}

/// Walk a field-access chain to find the root binding name.
///
/// `player.inner.health` → `Some("player")`
/// `self.field` → `Some("self")`
/// Anything not rooted in a simple identifier → `None`.
fn root_binding_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(name, _) => Some(name.as_str()),
        Expr::SelfValue { .. } => Some("self"),
        Expr::FieldAccess { receiver, .. } => root_binding_name(receiver),
        _ => None,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use ynz_ast::nodes::{Block, CallExpr, Expr, FunctionDecl, Item, Module, Stmt};
    use ynz_diagnostics::SourceSpan;

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
                generics: vec![],
                params: vec![],
                return_type: AstType::Nothing,
                body: Block {
                    stmts: vec![Stmt::Expr(Expr::Call(Box::new(CallExpr {
                        callee: Expr::Ident("_test_takes_nothing".into(), span(29, 48)),
                        type_args: None,
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

        let intrinsics = PrimitiveIntrinsicTable::m3().with_test_intrinsic(
            "_test_takes_nothing",
            vec![Type::Nothing],
            Type::Nothing,
        );
        let shape_table = crate::shapes::collect_shapes(&module, &mut DiagnosticBucket::new());
        let sig_table = crate::signatures::collect_signatures(&module, &mut DiagnosticBucket::new(), &shape_table);
        let (_, diags) = check(&module, &sig_table, &shape_table, &intrinsics);
        let diags: Vec<_> = diags.into_iter().collect();
        assert_eq!(diags.len(), 1, "Expected 1 type-mismatch diagnostic, got: {diags:#?}");

        let d = &diags[0];
        assert!(!d.what.is_empty(), "what must be non-empty");
        assert!(!d.what_instead.is_empty(), "what_instead must be non-empty");
        assert!(!d.why.is_empty(), "why must be non-empty");
    }
}
