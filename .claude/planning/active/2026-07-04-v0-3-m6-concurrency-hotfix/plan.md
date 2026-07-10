---
name: "v0-3-m6-concurrency-hotfix"
plan-id: "2026-07-04-v0-3-m6-concurrency-hotfix"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-producer-2026-07-04-m6", "plan-producer-2026-07-04-m6-amend1", "plan-producer-2026-07-04-m6-amend2", "plan-producer-2026-07-04-m6-amend3", "conductor-2026-07-09-m6-exec", "executor-2026-07-09-m6-phase0", "executor-2026-07-09-m6-phase0b-frago", "executor-2026-07-09-m6-phase1", "executor-2026-07-09-m6-phase1-seg2", "executor-2026-07-09-m6-phase1-seg3", "executor-2026-07-09-m6-phase1-seg4", "executor-2026-07-09-m6-frago004"]
created_at: "2026-07-04"
updated_at: "2026-07-09"
metadata:
  type: "plan"
---

# PLAN: v0.3-M6 — Concurrency Hotfix

> **Status note.** The dispatching brief requested `status: draft`. [REF-plan-format](../../../../docs/reference/REF-plan-format.md)'s
> frozen lifecycle enum is `stub | active | paused | done | superseded | abandoned` — `draft` is not
> a legal value (the status-folder hook keys off this exact enum). This is a complete OPORD body
> under `status: "stub"` instead — the same pre-conductor-approval convention M5
> (`2026-07-03-v0-3-m5-auto-soa`) used: the execution conductor flips `stub → active` at the approval
> gate (same file, same plan-id) once Mission/Intent/phases are confirmed real (they are, below).
> Recorded here rather than silently substituted — flagged again in the return to the orchestrator.
>
> **Execution gate: no phase starts until the v0.3-M5 auto-SoA merge is complete AND its tag is
> cut** (Patrick-signed sequencing decision, handled elsewhere). Branching M6 from `main` only after
> that merge is what kills the `emit.rs` collision risk (risk row R8 below) — starting early would
> reopen it. Plan now, execute later, per the same convention M5 used.

## 1. Situation

### Terrain (landscape) — recon 2026-07-04, grounded in `.claude/audits/2026-07-04-concurrency-release-audit.md` (Fable-verified) + direct file:line re-reads this session

- **P1-1 — the authoritative UFCS-suspends resolution already exists and is exactly what the 4 broken
  predicate sites must thread.** Direct read this session: `crates/ynz-typeck/src/check.rs`
  `check_method_call`'s `Type::Shape { name }` UFCS arm (:4357-4394) resolves the callee via
  `self.sig_table.fns.get(method)` (:4358), matches the first parameter's type against the receiver
  (:4360-4361), and reads `sig.suspends` for the kernel-mode guard (:4384: `if self.kernel_mode &&
  sig.suspends`). **This lookup — `sig_table.fns.get(method)` keyed off the receiver-matched
  first-param signature — is the ONE authoritative source.** The four broken sites never call it:
  `crates/ynz-typeck/src/may_block.rs:1296-1318` (`collect_calls_in_expr` MethodCall arm — no
  call-graph edge for `method`), `crates/ynz-typeck/src/cpu_admission.rs:823-828`
  (`expr_contains_suspending_call` — same gap), `crates/ynz-codegen/src/emit.rs:653-658`
  (`collect_callees_in_expr` — same gap, so frame layouts never embed the UFCS callee's sub-frame),
  `crates/ynz-codegen/src/emit.rs:8433-8440` (`is_direct_suspending_call` — only matches
  `Expr::Call`+`Ident`, so any `MethodCall` returns `false` and `wait x.suspendingFn()` falls through
  to the no-op `wait` arm and lowers as a synchronous call). Per
  [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md): thread this ONE
  lookup into all 4 sites — never re-derive a second method→fn mapping.
- **Failure mode when it falls through**: the synchronous wrapper drives the suspending callee via
  `ynz_rt_run_entrypoint` → `Handle::block_on` (`runtime.rs:966-1007`). That driver's own doc comment
  (`runtime.rs:921-925`) declares itself unreachable from inside an SM resume fn on a Tokio worker
  thread — UFCS breaks that invariant today. Exact mode (block vs panic) is pinned by a repro fixture
  in Phase 1 rather than assumed.
- **P4-3 — the escape hatch P1-1 routes traffic onto.** `emit.rs:15122-15137` retains an UNASSERTED
  synchronous `ynz_rt_run_entrypoint` fallback for any non-SM-classified caller reaching a suspending
  call. Its sibling recursive-path HARD-ERRORS at `emit.rs:11162` — this one silently emits instead.
  Directly coupled to P1-1: the UFCS misclassification is exactly what reaches this unguarded branch
  today; fixing P1-1 alone doesn't close the branch, it just stops (most) traffic from finding it.
- **P3-1/P2-2 — `caller_token` ABA + orphaned `pending_sends`.** `emit.rs:11651-11654` computes the
  token as a raw frame-pointer-to-int (`ptr_to_int`), no generation salt. `channel.rs:120` declares
  `pending_sends`; `channel.rs:270` inserts an entry keyed by that raw token when a sender suspends on
  a full channel. The drop ladder (`runtime.rs:591-693`, kind-2 `BgArgDropEntry`) only calls
  `ynz_channel_free` (a refcount decrement) on cancellation — it never purges the `pending_sends`
  entry. Allocator reuse of the freed frame address resurrects the dead task's stale entry under a
  new task's identity: the new task's `send(v2)` matches the stale slot, `v2` is silently discarded,
  the dead task's `v1` delivers instead. Routine trigger conditions (backpressure + cancellation +
  address reuse), confirmed reachable today.
  **Second token producer (Fable's personal plan-audit addendum,
  `.claude/audits/2026-07-04-concurrency-release-audit.md` "P3-1 ADDENDUM"), verified this session by
  direct read:** `handle.rs:326`'s `ynz_handle_send_poll` passes `handle_ptr as u64` as the
  `caller_token` for `h.send()` — and this lands in the exact SAME `pending_sends`
  `HashMap<u64, PendingSend>` on the shared `YnzChannel` (confirmed: `ynz_handle_send_poll` delegates
  straight into `ynz_channel_send_poll` on `handle.msg_chan`, `channel.rs:109-123`/`218-250`), not a
  separate map. `ynz_handle_free` (`handle.rs:337-351`) drops the handle and calls
  `ynz_channel_free(handle.msg_chan)` but never purges the handle-keyed `pending_sends` entry — a
  recycled handle heap address inherits the dead handle's suspended send, the identical ABA/orphan
  shape as the frame-pointer path. Any P3-1 fix must therefore enumerate BOTH producers (frame-ptr
  conduit tokens minted at `emit.rs:11651-11654`; handle-ptr tokens minted inside
  `ynz_handle_send_poll` at `handle.rs:326`) and purge at BOTH cancellation paths (the drop ladder's
  kind-2 `BgArgDropEntry` for frame tokens; inside `ynz_handle_free` — which already holds
  `msg_chan` — for handle tokens), per
  [authoritative-derivation.md](../../../rules/authoritative-derivation.md): ONE token-minting +
  purge scheme threaded to both producers, never two ad hoc schemes.
- **P3-2 — lost-wakeup window.** `channel.rs:311-339` (`ynz_channel_recv_poll`): `poll_recv`
  (registers with Tokio's single-slot waker) and `record_recv_waiter` are two separate critical
  sections. `channel.rs:331`'s Ready-path wakes every recorded waiter, which narrows (not
  eliminates) the window: a permanent hang needs consumer A's registration gap to straddle the
  channel's FINAL send.
- **P2-4/P2-1/P2-2 (refinement) — buffered-channel heap-element leak, and why P2-3 stays dead code.**
  `YnzChannel` (`channel.rs:109-123`) has no `Drop` impl; the buffer holds type-erased `i64` bit
  patterns with zero per-element drop-glue anywhere in the runtime. `channel.rs:109-123` also shows
  the object holds BOTH mpsc endpoints for its entire life — bare channels never close in production
  (tests only simulate closure via `std::mem::replace`, `channel.rs:536-539`/`557-560`). The
  closed-recv codegen arm at `emit.rs:~11834-11841` carries its own comment: "Structurally
  unreachable in v0.3-M4 (the channel object holds a sender)" and aborts loudly rather than hangs —
  so P2-1 is a KNOWN M4 design state, not a latent surprise, and P2-3's closed-send leak
  (`emit.rs:~11833-11960` closed1/closed2 blocks dropping no `value_bits`) is genuinely unreachable
  dead code until channel-close semantics ship. P2-4 (buffered elements at channel DROP, not at a
  closed-send) IS reachable today regardless of P2-1/P2-3 and is this milestone's scope.
- **P3-3 — shutdown mutex over-scoped.** `runtime.rs:316-354`'s `ynz_rt_shutdown` holds the RUNTIME
  mutex across the up-to-5s `shutdown_timeout` drain (the "lock drops here" comment is fiction — the
  lock is function-scoped). The correct pattern already exists in the same file:
  `ynz_rt_run_entrypoint:995-1006` extracts the owned Runtime and drains outside the lock.
- **`ynz run` signal masking.** `crates/ynz-driver/src/run.rs:75`:
  `status.code().unwrap_or(1)` converts a signal-terminated child (SIGSEGV et al.) into a
  diagnostic-free exit code 1. Surfaced by the M5 spike's own O0→optimizer investigation, where it
  actively hindered debugging.
- **P4-1 — preemption is 100% theater, doc still states it as locked+shipped.**
  `docs/internal/implementation/IMP-no-function-coloring.md:214-216` locks "check points at function
  call sites AND loop back-edges." Reality: `emit.rs:12356-12365` emits back-edge calls to
  `ynz_rt_check_preempt`, a documented no-op stub (`runtime.rs:281-299`, specifically the no-op body
  at :296-299). Net runtime preemption today: zero. The relaxation was pre-authorized by the roadmap
  (1190% O0 call-site cost per the M5 spike bench) but never written back to the design doc, and
  there is no `[[deferred_tooling_feature]]` registry entry — the exact undocumented-deferral shape
  `no-duct-tape.md` bans.
- **P4-2 — `background.cpuBound` specified, absent, undeferred.**
  `docs/internal/implementation/IMP-no-function-coloring.md:247` names `background.cpuBound process(data)` as
  the explicit force-the-other-pick override for CPU-bound auto-inference (per
  [`auto-promotion.md`](../../../rules/auto-promotion.md)'s override-direction checklist) — "final
  naming TBD." It was never implemented and carries no registry entry.
- **P4-4 — doc staleness, mixed verdicts.** FFI/`foreign` text reads present-tense but `foreign` is a
  registered v2+ deferral (registry-confirmed). `KernelModeRejectsWait` doc text says unshipped, but
  `check.rs:2441-2449`-region kernel-mode-suspension rejection is already implemented (confirmed live
  in the UFCS arm read above, `check.rs:4384-4392`, and the bare-call arm at `check.rs:2930-2937`).
  Auto-Arc (P2-6) needs **no action** — it is already correctly registry-deferred to v0.4+, and the
  registry entry self-diagnoses that `IMP-ownership.md` lacks the Arc sharing topology spec (audit
  P4-4 confirms; verified this session via the `auto-arc-cautionary-tint` /
  `auto-arc-codegen-emission` registry entries already in `registry/features.toml`).
- **`IMP-concurrency.md`'s "Design Divergences" section (:840 onward) is a live, well-established
  pattern** (four existing entries, each stating what ships / named cost / reversal path) — the
  correct home for the new bare-channel-non-closure entry (Phase 7), confirmed by direct read this
  session (`docs/internal/implementation/IMP-concurrency.md:838-876`).
- **Dynamic-dispatch × suspension coverage — newly-flagged open question (this amendment, session
  `plan-producer-2026-07-04-m6-amend3`), not yet verified.** The audit's correctness sweep exercised
  the UFCS (`value.method()`) call-form suspension-invisibility gap (P1-1) but never checked the
  sibling `dynamic Contract` vtable-dispatch call form for the identical shape of gap: a
  vtable-dispatched method whose resolved impl suspends may be equally invisible to
  `may_block.rs`'s call-graph builder, `cpu_admission.rs`, and `emit.rs`'s suspension predicates —
  and it is not yet confirmed whether typeck even permits a suspending function to satisfy a
  `follows` contract signature in the first place (if it can't, the gap is moot by construction). Not
  fixed blind: Phase 0 now verifies this (COVERED or GAP) before any assumption is made; a GAP verdict
  routes the fix into Phase 1's scope via the plan-amendment + FRAGO seam, since it shares the exact
  same authoritative-resolution threading Phase 1 already builds for the 4 UFCS predicate sites.
- **P2-7 — newly surfaced this session, NOT in the brief's enumerated scope.**
  `crates/ynz-runtime/src/handle.rs:297-303`'s `ynz_handle_recv_poll` panic path returns `Pending`
  with a possibly-unregistered waker — if the panic fires before waker registration the task may
  never wake (a hang, not a crash). Per the risk-engine's union rule (a risk set is the union of
  user-named and agent-found risks; the brief's "none beyond these 9 items" never suppresses a hazard
  this producer finds), this is recorded as a Future Requirement below rather than silently dropped
  OR silently folded into scope the brief never asked for.
- **Roadmap + Capability Ledger format confirmed by direct read**: `### Milestone N` sections follow a
  fixed shape (Value delivered / Execution plan / Depends on / Scope / Trigger to schedule / Ships
  via); the Capability Ledger appears as **two** `## Capability Ledger` sections in the current
  roadmap file (lines 365 and 417 — a pre-existing migration-era duplication, not this plan's defect
  to fix; Phase 8 adds M6's row to both so neither goes stale).
- **No `<project>/.claude/risk-anchors.md` override exists** (glob-confirmed) — this plan scores
  against [REF-risk-engine.md](../../../../docs/reference/REF-risk-engine.md)'s default code-domain
  anchor sheet.

### Weather (external constraints)

- **Hotfix cadence, no hard date.** Patrick explicitly accepted ship-now-fix-after for v0.3.0; this
  milestone's job is closing the gap, not racing a deadline.
- **Zero public users, pre-v1.0** — full breaking-ABI latitude per `ADR-versioning`; every change here
  is git-reversible (no Floor-A/Floor-B "no backout" condition anywhere in this milestone).
- **Execution gated on the v0.3-M5 merge + tag** (see the status-note banner above) — this is what
  keeps the `emit.rs` collision risk LOW rather than live.
- **M3c shadow-parity hotfix is explicitly OUT of this milestone** (Patrick-signed, do not reopen) —
  it is its own separate roadmap-tracked hotfix (`v0-3-m3c-shadow-parity`).
- **M7 (optimizer pipeline) is a sibling plan authored in parallel** — this milestone must not
  scope-creep into it. Every citation to "the optimizer" in this plan is a Future-Requirements pointer,
  never a task this plan executes.
- **All cargo/build commands run in Docker** (`docker compose run --rm dev ...`, no `-it`) per the
  project's `run-in-docker` convention — the host has no native `cargo`.

### Friendly forces

- **Higher intent**: roadmap
  [`2026-05-21-v0-3-concurrency-perf`](../2026-05-21-v0-3-concurrency-perf/roadmap.md). M6 is a
  hotfix against the already-tagged `v0.3.0` (+ pending `v0.3.x` M5 patch), not a new roadmap
  milestone in the original vision — it exists because the audit found real, confirmed bugs.
  This plan adds itself to the roadmap's milestone list + Capability Ledger in Phase 8 (Design-Doc
  Alignment item 3 below).
- **The concurrency-release audit** (`.claude/audits/2026-07-04-concurrency-release-audit.md`) is this
  plan's evidence base — every fix item traces to a CONFIRMED or CONFIRMED-BY-MECHANISM finding,
  Fable-verified against the live tree. THEORY/CLAIMED findings (P1-2, P2-5) are not fixed blind —
  Phase 0 verifies them first.
- **M5's authoritative-derivation discipline** (four silent-miscompile incidents across M3a/M3d/
  M3e/M3g, the write-time-guard capability-ledger row) is the direct precedent this plan's P1-1 and
  P2-4 fixes must not repeat — one authoritative source, threaded, never re-derived.
