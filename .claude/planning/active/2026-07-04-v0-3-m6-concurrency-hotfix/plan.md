---
name: "v0-3-m6-concurrency-hotfix"
plan-id: "2026-07-04-v0-3-m6-concurrency-hotfix"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-producer-2026-07-04-m6", "plan-producer-2026-07-04-m6-amend1", "plan-producer-2026-07-04-m6-amend2", "plan-producer-2026-07-04-m6-amend3", "conductor-2026-07-09-m6-exec", "executor-2026-07-09-m6-phase0", "executor-2026-07-09-m6-phase0b-frago", "executor-2026-07-09-m6-phase1", "executor-2026-07-09-m6-phase1-seg2", "executor-2026-07-09-m6-phase1-seg3", "executor-2026-07-09-m6-phase1-seg4", "executor-2026-07-09-m6-frago004", "executor-2026-07-09-m6-phase1b", "executor-2026-07-09-m6-phase1b-seg2", "executor-2026-07-09-m6-phase1b-seg3", "executor-2026-07-09-m6-phase1b-seg4", "executor-2026-07-09-m6-phase1b-seg7", "conductor-2026-07-10-m6-exec2", "executor-2026-07-10-m6-phase1b-fixloop1", "executor-2026-07-10-m6-frago008-012", "executor-2026-07-10-m6-phase1c-seg1", "executor-2026-07-10-m6-phase1c-seg2", "executor-2026-07-10-m6-phase1c-seg3", "executor-2026-07-10-m6-phase1c-seg4", "executor-2026-07-10-m6-phase1c-seg5", "executor-2026-07-10-m6-phase1c-seg6", "executor-2026-07-10-m6-phase1c-seg7", "executor-2026-07-10-m6-frago015", "executor-2026-07-10-m6-phase1d", "executor-2026-07-10-m6-phase1d-seg2", "executor-2026-07-10-m6-phase1d-seg3", "executor-2026-07-10-m6-phase1d-fixloop1", "executor-2026-07-10-m6-phase1d-fixloop2", "executor-2026-07-10-m6-phase1d-fixloop3", "executor-2026-07-10-m6-phase1d-fixloop3-seg2", "executor-2026-07-10-m6-phase1d-fixloop3-seg3", "executor-2026-07-10-m6-phase1d-fixloop4", "executor-2026-07-10-m6-phase2", "executor-2026-07-10-m6-phase2-fixup", "executor-2026-07-10-m6-store-site-stopgap", "executor-2026-07-10-m6-store-site-stopgap-fixloop1", "executor-2026-07-10-m6-phase3-seg3", "executor-2026-07-10-m6-phase3-fixloop1", "executor-2026-07-10-m6-phase3b-seg1", "executor-2026-07-10-m6-phase3b-seg2", "executor-2026-07-10-m6-phase3b-fixloop1", "executor-2026-07-10-m6-phase3c", "executor-2026-07-10-m6-phase3c-fix1", "executor-2026-07-10-m6-phase3c-fix2", "executor-2026-07-10-m6-phase3c-polish", "m6-fr24-crossplan-lift-2026-07-11", "executor-2026-07-11-m6-phase4", "executor-2026-07-11-m6-phase4-fixloop1", "executor-2026-07-11-m6-phase4b", "executor-2026-07-11-m6-phase5-seg1", "executor-2026-07-11-m6-phase5-seg2", "executor-2026-07-11-m6-phase5-seg3", "executor-2026-07-11-m6-phase5-frago028", "executor-2026-07-11-m6-phase5b", "executor-2026-07-11-m6-phase5b-nits"]
created_at: "2026-07-04"
updated_at: "2026-07-11"
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
- **P3-1/P2-2 — `caller_token` ABA + orphaned `pending_sends`.** `emit.rs:12205-12208` computes the
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
  conduit tokens minted at `emit.rs:12205-12208`; handle-ptr tokens minted inside
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
  closed-send) is confirmed UNREACHABLE via any compiled program today — same structural shape as
  P2-3, closed by the drop-story milestone rather than channel-close semantics (FRAGO 027: no codegen
  path ever releases the creator's channel reference, so `YnzChannel`'s last-ref drop never fires from
  compiled Yinz; the Phase 5 drop-glue mechanism is built and proven at the runtime-ABI level, folded
  into Future Requirements #13/#17 for the E2E path).
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
real multi-round engineering debugging, not a cosmetic shrug, even pre-1.0/zero-users). **Two signed
HIGH residuals exist in this table — R13, the shape-arg frame-backing UAF surfaced mid-execution by
FRAGO 004, and R14, the wider arg-UAF class (number/maybe/union + anonymous struct-literal args)
surfaced by Phase 1b's post-fix residual probes via FRAGO 006; their RISK OVERRIDE blocks (below the
table) are the plan's two signed HIGH-residual overrides, each accepted by Patrick 2026-07-09 —
R13 with Phase 1b as the trigger-to-close, R14 with Phase 1b (number half) + Phase 1c
(maybe/union + anonymous-aggregate half, per FRAGO 007) as the trigger-to-close.** All other residuals are
MEDIUM-or-below; MEDIUMs are recorded here and parked with triggers in Future Requirements.

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **R1 — UFCS-fix regression of existing (Call-based) suspension classification** (threading the authoritative resolution into 4 sites could break an already-working path) — *Phases 1–2* | C | II | H | Adversarial/RED-repro test class authored BEFORE the fix, gating the build — transitive UFCS, explicit `wait` on `MethodCall`, mixed UFCS+`Call`, `background`-spawned UFCS, PLUS full regression run of every pre-existing Call-based suspension fixture (**B2**, prob −1; proof: committed RED→GREEN fixture set, Phase 1 step 2/7) | **M** (D×II) | recorded |
| **R2 — pending_sends purge / token-salt fix is itself incomplete or racy on EITHER token producer (frame-ptr conduit tokens `emit.rs:12205-12208` OR handle-ptr tokens `handle.rs:326`)** — *Phase 3* | C | II | H | Adversarial/RED cancellation-during-send repro covering BOTH producers (frame-path AND handle-path backpressure + cancel + address-reuse simulation) gating the build, PLUS idempotency requirement on the purge at BOTH cancellation paths — the drop ladder AND `ynz_handle_free` (double-cancel is a safe no-op on either) (**B2**, prob −1; proof: committed RED→GREEN fixture pair, Phase 3 step 6) | **M** (D×II) | recorded |
| **R3 — lost-wakeup fix reorder introduces a new lock-ordering issue** — *Phase 4* | D | II | M | Adversarial multi-consumer RED repro gating the build, PLUS re-verification that P3-4's existing "no lock held across a blocking poll" clean bill still holds after the reorder (**B2**, prob −1; proof: committed fixture + re-verified clean-bill note, Phase 4 step 3/4) | **L** (E×II) | pass |
| **R4 — drop-glue ABI change (channel construction) miswires or under-covers elements** — *Phase 5* | C | II | H | `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity gate with NON-VACUOUS coverage (buffered heap-typed elements exercised, per M5's FRAGO-005 lesson against a vacuous zero-alloc pass) gating the build (**B2**, prob −1; proof: committed parity test + baseline, Phase 5 step 4) | **M** (D×II) | recorded |
| **R5 — shutdown mutex re-scope introduces a new race** — *Phase 6* | D | III | L | Mechanical mirror of the already-correct sibling pattern (`ynz_rt_run_entrypoint:995-1006`); no further mitigation needed | **L** (D×III) | pass |
| **R6 — `ynz run` signal-report change breaks existing exit-code-consuming callers** — *Phase 6* | D | IV | L | Purely additive diagnostic improvement (signal name added to the report; normal-exit and non-signal-failure paths unchanged) | **L** (D×IV) | pass |
| **R7 — docs/registry honesty sweep introduces a NEW factual drift** — *Phase 7* | D | IV | L | docs-consistency reviewer diffs every edited claim against the audit's own citations before merge | **L** (D×IV) | pass |
| **R8 — `emit.rs` merge collision between M5 and M6** — *Weather precondition* | E | III | L | Structural: M6 branches from `main` only AFTER the M5 merge + tag (Patrick-signed sequencing decision) — the collision window is eliminated by construction, not mitigated after the fact | **L** (E×III) | pass |
| **R9 — Phase-0 verification finds P1-2 and/or P2-5 are NOT dormant, OR finds a GAP in dynamic-dispatch × suspension coverage** (scope growth mid-plan) — *Phase 0* | C | III | M | This is the explicit PURPOSE of the Phase-0 gate — verify before deciding fix-vs-defer (or covered-vs-gap), per decision-philosophy's mandatory-assessment step; a non-dormant OR GAP finding routes through the plan-amendment + FRAGO seam (a GAP verdict adds the fix to Phase 1's scope, since it shares the same authoritative-resolution threading), never a silent scope change | **M** (C×III) | recorded — trigger: Phase 0 verdict itself |
| **R10 — demo/gallery/registry/roadmap reconciliation mechanical additions** — *Phase 8* | D | IV | L | Mechanical, docs-consistency + code-reviewer fan-out | **L** (D×IV) | pass |
| **R11 — P2-7 `handle_recv_poll` panic-then-pending hang** (un-deferred by user-directed Mission-scope and FIXED, not deferred — FRAGO 010) — *Phase 4b* | D | III | L | RED-repro (panic-before-registration) + register-before-poll fix mirroring Phase 4's discipline (**B2**, prob −1; proof: committed RED→GREEN repro, Phase 4b step 2/3) | **L** (E×III) | pass |
| **R12 — Sanitizer lane (Miri/TSan/ASan) surfaces a new confirmed bug beyond this phase's own immediate fix capacity** — *Phase 6b* | B | II | H | This phase's own existence is the engineered, bounded catch: any genuine new finding is triaged/routed through the plan-amendment + FRAGO seam before any release — never silently shipped, never silently dropped — inside a pre-1.0/zero-public-users/fully-git-reversible codebase, so a finding's real-world consequence drops from "would-be production Critical" to "caught, triaged, and fixed-or-properly-deferred pre-release" (**B2**, severity, −1 level; proof: the phase's own exit criteria — every finding triaged on the record — plus the Weather section's git-reversible/zero-users precondition) | **M** (B×III) | recorded |
| **R13 — shape-arg frame-backing UAF: a shape-value argument to a suspending callee stages a dangling stack pointer across Pending — confirmed silent-garbage miscompile, pre-existing in shipped v0.3.0, reproduces in pure `Call` form + auto-inserted transitive suspension, likely also `fixed<T>`** (FRAGO 004) — *Phase 1b* | A | II | EH | RED-repro + full-regression gate (**B2**, prob −1; proof: Phase 1b's committed deterministic RED→GREEN repros + M3a-class regression run) — real elimination needs the crossing-classifier fix itself (Phase 1b), not yet built at signing time | **H** (B×II) | **RISK OVERRIDE — SIGNED (Patrick, 2026-07-09; block below)** |
| **R14 — the shape-arg UAF class is WIDER than R13's signed scope: `number`/`maybe`/`union` args to a suspending callee (the `value_to_i64_bits` by-pointer staging arm, `emit.rs:12323`) AND anonymous struct-literal args also stage a dangling pointer across suspension → silent garbage; both pre-existing in shipped v0.3.0** (FRAGO 006 — surfaced by Phase 1b's post-fix residual probes; phase assignment re-drawn by FRAGO 007) — *Phase 1b (number) + Phase 1c (maybe/union + anonymous)* | A (number/maybe/union) / B (anon) | II | EH / H | RED-repro + full-regression + determinism gate (**B2**, prob −1; proof: Phase 1b's number RED→GREEN repro + Phase 1c's maybe/union + anon-arg RED→GREEN repros, each with N≥10 determinism proof) — real elimination needs the frame-backing fixes themselves (extend Phase 1b; new-machinery Phase 1c), not yet built at signing time | **H** (B×II / C×II — converges HIGH either way) | **RISK OVERRIDE — SIGNED (Patrick, 2026-07-09; block below)** |
| **R15 — sibling decimal128-across-a-concurrency-boundary defects: background-spawn `number` arg UAF (silent garbage) + cpu-member `number` arg ICE (loud crash) — confirmed LIVE, pre-existing in shipped v0.3.0** (FRAGO 009) — *Phase 1d* | C | II | H | RED-repro-before-fix for BOTH A (background-spawn UAF) and C (cpu-member ICE), full-regression + false-positive-sweep gate, PLUS the phase executor's own recorded design decision (gate-consistent-reject vs. eager i128 heap-copy) weighed against IMP-concurrency.md/IMP-no-function-coloring.md and this plan's own fixed<T>/channel<number> precedents (**B2**, prob −1; proof: committed RED→GREEN repros, Phase 1d step 4) | **M** (D×II) | recorded |
| **R16 — twin type-walker unification is itself incomplete or introduces a new divergence** (FRAGO 011, dormant hardening, un-deferred P1-2) — *Phase 5b* | D | III | L | Regression-gate the SM-resume + frame-layout suites explicitly (the exact fragile subsystem class that shipped M3a/M3d/M3e/M3g); grep-gate confirms zero second derivation (**B2**, prob −1; proof: committed regression-gate run, Phase 5b step 4) | **L** (E×III) | pass |

**RISK OVERRIDE — accepted residual: HIGH. SIGNED. (R13 —
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

**RISK OVERRIDE — accepted residual: HIGH. SIGNED. (R14 —
FRAGO 006, recorded verbatim from the audit sidecar.)**

- **Risk:** the shape-arg UAF class is wider than R13 — `number`/`maybe`/`union` args AND anonymous
  struct-literal args to a suspending callee also stage a dangling pointer across suspension →
  silent garbage; pre-existing v0.3.0.
- **Why not mitigable to LOW now:** same silent-miscompile class as R13; real elimination needs the
  frame-backing fix itself (extend Phase 1b for number; Phase 1c for maybe/union + anon, per
  FRAGO 007's re-homing — the signed class and disposition are unchanged).
- **Accepted by: Patrick — 2026-07-09** (interactive gate; disposition: **"Fully fix the entire
  class now"** — full frame-backing for the whole class, nothing rejected, nothing deferred).
- **Trigger to close:** the number fix (Phase 1b) AND the maybe/union + anonymous-aggregate fixes
  (Phase 1c) each land RED→GREEN + full-regression + determinism proof before Phase 6b's sanitizer
  lane and before release.

**Floor check.** No Floor-A "no backout exists" condition (every change is git-reversible) and no
Floor-B class (security/PII/money/irreversible-prod-op) fires anywhere in this table — R13 and R14
included (pre-v1.0, zero public users, git-reversible, runtime-only-within-one-execution).

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
1b. No shape, `fixed<T>`, or `number` argument to a suspending callee is left in
   a dying stack alloca across suspension — the confirmed pre-existing v0.3.0 UAF (parent Pending →
   stack dies → child resumes on a dangling pointer → silent garbage) is closed by extending the
   ONE authoritative crossing classifier: a shape arg is frame-backed via the existing `shape_embed`
   machinery; a `fixed<T>` arg is routed into the existing Check 2b `UnsupportedCrossingLocalType`
   guard → a deterministic teaching compile error (FRAGO 005 — no embed machinery exists for
   `fixed<T>`); a `number` arg is frame-backed so it WORKS across suspension —
   NOT rejected — via the same classifier plus consumer-side frame-backing plumbing in the
   arg-staging path (the staged pointer targets the parent frame's decimal128 slot region)
   (FRAGOs 006/007, R14 signed) —
   all proven by deterministic-across-runs RED→GREEN repros (Phase 1b, FRAGOs 004/006/007).
1c. No anonymous struct-literal, `maybe`, or `union` argument to a suspending callee is left in a
   dying stack alloca across suspension — the hard new-machinery portion of the same
   escape-through-callee-frame UAF class (anon: no LET name to anchor on, needs new anchoring;
   maybe: needs envelope+payload ownership machinery, no Maybe arm in the crossing strategy
   table; union: non-uniform repr, the documented `value_to_stable_bits` known-hole — all
   near-new-design, M3a-class caution) is closed with full frame-backing — maybe/union routed
   PAST Check 2b's rejection, never INTO it (nothing rejected, per R14) — proven by
   deterministic-across-runs RED→GREEN repros (Phase 1c, FRAGOs 006/007, R14 signed).
1d. The sibling decimal128-across-a-concurrency-boundary defects (FRAGO 009) are closed: a
   `number` argument to a `background`-spawned task (both the CPU-spawn and SM-spawn arms) no
   longer dereferences a stack-dangling pointer after the spawner returns, and a `number` argument
   into a cpu-member spawn no longer ICEs the compiler — via the phase executor's own recorded
   design decision (weighing gate-consistent-rejection against the existing `channel<number>`
   compile gate, `check.rs:3417-3451`, vs. an eager i128 heap-copy), proven by
   deterministic-across-runs RED→GREEN repros (Phase 1d). The conduit-send half of the same class
   (`emit.rs:11809`) is verified-safe-by-gate — `channel<number>` is compile-gated today, so this
   path is unreachable — and is recorded as a Future-Requirements deferral (#12), not fixed here.
   **Headline-vs-delivered reconciliation (fix-loop rounds 1-4, FRAGOs 016/018/019):** the DELIVERED
   scope of Phase 1d grew well past these two concurrency-boundary defects (A/C). Round 1 rerouted the
   interim int-literal→`number` guard from the ICE banner to a typeck teaching error; rounds 2-3
   discovered the class was near-compiler-wide (an int literal / `-IntLit` into a `number` slot
   segfaults, silently mis-compares, or ICEs across ~27 argument / construction / statement slots, not
   just the spawn boundary) and COMPLETED a single authoritative rejection guard
   (`reject_int_literal_number_slot`) over every such slot; round 4 closed the errors-wrapped `return`
   gap in that guard. So Phase 1d's real deliverable is BOTH the FRAGO-009 decimal128-boundary fix
   (A/C, D8/Option 2) AND the near-compiler-wide int-literal→`number` rejection guard. The two store
   sites (`let x: number = 5` and `hidden f: number = 5`) are now ALSO gated by that same guard (the
   v0.3-M6 store-site stopgap, FRAGO 020) — rejection is uniform across every facet; only the
   int→number COERCION (#9/#14) remains deferred to the `2026-07-04-v0-3-hotfix-int-literal-number`
   stub plan.
2. The block_on-fallback branch (`emit.rs:15122-15137`) is a compile-time hard error for any caller
   not reachable via the designated synchronous entry point — mirroring `emit.rs:11162`'s sibling.
3. A cancelled sender's `pending_sends` entry is purged (idempotently) and the `caller_token` is
   generation-salted, across BOTH token-producer sites (the frame-pointer conduit token minted at
   `emit.rs:12205-12208` AND the handle-pointer task-handle token minted at `handle.rs:326`) — the
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
9. P2-3 and the dynamic-dispatch × suspension predicate gap (FRAGO 002 → Future Requirements #10)
   are recorded as proper four-field deferrals in Future Requirements — never silent, never a loose
   checkbox. P2-5, confirmed LIVE in Phase 0 (FRAGO 001), is FIXED in Phase 3b. P1-2 (confirmed
   dormant, Phase 0), un-deferred by user-directed Mission-scope (FRAGO 011), is FIXED in Phase 5b
   (twin type-walker unification behind one authoritative resolution) rather than left dormant-and-
   deferred. P2-7 (newly surfaced, R11), un-deferred by user-directed Mission-scope (FRAGO 010), is
   FIXED in Phase 4b (the same register-before-poll discipline Phase 4 applied) rather than
   deferred.
10. The `ynz-runtime` crate is Miri-clean and clean under ThreadSanitizer/AddressSanitizer (or every
    finding is triaged on the record) — and a dedicated sanitizer CI job is live in
    `.github/workflows/ci.yml`, proven non-vacuous, so the bug classes this milestone fixes (UAF,
    double-free, data races) are mechanically hunted on every future push/PR, not just today.
11. Every surviving Future-Requirements deferral (P2-3; bare-channel end-of-stream/channel-close
    semantics; preemption real back-edge yield; `background.cpuBound`; the two orthogonal ICEs —
    roadmap-ledger row 441 and `fixed<T>` param-iteration; the dynamic-dispatch × suspension
    predicate gap; FRAGO-009's conduit-send-number deferral; and the Phase 1c per-iteration
    maybe/union heap-cell loop leak (#13, FRAGO 015)) is lifted into the roadmap's
    durable store — four-field payloads in the roadmap `audit.md`, pointer rows in the roadmap
    Capability Ledger, owner-tagged (preemption→M7; channel-close/P2-3→M8; the two ICEs +
    dynamic-dispatch→unscoped, row 441 flagged for Patrick's Gate-4 call; the heap-cell loop leak→
    the drop-story milestone, unscoped until it's scheduled) — so none of it becomes
    invisible when this plan archives to `done/` (Phase 8, FRAGO 012/015).

**Disciplined initiative.** When steps and reality diverge: **verify before you fix** (every fix in
this plan traces to a CONFIRMED or CONFIRMED-BY-MECHANISM audit finding; a THEORY/CLAIMED finding
gets verified — Phase 0 — before it is fixed or deferred). **Thread the one authoritative source; never
invent a second derivation** to unblock yourself — surface the blocker instead (CCIR-2). **A
mitigation with no committed proof artifact is worth zero** — do not claim a RED→GREEN fixture exists
without committing it. **No duct tape** — a fix that "mostly" closes a race or leak, with no four-field
deferral naming the remaining gap, is not done.

### 3.2 Concept

Sixteen phases (0–8, with 1b, 1c, AND 1d inserted between 1 and 2 — 1b/1c per FRAGOs 004/006,
Patrick-signed to run immediately after Phase 1 and BEFORE Phase 2, and 1d per FRAGO 009 sequenced
immediately after 1c as the third hard-new-machinery decimal128 phase; 3b inserted between 3 and 4
per FRAGO 001; 4b inserted between 4 and 5 per FRAGO 010 (un-deferred P2-7); 5b inserted between 5
and 6 per FRAGO 011 (un-deferred P1-2); and 6b inserted between 6 and 7 — see the amendment note in
Terrain). **Full sequencing: P0 → P1 → P1b → P1c → P1d → P2 → P3 → P3b → P4 → P4b → P5 → P5b → P6 →
P6b → P7 → P8.** **Gate first**
(P0 verifies the two THEORY findings, the dynamic-dispatch × suspension coverage question, and
confirms the execution-gate precondition). **The flagship blocker + its escape hatch** (P1 UFCS fix
on its carved 9-site scope; P1b the arg frame-backing UAF fix for shape/`fixed<T>`/number —
FRAGOs 004/006/007, signed sequencing P1 → P1b → P1c → P1d → P2; P1c the hard new-machinery
frame-backing fix for anonymous-aggregate + `maybe` + `union` args — FRAGOs 006/007, unnamed
temporaries need new anchoring, maybe/union need new crossing machinery; P1d the sibling
decimal128-across-a-concurrency-boundary defects (background-spawn `number` arg UAF + cpu-member
`number` arg ICE) — FRAGO 009, the third hard-new-machinery decimal128 phase, its fix APPROACH left
as the phase executor's own recorded design decision (gate-consistent-reject vs. eager i128
heap-copy), never conductor-pre-decided; P2 the block_on-fallback guard — sequenced after the
P1/P1b/P1c/P1d set because P2's correctness assertion depends on P1 actually being fixed, a
deliberate resequencing of the audit's raw synthesis order, recorded as Decision D1 below).
**Channel/scheduler correctness** (P3 ABA+orphan; P3b recursion-chain spike CPU-handle cleanup leak —
FRAGO 001, sequenced right after P3 in the same drop-ladder region; P4 lost-wakeup; P4b the
`handle_recv_poll` panic-then-pending hang — FRAGO 010, un-deferred P2-7, sequenced right after P4 in
the same register-before-poll discipline; P5 buffered-element leak; P5b the twin type-walker
unification — FRAGO 011, un-deferred P1-2, dormant-hardening — independent subsystems apart from the
P3→P3b and P4→P4b adjacencies, sequenced for one conductor's convenience, not a hard dependency
chain, except that P5b is a real dependency of P6b, below).
**Mechanical + honesty** (P6 two small independent fixes; P6b sanitizer lane — Miri/TSan/ASan on the
runtime crate, proven non-vacuous and CI-enforced going forward; P7 docs/registry sweep).
**Close-out** (P8 demo/gallery/roadmap/full-suite/release-handoff — extended by FRAGO 012 to lift
every surviving Future-Requirements deferral into the roadmap's durable store). Each phase ends
green-tree with its fixtures committed; Phase 1 (the flagship, >5 steps) checkpoints per the marks
below.

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

#### Phase 1b — arg frame-backing miscompile: shape/`fixed<T>`/number (confirmed UAF; pre-existing v0.3.0)

- **Task + purpose:** close the confirmed pre-existing use-after-free (FRAGO 004 — surfaced by
  Phase 1 segment 4, corroborated by the adversarial code-reviewer; scope GROWN by FRAGO 006 /
  signed R14, then re-scoped by FRAGO 007 to this phase's FEASIBLE cases — shape, `fixed<T>`,
  `number`; the `maybe`/`union` half is re-homed to Phase 1c) where a shape-value argument
  passed to a suspending callee is staged in the PARENT resume fn's STACK alloca: the child frame
  holds a `ptrtoint` of it; parent returns Pending → parent stack dies → child resumes on a
  dangling `self` → silent nondeterministic garbage. Reproduces in pure `Call` form
  (`wait crew(ship)`) AND auto-inserted transitive suspension; PRE-EXISTING in shipped v0.3.0
  (stash-to-`main` proof — NOT introduced by M6; UFCS merely made the shape-passing form natural
  to exercise); likely also affects `fixed<T>` (string/array/map are safe — heap-backed, stable
  pointer). **FRAGO 006 scope growth (R14, Patrick-signed "fully fix the entire class now"), as
  re-scoped by FRAGO 007: a `number` arg to a suspending callee shares the same
  escape-through-callee-frame class via the arg-staging `load()` copy → `value_to_i64_bits`
  by-pointer arm (`emit.rs:12323`) — a by-pointer stage of a parent-stack alloca (`number` prints
  0.000 vs 2.5, probe-confirmed LIVE). This phase frame-backs `number` so it WORKS across
  suspension — NOT rejected. `maybe`/`union` are also probe-confirmed LIVE UAFs but
  NEEDS-NEW-MACHINERY (no Maybe/Union arm in the crossing strategy table; the fixed<T>/anon
  category, not number-like) — their FULL frame-backing moves to Phase 1c per FRAGO 007, same
  signed R14 disposition, never rejected, never deferred.** Root cause:
  `locals_crossing_wait`/`collect_crossings_in_stmts`
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
     constructible) a `fixed<T>` stack-aggregate variant, PLUS (FRAGO 006, re-scoped by FRAGO
     007) a `number` repro — bind a `number` local, pass it to a suspending
     callee, assert the correct deterministic value. Each shape/number repro asserts
     the CORRECT value (deterministic, not garbage); the `fixed<T>` repro asserts the
     deterministic Check 2b compile error (FRAGO 005). Confirm each fails RED for the documented
     UAF reason, not some unrelated bug. (The `maybe`/`union` repros move to Phase 1c per FRAGO
     007 — their repro-locking follows the 1c disposition: assert-value frame-backing repros.)
  3. Fix: extend the ONE authoritative crossing classifier so a value passed BY POINTER to a
     suspending callee is classified as crossing — never a second frame-layout path
     (authoritative-derivation.md). Per type (FRAGOs 005/006/007): a **shape** arg → frame-backed
     via the existing `shape_embed` machinery (unchanged); a **`fixed<T>`** arg → routed into the
     EXISTING Check 2b `UnsupportedCrossingLocalType` guard (`check.rs:872-984`) → the UAF is
     closed as a deterministic teaching COMPILE ERROR (NOT frame-backing — no embed machinery
     exists for `fixed<T>`), consistent with the read-after-wait twin's handling; a **`number`**
     arg → widen the same classifier's candidate type-match arm
     (`collect_aggregate_args_to_suspending_calls`) to include LET-bound `number` (decimal128)
     idents that escape to a suspending callee, AND add the consumer-side frame-backing plumbing
     in the arg-staging path (both the embedded-frame and heap-boxed inline-poll staging loops)
     so the staged pointer targets the parent frame's 2-slot decimal128 region — full
     frame-backing, it WORKS across suspension. Verify the emitted IR for shape AND number args:
     the arg lives in the heap frame, not a dying stack alloca; the child's pointer points into
     the surviving frame.
  4. Full-regression: all Phase 1 fixtures (a, b, c, d) GREEN; all shape/fixed/number
     repros GREEN; full workspace suite green
     (`docker compose run --rm dev cargo test --workspace`); house clippy `-D warnings` clean.
     Explicitly re-verify no regression to the M3a/M4/M5 suspension + frame-layout suites (this
     is the fragile subsystem). Run an explicit **false-positive sweep** (mirror Phase 2's corpus
     false-positive discipline): confirm the extended classifier routes ONLY genuine
     escape-to-suspending-callee cases — no `fixed<T>` argument that does not escape to a
     suspending callee trips the Check 2b error, and no non-escaping `number`
     arg is wrongly frame-backed/affected.
  5. Non-vacuous determinism proof: run the shape + number repros N≥10 times each;
     assert the SAME correct value every run (the pre-fix signature was nondeterministic garbage
     across runs).
  6. Post-fix residual probes (FRAGO 005 — **executed segment 2; verdicts recorded and routed
     through FRAGOs 006/007 and R14**): anonymous-aggregate arg LIVE → Phase 1c (FRAGO 006);
     indexed arg NOT REACHABLE (typeck-rejected); loop-var shape arg NOT LIVE (heap array storage
     survives); `number` LIVE → THIS phase (signed R14); `maybe`/`union` probe-confirmed LIVE but
     NEEDS-NEW-MACHINERY (segment 4) → Phase 1c (FRAGO 007). Any genuinely-NEW LIVE class found
     beyond number/maybe/union/anon gets surfaced for its OWN FRAGO, never a quiet scope-add.
- **Exit criteria:** UAF closed via the one crossing-classifier extension (no second frame-layout
  path); RED→GREEN repros committed (fixture (b) + the pure-`Call` repro + the `number` repro
  GREEN with deterministic correct values; the `fixed<T>` repro GREEN as an
  asserted deterministic compile error per FRAGO 005), deterministic across runs (N≥10); the
  false-positive sweep clean (only genuine escape-to-suspending-callee cases affected — Check 2b
  for `fixed<T>`, frame-backing for number); full suite green including the
  M3a-class regression surface (M3a/M4/M5 suspension + frame-layout suites); residual-probe
  verdicts recorded (LIVE ⇒ surfaced for its own FRAGO, never self-folded); the FRAGO 004 signed
  RISK OVERRIDE's revisit-trigger satisfied — this phase's proof landing converts the
  accepted-HIGH interim risk (R13) to closed, fixed bug — and R14's shape+fixed+number portion
  closed (the maybe/union + anonymous-aggregate portion remains OPEN until Phase 1c lands, per
  FRAGO 007).
- **Reviewer fan-out:** code-reviewer (the crossing-classifier + frame-layout diff); adversarial
  gate-checker (does the repro genuinely exercise the dangling-stack window across pure-`Call`
  AND auto-inserted transitive suspension, and is the determinism proof non-vacuous?);
  design-doc-alignment reviewer (authoritative-derivation.md — the ONE crossing classifier
  extended, no second frame-layout path).
- **Model tag:** `(coding, high, medium)`

**Phase 1b complete (executor `executor-2026-07-09-m6-phase1b-seg7`, sealed 2026-07-10):** the arg
frame-backing UAF class is closed on this phase's full FRAGO-007 scope — **shape** (frame-backed via
the existing `shape_embed` machinery), **`fixed<T>`** (deterministic Check 2b teaching compile
error), **`number`** (frame-backed via the existing 2-slot decimal128 crossing machinery). ONE
classifier drives all three (`collect_aggregate_args_to_suspending_calls` → `mark_aggregate_arg`,
`check.rs`, type-match Shape | BuiltinFixed | Number{p≤34}); ONE shared staging rule
(`stage_suspending_call_arg_bits`, `emit.rs`) serves ALL THREE child-frame arg-staging loops —
the embedded-frame inline-poll, the heap-boxed recursive, AND (fix-loop 1, 2026-07-10) the
auto-parallelized I/O-group `emit_io_member_init` loop shared by the independent and fused group
paths (the seal's original "serves BOTH loops" wording was WRONG: the third loop had been missed,
leaving the number-arg UAF live on the auto-parallelization path until the fix-loop closed it) —
zero second frame-layout path, slot math read from the authoritative
`sm_crossing_slot_indices` (authoritative-derivation holds). IR verified for shape (seg-2 receipt)
AND number (seg 7): the pre-fix dangling pattern (`load()` copies the i128 into a fresh resume-fn
stack alloca, `ptr_to_int` of the temp staged into the child frame — probe-confirmed `0.000` vs
`2.5`) is GONE; the parent now stages a `getelementptr` into its own heap-resident frame's 2-slot
decimal128 region (kept current by the per-statement flush) and the child reads the surviving frame
bytes via the `sm_number_param_set` `inttoptr` indirection in `load()`. All repros GREEN with exact
deterministic values (fixture (b) GREEN in `v03_m6_ufcs_suspension.rs`; pure-Call shape `7`;
transitive chain `8`; number `2.5`; fixed<T> asserts the Check 2b teaching error); determinism
proved N=10 per repro in-test for shape AND number. False-positive sweep clean (IR-verified: a
non-escaping `number` arg stays a plain alloca with zero frame flushes; a non-escaping `fixed<T>`
arg trips no Check 2b error). M3a-class regression: none — 522/522 green with the integration
binary run alone; clippy `-D warnings` + `cargo fmt --check` clean. **R13 (shape) CLOSED; R14's
number portion CLOSED; R14's maybe/union + anonymous-aggregate portion remains OPEN → Phase 1c
(FRAGO 007).** *(Note: this "R14 number portion CLOSED" line was written pre-fix-loop, before the
Phase 1b boundary review found the third arg-staging loop [`emit_io_member_init`] still leaving the
number-arg UAF live on the auto-parallelized I/O-group path. Re-affirmed accurate post-fix-loop: the
fix-loop [`executor-2026-07-10-m6-phase1b-fixloop1`] closed that gap on all three staging loops, and
the Round-2 boundary review sealed Phase 1b clean — see the fix-loop entry and the Phase 1b SEAL
entry in `audit.md`. The line stands as written.)* **Deviation surfaced (not self-decided):**
`v03_m3e_alias_local_name_collision_runs_correctly` fails 3/3 under full-workspace parallel load
once this phase's 6 new tests (~26 added concurrent `ynz` process spawns) run alongside it, and
passes 9/9 isolated + 522/522 integration-alone + full-workspace-green with the new tests skipped —
IR-proven orthogonal to this diff (the fixture is all-int locals/literal args; its IR contains zero
of the new instruction patterns); the root fragility is the M3e test's own fixed 300ms shutdown
window (documented in its own comment as timing-sensitive). Routed to the deviation-judge → FRAGO
seam for disposition (candidate remedy: make the M3e fixture's background-task completion
deterministic instead of racing a fixed sleep).

**Phase 1b fix-loop 1 (executor `executor-2026-07-10-m6-phase1b-fixloop1`, 2026-07-10 — code-reviewer
BLOCKER closed):** the THIRD arg-staging loop (`emit_io_member_init`, serving the auto-parallelized
independent AND fused I/O-group paths) now routes through the one `stage_suspending_call_arg_bits`
helper; full staging-site sweep confirms exactly three child-frame arg-staging loops exist and all
three route through it (no fourth). RED→GREEN proven on both group callers (independent:
0.000/0.000 → 2.5/4.5; fused: 1226 + 0.000 → 1226/4.5), each locked with a value test + N=10
in-test determinism gate (`v0_3_m6_number_arg_parallel_group.ynz`,
`v0_3_m6_number_arg_fused_group.ynz`). Batched hardening: committed false-positive-sweep fixture
(`v0_3_m6_non_escaping_args_false_positive_sweep.ynz` — non-escaping number NOT wrongly
frame-backed, non-escaping fixed<int> NOT Check-2b-rejected) and the transitive-chain N=10
determinism gate. Orthogonal findings surfaced to the seam (not self-decided): the background-spawn
arg pipeline (`prepare_bg_arg_for_ctx`, no Number arm) and conduit send-value staging stage
decimal128 by-pointer under a DIFFERENT lifetime class (helper inapplicable) — potential separate
number-arg exposure needing its own probe/FRAGO; and the base read-after-suspension scan's
declaration-position-insensitive fixed/maybe flagging is PRE-EXISTING (reproduced pre-Phase-1b),
not a classifier regression. Gates: workspace 2300/2300 green (M3e flake did not fire), clippy
`-D warnings` + `cargo fmt --check` clean. See the fix-loop Session-log entry in `audit.md`.

#### Phase 1c — hard new-machinery frame-backing: anonymous-aggregate + `maybe` + `union` args across suspension (FRAGOs 006/007; pre-existing v0.3.0)

- **Task + purpose:** close the remaining, hard new-machinery portion of the R14-signed UAF class
  (FRAGO 006 — surfaced by Phase 1b's post-fix residual probes; scope re-drawn by FRAGO 007):
  three probe-confirmed LIVE UAFs whose fixes each need **near-new machinery** rather than
  Phase 1b's consumer-plumbing. (1) An **anonymous struct-literal argument** to a suspending
  callee is staged in a dying stack slot across suspension (`wait crew({...})` prints garbage,
  e.g. 4240380 vs 7) — same escape-through-callee-frame root cause as Phase 1b, but with NO LET
  name for the classifier to anchor on: needs **new anchoring for unnamed temporaries**. (2) A
  **`maybe`** argument is a LIVE UAF (probe-confirmed segment 4: compiles — Check 2b never fires
  on arg-escape — and prints NONDETERMINISTIC 13-15-digit pointer garbage vs the correct 42):
  there is NO Maybe arm in the crossing save/restore strategy table (`emit.rs:4492-4556`), and
  Maybe frame-backing needs **envelope+payload ownership machinery** (cf. the `maybe_to_owned` /
  `maybe_to_heap_cell` funnels). (3) A **`union`** argument is a LIVE UAF (probe-confirmed
  segment 4: deterministically WRONG variant — a Square arg prints `circle` 5/5 runs from the
  dangling tag read; the non-suspending control prints `square`): Union repr is **non-uniform**
  ({i64 tag, i64 data} tagged struct vs NULL ptr for `T | nothing`) — an explicitly documented
  KNOWN-HOLE in `value_to_stable_bits`, loud-fail-pinned by
  `m5_p3_sweep_union_readback_blocked_*.ynz`. All three are near-new-design territory, M3a-class
  caution (this subsystem carries a ~10-round whack-a-mole history, D7): RED repro before fix,
  one authoritative crossing classifier, never a second frame-layout path
  ([authoritative-derivation.md](../../../rules/authoritative-derivation.md)). Per the signed R14
  disposition ("fully fix the entire class now"), carried unchanged through FRAGO 007: full
  frame-backing — anon/`maybe`/`union` args WORK across suspension, NOT rejected. **Explicit
  Check 2 + Check 2b obligation (per R14's "nothing rejected"; FRAGO 014 ITEM 2 — Check 2 is a
  SECOND rejection surface consuming the SAME crossing set, `check.rs:811`): the maybe/union fix
  must route these crossing locals PAST BOTH Check 2's nested-shape rejection (`check.rs:817-870`)
  AND Check 2b's `UnsupportedCrossingLocalType` rejection (`check.rs:919-933`) — NOT into either;
  a compile-error reject is not an acceptable disposition — and must do so without a second
  classification path.** **Tracked minor (test-quality, Phase 1b boundary review):**
  `crates/ynz-driver/tests/v03_m6_shape_arg_frame_backing.rs`'s false-positive test's number-half
  WHY comment slightly overstates its causal mechanism — fold in a 1-line comment-clarification the
  next time this phase (or Phase 1d) edits that test file; not urgent enough to justify a standalone
  edit.
- **Steps**
  0. **GATING STEP (FRAGO 008 — deviation-judge JUSTIFIED/risk-neutral/no-signature; Patrick-ratified
     "ok tacking it onto the next phase").** Close the PRE-EXISTING full-workspace-load
     flake `v03_m3e_alias_local_name_collision_runs_correctly` (`integration.rs:2268-2308`) before
     any of this phase's own new fixtures land: its own comment concedes a fixed-shutdown-timing
     race margin, and Phase 1b's ~26 new concurrent `ynz` spawns already tipped it past that margin
     (fails 3/3 under full-workspace load, passes 9/9 isolated + 522/522 integration-alone —
     IR-proven orthogonal). Replace the fixed-timeout shutdown-race margin with a **real
     synchronization primitive** (a join/barrier/channel the background task closes BEFORE the test
     asserts). **GUARDRAIL: must REMOVE the race (real sync), NOT widen the sleep** — a bigger
     number is the same race with a longer fuse, i.e. duct tape. After this step, the full
     workspace suite is deterministically 522/522 under load and every downstream gate (including
     this phase's own step 4 full-regression run, below) is clean.
  1. CCIR-1: re-verify the cited lines against the live tree (the Phase 1b classifier extension
     `collect_aggregate_args_to_suspending_calls` / `mark_aggregate_arg` in
     `crates/ynz-typeck/src/check.rs`; the `shape_embed` frame-backing path + the crossing
     strategy table in `crates/ynz-codegen/src/emit.rs`; Check 2b `check.rs:919-933`; the
     anon-arg staging site) before acting on them.

     **CHECKPOINT** — citations re-verified against the live tree, receipts recorded in the
     handoff; ready to author the RED repros in a fresh dispatch if resumed later.

  2. Lock the RED repros (RED-repro-before-fix, one per case): (a) anon-arg — pass an anonymous
     struct literal to a suspending callee (`wait crew({...})` form); (b) `maybe`-arg — bind a
     `maybe<int>` local (fixture-proven constructible form: bare `m: maybe<int>` param shape,
     `` `42`.toInt() `` RHS, `.or(0)` read), pass it to a suspending callee; (c) `union`-arg —
     bind a union local (`shape Figure = Circle | Square` form), pass it to a suspending callee.
     Each asserts the CORRECT deterministic value (frame-backing disposition — assert-value, not
     assert-compile-error); confirm each fails RED pre-fix for the documented UAF reason
     (garbage / wrong variant, not some unrelated bug).

     **CHECKPOINT** — all three RED repros locked and confirmed failing for the documented UAF
     reason (Paper-Traced garbage / wrong variant, not an unrelated bug); scaffolding clean on a
     green-building tree (documented RED per this phase's RED-repro-before-fix discipline).

  3. The fixes — CORRECTED 4-part sub-step split (FRAGO 014; each via the ONE authoritative
     classifier, never a second frame-layout path; mechanism per FRAGO 013 — bind-time promotion
     to counted heap cells, the frame-embed reading being structurally impossible for union's
     NULL-none ABI). **Union is DEFERRED entirely (classifier-widen AND codegen) to 3c** so every
     sub-step boundary stays green-building: landing the Union classifier-widen before 3c's
     annotation override exists would make emit.rs's classification loop misclassify the
     union-annotated crossing local into `shape_embed_set` (Decision 12, handoff) and break the
     locked union RED fixture DIFFERENTLY (misclassification/ICE) — violating RED-repro
     discipline at the 3a→3b boundary.

     - **3a — typeck routing (Maybe + anon-StructLit ONLY; Union DEFERRED to 3c):** widen
       `mark_aggregate_arg` (`check.rs:7913-7918`) with `Type::Maybe` + anonymous `StructLit`
       args (NOT `Type::Union`); refactor `crossing_local_names_with_cpu_spike` (`check.rs:7553`)
       into a provenance-returning core exposing `arg_escape_only` (names first inserted by the
       arg-escape collector — ONE producer feeding both typeck Check 2/2b and codegen frame
       layout, no twin); Check 2 (nested-shape, `check.rs:817-870`) AND Check 2b
       (`check.rs:898-984`) skip rejection iff the name is arg-escape-only AND the effective type
       is Maybe. Exit: workspace builds; the maybe + anon RED repros now COMPILE (still UAF-RED —
       expected, fixed in 3b); the union repro stays FULLY RED and is NOT misclassified into
       shape_embed; the fixed<T> + M3a read-after-wait rejection fixtures stay byte-identical
       green.

     **CHECKPOINT** — 3a boundary: typeck routing landed on a green-building tree (documented
     RED = the locked repros); resume `phase-1c/step-3b` in a fresh dispatch if handed off.

     - **3b — codegen maybe + anon:** `store_binding` (`emit.rs:19836`) crossing-maybe →
       `maybe_to_heap_cell`; `stage_suspending_call_arg_bits` (`emit.rs:11274`) routes
       `Expr::StructLit` args through `value_to_stable_bits` (covers all three staging loops by
       construction; scope-minimal: `StructLit` only, other non-Ident forms stay recorded
       residuals). Exit: maybe + anon repros GREEN (4/6); union still RED.

     **CHECKPOINT** — 3b boundary: maybe + anon GREEN on a green-building tree; resume
     `phase-1c/step-3c` in a fresh dispatch if handed off.

     - **3c — codegen union (incl. ITS OWN classifier-widen + annotation override):** add
       `Type::Union` to the `mark_aggregate_arg` widen TOGETHER WITH the annotation-aware
       override in the `emit.rs:4493` classification loop (resolve the Let's Union annotation via
       `ast_type_to_typeck_type` — the union-ctor arm's own resolution, one source, no twin) so a
       union-annotated let does NOT misclassify into `shape_embed_set` (Decision 12); add the
       Check 2 AND Check 2b Union skip (arg-escape-only + annotation resolves to Union); new
       `union_to_heap_cell` (null → null; non-null → clone the {tag,data} envelope + tag-resolved
       payload deep-copy); union-ctor Let arm (`emit.rs:12641-12691`) crossing branch stores the
       cell into the pre-created sm_entry crossing alloca (never a fresh `outer_slot` — the
       clobber gotcha); `store_binding` Union arm (crossing → `union_to_heap_cell`; non-crossing
       → plain store, byte-identical); resolve the `value_to_stable_bits` KNOWN-HOLE doc
       textually only (NO Union arm added — Decision 15; persist surfaces + pins unchanged).
       Exit: all 6 repros GREEN; maybe/union verified routed PAST Check 2 AND Check 2b (nothing
       rejected, per R14) with provenance from the ONE producer, never a re-derived twin scan.

     **CHECKPOINT** — 3c boundary: all 6 repros GREEN on a green-building tree; resume
     `phase-1c/step-3d` in a fresh dispatch if handed off.

     - **3d — parity verdict + IR proof:** resolve (not assume) the carried open recon under this
       phase's own gates — heap-cell ownership/free semantics in LOOPS: exact-gap alloc=free
       parity probe (per-iteration crossing maybe binding through a suspending callee, pinned per
       the `integration.rs:5493-5554` convention), verdict RECORDED either way; verify the
       emitted IR per case: the arg is staged from a surviving allocation (counted heap cell),
       not a dying stack slot.

     **CHECKPOINT** — 3d boundary: parity verdict recorded + IR proof done; steps 4-5 in a fresh
     dispatch if handed off.
  4. Full-regression + determinism proof: all Phase 1/1b fixtures + repros GREEN; full workspace
     suite green (`docker compose run --rm dev cargo test --workspace`, deterministically —
     step 0's fix means this run is no longer at risk of the M3e load-flake); house clippy
     `-D warnings` clean; N≥10 runs of EACH new repro (anon, maybe, union), same correct value
     every run.
  5. False-positive sweep: no non-escaping anonymous aggregate, `maybe`, or `union` local (one
     consumed entirely before any suspension) is wrongly affected by the new anchoring/machinery;
     the pre-existing Check 2b read-after-wait rejections for maybe/union remain exactly as the
     design intends post-fix (no silent widening or narrowing beyond the arg-escape path's
     frame-backing).
- **Exit criteria:** step 0's M3e determinism fix lands (real sync primitive, race removed, not
  widened — FRAGO 008), so the full workspace suite is deterministically 522/522 under load; all
  three UAFs (anon, maybe, union) closed via the one crossing-classifier
  extension + the one strategy table (no second frame-layout path, no second classification
  path); RED→GREEN repros committed for all three, each deterministic across runs (N≥10);
  maybe/union verified routed PAST Check 2 AND Check 2b (nothing rejected, per R14; FRAGO 014
  ITEM 2); false-positive sweep
  clean; full suite green including the M3a-class regression surface (M3a/M4/M5 suspension +
  frame-layout suites); R14's maybe/union + anonymous-aggregate portion closed — with Phase 1b's
  shape+fixed+number portion already landed, this converts the whole accepted-HIGH R14 interim
  risk to closed, fixed bug (before Phase 6b's sanitizer lane and before release).
- **Reviewer fan-out:** code-reviewer (the anchoring + Maybe/Union machinery + frame-layout
  diff; AND step 0's M3e fixture fix — confirm it is a genuine synchronization primitive, not a
  widened sleep); adversarial gate-checker (do the repros genuinely exercise the dangling-stack window for
  an UNNAMED temporary and for maybe/union args; is each determinism proof non-vacuous; are
  maybe/union genuinely frame-backed rather than quietly rejected?); design-doc-alignment
  reviewer (authoritative-derivation.md — the ONE crossing classifier extended, no second
  frame-layout path).
- **Model tag:** `(coding, high, medium)`

**Phase 1c complete (executor `executor-2026-07-10-m6-phase1c-seg7`, 2026-07-10; segments 1-7):**
all 6/6 locked repros GREEN — the R14 maybe/union + anonymous-aggregate portion is CLOSED via
bind-time promotion to counted heap cells (FRAGO 013 mechanism), through the ONE crossing
classifier (`crossing_local_names_with_provenance` / `mark_aggregate_arg` widened Maybe + Union +
anon `StructLit`) and the one staging rule — no second frame-layout or classification path.
Maybe/union verified routed PAST Check 2 AND Check 2b (nothing rejected, per R14/FRAGO 014 ITEM 2;
the skips key on arg-escape-only provenance from the one producer). **IR-proven per case (step
3d):** maybe — `%m_env_cell = ynz_alloc` at bind time, staged bits are `ptrtoint` of the loaded
CELL pointer flushed through the parent's heap-frame crossing slot; union — envelope cell +
tag-switched payload deep-copy cell (`%fig_env_cell`/`%fig_pay1_cell`, null-preserving phi),
staged from the loaded cell pointer; anon — the stack literal is memcpy'd into
`%sm_arg_anon_cell = ynz_alloc(16)` and the CELL address is staged; in all three the dying stack
temp's address never reaches the child. **Step 3d heap-cell LOOP parity VERDICT (resolved, not
assumed): LEAK-BY-SHIPPED-DESIGN CONFIRMED** — a crossing maybe/union binding re-bound per
iteration orphans its cell(s) each pass (probe: 5-iter maybe loop + 3-iter union loop → alloc=12
free=1, gap exactly 11 = 5×1 envelope + 3×2 envelope+payload; Paper-Trace predicted before first
run, matched exactly, stable 4/4 runs). This is the M5 P3/FRAGO 009 never-drop-locals exact-gap
semantics surfacing through this phase's NEW promotion sites — pinned as
`v03_m6_p1c_heap_cell_loop_parity_pins_documented_per_iteration_leak` (integration.rs, fixture
`v0_3_m6_heap_cell_loop_parity.ynz`) so any new leak class or a landed drop story shifts it
loudly. **Disposition SURFACED to the deviation-judge → FRAGO seam (not self-decided):** candidate
four-field deferral — WHAT: per-iteration heap-cell leak for crossing maybe/union loop bindings;
WHY: freeing needs the ownership drop story (roadmap Future Requirements #6 class), out of this
hotfix's charter, same class the M5 FRAGO 009 verdict already ratified as exact-gap-accounted;
COST: the drop-story milestone (1-2 sessions) + updating the two parity pins; TRIGGER: the drop
story landing, or any real workload with an unbounded suspension loop over maybe/union bindings.
**False-positive sweep (step 5) clean:** the committed sweep fixture extended with non-escaping
maybe/union/anon-arg cases (prints 3.5/9/40/square/6; test renamed
`v03_m6_non_escaping_args_of_every_widened_type_are_not_wrongly_affected`, all prior assertions
retained) + IR-verified zero heap-cell promotion sites in the sweep fixture. **Step 4 gates:**
full workspace suite GREEN (`cargo test --workspace --no-fail-fast`, exit 0, count in audit.md
seg-7 entry; M3e determinism holding under load — FRAGO 008's real-sync fix), N=10 in-test
determinism per new repro (maybe/union/anon), `cargo fmt --all --check` clean, house
`cargo clippy --workspace -- -D warnings` clean, AND the two pre-existing ynz-driver test-file
warnings genuinely fixed (integration.rs `m3_return_value_in_nothing` now asserts the diagnostic's
load-bearing terms using the previously-dead `stderr`; cross_impl_consistency's nested if
collapsed) — `clippy -p ynz-driver --tests` reports 0 warnings. The tracked minor WHY-comment
clarification in the sweep test's number half was folded in (segment 2). With Phase 1b's
shape+fixed+number portion, the whole accepted-HIGH R14 interim risk is now closed, fixed bug.

#### Phase 1d — P1-1 sibling: decimal128-across-a-concurrency-boundary defects (background-spawn arg UAF + cpu-member arg ICE) (FRAGO 009; pre-existing v0.3.0)

- **Task + purpose:** close the sibling decimal128 (`number`)-across-a-concurrency-boundary defects
  surfaced by Phase 1b's fix-loop + boundary review (FRAGO 009): (A) a `number` argument to a
  `background`-spawned task is left dangling because `prepare_bg_arg_for_ctx` (`emit.rs:15386`) has
  no `Number` arm — both the CPU spawn arm (`ynz_rt_spawn_blocking`, `emit.rs:15840`) and the SM
  spawn arm (`ynz_rt_spawn`, `emit.rs:16060`) chase a stack-dangling pointer after the spawner
  returns, confirmed LIVE (5/5 deterministic `0.000...` vs `2.5`); (C) a `number` argument into a
  CPU-member spawn ICEs the compiler — `emit_cpu_member_spawn` (`emit.rs:9506-9509`) does
  `build_load(i64)` against a pointer-typed number param, mismatching LLVM's verifier and surfacing
  as "compiler bug" on otherwise-valid code (3/3, fused + pure-CPU); confirmed LIVE, a loud crash,
  not silent. (B) conduit-send decimal128 (`emit.rs:11809`) is VERIFIED-SAFE-BY-GATE —
  `channel<number>` is compile-gated by typeck (`check.rs:3369-3398`) with a teaching error naming
  this exact UAF class, so the Number-send path is unreachable from any current syntax; recorded as
  a Future-Requirements deferral (#12), not fixed here. Both A and C are pre-existing defects in the
  concurrency/auto-parallel charter (fable-probe-confirmed, HEAD `47abd29`), not M6 regressions. Per
  [authoritative-derivation.md](../../../rules/authoritative-derivation.md), this phase does NOT
  invent a new derivation of "does this number arg cross a concurrency boundary" — it extends the
  same crossing-classification discipline Phase 1b/1c already built.
- **Steps**
  1. **DESIGN DECISION (the phase executor's own call — charter: route-not-design, never
     conductor-pre-decided). CCIR-1: re-verify all cited lines against the live tree FIRST**
     (`emit.rs:15386`, `:15840`, `:16060` — `prepare_bg_arg_for_ctx` and its two spawn-arm
     consumers; `emit.rs:9506-9509` `emit_cpu_member_spawn`; `check.rs:3369-3398`
     `channel<number>` compile gate). Then choose the fix approach for A and C from two live
     options, weighed against `IMP-concurrency.md`/`IMP-no-function-coloring.md` and this plan's
     own precedents, and record the choice as a new Recorded Decision entry (mirroring D1–D7's
     format) BEFORE implementing:
     - **Option 1 — gate-consistently with the existing `channel<number>` compile gate**
       (`check.rs:3369-3398`): reject a `number` arg crossing this concurrency boundary with the
       SAME teaching-error class already shipped for `channel<number>` — cheap, consistent, and
       matches Phase 1b's `fixed<T>` precedent (Check 2b: reject rather than build new machinery).
     - **Option 2 — eager i128 heap-copy**: give `prepare_bg_arg_for_ctx` a `Number` arm that
       heap-copies the decimal128 bits before the spawn, so the arg WORKS across the boundary
       instead of being rejected — harder machinery, but would ALSO unlock `channel<number>`
       (removing the existing compile gate) as a side effect.
     Neither option is pre-selected in this plan text — the executor decides at run time and
     records the reasoning.
  2. Lock the RED repro(s) BEFORE the fix (per
     [verification.md](../../../rules/verification.md)), shaped by the chosen option: if Option 1,
     a repro asserting the deterministic teaching COMPILE ERROR (mirroring Phase 1b's `fixed<T>`
     repro); if Option 2, a repro asserting the CORRECT deterministic value across the boundary
     (mirroring Phase 1b's `number` repro). One repro each for (A) background-spawn (both the
     CPU-spawn and SM-spawn arms) and (C) cpu-member spawn. Confirm each fails RED pre-fix for the
     documented reason (garbage / ICE), not some unrelated bug.
  3. Implement the chosen fix for BOTH A (background-spawn, both arms) and C (cpu-member spawn) via
     ONE authoritative check/mechanism — never two ad hoc paths for the same boundary-crossing
     question (authoritative-derivation.md).
  4. Full-regression + determinism/false-positive sweep: RED repros GREEN; if Option 2, N≥10
     determinism proof (same correct value every run); full workspace suite green
     (`docker compose run --rm dev cargo test --workspace`); house clippy `-D warnings` clean;
     confirm no non-boundary-crossing `number` value is wrongly affected by the fix.
  5. Record B (conduit-send decimal128, `emit.rs:11809`) as Future-Requirements deferral #12:
     WHY — verified-safe-by-gate, unreachable today (`channel<number>` compile-gated); COST — small
     once `channel<number>`'s heap-copy machinery ships (reuses this phase's Option-2 mechanism
     directly if Option 2 was chosen; needs its own pass if Option 1 was chosen); TRIGGER —
     `channel<number>` heap-copy ships / a real workload needs it.
- **Exit criteria:** A + C closed via the phase executor's chosen, recorded design decision (Option
  1 or Option 2, never pre-decided in this plan text); RED→GREEN repro(s) committed for both;
  false-positive sweep clean; full suite green; B recorded as a proper four-field
  Future-Requirements deferral (#12).
- **Reviewer fan-out:** code-reviewer (the chosen fix's diff); adversarial gate-checker (does the
  repro genuinely exercise the background-spawn AND cpu-member boundary, and is any determinism
  proof non-vacuous?); design-doc-alignment reviewer (authoritative-derivation.md compliance, and
  that the recorded design decision is genuinely weighed against IMP-concurrency.md/
  IMP-no-function-coloring.md rather than asserted).
- **Model tag:** `(coding, high, medium)`

**Phase 1d complete (executor `executor-2026-07-10-m6-phase1d-seg3`, 2026-07-10; segments 1-3):**
the sibling decimal128-across-a-concurrency-boundary defects A + C are CLOSED via **Decision D8 =
Option 2 (eager decimal128 heap-copy at the spawn boundary)**, through ONE authoritative mechanism
(authoritative-derivation), all in `crates/ynz-codegen/src/emit.rs`: new `Cg::number_to_heap_cell`
(the single 16-byte boundary-cross copy both defects consume, beside `shape_bytes_to_heap_cell`) —
**Defect A** (background-spawn `number` arg UAF) via an unconditional `Type::Number{precision<=34}`
pre-gate in `prepare_bg_arg_for_ctx` returning `(cell, HeapShape{16})`, freed by BOTH spawn arms
(CPU-spawn `emit_bg_arg_frees` HeapShape + SM-spawn `BgArgDropEntry` kind-0; SM child-side
`sm_number_param_set` read unchanged, now derefing the heap cell); **Defect C** (cpu-member `number`
arg ICE) via the new first-param twin predicate `callee_takes_bare_number` consumed by BOTH
`emit_cpu_member_spawn` (stages the heap-cell pointer as the ctx word) AND `build_cpu_trampoline`
(reconstructs the pointer, passes it to the callee, frees the cell AFTER result packing — one
alloc / one free). N>34 bignum deliberately untouched. **8/8 locked RED repros GREEN** (value +
N=10 determinism, A CPU-arm/SM-arm + C pure-spike/fused). **Step 4 gates (segment 3):** confirming
full workspace suite **GREEN — `cargo test --workspace` exit 0, 2315 passed / 0 failed** (M3e
determinism holding under load: `cross_impl_consistency` 2/2 GREEN, 251s); `cargo fmt --all --check`
clean (one seg-2 leftover line in `emit.rs` formatted via `cargo fmt -p ynz-codegen` — no
`#[allow]`, no `--no-verify`); house `cargo clippy --workspace -- -D warnings` clean. **First
full-suite run had ONE transient flake** — `current_rss_bytes_returns_value_on_supported_platforms`
(`crates/ynz-watch/tests/long_session.rs`, a live-process RSS `mb > 0` assertion) failed with
"got 0MB"; it is in `ynz-watch` (explicitly not-this-executor's, dirty from a separate change), in
UNMODIFIED source (`memory.rs` + the test file both clean; the dirty `error.rs`/`rebuild.rs` don't
touch RSS polling), structurally unrelated to a codegen change in a different crate, passed 3/3 on
isolated re-run AND did NOT recur on the confirming full-suite re-run — surfaced as a deviation, not
this executor's to fix. **False-positive sweep — scope stated honestly (wording corrected in fix-loop round 1):** the
committed `v03_m6_non_escaping_args_of_every_widened_type_are_not_wrongly_affected` test GREEN,
PLUS the IR-level proof that a non-escaping `number` arg is NOT heap-copied — ZERO number heap-cell
markers (`bg_number` / `_num_ld` / `_num_bits_` / `spike_num_arg_ptr` / `number_to_heap_cell`) in
the IR, and the non-escaping `bump(x)` lowering to a plain by-pointer `call ... @bump(ptr ...)` —
now PERSISTED as the committed test `v03_m6_non_escaping_number_ir_stays_by_pointer`
(`crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs`), not a session-local `--emit-ir` run.
Scope: the sweep fixture contains ZERO spawn calls, so this sweep pins CODE-PATH DISJOINTNESS
(Phase 1b/1c's classifier + normal call lowering untouched by the Phase 1d machinery) — it does
NOT exercise Phase 1d's boundary discrimination. No in-boundary false-positive case exists under
Option 2's design: the `Type::Number{precision<=34}` pre-gate is UNCONDITIONAL at the spawn
boundary (every `number` arg that reaches it is heap-copied by intent), and in-boundary
correctness is asserted by the 8 value/N=10-determinism repros, not by the sweep. **Step 5:** Future-Requirements #12
(conduit-send decimal128) tightened — stale `check.rs:3369-3398` → live `check.rs:3417-3451`, COST
narrowed to state D8/Option 2 was chosen (reuses `number_to_heap_cell`, still needs its own conduit
send/recv marshalling pass; the `channel<number>` gate stays UNTOUCHED per D8 reason 5); new
Future-Requirements #14 records the surfaced **IntLit→`number`-param call-site coercion gap** (ICEs
even synchronously; the sibling call-site facet of #9's store-site ICE — same root class, different
site; scoped OUT of FRAGO 009's concurrency charter — the seg-3 guard's "clean compile error" was in
fact the generic ICE banner, corrected in fix-loop round 1 to a typeck teaching error, below)
as a candidate four-field deferral SURFACED for the deviation-judge → conductor seam (NOT
self-adjudicated; since RATIFIED as #14 via FRAGO 016, below). With Phase 1b (shape/fixed/number) and Phase 1c (maybe/union/anon), the whole
R15/FRAGO-009 interim risk is now closed, fixed bug. **DEVIATIONS SURFACED for the seam (NOT
self-decided):** (1) the IntLit→`number` call-site coercion scope-out (candidate #14); (2) the
transient not-mine ynz-watch RSS flake on the first full-suite run (unmodified source, did not
recur).

**Phase 1d fix-loop round 1 (executor `executor-2026-07-10-m6-phase1d-fixloop1`, 2026-07-10 —
post-review should-fixes; correctness already verified clean, 0 blockers):** (1) the interim
IntLit→`number` cpu-member guard's diagnostic REROUTED from the generic ICE banner to a proper
typeck teaching error — new WHAT/WHAT-INSTEAD/WHY diagnostic in `check_user_fn_call`
(`crates/ynz-typeck/src/check.rs`, mirroring the `channel<number>` gate convention, the codebase's
one established home for known-limitation compile errors; codegen has no user-facing diagnostic
path — every `emit_artifact` `Err(String)` hits `queries.rs`'s "compiler bug" wrapper), firing on
`f(5)` synchronous AND at the spawn boundary (the round-1 "EVERY call site uniformly" wording
overclaimed — the UFCS and generic-fn forms still bypassed it; corrected in fix-loop round 2,
below, which made the uniformity real); "FRAGO 009" and the raw `{span:?}` dump dropped from all user-visible
text; the codegen arm (`emit.rs` `emit_cpu_member_spawn`) retained as an unreachable internal
backstop naming the typeck gate. (2) The guard is now TESTED: RED confirmed against the pre-change
binary (ICE banner + FRAGO leak reproduced verbatim), GREEN post-change —
`v03_m6_int_literal_to_number_param_cpu_member_is_clean_teaching_error` (new fixture
`v0_3_m6_int_literal_number_arg_cpu_member.ynz`, the admitted-spike-group shape with `heavyGrow(5)`)
asserts non-zero exit, no hang, the teaching message present, and NEITHER "compiler bug" NOR "FRAGO"
in stderr. (3) Both narrative overclaims corrected (this note above + audit.md seg-3), and the
IR-level proof persisted as `v03_m6_non_escaping_number_ir_stays_by_pointer`. (4) Future-Req #14
RATIFIED per the deviation-judge's JUSTIFIED/risk-neutral verdict → **FRAGO 016** (fix routed to the
existing stub plan `2026-07-04-v0-3-hotfix-int-literal-number`, to be expanded — by the conductor's
separate coordination item, NOT by this plan — to cover both the #9 store-site and #14 call-site
facets under ONE coercion mechanism); the two code-reviewer polish minors filed as Future-Req #15
(`callee_takes_bare_number`/`callee_returns_bare_number` shared-scan-helper consolidation) and #16
(named shared const for the decimal128 16-byte cell size across the alloc/free ladder).

**Phase 1d fix-loop round 2 (executor `executor-2026-07-10-m6-phase1d-fixloop2`, 2026-07-10 —
post-review should-fixes; correctness verified clean, 0 blockers):** (1) round 1's "fires on EVERY
call site uniformly" claim was FALSE — code-reviewer proved two user-reachable call forms bypassed
the typeck gate and still ICE'd: the UFCS dot-call `p.scale(5)` (the MethodCall arm inferred
non-receiver args with no hint; `check_method_call` checked only receiver-vs-first-param — ALSO
breaking non-oop.md's identical-diagnostics-between-call-forms convention) and a generic fn's
concrete `number` param `scale(5, item)` (`check_generic_fn_call` discarded the `unify_param`
mismatch). Both locked RED against the pre-fix tree (UFCS: LLVM verifier reject `i64 5` vs pointer
param → "compiler bug" banner; generic: IntValue-vs-PointerValue internal panic banner), then the
gate was extracted into ONE shared `reject_int_literal_number_arg` helper (`check.rs`,
authoritative-derivation — no per-form twins) consumed by all THREE arg loops (plain
`check_user_fn_call`, UFCS `check_method_call` shape arm with args now threaded through, generic
`check_generic_fn_call`); RED→GREEN via
`v03_m6_int_literal_to_number_param_{ufcs,generic_fn}_is_clean_teaching_error` (new fixtures
`v0_3_m6_int_literal_number_arg_{ufcs,generic_fn}.ynz`), byte-identical diagnostic text across all
three forms — the uniformity claim (and `emit.rs`'s backstop comment, updated to name the shared
helper) is now TRUE. Gate keyed on exactly `(Type::Number, Expr::IntLit)`: `f(5.0)`, number idents,
and `f(-5)` (UnaryOp) stay untouched (full-suite false-positive sweep green). NOTE (pre-existing,
not introduced or fixed here): the diagnostic's rendered span is one line off for this error class
in ALL call forms (observed identically on the round-1 plain-form fixture) — an IntLit
span-rendering nit that dies with the interim guard when #14's coercion ships. (2) The untracked
trampoline arg-cell shutdown-drop leak (round-1's "noted, not fixed here" code comment,
`emit.rs:~9708`) formalized as four-field **Future-Req #17** + added to Phase 8's FRAGO-012
lift-list (FRAGO 017). (3) The IR-proof test hardened per test-quality: each negative marker now
has a POSITIVE control (the same marker asserted present in a boundary fixture's IR, cached
per-fixture builds) so a codegen rename flips the control red instead of silently hollowing the
sweep; the inert `number_to_heap_cell` marker (a Rust fn name, never an emitted IR value) dropped —
probe also proved `spike_num_free` inert (LLVM drops names on void calls), so the marker list is
exactly the four proven-emitted names (`bg_number`, `_num_ld`, `_num_bits_`, `spike_num_arg_ptr`).

**Phase 1d fix-loop round 3 — the guard COMPLETION sweep (executors
`executor-2026-07-10-m6-phase1d-fixloop3` [enumeration] + `-seg2` [RED lock + implement] +
`-seg3` [gates + close-out], 2026-07-10 — SCOPE EXPANSION, FRAGO 018).** Rounds 1–2 upgraded the
interim IntLit→`number` guard and extended it across the three *call-argument* forms (plain / UFCS /
generic). Round 3's enumeration then found the class was far wider than the call forms — an
exhaustive slot sweep (27 `infer_expr(_, Some())` sites classified, grep-completeness-argued: no
`Paren` AST node, zero `number`-param intrinsics, all named/cross-module calls routing through the
gated fns) surfaced REAL user-reachable danger beyond the three gated call forms: `array<number>.add(5)`
**segfaults (exit 139)**, `contains(5)` returns a **SILENT wrong `false` (exit 0)** — worse than a
crash — and ~24 further arg / construction / statement slots ICE or silently corrupt. The human chose
**complete the guard across EVERY IntLit / `-IntLit` → `number` slot** over defer, justified by that
finding. **Mechanism (one authoritative gate, authoritative-derivation):** `reject_int_literal_number_arg`
became a thin wrapper over the new role-parameterized `reject_int_literal_number_slot(NumberSlotRole, …)`
(`crates/ynz-typeck/src/check.rs`); a `-IntLit` (`UnaryOp{Neg, IntLit}`) arm added; the generic case
moved to ONE post-arg-loop `apply_substitution` pass (concrete + explicit `pass<number>(5)` +
sibling-bound `pick(price, 5)`, no double-emit, unresolved TypeParams never match Number); a single
authoritative `collection_method_arg_slots(receiver, method)` table (`crates/ynz-typeck/src/builtins.rs`,
beside the method-surface tables) drives ONE gate loop over the collection-method element positions;
and the hinted construction / statement slots (struct-lit field + map-hint twin, array/fixed literal
elements, map-literal key+value, index assigns incl. the map KEY, map bracket-key READ, field assign,
`return`, multi-case-if arm pattern) gate through the same slot fn. The role varies ONLY the WHAT
subject+noun and the WHAT-INSTEAD closing clause; the core teaching text stays byte-identical across
slots (call-form text byte-identical to round 2 save one recorded WHY-clause wording change,
"at a call site" → "automatically"). **24 committed RED fixtures + 25 committed tests** (24 slot tests +
false-positive sweep) in `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs`, all 24 flipped
RED→GREEN (teaching error, non-zero exit, no ICE banner). **Covered set is now COMPLETE for the
IntLit / `-IntLit` → `number` argument / construction / statement class. The two STORE sites
(`let x: number = 5` local binding; `hidden f: number = 5` field default) are ALSO rejected with the
SAME teaching error as of the v0.3-M6 store-site stopgap (FRAGO 020, human-directed "no duct tape",
2026-07-10): the REJECTION now covers every facet uniformly — only the int→number COERCION (#9/#14)
remains deferred to the `2026-07-04-v0-3-hotfix-int-literal-number` stub plan** (controls `x = 5`
reassign and `let x: number = 5.0` / `number`-typed variables stay clean, confirmed). Full step-4
gates GREEN: `cargo nextest run --workspace`
**2344 passed / 0 failed** (M3e `cross_impl_consistency` both GREEN under load, no flake fired);
`cargo clippy --workspace -- -D warnings` clean; `cargo fmt --all --check` clean. **FOUR deviations
surfaced to the deviation-judge → conductor seam (NOT self-adjudicated; recorded by FRAGO 018):**
(1) enumeration-completeness amendment — two map-bracket-KEY sibling slots the seg-1 table missed
(index-assign `names[5] = …` and index-read `names[5]`, both probe-confirmed silent type-corruption),
fixed IN-CLASS this round + tested; (2) NEW pre-existing decimal128 **by-value RETURN** garbage from a
synchronous user fn (nondeterministic, `print(toll(5.0))`), out of charter — candidate Future-Req;
(3) NEW pre-existing `map<number, V>` real-number-literal-key silent breakage (keys hash/compare by
pointer identity; `set(1.5,…)` then `get(1.5)` → `none`, exit 0), out of charter — candidate
Future-Req; (4) unchanged out-of-scope gaps from prior rounds — the general UFCS arg-validation gap
(`a.concat([5])` / `pick(5, price)`, number→int direction) and `array.remove` having no codegen
lowering arm for ANY type.

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
  `emit.rs:12205-12208` (bare-channel `.send()`) AND the handle-pointer token minted inside
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
  5. Implement the generation-salted `caller_token` covering BOTH producers, with the generation
     half joined RUNTIME-side (FRAGO 021 — the codegen mint sites stay untouched: the frame header
     is fully packed, so a codegen-side generation store would require frame-layout ABI surgery,
     and a salted token must be stable across re-polls of the same suspension): key
     `pending_sends` by `(caller_token, caller_generation)` from ONE global monotonic counter
     (`channel::next_caller_generation`; generation 0 reserved for bare unstamped ABI calls
     (substrate tests) and never mass-purged — the entrypoint sync drives are themselves stamped
     NONZERO per FRAGO 022, review fix-loop 1) minted at caller-identity birth — `task_gen` on
     `SpawnStateFnFuture`
     (every construction site), published via a thread-local RAII guard around the resume-fn call
     so the extern-C send ABI is byte-identical (root + embedded-child + chain-child frame tokens
     all carry it, a producer variant the original step text never enumerated); `send_gen` on
     `YnzTaskHandle`, passed explicitly by `ynz_handle_send_poll` — both through ONE keyed core
     (`channel_send_poll_guarded`), never two independently-salted token shapes — so a reused
     address (frame OR handle) cannot collide with a stale entry even inside the purge's own race
     window; plus an insert-time same-token/different-generation stale sweep as the missed-path
     leak backstop (two LIVE identities can never share a token address).

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

**Phase 3 complete (executor `executor-2026-07-10-m6-phase3-seg3`, 2026-07-10; segments 1-3):**
the P3-1 `caller_token` ABA class and the P2-2 orphaned `pending_sends` leak are CLOSED across
BOTH token producers via BOTH D2 mitigations, all runtime-side in
`crates/ynz-runtime/src/{channel.rs,runtime.rs,handle.rs}` (emit.rs untouched — zero codegen
delta). **(a) Purge:** ONE shared helper `purge_pending_sends(chan_ptr, generation)`
(null-safe, gen-0 no-op, purge-by-generation `retain`, idempotent by construction) wired into
BOTH cancellation paths — the drop ladder's kind-2 `BgArgDropEntry` arm (`runtime.rs`, BEFORE
`ynz_channel_free`) and `ynz_handle_free` (`handle.rs`, BEFORE releasing the conduit ref);
idempotency proven on both (repeated-cancel assertions + dedicated double-purge/purge-empty/
purge-null/gen-0 unit test). **(b) Salt:** ONE runtime-side generation scheme (FRAGO 021 —
runtime-side join, frame header packed): `pending_sends` keyed `(caller_token,
caller_generation)` from ONE global counter (`channel::next_caller_generation`, gen 0 reserved
for bare unstamped ABI calls only — FRAGO 022 stamped the entrypoint sync drives nonzero in the
review fix-loop); `task_gen` on `SpawnStateFnFuture` (all 3 construction sites) published via
`TaskGenGuard` thread-local RAII around every poll (extern-C send ABI byte-identical; covers
root + embedded-child + chain-child frame tokens); `send_gen` on `YnzTaskHandle` passed
explicitly by `ynz_handle_send_poll`; both mints through the ONE keyed core
`channel_send_poll_guarded` (authoritative-derivation — one scheme, never two); plus the
insert-time same-token/different-generation stale sweep as missed-path leak backstop.
**RED→GREEN pair committed to the tree** (`mod m6_pending_send_aba` in `lib.rs`: frame-path
through the REAL drop ladder with forced address reuse; handle-path through real
`ynz_rt_spawn_handle`/`ynz_handle_free`) — authored + proven RED segment 1 (Paper-Trace:
`pending_send_count` observed 1 post-cancel, expected 0), flipped GREEN by the fix segment 2 —
plus 2 deterministic unit tests (idempotency; same-token/different-generation collision).
**Step 7 gates (segment 3): full workspace `cargo nextest run --workspace` GREEN — 2354
passed / 0 failed / 6 skipped, exit 0** (= 2350 stopgap baseline + the 4 new Phase 3 tests, all
4 confirmed in-set and GREEN in the full run), ZERO flakes this run (the known transients —
`long_session` RSS, `contract_3_wait_inside_if`, `spawn_panic_ctx_no_leak` — all passed first
attempt); M4 channel/handle suites regression-free (0 failures workspace-wide); house
`cargo clippy --workspace -- -D warnings` exit 0; `cargo fmt --all --check` exit 0. No
`#[allow]`, no `--no-verify`, no weakened test; the 2 pre-existing `--all-targets`-only lints
(`tests/m2_runtime.rs:275`, `lib.rs:2830`, Phase-1d-era) left untouched per record. Exit
criteria ALL MET. FRAGO 021 (seg-1 deviations: citation drift + runtime-side salt refinement)
filed segment 2, pending boundary deviation-judge ratification.

**Phase 3 review fix-loop 1 (executor `executor-2026-07-10-m6-phase3-fixloop1`, 2026-07-10):**
two converged reviewers (code-reviewer + critical-path-integrity: 0 blockers — ABA closed for all
gen≠0 paths) returned 2 should-fixes; both CLOSED. **(1) gen-0 unprotected class ELIMINATED**
(preferred path — not the debug_assert+defer fallback, so nothing is deferred): the sync
entrypoint driver (`SyncStateFnFuture` / `ynz_rt_run_entrypoint`) now mints its own NONZERO
`task_gen` from the ONE counter and publishes it via the same `TaskGenGuard` around every poll —
EVERY production caller identity (spawned task, handle, sync drive) is stamped nonzero, the
purge/re-poll/sweep logic is uniform, and no unprotected gen-0 class exists in ANY build (the
release binary on the trading mount gets the protection compiled in). Verify-before-fix
Paper-Trace confirmed the reviewers' premise and found the class WIDER than stated: gen 0 covered
every `SyncStateFnFuture` drive (the codegen `main` wrapper AND every non-entry sync wrapper
called from a non-state-machine context — one shared driver, `runtime.rs`), all sharing ONE
identity; the eliminate path closes the whole class. Generation 0 remains reserved solely for
bare unstamped ABI calls (substrate tests); `purge_pending_sends`'s gen-0 no-op stays as the
never-mass-purge floor. Zero ABI/codegen delta (extern signatures byte-identical; `emit.rs`
untouched). **(2) deterministic handle-path ABA proof added:**
`handle::tests::handle_send_same_address_different_generation_never_collides` drives the REAL
`ynz_handle_send_poll` mint at ONE handle address under two explicit generations with the purge
withheld (the residual race window), asserting no key collision, the insert-time stale sweep, and
the new generation's value delivering — a broken salt fails it (the dead generation's 111 would
deliver instead of the live 222); the `lib.rs` handle repro's best-effort reuse loop is retained
with its fallback note repointed at the new deterministic test. **Gates ALL GREEN:**
`cargo nextest run --workspace` **2355 passed / 0 failed / 6 skipped, exit 0** (= 2354 Phase-3
baseline + the 1 new handle test; all 5 ABA/purge tests additionally confirmed green in a
targeted run). One unrelated PRE-EXISTING wall-clock perf test outside this phase's scope
(`ynz-typeck::symbol_lookup::test_cross_file_reference_count_estimate_completes_fast`, asserts
<5ms) failed once under full parallel load in the first run, passed in isolation (0.006s) and in
the full rerun — a non-recurring load transient in an untouched crate, surfaced for the record,
NOT silenced and NOT a runtime regression. `cargo clippy --workspace -- -D warnings` exit 0;
`cargo fmt --all --check` exit 0. No `#[allow]`, no `--no-verify`, no weakened test. FRAGO 022
records the step-5 gen-0-class refinement (pending boundary deviation-judge ratification).

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
  corrected; Phase 3b's own work is green (recursion-chain spike-handle leak closed; RED→GREEN
  repro GREEN; stale comment corrected). One corroborated-PRE-EXISTING orthogonal failure
  (`v03_m6_ufcs_background_spawned_method_call_runs`) remains — NOT caused by or curable within
  Phase 3b (reproduces at clean committed `fc7797f` with zero Phase-3b changes); tracked as a
  separate flagship milestone finding per FRAGO 023 (Deviation 2), under investigation this
  session, not deferred.
- **Completion note (2026-07-10, `executor-2026-07-10-m6-phase3b-seg2`):** COMPLETE. Leak closed by
  threading the SAME `cleanup_spike_cpu_handles` the root frame uses into the recursion-chain drop
  walk (one authoritative choke point, `runtime.rs`). RED→GREEN proven non-vacuously via new
  env-gated `CpuJoinHandle` parity counters (`handle_alloc=`/`handle_free=` lines, same
  `YNZ_ALLOC_COUNTER` gate): pre-fix `handle_alloc=4, handle_free=2` (integration) + drop-probe
  `0≠1` (unit); post-fix `4/4` + probe `1` — with positive control (`handle_alloc>=4`, confirming
  the Phase-0 reachability claim), durable negative control (`YNZ_SKIP_RECURSION_DROP=1` still
  leaks post-fix), and frame alloc=free regression guard. Stale `queries.rs:941-944` comment
  corrected. Full suite 2359/2360 passed (+5 new tests, 6 skipped); the single failure
  (`v03_m6_ufcs_background_spawned_method_call_runs`) is PRE-EXISTING — bisect-proven present with
  this phase's source diff fully reverted and the runtime re-embedded — adjudicated JUSTIFIED +
  risk-neutral by the deviation-judge and recorded as FRAGO 023 (exit-criterion #4 reframed
  accordingly; Deviation 2 tracks the flagship finding, under investigation this session, not
  deferred). Clippy/fmt clean. **Boundary review (2026-07-10):** reviewer fleet returned 0
  blockers (code-reviewer clean; critical-path-integrity clean; test-quality MEANINGFUL;
  rules-compliance 0-blocker; graveyard clean) — fix + repro delivered as specified.
  **Fix-loop round (`executor-2026-07-10-m6-phase3b-fixloop1`):** applied the fleet's one
  should-fix — `YNZ_SKIP_RECURSION_DROP` is now latched once at `ynz_rt_init` into a `static
  AtomicBool` read via a relaxed load in `SpawnStateFnFuture::drop` (same authoritative cached-flag
  pattern as `ALLOC_COUNTER_ENABLED`; behavior identical, per-drop env read removed from the
  task-drop hot path) — plus a timing-triage comment on the wall-clock cancel-timing integration
  tests (flakes = timing-margin drift; the deterministic unit test
  `recursion_chain_child_spike_handles_freed_on_drop` is the primary timing-independent proof).
  Gates re-run post-cleanup: build green; nextest 2359 passed / 1 failed (the tracked pre-existing
  `v03_m6_ufcs_background_spawned_method_call_runs` only) / 6 skipped; clippy `-D warnings` clean;
  fmt `--check` clean. Phase 3b meets its (reframed) exit criteria.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the repro genuinely exercise
  the nested-branch-arm self-recursive-host cancellation window, or a broader/different leak?);
  design-doc-alignment reviewer (authoritative-derivation.md — one cleanup choke point threaded to
  root + chain children, not a second path).
