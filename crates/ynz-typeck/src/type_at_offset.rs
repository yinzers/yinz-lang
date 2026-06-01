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
        // string → "string" narrowing for string methods
        AstType::Named(name, _) if name == "string" => Some(Type::String),
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

    #[test]
    fn test_int_annotated_let() {
        // WHY: Verifies annotated `let score: int` is resolved to Type::Int
        //      so after-dot completion narrows to int methods.
        let src = "function entrypoint() -> nothing {\n  let score: int = 5\n  score\n}";
        let (db, sf) = make_db_with("test.ynz", src);
        // offset of "score" in the let binding name
        let score_offset = src.find("score").unwrap();
        let ty = type_of_expression_at_offset(&db, sf, score_offset);
        assert_eq!(ty, Some(Type::Int));
    }

    #[test]
    fn test_float_annotated_let() {
        // WHY: float type annotation should narrow to Type::Float.
        let src = "function entrypoint() -> nothing {\n  let ratio: float = 1.5\n}";
        let (db, sf) = make_db_with("test.ynz", src);
        let ratio_offset = src.find("ratio").unwrap();
        let ty = type_of_expression_at_offset(&db, sf, ratio_offset);
        assert_eq!(ty, Some(Type::Float));
    }

    #[test]
    fn test_param_type_resolved() {
        // WHY: function parameters with type annotations should narrow completion
        //      the same way let-bindings do.
        let src = "function greet(share self: int) -> nothing { }";
        let (db, sf) = make_db_with("test.ynz", src);
        let self_offset = src.find("self").unwrap();
        let ty = type_of_expression_at_offset(&db, sf, self_offset);
        assert_eq!(ty, Some(Type::Int));
    }

    #[test]
    fn test_unannotated_let_returns_none() {
        // WHY: unannotated bindings require full inference; this pass returns None,
        //      which causes completion to show all primitive methods (safe fallback).
        let src = "function entrypoint() -> nothing {\n  let x = 5\n}";
        let (db, sf) = make_db_with("test.ynz", src);
        let x_offset = src.find("x = ").unwrap();
        let ty = type_of_expression_at_offset(&db, sf, x_offset);
        assert_eq!(ty, None);
    }

    #[test]
    fn test_shape_type_annotation() {
        // WHY: shape types are returned as Type::Shape so hover can look up doc comments;
        //      they don't narrow primitive completion (no primitive methods on shapes).
        let src = "function foo(p: Player) -> nothing { }";
        let (db, sf) = make_db_with("test.ynz", src);
        let p_offset = src.find('p').unwrap();
        let ty = type_of_expression_at_offset(&db, sf, p_offset);
        assert_eq!(
            ty,
            Some(Type::Shape {
                name: "Player".to_string()
            })
        );
    }

    #[test]
    fn test_out_of_function_returns_none() {
        // WHY: the pass only walks function bodies; top-level offsets return None.
        let src = "export const MAX = 100";
        let (db, sf) = make_db_with("test.ynz", src);
        let ty = type_of_expression_at_offset(&db, sf, 0);
        assert_eq!(ty, None);
    }
}
