# v0.3-M9 C7 — release overhead against pre-C7 Yinz and against Rust (Golden Rule 6 check)

Measures what the scope-exit release of C7 costs. Provenance record in the style of
`rust-equiv-raw-2026-07-17.md`: challenge by rerun, not by citation.

## Method

- **Compilers.** HEAD of `feat/v0-3-m9-c7-s3b` (C7 slices 1–3b) and the pre-C7 tree `73d25ac`
  (`git archive` into a scratch directory), each built `cargo build --release -p ynz-driver --bin ynz`,
  so both link the release-profile runtime. Default tier (`ynz build`, `default<O2>`).
- **Rust.** `benches/rust-equivalents/` built `--release` (rustc stable, idiomatic: overflow checks
  off). Three new equivalents sit beside the old three: `nested_cell`, `string_cycle`,
  `channel_share`. Every binary prints the same closed-form checksum as its Yinz twin (verified).
- **Timing.** Median wall time of 15–41 consecutive runs of the process (`date +%s%N` around the
  run, stdout to `/dev/null`), one host, one session, quiet machine (no concurrent build or test
  run). Medians in microseconds. No criterion: these are comparisons of ratios, not absolutes.
- **Host.** Linux sandbox, 16 logical CPUs. Absolute numbers are not comparable with the 2026-07-17
  record (different host); ratios are.
- **Yinz sources** are the ones of `opt_pipeline_calibration.rs` (`cpu_loop` 2^24, `shape_alloc` 2^23,
  `soa_physics` 64 players x 2^18) and three loops of 2^22 iterations:
  - `nested_cell`: `let tag: Tag = { id: i }`, `let wrap: Wrap = { inner: tag, count: 3 }`, read both.
  - `string_cycle`: `let text: string = i.toString()`, `acc = acc + text.count()`.
  - `channel_share`: `let held: channel<int> = passOn(ch)` where `passOn(share ch) -> channel<int>`
    returns its argument (the callee's `return` takes the counted reference, the scope exit gives it back).

## 1. The three standing workloads: no release inside the loop

The IR of the three hot loops is identical before and after C7 (diff of the emitted `.ll` after
removing numbers and file names): the only additions are, once after the loop, the release of the
`print` argument temporary (`ynz_string_release`) and, in `soa_physics`, one `ynz_array_release`.

| workload | pre-C7 | HEAD | Rust |
|---|---|---|---|
| cpu_loop | 42.6 ms | 42.8 ms | 17.4 ms |
| shape_alloc | 18.5 ms | 20.0 ms | 1.1 ms (Rust folds the loop away) |
| soa_physics | 69.3 ms | 72.9 ms | 10.0 ms |

shape_alloc and soa_physics show 4–8% between the two compilers with identical loops. It is code
layout, not release: relinking the SAME two `.ll` files with one `clang -O2` command flips the
order (shape_alloc: pre-C7 19.8 ms, HEAD 18.4 ms). C7 adds no cost to a loop that owns no heap.
The gap to Rust on these three predates C7 (the old record: 2.8x, 2.5x, 6.9x) and is the
per-iteration `ynz_rt_check_preempt` and stack save/restore plus Yinz's always-checked arithmetic,
not release.

## 2. Loops that allocate and release

| workload | pre-C7 (leaks) | HEAD | Rust (drops) | HEAD / Rust |
|---|---|---|---|---|
| nested_cell | 121.7 ms | 39.3 ms | 35.7 ms | 1.10x |
| string_cycle | 208.7 ms | 128.7 ms | 56.3 ms | 2.29x |
| channel_share | 8.7 ms (no count taken) | 53.3 ms | 44.1 ms | 1.21x |

Pre-C7 is slower on the two allocating loops because it never frees (fresh memory every
iteration); it is not a baseline for the cost of releasing.

**Release cost in isolation, counted references.** Removing the release call from the emitted IR
(`ynz_channel_free`), and `mem::forget` in Rust, with no allocation to distort it:

| | with release | without | release cost per iteration |
|---|---|---|---|
| Yinz `channel_share` | 53.3 ms | 31.6 ms | 5.2 ns |
| Rust `Arc::clone` + drop | 44.1 ms | 23.1 ms | 5.0 ns |

The release (an atomic decrement) costs what Rust's does. The remaining 8.5 ms of the gap to Rust
is the loop's preempt check and the out-of-line `ynz_channel_share` call, which are not release.

For `nested_cell` and `string_cycle` the same removal is not a valid isolation: dropping the free
makes the loop slower (89.9 vs 35.7 ms in Rust, 117 vs 39 ms in Yinz), because a leaking loop never
reuses a freed block. The release is the cheaper side there; what separates Yinz from Rust is the
allocation path (`ynz_alloc`, the string builder behind `toString`) and the loop overhead above.

## 3. The null check on a nested-shape release against Rust's `Box` drop

A nested shape's cell is released through a function that returns on a null cell (a field a `let`
took out of a temporary is null, `lower_taken_field`). Rust's `Box` drop has no such check.

