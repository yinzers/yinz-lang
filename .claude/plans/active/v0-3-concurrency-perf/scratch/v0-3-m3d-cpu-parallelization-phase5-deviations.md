# v0-3-m3d-cpu-parallelization Phase 5 Deviations — captured 2026-06-19

D_count: 4

(Combined diff = original Phase-5 test-only adversarial gate + the authorized in-phase
decimal128-return leak fix. Original Phase-5 executor reported ZERO deviations; all 4 below
come from the fix executor.)

## Scope Deviations (verbatim from executor reports)

Original Phase-5 executor: None — stayed within declared scope.

Fix executor:
- **Scope Deviation #1** (file: `crates/ynz-codegen/src/emit.rs`): touched outside Phase-5's
  declared test-only "Files (expected scope)". Rationale: AUTHORIZED — Patrick's 2026-06-19
  fix-in-phase decision (Findings Log coordinator-probe entry) + the Phase-5 deviation rule
  "M3d defect → fix commit within this phase". The file is in the plan front-matter `files:`
  list; the leak is a confirmed M3d-path-only defect on a core promoted class (decline-around
  unavailable), so the fix belongs in-phase per Rule 11 + no-duct-tape. Diff hunks:
  `crates/ynz-codegen/src/emit.rs:3157-3187`.

## Approach Deviations (verbatim from executor reports)

Original Phase-5 executor: None — implementation matched plan's named approaches.

Fix executor:
- **Deviation #1** (Your Fix steps 1–2): prompt said "add the missing free … free the
  worker-stabilized decimal128 heap allocation once its i128 has been copied into the caller's
  stable storage", I did "eliminated the heap allocation entirely — the SM wrapper now returns a
  stack-backed pointer matching the non-SM number ABI, so there is no heap block to free".
  Rationale: verify-first (LLVM IR) showed the caller's i128 copy happens at the caller's own
  call site AFTER the wrapper returns (main is byte-identical in both modes and never frees the
  returned pointer); a free inside the wrapper would dangle the pointee = the UAF the prompt
  explicitly warned against. The non-SM number ABI is already "return pointer to caller-local
  i128, caller copies-and-forgets, no free" — matching it in the SM wrapper makes the two
  lowerings ABI-identical (no-duct-tape: fixes the root mismatch, not a symptom) and is strictly
  a no-allocation fix, eliminating leak + double-free + UAF risk simultaneously. Diff hunks:
  `crates/ynz-codegen/src/emit.rs:3157-3187`.
- **Deviation #2** (Witness retirement): prompt said update only the two number-arm fixtures'
  KNOWN LEAK header notes, I additionally deleted the now-dead
  `m3d_assert_fires_byte_identical_no_alloc_check` helper (its only 2 callers were the rerouted
  number cells). Rationale: no-duct-tape — leaving an orphan unused test helper is dead code;
  removing it is part of a complete retirement. Diff hunks:
  `crates/ynz-driver/tests/integration.rs:5759-5799, crates/ynz-driver/tests/integration.rs:6448-6516`.
- **Deviation #3** (Plan writeback): prompt scoped writeback to the AC-1 Danger-matrix finding
  sub-bullet, I also corrected the AC-5/Step-10 sweep evidence (line 811): 2081→2080 test count,
  "test-only/zero-compiler-source" claim → "one authorized emit.rs touch", and logged the
  pre-existing m3e flake. Rationale: my diff falsifies those specific stated facts; leaving
  known-false statements in the plan violates no-duct-tape. Did NOT tick any AC checkbox or
  Phase Review Gate. Diff hunks:
  `.claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:801, .claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:811`.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: scope
- **rationale**: AUTHORIZED scope deviation — emit.rs touched outside Phase-5 test-only scope per Patrick 2026-06-19 fix-in-phase decision + Phase-5 deviation rule (M3d defect → fix commit within phase).
- **diff hunks**: crates/ynz-codegen/src/emit.rs:3157-3187
- **judge identity hash**: 1864320ba898c2eab7a69086ccda2d8268bcc08b
- **carry status**: fresh

### Deviation #2
- **type**: approach
- **rationale**: Eliminated heap allocation entirely; SM wrapper returns stack-backed pointer matching non-SM number ABI rather than adding a free (verify-first IR showed a free inside the wrapper would dangle the pointee = UAF).
- **diff hunks**: crates/ynz-codegen/src/emit.rs:3157-3187
- **judge identity hash**: 327df07cbc1cb5718738484eaa157faf80b44c5a
- **carry status**: fresh

### Deviation #3
- **type**: approach
- **rationale**: Deleted the now-dead m3d_assert_fires_byte_identical_no_alloc_check helper whose only 2 callers were the rerouted number cells.
- **diff hunks**: crates/ynz-driver/tests/integration.rs:5759-5799, crates/ynz-driver/tests/integration.rs:6448-6516
- **judge identity hash**: 06c8c4139bbc61084c719bcb42bb065dd0243e16
- **carry status**: fresh

### Deviation #4
- **type**: approach
- **rationale**: Corrected AC-5/Step-10 sweep evidence (2081→2080 test count, zero-compiler-source claim → one authorized emit.rs touch, logged pre-existing m3e flake).
- **diff hunks**: .claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:801, .claude/plans/active/v0-3-concurrency-perf/v0-3-m3d-cpu-parallelization.md:811
- **judge identity hash**: a24a4368a4339322a1adf9ab8e3dc70316dc53f8
- **carry status**: fresh
