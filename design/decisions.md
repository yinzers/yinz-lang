# Design Decisions — Index

Every design topic has its own file. This is the index. One line per topic, link to the file.

---

## Language Design

| Topic | File | What's in it |
|-------|------|--------------|
| Type system | `design/type-system.md` | `type`, `base`, `extends`, `follows`, `override`, generics, structural typing, `or`, `maybe`, `options`, `hidden`, type aliases |
| Ownership | `design/ownership.md` | `.share`/`.lend`/`.give`/`.copy`/`.freeze`, no direct array indexing |
| Collections | `design/collections.md` | `fixed`/`array`, no chaining, method naming, map additions |
| Error handling | `design/errors.md` | `errors` keyword, no try/catch, no Result types |
| Functions | `design/functions.md` | `function` keyword, `-> nothing`, no tuples, closure syntax |
| Numeric types | `design/numeric-types.md` | `number`/`float`/`int` — why three, why decimal is default |
| Naming | `design/naming.md` | Human-readable keywords, capital = type rule, comments syntax |
| Control flow | `design/control-flow.md` | No standalone `else`, multi-case `if`, exhaustiveness, jump table optimization |
| Scope | `design/scope.md` | Block scoping, no mutable globals, const expressions, export for sharing |
| Main entry | `design/main-entry.md` | `function main()`, file from yinz.toml, args from stdlib, errors to default handler |
| Doc comments | `design/doc-comments.md` | `///` only, no block docs, exported items only, field documentation |
| Testing | `design/testing.md` | Built-in `test` keyword, assertion functions, compiler-as-teacher output |
| Packages | `design/packages.md` | `ynz add/remove/update`, lock file, yinz_modules, tree shaking |
| Operators | `design/operators.md` | `follows` contracts, `Self` keyword, `print()` default, `&&`/`\|\|`/`!` symbols, bitwise symbols, no `===` |
| Sensitive values | `design/sensitive.md` | `sensitive` modifier, auto-redact in all output, `.reveal()` explicit opt-in, stripped from release |
| FFI | `design/ffi.md` | `foreign` keyword, wrap in safe functions, compiler requires `wait` |
| Iterables | `design/iterables.md` | `follows Iterable[T]`, `next()` with `maybe T`, hidden state fields |
| GPU dispatch | `design/gpu.md` | MVP2+ vision: `gpu` call-site keyword, compiler manages CPU/GPU dispatch |
| Destructuring | `design/destructuring.md` | Object only, no array, `as` rename, parameter destructuring |
| Type conversion | `design/type-conversion.md` | Dot methods, no `as` keyword, safe vs unsafe split, no ternary |
| Concurrency | `design/concurrency.md` | Auto-parallelization, `wait`, `background`, ownership with tasks |
| Modules | `design/modules.md` | `import`/`export`, no defaults, no wildcards, root-relative paths, stdlib auto-import |
| Configuration | `design/config.md` | Three layers, TOML choice, no env splitting, `set` functions |
| Linting & build | `design/linting.md` | Three-tier linting, debug vs release, compile speed principle |
| Versioning | `design/versioning.md` | Pre-release delete policy, post-release major bumps, no backwards compat |

## Compiler & Tooling

| Topic | File | What's in it |
|-------|------|--------------|
| Compiler design | `design/compiler.md` | Incremental builds, IDE language server, no-indexing rationale |

## Reference

| Topic | File | What's in it |
|-------|------|--------------|
| Golden rules | `design/golden-rules.md` | All 13 rules with full reasoning |
| Open questions | `design/open-questions.md` | Unresolved design decisions |

## Standard Library

All stdlib design lives in `design/stdlib/`. See `design/stdlib/overview.md` for the index.
