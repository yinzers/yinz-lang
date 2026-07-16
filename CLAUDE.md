# Yinz — Claude Project Rules

Yinz is a compiled systems programming language. Goal: Rust-level performance, TypeScript-level readability. Memory-safe, zero-cost abstractions, approachable by junior developers.

File extension: `.ynz`. Compiler target: LLVM native machine code.

---

## Golden Rules (check every design decision against these — lower number wins)

1. **Dot-first design.** If something can be `.method()` with autocomplete, do it that way. No cryptic symbols, no keywords to memorize.
2. **Self-documenting syntax.** Every keyword, function, and pattern should be readable by a jr dev who's never seen Yinz. If docs are required to understand a line, the design failed.
3. **No garbage collector.** Ownership-based memory management only. One memory model. The compiler handles allocation and freeing.
4. **Compiler does the hard work.** Smart defaults everywhere. Type inference inside function bodies. Developer writes simple code; compiler makes it fast.
5. **Compile-time safety.** Wrong = caught at compile time, never at runtime.
6. **Familiar syntax.** Borrow from TypeScript/JavaScript wherever possible.
7. **Step-by-step over chaining.** No method chaining. Each operation gets its own line with a named variable. Compiler fuses into a single optimized pass.
8. **Zero-cost abstractions.** High-level syntax compiles to the same machine code as hand-written low-level code.
9. **Fast to type.** Quick to write without sacrificing readability. `function` over `fn` is worth it; don't add ceremony where it adds nothing.
10. **Efficiency first, dynamic after.** Default = most performant. `fixed<T>` by default, opt into `array<T>` when you need growth.
11. **The compiler is a teacher.** Errors explain what went wrong AND why. Suggestions explain why one approach beats another. Every diagnostic answers WHAT happened, WHAT to do instead, and WHY. The WHY must be **specific and contextual** — not generic ("avoids allocation") but tied to the actual call site ("scores isn't used again after this, so sortInPlace() skips the allocation"). The compiler is a senior developer mentoring a junior developer — see [`docs/reference/REF-teaching-mission.md`](docs/reference/REF-teaching-mission.md) for the full mission.
12. **Human-readable over programmer jargon.** `options` not `enum`. `follows` not `implements`. `nothing` not `void`. If a non-programmer can guess the meaning, the naming is right. (Note: union types use `|` for consistency with TypeScript — `or` was triple-overloaded.)
13. **Capital letter = type. Everything else = lowercase.** Scan any line of code — capital letter means type, no capital means everything else (function, variable, keyword, module). `Player` is a type. `player` is a variable. `request` is a module. `Request` would be a type. Zero ambiguity.

---

## Project Layout

| Path | Purpose |
|------|---------|
| `/docs/` | ALL project documentation — see [`docs/README.md`](docs/README.md) for the index. `docs/reference/` = language spec (formerly `/spec/`) + cross-cutting cited-as-authority principles; `docs/internal/implementation/` = compiler design rationale (formerly `/design/`); `docs/internal/decisions/` = one-time locked architecture calls (ADRs); `docs/internal/scratchpad/` = unbuilt/future design ideas (formerly `/design/future/`, `/design/stdlib/`, `design/open-questions.md`). Migrated 2026-07-01 onto the global `IMP-documentation-system.md` taxonomy. |
| [`docs/internal/implementation/IMP-feature-registry.md`](docs/internal/implementation/IMP-feature-registry.md) | SSOT registry schema + carve-out policy (see [`registry/features.toml`](registry/features.toml)) |
| [`docs/internal/implementation/IMP-lsp.md`](docs/internal/implementation/IMP-lsp.md) | LSP server architecture — salsa wiring, JSON-RPC dispatch, capability negotiation, framework choice rationale, self-hosting migration plan |
| [`registry/features.toml`](registry/features.toml) | Single source of truth for all feature inventories (keywords, jargon, intrinsics, deferred features, hint domains) |
| `crates/ynz-registry/` | Crate that parses [`registry/features.toml`](registry/features.toml) + generates typed Rust constants via `build.rs` |
| `crates/ynz-lsp/` | LSP server — wraps existing salsa queries in JSON-RPC, consumes `ynz-registry` for autocomplete/hover/diagnostics |
| `crates/ynz-tmgrammar/` | TextMate grammar generator — reads `ynz-registry`, emits `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json` |
| `crates/ynz-fmt/` | Formatter library — zero-config canonical Yinz formatting, consumed by `ynz fmt` subcommand and v0.2-M5 LSP format-on-save |
| `crates/ynz-watch/` | Watch daemon — long-running terminal command for rebuild-on-save + re-run; consumes salsa-backed compiler queries shared with the rest of the workspace |
| `tooling/vscode-ynz/` | VSCode extension — spawns `ynz-lsp`, ships syntax highlighting and language association |
| `.claude/rules/` | Detailed rule files (loaded on demand) |
| `CLAUDE.md` | This file — rules for Claude |

