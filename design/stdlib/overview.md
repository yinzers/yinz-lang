# Standard Library — Overview

Design document for the Yinz standard library. Content here is in planning phase — no implementation yet.

---

## Philosophy

The standard library follows all 12 golden rules. Everything is dot-method driven, autocomplete-first, self-documenting, and designed so a jr dev can use it without reading docs. If Python can do it in one line, Yinz should too — but type-safe and compiled.

---

## Tree Shaking — Non-Negotiable

The compiler only includes stdlib code your program actually uses. If you never touch `Http`, `Tensor`, or `Date`, none of that code exists in your binary. Zero bloat.

```
function main() -> nothing {
  let x = math.sqrt(16)
  print(x)
}
// Binary contains: math.sqrt, print, and their dependencies only.
// Date, Http, File, Tensor, CLI — none of it is included.
```

This means: the stdlib can be as comprehensive as we want. Add everything. The compiler ships only what's used.

---

## Auto-Import — No Import Statement Needed

The standard library is always available. Developers never write `import { sqrt } from "math"`. They just write `math.sqrt()`. The compiler and IDE know the entire standard library at build time. Tree shaking strips anything unused.

This applies to all stdlib modules: `math`, `file`, `http`, `date`, `duration`, `json`, `csv`, `terminal`, `cli`, `path`, `directory`, `random`, `stats`, `convert`, `physics`, `matrix`, `vector`, `tensor`, `vectors`, `env`.

---

## Design Principles

1. **Everything is dot-methods on the relevant type.** `file.read()`, `date.now()`, `http.get()`. Autocomplete-driven discovery.
2. **All I/O uses the `errors` system.** File reads, HTTP calls, parsing — auto-propagate or handle explicitly.
3. **Immutable by default for data types.** Dates, strings — methods return new values, never mutate.
4. **`number` is decimal, `float` is binary, `int` is whole numbers.** Default prevents floating point surprises. IDE teaches when each is appropriate.
5. **Scripting should be as easy as Python.** One-file scripts, CLI tools, data processing — minimal boilerplate.
6. **Tree shaking is mandatory.** Be as comprehensive as wanted. No bloat.
7. **Speed is always the priority.** Vectorize, use SIMD, inline aggressively. Even precision operations should be as fast as possible.

---

## Module Areas

| File | Status |
|------|--------|
| [math.md](math.md) | Design phase |
| [dates.md](dates.md) | Design phase |
| [http.md](http.md) | Design phase |
| [concurrency.md](concurrency.md) | Partially resolved — discussion ongoing |
| [filesystem.md](filesystem.md) | Design phase |
| [cli.md](cli.md) | Design phase |
| [data.md](data.md) | Design phase |
| [ml.md](ml.md) | Design phase |
| [strings.md](strings.md) | Design phase |
| [markets.md](markets.md) | Design phase |

---

## Open Standard Library Questions

- Package manager design (like npm, cargo, pip)
- Testing framework (built-in or stdlib?)
- Database drivers (built-in SQLite? or packages only?)
- Logging framework
- Regex engine specifics
- Crypto / hashing (SHA, AES, etc.)
- Networking beyond HTTP (TCP, UDP, raw sockets)
- Process spawning and OS interaction
- Environment variables
- Compression (gzip, zstd)
