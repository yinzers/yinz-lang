# S2 verdict — per-field-in-loop-body access analysis spike: **GREEN**

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 2
- **Fixtures (this dir, 4 — Phase 4 step 1 consumes them):**
  `s2_fix1_qualifying_2field.ynz` · `s2_fix2_threefield_loop.ynz` · `s2_fix3_escaping.ynz` ·
  `s2_fix4_runtime_length.ynz`. Each fixture header carries its PRE-REGISTERED expected analysis
  (written before the spike ran) plus a hand-derivable expected stdout — the answer key existed
  before the results.
- **Proof protocol (real compiler, real pipeline — never a hand-written model, per the M2-HALT
  lesson):** throwaway pass `crates/ynz-typeck/src/spike_soa.rs` hooked env-gated
  (`YNZ_SPIKE_SOA=1`) into `check_query` immediately after `check(...)`, operating on the REAL
  `TypedModule` (`expr_types` keyed by `(span.start, span.end)` as the type oracle). Fixtures run
  via `./target/debug/ynz run <fixture>` in the worktree Docker `dev` container; all four exit 0
  and print their hand-derived stdout goldens (18/36 · 12/12/18 · 12/12 · 6).

## Result — all four match the pre-registered key EXACTLY

| Fixture | Expected verdict | Observed | Key evidence |
|---|---|---|---|
| fix1 qualifying 2-field | CANDIDATE | CANDIDATE | `provable_len=Some(4)`, `x` reads=1, `y` reads=1, union=2 |
| fix2 3-field loop | DECLINED(field-union 3 > 2) | identical | `a` **reads=2** (proves counting, not set-membership), `b`=1, `c`=1 |
| fix3 escaping | DECLINED(escapes: passed to `sumX()`) | identical | escape detected with the qualifying hot loop still present — escape is the SOLE decline reason |
| fix4 runtime-length | DECLINED(runtime-length: grown via `.add`) | identical | `growth_ops=1` beats the empty-literal `Some(0)` length |

Gate-off differential: the same binary with `YNZ_SPIKE_SOA` unset prints ONLY program stdout
(no analysis lines) — the spike is fully env-gated, zero default-path impact.

## Feasibility findings the Phase 4 `soa_candidate_query` builds on

1. **`TypedModule.expr_types` is a sufficient type oracle** for binding classification: the
   `Stmt::Let` initializer's `(span.start, span.end)` key resolves to
   `Type::BuiltinArray { elem: Type::Shape { .. } }` directly — no re-inference, no second
   derivation (the annotation AST type was never needed).
2. **Loop-element per-field counting is a simple context-stack walk:** `for (v in arr)` pushes
   `(v → arr)`; `FieldAccess { receiver: Ident(v), field }` inside the loop body attributes to
   `(arr, field)`. Nested-loop scoping falls out of push/pop. The walk is the same grain as
   `effective_ownership.rs`'s body walk — nothing novel-hard remains (risk E5 retired at spike level).
3. **Index-form reads are NOT the hot-loop surface:** `arr[i]` yields `maybe<Shape>` (typeck
   desugars `[]` to `.get()`), so direct `arr[i].field` is not idiomatic-typable Yinz — the SoA hot
   loop model is the `for (elem in arr)` form. The spike still handles the
   `FieldAccess{IndexAccess{Ident}}` pattern defensively, but Phase 4's admission model should key
   on for-in loops (matches the scratch doc's transform model).
4. **Escape/growth/runtime-length disqualifiers are all detectable at the same walk:** call-arg
   idents (incl. UFCS method-form calls — `arr.foo()` IS `foo(arr)`), `return arr`, `.add()`
   growth. Whole-value element uses (`process(p)`) are counted separately
   (`whole_value_uses`) — the cold-field fan-out input for Phase 5.
5. **Param arrays report nothing by construction** (only `Stmt::Let` bindings are tracked): a
   param array is cross-function by definition. Phase 4 must DECIDE this posture explicitly
   (decline-at-admission) rather than inherit it silently — flagged as a Phase 4 design point.
6. **Keyword trap for fixture authors:** `base` is a reserved word (`base shape`); `let base` is a
   parse error whose recovery cascade misleadingly points at shape declarations. Fixtures use `seed`.
7. **Static occurrence counts, not dynamic:** reads=2 for a field accessed twice per iteration —
   the count is per-source-occurrence. Phase 6's threshold calibration must define its "hot"
   criterion accordingly (occurrence count × provable trip count, not profiled frequency).

## Verdict criteria used (spike-grade mirror of Phase 4 admission, D3–D5)

escape → DECLINED(escapes) ▸ growth → DECLINED(runtime-length: grown) ▸ non-literal init →
DECLINED(runtime-length) ▸ empty union → DECLINED(no per-field loop access) ▸ union > 2 →
DECLINED(field-union > 2) ▸ else CANDIDATE. (SIZE_THRESHOLD deliberately absent — Phase 6 owns it.)
(The shape-level lend-self filter is likewise deliberately absent — spike scope: it mirrors
already-proven M4 machinery (`finalize_false_sharing`'s shape-decl scan, false_sharing.rs:131-134),
not the novel per-field-counting core E5 gates on — same rationale as the SIZE_THRESHOLD omission.)

## Teardown

Spike scaffolding (module `crates/ynz-typeck/src/spike_soa.rs`, `pub mod` line in
`crates/ynz-typeck/src/lib.rs`, env-gated hook in `crates/ynz-typeck/src/queries.rs::check_query`)
reverted after this verdict was recorded; tree rebuilt green and fix1 re-run on the pristine
compiler (program output only, exit 0). Only this note and the four fixtures persist.
