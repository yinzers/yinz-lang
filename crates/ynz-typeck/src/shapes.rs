use std::collections::{HashMap, HashSet};

use ynz_ast::nodes::{Item, Module, Stmt, Type as AstType};
use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::{
    generics::{GenericShapeDef, GenericShapeTable},
    types::{type_name, Type},
};

/// A resolved field on a shape.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
    pub is_hidden: bool,
    /// True when inherited from a parent shape via `extends`.
    pub is_inherited: bool,
    pub defined_at: SourceSpan,
}

/// A bare contract method signature stored on the contract shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractSigDef {
    pub name: String,
    /// Param types (not including self).
    pub param_tys: Vec<Type>,
    /// Param names (not including self) — the `{param}` slot of a chain-form transfer error.
    pub param_names: Vec<String>,
    /// The DECLARED ownership modifier of each non-`self` param, straight from the AST
    /// (`ContractSig.params[i].ownership`; `None` = bare). v0.3-M8 Phase 4: the `dynamic
    /// Contract` dispatch site runs the transfer decision on the contract's own modifiers —
    /// the only static truth for a runtime-resolved callee — and `follows` conformance
    /// checks the implementer's modifiers equal these.
    pub param_ownerships: Vec<Option<ynz_ast::nodes::OwnershipModifier>>,
    /// The declared receiver kind (`share self` / `lend self` / `give self`); `None` when the
    /// receiver is bare (the parser folds a bare `self` and "no self" into the same value).
    pub receiver: Option<ynz_ast::nodes::ReceiverKind>,
    pub ret_ty: Type,
}

/// A resolved shape declaration.
///
/// Fields include inherited ones (prepended from parent chain).
/// `extends` and `follows` names are stored for P3b verification.
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShapeTable {
    pub shapes: HashMap<String, ShapeDef>,
    /// M6: union type aliases from `shape Shape = Circle | Square` declarations.
    /// Keyed by alias name; value is the resolved union type.
    pub union_aliases: HashMap<String, Type>,
    /// Options type names from this module — allows field type resolution to
    /// produce Type::Options for `options Foo { ... }` types used in shape fields.
    ///
    /// @design-decision SAME-FILE ONLY — cross-file imported options types are NOT visible here.
    /// @rationale collect_shapes runs before collect_options and before imports are resolved,
    ///            so only a same-file pre-scan is possible at this stage.
    /// @cost-to-fix ~1 session: refactor type collection to a single pre-pass that collects
    ///              all type names (shapes + options + imported symbols) before any field
    ///              resolution runs. When that refactor ships, DELETE this field and the
    ///              pre-scan in collect_shapes — it becomes dead weight superseded by the
    ///              global type registry.
    /// @trigger Adding a second file to a project + using an imported options type in a shape field.
    pub options_names: HashSet<String>,
}

