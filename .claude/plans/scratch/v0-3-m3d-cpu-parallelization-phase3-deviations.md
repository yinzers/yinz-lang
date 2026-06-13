# v0-3-m3d-cpu-parallelization Phase 3 Deviations — captured 2026-06-13 (Slice 1, Round 3)

D_count: 2

> NOTE: Phase 3 is being built INCREMENTALLY across live-verified slices (Patrick decision,
> 2026-06-13), one commit at the phase boundary. This scratch file reflects the CURRENT slice's
> deviation set. Slice 1 = production trigger wiring + FrameLayout CPU-slot extension.
> Round 2 reworked Deviation #2's union logic (the union-poisoning fix).
> Round 3 makes `codegen_query` probe `spike_host_subset` against the SAME canonical EFFECTIVE
> suspend set `frame_layouts_query` uses (local ∪ imported-suspending), and builds the
> `suspends_with_promotions` set emit_artifact consumes from that effective set — the
> cross-boundary single-source-of-truth fix. Deviation #2's hunks change again; re-stated below.

## Scope Deviations (verbatim from executor report)

None — stayed within declared scope. (`emit.rs`, `queries.rs`, `crates/ynz-codegen/tests/`,
`crates/ynz-driver/tests/` are named in Phase 3's Files (expected scope); `queries.rs` in
ynz-codegen is explicitly authorized as in-scope codegen wiring per the task's note. Round 3
added one multi-file fixture dir
`crates/ynz-driver/tests/fixtures/v0_3_m3d_spike_s_imported_suspending_after_pair/`, one
integration test in `crates/ynz-driver/tests/integration.rs`, and one codegen-crate regression
test in `crates/ynz-codegen/tests/frame_layouts_query.rs` — all in declared scope. The
`crates/ynz-driver/tests/m2_state_machine_integration.rs` 1-line doc-link change cited as a
minor item is NOT in this changeset — it is already committed under the separate docs-relocation
commit 93506c0, not an uncommitted scope touch in Round 3.)

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Step 2, layout offsets): plan said `Replace the spike's hardcoded fixed offsets (SPIKE_HANDLE_0/1_OFFSET=32/40, SPIKE_RESULT_0/1_OFFSET=48/64, SPIKE_SLOT_RESERVE=6) with computed layout`, executor did `compute offsets/reserve in build_frame_layouts via build_cpu_group_slots + a shared cpu_group_slots_and_reserve helper; emission reads from FrameLayout::cpu_group_slots; the named SPIKE_* constants are RETAINED as a documented defensive fallback (used only if a layout entry is somehow absent — impossible for a promoted in-suspend-set function)`. Rationale: `the runtime drop-shim cleanup_spike_cpu_handles hardcodes handle offsets 32/40 as an ABI contract, so the layout MUST produce 32/40/48/64 for the current 2-member group; keeping the constants as a loud fallback means a future refactor that drops the layout entry fails predictably at the join rather than silently mis-offsetting, and removing them entirely would force the emission path to .expect() on the layout lookup — strictly worse failure ergonomics for zero benefit`. Diff hunks: crates/ynz-codegen/src/emit.rs:6590-6593, crates/ynz-codegen/src/emit.rs:7360-7385. (Unchanged through Round 3 — no Round-3 edit touched the layout-offset fallback or its emission path; Round-1 judge #1 PASS still applies.)
- **Deviation #2** (Step 1, suspend-set extension home — REWORKED again in Round 3): plan said `codegen_query ... unions PromotionOutput.promoted into the suspend set passed to emit_artifact`, executor did `reconcile the typeck promotion set down to the subset codegen can actually spike-HOST this slice (pub fn spike_host_subset) and union ONLY that host subset — probing spike_host_subset against the SAME canonical EFFECTIVE suspend set in BOTH query boundaries (local ∪ imported-suspending via build_effective_suspend_set), and building the suspends_with_promotions set emit_artifact consumes from that effective set. frame_layouts_query already used the effective set; Round 3 brought codegen_query into alignment (it previously probed against the BARE local set, the asymmetry the Round-2 gate flagged)`. Rationale: `the spike-host admission decision is made at TWO salsa query boundaries that BOTH size/lay-out the same heap frame. frame_layouts_query sizes the frame and codegen_query lays it out + emits the emit-time re-probe (lower_function_with_waits → spike_cpu_candidates reads suspends_with_promotions). If the two probe against DIFFERENT suspend sets, they can disagree on whether a function is a spike host: a host admitted by codegen but DECLINED by frame_layouts (because frame_layouts saw an imported suspending callee the bare set omits) would be laid out with the spike reserve but sized WITHOUT the imported child sub-frame — under-allocating the heap block and corrupting it when the imported child writes at its layout offset. One canonical effective set across both boundaries (and into emit_artifact's emit-time re-probe) keeps the two sizing decisions in lock-step — the single-source-of-truth invariant per no-duct-tape. Reachability note: for the entrypoint-only slice envelope, typeck's cpu_promotion_query guard-probe rollback ALREADY declines any entrypoint with a CPU group + a post-pair imported-suspending call (live-verified — promotion set empty for that shape), so the bare-vs-effective asymmetry is MASKED today, not live corruption; the fix removes the latent asymmetry so the masking cannot silently flip to corruption if a future slice relaxes the entrypoint-only gate or the guard-probe. The full-union approach (Round-1) separately poisoned nested host admission — the spike_host_subset reconciliation (Round-2) fixes that and is retained. Residual: a promoted inner host runs its own CPU group sequentially this slice (slice-2 carry-forward)`. Diff hunks: crates/ynz-codegen/src/queries.rs:108-119, crates/ynz-codegen/src/queries.rs:325-339, crates/ynz-codegen/src/emit.rs:6785-6799.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: (verbatim above — layout offsets computed in build_frame_layouts via shared helper; SPIKE_* constants retained as documented defensive fallback because the runtime drop-shim hardcodes 32/40 as an ABI contract; optional const-assert binding skipped as a cross-crate reach)
- **diff hunks**: crates/ynz-codegen/src/emit.rs:6590-6593, crates/ynz-codegen/src/emit.rs:7360-7385
- **carry status**: unchanged-round-3 (no Round-3 edit touched this mechanism; Round-1 judge #1 PASS still applies)

### Deviation #2
- **type**: approach
- **rationale**: (verbatim above — spike_host_subset probed against the SAME canonical EFFECTIVE suspend set in BOTH query boundaries; suspends_with_promotions built from the effective set; codegen_query brought into alignment with frame_layouts_query in Round 3 — the cross-boundary single-source-of-truth fix. Reachability of the original corruption is masked by typeck's guard-probe today (live-verified); the fix removes the latent asymmetry. spike_host_subset nested-host reconciliation from Round 2 retained.)
- **diff hunks**: crates/ynz-codegen/src/queries.rs:108-119, crates/ynz-codegen/src/queries.rs:325-339, crates/ynz-codegen/src/emit.rs:6785-6799
- **carry status**: reworked-round-3 (codegen_query probe + suspends_with_promotions now use the effective set — materially changed from Round 2's bare-set probe; re-judge required; prior identity hash invalidated)
