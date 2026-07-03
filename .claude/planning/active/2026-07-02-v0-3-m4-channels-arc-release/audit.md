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

- executor-2026-07-02-m4-p1 — 2026-07-02 — EXECUTED Phase 1 PARTIALLY (runtime substrate only). NO
  STOP condition triggered (no synchronous blocking call, no deadlock — the substrate is proven
  suspends-not-blocks); NO dormant override armed. Grounded in the P1 slice + IMP-no-function-coloring
  (Channel/Queue Primitives §94-165) + IMP-concurrency (handle/backpressure §146-181) +
  authoritative-derivation.md + the P0 STATUS locks (esp. Lock 8/9/11) + the persisted P0 spike report.
  Producer does NOT self-grade — this entry is the honest record, pending reviewer fan-out.
  **What landed & verified (the R1 deadlock-safety RUNTIME SUBSTRATE — the highest-risk slice):** a
  new `crates/ynz-runtime/src/channel.rs` module with the channel C-ABI over `tokio::sync::mpsc`
  (`sync` feature already enabled in P0): `ynz_channel_create(capacity)` (bounded — clamps `<1 → 1`;
  NO unbounded constructor, stdlib-design Rule 4), `ynz_channel_send_poll(chan, value, waker_ctx)`,
  `ynz_channel_recv_poll(chan, out, waker_ctx)`, `ynz_channel_free(chan)`, exported from
  `crates/ynz-runtime/src/lib.rs:3-5`. Return ABI mirrors `ynz_rt_async_sleep_poll`: 0=Ready /
  1=Pending / 2=Closed. Deadlock-safe BY CONSTRUCTION: endpoint futures live in the runtime-owned
  `YnzChannel` object (NOT the type-punned `sleep_handle` slot — trap door 1c avoided, Lock 11
  honored); the boxed send future owns a CLONED sender + value (no self-reference); the ONLY channel
  operations in the path are `try_send`, `poll_recv`, and one `send().await` future polled with the
  FORWARDED waker — zero `block_on`/`blocking_send`/`spawn_blocking`/`thread::sleep`/`.park()` (R1's
  no-blocking-call guarantee). This makes durable exactly what the P0 R5 spike proved with throwaway
  shims. 4 substrate tests GREEN via `cargo test -p ynz-runtime channel`
  (`send_on_full_suspends_then_resumes_after_drain` = the R1 proof; `recv_on_empty_suspends_then_delivers`;
  `send_to_closed_returns_closed` = Lock 8 typed-Closed shape; `recv_on_closed_drained_returns_closed`);
  `cargo build --workspace` GREEN; `cargo clippy -p ynz-runtime -- -D warnings` (lib) clean.
  **NOT the ynz-run gate:** these Rust runtime tests are SUBSTRATE evidence; the Step-6 `ynz run`
  deadlock-safety GATE is NOT satisfied for Phase 1 (it needs the codegen + Deviation A's resolution).
  **NOT done (Step 1 typeck/codegen half, Steps 2–7):** no `Type::BuiltinChannel`, no typeck/parser
  wiring, no `.send`/`.receive` method typeck, no codegen lowering, no R6 classifier sibling arm, no
  kernel gate, no `channel_capacity` muted hint, no registry entry, no hostile fixtures, no
  demo/gallery. `channel<T>` is NOT yet user-reachable — the substrate is an exported C-ABI awaiting
  its codegen consumer (deliberately NOT half-wired into a broken user surface).
  **Deviation A surfaced (for the deviation-judge → FRAGO seam; NOT self-decided, NOT self-applied):**
  Phase 1 step 6's `ynz run` "full-channel backpressure" hostile fixture + its "send-on-full suspends,
  timing-verified" exit criterion (plan.md:594-602) structurally require TWO Yinz tasks sharing ONE
  channel (producer fills+suspends, consumer drains) — a single-task send-on-full hangs with no
  drainer. The only Phase-1 spawn primitive is `background` fire-and-forget (M3b); the handle-form is
  Phase 2. But a `channel<T>` cannot be shared across a `background` boundary today: mpsc splits into a
  cloneable Sender + a single non-cloneable Receiver, `.give` moves the whole value, and `.share`/`.lend`
  across `background` are compile errors (`check.rs:2275-2280`/`2287-2292`). Phase 1's plan text does
  not specify cross-task channel sharing, and the P0 seed `p0-spike/composed-scenario.ynz:26` is itself
  a Phase-2 fixture using `lend out: channel<int>` (a cross-background compile error). Candidate durable
  fix (channel `.copy` clones the sender-half, sharing an Arc-backed receiver) overlaps Phase-3 auto-Arc
  ownership work — a real design decision the deviation-judge/Patrick must classify, not the executor.
  **Deviation B surfaced:** Lock 11 (plan.md:563-570) assigned the cross-suspension endpoint-persistence
  slot decision to P2, but a bare `channel.send()`-on-full in P1 also needs the CHANNEL handle local to
  persist across its own suspension (to re-poll the same in-flight send on resume). Runtime half honors
  Lock 11 (endpoint future in the runtime object); the open P1 codegen call is only WHERE the channel
  local persists — most likely the existing crossing-local frame-slot mechanism (`sm_crossing_names` /
  `FRAME_OFFSET_LOCALS_START`), needing no new frame-header slot. Surfaced for classification, not
  self-decided. **Recorded durable decisions (made without a human, reasons on record):** (a) channel
  value model = ONE combined object holding both mpsc endpoints (Phase-1 single-value shape; Phase 2
  splits for cross-task); (b) element type carried as i64 bits at the ABI (consistent with array/map
  runtime); (c) send/recv poll return codes 0/1/2 mirroring the sleep poll ABI; (d) in-flight send
  future owns a cloned sender (avoids self-reference AND the sleep_handle slot — trap door 1c). **Files
  touched:** `crates/ynz-runtime/src/channel.rs` (new), `crates/ynz-runtime/src/lib.rs` (module + 4
  exports), `plan.md` (Phase 1 PARTIAL STATUS banner), `audit.md` (this entry). **Pre-existing/out of
  scope (noted like P0's `design_future_sync`):** `cargo clippy -p ynz-runtime --all-targets` flags a
  `duplicated_attributes` lint at `crates/ynz-runtime/tests/m2_runtime.rs:275` (`#[repr(C)]`), a
  clippy-version sensitivity in a file P1 never touched (git-confirmed: only `src/lib.rs` + new
  `src/channel.rs` changed). Session-id appended; status remains active.

- executor-2026-07-02-m4-p1-r2 — 2026-07-02 — P1 CONTINUATION (dispatched by the execution conductor
  after the P1 reviewer fleet + human decision gate on the two surfaced deviations). Producer does NOT
  self-grade — this is the honest record, pending reviewer fan-out. Grounded in the P1 slice +
  IMP-no-function-coloring + IMP-concurrency + authoritative-derivation.md + the P0 STATUS locks + the
  persisted P0 spike + no-duct-tape. Three parts:
  **Part 1 — FRAGO application (both human-approved, deviation-judge-classified; APPLIED, not
  re-decided).** FRAGO 002 (descope Phase 1 Step 6's two-task `ynz run` backpressure fixture — Patrick
  confirmed via AskUserQuestion "yes his descop is ok"): struck the two-task/composed requirement from
  Step 6 + exit criteria; replaced with the runtime-substrate proof
  (`send_on_full_suspends_then_resumes_after_drain`) + grep-audit + single-task hostile fixtures;
  relocated the two-task composed proof to Phase 2's own R5 B2 gate (no coverage lost — Phase 2 already
  carries it). FRAGO 003 (bare-send channel-local persistence resolves via the existing crossing-local
  mechanism, risk-neutral): amended Lock 11 + Phase 1 Step 2 so the next executor does not re-derive it.
  BOTH written into `plan.md` in-place AND recorded as `## FRAGO 002` / `## FRAGO 003` in this file in
  the canonical FRAGO 001 Base/Trigger/Changes/Unchanged/Override shape. Deviations A/B in the P1
  banner marked RESOLVED. **Part 3 — three reviewer-fleet minor fixes to
  `crates/ynz-runtime/src/channel.rs`:** (1) `ynz_channel_create` doc rewritten to name typeck as the
  primary no-unbounded / non-positive-capacity gate with the clamp as a release-mode floor (the
  present-tense claim is now anchored to the typeck gate Part 2 will build, with the clamp/debug_assert
  as the defensive backstop); (2) `catch_unwind` added around BOTH `ynz_channel_send_poll` and
  `ynz_channel_recv_poll` polls (returning `CHANNEL_PENDING` on panic) via a shared `panic_payload_msg`
  helper — chose genuine panic-parity with the `ynz_rt_async_sleep_poll` "mirror" claim over narrowing
  the doc; (3) `debug_assert!(capacity >= 1, …)` above the release clamp. 4 channel substrate tests
  GREEN; `cargo clippy -p ynz-runtime -- -D warnings` (lib) clean. **Part 2 — typeck/codegen half +
  Steps 2–7: DELIBERATELY NOT half-wired (recorded durable decision, reason on the record).** Adding
  `Type::BuiltinChannel` end-to-end is a monolithic build (~24 typeck + ~27 codegen match sites);
  `llvm_type_for_ctx` returns `Option`/`None` for unhandled types, so the instant typeck resolves
  `channel<int>()` a user program reaches codegen — anything short of the complete chain is a
  type-checks-but-miscompiles broken surface (the exact half-feature `executor-2026-07-02-m4-p1`
  refused; no-duct-tape forbids). The load-bearing piece — mirroring `emit_wait_point`
  (`emit.rs:9731`) / `emit_sleep_poll_branch` (`state_machine.rs:697`) for a NEW suspension source
  whose handle persists as a crossing-local (FRAGO 003) — is the maximum-adversarial suspension-codegen
  domain P0 spent a HARD GATE de-risking and the silent-miscompile-across-suspension class that burned
  M3a/M3d/M3e/M3g. Shipping an unverified partial there would reintroduce exactly the R5/R1 risk. Part 2
  stands as documented remaining work (the `executor-2026-07-02-m4-p1` remaining-work map holds, now
  FRAGO-002-rescoped) and is surfaced as needing its own focused, end-to-end-verified execution — not
  silently skipped. **No new deviation surfaced.** **Files touched:**
  `crates/ynz-runtime/src/channel.rs` (three fixes), `plan.md` (FRAGO 002/003 in-place + Lock 11 +
  Step 2/6 + exit criteria + P1 banner + frontmatter session-id), `audit.md` (FRAGO 002, FRAGO 003,
  this entry). NO STOP condition fired; NO dormant override armed. Session-id appended; status remains
  active.

- executor-2026-07-02-m4-p1-r3 — 2026-07-02 — P1 CONTINUATION (dispatched to build the typeck→codegen
  chain the two prior rounds deliberately deferred). Producer does NOT self-grade — honest record,
  pending reviewer fan-out. Grounded in the P1 slice (post-FRAGO-002/003) + P0 STATUS locks (esp. Lock
  8/9/11) + FRAGO 003 + IMP-concurrency + IMP-no-function-coloring + authoritative-derivation.md +
  no-duct-tape + the committed substrate (`channel.rs` @ `a4f10c0`) + the persisted P0 spike.
  **Outcome: deep, complete codegen recon + an execution-ready implementation blueprint (below); ZERO
  source built (by design — see the all-or-nothing finding); ONE durable decision locked. Tree left at
  the verified-good substrate-only state.** NO STOP condition fired; NO dormant override armed.
  **Why no source shipped (honest, evidence-backed — not the prior "mixed-with-trivial-work" reason).**
  The remaining chain is genuinely **all-or-nothing**: `llvm_type_for_ctx` (`emit.rs:1360`) returns
  `None` for an unhandled type, so the moment typeck resolves `channel<int>()` to `Type::BuiltinChannel`,
  any user program reaches codegen and crashes/miscompiles — strictly WORSE than today's clean "`channel`
  is not a known type" diagnostic (a no-duct-tape regression). The r3 executor DID build the
  `Type::BuiltinChannel` variant + `type_name` arm to verify the approach compiles (not inspection-only,
  per verification.md), then `git checkout`-reverted it so the tree stays clean rather than half-wired.
  The codegen half is the maximum-adversarial silent-miscompile-across-a-suspension surface P0 burned a
  HARD GATE on; completing AND verifying it (deadlock-safe `ynz run` + alloc=free + grep-audit + demo +
  gallery + cross-impl) to that bar is a dedicated focused build, not a responsible one-pass job —
  rushing it reintroduces R1/R5/R6. Per the plan's own priority (honest PARTIAL + no-R-regression >
  completion), surfaced with a full blueprint instead of a rushed/half COMPLETE.
  **DURABLE DECISION (made without a human, reason on record): bare-channel methods are `.send(value)`
  and `.receive()`** — NOT `.recv()` (the dispatcher's flagged uncertainty). Three agreeing sources:
  remaining-work map #2 (Phase-1 text), IMP-concurrency:174-178, IMP-no-function-coloring:258, and the
  P0 seed `composed-scenario.ynz:28,32,44,46`. This is NOT a plan divergence (the plan already says
  `.receive()`); it is a confirmation, so no FRAGO.
  **EXECUTION-READY BLUEPRINT for the next P1 executor (every site verified against the live tree this
  round):**
  1. **Type foundation** — add `Type::BuiltinChannel { elem: Box<Type> }` to `ynz-typeck/src/types.rs`
     (mirror `BuiltinMap`, `types.rs:92`); add arms to `type_name` (`types.rs`, `channel<{elem}>`);
     `is_trivially_copyable` needs NO edit (it's `matches!`, defaults false — correct, channel is a heap
     ref). Update the doc `Current count: 20 → 21`. Variant-count tests: `tests/check.rs:56`
     (`m4_type_variant_count_locked`) checks only the first 10 M1–M4 variants — NO edit; `tests/maps.rs:652`
     (`m5p3c_type_variant_count_locked`, `all.len()`) — CHECK whether it enumerates all variants and bump
     if so; `tests/errors_typeck.rs:78/132` — same check.
  2. **Type resolution — THREE sites, all need a `"channel"` arm:** `check.rs::ast_type_to_type`
     Generic arm (`check.rs:3827`, next to `array`/`fixed`/`map`); `shapes.rs::resolve_ast_type` Generic
     arm (`shapes.rs:146`); AND `signatures.rs` — note `resolve_sig_type_with_params` (`signatures.rs:422`)
     currently produces `Type::Generic{name,args}` for ALL generics (verify how `array`/`map` in
     signatures normalize — the primary `sig_ast_type_to_type` at `signatures.rs:235` is the one to add
     the arm to). Also add `"channel"` to the bare-name-without-args error arm (`check.rs:3742-3752`).
  3. **Construction typeck** — `channel<T>()` (default capacity 64) / `channel<T>(N)`. This parses as an
     `Expr::Call` with `type_args: Some([T])` (parser `try_parse_type_args`, `parser.rs:3001`; call
     construction `parse_call`, `parser.rs:2813`). Add the construction arm alongside the `map<K,V>()`
     construction typeck (near `check.rs:3853`/`4566`, the `BuiltinMap` construction/empty-collection
     sites): reject a non-positive literal capacity ("no unbounded constructor" — stdlib Rule 4, capacity
     must be a positive int); default 64 when no arg. Add a KERNEL-MODE gate here mirroring the
     `background` gate (`check.rs:2226-2236`): channel construction → COMPILE ERROR under `--kernel`,
     WHAT/WHAT-INSTEAD/WHY.
  4. **Method typeck** — `.send(value)` and `.receive()`. The method-call typeck dispatch is at
     `check.rs:3433` (`if let Type::BuiltinMap ... receiver_ty`) — add a `Type::BuiltinChannel` block
     next to it. `.send(value: T) -> nothing errors` (typecheck the arg against `elem`; `errors` per
     Lock 8 — closed channel yields a typed Yinz channel-closed error, never raw Tokio `SendError`;
     wrap the return as `Type::ErrorsCapable { inner: Box::new(Type::Nothing) }`). `.receive() -> T`
     (returns `elem`). Reject any other method with the "`channel<{}>` does not have a method" shape
     (mirror `check.rs:3451`). These are UFCS over standalone ops, NOT OOP methods (non-oop.md). Add the
     kernel gate here too.
  5. **R6 classifier sibling arm** — add `pub fn channel_method_suspends(receiver: &Type, method: &str)
     -> bool` (true iff `receiver` is `Type::BuiltinChannel` and method ∈ {`send`,`receive`}) to
     `ynz-typeck/src/suspension_source.rs` — the module doc `suspension_source.rs:32-38` ALREADY earmarks
     this exact arm. Thread it (NEVER a second list) into: (a) may-block seeding, (b) cpu_admission
     decline, (c) codegen counting, (d) codegen lowering.
  6. **May-block seeding (typeck)** — `may_block::analyze` (`may_block.rs:96`) is purely AST-name-keyed
     with NO type access (`collect_calls_in_expr`'s `MethodCall` arm just recurses, `may_block.rs:981`).
     Do NOT thread types into may_block.rs. Instead, in `check.rs` (where `expr_types` IS available),
     compute the set of functions whose body contains a channel `.send()`/`.receive()` MethodCall (using
     `channel_method_suspends` + `expr_types`), and pass that set as `extra_seeds` to the existing
     `suspends_with_extra_seeds` (`may_block.rs:163`) — the CPU-promotion pass already uses this exact
     extra-seeds path, so a channel-using function becomes a state machine identically to a `wait` caller.
     Find where check.rs currently calls `may_block::analyze` and augment the seed set there.
  7. **CPU admission decline** — `cpu_admission.rs:806` already consumes the authoritative classifier;
     confirm a channel-using closure is declined for free once #6 marks it suspending (write the decline
     fixture the plan Step 2 requires). This should fall out of #6 with little/no new code.
  8. **Codegen — extern decls:** declare the 4 channel C-ABI fns in the runtime-fn table (next to
     `ynz_rt_async_sleep_create`/`_poll` — search `cg.rt.ynz_rt_async_sleep_create`, the `Rt` struct that
     holds `FunctionValue`s): `ynz_channel_create(i64)->ptr`, `ynz_channel_send_poll(ptr,i64,ptr)->i32`,
     `ynz_channel_recv_poll(ptr,ptr,ptr)->i32`, `ynz_channel_free(ptr)`. Signatures confirmed against
     `crates/ynz-runtime/src/channel.rs:104,148,231,274`.
  9. **Codegen — construction + type lowering:** `llvm_type_for_ctx` (`emit.rs:1360`) → channel is an
     opaque `ptr` (like `BuiltinArray`/`BuiltinMap` at `emit.rs:1885-1889`); `mangle_type`
     (`emit.rs:1313`) + `to_i64_bits`/`resolve_type` (`emit.rs:1845`) arms; lower `channel<T>()`
     construction to a `ynz_channel_create(capacity)` call returning the ptr.
  10. **Codegen — the NEW suspension lowering (the load-bearing, maximum-adversarial piece):**
      - `count_suspension_expr` (`emit.rs:3410`) currently returns 0 for `Expr::MethodCall` — it must
        count a channel `.send()`/`.receive()` as 1 suspension point. It's a type-FREE free fn, so thread
        `expr_types` (or a `&dyn Fn(&Expr)->Type` closure) through `count_suspension_points` /
        `count_suspension_stmt` / `count_suspension_expr` so it can call `channel_method_suspends`. This
        pre-count MUST match the lowering's actual suspension-point emission exactly (mismatch = state
        block over/under-allocation = the M3e-class detonation).
      - `lower_sm_stmt_with_wait` (`emit.rs:4571`) + `stmt_contains_wait`/`stmt_contains_suspending_call`
        gating (`emit.rs:3458`): add arms for `Stmt::Expr(MethodCall channel.send/.receive)` and
        `Stmt::Let { value: MethodCall channel.receive }`, routing to a NEW
        `emit_channel_suspend_point(cg, receiver, method, arg_opt, ...)` that MIRRORS `emit_wait_point`'s
        (`emit.rs:9731`) frame-slot/resume-point/`sm_post_wait`/`sm_suspend` machinery but: (i) NO create
        call — lower the receiver expr to the channel ptr; (ii) call `ynz_channel_send_poll(chan, value,
        waker_ctx)` (send: lower the arg to i64 bits) or `ynz_channel_recv_poll(chan, out_alloca,
        waker_ctx)` (recv: read `*out` on Ready, bind to the `let` name); (iii) THREE-way branch on the
        i32: `0`=Ready→post-wait, `1`=Pending→persist + `store_resume_point` + branch `pending_block`,
        `2`=Closed→construct the typed channel-closed `errors` value (Lock 8) / propagate. The channel
        HANDLE local persists across the Pending suspension via the EXISTING crossing-local frame-slot
        mechanism (`sm_crossing_names` / `FRAME_OFFSET_LOCALS_START`) per FRAGO 003 — no new frame-header
        slot, and the runtime object already owns the in-flight `pending_send` future so re-polling the
        same ptr resumes correctly.
      - GREP-AUDIT the NEW codegen path emits ONLY the poll calls above — no `block_on`/synchronous wait
        (R1). This is where a blocking call could sneak in; it is the single most important verification.
  11. **Muted hint + registry** — add the `channel_capacity` `[[muted_hint_domain]]` (placement_category
      = "Addition") to `registry/features.toml` (mirror an existing entry, `features.toml:2053-2145`);
      wire the typeck hint-pass + `crates/ynz-lsp/src/inlay_hint.rs` to render `⟨64⟩` inside the empty
      `channel<T>()` parens with a WHAT/WHAT-INSTEAD/WHY hover stating capacity + default-vs-user-set.
  12. **Fixtures + demo + gallery** — SINGLE-TASK hostile `.ynz` fixtures through `./target/debug/ynz run`
      (closed-channel send/recv, never-drained, capacity clamp / no-unbounded — FRAGO 002 rescoped the
      two-task composed fixture to Phase 2); alloc=free via `YNZ_ALLOC_COUNTER_OUTPUT`; `--no-auto-parallel`
      byte-identical. Extend `examples/pirates-roster/entrypoint.ynz` with real channel usage (regen
      `expected_stdout.txt` via its script) + create `examples/primantis-orders/v0_3_m4_errors.ynz` with
      kernel-gate-on-channel-op + closed-channel triggers, each with a `// WHY:` comment.
  **Files touched this round:** `plan.md` (Phase 1 STATUS banner r3 bullet + frontmatter session-id),
  `audit.md` (this entry). NO `.rs`/`registry/*.toml`/example source touched (the `Type::BuiltinChannel`
  probe was reverted). No cargo build/test re-run needed (no code-level change persisted). Session-id
  appended; status remains active.

