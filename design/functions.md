# Functions — Design Decisions

User spec: `spec/functions.md`

---

## `function` Keyword over `fn`

`function` spells it out. `fn` saves two characters.

**Why**: Golden Rule 9 (fast to type without sacrificing readability) and Golden Rule 2 (self-documenting). `fn` requires knowing the abbreviation. `function` is immediately readable by anyone who has seen JavaScript. The extra characters are worth it.

---

## `-> nothing` over `void`

Functions that don't return a value declare `-> nothing`. Always required — no omitting the return type.

**Why**: `void` is CS jargon that means nothing to a non-programmer. `nothing` reads like English — "this function returns nothing." Requiring it explicitly eliminates ambiguity in team codebases about whether a missing return type means void or was forgotten.

---

## No Tuples — Define a Type Instead

No tuple type. Returning multiple values requires defining a named type.

**Why**: Positional tuple slots are anonymous — `result.0` and `result.1` tell you nothing. Named fields are always self-documenting. The slight extra ceremony of defining a type pays off immediately in readability. `result.quotient` and `result.remainder` require no documentation.

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

Python's mutable default argument bug (`def append(x, lst=[])` sharing one list across all callers) cannot occur in Yinz. The ownership system prevents it by construction: a default value like `= []` creates an owned value. The first call that omits the argument takes ownership — moves it in. There is nothing left for a second call to share.

```ynz
function addTo(items: array<string>, list: array<string> = []) -> array<string> {
  ...
}
```

The `[]` default is owned. Ownership rules handle the rest — no special "evaluate fresh per call" compiler rule needed, no performance cost from repeated evaluation of expensive defaults.
