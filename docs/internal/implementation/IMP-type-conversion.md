---
name: "IMP-type-conversion"
description: "Design decisions for Yinz type conversion: dot-method conversions instead of an 'as' keyword, the safe-vs-unsafe (maybe-returning) split, and rejection of the ternary operator."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Type Conversion — Design Decisions

User spec: [`docs/reference/REF-type-conversion.md`](../../reference/REF-type-conversion.md)

---

## Dot Methods over `as` Keyword

All type conversions are dot methods. No `as` keyword. No angle bracket casts. Type `.to` on any value and autocomplete shows all available conversions.

**Conversion is a dot method, not an `as` keyword**: a developer types `.to` and autocomplete shows exactly what is available, and `value.toNumber()` reads on its own.

**Dot-first design (Golden Rule 11)**: If something can be a dot method with autocomplete, it should be. Type conversion is exactly this case.

---

## Safe vs Unsafe Conversion Split

Conversions that always succeed return the value directly. Conversions that might fail return `Maybe`.

**Safe (always succeed)**: Numeric type widening and narrowing, boolean to string, number to string. The result is always valid.

**Unsafe (might fail)**: String parsing. `"hello".toInt()` has no valid result. Returns `Maybe<Int>` — the compiler forces the caller to handle the failure case.

**Why this split**: The `Maybe` return type is the language's mechanism for expressing "this might not work." Applying it to conversions that can fail is consistent with the same pattern used everywhere else (`.get()`, `.find()`, `.first()`). The compiler enforces handling at the call site rather than letting bad parses produce garbage values silently.

---

## No Ternary Operator

No `condition ? a : b`. The pre-assignment pattern covers the same use case:
```
let value = `default`
if (condition) {
  value = "other"
}
```

**Why**: Ternaries are compact but hard to read and hard to extend. Pre-assignment is two lines but self-explanatory at any experience level. Consistent with the "step-by-step over chaining" philosophy (Golden Rule 9).