---

## Rules Files

| File | Load when |
|------|-----------|
| [`.claude/rules/non-oop.md`](.claude/rules/non-oop.md) | **LOAD FIRST** for any feature touching shapes/methods/dispatch/inheritance/contracts. Yinz is NOT object-oriented — data shapes + standalone functions + UFCS dot-call sugar. Drift back into OOP patterns is the most common modeling mistake. |
| [`.claude/rules/dot-postfix.md`](.claude/rules/dot-postfix.md) | Designing any syntax using dot-postfix (`value.x` vs `value.x()`). Parens for actions, no parens for access. |
| [`.claude/rules/vocabulary.md`](.claude/rules/vocabulary.md) | Any docs work — authoritative reference for Yinz user-facing terms (shape, value, map, options, etc.), the capital-letter-=-type rule, module/type case distinctions, and the renamed-concepts table |
| [`.claude/rules/inference.md`](.claude/rules/inference.md) | Designing IDE behavior, ownership UI, type-inference UI, any teaching surface where the compiler figures things out automatically |
| [`.claude/rules/auto-promotion.md`](.claude/rules/auto-promotion.md) | Designing any new feature, stdlib type, or compiler optimization — mandates auto-promotion analysis (silent codegen + muted hint + Tier 3 lint) when a stricter/faster form fits. The "fast by design even for inexperienced developers" pattern. |
| [`.claude/rules/stdlib-design.md`](.claude/rules/stdlib-design.md) | Designing or reviewing any stdlib module — six rules: pure-named methods are pure, no parallel APIs, no platform-default config, bounded queues, receiver-first args, codegen serialization. |
| [`.claude/rules/feature-registry.md`](.claude/rules/feature-registry.md) | Adding any new keyword, jargon entry, primitive method, type constant, deferred feature, diagnostic template, or muted-hint domain — all go in [`registry/features.toml`](registry/features.toml) first |
| [`.claude/rules/plan-invariants.md`](.claude/rules/plan-invariants.md) | Writing or reviewing milestone plans (M4 onward must include the 7-subsection Invariants block; v0.2-M2+ plans also require `### Feature Registry Entries`) |
| [`.claude/rules/spec-writing.md`](.claude/rules/spec-writing.md) | Writing or editing `docs/reference/REF-*.md` language-spec files |
| [`.claude/rules/language-design.md`](.claude/rules/language-design.md) | Making or reviewing language design decisions |
| [`.claude/rules/docs-checklist.md`](.claude/rules/docs-checklist.md) | Adding new `docs/internal/implementation/IMP-*.md` design docs, `docs/internal/scratchpad/SCRATCH-*.md` future-list ideas, or `docs/reference/REF-*.md` spec sections |
| [`.claude/rules/examples-structure.md`](.claude/rules/examples-structure.md) | Adding, renaming, or restructuring anything under `examples/` — flat layout, Pittsburgh-themed folder names, no nested workspaces |
| [`.claude/rules/authoritative-derivation.md`](.claude/rules/authoritative-derivation.md) | Designing or reviewing any compiler pass/guard/codegen path that consumes a derived analysis result (crossing/suspend sets, ABI/aliasing predicates, admission gates) — or anywhere two+ code paths must agree on the same computed answer. Thread the one authoritative source; never re-derive an "equivalent" twin. Design-time guard for the twin-computation-drift class that shipped silent miscompiles across M3a/M3d/M3e/M3g. |

