# `examples/pirates-roster/` — Yinz v0.1 Language Showcase

**Layout: single-entry project** (the canonical Yinz shape for ~95% of projects). One `yinz.toml`, one `entrypoint.ynz`, code organized into `services/` and `utils/` subfolders imported root-relatively. No `[entries]` table, no `ships/` folder — that's only for multi-entry projects (see `../stadium-fleet/` for that layout, available in v0.22).

**Theme:** Pittsburgh Pirates roster — players (Clemente, Wagner), stats (career hits, batting averages in thousandths, scouting flags), and lineups (starting / bench / IL via `options LineupStatus`). The entrypoint walks M1–M8 features in baseball-flavored context; companion modules (`services/players.ynz` for the `Pirate` shape, `utils/math_extra.ynz` for stat math) export reusable helpers.

A single growing project that demonstrates **every v0.1 language feature** as it ships. Compile + run after each milestone to see the language in action.

## How to run

```bash
source $HOME/.cargo/env
./target/debug/ynz run examples/pirates-roster/
```

## What each milestone adds

| Milestone | Status | New features in the demo |
|---|---|---|
| **M1** | ✅ shipped | `function entrypoint()`, `print(string)`, string literals |
| **M2** | ✅ shipped | Variables (`let`/`const`), arithmetic (`+ - * / %`), comparison, boolean (`&& \|\| !`), bitwise (`& \| ^ ~ << >>`), primitives (`int`, `float`, `number`, `boolean`), polymorphic `print`, `.toString()`/`.toNumber()`/`.toFloat()` intrinsics |
| **M3** | ✅ shipped | `if`/multi-case `if`/`else =>`, `while`, `for (x in range(...))`, early `return`, user-defined functions with params + return types, mutual recursion |
| **M4** | ✅ shipped | shapes (data + contract signatures), standalone functions taking receivers (UFCS dot-call sugar), ownership (`share`/`lend`/`give` in signatures, inferred at call sites), `.copy()`/`.freeze()`, `extends` (data-only), `follows` contracts, `hidden` fields, `dynamic Foo` runtime polymorphism, M2 catch-up (`int.max`/`int.min`/`number.epsilon`, `.wrappingAdd()`/`.saturatingAdd()`) |
| **M5** | ✅ shipped | generics (`<T>`), collections (`array<T>`, `fixed<T>`, `map<K,V>`), bracket sugar (`arr[i]`, `m["key"]`) |
| **M6** | ✅ shipped | `options`, union types (`\|`), `maybe T`, `is` narrowing |
| **M7** | ✅ shipped | full Unicode strings, interpolation, `errors` keyword + cascades, `Iterable<T>` / `FallibleIterable<T>` |
| **M8** | ✅ shipped | modules (`import`/`export`), doc comments (`///`), `sensitive`, `wait`/`background` concurrency keywords, bignum `number<N>` for N > 34 |

## Deferred features (placeholders in source)

Some features are locked but ship after v0.1. They appear in the source as commented placeholders pointing to their design docs:

- `arena scratch { ... }` — v0.2 per `design/future/arena.md`
- `verified { ... }` — v0.3+ (unsafe escape hatch, name reserved per `.claude/rules/vocabulary.md`)
- Self-referential shapes — v0.3+ per `design/future/self-references.md`
- `--kernel` mode (custom allocator plug-in) — v0.3+ per `design/future/no-runtime-mode.md`

When they ship, they replace the placeholder comments with real demo content.

## Why this exists

`.claude/rules/plan-invariants.md` `### Demo & Error Gallery` subsection: every milestone phase that adds executable surface MUST extend this file with the new feature in context. This is the human-eyes-on layer for the language UX — Patrick reviews each phase's demo to ensure the feature feels right before it ships.

## Companion: `examples/primantis-orders/`

The per-milestone error gallery (`examples/primantis-orders/m{N}_errors.ynz`) shows every compile error each milestone introduces. Each phase also extends the gallery for its milestone.
