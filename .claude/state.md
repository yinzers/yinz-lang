# Session State: ynz

**Last Updated**: 2026-05-12

---

## Active Workstreams

*(auto-rebuilt by SessionStart hook from `.claude/plans/active/*.md` front-matter — do not edit by hand)*

<!-- RADAR-START -->
*(no active workstreams)*
<!-- RADAR-END -->

---

## Environment & Commands (CRITICAL — survives compaction)

**Project**: ynz
**Package Manager**: {bun/npm/yarn/pnpm}
**Container**: {yes/no — exec pattern if yes}
**Database**: {connection / db name}

```bash
# Common commands
```

---

## Active Decisions (append with WHY)

- [2026-05-12] **Compiler implementation language = Rust**: Mature LLVM bindings (inkwell), strong ADT/pattern-matching for AST, salsa framework gives incremental builds + LSP "for free." See `design/compiler-language.md`.
- [2026-05-12] **MVP scope split into v0.1 / v0.2 / v0.3 / v1.0 / v2+**: Concurrency keywords parse from day 1 but run sequentially until v0.3 (when auto-parallelization optimization engages). See `design/mvp-scope.md`.
- [2026-05-12] **Error auto-propagation = flow-sensitive narrowing (Option B under, Option A in feel)**: If user calls `.failed()` before using the success value, auto-propagation suppressed; otherwise compiler auto-propagates at first use. Same `.failed()`/`.or()` API works inside AND outside `errors` functions. See `design/errors.md`.
- [2026-05-12] **Generic functions = v0.1, `[T]` syntax with `follows` constraints inline**: Type inference at call sites. `where` clauses rejected — inline keeps constraint visible next to the parameter. See `design/generics.md`.
- [2026-05-12] **Numeric types = handwritten, validated against IEEE 754 test vectors**: `number` = decimal128 (default), `number[N]` up to N=4096, `float` = f64, `int` = i64. Sized variants (`int[N]`, `f32`) deferred. Overflow panics by default with `.wrappingAdd()`/`.saturatingAdd()` escape valves. See `design/numeric-types.md` + `design/deferrals.md`.
- [2026-05-12] **Strings use `.get()` (code point) + `.byteAt()` + `.graphemeAt()`**: No `char` type. Default indexing is by Unicode code point. Bytes and graphemes are explicit alternates. See `spec/strings.md`.
- [2026-05-12] **Bracket sugar for `.get()` and `.set()` on all collections AND maps**: `arr[i]`, `m["key"]`, `s[i]` all desugar to `.get()`. Writes via `arr[i] = v` desugar to `.set()`. Strings immutable (no write sugar). Types reject bracket access entirely — forces dot for fields. Reverses earlier no-`map[key]` decision. See `design/collections.md`.
- [2026-05-12] **Iterable contract = two types (`Iterable[T]`, `FallibleIterable[T]`)**: In-memory collections follow `Iterable[T]`; I/O sources follow `FallibleIterable[T]`. Same `for` syntax; compiler infers fallibility from the source's contract and auto-propagates errors when needed. Stdlib adapters `.orSkipFailures()` and `.withErrors()` for ergonomic fallible-to-infallible conversion. See `design/iterables.md` + `spec/iterables.md`.
- [2026-05-12] **Import aliases + duplicate-name compile error**: TS-style `{ name as renamed }` and `namespace as renamed`. Duplicate names (including stdlib-vs-local collisions) refuse to silently pick — compile error forces aliasing. See `design/modules.md` + `spec/modules.md`.
- [2026-05-12] **Lock file = TOML, flat array of `[[package]]` tables**: Same format as `yinz.toml`. Diff-friendly, manually editable in emergencies. Install mechanism (content-addressed global cache, hard-links, parallel resolver, lazy integrity) aims for bun-class speed — v0.2 work. See `design/packages.md` + `spec/packages.md`.

---

## Superseded / Archived

- (none)

---

## Project-Wide Notes

*(cross-workstream context, gotchas, user preferences not tied to one plan)*
