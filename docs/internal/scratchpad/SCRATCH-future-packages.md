---
name: "SCRATCH-future-packages"
description: "Design notes on the compiled Yinz package binary format, specifying the per-export metadata (may-block flag, ownership signature, kernel-mode compatibility) it must reserve space for now and populate in v0.2."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Compiled Package Binary Format — Metadata Reservation

**Status**:
- **Locked design, v0.1 binary-format-reservation**: the binary format must reserve space for the metadata fields described below NOW. Bake-in-now item.
- **Locked, v0.2 implementation**: actual population of the metadata happens when v0.2 ships compiled packages.

User spec target: [`docs/reference/REF-packages.md`](../../reference/REF-packages.md) (already exists for source-level package model; this adds the BINARY format spec).

---

## The Decision

Yinz compiled packages (the binary artifact a Yinz library ships, equivalent to Rust's `.rlib` or Go's compiled archives) MUST embed enough metadata for downstream consumers to do all the compile-time analyses Yinz relies on **without re-parsing source**.

The binary format is designed and partially reserved in v0.1 even though most metadata won't be populated until v0.2. **Retrofitting later is painful** — every compiled package shipped before the reservation would have to be rebuilt or worked around. Reserving the format NOW costs nothing; doing it later costs everything.

---

## What metadata the binary format must include

Per exported function/type/value:

| Metadata field | Purpose | Used by |
|----------------|---------|---------|
| **May-block flag** | Does this function (or anything it transitively calls) suspend on I/O or call a may-block FFI? | Concurrency analysis ([`docs/internal/implementation/IMP-no-function-coloring.md`](../implementation/IMP-no-function-coloring.md)) — consumers do whole-program wait-insertion across package boundaries |
| **Ownership signature** | Param-by-param: is it `share`, `lend`, or `give`? Return type ownership. | Ownership analysis at call sites in consumer code. IDE muted-hint generation. |
| **Kernel-mode compatibility flags** | Does this require heap? Scheduler? Default panic handler? OS file I/O? | `--kernel` mode ([`docs/internal/implementation/IMP-no-runtime-mode.md`](../implementation/IMP-no-runtime-mode.md)) — consumers in kernel mode must reject uses of incompatible package items at compile time |
| **Allocator requirements** | Does this use the default allocator implicitly, or accept one via `.in(...)`? | Allocator-aware code generation. Arena-allocator integration. |
| **LLVM attribute hints** | Was this function's params declared `share` (gets `readonly` LLVM attribute)? Const-binding contexts? | LLVM codegen in consumer — preserve aliasing optimizations across package boundaries |
| **Self-referential markers** | Which shapes have self-references? Which fields are internal references? | Self-references analysis ([`docs/internal/scratchpad/SCRATCH-future-self-references.md`](SCRATCH-future-self-references.md)) — consumer code can safely move/copy these shapes |
| **Diagnostic provenance** | Source-position info for error messages that point into a precompiled package | Compiler-error rendering — even when the source isn't available, errors still cite the original location |
| **Doc comments** | The `///` doc comments on exported items | IDE hover, doc-gen |

The on-disk field names must use neutral terminology (e.g., `mayBlock`, NOT `inferredMayBlock` — the banned-jargon list prohibits `infer*` in user-facing tooling output, and binary metadata field names can leak into debug output).

---

## Why this is bake-in-now

The binary format is one of those rare design surfaces where retrofitting is catastrophically expensive:

1. **Already-shipped packages** would need to be rebuilt. If Yinz packages ship to a registry (eventually), every package author would have to rebuild. Multiply by the package ecosystem size.
2. **Versioning** would have to handle two binary formats. Old packages without metadata can't participate in cross-package may-block analysis; new packages can. Mixed projects become a footgun.
3. **Backward-compat hacks** would multiply — "if metadata is missing, assume X" — and each assumption becomes a silent correctness risk.

Compare: reserving space NOW costs ~zero (the format is being designed once anyway). Adding fields LATER costs the entire migration story above.

