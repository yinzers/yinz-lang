---
slug: v0-3-m3e-cross-module-frame-serialization
type: execution
owner: Patrick Rizzardi
status: active
roadmap: v0-3-concurrency-perf
depends_on: [v0-3-m3b-auto-parallelization]
plan_base: 7bdd5f9
files:
  - crates/ynz-codegen/src/emit.rs
  - crates/ynz-codegen/src/queries.rs
  - crates/ynz-codegen/src/state_machine.rs
  - crates/ynz-typeck/src/queries.rs
  - crates/ynz-typeck/src/resolve_import.rs
  - crates/ynz-typeck/src/signatures.rs
  - crates/ynz-typeck/src/exports.rs
  - crates/ynz-lsp/tests/**
  - crates/ynz-driver/tests/**
  - registry/features.toml
  - design/future/cross-module-frame-serialization.md
  - design/decisions.md
  - examples/pirates-roster/**
  - examples/primantis-orders/**
  - .claude/plans/roadmaps/v0-3-concurrency-perf.md
created: 2026-06-05
last_updated: 2026-06-05
---

# Plan: v0.3-M3e — Cross-Module FrameLayout (codegen-side cross-module query)
Created: 2026-06-05
Status: pending_approval

## Context & Why

**Goal.** Make EVERY analyzable cross-module *suspending* function call compile and run correctly — re-export chains (A→B→C), shape/number/float/errors-capable crossing-locals and returns across the boundary, transitive×caller-frame combos — with no silent miscompile and no SIGILL. On completion this **lifts the M3b Phase 1 universal-reject floor** that currently turns every cross-module suspending call into a clean compile error.

**Background — what exists today (branch `feat/m3b-auto-parallelization` @ `7bdd5f9`).** M3b Phase 1 shipped the SOLID half of cross-module suspension:
- `suspends` propagates across module boundaries (whole-program may-block analysis — the no-coloring model). `queries.rs`/`may_block.rs`/`resolve_import.rs` carry an exported fn's `suspends` flag into importing modules.
- A **universal reject** floor: `check_query` (typeck) emits a clean WHAT/WHAT-INSTEAD/WHY compile error (exit 1) for ANY call to an imported suspending function (`crates/ynz-typeck/src/queries.rs:366-402`, `emit_loud_reject_for_imported_suspending_calls` at `:475`). This floor is *sound by construction* — it rejects everything, so it cannot miss a case.

The reject replaced a **predictive guard** (`composed_frame_simple`) that tried to predict which cross-module combos codegen could handle. That guard leaked **5 distinct silent crashes** across two adversarial gate rounds, because it predicted codegen frame safety using a *different, shallower* typeck analysis (`compute_composed_frame_size` / `typeck_type_frame_slots`, `crates/ynz-typeck/src/resolve_import.rs:319-482`) — a **lossy parallel reimplementation** of codegen's real `build_frame_layouts` (no-duct-tape #7). The 5 escapes (the danger matrix M3e must cover):
1. **Re-export / multi-level transitive** (A→B→C; B re-exports a fn calling A's suspending export). `compute_composed_frame_size` skips imported suspending callees → exports 32 where 64 needed → C undersizes the embed → SIGILL exit 132.
2. **Shape crossing-local in a cross-module suspending callee.** Typeck counts "8 bytes per field"; codegen uses the real LLVM padded ABI size → undersized slot → abort 10/10.
3. **Errors-capable export that suspends transitively** (errors × transitive cross-product). The 16-byte `{i64,i64}` staging slot interacts with child sub-frame offsets the scalar can't reproduce → abort 3/3.
4. **Number/decimal128 crossing-local in a cross-module caller** (two-slot wide value) — combined frame mis-sized.
5. **Transitive × caller-frame combinations** — any multi-slot caller frame + transitive-suspender callee → wrong total size.

**Why now.** Any user who wants cross-module suspension is blocked until M3e ships. This is the last gap before cross-module concurrency is real (M3b Phases 2-6 — auto-parallel — resume after M3e, or independently).

**Success criteria.** The full danger matrix (every value type × {crossing-local, loop-var, return} × {direct, transitive, re-export-chain} × {scalar, shape, number/decimal128 two-slot, errors-capable}) RUNS CORRECTLY through the real compiler (`ynz run`, exact stdout, exit 0, run ≥2× for determinism, alloc-count leak-checked). Only genuinely-unanalyzable edges (dynamic-dispatch-through-vtable, FFI) still reject — via the existing may-block "unresolvable" path, NOT a frame-layout reject. The lossy typeck reimpl is DELETED. `cargo test --workspace` green. `--no-auto-parallel` and default produce byte-identical output on every fixture (existing cross-impl consistency gate).

---

## Research Findings (verified anchors, 2026-06-05)

All verified by reading the code on `7bdd5f9` (two Explore sweeps + one adversarial decision-risk analysis).

### The compilation model is SEPARATE compilation (decisive)
- **One LLVM module / one `.o` per source file, linked by the system C linker** (`crates/ynz-driver/src/build.rs:186-450`, `build_project` + `link_objects`). There is NO single merged LLVM module.
- `codegen_query` (`crates/ynz-codegen/src/queries.rs:22`) runs per source file. `emit_artifact` (`emit.rs:638`) iterates ONLY `typed.module.items` (the one file's AST). Imported functions are emitted as LLVM `declare` (external) stubs only; for imported suspending callees, the `ynz_sm_<name>_resume` resume fn is also forward-declared (`emit.rs:836-845`). The callee's resume *body* comes from the callee's own `.o`, resolved at link time.
- The importer composes its frame by reserving + placing the callee's sub-frame and emitting the inline poll-yield; the callee's resume body (in callee.o) operates on its own internal slots within the sub-frame base pointer it's handed.

### Shape ABI sizes require LLVM `TargetData` (decisive)
- `shape_abi_sizes` is computed via inkwell `TargetData::get_abi_size` on the LLVM struct types (`emit.rs:858-870`), using the module's data-layout string (set from `TargetMachine::get_default_triple()` + CPU `"generic"`, `emit.rs:656-674`). This is the real target-dependent padded layout — the thing typeck's "8 bytes per field" approximation gets wrong (escape #2). **An accurate layout cannot be computed without LLVM TargetData, which exists only in `ynz-codegen`.**

### `FrameLayout` and what the importer actually reads
- `FrameLayout` struct: `emit.rs:187-208`. Fields: `total_size: u64`, `n_locals: usize`, `children: Vec<(String,u64)>` (callee name → byte offset within this frame), `recursion_slot: Option<u64>`, `number_errors_staging_offset: Option<u64>`. No serde derives. Shape-embed decisions + crossing-local topology are computed at *emit time* (`sm_crossing_shape_embed_set`, `emit.rs:1415`), NOT stored in `FrameLayout`.
- `build_frame_layouts` (`emit.rs:239-367`) consumes `typed: &TypedModule`, `suspend_set`, `shape_abi_sizes`, `imported_fns`. Pure arithmetic (no inkwell calls inside). For imported callees it currently seeds from `imported_fns[name].composed_frame_size` (the lossy scalar, `emit.rs:284-292`).
- **Adversarial-verified — the importer reads ONLY two things from a callee's layout:** (a) `total_size` (used by the *parent's* `build_frame_layouts` to size + place the embedded child sub-frame and the parent's own following slots); (b) `n_locals` (read directly at `emit.rs:5478-5481` to cap the arg-write count into the callee's param slots). The importer does NOT need the callee's internal child offsets / EC staging / shape-embed slots — the callee's resume body (callee.o) owns those. The current call ABI writes only params into slots `[0..n_params)`; crossing locals are populated by the callee's own resume body. ⟹ **No `FrameLayout` enrichment is required** for M3e correctness (YAGNI). This assumption is RE-VERIFIED live in Phase 2; any combo that needs internal offsets is a BLOCK that surfaces an enrichment requirement.

### Crate dependency wall + salsa graph
- `ynz-codegen` depends on `ynz-typeck`; NOT vice versa. `FunctionSig` is in `ynz-typeck` (`signatures.rs:13-52`, holds `composed_frame_size: u64` at `:51`). No shared types crate. `db` (salsa `SourceFileRegistry`) IS available inside codegen; codegen can call `check_query(db, file)` + `module_signatures_query(db, file)` + `db.source_by_path(path)`.
- Salsa queries: `parse_query`, `module_signatures_query` (cycle-recovered), `check_query` (cycle-recovered), `exports_query`, `codegen_query`. `codegen_query` → `check_query` + `module_signatures_query`. Circular imports already produce clean diagnostics (cycle recovery) and codegen is skipped on errors.

### The lossy reimpl to delete + its blast radius
- `compute_composed_frame_size` + `typeck_type_frame_slots` (`resolve_import.rs:319-482`). Called from `load_export_table` (`resolve_import.rs:778`). Output stored in `FunctionSig.composed_frame_size`.
- `FunctionSig.composed_frame_size` blast radius (every site that breaks when the field is removed): production `signatures.rs:160` (init 0), write `resolve_import.rs:778`, reads `emit.rs:288-289`, and 7 LSP test `FunctionSig {…}` literals (`crates/ynz-lsp/tests/hover.rs:80,238,305`; `completion.rs:104,169,513,596`). Stale doc comment referencing the removed `is_composed_frame_simple` at `resolve_import.rs:369`.
- Registry: `[[deferred_language_feature]] name = "cross-module-frame-serialization"` (`registry/features.toml:2117-2123`, `ships_in = "v0.3-M3e"`). Its `why` text encodes the wrong mechanism ("serializes the full FrameLayout into the export table"); corrected in Phase 0, removed when shipped (Phase 3).

### M3a is the proven template (the intra-module version of this exact work)
- `.claude/plans/done/v0-3-m3a-suspension-codegen.md`: Phase 1 = **27 fix rounds**, Phase 2 = 7, Phase 3 = 8. The adversarial gate (code-reviewer Section-4 + deviation-judge) caught MULTIPLE silent miscompiles that the executor suite + coordinator pre-verify both MISSED ("untested orderings hide silent-wrong"). Each phase ran fixtures through `./target/debug/ynz run` with exact stdout + exit + alloc-count + determinism (≥2×) assertions. M3a deferred its hard cases to M3c the same way M3b P1 deferred cross-module to M3e. **M3e Phase 2 will be the same multi-round adversarial slog — budgeted, not a surprise.**

---

## Design-Doc Alignment

**Governing docs (both `[locked]` per `.claude/design-sources.md`):**
- `design/future/concurrency.md` — no-function-coloring model; whole-program may-block analysis; auto-suspension at call sites; "None of these changes may reintroduce function coloring" invariant.
- `design/concurrency.md` — suspension semantics + wait-ordering model.

**Alignment confirmation.** M3e is PURE codegen frame-composition for cross-module suspending calls. It adds NO function-signature-level marking, NO type-level async/await, NO caller-must-be-marked-because-callee-is rule. The `suspends` property is whole-program may-block analysis (already shipped, M3b P1). M3e is the codegen completion of what the no-coloring model already promises (cross-module suspension just works, automatically). ⟹ **Zero function-coloring risk; M3e strengthens the no-coloring contract by making it correct across modules.** Binary-package (`.ynzlib`) may-block + frame metadata (`design/future/packages.md`) is FUTURE; today modules are source-level (compiled together, system-linked), so M3e operates at the source-file/`.o` level — consistent with the doc's "v0.1 reserves space, later versions populate" staging.

**Milestone-boundary check.** M3e is a documented roadmap milestone (`.claude/plans/roadmaps/v0-3-concurrency-perf.md` → "Milestone 3e", SPLIT 2026-06-05 from M3b P1 by Patrick decision). It defers nothing the design says is load-bearing for THIS milestone; genuinely-unanalyzable edges (dynamic-dispatch, FFI) stay rejected via the existing may-block path (correct, not a band-aid).

**Design-doc CORRECTION (Patrick-authorized 2026-06-05).** The deferral doc `design/future/cross-module-frame-serialization.md` and the roadmap M3e rough-scope both prescribe a MECHANISM — "serialize the whole FrameLayout across the export table; carry it in the typeck-side `FunctionSig`, computed once in codegen." That mechanism is **structurally unsound** against two verified facts (separate compilation → no shared LLVM module; shape ABI sizes need LLVM `TargetData` → can't compute in typeck without re-creating the lossy reimpl we're killing). Per CLAUDE.md ("design doc wins unless Patrick decides to change it — then update the doc") and Patrick's 2026-06-05 instruction ("whatever the long-term answer is that follows our golden rules / no-duct-tape / design doc unless our design doc is wrong"), the doc's GOAL is kept and its MECHANISM is corrected to the sound realization (codegen-side cross-module `frame_layouts_query`). Phase 0 rewrites the doc + roadmap + `design/decisions.md` accordingly. This is recorded as an approved entry in `## Design Divergences`.

---

## Research Findings → Architecture (the locked mechanism)

**Codegen-side cross-module `frame_layouts_query` (single source of truth).**

1. Extract the frame-layout computation into a salsa-tracked query in `ynz-codegen`:
   `frame_layouts_query(db, source) -> Arc<HashMap<String, FrameLayout>>`. It creates an inkwell `Context` (local to the query; dropped before return — only `u64` sizes escape) to compute LLVM-accurate `shape_abi_sizes`, then runs `build_frame_layouts`.
2. A module's OWN emission (`emit_artifact`) consumes `frame_layouts_query(source)` instead of computing inline — so a module emits its functions against the SAME layout an importer will read (single source of truth by construction; salsa memoizes).
3. The importer's codegen, for an imported suspending callee, resolves the callee's `SourceFile` (via typeck's import resolution → `db.source_by_path`) and calls `frame_layouts_query(callee_file)` — reads the callee's REAL `FrameLayout` (`total_size`, `n_locals`) to reserve/place the embedded sub-frame and cap the arg-write.
4. **Re-export recursion (escape #1 fix):** `frame_layouts_query(B)` resolves B's imported suspending children's sizes by recursively calling `frame_layouts_query(A)` — NOT by reading `composed_frame_size`. This makes B's `total_size` correctly include A's sub-frame. Terminates (import DAG); salsa cycle recovery (return empty map) for the already-errored circular-import case.
5. DELETE `compute_composed_frame_size` + `typeck_type_frame_slots` + `FunctionSig.composed_frame_size`. LIFT the universal-reject guard (typeck just stops rejecting; dynamic-dispatch/FFI still reject via the existing unresolvable-edge path).

**Why this honors the golden rules.** ONE computation (`build_frame_layouts`), LLVM-accurate, used by both callee-emit and importer-embed → no parallel impl (no-duct-tape #7). Keeps the clean frontend/backend split (typeck stays LLVM-free; layout stays in the backend — Golden Rule 4, "compiler does the hard work" in the right place). No typeck→codegen crate cycle. No `FrameLayout` struct enrichment (YAGNI). The deferral doc's "carry in `FunctionSig`" was a mechanism, not a goal; the goal (importer gets the callee's exact layout) is met by the query.

**Three Phase-0 guards (from the adversarial decision-risk analysis — mandatory):**
- **G1 (Hole 1, CRITICAL):** `frame_layouts_query`'s inkwell `Context` MUST use the identical target triple (`TargetMachine::get_default_triple()`) + CPU (`"generic"`) as `emit_artifact`, extracted into ONE shared constructor (e.g. `state_machine::default_target_data()` or a `target.rs` helper) called by both. A divergent data-layout string = silent wrong `n_locals` for shape crossing-locals. No magic-string duplication.
- **G2 (Hole 3):** the query's imported-child size resolution is recursive via `frame_layouts_query(callee_file)`, never `composed_frame_size`.
- **G3 (Hole 5):** `frame_layouts_query` has salsa cycle recovery (empty map as `cycle_initial`).

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **Silent miscompile / SIGILL on an untested cross-module combo** (the M3a class — adversarial gate caught silent-wrong the suite missed) | **High** | **Critical** | The full danger matrix run through the REAL compiler (`ynz run`, exact stdout, exit 0, ≥2× for determinism, alloc-count leak check). Mandatory `adversarial-tester` agent + 5-reviewer fan-out on Phase 2; expect many fix rounds (M3a = 27/7/8). Universal reject is the rollback — anything not proven stays rejected. |
| **`frame_layouts_query` data-layout string diverges from `emit_artifact`** → wrong shape `n_locals` | Med | Critical | Guard G1: ONE shared target-machine constructor, used by both. Phase-1 gate asserts shape-crossing-local layouts byte-identical to today's intra-module emission. |
| **Re-export recursion mis-composes or fails to terminate** (escape #1) | Med | Critical | Guard G2 (recursive query, not scalar). Phase-1 unit test calls `frame_layouts_query` directly on a 3-module fixture and cross-checks B's `total_size` against B's own emission. Salsa cycle recovery (G3). |
| **The "importer needs only total_size + n_locals" assumption is wrong for some combo** (e.g. a path that pre-writes a crossing-local slot) | Low | High | Phase-2 verifies the assumption live across the full matrix. If any combo needs internal offsets → BLOCK; enrich `FrameLayout` with the specific slot map (scoped sub-task), re-gate. Not assumed — verified. |
| **Deleting `composed_frame_size` breaks LSP test `FunctionSig` literals + salsa result shape** | Low | Med | Blast radius enumerated (7 literal sites + 3 prod/read sites). Phase-2 step updates all in lockstep; `cargo test --workspace` gate. |
| **inkwell `Context` created inside a salsa query escapes / breaks Send** | Low | High | `codegen_query` already creates a `Context` inside a salsa query (same pattern). Query RETURNS only `Arc<HashMap<String,FrameLayout>>` (no inkwell types). Context dropped before return — same drop discipline as `emit_artifact`. |
| **`frame_layouts_query` recompiles struct types per call (perf)** | Low | Low | Salsa memoizes per `SourceFile`; cache miss only on file change. Same cost class as `codegen_query` already pays. LSP-session hot path is memoized. |
| **Regression in existing intra-module suspension (M3a) from the query extraction** | Med | High | Phase 1 is a pure refactor-to-query with ZERO behavior change required; gate = byte-identical IR/output on the full M3a fixture set + `--no-auto-parallel` consistency. |

---

## Questions

None blocking. The one architectural decision (codegen query vs export-table serialization) was resolved with Patrick 2026-06-05 (do the long-term-correct thing; the locked-doc mechanism is wrong → corrected). The "total_size + n_locals suffice" assumption is verified live in Phase 2 rather than asked.

---

## Risk Assessment & Rollout Strategy

**Risk level: HIGH** (codegen that changes execution layout/scheduling across module boundaries — the silent-miscompile class; the exact surface that HALTED v0.3-M2 and took M3a ~42 fix rounds).

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler internals. |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing behavior | **Yes** | Lifts the universal reject → cross-module suspending calls go from "compile error" to "run". Changes codegen layout sourcing (scalar → query). |
| Third-party integration | No | (inkwell/LLVM already a dep.) |
| Changes existing codegen output | **Yes (intra-module: must be byte-identical; cross-module: new behavior)** | |
| New capability, no equivalent | **Yes** | First working cross-module suspension. |

**Mitigations applied:**
- Universal reject = permanent rollback / kill-switch — any combo not proven by a live-run fixture stays rejected (sound floor). → caps blast radius.
- Exhaustive danger matrix through the real compiler + mandatory adversarial-tester + 5-reviewer fan-out on the lift phase. → HIGH→MEDIUM detection.
- Phase 1 is behavior-preserving (refactor-to-query); the behavior change is isolated to Phase 2 behind the matrix gate. → backward-compatible rollout of the machinery.
- `--no-auto-parallel` ≡ default byte-identical consistency gate on every fixture. → cross-impl correctness check.

**Rollout plan (AI-time, per CLAUDE.md Rule 10):**
1. Phase 0 (foundations) + Phase 1 (query, no behavior change) — low silent-wrong risk; land + gate.
2. Phase 2 (the lift) — the multi-round adversarial slog; do NOT advance until the full matrix is GREEN through the real compiler and all reviewers + the adversarial-tester PASS.
3. Phase 3 (demo/gallery/cumulative opus sweep) + `/release` for `v0.3.0-m3e`. The reject diagnostic + deferred registry entry are removed only after the matrix is green.

---

## Design Divergences

| Doc | What it says | What we do instead | Approved rationale (named cost + reversal path) |
|-----|-------------|-------------------|------------------------------------------------|
| `design/future/cross-module-frame-serialization.md` (NOT in `[locked]` registry; deferral note) + roadmap M3e rough-scope | "Serialize the whole `FrameLayout` across the export table — carry it in the typeck-side `FunctionSig`, computed once in codegen." | Codegen-side cross-module `frame_layouts_query` (single source of truth); typeck carries NOTHING (the reimpl is deleted); the importer reads the callee's layout via the query. | **Forced by verified facts** (Patrick-approved 2026-06-05): (1) separate compilation → no shared LLVM module for the importer to read; (2) shape ABI sizes need LLVM `TargetData` → typeck cannot compute the real layout without re-creating the lossy reimpl we're killing (no-duct-tape #7) OR linking LLVM into the type-checker (inverts the frontend/backend split). **Named cost of the chosen path:** the layout is in-memory (salsa `Arc`), so the FUTURE `.ynzlib` binary-package format (`design/future/packages.md`) will need to serialize the query result then — a clean future extension, not a now-requirement; this is the only thing "carry in `FunctionSig`" would have done earlier, and doing it now would be premature serialization of an in-process value (build-twice). **Reversal path:** none needed — the GOAL (importer gets the callee's exact layout) is met identically; only the transport differs. The deferral doc + roadmap + `design/decisions.md` are corrected in Phase 0 so the doc no longer says the wrong thing. |

---

## Documentation Deliverables

| Deliverable | Phase | Notes |
|---|---|---|
| Correct `design/future/cross-module-frame-serialization.md` to the codegen-query mechanism (keep WHAT/WHY/COST/TRIGGER shape; mark status accordingly) | 0 | The authorized design correction. |
| Correct roadmap `v0-3-concurrency-perf.md` Milestone 3e rough-scope (mechanism line) | 0 | Same correction, roadmap copy. |
| Add `design/decisions.md` index row + rationale for the mechanism decision (codegen cross-module query; WHY: separate-compilation + LLVM-ABI) | 0 | Per docs-checklist "Making a Design Decision". |
| Correct `registry/features.toml` `cross-module-frame-serialization` entry `why` text (interim) | 0 | Remove the wrong "serializes into the export table" phrasing. |
| Mark the deferral SHIPPED: remove the `[[deferred_language_feature]]` registry entry; update the deferral doc's status to "shipped in v0.3-M3e" | 3 | Feature ships → deferred entry retired. |
| `examples/pirates-roster/entrypoint.ynz` — cross-module suspension section (a suspending fn imported from a sibling module, called in realistic context) | 3 | Demo & Error Gallery invariant. |
| `examples/primantis-orders/v0_3_m3e_errors.ynz` — error gallery: the genuinely-unanalyzable rejects that REMAIN (dynamic-dispatch-through-vtable, FFI cross-module suspending), each with `// WHY:` | 3 | Demo & Error Gallery invariant. |

---

## Planned RED Repros

_(The danger-matrix fixtures assert CURRENT behavior — clean reject (exit 1) — from Phase 0, keeping the suite green at every phase boundary; Phase 2 FLIPS them to assert correct runtime output as a documented ratchet, test-first WITHIN Phase 2 (write the correct-output assertion = RED, implement the lift = GREEN, same phase). No intentional RED break is held ACROSS a phase boundary, so this section is empty by design.)_

| What's intentionally broken (file + function/symbol + line-range) | Locking RED test (path::test name) | Asserted contract (correct behavior) | Fixing phase (Phase N / Step — same plan) | Prod-exposure note (how condition 3 holds) |
|---|---|---|---|---|
| — | — | — | — | _(empty — no cross-phase planned RED repros; matrix is reject-asserting until the Phase-2 in-phase flip)_ |

---

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block — execute those actions (persist plan state → persist deviation scratch file → fan out all reviewers + N deviation-judges in parallel → coordinator writes Evidence + Phase Review Gates → handle verdicts → prompt commit). Canonical fan-out spec: `~/.claude/commands/execute-plan.md` Step 3.d–3.h (this file references it; does not duplicate).

**Per Patrick's working preferences (memory):** complete all phases without per-phase commit-approval gates (user reviews the full milestone at the end), BUT run the full reviewer fan-out after each phase. Reviewer/judge prompts MUST forbid state-mutating git (checkout/stash/reset/restore/commit) — they share the working tree. Parallel fix-agents must touch strictly disjoint file sets. Every fix-executor prompt includes comments.md + Rule 11 WHY-quality + Yinz-vocabulary reminders. Re-run the ORIGINAL repro + extended sweep after every "fixed". Codegen/runtime ACs are graded MET only from a LIVE run (binary output), never from surveying "documented" cases.

**Phase 2 additionally:** run the `adversarial-tester` agent on the lift diff BEFORE the reviewer fan-out (the silent-miscompile-risk phase), and run every danger-matrix fixture ≥2× (ASLR-garbage determinism check) + alloc-count leak assertions.

**Final phase additionally:** fan out the full reviewer-and-judge set with `model: "opus"` against the cumulative plan diff (`git diff <plan_base>` if any uncommitted, else `git diff <plan_base>..HEAD`; `plan_base = 7bdd5f9`); flip `status: active → done` only after all reviewers + cumulative judges PASS.

---

## Phases

### Phase 0: Foundations — design correction, shared target-machine constant, danger-matrix fixtures + harness (reject-asserting baseline)
**PR scope**: Correct the design docs/roadmap/registry to the codegen-query mechanism; extract the shared LLVM target-machine constructor (Guard G1); build the FULL cross-module danger-matrix fixture set + a harness that asserts the CURRENT clean-reject behavior. No compiler behavior change.
**Branch**: `feat/m3e-foundations`
**Flag**: N/A
**Est. lines**: ~400 (mostly fixtures + harness + docs)
**Ships via**: `/pr`
**Objective**: Lock the contract (the matrix) and the structural pre-req (the shared target constant) before any codegen change. After this phase the matrix exists and asserts reject; the data-layout-string single-source exists for Phase 1 to consume.
**Why this phase exists**: The danger matrix is the test-first contract for the whole milestone (M3a lesson: untested orderings hide silent-wrong). The shared target constant is Guard G1 — without it Phase 1's query can silently diverge from emission. Both are foundational and behavior-neutral, so they ship as one clean reviewable PR.
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:656-674` — `emit_artifact` builds the `TargetMachine` (`get_default_triple()` + CPU `"generic"`) + sets the module data layout (the thing to extract).
- `crates/ynz-codegen/src/emit.rs:858-870` — `shape_abi_sizes` via `TargetData::get_abi_size`.
- `crates/ynz-codegen/src/state_machine.rs:59-87` — frame constants (`FRAME_HEADER_SIZE`, slot helpers) — natural home for a `default_target_data()` helper, or a new `target.rs`.
- `design/future/cross-module-frame-serialization.md` (whole file) + `.claude/plans/roadmaps/v0-3-concurrency-perf.md:264-269` (M3e rough-scope) + `registry/features.toml:2117-2123` + `design/decisions.md` (index) — the docs to correct.
- `crates/ynz-driver/tests/integration.rs:1773-1958` + `crates/ynz-driver/tests/fixtures/v0_3_m3b_*` — the existing 8 M3b reject fixtures/tests to EXTEND into the full matrix.
**Files (expected scope)**: `crates/ynz-codegen/src/state_machine.rs` (or new `crates/ynz-codegen/src/target.rs`), `crates/ynz-codegen/src/emit.rs` (call the extracted constructor — behavior-identical), `design/future/cross-module-frame-serialization.md`, `.claude/plans/roadmaps/v0-3-concurrency-perf.md`, `design/decisions.md`, `registry/features.toml`, `crates/ynz-driver/tests/integration.rs`, `crates/ynz-driver/tests/fixtures/v0_3_m3e_*` (new multi-module fixture projects), `cspell.json` (doc words if needed).
**Deviation rule**: standard.
**Steps**:
1. **Design correction.** Rewrite `design/future/cross-module-frame-serialization.md`: keep the WHAT/WHY/COST/TRIGGER shape but replace the "serialize FrameLayout into the export table / move type to shared crate / typeck reads it" mechanism with the codegen-side cross-module `frame_layouts_query` mechanism + the two verified facts (separate compilation, LLVM-`TargetData`) that force it. Correct the roadmap M3e rough-scope mechanism line identically. Add a `design/decisions.md` index row + a short rationale block. Correct the `registry/features.toml:2121` `why` text (drop "serializes the full FrameLayout into the export table"; describe the codegen-query mechanism). DO NOT yet remove the deferred registry entry (feature not shipped until Phase 2).
2. **Guard G1 — extract the shared target-machine/data-layout constructor.** Add ONE function (e.g. `state_machine::default_target_machine()` / `default_target_data()`) that owns `get_default_triple()` + CPU `"generic"` + the data-layout string. Refactor `emit_artifact` (`emit.rs:656-674`) to call it. Behavior MUST be byte-identical (same triple/CPU/data-layout as today). This is the single source Phase 1's query will also call.
3. **Build the full danger-matrix fixtures.** Create multi-module fixture projects under `crates/ynz-driver/tests/fixtures/v0_3_m3e_*` covering the matrix axes: value type {int, bool, float, string, number/decimal128 (2-slot), shape, errors-capable `T errors`} × position {crossing-local live across the call, loop-var, return value} × call shape {direct (C calls A's exported suspending fn), 1-level transitive (A's export calls A's own helper that sleeps), re-export chain (A→B→C, B re-exports a fn calling A's suspending export), caller-also-has-own-frame (caller has its own multi-slot crossing locals AND calls an imported suspender)}. Reuse/extend the 8 existing `v0_3_m3b_*` fixtures. Each fixture is a runnable multi-module project (single-entry layout per `examples/README.md`).
4. **Harness asserting CURRENT behavior.** Add integration tests that run each fixture via the driver and assert the CURRENT universal-reject behavior: exit 1, stdout empty, stderr contains the "module boundary" diagnostic, `!stderr.contains("SIGILL")`. (These FLIP to correct-output assertions in Phase 2 — documented ratchet.) Keep the suite green.
5. **cspell** net-zero except any new doc words (e.g. `decimal`, `reimpl`) added deliberately.
**Acceptance criteria**:
- [ ] `design/future/cross-module-frame-serialization.md` + roadmap M3e + `design/decisions.md` + registry `why` text describe the codegen-query mechanism, NOT export-table serialization; the two forcing facts (separate compilation, LLVM-`TargetData`) are stated.
  - Evidence: (filled at phase completion)
- [ ] A single shared target-machine/data-layout constructor exists and `emit_artifact` calls it; `cargo test --workspace` green with byte-identical codegen output (no fixture/IR snapshot changes).
  - Evidence: (filled at phase completion — must cite the test run + that no snapshot diffs occurred)
- [ ] The full danger-matrix fixture set exists (every axis combination has a runnable multi-module fixture) and each is currently asserted as a clean reject (exit 1, no SIGILL).
  - Evidence: (filled at phase completion — list the fixtures + the passing reject-assertion tests)
- [ ] `cargo clippy -D warnings`, `cargo fmt --check`, `jargon_audit` all clean; cspell net-zero except declared doc words.
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No compiler behavior change (the constructor extraction is byte-identical; the matrix asserts existing reject behavior).
- [ ] Matrix axes are exhaustive (value type × position × call shape × wide/EC) — cross-checked against the 5 M3b escapes (all 5 represented).
- [ ] Docs follow WHAT/WHY shape; no banned jargon in user-facing text.
**Verification**: `cargo test --workspace`; `cargo run -p ynz-driver -- run` on 3-4 representative fixtures showing the clean reject; diff check that no codegen snapshot changed.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log** (filled during any fix loops by the coordinator):
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = front-matter `plan_base` `7bdd5f9` (Phase 1 of this plan; resolve via the inline ladder).

---

### Phase 1: `frame_layouts_query` — extract to a salsa query, local emission consumes it (zero behavior change)
**PR scope**: Add `frame_layouts_query` (salsa-tracked) in `ynz-codegen` with LLVM-accurate shape sizing (via the Phase-0 shared constructor), recursive cross-module child resolution (Guard G2), and cycle recovery (Guard G3). Wire `emit_artifact` to consume it for the local module. ZERO behavior change. Reject stays UP.
**Branch**: `feat/m3e-frame-layouts-query`
**Flag**: N/A
**Est. lines**: ~350
**Ships via**: `/pr`
**Objective**: Make ONE memoized computation the source of truth for frame layouts, with the cross-module recursion built and unit-proven in isolation — BEFORE the lift turns it on for user programs.
**Why this phase exists**: Isolates the highest-stakes machinery (the recursion that mis-composed in escape #1) from the behavior change (the lift). The recursion can be unit-tested directly against fixtures (bypassing the typeck reject, which is a check-phase concern) so it's proven correct before any user program depends on it — the M2-HALT safeguard ("prove the machinery against reality, not against the plan").
**Current-state anchors**:
- `crates/ynz-codegen/src/emit.rs:239-367` — `build_frame_layouts` (the pure computation to lift into the query).
- `crates/ynz-codegen/src/emit.rs:858-901` — where `shape_abi_sizes` + `frame_layouts` are computed inline in `build_module`; the call site that will consume the query instead.
- `crates/ynz-codegen/src/emit.rs:284-292` — the imported-child seeding from `composed_frame_size` (the recursion replaces this with `frame_layouts_query(callee_file)`).
- `crates/ynz-codegen/src/queries.rs:22` — `codegen_query` (the existing salsa query that creates an inkwell Context; the pattern to mirror for cycle recovery + Context-drop discipline).
- `crates/ynz-typeck/src/resolve_import.rs:56` — `resolve_module_path` (importer path + import module string → resolved path) for name→`SourceFile` resolution.
**Files (expected scope)**: `crates/ynz-codegen/src/queries.rs` (new `frame_layouts_query` + cycle fns), `crates/ynz-codegen/src/emit.rs` (extract `build_frame_layouts` so the query and `build_module` share it; `build_module`/`emit_artifact` consume the query result; thread `db` where needed), `crates/ynz-codegen/src/state_machine.rs`/`target.rs` (the Phase-0 constructor — used by the query too), `crates/ynz-codegen/tests/` (new unit tests for the query, incl. cross-module recursion).
**Deviation rule**: standard. Any case the query cannot compute MUST be representable (a clean `None`/error the importer can act on) — never a wrong number. Document any fallback.
**Steps**:
1. Extract `build_frame_layouts`'s computation into a form callable both from `build_module` and from the new query, taking the module's typed body + `suspend_set` + LLVM-derived `shape_abi_sizes` (via the Phase-0 shared constructor) + a callee-size resolver.
2. Add `#[salsa::tracked] frame_layouts_query(db, source) -> Arc<HashMap<String, FrameLayout>>` in `ynz-codegen/src/queries.rs`. Inside: `check_query(db, source)` + `module_signatures_query(db, source)` for the typed body + imports; create the inkwell `Context` via the Phase-0 constructor, compute `shape_abi_sizes`, run the computation. Drop the Context before returning (only `u64`/`FrameLayout` data escapes).
3. **Guard G2 (recursion).** The callee-size resolver, for an imported suspending callee, resolves the callee `SourceFile` (`resolve_module_path` + `db.source_by_path`) and reads `frame_layouts_query(callee_file)[callee].total_size` — NOT `composed_frame_size`. Recurses naturally for re-export chains.
4. **Guard G3 (cycle recovery).** Add `cycle_fn`/`cycle_initial` for `frame_layouts_query` returning an empty map (circular imports are already errors; codegen is skipped on errors — this is defense-in-depth against infinite recursion).
5. Wire `build_module`/`emit_artifact` to consume `frame_layouts_query(source)` for the local module instead of computing inline. (Reject is still UP, so no module with cross-module suspending calls reaches codegen — the recursion path is built but dormant for user programs.)
6. **Unit-prove the recursion in isolation.** Add `ynz-codegen` tests that register a 3-module re-export fixture's sources and call `frame_layouts_query` DIRECTLY, asserting: (a) B's `total_size` for the re-exporting fn equals what B's own emission would compute; (b) a shape-crossing-local callee's `n_locals` matches the LLVM-padded size (not the typeck "8 bytes" count); (c) cross-checks across modules are consistent. This proves escapes #1/#2 are fixed at the layout layer BEFORE the lift.
**Acceptance criteria**:
- [ ] `frame_layouts_query` exists, is salsa-tracked, computes LLVM-accurate layouts via the Phase-0 shared constructor, and `emit_artifact` consumes it for the local module.
  - Evidence: (filled at phase completion)
- [ ] ZERO behavior change: `cargo test --workspace` green; every existing intra-module + same-module-transitive suspension fixture (the full M3a matrix) produces byte-identical output; `--no-auto-parallel` ≡ default on every fixture.
  - Evidence: (filled at phase completion — cite the M3a fixture runs + no snapshot diffs)
- [ ] The recursion (Guard G2) is unit-proven in isolation: a direct `frame_layouts_query` test on a 3-module re-export fixture yields B's `total_size` correctly INCLUDING A's sub-frame, cross-checked against B's own emission; a shape-crossing-local callee's `n_locals` uses the LLVM-padded size.
  - Evidence: (filled at phase completion — cite the unit test names + asserted values)
- [ ] Cycle recovery (Guard G3) present; a circular-import fixture does not infinite-loop or ICE (still emits the existing clean circular-import diagnostic).
  - Evidence: (filled at phase completion — live run)
- [ ] `compute_composed_frame_size`/`typeck_type_frame_slots` still present (deleted in Phase 2) but NO LONGER READ by codegen for any case the query now covers — OR documented why an interim read remains. No new lossy path added.
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] Inkwell `Context` does not escape the query; return type holds no inkwell types.
- [ ] Guard G1 honored: the query and `emit_artifact` use the SAME target constructor (grep shows one definition, two callers).
- [ ] No `as any`-class escapes (Rust: no stray `unwrap`/`unsafe` added without justification); no test weakening.
- [ ] Recursion terminates on the import DAG; cycle recovery covers the circular case.
**Verification**: `cargo test --workspace`; the new `frame_layouts_query` unit tests; full M3a fixture sweep through `./target/debug/ynz run` (byte-identical); `--no-auto-parallel` consistency check.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log** (filled during any fix loops by the coordinator):
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = Phase 0's committed SHA.

---

### Phase 2: The LIFT — importer consumes the query cross-module, reject lifts, lossy reimpl deleted, full matrix GREEN (adversarial gate)
**PR scope**: Wire the importer's codegen to read `frame_layouts_query(callee_file)` for imported suspending callees; LIFT the universal-reject guard; DELETE `compute_composed_frame_size` + `typeck_type_frame_slots` + `FunctionSig.composed_frame_size`; flip the danger-matrix harness to assert correct runtime output. Every analyzable cross-module suspending combo RUNS correctly.
**Branch**: `feat/m3e-lift-cross-module-suspension`
**Flag**: N/A (the universal reject WAS the kill-switch; rollback = revert this PR)
**Est. lines**: ~500 (+ many fix-round iterations)
**Ships via**: `/pr`
**Objective**: Cross-module suspending calls go from "compile error" to "run correctly" across the full danger matrix, with the lossy parallel reimpl gone.
**Why this phase exists**: This is the headline — the actual lift. It is the silent-miscompile-class change (M3a took 27/7/8 rounds on the intra-module version). Isolated into one heavily-gated phase so the adversarial gate has a single, well-scoped diff to attack.
**Current-state anchors**:
- `crates/ynz-typeck/src/queries.rs:366-402` + `:475-583` — the universal-reject guard + helpers to DELETE.
- `crates/ynz-codegen/src/emit.rs:5391+` / `:5416-5488` / `:5478-5481` — `emit_suspending_call_inline_poll`: where the importer reads the callee layout (`children` offset + callee `n_locals`); the site that must consult `frame_layouts_query(callee_file)` for imported callees.
- `crates/ynz-typeck/src/resolve_import.rs:319-482` — `compute_composed_frame_size` + `typeck_type_frame_slots` to DELETE; `:778` write site; `:369` stale comment.
- `crates/ynz-typeck/src/signatures.rs:51,160` — `composed_frame_size` field + init to DELETE.
- `crates/ynz-codegen/src/emit.rs:284-292` — imported-child seeding to switch fully to the query.
- `crates/ynz-lsp/tests/hover.rs:80,238,305` + `completion.rs:104,169,513,596` — `FunctionSig` literals to update.
- `crates/ynz-driver/tests/integration.rs:1773-1958` — the M3b reject tests + Phase-0 matrix harness to FLIP to correct-output.
**Files (expected scope)**: `crates/ynz-codegen/src/emit.rs` (importer reads the query at the cross-module call site; thread `db` + callee path into the lowering), `crates/ynz-typeck/src/queries.rs` (delete reject guard + helpers), `crates/ynz-typeck/src/resolve_import.rs` (delete reimpl + write + stale comment), `crates/ynz-typeck/src/signatures.rs` + `exports.rs` (drop `composed_frame_size`), `crates/ynz-lsp/tests/{hover,completion}.rs` (drop the field from literals), `crates/ynz-driver/tests/integration.rs` (flip harness to correct-output), fixtures as needed.
**Deviation rule**: standard — any cross-module combo the query/importer cannot handle correctly MUST loud-reject (clean WHAT/WHAT-INSTEAD/WHY error), NEVER silent-wrong. Document each. (Genuinely-unanalyzable edges — dynamic-dispatch-through-vtable, FFI — keep rejecting via the existing may-block unresolvable path; that's correct, not duct tape.) If a combo needs the callee's INTERNAL slot offsets (beyond `total_size` + `n_locals`), STOP — that's a BLOCK surfacing a `FrameLayout` enrichment requirement; do not paper over it.
**Steps**:
1. **Importer cross-module consumption.** In the imported-suspending-call lowering (`emit_suspending_call_inline_poll` + `build_frame_layouts`'s imported-child path), resolve the callee `SourceFile` and read `frame_layouts_query(callee_file)[callee]` for `total_size` (sub-frame sizing/placement) + `n_locals` (arg-write cap). Thread `db` + the importer path through `build_module`/the `Cg` struct as needed.
2. **Lift the reject.** Delete the universal-reject guard activation (`queries.rs:366-402`) + `emit_loud_reject_for_imported_suspending_calls`/`emit_loud_reject_in_stmt`/`emit_loud_reject_in_expr` (`:475-583`). Typeck no longer rejects cross-module suspending calls; dynamic-dispatch/FFI still reject via the existing unresolvable-edge path.
3. **Delete the lossy reimpl.** Remove `compute_composed_frame_size` + `typeck_type_frame_slots` (`resolve_import.rs:319-482`), the `:778` write, and `FunctionSig.composed_frame_size` (`signatures.rs:51,160`) + its `exports.rs` carry. Update the 7 LSP test `FunctionSig` literals + the stale `resolve_import.rs:369` comment. Confirm nothing else reads the field.
4. **Flip the matrix harness to correct-output (test-first within this phase).** Rewrite the Phase-0 reject-assertion tests to assert exact runtime output (the contract): each working combo → correct stdout + exit 0. Write the assertion FIRST (RED, because the lift isn't wired yet), then make it GREEN by completing steps 1-3. Document the flip as a `// test-ratchet:` change (legitimate behavior change, not weakening).
5. **Run the FULL matrix live, ≥2× each, alloc-count leak-checked.** Every value type × position × {direct, transitive, re-export, caller-also-has-frame} × {scalar, shape, number/decimal128, errors-capable} runs correctly (`ynz run`, exact stdout, exit 0, identical across runs — ASLR-garbage determinism check; alloc=free leak assertion on the frame allocations).
5a. **Stacked-mis-sizing adversarial axes (plan-reviewer, the M3a "untested orderings hide silent-wrong" lesson) — each a live-run matrix fixture:**
   - **(i) Same callee called twice, live crossing-local between the two calls.** `FrameLayout` shares ONE embedded slot for repeated calls to the same callee — confirm a cross-module callee invoked twice (sub-frame reused) with a caller crossing-local live across BOTH calls doesn't get its value clobbered by the second call's sub-frame reuse.
   - **(ii) Re-export × EC × number cross-product.** B (middle module) is errors-capable, re-exports a fn calling A's suspending export, AND has its own number/decimal128 crossing local — stacks three independent mis-sizing sources (escape #1 + #3 + #4) in one frame. The single most likely place for a residual offset bug.
   - **(iii) Diamond import.** C imports both A and B; both A and B independently import + call the same suspending leaf D. Confirms `frame_layouts_query`'s salsa memoization + recursion on a DAG (not just a chain): D's layout computed once, embedded consistently in both A's and B's frames, C composes correctly.
   - **(iv) `--kernel` cross-module suspending call.** Confirm it hits the existing kernel-mode suspension reject (clean exit 1), NOT a newly-opened codegen path post-lift. Guards against the lift accidentally routing a suspending call to codegen under `--kernel`.
6. **Verify the architecture assumption live.** Confirm across the matrix that `total_size` + `n_locals` from the query suffice (no combo needs the callee's internal offsets). If any does → BLOCK; scope a `FrameLayout` enrichment (add the specific slot map), re-gate.
7. **Cross-impl consistency.** `--no-auto-parallel` ≡ default byte-identical on every fixture.
8. **Sweep stray field references.** After deleting `composed_frame_size`, sweep its non-`.rs` references too: fixture `.ynz` comments under `crates/ynz-driver/tests/fixtures/v0_3_m3b_loud_reject_*` and `design/future/index.md` (the latter is a Phase-0/3 doc-correction target). The completion grep gate is scoped to `*.rs` (see Verification) so fixture comments don't false-positive it.
**Acceptance criteria**:
- [ ] **NO SILENT FAILURE**: every analyzable cross-module suspending combo RUNS correctly (live `ynz run`, exact stdout, exit 0, ≥2× identical) — direct/transitive/re-export × {int, bool, float, string, number/decimal128, shape, errors-capable} × {crossing-local, loop-var, return}. NOWHERE a silent-wrong value or SIGILL/abort. The 5 former escapes all RUN correctly.
  - Evidence: (filled at phase completion — MUST cite live runs per combo: input → correct stdout, exit 0, run-twice-identical, alloc=free)
- [ ] The universal-reject guard is removed; a former-rejected cross-module suspending call now compiles + runs (the M3b reject fixtures flip to correct-output).
  - Evidence: (filled at phase completion — live run of ≥3 former-reject fixtures)
- [ ] `compute_composed_frame_size`, `typeck_type_frame_slots`, and `FunctionSig.composed_frame_size` are DELETED; `grep` shows zero remaining references; the 7 LSP literals + stale comment updated; `cargo test --workspace` green.
  - Evidence: (filled at phase completion — grep output + test run)
- [ ] Genuinely-unanalyzable cross-module edges (dynamic-dispatch-through-vtable, FFI) still loud-reject cleanly (exit 1, WHAT/WHAT-INSTEAD/WHY) — NOT via the deleted frame guard, via the existing unresolvable-edge path.
  - Evidence: (filled at phase completion — live run)
- [ ] The 4 stacked-mis-sizing adversarial axes (5a.i-iv) RUN correctly live: (i) double-call same callee with live crossing-local between → no clobber; (ii) re-export × EC × number cross-product → correct value; (iii) diamond import (shared leaf D) → consistent embed + correct compose; (iv) `--kernel` cross-module suspending call → clean kernel reject (exit 1), not a codegen path.
  - Evidence: (filled at phase completion — live runs per axis)
- [ ] The "total_size + n_locals suffice" assumption is confirmed live across the matrix (no combo required internal offsets), OR the FrameLayout enrichment it required is implemented + gated.
  - Evidence: (filled at phase completion)
- [ ] `--no-auto-parallel` ≡ default produce byte-identical stdout/stderr/exit on every fixture.
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] **NO SILENT FAILURE**: no cross-module suspending call silently miscompiles (wrong value) or crashes (SIGILL/abort). Every analyzable case works; only dynamic-dispatch/FFI loud-reject. (The floor Patrick set; M3b's first gate violated it twice, the second gate three more times.)
- [ ] Every reject path is a clean WHAT/WHAT-INSTEAD/WHY diagnostic (no ICE, no garbage); every run-case is deterministic across ≥2 runs.
- [ ] No frame allocation leaks (alloc=free on every suspending combo).
- [ ] No `as any`-class escapes; no test weakening (the harness flip is a ratcheted behavior change with `// test-ratchet:` + rationale, asserting MORE/correct behavior, not less).
**Verification**: the full danger matrix (incl. the 4 stacked-mis-sizing axes 5a.i-iv) through `./target/debug/ynz run` (≥2× each, alloc-count, ordering); `cargo test --workspace`; `grep -rn --include=*.rs 'composed_frame_size\|compute_composed_frame_size\|typeck_type_frame_slots' crates/` returns nothing (scoped to `.rs` so fixture `.ynz` comments don't false-positive); `--no-auto-parallel` consistency sweep; `adversarial-tester` agent on the lift diff BEFORE the reviewer fan-out.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log** (filled during any fix loops by the coordinator):
_(empty until a reviewer returns BLOCK — EXPECT multiple rounds per the M3a precedent.)_

**Exit Sequence — RUN THESE STEPS:** per Phase Execution Protocol. `$BASE` = Phase 1's committed SHA. **Additionally run the `adversarial-tester` agent on the lift diff before the reviewer fan-out** (the silent-miscompile-risk phase), and run every matrix fixture ≥2× with alloc-count assertions.

---

### Phase 3: Demo + error gallery + teaching validation + cumulative verification + release prep
**PR scope**: Mark the feature shipped (remove the deferred registry entry; update the deferral doc status); add the `pirates-roster` cross-module suspension demo + `primantis-orders` M3e error gallery; final cumulative opus reviewer sweep; `status → done`; `/release` prep.
**Branch**: `feat/m3e-demo-and-release`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr` (then `/release` for the milestone)
**Objective**: Validate the language UX hands-on (Patrick reviews via the demo + gallery), confirm the whole milestone holds together, and ship.
**Why this phase exists**: Demo & Error Gallery invariant + the cumulative cross-phase review (catches integration bugs per-phase reviews missed) + the release bookkeeping (the deferred feature retires).
**Current-state anchors**:
- `registry/features.toml:2117-2123` — the deferred entry to REMOVE (feature shipped).
- `design/future/cross-module-frame-serialization.md` — status → shipped.
- `examples/pirates-roster/entrypoint.ynz` — the growing demo to extend.
- `examples/primantis-orders/` — the per-milestone error gallery (add `v0_3_m3e_errors.ynz`).
**Files (expected scope)**: `registry/features.toml`, `design/future/cross-module-frame-serialization.md`, `examples/pirates-roster/**` (new cross-module suspension section + the imported module), `examples/primantis-orders/v0_3_m3e_errors.ynz`, `examples/primantis-orders/README.md`, `.claude/plans/roadmaps/v0-3-concurrency-perf.md` (M3e status → shipped), VSCode extension version bump if any tooling surface changed (likely N/A — no new diagnostics, only removed ones).
**Deviation rule**: standard.
**Steps**:
1. Remove the `[[deferred_language_feature]] cross-module-frame-serialization` entry from `registry/features.toml`; update `design/future/cross-module-frame-serialization.md` status to "shipped in v0.3-M3e"; mark M3e shipped in the roadmap.
2. Extend `examples/pirates-roster/entrypoint.ynz`: a suspending function imported from a sibling module, called in a realistic context (not `print(f())` — real work across the boundary), with the muted `wait` hint visible. Add the imported module file. `insta` stdout snapshot.
3. Create `examples/primantis-orders/v0_3_m3e_errors.ynz`: the rejects that REMAIN after M3e — cross-module suspending call through a `dynamic` vtable, and an FFI may-block boundary that can't be analyzed — each with a `// WHY:` comment naming the diagnostic class. `insta` stderr snapshot. (Note: the former universal-reject diagnostic is GONE — confirm the gallery no longer triggers it.)
4. Teaching validation: confirm the `wait_points` muted-hint fires correctly on the cross-module suspending call (it now has real cross-module data); hover text WHAT/WHAT-INSTEAD/WHY correct.
5. Cumulative verification: full danger matrix + M3a fixtures + `--no-auto-parallel` consistency one more time; `cargo test --workspace`, clippy `-D warnings`, fmt, jargon_audit all clean.
6. `status: active → done` after the cumulative opus reviewer sweep PASSes; `/release` prep for `v0.3.0-m3e`.
**Acceptance criteria**:
- [ ] The deferred registry entry is removed and the deferral doc + roadmap mark M3e shipped; `cargo test --workspace` (incl. registry generation) green.
  - Evidence: (filled at phase completion)
- [ ] `examples/pirates-roster/entrypoint.ynz` demonstrates cross-module suspension in realistic context; `ynz run` produces the snapshotted output; the imported module compiles + links.
  - Evidence: (filled at phase completion — live run + snapshot)
- [ ] `examples/primantis-orders/v0_3_m3e_errors.ynz` triggers ONLY the genuinely-unanalyzable rejects (dynamic-dispatch, FFI) — the former universal cross-module reject is gone; each trigger has a `// WHY:`; stderr snapshot stable.
  - Evidence: (filled at phase completion — live run + snapshot)
- [ ] Cumulative: full matrix + M3a fixtures green via `ynz run`; `--no-auto-parallel` ≡ default; `cargo test --workspace`, clippy, fmt, jargon_audit clean.
  - Evidence: (filled at phase completion)
**Quality gate**:
- [ ] No orphaned references to `composed_frame_size`/the reject diagnostic anywhere (registry, docs, tests, comments).
- [ ] Demo shows the feature doing real work (not a trivial `print`).
- [ ] Error gallery diagnostics follow WHAT/WHAT-INSTEAD/WHY; no banned jargon.
**Verification**: `ynz run` on the demo + gallery; `insta` snapshots; full `cargo test --workspace`; cumulative opus reviewer sweep against `git diff 7bdd5f9`.

**Phase Review Gates** (filled at phase completion by coordinator):
- [ ] code-reviewer: <verdict + ISO timestamp>
- [ ] rules-compliance-reviewer: <verdict + ISO timestamp>
- [ ] plan-adherence-verifier: <verdict + ISO timestamp>
- [ ] acceptance-verifier: <verdict + ISO timestamp>
- [ ] design-compliance-reviewer: <verdict + ISO timestamp>
- [ ] Committed: <commit SHA>

**Findings Log** (filled during any fix loops by the coordinator):
_(empty until a reviewer returns BLOCK)_

**Exit Sequence — RUN THESE STEPS (FINAL PHASE):** per Phase Execution Protocol + the final-phase cumulative opus sweep against `git diff 7bdd5f9` (or `git diff 7bdd5f9..HEAD` if clean). `$BASE` = Phase 2's committed SHA for the per-phase gate; `plan_base 7bdd5f9` for the cumulative sweep. Flip `status: active → done` only after all reviewers + cumulative judges PASS.

---

## Invariants This Milestone Must Preserve

### Safety
- No cross-module suspending call silently miscompiles (wrong value) or crashes (SIGILL/abort). Every analyzable combo RUNS correctly; only genuinely-unanalyzable edges (dynamic-dispatch-through-vtable, FFI) loud-reject (via the existing may-block unresolvable-edge path).
- A crossing local of ANY value type (int/bool/float/string/number-decimal128/shape/errors-capable) survives a cross-module suspension and holds its value after resume.
- An errors-capable cross-module suspending call returns the CORRECT value (ok and error paths), including the transitive case.
- A re-export chain (A→B→C) composes the full sub-frame tree correctly — C's frame reserves B's REAL total size including A's embedded sub-frame.
- No frame-allocation leaks: alloc=free on every suspending combo (cross-module included).
- Circular imports still emit a clean diagnostic (exit 1), never an ICE or infinite recursion.

### Performance
- `frame_layouts_query` is salsa-memoized per `SourceFile`; cache miss only on file change. The inkwell `Context` is created only on a cache miss and dropped before the query returns (no Context retained).
- Guard G1: the query and `emit_artifact` use ONE shared target-machine/data-layout constructor — shape ABI sizes are byte-identical between the layout used for emission and the layout the importer reads (no divergence-induced mis-sizing).
- Intra-module suspension codegen output is byte-identical to pre-M3e (the query extraction is behavior-preserving).
- `--no-auto-parallel` ≡ default produce byte-identical stdout/stderr/exit on every fixture (cross-impl consistency).
- **Auto-promotion analysis:** M3e introduces NO new user-facing feature with a stricter/faster form — it makes correct what was rejected (codegen frame composition). There is no `array→fixed`-style promotion candidate, no muted-hint domain, no Tier-3 lint. Stated explicitly so reviewers know it was considered, not forgotten. (The existing `wait_points` muted hint now fires with real cross-module data — validated in Phase 3 — but that is an existing domain, not a new auto-promotion.)

### Teaching
- The `wait_points` muted hint fires on cross-module suspending calls with correct WHAT/WHAT-INSTEAD/WHY hover text (now backed by real cross-module data).
- The remaining reject diagnostics (dynamic-dispatch/FFI cross-module suspension) follow WHAT/WHAT-INSTEAD/WHY and name the diagnostic class.
- The former universal-reject diagnostic ("Calling `{name}` across a module boundary isn't supported yet … until v0.3-M3e") is REMOVED — the feature it pointed at has shipped; no stale "not supported yet" text remains.
- No new banned-jargon words enter user-facing errors (audited by `jargon_audit`).

### Runtime Dependencies
- A cross-module suspending call requires the runtime scheduler (`libynz_rt`) + heap allocator (`ynz_alloc` for the composed frame) — the SAME deps as intra-module suspension. M3e adds NO new runtime dependency; it composes existing frame machinery across the module boundary.
- `frame_layouts_query` is a compile-time-only query (no runtime dependency); it links LLVM/inkwell, already a `ynz-codegen` dependency.

### Kernel-Mode Behavior
- Cross-module suspension under `--kernel` (no scheduler) behaves identically to intra-module suspension under `--kernel`: a suspending call has no scheduler to yield to, so it rejects via the existing kernel-mode suspension story (`design/future/no-runtime-mode.md`) — M3e does NOT change this. M3e neither enables nor newly-breaks kernel-mode suspension; the `--kernel` reject for suspension is orthogonal to the cross-module frame composition and remains as-is.
- **Gated, not just asserted:** Phase 2 axis 5a.iv adds a `--kernel` cross-module suspending fixture asserting the clean kernel reject — insurance that the lift did not accidentally open a codegen path under `--kernel` that the rest of the matrix would miss.

### Demo & Error Gallery
- `examples/pirates-roster/entrypoint.ynz` gains a cross-module suspension section (a suspending fn imported from a sibling module, called doing real work) — Phase 3.
- `examples/primantis-orders/v0_3_m3e_errors.ynz` gains intentional triggers for the rejects that REMAIN (dynamic-dispatch-through-vtable, FFI cross-module suspending), each with `// WHY:` — Phase 3. Both get `insta` snapshots.

### Feature Registry Entries
- **Phase 0 (modify):** `[[deferred_language_feature]] name = "cross-module-frame-serialization"` (`registry/features.toml:2117-2123`) — correct the `why` text to the codegen-query mechanism (drop "serializes the full FrameLayout into the export table").
- **Phase 3 (remove):** the same `[[deferred_language_feature]]` entry is REMOVED — the feature ships in M3e.
- **No new entries:** M3e adds NO new keyword, banned-jargon word, primitive intrinsic, type-attached constant, diagnostic template, or muted-hint domain. It REMOVES a deferred feature + a reject diagnostic. (The universal-reject diagnostic text was dynamic, not a `[[diagnostic_template]]` — no template entry to remove.) Stated explicitly so reviewers know it was considered, not forgotten.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each of the 4 phases is its own PR via `/pr` (foundations / query / lift / demo+release).
- **Shadow main branches**: M3e branches from `feat/m3b-auto-parallelization` @ `7bdd5f9` (M3b P1's universal-reject floor isn't on `main` yet); each phase branches from the prior phase's commit, no long-lived shadow.
- **Building the engine before shipping value**: Phase 1 (the query) is the only pure-infrastructure phase and it's behavior-preserving; the very next phase (2) delivers the user-visible value (cross-module suspension runs). No multi-phase engine-before-value gap.
- **Hotfix that isn't**: N/A — this is planned milestone work, not a hotfix.
- **Abandoned branches**: each phase merges via `/pr` before the next starts; M3b P1's branch is the documented base, not abandoned (M3b Phases 2-6 resume after M3e).
- **Flag graveyards**: no feature flag (the universal reject is the kill-switch; rollback = revert the lift PR). Nothing to clean up.

## Quality Checklist (verify at completion)
- [ ] All inputs validated — N/A (compiler internals; the "input" is Yinz source, validated by parse/typeck).
- [ ] Auth/authz — N/A.
- [ ] Error handling: every reject is a clean WHAT/WHAT-INSTEAD/WHY diagnostic (no ICE/garbage); loud-reject (never silent-wrong) for any unhandled cross-module combo.
- [ ] No SQL/XSS/path-traversal/secret exposure — N/A.
- [ ] Performance: `frame_layouts_query` memoized; Context dropped per query; intra-module output byte-identical; `--no-auto-parallel` ≡ default.
- [ ] Tests: full danger matrix (every value type × position × call shape × wide/EC) RUN live ≥2× with alloc-count + determinism; happy + reject (dynamic-dispatch/FFI) cases covered.
- [ ] Existing tests still pass (`cargo test --workspace` green; M3a fixtures byte-identical).
- [ ] Types complete (no Rust `as any`-class escapes / unjustified `unwrap`/`unsafe`); no test weakening (the harness flip is a ratcheted MORE-correct behavior change).
- [ ] Follows codegen/typeck conventions (one source of truth, no parallel impl — the lossy reimpl is DELETED).
- [ ] Every phase received all-reviewer + all-judge PASS before committing (Step 9a).
- [ ] Final cumulative opus reviewer sweep passed (Step 10f) against `git diff 7bdd5f9`.
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases (Step 9b).
