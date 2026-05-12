use std::sync::Arc;

use ynz_diagnostics::Diagnostic;
use ynz_parser::SourceFile;
use ynz_typeck::check_query;

use crate::{artifact::CompiledArtifact, emit::emit_artifact};

/// The output of the codegen pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CodegenOutput {
    pub artifact: CompiledArtifact,
    pub diagnostics: Vec<Diagnostic>,
}

/// Generate a relocatable object file for a source file.
///
/// This salsa-tracked query depends on `check_query`. It only runs if the
/// type checker found no errors (type errors produce a codegen-skipped output).
///
/// The return type is `Arc<CodegenOutput>` so salsa can cache it by pointer
/// identity without requiring `PartialEq` on the contained LLVM bytes.
#[salsa::tracked]
pub fn codegen_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<CodegenOutput> {
    let check = check_query(db, source);

    // Collect all upstream diagnostics.
    let mut diagnostics = check.diagnostics.clone();

    // If there are type errors, skip codegen — don't emit broken object files.
    if check
        .diagnostics
        .iter()
        .any(|d| d.severity == ynz_diagnostics::Severity::Error)
    {
        return Arc::new(CodegenOutput {
            artifact: CompiledArtifact {
                object_bytes: Vec::new(),
                ir_text: String::new(),
                sha256: [0u8; 32],
            },
            diagnostics,
        });
    }

    // Extract the string literal from the AST (M1: exactly one call to `print`
    // with one string-literal argument).
    let string_bytes = extract_print_string(&check.typed_module.module);

    let source_path = source.path(db);
    match emit_artifact(source_path.as_str(), &string_bytes, None) {
        Ok(artifact) => Arc::new(CodegenOutput {
            artifact,
            diagnostics,
        }),
        Err(msg) => {
            diagnostics.push(Diagnostic::error(
                ynz_diagnostics::SourceSpan::new(source_path.as_str(), 0, 0),
                format!("The compiler failed to produce machine code: {msg}"),
                "This is a compiler bug. Please report it with the source file attached.",
                "Machine-code generation failed inside the LLVM backend.",
            ));
            Arc::new(CodegenOutput {
                artifact: CompiledArtifact {
                    object_bytes: Vec::new(),
                    ir_text: String::new(),
                    sha256: [0u8; 32],
                },
                diagnostics,
            })
        }
    }
}

/// Walk the M1 AST and pull out the bytes of the string literal passed to `print`.
fn extract_print_string(module: &ynz_ast::nodes::Module) -> Vec<u8> {
    use ynz_ast::nodes::{Expr, Item, Stmt};

    for item in &module.items {
        match item {
            Item::Function(f) if f.name == "main" => {
                for stmt in &f.body.stmts {
                    if let Stmt::Expr(Expr::Call(call)) = stmt {
                        if let Expr::Ident(name, _) = &call.callee {
                            if name == "print" {
                                if let Some(Expr::StringLit(bytes, _)) = call.args.first() {
                                    return bytes.clone();
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Fallback: empty string (produces `puts("")`).
    Vec::new()
}
