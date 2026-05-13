# Deferral Ledger

Every feature that v0.1 does NOT ship, with explicit substitute and trigger conditions for when it lands. Per `~/.claude/rules/no-duct-tape.md`: a legitimate deferral names what's deferred, why, what substitutes for it in the meantime, and what flips the calculus.

Cross-referenced from any spec / design file that touches a deferred feature.

---

## Sized integer variants (`int<N>`, `uint<N>` for N != 64)

**What:** Angle-bracket-parameterized sized integers (`int<8>`, `int<16>`, `int<32>`, `uint<8>`, `uint<16>`, `uint<32>`, `uint<64>`, etc.) and signed equivalents.

**Why deferred:**
- Requires const generics (numeric values as type parameters) — a meaningful compiler sub-project that took Rust years to land well.
- v0.1 users have no legitimate need: FFI is deferred, binary protocols can use byte arrays + stdlib helpers.

**v0.1 substitute:** Use plain `int` (= i64) for all whole numbers. Covers ±9.2 × 10^18 — bigger than any count a human writes by hand. Precision loss vs sized variants only matters at FFI boundaries, which aren't in v0.1.

**Trigger to land:** Either (a) FFI work begins, OR (b) a real user workload needs to interop with a binary protocol that v0.1's byte-array helpers can't ergonomically handle.

**Planned syntax when it lands:** `int<N>` / `uint<N>` where N is a compile-time integer (probably 8, 16, 32, 64, 128, 256, 512). Matches existing angle-bracket convention (`array<T>`, `map<K,V>`, `fixed<T>`, `number<N>`).

**Cross-references:** `spec/numeric-types.md`, `design/numeric-types.md`

---

## Sized float variants (`f32`)

**What:** Single-precision (32-bit) IEEE 754 binary float.

**Why deferred:**
- Only essential for GPU compute and ML workloads (both v2+).
- For graphics/physics in v0.1, `float` (= f64) is overkill but works fine.

**v0.1 substitute:** Use `float` (f64) for all binary-float math. Slower than f32 on SIMD-heavy workloads but correct.

**Trigger to land:** GPU dispatch begins OR ML stdlib begins.

**Planned syntax when it lands:** Possibly `float<32>` to match the angle-bracket convention, OR a built-in `f32` type — decide when the use case is concrete.

**Cross-references:** `spec/numeric-types.md`, `design/gpu.md`

---

## Arbitrary-precision decimal beyond `number<4096>`

**What:** `number` precision larger than 4096 significant digits — true unbounded decimal arithmetic.

**Why deferred:**
- 4096 digits handles every realistic scientific calculation (gravitational wave numerics top out at ~200 digits; even number-theory research rarely exceeds 2000).
- Unbounded precision means unbounded memory per value and unbounded per-operation time — breaks the language's "predictable performance" character.
- Real arbitrary-precision libraries (GMP, MPFR) are massive projects.

**v0.1 substitute:** Use `number<N>` with N up to 4096. Compile error if a user tries `number<5000>` — error message explicitly points to this deferral.

**Trigger to land:** A real user workload genuinely exceeds 4096 digits AND can't be restructured to fit. This is a deliberately high bar.

**Planned form when it lands:** Either a separate `bignumber` type with heap-allocated coefficient, OR `number<arbitrary>` as a keyword in the angle-bracket position.

**Cross-references:** `spec/numeric-types.md`, `design/numeric-types.md`

---

## Auto-parallelization optimization

**What:** The compiler's automatic dependency-graph analysis that schedules independent operations to run in parallel without `wait`/`background` keywords. This is the "default fast" Yinz promise — most code is automatically parallel.

**Why deferred (only to v0.3, not v2+):** The dependency analysis is a significant sub-project. Shipping v0.1 + v0.2 with the optimization deferred means users get the SYNTAX (`wait`, `background` keywords parse, type-check, and have correct sequential semantics) but not the SPEED. Their code still WORKS — it just runs sequentially.