- **Model tag:** `(coding, high, medium)`

#### Phase 3c — FRAGO 024: `background ship.haul()` UFCS-receiver use-after-free (flagship, Key Outcomes 1 & 8)

- **Task + purpose:** close the CONFIRMED use-after-free (FRAGO 024 — root-caused this session,
  IR-proven, git-bisected) in the flagship `background ship.haul()` deliverable. Root cause:
  `background ship.haul()` is an `Expr::MethodCall` (UFCS), and the statement-level background
  give/copy ownership inference (`crates/ynz-typeck/src/check.rs:1388`) matches only `Expr::Call`,
  so the receiver ident `ship`'s span is NEVER inserted into `background_arg_inferred_ownership`.
  Downstream, `prepare_bg_arg_for_ctx` (`crates/ynz-codegen/src/emit.rs:15896-15913`) gates the
  Shape heap-upgrade on span membership → absent → the receiver rides into the task as a RAW
  POINTER to `entrypoint`'s resume-fn stack frame; when the entrypoint returns Pending at
  `wait sleep(120)` that frame dies and the spawned task reads `self.name` from freed stack
  (empty/garbage/crash). Congenital to `f921efe` (M6 P1-1), timing-masked through `46906d1`,
  deterministically exposed at HEAD by `070beca`'s TaskGenGuard timing perturbation. Contrast: the
  Call-form `background haul(ship)` heap-upgrades correctly — the only delta is the missing
  receiver upgrade. Squarely the flagship's Key Outcomes 1 & 8 correctness charter.
  **FRAGO 025 (fix-loop round 1) extends this phase to BOTH spawn forms of the same UAF class:**
  the HANDLE form `let h = background ship.haul()` (security-reproduced live: compiles clean with
  zero diagnostics, `check_background_handle_spawn` registers ownership / resolves the callee only
  for `Expr::Call` inners, so codegen spawns the receiver without the heap-upgrade — raw pointer
  into the dead spawner frame), plus a should-fix in the fix's own code: the `receiver_is_shape`
  predicate read a raw `scope.lookup` that is NOT narrowing-aware (a union receiver narrowed to a
  shape variant returned the un-narrowed union → predicate false → the same UAF one subcase over).
