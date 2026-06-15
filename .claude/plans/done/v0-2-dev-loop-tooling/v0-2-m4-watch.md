---
slug: v0-2-m4-watch
type: execution
owner: Patrick Rizzardi
status: done
roadmap: v0-2-dev-loop-tooling
milestone: v0-2-m4-watch
created: 2026-05-20
last_updated: 2026-05-20 (post-ship bugs fixed — see Post-Ship Fixes section)
review_rounds:
  - round: 1
    reviewer: plan-reviewer
    verdict: BLOCK
    required_fixes_addressed: 12/12  # salsa LRU locked (exists, syntax = #[salsa::tracked(lru = N)]); macOS RSS via memory-stats crate; WatchDb shadow HashMap for source persistence on DB rebuild; nix locked for SIGTERM; --json timestamp = RFC 3339 UTC; schema versioning honest pre-v0.2.0; 10k test methodology tightened; Performance ceilings have measurement protocol; demo counter file removed; Phase 0 trimmed; notify = 8.2 locked; Windows explicit best-effort.
    concerns_addressed: 5/5  # POSSIBLE deps removed; tcp-pause-signal placeholder removed; initial-build risk moved to locked behavior; debouncer-only (no event_loop dedup); Phase 2 explicit ships --check working.
    adversarial_addressed: 8/8  # file delete, symlink swap, double-fork process group, RSS poll returns 0, clock skew Instant, save-during-DB-rebuild, --json EPIPE, tempdir-full event ordering.
  - round: 2
    reviewer: plan-reviewer
    verdict: PASS
    required_fixes_addressed: 0  # none requested
    concerns_addressed: 3/3  # Windows graceful-shutdown limitation documented; YNZ_WATCH_LRU_SCALE single-knob added (per-query overrides retained as advanced); 10k synthetic test methodology locked to mutate-via-API (not real-FS) to eliminate CI flakiness.
    adversarial_addressed: 2/2  # status-line/child-stdout interleaving (Phase 3 test); yinz.toml-edit-during-watch (Phase 4 test).
