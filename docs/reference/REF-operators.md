---
name: "REF-operators"
description: "User-facing reference listing every Yinz operator (arithmetic, comparison, boolean, bitwise, assignment) with usage and precedence details."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
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
let active = true
let verified = true
let isAdmin = false
let isModerator = true
let loggedIn = true
let banned = false

if (active && verified) {
  print(`Access granted`)
}

if (isAdmin || isModerator) {
  print(`Editing allowed`)
}

if (!loggedIn) {
  print(`Please log in`)
}

// Combined — add parentheses when it helps
if (active && (isAdmin || isModerator) && !banned) {
  print(`Full access`)
}
```

`&&` means and. `||` means or. `!` means not.

A single `|` is for **union types**, not for "or": `shape Shape = Circle | Square`. In conditions, use `||` for "or".

---

## Bitwise operators

Bitwise operators work on the individual bits (the 0s and 1s) of a whole number. They are for permissions, flags, color channels and binary data formats:

```
let readFlag = 1
let writeFlag = 2
let value = 200
let mask = 15

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
let isAdmin = false
let isMember = true
let hasTicket = true
let canEnter = isAdmin || isMember && hasTicket
// IDE HINT: && evaluates before ||.
// Result is isAdmin || (isMember && hasTicket). Add parentheses to clarify intent.
```

---

## `print()` always works

You can print any value. You don't need any extra code.

**Shapes** print as `ShapeName { field: value, field: value }`:

```ynz
print(player)    // Player { name: Alice, health: 100 }
```

**Arrays** print as `[elem1, elem2, ...]`. An array with more than 20 items prints the first 20, then how many more there are:

```ynz
print(scores)               // [82, 91, 74, 88]
print(bigArray)             // [0, 1, 2, ... 19, ... and 980 more]
```

**Options** print their display text if one was declared, otherwise the choice's name:

```ynz
print(Timeframe.daily)      // Daily    (if declared as daily: `Daily`)
print(Status.active)        // active   (no display text declared)
```

To print a shape your own way, write a function that returns a `String` and use `${...}` in a backtick string:

```ynz
function describe(share self: Player) -> String {
    return `${self.name} (HP: ${self.health})`
}
print(player.describe())    // Alice (HP: 100)
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
