---
name: "REF-numeric-types"
description: "Yinz has three numeric types. Pick the one that matches what your values represent."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Numeric Types

Yinz has three numeric types. Pick the one that matches what your values represent.

---

## Number — the default (exact decimal arithmetic)

`Number` is the default. It does its math in base 10, the way you do on paper, so you get no surprises from floating point (the way computers usually store decimals):

```
let price = 0.1 + 0.2              // 0.3 — exact, not 0.30000000000000004
let tax = 19.99 * 0.07             // 1.3993 — exact
let change = 100.00 - 99.99        // 0.01 — exact
print(price)
print(tax)
print(change)
```

Use `Number` for anything where the exact value matters: money, measurements, user-facing numbers, or any case where you're not sure which type to pick.

---

## Why `Float` rounds and `Number` doesn't

Adding `0.1 + 0.2` as a `Float` gives `0.30000000000000004`, not `0.3`.

That isn't a bug. `Float` stores decimal numbers in **binary** (base 2, the 0s and 1s a computer uses), and most decimals can't be written exactly in binary.

It's the same reason `1/3` is `0.3333333...` forever in everyday base 10: it never ends. In binary, `0.1` is:

```text
0.0001100110011001100110011001100110011...   (the 0011 repeats forever)
```

A computer has to stop somewhere, so it stores the closest value it can. For `0.1` as a 64-bit `Float`, that is:

```text
0.1000000000000000055511151231257827021181583404541015625
                  ↑ what's actually in memory, not 0.1
```

Add two slightly-wrong values and the small errors add up: `0.30000000000000004`.

**`Number` avoids this** by storing values in base 10. It stores `0.1` as three exact pieces: positive, digits `1`, and a decimal point moved one place left. The math runs on whole-number digits, so nothing is lost to rounding.

You only lose precision if a result needs more digits than the limit (34 digits by default, far more than everyday math needs). When that happens, the result is rounded the same way every time: to the nearest digit, and to the even digit when exactly halfway (the IEEE 754 standard's rule).

---

## Number<N> — higher precision when you need it

For science, physics simulations, or any work that needs more than 34 digits of precision, use `Number<N>`:

```
let position: Number<70> = 0.0000000000000001
let precise: Number<200> = 1.5
print(position)
print(precise)
```

`N` is the number of significant digits (the digits that count, ignoring leading zeros).

`Number` is the same as `Number<34>`.

**Maximum precision: `Number<4096>`.** Larger values are a compile error:

```
let huge: Number<5000> = 0.001
// COMPILE ERROR: Expected a precision value between 1 and 4096 inside `Number<N>`.
// Write `Number<34>` for the default 34-digit decimal.
// The number in `Number<N>` sets how many decimal digits of precision you get.
```

**Mixing precisions in math** gives the higher precision automatically:

```
let shortValue: Number<34> = 1.0
let longValue: Number<100> = 2.0
let total = shortValue + longValue    // total is Number<100> — no precision lost
print(total)
```

---

## Float — fast binary arithmetic (opt-in)

`Float` stores numbers in binary. It is faster than `Number`, but it has the binary rounding errors described above:

```
let gravityPull: Float = 9.81
let seconds: Float = 2.5
let velocity: Float = gravityPull * seconds    // fast — tiny rounding is acceptable here
print(velocity)                                // 24.525000000000002

let width: Float = 1.5
let twice: Float = 2.0
let height: Float = width * twice              // graphics/physics — speed over precision
print(height)
```

Use `Float` when:
- You're doing graphics, physics, or simulations
- You've measured that `Number` is too slow and a tiny rounding error is acceptable

---

## Int — whole numbers only

`Int` has no decimal point. Use it for counts, indices, loop counters — anything that must be a whole number:

```
let count: Int = 42
let index: Int = 0
let score: Int = 100
print(count + index + score)
```

Range: `Int.min` to `Int.max` — roughly ±9.2 × 10^18 (about nine quintillion). That is plenty for any count you'd write by hand.

Assigning a decimal to an `Int` is a compile error:

```
let count: Int = 3.5
// COMPILE ERROR: This value is `Number`, but `count` is declared as `Int`.
// Change the annotation to `Number`, or use a different value.
// The value on the right side must match the type annotation on the left.
```
---

## Int overflow

*Overflow* means a result is too big for an `Int` to hold. By default the program stops with a panic (a crash that stays inside one task), in debug and release builds alike:

```
let count: Int = Int.max
count = count + 1
// RUNTIME ERROR: Int overflow in '+' at <file>:<line>:<column>
//
//   The value wrapped past the maximum (or minimum) for this type.
//
//   Use .wrappingAdd() if wrap-around is intentional.
//
//   Why: Yinz panics on integer overflow by default to prevent silent data corruption.
```

The `<file>:<line>:<column>` part stands for the file path and source position the message prints. The same message appears for `-` and `*`.

When you want the number to wrap around (jump to the other end of the range) or stop at the limit, ask for it by name:

```
let x: Int = Int.max
let wrapped = x.wrappingAdd(1)      // wraps to Int.min
let capped = x.saturatingAdd(1)     // stays at Int.max
print(wrapped)
print(capped)
```

Subtract and multiply work the same way:

| Function | Behavior |
|----------|----------|
| `.wrappingAdd(n)`, `.wrappingSub(n)`, `.wrappingMul(n)` | Wraps around on overflow |
| `.saturatingAdd(n)`, `.saturatingSub(n)`, `.saturatingMul(n)` | Stops at Int.max / Int.min |

Wrapping is rare (it is mostly for hashing and encryption), so you ask for it by name where you need it.

---

## When to use which

| What you're working with | Type |
|--------------------------|------|
| Money, prices, financial math | `Number` |
| Any general math where you want exact results | `Number` |
| Physics requiring >34 digits | `Number<70>` (or higher up to 4096) |
| Not sure | `Number` |
| Loop counters, array indices, counts of things | `Int` |
| Graphics positions, colors, transforms | `Float` |
| Physics simulations (where speed matters more than 34th-digit accuracy) | `Float` |
| Machine learning | `Float` |

When in doubt, use `Number`. The IDE suggests the others when it would help.

---

## Types the compiler picks for you

If you don't write a type, the compiler picks a numeric type from the value:

```
let playerCount = 42       // the compiler picks Int
let taxRate = 0.07         // the compiler picks Number
let gravity: Float = 9.81  // you asked for Float
print(playerCount)
print(taxRate)
print(gravity)
```

Mixing numeric types in one expression is a compile error — you must convert one side yourself:

```
let a: Int = 42
let b: Number = 1.5
let c = a + b
// COMPILE ERROR: `+` cannot be used with `Int` and `Number`.
// Convert the `Int` to `Number`: `myInt.toNumber() + myNumber`
// Yinz does not convert between types automatically. Both sides of an expression must have the same type.
```

Write the conversion out and the same expression builds:

```
let a: Int = 42
let b: Number = 1.5
let c = a.toNumber() + b   // Number + Number = Number
print(c)                   // 43.5
```

Because every conversion is written out, you never lose precision or change how a number behaves without seeing it.
