---
slug: v0-2-dev-loop-tooling
type: roadmap
owner: patrick
status: active
created: 2026-05-18
last_updated: 2026-05-20 (v0.2-M5 scope updated to reflect final execution plan)
milestones:
  - v0-2-m1-feature-inventory-sync
  - v0-2-m2-lsp-thin-slice
  - v0-2-m3-fmt
  - v0-2-m4-watch
  - v0-2-m5-lsp-full-and-release
---

# Roadmap: v0.2 — Dev-Loop Tooling + Drift-Free Architecture

## Round-1 Approval Resolutions (2026-05-18)

The following choices were locked in during initial roadmap review with Patrick. They override anything else in this doc if there's a conflict.

- **Milestone numbering: per-version prefix.** v0.2 uses `v0.2-M1` through `v0.2-M5`. v0.1 keeps its legacy `M1`–`M8` naming (no retroactive rename). Every future version restarts at `vX.Y-M1`. Rationale: by v1.0 we'd be at "M147" which is unreadable; version-prefixed milestone numbers stay self-explanatory forever.
- **LSP splits into TWO milestones** — Thin Slice (v0.2-M2) and Full (v0.2-M5). Thin Slice ships autocomplete + inline errors + basic hover + a VSCode plugin EARLY so Patrick can use his own LSP while v0.2-M3 (fmt) and v0.2-M4 (watch) cook in parallel. Full LSP adds go-to-def / rename / find-refs / muted hints / format-on-save and cuts the v0.2.0 tag. Rationale: Patrick explicitly said "want to use this asap, tough on human eyes."
- **VSCode plugin ships in v0.2-M2.** Not deferred to post-v0.2. The plugin is a thin `extension.json` + activate stub that spawns the `ynz-lsp` binary; published to the marketplace as a preview. Without this, the LSP is invisible to most users (and to Patrick during testing).
- **`ynz watch --json` mode probable inclusion in v0.2-M4.** If research phase finds it's >25% scope inflation, defer to a later version (v0.2.x or v0.3) AND add a registry entry for it so it can't be forgotten. Also serves as the canonical test that the SSOT registry tracks deferred TOOLING features, not just deferred LANGUAGE features.
- **Format-on-save**: fmt library + CLI ship in v0.2-M3; LSP's `textDocument/formatting` handler delegates to the library in v0.2-M5. Standard pattern, no surprises.
- **M7/M8 housekeeping**: stale `m7-*.md` and `m8-*.md` plans (both `status: done` in front-matter, sitting in `active/`) get `git mv`'d to `done/` as a side cleanup. They've reportedly been migrating back to `active/` on their own — flagged as a separate-from-v0.2 investigation (likely a hook or git workflow issue; we'll find the cause if it recurs).

## Vision

By the end of v0.2, a Yinz developer opens their editor, sees autocomplete that knows every keyword, type, primitive method, and reserved-but-not-yet-shipped feature; gets hover docs that explain WHAT/WHAT-INSTEAD/WHY; sees inline errors with the same teaching format the CLI produces; gets format-on-save with zero config; and (for terminal users) can run `ynz watch` for sub-second recompile feedback. Behind the scenes, every feature surface — keywords, intrinsics, banned jargon, reserved features, muted-hint domains, lint rules (when they ship), stdlib APIs (when they ship) — flows from ONE central registry that every consumer reads from. Adding a new language feature in v0.45 or renaming an internal method in v0.31 updates the registry in ONE place; the IDE, error messages, lint suggestions, and docs all pick it up automatically.

The non-vision is just as important: by the end of v0.2, we should NEVER ship a feature where the compiler knows about it but the IDE doesn't — because the IDE READS from the same source the compiler does. The drift class goes away by construction, not by discipline.

## Why Now

v0.1 shipped (tag `v0.1.0`, 830 tests, M1–M8 complete). The compiler is structured around `salsa` queries from day 1 specifically to make the LSP a "wrap existing queries over JSON-RPC" job instead of a 6-month rewrite (see `design/compiler-language.md` "Why Salsa"). The dev-loop tooling slot in `design/mvp-scope.md` is v0.2.

Two pressures combine to make this the right moment for the SSOT foundation:

1. **Drift is already starting.** As of v0.1, primitive intrinsics live in one table (`crates/ynz-typeck/src/intrinsics.rs`); banned jargon in another (`crates/ynz-diagnostics/src/banned_jargon.rs`); reserved-but-deferred features (`gpu`, `test`, `foreign`, sized ints, sized floats, arbitrary-precision decimal) are scattered across `design/mvp-scope.md` prose + ad-hoc lexer/typeck error handlers. Adding `int.max` in M5 touched five places that nothing forces to stay in sync. Stdlib starts in v0.5 — by v0.9 there will be hundreds of API surfaces, with the LSP needing every one. The registry needs to exist BEFORE the LSP exists, or the LSP gets built with hardcoded lists that drift.

2. **The user explicitly called this out.** Quoting Patrick: "In version 45 that doesn't exist for a long time we add a new feature. It would be nice if there was a way to NEVER forget to add the feature to the IDE so it's still super easy to use for users plus errors and muted text and comments are all good etc. In addition thinking about updating EXISTING things to have more, or less, or modified stuff. Like say we change an internal function call to be like .toDouble instead of toFloat (not actually doing it this is just an example) how can we ensure we don't mess up the IDE stuff and forget to update it." Build the SSOT before the IDE, not after.

## Constraints

- **Rust stable + LLVM 18** — same toolchain as v0.1 (see `crates/*` + `.cargo/config.toml`). No new global dependencies without explicit decision.
- **Salsa-first** — every milestone in this roadmap must use existing salsa queries as the compute layer; no parallel pipelines, ever.
- **The LSP and CLI share ONE codebase.** Per `design/compiler-language.md` — non-negotiable. Anything LSP-only is a JSON-RPC wrapper around existing queries.
- **`ynz fmt` is opinionated, zero-config.** Per `design/mvp-scope.md` v0.2 entry — Yinz has one style. No `.ynzfmt.toml`. Disagreement is between you and the formatter, not the formatter and itself.
- **No new language features in v0.2** — this milestone is tooling. Auto-promotion analyses, lint rules, and stdlib stay deferred to their assigned versions (v0.3, v0.4, v0.5+).
- **All compile errors continue to follow WHAT/WHAT-INSTEAD/WHY** — per `design/teaching-mission.md` and `design/compiler-errors.md`. The LSP renders the same diagnostics the CLI does.
- **No GC, no per-instance method storage, none of v0.1's locked properties weaken** — tooling only. The compiler binary's behavior on a `.ynz` file is identical before and after v0.2.
- **All milestone plans from M9 onward continue to follow `.claude/rules/plan-invariants.md`** — 6-subsection Invariants block required (Safety, Performance, Teaching, Runtime Dependencies, Kernel-Mode Behavior, Demo & Error Gallery). This is a load-bearing project rule that doesn't pause for tooling milestones.

## Architectural Decisions Made

These are LOCKED before any v0.2 execution plan starts. Each milestone's execution plan must conform.

- **Single Source of Truth (SSOT) registry pattern**: Every "feature surface" the IDE/compiler needs to know about lives in ONE declarative source (proc-macro, data-file, or table — exact shape decided in M9 execution plan). Consumers (lexer, parser, typeck, diagnostics, LSP, lint when v0.4 ships, docs) READ from the registry. No consumer hardcodes a list that the registry already contains. — Rejected alternatives: keep scattered registries with discipline ("we'll remember"). Rejected because Patrick explicitly said the discipline-only model is the failure mode he wants to engineer away. Also rejected: per-consumer registries with cross-checking tests. Rejected because that's the same drift class with an extra step.

- **Formatter ships as a library, with CLI + LSP front-ends both wired into it**: `ynz-fmt` crate exposes a `format(ast) -> string` API. The CLI command `ynz fmt` calls it. The LSP's `textDocument/formatting` and `textDocument/rangeFormatting` requests call it. Editors get format-on-save for free via standard LSP. — Rejected: separate CLI-only formatter, IDE format-on-save deferred. Rejected because format-on-save is the highest-value editor UX and shipping it later means users hand-type formatting fixes for months.

- **`ynz watch` and the LSP share salsa queries for incremental rebuild**: Both leverage the existing salsa memoization. Watch is the terminal-user version; LSP is the editor-user version. No new incremental-build infrastructure. — Rejected: watch as a thin wrapper that re-invokes `ynz build` cold each time. Rejected because the sub-second target from `design/compiler.md` is only achievable via incremental compute.

- **Reserved-but-not-yet-shipped features get teaching errors driven by the SSOT registry, NOT scattered lexer/typeck handlers**: When a user writes `gpu`, `test`, `foreign`, `number<5000>`, `int<32>`, `f32`, etc. — features the language has LOCKED designs for but hasn't shipped — the compile error pulls its "ships in vX.Y, here's why deferred, here's the substitute you can use today" content from the registry. Adding a new deferred feature in v0.N's design phase = adding ONE registry entry; the lexer, typeck, error rendering, and (post-M9) the LSP autocomplete all pick it up. — Rejected: leave existing scattered handlers in place, only register NEW deferred features. Rejected because half-migrated registries are worse than fully-scattered ones — readers don't know where to look.

- **Renames/internal-call changes covered by snapshot tests against the SSOT registry**: When an internal method gets renamed (e.g., the hypothetical `.toFloat` → `.toDouble`), the registry entry changes in ONE place. Snapshot tests for typeck diagnostics, LSP autocomplete responses, and hover docs all read from the registry and re-snapshot. The drift class is caught at PR-review time by a single snapshot diff. — Rejected: rely on grep across the codebase before rename. Rejected because it's the discipline-only model that Patrick called out.

- **The SSOT registry is enforced as a project RULE, not a convention**: Adding a new language feature, stdlib API, reserved-but-deferred feature, primitive method, banned jargon term, deferred tooling feature, or any other "feature surface" without a registry entry is a Bouncer-level violation (graveyard entry, CI grep, plan-invariant check). Every milestone plan from v0.2-M2 onward (and every milestone plan in EVERY future version v0.3 through v2+) MUST list "SSOT registry entries added by this milestone" as a required acceptance-criterion item. The plan-invariants rule (`.claude/rules/plan-invariants.md`) gets a new subsection for this. The `/plan` skill template gets updated. CLAUDE.md gets updated. The graveyard gets an entry. The discipline-via-tooling model is non-negotiable — this is what closes the drift class permanently. — Rejected: rely on developer memory / PR review. Rejected for the third time because that IS the failure mode.

- **LSP framework choice deferred to v0.2-M2 execution plan** (open question below): `tower-lsp` (async, well-maintained, used by many production Rust LSPs) vs `lsp-server` (lower-level, sync, what rust-analyzer is built on). Both work with salsa. Decision made when v0.2-M2 starts — locks the choice for v0.2-M5 too (no re-decision).

## Open Architectural Questions

- **SSOT registry implementation shape** — proc-macro that generates per-consumer accessors? A `ron`/`toml` data file loaded at compile time? A pure-Rust declarative table (`const REGISTRY: &[FeatureEntry] = &[...]`)? Blocks: v0.2-M1. Decided by: Patrick + v0.2-M1 execution-plan research phase. Needed before: v0.2-M1 execution planning begins.

- **LSP framework: `tower-lsp` vs `lsp-server` vs roll-our-own** — both crates have tradeoffs (async vs sync, opinionated vs bare). rust-analyzer uses `lsp-server` directly; many newer LSPs use `tower-lsp`. Blocks: v0.2-M2 (thin slice). Decided by: Patrick + v0.2-M2 research phase (Context7 + WebSearch current state of both crates). Needed before: v0.2-M2 execution planning begins. Locks the choice for v0.2-M5 (no re-decision when scaling to full LSP).

- **Formatter algorithm: rustfmt-style (preserve-some-author-intent) vs prettier-style (full reflow)?** Yinz's "one style, no config" implies prettier-style (every output is canonical, doesn't preserve user line breaks). But there are edge cases (long expressions, comments). Blocks: v0.2-M3. Decided by: Patrick + v0.2-M3 research phase. Needed before: v0.2-M3 execution planning begins.

