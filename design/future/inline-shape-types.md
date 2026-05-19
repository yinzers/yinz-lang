# Inline / Anonymous Shape Types — Single-Use Structural Types

> **Status**: Design open — pending Patrick's plan pass.
> **Target version**: language feature, so v0.1.x or a language-focused slot post-v0.2. Not v0.2 (tooling-only).
> **Trigger that surfaced it**: Patrick hit verbosity friction while testing — defining a one-off shape outside a function just to type a single binding scrolled the eye too far from the use site.

---

## The problem

Named `shape Foo { ... }` declarations are the right tool when a type is used in multiple places. But single-use types — a config struct for one function, an intermediate result that never leaves a loop — force you to scroll to the top of the file to see a type definition that is only meaningful in one spot. That's a coupling smell: the type's definition is physically separated from the only code that uses it.

## The rule

`shape Foo { ... }` only makes sense when `Foo` is used more than once. For one-off types, the definition should live at the call site.

## Proposed syntax (TypeScript-style structural types)

```ynz
// Named shape — correct when used in multiple functions
shape IntervalConfig {
    minutes: int
    timeframe: Timeframe
}

// Inline type — correct for a one-off config that never leaves this loop
const intervals: fixed<{ minutes: int, timeframe: Timeframe }> = [
    { minutes: 5,  timeframe: Timeframe.fiveMinute },
    { minutes: 60, timeframe: Timeframe.hourly },
]

for ({ minutes, timeframe } in intervals) {
    ...
}
```

## Golden rule alignment

- **Rule 2 (self-documenting)**: inline types document themselves at the point of use — the type and its consumer are in the same visual block
- **Rule 6 (familiar from TypeScript)**: TypeScript has both `interface Foo {}` (named) and `{ field: Type }` (inline) — Yinz developers will expect the same split
- **Rule 12 (human-readable)**: `{ minutes: int, timeframe: Timeframe }` reads cleanly without a name

## Open design questions

1. **Naming**: does the compiler generate an internal name for the type (for diagnostics, LLVM mangling), or is it truly anonymous? TypeScript generates positional names internally; Rust uses path-based names for closures.

2. **Equivalence**: are two inline types with identical field sets the same type? `{ a: int, b: string }` in two different files — same? TypeScript says yes (structural typing). Yinz currently uses nominal typing for shapes. **This is a significant decision** — see "Structural vs nominal" below.

3. **Where allowed**: inline types as annotations only (`:` position), or also in `follows` contracts? `shape Foo follows { compare(share self, share other: Self) -> int }` — is that useful or noise?

4. **Auto-promotion lint**: if a user writes the same inline type in two places, should a Tier 3 lint suggest extracting it to a named shape? This follows the auto-promotion pattern in `.claude/rules/auto-promotion.md`.

## Structural vs nominal — the blocking call

This is the decision that determines the scope of the feature:

- **If Yinz stays nominal (current behavior)**: two inline `{ minutes: int, timeframe: Timeframe }` types in different files are different types and can't be used interchangeably. The inline form is purely a "save a `shape` declaration" sugar; semantics unchanged.
- **If Yinz adopts structural for anonymous types only (TypeScript's model)**: you get the expected behavior — two identical inline shapes are interchangeable — but add real complexity to the type system. Nominal vs structural mixing has known footguns (subtype-vs-equivalence confusion, error message quality).

**Decide this first.** It determines whether this is a 2-day sugar feature or a 2-week type-system extension.

## Trigger to design

When a user repeatedly defines single-use named shapes just to satisfy the "no shapes in function bodies" rule. The current diagnostic ("move the `shape` declaration to the top level") becomes the signal — if that message fires more than a handful of times in real codebases, this feature is overdue.

Patrick already hit it once during testing (2026-05-19), so the signal is live.

## Related design

- `.claude/rules/non-oop.md` — Yinz's shapes-are-data model (must not get OOP-flavored by the inline addition)
- `design/type-system.md` — current nominal-typing decision; inline types' equivalence rule is a constrained extension to this
- `design/golden-rules.md` Rule 2 + Rule 6 + Rule 12 — the alignment rationale above
- `.claude/rules/auto-promotion.md` — the "lint when used 2+ times → suggest extraction" pattern question
- TypeScript reference: structural typing for object literal types is described in TS handbook "Structural Type System" section
