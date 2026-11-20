---
name: "REF-main"
description: "Every Yinz program starts at function entrypoint(). The compiler looks for it in the entry file set in yinz.toml."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Main Function

Every Yinz program starts at `function entrypoint()`. The compiler looks for it in the entry file set in `yinz.toml`.

---

## Basic

```
function entrypoint() -> Nothing {
  print(`Hello from the Burgh`)
}
```

---

## With error handling

`entrypoint` always returns `Nothing`, and you cannot add `errors` to it. Handle every call that can fail inside it. Here `.or(0)` supplies a fallback value:

```
function loadScore() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let result = loadScore()
  let score = result.or(0)
  print(score)
}
```

Adding `errors` to `entrypoint` is a compile error:

```
function entrypoint() -> Nothing errors {
  print(`Hello`)
}
// COMPILE ERROR: `entrypoint` must return `Nothing`, but this declares it returns `Nothing errors`.
// Change the return type to `Nothing`: `function entrypoint() -> Nothing`
// `entrypoint` is where the program starts. It runs and exits — it does not return a value to a caller.
```

See [Errors](REF-errors.md) for the other ways to handle a failure.

---

## Entry file

```toml
# yinz.toml
[project]
entry = "entrypoint.ynz"    # default — change to any .ynz file
```

The function is always called `entrypoint`. The file name is flexible — `app.ynz`, `server.ynz`, whatever makes sense for your project.
