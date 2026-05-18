use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{
    BinOpKind, Block, CallExpr, Expr, FunctionDecl, Item, MatchArm, MatchPatternKind, Module,
    PostfixOpKind, Stmt, StructLitField, Type as AstType, UnaryOpKind,
};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{
    builtins::{
        array_method_return, fixed_method_return, map_method_return, maybe_method_return,
        sensitive_method_return, string_method_return, STRING_METHODS,
    },
    generics::{
        apply_substitution, unify_param, GenericFnSig, GenericFnTable, GenericShapeTable,
        MonoSignature, MonomorphizationTable, Substitution,
    },
    intrinsics::PrimitiveIntrinsicTable,
    options_table::{collect_options, OptionsTable},
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

/// Run the M5 type checker over all function bodies.
///
/// Returns the typed module, monomorphization table, and accumulated diagnostics.
pub fn check(
    module: &Module,
    sig_table: &SignatureTable,
    shape_table: &ShapeTable,
    generic_fn_table: &GenericFnTable,
    generic_shape_table: &GenericShapeTable,
    intrinsics: &PrimitiveIntrinsicTable,
) -> (TypedModule, MonomorphizationTable, DiagnosticBucket) {
    let mut diags = DiagnosticBucket::new();
    let options_table = collect_options(module, &mut diags);

    let mut checker = Checker {
        intrinsics,
        sig_table,
        shape_table,
        generic_fn_table,
        generic_shape_table,
        options_table: &options_table,
        expr_types: HashMap::new(),
        diags,
        scope: Scope::new(),
        current_fn_ret: Type::Nothing,
        current_shape: None,
        type_param_scope: HashMap::new(),
        mono_table: MonomorphizationTable::default(),
        maybe_non_none: HashSet::new(),
        union_narrowed: HashMap::new(),
        union_aliases: collect_union_aliases(module, shape_table),
        errors_success_narrowed: HashSet::new(),
        errors_consumed: HashSet::new(),
        current_fn_errors_capable: false,
    };
    checker.check_module(module);
    let typed = TypedModule {
        module: module.clone(),
        expr_types: checker.expr_types,
    };
    (typed, checker.mono_table, checker.diags)
}

struct Checker<'b> {
    intrinsics: &'b PrimitiveIntrinsicTable,
    sig_table: &'b SignatureTable,
    shape_table: &'b ShapeTable,
    generic_fn_table: &'b GenericFnTable,
    generic_shape_table: &'b GenericShapeTable,
    options_table: &'b OptionsTable,
    expr_types: HashMap<(usize, usize), Type>,
    diags: DiagnosticBucket,
    scope: Scope,
    /// Return type of the function currently being checked.
    current_fn_ret: Type,
    /// Name of the shape whose method we're currently checking (for `self`/`Self` resolution
    /// and hidden-field visibility). `None` when checking a free function.
    current_shape: Option<String>,
    /// Type parameters in scope for the function currently being checked.
    /// Maps type-param name → unit; presence means the name resolves to `TypeParam`.
    type_param_scope: HashMap<String, ()>,
    /// Accumulated monomorphization entries for all generic call sites in this module.
    mono_table: MonomorphizationTable,
    /// Flow-sensitive tracking: binding names known to be non-none inside an `.exists()` guard.
    maybe_non_none: HashSet<String>,
    /// M6: binding names narrowed to a specific union variant inside an `is`-arm body.
    /// Maps binding name → narrowed type (the specific variant type).
    union_narrowed: HashMap<String, Type>,
    /// M6: named union type aliases from `shape Shape = Circle | Square` declarations.
    /// Maps alias name → resolved union type. Populated before check_module runs.
    union_aliases: HashMap<String, Type>,

    // ── M7 P3a: errors-capable flow tracking ─────────────────────────────────
    /// Flow-sensitive: binding names known to be in the success state after a
    /// `.failed() == false` check or after auto-propagation fired. These bindings
    /// have narrowed from `ErrorsCapable<T>` to `T`.
    errors_success_narrowed: HashSet<String>,
    /// Bindings that have been consumed by auto-propagation at first use or by
    /// a `.failed()` check. After consumption, calling `.failed()` is a compile
    /// error ("check-after-use").
    errors_consumed: HashSet<String>,
    /// Whether the function currently being checked is itself errors_capable.
    current_fn_errors_capable: bool,
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
                // M6: options declarations are validated and registered by collect_options()
                // which runs before check_module. Nothing to do here.
                Item::OptionsDecl(_) => {}
                // M8: import/export/const declarations — validated by collect_exports/imports
                // which runs before check_module. Function-body typeck is unaffected.
                Item::ImportDecl(_) | Item::ConstDecl(_) | Item::ReExport(_) => {}
            }
        }
    }

    fn check_function(&mut self, f: &FunctionDecl) {
        if f.return_type == AstType::Error || body_has_error_node(&f.body.stmts) {
            return;
        }

        if !f.generics.is_empty() {
            self.check_generic_function_body(f);
            return;
        }

        // ast_type_to_type resolves ErrorCapable → ErrorsCapable { inner } already.
        let ret_ty = self.ast_type_to_type(&f.return_type);
        self.current_fn_ret = ret_ty.clone();

        // M7 P3a: track whether the current function is errors-capable.
        self.current_fn_errors_capable = f.errors_capable;
        self.errors_success_narrowed.clear();
        self.errors_consumed.clear();

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
        // For ErrorsCapable functions, report the inner type name (not "string errors")
        // so the error message reads naturally. Also skip analysis for -> nothing errors
        // (inner is Nothing) since implicit fallthrough is valid for nothing-returning fns.
        let ret_ty_for_analysis = match &ret_ty {
            Type::ErrorsCapable { inner } => *inner.clone(),
            other => other.clone(),
        };
        if ret_ty != Type::Nothing && ret_ty != Type::Error && ret_ty_for_analysis != Type::Nothing
        {
            let analysis = analyze_return_paths(&f.body);
            if !analysis.all_paths_return {
                self.diags.push(Diagnostic::error(
                    f.span.clone(),
                    format!(
                        "`{}` must return a `{}` on every path, but some paths fall off the end without returning.",
                        f.name,
                        type_name(&ret_ty_for_analysis)
                    ),
                    "Add `return value` at the end of the function, or add an `else =>` default arm to any multi-case `if` that needs to return.",
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

    /// Type-check the body of a generic function under its type-parameter scope.
    ///
    /// Generic bodies are checked with TypeParam types in scope. No return-path
    /// analysis at P3a — the generic signature is trusted; codegen verifies per-instantiation.
    fn check_generic_function_body(&mut self, f: &FunctionDecl) {
        // Push type param names into the type_param_scope.
        for gp in &f.generics {
            self.type_param_scope.insert(gp.name.clone(), ());
        }

        let ret_ty = self.ast_type_to_type(&f.return_type);
        self.current_fn_ret = ret_ty;
        self.current_shape = None;

        self.scope.push();
        for param in &f.params {
            let param_ty = self.ast_type_to_type(&param.ty);
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

        // Clear type params when done with this function.
        for gp in &f.generics {
            self.type_param_scope.remove(&gp.name);
        }
    }

    fn check_stmts(&mut self, stmts: &[Stmt]) {
        // Collect early-return narrowing facts: when an `if (!m.exists()) { return }` or
        // `if (!m.exists()) { panic(...) }` is detected, mark `m` as non-none for all
        // subsequent statements in this block.
        let mut early_return_narrowed: Vec<String> = Vec::new();

        for stmt in stmts {
            // Apply any early-return narrowing facts from previous `if (!x.exists()) { return }`.
            for name in &early_return_narrowed {
                self.maybe_non_none.insert(name.clone());
            }

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
                    // Reassignment invalidates early-return narrowing for the target binding.
                    early_return_narrowed.retain(|n| n != target);
                    self.check_assign(target, target_span, value);
                }
                Stmt::If { cond, body, .. } => {
                    // Detect early-return narrowing: `if (!m.exists()) { <recognized-exit> }`.
                    // After this if, `m` is proven to exist for the rest of the block.
                    let negated_exists = self.extract_negated_exists_binding(cond);
                    let body_always_exits = analyze_return_paths(body).all_paths_return;
                    if !negated_exists.is_empty() && body_always_exits {
                        for name in &negated_exists {
                            early_return_narrowed.push(name.clone());
                        }
                    }
                    self.check_stmt_if(cond, body);
                }
                Stmt::Match {
                    scrutinee,
                    arms,
                    else_arm,
                    ..
                } => {
                    self.check_stmt_match(scrutinee, arms, else_arm.as_ref());
                }
                Stmt::While { cond, body, .. } => {
                    self.check_stmt_while(cond, body);
                }
                Stmt::For {
                    var,
                    var_span,
                    iter,
                    body,
                    ..
                } => {
                    self.check_stmt_for(var, var_span, iter, body);
                }
                Stmt::Return { value, span } => {
                    self.check_stmt_return(value.as_ref(), span);
                }
                Stmt::FieldAssign {
                    target,
                    value,
                    span,
                } => {
                    self.check_field_assign(target, value, span);
                }
                Stmt::IndexAssign {
                    receiver,
                    index,
                    value,
                    span,
                } => {
                    self.check_index_assign(receiver, index, value, span);
                }
            }
        }
        // Clean up early-return narrowing facts when leaving the block.
        for name in &early_return_narrowed {
            self.maybe_non_none.remove(name.as_str());
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

        // M7 P3c: range values are first-class — no restriction on storage.
        // (The M3 restriction is removed here.)

        let binding_ty = if let Some(ann_ty) = &annotated_ty {
            if value_ty == Type::Error || *ann_ty == Type::Error {
                Type::Error
            } else if !types_compatible(ann_ty, &value_ty) {
                self.diags.push(Diagnostic::error(
                    value.span().clone(),
                    format!(
                        "This value is `{}`, but `{}` is declared as `{}`.",
                        type_name(&value_ty),
                        name,
                        type_name(ann_ty)
                    ),
                    format!(
                        "Change the annotation to `{}`, or use a different value.",
                        type_name(&value_ty)
                    ),
                    "The value on the right side must match the type annotation on the left.",
                ));
                Type::Error
            } else if matches!(ann_ty, Type::Union { .. }) {
                // M6: for union type annotations, use the declared union type as the binding type,
                // not the concrete variant. `let s: Shape = circle` → s is Shape, not Circle.
                ann_ty.clone()
            } else {
                // Use the value_ty to preserve size information from ArrayLit inference.
                value_ty
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

        // Flow-sensitive narrowing: if condition is `m.exists()`, mark `m` as known-non-none
        // inside the if body so `.value` is allowed without another guard.
        let narrowed = self.extract_exists_binding(cond);
        for name in &narrowed {
            self.maybe_non_none.insert(name.clone());
        }

        // M7 P3a: if condition is `x.failed()`, mark `x` as "consumed by failed check"
        // inside the if body. After the block, `x` is narrowed to success.
        let failed_binding = self.extract_failed_binding(cond);
        for name in &failed_binding {
            self.errors_consumed.insert(name.clone());
        }

        self.scope.push();
        self.check_stmts(&body.stmts);
        self.scope.pop();

        // Remove narrowing flags after the block exits.
        for name in &narrowed {
            self.maybe_non_none.remove(name.as_str());
        }

        // M7 P3a: after `if (x.failed()) { ... }`, narrow `x` to success for subsequent code.
        for name in &failed_binding {
            self.errors_success_narrowed.insert(name.clone());
        }
    }

    /// M7 P3a: extract the binding name from a `.failed()` condition.
    ///
    /// Matches `x.failed()` → `vec!["x"]` so the if-body can use error fields
    /// and subsequent code sees `x` narrowed to its success type.
    fn extract_failed_binding(&self, cond: &Expr) -> Vec<String> {
        if let Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } = cond
        {
            if method == "failed" && args.is_empty() {
                if let Expr::Ident(name, _) = receiver.as_ref() {
                    // Only extract when the binding is actually ErrorsCapable.
                    if let Some(entry) = self.scope.lookup(name) {
                        if matches!(entry.ty, Type::ErrorsCapable { .. }) {
                            return vec![name.clone()];
                        }
                    }
                }
            }
        }
        Vec::new()
    }

    /// Extract the binding name from an `.exists()` condition for flow-sensitive narrowing.
    ///
    /// Matches `m.exists()` → `vec!["m"]` so the if-body can use `m.value`.
    /// Any other form returns an empty vec.
    fn extract_exists_binding(&self, cond: &Expr) -> Vec<String> {
        if let Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } = cond
        {
            if method == "exists" && args.is_empty() {
                if let Expr::Ident(name, _) = receiver.as_ref() {
                    return vec![name.clone()];
                }
            }
        }
        Vec::new()
    }

    /// Extract the binding name from a NEGATED `.exists()` condition for early-return narrowing.
    ///
    /// Matches `!m.exists()` → `vec!["m"]` — after this if-block always returns,
    /// `m` is non-none for the rest of the enclosing block.
    fn extract_negated_exists_binding(&self, cond: &Expr) -> Vec<String> {
        if let Expr::UnaryOp {
            op: ynz_ast::nodes::UnaryOpKind::Not,
            operand,
            ..
        } = cond
        {
            return self.extract_exists_binding(operand);
        }
        Vec::new()
    }

    fn check_stmt_match(&mut self, scrutinee: &Expr, arms: &[MatchArm], else_arm: Option<&Block>) {
        let scrutinee_ty = self.infer_expr(scrutinee, None);

        for arm in arms {
            match &arm.pattern.kind {
                MatchPatternKind::Value(pat_expr) => {
                    let pat_ty = self.infer_expr(pat_expr, Some(&scrutinee_ty));
                    if pat_ty != Type::Error
                        && scrutinee_ty != Type::Error
                        && pat_ty != scrutinee_ty
                    {
                        self.diags.push(Diagnostic::error(
                            pat_expr.span().clone(),
                            format!(
                                "This arm pattern is `{}`, but the matched value is `{}`.",
                                type_name(&pat_ty),
                                type_name(&scrutinee_ty)
                            ),
                            format!(
                                "Use a `{}` literal or expression as the pattern.",
                                type_name(&scrutinee_ty)
                            ),
                            "Each arm pattern must have the same type as the value being matched.",
                        ));
                    }
                }
                // Is: M6 union narrowing — validate variant, narrow inside arm body.
                MatchPatternKind::Is(type_path) => {
                    self.check_is_arm_pattern(&scrutinee_ty, type_path, &arm.pattern.span);
                    // Narrowing: inside this arm's body, the scrutinee binding is narrowed.
                    // We push a scope with the narrowed type for the binding if we can identify it.
                    // (Full binding-name extraction is P3b — basic case: scrutinee is a direct Ident)
                    let narrowed_name = simple_ident_name(scrutinee).map(|s| s.to_string());
                    if let Some(ref name) = narrowed_name {
                        self.union_narrowed.insert(
                            name.clone(),
                            Type::Shape {
                                name: type_path.name.clone(),
                            },
                        );
                    }
                    self.scope.push();
                    self.check_stmts(&arm.body.stmts);
                    self.scope.pop();
                    if let Some(ref name) = narrowed_name {
                        self.union_narrowed.remove(name);
                    }
                    continue; // skip the standard scope push/pop below
                }
                // OptionName: M6 options multi-case arm.
                MatchPatternKind::OptionName(variant_name) => {
                    self.check_option_name_arm(&scrutinee_ty, variant_name, &arm.pattern.span);
                }
            }
            self.scope.push();
            self.check_stmts(&arm.body.stmts);
            self.scope.pop();
        }

        // Exhaustiveness check for options multi-case.
        if let Type::Options { name: opts_name } = &scrutinee_ty {
            if let Some(entry) = self.options_table.get(opts_name) {
                let covered: std::collections::HashSet<&str> = arms
                    .iter()
                    .filter_map(|arm| {
                        if let MatchPatternKind::OptionName(v) = &arm.pattern.kind {
                            Some(v.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                if else_arm.is_none() {
                    let missing: Vec<&str> = entry
                        .variants
                        .iter()
                        .filter(|v| !covered.contains(v.as_str()))
                        .map(String::as_str)
                        .collect();
                    if !missing.is_empty() {
                        self.diags.push(Diagnostic::error(
                            scrutinee.span().clone(),
                            format!(
                                "Non-exhaustive options multi-case — `{}` has {} variants; {} are not handled: {}.",
                                opts_name,
                                entry.variants.len(),
                                missing.len(),
                                missing.join(", ")
                            ),
                            format!("Add the missing arms (e.g. `{} =>`) or add an `else =>` default arm.", missing[0]),
                            "The compiler knows every variant at compile time. A missing arm means some values would silently fall through — likely a bug.",
                        ));
                    }
                }
            }
        }

        // M6: Union exhaustiveness check for `Is` arms.
        if let Type::Union { variants } = &scrutinee_ty {
            let covered: std::collections::HashSet<String> = arms
                .iter()
                .filter_map(|arm| {
                    if let MatchPatternKind::Is(tp) = &arm.pattern.kind {
                        Some(tp.name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if else_arm.is_none() {
                let missing: Vec<String> = variants
                    .iter()
                    .filter_map(|v| {
                        if let Type::Shape { name } = v {
                            if !covered.contains(name) {
                                Some(name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                if !missing.is_empty() {
                    self.diags.push(Diagnostic::error(
                        scrutinee.span().clone(),
                        format!(
                            "Non-exhaustive union multi-case — {} variant{} not handled: {}.",
                            missing.len(),
                            if missing.len() == 1 { " is" } else { "s are" },
                            missing.join(", ")
                        ),
                        format!("Add the missing arms (e.g. `is {} =>`) or add an `else =>` default arm.", missing[0]),
                        "The compiler knows every union variant at compile time. A missing arm means some values silently fall through — likely a bug.",
                    ));
                }
            }
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

    fn check_stmt_for(&mut self, var: &str, var_span: &SourceSpan, iter: &Expr, body: &Block) {
        let iter_ty = self.infer_expr(iter, None);

        // M7 P3c: Iterable<T> protocol dispatch. Each built-in collection type
        // maps to an element type. User shapes are checked for a next() function
        // matching the Iterable<T> contract.
        let elem_ty = match &iter_ty {
            Type::Range { element, .. } => *element.clone(),
            Type::BuiltinArray { elem } => *elem.clone(),
            Type::BuiltinFixed { elem, .. } => *elem.clone(),
            Type::BuiltinMap { key, val } => Type::MapEntry {
                key: key.clone(),
                val: val.clone(),
            },
            // M7 P3c: string iteration yields one code-point string per step.
            Type::String => Type::String,
            // M7 P3c: user shape iteration — requires a standalone next() function
            // whose return type is maybe<T>. The element type T is extracted from it.
            Type::Shape { name } => self.infer_iterable_element_for_shape(name, iter.span()),
            Type::Error => Type::Error,
            other => {
                self.diags.push(Diagnostic::error(
                    iter.span().clone(),
                    format!("`for` loops over `{}` are not supported.", type_name(other)),
                    "Use `range(...)`, iterate over `array<T>`, `fixed<T>`, `map<K, V>`, `string`, or a shape that follows `Iterable<T>`.",
                    "For custom types, define `function next(lend self: YourShape) -> maybe<T>` to make them iterable.",
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

    /// M7 P3c: look up the element type T for iterating over a user-defined shape.
    ///
    /// A shape is iterable if there is a standalone `next` function in the signature
    /// table whose first parameter is `Shape { name }` and whose return type is
    /// `Maybe { inner: T }`. If no such function exists, emits a diagnostic and
    /// returns `Type::Error`.
    fn infer_iterable_element_for_shape(&mut self, shape_name: &str, span: &SourceSpan) -> Type {
        if let Some(sig) = self.sig_table.fns.get("next") {
            let shape_ty = Type::Shape {
                name: shape_name.to_string(),
            };
            if let Some((_, first_ty)) = sig.params.first() {
                if *first_ty == shape_ty {
                    // next() returns maybe<T> — extract T as the element type.
                    return match &sig.ret {
                        Type::Maybe { inner } => *inner.clone(),
                        other => {
                            self.diags.push(Diagnostic::error(
                                span.clone(),
                                format!("`{shape_name}.next()` must return `maybe<T>` to be iterable, but it returns `{}`.", type_name(other)),
                                format!("Change `function next(lend self: {shape_name}) -> {}` to return `maybe<T>` instead.", type_name(other)),
                                "The `Iterable<T>` contract requires `next(lend self) -> maybe<T>`. When the iterator is exhausted, return `none`.",
                            ));
                            Type::Error
                        }
                    };
                }
            }
        }
        self.diags.push(Diagnostic::error(
            span.clone(),
            format!("`{shape_name}` cannot be iterated — it does not follow `Iterable<T>`."),
            format!("Add `function next(lend self: {shape_name}) -> maybe<T>` to make it iterable."),
            "For a `for` loop to work on a custom shape, the shape needs a standalone `next` function returning `maybe<T>`. When the iterator is done, return `none`.",
        ));
        Type::Error
    }

    fn check_stmt_return(&mut self, value: Option<&Expr>, span: &SourceSpan) {
        let expected = self.current_fn_ret.clone();
        match (value, &expected) {
            (None, Type::Nothing) => {}
            (None, Type::Error) => {}
            (None, ret) => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!(
                        "`return` without a value, but this function must return `{}`.",
                        type_name(ret)
                    ),
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
                if val_ty != Type::Error && *ret != Type::Error && !types_compatible(ret, &val_ty) {
                    // M7 P3a: in an errors-capable function, returning the inner success
                    // type is valid (the auto-propagation machinery wraps it at codegen).
                    let compatible = if let Type::ErrorsCapable { inner } = ret {
                        types_compatible(&val_ty, inner)
                    } else if let Type::ErrorsCapable { inner } = &val_ty {
                        // Returning an ErrorsCapable value from an errors function is also valid.
                        self.current_fn_errors_capable && types_compatible(inner, ret)
                    } else {
                        false
                    };
                    if !compatible {
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
                // M8 P6: use the annotated precision when a number annotation is present.
                Some(Type::Number { precision }) => Type::Number { precision: *precision },
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
                // M4 P5: one-arg intrinsic methods (wrapping/saturating arithmetic).
                // Must NOT use `return` here — the match value feeds expr_types.insert below.
                if args.len() == 1 {
                    if let Some((expected_arg_ty, ret_ty)) =
                        self.intrinsics.lookup_method_1arg(&receiver_ty, method)
                    {
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
                        for arg in args.iter() {
                            self.infer_expr(arg, None);
                        }
                        self.check_method_call(&receiver_ty, method, method_span)
                    }
                } else {
                    for arg in args.iter() {
                        self.infer_expr(arg, None);
                    }
                    self.check_method_call(&receiver_ty, method, method_span)
                }
            }

            Expr::FieldAccess {
                receiver,
                field,
                field_span,
                ..
            } => {
                // M4 P5: type-attached constants (e.g. `int.max`, `number.epsilon`).
                // Intercept before inferring receiver type to avoid "undefined `int`" error.
                if let Expr::Ident(type_name_str, _) = receiver.as_ref() {
                    if let Some(const_ty) = type_attached_const_type(type_name_str, field) {
                        const_ty
                    } else if self.options_table.contains(type_name_str) {
                        // M6: OptionsValue — `Status.active` where Status is an options type.
                        self.check_options_value(type_name_str, field, field_span)
                    } else {
                        self.infer_field_access(receiver, field, field_span)
                    }
                } else {
                    self.infer_field_access(receiver, field, field_span)
                }
            }
            Expr::StructLit { fields, span } => self.check_struct_lit(fields, hint, span),
            Expr::PostfixOp { receiver, op, span } => self.check_postfix_op(receiver, op, span),
            Expr::SelfValue { span } => match self.scope.lookup("self") {
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
            },
            Expr::NoneLit { span } => {
                // M7 P3a: if the hint is ErrorsCapable wrapping Maybe, unwrap to the Maybe type.
                let effective_hint = match hint {
                    Some(Type::ErrorsCapable { inner })
                        if matches!(inner.as_ref(), Type::Maybe { .. }) =>
                    {
                        Some(inner.as_ref())
                    }
                    other => other,
                };
                match effective_hint {
                    Some(Type::Maybe { .. }) => effective_hint.unwrap().clone(),
                    // When hint is Type::Error, an upstream annotation error was already emitted —
                    // suppress the cascade by returning Error silently.
                    Some(Type::Error) => Type::Error,
                    None => {
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            "Cannot work out which type `none` should be here.",
                            "Annotate the binding: `let x: maybe<int> = none`.",
                            "`none` is the absent value of `maybe<T>` for some T. The compiler needs the annotation to know which T.",
                        ));
                        Type::Error
                    }
                    Some(other) => {
                        let other_name = type_name(other);
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            format!("`none` cannot be a `{other_name}` value."),
                            "Use `maybe<T>` for optional values: `let x: maybe<int> = none`.",
                            "`none` is only valid as the absent value of `maybe<T>`. It cannot be used where a concrete type is expected.",
                        ));
                        Type::Error
                    }
                }
            }
            Expr::IndexAccess {
                receiver,
                index,
                span,
            } => {
                let recv_ty = self.infer_expr(receiver, None);
                let _idx_ty = self.infer_expr(index, Some(&Type::Int));
                match &recv_ty {
                    Type::BuiltinArray { elem } | Type::BuiltinFixed { elem, .. } => Type::Maybe {
                        inner: elem.clone(),
                    },
                    Type::BuiltinMap { val, .. } => Type::Maybe { inner: val.clone() },
                    // M7 P3b: string bracket access desugars to .get(n) → maybe<string>
                    Type::String => Type::Maybe {
                        inner: Box::new(Type::String),
                    },
                    Type::Error => Type::Error,
                    other => {
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            format!("`{}` does not support bracket access.", type_name(other)),
                            "Bracket access works on `array<T>`, `fixed<T>`, `map<K, V>`, and `string`.",
                            "Use `.get(index)` on built-in collections or access shape fields with dot notation.",
                        ));
                        Type::Error
                    }
                }
            }
            Expr::ArrayLit { elements, span } => self.check_array_lit(elements, hint, span),
            Expr::MapLit { entries, span } => self.check_map_lit(entries, hint, span),
            // M6: `x is Foo` type-narrowing predicate — returns bool.
            Expr::Is {
                expr: inner,
                ty: type_path,
                span,
            } => self.check_is_expr(inner, type_path, span),
            // M7 P3b: interpolated string — validate that each ${...} expression
            // has a stringifiable type. Primitive types are always valid; shapes
            // require a standalone `toString` function.
            // M8 P4: if ANY interpoland is `sensitive`, the result is `sensitive string`.
            Expr::InterpolatedString(parts, _) => {
                let mut has_sensitive = false;
                for part in parts {
                    if let ynz_ast::nodes::StringPart::Expr(e, span) = part {
                        let part_ty = self.infer_expr(e, None);
                        if matches!(&part_ty, Type::Sensitive { .. }) {
                            has_sensitive = true;
                        } else if !is_stringifiable(&part_ty, self.sig_table) {
                            let type_name_str = type_name(&part_ty);
                            self.diags.push(Diagnostic::error(
                                span.clone(),
                                format!("`{type_name_str}` cannot be used inside a string interpolation."),
                                format!(
                                    "Add a `.toString()` method to `{type_name_str}`: \
                                     `function toString(share self: {type_name_str}) -> string {{ ... }}`"
                                ),
                                "String interpolation calls `.toString()` on each `${{}}` expression. \
                                 Primitive types (int, float, bool, string) work automatically. \
                                 Custom shapes need a standalone `toString` function.",
                            ));
                        }
                    }
                }
                if has_sensitive {
                    Type::Sensitive {
                        inner: Box::new(Type::String),
                    }
                } else {
                    Type::String
                }
            }
            // M8 P5: `wait expr` — same type as the inner expression (sequential semantics).
            Expr::Wait(inner, _) => self.infer_expr(inner, hint),
            // M8 P5: `background expr` — must be a function call; return type is Nothing
            // (return value is discarded). Ownership rules enforced in check_stmt.
            Expr::Background(inner, span) => {
                let inner_ty = self.infer_expr(inner, None);
                // background must wrap a function call — enforce this.
                if !matches!(inner.as_ref(), Expr::Call(_) | Expr::MethodCall { .. }) {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        "`background` must be followed by a function call.",
                        "Write `background process(data)` to call `process` in the background.",
                        "`background` schedules a function call to run outside the current scope. \
                         It cannot be applied to non-call expressions.",
                    ));
                }
                let _ = inner_ty;
                Type::Nothing // background discards the return value
            }
        };

        self.expr_types
            .insert((expr.span().start, expr.span().end), ty.clone());
        ty
    }

    fn resolve_ident(&mut self, name: &str, span: &SourceSpan) -> Type {
        // M6: if inside a union `is` arm, the binding may be narrowed to a specific variant.
        if let Some(narrowed_ty) = self.union_narrowed.get(name).cloned() {
            return narrowed_ty;
        }
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

            // M7 P3a: ErrorsCapable binding use handling.
            // Return the ErrorsCapable type as-is so method dispatch (.failed, .or, etc.)
            // can handle it. Auto-propagation fires when the binding is passed to a function
            // expecting the plain success type — that check happens in check_user_fn_call.
            if let Type::ErrorsCapable { inner } = &entry.ty {
                let inner = inner.as_ref().clone();

                // Already narrowed to success type (after .failed() check or prior use) —
                // return the success type directly.
                if self.errors_success_narrowed.contains(name) {
                    return inner;
                }

                if self.current_fn_errors_capable {
                    // Inside an errors function: auto-propagation fires — narrow the
                    // binding to its success type. The compiler will insert early-return-
                    // on-failure IR at P4a; for typeck, just return the inner type.
                    self.errors_success_narrowed.insert(name.to_string());
                    self.errors_consumed.insert(name.to_string());
                    return inner;
                }
                // Outside an errors function: return the full ErrorsCapable type.
                // check_user_fn_call will emit the diagnostic if it's passed as
                // a success-typed argument. Method calls (.failed, .or) are fine.
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
            // M8 P4: `sensitive(value)` constructor — wraps a string in Type::Sensitive.
            "sensitive" => {
                if call.args.len() != 1 {
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!("`sensitive` takes exactly one argument, got {}.", call.args.len()),
                        "Write `sensitive(`my secret value`)`.",
                        "`sensitive` marks a string value as sensitive so it auto-redacts in print output.",
                    ));
                    return Type::Error;
                }
                let arg_ty = self.infer_expr(&call.args[0], None);
                if arg_ty != Type::String && arg_ty != Type::Error {
                    self.diags.push(Diagnostic::error(
                        call.args[0].span().clone(),
                        format!(
                            "`sensitive` only wraps strings, not `{}`.",
                            type_name(&arg_ty)
                        ),
                        "Pass a string value: `sensitive(`my secret`)`.",
                        "Only string values can be marked sensitive in v0.1.",
                    ));
                    return Type::Error;
                }
                Type::Sensitive {
                    inner: Box::new(Type::String),
                }
            }
            name => {
                // Non-generic user-defined function?
                if let Some(sig) = self.sig_table.fns.get(name) {
                    let params = sig.params.clone();
                    let ownerships = sig.param_ownerships.clone();
                    let ret = sig.ret.clone();
                    let result = self.check_user_fn_call(call, name, &params, &ownerships, ret);
                    // M7 P3a: if the called function returns ErrorsCapable, handle context.
                    return self.handle_errors_capable_call_result(result, name, call.span.clone());
                }
                // Generic user-defined function?
                if let Some(sig) = self.generic_fn_table.fns.get(name) {
                    let sig = sig.clone();
                    return self.check_generic_fn_call(call, name, &sig);
                }
                // Unknown
                let mut candidates: Vec<&str> = self.sig_table.all_names();
                candidates.extend(self.generic_fn_table.all_names());
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
                for arg in &call.args {
                    self.infer_expr(arg, None);
                }
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
        // Shapes are printable — the compiler emits a default "ShapeName { field: val, ... }"
        // representation. User-defined toString() can override this.
        // M8 P4: sensitive values are printable (they emit [REDACTED]).
        // M8 P6: all number<N> precisions (including bignum) are printable.
        let is_printable = self.intrinsics.is_print_type(&arg_ty)
            || matches!(
                &arg_ty,
                Type::Shape { .. }
                    | Type::BuiltinArray { .. }
                    | Type::Sensitive { .. }
                    | Type::Number { .. }
            );
        if arg_ty != Type::Error && !is_printable {
            // M7 P3a: give a more helpful diagnostic for ErrorsCapable values.
            if let Type::ErrorsCapable { .. } = &arg_ty {
                self.diags.push(Diagnostic::error(
                    call.args[0].span().clone(),
                    "This function can fail, but the failure is not handled here.",
                    "Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.",
                    "When a function can fail, the failure must be handled somewhere. The compiler enforces this so failures can't silently pass through.",
                ));
            } else {
                let what_instead = match &arg_ty {
                    Type::BuiltinArray { .. } | Type::BuiltinFixed { .. } => {
                        "Loop and print each element: `for (item in collection) { print(item) }`"
                            .to_string()
                    }
                    Type::BuiltinMap { .. } => {
                        "Loop and print each entry: `for ((k, v) in collection) { print(k) }`"
                            .to_string()
                    }
                    _ => "Convert it to a string first with `.toString()`.".to_string(),
                };
                self.diags.push(Diagnostic::error(
                    call.args[0].span().clone(),
                    format!(
                        "`print` cannot display a `{}` value directly.",
                        type_name(&arg_ty)
                    ),
                    what_instead,
                    "`print` works with: int, float, number, bool, string, and any shape.",
                ));
            }
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
                Type::Range {
                    element: Box::new(Type::Int),
                    end_inclusive: false,
                }
            }
            n => {
                self.diags.push(Diagnostic::error(
                    call.span.clone(),
                    format!("`range` takes 1 or 2 arguments, but {} were given.", n),
                    "Use `range(end)` for 0..end or `range(start, end)` for start..end.",
                    "`range(end)` counts from 0 up to (but not including) end. `range(start, end)` starts at a specific value.",
                ));
                for arg in &call.args {
                    self.infer_expr(arg, None);
                }
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
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
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

            // M7 P3c: range values are first-class — can be passed as function arguments.
            if let Type::ErrorsCapable { .. } = &actual_ty {
                // M7 P3a: give a more helpful diagnostic for ErrorsCapable values passed
                // to a function expecting the success type.
                if !matches!(expected_ty, Type::ErrorsCapable { .. }) {
                    self.diags.push(Diagnostic::error(
                        arg.span().clone(),
                        "This function can fail, but the failure is not handled here.",
                        "Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.",
                        "When a function can fail, the failure must be handled somewhere. The compiler enforces this so failures can't silently pass through.",
                    ));
                }
            } else if actual_ty != Type::Error && !types_compatible(expected_ty, &actual_ty) {
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

    fn check_binop(&mut self, op: &BinOpKind, lhs: &Type, rhs: &Type, span: &SourceSpan) -> Type {
        if *lhs == Type::Error || *rhs == Type::Error {
            return Type::Error;
        }

        use BinOpKind::*;
        match op {
            Add | Sub | Mul | Div => match (lhs, rhs) {
                (Type::Int, Type::Int) => Type::Int,
                (Type::Float, Type::Float) => Type::Float,
                // M8 P6: mixed-precision promotion — result precision = max(lhs, rhs).
                (Type::Number { precision: pa }, Type::Number { precision: pb }) => {
                    Type::Number { precision: (*pa).max(*pb) }
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
                // M6: same-options-type comparison is valid.
                (Type::Options { name: a }, Type::Options { name: b }) if a == b => Type::Bool,
                // M6: cross-options-type comparison is a compile error.
                (Type::Options { name: a }, Type::Options { name: b }) => {
                    self.diags.push(Diagnostic::error(
                        span.clone(),
                        format!("Cannot compare `{a}` and `{b}` — they are different options types."),
                        format!("Compare values of the same options type, or convert both to the same type first."),
                        "Comparing values of different options types is almost always a bug — \
                         the tags have no shared meaning between types.",
                    ));
                    Type::Error
                }
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
            UnaryOpKind::Neg => {
                match operand {
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
                }
            }
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

    /// Type-check a call to a generic function, performing type inference and
    /// constraint checking, then recording the instantiation in the MonomorphizationTable.
    fn check_generic_fn_call(&mut self, call: &CallExpr, name: &str, sig: &GenericFnSig) -> Type {
        let non_self_params: Vec<(String, Type)> = sig
            .params
            .iter()
            .filter(|(p, _)| p != "self")
            .cloned()
            .collect();

        if call.args.len() != non_self_params.len() {
            self.diags.push(Diagnostic::error(
                call.span.clone(),
                format!(
                    "`{name}` takes {} argument(s), but {} were given.",
                    non_self_params.len(),
                    call.args.len()
                ),
                format!("Call it with {} argument(s).", non_self_params.len()),
                "Every function call must match the number of arguments the function declares.",
            ));
            for arg in &call.args {
                self.infer_expr(arg, None);
            }
            return Type::Error;
        }

        let mut subst: Substitution = HashMap::new();

        // Explicit type args (e.g. `foo<int>(x)`) seed the substitution directly.
        if let Some(type_args) = &call.type_args {
            for (tp_name, ast_ty) in sig.type_params.iter().zip(type_args.iter()) {
                let concrete = self.ast_type_to_type(ast_ty);
                subst.insert(tp_name.clone(), concrete);
            }
        }

        // Infer remaining type params from argument types; enforce ownership rules.
        // Ownerships aligned with non_self_params (skip the `self` entry if present).
        let skip = sig.params.len() - non_self_params.len();
        let non_self_ownerships: Vec<Option<ynz_ast::nodes::OwnershipModifier>> =
            sig.param_ownerships.iter().skip(skip).cloned().collect();
        let mut arg_types = Vec::new();
        for (i, (arg, (_, param_ty))) in call.args.iter().zip(non_self_params.iter()).enumerate() {
            let actual = self.infer_expr(arg, None);
            arg_types.push(actual.clone());
            if actual != Type::Error {
                let _ = unify_param(param_ty, &actual, &mut subst);
            }
            // Ownership enforcement (mirrors check_user_fn_call).
            let ownership = non_self_ownerships.get(i).and_then(|o| o.as_ref());
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
                                    "`const` bindings cannot be lent for mutation.",
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Verify all type params were resolved — emit one consolidated error if multiple are missing.
        let unresolved: Vec<&String> = sig
            .type_params
            .iter()
            .filter(|tp| !subst.contains_key(*tp))
            .collect();
        if !unresolved.is_empty() {
            match unresolved.len() {
                1 => {
                    let tp_name = unresolved[0];
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!("Cannot work out the type parameter `{tp_name}` for function `{name}` — pass a value or annotate explicitly."),
                        format!("Examples: `{name}(5)` (T = int) or `{name}<int>()`"),
                        "Yinz infers type parameters from the argument types. If there are no arguments, specify the type explicitly.",
                    ));
                }
                n => {
                    let list = unresolved
                        .iter()
                        .map(|tp| format!("`{tp}`"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.diags.push(Diagnostic::error(
                        call.span.clone(),
                        format!("{n} type parameters could not be resolved for `{name}`: {list}."),
                        format!("Annotate the call explicitly: `{name}<Type1, Type2>(...)` or pass typed arguments."),
                        "Yinz infers type parameters from the argument types. If there are no arguments, specify all types explicitly.",
                    ));
                }
            }
            return Type::Error;
        }

        // Verify follows constraints.
        for (tp_name, contracts) in &sig.constraints {
            let Some(concrete_ty) = subst.get(tp_name) else {
                continue;
            };
            let concrete_ty = concrete_ty.clone();
            for contract_name in contracts {
                match &concrete_ty {
                    Type::Shape { name: shape_name } => {
                        let satisfies = self
                            .shape_table
                            .get(shape_name)
                            .map(|def| def.follows.contains(contract_name))
                            .unwrap_or(false);
                        if !satisfies {
                            self.diags.push(Diagnostic::error(
                                call.span.clone(),
                                format!("Type `{shape_name}` does not follow contract `{contract_name}`."),
                                format!("To use `{shape_name}` here, add `follows {contract_name}` to its declaration AND implement the required methods."),
                                format!("`{name}<{tp_name} follows {contract_name}>` requires the concrete type to satisfy the `{contract_name}` contract."),
                            ));
                            return Type::Error;
                        }
                    }
                    other => {
                        self.diags.push(Diagnostic::error(
                            call.span.clone(),
                            format!("Type `{}` does not follow contract `{contract_name}` — only shapes can follow contracts.", type_name(other)),
                            format!("Use a shape type for `{tp_name}`, or remove the `follows {contract_name}` constraint."),
                            "`follows` constraints can only be satisfied by user-defined shapes.",
                        ));
                        return Type::Error;
                    }
                }
            }
        }

        // Compute concrete return type.
        let concrete_ret = apply_substitution(&sig.ret, &subst);

        // Record the instantiation.
        let concrete_type_args: Vec<Type> = sig
            .type_params
            .iter()
            .map(|tp| subst.get(tp).cloned().unwrap_or(Type::Error))
            .collect();
        let concrete_params: Vec<Type> = non_self_params
            .iter()
            .map(|(_, ty)| apply_substitution(ty, &subst))
            .collect();
        self.mono_table.record(
            name.to_string(),
            concrete_type_args,
            MonoSignature {
                param_types: concrete_params,
                ret_type: concrete_ret.clone(),
            },
        );

        concrete_ret
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

        // M8 P4: sensitive type method dispatch.
        if let Type::Sensitive { inner } = receiver_ty {
            let inner = inner.as_ref().clone();
            return sensitive_method_return(method, &inner, method_span, &mut self.diags);
        }

        // M6: reject `.toInt()` on bool — no silent 0/1 coercion.
        if *receiver_ty == Type::Bool && method == "toInt" {
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                "`.toInt()` is not available on `bool`.",
                "Use an `if` expression instead: `if (b) { 1 } else { 0 }`",
                "Automatic bool-to-int coercion is a common source of bugs. \
                 Yinz requires an explicit conversion.",
            ));
            return Type::Error;
        }

        // Primitive intrinsic methods (M2/M3 — toString, toFloat, etc.)
        if let Some(ret_ty) = self.intrinsics.lookup_method(receiver_ty, method) {
            return ret_ty;
        }

        // M5 P3b: built-in collection method dispatch.
        if let Type::BuiltinArray { elem } = receiver_ty {
            let elem = elem.as_ref().clone();
            return if let Some(ret) = array_method_return(method, &elem) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`array<{}>` does not have a method called `{method}`.", type_name(&elem)),
                    "Available methods: add, remove, get, set, count, first, last, contains, sort, filter, find, map, concat.",
                    "These are the built-in methods on `array<T>`. Check the spelling.",
                ));
                Type::Error
            };
        }
        if let Type::BuiltinFixed { elem, .. } = receiver_ty {
            let elem = elem.as_ref().clone();
            return if let Some(ret) = fixed_method_return(method, &elem) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`fixed<{}>` does not have a method called `{method}`.", type_name(&elem)),
                    "Available methods: get, set, count, first, last, contains, sort, filter, find, concat.",
                    "`fixed<T>` is a size-locked array — it does not have `.add()` or `.remove()`. Use `array<T>` for growable collections.",
                ));
                Type::Error
            };
        }
        if let Type::Maybe { inner } = receiver_ty {
            let inner = inner.as_ref().clone();
            return if let Some(ret) = maybe_method_return(method, &inner) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`maybe<{}>` does not have a method called `{method}`.", type_name(&inner)),
                    "Available methods: exists(), or(default).",
                    "`maybe<T>` values can only be checked with `.exists()` or given a default with `.or(default)`. Access the value with `.value` (after checking `.exists()`).",
                ));
                Type::Error
            };
        }
        if let Type::BuiltinMap { key, val } = receiver_ty {
            let key = key.as_ref().clone();
            let val = val.as_ref().clone();
            return if let Some(ret) = map_method_return(method, &key, &val) {
                ret
            } else {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!(
                        "`map<{}, {}>` does not have a method called `{method}`.",
                        type_name(&key),
                        type_name(&val)
                    ),
                    "Available methods: get, set, has, remove, count, keys, values, entries.",
                    "Check the spelling. Use `m[key]` for reads and `m[key] = value` for writes.",
                ));
                Type::Error
            };
        }
        if let Type::MapEntry { key, val } = receiver_ty {
            let key = key.as_ref().clone();
            let val = val.as_ref().clone();
            self.diags.push(Diagnostic::error(
                method_span.clone(),
                format!("`MapEntry<{}, {}>` does not have a method called `{method}`.", type_name(&key), type_name(&val)),
                "Use `entry.key` to get the key and `entry.value` to get the value.",
                "`MapEntry` values only have two fields: `.key` and `.value`. They have no methods.",
            ));
            return Type::Error;
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

        // M7 P3a: errors-capable value method dispatch.
        if let Type::ErrorsCapable { inner } = receiver_ty {
            let inner = inner.as_ref().clone();
            return self.check_errors_capable_method(method, method_span, &inner);
        }

        // M6: options type method dispatch.
        if let Type::Options { name: opts_name } = receiver_ty {
            return match method {
                "toString" => Type::String,
                other => {
                    self.diags.push(Diagnostic::error(
                        method_span.clone(),
                        format!("`{opts_name}` does not have a method called `{other}`."),
                        "Options types only have `.toString()` as a built-in method.",
                        "Method calls are checked at compile time. Only `.toString()` exists on options values.",
                    ));
                    Type::Error
                }
            };
        }

        // M7 P3b: string method dispatch.
        if receiver_ty == &Type::String {
            return if let Some(ret) = string_method_return(method) {
                ret
            } else {
                let available_list = STRING_METHODS.join(", ");
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("`string` does not have a method called `{method}`."),
                    format!("Available string methods: {available_list}."),
                    "Check the spelling. String methods are fixed built-ins — they cannot be extended.",
                ));
                Type::Error
            };
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
            format!(
                "`{}` does not have a method called `{method}`.",
                type_name(receiver_ty)
            ),
            what_instead,
            "Method calls are checked at compile time. Only the listed methods exist on this type.",
        ));
        Type::Error
    }

    /// M7 P3a: type-check a method call on an `errors`-capable value.
    ///
    /// Available methods: `.failed()`, `.or(default)`, `.message`, `.suggestions`,
    /// `.trace`, `.source`. All other method names are a compile error.
    fn check_errors_capable_method(
        &mut self,
        method: &str,
        method_span: &SourceSpan,
        inner: &Type,
    ) -> Type {
        match method {
            "failed" => {
                // .failed() returns bool. Records that the check happened.
                Type::Bool
            }
            "or" => {
                // .or(default) — returns the success type. Arg checking happens
                // at the call site where args are inferred; here return inner.
                inner.clone()
            }
            "message" => Type::String,
            "suggestions" => Type::BuiltinArray {
                elem: Box::new(Type::String),
            },
            "trace" => {
                // trace returns array<Frame> — Frame is a compiler-synthesized shape.
                Type::BuiltinArray {
                    elem: Box::new(Type::Shape {
                        name: "Frame".into(),
                    }),
                }
            }
            "source" => {
                // source returns SourceLoc — a compiler-synthesized shape.
                Type::Shape {
                    name: "SourceLoc".into(),
                }
            }
            other => {
                self.diags.push(Diagnostic::error(
                    method_span.clone(),
                    format!("An `errors`-capable value does not have a method called `{other}`."),
                    "Available methods: failed(), or(default), message, suggestions, trace, source.",
                    "An `errors`-capable value is the output of a call that can fail. Check `.failed()` first, then use the success value directly.",
                ));
                Type::Error
            }
        }
    }

    /// M7 P3a: after calling a function that returns `ErrorsCapable`, return the
    /// `ErrorsCapable` type as-is regardless of context.
    ///
    /// The caller must handle the `ErrorsCapable` type — either by chaining `.or()` /
    /// `.failed()` immediately (method dispatch handles it), or by storing in a binding
    /// (the binding carries the `ErrorsCapable` type). When the binding is later used
    /// as a success value in a non-errors function, `resolve_ident` emits the diagnostic.
    fn handle_errors_capable_call_result(
        &mut self,
        result: Type,
        _fn_name: &str,
        _call_span: SourceSpan,
    ) -> Type {
        // Always return the ErrorsCapable type — method chaining (.or, .failed)
        // and resolve_ident (for binding uses) handle the diagnostic responsibility.
        result
    }

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
            // Type param names resolve to TypeParam when inside a generic context.
            AstType::Named(n, _) if self.type_param_scope.contains_key(n) => {
                Type::TypeParam { name: n.clone() }
            }
            // M6: union type aliases resolve to the full union type.
            AstType::Named(n, _) if self.union_aliases.contains_key(n) => {
                self.union_aliases[n].clone()
            }
            // M6: options type names resolve to Type::Options.
            AstType::Named(n, _) if self.options_table.contains(n) => {
                Type::Options { name: n.clone() }
            }
            AstType::Named(n, _) if self.shape_table.contains(n) => Type::Shape { name: n.clone() },
            // M7 P3c: built-in compiler-synthesized types — always recognized.
            AstType::Named(n, _) if matches!(n.as_str(), "Frame" | "SourceLoc") => {
                Type::Shape { name: n.clone() }
            }
            // M7 P3c: first-class range type — `range` as a type annotation.
            AstType::Named(n, _) if n == "range" => Type::Range {
                element: Box::new(Type::Int),
                end_inclusive: false,
            },
            AstType::Named(n, span)
                if matches!(n.as_str(), "array" | "fixed" | "maybe" | "map" | "MapEntry") =>
            {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{n}` requires type argument(s)."),
                    format!("Write `{n}<T>` — for example, `map<string, int>` or `array<int>`.", n = n),
                    format!("`{n}` is a built-in generic type. It must have the required type arguments.", n = n),
                ));
                Type::Error
            }
            AstType::Named(n, _) if self.generic_shape_table.contains(n) => {
                // Bare generic shape name without type args — invalid in non-generic context.
                Type::Error
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
            AstType::SelfType { span } => match &self.current_shape {
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
            },
            AstType::Dynamic { contract, span } => {
                if self.shape_table.contains(contract) {
                    Type::Dynamic {
                        contract: contract.clone(),
                    }
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
            AstType::TypeParam { name, .. } => {
                if self.type_param_scope.contains_key(name) {
                    Type::TypeParam { name: name.clone() }
                } else {
                    Type::Error
                }
            }
            AstType::Generic {
                name,
                args,
                name_span,
                ..
            } => {
                // Catch capitalized built-in names (Array, Fixed, Map) — Golden Rule 13:
                // capital letter = type, everything else = lowercase. Built-ins are lowercase.
                let lower = name.to_lowercase();
                if name.as_str() != lower.as_str()
                    && matches!(lower.as_str(), "array" | "fixed" | "map")
                {
                    self.diags.push(Diagnostic::error(
                        name_span.clone(),
                        format!("`{name}` is not a type — built-in collection types are lowercase in Yinz."),
                        format!("Use `{lower}` (lowercase): `{lower}<...>`"),
                        "In Yinz, capital letter = user-defined shape, lowercase = built-in. \
                         `Array`, `Fixed`, and `Map` are not valid — use `array`, `fixed`, `map`.",
                    ));
                    return Type::Error;
                }
                let resolved_args: Vec<Type> =
                    args.iter().map(|a| self.ast_type_to_type(a)).collect();
                match name.as_str() {
                    "array" => {
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinArray {
                            elem: Box::new(elem),
                        }
                    }
                    "fixed" => {
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinFixed {
                            elem: Box::new(elem),
                            size: None,
                        }
                    }
                    "map" => {
                        let mut args = resolved_args.into_iter();
                        let key = args.next().unwrap_or(Type::Error);
                        let val = args.next().unwrap_or(Type::Error);
                        Type::BuiltinMap {
                            key: Box::new(key),
                            val: Box::new(val),
                        }
                    }
                    "MapEntry" => {
                        let mut args = resolved_args.into_iter();
                        let key = args.next().unwrap_or(Type::Error);
                        let val = args.next().unwrap_or(Type::Error);
                        Type::MapEntry {
                            key: Box::new(key),
                            val: Box::new(val),
                        }
                    }
                    _ => {
                        if self.generic_shape_table.contains(name) {
                            Type::Generic {
                                name: name.clone(),
                                args: resolved_args,
                            }
                        } else {
                            Type::Error
                        }
                    }
                }
            }
            AstType::Maybe { inner, .. } => {
                let inner_ty = self.ast_type_to_type(inner);
                Type::Maybe {
                    inner: Box::new(inner_ty),
                }
            }
            // M6: Union types — resolve each variant and return Type::Union.
            AstType::Union { variants, .. } => {
                let resolved: Vec<Type> =
                    variants.iter().map(|v| self.ast_type_to_type(v)).collect();
                // `T | none` is rewritten to `maybe<T>` per design/narrowing.md.
                if resolved.len() == 2 {
                    let none_idx = resolved.iter().position(|t| *t == Type::Error); // none resolves oddly
                    let _ = none_idx; // For now, leave `T | none` as Union; P3b note
                }
                // Single-variant union: typeck error (degenerate form).
                if resolved.len() < 2 {
                    Type::Error
                } else {
                    Type::Union { variants: resolved }
                }
            }
            // M7 P3a: `-> T errors` — resolve to ErrorsCapable wrapping the inner type.
            AstType::ErrorCapable { inner, .. } => {
                let inner_ty = self.ast_type_to_type(inner);
                Type::ErrorsCapable {
                    inner: Box::new(inner_ty),
                }
            }
            // M8 P4: `sensitive T` — resolve to Sensitive wrapping the inner type.
            AstType::Sensitive(inner) => {
                let inner_ty = self.ast_type_to_type(inner);
                // Only string is allowed as the inner type in v0.1. Other types
                // will produce type-mismatch errors downstream; no extra diagnostic needed.
                Type::Sensitive {
                    inner: Box::new(inner_ty),
                }
            }
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
            let shape_ty = Type::Shape {
                name: shape_name.clone(),
            };
            for contract_name in contracts {
                let Some(contract_def) = self.shape_table.get(contract_name) else {
                    continue; // already errored in collect_shapes
                };
                let contract_sigs = contract_def.contract_sigs.clone();
                let shape_def_span = self
                    .shape_table
                    .get(shape_name)
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
    fn infer_field_access(
        &mut self,
        receiver: &Expr,
        field: &str,
        field_span: &SourceSpan,
    ) -> Type {
        let receiver_ty = self.infer_expr(receiver, None);

        // M7 P3a: errors-capable value property access (message, suggestions, trace, source).
        // These are dot-property accesses (no parens) per the Yinz dot-postfix rule.
        if let Type::ErrorsCapable { inner } = &receiver_ty {
            let inner = inner.as_ref().clone();
            return match field {
                "message" => Type::String,
                "suggestions" => Type::BuiltinArray {
                    elem: Box::new(Type::String),
                },
                "trace" => Type::BuiltinArray {
                    elem: Box::new(Type::Shape {
                        name: "Frame".into(),
                    }),
                },
                "source" => Type::Shape {
                    name: "SourceLoc".into(),
                },
                other => {
                    // Not an error-property — check if it's a field on the inner type.
                    // Recurse by pretending the receiver has the inner type.
                    let _ = inner;
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("An `errors`-capable value does not have a field called `{other}`."),
                        "Available properties: message, suggestions, trace, source. Call `.failed()` first to check, or `.or(default)` for a fallback.",
                        "An `errors`-capable value is the output of a call that can fail. Check `.failed()` first, then access the success value directly.",
                    ));
                    Type::Error
                }
            };
        }

        // M5 P3c: `MapEntry<K,V>.key` / `.value` field access.
        if let Type::MapEntry { key, val } = &receiver_ty {
            return match field {
                "key" => key.as_ref().clone(),
                "value" => val.as_ref().clone(),
                other => {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("`MapEntry` does not have a field called `{other}`."),
                        "Use `.key` to get the key and `.value` to get the value.",
                        "`MapEntry<K, V>` has exactly two fields: `key: K` and `value: V`.",
                    ));
                    Type::Error
                }
            };
        }

        // M5 P3c: dot access on map — emit "use bracket syntax" error.
        if let Type::BuiltinMap { key, val } = &receiver_ty {
            self.diags.push(Diagnostic::error(
                field_span.clone(),
                format!("Cannot use `.{field}` to look up a map key."),
                format!("Map keys are runtime values — use `m[\"{field}\"]` to look up a key. For checking existence, use `m.has(\"{field}\")`.",),
                "Dot access is for shape fields with compile-time-known names. Map keys are dynamic — use bracket syntax.",
            ));
            let _ = (key, val);
            return Type::Error;
        }

        // M5 P3b: `maybe<T>.value` — flow-sensitive field access.
        if let Type::Maybe { inner } = &receiver_ty {
            if field == "value" {
                let inner = inner.as_ref().clone();
                // Check if the binding is known-non-none from a prior .exists() check.
                let is_safe = if let Expr::Ident(name, _) = receiver {
                    self.maybe_non_none.contains(name.as_str())
                } else {
                    false
                };
                if !is_safe {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        "`maybe.value` requires you to first check `m.exists()`.",
                        "Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.",
                        "The compiler cannot prove this `maybe` has a value here. `.value` without a prior `.exists()` check is a compile error.",
                    ));
                    return Type::Error;
                }
                return inner;
            } else {
                self.diags.push(Diagnostic::error(
                    field_span.clone(),
                    format!("`maybe<{}>` does not have a field called `{field}`.", type_name(inner.as_ref())),
                    "Use `.value` to get the value (after `.exists()` check), `.exists()` to check, or `.or(default)` for a safe fallback.",
                    "`maybe<T>` only has the virtual field `.value` (requires prior `.exists()` guard) and the methods `.exists()` and `.or()`.",
                ));
                return Type::Error;
            }
        }

        // Generic shape field access: `p.first` where `p: Pair<int, string>`.
        if let Type::Generic { name, args } = &receiver_ty {
            let name = name.clone();
            let args = args.clone();
            if let Some(generic_def) = self.generic_shape_table.get(&name) {
                let subst = generic_def.make_substitution(&args);
                return match generic_def.field_type(field, &subst) {
                    Some(ty) => ty,
                    None => {
                        let available = generic_def.field_names();
                        let suggestion = find_closest_name(field, &available);
                        let what_instead = match suggestion {
                            Some(close) => format!("Did you mean `{close}`?"),
                            None => format!("`{name}` has these fields: {}", available.join(", ")),
                        };
                        self.diags.push(Diagnostic::error(
                            field_span.clone(),
                            format!("`{name}` does not have a field called `{field}`.",),
                            what_instead,
                            "Field names must match exactly what was declared in the `shape` body.",
                        ));
                        Type::Error
                    }
                };
            }
        }

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

        // M7 P3c: built-in compiler-synthesized shapes — Frame and SourceLoc.
        // These are never user-declared in source; their fields are hardcoded here.
        if shape_name == "Frame" {
            return match field {
                "file" => Type::String,
                "line" => Type::Maybe {
                    inner: Box::new(Type::Int),
                },
                "function" => Type::String,
                other => {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("`Frame` does not have a field called `{other}`."),
                        "Frame has three fields: `file: string`, `line: maybe<int>`, `function: string`.",
                        "`Frame` is a compiler-synthesized shape that represents one stack frame in an error trace.",
                    ));
                    Type::Error
                }
            };
        }
        if shape_name == "SourceLoc" {
            return match field {
                "file" => Type::String,
                "line" => Type::Maybe {
                    inner: Box::new(Type::Int),
                },
                other => {
                    self.diags.push(Diagnostic::error(
                        field_span.clone(),
                        format!("`SourceLoc` does not have a field called `{other}`."),
                        "SourceLoc has two fields: `file: string`, `line: maybe<int>`.",
                        "`SourceLoc` is a compiler-synthesized shape that records the source position of an error.",
                    ));
                    Type::Error
                }
            };
        }

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
                format!("`{shape_name}` does not have a field called `{field}`.",),
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
    fn check_struct_lit(
        &mut self,
        fields: &[StructLitField],
        hint: Option<&Type>,
        span: &SourceSpan,
    ) -> Type {
        // M5 P3c: handle `let m: map<K,V> = { }` — empty struct lit with BuiltinMap annotation.
        // Non-empty struct lits with identifier keys are errors (should be MapLit with string keys).
        if let Some(Type::BuiltinMap { key, val }) = hint {
            if fields.is_empty() {
                return Type::BuiltinMap {
                    key: key.clone(),
                    val: val.clone(),
                };
            }
            self.diags.push(Diagnostic::error(
                span.clone(),
                "Map literals use string or integer keys, not field names.",
                "Write `{ \"key\": value }` instead of `{ key: value }` for map literals.",
                "Shape values use identifier field names. Map literals use string or integer literal keys.",
            ));
            for f in fields {
                self.infer_expr(&f.value, None);
            }
            return Type::Error;
        }

        let shape_name = match hint {
            Some(Type::Shape { name }) => name.clone(),
            // `let x: array<Symbol> = { ... }` — they wrote a shape value where an array goes.
            // Specific suggestion: wrap in brackets.
            Some(Type::BuiltinArray { elem }) => {
                let elem_name = type_name(elem);
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{{ ... }}` creates a single `{elem_name}` value, not an `array<{elem_name}>`."),
                    format!("Put it inside `[...]` to make an array: `[{{ ... }}]`"),
                    format!("`{{ ... }}` creates one value. `[...]` creates a collection. \
                             Use `array<{elem_name}>` when you need multiple values."),
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            Some(Type::BuiltinFixed { elem, .. }) => {
                let elem_name = type_name(elem);
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{{ ... }}` creates a single `{elem_name}` value, not a `fixed<{elem_name}>`."),
                    format!("Put it inside `[...]` to make a fixed array: `[{{ ... }}]`"),
                    format!("`{{ ... }}` creates one value. `[...]` creates a collection."),
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            Some(other) if *other != Type::Error => {
                let what_instead = match other {
                    Type::BuiltinMap { .. } =>
                        "For a map literal, use quoted string keys: `{ \"key\": value, \"key2\": value2 }`".to_string(),
                    Type::Union { .. } =>
                        "Check the union type — if one variant is a `shape`, annotate with its name; if it's a `map`, use quoted string keys.".to_string(),
                    _ =>
                        "Annotate the binding with a `shape` name: `let p: Player = { ... }`".to_string(),
                };
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!(
                        "A shape value `{{ ... }}` cannot produce a `{}` value.",
                        type_name(other)
                    ),
                    what_instead,
                    "Shape values use identifier field names (`name: value`). Map literals use quoted string keys (`\"name\": value`).",
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            Some(Type::Error) => {
                // Hint is Type::Error — an upstream diagnostic already explained the problem.
                // Don't cascade with a confusing "needs a type annotation" message.
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
            _ => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    "This shape value needs a type annotation — the compiler needs to know which `shape` to create.",
                    "Add a type annotation: `let p: Player = { ... }`",
                    "Shape values are anonymous — the `shape` type comes from the annotation on the left. Without it, the compiler cannot check field names or types.",
                ));
                for f in fields {
                    self.infer_expr(&f.value, None);
                }
                return Type::Error;
            }
        };

        let Some(shape_def) = self.shape_table.get(&shape_name) else {
            for f in fields {
                self.infer_expr(&f.value, None);
            }
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
            for f in fields {
                self.infer_expr(&f.value, None);
            }
            return Type::Error;
        }

        // Collect all missing required fields, then emit one consolidated diagnostic.
        let missing: Vec<&str> = shape_def
            .fields
            .iter()
            .filter(|sf| !sf.is_hidden && !fields.iter().any(|f| f.name == sf.name))
            .map(|sf| sf.name.as_str())
            .collect();
        match missing.len() {
            0 => {}
            1 => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Missing field `{}` in `{shape_name}` construction.", missing[0]),
                    format!("Add `{}: value` to the shape value.", missing[0]),
                    "Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.",
                ));
            }
            n => {
                let list = missing
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let add = missing
                    .iter()
                    .map(|name| format!("`{name}: value`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("{n} fields are missing from this `{shape_name}` value: {list}."),
                    format!("Add the missing fields: {add}."),
                    "Every visible field of a shape must be provided when constructing a value — the compiler cannot fill them in for you.",
                ));
            }
        }

        // Check each provided field: name must exist and value type must match.
        for lit_field in fields {
            let expected_ty = shape_def
                .fields
                .iter()
                .find(|f| f.name == lit_field.name)
                .map(|f| f.ty.clone());
            match expected_ty {
                None => {
                    let available: Vec<&str> = shape_def
                        .fields
                        .iter()
                        .filter(|f| !f.is_hidden)
                        .map(|f| f.name.as_str())
                        .collect();
                    let suggestion = find_closest_name(&lit_field.name, &available);
                    let what_instead = match suggestion {
                        Some(close) => format!("Did you mean `{close}`?"),
                        None => {
                            format!("`{shape_name}` has these fields: {}", available.join(", "))
                        }
                    };
                    self.diags.push(Diagnostic::error(
                        lit_field.name_span.clone(),
                        format!(
                            "`{shape_name}` does not have a field called `{}`.",
                            lit_field.name
                        ),
                        what_instead,
                        "Shape values can only set fields declared on the shape.",
                    ));
                    self.infer_expr(&lit_field.value, None);
                }
                Some(expected) => {
                    let actual = self.infer_expr(&lit_field.value, Some(&expected));
                    if actual != Type::Error && expected != Type::Error && actual != expected {
                        self.diags.push(Diagnostic::error(
                            lit_field.name_span.clone(),
                            format!(
                                "Field `{}` expects `{}`, but got `{}`.",
                                lit_field.name,
                                type_name(&expected),
                                type_name(&actual)
                            ),
                            format!(
                                "Pass a `{}` value for `{}`.",
                                type_name(&expected),
                                lit_field.name
                            ),
                            format!(
                                "`{shape_name}.{}` was declared as `{}`.",
                                lit_field.name,
                                type_name(&expected)
                            ),
                        ));
                    }
                }
            }
        }

        Type::Shape { name: shape_name }
    }

    /// Type-check a field assignment `target.field = value`.
    fn check_field_assign(&mut self, target: &Expr, value: &Expr, span: &SourceSpan) {
        let Expr::FieldAccess {
            receiver,
            field,
            field_span,
            ..
        } = target
        else {
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
                format!(
                    "Cannot assign `{}` to field `{field}` which has type `{}`.",
                    type_name(&value_ty),
                    type_name(&field_ty)
                ),
                format!("Pass a `{}` value.", type_name(&field_ty)),
                format!(
                    "The field `{field}` was declared as `{}`.",
                    type_name(&field_ty)
                ),
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
                if receiver_ty == Type::Error {
                    return Type::Error;
                }
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

    /// Type-check an array or fixed literal `[e1, e2, ...]`.
    fn check_array_lit(
        &mut self,
        elements: &[Expr],
        hint: Option<&Type>,
        span: &SourceSpan,
    ) -> Type {
        // Determine element type and whether this is a fixed literal from the hint.
        let (hint_elem, is_fixed) = match hint {
            Some(Type::BuiltinArray { elem }) => (Some(elem.as_ref().clone()), false),
            Some(Type::BuiltinFixed { elem, .. }) => (Some(elem.as_ref().clone()), true),
            Some(Type::Maybe { inner }) => {
                // maybe<array<T>> — the inner array has an element type.
                if let Type::BuiltinArray { elem } = inner.as_ref() {
                    (Some(elem.as_ref().clone()), false)
                } else {
                    (None, false)
                }
            }
            Some(Type::Shape { name }) => {
                // `let x: SomeShape = [...]` — shape is a single value, not a collection.
                // Emit one targeted error and return Type::Error to suppress the downstream
                // let-binding type mismatch, which would just be noise.
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`[]` is an array literal, but `{name}` is a shape — a single value, not a collection."),
                    format!("Use `array<{name}>` if you want a list: `let ... : array<{name}> = []`"),
                    format!("`{name}` holds one value. To store multiple `{name}` values, use `array<{name}>`."),
                ));
                return Type::Error;
            }
            _ => (None, false),
        };

        let mut elem_ty = hint_elem.clone().unwrap_or(Type::Error);
        for (i, elem) in elements.iter().enumerate() {
            let ty = self.infer_expr(elem, hint_elem.as_ref());
            if i == 0 && elem_ty == Type::Error {
                elem_ty = ty.clone();
            } else if ty != Type::Error && elem_ty != Type::Error && ty != elem_ty {
                self.diags.push(Diagnostic::error(
                    elem.span().clone(),
                    format!(
                        "Element {} has type `{}`, but the array expects `{}`.",
                        i + 1,
                        type_name(&ty),
                        type_name(&elem_ty)
                    ),
                    format!(
                        "Use a `{}` value here, or change the annotation.",
                        type_name(&elem_ty)
                    ),
                    "All elements of an array or fixed literal must have the same type.",
                ));
            }
        }

        let hint_is_error = matches!(hint, Some(Type::Error));
        if elem_ty == Type::Error && elements.is_empty() && !hint_is_error {
            // Only emit the "cannot work out element type" diagnostic when there's no
            // hint at all (bare `let arr = []`). When hint is Type::Error, an upstream
            // diagnostic already captured the annotation problem — don't cascade.
            self.diags.push(Diagnostic::error(
                span.clone(),
                "Cannot work out the element type of this empty array literal.",
                "Add a type annotation: `let arr: array<int> = []`.",
                "Without an annotation, the compiler cannot determine what type of elements this array holds.",
            ));
        }

        let size = elements.len();
        if is_fixed {
            Type::BuiltinFixed {
                elem: Box::new(elem_ty),
                size: Some(size),
            }
        } else {
            Type::BuiltinArray {
                elem: Box::new(elem_ty),
            }
        }
    }

    /// Type-check an index assignment `receiver[index] = value`.
    fn check_index_assign(
        &mut self,
        receiver: &Expr,
        index: &Expr,
        value: &Expr,
        span: &SourceSpan,
    ) {
        let recv_ty = self.infer_expr(receiver, None);
        let _idx_ty = self.infer_expr(index, Some(&Type::Int));
        match &recv_ty {
            Type::BuiltinArray { elem } => {
                let expected = elem.as_ref().clone();
                let val_ty = self.infer_expr(value, Some(&expected));
                if val_ty != Type::Error && expected != Type::Error && val_ty != expected {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "This value is `{}`, but the array holds `{}`.",
                            type_name(&val_ty),
                            type_name(&expected)
                        ),
                        format!("Assign a `{}` value.", type_name(&expected)),
                        "Index assignment must match the array's element type.",
                    ));
                }
            }
            Type::BuiltinFixed { elem, .. } => {
                let expected = elem.as_ref().clone();
                let val_ty = self.infer_expr(value, Some(&expected));
                if val_ty != Type::Error && expected != Type::Error && val_ty != expected {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "This value is `{}`, but the fixed array holds `{}`.",
                            type_name(&val_ty),
                            type_name(&expected)
                        ),
                        format!("Assign a `{}` value.", type_name(&expected)),
                        "Index assignment must match the fixed array's element type.",
                    ));
                }
            }
            Type::BuiltinMap { val: map_val, .. } => {
                let expected = map_val.as_ref().clone();
                let val_ty = self.infer_expr(value, Some(&expected));
                if val_ty != Type::Error && expected != Type::Error && val_ty != expected {
                    self.diags.push(Diagnostic::error(
                        value.span().clone(),
                        format!(
                            "This value has type `{}`, but the map holds `{}` values.",
                            type_name(&val_ty),
                            type_name(&expected)
                        ),
                        format!("Assign a `{}` value.", type_name(&expected)),
                        "Map value assignment must match the map's value type.",
                    ));
                }
            }
            Type::Error => {
                self.infer_expr(value, None);
            }
            other => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`{}` does not support index assignment.", type_name(other)),
                    "Index assignment works on `array<T>`, `fixed<T>`, and `map<K, V>`.",
                    "Only built-in collection types support `collection[index] = value` syntax.",
                ));
                self.infer_expr(value, None);
            }
        }
    }

    /// Type-check a map literal `{ "alice": 90, "bob": 85 }`.
    fn check_map_lit(
        &mut self,
        entries: &[(Expr, Expr)],
        hint: Option<&Type>,
        span: &SourceSpan,
    ) -> Type {
        let (hint_key, hint_val) = match hint {
            Some(Type::BuiltinMap { key, val }) => {
                (Some(key.as_ref().clone()), Some(val.as_ref().clone()))
            }
            _ => (None, None),
        };

        let mut key_ty = hint_key.clone().unwrap_or(Type::Error);
        let mut val_ty = hint_val.clone().unwrap_or(Type::Error);
        let mut seen_keys: std::collections::HashMap<String, ynz_diagnostics::SourceSpan> =
            std::collections::HashMap::new();

        for (key_expr, val_expr) in entries {
            let k = self.infer_expr(key_expr, hint_key.as_ref());
            let v = self.infer_expr(val_expr, hint_val.as_ref());

            if key_ty == Type::Error {
                key_ty = k.clone();
            }
            if val_ty == Type::Error {
                val_ty = v.clone();
            }

            if k != Type::Error && key_ty != Type::Error && k != key_ty {
                self.diags.push(Diagnostic::error(
                    key_expr.span().clone(),
                    format!(
                        "This key has type `{}`, but the map uses `{}` keys.",
                        type_name(&k),
                        type_name(&key_ty)
                    ),
                    format!(
                        "Use a `{}` key, or change the map annotation.",
                        type_name(&key_ty)
                    ),
                    "All keys in a map literal must have the same type.",
                ));
            }
            if v != Type::Error && val_ty != Type::Error && v != val_ty {
                self.diags.push(Diagnostic::error(
                    val_expr.span().clone(),
                    format!(
                        "This value has type `{}`, but the map holds `{}` values.",
                        type_name(&v),
                        type_name(&val_ty)
                    ),
                    format!(
                        "Use a `{}` value, or change the map annotation.",
                        type_name(&val_ty)
                    ),
                    "All values in a map literal must have the same type.",
                ));
            }

            // Duplicate-key detection for literal string/int keys.
            let key_repr = match key_expr {
                Expr::StringLit(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
                Expr::IntLit(n, _) => Some(n.to_string()),
                // M7: backtick strings with no interpolation are pure literals — check for duplicates.
                Expr::InterpolatedString(parts, _) => {
                    if parts.len() == 1 {
                        if let ynz_ast::nodes::StringPart::Lit(bytes, _) = &parts[0] {
                            Some(String::from_utf8_lossy(bytes).to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(key_str) = key_repr {
                if let Some(first_span) = seen_keys.get(&key_str) {
                    self.diags.push(
                        Diagnostic::error(
                            key_expr.span().clone(),
                            format!("Duplicate key `\"{key_str}\"` in this map literal — the key is listed twice."),
                            "Remove or rename one of the two entries so each key is unique.",
                            "The compiler refuses to silently pick one — duplicate keys in a map literal are always a mistake.",
                        ).with_related(first_span.clone(), "first occurrence here"),
                    );
                } else {
                    seen_keys.insert(key_str, key_expr.span().clone());
                }
            }
        }

        if (key_ty == Type::Error || val_ty == Type::Error) && entries.is_empty() {
            self.diags.push(Diagnostic::error(
                span.clone(),
                "Cannot work out the key and value types of this empty map literal.",
                "Add a type annotation: `let m: map<string, int> = {}`.",
                "Without an annotation, the compiler cannot determine what type of keys and values this map holds.",
            ));
        }

        Type::BuiltinMap {
            key: Box::new(key_ty),
            val: Box::new(val_ty),
        }
    }

    // ── M6: union + narrowing typeck ──────────────────────────────────────────

    /// Validate an `Is(TypePath)` arm pattern in a multi-case block.
    /// Emits a diagnostic if the named type is not a variant of the scrutinee's union.
    fn check_is_arm_pattern(
        &mut self,
        scrutinee_ty: &Type,
        type_path: &ynz_ast::nodes::TypePath,
        span: &SourceSpan,
    ) {
        match scrutinee_ty {
            Type::Union { variants } => {
                let valid: Vec<String> = variants
                    .iter()
                    .filter_map(|v| {
                        if let Type::Shape { name } = v {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !type_path.name.is_empty() && !valid.contains(&type_path.name) {
                    self.diags.push(Diagnostic::error(
                        type_path.span.clone(),
                        format!("`{}` is not a variant of this union.", type_path.name),
                        format!("Valid variants are: {}", valid.join(", ")),
                        "The `is TypeName` arm must name one of the union's declared variants.",
                    ));
                }
            }
            Type::Error => {} // suppress cascades
            other => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("`is {}` used on `{}` which is not a union type.", type_path.name, type_name(other)),
                    "The `is TypeName =>` arm form is for union types: `shape S = A | B`.",
                    "Union types have multiple variants — `is` checks which variant a value is at runtime. \
                     Use `variantName =>` for options types.",
                ));
            }
        }
    }

    // ── M6: options typeck helpers ────────────────────────────────────────────

    /// Typecheck `x is Foo` type-narrowing predicate expression.
    ///
    /// Returns `Type::Bool`. Validates that the scrutinee is a union type AND
    /// that the type name is a declared variant of that union.
    ///
    /// For the condition-form narrowing (`if (x is Foo) { ... }`), the
    /// actual narrowing fact is applied in `check_stmt_if` when it detects
    /// an `Expr::Is` condition. This method just produces the bool type.
    fn check_is_expr(
        &mut self,
        inner: &Expr,
        type_path: &ynz_ast::nodes::TypePath,
        _span: &SourceSpan,
    ) -> Type {
        let scrutinee_ty = self.infer_expr(inner, None);
        if type_path.name.is_empty() {
            return Type::Bool; // parse error already emitted
        }
        match &scrutinee_ty {
            Type::Union { variants } => {
                let variant_names: Vec<String> = variants
                    .iter()
                    .filter_map(|v| {
                        if let Type::Shape { name } = v {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !variant_names.contains(&type_path.name) {
                    self.diags.push(Diagnostic::error(
                        type_path.span.clone(),
                        format!("`{}` is not a variant of this union.", type_path.name),
                        format!("Valid variants are: {}", variant_names.join(", ")),
                        "The `is` check must name one of the union's declared variants.",
                    ));
                }
            }
            Type::Error => {}
            other => {
                // INFO-level: is-check on non-union (always true or wrong).
                // For now emit a regular error for structural correctness.
                self.diags.push(Diagnostic::error(
                    type_path.span.clone(),
                    format!("`is {}` used on `{}` which is not a union type.", type_path.name, type_name(other)),
                    "The `is TypeName` check is for union types: `shape S = A | B`.",
                    "Union types have multiple variants — `is` checks which variant a value is at runtime.",
                ));
            }
        }
        Type::Bool
    }

    // ── M6: options typeck helpers ────────────────────────────────────────────

    /// Typecheck an options value access: `OptionsTypeName.variantName`.
    ///
    /// Called from the `Expr::FieldAccess` handler when the receiver is an identifier
    /// that names an options type. Returns `Type::Options { name }` on success.
    fn check_options_value(&mut self, type_name: &str, variant: &str, span: &SourceSpan) -> Type {
        let entry = self.options_table.get(type_name).unwrap(); // caller verified contains()
        if entry.variants.contains(&variant.to_string()) {
            Type::Options {
                name: type_name.to_string(),
            }
        } else {
            let valid: Vec<&str> = entry.variants.iter().map(String::as_str).collect();
            self.diags.push(Diagnostic::error(
                span.clone(),
                format!("`{type_name}` has no variant named `{variant}`."),
                format!("Valid variants are: {}", valid.join(", ")),
                "Options variants must be declared in the `options` type body.",
            ));
            Type::Error
        }
    }

    /// Typecheck an `OptionName` arm in a multi-case `if`.
    ///
    /// Validates: scrutinee is an options type; variant name is valid for that type.
    fn check_option_name_arm(
        &mut self,
        scrutinee_ty: &Type,
        variant_name: &str,
        span: &SourceSpan,
    ) {
        match scrutinee_ty {
            Type::Options { name: opts_name } => {
                if let Some(entry) = self.options_table.get(opts_name) {
                    if !entry.variants.contains(&variant_name.to_string()) {
                        let valid: Vec<&str> = entry.variants.iter().map(String::as_str).collect();
                        self.diags.push(Diagnostic::error(
                            span.clone(),
                            format!("`{opts_name}` has no variant `{variant_name}`."),
                            format!("Valid variants are: {}", valid.join(", ")),
                            "Each arm in a multi-case `if` over an options type must name one of the declared variants.",
                        ));
                    }
                }
            }
            Type::Error => {} // already reported upstream
            other => {
                self.diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Cannot use variant-name arm `{variant_name}` on a `{}` value.", type_name(other)),
                    "Variant-name arms are for options types: `options Status { active, inactive }`.",
                    "The `variantName =>` arm form matches against named options variants. \
                     Use `is TypeName =>` to narrow a union, or a value pattern for other types.",
                ));
            }
        }
    }
}

/// Resolve a simple AST type to a typeck Type using available table info.
///
/// Used for union alias resolution before the full Checker is built.
/// Only handles: Named shapes, union of Named shapes, primitives.
fn resolve_alias_type(ast_ty: &AstType, shape_table: &crate::shapes::ShapeTable) -> Type {
    match ast_ty {
        AstType::Int => Type::Int,
        AstType::Float => Type::Float,
        AstType::Bool => Type::Bool,
        AstType::Number { precision } => Type::Number {
            precision: *precision,
        },
        AstType::Named(n, _) if n == "string" => Type::String,
        AstType::Named(n, _) if shape_table.contains(n) => Type::Shape { name: n.clone() },
        AstType::Union { variants, .. } => {
            let resolved: Vec<Type> = variants
                .iter()
                .map(|v| resolve_alias_type(v, shape_table))
                .collect();
            if resolved.len() < 2 {
                Type::Error
            } else {
                Type::Union { variants: resolved }
            }
        }
        _ => Type::Error,
    }
}

/// Collect `shape Name = Type` alias declarations from the module.
///
/// These are union type aliases like `shape Shape = Circle | Square | Triangle`.
/// The alias name maps to the resolved alias type.
fn collect_union_aliases(
    module: &Module,
    shape_table: &crate::shapes::ShapeTable,
) -> HashMap<String, Type> {
    let mut aliases = HashMap::new();
    for item in &module.items {
        if let Item::ShapeDecl(sd) = item {
            if let Some(alias_ast_ty) = &sd.alias_ty {
                let resolved = resolve_alias_type(alias_ast_ty, shape_table);
                aliases.insert(sd.name.clone(), resolved);
            }
        }
    }
    aliases
}

/// Check whether two types are compatible for assignment.
///
/// This is mostly structural equality, with one exception: `BuiltinFixed` ignores the
/// `size` field so that `let f: fixed<int> = [1, 2, 3]` does not fail (annotation has
/// `size: None`; the literal infers `size: Some(3)`).
fn types_compatible(a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::BuiltinFixed { elem: ea, .. }, Type::BuiltinFixed { elem: eb, .. }) => {
            types_compatible(ea, eb)
        }
        (Type::BuiltinArray { elem: ea }, Type::BuiltinArray { elem: eb }) => {
            types_compatible(ea, eb)
        }
        (Type::Maybe { inner: ia }, Type::Maybe { inner: ib }) => types_compatible(ia, ib),
        // M6: union type compatibility — same set of variants (order-insensitive for now).
        (Type::Union { variants: va }, Type::Union { variants: vb }) => {
            va.len() == vb.len()
                && va
                    .iter()
                    .zip(vb.iter())
                    .all(|(a, b)| types_compatible(a, b))
        }
        // M6: assigning a concrete variant type to a union is valid.
        // e.g., `let s: Circle | Square = { radius: 5.0 }` — Circle is a valid union value.
        (Type::Union { variants }, concrete) => {
            variants.iter().any(|v| types_compatible(v, concrete))
        }
        // M7 P3a: ErrorsCapable is compatible with itself when inner types match.
        (Type::ErrorsCapable { inner: ia }, Type::ErrorsCapable { inner: ib }) => {
            types_compatible(ia, ib)
        }
        // M8 P6: mixed-precision number compatibility.
        // Any number<A> is compatible with any number<B> — widening always succeeds;
        // narrowing (A > B) will emit a warning at the call site. Here we just
        // allow the assignment so the program can type-check. The narrowing warning
        // is emitted in check_let_stmt when we detect precision shrinkage.
        (Type::Number { .. }, Type::Number { .. }) => true,
        _ => a == b,
    }
}

/// Return whether `ty` can appear inside a string interpolation `${}`.
///
/// Primitive types are always stringifiable (they have implicit `.toString()`).
/// Shape types are stringifiable when a standalone `toString` function exists
/// whose first parameter is that shape type. All other types are not stringifiable.
fn is_stringifiable(ty: &Type, sig_table: &crate::signatures::SignatureTable) -> bool {
    match ty {
        Type::String | Type::Int | Type::Float | Type::Bool => true,
        Type::Number { .. } => true,
        Type::Error => true, // suppress cascade errors from upstream type failures
        Type::Options { .. } => true, // .toString() built-in on options types
        Type::Shape { name } => {
            // A shape is stringifiable if there's a standalone `toString` function
            // whose first parameter type is `Shape { name }`.
            if let Some(sig) = sig_table.fns.get("toString") {
                if let Some((_, first_ty)) = sig.params.first() {
                    return first_ty == &Type::Shape { name: name.clone() };
                }
            }
            false
        }
        _ => false,
    }
}

/// Return the typeck `Type` for a type-attached constant like `int.max` or `number.epsilon`.
///
/// Returns `None` if the (type_name, const_name) pair is not a known constant.
pub fn type_attached_const_type(type_name: &str, const_name: &str) -> Option<Type> {
    match (type_name, const_name) {
        ("int", "max") | ("int", "min") => Some(Type::Int),
        ("float", "max") | ("float", "min") | ("float", "epsilon") => Some(Type::Float),
        ("number", "max") | ("number", "min") | ("number", "epsilon") => {
            Some(Type::Number { precision: 34 })
        }
        _ => None,
    }
}

fn body_has_error_node(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Expr(e) => expr_has_error(e),
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => expr_has_error(value),
        Stmt::If { cond, body, .. } => expr_has_error(cond) || body_has_error_node(&body.stmts),
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            expr_has_error(scrutinee)
                || arms.iter().any(|arm| body_has_error_node(&arm.body.stmts))
                || else_arm
                    .as_ref()
                    .is_some_and(|b| body_has_error_node(&b.stmts))
        }
        Stmt::While { cond, body, .. }
        | Stmt::For {
            iter: cond, body, ..
        } => expr_has_error(cond) || body_has_error_node(&body.stmts),
        Stmt::Return { value, .. } => value.as_ref().is_some_and(expr_has_error),
        // M4 P3a: field assignment — not yet type-checked.
        Stmt::FieldAssign { target, value, .. } => expr_has_error(target) || expr_has_error(value),
        // M5 P1: index assignment — parser does not construct in P1; reached only if
        // out-of-sequence change happens. Walk sub-expressions for safety.
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => expr_has_error(receiver) || expr_has_error(index) || expr_has_error(value),
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
        Expr::FieldAccess { receiver, .. } | Expr::PostfixOp { receiver, .. } => {
            expr_has_error(receiver)
        }
        Expr::StructLit { fields, .. } => fields.iter().any(|f| expr_has_error(&f.value)),
        // M5 P1: bracket-index — propagate error check into receiver + index.
        Expr::IndexAccess {
            receiver, index, ..
        } => expr_has_error(receiver) || expr_has_error(index),
        // M5 P3b: array literal — propagate error check into all elements.
        Expr::ArrayLit { elements, .. } => elements.iter().any(expr_has_error),
        // M5 P3c: map literal — propagate error check into all keys and values.
        Expr::MapLit { entries, .. } => entries
            .iter()
            .any(|(k, v)| expr_has_error(k) || expr_has_error(v)),
        // M6: is-expression — propagate into the scrutinee.
        Expr::Is { expr, .. } => expr_has_error(expr),
        // M7: interpolated string — propagate into each interpolated sub-expression.
        Expr::InterpolatedString(parts, _) => parts.iter().any(|p| match p {
            ynz_ast::nodes::StringPart::Lit(_, _) => false,
            ynz_ast::nodes::StringPart::Expr(e, _) => expr_has_error(e),
        }),
        // M8 P5: wait/background — propagate into inner expression.
        Expr::Wait(inner, _) | Expr::Background(inner, _) => expr_has_error(inner),
    }
}

fn binop_display(op: &BinOpKind) -> &'static str {
    use BinOpKind::*;
    match op {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        Lt => "<",
        LtEq => "<=",
        Gt => ">",
        GtEq => ">=",
        EqEq => "==",
        NotEq => "!=",
        And => "&&",
        Or => "||",
        BitAnd => "&",
        BitOr => "|",
        BitXor => "^",
        Shl => "<<",
        Shr => ">>",
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
            if dist <= threshold {
                Some((dist, c))
            } else {
                None
            }
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
                name: "entrypoint".into(),
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
                // test-ratchet: M7 P1 adds errors_capable field to FunctionDecl
                errors_capable: false,
                // test-ratchet: M8 P1 adds is_exported
                is_exported: false,
                // test-ratchet: M8 P3 adds doc
                doc: None,
            })],
            span: span(0, 57),
        };

        let intrinsics = PrimitiveIntrinsicTable::m3().with_test_intrinsic(
            "_test_takes_nothing",
            vec![Type::Nothing],
            Type::Nothing,
        );
        let shape_table = crate::shapes::collect_shapes(&module, &mut DiagnosticBucket::new());
        let generic_shape_table =
            crate::shapes::collect_generic_shapes(&module, &mut DiagnosticBucket::new());
        let sig_table = crate::signatures::collect_signatures(
            &module,
            &mut DiagnosticBucket::new(),
            &shape_table,
        );
        let generic_fn_table = crate::signatures::collect_generic_signatures(
            &module,
            &mut DiagnosticBucket::new(),
            &shape_table,
        );
        let (_, _, diags) = check(
            &module,
            &sig_table,
            &shape_table,
            &generic_fn_table,
            &generic_shape_table,
            &intrinsics,
        );
        let diags: Vec<_> = diags.into_iter().collect();
        assert_eq!(
            diags.len(),
            1,
            "Expected 1 type-mismatch diagnostic, got: {diags:#?}"
        );

        let d = &diags[0];
        assert!(!d.what.is_empty(), "what must be non-empty");
        assert!(!d.what_instead.is_empty(), "what_instead must be non-empty");
        assert!(!d.why.is_empty(), "why must be non-empty");
    }
}
