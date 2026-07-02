---
name: "SCRATCH-future-array-by-value-element-storage"
description: "Future-milestone design notes for storing array<Shape> elements by value inline in the heap buffer, the long-term fix for a stack-dangling miscompile the interim M3a guard only masks."
tags:
  - "yinz-compiler"
created_at: "2026-06-04"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Array By-Value Element Storage (m3c-array-by-value) — Future Milestone

**Status**: PLANNED (committed long-term fix, Patrick-approved 2026-06-04). Tracked here as the design source for a dedicated `/plan` (`v0-3-m3c-array-by-value`).

**Why this exists**: v0.3-M3a Phase 3 surfaced a silent miscompile — `array<Shape>` with **runtime** field values, used as a crossing local / loop var across a `wait`, prints stack garbage. The interim fix (M3a) is a **loud-reject guard** (`ArrayShapeRuntimeFieldWithWait`) that turns the silent miscompile into a clean compile error. The interim guard is **conservative**: it fires on the full crossing-names set, which includes some after-last-wait constructions that the crossing analysis conservatively tracks as in-scope references (e.g. an array declared after a `wait` but used as a for-loop iterator). These cases are safe from the stack-dangling bug but are rejected anyway — loud over silent — because distinguishing them precisely would require a more complex analysis than the interim warrants. This doc captures the **long-term right answer** (by-value element storage) that LIFTS the guard entirely. Patrick: "We need it fixed long term."

---

## The Bug (root cause)

