---
name: Yinz Examples Structure Protocol
description: "Applies when adding, renaming, organizing or extending a demo or error gallery under examples/."
paths:
  - examples/**
---

- **Keep the layout flat:** every demo is a top-level folder under `examples/`, with no grouping subdirectories. Why: demos are independent, and nesting turns the folder into a dump.
- **Name each demo in kebab-case with a Pittsburgh theme:** sports (`pirates-roster`), neighborhoods, food (`primantis-orders`), bridges, industry and history, local slang, geography. Generic tech names (`basics`, `demo`, `sample`, `test`) are banned. A name outside these categories needs Chief's confirmation. Why: a theme makes demos memorable, and a generic name invites a dumping ground.
- **Treat `examples/` as no workspace:** there is no `examples/yinz.toml`, and each project has its own. Why: a workspace means binaries that ship together, and demos are independent teaching artifacts.
- **Make each demo a project or a gallery.** Decide before creating the folder. Why: it decides whether a `yinz.toml` exists.
  - **Project:** has a `yinz.toml`. Single-entry (one `entry`, code in plain subfolders) is the only buildable layout, so new projects use it, modeled on `pirates-roster`. Why: multi-entry is not supported by build or run.
  - **Gallery:** loose `.ynz` files showing one tool or compiler behavior. It has no `yinz.toml` unless the tool needs one (`ynz watch <dir>` does). Why: a gallery is not a project.
- **Give every example a README that says what it shows,** and for a project the theme too. Why: a demo with no stated purpose cannot be maintained.
- **List each new example in the examples index under Projects or Galleries, with a one-line description.** Why: the index is how readers find demos.
- **Give each stdlib module its own single-entry project;** the folder name need not contain the module name. Bundle modules only for a tight pair or trio (`file`, `path`, `directory`). Why: one project per module keeps each demo focused.
- **Extend the canonical demo for a new language feature:** put the feature in `examples/pirates-roster/entrypoint.ynz`, in a realistic context doing real work (not `print(featureName())`), instead of adding a new top-level demo. Why: one growing project shows the language as a whole.
- **A work item that adds compile-error classes gets one gallery file in `examples/primantis-orders/`, named `<version>_<slug>_errors.ynz`,** with each intentional error carrying a `// WHY:` comment that names the diagnostic class. Register the file in `crates/ynz-driver/tests/error_galleries.rs`, and leave existing gallery files as they are. Why: the test lists files by hand, so an unregistered file is never run, and one run of the file shows every diagnostic it holds.
- **Check the demo byte-exact against `examples/pirates-roster/expected_stdout.txt`** in `crates/ynz-driver/tests/integration.rs`, and regenerate that golden with `examples/pirates-roster/expected_stdout.txt.regenerate.sh` when the demo changes on purpose. **Check galleries by diagnostic count plus key phrases** in `error_galleries.rs`. Use no `insta` snapshots for either. Why: byte-exact output catches any drift in what users see, and count plus phrases catch a lost diagnostic without breaking on rewording.
- **Give a feature that has not shipped a placeholder comment in the demo** (`// arena scratch { ... } — not shipped yet`), and replace it with the real example when it ships. Why: the demo stays honest about what is coming.
