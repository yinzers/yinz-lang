---
name: Yinz Test Parallelism Protocol
description: "Applies when adding a fixture, corpus or sweep-style test harness."
paths:
  - crates/*/tests/**/*.rs
---

- **Reuse `parallel_sweep` in `crates/ynz-driver/tests/cross_impl_consistency.rs` rather than writing a second one.** It uses an atomic cursor, not pre-chunked ranges, because fixture cost varies by more than 10x and static chunks strand workers behind the slow ones. It sorts findings before asserting, because completion order is nondeterministic and failure output must not be. It sizes itself with `available_parallelism()`, capped at the item count. Why: it scales from a big workstation to a small CI runner with no tuning knob.
- **Run `ynz run` concurrently without extra locking.** It builds in a per-invocation temp directory, so parallel runs cannot collide.
- **Keep `/tmp` on tmpfs with `exec` in the dev container.** Why: `ynz run` links a binary there and executes it.