- **Folded where the cell is allocated in the same function.** In `nested_cell` the optimized IR
  frees the cell unconditionally (`tail call void @ynz_free(ptr nonnull %field_own_cell, i64 8)`):
  LLVM proves the pointer non-null. Same when `make` is a local function the optimizer inlines.
- **Present where the shape comes from another module** (`proj`: `make` exported from `helpers`;
  `%cell_taken.i.i = icmp eq ptr %call.elt, null` then a branch). A/B of the same IR with the branch
  replaced by an unconditional jump, one `clang -O2` link line for both, medians of 31 runs, three
  rounds:

| | with the check | without | Rust `Box` (out-of-line `make`) |
|---|---|---|---|
| 2^22 allocate + release | 42.5, 42.3, 41.9 ms | 41.5, 41.2, 41.5 ms | 38.0, 37.4, 37.8 ms |

The check costs 0.1–0.25 ns per release (0.4–1.1 ms over 4.2 million releases, 1–2.5%), a
predictable never-taken branch. It is a measurable cost over Rust's `Box` drop, small, and only
where the cell's origin is opaque. **Changed in C7 slice 4**: the check is emitted only for a shape some
`Take` in the module moves out (`NonOwnedReads::takes_nested_shape`; a nulled field is only ever in a
temporary of the taking module, released by that module's own release function).

**Re-measure after the change** (slice 4). `proj`: `helpers` exports `Tag { id }`, `Wrap { inner: Tag, count }`
and `make`; `entrypoint` runs `let wrap = make(i)`, `acc = acc + wrap.count` 2^22 times (the cell is
never read, otherwise LLVM proves it non-null and folds the check in both builds). Both builds are the
`--no-optimize` IR of each module, compiled separately by `clang -O2` and linked against the
release-profile runtime. The "with check" build is the same compiler with the condition forced on.
Optimized IR of the entry module: 1 `icmp eq ptr .., null` with the check, 0 without. Medians of 31
alternating runs, three rounds:

| | with the check | without (slice 4) |
|---|---|---|
| 2^22 allocate + release | 42.2, 41.6, 41.6 ms | 41.3, 41.5, 41.4 ms |

The difference is 0.05-0.9 ms over 4.2 million releases (0.01-0.2 ns each), inside the run-to-run
spread; the check is gone from the IR, which is what the change promises. Both forms are pinned by
`v03_m9_c7_slice4`.

## 4. What costs more than Rust (for the lead)

Nothing in C7's release is slower than Rust's equivalent per release (counted reference equal; the
nested-cell null check +0.1–0.25 ns where it is not folded). Absolute totals are above Rust's:
`nested_cell` +10%, `channel_share` +21%, `string_cycle` 2.3x, and the three standing workloads 2.5x,
17x (Rust removes the loop), 7x. None of that difference is release: the loops with no heap are
IR-identical to pre-C7, and the isolated release cost equals Rust's. It is the per-iteration preempt
check, the allocator and string-builder path, and checked arithmetic.
