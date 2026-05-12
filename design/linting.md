# Linting & Build — Design Decisions

User spec: `spec/linting.md`, `spec/tooling.md`. Full compiler design: `design/compiler.md`

---

## Three-Tier Linting

Compiler catches problems at three levels: errors (won't compile), warnings (compiles, flag the problem), suggestions (informational, IDE only by default).

**Configurable**: `[lint] level = "relaxed" | "balanced" | "strict"` in `yinz.toml`. Default is "balanced."

**Philosophy**: Catch real bugs and enforce code quality. Don't police style preferences. Every rule prevents an actual problem. The developer should feel helped, not harassed.

**Why suggestions are IDE-only by default**: Suggestions are the most subjective tier. Showing them in terminal output during CI would be too noisy and push developers to disable the whole system. IDE-only keeps them visible during development without friction in automated pipelines.

**Why "balanced" is the default**: "Relaxed" (errors only) misses real problems. "Strict" (treat warnings as errors) blocks CI on things that may be acceptable mid-development. "Balanced" catches real bugs in CI while keeping suggestions non-blocking.

---

## Compile Speed — Design Principle

Runtime performance is never sacrificed for compile speed. But compile speed is aggressively optimized through incremental compilation, caching, and smart dependency tracking.

**Not a golden rule**: The 12 golden rules describe the language from the user's perspective. Compile speed is a compiler implementation goal. Full design: `design/compiler.md`.

**Debug vs release builds**: `ynz build` = minimal LLVM optimization, fast compile. `ynz build --release` = full LLVM optimization, slow compile, maximum runtime speed. Development uses debug. Production uses release. These are different binaries — never deploy a debug build.
