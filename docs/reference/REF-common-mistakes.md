---
name: "REF-common-mistakes"
description: "If you're coming from JavaScript, Python, TypeScript, PHP, or another language, a few things work differently in Yinz. The compiler gives specific suggestions for each of these."
tags:
  - "yinz-compiler"
created_at: "2026-05-18"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Common Mistakes from Other Languages

If you're coming from JavaScript, Python, TypeScript, PHP, or another language, a few things work differently in Yinz. The compiler gives specific suggestions for each of these.

---

## Comments — use `//` not `#`

```
# this is a comment
// COMPILE ERROR: Yinz uses `//` for comments, not `#`.
// Change `#` to `//`: `// this is a comment`
```

```ynz
// this is a comment    ← correct
```

---

## Semicolons — not used in Yinz

```
let x = 5;
// COMPILE ERROR: Semicolons are not used in Yinz.
// Remove the `;` — Yinz uses newlines to end statements.
```

```ynz
let x = 5    ← correct — newline ends the statement
```

---

## Variable prefix `$` — not used in Yinz

```
let y = $x
// COMPILE ERROR: Variables in Yinz don't use a `$` prefix.
// Remove the `$`: write `x` instead of `$x`.
```

```ynz
let y = x    ← correct
```

---

## Optional suffix `?` — use `maybe<T>` instead

```
let z: int? = none
// COMPILE ERROR: The `?` optional suffix is not valid in Yinz.
// Use `maybe<T>` instead: `maybe<int>` not `int?`.
```

```ynz
let z: maybe<int> = none    ← correct
```

---

## String quotes — use backticks

See [Strings](REF-strings.md) for the full explanation. Short version: backticks only, always.

```ynz
let name = `Patrick`    ← correct
```

---

## Collection type casing — lowercase always

See [Collections](REF-collections.md) for details. `array`, `fixed`, `map` are all lowercase.

```ynz
let scores: array<int> = []    ← correct
```
