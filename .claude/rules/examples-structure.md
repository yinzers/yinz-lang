# Examples Structure Rule

How `examples/` is organized and how new demos get added. This rule is **load-bearing** — it stops `examples/` from drifting back into a flat dump of unrelated demos.

**Load when**: adding any new example directory, renaming an existing one, writing a milestone plan that ships a new demo, or reviewing a PR that touches `examples/**`. Also applies when designing a new stdlib module — its per-module example project goes under `examples/<themed-name>/`.

**SSOT**: `examples/README.md` is the user-facing index. This rule file is the contributor-facing discipline behind it.

---

## The Three Locked Decisions

1. **Flat layout.** Every demo is a top-level folder under `examples/`. No nesting. No grouping subdirectories (`examples/projects/`, `examples/galleries/`, etc.).
2. **Pittsburgh-themed folder names.** Bridges, neighborhoods, foods, sports figures, Steel City history, n'at. Generic-tech names like `basics/`, `errors/`, `fmt_demo/`, `ships_demo/` are banned.
3. **`examples/` is NOT a workspace.** No `examples/yinz.toml` with `[workspace]` / `[entries]`. Each demo project has its own `yinz.toml`; galleries have no `yinz.toml`. Demos don't share dependency graphs because they're not shipped together — they're independent educational artifacts.

The Three Decisions were made on 2026-05-20 (this session). The first two are aesthetic + organizational; the third is a semantic correctness constraint (`[workspace]` means "binaries that ship together," which `examples/` deliberately is not).

---

## Two Categories of Examples

Every `examples/<name>/` is either:

### Project examples (have a `yinz.toml`)

Demonstrate a Yinz project layout. Each project picks ONE layout:

- **Single-entry** — one `yinz.toml` with `entry = "..."`, code in plain subfolders. The canonical shape for ~95% of Yinz projects. **Current canonical example: `pirates-roster/`.**
- **Multi-entry** — one `yinz.toml` with `[entries]` table, ships under `ships/`. v0.22 feature. **Current canonical example: `stadium-fleet/`.**

Both project examples MUST have a `README.md` at the project root explaining what layout they're demonstrating and what theme dresses the content.

### Galleries (no `yinz.toml`)

Loose `.ynz` files exercising specific compiler/tool behavior. Not Yinz projects. Examples:

- **`primantis-orders/`** — per-milestone compile-error gallery (one file per M1-M8 + v0.2-M1-M3).
- **`burgh-poem/`** — formatter input demo (`messy.ynz`).
- **`incline-watcher/`** — `ynz watch` file-watcher demonstration (v0.2-M4). Themed as the Duquesne Incline tracking its elevation.

Galleries SHOULD have a `README.md` explaining what's exercised and why.

---

## Naming Discipline

Folder names MUST be Pittsburgh-themed. Approved categories:

| Category | Examples |
|---|---|
| **Sports** | `pirates-roster`, `steelers-playbook`, `penguins-stats` |
| **Neighborhoods** | `strip-district`, `mt-washington`, `lawrenceville`, `bloomfield`, `squirrel-hill` |
| **Food** | `primantis-orders`, `pierogi-stand`, `kennywood-eats`, `heinz-pickles` |
| **Bridges** | `clemente-bridge`, `fort-pitt-bridge`, `smithfield-st` |
| **Industry/history** | `homestead-mill`, `carnegie-library`, `westinghouse-factory` |
| **Slang/culture** | `burgh-poem`, `yinzer-greetings`, `n-at-translator` |
| **Geography** | `three-rivers`, `monongahela-flow`, `the-point` |

**Format**: kebab-case (`pirates-roster`, not `piratesRoster` or `pirates_roster`) — matches Rust crate naming and most folder conventions.

**Banned**: generic-tech names (`basics`, `demo`, `examples-2`, `fmt_demo`, `ships_demo`, `errors`, `test`, `sample`, `feature-x`). Anything that doesn't pass the "Pittsburgh-flavored" sniff test is rejected.

If you genuinely can't find a fitting Pittsburgh term, escalate to Patrick. Don't ship a generic name as a workaround.

---

## When Adding a New Example — Checklist

- [ ] **Project or gallery?** Decide upfront. Affects whether there's a `yinz.toml`.
- [ ] **Pittsburgh-themed folder name?** Run it past the Naming Discipline categories. Don't use a generic name.
- [ ] **Single-entry or multi-entry?** (Projects only.) Default to single-entry unless the demo SPECIFICALLY exists to demonstrate multi-entry.
- [ ] **Top-level only?** No nesting under `examples/projects/` or similar. Each demo is its own top-level folder.
- [ ] **README.md present?** Every example has a README explaining its purpose + theme.
- [ ] **Updated `examples/README.md` index?** Listed under the right category (Projects vs Galleries) with a one-line description.
- [ ] **Updated `.claude/rules/plan-invariants.md`?** If the new example is part of the canonical growth path for milestones, the rule reflects that.

---

## Adding to an Existing Example

Most milestone work doesn't create a new top-level demo — it extends `pirates-roster/` or `primantis-orders/`. Per `.claude/rules/plan-invariants.md` `### Demo & Error Gallery`:

- **New language feature in M1–M8** → extend `pirates-roster/entrypoint.ynz` with a new section. Don't make a new top-level demo for individual language features.
- **New compile-error class** → add an intentional trigger to the matching milestone file in `primantis-orders/` with a `// WHY:` comment naming the diagnostic class.

---

## v0.5+ Stdlib Module Examples

When v0.5+ stdlib modules ship, EACH gets its own per-module example project at `examples/<themed-name>/`. Rules:

- Use the **single-entry layout** (mirror `pirates-roster/`'s shape). No multi-entry unless the module's API specifically benefits from it.
- Folder name is Pittsburgh-themed per Naming Discipline above. The module name does NOT need to appear in the folder name — the theme is the convention.
- One stdlib module = one example project. Don't bundle multiple modules into one demo unless they're already a tight pair/trio (e.g., `file`+`path`+`directory`, `date`+`duration`).

Example future names: `incline-tracker` (date/duration v0.9 demo), `pierogi-stand` (file v0.5 demo), `kennywood-orders` (db v0.10 demo).

---

## Enforcement

This rule is **load-bearing**, not advisory. Violations should be caught at plan-review time or code-review time:

- Plan reviewer checks: does the milestone propose a new example? If yes, does the folder name pass the Naming Discipline?
- Code reviewer checks: does any PR add `examples/<generic-name>/`? Push back.
- Future: a Bouncer entry that greps for `examples/<generic-name>` patterns (if drift becomes a problem).

---

## Cross-References

- `examples/README.md` — user-facing index (SSOT for what each example demonstrates)
- `.claude/rules/plan-invariants.md` `### Demo & Error Gallery` — milestone-phase obligation to extend `pirates-roster/entrypoint.ynz` + the milestone's `primantis-orders/m{N}_errors.ynz`
- `design/open-questions.md` "Workspace / Multi-Package Projects" — why `examples/` isn't a workspace
- `design/mvp-scope.md` v0.22 — multi-entry layout source-of-truth
