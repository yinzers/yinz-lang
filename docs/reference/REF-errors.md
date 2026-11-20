---
name: "REF-errors"
description: "Some functions can fail. Yinz makes that explicit in the type, and the compiler enforces that you handle it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Error Handling

Some functions can fail. Yinz makes that explicit in the type, and the compiler enforces that you handle it.

---

## Marking functions that can fail

Add `errors` after the return type:

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function logScore(score: Int) -> Nothing errors {
  print(score)
}

// No errors keyword = this function can NEVER fail
function double(score: Int) -> Int {
  return score * 2
}

function entrypoint() -> Nothing {
  let average = safeDivide(10, 2)
  let result = average.or(0)
  print(double(result))
}
```

Output:

```text
10
```

The functions in the rest of this page use `safeDivide`. Dividing by zero is not an `errors` failure: it stops the program with a runtime error. A failure you can see as a value comes from a built-in that reports one, such as `send` on a channel that is already closed (shown in "What you can do with an `errors` result" below).

---

## Errors flow up automatically

If your function is also marked `errors`, a failed call to another `errors` function passes its error up to your caller. This is called *auto-propagation*: the error goes up on its own. You write no extra code for it:

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function averageScore(total: Int, games: Int) -> Int errors {
  let average = safeDivide(total, games)    // if this fails, the error goes up to the caller automatically
  return average                            // only gets here on success
}

function entrypoint() -> Nothing {
  let result = averageScore(10, 2)
  print(result.or(0))
}
```

Output:

```text
5
```

You write the happy path. The compiler handles the failure path.

---

## Recovering instead of passing the error up

Sometimes you want to print a message or fall back to a default instead of passing the error up to your caller. Check the result first:

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function averageOrZero(total: Int, games: Int) -> Int {
  let average = safeDivide(total, games)

  if (average.failed()) {
    print(average.message)
    return 0                            // recover with a fallback
  }

  return average                        // average is treated as a plain Int from here on
}

function entrypoint() -> Nothing {
  let result = averageOrZero(10, 2)
  print(result)
}
```

Output:

```text
5
```

The rule: **if you call `.failed()` on the result before using the success value, the error no longer goes up automatically for that variable.** You've taken responsibility for handling it. If you DON'T check, the error goes up the first time you use the value.

The same code gives two behaviors, and the compiler picks one from what you wrote. You never have to ask "which mode am I in?"

Check for failure BEFORE you use the value. After `if (average.failed()) { ... }`, the variable is treated as its plain success type in the remaining code.

---

## Handling errors in non-errors functions

If your function does NOT declare `errors`, you must handle every call that can fail. The compiler enforces this:

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function report(total: Int, games: Int) -> Nothing {
  let average = safeDivide(total, games)
  print(average)
}

function entrypoint() -> Nothing {
  report(10, 2)
}
```

The compiler stops with:

```text
Error: This function can fail, but the failure is not handled here.
   ╭─[ main.ynz:7:9 ]
   │
 7 │   print(average)
   │         ───┬───
   │            ╰───── Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.
   │
   │ Note: When a function can fail, the failure must be handled somewhere. The compiler enforces this so failures can't silently pass through.
───╯
```

Three ways to handle it:

**Choice 1 — Let the error go up to the caller (mark your function as errors)**

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function report(total: Int, games: Int) -> Nothing errors {
  let average = safeDivide(total, games)    // a failure goes up automatically
  print(average)
}

function entrypoint() -> Nothing {
  let outcome = report(10, 2)
  if (outcome.failed()) {
    print(`report failed`)
  }
}
```

Output:

```text
5
```

**Choice 2 — Use a default value**

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function report(total: Int, games: Int) -> Nothing {
  let attempt = safeDivide(total, games)    // might fail
  let average = attempt.or(0)               // use 0 if the division failed
  print(average)
}

function entrypoint() -> Nothing {
  report(10, 2)
}
```

Output:

```text
5
```

**Choice 3 — Check and handle explicitly**

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function report(total: Int, games: Int) -> Nothing {
  let average = safeDivide(total, games)
  if (average.failed()) {
    print(`no games played`)
    return
  }
  print(average)                            // average is treated as a plain Int after the failed() check
}

function entrypoint() -> Nothing {
  report(10, 2)
}
```

Output:

```text
5
```

---

## What you can do with an `errors` result

When you call an `errors` function, the result has these members until you handle the error (or it goes up to your caller):

- `.failed()` — did it fail? Gives a `Boolean`.
- `.message` — the error description, as a `String`. Only after a `.failed()` check.
- `.or(default)` — use this default if it failed. Gives the plain success type.

After `average.or(...)`, you get back a plain value — no more error handling needed.

`.message` and `.failed()` in action. Sending to a channel that was already closed is refused, and `send` is declared `-> Nothing errors`, so you can check it:

```ynz
function entrypoint() -> Nothing {
  let board: Channel<Int> = Channel<Int>(4)
  board.close()
  let late = board.send(9)
  if (late.failed()) {
    print(late.message)    // safe — the compiler knows this is the failed branch
  }
}
```

Output:

```text
The channel is closed — close() was called, so this value cannot be delivered. Check .failed() on the send, or send everything before close().
```

**`.message` requires a `.failed()` check first.** Reading it without that check is a compile error:

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function entrypoint() -> Nothing {
  let average = safeDivide(10, 2)
  print(average.message)
}
```

The compiler stops with:

```text
Error: `average.message` is read here, but `average` hasn't been checked with `.failed()` yet.
   ╭─[ main.ynz:7:17 ]
   │
 7 │   print(average.message)
   │                 ───┬───
   │                    ╰───── needs `.failed()` first
   │
   │ Note: Check `.failed()` on `average` first — store it in a `let`/`const` binding if it isn't one already, then write `if (average.failed()) { … average.message … }`. Or call `average.or(<default>)` for a fallback value instead of `.message`. — `.message`, `.suggestions`, `.trace`, and `.source` only carry meaningful data once `.failed()` is true — reading one before the check would report details for a call that might have succeeded. Yinz stops this at compile time instead of returning garbage at runtime.
───╯
```

`.suggestions`, `.trace` and `.source` are not available yet. Reading one inside a checked branch is a compile error:

```ynz
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b
}

function entrypoint() -> Nothing {
  let average = safeDivide(10, 2)
  if (average.failed()) {
    print(average.trace)
  }
}
```

The compiler stops with:

```text
Error: `.trace` isn't available yet on an `errors` value.
   ╭─[ main.ynz:8:19 ]
   │
 8 │     print(average.trace)
   │                   ──┬──
   │                     ╰──── not available yet
   │
   │ Note: Use `.message` to read what went wrong, after checking `.failed()`. — Only `.message` is wired up to a real value right now — `.trace` doesn't have one yet.
───╯
```

---

## `Maybe` vs `errors` — two different things

`Maybe<String>` means the name might not exist: absence, not failure. `-> Nothing errors` means the function might fail, with a message:

```ynz
function findPlayerName(share names: Array<String>, index: Int) -> Maybe<String> {
  let found = names.get(index)
  return found
}

function sendScore(lend board: Channel<Int>, score: Int) -> Nothing errors {
  board.send(score)
}

function entrypoint() -> Nothing {
  let names: Array<String> = [`Ana`, `Ben`]
  let missing = findPlayerName(names, 5)
  print(missing.exists())

  let board: Channel<Int> = Channel<Int>(4)
  let delivered = sendScore(board, 12)
  print(delivered.failed())
}
```

Output:

```text
false
false
```

A function can have both: `-> Maybe<Player> errors` means "might fail AND might find no player."