- **Steps**
  1. CCIR: re-verify the cited anchors against the live tree (`check.rs:1387-1442` inference
     block; `emit.rs:15896-15913` heap-upgrade gate; `emit.rs:16211` `synthesize_ufcs_call_expr`).
  2. Confirm the locked RED pre-fix on the unmodified tree:
     `v03_m6_ufcs_background_spawned_method_call_runs` fails (verify-before-fix; the pre-existing
     test IS the lock — never weaken or edit it to pass).
  3. Fix: extend the `check.rs:1387-1442` background give/copy inference block to ALSO match
     `Expr::MethodCall { receiver, args, .. }` when the receiver is a shape (a UFCS call) —
     normalize the spawn target to `[receiver, ...args]` (the typeck twin of codegen's EXISTING
     `synthesize_ufcs_call_expr` normalization at `emit.rs:16211`; never a second normalization
     scheme) and run the IDENTICAL per-plain-ident inference loop that inserts each ident span
     into `bg_inferred` / `background_arg_inferred_ownership`. The existing codegen Shape arm then
     heap-upgrades the receiver exactly as the Call form already does — NO codegen change
     ([authoritative-derivation.md](../../../rules/authoritative-derivation.md): thread the one
     inference path; no ad-hoc receiver pre-gate).
  4. If cheap and honest, add one sibling coverage case locking the class (a give-transferred
     multi-field receiver read past its first field after the task's own suspension, or a
     copy-inferred receiver); if it needs new machinery, record the candidate and skip.
  5. Gates (docker, nextest): forced runtime-then-driver rebuild (the driver `include_bytes!`s
     `libynz_runtime.a` — never test a stale embed); full `cargo nextest run --workspace`;
     `cargo clippy --workspace -- -D warnings`; `cargo fmt --all` (+ `--check`).
  6. (FRAGO 025, fix-loop round 1) Close the HANDLE-form twin: thread the SAME Call-form
     normalization into `check_background_handle_spawn` — register the shape receiver's span in
     `bg_inferred`/`background_arg_inferred_ownership` AND resolve `callee_name` from the
     MethodCall's method, via ONE shared normalization helper both spawn forms consume (never a
     second derivation); retire the stale "already diagnosed by the Background arm" comment. Add a
     handle-form RED→GREEN repro beside the statement-form tests.
  7. (FRAGO 025, fix-loop round 1) Harden the shape-receiver predicate at BOTH spawn sites: consume
     the narrowing-aware authoritative type source (`union_narrowed` overlay over scope — the same
     overlay order `resolve_ident` applies, via one shared side-effect-free read), never a raw
     `scope.lookup`. Add a narrowed-union repro if cheap; if it needs new machinery, record the
     candidate and skip.
