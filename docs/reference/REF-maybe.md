---
name: "REF-maybe"
description: "Some values might not exist. Maybe lets you say that in the type system, so the compiler can make sure you handle it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Maybe Types

Some values might not exist. `Maybe` lets you say that in the type system, so the compiler can make sure you handle it.

---

## The problem

A value can be missing: a player with no nickname, a file with no contents, a lookup that finds nothing. In Yinz you write that in the type, so the compiler catches a missing value before your program ever runs.

---

## Maybe<T> — a value that might not exist

```
let name: String = `Patrick`             // always has a value, guaranteed
let nickname: Maybe<String> = none       // might not have a value
```

`Maybe<String>` means: this is either a `String` or it's `none`. The compiler makes you handle the `none` case before you use the value.

---

## none — the absent value

`none` is the built-in value for "nothing here." It only works with `Maybe` types:

```
let name: String = none
// COMPILE ERROR: `none` cannot be a `String` value.
// Use `Maybe<T>` for optional values: `let x: Maybe<Int> = none`.
// `none` is only valid as the absent value of `Maybe<T>`. It cannot be used where a concrete type is expected.

let nickname: Maybe<String> = none    // fine
```

---

## Accessing a `Maybe` value

Three ways:

**Check if it exists first:**

```
if (nickname.exists()) {
  print(nickname.value)    // safe: the compiler knows there is a value inside this block
}
```

**Use a default if it's none:**

```
let display = nickname.or(`Anonymous`)    // get the value or a fallback
```

**`.value` without checking — compile error:**

```
print(nickname.value)
// COMPILE ERROR: `Maybe.value` requires you to first check `m.exists()`.
// Add a check: `if (m.exists()) { print(m.value) }`. Or use a default: `m.or(0)`.
// The compiler cannot prove this `Maybe` has a value here. `.value` without a prior `.exists()` check is a compile error.
```

---

## In function signatures

```
function firstScore(share scores: Array<Int>) -> Maybe<Int> {
  return scores.first()    // .first() returns Maybe<T>
}
```

The caller must handle the `Maybe`:

```
let scores: Array<Int> = [3, 5, 7]
let best = firstScore(scores)

// Option 1 — use a fallback
let bestOrZero = best.or(0)

// Option 2 — check first
if (best.exists()) {
  print(best.value)
}
```

---

## Functions you can call on a `Maybe`

```
value.exists()      // does it have a value? → Boolean
value.or(default)   // get the value, or the fallback if none → T
value.value         // get the value (compile error without an exists() check)
```

---

## Checking first, then returning early

After an early return, the compiler also knows `.value` is safe. You don't need to put the rest of your code inside the `if` block:

```
function firstOr(share nums: Array<Int>) -> Int {
  let r = nums.first()               // Maybe<Int>
  if (!r.exists()) { return -1 }     // leaves the function — below this line r must have a value
  return r.value                     // safe: you only get here if r.exists() was true
}
```

The early exit must be a `return`. A call to a function that returns `Nothing` doesn't count, because the function carries on after it.
