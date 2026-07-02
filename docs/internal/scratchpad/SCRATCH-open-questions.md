---
name: "SCRATCH-open-questions"
description: "Design decisions that haven't been made yet. When one is resolved, move it to its dedicated design file and remove the entry here."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "scratchpad"
---

# Open Questions

Design decisions that haven't been made yet. When one is resolved, move it to its dedicated design file and remove the entry here.

Resolved questions are NOT listed here — see [`docs/README.md`](../../README.md) for the index of resolved topics.

Deferred features (decided not to ship in v0.1) are documented per-version in [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) under the version where they're planned to land.

---

## Metaprogramming Mechanism — Commit to ONE

**Decision deferred; commitment locked**: when Yinz adds metaprogramming (compile-time code generation, derive macros, attribute macros, or any user-defined compile-time code transformation), it will ship **exactly one** mechanism. Not two systems "for different use cases." Not a v1 with a later v2 layered on top.

**Why this commitment is locked now even though the choice isn't**: Rust shipped `macro_rules!` with 1.0 (2015), then procedural macros in 1.30 (2018), and is developing a third system (declarative macros 2.0). RFC 1584 explicitly states the second system was designed because "there are several changes to the declarative macro system which are desirable but not backwards compatible" (https://rust-lang.github.io/rfcs/1584-macros.html). Result: every Rust developer must learn both systems to read existing crates; tooling (rust-analyzer, IDE expansion, debugger) had to be rewritten for each; the two systems will coexist forever. The cost compounds with every year and crate added.

The Rust lesson: a second mechanism is never additive — it splits the learning surface, the tooling, and the ecosystem.

**Trigger to decide which mechanism**: when a real Yinz use case needs metaprogramming AND cannot be handled by the compiler's built-in codegen (per [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 6 for serialization), pick the single mechanism. Candidate models to evaluate at decision time:
- **Compiler-driven derives only** (no user-facing macro syntax): user marks a `shape` with a derive attribute; compiler generates the implementation. Simpler, less expressive.
- **Token-level macros** (Rust proc-macro style): user-defined functions that transform AST. More expressive, more complex tooling.
- **Comptime evaluation** (Zig/D style): functions marked `comptime` execute at compile time, can generate code as values. Different mental model, potentially debugger-friendly.

When the trigger fires (a real use case needs metaprogramming), Patrick decides among these (or adds a candidate). Until then, this stays in open-questions as a locked commitment to "exactly one."

**Cross-references**: `lockin-stdlib-and-syntax.md` Finding #22 for the Rust dual-system pain.

---

## Network Module Design — `request` and `server` (HTTP, both directions)

When v0.15 (`request`, outbound) and v0.21 (`server`, inbound) come up for implementation, each needs a focused design session. General shape is locked; specifics need work.

### `request` (v0.15) — outbound, three-tier

**Tier 1 — High-level helpers:** `request.get(url)`, `request.post(url, body)`, `request.put(url, body)`, `request.delete(url)`, `request.websocket(url)`. The common-case API, dot-method-first.

**Tier 2 — Mid-level builder:** `request.build()` returns a configurable Request value; the caller mutates it step-by-step (`req.method("PATCH")`, `req.header(name, value)`, `req.timeout(5)`, then `req.send()`). No method chaining per Golden Rule 7.

**Tier 3 — Low-level socket access:** `net.tcp.connect(host, port)` returning a raw socket. The floor of the user-accessible network stack. Framework authors can build their own routing layer, their own protocol implementations, anything on top of this. Going lower means FFI (deferred to v2+).

**Open sub-questions for the v0.15 design session:**

- Exact dot-method names for the builder configuration (`.method(...)` vs `.setMethod(...)`?, `.header(name, value)` vs `.headers({...})` plural?)
- WebSocket lifecycle — connect, message events, close handling. Builds on FallibleIterable contract per [`docs/internal/implementation/IMP-iterables.md`](../implementation/IMP-iterables.md)?
- TLS configuration — defaults to validating certs; how to opt out for self-signed in dev?
- Streaming bodies — for large uploads/downloads, can the body be an iterable?
- Cookie handling — first-class or always manual headers?
- Proxy support — env var detection (`HTTP_PROXY`) automatic, or always explicit?

