---
name: "REF-maybe"
description: "Some values might not exist. maybe lets you say that in the type system, so the compiler can make sure you handle it."
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

Some values might not exist. `maybe` lets you say that in the type system, so the compiler can make sure you handle it.

---

## The problem

A value can be missing: a player with no nickname, a file with no contents, a lookup that finds nothing. In Yinz you write that in the type, so the compiler catches a missing value before your program ever runs.

---

## maybe<T> — a value that might not exist

```
let name: string = `Patrick`             // always has a value, guaranteed
let nickname: maybe<string> = none       // might not have a value
```

`maybe<string>` means: this is either a `string` or it's `none`. The compiler makes you handle the `none` case before you use the value.

---

## none — the absent value

`none` is the built-in value for "nothing here." It only works with `maybe` types:

```
let name: string = none
// COMPILE ERROR: string cannot be none.
// Use maybe<string> if the value is optional.

let name: maybe<string> = none    // fine
```

---

## Accessing a maybe value

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
// COMPILE ERROR: nickname is maybe<string> — value might be none.
// Use nickname.or(default) or check nickname.exists() first.
```

---

## In function signatures

```
function findPlayer(share roster: fixed<Player>, name: string) -> maybe<Player> {
  return roster.find(player => player.name == name)    // .find() returns maybe<T>
}
```

The caller must handle the `maybe`:

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

## `maybe<T>` and `T | none`

`maybe<string>` is the same type as `string | none`. They're interchangeable:

```
let a: maybe<string> = none
let b: string | none = none
// a and b are the same type — either works
```

`maybe` exists because it's shorter and reads more naturally. See [Unions](REF-unions.md).

---

## Functions you can call on a maybe

```
value.exists()      // does it have a value? → boolean
value.or(default)   // get the value, or the fallback if none → T
value.value         // get the value (compile error without an exists() check)
```

---

## Checking first, then returning early

After an early return, the compiler also knows `.value` is safe. You don't need to put the rest of your code inside the `if` block:

```
function firstOr(nums: share array<int>) -> int {
  let r = nums.first()               // maybe<int>
  if (!r.exists()) { return -1 }     // leaves the function — below this line r must have a value
  return r.value                     // safe: you only get here if r.exists() was true
}
```

The early exit must use `return`, `panic`, or an endless `loop`. Those are the only ways the compiler can be sure the function never continues. A normal call to a function that returns `nothing` doesn't count.
