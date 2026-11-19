# `examples/burgh-poem/` — Formatter Showcase

**Layout: gallery** (one loose `.ynz` file, no `yinz.toml`). Not a Yinz project.

**Theme:** a Pittsburgh fan greeting, counting beers, and measuring the distance between two stadium seats.

## What it exercises

`messy.ynz` is intentionally badly formatted but valid Yinz: functions and a shape with stray spaces, missing spaces around `:` and operators, and padded parentheses. It gives `ynz fmt` plenty to rewrite, so you can see the zero-config canonical format applied to real code. The file stays non-canonical on purpose; it is the formatter's demo fixture.

## How to run

```bash
ynz fmt examples/burgh-poem/messy.ynz           # rewrites the file in canonical form
ynz fmt --check examples/burgh-poem/messy.ynz   # prints the diff, exits 1 if not canonical
```

Run `git checkout examples/burgh-poem/messy.ynz` afterwards to restore the messy original.

## Why there is no `yinz.toml`

With no project root, `ynz fmt --all` skips this folder, so the fixture is never reformatted by accident. Formatter design: [`docs/internal/implementation/IMP-fmt.md`](../../docs/internal/implementation/IMP-fmt.md).
