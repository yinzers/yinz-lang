---
name: "v0-3-m4-channels-arc-release-audit"
plan-id: "2026-07-02-v0-3-m4-channels-arc-release"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-02-v0-3-m4-channels-arc-release

Append-only. *How the plan got here.* Read by the AAR and auditors, never by executors (they read the
current-truth plan.md slice).

## Session log
- plan-producer-2026-07-02-m4 — 2026-07-02 — Authored the OPORD from the assembled brief. Consumed the
  grilled brief + recon landscape + risk union; grounded in REF-plan-format / REF-risk-engine /
  REF-decision-philosophy + the v0.3 roadmap + IMP-no-function-coloring + IMP-concurrency. Confirmed
  the recon citations against the live codebase (share/lend-across-background reject real at
  check.rs:2257/2282 — roadmap's 1216 stale; `join_poll` one-shot; `[[lint_rule]]`/`channel_capacity`/
  `auto_arc` all net-new; no cautionary/red-tint LSP rendering exists; M3d plan status is done, ledger
  says active — stale, non-blocking). Scored ¶1 authoritatively (R1/R2 → M via B1 architectural
  elimination; R3 → M via B2 boundary matrix; R4 → L). Pre-positioned a dormant contingent override for
  R1/R2 (unsigned). Set status: active (Intent/End-State + 7 phases real).

- plan-producer-2026-07-02-m4-r2 — 2026-07-02 — CORRECTION PASS: replaced the plan body in place
  (same plan-id) on the orchestrator's corrected brief after (a) a plan-reviewer blocker — the missing
  composed-suspension risk (now R5, with three code-confirmed trap doors 1a/1b/1c, a P0 spike HARD
  GATE, and a build-blocking composed fixture gating P2) — and (b) a second, deeper recon pass finding
  the more severe may-block twin-derivation risk R6 (`M2_MAY_BLOCK_INTRINSICS` twin-defined at
  typeck/intrinsics.rs:24 + codegen/emit.rs:819; the M3a/M3d/M3e/M3g drift class; now the P0-first
  deliverable: unify to one authoritative classifier + parity/RED tripwire; initial EH → residual M).
  Also corrected: version bump `0.3.0-m7` → `0.3.0` (prior draft wrongly assumed 0.2.x) with CHANGELOG
  span `m7..HEAD`; stale reject citations (real sites: check.rs:2275-2280 share / 2287-2292 lend,
  block at 2253; prior draft's 2257/2282 superseded); added the check.rs:2269-2270 silent-skip edge to
  the R3 matrix; added R7 (no kernel gate for channel ops — new compile-time gate, B1 → L); added the
  Tokio `sync` feature enable (ynz-runtime/Cargo.toml:23) as an explicit P0 step. Re-read
  IMP-no-function-coloring + IMP-concurrency in full; the design-doc diff surfaced two doc-flagged
  interactions the brief did not name, both resolved on the record: `ec-wrapper-collect-on-completion`
  (trigger gated on the handle-form P2 ships — resolved: communication-only handle, path verified
  unreachable, trigger re-scoped) and cancel-via-drop handle-drop semantics (safe-drop committed,
  full cancel-injection contingent with surfaced deferral path). Re-scored ¶1 authoritatively:
  R6 EH→M (B1 unification −2 + B2 parity −1), R5 H→L (B1 spike-proven architecture −2 + B2 composed
  fixture −1), R1/R2 H→L (B1+B2), R3 H→M (B2 matrix), R7 H→L (B1 compile gate), R4 L. Two dormant
  contingent overrides pre-positioned unsigned (#1 R6 unification-infeasible → H; #2 blocking-call /
  fixture-deadlock → H). Session-id appended to the frontmatter chain; status remains active.

- plan-producer-2026-07-02-m4-r3 — 2026-07-02 — THIRD, NARROW CORRECTION PASS (pre-execution;
  Patrick's no-duct-tape catch). One violation fixed, nothing else touched: the r2 draft deferred
  `ec-wrapper-collect-on-completion` to Future Requirements behind a "communication-only handle"
  scope-narrowing ("the copy-before-free path stays unreachable") — a narrowing invented to dodge a
  required companion fix, not a legitimate four-field deferral. Confirming citations: IMP-concurrency:475
  (cost: "landing WITH the `background-handle-form` feature"), :477 (trigger: collection via
  `.send`/`.receive` — the exact P2 surface — "gated on the `background-handle-form` feature"), :463
  (section header ties collection to background-handle collection), and — verified this pass — the
  registry entry's own `ships_in = "v0.3-M4"` (`registry/features.toml:1164-1170`, kind
  `[[deferred_language_feature]]`). Per Patrick's standing rule (this plan DEPENDS on it → in-plan,
  build-blocking), the fix is now a P2 deliverable scored as risk R8: prob B (new code on frame-free
  timing; EC-wrapper area historically fragile — M3f staging-slot clobber), sev II (use-after-free /
  stale-ok-pointer, silent-wrong class), initial H; mitigations B1 static-keyed collection design
  (copy decision keyed on COMPILE-TIME spawn form, handle-owned heap buffer freed at handle drop —
  eliminates runtime collected-state tracking and the dangling-pointer class by construction; a
  stronger call than the suggested B2-minimum, reasoning recorded in §3.1) + B2 RED-repro fixture
  matrix (collected vs. fire-and-forget × ok/error × receive timing, build-blocking) → residual L
  (B→E), gate pass. Reconciled ALL sites: §3.1 recorded decision rewritten (communication-only
  narrowing removed; `.receive()` is one surface covering message replies AND completion-value
  delivery); End State outcome 2 extended; definition-of-done + §3.2 + P2 task/steps 4-6/exit
  criteria (fix implemented + matrix gate + BOTH registry entries retired); P6 step 1 (doc amendment
  now marks the deferral SHIPPED, not trigger-re-scoped); §3.4 gate list + new CCIR (h)
  anti-re-narrowing tripwire; C&S slice map (P2 carries R5+R2+R8); Safety invariant added; Feature
  Registry "Modify" → "Retire"; Design-Doc Alignment #10 rewritten as Match with the r2 defect on
  the record; Future Requirements entry deleted; factor sweep resource-cleanup extended. Explicitly
  NOT touched (verified-legitimate deferrals per the correction brief): cancel-injection
  (IMP-no-function-coloring locks no-cancel-once-dispatched), auto_arc red-tint, seq-cst,
  capacity-retune, roadmap-corrections, SoA×padding, R6/R5/R3 machinery, version-bump facts.
  Session-id appended; status remains active.

