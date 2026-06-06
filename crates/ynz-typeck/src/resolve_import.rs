/// Cross-file import resolution utilities.
///
/// Resolves `import { X } from "module"` to an actual file path and builds
/// the ExportTable for that file so the importing file's typeck can see its types.
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use ynz_ast::nodes::{FunctionDecl, ImportDecl, ImportItem, ImportKind, Item};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};
use ynz_parser::{parse_query, SourceFileRegistry};

use crate::{
    check::crossing_local_names,
    exports::{collect_exports, ExportTable},
    options_table::{collect_options, OptionsEntry},
    queries::{check_query, module_signatures_query},
    shapes::{ShapeDef, ShapeTable},
    signatures::FunctionSig,
    types::Type,
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
///
/// Returns `None` when the module does not exist, when path segments are invalid
/// (caller must have already checked via `module_path_has_traversal_segments`), or
/// when the resolved path escapes the project root after canonicalization.
pub fn resolve_module_path(importer_path: &str, module_str: &str) -> Option<PathBuf> {
    let importer = Path::new(importer_path);
    let importer_dir = importer.parent()?;

    // Walk up to find yinz.toml.
    let project_root = find_project_root(importer_dir);

    let base = project_root.as_deref().unwrap_or(importer_dir);
    let candidate = base.join(format!("{module_str}.ynz"));

    if candidate.exists() {
        // Canonicalize to handle case-insensitive filesystem collisions.
        let canon = std::fs::canonicalize(&candidate).ok()?;

        // Layer 2: boundary check. Catches symlink-based escapes that the
        // segment check (layer 1, in resolve_imports) cannot see.
        let root_canon = std::fs::canonicalize(base).ok()?;
        if !canon.starts_with(&root_canon) {
            return None;
        }

        Some(canon)
    } else if project_root.is_some() {
        // Had a project root but file not found there — no fallback.
        None
    } else {
        // No project root — try relative to importer dir.
        let rel = importer_dir.join(format!("{module_str}.ynz"));
        if rel.exists() {
            let canon = std::fs::canonicalize(&rel).ok()?;
            let root_canon = std::fs::canonicalize(importer_dir).ok()?;
            if !canon.starts_with(&root_canon) {
                return None;
            }
            Some(canon)
        } else {
            None
        }
    }
}

/// Returns `true` when `module_str` contains any path segment that is exactly
/// `..` or `.`. Only exact-segment matches count — a directory named `dir.v2`
/// or `dir..weird` is not blocked because those segments are not literally `..`.
pub fn module_path_has_traversal_segments(module_str: &str) -> bool {
    module_str.split('/').any(|seg| seg == ".." || seg == ".")
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

    let importer_canonical =
        std::fs::canonicalize(importer_path).unwrap_or_else(|_| PathBuf::from(importer_path));

    for import in imports {
        let module_str = &import.source;
        let span = import.source_span.clone();

        // Layer 1: reject any segment that is exactly `..` or `.` before touching
        // the filesystem. This blocks mid-path traversal like `foo/../../etc/passwd`.
        // Only exact-segment matches are rejected — names like `dir.v2` are fine.
        if module_path_has_traversal_segments(module_str) {
            diags.push(Diagnostic::error(
                span.clone(),
                format!("Import path \"{module_str}\" contains `..` or `.` segments."),
                "Use a project-root-relative path. If you need to import from a sibling \
                 directory, use the full path from the project root (`subdir/module`), \
                 never `../sibling/module`.",
                "Yinz import paths are anchored to the project root (the directory \
                 containing `yinz.toml`). Allowing `..` would let an import escape the \
                 project boundary and could leak source from outside the project to \
                 diagnostic output.",
            ));
            continue;
        }

        // Resolve the import path. Detect the no-yinz.toml case to give a better error.
        let has_project_root = find_project_root(std::path::Path::new(importer_path)).is_some();
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
            diags.push(Diagnostic::error(
                span.clone(),
                format!("Circular import detected: \"{module_str}\" is already being imported in this chain."),
                "Restructure the modules so neither imports the other, or move the shared types to a third file that both can import.",
                "Circular imports make compilation order undefined. Extract shared types to break the cycle.",
            ));
            continue;
        }

        // Get or build the ExportTable for the imported file using the same db.
        let export_table =
            load_export_table(db, &resolved_path, module_str, &span, visiting, diags);

        // Resolve named vs namespace imports.
        match &import.kind {
            ImportKind::Named(items) => {
                for item in items {
                    bind_named_import(
                        item,
                        &export_table,
                        module_str,
                        &mut bound_names,
                        &mut result,
                        diags,
                    );
                }
            }
            ImportKind::Namespace {
                local_name,
                local_name_span,
            } => {
                let local = local_name;
                if !bound_names.insert(local.clone()) {
                    diags.push(Diagnostic::error(
                        local_name_span.clone(),
                        format!("Import name `{local}` is already bound by a previous import."),
                        "Use a different local name with `import ns as otherName from \"...\"`",
                        "Each imported name must be unique in the local scope.",
                    ));
                }
                // Store namespace imports — all exports are accessible via the namespace.
                // For typechecking purposes, we make all exported types directly available.
                for (name, def) in &export_table.shapes {
                    result.shapes.insert(format!("{local}.{name}"), def.clone());
                }
                for (name, entry) in &export_table.options {
                    result
                        .options
                        .insert(format!("{local}.{name}"), entry.clone());
                }
                for (name, sig) in &export_table.functions {
                    result
                        .functions
                        .insert(format!("{local}.{name}"), sig.clone());
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
        let mut exported_names: Vec<&str> = export_table
            .shapes
            .keys()
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
            format!(
                "Use a different local name: `import {{ {exported} as otherName }} from \"...\"`"
            ),
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

/// Estimate the number of 8-byte frame slots needed to store one value of `ty`
/// in a suspending function's composed frame, using only typeck type information.
///
/// The LLVM ABI-precise byte count (from `TargetData::get_abi_size`) is unavailable
/// at typeck time. This approximation is conservative: it matches or slightly exceeds
/// the real count, so the frame allocated is the same size or slightly larger —
/// never smaller — than what codegen computes. An over-estimated frame is safe;
/// an under-estimated one would cause out-of-bounds writes.
///
/// Rules (mirrors `shape_frame_slots` + `crossing_local_total_slots` in emit.rs):
/// - `Number { precision ≤ 34 }` (decimal128): 2 slots (16 bytes)
/// - `ErrorsCapable { .. }`: 2 slots (two i64 fields)
/// - `Shape { name }`: recursively count field slots from ShapeTable; fallback = 1
/// - All other types (int, bool, float, string, ptr-family): 1 slot (8 bytes)
fn typeck_type_frame_slots(ty: &Type, shape_table: &ShapeTable) -> usize {
    match ty {
        Type::Number { precision } if *precision <= 34 => 2,
        Type::ErrorsCapable { .. } => 2,
        Type::Shape { name } => {
            let bytes = match shape_table.get(name) {
                Some(def) => {
                    let field_bytes: u64 = def
                        .fields
                        .iter()
                        .map(|f| typeck_type_frame_slots(&f.ty, shape_table) as u64 * 8)
                        .sum();
                    field_bytes.max(8) // at minimum 8 bytes (one slot)
                }
                None => 8, // unknown shape: 1 slot fallback
            };
            bytes.div_ceil(8) as usize
        }
        _ => 1,
    }
}

/// Compute the total composed frame size (bytes) for a suspending exported function.
///
/// This is a LOSSY APPROXIMATION of `build_frame_layouts` in emit.rs. It reimplements
/// the frame-layout arithmetic using typeck-derived shape sizes rather than the LLVM
/// TargetData ABI sizes the codegen pass uses. The result is conservative (≥ real
/// codegen value) for scalar types, but breaks down for three known combos:
///
/// 1. Re-export / multi-level transitive suspension (3-module chain): the approximation
///    for the middle module's frame cannot be reconstructed at the outer boundary.
/// 2. Shape-typed crossing locals: LLVM ABI sizes differ from the typeck field-count
///    approximation for shapes with non-8-byte fields or alignment padding.
/// 3. Errors-capable export with transitive child sub-frames: the EC staging slot
///    interacts with child offsets in ways the scalar formula cannot reproduce.
///
/// These three combos are caught at import-resolution time by `is_composed_frame_simple`
/// and rejected with a clean diagnostic (exit 1) rather than silently emitting a
/// wrong-sized sub-frame that causes SIGILL or memory corruption at runtime.
///
/// M3e (v0.3-M3e: see design/future/cross-module-frame-serialization.md) replaces this
/// function by serializing the full FrameLayout into the export table, making the guard
/// unnecessary and all three combos correctly embeddable at import sites.
fn compute_composed_frame_size(
    fn_decl: &FunctionDecl,
    all_items: &[ynz_ast::nodes::Item],
    suspends_set: &HashSet<String>,
    shape_table: &ShapeTable,
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
    visited: &mut HashSet<String>,
    memo: &mut std::collections::HashMap<String, u64>,
) -> u64 {
    // Frame header is always 32 bytes.
    const FRAME_HEADER_SIZE: u64 = 32;

    if let Some(&cached) = memo.get(&fn_decl.name) {
        return cached;
    }
    // Cycle guard: if we're currently computing this function's size, return
    // FRAME_HEADER_SIZE as a placeholder (the cycle will resolve on unwind).
    if !visited.insert(fn_decl.name.clone()) {
        return FRAME_HEADER_SIZE;
    }

    // Compute crossing locals.
    let param_names: Vec<&str> = fn_decl.params.iter().map(|p| p.name.as_str()).collect();
    let suspending_refs: HashSet<&str> = suspends_set.iter().map(|s| s.as_str()).collect();
    let crossings = crossing_local_names(
        &fn_decl.body.stmts,
        &param_names,
        &suspending_refs,
        expr_types,
    );

    // Count slots: params (1 slot each) + crossing locals (type-dependent).
    let n_params = fn_decl.params.len();
    let crossing_slots: usize = crossings
        .iter()
        .map(|cname| {
            // Look up the binding's typeck type via expr_types. Walking the AST stmts
            // to find the let binding's expr span is expensive; use the conservative
            // fallback (1 slot) unless the name appears in expr_types as a Number type
            // or ErrorsCapable type. Shapes are approximated by ShapeTable field count.
            // Exact correctness is not required — only the conservative (≥ real) property.
            let ty =
                find_let_typeck_type(&fn_decl.body.stmts, cname.as_str(), expr_types, shape_table);
            typeck_type_frame_slots(&ty, shape_table)
        })
        .sum();

    let n_locals = n_params + crossing_slots;
    let own_base = FRAME_HEADER_SIZE + (n_locals as u64) * 8;

    // Optional 16-byte staging slot for `-> number errors` functions.
    let staging_extra: u64 = if matches!(fn_decl.return_type, ynz_ast::nodes::Type::Number { .. })
        && fn_decl.errors_capable
    {
        16
    } else {
        0
    };

    let children_start = own_base + staging_extra;

    // Collect direct suspending callees, then sum their composed frame sizes.
    let callee_names = collect_direct_suspending_callees(&fn_decl.body.stmts, suspends_set);
    let mut cursor = children_start;
    for callee_name in &callee_names {
        // Skip self-recursion (infinite loop guard) and already-visited nodes.
        if callee_name == &fn_decl.name {
            continue;
        }
        // Find the callee FunctionDecl in the module items.
        let callee_decl = all_items.iter().find_map(|item| {
            if let Item::Function(f) = item {
                if f.name == *callee_name && suspends_set.contains(f.name.as_str()) {
                    return Some(f);
                }
            }
            None
        });
        if let Some(callee) = callee_decl {
            let child_size = compute_composed_frame_size(
                callee,
                all_items,
                suspends_set,
                shape_table,
                expr_types,
                visited,
                memo,
            );
            cursor += child_size;
        }
        // Imported callee (not in all_items): its composed_frame_size was set when
        // WE were loaded as an export. At this point the imported callee's check_query
        // already ran; we don't recurse into foreign modules here — the importer's
        // build_frame_layouts uses sig.composed_frame_size for those.
        // Skip: use FRAME_HEADER_SIZE placeholder for unknown callees (same as the
        // old codegen behaviour before this fix — at minimum we handle the local callees).
    }

    let total = cursor;
    memo.insert(fn_decl.name.clone(), total);
    visited.remove(&fn_decl.name);
    total
}

/// Walk expression AST to collect names of directly-called suspending functions.
/// Deduplicates by name (same callee called twice → one child sub-frame).
fn collect_direct_suspending_callees(
    stmts: &[ynz_ast::nodes::Stmt],
    suspends_set: &HashSet<String>,
) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for stmt in stmts {
        collect_callees_in_stmt(stmt, suspends_set, &mut seen, &mut out);
    }
    out
}

fn collect_callees_in_stmt(
    stmt: &ynz_ast::nodes::Stmt,
    suspends_set: &HashSet<String>,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    use ynz_ast::nodes::Stmt;
    match stmt {
        Stmt::Expr(value) => {
            collect_callees_in_expr(value, suspends_set, seen, out);
        }
        Stmt::Let { value, .. } => {
            collect_callees_in_expr(value, suspends_set, seen, out);
        }
        Stmt::Return { value: Some(e), .. } => {
            collect_callees_in_expr(e, suspends_set, seen, out);
        }
        Stmt::Return { value: None, .. } => {}
        Stmt::Assign { value, .. } => collect_callees_in_expr(value, suspends_set, seen, out),
        Stmt::FieldAssign { target, value, .. } => {
            collect_callees_in_expr(target, suspends_set, seen, out);
            collect_callees_in_expr(value, suspends_set, seen, out);
        }
        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            collect_callees_in_expr(receiver, suspends_set, seen, out);
            collect_callees_in_expr(index, suspends_set, seen, out);
            collect_callees_in_expr(value, suspends_set, seen, out);
        }
        Stmt::If { cond, body, .. } => {
            collect_callees_in_expr(cond, suspends_set, seen, out);
            for s in &body.stmts {
                collect_callees_in_stmt(s, suspends_set, seen, out);
            }
        }
        Stmt::While { cond, body, .. } => {
            collect_callees_in_expr(cond, suspends_set, seen, out);
            for s in &body.stmts {
                collect_callees_in_stmt(s, suspends_set, seen, out);
            }
        }
        Stmt::For { iter, body, .. } => {
            collect_callees_in_expr(iter, suspends_set, seen, out);
            for s in &body.stmts {
                collect_callees_in_stmt(s, suspends_set, seen, out);
            }
        }
        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            collect_callees_in_expr(scrutinee, suspends_set, seen, out);
            for arm in arms {
                for s in &arm.body.stmts {
                    collect_callees_in_stmt(s, suspends_set, seen, out);
                }
            }
            if let Some(eb) = else_arm {
                for s in &eb.stmts {
                    collect_callees_in_stmt(s, suspends_set, seen, out);
                }
            }
        }
    }
}

fn collect_callees_in_expr(
    expr: &ynz_ast::nodes::Expr,
    suspends_set: &HashSet<String>,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    use ynz_ast::nodes::Expr;
    match expr {
        Expr::Call(call_expr) => {
            if let Expr::Ident(name, _) = &call_expr.callee {
                if suspends_set.contains(name.as_str()) && seen.insert(name.clone()) {
                    out.push(name.clone());
                }
            }
            for arg in &call_expr.args {
                collect_callees_in_expr(arg, suspends_set, seen, out);
            }
            collect_callees_in_expr(&call_expr.callee, suspends_set, seen, out);
        }
        Expr::Wait(inner, _) => collect_callees_in_expr(inner, suspends_set, seen, out),
        Expr::BinOp { lhs, rhs, .. } => {
            collect_callees_in_expr(lhs, suspends_set, seen, out);
            collect_callees_in_expr(rhs, suspends_set, seen, out);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_callees_in_expr(operand, suspends_set, seen, out);
        }
        Expr::FieldAccess { receiver, .. } | Expr::PostfixOp { receiver, .. } => {
            collect_callees_in_expr(receiver, suspends_set, seen, out);
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_callees_in_expr(receiver, suspends_set, seen, out);
            for arg in args {
                collect_callees_in_expr(arg, suspends_set, seen, out);
            }
        }
        Expr::IndexAccess {
            receiver, index, ..
        } => {
            collect_callees_in_expr(receiver, suspends_set, seen, out);
            collect_callees_in_expr(index, suspends_set, seen, out);
        }
        _ => {}
    }
}

/// Look up a let-binding's type from stmts (shallow scan, not recursive into blocks).
/// Returns a conservative Type::Int fallback (1 slot) if not found.
fn find_let_typeck_type(
    stmts: &[ynz_ast::nodes::Stmt],
    name: &str,
    expr_types: &std::collections::HashMap<(usize, usize), Type>,
    _shape_table: &ShapeTable,
) -> Type {
    use ynz_ast::nodes::Stmt;
    for stmt in stmts {
        if let Stmt::Let { name: n, value, .. } = stmt {
            if n == name {
                let key = (value.span().start, value.span().end);
                if let Some(ty) = expr_types.get(&key) {
                    return ty.clone();
                }
            }
        }
    }
    Type::Int // 1-slot conservative fallback
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

    // Parse the importee FIRST (before module_signatures_query) so we can detect
    // circular imports before salsa enters a dependency cycle. module_signatures_query
    // recursively resolves imports — if the importee imports back a file already in
    // `visiting`, salsa would panic with an unhandled dependency cycle. Checking the
    // import declarations here lets us emit a clean error and return early.
    let parse = parse_query(db, sf);

    // Propagate parse errors as a summary diagnostic rather than re-emitting
    // every individual error under the importer's path.
    let parse_errors: Vec<_> = parse
        .diagnostics
        .iter()
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

    // Scan the importee's import declarations for any path that is already in the
    // visiting set. A match means this import chain would form a cycle: A imports B
    // and B (directly or transitively) imports A. Salsa cannot resolve such a cycle
    // because module_signatures_query is tracked — the second call on an in-progress
    // query panics. Detect it here and emit a clean diagnostic.
    for item in &parse.module.items {
        let ynz_ast::nodes::Item::ImportDecl(decl) = item else {
            continue;
        };
        let resolved_path_str = resolved_path.display().to_string();
        let back_path = match resolve_module_path(&resolved_path_str, &decl.source) {
            Some(p) => p,
            None => continue,
        };
        if visiting.contains(&back_path) {
            diags.push(Diagnostic::error(
                span.clone(),
                format!(
                    "Circular import: \"{module_str}\" imports back into the current module chain."
                ),
                "Move the shared definitions to a third file that both modules can import \
                 without creating a cycle.",
                "Circular imports create an unresolvable dependency order — the compiler \
                 cannot determine which module to compile first. Extract the shared types, \
                 functions, or shapes into a new file and import that file from both modules.",
            ));
            visiting.remove(resolved_path);
            return ExportTable::empty();
        }
    }

    // Use the salsa-memoized signature query — shapes and function signatures
    // are computed once and cached; salsa re-runs only when the source changes.
    let sig_output = module_signatures_query(db, sf);

    // OptionsTable isn't part of SignatureOutput, so we collect it inline here.
    // This pass is uncached; the shape/sig tables above are memoized via the
    // tracked query, so this is the only repeated work for cross-file imports.
    let mut dummy_diags = DiagnosticBucket::new();
    let options_table = collect_options(&parse.module, &mut dummy_diags);

    visiting.remove(resolved_path);

    // Build the export table using the pre-analysis sig_table for shape and options
    // resolution, then overwrite the `suspends` flag on each exported function with
    // the result from check_query. check_query runs the may-block fixpoint and
    // populates `suspends_set` — that is the authoritative per-file suspension
    // analysis. The sig_table from module_signatures_query always has suspends=false
    // (the analysis runs later in check_query), so without this correction every
    // imported function would appear non-suspending regardless of its body.
    //
    // Calling check_query here creates a salsa dependency: the importer's
    // module_signatures_query re-runs whenever the importee's check_query result
    // changes (e.g., the importee gains or loses a sleep call). That is the
    // correct incremental behaviour — if A's slow() gains a sleep, B's
    // module_signatures_query must re-run to see the updated suspends=true.
    let check_out = check_query(db, sf);
    let mut export_table = collect_exports(
        &parse.module,
        &sig_output.shape_table,
        &options_table,
        &sig_output.sig_table,
    );
    // Compute per-function frame sizes so the importing module's build_frame_layouts
    // can embed sub-frames at the real size instead of the FRAME_HEADER_SIZE placeholder.
    // composed_frame_size is still used by the importer's codegen (emit.rs) to size the
    // embedded sub-frame slot — it is the only cross-module frame metadata until M3e
    // ships full FrameLayout serialization.
    //
    // The typeck-side loud-reject guard (queries.rs) now universally rejects ALL calls to
    // imported suspending functions, so the composed_frame_size value is only reached for
    // SAME-MODULE suspending callees — the intra-module transitive case that works correctly.
    let mut frame_size_memo: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    for (name, sig) in export_table.functions.iter_mut() {
        sig.suspends = check_out.suspends_set.contains(name.as_str());
        if sig.suspends {
            // Find the FunctionDecl for this exported function.
            if let Some(fn_decl) = parse.module.items.iter().find_map(|item| {
                if let Item::Function(f) = item {
                    if f.name == *name {
                        return Some(f);
                    }
                }
                None
            }) {
                let mut visited: HashSet<String> = HashSet::new();
                sig.composed_frame_size = compute_composed_frame_size(
                    fn_decl,
                    &parse.module.items,
                    &check_out.suspends_set,
                    &sig_output.shape_table,
                    &check_out.typed_module.expr_types,
                    &mut visited,
                    &mut frame_size_memo,
                );
            }
        }
    }
    export_table
}
