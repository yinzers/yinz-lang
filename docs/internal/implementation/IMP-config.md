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
3. `set` functions in `main()` — runtime behavior (shutdown, threads, logging, error handling)

**Why `.env` for secrets**: secrets and connection strings differ on every machine, so they never belong in code or the compiled binary; the same binary reads different `.env` values at runtime.

**Why three layers**: Each layer has a different lifecycle, different audience, and different security requirements. Mixing them creates confusion about when values are read, who can see them, and what they affect.

---

## `yinz.toml` Is TOML

Project config is TOML: made for config files, allows comments, has explicit types with no implicit coercion, and is not whitespace-sensitive.

---

## No Environment-Specific `.env` Files

One `.env` per machine. No `.env.production`, `.env.staging`, `.env.local`.

**Why**: Environment-specific logic belongs in code, not in file naming conventions. Different machines have different `.env` files. Code branches on `env.get("ENV")` when behavior needs to differ. A static file naming scheme can't express conditional logic; code can.

---

## Runtime Config via `set` Functions, Not a Config File

Runtime behavior (`setShutdownTimeout`, `setThreadPoolSize`, `setErrorHandler`, `setLogLevel`) is configured through function calls in `main()`, not a config file.

**Why**: Runtime settings often depend on other values — environment variables, command-line arguments, conditions. A static config file can't branch on these. Code handles branching naturally:

```
let foundEnvironment = env.get("ENV")
let environment = foundEnvironment.or("dev")
if (environment == "production") {
  setThreadPoolSize(32)
} else {
  setThreadPoolSize(4)
}
```

A static file cannot express this; code can.
