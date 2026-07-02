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
  redirect("/login")
}

// Combined — standard precedence, add parentheses when it helps
if (active && (isAdmin || isModerator) && !banned) {
  grantFullAccess()
}
```

`&&` = AND. `||` = OR. `!` = NOT.

Note: `|` (single pipe) is for **union types**, not boolean OR: `shape Shape = Circle | Square`. `||` (double pipe) is boolean OR in expressions.

---

## Bitwise operators

For bit manipulation — permissions, flags, color channels, binary protocols:

```
let combined = FLAG_A | FLAG_B       // bitwise OR — combine bits
let masked = value & 0xFF            // bitwise AND — extract bits
let toggled = value ^ mask           // bitwise XOR — flip specific bits
let inverted = ~value                // bitwise NOT — flip all bits
let doubled = value << 1             // shift left — multiply by 2
let halved = value >> 1              // shift right — divide by 2
```

**Boolean vs bitwise — clean split:**

| Purpose | Operator | Example |
|---------|----------|---------|
| Boolean AND | `&&` | `if (active && verified)` |
| Boolean OR | `\|\|` | `if (admin \|\| mod)` |
| Boolean NOT | `!` | `if (!banned)` |
| Bitwise AND | `&` | `value & 0xFF` |
| Bitwise OR | `\|` | `FLAG_A \| FLAG_B` |
| Bitwise XOR | `^` | `value ^ mask` |
| Bitwise NOT | `~` | `~value` |
| Shift left | `<<` | `value << 4` |
| Shift right | `>>` | `value >> 1` |

**Permissions example:**
```
const READ: int = 1
const WRITE: int = 2
const EXECUTE: int = 4

let perms = READ | WRITE              // 3 — has read and write
if (perms & READ != 0) { ... }        // check a permission
perms = perms | EXECUTE               // add execute
perms = perms & ~WRITE                // remove write
```

---

## No compound assignment or increment

```
x += 5     // NOT valid — use x = x + 5
x -= 1     // NOT valid — use x = x - 1
x++        // NOT valid — use x = x + 1
x--        // NOT valid — use x = x - 1
```

One assignment operator: `=`. Makes mutations visible and explicit.

---

## Operator precedence — standard PEMDAS

```
1.  ()                   // parentheses
2.  !  ~                 // NOT operators
3.  *  /  %              // multiplication, division, remainder
4.  +  -                 // addition, subtraction
5.  <<  >>               // bit shifts
6.  <  >  <=  >=         // comparison
7.  ==  !=               // equality
8.  &                    // bitwise AND
9.  ^                    // bitwise XOR
10. |                    // bitwise OR
11. &&                   // boolean AND
12. ||                   // boolean OR
```

Same as C/Java/JavaScript. When in doubt, add parentheses.

The IDE hints when precedence might be misread:

```
let check = a || b && c
// IDE HINT: && evaluates before ||.
// Result is a || (b && c). Add parentheses to clarify intent.
```

---

## Operator overloading

Types can implement operators through `follows` contracts from the standard library. See [Operator Overloading](operators.md#overloading) for details.

---

## Overloading

> **Status**: Operator overloading is **deferred to v1.0**. In v0.1, users
> who want custom math types should write `.add()`, `.subtract()` methods
> explicitly. The design below is locked but not yet implemented.

The standard library defines contracts for operators. Each contract declares the bare signature of the function the implementing shape must provide. Implementations live as standalone functions at the file/module level (Yinz is not object-oriented — see [`.claude/rules/non-oop.md`](../../.claude/rules/non-oop.md)).

```ynz
// Contracts — bare-signature form (no `function` keyword, no body)
shape Addable {
  add(share self, share other: Self) -> Self
}

shape Equatable {
  equals(share self, share other: Self) -> boolean
}

shape Comparable follows Equatable {
  compareTo(share self, share other: Self) -> int
}

shape Printable {
  toString(share self) -> string
}
```

`Self` is a reserved keyword meaning "the type that follows this contract."

**Example:**

```ynz
// Shape declares data + follows clauses; no method bodies here
shape Vector2D follows Addable, Equatable, Printable {
  x: number
  y: number
}

// Standalone functions provide the implementations
// Compiler verifies these match each contract's signature when checking `follows`
function add(share self: Vector2D, share other: Vector2D) -> Vector2D {
  return { x: self.x + other.x, y: self.y + other.y }
}

function equals(share self: Vector2D, share other: Vector2D) -> boolean {
  return self.x == other.x && self.y == other.y
}

function toString(share self: Vector2D) -> string {
  return `(${self.x}, ${self.y})`
}

const a: Vector2D = { x: 1, y: 2 }
const b: Vector2D = { x: 3, y: 4 }

const c = a + b          // calls add(a, b) — operator overload resolves to the standalone function
const same = a == b      // calls equals(a, b)
print(a)                 // calls toString(a)
```

**Operator to function mapping:**

| Operator | Contract | Standalone function name |
|----------|----------|--------|
| `+` | `Addable` | `add` |
| `-` | `Subtractable` | `subtract` |
| `*` | `Multipliable` | `multiply` |
| `/` | `Divisible` | `divide` |
| `==` | `Equatable` | `equals` |
| `<` `>` `<=` `>=` | `Comparable` | `compareTo` |
| `print(x)` | `Printable` | `toString` |

The compiler looks up the function by name + first-parameter type (standard overload resolution) when the operator is used. `a + b` desugars to `add(a, b)`.

---

## `print()` always works

All types are printable — no `Printable` contract, no `.toString()` implementation required.

**Shapes** print as `ShapeName { field: value, field: value }`:

```ynz
print(player)    // Player { name: Alice, health: 100 }
```

**Arrays** print as `[elem1, elem2, ...]`. Large arrays are capped at 20 elements:

```ynz
print(scores)               // [82, 91, 74, 88]
print(bigArray)             // [1, 2, 3, ..., 20, ... and 980 more]
```

**Options** print their display string if one was declared, otherwise the variant name:

```ynz
print(Timeframe.daily)      // Daily    (if declared as daily: `Daily`)
print(Status.active)        // active   (bare variant, no display string)
```

To customize how a shape prints, write a standalone `toString(share self: YourShape) -> string` function and use string interpolation:

```ynz
function toString(share self: Player) -> string {
    return `${self.name} (HP: ${self.health})`
}
print(player)    // Alice (HP: 100)
```

---

## No `===` triple equals

`==` is always type-safe. Comparing incompatible types is a compile error:

```
let a: string = "5"
let b: number = 5
if (a == b) { }    // COMPILE ERROR: cannot compare string and number
```
