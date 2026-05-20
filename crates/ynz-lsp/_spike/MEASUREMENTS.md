# LSP Framework Spike — Measurements

Spike date: 2026-05-20
Rust toolchain: 1.95 stable

Both spikes implement: `initialize` lifecycle, `initialized`, `shutdown`, `did_open` (publishes one hardcoded diagnostic using a `DbSimulation` struct that mimics `CompilerDb`'s ownership model: `update(&mut self)` for mutations, `query(&self)` for reads).

---

## Results Table

| Metric | `lsp-server 0.7.9` | `tower-lsp 0.20.0` |
|---|---|---|
| Plumbing lines (total main.rs) | **108** | **102** |
| Direct Cargo deps | 3 (`lsp-server`, `lsp-types`, `serde_json`) | 3 (`tower-lsp`, `tokio`, `serde_json`) |
| Transitive dep tree nodes | **102** | **189** |
| Debug binary size | **17 MB** | **53 MB** |
| `&mut db` ergonomics | **Natural — owned in main loop** | **Arc<Mutex<Db>> required** |
| Async overhead | **None — synchronous** | tokio runtime (~200 transitive crates) |
| Test harness setup | Direct function call or thin sync wrapper | Async test runtime needed (tokio::test) |
| Diagnostic published in spike | ✅ PASS | ✅ PASS |
| Maintenance (crates.io last publish) | 2024-07-12 (0.7.9, rust-analyzer team) | 2023-09-11 (0.20.0, original repo) |
| Open-issue count (spike day) | ~38 (rust-analyzer monorepo) | >60 (original repo, declared unmaintained) |
| Fork/maintenance status | Actively maintained by rust-analyzer | Original abandoned; fork `tower-lsp-f 0.25.0-beta3` exists but is beta |

---

## `&mut db` Ergonomics — the Key Decision Point

salsa's `CompilerDb` requires `&mut self` for input mutations (`source_file.text().set(&mut db).to(new_text)`). Every `didChange` notification is a mutation.

**lsp-server** — the main loop owns `db` directly:
```rust
let mut db = CompilerDb::default(); // owned, no wrapper needed
for msg in &connection.receiver {
    // didChange:
    source_file.text().set(&mut db).to(new_text); // natural &mut
    // check_query:
    let output = check_query(&db, source); // &db reference
}
```

**tower-lsp** — `did_open`/`did_change` handlers receive `&self`, so `db` must be wrapped:
```rust
struct Backend {
    db: Arc<tokio::sync::Mutex<CompilerDb>>, // wrapper required
}
async fn did_change(&self, params: ...) {
    let mut db = self.db.lock().await; // lock acquisition on every request
    source_file.text().set(&mut db).to(new_text);
    // all subsequent queries hold the lock too — serializes the whole handler
}
```

For thin-slice (one Patrick, one editor), the `Arc<Mutex<Db>>` is fine — there's rarely contention. But the single-threaded dispatch model we're building (see `design/lsp.md`) maps naturally to `lsp-server`'s sync model. The wrapper adds boilerplate that adds no value for our architecture.

---

## Integration-Test Setup Cost

**lsp-server**: in-process tests call the handler functions directly. No async runtime needed. A thin wrapper struct drives requests synchronously. `cargo test` just works.

**tower-lsp**: tests need `#[tokio::test]` runtime. The `LspService` struct is async-native — calling handlers from tests requires spawning the full server in a task and communicating over channels. More boilerplate per test.

---

## Maintenance Status Detail (spike day: 2026-05-20)

`tower-lsp 0.20.0` was published 2023-09-11. The original GitHub repo `ebkalderon/tower-lsp` appears unmaintained (no commits since mid-2023). A maintained fork exists at `lebensterben/tower-lsp-server`, published to crates.io as `tower-lsp-f 0.25.0-beta3` (beta status as of spike day). Migrating to the fork would require changing the import name and potentially API adjustments from the beta state.

`lsp-server 0.7.9` was published 2024-07-12 by the rust-analyzer team. It is actively developed alongside rust-analyzer, which is the reference Rust LSP implementation. The API is stable and well-documented.

Migration cost estimate (if lsp-server choice is wrong later): ~2 days of refactor in v0.2-M5, primarily wrapping the DB in `Arc<Mutex<...>>` and switching the main loop to an async task model. Not catastrophic.

---

## Decision

**Winner: `lsp-server 0.7.9`**

**Rationale:**

1. **Natural `&mut db` ownership** — no Arc<Mutex> overhead for our single-threaded dispatch model
2. **Smaller footprint** — 102 vs 189 transitive deps; 17MB vs 53MB binary
3. **Simpler test harness** — in-process sync tests, no tokio::test boilerplate
4. **Better maintenance posture** — actively maintained by rust-analyzer team; tower-lsp original is abandoned, fork is beta
5. **Decision criterion** — "smaller plumbing+test footprint without forcing async semantics over the salsa DB" → lsp-server wins on both criteria

The plan says "default to tower-lsp if both pass." Both passed. But the maintenance status and ergonomic advantage are concrete evidence that the criterion prefers lsp-server. The default-to-tower-lsp tiebreaker was for the case where measurements were equal; they are not.

**This choice locks for v0.2-M5.** When M5 adds go-to-def, find-refs, rename, the sync dispatch model scales horizontally via salsa snapshots (salsa supports `Snapshot<CompilerDb>` for concurrent reads). The `lsp-server` architecture accommodates this: the main loop keeps `&mut db` for mutations; read-only queries that don't need mutation can snapshot and run concurrently. This is cleanly expressible in the sync model; in the async model it would need additional `Arc<...>` gymnastics.