- **`ynz watch`: daemon or simple-loop?** A daemon (long-running process holding salsa state) gets the LSP's sub-second incremental performance; a simple loop (re-spawn on save) is dumber and slower. The daemon is closer to the LSP architecturally and might share code. Blocks: v0.2-M4. Decided by: v0.2-M4 research phase. Likely outcome: daemon, sharing infrastructure with the LSP — confirm in plan.

- **Does the SSOT registry track stdlib APIs that don't exist yet (v0.5+ placeholders), or only language-level features?** Tracking stdlib placeholders means v0.2-M1 has to predict the shape of every v0.5+ module entry — premature. NOT tracking them means v0.5+ has to retrofit the registry. Compromise: v0.2-M1 designs the registry SCHEMA to accommodate stdlib API entries, but only POPULATES it with v0.1 language-level entries + the deferred-tooling-feature entry-type (so v0.2-M4 can register `ynz watch --json` if punted). Each v0.5+ stdlib milestone POPULATES its own entries. Blocks: v0.2-M1 scope. Confirm in v0.2-M1 execution plan.

- **Does the SSOT registry track deferred TOOLING features in addition to deferred LANGUAGE features?** Locked: YES (per Round-1 Approval Resolutions above). v0.2-M1's registry schema defines a `deferred-tooling` entry-kind alongside `deferred-language`. Same WHY/SUBSTITUTE/TRIGGER fields. Use case: `ynz watch --json` if v0.2-M4 punts it; future build-mode flags; etc.

