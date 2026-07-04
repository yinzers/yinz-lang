# SOA_SIZE_THRESHOLD calibration — raw numbers (2026-07-04)

Raw evidence AND the SOA_SIZE_THRESHOLD provenance record for Phase 6 step 3 of plan
`2026-07-03-v0-3-m5-auto-soa` (the "Step 3 calibration verdict" section at the bottom is the
provenance record, per that plan's FRAGO 017 — no separate provenance file exists). Environment
for everything below: Docker `dev` container (compose project `ynz-m5-worktree`) on WSL2 —
uncontrolled shared hardware, no CPU pinning (the E2 honesty posture).

## Day-of noise re-record (S3 protocol; `s3_bench.rs`, rustc -O, 3 process runs)

Same bench as the Phase 0 record (plan dir `spike-notes/s3_bench_noise.md`): N=1,000,000,
64-byte element, 1-of-8 hot-field scan-sum, 12 in-process reps per run, warmup + black_box.

| Process run | AoS mean µs | AoS cv | AoS spread | SoA mean µs | SoA cv | SoA spread |
|---|---|---|---|---|---|---|
| 1 | 3491.9 | 20.4% | 90.2% | 902.6 | 94.4% | 560.4% |
| 2 | 3527.8 | 12.0% | 51.0% | 633.1 | 4.5% | 18.6% |
| 3 | 3314.2 | 7.0% | 27.3% | 601.4 | 15.9% | 67.3% |

- Run 1 caught a load spike (one 5.8 ms AoS rep, one 3.7 ms SoA rep) — exactly why the
  protocol demands medians / min-of-K, never means of single runs.
- Cross-process mean drift: AoS 3314→3528 = 6.4%; SoA 601→903 = 50% (spike-driven; medians
  are stable: SoA per-rep medians ≈ 610-670 µs across runs).
- **The Phase 0 credibility bar stands: treat any mean-vs-mean effect below ~15% as "no
  detectable difference" in this environment.** Hot-field scan signal today: ~5.5×
  (3.3-3.5 ms vs 0.60-0.63 ms) — consistent with the Phase 0 record (~6×), far above floor.

## E13 crash-envelope bracket (2026-07-04; segment-3 corrected record)

**Segment-3 correction (supersedes segment 2's interpretation of the n8 SIGABRT).** The
n8 × r32768 SIGABRT segment 2 attributed to E13 stack growth was actually an UNRELATED
toolchain defect: `cargo bench` builds `ynz` in the release profile, and the driver embeds
`target/release/libynz_runtime.a` via `include_bytes!` (`crates/ynz-driver/build.rs`).
That archive was stale (pre-M5, Jul 3 04:08) — it lacked `ynz_array_new_sized` (forced-SoA
builds failed to link) while its pre-M5 old-ABI `ynz_array_*` symbols still resolved by
name against M5 codegen's calls, producing garbage field reads from the FIRST iteration
(n8 × r1 printed 1125818137567240 instead of 108; garbage `acc` grows ~1.13e15/rep and
trips the int-overflow panic, exit 134, at reps ≥ ~12288). Fixed by rebuilding
`ynz-runtime --release` before benching. Evidence receipts: `tmp-p6-probe/seg3-bracket/`
(untracked). Every crash row below is labeled with the toolchain that produced it.

E13 bracket with a HEALTHY (current-runtime) toolchain:

| Workload | Visits | For-in entries | Result |
|---|---|---|---|
| n512 × r1000 (segment-1 probe) | 512,000 | 1,000 | OK, exact checksum |
| n8 × r2048 … r16384 (segment 2) | 16,384-131,072 | 2,048-16,384 | OK, exact checksums |
| n8 × r32768 (segment 3, debug-built) | 262,144 | 32,768 | OK (3538944, exact) |
| n8 × r65536 (segment 3, debug-built) | 524,288 | 65,536 | **SIGSEGV** (exit 139) |
| n4096 × r1024 / n1024 × r4096 / n512 × r8192 (segment-1) | ~4.19M | 1,024-8,192 | SIGSEGV |

Rows recorded by segment 2 under the STALE release toolchain (kept for the audit trail;
they characterize the stale-runtime defect, NOT E13): n8 × r32768 SIGABRT at 262,144
visits (int-overflow panic from garbage reads — falsified as an E13 point by the healthy
n8 × r32768 pass above).

The envelope remains 2-dimensional even after the correction — n8 × r65536 crashes at
~524K visits while n512 × r1000 passes at ~512K visits — but the N=8 entries-axis
boundary sits between 32,768 (OK) and 65,536 (crash), not at segment 2's recorded
16,384/32,768 line. Proven-safe joint region used by the harness (unchanged, now with
larger margin): **visits ≤ 131,072 AND entries ≤ 16,384**.

## Full calibration run (segment 3, 2026-07-04 — healthy toolchain, post runtime-staleness fix)

`cargo bench -p ynz-driver --bench soa_calibration`, criterion 0.5, 12 samples/point, Flat
sampling, 1s warmup / 4s measurement, TOTAL_VISITS = 131,072 (R = 131072/N). All gates green
at every point: exact checksums, byte-identical dual-mode stdout, IR gate (soa symbols present
in forced-soa .ll, absent in forced-aos .ll). Workload: 64-byte 8-int-field Player, 2-of-8 hot
fields (x, y) read-accumulate — 4x theoretical SoA bandwidth edge. Shipped-binary codegen:
OptimizationLevel::None, no LLVM pass pipeline (what `ynz build` really emits).

Criterion medians; "net" = median minus the 1.6758 ms spawn_only median:

| N | AoS ms | SoA ms | AoS net ms | SoA net ms | SoA/AoS (net) |
|---|---|---|---|---|---|
| 8 | 3.0481 | 3.2672 | 1.3724 | 1.5914 | 1.160 |
| 16 | 3.1406 | 3.1362 | 1.4649 | 1.4604 | 0.997 |
| 32 | 3.0161 | 3.0881 | 1.3403 | 1.4123 | 1.054 |
| 64 | 2.9495 | 3.0641 | 1.2737 | 1.3883 | 1.090 |
| 128 | 2.9476 | 3.1055 | 1.2718 | 1.4297 | 1.124 |
| 256 | 2.9824 | 3.0541 | 1.3067 | 1.3783 | 1.055 |
| 512 | 2.9854 | 3.2236 | 1.3096 | 1.5478 | 1.182 |
| 1024 | 3.1138 | 3.1799 | 1.4380 | 1.5042 | 1.046 |
| 2048 | 3.1857 | 3.2800 | 1.5099 | 1.6042 | 1.062 |
| 4096 | 3.2807 | 3.3486 | 1.6049 | 1.6728 | 1.042 |

**There is NO crossover at any N in {8..4096}.** SoA is uniformly the same-or-slower in
shipped (O0) binaries: every point's net ratio is 1.00-1.18. Each individual point sits below
the ~15% noise floor, but the direction is uniform across 10 independent points — the honest
reading is "no detectable SoA benefit; slight consistent overhead."

## opt-18 -O2 diagnostic (evidence-only — the pre-registered p5-ir-evidence Claim 2 check)

Same .ll files the shipped compiler emits (N=4096, R=1024 = 4.19M visits; O0 binaries crash
here per E13 — only the O2 binaries were run), rebuilt via `opt-18 -O2` + `clang-18 -O2`,
linked against the same release runtime. Checksums exact in both modes (25776095232).
15 wall-clock runs each:

- AoS: median ≈ 12.45 ms wall (≈ 10.77 ms net of spawn)
- SoA: median ≈ 4.97 ms wall (≈ 3.29 ms net of spawn)
- **SoA/AoS under -O2 ≈ 0.31 — a ~3.3x SoA win**, consistent with the 4x theoretical
  bandwidth edge for 2-of-8 hot fields.

Conclusion: the SoA win is REAL but exists only under an optimization pipeline shipped
`ynz build` binaries never run. At the O0 codegen Yinz actually ships, SoA's full-gather
element access touches all fields anyway and the layout adds address arithmetic — the
measured shipped-binary improvement is ~0 (slightly negative). This is Phase 6 step 5's
order-of-magnitude STOP condition (measured ~1.0x vs the 10-40x claim), pre-registered by
segment 1 and now confirmed by measurement. SIZE_THRESHOLD calibration (step 3) is
undefined on this data — there is no crossover to calibrate against — so steps 3-5 halt
pending the conductor-routed decision on the milestone's performance claim.

## Step 3 calibration verdict (2026-07-04, segment 4 — the provenance record)

**SOA_SIZE_THRESHOLD = 64 ships UNCHANGED, as a documented conservative default — NOT a
crossover-calibrated constant.** Per plan FRAGO 017: crossover-based calibration is undefined on
the measured data, because **no SoA/AoS crossover exists at any N in {8..4096} in shipped O0
binaries** (see the full calibration run above — every net ratio is 1.00-1.18, SoA never faster).
There is no crossover N to compare 64 against, so the honest verdict is "keep the pre-M5 value
and say exactly why," not a derived number wearing unearned precision (risk E2's exact hazard).

The two measured results, stated separately and never conflated:

- **Shipped O0 binaries (what `ynz build` emits today — OptimizationLevel::None, zero LLVM pass
  pipeline):** measured net effect ≈ **1.0x** — no detectable SoA benefit; 9 of 10 points trend
  4-18% slower (N=16 sits at parity, ratio 0.997), each individually below the ~15% noise floor,
  SoA never faster at any point.
- **Identical generated IR under `opt-18 -O2` (diagnostic, N=4096):** SoA wins ≈ **3.3x** net of
  spawn (10.77 ms AoS vs 3.29 ms SoA; ratio 3.29/10.77 ≈ 0.31), consistent with the 4x
  theoretical bandwidth edge for 2-of-8 hot fields. The win is real and IR-confirmed but lives
  entirely in an optimization pipeline shipped binaries never run (plan risk E14).

Why keep 64 at all (rather than, say, disabling admission at O0): the SoA representation's
correctness is unconditional (byte-identical dual-mode output at every measured point, all
checksum/IR gates green), its measured O0 cost is below the noise floor, and the constant's
purpose is to bound admission for the day an LLVM pass pipeline makes the win real — a
conservative threshold with honest provenance beats churning the admission surface on data that
shows no crossover in either direction.

Provenance fields (E2 posture):
- **Workload:** 64-byte 8-int-field `Player`, 2-of-8 hot fields (x, y) read-accumulate hot loop
  (the roadmap physics-loop shape, S2-qualifying); N ∈ {8..4096}, TOTAL_VISITS = 131,072.
- **Machine:** Docker `dev` container on WSL2 — uncontrolled shared hardware, no CPU pinning.
  Numbers are medians (criterion, 12 samples/point); treat mean-vs-mean effects below ~15% as
  "no detectable difference" in this environment (S3 protocol, re-recorded day-of above).
- **Date:** 2026-07-04 (toolchain-health-gated run, post stale-runtime fix — see the E13 section).
- **Variance:** S3 noise re-record above (~15% floor; spike-prone shared host).
- **Revisit triggers:** (1) a dedicated perf environment becomes available OR a user-reported perf
  regression implicates the threshold (plan Future Requirements #2) → re-run this harness and
  re-derive; (2) **a future LLVM-pass-pipeline milestone ships** (plan risk E14) → the crossover
  question becomes well-defined for shipped binaries; re-calibrate against optimized codegen.
