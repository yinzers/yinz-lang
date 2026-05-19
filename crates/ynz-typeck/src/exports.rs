use std::collections::HashMap;

use ynz_ast::nodes::{Item, Module};

use crate::{
    options_table::{OptionsEntry, OptionsTable},
    shapes::{ShapeDef, ShapeTable},
    signatures::{FunctionSig, SignatureTable},
};

/// The symbols a module exposes to importers — only `export`-prefixed declarations.
///
/// Keys are the exported names. Options/shapes/functions are stored by the name
/// as declared in the exporting file; the importing file may rebind via `as alias`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportTable {
    pub shapes: HashMap<String, ShapeDef>,
    pub options: HashMap<String, OptionsEntry>,
    pub functions: HashMap<String, FunctionSig>,
}

impl ExportTable {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.options.is_empty() && self.functions.is_empty()
    }
}

/// Build the ExportTable for a module from already-resolved tables.
///
/// Only items with `is_exported = true` in the AST appear in the result.
/// The AST items are the source of truth for the flag; the resolved tables
/// supply the fully-typed ShapeDef/OptionsEntry/FunctionSig values.
pub fn collect_exports(
    module: &Module,
    shape_table: &ShapeTable,
    options_table: &OptionsTable,
    sig_table: &SignatureTable,
) -> ExportTable {
    let mut table = ExportTable::empty();

    for item in &module.items {
        match item {
            Item::ShapeDecl(s) if s.is_exported && s.alias_ty.is_none() => {
                if let Some(def) = shape_table.get(&s.name) {
                    table.shapes.insert(s.name.clone(), def.clone());
                }
            }
            Item::OptionsDecl(o) if o.is_exported => {
                if let Some(entry) = options_table.get(&o.name) {
                    table.options.insert(o.name.clone(), entry.clone());
                }
            }
            Item::Function(f) if f.is_exported => {
                if let Some(sig) = sig_table.fns.get(&f.name) {
                    table.functions.insert(f.name.clone(), sig.clone());
                }
            }
            _ => {}
        }
    }

    table
}