- **M5's fixture/gate house style** (RED-before-fix, `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity,
  non-vacuous-coverage discipline per M5's FRAGO-005 lesson) is reused directly rather than
  reinvented.

### Assumptions

| # | Assumption | Status |
|---|---|---|
| A1 | v0.3-M5 auto-SoA is merged to `main` and tagged before any M6 phase executes | **verified 2026-07-09 (Phase 0)** — `97e32f6` is an ancestor of `main`; tag `v0.3.0-m5` → `f5495c9` |
| A2 | The audit's file:line citations are accurate as of 2026-07-04; they may drift by execution time | **verified as of this session** (direct re-reads above); **re-verify per-phase at dispatch** (CCIR-1, ¶3.4) |
| A3 | P1-2's twin type-walkers (`emit.rs:8276` unsubstituted vs `emit.rs:8364` generic-substituted) are dormant because both frame-layout call sites filter `generics.is_empty()` | **verified 2026-07-09 (Phase 0): DORMANT** — SM classification gated `generics.is_empty()` (`emit.rs:252/308/1228/1277`); `lower_generic_function` uses empty suspend machinery + `sm_frame_ptr: None` (`emit.rs:1490-1496`); generics cannot suspend (`check.rs:4005-4020`). Field is named `Cg.type_subst`, not `type_params` |
| A4 | P2-5's recursion-chain cleanup gap is dormant because a self-recursive SM function cannot admit a CPU-parallel group (mutual exclusion between the recursion gate and spike admission) | **FALSIFIED 2026-07-09 (Phase 0): LIVE** for a NESTED (branch-arm) group in a zero-param self-recursive host — no gate exists (SCC pass skips self-loops, `queries.rs:900-917`; nested admission gates are block-local, `cpu_admission.rs:157-161/508-534`). Surfaced per CCIR-4 for the deviation-judge → FRAGO seam; **FRAGO 001 routed the fix to Phase 3b**; see audit.md `executor-2026-07-09-m6-phase0` |
| A5 | No project `risk-anchors.md` override exists | **verified** (glob, this session) |
| A6 | Docker `dev` service builds + tests the full workspace per project CLAUDE.md's documented commands | **verified** (house convention, unchanged since M5) |
| A7 | `IMP-concurrency.md`'s "Design Divergences" section (:840+) is the correct home for the new bare-channel entry | **verified** (direct read, this session) |
| A8 | The Capability Ledger's roadmap-level duplication (two `## Capability Ledger` headings) is pre-existing and out of this plan's charter to fix | **verified** (direct read, this session — not a new defect M6 introduces) |

### Risk Assessment

