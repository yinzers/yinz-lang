---
name: "IMP-functions"
description: "Design decisions for Yinz function syntax: the 'function' keyword over 'fn', explicit '-> Nothing' return type, and rejecting tuples in favor of named return types."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Functions — Design Decisions

User spec: [`docs/reference/REF-functions.md`](../../reference/REF-functions.md)

---

## `function` Keyword over `fn`

`function` spells it out. `fn` saves two characters.

**Why**: Golden Rule 13 (fast to type without sacrificing readability) and Golden Rule 4 (self-documenting). `fn` requires knowing the abbreviation. `function` is immediately readable by anyone who has seen JavaScript. The extra characters are worth it.

---

## `-> Nothing` over `void`

Functions that don't return a value declare `-> Nothing`. Always required — no omitting the return type.

**Why**: `void` is CS jargon that means nothing to a non-programmer. `Nothing` reads like English — "this function returns nothing." Requiring it explicitly eliminates ambiguity in team codebases about whether a missing return type means void or was forgotten.

---

## No Tuples — Define a Shape Instead

No tuple type. Returning multiple values requires declaring a named `shape`.

**Why**: Positional slots are anonymous — `result.0` and `result.1` tell you nothing. Named fields are self-documenting: `result.quotient` and `result.remainder` need no documentation.

---

## Full Closure Syntax — Three Forms

Arrow functions have three forms depending on how much type information is needed:

```
p => expr                       // simple — types inferred
p => { ... }                    // multi-line — types inferred
(p: T) -> R => { body }        // typed — explicit params and return type
```

**Why**: Consistent with named functions — same structure (params → return type → body), just `=>` where the function name would be. Return type only required when the compiler can't infer it. Complex callbacks like HTTP route handlers need typed closures to define contracts explicitly.

**Arrow functions are for callbacks only**: Standalone named functions use `function`. Arrow syntax is reserved for inline callbacks passed to other functions. This keeps the two uses visually distinct.

---

## Default Argument Values — Ownership Prevents Shared Mutable Defaults

A default value is never shared between calls. The ownership system guarantees it: a default like `= []` creates a fresh owned value for each call that omits the argument, so there is nothing left for a second call to share.

```ynz
function addTo(items: Array<String>, list: Array<String> = []) -> Array<String> {
  ...
}
```

The `[]` default is owned, so the ownership rules cover it with no special "evaluate fresh per call" rule.
