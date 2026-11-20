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
let n: number = 42.7
let i = n.toInt()        // 42 — drops the decimal part
let f = n.toFloat()      // 42.7 as a float
let s = n.toString()     // `42.7`

// Int conversions
let count: int = 10
let n = count.toNumber() // 10 as a number
let f = count.toFloat()  // 10.0 as a float
let s = count.toString() // `10`

// Float conversions
let v: float = 9.81
let n = v.toNumber()     // 9.81 as a number (exact decimal)
let s = v.toString()     // `9.81`

// Boolean conversions
let active: boolean = true
let s = active.toString() // `true`
```

---

## Conversions that can fail — they return `maybe`

Turning text into a number can fail if the text isn't a valid number. These conversions return a `maybe`, so the compiler makes you handle the failure case:

```
let input = `42`
let count = input.toInt()           // maybe<int>
let price = input.toNumber()        // maybe<number>
let velocity = input.toFloat()      // maybe<float>

// Handle it:
if (count.exists()) {
  print(`You entered: ${count.value}`)
}

// Or use a default:
let parsedCount = input.toInt()     // a maybe: the number, or nothing
let safeCount = parsedCount.or(0)   // a plain int, 0 if input isn't a number

// Failure case:
let hello = `hello`
let bad = hello.toInt()             // none — `hello` is not a number
let badCount = bad.or(0)            // 0 — the default
```

## Parsing hex, binary and octal text

For text in base 16 (hex), base 2 (binary) or base 8 (octal), use one function per base. All return `maybe<int>`, because the text might not be valid for that base.

```
let hexUpper = `FF`
let hexLower = `ff`
let binaryText = `1010`
let octalText = `17`

hexUpper.fromHex()       // maybe<int> — 255
hexLower.fromHex()       // maybe<int> — 255 (upper or lower case both work)
binaryText.fromBinary()  // maybe<int> — 10
octalText.fromOctal()    // maybe<int> — 15

// With a default:
let validHex = hexUpper.fromHex()
let validValue = validHex.or(0)     // 255
let badHexText = `ZZ`
let invalidHex = badHexText.fromHex()
let invalidValue = invalidHex.or(0) // 0 — `ZZ` is not valid hex

// Failure case:
let wrongHex = `GG`
let wrongOctal = `19`
wrongHex.fromHex()       // none — G is not a hex digit
wrongOctal.fromOctal()   // none — 9 is not an octal digit
```

There is no "choose the base" argument. Each base has its own function, so there is no number to remember or get wrong.

---

## Unions — no conversion needed, just check which kind

A union is a value that can be one of several shapes. Use `is` to ask which one it is. No cast needed:

```
if (shape is Circle) {
  print(shape.radius)    // inside this block the compiler knows it is a Circle
}
```

See [Unions](REF-unions.md).

---

## No `as` keyword

There is no cast keyword. Every conversion is written out by name:

```
let n: number = value as number    // COMPILE ERROR: use .toNumber() instead
let n: number = value.toNumber()   // correct
```

That way every conversion is easy to spot, easy to find with autocomplete, and handled in the same safe way.
