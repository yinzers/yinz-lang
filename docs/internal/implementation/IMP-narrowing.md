---
name: "IMP-narrowing"
description: "Design decisions for Yinz's compile-time type narrowing — the flow-sensitive rules table governing when '.value' (maybe<T>) and 'is' (union) narrowing are granted across control flow."
tags:
  - "yinz-compiler"
created_at: "2026-05-18"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Narrowing — Design Decisions

User spec: [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md) (for `.value` narrowing), [`docs/reference/REF-unions.md`](../../reference/REF-unions.md) (for `is` narrowing)

---

## What narrowing is

Narrowing is compile-time tracking of which type a value has been proven to be inside a given block. After the compiler proves a fact about a binding (e.g., "this `maybe<T>` has a value" or "this `Shape` is a `Circle`"), it widens the binding's type inside the proven block to the narrower form — allowing safe access without explicit casts or runtime checks.

M6 extends M5's narrow subset of narrowing to the full rules table below.

---

## Flow-Sensitive `.value` and `is` Narrowing Rules

Full rules table — every form the compiler tracks in M6.

| Form | Narrowing granted? | Notes |
|---|---|---|
| `if (m.exists()) { m.value }` | YES — `m` is `T` (not `maybe<T>`) inside then-block | Bread-and-butter case. Shipped in M5. |
| `if (!m.exists()) { ... } else { m.value }` | YES — `m` is `T` inside else-block | Negative form, symmetric. Shipped in M5. |
| `if (m.exists() && other) { m.value }` | YES — `&&` propagates flag; both RHS and body see `m` narrowed | Shipped in M5. |
| `if (other && m.exists()) { m.value }` | YES — order doesn't matter for AND | Shipped in M5. |
| `if (m.exists() \|\| other) { m.value }` | **NO** — `\|\|` does NOT propagate single-branch narrowing | See "||  non-propagation" below. M6 adds the locked diagnostic. |
| `if (a.exists() && b.exists()) { a.value; b.value }` | YES — independent flags per binding | Each `maybe` binding gets its own narrowing fact. |
| `if (m.exists()) { return ... } m.value` | YES — early-return narrowing (M6) | After the `if` block whose then-branch always exits via a recognized exit form, `m` is proven to be `T` for the rest of the enclosing block. |
| `if (m.exists()) { panic("...") } m.value` | YES — `panic` is a recognized exit form | Same as `return`. |
| `if (m.exists()) { loop { ... } } m.value` | YES — infinite loop is a recognized exit form | Only `loop { }` with no `break` — the compiler proves no exit from the loop body. |
| `if (m.exists()) { someFn() } m.value` | **NO** — non-recognized exit | `someFn()` returning `nothing` does NOT prove exit. See "recognized-exit set" below. |
| `if (x is Foo) { x.bar }` (x: `Foo \| Bar`) | YES — `x` is narrowed to `Foo` inside then-block | M6 union narrowing. |
| `if (!(x is Foo)) { ... } else { x.bar }` (x: `Foo \| Bar`) | YES — `x` is narrowed to `Foo` inside else-block | Negative union narrowing. |
| `if (!(x is Foo)) { return } x.bar` (x: `Foo \| Bar`) | YES — early-return narrows to `Foo` after the if | Combines early-return with union narrowing. |
| `if (m.exists()) { m = newMaybe(); m.value }` | **NO** — reassignment invalidates the narrowing flag | Any assignment to `m` clears the fact. |
| `if (x is Foo) { x = other; x.bar }` (x: `Foo \| Bar`) | **NO** — reassignment invalidates | Same rule for union narrowing. |
| `if (m.exists()) { mutatingFn(m.lend); m.value }` | **NO** — `lend` call invalidates | Passing `m` as `lend self` may invalidate the field backing the exists check. Conservative: the compiler always invalidates on `lend`. |
| `if (m.exists()) { immutableFn(m.share); m.value }` | YES — `share` does NOT invalidate | Read-only access cannot change whether `m` has a value. |
| `if (m.exists()) { () => m.value }` | **NO** — closures don't carry narrowing flags | Forward-defensive. Closures ship v0.3+; this rule is documented here to prevent drift when closures land. |
| `if (x is Foo) { ... }` where `x: Foo` (not a union) | INFO diagnostic (non-blocking) — check is always true | Precursor to v0.4 Tier 3 lint `prefer-no-redundant-is`. Compilation succeeds. |
| `m.value` with no surrounding proof | **NO** — compile error | Base case. Suggests `.exists()` check or `.or(default)`. |

