# Configuration

Configuration in Yinz lives in three places, each with a specific job.

---

## Layer 1: Project config — `yinz.toml`

A `yinz.toml` file in your project root defines metadata, dependencies, and build settings. The compiler reads it automatically — no flags, no setup.

```toml
# yinz.toml

[project]
name = "my-app"
version = "1.0.0"
entry = "entrypoint.ynz"

[build]
target = "native"           # native, wasm
optimization = "release"    # debug, release

[dependencies]
graphics = "1.2.0"
physics = "0.9.0"
```

`yinz.toml` is for things that don't change between environments — your project's identity and what it depends on.

---

## Layer 2: Environment variables — `.env`

Yinz automatically loads a `.env` file from your project root when the program starts. No setup, no imports, just create the file.

```
# .env — put this in .gitignore, never commit it
DATABASE_URL=postgres://localhost/mydb
PORT=3000
SECRET_KEY=abc123
STRIPE_KEY=sk_test_...
```

```
# .env.example — commit this to show teammates what vars are needed
DATABASE_URL=
PORT=3000
SECRET_KEY=
STRIPE_KEY=
```

Reading env vars in code:

```
let dbUrl = env.get("DATABASE_URL")              // -> maybe string — might not be set
let port = env.get("PORT").or("3000")            // use "3000" if PORT isn't set
let secret = env.get("SECRET_KEY")              // -> maybe string

if (dbUrl.exists()) {
  let db = database.connect(dbUrl.value)
}
```

**Why env vars?** Secrets and connection strings are different on every machine and environment. They should never be in your code or your compiled binary. The `.env` file lives on the machine and never gets committed. The binary reads it at runtime — different machines, different values, same binary.

**Missing `.env` is fine.** If the file doesn't exist, `env.get()` returns `none` for every key. The program decides how to handle that.

**No environment-specific `.env` files.** No `.env.production`, `.env.staging`, `.env.local`. Just `.env`. Different machines have different `.env` files. If you need different behavior in different environments, check an env var:

```
let environment = env.get("ENV").or("development")

let logLevel = "debug"
if (environment == "production") {
  logLevel = "error"
}
setLogLevel(logLevel)
```

---

## Layer 3: Runtime config — `set` functions in `main()`

Runtime behavior is configured in code, in your `main()` function, using `set` functions. Type `set` in autocomplete to see all available options.

```
function entrypoint() -> nothing {
  setShutdownTimeout(duration.minutes(5))
  setShutdownHandler(signal => { ... })
  setErrorHandler(e => { ... })
  setThreadPoolSize(16)
  setLogLevel("debug")

  // ... rest of app
}
```

**Why code and not a config file?** Runtime settings often depend on the environment, command-line arguments, or other conditions. Code handles this naturally:

```
function entrypoint() -> nothing {
  let env = env.get("ENV").or("development")

  if (env == "production") {
    setShutdownTimeout(duration.minutes(2))
    setLogLevel("error")
    setThreadPoolSize(32)
    return
  }
  setShutdownTimeout(duration.seconds(5))
  setLogLevel("debug")
  setThreadPoolSize(4)

  // ... rest of app
}
```

A static config file can't branch. Code can.

**Different apps, different configs:**

```
// Web server — graceful shutdown, more threads
function entrypoint() -> nothing {
  setShutdownTimeout(duration.seconds(30))
  setThreadPoolSize(16)
}

// Game — fast shutdown but save first
function entrypoint() -> nothing {
  setShutdownTimeout(duration.seconds(5))
  setShutdownHandler(signal => {
    wait saveGameState()
    wait savePlayerProgress()
  })
}

// Long-running simulation — give it time to checkpoint
function entrypoint() -> nothing {
  setShutdownTimeout(duration.minutes(10))
  setShutdownHandler(signal => {
    log("Checkpointing simulation...")
    wait checkpoint(simulationState)
  })
}

// Quick CLI script — no graceful shutdown needed
function entrypoint() -> nothing {
  setShutdownTimeout(duration.seconds(0))
}
```

---

## Summary — what goes where

| What | Where | Example |
|------|-------|---------|
| Project name, version | `yinz.toml` | `name = "my-app"` |
| Dependencies | `yinz.toml` | `graphics = "1.2.0"` |
| Build target, optimization | `yinz.toml` | `optimization = "release"` |
| Secrets, API keys, DB URLs | `.env` (git-ignored) | `DATABASE_URL=postgres://...` |
| Required var documentation | `.env.example` (committed) | `DATABASE_URL=` |
| Shutdown timeout | `setShutdownTimeout()` in `main()` | `setShutdownTimeout(duration.minutes(2))` |
| Thread pool size | `setThreadPoolSize()` in `main()` | `setThreadPoolSize(16)` |
| Error handling behavior | `setErrorHandler()` in `main()` | `setErrorHandler(e => { ... })` |
| Shutdown behavior | `setShutdownHandler()` in `main()` | `setShutdownHandler(s => { ... })` |
| Environment-specific logic | Code branching in `main()` | `if (env == "production") { ... }` |
