---
name: "v0-3-m8-concurrency-completion"
plan-id: "2026-07-04-v0-3-m8-concurrency-completion"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-producer-2026-07-04-m8-concurrency-completion", "plan-producer-2026-07-04-m8-amend1", "gate4-signatures-2026-07-04", "executor-2026-07-16-patrick-triage-application", "conductor-2026-09-03-m7-merge-and-precondition-clear", "m8-p1-20260903-a1", "m8-p1-fix1-20260903", "m8-p1-fix2-20260903", "m8-p1-fix3-20260903", "conductor-2026-09-03-m8-execution", "conductor-2026-09-03-m8-phase2", "m8-p2-20260903-a1", "m8-p2-fix1-20260903", "m8-p2-signoff-20260903", "m8-p2-signoff-fix1-20260903", "m8-p3-20260903-a1", "m8-p3-fix1-20260904", "m8-p4-20260904-a1", "m8-p4-20260904-a2", "m8-p4-fix1-20260904", "m8-p4-fix2-20260904", "m8-p4-fix3-20260904", "m8-p5-20260904-a1", "m8-p5-fix1-20260904", "m8-p5-fix2-20260904", "m8-p7-20260904-a1", "m8-p7-fix1-20260904", "m8-p8-20260904-a1", "m8-p8-fix1-20260904", "m8-p8-fix2-20260904"]
created_at: "2026-07-04"
updated_at: "2026-09-04"
branch: "feat/v0-3-m8-concurrency-completion"
worktree: null
metadata:
  type: "plan"
---

# PLAN: v0.3-M8 — Concurrency Completion

