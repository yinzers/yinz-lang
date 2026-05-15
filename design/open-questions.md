# Open Questions

Design decisions that haven't been made yet. When one is resolved, move it to its dedicated design file and remove the entry here.

Resolved questions are NOT listed here — see `design/decisions.md` for the index of resolved topics.

Deferred features (decided not to ship in v0.1) are documented per-version in `design/mvp-scope.md` under the version where they're planned to land.

---

## Metaprogramming Mechanism — Commit to ONE

**Decision deferred; commitment locked**: when Yinz adds metaprogramming (compile-time code generation, derive macros, attribute macros, or any user-defined compile-time code transformation), it will ship **exactly one** mechanism. Not two systems "for different use cases." Not a v1 with a later v2 layered on top.

**Why this commitment is locked now even though the choice isn't**: Rust shipped `macro_rules!` with 1.0 (2015), then procedural macros in 1.30 (2018), and is developing a third system (declarative macros 2.0). RFC 1584 explicitly states the second system was designed because "there are several changes to the declarative macro system which are desirable but not backwards compatible" (https://rust-lang.github.io/rfcs/1584-macros.html). Result: every Rust developer must learn both systems to read existing crates; tooling (rust-analyzer, IDE expansion, debugger) had to be rewritten for each; the two systems will coexist forever. The cost compounds with every year and crate added.

The Rust lesson: a second mechanism is never additive — it splits the learning surface, the tooling, and the ecosystem.

**Trigger to decide which mechanism**: when a real Yinz use case needs metaprogramming AND cannot be handled by the compiler's built-in codegen (per `.claude/rules/stdlib-design.md` Rule 6 for serialization), pick the single mechanism. Candidate models to evaluate at decision time:
- **Compiler-driven derives only** (no user-facing macro syntax): user marks a `shape` with a derive attribute; compiler generates the implementation. Simpler, less expressive.
- **Token-level macros** (Rust proc-macro style): user-defined functions that transform AST. More expressive, more complex tooling.
- **Comptime evaluation** (Zig/D style): functions marked `comptime` execute at compile time, can generate code as values. Different mental model, potentially debugger-friendly.

When the trigger fires (a real use case needs metaprogramming), Patrick decides among these (or adds a candidate). Until then, this stays in open-questions as a locked commitment to "exactly one."

**Cross-references**: `lockin-stdlib-and-syntax.md` Finding #22 for the Rust dual-system pain.

---

## HTTP Module Design — Three-Tier API

When v0.15 (`http` client) comes up for implementation, the module needs a focused design session. The general shape is locked but specifics need work:

**Tier 1 — High-level helpers:** `http.get(url)`, `http.post(url, body)`, `http.put`, `http.delete`, `http.websocket(url)`. The common-case API, dot-method-first.

**Tier 2 — Mid-level request builder:** `http.request().method("GET").header(...).timeout(5).send()` for cases that don't fit the simple helpers. Step-by-step style — every operation gets its own line.

**Tier 3 — Low-level socket access:** `net.tcp.connect(host, port)` returning a raw socket. The floor of the user-accessible network stack. Framework authors can build their own routing layer, their own protocol implementations, anything on top of this. Going lower means FFI (deferred to v2+).

**Open sub-questions for the v0.15 design session:**

- Exact dot-method names for the high-level tier (`http.get` vs `http.fetch` vs `http.request`?)
- WebSocket lifecycle — connect, message events, close handling. Builds on FallibleIterable contract per `design/iterables.md`?
- TLS configuration — defaults to validating certs; how to opt out for self-signed in dev?
- Streaming bodies — for large uploads/downloads, can the body be an iterable?
- Cookie handling — first-class or always manual headers?
- Proxy support — env var detection (`HTTP_PROXY`) automatic, or always explicit?

These get answered when v0.15 is up to design. Not blockers for v0.1-v0.14.

---

## Actor Primitives (if ever added)

If Yinz adds actor primitives beyond the current `background` + supervisor model: type messages from day 1. Akka's migration from untyped to typed actors (2009→2019) required near-complete rewrites of all actor code. Retrofitting type safety onto untyped message passing is a full rewrite, not an upgrade.

---

## Specialization (if `follows` ever needs it)

Rust's specialization (RFC 1210, 2015) has been unstable for 11 years because it creates a soundness hole: interaction with lifetime dispatch allows deriving `'static` from non-`'static` references without `unsafe`. The stdlib uses a restricted internal subset (`min_specialization`) but users cannot.

Yinz's `follows` constraints are simpler — no blanket impls, no `impl<T: Bound> Contract for T` patterns in v0.1. Specialization may never be needed. If it ever is, the Rust soundness research (Ralf Jung, Aaron Turon) must be reviewed first; don't ship specialization without resolving the lifetime-dispatch interaction.

---

## Workspace / Multi-Package Projects (v0.5+)

When workspace support is designed, the Cargo feature unification problem must be addressed explicitly: in Cargo, when multiple workspace members share a dependency, Cargo unifies all enabled features into one build — so a feature enabled by any member is enabled for all of them, even members that shouldn't have it. This causes unwanted build dependencies (e.g., needing cmake on machines building only a subset of the workspace) and binary size inflation.

The Yinz workspace design must decide upfront: does feature resolution scope per-member or does it unify? Per-member is more correct; unified is simpler to implement. Don't default to unified without consciously choosing it.

---


