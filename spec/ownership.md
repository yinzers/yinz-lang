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
greet(name)           // IDE shows muted "share" — read-only access
print(message)        // IDE shows muted "share" — read-only access
rename(player)        // IDE shows muted "lend" (red-tinted) — function modifies player
consume(data)         // IDE shows muted "give" (red-tinted) — function takes ownership
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
const player: Player = { name: "Patrick", health: 100 }
rename(player)
// COMPILE ERROR: player is `const`, but rename's signature requires `lend`.
// To allow modification, declare player with `let` instead.
```

---

## `.copy()` — when you need to keep a copy

```ynz
const original: Player = { name: "Patrick", health: 100 }
const backup = original.copy()    // produces a new owned value (cheap, trivially-copyable types only)
saveForever(backup)                // backup is given to saveForever; original is unchanged
```

`.copy()` is only allowed when every field of the value's type is trivially copyable (primitives all the way down). For shapes containing arrays, maps, or other heap-owned data, you write a standalone `copy()` function that does the deep copy explicitly and call it as a normal function. This prevents silent expensive copies.

---

## `.freeze()` — lock a value from further changes

Sometimes you want to build a value step-by-step then prevent any more changes:

```ynz
let config: ConfigBuilder = { rules: [] }
config.addRule("a", 1)
config.addRule("b", 2)
config.freeze()                    // lock from this point forward
runApp(config)                      // config can still be read; further mutation is a compile error
```

After `.freeze()`, the binding behaves like `const` for the rest of its scope. This is useful for build-then-lock patterns where you need mutability during construction but want to prevent accidental modification afterward.

---

## Summary

| What | Where it lives | When you type it |
|---|---|---|
| `share` / `lend` / `give` | Function signatures only | Always at signatures; never at call sites (compiler infers there) |
| `.copy()` | Body expression | When you want a cheap independent copy of a trivially-copyable value |
| `.freeze()` | Body expression | When you want to lock a binding from further mutation mid-function |

The compiler does the heavy lifting at call sites. The IDE shows what was inferred. You learn ownership by reading your own code with hints turned on.
