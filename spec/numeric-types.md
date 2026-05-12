# Numeric Types

Yinz has three numeric types. Pick the one that matches what your values represent.

---

## number — the default (exact decimal arithmetic)

`number` is the default. It uses decimal arithmetic — no floating point surprises:

```
let price = 0.1 + 0.2              // 0.3 — exact, not 0.30000000000000004
let tax = 19.99 * 0.07             // 1.3993 — exact
let change = 100.00 - 99.99        // 0.01 — exact
```

Use `number` for anything where the exact value matters: money, measurements, user-facing numbers, or any case where you're not sure which type to pick.

---

## float — fast binary arithmetic (opt-in)

`float` uses binary floating point — the same as most other languages. Faster than `number`, but may produce tiny rounding errors:

```
let velocity: float = 9.81 * 2.5    // fast — tiny rounding is acceptable here
let angle: float = math.sin(1.5)    // graphics/physics — speed over precision
```

Use `float` when:
- You're doing graphics, physics, or simulations
- You're working with machine learning or tensors
- You've measured that `number` is a bottleneck and a tiny rounding error is acceptable

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

Assigning a decimal to an `int` is a compile error:

```
let count: int = 3.5
// COMPILE ERROR: 3.5 is not a whole number. int only holds whole numbers.
// Use number or float for decimal values.
```

The IDE suggests `int` when you're using `number` for something that's clearly a whole number:

```
let pixels: number = 1920
// IDE HINT: number (decimal) is slower for pure integer math.
// Consider int for whole numbers.
```

---

## When to use which

| What you're working with | Type |
|--------------------------|------|
| Money, prices, financial math | `number` |
| Any general math where you want exact results | `number` |
| Not sure | `number` |
| Loop counters, array indices, counts of things | `int` |
| Graphics positions, colors, transforms | `float` |
| Physics simulations | `float` |
| Machine learning, tensors | `float` |

When in doubt, use `number`. The IDE suggests the others when it would help.

---

## Type inference

The compiler infers numeric types from values and usage:

```
let x = 42          // inferred as int
let y = 3.14        // inferred as number
let z: float = 1.0  // explicit float
```

Mixed-type expressions promote to the most capable type in the expression.

---

## Note on existing examples

Many examples throughout this spec use `number` for fields like `health` and `score` — those would more accurately be typed as `int`. They'll be updated in a future spec pass.
