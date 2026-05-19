/// Cross-file import resolution utilities.
///
/// Resolves `import { X } from "module"` to an actual file path and builds
/// the ExportTable for that file so the importing file's typeck can see its types.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use ynz_ast::nodes::{ImportDecl, ImportItem, ImportKind};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};
use ynz_parser::{parse_query, SourceFileRegistry};

use crate::{
    exports::{collect_exports, ExportTable},
    options_table::{collect_options, OptionsEntry},
    queries::module_signatures_query,
    shapes::ShapeDef,
    signatures::FunctionSig,
};

/// Result of resolving a single `import { ... } from "path"` declaration.
pub struct ResolvedImport {
    /// Local name → ShapeDef for imported shapes.
    pub shapes: std::collections::HashMap<String, ShapeDef>,
    /// Local name → OptionsEntry for imported options types.
    pub options: std::collections::HashMap<String, OptionsEntry>,
    /// Local name → FunctionSig for imported functions.
    pub functions: std::collections::HashMap<String, FunctionSig>,
}

impl ResolvedImport {
    pub fn empty() -> Self {
        Self {
            shapes: Default::default(),
            options: Default::default(),
            functions: Default::default(),
        }
    }
}

/// Resolve the file path for a module string like `"services/users"`.
///
/// Strategy:
/// 1. Walk up from `importer_path`'s directory to find `yinz.toml` → project root
/// 2. Resolve: `<project_root>/<module_str>.ynz`
/// 3. Fallback: `<importer_dir>/<module_str>.ynz` (single-file projects)
/// 4. Canonicalize via `std::fs::canonicalize` to detect case collisions on
///    case-insensitive filesystems (macOS, Windows).
pub fn resolve_module_path(importer_path: &str, module_str: &str) -> Option<PathBuf> {
    let importer = Path::new(importer_path);
    let importer_dir = importer.parent()?;

    // Walk up to find yinz.toml.
    let project_root = find_project_root(importer_dir);

    let base = project_root.as_deref().unwrap_or(importer_dir);
    let candidate = base.join(format!("{module_str}.ynz"));

    if candidate.exists() {
        // Canonicalize to handle case-insensitive filesystem collisions.
        std::fs::canonicalize(&candidate).ok()
    } else if project_root.is_some() {
        // Had a project root but file not found there — no fallback.
        None
    } else {
        // No project root — try relative to importer dir.
        let rel = importer_dir.join(format!("{module_str}.ynz"));
        if rel.exists() { std::fs::canonicalize(&rel).ok() } else { None }
    }
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join("yinz.toml").exists() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Resolve all `import` declarations in a module, returning merged imported symbols.
///
/// Emits diagnostics for:
/// - Missing module file
/// - Symbol not exported from target
/// - Duplicate local binding
/// - Self-import
/// - Circular import (detected via `visiting` set of canonical paths)
pub fn resolve_imports(
    imports: &[&ImportDecl],
    importer_path: &str,
    db: &dyn SourceFileRegistry,
    visiting: &mut HashSet<PathBuf>,
    diags: &mut DiagnosticBucket,
) -> ResolvedImport {
    let mut result = ResolvedImport::empty();
    let mut bound_names: std::collections::HashSet<String> = Default::default();

    let importer_canonical = std::fs::canonicalize(importer_path)
        .unwrap_or_else(|_| PathBuf::from(importer_path));

    for import in imports {
        let module_str = &import.source;
        let span = import.source_span.clone();

        // Resolve the import path. Detect the no-yinz.toml case to give a better error.
        let has_project_root = find_project_root(
            std::path::Path::new(importer_path).parent().unwrap_or(std::path::Path::new("."))
        ).is_some();
        let Some(resolved_path) = resolve_module_path(importer_path, module_str) else {
            if !has_project_root && module_str.contains('/') {
                // Cross-directory import with no project root — the most common cause.
                diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Module \"{module_str}\" not found — no `yinz.toml` in any parent directory."),
                    "Add a `yinz.toml` at your project root so the compiler knows where to look. Single-file imports work without it; cross-directory imports need it.",
                    "Cross-directory imports use paths relative to the `yinz.toml` location. Same-directory imports work without one.",
                ));
            } else {
                diags.push(Diagnostic::error(
                    span.clone(),
                    format!("Module \"{module_str}\" not found."),
                    format!("Check that `{module_str}.ynz` exists at the project root. Paths are project-root-relative without the `.ynz` suffix."),
                    "Import paths point to `.ynz` files relative to the `yinz.toml` location.",
                ));
            }
            continue;
        };

        // Self-import check.
        if resolved_path == importer_canonical {
            diags.push(Diagnostic::error(
                span.clone(),
                "A file cannot import from itself.",
                "Remove this import, or move the shared types to a separate file.",
                "Self-imports create a circular dependency with no resolution.",
            ));
            continue;
        }

        // Circular import detection.
        if visiting.contains(&resolved_path) {
            let cycle_path = resolved_path.display().to_string();
            diags.push(Diagnostic::error(
                span.clone(),
                format!("Circular import detected: \"{module_str}\" is already being imported in this chain."),
                "Restructure the modules so neither imports the other, or move the shared types to a third file that both can import.",
                "Circular imports make compilation order undefined. Extract shared types to break the cycle.",
            ));
            continue;
        }

        // Get or build the ExportTable for the imported file using the same db.
        let export_table = load_export_table(db, &resolved_path, module_str, &span, visiting, diags);

        // Resolve named vs namespace imports.
        match &import.kind {
            ImportKind::Named(items) => {
                for item in items {
                    bind_named_import(item, &export_table, module_str, &mut bound_names, &mut result, diags);
                }
            }
            ImportKind::Namespace { local_name, local_name_span } => {
                let local = local_name;
                if !bound_names.insert(local.clone()) {
                    diags.push(Diagnostic::error(
                        local_name_span.clone(),
                        format!("Import name `{local}` is already bound by a previous import."),
                        "Use a different local name with `import ns as alias from \"...\"`",
                        "Each imported name must be unique in the local scope.",
                    ));
                }
                // Store namespace imports — all exports are accessible via the namespace.
                // For typechecking purposes, we make all exported types directly available.
                for (name, def) in &export_table.shapes {
                    result.shapes.insert(format!("{local}.{name}"), def.clone());
                }
                for (name, entry) in &export_table.options {
                    result.options.insert(format!("{local}.{name}"), entry.clone());
                }
                for (name, sig) in &export_table.functions {
                    result.functions.insert(format!("{local}.{name}"), sig.clone());
                }
            }
        }
    }

    result
}

