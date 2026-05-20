# LSP Framework Spike

Phase 1 of v0.2-M2 built minimal "hello LSP" implementations against two frameworks to measure plumbing footprint and `&mut CompilerDb` ergonomics.

**Decision: `lsp-server 0.7.9`** — see `MEASUREMENTS.md` for full comparison.

## What the spikes measure

Each spike implements:
- `initialize` lifecycle + `initialized` + `shutdown` + `exit`
- `textDocument/didOpen` → publishes one diagnostic via `publishDiagnostics`
- A `DbSimulation` struct that mimics `CompilerDb` ownership (`update(&mut self)` / `query(&self)`)

The `DbSimulation` is the key ergonomics probe: it forces each framework to expose its answer to "how does `&mut db` work in your handler model?"

## Spikes

- `lsp_server/` — **WINNER**. Sync, single-thread dispatch, natural `&mut db`. Preserved as reference.
- `tower_lsp/` — **LOSER**. Deleted from tree after decision. See git history for code.

## Running the winning spike

```bash
cd crates/ynz-lsp/_spike/lsp_server
cargo run
```

Then pipe JSON-RPC messages to its stdin (see `../MEASUREMENTS.md` for the test harness used during the spike).
