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

## Why binary floats produce errors (and number doesn't)

You may have seen this in JavaScript or Python:

```
0.1 + 0.2 === 0.30000000000000004    // JavaScript
```

That isn't a bug. It happens because most modern languages use **binary floating point** to store decimal numbers, and most decimals can't be represented exactly in binary.

It's the same reason `1/3` in regular decimal is `0.3333333...` forever — base 10 can't divide cleanly. In base 2 (binary), `0.1` is:

```
0.0001100110011001100110011001100110011...   (the 0011 repeats forever)
```

Computers have to stop somewhere, so they store the closest possible value. For `0.1` in 64-bit binary float, that's:

```
0.1000000000000000055511151231257827021181583404541015625
                  ↑ what's actually in memory, not 0.1
```

Add two slightly-wrong values together and the small errors stack up: `0.30000000000000004`.

**`number` sidesteps this entirely** by storing values in base 10. The number `0.1` is stored as the trio `(positive, coefficient=1, exponent=-1)` — exactly. Arithmetic happens on whole-number coefficients, so there are no fractional bits to lose.

You only lose precision if you exceed the configured digit limit (34 digits by default — far more than typical math). When you do, rounding is deterministic (round half-even, per IEEE 754).

---

## number[N] — higher precision when you need it

For scientific computing, physics simulations, or any work that needs more than 34 digits of precision, use `number[N]`:

```
let position: number[70] = 0.0000000000000001
let chaotic: number[200] = initialCondition
```

`N` is the number of significant decimal digits.

| Range | Backing storage | Speed |
|-------|-----------------|-------|
| `number` (= `number[34]`) | u128 coefficient (hardware) | Fastest |
| `number[N]` for N ≤ 34 | u128 coefficient | Fastest |
| `number[N]` for N > 34 | Bignum coefficient | Slower per op, still bounded |

**Maximum precision: `number[4096]`.** Larger values are a compile error:

```
let huge: number[5000] = 0.001
// COMPILE ERROR: number[N] precision is capped at 4096 in v0.1.
//                If you genuinely need unbounded precision, see design/deferrals.md.
//                File an issue with your workload — the cap is intentional and
//                we want to know about real cases that exceed it.
```

**Mixing precisions in arithmetic** promotes to the higher precision automatically:

```
let a: number[34] = 1.0
let b: number[100] = 2.0
let c = a + b              // c is number[100] — promoted, no precision lost
```

**Assigning to a narrower precision** rounds with a compiler warning:

```
let high: number[100] = 0.123456789012345678901234567890
let low: number[34] = high
// COMPILER WARNING: assigning number[100] to number[34] will round to 34 digits.
//   If this is intentional, the warning can be silenced. Otherwise widen the target.
```

---

## float — fast binary arithmetic (opt-in)

`float` uses binary floating point — the same as most other languages. Faster than `number`, but produces the same binary-rounding errors described above:

```
let velocity: float = 9.81 * 2.5    // fast — tiny rounding is acceptable here
let angle: float = math.sin(1.5)    // graphics/physics — speed over precision
```

Use `float` when:
- You're doing graphics, physics, or simulations
- You're working with machine learning or tensors (once those land — see design/deferrals.md)
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

Range: `int.min` to `int.max` — roughly ±9.2 × 10^18. Plenty for any count you'd hand-write.

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

## int overflow

`int` panics on overflow by default — both in debug and release builds:

```
let count: int = int.max
count = count + 1
// RUNTIME ERROR: integer overflow at line 2.
//   count was at the max value and would have wrapped.
//   Use .wrappingAdd() if you want wrap-around behavior on purpose,
//   or .saturatingAdd() to cap at int.max.
```

When wrap-around or saturation is the behavior you actually want, ask for it explicitly:

```
let x: int = int.max
let wrapped = x.wrappingAdd(1)      // wraps to int.min
let capped = x.saturatingAdd(1)     // stays at int.max
```

The same suffix pattern applies to subtract / multiply / etc:

| Method | Behavior |
|--------|----------|
| `.wrappingAdd(n)`, `.wrappingSub(n)`, `.wrappingMul(n)` | Wraps on overflow |
| `.saturatingAdd(n)`, `.saturatingSub(n)`, `.saturatingMul(n)` | Caps at int.max / int.min |

**Why panic by default:** wrap-around is almost always a bug, not a feature. Making it loud at the call site is better than silent corruption downstream. If you want wrap (rare — usually cryptography or hashing), you ask for it visibly.

---

## When to use which

| What you're working with | Type |
|--------------------------|------|
| Money, prices, financial math | `number` |
| Any general math where you want exact results | `number` |
| Physics requiring >34 digits | `number[70]` (or higher up to 4096) |
| Not sure | `number` |
| Loop counters, array indices, counts of things | `int` |
| Graphics positions, colors, transforms | `float` |
| Physics simulations (where speed matters more than 34th-digit accuracy) | `float` |
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