fn bind_named_import(
    item: &ImportItem,
    export_table: &ExportTable,
    module_str: &str,
    bound_names: &mut std::collections::HashSet<String>,
    result: &mut ResolvedImport,
    diags: &mut DiagnosticBucket,
) {
    let exported = &item.exported_name;
    let local = &item.local_name;
    let span = item.exported_name_span.clone();

    // Check symbol exists in export table.
    let found_shape = export_table.shapes.get(exported);
    let found_options = export_table.options.get(exported);
    let found_fn = export_table.functions.get(exported);

    if found_shape.is_none() && found_options.is_none() && found_fn.is_none() {
        let mut exported_names: Vec<&str> = export_table.shapes.keys()
            .chain(export_table.options.keys())
            .chain(export_table.functions.keys())
            .map(|s| s.as_str())
            .collect();
        exported_names.sort();
        let names_list = if exported_names.is_empty() {
            "nothing is exported from this module".to_string()
        } else {
            format!("Exported names: {}", exported_names.join(", "))
        };
        diags.push(Diagnostic::error(
            span,
            format!("`{exported}` is not exported from \"{module_str}\"."),
            format!("Check the name and add `export` before the declaration in {module_str}.ynz. {names_list}."),
            "Only declarations prefixed with `export` are visible to other files.",
        ));
        return;
    }

    // Check duplicate local binding.
    if !bound_names.insert(local.clone()) {
        diags.push(Diagnostic::error(
            span,
            format!("Import name `{local}` is already bound by a previous import."),
            "Use a different local name with `import {{ {exported} as alias }} from \"...\"`",
            "Each imported name must be unique in the local scope.",
        ));
        return;
    }

    // Bind the symbol.
    if let Some(def) = found_shape {
        result.shapes.insert(local.clone(), def.clone());
    }
    if let Some(entry) = found_options {
        result.options.insert(local.clone(), entry.clone());
    }
    if let Some(sig) = found_fn {
        result.functions.insert(local.clone(), sig.clone());
    }
}