---

## Recognized-Exit Set for Early-Return Narrowing

**LOCKED in M6** — only these forms count as a recognized exit for early-return narrowing:

1. `return <expr>` — explicit return with a value
2. `return` — bare return (in a `nothing`-returning function)
3. `panic(msg)` — diverges; never returns
4. `loop { /* body with no `break` */ }` — infinite loop; never exits

**NOT in the recognized-exit set** (even if the user "knows" the function diverges):
- `someFn()` — any user function returning `nothing`; the typeck cannot prove it diverges
- `print(...)`, logging calls, any side-effect-only call
- `if (condition) { return }` — conditional return; the condition may be false

Rationale: the compiler can only prove divergence for constructs whose semantics are statically obvious. Adding a "function marked `#[diverges]`" annotation is a possible v0.2+ extension; for v0.1, the conservative set is the safe choice.

**Diagnostic when a user expects narrowing from a non-recognized exit**:

> **WHAT**: `.value` is not safe here. The `if` block on line N exits via `someFn()`, which the compiler cannot prove always diverges.
>
> **WHAT INSTEAD**: Use a recognized exit form in the if block:
>   1. `if (!m.exists()) { return }` — bare return
>   2. `if (!m.exists()) { panic("unreachable") }` — explicit panic
>   or restructure to: `if (m.exists()) { use(m.value) }`
>
> **WHY**: Only `return`, `panic`, and infinite loops are proven to never continue past the if-block. Other calls — even ones returning `nothing` — might return normally, leaving `m` still possibly-none in the tail.

---

## `||` Non-Propagation Diagnostic (LOCKED exact wording)

When a user writes `if (m.exists() || other) { m.value }`, the compiler rejects with:

> **WHAT**: `.value` is not safe here. The narrowing on `m` from `m.exists()` doesn't carry into this block because `||` only narrows when BOTH operands prove the same fact.
>
> **WHAT INSTEAD**: Pick one of:
>   1. `if (m.exists()) { ... use(m.value) ... }` — narrow before the body.
>   2. `let safe = m.or(defaultValue); use(safe)` — handle the `none` case with a fallback.
>   3. If the other condition is a separate concern, split into two `if`s.
>
> **WHY**: `||` is true when EITHER operand is true. If `other` is true and `m.exists()` is false, the body still runs but `m` is `none` — accessing `m.value` would crash. The compiler enforces this even when you "know" both will be true together, because the safety check costs nothing and the bug class costs a lot.

This is the most common narrowing rule users get wrong. The diagnostic is the teaching surface for it.

---

## `&&` Propagation — Both Directions

`if (cond1 && cond2) { body }`:
- Narrowings from `cond1` are visible when checking `cond2` (left-to-right short-circuit).
- Narrowings from both `cond1` and `cond2` are visible inside `body`.
- Order `(cond2 && cond1)` is symmetric — both facts still land in `body`.

`||` propagation: ONLY when both operands independently prove the SAME narrowing fact does the fact enter `body`. This is extremely rare in practice (e.g., `if (x is Foo || x is Foo)` — a tautology). In practice, `||` never propagates.

---

## Reassignment Invalidation

Any assignment `m = expr` inside a block clears all narrowing facts about `m`. The compiler treats reassignment conservatively — even if `expr` has type `maybe<T>` that is provably non-none, the existing fact is cleared and `m` goes back to requiring a new proof.