Scored via the global [REF-risk-engine.md](../../../../docs/reference/REF-risk-engine.md) (4×5 fixed
lookup; default code-domain anchor sheet — no project override). Every mitigation names its bucket and
proof obligation per the seed catalog; no proof → the step is 0. **No Floor B class fires** (no
money/PII/security-breach/irreversible-op in the anchor-sheet sense — every change here is
git-reversible, pre-v1.0, zero public users). **Severity is scored II-Critical for the
silent-miscompile-class fixes (R1, R2, R4), consistent with this project's own established convention**
(M5 scored the identical twin-derivation/silent-miscompile shape at Sev II — the recovery cost is
real multi-round engineering debugging, not a cosmetic shrug, even pre-1.0/zero-users). **One signed
HIGH residual exists in this table — R13, the shape-arg frame-backing UAF surfaced mid-execution by
FRAGO 004; its RISK OVERRIDE block (below the table) is the plan's ONE signed HIGH-residual override,
accepted by Patrick 2026-07-09 with Phase 1b as the trigger-to-close.** All other residuals are
MEDIUM-or-below; MEDIUMs are recorded here and parked with triggers in Future Requirements.

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **R1 — UFCS-fix regression of existing (Call-based) suspension classification** (threading the authoritative resolution into 4 sites could break an already-working path) — *Phases 1–2* | C | II | H | Adversarial/RED-repro test class authored BEFORE the fix, gating the build — transitive UFCS, explicit `wait` on `MethodCall`, mixed UFCS+`Call`, `background`-spawned UFCS, PLUS full regression run of every pre-existing Call-based suspension fixture (**B2**, prob −1; proof: committed RED→GREEN fixture set, Phase 1 step 2/7) | **M** (D×II) | recorded |
| **R2 — pending_sends purge / token-salt fix is itself incomplete or racy on EITHER token producer (frame-ptr conduit tokens `emit.rs:11651-11654` OR handle-ptr tokens `handle.rs:326`)** — *Phase 3* | C | II | H | Adversarial/RED cancellation-during-send repro covering BOTH producers (frame-path AND handle-path backpressure + cancel + address-reuse simulation) gating the build, PLUS idempotency requirement on the purge at BOTH cancellation paths — the drop ladder AND `ynz_handle_free` (double-cancel is a safe no-op on either) (**B2**, prob −1; proof: committed RED→GREEN fixture pair, Phase 3 step 6) | **M** (D×II) | recorded |
| **R3 — lost-wakeup fix reorder introduces a new lock-ordering issue** — *Phase 4* | D | II | M | Adversarial multi-consumer RED repro gating the build, PLUS re-verification that P3-4's existing "no lock held across a blocking poll" clean bill still holds after the reorder (**B2**, prob −1; proof: committed fixture + re-verified clean-bill note, Phase 4 step 3/4) | **L** (E×II) | pass |
| **R4 — drop-glue ABI change (channel construction) miswires or under-covers elements** — *Phase 5* | C | II | H | `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity gate with NON-VACUOUS coverage (buffered heap-typed elements exercised, per M5's FRAGO-005 lesson against a vacuous zero-alloc pass) gating the build (**B2**, prob −1; proof: committed parity test + baseline, Phase 5 step 4) | **M** (D×II) | recorded |
| **R5 — shutdown mutex re-scope introduces a new race** — *Phase 6* | D | III | L | Mechanical mirror of the already-correct sibling pattern (`ynz_rt_run_entrypoint:995-1006`); no further mitigation needed | **L** (D×III) | pass |
| **R6 — `ynz run` signal-report change breaks existing exit-code-consuming callers** — *Phase 6* | D | IV | L | Purely additive diagnostic improvement (signal name added to the report; normal-exit and non-signal-failure paths unchanged) | **L** (D×IV) | pass |
| **R7 — docs/registry honesty sweep introduces a NEW factual drift** — *Phase 7* | D | IV | L | docs-consistency reviewer diffs every edited claim against the audit's own citations before merge | **L** (D×IV) | pass |
| **R8 — `emit.rs` merge collision between M5 and M6** — *Weather precondition* | E | III | L | Structural: M6 branches from `main` only AFTER the M5 merge + tag (Patrick-signed sequencing decision) — the collision window is eliminated by construction, not mitigated after the fact | **L** (E×III) | pass |
| **R9 — Phase-0 verification finds P1-2 and/or P2-5 are NOT dormant, OR finds a GAP in dynamic-dispatch × suspension coverage** (scope growth mid-plan) — *Phase 0* | C | III | M | This is the explicit PURPOSE of the Phase-0 gate — verify before deciding fix-vs-defer (or covered-vs-gap), per decision-philosophy's mandatory-assessment step; a non-dormant OR GAP finding routes through the plan-amendment + FRAGO seam (a GAP verdict adds the fix to Phase 1's scope, since it shares the same authoritative-resolution threading), never a silent scope change | **M** (C×III) | recorded — trigger: Phase 0 verdict itself |
| **R10 — demo/gallery/registry/roadmap reconciliation mechanical additions** — *Phase 8* | D | IV | L | Mechanical, docs-consistency + code-reviewer fan-out | **L** (D×IV) | pass |
| **R11 — P2-7 `handle_recv_poll` panic-then-pending hang (newly surfaced, NOT fixed this milestone)** — *deferred* | D | III | L | Deferred to Future Requirements with a named trigger (Phase 3's register-before-poll pattern is the natural follow-on fix) — no mitigation needed to accept LOW as a documented deferral | **L** (D×III) | recorded — deferral, not a gate pass on unmitigated work |
| **R12 — Sanitizer lane (Miri/TSan/ASan) surfaces a new confirmed bug beyond this phase's own immediate fix capacity** — *Phase 6b* | B | II | H | This phase's own existence is the engineered, bounded catch: any genuine new finding is triaged/routed through the plan-amendment + FRAGO seam before any release — never silently shipped, never silently dropped — inside a pre-1.0/zero-public-users/fully-git-reversible codebase, so a finding's real-world consequence drops from "would-be production Critical" to "caught, triaged, and fixed-or-properly-deferred pre-release" (**B2**, severity, −1 level; proof: the phase's own exit criteria — every finding triaged on the record — plus the Weather section's git-reversible/zero-users precondition) | **M** (B×III) | recorded |
| **R13 — shape-arg frame-backing UAF: a shape-value argument to a suspending callee stages a dangling stack pointer across Pending — confirmed silent-garbage miscompile, pre-existing in shipped v0.3.0, reproduces in pure `Call` form + auto-inserted transitive suspension, likely also `fixed<T>`** (FRAGO 004) — *Phase 1b* | A | II | EH | RED-repro + full-regression gate (**B2**, prob −1; proof: Phase 1b's committed deterministic RED→GREEN repros + M3a-class regression run) — real elimination needs the crossing-classifier fix itself (Phase 1b), not yet built at signing time | **H** (B×II) | **RISK OVERRIDE — SIGNED (Patrick, 2026-07-09; block below)** |

**RISK OVERRIDE — accepted residual: HIGH. SIGNED. (The plan's ONE signed HIGH-residual override —
FRAGO 004, recorded verbatim from the audit sidecar.)**

- **Risk:** shape-value argument to a suspending callee stages a dangling stack pointer across
  Pending — confirmed UAF / silent-garbage miscompile; pre-existing in shipped v0.3.0; reproduces
  in `wait fn(shapeArg)` Call form + auto-inserted transitive suspension; likely also `fixed<T>`.
- **Why not mitigable to LOW now:** standard B2 only shifts Prob A→B; Sev II is the established
  silent-miscompile anchor (D7); real elimination needs the crossing-classifier fix itself
  (Phase 1b), not yet built.
- **Accepted by: Patrick — 2026-07-09** (interactive CCIR-5 sign gate; disposition: "Sign + fix 1b
  immediately next").
- **Trigger to revisit / close:** Phase 1b's RED→GREEN fixture + full-regression proof lands
  (converts accepted-HIGH interim risk → closed, fixed bug — before Phase 6b's sanitizer lane and
  before release).

**Floor check.** No Floor-A "no backout exists" condition (every change is git-reversible) and no
Floor-B class (security/PII/money/irreversible-prod-op) fires anywhere in this table — R13 included
(pre-v1.0, zero public users, git-reversible, runtime-only-within-one-execution).

### Cross-Cutting Factor Sweep (mandatory factors, woven into the risk rows + phases above)

- **security**: N/A — no auth/secrets/injection surface touched. The ABA (R2) and lost-wakeup (R3)
  fixes are race/TOCTOU-class, scored below, not security-class.
- **perf / BigO (mem + cpu)**: addressed. P1-1's fix reuses typeck's ALREADY-computed `sig.suspends` —
  O(1) lookup per call site, no new fixpoint. The generation-salted token adds one integer compare.
  The drop-glue walk is O(buffered-element-count) — the honest cost of correctly freeing what a leak
  previously left unfreed, not a regression against any prior CORRECT baseline. No new pass is added
  to the compiler's hot compile-time path.
- **accessibility**: N/A — compiler backend; no visual UI surface in this milestone's scope.
- **PII / privacy**: N/A — compiler-internal; no user data handled.
- **compliance**: N/A — no regulatory scope.
- **SEO**: N/A — not web-facing.
- **docs**: addressed extensively — Phase 7 (honesty sweep) + this plan's own Design-Doc Alignment.
- **reusability / DRY**: addressed — [authoritative-derivation.md](../../../rules/authoritative-derivation.md)
  is the DRY discipline this plan's P1-1 and P2-4 fixes exist to satisfy (thread the one source; one
  drop-glue choke point, never a second ad hoc path).
- **type-safety**: N/A — no new user-facing type surface; the token-salting change is a runtime-ABI
  concern, covered under Safety/Performance in the Invariants block below.
- **idempotency**: addressed — the `pending_sends` purge (Phase 3) is explicitly required to be
  idempotent (a double-cancel or already-purged entry is a safe no-op, never a panic/UB).
- **error-handling**: addressed — the block_on-fallback hard-error guard (Phase 2) and the `ynz run`
  signal report (Phase 6) are both explicitly fail-loud-instead-of-silent improvements.
- **observability / logging**: addressed — the `ynz run` signal-death report (Phase 6) is a direct
  observability win.
- **race / TOCTOU**: addressed extensively — R1 (transitional classification), R2 (ABA), R3
  (lost-wakeup), R5 (mutex over-scope) are exactly this category; Phase 6b's ThreadSanitizer lane is
  the mechanical, ongoing proof surface for this whole category — CI-enforced, not a one-time review.
- **resource-cleanup**: addressed — R4 (buffered-element leak), R2 (orphaned `pending_sends` entry),
  and the explicitly-deferred R11/P2-3 are all resource-cleanup findings; Phase 6b's Miri lane is the
  mechanical, ongoing proof surface for UAF/double-free/leak classes specifically.

## 2. Mission

Fix every M6-scoped finding from the 2026-07-04 concurrency-release audit against the already-released
v0.3.0 (a patch hotfix; zero public users) — the UFCS suspension-invisibility blocker, the
pending-sends ABA + orphan leak, the recv lost-wakeup window, the unasserted block_on fallback, the
buffered-channel leak, the shutdown mutex over-scope, and the `ynz run` signal-masking gap — plus
correct every docs/registry claim that currently overstates an unshipped mechanism, **because** the
flagship concurrency feature must actually deliver the correctness it was released to deliver, and a
teaching-mission compiler cannot ship documentation that lies about what it built.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** Close the confirmed gap between what v0.3.0 shipped and what it was supposed to ship:
`wait x.method()` must actually suspend (not silently run synchronously), the channel/scheduler races
and leaks the audit confirmed must be closed with production-grade fixes (no duct tape, every
deferral a real four-field record), and every doc/registry claim about preemption and CPU-bound
routing must state the truth about what exists today versus what is deferred and why.

**Key outcomes (definition of done):**

1. `wait x.method()` — transitively, explicitly, mixed with `Call`, and under `background` — is
   classified and lowered as a real suspension at every one of the enumerated broken predicate
   sites — the original 4 (`may_block.rs:1296`, `cpu_admission.rs:823-828`, `emit.rs:653-658`,
   `emit.rs:8433-8441`) PLUS the 5 coupled sites (`count_suspension_expr` `emit.rs:5117+`;
   `emit_suspending_call_inline_poll`/`_heap_boxed` `emit.rs:10906+`; `callee_name_from_call_expr`
   `emit.rs:7671`; `lower_expr_background` `emit.rs:15569`/`:15801`/`:12431`;
   `suspending_calls_in_subexpr_position` `check.rs:704`) per FRAGO 003 — all threaded from the ONE
   authoritative `sig_table.fns.get(method)` resolution; zero regression in existing Call-based
   suspension fixtures (Phase 1). **Correctness of a shape (or `fixed<T>`) aggregate ARGUMENT
   surviving that suspension — the pre-existing v0.3.0 shape-arg frame-backing UAF that keeps
   fixture (b) RED — is Phase 1b's deliverable, not Phase 1's (FRAGO 004, signed).**
1b. No shape (or `fixed<T>`) aggregate argument to a suspending callee is left in a dying stack
   alloca across suspension — the confirmed pre-existing v0.3.0 UAF (parent Pending → stack dies →
   child resumes on dangling `self` → silent garbage) is closed by extending the ONE authoritative
   crossing classifier so the value is frame-backed via the existing `shape_embed` machinery,
   proven by deterministic-across-runs RED→GREEN repros (Phase 1b, FRAGO 004).
2. The block_on-fallback branch (`emit.rs:15122-15137`) is a compile-time hard error for any caller
   not reachable via the designated synchronous entry point — mirroring `emit.rs:11162`'s sibling.
3. A cancelled sender's `pending_sends` entry is purged (idempotently) and the `caller_token` is
   generation-salted, across BOTH token-producer sites (the frame-pointer conduit token minted at
   `emit.rs:11651-11654` AND the handle-pointer task-handle token minted at `handle.rs:326`) — the
   ABA class and the orphan leak are both closed for both producers with committed RED→GREEN repros.
4. The `ynz_channel_recv_poll` register/poll race is closed (register-before-poll or a single lock
   across both), re-verified against P3-4's existing "no lock across a blocking poll" clean bill.
5. Buffered channel elements are freed at channel drop via a single codegen-registered drop-glue
   choke point (not a second ad hoc path) — alloc=free parity proven non-vacuously.
6. `ynz_rt_shutdown` never holds the RUNTIME mutex across its drain window; `ynz run` reports a
   signal-terminated child by name instead of masking it as exit code 1.
7. `IMP-no-function-coloring.md`'s preemption section and `background.cpuBound`'s spec both get
   accurate mechanism text plus real registry deferral entries; the bare-channel non-closure footgun
   is documented loudly in `IMP-concurrency.md`'s Design Divergences section; FFI/foreign and
   `KernelModeRejectsWait` doc tense is corrected.
8. `examples/pirates-roster/entrypoint.ynz` demonstrates `wait x.method()` in context;
   `examples/primantis-orders/m6_errors.ynz` exists with WHY-commented triggers for every new
   compile-time diagnostic this milestone adds; the roadmap + Capability Ledger record M6; the full
   workspace suite is green.
9. P2-3, P1-2 (confirmed dormant, Phase 0), P2-7 (newly surfaced), and the dynamic-dispatch ×
   suspension predicate gap (FRAGO 002 → Future Requirements #10) are recorded as proper four-field
   deferrals in Future Requirements — never silent, never a loose checkbox. P2-5, confirmed LIVE in
   Phase 0 (FRAGO 001), is FIXED in Phase 3b rather than deferred.
10. The `ynz-runtime` crate is Miri-clean and clean under ThreadSanitizer/AddressSanitizer (or every
    finding is triaged on the record) — and a dedicated sanitizer CI job is live in
    `.github/workflows/ci.yml`, proven non-vacuous, so the bug classes this milestone fixes (UAF,
    double-free, data races) are mechanically hunted on every future push/PR, not just today.

**Disciplined initiative.** When steps and reality diverge: **verify before you fix** (every fix in
this plan traces to a CONFIRMED or CONFIRMED-BY-MECHANISM audit finding; a THEORY/CLAIMED finding
gets verified — Phase 0 — before it is fixed or deferred). **Thread the one authoritative source; never
invent a second derivation** to unblock yourself — surface the blocker instead (CCIR-2). **A
mitigation with no committed proof artifact is worth zero** — do not claim a RED→GREEN fixture exists
without committing it. **No duct tape** — a fix that "mostly" closes a race or leak, with no four-field
deferral naming the remaining gap, is not done.

### 3.2 Concept

Twelve phases (0–8, with 1b inserted between 1 and 2 per FRAGO 004 — Patrick-signed to run
immediately after Phase 1 and BEFORE Phase 2 — 3b inserted between 3 and 4 per FRAGO 001, and 6b
inserted between 6 and 7 — see the amendment note in Terrain). **Gate first**
(P0 verifies the two THEORY findings, the dynamic-dispatch × suspension coverage question, and
confirms the execution-gate precondition). **The flagship blocker + its escape hatch** (P1 UFCS fix
on its carved 9-site scope; P1b the shape-arg frame-backing UAF fix — FRAGO 004, signed sequencing
P1 → P1b → P2; P2 the block_on-fallback
guard — sequenced after the P1/P1b pair because P2's correctness assertion depends on P1 actually being
fixed, a deliberate resequencing of the audit's raw synthesis order, recorded as Decision D1 below).
**Channel/scheduler correctness** (P3 ABA+orphan; P3b recursion-chain spike CPU-handle cleanup leak —
FRAGO 001, sequenced right after P3 in the same drop-ladder region; P4 lost-wakeup; P5
buffered-element leak — independent subsystems apart from the P3→P3b adjacency, sequenced for one
conductor's convenience, not a hard dependency chain).
**Mechanical + honesty** (P6 two small independent fixes; P6b sanitizer lane — Miri/TSan/ASan on the
runtime crate, proven non-vacuous and CI-enforced going forward; P7 docs/registry sweep).
**Close-out** (P8 demo/gallery/roadmap/full-suite/release-handoff). Each phase ends green-tree with its
fixtures committed; Phase 1 (the flagship, >5 steps) checkpoints per the marks below.

### 3.3 Phases

#### Phase 0 — Dormancy verification + execution-gate confirmation (GATE)

- **Task + purpose:** confirm the execution-gate precondition (M5 merged + tagged) before any other
  phase starts, resolve the two THEORY/CLAIMED audit findings (P1-2, P2-5) from "verify dormancy
  first" to a real fix-vs-defer decision — per decision-philosophy, never assume dormant — AND
  determine whether the dynamic-dispatch × suspension coverage question (this amendment's newly-flagged
  open question, Terrain above) is genuinely COVERED by the existing/soon-to-be-threaded predicates or
  is a real GAP — per decision-philosophy, never assume covered either.
- **Steps**
  1. Confirm the Weather precondition: `main` includes the M5 auto-SoA merge and the M5 tag is cut.
     **STOP the whole plan if not** — do not proceed to Phase 1.
  2. Re-verify every audit file:line citation this plan cites against the live tree (CCIR-1) — record
     any drift found; the fix's substance carries forward even if a line number moved.
  3. **P1-2 dormancy check**: read whether `Cg.type_params` can be non-empty when
     `find_let_typeck_type_in_stmts` (`emit.rs:8276`) or `find_let_type_in_stmts`
     (`emit.rs:8364`) run in SM-resume context, and confirm both frame-layout call sites still filter
     `generics.is_empty()`. Verdict: DORMANT or LIVE.
  4. **P2-5 dormancy check**: read whether a self-recursive SM function can admit a CPU-parallel group
     (the mutual-exclusion gate between the recursion slot and spike admission). Verdict: DORMANT or
     LIVE.
  5. **Dynamic-dispatch × suspension coverage check**: read whether the suspension predicates
     (`may_block.rs`'s call-graph builder, `cpu_admission.rs`'s admission check, `emit.rs`'s
     `collect_callees_in_expr` / `is_direct_suspending_call`) handle `dynamic Contract` method calls
     (vtable dispatch resolving to a suspending impl), and confirm whether typeck even permits a
     suspending function to satisfy a `follows` contract signature in the first place (if it can't,
     the gap is moot by construction — record that finding too). Verdict: COVERED or GAP.
  6. Route each verdict: DORMANT (P1-2/P2-5) → record as a Future-Requirements deferral (below) with
     the confirmed-dormant reasoning; LIVE (P1-2/P2-5) → file a plan-amendment + FRAGO adding a fix
     phase for that item (per [plan-source-of-truth.md](../../../rules/plan-source-of-truth.md) —
     never silently fold it into an existing phase's scope); COVERED (dynamic-dispatch × suspension) →
     record the confirmed-safe reasoning (this plan's own execution record — no further action needed);
     GAP (dynamic-dispatch × suspension) → file a plan-amendment + FRAGO adding the fix to Phase 1's
     scope, since it shares the exact same authoritative-resolution threading Phase 1 already builds
     for the 4 UFCS predicate sites — never fold it in silently without the seam.
- **Exit criteria:** execution-gate precondition confirmed; both dormancy verdicts recorded (DORMANT
  with reasoning, or a FRAGO adding the needed fix phase); the dynamic-dispatch × suspension coverage
  verdict recorded (COVERED with reasoning, or a FRAGO adding the fix to Phase 1's scope); any citation
  drift noted.
- **Reviewer fan-out:** adversarial gate-checker (are the dormancy AND coverage verdicts actually
  proven by reading the real code, not narrated?); design-doc-alignment reviewer (the execution-gate
  precondition itself).
- **Model tag:** `(coding, standard, small)`

**Phase 0 verdicts (executor `executor-2026-07-09-m6-phase0`, 2026-07-09):**

1. ✅ **Execution gate SATISFIED** — `97e32f6` (M5 merge) is an ancestor of `main`; tag `v0.3.0-m5`
   → `f5495c9`. Pre-existing `v0.3.0-m6`/`v0.3.0-m7` tags are pre-cut planning-era release-style
   commits (`f0d4946`/`e7c00a7`), not prior M6/M7 code work — non-blocking, flagged for release
   hygiene (a real M6 `/release` will collide with the existing `v0.3.0-m6` tag name).
2. ✅ **Citation re-verify (CCIR-1)**: all substantive citations MATCH the live tree. Trivial
   drifts: `run.rs:75` → `run.rs:76`; `Cg.type_params` is actually `Cg.type_subst`
   (`emit.rs:1763-1765`); roadmap's duplicate `## Capability Ledger` sections now at lines
   **417 and 471** (not 365/417 — Phase 8 must target the new lines); `runtime.rs` no-op stub
   body at :297-299. Full match list in audit.md.