> ## ⏭️ COLD-RESUME ENTRY POINT — Phase 0 ✅ done · Phase 1 ✅ signed off (narrowed) · **Phase 2 ✅ signed off 2026-09-03** (executors `m8-p2-20260903-a1`, `m8-p2-fix1-20260903`, `m8-p2-signoff-20260903`) · **Phase 3 ✅ complete 2026-09-03** (executor `m8-p3-20260903-a1` — loom substrate landed, spike GREEN, production no-op proven, six loom models with revert-proven teeth) · **Phase 4 ✅ CLOSED BY CEILING 2026-09-04** (executors `m8-p4-20260904-a1` RED seal at `6b8a34d`, `m8-p4-20260904-a2` implementation, fix rounds `m8-p4-fix1/fix2/fix3-20260904`; three grading rounds; two `errors`-surface blockers re-homed to a post-M8 hotfix branch per FRAGO 011 — parked 32/33/34 — none a channel-close defect) · **Phase 5 ✅ CLOSED 2026-09-04** (executor `m8-p5-20260904-a1`, fix rounds `m8-p5-fix1/fix2-20260904`; two grading rounds, terminal state CLEAN; spike GREEN, full beneficial condition shipped with the caller-side proof covering the member spawns themselves, one-spawn IR byte-identical, `auto_arc` hint firing on both spawn forms, R2 stays HIGH under its signed override) — **Phase 7 is next, AFTER the parked-40 hotfix merges back**: `fix/bg-arg-number-field` (FRAGO 012) runs in its own worktree `../ynz-lang-hotfix-bgarg`; sequence = hotfix PR → merge to `main` → merge `main` into this branch → author parked 43's fixture in that merge commit → dispatch Phase 7 (receipt already minted, `m8-p7-20260904-a1`)
>
> ### 🔀 RESTRUCTURED 2026-09-03 — FRAGO 008: Phase 1's ownership scope MOVED to Phase 2
>
> **Phase 1 is narrowed and ready for sign-off.** It keeps the close mechanism, the `.close()` name,
> bare `receive()` → `maybe<T>` (the handle's `T errors` stays deliberately distinct), the contract
> points, the `refuse_closed` collapse of the three CLOSED arms, the runtime free-site ruling
> (FRAGO 006), the `Option<ChannelElemDrop>` classification, `HandleChannelArgNeedsBinding`, and the
> four-field auto-close deferral.
>
> **Phase 2 absorbs** the entire *"who else holds this value"* question and its three diagnostics
> (`ConsumedBySend`, `ParamNeedsGive`, `SendPayloadNeedsCopy`) — because Phase 1 spent three review
> rounds re-deriving an ownership analysis by enumerating syntactic call-site shapes, while
> `crates/ynz-typeck/src/effective_ownership.rs`, a whole-program fixpoint that already answers it,
> sat unused one phase away. Producer named in `audit.md` FRAGO 008 and in `.claude/corpses.md`.
> Phase 2's own block carries the three failing programs any answer must defeat.
>
> **Phase 4 is blocked on BOTH Phase 1's and Phase 2's sign-offs.**
>
> **The three items owed at Phase 1's sign-off are ANSWERED (FRAGO 009, 2026-09-03):** fr12 rides
> Phase 2's design (Phase 4 implements) · `HandleChannelArgNeedsBinding` is a hard compile ERROR ·
> FRAGO 003's ratification is STANDING, traceability-gated (a fix round may edit downstream plan
> text when every edit traces to a ruling Patrick made that round; `plan-adherence` verifies the
> trace; an untraceable edit halts to him).
>
> **The UAF hotfix has LANDED** (PR #89, merged into this branch at `6143c1d`), so this branch no
> longer carries the three use-after-frees Phase 1's own review discovered.
>
> ---
>
> **Preconditions (all MET, verified 2026-09-03).**
>
> | Precondition | State |
> |---|---|
> | Gate 4 human read-through / risk-override signature | ✅ Signed 2026-07-04 (R2 override, see ¶1 Risk Assessment) |
> | Sibling **v0.3-M6** merged to `main` | ✅ PR #82, released **v0.3.2** |
> | Sibling **v0.3-M7** merged to `main` | ✅ PR #87, released **v0.3.3** (merged 2026-09-02, `f7eb2fa`) |
>
> This note previously said the plan was "deliberately held at `paused`" pending those merges. That
> is why `status` and this block contradicted each other for weeks and cost a later session a dozen
> tool calls of archaeology to resolve. Both merges have landed; `status: active` is now correct and
> this block is the record of why.
>
> **What Phase 0 must still actually do** (it is a re-verification gate, not a formality): confirm the
> merges on `main`, then re-read every file:line this plan cites against the POST-M6-POST-M7 tree and
> record drift. Both siblings touched `channel.rs` / `handle.rs` / `runtime.rs` / `emit.rs`, which is
> exactly the terrain this plan's ¶1 cites, so drift is expected rather than unlikely.
>
> **Tree changes since this plan was written that Phase 0's re-read should expect** (2026-09-02/03
> session, `main` at `cf17de3`):
> - `crates/ynz-fmt/src/comment_merge.rs` — `line_of` was O(filesize) per call, making `format()`
>   cubic (91s on a 1,352-line file); now precomputed newline offsets + `partition_point`. Unrelated
>   to this plan's terrain, but it is why the test suite timings below changed.
> - `crates/ynz-driver/tests/cross_impl_consistency.rs` — the corpus sweep is now parallel
>   (`parallel_sweep`), and both sweeps derive "is this program's output order scheduler-dependent?"
>   from the SOURCE (`output_order_is_scheduler_dependent`) rather than the filename. **Relevant to
>   this plan:** any new `background`/channel fixture it adds is now auto-classified correctly, with
>   no exclusion-list entry to remember.
> - `docker-compose.yml` — `/tmp` is tmpfs with `exec` in the dev container.
> - Full `cargo test --workspace` now runs in **~4.5 min** (was ~52 min). Budget verification
>   accordingly; the old "the suite takes an hour" assumption is dead.
> - New rule: [`.claude/rules/test-parallelism.md`](../../../rules/test-parallelism.md) — load it
>   before adding any fixture-looping test in this milestone.
>
> ---
>
> ### ▶️ UNBLOCKED (2026-09-03, fix round 2) — resume at **Phase 1, step 7** (Patrick's sign-off gate).
>
> Phase 1's round-2 review found a **use-after-free live on `main`** (v0.3.3, released): a task's
> `background` array argument sent into a channel was freed by the drop ladder while still sitting
> in the channel buffer. Patrick ruled it a hotfix on its own branch, ahead of M8 (**FRAGO 004** in
> `audit.md`). **That hotfix has LANDED and is in this branch's history**: `861fd4d` (runtime) +
> `30f6d36` (test isolation), merged as PR #89 at `ec014d8`, merged into this branch at `6143c1d`.
>
> **What shipped is NOT what FRAGO 004 ruling 2 described.** The ruling said *"the ladder consults
> consumption"* (codegen skips the free for a typeck-consumed binding). The shipped fix is a
> **runtime pointer-identity release protocol** (`release_ladder_payload`, `DriveIdentity`,
> `BG_ARG_KIND_RELEASED`) that consults the hand-off EVENT, not typeck — because one compiled body
> cannot know whether it was spawned or `wait`ed, the frame header is full, and a name match misses
> `let alias = rows`. The ruling's intent (one authoritative answer, no ladder taught about typeck)
> survives; its literal mechanism did not. The design section "Two mechanisms, one rule" in
> `IMP-concurrency.md` is the authoritative reconciliation: **both mechanisms stay** (typeck owns
> "may the source read this binding"; the runtime owns "does the ladder still own this
> allocation"), linked by the one `ChannelElemDrop` enum, and **P2-3's closed-send free moves into
> the runtime's CLOSED-first-poll path** (a codegen-side free would double-free a ladder-owned
> clone). Phase 4 inherits that section verbatim; do not re-derive it from this banner.
>
> **Exact resume state:**
> - Phase 0 — ✅ COMPLETE (record in `audit.md`).
> - Phase 1 — steps 1–6 complete through THREE review rounds + THREE fix rounds (`m8-p1-fix1`,
>   `m8-p1-fix2`, `m8-p1-fix3`). **Step 7 (Patrick's sign-off) is OPEN and has never been granted.**
>   Fix round 2 absorbed: FRAGO 004's ownership reconciliation (above); the `ConsumedBySend` WHY
>   rewrite; the `.copy()`-on-`map` ruling (with the finding that `map.copy()` already compiles today
>   as a silent ALIAS via codegen's `_ => Ok(recv_val)` catch-all); parked items 1, 2, 3, 4, 6, 9, 11
>   — item 11 is taken as a compile-ERROR guard (`HandleChannelArgNeedsBinding`). **Fix round 3
>   answered a BLOCKER**: the give-at-send rule did not flow through an ordinary call (`wait
>   producer(wire, rows)` then `rows.count()` — a use-after-free with no `background`; root cause
>   `check_arg_ownership` consumes only for a declared-`give` parameter, `check.rs:4599`/`:4646`).
>   Patrick ruled for guard A: a sent parameter must be declared `give` (`ParamNeedsGive`, which ALSO
>   closes the relay hole at `check.rs:4617` and retires the share refusal at `:4611`), and a
>   non-binding payload is refused (`SendPayloadNeedsCopy`). The design now ships **FOUR** new
>   compile-time diagnostics. Also round 3: `ChannelElemDrop` is `Option<{Array, Map}>`, not a
>   three-variant enum; THREE first-poll CLOSED arms collapse into one `refuse_closed` fallthrough;
>   `release_ladder_payload`'s kind filter gets one `ynz-abi` predicate + parity test; the
>   `map.copy()` independence fixture is RED-before-clone by obligation; FR#9's channel-door instance
>   is recorded as CLOSED by the guard (the container door stays). Parked items 5, 7, 8 stay parked
>   for Phase 4 (item 9 was absorbed in round 2 and is now marked; item 10 was fixed in round 1).
> - **Owed to Patrick at the sign-off gate — the design's "Open at sign-off" list, three items:**
>   fr12's disposition (unchanged, deliberately not decided by any fix round); the error-vs-warning
>   call on `HandleChannelArgNeedsBinding` (designed as an error, surfaced as HIS decision); and the
>   `give`-on-parameters requirement with its transit rule (a real language-ergonomics change —
>   confirm knowingly).
> - Phases 2, 3, 5, 7, 8, 9 — not started. Phase 4 — hard-blocked on Phase 1's sign-off. Phase 6 —
>   retired (FRAGO 001).
>
> ### ✅ PHASE 0 IS COMPLETE (2026-09-03).
>
> Double merge-and-tag gate **SATISFIED**: `main` at `cf17de3` carries PR #82 (M6, `10df6d7`) and
> PR #87 (M7, `f7eb2fa`, tagged v0.3.3). Branch for this plan: `feat/v0-3-m8-concurrency-completion`
> (frontmatter `branch:`), per the newly-written [`.claude/rules/branching.md`](../../../rules/branching.md) —
> `main` is protected, close-out is a PR, never a direct merge. Full Phase 0 record, including the
> complete drift table, is in `audit.md`.
>
> **Two amendments landed out of Phase 0 — read both before touching Phases 4 or 7:**
>
> **FRAGO 001 — Phase 6 is RETIRED.** M6 already shipped P2-7 (its FRAGO 010 / M6 Phase 4b,
> `b0cdbd3`); independently confirmed revert-sensitive. Nine phases now (0–5, 7–9), NOT renumbered.
> Details in the Phase 6 block below.
>
> **FRAGO 002 — two of this plan's citations are DANGLING, not merely offset.** They point at
> unrelated code in the current tree; navigating by them lands you in the wrong function:
>
> | Cited in this plan | Actually at, today | What the cited lines are NOW |
> |---|---|---|
> | `runtime.rs:591-693` "the drop ladder" (¶1 Terrain; the choke point **Phases 4 and 7 both wire through**) | **`runtime.rs:981-1050`** — `SpawnStateFnFuture::drop`, the kind-2 `BgArgDropEntry` arm calling `channel::purge_pending_sends` + `ynz_channel_free` | `ynz_rt_shutdown` — unrelated |
> | `check.rs:3392-3398` / `3047-3059` (kernel-mode gates, Invariants → Kernel-Mode Behavior) | construction gate **`~4316-4322`**; method-call gate **`~3972-3980`** | `background`/`share`/`lend` reject logic; an unrelated 1-arg intrinsic arg-type check |
>
> **Offset-only drift** (substance intact, just moved — safe to follow by name):
> `channel.rs:109-123` → `200-225` · `channel.rs:536-539`/`557-560` → `962`/`1123` ·
> `channel.rs:120` (`pending_sends`) → `213`, shape now `Mutex<HashMap<(u64,u64), PendingSendEntry>>` ·
> `emit.rs:~11833-11960` → `~12776-12961` (**P2-3's leak is genuinely still unfixed**; the
> "Structurally unreachable in v0.3-M4" comment is still there verbatim at `emit.rs:12852`) ·
> `IMP-no-function-coloring.md:281-294` → `295-311`. Exact, zero drift:
> `registry/features.toml:1229-1235` and `IMP-no-function-coloring.md:58`.

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
- **P2-7 — ❌ SUPERSEDED BY FRAGO 001 (2026-09-03). This bullet is stale; M6 DID fix P2-7** (its own
  FRAGO 010, shipped as M6 Phase 4b, commit `b0cdbd3`, in PR #82; independently confirmed
  revert-sensitive by a `code-reviewer-medium` on 2026-09-03). Phase 6 is retired. The original text
  is retained below unedited for the record — do not act on it.
  ~~`ynz_handle_recv_poll` panic-then-pending hang, newly surfaced in M6's audit, NOT fixed by
  M6.~~ `crates/ynz-runtime/src/handle.rs:297-303` — a panic inside the poll returns `Pending` with a
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
  brand-new codegen path instead of an already-audited one. [plan defect, found Phase 5: M7's FRAGO 001
  records that no `ynz_arc_*` symbol was declared or called from codegen; Phase 5 declared them first,
  bare like every sibling runtime decl] (FRAGO 013 — Phase 5 round 2)
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
  builds on top of [plan defect, found Phase 5: M7's FRAGO 001 records that no `ynz_arc_*` symbol was
  declared or called from codegen; Phase 5 declared them first, bare like every sibling runtime decl]
  (FRAGO 013 — Phase 5 round 2), plus the `--no-optimize` build-mode axis this plan's Phase 8 (fuzzing) reuses as a
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
| ~~**R6 — P2-7 handle-panic-hang fix reintroduces a race**~~ — **RETIRED, FRAGO 001 (2026-09-03)** | — | — | — | The hazard no longer exists in this plan's scope: M6 shipped the P2-7 fix itself (FRAGO 010 / M6 Phase 4b, `b0cdbd3`), so this plan writes no code on that path and cannot reintroduce a race there. Retirement is confirmed, not asserted — an independent reviewer reverted the ordering fix and watched both locking tests fail on the P2-7 hang assertion before restoring the tree | **n/a — risk retired** | closed |
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
4. **Milestone-boundary assumption flagged** — ⚠️ **PARTIALLY SUPERSEDED BY FRAGO 001 (2026-09-03):
   the P2-7 half of this claim is WRONG. M6 does own P2-7 and shipped it** (FRAGO 010 / M6 Phase 4b,
   `b0cdbd3`); only the P2-1/P2-3 and P2-6 halves stand. Original text follows:
   M6 owns every concurrency-release audit finding EXCEPT
   P2-1/P2-3 (channel-close design gap, this plan's Phases 1&4), P2-6 (Auto-Arc emission, this plan's
   Phases 2&5), and ~~P2-7 (handle panic-hang, this plan's Phase 6)~~ — M6's own plan text names this
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

> **Status (2026-09-03, dispatches `m8-p1-20260903-a1` + fix round `m8-p1-fix1-20260903`): steps
> 1–6 DONE; step 7 (Patrick sign-off) OPEN — Phase 4 stays blocked until it lands.** Decided:
> explicit **`.close()`** (non-suspending, `-> nothing`, no args, idempotent no-op on repeat),
> auto-close-on-last-producer DEFERRED (four fields recorded), bare-channel **`receive()` becomes
> `maybe<T>`** (`none` = end of stream; the HANDLE's `receive()` stays `T errors` because it carries
> the task's own failure — the two are deliberately different and the section says why), **`send()`
> gives its payload** for owned-heap element types (`array<T>`/`map<K,V>` — the sent binding is
> consumed; primitives and `string` unchanged; this is what makes the P2-3 closed-arm free sound and
> closes a live cross-task aliasing hole confirmed by probe on the current tree), send-after-close is
> a RUNTIME typed error (no compile diagnostic — cross-task flow is undecidable), close wakes every
> parked receiver. **ONE new compile-time diagnostic** (use-after-send, `ConsumedBySend`) — text in
> the section; it reaches Invariants → Teaching and Phase 9's gallery. `h.close()` NOT provided, with
> a real gap recorded as a four-field deferral (a call-materialized first channel arg on the handle
> form compiles today — probe: `background doubler(makeWire())` + `h.send(21)` prints 42 — and leaves
> no spawner binding to close). Doc home: `IMP-concurrency.md` "Channel Close — End-of-Stream
> Semantics"; the M6 Divergence entry is a pointer that retires at Phase 4. Registry for Phase 4:
> `[[primitive_intrinsic]]` ×3 (`close`, plus back-filling `send`/`receive` with this design's types
> and a NEW optional `param_ownership` schema field so `send`'s give is data), `[[deferred_language_feature]]`
> ×2 (`channel-auto-close-on-last-producer`, `background-handle-close`), `[[diagnostic_template]]` ×1
> (`ConsumedBySend`). The first draft's `T errors` typing and its unconditional closed-arm free were
> both reviewer BLOCKERs ruled on by Patrick; the section carries the rulings and the reasoning
> (vocabulary.md's `maybe`-vs-`errors` line; auto-propagation at `check.rs:3647–3653` turning
> end-of-stream into a task failure; `ynz_error_new` per normal loop exit). **Scope question
> surfaced, not absorbed:** fr12 (`channel<number>` marshalling) is separable from close and its
> design is NOT written by this phase — **disposition OPEN pending Patrick**: Phase 4 step or its own
> FRAGO'd design step.
>
> **Fix round 2 (`m8-p1-fix2-20260903`) — what changed in the section, so a reader of the text above
> is not misled (the registry count above is now `[[primitive_intrinsic]]` ×4 and
> `[[diagnostic_template]]` ×2):** (a) the hotfix `861fd4d` landed a RUNTIME pointer-identity release
> protocol for spawn-arg clones; the new subsection "Two mechanisms, one rule" rules that it and the
> typeck consume BOTH stay, states which is authoritative for what, names the `ChannelElemDrop` enum
> as their compile-time link, and corrects FRAGO 004 ruling 2's description of the mechanism; (b)
> **P2-3's closed-send free moves from codegen's closed arms into `channel_send_poll_guarded`'s
> CLOSED-first-poll path** (release + glue, mirroring the shipped re-poll-CLOSED arm), which
> inverts a shipped runtime doc comment and one test case — deliberately; (c) the "one owner at
> every moment" claim is scoped to payloads the task OWNS — FR#9's aliased bg args (`map`,
> `array<pointer-elem>`, union) are named as the class this design neither opens nor closes; (d)
> the `ConsumedBySend` WHY no longer says "is empty afterward" — it says the compiler refuses to
> build the read and nothing happens at runtime; (e) `.copy()` on `map<K,V>` is a Phase 4
> obligation with its registry entry, and the section records that `map.copy()` compiles TODAY as
> an alias no-op through codegen's catch-all (the FRAGO 014 stub class), so the copy must be real
> before the advice ships; (f) parked 1/3/4/6 are absorbed as text corrections (labels swapped
> back, enum-not-bool, "extract", the `consumed: Option<ConsumedBy>` signature change); parked 2
> mandates `alloca_in_entry_llvm` for the `maybe<T>` envelope; parked 11 becomes the
> `HandleChannelArgNeedsBinding` compile ERROR on the handle form. **The design now introduces TWO
> compile-time diagnostics**, both in Invariants → Teaching and Phase 9's gallery.
>
> **Fix round 3 (`m8-p1-fix3-20260903`) — a reviewer BLOCKER and four should-fixes; the diagnostic
> count above is now FOUR and `[[diagnostic_template]]` ×4:** (a) **the give-at-send rule did not
> flow through an ordinary call.** `function producer(wire, rows: array<int>) { wire.close();
> wire.send(rows) }` called as `wait producer(wire, rows)` consumed producer's parameter and left
> entrypoint's `rows` live — `print(rows.count())` read the payload the send core had just freed, a
> deterministic use-after-free with no `background` anywhere. Root cause: `check_arg_ownership`
> (`check.rs:4591–4648`) consumes the caller's binding only for a declared-`give` parameter
> (`:4599`); bare/`share` fall to `_ => {}` (`:4646`); and it runs only for plain idents
> (`:4798–4800`), so `wire.send(bucket.rows)` / `wire.send(matrix[0])` were never seen. **Patrick
> ruled for guard A**, both halves now in the design's "Ownership must flow through the call": (1)
> a parameter sent on an owned-heap channel must be declared `give` on its function —
> `ParamNeedsGive`, threading the EXISTING Give path so every call site AND every spawn site
> (the `Expr::Background` arm runs `infer_expr(inner)` → `check_user_fn_call` → the `:4799` site)
> consumes the caller's binding; (2) a non-ident payload (other than `.copy()` or a literal) is
> refused — `SendPayloadNeedsCopy`. **Transit decided**: the `give` obligation travels the WHOLE
> chain (A→B→C: B's parameter needs `give` too), enforced at the one site — `check_arg_ownership`'s
> Give arm, whose `:4617` silent consume of a bare/`lend` parameter IS the relay hole and becomes
> `ParamNeedsGive`; the error names the immediate frame only (one frame per compile, one word per
> fix, WHY says the word travels up). The share refusal at `:4611` is retired into the new template.
> Inferring `give` for bare parameters was weighed and rejected (ordering/fixpoint; silent change to
> the caller's program; signatures carry ownership). **FR#9's channel-door instance is CLOSED by
> guard (1)** — `background producer(wire, table)` with `give table` consumes the parent's `table` at
> the spawn — and the plan's FR#9 text says so; the container door (`bucket.add`) stays RED-pinned.
> (b) `ChannelElemDrop` is **`Option<ChannelElemDrop { Array, Map }>`**, not `{ None, Array, Map }`
> — an exhaustive match guarantees an arm, not that the arm registers glue (`Number =>
> const_null()` compiles); with `Option`, every `Some(kind)` arm is a function value. (c) **THREE**
> first-poll CLOSED arms, not two — `channel.rs:503` (`try_send → Closed`), `:554` (`Full`, then the
> fresh future's first poll `Ready(Err)`), and the new `None`-under-lock arm — collapse into one
> `refuse_closed` fallthrough (release + glue). (d) `release_ladder_payload`'s hand-maintained kind
> filter (`runtime.rs:933`) gets ONE `ynz-abi` predicate (`bg_arg_kind_is_releasable_payload`,
> inverted: everything but `SHARED_CHANNEL`/`RELEASED`), an `ALL_BG_ARG_KINDS` list, and a per-kind
> alloc/free parity test linking it to the ladder's free match (`runtime.rs:1125`). (e) the
> `map.copy()` independence fixture is committed **RED before** `ynz_map_clone` lands — the RED run
> is the evidence step 3a happened. Bookkeeping: parked item 9 marked ABSORBED (round 2); item 10
> corrected to fixed-in-round-1. The section header no longer says "DESIGN LOCKED"; an **"Open at
> sign-off"** list closes the section with fr12, the error-vs-warning call, and the `give`
> requirement as Patrick's three discrete decisions.
>
> **Obligations this design hands Phase 4 (each is cited in the section; listed here so the phase
> cannot miss one):** (1) the `receive()` retyping touches **19 bare-channel sites across 13 fixture
> files** plus `v0_3_m4_errors.ynz:80,93`, `pirates-roster/entrypoint.ynz:1083,1120–1122,1146`,
> `REF-concurrency.md:199–201,252`; (2) **no `channel<array<T>>`/`channel<map<K,V>>` E2E fixture
> exists anywhere in the tree** — one per element kind must be authored (round-trip, use-after-send
> compile error, send-after-close free with alloc/free parity, drop-with-buffered-element); (3) the
> "is this element owned-heap?" predicate is ONE shared definition typeck and codegen both read —
> **`Option<ChannelElemDrop { Array, Map }>`** (codegen needs the drop function; `Option` so that
> every `Some(kind)` arm MUST be a function value — a three-variant enum let `Number =>
> const_null()` compile), matched exhaustively inside `Some` in `channel_drop_glue` — never a bool,
> never a typeck twin; (4) `close` needs THREE typeck
> sites, not one: `known` (`check.rs:3993–3998`), the receiver-discipline + derivable-conduit guards
> (`check.rs:4011–4082`, which run unconditionally and justify themselves with "can suspend"), and
> the channel/handle-SHARED unknown-method string (`check.rs:4003`) which must split; (5) the
> `close()`-vs-`send()` ordering is linearized at the sender-lock clone — a send already holding a
> clone is a pre-close send and LANDS; a fixture or loom model demanding "refuse any `try_send` after
> `close()` returns" cannot pass and is wrong; (6) **loom is not a dependency yet** (no `loom` in any
> `Cargo.toml`) — Phase 3's `cfg(loom)` swap must land before the two new interleavings are
> model-checkable; (7) the existing `Consumed` template (`features.toml:1599–1603`) and its emitting
> code (`check.rs:3624–3629`) already disagree in wording — reconcile while adding `ConsumedBySend`
> at the same site; (8) **`.copy()` on `map<K,V>`** — new runtime `ynz_map_clone` (one-level deep
> copy mirroring `ynz_array_clone_primitive`), a `Type::BuiltinMap` arm in the `PostfixOpKind::Copy`
> lowering (`emit.rs:19301`), the registry entry, and an independence fixture in the shape of
> `m5_p5_copy_aos_independent.ynz` — lands BEFORE 3b so the diagnostic's advice is true, **and the
> fixture is committed RED before `ynz_map_clone` lands** (the RED run is the evidence the alias
> stub existed and 3a changed it); (9) **the shipped release protocol is untouched EXCEPT its
> CLOSED-first-poll path**, which has THREE arms — `None` under the sender lock; `try_send →
> Closed` (`channel.rs:503`); `Full` then the fresh future's first poll `Ready(Err)`
> (`channel.rs:554`) — all collapsed into ONE `refuse_closed` fallthrough doing
> `release_taken_value()` + `glue(value)`; the runtime doc comment and test case (c) of
> `ladder_is_untouched_when_the_channel_does_not_take_ownership` flip to the new contract, and
> codegen's `conduit_closed1`/`closed2` arms free nothing; (10) the `HandleChannelArgNeedsBinding`
> compile error at the handle-form pre-record (`check.rs:2321–2345`); (11) `pub enum ChannelElemDrop
> { Array, Map }` + `channel_elem_drop(&Type) -> Option<ChannelElemDrop>` in `ynz_typeck::types`,
> `match … { None => null, Some(kind) => <exhaustive, function-valued> }` in `channel_drop_glue`, and
> `check_channel_construction`'s `elem_supported` derived from it or parity-tested; (12) the
> `maybe<T>` envelope alloca via `alloca_in_entry_llvm`, never at `conduit_post`; (13)
> `is_consumed: bool` → `consumed: Option<ConsumedBy>` across the seven `ScopeEntry` constructors,
> `Scope::consume` gaining the cause, both `!is_consumed` guards; (14) the `channel<map<string,int>>`
> fixture covers BOTH a map built inside the sending task AND a map passed as a `give` bg arg and
> sent — both asserted CORRECT (the `give` consumes the parent's binding at the spawn; alloc/free
> parity) — plus the no-`give` variant as a `ParamNeedsGive` gallery trigger; FR#9's RED pin stays
> for the `bucket.add` container door only; (15) **`ParamNeedsGive`** at two sites that are one
> rule: the `send` arm (payload ident whose `ScopeEntry` is `is_param && param_ownership !=
> Some(Give)`) and `check_arg_ownership`'s Give arm (`check.rs:4599–4619` — the `:4617` silent
> consume of a bare/`lend` parameter becomes the error; the `:4611` share refusal is retired into
> the template, and whatever gallery/snapshot asserts the old share-refusal text is updated —
> `v0_3_m4_errors.ynz` first); the corpus compile confirms zero existing relay-through-bare-parameter
> instances; (16) **`SendPayloadNeedsCopy`** in the `send` arm: admitted payload forms are
> `Expr::Ident` (consumed), `.copy()` postfix (fresh), array/map literal (fresh); everything else is
> the error; (17) `release_ladder_payload`'s kind filter (`runtime.rs:933`) consumes ONE `ynz-abi`
> predicate `bg_arg_kind_is_releasable_payload(kind)` (inverted: `!= SHARED_CHANNEL && !=
> RELEASED`), with `ALL_BG_ARG_KINDS` and a per-kind alloc/free parity test against the ladder's free
> match (`runtime.rs:1125`), whose `_ => {}` arm gains a `debug_assert!(!releasable)`; (18) the
> ownership-flow fixture class: the reviewer's `wait producer(wire, rows)` program with and without
> `give`, a two-hop relay with and without `give` on the middle frame (error names the middle
> frame), the three refused payload forms beside the three admitted ones.

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

#### Phase 2 — Design: Auto-Arc Sharing Topology **+ the channel-send ownership rule** (Patrick sign-off gate)

> **FRAGO 008 (2026-09-03) — Phase 2's scope GREW. Read `audit.md`'s FRAGO 008 before starting.**
>
> Phase 1 spent three review rounds re-deriving an ownership analysis by enumerating syntactic
> call-site shapes, and each round found the same class of hole in the next syntactic form. The
> producer was named and the loop closed by re-diagnosis, not by patching instance four. **Patrick
> ruled the whole "who else holds this value" question moves here**, because this phase already owns
> `crates/ynz-typeck/src/effective_ownership.rs` — a whole-program Kleene fixpoint that converges
> under mutual recursion, runs before body checking, and already classifies "passed to a declared
> `give` position."
>
> **This phase now answers, by THREADING `effective_ownership`.** (Amended at Phase 2 sign-off,
> packet item (i), 2026-09-03, FRAGO 010: the design threads the fixpoint through ONE exhaustive,
> wildcard-free `Expr` match — `provenance()` — inside the owning module, `effective_ownership.rs`.
> That single match is the remedy for the corpse's ban on enumerating syntactic shapes at call
> sites, not an instance of it; "never enumerating `Expr::` variants" overstated the rule and is
> retired in favor of the ruled wording.) It answers:
> - Does `ch.send(value)` consume its argument, and how does that obligation transit call frames?
> - The `ConsumedBySend`, `ParamNeedsGive` and `SendPayloadNeedsCopy` diagnostics (drafted in
>   `IMP-concurrency.md`'s channel-close section — reuse the teaching text, re-derive the rule).
> - Which payload forms are admitted as fresh, and the `.copy()`-on-`map` obligation that makes the
>   diagnostics' own advice executable (`map.copy()` compiles today and silently aliases through a
>   codegen catch-all — the advice ships broken without `ynz_map_clone`).
>
> **Three concrete failing programs any answer must defeat** (all found by review, all currently
> compile-and-run correctly, all become use-after-frees under a naive consume rule):
> 1. `bucket.stash(rows)` — UFCS non-receiver arguments never reach `check_arg_ownership`
>    (`check.rs:3063-3085`, `:5431-5479`); only the receiver slot is checked.
> 2. `B(bucket.rows)` / `B(matrix[0])` into `function B(give rows: array<int>)` — the ident-only
>    gates (`check.rs:4798`, `:5114`, `:5444`) let a non-ident satisfy `give` while the caller still
>    holds the value.
> 3. `for (row in matrix) { wire.send(row) }` — a loop variable (`check.rs:2792-2803`) is a cell
>    pointer the parent still owns; the design refuses `matrix[0]` and admitted its loop-var twin.
>
> A fourth is visible and unaddressed: `dynamic Contract` dispatch carries no ownership modifiers at
> all (`shapes.rs:24-29`).
>
> **Also inherited:** the corrected fact that typeck DOES have whole-program ordering — so
> "one frame reported per compile" must be decided on its merits, not on the false premise that the
> infrastructure is missing. And `examples/primantis-orders/m6_errors.ynz:112-115` is a real
> blast-radius instance of `check.rs:4617`'s silent consume that any tightening converts into a second
> diagnostic on that line.
>
> **Phase 4 is now blocked on BOTH Phase 1's and Phase 2's sign-offs.**

> **FRAGO 009 (2026-09-03) — fr12 rides this phase.** Patrick's ruling: `channel<number>`
> decimal128 marshalling (fr12 — a 128-bit value through the channel's 64-bit `to_i64_bits` slot)
> is DESIGNED here, alongside the ownership rule, because parked item 3 already ties `number` to the
> `ChannelElemDrop` classification this phase owns; Phase 4 implements it. The design must state
> the marshalling representation, whether `number` joins the give-set or stays copy-through, and
> the alloc/free consequence either way. Same sign-off gate (step 6).

> ### Phase 2 STATUS — steps 1–6 ✅, FRAGO 008 absorption ✅, fr12 ✅; **step 6 (Patrick's sign-off) CLOSED 2026-09-03** (executors `m8-p2-20260903-a1`, `m8-p2-fix1-20260903`, `m8-p2-signoff-20260903`) — **exit criteria MET, Phase 5 UNBLOCKED**
>
> Deliverables (design only; no compiler code; seven throwaway probes run in the dev container and
> deleted — results in `audit.md`):
> - [x] **Step 1** — `EffectiveOwnership::Reads` is directly reusable for the TASK side (callee
>   position, transitive). The CALLER side needs one honest extension, `classify_binding_in_stmts`
>   (the existing per-name classifier over a statement suffix — same lattice, same walker), not a new
>   analysis. Two record corrections: the fixpoint runs AFTER the body check today (`check(...)`
>   `queries.rs:~423`, `analyze` `:503`) and must be hoisted (a reorder — it needs only parse +
>   signatures); and `arc.rs`'s "Arc-able floor" exists in a doc comment only, no code predicate.
> - [x] **Step 2** — Topology **(B)**: one shared heap copy, N task-held Arc references, the caller
>   keeps its original and holds ONE transient reference for the lexical extent of the spawn group.
>   (A) repoint-the-caller needs the scope-exit drop pass that does not exist and changes the
>   caller's frame representation (R2's class).
> - [x] **Step 3** — Beneficial iff ≥2 spawn statements in one block pass the same whole binding,
>   no suspension between first and last, task-side `Reads` on every member, caller-side `Reads`
>   between the spawns, and `arc_shareable(type)` (shape of primitive/string/inline-shape fields).
>   "Caller + 1 task" is NOT beneficial under (B). Loops, suspension-straddling groups, arrays/maps
>   are the named residual. **CHECKPOINT passed.**
> - [x] **Step 4** — `IMP-ownership.md` gained TWO sections: "Transfer — Who Else Holds This Value"
>   (the FRAGO 008 rule) and "Auto-Arc — Sharing Topology Across `background` Boundaries" (topology,
>   condition, proof, override analysis, residual). `IMP-no-function-coloring.md:58`'s dangling
>   pointer now names the section.
> - [x] **Step 5** — `auto_arc` hover text re-drafted, tied to the group's reader count `{n}`;
>   `.copy` → `.copy()`. Registry `modify` listed for Phase 5; `features.toml` not edited (design
>   phase).
> - [x] **FRAGO 008 absorption** — the rule is ONE `provenance()` function in
>   `effective_ownership.rs` (exhaustive `Expr` match, no wildcard; four values `Fresh` / `Whole` /
>   `Reaches` / `Unknown`) + binding ORIGIN and ALIAS CLASSES in `ScopeEntry` + ONE `check_transfer`
>   emit site called by a closed SINK list (channel send of `array`/`map`; every `give` position of
>   every call form incl. UFCS non-receiver args and `dynamic Contract`; `background` give) + two new
>   fixpoint facts (`consumed[fn][i]`, `returns_fresh[fn]`). Defeats the three failing programs AND
>   four more found by probe (alias-by-`let`, returns-a-piece, nested literal, dynamic). **`dynamic
>   Contract` is covered by construction** (`ContractSigDef` gains the AST's modifiers; dispatch site
>   threads the helper; `follows` checks modifier parity) — runtime exposure today is nil (codegen
>   refuses dynamic call sites, probe-confirmed). **One frame per compile → the WHOLE chain in one
>   compile** (the `consumed` fact; the false ordering premise is withdrawn; "infer give" stays
>   rejected on its two remaining reasons). `SendPayloadNeedsCopy` → **`TransferNeedsCopy`** (fires
>   at every sink). Container-store sinks (`add`/`set`/field-assign/index-assign) and literal
>   elements: deferred with four fields; a literal built from named heap values is `Reaches` (not
>   transferable) in the meantime.
> - [x] **fr12** — decimal128 crosses as a send-minted counted 16-byte cell (`number_to_heap_cell`,
>   the one existing helper), freed at the receive; glue `ChannelElemDrop::NumberCell`; **`number`
>   stays copy-through — it does NOT join the give set**, so the enum gains `transfers_source()`
>   (`Array|Map → true`, `NumberCell → false`) and typeck consumes on `is_some_and(transfers_source)`.
>   One alloc per send, one free per receive, parity-gated. `shape` and bignum stay deferred.
> - [x] **Step 6 — Patrick's sign-off.** SIGNED 2026-09-03, all twelve packet items ruled — the
>   SIGN-OFF record in `audit.md` (above FRAGO 009) is the authority. Sign-off round
>   `m8-p2-signoff-20260903` records it in the design, applies parked 19–27, and executes the owed
>   downstream plan edits under FRAGO 003's standing gate.
>
> **Round 1 grading (conductor `conductor-2026-09-03-m8-phase2`, 2026-09-03).** Green-check
> SKIPPED — docs-only diff, no compiler code (recorded, not silent). Seats derived from the round's
> own manifest: `plan-adherence-medium` (Fable) → `VERDICT: findings`, 0 blockers, 2 should-fix
> (the `background` inferred-give path is silent on `Cell`/`Reaches`/`Unknown` origins; the
> exhaustive-`Expr`-match-vs-"never enumerate" divergence is unsurfaced), 5 minor; no
> `frago-needed`. `doc-auditor-medium` (Fable) → `VERDICT: findings`, **1 BLOCKER** —
> `Stmt::Assign` (whole-binding reassignment, `rows = other`) is invisible to BOTH the origin/alias
> table (`IMP-ownership.md:226-236`) and the reused walker (`effective_ownership.rs:330-333`, `:379`
> classifies reassignment as a non-write), so alias-by-assign is a two-holder send and a
> between-spawns reassignment admits an Arc group sharing the OLD value; 4 should-fix (contract
> param modifiers are parser-optional, `parser.rs:3996-4012`, not REQUIRED as the doc says; a
> dangling `SCRATCH-audit-2026-07-11-memory-safety.md` cite behind the topology-(B) premise; dead
> `git log --grep=m8-p1-fix2/fix3` pointers; `Expr::PostfixOp` `.freeze()` has no provenance row);
> 3 minor. The executor's `code-reviewer` FIRE was routed to `doc-auditor` instead — the cited
> question ("do the claims about `effective_ownership.rs` hold against the module") is that
> charter, not a code-diff grade. Every other design claim against `effective_ownership.rs`,
> `queries.rs`, `arc.rs`, `ynz-ast`, `emit.rs`, `channel.rs` and the registry was verified true by
> line. Executor's in-place correction of `.claude/corpses.md` (fixpoint runs AFTER `check()`)
> RATIFIED by the conductor — both seats confirmed it against `queries.rs:423`/`:503`. Round 1
> sealed at `6a416c0`; fix round 2 answers `red:doc-auditor`.
>
> **Round 2 grading (fix round, executor `m8-p2-fix1-20260903`, Fable medium — the escalation
> notch is capped at medium in this house, receipt carries `red:doc-auditor`).** Green-check
> SKIPPED — docs-only. Seats re-derived from the round's own manifest: `plan-adherence-medium`
> (Fable) → `VERDICT: findings`, 0 blockers, 3 should-fix, 4 minor, no `frago-needed` (re-ran three
> probes itself; confirmed the blocker was fixed at the producer — a `Stmt`-exhaustive
> binding-event rule, not an `Assign` row); `doc-auditor-medium` (Fable, fresh dispatch — a
> different actor from round 1's, grading the whole section) → `VERDICT: findings`, 0 blockers, 7
> should-fix, 4 minor; `Stmt` exhaustiveness, scope-entry keying, walker extension, revive
> soundness, contract-modifier optionality and every git pointer verified against the code.
> **Round 2 terminal state: CLEAN.** All should-fix/minor collected to `.claude/plans/parked.md`
> items 19–27 with the trigger "applied in the round that records Patrick's sign-off". Step 6
> (sign-off) is the next action and is Patrick's.
>
> **Sign-off round grading (executor `m8-p2-signoff-20260903`, Sonnet high — coding/medium/
> moderate; docs-only, green-check SKIPPED).** Seats: `plan-adherence-medium` (Fable) →
> `VERDICT: findings`, **1 BLOCKER** — Phase 5 step 6 pre-ruled `false` for
> `bg_arg_kind_is_releasable_payload(ARC)` and attributed it to packet item (h), which only assigned
> the decision; 2 should-fix, 3 minor; every other plan hunk traced on CONTENT to its ruling
> letter; roadmap touch confirmed minimal (both duplicate ledger rows, parked 27). `doc-auditor-high`
> (Sonnet, after the gate denied a mis-addressed medium) → `VERDICT: findings`, 0 blockers, 2
> should-fix (the `maybe-move-out` registry bullet misstated the `[[deferred_language_feature]]`
> schema; a probe-count contradiction between the two IMP docs), 1 minor; parked 19–27 all verified
> true against the code. **Fix round (`m8-p2-signoff-fix1-20260903`, Sonnet medium,
> `red:plan-adherence`, six enumerated edits):** confirmed by a fresh `plan-adherence-medium`
> (Sonnet) → blocker FIXED (step 6 now carries the obligation and the design's leaning, no ruled
> answer), all five should-fixes landed, the added seventh probe sourced from a real run; 1 new
> should-fix — a dead `git log --grep=FRAGO-009` pointer, the SAME class as parked 26 → stop-test
> fired: closed at the producer by this phase's boundary commit carrying every cited token, and
> recorded as `.claude/corpses.md`'s second entry (parked 28). **Phase 2 terminal state: CLEAN.
> Phase-boundary commit next; frontier advances to Phase 3.**
>
> **Round 2 (fix, executor `m8-p2-fix1-20260903`, 2026-09-03; docs-only; six throwaway probes run
> and deleted, results in `audit.md`).** BLOCKER defeated at the producer, not the instance: the
> origin/alias table was an enumeration of binding-CREATING statement forms that omitted the
> binding-MUTATING one. Replaced by a **binding-event rule exhaustive over `Stmt`** (10 variants
> named; `Let` incl. shadowing, `Assign`, `For` incl. destructures, and params bind; the other
> seven do not) stating what a re-bind does to the previous class (leaves it; the old members keep
> their state; a consumed name that is reassigned is REVIVED — which also fixes a false error on
> today's tree, probe `eat(rows); rows = [4, 5]; rows.count()` refused). Caller-side Arc group: ONE
> predicate `stmt_rebinds` in `effective_ownership.rs`; the walker's `Stmt::Assign` arm becomes
> `Writes` on the tracked name (its `:330–333` comment corrected — named as the honest extension's
> second part); a TOP-LEVEL rebinding between spawns is a **group boundary**, a NESTED one is
> `Writes` (decline). Probe: `background render(scene, out); scene = other; background
> render(scene, out)` prints `8` today — round 1's group would have printed `2`. Should-fixes: (1)
> the `background` INFERRED-give path is NOT a `check_transfer` sink — class-aware liveness,
> `Give` only for `Owned`/`Param(give)`, everything else declines to `Copy` silently (codegen copies
> by type regardless; `bg_inferred` feeds only inlay hints); (2) the exhaustive-`Expr`-match
> divergence surfaced in the required form in the section and in the packet; (3) contract
> modifiers are OPTIONAL (parser), bare = never a give position, `follows` parity exact — the
> "REQUIRED on" line in "Signature-Level Declaration" corrected; (4) the dangling
> `SCRATCH-audit-2026-07-11-memory-safety.md` cite (never existed in git) replaced at all three
> sites with the code-direct premise (`ynz_handle_free` declared, zero emit sites) +
> `IMP-no-function-coloring.md` + the roadmap row; (5) dead `m8-p1-fix2/fix3` pointers → `de631bf`
> (verified by `git log -S`); (6) `.freeze()` provenance row (typed `nothing`, `Fresh`). Minors:
> `consumed[fn][i]` send case gated on the DECLARED param type; `emit.rs:12657`; round-by-round
> narrative trimmed to current state + pointer in both IMP docs and `corpses.md`. Step 6 still OPEN.
>
> **Downstream plan text this design invalidated is now EDITED** — Patrick's Phase 2 sign-off
> (`audit.md`'s SIGN-OFF record, 2026-09-03) authorized every edit the list above named under FRAGO
> 003's standing gate; each is applied and traced to FRAGO 010 (`audit.md`), the sign-off round
> `m8-p2-signoff-20260903`.

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
  Patrick's sign-off recorded (blocks Phase 5 until this lands). **MET — signed off 2026-09-03**
  (`audit.md`'s SIGN-OFF record, dispatch `m8-p2-signoff-20260903`); Phase 5 UNBLOCKED.
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
  reverted fix caught); the Tokio-internals boundary named explicitly. **MET 2026-09-03** (executor
  `m8-p3-20260903-a1`; completion block below) — pending the conductor's review round.
- **Reviewer fan-out:** adversarial gate-checker (does the spike's GREEN verdict genuinely prove
  tractability and detection, not just "the harness compiled"); code-reviewer (the loom-swappable type
  pattern applied to the real code); design-doc-alignment reviewer (the harness sits in front of the
  existing reactive test suite as an ADDITIONAL check, never a replacement).
- **Model tag:** `(coding, high, medium)` — checkpoint mark mandatory (>5 steps).

**Phase 3 complete (executor `m8-p3-20260903-a1`, 2026-09-03; one segment, CHECKPOINT passed
in-session, no handoff file):**

- **Steps 1–2, spike verdict GREEN.** Throwaway `tests/loom_spike.rs` (self-contained minimal
  mint/purge model on loom types; deleted after step 4 — a surviving second model of the same logic
  would be the parallel-implementation class): 4,518 interleavings exhausted in 153 ms unbounded,
  329 in 24 ms at `LOOM_MAX_PREEMPTIONS=2`; the unsalted-key (pre-M6 ABA) variant was reported by the
  model's OWN assertion (`left: 111, right: 222` — the dead caller's value delivered), not by a harness
  error. Both halves of the STOP condition held, so the phase proceeded.
- **Step 3, production no-op PROVEN, not asserted.** Method: pre-refactor baseline of the release
  `libynz_runtime.{rlib,a}` and a single-CGU LLVM-IR emit (`cargo rustc --release --lib --
  --emit=llvm-ir -C codegen-units=1`, 48,336 lines), rebuilt after the swap and diffed. Raw IR diff:
  78 lines, every one a `core::panic::Location` constant (`<{ ptr, [16 x i8] }>` = file/len/line/col)
  whose line number shifted by the inserted `cfg` lines; masking only those 16 payload bytes and the
  content-hash `@alloc_*` names gives a **0-line diff** (83 Location constants on both sides, same
  line count). Disassembled `.text` of every object in the staticlib: 0 instruction lines differ (only
  `.llvm.<module-hash>` section-name suffixes). Re-run after the loom-only `mpsc_witness` field was
  added: still 0 masked lines. R3's mitigation is satisfied by construction — `crate::sync` is a bare
  `pub(crate) use std::sync::{Arc, Mutex, MutexGuard}` in every non-loom build.
- **Step 4, the real code behind the swap.** `crates/ynz-runtime/src/sync.rs` (the ONE import site);
  `channel.rs`/`handle.rs` import `Arc`/`Mutex`/`MutexGuard` from it; `CURRENT_DRIVE` declared as two
  cfg twins (loom's `thread_local!` has no `const {}` form and loom runs every model thread on one OS
  thread, so a std thread-local would be shared); `CALLER_GENERATION` and `runtime.rs`'s Tokio
  lifecycle statics deliberately NOT swapped (documented in `sync.rs`). Six models in
  `src/loom_tests.rs`, all driving the real extern-C shims / the one keyed send core / the one purge
  helper / the real `SpawnStateFnFuture` drop ladder, unbounded exhaustive counts: ABA same-token
  new-generation 987 · orphan purge, frame producer 3 · orphan purge, handle producer (real
  `ynz_handle_send_poll` → `ynz_handle_free`) 3 · drop-ladder kind-2 arm with a live co-owner 12 ·
  drop-ladder kind-2 arm where the ladder may hold the last reference 9 · recv register-before-poll
  42,563 — whole lane ~1.5 s. A loom-only per-channel `mpsc_witness` atomic (one RMW before each Tokio
  mpsc call) makes the untracked Tokio calls dependent for loom's DPOR so their relative orders are
  exhaustively explored rather than incidentally; measured optional for the teeth below (caught with
  or without it), kept for the exhaustiveness guarantee, rationale on `YnzChannel::mpsc_step`.
- **Step 5, teeth — each fix reverted in the working tree, loom run, tree restored (git-diff sha256
  verified identical before/after every revert; script under the session scratchpad, not committed):**
  ABA unsalted key → `loom_aba_*` reports the dead value delivered ✓ · `purge_pending_sends` made a
  no-op → BOTH `loom_orphan_purge_on_frame_cancellation` and `loom_orphan_purge_on_handle_free` report
  the surviving entry ✓ · kind-2 arm's purge removed → `loom_drop_ladder_*_with_live_co_owner` reports
  the orphan ✓ · kind-2 arm's ORDER swapped (release ref, then purge) → at round 1 the lane only
  died with SIGSEGV (the live-co-owner model passed the swap clean); **since fix round 2 BOTH
  `loom_drop_ladder_*` models report it by assertion** (`assert_purged_before_released` — a
  co-owner probe: reference gone ⇒ the ladder's entry must already be purged, explored at every
  point of the ladder by loom), and the last-reference sequential case has its own deterministic
  test (`ladder_holding_last_reference_purges_parked_send_before_channel_teardown`, `lib.rs`)
  which the sanitizer lane runs — under the swap Miri reports UB at the dangling purge
  (`purge_pending_sends`, `channel.rs`), the class that lane exists for · recv poll-then-record → `loom_recv_*`
  reports `lost wakeup: consumer A is Pending with a value buffered and was never woken` ✓ (found in
  0.01 s). Two harness defects were found and fixed BY these teeth runs, both recorded so the next
  reader does not re-learn them: the drop-glue log was a process-global static shared by libtest's
  parallel test threads (fixed by per-iteration payload tagging, not by serializing), and the recv
  model's post-join probe `recv` itself drained `recv_waiters` and re-woke the receiver the race had
  lost (fixed by snapshotting wake counts before probing).
- **Step 6, boundary named** — Future Requirements #4 below rewritten with the concrete mechanism.
  Additional scoping: the handle-side P2-7 `ynz_handle_recv_poll` register-before-poll fix is a
  panic-then-Pending robustness property, not an interleaving one (a single receiver's mpsc slot is
  never clobbered), so it is NOT a loom model — its existing deterministic `HandleOrderProbe` test with
  `panic_at_mpsc_clone` stays the coverage.
- **Demo & Error Gallery:** considered, N/A — this phase adds no executable language surface (dev/CI
  harness only; zero change to `libynz_runtime.a`, proven above).
- **Round 1 grading (conductor, 2026-09-03).** `green-check-low` (Haiku) → `VERDICT: green` (27
  lib + 4 integration + 6 loom non-vacuous; clippy clean plain and `--cfg loom`; release build;
  gitleaks clean). Seats from the manifest: `plan-adherence-low` (Haiku) → clean; `test-quality-medium`
  (Sonnet) → **1 BLOCKER** — the kind-2 drop-ladder purge→free ORDER is asserted by no model: the
  last-reference loom model catches a swap only by SIGSEGV, the live-co-owner model
  (`loom_tests.rs:398-451`) passes the swap clean (re-run by the seat, 12 interleavings); 1 minor
  (CI runs unbounded only; bound-2 passes locally on all six); `code-reviewer-medium` (Fable) →
  `VERDICT: findings`, 0 blockers, 3 should-fix (the same ORDER gap as a should-fix — one producer,
  one fix; `mpsc_witness` has no `mpsc_step()` before the `retain` that drops parked `Send`
  futures, so the orphan models explore 3 interleavings each; the CI loom step lacks `pipefail`,
  making the grep the accidental sole failure gate), 1 minor (`IMP-concurrency.md:1019` overclaims
  "one import site" — `runtime.rs:26` is a named exemption). The one-bullet IMP edit was graded
  inside `code-reviewer`'s brief rather than earning a `doc-auditor` seat. **Conductor defect,
  recorded:** two tree-mutating seats (`test-quality` revert experiments, `code-reviewer` reads)
  ran concurrently in one checkout; `code-reviewer` observed the ladder arm in its REVERTED order
  mid-grade. Tree restoration was sha256-verified so the grades stand, but from here any seat that
  reverts code runs ALONE. Fix round 2 answers `red:test-quality`.
- **Round 2 (fix, executor `m8-p3-fix1-20260904`, 2026-09-04) — the blocker fixed at the producer.**
  The models checked *purge happened* and *refcount balanced*, never *which came first*; now the
  ORDER is an asserted state invariant: `loom_tests::assert_purged_before_released` (a co-owner
  holding its own reference probes `strong_count == 1 ⇒ !pending_send_contains(ladder key)`, both
  reads loom-tracked so loom schedules the probe at every point of the ladder), called in BOTH
  kind-2 models. Revert-proof (kind-2 arm swapped in the working tree, restored, `git diff` sha256
  identical before/after): live-co-owner model **FAILS BY ASSERTION** (was: passed clean),
  last-reference model **FAILS BY ASSERTION** (was: SIGSEGV). New deterministic test
  `ladder_holding_last_reference_purges_parked_send_before_channel_teardown` (`lib.rs`, module
  `m6_pending_send_aba`; main releases first so the ladder tears down; asserts the glue sequence
  `[parked, filler]` — purge glues the parked payload, teardown's drain glues the filler) passes
  under `cargo test` and under Miri; **under the swap it does NOT fail by assertion in any build**
  and cannot: the swap's first effect is a use-after-free inside `drop(fut)`, before any post-hoc
  assertion runs, and the only pre-corruption observation point (the element glue) sits behind an
  `extern "C"` boundary that cannot unwind. Observed: plain debug build → rustc's misaligned-pointer
  UB check aborts inside the dangling purge (SIGABRT); Miri → "Undefined Behavior" at
  `purge_pending_sends`'s `&*chan_ptr` (`channel.rs:756`) from `runtime.rs:1153`. That is the
  sanitizer lane's finding, which is where the dispatch placed this test; the deviation from
  "all three by assertion" is recorded, not smoothed. Should-fixes: `mpsc_step()` witness before
  both `retain`s that drop parked `Send` futures (`purge_pending_sends`, the insert-time sweep) —
  interleavings orphan_frame 3→9, orphan_handle 3→9, ladder_last 9→27, ladder_live 12→57 (probe +
  witness), aba 987→11,079, recv 42,563 unchanged; lane 1.5s→1.7s · CI loom step: `shell: bash` +
  `set -euo pipefail`, Patrick's 2026-09-03 blocking ruling recorded in the step comment, bounded
  run documented as a strict subset of the exhaustive unbounded run rather than added ·
  `IMP-concurrency.md` "one import site" scoped to `channel.rs`/`handle.rs` with the `RUNTIME`
  exemption named. Production kind-2 arm untouched; every `channel.rs` line added is
  `#[cfg(loom)]`/`#[cfg(all(test, loom))]`, so the round-1 no-op proof stands unchanged.
- **Round 2 grading (conductor, 2026-09-04).** `green-check-low` (Haiku) → green (106 lib, 6 loom,
  clippy both cfgs, release build, gitleaks clean) with `runner missing: miri` — **a false miss**:
  `cargo +nightly miri` is installed in the dev image and the pre-existing CI `sanitizers` job runs
  it on `ynz-runtime`; recorded, no halt. Seats: `code-reviewer-medium` (Sonnet, read-only, ran
  first) → clean (probe implication sound, both reads loom-tracked, witness precedes every drop
  path, every added line cfg-gated, `runtime.rs` zero-line diff); `test-quality-medium` (Sonnet,
  ran ALONE after, reverting the kind-2 arm itself) → `VERDICT: findings`, **0 blockers** — both
  kind-2 loom models now FAIL BY ASSERTION on the swap (`loom_tests.rs:415`); the deterministic
  last-reference test fails on the swap every run (5/6 SIGABRT via the misaligned-pointer UB check,
  1/6 by its own sequence assertion — the round-2 audit entry's "cannot in any build" is therefore
  overclaimed, parked 29); Miri reports the swap as UB at `channel.rs:756` and passes the correct
  tree; the probe's antecedent is reached (not vacuous); 15 × 16-thread runs stable; 1 minor parked
  (30, `GLUE_SEQUENCE` payload-collision guard). `plan-adherence` NOT dispatched: the round's one
  deviation was from the conductor's dispatch wording, not the plan — step 5 asks only that each
  reverted fix be caught, and it is. **Phase 3 terminal state: CLEAN. Exit criteria MET. Boundary
  commit on Patrick's standing go (2026-09-04, "if it all works out, move on"); frontier → Phase 4.**
- **CI:** a `Loom` step added to `.github/workflows/ci.yml`'s main job (own `target/loom` dir; asserts
  ≥1 test passed so a filter drift cannot pass vacuously; since round 2, `pipefail` so cargo's own
  exit status is the primary gate and the grep is only the vacuity guard).
- **Registry:** no entries — nothing user-facing.

#### Phase 4 — Implement: Channel Close Semantics + P2-3 Leak Fix

> **Status (2026-09-04, dispatch `m8-p4-20260904-a2`, segment 2 — IMPLEMENTATION COMPLETE, steps
> 2–8 done; pending conductor review):** every one of segment 1's 24 fixtures is GREEN
> (`cargo test -p ynz-driver --test v03_m8_channel_close`: 24 passed), the m8 gallery renders all
> 29 diagnostics with every pinned phrase, and the named suite is green — `ynz-driver`
> `integration` (530), `error_galleries` (10), `cross_impl_consistency` (7, both modes),
> `ynz-runtime --lib` (110 incl. the new `refuse_closed`/close/kind-parity gates), the loom lane
> (8 models, the two new close models revert-proven), `ynz-typeck` (all targets incl. the new
> `diagnostic_template_parity`), `ynz-registry`, `ynz-lsp`. Shipped: 3a `ynz_map_clone` + the
> `BuiltinMap` copy arm; 3d `ChannelElemDrop { Array, Map, NumberCell }` + `transfers_source()`
> + the send-minted / receive-freed cell; 3b the transfer rule (`provenance`, `consumed`,
> `returns_fresh` in the hoisted fixpoint; `Origin`/`ConsumedBy`/alias classes; ONE
> `check_transfer` at the closed sink list incl. UFCS non-receiver args and `dynamic Contract`
> with `follows` parity; the three registry-rendered diagnostics; `root_binding_name` twin
> collapsed; the `Consumed` template reconciled + the parity test); 2/3/4b `.close()` at the
> three typeck sites, bare `receive()` → `maybe<T>` through the one entry-block envelope, the
> 19-site rewrite + demo (new `m8_demo` section, golden regenerated) + spec; the SIGSEGV
> producer fixed (`HANDLE_RET_KIND_VALUE_MAYBE`); 3c `HandleChannelArgNeedsBinding`; 4 the
> runtime `refuse_closed` collapse, `bg_arg_kind_is_releasable_payload` + `ALL_BG_ARG_KINDS` +
> the per-kind parity test; `param_ownership` with `build.rs` validation; the M6 divergence entry
> retired; the registry entries. Deviations and "design says A; compiler has B" findings are in
> `audit.md`'s `m8-p4-20260904-a2` entry (four hotfix fixtures were relay-through-bare-parameter
> instances the design's corpus scan predated — corrected to `give`; two `number` fixtures used
> `.or(0)`, a form the language rejects by a shipped gate — corrected to `.or(0.0)`; the
> pre-existing maybe-cannot-cross-a-suspension limit shaped one fixture rewrite and stands).
>
> **Round 1 grading (conductor, 2026-09-04).** RED segment sealed at `6b8a34d` (green-check skipped —
> red by construction; `test-quality-high` graded the RED set → clean, and resolved the two parity
> "unknowns" from untouched runtime code). Implementation segment: `green-check-medium` (Sonnet) →
> red on fmt + one `manual_contains`; `executor-low` fix (`m8-p4-fix1-20260904`); `green-check-low`
> re-gate → fmt 0, all tests green (110 lib, 8 loom, 24/10/530/7 driver, typeck all targets, Miri 20),
> and **workspace `clippy --all-targets` red on pre-existing test-target lint debt in files this
> phase never touched** (`ynz-lsp/tests/*`, `ynz-numerics`, `ynz-diagnostics/tests/jargon_audit.rs`,
> `ynz-watch`); CI (`ci.yml:78`) runs clippy WITHOUT `--all-targets`, so it never sees them. Treated
> GREEN for this diff; the debt is parked (31). Seats from the manifest — four read-only in
> parallel, `test-quality` deferred to round 2 against the post-fix test set (recorded, not skipped):
> `ux-medium` (Sonnet) → **2 BLOCKERS**: every rendered m8-gallery diagnostic points 3–5 lines past
> its trigger (landing in the next function's `// WHY:` comment, leaking dev vocabulary to the
> user); a span past EOF silently drops the WHAT-INSTEAD/WHY block and the executor reordered ONE
> trigger around it, leaving `m8_close_with_args` as the new victim — a duct-tape tell; 1 should-fix
> (30+ snake_case gallery fn names), 2 minor. `doc-auditor-high` (Sonnet) → **1 BLOCKER**:
> `REF-concurrency.md:285-293`'s new example reads `late.message` on an `errors` value, which
> ICEs today (`field_gep: receiver is not a Shape, got ErrorsCapable` — no `.message` arm beside
> `.failed()`/`.or()`); 3 should-fix (stale `IMP-concurrency.md:987` paragraph contradicting its
> own header; milestone tag + internal path in `channel-element-heap-upgrade`'s `why`, LSP-rendered;
> stale "cheap, trivially-copyable" comment at `REF-ownership.md:83`). `code-reviewer-medium` (Fable)
> → **2 BLOCKERS**: `check_transfer` (`check.rs:5083`) returns silently on an already-consumed
> entry, so same-call alias pairs slip — `let other = rows; eat2(rows, other)` compiles and prints
> `3 3`, `background eat2(rows, other)` gives the ladder two descriptors for one allocation; the
> `NumberCell` predicate is RE-DERIVED at `emit.rs:12691`/`:12708` as `Type::Number { precision <=
> 34 }` instead of threading `channel_elem_drop` — the twin class the enum exists to kill; 3
> should-fix (`fixpoint_type_oracle` answers `None` for `let` locals → false `MayAlias` refusal on
> builders with a computed scalar; `copy_is_independent` claims "parity-tested" with no test; the
> parity ratchet's tier-2 check is token-presence), 2 minor. `plan-adherence-high` (Sonnet) → 0
> blockers, no `frago-needed`, every exit criterion independently re-run incl. the RED at `6b8a34d`
> in a worktree; 3 should-fix (`IMP-ownership.md:277` overclaims receiver parity vs item (j) and the
> code; the renderer-EOF bug had no durable record; the same untested "parity-tested" claim), 1
> minor. **Five blockers, three producers**: the diagnostics renderer's span handling (ux ×2), a
> missing `ErrorsCapable` `.message` codegen arm (doc), the transfer check's same-call ordering +
> one `ChannelElemDrop` twin (code). Fix round 2 answers `red:code-reviewer`.
>
> **Round 2 grading (fix round `m8-p4-fix2-20260904`, Fable medium; conductor, 2026-09-04).**
> Root cause of the renderer blockers: byte-indexed `SourceSpan` rendered under ariadne's default
> `IndexType::Char` — every gallery diagnostic since M4 pointed past its trigger by the file's
> multi-byte surplus (m6/m7 too); `IndexType::Byte` + `clamp_to_source` fixes all galleries.
> `green-check-medium` (Sonnet) → all lanes green (29/10/530/7 driver, typeck/diagnostics/abi/codegen
> all targets, 110 lib + 8 loom, release builds, gallery `// WHY:` excerpt count 0, gitleaks); red
> only on `clippy --all-targets` in `ynz-typeck` test files last touched at `3b7e6e9` — pre-M8,
> not in the diff, added to parked 31. Seats (four read-only in parallel): `ux-low` (Haiku) → clean
> — 10+ excerpts on their trigger lines, last diagnostic renders its full block, 32 camelCase / 0
> snake_case gallery fns; `plan-adherence-medium` (Sonnet) → clean — all 20 round-1 findings walked,
> each fixed or declined on record, the consumed-classes snapshot ruled sound and risk-neutral, every
> exit criterion re-run; `doc-auditor-medium` (Sonnet) → 0 blockers, 1 should-fix
> (`IMP-ownership.md:277` still carries the stale "typeck drops the modifiers… checks no ownership at
> all" sentence beside the corrected text, with a dead `check.rs:5391` cite — real site is
> `check.rs:~6153-6190`), 2 minor; `code-reviewer-medium` (Fable, seven probes) → **1 BLOCKER**:
> `.message` on a NOT-failed `errors` value SIGABRTs — `emit.rs:~19235-19248` calls
> `ynz_error_message(err_ptr)` unconditionally then `select`s "" on null, and `select` does not
> short-circuit; the arm's own comment claims the opposite; the new fixture covers only the failed
> path. **Producer named upstream**: `REF-errors.md:171-175` says `.message` without a `.failed()`
> check is a COMPILE ERROR; typeck types `.message` as `string` unconditionally (`check.rs:~6364`,
> `~7051`), so codegen was guarding a path the design forbids. 2 should-fix (that divergence; the
> `CHANNEL_ELEM_SUPPORTED_NAMES` parity test is hand-picked and `channel_elem_drop` keeps a `_ =>
> None`, so admitting a new element kind stays green — plus a duplicate per-variant sampler in
> `emit.rs`), 5 minor (`copy_lowering_arm` calls bignum `ByValue` though `precision > 34` lowers as
> a pointer — unreachable today; `TYPE_VARIANT_COUNT` hand constant; single-line-only const parser;
> `call_argument_text` string-skip fragility; a truncated registry comment). `test-quality` still
> DEFERRED — round 3 changes the test set again; it grades once, on the final set. **A fix that
> opened a gap earns exactly one more round: fix round 3 answers `red:code-reviewer`.**
>
> **Round 3 grading (fix round `m8-p4-fix3-20260904`, Sonnet high; conductor, 2026-09-04).**
> Both producers landed: the spec's `.message`-after-`.failed()` rule as a flow-sensitive typeck
> guard shared by both dispatch paths (`MessageBeforeFailedCheck`, registry-rendered, gallery
> trigger), and codegen's `select` replaced by a real `br`/`phi`; four latent typeck tests that
> asserted the UNCHECKED read compiled clean were corrected; one shared `type_variant_sampler`
> replaced two hand-written per-variant lists. `green-check-medium` (Sonnet) → green on every lane
> (typeck 222/95/13, codegen 17, driver 31/10/530/7, runtime 110 + loom 8, lsp, registry, release
> builds, gitleaks); `clippy --all-targets` red only on parked-31 files/lines outside every
> changed hunk. Seats: `ux-low` (Haiku) → clean; `code-reviewer-high` (Sonnet, probes) → **2
> BLOCKERS**: the guard is name-keyed, so `if (x.failed()) { let x = computeB(); print(x.message)
> }` admits an unchecked rebinding (prints `""` via the new codegen branch — no crash);
> `.trace`/`.suggestions`/`.source` inside a check ICE (`emit.rs:~19281`, "only .message") —
> pre-existing, loud; everything else (wrong binding, nesting, compound conditions, non-ident
> receiver, early-return refused-but-unpromised) held. **CEILING REACHED — Patrick's ruling (FRAGO
> 011): close Phase 4 by ceiling; both blockers + parked 32 share one ancestor (the `errors`-value
> field surface was never finished) and ride ONE hotfix branch `fix/errors-fields` AFTER M8
> closes; parked 33/34.** `test-quality-high` (Sonnet) dispatched ALONE on the final Phase 4 test
> set (the seat held back through three rounds so it grades what ships) → `VERDICT: findings`,
> **0 blockers**, five revert-proofs run by the seat all failed loud (release-before-glue;
> linearization; alias snapshot via the two exact-count typeck tests; `NumberCell`
> `transfers_source`; byte-span renderer + gallery caret check), 5 should-fix parked (35–39: the
> two open blockers have no RED pin — the hotfix branch's first commit; one fixture can't see the
> ladder slot; three alias fixtures overclaim in their comments; a whole-module `select` scan;
> close-wakes-all tested with one waiter), 2 minor. **Conductor incident, recovered:** that seat
> restored its reverts with `git checkout --`, which on the UNSEALED round-3 tree reset
> `check.rs`/`types.rs` to round 2's seal and wiped round 3's typeck work; recovered from the
> seat's own pre-experiment `git stash` object (`d624246`, dangling), verified by re-running the
> typeck (222/95/13) and driver (31/10) lanes to the counts green-check had observed, and by both
> revert fingerprints reading pristine. Third `.claude/corpses.md` entry written: a mutating seat
> runs only on a sealed tree and never restores via `git checkout --`. **Phase 4 terminal state:
> CLOSED BY CEILING (FRAGO 011). Exit criteria MET for its charter. Boundary commit on Patrick's standing go;
> frontier → Phase 5.**

> **Fix round 2 (dispatch `m8-p4-fix2-20260904`, 2026-09-04 — all five blockers closed, all
> eight should-fixes done, four minors done; uncommitted, pending conductor review).**
> Producer A was ONE renderer defect, not this phase's emit sites: `SourceSpan` offsets are
> bytes and `ariadne::Config::default()` indexes by CHARS, so every multi-byte character before
> a span (the galleries' `──────` rules and em dashes) pushed the caret forward by the byte
> surplus (m8: +28 bytes at line 19 → rendered at 22:11; +90 at line 92 → 95) and a span whose
> byte offset exceeded the file's char count was dropped by ariadne with its note — the m6/m7
> galleries were off too. Fix: `IndexType::Byte` + a past-EOF clamp in `render.rs`; three RED→
> GREEN tests in `ynz-diagnostics/tests/byte_spans.rs`; the gallery reorder workaround removed
> (the handle trigger is LAST on purpose now). Producer B: `.message` IS spec-defined
> (`REF-errors.md`), so the codegen arm landed in `lower_field_access` (null-safe select to the
> empty string); fixture `v0_3_m8_p4_errors_message_after_failed.ynz` RED (ICE) → GREEN.
> Producer C1: `check_transfer` takes the call form's PRE-CALL snapshot of consumed alias
> classes (`Scope::consumed_classes`); a consumed class absent from the snapshot was consumed
> by an earlier position of the same call and is reported through the ONE consumed-read
> rendering (`consumed_read_diag`, shared with `resolve_ident`); non-`give` positions run the
> same read check (`mix(give a, share b)`); `ConsumedBy::Given` gained `given` and the
> `Consumed` template a `{via}` slot. Three fixtures RED (`3 3` printed / compiled) → GREEN
> refused. C2: both NumberCell twins replaced with `channel_elem_drop(..) ==
> Some(NumberCell)`. Should-fixes: camelCase gallery names; `IMP-concurrency.md`'s CLOSED-
> first-poll paragraph rewritten to shipped state; the registry `why` cleaned; `REF-ownership.md:83`
> comment; the fixpoint oracle resolves `let` locals (annotation / literal / receiver-independent
> scalar builtin from the registry) — `wire.send(build(bucket))` RED refused → GREEN sends;
> `copy_lowering_arm` (exhaustive, the Copy lowering dispatches on it) + `copy_parity_tests`;
> the parity ratchet's tier 2 parses `registry_diag(` argument lists (exemption gone);
> `IMP-ownership.md:277` says what the code enforces. Minors: caret tag "given away"; `—` in
> `closed_msg`; the "Use one of" list rendered from `CHANNEL_ELEM_SUPPORTED_NAMES` with a parity
> test; `ALL_BG_ARG_KINDS` source-parity test. Verified (exit 0 each): fmt; typeck all-targets
> (incl. 6 new parity tests); diagnostics all-targets; abi; codegen `copy_parity`; driver
> `v03_m8_channel_close` 29, `error_galleries` 10, `integration` 530; runtime lib 110; loom 8;
> clippy clean on every touched lib/test target (the parked-31 test-target debt in
> `jargon_audit.rs` / `independence.rs` / typeck's older test files is untouched, as instructed);
> `ynz-driver --release` rebuilt. Demo golden unchanged (the demo prints no `.message`).

> **Fix round 3 (dispatch `m8-p4-fix3-20260904`, 2026-09-04 — the blocker closed, both
> producers fixed, all seven should-fix/minor items done; uncommitted, pending conductor
> review).** Producer A (upstream, typeck): `.message`/`.suggestions`/`.trace`/`.source` were
> typed unconditionally regardless of whether `.failed()` had been checked
> (`check_errors_capable_method`/`infer_field_access`); implemented the spec's rule
> (`REF-errors.md:171-175`) as a flow-sensitive guard — a new `errors_failed_true_branch: Vec
> <String>` pushed/popped strictly around `check_stmt_if`'s body-check for `if (x.failed())`,
> consulted by a new shared gate (`check_errors_field_needs_failed_check`) both dispatch paths
> call; a new `MessageBeforeFailedCheck` registry template + `DiagnosticKind` variant renders
> it. A genuinely separate, pre-existing bug surfaced while wiring the fix (see Deviations):
> `resolve_ident`'s auto-propagation stripped `ErrorsCapable` on FIRST use inside an `errors`-
> capable function for the `Expr::MethodCall` path only (a compensating restore existed there);
> `Expr::FieldAccess` had no equivalent, so `.message` inside `if (x.failed()) {...}` in an
> `errors`-capable function ICE'd ("`string` values do not have fields") before this fix — a
> new shared `restore_ec_receiver_ty` helper (replacing the duplicated inline block the
> MethodCall arm carried) fixes both dispatch paths the same way. Producer B (codegen defense):
> `emit.rs`'s `.message` arm called `ynz_error_message` unconditionally then `select`ed "" on
> null — `select` evaluates both operands, so the call ran on a null pointer on the success
> path; replaced with a real `br i1`/`phi` (call only inside the failed block). Corpus sweep
> (`.message`/`.suggestions`/`.trace`/`.source` across `crates/ynz-driver/tests/fixtures/*.ynz`
> and `examples/`): one prior instance, already inside `if (late.failed())`, legitimate,
> admitted unchanged. A SEPARATE sweep of `ynz-typeck`'s own embedded-source test corpus
> (missed by the fixture-only grep, caught by running `--all-targets`) found four latent-bug
> instances: `ec_method_{message,suggestions,trace,source}_resolves_in_ec_fn` in
> `crates/ynz-typeck/tests/check.rs` asserted the UNCHECKED read compiled clean — exactly the
> class this round closes — corrected to read inside `if (x.failed()) {...}`, preserving their
> original EC_METHODS-restoration intent. RED→GREEN: the blocker probe
> (`v0_3_m8_p4_fix3_message_before_failed_check.ynz`) ICEs→SIGABRT→refused across the three
> fix rounds; a new IR-level test
> (`m8_p4_fix3_message_call_sits_in_a_real_conditional_block_not_a_select`) asserts codegen's
> defense directly (`--emit-ir --no-optimize`, no `select`, a real `br i1`/`phi`) since typeck
> now refuses every source program that would reach the not-failed codegen path, making it
> unreachable from source. Should-fixes: items 3–9 all done (see Deviations for the full list
> and the one declined-as-unnecessary item). Verified (exit 0 each): fmt; clippy `-D warnings`
> on every touched lib/test target (ynz-diagnostics lib, ynz-typeck lib + the two touched test
> targets, ynz-codegen lib, ynz-abi lib, ynz-driver's three named test targets — the parked-31
> debt in untouched files, incl. `jargon_audit.rs`, stays); `ynz-typeck --all-targets` (222 in
> `check.rs` alone, 95 lib, 13 diagnostic_template_parity, rest unchanged); `ynz-codegen --lib`
> (17, incl. 2 new); `ynz-abi` (1); `ynz-diagnostics --all-targets` (unchanged, incl.
> jargon_audit passing as a TEST even though its clippy lint debt is parked); `ynz-driver`
> `v03_m8_channel_close` 31 (2 new), `error_galleries` 10 (1 new phrase, count 26→37 ceiling),
> `integration` 530; `ynz-runtime --lib` 110 + loom 8 (both untouched, confirmed green);
> `ynz-driver --release` rebuilt. Demo golden unchanged (pirates-roster never reads `.message`).
> A newly-discovered, OUT-OF-SCOPE defect is named, not fixed, in the Deviations note below —
> it predates this round and is independent of both producers this round closes.

- **Task + purpose:** implement Phase 1's signed-off design — the explicit close mechanism, the live
  typed channel-closed error, and P2-3's closed-send drop-glue leak fixed through M6's single choke
  point — now loom-instrumented from birth via Phase 3's substrate.
- **Steps**
  1. Confirm Phase 1's sign-off is recorded before starting (hard gate).
  2. Implement the decided close mechanism on `YnzChannel` (whatever architecture change Phase 1's
     design calls for — likely: the object stops treating its retained endpoint as a permanent producer,
     and/or a closed-flag/generation marker gates `send`/`receive` post-close), wired into Phase 3's
     loom-swappable substrate from the start (no retrofit).
  3. Wire `receive()` on a closed-and-drained channel to return ~~the typed channel-closed error (the
     Lock-8 path)~~ **`none` — Phase 1 retyped bare-channel `receive()` to `maybe<T>` (the Lock-8
     error path goes live on `send()` only; the handle's `receive()` keeps `T errors`)** — confirm the
     closed-recv codegen arm's existing "structurally unreachable" comment is removed and replaced
     with the real reachable path, built through the same `maybe<T>` construction every `.exists()`/
     `.value` site reads. Rewrite the 19 bare-channel `receive()` sites / 13 fixture files + the demo,
     gallery, and spec lines the design section enumerates. Extend `close` at all THREE typeck sites
     the design names (`known`; the unconditional receiver/derivable guards; the channel/handle-shared
     unknown-method string, which splits).
  3a. **Ship `.copy()` on `map<K,V>` (Patrick's ruling, 2026-09-03; precondition for 3b's advice):**
     new runtime `ynz_map_clone` (fresh header + `ctrl`/`keys`/`vals`/`insert_order` buffers via
     counted `ynz_alloc`, one-level byte copy mirroring `ynz_array_clone_primitive`), a
     `Type::BuiltinMap` arm in the `PostfixOpKind::Copy` lowering (`emit.rs:19301` — today `map`
     falls to `_ => Ok(recv_val)` and `table.copy()` silently ALIASES), the
     `[[primitive_intrinsic]] copy` entry on `receiver_type = "map"`, an independence fixture in the
     shape of `m5_p5_copy_aos_independent.ynz` — **authored and committed RED (failing on today's
     alias no-op) BEFORE `ynz_map_clone` and the codegen arm land, flipped GREEN in the commit that
     lands them; the RED run is the proof this step happened** — and the `REF-ownership.md:87`
     stale-text fix.
  3b. **Implement the transfer rule (Phase 2 signed design, precondition for step 4 — supersedes this
     step's earlier syntactic-classify shape; every clause below traces to `IMP-ownership.md`
     "Transfer — Who Else Holds This Value," FRAGO 010, signed 2026-09-03):**
     - **Hoist the `effective_ownership` fixpoint above the body check** (`queries.rs`: the fixpoint
       currently runs after `check(...)` at `:~423`, `analyze` at `:503`; it depends only on the
       parse and signature tables, both in hand at `:279–308` — a reorder, not a redesign). Every
       consumer below assumes the report exists before any body is checked.
     - **Add `provenance(expr, bindings) -> Provenance`** to `effective_ownership.rs` — ONE exhaustive
       `Expr` match, no wildcard arm, returning `Fresh`/`Whole(name)`/`Reaches(roots)`/`Unknown` per
       the classification table in `IMP-ownership.md`. No sink inspects syntax directly again.
     - **Add the binding-event function**, called from BOTH `Stmt::Let` (first declaration and
       shadowing) and `Stmt::Assign` (reassignment) — never a re-implementation beside the existing
       `Stmt::Assign` type check at `check.rs:2436–2465`. On `Let`: origin/alias class set per the
       initializer table. On `Assign`: the entry LEAVES its old class (old members keep their state)
       and its `consumed` flag is CLEARED (revive-on-reassign — `eat(rows); rows = [4, 5];
       rows.count()` becomes legal), then joins the new value's class per the same initializer table.
       `for` loop variables get `Cell` origin (`Stmt::For`, unchanged); `for`-destructure names are
       `Let` events with `Reaches(__shape)` provenance, not `Stmt::For` bindings (parked item 25).
     - **`ScopeEntry.is_consumed: bool` → `consumed: Option<ConsumedBy>`** (`Given { callee } | Sent {
       channel }`), `ScopeEntry` gains `origin: Origin` and an alias-class id (both recomputed at
       every binding event, never set once at creation). `Scope::consume(name, cause)` consumes the
       whole class. The six `ScopeEntry` constructors, all in `check.rs` (`:642`, `:1406`, `:1623`,
       `:1660`, `:1723`, `:2803` — parked item 27), and both `!is_consumed` guards (`:1510`, `:4617`)
       are updated to "not already consumed by any cause" — one field, no parallel `Option<String>`.
     - **Add `stmt_rebinds(stmt, name) -> bool`** (true for `Stmt::Assign`/`Stmt::Let`/`Stmt::For`
       binding `name`) and consult it from `effective_ownership.rs`'s walker: a rebinding of the
       tracked name returns `Writes` (correcting the `:330–333` doc comment and the `:379`
       by-value-only classification — the honest extension Phase 2 designed).
     - **Add the two fixpoint facts**: `consumed[fn][i]: bool` (a position is consumed when declared
       `give`, OR the body passes it whole to a consumed position, OR sends it whole on a channel —
       the send case gated on the parameter's DECLARED type, `channel_elem_drop(ty).is_some_and
       (transfers_source)`, never a receiver-type lookup the fixpoint cannot make); `returns_fresh[fn]:
       Freshness` (`Fresh` iff every `return` yields a value nobody else reaches). Both monotone
       false→true in the same loop; imported functions are `MayAlias`/not-consumed.
     - **Add `check_transfer(expr, sink)`** — the ONE emit site for all three transfer diagnostics
       (`ConsumedBySend`, `ParamNeedsGive`, **`TransferNeedsCopy`** — this is the registry/plan name;
       the earlier `SendPayloadNeedsCopy` was never registered or shipped). Wire it to the closed sink
       list: `channel<T>.send(v)` when `channel_elem_drop(T)` is `Some(kind)` and
       `kind.transfers_source()`; every declared-`give` position of every call form (plain call,
       monomorphized generic, UFCS dot-call — receiver AND non-receiver arguments — and `dynamic
       Contract` dispatch) through **one shared call-form/argument-list normalization**
       (`[receiver, args…]`, the same normalization `background_spawn_call_form` and
       `collect_aliasing_in_expr` already perform — Phase 4 makes the three call paths share it
       rather than adding a fourth loop); and the `background` spawn liveness INFERENCE (not a
       `check_transfer` call — class-aware: infer `Give` iff origin `Owned`/`Param(give)` AND no
       class member is read after; else infer `Copy`, and a `Copy` entry is still RECORDED for every
       `Whole(name)` spawn arg, never nothing — parked item 19, the fr23 UAF class). The
       `BgOwnership::Channel` branch (`check.rs:1461–1469`) still claims `channel<T>` arguments before
       this rule sees them (parked item 20).
     - In `check_conduit_method_call`'s `send` arm (`check.rs:4105–4149`), gated on
       `channel_elem_drop(elem).is_some_and(transfers_source)` — `pub enum ChannelElemDrop { Array,
       Map, NumberCell }` + `channel_elem_drop(&Type) -> Option<ChannelElemDrop>` in
       `ynz_typeck::types`, `channel_drop_glue` matching `None => null, Some(kind) => <exhaustive,
       function-valued arms>` (never a bool, never a twin) — call `check_transfer(payload, Sink::Send
       { channel })`; **extract** the const refusal `format!` (`:4602`) into a helper with a
       sink-supplied WHAT-INSTEAD (`{channel}.send({name}.copy())`). **Apply `ParamNeedsGive` at
       `check_arg_ownership`'s Give arm too** (`:4617`'s silent consume of a bare/`lend` parameter
       becomes the error; the `:4611` share refusal is retired into the template — update the
       gallery/snapshot that asserts the old text, `v0_3_m4_errors.ynz` first) — the whole chain is
       reported in ONE compile via `consumed[fn][i]`, never one frame per compile (the earlier
       callee-before-caller-ordering premise was false and is withdrawn). Compile the whole fixture
       corpus and confirm zero existing relay-through-bare-parameter instances, with the one known
       exception `examples/primantis-orders/m6_errors.ynz:112–115` (parked item 17 — update its
       `// WHY:` and the gallery count bound).
     - **`dynamic Contract` dispatch, covered by construction**: `ContractSigDef` (`shapes.rs`) gains
       `param_ownerships` and the receiver kind from the AST's `ContractSig.params`/`ReceiverKind`
       (**parser-optional, exactly as on a function with a body — no parser enforcement is added**;
       `follows` conformance checks EXACT parity, bare matches bare, `give` matches `give`); the
       dispatch site (`check.rs:5391–5421`) builds the same normalized argument list and calls
       `check_transfer` for every `give` position using the contract's declared modifiers. Runtime
       exposure today is nil (codegen refuses every dynamic-dispatch call site, probe-confirmed).
     - Derive `check_channel_construction`'s `elem_supported` (`check.rs:4286–4297`) from
       `channel_elem_drop` or parity-test it. The existing consumed-read site (`check.rs:3622–3631`)
       selects `Consumed`/`ConsumedBySend` by cause; reconcile the pre-existing `Consumed`
       template/code wording drift there (parked items 7/8).
  3c. **`HandleChannelArgNeedsBinding`:** at the handle-form spawn pre-record (`check.rs:2321–2345`),
     a non-`Ident` argument binding the callee's first `channel<T>` parameter is a compile ERROR with
     the design's three-slot text; statement form untouched. Registry template + gallery trigger.
  3d. **fr12 — `channel<number>` decimal128 marshalling (Phase 2 signed design, `IMP-concurrency.md`
     "fr12"; FRAGO 010, signed 2026-09-03):** the typeck gate on `Type::Number { precision <= 34 }` is
     lifted by extending `channel_elem_drop` (from which `elem_supported` derives — never by editing
     the list directly); send lowering copies the sender's 16-byte `i128` into a fresh
     `ynz_alloc(16)` cell through the existing `number_to_heap_cell` helper (`emit.rs:3802`, the same
     one the `background` decimal128 bg-arg path already uses); receive lowering loads the 16 bytes
     into the receiver's own storage and frees the cell before the `maybe<number>` envelope is built.
     Drop glue: `ChannelElemDrop::NumberCell → ynz_number_cell_free` (a named `ynz_free(ptr, 16)`
     C-ABI entry, registry-invisible per the feature-registry carve-out — compiler-internal glue).
     **`number` does NOT join the give set** — `transfers_source()` is `Array | Map => true, NumberCell
     => false`, so typeck consumes on `is_some_and(transfers_source)`, never on `is_some()` alone; a
     `number` channel binding stays usable after `send()`. `shape` elements and bignum `number`
     (precision > 34) stay REJECTED at construction. `modify [[deferred_language_feature]]
     channel-element-heap-upgrade` narrows to those two, with `substitute`/`why`/`triggers` rewritten
     to say decimal128 shipped in v0.3-M8. Retire `examples/primantis-orders/v0_3_m4_errors.ynz:98`
     (the `channel<number>` rejected-construction trigger) and replace it with a `channel<Player>`
     trigger so the error class keeps a gallery instance. Fixture class: send-then-receive round-trip
     (value round-trip through a suspension on the receive side, exercising the two-slot frame path);
     send-then-drop-with-buffered-element; send-after-close (freed via `refuse_closed`, a harmless
     no-op through `release_taken_value` since the cell is never ladder-owned); a parked send drained
     after close — all asserting exact alloc/free parity (`YNZ_ALLOC_COUNTER_OUTPUT`), one counted
     16-byte alloc per send, one counted free per receive.
  4. Fix P2-3 **in the runtime, not in codegen**: `channel_send_poll_guarded`'s **THREE** first-poll
     CLOSED outcomes — `None` sender under the lock; `try_send → Closed` (`channel.rs:503`); `Full`
     then the fresh endpoint future's first poll `Ready(Err)` (`channel.rs:554`) — collapse into ONE
     `refuse_closed` fallthrough that calls `release_taken_value()` then `glue(value)` when
     `drop_glue.is_some()` and returns `CHANNEL_CLOSED` — the same two-step the shipped
     re-poll-CLOSED arm performs; no arm returns `CHANNEL_CLOSED` except through it. Codegen's
     `conduit_closed1`/`conduit_closed2` build the error and free NOTHING (a codegen free of a
     ladder-owned clone + a non-releasing runtime = double free at retire). Flip the shipped doc
     comment ("a CLOSED result on a FIRST poll never releases") and test case (c) of
     `ladder_is_untouched_when_the_channel_does_not_take_ownership` to the new contract; add the
     CLOSED-first-poll alloc/free parity gate for each of the three arms. **Sound only after step
     3b** — without the consume, this free is a use-after-free on the sender's binding. Leave every
     other part of the release protocol (`DriveGuard`, handle-return release) exactly as shipped,
     with ONE exception: `release_ladder_payload`'s kind filter (`runtime.rs:933`) consumes a new
     `ynz-abi` predicate `bg_arg_kind_is_releasable_payload(kind)` (inverted — `!= SHARED_CHANNEL &&
     != RELEASED`) instead of its hand-listed `HEAP_SHAPE`/`HEAP_ARRAY`; add `ALL_BG_ARG_KINDS` in
     `ynz-abi` and a `ynz-runtime` per-kind alloc/free parity test linking the predicate to the
     ladder's free match (`runtime.rs:1125`), whose `_ => {}` arm gains
     `debug_assert!(!bg_arg_kind_is_releasable_payload(kind))`.
  4b. `receive()`'s `maybe<T>` envelope alloca goes through `alloca_in_entry_llvm` (`emit.rs:~2270`),
     hoisted to the entry block — `conduit_post` (`emit.rs:12785`) is inside the consumer's `while`
     body, so an insertion-point alloca grows the frame per iteration. `build_maybe_some`
     (`emit.rs:2382`, `#[allow(dead_code)]`) is not the model.

     **CHECKPOINT** — close mechanism + live error path + P2-3 fix all implemented; fixture authoring
     (next steps) not yet started.
  5. Author the RED→GREEN fixture class (per R1's mitigation): explicit close then receive-drains-then-
     `none`, double-close idempotency, drop-without-close (confirm this still behaves per the PRE-close
     behavior for any channel never explicitly closed — no regression), concurrent send-during-close
     (asserting the design's linearization — a send already holding its sender-lock clone LANDS; do
     not assert "refused after `close()` returns"). **Plus the owned-heap element class that does not
     exist for ANY element kind today** — one fixture each for `channel<array<int>>` and
     `channel<map<string, int>>`: send-then-receive round-trip, send-then-read (the `ConsumedBySend`
     compile error), send-after-close (payload freed via the glue, alloc/free parity via
     `YNZ_ALLOC_COUNTER_OUTPUT`), drop-with-buffered-element. **The map fixture covers BOTH shapes as
     correct programs**: a map built INSIDE the sending task, and a map passed as a **`give`** bg arg
     and sent (the `give` consumes the parent's binding at the spawn through `check_arg_ownership`;
     the un-cloned alias has one holder; alloc/free parity through retire) — the fix-round-3 guard
     closed FR#9's channel-door instance; only the `bucket.add` container door stays RED-pinned in
     `bg_arg_alias_container_add_red.ynz`. **Plus the ownership-flow class**: the reviewer's `wait
     producer(wire, rows)` program with `give` (parent's later read is the use-after-give error) and
     without (`ParamNeedsGive`); a two-hop relay `A → B → C` correct with `give` on both, and the
     error naming `B` when `B`'s parameter lacks it — both A's and B's errors in one build when A
     relays its own bare parameter; the three refused payload forms (a field, an index/loop-cell, a
     call result that returns a piece of its argument → **`TransferNeedsCopy`**, the registry/plan
     name — `SendPayloadNeedsCopy` was never registered or shipped) beside the admitted forms (an
     owned ident, `.copy()`, a literal of fresh elements, a constructor call, a `returns_fresh` call
     result bound by `let`). **Plus the eight probes of 2026-09-03 as gallery/fixture triggers**
     (`.claude/corpses.md`, `git log --grep=m8-p2`): `bucket.stash(rows)` UFCS non-receiver give;
     `eat(bucket.rows)` field; `for (row in matrix) { wire.send(row) }` loop cell; `let other = rows;
     wire.send(rows)` alias-by-`let`; `rows = other; wire.send(rows)` alias-by-**reassignment**; a
     shadowing `let rows = other; wire.send(rows)` alias-by-**shadow**; `let rows = pick(bucket);
     eat(rows)` returns-a-piece; `[a]` nested-literal `Reaches`; `dynamic Contract` give (typeck
     rejects, codegen ICEs — zero runtime exposure, a compile-time-only trigger). **Plus
     revive-on-reassign as a correct-program fixture**: `eat(rows); rows = [4, 5]; rows.count()`
     compiles and prints `4 5` (today it is wrongly refused "already given away" — the binding-event
     rule fixes this by construction; FRAGO 010, signed 2026-09-03). Plus the four-diagnostic gallery triggers
     (`ConsumedBySend` for array AND map; `ParamNeedsGive` at a send AND at a give-parameter call;
     `TransferNeedsCopy`; `HandleChannelArgNeedsBinding`). fr12's fixture class is step 3d's.
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
     load-bearing codegen mechanism):** on a minimal throwaway fixture, implement **topology (B)**
     (`IMP-ownership.md` "Auto-Arc," signed 2026-09-03, FRAGO 010): one `ynz_arc_new(struct_bytes)` +
     memcpy at the FIRST spawn of a group, held in a **caller-side transient**; `ynz_arc_clone` at
     every member spawn including the first (the transient's own reference is separate);
     `ynz_arc_free(transient)` immediately after the LAST member spawn statement of the group; each
     task's argument-drop ladder releases its own reference through a new `BgOwnership::Arc { group,
     first, last }` recording → `BG_ARG_KIND_ARC_SHAPE` ladder arm calling `ynz_arc_free(ptr, size)`.
     Reuse `EffectiveOwnership::Reads` for the task-side proof, and confirm (a) refcounts balance
     under a single spawn+join (`new`=1, N clones → N+1, transient released → N, tasks retire → 0),
     and (b) the emitted calls interact correctly with the frame-layout/state-machine embedding used
     for `background` spawns (no aliasing violation against existing `noalias`/`readonly` LLVM
     attributes M7's Phase 1 audit already confirmed on `ynz_arc_*`). GREEN/RED verdict before
     extending to the full codegen path. [plan defect, found Phase 5: M7's FRAGO 001 records that no
     `ynz_arc_*` symbol was declared or called from codegen; Phase 5 declared them first, bare like
     every sibling runtime decl] (FRAGO 013 — Phase 5 round 2)
  3. On GREEN, extend the emission to every Arc-eligible spawn boundary per Phase 2's decided
     beneficial-emission condition (FRAGO 010, signed 2026-09-03), checked at the statement-form
     liveness pass (`check.rs:1443–1515`):
     ALL of — **≥2 spawn statements in one block pass the same whole binding, no suspension between
     the first and the last** (a suspending statement between them ends the group; **"caller + 1
     task" is explicitly OUT** — the caller reads its own original, the one task reads its copy, which
     is the shipped one-copy path, and an Arc would add a header and atomics for nothing); task-side
     `report.ownership_of(callee, position) == Reads` on every member (`Unknown`/`Writes` declines the
     whole group); caller-side `classify_binding_in_stmts(v, stmts between first and last spawn) ==
     Reads`, where **`stmt_rebinds(v, stmt)` (`IMP-ownership.md`'s honest walker extension) makes a
     rebinding `Writes`** — EXCEPT a rebinding at the TOP LEVEL of the spawns' own block (`v = …`, a
     shadowing `let v`, a `for (v in …)`) is a **GROUP BOUNDARY, not a decline**: the group closes at
     the last spawn before it and a new group opens after it, each judged on its own member count; a
     rebinding INSIDE a nested block (`if (…) { v = other }`) IS `Writes` and declines the enclosing
     group (path-dependent, no statically placed block is right on both paths); and **`arc_shareable
     (ty)`** — a new predicate in `ynz_typeck::types`, a `shape` whose fields are transitively
     `int`/`float`/`bool`/`string`/inline `shape` (`number`, `array`/`map`/`maybe`/union fields, and
     every non-shape type are excluded — the residual below). Loops are out (one spawn statement, no
     group). For the single-reader/failing-condition case, confirm the existing `.copy` path is
     UNCHANGED (no regression to the correct-and-cheaper existing behavior) — nothing changes when any
     condition fails.

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
     loom coverage, if any). **Decide and record `bg_arg_kind_is_releasable_payload(BG_ARG_KIND_ARC_SHAPE)`
     (packet item (h), signed 2026-09-03 — packet item (h) assigned the decision here; it did not
     answer it)**: the design's reasoning (`IMP-ownership.md` §"What typeck records and what codegen
     reads") points to `false` — leak-one-count is conservative, never free-early — but Phase 5 must
     record its own decision with evidence rather than inherit an unruled answer; add the per-kind case
     to Phase 4's `ALL_BG_ARG_KINDS` parity test.
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

> ### Phase 5 STATUS — steps 1–8 ✅ EXECUTED 2026-09-04 (executor `m8-p5-20260904-a1`; nothing committed) — pending review
>
> - **Step 1** ✅ Phase 2's SIGN-OFF record (`audit.md` "SIGN-OFF — 2026-09-03 — Patrick signed off Phase 2") and R2's signed override (¶1) confirmed before step 2.
> - **Step 2 spike: GREEN.** Fixture `m8_arc_two_spawn_group.ynz` (promoted to the first real fixture, not deleted). RED baseline: correct output, ZERO `ynz_arc_*` calls in IR. After: 1 `ynz_arc_new` / 2 `ynz_arc_clone` / 1 `ynz_arc_free` (the transient) in the IR; (a) alloc=6 free=6 versus the one-spawn program's 4/4 — one counted Arc block replaced two per-task copies (+2 not +3), non-vacuous; (b) every `ynz_arc_*` call sits in basic block `sm_s0` before the first `ynz_channel_recv_poll` suspension, `%arc_new` is an SSA local never stored to a frame slot, the spawned task's data pointer rides the frame slot exactly like a heap copy, and the callee's `ptr noalias readonly` attributes describe a task that only reads (proven `Reads`). R2's "trigger to revisit" is answered: the frame/spawn-boundary interaction is clean by construction (a group never straddles a suspension), evidence toward a future re-score — not self-scored here.
> - **Step 3** ✅ the full condition: `admit_arc_group_for` (`check.rs`) — ≥2 spawn statements passing the same whole binding, group boundaries at a top-level rebinding / a conservatively-judged suspension / **a reachable `return` (one boundary added beyond the signed text — recorded in `IMP-ownership.md`)**, task-side `ownership_of == Reads`, caller-side `classify_binding_in_stmts == Reads`, `arc_shareable` (`types.rs`; floor = `int`/`float`/`bool`/`string` — **nested `shape` fields EXCLUDED because the tree stores them as pointers**, a design-text correction recorded in `IMP-ownership.md`). Both spawn forms, both runtime arms. One-spawn no-op: the pre-change IR of `m8_arc_one_spawn_noop.ynz` (sha256 `4a812358…`) is byte-identical after the change, achieved by declaring `ynz_arc_*` lazily on first use. Packet item **(h) decided: `bg_arg_kind_is_releasable_payload(BG_ARG_KIND_ARC_SHAPE) = false`**, pinned by the runtime's per-kind parity test staging the Arc kind with a live co-owner. Parked 16 fixed via the ONE `record_spawn_arg_ownership`.
> - **Step 4** ✅ `auto_arc` fires (`ynz_typeck::auto_arc_hints` → `inlay_hint.rs` domain 11, normal muted style, comment-style `// shared by reference count with {n} tasks — read-only`, reading the same `BgOwnership::Arc` record codegen emits from); three LSP tests.
> - **Step 5** ✅ `m8_arc_hammer_shared_shape.ynz`: four suspending tasks × 25 interleaved iterations reading one shared block (105000), alloc=free.
> - **Step 6** ✅ parity gates in `v03_m8_auto_arc.rs` (IR call counts + alloc==free + the +2-not-+3 delta); loom model `loom_arc_group_clone_per_task_then_transient_release_frees_exactly_once` (152 interleavings, exhaustive) driving the REAL `arc.rs` with its header atomic swapped through `crate::sync`; revert-proof: dropping the second task's `ynz_arc_clone` fails by assertion ("freed while this task still holds a reference", left 1 / right 0), tree restored from a saved copy (sha `b1628457…` before and after).
> - **Step 7** ✅ `auto-arc-codegen-emission` NARROWED to the design's residual (suspension-straddling groups, loops, arrays/maps and shapes with `number`/pointer fields); `auto_arc` hover text updated to the signed wording; `auto-arc-cautionary-tint` untouched.
> - **Step 8** — the affected lane run green by crate and test name (see the `audit.md` entry); the full suite is the gate agent's lane.
> - **Demo/gallery:** `pirates-roster` gained `m8_arc_demo` (three scouts, one shared file; golden regenerated byte-exact); no new compile diagnostic, so `m8_errors.ynz` is unchanged.
> - **Out of scope, recorded:** parked item 40 — a shape with a `number` field SIGSEGVs on ANY `background` spawn today (pre-existing copy-path bug, zero Arc involvement).
>
> **Round 1 grading (conductor, 2026-09-04).** Executor output SEALED at `0f62869` BEFORE any seat
> (corpse #3); a Haiku fix round (`m8-p5-fix1-20260904`, sealed `561476a`) answered the gate's two
> NEW reds — `arc_strong_count` dead under a `--cfg loom` non-test build; the Arc harness's
> counter file keyed by process pid, racing two tests on one fixture with an `unwrap_or("alloc=0")`
> mask — and the re-gate ran 5×11/11. `green-check-medium` (Fable — high/mechanical reads Opus
> medium, R2's residual) → green on every lane (alloc==free 6/6 non-vacuous direct run, loom 9,
> driver 11/31/10/530/7, typeck/codegen/lsp/registry, Miri on `arc::`); `--all-targets` red only on
> parked-31 files (two more `ynz-lsp` `cfg(test)` imports added to 31). **CHECKPOINT after step 3
> WAIVED by the conductor's dispatch** ("stop there only if you cannot finish in one segment") —
> recorded here per `plan-adherence`. Seats on `58ca95e..561476a`: `ux-low` (Haiku) → 0 blockers,
> 2 should-fix (the word "spawn" in the `auto_arc` hover and the deferral's `substitute`/`why` —
> banned, the Yinz word is `background`); `doc-auditor-high` (Sonnet) → 0 blockers, 3 should-fix
> (six stale `check.rs:NNN` cites in the Auto-Arc section, 500–650 lines off — the parked-21 class;
> `inlay_hint_passes.rs:18-24`'s module doc still says "no emission yet";
> `IMP-no-function-coloring.md:60` contradicts the line above it), 2 minor; `plan-adherence-medium`
> (Fable, six suspension probes of its own, IR re-read) → 0 blockers, no `frago-needed`, all five
> deviations risk-neutral, R2 stays HIGH under the override (a GREEN spike is evidence toward a
> re-score, never a self-score — the plan's own text), 5 should-fix (step 8's full suite is Phase
> 9's gate by design — recorded; the FR#5 placeholder the plan says Phase 5 replaces was not
> replaced; a false M7-attribute premise at `plan.md:~260,~290,~1644` — M7's own FRAGO 001 says no
> `ynz_arc_*` was ever declared from codegen; the audit's "all recorded in `IMP-ownership.md`" is
> false for deviation (3); the waived checkpoint), 5 minor; `code-reviewer-medium` (Fable, probed)
> → **1 BLOCKER, R2's class**: `admit_arc_group_for` classifies the caller side over
> `&stmts[first + 1..last]` — strictly BETWEEN member spawns — so a `lend` write inside a member's
> own later argument (`background render(scene, results, bump(scene))`) is invisible; the group is
> admitted, `ynz_arc_new` copies before `bump` runs, task 2 reads stale bytes (`task saw 6 / 6 /
> caller keeps 106` vs the `.copy()` control's `106 / 6 / 106`) — the fix is the range
> (`first..=last`), the walker's `Call` arm already returns `Writes` for a `lend` position; 1
> should-fix (`declared_writes_from_sigs` re-derives the map `queries.rs` already builds — the
> twin shape), 2 minor; every refcount path (panic, cancel, transient placement, early return)
> traced clean, (h)'s pin genuinely pins. `test-quality` DEFERRED to after the fix (grades the
> shipped set, alone, on a sealed tree). Fix round 2 answers `red:code-reviewer`.
>
> **Round 2 grading (fix round `m8-p5-fix2-20260904`, Fable medium, sealed `9af3883` BEFORE any
> seat; conductor, 2026-09-04).** The range fix landed with a RED-reproduced fixture (task 2 read
> stale bytes with 4 `ynz_arc_*` calls → declined, 0 calls, correct output); the executor
> corrected round 1's own finding on the record (a write in a member's LATER argument does not
> diverge — arg 0 is snapshotted first on both paths; the FIRST member's list is the divergent
> case); every should-fix from all four seats landed, plus one the round found itself — the LSP
> hint walker never visited the handle form, so parked 16's `give` label had no reader.
> `green-check-medium` (Fable) → green on every lane (typeck all targets incl. the new
> `suspends_parity_tests` with its floor asserted, codegen, lsp 28, driver 12/31/10/530/7,
> registry, runtime 110 + loom 9, release builds, the fixture's direct run and 0 `ynz_arc_` in its
> IR, gitleaks); `--all-targets` red only on parked-31 files. `code-reviewer-medium` (Fable,
> probed both member orders) → 0 blockers, both shapes decline with correct output, the twin is
> gone, the predicate walks the right side; 1 should-fix + 1 minor parked (42).
> `test-quality-medium` (Fable, ALONE, restoring from saved copies) → **0 blockers**, all five
> revert-proofs caught (a dropped clone crashed the compiled program; a skipped release showed as
> `alloc 6 != free 5`; (h) flipped failed the parity test; the old range re-admitted the group;
> `number` admitted failed the unit pin), 2 should-fix + 5 minor parked (43, 44). **Phase 5
> terminal state: CLEAN. Exit criteria MET. R2 stays HIGH under the signed override — the GREEN
> spike is evidence toward a re-score, never a self-score. Boundary commit on Patrick's standing
> go; frontier → Phase 7 once the parked-40 hotfix (FRAGO 012, own worktree, in flight) merges
> back.**

#### Phase 6 — P2-7: `ynz_handle_recv_poll` Panic-Then-Pending Hang — ❌ **RETIRED, FRAGO 001 (2026-09-03)**

> **FRAGO 001 — Phase 6 is RETIRED as already-satisfied. Do not execute it.**
>
> **Finding:** M6 did not defer P2-7; it un-deferred it under its own FRAGO 010 and shipped the fix as
> M6 Phase 4b, commit `b0cdbd3` ("fix(runtime): close ynz_handle_recv_poll panic-then-Pending hang
> (M6 P4b)", 2026-07-11), inside PR #82. This plan's ¶1 Terrain claim that P2-7 was "NOT fixed by M6"
> was written before that FRAGO landed and is stale.
>
> **Confirmed independently, not self-graded.** Phase 0's executor surfaced the finding; a separate
> `code-reviewer-medium` confirmed it adversarially rather than on the commit subject line:
> `record_recv_waiter(cx.waker())` is now the first statement inside the `catch_unwind` closure
> (`handle.rs:354`), before `poll_recv` (`:355`), so the exact panic-before-registration window from
> the audit's P2-7 report is closed; `handle::tests::handle_recv_poll_registers_waiter_before_polling`
> and `handle::tests::completion_wakes_receiver_after_panic_before_slot_registration`
> (`handle.rs:724`, `:798`) lock it, **proven revert-sensitive** — the reviewer mechanically swapped
> the ordering back to poll-first and both tests failed on the P2-7 hang assertion (`wakes == 0`),
> then restored the tree clean. M6's "no lock held across a blocking poll" invariant holds in the
> changed code (`recv_waiters` released before `outbox_rx` is taken; never nested). The handle-side
> and channel-side fixes are a genuine structural mirror.
>
> **Authority:** Patrick, 2026-09-03, directing "verify first, then retire" — the retirement is
> conditional authority that the confirmation above discharged.
>
> **Consequences applied in this same amendment:** risk row **R6 is retired** (its hazard no longer
> exists in this plan's scope); the ¶1 Terrain P2-7 bullet and the Design-Doc Alignment §4
> milestone-boundary claim are annotated as superseded; the Invariants → Safety P2-7 assertion is
> annotated as satisfied-by-M6 rather than owed by this plan. This milestone is now **nine phases**
> (0–5, 7–9); phase numbers are NOT renumbered, so every existing citation and `Plan-Phase:` trailer
> stays valid.
>
> **Two residuals inherited from M6, deliberately NOT absorbed here** (both already carry fielded
> deferrals in the roadmap's own `audit.md`, so neither is silent duct tape): the handle-side panic
> log message drops the panic payload string because `panic_payload_msg` is private to `channel.rs`
> (cosmetic, log text only); and the `recv_waiters`/`record_recv_waiter`/`wake_recv_waiters` trio is
> duplicated byte-identically between `channel.rs` and `handle.rs`. Recorded in this plan's Future
> Requirements as item 8 so a reader of THIS plan finds them without archaeology.

<details>
<summary>Original Phase 6 text, retained verbatim for the record (superseded — do not execute)</summary>


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

</details>

> ### Phase 7 STATUS — steps 1–2 ✅ recon done, **CHECKPOINT reached and the decision made: Branch B (RE-DEFER)**, step 4's four-field re-deferral AUTHORED 2026-09-04 (executor `m8-p7-20260904-a1`; nothing committed) — **AWAITING PATRICK'S SIGN-OFF** (step 4's own gate); step 5 (full suite) is the gate agent's lane
>
> - **Step 1** ✅ the recon question answered with evidence, not assumed: **no generic scope-exit drop dispatch exists for ANY droppable type.** `emit.rs` emits `ynz_array_drop`/`ynz_free`/`ynz_channel_free` only in task-side arg-free glue (`emit_bg_arg_frees`), the channel element-glue table (`channel_elem_drop`), and the spike trampoline's staged cell; `free_frame` frees a suspending callee's frame bytes with no per-slot walk; `ynz_handle_free` has zero call sites (IR read). The ONE choke point Phases 4/5 wired through — `SpawnStateFnFuture::drop`'s kind arms — is the CHILD task's retirement ladder over the CHILD's frame.
> - **Step 2** ✅ extending that ladder to handle bindings is NOT clean: scope exit is the PARENT's event on the PARENT's frame (block end / loop back-edge / `return` / the caller's `free_frame`), and the parent has no ladder unless it is itself a task — and even then the ladder fires at retirement, not scope exit. Ten probes (`audit.md` entry `m8-p7-20260904-a1`): P2/P4/P6/P9/P10 all show the child completing past the scope exit; P1/P1b show `ynz_rt_shutdown` stopping a still-running child at its next suspension when `entrypoint` returns.
> - **Step 4 (Branch B)** ✅ four fields in Future Requirements #3; `background-handle-cancel-injection` UPDATED (not retired) — `substitute`/`why`/`triggers` rewritten to the concrete finding, no milestone tags or internal paths, `ships_in = "a later version"`; `IMP-no-function-coloring.md` "Task Cancellation" amended to current state + the recon record + anchor; `IMP-concurrency.md`'s auto-close deferral carries the ruling that its drop-pass dependency is NOT satisfied by M8. Current state pinned loud in-suite: `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs` (2 tests, planned-RED inverse — they flip when the trigger fires), fixture `v0_3_m8_p7_handle_scope_exit_pin.ynz`.
> - **Step 3 (Branch A)** — not taken. **Step 5** — the affected lane ran green (the two pin tests, registry tests); the full suite is the gate agent's lane.
> - **Demo/gallery:** no executable surface and no diagnostic shipped, so `pirates-roster` and `m8_errors.ynz` are unchanged (N/A with reason, per the invariant).
> - **Sign-off needed from Patrick:** the re-deferral text (FR #3) and the registry entry's rewritten fields. Resume-at `phase-7/step-3` per the dispatch brief (the plan numbers Branch B as step 4; the seam is the same — the sign-off gate before the phase closes).

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
     time diagnostic this milestone adds — from Phases 1 and 2's signed design (FRAGO 010): the **use-after-send**
     diagnostic (`ConsumedBySend`: send an `array<int>` binding into a channel, then read it — and the
     same for a `map<string, int>` binding, so the gallery proves the `.copy()` advice is executable
     for both element kinds), **`ParamNeedsGive`** (a sent or given-away parameter whose function does
     not declare `give`, at both a send and a give-parameter call), **`TransferNeedsCopy`** (the
     signed name — supersedes the earlier `SendPayloadNeedsCopy` — a field, an index/loop-cell, and a
     call-result-that-returns-a-piece, beside the `const`-binding-sent and share-param-sent refusals
     it now covers), **`HandleChannelArgNeedsBinding`** (`let h = background doubler(makeWire())`),
     and `.close()`-with-arguments; plus `.close()` on a HANDLE (the split unknown-method list must
     read `send(value), receive()` there and `send(value), receive(), close()` on a channel); plus any
     new diagnostic Phase 7's Branch A might add, if it ships. Wire the new gallery's assertions into
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
  - Explicit close, then full drain, then `receive()` returns ~~the typed channel-closed error (the
    previously-dead Lock-8 path)~~ **`none` (Phase 1 decided `receive()` is `maybe<T>` on a bare
    channel — the Lock-8 error path goes live on `send()` only; the handle's `receive()` keeps its
    `T errors`)** — never blocks, never panics.
  - **`send()` on an owned-heap element type (`array<T>`/`map<K,V>`) consumes the sent binding** — a
    read of the binding after the send is a compile error (`ConsumedBySend`); a `const` binding
    cannot be sent without `.copy()`; **a PARAMETER binding cannot be sent unless its function
    declares it `give`** (`ParamNeedsGive`), and the same rule holds wherever a parameter is passed
    to a `give` parameter (`check_arg_ownership`'s Give arm — the relay hole at `check.rs:4617`), so
    the consume reaches every caller's binding through the existing give call-site path at every
    call AND every `background` spawn — reported for EVERY frame of the relay chain in one compile,
    never one frame per compile; **a payload that is not a named binding, a `.copy()`, or a
    literal cannot be sent** (`TransferNeedsCopy` — the registry/plan name; `SendPayloadNeedsCopy`
    was Phase 1's draft name and was never registered or shipped). **Alias classes close the
    by-`let`, by-reassignment, and by-shadow blind spots (FRAGO 010, signed 2026-09-03,
    `IMP-ownership.md` "Binding events, origin and alias classes"):** every binding event (`let`, shadowing `let`,
    reassignment, `for`, parameter) puts the name in the class of the value it now denotes,
    consuming any member consumes all — `let other = rows; wire.send(rows); other.count()`,
    `rows = other; wire.send(rows); other.count()`, and a shadowing `let rows = other;
    wire.send(rows)` are now `ConsumedBySend` on the alias read, where all three compile and print
    the un-consumed alias's count on today's tree. **A reassignment REVIVES a consumed name** — the
    entry leaves its old class and its `consumed` flag is CLEARED on `Assign`, so
    `eat(rows); rows = [4, 5]; rows.count()` is a CORRECT program (today it is wrongly refused
    "already given away" — this is a false-error fix, not a new restriction). The buffered value
    therefore has exactly one SOURCE-LEVEL holder at every moment (owner → channel → receiver) for
    any payload reached through named bindings and `give` parameters — a task-built value, a
    heap-CLONED bg arg (whose ladder slot the shipped runtime release protocol flips to `RELEASED`
    at the send), or an ALIASED bg arg (`map`, `array<pointer-elem>`, union) that arrived through a
    `give` parameter. Outside the guarantee: FR#9's container door (`bucket.add(rows)`, stays
    RED-pinned) and the FR#10 `.copy()`-alias-no-op types (which provenance classifies `Unknown`, so
    they cannot be transferred at all). This is the soundness precondition for the P2-3 closed-send
    free (now in the runtime's CLOSED-first-poll path, all three arms, per the design) AND closes two
    live holes on the current tree (probe 2026-09-03: `wire.send(rows)` then `rows.count()` compiles
    and runs today; and `wait producer(wire, rows)` with a bare parameter would have made the first
    hole a use-after-free — the fix-round-3 BLOCKER).
  - **`number` (fr12) stays copy-through, never joins the give set** (FRAGO 010, signed 2026-09-03,
    `IMP-concurrency.md` "fr12"): `send()` on a `channel<number>` mints a fresh 16-byte
    `ynz_alloc`'d cell for the payload — the sender's own `number` binding is untouched and stays
    usable, exactly like `int`. `ConsumedBySend`/`ParamNeedsGive`/`TransferNeedsCopy` never fire on a
    `number` channel.
  - **The typeck consume and the shipped runtime release protocol BOTH stay** — they answer
    different questions (source readability vs. ladder ownership) and are linked by the one
    `ChannelElemDrop` enum plus per-element-kind fixtures asserting BOTH the compile error AND exact
    alloc/free parity through task retire. Phase 4 removes neither and adds no codegen-side ladder
    edit. Per `IMP-concurrency.md` "Two mechanisms, one rule". Primitive and `string`
    channels are unchanged — every existing `ch.send(v)` fixture stays green. Locked by the new
    owned-heap fixture class (Phase 4), which does not exist yet for ANY element kind.
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
- **P2-7 is fixed via register-before-poll — ✅ ALREADY SATISFIED BY M6, NOT OWED BY THIS PLAN**
  (FRAGO 001, 2026-09-03). The assertion below still holds as a true statement about the tree; what
  changed is WHO discharged it. M6 Phase 4b (`b0cdbd3`) registers the waker before the poll body at
  `handle.rs:354`, locked by two revert-sensitive tests (`handle.rs:724`, `:798`) and re-verified
  against the no-lock-across-blocking-poll invariant. This plan inherits the guarantee and writes no
  code on that path. Original assertion, retained:
  ~~(Phase 6)~~ — the `ynz_handle_recv_poll` panic-then-pending
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
  is the OVERRIDE (`.copy()` at the spawn site, avoiding Arc entirely — no body-level `.give` exists;
  `PostfixOpKind` is `Copy | Freeze` only, corrected at Phase 2 sign-off, `IMP-ownership.md` "Override
  directions"; FRAGO 010, signed 2026-09-03) — a different mechanism from "rewrite to the stricter
  form" — so no lint rule name is minted for this feature.
- **Hover WHAT/WHAT-INSTEAD/WHY:** drafted at Phase 2 step 5 (updating the `auto_arc` domain's existing
  placeholder hover text to match Phase 2's decided topology), confirmed live at Phase 5 step 4. WHAT
  (the value is Arc-shared because ≥2 readers share it post-spawn); WHAT-INSTEAD (write `{name}.copy()`
  at the spawn site to force an independent copy instead); WHY tied to the ACTUAL call site's reader
  count, per Golden Rule 11's "specific and contextual" requirement — never a generic "avoids
  allocation."
- **Override directions (per auto-promotion.md "Override Patterns — Consider Both Directions"):**
  **force-the-other-pick** has a real use case (a caller wanting an independent copy despite ≥2 readers,
  e.g. to avoid the Arc header/atomic-op cost on a cold path) and is handled by EXISTING, ALREADY-
  TYPEABLE syntax — **`.copy()` at the spawn site, `.copy()` only** (there is no body-level `.give`
  syntax in Yinz — `PostfixOpKind` is `Copy | Freeze`; the earlier text naming `.give` as a spawn-site
  override was corrected at Phase 2 sign-off, `IMP-ownership.md` "Override directions": the give
  direction is already what the liveness inference does for a binding unused after the spawn, so
  there is nothing to force there) — `.copy()` is `Provenance::Fresh`, not `Whole`, so that spawn is
  not a group member and takes the shipped per-task copy path; no new API needed, documented at
  Phase 2 step 4.
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
  the same three-part format — decided and drafted at Phase 1 step 6. **Phase 1's decision: send-after-
  close is NOT compile-checkable (runtime typed error). FOUR new compile-time diagnostics: use-after-send
  (`ConsumedBySend`)** — a read of an `array<T>`/`map<K,V>` binding after it was sent into a channel;
  **`ParamNeedsGive`** — a parameter given away (sent into an owned-heap channel, or passed to a `give`
  parameter) whose function does not declare it `give` (fix round 3; replaces the share-parameter
  refusal at `check.rs:4611`; its WHY tells the user the `give` word travels up the call chain);
  **`TransferNeedsCopy`** (Phase 2's signed name; the Phase 1 draft's `SendPayloadNeedsCopy` was
  never registered or shipped) — a payload at ANY transfer sink (a send, or a `give` position of any
  call form) that someone else still reaches — a field, an index, a loop cell, a literal built from
  named values, or a call that returns a piece of its argument; **and `HandleChannelArgNeedsBinding`**
  — a handle-form spawn whose first channel argument is not a named binding (the `no-duct-tape` guard
  on the `background-handle-close` deferral; fix round 2). `ConsumedBySend` and `ParamNeedsGive`'s
  full WHAT/WHAT-INSTEAD/WHY text lives in [`IMP-ownership.md`](../../../../docs/internal/implementation/IMP-ownership.md)
  "Teaching text" (Phase 2 signed); `HandleChannelArgNeedsBinding`'s stays in `IMP-concurrency.md`
  "Teaching text" (Phase 1 signed). All four are drafted in full three-slot text, with gallery worked
  examples.
  `ConsumedBySend` is drafted in full WHAT/WHAT-INSTEAD/WHY in `IMP-ownership.md` "Teaching text" —
  the one home for all three transfer diagnostics, per the paragraph above (FRAGO 010, signed
  2026-09-03 — packet item (d), one home)
  (WHAT names the binding and the channel; WHAT-INSTEAD is the copyable `{channel}.send({name}.copy())`
  — executable for BOTH element kinds because `.copy()` ships on `map<K,V>` in Phase 4 step 3a — or
  "put this line above the `send()`"; WHY is the two-tasks-one-value reason with no internals, and it
  states the real behavior — the compiler refuses to build the read; nothing is "empty" at runtime —
  the earlier "is empty afterward" wording was a doc-audit BLOCKER and is gone). Emitted
  from the existing consumed-read site (`check.rs:3622–3631`) by consumption cause, never from a
  second read-check; registered as a `[[diagnostic_template]]`; confirmed live at Phase 4; in the
  Phase 9 gallery. `receive()`'s end-of-stream has NO teaching string — it returns `none`, which the
  user already knows how to read from every other `maybe<T>`.
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

- **Channel-close's new method — DECIDED at Phase 1 (2026-09-03): `[[primitive_intrinsic]]`
  `name = "close"`, `kind = "method"`, `receiver_type = "channel"`, `return_type = "nothing"`.** Phase 4
  step 7 adds it. Phase 1 also found `send`/`receive` were never registered (a pre-existing SSOT gap):
  Phase 4 back-fills both with this design's types — `receive` `return_type = "maybe<T>"`; `send`
  `kind = "method_1arg"`, `param_types = ["T"]`, `return_type = "nothing errors"` — and adds ONE
  optional schema field, `param_ownership` (list aligned with `param_types`; absent = today's
  semantics), set to `["give"]` on `send`, so the LSP hover derives "send gives its value" from data.
- **`[[deferred_language_feature]]` ×2 (new):** `channel-auto-close-on-last-producer` and
  `background-handle-close` — both with four fields carried verbatim from the design section.
- **`[[primitive_intrinsic]]` `name = "copy"`, `kind = "method"`, `receiver_type = "map"`,
  `param_types = []`, `return_type = "map<K, V>"`, `since = "v0.3-M8"` (new; Patrick's ruling
  2026-09-03, FRAGO 004 ruling 4).** Phase 4 step 3a adds it alongside the runtime `ynz_map_clone`
  and the codegen arm — same shape as the existing `array` (`features.toml:1987`) and `fixed`
  (`:2153`) `copy` entries.
- **`[[diagnostic_template]]` ×4 (new; corrected list, Phase 2 signed 2026-09-03, FRAGO 010):
  `ConsumedBySend`**
  — the use-after-send compile diagnostic; **`ParamNeedsGive`** — a parameter given away without a
  `give` declaration (fires at the send arm, at `check_arg_ownership`'s Give arm, and at every
  `give` position of every call form incl. `dynamic Contract` dispatch; retires the inline share
  refusal at `check.rs:4611–4616`; the whole relay chain reported in ONE compile); **`TransferNeedsCopy`**
  (Phase 2's signed name — supersedes the Phase 1 draft's `SendPayloadNeedsCopy`, which was never
  registered or shipped) — a payload at ANY transfer sink (not only a send) that someone else still
  reaches; **and `HandleChannelArgNeedsBinding`** — the handle-form channel-must-be-a-binding guard
  (Phase 1, fix round 2; parked item 11 taken as a guard). This resolves the "possible but not
  certain" item at the end of this subsection: the RUNTIME channel-closed message stays out of the
  registry (per-op codegen string, not a `DiagnosticKind`); the new COMPILE diagnostics go in.
- **`[[deferred_language_feature]]` `name = "maybe-move-out"` (new, Phase 2 signed 2026-09-03, packet
  item (g), FRAGO 010):**
  a consuming move-out-of-`maybe<T>` accessor, deferred with its four fields carried verbatim from
  `IMP-ownership.md` "What this makes sound, and what stays outside." Phase 4 does not add it; a
  relayed received value pays `.copy()` this milestone.
- **`modify [[deferred_language_feature]] name = "channel-element-heap-upgrade"` (fr12, Phase 2
  signed 2026-09-03, FRAGO 010):** narrows to `shape` elements and bignum `number` (precision > 34) only —
  `substitute`/`why`/`triggers` rewritten to state decimal128 (`number`, precision ≤ 34) shipped in
  v0.3-M8 via the send-minted `NumberCell`. No new `[[primitive_intrinsic]]` for fr12 (the element
  type widens; `send`/`receive` are unchanged); the runtime gains `ynz_number_cell_free` as a
  registry-invisible compiler-internal C-ABI entry (carve-out, per `.claude/rules/feature-registry.md`).
- **`auto-arc-codegen-emission`** (existing entry): retired if Phase 5's shipped emission covers the FULL
  beneficial-emission condition Phase 2 decides, OR narrowed to name the real remaining residual if
  Phase 2's topology leaves a bounded slice unimplemented (Phase 5 step 7) — the residual Phase 2
  already named: spawn groups straddling a suspension point, spawn groups inside a loop, and
  `array`/`map` values and shapes with pointer-cell or `number` fields (`arc_shareable`'s floor) —
  mirroring the `ec-wrapper-collect-on-completion` retirement-note convention.
- **`modify [[muted_hint_domain]] auto_arc` hover text (FRAGO 010, signed 2026-09-03):** the
  placeholder hover text is replaced with Phase 2's decided-topology wording, tied to the group's
  reader count `{n}` (`IMP-ownership.md` "The muted hint" — drafted at Phase 2 step 5, confirmed live
  at Phase 5 step 4).
- **`auto-arc-cautionary-tint`** (existing entry): stays unchanged, still deferred — no per-hint tint
  rendering path exists in `ynz-lsp` today (confirmed, ¶1 Terrain); this milestone does not touch it.
- **`background-handle-cancel-injection`** (existing entry): retired (Phase 7 Branch A) if the
  language-half cancellation ships for real, OR its `ships_in`/`triggers` fields are rewritten with a
  concrete, milestone-specific finding (Phase 7 Branch B) — never left with the prior vague wording.
  **Outcome 2026-09-04: Branch B — `modify [[deferred_language_feature]] name =
  "background-handle-cancel-injection"`: `substitute` (current run-to-completion-then-shutdown-stop
  semantics), `why` (the task-retirement-ladder-is-not-a-scope-exit finding), `triggers` (the
  scope-exit release pass with handles as one arm, the pin tests as the flip), `ships_in = "a later
  version"` — pending Patrick's sign-off (Future Requirements #3).**
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
- **Loom:** a `[target.'cfg(loom)'.dependencies]` entry on `ynz-runtime` (Phase 3, landed) — reachable
  only under `RUSTFLAGS='--cfg loom'`, deliberately NOT a Cargo feature (a feature is enableable by any
  consumer; a cfg is not, and it keeps tokio's own `all(test, loom)` gate inert). Loom test runs are
  their own invocation inside the same Docker `dev` service, never a separate toolchain:
  `RUSTFLAGS='--cfg loom' CARGO_TARGET_DIR=/work/target/loom cargo test -p ynz-runtime --release --lib
  -- loom_ --nocapture` (own target dir so the cfg'd rebuild never invalidates the main one).
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
3. **Track 3 re-deferral — Phase 7 took Branch B (RE-DEFER), 2026-09-04, executor
   `m8-p7-20260904-a1`; PENDING Patrick's sign-off (the phase's own exit criterion).** Evidence:
   ten probe programs with the alloc counter armed plus a `--emit-ir` read, recorded in `audit.md`
   entry `m8-p7-20260904-a1`; the current state is pinned loud in-suite by
   `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs` (two tests that flip when the trigger fires).
   - **WHAT is deferred:** the language half of cancel-via-drop — codegen calling `ynz_handle_free`
     when a `background` handle binding's scope ends (block end, loop-iteration end, every function
     exit path including early `return` and auto-propagated `errors`, and a state-machine frame's
     retirement on both of its free paths: the caller's `free_frame` and a spawned parent's drop
     ladder), plus the cancellation surfacing inside the child as a typed `errors` value. What the
     general mechanism must provide, so handles can be one arm of it: a per-scope-edge, per-binding
     release point for every heap-backed local type (`array`, `map`, `string`, `channel`, promoted
     `maybe`/union cells, handles) on the PARENT's frame at the parent's time; bindings consumed by
     the transfer rule (sent / given away / returned) skipped; loop-rebound locals released per
     iteration; one registration surface and one dispatch with a per-type arm — the source-scope
     twin of the runtime ladder's kind arms, never a second ladder.
   - **WHY a handle-only pass forks a second mechanism (probe evidence, by frame and by time):**
     (a) the ONE cleanup choke point — `SpawnStateFnFuture::drop`'s kind arms in
     `crates/ynz-runtime/src/runtime.rs` (Phase 4 wired `refuse_closed`/`ChannelElemDrop` through the
     channel glue; Phase 5 added `BG_ARG_KIND_ARC_SHAPE`) — is the CHILD task's retirement ladder: it
     walks the child's own frame's `BgArgDropEntry` descriptors when the child retires. A handle's
     scope exit is the PARENT's event on the PARENT's frame. Probes: P2 (handle bound in an `if`
     block; block exits, parent lives on; child prints `child: done` — no cancel; alloc=2/free=2);
     P4 (handle in a suspending helper called by `wait`; the helper's frame is freed by the caller's
     `free_frame` after Ready — memory only, no per-slot walk; child completes after
     `launch: returning`); P6 (non-suspending parent: `lower_let_background_handle`'s alloca branch;
     dies at `ret`; child completes); P9 (`for` loop re-binding `h` three times — three scope exits in
     one function, three children complete); P10 (parent is itself a `background` task — the only
     parent kind that HAS a ladder — and the grandchild still completes after `child: retiring`,
     because the ladder fires at retirement and never reads a handle slot). (b) NO local of ANY type
     is released at scope exit today: `emit.rs`'s only `ynz_array_drop`/`ynz_free`/`ynz_channel_free`
     emissions are the task-side arg-free glue `emit_bg_arg_frees`, the channel element-glue table
     built from `channel_elem_drop`, and the spike trampoline's staged decimal128 cell; P3's IR has
     zero `ynz_handle_free` calls (and none for anything else at the parent's exit). There is nothing
     to extend — a handle arm would have to invent the scope-edge enumeration the general pass needs
     verbatim, then the general pass would run beside it. (c) Semantics: handle scope exit meaning
     *stop the task* while the array/map/channel/`maybe` cell bound at the same scope exit stays held
     is a per-type inconsistency ([`auto-promotion.md`](../../../rules/auto-promotion.md)'s banned
     pattern 4) and a teaching contradiction (Golden Rule 11). (d) Weighed and rejected: a
     `BG_ARG_KIND_TASK_HANDLE` arm in the PARENT's ladder — one choke point, but it only exists when
     the parent is itself a task and fires at the parent's retirement, not the binding's scope exit:
     structured task-tree cancellation for nested spawns only, a different semantics from the locked
     model, silently. (e) A finding the probes surfaced about the doc's current-state claim: "never
     silently killed mid-work" holds only while `entrypoint` runs — at its return `ynz_rt_shutdown`
     stops every still-running task at its next suspension (P1/P1b: `child: start` printed, `child:
     done` never; alloc=1/free=1 — the ladder ran). `IMP-no-function-coloring.md` "Task Cancellation"
     now says so.
   - **COST to fix later:** the drop-story milestone's pass — 1–2 sessions per the roadmap's
     never-drop-locals row (scope-edge enumeration, per-type arms, transfer-rule skip, per-iteration
     release, the state-machine retirement hook on both free paths) — plus, for the handle arm itself,
     small: one arm calling the already-proven `ynz_handle_free`, the two pin tests rewritten into the
     Branch A fixtures (scope-exit cancel at the next suspension; alloc==free; the crossing-local,
     non-suspending-parent and loop shapes; double-free impossible by construction with a parity
     test), and the child-side typed `errors` surfacing, which is its own small design (Tokio abort
     drops the child's future; nothing in the child observes it today).
   - **TRIGGER:** the drop-story milestone landing its scope-exit release pass (handles join as one
     arm; the pin tests flip; `background-handle-cancel-injection` is retired then), OR a real
     workload needing to stop a running task before then (the command-channel substitute is the
     shipped answer meanwhile). Consequence for the two Phase 1 deferrals that named "the missing
     scope-exit drop pass": `channel-auto-close-on-last-producer`'s dependency is NOT satisfied by M8
     (its trigger stands; remaining blockers unchanged — the producer-role analysis, the producer/holder
     refcount split, and the pass itself); `background-handle-close` never depended on the pass (its
     trigger is a real program that cannot bind the channel first) — unaffected.
   - **(round-1 grading, 2026-09-04)** **Cheap guard considered, OPEN for Patrick at the sign-off:** the deferred behavior has live exposure — a task keeps running after its handle's binding leaves scope. `inference.md`'s Informational muted-hint category fits: a comment-style hint at the spawn, `// this task keeps running after `h` goes out of scope — `wait` on it or send it a stop signal to end it sooner`, from the same spawn walker the `auto_arc` hint uses. Cost: one LSP pass, no compiler change. Patrick decides at the re-deferral signature whether it ships with this milestone (Phase 9) or rides the pass.

4. **Loom's Tokio-internals boundary** (Phase 3, landed 2026-09-03) — **WHAT:** loom model-checks
   only the synchronization logic ynz-runtime owns directly (`pending_sends`, `recv_waiters`, the
   channel refcount, the published drive identity, the drop ladder's kind-2 arm, recv-poll ordering);
   it does NOT and cannot model-check Tokio's own internal `mpsc`/semaphore/scheduler implementation.
   **The mechanism, verified at Phase 3:** tokio gates its own loom paths on `cfg(all(test, loom))`,
   which is never true for a dependency, so under ynz-runtime's `RUSTFLAGS='--cfg loom'` tokio compiles
   as plain std and each `try_send`/`poll_recv`/endpoint-future poll is one opaque step; the
   loom-only `YnzChannel::mpsc_witness` atomic makes those steps DEPENDENT for loom's DPOR so their
   relative orders are exhaustively explored — the black box modeled as one atomic object — but
   nothing inside a step is. **WHY not closed:** structurally out of reach — loom cannot instrument
   code it doesn't control the compilation of. **COST:** unbounded/not applicable — this is a
   permanent scoping boundary, not a deferred task. **TRIGGER:** none; this is a named, permanent
   limitation, recorded so no future reader mistakes "loom-verified" for "every layer, including
   Tokio's own internals, model-checked."
5. **Auto-Arc residual (Phase 5 step 7; four fields recorded at fix round 2, FRAGO 013 — Phase 5
   round 2)** — **WHAT:** the beneficial-emission condition's declines that are NOT soundness declines
   but missing plumbing, named by the narrowed registry entry `auto-arc-codegen-emission` (`substitute`
   /`why`/`triggers`): (1) `background` groups that straddle a suspension point; (2) a `background`
   statement inside a loop (one statement per iteration — no group forms); (3) `array`/`map` values,
   and shapes with a `number`, `array`/`map`/`maybe`/union/`channel`, or nested-`shape` field
   (`arc_shareable` is the floor; nested shapes are stored as pointers per
   `shape_types.rs::llvm_field_type`, and `number` is additionally held out by parked item 40's live
   copy-path SIGSEGV, FRAGO 012). **WHY not absorbed here:** (1) needs the caller-side transient to be a
   crossing local (frame-backed compiler-owned temporaries do not exist — R2's layout hazard); (2) needs
   a caller-held reference that outlives one iteration, i.e. the language-wide scope-exit drop pass
   (Phase 7's question, M5 FR#6 / M6 FR#13–17); (3) needs a counted sharing substrate for pointer-
   carrying and 16-byte-aligned values — a different design from the byte-copy block `arc.rs` is.
   Each declines to the shipped per-task copy path by construction, so nothing is unsafe meanwhile;
   only the extra copies are paid. **COST:** (1) ~1 session once frame-backed temporaries exist (the
   admission drops its suspension boundary; the emission stores the transient in a frame slot); (2)
   ~1 session on top of the drop pass (the transient becomes a scoped local released by the pass);
   (3) a design round plus ~2 sessions (a header-plus-buffer refcount for arrays/maps; a 16-aligned
   block variant for `number`). **TRIGGER:** exactly the registry entry's `triggers` — (1) frame-backed
   compiler temporaries land; (2) the scope-exit cleanup mechanism lands; (3) a counted substrate for
   pointer-carrying values is designed; each slice widens the admission independently. The `number`
   slice additionally waits on parked item 40's hotfix.
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
   compiler-wide missing scope-exit drop-insertion pass — confirmed by code, not by
   `docs/internal/scratchpad/SCRATCH-audit-2026-07-11-memory-safety.md`, which was never committed to
   this repo (parked item 27, `m8-p2-signoff-20260903`): `ynz_handle_free` is declared in
   `crates/ynz-codegen/src/runtime_decls.rs:126` with zero emit sites — see
   [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
   "Task Cancellation" and the roadmap's never-drop-locals row — no drop-insertion pass exists at all,
   and these two leaks are symptoms of that gap, not independent bugs. The scope-drop design must not
   be finalized blind to this class. Whether the drop-story
   work itself lands in M8 or its own milestone is a plan-review question to resolve at M8's
   plan-review gate — this entry does not pre-decide that; it only ensures the design account for
   the class.
8. **Two M6-inherited residuals on the P2-7 waker-registration path** (surfaced 2026-09-03 by the
   independent confirmation that retired Phase 6 under FRAGO 001; recorded HERE so a reader of this
   plan finds them without archaeology — both already carry fielded deferrals in the roadmap's own
   `audit.md`, so neither is silent duct tape):
   (1) **Panic-payload log asymmetry.** The handle-side panic path logs a payload-less message
   because `panic_payload_msg` is private to `channel.rs`, while the channel-side logs the payload
   string. **WHY not fixed here:** cosmetic, log text only on a now-theoretical panic path; it does
   not narrow the hang-closing guarantee, and this plan writes no code on that path at all.
   **COST:** trivial (widen the helper's visibility, one call site). **TRIGGER:** the next milestone
   that touches `handle.rs`'s panic-reporting path, or any real panic there needing diagnosis.
   (2) **`recv_waiters` / `record_recv_waiter` / `wake_recv_waiters` duplicated byte-identically
   between `channel.rs` and `handle.rs`.** **WHY not fixed here:** unifying a waker registry across
   two FFI-boundary types is its own design call, and doing it inside a milestone that touches
   neither path would be scope drift. **COST:** small-to-medium (one shared registry type + both
   call sites migrated + loom coverage of the unified path). **TRIGGER:** Phase 3's loom substrate
   work, if it finds it needs one registry rather than two to model-check the ordering — otherwise
   the next milestone touching either recv-poll path.
9. **`background` arg escape door #4 — a ladder-owned array clone stored into an ALIASED outer
   container** (discovered 2026-09-03 during the `fix/bg-arg-channel-send-uaf` hotfix; live in the
   released v0.3.3 compiler; RED-pinned by `bg_arg_alias_container_add_is_a_known_uaf_red_pin` in
   `crates/ynz-driver/tests/integration.rs` against fixture `bg_arg_alias_container_add_red.ynz`).
   **WHAT is deferred:** `stash(bucket: array<array<int>>, rows: array<int>) { bucket.add(rows) }`
   spawned with `background` — `rows` is heap-cloned and owned by the task's drop ladder, but
   `bucket` is an `array<pointer-elem>` bg arg that `prepare_bg_arg_for_ctx`
   (`crates/ynz-codegen/src/emit.rs`, the `array<string/maybe/map/union/array>` and `map`/`union`
   fall-through arms) passes through UN-cloned, aliasing the parent's container. The task pushes
   its clone into the parent's bucket; the ladder frees the clone at retire; the parent's
   `bucket[0]` dangles (observed: garbage counts `471878446419399850` / `-924992314359518642` and
   SIGSEGV across 5 runs). The hotfix's pointer-identity release protocol closes the channel-send,
   handle-send, and handle-return doors and deliberately does NOT close this one. **WHY:** the
   escape exists because the container was aliased rather than cloned — a different producer from
   the three fixed doors (where the runtime is the hand-off point and can release the slot at the
   moment ownership moves). Hooking `ynz_array_push` to walk the current drive's ladder would put
   an O(descriptors) scan plus a thread-local read on a hot, synchronous, non-concurrency path to
   patch a symptom of the alias fall-through; both hotfix reviewers agreed the right fix is closing
   the fall-through (clone — or otherwise give a defined ownership — to `array<pointer-elem>` /
   `map` / `union` bg args), which is a design decision about what a `background` argument IS, not
   a runtime patch. **COST to fix later:** ~1 session — extend `prepare_bg_arg_for_ctx`'s per-type
   table with a defined deep-copy (or share) semantics for pointer-cell element arrays and maps,
   with the matching `BgArgFreeKind` free arm, a fixture per element class, and the RED pin flipped
   to its correct-world assertions (gap 6 with a real `bucket[0]` dereference, or `bucket.count()`
   = 0 if the container is cloned). **TRIGGER:** v0.3-M8's general ownership rule for `background`
   arguments — the scope-drop / drop-insertion design this plan's Track 3 and Future Requirement
   7(2) own must decide, once and for every heap type, whether a bg arg is cloned, shared, or
   given; this alias fall-through closes under that rule (the same phase that lands it), or
   earlier if a user hits the dangling-container read in the wild. **Channel-close design
   interaction (fix round 2, corrected in fix round 3):** round 2 named the aliased-bg-arg-then-
   `send` shape (`background producer(wire, table)` + `wire.send(table)` on a `channel<map<..>>`)
   as THIS door through the channel, neither opened nor closed by close semantics. **Round 3's
   `ParamNeedsGive` guard CLOSES that channel-door instance**: the send requires `give table` on
   `producer`, and `give` consumes the parent's `table` at the spawn through `check_arg_ownership`
   (the `Expr::Background` arm runs `infer_expr(inner)` → `check_user_fn_call`), so the parent can
   no longer read the aliased map; the channel or its receiver is the allocation's only holder, and
   the ladder has no descriptor for an un-cloned arg (nothing to double-free). Phase 4's
   `channel<map>` fixture asserts the `give`-map pass-and-send shape CORRECT (alloc/free parity), and
   the no-`give` variant as the gallery trigger. **What this FR still owns is the non-channel door
   only** — `bucket.add(rows)` storing a ladder-owned clone into an aliased container — which no
   channel rule can reach; its RED pin, cost, and trigger above are unchanged.
10. **`.copy()` codegen catch-all silently aliases for every type outside `Shape`/`array`**
    (discovered 2026-09-03 while recording FRAGO 004 ruling 4). **WHAT is deferred:** the
    `PostfixOpKind::Copy` lowering (`crates/ynz-codegen/src/emit.rs:19301`) ends in
    `_ => Ok(recv_val)` — the receiver's own pointer — and typeck's `check_postfix_op`
    (`check.rs:6939`) admits ANY receiver ("P3c will enforce trivially-copyable" never shipped). So
    `x.copy()` compiles and returns `x` itself for `maybe<T>`, union, `fixed<T>`, `dynamic` — the
    same FRAGO 014 alias-no-op stub class that was closed for `array` (`m5_p5_copy_aos_independent`)
    and, in Phase 4 step 3a, for `map`. **WHY not absorbed here:** Phase 4 closes exactly the type
    this milestone's diagnostic advice names (`map`); a `.copy()` audit across the remaining types
    is a decision about what `.copy()` MEANS for each (one-level? deep? refused?) and belongs with
    the ownership/drop story, not with channel close. **COST to fix later:** small-to-medium —
    per-type decision + arm (or a typeck refusal with a three-slot diagnostic for types that must
    not be copied), one independence fixture per admitted type. **TRIGGER:** the first diagnostic
    or spec example that recommends `.copy()` on one of those types, or Track 3 / FR 7(2)'s
    ownership rule for `background` arguments (which must decide what a copy of each heap type is).
    **Phase 2's transfer rule reaches this class without waiting for the audit (FRAGO 010, signed
    2026-09-03):**
    `provenance(expr).copy()` classifies `Unknown` unless `copy_is_independent(type)` holds
    (`IMP-ownership.md` "Classification" table), so a `.copy()` on `maybe<T>`, union, `fixed<T>`, or
    `dynamic` is refused as a transfer (`TransferNeedsCopy`) rather than silently admitted as an
    alias — this FR's remaining scope is auditing what `.copy()` on those types SHOULD mean, not
    whether transferring one of today's alias-no-op copies is caught.
11. **Two genuine runtime defects the Phase 8 owned-heap-channel widening surfaced (discovered
    2026-09-04, fix round `m8-p8-fix2-20260904`; RED-pinned nowhere yet — repro kept only in this
    entry and the round's session transcript; NOT fixed inline per CCIR item 5 / risk R5).** Both
    are avoided at the fuzz generator level with narrow, evidence-backed guards (`take_or_make_
    array`/`take_or_make_map`'s `suspension_seen` gate in `mod.rs`; the `send_count.max(...)`
    capacity floor in `stmt_background_drain_loop`) so the fuzz corpus and `cargo test
    --workspace` stay green while these are triaged — neither guard narrows an entire element
    kind or construct, each targets only the bisected shape.
    - **(a) A crossing-local heap-channel-send corruption.** **WHAT:** an `array<int>`/
      `map<string,int>` LOCAL declared BEFORE any suspension point in the SAME function (a
      `wait`, a `background`-handle `.receive()`, a channel `.receive()`) and later `.send()`-ed
      into a channel AFTER that suspension reads back corrupted on receive —
      `RUNTIME ERROR: killed by signal 6 (SIGABRT)`, a null/misaligned pointer dereference inside
      `ynz_map_count`/`ynz_array_count` (`crates/ynz-runtime/src/lib.rs:1058`/`:1411`). Minimal
      repro (bisected, both directions confirmed):
      ```
      function fetch1(n: int) -> int { wait sleep(1); return n + 6 }
      function entrypoint() -> nothing {
        let rows12: array<int> = [0]           // declared BEFORE the wait — CRASHES
        let v13 = wait fetch1(5)
        let wire15: channel<array<int>> = channel<array<int>>(1)
        wire15.send(rows12)
        let got16 = wire15.receive()
        if (got16.exists()) { print(got16.value.count().toString()) }
      }
      ```
      Swapping the two `let` lines (array declared AFTER the `wait`) is confirmed SAFE. Confirmed
      general to both owned-heap kinds; NOT reproduced by `channel<number>`, by reading the local
      WITHOUT sending it into a channel, or by an inline (non-`background`) channel's own
      close-then-drain in isolation. **WHY not fixed here:** this is exactly the frame-crossing /
      suspension-boundary hazard family M3a/M3d/M3e/M3g's twin-derivation corpses warned about
      (`authoritative-derivation.md`) — diagnosing which choke point (the channel-send transfer
      lowering, or the crossing-local frame-slot machinery itself) needs the fix is real
      engineering, not a fuzzing-harness-round task. **COST to fix later:** a diagnosis session
      (read `crossing_local_names`/the channel-send lowering against this exact repro) plus a fix
      whose shape depends on the diagnosis — likely small once the choke point is identified,
      per this class's usual shape. **TRIGGER:** the next milestone touching channel-send
      lowering or the crossing-local/suspension-frame machinery, OR a real workload hitting it
      (any program that builds an `array`/`map` before an I/O call and sends it afterward).
    - **(b) A capacity-forced-blocking `number` channel send loses a value.** **WHAT:** a
      `channel<number>(1)` fed by a `background` producer sending the SAME `number` binding 3
      times loses one value under `--no-optimize`/`--no-auto-parallel` (`8.6 * 3` prints `17.2`
      instead of `25.8`) — but ONLY when the producer is forced to actually BLOCK on a full
      buffer (`send_count > capacity`); 2 sends into capacity 1, or 3 sends into capacity 4, are
      both confirmed safe. **WHY not fixed here:** fr12's `number_to_heap_cell` marshalling
      interacting with the channel's backpressure/blocked-send path under `-O0` is a codegen or
      runtime diagnosis, not a fuzzing-harness-round task. **COST to fix later:** a diagnosis
      session (the backpressure/retry path for a blocked `ynz_channel_send` under `-O0`, cross-
      referenced against `number_to_heap_cell`) plus a fix of unknown size until diagnosed.
      **TRIGGER:** the next milestone touching `channel<number>` marshalling or the channel
      send/backpressure path, OR a real workload sending a repeated `number` value into a
      near-full channel.
    - **Both share one open question worth naming:** are these the SAME producer (a general
      "value crossing a suspension/blocking-send boundary" bug with two symptoms) or two
      independent ones? Bisection did not settle it — (a) needs no backpressure and no
      `--no-optimize`; (b) needs both. Whoever picks this up should check that first, per
      `root-cause.md`'s "cluster findings before fixing any" — fixing them as two independent
      symptoms would be the exact whack-a-mole that rule exists to prevent, if they turn out to
      share an ancestor.
