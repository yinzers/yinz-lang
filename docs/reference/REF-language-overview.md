---
name: "REF-language-overview"
description: "A programming language that is fast, safe and easy to read."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Yinz

A programming language that is fast, safe and easy to read.

You write clean, simple code. The compiler turns it into fast, safe machine code. There is no garbage collector (no background cleaner that pauses your program). A missing value can't crash your program, because the compiler makes you handle it. Memory bugs are caught while compiling, not while the program runs.

---

## A taste of Yinz

```ynz
shape Player {
  name: String
  health: Number
}

// Functions are written on their own; player.takeDamage(5) is a shortcut for takeDamage(player, 5)
function takeDamage(lend self: Player, amount: Number) -> Nothing {
  self.health = self.health - amount
}

function getTopPlayers(share players: Fixed<Player>, count: Number) -> Array<String> {
  let active = players.filter(p => p.health > 0)

  if (active.count() == 0) {
    return []
  }

  let ranked = active.sort(p => p.health, desc)
  let top = ranked.limit(count)
  return top.map(p => p.name)
}
```

---

## The 13 Golden Rules

Every decision in this language follows these rules. When something seems weird, one of these is why.

1. **No garbage collector** — ownership handles memory; no pauses, no overhead
2. **Compile-time safety** — every check that can run at compile time does; a failure that can't (file and network problems, division by zero) is explicit: `errors` or a task-isolated panic, never silent
3. **The compiler is a teacher** — errors explain what went wrong and suggest fixes
4. **Self-documenting syntax** — a junior developer who has never seen Yinz should understand any line
5. **Human-readable over jargon** — `options` not `enum`, `Nothing` not `void`, `follows` not `implements`
6. **Zero-cost abstractions** — high-level syntax compiles to what a careful Rust/C++ programmer would write by hand; the features themselves cost what they inherently cost
7. **Efficiency first, dynamic after** — the default path is always the fastest path
8. **Compiler does the hard work** — smart defaults, type inference, optimization
9. **Step-by-step over chaining** — each operation gets its own line and a name
10. **Capital letter = type** — scan any line: capital letter means type, built-in types included (`Int`, `String`, `Array<T>`); lowercase means everything else (variable, function, keyword, module)
11. **Dot-first design** — if something can be `.method()` with autocomplete, it is
12. **Familiar syntax** — borrowed from JavaScript/TypeScript where possible
13. **Fast to type** — quick without sacrificing readability

---

## How to Read This Spec

Each section covers one part of the language. They're self-contained — start wherever you're curious.

- [Variables](REF-variables.md) — `let` and `const`
- [Functions](REF-functions.md) — defining and calling functions
- [Ownership](REF-ownership.md) — how memory works without a garbage collector
- [Types](REF-types.md) — defining your own data shapes
- [Numeric Types](REF-numeric-types.md) — `Number`, `Float`, `Int`, and `Number<N>` for high precision
- [Options](REF-options.md) — a fixed set of named choices
- [Collections](REF-collections.md) — fixed arrays, growable arrays, and maps
- [Maybe Types](REF-maybe.md) — values that might not exist
- [Unions](REF-unions.md) — values that can be one of several types
- [Generics](REF-generics.md) — one shape or function that works with many types
- [Control Flow](REF-control-flow.md) — if, multi-case if, for, while, early returns
- [Destructuring](REF-destructuring.md) — pulling fields out of shapes
- [Type Conversion](REF-type-conversion.md) — converting between types with dot methods
- [Strings](REF-strings.md) — text and interpolation
- [Errors](REF-errors.md) — handling things that can go wrong
- [Concurrency](REF-concurrency.md) — running code at the same time, background tasks
- [Modules](REF-modules.md) — importing, exporting, and organizing code across files
- [Scope](REF-scope.md) — block scoping and file-level constants
- [Main Function](REF-main.md) — program entry point
- [Doc Comments](REF-doc-comments.md) — documenting your public API with `///`
- [Configuration](REF-config.md) — yinz.toml, .env, and runtime set functions
- [Coming from TypeScript](../guides/coming-from-typescript.md) — a guide to what changes if you already write TypeScript
- [Tooling](REF-tooling.md) — ynz build, ynz run, ynz watch, ynz test, ynz add
- [Testing](REF-testing.md) — writing and running tests
- [Packages](REF-packages.md) — adding and publishing packages
- [Operators](REF-operators.md) — math, comparison, boolean operators (`&&` `||` `!`), bitwise operators
- [Sensitive Values](REF-sensitive.md) — hiding secrets automatically with the `sensitive` modifier
- [Iterables](REF-iterables.md) — making your own shapes work in `for` loops with `follows Iterable`
- [FFI](REF-ffi.md) — calling C libraries and system APIs
- [Linting](REF-linting.md) — errors, warnings, and suggestions the compiler catches
- [Performance](REF-performance.md) — why Yinz is fast
