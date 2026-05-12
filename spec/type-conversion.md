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
