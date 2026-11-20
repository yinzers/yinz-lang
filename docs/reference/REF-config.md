---
name: "REF-config"
description: "Configuration in Yinz lives in three places, each with a specific job."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

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
let dbUrl = env.get(`DATABASE_URL`)              // -> maybe<string> — might not be set
let foundPort = env.get(`PORT`)                  // a maybe: the text, or nothing
let port = foundPort.or(`3000`)                  // a plain string, `3000` if PORT isn't set
let secret = env.get(`SECRET_KEY`)               // -> maybe<string>

if (dbUrl.exists()) {
  let db = database.connect(dbUrl.value)
}
```

Keep secrets and connection strings in `.env`, never in your code or your compiled program. The `.env` file stays on the machine and is never committed. The program reads it when it starts, so different machines can have different values and run the same compiled program.

**A missing `.env` is fine.** If the file doesn't exist, `env.get()` returns `none` for every key. Your program decides what to do about that.

**No environment-specific `.env` files.** No `.env.production`, `.env.staging`, `.env.local`. Just `.env`. Each machine has its own `.env` file. If you need different behavior in different places, check an environment variable:

```
let foundEnvironment = env.get(`ENV`)            // a maybe: the name, or nothing
let environment = foundEnvironment.or(`development`)

let logLevel = `debug`
if (environment == `production`) {
  logLevel = `error`
}
setLogLevel(logLevel)
```

---

## Layer 3: Runtime config — `set` functions in `entrypoint()`

Runtime behavior is configured in code, in your `entrypoint()` function, using `set` functions. Type `set` in autocomplete to see all available options.

```
function entrypoint() -> nothing {
  setShutdownTimeout(duration.minutes(5))
  setShutdownHandler(signal => { ... })
  setErrorHandler(error => { ... })
  setThreadPoolSize(16)
  setLogLevel(`debug`)

  // ... rest of app
}
```

What the settings do:

- `setShutdownTimeout` — how long the program may take to finish up after it is asked to stop.
- `setShutdownHandler` — code that runs when the program is asked to stop, such as saving data.
- `setErrorHandler` — code that runs when an error reaches the top of your program and nothing else has handled it.
- `setThreadPoolSize` — how many threads (separate lines of work) the program may use at the same time.
- `setLogLevel` — how much the program logs.

Runtime settings are code, so they can branch on environment variables, command-line arguments, or any other condition:

```
function entrypoint() -> nothing {
  let foundEnvironment = env.get(`ENV`)
  let environment = foundEnvironment.or(`development`)

  if (environment == `production`) {
    setShutdownTimeout(duration.minutes(2))
    setLogLevel(`error`)
    setThreadPoolSize(32)
    return
  }
  setShutdownTimeout(duration.seconds(5))
  setLogLevel(`debug`)
  setThreadPoolSize(4)

  // ... rest of app
}
```

A static config file can't branch. Code can.

**Different apps, different configs:**

```
// Web server — time to finish open requests before shutting down, more threads
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

// Long-running simulation — give it time to save its progress
function entrypoint() -> nothing {
  setShutdownTimeout(duration.minutes(10))
  setShutdownHandler(signal => {
    log(`Saving simulation progress...`)
    wait checkpoint(simulationState)
  })
}

// Quick command-line script — nothing to clean up on shutdown
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
| Shutdown timeout | `setShutdownTimeout()` in `entrypoint()` | `setShutdownTimeout(duration.minutes(2))` |
| Thread pool size | `setThreadPoolSize()` in `entrypoint()` | `setThreadPoolSize(16)` |
| Error handling behavior | `setErrorHandler()` in `entrypoint()` | `setErrorHandler(error => { ... })` |
| Shutdown behavior | `setShutdownHandler()` in `entrypoint()` | `setShutdownHandler(signal => { ... })` |
| Environment-specific logic | An `if` in `entrypoint()` | ``if (environment == `production`) { ... }`` |
