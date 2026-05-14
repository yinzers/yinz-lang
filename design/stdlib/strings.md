# Standard Library — String Utilities

String methods available on all `string` values. No import needed.

---

## Basic Operations

```
let upper = name.toUpper()
let lower = name.toLower()
let trimmed = input.trim()
let parts = csv.split(",")
let joined = names.join(", ")
let length = name.length
let sub = name.slice(0, 3)
```

---

## Searching & Testing

```
let contains = name.contains("trick")
let starts = name.startsWith("Pat")
let ends = name.endsWith("ick")
```

---

## Transforming

```
let replaced = name.replace("Pat", "Fr")
let padded = "42".padLeft(5, "0")              // "00042"
```

---

## Pattern Matching

```
let match = email.matches("[a-z]+@[a-z]+\\.[a-z]+")    // -> bool
let found = text.findAll("[0-9]+")                      // -> array<string>
```

---

## Expansion Candidates

- String builder for efficient concatenation in loops
- Unicode normalization
- Encoding/decoding (UTF-8, ASCII, Base64)
- Pluralization helpers
- Slug generation
- HTML escaping
- URL encoding/decoding
- Levenshtein distance (string similarity)
- Fuzzy matching
