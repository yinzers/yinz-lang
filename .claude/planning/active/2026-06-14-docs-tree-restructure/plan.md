---
name: "docs-tree-restructure"
plan-id: "2026-06-14-docs-tree-restructure"
status: "active"
roadmap-id: null
session-id: []
created_at: "2026-06-14"
updated_at: "2026-06-14"
metadata:
  type: "plan"
legacy:
  note: "Fields below are preserved verbatim from the pre-migration .claude/plans/ ledger-format frontmatter (2026-07-01 migration to .claude/planning/). session-id history was not tracked pre-migration."
  slug: docs-tree-restructure
  type: execution
  owner: Patrick Rizzardi
  status: active
  files:
    - docs/**
    - spec/**
    - design/**
    - .claude/rules/documentation.md
    - .claude/rules/docs-checklist.md
    - .claude/design-sources.md
    - CLAUDE.md
    - crates/**
    - examples/**
  created: 2026-06-14
  last_updated: 2026-06-15
  depends_on: [v0-3-m3d-cpu-parallelization]
---


# Plan: Documentation Tree Restructure → Global docs/ Standard

Created: 2026-06-14
Status: pending_approval

## ⚠️ EXECUTION PRECONDITION — BLOCKED ON m3d (read before running ANY phase)

> [!CAUTION]
> **DO NOT execute this plan until `v0-3-m3d-cpu-parallelization` is `status: done` (moved to `plans/done/`).**
>
> Phase 2 rewrites doc-path references in **55 crate source files** (`crates/**` doc-comments). m3d is *actively editing crate files* (`ynz-codegen/src/emit.rs`, `ynz-typeck/src/check.rs`, `ynz-typeck/src/lib.rs`, `ynz-driver/tests/integration.rs`, …). Running Phase 2 while m3d is in-flight = guaranteed merge conflicts on the compiler source and risk of derailing the milestone.
>
> **First action of execution (gate check):** confirm m3d is done. If it is NOT, STOP and warn Patrick. Do not proceed.

This plan is **plan-creation only** right now. It is parked until m3d ships.

---

## Context & Why

**Goal**: Migrate the project's two-tree documentation layout (`/spec` + `/design`) onto the new global docs standard — a single `docs/` tree with `docs/user/` (Diátaxis-tagged user-facing) and `docs/internal/{architecture,decisions}` (compiler design + append-only ADRs) — add a navigation index, tag every doc with a `doc-type`, extract buried dated decisions into ADRs, convert the stale project `docs-checklist.md` into a project-local `rules/documentation.md` that cites+extends the global rule, and repoint every cross-reference. **Plus (this session):** split spec docs by maturity — shipped/complete-feature specs become faithful `[locked]` docs in `docs/user/` (verified vs code); aspirational/unbuilt-feature idea-docs move as-is to a new `docs/internal/ideas/` bucket tagged `[aspirational]`; and the project rule encodes a **doc lifecycle** — when a plan locks a feature, that plan MUST design+lock the real doc and delete/trim its idea-doc.

**Why**: Three concrete pains Patrick hit (all real, all diagnosed this session):
1. **Can't tell internal vs user-facing vs algorithm docs apart** — no labeling, no index. The current `spec`/`design` split doesn't self-document (Patrick reads "spec" as internal; the name fails Golden Rules 2 & 12).
2. **No decision tree / map of the docs** — a reader has nowhere to "start here."
3. **Decisions appended into docs, buried** — `concurrency.md` carries dated `LOCKED 2026-06-05` headings and an appended `## Design Divergences (M3b)` section. Patrick stops reading at the first decision, mistaking it for the answer. This is exactly the **garden-path anti-pattern** `memory/doc-style.md` now bans.

The root cause: the project `docs-checklist.md` predates the global `documentation.md` (Jun 12), `doc-style.md`, and the new `doc-types.md` substance layer. The structure didn't drift — the rules under it moved and the project rule never followed.

**Background — what exists today**:
- `/spec/` (32 `.md`) — user-facing language spec. Per Patrick: **mostly aspirational idea-dumps**, not faithful to a built compiler; will be rewritten as each feature gets properly planned and locked. Topic-organized (`collections.md`, `ownership.md`).
- `/design/` (47 top-level `.md` + `future/` + `future/gui/` + `stdlib/`) — compiler design rationale.
- `design/decisions.md` (index-only), `design/open-questions.md`, `spec/overview.md`, `design/future/index.md` are the closest things to navigation today.
- `.claude/design-sources.md` globs `[locked] design/**/*.md` and `[locked] spec/**/*.md` — the design-compliance gate depends on these paths.

**The three global docs rules this plan targets** (the fixed standard):
- `~/.claude/rules/documentation.md` — WHEN/WHERE (the `docs/` layout, deliverable-of-the-change rule, §g project-extension mechanism).
- `~/.claude/memory/doc-style.md` — HOW to present (progressive disclosure, TL;DR, present-tense living-doc, garden-path ban).
- `~/.claude/memory/doc-types.md` — WHAT substance each doc-type carries (architecture = the algorithm; ADR = rejected alternatives; per-type litmus; `doc-type:` frontmatter; one-doc-one-mode; Diátaxis for user docs).

**Constraints**:
- **Must not derail m3d** — see Execution Precondition above.
- The move must never leave the repo with dangling `design/`/`spec/` refs in its final state.
- Aspirational spec content must be preserved faithfully — the restructure is **structural, not a content-rewrite** (except the targeted ADR extraction + present-tense of dated decision sections). Do not make idea-dump docs read as more authoritative/faithful than they are.

**Success criteria**:
- `docs/user/` + `docs/internal/{architecture,decisions}` exist; all content moved; zero dangling `design/`/`spec/` path refs anywhere in-tree (verified by grep).
- Every doc carries a `doc-type:` frontmatter tag.
- `docs/README.md` is a navigation index / decision-tree labeling internal vs user vs the doc-type buckets.
- Buried dated decisions are extracted into `docs/internal/decisions/` ADRs; their source docs read present-tense and link to the ADR.
- `.claude/rules/documentation.md` (project) cites+extends the global rule; `docs-checklist.md` is retired into it.
- `.claude/design-sources.md` globs updated to `docs/**`; the design-compliance gate still resolves.

## Research Findings

- **Cross-reference blast radius = 126 files** reference `design/`/`spec/` paths:
  - `crates/**` (Rust doc-comments): **55 files** — the large surface, and the m3d-collision surface.
  - `design/**` internal cross-links: 63 files.
  - `.claude/plans/**`: 32 files (mostly done/paused — scope note: **skip done-plans for link-fixing**).
  - `.claude/rules/` + `CLAUDE.md`: 13 files.
  - `spec/**` internal: 7. `examples/**`: 7. `registry/`: 1.
- **Spec is 32 topic files**, each mixing reference + explanation — confirms the "tag dominant mode, don't fragment" call over a full Diátaxis split.
- **ADR-extraction candidates** (docs carrying `LOCKED <date>` / `Design Divergences` / `DECIDED <date>` / dated parentheticals): `concurrency.md`, `no-function-coloring.md`, `type-system.md`, `linting.md`, `inline-shape-types.md`, `golden-rules.md`, `versioning.md`, `fmt.md`, `mvp-scope.md`, `lsp.md`, `stdlib/filesystem.md`, `stdlib/encoding.md`, `self-references.md`, `future/gui/*`, `open-questions.md`, `decisions.md` (17 files).
- **No project `plan-checklist.md` override** exists — global checklist applies, with compiler/docs-domain N/As noted (Valibot/SQL/auth/N+1 are all N/A for a docs move).
- **`design/decisions.md` is already index-only** — it becomes raw material for `docs/README.md` + the `docs/internal/decisions/` ADR index.
- The global layout ships a `user/` tree "ONLY when the product has end users" — Yinz qualifies (people who write Yinz code), so `docs/user/` applies with `doc-type` tags.

## Decision Ledger

| # | Decision / claim | Class | Citation / recorded answer |
|---|---|---|---|
| 1 | 126 files reference `design/`/`spec/` paths; 55 are in `crates/**` | verified | `grep -rIlE '\b(design|spec)/[a-z0-9-]+\.md'` per-area counts, this session (ERE alternation = bare `\|`, not `\\\|`) |
| 2 | m3d actively edits crate files (collision surface) | verified | git status: `M crates/ynz-codegen/src/emit.rs`, `M crates/ynz-typeck/src/check.rs`, `M crates/ynz-typeck/src/lib.rs` |
| 3 | `design-sources.md` globs `design/**` + `spec/**` as `[locked]`; gate depends on them | verified | `.claude/design-sources.md` lines: `- [locked] spec/**/*.md`, `- [locked] design/**/*.md` |
| 4 | `concurrency.md` carries garden-path dated decision sections (ADR candidates) | verified | `design/concurrency.md` headings: `## Suspension … (LOCKED 2026-06-05)`, `## Design Divergences (v0.3-M3b …)` |
| 5 | Classification = **shipped vs not** (applies to BOTH spec + design). SHIPPED feature → `[locked]` faithful doc (user spec → `docs/user/`, design → `docs/internal/architecture/`), fixed NOW. NOT shipped (incl. `design/future/*`, `design/stdlib/*`, unplanned spec+design) → `docs/internal/ideas/` `[aspirational]`, moved as-is. **TWO carve-outs stay `[locked]` despite not being shipped features — CONFIRMED by Patrick:** (a) HALT-governing set (`concurrency.md`, `no-function-coloring.md`, `no-runtime-mode.md`, `ide-hints.md` + the active milestone's design); (b) foundational/meta (`golden-rules.md`, vocabulary/`naming.md`, `teaching-mission.md`, `compiler-errors.md`, `versioning.md`, `feature-registry.md`, `mvp-scope.md`, `decisions.md`, `open-questions.md`) — language constitution + process, not features. The locked concurrency/backend docs still get an **audit pass** (Phase 5) to confirm they're baked to standard | needs-Patrick → resolved | Patrick (this session): "if they are rule docs they dont need moved… mainly talking about legitimate features that arnt implemented… anything concurrency/back is locked though id still like to audit" |
| 13 | Shipped-doc-locking execution model: a **research phase (Phase 5)** inventories every shipped feature + its docs and produces a **`lock-shipped-docs` roadmap** (one milestone per feature / sub-feature). Each milestone is then executed by a **fresh chat** given the rules + doc-list + specific feature (full context to research vs code + document right). The per-feature locking runs DOWNSTREAM of this plan, not inline | needs-Patrick → resolved | Patrick (this session): "fresh chat PER feature… one research phase, document how we should split up how many per chat… those docs we find can be sub phases" |
| 14 | Audit the LOCKED carve-out docs (foundational/meta + HALT-governing concurrency/backend) for standard-compliance — whys present, correct `doc-type`, present-tense, AND the right folder (`golden-rules.md`/vocabulary/`naming.md` are language law → may belong in a dedicated `docs/internal/foundations/` bucket; process docs like `mvp-scope.md`/`versioning.md`/`open-questions.md` → `docs/internal/process/`; NOT lumped in `architecture/`). Foldering decided in Phase 1; standard-audit in Phase 5; light fixes inline, substantial → roadmap milestone | needs-Patrick → resolved | Patrick (this session): "audit the rules docs too… maybe missing the whys… maybe they belong in a better folder than design" |
| 15 | The `lock-shipped-docs` roadmap **self-deletes when all its milestones reach `done`** — it's a one-time cleanup index, dead once complete. The per-feature execution plans remain archived in `plans/done/` as the forensic record | needs-Patrick → resolved | Patrick (this session): "once we fix ALL lock-shipped-docs we need to remove it because it would then be a dead file" |
| 9 | Aspirational bucket = `docs/internal/ideas/`; gate tier = `[aspirational]` (warns, non-blocking) | needs-Patrick → resolved | Patrick: "ideas folder gets the aspirational tag, design keeps locked" |
| 10 | `design-sources.md` 2026-06-13 policy ("[aspirational] tier intentionally unused; everything written is locked") is **amended** — tier un-retired for the `docs/internal/ideas/` bucket ONLY; design docs stay locked | needs-Patrick → resolved | Patrick approved un-retiring the tier for ideas |
| 11 | Doc lifecycle enforced at plan time: any feature-locking plan MUST design+lock the feature's idea-doc (convert to standard locked doc, delete/trim the idea-doc). Home: project `rules/documentation.md`; enforcement pointer in `plan-invariants.md`. A memory was rejected (fires only on recall) | needs-Patrick → resolved | Patrick: "bake it in… at time of planning part of that plan is to fix the doc, move it to a locked doc" |
| 12 | ALL shipped features have outdated, non-locked docs (user-facing AND internal) — they predate the doc rules. Converting them to locked, faithful docs is a **NOW** problem done IN THIS PLAN (a dedicated phase), NOT deferred. **No "split if large" escape** — deferring shipped-doc debt is the laziness `no-duct-tape.md` forbids. May span multiple sessions; all of it gets done (up to m3) | needs-Patrick → resolved | Patrick (this session): "those all need fixed. We cant just 'not' do it because we are lazy… that is a now problem… up to m3" |
| 6 | **Sequencing**: write the FULL plan (move + everything); gate execution on m3d-done via Precondition + `depends_on` | needs-Patrick → resolved | Patrick (this session): "just put that as an active requirement up top… then just do the FULL plan." |
| 7 | **Diátaxis depth**: tag dominant mode now; defer strict one-doc-one-mode split to per-feature lock-time (encoded in the project rule) | needs-Patrick → resolved | **Recommended by Claude; OVERRIDES Patrick's stated lean toward full-split.** Reasoning: full-fragmenting aspirational drafts is build-twice (`no-duct-tape.md` #11) + you can't author faithful `reference` for unbuilt features. **Flagged for Patrick to flip at approval** (see Questions). |
| 8 | Project `docs-checklist.md` predates + is out of sync with the three global doc rules | verified | `docs-checklist.md` describes only `spec/`+`design/`, no `docs/`, no doc-type, no living-doc/ADR rule, no §g citation |

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Crate-comment rewrites collide with m3d's active crate edits | High (if run early) | Merge conflicts; derails m3d | **Execution Precondition gate** — blocked until m3d `status: done`; first execution step verifies and STOPs/warns otherwise |
| Dangling `design/`/`spec/` refs left after partial move | Med | Broken doc links; gate mis-resolves | Phase 2 does move + repo-wide ref rewrite **atomically**; Phase 7 grep asserts **zero** remaining `design/`/`spec/` path refs |
| `design-sources.md` globs not updated in lockstep with move → locked-doc gate silently matches nothing (zero-match loud-warning) | Med | Design-compliance gate goes blind | Update globs to `docs/**` in the **same phase** as the move (Phase 2); Phase 7 confirms gate resolves to real files |
| Content lost / silently altered during ADR extraction or present-tense rewrite | Med | Design rationale destroyed | Extraction is **move-not-delete**; reviewer diffs for content preservation; **verify any migrated claim against actual code** before locking (`verification.md`) |
| Aspirational idea-dump docs get presented as faithful/locked reference | Med | Readers trust unbuilt-feature specs as shipped behavior | Move is structural; preserve content verbatim; doc-type tag reflects **mode, not maturity**; do not upgrade authority |
| Migrating a stale claim out of a done-plan as if it's current truth | Low-Med | Doc asserts something the code doesn't do | Done-plan sift (Phase 7) **verifies each claim against code** before migrating; unverifiable claims are dropped, not copied |
| Over-fragmenting the spec (full Diátaxis) explodes 32→60+ files for aspirational content | Med | Build-twice waste | Decision #7 RESOLVED: tag-now, split-at-lock |
| Shipped-doc locking is large — every shipped feature predates the doc rules, so all have non-locked user+internal docs | High | Multi-session effort | NOT deferred — Phase 5 structures it into a `lock-shipped-docs` roadmap (per-feature fresh-chat milestones, Decision #13); executed downstream, all done, none punted |
| Maturity misclassification — an aspirational doc tagged shipped-locked (or vice-versa) | Med | Idea-doc treated as committed reference, or shipped spec left replaceable | Classification verified against milestone state + code (mvp-scope, done-plans), not vibes; ambiguous → default to `[aspirational]` (safer — never over-commits) |
| Plan is now 7 phases (added shipped-doc inventory) | Low | Review fatigue | Each phase leaves a consistent repo + is independently reviewable; the heavy locking work is offloaded to the `lock-shipped-docs` roadmap (fresh chats), keeping THIS plan's phases bounded; not milestoned because a half-restructure is worse than none |

## Questions

1. **Diátaxis depth (Decision #7) — RESOLVED: Option A** (tag dominant mode now; strict per-mode split deferred to feature-lock, encoded in the project rule). Approved by Patrick.
2. **Internal sub-layout mapping** — Phase 1 produces the exact `design/*` → `docs/internal/…` per-file mapping table (architecture vs future vs stdlib vs the decisions index) for your review before Phase 2 executes any `git mv`. Flagging that the mapping is a Phase-1 deliverable, not pre-decided here.
3. **Classification — RESOLVED to shipped-vs-not** (Decisions #5, #9–#12): shipped feature → `[locked]` (fix NOW); not-shipped (incl. `future/`, `stdlib/`, unplanned spec+design) → `docs/internal/ideas/` `[aspirational]`; 2026-06-13 policy amended to un-retire `[aspirational]` for `ideas/**` only; shipped-doc fixing is a dedicated NOW phase, not deferred; convert-at-lock lifecycle enforced via `plan-invariants.md`.
4. **Two carve-outs — RESOLVED: both stay `[locked]`** (Patrick confirmed): the HALT-governing set + foundational/meta docs are not features and don't move to `ideas/`. The concurrency/backend locked docs additionally get an audit pass (Phase 5) to confirm they're baked to standard.
5. **Shipped-doc-locking representation — RESOLVED** (Decision #13): the per-feature locking is a `lock-shipped-docs` **roadmap** produced by Phase 5; each feature = one milestone = one fresh chat. If you'd rather it be strict in-plan sub-phases instead of a roadmap, say so — but roadmap milestones ARE the framework's "fresh chat per unit" tool.

## Risk Assessment & Rollout Strategy

**Risk level: LOW** (docs/comments only — no executable behavior, no data, no auth, no money; mechanical + reversible via git).

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | docs + comments only |
| Third-party integration | No | |
| Changes existing endpoints | No | |
| Compiler behavior change | No | crate edits are **doc-comments only**, zero codegen/logic change |

**Mitigations applied**: whole thing is on a `docs/` branch, reviewable as a diff, revertable with `git`. Read-only w.r.t. program behavior.

**Rollout plan**: LOW risk → single branch, full reviewer sweep at each phase, merge via `/pr` once m3d is clear. No staged rollout (nothing runtime to ramp).

## Design Divergences

| Doc | What it says | What we do instead | Approved rationale (named cost + reversal path) |
|-----|-------------|-------------------|------------------------------------------------|
| `doc-types.md` | one-doc-one-mode (Diátaxis) for user docs | Tag dominant mode now; defer strict per-mode file split to feature-lock time | Named cost: spec files temporarily carry mixed reference+explanation under one dominant `doc-type` tag until each feature is locked. Reversal: the project rule encodes "split at lock"; the split happens per-feature when content is authored faithfully. Avoids build-twice on aspirational drafts (`no-duct-tape.md` #11). **Pending Patrick's confirmation (Question 1).** |

## Documentation Deliverables

| Deliverable | Phase | Notes |
|---|---|---|
| `.claude/rules/documentation.md` (project) — cites+extends global `documentation.md` §g; encodes Yinz layout (`docs/user`+`docs/internal`), the "Diátaxis-split-at-feature-lock" rule, retires `docs-checklist.md` | Phase 1 | doc-type: n/a (rule file) |
| `.claude/plans/roadmaps/lock-shipped-docs.md` — roadmap breaking shipped-doc locking into per-feature fresh-chat milestones | Phase 5 | drives the downstream per-feature locking chats |
| `docs/README.md` — navigation index / decision-tree (internal vs user vs doc-type buckets) | Phase 6 | doc-type: overview |
| `docs/overview.md` — top-level board-altitude project explainer | Phase 6 | doc-type: overview |
| `docs/internal/decisions/*` — ADRs extracted from the 17 dated-decision docs | Phase 4 | doc-type: adr (append-only) |
| Per-doc `doc-type:` frontmatter across all moved docs | Phase 3 | architecture / reference / explanation / etc. |

## Planned RED Repros

| What's intentionally broken | Locking RED test | Asserted contract | Fixing phase | Prod-exposure note |
|---|---|---|---|---|
| — | — | — | — | _(empty — not a bug-fix plan; no intentionally-failing tests)_ |

## Behavioral Contract

| Gate | Input shape / fixture | Outcome | Why |
|---|---|---|---|
| — | — | — | _(empty — no gate-shaped/predicate work; this is a docs move)_ |

## Phase Execution Protocol

Each phase ends with the standard **Exit Sequence** (persist plan bookkeeping → resolve `$BASE` → fan out the 5 reviewers + N deviation-judges in parallel against the resolved diff → coordinator writes Evidence + Phase Review Gates → handle BLOCKs in the fix loop → prompt commit). Canonical fan-out spec: `~/.claude/commands/execute-plan.md` Step 3.d–3.h. Final phase additionally runs the cumulative Opus reviewer sweep (Step 10f) and flips `status: active` → `done`.

**Per-phase ships via**: `/pr`. **Milestone ships via**: `/release` (this is part of v0.3 docs hygiene; no separate release tag required — folds into the next `/release`).

## Phases

### Phase 1: Project rule + target layout + scaffold
**PR scope**: Author the project `rules/documentation.md` (extends global §g), define the full `design/*`+`spec/*` → `docs/…` mapping table, scaffold empty `docs/` dirs. No content moves yet.
**Branch**: `docs/restructure-scaffold`
**Flag**: N/A
**Est. lines**: ~250 (rule file + mapping table)
**Executor tier**: standard
**Ships via**: `/pr`
**Objective**: Lock the target shape on paper so Phase 2's mechanical move is unambiguous.
**Why this phase exists**: The internal sub-layout (architecture vs future vs stdlib vs decisions) is a judgment call that must be reviewed BEFORE any `git mv` runs.
**Current-state anchors**:
- `.claude/rules/docs-checklist.md` — the stale project rule this replaces
- `~/.claude/rules/documentation.md:155` — §g project-extension mechanism to cite
- `design/decisions.md` — index-only; becomes raw material for the decisions/ADR index
**Files (expected scope)**: `.claude/rules/documentation.md` (new), `.claude/rules/plan-invariants.md` (lifecycle requirement), `.claude/design-sources.md` (policy text amend), `docs/` (new empty dirs incl. `ideas/`), this plan file (mapping table appended)
**Deviation rule**: standard (per template).
**Steps**:
1. Write `.claude/rules/documentation.md`: `Extends: ~/.claude/rules/documentation.md`. Encode: Yinz layout (`docs/user/` = user-facing locked specs, `docs/internal/architecture` + `docs/internal/decisions` = compiler, **`docs/internal/ideas/` = aspirational idea-docs**), the **Diátaxis-split-at-feature-lock** rule (Decision #7), `doc-type` + **`status: aspirational | locked`** frontmatter requirement, the **doc lifecycle** (an idea-doc in `docs/internal/ideas/` must be designed+locked when its feature is planned, then deleted — or trimmed if only some of its ideas shipped), and retirement of `docs-checklist.md` (checklists fold in, repointed). Do NOT re-spec §a–§f of the global rule — cite it.
   - Also amend `.claude/design-sources.md`'s 2026-06-13 policy text: un-retire the `[aspirational]` tier **for `docs/internal/ideas/**` only**; all `/design/` docs and shipped-feature user specs remain `[locked]`.
   - Add a `### Doc Lifecycle` enforcement requirement to `.claude/rules/plan-invariants.md`: any plan that locks a feature whose spec lives in `docs/internal/ideas/` MUST include a step that designs+locks the real doc and deletes/trims the idea-doc (plan-reviewer checks plan-invariants → mechanical enforcement).
2. Produce the **per-file mapping table** with a **maturity column**, classifying every `spec/*` AND `design/*` doc as one of three buckets (verified against `mvp-scope.md` + done-plans + code, not guessed; ambiguous → `[aspirational]`):
   - **Shipped feature** → `[locked]` faithful doc (user spec → `docs/user/…`; design → `docs/internal/architecture/…`).
   - **Carve-out: HALT-governing** (`concurrency.md`, `no-function-coloring.md`, `no-runtime-mode.md`, `ide-hints.md` + active-milestone design) AND **foundational/meta** (`golden-rules.md`, `naming.md`/vocabulary, `teaching-mission.md`, `compiler-errors.md`, `versioning.md`, `feature-registry.md`, `mvp-scope.md`, `decisions.md`, `open-questions.md`) → `docs/internal/…` `[locked]` regardless of shipped status. **Foldering (Decision #14): decide the right sub-bucket — language law → `docs/internal/foundations/`, process docs → `docs/internal/process/` — NOT a blanket dump into `architecture/`.**
   - **Not shipped** (incl. `design/future/*`, `design/stdlib/*`, unplanned spec + design) → `docs/internal/ideas/…` `[aspirational]`, moved as-is.
   **Mixed-doc tiebreaker (plan-reviewer adversarial #1):** a doc that's part shipped / part unbuilt (e.g. `concurrency.md` — `wait`/`background` shipped, channels aspirational) routes by MAJORITY, carving the other part to a sibling; for HALT-governing docs the governing core stays `[locked]` and only the unbuilt sections carve to `ideas/`. Splitting beats a single mixed-status doc. Append the table to this plan under `## Layout Mapping`.
3. `mkdir` the `docs/` tree skeleton (empty dirs + `.gitkeep` where needed).
4. Retire `docs-checklist.md`: **delete it** and repoint its inbound refs to the new project rule in Phase 2's sweep. No stub — a stub is a second home for the same concern (Rule 00).
**Acceptance criteria**:
- [ ] `.claude/rules/documentation.md` exists, cites global rule, encodes Yinz layout + `ideas/` bucket + split-at-lock rule + doc-type + `status` requirement + the doc lifecycle
  - Evidence: (filled at phase completion)
- [ ] `plan-invariants.md` carries a `### Doc Lifecycle` requirement; `design-sources.md` policy text amended to un-retire `[aspirational]` for `docs/internal/ideas/` only
  - Evidence: (filled at phase completion)
- [ ] `## Layout Mapping` table covers every current `design/*` and `spec/*` file with a destination AND a maturity tag (`[locked]`/`[aspirational]`), classification verified against milestone/code state
  - Evidence: (filled at phase completion)
- [ ] `docs/` skeleton dirs exist (incl. `docs/internal/ideas/`); no content moved yet
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No design/spec content moved or deleted in this phase
- [ ] Project rule does not duplicate global §a–§f (pointer only)
**Verification**: `ls -R docs/`; read the new rule; confirm mapping table is exhaustive vs `ls design spec`.
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

### Phase 2: Atomic move + repo-wide reference rewrite
**PR scope**: `git mv` all `spec/` → `docs/user/` and `design/` → `docs/internal/…` per Phase 1's mapping, AND rewrite every `design/`/`spec/` path reference repo-wide (internal links, rules, `CLAUDE.md`, 55 crate doc-comments, `examples/`, `registry/`, active plans, `design-sources.md` globs). Pure path rewrite — zero content/tag/prose change.
**Branch**: `docs/restructure-move`
**Flag**: N/A
**Est. lines**: large but **mechanical** (path-only find/replace across ~120 files)
**Executor tier**: mechanical
**Ships via**: `/pr`
**Objective**: Relocate the trees and repoint all refs in one atomic move so the repo is never left with dangling refs.
**Why this phase exists**: A partial move leaves broken links; bundling move+refs keeps every intermediate state consistent.
**Current-state anchors**:
- `.claude/design-sources.md` — `[locked] design/**/*.md` + `[locked] spec/**/*.md` globs → become `docs/**`
- `crates/**` — 55 files with `design/…md` doc-comment refs
- `CLAUDE.md` Project Layout table — references `/spec/` and `/design/`
**Files (expected scope)**: `git mv` of `spec/**`+`design/**`; ref edits in `crates/**`, `.claude/rules/**`, `CLAUDE.md`, `examples/**`, `registry/**`, `.claude/plans/active/**`, `.claude/design-sources.md`, and the moved docs' internal links.
**Deviation rule**: standard. **Do NOT touch done/paused plans for link-fixing** (scope note); content sift of done-plans is Phase 7.
**Steps**:
1. `git mv` each file per the Phase 1 mapping table (preserves history).
2. `git mv` + repo-wide path rewrite driven **key-by-key off the Phase 1 mapping table** (each doc → its classified destination: shipped/HALT-governing/foundational → `docs/user/` or `docs/internal/architecture` `[locked]`; not-shipped incl. `design/future/*`, `design/stdlib/*`, unplanned → `docs/internal/ideas/` `[aspirational]`, moved as-is, no content rewrite). Apply path rewrites to crate comments, rules, CLAUDE.md, examples, registry, **active** plans, and moved-doc internal links. Skip `plans/done/` + `plans/paused/`.
3. Update `.claude/design-sources.md`: replace `design/**` + `spec/**` globs with `[locked] docs/internal/architecture/**`, `[locked] docs/internal/decisions/**`, `[locked] docs/user/**`, and **`[aspirational] docs/internal/ideas/**`**; repoint the high-stakes per-doc callouts (concurrency, no-function-coloring, no-runtime-mode, ide-hints).
4. Update `CLAUDE.md` Project Layout table rows to the `docs/` paths.
**Acceptance criteria**:
- [ ] `spec/` and `design/` dirs no longer exist; content lives under `docs/`
  - Evidence: (filled at phase completion)
- [ ] `grep -rIE '(design|spec)/[a-z0-9-]+\.md' . --include='*.md' --include='*.rs'` over tracked files (excluding `plans/done` + `plans/paused`) returns **zero** matches. **NOTE — use bare `|` for ERE alternation, NOT `\|` (which matches a literal pipe and falsely reports zero).**
  - Evidence: (filled at phase completion)
- [ ] `design-sources.md` globs point at `docs/**` and match real files (no zero-match loud-warning)
  - Evidence: (filled at phase completion)
- [ ] `cargo build --workspace` still succeeds (comment-only edits, no logic change)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No prose/content change — diff is path-string + `git mv` only
- [ ] No `doc-type` tags added yet (that's Phase 3)
- [ ] done/paused plans untouched
- [ ] The `[aspirational] docs/internal/ideas/**` glob and the file-move into `ideas/` land in **one commit** (avoids the Sentinel-Guard zero-match loud warning on an empty-bucket intermediate state)
- [ ] Old `spec/**` + `design/**` globs are fully **removed** from `design-sources.md` (not just supplemented) — a leftover glob emits a permanent zero-match warning once the dirs are gone (plan-reviewer adversarial)
**Verification**: the zero-dangling-ref grep; `cargo build --workspace`; spot-read 3 crate files + 3 moved docs for correct new paths. **Adversarial checks (plan-reviewer):** (a) nested-path correctness — confirm `design/future/gui/architecture.md` landed at the full nested dest, NOT a flattened `…/future.md`; the rewrite is driven key-by-key off the Phase 1 mapping table, never a blanket regex; (b) cross-link target resolution — trace at least one chain end-to-end (e.g. `decisions.md` → `concurrency.md`) and confirm the rewritten link resolves to the target's NEW home.
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

### Phase 3: doc-type frontmatter tagging + dominant-mode classification
**PR scope**: Add a `doc-type:` frontmatter tag to every moved doc, classifying internal docs (architecture / adr-index / overview) and user docs by **dominant** Diátaxis mode (mostly `reference`, some `explanation`). No file fragmentation (Decision #7).
**Branch**: `docs/restructure-tagging`
**Flag**: N/A
**Est. lines**: ~80 (frontmatter only, ~79 docs)
**Executor tier**: standard
**Ships via**: `/pr`
**Objective**: Make every doc self-identify its type so the right litmus + the index buckets apply.
**Why this phase exists**: doc-type is the discriminator the new standard + LSP + acceptance-verifier rely on; it's per-file judgment, kept separate from the mechanical move.
**Current-state anchors**: `~/.claude/memory/doc-types.md` — the type list + per-type litmus
**Files (expected scope)**: frontmatter of every file under `docs/`.
**Deviation rule**: standard.
**Steps**:
1. Tag internal architecture docs `doc-type: architecture`; the decisions index appropriately; `docs/internal/architecture/future/*` likewise architecture (design intent).
2. Tag user docs (`docs/user/`) by dominant mode — default `reference`; `explanation` where mostly "why/mental-model". Tag `docs/internal/ideas/` docs by dominant mode too (`doc-type` reflects **mode, not maturity**).
3. Add a `status:` frontmatter field to every doc: `locked` for `docs/user/` + `docs/internal/architecture` + `docs/internal/decisions`; `aspirational` for `docs/internal/ideas/`.
4. Where a single doc is genuinely two modes fighting, flag it in the PR for a follow-up split (do NOT split here unless trivial) — captured as a deviation if acted on.
**Acceptance criteria**:
- [ ] Every doc under `docs/` has a `doc-type:` frontmatter field with a valid value
  - Evidence: (filled at phase completion)
- [ ] Every doc has a `status:` field (`locked`/`aspirational`); every `docs/internal/ideas/` doc is `aspirational`, all others `locked`
  - Evidence: (filled at phase completion)
- [ ] Tags reflect dominant mode; no file fragmentation occurred (Decision #7)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No content rewrite — frontmatter only
- [ ] Aspirational specs tagged by mode, not downgraded/upgraded in authority
**Verification**: `grep -rL '^doc-type:' docs/ --include='*.md'` returns empty (every doc tagged).
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

### Phase 4: ADR extraction + present-tense hygiene
**PR scope**: Extract the dated/`LOCKED`/`Design Divergences` decision sections from the 17 candidate docs into append-only `docs/internal/decisions/*` ADRs; rewrite the source sections present-tense and link doc→ADR. Verify any decision claim against actual code before locking it into an ADR.
**Branch**: `docs/restructure-adrs`
**Flag**: N/A
**Est. lines**: ~600 (ADR files + present-tense edits)
**Executor tier**: complex
**Ships via**: `/pr`
**Objective**: Kill the garden-path pattern — docs hold current truth; dated decision history lives in append-only ADRs.
**Why this phase exists**: This is the fix for "decisions buried 10 deep, I stop reading at the first." High-care: must not lose rationale, must not assert unverified claims.
**Sizing note (plan-reviewer)**: ~600 lines across 17 docs is the real review-fatigue risk. Extract+verify in **batches** (e.g. ~5 docs at a time) with the executor self-reviewing each batch before the phase-boundary reviewer pass, rather than one monolithic 600-line diff.
**Current-state anchors**:
- `design/concurrency.md` → `docs/internal/architecture/concurrency.md` — `LOCKED 2026-06-05` headings + `## Design Divergences (M3b)` section
- the 17 ADR-candidate docs listed in Research Findings
- `~/.claude/memory/doc-types.md` ADR section — must-capture: context, decision, **rejected alternatives**, consequences + cost-to-reverse, status+date
**Files (expected scope)**: `docs/internal/decisions/*` (new ADRs), present-tense edits to the 17 source docs.
**Deviation rule**: standard. Any claim sourced from a done-plan or aspirational doc gets **verified against code** before it enters an ADR as fact (`verification.md`); unverifiable → record as "asserted, unverified" or drop, never launder into a fact.
**Steps**:
1. For each dated decision section: create an ADR (context/forces, decision, alternatives + why rejected, consequences + cost-to-reverse, status, date). ADRs are append-only.
2. Rewrite the source doc section present-tense ("Yinz does Y because X causes <problem>"), strip "LOCKED <date>" / "we used to" framing, link to the ADR for history.
3. Verify each migrated decision against the code it claims to describe; flag any that the code contradicts (do not silently "fix" the doc to match a guess).
**Acceptance criteria**:
- [ ] Each of the 17 candidate docs' dated-decision sections is either extracted to an ADR or justified as not-a-decision
  - Evidence: (filled at phase completion)
- [ ] Source docs read present-tense; no `LOCKED <date>`/"we used to"/"previously" garden-path framing remains
  - Evidence: (filled at phase completion)
- [ ] Each ADR carries context + decision + **rejected alternatives** + consequences + cost-to-reverse + date (doc-types.md ADR litmus)
  - Evidence: (filled at phase completion)
- [ ] Every migrated decision-claim is verified against code, or explicitly marked unverified
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No rationale lost — extraction preserved the WHY + alternatives
- [ ] No unverified claim asserted as fact
- [ ] ADRs are append-only (no in-place edits to an accepted ADR)
**Verification**: `grep -rE 'LOCKED [0-9]{4}|we used to|previously' docs/internal/architecture/` returns empty (or justified); read 3 ADRs against their litmus.
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

### Phase 5: Shipped-Doc Inventory + Locking Roadmap (research)
**PR scope**: Inventory every shipped feature and its user + internal docs; audit ALL locked carve-out docs (concurrency/backend + foundational/meta) for standard-compliance; produce a **self-deleting** `lock-shipped-docs` roadmap that breaks the locking into per-feature fresh-chat units. Produces the PLAN for locking, not the locked docs.
**Branch**: `docs/restructure-shipped-inventory`
**Flag**: N/A
**Est. lines**: ~400 (roadmap + inventory)
**Executor tier**: complex
**Ships via**: `/pr`
**Objective**: Structure the shipped-doc debt (every shipped feature predates the doc rules → non-locked docs) into fresh-chat-per-feature units, ordered and instruction-templated. NOT deferral — structured execution (Decision #12, #13).
**Why this phase exists**: The shipped-doc set is too large to lock well in one chat; Patrick's model is a fresh chat per feature with full context to research vs code. This phase produces the breakdown those chats run from.
**Current-state anchors**:
- `design/mvp-scope.md` — source of truth for "is feature X in v0.N / shipped?"
- `plans/done/**` — the executed milestones whose features shipped
- the HALT-governing locked docs (`concurrency.md`, `no-function-coloring.md`, `no-runtime-mode.md`, `ide-hints.md`) — audited here
**Files (expected scope)**: `.claude/plans/roadmaps/lock-shipped-docs.md` (new), this plan (inventory appended).
**Deviation rule**: standard.
**Steps**:
1. From `mvp-scope.md` + `plans/done/` + code, enumerate every SHIPPED feature; map each to its user spec + internal design doc(s).
2. Audit ALL locked carve-out docs against the standard (Decision #14): HALT-governing concurrency/backend docs vs code (baked-properly check) + foundational/meta docs (whys present? correct `doc-type`? present-tense? right folder per Phase 1?). List gaps; light fixes inline, substantial → a roadmap milestone.
3. Group the locking work into per-feature (or per-sub-feature, if big) units sized for one fresh chat; set ordering + docs-per-chat.
4. Write `.claude/plans/roadmaps/lock-shipped-docs.md`: one milestone per feature-group + a per-chat instruction template (rules to load, the doc list, the specific feature, verify-vs-code requirement, the locked-doc standard). The roadmap's front-matter/closing note declares it **self-deletes when all milestones reach `done`** (Decision #15 — one-time cleanup index; per-feature done-plans remain the record).
**Acceptance criteria**:
- [ ] Every shipped feature mapped to its user + internal docs; cross-checked vs `mvp-scope.md` + done-plans (none missed)
  - Evidence: (filled at phase completion)
- [ ] ALL locked carve-out docs audited vs standard (concurrency/backend vs code + foundational/meta for whys/doc-type/present-tense/folder); gaps listed (or "none")
  - Evidence: (filled at phase completion)
- [ ] `lock-shipped-docs` roadmap exists: per-feature milestones + ordering + fresh-chat instruction template + the self-delete-when-complete declaration (Decision #15)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Inventory verified against mvp-scope + code, not guessed
- [ ] No shipped feature silently omitted from the roadmap
- [ ] This phase produces the PLAN only — no shipped docs locked inline (that's the downstream fresh chats)
**Verification**: read the roadmap — could a fresh chat lock one feature's docs from rules + doc-list + feature alone?
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

### Phase 6: Navigation index + overview
**PR scope**: Write `docs/README.md` (the decision-tree / map: internal vs user, doc-type buckets, "start here") and `docs/overview.md` (board-altitude project explainer). Fold `design/decisions.md` + `spec/overview.md` content into the new index/overview as appropriate.
**Branch**: `docs/restructure-index`
**Flag**: N/A
**Est. lines**: ~300
**Executor tier**: standard
**Ships via**: `/pr`
**Objective**: Deliver the "I can tell what's what and where to start" fix.
**Why this phase exists**: The decision-tree is the single highest-leverage fix for Patrick's "idk which is internal vs user vs algo."
**Current-state anchors**: `design/decisions.md` (→ moved), `spec/overview.md` (→ moved), `~/.claude/memory/doc-style.md` (house style for the index/overview).
**Files (expected scope)**: `docs/README.md` (new), `docs/overview.md` (new); de-dupe the old index files.
**Deviation rule**: standard.
**Steps**:
1. Write `docs/README.md`: TL;DR table, the internal/user/doc-type map, a "start here" decision-tree, links to the ADR index. Follow `doc-style.md` (progressive disclosure, present-tense).
2. Write `docs/overview.md`: non-technical board-altitude explainer of Yinz.
3. Repoint/retire the old `decisions.md` + `overview.md` so there's one index home (Rule 00).
**Acceptance criteria**:
- [ ] `docs/README.md` labels internal vs user + doc-type buckets and has a "start here" path
  - Evidence: (filled at phase completion)
- [ ] `docs/overview.md` is board-altitude, no code/file-paths, understandable from TL;DR alone
  - Evidence: (filled at phase completion)
- [ ] No duplicate index — old `decisions.md`/`overview.md` folded or repointed (one home)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Index follows doc-style (TL;DR, present-tense, links-the-topic)
- [ ] No concern scattered across two index files
**Verification**: read `docs/README.md` — can a newcomer find internal vs user vs an ADR in <2 hops?
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

### Phase 7: Done-plan knowledge sift + final verification sweep
**PR scope**: (a) Confirm the Phase 5 `lock-shipped-docs` roadmap covers every shipped feature (completeness check — the conversion runs downstream as fresh chats, not here); (b) read-only sift of `plans/done/` for un-migrated durable knowledge, migrate only what's **verified against code**; (c) final verification sweep (zero dangling refs, full doc-type + `status` coverage, design-sources gate resolves incl. the `[aspirational]` glob, ADR present-tense clean).
**Branch**: `docs/restructure-verify`
**Flag**: N/A
**Est. lines**: ~200 (migrations) + verification
**Executor tier**: standard
**Ships via**: `/pr`
**Objective**: Catch durable knowledge stranded in done-plans (e.g. the m3d auto-parallelization algorithm) and confirm the whole restructure is internally consistent.
**Why this phase exists**: Done-plans are archived and forgotten; the algorithm-internals knowledge Patrick worried about lives there. But a done-plan's claim is a theory until code confirms it.
**Current-state anchors**: `plans/done/**` (sift targets), `crates/**` (the code to verify claims against).
**Files (expected scope)**: new/updated `docs/internal/architecture/*` (e.g. an `auto-parallelization-internals.md` if the m3d knowledge warrants it), verification output.
**Deviation rule**: standard. **Migrate a done-plan claim ONLY after verifying it against the actual code** (`verification.md`); unverifiable claims are not migrated.
**Steps**:
1. **Completeness check (not conversion):** confirm the Phase 5 `lock-shipped-docs` roadmap covers every shipped feature with no omissions — cross-check its milestone list against `mvp-scope.md` + `plans/done/`. The actual per-feature locking runs downstream as fresh chats; this step only verifies the roadmap is complete. (The fix-vs-route + verify-vs-code discipline lives in the roadmap's per-chat instruction template, not here.)
2. Sift `plans/done/` for durable doc-worthy content not already in a design doc (algorithms, locked decisions, dead-ends); verify each against code before writing it into a doc/ADR. Drop or mark-unverified anything the code doesn't confirm.
3. Final sweep: zero dangling `design/`/`spec/` refs (bare `|` ERE); every `docs/` file has `doc-type` + `status`; `design-sources.md` globs (incl. `[aspirational] docs/internal/ideas/**`) resolve to real files; no garden-path framing left; `cargo build --workspace` green.
**Acceptance criteria**:
- [ ] Phase 5 `lock-shipped-docs` roadmap covers every shipped feature — no omissions vs `mvp-scope.md` + `plans/done/`
  - Evidence: (filled at phase completion)
- [ ] `plans/done/` sifted; durable un-migrated content identified and (where verified) migrated
  - Evidence: (filled at phase completion)
- [ ] Every migrated claim cites the code that confirms it; unverified claims dropped or marked
  - Evidence: (filled at phase completion)
- [ ] Zero dangling `design/`/`spec/` path refs repo-wide (excluding done/paused plans)
  - Evidence: (filled at phase completion)
- [ ] `design-sources.md` globs resolve to real files; design-compliance gate runs clean
  - Evidence: (filled at phase completion)
- [ ] Each `design-sources.md` **per-doc callout** (`concurrency.md`, `no-function-coloring.md`, `no-runtime-mode.md`, `ide-hints.md`) resolves independently to its new path — not just the glob (plan-reviewer: a glob can match while a stale named callout points at a dead path)
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No unverified done-plan claim laundered into a doc as fact
- [ ] Verification greps all return clean
**Verification**: the dangling-ref grep; `grep -rL '^doc-type:' docs/`; design-sources resolution check; `cargo build --workspace`.
**Phase Review Gates**:
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>
**Findings Log**: _(empty until a reviewer returns BLOCK)_

## Documentation Deliverables
_(see the `## Documentation Deliverables` table above — this plan IS doc work; per-phase doc-ACs live in their phases.)_

## Quality Checklist (verify at completion)
- [ ] Inputs validated — N/A (docs/comments only, no user input)
- [ ] Auth/authz — N/A
- [ ] Error handling — N/A (no runtime behavior)
- [ ] Injection/XSS/secrets — N/A
- [ ] Performance/N+1 — N/A
- [ ] `cargo build --workspace` still green after crate-comment edits (Phase 2, Phase 7)
- [ ] Zero dangling `design/`/`spec/` refs (Phase 7)
- [ ] Every `docs/` file has a `doc-type` + `status` (Phase 3, Phase 7)
- [ ] `design-sources.md` globs (incl. `[aspirational] ideas/**`) resolve to real files (Phase 2, Phase 7)
- [ ] No garden-path/dated framing left in moved docs (Phase 4, Phase 7)
- [ ] `lock-shipped-docs` roadmap covers every shipped feature (Phase 5, completeness-checked Phase 7)
- [ ] Every phase received all-reviewer + all-judge PASS before commit (Step 9a)
- [ ] Final cumulative reviewer sweep passed (Step 10f)
- [ ] m3d was `status: done` before execution began (Execution Precondition)

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase = one PR via `/pr`; 7 phases = 7 PRs.
- **Shadow main branches**: each phase branches off the prior phase's committed tip; no long-lived shadow branch.
- **Building the engine before shipping value**: not milestoned because a half-restructure is worse than none — but phases are ordered so the highest-pain fixes (move consistency P2, ADR de-burial P4, index P6) each land reviewable; the index (P6) delivers the navigation value users feel. The shipped-doc locking (P5 → `lock-shipped-docs` roadmap) is structured into per-feature fresh-chat units, not deferred or punted (Decision #12/#13).
- **Hotfix that isn't**: N/A — no hotfix; this is gated, deliberate, blocked on m3d.
- **Abandoned branches**: gated on m3d so it can't half-start; each phase merges before the next begins.
- **Flag graveyards**: N/A — no feature flags (docs change, nothing to flag).
