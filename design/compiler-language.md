# Compiler Implementation Language — Design Decisions

User spec: N/A (internal compiler concern — not user-facing)

---

## Decision: Rust

The Yinz compiler is written in Rust.

**Stack:**
- **Language:** Rust (stable channel)
- **Build:** Cargo workspace, compiler split into crates (`ynz-ast`, `ynz-parser`, `ynz-typeck`, `ynz-mir`, `ynz-codegen`, `ynz-driver`)
- **Parser:** Hand-written recursive descent (not a parser generator)
- **Diagnostics:** `ariadne` for pretty error rendering with spans and suggestions
- **Incremental computation:** `salsa` (the framework `rust-analyzer` is built on) — gives incremental builds AND the LSP architecture
- **LLVM codegen:** `inkwell` (safe LLVM wrapper, zero-cost over the C++ API)
- **Optional alternative codegen (later):** `cranelift` for fast debug builds

---

## Why Rust over the alternatives

**Why not Zig:** Tempting (fast rebuilds, direct LLVM C API). But Zig is pre-1.0 — breaking changes on the build infrastructure are a risk. No salsa-equivalent for incremental computation. Smaller hiring pool. Debugging Zig bugs and Yinz bugs simultaneously is harder than debugging one.

**Why not TypeScript / Bun:** Fastest prototype-to-working, but the rewrite is inevitable — no production compiler ships in TypeScript. "We'll rewrite later" is the lie programmers tell themselves; the prototype always ships. Two compilers means two compilers forever. No real pattern matching. No real ADTs (`{ kind: "Foo", ... } | { kind: "Bar", ... }` works but is weaker than Rust enums). GC'd host caps performance forever.

**Why not C++:** LLVM is C++, so "no FFI overhead" is real. But `inkwell` wraps the C++ API with zero runtime cost — the marginal win isn't worth the foot-guns. Junior contributors in 2026 don't touch C++ for greenfield projects. CMake is its own subproject.

---

## Why hand-written recursive descent over a parser generator

Golden Rule 11 says "the compiler is a teacher." Compiler error messages are a first-class feature of Yinz, not an afterthought.

Parser generators (LALRPOP, chumsky, winnow) emit fast parsers, but their error recovery is generic — they say "expected `;`, got `}`" without context, position-specific suggestions, or recovery strategies. A hand-written parser can do all of these:

- **Position-specific suggestions** — when the user writes `function foo()` without a return type, the parser knows it just finished consuming the parameter list and can suggest `-> nothing` or a type at that exact span.
- **Error recovery** — skip to the next sensible boundary (next `}`, next statement) and keep parsing so the user gets ALL the errors in one compile, not just the first.
- **Multi-error reporting** — a hand-written parser naturally accumulates errors; generators tend to bail on the first.

Cost: writing the parser by hand takes longer than declaring a grammar. Acceptable cost for the error-message quality.

---

## Why Salsa for incremental computation

The compiler binary ships TWO frontends:
1. **`ynz` CLI** — `ynz build`, `ynz run`, `ynz test`
2. **`ynz-lsp`** — Language Server Protocol implementation for editors (autocomplete, hover, go-to-def, rename, inline errors)

Both frontends do the SAME work — parse, type-check, analyze. If we write two separate pipelines, they drift forever. Salsa is a demand-driven query framework: define queries (`parse(file_id) -> AST`, `type_of(node_id) -> Type`, etc.) and Salsa memoizes them and invalidates dependents when inputs change.

Same code path serves both frontends. The LSP becomes "expose existing queries over JSON-RPC" instead of "write a second compiler."

This is non-negotiable. If we don't commit to salsa from day 1, the LSP is a 6-month side-quest later instead of a few weeks.

---

## Known costs

**rustc compile times.** Real downside, mitigated by:
- Cargo workspace + small crates (recompile only what changed)
- `sccache` for shared compilation cache
- `mold` linker for fast incremental links
- `cargo check` (no codegen) for the inner-loop type-check workflow

Acceptable. The compiler-team's iteration speed is slower than a Bun-based prototype would be, but the resulting compiler is faster, smaller, and audit-friendly.

---

## Bootstrap and self-hosting

The Rust compiler is the *bootstrap* compiler. Yinz will eventually self-host — that is, the Yinz compiler will be written in Yinz. The Rust compiler keeps running as the reference implementation until the self-hosted compiler reaches feature parity. Self-hosting is a v2+ concern; the Rust compiler will live for years.

Writing the bootstrap in Rust means the compiler team absorbs good "how does an ownership-based compiler reason about itself" patterns. Writing the bootstrap in TS would teach nothing transferable to a self-hosted Yinz compiler.
