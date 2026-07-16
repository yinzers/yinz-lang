---
name: "IMP-ffi"
description: "Design rationale for Yinz's foreign function interface: the 'foreign' keyword over 'unsafe', the always-wrap-in-safe-Yinz-functions pattern, and required 'wait' on foreign calls."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Foreign Function Interface — Design Decisions

User spec: [`docs/reference/REF-ffi.md`](../../reference/REF-ffi.md)

> **Status: deferred to v2+, not implemented.** `foreign` is a registered
> `[[deferred_language_feature]]` (`registry/features.toml`, design doc
> [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) v2+ section) — it is not yet
> a reserved keyword, and none of the mechanisms below exist in the compiler today. Stdlib modules
> that need C interop call C internally via compiler-private mechanisms users never see. The
> decisions below are the locked DESIGN for when v2+ builds this out, not a description of current
> behavior.

---

## `foreign` Keyword over `unsafe`

`foreign function name(...) from "library"` — the keyword describes what the code IS, not just that it's dangerous.

**Why not `unsafe`**: `unsafe` (Rust's approach) tells you something is dangerous without telling you why. `foreign` tells you why — this code comes from another language. A junior developer reading `foreign function c_sqrt(...)` immediately understands the nature of the call. Golden Rule 12.

---

## Always Wrap in Safe Yinz Functions

The foreign declaration is never used directly by the rest of the codebase. It's wrapped immediately in a safe Yinz function with proper types, ownership annotations, and error handling.

**Why**: The foreign boundary is where safety guarantees break down. Keeping it small and contained limits the blast radius. Everything outside the wrapper is fully analyzable by the compiler. This is the same principle as keeping I/O at the edges of a system.

---

## Compiler Requires `wait` on All Foreign Calls

The compiler cannot analyze foreign code. It doesn't know if a foreign function has side effects, blocks, or modifies shared state. Requiring `wait` prevents the auto-parallelization system from accidentally racing foreign calls.

**Why not just exclude from parallelization silently**: Silent exclusion from parallelization would hide a potential performance issue. Requiring `wait` makes the exclusion explicit — the developer sees `wait c_read(...)` and knows this is a sequential call.

---

## Open Questions

- **C type mapping**: How do C types like `void*`, `char*`, `struct*`, and function pointers map to Yinz types? Is there a raw `pointer` type? See [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../scratchpad/SCRATCH-open-questions.md).
- **Error handling**: How do C-style error codes (return -1 on failure) integrate with the `errors` system? The wrapper function presumably converts, but the interface needs design.
