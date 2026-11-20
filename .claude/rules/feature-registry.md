---
name: Yinz Feature Registry Protocol
description: "Applies when adding a user-facing feature item: registry/features.toml comes first, code follows."
paths:
  - registry/features.toml
  - crates/ynz-registry/**
  - crates/ynz-diagnostics/src/**
  - crates/ynz-typeck/src/**
  - crates/ynz-parser/src/**
---

**Every new user-facing feature item goes in `registry/features.toml` first; code follows the registry.** Why: the registry is the single source, so lists the code generates from it cannot drift.

Add the entry kind that matches:

| Adding | Entry kind | Must include |
|---|---|---|
| Lexer keyword | `[[keyword]]` | |
| Foreign keyword Yinz rejects | `[[banned_declaration_keyword]]` | error text in `what_instead` and `why` |
| Word banned from error messages | `[[banned_jargon]]` | replacement word |
| Primitive method or free function | `[[primitive_intrinsic]]` (`kind = "free_fn"` for free functions) | every overload |
| Type-attached constant (`Int.max`) | `[[type_attached_constant]]` | both the typeck `value_type` and the codegen `value_literal` |
| Reserved, deferred language or tooling feature | `[[deferred_language_feature]]` / `[[deferred_tooling_feature]]` | WHY, SUBSTITUTE, SHIPS_IN, DESIGN_DOC |
| Reusable, canonical diagnostic message | `[[diagnostic_template]]` | per-site dynamic messages stay in code |
| IDE muted-hint domain | `[[muted_hint_domain]]` | `placement_category` exactly `"Addition"`, `"Replacement"` or `"Informational"`; the build rejects anything else |

**Carve-out:** a constant needs no entry when it is `#[cfg(test)]` or compiler-internal (no error message or IDE feature uses it). Put `// CARVE-OUT: <reason>` on its definition line. Why: a diff check flags unannotated `pub const` / `pub static` string arrays in the diagnostics, typeck and parser crates.

**If unsure, add the entry.** Why: an extra entry is easy to remove, and drift is hard to spot.

**A plan that adds feature items lists the registry entries it adds or changes, or says it adds none.** Why: code and registry then ship together, and a forgotten entry is caught at plan time.
