# Numeric Types — Design Decisions

User spec: `spec/numeric-types.md`

---

## Three Types: `number`, `float`, `int`

Three distinct numeric types:
- `number` — decimal arithmetic (base-10). Exact. `0.1 + 0.2 = 0.3`. Default for all math.
- `float` — binary floating point. Fast, may have tiny rounding. Opt-in for graphics, physics, ML.
- `int` — whole numbers only. For counts, indices, anything with no decimal.

**Why `number` as the default**: Eliminates the single most common JavaScript gotcha — floating point imprecision in financial and general math. `0.1 + 0.2 = 0.30000000000000004` is a JavaScript meme for a reason. The safe, correct path is the default path (Golden Rule 10).

**Why `float` is explicit opt-in**: Performance-sensitive code (graphics, physics, ML) needs binary floats. These developers know they need `float` and why. Junior developers who don't know the difference default to `number`, which is correct for 95% of use cases.

**Why `int`**: Whole-number operations (array indices, loop counters, counts) are faster with integer arithmetic than decimal. The IDE suggests `int` when it sees `number` used for clearly whole-number values.

**IDE teaching**: Warns when `float` is used for financial-looking values (likely a mistake). Suggests `int` when `number` is used for obviously whole-number math (performance improvement).

---

## Implementation: Handwritten, Not Crates

All three numeric types are **handwritten** in the compiler — no external crates for arithmetic. Reasoning:

- Yinz's flagship promise is "exact decimal math by default." Staking that promise on a third-party crate we haven't audited is duct-tape per `~/.claude/rules/no-duct-tape.md` — deferred risk dressed up as convenience.
- Handwriting doesn't mean inventing math. We implement to the **IEEE 754-2008 decimal128** standard (for `number`) and **IEEE 754 binary64** standard (for `float`). The math is specified by international standards — our job is the implementation, not the algorithm.
- The arithmetic lives in our source tree, fully readable and auditable.

**Validation strategy:**
- **IEEE 754-2008 conformance test vectors** — the standard ships a published test suite of millions of `(input, op, expected)` triples. Pass the suite, we are correct by definition.
- **Differential testing** — in CI, generate 10k random `(a, b, op)` tuples and assert that our implementation produces bit-identical results to a reference (Python `decimal` module for `number`, Rust `f64` for `float`).
- **Property tests** — commutativity, associativity (where applicable), distributivity, round-trip identity.

If we ever consider switching to a crate, the test suite IS the contract — drop in any implementation, run the tests, see if it passes.

---

## `number<N>` — Parameterized Precision

`number` defaults to 34 significant decimal digits (= `number<34>` = IEEE 754 decimal128). For higher-precision work (physics, scientific computing, number theory), users opt in to wider precision per-variable:

```yinz
let price: number = 19.99                    // 34 digits (default, fastest)
let position: number<70> = 0.000000001       // 70 digits
let chaotic: number<200> = startingCondition // 200 digits
```

**Maximum precision: 4096 digits.** Higher values are a compile error:

```
let huge: number<5000> = 0.001
// COMPILE ERROR: number<N> precision is capped at 4096 in v0.1.
//                See design/mvp-scope.md#v2--deferred-features for the
//                arbitrary-precision deferral.
```

**Why 4096:**
- Covers gravitational wave numerics (~200 digits), QCD calculations (~50 digits), and far exceeds any realistic physics workload.
- Memory per value bounded at ~1.7 KB — predictable, fits in `fixed<T>` without surprise.
- Multiplication cost bounded — at N=4096 with Karatsuba, still microseconds. No surprise milliseconds-per-op.
- Yinz's character is "predictable performance" — unbounded precision means unbounded per-op cost. Cap exists to preserve that character.

**Implementation:**
- N ≤ 34: backing storage is `u128` coefficient. Hardware-fast path.
- N > 34: backing storage is a fixed-size array of `u128` chunks. Bignum integer math on the coefficient. Slower per-op but bounded.
- All semantics identical (IEEE 754 decimal arithmetic), only the digit cap changes.

**Mixed-precision arithmetic:** binary operators promote to the higher precision. Assignment to a narrower precision rounds half-even with a compiler warning:

```yinz
let a: number<34> = 1.0
let b: number<100> = 2.0
let c = a + b           // c is number<100> — promoted
let d: number<34> = c   // COMPILER WARNING: assigning number<100> to number<34>
                        //   will round to 34 digits.
```

---

## `float` — IEEE 754 Binary64 (f64)

Standard double precision. Same as JS `number`, Rust `f64`, C `double`. Hardware-native on every modern CPU. Handwritten implementation validated against `f64` test vectors and differential tests against Rust's `f64`.

**Why binary float still has rounding errors** (and why we keep them): `float` is the IEEE 754 binary representation by design. The errors are a property of the format, not bugs. Users opt in to `float` precisely when they want hardware-speed and accept the rounding (graphics, physics, ML). The errors must reproduce IEEE behavior exactly — if Rust's `f64` says `0.1 + 0.2 = 0.30000000000000004`, ours must say the same.

**Sized variants (`f32`):** Deferred to v2+ — see `design/mvp-scope.md#v2--deferred-features`.

---

## `int` — Signed 64-bit (i64)

Range: ±9.2 × 10^18. Covers any count a human writes by hand. Signed because unsigned types are footguns (`array.count() - 1` on empty arrays — classic underflow bug).

**Overflow behavior: panic by default, in both debug and release.**

```yinz
let count: int = int.max
count = count + 1
// RUNTIME ERROR: integer overflow at line 3.
//   count was at max value and would have wrapped.
//   Use .wrappingAdd() for explicit wrap-around, .saturatingAdd() to cap.
```

**Explicit escape valves as dot methods** when wrap or saturate is the intended behavior:
- `.wrappingAdd(n)` — wraps to min/max on overflow
- `.wrappingMul(n)` — same
- `.saturatingAdd(n)` — caps at int.max / int.min
- `.saturatingMul(n)` — same
- (Same suffix pattern for any operation that can overflow.)

**Why panic over wrap:** Wrapping is almost always a bug, not a feature. Making the bug loud at the call site is better than silent corruption downstream. Users who genuinely want wrap (rare — usually cryptography, hashing) get to ask for it explicitly with `.wrappingAdd()`. That visibility is good — code review can spot it.

**Sized variants (`int<N>`, `uint<N>`):** Deferred to v2+ — see `design/mvp-scope.md#v2--deferred-features`.