### `server` (v0.21) — inbound

Module-singleton API: `server.route(method, path, handler)`, `server.middleware(fn)`, `server.listen(port)`. Shares `Request` / `Response` types with `request`.

**Open sub-questions for the v0.21 design session:**

- Handler signature shape — single `Request` argument vs split (`req`, `params`, `body`)?
- Middleware composition — ordering rules, short-circuit / abort semantics
- Path matching — `:id` placeholders, wildcard segments, regex routes?
- Multi-server-per-process — single singleton enough, or do we need a `Server` instance type for niche cases (admin port + public port)?
- Error handler hook — how does an unhandled `errors` propagation surface as a 5xx?
- Static file serving — bundled in `server`, or separate module?

These get answered when each version is up to design. Not blockers for v0.1-v0.14.

---

## Actor Primitives (if ever added)

If Yinz adds actor primitives beyond the current `background` + supervisor model: type messages from day 1. Akka's migration from untyped to typed actors (2009→2019) required near-complete rewrites of all actor code. Retrofitting type safety onto untyped message passing is a full rewrite, not an upgrade.

---

## Specialization (if `follows` ever needs it)

Rust's specialization (RFC 1210, 2015) has been unstable for 11 years because it creates a soundness hole: interaction with lifetime dispatch allows deriving `'static` from non-`'static` references without `unsafe`. The stdlib uses a restricted internal subset (`min_specialization`) but users cannot.

Yinz's `follows` constraints are simpler — no blanket impls, no `impl<T: Bound> Contract for T` patterns in v0.1. Specialization may never be needed. If it ever is, the Rust soundness research (Ralf Jung, Aaron Turon) must be reviewed first; don't ship specialization without resolving the lifetime-dispatch interaction.

---

## Workspace / Multi-Package Projects (v0.22+)

### Proposed model (discussed 2026-05-18, ships convention locked 2026-05-20)

Root-relative imports + tree shaking make most of the TypeScript monorepo machinery unnecessary. The proposed model:

- **Single `yinz.toml` at the project root** — one place for all dependencies. Tree shaking ensures each binary only contains what its entrypoint actually uses; dep listing all services' deps together doesn't inflate any individual binary.
- **`[entries]` table for named entrypoints**:
  ```toml
  [entries]
  calculator = "ships/calculator/entrypoint.ynz"
  greeter    = "ships/greeter/entrypoint.ynz"
  gateway    = "ships/gateway/entrypoint.ynz"
  ```
- **CLI entry override**: `ynz build calculator` or `ynz build ships/calculator/entrypoint.ynz` — building one ship doesn't require a per-ship toml.
- **Shared code is just a folder**: `shared/`, `lib/`, or any name. Imported root-relatively from any ship. No barrel files, no path mapping, no package.json workspace:* dance.
- **Ship-level dep override for the rare conflict case**: when two ships genuinely need different versions of the same third-party dep, an `[entries.calculator.dependencies]` override table (or similar) resolves it without splitting into separate toml files.

### Vocabulary — "ships"

Each named entry in `[entries]` is a **ship**. The fleet metaphor matches the model: each ship is one binary that sails independently to its users, but they're all built from one project, share one yinz.toml, and share one lockfile.

The **`ships/` folder** is the canonical convention for housing each ship's entrypoint:

```
project/
  yinz.toml              # [entries] declares the fleet
  shared/                # plain folder, root-relative imports
  ships/
    calculator/entrypoint.ynz
    greeter/entrypoint.ynz
```

**The folder name is a convention, not a requirement.** `[entries]` accepts any path; `apps/`, `services/`, `bin/`, anything works. But the canonical Yinz convention — used in scaffolding (`ynz ship new <name>` when it lands), examples (`examples/stadium-fleet/`), and docs — is `ships/`.

Pittsburgh-flavored pick (Pirates), 5-letter typo-resistant, library-ship vs binary-ship distinction reads cleanly, and `ynz ship new` is good CLI ergonomics.

### Concrete example

See `examples/stadium-fleet/` for a runnable-once-v0.22-lands demo project showing a single `yinz.toml` with two binary ships, one shared folder, root-relative cross-folder imports, and the build commands the `[entries]` table enables.

