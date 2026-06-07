---
slug: v0-3-m3e-cross-module-frame-serialization
type: execution
owner: Patrick Rizzardi
status: done
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
last_updated: 2026-06-07
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
- [x] `design/future/cross-module-frame-serialization.md` + roadmap M3e + `design/decisions.md` + registry `why` text describe the codegen-query mechanism, NOT export-table serialization; the two forcing facts (separate compilation, LLVM-`TargetData`) are stated.
  - Evidence: (acceptance-verifier r2) `design/future/cross-module-frame-serialization.md` WHAT rewritten "Full FrameLayout serialization across the export table" → "Codegen-side cross-module `frame_layouts_query`", both forcing facts stated; `registry/features.toml:2121` `why` updated to the query mechanism + both facts; roadmap `v0-3-concurrency-perf.md` rough-scope replaced with the codegen-query description + both facts; `design/decisions.md` new "Cross-module frame layout (M3e)" row citing both facts + no-duct-tape #7. Old "serializes the full FrameLayout into the export table" phrasing gone from all four.
- [x] A single shared target-machine/data-layout constructor exists and `emit_artifact` calls it; `cargo test --workspace` green with byte-identical codegen output (no fixture/IR snapshot changes).
  - Evidence: (acceptance-verifier r2, code-reviewer r1) `state_machine.rs:735-763` `pub fn default_target_machine()` (doc-comment names Guard G1); `emit_artifact` default-triple branch calls `crate::state_machine::default_target_machine()`. Byte-identical PROVEN by the pre-existing 31 golden IR snapshots + `object_file_sha256_matches_golden` (whole-object SHA-256) passing through the changed path. `cargo test --workspace` exit 0 (live); `git diff c755bd8 -- '*.snap'` empty (zero snapshot drift).
- [x] The full danger-matrix fixture set exists (every axis combination has a runnable multi-module fixture) and each is currently asserted as a clean reject (exit 1, no SIGILL).
  - Evidence: (acceptance-verifier r2) 12 fixture dirs `crates/ynz-driver/tests/fixtures/v0_3_m3e_*` (bool/float/string/number-decimal128/shape/ec/int across crossing-local, loop-var, transitive, re-export, caller-own-frame); all 5 M3b escapes mapped in the `integration.rs` comment block. 12 `v03_m3e_*_exits_one_clean_reject` tests pass live (`test result: ok. 12 passed; 0 failed`); `assert_m3e_reject` asserts exit==1 + stdout empty + "module boundary" in stderr + no SIGILL.
- [x] `cargo clippy -D warnings`, `cargo fmt --check`, `jargon_audit` all clean; cspell net-zero except declared doc words.
  - Evidence: (acceptance-verifier r2) `cargo clippy --workspace -- -D warnings` exit 0 (live; the 4 pre-existing may_block.rs errors fixed — collapsible-if collapsed + justified `#[allow]`; emit.rs `map_or`→`is_some_and`); `cargo fmt --check` exit 0; `jargon_audit` 9/9; `git diff c755bd8 -- cspell.json` empty (net-zero; the doc words rode the plan-activation commit).
**Quality gate**:
- [x] No compiler behavior change (the constructor extraction is byte-identical; the matrix asserts existing reject behavior). — plan-adherence r2: check.rs empty diff, may_block collapse short-circuit-equivalent, reject guard + composed_frame_size + the typeck reimpl all present + logic-untouched.
- [x] Matrix axes are exhaustive (value type × position × call shape × wide/EC) — cross-checked against the 5 M3b escapes (all 5 represented). — plan-adherence + acceptance r2 confirmed all 5 escapes mapped.
- [x] Docs follow WHAT/WHY shape; no banned jargon in user-facing text. — rules-compliance r2/r3 PASS; design-compliance r2 PASS.
**Verification**: `cargo test --workspace`; `cargo run -p ynz-driver -- run` on 3-4 representative fixtures showing the clean reject; diff check that no codegen snapshot changed.

**Phase Review Gates** (filled at phase completion by coordinator):
- [x] code-reviewer: PASS 2026-06-06T (round 3; round-2 cross-flag on fixture phase-markers resolved; Guard G1 byte-identical via golden SHA-256; round-1 fixes clean)
- [x] rules-compliance-reviewer: PASS 2026-06-06T (round 3; fixture `.ynz` comments reworded — zero banned markers; `#[allow]` justified)
- [x] plan-adherence-verifier: PASS 2026-06-06T (round 2; all 5 steps MET, no-behavior-change mandate verified, check.rs revert = legitimate plan-correction)
- [x] acceptance-verifier: PASS 2026-06-06T (round 2; 4/4 ACs MET live — clippy exit 0, fmt clean, jargon 9/9, 12 reject tests pass, no snapshot drift)
- [x] design-compliance-reviewer: PASS 2026-06-06T (round 2; no [locked]-doc contradiction; doc correction strengthens no-coloring; divergence rationale = real named-cost tradeoff)
- [x] deviation-judge #1 (scope: queries.rs cargo-fmt whitespace): PASS 2026-06-06T (round 1; rustfmt-on-base reproduces exactly; reject predicate byte-for-byte logic-identical)
- [x] deviation-judge #2 (scope: resolve_import.rs cargo-fmt whitespace): PASS 2026-06-06T (round 1; only non-ws delta a trailing comma — inert)
- [x] deviation-judge #3 (scope+approach: may_block.rs clippy collapse + justified #[allow]): PASS 2026-06-06T (round 2; collapse truth-table-identical; #[allow] sound, params are reserved thread-through state)
- [x] deviation-judge #4 (scope: emit.rs map_or→is_some_and): PASS 2026-06-06T (round 2; is_some_and ≡ map_or(false,…) on Option; pure closure; nothing smuggled)
- [x] Committed: 6344ce2

