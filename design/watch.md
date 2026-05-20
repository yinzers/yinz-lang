# `ynz watch` — Architectural Reference

> Audience: compiler contributors. For user docs, see `spec/watch.md` (ships with v0.2.0 final).
> Spec file for end users: `spec/watch.md`

Cross-references: `design/compiler.md`, `design/compiler-language.md`, `design/feature-registry.md`, `design/lsp.md`, `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md`

---

## Overview

`ynz watch` is a long-running terminal command that recompiles `.ynz` files on save and re-executes the program. Sub-second rebuild is achieved via a salsa-backed `CompilerDb` that lives for the lifetime of the watch process — file events mutate `SourceFile.text` inputs; salsa automatically invalidates and recomputes only the affected query results.

**Architecture: daemon (locked)**

One long-running process holds one `CompilerDb` instance. File events mutate `SourceFile.text` salsa inputs via `WatchDb`. Salsa invalidates downstream queries automatically. Sub-second target depends on this daemon pattern. No process-per-build design was considered; the per-salsa-DB overhead would defeat the sub-second requirement.

**Default behavior: build + run (locked)**

`ynz watch foo.ynz` rebuilds AND re-executes on every save. Mirrors `ynz run` semantics. Pass `--check` to skip the run step (CI gates, "I just want to see if it compiles" use case).

**Output: clear-screen by default (locked)**

Each rebuild cycle clears the terminal before printing the new status. Pass `--no-clear` to preserve scrollback (CI logs, debugging the watcher itself).

---

## Crate Layout

`crates/ynz-watch/` holds the watch daemon implementation. `crates/ynz-driver/src/watch.rs` is a thin shim that parses CLI args and calls `ynz_watch::run(config)`. This mirrors the `ynz-lsp` (M2) and `ynz-fmt` (M3) crate organization.

```
crates/ynz-watch/
  src/
    lib.rs          — pub API: run(WatchConfig) -> i32; WatchConfig struct
    error.rs        — WatchError enum + Result<T> alias
    watcher.rs      — notify + notify-debouncer-mini wrapper; WatchEvent iterator
    event_loop.rs   — main loop; event dispatch; shutdown
    db.rs           — WatchDb: CompilerDb + shadow HashMap<PathBuf, String>
    rebuild.rs      — orchestrate one rebuild cycle
    child.rs        — ChildHandle: spawn, kill, Drop
    output.rs       — terminal status-line formatting
    ui.rs           — clear-screen logic; TTY detection
    memory.rs       — cross-platform RSS polling via memory-stats crate
    lru.rs          — salsa LRU cap wiring + env-var overrides
    project.rs      — yinz.toml discovery; .ynz file enumeration
    json_events.rs  — typed event structs with #[derive(Serialize)]
    json_emitter.rs — JsonEmitter: write one NDJSON line per event; flush
  tests/
    file_watching.rs
    coalescing.rs
    rebuild_incremental.rs
    rebuild_errors.rs
    child_lifecycle.rs
    json_mode.rs
    long_session.rs
```

---

## File Watcher Integration

**Crates (locked versions)**:
- `notify = "8"` (8.2.0 current stable; 9.0.0-rc.* rejected — still rc)
- `notify-debouncer-mini = "0.7"` (0.7.0 current stable; compatible with notify 8)

**Cross-platform**:
| OS | Backend |
|---|---|
| Linux | inotify |
| macOS | FSEvents |
| Windows | ReadDirectoryChangesW |
| Network mounts (NFS, SSHFS, Docker volumes) | Falls back to polling |

When polling mode is detected at startup, watch prints: `"ynz watch is using filesystem polling on this mount; rebuild may lag"`.

**Editor-save patterns**:

Most editors (VS Code, Vim, Emacs) do NOT write directly to the target file. They write to a temp file and rename (atomic replace):
1. Write `foo.ynz.tmp` (CREATE event)
2. Rename `foo.ynz.tmp` → `foo.ynz` (RENAME/MOVED event)
3. Optional: CHMOD, CLOSE_WRITE, MODIFY events

Without coalescing, one save generates 3-4 raw events. `notify-debouncer-mini` coalesces within a configurable window (default 100ms, configurable via `YNZ_WATCH_DEBOUNCE_MS`).

**Single coalescing layer (locked)**:

Watch uses `notify-debouncer-mini` as the ONLY coalescing mechanism. There is NO second dedup layer in `event_loop.rs`. The debouncer is the single source of truth. This was explicitly locked after plan-review concern about double-dedup complexity causing missed events.

**File removal handling**:

