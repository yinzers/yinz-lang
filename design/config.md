# Configuration — Design Decisions

User spec: `spec/config.md`

---

## Three Layers

Configuration lives in three distinct layers:
1. `yinz.toml` — project metadata, dependencies, build settings (compile-time)
2. `.env` auto-loaded — secrets, connection strings, machine-specific values (runtime, never compiled in)
3. `set` functions in `main()` — runtime behavior (shutdown, threads, logging, error handling)

**Why three layers**: Each layer has a different lifecycle, different audience, and different security requirements. Mixing them creates confusion about when values are read, who can see them, and what they affect.

---

## TOML over JSON or YAML

**Why not JSON**: No comments — can't annotate config. Missing trailing commas cause cryptic errors. Every key needs quotes. Designed for machine data exchange, not human editing.

**Why not YAML**: Whitespace-sensitive — indentation errors cause silent failures. Implicit type coercion: `no` becomes boolean `false`, `NO` (Norwegian country code) becomes `false`. Has caused real production outages. Not worth the risk for a config file format.

**Why TOML**: Designed specifically for config files. Comments allowed. Explicit types with no implicit coercion. Not whitespace-sensitive. Used by Rust (Cargo.toml), Python (pyproject.toml). Clean and predictable.

---

## No Environment-Specific `.env` Files

One `.env` per machine. No `.env.production`, `.env.staging`, `.env.local`.

**Why**: Environment-specific logic belongs in code, not in file naming conventions. Different machines have different `.env` files. Code branches on `env.get("ENV")` when behavior needs to differ. A static file naming scheme can't express conditional logic; code can.

---

## Runtime Config via `set` Functions, Not a Config File

Runtime behavior (`setShutdownTimeout`, `setThreadPoolSize`, `setErrorHandler`, `setLogLevel`) is configured through function calls in `main()`, not a config file.

**Why**: Runtime settings often depend on other values — environment variables, command-line arguments, conditions. A static config file can't branch on these. Code handles branching naturally:

```
if (env.get("ENV").or("dev") == "production") {
  setThreadPoolSize(32)
} else {
  setThreadPoolSize(4)
}
```

A JSON/TOML file cannot express this. Code already can.
