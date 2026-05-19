use std::sync::Arc;

use ynz_diagnostics::Diagnostic;
use ynz_parser::SourceFile;
use ynz_typeck::{check_query, module_signatures_query};

use crate::{artifact::CompiledArtifact, emit::emit_artifact};

/// The output of the codegen pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CodegenOutput {
    pub artifact: CompiledArtifact,
    pub diagnostics: Vec<Diagnostic>,
}

/// Generate a relocatable object file for a source file.
///
/// Salsa-tracked — depends on `check_query`. Skips emission if there are
/// type errors (avoids emitting broken object files).
#[salsa::tracked]
pub fn codegen_query(db: &dyn salsa::Database, source: SourceFile) -> Arc<CodegenOutput> {
    let check = check_query(db, source);
    let mut diagnostics = check.diagnostics.clone();

    let has_errors = check
        .diagnostics
        .iter()
        .any(|d| d.severity == ynz_diagnostics::Severity::Error);

    if has_errors {
        return Arc::new(CodegenOutput {
            artifact: CompiledArtifact {
                object_bytes: Vec::new(),
                ir_text: String::new(),
                sha256: [0u8; 32],
            },
            diagnostics,
        });
    }

    let sig_output = module_signatures_query(db, source);
    let source_path = source.path(db);
    match emit_artifact(
        source_path.as_str(),
        &check.typed_module,
        &sig_output.shape_table,
        &sig_output.sig_table,
        &sig_output.generic_fn_table,
        &check.mono_table,
        None,
        &sig_output.imported_options,
    ) {
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
