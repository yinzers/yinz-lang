# Compiler Error Messages — Style Spec

The compiler's character as a teacher (Golden Rule 11, `design/teaching-mission.md`) is set by its error messages. If they use jargon, the language fails its own promise — regardless of how good the syntax design is. This file is the contract every error message must satisfy.

---

## Required Format — Three Parts

Every compiler diagnostic (error, warning, suggestion) follows this shape:

```
[severity prefix]: [WHAT — one sentence stating the issue in plain English]

  [WHAT TO DO INSTEAD — corrected code, ready to copy]

  [WHY — the reason: correctness, performance, convention, or safety]
```

**Severity prefixes:**
- `COMPILE ERROR:` — won't compile
- `RUNTIME ERROR:` — would crash at runtime; usually caught at compile time, sometimes runtime-only
- `WARNING:` — compiles, but indicates a problem
- `SUGGESTION:` — IDE hint, lowest urgency

**A diagnostic missing any of the three parts is incomplete.** A "WHAT" without "WHAT TO DO INSTEAD" leaves the user stuck. A "WHAT" with a fix but no "WHY" trains the user to apply fixes without understanding — exactly the failure mode the teaching mission rejects.

---

## The Jargon Ban-List

These words should NEVER appear in user-facing diagnostics. They require CS background knowledge that Yinz's audience may not have. Use the plain-English replacement instead.