impl ShapeTable {
    pub fn empty() -> Self {
        Self {
            shapes: HashMap::new(),
            union_aliases: HashMap::new(),
            options_names: HashSet::new(),
        }
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
            AstType::Number { precision } => Type::Number {
                precision: *precision,
            },
            AstType::Bool => Type::Bool,
            AstType::Named(n, _) if n == "string" => Type::String,
            // M6: union type aliases.
            AstType::Named(n, _) if self.union_aliases.contains_key(n) => {
                self.union_aliases[n].clone()
            }
            AstType::Named(n, _) if self.contains(n) => Type::Shape { name: n.clone() },
            // M7 P3c: built-in compiler-synthesized shapes.
            AstType::Named(n, _) if matches!(n.as_str(), "Frame" | "SourceLoc") => {
                Type::Shape { name: n.clone() }
            }
            // M7 P3c: first-class range type annotation.
            AstType::Named(n, _) if n == "range" => Type::Range {
                element: Box::new(Type::Int),
                end_inclusive: false,
            },
            // Options types declared in this module resolve to Type::Options.
            AstType::Named(n, _) if self.options_names.contains(n) => {
                Type::Options { name: n.clone() }
            }
            AstType::Error | AstType::Named(_, _) | AstType::Range { .. } => Type::Error,
            // AnonShape: hoisted to a synthetic named shape during collect_shapes.
            // Resolve to the canonical synthetic name so the rest of typeck sees a
            // plain Type::Shape.
            AstType::AnonShape { fields, .. } => Type::Shape {
                name: canonical_anon_name(fields),
            },
            // `dynamic Contract` resolves to Type::Dynamic carrying the contract name.
            // This is sufficient for the call-site coerce check in check.rs
            // (shape.follows.contains(contract)) and for the vtable global lookup
            // (vtable_ShapeName_ContractName emitted in M4 P3b).  Method dispatch
            // through a `dynamic` receiver is handled separately in emit.rs.
            AstType::Dynamic { contract, .. } => Type::Dynamic {
                contract: contract.clone(),
            },
            AstType::SelfType { .. } => Type::Error,
            // TypeParam: must be resolved in context (Checker::ast_type_to_type handles this).
            AstType::TypeParam { .. } => Type::Error,
            // Generic instantiation: P3a handles user-defined generics; P3b handles built-ins.
            AstType::Generic { name, args, .. } => {
                let resolved_args: Vec<Type> =
                    args.iter().map(|a| self.resolve_ast_type(a)).collect();
                match name.as_str() {
                    "array" => {
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinArray {
                            elem: Box::new(elem),
                        }
                    }
                    "fixed" => {
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinFixed {
                            elem: Box::new(elem),
                            size: None,
                        }
                    }
                    "map" => {
                        let mut args = resolved_args.into_iter();
                        let key = args.next().unwrap_or(Type::Error);
                        let val = args.next().unwrap_or(Type::Error);
                        Type::BuiltinMap {
                            key: Box::new(key),
                            val: Box::new(val),
                        }
                    }
                    "MapEntry" => {
                        let mut args = resolved_args.into_iter();
                        let key = args.next().unwrap_or(Type::Error);
                        let val = args.next().unwrap_or(Type::Error);
                        Type::MapEntry {
                            key: Box::new(key),
                            val: Box::new(val),
                        }
                    }
                    // v0.3-M4 Phase 2: `channel<T>` in signature/annotation position — required
                    // so a channel can be handed to a `background` task (the composed R5 shape:
                    // `function producer(lend out: channel<int>)`).
                    "channel" => {
                        let elem = resolved_args.into_iter().next().unwrap_or(Type::Error);
                        Type::BuiltinChannel {
                            elem: Box::new(elem),
                        }
                    }
                    _ => {
                        if self.contains(name) {
                            Type::Generic {
                                name: name.clone(),
                                args: resolved_args,
                            }
                        } else {
                            Type::Error
                        }
                    }
                }
            }
            // maybe<T>: P3b.
            AstType::Maybe { inner, .. } => {
                let inner_ty = self.resolve_ast_type(inner);
                Type::Maybe {
                    inner: Box::new(inner_ty),
                }
            }
            // M6: Union types in shape field/signature contexts.
            // These are rare (shape fields of union type); resolve conservatively.
            AstType::Union { variants, .. } => {
                let resolved: Vec<crate::types::Type> =
                    variants.iter().map(|v| self.resolve_ast_type(v)).collect();
                if resolved.len() < 2 {
                    crate::types::Type::Error
                } else {
                    crate::types::Type::Union { variants: resolved }
                }
            }
            // M7 P3a: `-> T errors` — resolve to ErrorsCapable wrapping the inner type.
            AstType::ErrorCapable { inner, .. } => {
                let inner_ty = self.resolve_ast_type(inner);
                crate::types::Type::ErrorsCapable {
                    inner: Box::new(inner_ty),
                }
            }
            // M8 P4: `sensitive T` — resolve to Sensitive.
            AstType::Sensitive(inner) => {
                let inner_ty = self.resolve_ast_type(inner);
                crate::types::Type::Sensitive {
                    inner: Box::new(inner_ty),
                }
            }
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
pub fn collect_shapes(
    module: &Module,
    imported_shapes: &HashMap<String, ShapeDef>,
    imported_options: &HashMap<String, crate::options_table::OptionsEntry>,
    diags: &mut DiagnosticBucket,
) -> ShapeTable {
    let mut table = ShapeTable::empty();

    // Seed the table with imported shapes so field type resolution sees them.
    for (name, def) in imported_shapes {
        table.shapes.insert(name.clone(), def.clone());
    }

    // Pre-pass: collect options type names (same-file AND imported) so field type
    // resolution recognizes them as valid types.
    table.options_names = module
        .items
        .iter()
        .filter_map(|i| {
            if let Item::OptionsDecl(o) = i {
                Some(o.name.clone())
            } else {
                None
            }
        })
        .chain(imported_options.keys().cloned())
        .collect();

    // Pre-pass: hoist AnonShape types to named ShapeDecls so all subsequent passes see
    // only Named types. Synthetic names are content-based (canonical) so structurally
    // identical inline shapes share the same name regardless of where they appear.
    // This pass scans ALL type positions: shape body fields, function params, return
    // types, and let-binding annotations.
    let mut synthetic_items: Vec<ynz_ast::nodes::ShapeDecl> = Vec::new();
    for item in &module.items {
        match item {
            Item::ShapeDecl(s) if s.alias_ty.is_none() => {
                for field in &s.fields {
                    collect_anon_shapes_in_type(&field.ty, &mut synthetic_items);
                }
            }
            Item::Function(f) => {
                for param in &f.params {
                    collect_anon_shapes_in_type(&param.ty, &mut synthetic_items);
                }
                collect_anon_shapes_in_type(&f.return_type, &mut synthetic_items);
                collect_anon_shapes_in_stmts(&f.body.stmts, &mut synthetic_items);
            }
            Item::ConstDecl(c) => {
                if let Some(ty) = &c.ty {
                    collect_anon_shapes_in_type(ty, &mut synthetic_items);
                }
            }
            _ => {}
        }
    }
    // Deduplicate: two identical inline shapes produce the same synthetic name; keep one.
    {
        let mut seen_names: HashSet<String> = HashSet::new();
        synthetic_items.retain(|s| seen_names.insert(s.name.clone()));
    }

    // Pass 1: collect all shape names + their raw AST data.
    let mut all_names: HashSet<String> = HashSet::new();
    // Register synthetic anonymous shapes first so they're visible to all real shapes.
    for s in &synthetic_items {
        all_names.insert(s.name.clone());
    }
    for item in &module.items {
        if let Item::ShapeDecl(s) = item {
            // M6: skip alias declarations (`shape Shape = Circle | Square`) — they're not
            // regular shapes with fields. UnionAliasTable handles them in typeck.
            if s.alias_ty.is_some() {
                continue;
            }

            if all_names.contains(&s.name) {
                diags.push(Diagnostic::error(
                    s.name_span.clone(),
                    format!(
                        "A shape named `{}` is already defined in this file.",
                        s.name
                    ),
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
        shapes: all_names
            .iter()
            .map(|n| {
                (
                    n.clone(),
                    ShapeDef {
                        name: n.clone(),
                        is_base: false,
                        extends: None,
                        follows: vec![],
                        fields: vec![],
                        contract_sigs: vec![],
                        defined_at: SourceSpan::new("", 0, 0),
                    },
                )
            })
            .collect(),
        union_aliases: HashMap::new(),
        options_names: table.options_names.clone(),
    };

    // Pass 2: resolve each shape's own fields, contract sigs, extends, and follows.
    // Process synthetic anonymous shapes first.
    for s in &synthetic_items {
        let ty = name_table.resolve_ast_type(&ynz_ast::nodes::Type::Nothing); // dummy
        let _ = ty;
        let mut own_fields: Vec<FieldDef> = Vec::new();
        for field in &s.fields {
            let ty = name_table.resolve_ast_type(&field.ty);
            // Inline anonymous shapes get the same unknown-field-type validation as
            // named shapes — without this, `{ x: UnknownType }` silently resolves to
            // Type::Error with no diagnostic. Use "inline type" as the display name
            // so error messages don't expose the `__anon__` internal identifier.
            emit_unknown_field_type_diag(
                &field.ty,
                &field.name,
                "inline type",
                &name_table,
                &[],
                diags,
            );
            own_fields.push(FieldDef {
                name: field.name.clone(),
                ty,
                is_hidden: field.is_hidden,
                is_inherited: false,
                defined_at: field.name_span.clone(),
            });
        }
        table.shapes.insert(
            s.name.clone(),
            ShapeDef {
                name: s.name.clone(),
                is_base: false,
                extends: None,
                follows: vec![],
                fields: own_fields,
                contract_sigs: vec![],
                defined_at: s.span.clone(),
            },
        );
    }

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
                    format!(
                        "Field `{}` is already declared on `{}`.",
                        field.name, s.name
                    ),
                    "Each field in a shape must have a unique name.",
                    "Two fields with the same name would make it impossible to tell them apart.",
                ));
                continue;
            }
            let ty = if let AstType::AnonShape {
                fields: anon_fields,
                ..
            } = &field.ty
            {
                Type::Shape {
                    name: canonical_anon_name(anon_fields),
                }
            } else {
                name_table.resolve_ast_type(&field.ty)
            };
            // Collect the shape's own type parameter names so the diagnostic
            // function can skip them (type params resolve to Type::Error in the
            // shape table but are not unknown types — they're substituted at use site).
            let type_param_names: Vec<&str> = s.generics.iter().map(|g| g.name.as_str()).collect();
            // Scan the type annotation for unknown named types and emit targeted diagnostics.
            emit_unknown_field_type_diag(
                &field.ty,
                &field.name,
                &s.name,
                &name_table,
                &type_param_names,
                diags,
            );
            if field.is_hidden && field.default.is_none() {
                let tn = type_name(&ty);
                let (what_instead, why) = if type_is_maybe(&ty) {
                    (
                        format!(
                            "Default to `none` since the type allows absence — \
                             `hidden {}: {} = none`.",
                            field.name, tn
                        ),
                        "Hidden fields can't be set by code in other files. Without a default, \
                         external construction would leave the field in an undefined state. \
                         Your field's type is `maybe ...`, so `none` is the natural default."
                            .to_string(),
                    )
                } else {
                    (
                        format!("Provide one — `hidden {}: {} = <value>`.", field.name, tn),
                        format!(
                            "Hidden fields can't be set by code in other files. Without a default, \
                             external construction would leave the field in an undefined state. \
                             If no sensible default applies here, change the type to \
                             `maybe {}` and default to `none`: \
                             `hidden {}: maybe {} = none`. \
                             The type then says `sometimes absent` and same-file code reads it \
                             through `.exists()` guards.",
                            tn, field.name, tn
                        ),
                    )
                };
                diags.push(Diagnostic::error(
                    field.name_span.clone(),
                    format!("Hidden field `{}` has no default value.", field.name),
                    what_instead,
                    why,
                ));
            }
            own_fields.push(FieldDef {
                name: field.name.clone(),
                ty,
                is_hidden: field.is_hidden,
                is_inherited: false,
                defined_at: field.name_span.clone(),
            });
        }

        // Resolve contract sigs.
        let contract_sigs: Vec<ContractSigDef> = s
            .contract_sigs
            .iter()
            .map(|sig| {
                let param_tys: Vec<Type> = sig
                    .params
                    .iter()
                    .map(|p| name_table.resolve_ast_type(&p.ty))
                    .collect();
                let ret_ty = name_table.resolve_ast_type(&sig.return_type);
                ContractSigDef {
                    name: sig.name.clone(),
                    param_tys,
                    param_names: sig.params.iter().map(|p| p.name.clone()).collect(),
                    param_ownerships: sig.params.iter().map(|p| p.ownership.clone()).collect(),
                    receiver: sig.receiver.clone(),
                    ret_ty,
                }
            })
            .collect();

        table.shapes.insert(
            s.name.clone(),
            ShapeDef {
                name: s.name.clone(),
                is_base: s.is_base,
                extends,
                follows,
                fields: own_fields, // inherited fields added in pass 3
                contract_sigs,
                defined_at: s.name_span.clone(),
            },
        );
    }

    // Pass 3: detect cyclic extends chains, then flatten inherited fields.
    detect_extends_cycles(&table, diags);
    flatten_inherited_fields(&mut table, diags);

    // Pass 4: detect direct field-type cycles (shape A has field of type shape A).
    detect_field_cycles(&table, diags);

    // M6: collect union type aliases (shape Shape = Circle | Square).
    // Must happen after all regular shapes are in `table` so variant types can be resolved.
    for item in &module.items {
        if let Item::ShapeDecl(s) = item {
            if let Some(alias_ast_ty) = &s.alias_ty {
                let resolved = table.resolve_ast_type(alias_ast_ty);
                table.union_aliases.insert(s.name.clone(), resolved);
            }
        }
    }

    table
}

fn detect_extends_cycles(table: &ShapeTable, diags: &mut DiagnosticBucket) {
    for name in table.shapes.keys() {
        // Perf: HashSet for O(1) membership checks; Vec built separately for the
        // chain string needed in the diagnostic message only when a cycle is found.
        let mut visited_set: HashSet<&str> = HashSet::new();
        let mut chain: Vec<String> = vec![name.clone()];
        visited_set.insert(name.as_str());
        let mut current = name.clone();
        while let Some(def) = table.shapes.get(&current) {
            let Some(parent) = &def.extends else { break };
            if visited_set.contains(parent.as_str()) {
                let chain_str = chain.join(" → ");
                if let Some(def) = table.shapes.get(name) {
                    diags.push(Diagnostic::error(
                        def.defined_at.clone(),
                        format!("`{name}` has a cyclic `extends` chain: {chain_str} → {parent}"),
                        "Break the cycle by removing one of the `extends` declarations.",
                        "`extends` is for data inheritance — a shape cannot inherit from itself, directly or through a chain.",
                    ));
                }
                break;
            }
            visited_set.insert(parent.as_str());
            chain.push(parent.clone());
            current = parent.clone();
        }
    }
}

fn flatten_inherited_fields(table: &mut ShapeTable, _diags: &mut DiagnosticBucket) {
    // Process shapes in parent-first (depth-first) order so that when we flatten
    // C extends B extends A, B is always fully flattened before C visits it.
    // HashMap iteration order is non-deterministic — without this ordering, C could
    // grab B's not-yet-flattened fields and silently miss A's fields when iteration
    // happens to visit C before B.
    let mut done: HashSet<String> = HashSet::new();
    // `visiting` tracks shapes currently on the DFS stack. If we see a shape already
    // in `visiting` during recursion, that's a cycle — skip rather than stack-overflow.
    // detect_extends_cycles already emitted a diagnostic for the cycle; we just stop.
    let mut visiting: HashSet<String> = HashSet::new();
    let names: Vec<String> = table.shapes.keys().cloned().collect();
    for name in &names {
        flatten_recursive(name, table, &mut done, &mut visiting);
    }
}

/// Recursively flatten a single shape, ensuring its parent is fully flattened first.
///
/// Time: O(n) where n = number of shapes in the table — each shape's body runs once
///       thanks to the `done` memo. Space: O(d) where d = max inheritance depth (the
///       DFS call stack).
///
/// - `done`: shapes whose full field list has been computed. Re-entry is a no-op.
/// - `visiting`: shapes currently on the DFS call stack. A name appearing in `visiting`
///   signals a cycle (already diagnosed by `detect_extends_cycles`). We skip rather
///   than recurse infinitely.
fn flatten_recursive(
    name: &str,
    table: &mut ShapeTable,
    done: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) {
    if done.contains(name) {
        return;
    }
    if visiting.contains(name) {
        // Cycle detected — detect_extends_cycles already emitted the diagnostic. Stop.
        return;
    }

    visiting.insert(name.to_string());

    // Clone the parent name out so we don't hold an immutable borrow while we recurse.
    let parent_name = table.shapes.get(name).and_then(|s| s.extends.clone());

    if let Some(parent) = parent_name {
        if table.shapes.contains_key(&parent) {
            flatten_recursive(&parent, table, done, visiting);
        }

        // Parent is now fully flattened (or unknown — already errored in detect_extends_cycles).
        let parent_fields: Vec<FieldDef> = match table.shapes.get(&parent) {
            Some(p) => p
                .fields
                .iter()
                .map(|f| FieldDef {
                    name: f.name.clone(),
                    ty: f.ty.clone(),
                    is_hidden: f.is_hidden,
                    is_inherited: true,
                    defined_at: f.defined_at.clone(),
                })
                .collect(),
            None => {
                visiting.remove(name);
                done.insert(name.to_string());
                return;
            }
        };

        // Prepend parent fields to child's own fields, skipping overridden names.
        let child = table.shapes.get_mut(name).unwrap();
        let own_names: HashSet<&str> = child.fields.iter().map(|f| f.name.as_str()).collect();
        let mut all_fields: Vec<FieldDef> = parent_fields
            .into_iter()
            .filter(|f| !own_names.contains(f.name.as_str()))
            .collect();
        all_fields.append(&mut child.fields);
        child.fields = all_fields;
    }

    visiting.remove(name);
    done.insert(name.to_string());
}

fn type_is_maybe(t: &Type) -> bool {
    matches!(t, Type::Maybe { .. })
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

// ── Generic shape collection (M5 P3a) ────────────────────────────────────────

/// Collect all generic shape declarations (`shape Pair<A, B> { ... }`) into a
/// `GenericShapeTable`.
///
/// Called from the same pre-pass as `collect_shapes`. Generic shapes are kept
/// separate because their field types contain `TypeParam` placeholders — they
/// cannot be stored in `ShapeTable` (which only holds concrete types).
pub fn collect_generic_shapes(module: &Module, diags: &mut DiagnosticBucket) -> GenericShapeTable {
    let mut table = GenericShapeTable::default();

    for item in &module.items {
        let Item::ShapeDecl(s) = item else { continue };
        if s.generics.is_empty() {
            continue; // non-generic — handled by collect_shapes
        }

        if table.contains(&s.name) {
            diags.push(Diagnostic::error(
                s.name_span.clone(),
                format!(
                    "A generic shape named `{}` is already defined in this file.",
                    s.name
                ),
                "Rename one of the two shapes.",
                "Yinz does not allow two shapes with the same name in the same file.",
            ));
            continue;
        }

        let type_params: Vec<String> = s.generics.iter().map(|gp| gp.name.clone()).collect();

        // Resolve field types — TypeParam references stay as TypeParam.
        let mut fields: Vec<FieldDef> = Vec::new();
        for field in &s.fields {
            let ty = resolve_field_type_in_generic_shape(&field.ty, &type_params);
            fields.push(FieldDef {
                name: field.name.clone(),
                ty,
                is_hidden: field.is_hidden,
                is_inherited: false,
                defined_at: field.name_span.clone(),
            });
        }

        let follows: Vec<String> = s.follows.iter().map(|(c, _)| c.clone()).collect();

        table.shapes.insert(
            s.name.clone(),
            GenericShapeDef {
                name: s.name.clone(),
                type_params,
                fields,
                follows,
                defined_at: s.name_span.clone(),
            },
        );
    }

    table
}

/// Resolve a field type annotation inside a generic shape body.
///
/// Names that appear in `type_params` become `Type::TypeParam`; all other names
/// follow normal resolution (built-in primitives only at P3a — concrete shapes
/// in generic field positions are P3b+).
fn resolve_field_type_in_generic_shape(ast_ty: &AstType, type_params: &[String]) -> Type {
    match ast_ty {
        AstType::Nothing => Type::Nothing,
        AstType::Int => Type::Int,
        AstType::Float => Type::Float,
        AstType::Number { .. } => Type::Number { precision: 34 },
        AstType::Bool => Type::Bool,
        AstType::Named(n, _) if n == "string" => Type::String,
        AstType::Named(n, _) if type_params.contains(n) => Type::TypeParam { name: n.clone() },
        AstType::Generic { name, args, .. } => {
            let resolved_args = args
                .iter()
                .map(|a| resolve_field_type_in_generic_shape(a, type_params))
                .collect();
            Type::Generic {
                name: name.clone(),
                args: resolved_args,
            }
        }
        _ => Type::Error,
    }
}

/// Emit a diagnostic when a shape field's type annotation contains an unrecognized
/// type name. Walks the AstType to find the first unknown Named node and points at it.
/// Compute the canonical synthetic name for an anonymous inline shape from its field list.
///
/// The name is content-based: fields are sorted by name and each contributes a
/// `_fieldname_typename` segment. This ensures structural equality —
/// `{ a: int, b: string }` and `{ b: string, a: int }` produce the same name
/// regardless of where in the file they appear.
///
/// Example: `{ bid: number, ask: number }` → `__anon__ask_number__bid_number`
pub(crate) fn canonical_anon_name(fields: &[ynz_ast::nodes::FieldDecl]) -> String {
    let mut sorted: Vec<_> = fields.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let segments: Vec<String> = sorted
        .iter()
        .map(|f| format!("{}__{}", f.name, ast_type_short_name(&f.ty)))
        .collect();
    format!("__anon__{}", segments.join("__"))
}

/// Produce a short, stable, identifier-safe name for an AST type — used only for
/// building canonical anon-shape names, not for user-facing output.
fn ast_type_short_name(ty: &AstType) -> String {
    match ty {
        AstType::Int => "int".to_string(),
        AstType::Float => "float".to_string(),
        AstType::Number { .. } => "number".to_string(),
        AstType::Bool => "boolean".to_string(),
        AstType::Named(n, _) => n.clone(),
        AstType::Nothing => "nothing".to_string(),
        AstType::Maybe { inner, .. } => format!("maybe_{}", ast_type_short_name(inner)),
        AstType::Generic { name, args, .. } => {
            let arg_str: Vec<_> = args.iter().map(ast_type_short_name).collect();
            format!("{}__{}", name, arg_str.join("__"))
        }
        AstType::AnonShape { fields, .. } => canonical_anon_name(fields),
        AstType::Union { variants, .. } => {
            let parts: Vec<_> = variants.iter().map(ast_type_short_name).collect();
            format!("union_{}", parts.join("_or_"))
        }
        AstType::ErrorCapable { inner, .. } => format!("errors_{}", ast_type_short_name(inner)),
        AstType::Sensitive(inner) => format!("sensitive_{}", ast_type_short_name(inner)),
        _ => "unknown".to_string(),
    }
}

/// Recursively extract AnonShape nodes from a type annotation, registering each as a
/// synthetic named ShapeDecl. The synthesized name is content-based (canonical) so that
/// structurally identical inline shapes share the same synthetic name regardless of
/// where they appear.
fn collect_anon_shapes_in_type(ast_ty: &AstType, out: &mut Vec<ynz_ast::nodes::ShapeDecl>) {
    match ast_ty {
        AstType::AnonShape { fields, span } => {
            let synth_name = canonical_anon_name(fields);
            out.push(ynz_ast::nodes::ShapeDecl {
                name: synth_name,
                name_span: span.clone(),
                is_base: false,
                generics: vec![],
                extends: None,
                follows: vec![],
                fields: fields.clone(),
                contract_sigs: vec![],
                alias_ty: None,
                span: span.clone(),
                is_exported: false,
                doc: None,
            });
            // Recurse into nested anon shapes inside field types.
            for field in fields {
                collect_anon_shapes_in_type(&field.ty, out);
            }
        }
        AstType::Union { variants, .. } => {
            for v in variants {
                collect_anon_shapes_in_type(v, out);
            }
        }
        AstType::Maybe { inner, .. } => {
            collect_anon_shapes_in_type(inner, out);
        }
        AstType::Generic { args, .. } => {
            for a in args {
                collect_anon_shapes_in_type(a, out);
            }
        }
        AstType::ErrorCapable { inner, .. } => {
            collect_anon_shapes_in_type(inner, out);
        }
        AstType::Sensitive(inner) => {
            collect_anon_shapes_in_type(inner, out);
        }
        _ => {}
    }
}

/// Walk function body statements, collecting any AnonShape type annotations from
/// `let`/`const` bindings into `out`.
fn collect_anon_shapes_in_stmts(stmts: &[Stmt], out: &mut Vec<ynz_ast::nodes::ShapeDecl>) {
    for stmt in stmts {
        match stmt {
            Stmt::Let { ty: Some(ty), .. } => {
                collect_anon_shapes_in_type(ty, out);
            }
            Stmt::If { body, .. } => {
                collect_anon_shapes_in_stmts(&body.stmts, out);
            }
            Stmt::Match { arms, else_arm, .. } => {
                for arm in arms {
                    collect_anon_shapes_in_stmts(&arm.body.stmts, out);
                }
                if let Some(else_block) = else_arm {
                    collect_anon_shapes_in_stmts(&else_block.stmts, out);
                }
            }
            Stmt::While { body, .. } => {
                collect_anon_shapes_in_stmts(&body.stmts, out);
            }
            Stmt::For { body, .. } => {
                collect_anon_shapes_in_stmts(&body.stmts, out);
            }
            _ => {}
        }
    }
}

/// Walk an AstType annotation and emit a diagnostic for each unknown named type.
/// Used at shape-collection time so the error points at the declaration, not usage.
fn emit_unknown_field_type_diag(
    ast_ty: &AstType,
    field_name: &str,
    shape_name: &str,
    name_table: &ShapeTable,
    type_params: &[&str],
    diags: &mut DiagnosticBucket,
) {
    let known = |n: &str| -> bool {
        matches!(
            n,
            "int"
                | "float"
                | "number"
                | "boolean"
                | "string"
                | "nothing"
                | "none"
                | "range"
                | "Frame"
                | "SourceLoc"
        ) || name_table.contains(n)
            || name_table.union_aliases.contains_key(n)
            || name_table.options_names.contains(n)
            || type_params.contains(&n)
    };

    match ast_ty {
        AstType::Named(n, span) if !known(n) => {
            diags.push(Diagnostic::error(
                span.clone(),
                format!("`{n}` is not a known type."),
                format!("Field `{field_name}` on `{shape_name}` cannot use `{n}`. Use a built-in or a `shape` name defined in this file."),
                "Built-ins: `int`, `float`, `number`, `boolean`, `string`. Collections: `array<T>`, `fixed<T>`, `map<K, V>`. Optionals: `maybe<T>`.",
            ));
        }
        AstType::Union { variants, .. } => {
            for v in variants {
                emit_unknown_field_type_diag(
                    v,
                    field_name,
                    shape_name,
                    name_table,
                    type_params,
                    diags,
                );
            }
        }
        AstType::Maybe { inner, .. } => {
            emit_unknown_field_type_diag(
                inner,
                field_name,
                shape_name,
                name_table,
                type_params,
                diags,
            );
        }
        AstType::Generic { args, .. } => {
            for a in args {
                emit_unknown_field_type_diag(
                    a,
                    field_name,
                    shape_name,
                    name_table,
                    type_params,
                    diags,
                );
            }
        }
        _ => {}
    }
}