- **Exit criteria:** the UAF is closed in BOTH spawn forms via the ONE authoritative typeck
  inference path (one spawn-target normalization, no second derivation; no codegen change);
  `v03_m6_ufcs_background_spawned_method_call_runs` flips RED→GREEN un-weakened, and the
  handle-form repro (`v03_m6_ufcs_background_handle_receiver_survives_spawner_frame`) flips
  RED→GREEN un-weakened; the shape-receiver predicate reads the narrowing-aware source at both
  sites; the full suite returns to 0 real failures (retiring the FRAGO 023 exit-criterion
  reframe — the tracked pre-existing failure is fixed, not tracked); no regression in the existing
  Call-form background fixtures (give/copy/channel arms) or the Phase 3b recursion-spike suite.
- **Completion note (2026-07-10, `executor-2026-07-10-m6-phase3c`):** COMPLETE. UAF closed in the
  ONE authoritative typeck inference path: the `check.rs` statement-level background give/copy
  block now normalizes the spawn target to its Call-form argument list (`Expr::Call` args as-is;
  shape-receiver `Expr::MethodCall` → `[receiver, ...args]`, the typeck twin of codegen's existing
  `synthesize_ufcs_call_expr`) and runs the IDENTICAL per-plain-ident inference loop over it — the
  receiver's ident span now enters `background_arg_inferred_ownership`, and codegen's existing
  Shape arm heap-upgrades it exactly as the Call form. ZERO codegen change; no second derivation;
  no ad-hoc receiver pre-gate. Verify-before-fix: RED re-confirmed pre-fix on the live tree with a
  fresh runtime embed (flagship test FAILED, fixture exit 1 / lost `Mon` output); GREEN post-fix.
  Sibling coverage added (small, no new machinery):
  `v03_m6_ufcs_background_give_receiver_multifield_survives_spawner_frame` + fixture
  `v0_3_m6_ufcs_background_multifield.ynz` — the task reads BOTH receiver fields (string + int)
  after its own suspension, locking the whole-struct heap upgrade. Gates (docker, nextest): full
  `cargo nextest run --workspace` **2361 run / 2361 passed / 0 failed / 6 skipped** (baseline
  2360 + 1 new test — 0 real failures; the flagship test and the Call-form background +
  Phase 3b recursion-spike suites all green); clippy `--workspace -- -D warnings` clean;
  `fmt --all` + `--check` clean. The locked RED test untouched/un-weakened. Three sibling gaps
  SURFACED for the deviation-judge → conductor seam (not self-adjudicated, no fix applied):
  (1) handle-form `let h = background ship.haul()` silently types the handle `Type::Error` —
  `check_background_handle_spawn` resolves no callee for a MethodCall inner and its "already
  diagnosed by the Background arm" comment is stale (the Background arm ACCEPTS UFCS since P1-1);
  (2) a NON-plain-ident shape receiver/arg (`background fleet.flagship.haul()`, and equally the
  Call-form `background haul(fleet.flagship)`) still rides membership-less as a raw pointer — a
  PRE-EXISTING class shared by both spawn forms, not introduced or widened here, needing
  field-projection give/copy machinery; (3) the large-copy Tier-3 warning loop is
  `Expr::Call`-only (teaching-parity gap for a copy-inferred UFCS receiver). All three need new
  machinery/decisions — candidates, not silent fixes.
