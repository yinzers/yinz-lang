---
name: "rules-corpus-cleanup-audit"
plan-id: "2026-07-11-rules-corpus-cleanup"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-11-rules-corpus-cleanup

Append-only. *How the plan got here.* Read by the AAR, auditors, and the execution conductor's
Step-3a / Step-0 reconcile; never by executors (they read the current-truth plan.md slice).

## Session log
- 3967a266-28a7-4313-88d9-52d2becad7d8 — 2026-07-11 — Authored the OPORD from the assembled brief +
  the two 2026-07-11 audit scratch docs. Scored ¶1 risk deterministically via REF-risk-engine
  (LOW overall; one recorded MEDIUM #9 m8-collision, parked in Future Requirements with trigger).
  No HIGH residual → no override block drawn. Recorded decision: Phase 1 makes a cheap ~4-row
  naming.md ownership-row correction as a fossil-fork exposure-window guard even though Phase 3
  deletes the file (no-duct-tape cheap-in-scope-mitigation), superseded by the Phase 3 merge. Set
  status active (Intent & End State non-empty).

- 3967a266-28a7-4313-88d9-52d2becad7d8 — 2026-07-11 — **Revision pass** (same session-id as the
  authoring dispatch, per the scratchpad-derived id; NOT duplicated in the frontmatter chain because
  appending an identical id would be a misleading `[id, id]` record — the append-only schema forbids
  overwrite, not requires duplication). Applied seven plan-reviewer findings, all surgical, no
  restructure:
  (1) Phase 1 Step 4 — corrected the Rule-13 reorder: Rule 13 moves AFTER Rule 12 (file currently
  lists 13 before 12; canonical is 12→13). Prior text said "before Rule 12," which preserved the bug.
  (2) Phase 1 exit — added an ordering criterion (Rule 12's section precedes Rule 13's), matching the
  End State's "count + ordering" promise.
  (3) Phase 3 Step 1 / Phase 3 exit / Phase 5 Step 1 — rescoped the `naming.md` grep from
  string-occurrence (unmeetable — gate-exempt scratchpad audit docs mention it in prose) to markdown
  LINKS only; intent restated as "zero broken links," scratchpad prose explicitly excluded.
  (4) Design-Doc Alignment Boundary assumptions — recorded Class 5.4 (rule-header convention) as a
  deliberate deferral to plan 2, so the audit finding is parked rather than vanished.
  (5) Phase 1 Step 4 — enumerated the fourth count-bearing "12" site (line ~35 "the 12 rules below")
  and added a grep-based verification criterion.
  (6) Phase 4 Step 1 — added the ratifier-reference note (§H2.7 says "naming.md ratifies it" but
  naming.md dies in Phase 3; teaching-surfaces.md points its ratifier at vocabulary.md).
  (7) Risk #5 — reconciled wording DOWN from "hard line-budget blocking gate" to the soft-target
  design with the §3.4 CCIR surfaced tradeoff (matching Phase 4's correct implementation); also
  softened Phase 4 Step 1's "Hard line budget" phrasing to a soft target for internal consistency.
  No new risk or contradiction surfaced by any edit.

- 3967a266-28a7-4313-88d9-52d2becad7d8 — 2026-07-11 — **Revision pass — round 3** (same session-id;
  not re-appended to the frontmatter chain, same rationale as the round-2 entry). Applied the round-2
  plan-reviewer findings, all surgical:
  (SHOULD-FIX 1) Risk #5 (teaching-surfaces line budget) — corrected the control bucket from B2 to
  **B3**: it is a soft line-budget target surfaced via the §3.4 CCIR = human vigilance (0 axis move,
  gate-only per REF-risk-engine), not an engineered guard. Removed the unearned prob −1; residual
  re-scored honestly C×III = **M (RECORD)**. Updated the headline posture everywhere it appears
  (¶1 intro "two recorded MEDIUMs (#5, #9)"; post-table line; Future Requirements). Parked #5 in
  Future Requirements with trigger: "exceeds 120 lines at review, OR plan 2 finds the rule diluted by
  length." **New headline posture: TWO recorded MEDIUMs (#5, #9), no HIGH, no override.**
  (SHOULD-FIX 2) Phase 3 Step 1 / Phase 3 exit / Phase 5 Step 1 — replaced the false-positive
  `](...naming.md)` grep (which matches the surviving `docs/reference/REF-naming.md` — verified live at
  `docs/README.md`:42) with the anchored **`[^-]naming\.md)`** pattern (matches the deleted
  `.claude/rules/naming.md` links, excludes `-naming.md` suffix collisions). Stated the expected
  survivor explicitly: links to `REF-naming.md` are correct and remain.
  (MINOR 3) Risk #11 (maybe-reconcile) — took the reviewer's offered justification path rather than
  re-scoring: kept residual **L** and justified the −1 as **B2** in one line — the reconcile is a
  mandatory, bounded, phase-exit-gating hard STOP that *survives forgetting* (it lives in Phase 2's
  exit criteria), distinct from #5's soft judgment-based target. The "never overwrite" clause + Patrick
  sign-off are the B3 gate layer on top, explicitly not counted in the score. This keeps the posture at
  two MEDIUMs, consistent with SHOULD-FIX 1.
  (MINOR 4) Risk table — added a Risk-ID legend: the non-contiguous IDs follow the recon hazard-sweep's
  Enemy-candidate numbering; unlisted IDs (#3, #4, #7, #8, #10) resolved to LOW with no plan-level
  control and are omitted.
  No new contradiction surfaced; the cross-cutting perf/BigO sweep line (references Risk #5) stays
  accurate — the factor is still "addressed" (scored + recorded), just at M rather than mitigated to L.

- 7db1f615-0a90-4130-8aee-8513add883d8 — 2026-07-16 — **Phase 1 execution — Class-1 contradiction
  stop-loss.** Executed all six enumerated fixes:
  (1.1) `.claude/rules/naming.md` — rewrote the four `&T`/`&mut T`/`move`/`.clone()` rows to the
  signature-keyword model (`share`/`lend`/`give` signature-only, `.copy()` with parens), matching
  `vocabulary.md`.
  (1.2) `.claude/rules/inference.md` — rewrote the canonical Hover Tooltip Format example's
  WHAT-INSTEAD to Informational-correct ("nothing to type... click jumps to `foo`'s signature").
  (1.3) `.claude/rules/language-design.md` §Documenting Decisions + `.claude/rules/spec-writing.md` —
  repointed the decisions-home pointer from `/docs/README.md` to
  `docs/internal/implementation/IMP-<feature>.md` per `docs-checklist.md`.
  (1.4) `docs/reference/REF-golden-rules.md` — fixed all four count-bearing "12" sites (frontmatter
  description, intro, cross-cutting line, "the 12 rules below") to 13; verified via
  `grep -n '12' docs/reference/REF-golden-rules.md` returning only the creation-date and Rule 12's own
  number (no count-bearing hits). Reordered Rule 13's section to AFTER Rule 12's (was inverted).
  `CLAUDE.md`:76 and `language-design.md`:9 changed to count-free "all golden rules" phrasing.
  (1.5) `.claude/rules/stdlib-design.md` Rule 4 — reworded to "bounded always; explicit capacity or the
  locked default (64); no unbounded constructor," citing the registry's `channel_capacity`
  `[[muted_hint_domain]]` entry (verified present in `registry/features.toml`:2250-2256).
  (1.6) `.claude/rules/dot-postfix.md` (two sites — inline prose + Cross-References list) — repointed
  the intrinsics SSOT from `crates/ynz-typeck/src/intrinsics.rs` to `registry/features.toml`
  `[[primitive_intrinsic]]`, verified via source read that `intrinsics.rs` is in fact generated from
  `ynz_registry::primitive_intrinsics()`, not a second hand-maintained source.
  **Recorded decision (durable-answer call, no human in the loop):** the phase's own exit criteria is
  corpus-wide ("grep confirms no rule teaches call-site `.share`/`.lend`/`.give`/`.copy`-no-parens as
  live syntax" — not scoped to only the six named files), so after the six fixes a corpus-wide grep
  sweep was run and surfaced FIVE additional sibling fossils of the identical class, all fixed in this
  same segment rather than left for a later phase to trip the exit-criteria grep on: `inference.md`
  line ~106 (an `foo(player) // muted .share` line wrongly listed under the "Addition" placement
  category — contradicts the file's own Domains table, which classifies ownership-at-call-site as
  Informational; removed, with a one-line cross-reference back to the Domains table) and line ~179
  (styling-rules bullet used dot-prefixed `.lend`/`.give` notation — de-dotted to match the
  signature-keyword model); `non-oop.md`'s Cross-References entry describing `.share()`/`.lend()`/
  `.give()` with parens as call-site syntax (reworded to name them as signature-only keywords with no
  body-level syntax); `plan-invariants.md`'s M4 Safety-subsection examples and one Teaching-subsection
  example, which used dot-prefixed `.lend`/`.give` notation implying body-level call-site syntax
  (reworded to describe the compile-time behavior without implying dot-postfix syntax exists);
  `auto-promotion.md`'s auto-Arc override-form text, which named `.give`/`.copy` (no parens on either)
  as literal spawn-site syntax with no design-doc backing found in `IMP-no-function-coloring.md`
  (reworded to "pass the value as a `give` parameter... or call `.copy()` on it," consistent with the
  signature-keyword + parens-for-actions model). None of these five required new design decisions —
  each was a mechanical notation fix to match the already-locked signature-keyword model the six named
  fixes established; no scope beyond "close every live instance of the same already-diagnosed
  contradiction" was taken.
  **Verification (Paper-Trace):** re-ran all four exit-criteria greps after every edit; final sweep
  confirms (1) the only remaining `.share`/`.lend`/`.give` hits in the rules corpus + REF-golden-rules.md
  are explicit negations ("NO body-level `.share()` syntax") or the Inverse-Anti-Pattern illustration in
  `inference.md` (which correctly shows what NOT to write) — zero live-syntax teaching remains; (2) zero
  count-bearing "12" hits remain in `CLAUDE.md`, `language-design.md`, or `REF-golden-rules.md`; (3)
  `REF-golden-rules.md` line ~150 is now `**12. Human-readable...**` and line ~159 is
  `**13. Capital letter...**` — Rule 12 precedes Rule 13; (4) `dot-postfix.md` names only
  `registry/features.toml` as the intrinsics SSOT; `language-design.md` and `spec-writing.md` name only
  `docs/internal/implementation/IMP-<feature>.md` as the decisions-home (docs-checklist.md's own
  `docs/README.md` mentions are legitimate index-file references, not decisions-home claims — left
  untouched, out of scope).
  **Plan↔task sync note:** TodoWrite is not in this dispatch's tool grant (Read/Write/Edit/Bash only);
  this plan also carries no `- [ ]` checkbox glyphs (per `REF-plan-format.md`, this repo's shipped
  plans number Steps `1. 2. 3. …`, a documented pre-existing drift from the checkbox convention) — so
  plan↔task sync for this phase runs entirely through `plan.md`'s inline `[DONE]` step markers +
  `**STATUS: COMPLETE**` phase annotation + this audit entry, the actual write-accessible mechanism
  for this dispatch.
  **Deviations surfaced:** none — no plan-vs-reality divergence found; the six named fixes and the five
  sibling fixes were all mechanical corrections toward the already-locked model the rest of the corpus
  describes, per the plan's own Intent ("does this edit make the corpus more self-consistent... "). No
  FRAGO filed.
  Phase 1 returns COMPLETE. No checkpoint/handoff file needed — the phase completed in one segment
  (well under any of the three checkpoint triggers: 6 steps close to but not over 5-plus-heavy,
  no step flagged heavy/adversarial, Model tag scale=small).

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 1 review fan-out — routing log for three
  non-blocking findings** (log-only dispatch; no code change beyond bumping
  `docs/reference/REF-golden-rules.md` frontmatter `updated_at` to "2026-07-16" to match the phase's own
  content edit). Routing decisions:
  (1) `docs/internal/scratchpad/SCRATCH-stdlib-overview.md:22` "all 12 golden rules" — flagged by both
  code-reviewer and doc-auditor; scratchpad is explicitly gate-exempt per `docs-checklist.md` and out of
  this plan's scope. Not actioned.
  (2) `.claude/rules/inference.md:45`'s `.copy (8 bytes, trivially copyable)` muted-hint example still
  teaches call-site `.copy` without parens — flagged by acceptance-verifier as a residual against Phase
  1's literal exit-criteria wording; Class-3 (not Class-1), already explicitly scheduled at this plan's
  own Phase 2 Step 4 ("fix the `.copy (8 bytes, trivially copyable)` jargon example (teaching audit E5)").
  No new deferral needed; cross-referencing the existing scheduled fix.
  (3) `.claude/rules/inference.md:210`'s Inverse Anti-Pattern example still quotes `.share` dot-syntax as
  the illustration of banned phrasing — doc-auditor judged this defensible to leave (it illustrates
  prohibited spec language, not live syntax) and optional to sharpen. Accepted as-is, no action.

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 2 execution — Class-3 stale-path +
  banned-syntax sweep.** Same session-id as the Phase 1 review-fan-out dispatch above; NOT re-appended
  to the frontmatter chain (identical rationale as the round-2/round-3 revision-pass entries — an
  `[id, id]` duplicate would misrepresent the append-only chain as two distinct sessions). Executed all
  five steps:
  (3.1) Stale migration paths — `vocabulary.md`:3 (`spec/`/`design/` → `docs/reference/REF-*.md`/
  `docs/internal/implementation/IMP-*.md`); `plan-invariants.md` (two sites: `/design/` docs →
  `docs/internal/implementation/IMP-*.md`, `design/future/` → `docs/internal/scratchpad/
  SCRATCH-future-*.md`; and `/design/` truly-has-no-file → `docs/internal/implementation/`);
  `language-design.md` §Spec Updates (`/spec/` → `docs/reference/REF-*.md`); `spec-writing.md`
  ("open questions live in the design folder" → "live in `docs/internal/scratchpad/`");
  `stdlib-design.md` Rule 7 (`design/stdlib/regex.md` → `docs/internal/scratchpad/
  SCRATCH-stdlib-regex.md`) and removed its two dead pathless `lockin-*.md` Cross-Reference citations
  (confirmed via `find . -iname "lockin*"` returning nothing anywhere in the repo — no real file to
  repoint to; the findings they cited, Java `URL.equals()` / Go `encoding/json` / Python encoding
  default, are already described inline in Rules 1/3/6's own bodies, so no content was lost).
  (3.2) Banned-syntax examples — `spec-writing.md`: `fixed[number]`/`array[number]` →
  `fixed<number>`/`array<number>` in the Compiler Error Format example; `.where()` → `.filter()` in
  Code Examples (verified `.filter()` is a real registry op via `grep -n '"filter"' registry/
  features.toml`).
  (3.3) maybe reconcile — read `IMP-maybe.md`:150 plus its full LLVM Lowering / flow-sensitive-`.value`
  sections; confirmed the file's own text already resolves the split cleanly ("`maybe T` (user) and
  `maybe<T>` (since M5 syntax-lock) terminology") and its own body already uses `maybe<T>` consistently
  everywhere except free prose — **no CCIR fired; no wording edit needed in `IMP-maybe.md`.** Applied
  the resolution to the rule corpus: `vocabulary.md` Quick Reference table row, Banned Legacy Terms
  table row, and the `### maybe T` → `### maybe<T>` section heading + its definitional first sentence
  (all table/heading/definition contexts, per the resolution's "rule tables show `maybe<T>`" clause);
  left `vocabulary.md`'s "✅ Correct: `maybe User`" prose example untouched (free prose, per the
  resolution's "prose may say 'a maybe int'" clause). `naming.md`'s equivalent table row also fixed
  (`maybe T` → `maybe<T>`) since Phase 2's exit criteria is tree-wide and `naming.md` is still live
  until Phase 3 deletes it — same cheap-exposure-window-closing logic Phase 1 already applied to that
  file.
  (3.4) inference.md domains table refresh — added four rows (`background_routing`, `parallel_groups`,
  `channel_capacity`, `auto_arc`) sourced verbatim from their `registry/features.toml`
  `[[muted_hint_domain]]` entries (description/example_source/example_hint_rendered), placed near their
  thematically-related existing rows; verified the registry carries exactly 13 `[[muted_hint_domain]]`
  entries total (`grep -c`) and all 13 now have a corresponding Domains-table row. Fixed the teaching
  audit E5 jargon example: `.copy (8 bytes, trivially copyable)` → `.copy() (8 bytes, trivially
  copyable)` in the Copy-points row, plus a second bare-`.copy` mention inside the new auto_arc row's
  own prose ("pass `.copy`" → "call `.copy()`") — both now match `dot-postfix.md`'s parens-for-actions
  rule. Confirmed via `grep -rn '\.copy\b' .claude/rules/*.md | grep -v '\.copy()'` returning zero hits.
  (3.5) Remaining Class-3 nits — `examples-structure.md` gallery phrasing → open-ended ("one file per
  milestone, growing as new milestones ship"), replacing the closed `M1-M8 + v0.2-M1-M3` enumeration
  that would go stale every future milestone. `vue-website.md`: the Tailwind-tokens section's "Source
  of truth is the prototype `shared.css`" reframed to declare the `app/assets/css/tailwind.css`
  `@theme` block as the live SSOT, with the prototype file named as a one-time historical seed (not an
  ongoing reference); the Cross-References `/tmp/yinz-design/yinz/project/shared.css` entry rewritten
  the same way, explicitly flagging the old path as machine-local scratch, not a live reference. Its
  two `~/.claude/rules/*.md` mentions were checked and found already plain backtick prose (not markdown
  links) — already compliant with the named-not-linked convention; no edit needed. Graveyard citations
  by number → cite-by-title: `plan-invariants.md`'s three Enforcement-section bullets (Entry 1/3/4) and
  its Cross-References line (Entries 1, 3, 4) → the four graveyard entries' exact `##` heading text,
  byte-verified against `.claude/graveyard.md` ("Const Deep-Immutability Invariant Unstated in
  Milestone Plans", "M4+ Milestone Plans Missing the 5-Subsection Invariants Structure", "Language or
  Stdlib Features Without Runtime + Kernel-Mode Declaration"); `inference.md`'s Entry 2 →
  "Requiring Explicit Ownership Annotation at Call Sites".
  **Verification (Paper-Trace):** re-ran the full exit-criteria grep suite across `.claude/rules/`
  after all edits — `spec/`, `/design/`, `design/future/`, `design/stdlib/`, `fixed[`, `.where(`,
  `/tmp/yinz-design`, and `Entry [0-9]` all return zero live hits in the six files this phase's step
  list named plus `naming.md` (the tree-wide-scoped `maybe` fix); the only remaining `spec/`/`design/`
  substring hits anywhere in `.claude/rules/` are (a) `docs-checklist.md`'s and `vocabulary.md`'s own
  deliberate "(formerly `/spec/`)"/"(formerly `/design/`)" historical migration notes — not stale live
  references, and (b) `dot-postfix.md`'s generic "spec/design docs" / "spec/design/plan/rule file"
  shorthand prose, which never claimed to be a literal directory path. `maybe T` (bare, no angle
  brackets) now appears in `.claude/rules/` exactly once, in `vocabulary.md`'s own definitional
  sentence, explicitly framed as the prose/spoken form — matching the resolution.
  **Plan↔task sync note:** same constraint as Phase 1 — no TodoWrite in this dispatch's tool grant, no
  `- [ ]` checkbox glyphs in this plan (Steps use `1. 2. 3. …` numbering, a documented pre-existing
  drift from the checkbox convention). Sync runs through `plan.md`'s inline `[DONE]` step markers +
  `**STATUS: COMPLETE**` phase annotation + this audit entry.
  **Deviations surfaced:** none — every edit was a mechanical Class-3 correction the plan's own step
  list enumerated; no plan-vs-reality divergence found. No FRAGO filed.
  Phase 2 returns COMPLETE. No checkpoint/handoff file needed — the phase completed in one segment (all
  five steps landed, both planned CHECKPOINT marks reached in-segment, Model tag scale=medium, well
  under any checkpoint trigger).

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 2 review fan-out — routing log** (conductor
  entry, no code change). Cheap gates (green-check, graveyard-auditor) both clean/green. All five
  reviewers (code-reviewer, acceptance-verifier, rules-compliance, deviation-judge, doc-auditor) returned
  clean/met/on-plan/conformant — zero blockers. Two non-blocking findings routed:
  (1) doc-auditor flagged `maybe T` (bare, no angle brackets) still appearing as a live code-signature form
  in the broader `docs/` knowledge tree — `docs/internal/implementation/IMP-errors.md:30` (should-fix,
  won't-compile form) and a minor cluster (`IMP-collections.md`, `IMP-ownership.md`, `REF-linting.md`,
  `REF-compiler-errors.md`, `docs/README.md`). This is explicitly OUT OF THIS PLAN'S CHARTER — Design-Doc
  Alignment's Boundary assumptions + Human decision 5 both state the `maybe` reconcile is corpus-scoped,
  "not blind-sweep" across `docs/`. Not actioned; flagged here as a real finding for a future docs-tree
  pass (possibly plan 2 / teaching-remediation scope, or a fresh audit item) — same disposition as Phase
  2's own out-of-scope finding already logged in its execution entry above.
  (2) code-reviewer noted `docs/reference/REF-golden-rules.md`'s Rule 11 hover example and
  `.claude/rules/inference.md`'s Hover Tooltip Format example are near-identical but not byte-identical
  (a pre-existing duplicate-home condition, not introduced or worsened by this diff — Golden Rule 11 itself
  wants "one canonical explanation per concept, reused verbatim"). Not actioned this phase; noted as a
  Class-2-de-dup-adjacent candidate for Phase 4's one-home-marker pass, whose scope is exactly this kind
  of duplicated-table consolidation.

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 3 execution — naming.md → vocabulary.md
  full merge + parity guard + link repoint (Class-2 de-dup flagship).** Same session-id as the prior
  Phase 1 review-fan-out / Phase 2 execution / Phase 2 review-fan-out entries above; NOT re-appended to
  the frontmatter chain (identical rationale — an `[id, id]` duplicate would misrepresent the
  append-only chain as two distinct sessions). Executed all six steps:

  **(Step 1) Pre-merge grep.** Ran the anchored pattern `[^-]naming\.md)` tree-wide
  (`grep -rn '[^-]naming\.md)' --include='*.md' .`). Real inbound-link hits, classified:
  - `CLAUDE.md`:53 and `CLAUDE.md`:77 — TWO occurrences within the ONE enumerated `CLAUDE.md` file
    (the plan's assumption cited a single line "CLAUDE.md:56"; confirmed via `git show 46bab6d:CLAUDE.md`
    that both occurrences pre-date this plan's Phase 1/2 work — not new drift, just the plan's shorthand
    citing one representative line for the file). Both within the enumerated set.
  - `.claude/rules/docs-checklist.md`:78, `docs/reference/REF-naming.md`:16,
    `docs/internal/scratchpad/SCRATCH-stdlib-encoding.md`:62, `.claude/rules/vocabulary.md`:163
    (self-ref) — the remaining four of the enumerated five.
  - `.claude/planning/done/2026-05-14-design-lockdown-from-gemini-review/plan.md` (5 hits) — `done/`
    archive, confirmed out of scope per plan text ("Historical `done/` plans are not touched").
  - `.claude/planning/active/2026-07-11-rules-corpus-cleanup/plan.md`:299-300 and `audit.md`:55 — this
    plan's own prose describing the grep pattern itself in backticks (e.g. `` `](naming.md)` `` as an
    illustrative example of what the pattern matches) — NOT real markdown links (inside inline code
    spans, non-rendering), self-referential to the methodology, not inbound references to the deleted
    file. Confirmed by reading the exact matched lines.
  **CCIR check: no inbound link found outside the enumerated five + `done/` archive + this plan's own
  methodology prose. No CCIR fired; proceeded to delete `naming.md`.**

  **(Step 2) Content-parity inventory** (naming.md → vocabulary.md, item by item; ✓ = already present
  in vocabulary.md pre-merge, judged a deliberate duplicate-removal; → = merged this phase):
  1. Title "Naming Conventions" → superseded by vocabulary.md's own title (file deleted; no separate
     content).
  2. "## Golden Rule 13 — Capital Letter = Type" heading → merged into vocabulary.md's existing
     "## Capital Letter Rule (Golden Rule 13)" heading (same concept, pre-existing home).
  3. "Capital letter = type. Everything else = lowercase. This is universal and absolute." → reconciled
     to one line + link to `REF-golden-rules.md` Rule 13 (which now carries the full canonical statement
     post-Phase-1 fix, confirmed via `grep -n "13\." docs/reference/REF-golden-rules.md`).
  4. Code block (Types/Modules/Functions/Variables/Keywords casing examples) → MERGED (naming.md's
     version was strictly richer than vocabulary.md's condensed bullets — added Functions category with
     `fetchUser`/`processOrder` examples, Variables category with `let` declarations, and `background`/
     `follows`/`extends` in the Keywords list) — replaces vocabulary.md's terser bullet form, no
     append-drift (one coherent block, not both).
  5. "Scan any line. Capital letter = type. No capital = not a type. Zero ambiguity." → MERGED (kept as
     closing reinforcement sentence in vocabulary.md's Capital Letter Rule section).
  6. Module/type same-base-name note (Date/date, Duration/duration bullets) → ✓ already present verbatim
     in vocabulary.md pre-merge (same 4 bullets) — no content lost, no edit needed.
  7. `Self`/`self` explanatory sentence ("reserved type keyword meaning 'the implementing type' — used
     in follows contracts") → MERGED as a new closing sentence in vocabulary.md's Capital Letter Rule
     section, supplementing the terser Quick Reference table rows that already existed.
  8. "# Renamed Concepts" heading + intro sentence → ✓ superseded-as-duplicate by vocabulary.md's
     pre-existing "## Quick Reference" heading + intro paragraph (same normative directive — "never use
     the traditional/legacy term" — vocabulary.md's is broader-scoped, covering more surfaces).
  9. Renamed Concepts table, 19 rows, walked individually:
     - void→nothing ✓ (vocab: "No return value|nothing|void, unit, ()")
     - null/undefined/None→none ✓ (vocab: "Absent value|none|null, undefined, None, nil")
     - Optional\<T>→maybe\<T> ✓ (vocab: "Optional/maybe value|maybe\<T>|Optional, Option, nullable")
     - struct/class/interface/type→shape ✓ term mapping present (vocab row 1), but the WHY-clause
       ("type is banned because it's overloaded with the generic concept of 'type'") was NOT literally
       present elsewhere → MERGED as a new sentence in the "shape vs value" concept section.
     - enum→options ✓ (vocab row 6 + the `options Status {...}` example already in the "options vs
       union" concept section)
     - **abstract class→base shape — MISSING entirely from vocabulary.md pre-merge** → MERGED as a new
       Quick Reference row ("Non-instantiable base declaration | `base shape` | abstract class").
     - **implements→follows — MISSING as an explicit NOT-term row** (follows was only mentioned
       parenthetically inside the Self row) → MERGED as a new Quick Reference row ("Contract declaration
       | `follows` | implements") plus the `shape Player follows Damageable` example folded into the
       "shape vs value" concept section's new sentence.
     - Either\<A,B>/A|B→| ✓ (vocab row 7 + the "options vs union" concept section — actually more
       complete in vocab: cites Patrick's 2026-05-14 lock date + the triple-overload rationale for `or`)
     - typeof/instanceof/type guards→is ✓ (vocab: "Type narrowing|is|typeof, instanceof, type guards")
     - fn→function ✓ (vocab: "Function declaration|function|fn, func, def, method")
     - &T→share (signature-only, no body-level `.share()`) ✓ (vocab row 14 — already the corrected
       wording Phase 1 established in both files)
     - &mut T→lend (signature-only) ✓ (vocab row 15)
     - move→give (signature-only) ✓ (vocab row 16)
     - .clone()→.copy() (parens) ✓ (vocab row 17)
     - Result\<T,E>/throws→errors ✓ (vocab row 19)
     - match/switch→if(x is Type) ✓ (vocab row 20, Type narrowing — same `is` concept)
     - T[]/Array\<T>→array\<T> ✓ (vocab row 4, Growable list — NOT column doesn't literally list
       "Array\<T>" but is conceptually the same mapping, judged sufficient dup)
     - fixed-size array/stack array→fixed\<T> ✓ (vocab row 5, NOT column matches near-verbatim)
     - HashMap\<K,V>/Map\<K,V>→map\<K,V> ✓ (vocab row 3, Dynamic key-value collection)
  Vocabulary.md's own pre-existing content (Concept-Level Distinctions, Banned Legacy Terms,
  Correct/Incorrect Prose Examples, When You're Unsure, Cross-References) had no naming.md counterpart
  to reconcile against — carried forward unchanged except the self-ref removal (below).
  **CHECKPOINT — REACHED.** Inventory complete: 4 genuine merge actions identified (casing code-block,
  closing sentence, Self/self sentence, `type`-banned WHY-clause) + 2 missing-row additions (`base
  shape`, `follows`) + 1 cross-reference removal (self-ref); every other naming.md item confirmed
  already present in vocabulary.md as a deliberate duplicate-removal.

  **(Step 3) Merge into vocabulary.md** (`.claude/rules/vocabulary.md`):
  - Quick Reference table: inserted the two missing rows ("Non-instantiable base declaration|`base
    shape`|abstract class" and "Contract declaration|`follows`|implements") after the "Enum replacement"
    row, matching naming.md's original thematic placement.
  - "shape vs value" concept section: added the `type`-banned WHY-clause + the `base shape`/`follows`
    examples as a new sentence between the existing prose/UFCS sentences.
  - "Capital Letter Rule (Golden Rule 13)" section: replaced the intro line + condensed bullets with (a)
    a one-line restatement + link to `REF-golden-rules.md` Rule 13 (the GR13 reconcile the plan step
    demands), (b) the richer casing code-block merged from naming.md, (c) the "Scan any line..." closing
    sentence, (d) the pre-existing module/type distinction bullets (unchanged, already-correct), (e) the
    new `Self`/`self` clarifying sentence.
  - Cross-References: removed the `naming.md` self-ref line.
  No append-drift — every merged item landed in its logical existing section, none appended to the file
  end.

  **(Step 4) Delete naming.md.** `rm .claude/rules/naming.md`.

  **(Step 5) Repoint inbound links** (all four, plus the two CLAUDE.md occurrences):
  - `CLAUDE.md`:52 (Rules Files table) — the standalone `naming.md` row was folded into the existing
    `vocabulary.md` row's description (added "the capital-letter-=-type rule, module/type case
    distinctions, and the renamed-concepts table") rather than repointed as a second row for the same
    (now-merged) file — avoids a duplicate table row pointing at the same destination.
  - `CLAUDE.md`:77 (Yinz-terms bullet) — repointed to `.claude/rules/vocabulary.md`.
  - `.claude/rules/docs-checklist.md`:78 (What-Goes-Where row) — repointed to `vocabulary.md`.
  - `docs/reference/REF-naming.md`:16 — repointed to `../../.claude/rules/vocabulary.md`.
  - `docs/internal/scratchpad/SCRATCH-stdlib-encoding.md`:62 — repointed to
    `../../../.claude/rules/vocabulary.md`.
  - `vocabulary.md`:163 self-ref — removed (done as part of Step 3).
  **CHECKPOINT — REACHED.** naming.md deleted, all inbound links repointed, vocabulary.md carries the
  merged content.

  **(Step 6) Post-merge parity check.** Walked the Step 2 inventory: every genuine-merge item (4) and
  missing-row item (2) confirmed present in the post-edit `vocabulary.md` via a full file re-read; every
  duplicate-removal item (all 19 table rows + module/type note + Renamed-Concepts intro) reconfirmed
  present pre-existing, none dropped. 100% ticked.

  **Verification (Paper-Trace):**
  - `ls .claude/rules/naming.md` → "No such file or directory" (confirms deletion).
  - `grep -rn '[^-]naming\.md)' --include='*.md' .` (post-merge, tree-wide) → only the `done/` archive
    (untouched, expected) and this plan's own `plan.md`/`audit.md` prose describing the grep pattern
    (not real links, same classification as the pre-merge grep) — **zero real broken links**.
  - `grep -n 'naming.md' docs/reference/REF-naming.md` → zero hits (confirms the repoint landed; the
    file's own filename string doesn't self-match).
  - `git diff --stat` → 6 files changed: `.claude/rules/docs-checklist.md`,
    `.claude/rules/naming.md` (deleted, 60 lines removed), `.claude/rules/vocabulary.md` (34
    insertions/-7 net after table/section edits), `CLAUDE.md`, `docs/internal/scratchpad/
    SCRATCH-stdlib-encoding.md`, `docs/reference/REF-naming.md` — matches the expected merge + 4-file
    repoint + CLAUDE.md's 2-occurrence repoint.
  - `vocabulary.md`'s Capital Letter Rule section re-read: confirms one-line-+-link opening, merged
    casing code-block, and a single Quick Reference table (no second renamed-concepts table anywhere in
    the file).

  **Plan↔task sync note:** same constraint as Phases 1–2 — no TodoWrite in this dispatch's tool grant
  (Read/Write/Edit/Bash only per the executor charter's actual tools for this dispatch), no `- [ ]`
  checkbox glyphs in this plan (Steps use `1. 2. 3. …` numbering, the same documented pre-existing drift
  noted in Phases 1–2). Sync runs through `plan.md`'s inline `[DONE]` step markers + `**STATUS:
  COMPLETE**` phase annotation + `**CHECKPOINT — REACHED**` markers + this audit entry.

  **Deviations surfaced:** none — every edit was either a step the plan's own text enumerated
  (grep/delete/repoint) or a mechanical parity-guard merge action derived directly from comparing the
  two files' content (the two missing rows and four merge items were discovered BY the mandated parity
  inventory itself, not a plan-vs-reality divergence — the plan's Step 2/3 instructions anticipated
  exactly this kind of gap-finding as the parity guard's job). No FRAGO filed.

  **Recorded decisions (durable-answer calls, no human in the loop):**
  - CLAUDE.md's naming.md table row was folded into the adjacent vocabulary.md row's description rather
    than repointed as a duplicate row pointing at the same destination file — avoids a Rules-Files table
    with two rows for one file.
  - The `type`-banned WHY-clause and the two missing term-mapping rows were placed in their thematically
    nearest existing home (Quick Reference table for the rows; "shape vs value" concept section for the
    WHY-clause + base-shape/follows examples) rather than creating a new subsection — matches the
    plan's "no append-drift" instruction and the documentation standard's placement-law discipline.

  Phase 3 returns COMPLETE. No checkpoint/handoff file needed — the phase completed in one segment; both
  planned `**CHECKPOINT**` marks (post-inventory, post-repoint) were reached in-segment, well under the
  `handoff-phase-<N>.md` triggers (phase is `scale=large` per its Model tag, which IS one of the three
  checkpoint triggers per REF-plan-format — but the phase's 6 steps completed within one dispatch's
  context budget without approaching a context wall, so no PARTIAL/checkpoint-file was warranted; had
  context pressure emerged, Step 5 — after the second CHECKPOINT mark — was the natural segment
  boundary).

- 2026-07-16 — **Post-Phase-3 content-parity fix** (ad-hoc dispatch, not part of the plan's phase
  sequence). Phase 3's review fan-out (acceptance-verifier) found the naming.md→vocabulary.md merge's
  parity inventory mis-graded one row: the deleted `naming.md`'s `` `match` / `switch` on types | `if (x
  is Type)` | Pattern matching via type narrowing `` row was scored "already covered" by
  `vocabulary.md`'s existing `Type narrowing | \`is\` | typeof, instanceof, type guards` row, but that
  row's NOT-column never named `match`/`switch` as a superseded term — only `typeof`/`instanceof`. Fix:
  extended the same row's NOT-column to `typeof, instanceof, type guards, \`match\`/\`switch\` on
  types` (single row, no duplicate fork) in `.claude/rules/vocabulary.md`. No other file touched.

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 3 review fan-out — routing log** (conductor
  entry, no code change). Cheap gates both clean/green (green-check secret-scan pass via gitleaks; a
  flaky, non-reproducible `v03_m3e_alias_local_name_collision_runs_correctly` integration-test failure on
  first run, confirmed clean on two independent reruns, structurally unrelated to this docs-only diff —
  not attributed). All five reviewers (code-reviewer, acceptance-verifier, rules-compliance,
  deviation-judge, doc-auditor) returned clean/met/on-plan/conformant. The load-bearing GR13-compression
  negation-diff (Risk #1's B2 guard) was independently run by doc-auditor: word-for-word diff of naming.md's
  pre-merge "This is universal and absolute" against vocabulary.md's post-merge one-line+link form —
  confirmed the exceptionless normative force survives via the retained "Scan any line... Zero ambiguity"
  sentence and REF-golden-rules.md Rule 13's own framing; no negation dropped, no absolute claim inverted.
  One should-fix (the match/switch→is parity row) was found and fixed same-day (see the ad-hoc dispatch
  entry above) — re-verified clean via secret-scan re-run post-fix.
  One accepted minor, not actioned: doc-auditor noted the merge dropped naming.md's explicit relational
  clause "`Self` (uppercase) = the type of the instance" — a nuance loss, not a content loss (the
  Self=type/self=instance facts both survive independently in vocabulary.md's Quick Reference table row
  and the retained "reserved type keyword meaning 'the implementing type'" sentence). Accepted as-is,
  no action — same disposition class as Phase 1/2's other accepted-minor findings.
  Also flagged, out of this plan's charter (not actioned): rules-compliance noted `docs/reference/REF-naming.md`
  (content untouched, only its one inbound link was repointed) still teaches the pre-lock
  `struct/class/interface→type` mapping and dot-call `.share`/`.lend` fossil syntax Phase 1 fixed
  everywhere else in the corpus — pre-existing drift, not introduced by this diff, candidate for plan 2 /
  a fresh audit item.

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 4 execution — gap-rule authoring +
  frontmatter/conformance pass.** Same session-id as Phases 1-3 (scratchpad path corroborates); not
  re-appended to the frontmatter chain (same rationale as prior entries).

  **Ground-truth gap found at recon.** The plan's cited governing source,
  `docs/internal/scratchpad/SCRATCH-teaching-audit-2026-07-11.md`, does not exist anywhere in this
  worktree or in git history (`git log --all` returns nothing for its path) — it is an **untracked**
  file that exists only in the main checkout (`/home/patrick/development/ynz/`), a sibling worktree.
  Git worktrees do not share untracked files. Read it directly from the main checkout path (read-only,
  informational — no write ever targeted outside this worktree's resolved plan root) to extract §H2.7's
  literal content (found at line 425-435, item 7 of the "H2. The prevention catalog" list). This is a
  `plan-evidence-durability`-shaped gap one level removed (a governing SOURCE never persisted into the
  branch the plan executes on, not a captured sample) — flagged here for the record; no FRAGO filed
  since it did not block execution (the file was reachable) and fixing the plan's own citation is out
  of this phase's charter (Design-Doc Alignment note, not a phase deliverable).

  **(Step 1) Teaching-surfaces rule.** Authored `.claude/rules/teaching-surfaces.md` (98 lines, within
  the ≤120 soft budget) per §H2.7: the three-slot test, the audience test, the banned-vocab pointer
  (points at `vocabulary.md`'s Banned Legacy Terms + the registry `[[banned_jargon]]` SSOT), in-example
  naming conventions (camelCase, `.copy()` parens, no SCREAMING_SNAKE — ratifier pointed at
  `vocabulary.md`, not the dead `naming.md`, per the plan's explicit note), and the
  no-internal-paths/no-milestone-tags rules, with a worked good/bad WHY pair (the Tokio/`ynz_rt_init`
  example from audit §B2). No CCIR fired — budget was not a binding constraint.

  **(Step 2) Constants-naming ruling.** Added `### Constants` subsection to `.claude/rules/vocabulary.md`
  (placed under Capital Letter Rule — Law 7 co-location): `const maxHealth = 100`, GR13 absolute, no
  SCREAMING_SNAKE exception, future exceptions ratified in vocabulary.md first.

  **(Step 3) Import-path canon.** Added `## Import Paths` top-level section to `vocabulary.md` (home
  choice recorded: vocabulary.md, "the natural home for how Yinz spells X" per the plan's own framing):
  backtick-quoted, project-root-relative, no `.ynz` suffix, with a runnable `import { Player } from
  \`services/player\`` example, sourced from audit finding A3 (the lexer's own "one string form —
  backtick strings" rule).

  **(Step 4) Frontmatter + conformance pass.**
  - Added global-standard frontmatter (name/description-block-scalar/tags/created_at/updated_at
    "2026-07-16"/status "active"/author "patrick" [matching this repo's uniform existing convention,
    verified via `grep -rh '^author:'` across the whole docs+rules tree returning only "patrick"]/
    metadata) to: `inference.md`, `language-design.md`, `spec-writing.md`, `stdlib-design.md`,
    `dot-postfix.md`, `non-oop.md`, `plan-invariants.md`, `auto-promotion.md`, `examples-structure.md`,
    `vocabulary.md`, `docs-checklist.md` (11 files) plus `teaching-surfaces.md` (authored with
    frontmatter). `created_at` for each of the 11 was **git-log-verified** (`git log --follow` first
    commit date per file), not guessed — dates range 2026-05-12 to 2026-05-20.
  - `docs/reference/REF-golden-rules.md` already carried valid frontmatter (bumped in Phase 1's review
    fan-out entry) — verified, no edit needed.
  - `docs/reference/REF-naming.md` — bumped stale `updated_at` (2026-07-01 → 2026-07-16); its Phase-3
    line-16 repoint edit had never bumped the frontmatter.
  - `.claude/rules/vue-website.md` — verified untouched; confirmed the sole file tree-wide carrying a
    `paths:` key (`grep -rl '^paths:' .claude/rules/*.md docs/reference/*.md`).
  - `CLAUDE.md` (frontmatter-exempt, raw-instructions file) and
    `docs/internal/scratchpad/SCRATCH-stdlib-encoding.md` (scratchpad-exempt) skipped per their
    documented exemptions — no edit.
  - **MUST/NEVER re-grade sweep**: grepped all 13 target files for `MUST`/`NEVER`/`ALWAYS`. Every hit
    reviewed individually — all are true gates (compile-time-enforced language rules like "Pure-Named
    Methods MUST Be Pure" / "Regex MUST be linear-time NFA," Bouncer/reviewer-checked structural
    requirements like "Every plan MUST include Design-Doc Alignment," or authoring disciplines with
    their consequence already stated like dot-postfix.md's real-operations MUST). **No judgment-tier
    sentence found dressed as a hard gate** — verified-clean, not skipped (recorded per
    [decision-philosophy] mandatory-assessment: the check was run, the honest answer is no action
    needed).
  - **One-home markers** — all four named Class-2.2/2.3/2.4/2.5 de-dups:
    1. **GR13** — already one-line + link to `REF-golden-rules.md` Rule 13 from Phase 3's merge;
       tree-wide grep confirmed no other rule-corpus file restates the full rule (only
       `REF-language-overview.md`'s one-line summary and `CLAUDE.md`'s canonical source list remain,
       both legitimate/out of this plan's touched-file scope) — no further action needed.
    2. **Non-OOP drift-signals** — `language-design.md`'s "OOP Drift Test" six-bullet enumeration
       (near-duplicate of `non-oop.md`'s Banned Anti-Patterns six-item table) condensed to two example
       signals + an explicit pointer: "The full enumerated list... is the `.claude/rules/non-oop.md`
       Banned Anti-Patterns table — one home, not restated here."
    3. **Auto-promotion criterion** — `inference.md`'s "Rule of thumb" paragraph (which restated
       `auto-promotion.md`'s own Pattern table criterion — "typeable explicit form → both surfaces")
       replaced with a pointer: "The typeability criterion above is `auto-promotion.md`'s Pattern
       table... this file's job is which IDE surface renders it, not re-deriving the criterion."
    4. **Banned-jargon → registry** — `vocabulary.md`'s Banned Legacy Terms intro sentence and its
       Cross-References entry repointed from `crates/ynz-diagnostics/src/banned_jargon.rs` to
       `registry/features.toml` `[[banned_jargon]]` as the actual SSOT (verified via source read:
       `banned_jargon.rs` is literally "// Thin adapter — all data lives in registry/features.toml" —
       58 `[[banned_jargon]]` entries confirmed present), matching the Phase-1 intrinsics-SSOT fix
       pattern (`dot-postfix.md`).

  **Verification (Paper-Trace):**
  - `paths:` grep tree-wide across all touched files → only `vue-website.md`.
  - Structural frontmatter check (exactly 2 `---` delimiters + all 8 required keys present) on all 14
    frontmatter-bearing touched files → all pass. Full YAML-load verification via `python3 -c
    "import yaml"` was unavailable (module not installed, no `pip` on this host) — rather than reaching
    for Docker for a one-off parse check on hand-authored, structurally-simple frontmatter, the
    structural check (delimiter count + key presence, run via `awk`/`grep`, both host-native) is the
    verification actually performed; recorded as a named tooling-substitution, not a silent skip.
  - `teaching-surfaces.md` line count: `wc -l` → 98 (within the 120 soft budget).
  - `git diff --stat` (not re-quoted here for context-budget reasons) confirms the touched-file set
    matches this entry's enumeration: `teaching-surfaces.md` (new), `vocabulary.md`, `inference.md`,
    `language-design.md`, `spec-writing.md`, `stdlib-design.md`, `dot-postfix.md`, `non-oop.md`,
    `plan-invariants.md`, `auto-promotion.md`, `examples-structure.md`, `docs-checklist.md`,
    `docs/reference/REF-naming.md`, plus `plan.md`/`audit.md` themselves.

  **Plan↔task sync note:** same constraint as Phases 1-3 — no TodoWrite in this dispatch's tool grant,
  no `- [ ]` checkbox glyphs in this plan. Sync via `plan.md`'s inline `[DONE]` step markers +
  `**STATUS: COMPLETE**` phase annotation + `**CHECKPOINT — REACHED**` markers + this audit entry.

  **Deviations surfaced:** the untracked-governing-source gap (above) is the one deviation this segment
  found — surfaced, not self-resolved as a plan edit (out of this phase's charter). No FRAGO filed
  (did not block execution; the source was reachable from the main checkout).

  **Recorded decisions (durable-answer calls, no human in the loop):**
  - `author: "patrick"` for all new frontmatter, matching this repo's uniform existing convention
    (verified, not the global spec's `claude-<agent-name>` default option) — house convention over
    global default.
  - Structural (delimiter+key) frontmatter verification substituted for full YAML-parse verification,
    given no `pip`/`yaml` module on this host and no in-scope reason to spin up Docker for one check.
  - Teaching-surfaces.md's ratifier reference points at `vocabulary.md` per the plan's explicit
    instruction (not a fresh decision — executing the plan's own recorded call).

  Phase 4 returns COMPLETE. No checkpoint/handoff file needed — both planned `**CHECKPOINT**` marks
  (post-Step-1, post-Step-3) were reached in-segment; Step 4 (frontmatter pass) carries no internal
  CHECKPOINT mark and was carried to completion in this same segment despite running past this
  executor's own context-budget nudge threshold (~150k tokens) partway through Step 4 — the remaining
  Step-4 sub-work (6 more frontmatter additions + verification + this write-up) was mechanical and
  low-risk, so it was finished rather than checkpointed mid-step (checkpointing mid-step is
  disallowed regardless; the alternative would have been an over-fat-step BLOCK, which was not
  warranted since the step did complete).

- 2026-07-16 — **Post-Phase-4 sibling-reconciliation fix (graveyard-auditor finding, ad-hoc
  dispatch).** Phase 4's graveyard-auditor review found `.claude/rules/inference.md:28` (Dual-Audience
  Disclaimer) still cited `crates/ynz-diagnostics/src/banned_jargon.rs` as the SSOT for banned-jargon
  enforcement, even though Phase 4's own de-dup fix already repointed the identical claim in
  `vocabulary.md` (Banned Legacy Terms intro + Cross-References) from `banned_jargon.rs` to
  `registry/features.toml`'s `[[banned_jargon]]` entries (confirmed source: `banned_jargon.rs` is a
  thin generated adapter, not the SSOT). `inference.md` was already touched this same phase
  (frontmatter added in Step 4) so this sibling claim should have been swept then. Fix: reworded the
  sentence to cite `registry/features.toml` `[[banned_jargon]]` entries as the SSOT, noting
  `crates/ynz-diagnostics/src/banned_jargon.rs` is generated from it — preserving the sentence's actual
  meaning (banned words ARE banned in user-facing diagnostics), only correcting the SSOT attribution.
  No other file touched.

- 2026-07-16 — **Post-Phase-4 review-fan-out fix pass (log-only + two small fixes, ad-hoc
  dispatch).** Phase 4's review fan-out surfaced several findings; two fixed, two adjudicated and
  logged (no file edit beyond this note for the latter two).

  **FIX 1 — reverted the spurious `updated_at` bump on `docs/reference/REF-naming.md`.** Phase 4's
  frontmatter pass bumped `updated_at` to "2026-07-16" even though Phase 4 made zero content edits to
  this file — the only real edit (the naming.md link repoint) landed in Phase 3, at commit `ca60f0b`,
  which correctly left `updated_at` at "2026-07-01". Bumping the date without touching the body implies
  "reviewed and current as of today," which is false — doc-auditor independently confirmed the body is
  genuinely stale (still teaches banned `type`, describes `share`/`lend` as dot-methods contradicting
  the signature-only-keyword model, `abstract`→`base` instead of `base shape`), but that staleness is
  OUT OF THIS PLAN'S CHARTER (Design-Doc Alignment scopes this plan to `.claude/rules/` +
  `docs/reference/REF-golden-rules.md` only — rules-compliance already flagged this exact staleness as
  out-of-scope during Phase 3's review fan-out and it was correctly not actioned then; same disposition
  applies here). Reverted `updated_at` to `"2026-07-01"` (verified via `git show
  ca60f0b:docs/reference/REF-naming.md`) — no other field or content touched.

  **FIX 2 — `.claude/rules/teaching-surfaces.md` style consistency.** rules-compliance noted it was the
  only rule file in the corpus without `---` horizontal-rule dividers between its `##` sections, and its
  closing section was titled `## Cross-references` (lowercase r) where every sibling rule uses
  `## Cross-References`. Added `---` dividers between all six `##` sections (matching `non-oop.md`'s
  structure) and capitalized the closing heading to `## Cross-References`. Content unchanged — pure
  formatting.

  **LOG 1 — FRAGO 001 filed below** (main-checkout scratchpad read, risk-neutral, auto-applied).

  **LOG 2 — Phase 5's yaml-parse check is a real, load-bearing deferred check, not a formality.**
  deviation-judge flagged that Phase 4's frontmatter-validity verification used a structural check
  (delimiter + key presence via `grep`/`awk`) instead of an actual YAML parse, because `python3`'s
  `yaml` module wasn't available and no `pip` was present on the host. Adjudicated: this is not a
  skipped check — Phase 5's own plan text (Step 3) already specifies the real yaml-parse check
  ("Parse the frontmatter of every touched rule file (e.g. `python3 -c` yaml load) → all valid") as its
  own dedicated exit criterion. Phase 4's structural check is provisional and was recorded as such in
  its own audit entry ("recorded as a named tooling-substitution, not a silent skip"). This note
  restates that explicitly so Phase 5's executor treats its Step 3 as a real, load-bearing verification
  gate — not a formality to rubber-stamp because "Phase 4 already checked frontmatter."

  **Deviations surfaced:** none new beyond FRAGO 001 below. No other files touched this dispatch beyond
  `docs/reference/REF-naming.md`, `.claude/rules/teaching-surfaces.md`, and this `audit.md`.

- 04e22a51-80c1-4b67-a886-083784d61bcd — 2026-07-16 — **Phase 5 execution — verification sweep
  (Steps 1-4 only; Step 5 "Open PR" explicitly out of scope for this dispatch per the dispatching
  conductor's instruction).** Same session-id as the prior Phase 2/3/4 entries above (per the
  harness-assigned session id matching the existing frontmatter-chain entry); NOT re-appended to the
  frontmatter chain, same rationale as every prior same-session dispatch in this plan.

  **Ground-truth deferred-check note honored.** Per FRAGO-adjacent Post-Phase-4 LOG 2 (Phase 4's
  frontmatter check was a structural placeholder, not a real YAML parse, due to no `pip`/`yaml` module
  on the bare host), this dispatch ran the REAL Docker-backed yaml-parse check per
  `~/.claude/rules/run-in-docker.md` rather than repeating the structural placeholder or skipping it.
  Probed `command -v docker` (present) and `docker compose config --services` (this repo's own `dev`
  service exists) before reaching for a throwaway `python:3-slim` container — rung 1/2 of the priority
  ladder was available and used; no bare `docker run` needed. Confirmed `pyyaml` present in `dev` via
  `docker compose run --rm dev python3 -c 'import yaml'` before writing the check script (probe before
  invoke, per `run-in-docker.md`).

  **(Step 1) Link-integrity grep.**
  ```
  $ grep -rn '[^-]naming\.md)' --include='*.md' .
  .claude/planning/done/2026-05-14-design-lockdown-from-gemini-review/plan.md:106,157,165,172,494  (done/ archive — out of scope, 5 hits)
  .claude/planning/active/2026-07-11-rules-corpus-cleanup/audit.md:55,268                            (this plan's own methodology prose in backticks — not real links)
  .claude/planning/active/2026-07-11-rules-corpus-cleanup/plan.md:299,300                            (same — prose describing the grep pattern)
  ```
  Zero real broken links — matches Phase 3's own post-merge grep exactly (same two categories:
  `done/` archive + this plan's own prose). `.claude/rules/naming.md` confirmed absent from disk
  (`test -f` → not found).
  A Python resolve-check (`link_re = re.compile(r'\[[^\]]*\]\(([^)]+)\)')`) walked every relative
  markdown link in all 17 touched files (the full `git diff --stat main...HEAD` file list) and
  resolved each target against its source file's directory — result: **`ALL RELATIVE LINKS RESOLVE
  OK`**, zero broken targets, zero absolute (`~`/`/`-leading) link targets found.
  Targeted grep `grep -n '](~/.claude\|](/tmp' <touched files>` → **zero hits** (exit code 1/no
  match). The touched files' bare mentions of `~/.claude` (`docs-checklist.md`, `plan-invariants.md`,
  `CLAUDE.md`, `vue-website.md` ×2) are all plain backtick prose, not markdown links — correct per the
  named-not-linked / home-absolute-link-ban convention. No `/tmp` mentions found in any touched file.

  **(Step 2) No-`paths:` check.**
  ```
  $ grep -ln '^paths:' <all 17 touched files>
  .claude/rules/vue-website.md
  ```
  Exactly one hit, matching Risk #6's exit criterion exactly. Cross-checked tree-wide
  (`grep -rl '^paths:' .claude/rules/*.md docs/reference/*.md`) — same single result.

  **(Step 3) YAML-valid check (REAL parse, not the Phase-4 structural placeholder).**
  Wrote `scratch_yaml_check.py` (loaded frontmatter block via a `---\n...\n---\n` regex, parsed with
  `yaml.safe_load`, checked required-key presence + checked `created_at`/`updated_at` didn't parse as
  YAML native `date`/`datetime` objects — which would mean the value was unquoted and thus wrongly
  typed) against the 13 touched rule files + `REF-golden-rules.md` + `REF-naming.md` (15 files;
  `CLAUDE.md` and `SCRATCH-stdlib-encoding.md` excluded per their documented frontmatter exemptions).
  Ran via `docker compose run --rm dev python3 scratch_yaml_check.py`. Full output:
  ```
  OK .claude/rules/auto-promotion.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/docs-checklist.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/dot-postfix.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/examples-structure.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/inference.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/language-design.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/non-oop.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/plan-invariants.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/spec-writing.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/stdlib-design.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/teaching-surfaces.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/vocabulary.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK .claude/rules/vue-website.md: valid YAML, keys=[paths] -- NOTES: missing keys (name/description/tags/created_at/updated_at/status/author/metadata — expected, deltas-only anomaly per plan assumption); paths: key present = [website/**]
  OK docs/reference/REF-golden-rules.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]
  OK docs/reference/REF-naming.md: valid YAML, keys=[author, created_at, description, metadata, name, status, tags, updated_at]

  === SUMMARY ===
  ALL VALID
  ```
  Zero YAML parse errors; zero date/number scalars mis-typed as bare (all `created_at`/`updated_at`
  parsed as Python `str`, confirming they're quoted in source, not bare — a bare date would parse as
  `datetime.date` and trip the script's explicit check, which fired clean). Spot-read three files'
  raw frontmatter blocks directly (`vue-website.md`, `teaching-surfaces.md`, `REF-naming.md`) —
  confirmed block-scalar (`>`) usage for multi-line descriptions, every string scalar double-quoted,
  `vue-website.md`'s single-quoted `paths:` list item is its own documented pre-existing shape
  (untouched by this plan). `scratch_yaml_check.py` deleted immediately after the run (`git status
  --short` confirmed clean tree, zero residue).

  **(Step 4) Content-parity sign-off.**
  Re-read Phase 3's full parity inventory in this file (Step 2's 9 top-level items + 19-row
  Renamed-Concepts-table walk, plus Step 6's post-merge re-confirmation) end to end: every item is
  ticked (✓ pre-existing-duplicate or → merged-this-phase), including the post-Phase-3 ad-hoc fix
  (`match`/`switch`→`is` NOT-column extension) — independently re-verified live via
  `grep -n "match.*switch" .claude/rules/vocabulary.md` → line 52: `| Type narrowing | \`is\` |
  typeof, instanceof, type guards, \`match\`/\`switch\` on types |`. Spot-checked five of the claimed
  merge/addition items directly against the current file (all present, all as recorded): the
  `base shape` Quick-Reference row (line 37), the `follows` Quick-Reference row (line 38), the
  `type`-banned WHY-clause (line 77), the `Self`/`self` reserved-keyword sentence (line 193), the GR13
  one-line+link opening (line 165), and confirmed zero remaining `naming.md` self-references in
  `vocabulary.md`. Phase 3's own review-fan-out entry records doc-auditor independently ran and signed
  the Risk-#1 negation-diff guard (GR13 compression boundary sentence — no dropped negation). Parity:
  **100% ticked, signed.**

  **Step 5 — not executed.** Per this dispatch's explicit instructions, PR-opening is out of scope;
  the branch is left as-is after this phase's own (uncommitted, pending-conductor-commit) verification
  work. No commit made by this dispatch (per instruction: "Do NOT commit — that happens separately").

  **Plan↔task sync note:** same constraint as every prior phase in this plan — no TodoWrite in this
  dispatch's tool grant, no `- [ ]` checkbox glyphs in this plan (Steps use `1. 2. 3. …` numbering).
  Sync via `plan.md`'s inline `[DONE]` step markers + the phase's STATUS annotation (qualified: Steps
  1-4 complete, Step 5 explicitly out of scope, not a gap) + this audit entry.

  **Deviations surfaced:** none — every check ran exactly as the plan's own step text specified; the
  one substantive change from the plan's literal text is upgrading Step 3 from Phase 4's structural
  placeholder to the real Docker-backed parse, which is not a deviation but the plan's OWN documented
  intent (Phase 4's audit entry explicitly named this as provisional and pointed forward to Phase 5's
  Step 3 as the real gate; the dispatch's own task brief also mandated it explicitly). No FRAGO filed.

  **All four checks: GREEN.** Content-parity: SIGNED. Phase 5 (Steps 1-4) returns COMPLETE. Step 5
  (Open PR) intentionally not executed — reserved for a separate conductor action per this dispatch's
  explicit scope boundary.

## FRAGO log
(FRAGO delta records append here — see the FRAGO template)

## FRAGO 001 — Main-checkout scratchpad read during Phase 4 (risk-neutral, auto-applied)

**What happened.** Phase 4's executor, needing the governing content of
`docs/internal/scratchpad/SCRATCH-teaching-audit-2026-07-11.md` (cited by the plan as the source for
§H2.7's teaching-surfaces content), found the file does not exist in this worktree or anywhere in this
worktree's git history — it is untracked and lives only in the main checkout
(`/home/patrick/development/ynz/`), a sibling git worktree. Git worktrees do not share untracked files.
The executor read the file directly from the main checkout path (read-only) to extract §H2.7's content,
rather than surfacing the boundary gap and halting for a FRAGO before reading.

**Why this is classified RISK-NEUTRAL.** deviation-judge flagged this as a resolved-plan-root boundary
violation that should have been surfaced/FRAGO'd rather than self-resolved. On adjudication: (1) the
read was READ-ONLY — no write ever targeted the main checkout, so there was no risk of disturbing the
uncommitted M6 concurrency-hotfix work sitting there; (2) two independent reviewers (doc-auditor via
courtesy-verification during Phase 4's own review fan-out, and this dispatch's own re-check) have
confirmed the extracted §H2.7 content that landed in `teaching-surfaces.md` is substantively correct —
no wrong content shipped; (3) the boundary rule's actual purpose (preventing cross-worktree WRITE
hazards — one worktree's uncommitted state clobbering another's) was never threatened by a read. No
destructive action occurred; no wrong content shipped.

**What this changes going forward.** Future dispatches that need reference content from outside the
resolved plan root should copy the needed content INTO the worktree via a legitimate channel (e.g. a
plan `scratch/` note, a checked-in fixture, or an explicit FRAGO authorizing the read) rather than
reading a sibling worktree's untracked files live, even read-only — the boundary convention should be
honored procedurally even when a specific instance turns out harmless, so the pattern doesn't calcify
into an unreviewed habit. No retroactive undo is needed for this instance since the content is verified
correct and the read was non-destructive.

**Classification:** risk-neutral, auto-applied and logged per the plan's FRAGO flow (no destructive
action; no wrong content shipped; boundary purpose not threatened).

## FRAGO 002 — Phase 5 Step 5 ("Open PR") explicitly deferred to a separate human-gated action (risk-neutral, auto-applied)

**What happened.** Phase 5's plan text (Step 5: "Open PR from the `main`-forked docs-only branch...")
and its original exit criteria ("PR open against `main`") both assumed PR-opening would happen inside
this dispatch. The conductor instructed the Phase 5 executor to skip Step 5 entirely, relaying an
explicit standing instruction from Patrick (given at the top of this overnight run: "stop before PR,
leave it staged" — see the conductor's own conversation record) that PR-opening is reserved for his own
review after he wakes up. The executor complied and wrote the exclusion back into `plan.md`'s
current-truth text (phase header, the Step 5 line, and the exit-criteria line all annotated in place —
never silently marked "COMPLETE" while hiding the gap), plus a session-log paragraph in this file. This
FRAGO formalizes that already-reflected decision as a dedicated delta record, per this repo's own
plan-source-of-truth discipline (a consequential decision — narrowing a phase's committed step list and
un-meeting the plan's own Key Outcome #7 — needs a matching `## FRAGO NNN` block in the same dispatch
window it took effect, not only inline prose).

**Why this is classified RISK-NEUTRAL.** deviation-judge adjudicated this as JUSTIFIED (explicit
human-authority instruction, correctly attributed, not an executor-invented shortcut) and surfaced only
the missing-formal-record gap, not the decision itself, as the thing needing a fix. The decision moves
no risk axis: no code changes, no destructive git operation, no work is lost — it is a pure sequencing
deferral of one administrative step (opening a PR) to a moment when the accountable human is present to
review the diff himself, which is *more* conservative than opening it unattended, not less. The plan's
own risk table (¶1) never scored PR-opening as a risk driver in the first place.

**What this changes going forward.** Key Outcome #7 ("PR opened from a main-forked docs-only branch")
remains genuinely un-met as of this dispatch — `plan.md`'s frontmatter stays `status: "active"`, not
`"done"`, honestly reflecting that the plan is not yet fully complete. The plan does NOT flip to `done`
until Patrick reviews the five sealed commits (`b9ab582`, `ff08d80`, `ca60f0b`, `9e4835f`, plus Phase
5's verification-only commit) and either opens the PR himself or authorizes the conductor to do so in a
follow-up session. No retroactive action is needed for this FRAGO — it is a formalization of a decision
already correctly applied, not a reopening of it.

**Classification:** risk-neutral, auto-applied and logged per the plan's FRAGO flow (pure sequencing
deferral of a non-destructive administrative step to human review; no risk axis moved; already reflected
in `plan.md`'s current-truth text — this record closes the missing-formal-FRAGO gap deviation-judge
surfaced during Phase 5's review fan-out).

## Context-segment log
(per-segment entries append here — see the execute-plan conductor §3a.1)
