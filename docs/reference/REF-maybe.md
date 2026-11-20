---
name: "REF-maybe"
description: "Some values might not exist. Maybe lets you say that in the type system, so the compiler can make sure you handle it."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
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
// COMPILE ERROR: String cannot be none.
// Use Maybe<String> if the value is optional.

let name: Maybe<String> = none    // fine
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
// COMPILE ERROR: nickname is Maybe<String> — value might be none.
// Use nickname.or(default) or check nickname.exists() first.
```

---

## In function signatures

```
function findPlayer(share roster: Fixed<Player>, name: String) -> Maybe<Player> {
  return roster.find(player => player.name == name)    // .find() returns Maybe<T>
}
```

The caller must handle the `Maybe`:

```
let player = findPlayer(roster, `Alice`)

// Option 1 — use a fallback
let foundPlayer = player.or(defaultPlayer)
let name = foundPlayer.name

// Option 2 — check first
if (player.exists()) {
  print(player.value.name)
}
```

---

## `Maybe<T>` and `T | none`

`Maybe<String>` is the same type as `String | none`. They're interchangeable:

```
let a: Maybe<String> = none
let b: String | none = none
// a and b are the same type — either works
```

`Maybe` exists because it's shorter and reads more naturally. See [Unions](REF-unions.md).

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
function firstOr(nums: share Array<Int>) -> Int {
  let r = nums.first()               // Maybe<Int>
  if (!r.exists()) { return -1 }     // leaves the function — below this line r must have a value
  return r.value                     // safe: you only get here if r.exists() was true
}
```

The early exit must use `return`, `panic`, or an endless `loop`. Those are the only ways the compiler can be sure the function never continues. A normal call to a function that returns `Nothing` doesn't count.
