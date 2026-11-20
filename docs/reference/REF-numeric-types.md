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

## number — the default (exact decimal arithmetic)

`number` is the default. It does its math in base 10, the way you do on paper, so you get no surprises from floating point (the way computers usually store decimals):

```
let price = 0.1 + 0.2              // 0.3 — exact, not 0.30000000000000004
let tax = 19.99 * 0.07             // 1.3993 — exact
let change = 100.00 - 99.99        // 0.01 — exact
```

Use `number` for anything where the exact value matters: money, measurements, user-facing numbers, or any case where you're not sure which type to pick.

---

## Why `float` rounds and `number` doesn't

Adding `0.1 + 0.2` as a `float` gives `0.30000000000000004`, not `0.3`.

That isn't a bug. `float` stores decimal numbers in **binary** (base 2, the 0s and 1s a computer uses), and most decimals can't be written exactly in binary.

It's the same reason `1/3` is `0.3333333...` forever in everyday base 10: it never ends. In binary, `0.1` is:

```
0.0001100110011001100110011001100110011...   (the 0011 repeats forever)
```

A computer has to stop somewhere, so it stores the closest value it can. For `0.1` as a 64-bit `float`, that is:

```
0.1000000000000000055511151231257827021181583404541015625
                  ↑ what's actually in memory, not 0.1
```

Add two slightly-wrong values and the small errors add up: `0.30000000000000004`.

**`number` avoids this** by storing values in base 10. It stores `0.1` as three exact pieces: positive, digits `1`, and a decimal point moved one place left. The math runs on whole-number digits, so nothing is lost to rounding.

You only lose precision if a result needs more digits than the limit (34 digits by default, far more than everyday math needs). When that happens, the result is rounded the same way every time: to the nearest digit, and to the even digit when exactly halfway (the IEEE 754 standard's rule).

---

## number<N> — higher precision when you need it

For science, physics simulations, or any work that needs more than 34 digits of precision, use `number<N>`:

```
let position: number<70> = 0.0000000000000001
let chaotic: number<200> = initialCondition
```

`N` is the number of significant digits (the digits that count, ignoring leading zeros).

| Range | Speed |
|-------|-------|
| `number` (the same as `number<34>`) | Fastest |
| `number<N>` for N ≤ 34 | Fastest |
| `number<N>` for N > 34 | Slower per operation, but still bounded |

**Maximum precision: `number<4096>`.** Larger values are a compile error:

```
let huge: number<5000> = 0.001
// COMPILE ERROR: Expected a precision value between 1 and 4096 inside `number<N>`.
//                Write `number<34>` for the default 34-digit decimal.
```

**Mixing precisions in math** gives the higher precision automatically:

```
let shortValue: number<34> = 1.0
let longValue: number<100> = 2.0
let total = shortValue + longValue    // total is number<100> — no precision lost
```

**Assigning to a lower precision** rounds the value and gives a compiler warning:

```
let high: number<100> = 0.123456789012345678901234567890
let low: number<34> = high
// COMPILER WARNING: assigning number<100> to number<34> will round to 34 digits.
//   If this is intentional, the warning can be silenced. Otherwise widen the target.
```

---

## float — fast binary arithmetic (opt-in)

`float` stores numbers in binary. It is faster than `number`, but it has the binary rounding errors described above:

```
let velocity: float = 9.81 * 2.5    // fast — tiny rounding is acceptable here
let angle: float = math.sin(1.5)    // graphics/physics — speed over precision
```

Use `float` when:
- You're doing graphics, physics, or simulations
- You've measured that `number` is too slow and a tiny rounding error is acceptable

The IDE warns you if `float` looks wrong:

```
let price: float = 19.99
// IDE HINT: Using float for financial values may cause rounding errors.
// Consider using number for exact arithmetic.
```

---

## int — whole numbers only

`int` has no decimal point. Use it for counts, indices, loop counters — anything that must be a whole number:

```
let count: int = 42
let index: int = 0
let score: int = 100
```

Range: `int.min` to `int.max` — roughly ±9.2 × 10^18 (about nine quintillion). That is plenty for any count you'd write by hand.

Assigning a decimal to an `int` is a compile error:

```
let count: int = 3.5
// COMPILE ERROR: 3.5 is not a whole number. int only holds whole numbers.
// Use number or float for decimal values.
```

The IDE suggests `int` when you use `number` for something that is clearly a whole number:

```
let pixels: number = 1920
// IDE HINT: number (decimal) is slower for pure integer math.
// Consider int for whole numbers.
```

---

## int overflow

*Overflow* means a result is too big for an `int` to hold. By default the program stops with a panic (a crash that stays inside one task), in debug and release builds alike:

```
let count: int = int.max
count = count + 1
// RUNTIME ERROR: integer overflow at line 2.
//   count was at the max value and would have wrapped.
//   Use .wrappingAdd() if you want wrap-around behavior on purpose,
//   or .saturatingAdd() to cap at int.max.
```

When you want the number to wrap around (jump to the other end of the range) or stop at the limit, ask for it by name:

```
let x: int = int.max
let wrapped = x.wrappingAdd(1)      // wraps to int.min
let capped = x.saturatingAdd(1)     // stays at int.max
```

Subtract and multiply work the same way:

| Function | Behavior |
|----------|----------|
| `.wrappingAdd(n)`, `.wrappingSub(n)`, `.wrappingMul(n)` | Wraps around on overflow |
| `.saturatingAdd(n)`, `.saturatingSub(n)`, `.saturatingMul(n)` | Stops at int.max / int.min |

Wrapping is rare (it is mostly for hashing and encryption), so you ask for it by name where you need it.

---

## When to use which

| What you're working with | Type |
|--------------------------|------|
| Money, prices, financial math | `number` |
| Any general math where you want exact results | `number` |
| Physics requiring >34 digits | `number<70>` (or higher up to 4096) |
| Not sure | `number` |
| Loop counters, array indices, counts of things | `int` |
| Graphics positions, colors, transforms | `float` |
| Physics simulations (where speed matters more than 34th-digit accuracy) | `float` |
| Machine learning | `float` |

When in doubt, use `number`. The IDE suggests the others when it would help.

---

## Types the compiler picks for you

If you don't write a type, the compiler picks a numeric type from the value:

```
let playerCount = 42       // the compiler picks int
let taxRate = 0.07         // the compiler picks number
let gravity: float = 9.81  // you asked for float
```

Mixing numeric types in one expression is a compile error — you must convert one side yourself:

```
let a: int = 42
let b: number = 1.5
let c = a + b        // COMPILE ERROR: `+` cannot be used with `int` and `number`.
                     // Convert the `int` to `number`: `a.toNumber() + b`
                     // Both sides of an expression must have the same type.

let c = a.toNumber() + b   // OK: number + number = number
```

Because every conversion is written out, you never lose precision or change how a number behaves without seeing it.
