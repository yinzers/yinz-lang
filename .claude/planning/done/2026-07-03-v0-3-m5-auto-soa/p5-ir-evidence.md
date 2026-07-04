# Phase 5 CHECKPOINT evidence — SoA hot-loop contiguity (release-mode IR)

Recorded by session `phase5-executor-2026-07-03-m5-seg3` (2026-07-04). Cited later by
Phase 6 (bench) and Phase 8 (completion gate). Tree state: branch `feat/v0-3-m5-auto-soa`,
commit `736a143` + Phase 5 working files (see plan.md Phase 5 STATUS).

## How it was produced (rerunnable)

```bash
docker compose run --rm dev bash -c "
  cp crates/ynz-driver/tests/fixtures/m5_p4_soa_qualifying.ynz target/p5-evidence/q.ynz
  ./target/debug/ynz build target/p5-evidence/q.ynz --emit-ir
  opt-18 -O2 -S target/p5-evidence/q.ll -o target/p5-evidence/q_opt.ll"
```

`opt-18 -O2` stands in for the release pipeline's LLVM optimization of the emitted
module (same LLVM 18 the driver links).

## Claim 1 — SoA lowering FIRED (not silently declined)

- Unoptimized `q.ll`: 146 `soa_new`-related lines; 438 `soa_new|_soa_*_addr` instruction
  names total for the qualifying fixture (`Point{x,y}`, 72 elements).
- Construction scatter uses COMPILE-TIME segment offsets (D2): `x` stores at
  `data + 8k`, `y` stores at `data + 576 + 8k` (cap 72 × 8-byte field ⇒ y-segment
  base = 576). Excerpt (unoptimized, elements 0–4):

```llvm
%soa_ctor_soa_y_addr  = getelementptr i8, ptr %soa_ctor_soa_data,  i64 576
%soa_ctor_soa_x_addr10 = getelementptr i8, ptr %soa_ctor_soa_data4, i64 8
%soa_ctor_soa_y_addr13 = getelementptr i8, ptr %soa_ctor_soa_data4, i64 584
%soa_ctor_soa_x_addr23 = getelementptr i8, ptr %soa_ctor_soa_data17, i64 16
%soa_ctor_soa_y_addr26 = getelementptr i8, ptr %soa_ctor_soa_data17, i64 592
```

## Claim 2 — hot-loop field loads are CONTIGUOUS (the vectorizer-friendliness criterion)

Optimized (`opt-18 -O2`) hot loop, in full. SROA has eliminated the gather
out-buffer entirely (`for_get_out.sroa.*` phis) — the cold-field-elision design
(design c: gather full element, let DSE/SROA drop unused fields) leaves EXACTLY
the two used fields' segment loads, each with an 8-byte stride over a contiguous
segment:

```llvm
for_body:
  %for_i.010 = phi i64 [ %next_i, %ov_ok938 ], [ 0, %entry ]
  %sx.09 = phi i64 [ %sum, %ov_ok938 ], [ 0, %entry ]
  %sy.08 = phi i64 [ %sum936, %ov_ok938 ], [ 0, %entry ]
  %for_get_call_soa_ib = icmp ult i64 %for_i.010, 72
  br i1 %for_get_call_soa_ib, label %for_get_call_soa_hit, label %for_get_call_soa_cont

for_get_call_soa_hit:
  %for_get_call_soa_data = load ptr, ptr %soa_new, align 8
  %for_get_call_soa_scale = shl nuw nsw i64 %for_i.010, 3
  %for_get_call_soa_x_addr = getelementptr i8, ptr %for_get_call_soa_data, i64 %for_get_call_soa_scale
  %for_get_call_soa_f0 = load i64, ptr %for_get_call_soa_x_addr, align 8
  %for_get_call_soa_y_addr = getelementptr i8, ptr %for_get_call_soa_x_addr, i64 576
  %for_get_call_soa_f1 = load i64, ptr %for_get_call_soa_y_addr, align 8
  br label %for_get_call_soa_cont

for_get_call_soa_cont:
  %for_get_out.sroa.0.1 = phi i64 [ %for_get_call_soa_f0, %for_get_call_soa_hit ], [ 0, %for_body ]
  %for_get_out.sroa.3.1 = phi i64 [ %for_get_call_soa_f1, %for_get_call_soa_hit ], [ 0, %for_body ]
  %ov_res = tail call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %sx.09, i64 %for_get_out.sroa.0.1)
```

- `x` loads walk `data + 8·i` (stride 8, one contiguous segment).
- `y` loads walk the same induction at a constant `+576` segment offset (stride 8,
  contiguous).
- Zero `<N x i64>` vector ops in this build: vectorization proper is blocked by
  `emit_loop_preempt`'s per-iteration call + the int-overflow check branches —
  per the phase design, **contiguous loads is the criterion, not "vectorized"**
  (the loads are exactly the shape the vectorizer/prefetcher wants).

## Claim 3 — dual-mode behavior parity (E6 end-to-end)

- `./target/debug/ynz run q.ynz` (SoA) vs `YNZ_NO_AUTO_PARALLEL=1` (AoS): both
  exit 0, stdout **byte-identical** (`cmp` clean; first lines `2628` / `5256`).
- `cargo test -p ynz-driver --test cross_impl_consistency` → 2 passed
  (`corpus_byte_identical_across_auto_parallel_modes`,
  `corpus_produces_deterministic_output_across_runs`) with SoA codegen LIVE —
  the dual-mode oracle now genuinely exercises AoS-vs-SoA divergence detection.

## Grep gates (Phase 5 exit obligations, run this segment)

- **E7** — every raw `rt.ynz_array_{new,push,get,set}` ref in emit.rs sits inside
  the choke-point section (lines 2544/2596/2656/2686/2776 post-edit; 2544 is
  `soa_scatter`'s OOB abort-parity call, in-section by design). Zero refs outside.
- **E3** — `grep -rn "soa_candidate|hot_fields|whole_value_uses" crates/ynz-codegen/src/`
  → zero hits: layout answers come ONLY from `LayoutDecisions` (`cg.layout.arrays`).
