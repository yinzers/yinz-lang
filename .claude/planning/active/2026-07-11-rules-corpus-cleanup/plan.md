---
name: "rules-corpus-cleanup"
plan-id: "2026-07-11-rules-corpus-cleanup"
status: "active"
roadmap-id: null
session-id: ["3967a266-28a7-4313-88d9-52d2becad7d8", "7db1f615-0a90-4130-8aee-8513add883d8", "04e22a51-80c1-4b67-a886-083784d61bcd"]
created_at: "2026-07-11"
updated_at: "2026-07-16"
metadata:
  type: "plan"
---

# PLAN: rules-corpus-cleanup

## 1. Situation

- **Terrain (landscape):** the project rule corpus — the 15 files in
  [`.claude/rules/`](../../../rules/) plus [`CLAUDE.md`](../../../../CLAUDE.md) as their always-loaded
  summary layer. The two 2026-07-11 audits
  ([`SCRATCH-rules-audit-2026-07-11.md`](../../../../docs/internal/scratchpad/SCRATCH-rules-audit-2026-07-11.md),
  [`SCRATCH-teaching-audit-2026-07-11.md`](../../../../docs/internal/scratchpad/SCRATCH-teaching-audit-2026-07-11.md))
  are the authoritative findings inventory (Classes 1–6 with file:line receipts). The corpus content
  is strong but carries fossils: some always-loaded rules teach a **pre-lock design the rest of the
  corpus explicitly bans**, and the sibling teaching audit traced the drift downstream into registry
  templates and hover text. This is a **docs-only** change set — no `crates/**` touched.
- **Weather (external constraints):** no markdown link-checker and no docs CI exist — link-integrity
  and content-parity are verified by **manual/scripted grep inside the plan**, not by a pipeline.
  `jargon_audit.rs` does **not** read rule files (it only names `vocabulary.md` in failure-message
  strings). The `plan-lifecycle.py index` hook fires on writes under `.claude/planning/` (expected,
  benign). No deadline. The docs-only branch **forks from `main`**, not the current
  `feat/v0-3-m6-concurrency-hotfix` branch.
- **Friendly forces:** this is **plan 1 of 2**. The follow-up teaching-remediation plan is *blocked
  on* the consolidated teaching-surfaces rule this plan authors (teaching audit §H2.7). A separate
  future effort (Class-4 `paths:` load-scoping) is split out behind a `prove-before-optimize`
  falsification run — **out of scope here**. The `m8-concurrency-completion` plan's Phase 1 will add
  a channel-close term to `vocabulary.md`, which this plan rewrites (see Risk #9 and Future
  Requirements).
- **Assumptions:**
  - *(verified)* Only [`vue-website.md`](../../../rules/vue-website.md) carries `paths:` frontmatter
    (glob `website/**`); it is a deltas-only anomaly and its frontmatter shape is **not** the model
    to copy — the descriptive global frontmatter shape is. Confirmed by reading the file.
  - *(verified)* [`IMP-maybe.md`](../../../../docs/internal/implementation/IMP-maybe.md):150 records a
    deliberate split — `maybe T` as user-facing prose terminology, `maybe<T>` as locked syntax since
    M5. The reconcile is a wording-alignment, not a design change.
  - *(verified)* The canonical [`REF-golden-rules.md`](../../../../docs/reference/REF-golden-rules.md)
    itself says "12 rules" (frontmatter description line 3, intro line 16, cross-cutting line 22) and
    lists Rule 13 out of canonical order — so the count/ordering fix must include this file, not just
    the rule corpus.
  - *(verified)* The five inbound links to `naming.md` needing repoint after the merge:
    [`CLAUDE.md`](../../../../CLAUDE.md):56, [`docs-checklist.md`](../../../rules/docs-checklist.md):78,
    [`REF-naming.md`](../../../../docs/reference/REF-naming.md):16,
    [`SCRATCH-stdlib-encoding.md`](../../../../docs/internal/scratchpad/SCRATCH-stdlib-encoding.md):62,
    and `vocabulary.md`:163 (self-ref, dies in the merge). Historical `done/` plans are **not**
    touched (archived history).
  - *(unverified)* No inbound link to `naming.md` exists outside the enumerated set + `done/`
    archive. **Verify** with a tree-wide grep at the top of Phase 3 before deleting the file.
  - *(unverified)* `IMP-maybe.md`'s current wording needs no edit for the three surfaces to agree.
    **Verify** during the Phase 2 reconcile step; if a deeper conflict surfaces, raise it as a plan
    question (design-doc-wins per [`CLAUDE.md`](../../../../CLAUDE.md)), do not silently overwrite.

