# Design Decisions — Index

Every design topic has its own file. This is the index. One line per topic, link to the file.

---

## Language Design

| Topic | File | What's in it |
|-------|------|--------------|
| Type system | `design/type-system.md` | `type`, `base`, `extends`, `follows`, `override`, structural typing, `or`, `maybe`, `options`, `hidden`, type aliases |
| Generics | `design/generics.md` | Type generics `[T]`, function generics, `follows` constraints inline, type inference at call sites |
| Ownership | `design/ownership.md` | `.share`/`.lend`/`.give`/`.copy`/`.freeze`, no direct array indexing |
| Collections | `design/collections.md` | `fixed`/`array`/`map`, no chaining, method naming, bracket sugar for `.get()`/`.set()`, string indexing methods |
| Error handling | `design/errors.md` | `errors` keyword, no try/catch, flow-sensitive auto-propagation narrowing |
| Functions | `design/functions.md` | `function` keyword, `-> nothing`, no tuples, closure syntax |
| Numeric types | `design/numeric-types.md` | `number`/`float`/`int`, `number[N]` parameterized precision (cap 4096), handwritten impls, overflow methods |
| Naming | `design/naming.md` | Human-readable keywords, capital = type rule, comments syntax |
| Control flow | `design/control-flow.md` | No standalone `else`, multi-case `if`, exhaustiveness, jump table optimization |
| Scope | `design/scope.md` | Block scoping, no mutable globals, const expressions, export for sharing |
| Main entry | `design/main-entry.md` | `function main()`, file from yinz.toml, args from stdlib, errors to default handler |
| Doc comments | `design/doc-comments.md` | `///` only, no block docs, exported items only, field documentation |
| Testing | `design/testing.md` | Built-in `test` keyword, setup/teardown (file+per-test), single-level groups, `assertFails`/`assertPanics`, file-level parallelism |
| Packages | `design/packages.md` | `ynz add/remove/update`, lock file, yinz_modules, tree shaking |
| Operators | `design/operators.md` | `follows` contracts, `Self` keyword, `print()` default, `&&`/`\|\|`/`!` symbols, bitwise symbols, no `===` |
| Sensitive values | `design/sensitive.md` | `sensitive` modifier, auto-redact in all output, `.reveal()` explicit opt-in, stripped from release |
| FFI | `design/ffi.md` | `foreign` keyword, wrap in safe functions, compiler requires `wait` (DEFERRED to v2+) |
| Iterables | `design/iterables.md` | `follows Iterable[T]`, `next()` with `maybe T`, hidden state fields |
| GPU dispatch | `design/gpu.md` | MVP2+ vision: `gpu` call-site keyword, compiler manages CPU/GPU dispatch (DEFERRED to v2+) |
| Destructuring | `design/destructuring.md` | Object only, no array, `as` rename, parameter destructuring |
| Type conversion | `design/type-conversion.md` | Dot methods, no `as` keyword, safe vs unsafe split, no ternary |
| Concurrency | `design/concurrency.md` | Auto-parallelization, `wait`, `background`, ownership with tasks (optimization DEFERRED to v0.3) |
| Modules | `design/modules.md` | `import`/`export`, no defaults, no wildcards, root-relative paths, stdlib auto-import |
| Configuration | `design/config.md` | Three layers, TOML choice, no env splitting, `set` functions |
| Linting & build | `design/linting.md` | "Compiler IS the linter" — three-tier diagnostics (errors/warnings/suggestions), curated v0.4 rule list, three-part WHAT/INSTEAD/WHY format, customization v1.x |
| Versioning | `design/versioning.md` | Pre-release delete policy, post-release major bumps, no backwards compat |

## Compiler & Tooling

| Topic | File | What's in it |
|-------|------|--------------|
| Compiler design | `design/compiler.md` | Incremental builds, IDE language server, no-indexing rationale |
| Compiler implementation language | `design/compiler-language.md` | Rust + Salsa + inkwell + ariadne + hand-written recursive descent — decision and rationale |
| MVP scope | `design/mvp-scope.md` | Granular 24-version sequence to v1.0 + 3 post-launch versions |
| Compiler error style | `design/compiler-errors.md` | Required three-part WHAT/WHAT-INSTEAD/WHY format, jargon ban-list, tone guide, multi-error strategy |

## Reference

| Topic | File | What's in it |
|-------|------|--------------|
| Golden rules | `design/golden-rules.md` | All 13 rules with full reasoning. Rule 11 expanded — teaching mission. |
| Teaching mission | `design/teaching-mission.md` | First-class language goal — compiler as mentor, three-part diagnostic format, university-adoption aspiration |
| Open questions | `design/open-questions.md` | Unresolved design decisions |
| Deferrals | `design/deferrals.md` | Features intentionally not in v0.1 — with substitute and trigger conditions |

## Standard Library

All stdlib design lives in `design/stdlib/`. See `design/stdlib/overview.md` for the index.
