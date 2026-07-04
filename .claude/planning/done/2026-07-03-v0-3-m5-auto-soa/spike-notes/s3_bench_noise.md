# S3 — bench noise probe: run-to-run variance record (E2 credibility bar for Phase 6)

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 3
- **Environment:** Docker `dev` container (compose project `ynz-m5-worktree`) on WSL2 —
  uncontrolled shared hardware, no CPU pinning, no isolation (the exact environment Phase 6's
  SIZE_THRESHOLD calibration will run in).
- **Bench:** `s3_bench.rs` (this dir — kept for Phase 6 reuse). `rustc -O`. N=1,000,000 elements;
  AoS = 64-byte element (1 hot i64 + 7 cold i64), scan-sum the hot field; SoA = contiguous
  `Vec<i64>` of the hot field. 12 in-process reps × 3 separate process runs, warmup pass first,
  `black_box` on every read and accumulator.

## Raw results (µs)

| Process run | AoS mean | AoS cv | AoS spread (min→max) | SoA mean | SoA cv | SoA spread |
|---|---|---|---|---|---|---|
| 1 | 4605 | 4.9% | 20.0% | 686 | 7.3% | 25.6% |
| 2 | 4140 | 4.3% | 14.2% | 695 | 8.1% | 31.9% |
| 3 | 4054 | 6.4% | 21.3% | 695 | 12.4% | 49.8% |

Full per-rep lists are in the bench stdout format (`*_all_us`) — regenerate any time:
`rustc -O -o /tmp/s3_bench spike-notes/s3_bench.rs && /tmp/s3_bench` (in the dev container, from `/work`).

## The noise floor (the honest numbers Phase 6 must respect)

- **Within-process rep noise:** cv 4.3–12.4%; worst observed min→max spread **49.8%** (SoA, run 3).
- **Cross-process mean drift:** AoS 4054→4605 µs = **13.6%**; SoA 686→695 µs = 1.3%.
- **⇒ Credibility bar: a measured effect below ~15% (mean-vs-mean) — and any single-rep
  difference below ~50% — is NOT evidence in this environment.** Phase 6 must (a) use the
  median or min-of-K of ≥12 reps, never single shots; (b) treat any threshold-crossover delta
  smaller than the noise floor as "no detectable difference"; (c) re-record this table on the
  day of calibration (the floor moves with host load).

## The signal (why calibration is still feasible here)

AoS-vs-SoA on the hot-field scan is **~6× (≈4.2 ms vs ≈0.69 ms)** — vastly above the noise
floor (theoretical bandwidth ratio for 1-of-8 hot fields is 8×; ~6× observed is consistent with
loop overhead). The effect Phase 6 is calibrating is order-of-magnitude-scale at large N, so the
crossover POINT (SIZE_THRESHOLD) is findable — but its precision is bounded by the ~15% floor:
**the shipped constant must be documented as an order-of-magnitude calibration, not a precise
optimum** (E2's honesty posture; the provenance/variance record ships WITH the constant, Phase 6
step 3).
