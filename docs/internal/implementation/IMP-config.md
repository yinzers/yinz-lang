---
name: "IMP-config"
description: "Design rationale for Yinz's three-layer configuration model (yinz.toml, .env, runtime set functions) and the choice of TOML for project config."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Configuration — Design Decisions

User spec: [`docs/reference/REF-config.md`](../../reference/REF-config.md)

---

## Three Layers

Configuration lives in three distinct layers:
1. `yinz.toml` — project metadata, dependencies, build settings (compile-time)
2. `.env` auto-loaded — secrets, connection strings, machine-specific values (runtime, never compiled in)
3. `set` functions in `entrypoint()` — runtime behavior (shutdown, threads, logging, error handling)

**Status:** layer 1 (`yinz.toml`) ships. Layers 2 and 3 are **designed, not built**: the registry has no `env` module and no `setShutdownTimeout`, `setShutdownHandler`, `setErrorHandler`, `setThreadPoolSize` or `setLogLevel` entry, so a program using them does not build.

**Why `.env` for secrets**: secrets and connection strings differ on every machine, so they never belong in code or the compiled binary; the same binary reads different `.env` values at runtime.

### Layer 2 design: `.env`

The program loads a `.env` file from the project root when it starts, with no setup or import. `.env` is git-ignored; a committed `.env.example` lists the variable names teammates need.

```
# .env
DATABASE_URL=postgres://localhost/mydb
PORT=3000
```

Values are read with `env.get(name)`, which returns `Maybe<String>`. A missing `.env` file is not an error: `env.get` returns `none` for every key and the program decides what to do.

```ynz
let foundPort = env.get(`PORT`)          // a maybe: the text, or nothing
let port = foundPort.or(`3000`)          // a plain String, `3000` if PORT is not set
```

### Layer 3 design: `set` functions

Settings are calls at the top of `entrypoint()`:

| Function | What it sets |
|---|---|
| `setShutdownTimeout(duration)` | how long the program may take to finish up after it is asked to stop |
| `setShutdownHandler(handler)` | code that runs when the program is asked to stop, such as saving data |
| `setErrorHandler(handler)` | code that runs when an error reaches the top and nothing else handled it (see [`IMP-errors.md`](IMP-errors.md)) |
| `setThreadPoolSize(count)` | how many threads the program may use at the same time |
| `setLogLevel(level)` | how much the program logs |

### What goes where

| What | Where | Example |
|------|-------|---------|
| Project name, version | `yinz.toml` | `name = "my-app"` |
| Dependencies | `yinz.toml` | `graphics = "1.2.0"` |
| Build target, optimization | `yinz.toml` | `optimization = "release"` |
| Secrets, API keys, DB URLs | `.env` (git-ignored) | `DATABASE_URL=postgres://...` |
| Required variable documentation | `.env.example` (committed) | `DATABASE_URL=` |
| Shutdown timeout, thread pool size | `entrypoint()` | `setThreadPoolSize(16)` |
| Error and shutdown behavior | `entrypoint()` | `setErrorHandler(error => { ... })` |
| Environment-specific logic | an `if` in `entrypoint()` | ``if (environment == `production`) { ... }`` |

**Why three layers**: Each layer has a different lifecycle, different audience, and different security requirements. Mixing them creates confusion about when values are read, who can see them, and what they affect.

---

## `yinz.toml` Is TOML

Project config is TOML: made for config files, allows comments, has explicit types with no implicit coercion, and is not whitespace-sensitive.

---

## No Environment-Specific `.env` Files

One `.env` per machine. No `.env.production`, `.env.staging`, `.env.local`.

**Why**: Environment-specific logic belongs in code, not in file naming conventions. Different machines have different `.env` files. Code branches on ``env.get(`ENV`)`` when behavior needs to differ. A static file naming scheme can't express conditional logic; code can.

---

## Runtime Config via `set` Functions, Not a Config File

Runtime behavior (`setShutdownTimeout`, `setThreadPoolSize`, `setErrorHandler`, `setLogLevel`) is configured through function calls in `entrypoint()`, not a config file.

**Why**: Runtime settings often depend on other values — environment variables, command-line arguments, conditions. A static config file can't branch on these. Code handles branching naturally:

```
let foundEnvironment = env.get(`ENV`)
let environment = foundEnvironment.or(`dev`)
if (environment == `production`) {
  setThreadPoolSize(32)
} else {
  setThreadPoolSize(4)
}
```

A static file cannot express this; code can.
