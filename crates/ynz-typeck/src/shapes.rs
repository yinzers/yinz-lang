use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Item, Module, Type as AstType};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::types::Type;

/// A resolved field on a shape.
#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
    pub is_hidden: bool,
    pub defined_at: SourceSpan,
}

/// A resolved shape declaration.
///
/// P3b will extend this with `extends` and `follows` resolution.
/// P3a only resolves data fields.
#[derive(Clone, Debug)]
pub struct ShapeDef {
    pub name: String,
    pub is_base: bool,
    pub fields: Vec<FieldDef>,
    pub defined_at: SourceSpan,
}

impl ShapeDef {
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// All shapes collected from a module.
#[derive(Clone, Debug, Default)]
pub struct ShapeTable {
    pub shapes: HashMap<String, ShapeDef>,
}

impl ShapeTable {
    pub fn empty() -> Self {
        Self { shapes: HashMap::new() }
    }

    pub fn get(&self, name: &str) -> Option<&ShapeDef> {
        self.shapes.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.shapes.contains_key(name)
    }

    /// Resolve a named AST type to a typeck Type, using this table for shape names.
    pub fn resolve_ast_type(&self, ast_ty: &AstType) -> Type {
        match ast_ty {
            AstType::Nothing => Type::Nothing,
            AstType::Int => Type::Int,
            AstType::Float => Type::Float,
            AstType::Number { .. } => Type::Number { precision: 34 },
            AstType::Bool => Type::Bool,
            AstType::Named(n, _) if n == "string" => Type::String,
            AstType::Named(n, _) if self.contains(n) => Type::Shape { name: n.clone() },
            AstType::Error | AstType::Named(_, _) | AstType::Range { .. } => Type::Error,
            // P3b: dynamic dispatch and Self type resolution.
            AstType::Dynamic { .. } | AstType::SelfType { .. } => Type::Error,
        }
    }
}

/// Collect all shape declarations from a module into a `ShapeTable`.
///
/// Validates:
/// - Duplicate shape names
/// - Cyclic field dependencies (shape A has field of type shape B which has field of type A)
///
/// Field types are resolved using the names collected in the first pass,
/// so forward references between shapes in the same file are allowed.
pub fn collect_shapes(module: &Module, diags: &mut DiagnosticBucket) -> ShapeTable {
    let mut table = ShapeTable::empty();

    // Pass 1: collect all shape names (for forward-reference resolution).
    let mut all_names: HashSet<String> = HashSet::new();
    for item in &module.items {
        if let Item::ShapeDecl(s) = item {
            if all_names.contains(&s.name) {
                diags.push(Diagnostic::error(
                    s.name_span.clone(),
                    format!("A shape named `{}` is already defined in this file.", s.name),
                    "Rename one of the two shapes — each shape in a file must have a unique name.",
                    "Yinz does not allow two shapes with the same name in the same file.",
                ));
            } else {
                all_names.insert(s.name.clone());
            }
        }
    }

    // Build a temporary ShapeTable with just names for type resolution.
    let name_table = ShapeTable {
        shapes: all_names.iter().map(|n| (n.clone(), ShapeDef {
            name: n.clone(),
            is_base: false,
            fields: vec![],
            defined_at: SourceSpan::new("", 0, 0),
        })).collect(),
    };

    // Pass 2: resolve each shape's fields.
    for item in &module.items {
        let Item::ShapeDecl(s) = item else { continue };
        if !all_names.contains(&s.name) {
            continue; // duplicate — already errored, skip
        }

        let mut fields = Vec::new();
        let mut seen_field_names: HashSet<String> = HashSet::new();

        for field in &s.fields {
            if !seen_field_names.insert(field.name.clone()) {
                diags.push(Diagnostic::error(
                    field.name_span.clone(),
                    format!("Field `{}` is already declared on `{}`.", field.name, s.name),
                    "Each field in a shape must have a unique name.",
                    "Two fields with the same name would make it impossible to tell them apart.",
                ));
                continue;
            }

            let ty = name_table.resolve_ast_type(&field.ty);

            // Hidden fields without a default are a parser-level error; typeck
            // accepts them here and lets the parser diagnostic suffice.

            fields.push(FieldDef {
                name: field.name.clone(),
                ty,
                is_hidden: field.is_hidden,
                defined_at: field.name_span.clone(),
            });
        }

        table.shapes.insert(s.name.clone(), ShapeDef {
            name: s.name.clone(),
            is_base: s.is_base,
            fields,
            defined_at: s.name_span.clone(),
        });
    }

    // Cycle detection: if shape A has a direct (non-pointer) field of type shape B
    // which directly or transitively has a field of type shape A → error.
    detect_field_cycles(&table, diags);

    table
}

fn detect_field_cycles(table: &ShapeTable, diags: &mut DiagnosticBucket) {
    for (name, def) in &table.shapes {
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(name.clone());
        if has_cycle(name, def, table, &mut visited) {
            diags.push(Diagnostic::error(
                def.defined_at.clone(),
                format!("`{}` contains a direct field cycle.", name),
                format!("Use a separate shape that holds a reference, or redesign to break the cycle."),
                "A shape cannot directly contain itself (or a chain that leads back to itself) — \
                 that would require infinite memory. Use indirection to express recursive structures.",
            ));
        }
    }
}

fn has_cycle(
    root: &str,
    def: &ShapeDef,
    table: &ShapeTable,
    visited: &mut HashSet<String>,
) -> bool {
    for field in &def.fields {
        if let Type::Shape { name } = &field.ty {
            if name == root {
                return true;
            }
            if visited.insert(name.clone()) {
                if let Some(nested) = table.shapes.get(name) {
                    if has_cycle(root, nested, table, visited) {
                        return true;
                    }
                }
            }
        }
    }
    false
}