- **Fix-loop round 1 completion note (2026-07-10, `executor-2026-07-10-m6-phase3c-fix1`, FRAGO
  025):** COMPLETE. **Deviation 1 (handle-form UAF blocker) CLOSED via the one authoritative
  path:** the spawn-target normalization is now ONE shared helper
  (`Checker::background_spawn_call_form` — the typeck twin of codegen's
  `synthesize_ufcs_call_expr`) consumed by BOTH spawn forms; `check_background_handle_spawn`
  pre-records ownership over the normalized `[receiver, ...args]` list (receiver span enters
  `background_arg_inferred_ownership` → codegen's existing Shape arm heap-upgrades it, zero
  codegen change) AND resolves `callee_name` from the normalization (UFCS callee = method name),
  retiring the stale "already diagnosed by the Background arm" comment. Verify-before-fix: the new
  handle-form repro `v03_m6_ufcs_background_handle_receiver_survives_spawner_frame` (+ fixture
  `v0_3_m6_ufcs_background_handle.ynz`) FAILED pre-fix with the exact security-reproduced
  signature (compiled clean, stdout `"\n7\ndone\n"` — empty `self.name` read from the dead
  spawner frame), PASSES post-fix (`"Mon\n7\ndone\n"`). **Deviation 2 (narrowed-union predicate
  should-fix) CLOSED at both sites:** the shape-receiver predicate now reads
  `Checker::binding_ty_narrowed` (the `union_narrowed` overlay over scope — the same overlay
  order `resolve_ident` applies; `resolve_ident`'s narrowing head now delegates to the same
  helper, so the readers cannot drift) instead of a raw `scope.lookup`; both spawn forms consume
  it through the one normalization helper. Narrowed-union repro probed live and recorded as a
  CANDIDATE, not a test: post-hardening the narrowed spawn compiled, registered ownership, and
  heap-upgraded — **re-classified by the round-2 re-review fleet as a confirmed reachable
  out-of-bounds read (CWE-125), fix-introduced by this round's predicate hardening** (the Shape
  heap-upgrade loads `sizeof(shape)` ≥ 64 bytes from the 16-byte `{tag,data}` union storage;
  probe printed `0` for `radius: 5.0`; 48+ byte over-read, IR-reproduced). Fail-closed REJECTED
  in fix-loop round 2 (FRAGO 026, next note); the durable payload extraction is Future
  Requirements #21.
  Statement-form flagship + multifield tests and all Call-form background fixtures stay green
  un-weakened. Gates (docker, nextest): `cargo nextest run --workspace --no-fail-fast`
  **2362 run / 2361 passed / 1 failed / 6 skipped**, the 1 = `ynz-runtime::m2_spike
  sync_bridge_overhead_measurement`, a wall-clock overhead-measurement flake under full parallel
  load that PASSES in isolation on rerun (same brittleness class as the known `symbol_lookup`
  flake) → **0 real failures** (baseline 2361 + 1 new test); clippy `--workspace -- -D warnings`
  clean; `fmt --all` + `--check` clean. FRAGO 025 deviations 3 & 4 echoed to Future Requirements
  #23/#22 as four-field entries (deviation 3 remains flagged for the milestone-seal human call;
  renumbered #21 → #23 by FRAGO 026's rescope, which reserves #21 for the narrowed-union durable
  extraction split out of it).