When a watched `.ynz` file disappears mid-watch:
1. Watch logs: `"path/to/foo.ynz vanished; watch continues, will re-pick-up on re-creation"`
2. Emits JSON `file-removed` event (in `--json` mode)
3. Watch does NOT crash
4. Shadow source state retains last-known text for the missing file (next rebuild uses shadow)
5. File re-creation re-triggers normal watch behavior

**Symlinks: follow (locked)**

Consistent with `ynz fmt` (M3). Watch resolves symlinks via `notify`'s default behavior. Symlink target swap mid-watch is NOT specially handled — notify delivers whatever events it delivers for the swap. Documented: if you swap a symlink's target while watching, restart `ynz watch` for guaranteed correct behavior.

---

## WatchDb — Incremental State

`WatchDb` is the central state struct for the watch daemon. It holds two state stores:

```
WatchDb {
    compiler_db: CompilerDb,              // salsa DB — derived cache
    sources: HashMap<PathBuf, String>,    // shadow store — source of truth
}
```

**Shadow map is the source of truth.** Salsa is a derived cache. This distinction matters for Layer 2 DB rebuild (see Memory Defense below).

**`update_source(path, text)` — write order (locked)**:
1. Write to `self.sources` (shadow map) FIRST
2. Then mutate salsa input: `source_file.set_text(&mut self.compiler_db, new_text).to(new_text)`

If the process panics between these two writes, shadow remains consistent with the last-known content. Salsa state may be stale but will be correct on the next query.

**`rebuild_db()` — Layer 2 periodic rebuild**:
1. Drop `self.compiler_db`
2. Create `CompilerDb::default()`
3. Iterate `self.sources` map
4. For each `(path, text)`: call `db.set_source_text(path, text.clone())`
5. Result: fresh DB with identical input state

Zero source-state loss across rebuild. The shadow IS the source of truth.

**File deletion during rebuild**: events arriving while `rebuild_db()` runs are queued in the channel. After rebuild completes, the queued event is processed against the fresh DB. No events lost, no crashes against stale handles.

---

## Incremental Rebuild Flow

Per-save cycle (simplified):

```
[file event] → update_source(path, text) → check_query(path)
                                         → (if errors) render diagnostics → print "✗ N errors"
                                         → (if clean, !--check) codegen_query(path) → spawn binary
                                         → print "✓ Built in N ms"
```

**Initial build on start (locked)**: when watch boots, one rebuild+run pass runs before waiting for events. User sees immediate compile status without having to save first.

**Single-event-at-a-time invariant (locked)**: watch processes one build/run cycle to completion before starting the next. Rapid saves coalesce into "one pending rebuild" — multiple saves during one cycle collapse to a single next rebuild. This prevents concurrent builds against the same `CompilerDb`.

**Status lines**:
- `"▶ Building…"` — rebuild in progress
- `"✓ Built in 250ms"` — clean build
- `"✗ 3 errors"` — compile errors (diagnostics follow)
- `"✓ Watching…"` — idle, waiting for next save

---

## Child Process Lifecycle

**Spawn**: after successful codegen (if `!config.check`), watch spawns the compiled binary as a child process.

**Process group (locked)**:
- Unix: child spawned in its own process group via `nix::unistd::setsid()` in a `pre_exec` hook. This catches double-forked children (programs that `fork()` and `exec()` grandchildren).
- Windows: `CREATE_NEW_PROCESS_GROUP` flag via `std::os::windows::process::CommandExt`.

**Graceful kill on rebuild or Ctrl+C**:
1. Unix: `nix::sys::signal::killpg(pgid, Signal::SIGTERM)` — hits entire process group
2. Poll `child.try_wait()` every 50ms for up to 2s
3. If still alive: `child.kill()` (SIGKILL on Unix, TerminateProcess on Windows)

**stdin/stdout/stderr**: all three are inherited from the watch process's terminal. Interactive programs (`terminal.readLine()` style) work under watch. Child stdout/stderr stream live to the user's terminal between watch status lines.

**Status-line interleaving prevention (locked)**:
Watch flushes its status line BEFORE child stdout begins streaming. On next rebuild, watch prints `\n` + clear sequence (if `--no-clear` is off) before resuming status output. No ANSI cursor positioning (terminals without ANSI fall back gracefully).

**Tempdir for compiled binary**: `$TMPDIR/ynz-watch-<pid>-<seq>/`. Old tempdir cleaned on next iteration; all cleaned via Drop on watch exit.

**Windows note (locked, documented limitation)**: On Windows, Ctrl+C immediately terminates the child via TerminateProcess; in-flight writes (file handles, partial stdout flushes) may be lost. This is fundamental to Windows' lack of POSIX signals, not a watch-specific bug.

