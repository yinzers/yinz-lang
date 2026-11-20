---
name: "REF-config"
description: "A yinz.toml file in your project root names your project and says which file starts the program."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Configuration

A `yinz.toml` file in your project root names your project and says which file starts the program. The compiler reads it automatically — no flags, no setup.

---

## yinz.toml

The file has one `[project]` section with three fields:

```toml
# yinz.toml

[project]
name = "my-app"
version = "1.0.0"
entry = "entrypoint.ynz"
```

| Field | What it does | Default |
|-------|--------------|---------|
| `name` | The name of your project | The name of the project folder |
| `version` | The version of your project | `0.0.0` |
| `entry` | The file that holds `function entrypoint()` | `entrypoint.ynz` |

If a field is missing, the compiler uses the default. See [Tooling](REF-tooling.md) for the commands that read this file.

---

## Fields the compiler does not know

The compiler skips any field other than the three above and prints a warning, so a typo does not stop your build:

```toml
[build]
target = "native"
```

```text
Warning: Unknown field `target` in yinz.toml — ignored.
```

A line without an `=` gets a warning too: ``Warning: Unrecognized line in yinz.toml: `…` — ignored.``
