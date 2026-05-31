# Changelog

All notable changes to the Yinz Language extension are documented here.

## [0.3.0-m2] — 2026-05-31

### Added

- **`wait` actually suspends** — hover over the `wait` keyword to see the v0.3-M2 docs (WHAT: "Suspends the calling function until the awaited expression completes. The OS thread is freed for other tasks during the suspension."). The v0.3-M1 placeholder text is gone; the real semantics are now live.
- **`sleepAsync(ms)` intrinsic** — non-blocking sleep. `wait sleepAsync(100)` suspends the calling function for 100ms without blocking the OS thread. Appears in autocomplete; hover shows the new may-block intrinsic doc.
- **New warnings**:
  - `wait print("hi")` → `WaitOnNonMayBlockWarning` (Tier 3 yellow squiggle): "The function `print` does not contain a suspension point; `wait` here changes nothing."
  - `wait 42` → `WaitOnNonCallExpression` (error): "`wait` must be followed by a function call."
  - `sleepAsync(100)` without `wait` → `UnawaitedSleepAsync` (Tier 3 warning): "`sleepAsync` creates a sleep handle but discards it without waiting."
  - State-machine fn calling state-machine fn without `wait` → `WaitRequiredOnStateMachineCall` (Tier 3 warning): guides toward writing `wait`; program still runs correctly via sync bridge.
- **`background` routing-distinction note** — hover over `background` shows which pool each call routes to: functions with `wait` go to the I/O pool; functions without `wait` go to the blocking pool.

### Screenshots

See `screenshots/wait-suspension.png.PLACEHOLDER` — full screenshot captured at release time (requires a live VSCode instance; not available in CI).

## [0.3.0-m1] — 2026-05-21

### Added

- **`background` runs on a separate thread** — hover over the `background` keyword to see updated v0.3-M1 docs (WHAT: "Runs the function on a separate thread"; WHAT INSTEAD: correct call forms; WHY: prior v0.2 behavior was sequential).
- **`wait` hover docs updated** — hover doc explains M1 semantics (synchronous; state-machine suspension arrives in v0.3-M2).
- **New compile errors**:
  - `background fn(lend-param)` → lend-cross-thread safety error with `.give`/`.copy()` fix suggestion.
  - `background fn(...)` in `--kernel` mode → error with explanation.
  - `wait expr` in `--kernel` mode → error with explanation.
- **New inlay hint** — `background fn(largeStruct.copy())` where estimated copy size > 64 bytes shows `.give (transfers ownership; no copy)` muted annotation inline.
- **Large-copy warning** — Tier 3 lint warning (yellow) on `.copy()` args > 64 bytes at `background` call sites.

### Screenshots

See `screenshots/background-concurrent.png` for the hover doc and inline hint in action.

## [0.2.0-m2] — 2026-05-20

Initial release (preview).

### Added

- Syntax highlighting for `.ynz` files: keywords, deferred features (illegal), banned declaration keywords (deprecated), strings, numbers, comments
- Inline diagnostics: Yinz compiler errors displayed as red squiggles with full WHAT/WHAT-INSTEAD/WHY teaching content
- Autocomplete: keywords, primitive methods filtered by receiver type, type-attached constants, deferred features (shown as deprecated)
- Hover docs: registry-sourced WHY content for every keyword, primitive intrinsic, type constant, deferred feature, and banned keyword
- TextMate grammar derived automatically from the Yinz feature registry — new keywords and features appear in the editor on rebuild
- Language association for `.ynz` files
- `yinz.server.path` configuration setting to point at a custom `ynz-lsp` binary location
