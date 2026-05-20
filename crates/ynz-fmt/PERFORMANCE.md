# ynz-fmt Performance Measurements — v0.2.0-m3

Recorded at Phase 6 verification, 2026-05-20.

**Machine**: Linux 6.6 (WSL2), x86_64
**Build**: `cargo build --release -p ynz-driver` (LLVM 18.1.8, Rust 1.95 stable)

## Budgets vs Actuals

| Benchmark | Budget | Actual | Status |
|---|---|---|---|
| `ynz fmt examples/pirates-roster/entrypoint.ynz` (median of 5) | < 100 ms | **41 ms** | ✓ |
| `ynz fmt <3000-line synthetic .ynz>` | < 500 ms | **16 ms** | ✓ |
| `ynz fmt --check --all examples/pirates-roster/` (1 file) | < 2 s | **44 ms** | ✓ |

## Notes

- The 3000-line synthetic file (`/tmp/big_synth.ynz`) was generated with 500 functions, each
  with 4 statements and arithmetic bodies.  Median across 3 runs.
- `ynz fmt --check` (read-only mode) was used for benchmarks to avoid filesystem I/O influencing
  the formatter's hot path.
- The dominating cost is parser overhead (`parse_query` + `lex_with_trivia`), not the
  formatter's own emit loop.  The emit loop is linear in AST node count; no super-linear paths.

## Trivia-lex overhead

`lex_with_trivia` runs a second pass over the source to capture `//` comments.  The additional
cost vs plain `lex` is O(n) in source length but negligible in practice: the `trivia_comments`
field is a simple `Option<Vec<Comment>>` conditional branch inside the existing lexer loop.
At the entrypoint.ynz scale (~400 LOC), the difference is < 1 ms.
