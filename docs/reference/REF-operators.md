---
name: "REF-operators"
description: "User-facing reference listing every Yinz operator (arithmetic, comparison, boolean, bitwise, assignment) with usage and precedence details."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Operators

---

## All operators at a glance

```
// Arithmetic
+    -    *    /    %

// Comparison
==    !=    <    >    <=    >=

// Boolean
&&    ||    !

// Bitwise
&    |    ^    ~    <<    >>

// Assignment
=
```

---

## Boolean operators

```
if (active && verified) {
  grantAccess()
}

if (isAdmin || isModerator) {
  allowEdit()
}

if (!loggedIn) {
  redirect(`/login`)
}

// Combined — add parentheses when it helps
if (active && (isAdmin || isModerator) && !banned) {
  grantFullAccess()
}
```

`&&` means and. `||` means or. `!` means not.

A single `|` is for **union types**, not for "or": `shape Shape = Circle | Square`. In conditions, use `||` for "or".

---

## Bitwise operators

Bitwise operators work on the individual bits (the 0s and 1s) of a whole number. They are for permissions, flags, color channels and binary data formats:

```
let combined = readFlag | writeFlag  // bitwise or — turn on every bit set in either
let masked = value & 0xFF            // bitwise and — keep only the bits set in both
let toggled = value ^ mask           // bitwise xor — flip the bits set in mask
let inverted = ~value                // bitwise not — flip every bit
let doubled = value << 1             // shift left — move bits left, which doubles the number
let halved = value >> 1              // shift right — move bits right, which halves the number
```

**Boolean and bitwise operators are separate:**

| Purpose | Operator | Example |
|---------|----------|---------|
| Boolean and | `&&` | `if (active && verified)` |
| Boolean or | `\|\|` | `if (admin \|\| mod)` |
| Boolean not | `!` | `if (!banned)` |
| Bitwise and | `&` | `value & 0xFF` |
| Bitwise or | `\|` | `readFlag \| writeFlag` |
| Bitwise xor | `^` | `value ^ mask` |
| Bitwise not | `~` | `~value` |
| Shift left | `<<` | `value << 4` |
| Shift right | `>>` | `value >> 1` |

**Permissions example:**
```
const readPermission: Int = 1
const writePermission: Int = 2
const executePermission: Int = 4

let permissions = readPermission | writePermission     // 3 — has read and write
let canRead = (permissions & readPermission) != 0      // true — read is one of the permissions
permissions = permissions | executePermission          // add execute
permissions = permissions & ~writePermission           // remove write
print(permissions)                                     // 5
```

Put parentheses around `permissions & readPermission`. Without them, `!=` runs first (see the order below), and the compiler stops with an error.

---

## No compound assignment or increment

```
score += 5       // NOT valid — use score = score + 5
score -= 1       // NOT valid — use score = score - 1
score++          // NOT valid — use score = score + 1
score--          // NOT valid — use score = score - 1
```

Writing `score += 5` gives this error:

```
Error: `+=` is not supported in Yinz.
 9 │   score += 5
   │          ─┬
   │           ╰── Use `x = x + n` instead.
```

There is one assignment operator: `=`. Every change to a variable is written out.

---

## Operator precedence: which operator goes first

Precedence is the order in which operators run when a line has more than one. The ones higher in this list run first, as in school math:

```
1.  ()                   // parentheses
2.  !  ~                 // not
3.  *  /  %              // multiply, divide, remainder
4.  +  -                 // add, subtract
5.  <<  >>               // bit shifts
6.  <  >  <=  >=         // comparison
7.  ==  !=               // equality
8.  &                    // bitwise and
9.  ^                    // bitwise xor
10. |                    // bitwise or
11. &&                   // boolean and
12. ||                   // boolean or
```

When in doubt, add parentheses.

The IDE warns you when a line could be misread:

```
let canEnter = isAdmin || isMember && hasTicket
// IDE HINT: && evaluates before ||.
// Result is isAdmin || (isMember && hasTicket). Add parentheses to clarify intent.
```

---

## Using operators on your own shapes

> **Not available yet.** Using `+`, `==` and similar operators on your own shapes is planned but not built. Until then, write the functions yourself and call them by name, such as `add(a, b)`. The rest of this section describes the planned design.

The standard library will define a contract for each operator. A contract lists the signature of the function that a shape must provide. Those functions are written on their own at the top level of a file, not inside the shape.

```ynz
// Contracts — signatures only (no `function` keyword, no body)
shape Addable {
  add(share self, share other: Self) -> Self
}

shape Equatable {
  equals(share self, share other: Self) -> Boolean
}

shape Comparable follows Equatable {
  compareTo(share self, share other: Self) -> Int
}

shape Printable {
  toString(share self) -> String
}
```

`Self` is a reserved keyword that means "the shape that follows this contract."

**Example:**

```ynz
// The shape lists its fields and the contracts it follows; no function bodies here
shape Vector2D follows Addable, Equatable, Printable {
  x: Number
  y: Number
}

// Standalone functions supply the behavior
// The compiler checks that they match each contract's signature
function add(share self: Vector2D, share other: Vector2D) -> Vector2D {
  return { x: self.x + other.x, y: self.y + other.y }
}

function equals(share self: Vector2D, share other: Vector2D) -> Boolean {
  return self.x == other.x && self.y == other.y
}

function toString(share self: Vector2D) -> String {
  return `(${self.x}, ${self.y})`
}

const a: Vector2D = { x: 1, y: 2 }
const b: Vector2D = { x: 3, y: 4 }

const c = a + b          // calls add(a, b)
const same = a == b      // calls equals(a, b)
print(a)                 // calls toString(a)
```

**Which function each operator calls:**

| Operator | Contract | Function name |
|----------|----------|---------------|
| `+` | `Addable` | `add` |
| `-` | `Subtractable` | `subtract` |
| `*` | `Multipliable` | `multiply` |
| `/` | `Divisible` | `divide` |
| `==` | `Equatable` | `equals` |
| `<` `>` `<=` `>=` | `Comparable` | `compareTo` |
| `print(x)` | `Printable` | `toString` |

When you use an operator, the compiler finds the function by its name and the type of its first parameter. `a + b` means the same as `add(a, b)`.

---

## `print()` always works

You can print any value. You don't need a `Printable` contract or your own `toString` function.

**Shapes** print as `ShapeName { field: value, field: value }`:

```ynz
print(player)    // Player { name: Alice, health: 100 }
```

**Arrays** print as `[elem1, elem2, ...]`. Large arrays are capped at 20 elements:

```ynz
print(scores)               // [82, 91, 74, 88]
print(bigArray)             // [1, 2, 3, ..., 20, ... and 980 more]
```

**Options** print their display text if one was declared, otherwise the choice's name:

```ynz
print(Timeframe.daily)      // Daily    (if declared as daily: `Daily`)
print(Status.active)        // active   (no display text declared)
```

To change how a shape prints, write a standalone `toString(share self: YourShape) -> String` function and use `${...}` in a backtick string:

```ynz
function toString(share self: Player) -> String {
    return `${self.name} (HP: ${self.health})`
}
print(player)    // Alice (HP: 100)
```

---

## Comparing with `==`

`==` checks the types as well as the values. Comparing two different types is a compile error:

```
let input: String = `5`
let count: Int = 5
if (input == count) { print(1) }
```

The compiler stops with this error:

```
Error: `==` cannot be used with `String` and `Int`.
 4 │   if (input == count) { print(1) }
   │       ───────┬──────
   │              ╰──────── Check that both sides have the same type.
```
