---
name: "v0-3-hotfix-int-literal-number-audit"
plan-id: "2026-07-04-v0-3-hotfix-int-literal-number"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-04-v0-3-hotfix-int-literal-number

Append-only. *How the plan got here.* Read by the AAR, auditors, and the execution conductor's
Step-3a / Step-0 reconcile; never by executors (they read the current-truth plan.md slice).

## Session log

- `gate4-signatures-2026-07-04` — 2026-07-04 — Created this WARNO stub as one of three closing
  actions from Patrick's Gate-4 approval of the v0.3-M6/M7/M8 sibling concurrency plans. Assigns
  roadmap Capability Ledger row 441 (both duplicate tables in
  [`2026-05-21-v0-3-concurrency-perf/roadmap.md`](../2026-05-21-v0-3-concurrency-perf/roadmap.md))
  its own small hotfix slot, per that row's own text ("needs its own small hotfix slot, Patrick to
  assign") and the ELEVATED-priority triage both M6 and M7 recorded when they explicitly declined to
  absorb this fix. Mission, Situation, and open-questions content drawn directly from the roadmap
  ledger row's own text and the M5 plan's Future Requirements #7 deferral it originates from; no new
  investigation performed this session. `status: "stub"` — mostly-TBD per the WARNO-stub convention;
  the fix-shape decision (codegen-side `IntLit` branch vs. typeck-level coercion materialization) and
  the sequencing relative to M6/M7/M8 are both open, unresolved questions this stub deliberately
  leaves for a graduation pass. Roadmap Capability Ledger row 441 updated in the SAME session, in both
  duplicate tables, to point at this plan-id (see roadmap's own `audit.md` / this plan's session-log
  entry there for the lockstep confirmation).

- `m6-p1d-crossplan-coord-2026-07-10` — 2026-07-10 — DOC-ONLY scope-widening pass recording a v0.3-M6
  Phase 1d discovery into this stub's `## Situation` section (no code, no compiler files touched). M6
  Phase 1d shipped a typeck REJECTION gate turning "int literal / negated int literal supplied where a
  `number` is expected" into a clean teaching error across arg / construction / statement slots, and
  in doing so confirmed this int-literal→`number` class has THREE sibling facets sharing ONE root (a
  raw `i64` reaches a decimal128 `number` slot with no coercion): (1) **store-site** `let x: number =
  5` (this stub's original target; M6 Future-Req #9) — ICEs; (2) **declaration-site field default**
  `hidden f: number = 5` — CONFIRMED ICE `Found IntValue "i64 5" but expected PointerValue variant` at
  `crates/ynz-codegen/src/emit.rs:20347`/`:20351`, from field-default lowering at `emit.rs:18233`
  (bare `lower_expr`, no expected-type hint); (3) **call-site / arg positions** `background f(5)` /
  collection-method args (M6 Future-Req #14) — currently REJECTED by M6's gate, wants coercion. The
  Situation bullet records that this stub's coercion fix should REPLACE rejection with actual
  int→number coercion across ALL THREE facets as ONE mechanism (fix-shape candidate 2 — typeck-level
  coercion at the retype point — naturally subsumes all three), and that the call-site audit the stub
  already calls for should enumerate all three facets' slots. RECORD-ONLY: the fix-shape decision stays
  the graduation pass's own call — no adjudication performed this session. Transcribed from the M6 plan
  (`2026-07-04-v0-3-m6-concurrency-hotfix`) Future-Req #9/#14 and Phase 1d evidence; M6's own plan.md /
  audit.md were NOT touched (another executor's territory). Session-id appended to this plan's
  frontmatter chain in the same action. Nothing committed or staged — conductor seals.

- `executor-2026-07-10-m6-store-site-stopgap` — 2026-07-10 — cross-plan reconciliation from the v0.3-M6
  store-site stopgap (M6 FRAGO 020, human-directed "no duct tape"). M6 landed a REJECTION stopgap that closes
  the raw-ICE exposure at BOTH store-site facets — `let x: number = 5` (this stub's original target, facet 1)
  and `hidden f: number = 5` (facet 2) now emit M6's shared int-literal→`number` teaching error instead of the
  "compiler bug" ICE banner. Updated this stub's Mission intro (ICE exposure now closed by the stopgap) and the
  SCOPE-WIDENED bullet (facets 1 & 2 flipped from "still ICE" to "now rejected-with-teaching by M6"; all THREE
  facets are now uniformly rejected). **The stub's own job is UNCHANGED:** REPLACE that rejection with an actual
  int→number COERCION (which accepts the int literal) across all three facets as ONE mechanism — the stopgap
  only closed the crash exposure, it did NOT implement the coercion, and the whole M6 rejection guard is to be
  REMOVED when this stub's coercion ships. RECORD-ONLY: no fix-shape adjudication performed. M6's plan.md /
  audit.md carry the stopgap's own FRAGO 020 (that is the other executor's territory; this session touched them
  only for the M6-side reconciliation directed by the task). Session-id appended to this plan's frontmatter
  chain in the same action. Nothing committed or staged — conductor seals.

## FRAGO log

(none — this plan has not been dispatched for execution; it is a freshly-authored stub.)

## Context-segment log

(none yet — no phase has been dispatched.)