## Milestones

### Milestone v0.2-M1: Feature Inventory & Sync Architecture
**Value delivered**: Every existing scattered registry (banned jargon, primitive intrinsics, reserved deferred features, type-attached constants, diagnostic templates, muted-hint domain table from `design/inference.md`) lives in one SSOT registry. Consistency tests catch new entries that miss a consumer. The compiler's behavior is unchanged from v0.1 — this is a refactor + foundation, not a feature. Future v0.N+ work has ONE place to declare new features AND a project-wide rule that says "you MUST add it there." Drift class closes permanently because the next 30 milestones can't accidentally bypass the registry — the plan-invariant check, graveyard entry, and CLAUDE.md rule all enforce it.
**Execution plan**: `v0-2-m1-feature-inventory-sync` (status: shipped — tag v0.2.0-m1, 2026-05-20)
**Depends on**: nothing — first up in v0.2
**Rough scope (deliverables, not phase-by-phase — that's for the execution plan)**:

*Registry implementation*:
- Research phase locks the registry shape (proc-macro / data-file / pure-Rust table — see open question above)
- Build the `ynz-registry` crate (or whatever shape research picks) exposing the registry to all other crates
- Schema accommodates: language features (keywords, types, primitive methods, intrinsics, operators, ownership modifiers), reserved-but-deferred features (with WHY/SUBSTITUTE/TRIGGER fields per `design/mvp-scope.md` v2+ entries), banned jargon (with replacement + reason), diagnostic templates, muted-hint domains, type-attached constants, lint rules (placeholder for v0.4), stdlib API entries (placeholder for v0.5+), AND **deferred tooling features** (CLI flags / LSP capabilities / build modes deferred to a later version — e.g., `ynz watch --json` if v0.2-M4 punts it). The "deferred tooling features" entry-type is the explicit answer to "what if it's not a language feature but we still don't want to forget it" — same WHY/SUBSTITUTE/TRIGGER fields, indexed under a separate kind in the registry.

*Migration of existing scattered registries (one phase each)*:
- `crates/ynz-diagnostics/src/banned_jargon.rs` → registry entries with consumer-reading API
- `crates/ynz-typeck/src/intrinsics.rs` (primitive method dispatch) → registry entries
- Type-attached constants (`int.max`, `int.min`, `number.epsilon`, etc.) → registry entries
- Reserved-but-deferred features (every entry in `design/mvp-scope.md` v2+ section AND every entry in `design/future/*.md`: arena, auto-soa, concurrency, http-framework, no-runtime-mode, packages, panic-safety, self-references, supervisor) → registry entries with locked-design pointers
- Keyword/token table (`crates/ynz-lexer/src/...` and `crates/ynz-parser/src/...`) → registry-driven where it makes sense (parser stays as-is, but the LIST of valid keywords becomes registry-driven so error suggestions can use it)
- Diagnostic message templates (`crates/ynz-diagnostics/src/...`) → registry entries with WHAT/WHAT-INSTEAD/WHY fields

*Consistency enforcement*:
- Consistency tests in `ynz-registry` crate that fail CI when: a banned-jargon entry has no replacement; a reserved feature has no WHY/SUBSTITUTE/TRIGGER fields filled in; a primitive method exists in the registry but typeck doesn't dispatch it; etc. Each test name maps to the drift class it prevents.
- Bouncer graveyard entry: "scattered registry without SSOT link" — grep-detectable pattern that catches PRs adding a hardcoded list-of-features in a Rust file that should have gone through the registry. Pattern enforcement via Bouncer postaudit per CLAUDE.md "Active Enforcement" section.

*Project rule + skill + doc updates (the "never forget" enforcement layer)*:
- **NEW** `design/feature-registry.md` — full design doc: schema, consumer API, when to add an entry, when an entry is the wrong fit, the "Explicit Carve-Outs" section per the risk table mitigation
- **NEW** `.claude/rules/feature-registry.md` — project rule loaded on demand for any milestone touching the registry or adding a language feature; references the design doc; lists the required-entry-types checklist for adding a new feature
- **UPDATE** `.claude/rules/plan-invariants.md` — add a new mandatory subsection `### Feature Registry Entries` to the Invariants block for every milestone plan from M10 onward. Required content: enumerate every registry entry the milestone adds, and for renames/changes, enumerate every registry entry the milestone modifies. The 6-subsection invariants block becomes 7-subsection.
- **UPDATE** `~/.claude/skills/plan/SKILL.md` (the global `/plan` skill) — Step 5c (Post-Plan Checklist) gets a new "Yinz-specific: SSOT Registry" subsection that asks "does this plan add or change any language-feature surface? if yes, are the registry entries listed in acceptance criteria?" The global skill needs this update because every Yinz milestone plan goes through `/plan`.
- **UPDATE** `CLAUDE.md` (project file) — "Golden Rules" gets a new entry (or the Project Layout section gets a new pointer): "Adding a language feature, stdlib API, reserved deferred feature, or any compiler-surface item REQUIRES a registry entry. See `design/feature-registry.md` and `.claude/rules/feature-registry.md`."
- **UPDATE** `.claude/graveyard.md` — new entry capturing the drift anti-pattern (e.g., "Adding a primitive method to typeck without registry entry") with the Bouncer grep pattern.
- **UPDATE** `design/mvp-scope.md` — each "Locked design, deferred from v0.1" entry gets a back-pointer to its registry entry once M9 ships ("Registry entry: `deferred.gpu`", etc.). Bidirectional linking per the carve-outs rule.
- **UPDATE** `design/compiler-language.md` — section on "How the LSP shares the compiler" gets a paragraph on "Both also share the SSOT registry."

*Demo & Error Gallery extension (per `.claude/rules/plan-invariants.md` Demo & Error Gallery subsection)*:
- New deferred-feature trigger entries in `examples/primantis-orders/m9_errors.ynz` if the registry rewrites any deferred-feature error rendering (likely yes — `gpu`, `test`, `foreign`, `int<32>`, `f32`, `number<5000>` all get registry-driven errors now).

*Acceptance criterion for M9 completion*:
- Every `crates/**` file is auditable: "what registry entries does this consume?" has a single grep-able answer. No consumer hardcodes a list the registry covers (subject to documented carve-outs).
- Every M10-onward execution plan (in this roadmap AND every future version's plans) MUST have a `### Feature Registry Entries` subsection in its Invariants block. Plan-invariants rule enforces.

Likely 8-12 phases total. v0.2-M1 is intentionally substantial — it's the foundation under EVERYTHING from v0.2 through v2+.

### Milestone v0.2-M2: LSP Thin Slice + VSCode Plugin
**Value delivered**: Patrick can install a VSCode extension and immediately get **autocomplete, inline errors, and basic hover** while editing `.ynz` files. Lets him eyes-on test the LSP architecture and provide real feedback while v0.2-M3 (fmt), v0.2-M4 (watch), and v0.2-M5 (LSP Full) are being built. Critical UX milestone — without it, the rest of v0.2 is invisible until the very end.
**Execution plan**: `v0-2-m2-lsp-thin-slice` (status: planned)
**Depends on**: v0.2-M1 minimum-viable-shape (needs keyword + type + intrinsic + diagnostic registry entries done; can start before v0.2-M1 is fully complete — the migration of every scattered registry is M1's full scope, but M2 only needs the registry's MVP to start consuming it)
**Rough scope (deliverables)**:
- LSP framework research (decide `tower-lsp` vs `lsp-server` — see Open Question)
- `ynz-lsp` crate scaffolding + JSON-RPC plumbing (stdio transport for editor compat)
- `textDocument/didOpen` / `didChange` / `didClose` wired to existing salsa queries
- **Autocomplete**: registry-driven for keywords, types, primitive methods, reserved-but-deferred features (compile errors show "ships in vX, here's the substitute" for any registry entry with a `deferred` kind)
- **Inline diagnostics** (`textDocument/publishDiagnostics`): reuse the existing `ynz-diagnostics` rendering — same WHAT/WHAT-INSTEAD/WHY format the CLI uses
- **Basic hover** (`textDocument/hover`): registry entry → markdown rendering (description + signature + WHY)
- **VSCode extension** (`ynz-vscode` repo or subdir): minimal `package.json` + `extension.ts` that spawns the `ynz-lsp` binary, registers `.ynz` language, syntax highlighting via TextMate grammar (could derive from registry too — the keyword list becomes the grammar token list)
- **Publish VSCode extension as "preview"** to marketplace so Patrick can install with one click. Includes a README pointing back to the Yinz repo.
- Integration tests: spin up `ynz-lsp` against a fixture project, send LSP JSON-RPC requests, assert responses
- `examples/pirates-roster/entrypoint.ynz` annotated comments showing "open this in VSCode with the Yinz extension installed to see X"
- Explicitly OUT of scope for M2 (deferred to v0.2-M5): go-to-definition, find-references, rename, format-on-save, muted-hint surfaces, code lenses, the whole "deferred-to-v0.2 sweep" of M7/todos.md items

Likely 6-8 phases. Sequencing note: can begin once v0.2-M1's keyword/type/intrinsic registry MVP lands (probably after M1 phase 3-4, not the full M1 completion).

### Milestone v0.2-M3: `ynz fmt`
**Value delivered**: `ynz fmt path.ynz` formats one file. `ynz fmt --all` formats the whole project. `ynz fmt --check` exits non-zero if any file would change (CI gate). `ynz-fmt` library API (separate crate) is ready for v0.2-M5's LSP to wire into format-on-save.
**Execution plan**: `v0-2-m3-fmt` (status: planned)
**Depends on**: v0.2-M1 (formatter consults registry for keyword spellings, reserved-name protection). Can run in parallel with v0.2-M2.
**Rough scope**: research phase locks formatter algorithm (rustfmt vs prettier style — see Open Question), design the `ynz-fmt` crate API, implement the formatter over the existing AST, idempotency property test (`fmt(fmt(x)) == fmt(x)` over all spec/ examples + proptest fuzz), CLI wiring (`ynz fmt`, `--all`, `--check`, `--stdin`), golden-file tests for canonical formatting of every spec/ + examples/pirates-roster example. Likely 4-6 phases.

### Milestone v0.2-M4: `ynz watch`
**Value delivered**: `ynz watch` runs in the terminal, recompiles on file save, sub-second turnaround for typical single-file edits. Output uses the same diagnostic rendering as `ynz build`. For developers not in an LSP-enabled editor (terminal/CI watchers), this is the dev-loop story. **PROBABLE inclusion: `--json` mode** emitting structured events (e.g., `{ "type": "build-start", "file": "..." }`, `{ "type": "diagnostic", ... }`) for build-automation tooling — confirmed in M4's research phase. If `--json` is >25% scope inflation, defer to a later version AND register it as a `deferred tooling feature` in the SSOT registry (per v0.2-M1's schema) so it can't be forgotten.
**Execution plan**: `v0-2-m4-watch` (status: planned)
**Depends on**: v0.2-M1 (watch consults registry for what to re-check). Can run in parallel with v0.2-M2 + v0.2-M3.
**Rough scope**: research locks daemon vs simple-loop (likely daemon, sharing infrastructure with v0.2-M5 LSP), file-watching via `notify` crate, salsa-driven incremental recompile, output rendering reusing `ynz-diagnostics`, `--json` event stream (probable — confirm in research phase) and tests, integration tests (touch file, assert recompile fires; touch file rapidly, assert no race), explicit perf check against the sub-second target from `design/compiler.md`. Likely 4-5 phases (5 if `--json` lands in M4; 3-4 if deferred).

### Milestone v0.2-M5: LSP Full + v0.2.0 Release
**Value delivered**: The thin LSP shipped in v0.2-M2 expands to a full editor experience: **8 new LSP capabilities** (go-to-definition, find-references, atomic rename, format-on-save, `textDocument/inlayHint` with 5 firing domains, code actions with quick-fixes, semantic tokens, doc-comment hover), **3 compiler correctness bug-fixes** (hidden-field default eval, dynamic-dispatch call-site coercion, UFCS const-lend check parity), **`ynz build --json`** structured NDJSON output, and **VSCode extension v0.2.0** with 8 feature screenshots. Cuts the **v0.2.0 release tag** (first plain-version tag; no `-mN` suffix).
**Execution plan**: `.claude/plans/active/v0-2-m5-lsp-full-and-release.md` (status: active; 15 phases: P0-P12 with P11 split into P11a/b/c)
**Depends on**: v0.2-M1 (SSOT registry — all 9 `[[muted_hint_domain]]` entries needed for inlay hints), v0.2-M2 (thin LSP foundation — server.rs, state.rs, position.rs, dispatch model), v0.2-M3 (`ynz-fmt::format` for format-on-save), v0.2-M4 (must be merged + tagged `v0.2.0-m4` before Phase 12 runs the version bump).

**What this milestone ships (locked)**:
- `textDocument/definition`, `textDocument/references`, `textDocument/rename` + `textDocument/prepareRename`
- `textDocument/formatting` + `textDocument/rangeFormatting` (adds `format_range` to `ynz-fmt`)
- `textDocument/inlayHint` — 5 of 9 domains fire today; 4 protocol-only for v0.3+ data
- `textDocument/codeAction` — WHAT-INSTEAD-driven quick-fixes from registry
- `textDocument/semanticTokens` — full + range; delta-encoded; keyword/type/fn/var classification
- Structured `Diagnostic.code` + `Diagnostic.data`; doc-comment hover via `leading_docs` parser attachment
- `ynz build --json` NDJSON; schema stabilized at v0.2.0
- 3 correctness bug-fixes closing `todos.md` "Soon" entries

**What remains deferred (with registry entries)**:
- LSP pull-diagnostics model (`lsp-pull-diagnostics` registry entry)
- 4 protocol-only inlay-hint domains (`lsp-inlay-hint-wait-points`, `-allocators`, `-lifetimes`, `-function-param-type`)
- Aliased re-export rename (`lsp-rename-aliased-re-export` registry entry)

**M2 deferrals targeted for closure in M5** (from `.claude/plans/done/v0-2-m2-lsp-thin-slice.md` Deferrals table):
- `textDocument/inlayHint`, `textDocument/codeAction`, `textDocument/semanticTokens` — planned Phases 6-8
- Doc-comment hover — planned Phase 10
- `Diagnostic.code` + `Diagnostic.data` — planned Phase 9
- Pull-diagnostics — explicitly deferred to v0.3+ via `registry/features.toml` `lsp-pull-diagnostics` entry

## Out of Scope

These are explicitly NOT v0.2 — listed here so a future chat reading this 3 months from now doesn't try to add them.

- **Auto-parallelization** — v0.3 per `design/mvp-scope.md`. The `wait`/`background` keywords already parse and run sequentially (M8 P5). The parallelization optimization itself is v0.3.
- **Auto-SoA layout transform** — v0.3.
- **Lint tier (Tier 3 suggestions)** — v0.4. The infrastructure to emit Tier 3 suggestions is a v0.4 concern. v0.2's SSOT registry should DESIGN its schema to accommodate lint rules (so v0.4 doesn't have to retrofit), but doesn't populate any.
- **Package manager** — v0.22.
- **Any stdlib module** — v0.5+ (file/path/directory), v0.6 (math), v0.7 (cli/env/process), v0.8 (json), etc. The SSOT registry schema should accommodate stdlib API entries (confirmed in open question above), but v0.2 only populates language-level entries.
- **Editor-specific extensions/plugins (EXCEPT VSCode)** — Neovim config, Emacs lsp-mode setup, JetBrains plugin, etc. The LSP protocol is editor-agnostic; non-VSCode editors connect via standard LSP. Writing per-editor distribution packages for those is post-v0.2 community/marketing work. **VSCode IS in scope** for v0.2-M2 specifically because Patrick will use it for testing while v0.2 develops — see v0.2-M2 above.
- **`ynz doc` (static doc generation) and `ynz repl`** — v1.1 per `design/mvp-scope.md`. Doc comments collected by M8 P3 are read by the LSP for hover, but the standalone `ynz doc` command is v1.1.
- **Lint customization config** — v1.x.
- **Operator overloading + custom iterables** — v1.0 launch milestone.
- **FFI, GPU, ML, markets, sized ints/floats, arbitrary-precision decimal** — v2+. Per the SSOT registry decision above, these get REGISTRY ENTRIES in M9 (so their compile errors are registry-driven) but no implementation.
- **Self-hosting (Yinz compiler in Yinz)** — v2+.
- **Embedded SQL IDE support (syntax coloring + formatting)** — Deferred to the database stdlib milestone (v0.5+). Rationale: the common case in Yinz is shapes + stdlib functions for DB access; raw SQL is the escape hatch for edge cases. The embedded SQL syntax (tagged template, block syntax, etc.) belongs in the database milestone design, not here — no point designing a syntax before the feature exists. IDE coloring and formatter support ship alongside that milestone. Formatting requirement to capture for that design: the first SQL keyword (`INSERT INTO`, `SELECT`, `FROM`, `WHERE`, etc.) is indented at the surrounding `const`-indentation + project-standard indent width (2 or 4 spaces — decided at design time); subsequent SQL lines follow standard SQL indentation from that baseline. Patrick's reference example:
  ```
  const query = sql`
      INSERT INTO bars
      SELECT
          symbol,
          time_bucket(INTERVAL '5 minutes', timestamp) AS timestamp,
          ...
      FROM bars
      WHERE timeframe = 'oneMinute'
      GROUP BY symbol, timestamp;
  `
  ```

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| SSOT registry design ossifies wrong, v0.2-M2/M3/M4/M5 reshape it under load and the "single source" promise breaks | Medium | High | v0.2-M1 research phase tries the registry against 3-4 different consumers (lexer, typeck, diagnostics, LSP autocomplete sketch from v0.2-M2 perspective) BEFORE locking the schema. Build for the actual consumers, not the imagined ones. |
| LSP framework choice (tower-lsp vs lsp-server) turns out to be wrong mid-v0.2-M2, requires migration | Medium | Medium | v0.2-M2 research phase spike: build a minimal "hello LSP" against each framework, use rust-analyzer's experience as ground truth. Decide before scaffolding the real `ynz-lsp` crate. Locked choice carries through to v0.2-M5 with no re-decision. |
| Formatter idempotency bug ships — `fmt(fmt(x)) != fmt(x)` — breaks CI for users using `--check` | Low | High | v0.2-M3 has explicit `fmt(fmt(x)) == fmt(x)` property test running across all spec/ examples + all `examples/pirates-roster` content. Fuzz test (`proptest`) over arbitrary AST inputs as the deeper guarantee. |
| `ynz watch` daemon holds stale salsa state under filesystem race conditions (rapid save during recompile) | Medium | Medium | Daemon design includes explicit "invalidate then recompute" sequence; integration test simulates rapid saves; if salsa's invalidation API is insufficient, escalate to salsa upstream or fall back to simple-loop. |
| Drift between LSP autocomplete and compiler typeck despite SSOT — registry covers names but not call-site context | High | Medium | The SSOT registry covers DECLARATIONS (what exists). Context-sensitive checks (e.g., "is this call-site receiver type compatible with this method") still live in typeck. v0.2-M1 designs the registry to expose ENOUGH info that LSP doesn't have to re-derive declarations, but accepts that typeck-level checking remains shared via salsa queries (which is the original architecture). |
| v0.2's tooling exposes existing compiler bugs that v0.1's CLI users hadn't hit (LSP renders errors on every keystroke; bugs become visible) | High | Medium | This is an acceptable cost — finding bugs is a feature, not a bug. v0.2-M2 ships the thin slice early specifically so Patrick can SURFACE these bugs while v0.2-M3/M4/M5 are cooking. v0.2-M5 budgets time for "fix the LLVM/typeck bugs the LSP surfaces." Tracker: `.claude/todos.md` "Soon" section accumulates items as M2 and M5 progress. |
| Format-on-save in editors interferes with LSP's incremental analysis (formatter rewrites file → LSP re-parses → autocomplete flickers) | Medium | Low | Standard LSP solution: format-on-save returns a TextEdit; editor applies it; LSP gets the new document via standard `textDocument/didChange`. Salsa's incremental compute handles the re-parse cheaply. Edge case is rare and well-understood. |
| The "never forget" promise becomes a religion that creates friction for legitimate parallel-registry use cases (e.g., a perf-critical inner-loop lookup that genuinely shouldn't go through SSOT) | Low | Low | v0.2-M1 design doc includes an "Explicit Carve-Outs" section: cases where a separate hand-curated table is allowed, with the rule that the carve-out MUST link back to the SSOT entry and the SSOT entry MUST link to the carve-out (bidirectional). Catches drift via "if A changes, you must touch B" by making the relationship explicit. |
| VSCode extension publishing process blocks v0.2-M2 (marketplace verification delays, publisher account setup) | Low | Medium | v0.2-M2 plan includes "register VSCode publisher account" as an early phase. Fallback: ship the extension as a `.vsix` file with manual-install instructions if marketplace approval delays. Patrick can install via VSCode "Install from VSIX" while marketplace catches up. |
| Stale `m7-*.md` / `m8-*.md` plan files keep migrating back from `done/` to `active/` after Patrick git-mv's them — unknown root cause | Low | Low | Side-investigation, not a v0.2 risk. Mentioned in Round-1 Resolutions. If recurs after the housekeeping mv, audit SessionStart hooks and any plan-restoration logic. Likely culprit: an autosave/recovery script. |

## Open Questions for Patrick

Round-1 questions all resolved (see Round-1 Approval Resolutions at top). No outstanding strategic questions block v0.2-M1 from starting. Architecture-level open questions remain (see "Open Architectural Questions" section above) — those are decided during each milestone's research phase, not by Patrick upfront.

When v0.2-M1 execution planning begins, the FIRST thing to confirm is the registry implementation shape (proc-macro / data-file / pure-Rust table). That's the only Patrick-input decision blocking M1 start.

---

## Cross-References

- `design/mvp-scope.md` — v0.2 scope (LSP + watch + fmt bundle)
- `design/compiler-language.md` — Salsa-first architecture, "Why Salsa" section
- `design/compiler.md` — sub-second incremental rebuild target
- `design/ide-hints.md` — muted-hint protocol (consumed by M12 LSP)
- `design/inference.md` — three placement categories (Addition / Replacement / Informational)
- `design/teaching-mission.md` — WHAT/WHAT-INSTEAD/WHY diagnostic format
- `design/compiler-errors.md` — banned-jargon list (current consumer of `banned_jargon.rs`, becomes SSOT registry entry in M9)
- `design/linting.md` — Tier 3 suggestions (v0.4, but M9's registry schema must accommodate)
- `.claude/rules/auto-promotion.md` — auto-promotion analysis (M12 LSP renders muted hints for these)
- `.claude/rules/plan-invariants.md` — required 6-subsection Invariants block for every M9–M12 execution plan
- `.claude/plans/done/v0-1-compiler.md` — v0.1 roadmap (template for this one's structure + completion approach)
