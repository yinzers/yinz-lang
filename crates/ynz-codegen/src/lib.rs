//! Yinz LLVM code generator.
//!
//! # Pipeline
//!
//! Typed AST ([`ynz_typeck::TypedModule`]) → [`codegen_query`] → [`CompiledArtifact`]
//! (relocatable object bytes + LLVM IR text + SHA-256 hash).
//!
//! # Entry points
//!
//! - [`codegen_query`] — Salsa-tracked query; given a [`ynz_parser::SourceFile`], runs
//!   the full pipeline (lex → parse → typecheck → codegen) and returns a
//!   [`CodegenOutput`] with the artifact and any diagnostics.  Re-running on unchanged
//!   input returns a cached result.
//! - [`sha256`] — standalone digest helper used for determinism checks in tests.
//!
//! # Module layout
//!
//! - `emit` — the main code-generation module.  [`emit::build_module`] is the 5-pass
//!   driver: Pass 0 (LLVM struct types for shapes), Pass 1 (forward-declare functions),
//!   Pass 1.5 (forward-declare monomorphized generics), Pass 1.6 (vtable globals), and
//!   Pass 2 (emit function bodies).
//! - `runtime_decls` — LLVM external-function declarations for every `ynz_*` C-ABI
//!   runtime shim. Kept separate so `emit` can reference them without circular imports.
//! - `shape_types` — maps `ShapeTable` entries to LLVM struct types.
//! - `vtable` — generates vtable globals for `dynamic Foo` dispatch.
//! - `artifact` — [`CompiledArtifact`] type + the hand-rolled SHA-256 implementation.

pub mod artifact;
pub mod emit;
pub mod queries;
pub mod runtime_decls;
pub mod shape_types;
pub mod vtable;

pub use artifact::{sha256, CompiledArtifact};
pub use queries::{codegen_query, CodegenOutput};
