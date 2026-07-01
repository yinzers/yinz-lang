# Cross-Module Frame Serialization (M3e)

**Status**: Shipped in v0.3-M3e. See `.claude/planning/done/2026-06-05-v0-3-m3e-cross-module-frame-serialization/plan.md`
for the full execution record (Phases 0–3). Implemented in commit `cbd027e`
(Phase 2 — the lift) on branch `feat/m3b-auto-parallelization`.

---

## WHAT

Codegen-side cross-module `frame_layouts_query` — a salsa-tracked query in `ynz-codegen`
that computes LLVM-accurate `FrameLayout` values for every suspending function in a source
file. The importer's codegen, for an imported suspending callee, resolves the callee's
source file and calls `frame_layouts_query(callee_file)` to read the callee's real
`total_size` and `n_locals` — the two values the importer needs to reserve and place the
embedded sub-frame and cap the arg-write count. The query is salsa-memoized per source file
and uses a single shared target-machine/data-layout constructor (Guard G1) so importer and
exporter always use byte-identical LLVM ABI sizes.

The mechanism corrects five classes of silent crash that the prior scalar `composed_frame_size`
approach could not handle:

- exact slot offsets for every crossing local (LLVM-derived ABI sizes, not typeck approximations)
- correct full sub-frame tree for re-export chains (recursive query call per callee)
- correct EC staging-slot interaction with child sub-frame offsets
- correct two-slot number/decimal128 crossing-local sizing
- correct transitive × caller-frame composition

---

## WHY

Two verified facts force the codegen-side query approach:

**Fact 1 — Separate compilation (decisive).** `ynz-codegen` compiles one LLVM module per
source file (one `.o` per file, linked by the system C linker). There is no single merged
LLVM module. The importer cannot reach into the callee's LLVM module at its own compile
time — the callee's `.o` comes from a separate `codegen_query` invocation. Any mechanism
that requires the importer to read data that was computed inside the callee's `emit_artifact`
pass must carry that data across the compilation boundary. The salsa in-process query result
(`Arc<HashMap<String, FrameLayout>>`) is the correct carrier: it is computed once per source
file, memoized, and available to any subsequent salsa query in the same process.

**Fact 2 — Shape ABI sizes require LLVM `TargetData` (decisive).** `FrameLayout` slot counts
depend on each crossing-local's size in bytes. For shape-typed crossing locals, the byte size
is the LLVM-padded ABI layout of the struct — target-dependent, alignment-padded, NOT the
typeck field-count approximation (8 bytes × N fields). `TargetData` exists only in
`ynz-codegen`. Any attempt to compute accurate layout in `ynz-typeck` would require either
re-creating the LLVM data layout string in the type-checker (rebuilding the lossy parallel
reimplementation we deleted) or linking LLVM into the type-checker (inverting the
frontend/backend split that keeps `ynz-typeck` LLVM-free).

**Why the prior mechanism was wrong.** The original deferral doc prescribed serializing the
full `FrameLayout` struct into the export table and carrying it in the typeck-side
`FunctionSig`. This fails on both facts: you cannot compute an accurate `FrameLayout` in
typeck (Fact 2), and carrying it in `FunctionSig` would be serializing an in-process value
across a fictitious boundary (separate compilation means the callee's layout is computed by
its own `codegen_query`, not at import-resolution time in typeck — Fact 1). The codegen-side
query is the sound realization: one computation, LLVM-accurate, in the right crate.

---

## COST (implementation — already paid in v0.3-M3e)

1. Extract `frame_layouts_query(db, source) -> Arc<HashMap<String, FrameLayout>>` in
   `ynz-codegen/src/queries.rs`. Creates a local inkwell `Context` (dropped before return —
   only `u64`/`FrameLayout` data escapes, no inkwell types in return value).

2. One shared target-machine/data-layout constructor (Guard G1) — a single function owning
   `TargetMachine::get_default_triple()` + CPU `"generic"` + data-layout string, called by
   both `emit_artifact` and `frame_layouts_query`. No magic-string duplication.

3. Recursive cross-module child resolution (Guard G2): the callee-size resolver, for an
   imported suspending callee, calls `frame_layouts_query(callee_file)` — NOT the deleted
   `composed_frame_size` scalar. Re-export chains (A→B→C) recurse naturally and terminate
   (import DAG).

4. Salsa cycle recovery (Guard G3): `cycle_fn`/`cycle_initial` returning an empty map for
   the already-handled circular-import case.

5. Delete `compute_composed_frame_size` + `typeck_type_frame_slots` + `FunctionSig.composed_frame_size`
   (the lossy parallel reimplementation, no-duct-tape #7).

6. Lift the M3b Phase 1 universal-reject guard once the full danger matrix runs correctly
   through the real compiler.

The `.ynzlib` binary-package format (`design/future/packages.md`), when it ships, will need
to serialize `frame_layouts_query` results into the package artifact. That is a clean future
extension — the in-process salsa `Arc` is not a serialization blocker today, and pre-building
the serialization now would be premature (build-twice against a not-yet-specified wire format).

---

## TRIGGER

The universal loud-reject guard from M3b Phase 1 (any imported suspending function →
compile error) is the direct trigger: any user who wants cross-module suspension is blocked
until M3e ships. When user reports of the rejection accumulate, or when a stdlib module needs
cross-module suspension in any form, M3e becomes load-bearing.

M3e replaces the universal reject with correct codegen so all analyzable cross-module
suspending combos work. Genuinely-unanalyzable edges (dynamic-dispatch-through-vtable, FFI
cross-module suspending calls) continue to reject via the existing may-block unresolvable-edge
path — that is correct behavior, not a band-aid.
