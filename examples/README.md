# Yinz Examples

This directory has two kinds of contents:

1. **Project-layout examples** — demonstrate the two valid Yinz project shapes (single-entry and multi-entry). Pick the one that matches your project.
2. **Diagnostic / tool demos** — files that exercise specific compiler behavior (error gallery, formatter input).

---

## Project layouts (pick one)

### `basics/` — single-entry layout (the v0.1 default)

One project, one `yinz.toml`, one entry point. Code organized into subfolders (`services/`, `utils/`) imported root-relatively. This is the canonical shape for ~95% of Yinz projects.

```
basics/
  yinz.toml             # entry = "entrypoint.ynz"
  entrypoint.ynz
  services/             # plain folder, imported as `services/...`
  utils/                # plain folder, imported as `utils/...`
```

Also serves as Yinz's **growing v0.1 language showcase** — every milestone (M1–M8) extends this project with the features it adds, so running `ynz run examples/basics/` exercises everything the language can do today. See `basics/README.md` for the per-milestone breakdown.

### `ships_demo/` — multi-entry layout (v0.22 preview)

One project, one `yinz.toml`, MULTIPLE entry points via `[entries]` table. Each entry is a **ship**; canonical folder convention is `ships/<name>/entrypoint.ynz`. Shared code lives in a sibling `shared/` (or similarly-named plain folder) imported root-relatively.

```
ships_demo/
  yinz.toml             # [entries] table — calculator + greeter
  shared/               # plain folder
  ships/
    calculator/entrypoint.ynz
    greeter/entrypoint.ynz
```

**Use multi-entry when** you genuinely have N binaries that ship together and share a dependency graph. **Use single-entry when** you have one binary (the common case).

The `[entries]` table is a v0.22 feature. Until v0.22 ships, multi-entry projects must split into N single-entry projects in separate repos OR use `ynz build <path>` to point at each entry manually (works today since the compiler accepts a path argument).

---

## Diagnostic + tool demos

### `errors/` — per-milestone error gallery

One file per milestone (`m1_errors.ynz`, `m2_errors.ynz`, ..., `m8_errors.ynz`). Each file deliberately triggers every compile-error class that milestone introduced. Running `ynz run examples/errors/m4_errors.ynz` produces every M4 diagnostic in one go — the canonical way to eyeball whether the teaching error messages are still good after a refactor.

These files are **not Yinz projects** (no `yinz.toml`). They're standalone scripts the compiler can process directly.

### `fmt_demo/` — formatter showcase

`messy.ynz` is intentionally badly formatted input. Run `ynz fmt examples/fmt_demo/messy.ynz` to see canonical Yinz formatting applied. Also not a project.

---

## Adding a new example

- **New language feature in M1–M8?** Extend `basics/entrypoint.ynz` per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery` — that's the canonical growth path. Don't create a new top-level demo for individual language features.
- **New stdlib module (v0.5+)?** Add a NEW per-module example project under `examples/<module>/` using the **single-entry layout** (mirror `basics/`'s shape). One module = one example project.
- **Demonstrating a project layout pattern?** Add to / amend `basics/` (single-entry) or `ships_demo/` (multi-entry). Don't introduce a third "layout example" — drift hurts more than additional coverage helps.
- **Demonstrating a tool behavior?** Add to `fmt_demo/` or create a new `<tool>_demo/` peer.