- plan-producer-2026-07-02-m4-r4 — 2026-07-02 — FOURTH, NARROW CORRECTION PASS (pre-execution;
  the plan-reviewer's third-pass findings, verdict "sound + nearly complete, 2 findings, 0
  blockers"). Two additive fixes, nothing else touched:
  (1) **The R5×R8 composition gap** (should-fix): neither the R5 composed-suspension fixture nor
  R8's collection matrix covered their intersection — a collected `-> T errors` background child
  that suspends on a full-channel `send()` mid-execution (R5's scenario) AND whose completion
  value is then collected via the copy-before-free path (R8's scenario) — a direct
  frame-lifetime/free-timing interaction in the same use-after-free silent-wrong class both risks
  individually guard, and the same missed-composition failure mode that produced R5 itself against
  the original draft (the recurring class named by the M3g AAR / `authoritative-derivation.md`).
  Fixed as a dedicated composed cell ADDED to P2's R8 matrix (recorded call: a cell, not a full
  cross-product axis — the intersection is only reachable on the collected arm, so an axis would
  manufacture meaningless fire-and-forget×suspend cells), build-blocking in P2 step 6 + P2 exit
  criteria, asserting: byte-correct collected value, no dangling ok-pointer, no double-free, frame
  survives the suspension correctly, frame + buffer each freed exactly once, alloc=free. Threaded
  through every seam it touches: R8 risk-row B2 proof cell, §3.1 definition-of-done, §3.4
  verify-before-complete gate list + CCIR (b), Safety invariant (R8 bullet). R5's fixture and R8's
  existing (non-composed) matrix cells unchanged, per the correction brief.
  (2) **The missing R8 dormant override** (minor, symmetry): R6 (dormant #1) and R5/R1/R2
  (dormant #2) each carry a pre-positioned unsigned override; R8 had none — if its P2 proof REDs
  (the grep-audit finds a runtime conditional-on-receive copy path instead of the claimed
  compile-time spawn-form-keyed copy, or the matrix finds a use-after-free/dangling/double-free),
  the B1 step zeroes and residual rises to lookup(C,II) = HIGH needing a signature, and dormant
  #2's deadlock-scoped arming trigger does not cover that memory-safety RED. Added DORMANT #3 per
  REF-risk-engine's contingent-override format: pre-drafted, unsigned, `Accepted by` blank, armed
  only by the R8 proof failing. §3.1 fallback (R8 RED → HALT, #3 arms) + gate-summary count
  (three dormant overrides) + definition-of-done ("no dormant override armed") reconciled. New
  override added rather than extending dormant #2's trigger (recorded call): #2's arming
  condition, "why not mitigable" rationale, and revisit trigger are all deadlock-class-specific —
  grafting a memory-safety condition onto it would blur which redesign a RED demands.
  No re-scoring of any as-designed residual (R8 stays L as designed); no new deferral; everything
  the reviewer's Job A pass cleared left byte-identical. Session-id appended; status remains
  active.

- executor-2026-07-02-m4-p0 — 2026-07-02 — EXECUTED Phase 0 (HARD GATE). All exit criteria met; NO
  STOP condition triggered; NO dormant override armed. Pending P0 reviewer fan-out (producer does not
  self-grade). Grounded in the P0 slice + IMP-no-function-coloring + IMP-concurrency +
  authoritative-derivation.md (design docs re-read; no plan-vs-design contradiction found).
  (1) **Tokio `sync` enabled** (`ynz-runtime/Cargo.toml:23` + dev-deps); workspace build green.
  (2) **R6 unification**: read the crate DAG (codegen depends on typeck; registry/abi are leaf) →
  single authoritative home `ynz-typeck/src/suspension_source.rs` (`BASE_SUSPENSION_INTRINSICS` +
  `is_base_suspension_intrinsic`, exported from lib.rs). Deleted the `emit.rs:819` twin + the dead
  `intrinsics.rs::is_may_block_callee`; threaded all 7 consumers (typeck: may_block/cpu_admission/
  check×2; codegen: emit×3) onto the one source; rewrote the misleading "M3 deletes this" docs.
  Grep proof clean (zero old refs; one literal list). Build-blocking tripwire
  `tests/suspension_source_single_definition.rs` proven RED→GREEN (re-inject twin ⇒ RED at exact
  file:line ⇒ remove ⇒ GREEN). typeck+codegen+driver(435)+runtime suites green — behavior-preserving.
  Recon findings surfaced: `SuspendSet` is ALSO twin type-aliased (`cpu_admission.rs:33` +
  `emit.rs:132`, both `HashSet<String>` — a trivial-alias milder instance of the same class, left for
  a follow-on); FFI `foreign`/`may-block` does NOT exist yet (no parser/AST), so it is a future
  extension point of the one classifier, not threaded today.
  (3) **R5 composed-suspension spike** — executed through the REAL compiler (`ynz run`), never a
  hand-written Rust model. Throwaway mechanism (all torn down): a `wait sleep(SENTINEL)` driver +
  throwaway runtime shims (`spike_composed.rs` + two sentinel branches in the sleep create/poll shims)
  that drove a real `tokio::sync::mpsc(1)` composed scenario with the FORWARDED waker, `sleep_handle`
  held NULL. Verdicts S1/S2/S3 all GREEN (sum=30, parent-suspended-×2, child-suspended-on-full,
  ordering held, ~26ms, clean exit; no blocking call; no trap door). Persisted per spike discipline:
  `p0-spike/R5-composed-suspension-spike-report.md` + `p0-spike/composed-scenario.ynz` (the Phase 2
  fixture seed, written in intended channel syntax). Teardown verified clean (zero throwaway
  identifiers in crates/; runtime.rs + runtime lib.rs byte-reverted).
  (4) **Design locks** (Assumptions 8–11) resolved with reasons — recorded in plan.md P0 STATUS:
  Lock 8 → Yinz-level `errors` wrapper (`send() -> nothing errors`, typed channel-closed error, never
  raw Tokio SendError, never silent drop); Lock 9 → typeck built-in generic type (like array/map/fixed,
  new `Type::BuiltinChannel`), NO `[[keyword]]`; Lock 10 → DEFER seq-cst via `[[deferred_language_feature]]`
  `seq-cst-ordering-opt-in` added to `registry/features.toml` (ship acquire-release only); Lock 11 →
  spike-proven NO new frame-header slot is forced (endpoints live in the handle/runtime object; P2
  persists one opaque pointer via a dedicated non-`sleep_handle` slot — ripple named-but-not-forced).
  **Deviation surfaced (for the deviation-judge, not self-adjudicated):** pre-existing `ynz-registry`
  test `design_future_sync` fails (`cannot read design/future/`) — stale path from the 2026-07-01 docs
  migration (commit 93506c0), unrelated to P0 (a TOML edit cannot cause a dir-read error); noted, not
  fixed (out of scope). Session-id appended; status remains active.

- executor-2026-07-02-m4-p0-r3 — 2026-07-02 — Round-3 plan-seam fix-loop: pure plan-seam text
  reconciliation, zero source-code changes, closing all 3 should-fix + 1 minor findings from
  deviation-judge's round-2 review of FRAGO 001 and the P0 STATUS banner. (1) Reconciled the P0
  STATUS banner ([`plan.md`](plan.md) Phase 0) — it read "pending the reviewer fan-out" while the
  Round-2 fix-loop resolution bullet below it already recorded the fan-out as complete; rewrote the
  banner to state completion across both rounds with the real tally (round 1: full fleet, 0 blockers,
  1 should-fix + 1 FRAGO candidate; round 2: both fixed, re-verify clean, this round-3 gap found and
  closed). (2) Added an "Agent-availability note" under Phase 0's "Reviewer fan-out" line recording
  the disclosed adversarial-tester → code-reviewer structural substitution (no `adversarial-tester`
  agent type exists in this environment) with the verbatim round-1 code-reviewer confirmation quote
  that the spike-verdict audit ("no self-graded ACCEPT") actually happened. (3) Restored FRAGO 001's
  canonical `Base`/`Trigger`/`Changes`/`Unchanged`/`Override` field shape in [`audit.md`](audit.md)
  (was: non-canonical `Classification:` field, `Override:` renamed to `Ratification:`, `Unchanged:`
  missing entirely) matching the M3g precedent's shape, and fixed the stale `emit.rs:132` citation in
  the `Changes:` bullet to the actual post-fix `pub use` line `emit.rs:137` (confirmed via
  `git diff HEAD` — `:132` is the correct historical citation for the WHAT bullet describing the
  pre-fix twin location; only the post-fix `Changes:` citation was stale). No `.rs`/`registry/*.toml`
  file touched; no cargo build/test re-run (nothing code-level changed). Session-id appended; status
  remains active.

## FRAGO log
(r3/r4 were pre-execution plan corrections, logged as session entries per the r2 precedent;
FRAGOs record execution-time divergences against a running phase — FRAGO 001 is the first.)

## FRAGO 001 — 2026-07-02 — session-id: executor-2026-07-02-m4-p0-r2 (P0 Round-2 fix-loop, dispatched by the execution conductor)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 0 (post-review fix-loop, round 2)
Trigger:   Phase-0 reviewer fleet (code-reviewer, test-quality, deviation-judge, acceptance-verifier,
           rules-compliance — 0 blockers, 2 real findings) surfaced the `SuspendSet` type-alias twin
           that P0 recon had deferred as a bare audit-log sentence ("left for a follow-on", session
           entry `executor-2026-07-02-m4-p0`) with NO plan.md Future-Requirements entry and NO
           four-field WHAT/WHY/COST/TRIGGER deferral. deviation-judge's ruling: the SCOPE decision to
           defer was justified, but the DEFERRAL RECORDING was not (it never reached the plan seam).
           The conductor's disposition: rather than back-fill a deferral, FIX it now in the fix-loop.
           Classification (conductor authority, per the plan's Command & Signal — risk-neutral FRAGO,
           AUTO-APPLIED, no signed override required):
             - WHAT:  `pub type SuspendSet = HashSet<String>;` was independently declared at BOTH
                      `crates/ynz-typeck/src/cpu_admission.rs:33` AND `crates/ynz-codegen/src/emit.rs:132`
                      — a second, milder instance of the twin-derivation class R6's unification targets.
             - WHY risk-neutral: `SuspendSet` is a Rust TYPE ALIAS (`HashSet<String>`), structurally
                      transparent — the two declarations resolve to the identical type and CANNOT
                      semantically diverge the way R6's content-list twin (`BASE_SUSPENSION_INTRINSICS`,
                      a value list whose membership can drift) could. No runtime behavior, no ABI, no
                      membership answer rides on it. Hence a type-alias unification is a risk-neutral
                      refactor, not a content-derivation twin — the conductor's stated basis for
                      auto-applying without a signature (unlike an R6-class change).
             - CITATION TRAIL: deviation-judge (deferral-recording-not-scope judgment call) +
                      code-reviewer and test-quality (independently found the R6 tripwire's
                      same-line-with-`[` scan too weak to catch a `cargo fmt` multi-line reformat or a
                      match-arm re-derivation). deviation-judge also confirmed NO crate-DAG obstacle:
                      `emit.rs` already imports sibling symbols from `ynz_typeck::cpu_admission`
                      (`FusedMemberClass`, `nested_group_representative_callee`).
Changes:
  - `crates/ynz-codegen/src/emit.rs:137`: independent `pub type SuspendSet = HashSet<String>;` DELETED,
    replaced by `pub use ynz_typeck::cpu_admission::SuspendSet;` (re-export of the ONE canonical home
    at `cpu_admission.rs:33`). `emit::SuspendSet` consumers (`queries.rs:15`, codegen test
    `frame_layouts_query.rs:28`) keep their path unchanged; grep confirms exactly one
    `type SuspendSet = ...` declaration workspace-wide. `cargo build`/`cargo test --workspace` green,
    tally unchanged (zero behavior — pure de-duplication).
  - `crates/ynz-typeck/tests/suspension_source_single_definition.rs`: R6 tripwire drift-scan hardened
    (the second reviewer finding — a fix-loop change, not a FRAGO-classified scope divergence, folded
    into the same dispatch): drops the `[`/same-line requirement; now flags both quoted leaf literals
    co-occurring within 5 lines. All three twin shapes (single-line literal / `cargo fmt` multi-line /
    match arm) proven RED→GREEN; clean tree GREEN; backtick-prose mentions do not false-positive.
  - `plan.md` Phase 0 P0 STATUS: Round-2 fix-loop resolution bullet added under the Step 2 (R6
    unification DONE) block — records the SuspendSet unification (superseding the "left for a
    follow-on" note) and the tripwire hardening; cites file:line before/after.
Unchanged: everything not listed — ¶1, ¶2, ¶3.1, ¶3.2, Phases 1-6, ¶4, ¶5, the Invariants section,
  Design-Doc Alignment, Future Requirements; no R6 content-list classifier, spike artifact, or design
  lock touched by this FRAGO.
Override:  N/A — risk-neutral, no signature required (per Command & Signal / IMP-frago-aar.md).
  Distinct from the R6 content-list twin, which — being drift-prone — would require the full
  unification + parity-test treatment, not a bare auto-apply.