---

## `--json` Structured Event Mode

`ynz watch --json foo.ynz` emits NDJSON on stdout, suppresses normal text output. One event per line. Every event includes `type`, `timestamp`, and `schema_version` fields.

**Timestamp format (locked)**: RFC 3339 UTC with milliseconds. Example: `"2026-05-20T14:30:00.123Z"`. Regex: `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$`. Locale-independent; downstream parsers handle timezone conversion.

**Schema version (locked)**:
- Pre-v0.2.0: `"schema_version": "v0.2-m4-unstable"` — announces the mutable status. Schema may change between intermediate milestone tags; consumers must pin to a specific Yinz binary version.
- Post-v0.2.0 final: suffix drops to `"v0.2"`; semver applies to schema field additions/removals.

### Event Schema (v0.2-m4-unstable)

```jsonc
// Every event includes these three fields:
// "type": string (kebab-case event name)
// "timestamp": RFC 3339 UTC + ms — "2026-05-20T14:30:00.123Z"
// "schema_version": "v0.2-m4-unstable"

{"type": "watch-ready",    "timestamp": "...", "schema_version": "v0.2-m4-unstable",
                           "watching": ["path/to/file.ynz"]}

{"type": "build-start",    "timestamp": "...", "schema_version": "...", "file": "..."}

{"type": "build-end",      "timestamp": "...", "schema_version": "...", "file": "...",
                           "outcome": "ok" | "errors", "duration_ms": 250}

{"type": "diagnostic",     "timestamp": "...", "schema_version": "...", "file": "...",
                           "severity": "error" | "warning" | "suggestion",
                           "span": {"start": 123, "end": 145},
                           "what": "...", "what_instead": "...", "why": "..."}

{"type": "child-spawn",    "timestamp": "...", "schema_version": "...", "pid": 12345}

{"type": "child-exit",     "timestamp": "...", "schema_version": "...", "pid": 12345,
                           "exit_code": 0}

{"type": "file-removed",   "timestamp": "...", "schema_version": "...", "file": "..."}

{"type": "memory-warning", "timestamp": "...", "schema_version": "...",
                           "rss_mb": 1024, "threshold_mb": 1024}

{"type": "memory-stop",    "timestamp": "...", "schema_version": "...",
                           "rss_mb": 4096, "threshold_mb": 4096}

{"type": "memory-unavailable", "timestamp": "...", "schema_version": "...",
                               "reason": "polling unavailable on this platform"}

{"type": "watch-shutdown", "timestamp": "...", "schema_version": "...",
                           "reason": "ctrl-c" | "fatal" | "oom" | "pipe-closed"}
```

### Event-Ordering Invariants

Consumers can rely on these ordering guarantees:
- `watch-ready` is always the FIRST event; `watch-shutdown` is always the LAST.
- Every `build-start` is followed by exactly one `build-end` for the same file.
- `child-spawn` ONLY appears after `build-end { outcome: "ok" }`. NEVER after `outcome: "errors"`.
- `child-exit` follows `child-spawn` (eventually; may be hours later for long-running programs).
- `memory-warning` precedes any `memory-stop`; `memory-stop` precedes `watch-shutdown { reason: "oom" }`.

### EPIPE Handling (locked)

If the downstream consumer pipe closes (e.g., `ynz watch --json | jq .` and `jq` exits), watch detects `EPIPE` on the next write. Response: emit `WatchShutdown { reason: "pipe-closed" }` to stderr as a last-ditch message, drop child, exit code 0. NOT a crash; clean termination.

### `yinz.toml` edits ignored (locked, documented limitation)

The `notify` watcher subscribes to `.ynz` files only. `yinz.toml` is read once at watch boot. Edits to `yinz.toml` (adding files, changing project config) do NOT trigger a rebuild or re-discovery. Restart `ynz watch` to pick up `yinz.toml` changes. Tracked in v0.5 package-manager milestone as `watch-yinz-toml-reload`.

---

## Memory Defense

Three layers protect against long-session memory growth (all three locked; see Constraints in plan `v0-2-m4-watch.md`):

### Layer 1: Salsa LRU Caps

Per-query LRU eviction caps applied via `#[salsa::tracked(lru = N)]`:

| Query | Cap | Rationale |
|---|---|---|
| `parse_query` | 128 | Cheap to recompute; keep more |
| `module_signatures_query` | 128 | Same |
| `check_query` | 64 | Moderate cost |
| `codegen_query` | 32 | Heaviest; smallest cap acceptable |

