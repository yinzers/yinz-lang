---
name: "SCRATCH-future-release-mode"
description: "Design notes for a future '--release' compiler flag that enables LLVM optimizations, strips debug info and dev-only diagnostics, and hardens panic/overflow behavior for production builds."
tags:
  - "yinz-compiler"
created_at: "2026-05-19"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# `--release` Flag for Optimized Builds

**Status**: Locked, target version TBD (tied to a dedicated optimization pass milestone — probably v0.4 alongside the linting tier, or a later perf-focused slot if scope demands).

User spec target: [`docs/reference/REF-tooling.md`](../../reference/REF-tooling.md) gets a `--release` flag section when implemented.

---

## The Decision

Yinz separates "dev builds" (the default — fast compile, debug info, all teaching diagnostics) from "release builds" (optimized for production — slower compile, smaller binary, dev-only features stripped).

Mechanism: a `--release` compiler flag on `ynz build` (and by extension `ynz run` if relevant) that:

1. **Enables LLVM optimization passes** — `-O3` (or whatever LLVM level proves to be the right default after benchmarking)
2. **Strips debug info** — smaller binary, no per-symbol debug tables
3. **Disables dev-only flags** — `--reveal-sensitive`, `--emit-ir`, and any future debug-only output flags either become no-ops or fail with a teaching error when combined with `--release`
4. **Hardens panic / overflow behavior** — `cfg!(debug_assertions)` equivalents flip; e.g., the SipHash zero-key fallback that's acceptable in dev becomes an unconditional `/dev/urandom` seed in release

---

## Why Yinz needs this

Two reasons, in order of importance:

1. **Production safety**. Dev-only flags like `--reveal-sensitive` that exist to print raw values from `sensitive` types must NOT survive into release builds. Today the runtime checks an environment variable at startup. Stripping that env-var check at compile time (under `--release`) is the only way to guarantee no accidental production leak.

2. **Binary size + runtime perf**. LLVM's `-O3` typically buys 2-5× perf on numeric workloads, 10-20% on general code, and 30-50% reduction in binary size when combined with `-flto` + symbol stripping. Real users running Yinz in production will care.

---

## Why this is deferred

The default Yinz compile path (`ynz build`, no flag) optimizes for **fast feedback** — quick compile, clean diagnostics, debug-friendly output. Most pre-v0.4 users will spend 95%+ of their time in this mode. Premature `--release` work distracts from getting the dev experience right first.

Triggers to implement:
- v0.4 perf work begins (probably the dedicated optimization-pass milestone)
- A real user runs Yinz in production and hits perf or binary-size friction
- Combined with a `--kernel` build (per [`docs/internal/implementation/IMP-no-runtime-mode.md`](../implementation/IMP-no-runtime-mode.md)), since kernel-mode usually wants the same optimization treatment

---

## Open sub-questions for the implementation milestone

- LLVM opt level: `-O2` or `-O3`? `-O3` is more aggressive (autovectorization, more inlining) but increases compile time substantially. `-O2` is the conservative pick and what most Rust crates use by default.
- LTO (link-time optimization): on or off in release builds? Trades compile time for smaller binary + faster runtime.
- Strip symbols by default in release? Convenient for production; bad for crash debugging. Decide based on whether Yinz has a crash-reporting story by then.
- Cross-compile: does `--release --target <triple>` work in v0.4, or wait until cross-compilation lands separately?
- Reproducible builds: does release mode pin LLVM version + opt-pass sequence + binary embedding so two builds of the same source produce byte-identical output? Probably yes, but worth confirming.

---

## Stripping dev-only flags from release builds

Concrete pattern Yinz will use when this lands:

```rust
// In the driver / runtime:
#[cfg(not(release_build))]
fn check_reveal_sensitive_env_var() -> bool {
    std::env::var("YNZ_REVEAL_SENSITIVE").is_ok()
}

#[cfg(release_build)]
fn check_reveal_sensitive_env_var() -> bool {
    false  // hard-coded: dev-only flag stripped from release
}
```

The `release_build` cfg gets set by the driver when `--release` is passed. Tests cover both modes.

This means `--reveal-sensitive` and any future dev-only flags need a corresponding "release-mode stub" decision at implementation time. Tracking this as a forward-compat invariant.

---

## Cross-references

- [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) — v0.4+ deferred features section (this entry)
- [`docs/internal/implementation/IMP-no-runtime-mode.md`](../implementation/IMP-no-runtime-mode.md) — `--kernel` flag (often combined with `--release`)
- [`docs/internal/scratchpad/SCRATCH-open-questions.md`](SCRATCH-open-questions.md) "CLI flags planned" — release/kernel/emit-ir entries
- [`docs/reference/REF-sensitive.md`](../../reference/REF-sensitive.md) — `--reveal-sensitive` flag (stripped from release per this design)
