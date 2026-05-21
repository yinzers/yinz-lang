//! Symbol lookup helpers for the LSP go-to-def / find-refs / rename requests.
//!
//! All five public functions are salsa-tracked so repeated LSP requests at the
//! same offset reuse the computed result.  The cache is invalidated automatically
//! when any source file changes.
//!
//! # The lookup order
//!
//! For any identifier at a given byte offset in a source file, resolution walks:
//! 1. Local scope — `let`/`const`/`for` bindings and function parameters declared
//!    before the offset in the enclosing function body.
//! 2. File-level declarations — shapes, options, functions, top-level consts
//!    declared in the same source file (including symbols merged in from imports
//!    by `module_signatures_query`; origin is resolved via `decl_span.file`).
//! 3. Imported symbols — names brought into scope via `import { ... } from "path"`.
//!
//! The first match wins.  If no match is found, `resolve_symbol_at` returns `None`.
use ynz_ast::nodes::{Block, Expr, FunctionDecl, Item, Stmt};
use ynz_diagnostics::SourceSpan;
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};
use ynz_registry::{banned_declaration_keyword_lookup, banned_jargon_lookup, keywords};

use crate::{
    ast_offset::{identifier_use_site_at_offset, span_contains},
    options_table::collect_options,
    queries::module_signatures_query,
};

// ─────────────────────────────────────────────────────────────────────────────
// Public API types
// ─────────────────────────────────────────────────────────────────────────────

/// The kind of a resolved symbol — distinguishes hover content rendering and rename eligibility.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Shape,
    Function,
    Options,
    OptionVariant,
    Const,
    LocalLet,
    LocalConst,
    Parameter,
}

/// A symbol resolved to its canonical declaration location.
///
/// Does NOT derive `Debug` because `SourceFile` (salsa input) does not implement Debug.
/// Use `sym.name` and `sym.kind` for display; compare `sym.origin_file` via `PartialEq`.
#[derive(Clone, PartialEq)]
pub struct ResolvedSymbol {
    pub kind: SymbolKind,
    /// The file that declares this symbol (may differ from the queried file).
    pub origin_file: SourceFile,
    /// The byte span of the declaration in `origin_file`.
    pub decl_span: SourceSpan,
    pub name: String,
}

