# Yinz

A programming language that's as fast as Rust and as readable as JavaScript.

You write clean, simple code. The compiler turns it into fast, safe machine code. No garbage collector. No runtime crashes from null. No mystery memory bugs.

---

## A taste of Yinz

```
shape Player {
  name: string
  health: number

  function takeDamage(lend self, amount: number) -> nothing {
    self.health = self.health - amount
  }
}

function getTopPlayers(share players: fixed<Player>, count: number) -> array<string> {
  let active = players.filter(p => p.health > 0)

  if (active.count() == 0) {
    return []
  }

  let ranked = active.sort(p => p.health, desc)
  let top = ranked.limit(count)
  return top.map(p => p.name)
}
```

If you've written JavaScript | TypeScript before, most of this should feel familiar.

---

## The 12 Golden Rules

Every decision in this language follows these rules. When something seems weird, one of these is why.

1. **Dot-first design** — if something can be `.method()` with autocomplete, it is
2. **Self-documenting syntax** — a jr dev who's never seen Yinz should understand any line
3. **No garbage collector** — ownership handles memory; no pauses, no overhead
4. **Compiler does the hard work** — smart defaults, type inference, optimization
5. **Compile-time safety** — wrong code fails at compile time, never at runtime
6. **Familiar syntax** — borrowed from JavaScript/TypeScript where possible
7. **Step-by-step over chaining** — each operation gets its own line and a name
8. **Zero-cost abstractions** — high-level syntax compiles to the same code as low-level
9. **Fast to type** — quick without sacrificing readability
10. **Efficiency first, dynamic after** — the default path is always the fastest path
11. **The compiler is a teacher** — errors explain what went wrong and suggest fixes
12. **Human-readable over jargon** — `options` not `enum`, `nothing` not `void`, `follows` not `implements`

---

## How to Read This Spec

Each section covers one part of the language. They're self-contained — start wherever you're curious.

- [Variables](variables.md) — `let` and `const`
- [Functions](functions.md) — defining and calling functions
- [Ownership](ownership.md) — how memory works without a garbage collector
- [Types](types.md) — defining your own data shapes
- [Numeric Types](numeric-types.md) — `number`, `float`, `int`, and `number<N>` for high precision
- [Options](options.md) — named value sets (like enums, but readable)
- [Collections](collections.md) — fixed arrays, growable arrays, and maps
- [Maybe Types](maybe.md) — values that might not exist
- [Unions](unions.md) — values that can be one of several types
- [Generics](generics.md) — types that work with any type
- [Control Flow](control-flow.md) — if, multi-case if, for, while, early returns
- [Destructuring](destructuring.md) — pulling fields out of types
- [Type Conversion](type-conversion.md) — converting between types with dot methods
- [Strings](strings.md) — text and interpolation
- [Errors](errors.md) — handling things that can go wrong
- [Concurrency](concurrency.md) — running code in parallel, background tasks
- [Modules](modules.md) — importing, exporting, and organizing code across files
- [Scope](scope.md) — block scoping and file-level constants
- [Main Function](main.md) — program entry point
- [Doc Comments](doc-comments.md) — documenting your public API with `///`
- [Configuration](config.md) — yinz.toml, .env, and runtime set functions
- [Tooling](tooling.md) — ynz build, ynz run, ynz watch, ynz test, ynz add
- [Testing](testing.md) — writing and running tests
- [Packages](packages.md) — adding and publishing packages
- [Operators](operators.md) — operator overloading, boolean operators (`&&` `||` `!`), bitwise operators
- [Sensitive Values](sensitive.md) — auto-redacting secrets with the `sensitive` type modifier
- [Iterables](iterables.md) — custom iteration with follows Iterable
- [FFI](ffi.md) — calling C libraries and system APIs
- [Linting](linting.md) — errors, warnings, and suggestions the compiler catches
- [Performance](performance.md) — why Yinz is fast
