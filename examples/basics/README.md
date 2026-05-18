# `examples/basics/` — Yinz v0.1 Language Showcase

A single growing project that demonstrates **every v0.1 language feature** as it ships. Compile + run after each milestone to see the language in action.

## How to run

```bash
source $HOME/.cargo/env
./target/debug/ynz run examples/basics/src/entrypoint.ynz
```

## What each milestone adds

| Milestone | Status | New features in the demo |
|---|---|---|
| **M1** | ✅ shipped | `function main()`, `print(string)`, string literals |
| **M2** | ✅ shipped | Variables (`let`/`const`), arithmetic (`+ - * / %`), comparison, boolean (`&& \|\| !`), bitwise (`& \| ^ ~ << >>`), primitives (`int`, `float`, `number`, `bool`), polymorphic `print`, `.toString()`/`.toNumber()`/`.toFloat()` intrinsics |
| **M3** | ✅ shipped | `if`/multi-case `if`/`else =>`, `while`, `for (x in range(...))`, early `return`, user-defined functions with params + return types, mutual recursion |
| **M4** | in-progress | shapes (data + contract signatures), standalone functions taking receivers (UFCS dot-call sugar), ownership (`share`/`lend`/`give` in signatures, inferred at call sites), `.copy()`/`.freeze()`, `extends` (data-only), `follows` contracts, `hidden` fields, `dynamic Foo` runtime polymorphism, M2 catch-up (`int.max`/`int.min`/`number.epsilon`, `.wrappingAdd()`/`.saturatingAdd()`) |
| **M5** | future | generics (`<T>`), collections (`array<T>`, `fixed<T>`, `map<K,V>`), bracket sugar (`arr[i]`, `m["key"]`) |
| **M6** | future | `options`, union types (`\|`), `maybe T`, `is` narrowing |
| **M7** | future | full Unicode strings, interpolation, `errors` keyword + cascades, `Iterable<T>` / `FallibleIterable<T>` |
| **M8** | future | modules (`import`/`export`), doc comments (`///`), `sensitive`, `wait`/`background` concurrency keywords, bignum `number<N>` for N > 34 |

## Deferred features (placeholders in source)

Some features are locked but ship after v0.1. They appear in the source as commented placeholders pointing to their design docs:

- `arena scratch { ... }` — v0.2 per `design/future/arena.md`
- `verified { ... }` — v0.3+ (unsafe escape hatch, name reserved per `.claude/rules/vocabulary.md`)
- Self-referential shapes — v0.3+ per `design/future/self-references.md`
- `--kernel` mode (custom allocator plug-in) — v0.3+ per `design/future/no-runtime-mode.md`

When they ship, they replace the placeholder comments with real demo content.

## Why this exists

`.claude/rules/plan-invariants.md` `### Demo & Error Gallery` subsection: every milestone phase that adds executable surface MUST extend this file with the new feature in context. This is the human-eyes-on layer for the language UX — Patrick reviews each phase's demo to ensure the feature feels right before it ships.

## Companion: `examples/errors/`

The per-milestone error gallery (`examples/errors/m{N}_errors.ynz`) shows every compile error each milestone introduces. Each phase also extends the gallery for its milestone.