3. ✅ **P1-2: DORMANT** — deferral #2 stands (confirmed dormant; SM machinery is
   `generics.is_empty()`-gated at `emit.rs:252/308/1228/1277`, generic lowering carries empty
   suspend machinery at `emit.rs:1490-1496`, generics cannot suspend per `check.rs:4005-4020`).
4. ⚠️ **P2-5: LIVE** (assumption A4 falsified) — a zero-param self-recursive suspending host with
   a NESTED branch-arm CPU group IS admitted as a spike host (no mutual-exclusion gate: SCC pass
   emits only `len >= 2` components so self-loops pass, `queries.rs:900-917` +
   `may_block.rs` Kosaraju `component.len() >= 2`; nested admission gates are block-local,
   `cpu_admission.rs:157-161/508-534`; M3g Phase 3 removed the co-resident-suspension decline),
   while the drop ladder cleans spike CPU handles on the ROOT frame only (`runtime.rs:607` vs
   chain walk `:659-680`) → chain-child `CpuJoinHandle` leak on cancellation. **Surfaced per
   CCIR-4 for the deviation-judge → FRAGO seam — not fixed, not folded into a phase by this
   executor.** Full evidence chain in audit.md. **Seam ruled (FRAGO 001, JUSTIFIED/RISK-NEUTRAL):
   fix routed to the new Phase 3b below.**
5. ⚠️ **Dynamic-dispatch × suspension: GAP at the predicate layer, UNREACHABLE today** — typeck
   permits a suspending fn to satisfy a `follows` contract (`check_follows_contracts`,
   `check.rs:5052-5136`, never reads `suspends`), and all four predicates are MethodCall-blind
   (same shape as P1-1); but every `dynamic Contract` call site hard-errors in codegen
   ("dynamic dispatch call sites not yet lowered in M4 P4", `emit.rs:14622-14625`) — a loud
   compile-time error, never a silent mis-suspension. **Surfaced for the seam to route**:
   either record-with-trigger (dynamic-dispatch lowering ships) or FRAGO folding predicate
   coverage into Phase 1's threading. **Seam ruled (FRAGO 002, JUSTIFIED/RISK-NEUTRAL):
   deferral-with-trigger per the D4/P2-3 precedent — recorded as Future Requirements #10, NOT
   folded into Phase 1.**

#### Phase 1 — P1-1: UFCS suspension invisibility (BLOCKER fix)

- **Task + purpose:** thread the ONE authoritative UFCS-suspends resolution
  (`sig_table.fns.get(method)`, per the Terrain citation above) into all enumerated broken predicate
  sites — the original 4 (`may_block.rs:1296`, `cpu_admission.rs:823-828`, `emit.rs:653-658`,
  `emit.rs:8433-8441`) PLUS the 5 coupled sites (`count_suspension_expr` `emit.rs:5117+`;
  `emit_suspending_call_inline_poll`/`_heap_boxed` `emit.rs:10906+`; `callee_name_from_call_expr`
  `emit.rs:7671`; `lower_expr_background` `emit.rs:15569`/`:15801`/`:12431`;
  `suspending_calls_in_subexpr_position` `check.rs:704`) per FRAGO 003 — so `wait x.method()`
  actually suspends, with a from-scratch RED fixture class authored BEFORE the fix per
  verify-before-you-fix. **Scope carve-out (FRAGO 004, signed):** fixture (b)'s residual RED — the
  shape-arg frame-backing UAF, a confirmed pre-existing v0.3.0 miscompile OUTSIDE this phase's
  9-site predicate-threading scope — is NOT this phase's deliverable; it is carved out to Phase 1b
  as that phase's locked RED-repro (a legitimate planned-RED per no-duct-tape's inverse:
  documented, locked by the failing test, closed by the immediately-next phase, never shipped
  alone).
- **Steps**
  1. Confirm the exact current shape of `check.rs`'s UFCS resolution (CCIR-1 re-verify — this plan's
     line numbers are this-session-verified but may drift); design the shared helper (or the minimal
     threading) that exposes the SAME `sig_table.fns.get(method)` lookup to `may_block.rs`,
     `cpu_admission.rs`, and both `emit.rs` sites — never a re-derivation.
  2. Author the RED fixture class BEFORE touching any of the 4 sites: transitive suspension via UFCS
     (`a()` calls `b.method()` which suspends), explicit `wait` directly on a `MethodCall` node, mixed
     UFCS+`Call` in one expression tree, and a `background`-spawned UFCS suspending call. Each fixture
     asserts the CORRECT (currently-failing) behavior. Commit RED, gating the build.

     **CHECKPOINT** — RED fixture class committed and confirmed failing for the documented reason
     (not some unrelated bug); scaffolding clean.
  3. Fix `may_block.rs:1296-1318` (`collect_calls_in_expr` MethodCall arm) to add the call-graph edge
     via the shared resolution. Re-run the transitive `SuspendSet` fixpoint tests.
  4. Fix `cpu_admission.rs:823-828` (`expr_contains_suspending_call`) the same way.
  5. Fix `emit.rs:653-658` (`collect_callees_in_expr`) so frame layouts embed the UFCS callee's
     sub-frame.
  6. Fix `emit.rs:8433-8440` (`is_direct_suspending_call`) to recognize a `MethodCall` whose resolved
     callee suspends, not only `Call`+`Ident`.

     **CHECKPOINT** — all 9 sites (the original 4 + the 5 coupled, per FRAGO 003) threaded from the
     one source; grep gate confirms zero independent method→fn suspend derivation anywhere outside
     the shared resolution (authoritative-derivation.md discipline).
  7. Run the RED fixture class + the FULL existing suspension/state-machine test suite (`docker
     compose run --rm dev cargo test --workspace`). RED fixtures flip to GREEN (per FRAGO 004:
     fixtures (a)/(c)/(d) — fixture (b) stays RED as Phase 1b's carved-out locked repro); every
     pre-existing Call-based suspension fixture stays GREEN (the R1 regression gate).
  8. Pin the audit's open question (block vs panic when UFCS previously fell through to the
     synchronous wrapper): capture the pre-fix failure mode via a controlled repro (on a throwaway
     branch/stash if the fix is already applied) and record which mode it WAS, closing the audit's
     "TBD" note.
- **Exit criteria:** all enumerated sites — the original 4 (`may_block.rs:1296`,
  `cpu_admission.rs:823-828`, `emit.rs:653-658`, `emit.rs:8433-8441`) PLUS the 5 coupled sites
  (`count_suspension_expr` `emit.rs:5117+`; `emit_suspending_call_inline_poll`/`_heap_boxed`
  `emit.rs:10906+`; `callee_name_from_call_expr` `emit.rs:7671`; `lower_expr_background`
  `emit.rs:15569`/`:15801`/`:12431`; `suspending_calls_in_subexpr_position` `check.rs:704`) per
  FRAGO 003 — read from the one shared authoritative source (grep-verified, zero second
  derivation); RED→GREEN fixture class committed for fixtures (a)/(c)/(d), with fixture (b)
  committed and carved out as Phase 1b's locked RED-repro (FRAGO 004 — the shape-arg
  frame-backing UAF, closed by Phase 1b immediately next, never shipped alone); zero regression
  in existing suspension fixtures; the historical block-vs-panic question answered and recorded.
- **Reviewer fan-out:** code-reviewer (the 9-site diff, per FRAGO 003); adversarial gate-checker
  (does the RED
  fixture class genuinely exercise transitive / explicit / mixed / `background` UFCS, not just one
  shape?); design-doc-alignment reviewer (authoritative-derivation.md compliance — zero second
  derivation, grep-gate evidence attached).
- **Model tag:** `(coding, high, medium)` — checkpoint marks mandatory (>5 steps).

**Phase 1 complete on carved scope (executor `executor-2026-07-09-m6-phase1-seg4`, 2026-07-09):**
all 9 FRAGO-003 sites threaded from the one authoritative resolution — grep gate PASS (one
classifier + one AST wrapper; the only name-keyed producer is the ratified pre-typing
`may_block.rs` fixpoint edge; zero second derivation); R1 zero-regression confirmed across every
pre-existing Call-based suspension fixture; house clippy green; RED fixture class 3/4 GREEN
(a/c/d); block-vs-panic answered and recorded: **PANIC-then-abort** (step 8, closing the audit's
"TBD"); fixture (b) → Phase 1b (FRAGO 004 — its locked RED-repro for the shape-arg frame-backing
UAF).

#### Phase 1b — shape-arg frame-backing miscompile (confirmed UAF; pre-existing v0.3.0)

- **Task + purpose:** close the confirmed pre-existing use-after-free (FRAGO 004 — surfaced by
  Phase 1 segment 4, corroborated by the adversarial code-reviewer) where a shape-value argument
  passed to a suspending callee is staged in the PARENT resume fn's STACK alloca: the child frame
  holds a `ptrtoint` of it; parent returns Pending → parent stack dies → child resumes on a
  dangling `self` → silent nondeterministic garbage. Reproduces in pure `Call` form
  (`wait crew(ship)`) AND auto-inserted transitive suspension; PRE-EXISTING in shipped v0.3.0
  (stash-to-`main` proof — NOT introduced by M6; UFCS merely made the shape-passing form natural
  to exercise); likely also affects `fixed<T>` (string/array/map are safe — heap-backed, stable
  pointer). Root cause: `locals_crossing_wait`/`collect_crossings_in_stmts`
  (`crates/ynz-typeck/src/check.rs:8122+`) is a lexical "read-AFTER-suspension" source scan that
  misses escape-through-a-callee-frame — the suspending callee holds the arg by pointer and reads
  it after its own suspend point, so the value DOES cross, but the scan never flags it → no
  `shape_embed` → plain stack alloca (`emit.rs:~4508`'s `is_shape → shape_embed_set` never
  fires). M3a-class caution: this subsystem carries a ~10-round whack-a-mole history (D7) — RED
  repro before fix, one authoritative crossing classifier, never a second frame-layout path
  ([authoritative-derivation.md](../../../rules/authoritative-derivation.md)).
- **Steps**
  1. CCIR-1: re-verify the cited lines against the live tree (`check.rs:8122+`
     `locals_crossing_wait`/`collect_crossings_in_stmts`; `emit.rs`'s `shape_embed`/`is_shape`
     frame-backing path — re-grep the exact lines rather than trust this plan's citations).
  2. Lock the RED repros (RED-repro-before-fix per
     [verification.md](../../../rules/verification.md)): fixture (b)
     `v0_3_m6_ufcs_explicit_wait.ynz` (already in tree, currently RED — Phase 1's carved-out
     locked repro) PLUS a pure-`Call` shape-arg repro (`const c = wait crew(ship)`) PLUS (if
     constructible) a `fixed<T>` stack-aggregate variant. Each asserts the CORRECT value
     (deterministic, not garbage). Confirm each fails RED for the documented UAF reason, not some
     unrelated bug.
  3. Fix: extend the ONE authoritative crossing classifier so a shape (and `fixed<T>`) aggregate
     passed BY POINTER to a suspending callee is classified as crossing → frame-backed via the
     existing `shape_embed` machinery — never a second frame-layout path
     (authoritative-derivation.md). Verify the emitted IR: the arg lives in the heap frame, not a
     dying stack alloca; the child's `self` points into the surviving frame.
  4. Full-regression: all Phase 1 fixtures (a, b, c, d) GREEN; full workspace suite green
     (`docker compose run --rm dev cargo test --workspace`); house clippy `-D warnings` clean.
     Explicitly re-verify no regression to the M3a/M4/M5 suspension + frame-layout suites (this
     is the fragile subsystem).
  5. Non-vacuous determinism proof: run the repro N times; assert the SAME correct value every
     run (the pre-fix signature was nondeterministic garbage across runs).
- **Exit criteria:** UAF closed via the one crossing-classifier extension (no second frame-layout
  path); RED→GREEN repros committed (fixture (b) + the pure-`Call` repro + the `fixed<T>`
  variant if constructible), deterministic across runs; full suite green including the M3a-class
  regression surface (M3a/M4/M5 suspension + frame-layout suites); the FRAGO 004 signed RISK
  OVERRIDE's revisit-trigger satisfied — this phase's proof landing converts the accepted-HIGH
  interim risk (R13) to closed, fixed bug.
- **Reviewer fan-out:** code-reviewer (the crossing-classifier + frame-layout diff); adversarial
  gate-checker (does the repro genuinely exercise the dangling-stack window across pure-`Call`
  AND auto-inserted transitive suspension, and is the determinism proof non-vacuous?);
  design-doc-alignment reviewer (authoritative-derivation.md — the ONE crossing classifier
  extended, no second frame-layout path).
- **Model tag:** `(coding, high, medium)`

#### Phase 2 — P4-3: block_on-fallback hard-error guard

- **Task + purpose:** close the escape hatch at `emit.rs:15122-15137` so any non-SM-classified caller
  reaching a suspending call is a compile-time hard error, mirroring the sibling recursive-path hard
  error at `emit.rs:11162` — sequenced immediately after Phase 1 because this guard's correctness
  assertion (unreachable for non-main callers) depends on P1-1 actually being fixed (Decision D1).
- **Steps**
  1. Read `emit.rs:11162`'s sibling hard-error pattern (message shape, diagnostic kind, WHAT/
     WHAT-INSTEAD/WHY text) to mirror exactly — no bespoke new diagnostic shape.
  2. Check `registry/features.toml` for an existing `[[diagnostic_template]]` covering
     `emit.rs:11162`'s error; reuse it if the shape matches, or add a new template entry if it
     genuinely doesn't (recorded via FRAGO if new — see Feature Registry Entries below).
  3. Add the assert-unreachable guard at `emit.rs:15122-15137`: any caller reaching this fallback that
     is not the designated synchronous entry point is a compile error. WHAT: caller wasn't classified
     as a state machine but reaches a suspending call. WHAT-INSTEAD: points at wrapping the call in a
     `wait`/`background` context, or filing a compiler bug if the caller looks correctly classified.
     WHY: a silent synchronous drive from inside the runtime either deadlocks or panics, per
     `runtime.rs:921-925`'s own documented invariant.
  4. Author a RED fixture proving the guard fires on a deliberately-miscategorized caller
     (constructed at the internal/fixture level, since post-Phase-1 this should be UNREACHABLE from
     real Yinz source) and confirm normal `wait x.method()` / `wait fn()` paths never trip the new
     hard error (a false-positive sweep across the full example/test corpus).
  5. Run the full suite; confirm zero regressions.
