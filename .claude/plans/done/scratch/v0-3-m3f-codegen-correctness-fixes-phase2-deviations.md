# v0-3-m3f-codegen-correctness-fixes Phase 2 Deviations — captured 2026-06-09 (round 2: after fix)

D_count: 3 documented (1 scope + 2 approach); judges spawned: 1 (only Approach #1 has an adversarial behavioral surface).

## Scope Deviations (verbatim from executor report)

**Scope Deviation #1** (`crates/ynz-driver/tests/integration.rs`): touched outside declared scope. Rationale: `cargo fmt --all` auto-formatted the Phase-1 assertion blocks (pure whitespace normalization, no semantic change). Diff hunks: `integration.rs:6510-6575`.

> COORDINATOR: NO judge — a `cargo fmt` whitespace change has no behavioral/adversarial surface (plan-adherence round-1 confirmed values unchanged). Documented now (was falsely "None" in the first executor run).

## Approach Deviations (verbatim from executor report)

**Deviation #1** (Step 3 — "leave the crossing path untouched"): executor reworked the crossing EC<Number> path to 3-slot storage (`{f0, i128_lo, i128_hi}` instead of `{f0, staging_ptr}`) AND kept it after the fix round. Rationale: p1 is a crossing local (live across p2's suspension); baseline stored the staging POINTER as f1, which same-callee reuse clobbers; 3-slot stores lo/hi directly → value-stable. Empirically: reverting the rework keeps `v03_m3b_p5_parallel_number_crosses_wait` GREEN (rework NOT needed for that test — concedes the original necessity claim), but executor claims it IS needed for AC1 (p1 crossing). FIX A (f0==0 guard) fixed the error-path regression separately. Diff hunks: `emit.rs:1474-1480, 2112-2130, 2197-2200, 2286-2421, 3214-3280, 3648-3710, 5443-5607, 5862-5880`.

**Deviation #2** (Step 2 — stack-alloca): executor used a per-binding i128 stack alloca placed at resume-fn entry. Rationale: "matches the plan's stated preference — stated for explicitness."

> COORDINATOR: NO judge — this is plan COMPLIANCE (the plan said "stack-alloca where lifetime permits"; executor did exactly that). No adversarial surface; nothing to adjudicate.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 (judge spawned)
- **type**: approach
- **rationale**: (above) crossing-path 3-slot rework kept; executor claims necessity for AC1 (p1 crossing), concedes it's not needed for v03_m3b_p5.
- **diff hunks**: emit.rs:1474-1480, 2112-2130, 2197-2200, 2286-2421, 3214-3280, 3648-3710, 5443-5607, 5862-5880
- **judge identity hash**: 4c031892e9193ec1433adc928da02993d2dc2824

> COORDINATOR GROUND-TRUTH for the judge: code-reviewer's round-1 #4 called this rework OVERSHOOT (staging slot is frame-stable in the composed heap frame; `ec_crossing_local_propagated_number` passed at baseline; recommended revert). The executor now rebuts: p1 is a SAME-CALLEE crossing local and the baseline path stored a staging POINTER that p2's reuse clobbers — so single-binding-crossing being correct at baseline doesn't prove same-callee-crossing is. The judge must settle OVERSHOOT vs NECESSARY: does reverting the 3-slot rework (keeping only non-crossing copy-on-bind) make AC1's p1 read 31.75 (aliased → rework necessary) or 24.50 (frame-stable → rework overshoot)? That single fact decides it.