| Jargon (banned) | Plain English (use this) |
|-----------------|---------------------------|
| propagate, propagation | "the error cascades to the caller", "an unhandled error cascades up through the call stack". **Exception:** "auto-propagation" / "auto-propagate" stays as Yinz's name for the feature, but the FIRST use in any user-facing doc must explain it in plain English ("the error cascades to the caller automatically"). |
| narrow, narrowing | "the compiler now treats X as a [type]", "from here on, X is [type]" |
| discriminator | "the tag that says which kind it is", "the label" |
| infer, inference | "figure out automatically", "guess from context", "work out from the value" |
| polymorphic | "works with any type", "is generic" |
| monomorphize, monomorphic | "make a separate version for each type used", "specialized" |
| covariant, contravariant | "fits anywhere a parent type fits" / "works in reverse" |
| lifetime (Rust sense) | "how long this value stays around", "how long this exists" |
| deref, dereference | "look up the value", "follow the reference" |
| alias (when not the syntax keyword) | "another name for the same thing" |
| shadow, shadowing | "using the same name as something declared outside this block" |
| coerce, coercion | "automatic conversion" |
| trait, interface | Use `follows` and "contract" (Yinz's terms) |
| fallible, infallible | "can fail", "can't fail" |
| first-class | Describe what makes it work, don't use the term |
| idiomatic | Replace with "typical Yinz style is..." or "the usual way is..." |
| ADT, algebraic data type | "a type that can be one of several shapes" |
| arity | "number of arguments" |
| variadic | "takes any number of arguments" |
| residual, remainder | "what's left over" |
| associated type | (avoid this concept until we have a user-facing name for it) |
| implementation (generic CS sense) | Describe what it does; avoid "implementation detail" |
| referentially transparent | Avoid — describe the behavior directly |
| immutable | "can't be changed", "fixed" |
| mutable | "can be changed" |
| AST, abstract syntax tree | "the structure of your code", "the parsed code" |
| invariant (math sense) | "always true", "must hold" |
| precondition / postcondition | "must be true before", "must be true after" |
| invariant violation | "this assumption was broken" |

**When in doubt, use the plain-English form even if slightly longer.** A jr dev who has to look up "monomorphize" is a jr dev who'll close the compiler output and ask a senior dev. That's a failure.

---

## What's NOT Banned (Yinz-Specific Terms)

These are Yinz's chosen names — use them freely:

- `errors`, `errors function`, `errors context` — the language's name for the system
- `maybe T`, `maybe`, `none` — the optional-value system
- `follows`, "follows the X contract" — Yinz's `interface`/`trait` replacement
- `share` / `lend` / `give` / `copy` / `.freeze` — ownership modifiers
- `options` — Yinz's `enum` replacement
- `type`, `field`, `method` — basic structure terms; universally understood
- `number`, `float`, `int`, `number<N>` — the type names
- `array`, `fixed`, `map` — collection type names
- `wait`, `background` — concurrency keywords
- `setup` / `teardown` — testing keywords
- `code point`, `byte`, `grapheme` — Unicode terms (kept because the alternatives are worse)

If a word is the official name of a Yinz feature, use it. The ban-list applies to programmer jargon, not Yinz's own vocabulary.

---

## Banned Declaration Keywords (Lexer-Level Enforcement)

These are different from the jargon ban-list above. The jargon ban-list catches WORDS in user-facing diagnostic STRINGS (enforced by `crates/ynz-diagnostics/tests/jargon_audit.rs`). Banned declaration keywords catch SYNTAX in user SOURCE — when the user writes the banned keyword in their own Yinz code, the lexer emits a three-part teaching error pointing to the Yinz keyword.

| Banned keyword (in user source) | Yinz keyword | Lands |
|---------------------------------|---------------|-------|
| `type Foo { ... }` | `shape Foo { ... }` | M4 lexer when shape is reserved |
| `struct Foo { ... }` | `shape Foo { ... }` | M4 lexer |
| `class Foo { ... }` | `shape Foo { ... }` | M4 lexer |
| `interface Foo { ... }` | `shape Foo { ... }` (or `follows` for contracts) | M4 lexer |
| `enum Status { ... }` | `options Status { ... }` | M4 lexer |
| `fn name() { ... }` | `function name() { ... }` | M3 lexer (already reserved as a banned keyword) |

**Important distinction**: this is NOT a BANNED_JARGON entry. The word `type` appears legitimately in many diagnostic strings (`"named type"`, `"return type"`, `"type annotation"`, `"this type"`) — banning it as a whole-word check in diagnostics would produce false positives everywhere. The ban is at the LEXER level for user-written source code only.

When M4 reserves the `shape` keyword in the lexer, it ALSO adds `type`, `struct`, `class`, `interface` as banned-keyword tokens with three-part teaching diagnostics. The teaching error format:

```
type Foo { name: string }
^^^^
COMPILE ERROR: Yinz uses `shape` to declare a data structure, not `type`.

  Replace `type Foo { ... }` with `shape Foo { ... }`.

  `type` is overloaded — it's also the everyday word for "what kind of thing this is."
  `shape` is unambiguous: a shape is the structure of your data. (Golden Rule 2:
  self-documenting syntax.)
```

This pattern (banned-keyword as lexer diagnostic) was already used for `fn` in M3 — see `crates/ynz-parser/src/lexer.rs:664` for the existing implementation pattern.

---

## Tone Guide

- **Direct, not condescending.** "You can't add to a fixed array" is fine. "Oh no! It looks like you tried to..." is too cute.
- **No accusatory voice.** Say "the compiler now treats X as Y" not "you tried to do Y."
- **Suggest concretely.** "Use `array<T>` instead" with the actual replacement code. Not "consider an alternative."
- **Quote the user's variable/type/function names.** "`scores` is a `map<string, number>`" — not "this variable is a map type." Reference what they wrote.
- **No exclamation marks.** Errors are not exciting.
- **No emoji.** Plain text only.

---

## Format Anatomy (concrete template)

```
[SEVERITY]: [WHAT — plain English, one sentence]

  [WHAT TO DO INSTEAD — corrected code, with the change visible]

  Why: [WHY — performance / correctness / convention / safety]

[Optional cross-reference: "See: spec/X.md#section"]
```

**Example — well-formed compile error:**

```
COMPILE ERROR: 'Player' is imported from two places.

  Rename one with `as`:
    import { Player } from "models/game"
    import { Player as LegacyPlayer } from "external/legacy"

  Why: If the same name came from two imports, code that used 'Player'
       would silently pick the last one imported. Rearranging imports
       could change which 'Player' your code refers to. Forcing a rename
       makes the choice visible.
```

**Example — well-formed runtime error:**

```
RUNTIME ERROR: integer overflow at line 12.

  count was at the maximum value (9223372036854775807) and would have
  wrapped to a negative number.

  Use .wrappingAdd() if you want wrap-around behavior on purpose:
    let safe = count.wrappingAdd(1)

  Or .saturatingAdd() to stay at the maximum:
    let capped = count.saturatingAdd(1)

  Why: Wrap-around is almost always a bug, not a feature. Letting it
       happen silently would corrupt later math. Yinz makes it loud
       at the call site so the bug is caught early.
```

**Example — well-formed suggestion:**

```
SUGGESTION: All keys in this map are compile-time strings.

  Consider a type instead — gives you direct field access:
    shape Scores { alice: number, bob: number, charlie: number }
    let scores: Scores = { alice: 90, bob: 85, charlie: 78 }

  Why: Type field access compiles to a single memory lookup (~1 CPU
       instruction). Map key access requires a hash lookup (~10-50
       instructions). For static keys, types are ~10x faster AND
       give you dot-access syntax with autocomplete.
```

---

## Multi-Error Reporting Strategy

The compiler reports ALL errors it finds in one pass — not just the first. Two reasons:

1. **Fix-it loop efficiency.** A user who sees 5 errors at once can fix them in one editing session instead of "fix one, recompile, see the next, fix one, recompile" five times.
2. **Catching cascades.** If error 3 was actually caused by error 1, showing both together lets the user see the chain.

**Cap:** if a file has more than 50 errors, stop after 50 and print `"... and N more errors. Fix the first batch and try again."` This prevents 10,000-error console floods when something fundamental is wrong (like a missing semicolon causing the parser to derail).

Errors from later passes (type check) only run if earlier passes (parse) have no errors in the relevant scope. No point type-checking unparseable code.

---

## Cross-References

Every error that has a corresponding spec section ends with a "See:" line:

```
COMPILE ERROR: Direct index access is not allowed.

  Use .get() — it returns maybe T and handles out-of-bounds safely:
    let item = items.get(5)

  Why: ...

See: spec/collections.md#safe-access
```

The IDE makes these clickable, jumping straight to the doc. Terminal output prints them as text.

---

## Audit Checklist (for reviewing existing error examples)

When auditing an error message in any spec or design file, verify:

- [ ] Has a clear WHAT, WHAT-INSTEAD, and WHY (the three required parts)
- [ ] Uses no jargon from the ban-list
- [ ] Quotes the user's variable/type/function names where applicable
- [ ] Has at least one concrete code suggestion
- [ ] Tone is direct, not cutesy or accusatory
- [ ] No exclamation marks, no emoji
- [ ] Cross-references the relevant spec section if applicable

A diagnostic that fails ANY of these gets rewritten. This is mechanical — it's an audit, not a judgment call.

---

## When New Rules / Features Add New Errors

Every PR that introduces new error messages must:
1. Use the three-part format
2. Pass the jargon ban-list
3. Include the error example in the relevant spec/design file (for documentation)
4. Get reviewed against this checklist

If a reviewer can't tell what a diagnostic is teaching, the diagnostic isn't doing its job. Rewrite it.
