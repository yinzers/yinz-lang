---
name: "REF-type-conversion"
description: "Convert between types using dot methods. No cast keyword. Type .to on any value and autocomplete shows available conversions."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Type Conversion

Convert between types using dot functions. There is no cast keyword. Type `.to` after any value and autocomplete shows the conversions you can use.

---

## Safe conversions — always succeed

These always give you a value. You don't need to handle a failure.

```
// Number conversions
let price: Number = 42.7
let priceAsFloat = price.toFloat()    // 42.7 as a Float
let priceText = price.toString()      // `42.7`

// Int conversions
let playerCount: Int = 10
let countAsNumber = playerCount.toNumber()  // 10 as a Number
let countAsFloat = playerCount.toFloat()    // 10.0 as a Float
let countText = playerCount.toString()      // `10`

// Float conversions
let gravity: Float = 9.81
let gravityAsNumber = gravity.toNumber()    // 9.81 as a Number (exact decimal)
let gravityText = gravity.toString()        // `9.81`

// Boolean conversions
let isActive: Boolean = true
let activeText = isActive.toString()        // `true`
```

---

## Conversions that can fail — they return `Maybe`

Turning text into a number can fail if the text isn't a valid number. Turning a `Number` or a `Float` into an `Int` can fail too, because the value might be too big for an `Int`. These conversions return a `Maybe`, so the compiler makes you handle the failure case:

```
let input = `42`
let count = input.toInt()           // Maybe<Int>
let price = input.toNumber()        // Maybe<Number>
let velocity = input.toFloat()      // Maybe<Float>

// Handle it:
if (count.exists()) {
  print(`You entered: ${count.value}`)
}

// Or use a default:
let parsedCount = input.toInt()     // a maybe: the number, or nothing
let safeCount = parsedCount.or(0)   // a plain Int, 0 if input isn't a number

// Failure case:
let hello = `hello`
let bad = hello.toInt()             // none — `hello` is not a number
let badCount = bad.or(0)            // 0 — the default

// A number to an int drops the decimal part:
let unitPrice: Number = 42.7
let wholePrice = unitPrice.toInt()  // a Maybe<Int>: 42
```

---

## Unions — no conversion needed, just check which kind

A union is a value that can be one of several shapes. Use `is` to ask which one it is. No cast needed:

```
if (figure is Circle) {
  print(figure.radius)    // inside this block the compiler knows it is a Circle
}
```

See [Unions](REF-unions.md).

---

## No `as` keyword

There is no cast keyword. Every conversion is written out by name:

```
let n: Number = value as Number    // COMPILE ERROR: use .toNumber() instead
let n: Number = value.toNumber()   // correct
```

That way every conversion is easy to spot, easy to find with autocomplete, and handled in the same safe way.
