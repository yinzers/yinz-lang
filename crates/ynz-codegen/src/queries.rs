use std::sync::Arc;

use ynz_diagnostics::{Diagnostic, DiagnosticBucket};
use ynz_parser::{SourceFile, SourceFileRegistry};
use ynz_typeck::{check_query, module_signatures_query};

use crate::{artifact::CompiledArtifact, emit::emit_artifact};

/// The output of the codegen pass.
#[derive(Clone, Debug, PartialEq)]
pub struct CodegenOutput {
    pub artifact: CompiledArtifact,
    pub diagnostics: DiagnosticBucket,
}

/// Generate a relocatable object file for a source file.
///
/// Salsa-tracked — depends on `check_query`. Skips emission if there are
/// type errors (avoids emitting broken object files).
// lru = 32: codegen is the heaviest query; smallest cache cap to bound memory.
#[salsa::tracked(lru = 32)]
pub fn codegen_query(db: &dyn SourceFileRegistry, source: SourceFile) -> Arc<CodegenOutput> {
    let check = check_query(db, source);
    let mut diagnostics = check.diagnostics.clone();

    if diagnostics.has_errors() {
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
            diagnostics.push(Diagnostic::file_error(
                source_path.as_str(),
                format!("The compiler failed to produce machine code: {msg}"),
                "This is a compiler bug. Please report it with the source file attached.",
                "Machine-code generation failed inside the backend.",
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