**Tuning surface**: one primary env var `YNZ_WATCH_LRU_SCALE` (default `1.0`) multiplies all four defaults proportionally. Advanced per-query overrides: `YNZ_WATCH_LRU_PARSE`, `YNZ_WATCH_LRU_SIG`, `YNZ_WATCH_LRU_CHECK`, `YNZ_WATCH_LRU_CODEGEN`.

### Layer 2: Periodic DB Rebuild

Drop + recreate `CompilerDb` every N=500 rebuilds OR T=4h elapsed, whichever comes first.

- **N default**: 500 (configurable via `YNZ_WATCH_REBUILD_AFTER`)
- **T default**: 4h (configurable via `YNZ_WATCH_REBUILD_AFTER_HOURS`)
- **Time source**: `std::time::Instant::now()` — monotonic; immune to NTP / clock-skew. Never `SystemTime::now()`.
- **State preservation**: shadow `HashMap<PathBuf, String>` repopulates the fresh DB. Zero source-state loss.

### Layer 3: RSS Polling

After every rebuild, sample process RSS via `memory-stats = "1.2"` crate.

| Threshold | Default | Env var | Action |
|---|---|---|---|
| Soft warn | 1024 MB | `YNZ_WATCH_RSS_WARN_MB` | Single stderr message + `MemoryWarning` JSON event; rate-limited to 1/60s |
| Hard stop | 4096 MB | `YNZ_WATCH_MAX_RSS_MB` | `MemoryStop` JSON event + WHAT/WHAT-INSTEAD/WHY message + exit code 2 |

**Hard-stop message (locked WHAT/WHAT-INSTEAD/WHY)**:
```
WHAT: ynz watch hit 4GB memory; this is the safety stop.
WHAT INSTEAD: Run `ynz watch <args>` to restart.
WHY: If this happens frequently, set YNZ_WATCH_REBUILD_AFTER=200 (default 500)
     to rebuild the compiler state more often. This releases accumulated
     salsa cache memory more frequently.
```

