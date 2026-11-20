---
name: "REF-packages"
description: "Yinz has no package commands. Share code between files with modules."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Packages

`ynz` has no package commands: there is no `ynz add`, `ynz remove`, `ynz update` or `ynz publish`, and the compiler ignores a `[dependencies]` section in `yinz.toml`.

To share code, put it in its own file and import it. See [Modules](REF-modules.md). To see which commands exist, see [Tooling](REF-tooling.md).