Patrick's instruction during the design-lockdown conversation: "bake this in everywhere it needs to RIGHT NOW." This file documents what "everywhere it needs to" means for the binary format.

---

## v0.1 obligations (Phase 5 forward-compat constraint)

The v0-1-compiler.md umbrella plan's Forward-Compatibility Constraints section (added in Phase 5 of the design-lockdown plan) must include:

> The compiled-package binary format MUST reserve space for: may-block metadata per function, ownership signatures per item, kernel-mode compatibility flags, allocator requirements, LLVM attribute hints, self-referential markers, diagnostic provenance, doc comments. v0.1 may leave these fields empty/default; v0.2+ populates them. The format itself must be versioned so additions don't break old consumers.

If the v0.1 implementation ships a binary format that DOESN'T reserve these fields, v0.2 has to break the format. Don't.

---

## Format design constraints

- **Versioned**: every binary package has a format-version field at a known offset. New fields can be added in newer versions; old consumers see the old version and skip the new fields.
- **Tagged sections**: metadata is stored in named sections (similar to LLVM bitcode or ELF sections), not at fixed offsets. New section kinds can be added without disturbing existing layouts.
- **Self-describing**: metadata sections reference each other by ID, not by file offset. Reordering sections doesn't break references.
- **Size-prefixed**: every section has a size header so consumers can skip unknown sections without parsing them.

The exact byte layout is a v0.2 implementation detail — this doc establishes the requirements, not the file format spec. v0.2 milestone plan picks the actual format.

---

## What this is NOT

- **Not the source-level package model** — that's in [`docs/reference/REF-packages.md`](../../reference/REF-packages.md) and [`docs/internal/implementation/IMP-packages.md`](../implementation/IMP-packages.md). Those documents are about `ynz add`, `ynz_modules`, `yinz.toml`, etc.
- **Not the package registry** — registry hosting/auth/publishing is a separate v0.2+ design (no doc yet; will be in `docs/internal/implementation/IMP-registry.md` when it's time).
- **Not an ABI specification** — Yinz packages are compiled per-target-triple. ABI stability across versions isn't promised (per [`docs/internal/decisions/ADR-versioning.md`](../decisions/ADR-versioning.md)'s pre-release delete policy). This is the BINARY metadata format, not a stable wire protocol.

---

## v0.2 Implementation notes

When v0.2 ships compiled-package emission:

- Define the exact binary format (LLVM bitcode wrapper? Custom format? See what `cargo` does for `.rlib` and learn from it without copying.)
- Populate every field with real data from the compiler's analysis
- Add the cross-package may-block, ownership, kernel-compat propagation in the compiler
- Test cross-package consumption end-to-end

The v0.2 milestone plan must include these. The format-design decision itself can be made later (during v0.2 planning); the RESERVATION (don't ship an incomplete format) is v0.1.

---

## Cross-references

- [`docs/internal/implementation/IMP-packages.md`](../implementation/IMP-packages.md) (source-level package model — `ynz add`, `ynz_modules`, etc.)
- [`docs/internal/implementation/IMP-no-function-coloring.md`](../implementation/IMP-no-function-coloring.md) (consumer of may-block metadata for cross-package analysis)
- [`docs/internal/implementation/IMP-no-runtime-mode.md`](../implementation/IMP-no-runtime-mode.md) (consumer of kernel-mode compatibility flags)
- [`docs/internal/scratchpad/SCRATCH-future-self-references.md`](SCRATCH-future-self-references.md) (consumer of self-referential markers)
- [`docs/internal/implementation/IMP-ownership.md`](../implementation/IMP-ownership.md) (consumer of ownership signatures for cross-package call-site analysis)
- [`.claude/planning/done/2026-05-12-v0-1-compiler/roadmap.md`](../../../.claude/planning/done/2026-05-12-v0-1-compiler/roadmap.md) "Forward-Compatibility Constraints" (Phase 5 locks the v0.1 obligation)