Rationale: tracking the provenance of every assignment would require alias analysis well beyond v0.1 scope. The conservative rule is safe and catches real bugs.

---

## Nested Early-Return — Scoping Rule

Only the OUTERMOST if-block's exit contributes a narrowing fact to the enclosing block's tail:

```ynz
function example(m: maybe<int>) -> int {
  if (!m.exists()) {               // outer if — recognized exit path
    if (someCondition()) { return -99 }  // inner if — does NOT add narrowing to outer tail
    return -1                      // outer if exits here
  }
  return m.value                   // safe: outer if always exits on !m.exists()
}
```

Inner if-blocks' exits do NOT propagate narrowing facts out of their containing block to the enclosing block's tail. The outer if block is what the early-return analysis walks.

---

## `is` Namespace Resolution

`is TypeName` looks up `TypeName` in the types-only namespace. A same-name binding in the values namespace does NOT shadow the type lookup:

```ynz
let Circle = 5         // binding in values namespace
if (shape) {
  is Circle => ...     // looks up Circle type — finds the Circle shape, not the binding
}
```

The binding `Circle` remains legal elsewhere in expression position; the `is` arm sees only the type.

---

## Closure Non-Propagation (Forward-Defensive)

Closures do not inherit narrowing flags from the enclosing scope. This is a conservative rule for when closures ship (v0.3+):

```ynz
if (m.exists()) {
  let f = () => m.value    // compile error even in M6: closure doesn't carry the flag
}
```

Documented here so the narrowing engine's architecture is closure-aware from M6 onward. No test fires in M6 (closures aren't runnable); the rule prevents the engine from being designed in a way that would silently allow this when closures land.

---

## IDE Muted Hints (v0.2 LSP obligation)

The narrowing analysis produces two informational annotations for the v0.2 LSP to surface:

1. Inside `if (x is Foo) { ... }`, hover on `x` shows: `narrowed to Foo (because of is-check on line N)` — INFORMATIONAL category (per [`.claude/rules/inference.md`](../../../.claude/rules/inference.md); no typeable equivalent; comment-style annotation).

2. Inside `if (m.exists()) { return ... } m.value`, hover on the post-if `m.value` shows: `narrowed via early-return on line N` — INFORMATIONAL.

These hints are not emitted in M6 (LSP is v0.2 work). M6 only builds the typeck infrastructure; v0.2 surfaces it. This document records the v0.2 obligation so it isn't forgotten.

---

## M7 — Errors-Capable Narrowing Rules

M7 extends the narrowing table with `errors`-capable values (parallel to `maybe<T>`). These use the same `NarrowingTable` infrastructure as M6 — a new "errors-capable" fact per binding.