`YnzArray` (`crates/ynz-runtime/src/lib.rs`) is a flat byte buffer with a **uniform 8-byte slot** per element. Every element is stored as one `i64` via `Cg::to_i64_bits`:
- `int` → the i64 value (inline, correct)
- `bool` → i1↔i64; `float` → bitcast (inline, correct)
- `string`/`array`/`map` → `ptr_to_int` of a **heap-stable** pointer (survives suspension — the pointed-to data is heap-owned/global)
- **`Shape` → `ptr_to_int` of a STACK ALLOCA** (the struct bytes live in the constructing function's stack frame)

When an `array<Shape>` crosses a `wait`, the function is a stackless state machine that **frees its stack on suspension** → the element pointers dangle. The array's own heap buffer survives, but the shape bytes it points to do not. A round-5 M3a patch made compile-time-**literal**-field shapes point to LLVM module globals (stable); **runtime**-field shapes still fall back to stack allocas → silent miscompile.

This is **latent even without suspension** — it only works today because non-suspending arrays are constructed + consumed in the same stack frame. The array does not OWN its element values; it points to transient stack data, violating GR3 (ownership) + GR8 (zero-cost).

## The Design-Correct Target — Option A: by-value inline

The array MUST own its element values. Store shape bytes **by value, inline** in the heap buffer (variable slot size = `sizeof(elem)`), exactly like `array<int>` stores i64 values inline. The heap buffer survives suspension → shapes survive, regardless of literal/runtime fields. One allocation for the whole buffer (GR8), no per-element heap (avoids the per-element-alloc leak class that bit the M3a-round-3 decimal128 fix), cache-friendly, mirrors the composed-frame model (a crossing shape's bytes live inline in the frame's slot region).

**Rejected — Option B (heap-owned per element + element-aware drop)**: one `ynz_alloc` per element violates GR8, and requires `ynz_array_drop` to become element-type-aware (it is currently element-blind) — the exact leak hazard the decimal128 round-3 patch hit. Globals work for literals only.

`array<string>` stays an 8-byte pointer slot (strings are already heap-owned; the pointer is stable). Only composites (Shape, and by symmetry `map<K,Shape>` values) need the variable-width treatment.

## Blast Radius (why this is its own 2-3 session plan, not a fix-round)

Breaking ABI change to `YnzArray`:
- Add `elem_size: i64` to the struct; allocate `cap * elem_size`.
- `ynz_array_new(elem_size)`; `ynz_array_push(arr, value: *const u8)` (memcpy `elem_size` bytes); `ynz_array_get(arr, idx, out: *mut u8)` returning a separate has-flag (the `{i64,i64}` maybe-convention can't carry variable-width values); `ynz_array_set(arr, idx, value: *const u8)`. `ynz_array_count`/`ynz_array_drop` unchanged (drop stays element-blind — safe while shapes are transitively trivially copyable; see Risk 3).
- `crates/ynz-codegen/src/runtime_decls.rs` — LLVM decls for all four.
- `crates/ynz-codegen/src/emit.rs` — ~20+ call sites: ArrayLit lowering, for-loop SM + non-SM paths, `lower_array_method` (`get`/`first`/`last`/`contains`/`add`/`set`), `IndexAccess` bracket sugar. Compute `elem_size` per construction (`TargetData::get_abi_size`). **Delete** `try_build_shape_global` + the SM for-loop shape-embed special-case (both become unnecessary — the array buffer is now the stable owner).
- `crates/ynz-runtime/src/lib.rs` `ynz_string_split` (pushes string ptrs).
- All `insta` IR/golden snapshots showing `@ynz_array_*` signatures + object-file SHAs.

This is a **general** array-codegen change (not scopeable to suspension without two parallel ABIs = duct tape). It affects non-suspension `array<Shape>` too (correctly — the storage was always wrong).

## Risks (from the architect analysis)
1. Broad snapshot churn (mechanical).
2. `elem_size`-per-type: miss a construction site → silent miscompile (same whack-a-mole as M3a-P1/P3; mitigate with an exhaustive call-site audit BEFORE coding).
3. **Element-aware drop**: today shapes are trivially copyable (primitive fields). When a shape gains a heap-owned field (`string`/`array`/`map`), an inline-stored `array<Shape>` would leak on drop unless `ynz_array_drop` becomes element-aware. Either implement element-aware drop OR loud-reject `array<Shape-with-heap-fields>` at typeck until then.
4. `map<K,Shape>` is the symmetric problem (already a pre-existing base bug — composite-of-shape map values miscompile with OR without suspension). Fix it in the same plan.
5. `ynz_array_get` maybe-convention change is the highest-risk surface (10+ read sites; each must handle scalar 8-byte AND shape N-byte).
6. The M3a P3 for-loop shape-embed logic SIMPLIFIES (memcpy source becomes the stable get-result buffer).

## Implementation sketch
See the 12-step sketch in the round-6 risk analysis (chat 2026-06-04). Core: add `elem_size` to YnzArray; mutate the four C-ABI fns to byte-pointer/out-buffer convention; update runtime_decls; update every emit.rs call site to pass `elem_size` (via `get_abi_size`) + use the pointer convention; delete `try_build_shape_global` + the SM shape-embed special-case; fix `map<K,Shape>` symmetrically; refresh snapshots; add a runtime-field `array<Shape>` crossing-wait fixture asserting correct output (lifts the M3a `ArrayShapeRuntimeFieldWithWait` guard).

## Scope verdict
2-3 sessions minimum. Its own `/plan` (`v0-3-m3c-array-by-value`). On completion, **remove** the M3a `ArrayShapeRuntimeFieldWithWait` loud-reject guard + its registry deferral (the feature now works).

## Cross-references
- [`.claude/planning/done/2026-06-01-v0-3-m3a-suspension-codegen/plan.md`](../../../.claude/planning/done/2026-06-01-v0-3-m3a-suspension-codegen/plan.md) (where the interim loud-reject ships)
- [`registry/features.toml`](../../../registry/features.toml) `array-shape-runtime-field-with-wait` (the interim deferral this milestone lifts)
- [`docs/internal/implementation/IMP-concurrency.md`](../implementation/IMP-concurrency.md) M3a Scope Boundaries (the interim deferral note)
- [`docs/internal/implementation/IMP-collections.md`](../implementation/IMP-collections.md) (array semantics), [`docs/internal/implementation/IMP-ownership.md`](../implementation/IMP-ownership.md) (GR3 value semantics)
