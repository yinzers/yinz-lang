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

```
function safeDivide(a: Int, b: Int) -> Int errors {
  return a / b          // fails when b is 0
}

function logScore(score: Int) -> Nothing errors {
  print(score)
}

// No errors keyword = this function can NEVER fail
function double(score: Int) -> Int {
  return score * 2
}
```

The functions in the rest of this page use `safeDivide`.

---

## Errors flow up automatically

If your function is also marked `errors`, a failed call to another `errors` function passes its error up to your caller. This is called *auto-propagation*: the error goes up on its own. You write no extra code for it:

```
function averageScore(total: Int, games: Int) -> Int errors {
  let average = safeDivide(total, games)    // if this fails, the error goes up to the caller automatically
  return average                            // only gets here on success
}
```

You write the happy path. The compiler handles the failure path.

---

## Recovering before the error goes up

Sometimes you want to print a message or fall back to a default before the error goes up to your caller. Check the result first:

```
function averageOrZero(total: Int, games: Int) -> Int errors {
  let average = safeDivide(total, games)

  if (average.failed()) {
    print(average.message)
    return 0                            // recover with a fallback
  }

  return average                        // average is treated as a plain Int from here on
}
```

The rule: **if you call `.failed()` on the result before using the success value, the error no longer goes up automatically for that variable.** You've taken responsibility for handling it. If you DON'T check, the error goes up the first time you use the value.

The same code gives two behaviors, and the compiler picks one from what you wrote. You never have to ask "which mode am I in?"

Check for failure BEFORE you use the value. After `if (average.failed()) { ... }`, the variable is treated as its plain success type in the remaining code.

---

## Handling errors in non-errors functions

If your function does NOT declare `errors`, you must handle every call that can fail. The compiler enforces this:

```
function report(total: Int, games: Int) -> Nothing {
  let average = safeDivide(total, games)
  print(average)
}
// COMPILE ERROR: This function can fail, but the failure is not handled here.
// Three options: (1) Mark this function `-> T errors` to pass failures up. (2) Use `.or(default)` for a fallback. (3) Check `.failed()` explicitly.
// When a function can fail, the failure must be handled somewhere. The compiler enforces this so failures can't silently pass through.
```

Three ways to handle it:

**Option 1 — Let the error go up to the caller (mark your function as errors)**

```
function report(total: Int, games: Int) -> Nothing errors {
  let average = safeDivide(total, games)    // a failure goes up automatically
  print(average)
}
```

**Option 2 — Use a default value**

```
function report(total: Int, games: Int) -> Nothing {
  let attempt = safeDivide(total, games)    // might fail
  let average = attempt.or(0)               // use 0 if the division failed
  print(average)
}
```

**Option 3 — Check and handle explicitly**

```
function report(total: Int, games: Int) -> Nothing {
  let average = safeDivide(total, games)
  if (average.failed()) {
    print(`no games played`)
    return
  }
  print(average)                            // average is treated as a plain Int after the failed() check
}
```

---

## What you can do with an `errors` result

When you call an `errors` function, the result has these members until you handle the error (or it goes up to your caller):

```
average.failed()           // did it fail? → Boolean
average.message            // error description → String (only after a .failed() check)
average.or(0)              // use this default if failed → T
```

After `average.or(...)`, you get back a plain value — no more error handling needed.

**`.message` requires a `.failed()` check first.** Reading it without that check is a compile error:

```
let average = safeDivide(10, 0)
print(average.message)
// COMPILE ERROR: `average.message` is read here, but `average` hasn't been checked with `.failed()` yet.
// Check `.failed()` on `average` first — store it in a `let`/`const` binding if it isn't one already, then write `if (average.failed()) { … average.message … }`. Or call `average.or(<default>)` for a fallback value instead of `.message`.
// `.message`, `.suggestions`, `.trace`, and `.source` only carry meaningful data once `.failed()` is true — reading one before the check would report details for a call that might have succeeded. Yinz stops this at compile time instead of returning garbage at runtime.
```

Correct:

```
if (average.failed()) {
  print(average.message)    // safe — the compiler knows this is the failed branch
}
```

`.suggestions`, `.trace` and `.source` are not available yet. Reading one inside a checked branch is a compile error:

```
// COMPILE ERROR: `.trace` isn't available yet on an `errors` value.
// Use `.message` to read what went wrong, after checking `.failed()`.
// Only `.message` is wired up to a real value right now — `.trace` doesn't have one yet.
```

---

## `Maybe` vs `errors` — two different things

```
Maybe<Player>      // this player might not exist — absence, not failure
-> Player errors   // this function might fail — failure with a message
```

A function can have both: `-> Maybe<Player> errors` means "might fail AND might find no player."
