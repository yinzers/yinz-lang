/// Cross-file import resolution utilities.
///
/// Resolves `import { X } from "module"` to an actual file path and builds
/// the ExportTable for that file so the importing file's typeck can see its types.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use ynz_ast::nodes::{ImportDecl, ImportItem, ImportKind};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};
use ynz_parser::{parse_query, SourceFile};

use crate::{
    exports::{collect_exports, ExportTable},
    options_table::collect_options,
    shapes::{collect_shapes, ShapeDef},
    options_table::OptionsEntry,
    signatures::{collect_signatures, FunctionSig},
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
    db: &dyn salsa::Database,
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

        // Resolve the import path.
        let Some(resolved_path) = resolve_module_path(importer_path, module_str) else {
            diags.push(Diagnostic::error(
                span.clone(),
                format!("Module \"{module_str}\" not found."),
                format!("Check the module name and ensure the file exists at `{module_str}.ynz`. Paths are project-root-relative without the `.ynz` suffix."),
                "Import paths point to `.ynz` files in the project. For a project with `yinz.toml`, paths are relative to the project root.",
            ));
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

/// Load (or use cached) ExportTable for a file at `resolved_path`.
///
/// Uses the same `db` so salsa can track the cross-file dependency. If the
/// file was registered by the driver (`db.source_by_path`), we get the exact
/// same SourceFile input and salsa memoizes correctly. If not registered
/// (single-file mode), we create a fresh SourceFile — results are correct but
/// not incrementally cached.
fn load_export_table(
    db: &dyn salsa::Database,
    resolved_path: &Path,
    module_str: &str,
    span: &SourceSpan,
    visiting: &mut HashSet<PathBuf>,
    diags: &mut DiagnosticBucket,
) -> ExportTable {
    let path_str = resolved_path.display().to_string();

    // Read the imported file from disk and create a SourceFile in the SAME salsa db.
    // Using the same db is mandatory — creating a separate db inside a salsa tracked
    // function panics ("Cannot change database mid-query").
    // Incremental optimization (v0.2): use the pre-registered SourceFile from the
    // driver's project load via CompilerDb::source_by_path so salsa tracks the
    // cross-file dependency. For now, reading fresh from disk is correct but not cached.
    let sf = match std::fs::read_to_string(resolved_path) {
        Ok(text) => SourceFile::new(db, path_str.clone(), text),
        Err(e) => {
            diags.push(Diagnostic::error(
                span.clone(),
                format!("Cannot read module \"{module_str}\": {e}."),
                "Check file permissions and that the file exists.",
                "The compiler must be able to read all imported files.",
            ));
            return ExportTable::empty();
        }
    };

    // Mark as visiting to detect circular imports in recursive calls.
    visiting.insert(resolved_path.to_path_buf());

    // Build the imported file's type tables.
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

    let mut dummy_diags = DiagnosticBucket::new();
    let shape_table = collect_shapes(&parse.module, &Default::default(), &Default::default(), &mut dummy_diags);
    let options_table = collect_options(&parse.module, &mut dummy_diags);
    let sig_table = collect_signatures(&parse.module, &mut dummy_diags, &shape_table);

    visiting.remove(resolved_path);

    collect_exports(&parse.module, &shape_table, &options_table, &sig_table)
}
