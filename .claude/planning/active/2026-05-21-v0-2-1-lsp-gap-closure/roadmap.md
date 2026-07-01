---
name: "v0-2-1-lsp-gap-closure"
plan-id: "2026-05-21-v0-2-1-lsp-gap-closure"
status: "active"
roadmap-id: null
session-id: []
created_at: "2026-05-21"
updated_at: "2026-05-21"
metadata:
  type: "roadmap"
legacy:
  note: "Fields below are preserved verbatim from the pre-migration .claude/plans/ ledger-format frontmatter (2026-07-01 migration to .claude/planning/). session-id history was not tracked pre-migration."
  slug: v0-2-1-lsp-gap-closure
  type: roadmap
  owner: Patrick Rizzardi
  status: active
  created: 2026-05-21
  last_updated: 2026-05-30
  milestones:
    - v0-2-1-m1-rename-shadowing
    - v0-2-1-m2-signature-help
    - v0-2-1-m3-code-lens
    - v0-2-1-m4-lint-tier3
    - v0-2-1-m5-doc-highlight-selection-range
    - v0-2-1-m6-type-def-implementation
    - v0-2-1-m7-type-hierarchy-call-hierarchy
    - v0-2-1-m8-diagnostic-enrichment-semantic-tokens
    - v0-2-1-m9-extension-polish-release
    - v0-2-1-m10-teaching-surface-bugfix
    - v0-2-1-m11-teaching-content-polish
---


# Roadmap: v0.2.1 — LSP Gap Closure

## Vision

By the end of v0.2.1, every standard language-extension feature a developer expects from a modern IDE works on Yinz code. Renaming a symbol never produces silent-wrong-output. Typing a function call shows parameter names and the active argument. Each top-level definition advertises its reference count inline. The tier-3 lint protocol fires real squiggles for the auto-promotion cases that already have analysis — `array<T>` should-be-fixed and `let` should-be-const.

From the user's perspective: the IDE goes from "works, but with notable gaps" to "as complete as TypeScript or Rust's editor experience." No language changes, no syntax changes — pure additive LSP work that v0.2.0 left on the table.

**v0.2.1 covers eleven focused milestones**: M1–M4 hit the four headline gaps from the initial audit (rename shadowing, signature help, code lens, lint tier-3). M5–M9 hit the second-pass spec audit done 2026-05-21 — every LSP 3.17 capability rust-analyzer or typescript-language-server ships that `ynz-lsp` does not yet advertise, plus the visual teaching surface (`DiagnosticTag::DEPRECATED`, semantic-token modifiers) that turns the registry's deprecated-feature catalog into actual struck-through editor rendering. M10–M11 are the **teaching-surface deep audit pass** done 2026-05-21 — four parallel analysis agents (`teaching-content-audit.md`, `diagnostic-why-quality.md`, `lsp-ux-friction.md`, `bugs.md`) surfaced 14 critical/high bugs and 100+ teaching-quality gaps. M10 fixes the bugs; M11 closes the content gaps.

---

## Why Now

v0.2.0 shipped the LSP foundation (130+ tests, full go-to-def / find-refs / rename / format / inlay hints / 9 code action types). The post-release audit identified four "Tier 2/3" gaps that are user-visible every day:

1. **Rename shadowing** is a Tier A correctness bug — renaming `score` → `x` when `x` already exists at a use-site silently produces a broken program. This can't wait for v0.3.
2. **Signature help** is the most visible gap — every function call shows nothing in the parameter tooltip.
3. **Code lens** is table stakes — VSCode and IntelliJ both surface reference counts inline above defs.
4. **Lint rules** — `design/linting.md` is fully specced, the protocol exists, and the two auto-promotion analyses (`array_to_fixed_promotion`, `let_to_const_promotion`) already run for inlay hints. The tier-3 yellow squiggle just needs to be emitted.

v0.3 is the concurrency milestone (Tokio runtime, state-machine codegen, auto-parallelization, auto-SoA — four full milestones already scoped in `v0-3-concurrency-perf`). It's the longest version in the v0.1 / v0.2 / v0.3 ladder. Shipping v0.2.1 first means users get a polished IDE while v0.3 cooks. Without v0.2.1, the gaps stay open for the whole v0.3 development window.

These nine features are independent of all v0.3 work — no concurrency dependencies, no codegen changes, no syntax surface to coordinate. The second-pass audit (M5–M9) is especially load-bearing for non-OOP Yinz: contracts (`follows`) are the entire polymorphism story, but there is no way today to ask the editor "what shapes follow this contract?" Without `implementation` and `typeHierarchy` providers, contract-based design is friction-heavy. Adding them is the difference between contracts being a teaching-quality feature and an editor-quality feature.

---

## Constraints

- **No language changes.** Zero new keywords, zero syntax changes, zero breaking changes to existing v0.1/v0.2 source code. v0.2.1 is purely additive on the editor side.
- **No compiler ABI changes.** Generated binaries from v0.2.0 source produce identical output under v0.2.1. The compiler crates that change (`ynz-typeck`, `ynz-lsp`, `ynz-registry`, `ynz-diagnostics`) gain new API surface but break no existing API.
- **VSIX must still install on existing v0.2.0 setups.** No new VSCode version requirement, no new platform requirement.
- **Plan invariants stay in force.** Per `.claude/rules/plan-invariants.md`, every milestone execution plan from v0.2-M2 onward must include the seven required Invariants subsections including `### Feature Registry Entries`. v0.2.1 milestones are no exception.
- **Teaching surface must ship in the same milestone as the feature.** Per the rule that landed in v0.3 prep: registry entries, hover docs, error gallery entries, and `examples/pirates-roster/entrypoint.ynz` extensions all ship with the feature, not in a follow-up. No "we'll add the docs next sprint" framings.
- **No work that overlaps with v0.3.** Concurrency-adjacent features (suspension-point lints, `wait`/`background` muted hints) explicitly stay out — they ship with v0.3-M3.
- **Carry forward all v0.2 rules**: WHAT/WHAT-INSTEAD/WHY diagnostic format, registry-as-SSOT for IDE surfaces, parity test enforcement (`tests/registry_parity.rs`).

---

## Architectural Decisions Made

These are locked before any execution plan starts. Each milestone's plan must conform.

- **Single tagged release for the headline milestones.** v0.2.1 is one tag, cut after M1–M9 (+ M10 bugfixes) merge. Individual milestones merge as their own PR(s) but do not get their own version bumps (no v0.2.1, v0.2.2, ... cascade). M11 ships separately as a `v0.2.1.1` content patch (Q13). Rationale: keeps the user-facing release notes coherent and matches the v0.2.0 model (M1–M5 merged, one tag cut at the end).

- **AMENDED 2026-05-30 — v0.2.1 is a PARALLEL TRACK to v0.3, pushed FIRST.** The roadmap was written 2026-05-21 assuming Cargo sat at `0.2.0` and v0.2.1 would tag before any v0.3 work. Reality: `v0.3.0-m1` already shipped (Cargo `version = "0.3.0-m1"`, commit `d509770`) and `v0-3-m2-wait-and-state-machines` is in flight. Patrick's locked call (2026-05-30): v0.2.1 LSP work is faster, so it ships on its OWN `0.2.1` tag FIRST — branched off `main`, built and released independently of the v0.3 line — then merged/rebased INTO the v0.3.0 branch afterward. v0.2.1 is NOT folded into a v0.3.x tag; it is its own release that v0.3 later absorbs. The "single tag at the end" model above still holds WITHIN the v0.2.1 track.