**v0.1/v0.2 substitute:** Sequential execution. All `wait` calls happen in order. All `background` calls run on a single thread. Correct behavior, no parallelism gain.

**Trigger to land:** v0.3 release.

**Cross-references:** `spec/concurrency.md`, `design/concurrency.md`, `design/mvp-scope.md`

---

## Within-File Test Parallelization and Cross-File Resource Locks

**What:** `parallel file` declaration to enable within-file test parallelism. `sequential "resource-name"` declarations to serialize files that share a resource (e.g., two test files both writing to the `users` DB table).

**Why deferred:** v0.13 ships with file-level parallelism only. The 95% case is "files in parallel, tests within a file sequential" and it works fine. Adding refinements upfront creates complexity for everyone to solve a problem only large test suites hit.

**v0.13 substitute:** Users are responsible for test isolation. `setup file { db.connect(...) }` opens a per-file connection. Files that genuinely share state should be designed differently or use `--serial` mode.

**Trigger to land:** v0.14+ if real demand surfaces (e.g., a project with massive test files that need within-file parallelism, or users reporting they can't isolate cross-file state cleanly).

**Cross-references:** `design/testing.md`, `spec/testing.md`

---

## Public Package Registry

**What:** Server-side infrastructure for hosting and serving Yinz packages — the `ynz add some-package` discovery + download flow against a public registry.

**Why deferred to v1.2 (not v0.5 with the package manager):** The language isn't publicly launched until v1.0. Before launch, breaking changes are fine; there's no community of authors to support. After v1.0 stabilizes, building the registry — in Yinz itself, as the project's first major dogfooding test — proves the language can build real services.

**v0.5–v1.1 substitute:** Package manager supports git URLs and local paths. `ynz add github:user/repo` works fine. Public registry isn't required for the package manager to be useful.

**Trigger to land:** v1.2 milestone, after v1.0 launch stabilizes.

**Cross-references:** `design/packages.md`, `design/mvp-scope.md`

---

## FFI (Foreign Function Interface)

**What:** The `foreign` keyword and machinery to call C / C++ / Rust libraries from Yinz.

**Why deferred:**
- Significant design surface (ownership across the boundary, type mapping, safety guarantees).
- Most v0.1 code doesn't need it — stdlib is in scope.
- Stdlib internals can use compiler-private FFI without exposing a user-facing `foreign` keyword.

**v0.1 substitute:** Stdlib modules that need C interop (file I/O, networking, math) call C internally via compiler-private mechanisms. Users don't see this. If a user genuinely needs to call a third-party C library, they have to wait for v2+ or contribute to the stdlib.

**Trigger to land:** v2+ work begins OR a stdlib gap creates a real need.

**Cross-references:** `spec/ffi.md`, `design/ffi.md`, `design/open-questions.md`

---

## GPU dispatch

**What:** The `gpu` call-site keyword and kernel compilation to GPU shader / compute languages.

**Why deferred:**
- Massive scope (kernel compilation, ABI design, runtime fallback).
- No v0.1 user has shown demand.
- Was tagged MVP2+ in original design.

**v0.1 substitute:** None — `gpu` keyword is reserved but not parseable. Compile error if used.

**Trigger to land:** v2+ AND a real ML/compute workload requires it.

**Cross-references:** `spec/concurrency.md`, `design/gpu.md`

---

## ML stdlib

**What:** Tensors, neural network primitives, autodiff, optimizers.

**Why deferred:** v0.1 stdlib focus is general-purpose. ML requires its own deep design (compatible with `float`/`f32`, GPU dispatch, etc.) and is a v2+ concern.

**v0.1 substitute:** None. ML workloads run in Python until then.

**Trigger to land:** v2+ AND GPU dispatch lands.

**Cross-references:** `design/stdlib/ml.md`

---

## Markets stdlib

**What:** Financial data ingestion, brokerage integrations, market data feeds.

**Why deferred:** Niche stdlib module. Not load-bearing for v0.1.

**v0.1 substitute:** None. Users write HTTP-based integrations directly.

**Trigger to land:** v2+.

**Cross-references:** `design/stdlib/markets.md`

---

## Operator overloading

**What:** User-defined types can `follows Add`, `follows Subtract`, etc. and use `+`, `-`, `*`, `/` operators.

**Why deferred:** Not load-bearing for v0.1. Built-in numeric types use the built-in operators just fine. Custom-type overloading is polish, not core.

**Substitute:** Users with custom math types write `.add()`, `.subtract()` methods explicitly. Verbose but works.

**Trigger to land:** v1.0 (public launch milestone).

**Cross-references:** `spec/operators.md`, `design/operators.md`, `design/mvp-scope.md`

---

## Custom iterables (`follows Iterable<T>` / `follows FallibleIterable<T>`)

**What:** User types can implement `Iterable<T>` or `FallibleIterable<T>` and be iterated with `for`.

**Why deferred:** Built-in `for` over collections (`array`, `fixed`, `map`, ranges) works without this. Built-in `for` over fallible iterables like `file.lines()` works in v0.6. Custom user types implementing the contracts is the extension that ships at v1.0.

**Substitute:** Users with iterable-like data expose a `.items()` method returning `array<T>` and `for (item in foo.items())`. Lossy compared to true iteration (materializes the whole collection) but works.

**Trigger to land:** v1.0.

**Cross-references:** `spec/iterables.md`, `design/iterables.md`, `design/mvp-scope.md`

---

## Self-hosted compiler

**What:** The Yinz compiler rewritten in Yinz (current bootstrap is in Rust).

**Why deferred:** Need feature parity in v1.0+ stable Yinz first. Bootstrap-in-Rust serves us for years.

**v0.1 substitute:** Rust bootstrap compiler — see `design/compiler-language.md`.

**Trigger to land:** v2+ AND the language is stable enough to self-host.

**Cross-references:** `design/compiler-language.md`

---

## Deprecation marking

**What:** A way to mark stdlib functions / language features as deprecated, with compiler warnings on use.

**Why deferred:** Only relevant post-v1.0 when backwards-compatibility kicks in. v0.1 follows the no-backwards-compatibility-pre-release policy — breaking changes are fine.

**Substitute:** None needed.

**Trigger to land:** v1.0 release (or shortly after).

**Cross-references:** `design/versioning.md`, `design/linting.md`

---

## `ynz doc` and `ynz repl`

**What:**
- `ynz doc` — generate static API documentation from `///` doc comments
- `ynz repl` — interactive REPL for learning and exploration

**Why deferred:** Polish tooling. Not blocking development or language usability. Post-launch additions.

**Substitute:** No static doc generation (read the source). No REPL (write a small script and `ynz run` it).

**Trigger to land:** v1.1 (post-launch polish milestone).

**Cross-references:** `spec/doc-comments.md`, `design/mvp-scope.md`

---

## Lint Customization Config

**What:** The `[lint]` section in `yinz.toml` becomes configurable — disable rules, adjust severity per rule, tune rule parameters (e.g., `max-function-length = 75`), define pattern-based custom rules, or disable built-in linting entirely (`enabled = false`).

**Why deferred to v1.x:** v0.4 ships the linting tier with curated defaults. Customization adds a configuration surface that should be designed against real usage patterns — too early creates a config syntax we're stuck with.

**Substitute (v0.4–v1.0):** Curated default rule set per `design/linting.md`. Cannot be customized; take-it-or-leave-it.

**Trigger to land:** v1.x (exact version decided based on demand — v1.3? v1.5?).

**Cross-references:** `design/linting.md`, `design/mvp-scope.md`

---

## How to add a new deferral

When a feature is decided to defer:

1. Add an entry to this file with all five sections: What / Why / Substitute / Trigger / Cross-references.
2. Cross-link from every spec or design file that mentions the deferred feature.
3. If there's a `design/open-questions.md` entry that becomes "deferred, not unresolved," remove it from open-questions and reference the deferral entry here.

If you can't write a concrete substitute or a concrete trigger, the deferral isn't legitimate — the feature must either ship or have its scope re-examined.