| Form | Narrowing granted? | Notes |
|---|---|---|
| `if (e.failed()) { e.message }` | YES — inside then-block, `e` is narrowed to "failed" state; `.message` / `.suggestions` / `.trace` / `.source` accessible | Bread-and-butter case. |
| `if (!e.failed()) { ... } else { e.message }` | YES — `e` is narrowed to "failed" state in else-block | Negative form. |
| `if (e.failed()) { return ... } use(e)` | YES — early-return narrowing: after the if, `e` is narrowed to its success type | Same early-return machinery as `maybe<T>`. |
| `if (e.failed()) { panic(...) } use(e)` | YES — panic is a recognized exit form | Parallel to `maybe<T>` rule. |
| `let result = e; result.failed()` | YES for `result` (not `e`) — the narrowing tracks the binding | Assignment to a new binding re-tracks. |
| `use(e)` without prior `.failed()` check (inside `errors` fn) | AUTO-PROPAGATION — compiler emits early return at this point; `e` is narrowed to success type after | The "eager feel" of the hybrid model. |
| `use(e)` without prior `.failed()` check (outside `errors` fn) | COMPILE ERROR — must handle | Forces `.or()`, `.failed()` check, or caller `errors`. |
| `if (e.failed()) { ... }` AFTER `use(e)` | COMPILE ERROR — check-after-use | `e` was already narrowed to success type at `use(e)`; `.failed()` no longer applies. Named diagnostic: "check-after-use". |
| `e.message` without `.failed()` check in scope | COMPILE ERROR — `.message` requires "failed" narrowing proof | Access to `.message`/`.suggestions`/`.trace`/`.source` gated by narrowing. |
| `if (e.failed() && other) { e.message }` | YES — `&&` propagates; inside body both facts hold | Same as `maybe<T>` `&&` rule. |
| `if (e.failed() \|\| other) { e.message }` | **NO** — `\|\|` does NOT propagate single-branch narrowing | Same as `maybe<T>` `\|\|` rule. Same diagnostic wording adapted for `errors`. |
| `e = newResult; use(e)` | AUTO-PROPAGATION tracks the new binding | Reassignment resets the narrowing fact for `e`. |
| `if (e.failed()) { mutatingFn(e.lend); e.message }` | **NO** — `lend` call invalidates the fact | Same rule as `maybe<T>`. |
| `if (e.failed()) { readFn(e.share); e.message }` | YES — `share` does NOT invalidate | Same rule as `maybe<T>`. |
| `for x in fallibleIter` (inside `errors` fn) | AUTO-PROPAGATION per step — each step's failure auto-propagates to caller | `FallibleIterable` for-loop is syntactic sugar over `errors`-capable `next()` calls. |
| `for x in fallibleIter` (outside `errors` fn without adapter) | COMPILE ERROR — fallible for-loop in non-errors context | Must use `.orSkipFailures()` / `.withErrors()` adapter OR mark caller `errors`. |

**Parallel to `maybe<T>`:** the errors-capable narrowing is the same machinery, second flavor. The `NarrowingTable` stores a fact-per-binding; for `maybe<T>` the fact is "exists"; for `errors`-capable the fact is "not-failed (success) / failed / unknown (pre-check)". The early-return narrowing algorithm is identical — only the fact type changes.

**"Check-after-use" diagnostic (exact wording):**

> **WHAT**: `.failed()` cannot be called here. The binding `raw` was first used as a plain value on line N (in `parseConfig(raw)`). After that first use, the compiler treated `raw` as its success type.
>
> **WHAT INSTEAD**: Move the `.failed()` check above the first use:
> ```ynz
> let raw = readFile("config.txt")
> if (raw.failed()) { return Config.default() }
> let parsed = parseConfig(raw)
> ```
>
> **WHY**: When you passed `raw` to `parseConfig()` without checking first, the compiler inserted auto-propagation at that point. Auto-propagation means: "if it failed, return the error to my caller." After that happens, `raw` is treated as a plain string — there's nothing left to check.

---

## Cross-References

- [`docs/reference/REF-maybe.md`](../../reference/REF-maybe.md) — user-facing `.value` narrowing surface
- [`docs/reference/REF-unions.md`](../../reference/REF-unions.md) — user-facing `is` narrowing surface
- [`docs/internal/implementation/IMP-maybe.md`](IMP-maybe.md) — M5's narrow-subset rules table (M6 extends it)
- [`docs/internal/implementation/IMP-unions.md`](IMP-unions.md) — union type layout and exact-type-match rule
- [`docs/internal/implementation/IMP-options.md`](IMP-options.md) — options type multi-case (exhaustiveness, not narrowing)
- [`docs/internal/implementation/IMP-control-flow.md`](IMP-control-flow.md) — early-return path analysis (M3's `return_paths.rs` is the substrate M6 extends)
- [`.claude/rules/inference.md`](../../../.claude/rules/inference.md) — muted-hint protocol for the v0.2 LSP surfaces
- `crates/ynz-typeck/src/narrow.rs` — flow-narrowing analysis implementation (M6)
- `crates/ynz-typeck/src/return_paths.rs` — return-path analysis (M3 substrate, extended in M6)
