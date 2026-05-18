# Main Function

Every Yinz program starts at `function entrypoint()`. The compiler looks for it in the entry file set in `yinz.toml`.

---

## Basic

```
function entrypoint() -> nothing {
  print("Hello from the Burgh")
}
```

---

## With error handling

```
function entrypoint() -> nothing errors {
  let config = file.read("config.ynz")
  let server = http.serve(3000)
  server.start()
}
```

Errors that aren't handled cascade all the way up to the default error handler — it prints a clean call trace and exits with code 1.

---

## Entry file

```toml
# yinz.toml
[project]
entry = "entrypoint.ynz"    # default — change to any .ynz file
```

The function is always called `entrypoint`. The file name is flexible — `app.ynz`, `server.ynz`, whatever makes sense for your project.

---

## Command-line arguments

Arguments come from the standard library, not from `entrypoint`'s parameters:

```
function entrypoint() -> nothing {
  let args = cli.args()                    // all arguments → array<string>
  let verbose = cli.flag("verbose")        // --verbose flag → bool
  let port = cli.option("port", "3000")   // --port option → string with default
}
```

See [Tooling](tooling.md) for the full CLI spec.

---

## Exit codes

```
function entrypoint() -> nothing {
  if (setupFailed) {
    process.exit(1)    // exit with failure code
  }
}
// Normal completion exits with code 0 automatically
```

Note: `process` module design is in progress.