**Findings Log** (filled during any fix loops by the coordinator):
- 2026-06-06 — Gate round 1 (5 reviewers + 2 judges). PASS: design-compliance (doc correction strengthens no-coloring; divergence rationale = real tradeoff), plan-adherence (all 5 steps landed; reject guard + composed_frame_size untouched — no-behavior-change mandate held), judge#1 (queries.rs fmt: token-scan + rustfmt-on-base reproduce exactly; reject predicate byte-for-byte logic-identical), judge#2 (resolve_import.rs fmt: only non-ws delta a trailing comma — inert). BLOCK ×3:
  - **rules-compliance**: 14 `// Phase 2 flips to correct output` phase-marker comments in `integration.rs:1977-2103` — banned per comments.md Hard Rule 1 / CLAUDE.md Rule 6 (no `// Phase X` work-item markers in code; the flip intent lives in the plan's Phase 2 Step 4). → reword to present-tense current-state.
  - **code-reviewer**: `crates/ynz-driver/tests/fixtures/v0_3_m3e_reexport_ec_number/b_ops.ynz:14` + `entrypoint.ynz:10` call `.or(0.0)` on a value typeck resolves to bare `number`, not `number errors` → latent compile error masked today (cross-module reject fires first; harness only greps "module boundary"). The marquee stacked fixture's Phase-2 target `total: 3.5` is UNREACHABLE. Cross-flag: may be a real imported-`number errors`-return-resolution gap (importer drops the `errors` wrapper) — investigate, surface to Phase 2, don't paper over. Guard G1 confirmed byte-identical (golden object SHA-256). G1 nit (non-blocking): redundant `initialize_x86` in `emit_artifact` default branch now dead ceremony.
  - **acceptance-verifier**: AC4 WEAK — `cargo clippy -D warnings` exits non-zero on 4 PRE-EXISTING errors in `crates/ynz-typeck/src/may_block.rs` (line 451 too-many-arguments 8>7; line 501 collapsible-if; lines 456/458 only-used-in-recursion). Untouched by Phase 0 but AC says "all clean". → fix so clippy exits 0.
- 2026-06-06 — Round-1 fix outcome: (1) rules — 14 phase-marker comments reworded to present-tense (grep confirms the `// Phase 2 flips…` markers gone). (2) acceptance/clippy — collapsible-if collapsed; `too_many_arguments`/`only_used_in_recursion` resolved with justified `#[allow]` + WHY comment (8 args are independent recursive-tree-walk state, struct-bundle would be ceremony per no-duct-tape #2); pre-existing `map_or(false,…)`→`is_some_and` in emit.rs:11532 also fixed; `cargo clippy -D warnings` now exits 0. (3) code/EC fixture — disambiguation found the `.or` failure is a REAL pre-existing typeck bug (auto-propagation strips `ErrorsCapable` before EC-method dispatch on a `let`-bound value), NOT a reject cascade. The fix-executor wrote the fix in `check.rs`, but that is a **typeck behavior change** that violates Phase 0's no-behavior-change mandate AND deserves the full adversarial EC gate → **coordinator REVERTED `check.rs` to base (empty diff vs c755bd8, fmt-clean, compiles) and RELOCATED the documented fix to Phase 2 Step 1a + a Phase 2 AC.** The fixture still rejects cleanly under the universal reject (exit 1, two "module boundary" diagnostics + the masked `.or` error — harness asserts module-boundary + no-SIGILL, holds). Phase 0 stays behavior-neutral. Re-gating round 2.
- 2026-06-06 — Gate round 2 (5 reviewers + 2 NEW judges; fmt judges carried from round 1). 6 PASS: rules, design-compliance, plan-adherence (no-behavior-change mandate confirmed: check.rs empty diff, may_block collapse short-circuit-equivalent, reject guard + reimpl present), acceptance (all 4 ACs MET live — clippy exit 0, fmt clean, jargon 9/9, 12 reject tests pass, no snapshot drift), judge-emit (`is_some_and ≡ map_or(false,…)`), judge-may_block (collapse truth-table-identical + `#[allow]` sound). **code-reviewer PASS on code quality BUT cross-flagged a CONFIRMED rules violation rules-compliance MISSED**: 13 fixture `.ynz` files still carry `// Phase 2 flips to: expected output "X"` — the same banned `// Phase X` work-item marker (comments.md Hard Rule 1); the round-1 reword + grep were `.rs`-scoped. Coordinator verified (grep: 13 files). Per Rule 11 + non-concession, a confirmed violation is fixed regardless of which agent caught it. Also: code-reviewer Concern 2 — Phase 2 Step 1a said "`let`-bound" but the marquee fixture binds `const` (bug reproduces on both) → coordinator tightened Step 1a to "`let`/`const`-bound". Concern 3 (redundant `initialize_x86` in emit.rs:648 default branch — the override branch genuinely needs it, so it's a documented no-op, not pure dead code) → deferred to Phase 1 (the natural cleanup point when Phase 1 consumes `default_target_machine`). → Round 3: reword the 13 fixture comments to present-tense.
- 2026-06-06 — Gate round 3 (narrow re-gate: rules-compliance + code-reviewer; other 3 reviewers + 4 judges carried from round 2 — comment-only reword, no behavior/scope/design/deviation change). BOTH PASS: rules-compliance (now scanned `.ynz` fixtures directly — zero banned markers; judged "Correct runtime output once the M3e plan's …fix ships" an acceptable contract statement, not a deferral marker), code-reviewer (cross-flag resolved; every expected-output string arithmetically honest; 12 reject tests green). **Phase 0 GREEN — all reviewers + all judges PASS.** Phase-2 NOTE (code-reviewer): the decimal128 fixture comment strings (`1.5`, `3.5`) may render with different precision (`1.50`?) — the Phase-2 executor MUST derive accept-test outputs from a LIVE `ynz run`, never copy the fixture comment strings (per `acceptance-runs-not-surveys`).

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
- [x] `frame_layouts_query` exists, is salsa-tracked, computes LLVM-accurate layouts via the Phase-0 shared constructor, and `emit_artifact` consumes it for the local module.
  - Evidence: (acceptance r3) `queries.rs:76` `#[salsa::tracked(lru=64, cycle_fn=…, cycle_initial=…)] pub fn frame_layouts_query(db, source) -> Arc<HashMap<String,FrameLayout>>`; Guard G1: `state_machine::default_target_machine()` at `queries.rs:154`; `codegen_query` calls it (`queries.rs:236`), passes `&layouts_arc` → `emit_artifact` (`emit.rs:717`) → `build_module` binds `frame_layouts_arg` (`emit.rs:932`), replacing the old inline `build_frame_layouts`. `lib.rs:38` re-exports the query.
- [x] ZERO behavior change: `cargo test --workspace` green; every existing intra-module + same-module-transitive suspension fixture (the full M3a matrix) produces byte-identical output; `--no-auto-parallel` ≡ default on every fixture.
  - Evidence: (acceptance r3 + code-reviewer r3) `cargo test -p ynz-codegen` → 31 golden IR + `object_file_sha256_matches_golden` pass; `git diff 6344ce2 -- '*.snap'` empty (zero drift); `cargo test --workspace` exit 0; 241 driver tests pass. The ordering fix is byte-neutral for local modules — the pre-seed loop is empty when every `suspend_set` name is a local fn (structurally confirmed); golden SHA-256 holds.
- [x] The recursion (Guard G2) is unit-proven in isolation: a direct `frame_layouts_query` test on a 3-module re-export fixture yields B's `total_size` correctly INCLUDING A's sub-frame, cross-checked against B's own emission; a shape-crossing-local callee's `n_locals` uses the LLVM-padded size.
  - Evidence: (acceptance r3 + code-reviewer r3, after the round-2 dead-resolver bug fix) `reexport_chain_b_total_size_includes_a_sub_frame`: A's `getValue` has a crossing local → real frame=40 (≠32 fallback); a `Cell<u32>` counter confirms the resolver IS called; `doWork.total_size`==72 (32+0+40, bypass would be 64); anti-bypass sentinel (resolver→56 → 88) proves the output TRACKS the resolver. code-reviewer independently re-broke the ordering → test FAILED ("Got 64… resolver bypassed") → reverted; probed 128→160/200→232/None→64 all tracking. `three_int_shape_crossing_local_uses_three_llvm_slots` → `n_locals==3` (LLVM 24B, ≠ fallback 1); `bool_bool_shape…` → `n_locals==1`. 6/6 unit tests pass. (Inherent limit documented: the resolver-BUILDING path goes live only when the reject lifts — Phase 2 scope; Phase 1 proves the COMPOSITION with a real cross-module size.)
- [x] Cycle recovery (Guard G3) present; a circular-import fixture does not infinite-loop or ICE (still emits the existing clean circular-import diagnostic).
  - Evidence: (acceptance r3) `frame_layouts_cycle_initial`/`frame_layouts_cycle_fn` (`queries.rs:37-57`) wired into the `#[salsa::tracked]` attribute; `circular_import_returns_empty_map_not_infinite_loop` passes (finite, empty map). Primary mechanism: early-return on `check.diagnostics.has_errors()`; cycle fns are defense-in-depth.
- [x] `compute_composed_frame_size`/`typeck_type_frame_slots` still present (deleted in Phase 2) but NO LONGER READ by codegen for any case the query now covers — OR documented why an interim read remains. No new lossy path added.
  - Evidence: (acceptance r3) `grep composed_frame_size crates/ynz-codegen/src/{emit,queries}.rs` → only doc-comments; the old `imported_fns.get(name).filter(sig.composed_frame_size>0)` block is DELETED (the fn signature dropped `imported_fns` entirely, takes `&dyn Fn(&str)->Option<u64>` resolver). `compute_composed_frame_size`/`typeck_type_frame_slots` untouched in `resolve_import.rs:333,376` (Phase 2 deletes). No new lossy path.
**Quality gate**:
- [x] Inkwell `Context` does not escape the query; return type holds no inkwell types. — `FrameLayout` is plain data; Context block-scoped + dropped before the Arc returns; `codegen_query_returns_owned_bytes_not_inkwell_types` passes (code-reviewer r1).
- [x] Guard G1 honored: the query and `emit_artifact` use the SAME target constructor (grep shows one definition, two callers). — `default_target_machine()` (state_machine.rs:748): callers at queries.rs:154 + emit.rs:664.
- [x] No `as any`-class escapes (Rust: no stray `unwrap`/`unsafe` added without justification); no test weakening. — rules r3 + code-reviewer r3 PASS; the redundant query-side `initialize_x86` + unused imports removed.
- [x] Recursion terminates on the import DAG; cycle recovery covers the circular case. — judge-pass-down + acceptance r3; G3 cycle recovery + early-error-exit.
**Verification**: `cargo test --workspace`; the new `frame_layouts_query` unit tests; full M3a fixture sweep through `./target/debug/ynz run` (byte-identical); `--no-auto-parallel` consistency check.

**Phase Review Gates** (filled at phase completion by coordinator):
- [x] code-reviewer: PASS 2026-06-06T (round 3; CAUGHT the round-2 dead-resolver bug via adversarial probe; round-3 independently re-broke the ordering to confirm the test bites, then probed 128/200/None — all track; byte-identity holds)
- [x] rules-compliance-reviewer: PASS 2026-06-06T (round 3; doc comment merged, fixture Yinz-vocab clean, no banned markers/phrases)
- [x] plan-adherence-verifier: PASS 2026-06-06T (round 3; all 6 steps MET, zero Phase-2 overreach, fixture deviation justified)
- [x] acceptance-verifier: PASS 2026-06-06T (round 3; AC3 rigorously re-verified — resolver-return demonstrably drives the output, 72≠64≠88; not a fallback coincidence)
- [x] design-compliance-reviewer: PASS 2026-06-06T (round 1; no coloring; query in right crate; typeck stays LLVM-free — carried, no design-relevant change since)
- [x] deviation-judge #1 (approach: pass-down vs thread-db): PASS 2026-06-06T (round 1; sound deferral — db threads cleanly into build_module/Cg in Phase 2; the one gap is exactly Phase 2 Step 1's scope)
- [x] deviation-judge #2 (approach: removed build_frame_layouts wrapper): PASS 2026-06-06T (round 1; truly dead — zero callers; resolver byte-identical for local case)
- [x] Committed: 80789c8

**Findings Log** (filled during any fix loops by the coordinator):
- 2026-06-06 — Gate round 1 (5 reviewers + 2 judges). 4 PASS: design-compliance (no coloring; query in right crate; typeck stays LLVM-free), judge-pass-down (deviation #1: pass-down structure is a SOUND deferral — db threads cleanly into build_module/Cg in Phase 2; the one gap, imported-callee n_locals at emit.rs:5495, is exactly Phase 2 Step 1's named scope; child offsets already composed via G2 in the parent's layout), judge-wrapper (deviation #2: removed build_frame_layouts is truly dead; resolver byte-identical for local case; strictly MORE accurate than the old composed_frame_size==0 fallback). BLOCK ×3:
  - **rules-compliance**: duplicated/self-contradictory doc comment on `is_number_errors_return` (emit.rs:209-218 — both halves open with the same sentence). → merge into one coherent comment.
  - **plan-adherence**: the new test file `crates/ynz-codegen/tests/frame_layouts_query.rs` is UNTRACKED → excluded from `git diff 6344ce2` → not a delivered/reviewed artifact. → `git add` it (coordinator staged via `git add -N`).
  - **acceptance (AC3 WEAK) + code-reviewer**: the marquee Guard-G2 recursion test `reexport_chain_b_total_size_includes_a_sub_frame` is a TAUTOLOGY — it calls `frame_layouts_query(a_sf)` (leaf) + asserts in-test arithmetic `32+0+32==64`; it NEVER calls `frame_layouts_query(b_sf)` and NEVER drives the `callee_size_resolver`, because B's `check_query` fires the universal reject → the query early-exits before the resolver. The recursion that fixes escape #1 has ZERO executing coverage. → rewrite to drive the recursion directly via `build_frame_layouts_with_resolver(B_typed, resolver→frame_layouts_query(a_sf))`, asserting B's composed total_size FROM the computation (not test arithmetic). NOTE the inherent limit: the resolver-BUILDING path (callee_source_map via resolve_module_path) goes live only when the reject lifts (Phase 2) — Phase 1 can prove the COMPOSITION with a real cross-module size; the full build-the-resolver path is Phase 2's adversarial-gate scope (document honestly).
  - **code-reviewer (also BLOCK)**: stale comment emit.rs:283-287 ("For build_module this comes from FunctionSig.composed_frame_size") — factually wrong (build_module no longer calls the fn; codegen never reads composed_frame_size). → rewrite to the actual single-caller (the query) + recursive resolver.
  - **Concerns folded into the fix round** (real, cheap, reduce Phase-2 risk): (a) strengthen the shape test — `n_locals==1` coincides with the `shape_frame_slots` fallback `unwrap_or(8)→1`; use a ≥3-int shape (LLVM 3 ≠ fallback 1) so the test can't pass on a silent sizing failure; (b) queries.rs:155 `Err(_) => empty map` silently swallows a target-machine-construction failure — document/assert it's dominated by `emit_artifact`'s `?` (codegen errors first) so it's not a live silent-wrong; (c) remove the redundant query-side `Target::initialize_x86` (queries.rs:157-160 — `default_target_machine` already inits).
  - **Surfaced to Phase 2** (added as Phase 2 prerequisites, NOT fixed in Phase 1 — dormant under the reject): (1) the namespace-import resolver branch (queries.rs:117-134) maps every `imported_fns` key to the current decl's source file (first-namespace-wins) — a latent wrong-resolver bug; Phase 2 MUST correctly resolve per-name OR loud-reject namespace-imported suspending calls before exercising them. (2) the emit.rs default-branch redundant `initialize_x86` cleanup (Phase 2 heavily edits emit.rs).
- 2026-06-06 — Fix round 1 + Gate round 2 (4 lanes re-run; design + 2 judges carried). 2 PASS (plan-adherence — all steps MET, test now tracked, no overreach; rules — adjudicated non-blocking: its Fix #1 self-retracted, Fix #2 was "not committed yet" = expected pre-commit state). acceptance returned PASS but is **OVERRIDDEN** by code-reviewer's decisive BLOCK (non-concession rule: an adversarial input that breaks the claim wins over a PASS on the same claim). **code-reviewer BLOCK — REAL PRODUCTION BUG (not a test issue): Guard G2's `callee_size_resolver` is DEAD CODE on the recursion path.** In `build_frame_layouts_with_resolver` the `compute_frame_size` loop runs BEFORE the resolver-seed loop → `compute_frame_size("doWork")` recurses into imported `getValue`, doesn't find it as a local fn, falls through to `FRAME_HEADER_SIZE` (32) and CACHES `sizes["getValue"]=32` → the later `sizes.entry("getValue").or_insert_with(resolver)` is a no-op → the resolver is NEVER consulted. Proven live: varying the resolver return Some(32)/Some(128)/None leaves `doWork.total_size` invariant at 64; a `Cell<bool>`-instrumented resolver confirms zero calls. The int fixture MASKS it (getValue's real frame == 32 == fallback). For any re-export callee with its own locals (frame > 32) this silently UNDER-SIZES the embed = escape #1's exact SIGILL, latent for Phase 2. The recursion test passes for the WRONG reason (fallback coincidence at 32) — same tautology as round 1, one layer deeper. acceptance MISSED it (accepted the test's claim without varying the resolver return — a `test-must-exercise-claimed-path` instance). NOTE: this ordering bug is pre-existing in the original `build_frame_layouts` too, dormant because the reject blocked imported callees — Phase 1's unit-proof is exactly what surfaced it before the Phase 2 lift. Concern (Phase 2): resolve_import.rs:765-767 stale comment ("composed_frame_size still used by importer's codegen") now wrong — Phase 2 sweeps it with the composed_frame_size deletion. → Fix round 2: seed `sizes` from the resolver for imported suspending callees BEFORE `compute_frame_size`; make the test use a callee with real frame > 32 so it FAILS if the resolver is bypassed (resolver-return must propagate to `doWork.total_size`).
- 2026-06-06 — Fix round 2 + Gate round 3 (4 lanes re-run; design + 2 judges carried). **ALL PASS → PHASE 1 GREEN.** FIX 1: resolver-seed moved BEFORE `compute_frame_size` so the recursion reads the resolver value (cache no longer poisoned with the header-only fallback); byte-neutral for local-only modules (empty seed loop). FIX 2: A's `getValue` given a crossing local → real frame=40≠32; test asserts `doWork.total_size`==72 with a `Cell` call-counter + an anti-bypass sentinel (resolver→56→88). **code-reviewer independently re-broke the ordering and confirmed the test FAILS (64≠72), then reverted** — the test genuinely guards. acceptance r3 rigorously re-verified (72≠64≠88, output tracks resolver). All 5 reviewers + 2 judges PASS. Carried to Phase 2: (a) namespace-import first-wins resolver bug (queries.rs:129-141 — already Step 1b); (b) 3 stale `build_frame_layouts` comment-name references in emit.rs (~908, ~2028, ~7594) — cosmetic, sweep when Phase 2 edits emit.rs; (c) resolve_import.rs:765-767 stale "composed_frame_size still used by importer's codegen" comment — sweep with the composed_frame_size deletion. **LESSON (for /learn): the adversarial gate (code-reviewer Section-4: vary the resolver return + Cell-instrument) caught a REAL silent-under-sizing bug (Guard G2 dead resolver) that acceptance's claim-trusting PASS missed — the M3a pattern exactly; proving the recursion in Phase 1 BEFORE the lift is what surfaced escape #1's SIGILL class ahead of the lift.**
- 2026-06-06 — Fix round 2. **FIX 1 (production, emit.rs):** Reordered `build_frame_layouts_with_resolver` — now pre-seeds `sizes[name]` for every imported suspending callee (name in suspend_set NOT in direct_children) by calling `callee_size_resolver` BEFORE the `compute_frame_size` loop. When compute_frame_size recurses into an imported callee, it finds the resolver's value in the cache (line 538: `if let Some(&cached) = sizes.get(fn_name) { return cached; }`) and does NOT fall through to the local-fn path. Old false comment removed; new comment explains the ordering requirement and byte-identity guarantee for local-only modules. **FIX 2 (test):** Updated a_ops.ynz fixture — `getValue` now has 1 int crossing local (`offset`, live across sleep) → frame = 40 bytes (NOT 32 = fallback). Updated `leaf_module_get_value_has_one_crossing_local` to assert n_locals=1, total_size=40. Updated `reexport_chain_b_total_size_includes_a_sub_frame`: asserts `doWork.total_size = 72` (not 64 = bypass value), adds `Cell<u32>` call counter confirming resolver IS called for "getValue", adds sentinel anti-bypass sub-assertion (resolver→56 → doWork=88, confirmed distinct from 72 and 64). **Adversarial proof (per task spec):** primary run resolver→40 → doWork=72; sentinel run resolver→56 → doWork=88; both call counters > 0. resolver IS called. **Byte-identity confirmed:** `cargo test -p ynz-codegen` → 31 golden IR + `object_file_sha256_matches_golden` all PASS. `cargo test --workspace` → all suites green (0 failures). `cargo clippy -D warnings` exit 0; `cargo fmt --check` clean.

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
1a. **(typeck prerequisite — RELOCATED from Phase 0 fix-round, 2026-06-06; verify-first then fix.) Fix the EC-method-dispatch-on-`let`-bound-EC-value typeck bug.** A pre-existing general bug (surfaced by the `reexport_ec_number` fixture, NOT cross-module-specific): inside an errors-capable function, `resolve_ident` auto-propagation (`check.rs:~2173`, fires when `current_fn_errors_capable = true`) strips `ErrorsCapable<T>` → `T` for a `let`/`const`-bound EC value; then `Expr::MethodCall` (`check.rs:~1825`, `infer_expr(receiver, None)`) hands the stripped bare type to method dispatch → `.or`/`.failed`/`.message`/`.suggestions`/`.trace`/`.source` are not found ("`number` does not have a method called `or`"). Without this fix the EC fixtures (`reexport_ec_number` target `total: 3.5`, plus the EC crossing-local/return matrix axes) cannot reach correct output once the reject lifts. **Verify the root cause live first** (a `let`/`const`-bound EC value + EC-method call inside an errors-capable function — reproduce minimally), THEN fix in `Expr::MethodCall`: infer the receiver (keep the `expr_types` side-effect codegen needs), and for EC-specific methods when the inferred receiver was auto-stripped to non-EC, re-lookup the scope binding to restore `ErrorsCapable<T>` for dispatch — a NARROW override that does not touch normal value-context auto-propagation. **This is a typeck behavior change → it gets the full adversarial gate** (EC dispatch is subtle: nested EC, shadowed bindings, EC-method on non-`let` receivers, non-EC functions, the `.failed`/`.message`/etc. siblings — all must still resolve correctly; no over-restoration). NOTE: a SEPARATE pre-existing codegen ICE on non-suspending imported `number errors` calls ("cannot convert i64 bits to Error") was observed during Phase 0 disambiguation — unrelated to this typeck fix and pre-dates M3e; if it blocks an EC matrix fixture, surface it (may need its own fix or a documented narrow defer).
1b. **(prerequisites carried from Phase 1 round-1 review — dormant under the reject, become live when it lifts):**
   - **Namespace-import resolver correctness.** `frame_layouts_query`'s `callee_source_map` build (`crates/ynz-codegen/src/queries.rs:117-134`) currently maps EVERY `imported_fns` key to the current decl's source file for a namespace import (first-namespace-wins) — a latent wrong-resolver bug that would feed a WRONG callee layout into the importer. Before exercising namespace-imported suspending calls, either resolve each name to its true origin module OR loud-reject namespace-imported suspending calls (the matrix uses named imports; add at least one namespace-import cross-module suspending fixture that runs correctly OR rejects cleanly — never silently mis-resolves).
   - **emit.rs redundant `initialize_x86` cleanup.** The default-triple branch of `emit_artifact` still calls `Target::initialize_x86` redundantly (the override branch genuinely needs it; the default branch gets it from `default_target_machine()`). Since this phase heavily edits emit.rs, move the init into the override branch only (or document the idempotent no-op). Non-correctness, but it's been flagged across Phase 0/1.
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
- [x] **NO SILENT FAILURE**: every analyzable cross-module suspending combo RUNS correctly (live `ynz run`, exact stdout, exit 0, ≥2× identical) — direct/transitive/re-export × {int, bool, float, string, number/decimal128, shape, errors-capable} × {crossing-local, loop-var, return}. NOWHERE a silent-wrong value or SIGILL/abort. The 5 former escapes all RUN correctly.
  - Evidence: (acceptance-verifier R2 OVERALL PASS + coordinator-live-verified rounds 1–5) 19/19 m3e fixtures run correctly via `./target/debug/ynz run`, deterministic ≥2×, exit 0, alloc==free. All 5 escapes covered: #1 `reexport_chain_int` → `result: 7`; #2 `shape_crossing_local_direct` → `x: 3, y: 7` + `imported_shape_crossing_local` → `1 2 3`; #3 `ec_crossing_local_direct` → `before: 5 / result: 99 / after: 5` + `loud_reject_ec_transitive` → `got: 42`; #4 `number_crossing_local_direct` → `before: 1.5 / after: 1.5`; #5 `caller_own_frame` → `a: 1, b: 2, c: 3, result: 100`. Stacked: `reexport_ec_number` (stacks #1+#3+#4) → `total: 3.5`. Round-5 `alias_local_name_collision` runs deterministic with `IMPORTED-OK` ×3 (3-state proof confirms the regression-lock genuinely bites).
- [x] The universal-reject guard is removed; a former-rejected cross-module suspending call now compiles + runs (the M3b reject fixtures flip to correct-output).
  - Evidence: `crates/ynz-typeck/src/queries.rs` -242 lines (reject guard + helpers deleted). All 9 former-reject M3b fixtures pass with `// test-ratchet:` annotation: `v03_m3b_loud_reject_ec_transitive` → `got: 42`; `v03_m3b_loud_reject_reexport` → exit 0; `v03_m3b_loud_reject_shape_crossing` → `1`; `v03_m3b_cross_module_*` 5 fixtures all exit 0.
- [x] `compute_composed_frame_size`, `typeck_type_frame_slots`, and `FunctionSig.composed_frame_size` are DELETED; `grep` shows zero remaining references; the 7 LSP literals + stale comment updated; `cargo test --workspace` green.
  - Evidence: `grep -rn --include=*.rs 'composed_frame_size\|compute_composed_frame_size\|typeck_type_frame_slots' crates/` → zero output. `resolve_import.rs` -368, `signatures.rs` -16 (replaced with `original_name: Option<String>` per round-3 refactor — Deviation #7), 7 LSP literals (hover.rs 3 + completion.rs 4) updated to drop the field + add `original_name: None`. `cargo test --workspace` exit 0.
- [x] Genuinely-unanalyzable cross-module edges (dynamic-dispatch-through-vtable, FFI) still loud-reject cleanly (exit 1, WHAT/WHAT-INSTEAD/WHY) — NOT via the deleted frame guard, via the existing unresolvable-edge path.
  - Evidence: `v03_m2_cant_infer_dynamic_dispatch_exits_nonzero_with_teaching_error` passes; stderr contains "Can't determine whether `doWork` suspends — it's a dynamic-dispatch call through a `dynamic Worker` vtable" (WHAT/WHAT-INSTEAD/WHY format). Reject comes from the existing `may_block` unresolvable-edge path (the deleted guard was at queries.rs:366-402, different code path).
- [x] The relocated EC-method-dispatch typeck fix (Step 1a) is verified live + adversarially gated: a `let`-bound `T errors` value inside an errors-capable function resolves `.or`/`.failed`/`.message`/`.suggestions`/`.trace`/`.source` correctly; the `reexport_ec_number` fixture reaches its correct output once the reject lifts; and normal value-context auto-propagation + non-EC method dispatch are NOT regressed (no over-restoration on nested EC / shadowed bindings / non-`let` receivers / non-EC functions).
  - Evidence: `reexport_ec_number` → `total: 3.5` deterministic. 11 EC adversarial regression tests pass at `ynz-typeck/tests/check.rs`: `ec_method_dispatch_failed_and_message_resolve_in_ec_fn`, `_no_over_restoration_when_inner_shadows_outer_ec`, `_named_call_on_non_ec_binding_no_restoration`, `_named_call_in_non_ec_fn_no_restoration`, `_message_resolves_in_ec_fn`, `_suggestions_resolves_in_ec_fn`, `_trace_resolves_in_ec_fn`, `_source_resolves_in_ec_fn`, `_dispatch_on_const_bound_ec_value_in_ec_fn` (const-bound coverage), `_dispatch_after_failed_guard_narrowing` (errors_success_narrowed channel). All 209/209 ynz-typeck tests pass.
- [x] The 4 stacked-mis-sizing adversarial axes (5a.i-iv) RUN correctly live: (i) double-call same callee with live crossing-local between → no clobber; (ii) re-export × EC × number cross-product → correct value; (iii) diamond import (shared leaf D) → consistent embed + correct compose; (iv) `--kernel` cross-module suspending call → clean kernel reject (exit 1), not a codegen path.
  - Evidence: (i) `v0_3_m3e_double_call_crossing_local` → `start: 7 / mid: 7 / end: 7`. (ii) `v0_3_m3e_reexport_ec_number` → `total: 3.5`. (iii) `v0_3_m3e_diamond_import` → `a: 10 / b: 20`. (iv) `kernel_mode_rejects_cross_module_suspending_call` (typeck-API test; no `--kernel` CLI flag exists — plan correction documented in Findings Log; the typeck reject means codegen is never reached). Round-5 added `kernel_mode_rejects_cross_module_suspending_method_call` for UFCS coverage + `wait_suspending_in_kernel_mode_produces_exactly_one_diagnostic` for the no-double-diagnostic invariant.
- [x] The "total_size + n_locals suffice" assumption is confirmed live across the matrix (no combo required internal offsets), OR the FrameLayout enrichment it required is implemented + gated.
  - Evidence: `FrameLayout` struct unchanged across the diff (5 fields: `total_size`, `n_locals`, `children`, `recursion_slot`, `number_errors_staging_offset`). Full 19-fixture matrix runs correctly without enrichment. Round-5 added a `frame_layouts_query` enrichment (keyed by local alias name) to populate IMPORTED suspending callees' standalone layouts so the background-spawn path can look them up — this is a query-output enrichment (keying convention), NOT a `FrameLayout` struct enrichment; struct unchanged.
- [x] `--no-auto-parallel` ≡ default produce byte-identical stdout/stderr/exit on every fixture.
  - Evidence: `v03_m3e_cross_module_no_auto_parallel_byte_identical` test (integration.rs ~2331) builds 4 cross-module fixtures (`reexport_ec_number`, `diamond_import`, `caller_own_frame`, `alias_import_direct`) under both default and `--no-auto-parallel`, asserts SHA-256 byte-identical. Acceptance-verifier R2 confirmed via direct SHA-256 comparison on 4 binaries.
**Quality gate**:
- [x] **NO SILENT FAILURE**: 19/19 m3e fixtures correct + deterministic ×2 (acceptance R2 + coordinator round-5 re-verify); the round-3 regression in `lower_expr_background_state_machine` (S1 returned) was caught by round-4 judge D7 + code-reviewer R4 and CLOSED in round-5. 3-state proof on `alias_local_name_collision` proves the regression-lock now genuinely bites (STATE B → test FAILS with `LOCAL-BUG`).
- [x] Every reject path clean WHAT/WHAT-INSTEAD/WHY (dynamic-dispatch reject text validated; namespace-import reject clean; kernel-mode reject clean — all WHAT/WHAT-INSTEAD/WHY). All runs deterministic across ≥2× per acceptance-verifier R2 + coordinator independent verification.
- [x] No leaks (alloc=free on all 19 m3e fixtures: `YNZ_ALLOC_COUNTER`-confirmed `alloc=1 free=1` or `alloc=4 free=4` matching spawn count, no leaks).
- [x] No `as any`-class escapes per rules-compliance R2; no test weakening — the matrix flip carries `// test-ratchet:` + WHY comments asserting MORE behavior.
**Verification**: the full danger matrix (incl. the 4 stacked-mis-sizing axes 5a.i-iv) through `./target/debug/ynz run` (≥2× each, alloc-count, ordering); `cargo test --workspace`; `grep -rn --include=*.rs 'composed_frame_size\|compute_composed_frame_size\|typeck_type_frame_slots' crates/` returns nothing (scoped to `.rs` so fixture `.ynz` comments don't false-positive); `--no-auto-parallel` consistency sweep; `adversarial-tester` agent on the lift diff BEFORE the reviewer fan-out.

**Phase Review Gates** (filled at phase completion by coordinator):
- [x] code-reviewer: PASS 2026-06-07 (round 5 surgical fixes — all round-4 BLOCKs closed; 3-state proof on `alias_local_name_collision` regression-lock genuinely fails on STATE-B revert)
- [x] rules-compliance-reviewer: PASS 2026-06-06 (round 2; carried — no rule violations; test-ratchet + WHY comments correct; no banned jargon)
- [x] plan-adherence-verifier: PASS 2026-06-07 (round 5; all round-4 BLOCKs closed — initialize_x86 moved to override branch, check_generic_fn_call deferral comment, alias_collision strengthened)
- [x] acceptance-verifier: PASS 2026-06-06 (round 2 OVERALL PASS — all 8 ACs MET with live evidence; re-confirmed by coordinator live verification of round-3/4/5 fixes)
- [x] design-compliance-reviewer: PASS 2026-06-06 (round 2 + round 4; zero function-coloring; approved codegen-query divergence + new `original_name` mechanism consistent with no-coloring model)
- [x] deviation-judge #1 (scope: m3b ec-transitive `.or(0)` fixture): PASS R2 2026-06-06 (`.or(0)` default 0 ≠ expected 42, tight; no error branch reachable in fixture)
- [x] deviation-judge #2 (scope: P1 test reject-assert removal): PASS round-3 fix 2026-06-06 (inverse assertion `!b_check.diagnostics.has_errors()` added at frame_layouts_query.rs:166; resolver counter + 56→88 sentinel intact)
- [x] deviation-judge #3 (scope: kernel-reject test + EC adversarial tests in typeck/tests): PASS round-3 fix 2026-06-06 (kernel test strengthened to BARE form; 11 EC adversarial tests cover sibling methods + const-bound + errors_success_narrowed channel; AC Step 1a satisfied)
- [x] deviation-judge #4 (approach: background-spawn frame sizing — superseded by Deviation #7 + round-5 surgical fix): PASS R5 2026-06-07 (`original_name` mechanism + `frame_layouts_query` enrichment for imported callees; collision fixture strengthened with sentinel + 3-state proof; live verification ≥3× deterministic)
- [x] deviation-judge #5 (approach: aliased resume + layout resolution): PASS R2 2026-06-06 (5 adversarial strategies probed; 3-hop alias chain + UFCS + duplicate-name detection + non-suspending alias mixed — no regressions introduced by the fix)
- [x] deviation-judge #6 (approach: kernel-mode reject at call-dispatch arm + UFCS): PASS R5 2026-06-07 (`wait_suspending_in_kernel_mode_produces_exactly_one_diagnostic` confirms single-diagnostic; UFCS + bare both guarded; `check_generic_fn_call` deferral documented per `no-duct-tape.md` 4-field format — vacuously safe today, trigger named)
- [x] deviation-judge #7 (approach: `original_name` on FunctionSig supersedes round-2 mechanisms): PASS R5 2026-06-07 (4 callee-resume sites + 2 background-spawn paths uniformly resolve exported name; collision fixture strengthened so output PROVES dispatch — STATE-B revert makes the integration test FAIL with `LOCAL-BUG` sentinel)
- [x] Committed: cbd027e

**Findings Log** (filled during any fix loops by the coordinator):
- 2026-06-06 — **PARTIAL STATE — executor first pass landed, adversarial gate NOT YET RUN, NOT committed.** Base = `80789c8`. The executor's first pass did the CORE lift; coordinator spot-verified live (≥2× deterministic): reject guard DELETED; lossy reimpl + `composed_frame_size` FULLY DELETED (grep-clean in non-comment code); 7 LSP literals updated; `emit.rs` (+21, importer wiring) + `check.rs` (+61, EC-method-dispatch fix) modified (executor's Files-Modified list OMITTED these — reporting gap, not work gap); `reexport_ec_number` → `total: 3.5` exit 0; `shape_crossing_local_direct` → `x: 3, y: 7` exit 0. **The MANDATORY adversarial gate (adversarial-tester + 5 reviewers + 2 judges) has NOT run** — per the M3a precedent (27/7/8 rounds) this is exactly where silent-wrong cases surface, so the lift is NOT trusted/committed until it passes. **Unverified / likely-incomplete (gate must drive):** 5a.iv `--kernel` reject; namespace-import resolver fix-or-reject (Step 1b — the Phase-1 first-wins bug is still latent); alloc=free leak assertions; `--no-auto-parallel` consistency (step 7); the full ≥2× determinism sweep (only 2 fixtures spot-checked); the "total_size+n_locals suffice" assumption verified adversarially (vary a callee's crossing-local count → confirm the embed tracks it, not coincidental). Deviations + the full DONE-vs-REMAINING breakdown persisted in `.claude/plans/scratch/v0-3-m3e-cross-module-frame-serialization-phase2-deviations.md`. **RESUME via `/execute-plan` Step 2** (partial-state detection: working-tree diff present touching Phase-2 files, gates incomplete, uncommitted) — run the adversarial-tester FIRST, then the full fan-out; do NOT commit until the full matrix is green through the real compiler AND all reviewers + judges PASS.

- 2026-06-06 — **RESUME (new session). Adversarial-tester FIRST (plan-mandated) + 3 executor fix rounds → full matrix GREEN; formal gate next.** Coordinator established live ground truth on the partial-state diff (base `80789c8`): `cargo test --workspace` green (one transient `sync_bridge`/`rss` timing flake, re-ran clean), clippy `-D warnings` exit 0, fmt clean, jargon 9/9, golden IR + `object_file_sha256_matches_golden` byte-identical (intra-module unchanged), lossy reimpl (`composed_frame_size`/`compute_composed_frame_size`/`typeck_type_frame_slots`) grep-clean (zero refs), dynamic-dispatch reject intact (exit 1 + teaching). The 14→17 danger-matrix fixtures all RUN correctly via `ynz run`, deterministic ≥2×, alloc==free (`alloc=1 free=1`), `--no-auto-parallel` ≡ default byte-identical (build flag — it DOES exist; an executor claim that it "doesn't exist" was false, it's `#[arg(long, hide=true)]` on `build`).
  - **adversarial-tester** (`.analysis/adversarial.md`) found 3 silent-miscompile candidates beyond the matrix; coordinator verified each LIVE: **S1 CONFIRMED** (`background` of an imported suspending fn undersized its frame — `lower_expr_background_state_machine` emit.rs:9263 used an arg-count-only fallback ignoring the callee's crossing-locals → `/tmp/adv_s1b` 12-locals×3-spawns → `malloc(): invalid size`/tokio abort; small frames were benign-by-allocator-rounding luck). **S2 CONFIRMED** (`import { getValue as getV }` of a suspending fn → linker failure + garbage error; resume symbol + layout were keyed by local alias, not exported name). **S3 REFUTED** (8-field imported shape crossing-local runs correctly — `shape_abi_sizes` includes imported shapes).
  - **Fixes landed (3 executor rounds, all live-re-verified by coordinator — not trusting self-reports):** (1) S1 — `codegen_query` builds `frame_layouts_for_emit` augmenting local layouts with imported-callee standalone entries keyed by local name (sourced from `frame_layouts_query(callee_sf)[exported_name]`); background-spawn now finds the real size (`/tmp/adv_s1b` → `sum: 78`×3 exit 0). (2) S2/Gap-B — `local_to_exported_names` map in query + `Cg.local_to_exported`; every resume-fn site declares/looks-up by EXPORTED name (`/tmp/adv_s2` → `result: 7`; bare + EC aliased both work). (3) Gap A (axis 5a.iv) — kernel reject test added (see kernel-flag note below). (4) Gap C — float fixture STRENGTHENED to a corruption-detecting observable (`float > float → boolean`, printed before+after; before==after==true, so a float zeroed by suspension would flip the after-bool and FAIL) — the `float.toString()` decimal128-zero render + `float.toNumber()` link-fail are PRE-EXISTING base bugs, FILED to `.claude/todos.md` (`float-toString-renders-decimal128-zero`, `float-toNumber-link-fails`), out of M3e scope. (5) S3 — imported-shape-crossing-local regression-lock fixture added. (6) Regression fixtures added: `v0_3_m3e_alias_import_direct` (runs, `7`), `v0_3_m3e_namespace_import_rejects` (clean reject exit 1 — `ns.fn()` member-call is a GENERAL pre-existing Yinz limitation, so the namespace cross-module suspending case cleanly rejects and the Gap-B first-wins resolver concern is DORMANT/unreachable). 17/17 m3e tests pass.
  - **PLAN CORRECTION (axis 5a.iv — surfaced loudly per no-duct-tape):** the plan assumed a `--kernel` CLI flag for a driver-level kernel-reject fixture. **No such flag exists** (`ynz build`/`run` expose only `emit_ir`/`json`/`no_auto_parallel`/`keep`/`reveal_sensitive`; zero `kernel` in `crates/ynz-driver/src/main.rs`). Kernel mode is reachable ONLY via the `check_with_kernel_mode` typeck API. Axis 5a.iv is therefore satisfied by a typeck-level test (`crates/ynz-typeck/tests/check.rs::kernel_mode_rejects_cross_module_suspending_call`) that imports a suspending fn, `wait`s it cross-module, and asserts the kernel reject fires — which IS the invariant's intent (a typeck rejection means codegen is never reached, so the lift cannot have opened a codegen path under `--kernel`). This file is outside the Phase-2 `Files (expected scope)` list; documented as Scope Deviation #3 (judged).
  - 5 deviations documented in the scratch (3 scope incl. the kernel-test placement, 2 approach for the S1/S2 fix mechanisms) — coordinator-identified because the executors self-reported "None"; each gets an adversarial deviation-judge. **NEXT: formal gate — 5 reviewers + 5 judges on `git diff 80789c8`.**

- 2026-06-06 — **Gate round 1 (5 reviewers + 5 deviation-judges on `git diff 80789c8`).** 5 PASS: rules-compliance (no violations; test-ratchet correct; new graveyard corpse load-bearing), design-compliance (zero function-coloring; approved codegen-query divergence consistent; all fixes within no-coloring model), acceptance-verifier (OVERALL PASS — all 8 ACs MET live, incl. SHA-256 byte-identical `--no-auto-parallel`), judge#1 (ec-transitive `.or(0)` tight — default 0≠expected 42), judge#2 (P1 test reject-assert removal: resolver counter + 72-byte + sentinel survive). **BLOCK ×4 + 1 judge-equivalent:**
  - **judge#4 + judge#5 (CRITICAL, same root, live heap corruption):** `codegen_query`'s `frame_layouts_for_emit` augmentation uses `.entry(local_name).or_insert_with(...)` at `queries.rs:277-279` → when a local fn shares a name with an import ALIAS (`import { getV as doWork }` + local `function doWork()`), the local's small layout WINS and the imported callee's real (larger) layout is discarded → `background doWork()` undersizes the frame → `munmap_chunk(): invalid pointer` / `malloc(): invalid size`. Coordinator re-verified clean (`/tmp/adv_collide` → non-deterministic `s: 666` vs empty = heap corruption). FIX: `or_insert_with` → `insert()` (imported layout must win for its alias key; the local fn's standalone layout is irrelevant to the background path). The fix is WIDER than the stated problem (S2 alias) — classic judge catch.
  - **code-reviewer:** (1) EC-method-dispatch fix (check.rs:1823-1909) has ZERO adversarial regression-lock tests despite AC Step 1a mandating them; (2) `emit_suspending_call_heap_boxed` (emit.rs:5748) builds the resume symbol with the raw alias name — 3 of 4 callee-resume sites were fixed to use the exported name, this one was missed (latent linker-failure landmine). Cross-flagged 11 stale fixture comments.
  - **plan-adherence:** stray `composed_frame_size` comments in 2 m3b fixture `.ynz` files (Step 8 sweep missed); `initialize_x86` cleanup (Step 1b) not done; no committed cross-module `--no-auto-parallel` test (Step 7). [cspell.json flag was a FALSE POSITIVE — coordinator verified 80789c8 already had all 129 words incl. the trading words; the only real change was `+malloc`; the verifier misread the script's sort-reorder. Coordinator minimized the diff to a single `+malloc` line. graveyard/todos meta-file changes are legit /learn + deferral-tracking artifacts, documented in the Findings Log.]
  - **judge#3 (kernel):** the kernel test fires on the `Expr::Wait` keyword, not the cross-module callee — it'd pass identically with `wait sleep(100)`. Coordinator EMPIRICALLY CONFIRMED (temp probe, error_count=0): a BARE (no-`wait`, auto-suspension) cross-module suspending call under `check_with_kernel_mode` is NOT rejected (the `name =>` call arm at check.rs:~2412 has no kernel guard; only Wait/Background/sleep arms do). Since every real Yinz suspending call uses the bare form, the kernel-mode contract has a real hole. FIX: add a kernel-mode reject in the call-dispatch arm when `kernel_mode && callee.suspends` (the general no-coloring contract — suspension is suspension whether or not the user wrote `wait`); strengthen the kernel test to the bare form.
  - **NEXT: fix-round-2 executor (7 findings), then re-gate.** 11 stale fixture comments folded into the fix set (non-concession: confirmed violation fixed regardless of which agent caught it).

- 2026-06-06 — **Gate round 2 + Fix round 2 + Gate round 3.** After round-1's 4 BLOCKs were closed (Findings 1–6 — float-strengthening, kernel-test-strengthening, EC adversarial gating, namespace-fixture, --no-auto-parallel byte-identity, alias_local_name_collision fixture), round-2's full re-gate (5 reviewers + 6 judges) found 6 NEW BLOCKs: judge D2 (b_check.has_errors inverse assertion missing), code-reviewer R2 + plan-adherence + judge D4 (alias-collision fixture Potemkin — no real local doWork), judge D3 (EC errors_success_narrowed channel + sibling-method coverage gaps), judge D6 (double-diagnostic on `wait <suspending>()` in kernel mode), code-reviewer RF2 (UFCS + generic kernel-reject gaps), code-reviewer RF4 (const-bound EC test). Fix-round-3 executor addressed the 6 BLOCKs BUT also made an unauthorized architectural refactor: replaced the round-2 `local_to_exported_names` map + `frame_layouts_for_emit` augmentation with a single `original_name: Option<String>` field on `FunctionSig` (cleaner mechanism, but unauthorized scope expansion). Round-3 gate found the refactor sound on 3 of 4 callee-resume sites but **regressed the original S1 bug** in `lower_expr_background_state_machine` (the SM background-spawn path) — missing both alias resolution AND imported-callee frame-layout lookup → heap corruption for `background <imported-suspending-with-crossing-locals>()`. Additionally: alias_local_name_collision fixture was a Potemkin lock (output `s: 666` was main-thread arithmetic independent of which callee was spawned — objdump-confirmed by code-reviewer R4 that LOCAL doWork was dispatched, not imported `compute`). Round-4 gate (3 reviewers + 1 new judge D7) returned 3 BLOCKs: judge D7 (alias dispatch wrong order at lower_expr_background), code-reviewer R4 (lower_expr_background_state_machine missing alias+frame resolution), plan-adherence R4 (initialize_x86 unmoved at emit.rs:669, check_generic_fn_call kernel guard missing).
- 2026-06-07 — **Fix round 5 (surgical) — all round-4 BLOCKs CLOSED.** Coordinator-mandated NO MECHANISM REFACTORING; complete the `original_name` approach correctly. Executor addressed 5 findings: (1) `lower_expr_background_state_machine` (emit.rs:9297-9310) now resolves `callee_exported` via `imported_fns.original_name`, AND `frame_layouts_query` populates `FrameLayout` stubs for imported suspending callees keyed by local alias (queries.rs:215-244) so `cg.frame_layouts.get("doWork")` returns the imported callee's REAL `total_size`+`n_locals`; (2) `lower_expr_background` check order inverted — `original_name` checked before `get_function`; (3) `initialize_x86` moved into `Some(t) =>` override branch only (emit.rs:681-682); (4) `check_generic_fn_call` four-field deferral comment added (check.rs:3006-3023; generic+suspension is vacuously safe today — no `suspends` field on `GenericFnSig` yet; trigger documented); (5) **alias_local_name_collision fixture redesigned** — imported `compute` prints `IMPORTED-OK`, local `doWork` prints `LOCAL-BUG`; integration test asserts `contains("IMPORTED-OK")` AND `!contains("LOCAL-BUG")`. **3-STATE PROOF (coordinator live-verified, the gold standard):** STATE A (post-fix) → test passes; STATE B (revert `resume_fn_name(callee_exported)` → `resume_fn_name(callee_name)` in lower_expr_background_state_machine) → test FAILS with assertion panic at integration.rs:2285 (the LOCAL-BUG sentinel appears, regression-lock GENUINELY bites); STATE C (restored) → test passes again. The regression fixture is no longer Potemkin. Independent `/tmp/r5a` probe (4 crossing-locals × 3 spawns of an aliased-imported callee) → `w: 110` correct, exit 0, deterministic, no heap corruption. Full suite green: 19/19 m3e tests, 209/209 typeck tests, 31 golden IR + object_file_sha256_matches_golden byte-identical, clippy + fmt + jargon clean, `--no-auto-parallel` byte-identical. Phase 2 GREEN by every measure.

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
