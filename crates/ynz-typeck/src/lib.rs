//! Yinz type checker.
//!
//! # Pipeline
//!
//! Parsed [`ynz_ast::nodes::Module`] → [`module_signatures_query`] (pre-pass) →
//! [`check_query`] (full check) → [`CheckOutput`] with a [`TypedModule`] and diagnostics.
//!
//! # Entry points
//!
//! - [`module_signatures_query`] — Salsa-tracked; collects shapes, function signatures,
//!   and imported symbols.  Cross-file import resolution runs here so that shape field
//!   type annotations can reference imported shapes and options types.  Returns a
//!   [`SignatureOutput`] — the input to the body-checking pass.
//! - [`check_query`] — Salsa-tracked; depends on `module_signatures_query` and
//!   `parse_query`.  Runs the full type-checking pass over all function bodies.
//!   Returns a [`CheckOutput`] with the typed module, monomorphization table, and
//!   combined diagnostics.
//! - [`check`] — the direct (non-Salsa) entry point used in unit tests; takes the
//!   pre-built tables as arguments.
//! - [`type_attached_const_type`] — resolves `int.max`, `number.epsilon`, etc. to their
//!   types; also used by the codegen to decide which constant to emit.
//!
//! # Module layout
//!
//! - `check` — the main [`Checker`] type; hosts `check_expr`, `infer_expr`, `check_stmt`
//!   and all specialised sub-checkers.  Flow-sensitive sets (`maybe_non_none`,
//!   `errors_success_narrowed`, etc.) live here.
//! - `shapes` — `collect_shapes`: builds [`ShapeTable`] from shape declarations.
//! - `signatures` — `collect_signatures`: builds [`SignatureTable`] from function items.
//! - `generics` — generic function / shape tables; monomorphization engine.
//! - `resolve_import` — cross-file import resolution; path validation + cycle detection.
//! - `scope` — the scoped name-resolution table used during body checking.
//! - `intrinsics` — primitive method dispatch table (`int.toString()`, `string.count()`, …).
//! - `options_table` — `collect_options`: maps `options` declarations to tag integers.
//! - `exports` — [`ExportTable`] used by the Salsa incremental build to detect
//!   cross-file signature changes.
//! - `return_paths` — control-flow analysis to verify all non-nothing functions return.
//! - `builtins` — built-in shape type helper (`Frame`, `SourceLoc`).
//! - `types` — the [`Type`] enum: all types the type checker knows.

pub mod ast_offset;
pub mod builtins;
pub mod inlay_hint_passes;
pub mod type_at_offset;
pub mod check;
pub mod exports;
pub mod generics;
pub mod intrinsics;
pub mod options_table;
pub mod queries;
pub mod resolve_import;
pub mod return_paths;
pub mod scope;
pub mod symbol_lookup;
pub mod shapes;
pub mod signatures;
pub mod types;

pub use check::{check, type_attached_const_type, TypedModule};
pub use generics::{GenericFnTable, GenericShapeTable, MonomorphizationTable};
pub use intrinsics::PrimitiveIntrinsicTable;
pub use queries::{check_query, module_signatures_query, CheckOutput, SignatureOutput};
pub use shapes::{ShapeDef, ShapeTable};
pub use signatures::SignatureTable;
pub use symbol_lookup::{
    cross_file_reference_count_estimate, def_site_for_offset, references_for_offset,
    rename_locations, resolve_symbol_at, RenameError, ResolvedSymbol, SymbolKind,
};
pub use types::Type;
pub use type_at_offset::type_of_expression_at_offset;
pub use inlay_hint_passes::{
    array_to_fixed_promotion_hints, copy_point_hints, let_to_const_promotion_hints,
    ownership_call_site_hints, variable_type_hints, CopyHint, OwnershipHint, PromotionHint,
    PromotionKind, TypeHint,
};
