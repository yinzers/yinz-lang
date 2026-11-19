# `examples/incline-watcher/` — `ynz watch` Demo

**Layout: gallery** (one `entrypoint.ynz` plus a minimal `yinz.toml`). It has a `yinz.toml` only because `ynz watch <dir>` needs a project root to watch; it is not a language showcase like `pirates-roster/`.

**Theme:** the Duquesne Incline tracking its current elevation up Mt Washington.

## What it exercises

The rebuild-on-save loop of `ynz watch`: you edit the elevation line in `entrypoint.ynz` and save; the daemon rebuilds and re-runs the program. The program is a single `entrypoint()` that prints one line, so any delay or output change you see comes from the watcher, not the code.

## How to run

```bash
ynz watch examples/incline-watcher/              # rebuild and re-run on every save
ynz watch examples/incline-watcher/ --json       # structured-event output
ynz watch examples/incline-watcher/ --check      # rebuild only, skip the execute step
ynz watch examples/incline-watcher/ --no-clear   # keep terminal scrollback between runs
```

Watch design: [`docs/internal/implementation/IMP-watch.md`](../../docs/internal/implementation/IMP-watch.md).
