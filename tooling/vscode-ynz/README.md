# Yinz Language Extension for VSCode

Syntax highlighting, inline diagnostics, autocomplete, and hover docs for `.ynz` files.

## What's new in v0.3.0-m8

- **Mixed CPU+I/O parallel-group hints** — a group can now MIX a number-crunching call and a
  waiting (I/O) call side by side; the compiler runs them at the same time through one shared
  continuation instead of the group declining to plain sequential order. Each member's muted
  comment is tagged per its OWN kind — only the crunching call says `— separate core`; the
  waiting call's comment omits it, since it overlaps on the event loop instead of a second core.
  Hover either one for the updated WHAT/WHAT-INSTEAD/WHY, which now explains that a group can mix
  both kinds of member and still overlap together.

## What's new in v0.3.0-m7

- **CPU parallel-group hints** — when the compiler auto-runs independent number-crunching
  calls on separate cores, every call in the group shows a muted comment:
  `// runs at the same time as line 12 — separate core`.  The hint appears for both CPU
  groups (M3d) and I/O-overlap groups (M3b), letting you see the compiler's full scheduling
  decision at a glance.  No source change required — the hint is informational only.
- **`wait` ordering-barrier explanation** — the warning for `wait` on a CPU-bound callee
  now explains that `wait` defeats parallelization by acting as an ordering barrier, not
  just that the `wait` has no effect.

## What's new in v0.3.0-m2

- **`wait` actually suspends** — `wait foo()` now suspends the calling function at a real state-machine boundary; the OS thread is freed for other tasks during the pause. Hover over `wait` to see the updated docs.
- **`sleepAsync(ms)` intrinsic** — non-blocking sleep. `wait sleepAsync(100)` suspends for 100ms without tying up a thread. Pairs with `sleepMs(ms)` (blocking, M1).
- **New warnings** — `wait` on a CPU-bound call (`wait print(...)`) shows a Tier 3 warning; `sleepAsync(...)` without `wait` shows a Tier 3 warning; state-machine-to-state-machine calls without `wait` show a guidance warning.
- **`background` routing distinction** — hover over `background` to see which thread pool the call targets (I/O pool for functions with `wait`, blocking pool for others).

## What's new in v0.3.0-m1

- **`background` actually runs on a separate thread** — `background fn(args)` now spawns on a background thread-pool runtime; main continues immediately. Hover over `background` to see the updated docs.
- **New safety errors** — `background fn(lend-param)` rejects mutable borrows across thread boundaries; `background`/`wait` in `--kernel` mode produce teaching errors.
- **Inline `.give` hint** — when `background fn(largeStruct.copy())` copies more than 64 bytes, an inline muted hint suggests `.give (transfers ownership; no copy)`.
- **`sleepMs(ms)` intrinsic** — synchronous thread sleep for demos and timing tests.

## What's new in v0.2.0

- **Go-to-definition** — Cmd+click any identifier to jump to its declaration (same file or cross-file)
- **Find All References** — right-click → "Find All References" lists every use-site across the project
- **Rename** — F2 on any symbol; all references update atomically
- **Format on save** — delegates to `ynz-fmt`; normalizes LF line endings
- **Inlay hints** — inline type annotations (`: int`), ownership modifiers (`share`/`lend`), auto-promotion hints
- **Code actions** — quick-fix lightbulb for every diagnostic with a WHAT-INSTEAD
- **Semantic tokens** — richer color differentiation: keywords / types / functions / variables
- **Doc-comment hover** — `///` doc comments appear in hover popups above the signature
- **Completion narrowing** — `score.` where `score: int` shows only int methods

## Features

- **Syntax highlighting** — keywords, deferred features (shown as illegal), banned keywords (shown as deprecated), strings, comments, literals
- **Inline diagnostics** — WHAT/WHAT-INSTEAD/WHY teaching content from the Yinz compiler
- **Autocomplete** — keywords, primitive methods, type constants, deferred features (marked deprecated)
- **Hover docs** — registry-sourced WHY content for every keyword, intrinsic, and deferred feature

## Prerequisites

Build the `ynz-lsp` binary from the repo root:

```bash
cargo build -p ynz-lsp --release
# copy to PATH or set yinz.server.path in VSCode settings
cp target/release/ynz-lsp ~/.local/bin/  # or any directory on your PATH
```

## Install

### Option A — From GitHub Release (current method)

Always-current stable URL (updates automatically with each release):

```bash
curl -L https://github.com/yinzers/yinz-lang/releases/latest/download/yinz-latest.vsix -o yinz-latest.vsix
code --install-extension yinz-latest.vsix
```

Or download a specific version from the [releases page](https://github.com/yinzers/yinz-lang/releases).

### Option B — VSCode Marketplace (coming soon)

Once published: search **"Yinz Language"** in the VSCode Extensions panel, or:

```
ext install yinz-lang.yinz
```

## Configuration

| Setting | Default | Description |
|---|---|---|
| `yinz.server.path` | `ynz-lsp` | Path to the `ynz-lsp` binary |

## Screenshots

Screenshots are tracked for a follow-up commit once the extension is installed and verified locally. Install the extension and open `examples/pirates-roster/entrypoint.ynz` to see syntax highlighting, hover docs, autocomplete, and inline diagnostics in action.
