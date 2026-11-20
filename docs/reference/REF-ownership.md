---
name: "REF-ownership"
description: "Yinz manages memory automatically, without a garbage collector. You never reserve or free memory by hand, and there are no unpredictable pauses."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-14"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Ownership

Yinz manages memory automatically, without a garbage collector (a background process that hunts for unused memory and can pause your program). You never reserve or free memory by hand, and there are no unpredictable pauses.

The rule: every value has exactly one owner. When the owner goes out of scope, the memory is freed automatically.

---

## The three modes

When a function takes a value, its signature says one of three things:

```ynz
function greet(share name: String) -> Nothing { ... }      // share — read-only; caller keeps ownership
function rename(lend player: Player) -> Nothing { ... }    // lend — function modifies it; caller keeps ownership
function consume(give data: Data) -> Nothing { ... }       // give — function takes it; caller loses it
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

## Giving away inside a loop

A loop runs its body again and again, so a value given away on one pass is gone on the next. The compiler refuses it:

```ynz
let rows: Array<Int> = [1, 2, 3]
for (turn in [1, 2]) {
  consume(rows)
  // COMPILE ERROR: `rows` was already given away and cannot be used here, because the previous
  // pass through this loop already gave it to `consume`.
}
```

Give a copy each pass (`consume(rows.copy())`), or make a new value inside the loop. Reassigning `rows` before the end of the body also works, as long as every path through the body does it.

---

## Giving away, then reassigning

A reassignment gives the name a new value, so the name is usable again after it, but only for the code the reassignment is sure to have run before:

```ynz
consume(rows)
rows = [4, 5]
print(rows.count())      // fine: every path got here through the reassignment

consume(rows)
if (ready) {
  rows = [4, 5]
}
print(rows.count())
// COMPILE ERROR: `rows` was already given away and cannot be used here, because the path where
// the `if` above did not run never reassigned it.
```

After an `if`, a `match` or a loop, `rows` is usable again only if every arm assigned it (a match needs an `else` arm, or to cover every variant). A loop body may run zero times, so a reassignment inside it does not count after the loop.

Putting a list, a map or a shape into a place hands it over, just like giving it to a function. That covers `bucket.add(rows)`, `bucket[0] = rows`, `player.items = rows`, a list or shape built from named values (`[rows, more]`) and `let moved = rows`. The place owns the value from then on, and reading `rows` afterwards is an error:

```ynz
let rows: Array<Int> = [1, 2, 3]
let bucket: Array<Array<Int>> = []
bucket.add(rows)
print(rows.count())
// COMPILE ERROR: `rows` was stored in `bucket` and cannot be used here — storing a value hands it over.
//   If you still need `rows` after the store, store a copy instead: `bucket.add(rows.copy())`.
//   If you only need it before the store, put this line above the store.
```

Numbers, booleans, options, strings and ranges are copied, so storing them gives nothing away.

A `for` loop can hand its rows over. When you never read the list again, not inside the loop and not after it, storing the loop variable takes the row out of the list instead of copying it:

```ynz
let matrix: Array<Array<Int>> = [[1, 2], [3, 4], [5, 6]]
let kept: Array<Array<Int>> = []
for (row in matrix) {
  kept.add(row)
}
```

If you read the list inside the loop or after it, each row still belongs to the list, so the compiler asks for a copy:

```ynz
let matrix: Array<Array<Int>> = [[1, 2], [3, 4], [5, 6]]
let kept: Array<Array<Int>> = []
for (row in matrix) {
  kept.add(row)
  // COMPILE ERROR: This line gives `row` away — it is handed to `kept` — but `row` is one cell of
  // `matrix`, which this loop is walking, so someone here still holds it.
  //   Give away a copy instead: `row.copy()`.
}
print(matrix.count())
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

For a list, that includes the items. Copying an `Array<Array<Int>>` gives you a new outer list holding new inner lists, so writing into the copy's first row leaves the original's first row alone:

```ynz
let inner: Array<Int> = [1, 2, 3]
let outer: Array<Array<Int>> = [inner]
let clone: Array<Array<Int>> = outer.copy()
let foundRow = clone[0]                        // a maybe: the row, or nothing
let cloneRow: Array<Int> = foundRow.or(inner)  // a plain list, `inner` if missing
cloneRow.set(0, 99)
// outer's first row still starts at 1
```

`.copy()` works on every simple value (numbers, strings, booleans, an `options` value, a `sensitive` value — a copied `sensitive` value stays hidden when it is printed, same as the original), on a shape, on an `Array<T>`, and on a `Map<K, V>`.

For the three that hold other values inside them, it depends on what is inside:

- **`Array<T>`** copies its items too, however deep they go — unless an item is something that cannot be copied at all (a `Channel`, say), and then the whole copy is refused.
- **`Fixed<T>`** holds its items in one block with no separate piece to follow, so it copies lists of simple values and shapes, and refuses a `Fixed` of lists or maps.
- **`Maybe<T>`** copies when what is inside is a simple value, a number, or a shape. `Maybe<Array<Int>>`, `Maybe<Map<String, Int>>`, `Maybe<Fixed<Int>>` and a `Maybe` of a `Maybe` are refused today — those are the natural things to wrap, so you will meet this. Copy the piece instead:

```ynz
let rows: Maybe<Array<Int>> = loadRows()
// let backup = rows.copy()          // COMPILE ERROR — see below
let theRows: Array<Int> = rows.or([])
let backup: Array<Int> = theRows.copy()   // copy the list itself
```

For a shape that holds arrays or maps inside it, `.copy()` copies the shape's own fields and the inner list stays shared. Write a standalone `copy()` function that copies those pieces and call it as a normal function — that keeps an expensive copy visible in your code.

### When `.copy()` refuses

Some things cannot be copied, and Yinz says so while you build rather than handing you back the same thing and letting you find out later:

```ynz
let orders: Channel<Int> = Channel<Int>(4)
let secondLine = orders.copy()
// COMPILE ERROR: `.copy()` cannot make a separate `Channel<Int>` value — a channel is the
// line two tasks talk over, not a value you hold.
//   Pass the channel itself to the task and every task holding it reads and writes the same
//   line. If you want a second, separate line, make one:
//     let replies: Channel<Int> = Channel<Int>()
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