- **Fix-loop round 2 completion note (2026-07-10, `executor-2026-07-10-m6-phase3c-fix2`,
  FRAGO 026):** COMPLETE. The narrowed-union background receiver — the confirmed reachable OOB
  read (CWE-125), fix-introduced by round 1's predicate hardening — is CLOSED fail-closed: the
  ONE spawn normalization (`background_spawn_call_form`) now detects a receiver whose shape-ness
  comes from union-narrowing (the `union_narrowed` overlay — the exact source
  `binding_ty_narrowed` reads; no second detection derivation) and emits a WHAT/WHAT-INSTEAD/WHY
  teaching compile error instead of routing into codegen's `Type::Shape` heap-upgrade — BOTH
  spawn forms (statement + handle) through the one shared helper, ZERO codegen change.
  Verify-before-fix (probed live pre-fix): the narrowed spawn compiled clean and ran, printing
  `0` for `radius: 5.0` (the heap copy loaded the 64-byte shape from the 16-byte `{tag,data}`
  union storage); post-fix it is a deterministic teaching error, exactly ONE diagnostic per
  spawn site (no double emission through the shared helper). RED→GREEN test
  `v03_m6_ufcs_background_narrowed_union_receiver_rejected_both_forms` (+ fixture
  `v0_3_m6_ufcs_background_narrowed_union.ynz`, both forms, distinct variants asserted); gallery
  trigger added to `examples/primantis-orders/m6_errors.ynz` (now 10 diagnostics; count+phrase
  assertions updated in `error_galleries.rs`). The diagnostic is a per-site dynamic message —
  the registry `[[diagnostic_template]]` carve-out, matching the Check 2b / M6 teaching-error
  convention (no registry entry; considered, not forgotten). Plain-shape paths un-regressed: the
  flagship statement/multifield/handle UFCS tests + all Call-form background fixtures green
  un-weakened. Gates (docker, nextest): `cargo nextest run --workspace --no-fail-fast`
  **2363 run / 2363 passed / 0 failed / 6 skipped** (baseline 2362 + 1 new test; the round-1
  `sync_bridge_overhead_measurement` flake passed this run); clippy `--workspace -- -D warnings`
  clean; `fmt --all` + `--check` clean. **Deviation surfaced — the FRAGO-prescribed
  WHAT-INSTEAD falsified live:** the suggested workaround (`let ship: Circle = fig` inside the
  `is` arm, then spawn `ship`) does NOT work on the live tree — probed: the narrowed re-bind
  copies the union storage too and prints `0` with no spawn involved — so the shipped message
  steers to a shape-typed binding at the value's creation site (the only working pattern,
  probe-verified). **Two sibling union-payload surfaces probed and SURFACED for the seam
  (pre-existing, same extraction family as #21, NOT fixed this round):** (a) direct field access
  on a narrowed binding (`fig.radius` inside `is Circle`) silently reads the union storage —
  prints `0` for `5.0`, silent-wrong with no spawn involved; (b) Call-form
  `background work(fig)` with a give-transferred UNION arg compiles and runs but the task's
  tag-match produces NO output (expected `circle`). Candidates for the seam, not silent fixes.
- **Polish round completion note (2026-07-10, `executor-2026-07-10-m6-phase3c-polish`):**
  COMPLETE. Teaching-text polish + honest deferral homing only — ZERO detection/rejection logic
  change, no test weakened. (1) **WHAT-INSTEAD reworded** (security should-fix): the narrowed-union
  rejection's WHAT-INSTEAD (`check.rs`, `background_spawn_call_form`) no longer reads as "re-bind
  the narrowed value here" — it now states the probe-verified working pattern (spawn on the
  original `<Variant>`-typed binding where the value is created, BEFORE the union store) AND
  explicitly warns against `let inner: <Variant> = <narrowed>` (the re-bind copies the union
  envelope into a shape-sized binding — the security-reproduced SIGSEGV, now FR #24(b)). WHAT and
  WHY unchanged; the asserted phrase ("a union value narrowed to `<Variant>` cannot yet be used as
  a `background` receiver") unchanged, so the rejection test + gallery assertions hold un-edited.
  (2) **Deferral cluster homed honestly** (deviation-judge should-fix): the two NON-concurrency
  union-narrowing-payload siblings — (a) narrowed direct field access silent-wrong, (b)
  union→shape re-bind OOB/SIGSEGV — split out to NEW FR #24 with the explicit "pre-existing
  general union-narrowing bugs, NOT concurrency defects, orthogonal to M6's charter"
  callout (the #18/#19/#20 mold); (c) the give-transferred union-arg surface (concurrency-adjacent)
  stays on FR #21, which now cross-references #24 (one extraction machinery closes both). A cheap
  interim fail-closed rejection of the (b) re-bind is recorded inside #24 as a CANDIDATE for its
  future owner, not self-decided. (3) **Example cleanliness:** the narrowed-union fixture
  (`v0_3_m6_ufcs_background_narrowed_union.ynz`) + the m6 gallery shapes swapped `float`→`int`
  fields so a copied example doesn't hit the SEPARATE pre-existing global `float.toString` bug
  (`let f: float = 5.0; print(f.toString())` prints `0.0`) — that bug is surfaced as a one-line
  candidate for the seam (far out of M6 scope, NOT fixed). Gates (docker, nextest, forced
  runtime→driver rebuild): `cargo nextest run --workspace --no-fail-fast` **2363 run /
  2363 passed / 0 failed / 6 skipped** (baseline held, 0 real failures; rejection test +
  m6 gallery pass with the reworded message — the asserted WHAT phrase is unchanged;
  plain-shape statement/multifield/handle + Call-form background fixtures green un-weakened);
  clippy `--workspace -- -D warnings` clean; `fmt --all` + `--check` clean. code-reviewer; critical-path-integrity (UAF/lifetime — does the receiver
  now genuinely survive the spawner's frame death on every spawn path?); security reviewer
  (memory-safety); rules-compliance (authoritative-derivation — one inference path threaded, no
  twin); test-quality (is the RED→GREEN flip genuine — the fix, not a weakened/vacuous test?).
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
- **Completion note (2026-07-11, `executor-2026-07-11-m6-phase4`; fix-loop round 1
  `executor-2026-07-11-m6-phase4-fixloop1`):** DONE — race closed via **register-before-poll**
  (plan option 1; the smaller diff, zero lock nesting — the hold-one-lock alternative would
  nest `recv_waiters`→`receiver` — and the ordering Phase 4b is already sequenced to mirror).
  `chan.record_recv_waiter(cx.waker())` now runs BEFORE `poll_recv` in `ynz_channel_recv_poll`
  (`channel.rs:461-462`; the plan's `311-339`/`331` anchors had drifted to `423-462`/`442` from
  P3-1's earlier additions — substance matched exactly). Correctness: a send's waiter drain is
  serialized against the record by the `recv_waiters` mutex — record-first ⇒ drained/woken;
  drain-first ⇒ the send's enqueue happens-before the subsequent `poll_recv`, which observes
  the value. The `Ready(Some)` exit drains the registration via `wake_recv_waiters` (self-wake
  = harmless spurious re-poll); the `Ready(None)` exit wakes nobody, deliberately — see the
  fix-loop record below. **Fix-loop round 1 (reviewer fleet, 2 converged should-fix):**
  (1) the first pass's sibling `Ready(None)` co-waiter-wake fix + its
  `recv_closed_observation_wakes_co_waiters` test were REVERTED per deviation-judge — the gap
  is LATENT, not live (every close-simulation in `channel.rs` is `#[cfg(test)]`-only; no
  production path closes a channel while a receiver survives, per Terrain P2-1), and it lands
  inside Future Requirements #4's explicitly-deferred channel-close-semantics territory (M8);
  the first pass's "hung forever on a closed channel" framing overstated present-day severity.
  Noticed-gap recorded as a note under FR #4, not a new deferral. Phase 4's shipped scope is
  therefore exactly the plan's: the register-before-poll reorder for the live send-race window,
  nothing about channel closure. (2) The literal 3-party repro from step 3 added (test-quality
  + acceptance-verifier converged): `live_send_after_slot_clobber_wakes_clobbered_receiver` —
  A suspends, C's poll clobbers the mpsc single-slot waker, a LIVE send fires; asserts A is
  woken AND observes the sent value on re-poll (deterministic single-threaded manual-`Waker`
  construction; no true-race variant, per plan step 3 it would be best-effort not the gate).
  **Committed test coverage (final):** `channel::tests::recv_poll_registers_waiter_before_polling`
  (RED→GREEN, watched fail pre-fix: drives the REAL extern fn with a manual `RawWaker` clone-probe
  that deterministically observes the register-vs-poll ordering — the window lives inside one call,
  so ordering-by-construction is the deterministic gate; the receiver mutex makes an in-call
  cross-consumer interleave structurally impossible to drive black-box single-threaded) and
  `channel::tests::live_send_after_slot_clobber_wakes_clobbered_receiver` (direct fixture of the
  3-party scenario, authored post-fix against the already-fixed code: the step-3 scenario
  end-to-end; the wake it asserts is mechanism-equivalent to the fix's drain-all, directly
  fixtured rather than argued by inspection). **P3-4 clean bill RE-VERIFIED from the final post-revert code, not carried
  forward:** the critical sections are strictly sequential — `recv_waiters` (released) →
  `receiver` (released, held only across the one non-blocking `poll_recv` as a statement
  temporary) → `recv_waiters` (Ready(Some) only, released); no two production locks ever held
  simultaneously, no new lock-ordering edge (R3 mitigated as designed); `wake()` under
  `recv_waiters` is the pre-existing P3-4-cleared pattern, unchanged. **Gates (docker, nextest,
  forced runtime→driver rebuild — `libynz_runtime.a` 05:24:27 < driver 05:24:28, embed fresh):**
  `cargo nextest run --workspace` **2365 run / 2365 passed / 0 failed / 6 skipped** (= baseline
  2363 + the ordering probe + the 3-party repro; the reverted sibling test was added and removed
  within Phase 4, net zero vs. baseline — the fix-loop dispatch's expected "2364" was an
  arithmetic slip, reconciled here). First full run hit the DOCUMENTED pre-existing
  `ynz-typeck::symbol_lookup::test_cross_file_reference_count_estimate_completes_fast` wall-clock
  flake (audit backlog: "will flake gates again"; same disposition as Phase 3's occurrence) —
  passed in isolation (0.005s) and in the full rerun; surfaced, NOT silenced. Clippy `--workspace
  -- -D warnings` clean; `fmt --all --check` clean. Files: `crates/ynz-runtime/src/channel.rs`
  (only code file) + `plan.md`/`audit.md`. Nothing committed (conductor seals).

#### Phase 4b — P2-7: `handle_recv_poll` panic-then-pending hang (FRAGO 010; un-deferred)

- **Task + purpose:** close the confirmed concurrency hang where `ynz_handle_recv_poll`'s panic path
  (`handle.rs:297-303`) can return `Pending` with a possibly-unregistered waker — if the panic fires
  before waker registration, the task may never wake. Un-deferred from Future Requirements #7 / R11
  per user-directed Mission-scope (FRAGO 010); fix mirrors Phase 4's register-before-poll discipline.
- **Steps**
  1. CCIR-1: re-confirm the current ordering in `ynz_handle_recv_poll` (`handle.rs:297-303`) — the
     panic path vs. waker registration — against the live tree.
  2. Author the RED repro BEFORE the fix (verify-before-you-fix): a panic-before-registration repro
     proving the task never wakes.
  3. Fix: apply the same register-before-poll discipline Phase 4 used for `ynz_channel_recv_poll`,
     extended to the handle poll path (`handle_recv_poll`) — a single, mirrored pattern, not a
     bespoke new ordering.
  4. Run the RED repro (now GREEN) + the full suite; explicitly confirm no regression to Phase 4's
     own fix or to `handle.rs`'s other poll paths.
- **Exit criteria:** hang closed; RED→GREEN repro committed; full suite green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the repro genuinely exercise
  the panic-before-registration window?).
- **Model tag:** `(coding, standard, small)`
- **Completion note (2026-07-11, `executor-2026-07-11-m6-phase4b`):** DONE — hang closed via the
  exact mirror of Phase 4's **register-before-poll** discipline, lifted to the handle seam.
  **CCIR-1 (step 1):** the plan's `handle.rs:297-303` anchor had drifted — the fn spans `290-319`
  pre-fix, panic arm `312-318`; substance confirmed exactly: `poll_recv` ran with NO registration
  of any kind (unlike `channel.rs`, whose `recv_waiters` registry predated P3-2, the handle path
  had no side registry at all), so a panic before mpsc's single-slot registration returned Pending
  permanently unwakeable. **Fix (step 3) — three mirrored parts, all required for the pattern to
  HOLD** (register-first into a registry nothing drains would be present-but-not-holding):
  (1) `recv_waiters: Mutex<Vec<Waker>>` on `HandleShared` (already Arc-shared between
  `YnzTaskHandle` and the child `HandleStateFnFuture`) with `record_recv_waiter`/
  `wake_recv_waiters` mirroring `YnzChannel`'s verbatim; (2) `handle.shared.record_recv_waiter(
  cx.waker())` BEFORE `poll_recv` in `ynz_handle_recv_poll` + the `Ready(Some)` drain;
  (3) producer-side drain: `HandleStateFnFuture::poll`'s Ready arm wakes `recv_waiters` after the
  completion `try_send` (the mirror of the channel send path's wake-after-enqueue). Same
  serialization argument as Phase 4 (record vs. drain serialized by the `recv_waiters` mutex;
  drain-first ⇒ enqueue happens-before the poll ⇒ value observed); panic case: record-first ⇒ any
  panic below returns Pending with the waker ALREADY recorded ⇒ the completion drain wakes it. The
  `Ready(None)`/Closed exit leaves the register-first entry recorded and wakes nobody — mirrors
  the channel's closed-exit convention (FR #4 / Phase 4 fix-loop revert precedent); for the handle
  Closed IS a live path (second `.receive()`) but returns a terminal answer — no hang; entries
  dedup per task (`will_wake`), freed at handle free. `#[expect(dead_code)]` on
  `YnzTaskHandle.shared` removed (the field is now genuinely read). **RED→GREEN (step 2 before
  step 3, staged):** structural substrate (registry + helpers + producer drain — behavior-identical
  while nothing records; the drain fired on an always-empty vec) + both tests landed FIRST against
  the UNCHANGED poll ordering and were watched FAIL 2/2 on the intended asserts (ordering probe:
  `registered_before_poll` false; behavioral repro: wakes == 0 — the literal P2-7 hang), then the
  two-statement recv-poll fix flipped both GREEN. **Tests:**
  `handle::tests::handle_recv_poll_registers_waiter_before_polling` (ordering probe — clone sites
  disambiguated by `recv_waiters` `try_lock` state, vacuity guard, semantic follow-through; the
  channel probe's mirror) and
  `handle::tests::completion_wakes_receiver_after_panic_before_slot_registration` (the
  panic-before-registration repro: the armed probe panics AT the mpsc slot-registration clone — a
  real panic through the fn's `catch_unwind`, stderr confirmed the fn's own
  "`ynz_handle_recv_poll` panicked (returning Pending)" eprintln fired — child then completes;
  asserts the wake arrives and the value is collected through the panic-poisoned,
  `lock_or_recover`-recovered outbox mutex). **Lock discipline (P3-4 pattern, verified from final
  code):** strictly sequential `recv_waiters` (released inside record) → `outbox_rx` (statement
  temporary across the one non-blocking `poll_recv`) → `recv_waiters` (`Ready(Some)` only);
  producer side `try_send` (no lock) → `recv_waiters`; no nesting, no new lock-ordering edge.
  **Gates (docker, nextest, FORCED runtime→driver rebuild — `touch crates/ynz-driver/build.rs`;
  `libynz_runtime.a` 07:09:30 < driver 07:09:59, embed fresh):** `cargo nextest run --workspace`
  **2367 run / 2367 passed / 0 failed / 6 skipped** (= baseline 2365 + the 2 new repros); explicit
  no-regression receipts: `channel::` 10/10 (Phase 4's fix intact) and `handle::` 6/6
  (`ynz_handle_send_poll` ABA + drop paths untouched and green); clippy `--workspace -- -D
  warnings` clean; `fmt --all --check` clean; the documented typeck flake did not fire. **Recorded
  decisions:** (a) zero diff outside `handle.rs` — `channel.rs` (sealed Phase 4 code) untouched;
  `panic_payload_msg` stays private there, so the handle's panic eprintln keeps its existing
  payload-less `Err(_)` shape (exact scope over cosmetic mirror); (b) the registry helpers are
  deliberately duplicated onto `HandleShared` rather than extracting a shared WakerSet out of
  sealed `channel.rs` — surfaced for the reviewer seam as a possible future DRY extraction, not
  self-applied. **Deviations:** line-anchor drift only (pre-warned by the plan; surfaced, no
  FRAGO). Residual noted for reviewers (same shape Phase 4 shipped, not new): a panic INSIDE
  `record_recv_waiter`'s own `waker.clone()` would still miss registration — identical residual to
  the sealed channel fix, theoretical for real Arc-based wakers; mirrored, not gold-plated. Files:
  `crates/ynz-runtime/src/handle.rs` (sole code file) + `plan.md`/`audit.md`. Nothing committed
  (conductor seals).

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
     - **FRAGO 028 addition:** invoke `drop_glue` at BOTH cancellation-path removal sites —
       `purge_pending_sends` (the live purge called from the real drop ladder and `ynz_handle_free`)
       and the insert-time stale-same-token/different-generation sweep inside
       `channel_send_poll_guarded` — not just at the channel's own `Drop` impl: an entry removed on
       either path is gone before `Drop` could ever see it, so its heap payload leaks otherwise.
       Reuses the EXISTING glue registered at construction (no new ABI surface, no new
       construction-site call).
  3. Explicitly confirm P2-3 (closed-send leak) is UNCHANGED by this phase — the closed1/closed2
     codegen blocks stay untouched and stay deferred (Future Requirements #1); this phase's drop-glue
     mechanism must not accidentally start reaching them.
  4. Author the alloc=free parity gate at the **runtime C-ABI level** (FRAGO 027 — NOT an E2E
     fixture): a `channel.rs` `#[test]` directly driving `ynz_channel_create` (with real per-type
     glue) → send heap-element values (`array`/`map`/`shape` — NOT `string`, which has no sound free
     and is invisible to the alloc counter) with elements still buffered → `ynz_channel_free` →
     assert alloc=free parity, and confirm the gate shows NON-ZERO allocations exercised (never a
     vacuous zero-alloc pass, per M5's FRAGO-005 lesson). This proves the drop-glue MECHANISM works
     correctly; it does NOT prove any compiled Yinz program currently reaches it — the channel's
     last-ref drop is E2E-unreachable today (no codegen path releases the creator's reference, per
     FRAGO 027), so an E2E fixture gate would be vacuous by construction.
  5. Run the full suite; confirm no regression to channel send/receive semantics.
- **Exit criteria:** drop-glue mechanism live; alloc=free parity gate GREEN with non-vacuous coverage
  AT THE RUNTIME-ABI LEVEL (E2E unreachable today — folded into Future Requirements #13/#17, see the
  deferral note there; FRAGO 027); the cancellation-path leak (purge_pending_sends + insert-time
  stale sweep) is closed with its own non-vacuous parity gate — not folded into the FR#13/#17
  E2E-unreachable-drop deferral (FRAGO 028); P2-3 confirmed untouched and still correctly deferred.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (parity-gate non-vacuousness, per the
  M5 FRAGO-005 precedent); design-doc-alignment reviewer (authoritative-derivation.md — one drop-glue
  choke point, no parallel path).
- **Model tag:** `(coding, high, medium)`
- **Completion note (2026-07-11, `executor-2026-07-11-m6-phase5-seg3`):** COMPLETE (Steps 2–5;
  Step 1 + FRAGO 027 landed by seg1/seg2). Drop-glue mechanism live: `YnzChannel.drop_glue`
  (`Option<unsafe extern "C" fn(i64)>` — preserves auto Send/Sync), registered ONCE at
  `ynz_channel_create(capacity, drop_glue)` (`channel.rs`); codegen synthesizes memoized
  per-element-type glue at the single `"channel"` intrinsic choke point (`emit.rs`
  `channel_drop_glue`, exactly TWO non-null arms — array→`ynz_array_drop`, map→`ynz_map_drop`;
  string/primitives null per the typeck element gate; no dead shape arm). `pending_sends` values
  became `PendingSendEntry { fut, value_bits }` so teardown can glue residual suspended-send
  payloads; `impl Drop for YnzChannel` drains buffered elements then walks residual entries
  (disjoint sets — no-double-free argument on the impl doc). `purge_pending_sends` initially shipped
  glue-less; the Phase 5 review's deviation-judge surfaced that as a REACHABLE live leak and
  FRAGO 028 closed it — see the FRAGO 028 completion addendum below. Parity gates at the
  runtime C-ABI (FRAGO 027): `channel_drop_glue_frees_buffered_heap_elements_alloc_free_parity`
  (array+map+shape-like glue, ≥18 counted allocs asserted — non-vacuous per M5 FRAGO-005) and
  `channel_drop_glue_frees_residual_pending_send_payload_alloc_free_parity` (≥4), both GREEN.
  13 IR golden snapshots regenerated (decl-line-only: `ynz_channel_create(i64)`→`(i64, ptr)`).
  Suite 2369/0/6 vs 2367/0/6 baseline (+2 = the new gates). D4 verified: closed1/closed2 conduit
  blocks byte-unchanged (`git diff` hunks all ≥ emit.rs:14678). P2-3 stays deferred.
- **FRAGO 028 completion addendum (2026-07-11, `executor-2026-07-11-m6-phase5-frago028`):** the
  cancellation-path glue-less leak (deviation-judge finding, FRAGO 028 → FIX now) is CLOSED. Both
  removal sites now invoke the registered `drop_glue` on each removed `PendingSendEntry`'s
  `value_bits`: `purge_pending_sends` (collect-under-lock, glue-after-unlock) and the insert-time
  stale-same-token/different-generation sweep in `channel_send_poll_guarded` (same pattern). No new
  ABI surface — reuses the construction-registered glue; no double-free (a glued entry is removed,
  so the `Drop` walk never sees it; parked payloads are never buffered). New non-vacuous parity
  gate `cancellation_purge_and_stale_sweep_glue_free_parked_payloads_alloc_free_parity`
  (`channel.rs` tests): capacity-1 array channel with real glue, 4 heap payloads (≥8 counted allocs
  asserted), exercises BOTH sites + last-ref drop, asserts alloc==free; RED→GREEN
  mutation-proven (glue calls neutralized → fails exactly alloc_delta=8/free_delta=4, the two
  parked payloads). Suite **2370/0/6** vs 2369/0/6 (+1 = the new gate; one load-induced timing
  flake of `recursion_cancellation_positive_control_heap_boxes_were_live` under the parallel run,
  channel-free M2 fixture, passes 3/3 in isolation — clean 2370/0/6 on re-run). clippy
  `-D warnings` + `fmt --check` green. `target/release` runtime→driver rebuilt in-session per the
  consumer-mount rule (release binary postdates the fix).

#### Phase 5b — P1-2: twin type-walker unification (FRAGO 011; un-deferred, dormant hardening)

- **Task + purpose:** unify the twin type-walkers (`find_let_typeck_type_in_stmts` `emit.rs:8276`
  vs. `find_let_type_in_stmts` `emit.rs:8364`, diverging only under a non-empty `Cg.type_subst`)
  behind ONE authoritative resolution — confirmed DORMANT at Phase 0 (no live bug today) but the
  exact twin-derivation class that shipped silent miscompiles across M3a/M3d/M3e/M3g. Un-deferred
  from Future Requirements #2 per user-directed Mission-scope (FRAGO 011), sequenced BEFORE Phase 6b
  so the sanitizer lane's scan of the runtime + compiler-generated fixtures runs against the unified
  walker.
- **Steps**
  1. CCIR-1: re-verify the cited lines against the live tree (`emit.rs:8276`, `emit.rs:8364`,
     `Cg.type_subst`).
  2. Design the single shared resolution
     ([authoritative-derivation.md](../../../rules/authoritative-derivation.md)) that both current
     call sites delegate to — never a second, independently-maintained walker.
  3. Fold both walkers behind the one shared resolution; confirm both frame-layout call sites
     (SM-resume + generic-lowering) delegate to it.
  4. Regression-gate the SM-resume + frame-layout suites explicitly (the exact fragile subsystem
     this class of bug has hit four times) — full workspace suite green; house clippy
     `-D warnings` clean; grep-gate confirms zero second derivation.
- **Exit criteria:** one authoritative type-walker resolution; both prior call sites delegate to it
  (grep-verified, zero second derivation); SM-resume + frame-layout suites green; no regression.
- **Reviewer fan-out:** code-reviewer; design-doc-alignment reviewer
  (authoritative-derivation.md compliance — grep-gate evidence attached).
- **Model tag:** `(coding, standard, small)`
- **Completion note (2026-07-11, `executor-2026-07-11-m6-phase5b`):** COMPLETE. CCIR-1: cited lines
  drifted (`emit.rs:8276`→`8585` typeck walker def at time of edit, `emit.rs:8364`→`8679` Cg walker
  def, `crossing_local_type_from_body` def now `8656`) but both function NAMES and the confirmed-
  dormant divergence shape (only under non-empty `Cg.type_subst`) matched the plan text exactly —
  line-number drift only, not a deviation. Confirmed at recon: `find_let_type_in_stmts`'s only call
  site is `crossing_local_type_from_body` (`emit.rs:8656`), itself called only once
  (`emit.rs:4708`, inside `lower_function_with_waits` — the SM-resume path); `find_let_typeck_type_in_stmts`
  has 3 call sites, all also inside `lower_function_with_waits`'s frame-layout/alloca-classification
  code (the cross-check at `emit.rs:4711`, `crossing_slot_indices` at `emit.rs:4432`, and
  `crossing_local_total_slots` at `emit.rs:8535` — the latter is also the "generic-lowering"
  frame-size-precomputation call path, reached from `compute_frame_size`/`build_frame_layouts_with_resolver`
  BEFORE any `Cg` exists). Confirmed `lower_function_with_waits` (hence every `Cg`/`cg_resume`
  constructed there, hence `type_subst`) is reachable ONLY for `f.generics.is_empty()` functions
  (`emit.rs:1298`, `emit.rs:4044`) and both `Cg` literals built inside it hardcode
  `type_subst: HashMap::new()` — so the divergence was confirmed dormant-by-construction, not just
  "no repro found." Unified: `find_let_type_in_stmts` now delegates its ENTIRE traversal to
  `find_let_typeck_type_in_stmts` (`find_let_typeck_type_in_stmts(stmts, target, cg.typed).map(|ty|
  cg.resolve_type(&ty))`) — the ~60-line duplicated `Stmt::Let`/`For`/`If`/`While`/`Match` match arms
  are deleted, not just refactored to call each other. Proved behavior-preserving (not just
  "probably fine"): `Cg::resolve_type` is a pure, structurally-homomorphic function over `Type`
  (recurses into `TypeParam`/`Generic`/`BuiltinArray`/`BuiltinFixed`/`Maybe`, clones everything
  else unchanged — `emit.rs:2007-2030`), so applying it once to the final selected type is
  observationally identical to applying it inline at each recursive match arm as the original code
  did — verified this algebraically for every arm (`Stmt::Let`'s direct value type,
  `Stmt::For`'s `BuiltinArray`/`BuiltinFixed`/`Range`/`BuiltinMap` iterator-derived element type)
  before editing, not asserted after. Grep-gate (zero second derivation): `grep -n 'Stmt::Let { name,
  value, .. } if name == target'` returns exactly ONE hit (`emit.rs:8592`, inside
  `find_let_typeck_type_in_stmts` — the sole remaining traversal). Full workspace suite: 0 failures
  (ran via `cargo test --workspace` — every `test result: ok` line, no FAILED, no panic); house
  `cargo clippy --workspace -- -D warnings` clean; `cargo fmt --all -- --check` clean. SM-resume +
  frame-layout suites explicitly re-run standalone: `cargo test -p ynz-codegen --test
  frame_layouts_query --test golden` — 9/9 and 34/34 passed, including every IR-snapshot/SHA256
  golden test (`ir_text_snapshot`, `m4_player_ir_snapshot`, `v03_m2_*_ir_snapshot` ×7,
  `object_file_sha256_matches_golden`, `m3_fib_sha256_golden`) byte-identical to pre-fix —
  confirms the unification is a pure no-op on current codegen output, not merely "tests still
  pass." No FRAGO needed (in-scope mechanical unification, no plan-vs-reality divergence beyond the
  already-noted line-number drift). Files touched: `crates/ynz-codegen/src/emit.rs` only.

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
  this one hotfix. Sequenced after Phases 1, 1b, 1c, 1d, 3, 3b, 4, 4b, 5, and 5b (a real dependency,
  not mere convenience — see Coordinating Instructions): the sanitizers must scan the FIXED runtime
  code, or their findings would just be re-discoveries of bugs already known and scheduled.
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
  4. **Durable-home deferral lift (FRAGO 012, amended by FRAGO 015 — a REVIEWED deliverable, not a
     promise).** Lift every surviving Future-Requirements deferral — P2-3; bare-channel end-of-stream/
     channel-close semantics; preemption real back-edge yield; `background.cpuBound`; the two orthogonal
     ICEs (roadmap-ledger row 441 + `fixed<T>` param-iteration); the dynamic-dispatch × suspension
     predicate gap (#10); FRAGO-009's conduit-send-number deferral (#12); the Phase 1c per-iteration
     maybe/union heap-cell loop leak (#13, FRAGO 015); the IntLit→`number` call-site coercion facet
     (#14, FRAGO 016 — lifted PAIRED with row 441/#9, both facets of one coercion mechanism routed
     to the `2026-07-04-v0-3-hotfix-int-literal-number` stub plan, pending Patrick's Gate-4 home
     call); the two Phase 1d polish minors (#15 twin-scan consolidation, #16 named decimal128
     cell-size const — FRAGO 016); and the trampoline staged arg-cell shutdown-drop leak (#17,
     FRAGO 017 — joins #13 in the never-drop-locals class, `unscoped → needs the drop-story
     milestone`) — into the roadmap's
     durable store (`2026-05-21-v0-3-concurrency-perf/`, which stays `active` until the whole v0.3
     campaign finishes, so it survives this plan's `git mv` to `done/`): the full four-field
     WHAT/WHY/COST/TRIGGER payload for each goes into the roadmap's own `audit.md`; a pointer row for
     each goes into the roadmap Capability Ledger (both sections, per step 3 above), owner-tagged
     (preemption → M7; channel-close + P2-3 → M8; the two ICEs + dynamic-dispatch →
     `unscoped → needs a milestone`, with roadmap-ledger row 441 explicitly flagged for Patrick's
     Gate-4 home call rather than silently absorbed into either M6 or M7; the heap-cell loop leak (#13)
     → `unscoped → needs the drop-story milestone`, same "flag rather than silently absorb" treatment).
     This is a reviewed
     deliverable — the reviewer fan-out below confirms it actually landed, not merely that it was
     attempted.
  5. Full cumulative gate: `docker compose run --rm dev cargo test --workspace` green; `docker compose
     run --rm dev cargo clippy --workspace -- -D warnings` clean; the byte-exact `pirates-roster`
     golden regenerated + committed; the `m6_errors.ynz` gallery assertions passing; **Phase 6b's
     sanitizer CI job confirmed present and green in `.github/workflows/ci.yml`** (a roadmap claim of a
     "delivered" sanitizer lane with no matching gate check here would itself be the kind of
     doc/reality drift this milestone exists to correct).
  6. Release handoff: per project `CLAUDE.md`'s release workflow, this phase confirms — but does not
     itself execute — the preconditions for `/pr` (if any phase's work isn't yet merged) then
     `/release` to cut the `v0.3.x` patch tag. The release skill is the correct actor for the actual
     cut; this step's job is only to confirm merged-PR state and version-bump readiness are met.
- **Exit criteria:** demo + gallery extended and wired into the test harness; roadmap + both
  Capability Ledger sections updated (including the sanitizer-CI-lane row); **every surviving
  Future-Requirements deferral lifted into the roadmap's durable store (four-field payload in
  roadmap `audit.md` + owner-tagged pointer row in the Capability Ledger, per FRAGO 012), confirmed
  landed by the reviewer fan-out**; full workspace green, including Phase 6b's sanitizer CI job
  confirmed present and green; release preconditions confirmed.
- **Reviewer fan-out:** code-reviewer; docs-consistency reviewer (demo comments, roadmap text);
  adversarial gate-checker (gallery completeness against every new diagnostic this milestone actually
  shipped; AND — FRAGO 012 — that every named surviving deferral genuinely landed in the roadmap's
  `audit.md` with all four fields and in the Capability Ledger with its owner tag, not merely
  claimed).
- **Model tag:** `(coding, standard, medium)`

### 3.4 Coordinating Instructions

- **Sequencing**: Phase 0 gates everything. **Phase 1b runs immediately after Phase 1, Phase 1c
  immediately after Phase 1b, Phase 1d immediately after Phase 1c (FRAGO 009 — the third
  hard-new-machinery decimal128 phase), and ALL THREE run BEFORE
  Phase 2 (FRAGOs 004/006/009 — Patrick-signed/user-directed sequencing: shipped memory-safety
  miscompiles are prioritized ahead of all remaining phases — P1 → P1b → P1c → P1d → P2), and all
  three remain hard prerequisites of Phase 6b
  (the sanitizer lane must scan ALL the FIXED frame-backing).** Phase 1 → Phase 2 is a hard dependency
  (Decision D1) — do not start Phase 2 before Phase 1's carved fixture set (a/c/d) is GREEN, and
  per the signed sequencing not before Phase 1b closes fixture (b) plus the number
  repro, Phase 1c closes the maybe/union + anon-arg repros (the full R14 class GREEN, per
  FRAGO 007's phase assignment), AND Phase 1d closes the sibling decimal128 background-spawn/
  cpu-member defects (FRAGO 009).
  Phases 3, 4, 5 are independent of each
  other and of Phases 1–2 (different subsystems); they are sequenced 3→4→5 for one conductor's
  convenience, not a hard dependency — a FRAGO reordering them is not a plan violation. **Phase 3b (FRAGO 001)
  is sequenced immediately after Phase 3** — same `runtime.rs:591-693` drop-ladder region, minimizing
  merge collision — **and is a hard prerequisite of Phase 6b** (the sanitizer lane must scan the FIXED
  recursion-chain cleanup path, same rationale as Phases 1/3/4/5). **Phase 4b (FRAGO 010) is sequenced
  immediately after Phase 4** — same register-before-poll region/discipline, minimizing merge
  collision — **and is a hard prerequisite of Phase 6b** (the sanitizer lane must scan the FIXED
  handle-poll path). **Phase 5b (FRAGO 011) is sequenced immediately after Phase 5 and BEFORE Phase
  6b** — a real dependency, not mere convenience (the sanitizer lane's scan of compiler-generated
  runtime fixtures should run against the unified type-walker, not the pre-unification twin). Phase 6 is
  independent of everything. **Phase 6b (sanitizer lane) has a real dependency, not mere convenience:
  it must run AFTER Phases 1, 1b, 1c, 1d, 3, 3b, 4, 4b, 5, and 5b land**, because Miri/TSan/ASan need to scan the FIXED runtime
  code (the UFCS threading, the arg frame-backing fixes — the sanitizer lane must scan ALL the
  FIXED frame-backing: shape/`fixed<T>`/number (Phase 1b), maybe/union + anonymous aggregates
  (Phase 1c), and the sibling decimal128 background-spawn/cpu-member fixes (Phase 1d), per
  FRAGOs 004/006/007/009 — the ABA purge, the recursion-chain spike-handle cleanup,
  the lost-wakeup reorder, the `handle_recv_poll` register-before-poll fix, the drop-glue mechanism,
  and the unified type-walker) — scanning
  the pre-fix state would only re-discover bugs already known and scheduled. It is independent of
  Phase 6 itself; sequenced 6→6b for one conductor's convenience. Phase 7 should follow Phase 0's
  dormancy verdicts (so the Future Requirements it cross-references are settled) AND follow Phase 6b
  (so the honesty sweep's own text, and Phase 8's roadmap reconciliation, can truthfully say the
  sanitizer CI lane is live rather than still-pending) — logically it follows all the fix phases (so
  its honesty sweep reflects the ACTUAL post-fix state). Phase 8 is last, extended by FRAGO 012 to
  lift every surviving deferral into the roadmap's durable store.
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
- No shape, `fixed<T>`, or `number` argument to a suspending callee is left in
  a dying stack alloca across suspension — the single authoritative crossing classifier, extended
  (never a second frame-layout path), frame-backs a shape arg via the existing `shape_embed`
  machinery (the child's `self` points into the surviving frame), routes a `fixed<T>` arg into the
  existing Check 2b `UnsupportedCrossingLocalType` guard → deterministic teaching compile error
  (FRAGO 005), and frame-backs a `number` arg via the same classifier plus the
  consumer-side plumbing in the arg-staging path so the staged
  pointer targets the surviving heap frame's decimal128 slot region (FRAGOs 006/007, R14);
  proven by deterministic-across-runs RED→GREEN repros (Phase 1b, FRAGOs 004/006/007).
- No anonymous struct-literal, `maybe`, or `union` argument to a suspending callee is left in a
  dying stack slot across suspension — the same single authoritative crossing classifier,
  extended with anchoring for unnamed temporaries plus new Maybe/Union crossing machinery in the
  one strategy table (never a second frame-layout path, never a second classification path),
  frame-backs all three so they work across suspension — maybe/union routed PAST Check 2b's
  rejection, never INTO it (nothing rejected, per R14); proven by deterministic-across-runs
  RED→GREEN repros (Phase 1c, FRAGOs 006/007, R14).
- No `number` argument crossing a `background`-spawn boundary (CPU or SM arm) or a cpu-member spawn
  boundary is left dangling — closed via the phase executor's own recorded design decision
  (gate-consistent-reject vs. eager i128 heap-copy), proven by deterministic-across-runs RED→GREEN
  repros (Phase 1d, FRAGO 009). The conduit-send half of the same class is verified-safe-by-gate
  (unreachable today via the existing `channel<number>` compile gate) and recorded as a
  Future-Requirements deferral, not silently left unaddressed.
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
- No `ynz_handle_recv_poll` panic can return `Pending` with an unregistered waker — the same
  register-before-poll discipline Phase 4 applied to `ynz_channel_recv_poll` is mirrored onto the
  handle poll path, proven by a RED→GREEN panic-before-registration repro (Phase 4b, FRAGO 010).
- No buffered channel element leaks at channel drop — alloc=free parity, proven non-vacuously at the
  runtime-ABI level (Phase 5; channel drop is E2E-unreachable today per FRAGO 027 — the E2E path
  folds into Future Requirements #13/#17's drop-story class). No cancelled/superseded suspended
  send's heap payload leaks at its removal either — `purge_pending_sends` and the insert-time stale
  sweep invoke the registered glue, proven by their own non-vacuous cancellation-path parity gate
  (FRAGO 028; this path IS live/reachable today, unlike last-ref drop).
- The two frame-layout/SM-resume type-walkers (`emit.rs:8276`/`:8364`) are unified behind ONE
  authoritative resolution rather than left as a dormant twin-derivation risk — proven by a
  grep-verified zero-second-derivation gate plus a clean SM-resume + frame-layout regression run
  (Phase 5b, FRAGO 011).
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
- Phase 1d's fix (whichever design option is chosen) adds no new fixpoint or pass — it is either a
  compile-time reject (reusing the existing `channel<number>` gate's classification cost) or a
  one-time heap-copy at the spawn boundary, O(1) per crossing argument either way.
- Phase 4b's register-before-poll reorder on `handle_recv_poll` (mirroring Phase 4's fix) adds no
  new BigO cost — same mechanical reorder, same cost class as Phase 4's own change.
- Phase 5b's type-walker unification REMOVES a redundant computation (one walker instead of two on
  the SM-resume path) — a strict improvement, not a regression.
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
- If Phase 1d chooses Option 1 (gate-consistent reject), the new diagnostic follows the SAME
  WHAT/WHAT-INSTEAD/WHY teaching shape already shipped for `channel<number>`
  (`check.rs:3369-3398`) and Phase 1b's `fixed<T>` Check 2b error — no bespoke new diagnostic shape.
- The narrowed-union background-receiver rejection (Phase 3c fix-loop round 2, FRAGO 026) follows
  WHAT/WHAT-INSTEAD/WHY per Golden Rule 11, in the same Check 2b fail-closed teaching register; its
  WHAT-INSTEAD suggestion was probe-verified against the live tree (the only working pattern —
  a shape-typed binding at the value's creation site — is what it teaches).

### Runtime Dependencies

- All 8 fix items operate entirely within the already-shipped Tokio-backed runtime (`libynz_rt.a`) —
  no new runtime dependency is introduced by this milestone.
- The channel drop-glue mechanism (Phase 5) depends on the existing heap allocator (malloc) — the same
  dependency channels already have; no new kernel-mode-relevant dependency is added.
- Phase 6b's Miri/TSan/ASan sanitizer lane is a dev-time/CI-time verification tool only — it adds NO
  runtime dependency to the shipped compiler or its `libynz_rt.a`; the `nightly` toolchain + sanitizer
  components live only in the Docker dev image and the CI job, never in a release build. Stated
  explicitly so reviewers know it was considered, not forgotten.
- Phase 1d, 4b, and 5b each operate entirely within the already-shipped Tokio-backed runtime and
  compiler internals — no new runtime dependency is introduced by any of the three.

### Kernel-Mode Behavior

- `--kernel` mode already rejects `wait`/`background`/`channel<T>` entirely (confirmed live this
  session, `check.rs`'s kernel-mode-suspension-rejection arms); none of M6's fixes touch that gate —
  every fix item lives behind the Tokio runtime path, which never runs in kernel mode, so no new
  kernel-mode compile-error surface is needed.
- The preemption honesty fix (Phase 7, docs-only) does not change kernel-mode behavior — kernel mode
  has no scheduler to preempt in the first place.
- Phase 1d/4b/5b's fixes all live behind the same Tokio-runtime / SM-resume compiler paths that
  already never run in kernel mode — none of the three phases opens a new kernel-mode compile-error
  surface.

### Demo & Error Gallery

- `examples/pirates-roster/entrypoint.ynz` gains a `wait x.method()` section demonstrating UFCS
  suspension in a realistic context (Phase 8 step 1); byte-exact golden regenerated + committed.
- `examples/primantis-orders/m6_errors.ynz` is created with WHY-commented triggers for every new
  compile-time diagnostic this milestone adds, wired into `crates/ynz-driver/tests/error_galleries.rs`
  (Phase 8 step 2). The `ynz run` signal-death report is explicitly noted as NOT belonging in this
  gallery (it is a CLI runtime report, not a compile diagnostic) rather than silently omitted.
- Neither Phase 1d, 4b, nor 5b adds a new user-facing demo section by itself: Phase 1d's fix (if
  Option 1) surfaces as a compile-time diagnostic candidate for `m6_errors.ynz` (Phase 8 step 2
  sweep covers it, since Phase 8 runs last); Phase 4b and 5b are internal correctness fixes with no
  new user-facing surface to demo. Stated explicitly so reviewers know each was considered, not
  skipped.

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
- Phase 1d must run the SAME `[[diagnostic_template]]` check as Phase 2 IF Option 1 (gate-consistent
  reject) is chosen: check whether `check.rs:3369-3398`'s existing `channel<number>` template covers
  the new site; reuse or add, recorded via FRAGO if new. If Option 2 (heap-copy) is chosen, no new
  diagnostic template is needed (nothing is rejected).
- No new keywords, banned-jargon words, primitive intrinsics, or type-attached constants — M6 is a
  compiler-internal correctness/leak/honesty hotfix with zero user-facing language-surface changes,
  including Phase 6b's sanitizer lane (compiler-internal CI/dev tooling, not a language feature) and
  Phases 1d/4b/5b (all compiler-internal correctness fixes). Stated explicitly so reviewers know
  every registry-entry-kind was considered, not skipped.

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
  fix, across BOTH token-producer sites** (frame-ptr conduit tokens `emit.rs:12205-12208` AND
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
- **D8 — Phase 1d (FRAGO 009) selects Option 2: eager decimal128 heap-copy at the spawn boundary**
  (the phase executor's own recorded design call, `executor-2026-07-10-m6-phase1d`, made at step 1
  BEFORE implementing, weighed against IMP-concurrency.md / IMP-no-function-coloring.md and this
  plan's own precedents; the `channel<number>` compile gate at `check.rs:3417-3451` — drifted from
  the cited :3369-3398 — stays UNTOUCHED). Reasons, in force order: (1) **design-doc alignment** —
  IMP-concurrency.md "Ownership with Background Tasks" specifies values cross the `background`
  boundary via compiler-inferred `.give`/`.copy` with "small value → `.copy`, cost trivial"; a
  gate-consistent REJECTION of a `number` background arg would contradict the governing design (the
  design doc wins over the cheaper fix). (2) **Option 1 is structurally unavailable for defect C** —
  cpu-member spawn is AUTO-parallelization (an auto-promotion surface, per auto-promotion.md): the
  user wrote valid sequential code and never opted in, so a teaching ERROR there is architecturally
  wrong; the only Option-1-shaped move for C would be an admission decline, which silently
  de-parallelizes every number workload forever — the exact "punishes instead of rewards" inverse
  the auto-promotion rule bans. A policy that rejects A but declines C is two policies, not one.
  (3) **precedent** — Phase 1b's signed R14 disposition already established "a `number` arg ...
  WORKS across suspension — NOT rejected" (§3.1 outcome 1b); rejecting the same value class at the
  sibling spawn boundary would ship an inconsistent teaching story (`wait f(x)` works;
  `background f(x)` errors). (4) **the "hard machinery" framing dissolved on CCIR-1 re-read** —
  `prepare_bg_arg_for_ctx` already carries the per-type heap-upgrade discipline (Shape / Maybe /
  array / MapEntry arms, the MapEntry one an unconditional pre-gate returning
  `BgArgFreeKind::HeapShape`), the heap-cell core family (`heap_cell`,
  `shape_bytes_to_heap_cell`, `maybe_to_heap_cell`) exists, and both spawn arms already own
  balanced free paths (closure-body `emit_bg_arg_frees`; `BgArgDropEntry` kind-0) — a
  `number_to_heap_cell` sibling is a small, precedented extension, not new machinery. (5) **scope
  containment** — Option 2 does NOT unlock `channel<number>` (the plan text's "side effect" framing
  overstated it): the conduit surface needs its own send/recv marshalling design pass; the gate
  stays, and Future-Requirements #12 records the mechanism-reuse trigger.

## Future Requirements / Revisit

1. **P2-3 — closed-send drop-glue leak** (`emit.rs:~11833-11960` closed1/closed2 blocks drop no
   `value_bits`). WHY deferred: structurally unreachable in production until channel-close semantics
   ship (P2-1's finding — a bare channel never closes today). COST to fix later: small once
   channel-close semantics land — reuses Phase 5's drop-glue fn-ptr mechanism directly. TRIGGER:
   channel-close semantics ship (see item 4 below).
2. **P1-2 — twin type-walkers** (`emit.rs:8276` vs `emit.rs:8364`). **Confirmed DORMANT (Phase 0) —
   un-deferred and FIXED in Phase 5b** per user-directed Mission-scope (FRAGO 011): the exact
   twin-derivation class that shipped silent miscompiles across M3a/M3d/M3e/M3g is unified behind
   one authoritative resolution rather than left as cleanup debt. No longer a deferral — this entry
   is retained so the audit-finding numbering (P1-2) resolves to its actual disposition rather than
   a stale "verify dormancy first" note (mirrors item 3's P2-5 treatment).
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
   production-representative concurrency use case ships. *Phase 4 note (2026-07-11, fix-loop
   round 1, `executor-2026-07-11-m6-phase4-fixloop1`):* Phase 4's executor independently noticed
   the same gap's Closed-co-waiter-wake-propagation facet during the P3-2 register-before-poll
   work — a receiver observing `Ready(None)` wakes no recorded co-waiter whose mpsc single-slot
   registration was clobbered. Confirmed presently LATENT, not live: every close-simulation in
   `channel.rs` is `#[cfg(test)]`-only (`mem::replace` sender swaps); no production path closes a
   channel while a receiver survives (matches Terrain P2-1 and this entry's own "structurally
   unreachable" framing). A first-pass piecemeal fix (drain-all wake on the `Ready(None)` arm +
   repro test) was REVERTED per deviation-judge review as landing inside this entry's deferred
   territory; the wake-propagation question is explicitly left for this entry's M8
   channel-close-semantics design pass to resolve properly, not silently fixed piecemeal.
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
   session). **Un-deferred and FIXED in Phase 4b** per user-directed Mission-scope (FRAGO 010): the
   SAME register-before-poll discipline Phase 4 applied to `channel.rs` is mirrored onto the handle
   poll path. No longer a deferral — this entry is retained so the audit-finding numbering (P2-7)
   resolves to its actual disposition rather than a stale "deferred" note (mirrors item 3's P2-5
   treatment).
8. **The `## Capability Ledger` section duplication in the roadmap** (two headings, lines 365 and 417,
   pre-existing migration artifact — noted, not fixed, by this plan; Phase 8 adds M6's row to both so
   neither goes stale relative to the other). WHY deferred: out of this hotfix's charter; a
   documentation-hygiene item, not a bug. COST to fix later: small (merge the two sections into one).
   TRIGGER: the next roadmap-editing session, or Patrick's explicit call to clean it up.
9. **Roadmap ledger row 441 — codegen ICE: bare int literal into a `number`-typed slot crashes the
   compiler (ELEVATED priority)** — the int→number COERCION stays unclaimed between M6 and M7 rather
   than being silently picked up here; **but the raw-ICE EXPOSURE at both store sites is now closed by
   the v0.3-M6 store-site stopgap (FRAGO 020, human-directed "no duct tape", 2026-07-10): both
   `let x: number = 5` and `hidden f: number = 5` now emit the SAME clean teaching error as every other
   int-literal→`number` slot instead of the raw "compiler bug" ICE banner.** WHAT (pre-stopgap root):
   `store`/`store_field`'s `Type::Number` arm assumes a decimal128-pointer representation while
   `Expr::IntLit` lowers to a raw `i64`; typeck admitted the coercion; codegen panicked on common valid
   code (e.g. `let x: number = 5`). **Declaration-site field defaults shared this exact store-site
   root** — `hidden f: number = 5` ICE'd identically (round 4 confirmed:
   `Found IntValue(i64 5) but expected PointerValue variant` at `emit.rs:20351`, from the field-default
   lowering site `emit.rs:18318` calling `lower_expr` with no type hint → raw i64 → `store_field` into a
   decimal128 slot), so both the local binding AND the shape-field default are the SAME store-site #9
   class. The stopgap REJECTS both facets via the shared `reject_int_literal_number_slot` gate
   (`NumberSlotRole::StoreBinding` in `check_let`; `NumberSlotRole::Field` at the `ShapeDecl` decl site);
   the int→number COERCION that would ACCEPT the int literal remains subsumed by the SAME mechanism in
   the `2026-07-04-v0-3-hotfix-int-literal-number` stub plan (see #14) and will REPLACE this rejection
   across all facets — this plan records the linkage and now files FRAGO 020 for the stopgap; it does
   NOT edit the stub plan beyond its own reconciliation note (a conductor→human coordination item).
   WHY the COERCION stays declined: this is
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
11. **Pre-existing backend ICE on `fixed<T>` PARAM iteration** ("cannot iterate fixed array with
    unknown size" — surfaced by Phase 1b segment 1 while constructing the `fixed<T>` escape
    fixture; the fixture's callee deliberately does not iterate its param to stay clear of it).
    **WHAT:** codegen ICEs when a function body iterates a `fixed<T>` received as a PARAMETER
    (size not statically known at the callee). **WHY deferred:** a different bug class entirely —
    a backend lowering ICE, not the UAF/crossing class Phase 1b closes; orthogonal to M6's
    concurrency charter (same non-absorption shape as D6/P2-7); it is a LOUD compile-time crash,
    never a silent miscompile. **COST to fix later:** ~0.5-1 session (thread the fixed-array size
    through the param ABI, or reject fixed-param iteration with a teaching error — needs its own
    small design pass). **TRIGGER:** the next milestone touching `fixed<T>` codegen/ABI, or a
    real user hitting the ICE on valid-looking code. (FRAGO 005 — recorded as a deferral, not a
    Phase 1b scope-add.)
12. **Conduit-send decimal128** (`emit.rs:11809`, surfaced by Phase 1b's fix-loop + boundary review
    alongside the sibling background-spawn/cpu-member defects Phase 1d fixes). **WHAT:** a
    `ptr_to_int` of a stack temp sent as a raw i64 into `mpsc<i64>`; a receiver on another frame
    would reconstruct a pointer into the sender's dead resume-fn stack — the same
    decimal128-across-a-concurrency-boundary UAF shape as Phase 1d's A/C. **WHY deferred:**
    VERIFIED-SAFE-BY-GATE — `channel<number>` is compile-gated by typeck (`check.rs:3417-3451` —
    drifted from the originally-cited :3369-3398 per D8/CCIR-1; confirmed live this segment)
    with a teaching error naming this exact UAF class, so this path is unreachable from any current
    syntax (unlike A/C, which Phase 1d confirms are LIVE). Fixing unreachable code now is
    speculative work against dead code, the same YAGNI-ceiling shape D4/P2-3 and FRAGO 002's #10
    already name. **COST to fix later:** small — Phase 1d selected **Option 2 (D8: eager i128
    heap-copy)**, so this deferral directly reuses the shipped `number_to_heap_cell` codegen helper
    (`emit.rs`) for the value copy; it still needs its OWN send/recv conduit-marshalling design pass
    to remove the `check.rs:3417-3451` gate — Option 2 does NOT unlock `channel<number>` on its own
    (D8 reason 5: the conduit surface is a separate marshalling problem from the spawn-arg boundary).
    **TRIGGER:** `channel<number>`'s heap-copy machinery ships (removing the existing compile
    gate), or a real workload needs `channel<number>` to work rather than be rejected. (FRAGO 009 —
    item B, recorded as a deferral, not a Phase 1d scope-add.)
13. **Per-iteration heap-cell leak for crossing maybe/union LOOP bindings** (Phase 1c step 3d, confirmed
    by exact-gap Paper-Trace proof — `v0_3_m6_heap_cell_loop_parity.ynz`, alloc=12/free=1, gap exactly
    11 = 5×1 maybe envelope + 3×2 union envelope+payload, predicted before first run, stable 4/4).
    **WHAT:** a crossing maybe/union binding re-bound each loop iteration orphans its promoted heap
    cell(s) (1-2 cells/iter) — held to process exit, never freed. **WHY deferred:** freeing needs the
    ownership drop story, out of this hotfix's charter — the same never-drop-locals class as M5's
    Future-Requirements #6, into which maybe/union heap cells now join alongside the existing
    string/array/map crossing locals. **COST to fix later:** the drop-story milestone (1-2 sessions) +
    updating the two parity pins. **TRIGGER:** the drop story lands, or a real unbounded-suspension-loop-
    over-maybe/union workload. Pinned loud in-suite via
    `v03_m6_p1c_heap_cell_loop_parity_pins_documented_per_iteration_leak` so any new leak class or a
    landed drop story shifts it loudly. (FRAGO 015 — deferral formalized from the Phase 1c completion
    note, per the deviation-judge's JUSTIFIED/risk-neutral verdict.)
    **Phase 5 / FRAGO 027 addition to this same never-drop-locals / drop-story class (also covers
    #17 — one class, no new numbered deferral):** Phase 5 confirmed that `YnzChannel`'s creator-side
    reference is never released by any codegen path today (`ynz_channel_create` emitted at exactly one
    site, `ynz_channel_share` is refcount-increment-only inside background-arg staging,
    `ynz_channel_free` emitted only in the background-task drop ladder, `ynz_handle_free` declared but
    never emitted) — so the channel's LAST-REF-DROP teardown built in Phase 5 is correct but currently
    unreachable from any compiled Yinz program; its non-vacuous parity proof lives at the runtime-ABI
    level (Phase 5 step 4). **Scope correction (FRAGO 028): the cancellation-path leak —
    `purge_pending_sends` (called from the live drop ladder on every task cancellation and from
    `ynz_handle_free`) and the insert-time stale-entry sweep removing a `PendingSendEntry` without
    invoking `drop_glue` — is NOT part of this deferral. That path is REACHABLE and live today, and it
    is FIXED (both sites invoke the registered glue, with their own non-vacuous cancellation-path
    parity gate); this entry covers ONLY the genuinely E2E-unreachable last-ref-drop mechanism.**
    This folds into the WHY/COST/TRIGGER already recorded here and in #17: the
    drop-story milestone that frees crossing locals is the same machinery that will release the
    creator's channel reference at scope exit (`.give`/`.share`/return-escape aware), at which point
    Phase 5's runtime-ABI parity pin gains its E2E sibling.
14. **Int-literal → `number`-param CALL-SITE coercion gap** (surfaced by Phase 1d segment 2 while
    resolving the cpu-member IntLit sub-case; the sibling call-site facet of item #9's store-site
    ICE — SAME root class, different site). **WHAT:** a bare int literal passed as a CALL ARGUMENT to
    a `number`-typed parameter (`f(5)` where `f(n: number)`) type-checks but ICEs at codegen — even
    in a plain SYNCHRONOUS call — because `lower_expr(Expr::IntLit)` emits a raw `i64`
    (`emit.rs:14514`) and the normal call-argument loop performs no int→number coercion
    (`emit.rs:14986-14990`), so the compiled callee is handed `call ptr @f(i64 5)` against its
    pointer-typed decimal128 param (LLVM verifier reject). Root cause is the SAME missing int→number
    coercion as item #9 (which is the STORE/binding-site facet, `let x: number = 5`); #9 and #14 are
    two sites of one gap and should very likely be fixed together (one int→number coercion, threaded
    at both the store site and the call-argument site — authoritative-derivation, one mechanism). Note
    (v0.3-M6 store-site stopgap, FRAGO 020, 2026-07-10): #9's store sites — like this call-site facet
    since round 1 — now REJECT with the shared teaching gate rather than ICE; rejection is uniform
    across ALL sites, and the one int→number COERCION this entry tracks remains the deferred piece that
    will REPLACE every such rejection.
    **WHY deferred (scoped OUT of Phase 1d):** this is a PRE-EXISTING GENERAL codegen coercion gap,
    orthogonal to FRAGO 009's decimal128-across-a-concurrency-boundary charter — it ICEs
    synchronously with no concurrency involved, and fixing it ONLY at the spawn boundary would ship
    an inconsistent teaching story (`background f(5)` would work while `f(5)` stays broken). It is a
    LOUD compile-time crash, never a silent miscompile. Phase 1d's cpu-member path guards the
    boundary-local sub-case to a clean teaching COMPILE ERROR (never a segfault — since fix-loop
    round 1, the typeck-level teaching error in `check_user_fn_call`, uniform across all call
    sites; the codegen arm is an unreachable internal backstop), so no
    concurrency-path regression rides on the deferral. **COST to fix later:** unchanged from #9's
    estimate — ~0.5-1 session (expected-type-aware `Expr::IntLit` lowering, or typeck-level
    int→number coercion; its own small design + call-site audit covering BOTH the store site (#9)
    and the call-argument site (this entry)). **TRIGGER:** the same Gate-4 conversation that assigns
    item #9 (roadmap ledger row 441) a home — Patrick assigns the int→number coercion class a home;
    or a real user hitting the ICE on valid-looking code. **RATIFIED — FRAGO 016 (deviation-judge
    verdict: scope-out JUSTIFIED / risk-neutral; formalized from the Phase 1d completion note,
    mirroring FRAGO 015/#13).** The FIX routes to the existing stub plan
    `2026-07-04-v0-3-hotfix-int-literal-number`, expanded to cover BOTH the store-site (#9) and
    call-site (this entry) facets under ONE coercion mechanism (authoritative-derivation) — that
    cross-plan expansion is a conductor→human coordination item, deliberately NOT applied to the
    stub plan by this plan's executors. Interim guard upgraded in fix-loop round 1 (typeck-level
    WHAT/WHAT-INSTEAD/WHY teaching error) and extended in fix-loop round 2 to the three
    user-reachable call-argument forms via the ONE shared `reject_int_literal_number_arg` gate —
    plain `f(5)`, UFCS dot-call `p.f(5)`, and a generic fn's concrete `number` param (round 1's gate
    lived only in `check_user_fn_call`; the UFCS and generic arg loops bypassed it and still ICE'd).
    **Fix-loop round 3 (FRAGO 018) then COMPLETED the guard** across the full class after enumeration
    surfaced real danger beyond the call forms (`array<number>.add(5)` segfault exit 139;
    `contains(5)` silent-wrong `false` exit 0; ~24 slots): `reject_int_literal_number_arg` is now a
    thin wrapper over the role-parameterized `reject_int_literal_number_slot`, which also matches a
    negated `-IntLit` literal and gates every collection-element slot (via the one
    `collection_method_arg_slots` table), struct / array / fixed / map literal, index / field
    assignment, `return`, and match-arm pattern. **Fix-loop round 4 (FRAGO 019) then closed the last
    honesty-gap in that "COMPLETE" claim:** the plain `return` slot did NOT cover an `errors`-wrapped
    return — `return 5` from a `-> number errors` fn slipped to the GENERIC mismatch because the shared
    gate's `Type::Number` match was false through the `ErrorsCapable` wrapper; the gate now unwraps
    `ErrorsCapable` before the `Type::Number` check (ONE gate, no parallel path), so an errors-return
    routes through the same teaching error as a plain `-> number` return. **So the guard is complete
    for the IntLit / `-IntLit` → `number` argument / construction / statement class INCLUDING the
    errors-wrapped return. The DECLARATION-SITE store sites — both `let x: number = 5` (local binding)
    AND `hidden f: number = 5` (shape field default) — are ALSO gated now, as of the v0.3-M6 store-site
    stopgap (FRAGO 020, human-directed "no duct tape", 2026-07-10):** both ICE'd pre-stopgap (the field
    default confirmed round 4: `Found IntValue(i64 5) but expected PointerValue variant` at
    `emit.rs:20436`, from `lower_expr`-with-no-hint at the field-default lowering site `emit.rs:18318`
    storing a raw i64 into a decimal128 slot — structurally identical to the `let x: number = 5` store
    at `emit.rs:20268`), and both now REJECT with the SAME teaching error via the shared
    `reject_int_literal_number_slot` gate (`NumberSlotRole::StoreBinding` in `check_let`;
    `NumberSlotRole::Field` at the `ShapeDecl` decl site — one gate, no per-slot twin per
    authoritative-derivation). Rejection is now uniform across every facet; the int→number COERCION
    (which will ACCEPT the int literal) stays deferred (routed to the stub plan — see #9). Tested by
    `v03_m6_int_literal_to_number_param_{cpu_member,ufcs,generic_fn}_is_clean_teaching_error` plus the
    24 round-3 slot tests, the round-4 `v03_m6_int_lit_number_return_errors_is_teaching_error`, the
    stopgap `v03_m6_int_lit_number_{let_store,hidden_field_default}_is_teaching_error`, and the
    false-positive sweep in `crates/ynz-driver/tests/v03_m6_number_spawn_boundary.rs`; the WHOLE guard
    must be REMOVED when the coercion ships (it rejects exactly the programs the coercion will accept).
15. **`callee_takes_bare_number` / `callee_returns_bare_number` twin-scan consolidation**
    (`emit.rs:18922` / `:18885`-region; code-reviewer polish minor, Phase 1d fix-loop round 1 —
    explicitly "not debt"). **WHAT:** the first-param predicate copies the return-type predicate's
    local-items + imported-fns scan plumbing verbatim; a shared scan helper (param-vs-ret selector
    as the parameter) would consolidate. **WHY deferred:** pure polish — both predicates are
    correct, each is a single authoritative consumer-shared source already (no drift risk named by
    the reviewer); consolidating mid-hotfix buys no behavior. **COST to fix later:** trivial
    (~30 min, one extraction + two call-site updates). **TRIGGER:** the next milestone that touches
    either predicate or adds a third bare-number callee probe. (FRAGO 016.)
16. **Decimal128 heap-cell size `16` as a named shared const** (alloc site `emit.rs:3459`
    `number_to_heap_cell`; free sites `:9717` trampoline `spike_num_free` and `:15798`
    `BgArgFreeKind::HeapShape { byte_size: 16 }`; code-reviewer polish minor, Phase 1d fix-loop
    round 1 — explicitly "not debt"). **WHAT:** the 16-byte cell size is a bare literal at the
    alloc site and both free sites with no compile-time link; a named shared const would give the
    alloc/free ladder a one-source link (authoritative-derivation-aligned). **WHY deferred:** pure
    polish — the three sites ship together in one mechanism authored in one phase; no observed or
    reviewer-named drift path today. **COST to fix later:** trivial (~15 min, one const + three
    substitutions). **TRIGGER:** the next milestone that touches the decimal128 boundary machinery
    (e.g. #12's conduit marshalling pass, which would add a fourth site). (FRAGO 016.)
17. **Trampoline staged decimal128 arg-cell leak on a blocking-pool task dropped UN-RUN at runtime
    shutdown** (`emit.rs` `build_cpu_trampoline` free site ~:9708-9721 `spike_num_free`; surfaced by
    Phase 1d's D8 mechanism, previously tracked only as a code comment — formalized in fix-loop
    round 2 per the graveyard + rules-compliance reviewers). **WHAT:** the cpu-member spawn site
    heap-allocates the 16-byte decimal128 arg cell and the trampoline frees it AFTER result packing
    (one alloc / one free); a blocking-pool task that is queued but dropped UN-RUN at runtime
    shutdown never executes its trampoline, so its one balancing free never runs — the staged cell
    leaks, held to process exit only (never a UAF, never a double-free). **WHY deferred:** same
    never-drop-locals class as M5's Future-Req #6 and this plan's #13 — freeing a staged cell whose
    consumer never ran needs the ownership drop story (a drop-glue registration for un-run task ctx
    words), out of this hotfix's charter; process-exit-only, zero live corruption exposure.
    **COST to fix later:** small once the drop-story milestone lands — register the staged cell
    with the same drop mechanism and update the free sites (the trampoline free + the shutdown
    drop path must be exactly-once between them). **TRIGGER:** the drop story lands, OR a real
    long-lived workload measurably accumulates un-run dropped blocking-pool tasks at shutdown.
    (FRAGO 017.)
18. **Synchronous decimal128 by-value RETURN garbage** (surfaced by Phase 1d fix-loop round 3's slot
    enumeration; formalized round 4). **WHAT:** a `number` returned BY VALUE from a synchronous user
    function prints nondeterministic garbage — `print(toll(5.0))` where `toll() -> number` returns a
    valid decimal literal yields 13-15-digit pointer garbage instead of the value (a stack-dangling
    decimal128 pointer returned by value, then read after the callee frame is gone). This is why the
    Phase 1d false-positive sweep fixture's `toll` deliberately returns `nothing` rather than a
    `number`. **WHY deferred (out of charter):** this is a **function-return ABI defect**, not a
    concurrency-boundary-argument defect — FRAGO 009's charter is decimal128 crossing a *concurrency*
    boundary (background-spawn / cpu-member args), whereas this corrupts on a plain SYNCHRONOUS return
    with no concurrency involved; orthogonal class, same non-absorption shape as #11/#14. It is a
    SILENT miscompile (wrong value, exit 0), so it is loud only under a value-asserting test.
    **COST to fix later:** a decimal128 return-ABI pass — return the 16-byte value by a hidden
    out-pointer (sret) or heap-cell the way Phase 1d's D8 heap-copies decimal128 across the spawn
    boundary; needs its own small design pass (return-slot ABI, not arg-slot). **TRIGGER:** the
    milestone that owns the decimal128 by-value-return ABI (return-ABI work — owning milestone TBD,
    flagged to Patrick at Gate-4), or a real user hitting garbage on a valid `-> number` return.
    (FRAGO 019 — deferral formalized from the round-3 deviation surface, per the deviation-judge's
    should-fix; NOT a round-4 scope-add.)
19. **`map<number, V>` real-number-literal-KEY silent breakage** (surfaced by Phase 1d fix-loop
    round 3; formalized round 4). **WHAT:** a `map<number, V>` keyed by a decimal literal never
    matches — `m.set(1.5, v)` then `m.get(1.5)` returns `none` (exit 0), because decimal128 keys
    hash and compare by POINTER IDENTITY, so two equal decimal literals `1.5` and `1.5` are distinct
    keys and a lookup never finds a prior insert. **WHY deferred (out of charter):** orthogonal to
    M6's concurrency-race/leak/honesty charter — a decimal128 map-key hashing/equality gap, not a
    concurrency defect; same pre-existing-and-orthogonal shape as #18. It is a SILENT-WRONG
    correctness bug (wrong `none`, exit 0), loud only under a value-asserting test. **COST to fix
    later:** implement decimal128 value-based hashing + equality for map keys (hash the 16-byte
    decimal payload, compare by value not pointer) — its own design pass (canonicalization of equal
    decimal representations, NaN/negative-zero handling). **TRIGGER:** a real workload needs
    `map<number, V>` with literal keys, or the decimal128 stdlib hashing work lands. (FRAGO 019 —
    deferral formalized from the round-3 deviation surface, per the deviation-judge's should-fix.)
20. **General UFCS arg-validation gap + `array.remove` has no codegen lowering** (pre-existing, carried
    unchanged from prior Phase 1d rounds; formalized round 4). **WHAT (two orthogonal sub-items):**
    (a) the general UFCS/collection arg-validation surface does NOT validate the `number`→`int`
    direction — `a.concat([5])` and `pick(5, price)` (an int literal where an `int` is fine but the
    surrounding number/int mixing is unchecked) pass typeck without the int→number gate's scrutiny in
    the reverse direction; (b) `array.remove` has NO codegen lowering arm for ANY element type — it is
    unimplemented at the backend, not merely for `number`. **WHY deferred (out of charter):** both are
    pre-existing gaps orthogonal to M6's concurrency charter and to the int-literal→number gate this
    plan completed (which covers the int-literal→`number` direction, not the reverse and not the
    `array.remove` lowering); neither is a concurrency defect. **COST to fix later:** (a) extend the
    arg-validation surface to the reverse direction — small, folds into #14's coercion + a validation
    pass; (b) implement the `array.remove` codegen lowering arm — its own small backend pass.
    **TRIGGER:** a real user hits either gap, or the milestone that owns collection-method codegen /
    the int↔number coercion class picks them up alongside #14. (FRAGO 019 — deferral formalized from
    the round-3 deviation surface, per the deviation-judge's should-fix.)
21. **Narrowed-union background receiver — the DURABLE fix: correct union-payload extraction so
    it WORKS** (FRAGO 026 rescope — split out of the pre-rescope #21, which conflated this
    fix-introduced, in-charter case with the genuinely-pre-existing non-plain-ident class, now
    #23, and mislabeled it "silent-wrong / lifetime-safe"). **WHAT:** make a union binding
    narrowed to a shape variant WORK as a `background` receiver (both spawn forms): extract the
    variant's payload across the spawn boundary, reusing the existing `union_to_heap_cell`
    envelope+tag-resolved deep-copy (`emit.rs:3248`, consumer `:13069`) that already does exactly
    this for the let-bound union arg-escape case. The memory-safety exposure itself is CLOSED:
    it was a **confirmed reachable OOB read (CWE-125)** — 48+ bytes past the 16-byte `{tag,data}`
    storage, IR-reproduced, fix-introduced by round 1's predicate hardening — and the round-2
    interim fix (FRAGO 026) rejects the spawn fail-closed with a deterministic teaching error
    (the Check 2b / FRAGO 005 / R14 precedent). One sibling surface probed 2026-07-10 belongs to
    the SAME extraction family and IS concurrency-adjacent (a `background` spawn), so it rides
    this entry: a Call-form `background work(fig)` with a give-transferred UNION arg compiles
    and runs but the task's tag-match produces NO output (expected `circle`). The two
    NON-concurrency siblings in the same extraction family — narrowed direct field access
    (silent-wrong) and union→shape re-bind (OOB/SIGSEGV) — are homed at **#24** (general
    pre-existing union-narrowing bugs, NOT spawn cleanup); the one extraction machinery should
    close #21 and #24 together. **WHY deferred:** the interim rejection removes the exposure;
    the extraction is spawn-path wiring + tests beyond this blocker-round's charter. **COST to
    fix later:** small-to-medium (<1 phase) — wire `union_to_heap_cell` into the background-arg
    heap-upgrade path for the narrowed-receiver (and union-arg) cases + RED→GREEN fixtures for
    both spawn forms + the give-transferred union-arg sibling (the same machinery closes #24's
    two general surfaces). **TRIGGER:** the milestone that owns
    union-payload extraction (same family as #23's narrowed-receiver half), or a user hits the
    teaching error and needs the working form.
22. **Call-only large-copy Tier-3 warning — UFCS-receiver teaching parity** (FRAGO 025
    deviation 4; minor, teaching-only, echoed here so it survives plan archival). **WHAT:** the
    background large-copy lint (`check.rs`, `BACKGROUND_LARGE_COPY_BYTES` loop) fires only for
    `Expr::Call` args; a UFCS receiver >64 bytes gets no give-vs-copy teaching warning. **WHY
    deferred:** teaching-parity only, zero correctness/memory-safety impact — and the phase
    already built the spawn-target normalization (`background_spawn_call_form`) the extension
    would reuse. **COST to fix later:** small (<1 session). **TRIGGER:** whichever future phase
    next touches background-spawn UFCS diagnostics.
23. **Non-plain-ident shape receivers/args in background-spawn position — BOTH spawn forms**
    (FRAGO 025 deviation 3; **flagged for the MILESTONE-seal human call**, echoed here so it
    survives plan archival; renumbered from the pre-rescope #21 by FRAGO 026, which split the
    narrowed-union case out to #21). **WHAT:** heap-upgrade non-plain-ident shape receivers/args
    in background-spawn position, both forms — `background fleet.flagship.haul()` /
    `ships[0].haul()`, and equally the Call-form `background haul(fleet.flagship)` — today they
    ride membership-less as raw pointers (`is_heap_arg`, `emit.rs:~15909`, gates on
    `Expr::Ident`/explicit `.copy()`, dropping any field-access/index expr to no-heap-upgrade).
    Pre-existing, shared by both spawn forms, NOT introduced or widened by Phase 3c.
    **WHY deferred:** needs new field-projection give/copy machinery beyond Phase 3c's
    give-transferred-plain-ident-receiver charter; building it now expands the phase for an
    unconfirmed-live exposure (`security` could NOT reproduce a live UAF for the simple
    field-access case — the base local's storage survived — and `critical-path` couldn't confirm
    the full blast radius; a latent asymmetry, not a confirmed-live blocker). **COST to fix
    later:** a dedicated fix (new codegen give/copy machinery for field/index/return-materialized
    receivers), ~1 phase. **TRIGGER:** a live UAF is reproduced for a non-plain-ident receiver,
    OR the milestone-seal review (per deviation-judge, route like the R13/R14 signed-risk
    overrides if confirmed live).
24. **General union-narrowing payload NOT extracted — the narrowed value is still the 16-byte
    `{tag,data}` union envelope (pre-existing; reproduces with NO concurrency involved)**
    (Phase 3c polish round, homed per the deviation-judge should-fix; both surfaces probed live
    2026-07-10 during the FRAGO 026 round). **WHAT (two surfaces, one root):** (a) direct field
    access on a narrowed union binding is SILENTLY WRONG — `if (fig is Circle) {
    print(fig.radius) }` prints `0` for `5.0`, exit 0: the field read is lowered against the
    union's `{tag,data}` storage, not the variant's payload (a Golden Rule 5 silent-wrong
    correctness bug); (b) re-binding a narrowed union value to a shape-typed binding is a
    MEMORY-SAFETY bug (CWE-125, security-reproduced as a SIGSEGV) — `let inner: Circle = fig`
    inside the `is Circle` arm copies the 16-byte union envelope into a shape-sized binding, and
    a subsequent pointer-field read (or `background inner.haul()`) reads out of bounds; this is
    the union→shape assignment-lowering face of the same root, and it is why the FRAGO 026
    teaching error's WHAT-INSTEAD explicitly warns AGAINST the re-bind. **Both surfaces
    reproduce with no `background`/spawn anywhere: pre-existing GENERAL union-narrowing
    correctness/memory-safety defects, NOT concurrency defects — orthogonal to M6's concurrency
    charter** (same non-absorption shape as #18/#19/#20); a future owner must NOT mis-triage
    them as spawn-family cleanup. **WHY deferred (out of charter):** union-payload extraction is
    general typeck/codegen lowering work with no concurrency dimension; M6's charter is
    concurrency races/leaks/honesty. The concurrency-reachable face of the same root (the
    narrowed `background` receiver) is already fail-closed rejected (FRAGO 026), and the
    teaching text steers users away from the (b) re-bind — but (a) and the (b) re-bind
    themselves remain reachable in plain non-concurrent code today (silent-wrong / SIGSEGV, no
    guard); a cheap interim fail-closed rejection of the union→shape re-bind (mirroring
    FRAGO 026's precedent) is a candidate for whoever owns this, surfaced at the Phase 3c polish
    round, not self-decided there. **COST to fix later:** the same `union_to_heap_cell`-based
    payload-extraction machinery as #21 — one design pass closes #21 and both surfaces here:
    (a) needs narrowed field-access lowering to resolve the payload (not the envelope);
    (b) needs union→shape assignment lowering to extract the payload (or reject the re-bind)
    rather than envelope-copy. **TRIGGER:** the milestone that owns union-payload extraction
    (land together with #21), OR a user hits the silent-wrong narrowed field read / the re-bind
    SIGSEGV in the wild. **LIFTED to the roadmap durable store 2026-07-11** (roadmap
    `2026-05-21-v0-3-concurrency-perf`: Capability Ledger pointer row in both tables, unscoped →
    needs a milestone, + four-field payload in its `audit.md` under Idempotency-Key
    `2026-07-04-v0-3-m6-concurrency-hotfix#24: union-narrowing-payload-extraction`) so it survives
    this plan's archival — Phase 8's deferral-lift should treat it as already homed, not re-lift it.