---

## When Working on This Project

- **Design docs (`docs/internal/implementation/IMP-*.md`) are the GOVERNING source of truth — read them before planning AND keep them open while executing.** `docs/internal/implementation/` (especially `docs/internal/scratchpad/SCRATCH-future-*.md` for end-state vision like [`docs/internal/implementation/IMP-no-function-coloring.md`](docs/internal/implementation/IMP-no-function-coloring.md)) defines what the language IS; a plan is just a route to that destination, never an override of it. Mandatory:
  - **Before planning ANY feature**: read the relevant `docs/internal/implementation/IMP-*.md` doc(s) for that feature. This is part of the `/plan` research step — searching the codebase for *how it works today* is NOT a substitute for reading the design for *what it's supposed to be*.
  - **While executing**: refer back to the design docs whenever a question or ambiguity pops up. The answer is usually already written down.
  - **On any contradiction or gap between the plan and a design doc: STOP and surface it explicitly** in the form **"design doc `X` says A; the plan says B"** — do NOT silently follow the plan. The design doc wins unless Patrick explicitly decides to change the design (in which case update the doc). A milestone (v0.3-M2) was HALTED because a plan shipped a `block_on` bridge that directly contradicted [`docs/internal/implementation/IMP-no-function-coloring.md`](docs/internal/implementation/IMP-no-function-coloring.md)'s documented no-coloring/whole-program-may-block-analysis model, and three rounds of review never caught it because they only checked the plan against itself. Never again — diff the plan against the design.
