//! Resolve the type of the expression at a given byte offset.
//!
//! Used by the LSP completion handler to narrow after-dot completions
//! (`score.` where `score: int` → show only int methods).
//!
//! # Limitations
//!
//! This pass reads the AST directly — it only resolves types that are
//! explicit in the source (parameter type annotations, let-binding type
//! annotations). Inferred types (unannotated `let x = 5`) return `None`.
//! Full inference requires the `check_query` pass; that cost is too high
//! for a per-keystroke completion path.  The common cases (annotated locals,
//! function parameters, imported shape fields) are covered by this pass.

use ynz_ast::nodes::{Block, FunctionDecl, Item, Module, Stmt, Type as AstType};
use ynz_parser::{parse_query, SourceFile, SourceFileRegistry};

use crate::{ast_offset::span_contains, types::Type};

/// Map an AST `Type` annotation to the resolved typeck `Type`.
///
/// Handles only the primitive cases relevant to completion narrowing.
/// Returns `None` for complex types (shapes, generics, dynamic) that
/// don't have narrowed primitive-method completions in the registry.
fn ast_type_to_primitive(ty: &AstType) -> Option<Type> {
    match ty {
        AstType::Int => Some(Type::Int),
        AstType::Float => Some(Type::Float),
        AstType::Bool => Some(Type::Bool),
        // String → "String" narrowing for string methods
        AstType::Named(name, _) if name == "String" => Some(Type::String),
        // Shape names — return as Shape for doc-hover; not narrowed for primitive completion
        AstType::Named(name, _) => Some(Type::Shape { name: name.clone() }),
        _ => None,
    }
}

/// Resolve the type of the identifier at `byte_offset` in `source`.
///
/// Walks function bodies for parameter and let-binding type annotations.
/// Falls back to file-level shape/function names for shape type completion.
///
/// Time: O(n) AST walk.  Space: O(depth) call stack.
pub fn type_of_expression_at_offset(
    db: &dyn SourceFileRegistry,
    source: SourceFile,
    byte_offset: usize,
) -> Option<Type> {
    let parse = parse_query(db, source);
    resolve_type_in_module(&parse.module, byte_offset)
}

/// Look up the declared type of the binding or parameter named `name` in the
/// innermost function that contains `byte_offset`.
///
/// Used by the hover handler: `identifier_use_site_at_offset` supplies the name
/// at an expression use-site; this function finds the corresponding declaration and
/// returns its annotated type. Only annotated let-bindings and parameters are
/// covered — unannotated bindings require the full `check_query` pass.
///
/// Time: O(n) AST walk where n = statements in the enclosing function.
/// Space: O(depth) call stack.
pub fn type_of_name_at_offset(module: &Module, name: &str, byte_offset: usize) -> Option<Type> {
    for item in &module.items {
        if let Item::Function(f) = item {
            if span_contains(&f.span, byte_offset) {
                // Check parameters first.
                for param in &f.params {
                    if param.name == name {
                        return ast_type_to_primitive(&param.ty);
                    }
                }
                // Walk the body for let-bindings by name, not by offset.
                return resolve_type_by_name_in_block(&f.body, name);
            }
        }
    }
    None
}

