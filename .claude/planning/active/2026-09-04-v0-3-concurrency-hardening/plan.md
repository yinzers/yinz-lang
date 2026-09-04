---
name: "v0-3-concurrency-hardening"
plan-id: "2026-09-04-v0-3-concurrency-hardening"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: []
tier: "hasty"
tier-reason: "Concurrency is a blocking gate on using Yinz at all; every known blocker is traced to a named producer and fixed at that producer, not patched per symptom. Scope is fixed (four phases, non-negotiable), deferral is forbidden, ambiguity is decided upstream. Small committed work riding Patrick's settled order."
created_at: "2026-09-04"
updated_at: "2026-09-04"
metadata:
  type: "plan"
---

# HASTY PLAN: v0.3 Concurrency Hardening

## 1. Situation

**Conductor's hazard sweep** (facts grounded in M8 Phase 7 completion, Phase 8 fuzzer findings, and roadmap FR audit):

- **Data / heap-value corruption**: crossing locals sent into channels after suspension read back corrupted; capacity-forced-blocking sends read garbage; no scope-exit release pass exists. **Verified**: `crates/ynz-codegen/src/emit.rs` emits free calls at only three places — the background-arg glue, the channel element-glue table, and the spike trampoline — and `ynz_handle_free` has zero call sites. **NOT yet verified, and it is Phase 2 question (a), not a premise of this plan**: whether an ordinary `array`/`map`/`string` local's heap buffer therefore leaks at function return. Treat the shared-producer reading of these symptoms as the hypothesis under test, not as a finding. **Evidence**: M8 FR #11(a), FR #11(b), and `.claude/plans/parked.md` entry 49. **Neither defect is RED-pinned — there is no committed fixture for either**; `crates/ynz-driver/tests/fuzz_grammar/` holds only `README.md` and `mod.rs`. They reproduce by removing that file's two generator guards, and its doc comments there are the committed record of what was measured.

- **Money / irreversibility**: live defects in the released compiler that corrupt user data on send/receive. **Evidence**: M8 Phase 8 fuzzer, FRAGO 015 findings 1–2.

- **Prod-state**: `let h = background work()` creates an immortal task when `h` goes out of scope; no language-level stop mechanism, only a workaround (send a signal down a channel the task receives on). **Evidence**: M8 FR #3 (re-deferral), v0.3-M8 Phase 7's guard (Tier 3 lint), the design doc `IMP-no-function-coloring.md` "Task Cancellation."

- **Security / injection / PII**: not directly applicable (concurrency bugs are memory-safety, not injection). **N/A**.

- **Reversibility**: Phase 2's diagnosis and FRAGOs are non-reversible writes to this plan's audit.md. Phase 3's fixes are live in the compiler. No rollback path per design — rollback is a deferral to the hotfix branch if a FRAGO introduces a regression. **Not a floor on risk**, only an acknowledgment that this plan's discovery work is immutable once landed.

- **Concurrency / races**: the entire plan's domain. **Covered in all four phases**.

- **Auth**: N/A — Yinz has no auth surface.

- **External deps**: Tokio preemption model is already committed (M1 Architectural Decision locked). No new external dependencies introduced. **N/A**.

---

## 2. Mission

