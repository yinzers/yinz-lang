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
  let raw = readFile(`config.txt`)    // if this fails, the error cascades to the caller automatically
  let config = parseConfig(raw)       // same — auto-propagates on failure
  return config                       // only gets here on success
}
```

You write the happy path. The compiler handles the failure path.

---

## Logging or recovering before propagating

Sometimes you want to log a warning, fall back to a default, or retry before letting the error cascade. Just check the result first:

```
function loadConfig() -> Config errors {
  let raw = readFile(`config.txt`)

  if (raw.failed()) {
    log.warn(`config missing, using default: ${raw.message}`)
    return Config.default()           // recover with a fallback
  }

  return parseConfig(raw)             // raw is treated as a plain string from here on
}
```

The rule: **if you call `.failed()` or `.message` on the result before using the success value, auto-propagation is suppressed for that variable.** You've taken responsibility for handling it. If you DON'T check, the compiler auto-propagates the first time you use the value.

Same code pattern, two behaviors picked by the compiler from what you wrote. You never have to think "which mode am I in?"

---

## Compile error if you check after using

```
function loadConfig() -> Config errors {
  let raw = readFile(`config.txt`)
  let parsed = parseConfig(raw)              // ← raw used here as a string
  if (raw.failed()) { return Config.default() }
}
// COMPILE ERROR: raw can no longer be checked for failure here.
//
//                Move the failure check above the line that uses raw as a string:
//                  let raw = readFile(`config.txt`)
//                  if (raw.failed()) { return Config.default() }
//                  let parsed = parseConfig(raw)
//
//                Why: When you passed raw to parseConfig() on line 3, the
//                compiler treated it as a plain string from that point on
//                (since you didn't check for failure first). Once raw is
//                treated as a plain string, the .failed() method no longer
//                applies. Always check for failure BEFORE using the value
//                if you want a chance to recover.
```

---

## Handling errors in non-errors functions

If your function does NOT declare `errors`, you must explicitly handle every fallible call. The compiler enforces this:

```
function loadConfig() -> Config {
  let raw = readFile(`config.txt`)
  // COMPILE ERROR: readFile() can fail but loadConfig() is not marked errors.
  // Either handle the error or add `errors` to this function.
}
```

Three ways to handle it:

**Option 1 — Let the error cascade to the caller (mark your function as errors)**

```
function loadConfig() -> Config errors {
  let raw = readFile(`config.txt`)    // auto-propagates
  return parseConfig(raw)
}
```

**Option 2 — Use a default value**

```
function loadConfig() -> Config {
  let raw = readFile(`config.txt`).or(`{}`)    // use `{}` if the file can't be read
  return parseConfig(raw)
}
```

**Option 3 — Check and handle explicitly**

```
function loadConfig() -> Config {
  let raw = readFile(`config.txt`)
  if (raw.failed()) {
    log(raw.message)
    return defaultConfig        // return a fallback Config
  }
  return parseConfig(raw)       // raw is treated as a plain string after the failed() check
}
```

---

## Dot methods on error results

When you call an `errors` function, the result has these methods until you handle the error (or auto-propagation cascades it to the caller):

```ynz
content.failed()           // did it fail? → boolean
content.message            // error description (only valid after .failed() check)
content.suggestions        // array<string> of next steps (may be empty)
content.trace              // array<Frame> — the call path
content.source             // SourceLoc — where the failure originated
content.or(`fallback`)     // use this default if failed → T
```

After `content.or(...)`, you get back a plain value — no more error handling needed.

After `if (content.failed()) { return ... }`, the variable is treated as its plain success type in the remaining code.

**`.message` and other error fields require a `.failed()` check first.** Accessing them without proof that the value failed is a compile error:

```ynz
let raw = readFile(`config.txt`)
print(raw.message)    // COMPILE ERROR: raw hasn't been checked for failure.
                      //   Check with raw.failed() first, or use raw.or(`default`).
```

Correct:

```ynz
if (raw.failed()) {
  print(raw.message)    // ✅ safe — compiler knows this is the failed branch
}
```

---

## The error's call trace

When an error propagates, it carries a full call trace. You can read it as structured data:

```ynz
let content = readFile(`config.txt`)

if (content.failed()) {
  print(`Error: ${content.message}`)

  for frame in content.trace {
    let lineInfo = frame.line.or(-1)
    print(`  ${frame.function}  ${frame.file}  line ${lineInfo}`)
  }
}
```

Each `Frame` value has:

```ynz
frame.file        // string — source file path
frame.line        // maybe int — line number (one-based), none for the truncation sentinel
frame.function    // string — function name
```

**`content.source`** is where the failure originated — a single location:

```ynz
content.source.file    // string
content.source.line    // maybe int
```

The trace captures the full call chain from the function where the error was created up to the current handler.

**Trace truncation:** if the call chain is more than 1024 frames deep, the trace is truncated and a sentinel frame is appended with `line: none` to mark the cutoff. Real programs rarely hit this limit.

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
ERROR: Could not read file `config.ynz`
  Reason: File not found

  main()              entrypoint.ynz    line 4
    → startServer()   server.ynz  line 8
      → loadConfig()  config.ynz  line 12
        → readFile()  ✖ failed here

  Suggestion: Check that `config.ynz` exists, or handle this error in loadConfig():

    let config = readFile(`config.ynz`)
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
