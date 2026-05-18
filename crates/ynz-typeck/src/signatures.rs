use std::collections::HashMap;

use ynz_ast::nodes::{Item, Module, OwnershipModifier, Type as AstType};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{
    generics::{GenericFnSig, GenericFnTable},
    shapes::ShapeTable,
    types::Type,
};

/// The resolved signature of a user-defined function.
#[derive(Clone, Debug)]
pub struct FunctionSig {
    /// (parameter_name, resolved_type) pairs, in declaration order.
    pub params: Vec<(String, Type)>,
    /// Ownership modifier for each parameter (None = share/inferred).
    pub param_ownerships: Vec<Option<OwnershipModifier>>,
    pub ret: Type,
    pub decl_span: SourceSpan,
}

/// All user-defined function signatures collected from a module.
#[derive(Clone, Debug)]
pub struct SignatureTable {
    pub fns: HashMap<String, FunctionSig>,
}

impl SignatureTable {
    pub fn empty() -> Self {
        Self { fns: HashMap::new() }
    }

    /// All function names in the table, for Levenshtein suggestions.
    pub fn all_names(&self) -> Vec<&str> {
        self.fns.keys().map(String::as_str).collect()
    }
}

/// Walk `module.items`, collect every function's signature, validate `main`.
///
/// Diagnostics emitted here:
/// - Duplicate function name
/// - Missing `main`
/// - `main` with non-`() -> nothing` signature
pub fn collect_signatures(
    module: &Module,
    diags: &mut DiagnosticBucket,
    shape_table: &ShapeTable,
) -> SignatureTable {
    let mut table = SignatureTable::empty();
    let mut main_checked = false;

    for item in &module.items {
        match item {
            // M4 P3a: shapes not yet collected into the signature table.
            Item::ShapeDecl(_) => continue,
            // M6: options declarations — P3a registers them in OptionsTable; not needed here.
            Item::OptionsDecl(_) => continue,
            Item::Function(f) if !f.generics.is_empty() => {
                // Generic functions are collected by collect_generic_signatures.
                continue;
            }
            Item::Function(f) => {
                let params: Vec<(String, Type)> = f
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), sig_ast_type_to_type(&p.ty, shape_table)))
                    .collect();
                let param_ownerships: Vec<Option<OwnershipModifier>> = f
                    .params
                    .iter()
                    .map(|p| p.ownership.clone())
                    .collect();
                let ret = sig_ast_type_to_type(&f.return_type, shape_table);

                if let Some(existing) = table.fns.get(&f.name) {
                    diags.push(
                        Diagnostic::error(
                            f.span.clone(),
                            format!("A function named `{}` is already defined in this file.", f.name),
                            "Rename one of the two functions — each function in a file must have a unique name.",
                            "Yinz does not allow two functions with the same name in the same file.",
                        )
                        .with_related(existing.decl_span.clone(), "first definition here"),
                    );
                    continue;
                }

                if f.name == "main" {
                    main_checked = true;
                    if !f.params.is_empty() {
                        diags.push(Diagnostic::error(
                            f.span.clone(),
                            "`main` must have no parameters.",
                            "Change the declaration to `function main() -> nothing { ... }`",
                            "`main` is the entry point. The program starts here with no arguments — use `process.args` to read command-line inputs (arriving in v0.8).",
                        ));
                    }
                    if ret != Type::Nothing && ret != Type::Error {
                        diags.push(Diagnostic::error(
                            f.span.clone(),
                            format!(
                                "`main` must return `nothing`, but this declares it returns `{}`.",
                                crate::types::type_name(&ret)
                            ),
                            "Change the return type to `nothing`: `function main() -> nothing`",
                            "`main` is the program entry point. It runs and exits — it does not return a value to a caller.",
                        ));
                    }
                }

                table.fns.insert(
                    f.name.clone(),
                    FunctionSig { params, param_ownerships, ret, decl_span: f.span.clone() },
                );
            }
        }
    }

    if !main_checked && !table.fns.contains_key("main") {
        diags.push(Diagnostic::error(
            module.span.clone(),
            "This file has no `main` function.",
            "Add a main function:\n  function main() -> nothing {\n    ...\n  }",
            "Every Yinz program needs a `main` function — that is where execution starts.",
        ));
    }

    table
}

/// Convert an AST type annotation to a typeck `Type` for signature purposes.
///
/// No diagnostic emission — the parser already caught unknown types.
/// Unknown / error types map to `Type::Error` so the signature table is always complete.
pub fn sig_ast_type_to_type(ast_ty: &AstType, shape_table: &ShapeTable) -> Type {
    shape_table.resolve_ast_type(ast_ty)
}

// ── Generic function signature collection (M5 P3a) ────────────────────────────

/// Collect all generic function declarations into a `GenericFnTable`.
///
/// Generic functions (`function identity<T>(...)`) are kept separate from
/// `SignatureTable` because their parameter/return types contain `TypeParam`
/// placeholders. `collect_signatures` skips them; this function picks them up.
pub fn collect_generic_signatures(
    module: &Module,
    diags: &mut DiagnosticBucket,
    shape_table: &ShapeTable,
) -> GenericFnTable {
    let mut table = GenericFnTable::empty();

    for item in &module.items {
        let Item::Function(f) = item else { continue };
        if f.generics.is_empty() {
            continue; // non-generic — handled by collect_signatures
        }

        if table.fns.contains_key(&f.name) {
            diags.push(Diagnostic::error(
                f.span.clone(),
                format!("A generic function named `{}` is already defined.", f.name),
                "Rename one of the two functions.",
                "Yinz does not allow two functions with the same name in the same file.",
            ));
            continue;
        }

        let type_params: Vec<String> = f.generics.iter().map(|gp| gp.name.clone()).collect();

        // Build constraint map: (type_param_name, [contract_names]).
        let constraints: Vec<(String, Vec<String>)> = f.generics.iter()
            .map(|gp| {
                let contracts: Vec<String> = gp.constraints.iter().map(|(c, _)| c.clone()).collect();
                (gp.name.clone(), contracts)
            })
            .filter(|(_, contracts)| !contracts.is_empty())
            .collect();

        let params: Vec<(String, Type)> = f.params.iter()
            .map(|p| (p.name.clone(), resolve_sig_type_with_params(&p.ty, &type_params, shape_table)))
            .collect();

        let param_ownerships: Vec<Option<OwnershipModifier>> = f.params.iter()
            .map(|p| p.ownership.clone())
            .collect();

        let ret = resolve_sig_type_with_params(&f.return_type, &type_params, shape_table);

        table.fns.insert(f.name.clone(), GenericFnSig {
            type_params,
            constraints,
            params,
            param_ownerships,
            ret,
            decl_span: f.span.clone(),
        });
    }

    table
}

/// Resolve an AST type in the context of a generic function signature.
///
/// Names that appear in `type_params` become `Type::TypeParam`; others go through
/// normal shape-table resolution.
fn resolve_sig_type_with_params(
    ast_ty: &AstType,
    type_params: &[String],
    shape_table: &ShapeTable,
) -> Type {
    match ast_ty {
        AstType::Named(n, _) if type_params.contains(n) => Type::TypeParam { name: n.clone() },
        AstType::Generic { name, args, .. } => {
            let resolved_args = args.iter()
                .map(|a| resolve_sig_type_with_params(a, type_params, shape_table))
                .collect();
            Type::Generic { name: name.clone(), args: resolved_args }
        }
        other => shape_table.resolve_ast_type(other),
    }
}