- **Risk Assessment:** scored via the global risk engine (`REF-risk-engine.md` — named, not linked,
  per the home-absolute-link ban). Domain: docs (internal tooling). Every edit is git-reversible → severity floors at
  **III Marginal** (reversible with effort), no Floor-B policy class applies (no security / PII /
  money / irreversible-op). No HIGH residual, no override; **two recorded MEDIUMs (#5, #9)**.
  **Risk-ID legend:** the IDs are non-contiguous because they follow the recon hazard-sweep's
  Enemy-candidate numbering; the unlisted candidate IDs (#3, #4, #7, #8, #10) resolved to **LOW** on
  the matrix without any plan-level control and are omitted from the table (nothing to mitigate,
  nothing to record).

  | Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
  |------|------|-----|---------|----------------------|----------|------|
  | #1 Merge drops a binding sentence (naming.md → vocabulary.md) | B | III | M | (i) mechanical content-parity inventory: every row/heading/normative sentence enumerated pre-merge, checked off post-merge — artifact gate that blocks completion (B2, prob −1); (ii) blocking reviewer word-for-word negation-diff on trimmed boundary sentences (B2, prob −1) | **L** | pass |
  | #2 Orphaned inbound links after merge/delete | C | III | M | grep-based link-integrity sweep as phase acceptance criterion — `naming.md` returns zero live refs tree-wide post-merge (B2, prob −1) | **L** | pass |
  | #5 Teaching-surfaces rule bloats (context weight) | C | III | M | soft line-budget target (≤120 lines) surfaced via the §3.4 CCIR — a written soft target that needs a human to weigh the budget-vs-completeness tradeoff and surface it rather than silently overrun or cut a §H2.7 item; moves no axis (**B3 — human vigilance, 0, gate-only**) | **M** | **RECORD** |
  | #6 Load-scoping smuggled in (`paths:` key added) | C | III | M | explicit acceptance criterion "no `paths:` key added to any file this plan touches (vue-website.md excepted)"; mechanical grep + reviewer block (B2, prob −1) | **L** | pass |
  | #9 m8-collision on vocabulary.md | B | III | M | B3-only: human sequencing — land this cleanup before m8 Phase 1 executes, or reconcile at merge. Human vigilance moves no axis (gate-only). | **M** | **RECORD** |
  | #11 maybe-reconcile silently overwrites a design doc | C | III | M | mandatory reconcile STEP (read `IMP-maybe.md`:150, apply its documented resolution) wired into Phase 2's exit criteria as a hard STOP that blocks phase advance until reconciled — an **engineered blocking gate** (bounded action, checkable phase-exit condition) that *survives forgetting* because it lives in the plan's exit criteria, unlike #5's soft judgment-based target: **B2, prob −1**. The "surface any deeper conflict, never overwrite" clause + Patrick's approval sign-off sit on top as the B3 gate layer (not counted in the score arithmetic). | **L** | pass |

  **Both recorded MEDIUMs (#5 line-budget, #9 m8-collision)** park in Future Requirements with their
  triggers. No RISK OVERRIDE block is drawn — no residual reached HIGH / EX-HIGH.

### Mandatory cross-cutting factor sweep

Output scales: an inapplicable factor collapses to "N/A — why". For this docs-only change most
collapse; the applicable ones fold into the risk table and phases above.

- **docs** — the entire deliverable; global-standard conformance is the mission (Phase 4).
- **reusability / DRY** — central: one-home-referenced de-dup (Class 2) is the core work (Phase 3).
- **perf / BigO** — N/A at runtime (no code); the sole analog is always-loaded context weight,
  addressed by the teaching-surfaces line budget (Risk #5).
- **race / TOCTOU** — the concurrency analog is two workstreams editing `vocabulary.md`; addressed
  as Risk #9.
- **resource-cleanup** — analog is dead-file cleanup: `naming.md` deleted, zero orphan links
  (Phase 3 + Phase 5 grep).
- **idempotency** — every phase is git-reversible and re-runnable without compounding damage; no
  special guard needed. Addressed by construction.
- **type-safety** — N/A (no code); the analog is YAML frontmatter validity, checked in Phase 5.
- **security · PII / privacy · compliance · SEO · accessibility · error-handling · observability**
  — N/A — internal tooling docs, no code, no external users, no UI, no runtime.
- **CIVIL / external-user impact** — N/A — internal tooling docs, no external users.

### Plan-invariants applicability (docs-only)

Per [`plan-invariants.md`](../../../rules/plan-invariants.md), the M4+ milestone-invariant subsections
apply to **compiler** milestones. This plan touches no `crates/**`, so:
`### Demo & Error Gallery` — **N/A** (no executable surface). `### Feature Registry Entries` —
**N/A** (no language feature). `### Kernel-Mode Behavior` / `### Runtime Dependencies` — **N/A** (no
runtime code). The `## Design-Doc Alignment` section (mandatory for *every* plan) is below.

### Design-Doc Alignment

- **Governing sources (findings inventory):** the two audit scratch docs
  ([`SCRATCH-rules-audit-2026-07-11.md`](../../../../docs/internal/scratchpad/SCRATCH-rules-audit-2026-07-11.md),
  [`SCRATCH-teaching-audit-2026-07-11.md`](../../../../docs/internal/scratchpad/SCRATCH-teaching-audit-2026-07-11.md)
  — §H2.7 is the spec for the new teaching-surfaces rule). These are the enumerated defect list this
  plan remediates; scratchpad docs stay gate-exempt as sources.
- **Reconcile anchor:** [`IMP-maybe.md`](../../../../docs/internal/implementation/IMP-maybe.md):150 owns
  the `maybe T` / `maybe<T>` split. The plan aligns the rule tables to it; it does not override it.
  Any divergence surfaces as a plan question (design-doc-wins).
- **Correction target:** [`REF-golden-rules.md`](../../../../docs/reference/REF-golden-rules.md) is the
  canonical golden-rule list; this plan corrects its count (12 → all/13) and Rule-13 ordering.
- **Global standard (named, not linked — home-absolute-link ban per the documentation standard):**
  the global `IMP-documentation-system.md` (frontmatter/linking/placement laws + the home-absolute
  link ban) and `REF-wording.md` (force-matching) govern every rewritten/new file. References to
  these global docs are **named in prose, never linked**.
- **Divergences:** none. This plan builds toward the corpus the rest of the rules already describe;
  it removes fossils rather than introducing a new model.
- **Boundary assumptions:** Class-4 `paths:` load-scoping is deferred to a separate
  `prove-before-optimize` effort (Patrick decision 3 — documented, not invented here). Plan 2
  (teaching remediation) depends on this plan's teaching-surfaces rule (documented dependency).
  **Class 5.4 (rule-header convention)** is out of scope per the brief — a candidate for plan 2
  (teaching remediation); recorded here as a deliberate deferral, not a dropped audit finding.

## 2. Mission

Remediate the `.claude/rules/` corpus per the two 2026-07-11 audits — fix the six Class-1
contradictions, sweep Class-3 staleness/banned-syntax, de-duplicate to one-home-referenced (merging
`naming.md` into `vocabulary.md`), author the three gap rules, and bring every rewritten/new file to
the global documentation standard — **without** changing load-scoping, **because** contradictory
always-loaded rules fork every session's output 50/50 and that fork has already drifted the compiler
and registry artifacts; severing the stale-rule → drifted-artifact chain is the point.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** One home per fact; no rule teaches the dead pre-lock design; every session loads a
corpus that agrees with itself. The executor's north star when a specific edit is ambiguous: **does
this edit make the corpus more self-consistent and closer to the shipped compiler's actual
behavior?** If yes, it serves the intent; if it introduces a second home for a fact or preserves a
fossil, it does not.

**Key outcomes / definition-of-done (the End State):**

1. All **six Class-1 contradictions** resolved (naming ownership rows, inference hover
   self-contradiction, decisions-home pointer, golden-rule count+ordering, channel Rule 4, intrinsics
   SSOT).
2. **Class-3 sweep** done — stale migration paths, banned-syntax examples, `maybe` reconcile,
   inference domains-table refresh, examples gallery phrasing, vue-website `/tmp` SSOT, graveyard
   cite-by-title — including the canonical `REF-golden-rules.md`.
3. `naming.md` **fully merged** into `vocabulary.md` (filename stays `vocabulary.md`; `naming.md`
   **deleted**); its ~5 inbound links repointed; a **content-parity checklist** proves no normative
   sentence was dropped.
4. **Three gap rules authored:** the consolidated teaching-surfaces rule (spec: teaching audit
   §H2.7, ≤~120 lines), the constants-naming ruling (camelCase; GR13 stays absolute), and the
   import-path canon (backtick-quoted, project-root-relative, no `.ynz` suffix).
5. Every rewritten/new rule file carries **global-standard descriptive YAML frontmatter** (name +
   block-scalar description + tags + created_at/updated_at/status/author/metadata) with **NO `paths:`
   key** (vue-website.md untouched).
6. `maybe` terminology **reconciled** with `IMP-maybe.md` (code examples `maybe<T>`; prose may say
   "a maybe int"; tables show `maybe<T>`) — not blind-swept.
7. Verification sweep passes: grep link-integrity (zero live `naming.md` refs), negation-diff review,
   no-`paths:` check, valid YAML frontmatter, content-parity sign-off; PR opened from a `main`-forked
   docs-only branch.

**Human decisions already made (baked in — do NOT re-open):** constants = camelCase, GR13 absolute
(decision 1); full merge, keep filename `vocabulary.md` (decision 2); `paths:`-scoping split out
(decision 3); import-path canon as stated (decision 4); `maybe` reconcile not blind-sweep (decision
5); global-standard conformance for all deliverables (decision 6).

### 3.2 Concept

Five phases follow the audit's fix-order, cheapest stop-loss first: (1) kill the live contradictions;
(2) mechanical stale/banned-syntax sweep; (3) the merge with its parity guard + link repoint; (4)
author the gap rules and run the frontmatter/conformance pass across every touched file; (5) verify
and PR. Phases are small and git-reversible; the merge (Phase 3) and the authoring pass (Phase 4) are
the two fat phases and carry checkpoint marks. Handoff between phases is the checkbox state in this
plan plus the audit sidecar.

### 3.3 Phases

#### Phase 1 — Class-1 contradiction stop-loss — **STATUS: COMPLETE** (session-id: 7db1f615-0a90-4130-8aee-8513add883d8)

- **Task + purpose:** resolve the six live contradictions that actively fork session output. Highest
  value in the plan (audit: "worth more than everything else combined"). Small, targeted edits.
- **Steps:**
  1. **[DONE] naming.md ownership rows (1.1) — cheap stop-loss.** Rewrite the four `&T→.share`, `.lend`,
     `.give`, `.copy` rows to the signature-keyword model (matching `vocabulary.md`). This file is
     deleted in Phase 3; this ~4-row edit is a deliberate cheap guard that closes the fossil-fork
     exposure window *now* rather than waiting for the merge (no-duct-tape: cheap in-scope mitigation
     for live exposure before the durable fix). Record the reasoning inline in the commit.
  2. **[DONE] inference.md hover self-contradiction (1.2).** Rewrite the canonical Hover Tooltip Format
     example so its WHAT-INSTEAD is Informational-correct: "nothing to type — the modifier lives on
     `foo`'s signature; click to jump there." Confirm it agrees with the same file's Domains table
     row (ownership-at-call-site = Informational).
     **CHECKPOINT** — the two flagship fossils (call-site `.share`/`.give`) no longer taught as live
     syntax anywhere in naming.md or inference.md; tree still consistent.
  3. **[DONE] decisions-home pointer (1.3).** In `language-design.md` §Documenting Decisions and
     `spec-writing.md`, replace the `docs/README.md` decisions-home pointer with
     `docs/internal/implementation/IMP-<feature>.md` per `docs-checklist.md`.
  4. **[DONE] golden-rule count + ordering (1.4).** Fix `REF-golden-rules.md`'s "12" count at **every**
     count-bearing site — frontmatter description (line ~3), intro (line ~16), cross-cutting "Before
     applying the 12 rules" (line ~22), AND "the 12 rules below" (line ~35): 12 → 13. Verify none
     remain via `grep -n '12' docs/reference/REF-golden-rules.md` returning no count-bearing hits.
     AND reorder Rule 13 into canonical position: **move Rule 13's section to AFTER Rule 12's** — the
     file currently lists Rule 13 (line ~150) BEFORE Rule 12 (line ~157), and canonical order is 12
     then 13, so Rule 13 moves down after Rule 12 (do NOT preserve the current inverted order).
     Change `CLAUDE.md`:55 and `language-design.md` to count-free phrasing ("all golden rules")
     everywhere except the canonical numbered list.
  5. **[DONE] channel Rule 4 (1.5).** Amend `stdlib-design.md` Rule 4 to "bounded always; explicit capacity
     or the locked default (64); no unbounded constructor" and cite the registry `channel_capacity`
     domain.
  6. **[DONE] intrinsics SSOT (1.6).** In `dot-postfix.md` (two sites), repoint the intrinsics
     source-of-truth from `crates/ynz-typeck/src/intrinsics.rs` to `registry/features.toml`
     `[[primitive_intrinsic]]`.
     **CHECKPOINT** — all six Class-1 contradictions closed; each edited file still internally
     consistent. **[REACHED — phase completed in one segment, no PARTIAL/handoff needed.]**
- **Exit criteria:** grep confirms no rule teaches call-site `.share`/`.lend`/`.give`/`.copy`-no-parens
  as live syntax; the golden-rule count reads 13 (or count-free) everywhere; **Rule 12's section
  precedes Rule 13's section in `REF-golden-rules.md`** (ordering, not just count); no rule names a
  second SSOT for intrinsics or decisions-home.
  **[VERIFIED — see audit.md session log for the grep transcript and the additional sibling-fossil
  fixes (inference.md Addition-category example + styling line, non-oop.md cross-ref, plan-invariants.md
  Safety/Teaching examples, auto-promotion.md auto-Arc override text) discovered by the corpus-wide
  exit-criteria grep beyond the six named steps.]**
- **Reviewer fan-out:** documentation-standards reviewer (verifies each contradiction is resolved in
  the correct direction, not merely reworded), diffing against the cited design docs per
  plan-invariants Step 7.
- **Model tag:** `(docs-edit, high, small)`.

#### Phase 2 — Class-3 stale-path + banned-syntax sweep — **STATUS: COMPLETE** (session-id: 04e22a51-80c1-4b67-a886-083784d61bcd)

- **Task + purpose:** one mechanical sweep of migration-fossil paths, won't-compile examples, and the
  stale inference domains table. The CLAUDE.md migration note is the map.
- **Steps:**
  1. **[DONE] Stale migration paths (3.1).** Sweep `vocabulary.md`:1 (`spec/`, `design/`),
     `plan-invariants.md` (`/design/`, `design/future/`), `language-design.md` §Spec Updates
     (`/spec/`), `spec-writing.md` ("the design folder"), `stdlib-design.md` Rule 7
     (`design/stdlib/regex.md` → `docs/internal/scratchpad/SCRATCH-stdlib-*.md`) and its dead pathless
     `lockin-*.md` citations (remove or repoint to real files — relative markdown links only).
  2. **[DONE] Banned-syntax examples (3.2).** In `spec-writing.md`: `fixed[number]`/`array[number]` →
     `fixed<number>`/`array<number>` (square-bracket generics are parser-banned); nonexistent
     `.where()` → `.filter()` (real registry op).
     **CHECKPOINT** — no rule file cites a pre-migration path or a won't-compile example; tree
     consistent. **[REACHED]**
  3. **[DONE] maybe reconcile (3.3).** Read `IMP-maybe.md`:150. Applied the resolution: code examples
     always `maybe<T>`; prose may say "a maybe int"; rule tables show `maybe<T>`. `IMP-maybe.md`
     itself required no wording change — its own text already states the split cleanly ("`maybe T`
     (user) and `maybe<T>` (since M5 syntax-lock) terminology," and its own body uses `maybe<T>` in
     every table/code context and reserves "`maybe T`" for prose sentences only) — **no CCIR fired,
     no deeper conflict found.** (Patrick confirms the resolution at plan approval.)
  4. **[DONE] inference.md domains table refresh (3.4).** Added the four shipped domains
     (`background_routing`, `parallel_groups`, `channel_capacity`, `auto_arc`) to the "Domains This
     Applies To" table (registry has 13 total, verified via `grep -c '\[\[muted_hint_domain\]\]'
     registry/features.toml`); fixed the `.copy (8 bytes, trivially copyable)` jargon example
     (teaching audit E5) to `.copy() (8 bytes, trivially copyable)` in `inference.md` (the surviving
     file — `naming.md` never carried this example) plus a second bare-`.copy` mention in the new
     auto_arc row's hover-style prose. Integrated in place — no append-drift.
  5. **[DONE] Remaining Class-3 nits.** `examples-structure.md` gallery phrasing → open-ended ("one
     file per milestone, growing as new milestones ship"); `vue-website.md` `/tmp/yinz-design/.../
     shared.css` SSOT → the `@theme` block declared as the live SSOT (the prototype file reframed as
     a one-time historical seed, both at its Tailwind-tokens section and its Cross-References entry);
     its `~/.claude` refs were already plain backtick prose, not markdown links — no change needed
     (already compliant with the named-not-linked convention). Graveyard citations by number
     (plan-invariants "Entries 1,3,4" ×2 sites incl. Cross-References, inference "Entry 2") →
     cite-by-title, titles verified byte-exact against `.claude/graveyard.md`'s actual `##` headings.
     **CHECKPOINT** — Class-3 sweep complete; `maybe` reconcile applied consistently across rule
     tables and IMP-maybe.md. **[REACHED — phase completed in one segment, no PARTIAL/handoff
     needed.]**
- **Exit criteria:** tree-wide grep for `spec/`, `/design/`, `design/future/`, `design/stdlib/`,
  `fixed[`, `.where(`, `/tmp/yinz-design`, and graveyard "Entry <N>" citations returns zero live hits
  in touched files; `maybe T` remains only as prose, `maybe<T>` in every code example and table.
  **[VERIFIED — see audit.md session log for the grep transcript; the only remaining `spec/`/`design/`
  hits tree-wide in `.claude/rules/` are deliberate historical "(formerly `/spec/`)"/"(formerly
  `/design/`)" migration notes (already in that form pre-Phase-2, in files this phase's step list
  never named) plus generic "spec/design docs" prose shorthand in `dot-postfix.md` — neither is a
  stale live-path reference.]**
- **Reviewer fan-out:** documentation-standards reviewer (relative-links + no-stale-path + real-ops
  examples); confirms the `maybe` reconcile matches `IMP-maybe.md`.
- **Model tag:** `(docs-edit, high, medium)`.

#### Phase 3 — naming.md → vocabulary.md full merge + parity guard + link repoint — **STATUS: COMPLETE** (session-id: 04e22a51-80c1-4b67-a886-083784d61bcd)

- **Task + purpose:** the Class-2 de-dup flagship. Merge `naming.md` wholesale into `vocabulary.md`,
  delete `naming.md`, repoint its inbound links — guarded by a mechanical content-parity inventory so
  no normative sentence is lost (Risk #1). This supersedes Phase 1's cheap naming.md stop-loss.
- **Steps:**
  1. **[DONE] Pre-merge grep.** Tree-wide grep for markdown LINKS to the deleted `.claude/rules/naming.md` —
     use the anchored pattern **`[^-]naming\.md)`** (which matches same-dir `](naming.md)` and
     `](.../rules/naming.md)` links but EXCLUDES the surviving `docs/reference/REF-naming.md`, whose
     `-naming.md` is a suffix collision — verified live at `docs/README.md`:42, `](reference/REF-naming.md)`).
     **Expected survivor:** links to `REF-naming.md` are correct and remain — they are NOT inbound
     references to the deleted file. Confirm the inbound link set for the deleted file is exactly the
     enumerated five + `done/` archive (verifies the unverified assumption before deleting). Prose mentions of the
     string `naming.md` inside the gate-exempt scratchpad audit docs
     (`SCRATCH-teaching-audit-2026-07-11.md`, `SCRATCH-rules-audit-2026-07-11.md`,
     `SCRATCH-audit-2026-07-11-codegen-miscompiles.md`) are NOT links and are out of scope — the plan
     forbids touching those sources; do not treat them as inbound references.
     **[VERIFIED — see audit.md session log: the anchored grep's real inbound-link set was exactly the
     enumerated five (CLAUDE.md ×2 occurrences within one file, docs-checklist.md, REF-naming.md,
     SCRATCH-stdlib-encoding.md, vocabulary.md self-ref) + the `done/` archive; the only other anchored
     hits were this plan's own `plan.md`/`audit.md` prose describing the grep pattern in backticks
     (not real markdown links) — no CCIR fired, nothing outside the enumerated five.]**
  2. **[DONE] Build the content-parity inventory.** Enumerate every table row, heading, and normative
     sentence of `naming.md` AND `vocabulary.md` into a checklist (artifact lands in the PR / this
     plan's dir). This is the Risk-#1 B2 guard — build it before editing.
     **CHECKPOINT — REACHED.** Parity inventory complete; every naming.md item mapped to its vocabulary.md
     destination (merge, or supersede-as-duplicate with the surviving copy named). Full inventory recorded
     in audit.md session log.
  3. **[DONE] Merge into vocabulary.md.** Integrate naming.md's casing rule + module/type case distinction +
     the corrected (signature-keyword) term mappings into their logical homes in vocabulary.md — no
     append-drift; reconcile the GR13 restatement to one line + link to the canonical
     `REF-golden-rules.md`; de-dup the renamed-concepts table to the single vocabulary.md home.
  4. **[DONE] Delete naming.md.** Remove the file (git history is the archive).
  5. **[DONE] Repoint inbound links.** Update `CLAUDE.md`:56, `docs-checklist.md`:78 (What-Goes-Where row),
     `REF-naming.md`:16, `SCRATCH-stdlib-encoding.md`:62 to point at `vocabulary.md` (relative
     markdown links). Remove the `vocabulary.md`:163 self-ref.
     **CHECKPOINT — REACHED.** naming.md deleted, all inbound links repointed, vocabulary.md carries the merged
     content.
  6. **[DONE] Post-merge parity check.** Walk the inventory: confirm every enumerated naming.md item now
     lives in vocabulary.md (or is a named, deliberate duplicate-removal). Tick each box.
     **[VERIFIED — see audit.md session log for the fully-ticked inventory.]**
- **Exit criteria:** `naming.md` absent; tree-wide grep for markdown LINKS to the deleted
  `.claude/rules/naming.md` (the anchored `[^-]naming\.md)` pattern — excludes the surviving
  `REF-naming.md`) returns zero live links (only `done/` archive + git history) — the intent
  is **zero broken links**, NOT zero occurrences of the string (gate-exempt scratchpad audit docs keep
  their prose mentions and are excluded; links to `REF-naming.md` are the expected survivor and remain);
  parity inventory 100% checked; vocabulary.md carries GR13 as
  one-line-+-link and one renamed-concepts table.
  **[VERIFIED — see audit.md session log for the post-merge grep transcript: `.claude/rules/naming.md`
  absent from disk; anchored grep tree-wide returns zero real markdown links (only this plan's own
  plan.md/audit.md prose describing the grep pattern + the `done/` archive, both excluded by the exit
  criteria's own terms); parity inventory 100% ticked; `vocabulary.md`'s Capital Letter Rule section is
  one-line-+-link to `REF-golden-rules.md` Rule 13 followed by the merged casing examples, and the
  Quick Reference table is the single renamed-concepts home (naming.md's table fully absorbed, two
  previously-missing rows added: `base shape`, `follows`).]**
- **Reviewer fan-out:** (a) content-parity reviewer — word-for-word negation-diff on trimmed boundary
  sentences + walks the parity inventory (the Risk-#1 blocking guard); (b) documentation-standards
  reviewer — one-home / relative-links / placement-law-4 (no append-drift). Both block.
- **Model tag:** `(docs-merge, high, large)`.

#### Phase 4 — gap-rule authoring + frontmatter/conformance pass

- **Task + purpose:** author the three missing rules and bring every file this plan touched to the
  global documentation standard (descriptive frontmatter, wording force-matching, one-home markers).
- **Steps:**
  1. **Teaching-surfaces rule.** Author `.claude/rules/teaching-surfaces.md` to teaching audit §H2.7:
     the three-slot test (WHAT states the problem; WHAT-INSTEAD is copyable/actionable; WHY is
     contextual, non-circular, cites no internals), the audience test (18-yo JS dev, no Googling),
     the banned-vocab pointer, naming conventions inside examples (camelCase, `.copy()` parens, no
     SCREAMING_SNAKE unless ratified), and the no-internal-paths / no-milestone-tags rules. **Note on
     the ratifier reference:** §H2.7's literal text says "no SCREAMING_SNAKE unless naming.md
     ratifies it," but `naming.md` is deleted in Phase 3 and the constants ruling (Step 2 below)
     lands in `vocabulary.md` — so the new teaching-surfaces.md points its ratifier reference at
     `vocabulary.md`, not the dead `naming.md`. **Line budget: target ≤120 lines** — a soft target,
     not a hard cap: if §H2.7 cannot fit within it, surface the budget-vs-completeness tradeoff per
     the §3.4 CCIR (never silently overrun or silently cut a §H2.7 item) (Risk #5). This is the
     artifact plan 2 depends on.
     **CHECKPOINT** — teaching-surfaces.md drafted within budget; §H2.7 checklist items all present.
  2. **Constants-naming ruling.** Add the paragraph to the merged `vocabulary.md`: constants are
     camelCase (`const maxHealth = 100`); GR13 "capital = type" stays absolute — no SCREAMING_SNAKE
     exception. (Parser diagnostics teaching `MAX_HEALTH` are plan-2 scope; this plan writes only the
     ruling.)
  3. **Import-path canon.** Add a short section to the merged `vocabulary.md` (the natural home for
     "how Yinz spells X"): import paths are backtick-quoted, project-root-relative, no `.ynz` suffix.
     (The parser's stray double-quote acceptance is plan-2 / compiler scope.) Record the home choice.
     **CHECKPOINT** — all three gap rules authored and placed in their one home.
  4. **Frontmatter + conformance pass.** Add global-standard descriptive YAML frontmatter to every
     rewritten/new rule file (name + block-scalar description + tags + created_at/updated_at/status/
     author/metadata; every string scalar double-quoted, booleans/numbers bare) — **NO `paths:` key**
     on any file (vue-website.md untouched). Re-grade inflated MUST/NEVER on judgment-tier sentences
     to reasoned-soft phrasing (REF-wording, named-not-linked). Add one-home markers where a rule now
     points instead of restating (Class 2.2/2.3/2.4/2.5 de-dups: GR13, non-OOP drift-signals,
     auto-promotion criterion, banned-jargon → registry).
- **Exit criteria:** three new/consolidated rules exist in one home each; teaching-surfaces.md within
  budget; every touched rule file opens with valid descriptive frontmatter and zero `paths:` keys
  (except vue-website.md); duplicated tables reduced to one home + pointers.
- **Reviewer fan-out:** documentation-standards reviewer (frontmatter validity, wording
  force-matching, one-home law); teaching-content reviewer (grades teaching-surfaces.md against §H2.7
  for completeness + line budget).
- **Model tag:** `(docs-authoring, high, large)`.

#### Phase 5 — verification sweep + PR

- **Task + purpose:** since no CI catches doc breakage, verify link-integrity, content-parity, and
  the no-scoping guarantee by scripted grep, then open the PR.
- **Steps:**
  1. **Link-integrity grep.** Tree-wide: zero live markdown LINKS to the deleted
     `.claude/rules/naming.md` (the anchored `[^-]naming\.md)` pattern — excludes the surviving
     `REF-naming.md`; the intent is zero broken links, not zero string occurrences; gate-exempt
     scratchpad audit docs keep their prose mentions, and links to `REF-naming.md` remain correct);
     every relative markdown link in a
     touched file resolves to a real path (scripted resolve-check); no bare-text or machine-local
     (`~/.claude`, `/tmp`) links in touched files.
  2. **No-`paths:` check.** Grep every file this plan touched for a `paths:` frontmatter key → must
     return only `vue-website.md` (Risk #6).
  3. **YAML-valid check.** Parse the frontmatter of every touched rule file (e.g. `python3 -c` yaml
     load) → all valid; string scalars quoted, booleans/numbers bare.
  4. **Content-parity sign-off.** Confirm the Phase-3 parity inventory is 100% ticked and the
     negation-diff reviewer signed.
  5. **Open PR** from the `main`-forked docs-only branch (via `/pr` or manual), referencing this
     plan-id and both audit scratch docs.
- **Exit criteria:** all four grep/parse checks green; parity signed; PR open against `main`.
- **Reviewer fan-out:** documentation-standards reviewer (final gate — runs the grep suite and
  confirms no regression against Class-6 "healthy, don't break" files).
- **Model tag:** `(verification, high, small)`.

### 3.4 Coordinating Instructions

- **Sequencing:** Phase 1 (stop-loss) first for immediate value; Phase 3 (merge) supersedes Phase
  1's cheap naming.md correction, so the Phase-1 naming.md edit is a *closing-the-window* guard, not
  wasted work. Phases are otherwise independently reversible; do not batch Phase 3 into another phase
  — its parity guard needs its own reviewer gate.
- **Verify-before-complete gates:** Phase 3 cannot close until the content-parity inventory is 100%
  ticked AND the negation-diff reviewer signs (Risk #1). Phase 5's no-`paths:` grep gates the PR
  (Risk #6).
- **CCIR — surface mid-flight, do not silently decide:**
  - the `maybe` reconcile revealing a conflict `IMP-maybe.md` does not already resolve (design-doc
    contradiction → plan question, not overwrite);
  - any inbound `naming.md` reference outside the enumerated five surfacing in the Phase-3 pre-merge
    grep (the assumption breaks → re-scope the repoint set);
  - the teaching-surfaces rule not fitting §H2.7 within the ≤120-line budget (raise the budget-vs-
    completeness tradeoff rather than silently overrun or silently cut a §H2.7 item).
- **Do NOT:** add a `paths:` key to any file (decision 3 — split out); touch `vue-website.md`'s
  existing `paths:` frontmatter; touch `done/` archived plans; rewrite the scratchpad audit sources
  (exempt); fix the downstream compiler/registry teaching text (that is plan 2).

## 4. Sustainment

- **Env / tooling:** plain markdown edits — no build, no container, no language runtime needed.
  Verification is `grep` / `git` / `python3 -c` (yaml) on the host (all host-native). No Docker.
- **Branch:** fork a docs-only branch from `main` (not `feat/v0-3-m6-concurrency-hotfix`).
- **Inputs:** the two audit scratch docs (findings inventory), `IMP-maybe.md` (reconcile anchor),
  `REF-golden-rules.md` (correction target). Global `IMP-documentation-system.md` + `REF-wording.md`
  (named, not linked) as the conformance standard.

## 5. Command & Signal

- **Ownership:** single-executor docs work; each phase owned by the dispatched executor, gated by the
  reviewer fan-out named per phase.
- **Succession:** a fresh session resumes from this `plan.md`'s checkbox/checkpoint state + the
  session-id chain + the audit sidecar. The fat phases (3, 4) write `handoff-phase-<N>.md` on a
  checkpoint segment per the plan-format handoff convention.
- **Audit trail:** [`audit.md`](./audit.md) sidecar; session-id chain in frontmatter.

## Future Requirements / Revisit

- **#5 teaching-surfaces line budget (RECORDED MEDIUM).** *What:* the consolidated
  `teaching-surfaces.md` may exceed its soft ≤120-line target while still needing every §H2.7 item —
  the only control is the §3.4 CCIR (human weighs the budget-vs-completeness tradeoff and surfaces it),
  which moves no axis (B3). *Why deferred:* no engineered guard can compress the content without a
  judgment call about which §H2.7 items to condense; auto-truncation would silently drop teaching
  content. *Cost:* one focused revision pass to tighten prose (~minutes-to-an-hour). *Trigger:*
  **`teaching-surfaces.md` exceeds 120 lines at review time, OR plan 2 finds the rule diluted by
  length** — at which point run the tightening pass against §H2.7 completeness.
- **#9 m8-collision (RECORDED MEDIUM).** *What:* `m8-concurrency-completion` Phase 1 will add a
  channel-close term to `vocabulary.md`, which this plan rewrites (and rewrites `stdlib-design.md`
  Rule 4). *Why deferred:* the only available control is human sequencing (B3 — no engineered guard
  moves the score). *Cost:* a merge reconcile of one term into the rewritten `vocabulary.md`
  (~minutes). *Trigger:* **m8 Phase 1 begins before this plan merges** — at which point land this
  cleanup first, or reconcile the channel-close term into the merged `vocabulary.md` at merge time.
- **Class-4 `paths:` load-scoping (split-out effort).** *What:* scope the currently-always-loaded
  rules via `paths:` frontmatter. *Why deferred:* changes what binds every future turn — requires a
  `prove-before-optimize` falsification run (Patrick decision 3), not this audit's hunch. *Cost:* a
  separate multi-run experiment + plan. *Trigger:* a decision to run the falsification protocol.
  This plan adds descriptive frontmatter **only** and asserts no `paths:` key.
- **Plan 2 — teaching remediation.** *What:* the compiler/registry teaching-text remediation
  (teaching audit §§A–F + the structural guards §H2). *Why deferred:* out of the docs-corpus charter;
  depends on this plan's teaching-surfaces rule. *Cost:* a standalone multi-phase plan touching
  `crates/**` + registry + tests. *Trigger:* this plan's teaching-surfaces rule merges.
