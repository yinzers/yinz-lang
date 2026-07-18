# v0.3-M7 Phase 7 — Rust-equivalent comparison raw numbers (Step 3 record)

Provenance record for the `rust_equiv` group of `opt_pipeline_calibration.rs`
(sibling of the Step 2 record, `opt-pipeline-raw-2026-07-17.md`). Every number
traces to a committed benchmark run of the committed harness — nothing
hand-waved. Challenge by rerun, not by citation.

## Run provenance

- **Date:** 2026-07-17
- **Tree state:** `108e32024d22885975c86d43f5eff8675f116a2d` (Phase 6 sealed) +
  the Phase 7 harness + rust-equivalents package (this file's own commit).
- **Host:** 12th Gen Intel Core i7-12700K, 18 threads visible, WSL2 Linux,
  inside the `ynz-dev` container (the canonical build environment).
- **Command:** `docker compose run --rm dev cargo bench -p ynz-driver --bench opt_pipeline_calibration`
  — ONE session produced every number below (both languages, all groups), so
  the cross-language comparison is same-host, same-boot, same-run.
- **Yinz side:** bench (release) profile compiler via `CARGO_BIN_EXE_ynz`, so
  workload binaries link the release-profile `libynz_rt` (verified against
  `crates/ynz-driver/build.rs` — `PROFILE` at driver-build time selects the
  embedded runtime archive). Default tier = the shipped `ynz build`
  (`default<O2>` mid-end + Aggressive backend).
- **Rust side:** the hand-authored programs in `benches/rust-equivalents/`
  (deliberately NOT a workspace member — placement decision in its Cargo.toml
  header), built by the harness at run time via `cargo build --manifest-path`,
  rustc 1.96.0 stable (the container toolchain, verified via `rustc
  --version`), at two profiles:
  - `release` — idiomatic defaults (opt-level=3, codegen-units=16, no LTO,
    overflow-checks OFF). **Primary comparison**: Rust as real projects ship it.
  - `release-checked` — `overflow-checks = true`. **Secondary**: matches Yinz's
    always-checked int arithmetic, quantifying the semantic-difference cost.
- **Gates:** every binary (both languages, both Rust profiles) printed exactly
  the same closed-form checksum as its Yinz twin before benching — one shared
  oracle across languages. The dual-mode stdout and IR gates are Yinz-tier
  gates (Step 2) and do not apply cross-language.

## Raw medians (criterion, sample_size=10, flat sampling, 10s measurement)

| bench point | median | 95% CI |
|---|---|---|
| overhead/spawn_only (Yinz reps=0 binary) | 3.310 ms | [3.225, 3.396] |
| rust_equiv/spawn_only (Rust print-0 probe) | 0.847 ms | [0.752, 1.024] |
| opt_pipeline/cpu_loop/o0 | 59.958 ms | [58.449, 61.546] |
| opt_pipeline/cpu_loop/default | 35.670 ms | [34.827, 36.696] |
| opt_pipeline/shape_alloc/o0 | 43.063 ms | [42.104, 44.126] |
| opt_pipeline/shape_alloc/default | 16.094 ms | [15.750, 16.449] |
| opt_pipeline/soa_physics/o0 | 83.096 ms | [82.063, 84.274] |
| opt_pipeline/soa_physics/default | 61.505 ms | [59.069, 64.073] |
| rust_equiv/cpu_loop/release | 12.853 ms | [12.694, 13.029] |
| rust_equiv/shape_alloc/release | 6.524 ms | [6.472, 6.575] |
| rust_equiv/soa_physics/release | 8.934 ms | [8.859, 9.013] |
| rust_equiv/cpu_loop/release-checked | 15.608 ms | [14.592, 16.880] |
| rust_equiv/shape_alloc/release-checked | 8.859 ms | [8.736, 8.989] |
| rust_equiv/soa_physics/release-checked | 6.709 ms | [6.355, 7.012] |

This session's A/B replication (net default-over-o0: 1.75x / 3.11x / 1.37x)
agrees with the Step 2 record's 1.72x / 3.01x / 1.49x within the CIs.

## Startup / runtime-init cost (a named comparability item)

Each language's workloads are read net of its OWN spawn baseline. The
baselines differ: 3.310 ms (Yinz — process spawn + Yinz runtime init) vs
0.847 ms (Rust — process spawn only). **Measured Yinz runtime-init cost:
~2.46 ms per process.** Irrelevant to long-running programs, dominant for
sub-10ms CLI invocations; excluded from the ratios below so they measure
compute, not startup.

## The measured position: Yinz `ynz build` vs Rust `cargo --release`

Net = each side's own spawn baseline subtracted. Ratio = Yinz-default net ÷
Rust net (higher = Rust faster).

| workload | Yinz default (net) | Rust release (net) | **gap vs release** | Rust checked (net) | gap vs checked |
|---|---|---|---|---|---|
| cpu_loop | 32.360 ms | 12.006 ms | **2.70x** | 14.761 ms | 2.19x |
| shape_alloc | 12.784 ms | 5.677 ms | **2.25x** | 8.012 ms | 1.60x |
| soa_physics | 58.195 ms | 8.087 ms | **7.20x** | 5.862 ms | 9.93x |

**Honest headline: on these three microworkloads, idiomatic Rust `--release`
is ~2.2–2.7x faster than shipped Yinz on scalar/shape work and ~7–10x faster
on the array-scan workload. Yinz is NOT at Rust parity as of v0.3-M7.** What
the pipeline DID achieve (Step 2 record) is real: 1.4–3.1x over Yinz's own
`--no-optimize` tier, with correctness gates proving the wins are honest.

Where the gap comes from (consistent with the Step 2 record's headroom
analysis, stated as evidence-backed attribution, not excuse):

- **Opaque runtime-call floor** — Yinz element access lowers to
  `ynz_array_get` calls LLVM cannot inline or vectorize across; Rust iterates
  a `Vec` directly and vectorizes freely (~0.48 ns/visit vs ~3.46 ns/visit on
  soa_physics — the 7x is almost entirely this floor).
- **Always-on overflow checks** — quantified by the release-checked column:
  matching Yinz's semantics narrows the scalar gaps to 2.19x (cpu_loop) and
  1.60x (shape_alloc). Roughly a fifth to a third of the scalar gap is this
  semantic choice, not optimizer quality.
- The remainder is mid-end/backend maturity (no LTO, no PGO, no
  vectorization tuning in Yinz's `default<O2>` tier today).

## What is and is not comparable

- **Comparable:** same algorithmic work, same hardcoded rep counts, same
  closed-form checksum oracle, same host, same session, both sides net of
  their own measured spawn cost, both compilers on LLVM backends.
- **Not identical optimization configs:** Rust release defaults to
  opt-level=3 / codegen-units=16 / no LTO; Yinz's default tier is
  `default<O2>` + Aggressive backend. This is deliberate — the comparison is
  shipped-default vs shipped-default, not a tuned-flag shootout.
- **Overflow semantics differ by design:** Yinz always checks; Rust release
  wraps. Both Rust profiles are reported (labeled) so neither side's
  semantics is silently privileged.
- **soa_physics is the least apples-to-apples workload:** the Yinz binary
  pays the runtime array-ABI per element (and its layout is subject to the
  default SoA admission decision); the Rust program is a plain `Vec` scan.
  It honestly measures shipped-binary vs shipped-binary, but it reflects the
  runtime-call ABI floor more than mid-end optimizer parity — which is
  exactly why it shows the largest gap.
- **Microbenchmark sensitivity:** release-checked is FASTER than release on
  soa_physics (6.709 vs 8.934 ms — reproduced across two independent runs;
  codegen/unroll luck under different check insertion). Within-Rust,
  same-program variance of that size (~25%) bounds how finely any single
  ratio here should be read. Three workloads on one host is an evidence
  base, not a benchmark suite of record.
- **Idiomatic-shape difference:** Rust programs use `for` loops (idiomatic)
  vs Yinz's `while` loops; both lower to identical loop structure and this is
  noise-level.
