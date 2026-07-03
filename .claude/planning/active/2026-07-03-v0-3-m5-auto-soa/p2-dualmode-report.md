# P2 dual-mode oracle report — post-fix-round-3 run (durable evidence)

- **Session:** phase2-fixround3-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan
  `2026-07-03-v0-3-m5-auto-soa`, Phase 2 fix round 3 (segment 2)
- **Tree:** worktree `feat/v0-3-m5-auto-soa` (fork @ `1ac52fd`) with the full uncommitted P2
  by-value cut, the round-1 get-side ownership fix (`store_binding` / `shape_bytes_to_owned` /
  `maybe_to_owned` / `shape_bytes_into_embed_slot`), PLUS the round-2 persist-boundary fix
  (`store_field` Shape/Maybe heap-cell arms + the `map_value_to_stable_bits` choke point at all
  four map insert sites, `crates/ynz-codegen/src/emit.rs`), the 8 round-2 escape tripwire
  fixtures, and the `ynz-fmt` nested-generic space fix (`crates/ynz-fmt/src/walker.rs
  close_generic` — round-2's `map<string, maybe<Part> >` fixture exposed the formatter emitting
  un-reparseable `>>`), PLUS the round-3 fixes: `value_to_stable_bits` (the generalized
  marshalling choke point — rename of `map_value_to_stable_bits`, now also routing
  `array_elem_src_ptr`'s non-shape element writes) and the bg-spawn maybe-arg heap upgrade
  (`prepare_bg_arg_for_ctx` Maybe arm + `BgArgFreeKind::HeapMaybeEnv`), with the 2 round-3
  tripwire fixtures.
- **Why this run exists:** FRAGO 009-round evidence-durability item — sweep tallies must live in
  a durable file, not a chat return; this record is regenerated after each fix round. This run is
  post-round-3 (array-maybe-element + spawn-maybe-arg persist fixes landed).

## Methodology

For every fixture in `crates/ynz-driver/tests/fixtures/*.ynz` (447 at run time), in the worktree
Docker `dev` container, debug-profile compiler:

1. copy to an isolated tmpdir; `ynz build <prog>.ynz` (default mode) → run → capture stdout;
2. `ynz build <prog>.ynz --no-auto-parallel` (sequential mode) → run → capture stdout;
3. byte-compare. On mismatch, re-run default mode once: if default differs run-to-run the
   fixture is classified NONDET (timing-dependent), else DIFF.
4. Build/compile failures (rejection fixtures — compile errors by design) recorded as SKIP.

All stages under `timeout 60`. Sweep script: `p2_dualmode_sweep.sh` (scratch, deleted after the
run; this file preserves the methodology).

## Tallies

| bucket | count | of 447 |
|---|---|---|
| byte-identical across modes | **359** | |
| SKIP (compile-reject by design / build fail) | 84 | |
| DIFF — documented intended-divergence exclusions | 2 | see below |
| NONDET — timing fixtures, nondet in default mode alone | 2 | see below |
| sequential-build-fails-while-default-succeeds anomalies | 0 | |
| **real divergences** | **0** | |

**The 2 DIFFs (both array-free — verified `grep -c array` = 0; both divergent BY DESIGN):**
- `v0_3_m3b_p4_model_a_intended_reorder.ynz` — the documented Model-A intended-reorder exclusion
  class (same fixture flagged in the segment-2 run).
- `v0_3_m3g_overlap_proof.ynz` — the ordering-overlap proof: its own header specifies that the
  dual-mode ordering DIFFERENCE is the property under test (fused mode must interleave START
  markers; sequential mode must not).

**The 2 NONDETs (default mode differs run-to-run — timing, not layout/ABI):**
- `v0_3_m2_concurrent_waits_proof.ynz`
- `v0_3_m3g_e8_pool_exhaustion_stress.ynz`

## Comparison with prior runs

- Segment-2 (pre-fix-loop): 340 identical / 0 real divergences / 2 flagged (intended-reorder
  exclusion class, array-free) / 3 nondet, on 431 fixtures.
- Post-fix-loop (round 1): 349 identical / 84 skip / 2 DIFF / 2 NONDET, on 437 fixtures.
- Post-fix-round-2: 357 identical / 84 skip / 2 DIFF / 2 NONDET / 0 anomalies, on 445 fixtures
  (+8 round-2 persist-boundary tripwires, all byte-identical; same 2 DIFFs, same 2 NONDETs).
- **This run (post-fix-round-3): 359 identical / 84 skip / 2 DIFF / 2 NONDET / 0 anomalies, on
  447 fixtures** — 445 → 447 (+2 round-3 tripwires,
  `m5_p2_byval_array_maybe_elem_write_escape` + `m5_p2_byval_bg_maybe_arg_escape`, both
  byte-identical across modes), 357 → 359 identical (exactly the +2 new), the SAME two
  documented DIFFs (`v0_3_m3b_p4_model_a_intended_reorder`, `v0_3_m3g_overlap_proof`) and the
  SAME two timing NONDETs (`v0_3_m2_concurrent_waits_proof`,
  `v0_3_m3g_e8_pool_exhaustion_stress`), same 0 real divergences. **The round-3
  array-maybe-element + spawn-maybe-arg persist fixes introduced no dual-mode divergence.**

## Honesty notes

- The sweep runs each mode once (plus one nondet-classification re-run) — it is a divergence
  DETECTOR, not a statistical proof; the timing-class fixtures' bucket wobble is expected and
  documented rather than suppressed.
- SKIP=84 counts fixtures whose build fails in BOTH modes (rejection galleries etc.); the
  integration suite, not this sweep, asserts their diagnostics.