**RSS poll failure**: if `memory_stats()` returns `None` (platform doesn't support polling): emit `MemoryUnavailable` JSON event once, set an internal flag, continue without the hard-stop safety net. Watch does not crash.

---

## Cross-Platform Notes

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| File watching | inotify | FSEvents | ReadDirectoryChangesW |
| Child process group | `nix::unistd::setsid()` via `pre_exec` | Same | `CREATE_NEW_PROCESS_GROUP` |
| Graceful child kill | SIGTERM → 2s → SIGKILL | Same | TerminateProcess (no grace) |
| RSS polling | `/proc/self/status` (via `memory-stats`) | mach `task_info` (via `memory-stats`) | `GetProcessMemoryInfo` (via `memory-stats`) |
| Clear screen | ANSI `\x1b[2J\x1b[H` | Same | Same (if terminal supports ANSI) |

**Windows graceful-shutdown limitation** (documented, not a bug): Windows has no POSIX signals. Ctrl+C sends `CTRL_C_EVENT` to the console; watch kills the child via `TerminateProcess` which is immediate (no grace period). In-flight I/O in the child (partial file writes, network buffers) may be lost. This is fundamental to Windows' architecture. Tracked in `todos.md` as `watch-windows-validation` for when Yinz formally supports Windows.

---

## Debounce Strategy

**Default window**: 100ms (configurable via `YNZ_WATCH_DEBOUNCE_MS`).

Rationale: editor save sequences typically settle within 50ms (empirical: VS Code atomic-write on macOS takes ~30ms). 100ms is conservative without feeling laggy. Matches `cargo-watch` default.

**Why `notify-debouncer-mini` instead of rolling our own**: established crate, single-purpose, well-tested cross-platform, compatible with notify 8. Rolling our own would duplicate well-understood logic and add CI surface.

---

## Project Mode vs Single-File Mode

`ynz watch` uses three invocation modes (mirrors `ynz build` / `ynz run`):

| Invocation | Mode | Behavior |
|---|---|---|
| `ynz watch foo.ynz` | Single-file | Watches `foo.ynz` only |
| `ynz watch .` | Project | Reads `yinz.toml` at cwd; watches all `.ynz` files |
| `ynz watch ./path/` | Project (explicit root) | Reads `yinz.toml` at given path |

**No `yinz.toml` in project mode**: emits `watch-no-yinz-toml` diagnostic and exits code 2.

**`yinz.toml` edits during watch**: ignored until restart. Watch subscribes only to `.ynz` files. See locked limitation above.

---

## Env Vars (all tunable, none required)

| Var | Default | Purpose |
|---|---|---|
| `YNZ_WATCH_DEBOUNCE_MS` | 100 | File-event debounce window in ms |
| `YNZ_WATCH_REBUILD_AFTER` | 500 | Layer 2: drop + recreate DB after N rebuilds |
| `YNZ_WATCH_REBUILD_AFTER_HOURS` | 4 | Layer 2: also rebuild after N hours |
| `YNZ_WATCH_MAX_RSS_MB` | 4096 | Layer 3: hard-stop RSS ceiling in MB |
| `YNZ_WATCH_RSS_WARN_MB` | 1024 | Layer 3: soft-warn RSS threshold in MB |
| `YNZ_WATCH_LRU_SCALE` | 1.0 | Layer 1: multiply all four LRU caps proportionally |
| `YNZ_WATCH_LRU_PARSE` | (from scale) | Advanced: override parse_query LRU cap directly |
| `YNZ_WATCH_LRU_SIG` | (from scale) | Advanced: override module_signatures_query LRU cap |
| `YNZ_WATCH_LRU_CHECK` | (from scale) | Advanced: override check_query LRU cap |
| `YNZ_WATCH_LRU_CODEGEN` | (from scale) | Advanced: override codegen_query LRU cap |

---

## Exit Codes

| Code | Meaning |
|---|---|
| 0 | Clean exit (Ctrl+C with no pending build; or clean shutdown) |
| 1 | First-build compile error when `--check --exit-on-first-error` passed |
| 2 | Infrastructure error: file watcher init failed, no `yinz.toml`, OOM hard-stop, etc. |

---

## Future-Proofing

### Interactive commands (deferred to v0.3+)

Press `r` to trigger manual rebuild, `q` to quit, etc. Not in M4 — M4 ships a non-interactive loop only. Design pointer: when this ships, the key-reading loop belongs in `event_loop.rs` alongside the file-event loop, not as a separate thread. Registered in `registry/features.toml` as `ynz-watch-interactive-commands`.

### Config file rejection (explicit)

`.ynzwatch.toml` (or equivalent per-watch config file) was considered and explicitly rejected. Yinz's zero-config ethos means env vars only for tuning. A config file would add discovery logic, schema versioning, and documentation overhead for parameters most users never touch. If real demand emerges, the decision can be revisited. Registered in `registry/features.toml` as `ynz-watch-config-file` (rejection, not deferral).

### `yinz.toml` hot-reload (deferred to v0.5)

Watch currently reads `yinz.toml` once at boot. Hot-reload (picking up file additions, project config changes without restart) defers to the v0.5 package-manager milestone. Registered in `todos.md` as `watch-yinz-toml-reload`.

### LSP shared daemon (deferred to v0.3+)

`ynz-watch` and `ynz-lsp` each hold an independent `CompilerDb`. Potential shared-daemon architecture (one DB, two event consumers) deferred to v0.3+. Registered in `todos.md` as `watch-lsp-shared-daemon`.

---

## Measurement (Phase 6)

Performance ceilings are measured in Phase 6 against the following protocol (per Performance Invariants in the plan file):

- **Hardware**: GitHub Actions `ubuntu-latest` runner
- **Build profile**: release
- **Fixtures**: `crates/ynz-watch/benches/fixtures/perf_500.ynz`, `perf_5000.ynz`, `perf_project/`
- **Sample protocol**: 10 cold + 100 warm runs; report p50 + p99 of warm runs
- **Source**: `--json` mode `BuildStart`/`BuildEnd` timestamps

Phase 6 results will be filled in here after measurement.

---

## Cross-References

- `design/compiler.md` — salsa architecture; quick-start watch summary
- `design/compiler-language.md` — salsa-first architecture; "Why Salsa" section
- `design/feature-registry.md` — SSOT for deferred tooling features registered from M4
- `design/lsp.md` — parallel daemon (ynz-lsp); no shared code with watch (independent daemons, parallel-shippable per roadmap)
- `design/teaching-mission.md` — WHAT/WHAT-INSTEAD/WHY format watch diagnostics use
- `design/compiler-errors.md` — banned-jargon source-of-truth; watch must not emit banned words
- `design/versioning.md` — pre-v1.0 breaking-change policy; `--json` schema version strategy
- `registry/features.toml` — `ynz-watch-interactive-commands`, `ynz-watch-config-file` deferred entries
- `.claude/plans/active/v0-2-m4-watch.md` — implementation plan; all constraints + locked decisions
- `.claude/plans/done/v0-2-m2-lsp-thin-slice.md` — daemon-with-CompilerDb pattern reference
- `.claude/plans/done/v0-2-m3-fmt.md` — library + CLI shape reference; cross-platform test patterns
