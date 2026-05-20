# Dot-Postfix Rule — Parens for Actions, No Parens for Access

Loaded when designing any new syntax that uses dot-postfix (`value.something`) or when writing examples in spec/design docs. Apply alongside `.claude/rules/non-oop.md` (UFCS context) and `.claude/rules/inference.md` (call-site modifier inference).

---

## The Rule

> **If it accesses a value (a field or a type-attached constant), no parens. If it performs an action (calls a function, runs a transformation), parens.**

| Form | Use for | Examples |
|---|---|---|
| `value.name` (no parens) | Field access; type-attached constant | `player.health`, `int.max`, `number.epsilon` |
| `value.name(args)` (parens) | Function call via UFCS sugar; transformation operation | `player.heal(20)`, `score.toString()`, `cfg.freeze()` |

The parens (or lack thereof) signal what's happening at a glance. No ambiguity about whether a dot-postfix is reading data or performing an action.

---

## Examples (all using real Yinz operations from the current scope)

### Field access — no parens

```ynz
shape Player { name: string, health: int }
const p: Player = { name: "Patrick", health: 100 }

p.name                     // field access — no parens
p.health                   // field access — no parens
```

### Type-attached constants — no parens (added in M4 P5 catch-up)

```ynz
int.max                    // 9223372036854775807
int.min                    // -9223372036854775808
number.epsilon             // smallest representable positive decimal128
number.max                 // largest representable decimal128
```

### Method calls via UFCS — parens (M2-shipped intrinsics + M4 user-defined functions)

```ynz
score.toString()           // M2 intrinsic — toString on int
val.toFloat()              // M2 intrinsic
val.toNumber()             // M2 intrinsic

p.heal(20)                 // M4 user-defined — UFCS sugar for heal(p, 20)
p.greet()                  // M4 user-defined — UFCS sugar for greet(p)
```

### Body transformations — parens (M4)

```ynz
const backup = original.copy()      // produces a new owned value
configBuilder.freeze()              // locks the binding from further mutation
```

---

## Why this rule exists

Without it, dot-postfix syntax is ambiguous: `value.something` could be a field read or a method call. The convention removes the ambiguity by attaching a hard syntactic signal:

- **No parens = pure read.** No side effect; no allocation; no transformation. Read the data and move on.
- **Parens = action.** Something is happening — a function runs, a value is transformed, state may change.

This matches Yinz's broader "self-documenting syntax" rule (Golden Rule 2). A jr dev seeing `int.max` immediately knows it's a constant; seeing `score.toString()` immediately knows a function runs. No mental disambiguation step.

---

## What this rule DOES NOT govern

- **Ownership modifiers** (`share`/`lend`/`give`) are NOT body-level dot-postfix operations. They're inferred at call sites from the callee's signature and rendered as IDE muted hints. See `.claude/rules/inference.md`.
- **Operator overloading** at definition time uses bare-signature contract syntax inside `shape` blocks (per `.claude/rules/non-oop.md`). The dot-postfix rule applies at call sites; contract declarations are a different syntactic position.
- **Module imports** (`request.get(url)`) follow the same rule — `request` is a module name; `.get(url)` is a function call with parens. Consistent.

---

## Examples-must-use-real-operations rule

Every example in this file (and any spec/design/plan/rule file) MUST use real Yinz operations from the current scope. No invented APIs for illustration. The `int.parse("42")` mistake (parse doesn't exist; we use `.toNumber()`) won't repeat.

Concretely: when writing an example, only use:
- Operations already shipped in M1/M2/M3 (`print`, primitive intrinsics, control flow, user functions)
- Operations locked in the current milestone's plan (M4: shapes, methods via UFCS, ownership, `.copy()`, `.freeze()`, `extends`, `follows`, `dynamic`, type-attached constants)
- The exact operations defined inline in the same example

If a new API name appears in your example, check it's real before saving. Cross-reference `crates/ynz-typeck/src/intrinsics.rs` for the M2 primitive intrinsic table.

---

## Cross-References

- `.claude/rules/non-oop.md` (UFCS — defines `value.method()` as sugar for `method(value)`; this file says when method calls use parens)
- `.claude/rules/inference.md` (ownership modifier inference at call sites — NOT body-level dot-postfix)
- `.claude/rules/vocabulary.md` (Yinz user-facing terms — field vs method vocabulary)
- `.claude/rules/spec-writing.md` (examples must be runnable Yinz; aligns with the real-operations rule above)
- `design/golden-rules.md` Rule 2 (self-documenting syntax) — this rule operationalizes Rule 2 for dot-postfix
- `crates/ynz-typeck/src/intrinsics.rs` (M2 primitive intrinsic table — source of truth for which dot-postfix methods exist on primitives)
