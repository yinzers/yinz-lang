# Error Handling

Some functions can fail. Yinz makes that explicit in the type, and the compiler enforces that you handle it.

---

## Marking functions that can fail

Add `errors` after the return type:

```
function readFile(path: string) -> string errors {
  // returns a string on success
  // triggers an error on failure
}

function deleteFile(path: string) -> nothing errors {
  // returns nothing on success
  // triggers an error on failure
}

// No errors keyword = this function can NEVER fail
function double(x: number) -> number {
  return x * 2
}
```

---

## Auto-propagation in errors functions

If your function is also marked `errors`, any call to an `errors` function auto-propagates on failure. No boilerplate:

```
function loadConfig() -> Config errors {
  let raw = readFile("config.txt")    // if this fails, the error bubbles up automatically
  let config = parseConfig(raw)       // same — auto-propagates on failure
  return config                       // only gets here on success
}
```

`raw` and `config` are automatically the success types (`string` and `Config`). You just write the happy path.

---

## Handling errors in non-errors functions

If your function does NOT declare `errors`, you must explicitly handle every fallible call. The compiler enforces this:

```
function loadConfig() -> Config {
  let raw = readFile("config.txt")
  // COMPILE ERROR: readFile() can fail but loadConfig() is not marked errors.
  // Either handle the error or add "errors" to this function.
}
```

Three ways to handle it:

**Option 1 — Propagate it (mark your function as errors)**

```
function loadConfig() -> Config errors {
  let raw = readFile("config.txt")    // auto-propagates
  return parseConfig(raw)
}
```

**Option 2 — Use a default value**

```
function loadConfig() -> Config {
  let raw = readFile("config.txt").or("{}")    // use "{}" if the file can't be read
  return parseConfig(raw)
}
```

**Option 3 — Check and handle explicitly**

```
function loadConfig() -> Config {
  let raw = readFile("config.txt")
  if (raw.failed()) {
    log(raw.message)
    return defaultConfig        // return a fallback Config
  }
  return parseConfig(raw)       // raw is narrowed to string after the failed() check
}
```

---

## Dot methods on error results

When you call an `errors` function in a non-errors context, the result has these methods until you handle it:

```
content.failed()           // did it fail? → bool
content.message            // error description (only valid when failed)
content.or("fallback")     // use this default if failed → T
```

After `content.or(...)`, you get back a plain value — no more error handling needed.

After `if (content.failed()) { return ... }`, the variable is narrowed to the success type in the remaining code.

---

## `maybe` vs `errors` — two different things

```
maybe User         // this user might not exist — absence, not failure
-> User errors     // this function might fail — failure with a message and call trace
```

A function can have both:

```
function findUser(id: string) -> maybe User errors {
  let data = readDatabase(id)                              // might fail (connection error)
  let user = data.filter(u => u.id == id).first()          // might be none (user not found)
  return user
}
```

---

## What happens when an error reaches the top

If an error propagates all the way up without being handled, the program stops and prints a clean, readable error:

```
ERROR: Could not read file "config.ynz"
  Reason: File not found

  main()              main.ynz    line 4
    → startServer()   server.ynz  line 8
      → loadConfig()  config.ynz  line 12
        → readFile()  ✖ failed here

  Suggestion: Check that "config.ynz" exists, or handle this error in loadConfig():

    let config = readFile("config.ynz")
    if (config.failed()) {
      // handle missing config
    }
```

---

## Custom error handler

Override the default behavior with a custom handler:

```
setErrorHandler(e => {
  log(`[${timestamp()}] ${e.message}`)
  log(e.trace)
  sendToMonitoring(e)
})
```

`e.message` — human-readable error description
`e.trace` — call path as structured data
`e.source` — file and line where the error originated

---

## Open design question

The exact behavior of auto-propagation in `errors` functions — specifically whether you can manually check `.failed()` on a sub-call inside an `errors` function (to log before propagating) — is still being finalized. See `/design/open-questions.md` for details.