- **Exit criteria:** guard live; RED fixture for the guard itself; zero false positives across the
  whole example/test corpus; registry `diagnostic_template` check recorded (reused or newly added).
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (false-positive sweep); design-doc-alignment
  reviewer (IMP-no-function-coloring.md's no-bridge invariant — this guard is what keeps it
  enforced, not merely documented).
- **Model tag:** `(coding, standard, small)`

#### Phase 3 — P3-1/P2-2: `pending_sends` ABA + orphan purge (BOTH token producers)

- **Task + purpose:** eliminate the `caller_token` ABA class and the orphaned `pending_sends` leak
  across BOTH token-producer sites — the frame-pointer conduit token minted in codegen at
  `emit.rs:11651-11654` (bare-channel `.send()`) AND the handle-pointer token minted inside
  `ynz_handle_send_poll` at `handle.rs:326` (`h.send()`), both landing in the SAME
  `pending_sends` map on the shared `YnzChannel` — via (a) purge-on-cancellation at BOTH
  cancellation paths (the drop ladder's kind-2 `BgArgDropEntry` for frame tokens; inside
  `ynz_handle_free` — which already holds `msg_chan` — for handle tokens) through ONE shared purge
  helper, and (b) a generation-salted token, from ONE shared salting scheme covering both token
  kinds, as defense-in-depth — both mitigations, not either/or (Decision D2), because purge alone
  still has a residual race window between cancellation and purge completing. Fable's personal
  plan-audit (`.claude/audits/2026-07-04-concurrency-release-audit.md` "P3-1 ADDENDUM") found the
  second producer after both the scout and the original reviewer pass missed it — per
  [authoritative-derivation.md](../../../rules/authoritative-derivation.md), the fix threads ONE
  scheme to every producer, never patches only the first one found.
- **Steps**
  1. Re-confirm BOTH cancellation paths' current behavior: the drop ladder's kind-2
     `BgArgDropEntry` handling (`runtime.rs:591-693`, confirm it currently only calls
     `ynz_channel_free`) AND `ynz_handle_free` (`handle.rs:337-351`, confirm it calls
     `ynz_channel_free(handle.msg_chan)` but never purges the handle-keyed entry).
  2. Design and implement the ONE shared purge helper (authoritative-derivation.md) — takes a
     channel pointer + a `caller_token`, removes the matching `pending_sends` entry if present.
     **Must be idempotent by construction** — a double-cancel or already-purged entry (no matching
     key) is a safe no-op, never a panic or UB.
  3. Wire the purge into the drop ladder's kind-2 arg-drop entry (frame-ptr token) BEFORE (or
     atomically with) `ynz_channel_free`'s refcount decrement.

     **CHECKPOINT** — frame-path purge wired to the shared helper, confirmed idempotent (a
     repeated-cancel unit test on the frame path passes); ready to wire the handle path in a fresh
     dispatch if resumed later.
  4. Wire the SAME shared helper into `ynz_handle_free` (handle-ptr token) — it already holds
     `handle.msg_chan`, so it purges its own handle-keyed entry BEFORE releasing the channel ref via
     `ynz_channel_free`. Confirm this purge is also idempotent (a repeated-cancel unit test on the
     handle path passes).
  5. Implement the generation-salted `caller_token` covering BOTH producers: replace the raw
     frame-pointer token (`emit.rs:11651-11654`) AND the raw handle-pointer token (`handle.rs:326`)
     with a `(ptr, generation_counter)` pair (or equivalent monotonic salt) from ONE shared salting
     scheme threaded to both mint sites — never two independently-salted token shapes — so a reused
     address (frame OR handle) cannot collide with a stale entry even inside the purge's own race
     window.

     **CHECKPOINT** — both purge call sites wired to the one shared helper (idempotent on both
     paths) and the token salted for both producers from one scheme; ready to author the RED repro
     suite in a fresh dispatch if resumed later.
  6. Author the RED repro suite covering BOTH producers: **(a) frame-path** — cancel a task
     suspended on `send` under channel backpressure, force or instrument frame-address reuse
     (whichever is provably exercisable in the test harness), assert the new task's send delivers
     and the old task's stale entry never resurfaces; **(b) handle-path** — a task's `h.send()`
     suspends under backpressure, the handle is freed via `ynz_handle_free`, a new handle allocated
     at the (forced/instrumented) reused address sends — assert delivery correctness and no stale
     resurrection. Commit RED before the fix lands; gate the build on GREEN after.
  7. Run the full suite + both new fixtures; confirm no regression in M4's existing channel/handle
     fixture suites.
- **Exit criteria:** purge implemented at BOTH cancellation paths via the one shared helper and
  proven idempotent on both; token salted for BOTH producers via one shared scheme; RED→GREEN
  fixture pair committed (frame-path AND handle-path variants); full suite green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the fixture pair genuinely
  exercise the ABA window for BOTH producers — frame AND handle — or only one, or only the leak
  half?); design-doc-alignment reviewer (`IMP-concurrency.md`'s cancel-via-drop invariant and its
  P4-5 "actively guarded" self-assessment — confirm it still holds after this fix; and
  authoritative-derivation.md compliance — one purge helper, one salting scheme, threaded to both
  producers, never two ad hoc schemes).
- **Model tag:** `(coding, high, medium)` — checkpoint marks mandatory (>5 steps).

#### Phase 3b — P2-5: recursion-chain × spike CPU-handle cleanup leak (LIVE — confirmed Phase 0)

- **Task + purpose:** close the confirmed cancellation-cleanup leak (FRAGO 001 — Phase 0 falsified
  assumption A4) where a recursion-chain child suspended at its CPU join leaks its boxed
  `CpuJoinHandle`s. Root cause (Phase-0-verified, corroborated by the adversarial gate-checker):
  `SpawnStateFnFuture::drop` runs `cleanup_spike_cpu_handles` on the ROOT frame only
  (`runtime.rs:607`), while the recursion-chain walk (`runtime.rs:659-680`) frees each child's
  sleep handle + frame but never its spike CPU handles. Reachable via a nested branch-arm
  CPU-parallel group in a zero-param self-recursive suspending host (no mutual-exclusion gate
  excludes it: SCC self-loops survive `find_mutual_suspension_cycles`' `len() >= 2` filter at
  `may_block.rs:1617`; nested admission is block-local at `cpu_admission.rs:508-534`; M3g Phase 3
  removed the co-resident-suspension decline). Assumption A4 falsified.
