use std::collections::HashMap;

use ynz_ast::nodes::{Item, Module, Type as AstType};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::types::Type;

/// The resolved signature of a user-defined function.
#[derive(Clone, Debug)]
pub struct FunctionSig {
    /// (parameter_name, resolved_type) pairs, in declaration order.
    pub params: Vec<(String, Type)>,
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
pub fn collect_signatures(module: &Module, diags: &mut DiagnosticBucket) -> SignatureTable {
    let mut table = SignatureTable::empty();
    let mut main_checked = false;

    for item in &module.items {
        match item {
            Item::Function(f) => {
                let params: Vec<(String, Type)> = f
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), sig_ast_type_to_type(&p.ty)))
                    .collect();
                let ret = sig_ast_type_to_type(&f.return_type);

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
                    FunctionSig { params, ret, decl_span: f.span.clone() },
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
pub fn sig_ast_type_to_type(ast_ty: &AstType) -> Type {
    match ast_ty {
        AstType::Nothing => Type::Nothing,
        AstType::Int => Type::Int,
        AstType::Float => Type::Float,
        AstType::Number { .. } => Type::Number { precision: 34 },
        AstType::Bool => Type::Bool,
        AstType::Named(n, _) if n == "string" => Type::String,
        AstType::Error | AstType::Named(_, _) | AstType::Range { .. } => Type::Error,
    }
}
