---
name: "REF-ownership"
description: "Yinz manages memory automatically — without a garbage collector. No malloc. No free. No unpredictable pauses."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Ownership

Yinz manages memory automatically — without a garbage collector. No `malloc`. No `free`. No unpredictable pauses.

The rule: every value has exactly one owner. When the owner goes out of scope, the memory is freed automatically.

---

## The three modes

When a function takes a value, its signature says one of three things:

```ynz
function greet(share name: string) -> nothing { ... }      // share — read-only; caller keeps ownership
function rename(lend player: Player) -> nothing { ... }    // lend — function modifies it; caller keeps ownership
function consume(give data: Data) -> nothing { ... }       // give — function takes it; caller loses it
```

That's the contract. Anyone reading the signature knows what happens to the value.

---

## You don't type the mode at the call site

When you call a function, you just pass the value normally. The compiler reads the callee's signature and figures out what to do — share, lend, or give. The IDE shows what was inferred as muted text so you can see what's happening:

```ynz
greet(name)           // IDE shows muted `share` — read-only access
print(message)        // IDE shows muted `share` — read-only access
rename(player)        // IDE shows muted `lend` (red-tinted) — function modifies player
consume(data)         // IDE shows muted `give` (red-tinted) — function takes ownership
```

Hovering on any muted hint shows a tooltip explaining WHAT it means, WHAT INSTEAD you'd write to make it explicit (on the function signature, not the call site), and WHY the compiler chose it.

There is no body-level syntax for these three modes — they exist only in signatures. You never type `.share()` / `.lend()` / `.give()` in source.

---

## What happens after the value is given away

Once a function takes ownership via `give`, you can't use the value anymore. The compiler catches this:

```ynz
consume(data)              // give inferred from consume's signature — data transferred
print(data)
// COMPILE ERROR: data was transferred to consume() on the previous line.
// It no longer exists here. Use .copy() if you need to keep a copy.
```

---

## `const` bindings get extra protection

A `const` binding can only be shared (read). The compiler refuses to infer `lend` or `give` for a `const` value:

```ynz
const player: Player = { name: `Patrick`, health: 100 }
rename(player)
// COMPILE ERROR: player is `const`, but rename's signature requires `lend`.
// To allow modification, declare player with `let` instead.
```

---

## `.copy()` — when you need to keep a copy

```ynz
const original: Player = { name: `Patrick`, health: 100 }
const backup = original.copy()    // a new value of your own
saveForever(backup)                // backup is given to saveForever; original is unchanged
```

`.copy()` gives you a value nobody else can reach. Change the copy and the original does not move; change the original and the copy does not move.

For a list, that includes the items. Copying an `array<array<int>>` gives you a new outer list holding new inner lists, so writing into the copy's first row leaves the original's first row alone:

```ynz
let inner: array<int> = [1, 2, 3]
let outer: array<array<int>> = [inner]
let clone: array<array<int>> = outer.copy()
let cloneRow: array<int> = clone[0].or(inner)
cloneRow.set(0, 99)
// outer's first row still starts at 1
```

`.copy()` works on a shape, on an `array<T>`, on a `fixed<T>`, on a `map<K, V>`, on a `maybe<T>`, and on every simple value (numbers, strings, booleans, an `options` value, a `sensitive` value). For a shape that holds arrays or maps inside it, write a standalone `copy()` function that copies those pieces and call it as a normal function — that keeps an expensive copy visible in your code.

### When `.copy()` refuses

Some things cannot be copied, and Yinz says so while you build rather than handing you back the same thing and letting you find out later:

```ynz
let orders: channel<int> = channel<int>(4)
let secondLine = orders.copy()
// COMPILE ERROR: `.copy()` cannot make a separate `channel<int>` value — a channel is the
// line two tasks talk over, not a value you hold.
//   Pass the channel itself to the task and every task holding it reads and writes the same
//   line. If you want a second, separate line, make one:
//     let replies: channel<int> = channel<int>()
//   Why: a second channel holding the same messages would not help you — the task on the
//   other end is listening on the first one. Anything you sent into the copy would go
//   nowhere, and nothing would tell you.
```

The same happens for a task handle (it names one running task), for a value written with `|` (which of the two it is, is only settled while the program runs), for a `dynamic` value, for a loop's map entry, and for an `errors` value you have not checked with `.failed()` yet. Every one of those errors tells you what to do instead.

---

## `.freeze()` — lock a value from further changes

Sometimes you want to build a value step-by-step then prevent any more changes:

```ynz
let config: ConfigBuilder = { rules: [] }
config.addRule(`a`, 1)
config.addRule(`b`, 2)
config.freeze()                    // lock from this point forward
runApp(config)                      // config can still be read; further mutation is a compile error
```

After `.freeze()`, the binding behaves like `const` for the rest of its scope. This is useful for build-then-lock patterns where you need mutability during construction but want to prevent accidental modification afterward.

---

## Summary

| What | Where it lives | When you type it |
|---|---|---|
| `share` / `lend` / `give` | Function signatures only | Always at signatures; never at call sites (compiler infers there) |
| `.copy()` | Body expression | When you need a value of your own that nobody else can reach |
| `.freeze()` | Body expression | When you want to lock a binding from further mutation mid-function |

The compiler does the heavy lifting at call sites. The IDE shows what was inferred. You learn ownership by reading your own code with hints turned on.
