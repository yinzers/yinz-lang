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

## FRAGO log
(FRAGO delta records append here — see the FRAGO template)

## Context-segment log
(per-segment entries append here — see the execute-plan conductor §3a.1)
