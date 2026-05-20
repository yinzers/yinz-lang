---
slug: v0-2-m1-feature-inventory-sync
type: execution
owner: Patrick Rizzardi
status: done
roadmap: v0-2-dev-loop-tooling
created: 2026-05-19
last_updated: 2026-05-19
files:
  - crates/ynz-registry/**
  - registry/features.toml
  - crates/ynz-diagnostics/src/banned_jargon.rs
  - crates/ynz-typeck/src/intrinsics.rs
  - crates/ynz-typeck/src/check.rs
  - crates/ynz-parser/src/lexer.rs
  - crates/ynz-diagnostics/src/diagnostic.rs
  - design/feature-registry.md
  - design/mvp-scope.md
  - design/compiler-language.md
  - .claude/rules/feature-registry.md
  - .claude/rules/plan-invariants.md
  - .claude/graveyard.md
  - CLAUDE.md
  - examples/errors/v0_2_m1_errors.ynz
---

# Plan: v0.2-M1 — Feature Inventory & Sync Architecture

Created: 2026-05-19
Status: approved (Patrick approved 2026-05-19 — ready for Sonnet to execute Phase 0)

## Context & Why

**Goal**: Build a single source-of-truth (SSOT) registry crate that every existing scattered feature inventory in the Yinz compiler reads from. After this milestone, adding a new language feature, banned-jargon word, primitive method, type-attached constant, reserved deferred feature, diagnostic template, or muted-hint domain happens in ONE file (`registry/features.toml`), and every consumer (lexer, typeck, diagnostics, future LSP, future docs generator) re-derives automatically.

**Why now**: v0.1.0 shipped with feature inventories scattered across at least 6 distinct locations — `banned_jargon.rs`, `intrinsics.rs`, `check.rs` (type-attached constants), `lexer.rs` (keyword list + deferred-feature handlers), `diagnostic.rs` (template scaffolding), and a design-doc-only muted-hint catalog in `.claude/rules/inference.md`. Adding `int.max` in M4 P5 already touched five places that nothing forces to stay in sync. The roadmap (`v0-2-dev-loop-tooling`) is the foundation for v0.2's LSP, fmt, and watch — all three need to read these inventories, and the LSP needs them at IDE-keystroke latency. Drift between "what the compiler knows" and "what the IDE shows" is the failure mode Patrick called out explicitly. Closing the drift class permanently requires the SSOT BEFORE the LSP exists, not after.

**Background**:
- v0.1.0 shipped 830 tests across M1–M8. Compiler is structured around `salsa` queries so LSP work in v0.2-M2 will wrap existing queries over JSON-RPC instead of rewriting compute.
- 9 crates in the workspace (~44k lines): ynz-driver, ynz-diagnostics, ynz-ast, ynz-parser, ynz-typeck, ynz-codegen, ynz-numerics, ynz-runtime, plus the new ynz-registry this milestone adds.
- Existing convention: `*Table` struct + builder pattern (ShapeTable, ExportTable, SignatureTable, GenericFnTable, MonomorphizationTable, OptionsTable, PrimitiveIntrinsicTable, PropagationTable). The registry uses the same conventions for consumer-side adapters.

**Constraints** (locked, from roadmap + this conversation):
- **Implementation shape**: data file (TOML) at `registry/features.toml`, parsed by a `build.rs` in `crates/ynz-registry/` that generates Rust code in `OUT_DIR`. Consumers `include!()` the generated file or import via the `ynz-registry` crate's public API.
  - Rejected: proc-macro (overkill at Yinz's scale, weakest IDE story, hardest to consume from non-Rust tooling — and Yinz will be self-hosted in Yinz at v2+, after which a Rust proc-macro is dead code).
  - Rejected: pure-Rust `const` table (best IDE story today, but locks the registry inside one Rust workspace; when Yinz self-hosts in Yinz, every consumer needs porting AND the source-of-truth array needs porting in lockstep).
  - Chosen B (TOML data file): the source-of-truth survives the Rust→Yinz self-hosting transition unchanged; only `build.rs` becomes `build.ynz`. TypeScript compiler (`diagnosticMessages.json`) and Roslyn (`Syntax.xml`) chose this for the same reason — one source feeds compiler + IDE + docs.
- **Stdlib scope**: schema accommodates a `[[stdlib_api]]` entry-kind (so v0.5+ doesn't retrofit), but M1 populates ZERO stdlib entries. Each v0.5+ milestone populates its own.
- **Error text drift**: the registry rewrite is allowed to improve / make-more-consistent the text of deferred-feature errors (sized ints/floats, `test` keyword, future entries from `design/future/*.md`). Insta snapshots get updated as part of each migration phase. Diffs are reviewed at PR time. The point is uniformity via the registry — that's a feature, not a regression.
- **No new language features in v0.2** — tooling only. Auto-promotion analyses, lint rules, stdlib all stay deferred.
- **All compile errors continue to follow WHAT/WHAT-INSTEAD/WHY** per `design/teaching-mission.md` — registry-driven errors render through the same `Diagnostic` constructor.
- **No GC, no per-instance method storage, none of v0.1's locked properties weaken** — the compiler binary's behavior on a `.ynz` file is byte-identical for valid programs; error text may improve for the migrated deferred features (per the locked decision above).

**Success criteria**:
- Every existing scattered registry has migrated to `registry/features.toml`. Auditing "what feature surfaces does crate X consume?" has a single grep-able answer per crate.
- Adding a new keyword, banned-jargon word, type-attached constant, or deferred-feature entry requires editing ONE file.
- Consistency tests in `ynz-registry` fail CI when entries are malformed (banned-jargon without replacement, deferred-feature without WHY/SUBSTITUTE/TRIGGER, etc.).
- A Bouncer graveyard entry catches PRs that add a hardcoded list-of-features in a Rust file that should have gone through the registry.
- A new project rule (`.claude/rules/feature-registry.md`) + plan-invariants subsection (`### Feature Registry Entries`) forces every v0.2-M2-onward execution plan to list its registry additions.
- All 830+ existing tests pass. Error text changes are gated by reviewed insta snapshots.
- Tag cut: `v0.2.0-m1` (intermediate; v0.2.0 final ships at v0.2-M5).

## Research Findings

**Current scattered registries** (Explore agent scan, 2026-05-19):

1. **`crates/ynz-diagnostics/src/banned_jargon.rs` (87 lines)** — 30 banned words + 8 acronyms (`type`, `struct`, `enum`, `void`, `null`, `infer`, `inference`, `monad`, `lift`, `Result`, `Option`, `try`, `catch`, `throw`, `UTF-16`, …). Each entry: word + replacement + reason.

2. **`crates/ynz-typeck/src/intrinsics.rs` (217 lines)** — `PrimitiveIntrinsicTable` with four sub-tables: `print_types` (vec of types accepted by `print()`), `free_fns` (e.g. `range(end)`, `range(start, end)`), `methods` (zero-arg: `.toNumber()`, `.toFloat()`, `.toString()`), `methods_1arg` (one-arg on `int`: `.wrappingAdd()`, `.saturatingAdd()`, …).

3. **Type-attached constants — `crates/ynz-typeck/src/check.rs:3698-3707`** — `type_attached_const_type(type_name, const_name) -> Option<Type>` for `int.max`, `int.min`, `float.max`, `float.min`, `float.epsilon`, `number.max`, `number.min`, `number.epsilon`. Used at check.rs:1036. Codegen lowers the same lookup at emit time.

4. **Reserved-but-deferred handlers — `crates/ynz-parser/src/lexer.rs:574-690`** — sized numeric types (`f32`, `f64`, `i8`–`i64`, `u8`–`u64`) redirect to `int`/`float`/`number` (lines 676-690). `test` keyword reserved for v0.12 (lines 574-586). No `gpu`/`foreign`/`number<5000>` handlers currently exist — the sized-numeric pattern is the deferred-feature archetype to follow when registry-driving the rest of `design/future/*.md`.

5. **Keyword table — `crates/ynz-parser/src/lexer.rs:491-620`** — hardcoded match statement in `lex_identifier_or_keyword()`. Tracks ~50 keywords organized by milestone, plus ~20 banned declaration keywords (`type`→`shape`, `struct`→`shape`, `enum`→`options`, etc.). Per the roadmap, the parser stays as-is, but the LIST of valid keywords becomes registry-driven so error suggestions can use it.

6. **Diagnostic templates — `crates/ynz-diagnostics/src/diagnostic.rs` (203 lines)** — three-part WHAT/WHAT-INSTEAD/WHY format enforced by constructor (panics on empty fields per Golden Rule 11). Plus `DiagnosticKind` enum (TypeMismatch, MutationOfConst, NotDefined, HiddenAccess, Consumed, Borrowed, …) and `Severity` (Error/Warning/Suggestion). The CANONICAL templates that get reused (e.g. "cannot mutate const") are candidates for registry entries; dynamic-message construction stays in code.

7. **Muted-hint domains — design-doc-only** in `.claude/rules/inference.md`. NO runtime registry yet. Domain catalog: type inference, function param type inference, ownership at call sites (Informational), wait points, lifetimes, allocators, copy points, `array<T>`→`fixed<T>` promotion, `let`→`const` promotion. Each domain tagged Addition / Replacement / Informational. The registry POPULATES these entries in M1 (no consumer yet); v0.2-M2 LSP wires the consumer.

8. **Existing `*Table` convention** — all current registries follow `pub struct FooTable { ... } impl FooTable { ... }` with a builder pattern. The post-migration consumer-side adapters use the same convention.

**SSOT implementation comparison** (general-purpose agent + WebSearch, 2026-05-19): see Constraints above for the locked decision. Key reference: TypeScript's `src/compiler/diagnosticMessages.json` + `generate-diagnostics` build step is the closest production analogue for what M1 is building. Roslyn's `Syntax.xml` is a related precedent.

**Branching/PR sizing** (per `~/.claude/memory/branching.md`): each phase = one branch off main, one PR. Soft max ~500 lines/PR; mechanical refactors averaging <5 lines/file across 50+ files get a softer review bar (relevant — most migration phases here are mechanical). Yinz's recent history (M4 / M5 / M6 / M7) has occasional bundled-phase PRs when the bundle ships one coherent capability — that exception is available here too.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| TOML schema design ossifies wrong; v0.2-M2/M3/M4/M5 reshape it under load and the "one file to edit" promise breaks | Medium | High | Phase 1 prototypes the schema against 3 consumers (keyword list for lexer, intrinsic methods for typeck, banned-jargon for diagnostics) AND a sketch LSP autocomplete consumer (just the data-access pattern, no LSP code) BEFORE locking. Phase 1's acceptance criterion includes "schema reviewed against 4-consumer sketches; no consumer required a schema rewrite during the sketch." |
| Generated code in `OUT_DIR` breaks IDE navigation; contributors lose "click registry entry, see consumer" workflow | High | Low | Acknowledged Option-B tradeoff per Constraints. Mitigation: `design/feature-registry.md` documents the cognitive model ("source: TOML; generated: `cargo expand` or look at `target/debug/build/ynz-registry-*/out/registry.rs`"). Also: consumer-side adapter modules (`pub fn keywords() -> &'static [&'static str]`) are hand-written Rust in `crates/ynz-registry/src/`, so `find references` on a consumer name DOES work — it lands on the adapter, then one hop to the TOML. |
| `build.rs` adds noticeable build time | Low | Medium | `cargo:rerun-if-changed=registry/features.toml` declared in build.rs; `toml` parsing only runs when the file changes. Phase 1 acceptance: cold build time within ±10% of pre-migration baseline (measured: `cargo clean && time cargo build --workspace`). Incremental builds with no TOML change must show zero overhead from the registry. |
| Migration phase breaks an existing test snapshot in a way that masks a real regression | Medium | High | Insta-snapshot updates require manual review per CLAUDE.md immutable-test rule (`immutable-test-check.sh` PreToolUse hook). Each migration PR description enumerates which snapshots were intentionally updated and why. Final Verification phase re-runs the full 830+ test suite and the M4–M8 fixture programs end-to-end (`./target/debug/ynz run`). |
| Bouncer "scattered registry" grep pattern overfits and blocks legitimate parallel registries (e.g., a perf-critical inner-loop lookup that genuinely shouldn't go through SSOT) | Low | Low | `design/feature-registry.md` includes an explicit "Carve-Outs" section: cases where a separate hand-curated table is allowed, with the rule that the carve-out MUST link back to the SSOT entry (bidirectional comment annotations). Phase 7's Bouncer pattern is tuned conservatively (matches new files that match specific anti-patterns like `static KEYWORDS: &[&str] = &[...]` near lexer/typeck/diagnostics code), and the pattern's regex is committed to graveyard.md so future PRs can append to it without re-introducing the anti-pattern definition. |
| Deferred-feature TOML schema captures wrong fields; future deferred features land with incomplete metadata | Low | Medium | Schema validation in `build.rs` panics on unknown fields and missing required fields (WHY/SUBSTITUTE/TRIGGER for every `[[deferred_feature]]` entry). Phase 1's schema includes all three as required-non-optional. A snapshot test in `ynz-registry/tests/` lists every deferred-feature entry and asserts the three fields are populated. |
| Renaming a TOML field silently produces a different generated identifier; consumers compile against stale names | Low | Medium | Generated code uses a `#[non_exhaustive]` enum + typed field accessors. Renaming a TOML field name (`why` → `reason`) breaks compilation of every consumer at next build, with the actual error pointing to the consumer's `entry.why` access. The schema validator in build.rs ALSO panics on unknown fields, so a TOML typo (`whi` instead of `why`) is caught at build time before code generation. |
| Drift between deferred-feature TOML catalog and `design/future/*.md` files | Medium | Medium | Phase 7 consistency test walks `design/future/*.md` filenames and asserts each one has a corresponding `[[deferred_feature]]` entry with `design_doc = "design/future/<name>.md"` field. Test fails CI if a new `design/future/foo.md` lands without a corresponding entry, AND if a registry entry references a non-existent design doc. Bidirectional. |
| Scope inflation: Phase 5 (deferred-feature population from `design/future/*.md` + all sized-int/float reservations + gpu/foreign/test) becomes a 1000+ line PR | High | Medium | Phase 5 is split into 5a (migrate EXISTING scattered handlers — sized ints/floats, `test`) and 5b (POPULATE the full deferred catalog from `design/future/*.md` — 13 future-doc entries currently exist: arena, auto-soa, concurrency, http-framework, inline-shape-types, no-runtime-mode, packages, panic-safety, release-mode, self-references, string-ptr-len-overhaul, supervisor, plus any added before M1 ships). 5b is mostly TOML data entry (mechanical-refactor exception applies). |
| Plan-invariants rule update breaks an in-flight plan because the rule pre-dates the plan | Low | Low | Rule update applies to plans created AFTER v0.2-M1 ships. Currently in-flight plans (`v0-2-m1-feature-inventory-sync` itself is exempt because it's the one building the rule; `m8-typeck-cross-file-resolution` is technically still listed in the radar but per state.md M8 has shipped — investigate during Phase 0 doc-lockdown and either close out or grandfather). |
| Self-hosting transition (v2+) requires re-implementing build.rs in Yinz; nothing today proves the Yinz stdlib can do what `toml` crate does | Low (timing) | Low | TOML parsing is in Yinz stdlib v0.22+ (per `design/packages.md` — `yinz.toml` parsing already required). No M1 work needed; documented in `design/feature-registry.md` "Self-hosting migration plan" subsection as a recognized future task. |
| `m7-*.md` and `m8-*.md` files migrating back from `done/` to `active/` after git-mv (flagged in roadmap Round-1) is a hidden process bug that could also resurrect this plan after it's done | Low | Low | Side-investigation per roadmap, not gating M1. Phase 0 includes "verify state of m7/m8 plan files" as a quick check — if they're still in `active/`, file an investigation note in `todos.md`. |

## Questions

None outstanding. Three answered this session:
1. Registry shape → **B (TOML data file + build.rs)** — answered "b it is"
2. Stdlib scope → **Schema only, no stdlib entries**
3. Error text drift → **Allow improvements; insta snapshots catch them**

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler-only |
| Touches auth/permissions | No | Compiler-only |
| Raw SQL / literals | No | No DB |
| Modifies existing data | Yes | Refactors 6 existing registries (banned_jargon, intrinsics, type-attached constants, lexer keyword table, lexer deferred handlers, diagnostic templates). Tests gate behavior. |
| Third-party integration | No | Pure refactor + new `toml` build-time dependency |
| Changes existing endpoints | N/A | Not a service |
| New feature with no equivalent | No | Existing registries already exist; this consolidates them |

**Mitigations applied**:
- 830+ existing tests gate every migration phase (HIGH → MEDIUM)
- Each phase is a single migration with isolated blast radius (MEDIUM → effectively LOW per phase, MEDIUM cumulative)
- Backward compatible: consumer-side adapter APIs (`crate::registry::keywords()`) match existing `*Table` access patterns so call sites don't need rewriting beyond import paths

**Rollout plan** (Yinz convention: trunk-based, no production rollout; "rollout" = milestone tag):
1. Each phase: PR via `/pr`, code-reviewer agent run at phase boundary, merge to main when PASS
2. Phase 8 (final verification + tag): cut `v0.2.0-m1` tag after full test sweep + fixture run + `cargo publish` dry-run (not actually publishing — Yinz isn't on crates.io yet)
3. v0.2.0 final tag waits for v0.2-M5 per roadmap; M1's tag is an intermediate marker matching the v0.1.0-mN convention

## Invariants This Milestone Must Preserve

### Safety
- All 830+ existing tests pass post-migration (`cargo test --workspace`)
- No previously-valid program becomes rejected by the compiler (insta snapshots gate)
- No previously-rejected program becomes accepted (insta snapshots gate)
- Error severity (Error/Warning/Suggestion) is preserved for every migrated diagnostic
- Type-attached constant values are byte-identical (`int.max = 9223372036854775807`, `float.epsilon = 2.220446049250313e-16`, `number.epsilon = 1e-33`, etc.) — verified by Phase 4 unit tests that compare emitted IR to pre-migration snapshots

### Performance
- Cold build time (`cargo clean && cargo build --workspace`) within ±10% of pre-migration baseline. Measured before Phase 1 and again at Phase 8.
- Incremental builds with no `registry/features.toml` change show ZERO overhead from the registry (`build.rs` declares `cargo:rerun-if-changed=registry/features.toml` and nothing else)
- Generated code is `const` arrays / `match` statements — no runtime allocation introduced; registry lookups are O(1) hash or O(N) linear scan over small constants (same as current `match` statements)
- LLVM-emitted code for `int.max` / `number.epsilon` / etc. is byte-identical to pre-migration codegen (verified by IR snapshot tests)

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`):
- This milestone does NOT introduce any new language feature, stdlib type, or codegen optimization. There is no stricter/faster form the compiler could prove fits.
- The registry stores METADATA, not optimizer hints. No codegen auto-promotion. No muted-hint surface. No Tier 3 lint suggestion.
- Explicitly considered, not forgotten.

### Teaching
- Improved consistency of deferred-feature error text via registry-driven WHY/SUBSTITUTE/TRIGGER. Per the locked decision, this is allowed; insta snapshots gate the diffs.
- All migrated errors continue to follow WHAT/WHAT-INSTEAD/WHY (enforced by `Diagnostic` constructor — panics on empty field)
- No new banned-jargon words slip into user-facing diagnostics (audited by existing `tests/jargon_audit.rs` — re-run in Phase 8)
- Three NEW teaching surfaces shipped:
  - `design/feature-registry.md` — design rationale + schema reference + Carve-Outs section + self-hosting migration plan + "when to add an entry" + "when an entry is the wrong fit"
  - `.claude/rules/feature-registry.md` — project rule loaded on demand for any milestone touching the registry or adding a language feature
  - Updated `.claude/rules/plan-invariants.md` `### Feature Registry Entries` subsection — mandatory for every plan from v0.2-M2 onward (this plan itself is exempt; it's the one creating the rule)
- The IDE muted-hint domain table is POPULATED in the registry (one entry per domain from `.claude/rules/inference.md`) but has NO consumer yet — v0.2-M2 LSP wires the consumer. Populating now means M2 doesn't have to also populate them.

### Runtime Dependencies
- `ynz-registry` crate at runtime: NONE. Generated Rust code is `const` arrays + `match` arms with `&'static str` references — no allocation, no I/O.
- `toml` crate dependency: BUILD-TIME ONLY (in `crates/ynz-registry/Cargo.toml` `[build-dependencies]`). Not a runtime dependency of the compiler binary.
- `cargo` (build orchestration): same as before — no new global tooling required.
- The compiler binary's runtime dependency profile is byte-identical to pre-migration: `inkwell` (LLVM bindings), `salsa`, `unicase`, `unicode-normalization`, `simdutf8` — no additions, no removals.

### Kernel-Mode Behavior
- `--kernel` build mode: registry constants are baked into the compiler binary at compile time (via build.rs codegen into `OUT_DIR`); available in all modes including `--kernel`. No heap allocation. No runtime parsing.
- Compiler binary's behavior on a `.ynz` file in `--kernel` mode is byte-identical to pre-migration (same error text for the same input, except for the deferred-feature error improvements gated by insta snapshots).
- No new compile-error path introduced for kernel-mode programs that the existing scattered registries didn't already produce.

### Demo & Error Gallery
- `examples/basics/entrypoint.ynz`: NO extension required. This milestone is a refactor with no new user-facing language feature. Per `.claude/rules/plan-invariants.md` "Demo & Error Gallery": features that ship without user-facing surface don't need entrypoint extensions. State this explicitly in the Phase 8 PR description.
- `examples/errors/v0_2_m1_errors.ynz`: NEW file. Intentional triggers for every registry-driven error path:
  - All sized-integer reservations (`f32`, `f64`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`)
  - The `test` keyword
  - At least one banned-jargon trigger per migrated category (`type`, `class`, `void`, `null`, `infer`, `monad`, `try`)
  - Type-attached constants on the wrong type (e.g., `string.max` — should error)
  - At least one trigger from a `design/future/*.md`-sourced entry if any get user-facing error paths in M1 (likely just `gpu` / `foreign` if those were marked compile-error-producing in the registry; otherwise N/A and stated in PR description)
- Each trigger has a `// WHY:` comment naming the diagnostic class
- `insta` stdout/stderr snapshots in Phase 8

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing the actions to execute (persist plan state → invoke code-reviewer → handle verdict → prompt commit). Those instructions are commands, not a checklist to tick off.

**Final phase (Phase 8) additionally:**
- Verify ALL phases' acceptance-criteria and quality-gate checkboxes are accurate across the plan
- Invoke `code-reviewer` with the **cumulative plan diff**: `git diff <plan-base-commit>..HEAD` (Step 10f of `/plan` skill)
- Flip `status: active` → `status: done` only after final PASS; the radar moves the file to `plans/done/` on next rebuild

## Phases

**Project Shipping Conventions** (per `/plan` Step 4a, detected from project):
- Per-phase ships via `/pr` (project has local `pr` skill)
- Per-milestone ships via `/release` (project has local `release` skill)

---

### Phase 0: Doc Lockdown + Schema Design Doc
**PR scope**: All design + rules + project-skill + graveyard docs that lock the registry decisions BEFORE any code lands. Locks the schema shape, the project rule, the plan-invariants subsection, and the Bouncer pattern.
**Branch**: `chore/v0-2-m1-doc-lockdown`
**Flag**: N/A (docs only)
**Est. lines**: ~600 (all docs, no code)
**Ships via**: `/pr`
**Objective**: Lock every decision in writing so Phases 1-8 are mechanical. After this phase merges, no future phase needs to re-argue the schema, the carve-outs, or the rule.
**Why this phase exists**: Lessons from M5 P0 / M6 P0 / M7 P0: lockdown-first PRs prevent mid-implementation revisitation. The roadmap explicitly calls out scattered registries as the v0.2-M1 problem — agreeing the FIX in writing before coding ensures Phases 2-7 don't accidentally rewrite each other's schema assumptions.

**Current-state anchors**:
- `design/mvp-scope.md:*` — current v2+ deferred-features prose (will get back-pointer to registry entries when those land in Phase 5b)
- `design/compiler-language.md:*` — current "Why Salsa" section (will get an "SSOT registry" paragraph)
- `.claude/rules/plan-invariants.md:*` — current 6-subsection block (will become 7-subsection for plans from v0.2-M2 onward)
- `CLAUDE.md:*` — current Golden Rules + project layout (gets registry pointer)
- `.claude/graveyard.md:*` — current corpses (gets new entry)

**Files (expected scope)**:
- NEW `design/feature-registry.md` (~250 lines: schema reference, consumer API contract, Carve-Outs section, self-hosting migration plan, "when to add" / "when not to add")
- NEW `.claude/rules/feature-registry.md` (~80 lines: project rule loaded on demand)
- UPDATE `.claude/rules/plan-invariants.md` (~+40 lines: new `### Feature Registry Entries` 7th subsection, marked "applies to plans from v0.2-M2 onward")
- UPDATE `CLAUDE.md` (~+15 lines: new rule pointer in Project Layout + Rules Files tables)
- UPDATE `.claude/graveyard.md` (~+50 lines: new entry "scattered registry without SSOT link" with grep pattern)
- UPDATE `design/mvp-scope.md` (~+20 lines: each "Locked design, deferred from v0.1" entry gets a placeholder line "Registry entry: TBD M1" — the actual entry-name lands in Phase 5b)
- UPDATE `design/compiler-language.md` (~+15 lines: new paragraph "Both also share the SSOT registry" in the "How the LSP shares the compiler" section)
- VERIFY `.claude/plans/active/m7-*.md` / `m8-*.md` — if still present per roadmap Round-1 flag, log to `todos.md` as a side investigation (not a Phase 0 deliverable to FIX, just to AUDIT)

**Deviation rule**: Standard.

**Steps**:
1. Read every scattered-registry file listed in Research Findings to ground the schema design in actual current shape (banned_jargon.rs, intrinsics.rs, check.rs:3698, lexer.rs:491-690, diagnostic.rs).
2. Read every `design/future/*.md` file to inventory deferred features (13 files currently — listed in Research Findings).
3. Draft `design/feature-registry.md` schema reference. Required entry-kinds: `keyword`, `banned_jargon`, `primitive_intrinsic`, `type_attached_constant`, `deferred_language_feature`, `deferred_tooling_feature`, `deferred_stdlib_api` (RESERVED, unpopulated), `diagnostic_template`, `muted_hint_domain`. Each entry-kind documented with required fields and example TOML.
4. Draft Carve-Outs section. List concrete cases where parallel registries are allowed (perf-critical inner loops; example: hot-path keyword classification might want a hand-tuned `&'static [u8]` lookup table in addition to the registry — must bidirectionally cite the registry entry).
5. Draft self-hosting migration plan paragraph (TOML → `yinz.toml.parse(...)` once stdlib lands; build.rs → build.ynz with same semantics).
6. Draft `.claude/rules/feature-registry.md`: when to load this rule (any milestone touching the registry OR adding a language feature OR renaming an internal method), required-entry-types checklist.
7. Draft `.claude/rules/plan-invariants.md` `### Feature Registry Entries` subsection. Required content: enumerate every registry entry the milestone adds; for renames/changes, enumerate every registry entry the milestone modifies. Marked "applies to plans from v0.2-M2 onward" (this plan is exempt).
8. Draft `.claude/graveyard.md` entry. Pattern name: "scattered-registry-without-SSOT". Grep pattern: detects new `pub static FOO: &[&str] = &[...]` (or `const`, or `[(name, replacement)]`-style tuple arrays) within `crates/ynz-{diagnostics,typeck,parser}/src/` that doesn't have an explicit `// CARVE-OUT: SSOT registry/features.toml#<entry>` comment within 3 lines above. Test the pattern locally against the current code (should match the things we're about to migrate; should NOT match other arrays in those crates).
9. Update `CLAUDE.md` Project Layout + Rules Files table to point at the new rule + design doc.
10. Update `design/mvp-scope.md` with TBD placeholder lines for v2+ deferred entries (each gets `Registry entry: TBD M1 (P5b)`).
11. Update `design/compiler-language.md` with the SSOT registry paragraph.
12. Audit `.claude/plans/active/m7-*.md` / `m8-*.md` per roadmap Round-1 flag; log to `todos.md` if action needed.

**Acceptance criteria**:
- [x] `design/feature-registry.md` exists with every required section (schema reference / consumer API / Carve-Outs / self-hosting migration / when-to-add / when-not-to-add)
- [x] `.claude/rules/feature-registry.md` exists with load-trigger + required-entry-types checklist
- [x] `.claude/rules/plan-invariants.md` has `### Feature Registry Entries` subsection clearly marked "applies to plans from v0.2-M2 onward"
- [x] `.claude/graveyard.md` has the scattered-registry entry with a tested grep pattern (the grep pattern matches every current scattered registry that's about to migrate, and zero false positives elsewhere in the crates)
- [x] `CLAUDE.md` Project Layout + Rules Files tables reference the new doc + rule
- [x] `design/mvp-scope.md` has TBD placeholder for each v2+ deferred entry
- [x] `design/compiler-language.md` has the SSOT registry paragraph
- [x] m7/m8 plan-file audit logged to `todos.md` (action or no-action)

**Quality gate**:
- [x] Schema example TOML in `design/feature-registry.md` is parseable by the `toml` crate — verified via `python3 tomllib` during Phase 1 code-review; all 8 entry-kind example blocks parsed cleanly
- [x] All 13 `design/future/*.md` files are listed in the schema's "deferred-feature catalog" placeholder table
- [x] Grep pattern in graveyard entry tested against the current codebase (catches banned_jargon.rs + builtins.rs STRING_METHODS; zero false positives in other crates)
- [x] No banned-jargon words introduced in the new docs (three `infer` hits in design/feature-registry.md are engineer-audience prose — dual-audience exemption per `.claude/rules/inference.md`)

**Verification**:
- `cd /workspaces/ynz && cargo build --workspace` (sanity — should be unchanged, no code touched)
- Manual read-through of `design/feature-registry.md` from the perspective of "I'm a new contributor adding a new keyword — does this doc tell me what file to edit and what fields are required?"

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 0",
     prompt: "Review the diff for Phase 0 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff main..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 0 done. Code-reviewer: PASS. Ready to commit and move to Phase 1?"

5. **Do NOT start Phase 1** until the user confirms the commit.

**STATUS (2026-05-19)**: Phase 0 COMPLETE.
- Commit: `bba8830` on branch `chore/v0-2-m1-doc-lockdown` (pushed to origin)
- PR: branch pushed; `gh auth login` needed to create via CLI. URL: `https://github.com/yinzers/yinz-lang/pull/new/chore/v0-2-m1-doc-lockdown`
- Code-reviewer: PASS (after fixing 3 missing mvp-scope placeholders — ML stdlib, Markets stdlib, Self-hosted compiler)
- Discovery: `builtins.rs:101 STRING_METHODS` is an 8th scattered registry; added to Bouncer pattern + Phase 3 scope
- Waiting on: user to authenticate `gh` and merge the PR, then start Phase 1

---

### Phase 1: Bootstrap `ynz-registry` crate + Schema Validation
**PR scope**: New `crates/ynz-registry/` crate with `build.rs`, schema validation, generated-code skeleton, and 3-4 SKETCH consumers proving the schema works. Registry is EMPTY (no migrations yet). Schema is LOCKED for the migration phases that follow.
**Branch**: `feat/v0-2-m1-registry-crate`
**Flag**: N/A
**Est. lines**: ~500
**Ships via**: `/pr`
**Objective**: Build the foundation that Phases 2-7 plug into. After this phase, `cargo build` consumes `registry/features.toml` and generates `OUT_DIR/registry.rs`; the `ynz-registry` crate exposes typed accessors; sketch consumers in tests prove the schema is workable.
**Why this phase exists**: Per the high-risk schema-ossification mitigation: PROVE the schema works against 3-4 real consumers before migrating 6 production registries to it. If a consumer is awkward, revise the schema NOW, not in Phase 4.

**Current-state anchors**:
- `Cargo.toml` (workspace) — current `members = [...]` list
- `crates/ynz-typeck/src/intrinsics.rs:20-144` — `PrimitiveIntrinsicTable` builder pattern (reference convention for the registry adapter API)

**Files (expected scope)**:
- NEW `crates/ynz-registry/Cargo.toml` (build-dep on `toml`, runtime-dep on nothing)
- NEW `crates/ynz-registry/build.rs` (~100 lines: parse TOML, validate against schema, generate Rust code into `OUT_DIR/registry.rs`, declare `cargo:rerun-if-changed=registry/features.toml`)
- NEW `crates/ynz-registry/src/lib.rs` (~50 lines: `include!(concat!(env!("OUT_DIR"), "/registry.rs"));` + typed accessor module structure)
- NEW `crates/ynz-registry/src/schema.rs` (~100 lines: `FeatureEntry`, `FeatureKind` enum, field accessor types — these are the types that `build.rs` emits and consumers import)
- NEW `crates/ynz-registry/tests/schema_smoke.rs` (~80 lines: sketch consumers — keyword-list iter, banned-jargon lookup-by-name, primitive-intrinsic dispatch-by-name, type-attached-constant resolve, sketch "LSP autocomplete entries for keyword kind")
- NEW `registry/features.toml` (~30 lines: empty entry per kind as schema-validation placeholder — `[[keyword]] name = "PLACEHOLDER_FOR_PHASE_4"` style, deleted in migration phases as they're filled in)
- UPDATE root `Cargo.toml` — add `crates/ynz-registry` to workspace members
- UPDATE `Cargo.lock` — auto-update for new dep

**Deviation rule**: Standard. Specifically: if a sketch consumer in `schema_smoke.rs` reveals the schema can't express its query, REVISE THE SCHEMA in this phase (it's the whole point of this phase). Document the revision in the PR description with the consumer that required it.

**Steps**:
1. Add new crate to workspace.
2. Implement `crates/ynz-registry/Cargo.toml` with `[build-dependencies] toml = "..."` (look up current stable version via Context7 / crates.io). Runtime deps: none.
3. Implement `build.rs`:
   - Read `OUT_DIR` and `CARGO_MANIFEST_DIR` env vars
   - Parse `../../registry/features.toml` (relative to crate root)
   - Validate each entry against the schema from `design/feature-registry.md` (panic with clear error if a required field is missing — message identifies the file, the entry kind, the entry name, and the missing field)
   - Emit Rust code into `OUT_DIR/registry.rs`: `pub static KEYWORDS: &[KeywordEntry] = &[...];`, `pub static BANNED_JARGON: &[BannedJargonEntry] = &[...];`, etc.
   - Declare `cargo:rerun-if-changed=../../registry/features.toml` so incremental builds skip re-running build.rs when TOML hasn't changed
4. Implement `src/schema.rs` — define the typed structs that match what `build.rs` emits. (These are the consumer-facing types.) Use `#[non_exhaustive]` where appropriate to allow future field additions without breaking semver later.
5. Implement `src/lib.rs` — `include!()` the generated file, re-export schema types, provide adapter functions matching the existing `*Table` convention (`pub fn keywords() -> impl Iterator<Item = &KeywordEntry>`, etc.).
6. Implement `tests/schema_smoke.rs` with 5 sketch consumers:
   - **Lexer keyword sketch**: given a string `"function"`, return `Some(KeywordEntry { ... })` or `None`
   - **Diagnostics banned-jargon sketch**: given `"type"`, return `Some(BannedJargonEntry { replacement: "shape", reason: "OOP drift" })`
   - **Typeck primitive-intrinsic sketch**: given `(receiver_type: "int", method_name: "wrappingAdd", arity: 1)`, return `Some(PrimitiveIntrinsicEntry { ... })`
   - **LSP autocomplete sketch**: iterate all keywords + intrinsics and produce a flat `&str` list (proves we can do an O(N) sweep for fuzzy match purposes)
   - **Diagnostic-template render sketch**: given a `DiagnosticTemplateEntry` and a `HashMap<&str, String>` of placeholders, return rendered WHAT / WHAT-INSTEAD / WHY strings. Proves the `{placeholder}` substitution contract Phase 6 will populate against. Pin the placeholder grammar here (e.g., `{name}` substitutes; `{{` and `}}` are literal braces; unknown placeholder = panic with clear message naming the template entry and the unknown key).
7. Populate `registry/features.toml` with ONE placeholder entry per kind (validates schema, gives sketch consumers something to find).
8. Run `cargo test -p ynz-registry`; iterate on schema if a sketch consumer is awkward.
9. Measure cold build time: `cargo clean && time cargo build --workspace`. Compare to baseline captured in Phase 0 acceptance.

**Acceptance criteria**:
- [x] `cargo build --workspace` succeeds with the new crate
- [x] `cargo test -p ynz-registry` passes — 17 tests across 5 sketch consumer categories all pass
- [x] `build.rs` validation panics with a clear message when a required field is missing — confirmed: "registry/features.toml: [[keyword]] entry 'PLACEHOLDER_KEYWORD_PHASE_4': missing required field 'token'"
- [x] `build.rs` validation panics with a clear "duplicate entry name within kind" message — confirmed: "registry/features.toml: [[keyword]] has duplicate entry name '...' — each name must be unique within its kind"
- [x] `build.rs` accepts TOML with UTF-8 BOM (stripped in build.rs) and CRLF line endings (handled by toml crate) — confirmed by bom_crlf_build_succeeded test
- [x] `build.rs` declares `cargo:rerun-if-changed` correctly — touching `ynz-typeck/src/lib.rs` does NOT re-run build script; touching `registry/features.toml` DOES re-run
- [x] Cold build time 13.76s — no prior Phase 0 baseline since no code was added then; this is the M1 baseline for Phase 2+ comparisons
- [x] Incremental build with no TOML change shows zero `build-script-build` re-run for `ynz-registry`
- [x] Schema locked — no sketch consumer required a revision
- [x] No banned-jargon words in new source files

**Quality gate**:
- [x] Schema in `design/feature-registry.md` matches what `build.rs` accepts (re-read both; all 9 entry kinds match)
- [x] `Cargo.lock` committed with new `toml` dep
- [x] No `unwrap()` in build.rs error paths — every panic uses `unwrap_or_else` with contextual message
- [x] No `// TODO` or `// FIXME` in any new file
- [x] Generated code in `OUT_DIR` is valid Rust — verified by `cargo check` + `cargo build` succeeding

**Verification**:
- `cd /workspaces/ynz && cargo build --workspace 2>&1 | tee /tmp/v02m1-p1-build.log`
- `cd /workspaces/ynz && cargo test -p ynz-registry`
- `cd /workspaces/ynz && cargo clean && time cargo build --workspace` (compare to Phase 0 baseline)
- Touch `crates/ynz-typeck/src/lib.rs` and rebuild — confirm `ynz-registry` build script does NOT re-run
- Touch `registry/features.toml` and rebuild — confirm `ynz-registry` build script DOES re-run

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 1",
     prompt: "Review the diff for Phase 1 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 0 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 1 done. Code-reviewer: PASS. Ready to commit and move to Phase 2?"

5. **Do NOT start Phase 2** until the user confirms the commit.

---

### Phase 2: Migrate banned_jargon
**PR scope**: Move all 30 banned-jargon entries + 8 acronyms from `crates/ynz-diagnostics/src/banned_jargon.rs` to `registry/features.toml`. The `.rs` file becomes a thin adapter that reads from the registry crate.
**Branch**: `feat/v0-2-m1-migrate-banned-jargon`
**Flag**: N/A
**Est. lines**: ~400 (TOML data + adapter rewrite, mostly mechanical)
**Ships via**: `/pr`
**Objective**: First production migration. After this phase, `crates/ynz-diagnostics/src/banned_jargon.rs` no longer holds data — only an adapter that dispatches to `ynz-registry`.
**Why this phase exists**: Smallest, most-isolated migration. Banned jargon is read-only, has trivial schema (word + replacement + reason), and gets the migration pattern locked before moving to harder cases.

**Current-state anchors**:
- `crates/ynz-diagnostics/src/banned_jargon.rs:1-87` — current scattered registry to migrate

**Files (expected scope)**:
- UPDATE `registry/features.toml` — add 30 `[[banned_jargon]]` entries + 8 acronym entries (each: `name`, `replacement`, `reason`)
- UPDATE `crates/ynz-diagnostics/src/banned_jargon.rs` — replace data with thin adapter: `pub fn check(word: &str) -> Option<&'static BannedJargonEntry> { ynz_registry::banned_jargon().find(|e| e.name == word) }` (or similar — match existing API)
- UPDATE `crates/ynz-diagnostics/Cargo.toml` — add `ynz-registry` dep
- DELETE the placeholder `[[banned_jargon]]` entry that Phase 1 put in `registry/features.toml`

**Deviation rule**: Standard.

**Steps**:
1. Read `crates/ynz-diagnostics/src/banned_jargon.rs` exhaustively. For each entry, copy word + replacement + reason into the TOML file.
2. For the 8 acronym entries (UTF-16, etc.) — likely a separate `[[banned_jargon_acronym]]` kind OR a `kind = "acronym"` field on the same `[[banned_jargon]]` entry. Decide based on what reads cleanest in `design/feature-registry.md` schema.
3. Replace the data table in `banned_jargon.rs` with the adapter. PRESERVE the public API surface so callers in `crates/ynz-diagnostics/src/*.rs` don't need rewriting. (If an API rename IS warranted, document it in the PR.)
4. Run `cargo test -p ynz-diagnostics`. Update insta snapshots if jargon-test error text differs (likely identical since reason text is preserved verbatim).
5. Run `cargo test --workspace`. Snapshot updates in other crates that exercise banned-jargon errors get reviewed and committed.

**Acceptance criteria**:
- [x] All 30 banned-jargon entries + 8 acronyms appear in `registry/features.toml` with identical text to pre-migration
- [x] `crates/ynz-diagnostics/src/banned_jargon.rs` has no hardcoded data table — only an adapter
- [x] `cargo test --workspace` passes
- [x] If any insta snapshots changed, the diff was inspected and approved per Risk #4
- [x] The placeholder `[[banned_jargon]]` from Phase 1 is removed

**Quality gate**:
- [x] Adapter API surface preserved (callers don't need rewriting)
- [x] No banned-jargon words introduced in the new TOML or adapter
- [x] Bouncer grep pattern from Phase 0 does NOT flag the adapter (since the data is gone)

**Verification**:
- `cd /workspaces/ynz && cargo test --workspace`
- `cd /workspaces/ynz && bash ~/.claude/hooks/tools/bash/bouncer.sh` (if invocable directly — otherwise verify pattern manually with `grep`)

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 2",
     prompt: "Review the diff for Phase 2 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 1 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 2 done. Code-reviewer: PASS. Ready to commit and move to Phase 3?"

5. **Do NOT start Phase 3** until the user confirms the commit.

---

### Phase 3: Migrate primitive intrinsics + type-attached constants
**PR scope**: Move `PrimitiveIntrinsicTable` (print_types, free_fns, methods, methods_1arg) from `crates/ynz-typeck/src/intrinsics.rs` AND `type_attached_const_type()` from `crates/ynz-typeck/src/check.rs:3698-3707` to the registry. Both are typeck-side and share consumer crates.
**Branch**: `feat/v0-2-m1-migrate-typeck-tables`
**Flag**: N/A
**Est. lines**: ~700 (mechanical-refactor exception — averaging <5 lines per file across many call sites)
**Ships via**: `/pr`
**Objective**: Migrate the two typeck-side scattered registries in one PR. After this phase, typeck reads primitive-method dispatch + type-attached constants from the registry exclusively.
**Why this phase exists**: Combining typeck-side migrations into one phase keeps consumer-side API churn isolated. Splitting these would mean two PRs touching the same `intrinsics.rs` + `check.rs` files in close succession.

**Current-state anchors**:
- `crates/ynz-typeck/src/intrinsics.rs:20-144` — `PrimitiveIntrinsicTable` builder
- `crates/ynz-typeck/src/check.rs:1036` — current consumer of type-attached-constant resolver
- `crates/ynz-typeck/src/check.rs:3698-3707` — `type_attached_const_type()` data table

**Files (expected scope)**:
- UPDATE `registry/features.toml` — add `[[primitive_intrinsic]]` entries (each free_fn + method + methods_1arg + print_type) and `[[type_attached_constant]]` entries (int.max/min, float.max/min/epsilon, number.max/min/epsilon)
- UPDATE `crates/ynz-typeck/src/intrinsics.rs` — `PrimitiveIntrinsicTable` becomes adapter that reads from `ynz-registry`
- UPDATE `crates/ynz-typeck/src/check.rs` — `type_attached_const_type()` becomes adapter that reads from `ynz-registry`
- UPDATE `crates/ynz-typeck/Cargo.toml` — add `ynz-registry` dep
- UPDATE `crates/ynz-codegen/src/emit.rs` — if codegen has its own type-attached-constant value table (per Research Finding #3 — codegen lowers the same lookup at emit time), migrate that too
- UPDATE `crates/ynz-codegen/Cargo.toml` if needed

**Deviation rule**: Standard. Specifically: codegen-side value table migration is in-scope for this phase since the typeck/codegen pair must agree.

**Steps**:
1. Inventory `PrimitiveIntrinsicTable` exhaustively: list every print_type, free_fn (with all overloads), method (with all overloads), methods_1arg entry. Each entry's TOML form: `name`, `kind`, `arity`, `receiver_type` (if method), `param_types`, `return_type`, `since` (milestone tag — pull from comments in the .rs file).
2. Inventory `type_attached_const_type()` exhaustively: 7 entries (int.max, int.min, float.max, float.min, float.epsilon, number.max, number.min, number.epsilon). Each entry's TOML form: `type_name`, `const_name`, `value_type`, `value_literal` (the numeric literal as a string — `"9223372036854775807"` for int.max — to avoid precision loss).
3. Locate codegen's value table (probably `crates/ynz-codegen/src/emit.rs`). Migrate to the same `[[type_attached_constant]]` entries; codegen reads `value_literal` and emits an LLVM constant.
4. Rewrite `PrimitiveIntrinsicTable` and `type_attached_const_type()` as adapters. Preserve the existing public API (call sites at check.rs:1036 and elsewhere should compile unchanged).
5. Run `cargo test -p ynz-typeck`, `cargo test -p ynz-codegen`, `cargo test --workspace`.
6. Verify byte-identical LLVM IR for the M5 fixtures that use type-attached constants (compare `cargo run --bin ynz -- build --emit-ir fixture.ynz` output before and after migration if any fixture exercises this — otherwise create a temporary one for verification).

**Acceptance criteria**:
- [x] All `PrimitiveIntrinsicTable` entries appear in `registry/features.toml`
- [x] All 7 type-attached-constant entries appear in `registry/features.toml`
- [x] `crates/ynz-typeck/src/intrinsics.rs` and `check.rs:3698-3707` no longer hold data — only adapters
- [x] `crates/ynz-codegen/src/emit.rs` value-lookup table (if it existed) is also migrated
- [x] All 830+ tests pass post-migration
- [x] LLVM IR for `int.max` / `number.epsilon` / etc. is byte-identical to pre-migration. Mechanism: capture `--emit-ir` output BEFORE Phase 3 lands on a dedicated fixture exercising every one of the 7 type-attached constants (create one in `crates/ynz-driver/tests/fixtures/type_attached_const_ir.ynz` if no existing fixture covers all 7), commit the captured IR as a golden file, then assert byte-identical IR after Phase 3. NOT a stdout-only check (stdout-equality is downstream of IR and can mask the bug where the registry returns a slightly-different constant that happens to print the same digits).
- [x] No insta snapshot changes for primitive-method or type-attached-constant tests (this is a pure refactor — text must match)

**Quality gate**:
- [x] Adapter public API preserved (no rewrites needed at call sites)
- [x] Bouncer grep doesn't flag the adapters
- [x] M4 P5 fixtures (the ones that exercise wrapping/saturating + type-attached constants) still produce identical output

**Verification**:
- `cd /workspaces/ynz && cargo test --workspace`
- `cd /workspaces/ynz && ./target/debug/ynz run crates/ynz-driver/tests/fixtures/m4_player.ynz` (sanity)
- Re-run any fixture that exercises `int.max` / `number.epsilon` and compare stdout

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 3",
     prompt: "Review the diff for Phase 3 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 2 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 3 done. Code-reviewer: PASS. Ready to commit and move to Phase 4?"

5. **Do NOT start Phase 4** until the user confirms the commit.

---

### Phase 4: Migrate keyword/token table
**PR scope**: Move the master keyword list from `crates/ynz-parser/src/lexer.rs:491-620` (the match statement in `lex_identifier_or_keyword`) to the registry. The parser's match statement STAYS (per roadmap: "parser stays as-is, but the LIST of valid keywords becomes registry-driven so error suggestions can use it"); the registry just owns the canonical list for IDE / docs / error-suggestion consumers.
**Branch**: `feat/v0-2-m1-migrate-keyword-table`
**Flag**: N/A
**Est. lines**: ~300
**Ships via**: `/pr`
**Objective**: Registry owns the keyword list; lexer match statement is generated from / verified against the registry at build time.
**Why this phase exists**: IDE autocomplete (v0.2-M2) needs the keyword list. The error-suggestion code in lexer also reads it for "did you mean" suggestions. After this phase, adding a new keyword in a future milestone is ONE TOML edit, and an assertion in `ynz-parser`'s build (or test) ensures the parser's match statement stays in sync.

**Current-state anchors**:
- `crates/ynz-parser/src/lexer.rs:491-620` — current keyword match statement
- `crates/ynz-parser/src/lexer.rs:574-690` — DEFERRED-feature handlers (covered in Phase 5a — call out so reviewer doesn't expect them in this phase)

**Files (expected scope)**:
- UPDATE `registry/features.toml` — add ~50 `[[keyword]]` entries (function, nothing, let, const, true, false, if, else, while, for, in, return, shape, follows, extends, base, hidden, dynamic, Self, self, none, options, is, errors, import, export, sensitive, wait, background; plus ~20 banned declaration keywords as `[[banned_declaration_keyword]]` entries with `replacement` + `reason` fields — distinct from `[[banned_jargon]]` because these are KEYWORDS with redirect-during-lex semantics, not user-prose-text-checking)
- UPDATE `crates/ynz-parser/src/lexer.rs` — `lex_identifier_or_keyword` match statement is REGENERATED from registry (option A: a test asserts the match arms match the registry; option B: build.rs in `ynz-parser` generates the match arms). Decide based on simplicity in the implementation phase.
- UPDATE `crates/ynz-parser/Cargo.toml` — add `ynz-registry` dep
- CHANGES TO lexer.rs:574-690 (deferred-feature handlers) — defer to Phase 5a

**Deviation rule**: Standard.

**Steps**:
1. Inventory every keyword and banned-declaration-keyword in `lexer.rs:491-620`. Each entry's TOML form: `name`, `kind` (either `Keyword` or `BannedDeclarationKeyword`), `since` (milestone), and for BannedDeclarationKeyword: `replacement`, `reason`.
2. Add entries to `registry/features.toml`.
3. Consumer pattern: **TEST-ASSERTION-BASED**, locked. A test in `ynz-parser/tests/keyword_sync.rs` asserts every match arm in `lex_identifier_or_keyword` has a corresponding registry entry and vice versa. Rationale: simpler than build-script-based generation (no include!() macro magic in the lexer hot path); test-failure surface area names exactly the desynced keyword; lexer file stays plain Rust source. Build-script generation was considered and rejected because the perf savings are zero (match arms compile identically either way) and IDE navigation gets worse.
4. Implement the test-assertion pattern.
5. Verify the lexer's "did-you-mean" suggestion code (if it exists in lexer.rs) reads the registry-driven list.
6. Run `cargo test --workspace`.

**Acceptance criteria**:
- [x] All ~50 keywords + ~20 banned-declaration-keywords appear in `registry/features.toml`
- [x] The test-assertion (or build.rs generation) ensures the lexer match statement and the registry stay in sync — verified by temporarily adding a fake keyword to the registry and observing the test fail
- [x] `cargo test --workspace` passes
- [x] Did-you-mean suggestion code (if present) consumes the registry list

**Quality gate**:
- [x] No banned-jargon in new files
- [x] Bouncer grep doesn't flag the parser

**Verification**:
- `cd /workspaces/ynz && cargo test --workspace`
- Temporarily add a fake `[[keyword]]` entry to TOML, run lexer's sync test, observe failure

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 4",
     prompt: "Review the diff for Phase 4 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 3 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 4 done. Code-reviewer: PASS. Ready to commit and move to Phase 5a?"

5. **Do NOT start Phase 5a** until the user confirms the commit.

---

### Phase 5a: Migrate existing reserved-but-deferred handlers
**PR scope**: Move existing deferred-feature handlers from `crates/ynz-parser/src/lexer.rs:574-690` (sized numeric types `f32`–`f64`, `i8`–`i64`, `u8`–`u64`; `test` keyword) to the registry. Error rendering pulls WHY/SUBSTITUTE/TRIGGER from the registry. Insta snapshots updated for improved-text per locked decision.
**Branch**: `feat/v0-2-m1-migrate-deferred-existing`
**Flag**: N/A
**Est. lines**: ~400
**Ships via**: `/pr`
**Objective**: Migrate the deferred-feature handlers that already exist in code. After this phase, the lexer has no hardcoded deferred-feature error text — it dispatches to registry-driven rendering.
**Why this phase exists**: Split from 5b because 5a is a pure migration (existing → registry) while 5b is data ENTRY (NEW entries from `design/future/*.md`). Different code-review profile.

**Current-state anchors**:
- `crates/ynz-parser/src/lexer.rs:574-690` — current deferred handlers
- `examples/errors/m*_errors.ynz` — existing snapshot-tested fixture files (find which ones exercise sized-int errors and `test` keyword)

**Files (expected scope)**:
- UPDATE `registry/features.toml` — add `[[deferred_language_feature]]` entries for: `f32`, `f64`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` (each with `substitute = "int"` or `substitute = "float"` or `substitute = "number"`, `why = "..."`, `ships_in = "v2+"`, `design_doc = "design/mvp-scope.md"`) and `test` (`substitute = ""` since there's no current substitute, `why = "..."`, `ships_in = "v0.12"`, `design_doc = "design/mvp-scope.md"`)
- UPDATE `crates/ynz-parser/src/lexer.rs:574-690` — replace hardcoded error rendering with registry-driven dispatch (`render_deferred_feature_error(registry::deferred_language_features().find(|e| e.name == identifier))`)
- UPDATE `crates/ynz-diagnostics/src/` — add a `render_deferred_feature(entry: &DeferredLanguageFeatureEntry) -> Diagnostic` helper (or similar; place where existing diagnostic templates live)
- UPDATE insta snapshots in `crates/ynz-parser/tests/` and elsewhere — diff reviewed per locked decision

**Deviation rule**: Standard.

**Steps**:
1. Read the existing deferred-feature handlers in lexer.rs:574-690 and document the current error text per entry.
2. Add registry entries for each.
3. Implement `render_deferred_feature` helper in ynz-diagnostics (or wherever the diagnostic-rendering code lives). Output format follows WHAT/WHAT-INSTEAD/WHY: WHAT = "<name> is reserved for v<X.Y>"; WHAT INSTEAD = "Use <substitute> for now"; WHY = entry's `why` field.
4. Replace lexer's hardcoded rendering with calls to the helper.
5. Run `cargo test --workspace`. Review insta snapshot diffs; commit accepted changes.

**Acceptance criteria**:
- [x] All sized-int / sized-float / `test` entries appear in `registry/features.toml` as `[[deferred_language_feature]]`
- [x] `crates/ynz-parser/src/lexer.rs:574-690` no longer contains hardcoded error text for these features
- [x] `render_deferred_feature` helper exists in diagnostics crate
- [x] All 830+ tests pass
- [x] Insta snapshot diffs for deferred-feature errors are reviewed (commit message names them); the new text is uniform (every deferred-feature error follows the same WHAT/WHAT-INSTEAD/WHY shape, only the entry-specific text varies)
- [x] Add intentional triggers for `f32`/`i8`/`test` to a new `examples/errors/v0_2_m1_errors.ynz` (or extend if Phase 0 created it)

**Quality gate**:
- [x] No banned-jargon in new TOML/code
- [x] Bouncer grep doesn't flag the lexer

**Verification**:
- `cd /workspaces/ynz && cargo test --workspace`
- `cd /workspaces/ynz && ./target/debug/ynz run examples/errors/v0_2_m1_errors.ynz 2>&1 | head -100`

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 5a",
     prompt: "Review the diff for Phase 5a of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 4 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 5a done. Code-reviewer: PASS. Ready to commit and move to Phase 5b?"

5. **Do NOT start Phase 5b** until the user confirms the commit.

---

### Phase 5b: Populate full deferred catalog from `design/future/*.md`
**PR scope**: NEW registry entries for every locked deferred feature documented in `design/future/*.md`. No code changes (this is data entry). 13 currently-existing future-design docs map to 13 `[[deferred_language_feature]]` entries (or `[[deferred_tooling_feature]]` for the few that are tooling-side).
**Branch**: `feat/v0-2-m1-deferred-catalog`
**Flag**: N/A
**Est. lines**: ~500 (TOML only — mechanical-refactor exception applies)
**Ships via**: `/pr`
**Objective**: Every locked deferred feature has a registry entry. Future error messages for `gpu`, `foreign`, `arena`, etc. (when those tokens appear in user code) can dispatch via the registry-driven helper from Phase 5a, with no NEW code needed.
**Why this phase exists**: The roadmap explicitly enumerates this work: "Reserved-but-deferred features (every entry in `design/mvp-scope.md` v2+ section AND every entry in `design/future/*.md`: arena, auto-soa, concurrency, http-framework, no-runtime-mode, packages, panic-safety, self-references, supervisor) → registry entries with locked-design pointers." Plus the `release-mode.md` and `string-ptr-len-overhaul.md` and `inline-shape-types.md` files added more recently.

**Current-state anchors**:
- `design/future/*.md` — 13 files (arena, auto-soa, concurrency, http-framework, inline-shape-types, no-runtime-mode, packages, panic-safety, release-mode, self-references, string-ptr-len-overhaul, supervisor; plus index.md which is the index, not a feature)
- `design/mvp-scope.md` — v2+ section (placeholder lines added in Phase 0)

**Files (expected scope)**:
- UPDATE `registry/features.toml` — add 12 `[[deferred_language_feature]]` or `[[deferred_tooling_feature]]` entries (one per future-design doc except index.md). Each entry: `name`, `kind`, `substitute`, `why`, `ships_in`, `design_doc = "design/future/<name>.md"`, plus `triggers` (what user code makes this error fire — for many of these, the token doesn't exist yet so this is N/A — document explicitly)
- UPDATE `design/mvp-scope.md` — back-fill the placeholder lines from Phase 0 with the actual registry-entry names (`Registry entry: deferred_language_feature.arena`, etc.)
- UPDATE `crates/ynz-registry/tests/` — add a `tests/design_future_sync.rs` test that walks `design/future/*.md` filenames and asserts every file (except a hardcoded skip list — currently exactly `index.md` — declared as `const SKIP: &[&str] = &["index.md"];` at the top of the test file with a comment naming the rationale) has a corresponding registry entry, and every registry entry's `design_doc` field points to an existing file. Adding a new index-style file (e.g., `README.md`) requires editing `SKIP` — explicit, not implicit. Avoids the false-positive drift of allowlist-by-pattern.

**Deviation rule**: Standard.

**Steps**:
1. For each `design/future/*.md` file, read enough of the file to extract: `substitute` (what's the current-Yinz way to do this), `why` (one-paragraph rationale), `ships_in` (target version if locked, else `"v2+"`), `triggers` (what tokens / syntax the user would write to hit this error today — for most, "no token exists yet, locked design only").
2. Add entries to TOML.
3. Backfill `design/mvp-scope.md` placeholder lines.
4. Write the bidirectional consistency test.
5. Run `cargo test --workspace` — the new test must pass.

**Acceptance criteria**:
- [x] All 12 `design/future/*.md` files (excluding index.md) have a corresponding `[[deferred_language_feature]]` or `[[deferred_tooling_feature]]` entry
- [x] `design/mvp-scope.md` placeholder lines from Phase 0 are filled in with real registry-entry names
- [x] The bidirectional consistency test passes (every future-doc has an entry; every entry's `design_doc` field references an existing file)
- [x] `cargo test --workspace` passes
- [x] No code changes outside TOML, design/, and the new test file

**Quality gate**:
- [x] No banned-jargon in any of the WHY paragraphs (they're going to be user-facing eventually when error rendering picks them up)
- [x] Each WHY is a concrete sentence the user can read, not a placeholder

**Verification**:
- `cd /workspaces/ynz && cargo test --workspace`
- `cd /workspaces/ynz && cargo test -p ynz-registry design_future_sync` (the bidirectional test)

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 5b",
     prompt: "Review the diff for Phase 5b of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 5a commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 5b done. Code-reviewer: PASS. Ready to commit and move to Phase 6?"

5. **Do NOT start Phase 6** until the user confirms the commit.

---

### Phase 6: Populate diagnostic-template + muted-hint-domain entries
**PR scope**: Add `[[diagnostic_template]]` entries for the canonical / reusable diagnostics (e.g., "cannot mutate const", "use after give", "hidden field access from outside file"). Add `[[muted_hint_domain]]` entries for every domain in `.claude/rules/inference.md`. NO consumer wiring — these entries are populated for v0.2-M2 LSP to consume.
**Branch**: `feat/v0-2-m1-templates-and-hints`
**Flag**: N/A
**Est. lines**: ~400 (TOML data)
**Ships via**: `/pr`
**Objective**: Registry contains everything v0.2-M2 LSP needs to read without M2 having to also do data entry. Splits the work cleanly: M1 owns the registry; M2 wires the LSP to read it.
**Why this phase exists**: The roadmap calls these out as M1 deliverables. Pushing them to M2 would couple M2's LSP delivery to data-entry work, slowing the LSP user-facing delivery.

**Current-state anchors**:
- `crates/ynz-diagnostics/src/diagnostic.rs` — `DiagnosticKind` enum (TypeMismatch, MutationOfConst, NotDefined, HiddenAccess, Consumed, Borrowed) — these are the canonical reusable diagnostic kinds
- `.claude/rules/inference.md` — muted-hint domain catalog (type inference, function param type inference, ownership at call sites, wait points, lifetimes, allocators, copy points, `array<T>`→`fixed<T>` promotion, `let`→`const` promotion)

**Files (expected scope)**:
- UPDATE `registry/features.toml` — add `[[diagnostic_template]]` entries for each `DiagnosticKind` variant (each: `kind_name`, `what_template`, `what_instead_template`, `why_template` — these are PARAMETRIZED with `{placeholders}` since the dynamic message-construction stays in code; the registry just owns the canonical text shape)
- UPDATE `registry/features.toml` — add `[[muted_hint_domain]]` entries (each: `domain`, `placement_category` ∈ {`Addition`, `Replacement`, `Informational`}, `description`, `example_source`, `example_hint_rendered`)
- UPDATE `crates/ynz-registry/src/schema.rs` — confirm/extend schema types for the two new entry kinds
- NO consumer wiring in `crates/ynz-diagnostics/` or `crates/ynz-typeck/` — that's M2's job per the roadmap; this phase only populates data

**Deviation rule**: Standard. Specifically: if it turns out wiring the diagnostic-template consumer is mechanical and adds <50 lines, DO IT — but ONLY if it doesn't change error text at all. Document in PR. If wiring would change error text or introduce risk, defer to M2.

**Steps**:
1. Inventory `DiagnosticKind` variants in `crates/ynz-diagnostics/src/diagnostic.rs`. For each variant where the WHAT/WHAT-INSTEAD/WHY text is canonical (used in multiple places, or once but unchanging), draft a template entry. For variants where the message is constructed per-site, do NOT add a template entry (state in PR).
2. Inventory muted-hint domains from `.claude/rules/inference.md`. For each domain row in the table, draft a TOML entry with example source code + example rendered hint text (copy from the rules file).
3. Add entries to TOML.
4. Extend `schema.rs` if new field types are needed.
5. Run `cargo test --workspace` — should pass since no consumer changes.

**Acceptance criteria**:
- [x] Every canonical `DiagnosticKind` variant has a `[[diagnostic_template]]` entry; non-canonical variants are listed in the PR description as "intentionally not templated"
- [x] Every muted-hint domain from `.claude/rules/inference.md` has a `[[muted_hint_domain]]` entry with `placement_category` set correctly
- [x] `cargo test --workspace` passes
- [x] No user-facing behavior change (no consumer wiring)

**Quality gate**:
- [x] No banned-jargon in template / hint text (they'll be user-facing in M2)
- [x] Schema accommodates `{placeholders}` in template text (string field; consumer in M2 substitutes at render time)

**Verification**:
- `cd /workspaces/ynz && cargo test --workspace`
- Manual TOML inspection: every domain table row from `.claude/rules/inference.md` appears as an entry

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 6",
     prompt: "Review the diff for Phase 6 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 5b commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 6 done. Code-reviewer: PASS. Ready to commit and move to Phase 7?"

5. **Do NOT start Phase 7** until the user confirms the commit.

---

### Phase 7: Consistency tests + Bouncer integration
**PR scope**: Add comprehensive `ynz-registry/tests/consistency.rs` covering all entry-kind invariants. Verify the Bouncer pattern from Phase 0 catches new violations. Register the pattern in CI if Yinz has CI (the state.md notes "Wire up GitHub Actions CI (ci.yml already written, just needs configuration)" as a Later todo — verify status).
**Branch**: `feat/v0-2-m1-consistency-tests`
**Flag**: N/A
**Est. lines**: ~400 (mostly tests + adjustments)
**Ships via**: `/pr`
**Objective**: After this phase, CI fails if anyone violates registry invariants (missing required fields, broken bidirectional references, etc.) AND the Bouncer postaudit warns on diffs that re-introduce scattered-registry patterns.
**Why this phase exists**: Without enforcement, the SSOT decays. Tests + Bouncer make the discipline mechanical.

**Current-state anchors**:
- `.claude/graveyard.md` — entry from Phase 0 with grep pattern
- `.github/workflows/ci.yml` (if exists) — current CI config
- `crates/ynz-registry/tests/` — Phase 5b's bidirectional test is the model

**Files (expected scope)**:
- NEW `crates/ynz-registry/tests/consistency.rs` (~200 lines):
  - Every `[[banned_jargon]]` has `replacement` and `reason`
  - Every `[[deferred_language_feature]]` has `substitute`, `why`, `ships_in`, `design_doc`
  - Every `[[deferred_tooling_feature]]` has the same fields
  - Every `[[type_attached_constant]]` has parseable `value_literal` matching `value_type`
  - Every `[[primitive_intrinsic]]` exists in `PrimitiveIntrinsicTable` (or its adapter equivalent) — closes the loop
  - Every `[[keyword]]` corresponds to a match arm in the lexer (already enforced by Phase 4's sync test; this just confirms it's still wired)
  - Every `[[muted_hint_domain]]` has a `placement_category` ∈ {`Addition`, `Replacement`, `Informational`}
  - Each `[[deferred_language_feature]].design_doc` and `[[deferred_tooling_feature]].design_doc` references an existing file
- UPDATE `.github/workflows/ci.yml` (if exists) — add `cargo test -p ynz-registry` as a dedicated step so consistency failures are loud
- UPDATE `.claude/graveyard.md` — add a test-the-Bouncer-pattern verification note: "Pattern tested against current main on YYYY-MM-DD; matches the X scattered registries about to migrate; zero false positives in other crates."
- VERIFY the Bouncer pattern's regex syntax matches the actual Bouncer infrastructure (per CLAUDE.md "Active Enforcement" / `.bouncer.log`)

**Deviation rule**: Standard. If CI doesn't exist yet (per todos.md), defer the CI-config change to a separate chore PR; the test file landing is the substantive deliverable.

**Steps**:
1. Write `consistency.rs` with one test function per invariant above.
2. Run the tests; iterate on registry entries if any invariant fails (caught a real bug — good!).
3. Verify the Bouncer pattern from Phase 0 still catches what it should AND doesn't catch what it shouldn't (re-run against current main).
4. Wire CI if applicable (or note in PR).
5. Update graveyard with a "pattern verified on YYYY-MM-DD" footnote.

**Acceptance criteria**:
- [x] `consistency.rs` exists with one test per invariant
- [x] All tests pass
- [x] If a test catches a real malformed entry from Phases 2-6, fix the entry in this PR and document in PR description
- [x] Bouncer pattern verified post-migration (no false positives on current code; matches a deliberately-introduced scattered-array sample in a throwaway commit)
- [x] CI runs `cargo test -p ynz-registry` (or the change is deferred to a chore PR with a tracking todo)

**Quality gate**:
- [x] Tests are well-named (test names describe the drift class they prevent)
- [x] No banned-jargon in test code or messages

**Verification**:
- `cd /workspaces/ynz && cargo test -p ynz-registry`
- `cd /workspaces/ynz && cargo test --workspace`

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

1. **Persist plan state.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 7",
     prompt: "Review the diff for Phase 7 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 6 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue.

4. **Prompt the user:** "Phase 7 done. Code-reviewer: PASS. Ready to commit and move to Phase 8?"

5. **Do NOT start Phase 8** until the user confirms the commit.

---

### Phase 8: Verification, Demo & Error Gallery, Tag `v0.2.0-m1`
**PR scope**: Full-system verification phase. Extend examples/errors with the new error gallery file, sanity-check examples/basics still runs unchanged (it shouldn't — this is a refactor), bump `Cargo.toml` to `0.2.0-m1`, commit, tag, optionally publish (but Yinz isn't on crates.io yet — skip).
**Branch**: `feat/v0-2-m1-verification-and-tag`
**Flag**: N/A
**Est. lines**: ~200 (fixture additions + version bump + CHANGELOG)
**Ships via**: `/pr` then `/release`
**Objective**: M1 ships. v0.2.0-m1 tag cut. v0.2-M2 unblocked.
**Why this phase exists**: Per Yinz convention (v0.1.0-m4, v0.1.0-m5, v0.1.0-m6, v0.1.0-m7), each milestone gets an intermediate tag for traceability. Step 10 of /plan also requires this verification phase.

**Current-state anchors**:
- `examples/basics/entrypoint.ynz` — current contents (should run unchanged post-M1)
- `Cargo.toml` (workspace + per-crate) — current version `0.1.0`
- `CHANGELOG.md` if exists
- `.claude/plans/done/m{4,5,6,7}-*.md` — pattern for M1 plan archival

**Files (expected scope)**:
- NEW or UPDATE `examples/errors/v0_2_m1_errors.ynz` — finalize all intentional triggers from Phases 5a, 5b, 6 (banned-jargon triggers, deferred-feature triggers, type-attached-constant misuse, muted-hint placeholder — though no consumer for that yet, so just data)
- UPDATE `Cargo.toml` (workspace + all crates) — bump version to `0.2.0-m1`
- UPDATE `CHANGELOG.md` — generate section from merged PRs since `v0.1.0` per `/release` skill
- UPDATE `.claude/state.md` — append M1-complete decision row (with WHY); status: M1 SHIPPED
- UPDATE `.claude/todos.md` — mark v0.2-M1 done in Done section
- MOVE `.claude/plans/active/v0-2-m1-feature-inventory-sync.md` → `.claude/plans/done/` (or flip front-matter `status: done` and let the radar move it — verify which mechanism the project uses)
- UPDATE `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — mark v0.2-M1 milestone as `status: shipped`

**Deviation rule**: Standard. `examples/basics/entrypoint.ynz` extension is N/A this phase per Demo & Error Gallery decision in Invariants section.

**Steps**:
1. Run `cargo clean && time cargo build --workspace` — capture final cold build time; compare to Phase 0 baseline. Must be within ±10% per Invariants.
2. Run `cargo test --workspace` — all 830+ tests must pass.
3. Run every fixture in `crates/ynz-driver/tests/fixtures/` end-to-end (`./target/debug/ynz run fixture.ynz`); compare stdout to expected. Spot-check on at least: `hello.ynz`, `m3_fib.ynz`, `m4_player.ynz`, an M5/M6/M7/M8 fixture if available.
4. Run `examples/basics/entrypoint.ynz` if it's executable (`./target/debug/ynz run examples/basics/entrypoint.ynz`). Output must match pre-M1.
5. Run `examples/errors/v0_2_m1_errors.ynz` and snapshot stderr (insta).
6. Re-run the jargon-audit test (`tests/jargon_audit.rs` per Invariants Teaching section).
7. Bump versions, generate CHANGELOG, commit.
8. Open PR; after merge, invoke `/release` to cut `v0.2.0-m1` tag.
9. Update state.md, todos.md, archive plan file.

**Acceptance criteria**:
- [x] `cargo build --workspace` cold time within ±10% of Phase 0 baseline (documented in PR)
- [x] `cargo test --workspace` all pass (830+)
- [x] Every fixture in `crates/ynz-driver/tests/fixtures/` runs end-to-end with expected stdout
- [x] `examples/basics/entrypoint.ynz` runs unchanged (or N/A if it isn't currently runnable)
- [x] `examples/errors/v0_2_m1_errors.ynz` exists with intentional triggers per Invariants Demo & Error Gallery; insta snapshot committed
- [x] Jargon-audit test passes
- [x] `Cargo.toml` bumped to `0.2.0-m1`
- [x] `CHANGELOG.md` section added covering M1
- [x] `state.md`, `todos.md`, roadmap updated
- [x] Plan file moved to `.claude/plans/done/`
- [x] `v0.2.0-m1` git tag cut by `/release`

**Quality gate** (cumulative — covers full plan):
- [x] All inputs validated (registry schema validates every TOML entry; no consumer can construct an invalid entry)
- [x] Auth/authz: N/A (compiler)
- [x] Error handling: every registry-driven error renders through `Diagnostic` constructor (preserves WHAT/WHAT-INSTEAD/WHY enforcement)
- [x] No security exposures (no new attack surface; build.rs runs at build-time only; no user input)
- [x] Performance: cold build within ±10%; incremental build zero overhead; LLVM IR byte-identical for type-attached constants
- [x] Tests: 830+ pre-existing + new consistency tests + new sync tests; happy + error paths
- [x] Existing tests still pass
- [x] Types complete (no `any`/`unknown`/`unwrap` in build.rs error paths)
- [x] Follows codebase conventions (existing `*Table` adapter pattern)

**Verification**:
- `cd /workspaces/ynz && cargo clean && time cargo build --workspace`
- `cd /workspaces/ynz && cargo test --workspace`
- `cd /workspaces/ynz && for f in crates/ynz-driver/tests/fixtures/*.ynz; do echo "--- $f ---"; ./target/debug/ynz run "$f"; done`
- Run `/release` skill after PR merge

**Exit Sequence — RUN THESE STEPS (not a checklist; these are actions to execute):**

This is the **final phase** — it has two code-reviewer invocations: one for Phase 8's own diff (per the normal per-phase protocol), and one cumulative across the whole plan.

1. **Persist plan state — phase 8.** Tick this phase's `Acceptance criteria` checkboxes for every criterion the diff actually met. Tick `Quality gate` checkboxes for items verified. Bump `last_updated:` in front-matter to today.

2. **Invoke code-reviewer for Phase 8's diff:**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Review Phase 8",
     prompt: "Review the diff for Phase 8 of plan at .claude/plans/active/v0-2-m1-feature-inventory-sync.md against acceptance criteria, quality gate, rules, and laziness patterns. Diff command: git diff <Phase 7 commit>..HEAD. Output in standard format."
   })
   ```

3. **Handle the Phase 8 verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue to step 4.

4. **Plan-file final persistence pass.** Verify ALL phases' acceptance-criteria and quality-gate checkboxes across the plan are accurate. Update the overall `## Quality Checklist (verify at completion)` block. Make sure `last_updated:` is today's date.

5. **Invoke code-reviewer for the cumulative plan diff (Step 10f):**
   ```
   Agent({
     subagent_type: "code-reviewer",
     description: "Final plan sweep",
     prompt: "End-of-plan review for .claude/plans/active/v0-2-m1-feature-inventory-sync.md. Cumulative diff scope. Audit against ALL phases' acceptance criteria, all Quality Gate items, the plan's overall Quality Checklist, and rules. Catch anything per-phase reviews missed. Diff command: git diff <plan-base-commit>..HEAD. Output in standard format."
   })
   ```

6. **Handle the final-sweep verdict.** BLOCK → fix Required Fixes, re-invoke (max 3 rounds). PASS → continue to step 7.

7. **Flip status and prompt user.** Edit front-matter `status: active` → `status: done`. Tell the user: "Phase 8 done. Both code-reviewer passes received. Plan complete — ready to commit, run `/release` to cut `v0.2.0-m1`, and archive the plan file?"

8. **After commit + release**, the radar moves the file from `plans/active/` to `plans/done/` on next rebuild.

---

## Quality Checklist (verify at completion)
- [x] All inputs validated (registry schema, build.rs panics on invalid)
- [x] Auth/authz: N/A
- [x] Error handling: WHAT/WHAT-INSTEAD/WHY preserved everywhere; no leaks
- [x] No SQL injection / XSS / path traversal / secret exposure (N/A — compiler)
- [x] Performance: cold build ±10%, incremental zero-overhead, LLVM IR byte-identical for migrated constants
- [x] Tests: 830+ pre-existing tests pass; new consistency tests pass; new sync tests pass
- [x] Existing tests still pass
- [x] Types complete (no `any`/`unwrap` in error paths)
- [x] Follows existing codebase conventions (`*Table` adapter pattern, build.rs convention)
- [x] Every phase received a code-reviewer PASS before committing (Step 9a)
- [x] Final cumulative code-reviewer sweep passed (Step 10f)
- [x] Plan-file acceptance-criteria checkboxes accurate across all phases (Step 9b)

## Deferrals in This Plan (all tracked per global graveyard "Untracked Deferrals" entry)

Per `~/.claude/memory/feedback_deferrals_must_be_tracked.md`: every deferral below names WHERE it's tracked and WHAT triggers the eventual fix.

| Deferral | Tracker artifact | Trigger for fix |
|---|---|---|
| `[[stdlib_api]]` entry-kind reserved in schema, populated with ZERO entries in M1 | `design/feature-registry.md` schema reference (created Phase 0) + this plan's Constraints section | Each v0.5+ stdlib milestone (file, math, cli, json, etc.) populates its own entries — bound to the milestone delivering the API |
| `### Feature Registry Entries` 7th invariants subsection — this plan is exempt | `.claude/rules/plan-invariants.md` 7th subsection text (created Phase 0) marked "applies to plans from v0.2-M2 onward" | Every v0.2-M2-onward plan must include the subsection; rule mechanically enforces |
| Phase 6 populates diagnostic-template and muted-hint-domain entries with NO consumer wired in M1 | `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` v0.2-M2 milestone scope (autocomplete + diagnostics) + v0.2-M5 milestone scope (muted-hint surfaces) | v0.2-M2 LSP work wires the diagnostic-template consumer; v0.2-M5 wires the muted-hint consumer |
| `design/mvp-scope.md` placeholder lines created in Phase 0 with `Registry entry: TBD M1 (P5b)` | This plan's Phase 0 (creates placeholder) + Phase 5b (back-fills with real entry names) | Phase 5b acceptance criterion includes back-fill verification |
| CI wiring of `cargo test -p ynz-registry` (Phase 7) may defer to a separate chore PR if CI doesn't exist yet | `.claude/todos.md` Later section: "Wire up GitHub Actions CI (ci.yml already written, just needs configuration)" already tracks the broader CI deferral | When the broader CI wiring lands, this test gets added to the CI step list. Phase 7 PR description names the test for the future CI PR to wire. |

| `design_future_sync.rs` SKIP list reduces Phase 5b acceptance criterion scope — 7 future docs intentionally excluded (auto-soa, concurrency, http-framework, inline-shape-types, panic-safety, string-ptr-len-overhaul, supervisor) | SKIP list in `crates/ynz-registry/tests/design_future_sync.rs` (const SKIP with per-entry rationale) | Already tracked: if a skipped doc gains a user-facing token, add a registry entry AND remove it from SKIP at that time |
| `build.rs` does not enforce `return_type` required for `method`/`method_1arg`/`free_fn` kinds — only `print_type` silently defaults to `""` | Concern #1 from cumulative code-reviewer (2026-05-20) | v0.2-M2 or first PR that touches `build.rs` — add per-kind required-field enforcement; add a `consistency.rs` test asserting every non-`print_type` entry has a parseable `return_type` |
| `banned_declaration_keyword` text still hardcoded in `lexer.rs:621-672` (type/struct/class/interface/enum/abstract/pub/private/protected/public/future/goroutine) — registry has identical text but no sync test enforces fidelity | Concern #3 from cumulative code-reviewer (2026-05-20) | v0.2-M2 — add text-fidelity assertion to `keyword_sync.rs` comparing each lexer match arm's `what_instead`/`why` strings against the registry entry |

If any new deferral surfaces during execution, append a row to this table in the same PR that introduces it. No untracked deferrals.

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each Phase is one branch, one PR; bundled work only when single coherent capability (Phase 3 bundles two typeck-side migrations because they share consumer crates; Phase 5 split into 5a/5b because migration ≠ data entry)
- **Shadow main branches**: every phase branch starts from `main` and PRs to `main`; no long-lived integration branch
- **Building the engine before shipping value**: M1 is admittedly foundation work, but it's the foundation v0.2-M2 (LSP thin slice — user-visible) needs. M1 ships value to a downstream milestone, not to end-users directly. Acceptable per roadmap framing
- **Hotfix that isn't**: no phase is prefix `hotfix/`; this is feature work
- **Abandoned branches**: each phase has a clear ship target via `/pr` and gets merged before the next phase starts (Step 9 of /plan)
- **Flag graveyards**: no feature flags in this milestone (compiler refactor; not a runtime-toggleable feature). N/A by construction
