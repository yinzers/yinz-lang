# v0-3-m2-wait-and-state-machines Phase 8 Deviations — captured 2026-06-01

D_count: 1

## Scope Deviations (verbatim from executor report)

Scope Devs #1-#5 (all necessary follow-ons of the rename, NOT creep): `m2_runtime.rs` + `m2_spike.rs` (import/call old symbol name → must update to compile); 13 golden IR snapshots (literal `declare i32 @ynz_rt_call_state_machine_sync` in IR text → symbol-name substitution only, no semantic change, avoids `.snap.new`); 2 `.ll` fixtures (hand-written IR referencing the old symbol — NOTE: `.ll` is gitignored so these edits are untracked/moot); `.claude/state.md` (Phase 8 Step 2 explicitly directs recording the guard re-decision there). Coordinator: all necessary for the rename to compile + the guard-decision deliverable. None are scope creep.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Step 1: bridge deletion → rename): plan said "delete `ynz_rt_call_state_machine_sync`" (Option A preferred); executor chose Option B (rename → `ynz_rt_run_entrypoint`). Rationale: "LLVM IR can only call C-ABI symbols by name; the codegen-emitted main wrapper is a C function that must call into Rust via a named extern symbol. There is no mechanism to 'delete' a C-ABI symbol and replace it with inlined Rust — the boundary is structural. Option A and Option B converge to the same outcome: the old 'bridge' symbol is gone, replaced by a correctly-named program-entry driver. The plan explicitly authorized Option B with honest rename+rationale when full deletion is genuinely infeasible." Diff hunks: crates/ynz-runtime/src/runtime.rs:657-742, crates/ynz-codegen/src/runtime_decls.rs:194-199, crates/ynz-codegen/src/emit.rs:1747-1760.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: approach
- **rationale**: bridge "delete" (Option A) → "rename to ynz_rt_run_entrypoint" (Option B) because the codegen-emitted C-ABI main wrapper must call a named extern; full deletion is architecturally impossible. Plan explicitly authorized Option B with honest rename + tradeoff. Outcome identical: zero "bridge"/"Shape B" framing; the symbol is the legitimate program-entry driver, never reachable from a resume fn (objdump-proven, no_bridge test passes).
- **diff hunks**: crates/ynz-runtime/src/runtime.rs:657-742, crates/ynz-codegen/src/runtime_decls.rs:194-199, crates/ynz-codegen/src/emit.rs:1747-1760