fn resolve_type_by_name_in_block(block: &Block, name: &str) -> Option<Type> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let {
                name: bname,
                ty: Some(ty),
                ..
            } if bname == name => {
                return ast_type_to_primitive(ty);
            }
            Stmt::Let { .. } => {}
            Stmt::If { body, .. } => {
                if let Some(t) = resolve_type_by_name_in_block(body, name) {
                    return Some(t);
                }
            }
            Stmt::While { body, .. } => {
                if let Some(t) = resolve_type_by_name_in_block(body, name) {
                    return Some(t);
                }
            }
            Stmt::For { body, .. } => {
                if let Some(t) = resolve_type_by_name_in_block(body, name) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) = resolve_type_by_name_in_block(&arm.body, name) {
                        return Some(t);
                    }
                }
                if let Some(b) = else_arm {
                    if let Some(t) = resolve_type_by_name_in_block(b, name) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn resolve_type_in_module(module: &Module, byte_offset: usize) -> Option<Type> {
    for item in &module.items {
        if let Item::Function(f) = item {
            if span_contains(&f.span, byte_offset) {
                return resolve_type_in_function(f, byte_offset);
            }
        }
    }
    None
}

fn resolve_type_in_function(f: &FunctionDecl, byte_offset: usize) -> Option<Type> {
    // Check parameters first: `function foo(score: int)` → `score` has type int
    for param in &f.params {
        if span_contains(&param.name_span, byte_offset) {
            return ast_type_to_primitive(&param.ty);
        }
    }
    // Walk the function body for let/const bindings
    resolve_type_in_block(&f.body, byte_offset)
}

fn resolve_type_in_block(block: &Block, byte_offset: usize) -> Option<Type> {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let {
                name_span,
                ty: Some(ty),
                ..
            } if span_contains(name_span, byte_offset) => {
                return ast_type_to_primitive(ty);
            }
            Stmt::Let { .. } => {} // unannotated let — can't determine type without inference
            Stmt::If { body, .. } => {
                if let Some(t) = resolve_type_in_block(body, byte_offset) {
                    return Some(t);
                }
            }
            Stmt::While { body, .. } => {
                if let Some(t) = resolve_type_in_block(body, byte_offset) {
                    return Some(t);
                }
            }
            Stmt::For { body, .. } => {
                if let Some(t) = resolve_type_in_block(body, byte_offset) {
                    return Some(t);
                }
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    if let Some(t) = resolve_type_in_block(&arm.body, byte_offset) {
                        return Some(t);
                    }
                }
                if let Some(b) = else_arm {
                    if let Some(t) = resolve_type_in_block(b, byte_offset) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ynz_parser::{CompilerDb, SourceFile};

    fn make_db_with(path: &str, src: &str) -> (CompilerDb, SourceFile) {
        let mut db = CompilerDb::default();
        let sf = SourceFile::new(&db, path.to_string(), src.to_string());
        db.register_source(sf);
        (db, sf)
    }

    /// One named test per case so the runner names the failing case.
    /// The offset is that of the first occurrence of `$needle` in `$source`.
    macro_rules! type_at_offset_case {
        ($test_name:ident, $source:literal, $needle:literal, $expected:expr) => {
            // bug: LSP completion narrows an annotated receiver to the wrong type, or invents a type for an unannotated one | tier: Medium
            #[test]
            fn $test_name() {
                let (db, source_file) = make_db_with("test.ynz", $source);
                let offset = $source.find($needle).unwrap();
                let resolved = type_of_expression_at_offset(&db, source_file, offset);
                assert_eq!(resolved, $expected);
            }
        };
    }

    // Case bug: the let-binding name loses its annotation, so after-dot completion offers every method.
    type_at_offset_case!(
        annotated_let_resolves_to_its_annotation,
        "function entrypoint() -> Nothing {\n  let score: Int = 5\n  score\n}",
        "score",
        Some(Type::Int)
    );
    // Case bug: parameters are skipped and only let-bindings are narrowed.
    type_at_offset_case!(
        annotated_parameter_resolves_to_its_annotation,
        "function greet(share self: Int) -> Nothing { }",
        "self",
        Some(Type::Int)
    );
    // Case bug: a shape annotation is reported as a primitive (or lost), so hover cannot find the shape's docs.
    type_at_offset_case!(
        shape_annotation_resolves_to_a_shape,
        "function foo(p: Player) -> Nothing { }",
        "p:",
        Some(Type::Shape {
            name: "Player".to_string()
        })
    );
    // Case bug: an unannotated binding is given a guessed type; completion must fall back to all methods.
    type_at_offset_case!(
        unannotated_let_resolves_to_nothing,
        "function entrypoint() -> Nothing {\n  let x = 5\n}",
        "x = ",
        None
    );
    // Case bug: a top-level offset outside any function is walked as if it were inside one.
    type_at_offset_case!(
        offset_outside_a_function_resolves_to_nothing,
        "export const MAX = 100",
        "export",
        None
    );
}
