# `examples/stadium-fleet/` — Multi-Entry Project Layout Preview (v0.22)

**Layout: multi-entry project** (N binaries that ship from one codebase). For the canonical single-entry shape that the other ~95% of Yinz projects use, see `../pirates-roster/` instead.

**Theme:** stadium-services fleet — `scoreboard` tallies the runs each inning; `concessions` broadcasts the food specials over the PA. Both ships sail out of the same Pittsburgh ballpark, sharing one `yinz.toml`, one dependency graph, and the helpers under `shared/`.

> **Status: v0.22 preview, NOT YET BUILDABLE.** This directory demonstrates the proposed multi-entry project layout that lands with the package manager in v0.22. Today's compiler accepts only a single `entry = "..."` in `yinz.toml`. The `[entries]` table shown here is the v0.22 target schema — `ynz build` from this directory currently fails. Read these files as documentation-by-example, not as runnable code.

---

## The metaphor

A **project** is a fleet. Each **ship** is one named entry point — a binary you ship from this codebase. Ships sail under one flag (one `yinz.toml`, one shared dependency graph, one lockfile). Shared code is just plain folders; nothing extra to learn.

A ship is just a folder under `ships/` containing an `entrypoint.ynz`. That's it. No per-ship config file, no per-ship name/version, no workspace member glob. The `ships/` folder is a **convention**, not enforced — you can name it anything (`apps/`, `services/`, `bin/`) and the compiler doesn't care. But the canonical Yinz convention is `ships/`.

## This demo

```
stadium-fleet/
  yinz.toml                    # single root toml — names all entry points
  shared/                      # plain folder of shared code, imported root-relatively
    scoring.ynz
    announcements.ynz
  ships/                       # convention: this folder holds ship entry points
    scoreboard/
      entrypoint.ynz           # ynz build scoreboard → produces ./scoreboard binary
    concessions/
      entrypoint.ynz           # ynz build concessions → produces ./concessions binary
```

**The root `yinz.toml`** lists every ship by name and points to its entry file:

```toml
name = "stadium-fleet"
version = "0.1.0"

[entries]
scoreboard  = "ships/scoreboard/entrypoint.ynz"
concessions = "ships/concessions/entrypoint.ynz"
```

**Cross-folder imports** use root-relative paths from the project root (where `yinz.toml` lives):

```
// ships/scoreboard/entrypoint.ynz
import { sum, max } from `shared/scoring`
```

The path `shared/scoring` resolves to `<project_root>/shared/scoring.ynz`. No workspace member lookup, no package name resolution — just file-tree paths.

## Build commands (v0.22 target)

```
ynz build                       # builds every entry in [entries]
ynz build scoreboard            # builds just the scoreboard ship
ynz run scoreboard              # builds + runs the scoreboard ship
ynz build scoreboard --release  # release build of just the scoreboard ship
```

When there's only ONE entry (single-`entry =` mode, the v0.1 default), `ynz build` with no arg just builds the one binary — same as today. The `[entries]` table activates the multi-ship behavior.

## The planned design

- **One dependency graph:** every ship shares the root `[dependencies]` table, and tree shaking keeps each binary to what its entry imports.
- **One lockfile:** `yinz.lock` at the root.
- **No package names:** an import path such as ``shared/scoring`` is a file path.
- **No per-ship versions:** projects that need independent versions are separate Yinz projects.

See [`docs/internal/scratchpad/SCRATCH-open-questions.md`](../../docs/internal/scratchpad/SCRATCH-open-questions.md) "Workspace / Multi-Package Projects (v0.22+)" for the locked design rationale.
