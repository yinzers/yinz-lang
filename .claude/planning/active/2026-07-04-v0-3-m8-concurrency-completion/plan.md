---
name: "v0-3-m8-concurrency-completion"
plan-id: "2026-07-04-v0-3-m8-concurrency-completion"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-producer-2026-07-04-m8-concurrency-completion", "plan-producer-2026-07-04-m8-amend1", "gate4-signatures-2026-07-04", "executor-2026-07-16-patrick-triage-application"]
created_at: "2026-07-04"
updated_at: "2026-07-16"
metadata:
  type: "plan"
---

# PLAN: v0.3-M8 — Concurrency Completion

> **Status note.** This is a complete OPORD — ¶3.1 Intent & End State is non-empty, every phase is
> concrete, the risk table is scored — which would ordinarily flip `status` straight to `active`. It
> is deliberately held at `paused` instead, matching the exact convention the sibling
> [`2026-07-04-v0-3-m7-optimizer-pipeline`](../../paused/2026-07-04-v0-3-m7-optimizer-pipeline/plan.md)
> plan set: `paused` is the **conductor pre-approval state** for a plan that is fully written but
> gated on real external preconditions on EXECUTION start, not on anything wrong with the document
> itself — **(1)** the orchestrator's human read-through/approval checkpoint, which has not yet run
> on this plan, and **(2)** the double merge-and-tag precondition (¶1 Friendly forces; CCIR item 1):
> Phase 0 cannot begin until BOTH the sibling v0.3-M6 hotfix plan AND the sibling v0.3-M7 optimizer
> plan have merged to `main`. The orchestrator flips `status` to `active` once Gate 4 clears and both
> merges land — a plain frontmatter edit on this same file, per the status lifecycle
> ([`REF-plan-format.md`](../../../../../.claude/docs/reference/REF-plan-format.md)).

## 1. Situation

### Terrain (landscape) — grounded in `.claude/audits/2026-07-04-concurrency-release-audit.md`, direct file:line re-reads this session, and the sibling M6/M7 plans

- **P2-1 / P2-1 refinement — the bare-channel non-closure footgun M6 documents but does not fix.**
  `crates/ynz-runtime/src/channel.rs:109-123` — `YnzChannel` holds BOTH the `Sender` and `Receiver`
  endpoints as its own fields for its entire life. `poll_recv` only returns `Ready(None)` when every
  `Sender` clone drops — but the object's OWN retained `Sender` means that condition can never occur
  in production; the existing tests only simulate closure via `std::mem::replace` (`channel.rs:536-539`,
  `557-560`). The closed-recv codegen arm (`emit.rs:~11834-11841`) already carries the comment
  "Structurally unreachable in v0.3-M4 (the channel object holds a sender)" and aborts loudly — so this
  is a known M4 design state (M6 Phase 7 documents it loudly in `IMP-concurrency.md`'s Design
  Divergences section), not a latent surprise. **This milestone is the first to actually design and
  ship the fix** the M6 documentation entry explicitly defers to "channel-close semantics ship."
- **P2-3 — the closed-send drop-glue leak, whose trigger this milestone fires.** `emit.rs`'s closed1/
  closed2 blocks (~`:11833-11960`) build the typed closed-channel error and branch to `post` without
  freeing `value_bits` (no `ynz_array_drop`/`ynz_free` for heap-typed payloads). M6's audit synthesis
  explicitly named this "genuinely unreachable dead code until channel-close semantics ship" and
  deferred it with the trigger **"channel-close semantics ship."** Per
  [`no-duct-tape.md`](../../../rules/no-duct-tape.md)'s deferral discipline, a deferral's trigger firing
  obligates the fix in the SAME milestone that fires it — this plan wires the leak fix through the SAME
  drop-glue choke point M6 Phase 5 registers for buffered-element cleanup (authoritative-derivation.md:
  one choke point, never a second ad hoc drop path).
- **P2-6 / registry self-diagnosis — auto-Arc substrate exists, unwired, and the reason is a genuine
  spec gap, not neglect.** `crates/ynz-runtime/src/arc.rs` (`ynz_arc_new`/`clone`/`free`) is
  concurrency-hammer-tested and confirmed correct by direct read (audit P2-6). Zero codegen call sites
  exist. The `registry/features.toml` `auto-arc-codegen-emission` entry's own `why` field (direct read,
  this session, lines 1229-1235) states the exact reason emission was deferred: (a) a caller/task Arc
  **sharing-topology** decision that `IMP-no-function-coloring.md:58` points to `IMP-ownership.md` for,
  but `IMP-ownership.md` genuinely never specifies it (confirmed: `IMP-ownership.md`'s "Ownership
  Concepts" / "Call-Site Inference" sections cover `share`/`lend`/`give`/`.copy()`/`.freeze()` in depth
  but contain ZERO text on cross-thread Arc sharing — the silence is real, not a citation-depth miss),
  and (b) the entry names the EXACT reusable authoritative proof: `ynz_typeck::effective_ownership`
  (`EffectiveOwnership::Reads`, confirmed present at `crates/ynz-typeck/src/effective_ownership.rs`) —
  the read-only proof that already exists and must be threaded, never re-derived (the same
  authoritative-derivation.md discipline the fragile name-based auto-parallel write-effect analysis
  violated and had to be removed for, per `IMP-concurrency.md`'s "Design Divergences" section). Patrick
  pulls this deferral forward into this milestone; the `auto-arc-cautionary-tint` registry entry (red-
  tint LSP styling) stays separately deferred — no per-hint tint rendering path exists in `ynz-lsp`
  today (confirmed, registry `why` field) — this milestone wires the hint to FIRE in normal muted
  style, never the color.
- **`IMP-no-function-coloring.md`'s "Task Cancellation" section — the locked end-state model for Track 3.**
  (Direct read, lines 281-294.) The runtime half (`ynz_handle_free` aborting the child at its next
  suspension point, safe-drop proven) is SHIPPED. The LANGUAGE half — codegen automatically calling
  `ynz_handle_free` when a handle binding's scope ends — is explicitly SHIPPED-DEFERRED, with the
  registry entry `background-handle-cancel-injection` naming its own trigger as *"the language-wide
  automatic scope-cleanup mechanism shipping... OR a real workload needing to cancel a running task."*
  This milestone's Track 3 is a direct attempt to satisfy that trigger — but the SAME doc's own
  parenthetical warns "a handle-only drop pass would fork a second cleanup mechanism the eventual
  general one must unify" (authoritative-derivation.md, again). Whether extending the SAME choke point
  M6/this-plan already register for channel drop-glue is a small, contained extension, or whether it
  genuinely requires the general language-wide scope-drop mechanism, is a real open question this
  plan's Phase 7 investigates rather than assumes either way.
- **P2-7 — `ynz_handle_recv_poll` panic-then-pending hang, newly surfaced in M6's audit, NOT fixed by
  M6.** `crates/ynz-runtime/src/handle.rs:297-303` — a panic inside the poll returns `Pending` with a
  possibly-unregistered waker; if the panic fires before waker registration the task may never wake (a
  hang, not a crash). M6's own Phase 4 (`ynz_channel_recv_poll` lost-wakeup fix, P3-2) establishes the
  exact register-before-poll pattern this bug needs. Per the brief, this plan absorbs P2-7 as a small,
  contained fix mirroring that pattern — M6 fixed the sibling channel-side race; this plan closes the
  handle-side panic-then-pending variant M6 explicitly left as a Future Requirement.
- **Roadmap Capability Ledger row "Selective hot-field-only element materialization" (both duplicate
  tables, ~line 390 and ~442) — confirmed NOT this milestone's concern.** It is a SoA-specific gather-
  selectivity perf gap (`soa_gather_into`/`array_elem_get_into` never consuming `hot_fields`), entirely
  orthogonal to concurrency correctness/completion. M7's own Roadmap Reconciliation table already
  records "NOT absorbed" for this row against the optimizer-pipeline charter; this plan independently
  confirms the SAME non-absorption against ITS OWN (concurrency-completion) charter, for a DIFFERENT
  reason — not merely inheriting M7's disposition.
- **`background.cpuBound` (P4-2) — confirmed still unclaimed.** Neither M6 nor M7 absorbs this
  (both explicitly park it in Future Requirements as "not this plan's charter"). It is not in the
  brief's four tracks either. Recorded here so a third consecutive plan doesn't silently skip it
  without a trace.
- **Loom feasibility — bounded honestly against what ynz-runtime actually owns.** `pending_sends`
  (`channel.rs:120`, a `HashMap<u64, PendingSend>` guarded by a lock ynz-runtime owns directly), the
  `caller_token` mint/purge logic M6 Phase 3 fixes, the drop ladder (`runtime.rs:591-693`), and the
  register/poll ordering M6 Phase 4 fixes are all synchronization logic ynz-runtime owns and can put
  behind loom-swappable types. Tokio's OWN internal `mpsc` implementation is NOT ynz-runtime's code and
  cannot be loom-model-checked from outside it — the brief's own "scope it honestly" instruction is
  read literally: loom's checked surface is the synchronization logic THIS project owns, never a claim
  to have model-checked Tokio's internals.
- **Fuzzing oracle — M7 builds exactly the self-checking surface this milestone's Track 4b needs.**
  `crates/ynz-driver/tests/cross_impl_consistency.rs` (existing, extended by M7 Phase 5 to also cover
  `--no-optimize` vs. default) already asserts byte-identical stdout/stderr/exit-code across build-mode
  combinations for hand-written fixtures. Track 4b's differential fuzzing reuses this SAME oracle
  logic against GENERATED programs instead of hand-written ones — extending, not re-deriving, the
  consistency-checking mechanism.
- **No `<project>/.claude/risk-anchors.md` override exists** (glob-confirmed this session) — this plan
  scores against [REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md)'s
  default code-domain anchor sheet, same as M6/M7.

### Weather (external constraints)

- **Double execution gate: both M6 AND M7 merged + tagged before Phase 0 begins.** Per the brief,
  M8 is "sequenced after M6 (correctness hotfix) and M7 (optimizer)" — both. This is stricter than
  either sibling plan's own single-predecessor gate (M6 gates on the M5 tag; M7 gates on the M6 merge)
  because M8's Auto-Arc codegen (Phase 5) benefits from M7's Phase 1 LLVM-attribute audit of
  `ynz_arc_*`/`ynz_channel_*` extern declarations already having run under a real optimized pipeline —
  building Arc emission before that audit exists would risk discovering attribute problems on top of a
  brand-new codegen path instead of an already-audited one.
- **No hard date.** Same "hotfix/completion cadence, ship when right" posture as M6/M7.
- **Zero public users, pre-v1.0** — full breaking-ABI latitude per `ADR-versioning`; every change here
  is git-reversible (no Floor-A/Floor-B "no backout" condition anywhere in this milestone).
- **All cargo/build commands run in Docker** (`docker compose run --rm dev ...`, no `-it`) per the
  project's `run-in-docker` convention.
- **Row 442 (selective hot-field gather) is explicitly OUT of this milestone** — orthogonal SoA-gather
  perf gap, confirmed above; recorded again in Future Requirements + the Roadmap Reconciliation table
  so no reviewer mistakes the silence for an oversight.
- **`background.cpuBound` (P4-2) is explicitly OUT of this milestone** — not named in the brief's four
  tracks; recorded in Future Requirements.

### Friendly forces

- **Higher intent**: roadmap
  [`2026-05-21-v0-3-concurrency-perf`](../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md). M8 is the
  vision-completion milestone the roadmap's own "Why Now" section gestures at — closing the gap
  between what the design docs promise (channel close, auto-Arc, cancel-via-drop, systematic
  verification) and what the concurrency-release audit found actually shipped.
- **The concurrency-release audit** (`.claude/audits/2026-07-04-concurrency-release-audit.md`) is this
  plan's evidence base for P2-1/P2-3/P2-6/P2-7, same as M6/M7.
- **Sibling M6** (`2026-07-04-v0-3-m6-concurrency-hotfix`, status `stub`) ships the channel drop-glue
  choke point (its Phase 5) this plan's Phase 4 wires the closed-send leak fix through, the
  `pending_sends` purge + register-before-poll patterns this plan's Phase 6 (P2-7) mirrors, and the
  `IMP-concurrency.md` Design Divergences entry documenting the bare-channel footgun this plan's Phase 4
  retires. **This plan does not re-fix anything M6 owns.**
- **Sibling M7** (`2026-07-04-v0-3-m7-optimizer-pipeline`, status `paused`) ships the real LLVM pass
  pipeline and the exhaustive `extern "C"` attribute audit this plan's Phase 5 (Auto-Arc emission)
  builds on top of, plus the `--no-optimize` build-mode axis this plan's Phase 8 (fuzzing) reuses as a
  differential-oracle axis. **This plan does not re-fix anything M7 owns** (no second optimizer work,
  no second preemption work).
- **M5's authoritative-derivation discipline** (four silent-miscompile incidents across
  M3a/M3d/M3e/M3g) and **M6's own re-confirmation of the same discipline** (the `pending_sends` purge
  threaded to BOTH token producers from one scheme) are the direct precedent this plan's Auto-Arc
  emission (reuse `effective_ownership`, never re-derive) and channel drop-glue (one choke point) must
  not repeat drift on.

