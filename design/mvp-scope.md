# MVP Scope — Versioning Roadmap

What ships in each version of Yinz, and why. This is the source of truth for "is feature X in v0.1?"

---

## v0.1 — Proof of concept ("Yinz compiles and runs")

The minimum viable language. Everything load-bearing for the syntax surface ships here. Anything that DOESN'T ship here gets a substitute or a compile error pointing to `design/deferrals.md`.

**Language features (all v0.1):**
- Variables (`let`, `const`)
- Functions (with ownership modifiers: `share`/`lend`/`give`/`copy`/`.freeze`)
- Types (struct-like with fields and methods)
- Options (named value sets, replaces enums)
- Unions (`type Foo = A or B or C`)
- Maybe types (`maybe T`, `none`, `.exists()`, `.value`, `.or(default)`)
- Generics — both **type generics** (`array[T]`, `map[K,V]`) AND **function generics** (`function foo[T](...)`)
- Collections (`fixed[T]`, `array[T]`, `map[K,V]`)
- Control flow (`if`, multi-case `if`, `for`, `while`, early returns)
- Strings (with interpolation, indexing, byte/grapheme access)
- Scope (block scoping, file-level constants)
- Destructuring (object destructuring, no array destructuring)
- Type conversion (dot methods, no `as` keyword)
- Errors (`errors` keyword, auto-propagation with flow-sensitive narrowing)
- Ownership (`share`/`lend`/`give`/`copy`/`.freeze`)
- Numeric types (`number` = decimal128, `float` = f64, `int` = i64, `number[N]` parameterized)
- Concurrency keywords parse + type-check (`wait`, `background`) — runs SEQUENTIALLY in v0.1
- Modules (`import`, `export`, root-relative paths, stdlib auto-import)
- Main entry (`function main()`)
- Doc comments (`///`)
- Sensitive type modifier (auto-redact in output)
- Bracket sugar for `.get()` / `.set()` on collections
- Operators (`+`, `-`, `*`, `/`, `%`, `&&`, `||`, `!`, comparison, bitwise)
- Operator overloading — NOT in v0.1 (only built-in operators work)

**Tooling (v0.1):**
- `ynz build` — compile a project
- `ynz run` — compile + execute
- `ynz test` — basic test runner
- Decent compile error messages (Rule 11 — "compiler is a teacher" applies from day 1)

**Stdlib (v0.1):**
- Strings (split, trim, pad, replace, search)
- Math (basic arithmetic, trig, log, exp, abs, min, max, floor, ceil, round)
- File I/O (read, write, exists, delete)
- Env vars
- JSON parse / stringify
- Basic date / time (`date.now()`, `date.from()`, comparisons)

---

## v0.2 — Usable ("Yinz is usable for real projects")

Ships when v0.1 is stable and the language has been dogfooded in small projects.

- **Auto-parallelization optimization NOT yet engaged** — concurrency keywords still run sequentially. (Auto-parallel is v0.3.)
- Language server (LSP) — autocomplete, hover, go-to-def, rename, inline errors. Built on the salsa queries the v0.1 compiler already uses.
- Package manager (`ynz add`, `ynz remove`, `ynz update`)
- `ynz watch` (rebuild on file change)
- Full stdlib expansion: HTTP client, full date/duration module, more file/path utilities, regex, encoding (base64, hex), random
- Three-tier linting (errors / warnings / suggestions)
- Polished error messages — every error pattern reviewed for teaching quality

---

## v0.3 — Auto-parallel ("Yinz is fast by default")

The auto-parallelization optimization activates. Compiler dependency analysis kicks in; independent operations actually run in parallel. Code written in v0.1 that used `wait`/`background` semantics works unchanged but now goes faster.

This is a separate milestone because the dependency analysis is a complex sub-project — bounding it to its own version lets v0.2 ship without waiting on it.

---

## v1.0 — Stable ("Yinz spec is locked")

Ships when the language and stdlib have stabilized enough to commit to backward compatibility.

- Operator overloading (`follows Add`/`Subtract`/`Multiply`/`Divide`/`Display`/`Compare`)
- Custom iterables (`follows Iterable[T]`)
- Formal grammar lock — the EBNF / parser becomes the contract
- All compile errors reviewed for tone and teaching quality
- Backwards-compatibility policy kicks in (see `design/versioning.md`)

---

## v2+ — Deferred features

See `design/deferrals.md` for the authoritative ledger. Headline entries:

- **FFI** (foreign function interface — call C/Rust/C++ libraries)
- **GPU dispatch** (the `gpu` call-site keyword, kernel compilation)
- **Sized integer variants** (`int[N]`, `uint[N]` for N != 64)
- **Sized float variants** (`f32`)
- **Arbitrary-precision decimal** beyond `number[4096]`
- **ML stdlib** (tensors, neural net primitives)
- **Markets stdlib** (financial data, brokerage integrations)
- **Self-hosted compiler** (Yinz compiler written in Yinz)
- **Deprecation marking** (only relevant post-1.0)

---

## How to decide if a feature is v0.1

Three questions:

1. **Does the v0.1 syntax surface depend on this?** If yes, it ships now. (Example: ownership keywords change every function signature — can't defer.)
2. **Can users meaningfully write programs without this?** If yes, it can probably defer. (Example: GPU dispatch — programs work without it.)
3. **Does deferring create a substitute users must learn?** If yes, prefer to ship the real thing or document the substitute clearly. (Example: FFI deferral means the v0.1 stdlib needs internal compiler magic to expose C libs — substitute is acceptable.)

When in doubt, prefer v0.1. Cutting later is fine; adding later is harder.
