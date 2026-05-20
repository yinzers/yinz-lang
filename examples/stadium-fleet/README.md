# `examples/stadium-fleet/` — Multi-Entry Project Layout Preview (v0.22)

**Layout: multi-entry project** (N binaries that ship from one codebase). For the canonical single-entry shape that the other ~95% of Yinz projects use, see `../pirates-roster/` instead.

**Theme (in-progress):** stadium-services fleet — scoreboard, concessions, ticketing as ships running out of one Pittsburgh ballpark. Today's calculator/greeter content stands in until the stadium-themed rewrite lands.

> **Status: v0.22 preview, NOT YET BUILDABLE.** This directory demonstrates the proposed multi-entry project layout that lands with the package manager in v0.22. Today's compiler accepts only a single `entry = "..."` in `yinz.toml`. The `[entries]` table shown here is the v0.22 target schema — `ynz build` from this directory currently fails. Read these files as documentation-by-example, not as runnable code.

---

## The metaphor

A **project** is a fleet. Each **ship** is one named entry point — a binary you ship from this codebase. Ships sail under one flag (one `yinz.toml`, one shared dependency graph, one lockfile). Shared code is just plain folders; nothing extra to learn.

A ship is just a folder under `ships/` containing an `entrypoint.ynz`. That's it. No per-ship config file, no per-ship name/version, no workspace member glob. The `ships/` folder is a **convention**, not enforced — you can name it anything (`apps/`, `services/`, `bin/`) and the compiler doesn't care. But the canonical Yinz convention is `ships/`.

## This demo

```
ships_demo/
  yinz.toml                    # single root toml — names all entry points
  shared/                      # plain folder of shared code, imported root-relatively
    math.ynz
    strings.ynz
  ships/                       # convention: this folder holds ship entry points
    calculator/
      entrypoint.ynz           # ynz build calculator → produces ./calculator binary
    greeter/
      entrypoint.ynz           # ynz build greeter    → produces ./greeter binary
```

**The root `yinz.toml`** lists every ship by name and points to its entry file:

```toml
name = "ships-demo"
version = "0.1.0"

[entries]
calculator = "ships/calculator/entrypoint.ynz"
greeter    = "ships/greeter/entrypoint.ynz"
```

**Cross-folder imports** use root-relative paths from the project root (where `yinz.toml` lives):

```
// ships/calculator/entrypoint.ynz
import { sum, max } from `shared/math`
```

The path `shared/math` resolves to `<project_root>/shared/math.ynz`. No workspace member lookup, no package name resolution — just file-tree paths.

## Build commands (v0.22 target)

```
ynz build                      # builds every entry in [entries]
ynz build calculator           # builds just the calculator ship
ynz run calculator             # builds + runs the calculator ship
ynz build calculator --release # release build of just the calculator ship
```

When there's only ONE entry (single-`entry =` mode, the v0.1 default), `ynz build` with no arg just builds the one binary — same as today. The `[entries]` table activates the multi-ship behavior.

## Why this shape instead of per-ship yinz.toml files

TypeScript monorepos and Rust workspaces both proliferate config files — every nested package gets its own manifest, every package needs an entry in the workspace root, every shared dep gets duplicated. The cost compounds: refactors touch 10+ tomls, deps drift, lockfiles fragment.

Yinz's "fancy import setup" (root-relative paths + tree shaking) removes the structural reasons to split config across folders:

1. **One dep graph** — every ship in the project shares the same `[dependencies]` table. Tree shaking ensures each binary only contains what its entry actually imports; listing a dep at the root doesn't bloat unused binaries.
2. **One lockfile** — `yinz.lock` at the root, one resolution pass for the whole project, no per-folder drift.
3. **No package names to maintain** — `import { X } from "shared/math"` is a file path, not a package name. Renaming a folder doesn't require updating package.json in 12 places.

Per-ship versioning is the only thing this loses vs the per-toml model, and it isn't load-bearing for the 95% case (one repo = one version cadence). Projects that genuinely need per-ship independent versioning can ship separate Yinz projects.

## Why "ships"

- Fleet metaphor matches the multi-entry-from-one-project model — each ship is its own binary that sails independently to its users, but they're all built and shipped from the same shipyard.
- Pirates-flavored (Pittsburgh Pirates → Yinz's hometown team).
- 5 letters, easy to type, hard to typo.
- CLI ergonomics: `ynz build calculator` reads naturally (the entry NAMES read like ships' names).
- The `ships/` folder convention reads as documentation — anyone seeing the layout instantly knows "these are this project's binaries."

See `design/open-questions.md` "Workspace / Multi-Package Projects (v0.22+)" for the locked design rationale.
