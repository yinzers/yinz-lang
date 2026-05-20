---
slug: m5-generics
owner: patrick
status: shipped
files:
  - Cargo.toml
  - crates/ynz-ast/**
  - crates/ynz-parser/**
  - crates/ynz-typeck/**
  - crates/ynz-codegen/**
  - crates/ynz-runtime/**
  - crates/ynz-diagnostics/**
  - crates/ynz-driver/**
  - tests/**
  - examples/pirates-roster/entrypoint.ynz
  - examples/primantis-orders/m5_errors.ynz
  - design/generics.md
  - design/maybe.md
  - design/collections.md
  - spec/generics.md
  - spec/maybe.md
  - spec/collections.md
  - .claude/plans/active/v0-1-compiler.md
created: 2026-05-17
last_updated: 2026-05-17-r4
depends_on: v0-1-compiler
---

# Plan: M5 — Generics + Collections + Maybe

Created: 2026-05-17
Status: approved (Patrick OK 2026-05-17) — Phase 0 SHIPPED (commit `524ca2e`, branch `chore/m5-doc-lockdown` pushed, awaiting merge to main)

## Phase Execution Status

| Phase | Status | Commit / Branch | Notes |
|---|---|---|---|
| P0 — Doc lockdown | SHIPPED on main | `524ca2e` + `b2c528e` + `cf53ad5` merged 2026-05-17 | master plan M5/M6/M7 paragraphs updated to `<>`; `design/maybe.md` created with locked tables; `spec/maybe.md` syntax-updated; `design/generics.md` cross-ref; M5 plan landed |
| P1 — Lexer + AST scaffolding | SHIPPED on main | `49940c9` + `3c18a62` merged 2026-05-17 | Tok::None + lexer keyword `none`; 3 Type variants (TypeParam, Generic, Maybe); 2 Expr variants (NoneLit, IndexAccess); 1 Stmt variant (IndexAssign); GenericParam struct; FunctionDecl/ShapeDecl `generics` field; CallExpr.type_args field; 4 m5_*_variant_count_locked tests + m5_none_keyword_lexes; stale M3/M4 doc comments collateral-fixed; typeck + codegen + return_paths stub match arms; 4 snapshot files additively updated. 401 tests green. |
| P2 — Parser | SHIPPED on main | `faaf13e`, branch `feat/m5-parser`, PR #18 | parse_generic_params (decl `<T>`, `<T follows C>`, `<T,U>`); parse_type_with_depth (maybe<T>, Generic, depth cap 16); Token::None → NoneLit; `[` postfix → IndexAccess; IndexAssign; try_parse_type_args (speculative `<`, 32-token budget, backtrack); 34 new parse tests; 438 tests green. |
| P3a — Typeck generics engine | SHIPPED on main | `a4bffb1`, branch `feat/m5-typeck-generics`, PR #19 | generics.rs (Substitution, unify_param, apply_substitution, MonomorphizationTable, GenericFnTable, GenericShapeTable); Type::TypeParam + Type::Generic; collect_generic_shapes + collect_generic_signatures; check_generic_function_body + check_generic_fn_call (inference + constraint + ownership + mono recording); field access through Type::Generic. 27 new tests. 465 tests green. |
| P3b — Typeck array/fixed/maybe | SHIPPED on main | `2888f9c` PR #20 | BuiltinArray/BuiltinFixed/Maybe types; ArrayLit; bracket desugar; flow-sensitive .value; 528 tests. |
| P3c — Typeck map | SHIPPED on main | `65a4e7d` PR #21 | BuiltinMap/MapEntry; MapLit; bracket sugar; for-loop MapEntry; 569 tests. |
| P4a — Codegen mono + array/fixed/maybe | SHIPPED on main | PR #22 | Monomorphization; ynz_array_*; maybe {i64,i64}; ArrayLit/IndexAccess/IndexAssign lowering; 4 fixtures. |
| P4b — Codegen map | SHIPPED on main | PR #23 | SipHash-2-4; Swiss Tables; MapLit/bracket/iter; m5_map.ynz. |
| P5 — Driver + fixtures + examples | SHIPPED on main | PR #24 | 5 integration tests; basics M4+M5 demo; m5_errors gallery; mono dispatch fix. |
| P6 — Verification + tag `v0.1.0-m5` | SHIPPED | `v0.1.0-m5` | 574 tests; all fixtures pass; Cargo.toml bumped. |

## Context & Why

**Goal.** Ship M5 of the Yinz v0.1 compiler — the milestone that introduces type parameters (the universal machinery that makes the rest of the language work). When M5 ships, users can write generic functions and shapes, instantiate built-in `array<T>` / `fixed<T>` / `map<K, V>` collections, bracket-index them safely (`arr[0]`, `m["key"]`) returning `maybe<T>`, iterate them with `for (x in collection)`, and lean on the `maybe<T>` primitive throughout. The milestone closes the M4-locked auto-promotion obligation (`array<T>` → `fixed<T>` codegen surface) and unblocks all downstream collections-using milestones (M6 options/unions, M7 strings/errors/iterables, every stdlib module from v0.5 onward).

**Why now.** M4 shipped (`tag v0.1.0-m4`, 316 tests, `shapes + UFCS + ownership`). Without M5, every milestone after M4 is paralyzed — M6 narrowing on union types needs the generics engine to express `maybe T`, M7 strings need `array<byte>` semantics for byte slicing, every stdlib module needs `array<T>`/`map<K,V>` to express its surface. Continuing without M5 means stdlib design drift: design docs reference `array<T>` semantics that the compiler can't yet validate.

**Background.** M1–M4 built the type system without parameterization — every type is concrete. M5 adds the type-parameter dimension. The work touches every crate: lexer (no new tokens but `<` / `>` get new contextual roles in type position), parser (type-param syntax in decls, generic instantiations in type position, bracket-index in expression position, contextual disambiguation of `<` at call sites), typeck (the generics engine: type variables, substitution, monomorph queue, constraint checking against `follows`), codegen (a specialized LLVM function/type per concrete instantiation), runtime (array/fixed allocation/growth/drop; map runtime with Swiss Tables + SipHash). The full v0.1 design lives across `design/generics.md` and `design/collections.md`.

**Constraints.**
- Rust stable toolchain. LLVM 18 via inkwell. Salsa from day 1.
- All M5 diagnostics follow WHAT/WHAT-INSTEAD/WHY three-part format.
- Banned-jargon list (`design/compiler-errors.md` + `crates/ynz-diagnostics/src/banned_jargon.rs`) — in particular `monomorphize`, `polymorphic`, `covariant`, `contravariant` MUST NOT appear in user-facing diagnostics.
- Generic syntax is `<>` (NOT `[]`) per `design/generics.md` locked design. The master plan v0-1-compiler.md uses `[T]` in M5's one-paragraph entry — that paragraph is OUT OF DATE; P0 below updates it.
- The `maybe T` primitive moves from M6 to M5 (locked in this plan's first-question answer). M6 still ships options/unions/`if (x is Type)` narrowing — but `maybe T` lands here so `.get()` on collections returns `maybe T` from day 1.
- Map default tier is **Swiss Tables + SipHash-2-4** with **perfect-hash codegen for all-static-key literals**. The xxhash3 fast opt-in and identity-hash for int keys are deferred to a later milestone (surface syntax for the fast opt-in remains unlocked, per `design/collections.md` "Surface syntax is deliberately NOT locked here").
- `for (x in collection)` for built-in `array<T>` / `fixed<T>` / `map<K,V>` is a typeck + codegen special-case in M5, marked with `REPLACE-AT M7` comments. M7's `Iterable<T>` protocol replaces the special-case. Mirrors the M3 range() pattern.
- `array<T>` → `fixed<T>` auto-promotion ships its **codegen surface** in M5 (silent perf win). The **Tier 3 lint surface** (`prefer-fixed-when-immutable`) waits for v0.4 (lint tier). The **muted IDE hint** waits for v0.2 (LSP). The split is per `.claude/rules/auto-promotion.md` "Two Surfaces for the Same Decision."

**Success criteria for M5 (this milestone's contract):**

A runnable program covering every M5 feature:

```ynz
shape Pair<A, B> { first: A, second: B }

function identity<T>(give value: T) -> T { return value }

function findMax<T follows Comparable>(share items: array<T>) -> maybe<T> {
  if (items.count() == 0) { return none }
  let best: T = items[0].value
  for (item in items) {
    if (item.compare(best) > 0) { best = item }
  }
  return best
}

function entrypoint() -> nothing {
  let nums: array<int> = [3, 1, 4, 1, 5, 9, 2, 6]
  let fixedColors: fixed<string> = ["red", "green", "blue"]
  let scores: map<string, int> = { "alice": 90, "bob": 85 }

  let n = identity(42)                        // T inferred as int
  let coord = identity<Pair<int, int>>({ first: 10, second: 20 })

  let max = findMax(nums)
  if (max.exists()) { print(max.value) }      // 9

  print(scores["alice"].or(0))                // 90
  print(scores["missing"].or(0))              // 0
  print(fixedColors[1].or("none"))            // green

  for (color in fixedColors) { print(color) } // red / green / blue
  for (entry in scores) { print(entry.key) }  // alice / bob (insertion order)
}
```

Compiles, runs, exits 0, produces the expected stdout. Plus: every M4 invariant carries forward (ownership analysis, `const` deep immutability, LLVM `readonly`/`noalias` attributes, dispatch contracts), with monomorphization preserving them across instantiations.

---

## FINAL LOCKED DECISIONS (this plan's source of truth)

These were resolved during plan-drafting (2026-05-17) and override anything inconsistent elsewhere. Numbered to support `r1`/`r2` revision tracking if reviewer rounds change anything.

### Scope decisions (locked r1, 2026-05-17)

- **`maybe<T>` ships in M5, not M6.** Implemented as a built-in generic primitive on top of the generics engine. `none` literal, `.exists() -> bool`, `.value -> T` (with compile-time enforcement: `.value` requires flow-sensitive proof via `.exists()` check; otherwise compile error suggesting `.or(default)`). M6 still ships `options` declarations, `A | B` union types, and `if (x is Type)` narrowing — but `maybe<T>` moves to M5.
- **Map = Swiss Tables + SipHash-2-4 default + perfect-hash for static-key literals.** Defer xxhash3 fast opt-in (and its surface syntax) and identity-hash for int keys to a future milestone (no specific milestone bound — picks up when there's a workload demanding it).
- **`for (x in collection)` is special-cased in typeck + codegen for built-in `array<T>` / `fixed<T>` / `map<K,V>`.** Each special-case site carries a `REPLACE-AT M7` comment. M7's `Iterable<T>` protocol replaces the special-case.
- **Auto-promotion `array<T>` → `fixed<T>`: codegen surface only in M5.** The Tier 3 lint (`prefer-fixed-when-immutable`) defers to v0.4. The muted IDE hint defers to v0.2.

### `none` type inference rules (locked r1)

The `none` literal produces a type `maybe<T>` for some unknown T. T is resolved from CONTEXT. Locked context-resolution rules:

| Context | T resolution |
|---|---|
| `let x: maybe<U> = none` | T = U (from binding annotation) |
| `function foo() -> maybe<U> { return none }` | T = U (from return type) |
| `arr.add(none)` where `arr: array<maybe<U>>` | T = U (from parameter type at receiver method dispatch) |
| `if (cond) { value_of_maybe_U } else { none }` | T = U (from sibling branch type) — typeck infers from the non-none branch |
| `let m: map<string, maybe<U>> = { "a": some(5), "b": none }` | T = U (from map value-type annotation; per-entry context flows from the annotation) |
| `let x = none` (no annotation, no enclosing call/return) | **Compile error** — "Cannot work out which type `none` should be here. Annotate the binding: `let x: maybe<int> = none`". |
| `identity(none)` (single-arg generic call, no other parameters constrain T) | **Compile error** — "Cannot work out the type parameter T from a `none` argument alone. Either pass a concrete value OR annotate the result: `let x: maybe<int> = identity(none)`". |
| `pair(none, 5)` (multi-arg generic call, one arg constrains T) | T = int for first param (`none` constrained to `maybe<int>` if the parameter declares `give value: maybe<A>`; otherwise compile error). Each `none` is resolved independently against its parameter slot. |
| `foo(none)` where `foo(give x: maybe<U>) -> ...` (non-generic function with maybe parameter) | T = U (from parameter type) |

**The general rule:** `none` resolves T by walking up the AST one node at a time, looking for a context that types it as `maybe<U>` for a concrete U. The walk terminates at: (a) the immediate type annotation, (b) the enclosing call's parameter type, (c) the enclosing return type, (d) a sibling branch (if/else) with a `maybe<U>` type. If the walk exhausts these without finding a U, compile error with the suggested-annotation diagnostic.

### Flow-sensitive `.value` enforcement rules (locked r1)

`.value` on a `maybe<T>` binding requires compile-time proof that the value is not none. M5 ships a NARROW subset of flow narrowing (full narrowing including negation/short-circuit/early-return is M6's `if (x is Type)` work). Locked rules for M5:

| Form | `.value` access permitted? | Notes |
|---|---|---|
| `if (m.exists()) { m.value }` | YES — flag set on `m` inside the then-block | The bread-and-butter case. |
| `if (!m.exists()) { ... } else { m.value }` | YES — flag set on `m` inside the else-block | Symmetric to positive form; spec'd in M5. |
| `if (m.exists() && other) { m.value }` | YES — short-circuit AND propagates the flag | The right-hand side and the body both see `m` narrowed. |
| `if (other && m.exists()) { m.value }` | YES — same | Order doesn't matter for AND. |
| `if (m.exists() || other) { m.value }` | NO — OR doesn't guarantee `m` narrows | `.value` here produces a compile error pointing at the OR. Suggest `.or(default)`. |
| `if (a.exists() && b.exists()) { b.value }` | YES — flag set independently on `a` and `b` | Independent maybes each get their own flag. |
| `if (m.exists()) { return ... } m.value` | NO — early-return narrowing is M6 work | M5 produces a compile error pointing to M6's narrowing for the early-return form. Workaround: restructure as `if (!m.exists()) { return ... } m.value` is ALSO M6 work. Workaround that works in M5: `if (m.exists()) { use_value(m.value) } else { return ... }`. |
| `for (i in range(3)) { if (m.exists()) { m.value } }` | YES — flag scoped to the if-block; loop doesn't change `m` | The flag is per-binding, per-block; reassignment to `m` inside the if-block invalidates the flag for further uses within that block. |
| `if (m.exists()) { m = newMaybe(); m.value }` | NO — reassignment to `m` invalidates the flag | The flag is on the EXACT binding-state, not the name. After reassignment, narrowing must be re-proved. Compile error with the offending reassign site cited. |
| `if (m.exists()) { closureCapture(() => m.value) }` | NO — closures don't carry the narrowing flag (closures are v0.3+ feature anyway, so this rule is forward-defensive) | M5 doesn't ship closures; rule is documented for the future. |
| `m.value` with no surrounding `.exists()` check | NO — compile error | The base case. Suggest both `.exists()` check and `.or(default)`. |

**Why only this subset:** the simple positive/negative/AND cases cover the 95% of real usage. The early-return form requires negation-narrowing infrastructure that M6 owns. Reassignment invalidation is mechanical (track the binding-state, not just the name). Closures don't ship until v0.3+.

### Syntax decisions (locked r1)

- **Type-parameter syntax = `<T>` everywhere.** `function foo<T>(...)`, `shape Pair<A, B> { ... }`, `array<int>`, `Pair<int, int>`, `maybe<Player>`. NEVER `[T]`. The master plan v0-1-compiler.md M5 paragraph uses `[T]` — P0 below corrects it.
- **Constraints are inline:** `function sort<T follows Comparable>(...)` — NOT separate `where` clauses. Locked in `design/generics.md`.
- **Generic call-site disambiguation:** TypeScript-style. At call sites, the parser sees `foo<T>(args)` and tries to parse `<T>` as a type-parameter list FIRST; if that fails, it backtracks to comparison. The disambiguation rule: a `<` after an identifier at expression-call position, where the contents up to `>` parse as a comma-separated TYPE list and `>` is immediately followed by `(`, is a generic call. Otherwise it's the `<` comparison operator. No turbofish (`foo::<T>(x)`) syntax.
- **Bracket-index expression = parser-level new `Expr::IndexAccess { receiver, index, span }`.** Desugar to `.get(index)` in typeck. Index-assign `arr[i] = v` desugars to `arr.set(i, v)`.
- **`none` is a reserved keyword.** Lexer adds `Tok::None`. Parser produces `Expr::NoneLit { span }`. Typeck assigns it type `maybe<T>` where T is inferred from context.
- **`array` / `fixed` / `map` / `maybe` are NOT keywords.** They're built-in type names resolved at typeck time. The lexer treats them as identifiers; typeck's type-name resolution recognizes them.

### Compiler architecture decisions (locked r1)

- **Monomorphization happens at codegen time, driven by a salsa query.** Typeck produces a `MonomorphizationTable` mapping `(generic_decl_id, instantiation_types)` pairs to specialized signatures. Codegen consumes the table and emits a specialized LLVM function/type per pair. No runtime dispatch unless the user opts in via `dynamic Foo` (M4 feature, no M5 change).
- **`maybe<T>` LLVM lowering — decision table (locked):** the lowering is chosen per concrete T at monomorph time. The decision is mechanical from T's storage class; no heuristics, no per-binding override:

  | Concrete T | Encoding | LLVM type | Why |
  |---|---|---|---|
  | `int`, `float`, `number`, `bool` (primitives) | Tagged union | `struct { i1 has_value, T value }` | Fits in 2 words; tag is cheaper than reserving a sentinel value of T |
  | Heap-allocated shape pointer (M4 `ynz_alloc`-backed shape) | Null-pointer | `T*` (where null = none) | One word; no tag byte needed; matches Rust's `Option<Box<T>>` optimization |
  | `fixed<U, N>` (stack-allocated array) | Tagged union | `struct { i1 has_value, [N x U] value }` | The whole array is inline; null-pointer encoding doesn't apply (no pointer) |
  | `array<U>` (heap array header) | Null-pointer | `*Array<U>` (where null = none) | The header is heap-allocated; pointer = none/some discriminator |
  | `map<K, V>` (heap map header) | Null-pointer | `*Map<K, V>` | Same as `array<U>` |
  | `Pair<A, B>` and other generic shapes with no pointer field | Tagged union | `struct { i1 has_value, Pair<A, B> value }` | No pointer to repurpose as discriminator; explicit tag required |
  | `dynamic Foo` (fat pointer, two words: data + vtable) | Null-pointer on the DATA slot | `struct { *T data, *Vtable vt }` (where data == null → none) | data-pointer is unambiguously the "object" reference; null in that slot is the natural sentinel; vtable slot is ignored when none |
  | `string` (heap-allocated UTF-8 bytes) | Null-pointer | `*String` (where null = none) | Same as heap shape pointer |
  | `maybe<maybe<T>>` (nested) | **REJECTED at typeck — compile error** | — | Nested maybe is almost always a code smell (caller likely meant to flatten); the rare legitimate case is rejected for v0.1 with a three-part error pointing to `maybe<T>` |
  | All other generic-shape instantiations | Tagged union | `struct { i1 has_value, ShapeT value }` | Default — safe but slightly more memory than null-pointer encoding when applicable |

  The table is consulted for every load/store of `maybe<T>` at the LLVM level. IR-snapshot tests in P4a assert each row of the table produces the expected LLVM type. **Why no per-binding override:** the encoding is implementation-detail; users see only `maybe<T>` and `none`. Adding a `dense<T>` / `pointer-niche<T>` opt-in surface is duct tape; the compiler picks the right one automatically. **Why reject `maybe<maybe<T>>`:** allowing it forces a 2-bit tag (some-some / some-none / none) and that distinction is almost never what the user meant. M5 ships the compile error; if a real use case emerges, design/maybe.md gets a section in v0.2+.
- **`array<T>` runtime:** heap-allocated header `{ i64 len, i64 cap, T* data }` allocated via `ynz_alloc`; growth at 1.5×; bounds check on `.set(i, v)` emits `ynz_panic` with descriptive message; drop emits per-element drop loop + `ynz_free`.
- **`fixed<T>` runtime:** stack-allocated `[N x T]` via alloca; size known at compile time; bounds check on `.set(i, v)` for non-literal index emits `ynz_panic`; literal-index out-of-bounds = compile error.
- **`map<K, V>` runtime:** Swiss Tables (open-addressing + SIMD metadata scan). SipHash-2-4 with per-process random key (initialized at program startup from OS entropy via a `ynz_siphash_init` runtime hook). Insertion-order tracked via parallel index array. All map operations go through a `ynz_map_*` runtime symbol set.
- **Perfect-hash codegen:** when the compiler sees a map literal `{ "alice": 1, "bob": 2 }` with all-static-string keys, it emits a compile-time-generated perfect-hash function (no probing, no SipHash) into the binary. The user sees no syntactic difference; the IDE muted hint shows it (v0.2 work; M5 emits codegen unconditionally).
- **Auto-promotion `array<T>` → `fixed<T>`:** typeck salsa query analyzes each `let name: array<T> = literal` binding for the never-grown property (no `.add()` / `.remove()` / `.resize()` calls; no `lend`-parameter passes that might grow). If proven, typeck records the promotion in a `PromotionReport`; codegen consumes the report and emits `fixed<T, N>` codegen instead. Source is untouched.
- **Generic dispatch is monomorphized:** each `array<int>` and `array<string>` is a fully distinct LLVM type and produces a fully distinct method-table per concrete element type. Zero virtual dispatch cost unless user wrote `dynamic Foo`.

### Catch-up & Future work

- **`prefer-fixed-when-immutable` Tier 3 lint** → v0.4 (lint tier). Codegen lands now; lint surfaces when lint tier exists.
- **xxhash3 fast opt-in + surface syntax** → future milestone, no specific bound. The fast opt-in unlocks when a real workload needs it.
- **Identity hash for `map<int, V>`** → future milestone. Same trigger.
- **Map "trusted-keys" syntax** → future milestone, locks alongside xxhash3.
- **`for (x in custom_iterable)` (user-defined `follows Iterable<T>`)** → M7. M5 only handles built-in collections.
- **IDE muted hints for auto-promotion, hash tier, generic instantiation** → v0.2 LSP. M5 produces the data; LSP wraps it.
- **Generic method dispatch with N-deep `follows` constraints** → M5 ships `T follows Comparable` (single constraint OR comma-list of constraints). Higher-kinded types, lifetime params, associated types are explicitly NOT in v0.1 (per `design/generics.md` "What's NOT in v0.1").
- **`.update({...})` map bulk-update syntax** → DEFERRED past M5. The object-literal-as-update-payload syntax requires desugar rules that aren't load-bearing for v0.1. M5 ships `.set(key, value)` one at a time; v0.2+ revisits when there's a real workload that hits the verbosity cost. Removes Required Fix #10's "at-code-time decision" smell.
- **Generic shape cycle creation through `maybe<Self>` field mutation** → permitted at runtime, with documented memory leak. Per `design/ownership.md`, cycle detection is a borrow-checker concern; the v0.1 borrow checker doesn't detect cycles. M5 ships `shape Node<T> { value: T, next: maybe<Node<T>> }` workably, but a user-created cycle (`n1.next = some(n2); n2.next = some(n1)`) leaks both nodes on scope exit. Documented in `design/maybe.md` (P0 creates it); negative fixture `m5_cycle_leak.ynz` includes a comment explaining the leak. This is the v0.1 design choice (not duct tape) — cycle-collection or borrow-checker cycle-detection waits for v0.2+ when the LSP enables interactive cycle visualization. Stated as a Quality Gate "documented leak (intentional v0.1 limitation), not silent."
- **Module-level / top-level `let` bindings of map type** → REJECTED at typeck in M5 with a teaching error pointing to M8 (modules). The `ynz_siphash_init` runtime hook is called from `main`'s prologue ONLY; module-level map literals would need pre-main initialization order which isn't designed in M5. M8's module work decides whether top-level maps get a pre-`main` static init or a lazy init or stay rejected. Negative fixture `m5_top_level_map_rejected.ynz` covers this. (Note: top-level `let` bindings exist as M2 surface — but M2 binds them to compile-time-evaluable values; map literals require runtime allocation + SipHash key, which the current driver doesn't initialize until `main`. Hence the rejection.)
- **Perfect-hash CHM92 retry exhaustion fallback** → silent fallback to Swiss Tables + SipHash codegen with a one-line debug log (gated behind `--debug-codegen` flag for v0.2+; M5 logs to stderr only when `YINZ_DEBUG_PERFECT_HASH=1` env var is set). CHM92 is tried with 16 different random seeds; if all 16 fail to find a perfect hash function for the literal's keys, codegen emits the Swiss Tables path. Observable behavior is identical (correct map operations); only the IR differs. IR-snapshot test `m5_perfect_hash_fallback.ll` documents the fallback path for a known-pathological key set (constructed via published CHM92 worst-case inputs).

---

## Research Findings

- `design/generics.md` locks the full v0.1 generics scope. `<>` syntax everywhere, inline `follows` constraints, call-site type inference, no higher-kinded types, no lifetime params. M5 implements this design verbatim.
- `design/collections.md` locks the full v0.1 collections scope: `fixed<T>` stack, `array<T>` heap with 1.5× growth, `map<K, V>` = Swiss Tables + four-tier hashing (M5 ships 2/4 tiers — see locked decisions), bracket sugar desugaring to `.get()`/`.set()`, insertion-order map iteration, compiler-auto-reorder of shape fields (already shipped M4).
- `spec/maybe.md` documents the user-facing `maybe T` surface — `.exists()`, `.value`, `.or(default)`, `none` literal. Already written; M5 implements this surface.
- `spec/generics.md` documents user-facing generics — `<T>` syntax, type inference, `follows` constraints, multiple type parameters. Already written; M5 implements this surface.
- `spec/collections.md` documents user-facing collections — three collection types, bracket sugar, dot methods, nested-collection idioms. Already written; M5 implements this surface.
- `design/maybe.md` does NOT exist. P0 creates it with the design rationale for moving `maybe T` from M6 → M5 + the LLVM lowering decisions.
- `crates/ynz-ast/src/nodes.rs` current state: `Type` enum has 8 variants (Nothing, Named, Error, Int, Float, Number, Bool, Range, Dynamic, SelfType — wait, that's 10 including comments; check the variant-count test for actual count). The variant-count tests are pinned; adding variants requires `// test-ratchet: <reason>` markers.
- `crates/ynz-diagnostics/src/banned_jargon.rs` already bans `monomorphize` / `monomorphic` / `polymorphic` / `covariant` / `contravariant` / `infer` / `inference`. M5 user-facing diagnostics must work around these. Suggested wording: instead of "monomorphizes for each type", say "specialized for each type used"; instead of "polymorphic", say "works with any type that follows the contract."
- Hand-written parser disambiguation of `<` at call sites: speculative parse with backtracking. Cost is O(n) in the type-list length; bounded by a 32-token lookahead cap to prevent pathological inputs. Same technique TypeScript's parser uses.
- Salsa monomorph queue: a `tracked` query `monomorphization_table(typed_module) -> Arc<MonomorphizationTable>` accumulates every concrete instantiation seen during typeck. Codegen consumes the table; salsa invalidates only the affected instantiations on edit. Avoids re-monomorphizing the world on every keystroke.
- Auto-promotion data-flow analysis: per-function, single-pass scan of the binding's def-use chain. Cost is O(uses); in M5 the analysis runs at typeck time, salsa-cached per function.
- Swiss Tables reference implementations: Google Abseil (C++), Rust hashbrown (port of Abseil), Go 1.24 (Datadog's port). M5 uses Rust crate `hashbrown` for the in-compiler typeck-side dictionary needs (internal compiler infra) and writes a Yinz-runtime Swiss Tables implementation in C (or hand-rolled Rust compiled to native code) for the user-facing `map<K, V>` runtime.
- SipHash-2-4 implementation: well-defined algorithm; can be hand-rolled in ~150 lines of C. Rust `siphasher` crate provides a reference; the C port mirrors it.
- Perfect-hash codegen: uses the CHM92 algorithm (Czech / Havas / Majewski, 1992) or BBHash for compile-time-known small key sets. Implementable as a compile-time Rust pass that generates LLVM IR encoding a hash table lookup.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Monomorphization explosion: code bloat from many concrete instantiations | Medium | Slow compiles, large binaries | Dedupe identical instantiations at codegen time (a `Vec<int>` used in 50 places produces ONE specialized type). Cap monomorph depth at 16 levels (`Pair<Pair<Pair<...>>>`) to prevent infinite recursion via cyclic generics; emit teaching error at depth > 16. |
| Generic call-site disambiguation backtracking is slow on adversarial input | Medium | DoS in IDE compile path | Cap the speculative-parse lookahead at 32 tokens for the `<...>` window. If the window doesn't close in 32 tokens, abandon the speculative parse and fall back to `<` as comparison. Snapshot test with a 1000-token `<` chain confirms no exponential blowup. |
| Swiss Tables runtime correctness (lots of pointer arithmetic, SIMD code paths) | High | Wrong map results in production — silent | M5 P5 ships a property-based fuzz test (`crates/ynz-runtime/tests/map_fuzz.rs`) that compares `ynz_map_*` against a `BTreeMap<K, V>` reference oracle across 10k random operation sequences. Different hash seeds each run. CI fails on any divergence. Plus: deliberate adversarial test (`map_hashdos.rs`) using known-colliding SipHash inputs confirms DoS resistance. |
| SipHash-2-4 initialization: random key must be set before any map operation | Medium | Crash on first map use if init missed | `ynz_siphash_init` is called automatically from the prologue of `main` (compiler emits the call). Driver integration test confirms a program using a map literal at file scope (M8 work) doesn't crash; M5 only needs init-before-main-body. |
| `maybe<T>` lowering: tagged union vs null-pointer encoding inconsistency | High | Codegen produces wrong field offset; map/array reads return garbage | Per-instantiation lowering decision is made at monomorph-time and recorded in the `MonomorphizationTable`. Codegen consults the table for every load/store of a `maybe<T>` field. IR-snapshot tests assert the chosen lowering matches the expected for each T. |
| Bracket sugar `.set()` lowering allows literal-OOB on `fixed<T>` to slip past typeck | Medium | Runtime panic where a compile error should fire | Typeck checks `fixed<T, N>` writes with literal int index against N at typeck time. Negative fixture `m5_fixed_literal_oob.ynz` asserts compile error with the bound + the offending index. Non-literal indices defer to runtime panic. |
| Auto-promotion `array<T>` → `fixed<T>` emits wrong codegen when binding crosses function call | Medium | Silent perf regression OR wrong codegen | Typeck's def-use analysis treats ANY pass to a function parameter declared `lend array<T>` as "could be grown" and refuses promotion. Only `share array<T>` parameters are safe (read-only). Positive + negative fixtures cover both paths. |
| Map perfect-hash codegen for literals with duplicate static keys | Low | Compile-time panic OR wrong hash table | Typeck detects duplicate literal keys at struct-literal time and emits a three-part diagnostic naming both spans. Positive test asserts `{ "a": 1, "a": 2 }` is a compile error. |
| Generic constraint check failure produces banned-jargon ("does not satisfy contract Comparable") | Medium | Jargon audit fails CI; teaching mission violated | Diagnostic wording explicitly says "Type Player does not follow contract Comparable" (matches `design/generics.md` example). Jargon-audit test (workspace-wide grep) catches any "satisfies", "monomorph", "polymorphic" etc. |
| `for (x in collection)` typeck special-case becomes load-bearing, hard to unwind at M7 | Medium | M7 ends up rewriting more than expected | Every special-case site carries a `REPLACE-AT M7` comment AND an entry in the M7 catch-up obligations section of the master plan. M7's plan reviews this list and unwinds in the same PR that introduces `Iterable<T>`. |
| Salsa query invalidation explodes when typeck recomputes the monomorph table | Medium | Sub-second incremental compile target missed | The monomorph table is keyed by `(decl_id, type_args)`; only the affected entries invalidate on edit. Salsa's per-key invalidation handles this. Benchmark before merge: 1000-function module, 1-character edit, sub-second incremental compile confirmed. |
| Map iteration order regression (random instead of insertion) | Low | Test failures across the codebase; spec violation | Iteration goes through the parallel insertion-order index array, NOT the Swiss Table bucket array. Iteration-order property test confirms insert-then-iterate produces the original order. |
| `none` literal type-inference fails in ambiguous contexts (e.g., `let x = none`) | High | User hits opaque "cannot infer type" with no useful suggestion | Three-part diagnostic: WHAT (T cannot be inferred from `none` alone); WHAT-INSTEAD (annotate the binding: `let x: maybe<int> = none`); WHY (none works for any maybe T; without a type elsewhere in the call site, the compiler doesn't know which one). |
| Generic shape default fields with self-reference (e.g., `shape Node<T> { next: maybe<Node<T>> = none }`) | Medium | Infinite type expansion in monomorph | Self-references are detected at monomorph-time (the type appears in its own field). Allowed when the field is `maybe<Self>` or `array<Self>` (indirection breaks recursion); banned for direct self-fields. Negative fixture covers the banned form. |

---

## Questions

(None outstanding — the four strategic scope questions were locked above. Remaining decisions are tactical and can be made during phase execution.)

---

## Risk Assessment & Rollout Strategy

**Risk level: LOW (production) / MEDIUM-HIGH (architectural).**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | — |
| Touches auth/permissions | No | — |
| Raw SQL / literals | No | — |
| Modifies existing data | No | Greenfield additions; no schema changes. M4 invariants carry forward. |
| Third-party integration | Yes | inkwell (LLVM), salsa, ariadne, optional `hashbrown` (internal use), system linker. No new system-level deps in produced binaries beyond libc (and the `ynz_*` runtime library M4 already established). |
| Changes existing endpoints | No | — |
| Wrong foundational choice cascades | Yes | The generics engine is load-bearing for every milestone v0.5+. Wrong-shape decisions made here propagate into the stdlib design. Monomorphization vs runtime dispatch is the biggest "cannot undo" choice — locked to monomorph (per `design/generics.md`). |

**Mitigations applied:**
- Plan-reviewer pass before any phase begins (Step 7 of /plan).
- Property-based fuzz tests on the map runtime (`map_fuzz.rs` + `map_hashdos.rs`).
- IR-snapshot tests on every codegen surface; `// test-ratchet:` required to weaken them.
- Auto-promotion is codegen-only in M5 — defers source-level user-facing churn to v0.4 (lint tier) and v0.2 (LSP).
- Incremental-compile benchmark gate before tag.
- M7 catch-up obligations list maintained — the `for (x in collection)` special-case has a documented removal trigger.

**Rollout plan:** N/A. The compiler is a development tool with no users yet. Tag `v0.1.0-m5` after P7 verification.

---

## Roadmap (milestones — see `v0-1-compiler.md` for the full index)

(Only the M5 entry is repeated here for context; full roadmap lives in the master plan.)

### Milestone 5 (M5): Generics + Collections + Maybe — multi-session
- **Type parameters everywhere:** `function foo<T>(...)`, `shape Box<T> { ... }`, `array<int>`, `Pair<int, int>`, `maybe<Player>`. Syntax = `<>`, NEVER `[]`.
- **Three built-in collection types:** `array<T>` (heap, growable, 1.5× growth), `fixed<T>` (stack, size-locked), `map<K, V>` (Swiss Tables + SipHash + perfect-hash for static-key literals).
- **`maybe<T>` primitive** (moved from M6): `none` literal, `.exists()`, `.value`, `.or(default)`. Flow-sensitive `.value` enforcement.
- **Bracket sugar:** `arr[i]` → `.get(i)`, `arr[i] = v` → `.set(i, v)`. Returns `maybe<T>` for reads. Works on `array`, `fixed`, `map`, `string` (string indexing produces single-codepoint strings — already partially in M2 string spec).
- **Built-in iteration:** `for (x in collection)` special-cased for the three built-in collection types, with REPLACE-AT M7 markers.
- **Auto-promotion:** `array<T>` → `fixed<T>` codegen surface (silent perf win) when typeck proves never-grown.
- **Constraint contracts:** `function sort<T follows Comparable>(...)` inline.
**Flag**: N/A
**Status**: in planning (this plan)
**Depends on**: M4
**Ships via**: `/pr` per phase; `/release` at milestone end (tag `v0.1.0-m5`)

---

## What M5 explicitly is NOT (deferred to later milestones)

- `options` declarations — M6
- Union types `A | B` (NOT including `maybe<T>` — that ships in M5) — M6
- `if (x is Type)` pattern narrowing — M6
- Custom user-defined iterables via `follows Iterable<T>` — M7
- `errors` keyword + cascades — M7
- Full Unicode strings (`.byteAt`, `.graphemeAt`) — M7
- Modules / imports — M8
- Doc comments — M8
- Sensitive type modifier — M8
- Concurrency keyword parsing — M8
- Bignum `number<N>` for N > 34 — M8
- IDE muted-hint surfaces for auto-promotion, hash tier, generic instantiation — v0.2 LSP
- Tier 3 lint suggestions (`prefer-fixed-when-immutable`, etc.) — v0.4
- xxhash3 fast opt-in for maps + surface syntax — future (unbounded)
- Identity-hash for `map<int, V>` — future (unbounded)
- Higher-kinded types, lifetime parameters, associated types, general const generics — NEVER in v0.1 per `design/generics.md`

**If a phase below feels like it's drifting into any of the above, STOP and re-plan.**

---

## Phases

> Per `~/.claude/memory/branching.md`: each phase = one PR; one branch; merges to `main` before next phase starts. Phase boundaries enforce review cadence.

---

### Phase 0: Doc lockdown (master plan + design/maybe.md + master-plan M5 paragraph)

**PR scope**: Update the master plan's M5 paragraph (currently uses `[T]` syntax + lists `maybe` in M6); create `design/maybe.md`; add a one-line cross-reference in `design/generics.md` for `maybe<T>` as the canonical generic example; update todos.md to remove the "use `<>` not `[]`" idea-bin entry (resolved by this plan).
**Branch**: `chore/m5-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~300 (docs only)
**Ships via**: `/pr`
**Objective**: Lock the M5 design surface before any code lands. Future contributors reading the master plan see the correct M5 scope; future contributors reading `design/maybe.md` see the rationale for moving it from M6 to M5.
**Why this phase exists**: per `no-duct-tape.md` — the master plan currently lists M5 as `array[T]` / `fixed[T]` / `map[K,V]` and says `maybe T` is in M6. Both are wrong. Shipping M5 P1-P7 without fixing the master plan first means every future plan-reading session sees stale info and may make decisions on it. Fix the source of truth first.
**Current-state anchors**:
- `.claude/plans/active/v0-1-compiler.md:184` — M5 milestone paragraph (uses `[T]` syntax, lists `Iterable[T]` reservation but no `maybe<T>` mention)
- `.claude/plans/active/v0-1-compiler.md:190` — M6 milestone paragraph (lists `maybe T` — needs removal)
- `.claude/todos.md:20` — `<>` generics syntax idea-bin entry (resolved here)
- `design/generics.md` — already correct (uses `<>`); add a cross-reference to design/maybe.md
- `spec/maybe.md` — currently uses `maybe string` (no angle brackets). M5 syntax is `maybe<string>`. P0 updates spec/maybe.md to use `<>` throughout for consistency with `array<T>` / `fixed<T>` / `map<K,V>`.
**Files (expected scope)**:
- `.claude/plans/active/v0-1-compiler.md` — update M5 and M6 milestone paragraphs
- `.claude/todos.md` — remove resolved entry
- `design/maybe.md` — CREATE (new file). MUST include the maybe<T> LLVM lowering decision table, the none type inference rules, the flow-sensitive .value enforcement rules, and the cycle-leak documented limitation (all four are locked in this plan's FINAL LOCKED DECISIONS section; design/maybe.md is the durable home for them).
- `design/generics.md` — add 1-2 line cross-reference to design/maybe.md
- `spec/maybe.md` — update syntax `maybe T` → `maybe<T>` throughout (with diff-only changes; preserve structure + examples)
**Deviation rule**: NO code changes in this PR. If you find yourself touching `crates/**`, STOP — that's P1+.
**Steps**:
1. Open `.claude/plans/active/v0-1-compiler.md` at line ~184. Rewrite the M5 paragraph to: `array<T>` / `fixed<T>` / `map<K, V>` (use `<>`); add `maybe<T>` to M5 scope; remove `maybe<T>` from M6's paragraph.
2. Open `.claude/todos.md` at line ~20. Remove the "`<>` generics syntax — compiler" idea-bin entry (mark resolved by this plan).
3. Create `design/maybe.md` with sections: "User Spec" (one-line link to spec/maybe.md), "Why maybe<T> ships in M5 instead of M6" (cite this plan), "LLVM Lowering Decision Table" (copy the table from this plan's FINAL LOCKED DECISIONS section verbatim — it lives durably in design/maybe.md, and the plan becomes archival once M5 ships), "`none` type inference rules" (copy from FINAL LOCKED DECISIONS), "Flow-sensitive .value enforcement rules" (copy from FINAL LOCKED DECISIONS), "Documented v0.1 limitation: cycle leak through `maybe<Self>` mutation" (the locked decision), "Why maybe<T> is built-in, not a stdlib generic" (it's a primitive of the type system, used in every collection's `.get()` signature).
4. Update `spec/maybe.md`: rewrite `maybe string` → `maybe<string>`, `maybe number` → `maybe<number>`, etc. throughout. Preserve structure + examples; only syntax changes.
5. Add to `design/generics.md` (near "What's NOT in v0.1"): "See `design/maybe.md` for `maybe<T>` — the first built-in generic primitive shipped via M5's generics engine."
**Acceptance criteria**:
- [ ] Master plan's M5 paragraph uses `<>` syntax and lists `maybe<T>` in M5 scope
- [ ] Master plan's M6 paragraph does NOT list `maybe<T>` (moves to M5)
- [ ] `design/maybe.md` exists with the LLVM-lowering decision table, none-inference rules, flow-sensitive `.value` rules, and cycle-leak documented limitation
- [ ] `design/generics.md` cross-references `design/maybe.md`
- [ ] `spec/maybe.md` uses `maybe<T>` syntax throughout (no bare `maybe string` remnants)
- [ ] `.claude/todos.md` no longer has the `<>` generics idea-bin entry
- [ ] Jargon audit passes (`cargo test -p ynz-diagnostics --test jargon_audit`) — no jargon snuck into the new design doc
**Quality gate**:
- [ ] No "monomorphize" / "polymorphic" / banned-jargon words in design/maybe.md
- [ ] design/maybe.md uses Yinz vocab: "shape" not "type", "follows" not "implements", "none" not "null"
**Verification**: `git diff --name-only main..` lists exactly the 4 files; `cargo test --workspace` still passes (no code changed); `cargo test -p ynz-diagnostics --test jargon_audit` passes.

---

### Phase 1: Lexer + AST scaffolding (type-param syntax, `none`, bracket-index, CallExpr type-args, banned-jargon)

**PR scope**: Add `Tok::None` to the lexer. Add AST variants for type parameters (`Type::TypeParam`, `Type::Generic`, `Type::Maybe`), expressions (`Expr::NoneLit`, `Expr::IndexAccess`), and the new statement (`Stmt::IndexAssign`). Add `GenericParam` struct. Add `generics: Vec<GenericParam>` field to `FunctionDecl` and `ShapeDecl`. **Extend `CallExpr` with `type_args: Option<Vec<Type>>` field** (defaulting to `None` for backwards-compat with M1-M4 call sites). Update variant-count tests with `// test-ratchet:` markers.

**Expected post-P1 variant counts** (asserted by the variant-count locked tests):
- `Type`: 13 (was 10 after M4: Nothing, Named, Error, Int, Float, Number, Bool, Range, Dynamic, SelfType — note the M3-pinned doc comment says "Current count: 8" but M4 added Dynamic + SelfType bringing it to 10; P1 verifies the actual current count first then adds TypeParam + Generic + Maybe → 13). **If the M4 test count is found to be wrong, fix the count message in the SAME PR**; do not ship M5 P1 on a stale count assertion.
- `Expr`: 15 (was 13 after M4: 10 base + StructLit + PostfixOp + FieldAccess + SelfValue — verify); P1 adds NoneLit + IndexAccess → 15.
- `Stmt`: 9 (was 8 after M4: 7 base + FieldAssign; M3 doc comment may also be stale — verify); P1 adds IndexAssign → 9.
**Branch**: `feat/m5-lexer-ast`
**Flag**: N/A
**Est. lines**: ~600 (lexer ~50 + AST struct/enum additions + test-ratchet markers + jargon entries)
**Ships via**: `/pr`
**Objective**: After P1, parsing `none` produces a `Tok::None` token; `Expr::NoneLit` exists in the AST; the variant-count tests are updated to reflect the new shape. No parsing logic for generic syntax yet (P2).
**Why this phase exists**: Lock the AST + token shape before parser work in P2 so reviewers see the structural decisions in isolation. The same pattern M4 P1 used.
**Current-state anchors**:
- `crates/ynz-parser/src/token.rs` — Tok enum (57 variants after M4)
- `crates/ynz-parser/src/lexer.rs` — keyword recognizer
- `crates/ynz-ast/src/nodes.rs` — Type enum (8 variants), Expr enum (10 variants per existing test-ratchet count), Stmt enum (8 variants)
- `crates/ynz-diagnostics/src/banned_jargon.rs` — workspace-wide grep allowlist
**Files (expected scope)**:
- `crates/ynz-parser/src/token.rs` — add `Tok::None`
- `crates/ynz-parser/src/lexer.rs` — recognize `none` keyword
- `crates/ynz-ast/src/nodes.rs` — add `GenericParam`, `Type::TypeParam`, `Type::Generic`, `Type::Maybe`, `Expr::NoneLit`, `Expr::IndexAccess`, `Stmt::IndexAssign`; add `generics: Vec<GenericParam>` field to `FunctionDecl` + `ShapeDecl`; extend `CallExpr` with `type_args: Option<Vec<Type>>` field
- `crates/ynz-ast/tests/variant_count.rs` (or equivalent) — update with `// test-ratchet:` markers
- `crates/ynz-diagnostics/src/banned_jargon.rs` — confirm `monomorphize`/`polymorphic`/etc. are already there (no change needed unless found missing)
**Deviation rule**: NO parser logic in this phase — parser changes are P2. NO typeck recognition of new types — that's P3.
**Steps**:
1. Add `Tok::None` to `crates/ynz-parser/src/token.rs`. Add to `Display` impl.
2. Update `crates/ynz-parser/src/lexer.rs` keyword table: `"none" -> Tok::None`. Snapshot test: lexing `let x: maybe<int> = none` produces the expected token sequence.
3. Add `crates/ynz-ast/src/nodes.rs::GenericParam { name: String, name_span: SourceSpan, constraints: Vec<(String, SourceSpan)>, span: SourceSpan }`.
4. Extend `Type` enum: `TypeParam { name: String, span: SourceSpan }`, `Generic { name: String, name_span: SourceSpan, args: Vec<Type>, span: SourceSpan }`, `Maybe { inner: Box<Type>, span: SourceSpan }`. Mark each with `// test-ratchet: M5 adds <variant> for ...`.
5. Extend `Expr` enum: `NoneLit { span: SourceSpan }`, `IndexAccess { receiver: Box<Expr>, index: Box<Expr>, span: SourceSpan }`. Mark with test-ratchet.
6. Extend `Stmt` enum: `IndexAssign { receiver: Box<Expr>, index: Box<Expr>, value: Expr, span: SourceSpan }`. Mark with test-ratchet.
7. Add `generics: Vec<GenericParam>` field to `FunctionDecl` and `ShapeDecl`. Update all construction sites (lib internal only — parser fills in `Vec::new()` for now; tests pass with empty generics list).
7a. Extend `CallExpr` with `type_args: Option<Vec<Type>>` field. Defaults to `None` (inference). Update construction sites; M1-M4 tests pass with `None`.
8. Update variant-count tests with proper test-ratchet messages. **Verify the M3/M4-pinned counts against current code FIRST** — if `m3_type_variant_count_locked` claims `Current count: 8` but the file actually has 10 variants, fix the count + the doc comment in this PR as collateral.
9. Run `cargo test --workspace` and confirm all M4 tests still pass (no behavioral change yet, just AST scaffolding).
**Acceptance criteria**:
- [ ] Lexing `let x: maybe<int> = none` produces tokens including `Tok::None`
- [ ] `Type::TypeParam`, `Type::Generic`, `Type::Maybe`, `Expr::NoneLit`, `Expr::IndexAccess`, `Stmt::IndexAssign` exist
- [ ] `GenericParam` struct exists with `name`/`constraints`/`span`
- [ ] `FunctionDecl.generics` and `ShapeDecl.generics` fields exist and default to empty
- [ ] `CallExpr.type_args: Option<Vec<Type>>` field exists and defaults to `None`
- [ ] Variant counts asserted by the locked tests match the actual code (collateral fix if M4 left stale numbers)
- [ ] All M4 tests still pass
**Quality gate**:
- [ ] No `unwrap()` introduced
- [ ] No banned-jargon in any new comment or struct doc
- [ ] No parser logic touching the new variants (P2 work)
- [ ] No typeck logic recognizing the new types (P3 work)
**Verification**: `cargo test --workspace` green; `cargo test -p ynz-diagnostics --test jargon_audit` green; snapshot test for the `let x: maybe<int> = none` token stream produces expected output.

---

### Phase 2: Parser extension (generic decls, generic instantiations, bracket index, contextual `<` disambiguation)

**PR scope**: Parse `<T>` type-parameter lists in function and shape declarations; parse generic type instantiations (`array<int>`, `Pair<A, B>`, `maybe<T>`) in type positions; parse `none` literal as `Expr::NoneLit`; parse `arr[i]` as `Expr::IndexAccess`; parse `arr[i] = v` as `Stmt::IndexAssign`; parse generic call-site syntax `foo<T>(args)` with TypeScript-style speculative parsing; parse `follows Trait1, Trait2` constraints inline in type-param lists.
**Branch**: `feat/m5-parser`
**Flag**: N/A
**Est. lines**: ~1400 (parser ~900 + 50+ parse snapshot tests + error-recovery tests)
**Ships via**: `/pr`
**Objective**: After P2, a source file containing every M5 surface (generic functions, generic shapes, generic instantiations, bracket index, index-assign, none, maybe<T> annotations, follows constraints) parses without error AND produces the expected AST. Typeck still rejects the new types (P3) — but parser is green.
**Why this phase exists**: Parser correctness is its own phase. Disambiguating `<` at call sites is the trickiest piece — needs its own snapshot suite and adversarial tests (deeply-nested generics, broken syntax that should NOT parse as generic, comparison-vs-generic boundary cases). Land before typeck so reviewers see parse decisions in isolation.
**Current-state anchors**:
- `crates/ynz-parser/src/parser.rs` — entry points and combinators
- `crates/ynz-parser/tests/snapshots.rs` (M4 file) — existing parse-snapshot patterns
**Files (expected scope)**:
- `crates/ynz-parser/src/parser.rs` — generic-param parser, generic-instantiation parser in type position, bracket-index expr, index-assign stmt, none-literal, speculative `<` parsing at call sites, multi-constraint `follows` list
- `crates/ynz-parser/tests/snapshots.rs` — new snapshot tests
- `crates/ynz-parser/tests/recovery.rs` — error recovery for broken generic syntax
**Deviation rule**: NO typeck recognition of the new types (P3). NO codegen (P4-P5).
**Steps**:
1. Implement generic-param-list parser: `<T>`, `<T, U>`, `<T follows Comparable>`, `<T follows A, B>`, `<T follows Comparable, U follows Other>`. Recover gracefully on malformed lists.
2. Wire into `FunctionDecl` parser: `function name<T>(...)`. Wire into `ShapeDecl` parser: `shape Name<T> { ... }`.
3. Implement generic-instantiation parser in type position: `array<int>`, `Pair<int, string>`, `maybe<Pair<A, B>>` (nested). Cap depth at 16 to prevent pathological inputs.
4. Implement bracket-index expression: `arr[i]` → `Expr::IndexAccess`. Handle chained: `m["alice"][0]` — `IndexAccess` containing `IndexAccess`.
5. Implement index-assign statement: `arr[i] = v` → `Stmt::IndexAssign`. Distinguish from field-assign at parse time by the bracketed index.
6. Implement `none` literal: `Tok::None` → `Expr::NoneLit`.
7. Implement contextual `<` disambiguation at call site: when parsing a postfix expression and the current token is `<`, speculatively try parsing a type-argument list followed by `(`. If success → generic call, populate `CallExpr.type_args = Some(types)`. If failure within 32 tokens → backtrack and treat `<` as comparison. (The `type_args` field on `CallExpr` is added in P1, NOT here.)
8. Snapshot tests: 30+ parse tests covering every surface, plus adversarial:
   - `a < b` (comparison, NOT generic call)
   - `foo<T>(x)` (generic call with one type arg)
   - `foo<T, U>(x, y)` (generic call with two type args)
   - `let x = a < b > c` (left-to-right comparison chain; not generic)
   - `arr[a < b]` (comparison inside index)
   - Deeply nested: `Pair<Pair<int, string>, Pair<Pair<int, int>, string>>`
   - Broken: `foo<` (no close); `foo<T,` (no close); `foo<T(x)` (close-paren before close-angle)
9. Recovery tests: malformed generic decls produce a single descriptive error + continue parsing the rest of the module.
**Acceptance criteria**:
- [ ] `function identity<T>(give value: T) -> T { return value }` parses to `FunctionDecl` with `generics: [T]`
- [ ] `shape Pair<A, B> { first: A, second: B }` parses to `ShapeDecl` with `generics: [A, B]`
- [ ] `let arr: array<int> = [1, 2, 3]` parses with `Type::Generic { name: "array", args: [Int] }` annotation
- [ ] `arr[0]` parses to `Expr::IndexAccess`
- [ ] `arr[0] = 5` parses to `Stmt::IndexAssign`
- [ ] `let x: maybe<int> = none` parses with `Type::Maybe { inner: Int }` and `Expr::NoneLit`
- [ ] `identity<int>(5)` parses to a `Call` with `type_args: Some([Int])`
- [ ] `a < b > c` parses as a comparison chain, NOT a generic call (regression test)
- [ ] `function sort<T follows Comparable>(...)` parses with `constraints: [Comparable]` on T
- [ ] 30+ parse snapshot tests green
- [ ] All M4 parse tests still pass (no regressions)
**Quality gate**:
- [ ] No `unwrap()` in parser code
- [ ] Speculative `<` parsing has a 32-token cap; adversarial-input regression test passes
- [ ] Generic-instantiation depth cap (16 levels) emits a three-part diagnostic when exceeded
- [ ] Error recovery produces ONE diagnostic per malformed construct, not a cascade
**Verification**: `cargo test -p ynz-parser` green; snapshot test running every M5 surface end-to-end through the parser; adversarial benchmark (10k-token `<<<...` chain) completes in <1s.

---

### Phase 3a: Typeck — generics engine (type params, substitution, monomorph table, constraint checking)

**PR scope**: Build the generics engine in typeck. Add type-param scope tracking, type substitution, monomorphization table population, constraint checking against `follows`. Add typeck for generic function declarations and generic shape declarations. Generic instantiations in type position are resolved here. Type inference at call sites is implemented here. Built-in types (`array`/`fixed`/`map`/`maybe`) are NOT recognized yet (P3b).
**Branch**: `feat/m5-typeck-generics`
**Flag**: N/A
**Est. lines**: ~1800
**Ships via**: `/pr`
**Objective**: After P3a, a generic function like `function identity<T>(give value: T) -> T` type-checks: typeck sees `T` as a type parameter, substitutes the concrete type at each call site, records the instantiation in the `MonomorphizationTable`, and verifies any `follows` constraints. Generic shapes follow the same pattern. NO codegen yet — typeck is verified via the monomorphization table contents and typed-AST snapshots.
**Why this phase exists**: The generics engine is the load-bearing piece of M5. Land it in isolation so reviewers see substitution, constraint check, and the monomorph queue without the noise of built-in collection types.
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs` — entry points
- `crates/ynz-typeck/src/types.rs` — Type representation
- `crates/ynz-typeck/src/scope.rs` — scope tracking
- `crates/ynz-typeck/src/shapes.rs` — ShapeTable + shape type checks
- `crates/ynz-typeck/src/signatures.rs` — function signatures
**Files (expected scope)**:
- `crates/ynz-typeck/src/generics.rs` (NEW) — TypeParam scope, Substitution, MonomorphizationTable, constraint check
- `crates/ynz-typeck/src/check.rs` — wire generics into function/shape decl checking + call-site inference
- `crates/ynz-typeck/src/queries.rs` — new salsa query `monomorphization_table(typed_module) -> Arc<MonomorphizationTable>`
- `crates/ynz-typeck/tests/generics_typeck.rs` (NEW) — 40+ tests
**Deviation rule**: NO recognition of built-in collection types here (P3b). NO codegen (P4-P5). NO `maybe<T>` semantics (P3b — `maybe<T>` is a built-in generic primitive built on top of this engine).
**Steps**:
1. Define `TypeVar` (an unresolved type parameter during inference). Define `Substitution = HashMap<TypeVarId, Type>`.
2. Define `MonomorphizationTable = HashMap<(DeclId, Vec<Type>), MonoSignature>` — keyed by (generic-decl, instantiation type args).
3. Implement type-param scope: when entering a generic function/shape body, push `<T>`'s name into the scope under `Type::TypeParam`.
4. Implement `unify(Type, Type, &mut Substitution) -> Result<(), TypeError>` — Hindley-Milner style; locked to NO higher-kinded vars.
5. Implement call-site inference: at `foo(arg)` where `foo` is generic, instantiate each `<T>` with a fresh `TypeVar`, unify each parameter type with the corresponding argument type, resolve the substitution, record the concrete instantiation in the table.
6. Implement explicit type args at call sites: `foo<int>(x)` skips inference and uses the user-given types; still records in the table.
7. Implement constraint checking: when a type-param has `T follows Comparable`, at every instantiation site, verify the concrete T follows the Comparable contract (reuse M4 P3b's follows-verification machinery).
8. Generic function decl typeck: substitute body types, type-check the body under the type-param scope, record the generic signature.
9. Generic shape decl typeck: type-check field types under the type-param scope, record the generic shape in ShapeTable with its type-param list.
10. Salsa-track the MonomorphizationTable query. On any change to a generic decl, recomputed; on any change to a non-generic decl, unaffected.
11. Diagnostics (every one follows WHAT/WHAT-INSTEAD/WHY):
    - "Cannot work out the type parameter T for function `identity` — pass a value or annotate explicitly. Examples: `identity(5)` (T = int) or `identity<int>()`."
    - "Type Player does not follow contract Comparable. To make Player sortable, add `follows Comparable` to its declaration AND implement the compare(share self, share other: Self) -> int function."
    - "Generic type list nesting exceeds 16 levels — this is almost certainly a cyclic type. Check for `shape Node<T> { value: Node<T> }` patterns and break the cycle with `maybe<Node<T>>` or `array<Node<T>>`."
    - "Two different concrete types for T at this call site: int and string. The compiler needs a single type per parameter; pick one (e.g., `identity<int>(5)`)."
**Acceptance criteria**:
- [ ] `function identity<T>(give value: T) -> T { return value }` type-checks
- [ ] `let n = identity(5)` records instantiation `(identity, [int])` in MonomorphizationTable
- [ ] `let p = identity<string>("hello")` records `(identity, [string])`
- [ ] `shape Pair<A, B> { first: A, second: B }` type-checks
- [ ] Field access through generic shapes: given `let p: Pair<int, string> = ...`, `p.first` is `int`
- [ ] `function sort<T follows Comparable>(...)` rejects a non-Comparable concrete type at the call site with the expected three-part error
- [ ] Type inference works for nested generics: `pair(5, "hello")` infers `A=int, B=string`, records `(pair, [int, string])`
- [ ] 40+ typeck tests green
**Quality gate**:
- [ ] No banned-jargon in any error message (jargon audit green)
- [ ] MonomorphizationTable salsa-tracked; benchmark confirms small edit only invalidates affected entries
- [ ] No unwrap() in the engine
- [ ] All M4 typeck tests still pass (no regressions)
- [ ] Generic-instantiation depth cap (16) raises a teaching error
**Verification**: `cargo test -p ynz-typeck --test generics_typeck` green; `cargo test --workspace` green.

---

### Phase 3b: Typeck — built-in `maybe<T>` + `array<T>` + `fixed<T>` + bracket sugar + for-loop iteration

**PR scope**: Wire built-in generic types `maybe<T>`, `array<T>`, `fixed<T>` into the type-name resolver. Add their method tables. Implement bracket-sugar desugaring (`arr[i]` → `arr.get(i)`, `arr[i] = v` → `arr.set(i, v)`). Implement flow-sensitive `.value` enforcement on `maybe<T>`. Implement `for (x in collection)` special-case for these three types. Auto-promotion analysis (`array<T>` → `fixed<T>`) typeck side: scan def-use, record promotions in a PromotionReport salsa query for codegen to consume.
**Branch**: `feat/m5-typeck-array-fixed-maybe`
**Flag**: N/A
**Est. lines**: ~1600
**Ships via**: `/pr`
**Objective**: After P3b, `let arr: array<int> = [1, 2, 3]` type-checks; `arr[0]` produces `maybe<int>`; `arr[0] = 5` is a valid statement; `let m: maybe<int> = none` type-checks; `m.value` is a compile error without prior `.exists()` check; `for (x in arr)` type-checks with `x: int`. Auto-promotion is recorded but not codegened yet (P4a).
**Why this phase exists**: Validates the generics engine from P3a against its first real consumers. The built-in types use the SAME engine but have hand-rolled method tables (vs user-defined `follows` contracts).
**Current-state anchors**:
- `crates/ynz-typeck/src/intrinsics.rs` — primitive intrinsic table from M2
- `crates/ynz-typeck/src/types.rs` — Type representation
- M3 `range()`-special-case in `check.rs` (around line ~408 per grep earlier) — for-loop boundary
**Files (expected scope)**:
- `crates/ynz-typeck/src/builtins.rs` (NEW) — `array<T>` / `fixed<T>` / `maybe<T>` registration with method tables
- `crates/ynz-typeck/src/check.rs` — bracket-sugar desugar; for-loop special-case extension; `.value` flow-sensitive check
- `crates/ynz-typeck/src/scope.rs` — `MaybeKnownNonNone` flag in scope (for `.value` after `.exists()`)
- `crates/ynz-typeck/src/queries.rs` — `promotion_report(typed_module) -> Arc<PromotionReport>` salsa query
- `crates/ynz-typeck/tests/builtins.rs` (NEW)
**Deviation rule**: NO `map<K, V>` here (P3c). NO codegen (P4-P5). The actual auto-promotion CODEGEN ships in P4a; this phase just records the analysis.
**Steps**:
1. Register `array<T>` in the type-name resolver. Define its method table: `.add(lend self, value: T) -> nothing`, `.remove(lend self, index: int) -> nothing`, `.get(share self, index: int) -> maybe<T>`, `.set(lend self, index: int, value: T) -> nothing`, `.count(share self) -> int`, `.first(share self) -> maybe<T>`, `.last(share self) -> maybe<T>`, plus `.filter`/`.map`/`.find`/`.contains`/`.unique`/`.limit`/`.concat`/`.append`/`.prepend`/`.sort` per spec/collections.md.
2. Register `fixed<T>` in the type-name resolver. Method table is a subset of `array<T>`'s (no `.add`/`.remove`/`.removeFirst`/`.removeLast`; `.set` valid; `.append`/`.prepend` return new collections). Track size `N` at the type level for literal-OOB compile errors.
3. Register `maybe<T>` in the type-name resolver. **The shape has BOTH a virtual-field table AND a method table** (per `.claude/rules/dot-postfix.md` — `.value` is access without parens, `.exists()` and `.or()` are actions with parens):
   - **Virtual field**: `.value: T` — typeck-validated, requires flow-sensitive proof per the locked rules above. NOT a method. The compiler treats `m.value` as a field-access expression; codegen extracts the value-slot from the maybe's LLVM lowering.
   - **Method table**: `.exists(share self) -> bool`, `.or(share self, default: T) -> T`.
   - This makes `maybe<T>` the FIRST built-in type with both virtual-fields and methods. Future built-ins (none planned in v0.1) follow this pattern.
4. Implement bracket-sugar desugar in typeck: `Expr::IndexAccess { receiver, index }` → method-call `receiver.get(index)` if receiver is a built-in collection; compile error with the existing "bracket access is for collections" diagnostic if receiver is a shape.
5. Implement index-assign desugar: `Stmt::IndexAssign { receiver, index, value }` → method-call `receiver.set(index, value)`. Type-check the call.
6. Implement `none` typing per the locked rules table in "FINAL LOCKED DECISIONS" → "`none` type inference rules". `Expr::NoneLit { span }` produces a `TypeVar` constrained to `maybe<T>`; T resolves via the context-walking algorithm specified there (binding annotation → return type → enclosing call parameter type → sibling branch type → compile error). Each `none` is resolved independently against its own immediate context — no global propagation.
7. Implement flow-sensitive `.value` enforcement per the locked rules table in "FINAL LOCKED DECISIONS" → "Flow-sensitive `.value` enforcement rules". M5 ships the positive-condition, negated `if (!m.exists()) {...} else {...}`, and short-circuit-AND cases. Early-return narrowing (e.g., `if (m.exists()) { return ... } m.value`) is M6 work — M5 produces a teaching error pointing to M6 for those forms. Reassignment to `m` inside an `if (m.exists())` block invalidates the flag; closures don't carry the flag (closures are v0.3+).
8. Implement for-loop special-case extension:
   - `for (x in arr)` where `arr: array<T>` → `x: T`. `// REPLACE-AT M7: dispatch via Iterable<T>`.
   - `for (x in fixed)` where `fixed: fixed<T>` → `x: T`. Same comment.
   - `for (x in map)` where `map: map<K, V>` → `x: Entry<K, V>` with `.key`/`.value` fields. Same comment.
9. Implement auto-promotion analysis: salsa query `promotion_report(typed_module)` walks each `let ident: array<T> = literal` binding's def-use; if no `.add()` / `.remove()` / `.resize()` and no `lend array<T>` parameter pass, record `(binding_id, fixed<T, literal_count>)`. P4a consumes this.
10. Diagnostics for every new error class. Examples:
    - "`m.value` requires you to first check `m.exists()`. The compiler can't be sure `m` has a value here. Try: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`."
    - "Cannot work out which type `none` should be here. `none` is the absent value of `maybe<T>` for some T — annotate the binding: `let x: maybe<int> = none`."
    - "Index 5 is out of bounds for `fixed<int, 3>`. `fixed` has a size locked at creation; this collection holds 3 items, so valid indices are 0..2."
    - "`for (x in arr)` over user-defined types arrives in M7 with the `Iterable<T>` protocol. For now, you can iterate built-in `array<T>`, `fixed<T>`, and `map<K, V>`."
11. Test cases for every method on every built-in (~80+ tests).
**Acceptance criteria**:
- [ ] `let arr: array<int> = [1, 2, 3]` type-checks
- [ ] `arr.add(4)` type-checks
- [ ] `arr[0]` produces `maybe<int>`
- [ ] `arr[0] = 5` type-checks
- [ ] `let f: fixed<string> = ["a", "b", "c"]` type-checks with `Type::Generic { name: "fixed", args: [Generic { name: "string", ... }] }` carrying the size N=3
- [ ] `f[5]` (literal OOB on size-3 fixed) is a compile error
- [ ] `f[i]` (non-literal) defers to runtime
- [ ] `let m: maybe<int> = none` type-checks
- [ ] `m.value` without prior `.exists()` is a compile error
- [ ] `if (m.exists()) { m.value }` type-checks
- [ ] `m.or(0)` produces `int`
- [ ] `for (x in arr)` binds `x: int` and type-checks the body
- [ ] auto-promotion analysis records `(binding_id, fixed<T, N>)` for proven-never-grown bindings
- [ ] 80+ typeck tests green
**Quality gate**:
- [ ] No banned-jargon in any error message
- [ ] PromotionReport salsa-cached
- [ ] All P3a tests still pass
- [ ] Flow-sensitive `.value` check produces a useful error, not a generic "type mismatch"
**Verification**: `cargo test -p ynz-typeck` green; `cargo test --workspace` green.

---

### Phase 3c: Typeck — built-in `map<K, V>` + bracket sugar for keys + map iteration

**PR scope**: Wire built-in `map<K, V>` into the type-name resolver. Add its method table. Implement bracket-sugar for map keys (`m["alice"]` → `m.get("alice")` → `maybe<V>`). Implement map iteration (`for (entry in m)` → `entry: Entry<K, V>`). Implement perfect-hash analysis: when a map literal has all-static-string keys, mark the literal as "perfect-hash candidate" for codegen consumption (codegen ships in P4b). Implement duplicate-key detection in map literals.
**Branch**: `feat/m5-typeck-map`
**Flag**: N/A
**Est. lines**: ~1000
**Ships via**: `/pr`
**Objective**: After P3c, `let scores: map<string, int> = { "alice": 90, "bob": 85 }` type-checks; `scores["alice"]` produces `maybe<int>`; `scores["alice"] = 95` type-checks; `for (entry in scores) { entry.key }` type-checks; `{ "alice": 1, "alice": 2 }` produces a duplicate-key compile error. Perfect-hash candidacy is recorded in the typed AST; codegen consumes it in P4b.
**Why this phase exists**: Map is the most complex built-in type. Splitting it from `array`/`fixed`/`maybe` (P3b) keeps each PR reviewable. The Swiss Tables / SipHash codegen lives in P4b; this phase only handles typeck.
**Current-state anchors**:
- P3b artifacts (`builtins.rs`, `promotion_report` query)
- M4 ShapeTable for `MapEntry<K, V>` synthesis (we'll synthesize an internal `MapEntry<K, V>` shape for map iteration)
**Files (expected scope)**:
- `crates/ynz-typeck/src/builtins.rs` — extend with `map<K, V>` registration
- `crates/ynz-typeck/src/check.rs` — extend bracket-sugar to maps; extend for-loop special-case to maps
- `crates/ynz-typeck/src/maps.rs` (NEW) — perfect-hash candidacy analysis; duplicate-key check
- `crates/ynz-typeck/src/queries.rs` — `perfect_hash_candidates(typed_module) -> Arc<...>`
- `crates/ynz-typeck/tests/maps.rs` (NEW)
**Deviation rule**: NO Swiss Tables codegen (P4b). NO SipHash impl (P4b). NO perfect-hash codegen (P4b).
**Steps**:
1. Register `map<K, V>` in the type-name resolver. Define its method table: `.get(share self, key: K) -> maybe<V>`, `.set(lend self, key: K, value: V) -> nothing`, `.has(share self, key: K) -> bool`, `.remove(lend self, key: K) -> nothing`, `.keys(share self) -> array<K>`, `.values(share self) -> array<V>`, `.entries(share self) -> array<MapEntry<K, V>>`, `.count`, `.filter`, `.find`, `.sort`. (`.update({...})` is DEFERRED past M5 per locked decision — not in the method table.)
2. Synthesize an internal `MapEntry<K, V>` shape for iteration: `shape MapEntry<K, V> { key: K, value: V }`. **Locked behavior**:
   - Name: `MapEntry` (NOT `Entry`) — chosen to avoid colliding with future stdlib types or user-defined `Entry` shapes. Reserved by M5; user attempts to declare `shape MapEntry<K, V> { ... }` produce a compile error "MapEntry is a reserved built-in shape name."
   - Visibility: USER-NAMABLE. Users can annotate `let e: MapEntry<string, int> = ...` and use it as a function parameter type. Treated as if `pub shape MapEntry<K, V> { key: K, value: V }` lived in a built-in prelude.
   - Ownership semantics on `entry.key` and `entry.value` during iteration: the iteration variable is **share-borrowed from the map**. Reading `entry.key` and `entry.value` produces share-borrowed references to the map's actual storage (no copy). The iteration body cannot `.lend` or `.give` the entry (compile error per M4 ownership rules). To mutate or own a copy, the user explicitly writes `entry.key.copy()` / `entry.value.copy()`.
   - Monomorphization: yes — `MapEntry<string, int>` and `MapEntry<int, Player>` are distinct types per the standard monomorph machinery.
   - Direct construction: REJECTED. `MapEntry<K, V>` cannot be instantiated via struct literal (the M5 typeck checks for this; the runtime only produces them during iteration). Users construct map entries via `m.set(k, v)`.
3. Implement bracket-sugar for maps: `m["alice"]` → `m.get("alice")` → `maybe<V>`. Same desugar path as P3b, just dispatch based on receiver type.
4. Implement map iteration in for-loop special-case: `for (entry in m) where m: map<K, V>` → `entry: Entry<K, V>`. `// REPLACE-AT M7: dispatch via Iterable<T>`.
5. Implement duplicate-key detection: walk every map literal's entries; build a HashMap<K_literal, span_first> while parsing; on duplicate, emit a three-part diagnostic naming both spans.
6. Implement perfect-hash candidacy analysis: when a map literal has all-static keys AND all keys are string literals (or int literals), record `(literal_span, keys: Vec<...>)` in a new `perfect_hash_candidates` salsa query. Codegen in P4b consumes this.
7. Diagnostics:
   - "Duplicate key `\"alice\"` in this map literal — the key is listed twice. Each key must be unique. (The compiler refuses to silently pick one — name a single intended value.)"
   - "Cannot use `.alice` to look up a key. Dot access is for shape fields, which have compile-time-known names. Map keys are runtime values — use `scores[\"alice\"]` to look up the `\"alice\"` key. If your keys are actually fixed and known at compile time, consider a `shape` instead of a `map`."
   - "`for (x in map)` binds `x` to an `MapEntry<K, V>`. Use `x.key` and `x.value` to access the parts."
8. Test cases: ~40+ tests covering construction, access, mutation, iteration, duplicate keys, perfect-hash candidacy, key-type mismatch.
**Acceptance criteria**:
- [ ] `let scores: map<string, int> = { "alice": 90, "bob": 85 }` type-checks
- [ ] `scores["alice"]` produces `maybe<int>`
- [ ] `scores["alice"] = 95` type-checks
- [ ] `scores["alice"].or(0)` produces `int`
- [ ] `for (entry in scores) { print(entry.key) }` type-checks with `entry: Entry<string, int>`
- [ ] `{ "alice": 1, "alice": 2 }` produces a duplicate-key compile error naming both spans
- [ ] `scores.count()` produces `int`
- [ ] `scores.keys()` produces `array<string>`
- [ ] Perfect-hash candidates recorded for all-static-key literals
- [ ] 40+ tests green
**Quality gate**:
- [ ] No banned-jargon in any error message
- [ ] All P3a/P3b tests still pass
- [ ] Map literal key-type mismatch produces a useful error
**Verification**: `cargo test -p ynz-typeck` green; `cargo test --workspace` green.

---

### Phase 4a: Codegen — monomorphization scaffolding + `maybe<T>` + `array<T>` + `fixed<T>` runtime + auto-promotion

**PR scope**: Implement codegen-side monomorphization. For each `(decl, type_args)` pair in the MonomorphizationTable, emit a specialized LLVM function/struct. Implement `maybe<T>` LLVM lowering (tagged-union vs null-pointer encoding per-instantiation). Implement `array<T>` runtime (`ynz_array_*` symbols, 1.5× growth, drop loop + free). Implement `fixed<T>` runtime (alloca + bounds-check intrinsics). Implement bracket-sugar lowering (already desugared by typeck to `.get()`/`.set()`; codegen lowers those method calls to the runtime symbols). Implement auto-promotion: consume the PromotionReport and emit `fixed<T, N>` codegen for promoted bindings.
**Branch**: `feat/m5-codegen-mono-array-fixed`
**Flag**: N/A
**Est. lines**: ~2500
**Ships via**: `/pr`
**Objective**: After P4a, every program in M5 surface that doesn't use `map<K, V>` runs end-to-end through the full compile pipeline. `let arr: array<int> = [1, 2, 3]; arr.add(4); print(arr[0].or(0))` compiles, runs, prints `1`. `let f: fixed<string> = ["a", "b"]; print(f[0].or("x"))` compiles, runs, prints `a`. `let m: maybe<int> = none; print(m.or(99))` compiles, runs, prints `99`. Auto-promoted bindings produce fixed-codegen.
**Why this phase exists**: Map codegen is large enough for its own phase (P4b). Splitting array/fixed/maybe out keeps each phase's LLVM IR review tractable.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs` — codegen entry
- `crates/ynz-codegen/src/runtime_decls.rs` — extern symbol declarations (M4)
- `crates/ynz-codegen/src/shape_types.rs` — shape LLVM-type construction (M4)
- `crates/ynz-codegen/src/vtable.rs` — vtable emission (M4)
- `crates/ynz-runtime/src/lib.rs` — runtime C symbol shims (M4 ships `ynz_alloc` / `ynz_free`)
**Files (expected scope)**:
- `crates/ynz-codegen/src/mono.rs` (NEW) — monomorphization driver
- `crates/ynz-codegen/src/maybe.rs` (NEW) — maybe lowering per-instantiation
- `crates/ynz-codegen/src/collections_codegen.rs` (NEW) — array/fixed lowering
- `crates/ynz-codegen/src/runtime_decls.rs` — declare new runtime symbols (`ynz_array_new`, `ynz_array_push`, `ynz_array_grow`, `ynz_array_drop`, etc.)
- `crates/ynz-codegen/src/emit.rs` — wire generic decl emission through mono.rs; consume PromotionReport
- `crates/ynz-runtime/src/lib.rs` — implement `ynz_array_*` runtime in C (or hand-rolled Rust compiled to native)
- `crates/ynz-runtime/build.rs` — compile the runtime
- `crates/ynz-codegen/tests/snapshots.rs` — IR-snapshot tests
- `crates/ynz-driver/tests/fixtures/m5_*.ynz` (NEW) — runnable end-to-end tests
**Deviation rule**: NO map codegen here (P4b). The runtime symbols for map go in P4b.
**Steps**:
1. Implement `mono.rs`: walk MonomorphizationTable; for each entry, emit a specialized LLVM function with mangled name (e.g., `identity_int`, `Pair_int_string_first`). De-dupe identical instantiations.
2. Implement `maybe.rs`: for each `maybe<T>` instantiation, decide tagged-union vs null-pointer encoding. Record the choice in a side-table; codegen consults it for every load/store. IR-snapshot tests for both encodings.
3. Implement `collections_codegen.rs`: lower `array<T>.add` to `call @ynz_array_push(...)`; lower `array<T>.get` to `call @ynz_array_get(...)` returning `maybe<T>`; etc. Lower `fixed<T>` to alloca + GEP + bounds-check + ynz_panic. Bracket-sugar at this point has already been desugared by typeck to method calls; codegen lowers the method calls.
4. Implement `ynz_array_*` runtime symbols in `crates/ynz-runtime/src/lib.rs` (or a C file built via `build.rs`):
   - `ynz_array_new(elem_size: i64) -> *Array` — initial cap = 8
   - `ynz_array_push(arr: *Array, value: *T) -> nothing` — grows by 1.5× when full
   - `ynz_array_get(arr: *Array, idx: i64) -> MaybeT` — returns tagged-union or null per type
   - `ynz_array_set(arr: *Array, idx: i64, value: *T) -> nothing` — panics on OOB
   - `ynz_array_count(arr: *Array) -> i64`
   - `ynz_array_drop(arr: *Array, drop_elem: fn(*T)) -> nothing` — runs per-element drop then frees the storage
5. Consume `PromotionReport`: for each promoted binding, emit `fixed<T, N>` codegen instead of `array<T>` codegen. Same `.get()`/`.set()` lowering path; just different storage.
6. Wire `ynz_panic` for runtime bounds-check (M2 already has the symbol).
7. IR-snapshot tests for every new codegen surface:
   - `maybe<int>` tagged-union lowering
   - `maybe<*Player>` null-pointer-encoding lowering
   - `array<int>` end-to-end (alloc, push, get returning maybe, drop)
   - `fixed<int, 3>` end-to-end
   - Auto-promoted binding (`let arr: array<int> = [1,2,3]` proven never-grown) emits fixed codegen
   - LLVM `readonly`/`noalias` attributes carry over to monomorphized functions
8. Runnable end-to-end fixtures: `m5_identity.ynz`, `m5_pair.ynz`, `m5_array.ynz`, `m5_fixed.ynz`, `m5_maybe.ynz`, `m5_auto_promotion.ynz`.
**Acceptance criteria**:
- [ ] `m5_identity.ynz` (`function identity<T>(give value: T) -> T; print(identity(42))`) → compiles, prints `42`, exits 0
- [ ] `m5_pair.ynz` (`shape Pair<A, B>; let p = pair(10, 20); print(p.first)`) → prints `10`
- [ ] `m5_array.ynz` exercises `.add` / `.get` / `.count` / iteration → expected output
- [ ] `m5_fixed.ynz` exercises `.get` / `.set` / iteration → expected output
- [ ] `m5_maybe.ynz` exercises `none` / `.exists` / `.value` / `.or` → expected output
- [ ] `m5_auto_promotion.ynz` (an `array<int>` proven never-grown) → IR contains fixed-allocation codegen, not malloc
- [ ] IR-snapshot tests for every new surface pass
- [ ] LLVM `readonly`/`noalias` attributes present on monomorphized functions per M4 invariants
**Quality gate**:
- [ ] No `unwrap()` in codegen paths
- [ ] All M4 codegen tests still pass
- [ ] Runtime symbols use `ynz_*` prefix (matches M4 convention)
- [ ] Bounds-check panic message is teaching-friendly ("index 5 out of bounds for array<int> of length 3")
- [ ] Drop emission produces zero leaks (valgrind clean on the M5 fixture suite — runs in CI if valgrind available, else manual)
**Verification**: `cargo test -p ynz-codegen` green; runnable fixtures produce expected stdout via `./target/debug/ynz run`; IR snapshots match committed goldens.

---

### Phase 4b: Codegen — `map<K, V>` runtime (Swiss Tables + SipHash + perfect-hash codegen)

**PR scope**: Implement `map<K, V>` codegen end-to-end. Ship Swiss Tables runtime in C (or hand-rolled Rust compiled to native). Implement SipHash-2-4 with per-process random key (init from OS entropy in `ynz_siphash_init`). Implement perfect-hash codegen for map literals with all-static keys: generate a compile-time perfect hash function into the binary. Implement insertion-order tracking via parallel index array. Implement iteration in insertion order. Property-based fuzz tests + HashDoS adversarial test.
**Branch**: `feat/m5-codegen-map`
**Flag**: N/A
**Est. lines**: ~3000 (substantial standalone — Swiss Tables alone is ~800 lines C; SipHash ~200 lines; perfect-hash codegen ~400 lines Rust; lots of testing infrastructure)
**Ships via**: `/pr`
**Objective**: After P4b, every M5 surface program runs end-to-end. `let m: map<string, int> = { "alice": 90 }; print(m["alice"].or(0))` compiles, runs, prints `90`. `for (entry in m) print(entry.key)` iterates in insertion order. Map operations are DoS-safe.
**Why this phase exists**: Map alone is milestone-sized work. Splitting it ensures the Swiss Tables / SipHash / perfect-hash codegen each get focused review attention.
**Current-state anchors**:
- P4a runtime infrastructure (`ynz-runtime`)
- P3c `perfect_hash_candidates` salsa query
**Files (expected scope)**:
- `crates/ynz-codegen/src/map_codegen.rs` (NEW) — map operation lowering + perfect-hash codegen
- `crates/ynz-codegen/src/perfect_hash.rs` (NEW) — CHM92 or BBHash compile-time codegen
- `crates/ynz-runtime/src/map.c` (NEW — or `map.rs`) — Swiss Tables impl
- `crates/ynz-runtime/src/siphash.c` (NEW — or `siphash.rs`)
- `crates/ynz-runtime/tests/map_fuzz.rs` (NEW) — property-based fuzz vs BTreeMap oracle
- `crates/ynz-runtime/tests/map_hashdos.rs` (NEW) — adversarial keys confirming DoS resistance
- `crates/ynz-codegen/tests/snapshots.rs` — IR-snapshot tests
- `crates/ynz-driver/tests/fixtures/m5_map_*.ynz` — runnable end-to-end tests
**Deviation rule**: NO xxhash3 (deferred); NO identity-hash (deferred). M5 ships SipHash + perfect-hash only.
**Steps**:
1. Implement SipHash-2-4 in C (or hand-rolled Rust): standard algorithm, ~200 lines. Reference: https://131002.net/siphash/siphash.pdf. Property test against a reference impl (cargo `siphasher` crate for typeck-side; pure for runtime).
2. Implement Swiss Tables in C (or hand-rolled Rust): open-addressing + SIMD metadata scan. Reference: hashbrown (Rust port of Abseil). ~800 lines. Property test against `BTreeMap` reference oracle for correctness.
3. Implement `ynz_siphash_init` runtime hook: reads 16 bytes from `/dev/urandom` (Linux) / `getentropy(3)` (macOS) at startup. Driver emits a call to this in `main`'s prologue.
4. Implement `ynz_map_*` runtime symbols: `ynz_map_new(key_size, value_size, key_eq_fn) -> *Map`; `ynz_map_get(map, key) -> MaybeV`; `ynz_map_set(map, key, value) -> nothing`; `ynz_map_remove(map, key) -> nothing`; `ynz_map_count`; `ynz_map_drop`; `ynz_map_iter_init`; `ynz_map_iter_next`. Insertion order tracked via parallel `Vec<index>`.
5. Implement perfect-hash codegen (`perfect_hash.rs`): for each entry in `perfect_hash_candidates`, run CHM92 (or BBHash) at compile time to derive a collision-free hash function for the literal's keys. Emit a specialized lookup function into the binary; map operations for this specific literal use the perfect-hash function (no SipHash, no probing).
6. Wire map literal codegen: if the literal is a perfect-hash candidate AND all-static-key, use the perfect-hash path; otherwise use the Swiss Tables path.
7. Lower map operations: `m.get(k)` → `call @ynz_map_get(...)`; `m.set(k, v)` → `call @ynz_map_set(...)`; etc.
8. Lower map iteration: `for (entry in m)` → `ynz_map_iter_init` + loop calling `ynz_map_iter_next` + bind `entry.key` / `entry.value`.
9. Property fuzz tests (`map_fuzz.rs`): 10k random operation sequences (insert/delete/get/iterate) compared against `BTreeMap<K, V>` reference oracle. Different seeds each CI run. CI fails on any divergence.
10. HashDoS adversarial test (`map_hashdos.rs`): use a known-colliding key set (from public SipHash research) and confirm the Swiss Tables runtime handles it within O(n log n), not O(n²).
11. Runnable end-to-end fixtures: `m5_map_basic.ynz`, `m5_map_iteration.ynz`, `m5_map_perfect_hash.ynz`, `m5_map_complex_keys.ynz`.
**Acceptance criteria**:
- [ ] `m5_map_basic.ynz` (`let m: map<string, int> = { "alice": 90 }; print(m["alice"].or(0))`) → prints `90`
- [ ] `m5_map_iteration.ynz` iterates in insertion order → expected sequence
- [ ] `m5_map_perfect_hash.ynz` (all-static-key literal) → IR contains the perfect-hash function, NOT a SipHash call
- [ ] `m5_map_complex_keys.ynz` (runtime keys) → IR contains the SipHash + Swiss Tables path
- [ ] `map_fuzz.rs` 10k random ops match `BTreeMap` reference oracle
- [ ] `map_hashdos.rs` adversarial keys complete within expected bound
- [ ] All M4 codegen tests still pass
- [ ] valgrind clean on the M5 map fixture suite
**Quality gate**:
- [ ] SipHash key initialized BEFORE any map operation (no zero-key window)
- [ ] No `unwrap()` in runtime or codegen paths
- [ ] Iteration order = insertion order (property test confirms)
- [ ] Perfect-hash codegen handles edge cases: 0 keys, 1 key, 1024 keys; bails to Swiss Tables for >1024
**Verification**: `cargo test --workspace` green; `cargo test -p ynz-runtime` green (fuzz + hashdos); runnable fixtures produce expected stdout.

---

### Phase 5: Driver integration + M5 fixture suite + examples/pirates-roster + examples/primantis-orders

**PR scope**: Wire all M5 phases together in the driver. Add the M5 fixture suite (`crates/ynz-driver/tests/fixtures/m5_*.ynz`). Extend `examples/pirates-roster/entrypoint.ynz` with M5 features in context (generic function, generic shape, all three collections, maybe, bracket access, for-iteration). Create `examples/primantis-orders/m5_errors.ynz` exercising every M5 compile-error class. Ensure the M5 success-criteria program (from the Context section above) runs end-to-end.
**Branch**: `feat/m5-driver-fixtures-examples`
**Flag**: N/A
**Est. lines**: ~800 (mostly .ynz files + integration test wiring)
**Ships via**: `/pr`
**Objective**: After P5, `cargo test --workspace` runs every M5 fixture through the full compile + run pipeline. `examples/pirates-roster/entrypoint.ynz` demonstrates every v0.1 feature through M5 in one growing program. `examples/primantis-orders/m5_errors.ynz` triggers every M5-class diagnostic in one file. Both files are reviewed by Patrick (hands-on UX validation per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery`).
**Why this phase exists**: Per `### Demo & Error Gallery` invariant — every M5+ phase that adds executable surface MUST extend these two files. This phase consolidates all of M5's contributions and the success-criteria program in one verifiable bundle.
**Current-state anchors**:
- `crates/ynz-driver/tests/fixtures/m4_*.ynz` — existing M4 fixture pattern
- `examples/pirates-roster/entrypoint.ynz` — already has M1-M4 demonstrations
- `examples/primantis-orders/m4_errors.ynz` — already has M4 error triggers
**Files (expected scope)**:
- `crates/ynz-driver/tests/fixtures/m5_*.ynz` — 12+ fixtures covering every M5 feature
- `crates/ynz-driver/tests/integration.rs` — wire fixtures into the test harness
- `examples/pirates-roster/entrypoint.ynz` — extend with M5 features
- `examples/primantis-orders/m5_errors.ynz` — CREATE; trigger every M5 compile-error class
- `examples/primantis-orders/Cargo.toml` (or analog) — wire m5_errors into the gallery
**Deviation rule**: NO new compiler logic in this phase. If a fixture exposes a bug, the bug fix goes in a separate PR (back to whichever phase owns it).
**Steps**:
1. Create the M5 success-criteria fixture (the full program from the Context section above).
2. Create per-feature fixtures: identity function, generic shape, generic-shape with constraint, array operations, fixed operations, map operations, maybe operations, for-loop iteration over each collection type, auto-promoted-array, bracket access on each collection, index-assign on each collection, none-literal in various contexts.
2a. **Cross-impl consistency fixtures (Tier A required)**:
    - `m5_bracket_vs_get.ynz` — same logical program written twice: once with `arr[i]`, once with `arr.get(i)`. Assert byte-identical stdout (and ideally IR-snapshot equality for the inner call sites).
    - `m5_auto_promoted_vs_declared_fixed.ynz` — pair: program A declares `let arr: array<int> = [1,2,3]` (never grown), program B declares `let arr: fixed<int> = [1,2,3]`. Assert byte-identical stdout AND drop-behavior (valgrind-clean both).
    - `m5_perfect_hash_vs_siphash.ynz` — pair: program A uses `let m: map<string, int> = { "alice": 90, "bob": 85 }` (all-static keys → perfect-hash); program B uses `let m: map<string, int> = {}; m.set("alice", 90); m.set("bob", 85)` (runtime keys → SipHash). Assert byte-identical stdout for the same lookup operations.
2b. **Adversarial fixtures (per reviewer concerns)**:
    - `m5_maybe_dynamic.ynz` — exercises `maybe<dynamic Damageable>` lowering (data-pointer null-encoding).
    - `m5_array_of_maybe.ynz` — exercises `array<maybe<T>>` round-trip: `.add(none)` / `.add(some(v))` / `arr[i]` recovery.
    - `m5_perfect_hash_one_key.ynz` — N=1 perfect-hash edge case; IR-snapshot the fast path.
    - `m5_perfect_hash_pathological.ynz` — uses a published CHM92-pathological key set; asserts the silent fallback to Swiss Tables (IR contains SipHash call, not perfect-hash function).
    - `m5_generic_inference_with_none.ynz` — `identity(none)` produces the expected compile error with both suggested fixes.
    - `m5_drop_order_array_of_maps.ynz` — `let arr: array<map<string, int>> = [...]`; assert IR emits `ynz_map_drop` per element before `ynz_array_drop`.
    - `m5_top_level_map_rejected.ynz` — top-level `let m: map<string, int> = { ... }` produces compile error pointing to M8.
    - `m5_cycle_leak.ynz` — `shape Node<T> { value: T, next: maybe<Node<T>> }` followed by cycle creation through mutation; documents the intentional v0.1 leak; comment cites the locked decision.
    - `m5_maybe_dynamic_null_data.ynz` — `maybe<dynamic Foo>` with `none`; assert IR codegen checks the data-slot for null BEFORE reading the vtable slot. Prevents silent UB if codegen reads vtable before data-null check.
    - `m5_map_mutation_during_iter.ynz` — `for (entry in m) { m.set("new", v) }`. Locked behavior: COMPILE ERROR with three-part diagnostic citing the `lend self` on `.set()` colliding with the iteration's outstanding share-borrow. (Borrow check from M4 already handles this — fixture confirms it works through the new map iteration machinery.)
    - `m5_generic_through_dynamic.ynz` — `findMax(items: array<dynamic Comparable>)`. Locked behavior: monomorphizes ONCE for `dynamic Comparable` as the element type (dynamic IS the concrete type at the boundary). Fixture asserts a single monomorph entry, not one per concrete Comparable.
3. Wire each fixture into `crates/ynz-driver/tests/integration.rs` — each runs `ynz run` and asserts stdout + exit code.
4. Extend `examples/pirates-roster/entrypoint.ynz` with M5 features in context. Use realistic names; show the feature doing real work (not `print(identity(5))` but a small useful pattern).
5. Create `examples/primantis-orders/m5_errors.ynz`. One file demonstrating EVERY M5 compile error class (compiler multi-errors up to 50 per compile). Each trigger has a `// WHY:` comment naming the diagnostic class.
6. Add insta snapshot for `examples/primantis-orders/m5_errors.ynz` stderr (asserts the diagnostic output matches the committed golden).
7. Manual review pass: patrick reads `examples/pirates-roster/entrypoint.ynz` and `examples/primantis-orders/m5_errors.ynz` and confirms the UX feels right. If diagnostic wording feels off, fix it (small enough PR scope to absorb).
**Acceptance criteria**:
- [ ] M5 success-criteria fixture compiles, runs, produces expected stdout, exits 0
- [ ] 12+ per-feature fixtures pass
- [ ] `examples/pirates-roster/entrypoint.ynz` demonstrates every M5 feature in context
- [ ] `examples/primantis-orders/m5_errors.ynz` triggers every M5 error class
- [ ] stderr snapshot for `examples/primantis-orders/m5_errors.ynz` matches the committed golden
- [ ] All M1-M4 fixtures still pass
- [ ] Patrick has read and signed off on both examples files
**Quality gate**:
- [ ] No banned-jargon in any committed diagnostic output snapshot
- [ ] Each `// WHY:` comment in m5_errors.ynz names the diagnostic class
- [ ] Examples files use realistic names (no `foo`/`bar`/`baz`)
- [ ] Examples files step-by-step (no chaining)
**Verification**: `cargo test --workspace` green; `./target/debug/ynz run examples/pirates-roster/entrypoint.ynz` produces expected stdout; `./target/debug/ynz build examples/primantis-orders/m5_errors.ynz` produces expected stderr.

---

### Phase 6: Verification sweep + tag `v0.1.0-m5`

**PR scope**: Run the full Step 10 verification sweep (TODO sweep, todos.md cross-check, shortcut detection, quality-checklist verification). Confirm every M5 invariant. Bump `Cargo.toml` workspace version to `0.1.0-m5`. Generate CHANGELOG section. Tag `v0.1.0-m5`.
**Branch**: `chore/m5-verification`
**Flag**: N/A
**Est. lines**: ~150 (CHANGELOG entry + Cargo.toml bumps)
**Ships via**: `/release` (cuts the tag and pushes per `release.md` skill)
**Objective**: M5 is shipped, tagged, and the master plan's M6 milestone paragraph is ready for `/plan M6`.
**Why this phase exists**: Verification is the final ratchet — without it, M5 can ship with orphaned TODO comments, stale todos.md items, or quality-checklist gaps. Same pattern M1-M4 used.
**Current-state anchors**:
- `Cargo.toml` workspace `version = "0.1.0-m4"` (after M4 ship)
- `.claude/plans/active/v0-1-compiler.md` `M5 Status: in planning` (this plan exists)
**Files (expected scope)**:
- `Cargo.toml` (workspace + per-crate versions)
- `CHANGELOG.md` (M5 section)
- `.claude/plans/active/v0-1-compiler.md` (update M5 milestone status → COMPLETE)
- `.claude/state.md` (M5 active decisions, tag)
- `.claude/todos.md` (remove M5-completed items; surface M6 catch-up obligations)
- `.claude/plans/done/m5-generics.md` (this plan, moved from active)
**Deviation rule**: NO new compiler logic. If a verification check fails, fix the underlying issue in a separate PR back to the owning phase.
**Steps**:
1. **TODO sweep**: `grep -rn "TODO\|FIXME\|HACK\|XXX\|TEMP\|PLACEHOLDER" crates/ examples/` — confirm no orphaned items. Any found get either resolved or moved to `.claude/todos.md`.
2. **REPLACE-AT M7 sweep**: confirm every `for (x in collection)` special-case site in typeck and codegen has a `REPLACE-AT M7` comment; confirm the M7 catch-up obligations section of the master plan lists this work.
3. **Auto-promotion analysis verification** (per `.claude/rules/plan-invariants.md` `### Performance`): confirm M5 covers every auto-promotion candidate; document each in the invariants section (already done in this plan's Invariants section — verify).
4. **Jargon audit**: `cargo test -p ynz-diagnostics --test jargon_audit` green; no banned word in any user-facing diagnostic.
5. **Quality checklist run**: every item below verified with evidence (file path, test name, output).
6. **Bump versions**: workspace + per-crate to `0.1.0-m5`.
7. **CHANGELOG**: M5 section listing every shipped feature with line citations.
8. **Move plan**: `git mv .claude/plans/active/m5-generics.md .claude/plans/done/`.
9. **Update master plan**: mark M5 status COMPLETE with tag + test count + commit hash.
10. **Tag**: `git tag v0.1.0-m5` per `/release` skill convention.
**Acceptance criteria**:
- [ ] All M5 fixtures pass
- [ ] Zero orphaned TODO/FIXME comments in shipped code
- [ ] All `REPLACE-AT M7` sites accounted for
- [ ] Jargon audit green
- [ ] valgrind clean on M5 fixture suite (or documented platform exception)
- [ ] Test count documented in state.md (expected: ~400-450 tests, up from M4's 316)
- [ ] CHANGELOG entry references this plan + every M5 PR
- [ ] Cargo.toml workspace version = `0.1.0-m5`
- [ ] Master plan M5 paragraph status = COMPLETE
- [ ] Plan moved to `done/`
- [ ] Tag `v0.1.0-m5` pushed
**Quality gate**:
- [ ] Patrick has reviewed the M5 examples files and signed off on UX
- [ ] No `// REPLACE-AT` markers outside the M7 catch-up scope
- [ ] No `unwrap()` introduced anywhere in M5 phases
- [ ] All M4 invariants still hold (run M4 fixtures, confirm no regressions)
**Verification**: `cargo test --workspace` green; `./target/debug/ynz --version` reports `0.1.0-m5`; `git tag --list 'v0.1.0-m5'` returns the tag.

---

## Quality Checklist (verify at completion of M5)

- [ ] All inputs validated at compiler/parser boundaries (every new AST variant has a parse-error recovery path)
- [ ] Generic-call disambiguation has a documented adversarial-input regression test (no exponential blowup)
- [ ] No N+1 or unbounded queries in the salsa graph (incremental-compile benchmark gate)
- [ ] No banned-jargon in any user-facing diagnostic (jargon audit CI green)
- [ ] Map runtime is DoS-safe by default (SipHash + per-process random key; HashDoS adversarial test passes)
- [ ] Property-based fuzz test confirms map runtime correctness against `BTreeMap` reference oracle (10k operations × multiple seeds)
- [ ] Auto-promotion `array<T>` → `fixed<T>` correctness: typeck refuses promotion when any uncertain path could grow the array (negative + positive fixtures)
- [ ] LLVM `readonly`/`noalias` attributes carry over to monomorphized functions per M4 invariants
- [ ] `maybe<T>` `.value` enforcement is flow-sensitive (compile error without prior `.exists()` check or `.or()` fallback)
- [ ] Map iteration order = insertion order (property test confirms; cross-platform deterministic)
- [ ] `fixed<T>` literal-OOB caught at compile time when size + index are both known
- [ ] Runtime bounds-check on `array<T>.set()` panics with descriptive message (length + offending index)
- [ ] All M5 diagnostics follow WHAT/WHAT-INSTEAD/WHY three-part format
- [ ] Existing M4 fixtures still pass (no regressions)
- [ ] Types are complete (no `any` equivalent — Rust unwrap/expect minimized; `Option`/`Result` plumbed properly)
- [ ] Follows existing codebase conventions (M4 pattern for new phases, salsa queries, fixture structure)

---

## Invariants This Milestone Must Preserve

> Required by `.claude/rules/plan-invariants.md`. Each subsection lists testable assertions, not vague aspirations.

### Safety

- **`maybe<T>.value` requires flow-sensitive proof of `.exists()`** — accessing `.value` without prior `.exists()` check or `.or()` fallback is a compile error. M5 P3b enforces; negative fixture `m5_maybe_value_unchecked.ynz` produces a three-part error suggesting both alternatives.
- **`none` literal cannot bind to a non-`maybe` type** — `let x: int = none` is a compile error. M5 P3b enforces; negative fixture `m5_none_wrong_type.ynz`.
- **Bracket access on a `shape` is rejected** — `player["name"]` is a compile error pointing to dot access. M5 P3b enforces (extends M4 P3a's diagnostic for clarity); negative fixture `m5_bracket_on_shape.ynz`.
- **`fixed<T, N>` literal-out-of-bounds caught at compile time** — `let f: fixed<int> = [1, 2, 3]; f[5] = 10` is a compile error citing the bound and the offending index. M5 P3b enforces; negative fixture `m5_fixed_literal_oob.ynz`.
- **Map literal duplicate keys are rejected** — `{ "alice": 1, "alice": 2 }` is a compile error naming both spans. M5 P3c enforces; negative fixture `m5_map_duplicate_key.ynz`.
- **Generic constraints are enforced at every instantiation site** — calling `sort<T follows Comparable>(items)` with `items: array<NonComparable>` is a compile error citing the contract and the missing methods. M5 P3a enforces; negative fixture `m5_constraint_violation.ynz`.
- **Generic type-parameter inference failure produces a teaching error** — `let x = identity()` (no args, no annotation) produces a three-part error suggesting both `identity(5)` and `let x: int = identity<int>()`. M5 P3a enforces; negative fixture `m5_inference_failed.ynz`.
- **`for (x in collection)` over a non-built-in type is rejected with an M7 pointer** — using a user-defined shape as the iter source produces a three-part error pointing to `Iterable<T>` in M7. M5 P3b enforces; negative fixture `m5_for_over_shape.ynz`.
- **Map iteration order = insertion order** — programmatic test confirms (`m5_map_iteration_order.ynz` asserts the sequence matches insertion).
- **All M4 ownership invariants carry forward to monomorphized functions** — `const` bindings still emit `readonly`; `lend` parameters still emit `noalias`; consume-tracking still works through generic instantiations. P4a IR-snapshot tests assert.
- **Self-referential generic shapes through `maybe<Self>` or `array<Self>` work; direct self-fields are rejected** — `shape Node<T> { value: T, next: maybe<Node<T>> }` works; `shape BadNode<T> { value: T, next: Node<T> }` is a compile error. M5 P3a enforces.
- **`maybe<maybe<T>>` is rejected at typeck** — nested maybe is a code smell (almost always means "flatten me"); compile error with three-part diagnostic suggesting `maybe<T>` directly. M5 P3b enforces; negative fixture `m5_nested_maybe_rejected.ynz`.
- **Top-level `let` bindings of map type are rejected at typeck** — `ynz_siphash_init` runs from `main`'s prologue only; top-level map literals would need pre-main init that M5 doesn't ship. M5 P3c enforces; negative fixture `m5_top_level_map_rejected.ynz` with three-part error pointing to M8.
- **Map iteration produces share-borrowed `MapEntry<K, V>` values** — `.key`/`.value` are share-borrowed from map storage; `.lend`/`.give` on them produces a compile error per M4 ownership rules. Positive + negative fixtures cover this.
- **Cycle creation through `maybe<Self>` mutation is a documented v0.1 limitation** — `shape Node<T> { value: T, next: maybe<Node<T>> }` allows cycle creation at runtime; the v0.1 borrow checker does NOT detect cycles, so a user-created cycle leaks both nodes. Documented in design/maybe.md; intentional v0.1 behavior, NOT a bug. Borrow-checker cycle-detection is v0.2+ work. The leak is documented in fixture `m5_cycle_leak.ynz`.

### Performance

The codegen contract:

- **Monomorphization at codegen time** — each `<T = ConcreteType>` instantiation produces a fully specialized LLVM function/struct. IR-snapshot tests confirm: `identity<int>` and `identity<string>` are distinct functions with distinct mangled names.
- **De-dupe identical instantiations** — `identity<int>` used in 50 places produces ONE specialized function. IR-snapshot confirms call sites all reference the same function.
- **Generic dispatch produces direct calls** — `pair(5, 10)` lowers to `call @pair_int_int(...)`, NOT a runtime lookup. IR-snapshot asserts.
- **`array<T>` 1.5× growth** — measured against `crates/ynz-runtime/tests/array_growth.rs` with a sequence of `push` calls confirming cap = 8 → 12 → 18 → 27 → ... .
- **`fixed<T>` is stack-allocated** — IR-snapshot confirms `alloca [N x T]`, no `call @ynz_alloc`.
- **`map<K, V>` default is Swiss Tables + SipHash-2-4** — IR-snapshot asserts the SipHash call in a runtime-keyed map's `.get()` lowering.
- **`map<K, V>` literal with all-static-keys uses perfect-hash codegen** — IR-snapshot asserts the perfect-hash function in the literal's `.get()` lowering (NO SipHash call).
- **LLVM `readonly`/`noalias` attributes carry over to monomorphized functions** — IR-snapshot asserts the attributes are present on each instantiation per M4 invariant.
- **Bracket sugar lowers to direct `.get()`/`.set()` IR with no extra indirection** — IR-snapshot for `arr[i]` matches IR-snapshot for `arr.get(i)`.
- **Map iteration overhead = one pointer per entry for insertion-order tracking** — documented in the runtime, asserted by allocation-tracking test.
- **Drop emission for `array<T>`** — IR-snapshot confirms per-element drop loop followed by `ynz_free`. valgrind clean on map+array-heavy fixtures.

**Auto-promotion analysis** (mandatory per `.claude/rules/auto-promotion.md`):

- **`array<T>` → `fixed<T>` promotion** (canonical example from `design/collections.md`):
  - **Codegen auto-promotion**: M5 P4a emits the `fixed<T, N>` codegen for proven-never-grown bindings. Always-on. Tested in `m5_auto_promotion.ynz`.
  - **Muted IDE hint**: NOT in M5 — IDE surfaces land in v0.2 LSP per `design/ide-hints.md`. M5 produces the data (PromotionReport salsa query); LSP wraps it later.
  - **Tier 3 lint suggestion (`prefer-fixed-when-immutable`)**: NOT in M5 — lint tier lands in v0.4 per `design/linting.md`. M5 records what the lint would have said in a side-table for v0.4 to pick up.
  - Why split: per `.claude/rules/auto-promotion.md` "Two Surfaces for the Same Decision" — codegen surface is unblocked now; the teaching surfaces wait for their infrastructure. This is NOT a duct-tape deferral; it's the documented split per `inference.md`.
- **`let` → `const` promotion**: NOT in M5 (carried by v0.4 lint and the M4 P4 `readonly`-from-const-binding codegen which already ships).
- **Sort stability auto-pick by element type**: NOT in M5 — `.sort()` ships in M5 with a stable-only implementation; the type-based auto-pick (`design/collections.md` Sort section) is deferred until the stability is observable enough to care.
- **Map literal pre-size at compile time**: M5 P3c records the entry count; M5 P4b emits Swiss Tables with `ceil(count / load_factor)` initial buckets. Codegen auto-promotion only (no source-level form). IR-snapshot confirms.
- **Field auto-reorder for shape layout**: ALREADY shipped in M4 P4. M5 carries forward unchanged.
- **No new auto-promotion candidates introduced by M5's other features** (`maybe<T>` lowering choices, generic instantiation dedup, monomorphization). Each was evaluated; none has a stricter-form-the-compiler-could-have-picked the user could express differently.

### Teaching

M5 adds approximately 35-40 new diagnostic classes. Each invariant below is testable:

- **Every M5 diagnostic follows WHAT/WHAT-INSTEAD/WHY three-part format** — enforced by the `Diagnostic` constructor's three-non-empty-field assertion (M1+ carry-forward).
- **Banned jargon is absent** — `crates/ynz-diagnostics/tests/jargon_audit.rs` confirms `monomorphize`/`polymorphic`/`covariant`/`contravariant`/`infer`/`inference`/etc. don't appear in any user-facing diagnostic. M5 wording uses plain English: "specialized for each type used" instead of "monomorphized", "works with any type that follows the contract" instead of "polymorphic".
- **Generic-call type-inference failure names the function and the parameters that couldn't be worked out** — `identity()` produces "Cannot work out the type parameter T for function `identity` ..." with concrete suggested alternatives.
- **Constraint violation names the contract AND the concrete-type AND the missing methods** — `sort(players)` where Player doesn't follow Comparable produces "Type Player does not follow contract Comparable. To make Player sortable, add `follows Comparable` to its declaration AND implement: compare(share self, share other: Self) -> int".
- **Duplicate map-key error names BOTH key spans** (using Ariadne's related-span feature) — `m5_map_duplicate_key.ynz`'s stderr snapshot asserts.
- **Bracket-on-shape error suggests dot access** — `player["name"]` produces "Bracket access is for collections (array, fixed, map, string). Use dot access on shapes: player.name".
- **`maybe<T>.value` unchecked error suggests both `.exists()` check and `.or(default)`** — `m5_maybe_value_unchecked.ynz`'s stderr snapshot asserts.
- **`none` type-inference failure suggests annotating the binding** — `let x = none` produces "Cannot work out which type `none` should be here. `none` is the absent value of `maybe<T>` for some T — annotate the binding: `let x: maybe<int> = none`".
- **`fixed<T, N>` literal-OOB error names the bound AND the offending index** — `f[5]` on `fixed<int>` of size 3 produces "Index 5 out of bounds for `fixed<int>` of length 3. Valid indices are 0..2".
- **Runtime `array<T>.set()` bounds-check panic message is descriptive** — `ynz_panic` payload includes "index X out of bounds for array<T> of length N. `.set()` replaces existing elements only; use `.add()` to append.".
- **Generic-call-vs-comparison parse error is teaching** — `foo<x>(y)` where x isn't a type produces a three-part error explaining the ambiguity and suggesting both forms.
- **Generic-type-nesting-depth-exceeded error suggests breaking the cycle with `maybe<T>` or `array<T>`** — pathological input produces a teaching error pointing at the cyclic shape declaration.
- **For-loop-over-shape error points to M7** — `for (x in player)` where player is a shape produces "Iteration over user-defined types arrives in M7 with the `Iterable<T>` protocol. For now, you can iterate built-in `array<T>`, `fixed<T>`, and `map<K, V>`."
- **IDE muted-hint surfaces (auto-promotion, hash tier, generic instantiation)** — M5 does NOT ship; v0.2 LSP does (per `design/ide-hints.md`). Cross-reference recorded.

### Runtime Dependencies

Per `### Kernel-Mode Behavior` below + `design/future/no-runtime-mode.md`, every M5 feature declares its runtime requirements:

- **Generic compilation (monomorphization)**: no runtime dependency — compile-time only.
- **Generic shape instantiation**: no runtime dependency beyond what the concrete fields require.
- **Constraint checking (`follows`)**: no runtime dependency — compile-time only.
- **`maybe<T>` semantics**: no runtime dependency — compile-time (tagged-union or null-pointer encoding lowered to LLVM types).
- **`none` literal**: no runtime dependency — compile-time constant.
- **`.exists()` / `.value` / `.or()`**: no runtime dependency — compile-time-resolved per the tagged-union encoding (just LLVM extract/compare/select).
- **`fixed<T, N>` storage**: no runtime dependency — `alloca`.
- **`fixed<T>.set` non-literal bounds check**: depends on `ynz_panic` (M2 runtime symbol).
- **`array<T>` storage and growth**: depends on `ynz_alloc` / `ynz_free` (M4 runtime; no NEW symbol). Growth at 1.5× is a runtime-side implementation detail.
- **`array<T>.set` bounds check + panic**: depends on `ynz_panic`.
- **`array<T>` drop**: depends on `ynz_free` (M4 carry-forward) + per-element drop loop emitted by codegen.
- **`map<K, V>` storage**: depends on `ynz_alloc` / `ynz_free` PLUS a NEW Swiss Tables runtime symbol set (`ynz_map_new`, `ynz_map_get`, `ynz_map_set`, `ynz_map_remove`, `ynz_map_count`, `ynz_map_drop`, `ynz_map_iter_init`, `ynz_map_iter_next`).
- **`map<K, V>` hashing**: depends on a NEW `ynz_siphash_init` runtime symbol (initializes per-process random key from OS entropy) AND `ynz_siphash` (the actual hash function).
- **Perfect-hash codegen**: NO runtime dependency — the perfect-hash function is compile-time-generated and emitted directly into the binary. The map operations for that specific literal bypass the SipHash runtime.
- **Map iteration**: depends on the iterator state in the map runtime (no new symbol; part of `ynz_map_iter_*`).
- **Bracket sugar lowering**: no new runtime dependency — desugars to existing `.get()`/`.set()` runtime symbols.
- **For-loop over built-in collections**: no new runtime dependency — desugars to the collection's iteration symbol.
- **Auto-promotion `array<T>` → `fixed<T>`**: REMOVES a runtime dependency (the binding no longer uses `ynz_alloc`/`ynz_free`).

### Kernel-Mode Behavior

For each M5 runtime dependency above, the `--kernel` mode (v0.3+) behavior is locked:

- **Compile-time-only features** (generics machinery, constraint check, `maybe<T>` semantics, `none`, monomorphization, perfect-hash codegen, auto-promotion analysis, bracket-sugar desugar, for-loop special-case, `fixed<T>` alloca): **always work in `--kernel` mode**. No compile error; no plug-in API required.
- **`fixed<T>` non-literal bounds-check panic via `ynz_panic`**: **always works in `--kernel` mode** — `ynz_panic` is a kernel-compatible symbol per M2's design.
- **`array<T>` storage and growth via `ynz_alloc`/`ynz_free`**: **COMPILE ERROR in `--kernel` mode** unless the user provides a plug-in allocator via `... .in(myKernelAllocator)` syntax (v0.3+). Same as M4 heap-shape allocation. Error message: WHAT (this `array<T>` requires heap allocation) / WHAT-INSTEAD (declare a custom allocator and route this allocation through it, OR use `fixed<T>` if the size is known) / WHY (kernel modules can't depend on libc malloc). Pointer to `design/future/no-runtime-mode.md`.
- **`map<K, V>` storage**: **COMPILE ERROR in `--kernel` mode** unless plug-in allocator (same as `array<T>`).
- **`map<K, V>` SipHash initialization (`ynz_siphash_init`)**: depends on OS entropy. **In `--kernel` mode**, the kernel must provide an entropy source the user routes via a plug-in (v0.3+ design TBD; for M5 the kernel-mode behavior is "COMPILE ERROR unless user provides hash randomness source"). Error message names the requirement and points to `design/future/no-runtime-mode.md`.
- **Perfect-hash codegen**: **always works in `--kernel` mode** — no runtime dependency.
- **Map iteration**: same dependency as map storage — same `--kernel` rules.
- **Bounds-check panic (`ynz_panic`)**: **always works in `--kernel` mode**.
- **`array<T>` → `fixed<T>` auto-promotion**: **always works in `--kernel` mode** — the promoted binding has no runtime allocation, so `--kernel` is unblocked for any binding the auto-promotion proved safe.

**Forward declaration to v0.3 plug-in allocator API.** `... .in(myKernelAllocator)` syntax and the `Allocator` plug-in shape are v0.3 deliverables. M5 does NOT implement them; the kernel-mode compile error for `array<T>`/`map<K, V>` says "kernel-mode disabled" and points to `design/future/no-runtime-mode.md`.

### Demo & Error Gallery

- **`examples/pirates-roster/entrypoint.ynz` MUST be extended in P5** to demonstrate every M5 feature in context. Each feature should appear in a small but realistic snippet (not `print(identity(5))` alone — show a generic function used in actual computation; show a map carrying real data; show a for-loop doing real work).
- **`examples/primantis-orders/m5_errors.ynz` MUST be created in P5** with intentional triggers for every M5 compile-error class. Each trigger has a `// WHY:` comment naming the diagnostic class. One file produces ALL the M5 diagnostics in a single compile (per `design/compiler-errors.md` 50-error cap, well within bound).
- **Both files get insta stdout/stderr snapshots** committed to the repo. Updating these snapshots requires a `// test-ratchet: <reason>` marker.
- **Patrick must read and sign off on both files before P5 merges** — this is the hands-on UX validation that automated tests can't replace.

---

## Out-of-Scope For This Plan (M5 guardrails)

Restated here as the bottom-line guardrail; redundant with "What M5 explicitly is NOT" above for sanity-on-skim.

- `options` declarations — M6
- Union types `A | B` (NOT `maybe<T>` — ships in M5) — M6
- `if (x is Type)` pattern narrowing — M6
- Custom user-defined iterables via `follows Iterable<T>` — M7
- `errors` keyword + cascades — M7
- Full Unicode strings (`.byteAt`, `.graphemeAt`) — M7
- Modules / imports — M8
- Doc comments — M8
- `sensitive` modifier — M8
- Concurrency keyword parsing — M8
- Bignum `number<N>` for N > 34 — M8
- IDE muted-hint surfaces for auto-promotion, hash tier, generic instantiation — v0.2 LSP
- Tier 3 lint suggestions (`prefer-fixed-when-immutable`, etc.) — v0.4 lint tier
- xxhash3 fast opt-in for `map<K, V>` + its surface syntax — future (unbounded)
- Identity-hash for `map<int, V>` — future (unbounded)
- Sort element-type auto-pick (stable-on-shape / unstable-on-primitive) — future (M5 ships stable-only)
- `.update({...})` map bulk-update syntax — DEFERRED past M5 per locked decision (object-literal-as-update-payload design not load-bearing for v0.1; revisit v0.2+)
- Module-level / top-level `let` bindings of map type — REJECTED in M5 with M8 pointer (per locked decision; ynz_siphash_init runs from main's prologue only)
- `maybe<maybe<T>>` (nested) — REJECTED in M5 with compile error (per locked decision; almost always a code smell)
- Early-return narrowing for `.value` (`if (m.exists()) { return... } m.value`) — M6 (per locked flow-sensitive rules table)
- Plug-in allocator API for `--kernel` mode — v0.3 (`design/future/no-runtime-mode.md`)
- Higher-kinded types, lifetime parameters, associated types, general const generics — NEVER in v0.1

If you find yourself adding code that touches any item above, STOP and either re-plan this milestone or escalate the work to its proper milestone.

---

## M5 Catch-Up Obligations (recorded so downstream milestones don't orphan them)

- **`prefer-fixed-when-immutable` Tier 3 lint suggestion**: belongs to v0.4 (lint tier). M5 records what the lint would say in a salsa side-table (`PromotionReport`); v0.4 lint tier picks it up. Recorded.
- **Muted IDE hint for auto-promotion**: belongs to v0.2 LSP. M5 produces the data; LSP wraps it. Recorded.
- **`for (x in collection)` special-case** for `array<T>`, `fixed<T>`, AND `map<K, V>`: belongs to M7. Every `REPLACE-AT M7` site in typeck + codegen MUST be unwound by M7's `Iterable<T>` plan. Three distinct special-case sites in typeck + three in codegen — six total. Recorded.
- **`MapEntry<K, V>` synthesis**: revisited at M7. M7 decides whether `MapEntry<K, V>` remains a built-in or is replaced by a user-defined iteration-element shape via `Iterable<T>`'s element-type parameterization. M5 ships it built-in; recorded so M7 plan addresses it.
- **M3 `range()` typeck special-case**: M5 does NOT unify it with the new for-loop machinery — M5 keeps `range()` as its existing special-case AND adds new special-cases for `array<T>`/`fixed<T>`/`map<K,V>`. M7's `Iterable<T>` unification absorbs both. Recorded so M7 doesn't orphan the `range()` path.
- **`for (x in user_shape)` user-defined iteration**: requires M7's `Iterable<T>` protocol. M5 emits a teaching error pointing to M7. Recorded.
- **CHM92 perfect-hash fallback observability** (plan-reviewer Round 2 concern #1): the silent fallback to Swiss Tables when CHM92 exhausts 16 seeds is currently gated behind `YINZ_DEBUG_PERFECT_HASH=1` env var. Easy to forget the flag exists when investigating a perf regression six months out. **v0.2 carry-over**: when the LSP / `--verbose` compile-summary lands in v0.2, surface a per-compile count of `perfect_hash_fallbacks: N` so users see the fallback rate without env-var hunting. Recorded.
- **Monomorph nesting depth cap = 16** (plan-reviewer Round 2 concern #2): the cap was chosen by precedent — Rust's default `type_length_limit` is 1,048,576 (a size limit, not a depth limit) and `recursion_limit` defaults to 128, but those measure different things than Yinz's "nested generic instantiation depth." 16 was picked because (a) every real-world generic in TypeScript/Rust stdlib monomorphizes ≤ 5 levels deep, (b) anything > 16 is virtually always a cyclic type that the user should break with `maybe<T>` or `array<T>`, (c) the cap is a teaching surface — the diagnostic at depth-16 names the cycle. **Forward-revisit trigger**: if a real workload trips the cap (e.g., a stdlib type emerges that needs 20+ levels), revisit. Otherwise 16 stays. Recorded.
- **M7 catch-up cross-reference** (plan-reviewer Round 2 concern #3): when M7 is planned, its plan file MUST explicitly reference this milestone's REPLACE-AT M7 sites in its scope section — six sites total (three in typeck + three in codegen for the array/fixed/map for-loop special-cases) PLUS the M3 `range()` special-case PLUS the `MapEntry<K,V>` synthesis revisit. M7 unifies all of them under `Iterable<T>` in one PR. Without this cross-reference, the markers rot and become silent technical debt. Recorded as a pre-condition for M7 plan acceptance.
- **xxhash3 fast opt-in for `map<K, V>`**: surface syntax deliberately deferred. Picks up when a real workload demands it. Recorded.
- **Identity-hash for `map<int, V>`**: same as above. Recorded.
- **Sort element-type auto-pick**: M5 ships stable-only `.sort()`; the type-based auto-pick (per `design/collections.md` Sort section) waits until a real perf-sensitive workload demands it. Recorded.
- **`.update({...})` on maps with object-literal**: M5 ships a partial form (or defers entirely if the design is fragile). If deferred, recorded here.
- **Plug-in allocator API for `--kernel` mode for `array<T>` and `map<K, V>`**: v0.3+ per `design/future/no-runtime-mode.md`. M5's `ynz_alloc`/`ynz_free` + `ynz_map_*` are libc-wrappers; v0.3 swaps in the plug-in surface. Recorded.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each P0-P6 phase is one PR with one branch. The phase template above repeats this verbatim. Reviewer reviews per-phase, not per-commit. Phase 4a (codegen mono + array + fixed + maybe) is the largest single PR but stays one PR — splitting codegen across two PRs would require an intermediate state where typeck produces a MonomorphizationTable that codegen partially consumes, which is harder to review than one complete PR. Phase 4b (map) is also large but standalone (its own runtime files + tests).
- **Shadow main branches**: every phase branches from `main` (the trunk after the prior phase merged) and merges back to `main` via `/pr`. No long-lived feature branches; no rebase trains.
- **Building the engine before shipping value**: each phase delivers a coherent slice. P1 lexer/AST alone is reviewable + green; P2 parser alone produces ASTs with diagnostics; P3a typeck alone produces typed-AST with monomorph table; etc. The first end-to-end runnable M5 program (M5 success criteria) arrives at P5 end. P0 doc work is infra-first BUT it's explicitly called out — it fixes the source-of-truth before any code lands, NOT dressed up as value.
- **Hotfix that isn't**: M5 is not a hotfix. If a real hotfix happens during M5 development (e.g., M4 bug), it ships on its own branch with its own PR, NOT smuggled inside an M5 phase. P0 doc lockdown is explicitly scoped to M5-design-correctness, not to general hotfixing.
- **Abandoned branches**: if a phase PR isn't ready to merge, it gets closed and re-opened on a fresh branch; no zombie branches. Phase 6 verification confirms every M5 phase landed and no orphan branches exist via `git branch -a | grep feat/m5`.
- **Flag graveyards**: M5 ships behind no feature flag. The generics, maybe, collections, bracket sugar, for-loop iteration are all unconditionally enabled when this code merges. No `--enable-generics` flag; nothing to clean up later. The auto-promotion is also unconditional (codegen surface only; no opt-in/opt-out).

---

## Reviewer Disputes

### Round 1 (2026-05-17) — Plan-reviewer BLOCKED with 12 required fixes; planner addressed all 12 in-plan (no pushback)

Reviewer flagged 12 required fixes against the initial draft. All 12 addressed:

1. **`Type` enum variant count claim** — added explicit "verify-current-counts-first" step to P1; expected post-P1 counts documented (`Type` 13, `Expr` 15, `Stmt` 9 — pending P1 verification).
2. **`CallExpr.type_args` field placement** — moved from P2 step 7 into P1 (AST shape changes belong in P1).
3. **`maybe<T>` LLVM lowering decision criterion** — added a 9-row lowering decision table to FINAL LOCKED DECISIONS; each row has an IR-snapshot test name.
4. **`MapEntry<K, V>` synthesis hand-waved** — added full spec: name (MapEntry not Entry), visibility (user-namable, reserved built-in), ownership (share-borrowed from map), monomorphization (yes), construction (rejected — only iteration produces).
5. **`none` inference rule unspecified** — added a 9-row context-resolution table to FINAL LOCKED DECISIONS.
6. **Flow-sensitive `.value` enforcement spec gap** — added a 10-row rules table to FINAL LOCKED DECISIONS; M5 ships positive + negated-else + AND short-circuit; early-return narrowing punted to M6 with teaching error.
7. **Cycle creation through `maybe<Self>` mutation unspecified** — locked decision: v0.1 leaks (documented limitation, not duct tape); borrow-checker cycle-detection is v0.2+. Negative fixture documents the leak.
8. **Hash randomness init race for top-level maps** — locked decision: REJECT top-level map bindings in M5 with M8 pointer; ynz_siphash_init runs from main's prologue only.
9. **Perfect-hash CHM92 retry exhaustion** — locked decision: 16 retries with different seeds, then silent fallback to Swiss Tables. IR-snapshot documents the fallback.
10. **`.update({...})` at-code-time decision** — locked: DEFERRED past M5. Added to Out-of-Scope.
11. **`.value` field-not-method confusion** — clarified P3b step 3: `maybe<T>` has BOTH a virtual-field table AND a method table per dot-postfix.md.
12. **Cross-impl consistency tests for Tier A** — added three pair-fixtures to P5: bracket-vs-`.get()`, auto-promoted-array-vs-declared-fixed, perfect-hash-vs-SipHash.

Plus reviewer's "Suggested Adversarial Cases" added to P5 step 2b as 8 explicit fixtures.

No reviewer points pushed back on — all 12 were genuine spec gaps.

---
