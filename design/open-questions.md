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

### Proposed model (discussed 2026-05-18 — to be formally decided at v0.5 planning)

Root-relative imports + tree shaking make most of the TypeScript monorepo machinery unnecessary. The proposed model:

- **Single `yinz.toml` at the project root** — one place for all dependencies. Tree shaking ensures each binary only contains what its entrypoint actually uses; dep listing all services' deps together doesn't inflate any individual binary.
- **`[entries]` table for named entrypoints**:
  ```toml
  [entries]
  users   = "services/users/entrypoint.ynz"
  orders  = "services/orders/entrypoint.ynz"
  gateway = "services/gateway/entrypoint.ynz"
  ```
- **CLI entry override**: `ynz build users` or `ynz build services/users/entrypoint.ynz` — building one service doesn't require a per-service toml.
- **Shared code is just a folder**: `shared/`, `lib/`, or any name. Imported root-relatively from any service. No barrel files, no path mapping, no package.json workspace:* dance.
- **Service-level dep override for the rare conflict case**: when two services genuinely need different versions of the same third-party dep, an `[entries.users.dependencies]` override table (or similar) resolves it without splitting into separate toml files.

### Cargo feature-unification problem

When workspace support is designed, the Cargo feature unification problem must be addressed explicitly: in Cargo, when multiple workspace members share a dependency, Cargo unifies all enabled features into one build — so a feature enabled by any member is enabled for all of them, even members that shouldn't have it. This causes unwanted build dependencies (e.g., needing cmake on machines building only a subset of the workspace) and binary size inflation.

The single-root-toml model above sidesteps this if Yinz has no feature-flag system (v0.1 does not). If feature flags are ever added, feature resolution must scope per-entrypoint, not unify across all entries. Don't default to unified without consciously choosing it.

---



---

## Formatter — Trailing Comment Alignment (v0.2-M3 directive)

**Locked directive for `ynz fmt` (v0.2-M3)**: trailing inline comments after real code are aligned within contiguous declaration blocks. Whole-line comments (single OR multi-line) are NOT aligned to the trailing-comment column — they stay at the surrounding indent level.