### Assumptions

| # | Assumption | Status |
|---|---|---|
| A1 | v0.3-M6 is merged to `main` before any M8 phase executes | **unverified** — future event, enforced as the execution gate (banner above), not assumed true at plan time |
| A2 | v0.3-M7 is merged to `main` before any M8 phase executes | **unverified** — same, second half of the double gate |
| A3 | `IMP-ownership.md` genuinely contains zero text on cross-thread Arc sharing topology (not merely under-cited) | **verified** (direct read this session, "Ownership Concepts" through "No Direct Array Indexing" sections) |
| A4 | `ynz_typeck::effective_ownership`'s `EffectiveOwnership::Reads` is a real, existing, reusable analysis output | **verified** (module exists, confirmed via file listing this session; semantic correctness of reuse is Phase 2's own job to confirm by reading the module, not re-verified here) |
| A5 | The audit's file:line citations for P2-1/P2-3/P2-6/P2-7 are accurate as of 2026-07-04; may drift by execution time (after M6/M7 land and change these same files) | **partially unverified by construction** — M6 Phase 3-5 and M7 Phase 1-2 both touch `channel.rs`/`handle.rs`/`runtime.rs`; Phase 0 below re-verifies every citation against the POST-M6-POST-M7 tree, not the pre-merge tree these citations were read against |
| A6 | `pending_sends`, the drop ladder, and the recv-poll register/poll ordering are synchronization logic ynz-runtime owns directly (not inside Tokio's own compiled internals) and can sit behind loom-swappable types | **verified** (direct read, `channel.rs`/`runtime.rs`, this session and M6's own citations) — the CLAIM that loom can PRACTICALLY exhaust the relevant state space is Phase 3's own spike-gated question, not assumed here |
| A7 | No project `risk-anchors.md` override exists | **verified** (glob, this session) |
| A8 | Docker `dev` service builds + tests the full workspace per project CLAUDE.md's documented commands | **verified** (unchanged house convention) |
| A9 | Row 442 (selective hot-field gather) and `background.cpuBound` (P4-2) remain correctly un-absorbed by both M6 and M7 as of this session | **verified** (direct read of both sibling plans' Future Requirements sections) |

### Risk Assessment

Scored via the global [REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md)
(4×5 fixed lookup; default code-domain anchor sheet — no project override). **Severity is scored
II-Critical for the silent-miscompile-class fixes (R1, R2), consistent with this project's own
established convention** (M6 scored the identical twin-derivation/silent-miscompile shape at Sev II;
the recovery cost is real multi-round engineering debugging even pre-1.0/zero-users). **No Floor B
class fires** (no money/PII/security-breach/irreversible-op in the anchor-sheet sense — every change
here is git-reversible, pre-v1.0, zero public users). **One HIGH residual in this table — R2 — carries
a SIGNED RISK OVERRIDE (see immediately after the table; signed by Patrick at Gate-4 approval,
2026-07-04); every OTHER residual stays at or below MEDIUM.** R1's channel-close hazard genuinely reuses an already-authoritative, already-tested source
(the M6-established drop-glue choke point) and mitigates cleanly to MEDIUM. R2's Auto-Arc hazard is
narrower than that reuse initially suggests: reusing `effective_ownership::EffectiveOwnership::Reads`
closes the MISCLASSIFICATION mode the removed auto-parallel write-effect analysis failed on, but this
milestone's Phase 5 is still net-new codegen inserting Arc-wrap/refcount calls at spawn boundaries —
the SEPARATE frame/spawn-boundary-layout failure mode R8 (M7's own HIGH-residual risk) names is not
reduced by that reuse, because `EffectiveOwnership::Reads` proves WHO may read a value, not whether
the NEW call sites interact safely with the suspension/state-machine frame layout. Phase 5's own spike
step (step 2) explicitly concedes this interaction as an open question the spike itself must prove, not
assume — so R2 is scored on the honest hazard, not stretched to match R1's narrower shape.

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **R1 — channel-close semantics change regresses existing (M4/M6-fixed) channel behavior** (removing/altering the channel object's self-held endpoint changes send/receive/drop ABI) — *Phases 1–4* | C | II | H | Adversarial/RED-repro test class authored BEFORE the fix (explicit `.close()`/end-of-stream call, receive-after-close, drop-without-close, concurrent send-during-close) PLUS a full regression run of every pre-existing M4/M6 channel/handle fixture (**B2**, prob −1; proof: committed RED→GREEN fixture set + zero regression in the pre-existing suite, Phase 4 exit criteria) | **M** (D×II) | recorded |
| **R2 — Auto-Arc codegen emission introduces a refcount imbalance** (silent leak or use-after-free class — the exact hazard family M3a/M3d/M3e/M3g's twin-derivation corpses warned about) — *Phase 5* | B | II | H | Reuse the ALREADY-authoritative `effective_ownership::EffectiveOwnership::Reads` proof (never re-derive a second read-only classifier — closes the exact MISCLASSIFICATION mode the removed auto-parallel write-effect analysis failed on) PLUS a spike-gated minimal emission proven on a throwaway fixture before the full codegen path, PLUS the existing concurrency-hammer Arc test extended to cover the NEW codegen-emitted call sites, PLUS a non-vacuous `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity gate (**B2** adversarial/RED-repro + spike-gate, prob −1; proof: committed spike verdict + hammer-fixture extension + parity gate, Phase 5 exit criteria) — re-lookup(C, II) = **H, unchanged** (Critical severity does not clear High until probability reaches D; `EffectiveOwnership::Reads` reuse closes the MISCLASSIFICATION hazard but does not reduce the SEPARATE frame/spawn-boundary-layout failure mode this net-new codegen path shares with R8's hazard family — see the RISK OVERRIDE block below) | **H** (C×II) | **H — override SIGNED (see block below)** |
| **R3 — loom refactor destabilizes ynz-runtime's existing (M6-fixed) synchronization logic** — *Phase 3* | C | III | M | The refactor is architecturally a type-alias/cfg swap (`#[cfg(not(loom))]` resolves to the exact existing `std`/Tokio-primitive types in production; only `#[cfg(loom)]` test builds see the swapped types) — a spike proves this is non-observable in production builds BEFORE the full harness lands (**B2** canary/staged, prob −1; proof: Phase 3's spike verdict + a production-build diff showing zero generated-code change) | **L** (D×III) | pass |
| **R4 — Track 3 (scope-drop cancellation) design ball​oons into the general drop system mid-phase** — *Phase 7* | C | III | M | The phase's OWN structure makes ballooning a non-failure: Step 1 investigates: if the SAME choke point M6/Phase 4 register for channel drop-glue extends cleanly to handle bindings, implement; if it genuinely requires the general mechanism, STOP and author the formal re-deferral with Patrick's sign-off — both branches are legitimate exits, built into the phase's own exit criteria, not an escape hatch bolted on after the fact (**B1** eliminate — the failure mode is structurally converted into a legitimate outcome; prob −2) | **L** (E×III) | pass |
| **R5 — structured fuzzing (Track 4b) finds a genuine miscompile mid-milestone, threatening scope flood** — *Phase 8* | B | III | M | Every finding routes through the plan-amendment/FRAGO seam (per [plan-source-of-truth.md](../../../rules/plan-source-of-truth.md)) — never a silent inline fix or a silent scope expansion; the CCIR below names this explicitly (**B2** engineered guard — bounded, gate-like routing; prob −1) | **M** (C×III) | recorded |
| **R6 — P2-7 handle-panic-hang fix reintroduces a race** (mirrors M6 Phase 4's exact pattern) — *Phase 6* | D | III | L | Mechanical mirror of the already-fixed sibling pattern (M6 Phase 4's register-before-poll), re-verified against the SAME "no lock held across a blocking poll" invariant M6 re-confirmed | **L** (D×III) | pass |
| **R7 — docs/registry reconciliation sweep introduces a new factual drift** — *Phase 9* | D | IV | L | docs-consistency reviewer diffs every edited claim against this plan's own citations before merge | **L** (D×IV) | pass |
| **R8 — roadmap/Capability Ledger reconciliation mechanical additions** — *Phase 9* | D | IV | L | Mechanical, docs-consistency + code-reviewer fan-out; both duplicate tables updated in lockstep per the established M6/M7 both-tables convention | **L** (D×IV) | pass |

**Floor check.** No Floor-A "no backout exists" condition (every change is git-reversible) and no
Floor-B class (security/PII/money/irreversible-prod-op) fires anywhere in this table.

R2's residual lands HIGH and, per the frozen risk-engine catalog's available patterns, cannot be
honestly mitigated further at plan-authoring time (see the RISK OVERRIDE block immediately below —
drafted with the work shown; **signed by Patrick at Gate-4 approval, 2026-07-04** — this producer
never self-signs a HIGH residual, so the signature is the orchestrator/Patrick's own, not this
producer's). Every OTHER residual in this table stays MEDIUM or LOW; no policy floor fires anywhere in
this table (still no money/PII/security/no-backout dimension). If Phase 0's re-verification or any
other phase surfaces a FURTHER NEW risk that scores HIGH, it is surfaced immediately per the CCIR
below — **never self-signed**; the orchestrator's override gate is the only place a HIGH residual gets
accepted.

**RISK OVERRIDE — accepted residual: HIGH** (R2; work shown per
[REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md)'s gate; this is a
producer-drafted surface for the orchestrator's human override gate — it is never self-signed):

```
RISK OVERRIDE — accepted residual: HIGH
  Risk:                     R2 — the Phase 5 Auto-Arc codegen-emission transform (emitting
                            ynz_arc_new/clone/free at Arc-eligible spawn boundaries, reusing
                            effective_ownership::EffectiveOwnership::Reads as the read-only proof)
                            introduces a refcount imbalance (silent leak or use-after-free) AND/OR a
                            frame-layout/spawn-boundary interaction hazard — net-new codegen in the
                            same silent-miscompile family as R1, and this repo's four-milestone
                            twin-derivation/frame history (M3a/M3d/M3e/M3g), directly echoing M7's own
                            R8 (the back-edge poll-yield transform).
  Why not mitigable to LOW: Initial lookup(B, II) = HIGH. The one honestly-provable catalog
                            mitigation — Adversarial/RED-repro + spike-gate (B2, probability, −1;
                            proof: committed spike verdict + hammer-fixture extension + non-vacuous
                            alloc=free parity gate, Phase 5 exit criteria) — shifts probability B→C.
                            Re-lookup(C, II) = HIGH, UNCHANGED: Critical severity does not clear High
                            until probability reaches D. No second catalog mitigation honestly
                            applies: (a) reusing `EffectiveOwnership::Reads` closes the
                            MISCLASSIFICATION mode only — it is a genuine, valuable design constraint
                            (satisfies authoritative-derivation.md; named in Phase 2/5) but it does not
                            touch the SEPARATE frame/spawn-boundary-layout failure mode, so counting it
                            as a second independent probability-axis shift would double-count one proof
                            against two distinct hazards; (b) the severity-axis B1 patterns
                            (made-reversible / idempotency) don't map to a compiler miscompile, and
                            this plan's own severity-anchor selection (pre-release, fully
                            git-reversible) already prices reversibility into Sev II rather than Sev I
                            — re-applying git-revertibility as a SECOND mitigation step would
                            double-count the same fact; (c) a second probability-axis pattern
                            (canary/staged exposure) does not honestly apply either — its precondition
                            ("small slice first, auto-halt on metric") presumes staged PRODUCTION
                            exposure, which does not exist for compiler-internal, pre-release codegen
                            work; stretching it to fit would be exactly the self-serving cell-picking
                            REF-risk-engine.md's "not a vibes table" clause forbids, and the same
                            discipline M7's own R8 override refused to violate.
  Accepted by:              Patrick (Gate-4 approval, conducted 2026-07-04)
  Date:                     2026-07-04
  Trigger to revisit:       Before Phase 5 Step 2 begins. Re-score if either (a) Phase 5 Step 2's own
                            spike verdict proves the frame/spawn-boundary interaction is clean (no
                            aliasing violation against existing noalias/readonly LLVM attributes, no
                            interaction with the suspension/state-machine frame layout) — a GREEN spike
                            verdict is evidence toward a future re-score, though the risk-engine's own
                            "no second catalog mitigation" analysis above means a clean spike alone does
                            not automatically clear HIGH without a genuinely new B1/B2 catalog pattern
                            (a deliberate REF-risk-engine.md authoring act, never an inline plan-time
                            invention) — OR (b) Phase 0's re-verification against the post-M6/M7 tree
                            changes this risk's probability/severity picture.
```

### Cross-Cutting Factor Sweep (mandatory factors, woven into the risk rows + phases above)

- **security**: N/A — no auth/secrets/injection surface touched. Race/TOCTOU-class hazards (R1, R2,
  R3) are scored on their own merits below, not as security-class.
- **perf / BigO (mem + cpu)**: addressed. Auto-Arc emission (Phase 5) is ITSELF a perf feature —
  replacing a per-task deep copy with one refcount-shared allocation for genuinely-shared read-only
  values. Channel-close (Phase 4) adds O(1) state per channel (a closed flag / generation marker), not
  a new pass. Loom (Phase 3) and fuzzing (Phase 8) are dev/CI-time only — zero cost to compiled Yinz
  binaries. No new pass is added to the compiler's hot compile-time path.
- **accessibility**: N/A — compiler/runtime backend; no visual UI surface in this milestone's scope
  beyond the muted-hint wiring already governed by Teaching below.
- **PII / privacy**: N/A — compiler-internal; no user data handled.
- **compliance**: N/A — no regulatory scope.
- **SEO**: N/A — not web-facing.
- **docs**: addressed extensively — Phases 1, 2, 7 write real design-doc sections (`IMP-concurrency.md`
  channel-close design, `IMP-ownership.md` Arc-topology design, the Task-Cancellation section's
  resolution or re-deferral); Phase 9 sweeps registry/roadmap honesty.
- **reusability / DRY**: central — [authoritative-derivation.md](../../../rules/authoritative-derivation.md)
  governs both flagship fixes (Phase 5 reuses `effective_ownership` rather than re-deriving a read-only
  proof; Phase 4 reuses M6's single drop-glue choke point rather than forking a second cleanup path).
- **type-safety**: N/A beyond existing guarantees — no new user-facing type surface (channel-close's
  method and any Track-3 syntax are additive operations on existing types, not new types).
- **idempotency**: addressed — channel `.close()` (or whatever Phase 1 names it) must be idempotent
  (a double-close is a safe no-op, never a panic); Phase 7's handle-scope-drop, if implemented, reuses
  the already-idempotent `ynz_handle_free`.
- **error-handling**: addressed — the typed channel-closed error (the currently-dead Lock-8 path)
  becomes reachable and live (Phase 4); P2-7's fix (Phase 6) converts a silent hang into either correct
  wake-up or a loud, diagnosable failure.
- **observability / logging**: minor — no new user-facing logging surface required; the fuzzing
  harness's CI output (Phase 8) is dev/CI-facing, not a shipped observability feature (named here as
  considered and scoped out, not silently dropped).
- **race / TOCTOU**: central to the entire milestone — R1 (channel-close), R2 (Arc refcount), R3 (loom
  substrate itself targets this class directly), R5 (fuzzing differential oracle), R6 (P2-7) are all
  exactly this category.
- **resource-cleanup**: central — P2-3's leak fix (Phase 4), Arc refcount correctness (Phase 5), and
  Track 3's scope-drop investigation (Phase 7) are all resource-cleanup concerns.

## Design-Doc Alignment

Governing docs read live this session, per
[`.claude/rules/plan-invariants.md`](../../../rules/plan-invariants.md) `## Design-Doc Alignment`.

**Cited governing docs:**
[`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
("Channel/Queue Primitives," "Atomic Ordering Default," "Task Cancellation") ·
[`IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) ("Design
Divergences," the M3a-era deferral-writing pattern this plan's Phase 1 reuses for the channel-close
design section) ·
[`IMP-ownership.md`](../../../../docs/internal/implementation/IMP-ownership.md) (the Arc-topology
silence this plan's Phase 2 fills) ·
[`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) ·
[`registry/features.toml`](../../../../registry/features.toml) (`auto-arc-codegen-emission`,
`auto-arc-cautionary-tint`, `background-handle-cancel-injection`).

**Citation-depth verification (read live, not assumed):**
- `IMP-no-function-coloring.md`'s "Channel/Queue Primitives" section genuinely specifies bounded-by-
  default construction and the muted-hint pattern, but is SILENT on close/end-of-stream semantics
  entirely — confirming this is a real design gap, not merely an under-read one.
- `IMP-ownership.md` genuinely contains zero cross-thread-Arc-topology text (direct read, full file,
  this session) — the citation from `IMP-no-function-coloring.md:58` pointing there is a real, live
  dangling pointer, not a depth-miss on this plan's part.
- `IMP-no-function-coloring.md`'s "Task Cancellation" section genuinely specifies the LOCKED END-STATE model
  (cancel-via-drop at the next `wait` point, cleanup via existing drop semantics) and names its own
  language-half gap precisely — depth confirmed, this plan's Phase 7 is answering a real, specific,
  already-articulated open question, not inventing one.
- `authoritative-derivation.md` genuinely specifies the exact discipline Phase 4/5 need ("thread the
  SAME authoritative value/query... never let a second surface re-derive its own equivalent answer") —
  not a topical citation.

**Divergences:**

1. **`IMP-no-function-coloring.md` says** (Channel/Queue Primitives) channels are bounded by
   construction with an auto-inferred default capacity; **it says nothing about close semantics.**
   **This plan does NOT diverge from a stated claim** — there is no claim to contradict — but it fills
   a genuine silence: Phase 1 writes the missing design section (in `IMP-concurrency.md`, per its
   established Design Divergences home, matching M6 Phase 7's precedent of adding a new entry there
   rather than inventing a new doc-home), with Patrick's sign-off gate before Phase 4 implements it.
2. **`IMP-ownership.md` is silent on cross-thread Arc sharing topology**, despite
   `IMP-no-function-coloring.md:58` citing it as the mechanism's home. **This plan does NOT paper over
   the silence** — Phase 2 writes the missing section directly into `IMP-ownership.md`, resolving the
   dangling cross-reference, gated on Patrick's sign-off before Phase 5 implements against it.
3. **`IMP-no-function-coloring.md`'s Task Cancellation section says** the language-half gap (auto-`ynz_handle_free`
   at handle-scope-exit) is deferred to "the language-wide automatic scope-cleanup mechanism shipping."
   **This plan's Phase 7 either satisfies that trigger for real (if the existing drop-glue choke point
   extends cleanly) or formally re-defers with an updated registry entry and Patrick's sign-off** — it
   does not silently assume either outcome; the phase text carries both branches explicitly.
4. **Milestone-boundary assumption flagged**: M6 owns every concurrency-release audit finding EXCEPT
   P2-1/P2-3 (channel-close design gap, this plan's Phases 1&4), P2-6 (Auto-Arc emission, this plan's
   Phases 2&5), and P2-7 (handle panic-hang, this plan's Phase 6) — M6's own plan text names this
   boundary explicitly ("P2-3... genuinely unreachable dead code until channel-close semantics ship,"
   "P2-6... needs NO action this milestone — already correctly deferred," and P2-7 recorded in M6's own
   Future Requirements). This plan does not re-fix anything M6 or M7 already own; confirmed by direct
   read of both sibling plans' texts, not assumed.

## 2. Mission

Complete the concurrency feature set the design docs promise but the concurrency-release audit found
unshipped or unwired — channel close / end-of-stream semantics, Auto-Arc cross-thread sharing, a
resolved-or-re-deferred scope-drop cancellation model, and systematic loom + fuzzing verification —
**because** a teaching-mission compiler's flagship feature cannot stay silently incomplete against its
own design docs, and the two silent design-doc gaps this milestone closes (`IMP-ownership.md`'s
Arc-topology silence, the bare-channel non-closure footgun) are exactly the kind of undocumented debt
[`no-duct-tape.md`](../../../rules/no-duct-tape.md) exists to force into the open.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** Close the vision gap, not another bug list: every track in this plan ships a real design
decision (recorded, sign-off-gated) BEFORE its implementation lands, reuses an already-authoritative
source rather than re-deriving one wherever the M3a/M3d/M3e/M3g/M6 history warns it must, and leaves
every unresolved edge honestly parked with a trigger — never a silent gap.

**Key outcomes (definition of done):**

1. Channel close / end-of-stream semantics are designed (Patrick-signed), implemented, and shipped: an
   explicit close operation (name decided in Phase 1 against vocabulary.md + Golden Rule 12) makes
   `receive()` on a closed-and-drained channel return the typed channel-closed error (the currently-dead
   Lock-8 path goes live); P2-3's closed-send drop-glue leak is fixed through the SAME choke point M6
   registers for buffered-element cleanup; zero regression in the full pre-existing M4/M6 channel/
   handle fixture suite.
2. Auto-Arc cross-thread sharing ships: `IMP-ownership.md` states the caller/task sharing topology for
   real; codegen emits `ynz_arc_new`/`clone`/`free` at Arc-eligible spawn boundaries, proven by REUSING
   `effective_ownership::EffectiveOwnership::Reads` (never a second read-only classifier); the
   cautionary muted hint FIRES in normal styling (the red-tint stays separately deferred, unchanged);
   the `auto-arc-codegen-emission` registry entry is retired or narrowed to its real residual; alloc=free
   parity is proven non-vacuously against a concurrent-hammer fixture extended to cover the new codegen
   path.
3. Loom-based model checking covers the synchronization logic ynz-runtime owns directly (`pending_sends`
   mint/purge, the drop ladder, the recv-poll register/poll ordering) — honestly bounded to exclude
   Tokio's own internals — and lands BEFORE the new channel-close and Arc code, so both are
   loom-checked from birth, not retrofitted.
4. Source-level scope-drop cancellation is EITHER genuinely shipped (the existing drop-glue choke point
   extends cleanly to handle bindings) OR formally re-deferred with an updated registry entry and
   Patrick's sign-off — never silently left ambiguous.
5. A structured (grammar-constrained, type-valid-by-construction) fuzzing harness generates `.ynz`
   programs, compiles+runs each across `--no-optimize`/`--no-auto-parallel`/default mode combinations,
   and asserts identical observable behavior via the SAME oracle M7 built — wired into CI with a bounded
   time/iteration budget, never open-ended.
6. P2-7 (handle panic-then-pending hang) is fixed, mirroring M6 Phase 4's register-before-poll pattern.
7. `examples/pirates-roster/entrypoint.ynz` demonstrates channel close (and Auto-Arc, if a demonstrable
   surface exists post-Phase-5) in context; `examples/primantis-orders/m8_errors.ynz` carries WHY-
   commented triggers for every new compile-time diagnostic; the roadmap + BOTH duplicate Capability
   Ledger tables record M8, with row 442 and `background.cpuBound` explicitly reconciled as NOT
   absorbed; the full workspace suite is green.
8. Every unresolved edge (a re-deferred Track 3, any Auto-Arc topology residual, loom's Tokio-internals
   boundary, the fuzzing corpus's own backlog) is recorded in Future Requirements with a real trigger —
   never a loose checkbox.

**Disciplined initiative.** When steps and reality diverge: **verify before you fix** (every fix traces
to a CONFIRMED audit finding or a design decision this plan's own phases gate). **Thread the one
authoritative source; never invent a second derivation** to unblock yourself — surface the blocker
instead (CCIR-2). **A mitigation with no committed proof artifact is worth zero.** **No duct tape** — a
fix that "mostly" closes a design gap, with no four-field deferral naming the remaining gap, is not
done. **Design phases gate their own implementation phases** — Phase 4 does not start until Phase 1's
sign-off lands; Phase 5 does not start until Phase 2's sign-off lands.

### 3.2 Concept

Ten phases (0–9). **Gate first** (Phase 0 confirms the double merge-and-tag precondition + re-verifies
terrain against the post-M6/M7 tree). **Design phases run in parallel-safe order** (Phase 1 channel-
close design, Phase 2 Arc-topology design — neither depends on the other). **Loom substrate lands next**
(Phase 3), covering the EXISTING (M6-fixed) synchronization logic before any new code is added on top of
it, per the brief's explicit sequencing instruction. **Implementation follows each design's sign-off**
(Phase 4 channel-close + P2-3 fix; Phase 5 Auto-Arc emission — both now loom-instrumented from birth).
**Small mechanical fix** (Phase 6, P2-7). **Track 3** (Phase 7, design + contingent implementation, can
run anytime per the brief — sequenced here for convenience, not a hard dependency). **Verification
completion** (Phase 8, structured fuzzing). **Close-out** (Phase 9: demo/gallery/registry/roadmap/full-
suite/release-handoff).

### 3.3 Phases

#### Phase 0 — Gate: double merge-and-tag precondition + terrain re-verification

- **Task + purpose:** confirm BOTH sibling plans (M6, M7) have merged to `main` before any other phase
  starts, and re-verify every audit citation this plan depends on against the POST-M6-POST-M7 tree
  (both siblings touch `channel.rs`/`handle.rs`/`runtime.rs`/`emit.rs`).
- **Steps**
  1. Confirm `main` includes the M6 hotfix merge AND the M7 optimizer-pipeline merge. **STOP the whole
     plan if either is missing** — do not proceed to Phase 1.
  2. Re-read `channel.rs`, `handle.rs`, `runtime.rs`, and the relevant `emit.rs` regions this plan cites
     (P2-1/P2-3/P2-6/P2-7 terrain above) against the current tree; record any line-number drift (the
     fix's substance carries forward even if a line number moved) and any substantive change M6/M7
     already made that alters this plan's assumptions (e.g., if M6's Phase 3 fix changed
     `pending_sends`'s shape in a way that affects Phase 3's loom-substrate design).
  3. Confirm the `auto-arc-codegen-emission`, `auto-arc-cautionary-tint`, and
     `background-handle-cancel-injection` registry entries are still present and unchanged from the
     citations in ¶1 Terrain.
  4. Confirm row 442 (selective hot-field gather) and `background.cpuBound` (P4-2) remain correctly
     un-absorbed in both the roadmap's Capability Ledger tables and M6/M7's Future Requirements —
     re-affirm this plan's own non-absorption decision on the same grounds.
- **Exit criteria:** double merge-and-tag precondition confirmed; every cited file:line re-verified
  against the current tree with drift recorded; registry entries confirmed present; non-absorption
  decisions re-affirmed.
- **Reviewer fan-out:** design-doc-alignment reviewer (the execution-gate precondition + the
  re-verification itself).
- **Model tag:** `(coding, standard, small)`

#### Phase 1 — Design: Channel Close / End-of-Stream Semantics (Patrick sign-off gate)

- **Task + purpose:** decide, design, and document the channel-close mechanism BEFORE any
  implementation lands — the DESIGN-FIRST phase the brief mandates, producing an `IMP-concurrency.md`
  section + a Patrick sign-off gate.
- **Steps**
  1. Read `channel.rs`'s current endpoint-holding architecture in full (not just the cited lines) to
     understand exactly why the object retaining both endpoints structurally prevents closure, and what
     changing that would require (does the object need to stop holding its own `Sender`? does each
     logical producer need its own tracked clone-count distinct from the object's internal reference?).
  2. Enumerate mechanism candidates with an honest feasibility note on each: **(a) explicit close** — a
     dot-postfix action method the caller calls when done sending (simplest, no producer-ref tracking
     needed, but requires the CALLER to remember to call it); **(b) auto-close-on-last-producer-drop** —
     the channel closes itself when every logical sender-side binding has gone out of scope (matches
     Rust/Tokio's own model, but requires the channel to distinguish "my own internal Sender clone" from
     "a logical producer's Sender clone" — genuinely harder given today's type-erased Arc-refcounted
     channel handle, per the brief's own framing); **(c) both** — explicit close as the primary
     mechanism, auto-close as a future enhancement once producer-ref tracking exists.
  3. Evaluate naming candidates for the explicit-close method against
     [`.claude/rules/vocabulary.md`](../../../rules/vocabulary.md) and Golden Rule 12 (human-readable,
     no jargon) — candidates to weigh: `.close()`, `.done()`, `.finish()` — pick the one an HS-grad
     reads most naturally as "I'm done sending," and confirm it doesn't collide with any planned
     future-stdlib naming (no `file`/`io` module exists yet to collide with, confirmed).
  4. Decide the mechanism (explicit-only, or explicit + a stated auto-close deferral) and the method
     name. Record the decision AND the reasoning — this is the durable design call the brief scopes to
     this phase, not something a later phase re-litigates.

     **CHECKPOINT** — mechanism + name decided and reasoned; design-doc drafting (next steps) not yet
     started.
  5. Write the design into `IMP-concurrency.md`'s established "Design Divergences" section home (per M6
     Phase 7's precedent, which already added the bare-channel-footgun entry there) — OR promote it to
     its own subsection if the mechanism is substantial enough to outgrow the Divergences format (call
     this at draft time, not presupposed). The section states: the mechanism, the idempotency contract
     (a double-close is a safe no-op), how `receive()` behaves post-close-and-drain (returns the typed
     channel-closed error — confirm which existing error-type variant this reactivates, per the audit's
     "Lock-8" naming), the deliberate NON-mechanism (if auto-close-on-drop is deferred, name it as a
     real four-field deferral, not silent), and retire or rewrite the bare-channel-footgun entry M6
     Phase 7 added (it becomes stale once this ships).
  6. Draft the WHAT/WHAT-INSTEAD/WHY teaching text for the newly-live channel-closed error (Golden Rule
     11) and any new compile-time diagnostic the mechanism introduces (e.g., calling `.send()` after
     close, if that becomes a compile-time-checkable case rather than a runtime error — decide and
     record which).
  7. **Patrick sign-off gate**: surface the decided mechanism, the drafted doc section, and the teaching
     text for explicit approval before Phase 4 implements against it.
- **Exit criteria:** mechanism + name decided and reasoned; `IMP-concurrency.md` section drafted;
  teaching text drafted; Patrick's sign-off recorded (blocks Phase 4 until this lands).
- **Reviewer fan-out:** docs-consistency reviewer (vocabulary.md + Golden Rule 12 compliance on the
  naming call); design-doc-alignment reviewer (does the design genuinely resolve the silence, or paper
  over it).
- **Model tag:** `(reasoning, high, medium)` — checkpoint mark mandatory (>5 steps).

#### Phase 2 — Design: Auto-Arc Sharing Topology (Patrick sign-off gate)

- **Task + purpose:** write the missing caller/task Arc-sharing-topology section into
  `IMP-ownership.md`, resolving the registry's own self-diagnosed gap, reusing
  `effective_ownership::EffectiveOwnership::Reads` as the authoritative read-only proof — a Patrick
  sign-off gate before Phase 5 implements against it. Parallel-safe with Phase 1 (no shared surface).
- **Steps**
  1. Read `crates/ynz-typeck/src/effective_ownership.rs` in full to confirm exactly what
     `EffectiveOwnership::Reads` proves today (read-only usage of a `share`-eligible binding) and
     whether it is DIRECTLY reusable for the cross-thread-sharing question (is a value read-only in the
     CALLER after the spawn AND read-only in the SPAWNED task, or does the analysis need a small,
     honest extension to answer the two-sided question — record which, do not assume).
  2. Decide the sharing topology: does a shared Arc repoint the CALLER's own binding to the Arc'd
     allocation (so the caller and every spawned task share one physical allocation), or does each
     spawned task receive an independent Arc CLONE of one shared allocation (same allocation, N
     refcounted handles) while the caller keeps its own direct access? Read `IMP-concurrency.md`'s
     "Ownership with Background Tasks" section (the existing `.give`/`.copy` inference table) to confirm
     which topology composes cleanly with the ALREADY-shipped `.give`/`.copy` auto-inference rather than
     creating a THIRD, competing inference path.
  3. Decide the BENEFICIAL-emission proof obligation precisely (per the registry entry's own `why`
     field): a single-task spawn with no other reader is a pessimization (Arc header + atomic ops for
     zero sharing benefit) — record the exact condition under which emission is beneficial (≥2 readers
     of one allocation: the caller after spawn-return, plus ≥1 spawned task; or ≥2 spawned tasks sharing
     one value) versus when the existing `.copy` path stays correct-and-cheaper.

     **CHECKPOINT** — topology + beneficial-emission condition decided and reasoned; doc-drafting (next
     steps) not yet started.
  4. Write the section into `IMP-ownership.md` (a new subsection, cross-referenced from
     `IMP-no-function-coloring.md:58`'s existing dangling pointer) stating: the topology, the
     beneficial-emission condition, the reuse of `EffectiveOwnership::Reads` (or its honest extension,
     per step 1) as the read-only proof, and the override-direction analysis per
     [`.claude/rules/auto-promotion.md`](../../../rules/auto-promotion.md) (force-the-auto-pick and
     force-the-other-pick — does `.give`/`.copy`'s existing explicit-override syntax already cover both
     directions, or does Auto-Arc need its own, e.g. `.share` reinterpreted at a `background` boundary —
     decide and record, checking against `IMP-concurrency.md`'s existing hard-error on `.share` across
     `background` boundaries so this doesn't silently reopen that guard).
  5. Draft the cautionary muted-hint hover text update (the `auto_arc` domain already exists in the
     registry with placeholder hover text) — confirm it still matches the decided topology; update if
     the topology decision changes what the hint should say.
  6. **Patrick sign-off gate**: surface the decided topology, the drafted `IMP-ownership.md` section,
     and the override-direction analysis for explicit approval before Phase 5 implements against it.
- **Exit criteria:** topology + beneficial-emission condition decided and reasoned; `IMP-ownership.md`
  section written, resolving the dangling cross-reference; override-direction analysis complete;
  Patrick's sign-off recorded (blocks Phase 5 until this lands).
- **Reviewer fan-out:** design-doc-alignment reviewer (does the section genuinely resolve the silence
  and the dangling cross-reference); code-reviewer (does the reuse of `EffectiveOwnership::Reads` hold
  up against a direct read of that module, not merely the registry's own characterization of it).
- **Model tag:** `(reasoning, high, medium)` — checkpoint mark mandatory (>5 steps).

#### Phase 3 — Loom Substrate: Spike + Model-Checking Harness for Runtime Sync Primitives

- **Task + purpose:** prove loom can practically model-check the synchronization logic ynz-runtime owns
  directly (`pending_sends` mint/purge, the drop ladder, the recv-poll register/poll ordering — all
  M6-fixed logic), establish the loom-swappable-type pattern, and land it BEFORE Phases 4/5 add new
  state on top of it. This is a [plan-spike-discipline](../../../rules/plan-spike-discipline.md)
  Facet-1 hard gate — real refactor work, scoped honestly to exclude Tokio's own internals.
- **Steps**
  1. **Spike (hard gate):** on a throwaway scratch crate or module, put a MINIMAL reproduction of the
     `pending_sends` mint/purge logic (a `HashMap<u64, PendingSend>` guarded by a lock, keyed by a
     salted token) behind loom-swappable types (`loom::sync::Mutex` under `#[cfg(loom)]`,
     `std::sync::Mutex` otherwise) and run loom against it, exhaustively checking the exact
     ABA/orphan-purge invariant M6 Phase 3 establishes. Confirm loom's state-space explosion is
     TRACTABLE for this scope (bounded iteration count, completes in CI-reasonable time) — if it is not,
     this is a RED verdict, not a silent timeout tolerated away.
  2. **STOP-condition:** GREEN if the spike's loom run completes in bounded time and catches an
     intentionally-reintroduced version of the ABA bug (prove the harness actually detects the failure
     mode, not just that it runs). RED if loom cannot practically explore the real state space, or
     cannot be swapped in without changing production-path types — in which case Track 4a's honest shape
     is a documented deferral (four fields) rather than a half-built harness, surfaced via the CCIR
     below.
  3. On GREEN: confirm via a compiled-binary diff (or an IR/codegen-level check) that the
     `#[cfg(not(loom))]` production path is BYTE-IDENTICAL to pre-refactor codegen — the substrate must
     be provably a no-op in production builds, per R3's mitigation.

     **CHECKPOINT** — spike GREEN, state-space tractability proven, production-path no-op confirmed;
     applying the pattern to the real (non-scratch) `channel.rs`/`handle.rs`/`runtime.rs` code (next
     steps) not yet started.
  4. Apply the proven pattern to the REAL `pending_sends` mint/purge logic, the drop ladder
     (`runtime.rs:591-693`), and the recv-poll register/poll ordering (M6 Phase 4's fix) — behind the
     SAME loom-swappable types, with loom model-check tests exhaustively covering: the ABA/orphan-purge
     invariant (both token-producer paths, per M6's addendum), the drop-ladder ordering, and the
     register-before-poll ordering.
  5. Run the loom suite + the full pre-existing test suite; confirm zero regression and confirm loom
     genuinely catches each of the three invariants above by TEMPORARILY reverting each fix in a
     disposable branch and confirming loom flags it (proof the harness has teeth, not merely presence).
  6. Document the harness's EXPLICIT boundary: it covers ynz-runtime-owned synchronization state; it
     does NOT and cannot model-check Tokio's own internal `mpsc`/scheduler implementation — record this
     as a named scoping decision in this plan's Future Requirements, not a silent gap.
- **Exit criteria:** GREEN spike verdict with a proven-tractable state space; production-path no-op
  confirmed; the real (non-scratch) sync logic covered with loom tests proven to have teeth (each
  reverted fix caught); the Tokio-internals boundary named explicitly.
- **Reviewer fan-out:** adversarial gate-checker (does the spike's GREEN verdict genuinely prove
  tractability and detection, not just "the harness compiled"); code-reviewer (the loom-swappable type
  pattern applied to the real code); design-doc-alignment reviewer (the harness sits in front of the
  existing reactive test suite as an ADDITIONAL check, never a replacement).
- **Model tag:** `(coding, high, medium)` — checkpoint mark mandatory (>5 steps).

#### Phase 4 — Implement: Channel Close Semantics + P2-3 Leak Fix

- **Task + purpose:** implement Phase 1's signed-off design — the explicit close mechanism, the live
  typed channel-closed error, and P2-3's closed-send drop-glue leak fixed through M6's single choke
  point — now loom-instrumented from birth via Phase 3's substrate.
- **Steps**
  1. Confirm Phase 1's sign-off is recorded before starting (hard gate).
  2. Implement the decided close mechanism on `YnzChannel` (whatever architecture change Phase 1's
     design calls for — likely: the object stops treating its retained endpoint as a permanent producer,
     and/or a closed-flag/generation marker gates `send`/`receive` post-close), wired into Phase 3's
     loom-swappable substrate from the start (no retrofit).
  3. Wire `receive()` on a closed-and-drained channel to return the typed channel-closed error (the
     Lock-8 path) — confirm the closed-recv codegen arm's existing "structurally unreachable" comment
     is removed and replaced with the real reachable path.
  4. Fix P2-3: route the closed-send blocks' payload cleanup through the SAME drop-glue choke point M6
     Phase 5 registers for buffered-element cleanup (authoritative-derivation.md — confirm this is
     literally the same function pointer / call site, not a second implementation).

     **CHECKPOINT** — close mechanism + live error path + P2-3 fix all implemented; fixture authoring
     (next steps) not yet started.
  5. Author the RED→GREEN fixture class (per R1's mitigation): explicit close then receive-drains-then-
     errors, double-close idempotency, drop-without-close (confirm this still behaves per the PRE-close
     behavior for any channel never explicitly closed — no regression), concurrent send-during-close.
     Commit RED before the fix, confirm GREEN after.
  6. Retire or rewrite the bare-channel-footgun `IMP-concurrency.md` Design Divergences entry M6 Phase 7
     added — it is stale once this ships; replace with a pointer to the new design section (Phase 1's
     doc work) rather than leaving two contradictory doc entries live.
  7. Add the registry entry for the close method (kind depends on Phase 1's decision — likely a
     `[[primitive_intrinsic]]` entry for the channel-attached method) per
     [`.claude/rules/feature-registry.md`](../../../rules/feature-registry.md).
  8. Run the full pre-existing M4/M6 channel/handle fixture suite + the new RED→GREEN class + the Phase
     3 loom suite together; confirm zero regression.
- **Exit criteria:** close mechanism live; typed channel-closed error reachable; P2-3 leak fixed through
  the one choke point; RED→GREEN fixture class committed; stale doc entry retired; registry entry added;
  full suite (existing + new + loom) green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the fixture class genuinely
  exercise close/drain/double-close/concurrent-close, not just the happy path); design-doc-alignment
  reviewer (does the implementation match Phase 1's signed-off design exactly, not a drifted variant).
- **Model tag:** `(coding, high, large)` — checkpoint mark mandatory.

#### Phase 5 — Implement: Auto-Arc Codegen Emission + Cautionary Hint + Fixtures

- **Task + purpose:** implement Phase 2's signed-off topology — codegen emission of
  `ynz_arc_new`/`clone`/`free` at Arc-eligible spawn boundaries, reusing `effective_ownership` as the
  sole read-only proof, wiring the muted hint to fire, and proving refcount correctness non-vacuously —
  now loom-instrumented from birth via Phase 3's substrate. **R2 (¶1 Risk Assessment) governs this
  phase's correctness hazard** — its HIGH residual carries a RISK OVERRIDE, signed by Patrick at
  Gate-4 approval (2026-07-04), clearing this phase to begin Step 2 (the spike).
- **Steps**
  1. Confirm Phase 2's sign-off is recorded before starting (hard gate).
  2. **Spike (per [plan-spike-discipline](../../../rules/plan-spike-discipline.md) Facet 1 — net-new,
     load-bearing codegen mechanism):** on a minimal throwaway fixture, emit the Arc-wrap at ONE spawn
     site, reusing `EffectiveOwnership::Reads` for the proof, and confirm (a) refcounts balance under a
     single spawn+join, and (b) the emitted calls interact correctly with the frame-layout/state-machine
     embedding used for `background` spawns (no aliasing violation against existing `noalias`/`readonly`
     LLVM attributes M7's Phase 1 audit already confirmed on `ynz_arc_*`). GREEN/RED verdict before
     extending to the full codegen path.
  3. On GREEN, extend the emission to every Arc-eligible spawn boundary per Phase 2's decided beneficial-
     emission condition (≥2 readers of one allocation) — for the single-reader case, confirm the
     existing `.copy` path is UNCHANGED (no regression to the correct-and-cheaper existing behavior).

     **CHECKPOINT** — spike GREEN, full emission implemented for the beneficial case, existing `.copy`
     path confirmed unchanged; hint-wiring and fixture work (next steps) not yet started.
  4. Wire the `auto_arc` muted-hint domain (already registered) to actually FIRE in `crates/ynz-lsp/src/
     inlay_hint.rs`, in normal muted styling (the `auto-arc-cautionary-tint` red-tint entry stays
     separately, unchanged, deferred — no per-hint tint path exists yet, confirmed in ¶1 Terrain).
  5. Extend the EXISTING concurrency-hammer Arc test (`arc.rs`'s substrate-level hammer test) to also
     exercise the NEW codegen-emitted call sites end-to-end (a compiled `.ynz` program spawning multiple
     tasks sharing one Arc'd value under real concurrent load), not just the runtime substrate in
     isolation.
  6. Author the non-vacuous `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity gate for the new emission path
     (per M5's FRAGO-005 lesson — confirm real, non-zero Arc allocations are exercised, never a vacuous
     pass), plus a loom test (Phase 3's substrate) covering the Arc refcount acquire-release protocol
     under the new codegen-emitted call pattern specifically (distinct from the existing substrate-level
     loom coverage, if any).
  7. Retire the `auto-arc-codegen-emission` registry entry (if the shipped emission covers the FULL
     beneficial-emission condition Phase 2 decided) OR narrow it to name the real remaining residual
     (if Phase 2's topology decision left a bounded slice unimplemented — e.g., multi-task fan-out beyond
     N readers) — mirror the `ec-wrapper-collect-on-completion` retirement-note convention.
  8. Run the full pre-existing suite + the new hammer-fixture extension + the parity gate + the loom
     tests together; confirm zero regression.
- **Exit criteria:** R2's RISK OVERRIDE is signed before Step 2 begins; spike GREEN; emission live for
  the beneficial case; existing `.copy` path unchanged; muted hint fires; hammer fixture extended;
  parity gate non-vacuous; loom coverage added; registry entry retired or honestly narrowed; full suite
  green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (R2: does the spike + hammer-fixture
  extension + parity gate genuinely prove refcount and frame-layout correctness before merge, not
  merely asserted; does the parity gate and hammer extension genuinely exercise the new codegen path
  under real concurrent load, not a single-threaded happy path); design-doc-alignment reviewer
  (authoritative-derivation.md — `EffectiveOwnership::Reads` reused, never re-derived, grep-verified).
- **Model tag:** `(coding, high, large)` — checkpoint mark mandatory.

#### Phase 6 — P2-7: `ynz_handle_recv_poll` Panic-Then-Pending Hang

- **Task + purpose:** close the newly-surfaced (M6 audit) handle-side panic-then-pending hang, mirroring
  M6 Phase 4's exact register-before-poll fix for the sibling channel-side race.
- **Steps**
  1. Re-confirm `handle.rs:297-303`'s current behavior (a panic inside the poll returns `Pending` with
     a possibly-unregistered waker) against the post-M6 tree (M6 Phase 4 may have touched adjacent
     code — confirm this specific path is unchanged by that fix).
  2. Fix: register the waker BEFORE the poll body that could panic, mirroring M6 Phase 4's channel-side
     pattern exactly (same ordering discipline, same reasoning) — or hold a single lock across
     register+poll, whichever mirrors M6's chosen shape most closely.
  3. Author a RED repro: force a panic inside the poll body via a controlled test harness, confirm the
     task is still woken (does not hang) after the fix.
  4. Re-verify M6's own "no lock held across a blocking poll" invariant still holds after this change
     (do not assume — re-read the changed code against that specific invariant, same discipline M6
     Phase 4 applied to itself).
  5. Run the full suite; confirm zero regression.
- **Exit criteria:** panic-then-pending hang closed; RED→GREEN fixture; the no-lock-across-blocking-poll
  invariant re-verified, not merely carried forward.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the repro genuinely force the
  panic-before-registration window, not a different failure shape).
- **Model tag:** `(coding, standard, small)`

#### Phase 7 — Track 3: Source-Level Scope-Drop Cancellation (Design + Contingent Implementation)

- **Task + purpose:** investigate whether extending the SAME drop-glue choke point M6/Phase 4 register
  for channel cleanup to `background` handle bindings is a small, contained fix — ship it if so; author
  a formal, signed re-deferral if the investigation finds it genuinely requires the general language-
  wide scope-drop mechanism. Both branches are legitimate; this phase does not force one.
- **Steps**
  1. Read the compiler's actual scope-exit cleanup dispatch for existing droppable types (arrays, maps,
     shapes, strings, and — post-Phase-4 — channels) to determine: is there ALREADY a generic "walk
     every local of a droppable type at scope exit, call its drop fn" mechanism these all plug into, or
     is each type's cleanup ad hoc? This is the load-bearing recon question the brief's contingency
     hinges on.
  2. If a generic per-type drop-dispatch choke point genuinely exists: evaluate whether adding
     `background` handle bindings to that SAME dispatch (calling the already-tested, already-loom-
     covered `ynz_handle_free` at scope exit) is architecturally clean — no new cancellation semantics
     needed, since `ynz_handle_free` already implements cancel-via-drop correctly per
     `IMP-no-function-coloring.md`'s locked end-state model.

     **CHECKPOINT** — recon complete, decision point reached (implement vs. re-defer); the chosen
     branch's work (next steps) not yet started.
  3. **Branch A — implement:** wire handle bindings into the existing dispatch; author a RED→GREEN
     fixture proving a handle binding going out of scope without an explicit `.receive()`/`.send()`
     cancels the task at its next suspension point (per the already-shipped runtime behavior); confirm
     this reuses Phase 3's already-loom-covered `ynz_handle_free` path (no NEW sync primitive
     introduced, so no additional loom work required — cite this reuse explicitly); update
     `IMP-no-function-coloring.md`'s Task Cancellation section to state the language half is now SHIPPED; retire
     the `background-handle-cancel-injection` registry entry.
  4. **Branch B — re-defer:** if the recon in step 1 finds no clean generic dispatch exists (each type's
     cleanup is ad hoc, and unifying them IS the general mechanism the doc's own parenthetical warns
     about), author a fresh four-field deferral (WHAT/WHY/COST/TRIGGER) and update
     `background-handle-cancel-injection`'s `ships_in`/`triggers` fields to reflect this milestone's
     concrete finding (not a repeat of the vague prior wording) — surface for Patrick's sign-off before
     closing this phase, since this is a real design call about a locked end-state model, not a routine
     technical decision.
  5. Whichever branch fires, run the full pre-existing suite; confirm zero regression.
- **Exit criteria:** the recon question (does a generic drop-dispatch choke point exist) is answered
  with evidence, not assumed; EITHER Branch A ships with a proven fixture and a retired registry entry,
  OR Branch B lands a signed, concrete four-field re-deferral; full suite green either way.
- **Reviewer fan-out:** code-reviewer; design-doc-alignment reviewer (does the chosen branch genuinely
  match `IMP-no-function-coloring.md`'s locked end-state model, and does Branch A avoid forking a second cleanup
  mechanism per authoritative-derivation.md).
- **Model tag:** `(reasoning, high, medium)` — checkpoint mark mandatory (>5 steps, contingency
  branching).

#### Phase 8 — Structured Fuzzing Harness + CI Wiring (Track 4b)

- **Task + purpose:** build a grammar-constrained (type-valid-by-construction) `.ynz` program generator,
  drive it through build+run across mode combinations, and assert observable-behavior equivalence via
  M7's existing cross-implementation-consistency oracle — wired into CI with a bounded budget.
- **Steps**
  1. **Spike (net-new, load-bearing infra — plan-spike-discipline Facet 1):** build a MINIMAL grammar
     covering a small composable subset (independent statements, `wait`/`background` spawns, `channel<T>`
     send/receive, basic shape/array operations) that generates programs guaranteed to TYPECHECK by
     construction (a combinator-based generator drawing only from valid, type-consistent constructs —
     never unconstrained token fuzzing, which would mostly generate typeck-rejected garbage and waste
     cycles). Confirm a small generated sample compiles and runs across at least one mode.
  2. **STOP-condition:** GREEN if the generator reliably produces compiling, running programs (a
     meaningful hit rate, not 0%). RED if the constrained grammar still produces mostly-rejected
     programs — in which case narrow the grammar further before proceeding, or record a scoped-down
     Track 4b as a documented deferral rather than shipping a low-yield harness.
  3. On GREEN, extend the generator's grammar coverage to the full composable subset named in step 1,
     and wire each generated program through `crates/ynz-driver/tests/cross_impl_consistency.rs`'s
     existing oracle logic (extended, not re-derived) across `--no-optimize` / `--no-auto-parallel` /
     default mode combinations — assert byte-identical stdout/stderr/exit-code across every combination
     for every generated program.

     **CHECKPOINT** — generator grammar complete, oracle wiring complete; CI budget + backlog-routing
     work (next steps) not yet started.
  4. Set a bounded time/iteration budget for CI (a fixed corpus size or wall-clock cap per CI run — never
     open-ended AFL-style continuous fuzzing) and wire it into
     [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml) as its own job, non-blocking on
     first landing (a genuine finding routes through the FRAGO seam per R5, not an immediate build
     break, until the harness has proven itself stable over some real CI runs).
  5. Run the harness locally for a real (not toy) corpus size; if it surfaces any genuine miscompile,
     route it through the plan-amendment/FRAGO seam per R5's mitigation — do not silently fix inline
     unless the finding is trivially the SAME class already fixed by an earlier phase in this plan (name
     which, if so).
  6. Document the harness's scope (the grammar's coverage, the mode-combination matrix, the CI budget)
     and its own backlog mechanism (where a corpus of interesting failing/regression cases gets saved
     for future replay) in a short design note alongside the harness code.
- **Exit criteria:** spike GREEN with a meaningful hit rate; full grammar wired to the extended oracle;
  CI job wired with a bounded, documented budget; any genuine finding routed through the FRAGO seam, not
  silently absorbed or silently dropped.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the generator genuinely produce
  type-valid, non-trivial programs, or a narrow toy subset masquerading as coverage); design-doc-
  alignment reviewer (does the CI wiring's budget genuinely bound runtime, per the "never open-ended"
  requirement).
- **Model tag:** `(coding, high, medium)` — checkpoint mark mandatory (>5 steps).

#### Phase 9 — Demo & Error Gallery + Registry/Roadmap Reconciliation + Full-Suite Gate + Release Handoff

- **Task + purpose:** close the plan-invariants Demo & Error Gallery obligation, reconcile the roadmap +
  BOTH duplicate Capability Ledger tables, run the cumulative full-suite gate, and confirm release
  preconditions.
- **Steps**
  1. Extend `examples/pirates-roster/entrypoint.ynz` with a channel-close section demonstrating the
     decided mechanism in a realistic context (a Pirate/Ship-domain producer/consumer pair, not a bare
     demo) — real operations only, per
     [`.claude/rules/dot-postfix.md`](../../../rules/dot-postfix.md)'s examples-must-use-real-operations
     rule. If Phase 5's Auto-Arc emission has a demonstrable source-level surface (it may not — codegen-
     only per Phase 2's design), add that too; otherwise state explicitly why no new demo section
     applies (informational-only, no typeable form). Regenerate + commit the byte-exact golden.
  2. Create `examples/primantis-orders/m8_errors.ynz` with intentional triggers for every new compile-
     time diagnostic this milestone adds (Phase 1/4's channel-close diagnostics; any new diagnostic
     Phase 7's Branch A might add, if it ships). Wire the new gallery's assertions into
     `crates/ynz-driver/tests/error_galleries.rs` (diagnostic-count + key-phrase convention).
  3. Update the roadmap's `milestones:` frontmatter list (add
     `v0-3-m8-concurrency-completion`) and BOTH duplicate `## Capability Ledger` tables (per the
     established M6/M7 both-tables convention — updated in lockstep, never one and not the other).
     **Both tables currently carry one combined placeholder row — "Concurrency completion... status:
     being authored" (roadmap.md line ~445 and its duplicate at line ~499) — authored when this plan
     was itself only a roadmap entry.** That single placeholder row is **REPLACED BY the four granular
     rows below, in BOTH tables, in the same lockstep edit** — it does not survive alongside them as a
     fifth, now-stale summary row:
     - New row: channel close / end-of-stream semantics — ABSORBED, this plan's Phases 1 & 4.
     - New row: Auto-Arc codegen emission — ABSORBED, this plan's Phases 2 & 5 (narrow the Notes column
       to whatever residual Phase 5 step 7 named, if any).
     - New row: source-level scope-drop cancellation — ABSORBED (Branch A) or NOT fully absorbed with
       the Branch B re-deferral cited (Notes column states which).
     - New row: loom + structured fuzzing verification — ABSORBED, this plan's Phases 3 & 8, with the
       Tokio-internals boundary named in Notes.
     - Row "Selective hot-field-only element materialization" (both tables): **NOT absorbed** —
       orthogonal SoA-gather perf gap, unrelated to concurrency completion (confirmed independently of
       M7's own non-absorption, for this milestone's own reason).
     - `background.cpuBound` (P4-2, not a ledger row but tracked in M6/M7 Future Requirements): confirm
       it remains correctly un-absorbed; this plan's own Future Requirements carries it forward.
  4. Run the cumulative full workspace gate: `cargo fmt --check && cargo clippy --workspace -- -D
     warnings && cargo test --workspace && cargo build --workspace --release`, plus the Phase 3 loom
     suite and the Phase 8 fuzzing harness's bounded local run, all green together — never proven
     piecemeal.
  5. Confirm every Future Requirements entry below is present with its four fields, cross-referencing
     the registry entries this plan retired or narrowed.
- **Exit criteria:** demo + gallery extended (or explicitly N/A with reason); both Capability Ledger
  tables + roadmap milestones list updated in lockstep; cumulative full-suite gate green; Future
  Requirements complete and cross-referenced.
- **Reviewer fan-out:** docs-consistency reviewer; code-reviewer; design-doc-alignment reviewer (final
  sweep against every citation this plan made).
- **Model tag:** `(coding, standard, medium)` — checkpoint mark mandatory (>5 steps).

### 3.4 Coordinating Instructions

- **Hard sequencing:** Phase 0 gates everything. Phase 4 does not start before Phase 1's sign-off.
  Phase 5 does not start before Phase 2's sign-off. Phase 3 (loom substrate) lands before Phase 4 and
  Phase 5's IMPLEMENTATION work begins (design phases 1/2 may run before, during, or after Phase 3 —
  they are pure documentation/decision work with no shared code surface).
- **CCIR (surface immediately, never silently absorb or silently drop):**
  1. If Phase 0 finds M6 or M7 has NOT merged — halt, do not proceed.
  2. If Phase 1 or Phase 2's design investigation finds the mechanism requires MORE than a contained
     change (e.g., channel-close genuinely requires a producer-ref-counting redesign beyond an explicit
     `.close()`; Arc topology genuinely requires extending `effective_ownership` itself rather than
     reusing it as-is) — surface before drafting the sign-off gate; this is a scope discovery, not a
     silent absorption.
  3. If Phase 3's loom spike returns RED — surface immediately; Track 4a's shape becomes a documented
     deferral via the plan-amendment/FRAGO seam, not a quietly-scaled-back harness.
  4. If Phase 7's recon finds the generic drop-dispatch choke point does NOT exist — take Branch B
     (re-defer) as designed; this is not a failure, it is the phase's own honest exit.
  5. If Phase 8's fuzzing harness finds a genuine miscompile — route it through the plan-amendment/
     FRAGO seam per R5; never fix inline unless it is trivially the SAME class an earlier phase in THIS
     plan already fixed (name which).
  6. Any newly-discovered risk that scores HIGH or EX-HIGH at any point — surface immediately per the
     risk-engine gate; this plan carries exactly one pre-drafted override, R2's (SIGNED by Patrick at
     Gate-4 approval, 2026-07-04), and no other — any OTHER HIGH/EX-HIGH finding gets its own
     freshly-drafted, unsigned override, never folded into R2's or self-signed.

## Invariants This Milestone Must Preserve

Per [`.claude/rules/plan-invariants.md`](../../../rules/plan-invariants.md) (M4-onward mandatory
section; v0.2-M2-onward `### Feature Registry Entries`) — seven required sub-sections, each a testable
assertion, not an aspiration.

### Safety

- **Channel-close semantics cannot regress existing (M4/M6-fixed) channel behavior — RED class as
  testable assertions (Phase 4 step 5, gating Phase 4's exit criteria):**
  - Explicit close, then full drain, then `receive()` returns the typed channel-closed error (the
    previously-dead Lock-8 path) — never blocks, never panics.
  - Double-close is a safe no-op (the idempotency contract Phase 1 decides and Phase 4 implements) —
    never a panic on a second `.close()`/equivalent call.
  - Drop-without-close (a channel NEVER explicitly closed) behaves byte-for-byte identically to the
    pre-Phase-4 behavior — zero regression, proven against the FULL pre-existing M4/M6 channel/handle
    fixture suite (Phase 4 exit criteria), not merely the new RED→GREEN class in isolation.
  - Concurrent send-during-close does not race or corrupt channel state — covered by the same RED→GREEN
    class AND Phase 3's loom coverage of the same ordering (Phase 4 step 2's loom-instrumented-from-birth
    requirement).
  - P2-3's closed-send drop-glue leak is fixed through the SAME choke point M6 registers for
    buffered-element cleanup (Phase 4 step 4) — no heap-typed payload leaks on a closed-send, and no
    SECOND drop-glue implementation is introduced (authoritative-derivation.md).
- **Arc refcount balance is proven non-vacuously** — the `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free parity
  gate (Phase 5 step 6) exercises REAL, non-zero Arc allocations under the NEW codegen-emitted call
  sites (per M5's FRAGO-005 lesson: a vacuous zero-alloc pass is not proof of anything).
- **Loom lanes are green** — Phase 3's loom suite (the `pending_sends` mint/purge ABA/orphan-purge
  invariant, the drop-ladder ordering, the register-before-poll ordering) is proven to have TEETH: each
  temporarily-reverted fix is caught by loom (Phase 3 step 5), not merely present. The loom suite runs
  GREEN together with the full pre-existing test suite at every subsequent phase gate that touches the
  covered surface (Phase 4 step 8, Phase 5 step 8, Phase 9 step 4) — never proven piecemeal.
- **P2-7 is fixed via register-before-poll** (Phase 6) — the `ynz_handle_recv_poll` panic-then-pending
  hang is closed by registering the waker BEFORE the poll body that could panic, mirroring M6 Phase 4's
  channel-side register-before-poll fix exactly (Phase 6 step 2); a RED repro (Phase 6 step 3) confirms
  the task wakes rather than hangs after the fix; the "no lock held across a blocking poll" invariant is
  RE-VERIFIED against the changed code (Phase 6 step 4), not merely carried forward from M6.

### Performance

**Auto-promotion analysis (mandatory per [`auto-promotion.md`](../../../rules/auto-promotion.md)):**
Auto-Arc IS a genuine instance of this rule — the compiler proving a shared, read-only value's actual
usage fits a stricter/faster form (one refcount-shared allocation instead of a per-task deep copy) and
picking that form automatically. Analyzed against the rule's own checklist:

- **Is there a stricter/faster form?** Yes — Arc-sharing one allocation across ≥2 readers (Phase 2's
  decided beneficial-emission condition) instead of an independent `.copy` per spawned task.
- **Can the compiler prove the stricter form fits in some cases?** Yes — reusing
  `effective_ownership::EffectiveOwnership::Reads` (Phase 2 step 1) as the read-only proof; never a
  second, re-derived classifier (authoritative-derivation.md).
- **Codegen auto-promotion: YES.** Phase 5 emits `ynz_arc_new`/`clone`/`free` at Arc-eligible spawn
  boundaries for the beneficial case; the existing `.copy` path is confirmed UNCHANGED and stays
  correct-and-cheaper for the single-reader case (Phase 5 step 3).
- **Muted hint: YES.** The `auto_arc` registry domain (already registered, per ¶1 Terrain P2-6) fires in
  normal muted styling at Phase 5 step 4. Per [`inference.md`](../../../rules/inference.md)'s
  placement-category test, this is the **Informational** category — no body-level Yinz syntax exists to
  make the Arc-decision itself explicit (only the OVERRIDE direction is typeable, below); click jumps to
  the spawn call site/signature rather than inserting new source. The cautionary red-tint styling
  (`auto-arc-cautionary-tint`) stays separately deferred, unchanged — this milestone wires the hint to
  fire in NORMAL muted style only (Phase 2 step 5, Phase 5 step 4).
- **Tier 3 lint suggestion: NO — considered and declined.** Per auto-promotion.md's own criterion, a
  lint suggestion applies "when explicit form would benefit code review" — but there is no typeable
  explicit form of "make this Arc'd" to suggest rewriting TOWARD (contrast `array→fixed`/`let→const`,
  which have a losing alternative SOURCE form the lint recommends adopting). The only user-facing lever
  is the OVERRIDE (`.give`/`.copy` at the spawn site, avoiding Arc entirely) — a different mechanism
  from "rewrite to the stricter form" — so no lint rule name is minted for this feature.
- **Hover WHAT/WHAT-INSTEAD/WHY:** drafted at Phase 2 step 5 (updating the `auto_arc` domain's existing
  placeholder hover text to match Phase 2's decided topology), confirmed live at Phase 5 step 4. WHAT
  (the value is Arc-shared because ≥2 readers share it post-spawn); WHAT-INSTEAD (write `.give`/`.copy`
  at the spawn site to force an independent copy instead); WHY tied to the ACTUAL call site's reader
  count, per Golden Rule 11's "specific and contextual" requirement — never a generic "avoids
  allocation."
- **Override directions (per auto-promotion.md "Override Patterns — Consider Both Directions"):**
  **force-the-other-pick** has a real use case (a caller wanting an independent copy despite ≥2 readers,
  e.g. to avoid the Arc header/atomic-op cost on a cold path) and is handled by EXISTING, ALREADY-
  TYPEABLE syntax — `.give` or `.copy` at the spawn site — per auto-promotion.md's own canonical
  "Auto-Arc cross-thread" example; no new API needed, documented at Phase 2 step 4.
  **Force-the-auto-pick** (force Arc when the compiler would NOT have picked it) is a deliberate
  no-override case: manufacturing Arc-sharing for a single-reader value has a real cost (header + atomic
  ops) with zero benefit, so — mirroring auto-promotion.md's own reasoning for this exact shape — no
  override exists; recorded at Phase 2 step 4 as a deliberate omission, not an oversight.

**Channel-close (Phases 1 & 4): NO auto-promotion candidate — stated explicitly**, per the rule's own
instruction to record consideration rather than silence. Channel-close introduces a new EXPLICIT
operation (the close method itself); there is no per-usage "stricter form the compiler can prove fits in
some cases" the way `array→fixed` or `let→const` are per-binding proofs with a losing alternative form —
every channel gets identical close/idempotency/error-on-drained-receive behavior regardless of usage
pattern. Considered and declined.

- Loom (Phase 3) and structured fuzzing (Phase 8) are dev/CI-time only — zero cost to compiled Yinz
  binaries; no new pass is added to the compiler's hot compile-time path (restated here from ¶1
  Cross-Cutting Factor Sweep for invariant-section completeness).
- Channel-close (Phase 4) adds O(1) state per channel (a closed-flag/generation marker) — not a new
  pass, no asymptotic change to send/receive.

### Teaching

- The newly-live typed channel-closed error (the previously-dead Lock-8 path) follows WHAT/WHAT-INSTEAD/
  WHY per Golden Rule 11 — drafted at Phase 1 step 6, confirmed live at Phase 4 step 3.
- Any new compile-time diagnostic the close mechanism introduces (e.g. a compile-time-checkable
  `.send()`-after-close case, if Phase 1 decides that's checkable rather than a runtime error) follows
  the same three-part format — decided and drafted at Phase 1 step 6.
- The `auto_arc` muted-hint hover text (see Performance above) is drafted at Phase 2 step 5 and must
  itself follow WHAT/WHAT-INSTEAD/WHY, tied to the actual call site's reader count — never a generic
  "avoids allocation" explanation, per Golden Rule 11's contextual-specificity requirement.
- No new banned-jargon words are anticipated from this milestone's work — channel-close naming is
  evaluated against vocabulary.md + Golden Rule 12 at Phase 1 step 3 specifically to avoid introducing
  one; audited by the existing `tests/jargon_audit.rs` at every phase gate.
- If Phase 7 (Track 3 Branch A) ships, its language-half change (auto-`ynz_handle_free` at scope exit)
  introduces NO new diagnostic — cancellation surfaces via the ALREADY-shipped `errors`-propagation path
  (`IMP-no-function-coloring.md`'s Task Cancellation section), not a new error class.

### Runtime Dependencies

- **Loom (Phase 3): a DEV-dependency only, cfg-gated — stated explicitly, ZERO production dependency.**
  Added behind `#[cfg(loom)]` / a `loom` cargo feature on `ynz-runtime`; the `#[cfg(not(loom))]`
  production path resolves to the exact existing `std`/Tokio-primitive types (R3's mitigation, Phase 3
  step 3's production-path no-op confirmation) — compiled Yinz binaries never link loom.
- **Structured fuzzing (Phase 8): CI/dev-only.** The generator, the oracle wiring, and the CI job all run
  at CI time against compiled test binaries; zero runtime dependency added to compiled Yinz programs.
- **Auto-Arc emission (Phase 5) depends on the EXISTING malloc-backed `ynz_alloc` substrate** via the
  already-shipped `ynz_arc_new`/`clone`/`free` (concurrency-hammer-tested and confirmed correct by direct
  read, ¶1 Terrain P2-6) — no new dependency; codegen adds only NEW CALL SITES to an already-existing,
  already-tested runtime function set.
- **Channel-close (Phase 4) adds new runtime STATE** (a closed-flag/generation marker per channel) but
  **no new external dependency** — the same allocator/Tokio substrate already in use for channels since
  M4.
- P2-7's fix (Phase 6) and Track 3 (Phase 7, if Branch A) reuse EXISTING runtime primitives
  (`ynz_handle_free`, the existing waker-registration machinery) — no new dependency either way.

### Kernel-Mode Behavior

- **This milestone introduces ZERO new kernel-mode consideration — stated explicitly, because none of
  its surface reaches kernel mode.** `--kernel` mode already rejects `wait`/`background`/`channel<T>`
  entirely at compile time (confirmed live this session, `crates/ynz-typeck/src/check.rs`'s
  kernel-mode-rejection arms — the `channel<T>` construction gate at `check.rs:3392-3398`, the
  `.{method}()` channel-operation gate at `check.rs:3047-3059`, and the `wait`/`background` keyword gates
  — matching M6's own Kernel-Mode section's identical citation and confirmation). Every fix and feature
  this plan ships — channel-close (Phases 1 & 4), Auto-Arc (Phases 2 & 5), P2-7 (Phase 6), Track 3
  scope-drop cancellation (Phase 7) — lives entirely behind the Tokio runtime path, which never runs in
  kernel mode. None of these can be reached from a `--kernel` build; **no new kernel-mode compile-error
  surface is needed.**
- Loom (Phase 3) and structured fuzzing (Phase 8) are dev/CI-only surfaces with no runtime-mode dimension
  at all — kernel mode is a `ynz build --kernel` property of COMPILED Yinz programs, not of the dev
  toolchain that verifies the compiler itself.

### Demo & Error Gallery

- `examples/pirates-roster/entrypoint.ynz` gains a channel-close section (Phase 9 step 1) demonstrating
  the decided mechanism in a realistic Pirate/Ship-domain producer/consumer context — real operations
  only, per [`.claude/rules/dot-postfix.md`](../../../rules/dot-postfix.md)'s
  examples-must-use-real-operations rule; the byte-exact golden (`expected_stdout.txt`) is regenerated
  and committed. If Phase 5's Auto-Arc emission has a demonstrable source-level surface, it is added too;
  otherwise Phase 9 step 1 states explicitly why not (informational-only, no typeable form — matching the
  Performance subsection's analysis above).
- `examples/primantis-orders/m8_errors.ynz` is created (Phase 9 step 2) with WHY-commented intentional
  triggers for every new compile-time diagnostic this milestone adds — Phase 1/4's channel-close
  diagnostics at minimum, plus any diagnostic Phase 7's Branch A might add if it ships — wired into
  `crates/ynz-driver/tests/error_galleries.rs`'s diagnostic-count + key-phrase convention.
- Verification is byte-exact for the demo (not `insta`), matching the established M3-series/
  plan-invariants convention.

### Feature Registry Entries

- **Channel-close's new method (kind TBD, likely `[[primitive_intrinsic]]`):** Phase 1 decides the
  method name (candidates: `.close()`, `.done()`, `.finish()`) against vocabulary.md + Golden Rule 12;
  Phase 4 step 7 adds the corresponding registry entry once the kind and name are locked. Enumerated
  here as a KIND, not yet a name, per the plan-invariants rule's own allowance for "name TBD at design
  sign-off."
- **`auto-arc-codegen-emission`** (existing entry): retired if Phase 5's shipped emission covers the FULL
  beneficial-emission condition Phase 2 decides, OR narrowed to name the real remaining residual if
  Phase 2's topology leaves a bounded slice unimplemented (Phase 5 step 7) — mirroring the
  `ec-wrapper-collect-on-completion` retirement-note convention.
- **`auto-arc-cautionary-tint`** (existing entry): stays unchanged, still deferred — no per-hint tint
  rendering path exists in `ynz-lsp` today (confirmed, ¶1 Terrain); this milestone does not touch it.
- **`background-handle-cancel-injection`** (existing entry): retired (Phase 7 Branch A) if the
  language-half cancellation ships for real, OR its `ships_in`/`triggers` fields are rewritten with a
  concrete, milestone-specific finding (Phase 7 Branch B) — never left with the prior vague wording.
- **Explicitly none** for the rest: no new keywords, banned_declaration_keywords, banned_jargon words,
  or type_attached_constants are anticipated from this milestone's work — stated explicitly so reviewers
  know it was considered, not forgotten. (A diagnostic_template entry for the newly-live channel-closed
  error is possible but not certain — Phase 1 step 6/Phase 4 step 3 decide whether the message is
  canonical/reusable enough to warrant one, or stays a per-site dynamic message; recorded as a live,
  undecided item rather than silently assumed either way.)

## 4. Sustainment

- **Docker (universal project convention):** `docker compose run --rm dev cargo build --workspace`,
  `docker compose run --rm dev cargo test --workspace`, `docker compose run --rm dev cargo clippy
  --workspace -- -D warnings`, `docker compose run --rm dev cargo fmt --all`. No `-it`; every dispatch
  non-interactive.
- **Loom:** added as a dev-dependency behind `#[cfg(loom)]` / a `loom` cargo feature on `ynz-runtime`
  (Phase 3); loom test runs are their own `cargo test --features loom` invocation inside the same
  Docker `dev` service, never a separate toolchain.
- **Reference artifacts:** the concurrency-release audit
  (`.claude/audits/2026-07-04-concurrency-release-audit.md`) is the primary evidence base, same as M6/M7.
  `registry/features.toml`'s `auto-arc-codegen-emission` / `auto-arc-cautionary-tint` /
  `background-handle-cancel-injection` entries are read live at Phase 0 and updated at Phases 5/7/9.
- **CI:** [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml), Linux-only. Phase 8 adds
  a new, bounded, non-blocking-on-first-landing fuzzing job.
- **Sibling plans:** v0.3-M6 (`2026-07-04-v0-3-m6-concurrency-hotfix`, status `stub`) and v0.3-M7
  (`2026-07-04-v0-3-m7-optimizer-pipeline`, status `paused`) — this plan branches from `main` only after
  BOTH merge (Phase 0 CCIR item 1).

## 5. Command & Signal

- **Ownership:** each phase is picked up by whichever executor session the execute-plan conductor
  dispatches next; no named individual owner beyond Patrick's overall sign-off/release authority (his
  explicit sign-off gates Phases 1, 2, and — conditionally — Phase 7 Branch B).
- **Succession:** standard plan-format succession — this `plan-id` + the session-id chain + checkbox
  state in this file. Phases 1, 2, 4, 5, 7, and 8 (checkpoint-marked) use `handoff-phase-<N>.md` per the
  [Handoff file convention](../../../../../.claude/docs/reference/REF-plan-format.md#handoff-file-convention)
  when a segment checkpoints.
- **Audit trail:** `audit.md`, sibling to this `plan.md` in whichever status-folder currently holds it
  (created at the first amendment pass; the status↔folder invariant moves the whole directory when
  status flips) — session log + FRAGO log, append-only. The roadmap's own `audit.md` receives the
  Phase 9 ledger-reconciliation entry as a separate append, not a duplicate of this plan's own record.

## Future Requirements / Revisit

1. **Selective hot-field-only element materialization** (roadmap Capability Ledger row, ~line 390/442) —
   **WHAT:** SoA codegen computes `hot_fields` but never consumes it selectively. **WHY not absorbed
   here:** orthogonal SoA-gather perf gap, unrelated to concurrency correctness/completion — folding it
   in would mix two unrelated fix classes. **COST/TRIGGER:** unchanged from the roadmap's own text
   (~1 dedicated session; before or alongside any future optimization-pipeline milestone). Recorded here
   independently of M7's own non-absorption, for this milestone's own charter-boundary reason.
2. **`background.cpuBound` explicit override syntax** (concurrency-release-audit P4-2, MEDIUM) —
   **WHAT/WHY:** the auto-promotion force-the-other-pick direction for CPU-bound task routing, specified
   in `IMP-no-function-coloring.md:247` but never implemented, no registry entry. **WHY not absorbed
   here:** not named in this milestone's four tracks; a different capability (task-routing override, not
   channel-close/Arc/cancellation/verification). **COST/TRIGGER:** unchanged from M6/M7's own text —
   small (spawn-site annotation + registry entry); the next milestone touching `background`/task-routing
   surface.
3. **(Contingent) Track 3 re-deferral, if Phase 7 takes Branch B** — the concrete four-field deferral
   text lands here at Phase 7 execution time, replacing this placeholder, with the updated
   `background-handle-cancel-injection` registry entry's `ships_in`/`triggers` fields cited as evidence.
4. **Loom's Tokio-internals boundary** — **WHAT:** loom model-checks only the synchronization logic
   ynz-runtime owns directly (`pending_sends`, the drop ladder, recv-poll ordering); it does NOT and
   cannot model-check Tokio's own internal `mpsc`/scheduler implementation. **WHY not closed:**
   structurally out of reach — loom cannot instrument code it doesn't control the compilation of.
   **COST:** unbounded/not applicable — this is a permanent scoping boundary, not a deferred task.
   **TRIGGER:** none; this is a named, permanent limitation, recorded so no future reader mistakes "loom-
   verified" for "every layer, including Tokio's own internals, model-checked."
5. **(Contingent) Any Auto-Arc topology residual Phase 5 step 7 names** — if Phase 2's decided topology
   covers a bounded slice (e.g. single-shared-value spawns but not full N-way fan-out), the concrete
   four-field deferral for the residual lands here at Phase 5 execution time, replacing this
   placeholder, with the narrowed registry entry cited as evidence.
6. **Fuzzing corpus backlog** — **WHAT:** interesting failing/regression cases the structured fuzzer
   surfaces during and after this milestone need a durable home (a saved corpus for replay, not
   discarded after each CI run). **WHY not fully specified here:** the exact backlog mechanism is
   Phase 8's own design-note deliverable (step 6), not pre-decided at plan-authoring time. **COST:**
   small (a `fixtures/fuzz-corpus/` directory + a replay test harness, per Phase 8's design note).
   **TRIGGER:** Phase 8's own execution; this entry tracks that the design note gets written, not a
   separate task.
7. **Patrick-directed addition 2026-07-16 (M6 completion triage)** — two items assigned to this
   milestone by Patrick's own triage of the M6 completion review:
   (1) **fr12 — `channel<number>` decimal128 send/recv marshalling design** (roadmap Capability
   Ledger row, Idempotency-Key
   `2026-07-04-v0-3-m6-concurrency-hotfix#8-fr12: conduit-send-decimal128-marshalling`) is assigned
   to THIS milestone so it rides the same design pass as channel close-semantics — one design head
   for both. Not a bug today (compile-gated, unreachable); the assignment is a sequencing decision,
   not a correctness fix.
   (2) **fr13-fr17 — the never-drop-locals leak class** (per-iteration maybe/union heap-cell loop
   leak + the shutdown-dropped trampoline staged decimal128 arg-cell leak; Idempotency-Key
   `2026-07-04-v0-3-m6-concurrency-hotfix#8-fr13-fr17: never-drop-locals-heap-cell-and-trampoline-leaks`)
   is a named DESIGN INPUT to this plan's scope-drop cancellation model. Its root cause is the
   compiler-wide missing scope-exit drop-insertion pass
   (`docs/internal/scratchpad/SCRATCH-audit-2026-07-11-memory-safety.md`, finding M1 CRITICAL: no
   drop-insertion pass exists at all — these two leaks are symptoms of that gap, not independent
   bugs). The scope-drop design must not be finalized blind to this class. Whether the drop-story
   work itself lands in M8 or its own milestone is a plan-review question to resolve at M8's
   plan-review gate — this entry does not pre-decide that; it only ensures the design account for
   the class.
