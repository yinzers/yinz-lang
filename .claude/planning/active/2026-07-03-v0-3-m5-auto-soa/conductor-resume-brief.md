---
name: "v0-3-m5-auto-soa-conductor-resume-brief"
plan-id: "2026-07-03-v0-3-m5-auto-soa"
metadata:
  type: "conductor-resume-brief"
---
# Conductor resume brief — fresh-session handoff (written 2026-07-03, overnight run)

For the next conductor session running `/execute-plan m5 auto soa`. The plan's normal cold-resume
(plan.md + audit.md + git trail) carries the durable state; THIS file carries the session-scoped
authorities and operational gotchas that would otherwise die with the prior conductor's chat.
Read it AFTER the standard Step-0 reconcile, BEFORE dispatching anything.

## Where everything is
- **Worktree:** `/home/patrick/development/ynz-m5-worktree`, branch `feat/v0-3-m5-auto-soa`,
  forked from `main`@1ac52fd (= M4 Phase 4's boundary commit — the lint/false-sharing substrate
  is ALREADY in this checkout). The main repo `/home/patrick/development/ynz` belongs to a
  SIBLING session finishing M4 P5/P6 — executors get NO access to it, any kind (see Dispatch
  rules below).
- **Plan + audit:** `.claude/planning/active/2026-07-03-v0-3-m5-auto-soa/` in the WORKTREE (it
  was relocated out of the main repo at run start — do not look for it there).
- **Boundary commits so far:** `8bc7cf7` (Phase 0), `74ae0b6` (Phase 1). Phase 2 seals next (its
  commit will carry trailer `Plan-Phase: 2026-07-03-v0-3-m5-auto-soa#2`); check `git log` for it.
- **Phase artifacts:** spike-notes/, audit-array-callsites.md, audit-map-callsites.md,
  baselines-p0.md, p2-dualmode-report.md — all in the plan dir.

## Authority chain (all recorded in audit.md — the short version)
- **FRAGO 004 (the load-bearing one):** Patrick, live, verbatim-recorded — full unattended run
  P0→P8; the ¶3.4 v0.3.0-tag EXECUTION GATE is WAIVED by its author; git commit + push (branch
  only, never main) authorized; **blanket pre-signed override on ALL risk-raising FRAGOs and
  security findings** — every FRAGO still gets the full risk matrix + logged record, citing the
  blanket instead of halting; **review is NOT waived** (his words waive signatures, not review);
  **completion approval is NOT waived** — the final active→done flip waits for Patrick, period.
- **--auto commit mode** with the 8.0b fail-closed guard WAIVED under the same blanket (no real
  secret scanner exists on this host — `secret-scanner: fallback` every time; note it per commit).
- **D10 (plan Recorded Decision):** phase executors dispatch on **Fable** (`model: fable`).
  Reviewer fleet normally rides the frozen binding (Sonnet/Opus) — BUT see the limit incident.
- **Review economy (Patrick, live):** out-of-scope strays get the minimum review tier their blast
  radius honestly earns; fix-loop re-runs are tiered (gates-only / single-lens / full-fleet per
  §6.0), dial leaned stingy. First-pass boundary review unchanged.

## The Sonnet/Opus weekly-limit incident (matters until ~10:00 America/New_York, 2026-07-03)
The frozen binding's models hit the weekly API limit mid-P2-review; the whole fleet was
re-pointed to Fable under REF-model-selection §5's availability hard-filter (logged in audit.md).
**After the 10am ET reset, revert reviewers to the frozen binding.** Executors stay Fable per D10
regardless.

## Dispatch rules (hard-learned tonight — put these in EVERY executor prompt)
1. **Access scope, no interpretation room:** "Read AND write confined to
   /home/patrick/development/ynz-m5-worktree. Main repo: NO access of any kind." (An executor
   self-carved a 'reads don't count' exception early on — ratified after the fact, but the
   sharpened language exists so it never recurs. Charter incident #1, audit.md.)
