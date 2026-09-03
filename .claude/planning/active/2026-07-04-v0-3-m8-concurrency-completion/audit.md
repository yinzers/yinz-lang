---
name: "v0-3-m8-concurrency-completion-audit"
plan-id: "2026-07-04-v0-3-m8-concurrency-completion"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-04-v0-3-m8-concurrency-completion

Append-only. *How the plan got here.* Read by the AAR, auditors, and the execution conductor's
Step-3a / Step-0 reconcile; never by executors (they read the current-truth plan.md slice).

## Session log

- `plan-producer-2026-07-04-m8-concurrency-completion` — 2026-07-04 — Authored the complete OPORD from
  the assembled brief. Read the concurrency-release audit
  (`.claude/audits/2026-07-04-concurrency-release-audit.md`), the frozen plan-format/risk-engine/
  decision-philosophy references, `IMP-no-function-coloring.md`, `IMP-concurrency.md`, `IMP-ownership.md`
  (confirmed live: it genuinely contains zero cross-thread Arc-sharing-topology text), the relevant
  `registry/features.toml` entries (`auto-arc-codegen-emission`, `auto-arc-cautionary-tint`,
  `background-handle-cancel-injection`), and both sibling plans (`2026-07-04-v0-3-m6-concurrency-hotfix`,
  `2026-07-04-v0-3-m7-optimizer-pipeline`) to confirm zero scope overlap. Scored the risk table against
  the default code-domain anchor sheet (no project override — glob-confirmed). Set `status: "paused"`
  per the conductor-pre-approval convention M7 established, gated on a double merge-and-tag precondition
  (M6 AND M7 must both merge before Phase 0 begins) plus the orchestrator's Gate 4 read-through. No HIGH
  residual anywhere in the risk table — every hazard reuses an already-authoritative source
  (`effective_ownership` for Arc, M6's drop-glue choke point for channel-close) rather than inventing a
  new frame-layout-affecting transform, keeping this milestone's hazard surface narrower than M7's own
  R8. Ten phases (0–9): gate, two parallel-safe design phases (channel-close, Arc topology) each with a
  Patrick sign-off gate, a loom-substrate phase sequenced before both implementation phases per the
  brief's explicit instruction, the two implementation phases, a small P2-7 mechanical fix, a
  design-plus-contingent-implementation phase for scope-drop cancellation, a structured-fuzzing phase,
  and close-out (demo/gallery/registry/roadmap reconciliation/full-suite gate).

- `plan-producer-2026-07-04-m8-amend1` — 2026-07-04 — Amendment pass resolving a plan-review's full
  finding set (2 BLOCKERs, 3 SHOULD-FIXes) before this plan's Gate 4 read-through. Re-read
  `REF-plan-format.md`, `REF-risk-engine.md`, `.claude/rules/plan-invariants.md`, and both sibling M6/M7
  plans' Invariants sections (M7 lines ~734-855) live before amending, per this producer's own
  read-at-start discipline.
  - **BLOCKER 1 (missing Invariants section):** authored the full `## Invariants This Milestone Must
    Preserve` section (all 7 subsections — Safety, Performance, Teaching, Runtime Dependencies,
    Kernel-Mode Behavior, Demo & Error Gallery, Feature Registry Entries), inserted between `### 3.4
    Coordinating Instructions` and `## 4. Sustainment`, matching M6/M7 sibling shape. Notably: ran the
    full auto-promotion.md checklist against Auto-Arc (a genuine instance — codegen yes, muted hint yes
    via the already-registered `auto_arc` domain firing at Phase 5 step 4, Tier 3 lint NO with reasoning,
    override directions both analyzed — force-the-other-pick already covered by existing `.give`/`.copy`
    syntax per auto-promotion.md's own canonical example, force-the-auto-pick a deliberate no-override);
    confirmed channel-close has NO auto-promotion candidate (stated explicitly, not silently skipped);
    confirmed via direct `check.rs` grep that `--kernel` already rejects `wait`/`background`/`channel<T>`
    entirely (lines 3392-3398, 3047-3059), so this milestone's entire surface is unreachable from kernel
    mode — zero new kernel-mode consideration, stated explicitly rather than left silent.
  - **BLOCKER 2 (R2 mis-scored):** re-scored R2 honestly — initial Prob was C, should be **B** (Likely):
    this is net-new codegen in the four-milestone silent-miscompile hazard family (M3a/M3d/M3e/M3g), and
    reusing `effective_ownership::EffectiveOwnership::Reads` closes only the MISCLASSIFICATION mode, not
    the SEPARATE frame/spawn-boundary-layout interaction hazard Phase 5's own spike step (step 2)
    explicitly concedes as an open question. B×II = H initial; the B2 adversarial/RED-repro + spike-gate
    mitigation (prob −1) shifts B→C; re-lookup(C, II) = H, UNCHANGED (Critical severity does not clear
    High until probability reaches D — same rule M7's own R8 override already established). Did NOT
    stretch a second catalog mitigation dishonestly to force a MEDIUM landing (M7's R8 precedent explicitly
    rejects that move). Drafted a full unsigned RISK OVERRIDE block for R2 mirroring M7 R8's shape
    (risk/why-not-mitigable/blank Accepted-by+Date/trigger-to-revisit citing Phase 5 Step 2's own spike
    verdict as the evidence path toward a future re-score) — signature line intentionally blank; this
    producer never self-signs a HIGH residual. Updated the risk-table row, the "No HIGH residual" intro
    prose (now "One HIGH residual — R2"), the Floor-check paragraph, Phase 5's task+purpose/exit-criteria/
    reviewer-fan-out text, and CCIR item 6 all in the same pass so the plan is internally consistent (R1's
    scoring was reviewer-confirmed correct and left untouched — a runtime-state change, not a frame-layout
    one).
  - **SHOULD-FIX 1:** swept the whole plan for "Task Cancellation" / "IMP-concurrency" and fixed all six
    misattributions (¶1 Terrain heading, Design-Doc Alignment citation-depth-verification bullet, Design-
    Doc Alignment divergence #3, Phase 7 step 2, Phase 7 step 3 Branch A, Phase 7 reviewer-fan-out) — the
    Task Cancellation section genuinely lives at `IMP-no-function-coloring.md:281-298` (confirmed by direct
    read this session), never `IMP-concurrency.md`. Verified the Design-Doc Alignment "Cited governing
    docs" line (line ~249) was ALREADY correct (Task Cancellation already listed under
    `IMP-no-function-coloring.md` there) — left untouched.
  - **SHOULD-FIX 2:** fixed the ¶1 Friendly-forces roadmap link — was `../2026-05-21-v0-3-concurrency-
    perf/roadmap.md` (wrong depth from this plan's `paused/` location), now
    `../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md`, matching M7's own sibling-plan link
    pattern and confirmed against the roadmap's actual on-disk location
    (`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md`, glob-verified).
  - **SHOULD-FIX 3:** Phase 9 step 3 now states explicitly that the roadmap's existing combined
    "Concurrency completion... status: being authored" placeholder row (present in BOTH duplicate
    Capability Ledger tables, roadmap.md lines ~445 and ~499) is REPLACED BY the four granular rows this
    plan adds, in both tables, in the same lockstep edit — not left standing as a stale fifth row.
  - **Confirmed out of scope, not touched:** the global `~/.claude` spec-link-unreachability systemic gap
    (all three M6/M7/M8 siblings share it, not this plan's defect to fix); the directory split (already
    fixed on disk — confirmed `audit.md` sits beside `plan.md` in `paused/2026-07-04-v0-3-m8-concurrency-
    completion/` this session, no action needed).
  - Appended this session's id to the `plan.md` frontmatter `session-id` array in the same action as this
    entry (never minted separately).

- `gate4-signatures-2026-07-04` — 2026-07-04 — Signature event: Patrick signed R2's RISK OVERRIDE
  (Auto-Arc codegen-emission refcount/frame-layout hazard, ¶1 Risk Assessment) as part of Gate-4
  approval covering all three sibling concurrency plans (M6/M7/M8). Filled `Accepted by: Patrick
  (Gate-4 approval, conducted 2026-07-04)` and `Date: 2026-07-04` on the previously-blank signature
  lines; updated R2's Gate cell from `BLOCKED — unsigned RISK OVERRIDE below` to `H — override SIGNED
  (see block below)`; reconciled every other plan-text mention asserting R2's override was still
  unsigned (the pre-table intro sentence, the post-table paragraph preceding the override block,
  Phase 5's task+purpose sentence, and CCIR item 6) so the plan is internally consistent. Appended
  `session-id: "gate4-signatures-2026-07-04"` to the frontmatter chain (append-only — both prior
  session-ids preserved).

- `m8-p1-20260903-a1` — 2026-09-03 — **Phase 1 executed (design only, no compiler code): steps 1–6
  done, step 7 (Patrick sign-off) OPEN.** Read `channel.rs` end-to-end (endpoint-holding
  architecture, `pending_sends` keying, purge, `Drop`), `handle.rs`'s `outbox_tx: Option<Sender>` +
  `.take()` close precedent, the `emit.rs` conduit closed arms (`~12749-12961`, `closed_msg` text and
  the aborting `ChanRecv` arm), typeck's `check_conduit_method_call` + `CHANNEL_SUSPENDING_METHODS`,
  `IMP-concurrency.md`, `IMP-no-function-coloring.md` (silent on close — no contradiction with the
  plan), `REF-concurrency.md`/`REF-errors.md`/`REF-maybe.md`/`REF-control-flow.md` (no `break`
  keyword exists — the taught consumer loop is flag-driven), vocabulary/dot-postfix/teaching-surfaces
  rules, Golden Rule 12, and the registry schemas. Decisions: explicit `.close()` (chosen over
  `.done()`/`.finish()`/`.end()` — matches the word every existing error string and runtime constant
  already uses for the state); auto-close-on-last-producer DEFERRED with four fields (needs role
  analysis + the missing scope-exit drop pass + a producer/holder refcount split — a redesign, not an
  extension; CCIR-2 discovery routed as a deferral, not absorbed); bare `receive()` → `T errors`
  (one `.receive()` convention with the handle form; `maybe<T>` weighed/rejected; ~12 fixture sites +
  demo + gallery + spec change in Phase 4); send-after-close = runtime typed error, no compile
  diagnostic; `close()` wakes all recv-waiters (settles the co-waiter facet `channel.rs` left to M8);
  idempotent double-close; in-flight pre-close sends complete; P2-3 fix routed through the registered
  drop glue. Doc: new `IMP-concurrency.md` section "Channel Close — End-of-Stream Semantics"
  (promoted out of the Divergences format — six load-bearing parts); the M6 Divergence entry rewritten
  to a pointer that retires at Phase 4; one cross-ref line added to `IMP-no-function-coloring.md`.
  Teaching text (send-after-close, receive-after-drain, the two extended compile diagnostics) drafted
  in the section. Registry kinds recorded for Phase 4 (`[[primitive_intrinsic]]` incl. registering the
  un-registered `send`/`receive`; `[[deferred_language_feature]]`; no `[[diagnostic_template]]`).
  **fr12 surfaced as a scope question**: separable from close by construction; its marshalling design
  is not written by this phase. No tests run (no code touched). No handoff file (phase ran to its
  sign-off boundary in one segment).

- `m8-p1-fix1-20260903` — 2026-09-03 — **Phase 1 fix round (design only, no compiler code): two
  reviewer BLOCKERs ruled on by Patrick applied, six should-fix items addressed; step 7 (sign-off)
  still OPEN.** BLOCKER 1: the first draft's P2-3 closed-arm free was a use-after-free — typeck's
  `send` arm never consumes its argument (`check.rs:4105–4149`; the only `scope.consume` sites are
  `:1511` and `:4618`), codegen lowers the payload by bare `to_i64_bits` (`emit.rs:12641–12649`).
  Ruling applied: `send()` gives its payload for owned-heap element types (`array`/`map` — the exact
  set `channel_drop_glue` registers glue for, `emit.rs:15511–15515`; primitives and `string`
  unchanged), mirroring the spawn-arg give path; new compile diagnostic `ConsumedBySend` drafted in
  full three-part form; emitted from the ONE existing consumed-read site by cause. **Probe on the
  current tree confirmed the hole is live today**: a `channel<array<int>>` program sending `rows`
  then printing `rows.count()` compiles and runs. Found and recorded: no `channel<array<T>>`/`map`
  E2E fixture exists anywhere (only `channel_construct.ynz:14`, `channel<string>`, construction
  only). BLOCKER 2: `receive()` retyped to `maybe<T>` (vocabulary.md: `maybe` is for normal absence,
  `errors` for failure; auto-propagation at `check.rs:3647–3653` would have made end-of-stream the
  task's failure in both shipped channel-consuming task fns; `ynz_error_new` per normal loop exit at
  `emit.rs:12802–12813`); handle's `receive()` stays `T errors` with the reason written so nobody
  "unifies" them; `tallyScores` rewritten to `.exists()`/`.value`. Should-fix: (1) lock-ordering
  nuance — the sender-lock clone is the linearization point, a send holding a clone is a pre-close
  send (`channel.rs:445–446`); (2) `h.close()` argument replaced (message-to-child vs lifecycle act
  on the channel), and the non-ident-first-channel-arg question answered by code read
  (`check.rs:2321–2345` idents only; `bg_arg_is_provably_safe` admits a call as `Give`;
  `prepare_bg_arg_for_ctx` shares by type) AND by probe (`background doubler(makeWire())` +
  `h.send(21)` prints 42) — a real gap, recorded as the `background-handle-close` four-field
  deferral; (3) blast radius corrected to 19 sites / 13 fixture files, enumerated, plus
  `REF-concurrency.md:252` which the review's own list missed; (4) loom named as not-yet-a-dependency
  (no `loom` in any `Cargo.toml`), Phase 3's swap gates the model-check; (5) three typeck sites
  named for `close` (`known`, the unconditional receiver/derivable guards `:4011–4082`, the shared
  unknown-method string `:4003`); (6) the "extend THIS site" quote re-attributed to
  `check.rs:3988–3992`. fr12 left OPEN with one line marking it pending Patrick. Registry list
  updated (`param_ownership` schema field; 2 deferrals; 1 template; plus a pre-existing
  `Consumed`-template-vs-code wording drift found and assigned to Phase 4). Plan: Phase 1 status
  block rewritten with the seven Phase 4 obligations; Safety/Teaching/Feature-Registry invariants
  and Phase 9 step 2 updated. No tests run (no code touched; the three probe programs were
  throwaway — created, run in the dev container, deleted). No handoff file.

## FRAGO log

(the note below this pair predates execution; FRAGO 001/002 are the first real mid-execution
delta-records against this running plan)

### FRAGO 001 — 2026-09-03 — Phase 6 RETIRED as already-satisfied by M6

- **Trigger:** Phase 0's terrain re-verification found this plan's P2-7 premise stale.
- **Finding:** M6 did not defer P2-7. It un-deferred it under its own FRAGO 010 and shipped the fix
  as M6 Phase 4b, commit `b0cdbd3`, inside PR #82.
- **Confirmation (not self-graded):** the finding came from an executor and was NOT applied on that
  basis. A separate `code-reviewer-medium` confirmed it adversarially: `record_recv_waiter(cx.waker())`
  is the first statement inside the `catch_unwind` closure (`handle.rs:354`), before `poll_recv`
  (`:355`), closing the exact panic-before-registration window the audit reported; two tests lock it
  (`handle.rs:724`, `:798`) and were **proven revert-sensitive** — the reviewer swapped the ordering
  back to poll-first, both failed on the P2-7 hang assertion (`wakes == 0`), tree restored clean.
  M6's no-lock-across-blocking-poll invariant holds; handle-side and channel-side are a genuine
  structural mirror.
- **Authority:** Patrick, 2026-09-03 — "verify first, then retire." Conditional authority discharged
  by the confirmation above.
- **Applied:** Phase 6 block marked RETIRED with its original text preserved in a `<details>` fold;
  risk row R6 retired; ¶1 Terrain P2-7 bullet and Design-Doc Alignment §4 boundary claim annotated as
  superseded; Invariants → Safety P2-7 assertion annotated satisfied-by-M6. Phases NOT renumbered —
  nine phases (0–5, 7–9), every existing citation and future `Plan-Phase:` trailer stays valid.
- **Residuals carried forward:** two M6-inherited items (panic-payload log asymmetry; the duplicated
  `recv_waiters` trio) recorded as Future Requirements item 8 rather than absorbed.

### FRAGO 004 — 2026-09-03 — M8 PAUSED at Phase 1 step 7; a live use-after-free on `main` takes priority

- **Trigger:** Phase 1's round-2 `code-reviewer` seat, grading the repaired channel-close design,
  found the design's "the buffered value has exactly one owner at every moment" claim false — and in
  proving it, reproduced a **use-after-free that exists on `main` today** (v0.3.3, released).
- **The bug, root cause named:** typeck's consume is not the only ownership authority.
  `prepare_bg_arg_for_ctx` gives a spawned task a heap clone of any `array<int|float|bool|shape>`
  argument tagged `BgArgFreeKind::HeapArrayPrimitive` (`emit.rs:16888`, `:16918`), and the task's
  drop ladder frees it at retirement (`emit.rs:17028`, `:17516`). If the task sends that parameter
  into a channel, the SAME pointer is in the channel buffer AND still on the ladder's free list.
  Nothing connects `check_conduit_method_call`'s consume to codegen's ladder. **Reproduced on the
  current tree:** a `producer(wire: channel<array<int>>, rows: array<int>)` spawned as
  `background producer(wire, rows)` that sends `rows` printed `-4760032263271174595` for
  `got.count()` in the spawner.
- **Why this is one bug and not two.** The M8 Phase 1 design blocker and the shipped UAF share a
  single ancestor: codegen's drop ladder and typeck's ownership view are independent owners of the
  same pointer. Per `root-cause.md`'s cluster rule, they get ONE fix at the ancestor, not one patch
  each. Patrick's ruling ("ladder consults consumption") is therefore both the hotfix's mechanism and
  the substrate M8 Phase 4 builds on — Phase 4 inherits it rather than re-deriving it.
- **Patrick's rulings, 2026-09-03** (four, taken together at one gate):
  1. **Hotfix now, on its own branch, separate from M8.** The released compiler is mounted read-only
     by external consumer projects via `target/release`; a memory-safety bug there cannot wait for a
     ten-phase milestone.
  2. **Ladder consults consumption** — codegen skips the ladder free for a binding typeck already
     marked consumed by a send. Threads the one authoritative answer into the second consumer per
     `authoritative-derivation.md`, rather than teaching typeck about `BgArgFreeKind`.
  3. **FRAGO 003 ratified** (see below).
  4. **`.copy()` ships on `map<K,V>` before Phase 4**, so the use-after-send diagnostic's advice is
     executable for every type the consume rule covers.
- **M8 state at pause:** Phase 0 COMPLETE. Phase 1 steps 1–6 complete through two review rounds;
  **step 7 (Patrick's sign-off) is still OPEN and was never granted** — the four rulings above are
  decisions feeding the design, NOT the sign-off itself. Phase 4 remains hard-blocked. An outstanding
  Phase 1 fix round is owed before sign-off, carrying: this FRAGO's ownership resolution, the
  `ConsumedBySend` WHY rewrite (doc-auditor blocker — "is empty afterward" teaches a runtime-emptiness
  model when the real behavior is a compile error), the `map`-`.copy()` ruling, and the parked items
  in `.claude/plans/parked.md`.
- **Still unruled and owed to Patrick at sign-off:** fr12's disposition (ride this design pass, per
  his July triage, or become its own step).

### FRAGO 003 — 2026-09-03 — Downstream plan edits and the `param_ownership` schema field, RATIFIED

- **Trigger:** Phase 1's round-2 `plan-adherence` seat tagged `frago-needed`.
- **Finding:** the round-1 fix executor, from inside a DESIGN phase, edited downstream plan sections
  — Phase 4 steps 3/3b/4/5, Phase 9 step 2, and three Invariants subsections (Safety, Teaching,
  Feature Registry Entries) — and added a new optional schema field, `param_ownership`, to
  `[[primitive_intrinsic]]` (there is no ownership field today; `features.toml:581-587`). The
  Feature Registry Entries subsection carries Patrick's Gate-4 signature of 2026-07-04, so the plan
  as approved is not the plan as it now stands. A registry SCHEMA change also has cross-crate blast
  radius into `crates/ynz-registry/build.rs` codegen.
- **What was NOT wrong:** the reviewer independently verified every edit traces to one of Patrick's
  two rulings or a directly-consequent obligation the design doc itself justifies. **No smuggled
  decisions.** It also re-derived the 19-site blast radius from scratch and matched, correctly
  excluding every handle-side `.receive()`. The gap was process, not content.
- **The counter-argument, recorded because it is sound:** leaving Phase 4 step 3 unedited would have
  left it instructing an implementer to wire `receive()` to `T errors`, contradicting the `maybe<T>`
  ruling made in the same round. A self-contradictory plan is worse than the overreach.
- **Authority:** Patrick, 2026-09-03 — ratify the edits, keep `param_ownership`, log the formal
  FRAGO, and re-sign the changed Feature Registry Entries subsection.
- **Applied:** this record. `param_ownership` stands as a ratified schema addition; its build-time
  validation is parked (`.claude/plans/parked.md`, item 5) as Phase-4 implementation work — without
  it a typo or a length misalignment ships silently as hover text.

### FRAGO 002 — 2026-09-03 — Two dangling citations corrected in the cold-resume banner

- **Trigger:** Phase 0's drift check distinguished offset-only drift from citations that now point at
  unrelated code entirely.
- **Finding:** `runtime.rs:591-693` ("the drop ladder" — the choke point **Phases 4 and 7 both wire
  through**) is now `ynz_rt_shutdown`; the real drop ladder is `runtime.rs:981-1050`. The kernel-mode
  gates cited as `check.rs:3392-3398`/`3047-3059` are now `~4316-4322` and `~3972-3980`.
- **Applied:** a correction table in the plan's cold-resume banner, where a resuming reader hits it
  before navigating by any citation. Original prose left unedited — the banner is the correction of
  record. Offset-only drift listed alongside it so a reader can tell the two classes apart.
- **Why this was not left to the phases that use it:** Phase 4 and Phase 7 both navigate to the drop
  ladder by that citation. A wrong anchor there routes an executor into shutdown code while it
  believes it is reading cleanup dispatch — precisely the kind of silent wrong-turn this plan's own
  `authoritative-derivation.md` discipline exists to prevent.

## Context-segment log

(none yet — this plan has not begun execution)

- `conductor-2026-09-03-m7-merge-and-precondition-clear` — 2026-09-03 — **Preconditions cleared; plan
  is now genuinely startable at Phase 0.** No plan content changed beyond the status block and
  frontmatter; execution has still NOT begun.
  - **Cleared the double merge-and-tag precondition.** M7 was complete and sealed but its branch had
    never merged: PR #87 sat open with a red sanitizer lane. Root-caused that red to a pre-existing
    TSan flake in `panic_reraises_in_parent` (a 50ms sleep then a single poll — the poll returned
    `Poll::Pending` under instrumentation, so `resume_unwind` never fired). Confirmed pre-existing,
    not an M7 regression: the identical failure occurred on `main` at the released v0.3.2 sha on
    2026-07-16 and passed on that same sha on 07-24/07-29/07-31. Fixed by polling to readiness with
    a liveness deadline (`12f397b`); a duplicated `check_preempt` benchmark hiding behind it was
    collapsed (`67b3148`). PR #87 merged at `f7eb2fa`; v0.3.3 cut and tagged.
  - **Corrected the status/frontmatter contradiction.** `status: "active"` had been set while the
    status note still declared the plan "deliberately held at `paused`" pending those merges. A cold
    resume this session cost roughly a dozen tool calls to resolve which was true. The note is now a
    COLD-RESUME ENTRY POINT block stating the entry phase, a precondition table with evidence, and
    the tree changes Phase 0's re-read should expect.
  - **Tree changes this session that Phase 0's re-verification will encounter** (all on `main` at
    `cf17de3`, PR #88): `ynz_fmt::format()` was cubic — `comment_merge::line_of` rescanned the source
    from byte 0 per call, 91.18s on the 1,352-line `pirates-roster/entrypoint.ynz`, live in the
    just-released v0.3.3 — now `partition_point` over precomputed newline offsets (88.86s → 0.06s).
    The corpus sweep is parallel, and BOTH corpus sweeps now derive scheduler-dependence from the
    fixture SOURCE rather than its filename (the old name-substring proxy had drifted: 91 fixtures
    use `background`, only 16 were named for it, leaving 75 asserting a byte-identical ordering the
    language never promised). `/tmp` is tmpfs+exec in the dev container. Full workspace suite
    51.8 min → 4.5 min, same coverage.
  - **Relevant to this milestone specifically:** any new `background`/channel fixture M8 adds is now
    auto-classified by the corpus sweeps with no exclusion-list entry to remember; and
    `.claude/rules/test-parallelism.md` must be loaded before adding any fixture-looping test here.
  - Appended this session's id to the frontmatter `session-id` chain in the same action as this entry.

- `conductor-2026-09-03-m8-execution` — 2026-09-03 — **Phase 0 EXECUTED. Verdict: PROCEED.**
  Dispatch `executor-medium` (cell: coding/low/mechanical), dispatch-id `m8-p0-20260903-a1`.
  Read-only gate; zero code changes, so zero reviewer seats derived and green-check skipped — a
  round with no gradeable decision earns zero seats per `reviewer-seats.md`'s own carve-out, applied
  mechanically by the conductor. No round-seal commit either: nothing to seal.
  - **Pre-flight:** no `.claude/rules/branching.md` existed. Patrick's answer recorded and written to
    that file: `main` is protected, plan work lives on `feat/<slug>`, close-out is a PR via `/pr`,
    never a direct merge. Plan frontmatter now carries `branch:
    feat/v0-3-m8-concurrency-completion` so a cold resume can find the ref. A second gate (the
    worktree-ask gate) was answered "stay on this branch, in this checkout" and granted for the
    session.
  - **Double merge-and-tag gate: SATISFIED.** `main` at `cf17de3` contains PR #82 (M6, merge
    `10df6d7`) and PR #87 (M7, merge `f7eb2fa`, tagged v0.3.3 at `1aee207`).
  - **CRITICAL FINDING — Phase 6's premise is false, not drifted. P2-7 appears already fixed by M6
    itself.** M6 un-deferred P2-7 under its own FRAGO 010 and shipped it as M6 Phase 4b, commit
    `b0cdbd3` ("fix(runtime): close ynz_handle_recv_poll panic-then-Pending hang (M6 P4b)",
    2026-07-11), inside PR #82; the register-before-poll fix reads as live at `handle.rs:339-378`.
    If confirmed, this plan's ¶1 Terrain ("NOT fixed by M6"), its Design-Doc Alignment §4 boundary
    claim, risk row R6, and all of Phase 6 target already-completed work. **Not auto-applied:** the
    finding is a single unconfirmed executor claim, and retiring a whole phase is a mission-scope
    call. Patrick directed "verify first, then retire" — an independent `code-reviewer-medium`
    confirmation was dispatched before any amendment lands. This entry records the finding as
    PENDING CONFIRMATION, not as fact.
  - **Citation drift recorded — two are DANGLING, not merely offset (they point at unrelated code
    today):**
    - `runtime.rs:591-693` "the drop ladder" → those lines are now `ynz_rt_shutdown`. The real drop
      ladder Phases 4/7 must wire through (the kind-2 `BgArgDropEntry` arm calling
      `channel::purge_pending_sends` + `ynz_channel_free`) is at `runtime.rs:981-1050`.
    - `check.rs:3392-3398` / `3047-3059` (kernel-mode gates, cited in this plan's Kernel-Mode
      Behavior subsection) → construction gate is now `~4316-4322`, method-call gate `~3972-3980`.
      Substance holds; the cited lines are unrelated code.
  - **Citation drift, substance intact (offsets only):** `channel.rs:109-123` → `200-225`;
    `channel.rs:536-539`/`557-560` → `962`/`1123`; `channel.rs:120` (`pending_sends`) → `213`, shape
    now `Mutex<HashMap<(u64,u64), PendingSendEntry>>`; `emit.rs:~11833-11960` → `~12776-12961`, with
    the P2-3 leak genuinely still unfixed and the "Structurally unreachable in v0.3-M4" comment still
    present verbatim at `emit.rs:12852`; `IMP-no-function-coloring.md:281-294` → `295-311`.
    `registry/features.toml:1229-1235` and `IMP-no-function-coloring.md:58` are exact, zero drift.
  - **Registry entries confirmed present and matching this plan's characterization:**
    `auto-arc-codegen-emission` (`features.toml:1230`), `auto-arc-cautionary-tint` (`:1351`),
    `background-handle-cancel-injection` (`:1174`).
  - **Non-absorption re-affirmed in BOTH duplicate Capability Ledger tables** (roadmap lines 446/516
    and 452/522): "Selective hot-field-only element materialization" and `background.cpuBound`
    (P4-2) both still un-absorbed, triggers unchanged.
  - **Assumption A4 confirmed:** `crates/ynz-typeck/src/effective_ownership.rs` exists;
    `EffectiveOwnership::{Reads, Unknown, Writes}` is real. Semantic-reuse correctness deliberately
    left to Phase 2, per that phase's own step 1.
