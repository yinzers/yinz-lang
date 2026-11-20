---
name: "REF-testing"
description: "Yinz has no test runner. Check a function by calling it from entrypoint() and printing the result."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Testing

Yinz has no test runner: there is no `ynz test` command, and `test` is a reserved word you cannot use as a name. Check a function by calling it from `entrypoint()` and printing the result.

---

## Checking a function

```
function addBonus(score: Int, bonus: Int) -> Int {
  return score + bonus
}

function entrypoint() -> Nothing {
  let total = addBonus(90, 5)
  print(total)
}
```

Run it with `ynz run`:

```
ynz run entrypoint.ynz
```

The program prints `95`. If the number is not the one you expect, the function has a bug. See [Tooling](REF-tooling.md) for every `ynz` command.

---

## Rerunning when you save

`ynz watch` rebuilds and reruns your program every time you save a file:

```
ynz watch entrypoint.ynz
```
