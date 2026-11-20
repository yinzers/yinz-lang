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

Add `errors` after the return type when something inside the function can fail:

```
function loadScore() -> Int errors {
  return 42
}

function entrypoint() -> Nothing errors {
  let score = loadScore()
  print(score)
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