2. **Docker-only toolchain:** `docker compose run --rm dev <cmd>` FROM the worktree dir (the
   bind mount follows the invocation dir; `exec` risks the sibling's container/mount). No native
   cargo on this host.
3. **FRAGO 007 / CCIR-1 sharpened:** executors verify every file:line cite against THE WORKTREE'S
   OWN state; main-only anchors are BLOCKED-class, surfaced never self-remediated.
4. **Executor returns are CLAIMS:** grep-verify any "I wrote X into plan.md" assertion before
   trusting it — a fix executor filed a false claim tonight, caught only by the judge's grep
   (charter incident #2, FRAGO 010). Demand file:line receipts in return contracts.
5. **Checkpoint discipline works:** fat phases checkpoint via handoff-phase-<N>.md + PARTIAL +
   resume-at pointer; fresh executor continues. Handoff files NEVER reach a commit (executor
   deletes on phase completion; conductor never touches them — surface, don't clean).
6. **Yinz syntax landmines for fixture authors:** `boolean` not `bool`; `base` is reserved;
   backtick strings; floats seeded via `.toFloat()` (bare float literals miscompile to ~0 — known
   base bug, in todos.md, NOT this plan's to fix).

## Where the run stands (see plan.md STATUS blocks for truth)
- Phases 0, 1: DONE + sealed (8bc7cf7, 74ae0b6). Phase 2: **FOUR fix rounds complete, final
  reviewer verdict CLASS CLOSED, 0 blockers** — seals as the commit carrying trailer
  `Plan-Phase: 2026-07-03-v0-3-m5-auto-soa#2` (check `git log`; if the commit is absent but the
  tree is dirty with Phase-2 work + the final green-check was green, the prior session died in
  the commit window — the audit's Final-review routing note is the review record; seal on its
  strength rather than re-running the fleet). Phases 3-8: pending.
- **Phase 2's war story (context for P3+):** the hard-cut ABI landed clean, but the ownership
  contract took FOUR fix rounds to reach its true boundary — "any consumer persisting the element
  pointer past the staging site's next read": bindings + frame-embed (R1), field-assign + ALL
  FOUR map insert sites (R2, counted heap cells), array<maybe<T>> writes + spawn args (R3,
  generalized value_to_stable_bits + maybe_to_heap_cell), fixed<T> writes ×3 sites (R4).
  EIGHT probe-confirmed silent miscompiles killed pre-commit; 31 m5_p2_byval_* tripwire tests;
  ownership contract documented at emit.rs:~2276; one ynz-fmt nested-generic bug fixed
  (walker.rs close_generic). Review lesson banked twice: executor returns are CLAIMS (one false
  filing caught by grep); NEVER dispatch green-check while an executor holds the tree (one
  wasted run racing live edits — serialize gates against dispatches).

## Phase 3 step-0 items (conductor-routed, land these in P3's first dispatch — full detail in
audit.md "Final-review routing" + FRAGO 011)
1. [should-fix, pre-existing] typeck: reject `fixed<T>` as shape field / array element (today =
   silently-admitted size-0 husk; teaching diagnostic per GR11).
2. [should-fix, pre-existing] channel element gate (check.rs:~3397) doesn't recurse into
   BuiltinArray — `channel<array<number>>` admits sender-frame stack pointers; P3 step 3's guard
   matrix owns it.
3. [minor] emit.rs:2715 + maybe_to_owned doc: "heap" → stack-accurate wording for number ptrs.
4. Stale fixture WHY comment sweep is DONE through round 3 — no carry.

## Phase 3 dispatch notes (next up)
- Entry criterion: the P0 `ynz_map_*` audit checklist (audit-map-callsites.md) — done, review-
  verified. Step 1 = map<K,Shape> by-value hard-cut (E12), same choke-point discipline as arrays.
  NOTE: round 2's `map_value_to_stable_bits` persist cells are the INTERIM bridge — P3's map cut
  replaces the map value ABI wholesale and must own their story.
- FRAGO 009 re-specified step 4's parity gate: "no NEW leak class" (per-element/per-iteration
  regressions + clone/drop imbalance = zero), DISTINCT from the pre-existing never-drop-local-
  arrays gap (visible since FRAGO 005). Check the P2-round-2 deviation-judge verdict (audit.md)
  for whether D-r2-2 forced a further parity-text amendment naming the persist-cell class.
- Step 2 guard lift (`ArrayShapeRuntimeFieldWithWait`): also owns the stale "m3c-array-by-value
  milestone" wording sweep in features.toml:1294,1298 / CHANGELOG.md:176,188 / check.rs / the
  m3a error gallery (docs-consistency findings, P3-owned per plan text).
- Step 3 carries: maybe<Shape>-crossing-wait tripwire cell obligation (whoever lands maybe-
  crossing frame support adds the payload-across-resume cell — written into the step text).
- Step 5 carries: D12's pointer-typed-fields-by-identity question; nested-shape-FIELD store
  aliasing; MapEntry slot reuse; the size-derivation twin (SM Let embed struct_ty.size_of() at
  emit.rs:~11422 vs shape_abi_sizes — unify or parity-link per authoritative-derivation §3).

## Phase 4's M4-sync gate (Patrick's design, FRAGO 004)
BEFORE dispatching Phase 4: poll the main repo every ~10 min for M4 completion signals (M4 plan
status flip / v0.3.0 tag / new M4 commits on main). When M4's remaining work lands: fetch +
`git merge main` into `feat/v0-3-m5-auto-soa`, resolve, re-verify A1's cites (CCIR-1), THEN
dispatch P4. Phase 1's roadmap regions were verbatim-imported from main's UNCOMMITTED tree and
are flagged UNCONFIRMED until this merge — the P4 dispatch must diff the merged result and
confirm the fold amendments survived (conductor ratification note, audit.md). Expect conflicts
in roadmap.md §M4/§M5/ledgers — worktree's carry the fold, by design. If M4 lands nothing beyond
1ac52fd, proceed on the present substrate and record that the merge found nothing.

## Carried residuals (durable homes exist; listed for dispatch convenience)
- P7: SCRATCH-array-by-value:42 stale heading; REF-mvp-scope.md:239 DAP line needs the deferral
  pointer; D12 + copy-on-persist docs homes (IMP-collections + REF-collections reconcile).
- P6 step 5: the "SROA eats binding memcpys" assertion-to-be-checked (FRAGO 010).
- P8: pirates-roster golden nondeterminism note (baselines-p0.md) before any byte-exact regen.

## Task-tracker state (session-scoped — recreate if the harness list is empty)
9 tasks, one per phase. At brief time: #1 #2 completed, #3 in_progress (seal it on commit),
#4-#9 pending.
