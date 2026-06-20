# v0-3-m3a-suspension-codegen Phase 3 Deviations — ROUND 6 (captured 2026-06-04)

D_count: 2 approach (+ established scope touches: registry/features.toml, design/concurrency.md, ynz.tmLanguage.json, integration.rs)

> Round 6 = Patrick-directed INTERIM loud-reject guard (`ArrayShapeRuntimeFieldWithWait`) for the round-5 8th head (array<Shape> with runtime field values crossing a wait → silent stack-garbage). Long-term fix = by-value element storage, tracked as milestone m3c-array-by-value (design/future/array-by-value-element-storage.md + todos.md). North star honored: silent → loud.

## Round-6 (coordinator-verified live)
- Runtime-field array<Shape> crossing EXPLICIT wait → exit 1, `ArrayShapeRuntimeFieldWithWait` clean diagnostic (WHAT + concrete rewrite WHAT-INSTEAD + full root-cause WHY + design ref). Deterministic (twice).
- Runtime-field array<Shape> crossing INFERRED suspension (bare `pause()` call, no explicit wait) → ALSO loud-rejects (crossing analysis recognizes the call as a suspension point — the guard is NOT explicit-wait-limited; the executor's feared edge did not materialize).
- Literal-field array<Shape> crossing wait → STILL WORKS (30) — round-5 win preserved, no over-reject.
- Runtime-field array<Shape> NOT crossing a wait → WORKS (30) — no over-reject.
- registry [[deferred_language_feature]] array-shape-runtime-field-with-wait + design/concurrency.md note + 2 fixtures + tmLanguage regen. jargon fixed (struct/implementation banned words). 216 integ + 31 SM tests 0 failures, clippy/fmt/jargon clean.

## Approach Deviations (verbatim from round-6 executor report)

- **Deviation #1** (guard predicate — `is_let_declared_before_wait_in_stmts` pre-check): task said "detect array<Shape> with runtime field values that CROSSES a wait." The crossing analysis (`locals_crossing_wait`) is intentionally conservative and can mark a `let` declared AFTER the first wait as a crossing candidate for a later suspension → would over-reject arrays constructed after the wait. Added `is_let_declared_before_wait_in_stmts` walking top-level stmts so the guard fires only if the array `let` appears before a suspension. Rationale: tighter scoping avoids false positives; the crossing analysis alone is too broad for this guard. Diff hunks: `crates/ynz-typeck/src/check.rs:6577-6612`. **[COORDINATOR-VERIFIED: no over-reject (array-after-wait works) AND still catches inferred-suspension crossings.]**

- **Deviation #2** (literal predicate — IntLit|BoolLit only): task said "not a compile-time IntLit/BoolLit/FloatLit/etc." Yinz AST has no `FloatLit` (float literals are `NumberLit`). `expr_is_compile_time_literal` uses `IntLit | BoolLit` only — the exact set `try_build_shape_global` can fold to a stable global. `NumberLit`/`StringLit`/`NoneLit` fields also cause `try_build_shape_global` to return None (stack alloca → unsafe), so the guard correctly fires for them. Rationale: mirrors the codegen fold-ability exactly. Diff hunks: `crates/ynz-typeck/src/check.rs:6614-6626`.

## Resolved spawn list

### Deviation #1 (approach) — guard pre-check scoping [JUDGE D1 — over/under-reject boundary]
- type: approach | hunks: crates/ynz-typeck/src/check.rs:6577-6612 | hash: 3b8a5c30e6baac4b4e0e2a470c2aab8d0b5333ad

### Deviation #2 (approach) — literal predicate [JUDGE D2 — does it fire on NumberLit/StringLit fields?]
- type: approach | hunks: crates/ynz-typeck/src/check.rs:6614-6626 | hash: 0745fc54be56d9ec054c7c56f9f6527c57cb19dd
