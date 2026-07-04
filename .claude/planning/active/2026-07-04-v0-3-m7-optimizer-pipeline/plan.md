---
name: "v0-3-m7-optimizer-pipeline"
plan-id: "2026-07-04-v0-3-m7-optimizer-pipeline"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-author-2026-07-04-m7-optimizer", "plan-amend-2026-07-04-m7-blockers", "plan-amend-2026-07-04-m7-links", "plan-amend-2026-07-04-m7-phase6-yield", "gate4-signatures-2026-07-04"]
created_at: "2026-07-04"
updated_at: "2026-07-04"
metadata:
  type: "plan"
---

# PLAN: v0.3-M7 — Optimizer Pipeline

> **Frontmatter status — `paused`, and this is the correct value, not a deviation.** This OPORD is
> complete by the plan-producer charter's own graduation rule — ¶3.1 Intent & End State is non-empty,
> every phase is concrete, the risk table is scored — which would ordinarily flip status straight to
> `active`. It is deliberately held at `paused` instead, because `paused` is the **conductor-set
> pre-approval state** for a plan in exactly this shape: fully written, but gated on two real,
> external preconditions on EXECUTION start, not on anything wrong with the document itself —
> **(1) Gate 4**, the orchestrator's human read-through/approval checkpoint, which has not yet run on
> this plan, and **(2) the M6-merge precondition** (¶1 Friendly forces; CCIR item 1) — Phase 1 cannot
> begin until the sibling v0.3-M6 hotfix plan merges to `main`. The orchestrator flips `status` to
> `active` once Gate 4 clears and M6 has merged — a plain frontmatter edit on this same file, per the
> status lifecycle ([`REF-plan-format.md`](../../../../../.claude/docs/reference/REF-plan-format.md)).

## 1. Situation

**Terrain (landscape).** The Yinz compiler (`crates/ynz-codegen`, `crates/ynz-typeck`,
`crates/ynz-runtime`) emits every code path — arrays, shapes, the concurrency state-machine engine,
channels, Arc ops — through exactly two `TargetMachine` creation sites
(`crates/ynz-codegen/src/emit.rs:879`, `crates/ynz-codegen/src/state_machine.rs:755`), both hardcoded
to `OptimizationLevel::None`. This is the ONLY optimization-level configuration point in the entire
codegen crate (grep-confirmed, roadmap capability-discovery 2026-07-04). Zero LLVM pass-pipeline code
exists anywhere in the workspace (`run_passes`/`PassBuilderOptions`/`PassManager` — zero hits,
repo-wide grep). `inkwell` is pinned at `0.9.0` (`llvm18-1-prefer-dynamic` feature,
[`Cargo.toml`](../../../../Cargo.toml):29) — whether it cleanly exposes LLVM 18's new-PM
`run_passes` API is **unverified**; this is 100% net-new code for this milestone.