Single-entry layout (the v0.1 default, ~95% case) is demonstrated by `examples/pirates-roster/` — same one-yinz.toml model but with `entry =` instead of `[entries]`, and no `ships/` folder needed. See [`examples/README.md`](../../../examples/README.md) for when to pick which.

### What v0.22 has to add to the compiler

1. `[entries]` parsing in yinz.toml (alongside / mutually exclusive with single `entry =`).
2. `ynz build <name>` arg-or-flag plumbing that resolves a name from `[entries]` to a path.
3. `ynz build` (no arg) loops through every `[entries]` member and produces each binary.
4. Default-build-target rule: if only `entry =` is set, today's behavior; if only `[entries]` is set, no-arg build builds all; if both are set, the table wins.
5. Workspace-root discovery from a subdirectory: walk up to find the `yinz.toml`, then resolve paths relative to that.

These are all incremental over today's single-entry compiler. No new IR, no new typeck pass — just driver/manifest/CLI plumbing.

### Diagnostic: pre-v0.22 builds of a multi-entry project

**Observation captured 2026-05-20 from sweeping `examples/stadium-fleet/`** (the multi-entry preview demo). When today's single-entry compiler is pointed at a directory that contains multiple `entrypoint()` functions — which is the natural shape of a multi-entry project even before `[entries]` parsing lands — it produces this diagnostic:

> Error: Project has more than one `entrypoint` function — defined in `…/ships/concessions/entrypoint.ynz` and `…/ships/scoreboard/entrypoint.ynz`.

That's *technically correct* (the single-entry rule says exactly one entry per project), but it's the wrong **teaching** message for someone who's intentionally trying out the multi-entry layout per `examples/stadium-fleet/`. They get told "you have too many entrypoints" instead of "you're using the multi-entry layout; `[entries]` table parsing arrives in v0.22."

**v0.22 fix**: when the parser sees a project with no `entry =` but with multiple files containing `entrypoint()` functions, OR when it sees a `[entries]` table at all, the diagnostic should switch from "more than one `entrypoint`" to a multi-entry-aware path:

- If `[entries]` is present in yinz.toml → parse it, build the named entries (the new behavior).
- If multiple `entrypoint` functions exist with no `[entries]` table → emit a teaching diagnostic that points the user at `examples/stadium-fleet/` and [`examples/README.md`](../../../examples/README.md) for the multi-entry layout, suggesting they add an `[entries]` table.

The current "more than one entrypoint" error message is a candidate to rewrite as a `entries-table-suggestion` lint that fires post-parse, ahead of the single-entry uniqueness check, so the user gets the welcoming "did you mean multi-entry?" message rather than the harsh "too many entrypoints" error.

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

**Cross-references**: `v0-2-dev-loop-tooling.md` v0.2-M3 milestone scope; [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md) v0.2 entry (no-config opinionated formatter); [`docs/internal/implementation/IMP-doc-comments.md`](../implementation/IMP-doc-comments.md) (`///` syntax — doc comments are anchored, not aligned).

---

## CLI Flags Planned for Future Versions

Tracked so the help text reflects what currently ships and the design log remembers what's coming.

- `--release` — release-mode codegen (LLVM `-O3`, no debug info, smaller binary).
  Target version: v0.X (TBD; tied to dedicated optimization pass milestone).
- `--kernel` — kernel-mode build (no heap allocator, no panic-on-OOM, no threading).
  See [`docs/internal/implementation/IMP-no-runtime-mode.md`](../implementation/IMP-no-runtime-mode.md). Target version: v0.3 per [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md).
- `--reveal-sensitive` — **SHIPPED (2026-05-19 morning batch)**. Driver propagates `YNZ_REVEAL_SENSITIVE=1` to child process; runtime OnceLock reads it. Release-build stripping deferred to v0.X when `--release` flag lands.

These flags are NOT in the v0.1 driver. Do not document them in `--help` until they ship.

---

<!-- Inline / Anonymous Shape Types: RESOLVED and shipped v0.1-polish 2026-05-19.
     Design decision: structural typing for anonymous shapes, nominal for named shapes.
     Implementation: canonical-name hoisting (no new Type variant).
     See docs/internal/implementation/IMP-inline-shape-types.md for the full design. -->

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