/// Load (or build) the ExportTable for a file at `resolved_path`.
///
/// Looks up the SourceFile via `db.source_by_path` so salsa tracks the
/// cross-file dependency and uses the exact in-memory text (not stale disk
/// content). Files not registered in the project (single-file mode) return an
/// error diagnostic — cross-file imports require a `yinz.toml` project.
///
/// Uses the memoized `module_signatures_query` for shapes and signatures so
/// salsa can skip re-running those passes on unchanged imported files.
fn load_export_table(
    db: &dyn SourceFileRegistry,
    resolved_path: &Path,
    module_str: &str,
    span: &SourceSpan,
    visiting: &mut HashSet<PathBuf>,
    diags: &mut DiagnosticBucket,
) -> ExportTable {
    let path_str = resolved_path.display().to_string();

    // Look up the SourceFile from the salsa registry. The driver registers all
    // project files before any query runs, so every valid import is present here.
    // Files missing from the registry were not part of the project load —
    // single-file mode cannot import across files.
    let sf = match db.source_by_path(&path_str) {
        Some(sf) => sf,
        None => {
            diags.push(Diagnostic::error(
                span.clone(),
                format!("Module \"{module_str}\" was not registered in the project."),
                "Check that the file exists under your project root and that `yinz.toml` is present.",
                "Yinz only resolves imports inside projects (a directory with `yinz.toml`). \
                 Compiling a single file with `ynz build foo.ynz` has no project context — \
                 add a `yinz.toml` and put your files under that directory if you need imports.",
            ));
            return ExportTable::empty();
        }
    };

    // Mark as visiting to detect circular imports in recursive calls.
    visiting.insert(resolved_path.to_path_buf());

    // Use the salsa-memoized signature query — shapes and function signatures
    // are computed once and cached; salsa re-runs only when the source changes.
    let sig_output = module_signatures_query(db, sf);
    let parse = parse_query(db, sf);

    // Propagate parse errors as a summary diagnostic rather than re-emitting
    // every individual error under the importer's path.
    let parse_errors: Vec<_> = parse.diagnostics.iter()
        .filter(|d| d.severity == ynz_diagnostics::Severity::Error)
        .collect();
    if !parse_errors.is_empty() {
        diags.push(Diagnostic::error(
            span.clone(),
            format!("Module \"{module_str}\" has parse errors — fix those first."),
            format!("Run `ynz build {module_str}.ynz` to see the errors in that file."),
            "The compiler cannot resolve exports from a file with parse errors.",
        ));
        visiting.remove(resolved_path);
        return ExportTable::empty();
    }

    // OptionsTable isn't part of SignatureOutput, so we collect it inline here.
    // This pass is uncached; the shape/sig tables above are memoized via the
    // tracked query, so this is the only repeated work for cross-file imports.
    let mut dummy_diags = DiagnosticBucket::new();
    let options_table = collect_options(&parse.module, &mut dummy_diags);

    visiting.remove(resolved_path);

    collect_exports(&parse.module, &sig_output.shape_table, &options_table, &sig_output.sig_table)
}