A baseline-verified spike flipped both sites to `OptimizationLevel::Default` and found **6/470**
`ynz-driver` integration failures — ALL on `number` (decimal128, 16-byte) crossing-local or EC-collect
paths, direct-repro SIGSEGV confirmed; every structurally-identical int/bool/float/string/shape/array/map
sibling passed. The **primary durable evidence** of this verdict is
[`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)'s
"Phase-0 spike — O0 → Default optimization" section (the exact failing-fixture list, the direct-repro
SIGSEGV, and the Fable-verified caveat that a GREEN verdict here could be a false negative). The spike's
mechanical 2-line change itself is preserved as a checked-in unified diff at
[`spike-o0-flip.patch`](./spike-o0-flip.patch) (plan-relative, this directory) — reconstructed and
byte-verified against this repo's current tree, replacing the original throwaway worktree, which was
gitignored, uncommitted, lived in a different clone, and had its branch reused (no longer preserves
anything). Phase 1 reads `spike-o0-flip.patch` directly rather than re-deriving the mechanical change
from memory; it reads the audit doc's section for the evidence of what that change broke. Two code
comments (`emit.rs:9961-9963`, `emit.rs:10717-10719`) blame `mem2reg`, but `TargetMachine`'s
`OptimizationLevel` drives **backend** passes (ISel/regalloc/scheduling) at codegen time, not the
mid-end IR pipeline where `mem2reg`/SROA/DCE live — that attribution is an **unverified theory**, not a
settled fact. This plan's Phase 1 exists specifically to stop that theory being carried forward
unverified.

Two other confirmed, independent bugs sit in this same neighborhood and must not be conflated with the
optimizer question itself: (a) a **general hot-loop O0 stack-exhaustion SIGSEGV** at ~4.19M total
loop-visits, reproducible in BOTH AoS and SoA layout modes, root-caused to nothing yet (roadmap
Capability Ledger, "General hot-loop O0 stack-exhaustion ceiling fix," Patrick-signed next-fix-priority
BUG per his 2026-07-04 triage policy) — the `soa_calibration.rs` bench harness caps `TOTAL_VISITS` at
131,072 specifically to dodge it; (b) `ynz run` masks a SIGSEGV as a diagnostic-free exit code `1`
(`crates/ynz-driver/src/run.rs:75`, `status.code().unwrap_or(1)`) — flagged for **M6**, not this plan.

**Weather (external constraints).** CI is Linux-only (`ubuntu-latest`, LLVM 18 via apt,
`LLVM_SYS_181_PREFIX` pinned — [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml)).
All cargo commands run via `docker compose run --rm dev ...` (no `-it`, non-interactive) per this
project's dev-container convention. `crates/ynz-codegen/tests/golden.rs` records IR-text and
object-file SHA-256 goldens for `x86_64-unknown-linux-gnu` only — this plan's Phase 5 invalidates and
regenerates **every one** of them; this is expected, in-scope work, not scope creep.

**Friendly forces.** This plan branches from `main` **after** the sibling hotfix plan (v0.3-M6, authored
in parallel, not yet a file on disk under `.claude/planning/`) merges. M6 owns the concurrency-release
audit's correctness findings (P1-1 UFCS suspension invisibility, P3-1/P2-2 `pending_sends` ABA, P3-2
lost-wakeup, P4-3 unasserted `block_on` fallback, P2-4 buffered-element leak, P3-3 shutdown-mutex scope,
and the `ynz run` signal-masking bug) — this plan does **not** re-fix any of them.
**Assumption (unverified):** M6 ships and merges before this plan's Phase 1 begins. This is the exact
sequencing the assembling brief states as Patrick's intent, not an assumption this plan invents — but
since no M6 plan file exists yet at authoring time, the executor MUST confirm M6 is merged to `main`
before starting Phase 1 (see Coordinating Instructions CCIR). If M6 has not merged, halt and report —
do not build atop unmerged, unstable correctness fixes.

**Assumptions:**
- M6 merges before Phase 1 starts — **unverified**, confirm at Phase 1 kickoff.
- `inkwell` 0.9.0 exposes enough of LLVM 18's PassBuilder surface to build a `run_passes`-equivalent
  pipeline — **unverified**, Phase 0 is the gating spike.
- The spike's 6 failures are a **symptom** of a broader class (missing/incorrect LLVM attributes on
  runtime FFI declarations, or genuine frame/stack-slot handling under real optimization) rather than
  an isolated decimal128-only bug — **unverified**, Phase 1's exhaustive sweep is the test.
- The `--no-auto-parallel` / `default_target_machine` precedent (already the single shared constructor
  in `state_machine.rs`) is the correct place to thread the new pipeline config — **verified** by direct
  reading of both call sites (Situation, above); this plan extends it rather than re-deriving a second
  config path, per [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md).

**Cross-cutting factor sweep** (folds into the risk table below and the phase texts; factors with no
applicable risk get their one-line "N/A — why" here, per the mandatory-factors discipline):
- **Security / PII / compliance / SEO / accessibility:** N/A — this milestone changes a compiler
  codegen tier; no new external input, no new UI/IDE surface, no new user data, no web-facing surface.
- **Type-safety:** N/A beyond existing guarantees — no new language types; the optimizer must not
  change *typed* program behavior, only speed (covered under Safety invariants).
- **Perf/BigO (mem+cpu):** central to this milestone — see R5/R6 below and the Performance invariant.
- **Reusability/DRY:** central — R4 below (the dual-TargetMachine-site hazard) and the reuse of
  `soa_calibration.rs`'s harness pattern for Phase 7 rather than a fresh one.
- **Idempotency:** relevant — reproducible-build invariant (Safety) requires repeated `ynz build` runs
  on the same input to produce byte-identical objects.
- **Error-handling:** relevant — Phase 0/1 must fail the *build* cleanly (not panic) if the pipeline
  API genuinely isn't available; covered in Phase 0 exit criteria.
- **Observability/logging:** minor — no new user-facing surface is required this milestone;
  `YNZ_OPT_FORCE` (Phase 3, dev/bench-only, mirrors `YNZ_SOA_FORCE`) is not a shipped observability
  feature. A user-facing "which tier compiled this binary" surface is out of scope; not silently
  dropped — named here as considered and declined for this milestone.
- **Race/TOCTOU + resource-cleanup:** central — R1 below (drop-glue / Arc / channel calls surviving
  DCE/reordering) is exactly this concern.
- **Civil considerations:** N/A — compiler-internal backend work, no user-facing surface change.

**Risk Assessment** (scored via [REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md),
deterministic lookup; no floor class fires — no money/PII/security/no-backout dimension is present,
this is pre-release compiler-internal work, fully git-reversible):

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **R1 — optimizer flip miscompiles suspension frames / runtime FFI calls** (proven: 6/470 decimal128/EC-crossing SIGSEGVs; unproven: whether `ynz_arc_*`/`ynz_channel_*`/drop-glue calls carry correct LLVM attributes to survive DCE/reordering) — *Phases 1–2, 5* | A | III | HIGH | Committed RED fixture set (the 6 spike fixtures + every sibling Phase 1's sweep finds) gates the build; root-cause-before-fix ordering (**B2 adversarial/RED-repro**, prob −1; proof: failing fixtures committed before any fix lands) | **MEDIUM** (B×III) | recorded |
| **R2 — general hot-loop O0 stack-exhaustion SIGSEGV** (ledger row 439, absorbed by this plan) confounds honest benchmarking and is a live bug independent of SoA — *Phase 4* | A | III | HIGH | Root-cause + eliminate the failure mode (alloca/stack-growth fix + a stress regression fixture) (**B1 eliminate**, prob −2; proof: Phase 4's fixture running past the old ~4.19M-visit crash envelope) | **MEDIUM** (C×III) | recorded |
| **R3 — inkwell 0.9.0 may not cleanly expose LLVM 18's PassBuilder/`run_passes` surface** (net-new code, zero existing call sites) — *Phase 0* | B | III | MEDIUM | Hard-gate P0 spike with explicit accept/reject STOP-conditions before any durable phase depends on it (**B2 canary/staged**, prob −1; proof: Phase 0's persisted spike verdict) | **MEDIUM** (C×III) | recorded |
| **R4 — dual `TargetMachine` creation sites drift on pipeline config**, silently mismatching the main path vs. the state-machine path (this roadmap's own recurring authoritative-derivation corpse class — 4 confirmed instances in M4 alone) — *Phase 2* | B | II | HIGH | Thread ONE authoritative constructor (extend `default_target_machine`; delete the second inline construction) — the divergence class cannot exist with one source (**B1 eliminate**, prob −2; proof: grep-verified single construction call site + both consumers threaded from it) | **MEDIUM** (D×II) | recorded |
| **R5 — LLVM passes regress `ynz build` compile-time** beyond the roadmap's existing <10% wall-clock budget on `pirates-roster` — *Phase 3* | C | III | MEDIUM | Measured wall-clock gate before the default tier ships (**B2 canary w/ auto-reject**, prob −1; proof: committed before/after timing in Phase 3's exit criteria) | **LOW** (D×III) | pass |
| **R6 — preemption call-site checks reintroduce the 1190% overhead** measured previously at O0 (wrong-tier evidence) if added blindly — *Phase 6* (call-site checks are, like back-edge checks, codegen-emitted poll-yield sites — never runtime-implicit magic; this risk is about the OVERHEAD of emitting them, orthogonal to R8's frame-layout-correctness hazard for the back-edge mechanism itself) | C | III | MEDIUM | Ship call-site checks ONLY if a fresh O2 measurement clears a pre-registered threshold; otherwise the four-field deferral is the only path — the bad outcome cannot ship (**B1 eliminate via measurement-gated decision**, prob −2; proof: Phase 6's committed O2 measurement + explicit accept/reject line) | **LOW** (E×III) | pass |
| **R7 — optimizer/golden non-determinism** (LLVM pass-ordering or other codegen non-determinism could break the byte-identical 2-run golden-regeneration gate) — *Phase 5* | C | III | MEDIUM | The Phase 5 two-independent-run gate itself (**B2 engineered guard**, probability, −1; proof: golden regeneration re-run a second independent time, byte-diffed against the first — Phase 5 step 3) | **LOW** (D×III) | pass |
| **R8 — the back-edge poll-yield codegen transform introduces a NEW frame-layout/crossing-local suspension hazard** (turning a qualifying loop back edge INSIDE a state-machine function into a new poll-yield suspension point — store `resume_point`, flush crossing locals, return `Pending` — is net-new codegen logic in the same silent-miscompile family as R1, and this repo's four-milestone twin-derivation/frame history: M3a/M3d/M3e/M3g, per [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md)) — *Phase 6* | B | II | HIGH | Adversarial/RED-repro fixtures: loop-crossing-local suspension fixtures (the SM-positive case AND the non-SM residual case) authored and committed BEFORE the transform lands, gating the build (**B2 adversarial/RED-repro**, probability, −1; proof: failing fixtures committed pre-implementation, Phase 6 Steps 1 & 3) — re-lookup(C, II) = **HIGH, unchanged** (Critical severity does not clear High until probability reaches D; no second honestly-provable catalog mitigation applies — full work-shown in the RISK OVERRIDE block immediately below) | **HIGH** (C×II) | **BLOCKED — unsigned RISK OVERRIDE below** |

R8's residual lands HIGH and, per the frozen risk-engine catalog's available patterns, cannot be
honestly mitigated further at plan-authoring time (see the RISK OVERRIDE block immediately below —
drafted with the work shown, signature deliberately left blank; this producer never self-signs a HIGH
residual). Every OTHER residual in this table stays MEDIUM or LOW; no policy floor fires anywhere in
this table (still no money/PII/security/no-backout dimension). If Phase 1's sweep or any other phase
surfaces a FURTHER NEW risk that scores HIGH, it is surfaced immediately per the CCIR below — **never
self-signed**; the orchestrator's override gate is the only place a HIGH residual gets accepted.

**RISK OVERRIDE — accepted residual: HIGH** (R8; work shown per [REF-risk-engine.md](../../../../../.claude/docs/reference/REF-risk-engine.md)'s gate; this is a producer-drafted surface for the orchestrator's human override gate — it is never self-signed):

```
RISK OVERRIDE — accepted residual: HIGH
  Risk:                     R8 — the Phase 6 back-edge poll-yield codegen transform (turning a
                            qualifying state-machine-function loop back edge into a new poll-yield
                            suspension point: store resume_point, flush crossing locals via the
                            existing suspension machinery, return Pending) introduces a new
                            frame-layout/crossing-local hazard in the same silent-miscompile family
                            as R1, and this repo's four-milestone twin-derivation/frame history
                            (M3a/M3d/M3e/M3g).
  Why not mitigable to LOW: Initial lookup(B, II) = HIGH. The one honestly-provable catalog
                            mitigation — Adversarial/RED-repro test (B2, probability, −1; proof:
                            loop-crossing-local suspension fixtures, both the SM-positive case and
                            the non-SM residual, authored and committed BEFORE the transform lands,
                            Phase 6 Steps 1 & 3) — shifts probability B→C. Re-lookup(C, II) = HIGH,
                            UNCHANGED: Critical severity does not clear High until probability
                            reaches D. No second catalog mitigation honestly applies: (a) the
                            severity-axis B1 patterns (made-reversible / idempotency) don't map to a
                            compiler miscompile, and this plan's own severity-anchor selection
                            (pre-release, fully git-reversible) already prices reversibility into
                            Sev II rather than Sev I — re-applying git-revertibility as a SECOND
                            mitigation step would double-count the same fact; (b) a second
                            probability-axis pattern (canary/staged exposure) does not honestly
                            apply either — its precondition ("small slice first, auto-halt on
                            metric") presumes staged PRODUCTION exposure, which does not exist for
                            compiler-internal, pre-release codegen work; stretching it to fit would
                            be exactly the self-serving cell-picking REF-risk-engine.md's "not a
                            vibes table" clause forbids. Reusing the existing authoritative
                            suspension machinery (`store_resume_point` / `flush_var_slot_to_frame` —
                            the same functions the wait-suspension path already uses and tests) is a
                            genuine, valuable design constraint (satisfies authoritative-derivation.md;
                            named in Phase 6 Step 1) — but it is recorded here as a SCOPING decision,
                            not double-counted as a second, independent catalog mitigation step.
  Accepted by:              Patrick (Gate-4 approval, conducted 2026-07-04)
  Date:                     2026-07-04
  Trigger to revisit:       Before Phase 6 Step 2 begins. Re-score if either (a) Phase 1's exhaustive
                            R1 sibling-sweep changes this risk's probability/severity picture, or
                            (b) a genuinely new B1/B2 catalog mitigation is authored into
                            REF-risk-engine.md (a deliberate authoring act, never an inline
                            plan-time invention) before Phase 6 begins.
```

## Design-Doc Alignment

Governing docs read at plan time; every divergence enumerated as "doc says A; plan does B because C,"
per [`.claude/rules/plan-invariants.md`](../../../rules/plan-invariants.md) `## Design-Doc Alignment`.

**Cited governing docs:**
[`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
"Scheduler Preemption Model" section (lines 214–244) ·
[`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) ·
[`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
(the concurrency-release audit this plan absorbs P4-1 and the O0→optimizer synthesis item from).

**Citation-depth verification (read live, not assumed):**
- `IMP-no-function-coloring.md`'s "Scheduler Preemption Model" section genuinely SPECIFIES the
  mechanism it is cited for, not merely names the topic: it locks compile-time-assisted safe-point
  preemption with checks at BOTH function call sites AND loop back-edges, a ~10ms default time
  quantum, and auto-inferred CPU-bound task routing (lines 216, 236, 238). Depth confirmed — this
  plan can cite it as ground truth for what the doc currently claims is shipped.
- [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) genuinely specifies the
  exact discipline this plan's R4/Phase 2 needs: "thread the SAME authoritative value/query into all
  of them... never let a second surface re-derive its own equivalent answer." Depth confirmed — not a
  topical citation.

**Divergences:**

1. **`IMP-no-function-coloring.md` says** (locked pre-v0.2) preemption checks fire at BOTH call sites
   AND loop back-edges; **reality is** codegen emits back-edge calls only, and they call a documented
   no-op stub (`runtime.rs:281-299`) — call-site checks were never implemented, and no
   `[[deferred_language_feature]]` registry entry was ever recorded (audit finding P4-1, HIGH). The
   doc has been FALSE since it was written. **The mechanism gap is architectural, not merely
   unimplemented** — this was corrected in this plan's own authoring after a personal plan-audit
   finding (see [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
   "M7-plan addendum"): `ynz_rt_check_preempt` is a synchronous `extern "C"` callee, which
   structurally CANNOT yield the enclosing Tokio task by itself. A real fix requires CODEGEN to turn
   qualifying loop back edges INSIDE state-machine functions into poll-yield suspension points (store
   `resume_point`, flush crossing locals via the already-existing suspension machinery
   `store_resume_point`/`flush_var_slot_to_frame`, return `Pending`), with the runtime function reduced
   to a cheap, synchronous budget CHECK the codegen-emitted branch consumes. Non-SM (plain synchronous)
   functions can never cooperatively yield this way — their only protection is the EXISTING
   CPU-admission routing to the blocking pool (see the Runtime Dependencies invariant below for the
   named residual this leaves: CPU-heavy code inside a non-SM function that admission misses).
   **This plan does B, not A, because C:** Phase 6 makes the
   doc TRUE either by (a) shipping the missing call-site checks for real — restoring the doc's
   original claim — or (b) rewriting the section to state the TRUE shipped mechanism (back-edge only)
   plus a proper four-field deferral, per whichever the fresh, pre-registered O2 measurement decides —
   either way the doc is rewritten to state the true THREE-part architecture: (i) SM-function back
   edges = codegen poll-yield (new, this milestone), (ii) non-SM CPU-bound work = blocking-pool routing
   (already shipped, unaffected), (iii) the named non-SM-admission-miss residual. Either branch closes
   P4-1 honestly; neither leaves the doc silently wrong. This disposition is
   Patrick-signed via the roadmap's row-443/M7-scoping note (¶1 Terrain) and the M6/M7 triage split
   below.
2. **`authoritative-derivation.md` says** thread one authoritative constructor, never re-derive a
   second; **this plan's model matches exactly, no divergence** — Phase 2 closes R4 by deleting the
   inline `TargetMachine` construction at `emit.rs:879` and routing it through
   `state_machine.rs::default_target_machine`, per the doc's own prescription. Confirmed compliant.

**Milestone-boundary assumption flagged:** M6 owns the concurrency-release audit's correctness
findings (P1-1, P3-1/P2-2, P3-2, P4-3, P2-4, P3-3, and the `ynz run` signal-masking bug — ¶1 Friendly
forces enumerates these explicitly). **P4-1 (preemption honesty) is NOT in that M6 list** — it is this
plan's Phase 6, because P4-1's honest resolution structurally REQUIRES a real O2 measurement that only
exists once Phase 3 ships (the audit's own Priority 5 section states this: "'As fast or faster than
Rust' is unfalsifiable until the optimizer pipeline exists... Recommended plan shape: optimizer
milestone FIRST"). This boundary is drawn by the audit document itself, not invented by this plan —
stated here so the M6/M7 split has zero ambiguity for either plan's reviewer.

**Pre-existing, phase-untouched behavior claims re-verified at recon time (not carried forward
blind):** the audit's own Fable-verified finding that "TargetMachine opt level ≠ IR pass pipeline —
flipping the enum alone does NOT run mem2reg/SROA" (audit doc, Priority 5) is the exact claim ¶1
Terrain's "unverified theory" framing rests on for the spike's mem2reg attribution; it is recon-cited
directly against the audit doc's text, not assumed. Likewise the claim that
`state_machine.rs::default_target_machine` is already the single shared constructor for the
non-override branch is **verified** by direct reading of both call sites (¶1 Assumptions), not
inferred.

## 2. Mission

The Yinz compiler team replaces the hardcoded `OptimizationLevel::None` codegen pipeline with a real,
root-caused, safety-verified LLVM optimization pipeline for `ynz build` — after v0.3-M6's correctness
hotfixes merge to `main` — so that every performance claim about Yinz (concurrency, SoA, and beyond)
becomes falsifiable against measured evidence instead of a compiler that structurally never optimizes,
and the language's "Rust-level performance" positioning can be pursued on real numbers.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** Turn on real optimization safely: root-cause the ONE proven hazard (the spike's decimal128
SIGSEGVs) before any durable phase depends on an unverified theory about it, sweep exhaustively for
undiscovered siblings, and close the two adjacent bugs (the O0 stack-exhaustion ceiling, and honest
benchmark integrity) that would otherwise corrupt every measurement this milestone produces.

**Key outcomes:**
1. `ynz build` compiles through a real LLVM pass pipeline by default; `ynz build --no-optimize` is the
   documented escape hatch back to the old O0 behavior (mirrors `--no-auto-parallel`'s exact CLI/env
   threading pattern).
2. Every suspension/concurrency fixture — the full `ynz-driver` + `ynz-codegen` test suite (830+ tests
   pre-existing), the 6 spike-failing fixtures, and every sibling Phase 1's sweep finds — is GREEN under
   the new pipeline, proven by re-run, not asserted from the spike alone.
3. Real back-edge preemption ships as a NEW codegen poll-yield transform for state-machine functions
   (today only the CALL SITES exist at back edges — they target a documented no-op stub, so runtime
   preemption is currently ZERO, per audit finding P4-1; `ynz_rt_check_preempt` is a synchronous
   `extern "C"` callee and structurally cannot itself yield a Tokio task, so the real fix lives in
   codegen, never inside the stub) AND call-site preemption (likewise codegen-emitted poll-yield
   sites, never runtime-implicit magic) is EITHER shipped
   for real (fresh O2 measurement clears the threshold) OR honestly deferred with a registry entry —
   either way, [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
   "Scheduler Preemption Model" section states the TRUE, three-part shipped architecture: (a)
   SM-function back edges = codegen poll-yield (new, this milestone), (b) non-SM CPU-bound work =
   blocking-pool routing (already shipped, unaffected), (c) a named residual — CPU-heavy code inside a
   non-SM function that admission misses — closing the audit's P4-1
   doc-vs-reality gap honestly rather than asserting full coverage.
4. The O0 stack-exhaustion SIGSEGV (roadmap ledger row 439) is root-caused and fixed, unblocking honest
   hot-loop benchmarking (the `soa_calibration.rs` 131,072-visit cap can be reassessed).
5. A committed, reproducible O0-vs-optimized A/B benchmark suite AND an honestly-framed Rust-equivalent
   comparison suite exist and run clean in CI — reporting the TRUE measured number, whatever it is. If
   Phase 7's numbers fall short of "as fast as Rust," the Mission and this Key Outcome get reconciled to
   state that honestly (per [plan-source-of-truth.md](../../../../rules/plan-source-of-truth.md)'s
   execution-time reframe discipline) rather than buried or asserted away.
6. Every golden (`crates/ynz-codegen/tests/golden.rs` IR-text + object-SHA-256, and
   `examples/pirates-roster/expected_stdout.txt`) is regenerated and verified **stable across at least 2
   independent regeneration runs** — not a single-run commit (this repo has an existing, named failure
   mode for exactly that: the M4 audit's "stable across 5 runs" claim that was never actually
   CI-enforced; this plan does not repeat it).

**Definition of done.** All 6 outcomes above are met; the full CI matrix
(`cargo fmt --check && cargo clippy --workspace -- -D warnings && cargo test --workspace && cargo build
--workspace --release`) is green; the roadmap's Capability Ledger reflects this plan absorbing row 439
and shipping row 443, with rows 438/440/441/442 explicitly left unabsorbed and reasoned (§Roadmap
Reconciliation, Phase 8).

**Disciplined-initiative guidance.** When a literal step and reality diverge — inkwell 0.9.0 genuinely
can't expose `run_passes` cleanly, or the Phase 1 sweep finds a THIRD O0-reliant path nobody
anticipated, or a golden fails to stabilize across repeated runs — the fallback is the Purpose above:
**root-cause before fix, prove before ship, never paper over a suspension-frame or runtime-FFI-call
correctness question with a hopeful guess.** Any newly-discovered O0-reliance is a NEW risk row (surface
it), never a silent in-place patch. When genuinely uncertain whether a finding is in-scope for this
plan or belongs in Future Requirements, the test is: does it touch the pipeline's correctness or the
two absorbed bugs (R1–R2)? If not, name it and defer it — do not expand scope silently.

### 3.2 Concept

**Ordering note (hard-sequenced, not either-order):** Phase 4 Step 1 re-confirms the O0
stack-exhaustion crash **under Phase 3's already-flipped optimizer default**, since the failure
envelope may shift once real optimization is live — Phase 4 cannot honestly run before Phase 3
ships. Phase 3 → Phase 4 is therefore a **hard sequence**, not an either-order pair; this is also
what Phase 7's benchmarks need (an honest row-439 repro under the LIVE pipeline, not the stale O0
one).

Nine phases, strictly root-cause-before-fix ordered. **Phase 0** hard-gates the one net-new mechanical
assumption (does `inkwell` 0.9.0 expose enough LLVM 18 PassBuilder surface) before anything durable
depends on it. **Phase 1** root-causes the spike's actual failure mechanism and exhaustively sweeps for
siblings — producing a committed RED fixture set, not a fix. **Phase 2** fixes what Phase 1 found and
threads the single authoritative `TargetMachine` constructor (closing R4). **Phase 3** wires the real
pass pipeline through that one constructor, with the `--no-optimize` escape hatch. **Phase 4** fixes the
absorbed O0 stack-exhaustion bug (row 439) — root-caused independently of the optimizer question, but
its own Step 1 re-confirms the crash under Phase 3's now-live pipeline, so it is hard-sequenced AFTER
Phase 3, never before or in parallel. **Phase 5** regenerates every invalidated golden and re-runs the
full suite — the proof phase for outcomes 2 and 6. **Phase 6** resolves the preemption honesty question
(real call-site checks or a proper deferral). **Phase 7** builds the two benchmark suites (A/B,
Rust-comparison) now that rows R1/R2 are closed. **Phase 8** closes the loop on documentation, registry,
and roadmap reconciliation. Phases 0–2 gate everything after them; **Phase 3 then Phase 4 run in that
strict order** once 0–2 are green (see the Ordering note above); Phases 5–8 are strictly sequential
after 3 and 4.

### 3.3 Phases

#### Phase 0 — P0 Spike: inkwell / LLVM PassBuilder Feasibility

- **Task + purpose:** Prove, on a throwaway scratch module, that `inkwell` 0.9.0 (LLVM 18) can build
  and run a real optimization pass pipeline before any durable phase assumes it can. This is the
  [plan-spike-discipline](../../../../rules/plan-spike-discipline.md) Facet 1 hard gate for R3.
- **Steps:**
  1. Read `inkwell` 0.9.0's public API surface (via `cargo doc` or the vendored source under the Cargo
     registry cache) for `PassBuilderOptions`, `TargetMachine::run_passes`, or any equivalent new-PM
     entry point exposed at this pinned version.
  2. Write a throwaway scratch Rust binary (outside the crate tree, e.g.
     `scratch/opt-pipeline-spike/`) that builds a minimal LLVM module (one function, one alloca, one
     dead store) and calls the discovered API with an `"default<O2>"`-style pass pipeline string (or
     whatever `inkwell` 0.9.0's actual signature requires).
  3. Confirm the resulting IR shows the dead store eliminated (proof the pipeline actually ran, not a
     no-op success return).
  4. Record the exact API shape (function name, signature, pass-pipeline string format) in this
     phase's scratch notes for Phase 3 to consume directly — do not re-discover it there.
- **STOP-conditions (hard gate):** RED if `inkwell` 0.9.0 exposes no usable pass-pipeline entry point at
  all (would require either an inkwell version bump — its own ADR-worthy decision, since a version bump
  changes the entire crate's LLVM binding surface — or a raw `llvm-sys`/C-API escape hatch). GREEN if a
  working call sequence is found and the dead-store elimination proof passes.
- **Exit criteria:** GREEN verdict recorded with the working API shape; scratch binary + its output
  persisted as a checked-in note (per plan-spike-discipline Facet 2 — the API shape is exactly the kind
  of artifact a later phase needs and must not be thrown away with the rest of the scaffolding); the
  scratch binary itself is NOT committed to the crate tree (throwaway).
- **Reviewer fan-out:** adversarial gate-checker (is the GREEN verdict genuinely proven — did the dead
  store actually disappear from the IR, not just "the call returned Ok").
- **Model tag:** `(coding, high, small)`

#### Phase 1 — Root-Cause the Spike Failures + Exhaustive Sibling Sweep

- **Task + purpose:** Replace the unverified "mem2reg" theory (`emit.rs:9961-9963`, `:10717-10719`)
  with a confirmed root cause via bisection, and exhaustively sweep for every other O0-reliant path —
  producing a committed RED fixture set that gates Phase 2's fix. **Do not fix anything in this phase.**
- **Steps:**
  1. Confirm M6 has merged to `main` (CCIR precondition — halt and report if not).
  2. Reproduce the spike's 2-line diff in a fresh worktree by applying the checked-in
     [`spike-o0-flip.patch`](./spike-o0-flip.patch) directly — do not re-derive it from memory. Cross-
     reference the 6/470 failing-fixture list in
     [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)'s
     "Phase-0 spike" section as the evidence of what the applied diff broke.
  3. Bisect the actual failing pass: compile the failing fixture
     (`v0_3_m3a_p1_ec_crossing_local_propagated_number`) at `-O0` and `-O2` via the LLVM `opt`/`llc`
     CLI tools directly (bypassing `ynz build`), diffing the generated assembly/IR at each pass-pipeline
     stage to find which specific pass changes behavior. Confirm or refute the mem2reg theory with this
     evidence — do not assert either way without the diff in hand (Paper-Trace: observed vs. expected
     vs. residual vs. hypothesis vs. evidence path, per this session's verification discipline).
     **CHECKPOINT** — root-cause hypothesis confirmed with a Paper-Trace and a minimal repro; sibling
     sweep (next steps) not yet started.
  4. Exhaustively grep every `extern "C"` runtime declaration the codegen crate calls into
     (`ynz_array_*`, `ynz_map_*`, `ynz_channel_*`, `ynz_arc_*`, drop-glue helpers,
     `ynz_rt_check_preempt`, `ynz_rt_spawn*`) and confirm each carries LLVM attributes correct for its
     REAL side-effect profile (no false `readnone`/`speculatable`/`nofree` on anything with an
     observable effect; correct `noalias`/`nocapture` where the ownership model guarantees it). This is
     the general form of R1 the narrow decimal128 spike only sampled.
  5. For each additional O0-reliant path found (beyond the 2 known comments), author a RED fixture that
     fails when compiled optimized, mirroring the 6 spike fixtures' shape.
  6. Commit the full RED fixture set (spike's 6 + any new ones) as failing/ignored tests gating Phase 2
     — this is the R1 mitigation's proof artifact.
- **Exit criteria:** a confirmed, evidenced root cause (not a theory); a complete, committed RED fixture
  set; zero fixes attempted in this phase.
- **Reviewer fan-out:** code-reviewer (sweep completeness — did it actually cover every runtime
  declaration, not a sample); adversarial gate-checker (does the root-cause claim survive an
  independent read of the bisection evidence); design-doc-alignment reviewer (does the finding
  contradict anything [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) or
  [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
  already commits to).
- **Model tag:** `(coding, high, medium)`

#### Phase 2 — Fix the Root Cause + Thread the Single Authoritative `TargetMachine` Constructor

- **Task + purpose:** Fix exactly what Phase 1 root-caused (attribute corrections and/or frame-handling
  hardening) and eliminate R4 by extending `state_machine.rs::default_target_machine` into the ONE
  authoritative constructor both call sites use — never a second, independently-configured
  `TargetMachine` creation.
- **Steps:**
  1. Implement the fix Phase 1's evidence points to (attribute corrections on the affected runtime
     declarations, and/or explicit frame-slot handling that survives the confirmed pass).
  2. Re-run the full RED fixture set from Phase 1; confirm every one now passes optimized.
     **CHECKPOINT** — RED set green; root cause fix committed; pipeline-wiring work not yet started.
  3. Extend `default_target_machine` to accept the pipeline configuration Phase 3 will need (a
     parameter, not a second global), without yet turning optimization on by default (that is Phase 3's
     job — keep this phase's diff scoped to the constructor's SHAPE, not its default value).
  4. Delete the inline `TargetMachine` construction in `emit.rs:879`; route it through
     `default_target_machine` instead — grep-verify zero remaining independent construction sites.
  5. Re-run the full pre-existing test suite (830+ tests) to confirm the constructor threading
     introduced no behavior change yet (still O0 by default at this point in the sequence).
     **CHECKPOINT** — single authoritative constructor verified (grep + green suite); ready for Phase 3
     to flip the actual default.
- **Exit criteria:** RED set green; exactly one `TargetMachine` construction call site in the crate
  (grep-verified); full suite green; no default behavior change yet.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (the RED-set-green claim); a dedicated
  grep-verification for the single-constructor invariant (can be folded into code-reviewer's pass).
- **Model tag:** `(coding, high, large)` — scale=large (attribute audit + constructor migration);
  checkpoint marks mandatory.

#### Phase 3 — Wire the Real Optimization Pass Pipeline

- **Task + purpose:** Turn the pipeline on by default through the Phase 2 constructor, with a
  `--no-optimize` escape hatch and a `YNZ_OPT_FORCE` dev/bench override, using Phase 0's proven API
  shape.
- **Steps:**
  1. Using Phase 0's recorded API shape, implement the real pass-pipeline call inside
     `default_target_machine` (or a sibling function it calls), defaulting to a real optimizing tier
     (target: LLVM's `default<O2>`-equivalent, or `Os` if compile-time budget demands it — pick ONE,
     record the choice and why).
  2. Add a `--no-optimize` CLI flag to `ynz build`, threaded through the salsa barrier via an env var —
     mirror `--no-auto-parallel`'s exact existing pattern in `crates/ynz-driver/src/main.rs` (same
     plumbing shape, new name).
  3. Add a `YNZ_OPT_FORCE` env override for the benchmark harness (Phase 7), mirroring `YNZ_SOA_FORCE`'s
     precedent — dev/test-only, never a shipped user surface.
  4. Measure `ynz build --release` wall-clock on `examples/pirates-roster/` before/after this phase;
     confirm the <10% increase budget (existing roadmap risk) — this is R5's mitigation proof.
  5. Run the full pre-existing test suite (830+ tests) plus Phase 1/2's RED set under the NOW-DEFAULT
     optimized pipeline; every one must be green.
     **CHECKPOINT** — default pipeline live, compile-time budget proven, full suite green.
- **Exit criteria:** `ynz build` optimizes by default; `--no-optimize` proven to reproduce the exact old
  O0 output byte-for-byte; compile-time budget met; full suite green.
- **Reviewer fan-out:** code-reviewer; design-doc-alignment reviewer (the CLI-flag pattern vs.
  `--no-auto-parallel` precedent); adversarial gate-checker (does `--no-optimize` genuinely reproduce
  old behavior, or does it silently differ).
- **Model tag:** `(coding, high, medium)`

#### Phase 4 — Fix the O0 Stack-Exhaustion SIGSEGV (Absorb Ledger Row 439)

- **Task + purpose:** Root-cause and fix the general hot-loop stack-exhaustion SIGSEGV at ~4.19M
  total loop-visits (reproducible identically in AoS and SoA modes) — root-caused independently of
  the optimizer question, but **hard-sequenced after Phase 3** (never in parallel or before): Step 1
  re-confirms the crash under Phase 3's now-live optimized pipeline before root-causing it, since the
  failure envelope may shift once real optimization is on. Gates honest benchmarking (R2). This is the
  plan's Patrick-signed absorption of roadmap ledger row 439.
- **Steps:**
  1. Reproduce the crash via the existing characterization in `soa_calibration.rs`'s header comment and
     the M5 plan's Future Requirements #13 / risk E13 (starting point, not settled fact — confirm it
     still reproduces under this plan's already-flipped optimizer default from Phase 3, since the
     failure envelope may shift).
  2. Root-cause via the per-iteration `alloca` stack-growth theory (loop-body frame lifetime at
     O0/optimized) — confirm with a minimal repro and a stack-size measurement across increasing visit
     counts, not assumption.
  3. Implement the fix (likely: hoisting or reusing the loop-body frame instead of a fresh alloca per
     iteration, or an explicit stack-probe/growth strategy).
  4. Author a stress regression fixture that runs well past the old ~4.19M-visit crash envelope and
     asserts a correct checksum with no crash.
  5. Re-evaluate (do not blindly raise) the `soa_calibration.rs` 131,072-visit cap now that the
     underlying bug is fixed — raise it only with fresh evidence of a new safe ceiling, and note the
     change in that file's own header comment.
- **Exit criteria:** stress fixture passes at ≥10x the old crash envelope; `soa_calibration.rs`'s cap
  reassessed with evidence; full suite still green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (does the stress fixture actually
  exceed the old envelope, not just claim to).
- **Model tag:** `(coding, high, medium)`

#### Phase 5 — Regenerate Goldens + Full Suite Re-Verification

- **Task + purpose:** Prove Key Outcomes 2 and 6 — every golden regenerated, every fixture green,
  stability proven across repeated runs (not a single-commit claim).
- **Steps:**
  1. Regenerate every `crates/ynz-codegen/tests/golden.rs` IR-text and object-SHA-256 snapshot under
     the new default pipeline (the file's own doc comment states it auto-regenerates on first run —
     use that mechanism, review every diff by hand, do not blind-accept).
  2. Regenerate `examples/pirates-roster/expected_stdout.txt` via its own
     `expected_stdout.txt.regenerate.sh` — confirm the stdout content is IDENTICAL to the pre-M7
     baseline (the optimizer must not change observable program behavior; if stdout differs, that is a
     correctness bug, not an expected regeneration — halt and investigate before accepting).
     **CHECKPOINT** — first-pass regeneration complete; stability proof (next steps) not yet run.
  3. Re-run golden generation a SECOND independent time (fresh process invocation, not a repeated
     assertion inside one run) and diff against the first regeneration — every byte must match. This
     is the stability proof this plan's Safety invariant requires, closing the gap the M4 audit's
     unenforced "stable across 5 runs" claim left open.
  4. Run the FULL pre-existing test suite (830+ tests), the Phase 1/2 RED set, and the Phase 4 stress
     fixture together — one combined green run.
  5. Run the existing cross-implementation consistency harness
     (`crates/ynz-driver/tests/cross_impl_consistency.rs`, `--no-auto-parallel` vs. default) under the
     new pipeline — confirm identical stdout/stderr/exit-code, now ALSO across `--no-optimize` vs.
     default-optimized (extend the harness's assertion matrix to include this new axis).
     **CHECKPOINT** — full suite + stability proof + cross-implementation matrix all green; ready for
     documentation/demo sign-off.
- **Exit criteria:** all goldens regenerated and stable across 2 independent runs; `pirates-roster`
  stdout byte-identical to pre-M7 baseline; full suite green; cross-implementation matrix covers the
  new `--no-optimize` axis.
- **Reviewer fan-out:** code-reviewer; test-quality (does the stability proof actually re-invoke a
  fresh process, not just re-assert the same run); adversarial gate-checker (the cross-implementation
  matrix's new axis).
- **Model tag:** `(coding, high, large)` — scale=large (this is the whole-suite proof phase);
  checkpoint marks mandatory.

#### Phase 6 — Preemption: Codegen Back-Edge Poll-Yield + Call-Site Re-Measurement Decision

- **Task + purpose:** Ship REAL back-edge preemption via a CODEGEN transform (not "inside
  `ynz_rt_check_preempt`" — that framing was architecturally wrong and was corrected in this plan's
  own authoring per a personal plan-audit finding: a synchronous `extern "C"` callee cannot yield the
  enclosing Tokio task by itself; see the Design-Doc Alignment divergence 1 above for the full
  correction and [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)
  "M7-plan addendum" for the source finding). The true mechanism: codegen turns a qualifying loop back
  edge INSIDE a state-machine function into a poll-yield suspension point (store `resume_point`, flush
  crossing locals via the existing suspension machinery, return `Pending`); `ynz_rt_check_preempt`
  becomes a cheap, synchronous budget CHECK the codegen-emitted branch consumes, never the yield
  itself. Non-SM (plain synchronous) functions can NEVER cooperatively yield this way — their
  protection is the EXISTING CPU-admission routing to the blocking pool; this phase names that
  residual explicitly rather than silently. Also re-measure call-site check cost under the NOW-real O2
  pipeline (the 1190% figure was measured at O0, where nothing inlines — wrong-tier evidence); ship
  call-site checks (likewise codegen-emitted poll-yield sites, never runtime magic) if the fresh number
  clears an explicit threshold, else record a proper four-field deferral with a registry entry. Update
  [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
  "Scheduler Preemption Model" section to state the TRUE three-part shipped architecture — closing the
  audit's P4-1 doc-vs-reality gap honestly either way. **R8 (¶1 Risk Assessment) governs this phase's
  correctness hazard** — its HIGH residual carries an unsigned RISK OVERRIDE that must be signed by
  the orchestrator/Patrick before Step 2 (implementation) begins.
- **Steps:**
  1. **DESIGN step (gates everything after it).** Specify the codegen back-edge poll-yield transform
     for SM functions in a written design note, covering: (a) WHICH loops qualify — loop back edges
     INSIDE a state-machine (wait-containing) function only; (b) WHAT the yield emits — a
     `resume_point` store + a crossing-local flush, both via the EXISTING authoritative suspension
     machinery (`store_resume_point`/`flush_var_slot_to_frame`, `state_machine.rs`/`emit.rs` — reuse,
     never a second, parallel frame-flush implementation, per
     [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md)) + a `Pending` return,
     with the resume path reloading and continuing the loop; (c) the BUDGET mechanism — a cheap
     runtime counter/time check inside `ynz_rt_check_preempt` deciding WHETHER to yield at this back
     edge (the check legitimately lives in the runtime function and returns a bool; the YIELD itself is
     codegen's, never the runtime's); (d) explicitly what happens to loops in NON-SM functions —
     NOTHING new: they have no back-edge yield mechanism at all, and their existing protection is
     CPU-admission routing to the blocking pool (already shipped) — name this residual in the design
     note, do not silently omit it.
     **CHECKPOINT** — design note written, covering all four points above; implementation not yet
     started.
  2. Implement the codegen transform (turn a qualifying SM-function loop back edge into a poll-yield
     suspension point per the Step 1 design note) and implement `ynz_rt_check_preempt` as the real,
     cheap, synchronous budget-check-and-decide function (returns a bool; performs no yield itself) —
     wired to the already-emitted back-edge call sites (`emit.rs:12356-12365`).
  3. Author the starvation-proof fixture set (R8's committed RED-fixture mitigation, per the ¶1 Risk
     Assessment): (a) a hot CPU-bound-loop-with-no-function-calls fixture placed INSIDE a
     state-machine (wait-containing) function — the exact starvation shape
     [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
     preemption section exists to prevent — and confirm another task on the same worker gets scheduled
     time under the new real back-edge poll-yield; (b) a companion fixture with the IDENTICAL hot loop
     placed INSIDE a plain (non-SM, non-wait-containing) function, and confirm it is NOT preempted by
     this phase's new mechanism — relying instead on the existing CPU-admission blocking-pool routing —
     documenting the residual explicitly as a passing, expected-behavior fixture, not a silent gap.
     **CHECKPOINT** — real back-edge poll-yield implemented and fixture-proven (both the SM-positive
     case and the documented non-SM residual); call-site re-measurement work not yet started.
  4. **Pre-register the acceptance threshold BEFORE measuring** (per no-duct-tape's proof-before-ship
     discipline: decide the bar, then look) — e.g. "call-site check overhead must be ≤X% on a
     representative call-heavy microbenchmark under the Phase 3 default pipeline."
  5. Add call-site preempt-check emission (mirroring the existing back-edge emission pattern — likewise
     a codegen-emitted poll-yield site, never runtime-implicit magic) behind a
     compile-time toggle; measure the overhead on the pre-registered microbenchmark under the default
     optimized pipeline.
     **CHECKPOINT** — pre-registered threshold set and the fresh O2 measurement taken; the ship/defer
     decision (next step) not yet made.
  6. **Decision (measurement-gated, R6's mitigation):** if the fresh number clears the pre-registered
     threshold, ship call-site checks unconditionally (matches
     [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
     original lock exactly — no divergence to record). If it does NOT clear the threshold, do not ship
     them; author the four-field deferral (WHAT/WHY/COST/TRIGGER) plus a `[[deferred_language_feature]]`
     registry entry (name TBD at this step, e.g. `preempt-callsite-checks`) — closing audit finding P4-1
     honestly either way.
  7. Update [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
     "Scheduler Preemption Model" section to state the TRUE, three-part shipped architecture: (a)
     SM-function back edges = codegen poll-yield (new, this milestone), (b) non-SM CPU-bound work =
     blocking-pool routing (already shipped, unaffected), (c) the named residual — CPU-heavy code
     inside a non-SM function that admission misses — plus call-site checks either
     shipped-and-described or deferred-with-a-registry-citation.
- **Exit criteria:** the Step 1 design note is written and covers all four points; the codegen
  transform reuses the existing authoritative suspension machinery (grep-verified — no second,
  parallel frame-flush implementation introduced); back-edge poll-yield is real and fixture-proven for
  BOTH the SM-positive case and the documented non-SM residual; the call-site decision is
  measurement-gated with a pre-registered threshold (not measured-then-rationalized); the design doc
  states the true three-part shipped architecture, not aspiration; R8's RISK OVERRIDE is signed before
  Step 2 begins.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (R8: does the RED-fixture set — both
  the SM-positive and non-SM-residual cases — genuinely prove frame-layout correctness before merge,
  not merely asserted; does the transform genuinely reuse the authoritative suspension machinery
  rather than re-deriving a parallel one); design-doc-alignment reviewer (doc now matches reality,
  either direction, and states the true three-part architecture); docs-consistency reviewer (the
  deferral's registry entry, if any, is well-formed).
- **Model tag:** `(coding, high, medium)` — quality-bar raised from `standard` to `high` because this
  is now a frame-layout-affecting codegen transform in R8's silent-miscompile hazard class (matching
  Phase 1/2's quality bar for the same hazard family); 7 steps trips the >5-step checkpoint trigger;
  checkpoint marks required (after Step 1, after Step 3, after Step 5).

#### Phase 7 — Benchmark Suites: O0-vs-Optimized A/B + Rust-Equivalent Comparison

- **Task + purpose:** Build the honest evidence base this whole milestone exists to produce — extending
  `soa_calibration.rs`'s harness pattern (criterion, compiled-.ynz-binary driving, checksum + IR-content
  gates) rather than writing a new one from scratch (reusability).
- **Steps:**
  1. Extend the `soa_calibration.rs` pattern into a new `opt_pipeline_calibration.rs` bench: drives
     compiled `.ynz` workload binaries at `--no-optimize` vs. default (using `YNZ_OPT_FORCE` from
     Phase 3), across a small representative workload set (a CPU-bound loop, a shape-heavy allocation
     workload, the SoA physics-update workload already characterized in M5) — now that Phase 4 has
     fixed the visit-count ceiling, reassess whether the old 131,072 cap can be relaxed for this new
     harness (do not blindly copy the cap without re-checking).
  2. Every workload binary passes the same three gates `soa_calibration.rs` established: checksum
     tripwire, byte-identical stdout across modes (dual-mode oracle), and an IR-content gate confirming
     the optimized binary's `.ll` actually differs from the unoptimized one (proof the pipeline ran, not
     a silent no-op — mirrors the M3d silent-decline tripwire class this repo has been burned by
     before).
     **CHECKPOINT** — A/B harness green, committed, with raw numbers recorded.
  3. Build the Rust-equivalent comparison suite: for each representative workload above, hand-author an
     idiomatic `cargo --release` Rust program doing equivalent work, run both, and report wall-clock
     honestly — document explicitly what is and is not comparable (e.g., Rust's `--release` LTO/codegen
     defaults vs. Yinz's chosen tier; startup/runtime init cost; any workload where the comparison isn't
     apples-to-apples, state why).
  4. Reconcile the Mission's "as fast or faster than Rust" framing against these real numbers per
     [plan-source-of-truth.md](../../../../rules/plan-source-of-truth.md)'s execution-time reframe
     discipline: if the numbers show parity or better, state that plainly; if they show a gap, rewrite
     the Mission/Key-Outcome-5 text to state the TRUE measured position and name the gap as a Future
     Requirement (never leave the headline overclaiming past what Phase 7 actually measured).
     **CHECKPOINT** — Rust-comparison suite green, numbers committed, Mission text reconciled if needed.
- **Exit criteria:** both harnesses committed and green in CI; every claimed number traces to a
  committed benchmark run, not a hand-wave; the Mission/Key-Outcome text matches reality.
- **Reviewer fan-out:** code-reviewer (harness correctness); docs-consistency reviewer (does the
  Mission text now match the Phase 7 numbers, per the reconciliation discipline); adversarial
  gate-checker (the IR-content gate and dual-mode oracle actually prove what they claim).
- **Model tag:** `(coding, standard, large)` — scale=large (two harnesses, cross-language comparison
  authoring); checkpoint marks mandatory.

#### Phase 8 — Documentation, Registry, and Roadmap Reconciliation

- **Task + purpose:** Close the loop — CHANGELOG, feature registry (if Phase 6 deferred call-site
  checks), and the roadmap's milestones list + **BOTH** Capability Ledger tables (add M7, reconcile
  rows 438–443 per §Roadmap Reconciliation below).
- **Steps:**
  1. Add a `### Milestone 7: v0.3-M7 — Optimizer Pipeline` section to
     [`roadmap.md`](../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md) (mirroring the existing
     per-milestone section shape) and append `v0-3-m7-optimizer-pipeline` to the roadmap frontmatter's
     `milestones` list.
  2. **The roadmap carries TWO duplicate Capability Ledger tables** (a pre-existing, pre-M7 condition,
     not something this plan introduces): `## Capability Ledger (SSOT for capability → milestone
     ownership)` at roadmap.md's line ~365, and `## Capability Ledger` at roadmap.md's line ~417
     (merged 2026-07-01 from the pre-migration companion `capability-ledger.md` file — the roadmap's
     own text says so at that heading). Both currently carry byte-identical rows 438–443. **Update BOTH
     tables in lockstep, in this same step** — the sibling v0.3-M6 plan independently commits to
     updating both tables for its own rows, so this is an established, shared convention for this
     roadmap, not new ceremony invented here. In each table: mark ledger row 439 (general hot-loop O0
     stack-exhaustion) **shipped by M7**; mark ledger row 443 (LLVM optimization pass pipeline)
     **shipped by M7**; leave rows 438 (authoritative-derivation write-time guard), 440 (stale-archive
     ABI-version-checked embedding), 441 (codegen ICE: bare int literal into `number`), and 442
     (selective hot-field-only element materialization) **unchanged and explicitly annotated as NOT
     absorbed by M7**, each with the one-line reason from §Roadmap Reconciliation below. A diff that
     updates only one of the two tables is an incomplete Phase 8 — grep both headings to confirm parity
     before calling this step done.
  3. If Phase 6 deferred call-site preemption checks, add its `[[deferred_language_feature]]` registry
     entry to [`registry/features.toml`](../../../../registry/features.toml) now (if not already
     added at Phase 6 authoring time).
  4. CHANGELOG entry for the milestone; confirm no stray references to "compiles at O0" survive in any
     doc this milestone's grep sweep touches.
- **Exit criteria:** roadmap and registry reflect the true post-M7 state; no unreconciled ledger rows.
- **Reviewer fan-out:** docs-consistency reviewer; design-doc-alignment reviewer (final check that
  nothing in this plan's execution silently contradicted a cited design doc without being surfaced).
- **Model tag:** `(general/mechanical, floor, medium)`

### 3.4 Coordinating Instructions

- **Sequencing:** Phases 0–2 gate everything after them (R3, R1, R4 must be closed before the default
  flips). **Phase 3 then Phase 4 run in that strict, hard-sequenced order** once 0–2 are green — Phase
  4 Step 1 re-confirms the O0 stack-exhaustion crash under Phase 3's now-live optimized pipeline, so
  Phase 4 cannot honestly start before Phase 3 ships (this is also what Phase 7's benchmarks need: an
  honest row-439 repro under the LIVE pipeline, not the stale O0 one). Phases 5–8 are strictly
  sequential after 3 and 4.
- **Verify-before-complete gate (every phase):** the phase's own exit criteria must be independently
  re-run by the closing executor, not just narrated as done — this repo's own precedent (the M4 audit's
  unenforced "stable across 5 runs" claim) is exactly the failure this plan's Phase 5 stability proof
  exists to not repeat elsewhere.
- **CCIR — surface immediately, mid-flight, never silently absorb:**
  1. M6 has not merged to `main` when Phase 1 is dispatched — halt, do not proceed on unmerged
     correctness fixes.
  2. Phase 0 returns a RED spike verdict (inkwell 0.9.0 cannot expose a usable pass-pipeline entry
     point) — halt for re-design (an inkwell version bump or a raw-`llvm-sys` escape hatch is itself a
     new decision requiring its own review, not a quiet substitution).
  3. Any newly-discovered O0-reliant path during Phase 1's sweep beyond the two known comments — each
     gets its own RED fixture and risk-row treatment before Phase 2 attempts a fix; never fixed
     silently alongside the known cases.
  4. Any NEW risk scoring HIGH/EX-HIGH during execution — surfaced unsigned, per the risk engine's
     override gate; never self-signed by an executor.
  5. Any golden or fixture that fails to stabilize across the Phase 5 repeated-run proof — this is a
     correctness signal (nondeterministic codegen), not a flaky-test annoyance; halt and investigate.
- **Design-doc alignment during execution:** if any phase's finding contradicts
  [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
  or [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md), state it as
  "design doc `X` says A; this plan does B because `<reason>`" and surface for sign-off — never
  silently follow the plan over the doc.

## Invariants This Milestone Must Preserve

### Safety

- The spike's 6 failing fixtures (decimal128/EC-crossing paths) AND every sibling fixture Phase 1's
  exhaustive sweep discovers pass GREEN under the real optimized pipeline before Phase 3 flips the
  default — testable via the committed RED fixture set (Phase 1 exit criteria; Phase 2 exit criteria).
- No new silent-miscompile class survives the milestone: every audited `ynz_array_*`/`ynz_map_*`/
  `ynz_channel_*`/`ynz_arc_*`/drop-glue/`ynz_rt_check_preempt`/`ynz_rt_spawn*` call site carries LLVM
  attributes correct for its real side-effect profile (Phase 1 step 4), and the full 830+-test
  `ynz-driver` suite plus the Phase 1/2 RED set plus the Phase 4 stress fixture are green TOGETHER
  (Phase 5 step 4) — never proven piecemeal.
- `ynz build --no-optimize` reproduces the exact pre-M7 O0 output byte-for-byte (Phase 3 exit
  criteria) — the escape hatch is PROVEN equivalent, not merely assumed close.
- The cross-implementation consistency harness (`--no-auto-parallel` × `--no-optimize` × default) shows
  byte-identical stdout/stderr/exit-code across every mode combination (Phase 5 step 5) — across
  wait/background/EC suspension paths, not just the happy path.
- Reproducible-build invariant: two independent, fresh-process golden-regeneration runs on identical
  input produce byte-identical output (Phase 5 step 3) — closing the M4 audit's unenforced "stable
  across 5 runs" gap (R7 below covers the risk this invariant is proving down to LOW).
- Exactly ONE `TargetMachine` construction call site exists in the crate after Phase 2 (grep-verified,
  Phase 2 exit criteria) — the R4 dual-constructor drift class cannot exist with a single source.

### Performance

- **Auto-promotion analysis (mandatory per [`auto-promotion.md`](../../../rules/auto-promotion.md)):**
  this milestone creates **NO new auto-promotion candidate.** The optimizer pipeline is a backend/
  codegen-tier change applied UNIFORMLY to every compiled program — there is no per-construct
  "stricter form fits in some cases" proof the compiler makes case-by-case (contrast `array→fixed` or
  `let→const`, which are per-binding proofs with a losing alternative form). `--no-optimize` is a
  global CLI escape hatch, not a per-site override direction in the auto-promotion sense. Considered
  and declined — stated explicitly so reviewers know it was evaluated, not skipped.
- O0-vs-optimized A/B expectations (Phase 7): the compiled workload set must show a MEASURABLE
  difference between `--no-optimize` and default-optimized binaries, proven by an IR-content gate
  (Phase 7 step 2) confirming the pass pipeline actually ran — not asserted from a silent no-op. The
  actual magnitude reported is whatever Phase 7 measures, honestly, with no pre-committed number.
- Compile-time budget: `ynz build --release` wall-clock on `pirates-roster` stays within the existing
  <10% roadmap budget (Phase 3 step 4, R5's mitigation proof).
- Golden-stability doubles as a determinism guarantee: the pass pipeline must not introduce
  nondeterministic codegen ordering that would break the byte-identical 2-independent-run gate (R7).

### Teaching

- No new user-facing diagnostic class is anticipated from wiring the optimizer pipeline itself
  (Phases 0–3, 5, 7 are backend/tooling work with no new parse/typeck-visible surface). `--no-optimize`'s
  CLI help text follows the existing `--no-auto-parallel` precedent's shape (mirrored, not reinvented —
  Phase 3 step 2).
- If Phase 1's sweep or Phase 4's fix unexpectedly surfaces a genuinely new compile-error class, it
  follows the WHAT/WHAT-INSTEAD/WHY format per Golden Rule 11 and earns a gallery entry (see Demo &
  Error Gallery below) — named as a live possibility per the CCIR above, never assumed away.
- Phase 6 rewrites [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)'s
  "Scheduler Preemption Model" section to state the TRUE, three-part shipped architecture — (a)
  SM-function back-edge codegen poll-yield (new), (b) non-SM CPU-bound blocking-pool routing (already
  shipped), (c) the named non-SM-admission-miss residual — regardless of whether Phase 6 ships
  call-site checks or records a deferral for that one sub-piece; this is a
  documentation-teaching fix (not a compiler diagnostic), held to the same never-silently-diverge
  standard as any other diagnostic.

### Runtime Dependencies

- **This milestone adds NO new runtime dependency**, stated explicitly. The optimizer pipeline runs
  entirely at compile time (inside `ynz build`, via `inkwell`'s LLVM PassBuilder surface, Phase 0/3);
  it adds no new library, syscall, allocator, or scheduler dependency to COMPILED Yinz binaries.
- Real back-edge preemption (Phase 6) uses the EXISTING Tokio dependency already present in
  `ynz-runtime` — not a new one. The mechanism is a CODEGEN transform (poll-yield suspension points at
  qualifying SM-function loop back edges, reusing the existing `store_resume_point`/
  `flush_var_slot_to_frame` suspension machinery and Tokio's `Poll::Pending`/waker plumbing already
  wired for `wait`), never new logic added inside `ynz_rt_check_preempt` itself — that function is
  reduced to a cheap, synchronous budget CHECK the codegen-emitted branch consumes; it never performs
  the yield. Non-SM (plain synchronous) functions have no back-edge yield mechanism at all — their
  existing protection is CPU-admission routing to the blocking pool (already shipped); the named
  residual is CPU-heavy code inside a non-SM function that admission misses (see Future Requirements /
  Revisit #8).
- `--no-optimize` (CLI flag) and `YNZ_OPT_FORCE` (dev/bench env var): dev/CLI-only surfaces, no runtime
  dependency of their own.

### Kernel-Mode Behavior

- `ynz build --kernel` routes through the SAME optimizer pipeline as the default build — stated
  explicitly: **no kernel-mode-specific optimizer behavior is introduced.** The LLVM pass pipeline is
  orthogonal to the `--kernel` runtime-mode gate (kernel mode restricts allocator/scheduler USE at
  typeck/emit time; it says nothing about which backend passes run over already-emitted IR).
  `--kernel` + default-optimized and `--kernel` + `--no-optimize` both compile; the existing kernel-mode
  compile-error gates (heap-allocating shapes, etc.) fire identically regardless of optimization tier.
- Real back-edge poll-yield preemption (Phase 6): unaffected by kernel mode — `background`/scheduler
  behavior is already unavailable in `--kernel` builds without a user-supplied primitive
  ([`IMP-no-runtime-mode.md`](../../../../docs/internal/implementation/IMP-no-runtime-mode.md)),
  unchanged by this plan. The non-SM CPU-admission residual named above is likewise orthogonal to
  `--kernel` mode — kernel-mode compilation already restricts `background`/scheduler use entirely, so
  the residual applies only to non-kernel builds.

### Demo & Error Gallery

- **The `examples/pirates-roster/entrypoint.ynz` byte-exact golden staying GREEN under the new pipeline
  IS this milestone's demo obligation** — Phase 5 step 2 regenerates and byte-diffs
  `expected_stdout.txt` against the pre-M7 baseline; any divergence is treated as a correctness bug, not
  an expected regeneration (Phase 5's own exit criteria). **No new demo section is required**: this
  milestone changes the compiler's BACKEND, not the language surface — there is no new syntax/feature
  for `pirates-roster` to demonstrate in context.
- **No new compile-error class is anticipated to ship from this milestone — stated explicitly, per the
  M5 precedent of an explicit recorded "zero new classes" note rather than silence.** Phases 0–3, 5, 7
  are backend/tooling work with no new diagnostic surface; Phase 4's fix removes a crash rather than
  adding an error class; Phase 6 either ships a runtime behavior change (no new diagnostic) or records a
  registry deferral (not a diagnostic). **Therefore `examples/primantis-orders/` needs no new
  `m7_errors.ynz` gallery file.** If Phase 1's sweep or Phase 4's fix unexpectedly surfaces a genuinely
  new user-facing error class, that becomes a live CCIR (¶3.4) and a gallery file is authored at that
  point — the same "recorded note if zero new classes ship" pattern the M5 plan set precedent for.

### Feature Registry Entries

- The preemption mechanism update (Phase 6): if call-site checks ship for real, **NO new registry
  entry** (matches `IMP-no-function-coloring.md`'s original lock exactly — Phase 6 step 6). If
  deferred instead, exactly ONE new `[[deferred_language_feature]]` entry (name TBD at Phase 6
  execution time, e.g. `preempt-callsite-checks`), per the four-field deferral.
- `--no-optimize` (CLI flag) and `YNZ_OPT_FORCE` (dev/bench env var): **no registry entry** — mirrors
  the existing precedent already set by `--no-auto-parallel`/`YNZ_SOA_FORCE`, neither of which carry
  registry entries (CLI flags and internal test-only env vars are not language keywords, jargon,
  intrinsics, or diagnostic templates — the registry schema has no row kind for them). Stated
  explicitly so reviewers know it was considered, not forgotten.
- **Explicitly none** for the rest: no new keywords, banned_declaration_keywords, banned_jargon,
  primitive_intrinsics, type_attached_constants, diagnostic_templates, or muted_hint_domain entries —
  this milestone's work is entirely backend/codegen-tier with no new language surface.

## 4. Sustainment

- **Docker (this project's universal convention):** `docker compose run --rm dev cargo build -p
  ynz-codegen`, `docker compose run --rm dev cargo test --workspace`, `docker compose run --rm dev
  cargo clippy --workspace -- -D warnings`, `docker compose run --rm dev cargo bench -p ynz-driver
  --bench soa_calibration` (existing) and the new `--bench opt_pipeline_calibration` (Phase 7). No
  `-it`; every dispatch is non-interactive.
- **LLVM/inkwell:** LLVM 18 via apt (`LLVM_SYS_181_PREFIX`), `inkwell` pinned at `0.9.0`
  (`llvm18-1-prefer-dynamic` feature). Phase 0's spike works within this pin; a version bump is its own
  decision (CCIR item 2), never a quiet substitution mid-phase.
- **Reference artifact (read-only):** the original spike's 2-line O0→Default diff, preserved as a
  checked-in unified diff at [`spike-o0-flip.patch`](./spike-o0-flip.patch) (this plan's own directory)
  per plan-spike-discipline Facet 2 — Phase 1 reads it, does not recreate it from memory. The
  evidence of what that diff broke (the 6/470 failure list, direct-repro SIGSEGV) lives in
  [`.claude/audits/2026-07-04-concurrency-release-audit.md`](../../../audits/2026-07-04-concurrency-release-audit.md)'s
  "Phase-0 spike" section — the primary durable evidence Phase 1 corroborates against.
- **CI:** [`.github/workflows/ci.yml`](../../../../.github/workflows/ci.yml), Linux-only
  (`ubuntu-latest`). Every phase's exit criteria must keep `cargo fmt --check`, `cargo clippy --workspace
  -- -D warnings`, `cargo test --workspace`, and `cargo build --workspace --release` green.
- **Golden regeneration tooling:** `crates/ynz-codegen/tests/golden.rs` (auto-regenerates on first run,
  per its own doc comment) and
  `examples/pirates-roster/expected_stdout.txt.regenerate.sh`.
- **Sibling plan:** v0.3-M6 (correctness hotfixes, authored in parallel, not yet a plan file on disk) —
  this plan branches from `main` after M6 merges (CCIR item 1).

## 5. Command & Signal

- **Ownership:** each phase is picked up by whichever executor session the execute-plan conductor
  dispatches next; no named individual owner beyond Patrick's overall sign-off/release authority.
- **Succession:** standard plan-format succession — this `plan-id` + the session-id chain + checkbox
  state in this file. Phases 2, 5, and 7 (scale=large, checkpoint-marked) use `handoff-phase-<N>.md`
  per the [Handoff file convention](../../../../../.claude/docs/reference/REF-plan-format.md#handoff-file-convention)
  when a segment checkpoints.
- **Audit trail:** `audit.md`, sibling to this `plan.md` in whichever status-folder currently holds
  it (created at this amendment pass, under `paused/`; the status↔folder invariant moves the whole
  directory — `plan.md` + `audit.md` together — when status flips to `active`, never a path an
  executor hardcodes) — session log + FRAGO log, append-only. The roadmap's own `audit.md` receives
  the Phase 8 ledger-reconciliation entry as a separate append, not a duplicate of this plan's own
  record.

## Future Requirements / Revisit

1. **Selective hot-field-only element materialization** (roadmap ledger row 442 / M5 Future
   Requirements #15, FRAGO 020) — **WHAT:** SoA codegen computes `hot_fields` via `soa_candidate_query`
   but `soa_gather_into`/`array_elem_get_into` never consume it; every field is gathered unconditionally.
   **WHY not absorbed here:** a different concern (SoA-specific gather selectivity, not the backend pass
   pipeline) — folding it in would mix two unrelated fix classes into one review surface and blow up
   this plan's phase count. **COST:** ~1 dedicated session + E3/E6/E9-style re-review per its own
   ledger text (every full-element consumer must be re-audited). **TRIGGER:** before or alongside any
   future optimization-pipeline milestone — since this plan IS that milestone, the trigger is live now;
   recorded here as a deliberate, reasoned non-absorption for Patrick's own call on sequencing, not a
   silent drop.
2. **Build/release-tooling: ABI-version-checked runtime archive embedding** (roadmap ledger row 440) —
   **WHAT:** `ynz-driver/build.rs` embeds the runtime archive with no ABI/version check; a stale
   cross-profile archive can silently miscompile. **WHY not absorbed:** unrelated bug class (build/release
   tooling), already has a cheap operational mitigation in place (rebuild-from-clean, per the M5 FRAGO
   018 precedent cited in this repo's own `no-duct-tape.md`). **COST/TRIGGER:** unchanged from the
   ledger's own text — the next milestone touching `ynz-runtime`'s ABI, or external binary distribution.
3. **Codegen ICE: bare int literal into a `number`-typed slot** (roadmap ledger row 441, ELEVATED
   priority per Patrick's 2026-07-04 triage — a real user-facing crash on common valid code) — **WHAT:**
   `store`/`store_field`'s `Type::Number` arm assumes a decimal128-pointer representation while
   `Expr::IntLit` lowers to a raw `i64`; typeck admits the coercion, codegen panics. **WHY not absorbed:**
   pure pre-existing literal-lowering bug, orthogonal to the optimizer pipeline and to concurrency
   codegen. **COST/TRIGGER:** unchanged from the ledger's own text — next milestone touching
   numeric-literal codegen, or immediately if a real user hits it. Likely belongs to v0.3-M6 or a
   dedicated hotfix given its ELEVATED classification; not this plan's charter.
4. **Authoritative-derivation write-time guard** (roadmap ledger row 438) — **WHAT:** a mechanical,
   write-time hook catching a second independent derivation of an already-authoritative value before it
   lands. **WHY not absorbed:** a `hook-author` artifact (process/tooling), unrelated to this milestone's
   codegen/perf charter. **COST/TRIGGER:** unchanged — Patrick's own prioritization call, or the next
   milestone-planning session touching compiler-pass/derived-constant work.
5. **`background.cpuBound` explicit override syntax** (concurrency-release-audit P4-2, MEDIUM — the
   auto-promotion "force-the-other-pick" direction documented in
   [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md)
   but never implemented, no registry entry) — **WHAT/WHY:** unrelated to the optimizer flip itself; a
   different capability (CPU-bound task-routing override). **COST:** small (spawn-site annotation +
   registry entry). **TRIGGER:** the next milestone touching `background`/task-routing surface — v0.3-M6
   or a future one; not this plan's charter.
6. **(Contingent) Preemption call-site checks deferred** — if Phase 6's fresh O2 measurement fails its
   pre-registered acceptance threshold, the four-field deferral (WHAT/WHY/COST/TRIGGER) and its
   `[[deferred_language_feature]]` registry entry land here at Phase 6 execution time, with the actual
   measured number cited as evidence — pre-registered as a live possibility so Phase 6 has a ready home
   rather than inventing one mid-flight.
7. **"As fast or faster than Rust" — honest gap, if any, from Phase 7's real numbers** — this plan does
   NOT assert full Rust parity as an achieved outcome; Phase 7 reports the true measured position. Any
   remaining gap (missing PGO, missing LTO, missing vectorization tuning, or any other concrete
   follow-on) becomes its own named future revisit once Phase 7's numbers exist — not asserted now,
   since asserting it before the measurement would be exactly the overclaim
   [plan-source-of-truth.md](../../../../rules/plan-source-of-truth.md) exists to prevent.
8. **Named residual: CPU-heavy code inside a non-SM (non-wait-containing) function that CPU-admission
   misses** — **WHAT:** Phase 6's back-edge poll-yield mechanism only exists for loops inside
   state-machine functions; a plain synchronous function's hot loop can never cooperatively yield this
   way, and its only protection is the existing CPU-admission routing to the blocking pool, which is a
   heuristic (admission), not a guarantee. **WHY not fixed now:** no cooperative-yield mechanism exists
   for non-SM functions in this language's design (per
   [`IMP-no-function-coloring.md`](../../../../docs/internal/implementation/IMP-no-function-coloring.md) —
   plain functions are, by design, synchronous and non-suspending); closing this residual would require
   either a new function-coloring-adjacent mechanism (a real design change, out of this phase's charter)
   or provably tightening admission's classification, neither of which this plan's scope covers.
   **COST:** unknown until scoped — likely a dedicated design session revisiting
   `cpu_admission.rs`'s classification boundary. **TRIGGER:** a real, reproduced starvation incident
   traced to a non-SM CPU-bound function that admission misclassified, or the next milestone
   revisiting `cpu_admission.rs`/scheduler design.

## Roadmap Reconciliation (executed at Phase 8; recorded here so the executor has zero ambiguity)

Per the roadmap's Capability Ledger — **the roadmap carries two duplicate tables with byte-identical
rows 438–443**: `## Capability Ledger (SSOT for capability → milestone ownership)` (roadmap.md line
~365) and `## Capability Ledger` (roadmap.md line ~417, merged 2026-07-01 from the pre-migration
`capability-ledger.md` companion file). **Phase 8 Step 2 updates BOTH tables in lockstep** — the
sibling v0.3-M6 plan independently commits to the same both-tables convention for its own rows, so
this is this roadmap's established practice, not new ceremony. The disposition below applies
identically to both tables (rows numbered by table order, unscoped rows only, 438–443):

| Row | Capability | This plan's disposition |
|---|---|---|
| 438 | Authoritative-derivation write-time guard | **NOT absorbed** — process/tooling, unrelated (Future Requirements #4) |
| **439** | **General hot-loop O0 stack-exhaustion ceiling fix** | **ABSORBED — Phase 4** |
| 440 | Build/release-tooling ABI-version-checked runtime archive | **NOT absorbed** — separate bug class, already mitigated (Future Requirements #2) |
| 441 | Codegen ICE: bare int literal into `number`-typed slot | **NOT absorbed** — unrelated correctness bug (Future Requirements #3) |
| 442 | Selective hot-field-only element materialization | **NOT absorbed** — different concern, reasoned non-absorption (Future Requirements #1) |
| **443** | **Add an LLVM optimization pass pipeline to `ynz build`** | **ABSORBED — this plan's core scope (Phases 0–3, 5–7)**, per Patrick's own note flagging it "the single most strategically important item on this list" given the Rust-level-performance positioning and flagship-concurrency framing |
