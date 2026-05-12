# Type Conversion — Design Decisions

User spec: `spec/type-conversion.md`

---

## Dot Methods over `as` Keyword

All type conversions are dot methods. No `as` keyword. No angle bracket casts. Type `.to` on any value and autocomplete shows all available conversions.

**Why no `as`**: `as` is a programmer keyword that requires knowing the type system to use correctly. Dot methods are autocomplete-discoverable — a developer types `.to` and sees exactly what's available. `value.toNumber()` is self-documenting; `value as number` requires knowing what `as` does.

**Dot-first design (Golden Rule 1)**: If something can be a dot method with autocomplete, it should be. Type conversion is exactly this case.

---

## Safe vs Unsafe Conversion Split

Conversions that always succeed return the value directly. Conversions that might fail return `maybe`.

**Safe (always succeed)**: Numeric type widening and narrowing, bool to string, number to string. The result is always valid.

**Unsafe (might fail)**: String parsing. `"hello".toInt()` has no valid result. Returns `maybe int` — the compiler forces the caller to handle the failure case.

**Why this split**: The `maybe` return type is the language's mechanism for expressing "this might not work." Applying it to conversions that can fail is consistent with the same pattern used everywhere else (`.get()`, `.find()`, `.first()`). The compiler enforces handling at the call site rather than letting bad parses produce garbage values silently.

---

## No Ternary Operator

No `condition ? a : b`. The pre-assignment pattern covers the same use case:
```
let value = "default"
if (condition) {
  value = "other"
}
```

**Why**: Ternaries are compact but hard to read and hard to extend. Pre-assignment is two lines but self-explanatory at any experience level. Consistent with the "step-by-step over chaining" philosophy (Golden Rule 7).