- **AMENDED 2026-05-30 — version handling for the v0.2.1 track.** Because `main`'s Cargo already reads `0.3.0-m1`, the v0.2.1 work happens on a dedicated branch where `Cargo.toml` is set to `0.2.1` for the release commit. The original "stays at 0.2.0 until release" note is VOID — the baseline is now `0.3.0-m1` on main. The v0.2.1 release branch carries its own `0.2.1` version string; the post-release merge into v0.3 keeps v0.3's higher version (no downgrade — git merge favors the v0.3 Cargo version, the LSP feature code merges cleanly since it's additive). M9's release phase owns reconciling this.

- **No new compiler crates.** All work lands in existing crates: `ynz-typeck` (new salsa queries), `ynz-lsp` (new request handlers + handler modules), `ynz-registry` (new schema entries for lint rules), `ynz-diagnostics` (new `DiagnosticKind` variants if needed). Adding a `ynz-lints` crate would be the right move IF the lint rule count grows past ~10 — at 2 starter rules it's premature.

- **VSIX rebuilds per milestone for testing, single bump for release.** Each milestone may produce intermediate VSIX builds for Patrick to install and verify the feature. The published `yinz-0.2.1.vsix` and `yinz-latest.vsix` only land on the release commit.

- **`signature_help_at_offset` salsa query lives in `crates/ynz-typeck/src/queries.rs`** alongside `module_signatures_query` and `check_query`. Returns `Option<SignatureHelpInfo>` carrying the matched `FunctionSig`, active parameter index (0-based), and parameter count. Reuses the existing AST-offset walker in `ast_offset.rs` — no new AST traversal infrastructure.

- **`bindings_in_scope_at` API for rename shadowing detection lives in `crates/ynz-typeck/src/scope.rs`** as an extension to the existing scope walker. Returns the set of bindings visible at a byte offset. The LSP rename handler queries this for each use-site before generating the workspace edit; if the new name appears at any use-site (and isn't the symbol being renamed), the rename rejects with a new error code.

- **Code lens scope = "N references" only.** No "run test" lens (no test framework yet — v0.4), no "show implementations" lens (contracts via `follows` use existing find-references). Single lens type keeps the milestone tight and validates the protocol works before committing to multi-lens design.

- **Code lens uses existing `cross_file_reference_count_estimate` query (from M5).** Pre-existing salsa-cached <5ms p95 query. Each code lens computes reference count via this query — sub-frame on `examples/pirates-roster/` (3 files, ~600 lines).

- **Lint rule scope = infrastructure + 2 starter rules.** Locked rules:
  1. `prefer-fixed-when-immutable` — fires on `let arr: array<T> = [...]` that never grows. Analysis already exists (`array_to_fixed_promotion_hints` in `inlay_hint_passes.rs`).
  2. `mutable-when-const-suffices` — fires on `let x = ...` that's never reassigned/mutated/lent. Analysis already exists (`let_to_const_promotion_hints`).
  Both rules ship with full WHAT/WHAT-INSTEAD/WHY suggestion text, registry entries, and integration with the parity test.

- **No new lint rules beyond the 2 starter rules in v0.2.1.** Other rules from `design/linting.md` ("repeated-inline-shape", "unused-import-found", etc.) are explicitly deferred. The 2 starter rules ship the *infrastructure* — adding rule #3 in v0.3+ should be a single registry entry plus an analysis pass, not a milestone-scale effort.

- **Lint suggestions use existing `Severity::Suggestion` tier** in `ynz-diagnostics`. The `DiagnosticKind` enum gets a `LintSuggestion { rule_name: String }` variant so code actions and the parity test can identify lint-emitted suggestions. The LSP already serializes Suggestion severity as `DiagnosticSeverity::HINT` (4) — no client-side changes needed.

- **`[[lint_rule]]` schema added to `registry/features.toml`** with fields: `name` (kebab-case), `severity` (currently always "suggestion" — Tier 3), `target` (a free-form description of when the rule fires), `what_instead` (the suggested fix), `why` (the rationale), `since` (milestone). Generated into `LintRuleEntry` in the registry crate. The parity test (`tests/registry_parity.rs`) gains a check that every lint rule emitted by the compiler has a registry entry.

- **Signature help capability `triggerCharacters`: `(`, `,`, `<`** — opens on call start, re-evaluates on argument separator, opens on generic type args. `retriggerCharacters` left to client default.

- **Rename shadowing error code = -32007** (`RenameWouldShadowLocalBinding`). Extends the existing 6 stable rename error codes (-32001 through -32006). Documented in `rename.rs` alongside the others. WHAT/WHAT-INSTEAD/WHY: "Renaming would create a local binding named `{new}` at one or more use-sites where `{new}` already refers to a different value. Pick a different name."

- **Parity test enforcement extends to new surfaces.** `tests/registry_parity.rs` already covers `builtins.rs` ↔ registry. v0.2.1 adds: (a) signature help reachability — every function signature returned by the salsa query is reachable from the AST that produced it; (b) lint rule registration — every `DiagnosticKind::LintSuggestion { rule_name }` emitted in the compiler has a `[[lint_rule]]` registry entry; (c) semantic-token modifier coverage — every `[[muted_hint_domain]]` whose category implies a visual modifier (e.g., `ownership_call_site` with `MODIFICATION` for `lend`) is verified to emit the modifier; (d) deprecated-feature tag coverage — every `[[deferred_language_feature]]` and `[[banned_jargon]]` identifier seen by the LSP emits `DiagnosticTag::DEPRECATED`. No silent drift between registry catalogs and editor surfaces.

- **M5: `documentHighlight` reuses references query.** Same scope-walker as find-refs, hard-filtered to the active file. Highlight kind: `READ` for value reads, `WRITE` for assignments and `lend`-passed args, `TEXT` for everything else. UFCS sites highlight together — `player.heal(20)` and `heal(player, 20)` both register as references to the same `heal` symbol because the AST has already desugared one to the other.

- **M5: `selectionRange` walks the AST node chain bottom-up.** Order: identifier → smallest containing expression → containing statement → containing block → containing function/shape/options declaration → file. Uses the existing `ast_offset.rs` walker — no new infrastructure.

- **M6: `typeDefinition` returns the underlying shape/options declaration site, not the type-position-token site.** Example: `let p: Player = ...; p<here>` returns the location of `shape Player { ... }`, not the location of the type annotation `Player` in the `let` binding. Same rule for `let r: Result<int> = ...` (where `Result` is a union alias) — jump to the `shape Result = Ok | Err` declaration.

- **M6: `implementation` provider is the contract → followers index.** When the cursor is on a contract name (a `shape` whose body has bare-signature declarations, no fields), the provider returns the location of every `shape X follows ContractName { ... }` in the project. When the cursor is on a shape that itself declares `follows`, the provider returns the contract's declaration site (the reverse direction). Built from a `contract_followers_index` salsa query populated during the existing signature-pass — no second walk of the project.

- **M6: For non-contract shapes (those with fields), `implementation` returns nothing rather than confusing fallback.** Specifically: `implementation` on `shape Player` (a data shape, not a contract) returns an empty list. Rationale: the LSP spec defines this provider as "implementations of an interface/protocol." For Yinz, that maps to contracts only. The user gets `typeDefinition` for the data-shape case. Empty-list response keeps the editor from showing stale/wrong results.

- **M7: `typeHierarchy` unifies `extends` and `follows` into one hierarchy.** SUPERTYPES = parent shapes (via `extends`) + contracts (via `follows`). SUBTYPES = child shapes that extend OR follow this shape. The LSP spec doesn't distinguish edge types in the hierarchy tree, so we annotate each node's `detail` field with the relationship (`extends` vs `follows`) for the user. Rationale: keeping them in one view matches user mental model ("what types relate to this type?") rather than splitting hairs about the relationship kind.

- **M7: `callHierarchy` incoming = find-refs filtered to call sites; outgoing = AST walk of function body.** Both reuse existing salsa queries. The `prepareCallHierarchy` request returns a `CallHierarchyItem` for the function/method at the cursor; subsequent `incomingCalls`/`outgoingCalls` requests return tree levels lazily. For UFCS calls, the incoming edge is keyed on the function name regardless of which call form (`player.heal(20)` vs `heal(player, 20)`) was used — the AST desugaring already normalizes this.

- **M8: `DiagnosticTag::DEPRECATED` emitted for banned-jargon + deferred-feature identifiers; `DiagnosticTag::UNNECESSARY` reserved for v0.2.1 lint emission (unused imports/bindings — when that lint ships).** The deprecated tag draws on the existing registry `[[banned_jargon]]` and `[[deferred_language_feature]]` entries — no new catalog. M8 wires the tag flag through `ynz_diagnostics::Diagnostic` (new field) and `diagnostic_transform.rs` (one mapping line). The `UNNECESSARY` tag has no current emitter — added later when an unused-import lint ships, which is a future v0.2.x or v0.3 milestone.

- **M8: Cross-file `related_information` requires multi-file text resolver in LSP state.** Today `diagnostic_transform.rs` silently filters cross-file related-info because the converter only has one file's text/LineTable. M8 adds `ServerState::text_and_line_table_for(path) -> Option<(&str, LineTable)>` — checks open documents first, falls back to filesystem read with a small per-request cache. Diagnostics like "name conflicts with X declared in services/crew.ynz" then surface the cross-file pointer in the editor.

- **M8: Semantic-token legend extends from 10 token types → 13, and from 0 modifiers → 6. The legend order is permanently locked.** New token types (indices 10–12, in order): `STRUCT` (shapes with fields), `ENUM` (options blocks), `INTERFACE` (contracts — shapes without fields). New modifiers (indices 0–5, in order): `DEPRECATED`, `READONLY`, `MODIFICATION`, `DEFINITION`, `DEFAULT_LIBRARY`, `STATIC`. The legend order is part of the LSP wire protocol — once shipped, the indices cannot be reordered without breaking clients that cached the initial `SemanticTokensLegend`. New modifiers ship at higher indices; new token types append to the end.

- **M8: Modifier mapping is rule-driven by registry + typeck.** `DEPRECATED` ← name appears in `[[banned_jargon]]` or `[[deferred_language_feature]]`. `READONLY` ← typeck classifies binding as `const`. `MODIFICATION` ← typeck classifies binding as `lend` parameter at this site (matches the cautionary red-tinted muted hint per `.claude/rules/inference.md`). `DEFINITION` ← span is a declaration site, not a use site. `DEFAULT_LIBRARY` ← name resolves to a primitive intrinsic (`toString`, `byteAt`) or a type-attached constant (`int.max`). `STATIC` ← type-attached constant only (subset of `DEFAULT_LIBRARY`, distinguishable in themes).

- **M9: `completionItem/resolve` splits hover content from the lightweight item.** Lightweight item carries `label`, `kind`, `sort_text`, `filter_text`. Resolve hook adds `documentation`, `detail`, `additionalTextEdits` (for auto-imports). Reduces initial completion payload by ~80% on lists with 100+ items. `resolve_provider: true` flipped in `capabilities.rs`.

- **M9: VSCode commands ship with `yinz.server.restart` and `yinz.server.showLog` only.** Both are table stakes — every language client provides them. Implementation: extension-side TypeScript calls `client.restart()` and channel reveal. NO `yinz.examples.open`, NO `yinz.registry.browse` — those graduate from v0.2.1 only when the docs site / registry browser exist (v0.3+).

- **M9: Snippets file ships 5 templates only — `shape`, `options`, `function`, `for`-loop, `if-is`-narrowing.** No contract snippet (users learn `follows` by exposure, not by snippet). No UFCS-call snippet (no point — both forms work, and the user types what reads naturally). Snippet location: `tooling/vscode-ynz/snippets/ynz.code-snippets`, contributed via `package.json` `contributes.snippets`. Each snippet body uses Yinz vocabulary exclusively (`shape`, `options`, `follows`, `nothing`, `errors` — never `struct`/`class`/`enum`/`void`).

- **M9: Per-domain inlay hint toggles for the 5 currently-firing domains only.** Settings keys: `yinz.inlayHints.variableType.enabled`, `yinz.inlayHints.ownership.enabled`, `yinz.inlayHints.copyPoints.enabled`, `yinz.inlayHints.arrayToFixedPromotion.enabled`, `yinz.inlayHints.letToConstPromotion.enabled`. Defaults: all `true`. The 4 protocol-only domains (`functionParamType`, `waitPoints`, `lifetimes`, `allocators`) get NO setting yet — they don't fire data, so a toggle would be confusing. Their settings ship when their analysis ships (v0.3+).

- **M9: Master `yinz.inlayHints.enabled` setting kills all hints at once.** Lives alongside the per-domain toggles, ANDed in: a hint fires only when master AND per-domain are both `true`. Matches the typescript-language-server / rust-analyzer pattern.

- **M9: Settings flow via `workspace/configuration` pull, not `initializationOptions`.** Server sends `workspace/configuration` request on startup AND on every `workspace/didChangeConfiguration` notification, pulls the current values, caches them in `ServerState`. The inlay-hint passes consult the cache; no recompilation needed when the user toggles a domain.

- **M9 final phase: `Cargo.toml` version `0.2.1` + tag cut ON THE v0.2.1 BRANCH, then merge-into-v0.3 step (AMENDED 2026-05-30).** On the dedicated v0.2.1 branch, set `Cargo.toml` to `0.2.1`, generate the CHANGELOG section from merged PRs since the `v0.2.0` tag, commit, tag `v0.2.1`, push (with user approval), publish `yinz-0.2.1.vsix` AND `yinz-latest.vsix` to the GitHub release. THEN, as an explicit follow-up step Patrick called out: merge/rebase the v0.2.1 LSP work into the v0.3.0 line so v0.3 inherits all the LSP gap-closure features. The merge keeps v0.3's Cargo version (no downgrade); LSP code is additive and merges cleanly. M9's plan must include this merge-into-v0.3 step as a final phase, not leave it implicit.

- **Milestone execution model: parallel agent dispatch from the main chat.** Each milestone (M1–M9) is implemented by a delegated agent (spawned via `~/.claude/skills/delegate/SKILL.md` patterns) working in an isolated git worktree (`isolation: "worktree"` on the Agent tool call). All independent milestones may be in flight simultaneously after each one's plan-reviewer pass. Merge order to main: M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9. The main chat owns the merge sequence and resolves conflicts at the small shared surface (`crates/ynz-lsp/src/capabilities.rs`, `server.rs`, `lib.rs`, `state.rs`).

- **M6 → M7 hard dependency in the parallel model.** M7 reuses M6's `contract_followers_index_query` for the typeHierarchy SUBTYPES direction. M7's agent may BEGIN work in parallel with M6 by stubbing `contract_followers_index_query` with `unimplemented!()` and a `// REPLACE-AT M7: depends on M6` marker. M7 cannot MERGE to main until M6 lands. During M7's rebase-against-main pass, the stub is removed and replaced with the real query call. No other inter-milestone dependencies exist.

- **M10 (bug fixes) is dispatchable independently of M1–M9.** M10 touches `crates/ynz-typeck/src/check.rs`, `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-lsp/src/hover.rs`, `crates/ynz-lsp/src/inlay_hint.rs`, `crates/ynz-lsp/src/code_action.rs`, `crates/ynz-lsp/src/capabilities.rs` (one line — remove space-trigger). None of these files conflict with M1–M9's primary new-file additions; the M9 `capabilities.rs` flip for `completion_item.resolve_provider: true` is a different line from M10's space-trigger removal. Recommend dispatching M10 first or in parallel with M1 because the bugs it fixes are user-facing right now.

- **M10 unified-PR for the unused-import false-positive bug class.** Bug 1 (the user-reported `Timeframe` false positive) shares root cause with five additional sites: `is`-narrowing, `extends`/`follows`, shape field type annotations, `dynamic Contract`, generic shape names. All six are "missing `self.referenced_names.insert(name)` at this AST position." M10 P0 ships one PR with all six inserts plus walks for `Item::ShapeDecl` and `Item::ConstDecl` in `check_module`, and one regression-test file covering all six patterns. Splitting these into separate PRs would just create six near-identical PRs with shared test scaffolding.

- **M10 inlay-hint positioning fix + Yinz-yellow color contribution.** Replacement-category hints (`array_to_fixed_promotion`, `let_to_const_promotion`) move from `position: span.start` to `position: <end-of-statement-or-pre-user-comment>`. New helper `end_of_let_statement_or_before_comment(text, stmt_span) -> usize` scans the line from `stmt_span.end` for a `//` (excluding inside strings) and returns either the `//` position or end-of-line. Same helper used for any future Replacement-category hints. Color is set in `tooling/vscode-ynz/package.json` via `contributes.configurationDefaults` `"[ynz]": { "editor.inlayHints.foreground": "<yinz-yellow-hex>" }` — language-scoped, doesn't affect other languages, user can override. The exact hex code is a question for Patrick before M10 P4 starts.

- **M10 `collect_maybe_mutated` over-suppression fix threads `sig_table` through the walker.** Currently `inlay_hint_passes.rs:163-170` marks every call argument as "possibly mutated" unconditionally. Fix: look up the callee's signature, mark the argument as mutated ONLY if the corresponding parameter is `lend` or `give`. `share` parameters never mutate (definitional). Requires (a) plumbing `sig_table: &SigTable` through `collect_maybe_mutated`, `collect_maybe_mutated_expr`, and the five `collect_*_block` walkers, (b) handling generic-fn lookups via `generic_fn_table` fallback, (c) handling UFCS method-call form (`player.heal(20)` should resolve `heal` the same as `heal(player, 20)`).

- **M11 (content polish) is dispatchable independently of all other milestones.** M11 touches `registry/features.toml`, `crates/ynz-typeck/src/check.rs` (specific WHY-quality improvements at named lines), `crates/ynz-registry/src/lib.rs` (per-domain hover WHY templates). Zero file conflict with M1–M9; minimal overlap with M10 (M11 P4 touches the same `check.rs` diagnostic-emission sites M10 P8 touches for jargon fixes — but the lines are different, no merge conflict). M11 can start before, during, or after the v0.2.1 release tag — its content fixes are additive and the release notes call them out as "v0.2.1 content patches."

- **M11 P0 (96 primitive intrinsic doc fields) is mechanical content work.** Template per entry: 2-sentence `doc` field — first sentence states WHAT the intrinsic does, second sentence states WHEN to use it (vs. a sibling intrinsic). Example for `int.wrappingAdd(other)`: `"Add two integers, wrapping on overflow instead of panicking. Use this when overflow is expected and silent wrap is the desired behavior (cryptographic hashing, hash-table indexing). For overflow-checked addition use `+` directly — it panics in debug builds and saturates in release per `--release` policy."` The 96 entries can be split across multiple PRs grouped by primitive type (int methods, float methods, string methods, bool methods).

- **M11 P2 (per-domain hover WHY templates) replaces the generic `placement_category` fallback in `crates/ynz-registry/src/lib.rs:111-145`.** Today all 5 Addition-category domains share one WHY paragraph. After M11 P2, each muted-hint domain entry in `registry/features.toml` carries its own `hover_why_template` field that gets interpolated with context (binding name, type name, callee name) at hover-render time. Schema addition: optional `hover_why_template: String` field on `[[muted_hint_domain]]`.

- **No M12 inside v0.2.1.** Visual polish concepts (Pretty TS Errors-style tooltip rendering with markdown color-coding for WHAT/WHAT-INSTEAD/WHY sections, code blocks with embedded TM syntax highlighting in hover popups) are deferred to v0.2.2 or v0.3 polish work. Tracked in `design/lsp.md` as a future-design subsection but explicitly NOT in v0.2.1 scope. Rationale: the visual polish IS valuable but requires UX iteration (markdown rendering quirks differ across editors), and shipping it under v0.2.1 risks scope balloon. Land it as its own roadmap when v0.2.1 is shipped and we know the content quality is right.

These block specific milestones. Resolve before the relevant milestone's execution plan starts.

- **Rename-shadowing UX: reject or prompt?** When a rename would shadow another binding at a use-site, the LSP can either: (a) reject the rename with error -32007 (no rename happens), or (b) accept the rename and let VSCode warn the user with a confirmation dialog. (a) is safer (no chance of silent wrong output) but more disruptive. (b) is more user-friendly but relies on the client to render the warning correctly. — Blocks M1 P0.

- **Code lens computation: per-request vs salsa-cached?** Each code lens computes a reference count via `cross_file_reference_count_estimate`. Two design options: (1) compute on every `textDocument/codeLens` request (simple, may be slow on large files), (2) pre-compute on file open and cache per-file with salsa invalidation (more complex, sub-frame guaranteed). v0.2.0's M5 budget for `cross_file_reference_count_estimate` is <5ms p95, so (1) should work for `examples/pirates-roster/`-scale projects — but production codebases may have 50+ top-level defs. — Blocks M3 P0; M3 P0 includes a perf-spike phase.

- **Lint rule severity default — "suggestion" vs "warning"?** `prefer-fixed-when-immutable` and `mutable-when-const-suffices` are auto-promotions the compiler already applies silently. The squiggle is purely teaching — it doesn't change codegen. Two options: (a) Severity::Suggestion (Tier 3 — `DiagnosticSeverity::HINT`, faint dotted underline by default in VSCode), (b) Severity::Warning (Tier 2 — yellow squiggle). The design doc `linting.md` calls these "Tier 3 lints" but VSCode renders Tier 3 (HINT) as nearly invisible by default, which defeats the teaching mission. — Blocks M4 P0.

- **Signature help re-trigger behavior across line wraps and multi-line arg lists?** VSCode's default behavior is to dismiss signature help on certain key combos (Esc, Enter sometimes). Yinz call sites can span multiple lines for readability. Need to validate that signature help stays open across line continuations or document the limitation. — Blocks M2 P3 (integration testing).

---

## Execution Plan Batching (LOCKED 2026-05-30)

The 11 milestones do NOT each become a separate execution plan. Patrick's call (2026-05-30): combine the genuinely-small, independent milestones into shared plans; keep the medium/large ones separate. This is consistent with the one-plan-at-a-time protocol — the protocol forbids *detail-planning all 11 upfront*, not a single plan covering several small, independent milestones a single agent can execute in one session without context loss.

**Future-me: when you `/plan` the next piece of v0.2.1, use this table to know what to bundle. Write ONE plan per row, when that row's turn comes — do NOT pre-plan rows that aren't being dispatched yet.**

| Plan # | Milestones in the plan | Size | Why batched / kept separate | Dispatch order |
|---|---|---|---|---|
| **1** | **M10** (teaching-surface bugfix) | Large (9 phases) | Standalone — 14 audited bugs, touches files no other milestone touches. Ships FIRST per Q12 (user-facing bugs out in days). | **1st** |
| **2** | **M1 + M3 + M5** | 3× Small | All three reuse existing salsa queries, all independent, all "small LSP nav/correctness additions." One agent knocks them out in one session. | 2nd |
| **3** | **M2** (signature help) | Medium | New salsa query + handler — enough surface to stand alone. | parallel-eligible |
| **4** | **M4** (lint tier-3) | Medium | Registry-schema work (`[[lint_rule]]`) + new `lints.rs` module — distinct surface. | parallel-eligible |
| **5** | **M6 + M7** (type-def/impl + hierarchies) | 2× Medium | M7 has a hard dependency on M6's `contract_followers_index_query` (see locked M6→M7 decision). Natural pair — both are type-navigation, M7's SUBTYPES reuses M6's index. Plan them together; M6's phases land before M7's. | parallel-eligible (internally sequenced) |
| **6** | **M8** (diagnostics + semantic tokens) | Medium-large | Semantic-token legend lock + cross-file related-info + diagnostic tags — distinct surface. | parallel-eligible |
| **7** | **M9** (extension polish + release) | Medium + release | LAST. Depends on M1–M8 merged. Owns the version bump, tag cut, AND the merge-into-v0.3.0 step (amended decision above). | last |
| **8** | **M11** (content polish) | Large (11 phases) | Standalone — ships as `v0.2.1.1` content patch AFTER the main tag (Q13). 2 new phases added 2026-06-01: P_A (go-model `//` comment implementation) + P_B (colored function signature hover). Shape/options hover already shipped ad-hoc on main. | after release tag |

**Batching rationale captured so it survives compaction**: the only INTRA-plan dependency is M6→M7 (inside Plan 5). All other plans are independent and parallel-dispatchable via the worktree-agent model (locked architectural decision "Milestone execution model: parallel agent dispatch"). The merge-to-main sequence for the headline features stays M1→M9; M10 can merge any time (no conflicts); M11 lands post-tag.

## Milestones

### Milestone 1: Rename Shadowing Fix
**Value delivered**: Renaming a symbol can never produce silent-wrong-output by creating an accidental shadow. The compiler rejects the rename with a clear error pointing at the conflicting use-site, and the user picks a different name. A Tier A correctness bug class is eliminated.
**Execution plan**: `v0-2-1-m1-rename-shadowing` (status: planned)
**Depends on**: Nothing — focused typeck API addition.
**Rough scope**: Add `bindings_in_scope_at(db, sf, byte_offset) -> HashSet<String>` to `crates/ynz-typeck/src/scope.rs`. Extend `crates/ynz-lsp/src/rename.rs` to query this for each use-site; reject with error code -32007 if the new name shadows. Add tests including the exact scenario from the v0.2.0 audit ("rename `qux` → `x` where `x` is locally bound at one use-site"). Extend the rename error catalog in the registry. Update `examples/pirates-roster/entrypoint.ynz` with a rename scenario that demonstrates the shadowing check.

### Milestone 2: Signature Help
**Value delivered**: Typing inside a function call shows the parameter list, names, types, and active argument position inline. Reduces "what am I supposed to pass here?" friction to zero. The first thing TypeScript and Rust developers expect from any IDE — Yinz finally has it.
**Execution plan**: `v0-2-1-m2-signature-help` (status: planned)
**Depends on**: Nothing — new salsa query + LSP handler.
**Rough scope**: New `signature_help_at_offset(db, sf, byte_offset)` salsa query in `crates/ynz-typeck/src/queries.rs`. New `crates/ynz-lsp/src/signature_help.rs` module with `signature_help_response(state, uri, position) -> Option<SignatureHelp>`. Wire `textDocument/signatureHelp` request in `server.rs`. Advertise `signature_help_provider` with `triggerCharacters: ["(", ",", "<"]` in `capabilities.rs`. Tests covering: nested calls, calls with no args, calls with default args (when supported), calls inside multi-line expressions, after-`,` retrigger. Extend the canonical demo project.

### Milestone 3: Code Lens — References Count
**Value delivered**: Every top-level function, shape, and options block shows "N references" inline above its definition. Click the lens to jump to all references. Makes the codebase self-navigating without the user opening Find Symbols.
**Execution plan**: `v0-2-1-m3-code-lens` (status: planned)
**Depends on**: Nothing — uses existing `cross_file_reference_count_estimate` query.
**Rough scope**: New `crates/ynz-lsp/src/code_lens.rs` with `code_lens_response(state, uri) -> Vec<CodeLens>`. Walks the parsed module's top-level items, calls `cross_file_reference_count_estimate` per def, returns one `CodeLens` per item with a `command` payload that triggers `editor.action.referenceSearch.trigger` (VSCode-standard). Wire `textDocument/codeLens` request handler in `server.rs`. Advertise `code_lens_provider` in `capabilities.rs`. M3 P0 perf-spike phase resolves the per-request-vs-cached open question with measurements.

### Milestone 4: Lint Rules Tier 3 — Infrastructure + 2 Starter Rules
**Value delivered**: The two auto-promotion analyses that already fire as inlay hints (`array_to_fixed_promotion`, `let_to_const_promotion`) ALSO fire as tier-3 lint suggestions — yellow squiggles in the editor with full WHAT/WHAT-INSTEAD/WHY hover. Future lint rules ship via a single registry entry + analysis pass, not a milestone-scale effort.
**Execution plan**: `v0-2-1-m4-lint-tier3` (status: planned)
**Depends on**: Nothing — additive diagnostic emission path.
**Rough scope**: Add `[[lint_rule]]` schema to `registry/features.toml` and `crates/ynz-registry/src/schema.rs`. New `crates/ynz-typeck/src/lints.rs` module with rule traits. Extend `DiagnosticKind` with `LintSuggestion { rule_name: String }`. Wire emission through `check_query`. Ship two rules: `prefer-fixed-when-immutable` and `mutable-when-const-suffices` — both reuse existing analyses from `inlay_hint_passes.rs`. Extend `tests/registry_parity.rs` with the lint-rule reachability check. Extend `examples/primantis-orders/` with a fixture that triggers both lints intentionally.

### Milestone 5: Document Highlight + Selection Range
**Value delivered**: cursor on an identifier highlights every other occurrence in the current file (the visual-scan tool every TS/Rust dev uses every minute). Smart-expand selection (Cmd+Shift+L / Alt+Shift+→) grows by AST node — identifier → call → block → function — instead of falling back to whitespace-delimited word selection. UFCS sites highlight together (`player.heal(20)` and `heal(player, 20)` register as references to the same symbol).
**Execution plan**: `v0-2-1-m5-doc-highlight-selection-range` (status: planned)
**Depends on**: nothing — both reuse existing salsa queries.
**Rough scope**: new `crates/ynz-lsp/src/document_highlight.rs` reuses the existing references query with a same-file filter and a `DocumentHighlightKind` classifier (READ / WRITE / TEXT). New `crates/ynz-lsp/src/selection_range.rs` walks the parsed AST node chain bottom-up via the existing `ast_offset.rs` walker. Advertise `document_highlight_provider` and `selection_range_provider` in `capabilities.rs`. Tests: doc-highlight across UFCS sites; selection-range chain depth on a 5-nested-call fixture. Extend `examples/pirates-roster/entrypoint.ynz` with a section that demonstrates both navigation aids.

### Milestone 6: Type Definition + Implementation Provider
**Value delivered**: cmd-click on a value with alt jumps to the value's TYPE (not the variable declaration). Cursor on a contract (`shape Comparable`) and "Go to Implementations" returns every shape that follows the contract. This is the missing piece for contract-based design in non-OOP Yinz — without it, going from a `follows Damageable` contract to its implementers means find-refs + manually filtering.
**Execution plan**: `v0-2-1-m6-type-def-implementation` (status: planned)
**Depends on**: nothing — `contract_followers_index` salsa query is a new but small addition.
**Rough scope**: new `crates/ynz-lsp/src/type_definition.rs` and `crates/ynz-lsp/src/implementation.rs`. New `contract_followers_index_query` in `ynz-typeck/src/queries.rs` populated alongside the existing signature pass — maps contract name → `Vec<shape_decl_loc>`. Advertise `type_definition_provider` and `implementation_provider` in `capabilities.rs`. Tests: type-def on a `let p: Player = ...` jumps to `shape Player`; implementation on a contract returns all followers; implementation on a data-shape returns an empty list (locked decision). Extend `examples/pirates-roster/entrypoint.ynz` with a contract + 2+ followers and demonstrate the navigation.

### Milestone 7: Type Hierarchy + Call Hierarchy
**Value delivered**: tree-structured navigation for `extends` chains (parent → child shapes) and `follows` relationships, unified into one type-hierarchy view. Call hierarchy gives "incoming calls" and "outgoing calls" as expandable trees — the IntelliJ-style impact-of-change tool. Both extend existing find-refs from flat list to tree.
**Execution plan**: `v0-2-1-m7-type-hierarchy-call-hierarchy` (status: planned)
**Depends on**: M6 — type hierarchy reuses the `contract_followers_index` for the SUBTYPES direction.
**Rough scope**: new `crates/ynz-lsp/src/type_hierarchy.rs` with `prepare`, `supertypes`, `subtypes` handlers. New `crates/ynz-lsp/src/call_hierarchy.rs` with `prepare`, `incoming_calls`, `outgoing_calls` handlers. Supertypes walks `extends` chain + `follows` declarations; subtypes uses the contract-followers index plus a new `shape_children_index` for `extends` reverse-lookup. Call hierarchy reuses references query for incoming, AST walk for outgoing. Advertise `type_hierarchy_provider` and `call_hierarchy_provider` in `capabilities.rs`. Tests: 3-level `extends` chain navigates supertypes and subtypes; call hierarchy on a recursively-called `fib` function shows expected incoming tree.

### Milestone 8: Diagnostic Enrichment + Semantic Token Modifiers
**Value delivered**: deprecated features (banned-jargon like `enum` → `options`, deferred features like `verified { }`) render struck-through in the editor instead of plain-squiggle. Unused imports/bindings render dimmed when the unused-import lint ships. Cross-file related-information surfaces ("see also: services/crew.ynz") instead of being silently dropped. Semantic-token modifiers turn `const` italic / `lend` parameter-cautionary / primitive intrinsics dimmed — the visual teaching surface that pairs with muted inlay hints.
**Execution plan**: `v0-2-1-m8-diagnostic-enrichment-semantic-tokens` (status: planned)
**Depends on**: nothing — additive across multiple narrow files.
**Rough scope**: extend `ynz_diagnostics::Diagnostic` with a `tag: Option<DiagnosticTag>` field. Banned-jargon and deferred-feature diagnostics emit `Deprecated`. Wire the tag through `diagnostic_transform.rs` as `lsp_types::DiagnosticTag::DEPRECATED`. Add `ServerState::text_and_line_table_for(path)` for cross-file related-info resolution; remove the silent same-file filter in `diagnostic_transform.rs`. Extend `SEMANTIC_TOKEN_LEGEND` with `STRUCT` / `ENUM` / `INTERFACE` token types (indices 10–12) and 6 modifiers (`DEPRECATED` / `READONLY` / `MODIFICATION` / `DEFINITION` / `DEFAULT_LIBRARY` / `STATIC`, indices 0–5). Modifier emission rules per the locked architectural decision above. Extend parity test to verify modifier coverage. Tests: deprecated tag rendered on `enum` keyword diagnostic; cross-file related-info appears in a diagnostic that points across files; semantic-tokens emit `READONLY` on every `const` binding.

### Milestone 9: Extension Polish + Release
**Value delivered**: completion item resolve cuts payload size for big completion lists. Snippets ship for 5 canonical patterns (shape, options, function, for, if-is-narrowing). Two commands ship (restart server, show log). Per-domain inlay hint toggles let advanced users hide noise without killing all hints. Cargo.toml bumps to 0.2.1; release tag cut.
**Execution plan**: `v0-2-1-m9-extension-polish-release` (status: planned)
**Depends on**: M1–M8 merged to main before the version bump.
**Rough scope**: split `to_lsp_completion_item` into lightweight item + resolve hook; flip `resolve_provider: true`. Ship `tooling/vscode-ynz/snippets/ynz.code-snippets` with 5 templates using Yinz vocabulary exclusively. Add `yinz.server.restart` and `yinz.server.showLog` commands in `package.json` + `extension.ts`. Add `yinz.inlayHints.enabled` master setting + 5 per-domain settings in `package.json` `contributes.configuration`. Server pulls settings via `workspace/configuration` request, caches in `ServerState`, consults in inlay-hint passes. Final phase: bump `Cargo.toml` to `0.2.1`, regenerate CHANGELOG from PRs since `v0.2.0` tag, commit, tag, push (with user approval), publish `yinz-0.2.1.vsix` + overwrite `yinz-latest.vsix` per the project release convention.

### Milestone 10: Teaching-Surface Correctness (Bug Hunt Cluster)
**Value delivered**: every bug found by the 2026-05-21 four-agent teaching-surface audit gets fixed. Six false-positive unused-import sites (including the user-reported `Timeframe` case) stop firing on valid code. Inlay hints stop rendering at the wrong line position. The `array_to_fixed_promotion` hint becomes click-to-make-explicit (matching its sibling). Promotion hints stop being silently suppressed on every binding that's passed to a function. Hover stops returning the keyword hover when the user named a variable after a keyword. The `BannedJargon` diagnostic gets a quick-fix lightbulb. The space character stops triggering the completion popup on every prose keystroke. The `booleanean` typo and three `infers` banned-jargon violations get fixed.
**Execution plan**: `v0-2-1-m10-teaching-surface-bugfix` (status: planned)
**Depends on**: nothing — touches files that no other milestone touches.
**Rough scope** (one phase per bug cluster):
- **P0 — Unused-import false-positive bug class (6 sites)**: add `self.referenced_names.insert(name)` at six AST positions in `crates/ynz-typeck/src/check.rs` (Bug 1 `check_options_value:3574`, Bug 2.1 `check_is_arm_pattern:3472` + `check_is_expr:3522`, Bug 2.2 `check_follows_contracts:2502` + walk for `Item::ShapeDecl`, Bug 2.3 walk for shape field types + `Item::ConstDecl`, Bug 2.4 `AstType::Dynamic:2294`, Bug 2.5 `AstType::Generic:2379`). One regression-test file covering all six patterns. Single PR — shared root cause.
- **P1 — `Stmt::Match.else_arm` blindspot fix**: extend all five `collect_*_block` walkers in `inlay_hint_passes.rs` to visit `else_arm.as_ref()` in addition to `arms`. Currently `else_arm` is silently skipped, causing `let_to_const_promotion` to fire on bindings mutated inside `else =>` catch-all arms.
- **P2 — Nested `FieldAssign`/`IndexAssign` root-binding tracking**: replace the one-level `if let Expr::Ident(name, _) = receiver.as_ref()` checks in `inlay_hint_passes.rs:119-134` with a `root_ident()` walker that descends through chained `FieldAccess`/`IndexAccess` to find the root identifier. Fixes `player.address.street = "x"` not marking `player` as mutated.
- **P3 — `collect_maybe_mutated` over-suppression fix**: thread `sig_table` through `collect_maybe_mutated_expr` and the five `collect_*_block` walkers. Mark call arguments as mutated ONLY when the callee's parameter is `lend` or `give`; `share` parameters don't mutate. Add generic-fn fallback via `generic_fn_table`. Add `Expr::MethodCall` arm for UFCS. Recover the promotion-hint coverage that's currently invisible on 95% of real bindings.
- **P4 — Inlay hint positioning fix + Yinz-yellow color contribution**: replace `position: span.start` with end-of-statement-or-pre-user-comment position via new helper `end_of_let_statement_or_before_comment`. Apply to `array_to_fixed_promotion_hints` and `let_to_const_promotion_hints`. Add `tooling/vscode-ynz/package.json` `contributes.configurationDefaults` `"[ynz]": { "editor.inlayHints.foreground": "<yinz-yellow-hex>" }` — pending Patrick's hex code.
- **P5 — `array_to_fixed_promotion` click-to-make-explicit wire-up**: build `array_to_fixed_edit` helper (analogous to existing `let_to_const_edit`) that replaces the `array` token in the type annotation with `fixed`. Requires `PromotionHint` struct to carry the type-annotation byte range. Switch `array_to_fixed_promotion` from `make_hint` to `make_hint_with_edit`.
- **P6 — Hover fixes**: (a) reorder hover lookup in `hover.rs:112-121` so user-defined symbols win over registry keywords for non-position-bound tokens like `share` / `lend` / `give` (fixes "hovering on `let share = 5; share + 1`" returning the keyword hover); (b) change the cursor-in-token check from `< tok.span.end` to `<= tok.span.end` to handle end-of-token cursor positions (or fall back to previous token).
- **P7 — Space-trigger removal + `BannedJargon` quick-fix**: remove `" ".to_string()` from `capabilities.rs:38` `trigger_characters`. Extend `code_action_response` in `code_action.rs:57-76` with a `DiagnosticKind::BannedJargon { term }` arm that calls `lsp_code_action_replacement_for("BannedJargon", term)`. Extend `lsp_code_action_replacement_for` in `crates/ynz-registry/src/lsp_adapter.rs` to search `[[banned_jargon]]` entries.
- **P8 — Typo + jargon fix-ups**: fix `booleanean` typo at `check.rs:1441`. Replace `"infers"` with `"figures out"` at `check.rs:1829`, `1835`, `1848`. Replace any other `[[muted_hint_domain]]` description fields that use `infer`/`inferred` in user-facing text (per teaching-content audit).

### Milestone 11: Teaching-Content Polish (Content Push)
**Value delivered**: Go-model `//` comment syntax implemented (replaces `///` as canonical form; `///` stays working as alias). Function signatures render as colored `ynz` code blocks in hover instead of gray text. Field hover on dot-access (`p.health` cursor on `health`) returns the field type. The 96 primitive intrinsic entries that ship with empty hover docs get populated. The 9 pedagogically critical keywords get hover content beyond "Introduced in M4". Per-domain hover WHY templates replace the generic `placement_category` fallback. Inlay hint render strings get concrete-type + binding-name context. The 15 highest-leverage diagnostic WHY-quality improvements ship. Shape and options hover (field/variant listings with syntax-highlighted code blocks) already shipped as an ad-hoc fix 2026-06-01 — this milestone closes the remaining gaps.
**Execution plan**: `v0-2-1-m11-teaching-content-polish` (status: planned)
**Depends on**: nothing — pure content + small parser/lexer additions for go-model comments + small typeck additions for field hover.
**Rough scope** (one phase per content cluster):
- **P_A — Go-model `//` comment implementation** *(new — locked 2026-06-01, see `design/doc-comments.md`)*: change the lexer to emit `Token::LineComment(text)` for `//` lines immediately before a top-level declaration instead of discarding them as trivia. Change `///` handling to produce the same token (both strip leading slashes — `///` becomes an alias, not a distinct token). Update the parser's declaration handling (`parse_shape_decl`, `parse_function_decl`, `parse_options_decl`, `parse_const_decl`) to look backwards for preceding `LineComment` tokens with no intervening blank lines and collect them as the `doc: Option<String>` field. Remove the old `Token::DocComment` path (or fold it into `LineComment`). Update `spec/doc-comments.md` examples from `///` to `//`. Add regression tests: `//` immediately before declaration attaches; blank line between breaks attachment; `//` inside function body does NOT attach; `///` works identically to `//`.
- **P_B — Function signature as colored `ynz` hover block** *(new)*: change `user_symbol_hover` in `hover.rs` to render the function signature as a fenced ` ```ynz ``` ` code block instead of `## \`function name(...)\`` gray text. The fenced block gets full TextMate grammar syntax highlighting in VSCode — keyword, type, and ownership-modifier colors automatically. Format: doc comment prose (if any) → `---` separator → ` ```ynz\nfunction name(params) -> ReturnType\n``` `. Update the two doc-comment hover tests in `tests/hover.rs` that assert on the old `##` format. Add a test that asserts the hover body contains ` ```ynz ` to lock the colored-block format.
- **P0 — Primitive intrinsic doc fields (96 entries)**: write a 2-sentence `doc` field for every `[[primitive_intrinsic]]` entry in `registry/features.toml` that's missing one. Template: WHAT the intrinsic does (1 sentence), WHEN to use it vs. a sibling intrinsic (1 sentence). Split across multiple PRs grouped by primitive type (int methods, float methods, string methods, bool methods, number methods). Highest-priority entries first: `wrappingAdd` / `wrappingSub` / `wrappingMul` (only overflow escape); `sortFast` / `sortStrict` (auto-promotion overrides); `count` / `byteCount` / `graphemeCount` (semantically distinct).
- **P1 — Keyword doc fields (9 entries)**: add an optional `doc` field to `[[keyword]]` schema in `crates/ynz-registry/src/schema.rs`. Populate for `follows`, `extends`, `base`, `dynamic`, `hidden`, `wait`, `background`, `errors`, `sensitive`. Format: 2-sentence summary + 1-line code example. Extend `lsp_hover_for_token` keyword branch to append `doc` when present.
- **P2 — Per-domain hover WHY templates**: extend `[[muted_hint_domain]]` schema with optional `hover_why_template: String` field. Populate for all 9 domains (the 5 firing + 4 protocol-only). Refactor `crates/ynz-registry/src/lib.rs:111-145` `lsp_inlay_hint_hover_for` to interpolate the per-domain template with context (binding name, type name, callee name) instead of returning a single generic paragraph per placement category. Removes the Rule 11 violation where Addition-category hover WHYs are mechanically generic.
- **P3 — Inlay hint render text refinements**: rewrite the 5 firing hint label strings in `inlay_hint_passes.rs` to include concrete type + binding name context. Examples: `// promoted to fixed<{T}, {N}> — {name} never grown` for `array_to_fixed`; `// effectively const — {name} never reassigned` for `let_to_const`; `: {T} (figured out from {expr})` for variable_type (showing what the inference saw). The string templates accept substitution args from the PromotionHint/TypeHint structs.
- **P4 — Top 15 diagnostic WHY-quality improvements**: ship the 15 highest-leverage diagnostic improvements identified by the diagnostic-quality audit (`/workspaces/ynz/.analysis/diagnostic-why-quality.md`). Examples: argument-count mismatch's WHAT-INSTEAD shows the actual signature instead of just a count; use-after-give names the consuming function via the existing `with_related` span; const-reassignment WHY explains the compile-time contract instead of restating the rule; condition-not-boolean WHY suggests `x != 0` when the actual type is int (contextual variant). Each improvement is a 1–5 line code change at a named line in `check.rs`.
- **P5 — Sized-integer deferred entry rewrites (9 entries)**: rewrite the 9 copy-pasted WHY paragraphs in `[[deferred_language_feature]]` entries for i8/i16/i32/u8/u16/u32 etc. Use the existing `u64` entry as the model (concrete range, concrete use-case).
- **P6 — Muted-hint domain description fixes**: replace banned `infer`/`inferred` in 3 `[[muted_hint_domain]]` description fields with "figures out" or "the compiler picks" per `.claude/rules/vocabulary.md`.
- **P7 — Code action description field population**: every code action returned by `code_action.rs` gets a `description` field with a one-sentence WHY explaining why the old form was banned (sourced from the registry's `[[banned_jargon]]` or `[[banned_declaration_keyword]]` `why` field). Users clicking the lightbulb learn the rule, not just the fix.
- **P8 — Field hover on dot-access** *(shape/options hover shipped ad-hoc 2026-06-01 — field hover is the remaining half)*: add field-access detection to `hover_response`. When the cursor token is an identifier that appears inside a `FieldAccess` expression in the AST, look up the receiver's declared type from annotated parameters and let-bindings (cheap AST walk, no full type inference needed for the common case). Resolve the field's type from `shape_table`. Emit `field_name: FieldType` plus the field's trailing `//` doc comment when present. Edge case: unannotated receiver type falls back to `None` hover — never wrong, sometimes incomplete (full inference deferred to a future check_query hover path).

---

## Out of Scope

- **Multi-rule lint pack (>2 rules) in v0.2.1.** Adding rule #3, #4, #N is a v0.3+ task — each gets a registry entry + analysis pass under the infrastructure that M4 ships. Mid-roadmap requests to "while we're at it, add rule X" get redirected to a follow-up plan. Rationale: locks the M4 scope to "infrastructure + 2 rules" so it doesn't balloon.
- **Code lens "run test" / "debug test" lenses.** Requires a test framework (v0.4) and a debug adapter (v0.5+). Not blocking IDE quality.
- **Code lens "show implementations" lens for `follows`.** The existing find-references handler already covers this navigation. Adding a dedicated lens is redundant in v0.2.1.
- **Signature help for lambdas.** Lambdas are deferred to M8/v0.3+; signature help waits for them. The signature help that ships in M2 covers all v0.1/v0.2 function calls.
- **Cross-language rename (Yinz symbol from CSS/HTML).** No cross-language scenarios exist.
- **User-defined lint rules (plugin system).** Documented in `design/linting.md` as a v1.x+ surface. Out of scope for any v0.x milestone.
- **Concurrency-adjacent lints** (suspension-point hints, `wait`/`background` muted hints). These ship with v0.3-M3 (`wait_points` muted-hint domain, currently protocol-only). Including them in v0.2.1 would prematurely lock decisions that v0.3 architecture should make.
- **Performance optimization sweep across the LSP.** Step-7 perf budgets stay where they are; v0.2.1 hits them but doesn't aggressively pursue lower numbers.
- **VSCode extension theme work.** v0.2.0's TextMate grammar stays. No semantic-token modifier additions, no theme JSON, no icon work.
- **`ynz-lints` crate extraction.** Premature at 2 rules. Revisit when the rule count crosses ~10.
- **`onTypeFormatting` provider (format-as-you-type).** Not included in v0.2.1. Format-on-save via `ynz fmt` already covers the canonical formatting need. `onTypeFormatting` adds keystroke-driven formatting that could fight with the save-time formatter. Revisit if user feedback emerges that wants live formatting.
- **Custom `.ynz` file icon contribution.** Not in v0.2.1. Effort-vs-payoff judgment — VSCode's default text icon is fine until brand identity matters (closer to v1.0).
- **Walkthrough / welcome page** (`contributes.walkthroughs`). Not in v0.2.1. Effort-vs-payoff — the example projects and the README cover onboarding adequately for the v0.2 audience size.
- **`linkedEditingRange` provider.** Not planned. Yinz has no paired-identifier syntax that would benefit (no JSX-style tag pairing). Re-evaluate if a future feature introduces one.
- **`inlineValue` provider.** Blocked by the debug adapter (v0.5+, no design doc yet). Tracked in `design/lsp.md` "Capability Inventory" table.
- **`moniker` provider.** Blocked by the package manager (v0.22). Cross-repo navigation requires per-package symbol monikers. Tracked in `design/lsp.md`.
- **Notebook controller.** No Yinz notebook format exists. Not planned for any current milestone.
- **`workspace/didChangeWatchedFiles` server-side handler.** Today the extension creates the watcher but the LSP server doesn't subscribe. Ships when v0.22 package manager needs lock-file change detection OR when external file edits (CLI formatter, code-mod scripts) need to invalidate salsa inputs. Tracked in `design/lsp.md`.
- **Pull-diagnostics (`textDocument/diagnostic`).** Already deferred per registry `lsp-pull-diagnostics` entry.
- **`codeDescription` URI on diagnostics.** Already deferred per `design/lsp.md` — ships when the docs site lands.
- **Refactor code action kinds (`SOURCE_ORGANIZE_IMPORTS`, `REFACTOR_EXTRACT`, `REFACTOR_INLINE`).** Blocked by the refactor catalog (need import-sorter for organize, extract-function pass for extract). Tracked in `design/lsp.md` "Capability Inventory" table.
- **`yinz.examples.open` and `yinz.registry.browse` VSCode commands.** Defer until the docs site / registry browser exist (v0.3+). The two commands shipping in M9 (`yinz.server.restart`, `yinz.server.showLog`) are the unconditional table-stakes pair.
- **Per-domain inlay hint toggles for the 4 protocol-only domains** (`functionParamType`, `waitPoints`, `lifetimes`, `allocators`). No setting until the underlying analysis ships data — a toggle for "hide this thing that doesn't render" would be noise. Their settings land when their analysis lands (v0.3+).
- **Semantic-token modifier expansions beyond the 6 in M8.** Future candidates (e.g., `ABSTRACT` for `base shape`, `ASYNC` for `wait` calls once concurrency lands) ship in later versions. The M8 legend order is permanently locked; new modifiers append at higher indices.

- **Visual polish phase (Pretty TypeScript Errors-style tooltip rendering).** Deferred to a v0.2.2 follow-up roadmap. Concept: color-coded tooltip sections (WHAT in red, WHAT-INSTEAD in yellow, WHY in blue via markdown HTML `<span style>` or emoji prefixes 🔴🟡🔵); embedded code blocks with TM-syntax-highlighted Yinz snippets in hover popups (use ` ```ynz ` fenced blocks — the TM grammar is already shipped with the extension); bordered/boxed popup styling via CSS-styled markdown. Tracked as a future-design subsection in `design/lsp.md`. NOT in v0.2.1 scope — the underlying content quality (M10–M11) must land first; styling broken content is wasted effort.

- **Type-mismatch quick-fix actions.** Identified as a HIGH opportunity by the UX-friction audit (wrap `int → string` mismatch with `.toString()` automatically; same for `float → int`, `int → float`, `number → string`). Deferred to v0.2.2 or v0.3 because the design requires a lookup table mapping `(actual, expected) → intrinsic` AND the diagnostic must carry both types in its data. M10/M11 fix higher-leverage gaps first.

- **Multi-symbol import quick-fix surgical removal.** Today the `UnusedImport` quick-fix deletes the whole import line even when only one of N named items is unused (per `code_action.rs:205-208` `intentional for v0.2`). Deferred to v0.2.2 or v0.3 because it requires AST per-name spans inside an import declaration; M10/M11 don't touch the AST.

- **Hover end-of-token edge case beyond the M10 P6 fix.** Bug 2.12 in the audit identified the strict `< tok.span.end` check; M10 P6 ships the `<=` fix OR previous-token fallback. Edge cases beyond that (cursor inside a comment block adjacent to a token, cursor at start-of-file before any token) are deferred — they're not user-felt.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Lint rules milestone scope balloons mid-sprint ("just add this one more rule") | Medium | High | Architectural decision locks scope to 2 starter rules. Requests for additional rules redirect to v0.3 follow-up plans. No scope creep accepted without a new plan. |
| Rename shadowing fix requires per-use-site scope walk that proves too slow on large files | Low | Medium | Existing salsa caching covers most repeated lookups. M1 P0 includes a perf benchmark on `examples/pirates-roster/`. Correctness wins over speed — if slow, ship the correct version and optimize in v0.3. |
| Code lens "N references" causes editor lag on large files via per-keystroke recomputation | Medium | High | M3 P0 perf spike resolves the per-request-vs-cached question with measurements. Budget: <5ms p95 per file for v0.2.1 release. If exceeded, switch to cached variant before merging. |
| Signature help interaction with completion popup creates UX confusion | Low | Low | LSP protocol separates `signatureHelp` from `completion` — they're orthogonal requests. M2 integration tests cover the overlap case (typing inside `(` while completion is active). |
| Lint suggestion severity (HINT vs WARNING) gets the visibility tradeoff wrong | Medium | Medium | M4 P0 resolves the severity open question with a screenshot comparison from a real `pirates-roster/` build. User feedback drives the choice. |
| Lint rule fires too eagerly, becomes noise rather than teaching | Medium | High | Both starter rules ship with the same analysis already running silently for inlay hints. The squiggle is purely surfacing what's already happening — no new analysis, no new false positives. |
| VSIX install confusion across version (users on v0.2.0 install latest, can't tell they're now on v0.2.1) | Low | Low | Final M4 phase bumps version, updates CHANGELOG, verifies `ynz --version` prints `0.2.1`, updates `yinz-latest.vsix` symlink. Release notes published with the tag. |
| One of the nine milestones can't ship cleanly — should the rest of v0.2.1 ship without it? | Medium | Medium | Each milestone is independent (except M7's soft dependency on M6's `contract_followers_index`, which can ship without M7 if needed). If a single milestone hits an architectural snag, v0.2.1 ships with that milestone deferred to v0.2.2. Roadmap front-matter `milestones:` list updated accordingly. |
| Registry parity test gets stricter and breaks the M4 lint integration mid-sprint | Low | Medium | M4 explicitly EXTENDS the parity test as a phase deliverable. Stricter checks are designed-in, not retrofitted. |
| Roadmap doubles in size — pressure to skip phases or per-milestone code reviews builds | Medium | High | Existing rule: every phase gets a code-reviewer agent before merge (per `feedback_all_phases_then_review.md`). All-phases-then-review model means user sees the full milestone diff at the end, not per phase, but the code-reviewer agent runs after every phase. No bypass. |
| Semantic-token legend order locks accidentally mid-sprint (modifier added at wrong index, breaking client caches) | Low | High | M8 includes a snapshot test (`semantic_tokens_legend_snapshot.rs`) that asserts the exact order. Adding a modifier requires updating the snapshot deliberately. Index changes are caught as a test diff. |
| `implementation` provider returns confusing results for shapes that are BOTH contracts (have bare signatures) AND data shapes (have fields) — Yinz allows this combination via `base shape` | Low | Medium | M6 architectural decision: `implementation` operates on shapes with NO fields (pure contracts). For mixed shapes, return empty list rather than partial results. Tested with a `base shape` fixture in M6. |
| Inlay hint toggles cause user confusion ("I disabled the hint but it still shows up because it's from a different domain") | Low | Low | M9 ships with a single short explainer in the README ("each domain controls one hint type — see hover tooltips for which domain produced a given hint"). Hover tooltip text annotated with domain name where space permits. |
| Cross-file related-info filesystem reads cause LSP responsiveness regressions on large projects | Low | Medium | M8 caches the resolved (path → text + line table) per LSP request, not per session. Repeat lookups in one request are O(1); cross-request invalidation handled by salsa input updates. Budget: cross-file related-info added latency ≤ 5ms per diagnostic batch. |
| Snippet content drifts from canonical Yinz vocabulary (e.g., someone adds `enum` snippet thinking it's helpful) | Low | High | M9 ships a snapshot test against `snippets/ynz.code-snippets` content that asserts no banned-jargon strings appear (`enum`, `struct`, `class`, `void`, `null`, etc.). Snippet additions in future PRs must pass the jargon assertion. |
| Parallel agent dispatch creates merge conflicts at the shared LSP surface (`capabilities.rs`, `server.rs`, `lib.rs`, `state.rs`) | Medium | Medium | Main chat owns the merge sequence (M1 → M9). Each agent works in an isolated git worktree. Conflicts at the shared surface are mechanical (add a line to a list) and resolved during the main-chat-driven rebase pass before merge. M6→M7 stub-and-rebase pattern documented in the locked architectural decision. |
| M7 agent ships its branch before M6 lands (race condition in parallel dispatch) | Low | High | M7 agent's plan-reviewer pass MUST verify the `// REPLACE-AT M7: depends on M6` stub is present and not silently removed. Main chat checks M6 is merged-to-main before approving M7's PR. CI blocks any branch that builds successfully WITH the stub in place (the stub `unimplemented!()` panics in any test that touches typeHierarchy SUBTYPES). |
| Main chat loses track of which agents are in flight, dispatches duplicate work | Medium | Medium | Per-milestone front-matter `slug:` field is bound to the agent's worktree via `/init-chat` at agent startup. Main chat maintains a live tally of dispatched/in-progress/merged milestones in this roadmap's RADAR-START block (auto-rebuilt by SessionStart hook). Duplicate dispatch is visible immediately. |
| M10 P3 `collect_maybe_mutated` over-suppression fix changes hint coverage drastically — users see hints suddenly appearing on bindings they'd previously assumed never triggered | High | Low | Document the behavior change in the v0.2.1 CHANGELOG. The new behavior is the CORRECT behavior — the old over-suppression was a bug. Per-domain inlay hint toggles (M9) let users mute the suddenly-loud domain if they want; defaults remain `true`. |
| M10 P0 unified PR (6 unused-import sites) is rejected by reviewer because it's "too many changes in one PR" | Low | Medium | The 6 sites share root cause AND share a test file — splitting creates near-duplicate review work. M10 P0 plan-reviewer pass MUST explicitly justify the unified PR in the plan; if the reviewer still wants splits, fall back to 3 PRs grouped by AST-position-family (variant-access; pattern-matching; declaration-positions). |
| M11 P0 96-entry doc population stalls because the entries get reviewed too aggressively (every WHEN-to-use sentence litigates an edge case) | Medium | Medium | M11 P0 plan-reviewer pass establishes a docstring TEMPLATE up-front (sentence 1: WHAT; sentence 2: WHEN vs sibling). Reviewers apply the template, not their own preference for prose style. Per-PR scope: one primitive type's intrinsics at a time (e.g., one PR for all `int.*` intrinsics, one for all `string.*`). |
| M11 P2 per-domain hover WHY templates change every existing hover tooltip — users notice mass tooltip rewrites and worry about regression | Low | Low | M11 P2 ships a snapshot test that asserts the new templates render correctly for every domain. The change is purely additive content quality — no behavior change. |
| `editor.inlayHints.foreground` Yinz-yellow color contribution conflicts with user's existing theme override | Low | Low | The setting is language-scoped (`[ynz]` block) so it only applies to .ynz files. Users who've already configured `editor.inlayHints.foreground` globally still see their global setting unless they want the Yinz-yellow scope. Document in M10 P4's PR description. |

---

## Resolved Questions (Locked 2026-05-21)

All ten roadmap-level strategy questions resolved in the planning chat. Each is recorded here with the locked answer; new questions should be appended below with their own ANSWERED note when resolved.

1. **Lint rules scope** — **ANSWERED 2026-05-21**: lock to "infrastructure + 2 starter rules" (`prefer-fixed-when-immutable`, `mutable-when-const-suffices`). Adding rule #3 is a v0.3+ follow-up plan; it inherits M4's infrastructure with no milestone-scale effort.

2. **Release model** — **ANSWERED 2026-05-21**: single `v0.2.1` tag cut after all 9 milestones merge to main. No per-milestone version cascade. Matches v0.2.0's model (M1–M5 → one tag).

3. **Milestone ordering (M1–M4)** — **ANSWERED 2026-05-21**: keep proposed order — M1 rename shadowing (correctness bug) → M2 signature help → M3 code lens → M4 lint tier-3. No reordering.

4. **Concurrent execution of independent milestones** — **ANSWERED 2026-05-21**: parallel agent dispatch from the main chat is the locked execution model. Each milestone gets a delegated agent in an isolated git worktree. Main chat orchestrates dispatch, plan-reviewer pass, and merge sequence. See the architectural decision above ("Milestone execution model: parallel agent dispatch from the main chat") and the M6→M7 dependency note for the only inter-milestone constraint.

5. **Auto-promotion `.append` steering note** — **ANSWERED 2026-05-21**: doc-only for v0.2.1 (no lint emission for `.append()` → `.add()`). The doc tooltip must follow Golden Rule 11 (WHAT / WHAT-INSTEAD / WHY with a specific-and-contextual WHY tied to the call site). Verified that the v0.2.0 hover-doc field for `.append()` carries the steering text — if it does NOT yet follow Rule 11, that's a discoverable cleanup item separate from v0.2.1.

6. **M5–M9 ordering** — **ANSWERED 2026-05-21**: keep proposed order — M5 (doc-highlight + selection-range, cheap nav polish) → M6 (typeDefinition + implementation, the contract-navigation win) → M7 (type + call hierarchy, depends on M6's `contract_followers_index`) → M8 (diagnostic enrichment + semantic-token modifiers) → M9 (extension polish + release). No reordering or combining.

7. **Snippet list** — **ANSWERED 2026-05-21**: ship the 5 locked snippets (`shape`, `options`, `function`, `for`, `if-is`). Do NOT add a 6th `errors`-function snippet — the `errors` keyword is one extra token typed after the `function` snippet expands. Per the locked M9 architectural decision, all snippet content uses Yinz vocabulary exclusively; the snapshot test asserts no banned-jargon strings appear.

8. **Per-domain inlay hint defaults** — **ANSWERED 2026-05-21**: all 5 currently-firing domains default to `enabled: true`. Monitor user feedback in the v0.2.1 → v0.2.2 window; flip defaults if any domain is confirmed-noisy (current suspicion: `copyPoints` may need to be flipped to `false` once real Yinz codebases exist).

9. **`workspace/configuration` vs `initializationOptions`** — **ANSWERED 2026-05-21**: pull model via `workspace/configuration` + `workspace/didChangeConfiguration`. The user-friendly path — settings changes apply live without a server restart. Slightly more code in `state.rs` (setting-cache + invalidation), worth it.

10. **Code action expansion beyond QUICKFIX** — **ANSWERED 2026-05-21**: leave all 4 deferred refactor code action kinds (`source.organizeImports`, `source.fixAll`, `refactor.extract`, `refactor.inline`) as Deferred. They are tracked in `design/lsp.md` Capability Inventory AND have `[[deferred_tooling_feature]]` registry entries (`lsp-code-action-organize-imports`, `lsp-code-action-fix-all`, `lsp-code-action-refactor-extract`, `lsp-code-action-refactor-inline`). A v0.3+ follow-up plan picks up `organizeImports` as the cheapest early win when the import-sorter pass is ready; extract/inline wait for the refactor catalog (v0.4+).

## Open Questions for Patrick (M10–M11 — Need Resolution Before Dispatch)

11. **Yinz-yellow hex code for inlay-hint color contribution** — **ANSWERED 2026-05-21**: `#ffd23f` (primary accent, "Pittsburgh gold", sourced from `website/app/assets/css/tailwind.css:22` `--color-gold`). The deeper `#fcb514` (Pirates gold) and softer `#fff3b8` (highlight on dark) variants exist in the same palette but are reserved for non-inlay accents. M10 P4 ships `"[ynz]": { "editor.inlayHints.foreground": "#ffd23f" }` in `tooling/vscode-ynz/package.json` `contributes.configurationDefaults`.

12. **M10 ordering — dispatch first, in parallel with M1, or sequential after M1–M9?** — **ANSWERED 2026-05-21**: dispatch M10 FIRST, before M1 (option A). Bug fixes ship to users in days, not weeks. Other milestones follow.

13. **M11 release-coupling — ship inside the v0.2.1 tag or follow-up as v0.2.1.1?** — **ANSWERED 2026-05-21**: ship v0.2.1 with M1–M10, tag the release, then M11 ships as a `v0.2.1.1` content patch (option B). CHANGELOG calls it out as "content fixes, no API changes." Decouples content velocity from release velocity.

14. **M11 P0 doc-entry review model — one PR per primitive type, or one PR for all 96 entries?** — Deferred to the M11 execution plan. The roadmap is intentionally broad; the per-PR-vs-bundle decision belongs in the milestone's own `/plan` pass once it starts.

15. **M11 P3 inlay hint render template format** — Deferred to the M11 execution plan. Same reasoning as Q14: implementation-detail decision that belongs in the milestone's own plan.

16. **Visual polish phase (Pretty TS Errors-style) — dedicated roadmap, or v0.2.2 milestone, or v0.3 polish work?** — **ANSWERED 2026-05-21**: deferred. It's a VSCode extension polish concern, not a v0.2.1 release-blocker. When v0.2.1 ships and we know the content quality is right, draft the visual-polish roadmap as its own thing. Until then, the design inspiration lives in `design/lsp.md` "Visual Polish Concepts" subsection — that's enough to not forget.

## Capability Ledger

(merged from the pre-migration companion `capability-ledger.md` file — 2026-07-01)

Every capability the initiative will deliver maps to a milestone. Milestone 10 shipped (its child plan `v0-2-1-m10-teaching-surface-bugfix` is `done`); the Notes column records what it delivered. Milestones 1–9 and 11 have no child execution plan yet — they are `NEEDS-PLANNED` (run `/plan` when each is picked up), owned by their roadmap-sequenced milestone slug.

| Capability | Owning milestone | Status | Notes |
|---|---|---|---|
| Rename-shadowing rejection — a rename can never produce silent-wrong-output via accidental shadow; rejected with use-site error | v0-2-1-m1-rename-shadowing | NEEDS-PLANNED | No child plan yet. |
| Signature help — parameter list, names, types, active-argument position shown inline inside a call | v0-2-1-m2-signature-help | NEEDS-PLANNED | No child plan yet. |
| Code lens — "N references" inline above every top-level function/shape/options block, click to jump | v0-2-1-m3-code-lens | NEEDS-PLANNED | No child plan yet. |
| Lint Tier 3 infrastructure + 2 starter rules (`array_to_fixed_promotion`, `let_to_const_promotion` as tier-3 lints with WHAT/WHAT-INSTEAD/WHY hover) | v0-2-1-m4-lint-tier3 | NEEDS-PLANNED | No child plan yet. Future lint rules ship via one registry entry + analysis pass. |
| Document highlight + selection range — occurrence highlight, AST-node smart-expand selection, UFCS-site co-highlight | v0-2-1-m5-doc-highlight-selection-range | NEEDS-PLANNED | No child plan yet. |
| Type-definition + implementation provider — jump-to-TYPE, "Go to Implementations" for contract followers | v0-2-1-m6-type-def-implementation | NEEDS-PLANNED | No child plan yet. Missing piece for contract-based design in non-OOP Yinz. |
| Type hierarchy + call hierarchy — tree navigation for extends/follows chains and incoming/outgoing calls | v0-2-1-m7-type-hierarchy-call-hierarchy | NEEDS-PLANNED | No child plan yet. |
| Diagnostic enrichment + semantic-token modifiers — struck-through deprecated features, dimmed unused, cross-file related-info, const/lend/intrinsic token styling | v0-2-1-m8-diagnostic-enrichment-semantic-tokens | NEEDS-PLANNED | No child plan yet. |
| Extension polish + release — completion-item resolve, 5 snippets, 2 commands, per-domain inlay toggles, 0.2.1 release tag | v0-2-1-m9-extension-polish-release | NEEDS-PLANNED | No child plan yet. |
| Teaching-surface correctness — fix every bug from the 2026-05-21 four-agent audit (false-positive unused-import, inlay-line position, click-to-explicit promotion hint, suppressed promotion hints, keyword-name hover, BannedJargon quick-fix, space-triggered completion popup, `booleanean`/`infers` jargon) | v0-2-1-m10-teaching-surface-bugfix | done | Delivered (child `v0-2-1-m10-teaching-surface-bugfix`, status: done — Phase 2 complete). |
| Teaching-content polish — Go-model `//` comment syntax, colored signature hover, field hover on dot-access, 96 intrinsic hover docs, 9 keyword hover entries, per-domain WHY templates, inlay context strings, 15 diagnostic WHY improvements | v0-2-1-m11-teaching-content-polish | NEEDS-PLANNED | No child plan yet. |