/// Why a rename request was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameError {
    /// The cursor is not on a renameable symbol (keyword, literal, punctuation).
    NotARenameable,
    /// The proposed new name is a Yinz reserved keyword.
    ///
    /// WHAT INSTEAD: choose a name that is not a Yinz keyword.
    /// WHY: keywords have special meaning in the grammar; using one as an identifier
    /// would make the file unparseable.
    NewNameIsReservedKeyword(String),
    /// The proposed new name appears in the banned-jargon list or the banned-declaration-keyword list.
    ///
    /// WHAT INSTEAD: use the Yinz term (registry entry for the replacement; second `String` field).
    /// WHY: banned words either are reserved for diagnostic messages (jargon) or trigger
    /// special lexer behavior (declaration keywords), making them unusable as identifiers.
    NewNameIsBannedJargon(String, String),
    /// The proposed new name is not a valid Yinz identifier.
    ///
    /// WHAT INSTEAD: identifiers start with a letter and contain only letters,
    /// digits, and underscores (`[a-zA-Z][a-zA-Z0-9_]*`).
    /// WHY: the parser would reject the file if a non-identifier were used here.
    NewNameInvalidIdentifier(String),
    /// The symbol is declared in another file; rename must be triggered from that file.
    ///
    /// WHAT INSTEAD: open the origin file and trigger rename from the declaration site.
    /// WHY: the origin file owns the name; renaming from an import site renames only
    /// the import binding, not the canonical declaration the symbol refers to.
    CannotRenameImportedSymbolInThisFile(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Salsa-tracked public queries
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the symbol at `byte_offset` in `source` to its canonical declaration.
///
/// Time: O(n) AST walk + O(imports) cross-file lookups on cache miss.
/// Space: O(1) on cache hit; O(import depth) on cache miss.
#[salsa::tracked]
pub fn resolve_symbol_at(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    byte_offset: usize,
) -> Option<ResolvedSymbol> {
    let parse = parse_query(db, source);
    let source_text = source.text(db);

    if byte_offset >= source_text.len() {
        return None;
    }

    let (name, _span) = identifier_use_site_at_offset(&parse.module, byte_offset)?;

    // Build the symbol tables for this file.
    let sig_output = module_signatures_query(db, source);

    // 1. Local scope: walk all function bodies to find let/const/param bindings.
    for item in &parse.module.items {
        if let Item::Function(f) = item {
            if !span_contains(&f.span, byte_offset) {
                continue;
            }
            if let Some(sym) = resolve_local(&name, f, byte_offset, source, db) {
                return Some(sym);
            }
        }
    }

    // 2. File-level declarations: shapes, options, functions, consts.
    // `sig_output.shape_table` and `sig_output.sig_table` include BOTH locally declared
    // AND imported symbols (module_signatures_query merges imported symbols into the same
    // tables so typeck can see them). Distinguish origin by comparing the decl_span's
    // file path against the current source file's path — if they differ, look up the
    // actual origin SourceFile from the db.
    let source_path = source.path(db);

    let origin_sf_for_span = |span: &SourceSpan| -> SourceFile {
        if span.file == source_path || span.file.is_empty() {
            source
        } else {
            db.source_by_path(&span.file).unwrap_or(source)
        }
    };

    if let Some(def) = sig_output.shape_table.get(&name) {
        let origin_file = origin_sf_for_span(&def.defined_at);
        return Some(ResolvedSymbol {
            kind: SymbolKind::Shape,
            origin_file,
            decl_span: def.defined_at.clone(),
            name,
        });
    }

    if let Some(sig) = sig_output.sig_table.fns.get(&name) {
        let origin_file = origin_sf_for_span(&sig.decl_span);
        return Some(ResolvedSymbol {
            kind: SymbolKind::Function,
            origin_file,
            decl_span: sig.decl_span.clone(),
            name,
        });
    }

    // Check options types and their variants.
    // Diagnostics discarded intentionally: this is a read-only lookup path;
    // check_query owns diagnostic emission for the same module.
    let mut dummy_diags = ynz_diagnostics::DiagnosticBucket::new();
    let options_table = collect_options(&parse.module, &mut dummy_diags);

    if let Some(entry) = options_table.get(&name) {
        return Some(ResolvedSymbol {
            kind: SymbolKind::Options,
            origin_file: source,
            decl_span: entry.decl_span.clone(),
            name,
        });
    }

    // Check options variants by walking the AST OptionsDecl items (the OptionsEntry
    // stores only names; variant spans live in the AST).
    for item in &parse.module.items {
        if let Item::OptionsDecl(o) = item {
            for (variant_name, variant_span, _) in &o.variants {
                if variant_name == &name {
                    return Some(ResolvedSymbol {
                        kind: SymbolKind::OptionVariant,
                        origin_file: source,
                        decl_span: variant_span.clone(),
                        name,
                    });
                }
            }
        }
    }

    // Top-level consts.
    for item in &parse.module.items {
        if let Item::ConstDecl(c) = item {
            if c.name == name {
                return Some(ResolvedSymbol {
                    kind: SymbolKind::Const,
                    origin_file: source,
                    decl_span: c.name_span.clone(),
                    name,
                });
            }
        }
    }

    // 3. Imports.
    if let Some(sym) = resolve_via_imports(&name, db, source, &source_path) {
        return Some(sym);
    }

    None
}

/// Return the declaration location of the symbol at `byte_offset`.
///
/// Thin wrapper over `resolve_symbol_at` that drops the kind and returns just
/// the `(origin_file, decl_span)` pair the LSP go-to-def handler needs.
///
/// Time: same as `resolve_symbol_at`.
#[salsa::tracked]
pub fn def_site_for_offset(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    byte_offset: usize,
) -> Option<(SourceFile, SourceSpan)> {
    resolve_symbol_at(db, source, byte_offset)
        .map(|sym| (sym.origin_file, sym.decl_span))
}

/// Find every use-site of the symbol at `byte_offset` across all registered files.
///
/// `include_decl: true` adds the declaration span to the returned list.
/// `include_decl: false` returns only use-sites (not the declaration itself).
///
/// Time: O(files × AST size) on cache miss.
/// Space: O(results) for the returned vec.
#[salsa::tracked]
pub fn references_for_offset(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    byte_offset: usize,
    include_decl: bool,
) -> Vec<(SourceFile, SourceSpan)> {
    let Some(canonical) = resolve_symbol_at(db, source, byte_offset) else {
        return vec![];
    };

    let mut results: Vec<(SourceFile, SourceSpan)> = vec![];

    if include_decl {
        results.push((canonical.origin_file, canonical.decl_span.clone()));
    }

    // Walk all registered files, collecting use-sites that resolve to the SAME canonical symbol.
    // Each candidate Ident is re-resolved via `resolve_symbol_at` so shadowing is handled
    // correctly: an inner-scope `let count = 2` shadowing an outer `let count = 1` will
    // produce a DIFFERENT `ResolvedSymbol` (different `decl_span`) and be excluded.
    // Perf: `resolve_symbol_at` is salsa-tracked per (source, byte_offset); repeated
    // calls for the same offset within a session are cache hits.
    let all_paths = db.all_source_paths();
    for path in all_paths {
        let Some(sf) = db.source_by_path(&path) else {
            continue;
        };
        let parse = parse_query(db, sf);
        collect_use_sites_in_module(&parse.module, &canonical, sf, db, &mut results);
    }

    // Remove the declaration span itself from use-sites (already added by include_decl above).
    if !include_decl {
        results.retain(|(sf, span)| {
            !(*sf == canonical.origin_file && *span == canonical.decl_span)
        });
    }

    results
}

/// Fast upper-bound on the number of files that may reference the symbol at `byte_offset`.
///
/// Reads only `ExportTable`s — does NOT walk ASTs.  Returns within a few milliseconds.
/// Used by the LSP find-references handler's progress-emission predicate.
///
/// Time: O(files) ExportTable reads.  Space: O(1).
#[salsa::tracked]
pub fn cross_file_reference_count_estimate(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    byte_offset: usize,
) -> usize {
    let Some(canonical) = resolve_symbol_at(db, source, byte_offset) else {
        return 0;
    };

    // Only exported symbols can appear in other files.
    let sig_output = module_signatures_query(db, canonical.origin_file);
    let is_exported = sig_output.sig_table.fns.contains_key(&canonical.name)
        || sig_output.shape_table.get(&canonical.name).is_some();

    if !is_exported {
        return 1; // local-only — only the current file references it
    }

    // Count files that import from the origin file (ExportTable read only — no AST walk).
    let origin_path = canonical.origin_file.path(db);
    let all_paths = db.all_source_paths();
    let mut count = 1usize; // at least the origin file
    for path in all_paths {
        if path == origin_path {
            continue;
        }
        let Some(sf) = db.source_by_path(&path) else {
            continue;
        };
        let parse = parse_query(db, sf);
        // Check if this file imports from the origin file without walking the full AST.
        let imports_origin = parse.module.items.iter().any(|item| {
            if let Item::ImportDecl(d) = item {
                crate::resolve_import::resolve_module_path(&path, &d.source)
                    .map(|p| p.display().to_string() == origin_path)
                    .unwrap_or(false)
            } else {
                false
            }
        });
        if imports_origin {
            count += 1;
        }
    }
    count
}

/// Find all locations that must be updated to rename the symbol at `byte_offset`.
///
/// Validates `new_name` before walking — returns `Err` immediately for invalid names.
/// Returns `Err(CannotRenameImportedSymbolInThisFile)` when the symbol is declared
/// in another file (rename must start at the origin).
///
/// Time: O(files × AST size) on cache miss.  Space: O(results).
#[salsa::tracked]
pub fn rename_locations(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    byte_offset: usize,
    new_name: String,
) -> Result<Vec<(SourceFile, SourceSpan)>, RenameError> {
    // Validate the new name first — fast fail before any file walking.
    validate_new_name(&new_name)?;

    let canonical = resolve_symbol_at(db, source, byte_offset)
        .ok_or(RenameError::NotARenameable)?;

    // Only allow rename from the declaration file.
    if canonical.origin_file != source {
        let origin_path = canonical.origin_file.path(db);
        return Err(RenameError::CannotRenameImportedSymbolInThisFile(origin_path));
    }

    // Collect all use-sites (including declaration).
    let all_sites = references_for_offset(db, source, byte_offset, true);
    Ok(all_sites)
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Walk a function body looking for a `let`/`const`/`for` binding or parameter
/// named `name` that is in scope at `byte_offset`.
///
/// Scope is respected: inner-scope bindings shadow outer ones.  The offset must
/// fall after the binding's declaration point (no forward references for locals).
fn resolve_local(
    name: &str,
    f: &FunctionDecl,
    offset: usize,
    source: SourceFile,
    _db: &dyn SourceFileRegistry,
) -> Option<ResolvedSymbol> {
    // Check parameter names first — they are in scope for the entire body.
    for p in &f.params {
        if p.name == name {
            return Some(ResolvedSymbol {
                kind: SymbolKind::Parameter,
                origin_file: source,
                decl_span: p.name_span.clone(),
                name: name.to_string(),
            });
        }
    }

    // Walk the body, collecting bindings visible at `offset`.
    find_binding_in_block(&f.body, name, offset, source)
}

fn find_binding_in_block(
    block: &Block,
    name: &str,
    offset: usize,
    source: SourceFile,
) -> Option<ResolvedSymbol> {
    // Walk stmts in order; once we pass the offset, stop (no forward references).
    let mut best: Option<ResolvedSymbol> = None;

    for stmt in &block.stmts {
        // Stop if the statement starts AFTER the offset (the binding isn't visible yet).
        if let Some(stmt_start) = stmt_start(stmt) {
            if stmt_start > offset {
                break;
            }
        }

        match stmt {
            Stmt::Let { is_const, name: binding_name, name_span, .. } if *binding_name == name => {
                let kind = if *is_const { SymbolKind::LocalConst } else { SymbolKind::LocalLet };
                best = Some(ResolvedSymbol {
                    kind,
                    origin_file: source,
                    decl_span: name_span.clone(),
                    name: name.to_string(),
                });
            }
            Stmt::If { body, .. } | Stmt::While { body, .. } => {
                if let Some(inner) = find_binding_in_block(body, name, offset, source) {
                    best = Some(inner);
                }
            }
            Stmt::For { var, var_span, body, .. } => {
                if var == name {
                    best = Some(ResolvedSymbol {
                        kind: SymbolKind::LocalLet,
                        origin_file: source,
                        decl_span: var_span.clone(),
                        name: name.to_string(),
                    });
                }
                if let Some(inner) = find_binding_in_block(body, name, offset, source) {
                    best = Some(inner);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(inner) = find_binding_in_block(&arm.body, name, offset, source) {
                        best = Some(inner);
                    }
                }
                if let Some(b) = else_arm {
                    if let Some(inner) = find_binding_in_block(b, name, offset, source) {
                        best = Some(inner);
                    }
                }
            }
            _ => {}
        }
    }

    best
}

fn stmt_start(stmt: &Stmt) -> Option<usize> {
    match stmt {
        Stmt::Let { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::FieldAssign { span, .. }
        | Stmt::IndexAssign { span, .. } => Some(span.start),
        Stmt::Expr(e) => Some(e.span().start),
    }
}

/// Resolve `name` by following the import declarations in `source`.
///
/// Returns a `ResolvedSymbol` when the name is found in an imported file's
/// export table, `None` otherwise.
fn resolve_via_imports(
    name: &str,
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    source_path: &str,
) -> Option<ResolvedSymbol> {
    let parse = parse_query(db, source);

    for item in &parse.module.items {
        let Item::ImportDecl(decl) = item else {
            continue;
        };

        // Map local name → exported name (handling `as alias`).
        let exported_name: Option<String> = match &decl.kind {
            ynz_ast::nodes::ImportKind::Named(items) => {
                items.iter().find(|i| i.local_name == name).map(|i| i.exported_name.clone())
            }
            ynz_ast::nodes::ImportKind::Namespace { local_name, .. } => {
                // Namespace imports bind as `ns.Name` — check prefix form.
                if name.starts_with(&format!("{local_name}.")) {
                    Some(name[local_name.len() + 1..].to_string())
                } else {
                    None
                }
            }
        };

        let Some(exported) = exported_name else {
            continue;
        };

        let Some(resolved_path) = crate::resolve_import::resolve_module_path(source_path, &decl.source) else {
            continue;
        };

        let path_str = resolved_path.display().to_string();
        let Some(origin_sf) = db.source_by_path(&path_str) else {
            continue;
        };

        let origin_sig = module_signatures_query(db, origin_sf);

        if let Some(def) = origin_sig.shape_table.get(&exported) {
            return Some(ResolvedSymbol {
                kind: SymbolKind::Shape,
                origin_file: origin_sf,
                decl_span: def.defined_at.clone(),
                name: name.to_string(),
            });
        }
        if let Some(sig) = origin_sig.sig_table.fns.get(&exported) {
            return Some(ResolvedSymbol {
                kind: SymbolKind::Function,
                origin_file: origin_sf,
                decl_span: sig.decl_span.clone(),
                name: name.to_string(),
            });
        }

        // Check options in origin file.
        // Diagnostics discarded intentionally — read-only cross-file lookup path.
        let origin_parse = parse_query(db, origin_sf);
        let mut dummy = ynz_diagnostics::DiagnosticBucket::new();
        let options = collect_options(&origin_parse.module, &mut dummy);
        if let Some(entry) = options.get(&exported) {
            return Some(ResolvedSymbol {
                kind: SymbolKind::Options,
                origin_file: origin_sf,
                decl_span: entry.decl_span.clone(),
                name: name.to_string(),
            });
        }
    }

    None
}

/// Walk all expressions in a module and collect spans where `canonical.name` appears as
/// a symbol reference that resolves to the SAME canonical symbol.
///
/// Each candidate `Ident` with the matching name is re-resolved via `resolve_symbol_at`
/// so inner-scope shadow bindings are excluded — a `let count = 2` that shadows an outer
/// `let count = 1` resolves to a different `ResolvedSymbol` (different `decl_span`) and
/// is not added to `results`.  This prevents rename from touching the wrong binding.
fn collect_use_sites_in_module(
    module: &ynz_ast::nodes::Module,
    canonical: &ResolvedSymbol,
    sf: SourceFile,
    db: &dyn SourceFileRegistry,
    results: &mut Vec<(SourceFile, SourceSpan)>,
) {
    for item in &module.items {
        match item {
            Item::Function(f) => collect_use_sites_in_function(f, canonical, sf, db, results),
            Item::ConstDecl(c) => collect_use_sites_in_expr(&c.value, canonical, sf, db, results),
            _ => {}
        }
    }
}

fn collect_use_sites_in_function(
    f: &FunctionDecl,
    canonical: &ResolvedSymbol,
    sf: SourceFile,
    db: &dyn SourceFileRegistry,
    results: &mut Vec<(SourceFile, SourceSpan)>,
) {
    collect_use_sites_in_block(&f.body, canonical, sf, db, results);
}

fn collect_use_sites_in_block(
    block: &Block,
    canonical: &ResolvedSymbol,
    sf: SourceFile,
    db: &dyn SourceFileRegistry,
    results: &mut Vec<(SourceFile, SourceSpan)>,
) {
    for stmt in &block.stmts {
        collect_use_sites_in_stmt(stmt, canonical, sf, db, results);
    }
}

fn collect_use_sites_in_stmt(
    stmt: &Stmt,
    canonical: &ResolvedSymbol,
    sf: SourceFile,
    db: &dyn SourceFileRegistry,
    results: &mut Vec<(SourceFile, SourceSpan)>,
) {
    match stmt {
        Stmt::Let { value, .. } | Stmt::Assign { value, .. } => {
            collect_use_sites_in_expr(value, canonical, sf, db, results);
        }
        Stmt::Expr(e) => collect_use_sites_in_expr(e, canonical, sf, db, results),
        Stmt::Return { value: Some(e), .. } => {
            collect_use_sites_in_expr(e, canonical, sf, db, results);
        }
        Stmt::Return { value: None, .. } => {}
        Stmt::If { cond, body, .. } => {
            collect_use_sites_in_expr(cond, canonical, sf, db, results);
            collect_use_sites_in_block(body, canonical, sf, db, results);
        }
        Stmt::Match { scrutinee, arms, else_arm, .. } => {
            collect_use_sites_in_expr(scrutinee, canonical, sf, db, results);
            for arm in arms {
                collect_use_sites_in_block(&arm.body, canonical, sf, db, results);
            }
            if let Some(b) = else_arm {
                collect_use_sites_in_block(b, canonical, sf, db, results);
            }
        }
        Stmt::While { cond, body, .. } => {
            collect_use_sites_in_expr(cond, canonical, sf, db, results);
            collect_use_sites_in_block(body, canonical, sf, db, results);
        }
        Stmt::For { iter, body, .. } => {
            collect_use_sites_in_expr(iter, canonical, sf, db, results);
            collect_use_sites_in_block(body, canonical, sf, db, results);
        }
        Stmt::FieldAssign { target, value, .. } => {
            collect_use_sites_in_expr(target, canonical, sf, db, results);
            collect_use_sites_in_expr(value, canonical, sf, db, results);
        }
        Stmt::IndexAssign { receiver, index, value, .. } => {
            collect_use_sites_in_expr(receiver, canonical, sf, db, results);
            collect_use_sites_in_expr(index, canonical, sf, db, results);
            collect_use_sites_in_expr(value, canonical, sf, db, results);
        }
    }
}

/// Add `span` to `results` if `resolve_symbol_at(db, sf, span.start)` resolves to `canonical`.
///
/// This is the shadow-correctness gate: a name-match at a position that resolves to a
/// DIFFERENT binding (shadowing inner let or unrelated file-level symbol) is excluded.
fn emit_if_same_canonical(
    span: &SourceSpan,
    canonical: &ResolvedSymbol,
    sf: SourceFile,
    db: &dyn SourceFileRegistry,
    results: &mut Vec<(SourceFile, SourceSpan)>,
) {
    if let Some(resolved) = resolve_symbol_at(db, sf, span.start) {
        if resolved.origin_file == canonical.origin_file && resolved.decl_span == canonical.decl_span {
            results.push((sf, span.clone()));
        }
    }
}

fn collect_use_sites_in_expr(
    expr: &Expr,
    canonical: &ResolvedSymbol,
    sf: SourceFile,
    db: &dyn SourceFileRegistry,
    results: &mut Vec<(SourceFile, SourceSpan)>,
) {
    match expr {
        Expr::Ident(n, span) if n == &canonical.name => {
            emit_if_same_canonical(span, canonical, sf, db, results);
        }
        Expr::Call(call) => {
            collect_use_sites_in_expr(&call.callee, canonical, sf, db, results);
            for arg in &call.args {
                collect_use_sites_in_expr(arg, canonical, sf, db, results);
            }
        }
        Expr::MethodCall { receiver, method, method_span, args, .. } => {
            if method == &canonical.name {
                emit_if_same_canonical(method_span, canonical, sf, db, results);
            }
            collect_use_sites_in_expr(receiver, canonical, sf, db, results);
            for arg in args {
                collect_use_sites_in_expr(arg, canonical, sf, db, results);
            }
        }
        Expr::FieldAccess { receiver, .. } => {
            collect_use_sites_in_expr(receiver, canonical, sf, db, results);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_use_sites_in_expr(lhs, canonical, sf, db, results);
            collect_use_sites_in_expr(rhs, canonical, sf, db, results);
        }
        Expr::UnaryOp { operand, .. } => {
            collect_use_sites_in_expr(operand, canonical, sf, db, results);
        }
        Expr::StructLit { fields, .. } => {
            for f in fields {
                collect_use_sites_in_expr(&f.value, canonical, sf, db, results);
            }
        }
        Expr::PostfixOp { receiver, .. } => {
            collect_use_sites_in_expr(receiver, canonical, sf, db, results);
        }
        Expr::IndexAccess { receiver, index, .. } => {
            collect_use_sites_in_expr(receiver, canonical, sf, db, results);
            collect_use_sites_in_expr(index, canonical, sf, db, results);
        }
        Expr::ArrayLit { elements, .. } => {
            for e in elements {
                collect_use_sites_in_expr(e, canonical, sf, db, results);
            }
        }
        Expr::MapLit { entries, .. } => {
            for (k, v) in entries {
                collect_use_sites_in_expr(k, canonical, sf, db, results);
                collect_use_sites_in_expr(v, canonical, sf, db, results);
            }
        }
        Expr::Is { expr, .. } => collect_use_sites_in_expr(expr, canonical, sf, db, results),
        Expr::InterpolatedString(parts, _) => {
            for part in parts {
                if let ynz_ast::nodes::StringPart::Expr(e, _) = part {
                    collect_use_sites_in_expr(e, canonical, sf, db, results);
                }
            }
        }
        Expr::Wait(inner, _) | Expr::Background(inner, _) => {
            collect_use_sites_in_expr(inner, canonical, sf, db, results);
        }
        Expr::Ident(..)
        | Expr::StringLit(..)
        | Expr::IntLit(..)
        | Expr::NumberLit(..)
        | Expr::BoolLit(..)
        | Expr::NoneLit { .. }
        | Expr::SelfValue { .. }
        | Expr::Error(..) => {}
    }
}

/// Validate that `new_name` is a legal Yinz identifier and not reserved.
///
/// Called by `rename_locations` before any file scanning.
fn validate_new_name(name: &str) -> Result<(), RenameError> {
    if name.is_empty()
        || !name.starts_with(|c: char| c.is_alphabetic())
        || !name.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(RenameError::NewNameInvalidIdentifier(name.to_string()));
    }

    if keywords().any(|kw| kw.name == name) {
        return Err(RenameError::NewNameIsReservedKeyword(name.to_string()));
    }

    if let Some(entry) = banned_jargon_lookup(name) {
        return Err(RenameError::NewNameIsBannedJargon(
            name.to_string(),
            entry.replacement.to_string(),
        ));
    }

    // Banned declaration keywords (class, enum, struct, etc.) are also invalid
    // identifiers because the lexer would tokenize them specially, not as Ident.
    if banned_declaration_keyword_lookup(name).is_some() {
        return Err(RenameError::NewNameIsBannedJargon(
            name.to_string(),
            "use a Yinz-appropriate name (e.g. `shape` instead of `class`)".to_string(),
        ));
    }

    Ok(())
}