- executor-2026-07-02-m4-p1-r4 — 2026-07-02 — P1 CONTINUATION (dispatched to execute the r3 blueprint
  end-to-end). Producer does NOT self-grade — honest record, pending reviewer fan-out. Grounded in the
  P1 slice (post-FRAGO-002/003) + the r3 blueprint in this file + P0 STATUS locks (8/9/11) +
  IMP-no-function-coloring + IMP-concurrency + authoritative-derivation.md + no-duct-tape + verification.md
  + the committed substrate (`channel.rs`) + the P0 seed. **Outcome: BUILT-AND-COMPILED the entire typeck
  half + the codegen extern/mangle/construction pieces against the live tree, then REVERTED to
  substrate-good on two NEW findings; net source change this round = zero (by design, all-or-nothing).**
  NO STOP condition fired; NO dormant override armed.
  **What was built and VERIFIED-COMPILING this round (then reverted per all-or-nothing):**
  - **Typeck half (blueprint steps 1–6, `cargo build -p ynz-typeck` GREEN):**
    (1) `Type::BuiltinChannel { elem: Box<Type> }` + `type_name` arm (`types.rs`); the variant-count
    lock tests don't enumerate it (`m4`/`m5p3c`/`m7` lock SUBSETS), so no ratchet edit needed — the
    exhaustive-match compiler errors are the real safety net.
    (2) Resolution arms: `check.rs::ast_type_to_type` Generic `"channel"` arm + the
    `array|fixed|maybe|map|MapEntry|channel` bare-name-requires-args arm + the capital-letter guard;
    `shapes.rs::resolve_ast_type` Generic `"channel"` arm (signatures.rs `sig_ast_type_to_type` gets it
    free — it delegates to `resolve_ast_type`).
    (3) `check_channel_construction`: exactly-one-type-arg, default-64 / explicit-int capacity,
    non-positive-LITERAL reject (stdlib Rule 4), non-int capacity reject, kernel gate (R7). Wired as a
    `"channel"` arm in `check_call`.
    (4) `check_channel_method`: `.send(value)` typechecks arg against `elem` → `nothing errors`
    (Lock 8), `.receive()` (0 args) → `elem`, unknown-method + kernel-gate arms. Wired into the
    `Expr::MethodCall` handler ahead of the intrinsic/collection dispatch (args are in scope there).
    (5) R6 sibling classifier `channel_method_suspends(receiver: &Type, method: &str) -> bool` added to
    `suspension_source.rs` — the ONE authoritative home the module doc earmarked; no second list.
    (6) may-block SEED wiring in `queries.rs`: `channel_suspending_fn_seeds(module, expr_types)` (a new
    `expr_types`-keyed walker in `check.rs` using the R6 classifier) → `suspends_with_extra_seeds`, run
    AFTER `check` produces `expr_types`, GATED on channel-presence so `channel_seeds.is_empty()`
    reproduces `may_block_result.suspends` byte-for-byte (zero change for non-channel programs). Also a
    supplementary channel-augmented mutual-recursion cycle check that skips already-name-based-reported
    cycles.
  - **Codegen pieces that compiled:** 4 channel C-ABI extern decls in `runtime_decls.rs`
    (signatures matched against `channel.rs:104/148/231/274`); the `mangle_type` `"channel"` arm; the
    `channel<T>()` construction lowering (`ynz_channel_create(capacity)`) in `lower_expr`'s Call
    dispatch (default 64 / lowered int arg). `llvm_type_for_ctx` already routes unknown types to `ptr`
    via its `_ =>` arm, so channel-as-ptr fell out for free.
  **The two NEW findings that drove the revert (concrete, previously-unstated — NOT a re-run of r3):**
  - **FINDING 1 — the crossing-local / suspension-counting / SM-routing machinery is FUNCTION-NAME-keyed
    and cannot see a channel MethodCall; r3's "reuse the EXISTING crossing-local mechanism (FRAGO 003)"
    understates the work.** `crossing_local_names_with_cpu_spike` → `locals_crossing_wait` →
    `collect_crossings_in_stmts` (`check.rs:6779/7394/7510`, consumed by codegen at `emit.rs:327/751/2414`)
    decide "suspension point?" via `is_suspending_call(c, suspending)` (function-NAME-keyed, `check.rs`)
    and `block_contains_inferred_suspension` — a channel `ch.send()`/`ch.receive()` `Expr::MethodCall`
    matches NONE. Same for `count_suspension_expr` (`emit.rs:3410`, returns 0 for `MethodCall`) and the
    SM routing gate (`emit.rs:4142`). Without threading TYPE-awareness (`expr_types` +
    `channel_method_suspends`) through all of it, the channel handle is not marked crossing → not
    persisted → garbage ptr on resume (the M3a/M3d/M3e/M3g silent-miscompile class). A more-tractable
    decomposition recorded for the next round: mark every `channel<T>`-typed local as crossing via the
    already-threaded `expr_types` in `crossing_local_names` (a SOUND over-approximation — over-marking
    reserves a harmless slot; under-marking is the M3-class bug) rather than rewriting the sprawling
    suspension-trigger detection; then make only `count_suspension_points` + the routing gate +
    `lower_sm_stmt_with_wait` channel-aware via `cg.typed`, plus the new 3-way
    `emit_channel_suspend_point` (Ready/Pending/Closed) + the channel-ptr arm in `value_to_i64_bits`
    (`emit.rs:9905`).
  - **FINDING 2 (decisive) — per FRAGO 002, Phase 1's single-task scope has NO `ynz run` fixture that
    exercises the codegen SEND-ON-FULL SUSPEND→persist→resume path.** That path only fires when a
    producer suspends on a full channel WHILE another task drains — the two-task composed scenario the
    P0 seed `p0-spike/composed-scenario.ynz` itself labels Phase-2 (needs `background`-handle cross-task
    sharing, `lend out: channel<int>` across a `background` boundary), which FRAGO 002 explicitly
    relocated to Phase 2's build-blocking R5 gate. Every Phase-1 single-task fixture exercises only the
    fast-path Ready (interleaved send/receive within capacity), the Closed branch, or clean free/no-leak.
    So building `emit_channel_suspend_point`'s suspend path in Phase 1 ships the maximum-adversarial
    M3-class suspension codegen with its generated-IR persist/resume path un-`ynz-run`-provable until
    Phase 2 — a direct tension with verify-don't-assert and R1/R5.
  **DECISION (on the record; no-duct-tape all-or-nothing + the plan's priority "honest PARTIAL +
  no-R-regression > completion"):** landing the typeck half alone REGRESSES today's clean "channel is
  not a known type" diagnostic into a codegen miscompile/crash (`llvm_type_for_ctx` routes it to `ptr`,
  and the suspension path isn't channel-aware), so the whole chain was `git checkout`-reverted. Tree
  verified back at substrate-good: `cargo build --workspace` GREEN, all 4 `ynz-runtime` channel
  substrate tests GREEN post-revert. The typeck half is proven implementable-and-compiling (built, not
  inspected — verification.md satisfied for the typeck half); the codegen suspension path + its
  verification placement is surfaced, not self-decided.
  **DEVIATION C surfaced (for the deviation-judge → FRAGO seam; NOT self-decided, NOT self-applied):**
  the codegen suspension-point build's PLACEMENT is a phase-boundary scope call — (a) build the full
  Phase-1 codegen with the suspend path verified only by the runtime substrate + grep-audit + deferred
  to Phase 2's composed fixture, OR (b) fold the suspension-codegen build into Phase 2 where the
  two-task composed R5 fixture (already Phase 2's build-blocking gate) can end-to-end-verify it. The
  executor does NOT choose; the orchestrator's seam does. **Recorded durable decisions (made without a
  human, reasons on record):** (a) mark-all-channel-typed-locals-as-crossing is the sound
  over-approximation for the crossing gap (over-marking safe, under-marking is the M3 bug); (b) the
  may-block channel seed is gated on channel-presence so non-channel programs are byte-identical; (c)
  the R6 classifier is a single `channel_method_suspends` fn keyed on receiver-type + method-name, never
  a second list. **Files touched this round:** `plan.md` (Phase 1 STATUS r4 bullet + frontmatter
  session-id), `audit.md` (this entry). NO `.rs`/`registry`/example source persisted (all built-then-reverted).
  No cargo build/test re-run needed beyond the post-revert substrate confirmation above. Session-id
  appended; status remains active.

- executor-2026-07-02-m4-p1-r5 — 2026-07-02 — P1 COMPLETION (dispatched to apply the human-approved
  FRAGO 004 and BUILD Phase 1's now-correctly-scoped construction-only work). Producer does NOT
  self-grade — honest record, pending reviewer fan-out. Grounded in the P1 slice + P0 STATUS locks
  (esp. Lock 8/9/11) + FRAGO 002/003 + the r4 blueprint (audit.md) + IMP-concurrency +
  IMP-no-function-coloring + authoritative-derivation.md + no-duct-tape + verification.md + the
  committed substrate (`channel.rs`) + the P0 seed. NO STOP condition fired; NO dormant override
  armed.
  **Part 1 — FRAGO 004 APPLIED (Patrick-approved disposition (b) via the recommended default;
  deviation-judge-verified; APPLIED, not re-decided).** Redrew the Phase 1/Phase 2 boundary: the
  suspending `.send()`/`.receive()` method surface and ALL its suspension codegen (the R6 sibling
  arm, the crossing-local/count-suspension/SM-routing type-awareness threading, `emit_channel_suspend_point`,
  Lock 8's typed-`errors` on `.send()`, the backpressure teaching text, AND the bare-channel
  send/recv suspend→resume proof) moved to Phase 2, where the two-task composed R5 fixture can
  end-to-end-verify it. Phase 1 rescoped to the fully-verifiable construction-only subset. Written
  into `plan.md` in-place (Phase 1 header/task/steps/exit-criteria + a concise COMPLETE STATUS
  banner replacing the r1–r4 saga; Phase 2 task/purpose/steps/exit-criteria grown; §5 slice map;
  frontmatter session-id) AND recorded as `## FRAGO 004` below in the canonical
  Base/Trigger/Changes/Unchanged/Override shape.
  **Part 2 — BUILT Phase 1's construction-only scope, end-to-end and verified GREEN:**
  (1) `Type::BuiltinChannel { elem }` (`types.rs`, 20→21, `type_name`) + the three resolution arms
  (`check.rs` `ast_type_to_type` Generic `"channel"` arm, bare-name-requires-args arm, capital-letter
  guard). (2) `check_channel_construction` (`check.rs`): one-type-arg, default-64 / explicit-int
  capacity, non-positive-LITERAL reject (handles `0` AND negated `-N` via `UnaryOp::Neg`), non-int
  reject, too-many-args reject, missing-element reject, kernel gate (R7) — wired as the `"channel"`
  arm in `check_call`. (3) Codegen: `ynz_channel_create` extern (`runtime_decls.rs`); `mangle_type`
  + `llvm_type_for` + `alloca` channel-as-`ptr` arms; the `"channel"` construction lowering in
  `lower_expr` → `ynz_channel_create(capacity)`; the two compile-forced exhaustive classifiers
  (`parity_case` + `value_to_i64_bits` in `emit.rs`; `runtime_axis_coverage` in `integration.rs`)
  extended — channel = single owning heap ptr that Phase 1 DECLINES as a CPU-background return (both
  CPU-result-ABI gates already agree on `false` via their catch-alls; NEITHER gate touched —
  authoritative-derivation.md honored). 13 IR golden snapshots updated (each: only the new
  `declare ptr @ynz_channel_create(i64)` line). (4) `channel_capacity` `[[muted_hint_domain]]`
  registered (`registry/features.toml`, Addition, `⟨64⟩` + hover WHAT/WHAT-INSTEAD/WHY); LSP
  inlay-firing wiring is P5 (the `allocators` registered-not-yet-firing precedent). (5)+(6) Fixtures:
  `v0_3_m4_channel_construct.ynz` (runs GREEN) + `v0_3_m4_channel_bad_capacity.ynz` (compile-rejected);
  integration tests `v0_3_m4_channel_construct_{runs_through_real_compiler,alloc_equals_free,no_auto_parallel_byte_identical}`
  + `_bad_capacity_rejected_at_compile_time`; 4 typeck unit tests
  (`channel_construction_in_kernel_mode_rejected` = the R7 kernel trigger, `_default_and_explicit_capacity_typecheck`,
  `_non_positive_capacity_literal_rejected`, `_missing_element_type_and_wrong_capacity_type_rejected`).
  (7) Demo: documented construction placeholder in `pirates-roster/entrypoint.ynz` (construction-only
  has no observable output → real round-trip grows the demo in Phase 2; golden unchanged); gallery
  `examples/primantis-orders/v0_3_m4_errors.ynz` (5 construction diagnostics) + `v0_3_m4_gallery_fires_expected_diagnostics`.
  **Verification:** `cargo build --workspace` GREEN; `cargo test -p ynz-typeck` / `-p ynz-codegen`
  / `-p ynz-driver` (435-integration + galleries + cross_impl + demo golden) / `-p ynz-registry`
  all GREEN; `cargo clippy -p ynz-typeck -p ynz-codegen -- -D warnings` clean.
  **alloc=free (honest, on record):** `ynz_channel_create` uses Rust `Box`, NOT the counted
  `ynz_alloc` (like `map`/`array`'s libc `malloc`), so a channel-construction program keeps
  `ynz_alloc`/`ynz_free` balanced at `alloc==free` (0==0), and — matching maps/arrays — a channel
  local is NOT scope-freed in Phase 1 (no scope-drop mechanism exists to hook; a language-wide gap,
  not channel-specific); `ynz_channel_free` is runtime-test-proven and reserved for Phase 2's
  handle-drop.
  **Recorded durable decisions (made without a human, reasons on record):** (a) `channel_capacity`
  ships as a REGISTERED-not-yet-firing muted-hint domain in Phase 1 (registry SSOT) with the
  inlay-firing wiring in P5 — mirroring the `allocators` precedent and the plan's own P5 step 1
  assignment; a typeck detection pass with no LSP consumer would be a dangling half-surface. (b)
  Channel construction follows the existing heap-local pattern (constructed, not scope-freed —
  identical to `map`/`array`), rather than special-casing a channel-only scope-drop (there is no
  drop-insertion pass to hook, and special-casing channels would be inconsistent); recorded so the
  Phase-2 executor wires `ynz_channel_free` deliberately at handle-drop, not as an ad-hoc Phase-1
  patch. (c) Channel DECLINES the CPU-background result ABI in Phase 1 (both gates already agree via
  catch-alls; no gate edited) rather than admitting it — sequential lowering is always correct and
  admitting would touch the authoritative-derivation-sensitive CPU-result-ABI gate pair for a
  capability Phase 1 does not need. **No new deviation surfaced.** **Files touched:** see FRAGO 004
  Changes + this entry. Session-id appended; status remains active (P1 COMPLETE; P2 next).

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

## FRAGO 002 — 2026-07-02 — session-id: executor-2026-07-02-m4-p1-r2 (P1 continuation; deviation-judge-classified, Patrick-confirmed via the execution conductor)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 1 (Step 6 + exit criteria)
Trigger:   Deviation A (surfaced by `executor-2026-07-02-m4-p1`, session log above) — Phase 1 Step 6
           and its exit criteria, as originally written, demanded a "full-channel backpressure"
           `ynz run` fixture that structurally requires TWO Yinz tasks sharing ONE `channel<T>` (a
           producer that fills + suspends, a consumer that drains). But `channel<T>` cannot cross a
           `background` boundary today: `.share`/`.lend` across `background` are compile errors
           (`crates/ynz-typeck/src/check.rs:2275-2280`/`2287-2292`) and `.give` moves the whole value,
           leaving the other side with nothing. The P0 spike seed `p0-spike/composed-scenario.ynz` is
           itself labeled (its own header, lines 1-9) as the SEED for Phase 2's build-blocking composed
           fixture, not Phase 1's — confirming the two-task cross-boundary capability genuinely lands in
           Phase 2 (handle-form), not Phase 1. deviation-judge confirmed the SCOPE reading; Patrick
           confirmed the descope via AskUserQuestion on 2026-07-02 ("yes his descop is ok"). This is
           the recorded APPLICATION of that human-approved classification, not a re-decision by the
           executor.
Changes:
  - `plan.md` Phase 1 Step 6: STRUCK the two-task / `ynz run`-composed-backpressure requirement.
    REPLACED with: Phase 1's R1 (send-on-full-suspends-without-blocking) evidence is the
    runtime-substrate proof already GREEN (`cargo test -p ynz-runtime channel`, specifically
    `send_on_full_suspends_then_resumes_after_drain`) PLUS the grep-audit (no synchronous blocking
    call in the emitted path) PLUS Phase-1-achievable SINGLE-TASK hostile fixtures through `ynz run`
    (closed-channel send/recv, never-drained, capacity clamp / no-unbounded — none needing cross-task
    sharing), alloc=free. Explicitly notes the end-to-end two-task composed suspend-then-resume proof
    is NOT lost — it is Phase 2's OWN pre-existing build-blocking R5 B2 composed fixture (Phase 2
    Step 6), grown from the same `composed-scenario.ynz` seed.
  - `plan.md` Phase 1 exit criteria: the `ynz-run` composed-fixture bullet removed; the send-on-full
    proof re-pointed to the runtime-substrate test + grep-audit; single-task hostile fixtures named;
    a parenthetical relocates the two-task composed proof to Phase 2's R5 gate.
Unchanged: Phase 2 text (already correct — already carries the identical two-task composed fixture as
  its own build-blocking R5 gate, so NO coverage is lost); Phase 0; Phases 3-6; the R1/R5/R6/R7
  risk-table rows and their scores (no re-scoring — residual stays L as designed; this FRAGO only
  relocates WHERE a proof-obligation is textually gated, not the risk score); §3.1 Intent/End-State
  outcomes; the Invariants section; Design-Doc Alignment; Future Requirements.
Override:  N/A — risk-neutral (deviation-judge classification, confirmed by Patrick via
  AskUserQuestion on 2026-07-02). No coverage is lost — Phase 2 already carries the identical fixture
  as its own build-blocking gate — so no signed override is required.

## FRAGO 003 — 2026-07-02 — session-id: executor-2026-07-02-m4-p1-r2 (P1 continuation; deviation-judge-classified, auto-applied per the risk-neutral disposition)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 0 STATUS Lock 11 + Phase 1 Step 2
Trigger:   Deviation B (surfaced by `executor-2026-07-02-m4-p1`, session log above) — Lock 11 (P0)
           resolved WHERE endpoint futures live (the runtime/handle object, NOT the frame header) but
           left open where the channel-LOCAL HANDLE POINTER persists across the calling task's OWN
           suspension on a bare `send()`-on-full in Phase 1 (a question distinct from Phase 2's
           handle-form endpoint-future placement). deviation-judge confirmed this resolves via the
           EXISTING crossing-local frame-slot mechanism — the same mechanism `emit.rs` already uses
           for locals crossing a `wait` point (`sm_crossing_names` / `FRAME_OFFSET_LOCALS_START`) —
           with NO new frame-header slot and no `FRAME_HEADER_SIZE`/`FrameLayout` ripple.
Changes:
  - `plan.md` Phase 0 STATUS, Lock 11 bullet: appended an explicit note that P1's bare-`send()`-on-full
    channel-local persistence is a SEPARATE, narrower question from P2's endpoint-future placement, and
    resolves via the EXISTING crossing-local mechanism (no new frame-header slot), confirmed by FRAGO 003.
  - `plan.md` Phase 1 Step 2: added the same note in the step's own text so the NEXT executor does not
    re-derive it — the channel-handle local persists across a bare-send suspension via the existing
    `sm_crossing_names` / `FRAME_OFFSET_LOCALS_START` crossing-local path.
Unchanged: everything else in Lock 11 (the frame-header-not-forced verdict, endpoints-in-runtime-object,
  the P2 endpoint-future placement decision); all other P0 design locks (8/9/10); all other phases; the
  risk table (no re-scoring).
Override:  N/A — risk-neutral, auto-applied per the deviation-judge's classification (narrow; it
  identifies the EXISTING mechanism that already resolves the question, introducing no new machinery).

## FRAGO 004 — 2026-07-02 — session-id: executor-2026-07-02-m4-p1-r5 (P1 completion; conductor-classified, Patrick-approved via the recommended default per the decision-philosophy safe-default protocol)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ the Phase 1 / Phase 2 boundary
Trigger:   Deviation C (surfaced by `executor-2026-07-02-m4-p1-r4`, session log above) — the channel
           send/recv suspension-codegen build's PLACEMENT is a phase-boundary scope call. deviation-judge
           VERIFIED independently (by reading the live code) that the channel send/recv
           suspend→persist→resume codegen path CANNOT be end-to-end-verified by any Phase-1 fixture, on
           two confirmed facts: (1) the crossing-local / suspension-detection machinery (`is_suspending_call`
           at `check.rs` — the r4 report's `~8332` may have shifted, verify via grep; `collect_crossings_in_stmts`;
           `count_suspension_expr` at `emit.rs`; the SM routing gate) is entirely function-NAME-keyed and does
           NOT fire on an `Expr::MethodCall` like `ch.send(v)` — real, material threading work beyond what
           FRAGO 003 implied; (2) a parked/suspended task cannot drain itself, so the suspend→resume path can
           ONLY be exercised by a two-task scenario (one suspends on send-full, another drains) — and FRAGO 002
           already established two-task channel sharing is a Phase-2-only capability (no `.share`/`.lend` across
           `background` in Phase 1). No single-task alternative exists (deviation-judge checked + confirmed
           explicitly). Building the suspension codegen in Phase 1 would ship unverified maximum-adversarial
           suspension IR for an entire phase — the "shipping on faith" posture R1/R5/dormant-override-#2 forbid.
           Disposition (b) — fold the send/recv suspension-codegen build into Phase 2 — was the deviation-judge's
           verified recommendation; Patrick approved it. Classification (conductor authority): risk-neutral
           relocation — it moves WHERE work is gated, not the risk assessment. Applied per the
           decision-philosophy safe-default protocol: Patrick did not respond within the timeout window, so the
           recommended default (verified independently by BOTH the r4 executor's live-code build-then-revert AND
           deviation-judge's separate code-read) was applied.
Changes:
  - `plan.md` Phase 1 — REWROTE scope to construction-only: header (`send/recv suspension codegen → Phase 2
    per FRAGO 004`, dropped the `⚠ M2-HALT-adjacent` marker — no suspension point remains in P1),
    Task+purpose, Steps (1 type/resolution, 2 construction+capacity+codegen, 3 kernel gate, 4 `channel_capacity`
    domain registered, 5 alloc=free, 6 single-task fixtures, 7 demo+gallery — REMOVED the `.send()`/`.receive()`
    method surface, the R6 sibling arm, the suspension codegen, Lock 8's typed-`errors` on `.send()`, and the
    backpressure teaching text), Exit criteria, and the P1 STATUS banner (concise COMPLETE banner replacing the
    r1–r4 saga — full history preserved in the session log above).
  - `plan.md` Phase 2 — GREW scope: Task+purpose now names the folded-in bare-channel send/recv
    suspension-codegen build (the R6 `channel_method_suspends` sibling arm, the type-awareness threading through
    the function-name-keyed crossing/counting/routing machinery — the real Finding-1 work — `emit_channel_suspend_point`,
    Lock 8 typed `errors`, backpressure text), notes the LIKELY-SHARED implementation between the bare-channel
    and handle-form suspension drives (ONE mechanism, build once — authoritative-derivation.md), added a new
    Step 2 (bare-channel method surface + suspension codegen) and renumbered Steps 3–8, added the bare-channel
    two-task send/recv suspend→resume proof to the Step-7 gates + Exit criteria, and grew the demo/gallery step
    to add the real send/recv round-trip + closed-channel/backpressure diagnostics.
  - `plan.md` §5 slice map: P1 now carries R7 + R1's runtime-substrate proof only; P2 carries
    R5+R2+R8+R1+R6-sibling-arm (R1's send/recv suspension-codegen gate relocated to P2).
  - `plan.md` frontmatter: session-id `executor-2026-07-02-m4-p1-r5` appended.
Unchanged: Phase 0; Phases 3–6; the R1/R5/R6/R7 risk-table SCORES themselves (NO re-scoring — this FRAGO
  relocates WHERE work is gated, not the risk assessment; residuals stay as designed); FRAGO 001/002/003
  (untouched, still valid); §3.1 Intent/End-State outcomes; the dormant overrides; the Invariants section;
  Design-Doc Alignment; Future Requirements.
Override:  N/A — risk-neutral relocation (conductor classification). Patrick did not respond within the
  timeout window, so the recommended default — verified independently by BOTH the r4 executor's live-code
  build-then-revert AND deviation-judge's separate code-read — was applied per the decision-philosophy
  safe-default protocol. No coverage is lost: the send/recv suspend→resume proof lands in Phase 2 where the
  two-task composed R5 fixture (already Phase 2's build-blocking gate) can end-to-end-verify it. No signed
  override required.

## Session log — executor-2026-07-02-m4-p2-r1 — 2026-07-02

Phase 2 execution, round 1 (PARTIAL — conductor ordered a wrap-up handoff mid-phase; a fresh
continuation dispatch picks up the remainder). Landed and verified, all through the real compiler
inside the dev container:

- R6 sibling arm (`channel_method_suspends` / `CHANNEL_SUSPENDING_METHODS`) in
  `suspension_source.rs`; threaded into may_block (new syntactic conduit-binding resolver),
  the crossing analysis (+`expr_types` threading through `locals_crossing_wait` /
  `collect_crossings_in_stmts` / `block_suspends_m3d` / shadow helpers + conduit-local
  crossing marking), suspension counting, and ONE new SM-router helper `stmt_needs_sm_walker`
  (replacing 7 scattered routing disjuncts — one of which, the auto-parallel Singleton arm,
  was initially missed and root-caused via the smoke fixture).
- Bare-channel `.send()`/`.receive()` typeck (Lock 8 `nothing errors` on send; elem-restricted;
  kernel gate; named-receiver + statement-position teaching errors) + the 3-way
  `emit_channel_suspend_point` codegen (per-frame caller_token; Closed→typed error via
  `ynz_error_new`; ChanRecv-Closed → loud `ynz_unhandled_error` abort — structurally
  unreachable in v0.3).
- Runtime: `channel.rs` refactored to Arc-shared + internally synchronized (per-caller-token
  pending sends; multi-waiter recv wakeups); `ynz_channel_share`/`_free`; NEW `handle.rs`
  (`ynz_rt_spawn_handle`, recv/send/free ABI; `HandleStateFnFuture` extracts the completion
  from the return slot BEFORE the frame-freeing drop — R8 copy-before-free by construction;
  `HANDLE_RET_KIND_*` seam constants in `ynz-abi`). Trap doors 1a/1b/1c structurally absent;
  no frame-header ripple (spike verdict honored).
- Handle-form lift: `Type::BackgroundHandle` (inferred-only), non-suspending callee teaching
  reject, `h.send` typed against the callee's first `channel<T>` param (recorded decision),
  `h.receive` = `T errors`; `lower_let_background_handle` → compile-time spawn-form-keyed
  `ynz_rt_spawn_handle` (bare spawn IR byte-for-byte unchanged — golden diffs are new
  declare lines only). Channel args to `background` are refcount-SHARED
  (`BgOwnership::Channel`, `BgArgFreeKind::SharedChannel`, drop-ladder kind 2), with the
  `lend`/`share` borrow rejects exempting channel-typed params.
- Fixtures GREEN via `ynz run` (30s guards, none tripped): smoke, composed (R5), composed-bare
  (FRAGO 004 proof), R8 matrix (ok before/after, err, number wide-value, fire-forget
  unchanged), composed R5×R8 cell, never-received, pool-exhaustion (12 handles),
  never-drained. Full workspace suite: 2139 passed / 0 failed (one transient ynz-driver
  integration flake in one of three runs, not reproduced, not root-caused — continuation
  should watch it). 13 IR goldens re-accepted (declaration-only diffs). Stale
  `m8_background_let_binding_rejected` replaced by non-suspending-reject + suspending-clean
  tests. Jargon audit fixed ("result" → "value" in the nested-position teaching error).

NOT done (remainder for the continuation, per the wrap-up order): Step 4 deferral RECORDING
(runtime abort path exists + tested; language-wide scope-drop wiring deferral text + registry
entry pending), Step 6 registry retirements + new deferred entries, Step 7 test-wiring of the
GREEN fixtures (build-blocking integration tests, alloc=free assertions, --no-auto-parallel
variants, cpu_admission decline fixture, R1/R6/no-runtime-conditional grep-audits, tripwire
test extension), Step 8 demo + gallery. Deviations surfaced in the executor's return report
(handle `.send` typing decision; elem-type restriction; ChanRecv-Closed reachability note;
non-suspending-callee handle deferral).

## Session log — executor-2026-07-02-m4-p2-r2 — 2026-07-02

Phase 2 execution, round 2 (COMPLETION — closed the r1 wrap-up's remaining steps 4/6/7/8;
phase exit criteria met; full workspace suite run at close). All verification through the real
compiler inside the dev container.

- **Design-doc alignment check (r1's flagged `h.send` typing decision): CONFIRMED, no
  contradiction.** IMP-concurrency:165-183 + REF-concurrency:165-173 define only the
  parent-side handle surface; no child-side read mechanism or implicit-inbox concept exists in
  any design doc. The first-`channel<T>`-parameter typing stands (consistent with
  bounded-by-default and the teaching test). On record ahead of the phase-boundary reviewer
  fan-out.
- **Step 7 (build-blocking wiring):** 16 `v0_3_m4_p2_*` gates in `integration.rs` — every r1
  fixture now asserts expected stdout + `--no-auto-parallel` byte-identical + alloc==free AND
  alloc>0; R8 spawn-form-keying proven structurally at the IR level (fire-and-forget: 1
  `ynz_rt_spawn`/0 handle; collected: the inverse); new `cpu_admission` DECLINE→FIRE fixture
  pair (`v0_3_m4_channel_cpu_admission_declines.ynz` 0 spawns / `_fires.ynz` 2 spawns —
  decline attributable to channel classification, masked-branch discipline); R1 grep-audit
  made permanent (`ynz-runtime/tests/no_blocking_in_conduit_path.rs`); R6 tripwire extended
  to the conduit-method list — which caught a REAL latent twin (`check.rs` known-method
  `matches!(method, "send" | "receive")`) now threaded through `channel_method_suspends`.
- **Step 6 (registry):** retired `ec-wrapper-collect-on-completion` +
  `background-handle-form` (removed + shipped-comments per the cross-module-frame precedent);
  added `background-handle-cancel-injection`, `channel-op-expression-position`,
  `background-handle-nonsuspending-callee`. Consistency + jargon suites green.
- **Step 4 (deferral recording):** four-field deferral in plan.md Future Requirements
  (language-level scope-drop wiring deferred; runtime abort path shipped + substrate-proven;
  `ynz_handle_free` declared-but-never-emitted confirmed as the intended v0.3 scope per the
  §3.1 locked decision). No new code.
- **Step 8 (demo + gallery):** `pirates-roster` `m4_demo()` — real capacity-1 backpressure
  round-trip + handle send/receive cycle; golden regenerated (M2 window reorder is inside the
  test's existing presence-only relaxation; deterministic tail byte-exact). `v0_3_m4_errors.ynz`
  grew 8 Phase-2 triggers (13 total, one per class) + the documented closed-channel-send
  runtime-error block; `error_galleries.rs` asserts count 12–15 + 9 new key phrases incl. the
  backpressure teaching text.
- **Gate hygiene:** `cargo clippy --workspace -- -D warnings` was RED on four r1 lints —
  fixed (ownership-only `shared` Arc kept with `#[expect(dead_code)]` + UAF-invariant comment;
  redundant closure; unused mut; type_complexity alias). `cargo fmt --all` applied. The r1
  transient integration flake did not recur.

Files touched: `crates/ynz-typeck/src/check.rs` (authoritative-source threading + clippy),
`crates/ynz-typeck/tests/suspension_source_single_definition.rs` (method-list tripwire),
`crates/ynz-runtime/tests/no_blocking_in_conduit_path.rs` (NEW), `crates/ynz-runtime/src/handle.rs`
(expect-attribute + comment), `crates/ynz-codegen/src/emit.rs` (clippy only),
`crates/ynz-driver/tests/integration.rs` (16 gates), `crates/ynz-driver/tests/error_galleries.rs`,
`crates/ynz-driver/tests/fixtures/v0_3_m4_channel_cpu_admission_{declines,fires}.ynz` (NEW),
`registry/features.toml`, `examples/pirates-roster/{entrypoint.ynz,expected_stdout.txt}`,
`examples/primantis-orders/v0_3_m4_errors.ynz`, `plan.md` (Progress r2 + Future Requirements
deferral + session chain).

Final close-out (same session): registry retirements rippled into the tmgrammar
deferred-features pattern — committed `tooling/vscode-ynz/syntaxes/ynz.tmLanguage.json`
regenerated via `cargo run -p ynz-tmgrammar` (the snapshot test's own remedy; 1-line diff).
Full workspace suite: 2158 passed / 0 failed, exit 0 (r1's 2139 + exactly the 19 new gates:
16 integration + 2 runtime R1-audit + 1 tripwire). Clippy `-D warnings` green; fmt applied;
the r1 transient flake did not recur in two full runs. Phase 2 exit criteria all met —
phase-boundary reviewer fan-out (code-reviewer + adversarial-tester + opus adversarial gate)
is the conductor's next dispatch, not this executor's.

## FRAGO 005 — 2026-07-02 — session-id: executor-2026-07-02-m4-p2-r3 (Phase 2 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 2 (handle-form `.send()` typing surface)
Trigger:   Deviation surfaced by `executor-2026-07-02-m4-p2-r1` (the "handle `.send` typing
           decision" in its return report) and design-doc-checked by `-p2-r2`: IMP-concurrency.md
           (:165-183) and REF-concurrency.md (:165-173) define only the PARENT-side
           `.send()`/`.receive()` handle surface — NO child-side read mechanism is documented in
           any design doc. Reality: the compiler types `h.send(v)` against the CALLEE'S FIRST
           `channel<T>` parameter (the conduit the send feeds) — compile-time-safe, and the only
           non-contradictory choice for a genuinely underdetermined surface. Live-code citations
           (grep-verified this round, not carried from prior reports): `check.rs:1375` (handle-form
           surface doc), `check.rs:1456-1457` (`msg_elem` = the element type of the callee's first
           `channel<T>` parameter), `check.rs:3059-3066` (task-takes-no-channel teaching error) +
           `check.rs:3097-3103` (element-type-mismatch teaching error) — both diagnostics name the
           first-`channel<T>`-parameter convention in their WHY text. deviation-judge
           classification: JUSTIFIED — the docs are silent and a shipping compiler must pick
           something; risk-neutral (no re-scoring).
Changes:
  - `plan.md` Phase 5 Step 2: appended a clause requiring the `REF-concurrency.md` update (already
    a planned P5 step) to EXPLICITLY document the h.send-feeds-the-callee's-first-`channel<T>`-
    parameter convention — the FRAGO's follow-up obligation. The convention must land in the user
    spec, not remain implicit in code comments only.
Unchanged: the shipped `h.send` typing itself (landed at P2 r1, design-doc-checked at P2 r2 —
  this FRAGO records it, nothing code-level moves); all Phase 2 text; Phases 0-4 and 6; the risk
  table (NO re-scoring — risk-neutral); §3.1 Intent/End-State; the Invariants section; Design-Doc
  Alignment; Future Requirements; FRAGO 001-004.
Override:  N/A — risk-neutral record of a deviation-judge-JUSTIFIED classification; no signed
  override required.

## FRAGO 006 — 2026-07-02 — session-id: executor-2026-07-02-m4-p2-r3 (Phase 2 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 2 (channel runtime sharing model) +
           Phase 3 Step 2 (boundary exactness — the should-fix companion)
Trigger:   Phase 1's single-owner `&mut` channel model was explicitly scoped OUT of cross-task use
           (FRAGO 002 / Deviation A, on record above). Phase 2's OWN build-blocking R5 composed
           fixture — a `background`-shared channel where one task suspends on send-on-full while
           another drains — is impossible without safe shared access. Reality forced the refactor:
           `crates/ynz-runtime/src/channel.rs` rewritten Arc-shared + internally synchronized
           (per-caller-token pending sends, multi-waiter recv wakeups, `ynz_channel_share`/`_free`
           refcounting — session entry `executor-2026-07-02-m4-p2-r1`). deviation-judge
           classification: JUSTIFIED — reality forced it, not scope creep; risk-neutral.
           SHOULD-FIX COMPANION (the reason this FRAGO carries a plan amendment): the refactor
           produced a concrete EXEMPTION of `channel<T>`-typed parameters from the
           share/lend-across-`background` rejects — the `borrowed_non_channel` closure at
           `check.rs:2455-2467` (exemption comment 2455-2459; the share reject now fires at 2468,
           lend at 2477; grep-verified this round — the r2-draft's `2275-2280`/`2287-2292`
           citations are stale, and the non-ident/unresolvable-callee silent-skip edge is now
           `check.rs:2442-2454`). That exemption sits directly inside the code Phase 3's R3
           boundary matrix is chartered to exhaustively test, so it must be plan-text ground truth
           for Phase 3's executor — not silently rediscovered or, worse, re-derived as a twin
           (`authoritative-derivation.md`).
Changes:
  - `plan.md` Phase 3 Step 2: inserted a "FRAGO 006 note — Phase 2 shipped ground truth" block —
    (a) the channel-param exemption with its CURRENT grep-verified citations
    (`check.rs:2455-2467`, rejects at 2468/2477), to be READ as already-shipped ground truth for
    the "should-Arc is not falsely rejected" cell on channel-typed values (verify it holds, do NOT
    reimplement); (b) an explicit staleness flag on the step's r2-draft `check.rs` citations
    (`2275-2280`/`2287-2292`/`2269-2270` — silent-skip edge now `2442-2454`), directing Phase 3's
    executor to re-grep for current lines rather than trust the plan's citations.
Unchanged: the shipped Arc-shared refactor itself (landed at P2 r1, gate-wired at P2 r2 — this
  FRAGO records it, nothing code-level moves); Phase 3 Steps 1/3/4/5 and its exit criteria (the
  matrix obligation is unchanged — the note only feeds it ground truth); Phases 0-2 and 4-6; the
  risk table (NO re-scoring — R3's matrix mitigation stands exactly as designed); §3.1; the
  Invariants section; Design-Doc Alignment; Future Requirements; FRAGO 001-004.
Override:  N/A — risk-neutral record of a deviation-judge-JUSTIFIED classification; no signed
  override required.

## FRAGO 007 — 2026-07-02 — session-id: executor-2026-07-02-m4-p2-r3 (Phase 2 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 2 (channel element-type surface)
Trigger:   Phase 2 shipped a channel element-type restriction the plan never spelled out:
           `shape`/`number` elements REJECTED at channel construction; only
           int/float/boolean/string/array<T>/map<K,V> allowed (`check.rs:3151-3185` — the
           `elem_supported` `matches!` at 3158-3167, the teaching diagnostic at 3168-3185;
           grep-verified this round). Technically forced, not arbitrary: channels carry values
           through a single 64-bit slot (scalars by value, heap-stable pointers otherwise); a
           `shape`/`number` value is backed by SENDER-STACK storage that is gone by the time the
           receiver reads it — the same dangling-pointer class the pre-existing
           `UnsupportedCrossingLocalType` safety discipline (M3a) rejects for locals crossing a
           `wait`. deviation-judge classification: JUSTIFIED; risk-neutral. SHOULD-FIX COMPANION:
           the code comment's own WHY text (`check.rs:3182`: "Per-type heap-copying for these
           ships in a later milestone") promises a deferral that never got its
           `[[deferred_language_feature]]` registry entry — despite the elem-type restriction
           being surfaced as a deviation in the P2-r1 return report — a feature-registry-rule
           violation (deferred features are registry SSOT entries, not code-comment promises).
Changes:
  - `registry/features.toml`: NEW `[[deferred_language_feature]]` entry
    `channel-element-heap-upgrade` (inserted with the other Phase-2 channel entries, after
    `background-handle-nonsuspending-callee`, before `seq-cst-ordering-opt-in`): substitute =
    send fields separately / pack into array-or-map / heap-allocated container the user manages;
    why = single-64-bit-slot ABI cannot safely carry sender-stack-backed values across a task
    boundary (dangling-pointer class, mirrors `UnsupportedCrossingLocalType`), lifting needs a
    per-type heap-upgrade at the send site; ships_in = "v0.4+" (the code comment promises only
    "a later milestone" with no version — "v0.4+" is the honest floor: no remaining v0.3-M4 phase
    plans it and Phase 6 closes v0.3); design_doc = IMP-concurrency.md; triggers = shape/number
    channel construction (rejected today) → lift when per-type heap-upgrade ships.
    Validated: `docker compose run --rm dev cargo build -p ynz-registry` GREEN + full
    `cargo test -p ynz-registry` GREEN (64 tests across 5 binaries incl. the consistency suite,
    0 failed, exit 0).
Unchanged: the shipped restriction itself and its teaching diagnostic (correct as-built — this
  FRAGO records it and back-fills the missing registry seam, nothing code-level moves); all
  phases' text (no plan.md body change rode on this FRAGO); the risk table (NO re-scoring);
  §3.1; the Invariants section (the plan's `### Feature Registry Entries` obligation is what this
  entry satisfies); Design-Doc Alignment; Future Requirements; FRAGO 001-006.
Override:  N/A — risk-neutral record of a deviation-judge-JUSTIFIED classification; no signed
  override required.

## Session log — executor-2026-07-02-m4-p2-r3 — 2026-07-02

Narrow post-review follow-up dispatch (task-spec from the conductor, routing deviation-judge's
Phase-2 verdict: 3 JUSTIFIED FRAGO candidates + 2 should-fix findings). Producer does NOT
self-grade — recorded, not re-adjudicated; all classifications are deviation-judge's, applied
per the conductor's routing. Four deliverables, all landed this dispatch:
(1) FRAGO 005/006/007 recorded above in the canonical Base/Trigger/Changes/Unchanged/Override
shape — every `check.rs` citation re-grepped against the live tree this round (exemption
`2455-2467` + rejects `2468`/`2477`; silent-skip edge `2442-2454`; elem-type restriction
`3151-3185` with the "later milestone" promise at `3182`; h.send first-channel-param typing
`1375`/`1456-1457`/`3059-3066`/`3097-3103`), never trusted from prior reports.
(2) `plan.md` Phase 3 Step 2 amended in place (FRAGO 006 note: exemption as ground truth +
stale-citation flag). (3) `registry/features.toml` `channel-element-heap-upgrade` added
(FRAGO 007 companion); `ynz-registry` build + full test suite GREEN (64/64, exit 0) in the dev
container. (4) `plan.md` Phase 5 Step 2 amended in place (FRAGO 005 follow-up: REF-concurrency
must document the h.send convention explicitly). Frontmatter session-id chain appended.
Nothing else in Phase 2 or elsewhere touched — no `.rs` source, no examples, no docs. No new
deviation surfaced; NO STOP condition fired; NO dormant override armed. Session-id appended;
status remains active.

## Session log — executor-2026-07-02-m4-p3 — 2026-07-02

Phase 3 (auto-Arc cross-thread wrapping + boundary exactness). Grounded in: the plan's Phase 3
slice (§3.3), §2 Mission, §3.1 Intent & End State (outcome 5 + the auto_arc red-tint staging
decision), §3.4 CCIR (d)/(e), the Invariants Safety/Performance/Teaching auto-Arc bullets, and
Design-Doc Alignment #2/#4/#8; the design docs IMP-no-function-coloring (§Runtime item 4 auto-Arc,
"Atomic Ordering Default" acquire-release), IMP-concurrency ("Ownership with Background Tasks — Why
`.share` Fails", :189/:878), IMP-ownership (cited as the auto-Arc detail home — found NOT to
actually specify the mechanism); FRAGO 006 (Phase 2 channel exemption ground truth) and FRAGO 007;
authoritative-derivation.md; no-duct-tape.md; verification.md. Every check.rs citation re-grepped
against the live tree this session (never trusted from the plan text or FRAGO 006, both of which
flag their own citations stale) — e.g. `borrowed_non_channel` was at check.rs:2516 at grep time
(the plan/FRAGO cited 2455-2467/2275-2280); the silent-skip guard at check.rs:2509-2510.

WHAT I BUILT (all verified through the real compiler `./target/debug/ynz run` in the dev
container, never by assertion):

- **R3 boundary-exactness matrix — the load-bearing build-blocking safety gate (11 GREEN cells).**
  11 fixtures `crates/ynz-driver/tests/fixtures/v0_3_m4_p3_*.ynz` + 11 tests in
  `crates/ynz-driver/tests/integration.rs` (`v0_3_m4_p3_*`). Full cross-product both directions:
  share/lend (concrete + generic + UFCS-method) REJECT loudly; give/copy/channel CROSS SAFELY (no
  false reject; `cross_copy` keeps the caller's `42`); non-ident + unresolvable callees ERROR
  loudly (not a silent hole). Cross cells: `--no-auto-parallel` byte-identical + alloc==free +
  alloc>0; stable across 5 repeated runs. Matrix NOT narrowed to pass (per the disciplined-initiative
  fallback) — the "should-Arc" cells assert the SAFE crossing (via copy/give), which IS the R3
  safety property; auto-Arc is the deferred perf-form of that same crossing (see the surfaced
  deviation).

- **Silent-skip gap CLOSED — recorded decision (Step 2 "reject it loudly").** VERIFIED empirically
  (probe `background reveal<Config>(cfg)` with a `share` param) that the gap was a
  teaching-consistency gap, NOT a memory-safety hole — the background ABI heap-copies the arg into
  the task ctx regardless of the callee's declared borrow, so no live-borrow ever dangled; a
  generic share-param spawn simply ran by copy. Chose to CLOSE (over merely record) because it is
  chartered Step-2 work, cheap, makes the boundary exact for the future emission, and removes a
  latent hole. Closed by EXTENDING the ONE `borrowed_non_channel` predicate to resolve generic
  callees via `generic_fn_table.fns` as a sibling `.or_else` arm (never a forked second predicate —
  authoritative-derivation.md), `crates/ynz-typeck/src/check.rs:2509-2560`. Verified no existing
  fixture spawns a generic function under `background` before changing behavior; typeck suite (93+
  tests) stayed green.

- **Channel exemption VERIFIED holding (FRAGO 006 ground truth), not reimplemented.**
  `cross_channel_exempt` proves `background feed(share ch: channel<int>)` is exempt and round-trips.
  The exemption clause (`!matches!(ty, BuiltinChannel)`) is untouched.

- **Auto-Arc runtime substrate** `crates/ynz-runtime/src/arc.rs` (NEW; exported lib.rs:5):
  `ynz_arc_new/clone/free`, counted-alloc-backed (so alloc=free proves the control block freed),
  acquire-release refcount discipline (Relaxed clone / Release-dec + Acquire-fence last-drop — NOT
  seq-cst, per IMP-no-function-coloring "Atomic Ordering"). 3 unit tests incl. an 8-thread×1000
  concurrent hammer. This is the sound realization of the design's mandated ordering and the ready
  hook for the deferred emission.

- **Registry:** `auto_arc` `[[muted_hint_domain]]` (Informational, cautionary WHAT/WHY hover)
  registered-not-yet-firing (channel_capacity precedent); `auto-arc-cautionary-tint`
  `[[deferred_tooling_feature]]` (pre-authorized §3.1 — no per-hint tint path in ynz-lsp; teaching
  text ships, only color stages); `auto-arc-codegen-emission` `[[deferred_language_feature]]` (the
  SURFACED emission deferral — see below). tmgrammar regenerated + committed. jargon_audit green
  (reworded one "implementation" occurrence).

- **Demo + gallery:** gallery grew the auto-Arc boundary trigger (generic share-param callee, the
  closed gap; error_galleries.rs count 14→15, range 14–17, + boundary key-phrase); demo grew a
  documented auto-Arc placeholder (P1 precedent — emission deferred, comment-only, golden unchanged).

DEVIATION SURFACED (NOT self-decided — for deviation-judge / conductor; no `## FRAGO` block
self-filed, per the executor charter / agent-charter-discipline.md):

- The plan's Phase 3 exit criteria assume the auto-Arc CODEGEN EMISSION ships this phase (a RUNNING
  auto-Arc program: "correct under repeated runs", "hint fires", "alloc=free — Arc control blocks
  freed"). It does NOT. A sound-and-beneficial emission is blocked on a design-underspecified
  sharing TOPOLOGY (IMP-no-function-coloring:58 → IMP-ownership.md, which does not actually specify
  it; IMP-concurrency:189 locks `.share`-across-background as an error) — a DESIGN call, not an
  executor call — and would require reusing `ynz_typeck::effective_ownership` (`Reads`,
  queries.rs:491) as the read-only oracle, never re-deriving the fragile removed mutation analysis
  (IMP-concurrency:878, the authoritative-derivation corpse). A minimal single-value emission is a
  pessimization (Golden Rule 8), so there is no small sound-and-beneficial slice; a speculative one
  risks R3's false-Arc silent data race (dormant override #3 class). The R3 SAFETY property holds
  today WITHOUT the emission: the read-only value crosses safely via copy (proven by `cross_copy`).
  Surfaced in the plan's P3 STATUS banner + the return report; recorded factually as the
  `auto-arc-codegen-emission` registry deferral (explicitly marked SURFACED-pending-classification).
  Conductor disposition: (a) accept the deferral, or (b) re-dispatch to implement emission after a
  topology design decision. I do not decide the FRAGO.

Verification commands (dev container): `cargo test --workspace` → ALL_GREEN; `cargo clippy
--workspace -- -D warnings` → clean; `cargo fmt --all` applied; `cargo test -p ynz-driver --test
integration v0_3_m4_p3` → 11 passed; `cargo test -p ynz-driver --test error_galleries v0_3_m4` →
1 passed; `cargo test -p ynz-runtime --lib arc::` → 3 passed; `cargo test -p ynz-registry` → green;
`cargo test -p ynz-tmgrammar` → green (after regenerate). NO STOP condition fired; NO dormant
override armed. Session-id appended; status remains active.

## FRAGO 008 — 2026-07-03 — session-id: executor-2026-07-02-m4-p3-r2 (Phase 3 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 3 (auto-Arc codegen emission scope)
Trigger:   Deviation surfaced by `executor-2026-07-02-m4-p3` (the ⚠ P3 STATUS banner + return
           report; recorded factually in `registry/features.toml` as the
           `auto-arc-codegen-emission` `[[deferred_language_feature]]`, explicitly marked
           SURFACED-pending-classification — NOT self-decided). deviation-judge classification,
           applied here verbatim: Phase 3 exit criteria (plan.md §3.3, "auto-Arc programs correct
           under repeated runs"; "hint fires with correct hover"; "alloc=free — Arc control blocks
           freed") and §3.1 Key Outcome #5 assume the auto-Arc CODEGEN EMISSION ships this phase;
           reality is the safety-half (boundary matrix, 11/11 GREEN) + runtime substrate (arc.rs,
           hammer-tested) + registered-not-firing hint domain ship, emission does NOT — JUSTIFIED:
           IMP-no-function-coloring:58 points to IMP-ownership.md for the sharing-topology
           mechanism, but IMP-ownership.md carries zero Arc content (grep-verified), and
           IMP-concurrency:189 locks `.share`-across-`background` as a hard error, creating a
           genuine, verified design gap on WHO ends up sharing WHAT allocation — a load-bearing
           safety decision no executor should freelance, distinct from the r3 corpse (CCIR-h)
           where the mechanism WAS fully specified and explicitly gated to this milestone.
           Risk-neutral: R3's scored HIGH-severity residual is earned entirely by the shipped
           boundary matrix, unaffected by the emission deferral (the pre-existing safe deep-copy
           crossing is unchanged). Disposition taken: (a) accept the deferral — emission → v0.4+ /
           a topology-design pass (per the surfaced options; classification auto-applied+logged as
           risk-neutral, no signature needed).
Changes:
  - `plan.md` Phase 3 exit criteria: annotated in place — original text preserved, each sub-claim
    marked MET (the boundary-safety matrix, GREEN + build-blocking) vs DEFERRED (the codegen
    emission and everything downstream of it: the hint firing, the alloc=free-for-Arc-control-
    blocks proof of a running program, the "correct under repeated runs" claim for an actual
    Arc-wrapped program), so a future reader is not misled about what shipped.
  - `plan.md` §3.1 Key Outcome #5: split in place per `plan-source-of-truth.md`'s headline-vs-
    deferrals reconciliation — the boundary-exactness half is marked MET; the "Auto-Arc wraps
    cross-thread shared state" half is marked DEFERRED to the registry deferral (the headline no
    longer promises what the plan's own body defers).
  - `plan.md` Future Requirements / Revisit: ADDED an `auto-arc-codegen-emission` entry pointing
    at the `registry/features.toml` `[[deferred_language_feature]]` entry as the single source of
    the four-field WHAT/WHY/COST/TRIGGER record (mirrored, not duplicated).
  - `plan.md` ⚠ P3 STATUS banner: updated SURFACED-DEFERRED-pending-classification → classified
    JUSTIFIED + applied as FRAGO 008; the surfaced-deviation block's "conductor disposition
    options" tail updated to record disposition (a) taken.
  - `plan.md` frontmatter: session-id `executor-2026-07-02-m4-p3-r2` appended.
Unchanged: everything code-level (the matrix, arc.rs, check.rs, fixtures, registry — nothing
  moves; this FRAGO reconciles plan text with already-shipped-and-reviewed reality); Phases 0-2
  and 4-6; the risk table (NO re-scoring — risk-neutral per the classification above; R3's matrix
  mitigation stands exactly as shipped); the Invariants section; Design-Doc Alignment; the three
  reviewer should-fix findings (routed as roadmap-level deferral records in
  `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`, not fixed this phase — see
  this session's log entry); FRAGO 001-007.
Override:  N/A — risk-neutral, no signed override required (R3's shipped boundary matrix already
  earns its scored residual; the deferred emission doesn't touch that mitigation).

## Session log — executor-2026-07-02-m4-p3-r2 — 2026-07-03

Narrow post-review bookkeeping dispatch (task-spec from the conductor, routing the reviewer
fleet's Phase-3 verdicts: 1 deviation-judge-JUSTIFIED FRAGO candidate + 3 should-fix findings).
Producer does NOT self-grade — every classification applied here is the deviation-judge's /
reviewers', per the conductor's routing (`agent-charter-discipline.md`). No production or test
code touched this round. Deliverables, all landed this dispatch:
(1) FRAGO 008 recorded above (canonical Base/Trigger/Changes/Unchanged/Override shape) applying
the deviation-judge's JUSTIFIED classification of the auto-Arc emission deferral; risk-neutral,
auto-apply+log, no signature.
(2) `plan.md` reconciled in place per FRAGO 008's Changes list: Phase 3 exit criteria split
MET/DEFERRED; §3.1 Key Outcome #5 split per the headline-vs-deferrals discipline; Future
Requirements entry added (points at the registry entry as single source); P3 STATUS banner
updated to classified; frontmatter session-id appended.
(3) Three should-fix findings routed to their durable per-phase deferral home —
`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md` (NEW file; the roadmap dir
had no audit sidecar) — as phase-boundary-attributed four-field deferral records with
Idempotency-Key sentinels: (a) the R3 matrix header's "full cross-product" overclaim
(`integration.rs:9773`), (b) the arc.rs hammer test never exercising the contended 1→0
last-release window (`arc.rs:147`), (c) the "stable across 5 repeated runs" claim being an
ephemeral manual observation, not CI-enforced coverage (`plan.md:963` / `audit.md:959`).
Grep-verified all three anchors against the live tree before recording. NOT fixed this phase —
non-blocking, deferred by reviewer disposition; TRIGGERs bind them to the auto-Arc emission
pickup (FRAGO 008's deferral) or the next touch of the affected surfaces.
Recorded decision: finding-slug for the plan.md anchor computed by the literal collapse recipe
(lowercase; each maximal non-[a-z0-9] run → one hyphen), which yields a LEADING hyphen from the
anchor's leading `.` — kept as-is for deterministic re-derivation (the recipe has no trim step).
The pre-existing dirty `roadmap.md` (unrelated changes) untouched. NO STOP condition fired; NO
dormant override armed. Session-id appended; status remains active.

## Session log — executor-2026-07-02-m4-p4 — 2026-07-03

Phase 4 executed in full (all five steps), grounded in: the plan slice (¶2 Mission, §3.1
outcomes 6–7, §3.4, §Invariants Performance/Teaching/Feature-Registry), REF-plan-format,
REF-decision-philosophy, REF-context-budget, no-duct-tape, verification, auto-promotion.md,
feature-registry.md, authoritative-derivation.md, plan-invariants §Demo & Error Gallery,
examples-structure.md, IMP-no-function-coloring (§False Sharing, §Sleep Intrinsics),
IMP-feature-registry, `suspension_source.rs` + the live crate reality (recon re-read, not
memory).

**Landed.** (1) `[[lint_rule]]` entry-kind built generically end-to-end: TOML schema +
build.rs parser (severity "error" rejected at parse time) + `LintRuleEntry` typed constants +
accessors/`lint_rule_diagnostic_parts`/`lsp_lint_rule_hover_for` in ynz-registry +
`DiagnosticKind::LintRule` (LSP `Diagnostic.code` = kebab-case rule id, flows through the
existing diagnostic_transform path unchanged) + the one generic typeck firing helper
(`lints.rs::lint_diagnostic`); two entries: `cross-thread-fields-not-padded`,
`prefer-yielding-sleep`; schema documented in IMP-feature-registry.md. (2) False-sharing
padding: raw cross-thread record captured at the single `Expr::Background` typeck arm (both
spawn forms route through it); ONE partition (`false_sharing.rs::finalize_false_sharing`) →
`TypedModule::cross_thread_padded_shapes`, threaded into codegen layout AND frame sizing
(both `emit_shape_types` call sites); per-field `{T,[pad x i8]}` 64-byte slots (index-
preserving), 64-byte padded-lit allocas, const-global fold declined for padded shapes.
(3) Both lints fire live with three-part registry text; sleep lint kernel-gated off.
(4) `--no-auto-parallel` gate proven at analysis level (own-process env test) AND end-to-end
(`v0_3_m4_p4_padding_gate.ynz`: padded IR default / genuinely-unpadded IR sequential /
byte-identical stdout); field offsets proven 64·i with ABI size 64·n on the real target data
layout (`false_sharing_padding.rs`); auto-reorder conflict: verified NO reorder pass exists in
codegen (declaration-order layout; "reorder" is prose-only) — criterion holds with evidence.
(5) pirates-roster padding demo + golden regenerated (stable ×3; the 8-pirates order variance
is the known relaxed section, verified pre-existing at pristine HEAD); v0_3_m4_errors.ynz
gained both lint triggers with `// WHY:`; error_galleries assertions extended (error count
unchanged).

**Verification.** In the dev container: `cargo test --workspace` GREEN (121 suites, 0
failures — including the previously-failing `design_future_sync`, now passing); `cargo clippy
--workspace -- -D warnings` GREEN; `cargo fmt --all` applied + `--check` clean; targeted runs
of every new test file GREEN; both lints observed rendering through the real `ynz run`.

**Deviations SURFACED, not self-adjudicated (full text in the P4 STATUS banner, plan.md):**
D1 `--no-auto-parallel` premise vs `background`-still-spawns reality (exit criterion met as
written; residual perf-only-safe); D2 "existing shape-field auto-reorder" is design intent,
not code; D3 IMP-no-function-coloring:201's share/lend detection premise is stale against
P3's hard-error lock (P6 design-doc sweep flagged); D4 unpaddable class realized as
cross-module-visible layout (FFI shapes don't exist in v0.3). For the deviation-judge; no
FRAGO decided or applied by this producer.

**Recorded decisions (reasons in the P4 banner + code comments):** wrapper-element padding
(zero index remapping); conservative-inclusive crossing collection; declaration-based
large-copy estimate; cross-TU-visibility decline class; golden.rs harness gates on
non-Suggestion diagnostics (mirrors typeck's assert_clean ratchet — suggestions are
informational by design, not a weakened gate).

Producer does NOT self-grade — reviewer fan-out (code-reviewer: mechanism generality +
design-doc diff) has NOT run. NO STOP condition fired; NO dormant override armed; the
pre-existing dirty `roadmap.md` + `cspell.json` untouched. Session-id appended; status
remains active.

## FRAGO 009 — 2026-07-03 — session-id: executor-2026-07-02-m4-p4-r2 (Phase 4 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 4 (false-sharing padding + decline-lint
           current usefulness)
Trigger:   deviation-judge second-round follow-up on the P4 diff, REFINING D3/D4 (which correctly
           explained WHY the detection trigger had to change but did not record that the realized
           trigger currently produces zero benefit): the padding transform and its paired
           decline-lint (`cross-thread-fields-not-padded`) are currently inert-with-real-cost —
           they pad/decline exclusive-ownership give/copy/channel crossings that structurally
           cannot false-share. False sharing requires two threads concurrently accessing the SAME
           instance's neighboring fields; every crossing v0.3-M4 can produce today is an
           exclusive-ownership handoff (`give` + use-after-give hard error; `.copy` = separate
           instance; channel = moved payload). The ONE mechanism that would create a
           genuinely-shared concurrent instance — auto-Arc codegen emission — is itself validly
           deferred to v0.4 by FRAGO 008 (this same plan). So the transform imposes real memory
           cost (64 bytes/field on module-local shapes crossing a `background` boundary) for zero
           current throughput benefit, until auto-Arc ships. Classification: JUSTIFIED,
           risk-neutral; disposition (a) taken — ship as a documented forward-looking no-op with
           a four-field WHAT/WHY/COST/TRIGGER honesty record.
Changes:
  - ¶3.3 Phase 4 STATUS banner: ADDED the four-field FRAGO 009 deferral note (WHAT: transform +
    decline-lint inert-with-real-cost, zero throughput benefit, 64 bytes/field bloat on
    module-local `background`-crossing shapes; WHY: auto-Arc emission — the one mechanism that
    creates genuine sharing — is FRAGO 008-deferred to v0.4, and the plumbing built here is what
    that emission threads into rather than re-derives; COST: bounded, 64 bytes/field on the
    narrow crossing class only; TRIGGER: auto-Arc codegen emission shipping — shares registry
    entry `auto-arc-codegen-emission`'s own TRIGGER), explicitly marked as REFINING (not
    overturning) D3/D4; D3/D4 entries annotated with their applied classifications; banner
    headline updated COMPLETE-pending-fan-out → review verdicts APPLIED.
  - ¶Future Requirements / Revisit: ADDED the matching mirrored entry. Deliberately NOT a
    registry `[[deferred_language_feature]]`: the padding transform ships as real, working,
    tested code — this deferral is about its CURRENT USEFULNESS pending auto-Arc, not about
    deferring the code itself; recorded as a no-duct-tape documented-tradeoff
    (WHAT/WHY/COST/TRIGGER), the four-field record living in the P4 banner.
  - `examples/pirates-roster/entrypoint.ynz` (~:1083 InningTally declaration comment,
    ~:1128 m4_demo call-site comment): CORRECTED per the deviation-judge's bundled honesty
    requirement — the prior prose claimed padding stops two tasks "fighting over one line",
    implying concurrent field access; false, since `inning` is GIVEN to `tallyInning`
    (use-after-give is a hard compile error), so access is strictly sequential/exclusive.
    Rewritten to state the shape IS padded (real, observable) as forward-looking infrastructure
    illustrating the MECHANISM, with no live contention prevented YET (arrives with auto-Arc,
    v0.4). Pure comment change; the demo's printed output untouched; golden verified
    byte-identical through the real byte-exact golden test post-change.
  - ¶1 Risk Assessment: NO CHANGE (risk-neutral — no re-scoring of any row).
Unchanged: everything code-level in the padding transform, both lints, tests, and registry
  entries (the transform ships exactly as reviewed — real, working, tested); FRAGO 001-008;
  Phases 0-3 and 5-6 (except the FRAGO 011 P6 bullet, recorded separately); the Invariants
  section; Design-Doc Alignment.
Override:  N/A — risk-neutral (a documentation/honesty correction on already-shipped,
  already-tested, working code — no behavior changes, no re-scoring of any risk row).

## FRAGO 010 — 2026-07-03 — session-id: executor-2026-07-02-m4-p4-r2 (Phase 4 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 4 (Step 4 `--no-auto-parallel` premise —
           deviation D1)
Trigger:   plan Phase 4 Step 4's premise ("under sequential lowering the authoritative analysis
           yields no cross-thread fields") and `crates/ynz-driver/src/main.rs:76`'s doc comment
           ("Forces sequential execution of all `background` tasks") both overstate what
           `--no-auto-parallel` actually does. Reality (verified independently by deviation-judge
           and code-reviewer; re-confirmed against disk this dispatch): the flag only gates the
           AUTO-PARALLEL independence-analysis pass (`emit.rs:3936` dumb-sequential baseline) and
           whatever reads its predicate (`no_auto_parallel_env()` — including this phase's padding
           analysis); the runtime never reads the env var; an explicit, user-written `background`
           spawn still runs on a real thread regardless of the flag. This does NOT weaken the
           phase's actual exit criterion (padding correctly self-gates off via the SAME flag
           predicate the padding analysis reads) and does NOT weaken the `--no-auto-parallel`
           cross-impl byte-identical guarantee (that determinism comes from the language's
           existing channel-synchronization semantics, not from thread suppression — confirmed
           end-to-end by the Phase 4 gate test,
           `integration.rs::v0_3_m4_p4_padding_gates_off_under_no_auto_parallel_with_identical_output`).
Changes:
  - `crates/ynz-driver/src/main.rs:76` doc comment: CORRECTED — now states the flag disables the
    auto-parallelization independence-analysis pass and everything gating on its predicate (e.g.
    false-sharing padding), does NOT suppress explicit `background` spawns (the runtime never
    reads it), and that cross-mode determinism comes from channel-synchronization semantics. The
    old "Forces sequential execution of all `background` tasks" claim was never true post-M1.
  - ¶3.3 Phase 4 Step 4: premise text AMENDED in place to match reality (self-gating via the
    shared `no_auto_parallel_env()` predicate), with an inline FRAGO 010 correction note; exit
    criterion unweakened.
  - ¶3.3 Phase 4 STATUS banner deviation D1: annotated classified-JUSTIFIED + applied.
Unchanged: all flag behavior and code paths (zero behavior change — documentation-accuracy only);
  the Phase 4 gate test and its assertions; the risk table (NO re-scoring); FRAGO 001-009;
  everything not listed.
Override:  N/A — risk-neutral, pure documentation-accuracy correction, zero behavior change.

## FRAGO 011 — 2026-07-03 — session-id: executor-2026-07-02-m4-p4-r2 (Phase 4 post-review follow-up; deviation-judge classified JUSTIFIED — this record APPLIES that classification, it does not re-adjudicate; D3+D4 folded together)
Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 4 (deviations D3+D4 — design-doc
           staleness, forward-scheduled to Phase 6 Step 1's already-scheduled design-doc sweep)
Trigger:   two `IMP-no-function-coloring.md` statements are stale against what Phase 3 locked and
           Phase 4 shipped. (1) `:201` says padding detection keys on "`.share`/`.lend` across
           `background`" — stale: Phase 3 made `.share`/`.lend` across `background` a HARD compile
           error (IMP-concurrency:189); detection is correctly implemented on the legal
           give/copy/channel crossing set instead. (2) `:205`'s "FFI-shaped" illustrative example
           for the unpaddable class is also stale — FFI (`foreign`) doesn't exist until v2+; the
           real v0.3 unpaddable class is cross-module-visible-layout shapes
           (exported/imported/anon-structural). `IMP-no-function-coloring.md` is deliberately NOT
           edited this round: Phase 6 Step 1 already carries the scheduled design-doc sweep
           (confirmed against plan.md's Phase 6 text); this FRAGO's job is ensuring P6's executor
           applies these two named corrections directly instead of re-deriving them.
Changes:
  - ¶3.3 Phase 6 Step 1: ADDED an explicit MUST bullet naming both corrections (not a vague
    "sweep the docs" line). The exact stale lines and their accurate replacements, for P6 to
    apply verbatim:
      (a) `IMP-no-function-coloring.md:201` — stale claim: padding detection keys on
          "`.share`/`.lend` across `background` spawn sites". Accurate replacement: detection
          keys on the LEGAL crossing set — give/copy arguments, `channel<T>` conduit element
          types, and callee return types at `background` spawn sites — because `.share`/`.lend`
          across `background` is a hard compile error as of v0.3-M4 Phase 3
          (IMP-concurrency:189); the doc's intent ("ownership crossing thread boundaries") is
          realized on the legal modifier set.
      (b) `IMP-no-function-coloring.md:205` — stale example: the unpaddable class illustrated as
          "FFI-shaped" (`#[repr(C)]`-equivalent) fields. Accurate replacement: the v0.3
          unpaddable (decline-lint) class is cross-module-visible-layout shapes — exported /
          imported / `__anon__*` structural shapes — because each module compiles to its own
          object file, so padding only the spawning module's view would fork one type's layout;
          `foreign`/FFI is a v2+ `[[deferred_language_feature]]` and cannot occur in v0.3.
  - ¶3.3 Phase 4 STATUS banner deviations D3+D4: annotated classified-JUSTIFIED + applied as
    this FRAGO (forward-scheduled), with the FRAGO 009 refinement cross-noted.
  - `IMP-no-function-coloring.md`: NO CHANGE this round (explicitly deferred to Phase 6 Step 1).
Unchanged: all code and tests (no code change this round — pure forward-scheduling note);
  the risk table; FRAGO 001-010; everything not listed.
Override:  N/A — risk-neutral, no code change this round, pure forward-scheduling note.

## Session log — executor-2026-07-02-m4-p4-r2 — 2026-07-03

Narrow post-review bookkeeping dispatch (task-spec from the conductor, routing the reviewer
fleet's Phase-4 verdicts: two deviation-judge rounds → FRAGO 009/010/011, plus 3 minor
should-fix findings deferred). Producer does NOT self-grade — every classification applied here
is the deviation-judge's / reviewers', per the conductor's routing
(`agent-charter-discipline.md`); nothing re-derived, nothing re-adjudicated. Scoped production
changes: exactly TWO text-only fixes (zero behavior change) — (a) the
`examples/pirates-roster/entrypoint.ynz` InningTally comments (~:1083, ~:1128) corrected per
FRAGO 009's bundled honesty requirement (exclusive-ownership give crossing = no concurrent
contention yet; padding illustrates the mechanism), and (b) `crates/ynz-driver/src/main.rs:76`'s
stale flag doc comment corrected per FRAGO 010. Deliverables, all landed this dispatch:
(1) FRAGO 009 (padding = documented forward-looking no-op; four-field WHAT/WHY/COST/TRIGGER in
the P4 banner + mirrored Future Requirements entry; NOT a registry deferred-feature — the code
ships real/working/tested; REFINES D3/D4).
(2) FRAGO 010 (D1: `--no-auto-parallel` premise corrected in plan Step 4 + `main.rs:76`; exit
criterion unweakened; every factual claim re-verified against disk before writing — the runtime
never reads `YNZ_NO_AUTO_PARALLEL`, `emit.rs:3936` is the only gate, grep-confirmed).
(3) FRAGO 011 (D3+D4: both IMP-no-function-coloring corrections pre-named with exact replacement
text; P6 Step 1 gained an explicit MUST bullet; the doc itself deliberately untouched — P6 owns
the sweep).
(4) Three minor findings routed to the durable per-phase deferral home
(`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md`, appended — not
overwritten) with Idempotency-Key sentinels under prefix
`2026-07-02-v0-3-m4-channels-arc-release#4:`: (a) `prefer-yielding-sleep` drops the `-when-Y`
naming-convention clause (`registry/features.toml:2306`), (b) stale cross-reference comment
naming nonexistent `crates/ynz-driver/tests/false_sharing_gating.rs`
(`false_sharing_no_auto_parallel_gate.rs:13`; real test = `integration.rs`
`v0_3_m4_p4_padding_gates_off_under_no_auto_parallel_with_identical_output`, verified at
:9930), (c) lint fires before the arity guard in `check_sleep_blocking_call`
(`crates/ynz-typeck/src/check.rs:3602`, fn verified at that exact line; lint at :3610 precedes
the `call.args.len() != 1` guard at :3630 — cosmetic noise on already-erroring code,
non-crashing). All three anchors grep-verified against the live tree before recording; no
duplicate Idempotency-Keys existed.
Verification: `cargo build --workspace` GREEN in the dev container; the pirates-roster
byte-exact golden test re-run post-comment-change and GREEN (pure comment change — stdout
verified unchanged, golden NOT regenerated because no output changed).
The pre-existing dirty `roadmap.md` + `cspell.json` untouched. NO STOP condition fired; NO
dormant override armed. Session-id appended to plan.md frontmatter; status remains active.

## FRAGO 012 — 2026-07-03 — session-id: conductor (Patrick's direct real-time scope call during
Phase 5 execution — not a build-time discovery, not deviation-judge-classified; a live human
descope, applied + logged per the risk-neutral FRAGO path, no signature required)

Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 5 Step 3
Trigger:   Patrick, live, mid-Phase-5: the VSCode extension is not being published for a while, so
           the screenshot deliverable in Step 3 ("VSCode extension version bump + screenshots
           (channels, handle-form, auto-Arc hint)") has no consumer right now and is explicitly
           descoped. This is a direct human scope decision, not a discovered divergence — no
           deviation-judge adjudication applies; it is applied verbatim and logged for the record
           per `plan-source-of-truth.md`.
Changes:
  - ¶3.3 Phase 5 Step 3: NARROWED to "VSCode extension version bump" only. Screenshot capture
    REMOVED from this phase's scope (channels/handle-form/auto-Arc hint screenshots are not
    required — no publish is scheduled).
  - ¶3.3 Phase 5 Exit criteria: "screenshots attached" REMOVED from the exit-criteria list.
  - Future Requirements / Revisit: mirrored four-field note — WHAT: VSCode extension screenshots
    (channels/handle-form/auto-Arc hint) deferred; WHY: extension has no scheduled publish date,
    so screenshots have no current consumer (Patrick's direct call, real-time); COST: capturing 3
    screenshots against a version-bumped extension, roughly one short session whenever a publish
    is actually scheduled; TRIGGER: the VSCode extension publish is actually scheduled.
Unchanged: everything else in Phase 5 (inlay hint wiring, REF-concurrency.md update including the
  backpressure teaching text + `h.send(v)` convention, demo/gallery consolidation, jargon audit,
  cross-impl sweep); the version bump itself still ships; all of Phases 0-4 and Phase 6; the risk
  table (NO re-scoring — this is a docs/asset deliverable removal, not a safety/correctness
  change).
Override:  N/A — risk-neutral (scope reduction on a non-safety, non-correctness deliverable;
  no re-scoring of any risk row; direct human authorization, applied verbatim).

## FRAGO 013 — 2026-07-03 — session-id: conductor (Phase 5 post-review follow-up; deviation-judge
classified Deviation D1 JUSTIFIED and surfaced this as a FRAGO candidate — this record APPLIES that
classification, it does not re-adjudicate)

Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 5 Steps 1-2 + ¶3.1 outcome #7 (stale text
           predating FRAGO 008)
Trigger:   Phase 5's own step text ("wire `inlay_hint.rs` for `channel_capacity` + `auto_arc`";
           "update REF-concurrency.md ... for channels + handle-form + auto-Arc") and ¶3.1 outcome
           #7 ("`channel_capacity` ... and `auto_arc` ... muted-hint domains fire") were both written
           before Phase 3's FRAGO 008 deferred the auto-Arc CODEGEN EMISSION to v0.4+ and were never
           reconciled afterward — FRAGO 008's own Changes list only touched Phase 3 text. The
           Phase-5 executor correctly declined to self-fix this (narrow-charter discipline) and
           surfaced it as Deviation D1; deviation-judge independently verified against the live tree
           (`registry/features.toml:1229-1235`'s own TRIGGER text ties `auto_arc` hint-wiring to the
           emission landing, not to P5) and classified JUSTIFIED, risk-neutral, surfacing the text
           reconciliation as a FRAGO candidate rather than grading it a Phase-5 defect.
Changes:
  - ¶3.3 Phase 5 Steps 1-2: AMENDED in place — "wire `inlay_hint.rs` for `channel_capacity` +
    `auto_arc`" → "wire `inlay_hint.rs` for `channel_capacity` (`auto_arc` wiring deferred to
    landing WITH the v0.4+ auto-Arc codegen emission per FRAGO 008 — see `registry/features.toml`'s
    `auto-arc-codegen-emission` TRIGGER text)"; the REF-concurrency.md step's "... + auto-Arc" is
    corrected to "... (auto-Arc user-facing documentation deferred alongside its codegen emission —
    FRAGO 008)".
  - ¶3.1 outcome #7: AMENDED — "`channel_capacity` ... and `auto_arc` ... muted-hint domains fire"
    → "`channel_capacity` fires (Addition category, P5); `auto_arc` is registered with its full
    WHAT/WHY teaching hover but does not yet fire — its wiring is tied to the v0.4+ auto-Arc codegen
    emission landing (FRAGO 008), not to this milestone's Phase 5."
  - Phase 5 STATUS banner: Deviation D1 annotated classified-JUSTIFIED + applied via this FRAGO.
Unchanged: all code (zero behavior change this round — text reconciliation only); FRAGO 001-012;
  Phases 0-4 and Phase 6; the risk table (NO re-scoring — a documentation-accuracy correction on an
  already-decided, already risk-neutral deferral).
Override:  N/A — risk-neutral, pure text-reconciliation, zero behavior change, deviation-judge
  pre-classified JUSTIFIED.

## Session log — executor-2026-07-02-m4-p5 — 2026-07-03

Phase 5 executed in full (all five steps, Step 3 as narrowed by FRAGO 012 mid-phase), grounded
in: the plan slice (¶2 Mission, §3.1 outcomes 7–9, §3.4, §Invariants Teaching/Demo &
Error Gallery), REF-plan-format, REF-decision-philosophy, REF-context-budget, no-duct-tape,
verification, spec-writing.md, docs-checklist.md, inference.md, vocabulary/naming, plan-invariants
§Demo & Error Gallery, FRAGOs 008/009/010/011 (read from plan.md + this sidecar) and FRAGO 012
(applied verbatim, live), plus the live crate reality (inlay_hint.rs, inlay_hint_passes.rs,
check.rs channel arms, cross_impl_consistency.rs, error_galleries.rs — recon re-read, not memory).

**Landed.**
(1) `channel_capacity` inlay hint end-to-end: NEW typeck pass
`inlay_hint_passes.rs::channel_capacity_hints` + `ChannelCapacityHint` (salsa-tracked; fires only
on `channel<T>()` with type args + zero value args; position = the closing-paren byte, proven
`span.end - 1` against parse_call's span construction) → lib.rs export → LSP Domain 10 in
`crates/ynz-lsp/src/inlay_hint.rs` (`⟨64⟩` label, registry hover, zero-width TextEdit inserting
`64`); module docs in both files updated (firing tables + auto_arc registered-not-firing note);
stale `features.toml` channel_capacity comment corrected (P1 shipped the domain only, not the
hint pass). `auto_arc` NOT wired — deviation D1, surfaced in the P5 banner, per FRAGO 008 + the
`auto-arc-codegen-emission` TRIGGER text (features.toml:1235).
(2) `docs/reference/REF-concurrency.md`: new Channels section (bounded-by-construction, default-64
hint, backpressure teaching text VERBATIM as mandated, element-type/capacity/statement-position
rules with real captured error text) + new handle-form section documenting the FRAGO 005
`h.send(v)` → first-`channel<T>`-parameter convention explicitly ("no hidden mailbox") +
`.receive()` messages-or-completion (typed errors) + channel exemption woven into the Ownership
section (was contradicting the new examples); intro/closing → "Two keywords and one type";
auto-Arc deliberately NOT documented as live (D1).
(3) VSCode extension: package.json `0.3.0-m7` → `0.3.0`; README What's-new + CHANGELOG `[0.3.0]`
(honest "Not yet firing" auto_arc note); tsc build clean (host Node v22 — `command -v` probe hit,
ran natively per the run-in-docker ladder). Screenshots: two placeholders created pre-FRAGO-012
were DELETED when the FRAGO landed; CHANGELOG carries the deferred note; plan.md Step 3 + exit
criteria + Future Requirements edited per FRAGO 012's exact wording (the conductor's explicit
instruction — the one plan-body edit authorized beyond my banner/frontmatter bookkeeping).
(4) Demo/gallery consolidation: verified entrypoint.ynz already carries all M4 surfaces (P2/P3/P4
work); golden regenerated via the script — diff confined to the KNOWN relaxed 8-pirates
nondeterministic section, reverted to pristine, byte-exact test proven GREEN against it; gallery
verified complete (16 classes + 2 documented runtime/kernel blocks).

**Verification (all in the dev container unless noted).** `cargo build --workspace` GREEN;
`cargo test -p ynz-lsp --test inlay_hint` 22/22 (3 new channel_capacity tests: fires with
position+tooltip+zero-width-edit asserted, suppressed-explicit-capacity,
suppressed-missing-element-type); real error text captured via `ynz run` on scratch fixtures
(send-wrong-type incl. backpressure note, zero-capacity, handle-no-channel-param); both spec
examples run (`runs this game: 10`, `prospect grade: 42`);
`examples_basics_runs_end_to_end` GREEN; `error_galleries` 9/9; `jargon_audit` 9/9;
`cross_impl_consistency` 2/2 (~237s full corpus, both modes) with all 30 M4 fixtures verified
inside the swept set (exclusion-substring check run explicitly); FULL workspace suite
**2199 passed / 0 failed**; `cargo clippy --workspace -- -D warnings` GREEN;
`cargo fmt --all` applied + `--check` clean (post-fmt LSP inlay tests re-run 22/22).

**Deviations SURFACED, not self-adjudicated:** D1 (P5) — Phase 5 Steps 1–2 name `auto_arc`
wiring + auto-Arc spec content; text predates FRAGO 008 and was never reconciled into this
phase's steps. Executed per FRAGO 008's applied record; step text left untouched (no FRAGO
authorizes rewriting it); full statement in the P5 STATUS banner for the deviation-judge.

**Recorded decisions (reasons in the P5 banner):** extension version `0.3.0` exact (P6 packages
at the same final version); golden noise-diff reverted (scheduler churn in the one relaxed
section, byte-exact proof kept against pristine golden); features.toml comment fix (factual,
no schema change).

Producer does NOT self-grade — reviewer fan-out (code-reviewer: jargon + spec-register + oracle
completeness) has NOT run. NO STOP condition fired; NO dormant override armed. The pre-existing
dirty `roadmap.md` + `cspell.json` untouched. Session-id appended to plan.md frontmatter; status
remains active.

## FRAGO 014 — 2026-07-03 — session-id: conductor (Phase 5 fix-round follow-up; trivial stale-text
tail the executor surfaced as new Deviation D2, not self-fixed — classified here directly: a leftover
clause contradicting an already-applied, already-signed descope has no adjudication question left to
resolve, so this record both classifies and applies rather than routing a one-clause fix through a
full deviation-judge round-trip)

Base:      2026-07-02-v0-3-m4-channels-arc-release @ ¶3.1 outcome #7 (trailing clause)
Trigger:   FRAGO 013's outcome #7 rewrite (this same file, above) replaced the domains sentence but
           did not touch outcome #7's trailing clause, which still reads "...WHAT/WHAT-INSTEAD/WHY;
           VSCode extension bumped with screenshots." — directly contradicting FRAGO 012 (Patrick's
           live descope of the screenshot deliverable, applied two rounds earlier). The Phase-5
           fix-round executor caught this itself and surfaced it as Deviation D2 rather than
           silently patching text it had just finished editing.
Changes:
  - ¶3.1 outcome #7's trailing clause: "VSCode extension bumped with screenshots." →
    "VSCode extension bumped (screenshots deferred — FRAGO 012)."
Unchanged: everything else; FRAGO 001-013; the risk table (NO re-scoring — trivial text-consistency
  fix only).
Override:  N/A — risk-neutral, one-clause text fix reconciling two already-applied, already-logged
  decisions (FRAGO 012 + FRAGO 013) against each other. No new judgment made.

## FRAGO 015 — 2026-07-03 — session-id: conductor (D3 follow-up to FRAGO 012/013/014's screenshots
reconciliation; the executor correctly declined to sweep this in beyond FRAGO 014's named scope and
surfaced it instead — same trivial reconciling-two-already-logged-decisions shape as FRAGO 014, no
new adjudication required)

Base:      2026-07-02-v0-3-m4-channels-arc-release @ Invariants "### Demo & Error Gallery"/Teaching
           bullet (plan.md:1536)
Trigger:   The screenshots-descope contradiction (FRAGO 012) survived a THIRD independent spot the
           prior two reconciliation FRAGOs (013, 014) didn't name: the Invariants section's own
           bullet "VSCode extension version-bumped with screenshots of the new surfaces." — still
           asserting screenshots ship, three FRAGOs after they were descoped.
Changes:
  - `plan.md:1536`: "VSCode extension version-bumped with screenshots of the new surfaces." →
    "VSCode extension version-bumped (screenshots deferred — FRAGO 012)."
Unchanged: everything else; FRAGO 001-014; the risk table (NO re-scoring).
Override:  N/A — risk-neutral, one-clause text fix reconciling an already-applied, already-logged
  decision (FRAGO 012) against a line the two prior sweep FRAGOs missed. No new judgment made.

## FRAGO 016 — 2026-07-03 — session-id: conductor (final review-fleet re-check on Phase 5's fix-loop
round; two independently-confirmed should-fix findings, both plain missed-spots from the same class
of sweep gap as D2/D3 — deviation-judge explicitly classified the demo-comment one UNJUSTIFIED/not-
a-FRAGO-candidate, i.e. just fix it; the Invariants line is the identical "stale text a text-only
sweep didn't reach" shape, no new adjudication needed for either)

Base:      2026-07-02-v0-3-m4-channels-arc-release @ Invariants "### Teaching" (plan.md:1528-1529)
           and `examples/pirates-roster/entrypoint.ynz:1059`
Trigger:   Full-fleet re-verification after the P5-r2 fix round found two residual staleness spots
           neither the blocker fix nor FRAGO 012-015's sweeps reached: (1) acceptance-verifier —
           Invariants → Teaching still asserts "`auto_arc` ... fire[s] via `inlay_hint.rs`",
           contradicting the already-applied FRAGO 008/013 reconciliation (auto_arc is registered,
           does not fire, wiring deferred to v0.4+); (2) rules-compliance + acceptance-verifier +
           deviation-judge (all three, independently) — the demo file's own inline comment still
           shows the pre-restyle `⟨64⟩` decorated notation, contradicting the actually-shipped plain
           `64` rendering everywhere else (REF-concurrency.md, IMP-no-function-coloring.md, the LSP
           code itself).
Changes:
  - `plan.md:1528-1529` (Invariants → Teaching bullet): "`channel_capacity` (Addition) + `auto_arc`
    (Informational, cautionary hover) fire via `inlay_hint.rs`" → "`channel_capacity` (Addition)
    fires via `inlay_hint.rs`; `auto_arc` (Informational, cautionary hover) is registered with its
    full hover text but does not yet fire — wiring deferred to the v0.4+ auto-Arc codegen emission
    per FRAGO 008/013."
  - `examples/pirates-roster/entrypoint.ynz:1059`: comment "// default capacity 64 (IDE shows
    ⟨64⟩)" → "// default capacity 64 (IDE shows 64)" — matching the actually-shipped plain-text
    label convention (no stdout/golden impact — comment only).
Unchanged: everything else; FRAGO 001-015; the risk table (NO re-scoring — both are text-only
  consistency fixes, zero behavior change).
Override:  N/A — risk-neutral, two one-clause text fixes reconciling already-applied, already-
  logged decisions (FRAGO 008/013 and the P5-r2 label restyle) against spots earlier sweeps missed.
  No new judgment made.

## Session log — executor-2026-07-02-m4-p5-r2 — 2026-07-03 (Phase 5 post-review FIX ROUND)

Applied the review fleet's Phase 5 findings on top of the previously-verified-GREEN phase,
grounded in: the P5 slice + STATUS banner, FRAGO 012/013 (this sidecar), the live tree (check.rs
channel arms, emit.rs channel lowering, inlay_hint_passes.rs, ynz-lsp inlay_hint.rs + tests,
registry features.toml — all re-read, not memory), authoritative-derivation.md, inference.md,
no-duct-tape, verification, REF-plan-format/decision-philosophy/context-budget.

**FRAGO 013 applied VERBATIM** (three edits, exact wording from its Changes list): Phase 5
Steps 1–2 auto-Arc deferral reconciliation; ¶3.1 outcome #7 channel_capacity-fires /
auto_arc-registered-not-firing rewrite; P5 banner D1 annotated classified-JUSTIFIED + applied.

**BLOCKER fixed — capacity twin-derivation** (authoritative-derivation.md, 5th recurrence of the
M3a/M3d/M3e/M3g class): `DEFAULT_CHANNEL_CAPACITY` is now the ONE authoritative
`pub const` (module-level in `crates/ynz-typeck/src/check.rs`, exported via lib.rs) threaded into:
codegen's no-arg construction default (`crates/ynz-codegen/src/emit.rs` `channel` arm — literal
`const_int(64,..)` REMOVED), all three check.rs teaching errors quoting the default (format!-
threaded), and the LSP label + click-edit (`crates/ynz-lsp/src/inlay_hint.rs` Domain 10). The
dead `let _ = DEFAULT_CHANNEL_CAPACITY;` anchor removed. Registry hover prose (TOML, cannot read
a Rust const) pinned by a new mechanical parity test
(`test_channel_capacity_registry_hover_states_the_authoritative_default`). PROOF (not eyeballed):
throwaway const=3 build — behavioral fixture backpressured after exactly `sent 3` before
`draining` (codegen moved) and the LSP fires-test FAILED its `"64"` label assert (hint moved);
const reverted to 64, fixture deleted.

**Minors fixed:** hint gated on exactly one type arg (`channel<A,B>()` suppression + new test);
LSP module doc non-firing list now carries all 4 domains incl. `allocators`; label re-styled
`⟨64⟩` → plain muted `64` per inference.md one-renderer-per-category (tests updated to the plain
label; registry `example_hint_rendered`, REF-concurrency.md:185, IMP-no-function-coloring.md:109
example, VSCode README/CHANGELOG, and plan.md's own label references swept for consistency).

**Deferred (NOT fixed — shared pre-existing gap, zero regression):** all-5-walkers
FieldAssign.target/IndexAssign.receiver recursion gap → four-field WHAT/WHY/COST/TRIGGER note
appended to the roadmap sidecar
`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/audit.md` with
`Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#5: crates-ynz-typeck-src-inlay-hint-passes-rs-1554`
(checked unique before writing).

**Deviation SURFACED, not self-fixed:** D2 — ¶3.1 outcome #7's trailing "VSCode extension bumped
with screenshots" contradicts FRAGO 012's screenshot descope; FRAGO 013's replacement covered
only the domains sentence. Flagged in the P5 banner for the deviation-judge.

**Verification (dev container):** `cargo build --workspace` GREEN; `cargo clippy --workspace
-- -D warnings` GREEN; `cargo fmt --all` applied + `--check` clean; `ynz-lsp` inlay_hint suite
24/24 (22 prior + 2 new: two-type-args suppression, registry parity); full `cargo test
--workspace` result recorded in the executor's return. Roadmap.md's own `⟨64⟩` notation (lines
325/334) left untouched — file carries pre-existing uncommitted user edits; surfaced in the
return instead.

## FRAGO 017 — 2026-07-03 — session-id: conductor (Phase 6 post-execution follow-up; deviation-judge
classified P6-D1 JUSTIFIED with an explicit FRAGO recommendation, and P6-D2 JUSTIFIED folded into the
same FRAGO per the judge's own recommendation — this record APPLIES both classifications, it does
not re-adjudicate)

Base:      2026-07-02-v0-3-m4-channels-arc-release @ Phase 6 title / Task+purpose / Step 4 / Exit
           criteria (plan.md:1408-1432)
Trigger:   The plan's own premise, carried unchanged from the original draft through r2/r3/r4
           (¶1 Situation Assumption 12, explicitly marked `unverified — verify at P6`), claimed "M3f
           and M3g are merged to main, un-tagged, no CHANGELOG entries." The Phase-6 executor's R4
           span verification (Step 3) checked this against reality and found it HALF stale:
           M3f's merge commit `51c948b` IS an ancestor of tag `v0.3.0-m6` (`git merge-base
           --is-ancestor`), and `CHANGELOG.md`'s `[0.3.0-m6]` section already covers it — M3f is NOT
           un-tagged and must NOT be re-covered by the pending `v0.3.0` CHANGELOG section (P6-D1).
           Separately, the naive `v0.3.0-m7..HEAD` span also contains the entire M3d implementation
           (which merged to main after the `m7` tag was cut, so its commits are technically inside
           the span) even though `CHANGELOG.md`'s `[0.3.0-m7]` section already describes M3d — a
           naive commit-range CHANGELOG generator would double-cover it (P6-D2). Both independently
           verified by deviation-judge against `CHANGELOG.md`'s actual sections. Deviation-judge
           classified P6-D1 JUSTIFIED with an explicit FRAGO recommendation (the exit criterion is
           now an affirmatively wrong instruction sitting in the seam Step 4 will read) and
           recommended folding P6-D2's caution into the same FRAGO so a fresh `/release` session
           sees both hazards in Step 4's own text, not only the STATUS banner.
Changes:
  - `plan.md:1408` (Phase 6 title): "v0.3.0 release fold (M3f + M3g + M4)" → "v0.3.0 release fold
    (M3g + M4; M3f already released at `v0.3.0-m6`)".
  - `plan.md:1409` (Task + purpose): "Cut the final `v0.3.0` tag folding the un-tagged M3f + M3g
    work." → "Cut the final `v0.3.0` tag folding the un-tagged M3g work. (M3f is NOT un-tagged — it
    shipped at `v0.3.0-m6`; see FRAGO 017.)"
  - `plan.md:1429-1430` (Step 4): "`/release` cuts `v0.3.0` (final, NO `-mN` suffix); VSCode `.vsix`
    assets per convention (`yinz-{version}.vsix` + `yinz-latest.vsix --clobber`)." → "`/release` cuts
    `v0.3.0` (final, NO `-mN` suffix). CHANGELOG generation MUST cover M3g + M4 ONLY — M3f is already
    covered at `[0.3.0-m6]` and must NOT be re-covered (FRAGO 017/P6-D1); M3d is already covered at
    `[0.3.0-m7]` despite its commits falling inside the naive `m7..HEAD` span (a commit-range
    generator would double-cover it — FRAGO 017/P6-D2, exclude already-CHANGELOGed M3d content).
    VSCode `.vsix` assets per convention (`yinz-{version}.vsix` + `yinz-latest.vsix --clobber`)."
  - `plan.md:1431-1432` (Exit criteria): "CHANGELOG demonstrably spans M3f + M3g + M4" →
    "CHANGELOG demonstrably spans M3g + M4 (M3f already covered at `v0.3.0-m6`, NOT re-covered; M3d
    already covered at `v0.3.0-m7`, NOT re-covered despite falling inside the naive commit span)".
  - Phase 6 STATUS banner: P6-D1 and P6-D2 annotated classified-JUSTIFIED + applied via FRAGO 017.
Unchanged: everything else; FRAGO 001-016; Phases 0-5 (already sealed); the risk table (NO
  re-scoring — a stale-premise correction to the release-fold's own CHANGELOG scope, not a
  safety/correctness change to shipped code).
Override:  N/A — risk-neutral, plan-text corrections reconciling Phase 6's own premise against
  verified git/CHANGELOG reality; zero code change; deviation-judge pre-classified both JUSTIFIED.

## Session log — executor-2026-07-02-m4-p6 — 2026-07-03

Phase 6 STEPS 1–3 ONLY (dispatch-scoped: Step 4 — `/release` tag cut + `.vsix` upload — is
explicitly OUT of this dispatch, human-gated on Patrick; nothing tagged, pushed, or published).
Grounded in: the Phase 6 slice + ¶2/¶3.1 + R4, REF-plan-format, FRAGO 011's exact replacement
text (applied verbatim, not re-derived), FRAGO 008/009 records, the live design docs +
crate reality (recon re-read, not memory).

**Landed.** (1) Design-doc shipped-status sweep: `IMP-no-function-coloring.md` — Channel section
SHIPPED v0.3-M4 milestone note (+ default capacity locked note: 64, `DEFAULT_CHANNEL_CAPACITY`,
verified live at `crates/ynz-typeck/src/check.rs:37`); FRAGO 011 (a)+(b) applied at the
False-Sharing section (legal crossing set; cross-module-visible-layout decline class — both
stale spots verified still-live on disk before editing); False-Sharing milestone line SHIPPED
v0.3-M4 + FRAGO 009 forward-looking-no-op honesty note; sleep-lint row SHIPPED v0.3-M4.
`IMP-concurrency.md` `ECWrapperResultCollection` SHIPPED v0.3-M4 (verified against
`crates/ynz-runtime/src/handle.rs` "R8 — copy-before-free, compile-time spawn-form-keyed" and
the retired `ec-wrapper-collect-on-completion` note at `registry/features.toml:1164`; the
section's "conditional on whether the handle is collected" prediction corrected to the shipped
compile-time spawn-form-keyed mechanism). (2) `Cargo.toml:21` `0.3.0-m7` → `0.3.0`; full
`cargo build --workspace` GREEN in the dev container (all 14 crates at v0.3.0; no other live
`-m7` reference — CHANGELOG/state.md/VSCode-README hits are historical sections).
(3) R4 span verification with real command output: `git describe --tags --abbrev=0` =
`v0.3.0-m7` (most recent tag); `git log --oneline v0.3.0-m7..HEAD` = 61 commits including all
of M4 (`d93f4c8`…`372927f`) and M3g (`87a63b7` et al.; M3g absent from CHANGELOG — confirmed by
grep). CHANGELOG generation itself NOT performed (Step 4 / `/release` owns it).

**Deviations SURFACED, not self-adjudicated (full text in the P6 STATUS banner):**
P6-D1 — "M3f merged un-tagged, no CHANGELOG entries" is stale: M3f (merge `51c948b`) IS an
ancestor of tag `v0.3.0-m6` and CHANGELOG `[0.3.0-m6]` already covers it; the m7..HEAD span
correctly contains zero M3f commits; v0.3.0's CHANGELOG must not re-cover M3f. P6-D2 — the span
contains the whole M3d implementation by ancestry (M3d branch merged after the m7 tag) though
`[0.3.0-m7]` already describes M3d; naive commit-list generation would double-cover it. Both
routed to the deviation-judge + Step 4's human-gated `/release`.

**Plan-structure repair (surfaced + fixed):** the `#### Phase 6 — v0.3.0 release fold
(M3f + M3g + M4)` slice-anchor heading was accidentally deleted by P5 commit `372927f`
(verified `git log -S '#### Phase 6'`); restored byte-identical from the pre-deletion revision
(`d93f4c8:plan.md:734`) so the phase has its anchor and the P6 banner a home. Structural
restoration of an accidental deletion only — no content judgment; flagged for the
deviation-judge alongside P6-D1/D2.

Plan↔task sync: no TodoWrite tool in this dispatch's grant — sync is the P6 STATUS banner +
this entry (the phase's Steps are numbered lines, not checkbox glyphs, per this plan's
convention). Pre-existing dirty `roadmap.md` + `cspell.json` untouched. NO STOP condition
fired; NO dormant override armed. Session-id `executor-2026-07-02-m4-p6` appended; status
remains active (phase open pending Step 4 + reviewer fan-out + Patrick sign-off).

## Session log — executor-2026-07-02-m4-p6 (FRAGO 017 application) — 2026-07-03

Coordinator routed the deviation-judge's verdicts back: P6-D1 JUSTIFIED (explicit FRAGO required —
the exit criterion was an affirmatively WRONG instruction in the seam Step 4 reads, not a mere
footnote), P6-D2 JUSTIFIED folded into the same FRAGO, P6-D3 (Phase 6 heading restoration)
JUSTIFIED with NO FRAGO (pure mechanical repair, correct as left). Applied FRAGO 017's four Changes
verbatim — NOT re-adjudicated (deviation-judge pre-classified; this dispatch only applies): (1)
Phase 6 title `plan.md:1408` "(M3f + M3g + M4)" → "(M3g + M4; M3f already released at `v0.3.0-m6`)";
(2) Task+purpose "un-tagged M3f + M3g" → "un-tagged M3g" + M3f-shipped-at-m6 note; (3) Step 4 text
gained the M3f-exclusion (already at `[0.3.0-m6]`) AND M3d-exclusion (already at `[0.3.0-m7]`,
double-cover hazard) instructions directly, so a fresh `/release` session sees both hazards in the
step itself, not only the banner; (4) Exit criteria "spans M3f + M3g + M4" → "spans M3g + M4 (M3f
covered at m6, NOT re-covered; M3d covered at m7, NOT re-covered)". P6-D1/D2 banner blocks annotated
classified-JUSTIFIED + applied via FRAGO 017; P6-D3 annotated JUSTIFIED-no-FRAGO. Pure plan-text,
zero code change — no rebuild/retest. Session-id `executor-2026-07-02-m4-p6` already current in the
frontmatter chain (same dispatch). Status remains active (Phase 6 open pending Step 4 + Patrick
sign-off).

**Phase 6 Step 4 + release, completed 2026-07-03 (conductor-executed, Patrick's explicit go):**
`/release` cut `v0.3.0` per the corrected FRAGO-017 CHANGELOG scope (M3g + M4 only — M3f/M3d
correctly excluded). Commits: `f6b8306` (Phase 6 boundary, #6) → `16877d9` (chore: pre-existing
roadmap M4/M5-split + cspell dictionary sync, committed properly per Patrick's "no duct tape" call
rather than stashed) → `8f33d29` (CHANGELOG). Tag `v0.3.0` (annotated) pushed; GitHub release
published at https://github.com/yinzers/yinz-lang/releases/tag/v0.3.0 with notes generated verbatim
from the CHANGELOG entry; `yinz-0.3.0.vsix` + `yinz-latest.vsix` uploaded per this project's
VSCode-release convention. Rollout mode: PLAIN (no feature-flag usage in the project source — the
only hits were `node_modules` third-party noise). Phase 6 exit criteria fully MET. Phase 6 marked
DONE. All phases 0-6 of this plan are now sealed.

## Completion-Gate coupling decision — 2026-07-03 — session-id: conductor

**Heuristic (§9.0.1): RUN, not skip.** Seven phases (0-6), declared touched-surfaces overlap
heavily and are NOT pairwise disjoint — `crates/ynz-typeck/src/check.rs`, `crates/ynz-codegen/src/emit.rs`,
`registry/features.toml`, `docs/internal/implementation/IMP-no-function-coloring.md`, and
`examples/pirates-roster/entrypoint.ynz` are each touched by 3+ phases (P0/P1/P2/P3/P4 all touch
`check.rs`; P1/P2/P3/P4 touch `emit.rs`; P1/P3/P4/P5 touch `registry/features.toml`). Fail-safe
default-to-run (R2) applies regardless — this is not a borderline call.

**Range**: `d81df91..f6b8306` (parent of Phase 0's boundary commit `d93f4c8` → Phase 6's boundary
commit `f6b8306` — the last phase boundary; no `#fix` commit exists yet, first entry). 9 commits.
Fanning out the three cross-phase lenses (code-reviewer with the range, acceptance-verifier +
deviation-judge with the assembled whole-plan scope) now.

**Fan-out results — 0 blockers across all three lenses:**
- **code-reviewer** (reuse/consolidation-only lens): 1 minor — `crates/ynz-runtime/src/handle.rs`'s
  `lock_or_recover<T>` is a byte-identical copy of P1's `crates/ynz-runtime/src/channel.rs:97`
  helper instead of importing it. Everything else cross-phase (suspension classification, conduit-
  binding origin, `DEFAULT_CHANNEL_CAPACITY`, the padded-shapes set, frame-ABI offsets, the
  `[[lint_rule]]` mechanism) correctly threads ONE authoritative source across phases — the exact
  discipline this repo's `authoritative-derivation.md` demands, held.
- **acceptance-verifier** (§3.1-integrated-whole + campaign-slice, BOTH targets): MET — all ten Key
  Outcomes + Definition of Done verified against real artifacts; two outcomes (auto-Arc emission,
  padding's current no-op status) correctly split MET/DEFERRED via FRAGO 008/009, not silent gaps.
  3 should-fix: (a) the roadmap's own M4 status/ledger rows are stale ("NOT YET PLANNED"/
  "NEEDS-PLANNED") now that M4 shipped as `v0.3.0` — pre-acknowledged in this plan's own Future
  Requirements as a deferred roadmap-maintenance item, but live staleness today; (b) the roadmap
  ledger's plain-reading text overstates auto-Arc/padding as fully-delivered capabilities without
  pointing at FRAGO 008/009's honest deferral; (c) the tag-push/GitHub-release facts were
  unverifiable from that dispatch's tool-restricted scope (no Bash/network) — CONDUCTOR CONFIRMS
  BOTH DIRECTLY: `git push origin v0.3.0` succeeded (`* [new tag] v0.3.0 -> v0.3.0`), the GitHub
  release was created (`gh release create` returned
  https://github.com/yinzers/yinz-lang/releases/tag/v0.3.0), and both `.vsix` assets are confirmed
  attached (`gh release view --json assets`). (c) is closed, no fix needed; (a)/(b) are real and
  worth closing now rather than leaving parked.
- **deviation-judge** (cross-phase-interaction only): 1 should-fix, UNJUSTIFIED-as-stray (not a
  FRAGO candidate — no adjudication needed, just a plain sweep miss) —
  `docs/internal/implementation/IMP-no-function-coloring.md`'s "Task Cancellation — Locked
  Pre-v0.2" section still claims the runtime "injects a cancellation signal at the task's next
  suspension point" on handle drop; P2's own shipped reality (its Future Requirements deferral,
  code-confirmed — zero `ynz_handle_free` call sites in `emit.rs`) is that NO scope-drop
  cancellation mechanism exists at all (fire-and-forget-to-completion, by design, until a future
  language-wide scope-drop mechanism ships). `plan.md`'s own §3.1 recorded decision and Design-Doc
  Alignment #9 still carry the same narrower, now-stale framing. This is the identical
  "text-reconciliation sweep gap" class FRAGO 013-016 already fixed elsewhere in this same plan —
  P6's design-doc sweep touched the sibling Channel/False-Sharing/Sleep sections but missed this
  one.

**Routing: all four items are cheap, well-diagnosed, direct fixes — fixing now rather than
deferring** (per this plan's established pattern all session: small, safe, well-understood findings
get fixed on the spot; only genuine scope-creep gets a durable four-field deferral). None require a
FRAGO (no justified divergence being adjudicated — these are plain corrections of stale text /
duplicated code, the same class already fixed directly via FRAGO 014-016's mechanism, or in this
case simple enough to just fix without even a FRAGO wrapper). Dispatching the fix round now.

## Session log — executor-2026-07-03-m4-gate-fix — 2026-07-03

Completion-gate cumulative fix round (post-completion cleanup, NOT a new phase — no `#### Phase`
heading added). All four items from the Fan-out results above fixed directly:

**Fix 1 — `lock_or_recover` dedup (code-reviewer minor).** `crates/ynz-runtime/src/channel.rs:98`'s
helper promoted to `pub(crate)`; `crates/ynz-runtime/src/handle.rs` now imports it via the existing
`use crate::channel::{...}` line and its byte-identical local copy (old handle.rs:90-93) is deleted
(the then-unused `MutexGuard` import removed alongside — `-D warnings` would have failed on it).
ONE definition remains, in the module that originated the discipline (P1). Verified in the dev
container: `cargo build --workspace` GREEN, `cargo clippy --workspace -- -D warnings` GREEN,
`cargo test -p ynz-runtime` all suites pass, full `cargo test --workspace` result recorded in the
executor's return.

**Fix 2 — roadmap M4 staleness (acceptance-verifier a+b).**
`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md`: (1) Milestone 4's
"Execution plan" status line "NOT YET PLANNED" → DONE/SHIPPED as `v0.3.0` (tagged + published
2026-07-03) with a link to this plan; (2) both Capability Ledger rows for
`v0-3-m4-channels-arc-release` ("planned" at the Capability Ledger table; "NEEDS-PLANNED. No child
plan yet." at the ownership-map table) → **shipped**/**DONE**, each carrying the FRAGO 008/009
deferral pointer so the ledger's plain reading no longer overstates auto-Arc codegen emission
(deferred v0.4+, `auto-arc-codegen-emission`) or the padding transform's current throughput benefit
(forward-looking no-op until auto-Arc emission) as fully live. Incidental factual correction in the
same rows: "folds M3f + M3g tags" → "folds M3g tag; M3f already shipped at `v0.3.0-m6`", matching
FRAGO 017's verified reality rather than repeating the stale premise.

**Fix 3 — stale Task Cancellation claim (deviation-judge, plain sweep miss — no adjudication).**
Premise re-verified before editing (not asserted): `grep -rn ynz_handle_free crates/ynz-codegen/src/`
→ hits ONLY in `runtime_decls.rs` (:103-104 declaration comment + field, :492-494 declare_fn); ZERO
call sites in `emit.rs` or anywhere else — codegen never emits the call, so a dropped handle's task
runs to completion (fire-and-forget), never silently killed.
`docs/internal/implementation/IMP-no-function-coloring.md` "Task Cancellation — Locked Pre-v0.2":
model paragraph reframed as "The locked end-state model", user-facing model marked "(end-state)",
and an **Implementation milestone: SHIPPED-DEFERRED v0.3-M4** status line added mirroring the
sibling Channel/False-Sharing/Sleep sections' P6 treatment — runtime half live + substrate-proven
(safe-drop, alloc=free-gated), language half (scope-drop-triggered `ynz_handle_free` emission,
child-side typed-`errors` cancellation, `.cancel()` API) NOT implemented, deferred per the
four-field Future Requirements entry (registry `background-handle-cancel-injection`; trigger:
language-wide scope-drop mechanism OR a real cancellation workload). The two plan.md spots carrying
the same stale hypothetical framing corrected to state plainly cancel-injection did NOT ship: §3.1
recorded decision ("Handle-drop semantics", ~plan.md:412) and Design-Doc Alignment #9
(~plan.md:1700), both now pointing at the Future Requirements deferral.

Bookkeeping: session-id `executor-2026-07-03-m4-gate-fix` appended to plan.md's frontmatter chain;
no phase heading added; no FRAGO filed (per the routing decision above — plain corrections, no
justified divergence adjudicated).