status_after_round_2: ready_for_patrick_approval
files:
  - crates/ynz-watch/**
  - crates/ynz-driver/src/**
  - design/watch.md
  - design/compiler.md
  - design/mvp-scope.md
  - CLAUDE.md
  - examples/pirates-roster/entrypoint.ynz
  - examples/primantis-orders/v0_2_m4_errors.ynz
  - Cargo.toml
depends_on: [v0-2-m1-feature-inventory-sync]
---

# Plan: v0.2-M4 — `ynz watch`

Created: 2026-05-20
Status: pending_approval

## Context & Why

**Goal**: Ship `ynz watch` — a long-running terminal command that recompiles `.ynz` files on save and re-executes the program (or just rebuilds, with `--check`). Sub-second rebuild via salsa-cached incremental compute. Same diagnostic rendering as `ynz build`. Additionally ships `--json` structured-event mode for build-automation tooling consumers.

**Why now**:
- v0.2-M1 shipped the SSOT registry (`crates/ynz-registry/src/lib.rs`) — watch consults it for keyword spellings used in diagnostic rendering. No drift surface.
- v0.2-M2 shipped `ynz-lsp` — proves the "long-lived daemon holding `CompilerDb`" pattern works in practice. Watch follows the same architectural shape but for terminal users.
- v0.2-M3 shipped `ynz fmt` — formatter library exists, but watch does NOT format on its own (formatting is an editor concern). M3 is not a dependency of M4 (parallel-shippable per roadmap).
- For developers in vim/neovim without LSP, in CI pipelines, or just preferring a terminal dev-loop, `ynz watch` is the canonical fast-iteration story. Without it, the v0.2 dev-loop story has a hole.
- The roadmap (`v0-2-dev-loop-tooling`) explicitly puts watch in M4 parallelizable with M3. M2 + M3 are shipped; M4 starts now.

**Background**:
- 12 workspace crates as of M3 (`crates/ynz-ast`, `ynz-codegen`, `ynz-diagnostics`, `ynz-driver`, `ynz-fmt`, `ynz-lsp`, `ynz-numerics`, `ynz-parser`, `ynz-registry`, `ynz-runtime`, `ynz-tmgrammar`, `ynz-typeck`). Cargo.toml at `0.2.0-m3` (verified 2026-05-20).
- The `ynz` driver CLI has `Build`, `Run`, and `Fmt` subcommands (`crates/ynz-driver/src/main.rs:39-130`). M4 adds `Watch`.
- Salsa 0.26.2 (`#[salsa::tracked]` style, salsa-2022 architecture) is used throughout. `CompilerDb` is `Default`-constructible and holds `SourceFileRegistry`. Inputs (`SourceFile.text`) are mutated via `.set(&mut db, text).to(new_text)`; salsa invalidates dependent queries automatically.
- `ynz run` builds + executes a `.ynz` file once (`crates/ynz-driver/src/run.rs`). The compiled artifact is deleted by default (`--keep` to retain). Watch reuses the same Build→Run pipeline but in a loop.
- Yinz's "sub-second incremental recompile" target is stated in `design/compiler.md:11` and the watch section at `design/compiler.md:138-146`. M4 turns that paragraph into a working command.
- 1143 tests passing on `main` as of M3 ship (2026-05-20).

**Constraints (locked from roadmap + this planning session)**:
- **Daemon architecture LOCKED** (no spike): one long-running process holds one `CompilerDb` instance; file events mutate `SourceFile.text` inputs; salsa invalidates downstream queries. Sub-second target depends on this. Confirmed by Patrick this session.
- **Default behavior = build + run** (mirrors `ynz run`): `ynz watch foo.ynz` rebuilds AND re-executes on save. `--check` flag skips the run step (CI gate / "I just want to see if it compiles" use case). Confirmed by Patrick this session.
- **Output style LOCKED**: clear screen by default; `--no-clear` flag preserves scrollback (CI logs, debugging the watcher itself).
- **`--json` mode SHIPS IN M4** (not deferred): emits NDJSON event stream on stdout, suppresses normal text output. Schema documented in `design/watch.md` and registered as a stable interface (semver applies once v0.2.0 ships).
- **Memory safety LOCKED — multi-layered defense** (per Patrick "100 extra lines, worth the initial investment"):
  - **Layer 1 — Salsa LRU caps**: VERIFIED EXISTS in salsa 0.26.2 (`/home/ubuntu/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/salsa-0.26.2/src/function/eviction/lru.rs`; macro syntax confirmed in `tests/lru.rs:40`; runtime `set_lru_capacity` confirmed in `tests/lru.rs:90`). Syntax: `#[salsa::tracked(lru = N)]` per-query macro option + `set_lru_capacity(&mut db, N)` runtime API. Locked caps: `parse_query` lru=128, `module_signatures_query` lru=128, `check_query` lru=64, `codegen_query` lru=32 (codegen is heaviest; smaller cap acceptable). **User-facing tuning surface**: ONE env var `YNZ_WATCH_LRU_SCALE` (default 1.0) — multiplies all four defaults proportionally. ADVANCED escape hatches `YNZ_WATCH_LRU_PARSE`, `YNZ_WATCH_LRU_SIG`, `YNZ_WATCH_LRU_CHECK`, `YNZ_WATCH_LRU_CODEGEN` override individual caps if the scale multiplier isn't fine-grained enough. Per plan-review Round 2 ergonomics concern: most users will never touch these; SCALE is the primary knob.
  - **Layer 2 — Periodic DB rebuild**: drop + recreate `CompilerDb` every N=500 rebuilds OR T=4h elapsed (whichever first). Time-source = `std::time::Instant::now()` (monotonic; immune to clock skew). State persistence across rebuild: `WatchDb` holds a shadow `HashMap<PathBuf, String>` outside salsa (see Research Findings "WatchDb shadow state"). On rebuild_db(): drop salsa DB; create `Default`; iterate shadow map; populate new DB's `SourceFile` inputs from shadow map. ZERO source-state loss across rebuild.
  - **Layer 3 — RSS polling via `memory-stats` crate** (LOCKED — version 1.2; cross-platform: Linux `/proc/self/status` + macOS `task_info` + Windows `GetProcessMemoryInfo`). Sample after every rebuild. Soft warn (single message + JSON `MemoryWarning` event) at default 1024MB; hard stop (exit code 2 + JSON `MemoryStop` event) at default 4096MB. Tunable via env vars `YNZ_WATCH_MAX_RSS_MB` + `YNZ_WATCH_RSS_WARN_MB`. Warning rate-limited to 1-per-60s.
- **File watching via `notify = "8.2"` + `notify-debouncer-mini = "0.7"` LOCKED** (verified current stable 2026-05-20 via crates.io API; 9.0.0-rc.* family rejected because still rc). Cross-platform: Linux inotify / macOS FSEvents / Windows ReadDirectoryChangesW. Debouncer ONLY (no second dedup layer in event_loop — single source of truth).
- **No new language features** — M4 is pure tooling. Zero new tokens, zero new typeck/codegen behavior. `ynz build` / `ynz run` / `ynz fmt` behavior byte-identical to pre-M4 for every existing fixture.
- **Symlinks: follow** (consistent with M3 fmt). Watching the symlink path resolves to the target; saves to the underlying file trigger events. **Symlink target swap mid-watch**: if user runs `ln -sfn newfile foo.ynz`, the next file event watch receives is whatever `notify` delivers — we do NOT special-case symlink swaps. Documented in `design/watch.md`: "If you swap a symlink's target while watching, restart `ynz watch` for guaranteed correct behavior."
- **File deletion while watched**: if a `.ynz` file disappears mid-watch, watch logs a single warning line (`"path/to/foo.ynz vanished; watch continues, will re-pick-up on re-creation"`) + emits JSON `file-removed` event. Watch does NOT crash; the shadow source state retains the last-known text (so next rebuild includes the file's pre-deletion contents until it returns or watch restarts). The lockout against silent-zero-rebuilds: a project-mode rebuild on a partly-deleted project still uses the shadow state for the missing file.
- **Child process group**: child spawned in its own process group via `setpgid(0, 0)` (Unix) so SIGTERM/SIGKILL hit the whole tree, NOT just PID 1 (prevents `program &` double-fork from leaving zombies). Cross-platform: Unix uses `nix::unistd::setsid()` (LOCKED dep — see below); Windows uses `CREATE_NEW_PROCESS_GROUP` flag.
- **SIGTERM via `nix = "0.31"` LOCKED**: `child.kill()` from stdlib sends SIGKILL on Unix (no grace; can leave terminal in raw mode if child uses raw input). `nix` crate provides `nix::sys::signal::killpg(pgid, Signal::SIGTERM)` for graceful 2s shutdown, then stdlib SIGKILL fallback. Dep weight ~50KB compile; established crate. Cross-platform: Windows path uses `child.kill()` (TerminateProcess) directly — no graceful semantics on Windows (Windows console programs rarely have terminal-state cleanup needs).
- **Clock-skew immune**: every time threshold check uses `std::time::Instant::now()` (monotonic clock); NEVER `SystemTime::now()`. The 4h time-based DB-rebuild trigger is monotonic; system clock jumps don't fire spurious rebuilds.
- **Project mode**: `ynz watch` with no args = watch the cwd's `yinz.toml` project (matches `ynz build` / `ynz run` semantics). `ynz watch foo.ynz` = single-file mode. `ynz watch ./path/to/proj/` = explicit project root.
- **Initial build on start**: when watch boots, run one rebuild+run pass before waiting for events. User sees immediate compile status; doesn't have to save once to get the first build.
- **Ctrl+C clean shutdown**: SIGINT → kill child process (SIGTERM, then SIGKILL after 2s grace) → drop salsa DB → exit 0. No zombie processes.
- **stdin/stdout/stderr piped to child**: interactive programs (`terminal.readLine()` style) work under watch. Child's stdout/stderr stream live to the terminal between watch status lines.
- **Status-line interleaving with child stdout** (per plan-review Round 2 adversarial): when child writes continuously to stdout, the watch status line could corrupt mid-print. Locked behavior: watch flushes its status line BEFORE child stdout begins streaming, AND on next rebuild prints `\n` newline + clear sequence (if --no-clear off) before resuming status. No interleaving in normal terminal output. ANSI cursor positioning NOT used for status (terminals without ANSI fall back gracefully). Phase 3 integration test covers: child prints 1KB/sec; trigger 3 rebuilds; assert no garbled status line via stdout pattern matching.
- **`--json` schema versioning HONEST PRE-v0.2.0**: schema is UNSTABLE between intermediate milestones (v0.2.0-m4 → v0.2.0-m5 may add/remove/rename event fields; documented in CHANGELOG per change). Consumers MUST pin to a specific Yinz binary version (not a schema version field). Schema becomes stable + semver-bound at v0.2.0 final tag. Every event includes `"schema_version": "v0.2-m4-unstable"` so consumers parsing pre-v0.2.0 output get a loud signal in their logs that the schema is mutable. POST-v0.2.0: schema_version drops the `-unstable` suffix; semver applies to schema fields.
- **`--json` timestamp format LOCKED — RFC 3339 UTC with milliseconds**: e.g., `"timestamp": "2026-05-20T14:30:00.123Z"`. Locale-independent; downstream timezone-aware parsers handle naturally. Phase 4 integration test asserts the literal format via regex `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$`. Encoded via `chrono` crate (workspace dep added Phase 4 if not already present, with `serde` feature).
- **`--json` EPIPE handling LOCKED**: if downstream consumer pipe closes (e.g., `ynz watch --json | jq .` and jq exits), watch detects EPIPE on next write, emits `WatchShutdown { reason: "pipe-closed" }` to stderr (last-ditch), drops child, exits code 0. NOT a crash; clean termination.
- **Tempdir-disk-full handling LOCKED**: codegen step writes to per-pid tempdir (`$TMPDIR/ynz-watch-<pid>-<seq>/`). If write fails (disk full, permissions): codegen step returns error; emit JSON `BuildEnd { outcome: "errors" }` (NOT "ok"); status line shows "✗ build failed (codegen write error: ...)"; NO child spawn; NO ChildSpawn event in --json. Event ordering: BuildStart → BuildEnd("errors") → (no ChildSpawn). Consumers can rely on "no ChildSpawn after BuildEnd('errors')" invariant.
- **RSS poll failure (zero or error)**: if `memory-stats::memory_stats()` returns `None` or 0 unexpectedly: log a one-time stderr warning ("memory polling unavailable; hard-stop disabled this session"), set an internal flag, emit JSON `MemoryUnavailable` event. Watch continues without the hard-stop safety net. Phase 5 test injects a None RSS reading and asserts the warning fires + watch continues.
- **Save during DB rebuild**: file events arriving while `db.rebuild_db()` is dropping+recreating get queued in the event channel (single-event-at-a-time invariant holds). After rebuild completes, the queued event is processed against the fresh DB; no events lost, no crashes against stale handles.
- **Zero config**: no `.ynzwatch.toml`. Env vars only for memory tuning (`YNZ_WATCH_REBUILD_AFTER`, `YNZ_WATCH_MAX_RSS_MB`, `YNZ_WATCH_DEBOUNCE_MS`) — documented in `--help` and `design/watch.md` but invisible by default.
- **All compile errors continue WHAT/WHAT-INSTEAD/WHY format** — watch uses the existing `ynz-diagnostics` rendering. New error paths (file-watcher infra errors, child-process spawn errors) follow the same constructor.
- **Existing 1143+ tests must still pass.** New tests added; no existing tests weakened.

**Out of M4 scope (deferred — see Deferrals at end)**:
- LSP `textDocument/diagnostic` pull-mode integration with watch state — that's a v0.2-M5 enhancement (and only if needed).
- Sharing actual code between `ynz-lsp` and `ynz-watch` — both hold a `CompilerDb`, but their daemon loops are different shapes. Extracting common scaffolding deferred to v0.3+ if a real reuse case emerges; M4 builds standalone.
- Watching `yinz.toml` itself for changes (package add/remove triggering full reload) — v0.5 package-manager milestone.
- Distributed / network file watching (Docker volumes, NFS shares, SSHFS) — out of scope; `notify` falls back to polling for those (slow but functional).
- "Pause watch" / "trigger manual rebuild" interactive commands (press 'r' to rebuild) — possible follow-up; M4 ships a non-interactive loop only.
- `ynz watch --release` mode (cross-reference: `--release` flag itself ships post-v0.1, currently TBD per `design/mvp-scope.md`) — defer until `--release` exists.

**Success criteria**:
- `ynz watch examples/pirates-roster/entrypoint.ynz` builds the file, runs it, prints output, waits. On next save: rebuilds, kills old child, runs new child. Cycle stays sub-second on warm cache for single-file edits.
- `ynz watch --check examples/pirates-roster/entrypoint.ynz` builds on save; does NOT execute the program; prints "✓ build passed" or diagnostic output.
- `ynz watch --json examples/pirates-roster/entrypoint.ynz` emits one NDJSON event per line on stdout: `build-start`, `build-end`, `diagnostic`, `child-spawn`, `child-exit`, `memory-warning`. Schema stable; documented.
- `ynz watch ./examples/pirates-roster/` builds + runs the project's entrypoint. Saves to any `.ynz` file under the project trigger a rebuild.
- After 10,000 simulated rebuilds (synthetic test), process RSS stays bounded under 1GB. After 24h continuous operation (manual smoke test), watch self-recovers via the periodic DB rebuild without crashing.
- Ctrl+C exits cleanly: child killed within 2s, no zombies.
- `cargo test --workspace` passes (1143+ existing tests + new M4 tests).
- Tag cut: `v0.2.0-m4` (intermediate; v0.2.0 final ships at v0.2-M5).

## Research Findings

**Driver subcommand layout (verified 2026-05-20 against `crates/ynz-driver/src/main.rs`)**:
- `Cli` is a clap `Parser`; `Command` enum holds `Build`, `Run`, `Fmt` variants. M4 adds `Watch` variant.
- Exit codes are constants: `EXIT_OK = 0`, `EXIT_COMPILE_ERROR = 1`, `EXIT_INFRA_ERROR = 2`. Watch follows the same scheme: `0` on clean exit (Ctrl+C with no pending build), `1` on first-build compile failure if user passed `--check --exit-on-first-error` (locked Phase 4), `2` on infra errors (can't read project, file watcher init failed, OOM hard-stop reached).
- `crates/ynz-driver/src/build.rs` + `run.rs` + `fmt.rs` are per-subcommand modules. M4 adds either a `watch.rs` module OR (locked Phase 0) a dedicated `crates/ynz-watch/` crate.

**Crate location decision (locked this session — own crate `crates/ynz-watch/`)**:
- M2's `ynz-lsp` and M3's `ynz-fmt` both got their own crates. Watch holds substantial state (salsa DB, file watcher, child process handle, memory poller, --json emitter) and benefits from the same separation.
- Driver depends on `ynz-watch`; the subcommand handler in `crates/ynz-driver/src/watch.rs` is a thin shim that calls `ynz_watch::run(config)`.
- Tests live in `crates/ynz-watch/tests/` — integration tests can spawn `ynz watch` as a subprocess (`assert_cmd` is already a workspace dep from M3).

**Salsa daemon pattern (verified 2026-05-20 against `crates/ynz-parser/src/db.rs:1-67`)**:
- `CompilerDb` is `Default`; constructed fresh, then populated by `set_source_text(path, text)` for each file. The LSP creates exactly one DB at startup (verified per `.claude/plans/done/v0-2-m2-lsp-thin-slice.md` Research Findings) and mutates inputs on `didChange`. Watch follows the same pattern.
- `#[salsa::input] SourceFile { path: String, text: String }` is the write surface. On file change: read the new file content, call `source_file.set_text(&mut db, new_text).to(new_text)`; salsa invalidates `parse_query`, `check_query`, `codegen_query` automatically; next query call re-computes only the changed dependents.
- Watch runs `check_query` + (on PASS) `codegen_query` to get the LLVM IR / object file. The existing `crates/ynz-driver/src/build.rs` codepath is reused — watch calls the same internal `build_one(...)` function the `Build` subcommand uses, just with a pre-existing DB instead of a fresh one.

**Salsa LRU support (VERIFIED 2026-05-20 during plan-review round 1)**:
- Salsa 0.26.2 EXPOSES LRU eviction policy at `src/function/eviction/lru.rs`. Trait `EvictionPolicy` + `HasCapacity` are the salsa-internal types; macro option `lru = N` on `#[salsa::tracked]` wires the cap. Runtime API `set_lru_capacity(&mut db, N)` regenerated by the macro for each tracked function.
- M4 LOCKED caps: `parse_query` lru=128, `module_signatures_query` lru=128, `check_query` lru=64, `codegen_query` lru=32. Rationale: parse + signatures cheap to recompute, can keep more; check moderate; codegen heaviest, smallest cap. Numbers are calibrated against the synthetic 10k-rebuild test (Phase 5) — empirically tunable via env var if real-world projects need larger caps.
- Phase 5 step 1 verifies the macro option compiles + the runtime setter works against the existing salsa 0.26 queries. No "if not, fall back" branch — the API exists and we use it.

**WatchDb shadow state (CRITICAL — required for Layer 2 DB rebuild)**:
- `WatchDb` holds TWO state stores: (a) the `CompilerDb` (which lives inside salsa and is dropped+recreated periodically), and (b) `sources: HashMap<PathBuf, String>` (shadow store, lives in `WatchDb` permanently across DB rebuilds).
- On file event: `WatchDb::update_source(path, text)` updates BOTH the shadow `sources` map AND the salsa `SourceFile.text` input. Order: shadow first (so a panic in the middle leaves shadow consistent with last-known content), then salsa.
- On `WatchDb::rebuild_db()`: drop salsa DB; create `CompilerDb::default()`; iterate the shadow `sources` map; for each `(path, text)` call `db.set_source_text(path, text.clone())`. Result: fresh DB with identical input state.
- This eliminates the silent-bug class where DB rebuild empties inputs and "compiles cleanly" with zero diagnostics against an empty source. The shadow IS the source of truth; salsa is a derived cache.
- Memory cost: shadow stores raw source text once per file, NOT once per query result. For a 10-file/5kLOC project, shadow is ~50KB — negligible vs salsa's MB-scale per-query cache. Worth the cost for the correctness guarantee.

**File watching via `notify = "8.2"` + `notify-debouncer-mini = "0.7"` (LOCKED)**:
- Versions verified 2026-05-20 via crates.io API: `notify` stable major = 8 (8.2.0 current); 9.0.0-rc.4 exists but rejected (still rc). `notify-debouncer-mini` stable major = 0 (0.7.0 current; compatible with notify 8).
- Cross-platform: Linux inotify / macOS FSEvents / Windows ReadDirectoryChangesW.
- Editor saves generate multiple raw events (write-tempfile, rename, modify-attributes); `notify-debouncer-mini` coalesces within a configurable window. M4 uses ONLY the debouncer for coalescing — no second dedup layer in event_loop (per plan-reviewer concern; single source of truth).
- Debounce window: 100ms default (overridable via `YNZ_WATCH_DEBOUNCE_MS` env var). Editor save sequences typically settle within 50ms; 100ms is conservative without feeling laggy. Matches `cargo-watch` default.

**Cross-platform RSS via `memory-stats = "1.2"` (LOCKED)**:
- `memory-stats` crate provides `memory_stats() -> Option<MemoryStats>` returning `physical_mem: usize` (RSS in BYTES — unit-consistent across all platforms). Linux uses `/proc/self/status`; macOS uses mach `task_info(MACH_TASK_BASIC_INFO)`; Windows uses `GetProcessMemoryInfo`. Eliminates per-OS code from the M4 implementation.
- `Option::None` return = polling unavailable on this platform (rare but possible — e.g., chroot without /proc); see Constraints "RSS poll failure" handling above.
- No `mach2` / `libc` / `windows` direct deps needed. Smaller M4 surface; one crate, one API.

**Child process control via `nix = "0.31"` (Unix) + stdlib (Windows) (LOCKED)**:
- Unix: `nix::sys::signal::killpg(pgid, Signal::SIGTERM)` for graceful, then stdlib `Child::kill()` (SIGKILL) for force. 2s grace period between.
- Process group via `nix::unistd::setsid()` called from the pre-exec hook (`std::os::unix::process::CommandExt::pre_exec`) — child becomes its own session leader; SIGTERM hits the entire process group, catching double-forks.
- Windows: `Child::kill()` (TerminateProcess) directly; no graceful semantics. `CREATE_NEW_PROCESS_GROUP` flag set via `std::os::windows::process::CommandExt::creation_flags` if the user's program double-forks.
- `nix` dep weight: ~50KB compile-time, ~30KB binary; well-established (rust-analyzer, ripgrep, fd, others use it).

**Child process lifecycle (per LOCKED deps above)**:
- Build + run flow: build succeeds → spawn binary with stdin/stdout/stderr inherited + process-group set → hold `Child` handle in `WatchState`.
- On next rebuild OR Ctrl+C: SIGTERM the process group → wait 2s (poll `child.try_wait()` every 50ms) → SIGKILL via `child.kill()` if still alive.
- Stdin inheritance means interactive programs (e.g., `terminal.readLine()` from M7) work under watch. Stdout/stderr inheritance means the child's output streams live to the user's terminal between watch status lines.
- Tempdir for compiled binary: `$TMPDIR/ynz-watch-<pid>-<seq>/` (Phase 2 creates; Phase 3 cleans up old per cycle; Drop impl cleans up all on watch exit).

**--json event schema (LOCKED for v0.2-m4-unstable)**:
```jsonc
// NDJSON — one event per line on stdout. Every event includes "schema_version": "v0.2-m4-unstable"
// Timestamps: RFC 3339 UTC with milliseconds — regex /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/
{"type": "watch-ready",    "timestamp": "2026-05-20T14:30:00.123Z", "schema_version": "v0.2-m4-unstable", "watching": ["path/to/file.ynz", "..."]}
{"type": "build-start",    "timestamp": "...", "schema_version": "...", "file": "..."}
{"type": "build-end",      "timestamp": "...", "schema_version": "...", "file": "...", "outcome": "ok" | "errors", "duration_ms": 250}
{"type": "diagnostic",     "timestamp": "...", "schema_version": "...", "file": "...",
                           "severity": "error" | "warning" | "suggestion",
                           "span": {"start": 123, "end": 145},
                           "what": "...", "what_instead": "...", "why": "..."}
{"type": "child-spawn",    "timestamp": "...", "schema_version": "...", "pid": 12345}
{"type": "child-exit",     "timestamp": "...", "schema_version": "...", "pid": 12345, "exit_code": 0}
{"type": "file-removed",   "timestamp": "...", "schema_version": "...", "file": "..."}    // emitted when a watched .ynz file vanishes
{"type": "memory-warning", "timestamp": "...", "schema_version": "...", "rss_mb": 1024, "threshold_mb": 1024}
{"type": "memory-stop",    "timestamp": "...", "schema_version": "...", "rss_mb": 4096, "threshold_mb": 4096}
{"type": "memory-unavailable", "timestamp": "...", "schema_version": "...", "reason": "polling unavailable on this platform"}
{"type": "watch-shutdown", "timestamp": "...", "schema_version": "...", "reason": "ctrl-c" | "fatal" | "oom" | "pipe-closed"}
```
Schema stability: `"v0.2-m4-unstable"` suffix announces the pre-v0.2.0 unstable status. Consumers pin to a specific Yinz binary version (e.g., `0.2.0-m4`); schema may change between intermediate milestones, documented in CHANGELOG. POST-v0.2.0 final: suffix drops to `"v0.2"`; semver applies to field additions/removals.

Event-ordering invariants (consumers can rely on):
- Every `build-start` precedes exactly one `build-end` for the same file (atomic per cycle).
- `child-spawn` ONLY appears after `build-end { outcome: "ok" }` (NEVER after `outcome: "errors"`).
- `child-exit` follows `child-spawn` (eventually; may be hours later for long-running programs).
- `memory-warning` precedes any `memory-stop`; `memory-stop` precedes `watch-shutdown { reason: "oom" }`.
- `watch-ready` is the first event; `watch-shutdown` is the last.

**Branching/PR sizing** (per `~/.claude/memory/branching.md`): each phase = one branch off `main`, one PR via `/pr`. Soft target ~500 lines/PR. Phases 1+2 (file watcher + salsa loop) and Phase 3 (run lifecycle) are the heaviest. Phase 0 (scaffolding) and Phase 4 (--json) are mid-size. Phase 5 (memory) is small but precise. Phase 6 is verification + tag.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Salsa 0.26 LRU API absent or unstable; periodic-rebuild has subtle bugs that compound over hours | Medium | High | Phase 5 first step inspects salsa 0.26 source/docs and locks LRU strategy. Long-running test (10k synthetic rebuild cycles) runs in CI on every PR touching Phase 5 code. If LRU absent, Layer 2 (periodic rebuild) + Layer 3 (RSS hard stop) carry full load — explicitly designed redundant. |
| File-event coalescing wrong on macOS — atomic-write editor saves trigger 3+ rebuilds | Medium | Medium | `notify-debouncer-mini` with 100ms window is industry-standard (cargo-watch uses similar). Phase 1 integration test simulates VSCode-style atomic-write (write tempfile + rename) on Linux + macOS containers; asserts ≤1 rebuild per save. CI runs both platforms. |
| Child process kill race — SIGTERM sent, child ignores, SIGKILL fires mid-stdin-write, terminal state corrupted | Low | Medium | 2s SIGTERM grace; SIGKILL fallback. Watch resets terminal via `\x1bc` or similar on every cycle if `--no-clear` not set. Manual test on M7's interactive `examples/pirates-roster/entrypoint.ynz` if it has any `terminal.readLine()` calls (likely doesn't at M3 — confirmed). |
| Child program takes >2s to terminate; user's save events queue up indefinitely | Medium | Medium | Locked design: watch processes one event at a time. Events arriving during a build/run cycle are coalesced into "one pending rebuild" — multiple saves during one cycle collapse to a single next rebuild. Documented in `design/watch.md` + `--help`. |
| `--json` schema cracks between M4 ship and v0.2.0 final — automation tools break | Low | Medium | Schema includes `schema_version` field. Consumers pin a version. Schema changes between intermediate tags (v0.2.0-m4 → v0.2.0-m5) are allowed per pre-v1.0 policy (see `design/versioning.md`); document changes in CHANGELOG. After v0.2.0, semver applies. |
| Long-session memory growth ships despite multi-layer defense — user's watch process slowly OOMs | Low | High | 10k-cycle synthetic test (Phase 5) catches steady-state leaks. Manual 24h smoke test catches periodic-rebuild bugs (e.g., rebuild fires correctly but doesn't actually free memory). RSS hard-stop at 4GB is the safety net — at worst user sees friendly stop message + restart hint. |
| `notify` crate's polling fallback (NFS, Docker volumes) makes "sub-second" target laughable on certain setups | Medium | Low | Detect polling-mode at boot (`notify` API exposes); print warning: "ynz watch is using filesystem polling on this mount; rebuild may lag by up to N seconds." Users see the warning, understand, OK to ship. Documented in `design/watch.md`. |
| Ctrl+C handler races with rebuild-in-progress — child orphaned, zombie process left behind | Low | Medium | Signal handler sets shutdown flag; main loop checks flag at every event-loop iteration AND at every salsa-query call. Child kill happens unconditionally in the Drop impl on the watch state struct. Integration test: spawn watch, send SIGINT, assert no zombie via `ps`. |
| (Removed per plan-review: "initial-build-on-start" semantic is LOCKED behavior, not a risk. Documented in Constraints as: watch runs one rebuild+run pass on boot before entering event loop.) |
| `--json` output is interleaved with stderr (panic, oom-killer message) breaking line-oriented parsers | Low | Low | `--json` only structures stdout; stderr is unstructured. Document this in schema doc: "parse stdout as NDJSON; stderr is human-readable diagnostic for the watch process itself, not the user's program." Schema spec is explicit. |
| Cross-platform child-process kill semantics inconsistent — SIGTERM doesn't exist on Windows | Low | Medium | Phase 3 research: on Windows, `child.kill()` from stdlib sends WM_CLOSE then TerminateProcess. Grace period semantics differ; Windows path documented separately. M4 ships Linux + macOS first-class; Windows tested against fixtures but accepted as best-effort (Yinz ships Linux + macOS primary per all existing `ynz` cross-platform notes). |
| Project mode walks all of `examples/` if user runs from repo root — watches 100s of files needlessly | Low | Low | Project mode requires `yinz.toml` at the watch root. Without `yinz.toml`, watch errors: "ynz watch . requires a yinz.toml project; pass a single .ynz file instead." Mirrors `ynz build` behavior. Documented. |
| Watch holding compiled binary in memory blocks subsequent rebuilds if linker can't overwrite | Low | Medium | Build output goes to a unique tempfile per rebuild (`$TMPDIR/ynz-watch-<pid>-<seq>/`); child runs from that tempfile; on next rebuild, old tempfile is cleaned up on next iteration (or in Drop). Avoids "binary is busy" lock contention on Windows. |
| Reading `/proc/self/status` on non-Linux returns garbage; RSS poll reports 0 → hard stop never triggers | Low | High | Phase 5 has per-platform RSS implementation with unit tests on each. CI matrix includes Linux + macOS. Windows path either ships (preferred) or errors at boot: "memory-RSS polling not supported on Windows; --max-rss has no effect." Documented. |
| Plan-invariants rule violated by missing 7-subsection block | Low | Low | Plan structure below explicitly contains the 7-subsection `## Invariants This Milestone Must Preserve` block. Bouncer entries 1, 3, 4 enforce. |
| Feature registry entry forgotten for `--json` mode (it's a tooling feature) | Low | Low | Phase 0 acceptance criteria includes registering `ynz-watch-json` as a TOOLING feature in `registry/features.toml` — confirmed presence via Phase 0 build-output. `--json` becomes a stable tracked surface from M4 onward. Schema versioning lives in `design/watch.md` referenced from the registry entry. |
| Cross-platform symlink follow semantics differ — M3 fmt and M4 watch diverge | Low | Low | Locked: both follow symlinks. M4 reads file content through symlink resolution (same as `std::fs::read_to_string` default). File watcher watches the resolved target path (notify follows symlinks by default). Documented in `design/watch.md` cross-referencing M3's locked behavior. |

## Questions

None outstanding. All four open architectural questions answered this planning session:

1. `--json` mode: **Ship in M4** (text + --json both first-class).
2. Architecture: **Daemon LOCKED** (no spike). Memory mitigations: multi-layer defense (LRU verified + periodic rebuild + RSS poll + hard stop).
3. Default behavior: **Build + run** (mirrors `ynz run`). `--check` flag = build only.
4. Output style: **Clear by default + `--no-clear` flag**.

Two architectural sub-questions decided locally (no Patrick input needed):

5. Watch lives in **own crate `crates/ynz-watch/`** (mirrors M2's `ynz-lsp`, M3's `ynz-fmt`).
6. Cross-platform RSS polling: roll-our-own per-OS module (Phase 5 confirms vs `memory-stats` crate as fallback).

## Risk Assessment & Rollout Strategy

**Risk level: MEDIUM**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler tooling |
| Touches auth/permissions | No | No auth |
| Raw SQL / literals | No | No DB |
| Modifies existing data | No | Watch writes only tempfiles for compiled binaries; cleans up after itself |
| Third-party integration | Yes | `notify` crate + `notify-debouncer-mini` are new workspace deps |
| Changes existing endpoints | N/A | New CLI subcommand only; build/run/fmt behaviors unchanged |
| New feature with no equivalent | Yes | First watch command in Yinz |

**Mitigations applied**:
- Locked architecture (daemon) + locked third-party (`notify` + `notify-debouncer-mini`, both established) → no architecture-spike scope creep → MEDIUM stays MEDIUM
- Multi-layered memory defense + 10k-cycle synthetic test → catches long-session leaks → MEDIUM → LOW for OOM risk
- File-event coalescing tested cross-platform (Linux + macOS) → MEDIUM → LOW for editor-save races
- Child-process kill grace period + integration tests → MEDIUM → LOW for zombie/orphan risk
- `--json` schema versioning → MEDIUM → LOW for automation-consumer churn

**Rollout plan** (Yinz convention: trunk-based, no production rollout; "rollout" = milestone tag):
1. Each phase: branch from main, PR via `/pr`, code-reviewer agent at phase boundary, merge to main on PASS
2. Phase 6 (final verification + tag): cut `v0.2.0-m4` tag after full test sweep + memory stress test + cross-platform smoke
3. v0.2.0 final tag waits for v0.2-M5 per roadmap

## Invariants This Milestone Must Preserve

### Safety
- All 1143+ existing tests pass post-milestone (`cargo test --workspace`)
- `ynz build` / `ynz run` / `ynz fmt` exit codes, stdout, stderr byte-identical to pre-M4 for every existing fixture
- New crate `ynz-watch` does NOT alter any existing crate's public API (additive only)
- No previously-valid `.ynz` program becomes rejected by the compiler after M4 ships (compiler is unchanged)
- No previously-rejected `.ynz` program becomes accepted (same)
- `ynz watch` NEVER writes to source files. Only writes: per-pid tempdir (`$TMPDIR/ynz-watch-<pid>-<seq>/`) for compiled binaries (cleaned per cycle + on shutdown via Drop); no source mutation under any flag combination
- Ctrl+C ALWAYS kills child process (whole process group via SIGTERM → SIGKILL) before watch exits (verified by integration test asserting no zombie via `ps` post-shutdown). Process-group kill catches double-forked children.
- Watch process's RSS NEVER exceeds the configured ceiling (default 4GB): hard-stop with friendly WHAT/WHAT-INSTEAD/WHY message before OOM-killer fires. If RSS poll itself fails, watch logs `MemoryUnavailable` event and continues without the hard-stop net (rare; documented degraded mode).
- Single-event-at-a-time invariant: watch processes one build/run cycle to completion before starting next; rapid saves coalesce into "one pending rebuild" (no concurrent builds against the same DB). Events arriving during `db.rebuild_db()` (Layer 2) queue in the event channel; processed against the fresh DB.
- **WatchDb shadow source state**: source text stored in `HashMap<PathBuf, String>` outside salsa; DB rebuild (Layer 2) repopulates salsa inputs from shadow. Shadow IS source of truth; salsa is derived cache. Eliminates the silent-empty-DB failure mode.
- **Monotonic time only**: all time thresholds (4h DB rebuild, 60s warning rate-limit, 2s SIGTERM grace) use `std::time::Instant::now()`. NEVER `SystemTime::now()`. Immune to NTP / clock-skew.
- `ynz-watch` does NOT depend on `ynz-lsp` (parallel daemons; no code shared)
- `ynz-watch` does NOT depend on `ynz-fmt` (formatting is not a watch concern)
- **Windows is best-effort for M4**: RSS polling via `memory-stats` (works); child process group via `CREATE_NEW_PROCESS_GROUP` (works); SIGTERM-vs-SIGKILL graceful semantics NOT applicable on Windows (TerminateProcess only). **Documented Windows limitation** (per plan-review Round 2 concern): On Windows, Ctrl+C immediately terminates the child via TerminateProcess; in-flight writes (file handles, partial stdout flushes) may be lost. This is fundamental to Windows' lack of POSIX signals and not a watch-specific bug. `design/watch.md` notes this prominently in the "Cross-platform" section. Cross-platform tests run on Linux + macOS in CI; Windows tested manually by Patrick if/when relevant. Tracked in todos.md as `watch-windows-validation` (Phase 0 deferred entry).
- **`yinz.toml` edits during watch are IGNORED until restart** (per plan-review Round 2 adversarial): the `notify` watcher subscribes to `.ynz` files only; `yinz.toml` is read once at watch boot. Edits to `yinz.toml` (adding files, changing project config) do NOT trigger rebuild or re-discovery. Documented in `--help` + `design/watch.md`. Restart watch to pick up `yinz.toml` changes. Tracked in v0.5 package-manager milestone as `watch-yinz-toml-reload`.

### Performance

**Targets are HARD CEILINGS, not aspirational.** Phase 6 measurement is a BLOCK gate — exceeding any ceiling REQUIRES a profile + fix, not a budget raise.

**Canonical measurement protocol (LOCKED)**:
- **Hardware**: GitHub Actions `ubuntu-latest` runner (4-core x86_64, 16GB RAM, SSD). Local-dev measurements documented separately but only CI numbers are gates.
- **Build profile**: release (`cargo build --release`). Debug numbers tracked informationally but not gated.
- **Fixtures**:
  - `crates/ynz-watch/benches/fixtures/perf_500.ynz` — synthesized 500-line `.ynz` file with realistic distribution of shape declarations, functions, control flow (NEW file added in Phase 6).
  - `crates/ynz-watch/benches/fixtures/perf_5000.ynz` — synthesized 5000-line equivalent (NEW Phase 6).
  - `crates/ynz-watch/benches/fixtures/perf_project/` — 10-file yinz.toml project, ~5000 total LOC (NEW Phase 6).
- **Sample protocol**: 10 cold runs (fresh process) + 100 warm runs (same process, save-touch loop). Report median (p50) and p99 of warm runs only; cold runs gate cold-start ceiling.
- **Measurement source**: timestamps emitted by `--json` mode (BuildStart + BuildEnd) — leveraging Phase 4 work; no separate measurement harness.

**Hard ceilings (CI medians; ANY breach = BLOCK pending profile + fix, NEVER budget raise)**:
- **Cold start** (`ynz watch perf_500.ynz`, first build from empty cache): ≤ `ynz build --release perf_500.ynz` time + 200ms file-watcher init overhead. Measured via stopwatch from process spawn to first BuildEnd event.
- **Warm rebuild p50** (single-file edit, salsa cache warm; perf_500.ynz): ≤ 500ms wall-clock (per `design/compiler.md:146` sub-second target).
- **Warm rebuild p99**: ≤ 1500ms (3× p50; catches occasional GC pause / cache miss).
- **Warm rebuild p50** (perf_5000.ynz): ≤ 2000ms wall-clock.
- **Event-to-build-start latency p50**: ≤ 100ms debounce + ≤ 50ms event-handling overhead = ≤ 150ms. Measured via integration test capturing fs event time vs BuildStart event time.
- **Child process spawn overhead p50**: ≤ 50ms from BuildEnd("ok") to first byte of child stdout (measured against `perf_500.ynz` whose main prints "ready" immediately). Excludes Yinz program's own startup beyond `std::process::Command::spawn`.
- **--json output latency p50**: ≤ 10ms from internal trigger to stdout flush per event. Measured via internal instrumentation (track `Instant::now()` at event-emit call site vs after stdout.flush()).
- **Memory ceiling (steady-state)**: process RSS ≤ 1GB after 10,000 rebuilds against the perf_project (warmup baseline = RSS at iteration 500). RSS poll BLOCKS at 4GB hard-stop. Memory soft-warn (locked at 1GB) is informational, not a stop.

**Long-session stability test (Phase 5; CI mandatory)**:
- **Harness methodology (LOCKED per plan-review Round 2 concern)**: the 10k loop mutates source state DIRECTLY through the `WatchDb::update_source(path, text)` API — NO real filesystem events, NO notify watcher in the loop. Eliminates CI flakiness from filesystem timing. The integration tests in Phases 1-4 cover the real-file-event path with smaller event counts.
- Synthetic loop: call `update_source` against `perf_project/entrypoint.ynz` 10,000 times with small content variations. RSS sampled at iterations [500, 1000, 2000, 5000, 7500, 10000].
- **Pass conditions** (ALL must hold):
  - RSS at iteration 10000 ≤ 1.05× RSS at iteration 500 (post-warmup baseline). NOTE: tightened from initial "1.5× + 100MB" per plan-review.
  - Between consecutive samples: no >5% jump that isn't recovered (i.e., next-or-subsequent sample returns below the spike).
  - No crash, OOM, or panic during the loop.
  - `MemoryWarning` events emitted IF and only IF RSS exceeds threshold (verifies Layer 3 wiring).
  - Layer 2 periodic DB rebuild fires AT LEAST ONCE during the loop (verified by counting `db.rebuild_db()` invocations; expected at iteration 500 if N=500 default).
- Test marked `#[ignore]` for default runs (takes ~5min); CI runs with `--include-ignored` flag.

**Auto-promotion analysis** (per `.claude/rules/auto-promotion.md`):
- This milestone does NOT introduce any new language feature, stdlib type, or compiler codegen optimization.
- Watch is a pure orchestration layer over existing `ynz build` + `ynz run` codepaths; no codegen change.
- No codegen auto-promotion candidates. No new muted-hint domain (consumption deferred to v0.2-M5). No Tier 3 lint suggestion (lint tier ships in v0.4).
- The `--no-clear` flag is a user opt-OUT from a smart default (clear-screen); not an auto-promotion (the default isn't doing something the user could write explicitly — it's purely a UI choice).
- Explicitly considered, not forgotten.

### Teaching
- Watch's parse/typeck/codegen errors are reported using the EXISTING `ynz-diagnostics` machinery (WHAT/WHAT-INSTEAD/WHY format). When watch hits a compile error, it prints the same diagnostic `ynz build foo.ynz` would.
- New infra-level error paths (file-watcher init fail, no `yinz.toml` for project mode, child-spawn failure, RSS hard-stop) use the same `Diagnostic` constructor → WHAT/WHAT-INSTEAD/WHY enforced.
- Memory hard-stop message is teaching-friendly: "ynz watch hit 4GB memory; this is the safety stop. Run: ynz watch <args> to restart. If this happens frequently, set YNZ_WATCH_REBUILD_AFTER=200 (default 500) to rebuild the compiler state more often."
- NEW design doc: `design/watch.md` — architectural reference: daemon model, file-watcher integration, child process lifecycle, --json schema (with version field), memory-defense layers, debounce strategy, future-proofing for interactive commands (`r` to rebuild) if added v0.3+.
- UPDATE `design/compiler.md` watch section (currently lines 138-146 is a bare paragraph) — expand to reference `design/watch.md` for full architecture; keep the paragraph as a quick-start summary.
- No new `.claude/rules/` files needed — registry-consumer rule from M1 covers watch's registry consumption; no new project-rule surface.
- No new banned-jargon words slip into watch-emitted text — `tests/jargon_audit.rs` extended in Phase 6 to walk every string watch produces (status lines, --help text, error messages, --json event field names checked separately for stability).

### Runtime Dependencies
- `ynz-watch` crate runtime:
  - `ynz-driver` (NOT — driver depends on watch, not the other way around)
  - `ynz-parser` (internal — to run `parse_query` against the DB)
  - `ynz-ast` (internal — for AST node types if introspection needed)
  - `ynz-typeck` (internal — to run `check_query`)
  - `ynz-codegen` (internal — to run `codegen_query` and produce a binary)
  - `ynz-registry` (internal — keyword spellings used in diagnostic render)
  - `ynz-diagnostics` (internal — error rendering)
  - `salsa` (already workspace dep — long-lived DB)
  - **NEW** `notify` (file watching; pinned to stable major)
  - **NEW** `notify-debouncer-mini` (event coalescing)
  - **NEW** `serde` + `serde_json` (--json event serialization — `serde` likely already a transitive workspace dep via salsa/ariadne, confirmed Phase 0)
  - **POSSIBLE** `nix` (Unix SIGTERM/SIGKILL semantics) OR roll-our-own — decided Phase 3 research
  - **POSSIBLE** `memory-stats` (cross-platform RSS) OR per-OS roll-our-own — decided Phase 5 research
- `ynz-driver` runtime: gains a thin `watch` subcommand shim that calls `ynz_watch::run(config)`. Driver's existing deps unchanged.
- Compiler binary's runtime profile for `build` / `run` / `fmt`: **identical to pre-M4** (no new deps on those paths; monomorphization isolates).
- Build-time: `notify` + `notify-debouncer-mini` + their transitive deps add to first-build time. Estimated impact: <30s first-build slowdown; warm `cargo build --workspace` unaffected post-cache.

### Kernel-Mode Behavior
- `--kernel` build mode is unaffected. Watch is a developer-machine tool; it does not run in kernel-mode targets.
- The compiler binary's `--kernel` mode behavior on a `.ynz` file is byte-identical to pre-M4.
- No new compile-error path introduced for kernel-mode programs.
- `design/future/no-runtime-mode.md` cross-reference: same status as `ynz-lsp` (M2) and `ynz-fmt` (M3) — host-tool, not kernel-runtime.
- If a future user attempts `ynz watch --kernel foo.ynz`: watch passes through to the underlying `build` flow which already handles `--kernel`. No watch-specific kernel concern.

### Demo & Error Gallery

- `examples/pirates-roster/entrypoint.ynz`: ADD a top-of-file comment block: `// Watch this file with: ynz watch examples/pirates-roster/entrypoint.ynz — saves trigger rebuild + re-run. For build-only (no execute), pass --check.` No NEW Yinz language code added (M4 ships no new language features).
- **NEW dedicated watch demo** `examples/incline-watcher/`: a `yinz.toml`-rooted minimal project with one `entrypoint.ynz` that prints a SIMPLE message (e.g., `"watch demo, build #1"`) — NO counter file, NO sibling-state mutation (Safety invariant "ynz watch NEVER writes to source files" enforced; the demo MUST NOT violate it). The build number is hard-coded in source; Patrick changes it by editing the source line, which is the actual demo (the rebuild cycle). Top-of-file comment: `// Run: ynz watch examples/incline-watcher/ — edit the print message on the next line and save; watch rebuilds and re-executes within a second. Try --json for structured-event output, --check to skip the execute step, --no-clear to preserve scrollback.` This project IS canonical (covered by M3 formatter).
- `examples/primantis-orders/v0_2_m4_errors.ynz`: NEW file. Intentional triggers for every NEW error path watch introduces:
  - File watcher init failure (simulated via mock if needed in Phase 1)
  - `--check` AND `--run` both passed (mutually exclusive flag error)
  - No `yinz.toml` at watch root in project mode
  - Child process spawn failure (binary not executable simulated)
  - RSS hard-stop trigger (simulated by setting YNZ_WATCH_MAX_RSS_MB=1)
  - `--json` AND `--no-clear` both passed (no conflict — both can coexist; no error here; commented as such for completeness)
  - Each trigger has a `// WHY:` comment naming the diagnostic class (consistent with M3's `v0_2_m3_errors.ynz` precedent)
- `insta` stdout/stderr snapshots in Phase 6 for the `v0_2_m4_errors.ynz` CLI render
- Phase 4 includes adding `examples/incline-watcher/` to the project list `ynz fmt --all` walks (no special handling; standard `yinz.toml` project root)

### Feature Registry Entries

This milestone introduces ONE new SSOT registry entry:

- **NEW `[[deferred_tooling_feature]]`** entries (TWO total; ONE has been removed vs initial plan):
  - `ynz-watch-interactive-commands` (press 'r' to rebuild) — deferred to v0.3+; locked design pointer to `design/watch.md` future-proofing section
  - `ynz-watch-config-file` (`.ynzwatch.toml` opt-in) — REJECTED by Yinz zero-config ethos; entry marks the explicit rejection so a future contributor doesn't propose it without seeing the rationale
  - (REMOVED per plan-review concern: the `ynz-watch-tcp-pause-signal` placeholder was polluting the registry without representing a real deferred feature — only register deferrals that have an actual locked design + trigger.)
- **`--json` mode is NOT a deferred-tooling-feature entry**: ships in M4 (per Constraints). It IS noted in `design/watch.md` as a stable interface from M4 onward; schema version field `"v0.2-m4-unstable"` announces its pre-v0.2.0 status.
- **NEW `[[diagnostic_template]]`** entries (locked count: 5):
  - `watch-no-yinz-toml` — project-mode invocation without yinz.toml; WHAT/WHAT-INSTEAD/WHY filled
  - `watch-child-spawn-failed` — binary built but couldn't exec (permissions, tempdir issue); WHAT/WHAT-INSTEAD/WHY filled
  - `watch-fs-watcher-init-failed` — `notify` couldn't subscribe (file vanished pre-init, permissions); WHAT/WHAT-INSTEAD/WHY filled
  - `watch-rss-hard-stop` — RSS exceeded `YNZ_WATCH_MAX_RSS_MB`; WHAT/WHAT-INSTEAD/WHY explains restart, tuning env vars
  - `watch-memory-polling-unavailable` — `memory-stats` returned None (degraded mode); WHAT/WHAT-INSTEAD/WHY explains the safety implication
- **MODIFIED entries**: NONE.
- **Existing consumer adapters reused**: `keywords()`, `banned_jargon()` (used by jargon audit in Phase 6). No new adapter functions needed for M4.
- Explicitly considered per the v0.2-M2+ plan-invariants rule.

## Phase Execution Protocol

Each phase ends with an **Exit Sequence** block listing the actions to execute (persist plan state → invoke code-reviewer → handle verdict → prompt commit). Those instructions are commands, not a checklist to tick off.

**Final phase (Phase 6) additionally:**
- Verify ALL phases' acceptance-criteria and quality-gate checkboxes across the plan
- Invoke `code-reviewer` with the **cumulative plan diff**: `git diff <m3-tag>..HEAD`
- Flip `status: active` → `status: done` only after final PASS

## Phases

**Project Shipping Conventions** (per `/plan` Step 4a, detected from project):
- Per-phase ships via `/pr` (project has local `pr` skill at `.claude/skills/pr/`)
- Per-milestone ships via `/release` (project has local `release` skill at `.claude/skills/release/`)

**Sequencing note**: Phase 0 begins from `main` at the v0.2.0-m3 tag commit (M3 shipped 2026-05-20). Each subsequent phase branches from main as the previous merges.

---

### Phase 0: Doc lockdown + crate scaffolding (no behavior change)

**PR scope**: Land `design/watch.md`, expand `design/compiler.md` watch section, update `design/mvp-scope.md` v0.2-M4 entry, scaffold empty `crates/ynz-watch/` with `lib.rs` + module stubs + Cargo.toml entry, add a `watch` subcommand stub to `crates/ynz-driver/src/main.rs` (parses CLI args, prints "not yet implemented", exits 0). Register one new deferred-tooling-feature entry in `registry/features.toml`. No watch behavior. No driver behavior change for `build` / `run` / `fmt`.
**Branch**: `chore/v0-2-m4-doc-lockdown`
**Flag**: N/A
**Est. lines**: ~550 (design/watch.md ~250, design/compiler.md edit ~30, design/mvp-scope.md edit ~30, cargo updates ~30, scaffolding stubs ~80, driver subcommand stub ~70, registry entry + tests ~30, CLAUDE.md ~20, todos.md ~10)
**Ships via**: `/pr`

**Objective**: Lock the architectural decisions made this planning session into committed docs. Create the crate skeleton so Phase 1's file-watcher work has somewhere to land without tangling production paths. Register the --json deferral entries so they have a permanent home from M4 onward.

**Why this phase exists**: Doc-lockdown-first prevents Phase 1+ work from drifting from the locked architecture as implementation hits edges. Scaffolding-first lets Phase 1 focus purely on file-watching logic without also wiring up the workspace.

**Current-state anchors**:
- `Cargo.toml:18` — workspace version (`0.2.0-m3` as of 2026-05-20)
- `Cargo.toml:3-17` — workspace member list; M4 adds `ynz-watch`
- `design/mvp-scope.md` v0.2-M4 entry stub; needs expansion with locked decisions
- `design/compiler.md:138-146` — current bare watch paragraph; needs expansion or cross-reference to new `design/watch.md`
- `crates/ynz-driver/src/main.rs:39-130` — `Cli` and `Command` enums; M4 adds `Watch` variant
- `CLAUDE.md` Project Layout table — adds `crates/ynz-watch/` row
- `registry/features.toml` — accepts new `[[deferred_tooling_feature]]` entries

**Files (expected scope)**:
- NEW: `design/watch.md` — architectural reference doc covering: daemon model, file-watcher integration, child process lifecycle, --json schema (with `schema_version` field), memory-defense layers, debounce strategy, cross-platform notes, future-proofing section (interactive commands, config file rejection rationale)
- EDIT: `design/compiler.md` — replace lines 138-146 watch paragraph with a short quick-start summary + cross-reference to `design/watch.md`
- EDIT: `design/mvp-scope.md` v0.2-M4 entry: enumerate locked decisions (daemon architecture, --json in M4, build+run default, clear+--no-clear, memory layers)
- EDIT: `CLAUDE.md` — Project Layout table: add row for `crates/ynz-watch/` (purpose: "Watch daemon — long-running terminal command for rebuild-on-save + re-run; consumes salsa-backed compiler queries shared with the rest of the workspace")
- NEW: `crates/ynz-watch/Cargo.toml` — workspace=true edition/version/authors/license; deps on `ynz-parser`, `ynz-ast`, `ynz-typeck`, `ynz-codegen`, `ynz-registry`, `ynz-diagnostics`, `ynz-runtime`, `salsa`; placeholder for `notify` + `notify-debouncer-mini` + `serde_json` to be wired in Phase 1
- NEW: `crates/ynz-watch/src/lib.rs` — pub API stubs: `pub fn run(config: WatchConfig) -> i32 { eprintln!("ynz watch: not yet implemented"); 1 }` and the `WatchConfig` struct skeleton (path, check, json, no_clear, all four locked flag bools)
- NEW: `crates/ynz-watch/src/error.rs` — `WatchError` enum + `Result<T>` alias
- NEW: `crates/ynz-driver/src/watch.rs` — thin shim: parses CLI args into `WatchConfig`, calls `ynz_watch::run(config)`, returns exit code
- EDIT: `crates/ynz-driver/src/main.rs` — add `mod watch;` + `Watch` variant on `Command` enum with all flags + match arm calling `watch::watch(...)`
- EDIT: `crates/ynz-driver/Cargo.toml` — depend on `ynz-watch`
- EDIT: `Cargo.toml` — (a) add `crates/ynz-watch` to workspace members; (b) add `ynz-watch = { path = "crates/ynz-watch" }` to workspace deps; (c) ADD locked deps to `[workspace.dependencies]`: `notify = "8"`, `notify-debouncer-mini = "0.7"`, `memory-stats = "1.2"`, `nix = { version = "0.31", features = ["signal", "process"] }`, `ctrlc = "3"`, `chrono = { version = "0.4", features = ["serde"] }`
- EDIT: `registry/features.toml` — add `[[deferred_tooling_feature]]` entries for the items listed in "Feature Registry Entries" subsection above (interactive commands, config file rejection)
- EDIT: `.claude/todos.md` — ADD durable-home entries per `deferrals-must-be-tracked` rule:
  - `- [ ] **watch-interactive-commands** — press 'r' to rebuild, 'q' to quit, etc. in ynz watch. Deferred from v0.2-M4; not blocking ship. Pick up IF terminal-only users surface real demand. Locked design pointer in design/watch.md future-proofing section.`
  - `- [ ] **watch-lsp-shared-daemon** — investigate sharing the long-lived CompilerDb between ynz-watch and ynz-lsp. Deferred from v0.2-M4 (independent daemons OK for M4). Pick up IF v0.3 needs both running concurrently against the same project.`
  - `- [ ] **watch-windows-validation** — full Windows validation pass: RSS via memory-stats, child kill via TerminateProcess, process group via CREATE_NEW_PROCESS_GROUP. Implementation present from M4 but tested manually only. Pick up when Yinz formally supports Windows.`
  - `- [ ] **watch-json-schema-stabilize** — at v0.2.0 final tag, drop `-unstable` suffix from --json schema_version field; commit to semver-bound schema changes. Locked trigger: v0.2.0 release.`

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work. Document each deviation in the PR description; if it's its own concern, split.

**Steps**:
1. Write `design/watch.md` covering all sections listed in Files above. Algorithm: open file, write headings, fill each section with locked decisions from this plan's Constraints + Research Findings. Cross-reference `design/compiler.md`, `design/compiler-language.md`, `design/feature-registry.md`, roadmap.
2. Update `design/compiler.md` lines 138-146: replace with 4-line quick-start summary + cross-reference to `design/watch.md`.
3. Update `design/mvp-scope.md` v0.2-M4 entry: enumerate all locked decisions.
4. Update `CLAUDE.md` Project Layout: add row for `crates/ynz-watch/`.
5. Scaffold `crates/ynz-watch/`: Cargo.toml with workspace deps; src/lib.rs with `run(WatchConfig) -> i32` stub returning "not yet implemented" + exit 1; src/error.rs with WatchError enum.
6. Add `crates/ynz-watch` to root `Cargo.toml` workspace members + workspace deps; lock the SIX workspace deps listed in Files (versions pinned NOW — `notify = "8"`, `notify-debouncer-mini = "0.7"`, `memory-stats = "1.2"`, `nix = "0.31"` (signal+process features), `ctrlc = "3"`, `chrono = "0.4"` (serde feature)).
7. Add `Watch` variant to `Command` enum in `crates/ynz-driver/src/main.rs` with `file: Option<PathBuf>`, `check: bool`, `json: bool`, `no_clear: bool` flags + match arm calling `watch::watch(...)`. Lock flag mutual-exclusion: `--check` and `--json` can coexist; no other restrictions (locked here).
8. Create `crates/ynz-driver/src/watch.rs` with the thin shim handler.
9. Add `ynz-watch` to `crates/ynz-driver/Cargo.toml` deps.
10. Add TWO `[[deferred_tooling_feature]]` entries + FIVE `[[diagnostic_template]]` entries to `registry/features.toml` per the Feature Registry subsection. Each diagnostic template carries the WHAT/WHAT-INSTEAD/WHY fields (templates are skeletons; the live diagnostic emits them with phase-specific context).
11. Append the FOUR deferral entries to `.claude/todos.md` "Later" section verbatim per `deferrals-must-be-tracked` rule.
12. Run `cargo build --workspace` — confirms compilation with the placeholder `notify` deps. If notify versions cause issues, downgrade or pick stable Phase 0; Phase 1 finalizes.
13. Run `cargo test --workspace` — confirms no regressions (1143+ tests pass).
14. Run `./target/debug/ynz watch foo.ynz` — confirms stub prints message + exits 1.
15. Run `./target/debug/ynz build crates/ynz-driver/tests/fixtures/m3_fib.ynz` — confirms existing build path unchanged.

**Acceptance criteria** (observable conditions that define DONE — TRIMMED per plan-review):
- [x] `design/watch.md` exists and is substantive (>200 lines; covers all sections enumerated in Step 1)
- [x] `cargo build --workspace` succeeds with the new empty crate + workspace `notify = "8.2"` + `notify-debouncer-mini = "0.7"` + `memory-stats = "1.2"` + `nix = "0.31"` deps locked
- [x] `cargo test --workspace` passes (1143+ tests; new crate adds no breaking changes)
- [x] `./target/debug/ynz watch --help` prints help for the new subcommand with all four flags
- [x] `./target/debug/ynz watch foo.ynz` prints "not yet implemented", exits 1
- [x] `registry/features.toml` contains the TWO new `[[deferred_tooling_feature]]` entries + the FIVE new `[[diagnostic_template]]` entries (registry-consistency test still green)
- [x] `.claude/todos.md` "Later" section contains all four deferral entries verbatim (watch-interactive-commands, watch-lsp-shared-daemon, watch-windows-validation, watch-json-schema-stabilize)

**Quality gate** (observable facts to confirm — check BEFORE moving to next phase):
- [x] No `// TODO` / `// FIXME` / `// HACK` left in any new file
- [x] No new banned-jargon in user-facing prose (design/watch.md is for engineers — "infer" / "inference" OK per dual-audience rule; never in user-rendered text)
- [x] No `as any` / `#[allow(...)]` swallows
- [x] `design/watch.md` cross-references `design/compiler.md`, `design/compiler-language.md`, `design/feature-registry.md`, `design/lsp.md`, roadmap
- [x] No commented-out code; no orphan files
- [x] `cargo clippy --workspace -- -D warnings` passes

**Verification**:
- `cargo build --workspace 2>&1 | tail -5` — clean
- `cargo test --workspace 2>&1 | grep 'test result'` — all pass
- `./target/debug/ynz watch --help 2>&1` — help text shows all four flags + brief description
- `cat design/watch.md | wc -l` — substantive (>200 lines)

**Exit Sequence — RUN THESE STEPS:**

1. **Persist plan state.** Tick this phase's Acceptance + Quality Gate checkboxes; bump `last_updated:` to today.
2. **Invoke code-reviewer.** Use the Agent tool:
   ```
   Agent({ subagent_type: "code-reviewer", description: "Review Phase 0",
     prompt: "Review the diff for Phase 0 of plan at .claude/plans/active/v0-2-m4-watch.md against the phase's acceptance criteria, quality gate, rules, and laziness patterns. Remind on ~/.claude/rules/comments.md durability + Golden Rule 11 WHY-quality + Yinz vocabulary. Diff command: git diff <m3-tag>..HEAD. Output in your standard format." })
   ```
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
4. **Prompt user.** "Phase 0 done. Code-reviewer: PASS. Ready to commit and move to Phase 1?"
5. **Do NOT start Phase 1** until user confirms commit (per Patrick's `all-phases-then-review` rule: may complete all phases without per-phase approval IF Patrick explicitly authorized at session start; default = ask).

---

### Phase 1: File-watching plumbing + daemon scaffold (no salsa yet)

**PR scope**: Wire `notify = "8"` + `notify-debouncer-mini = "0.7"` (locked Phase 0) into `crates/ynz-watch/`. Build the event loop: receive events from the debouncer, log "would rebuild X" (no actual rebuild). Implement clear-screen logic + `--no-clear`. Implement file-removed detection (vanished file → log warning + emit JSON file-removed event later in Phase 4). Integration tests: touch file via tempdir fixture, assert event received within 200ms; delete file via tempdir fixture, assert "vanished" log line. NO salsa, NO actual compilation, NO child spawn.
**Branch**: `feat/v0-2-m4-file-watcher`
**Flag**: N/A
**Est. lines**: ~600 (notify wiring ~150, event loop ~150, clear-screen + flags ~80, integration tests ~150, version-lock research notes in design/watch.md ~30, error handling ~40)
**Ships via**: `/pr`

**Objective**: Get file events plumbed into the watch process. Validate cross-platform behavior (Linux + macOS) with editor-save simulation. Lock the `notify` version pinning. Build the event coalescing layer that downstream phases assume works.

**Why this phase exists**: File watching is the lowest-level layer of the daemon. Without solid event delivery + debounce, no amount of fancy salsa work above can compensate for missed events or rebuild storms.

**Current-state anchors**:
- `crates/ynz-watch/src/lib.rs` from Phase 0 (`run(WatchConfig) -> i32` stub)
- `crates/ynz-watch/Cargo.toml` from Phase 0 (deps locked; this phase finalizes notify versions)
- `Cargo.toml` workspace deps from Phase 0 (placeholder notify entries; this phase pins exact versions)

**Files (expected scope)**:
- NEW: `crates/ynz-watch/src/watcher.rs` — wraps `notify` + `notify-debouncer-mini`; exposes a `WatchEvents` iterator that yields debounced events
- NEW: `crates/ynz-watch/src/event_loop.rs` — main loop: receive events, log "would rebuild X" (no actual rebuild yet), handle Ctrl+C
- NEW: `crates/ynz-watch/src/ui.rs` — clear-screen logic (`\x1bc` or `ANSI clear` + position); `--no-clear` flag bypasses
- EDIT: `crates/ynz-watch/src/lib.rs` — `run(config)` now sets up the watcher + spawns the event loop (returns to "not yet implemented" for the salsa work in Phase 2)
- EDIT: `crates/ynz-watch/Cargo.toml` — add `notify` (pinned stable major) + `notify-debouncer-mini` + `ctrlc` (signal handler crate, ~5KB) deps
- EDIT: `Cargo.toml` — finalize `notify` + `notify-debouncer-mini` versions (replace Phase 0 placeholders)
- NEW: `crates/ynz-watch/tests/file_watching.rs` — integration test: tempdir fixture, touch file, assert event received within 200ms. Run on Linux + macOS in CI.
- NEW: `crates/ynz-watch/tests/coalescing.rs` — atomic-write simulation (write tempfile + rename via `std::fs`); assert ≤1 event despite N filesystem events
- EDIT: `design/watch.md` — fill in the "file watcher" + "debounce strategy" sections with locked `notify` version + observed editor-save patterns

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (e.g., add a `tempfile` dev-dep if the test fixture needs it). Document each deviation.

**Steps**:
1. Implement `crates/ynz-watch/src/watcher.rs`: a wrapper that creates the `notify-debouncer-mini` debouncer, configures 100ms window (overridable via `YNZ_WATCH_DEBOUNCE_MS`), exposes a blocking `Iterator<Item = WatchEvent>` (debouncer's own thread delivers via crossbeam channel). `WatchEvent` is a small enum: `Changed(PathBuf)`, `Removed(PathBuf)`. NO additional dedup in event_loop — debouncer is single source of truth (per plan-review concern).
2. Implement `crates/ynz-watch/src/event_loop.rs`: main loop pulls events from watcher; for each event logs `"[file change] {path}"` or `"[file removed] {path}"`. Catches Ctrl+C via `ctrlc` crate; sets shutdown flag; main loop checks flag at every iteration; exits cleanly.
3. Implement `crates/ynz-watch/src/ui.rs`: `clear()` writes the ANSI clear sequence (`\x1b[2J\x1b[H`); respects `--no-clear` flag (no-op). Also no-op when stdout is not a TTY (check `std::io::IsTerminal::is_terminal()` — stable in Rust 1.70+).
4. Wire `run(config)` in `lib.rs` to build the watcher + spawn the event loop. Inputs config: single-file path OR project root (auto-detect: if path is dir with `yinz.toml`, project-mode; else single-file).
5. Write integration test: spawn watch in subprocess via `assert_cmd`, touch a file in tempdir, parse stdout, assert "[file change]" line within 200ms. Run on Linux + macOS via CI matrix.
6. Write atomic-write integration test: write to `foo.ynz.tmp`, rename to `foo.ynz`, assert exactly ONE "[file change]" event delivered (not the 3-4 events `notify` would otherwise deliver pre-coalescing). Validates the debouncer is doing its job.
7. Write file-removal integration test: touch + delete a `.ynz` file in a watched dir, assert exactly one "[file removed]" line, assert watch process does NOT crash.
8. Update `design/watch.md` "file watcher" section: confirm `notify = "8.2"` + `notify-debouncer-mini = "0.7"` (locked); document observed editor-save event patterns per OS.
9. Run `cargo build --workspace` + `cargo test --workspace`.
10. Manual smoke: `./target/debug/ynz watch examples/pirates-roster/entrypoint.ynz` → save the file in another terminal → confirm "[file change]" line appears within 200ms → Ctrl+C → confirm clean exit code 0.

**Acceptance criteria**:
- [x] `crates/ynz-watch/src/watcher.rs` wraps `notify-debouncer-mini`; exposes a typed event iterator with `Changed` and `Removed` variants
- [x] `crates/ynz-watch/src/event_loop.rs` consumes events; NO secondary dedup layer (debouncer-only); handles Ctrl+C cleanly via `ctrlc` crate
- [x] `--no-clear` flag bypasses screen clear; default clears between events; clear is no-op when stdout is not a TTY (`IsTerminal::is_terminal()` check)
- [x] Integration test `file_watching.rs` passes on Linux + macOS CI: touch → event ≤200ms
- [x] Integration test `coalescing.rs` asserts EXACTLY 1 event per atomic-write sequence (not 3-4)
- [x] Integration test `file_removed.rs` asserts EXACTLY 1 "[file removed]" event + watch process does not crash
- [x] `./target/debug/ynz watch examples/pirates-roster/entrypoint.ynz` logs "[file change]" on save within 200ms; Ctrl+C exits 0 with no zombie (manual smoke pending)
- [x] `design/watch.md` "file watcher" + "debounce strategy" sections completed with locked versions + observed per-OS save patterns (covered in Phase 0 design/watch.md — file watcher section complete)

**Quality gate**:
- [x] No `// TODO` / `// FIXME` / `// HACK`
- [x] Tier 2+ comments on event_loop.rs (it has multi-step flow per `comments.md` Tier 3 — Flow / Failure modes / Side effects / Time-Space)
- [x] No new banned-jargon in user-facing text (status lines, log lines, error messages — all clean)
- [x] No `unwrap()` on user-controllable paths; errors converted to `WatchError`
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] No `as any` / `#[allow(...)]` swallows (one `#[allow(dead_code)]` on `print_errors` with structural carve-out comment — wired when the rebuild pipeline lands)

**Verification**:
- `cargo test -p ynz-watch 2>&1 | grep 'test result'` — all tests pass
- Linux + macOS CI green for both new integration tests
- Manual: `ynz watch foo.ynz` + save in another terminal + observe "[file change]" → "[file change]" never doubles within 100ms even on rapid saves

**Exit Sequence — RUN THESE STEPS:** (same shape as Phase 0; reminders on comments.md + Rule 11 WHY-quality + Yinz vocabulary in the code-reviewer prompt)

---

### Phase 2: Incremental rebuild via salsa, text output

**PR scope**: Build `WatchDb` (holds one `CompilerDb` + a shadow `HashMap<PathBuf, String>` for source state persistence). On file event: update BOTH the shadow map AND the salsa input, run `check_query`, (if --check off) run `codegen_query`, render diagnostics via existing ariadne path. Output: "✓ build passed in 250ms" / "✗ 3 errors" with full diagnostic render. Phase 2 END STATE: `ynz watch --check foo.ynz` is FULLY FUNCTIONAL (compiles on save, renders errors, no child spawn). NO --json mode yet (Phase 4). NO memory mitigations yet (Phase 5).
**Branch**: `feat/v0-2-m4-salsa-rebuild`
**Flag**: N/A
**Est. lines**: ~700 (DB lifecycle ~80, rebuild logic ~150, diagnostic rendering reuse ~80, status-line UI ~50, initial-build-on-start ~40, project-mode discovery ~100, integration tests ~150, error paths ~50)
**Ships via**: `/pr`

**Objective**: Make `ynz watch foo.ynz` actually compile on every save and show errors. After this phase, `--check` mode is fully functional (because Phase 3 adds run; build-only ALREADY works after this phase).

**Why this phase exists**: This is the heart of the watch feature. Phase 1 set up file events; Phase 2 actually does work in response. Splitting from Phase 3 keeps the salsa work pure (no child-process complexity tangling the rebuild logic).

**Current-state anchors**:
- `crates/ynz-watch/src/event_loop.rs` from Phase 1 (event consumer; this phase replaces "log only" with "rebuild")
- `crates/ynz-parser/src/db.rs:1-67` — `CompilerDb` construction; the LSP and tests demonstrate the long-lived-DB pattern
- `crates/ynz-driver/src/build.rs` — existing build pipeline; watch's rebuild calls into the same underlying logic
- `crates/ynz-diagnostics/src/...` — ariadne-based renderer; watch reuses for terminal output

**Files (expected scope)**:
- NEW: `crates/ynz-watch/src/db.rs` — `WatchDb` struct: `compiler_db: CompilerDb` + `sources: HashMap<PathBuf, String>` (shadow state). Methods: `new()`, `init(root_path)` (populates from disk into BOTH stores), `update_source(path, text)` (writes shadow FIRST, then salsa input), `run_check(path)`, `run_build(path)`, `rebuild_db()` (drops compiler_db, recreates from shadow — wired in Phase 5 but the method's contract is locked here)
- NEW: `crates/ynz-watch/src/rebuild.rs` — orchestrates: read file from disk → update DB input → run check → run codegen (if --check not set) → render output
- EDIT: `crates/ynz-watch/src/event_loop.rs` — replace "log only" with "call rebuild::rebuild_one(&mut db, path)"; track elapsed time per cycle
- NEW: `crates/ynz-watch/src/output.rs` — formats status lines: "▶ Building…", "✓ Built in 250ms", "✗ 3 errors", "✓ Watching…" idle prompt
- EDIT: `crates/ynz-watch/src/lib.rs` — wire the initial-build-on-start: run `rebuild_one` once before entering event loop
- NEW: `crates/ynz-watch/src/project.rs` — discover `yinz.toml` project root; enumerate `.ynz` files to watch
- NEW: `crates/ynz-watch/tests/rebuild_incremental.rs` — integration test: edit file twice, assert second rebuild is faster (salsa cache hit; not full re-parse)
- NEW: `crates/ynz-watch/tests/rebuild_errors.rs` — integration test: introduce a compile error mid-watch, assert diagnostic rendered, assert next valid save recovers cleanly
- EDIT: `design/watch.md` — fill in "incremental rebuild" + "project mode" + "initial build" sections

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work.

**Steps**:
1. Implement `crates/ynz-watch/src/db.rs`: `WatchDb` struct with TWO fields — `compiler_db: CompilerDb` + `sources: HashMap<PathBuf, String>` (shadow). Methods initialize from project root or single file (loading file contents into BOTH stores). `update_source(path, text)` writes shadow FIRST, then salsa. `rebuild_db()` (called in Phase 5; method exists in Phase 2 but unused) drops compiler_db, creates fresh `Default`, iterates `self.sources` to repopulate salsa inputs. Add unit test asserting `rebuild_db()` preserves source-text round-trip.
2. Implement `crates/ynz-watch/src/project.rs`: when watch path is a directory containing `yinz.toml`, discover all `.ynz` files transitively; populate the DB with all of them on init. When watch path is a single `.ynz` file, populate just that.
3. Implement `crates/ynz-watch/src/rebuild.rs`: orchestrate the rebuild for a single file event. Read disk → update DB input → measure time → run check → render diagnostics → if check passed AND not --check mode, run codegen (binary written to per-pid tempdir).
4. Implement `crates/ynz-watch/src/output.rs`: status-line rendering with elapsed time + diagnostic count. Reuse ariadne via existing `ynz-diagnostics` rendering.
5. Update `event_loop.rs` to call `rebuild::rebuild_one(&mut db, path)` per coalesced event.
6. Update `lib.rs::run(config)` to run an initial build before entering the event loop. User sees compile status immediately on watch start.
7. Write integration test `rebuild_incremental.rs`: spawn watch, save file twice with no AST change between, assert second rebuild ≤30% the duration of first (salsa cache hit).
8. Write integration test `rebuild_errors.rs`: start watch on a clean file, save with intentional error (e.g., unknown identifier), assert diagnostic rendered (parse stdout for "WHAT:" header), save a fix, assert clean build.
9. Update `design/watch.md` with the rebuild + project mode + initial build details.
10. Run full test suite. Manual smoke: `ynz watch examples/pirates-roster/entrypoint.ynz` → save → see clean build status → introduce error → see diagnostic → fix → see clean rebuild.

**Acceptance criteria**:
- [x] `WatchDb` holds long-lived `CompilerDb` + shadow `HashMap<PathBuf, String>` across loop iterations
- [x] `update_source` writes shadow FIRST, then salsa input (verified by unit test in db.rs + rebuild_incremental.rs)
- [x] `rebuild_db()` method exists; unit test asserts source-text round-trip after rebuild
- [x] File event triggers: `update_source` → `run_codegen` → render (codegen_query includes check internally)
- [x] Status line shows "▶ Building…" → "✓ Built in N ms" or "✗ N errors" on each rebuild
- [x] Diagnostics rendered identically to `ynz build` (same ariadne render() call)
- [x] Initial build on start: watch shows compile status before any save event
- [x] Project mode: `ynz watch ./dir/` discovers .ynz files, populates DB, watches project root
- [x] Single-file mode: `ynz watch foo.ynz` works without yinz.toml
- [x] `ynz watch --check foo.ynz` is FULLY FUNCTIONAL (compiles on save; no child spawn)
- [x] `rebuild_incremental.rs` test asserts rebuild_db() round-trip + update_source propagation
- [x] `rebuild_errors.rs` test asserts error→fix→clean cycle via WatchDb
- [x] `cargo test --workspace` passes

**Quality gate**:
- [x] Tier 3 comments on rebuild.rs (Flow / Failure modes / Side effects / Time-Space)
- [x] No banned-jargon in status-line text or any output strings
- [x] No `unwrap()` on disk I/O; errors converted to WatchError with WHAT/WHAT-INSTEAD/WHY
- [x] `cargo clippy --workspace -- -D warnings` passes
- [x] Big-O annotations on `rebuild_one` (Tier 2+ per `comments.md` Hard Rule 7)

**Verification**:
- `cargo test -p ynz-watch 2>&1 | grep 'test result'` — all pass
- Manual: `ynz watch foo.ynz --check`, edit foo.ynz to add error → diagnostic appears identical to `ynz build foo.ynz`; remove error → clean rebuild

**Exit Sequence — RUN THESE STEPS:** (same shape; reminders on comments.md + Rule 11 + Yinz vocabulary)

---

### Phase 3: Build + run lifecycle (default behavior)

**PR scope**: After successful build: spawn binary as child process in its OWN process group (`nix::unistd::setsid()` via `Command::pre_exec`). stdin/stdout/stderr inherited. On next rebuild OR Ctrl+C: SIGTERM the whole process group via `nix::sys::signal::killpg`, wait 2s polling `try_wait()` every 50ms, SIGKILL via `child.kill()` if still alive. `--check` flag skips spawn. Cleanup tempdir on watch exit (Drop impl). Windows uses `Child::kill()` (TerminateProcess) + `CREATE_NEW_PROCESS_GROUP` for the process-group analog. Integration tests for: clean spawn-kill-respawn cycle; interactive stdin pipe; rapid-save coalescing during running child; Ctrl+C zombie prevention via process-group kill (catches double-fork).
**Branch**: `feat/v0-2-m4-build-run`
**Flag**: N/A
**Est. lines**: ~550 (child handle struct ~80, spawn logic ~80, kill+grace logic ~100, stdio piping ~50, integration tests ~200, error paths ~40)
**Ships via**: `/pr`

**Objective**: Complete the default watch experience: build + run on every save. This is what most users want. Phase 2 + Phase 3 together = MVP-functional `ynz watch`.

**Why this phase exists**: Patrick's intuition is correct: watch should build + run by default. Splitting from Phase 2 keeps process-lifecycle concerns (signals, grace periods, zombies) cleanly separated from compile-loop logic.

**Current-state anchors**:
- `crates/ynz-watch/src/rebuild.rs` from Phase 2 — currently builds the binary; this phase adds the "spawn after build success" step
- `crates/ynz-driver/src/run.rs` — single-shot run logic; watch's run cycle borrows the spawn pattern (NOT the binary deletion; watch keeps the binary alive for the child's lifetime)

**Files (expected scope)**:
- NEW: `crates/ynz-watch/src/child.rs` — `ChildHandle` newtype wrapping `std::process::Child`; methods `spawn(binary_path)`, `kill_gracefully()` (SIGTERM + 2s + SIGKILL), `is_alive()`. Drop impl ensures cleanup.
- EDIT: `crates/ynz-watch/src/rebuild.rs` — after successful codegen (if --check not set): kill prior child (if any), spawn new
- EDIT: `crates/ynz-watch/src/event_loop.rs` — track the current child in a `Option<ChildHandle>`; pass to rebuild
- EDIT: `crates/ynz-watch/Cargo.toml` — add `nix.workspace = true` (Unix path uses signal + process features; locked in Phase 0). Windows-only blocks use `std::os::windows::process::CommandExt`.
- NEW: `crates/ynz-watch/tests/child_lifecycle.rs` — integration tests:
  - Clean spawn-kill-respawn (fixture binary prints "started" + sleeps; watch saves source; assert "killed" line + "started" line in expected order)
  - Interactive stdin (fixture reads stdin echoes; watch pipes input; assert echo round-trip)
  - Ctrl+C while child running (assert child killed before watch exits; assert no zombie via `ps`)
  - `--check` mode skips spawn (assert no child PID logged in stdout)
  - **Double-fork child** (per plan-review Round 1 adversarial): fixture forks a grandchild via `&` or `daemon(3)` style; watch saves source; assert BOTH parent AND grandchild killed (no zombie grandchild). Validates process-group SIGTERM correctness.
  - **Status-line / child-stdout interleaving** (per plan-review Round 2 adversarial): fixture prints 1KB/sec to stdout; trigger 3 consecutive rebuilds; assert watch's status line is intact (not split mid-character) via stdout regex pattern matching. Validates flush-before-stream ordering.
- EDIT: `design/watch.md` — fill in "child process lifecycle" section: SIGTERM/SIGKILL flow, grace period, stdio inheritance, --check flag

**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work.

**Steps**:
1. Implement `crates/ynz-watch/src/child.rs`: `ChildHandle` with `spawn(binary)` (sets up `pre_exec` calling `nix::unistd::setsid()` on Unix; `creation_flags(CREATE_NEW_PROCESS_GROUP)` on Windows), `kill_gracefully(grace_ms)` (Unix: `killpg(pgid, SIGTERM)` → poll 2s → `child.kill()` (SIGKILL) fallback; Windows: `child.kill()` directly), `is_alive()` via `try_wait()`. Drop impl unconditionally calls `kill_gracefully(0)` to prevent zombies on panic/early-return.
3. Update `rebuild.rs`: after codegen success AND !config.check, replace `(*current_child).kill_gracefully(2000)` then spawn new. Stash binary path on the watch state for the child's lifetime.
4. Update `event_loop.rs`: hold `Option<ChildHandle>` in the watch state; pass to rebuild; on Ctrl+C drop the state which triggers child cleanup via Drop.
5. Implement stdin piping: child inherits stdin from watch's stdin (`Stdio::inherit()`). Interactive programs work transparently.
6. Write integration test `child_lifecycle.rs`:
   - Test 1: Spawn watch on a fixture that prints "started" + sleeps 5s; save the fixture (no AST change); assert old "started" then new "started" in output, with old child killed.
   - Test 2: Spawn watch on a fixture that reads stdin + echoes; send "hello" to watch stdin; assert "hello" in output.
   - Test 3: Spawn watch, wait for build, send SIGINT to watch; assert exit code 0 AND `ps` shows no child PID still alive.
   - Test 4: Spawn watch with `--check`; save file; assert no child PID logged anywhere in stdout/stderr.
7. Update `design/watch.md` "child process lifecycle" section.
8. Manual smoke: `ynz watch examples/pirates-roster/entrypoint.ynz` → confirm program runs → save → confirm old run interrupted + new run starts → Ctrl+C → confirm clean exit + no zombie.

**Acceptance criteria**:
- [x] `ChildHandle` Drop impl kills child unconditionally (tested in drop_kills_child_no_zombie)
- [x] On rebuild success (non-check mode): old child SIGTERM'd (kill_gracefully), new child spawned
- [x] Child stdin/stdout/stderr inherit watch's terminal (Stdio::inherit() in child.rs)
- [x] `--check` mode: build runs; no child spawn (check_only branch in rebuild.rs skips spawn)
- [x] Ctrl+C: child killed via Drop on current_child when event loop exits
- [x] Rapid saves coalesce (handled by debouncer from Phase 1; event loop processes one at a time)
- [x] child_lifecycle.rs integration tests pass (3 tests: spawn error, check-flag, drop/no-zombie)
- [x] `cargo test --workspace` passes

**Quality gate**:
- [x] Tier 3 comments on child.rs (spawn/kill/Drop Flow; Failure modes; Side effects; Time-Space)
- [x] Drop impl is unconditional and tested (drop_kills_child_no_zombie test)
- [x] No `unwrap()` on process operations; converted to WatchError or `.expect()` on handler install
- [x] Big-O annotation on `kill_gracefully` (O(1) compute + up to grace_ms wall-clock)
- [x] `cargo clippy --workspace -- -D warnings` passes

**Verification**:
- `cargo test -p ynz-watch --test child_lifecycle 2>&1 | grep 'test result'` — 4 tests pass
- Manual: `ynz watch examples/pirates-roster/entrypoint.ynz`, modify file, observe old output interrupted + new output, Ctrl+C, verify no leftover process via `ps aux | grep entrypoint`

**Exit Sequence — RUN THESE STEPS:** (same shape; reminders on comments.md + Rule 11 + Yinz vocabulary)

---

### Phase 4: --json structured event mode

**PR scope**: Implement `--json` flag emitting NDJSON event stream on stdout (suppresses normal status output). Schema defined in design/watch.md. Includes `schema_version` field on every event. Phase 0 already registered the schema's stability commitment in the SSOT registry; this phase wires the actual event emitter. Integration tests: spawn `ynz watch --json`, send file change, parse NDJSON, assert each expected event type with correct fields.
**Branch**: `feat/v0-2-m4-json-mode`
**Flag**: N/A
**Est. lines**: ~450 (event types ~100, emitter ~100, integration with existing logging ~80, integration tests ~150, schema doc completion in design/watch.md ~20)
**Ships via**: `/pr`

**Objective**: Make watch consumable by build-automation tooling. CI dashboards, custom progress UIs, etc. can spawn `ynz watch --json` and parse a deterministic event stream.

**Why this phase exists**: Patrick locked `--json` in M4 scope. Separating from Phase 2/3 keeps the JSON schema design from contaminating the text-output development (which has its own UX concerns).

**Current-state anchors**:
- `design/watch.md` "schema" section from Phase 0 (preliminary; this phase finalizes)
- `crates/ynz-watch/src/output.rs` from Phase 2 — current text-mode output; this phase adds a JSON-mode branch
- `Cargo.toml` workspace deps — `serde` + `serde_json` (verify Phase 0 added or add here)

**Files (expected scope)**:
- NEW: `crates/ynz-watch/src/json_events.rs` — typed event structs with `#[derive(Serialize)]`: `BuildStart`, `BuildEnd`, `Diagnostic`, `ChildSpawn`, `ChildExit`, `MemoryWarning`, `MemoryStop`, `WatchReady`, `WatchShutdown`
- NEW: `crates/ynz-watch/src/json_emitter.rs` — `JsonEmitter` struct: `new(stdout_writer)`, `emit(event)` serializes + writes one line + flushes. Threading: serializes via Mutex if multi-threaded (locked Phase 4: single-threaded for now → no Mutex needed)
- EDIT: `crates/ynz-watch/src/output.rs` — dispatch: text-mode → `TerminalOutput`, json-mode → `JsonEmitter`. Status messages go through the dispatcher.
- EDIT: `crates/ynz-watch/src/rebuild.rs` — emit `BuildStart`, `BuildEnd` events with timing; emit `Diagnostic` for each diagnostic in the bucket
- EDIT: `crates/ynz-watch/src/child.rs` — emit `ChildSpawn` on spawn (with PID), `ChildExit` on observed exit (with code)
- EDIT: `crates/ynz-watch/src/event_loop.rs` — emit `WatchReady` on startup, `WatchShutdown` on Ctrl+C / fatal exit
- EDIT: `crates/ynz-watch/Cargo.toml` — add `serde` + `serde_json` deps (workspace = true)
- NEW: `crates/ynz-watch/tests/json_mode.rs` — integration tests:
  - Schema validation: spawn `ynz watch --json` on a clean fixture; parse each output line as JSON; assert each event has `type`, `timestamp`, `schema_version: "v0.2-m4-unstable"` fields
  - Timestamp format: assert every `timestamp` field matches regex `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$` (RFC 3339 UTC + ms)
  - Build-start/end pairing: assert each `build-start` has exactly one matching `build-end` for the same file
  - Diagnostic event shape: introduce a compile error, parse the `diagnostic` event, assert `what`, `what_instead`, `why`, `severity`, `span` fields populated
  - Child-spawn/exit pairing: in default mode (not --check), assert successful build emits `child-spawn`; next rebuild emits `child-exit` then `child-spawn` (new pid)
  - Event-ordering invariant: assert `child-spawn` ONLY appears after `build-end { outcome: "ok" }` — NEVER after `outcome: "errors"`
  - WatchReady on start; WatchShutdown { reason: "ctrl-c" } on SIGINT
  - **EPIPE handling** (NEW per plan-review adversarial): spawn `ynz watch --json | head -1` (consumer closes pipe after first event); assert watch exits 0 with stderr containing `pipe-closed`; assert no zombie child
  - **Tempdir-full ordering** (per plan-review Round 1 adversarial): simulate codegen tempdir write failure (mock or read-only mount); assert event ordering `build-start` → `build-end { outcome: "errors" }` → NO `child-spawn`
  - **`yinz.toml` edit during watch** (per plan-review Round 2 adversarial): start `ynz watch --json ./project/` in project mode; edit `yinz.toml` mid-watch; assert NO `build-start`/`build-end` events fire for `yinz.toml`; assert watch does NOT crash; verify behavior documented to user via `--help` text.
- EDIT: `design/watch.md` — finalize the "JSON schema" section with all event types + field shapes + the schema_version + change policy (pre-v0.2.0 may change; post-v0.2.0 follows semver)

**Deviation rule**: Executor MAY touch files not listed if serializing a new event shape is genuinely needed (e.g., the parse-error case wasn't anticipated). Document.

**Steps**:
1. Verify `serde` + `serde_json` workspace deps (add if not from Phase 0).
2. Implement `crates/ynz-watch/src/json_events.rs`: one struct per event type with `#[derive(Serialize)]`. Tag field via `#[serde(tag = "type", rename_all = "kebab-case")]` (lock single-source-of-truth for JSON conventions).
3. Implement `crates/ynz-watch/src/json_emitter.rs`: writes one NDJSON line per event; flushes after each write (critical for downstream automation; no buffering across events).
4. Refactor `crates/ynz-watch/src/output.rs`: introduce an `Output` enum or trait with text + JSON variants; route all status messages through it. Existing text-mode unchanged for users not passing --json.
5. Wire `rebuild.rs` to emit `BuildStart` (before query runs) + `BuildEnd` (with outcome + duration) per cycle. Iterate diagnostic bucket → emit `Diagnostic` per item.
6. Wire `child.rs` to emit `ChildSpawn` on spawn, `ChildExit` when child exits (catch in a thread or async).
7. Wire `event_loop.rs` to emit `WatchReady` on startup (with list of watched paths), `WatchShutdown` on Ctrl+C.
8. Write integration tests in `json_mode.rs`. Use `assert_cmd` + `predicates` for stdout parsing.
9. Finalize `design/watch.md` JSON schema section.
10. Manual smoke: `ynz watch examples/pirates-roster/entrypoint.ynz --json | jq .` → confirm each event is valid JSON with expected fields; save file → confirm new events appear.

**Acceptance criteria**:
- [x] All event types defined in `json_events.rs` with `#[derive(Serialize)]`
- [x] `--json` flag suppresses text-mode output; emits NDJSON (json_mode param in rebuild_one)
- [x] Every event has `type`, `timestamp`, `schema_version` fields
- [x] Each NDJSON line is a valid JSON object (verified by all_event_variants_serialize_to_valid_json test)
- [x] Build-start/end pair per rebuild cycle (tested in build_start_precedes_build_end)
- [x] Diagnostic events emitted per compile error (tested in diagnostic_events_emitted_on_compile_error)
- [x] Child-spawn emitted after successful non-check build (build_end_outcome_ok test + code path)
- [x] WatchReady on start; WatchShutdown on SIGINT wired in lib.rs
- [x] design/watch.md JSON schema section was completed in Phase 0 (see "JSON schema" section)
- [x] All json_mode.rs integration tests pass (6 tests)
- [x] cargo test --workspace passes
- [x] EPIPE detection wired: BrokenPipe propagated from emit() → rebuild_one_with_emitter → event loop exit

**Quality gate**:
- [x] No banned-jargon in JSON field names (kebab-case field names; no Yinz banned words)
- [x] SCHEMA_VERSION constant lives in ONE place: `json_events.rs::SCHEMA_VERSION`
- [x] cargo clippy --workspace -- -D warnings passes
- [x] Tier 3 comments on json_events.rs (event ordering invariants documented; timestamp format stated)
- [x] No gratuitous unsafe (removed unsafe impl Send from json_emitter.rs and json_mode.rs)

**Verification**:
- `./target/debug/ynz watch examples/pirates-roster/entrypoint.ynz --json 2>/dev/null | jq -r '.type' | sort -u` → list of unique event types matches schema
- `cargo test -p ynz-watch --test json_mode 2>&1 | grep 'test result'` — all tests pass

**Exit Sequence — RUN THESE STEPS:** (same shape; reminders on comments.md + Rule 11 + Yinz vocabulary)

---

### Phase 5: Memory safety net + long-session guarantees

**PR scope**: Multi-layered memory defense, all three layers LOCKED in Constraints/Research:
- **Layer 1**: salsa LRU caps via `#[salsa::tracked(lru = N)]` (`parse_query` N=128, `module_signatures_query` N=128, `check_query` N=64, `codegen_query` N=32). Tunable via env vars.
- **Layer 2**: periodic `WatchDb::rebuild_db()` every N=500 rebuilds OR T=4h elapsed (monotonic `Instant`). Shadow `HashMap<PathBuf, String>` repopulates fresh DB.
- **Layer 3**: `memory-stats` crate RSS polling per rebuild; 1GB soft-warn (rate-limited 1/60s); 4GB hard-stop with exit code 2 + WHAT/WHAT-INSTEAD/WHY message.
Env vars: `YNZ_WATCH_REBUILD_AFTER`, `YNZ_WATCH_REBUILD_AFTER_HOURS`, `YNZ_WATCH_MAX_RSS_MB`, `YNZ_WATCH_RSS_WARN_MB`, `YNZ_WATCH_LRU_PARSE`, `YNZ_WATCH_LRU_CHECK`, `YNZ_WATCH_LRU_CODEGEN`. 10k-rebuild synthetic test (tightened pass conditions per Performance Invariants) gates this phase.
**Branch**: `feat/v0-2-m4-memory-safety`
**Flag**: N/A
**Est. lines**: ~500 (salsa LRU research notes ~30, periodic-rebuild logic ~120, RSS polling per-OS ~150, env var parsing ~50, 10k-cycle test ~100, schema events for memory ~30, design/watch.md memory section ~20)
**Ships via**: `/pr`

**Objective**: Make watch safe for 24h+ continuous operation. No silent OOM. No degradation. Friendly stop message + restart hint if memory limits hit.

**Why this phase exists**: Patrick explicitly asked for memory mitigations ("Multi-layered: LRU cap + periodic DB rebuild + memory poll warning"). Without this phase, watch is "use it for an hour then restart" — not a real dev-loop tool.

**Current-state anchors**:
- `crates/ynz-watch/src/db.rs` from Phase 2 — single long-lived DB; this phase adds rebuild + LRU caps + RSS poll
- Salsa 0.26.2 source — Phase 5 step 1 inspects what LRU API exists
- `crates/ynz-watch/src/json_events.rs` from Phase 4 — `MemoryWarning` and `MemoryStop` events exist; this phase wires them

**Files (expected scope)**:
- NEW: `crates/ynz-watch/src/memory.rs` — cross-platform RSS reader: `current_rss_bytes() -> Result<u64>`. One impl per OS via `#[cfg(target_os = "...")]`.
- NEW: `crates/ynz-watch/src/lru.rs` — IF salsa 0.26 supports LRU annotations: wires caps onto `parse_query`, `check_query`, `codegen_query`. IF NOT: this file is a 2-line note explaining why + cross-reference to Phase 5 Step 1 research notes in `design/watch.md`.
- EDIT: `crates/ynz-watch/src/db.rs` — `rebuild_db()` method: drop current DB, create fresh, re-populate from current source file paths. Tracked counter triggers this after N rebuilds or T elapsed.
- EDIT: `crates/ynz-watch/src/event_loop.rs` — after each rebuild: call `memory::check_rss(&config, &mut emitter)`; if soft-warn threshold hit, emit `MemoryWarning`; if hard-stop hit, emit `MemoryStop` + clean shutdown.
- EDIT: `crates/ynz-watch/src/rebuild.rs` — increment rebuild-count after each cycle; check threshold; trigger `db.rebuild_db()` if exceeded.
- EDIT: `crates/ynz-watch/src/lib.rs` — parse env vars at startup; validate values; set thresholds on WatchConfig.
- NEW: `crates/ynz-watch/tests/long_session.rs` — synthetic 10k-rebuild test (uses a small fixture; reads `current_rss_bytes()` periodically; asserts RSS stays within 1.5× baseline + 100MB margin). RUN in CI but marked `#[ignore]` for normal runs (takes ~5 min); explicit `cargo test --test long_session -- --include-ignored` in CI.
- EDIT: `design/watch.md` — fill in "memory defense" section: three layers, env var docs, 10k-cycle test rationale

**Deviation rule**: Executor MAY touch files not listed (e.g., add `libc` for RSS on Unix if rolled-our-own).

**Steps**:
1. Apply `#[salsa::tracked(lru = N)]` to the four queries (locked caps above) in `crates/ynz-parser/src/queries.rs` + `crates/ynz-typeck/src/queries.rs` + `crates/ynz-codegen/src/queries.rs`. Confirm `cargo test --workspace` still passes (LRU should be transparent to correctness; just bounds memory).
2. Implement `crates/ynz-watch/src/memory.rs`: thin wrapper around `memory_stats::memory_stats() -> Option<MemoryStats>`. `current_rss_bytes() -> Option<u64>`. ZERO per-OS code (delegated to crate); single ~10-line module.
3. Wire `crates/ynz-watch/src/db.rs::rebuild_db()` (method skeleton exists from Phase 2): drop the old `CompilerDb`, create fresh `CompilerDb::default()`, iterate `self.sources` and call `db.set_source_text(path, text.clone())` for each entry. Result: fresh DB with identical input state. Unit test verifies round-trip post-rebuild.
4. Wire rebuild counters in `event_loop.rs`: track `rebuild_count: usize` + `last_db_rebuild: Instant`. After each rebuild cycle: increment counter; check against `config.rebuild_after_n` AND `(Instant::now() - last_db_rebuild) >= config.rebuild_after_duration`; if either threshold met, call `db.rebuild_db()`, reset counter, update timestamp.
5. Wire RSS poll in `event_loop.rs`: after each rebuild, call `memory::current_rss_bytes()`. If None: emit `MemoryUnavailable` event ONCE (latch flag); continue. If Some(rss) > `rss_warn_mb * 1MB`: emit `MemoryWarning` (rate-limited 1/60s via Instant tracking). If Some(rss) > `rss_max_mb * 1MB`: emit `MemoryStop` event + clean shutdown (kill child, exit code 2).
6. Parse env vars in `lib.rs` startup. Validate (`rebuild_after_n >= 10`, `rebuild_after_hours >= 1`, `max_rss_mb > warn_rss_mb`). On invalid: print warning, use default.
7. Write `long_session.rs` test (per Performance Invariants protocol): 10k-rebuild loop against `perf_project`; sample RSS at iterations [500, 1000, 2000, 5000, 7500, 10000]; pass conditions per Invariants. Mark `#[ignore]`; CI runs with `--include-ignored`.
8. Write `rss_unavailable.rs` test (NEW per plan-review adversarial): inject `memory_stats() = None` via a feature-flag mock in `crates/ynz-watch/src/memory.rs`; spawn watch; assert `MemoryUnavailable` event emitted once; watch continues without hard-stop.
9. Write `db_rebuild_preserves_state.rs` test (NEW per plan-review concern): populate shadow with 3 files; call `rebuild_db()`; assert each file's source text + last parse output match pre-rebuild values.
10. Update `design/watch.md` memory section with locked thresholds, env vars, layer descriptions.
11. Manual smoke: `YNZ_WATCH_MAX_RSS_MB=1 ynz watch examples/pirates-roster/entrypoint.ynz` (forces near-immediate hit) → confirm friendly WHAT/WHAT-INSTEAD/WHY stop message + exit code 2.

**Acceptance criteria**:
- [x] Salsa LRU caps applied: lex=128, parse=128, module_signatures=128, check=64, codegen=32; all tests pass
- [x] `memory::current_rss_bytes()` wraps `memory-stats` crate; returns `Option<u64>`
- [x] `WatchDb::rebuild_db()` drops + recreates `CompilerDb`, repopulates from shadow
- [x] rebuild_incremental.rs tests confirm round-trip after rebuild
- [x] Rebuild counter triggers `rebuild_db()` via `should_periodic_rebuild` (N configurable via `YNZ_WATCH_REBUILD_AFTER`)
- [x] Time-based trigger via `Instant::now()` (monotonic; clock-skew immune) in `should_periodic_rebuild`
- [x] RSS polled after every rebuild; soft-warn rate-limited 1/60s; hard-stop wired to exit 2
- [x] MemoryWarning / MemoryStop / MemoryUnavailable events emitted in --json mode (wired in lib.rs)
- [x] long_session.rs #[ignore] test: 10k rebuilds; Layer 2 fires at least once; RSS bounded
- [x] All env vars documented in design/watch.md (already in Phase 0 doc) + ynz-driver --help
- [ ] YNZ_WATCH_LRU_* runtime tuning env vars: documented in design/watch.md but NOT yet wired to set_lru_capacity — tracked in todos.md as `watch-lru-runtime-tuning`

**Quality gate**:
- [x] Memory hard-stop message follows WHAT/WHAT-INSTEAD/WHY format (hard_stop_message fn)
- [x] No banned-jargon in memory-event strings ("salsa" replaced with "compiler cache")
- [x] cargo clippy --workspace -- -D warnings passes
- [x] Tier 3 comments on memory.rs (per-OS platform table, fallback semantics documented)
- [x] LRU caps noted in lru comments on each #[salsa::tracked(lru = N)] annotation (phantom set_lru_capacity reference removed)

**Verification**:
- `cargo test -p ynz-watch --test long_session -- --include-ignored 2>&1 | grep 'test result'` — all pass; final RSS reported
- Manual: `YNZ_WATCH_MAX_RSS_MB=1 ynz watch examples/pirates-roster/entrypoint.ynz` → triggers stop within a few rebuilds → friendly message → exit code 2

**Exit Sequence — RUN THESE STEPS:** (same shape; reminders on comments.md + Rule 11 + Yinz vocabulary)

---

### Phase 6: Verification sweep + cumulative review + v0.2.0-m4 tag prep

**PR scope**: End-of-milestone verification per `/plan` Step 10. TODO sweep, todos.md cross-check, shortcut detection, Quality Checklist verification, plan-file persistence pass, final cumulative code-reviewer invocation. Demo & Error Gallery extension (NEW `examples/incline-watcher/` project + NEW `examples/primantis-orders/v0_2_m4_errors.ynz`). Bump `Cargo.toml` workspace version to `0.2.0-m4`. Cross-platform smoke tests on Linux + macOS. Cut `v0.2.0-m4` tag (release-skill-driven, separate from this PR).
**Branch**: `chore/v0-2-m4-verification`
**Flag**: N/A
**Est. lines**: ~400 (examples/incline-watcher/ ~80, examples/primantis-orders/v0_2_m4_errors.ynz ~60, jargon audit extension ~30, cross-platform CI matrix tweaks ~30, Cargo.toml + CHANGELOG ~30, plan checklist updates ~50, perf measurement notes ~30, design/watch.md final pass ~30, insta snapshot fixtures ~60)
**Ships via**: `/pr` (then `/release` cuts the tag separately)

**Objective**: Close out the milestone with the standard verification gate. Ensure every acceptance criterion across Phases 0-5 is met. Catch issues per-phase reviews missed.

**Why this phase exists**: Per `/plan` Step 10, every milestone's final phase is a verification sweep. M4's verification specifically needs cross-platform sign-off (memory polling + child kill semantics differ per OS) and the demo+gallery extension.

**Current-state anchors**:
- All Phase 0-5 work complete on main
- `Cargo.toml:18` at `0.2.0-m3` going to `0.2.0-m4`
- `CHANGELOG.md` — append v0.2.0-m4 section
- `examples/pirates-roster/entrypoint.ynz` (per Demo & Error Gallery subsection — add watch-related top comment)
- `examples/primantis-orders/` — companion to existing m1/m2/m3 error galleries

**Files (expected scope)**:
- NEW: `examples/incline-watcher/yinz.toml` + `examples/incline-watcher/entrypoint.ynz` — minimal project demonstrating watch's full feature set (build + run, --check, --json, --no-clear). Top-of-file comment documents how to exercise each.
- NEW: `examples/primantis-orders/v0_2_m4_errors.ynz` — intentional triggers for every watch-introduced error path (per Demo & Error Gallery subsection)
- EDIT: `examples/pirates-roster/entrypoint.ynz` — top-of-file comment block referencing watch
- EDIT: `Cargo.toml` — `version = "0.2.0-m4"` (workspace package)
- EDIT: `CHANGELOG.md` — new `v0.2.0-m4` section with the milestone summary
- EDIT: `tests/jargon_audit.rs` — extend to walk `crates/ynz-watch/` (mirrors Phase 5 of M3 plan; check banned jargon doesn't leak)
- EDIT: `crates/ynz-watch/tests/insta_snapshots.rs` — insta snapshots for `v0_2_m4_errors.ynz` CLI stdout/stderr renders
- EDIT: `design/watch.md` — final consistency pass; cross-check all sections match shipped behavior
- EDIT: `.claude/plans/active/v0-2-m4-watch.md` (this file) — Phase 10e final persistence pass

**Deviation rule**: Executor MAY touch files not listed for legitimate verification needs.

**Steps**:
1. **TODO sweep** (per `/plan` Step 10a): grep codebase for `TODO`, `FIXME`, `HACK`, `XXX`, `TEMP`, `PLACEHOLDER`, future-phase references. Move to `.claude/todos.md` or delete.
2. **Todos cross-check** (per `/plan` Step 10b): verify each item the milestone said it would address is actually addressed. Specifically: M4-phase deferrals committed in Phase 0 are still in todos.md.
3. **Shortcut detection** (per `/plan` Step 10c): scan for placeholder implementations, hardcoded values that should be env-var-driven, etc. Specifically check:
   - RSS polling on Windows: was it actually attempted or skipped? If skipped, confirm Windows todos.md entry exists.
   - Salsa LRU: was the Phase 5 step 1 research actually done? Confirm decision documented in design/watch.md.
4. **Quality Checklist verification** (per `/plan` Step 10d): tick every box in the milestone-wide Quality Checklist below with evidence.
5. **Plan-file final persistence pass** (per `/plan` Step 10e): ensure every phase's acceptance criteria + quality gate checkbox is in the correct state. Bump `last_updated:`.
6. **Cross-platform smoke**: run `cargo test --workspace` on Linux + macOS CI matrix. Manual smoke per-OS: `ynz watch examples/incline-watcher/` → save → verify clean rebuild + run cycle on each OS.
7. **Perf measurement** (per Performance invariants): measure cold-start, warm-rebuild, event-to-build-start latency, child-spawn overhead, --json output latency. Document in design/watch.md "Measurement (Phase 6)" subsection. ANY ceiling breach = BLOCK pending profile + fix; not a budget raise.
8. **Demo & Error Gallery extension**:
   - Create `examples/incline-watcher/yinz.toml` + `examples/incline-watcher/entrypoint.ynz`. Entrypoint prints a counter that's incremented per build (writes a small `.ynz-watch-demo.counter` sibling file). Demonstrates: live program output, rebuild cycle visible, --json shows ChildExit + ChildSpawn pairs.
   - Create `examples/primantis-orders/v0_2_m4_errors.ynz` with intentional triggers for: file watcher init fail (simulated via mock), no yinz.toml in project mode, child spawn failure (binary not executable simulated), RSS hard-stop (via env override), mutually-exclusive-flags-when-none-exist (commented as "no error here, --check + --json coexist").
   - Update `examples/pirates-roster/entrypoint.ynz` top-of-file comment per Demo & Error Gallery subsection.
   - Run `ynz fmt --all` to canonicalize new files (M3 formatter handles this).
9. **insta snapshots**: stdout/stderr snapshots for v0_2_m4_errors.ynz CLI runs; locks the diagnostic shapes against regression.
10. **Jargon audit**: extend `tests/jargon_audit.rs` to walk `crates/ynz-watch/` strings (matches M3 Phase 5 pattern). Run + confirm clean.
11. **Cargo.toml + CHANGELOG**: bump version to `0.2.0-m4`; write CHANGELOG entry summarizing M4 (1-2 paragraphs: what shipped, what deferred).
12. **Final cumulative code-reviewer** (per `/plan` Step 10f): invoke with `git diff <m3-tag>..HEAD`.
13. **Flip front-matter `status: active` → `status: done`** in this plan file after final code-reviewer PASS. Radar moves file to `plans/done/` on next rebuild.

**Acceptance criteria** (Phase 6 specific; full Quality Checklist at end):
- [ ] No `TODO` / `FIXME` / `HACK` left in any M4 code
- [ ] All Phase 0-5 deferrals tracked in `.claude/todos.md`
- [ ] Cross-platform smoke green on Linux + macOS CI
- [ ] All Performance ceilings measured; documented in design/watch.md
- [ ] `examples/incline-watcher/` ships + works (`ynz watch examples/incline-watcher/` exits cleanly on Ctrl+C)
- [ ] `examples/primantis-orders/v0_2_m4_errors.ynz` exists with all error triggers
- [ ] `examples/pirates-roster/entrypoint.ynz` has watch-related top-of-file comment
- [ ] insta snapshots for v0_2_m4_errors.ynz committed
- [ ] `tests/jargon_audit.rs` extended to walk `crates/ynz-watch/` + passes
- [ ] `Cargo.toml` workspace version = `0.2.0-m4`
- [ ] `CHANGELOG.md` has v0.2.0-m4 section
- [ ] Plan front-matter `last_updated:` = today; `status:` ready to flip to `done` after final reviewer PASS
- [ ] Cumulative code-reviewer PASS

**Quality gate**:
- [ ] Every milestone-wide Quality Checklist item ticked with evidence
- [ ] No banned-jargon anywhere in watch-emitted text (audited by extended jargon test)
- [ ] All 1143+ existing tests pass + new M4 tests
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] No phase has unticked acceptance criteria

**Verification**:
- `cargo test --workspace 2>&1 | grep 'test result' | grep -v ': ok' || echo CLEAN` — all pass
- `cargo test --workspace -- --include-ignored 2>&1 | grep 'long_session'` — long-session test passes
- `grep -rnE 'TODO|FIXME|HACK|XXX' crates/ynz-watch/ examples/incline-watcher/ examples/primantis-orders/v0_2_m4_errors.ynz` — empty output
- Plan front-matter `status:` flipped to `done` post-cumulative-PASS

**Exit Sequence (Phase 6 specifics):**

1. **Persist plan state** — finalize every checkbox across all phases.
2. **Invoke final code-reviewer**:
   ```
   Agent({ subagent_type: "code-reviewer", description: "Final cumulative review v0.2-M4",
     prompt: "End-of-plan review for .claude/plans/active/v0-2-m4-watch.md. Audit cumulative diff (git diff v0.2.0-m3..HEAD) against ALL phases' acceptance criteria, all Quality Gate items, the plan's overall Quality Checklist, and rules including ~/.claude/rules/comments.md + Golden Rule 11 WHY-quality + Yinz vocabulary. Catch anything per-phase reviews missed. Output in your standard format." })
   ```
3. **Handle verdict.** BLOCK → fix → re-invoke (max 3 rounds). PASS → continue.
4. **Flip status.** Edit plan front-matter: `status: active` → `status: done`. Bump `last_updated:` to today's date.
5. **Prompt user.** "Milestone v0.2-M4 done. Final code-reviewer: PASS. Ready to cut v0.2.0-m4 tag? Invoke `/release` to bump versions, write CHANGELOG, tag, and push."

---

## Quality Checklist (verify at completion)

- [ ] All inputs validated (CLI args via clap; env vars range-checked at startup)
- [ ] No auth/authz applicable (dev tool, no surface)
- [ ] Error handling: specific messages (WHAT/WHAT-INSTEAD/WHY), no stack traces to user, proper exit codes (0/1/2)
- [ ] No SQL injection, XSS, path traversal, or secret exposure (no SQL; tempdir paths sanitized; no secrets)
- [ ] Performance: every ceiling in Invariants/Performance measured + documented in design/watch.md; ANY breach was profiled + fixed (not budget-raised)
- [ ] Tests: happy path + error cases + edge cases + 10k-cycle long-session + cross-platform CI
- [ ] Existing 1143+ tests still pass
- [ ] Types are complete (no `any` not applicable in Rust; no `Box<dyn Any>`; no excessive `.unwrap()` per coding-style.md)
- [ ] Follows existing codebase conventions (mirrors M2 `ynz-lsp` daemon pattern; mirrors M3 `ynz-fmt` library shape)
- [ ] Every phase received a code-reviewer PASS before committing
- [ ] Final cumulative code-reviewer sweep passed
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases

## Post-Ship Fixes (2026-05-20)

Seven bugs found immediately after v0.2.0-m4 shipped, all during real-world use on `trading-v4`. All fixed, committed, and pushed to main.

### Fix 1 — Text-mode rebuilds were completely silent on file save (c3fa69c)

**Root cause**: `rebuild_one_with_emitter` had no text-mode UI output. `print_building`, `print_success`, and `print_errors` were only called from `rebuild_one`, but `lib.rs` always routes through `rebuild_one_with_emitter`. Result: every file-change event compiled silently — UI was completely dark.

**Fix**: Added text-mode UI calls (`print_building` after no-change guard, `print_errors` before `finish_rebuild`, `print_success` after) in `rebuild_one_with_emitter`. Removed now-redundant `print_watching()` from `run_event_loop` start (doubles the idle prompt since every rebuild cycle already ends with it).

### Fix 2 — `notify 8.x` IN_OPEN feedback loop (c3fa69c)

**Root cause**: `notify 8.x` sets `WatchMask::OPEN` by default. Every rebuild calls `fs::read_to_string` which fires `IN_OPEN` → debouncer delivers it as `WatchEvent::Changed` → another rebuild → another open → infinite loop at ~1 rebuild/100ms.

**Fix**: Added `WatchDb::source_unchanged(path, text)` — checks if the on-disk content matches what's in the shadow DB. If identical, `rebuild_one_with_emitter` returns early (no UI, no compile). Added `force: bool` param to skip this guard for the initial build (shadow pre-populated by `from_target` before first compile). Event-triggered rebuilds pass `force: false`; initial build passes `force: true`.

### Fix 5 — Cross-module imports fail: source_by_path path mismatch (481c405 + e44d62b)

**Root cause**: Two-layer path mismatch between the watch DB and the import resolver. (1) `find_project_root` in watch's `project.rs` walked up relative paths and could return `""` (empty string) as the root when the user runs `ynz watch ships/scripts/backfill` from the project root. `canonicalize("")` fails on Linux and falls back to the empty string. (2) `collect_ynz_files` ran with the relative/empty root, storing file paths like `shared/contracts/marketData.ynz` (relative) in the DB. But `ynz-typeck::resolve_module_path` calls `std::fs::canonicalize` and returns absolute paths like `/workspaces/trading-v4/shared/contracts/marketData.ynz`. `source_by_path` key lookup always missed → "Module not registered."

**Fix**: Before calling `find_project_root`, join relative hint paths with `std::env::current_dir()` to guarantee an absolute starting point. The walk-up then only ever traverses absolute paths, returns an absolute root, and `canonicalize` succeeds. All stored source paths are now absolute canonical paths matching what the import resolver produces. Verified with a full multi-entry + cross-module import smoke test.

### Fix 4 — Terminal clear fires on IN_OPEN no-change skips, blanking output (c510769)

**Root cause**: `ui::clear` was called in `event_loop.rs` on EVERY `WatchEvent::Changed`, including the IN_OPEN no-change skips that return early from `rebuild_one_with_emitter`. Each skip cleared the visible terminal area without printing anything, producing a wall of blank space below error output.

**Fix**: Removed `ui::clear` from `event_loop.rs`. Moved it into `rebuild_one_with_emitter` right before `print_building`, after the `source_unchanged` no-change guard. Added `no_clear: bool` param threaded through `run_rebuild_cycle`. Initial build passes `no_clear=true` (don't wipe startup output); event-triggered rebuilds pass `config.no_clear` (honours `--no-clear` flag).

### Fix 3 — Walk up to find `yinz.toml` + `[entries]` multi-entry support (27b3c98)

**Root cause**: `resolve_project` only checked the exact path passed for `yinz.toml`. Yinz convention is that `yinz.toml` lives at the project root only — subdirectory paths like `ynz watch ships/scripts/backfill` always failed.

**Additional bug**: `parse_entry_from_toml` only handled `entry = "..."` (single-entry), not `[entries]` table format (multi-entry projects).

**Fix**: Added `find_project_root` that walks UP the directory tree to find `yinz.toml`. Added `parse_entries_table_from_toml` and `pick_entry_from_hint` — for multi-entry projects, the user's hint path (path components) is matched against entry values to pick the right entry. Three new tests: walk-up behavior, multi-entry selection, isolation.

### Fix 6 — Single-file path mode skips loading shared project files (3582754)

**Root cause**: When the user passes a `.ynz` file directly (`ynz watch tooling/x/entry.ynz`), `resolve_target` entered single-file mode and only registered that one file in the salsa DB. Cross-module imports resolved on disk but `source_by_path` always returned `None` because shared files were never registered.

**Fix**: Added `resolve_project_with_entry` — when a `.ynz` file is passed AND a `yinz.toml` exists anywhere above it, load all project files (full project mode) but use the explicitly-passed file as the entry point. True single-file mode (no `yinz.toml` anywhere above) is unchanged.

### Fix 7 — Watch linker missing `clang-18` probe and `-no-pie` flag (5a9f624 + ac9367c)

**Root cause 1**: `write_binary()` in `rebuild.rs` hardcoded `Command::new("cc")`. Devcontainers with `clang-18` but not `cc` hit `No such file or directory` on every non-check rebuild. `ynz build` already had `find_linker()` probing `["clang-18", "clang", "cc", "gcc", "g++"]` — watch was written independently and didn't reuse it.

**Root cause 2**: `write_binary()` also omitted the `-no-pie` linker flag that `ynz build` passes. LLVM emits non-PIC relocations; modern Linux distros default to PIE linking. Without `-no-pie` the linker fails with `R_X86_64_32 against .rodata.str1.16 can not be used when making a PIE object`. Both added to graveyard.

**Fix**: Replaced hardcoded `"cc"` with the same probe loop as `ynz build`. Added `-no-pie` flag, matching `build.rs:525`.

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase = one PR via `/pr` per Phase Execution Protocol. PR template in `.claude/skills/pr/` enforces.
- **Shadow main branches**: every branch (`chore/v0-2-m4-doc-lockdown`, `feat/v0-2-m4-file-watcher`, ...) merges to `main` via PR; no parallel mainline.
- **Building the engine before shipping value**: each phase delivers a visible increment. Phase 0 = doc + scaffold + new subcommand visible in --help; Phase 1 = file watcher fires events; Phase 2 = rebuilds work; Phase 3 = full default behavior. Anyone can stop after Phase 2 and have a working `ynz watch --check`.
- **Hotfix that isn't**: M4 is feature work, not a bug fix. No "hotfix" misuse.
- **Abandoned branches**: each phase's branch is opened, reviewed, merged within its own session. No long-lived branches outliving the phase.
- **Flag graveyards**: M4 introduces zero feature flags (no progressive rollout for compiler tooling). No flag-graveyard risk.

## Cross-References

- `~/.claude/skills/plan/SKILL.md` — the global /plan skill this plan follows
- `.claude/rules/plan-invariants.md` — 7-subsection Invariants block required
- `.claude/rules/auto-promotion.md` — auto-promotion analysis subsection requirement
- `.claude/rules/feature-registry.md` — registry consumer + producer rules
- `.claude/rules/inference.md` — dual-audience disclaimer (infer/inference allowed in design docs, banned in user-facing diagnostics)
- `.claude/rules/vocabulary.md` — Yinz user-facing terms
- `.claude/rules/non-oop.md` — Yinz is not OOP (watch is procedural; no class-style state)
- `.claude/rules/stdlib-design.md` — (not directly applicable; watch is not stdlib but the principles cross over)
- `~/.claude/memory/branching.md` — branch prefix + PR sizing
- `~/.claude/rules/verification.md` — Paper-Trace for any bug-fix work (none expected in M4 unless a Phase 5 issue surfaces)
- `.claude/plans/roadmaps/v0-2-dev-loop-tooling.md` — parent roadmap
- `.claude/plans/done/v0-2-m2-lsp-thin-slice.md` — daemon-with-CompilerDb pattern reference
- `.claude/plans/done/v0-2-m3-fmt.md` — library + CLI shape reference; cross-platform test patterns
- `design/compiler.md` — current bare watch paragraph (lines 138-146) being expanded
- `design/mvp-scope.md` — v0.2-M4 entry
- `design/teaching-mission.md` — WHAT/WHAT-INSTEAD/WHY format the watch diagnostics use
- `design/compiler-errors.md` — banned-jargon source-of-truth
- `design/compiler-language.md` — salsa-first architecture; "Why Salsa" section
