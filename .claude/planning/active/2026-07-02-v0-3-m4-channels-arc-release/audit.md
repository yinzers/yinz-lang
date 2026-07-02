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
