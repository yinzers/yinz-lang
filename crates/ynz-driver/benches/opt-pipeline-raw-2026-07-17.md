# v0.3-M7 Phase 7 — O0-vs-optimized A/B raw numbers (Step 2 record)

Provenance record for `opt_pipeline_calibration.rs` (same convention as
`soa-threshold-raw-2026-07-04.md`). Every number below traces to a committed
benchmark run of the committed harness — nothing hand-waved.

## Run provenance

- **Date:** 2026-07-17
- **Tree state:** `108e32024d22885975c86d43f5eff8675f116a2d` (Phase 6 sealed) + the
  Phase 7 A/B harness itself (this file's own commit).
- **Host:** 12th Gen Intel Core i7-12700K, 18 threads visible, WSL2 Linux, inside
  the `ynz-dev` container (the canonical build environment).
- **Command:** `docker compose run --rm dev cargo bench -p ynz-driver --bench opt_pipeline_calibration`
- **Harness gates:** all three passed for every workload before benching
  (checksum tripwire, dual-mode byte-identical stdout oracle, IR-content gate —
  the default-tier `.ll` differs from the o0 `.ll`, proving the mid-end pipeline
  ran; `--emit-ir` prints post-pipeline IR and builds are Phase-5-deterministic,
  so byte-equality would prove a silent no-op).
- **Compiler binary:** bench (release) profile via `CARGO_BIN_EXE_ynz` — workload
  binaries therefore link the release-profile `libynz_rt` runtime, matching what
  a shipped `ynz build` produces. (A debug-profile compiler links a debug
  runtime and inflates all wall-clocks several-fold; those numbers are not
  comparable and are not recorded here.)
- **Tiers:** `YNZ_OPT_FORCE=o0` (byte-for-byte the `--no-optimize` escape hatch)
  vs `YNZ_OPT_FORCE=default` (the shipped `ynz build` default: `default<O2>`
  mid-end + Aggressive backend, per Phase 3).

## Visit-budget re-assessment (Step 1 obligation)

soa_calibration's 131,072-visit cap was NOT copied. Per its own Phase-4
re-evaluation record and the `hot_loop_stack_stress.rs` lock (67,108,864 visits
green at both tiers), the old O0 stack-growth crash envelope (ledger row 439) is
eliminated — the cap is a bench-runtime budget only. This harness raises the
per-workload budget to 8.4M–16.8M visits so per-run wall-clock (~16–90ms)
dominates the measured ~3.2ms spawn overhead; at 131,072 visits every run would
be spawn-dominated and the A/B ratio would be an artifact of exec cost.

## Raw medians (criterion, sample_size=10, flat sampling, 10s measurement)

| bench point | median | 95% CI |
|---|---|---|
| overhead/spawn_only | 3.234 ms | [3.091, 3.390] |
| cpu_loop / o0 | 56.566 ms | [55.277, 58.303] |
| cpu_loop / default | 34.235 ms | [33.787, 34.842] |
| shape_alloc / o0 | 40.971 ms | [40.233, 41.980] |
| shape_alloc / default | 15.784 ms | [15.702, 16.131] |
| soa_physics / o0 | 86.975 ms | [85.236, 93.098] |
| soa_physics / default | 59.449 ms | [58.706, 61.066] |

## Speedup: default (`ynz build`) over `--no-optimize`

Net = spawn overhead (3.234 ms median) subtracted from both sides — the
per-workload compute ratio, which is what the pipeline actually changes.

| workload | visits | gross | net |
|---|---|---|---|
| cpu_loop (scalar add+rem, 2^24 iters) | 16.8M | 1.65x | **1.72x** |
| shape_alloc (per-iter shape literal, 2^23 iters) | 8.4M | 2.60x | **3.01x** |
| soa_physics (M5 Player hot-x/y scan, N=64) | 16.8M | 1.46x | **1.49x** |

## Honest framing notes

- These are O0-vs-default numbers for the SAME Yinz binary — they prove the
  pipeline produces real wall-clock wins (the milestone's falsifiability goal),
  not any cross-language claim. The Rust-equivalent comparison is Phase 7
  Step 3's separate suite and its own record.
- The optimizer's headroom is bounded by opaque runtime calls (`ynz_array_get`,
  print, overflow-checked arithmetic panics reference runtime globals) that
  LLVM cannot inline or fold across — visible in soa_physics's smaller win
  (array traffic per visit) vs shape_alloc's larger one (allocas + field reads
  the mid-end can promote).
- Single-host, single-run record; criterion CIs above bound the noise. The
  suite is committed and rerunnable — challenge by rerun, not by citation.