**Canonical example** (Patrick's `shape Symbol`):

```ynz
export shape Symbol {
    symbol: string          // aapl
    name: string            // apple
    assetClass: AssetClass  // stock
    exchange: string        // nyse

    sector: string
    industry: string

    float: number           // 19_822_143
    marketCap: number       // 5_712_092

    // lastSyncedAt: datetime
}
```

The four `symbol`/`name`/`assetClass`/`exchange` trailing comments align in a column. The `float`/`marketCap` pair forms a SEPARATE alignment block (blank line broke it). `// lastSyncedAt: datetime` is a whole-line comment — anchored to the field's indent column, not pushed out to align with the trailing-comment column above.

**Rules**:
1. Trailing comments (`code  // text`) within a contiguous run of declarations align to the rightmost code-end-column in the run, plus two spaces.
2. **Blank line breaks the run.** Each contiguous declaration block aligns independently.
3. **Whole-line comments** (line starts with `//`) are anchored to the surrounding indent level. They do NOT extend or join the trailing-comment alignment.
4. **Multi-line whole-line comment blocks** (consecutive `//` lines) stay at indent level — never aligned to a trailing-comment column.
5. Applies inside `shape` field declarations, `options` variants, `const` blocks of homogeneous declarations, and any other "table-like" declaration context. Does NOT apply inside function bodies (statements don't form alignment groups — each statement is its own thing).

**Why this is locked, not configurable**: per v0.2 constraint "`ynz fmt` is opinionated, zero-config" (roadmap `v0-2-dev-loop-tooling.md`). Alignment is either on always or off always. Patrick picked on (this directive).

**Why on always**: Yinz uses `shape` declarations heavily, fields are usually short, trailing comments are usually short "what this IS" hints. The columnar layout reads as a table. Diff-noise cost (re-flow on field add/remove) is the formatter's job to absorb, not the user's. Mixed-length-field edge case (one long field name pushes the column out to 60+) is an acceptable cost — the alternative (no alignment, ever) gives up the common-case readability win to avoid an uncommon edge case.

**Future config (NOT v0.2)**: if real Yinz codebases hit the mixed-length edge case often enough to justify it, a future version may add a per-file or per-block opt-out. v0.2 ships on-always with no opt-out per the no-config rule. Trigger to revisit: a documented pattern of Yinz codebases working around the alignment (e.g., reordering fields by length, splitting blocks artificially to avoid alignment cascade). Until that pattern emerges, no config.

**M3 research phase still locks**:
- Algorithm for finding the contiguous-declaration-block boundaries (probably: same indent level + same statement-kind + no blank lines between)
- Tab-vs-space padding character (probably: spaces only, per most modern formatters — but research phase confirms)
- Behavior when a comment line is longer than line-width (probably: alignment column wins; line wraps after the comment text; research phase confirms)
- Interaction with `///` doc comments — doc comments on fields are whole-line and stay at indent column per rule 3; trailing `///` is rare and probably banned at the lexer level for fields (a doc comment on a field belongs above the field, not after it)

**Cross-references**: `v0-2-dev-loop-tooling.md` v0.2-M3 milestone scope; `design/mvp-scope.md` v0.2 entry (no-config opinionated formatter); `design/doc-comments.md` (`///` syntax — doc comments are anchored, not aligned).

---

## CLI Flags Planned for Future Versions

Tracked so the help text reflects what currently ships and the design log remembers what's coming.

- `--release` — release-mode codegen (LLVM `-O3`, no debug info, smaller binary).
  Target version: v0.X (TBD; tied to dedicated optimization pass milestone).
- `--kernel` — kernel-mode build (no heap allocator, no panic-on-OOM, no threading).
  See `design/future/no-runtime-mode.md`. Target version: v0.3 per `design/mvp-scope.md`.
- `--reveal-sensitive` — show all sensitive values in output (dev-only; stripped from release builds).
  Target version: pending audit batch. Design is locked in `design/sensitive.md`.

These flags are NOT in the v0.1 driver. Do not document them in `--help` until they ship.

---

## Inline / Anonymous Shape Types — Single-Use Structural Types

**Status**: design open, needs plan pass. Triggered by Patrick hitting verbosity friction defining one-off shapes during testing (2026-05-19).

**The friction**: named `shape Foo { ... }` is the right tool when a type is used multiple times, but single-use types (a config struct for one function, an intermediate result that never leaves a loop) force scrolling to the top of the file. Type definition is physically separated from its only use.

**Proposed syntax** (TypeScript-style):

```ynz
const intervals: fixed<{ minutes: int, timeframe: Timeframe }> = [
    { minutes: 5,  timeframe: Timeframe.fiveMinute },
    { minutes: 60, timeframe: Timeframe.hourly },
]
```

**Blocking design call**: structural vs nominal for anonymous types. If Yinz stays nominal, inline types are just sugar for an anonymous-but-fresh shape (2-day feature). If Yinz adopts structural for anonymous types (TypeScript's model), two identical inline shapes are interchangeable across files — bigger type-system extension.

**Full design doc**: `design/future/inline-shape-types.md` (4 open design questions catalogued there).

**Target version**: language feature, v0.1.x or post-v0.2 language slot. NOT v0.2 (tooling-only).

---

## Type Collection Ordering — Options/Shapes Must See Each Other (v0.2+)

`collect_shapes` and `collect_options` run as separate passes in the wrong order. `collect_shapes` runs first (in `module_signatures_query`) but needs to know about options type names to resolve field types like `timeframe: Timeframe`. `collect_options` runs later inside `check_query`.

**Current workaround** (2026-05-18): `collect_shapes` does a same-file pre-scan for `OptionsDecl` names and stores them in `ShapeTable.options_names`. Works for same-file only. Cross-file imported options types in shape fields still fail.

**Proper fix**: refactor to a single "type name collection" pre-pass that runs before any field type resolution:
1. Scan all module items (shapes, options, imports) to build a full `TypeNameRegistry`
2. Pass that registry to `collect_shapes` and `collect_options` instead of each doing their own pre-scans
3. Field type resolution then has full visibility of all type names regardless of file or declaration order

**Trigger**: a user creates a multi-file project where an imported options type is used in a shape field annotation. The workaround produces "not a known type" diagnostic for the imported options type.

**Removal**: when the proper fix ships, `ShapeTable.options_names` and its pre-scan in `collect_shapes` are deleted — they're superseded by the global type registry.
