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
    /// True when inherited from a parent shape via `extends`.
    pub is_inherited: bool,
    pub defined_at: SourceSpan,
}

/// A bare contract method signature stored on the contract shape.
#[derive(Clone, Debug)]
pub struct ContractSigDef {
    pub name: String,
    /// Param types (not including self).
    pub param_tys: Vec<Type>,
    pub ret_ty: Type,
}

/// A resolved shape declaration.
///
/// Fields include inherited ones (prepended from parent chain).
/// `extends` and `follows` names are stored for P3b verification.
#[derive(Clone, Debug)]
pub struct ShapeDef {
    pub name: String,
    pub is_base: bool,
    /// The parent shape name if `extends` was used.
    pub extends: Option<String>,
    /// Contract shapes this shape must satisfy.
    pub follows: Vec<String>,
    /// All fields — own fields plus inherited (inherited come first).
    pub fields: Vec<FieldDef>,
    /// Bare contract method signatures declared in this shape's body.
    pub contract_sigs: Vec<ContractSigDef>,
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
            // M5 P3a / P3b: generics engine + built-in collections.
            AstType::TypeParam { .. } | AstType::Generic { .. } | AstType::Maybe { .. } => Type::Error,
        }
    }
}

/// Collect all shape declarations from a module into a `ShapeTable`.
///
/// Validates:
/// - Duplicate shape names
/// - `extends` parent exists; no cyclic extends chains
/// - Cyclic field dependencies (direct field type cycles)
///
/// Field types are resolved in a forward-reference-friendly two-pass approach.
/// Inherited fields (from `extends`) are prepended to the child's field list.
pub fn collect_shapes(module: &Module, diags: &mut DiagnosticBucket) -> ShapeTable {
    let mut table = ShapeTable::empty();

    // Pass 1: collect all shape names + their raw AST data.
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

    // Temporary name-only table for type resolution (enables forward references).
    let name_table = ShapeTable {
        shapes: all_names.iter().map(|n| (n.clone(), ShapeDef {
            name: n.clone(),
            is_base: false,
            extends: None,
            follows: vec![],
            fields: vec![],
            contract_sigs: vec![],
            defined_at: SourceSpan::new("", 0, 0),
        })).collect(),
    };

    // Pass 2: resolve each shape's own fields, contract sigs, extends, and follows.
    for item in &module.items {
        let Item::ShapeDecl(s) = item else { continue };
        if !all_names.contains(&s.name) {
            continue; // duplicate — already errored
        }

        // Validate extends parent exists.
        let extends = s.extends.as_ref().map(|(parent, parent_span)| {
            if !all_names.contains(parent) {
                diags.push(Diagnostic::error(
                    parent_span.clone(),
                    format!("`{parent}` is not defined — cannot extend it."),
                    format!("Declare `shape {parent} {{ ... }}` in this file before extending it."),
                    "`extends` is data-only inheritance — the parent shape must be declared in the same file.",
                ));
            }
            parent.clone()
        });

        // Validate follows contracts exist.
        let follows: Vec<String> = s.follows.iter().map(|(contract, contract_span)| {
            if !all_names.contains(contract) {
                diags.push(Diagnostic::error(
                    contract_span.clone(),
                    format!("`{contract}` is not defined — cannot follow it."),
                    format!("Declare `shape {contract} {{ ... }}` with bare method signatures in this file."),
                    "`follows` names a contract shape — a shape whose body contains only bare method signature declarations.",
                ));
            }
            contract.clone()
        }).collect();

        // Resolve own fields.
        let mut own_fields: Vec<FieldDef> = Vec::new();
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
            own_fields.push(FieldDef {
                name: field.name.clone(),
                ty,
                is_hidden: field.is_hidden,
                is_inherited: false,
                defined_at: field.name_span.clone(),
            });
        }

        // Resolve contract sigs.
        let contract_sigs: Vec<ContractSigDef> = s.contract_sigs.iter().map(|sig| {
            let param_tys: Vec<Type> = sig.params.iter()
                .map(|p| name_table.resolve_ast_type(&p.ty))
                .collect();
            let ret_ty = name_table.resolve_ast_type(&sig.return_type);
            ContractSigDef { name: sig.name.clone(), param_tys, ret_ty }
        }).collect();

        table.shapes.insert(s.name.clone(), ShapeDef {
            name: s.name.clone(),
            is_base: s.is_base,
            extends,
            follows,
            fields: own_fields, // inherited fields added in pass 3
            contract_sigs,
            defined_at: s.name_span.clone(),
        });
    }

    // Pass 3: detect cyclic extends chains, then flatten inherited fields.
    detect_extends_cycles(&table, diags);
    flatten_inherited_fields(&mut table, diags);

    // Pass 4: detect direct field-type cycles (shape A has field of type shape A).
    detect_field_cycles(&table, diags);

    table
}

fn detect_extends_cycles(table: &ShapeTable, diags: &mut DiagnosticBucket) {
    for name in table.shapes.keys() {
        let mut visited: Vec<String> = vec![name.clone()];
        let mut current = name.clone();
        while let Some(def) = table.shapes.get(&current) {
            let Some(parent) = &def.extends else { break };
            if visited.contains(parent) {
                let chain = visited.join(" → ");
                if let Some(def) = table.shapes.get(name) {
                    diags.push(Diagnostic::error(
                        def.defined_at.clone(),
                        format!("`{name}` has a cyclic `extends` chain: {chain} → {parent}"),
                        "Break the cycle by removing one of the `extends` declarations.",
                        "`extends` is for data inheritance — a shape cannot inherit from itself, directly or through a chain.",
                    ));
                }
                break;
            }
            visited.push(parent.clone());
            current = parent.clone();
        }
    }
}

fn flatten_inherited_fields(table: &mut ShapeTable, _diags: &mut DiagnosticBucket) {
    // Collect the inheritance order to avoid borrow conflicts.
    let names: Vec<String> = table.shapes.keys().cloned().collect();
    for name in &names {
        let parent_name = table.shapes[name].extends.clone();
        let Some(parent) = parent_name else { continue };

        // Collect parent fields (may themselves be inherited — already flattened if
        // we process in topological order, but for simplicity just grab what's there).
        let parent_fields: Vec<FieldDef> = match table.shapes.get(&parent) {
            Some(p) => p.fields.iter().map(|f| FieldDef {
                name: f.name.clone(),
                ty: f.ty.clone(),
                is_hidden: f.is_hidden,
                is_inherited: true,
                defined_at: f.defined_at.clone(),
            }).collect(),
            None => continue, // parent doesn't exist — already errored
        };

        // Prepend parent fields to child's own fields, skipping overridden names.
        let child = table.shapes.get_mut(name).unwrap();
        let own_names: HashSet<&str> = child.fields.iter().map(|f| f.name.as_str()).collect();
        let mut all_fields: Vec<FieldDef> = parent_fields.into_iter()
            .filter(|f| !own_names.contains(f.name.as_str()))
            .collect();
        all_fields.append(&mut child.fields);
        child.fields = all_fields;
    }
}

fn detect_field_cycles(table: &ShapeTable, diags: &mut DiagnosticBucket) {
    for (name, def) in &table.shapes {
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(name.clone());
        if has_cycle(name, def, table, &mut visited) {
            diags.push(Diagnostic::error(
                def.defined_at.clone(),
                format!("`{}` contains a direct field cycle.", name),
                "Use a separate shape that holds a reference, or redesign to break the cycle.".to_string(),
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