- **Yinz is NOT object-oriented.** Data shapes hold fields + contract signatures; methods are standalone functions; `value.method()` is parser-level sugar for `method(value)` (UFCS — both call forms work). NO methods inside shape declarations; NO `override` keyword; `extends` is data-only inheritance. See [`.claude/rules/non-oop.md`](.claude/rules/non-oop.md) for the full model — this is the most common modeling mistake to drift back into. Locked r10–r13 (2026-05-16).
- **Every milestone plan MUST grow the canonical demo project + error gallery.** Per [`.claude/rules/plan-invariants.md`](.claude/rules/plan-invariants.md) `### Demo & Error Gallery` subsection: each phase that adds executable surface MUST extend `examples/pirates-roster/entrypoint.ynz` with the new feature in context AND extend `examples/primantis-orders/m{N}_errors.ynz` with intentional triggers for every new compile error class. This is how Patrick reviews the language UX after each phase — without it, features ship and never get hands-on validation. The `pirates-roster/` project covers EVERY v0.1 language feature (M1–M8) in one growing demo; stdlib modules (v0.5+) get their own per-module example projects under `examples/<themed-name>/` (use the SINGLE-ENTRY layout — mirror `examples/pirates-roster/`'s shape, one yinz.toml + one entrypoint.ynz + plain subfolders, Pittsburgh-themed folder name per [`.claude/rules/examples-structure.md`](.claude/rules/examples-structure.md)).
- **Project layout has two locked shapes (per [`examples/README.md`](examples/README.md))**: single-entry (`yinz.toml` with one `entry = "..."`, code in plain subfolders, used by `examples/pirates-roster/` and all stdlib module examples — the ~95% case) and multi-entry (one `yinz.toml` with `[entries]` table, ships under `ships/`, shared code in plain folders — v0.22 feature, previewed in `examples/stadium-fleet/`). When in doubt, single-entry. The `[entries]` multi-ship shape is opt-in for projects that genuinely have N co-shipped binaries.
- Check every proposed language feature against all golden rules before suggesting it
- Always use Yinz terms — see [`.claude/rules/vocabulary.md`](.claude/rules/vocabulary.md) for the full reference
- Language-spec files (`docs/reference/REF-*.md`) are written for a HS grad — short sections, example-heavy, plain English
- No method chaining in code examples; step-by-step with named variables
- Living design decisions go in the relevant `docs/internal/implementation/IMP-*.md` file (WHY captured); one-time locked calls get their own `docs/internal/decisions/ADR-*.md`
- Open questions go in [`docs/internal/scratchpad/SCRATCH-open-questions.md`](docs/internal/scratchpad/SCRATCH-open-questions.md)
- When a design decision is made, move it from open questions to its `IMP-*.md`/`ADR-*.md` home
- Every example in spec/design/plan/rule files MUST use real Yinz operations from the current scope — no invented APIs for illustration (see [`.claude/rules/dot-postfix.md`](.claude/rules/dot-postfix.md) "Examples-must-use-real-operations rule")

---

## Tech Stack

**Compiler target**: LLVM 18 (`inkwell` bindings, `llvm-18-dev` / `clang-18` / `libclang-18-dev`)
**Compiler implementation language**: Rust (stable)
**Package manager**: Cargo (workspace, [`Cargo.toml`](Cargo.toml))
**Node.js**: v22 (for `tooling/vscode-ynz/` VSCode extension build — `npm install && npx vsce package`)

**Dev container**: `docker-compose.yml` defines a `dev` service (image `ynz-dev`, built from `Dockerfile`).
All compiler build / test / fixture work runs inside this container. The bind mount `.:/work` makes
`target/` host-readable (uid 1000 / patrick) so `trading-v4` can mount `target/release`.

**`target/release` is a live consumer mount, not a release-only artifact.** External projects
(`trading-v4`, `backfillMarketData`, etc.) mount `../ynz/target/release` read-only and run whatever
binary is currently sitting there — continuously, at dev time, not just after a tagged `/release`.
**Any fix to `ynz-driver` / `ynz-watch` / `ynz-lsp` that needs to reach one of those consumer
projects must be rebuilt with `--release`, in the SAME session as the fix** — a `cargo build`
(debug) proves the code compiles and runs, but it does NOT update the binary those mounts actually
read. Forgetting the `--release` rebuild reproduces the exact bug you just "fixed" the moment the
consumer project re-runs it. See CHANGELOG/git history around 2026-07-06 for the incident this
note was added from (a `ynz-watch` linker fix verified against `target/debug`, shipped, and still
reproduced in `backfillMarketData` because `target/release` was stale).

```bash
# Start the dev container (background)
docker compose up -d dev

# Or run one-shot commands (container exits after each)
docker compose run --rm dev cargo build --workspace
docker compose run --rm dev cargo build -p ynz-driver
docker compose run --rm dev cargo build -p ynz-driver --release
docker compose run --rm dev cargo build -p ynz-lsp --release
docker compose run --rm dev cargo test --workspace
docker compose run --rm dev cargo clippy --workspace -- -D warnings
docker compose run --rm dev cargo fmt --all

# Run the compiler directly inside a running dev container
docker compose exec dev ./target/debug/ynz run crates/ynz-driver/tests/fixtures/hello.ynz
# → hello, yinz

# VSCode extension build (runs as root-step apt + user-step npm)
docker compose run --rm dev bash -c "cd tooling/vscode-ynz && npm install && npm run build && npx vsce package --no-yarn"
```

**Cargo registry cache**: stored in the named Docker volume `cargo-registry` (compose name;
Docker prefixes the project name at runtime, so `docker volume ls` shows it as
`ynz_cargo-registry`). Mounted at `/home/ubuntu/.cargo/registry`. Crates survive container
rebuilds. `target/` stays on the host bind mount — do NOT put it in a named volume.

```bash
# CLI commands (spec — not yet implemented in the Yinz language itself)
ynz build entrypoint.ynz
ynz run entrypoint.ynz
```

---

## Release Workflow

Two project-local skills handle the ship cycle:

- `/pr` — opens a draft PR for the current feature branch (auto-detects milestone from Cargo.toml; scans `.claude/plans/active/` for plan reference)
- `/release` — cuts a tagged milestone release (bumps [`Cargo.toml`](Cargo.toml), generates CHANGELOG section from merged PRs since last tag, commits, tags, pushes with user approval)

**When to invoke each**:

| Situation | Skill |
|---|---|
| Phase work complete on feature branch | `/pr` |
| Milestone complete (all phases merged to main) | `/release` |
| Unsure which | Just invoke either — they auto-detect and route to the other if the signals point that way |

**Proactive reminders** — Claude should suggest `/release` when:
- All tasks in the active milestone plan are checked off, AND
- All milestone PRs are merged to main, AND
- [`Cargo.toml`](Cargo.toml) still shows the previous milestone version (i.e., the bump hasn't happened yet)

Don't wait to be asked. If you see "ready to ship" signals, surface them.

**VSCode extension release convention** — every milestone release that touches `tooling/vscode-ynz/` MUST attach TWO `.vsix` assets to the GitHub release:
1. `yinz-{version}.vsix` — versioned artifact
2. `yinz-latest.vsix` — always overwritten (`--clobber`) so the stable URL never changes:
   `https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix`

This stable URL is how external projects pin to the Yinz extension without updating their install script every release. Never skip the `yinz-latest.vsix` upload.

---

## Claude Code Conventions

This project uses `.claude/` for AI-assisted workflow state — plain markdown files, no special tooling required.

- **`.claude/planning/{active,paused,done}/<plan-id>/`** — the SOLE home for project state, decisions, and active workstreams (superseded `.claude/state.md` and `.claude/todos.md`, both removed 2026-07-04 — that radar/backlog job now lives entirely in the plan-format schema below; there is no separate state or todo file to read at session start). Plan storage in the **global plan-format schema** (`~/.claude/docs/reference/REF-plan-format.md`; this repo migrated onto it 2026-07-01, replacing the short-lived pre-migration `.claude/plans/` ledger format). `<plan-id>` is `<created-date>-<slug>`; each directory holds `roadmap.md` (for a roadmap) or `plan.md` (for a milestone/standalone plan — current truth, not chat history), plus `audit.md` when history exists (append-only session/FRAGO sidecar). Roadmaps and plans are siblings in the same buckets, linked only by the `roadmap-id` frontmatter field (never by path) — e.g. a milestone plan can sit in `done/` while its still-active roadmap sits in `active/`. The bucket reflects the plan's frontmatter `status`, not its history; move the whole `<plan-id>` directory when status changes.
- **[`.claude/planning/_index.md`](.claude/planning/_index.md)** — auto-generated grouped view of every roadmap/plan on disk (roadmaps → nested milestones, standalone plans separate, broken `roadmap-id` links surfaced under `⚠ Unknown roadmap`). Regenerated by the global `plan-lifecycle.py index` hook on every planning write — **never hand-edit it**; it's overwritten. Regenerate manually after an out-of-band edit with: `echo '{"tool_input": {"file_path": "<abs-path-to-any-plan.md-or-roadmap.md>"}}' | python3 ~/.claude/tools/plan-lifecycle.py index`.
- **[`.claude/graveyard.md`](.claude/graveyard.md)** — known failure patterns specific to this project.

If you see an active plan file, read it before continuing the work it describes.

**Migration note (2026-07-01)**: the old `.claude/plans/{active,paused,done}/<initiative>/{roadmap.md,capability-ledger.md,<milestone>/plan.md,scratch/*.md}` layout was mechanically migrated to `.claude/planning/`. `capability-ledger.md` files were merged into their roadmap's `plan.md` body as a `## Capability Ledger` section (the new schema has no separate ledger file — the ledger lives inside the roadmap plan, per its by-id-linking model). `scratch/*-deviations.md` files were concatenated verbatim into each plan's `audit.md` under "Migrated scratch/ deviation notes" — **not** reformatted into individual FRAGO delta-records; that reformatting is real remaining work if a plan needs its history in the new format. Historical `session-id` chains were not tracked pre-migration, so every migrated plan starts with `session-id: []`.