Trace every concurrency blocker discovered in v0.3-M2 through M8 to its named producer; cluster findings by ancestor; fix each producer once per cluster, never patch per symptom; retire all live exposure (data corruption, immortal tasks) and close all channels (M8 Phase 1–2's remaining deferrals); end state: programs using the default auto-concurrency produce correct output, and spawned tasks can be stopped by the language not by manual workarounds.

---

## 3. Execution

### 3.1 Intent & End State

**Purpose**: Concurrency works. A program using `wait` and `background` with channels produces correct output; an ordinary local is released when its scope exits; a handle binding's scope end stops the task (or schedules cancellation at its next suspension). Every finding from M2–M8's deferral sections and audit is the producer of itself, not a downstream symptom being patched elsewhere.

**Key tasks**:
1. Audit every concurrency plan (M2–M8) into one consolidated blocker register with durable homes.
2. Diagnose each blocker to its producer with evidence-backed probes; output FRAGOs one per producer.
3. Execute each FRAGO (steps will be defined by Phase 2's diagnosis; cannot pre-specify).
4. Land the scope-exit release pass as the general mechanism for cleaning up locals on all control-flow edges.

**What done looks like**: M2–M8 deferrals are either fixed or re-deferred with a new trigger in the roadmap's own registry; M8 Phase 8's two fuzzer-surfaced defects (neither RED-pinned today — pinning them is Phase 2's own output) are diagnosed to their producers and RED-pinned (no finding remains undiagnosed); Phase 7's re-deferral stands with Patrick's signature and a durable record of its evidence; scope-exit releases are emitted at every block exit, loop-iteration end, and function return for every local type (handles, arrays, maps, strings, channels, promoted maybe/union cells); background tasks can be stopped at language level, not by manual channel workarounds.

---

### 3.2 Concept

**Phase 1** — read-only audit of every M2–M8 plan's `## Future Requirements / Revisit` section and the roadmap's own Capability Ledger, producing one consolidated blocker list with durable homes (roadmap Capability Ledger, `registry/features.toml` `[[deferred_*]]` entries, `.claude/plans/parked.md`). M8 has been audited; M2–M7 have not. Output is a cross-referenced index, not code.

**Phase 2** — diagnosis phase answering two questions (exact questions given in Phase 2 task block): does an ordinary heap local EVER get freed at scope exit, and do several Phase 8 findings share one ancestor producer. Two probes (alloc counter, IR read) plus two optional follow-up probes (call-chain verify, owner-type classification). Output: one FRAGO per confirmed root cause, appended to this plan's `audit.md`. Phases 3 and 4 are **blocked** on Phase 2's FRAGO list — neither phase executes until the producer list is settled.

**Phase 3** — execute the FRAGOs. Phase 3's steps are **determined by Phase 2's output** and cannot be pre-specified here. Discipline: one RED pin per FRAGO before any fix; fix at the most upstream reachable point (the producer, not a symptom); one fix per ancestor, never one patch per symptom. One session minimum per FRAGO (diagnosis from Phase 2 already done).

**Phase 4** — the scope-exit release pass. Every local released at scope exit, with `background` handles as ONE ARM of the general mechanism — never a handle-only pass. Retires the Tier 3 lint from v0.3-M8 Phase 7's guard, flips the two pin tests in `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs`, and retires the `background-handle-cancel-injection` registry entry. **RISK GATE (HIGH, signed override required if Phase 4 overruns Phase 3)**: v0.3-M8 Phase 7 concluded this pass is "a milestone of its own, not a phase." Phase 4 may split into its own milestone once Phase 2 sizes it; decide at Phase 3 close before entering Phase 4.

**Handoff between phases**: Phase 1 output (blocker list index) is read into Phase 2. Phase 2 output (FRAGO list) is read into Phase 3 and Phase 4. No phase output loops back to an earlier phase.

---

### 3.3 Phases

#### Phase 1 — Complete the Blocker List

**Task & Purpose**: Audit every concurrency plan from v0.3-M2 through v0.3-M8 into one consolidated register. No code changes. Read-only.

**Rationale**: Fixing from an incomplete list is how a blocker gets discovered halfway through Phase 3. One source of truth (the roadmap's Capability Ledger) has been de-duplicated; two other homes (`.claude/plans/parked.md` and `registry/features.toml`'s `[[deferred_*]]` entries) may have diverged. M8 Phase 9 checked its own deferrals; M2–M7 have not.

**Steps**:
1. Read `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md` — search `Capability Ledger` section. Count rows, verify no `STALE` mark or duplicate entries.
2. For each M2–M7 plan in `.claude/planning/done/`, read the `## Future Requirements / Revisit` section. Record every entry by ID (FR #N) and plan (2026-XX-v0-3-mN).
3. Grep `.claude/plans/parked.md` for entries mentioning concurrency, channels, `background`, handles, `wait`, or scope-exit. Record by item number.
4. Grep `registry/features.toml` for `[[deferred_language_feature]]` and `[[deferred_tooling_feature]]` entries with `why` or `substitute` text mentioning concurrency. Record by name.
5. Cross-index the four sources (roadmap Ledger, M2–M8 FR sections, parked.md, registry). Flag every entry that appears in only one source (stale, needs home). Flag every entry that appears in two+ sources with different descriptions (reconcile).
6. Produce a consolidated list with four columns: (blocker ID, short what, durable home by source, FR cross-reference). State plainly any missing homes — do not invent one.

**Exit criteria**: No deferral in any M2–M8 plan lacks a durable home in the roadmap or registry. Every entry with multiple sources has identical descriptions, or explicit reconciliation is recorded. M2–M7 are audited by name; M8 is spot-checked (Phase 9 already audited it).

---

#### Phase 2 — Diagnosis

**Task & Purpose**: Answer two questions with evidence-backed probes. Output: one FRAGO per confirmed root cause. Read-only plus two targeted probes.

**Question (a)**: Does an ordinary heap local EVER get freed at scope exit? **Claim to verify**: `crates/ynz-codegen/src/emit.rs` emits free calls at only three places: the background-arg glue, the channel element-glue table, and the spike trampoline. `ynz_handle_free` has zero call sites. If true, an ordinary `array`/`map`/`string` local's heap buffer may never be freed when its function returns (the one producer of the scope-exit leak class).

**Probe (a)**: Grep `emit.rs` for `"ynz_array_drop"`, `"ynz_free"`, `"ynz_channel_free"`, `"ynz_map_drop"`, `"ynz_string_free"`, `"ynz_handle_free"`. Record every call site (file:line, function name, context). Verify (count, context match, zero for `ynz_handle_free`).

**Question (b)**: Do these share one ancestor: M8 FR #11(a) (crossing-local heap-channel-send corruption), M8 FR #11(b) (capacity-forced-blocking channel send garbage), M8 FR #9 (background-arg escape door #4), M8 FR #10 (`.copy()` catch-all aliasing), and parked items 32/33/34 (three `errors`-surface defects discovered in Phase 4)? **Suspicion**: several are one bug — heap values crossing task and channel boundaries, either aliased or freed prematurely.

**Probe (b1)**: Reproduce both defects from the fuzz harness, which is the only mechanism that produces them today. **There are no committed fixture programs for either — neither defect is RED-pinned anywhere in the tree** (`.claude/plans/parked.md` entry 49), and `crates/ynz-driver/tests/fuzz_grammar/` contains exactly two files, `README.md` and `mod.rs`. Do not go looking for a fixture; generate the programs. In `crates/ynz-driver/tests/fuzz_grammar/mod.rs`: remove `Builder::suspension_seen`'s reuse guard to surface (a) (a 256-seed sweep produced 35 findings, 28 of them silent wrong output at exit 0), and remove the `send_count`-versus-capacity floor to surface (b) (6–7 findings per 256 seeds). Read that file's doc-comment narrative at both guards FIRST — it is the committed record of what was already measured, including the corrected symptom rates from Phase 8 round 4. Then capture a minimal reproducing program for each, compile both with IR emit, and diff the send/receive paths against a passing baseline. Record: which codegen path each takes, and whether they share one. **Capturing those two minimal programs as committed RED pins is itself an output of this probe** — the absence of one is why this diagnosis costs a probe instead of a read.

**Probe (b2)**: Optional, if (b1) suggests a shared path. Read the shared path (e.g., channel-send lowering, crossing-local frame-slot machinery). Classify: does the path assume a local outlives a suspension, or does it assume the local is freed before entry? Is the assumption checked?

**Exit criteria**: Question (a) is settled with a verified count. Question (b) is settled with evidence-backed clustering. FRAGOs are written: one per confirmed producer, including (1) what the producer emits, (2) which Phase 1 blocker(s) it produces, (3) the evidence (probe results, code read, failed test), (4) any preconditions for Phase 3 fix (e.g., loom substrate must exist, must not run during a phase that also reverts code). **MET when FRAGOs are appended to this plan's `audit.md` and Patrick has reviewed them** (sign-off gate before Phase 3 starts; see Coordinating Instructions below).

---

#### Phase 3 — Execute the FRAGOs

**Task & Purpose**: Deliver one fix per FRAGO at its identified producer. Steps cannot be pre-specified because they depend on Phase 2's diagnosis.

**Discipline**:
- One RED pin per FRAGO before any fix (a failing test that will pass after the fix, used to verify the fix is real and not a no-op).
- Fix at the most upstream reachable point (the producer, not a symptom patch).
- One fix per ancestor; if two FRAGOs share a producer, one fix handles both.
- When a fix touches the crossing-local or frame-slot machinery, re-read `authoritative-derivation.md` before writing code — parallel implementations diverge silently.

**Exit criteria**: Every FRAGO has a RED pin and a fix. All pins flip to GREEN. The two M8 Phase 8 fuzzer findings (the genuine defects FRAGO 015 names) have either been fixed or re-deferred with explicit evidence in the plan's Future Requirements section. Clippy/fmt/test suite clean. **MET when Phase 3's work is merged to this plan's branch** (code review happens in parallel; see Coordinating Instructions).

---

#### Phase 4 — The Scope-Exit Release Pass

**Task & Purpose**: Emit free/release calls at every scope exit for every heap-backed local type (`array<T>`, `map<K,V>`, `string`, `channel<T>`, promoted `maybe<T>` and union cells, `background` handle bindings). Handles are one arm of the general mechanism.

**Key outputs**:
- Codegen emits free calls at block exit, loop-iteration end, function return (both normal `ret` and early `return`), and state-machine frame retirement (both free paths: caller's `free_frame` after Ready, and spawned parent's drop ladder).
- Transfer rule (sent / given away / returned) skips release on the transferred binding (ownership moved, no release needed).
- The Tier 3 lint from v0.3-M8 Phase 7's guard is retired (no longer needed; scope exit now actually stops the task).
- `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs::test_background_handle_stopped_when_scope_ends_after_child_starts` and `test_background_handle_stopped_when_scope_ends_before_child_runs` both flip from XPASS/XFAIL to PASS.
- `background-handle-cancel-injection` registry entry is retired.

**Exit criteria**: Every local has a corresponding release call in all exit paths. Tests pass. Fuzzer clean. M7's two handle-scope pins are green. **MET when Phase 4 is complete and merged** (code review in parallel).

**Risk gate (HIGH, signed override required if Phase 4 overruns Phase 3)**: M8 Phase 7 evidence concluded this pass is "a milestone of its own, not a phase — 1–2 sessions per the roadmap's never-drop-locals row." Phase 4 may exceed this plan's time budget and require being carved into its own milestone. Decision point: at Phase 3 close-out, size Phase 4 against the remaining budget. If Phase 4 > budget, escalate to Patrick for deferral approval (with a signed override, record the decision in this plan's Future Requirements, update the roadmap status, and defer the pass to its own v0.3-M9 milestone). If Phase 4 ≤ budget, proceed.

---

### 3.4 Coordinating Instructions

**Phase 2 is a gate**: No Phase 3 work starts until Phase 2's FRAGOs are written and Patrick has reviewed them. Phase 3 and 4 are both blocked on this gate.

**Phase 3 review happens in parallel with Phase 3 execution**: Each fix lands as a PR with its RED pin and evidence documented in the commit message and plan audit. The executor does not wait for review between FRAGOs; multiple FRAGOs can be in flight at once. Patrick reviews each FRAGO's fix before it merges to the plan branch.

**Phase 4 size gate**: Before Phase 4 starts, record the estimated time budget (sessions, tasks). If the pass appears to exceed the per-phase budget, escalate to Patrick immediately. Do not code blind. Do not discover scope-exit release is its own milestone mid-implementation.

**Loom substrate precondition**: If any FRAGO's fix depends on loom model-checking (e.g., a synchronization-primitive change), confirm M8 Phase 3's loom substrate is still live and the new logic is model-checkable before writing code. Do not add loom-requiring changes to a phase where loom work is not already in scope.

**Crossing-local and frame-slot fixes**: If Phase 3 produces a FRAGO touching crossing locals or frame slots, freeze all other tree-mutating work in that session. Two concurrent code-reading seats (one FRAGO fix + one design review) on the same choke point have led to reversed state observations in the past (`m8-p3` incident); single-threaded discipline is the guard.

---

## 4. Sustainment

**Dependencies**: Tokio runtime (already embedded in `libynz_rt.a`; no new external deps). Loom for model-checking (already in dev-time `Cargo.toml`; production-build no-op per M8 Phase 3 proof).

**Environment**: Dev container (`docker-compose.yml`). `/tmp` is tmpfs with exec, required by `ynz run` which links a binary there.

**Tooling**: Alloc counter (Phase 2 Probe (a), to be built as a temporary instrumentation). Fuzzer (Phase 8 proves it runs; reuse its corpus and generators).

**Credentials**: None.

---

## 5. Command & Signal

**Ownership**: Patrick Rizzardi (this plan is driven by his settled order; every FRAGO decision gates on his sign-off).

**Cold-resume pointer**: Read this file top-to-bottom. Then read `.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md` Capability Ledger to understand what "one consolidated blocker register" means. Then read M8's plan `## Future Requirements` section for the exact deferrals this plan inherits. Then Phase 1 begins.

**Audit trail**: Commit messages follow `Co-Authored-By: Claude Opus 5 (1M context)` convention + `Claude-Session: https://claude.ai/code/session_...`. Phase 2 findings and FRAGOs are written to `.claude/planning/active/2026-09-04-v0-3-concurrency-hardening/audit.md` (created at Phase 2 close-out, appended through Phase 3). Each FRAGO landing is recorded by session ID and executor name (e.g., `m8-p2-signoff-20260903`). Risk decisions (Phase 4 size gate) are recorded with Patrick's signature if a deferral is chosen.

---

## Design-Doc Alignment

**Governing design docs**:

1. **`docs/internal/implementation/IMP-no-function-coloring.md`** — the no-function-coloring model and Task Cancellation section. **Specifies**: (a) whole-program may-block analysis (M2 scope completed); (b) auto-inserted suspension points at call sites (M2 completed); (c) preemption-check insertion at loop back-edges and function calls (M1 completed as per Architectural Decisions); (d) auto-Arc sharing topology across `background` boundaries with read-only proof (M8 Phase 2 specified, Phase 5 implemented the BENEFICIAL-EMISSION subset). **Silent on**: the scope-exit drop mechanism for handles (Phase 7 re-deferred its entire design; Task Cancellation section says "tasks stop at scope end" but names zero codegen path to implement it). This plan's Phase 2 diagnosis and Phase 4 execution answer the silence.

2. **`docs/internal/implementation/IMP-concurrency.md`** — core concurrency semantics (Suspension vs. Ordering, Reads vs. Writes, Loop Iterations). **Specifies**: auto-parallelization for independent operations; data-dependency and ownership-based ordering; `wait` as explicit ordering only (not suspension). **Silent on**: scope-exit release (the authoritative-derivation class — one drop-insertion pass, never two parallel implementations). This plan's Phase 4 is the implementation of the unspecified mechanism.

3. **`docs/internal/implementation/IMP-ownership.md`** — call-site ownership inference (`share`/`lend`/`give`), `.copy()` semantics, auto-Arc sharing-topology section (M8 Phase 2 added it). **Specifies**: transfer rule (sent, given, returned); effective-ownership proof for read-only inference. **Silent on**: scope-exit transfer handling (when a binding is transferred via `send`, does the scope-exit release apply to the original binding or does ownership move? The transfer rule is silent on where transfer happens in the control-flow edges). This plan's Phase 4 must define the transfer-rule intersection with scope-exit enumeration.

**Verification of cited specifications**:

- `IMP-no-function-coloring.md`'s "Task Cancellation" section (cited by heading, not line number — anchors in this repo have drifted 500–650 lines): correctly specifies suspension correctness (auto-inserted, no function coloring), correctly specifies that Tokio will stop a task at its next suspension once it's dropped — but does NOT specify the compiler-side mechanism to call the drop (emit.rs writes zero `ynz_handle_free` today). **Finding**: specification is incomplete; the mechanism is the silence this plan's Phase 4 fills.

- `IMP-concurrency.md` entire "Suspension vs. Ordering" section: correctly specifies what suspension and ordering are. Does NOT specify when locals are freed relative to control-flow edges. **Finding**: specification gap; not inconsistent with this plan, only incomplete.

- `IMP-ownership.md` transfer rule: specifies which bindings are consumed by a send/give/return. Does NOT specify whether a binding that is transferred also skips scope-exit release on the original declaration site (it does, per Phase 4 discipline). **Finding**: discipline is consistent with the rule's intent (ownership moved = no release on the original site); the plan's Phase 4 must implement the interaction explicitly.

---

## Invariants This Milestone Must Preserve

### Safety

- No use-after-free on local bindings (Phase 4's scope-exit release catches this).
- No double-free on local bindings (transfer rule skip + parity test).
- Channel send does not corrupt the payload (Phase 3's FRAGO for crossing-local + blocked-send).
- Handle scope exit does not cause use-after-free in the parent (Phase 4 proves this via the two pin tests).

### Performance

No auto-promotion candidates identified for Phase 2–4's scope. Scope-exit release is mandatory overhead (correctness, not optimization), not subject to auto-promotion. Performance impact: one codegen path per exit edge per local type (overhead is the released-memory guarantee, not a perf choice to make cheaper).

### Teaching

Phase 3's fixes must update three-slot diagnostics (WHAT/WHAT-INSTEAD/WHY) where new compiler errors are introduced. The guard for M8 Phase 7's re-deferral is **already decided by Patrick (2026-09-04): a Tier 3 lint, NOT a muted hint** — he prefers a known deferred defect to nag loudly rather than sit in passive gray text, and a Tier 3 lint is visible in the editor AND in compile output where a muted hint is neither. It fires only on the exposing shape (a bound handle whose scope ends with no `wait` on it), never on every spawn. Its text carries the three slots: WHAT — this task keeps running after `h` goes out of scope; WHAT-INSTEAD — `wait` on it, or send it a stop signal (close the channel it receives on); WHY — nothing in Yinz releases a local when its scope ends yet, so the receipt going away does not stop the work. That lint is small separate work landing BEFORE this plan; **this plan's Phase 4 retires it.** Phase 3's fixes must carry three-slot text for any new compile error they introduce.

### Runtime Dependencies

Phase 3's FRAGOs may add new runtime calls (e.g., rebalancing a synchronization primitive). Each new call is a new runtime dependency; record it here and in the Kernel-Mode Behavior section. Phase 4's scope-exit release calls `ynz_array_drop`, `ynz_free`, etc. — already exist, no new dependencies.

### Kernel-Mode Behavior

Phase 3's FRAGOs: each new runtime call is classified by whether it's allowed in `--kernel` mode. Likely answer for scope-exit release: **allowed** (local cleanup is non-blocking, per-scope). If a FRAGO introduces a may-block runtime call, `--kernel` mode must reject it at compile time with a WHAT/WHAT-INSTEAD/WHY diagnostic. Phase 4's release calls are allowed in `--kernel` mode.

### Demo & Error Gallery

Phase 3's new compile errors are added to `examples/primantis-orders/m8_errors.ynz` with `// WHY: <diagnostic-class>` comments (or higher milestone's gallery if Phase 3 slip causes it to ship in M9). Phase 4's handle-cancellation behavior is demonstrated in `examples/pirates-roster/entrypoint.ynz` with a spawned task that prints before the handle scope ends, showing the task actually stops (vs. running to completion).

### Feature Registry Entries

Phase 3 may retire registry entries (e.g., `background-handle-cancel-injection` if Phase 4 ships, else stays deferred with new trigger). Phase 4 retires `background-handle-cancel-injection` if it ships. Record all entries touched by phase:
- **Retiring**: (deferred_language_feature — verified against `registry/features.toml`) `background-handle-cancel-injection` — Phase 4 closes the underlying defect; the Tier 3 lint is no longer needed.
- **Modifying**: (deferred_language_feature) entries named by Phase 1's blocker audit may be modified with corrected descriptions if Phase 2's diagnosis changes their trigger or scope. Record each modification.
- **No new entries** expected from Phases 1–2 (diagnosis, no language surface). Phase 3 may add entries if a FRAGO introduces new muted-hint domains or lint rules (record if it happens).

---

## Future Requirements / Revisit

**Phase 2 gate (before Phase 3 starts)**: Patrick reviews the FRAGO list and signs off on the producer clustering. If Phase 2 discovers that a blocker is unfixable in this plan's scope (e.g., requires redesigning the ownership system), it is re-deferred with explicit evidence and a new trigger.

**Phase 4 size gate (before Phase 4 starts)**: Estimate scope-exit release effort. If the pass requires more sessions than the per-phase budget allows, escalate to Patrick for deferral approval. Deferral outcome: defer the pass to v0.3-M9 ("drop-story" milestone), update the roadmap status, retire `background-handle-cancel-injection` registry entry with deferred-to-M9 trigger. If deferred, M8's re-deferral of handle cancellation (Phase 7 FR #3) is unaffected (it has its own separate trigger).

**Phase 3 regression gate (after Phase 3 closes)**: If any FRAGO fix introduces a regression (a test that was green goes red), the fix is reverted and the blocker is re-deferred with the regression documented and a new trigger (e.g., "fix the regression before retrying").

**Contingency — Phase 3 discovers a blocker is deferred to a future release**: Phase 2 may discover that one producer cannot be fixed in this plan's scope (e.g., requires the drop-story milestone). That blocker is explicitly deferred in this plan's audit.md with four fields: WHAT (the blocker), WHY (the technical reason it's out of scope), COST (effort to fix later), TRIGGER (what must happen first).

