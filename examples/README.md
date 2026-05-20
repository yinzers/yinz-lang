# Yinz Examples

Demos live here. Two categories:

1. **Project examples** — full Yinz projects (have a `yinz.toml`). Demonstrate the two valid project layouts.
2. **Galleries** — loose `.ynz` files that exercise specific compiler/tool behavior. NOT Yinz projects.

All folders are **Pittsburgh-themed** by convention — see `.claude/rules/examples-structure.md` for the rules around adding new ones.

---

## Project examples (each has a `yinz.toml`)

### `pirates-roster/` — single-entry layout (the v0.1 default)

One project, one `yinz.toml`, one entry point. Code organized into subfolders (`services/`, `utils/`) imported root-relatively. The canonical shape for ~95% of Yinz projects.

```
pirates-roster/
  yinz.toml             # entry = "entrypoint.ynz"
  entrypoint.ynz
  services/             # plain folder, imported as `services/...`
  utils/                # plain folder, imported as `utils/...`
```

**Doubles as the v0.1 language showcase** — every milestone (M1–M8) extends this project with the features it adds. Running `ynz run examples/pirates-roster/` exercises everything the language can do today. See `pirates-roster/README.md` for the per-milestone breakdown.

### `stadium-fleet/` — multi-entry layout (v0.22 preview, not yet buildable)

One project, one `yinz.toml`, MULTIPLE entry points via `[entries]` table. Each entry is a **ship**; canonical folder convention is `ships/<name>/entrypoint.ynz`. Shared code lives in a sibling `shared/` (or similarly-named plain folder) imported root-relatively.

```
stadium-fleet/
  yinz.toml             # [entries] table — multiple ships
  shared/               # plain folder
  ships/
    <ship-a>/entrypoint.ynz
    <ship-b>/entrypoint.ynz
```

**Use multi-entry when** you genuinely have N binaries that ship together and share a dependency graph. **Use single-entry when** you have one binary (the common case — pick `pirates-roster/`'s shape).

The `[entries]` table is a v0.22 feature. Pre-v0.22, multi-entry projects must split into N single-entry projects in separate repos OR use `ynz build <path>` to point at each entry manually.

---

## Galleries (loose files, no `yinz.toml`)

### `primantis-orders/` — per-milestone compile-error gallery

One `.ynz` file per milestone (`m1_errors.ynz` through `m8_errors.ynz` plus `v0_2_m{1,2,3}_errors.ynz`). Each file deliberately triggers every compile-error class that milestone introduced. Running `ynz build examples/primantis-orders/m4_errors.ynz` (or any other) produces every diagnostic for that milestone in one go — the canonical way to eyeball whether the teaching error messages are still good after a refactor.

Themed around Primanti's-style restaurant orders going wrong (wrong toppings, missing ingredients, the kitchen catching fire) because that's a Pittsburgh universal.

### `burgh-poem/` — formatter showcase

`messy.ynz` is intentionally badly-formatted Yinz code. Run `ynz fmt examples/burgh-poem/messy.ynz` to see canonical Yinz formatting applied (or `ynz fmt --check` to see the unified diff). Not a project — single file.

### `watch-demo/` *(coming with v0.2-M4)*

The `ynz watch` file-watcher demo. Ships when the M4 watch milestone wraps.

---

## Adding a new example

1. **Pick a Pittsburgh-themed folder name.** Bridges, neighborhoods, foods, sports figures, Steel City history, n'at. See `.claude/rules/examples-structure.md` for the discipline.
2. **Decide kind**: project (gets a `yinz.toml`, single-entry by default) OR gallery (loose files).
3. **For new language features in M1–M8**: extend `pirates-roster/entrypoint.ynz` per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery`. Don't create a new top-level demo for individual language features — `pirates-roster/` is the canonical growth path.
4. **For new stdlib modules (v0.5+)**: add a NEW per-module example project using the single-entry layout (mirror `pirates-roster/`'s shape). One module = one example project. Pick a Pittsburgh-themed name for the folder.
5. **For demonstrating a project layout pattern**: amend `pirates-roster/` (single-entry) or `stadium-fleet/` (multi-entry). Don't introduce a third layout example.
6. **For new error classes**: add intentional triggers to the corresponding milestone file in `primantis-orders/` per the gallery's existing pattern.
7. **For tool behavior demos** (formatter, watch, lsp): add to the matching gallery folder or create a peer one with a themed name.

The structure is **flat**. Do NOT make `examples/` itself a multi-entry workspace (`[workspace]` semantically means "binaries that ship together," which examples never are). Each demo is its own top-level folder.
