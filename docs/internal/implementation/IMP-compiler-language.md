---
name: "IMP-compiler-language"
description: "The Yinz compiler is written in Rust: cargo workspace of small crates, hand-written recursive-descent parser, ariadne diagnostics, salsa incremental queries shared by the CLI and LSP, inkwell for LLVM codegen."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-19"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Compiler Implementation Language

Design document for the language and toolchain the Yinz compiler is built with. Not user-facing.

---

## Stack

- **Language:** Rust (stable channel).
- **Build:** Cargo workspace; the compiler is split into crates (`ynz-ast`, `ynz-parser`, `ynz-typeck`, `ynz-mir`, `ynz-codegen`, `ynz-driver`).
- **Parser:** hand-written recursive descent, not a parser generator.
- **Diagnostics:** `ariadne` renders errors with spans and suggestions.
- **Incremental computation:** `salsa` (the framework `rust-analyzer` is built on) gives incremental builds and the LSP architecture.
- **LLVM codegen:** `inkwell`, a safe LLVM wrapper with no runtime cost over the C++ API.
- **Later, optional:** `cranelift` as a fast-debug-build backend.

---

## Hand-written recursive descent

Golden Rule 3 says the compiler is a teacher, so error messages are a first-class feature. A hand-written parser gives:

- **Position-specific suggestions.** After a parameter list with no return type, the parser can suggest `-> Nothing` or a type at that exact span.
- **Error recovery.** It skips to the next sensible boundary (next `}`, next statement) and keeps parsing, so one compile reports all errors.
- **Multi-error reporting.** Errors accumulate instead of stopping at the first.

The cost is more parser code than a declared grammar; the error-message quality is worth it.

---

## Salsa: one pipeline for CLI and LSP

The compiler binary ships two frontends:

1. **`ynz` CLI:** `ynz build`, `ynz run`, `ynz test`.
2. **`ynz-lsp`:** the language server for editors (autocomplete, hover, go-to-definition, rename, inline errors).

Both run the same work (parse, type-check, analyze). Salsa is a demand-driven query framework: queries such as `parse(file_id) -> AST` and `type_of(node_id) -> Type` are memoized and their dependents are invalidated when inputs change. One code path serves both frontends, so the LSP exposes existing queries over JSON-RPC and no second pipeline exists. Salsa is in from day 1; adding it later would be a large retrofit.

Both frontends also share the **SSOT feature registry** ([`registry/features.toml`](../../../registry/features.toml), parsed by `crates/ynz-registry/build.rs`). It is the single source of truth for keyword lists, banned-jargon words, primitive intrinsics, type-attached constants, the deferred-feature catalog and IDE muted-hint domains. The CLI reads it to generate diagnostics; the LSP reads it to generate autocomplete suggestions, hover docs and muted hints. See [`IMP-feature-registry.md`](IMP-feature-registry.md) for the schema and [`IMP-lsp.md`](IMP-lsp.md) for how the LSP consumes it.

---

## Compile-time cost and mitigations

`rustc` compile times are the known downside. They are mitigated by:

- Cargo workspace with small crates, so only changed crates recompile.
- `sccache` for a shared compilation cache.
- `mold` linker for fast incremental links.
- `cargo check` (no codegen) for the inner-loop type-check workflow.

---

## Bootstrap and self-hosting

The Rust compiler is the *bootstrap* compiler. Yinz will eventually self-host, with the compiler written in Yinz. The Rust compiler stays the reference implementation until the self-hosted compiler reaches feature parity. Self-hosting is a v2+ concern (see [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md)).

Decided: Rust over Zig, TypeScript and C++ (search git history for "compiler-language").
