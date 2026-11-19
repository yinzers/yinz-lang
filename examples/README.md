# Yinz Examples

Demos live here. Two categories:

1. **Project examples** — full Yinz projects (have a `yinz.toml`). Single-entry (one `entry`, code in plain subfolders) is the only buildable layout, so every new project uses it.
2. **Galleries** — loose `.ynz` files that exercise specific compiler/tool behavior. NOT Yinz projects; a gallery has a `yinz.toml` only when its tool needs one (`ynz watch <dir>` does).

All folders are **Pittsburgh-themed** by convention — see [`.claude/rules/examples-structure.md`](../.claude/rules/examples-structure.md) for the rules around adding new ones.

---

## Project examples (each has a `yinz.toml`)

### `pirates-roster/` — single-entry layout (the only buildable layout)

One project, one `yinz.toml`, one entry point. Code organized into subfolders (`services/`, `utils/`) imported root-relatively. The canonical shape for ~95% of Yinz projects.

```
pirates-roster/
  yinz.toml             # entry = "entrypoint.ynz"
  entrypoint.ynz
  services/             # plain folder, imported as `services/...`
  utils/                # plain folder, imported as `utils/...`
```

**Doubles as the v0.1 language showcase** — every milestone (M1–M8) extends this project with the features it adds. Running `ynz run examples/pirates-roster/` exercises everything the language can do today. See `pirates-roster/README.md` for the per-milestone breakdown.

### `stadium-fleet/` — multi-entry layout (not buildable; do not copy)

A preview of a multi-entry project (`[entries]` table). `ynz build` and `ynz run` do not support multi-entry yet, so this folder is documentation by example only. New projects use the single-entry layout of `pirates-roster/`.

---

## Galleries (loose files, no `yinz.toml` unless their tool needs one)

### `primantis-orders/` — compile-error gallery

One `.ynz` file per work item that adds compile-error classes, named `<version>_<slug>_errors.ynz` (older files are named `m1_errors.ynz` through `m8_errors.ynz` and `v0_2_m{1,2,3}_errors.ynz`). Each file deliberately triggers every compile-error class that work item introduced. Running `ynz build examples/primantis-orders/m4_errors.ynz` (or any other) produces every diagnostic in that file in one go — the canonical way to eyeball whether the teaching error messages are still good after a refactor.

Themed around Primanti's-style restaurant orders going wrong (wrong toppings, missing ingredients, the kitchen catching fire) because that's a Pittsburgh universal.

### `burgh-poem/` — formatter showcase

`messy.ynz` is intentionally badly-formatted Yinz code. Run `ynz fmt examples/burgh-poem/messy.ynz` to see canonical Yinz formatting applied (or `ynz fmt --check` to see the unified diff). Not a project — single file. See [`burgh-poem/README.md`](burgh-poem/README.md).

### `incline-watcher/` — `ynz watch` file-watcher demo

Themed as the Duquesne Incline tracking its current elevation up Mt Washington. Edit the elevation in `entrypoint.ynz` and save; `ynz watch examples/incline-watcher/` rebuilds and re-executes within a second. Try `--json` for structured-event output, `--check` to skip the execute step, `--no-clear` to preserve scrollback. It carries a `yinz.toml` because `ynz watch <dir>` needs one. See [`incline-watcher/README.md`](incline-watcher/README.md).

---

## Adding a new example

1. **Pick a Pittsburgh-themed folder name.** Bridges, neighborhoods, foods, sports figures, Steel City history, n'at. Keep the layout flat: every example is a top-level folder under `examples/`, with no grouping subfolders and no `examples/yinz.toml`.
2. **Decide kind**: project (gets a `yinz.toml`, single-entry by default) OR gallery (loose files; no `yinz.toml` unless its tool needs one).
3. **For new language features (every milestone — the demo grows each time)**: extend `pirates-roster/entrypoint.ynz` with the new feature in context, in every phase that adds executable surface. Don't create a new top-level demo for individual language features — `pirates-roster/` is the canonical growth path.
4. **For new stdlib modules (v0.5+)**: add a NEW per-module example project using the single-entry layout (mirror `pirates-roster/`'s shape). One module = one example project; bundle modules only for a tight pair or trio (`file`, `path`, `directory`). Pick a Pittsburgh-themed name for the folder.
5. **For demonstrating a project layout pattern**: amend `pirates-roster/`. Don't add a second layout example.
6. **For new error classes**: add one new gallery file in `primantis-orders/`, named `<version>_<slug>_errors.ynz`, with each intentional error carrying a `// WHY:` comment that names the diagnostic class. Register the file in `crates/ynz-driver/tests/error_galleries.rs` (the test lists files by hand, so an unregistered file is never run) and leave existing gallery files as they are.
7. **For tool behavior demos** (formatter, watch, lsp): add to the matching gallery folder or create a peer one with a themed name.

The structure is **flat**. Do NOT make `examples/` itself a multi-entry workspace (`[workspace]` semantically means "binaries that ship together," which examples never are). Each demo is its own top-level folder.
