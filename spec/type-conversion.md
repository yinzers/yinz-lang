# Type Conversion

Convert between types using dot methods. No cast keyword. Type `.to` on any value and autocomplete shows available conversions.

---

## Safe conversions — always succeed

These always produce a value. No error handling needed.

```
// Number conversions
let n: number = 42.7
let i = n.toInt()        // 42 — truncates decimal
let f = n.toFloat()      // 42.7 as float
let s = n.toString()     // "42.7"

// Int conversions
let count: int = 10
let n = count.toNumber() // 10 as number
let f = count.toFloat()  // 10.0 as float
let s = count.toString() // "10"

// Float conversions
let v: float = 9.81
let n = v.toNumber()     // 9.81 as number (exact decimal)
let s = v.toString()     // "9.81"

// Bool conversions
let active: bool = true
let s = active.toString() // "true"
```

---

## Unsafe conversions — might fail, return `maybe`

Parsing a string into a number might fail if the string isn't valid. These return `maybe` so the compiler forces you to handle the failure case:

```
let input = "42"
let count = input.toInt()           // maybe int
let price = input.toNumber()        // maybe number
let velocity = input.toFloat()      // maybe float

// Handle it:
if (count.exists()) {
  print(`You entered: ${count.value}`)
}

// Or use a default:
let count = input.toInt().or(0)

// Failure case:
let bad = "hello".toInt()           // none — "hello" is not a number
let bad = "hello".toInt().or(0)     // 0 — fallback
```

## Parsing non-decimal strings

For hex, binary, and octal strings, use explicit methods. All return `maybe int` — the string might not be valid for that base.

```
"FF".fromHex()        // maybe int — 255
"ff".fromHex()        // maybe int — 255 (case-insensitive)
"1010".fromBinary()   // maybe int — 10
"17".fromOctal()      // maybe int — 15

// With fallback:
"FF".fromHex().or(0)  // 255
"ZZ".fromHex().or(0)  // 0 — "ZZ" is not valid hex

// Failure case:
"GG".fromHex()        // none — G is not a valid hex digit
"19".fromOctal()      // none — 9 is not a valid octal digit
```

There is no "parse with radix" argument. Each base has its own method — unambiguous, no magic numbers.

---

## Union types — no conversion needed, just check which kind

For union types, use `is` to ask which kind it is. No cast required:

```
if (shape is Circle) {
  print(shape.radius)    // compiler already knows shape is Circle here
}
```

See [Unions](unions.md).

---

## No `as` keyword

There is no cast keyword. Every conversion is explicit and named:

```
let n: number = value as number    // COMPILE ERROR: use .toNumber() instead
let n: number = value.toNumber()   // correct
```

This keeps conversions visible, autocomplete-discoverable, and consistently safe.