- **Steps**
  1. CCIR-1: re-verify the cited lines against the live tree (`runtime.rs:607`,
     `runtime.rs:659-680`, `may_block.rs:1617`, `cpu_admission.rs:508-534`, and
     `queries.rs:900-917` in `ynz-typeck` — note the correct crate is `ynz-typeck/src/queries.rs`,
     a Phase-0 citation-drift correction).
  2. Author a RED repro BEFORE the fix (verify-before-you-fix, per
     [verification.md](../../../rules/verification.md)): a zero-param self-recursive suspending
     host with a nested branch-arm pure-CPU group, cancelled while a chain child is suspended at
     its CPU join; assert (via `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity or handle-count
     instrumentation) that the child's `CpuJoinHandle`s are freed on cancellation. Commit RED,
     gating the build.
  3. Fix: extend the recursion-chain drop walk (`runtime.rs:659-680`) to call the SAME
     `cleanup_spike_cpu_handles` on each chain child that the root frame already uses
     (`runtime.rs:607`) — one authoritative cleanup path threaded to both root and chain children,
     never a second ad hoc drop path
     ([authoritative-derivation.md](../../../rules/authoritative-derivation.md)).
  4. Correct the stale `queries.rs:942-943` comment (in `ynz-typeck`) that still claims structural
     inertness via the Phase-3-removed co-resident-suspension decline — the docs-honesty sibling
     of this leak fix, squarely in M6's "docs must not lie" charter.
  5. Run the full suite + the new RED fixture (now GREEN); confirm no regression in M3g/M4
     recursion + CPU-group fixtures.
- **Exit criteria:** leak closed via the one shared cleanup choke point; RED→GREEN repro committed
  with non-vacuous alloc=free (or handle-count) parity; stale `queries.rs:942-943` comment
  corrected; full suite green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the repro genuinely exercise
  the nested-branch-arm self-recursive-host cancellation window, or a broader/different leak?);
  design-doc-alignment reviewer (authoritative-derivation.md — one cleanup choke point threaded to
  root + chain children, not a second path).
- **Model tag:** `(coding, high, medium)`

#### Phase 4 — P3-2: `ynz_channel_recv_poll` lost-wakeup window

- **Task + purpose:** close the register-before-poll race in `channel.rs:311-339` without
  reintroducing a lock-held-across-a-blocking-poll violation (P3-4's existing clean bill).
- **Steps**
  1. Re-confirm the current ordering (`poll_recv` then `record_recv_waiter` as two separate critical
     sections) and that `channel.rs:331`'s Ready-path wakes every recorded waiter.
  2. Fix: reorder to register-the-waiter-before-poll, or hold one lock across poll+record — pick
     whichever is the smaller, more mechanical diff consistent with the existing lock discipline.
  3. Author the RED repro: consumer A registers late, consumer C's poll clobbers the single-slot
     waker, a send fires before A's registration — assert A is always woken. The repro should drive
     `ynz_channel_recv_poll` via direct synchronous unit-test polling with manual wakers (deterministic
     interleaving by construction, per the `channel.rs` test precedent — see its existing
     `CountingWaker`/manual-`Waker` pattern) rather than real thread races; if a true-race variant is
     added it is best-effort, not the gate.
  4. Run the full suite; explicitly re-verify P3-4's "no lock held across a blocking poll" clean bill
     still holds after the reorder (do not just assume it — re-read the changed code against that
     specific invariant).
- **Exit criteria:** race closed; RED→GREEN fixture; P3-4's clean-bill invariant re-verified, not
  merely carried forward.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the repro exercise the narrow
  window Fable identified, or a broader/different hang?).
- **Model tag:** `(coding, standard, small)`

#### Phase 5 — P2-4: buffered-channel heap-element leak (design decision + fix)

- **Task + purpose:** stop buffered channel elements (and any residual `pending_sends` payloads) from
  leaking at channel drop via a single codegen-registered drop-glue function pointer, registered once
  at channel construction — the ABI-blast-radius-flagged design decision (D3 below) — and prove
  non-vacuous alloc=free parity.
- **Steps**
  1. **Design decision (recorded as D3 below):** register a drop-glue fn ptr at `channel<T>()`
     construction (T is statically known there), stored on `YnzChannel`, over a separate
     drain-on-last-free mechanism — because draining still needs per-element drop knowledge, so the
     fn-ptr mechanism is required either way; registering it ONCE at construction is the single
     authoritative choke point (authoritative-derivation.md), avoiding a second ad hoc drop path P2-3's
     eventual fix would otherwise have to reconcile with. **Named ABI blast radius**: `ynz_channel_new`'s
     C-ABI signature gains a parameter; every codegen call site constructing a `channel<T>()` must be
     updated — bounded to channel-construction sites (materially smaller than M5's ~20+-site array-ABI
     cut).

     **CHECKPOINT** — design decision (D3) recorded and the ABI blast radius named; ready to hand off
     the field-addition + codegen wiring work to a fresh dispatch if resumed later.
  2. Implement: add the drop-glue fn ptr field to `YnzChannel`; wire codegen to synthesize/pass the
     glue at each `channel<T>()` construction; add an explicit teardown routine (there is currently no
     `Drop` impl, audit-confirmed) that walks the buffer — and, defensively, any residual
     `pending_sends` payloads — invoking the glue on the channel's last-ref drop.
  3. Explicitly confirm P2-3 (closed-send leak) is UNCHANGED by this phase — the closed1/closed2
     codegen blocks stay untouched and stay deferred (Future Requirements #1); this phase's drop-glue
     mechanism must not accidentally start reaching them.
  4. Author the alloc=free parity gate: `YNZ_ALLOC_COUNTER_OUTPUT` before/after for a channel holding
     heap-element types (`string`/`array`/`map`/`shape`) dropped with elements still buffered — assert
     parity, and confirm the gate shows NON-ZERO allocations exercised (never a vacuous zero-alloc
     pass, per M5's FRAGO-005 lesson).
  5. Run the full suite; confirm no regression to channel send/receive semantics.
- **Exit criteria:** drop-glue mechanism live; alloc=free parity gate GREEN with non-vacuous coverage;
  P2-3 confirmed untouched and still correctly deferred.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (parity-gate non-vacuousness, per the
  M5 FRAGO-005 precedent); design-doc-alignment reviewer (authoritative-derivation.md — one drop-glue
  choke point, no parallel path).
- **Model tag:** `(coding, high, medium)`

#### Phase 6 — Mechanical fixes: shutdown mutex scope + `ynz run` signal masking

- **Task + purpose:** two small, independent, mechanical correctness fixes bundled for phase
  efficiency — neither depends on any other phase in this plan.
- **Steps**
  1. **P3-3**: scope `ynz_rt_shutdown`'s RUNTIME mutex (`runtime.rs:316-354`) so it does not hold
     across the up-to-5s `shutdown_timeout` — mirror the already-correct
     `ynz_rt_run_entrypoint:995-1006` pattern (extract the owned Runtime, drop the lock, drain
     outside).
  2. **`ynz run` signal masking**: fix `crates/ynz-driver/src/run.rs:75` so a signal-terminated child
     process reports the signal by name rather than silently returning exit code 1 (use
     `std::os::unix::process::ExitStatusExt::signal()` or equivalent); write the report as a
     WHAT/WHAT-INSTEAD/WHY-quality message per Golden Rule 11 ("the program was killed by signal N
     (SIGSEGV) — likely an out-of-bounds access or a compiler miscompile; file a bug with a minimal
     repro if the source looks correct").
  3. Author a regression fixture for each: a concurrent-thread-hits-fallback test proving the mutex is
     not held across the drain window; a `ynz run` invocation against a program that crashes with a
     known signal, asserting the CLI reports the signal, not bare exit 1.
  4. Run the full suite.
- **Exit criteria:** both fixes live; both have regression fixtures; full suite green.
- **Reviewer fan-out:** code-reviewer; docs-consistency reviewer (Golden Rule 11 wording on the new
  signal diagnostic).
- **Model tag:** `(coding, standard, small)`

#### Phase 6b — Sanitizer lane (Miri / ThreadSanitizer / AddressSanitizer) + CI enforcement

- **Task + purpose:** run the `ynz-runtime` crate's own unit tests, plus this milestone's new
  concurrency integration fixtures, under Miri and under ThreadSanitizer/AddressSanitizer — the
  mechanical hunt for exactly the bug classes this milestone fixed by hand (UAF, double-free, data
  races) — and wire a permanent CI job so those classes are hunted on every future push/PR, not just
  this one hotfix. Sequenced after Phases 1, 1b, 3, 3b, 4, and 5 (a real dependency, not mere convenience —
  see Coordinating Instructions): the sanitizers must scan the FIXED runtime code, or their findings
  would just be re-discoveries of bugs already known and scheduled.
- **Steps**
  1. Verify toolchain availability inside the `dev` Docker image before assuming it: confirmed by
     direct read this session — `Dockerfile` installs only the `stable` Rust toolchain via `rustup`
     (`rustup-init.sh -y --default-toolchain stable`); there is no `nightly` toolchain and no `miri` /
     `rust-src` / sanitizer-capable component present. Add them to the `Dockerfile` (in-scope for this
     phase): `rustup toolchain install nightly && rustup component add --toolchain nightly miri
     rust-src` (or the equivalent invocation current at execution time — re-verify the exact `rustup`
     syntax rather than assume it's unchanged), then rebuild the `ynz-dev` image
     (`docker compose build dev`).
  2. Run the Miri lane: `docker compose run --rm dev cargo +nightly miri test -p ynz-runtime`. Record
     every reported UB/leak finding.
  3. Run the same crate + this milestone's concurrency integration fixtures under ThreadSanitizer:
     `docker compose run --rm dev bash -c 'RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test -p
     ynz-runtime --target x86_64-unknown-linux-gnu'` (nightly sanitizer support requires a `-Z`
     unstable flag and, on some toolchains, `-Zbuild-std` — confirm the exact working invocation live
     rather than assume this one is complete; a genuinely-verified working command replaces this one
     in the committed CI job). Repeat for AddressSanitizer (`-Zsanitizer=address`). If either sanitizer
     proves impractically noisy on this codebase (a known false-positive class, e.g. some Tokio runtime
     internals under TSan), record that decision explicitly — never silently drop a lane without a
     named reason.

     **CHECKPOINT** — both sanitizer lanes have been run at least once locally (Miri + TSan, ASan if
     viable) and every raw finding is captured; ready to hand off triage to a fresh dispatch if
     resumed later.
  4. Fix or explicitly triage every finding on the record. A sanitizer finding is a CONFIRMED bug per
     [verification.md](../../../rules/verification.md) — never a theory to wave off. Each finding gets
     either a real fix + regression fixture (in-scope) or a proper four-field
     [no-duct-tape.md](../../../rules/no-duct-tape.md) deferral (WHAT/WHY/COST/TRIGGER) if it is
     genuinely out of this phase's bounded scope — and never a bare "known sanitizer false positive"
     claim without first checking the well-known Rust/Tokio false-positive corpus (e.g. std's own
     documented Miri/TSan exceptions) to confirm it really is one rather than a real bug being waved
     away.
  5. Author the CI job in `.github/workflows/ci.yml`: a new job running the Miri lane + the TSan lane
     (and ASan, unless step 3 recorded a named reason not to) against `ynz-runtime`, on the same
     `push`/`pull_request` triggers as the existing `build-and-test` job.
  6. **Prove the new CI job is non-vacuous** (per M5's FRAGO-005 lesson against vacuous gates — a gate
     that always passes is worse than no gate, because it reads as coverage that isn't there): on a
     throwaway branch, deliberately reintroduce one of the confirmed bug classes this milestone fixed
     (e.g., revert Phase 3's purge, or a synthetic UAF in a scratch test), confirm the new Miri/TSan job
     actually fails RED, then discard the throwaway revert. Commit only the passing, proven-firing CI
     job definition.
  7. Run the full workspace suite (`docker compose run --rm dev cargo test --workspace`); confirm no
     regression to any existing test.
- **Exit criteria:** sanitizer lanes green (or every finding triaged on the record with a proper
  four-field deferral where genuinely out of scope); the new CI job is committed AND proven
  non-vacuous (step 6's throwaway-revert proof); the Dockerfile toolchain addition is committed if it
  was needed; full workspace suite green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (the non-vacuousness proof from step 6
  is real — did the job genuinely fail red on the reintroduced bug, not just get asserted to; is every
  finding genuinely triaged with a named reason, not hand-waved as a false positive?).
- **Model tag:** `(coding, standard, medium)`

#### Phase 7 — Docs/registry honesty sweep (P4-1, P4-2, P2-1 footgun doc, P4-4) + deferral recording

- **Task + purpose:** stop the docs/registry lying about unshipped mechanisms; add the two required
  registry entries; document the bare-channel footgun loudly; correct stale doc tense; record every
  explicit non-fix as a proper four-field deferral.
- **Steps**
  1. `IMP-no-function-coloring.md` "Scheduler Preemption Model" (:214-216): keep the starvation-
     guarantee INTENT text; correct the MECHANISM claim — today's back-edge calls (`emit.rs:12356-
     12365`) invoke a documented no-op stub (`runtime.rs:281-299`, no-op body at :296-299); mark it
     "stub today; real back-edge yield ships in M7, call-site checks re-measured under a real LLVM
     pass pipeline (the 1190% cost that pre-authorized the relaxation was measured at O0)."
  2. Add the `[[deferred_tooling_feature]]` registry entry for the preemption mechanism (name
     `cooperative-preemption-back-edge-yield`; substitute names the current no-op reality; why cites
     the 1190% O0 measurement + the pre-1.0/zero-users acceptance; `ships_in = "v0.3-M7 (optimizer
     milestone) — re-measured under a real LLVM pass pipeline"`; `design_doc =
     "docs/internal/implementation/IMP-no-function-coloring.md"`; `triggers = "M7's optimizer pipeline
     lands and back-edge preemption cost is re-measured under real LLVM passes, OR a real workload
     demonstrates starvation before then."`).
  3. Add the `[[deferred_language_feature]]` registry entry for `background.cpuBound` (substitute
     names today's auto-inference-only reality; why cites `IMP-no-function-coloring.md:247`'s spec-but-
     unimplemented state per `auto-promotion.md`'s override-direction checklist; `ships_in = "TBD —
     pending a real workload demonstrating the auto-inference gets it wrong"`; `design_doc =
     "docs/internal/implementation/IMP-no-function-coloring.md"`; `triggers = "A real workload where CPU-bound
     auto-inference misroutes a task to the I/O scheduler, causing measurable starvation."`); fix any
     present-tense wording implying it already ships.
  4. Extend `IMP-concurrency.md`'s "Design Divergences" section (:840 onward, matching its existing
     WHAT-ships/named-cost/reversal-path pattern) with a new entry documenting the bare-channel
     receive-forever footgun: a bare `channel<T>` never closes because the object retains both
     endpoints for its whole life (`channel.rs:109-123`), so `receive()` after all producers finish
     parks forever; name channel-close semantics as a real, not-yet-designed roadmap item (Future
     Requirements #4), not an M6 deliverable. Check `docs/reference/` for a user-facing channel spec
     file and cross-reference it if one exists; if none exists yet, note that gap too rather than
     silently skipping the user-facing half.

     **CHECKPOINT** — both registry entries (preemption + `background.cpuBound`) authored and the
     bare-channel Design Divergences entry landed; ready to hand off to the doc-tense correction steps
     below in a fresh dispatch if resumed later.
  5. Fix the FFI/`foreign`-keyword present-tense doc text (it is a registered v2+ deferral, not
     shipped).
  6. Fix the `KernelModeRejectsWait` doc text (says unshipped; `check.rs`'s kernel-mode-suspension
     rejection — confirmed live in both the UFCS arm :4384-4392 and the bare-call arm :2930-2937 —
     already implements it).
  7. Record explicitly that P2-6 (auto-Arc unwired) needs NO action this milestone — already correctly
     deferred to v0.4+ with a self-diagnosing registry entry (confirmed this session) — so a reviewer
     doesn't mistake it for a silent gap.
  8. Cross-reference this plan's Future Requirements section (below) for the P2-3, P1-2
     (post-Phase-0), P2-7, and #10 dynamic-dispatch × suspension (FRAGO 002) deferrals — confirm
     each is present with its four fields. P2-5 is NOT a deferral post-Phase-0: it was confirmed
     LIVE and is fixed in Phase 3b (FRAGO 001) — confirm Future Requirements #3 records that
     disposition rather than a stale dormancy note.
- **Exit criteria:** preemption + `background.cpuBound` registry entries live with real four-field
  deferrals; `IMP-concurrency.md` Design Divergences carries the bare-channel entry; FFI +
  `KernelModeRejectsWait` doc text corrected; all Future-Requirements deferrals cross-referenced and
  complete.
- **Reviewer fan-out:** docs-consistency reviewer (diff every edited claim against the audit's own
  citations); design-doc-alignment reviewer (registry entries match `auto-promotion.md`'s checklist).
- **Model tag:** `(coding, standard, small)`

#### Phase 8 — Demo & Error Gallery + roadmap reconciliation + full-suite gate + release handoff

- **Task + purpose:** close the plan-invariants Demo & Error Gallery obligation, reconcile the roadmap
  + Capability Ledger, run the cumulative full-suite gate, and confirm release preconditions.
- **Steps**
  1. Extend `examples/pirates-roster/entrypoint.ynz` with a `wait x.method()` section demonstrating
     UFCS suspension in a realistic context (a Pirate/Ship-domain method performing real work, not a
     bare `print(featureName())`) — real operations only, no invented APIs, per
     [`.claude/rules/dot-postfix.md`](../../../rules/dot-postfix.md)'s examples-must-use-real-operations
     rule. Regenerate + commit the byte-exact golden.
  2. Create `examples/primantis-orders/m6_errors.ynz` with intentional triggers for the new
     compile-time diagnostics this milestone adds (Phase 2's block_on-fallback hard error, if
     constructible from real syntax post-Phase-1 — otherwise document explicitly why it is
     compiler-internal-only and point at Phase 2's own internal regression fixture instead). Note
     explicitly that the `ynz run` signal-death report (Phase 6) is a CLI runtime report, not a
     compile diagnostic, and does not belong in this gallery file — state that distinction with a
     `// WHY:` comment rather than silently omitting it. Wire the new gallery's assertions into
     `crates/ynz-driver/tests/error_galleries.rs` (diagnostic-count + key-phrase convention, matching
     the existing `m{N}_errors.ynz` house style).
  3. **Roadmap reconciliation**: add `v0-3-m6-concurrency-hotfix` to the roadmap's `legacy.milestones`
     frontmatter list; add a `### Milestone 6` section to
     `2026-05-21-v0-3-concurrency-perf/roadmap.md` mirroring the existing per-milestone shape (Value
     delivered / Execution plan / Depends on / Scope / Trigger to schedule / Ships via); add M6's
     fix-list as rows to BOTH existing `## Capability Ledger` sections (the roadmap currently carries
     two — lines 365 and 417, a pre-existing migration-era duplication this plan does not otherwise
     fix, but a new row must land in both so neither goes stale relative to the other) — **including a
     row noting the new Miri/TSan/ASan sanitizer CI lane (Phase 6b) as a delivered, ongoing
     continuous-verification capability**, not just the one-time bug fixes.
  4. Full cumulative gate: `docker compose run --rm dev cargo test --workspace` green; `docker compose
     run --rm dev cargo clippy --workspace -- -D warnings` clean; the byte-exact `pirates-roster`
     golden regenerated + committed; the `m6_errors.ynz` gallery assertions passing; **Phase 6b's
     sanitizer CI job confirmed present and green in `.github/workflows/ci.yml`** (a roadmap claim of a
     "delivered" sanitizer lane with no matching gate check here would itself be the kind of
     doc/reality drift this milestone exists to correct).
  5. Release handoff: per project `CLAUDE.md`'s release workflow, this phase confirms — but does not
     itself execute — the preconditions for `/pr` (if any phase's work isn't yet merged) then
     `/release` to cut the `v0.3.x` patch tag. The release skill is the correct actor for the actual
     cut; this step's job is only to confirm merged-PR state and version-bump readiness are met.
- **Exit criteria:** demo + gallery extended and wired into the test harness; roadmap + both
  Capability Ledger sections updated (including the sanitizer-CI-lane row); full workspace green,
  including Phase 6b's sanitizer CI job confirmed present and green; release preconditions confirmed.
- **Reviewer fan-out:** code-reviewer; docs-consistency reviewer (demo comments, roadmap text);
  adversarial gate-checker (gallery completeness against every new diagnostic this milestone actually
  shipped).
- **Model tag:** `(coding, standard, medium)`

### 3.4 Coordinating Instructions

- **Sequencing**: Phase 0 gates everything. **Phase 1b runs immediately after Phase 1 and BEFORE
  Phase 2 (FRAGO 004 — Patrick-signed sequencing, 2026-07-09: a shipped memory-safety miscompile
  is prioritized ahead of all remaining phases), and it remains a hard prerequisite of Phase 6b
  (the sanitizer lane must scan the FIXED frame-backing).** Phase 1 → Phase 2 is a hard dependency
  (Decision D1) — do not start Phase 2 before Phase 1's carved fixture set (a/c/d) is GREEN, and
  per the signed sequencing not before Phase 1b closes fixture (b) (the full class GREEN).
  Phases 3, 4, 5 are independent of each
  other and of Phases 1–2 (different subsystems); they are sequenced 3→4→5 for one conductor's
  convenience, not a hard dependency — a FRAGO reordering them is not a plan violation. **Phase 3b (FRAGO 001)
  is sequenced immediately after Phase 3** — same `runtime.rs:591-693` drop-ladder region, minimizing
  merge collision — **and is a hard prerequisite of Phase 6b** (the sanitizer lane must scan the FIXED
  recursion-chain cleanup path, same rationale as Phases 1/3/4/5). Phase 6 is
  independent of everything. **Phase 6b (sanitizer lane) has a real dependency, not mere convenience:
  it must run AFTER Phases 1, 1b, 3, 3b, 4, and 5 land**, because Miri/TSan/ASan need to scan the FIXED runtime
  code (the UFCS threading, the shape-arg frame-backing fix — the sanitizer lane must scan the
  FIXED frame-backing, per FRAGO 004 — the ABA purge, the recursion-chain spike-handle cleanup,
  the lost-wakeup reorder, the drop-glue mechanism) — scanning
  the pre-fix state would only re-discover bugs already known and scheduled. It is independent of
  Phase 6 itself; sequenced 6→6b for one conductor's convenience. Phase 7 should follow Phase 0's
  dormancy verdicts (so the Future Requirements it cross-references are settled) AND follow Phase 6b
  (so the honesty sweep's own text, and Phase 8's roadmap reconciliation, can truthfully say the
  sanitizer CI lane is live rather than still-pending) — logically it follows all the fix phases (so
  its honesty sweep reflects the ACTUAL post-fix state). Phase 8 is last.
- **CCIR-1 (re-verify citations)**: every phase re-verifies this plan's file:line citations against
  the live tree at dispatch time before acting on them — the audit and this plan's own recon are
  this-session-accurate as of 2026-07-04 but WILL drift by execution time (per the Weather note on
  execution being gated behind the M5 merge).
- **CCIR-2 (no second derivation)**: if any phase's executor finds itself computing "does this suspend
  / is this token valid / does this element need drop-glue" a SECOND way anywhere, STOP and thread the
  authoritative source instead — never invent a parallel derivation to unblock. Surface it if the
  authoritative source isn't reachable from where it's needed.
- **CCIR-3 (execution-gate precondition)**: if Phase 0 finds the M5-merge-then-tag precondition has NOT
  actually happened, STOP — do not begin Phase 1 or any later phase.
- **CCIR-4 (scope growth from Phase 0)**: if Phase 0's dormancy verdicts find P1-2 or P2-5 LIVE (not
  dormant), route the fix through the plan-amendment + FRAGO seam before doing the fix work — never
  silently fold it into an existing phase.
- **CCIR-5 (HIGH residual surfacing)**: if any phase's own verification proves a mitigation doesn't
  actually hold (a RED fixture that won't go GREEN, a parity gate that stays non-zero, a race that
  reproduces even after the fix), STOP, re-score the risk, and if it lands HIGH draft the RISK
  OVERRIDE block with the work shown — never sign it; route to the conductor.
- **Verify-before-complete gate**: every phase's exit criteria require a COMMITTED fixture or gate
  artifact, never a narrated "should work now" — per [verification.md](../../../rules/verification.md).

## 4. Sustainment

- **Build/test**: `docker compose run --rm dev cargo build --workspace`,
  `docker compose run --rm dev cargo test --workspace`,
  `docker compose run --rm dev cargo clippy --workspace -- -D warnings` — no `-it`, no host-native
  cargo.
- **Alloc-parity tooling**: `YNZ_ALLOC_COUNTER_OUTPUT` (existing house convention, reused from M5)
  for Phase 5's non-vacuous parity gate.
- **Sanitizer tooling (Phase 6b)**: `docker compose run --rm dev cargo +nightly miri test -p
  ynz-runtime` (Miri lane); `RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test -p ynz-runtime
  --target x86_64-unknown-linux-gnu` (ThreadSanitizer, run inside the container) and the AddressSanitizer
  analog (`-Zsanitizer=address`). Requires a `nightly` toolchain + `miri`/`rust-src` components, which
  the phase adds to the `Dockerfile` if not already present (confirmed absent this session — the image
  currently installs `stable` only).
- **Fixtures**: `crates/ynz-typeck/tests/`, `crates/ynz-codegen/tests/`, `crates/ynz-runtime/` unit
  tests, `crates/ynz-driver/tests/integration.rs` (byte-exact `pirates-roster` golden),
  `crates/ynz-driver/tests/error_galleries.rs` (the new `m6_errors.ynz`).
- **No new external dependencies, no new env vars beyond what already exists** (`YNZ_ALLOC_COUNTER_OUTPUT`,
  `YNZ_NO_AUTO_PARALLEL`) — if Phase 3's ABA repro genuinely needs a test-only address-reuse-forcing
  knob, it is internal-test-only (mirroring `YNZ_SOA_FORCE`'s house convention from M5), never
  user-facing syntax. **Exception, named explicitly**: Phase 6b adds a `nightly` Rust toolchain +
  `miri`/`rust-src` components to the Docker dev image and to CI — a dev/CI-time TOOLCHAIN addition,
  not a new runtime dependency or env var; it never ships in any release build (see Runtime
  Dependencies below).

## 5. Command & Signal

- **Ownership**: per-phase, assigned to the dispatched executor at conductor dispatch time (Model tag
  → `REF-model-selection.md` lookup, mechanical).
- **Succession**: `plan-id` (`2026-07-04-v0-3-m6-concurrency-hotfix`) + the `session-id` chain in this
  file's frontmatter + checkbox/step state in ¶3.3 is the resumable truth. A checkpointing executor in
  Phase 1 writes `handoff-phase-1.md` per [REF-plan-format.md](../../../../docs/reference/REF-plan-format.md)'s
  handoff-file convention.
- **Audit trail**: `audit.md` (sidecar, created at first execution session — not created by this
  producer dispatch) carries the session log + FRAGO log per the frozen template.

## Invariants This Milestone Must Preserve

### Safety

- No `wait x.method()` call is silently lowered as a synchronous blocking call — testable via
  Phase 1's RED→GREEN fixture class asserting suspend-correct behavior at all 9 predicate sites
  (the original 4 + the 5 coupled, per FRAGO 003; fixtures (a)/(c)/(d) close in Phase 1, fixture
  (b) closes in Phase 1b per FRAGO 004's carve-out).
- No shape (or `fixed<T>`) aggregate argument to a suspending callee is left in a dying stack
  alloca across suspension — it is frame-backed via the one `shape_embed` crossing classifier
  (the single authoritative crossing classifier, extended — never a second frame-layout path);
  the child's `self` points into the surviving frame, proven by a deterministic-across-runs
  RED→GREEN repro (Phase 1b, FRAGO 004).
- No suspending call reaches the block_on fallback (`emit.rs:15122-15137`) from a non-designated
  caller without a compile-time hard error (Phase 2).
- No cancelled task's `pending_sends` entry survives its cancellation, from EITHER token producer
  (frame-ptr conduit tokens or handle-ptr tokens) — the purge is unconditional and idempotent at
  both cancellation paths (the drop ladder and `ynz_handle_free`) via one shared helper (Phase 3).
- No token-producer address reuse — a freed FRAME address OR a freed HANDLE address — can
  resurrect a dead task's send under a live task's identity — the generation-salted `caller_token`,
  minted from one shared salting scheme covering both producers, closes the ABA window for both
  (Phase 3).
- No recursion-chain child's spike `CpuJoinHandle`s survive its cancellation — the recursion-chain
  drop walk frees each child's spike CPU handles via the same authoritative `cleanup_spike_cpu_handles`
  choke point the root frame uses, proven by Phase 3b's RED→GREEN alloc=free (or handle-count) parity
  fixture (Phase 3b).
- No multi-consumer receive can be lost to the register/poll race window (Phase 4), and P3-4's
  existing "no lock held across a blocking poll" clean bill is re-verified, not merely assumed, after
  the reorder.
- No buffered channel element leaks at channel drop — alloc=free parity, proven non-vacuously
  (Phase 5).
- `ynz_rt_shutdown` never holds the RUNTIME mutex across its up-to-5s drain window (Phase 6).
- A signal-terminated child process is reported as a signal death, never masked as bare exit code 1
  (Phase 6).
- The `ynz-runtime` crate is Miri-clean and clean under ThreadSanitizer/AddressSanitizer, with every
  exception explicitly triaged on the record — CI-enforced going forward via the dedicated,
  proven-non-vacuous sanitizer job (Phase 6b).

### Performance

- No new O(n²)-or-worse pass is introduced: Phase 1's UFCS threading reuses typeck's ALREADY-computed
  `sig_table.fns.get(method).suspends` resolution — an O(1) lookup per call site, not a new fixpoint.
- The generation-salted token (Phase 3) adds one integer compare at send/recv time — negligible, no
  BigO change.
- The channel drop-glue walk (Phase 5) is O(buffered-element-count) — the honest cost of correctly
  freeing what a leak previously left unfreed, not a regression against any prior CORRECT baseline (a
  leak was O(1)-but-wrong; correctly freeing is O(n)-and-right).
- **Auto-promotion analysis (mandatory per `.claude/rules/auto-promotion.md`): no NEW auto-promotion
  candidate this milestone.** M6 is a correctness/liveness/leak/honesty hotfix — it restores designed
  behavior (UFCS suspension, the preemption guarantee's documented intent) rather than introducing a
  new stricter/faster form the compiler could prove fits. The one auto-promotion-ADJACENT item —
  `background.cpuBound`'s override direction — is EXISTING design (CPU-bound auto-inference already
  shipped, `IMP-no-function-coloring.md:238`); this milestone only adds the missing registry deferral entry for
  its override form (Phase 7), per `auto-promotion.md`'s checklist obligation to document deliberate
  omissions explicitly rather than silently skip them. Stated here so reviewers know it was
  considered, not forgotten.

### Teaching

- The block_on-fallback hard error (Phase 2) follows WHAT/WHAT-INSTEAD/WHY per Golden Rule 11.
- The `ynz run` signal-death report (Phase 6) follows WHAT/WHAT-INSTEAD/WHY, naming the signal and the
  likely cause.
- No new banned-jargon words are introduced (audited by the existing `tests/jargon_audit.rs`
  convention — unchanged by this milestone).
- The bare-channel non-closure footgun gets loud, explicit user-facing documentation (Phase 7) — a
  liveness footgun a user could hit today with zero compiler warning.

### Runtime Dependencies

- All 8 fix items operate entirely within the already-shipped Tokio-backed runtime (`libynz_rt.a`) —
  no new runtime dependency is introduced by this milestone.
- The channel drop-glue mechanism (Phase 5) depends on the existing heap allocator (malloc) — the same
  dependency channels already have; no new kernel-mode-relevant dependency is added.
- Phase 6b's Miri/TSan/ASan sanitizer lane is a dev-time/CI-time verification tool only — it adds NO
  runtime dependency to the shipped compiler or its `libynz_rt.a`; the `nightly` toolchain + sanitizer
  components live only in the Docker dev image and the CI job, never in a release build. Stated
  explicitly so reviewers know it was considered, not forgotten.

### Kernel-Mode Behavior

- `--kernel` mode already rejects `wait`/`background`/`channel<T>` entirely (confirmed live this
  session, `check.rs`'s kernel-mode-suspension-rejection arms); none of M6's fixes touch that gate —
  every fix item lives behind the Tokio runtime path, which never runs in kernel mode, so no new
  kernel-mode compile-error surface is needed.
- The preemption honesty fix (Phase 7, docs-only) does not change kernel-mode behavior — kernel mode
  has no scheduler to preempt in the first place.

### Demo & Error Gallery

- `examples/pirates-roster/entrypoint.ynz` gains a `wait x.method()` section demonstrating UFCS
  suspension in a realistic context (Phase 8 step 1); byte-exact golden regenerated + committed.
- `examples/primantis-orders/m6_errors.ynz` is created with WHY-commented triggers for every new
  compile-time diagnostic this milestone adds, wired into `crates/ynz-driver/tests/error_galleries.rs`
  (Phase 8 step 2). The `ynz run` signal-death report is explicitly noted as NOT belonging in this
  gallery (it is a CLI runtime report, not a compile diagnostic) rather than silently omitted.

### Feature Registry Entries

- New `[[deferred_tooling_feature]]`: `cooperative-preemption-back-edge-yield` — the preemption
  mechanism's stub-to-real deferral (Phase 7 step 2), `ships_in = "v0.3-M7"`,
  `design_doc = "docs/internal/implementation/IMP-no-function-coloring.md"`.
- New `[[deferred_language_feature]]`: `background.cpuBound` — the CPU-bound explicit-override form
  (Phase 7 step 3), `ships_in = "TBD — pending a real workload demonstrating the auto-inference gets it
  wrong"`, `design_doc = "docs/internal/implementation/IMP-no-function-coloring.md"`.
- Phase 2 must check whether `emit.rs:11162`'s existing `[[diagnostic_template]]` covers the new
  block_on-fallback hard-error site; reuse it if the shape matches, or add a new template entry if it
  genuinely doesn't — recorded via FRAGO if new, never a silent scope addition.
- No new keywords, banned-jargon words, primitive intrinsics, or type-attached constants — M6 is a
  compiler-internal correctness/leak/honesty hotfix with zero user-facing language-surface changes,
  including Phase 6b's sanitizer lane (compiler-internal CI/dev tooling, not a language feature).
  Stated explicitly so reviewers know every registry-entry-kind was considered, not skipped.

## Design-Doc Alignment

**Cited governing docs:**
[`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
(Scheduler Preemption Model :214-238; `background.cpuBound` :247; the no-coloring invariant :256-266) ·
[`IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) (Design Divergences
:838-876) ·
[`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) ·
[`auto-promotion.md`](../../../rules/auto-promotion.md) ·
[`no-duct-tape.md`](../../../rules/no-duct-tape.md) ·
[roadmap `2026-05-21-v0-3-concurrency-perf`](../2026-05-21-v0-3-concurrency-perf/roadmap.md).

**Confirmation — this plan's model matches the cited docs, with the following divergences enumerated:**

1. **`IMP-no-function-coloring.md:214-216` says** preemption check points at function call sites AND
   loop back-edges are "locked" (i.e., implemented); **this plan does NOT implement real back-edge
   yield** — it corrects the doc's mechanism claim to "stub today" and adds the missing
   `[[deferred_tooling_feature]]` registry entry, deferring the real implementation to M7 (the
   optimizer milestone). Reason: the 1190% O0 call-site-cost measurement (M5's spike) means
   implementing it for real before the optimizer pipeline exists pays that cost with zero offsetting
   benefit; this is a documentation-honesty fix, not an implementation fix, and the roadmap already
   pre-authorized the relaxation (just never wrote it back to the doc or the registry) — Patrick-signed
   disposition per the brief.
2. **`IMP-no-function-coloring.md:247` says** `background.cpuBound process(data)` is the explicit override
   ("final naming TBD"); **this plan does not implement it** — it adds the missing
   `[[deferred_language_feature]]` registry entry per `auto-promotion.md`'s override-direction
   checklist (a deliberate, now-documented omission) rather than picking a final name and building it.
   Reason: Patrick-ratified disposition per the brief — the auto-inferred common case already covers
   today's needs; implementing an unused override form now would be speculative (YAGNI) ahead of a
   real workload demonstrating the need.
3. **No governing doc currently documents the bare-channel non-closure footgun** (P2-1) as a
   user-facing behavior — this plan ADDS it to `IMP-concurrency.md`'s Design Divergences section
   (Phase 7) rather than treating its absence as a gap to silently carry forward. Reason: per the
   brief's disposition — document the footgun loudly now; channel-close semantics themselves are a
   real, undesigned roadmap item, out of this hotfix's scope.
4. **Milestone-boundary assumption**: this plan depends on the v0.3-M5 auto-SoA merge + tag landing
   BEFORE any phase executes (Weather). That dependency is handled elsewhere (per the brief) and is
   not invented by this plan — it is the stated precondition for the `emit.rs` collision risk (R8)
   staying LOW rather than live.
5. **Behavior claims about untouched adjacent code, recon-cited per plan-invariants §Design-Doc
   Alignment (4)**: P2-6 (auto-Arc unwired) needing no action is a claim re-verified this session
   against the live `registry/features.toml` entries (`auto-arc-cautionary-tint`,
   `auto-arc-codegen-emission`) — confirmed, not inferred. `KernelModeRejectsWait` being already
   implemented is confirmed by direct read of `check.rs`'s two kernel-mode-suspension-rejection arms
   (:4384-4392 UFCS, :2930-2937 bare call) this session, not carried forward from the audit's own
   citation unread. P3-4's "no lock-ordering inversions" clean bill is EXPLICITLY re-verified in Phase
   4 after the lost-wakeup fix (not assumed to survive the change untouched) — named directly in that
   phase's exit criteria.

### Recorded Decisions (durable calls made at plan time, per decision-philosophy — reasons on the record)

- **D1 — Phase 2 (block_on-fallback hard-error guard) is sequenced immediately after Phase 1** (UFCS
  fix), diverging from the audit's raw synthesis numbering (which interleaves P3-1/P3-2 before P4-3).
  Reason: the guard's "assert unreachable for non-main callers" correctness claim depends on P1-1
  actually being fixed; verifying the guard before P1-1 lands would be verifying an assertion against
  a precondition known to be false.
- **D2 — both purge-on-cancellation AND generation-salted-token are implemented for the ABA/orphan
  fix, across BOTH token-producer sites** (frame-ptr conduit tokens `emit.rs:11651-11654` AND
  handle-ptr tokens `handle.rs:326` — per Fable's personal plan-audit "P3-1 ADDENDUM") (Phase 3), not
  either/or and not first-producer-found-only. Reason: purge closes the steady-state
  leak/resurrection; the salt is defense-in-depth against any residual race window between
  cancellation firing and the purge completing; per authoritative-derivation.md, both mitigations
  thread ONE scheme to every producer rather than patch only the first one found — consistent with
  the Mission's "production-grade, no duct tape" intent.
- **D3 — the buffered-channel drop-glue mechanism (Phase 5) is registered at channel CONSTRUCTION**,
  not as a separate drain-on-last-free path. Reason: both approaches need per-element drop knowledge
  regardless; registering once at construction is the single authoritative choke point
  (authoritative-derivation.md), avoiding a second ad hoc drop path P2-3's eventual fix would have to
  reconcile with later.
- **D4 — P2-3 (closed-send leak) stays deferred** even though Phase 5 builds the exact drop-glue
  mechanism it would eventually need. Reason: the closed1/closed2 codegen path is structurally
  UNREACHABLE until channel-close semantics ship (P2-1's finding) — wiring it now would exercise dead
  code no test can reach, i.e., speculative work the YAGNI ceiling forbids.
- **D5 — P1-2, P2-5, AND the dynamic-dispatch × suspension coverage question all get a Phase-0
  verification gate before any fix-vs-defer (or covered-vs-gap) decision.** Reason: per
  decision-philosophy's mandatory-assessment step — verify, don't assume dormant or covered, even
  though the audit's own THEORY framing suggests P1-2/P2-5 dormancy is likely and the UFCS fix's own
  design suggests the dynamic-dispatch question MIGHT already be covered once the shared resolution is
  threaded — assume nothing without reading the actual code.
- **D6 — P2-7 (`handle_recv_poll` panic-then-pending hang) is added to Future Requirements as a
  NEWLY-SURFACED finding, not in the brief's explicit 9-item scope list.** Reason: per the risk-engine's
  input-set union rule, an agent-found risk is never silently dropped just because the human-provided
  scope list didn't name it — it is recorded as a deferral (out of THIS hotfix's named scope), not
  silently fixed (scope creep) or silently ignored (the violation this rule exists to prevent).
- **D7 — Severity II-Critical is used for the silent-miscompile-class risk rows (R1, R2, R4)**,
  consistent with this project's own established scoring convention (M5 scored the identical
  twin-derivation/silent-miscompile shape at Sev II even pre-1.0/zero-users), because the real cost —
  multi-round whack-a-mole debugging, per M3a's ~10-round precedent — is genuine engineering cost, not
  a cosmetic shrug just because there are no external users yet.

## Future Requirements / Revisit

1. **P2-3 — closed-send drop-glue leak** (`emit.rs:~11833-11960` closed1/closed2 blocks drop no
   `value_bits`). WHY deferred: structurally unreachable in production until channel-close semantics
   ship (P2-1's finding — a bare channel never closes today). COST to fix later: small once
   channel-close semantics land — reuses Phase 5's drop-glue fn-ptr mechanism directly. TRIGGER:
   channel-close semantics ship (see item 4 below).
2. **P1-2 — twin type-walkers** (`emit.rs:8276` vs `emit.rs:8364`). WHY deferred: Phase 0 verifies
   dormancy first; if confirmed dormant (both frame-layout call sites filter `generics.is_empty()`),
   unifying the walkers is cleanup, not a live bug. COST to fix later: ~0.5 session (fold the two
   walkers behind one shared resolution). TRIGGER: a crossing local's raw type resolves to an
   unsubstituted generic in SM-resume context, OR the next milestone touching generics+suspension
   together.
3. **P2-5 — recursion-chain × spike cleanup gap** (`runtime.rs:591-693`, root-frame spike-handle
   cleanup vs recursion-chain-children sleep-handle-only cleanup). **Confirmed LIVE — fixed in
   Phase 3b** (Phase 0, 2026-07-09, falsified assumption A4: no mutual-exclusion gate exists for a
   nested branch-arm CPU group in a zero-param self-recursive suspending host; FRAGO 001 routed the
   fix to the new Phase 3b, which closes the leak via the shared `cleanup_spike_cpu_handles` choke
   point). No longer a deferral — this entry is retained so the audit-finding numbering (P2-5)
   resolves to its actual disposition rather than a stale "verify dormancy first" note.
4. **Bare-channel end-of-stream / channel-close semantics** (P2-1's underlying design gap — the
   footgun is documented loudly per Phase 7, but the FEATURE is not designed). WHY deferred: this is a
   real design question (what does `.close()` look like; does last-sender-drop auto-close given the
   channel object itself holds a Sender?) — out of this hotfix's scope by the brief's explicit
   disposition. COST to fix later: unknown — needs its own design pass before a cost estimate is
   honest. TRIGGER: a real user/workload needs bounded-lifetime channel consumption, or before any
   production-representative concurrency use case ships.
5. **Preemption real back-edge yield** — registry entry `cooperative-preemption-back-edge-yield`
   (Phase 7). WHY deferred: 1190% O0 call-site cost (M5 spike); no offsetting benefit until the
   optimizer pipeline exists. COST to fix later: implementation-sized, folded into M7's scope. TRIGGER:
   M7's optimizer pipeline lands and the cost is re-measured under real LLVM passes.
6. **`background.cpuBound` explicit override** — registry entry (Phase 7). WHY deferred: no real
   workload has yet demonstrated the auto-inference gets CPU-bound routing wrong; building an unused
   override is speculative. COST to fix later: small (naming + one typeck/codegen surface once a real
   need is named). TRIGGER: a real workload where auto-inference misroutes a CPU-bound task, causing
   measurable starvation.
7. **P2-7 — `handle_recv_poll` panic-then-pending hang** (`handle.rs:297-303`, newly surfaced this
   session — not in the brief's 9-item scope). WHY deferred: out of this hotfix's named scope; fixing
   it correctly likely wants the SAME register-before-poll discipline Phase 4 just applied to
   `channel.rs`, so it is a natural, low-cost follow-on rather than urgent standalone work. COST to fix
   later: small (~0.5 session, mirrors Phase 4's fix shape). TRIGGER: a real hang reproduces in
   practice, or the next milestone touching `handle.rs`'s poll path.
8. **The `## Capability Ledger` section duplication in the roadmap** (two headings, lines 365 and 417,
   pre-existing migration artifact — noted, not fixed, by this plan; Phase 8 adds M6's row to both so
   neither goes stale relative to the other). WHY deferred: out of this hotfix's charter; a
   documentation-hygiene item, not a bug. COST to fix later: small (merge the two sections into one).
   TRIGGER: the next roadmap-editing session, or Patrick's explicit call to clean it up.
9. **Roadmap ledger row 441 — codegen ICE: bare int literal into a `number`-typed slot crashes the
   compiler (ELEVATED priority)** — this plan explicitly DECLINES it; it stays unclaimed between M6 and
   M7 rather than being silently picked up here. WHAT: `store`/`store_field`'s `Type::Number` arm
   assumes a decimal128-pointer representation while `Expr::IntLit` lowers to a raw `i64`; typeck admits
   the coercion; codegen panics on common valid code (e.g. `let x: number = 5`). WHY declined: this is
   NOT a concurrency-audit finding — it is a pre-existing literal-lowering bug orthogonal to M6's
   confirmed concurrency-race/leak/honesty charter; mixing an unrelated ICE fix into a hotfix milestone
   widens this plan's blast radius for no charter-aligned benefit (M7's own Future Requirements #3
   independently reached the same non-absorption verdict — "likely belongs to v0.3-M6 or a dedicated
   hotfix" — but neither plan claims it, so it is named here rather than left to fall through the gap
   between the two). COST to fix later: unchanged from the roadmap ledger's own estimate — ~0.5-1
   session (expected-type-aware `Expr::IntLit` branch, or typeck-level int→number coercion; its own
   small design + call-site audit). TRIGGER: Gate-4 conversation — Patrick assigns row 441 a home
   (flagged to him explicitly; not auto-claimed by either M6 or M7).
10. **Dynamic-dispatch × suspension predicate blindness** (`check.rs` `check_follows_contracts`
    never reads `suspends`; the four suspension predicates — `may_block.rs` call-graph,
    `cpu_admission.rs`, `emit.rs` `collect_callees_in_expr` + `is_direct_suspending_call` — are all
    MethodCall-blind for the vtable-resolved `dynamic Contract` form, the same shape as P1-1's UFCS
    gap). **WHY deferred:** every `dynamic Contract` call site hard-errors at codegen today
    (`emit.rs:14622-14625`, "not yet lowered in M4 P4") — zero live exposure (a loud compile error,
    never a silent mis-suspension), so no reachable test can exercise a fix; coding the predicate
    threading now is speculative work against dead code, the same YAGNI-ceiling shape D4/P2-3
    already names. **COST to fix later:** small — reuses Phase 1's shared authoritative-resolution
    threading directly; should land in the SAME future phase that lowers `dynamic Contract`
    codegen, not as a separate follow-on. **TRIGGER:** `dynamic Contract` call-site codegen
    lowering ships (the remaining M4 P4 work — owning milestone TBD, flagged to Patrick at Gate-4
    rather than left "someday"). (FRAGO 002 — deferral-with-trigger per the D4/P2-3 precedent, not
    a fix phase.)
