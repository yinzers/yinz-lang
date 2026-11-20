---
name: "REF-compiler-errors"
description: "The compiler's character as a teacher (Golden Rule 3, docs/reference/REF-teaching-mission.md) is set by its error messages. If they use jargon, the language fails its own promise — regardless of how good the syntax design is...."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Compiler Error Messages — Style Spec

The compiler's error messages show whether it really acts as a teacher (Golden Rule 3, [REF-teaching-mission.md](REF-teaching-mission.md)). If they use jargon, the language breaks its own promise, however good the syntax is. This page lists what every error message must do.

---

## Required Format — Three Parts

Every compiler diagnostic (error, warning, suggestion) follows this shape:

```
[severity prefix]: [WHAT — one sentence stating the issue in plain English]

  [WHAT TO DO INSTEAD — corrected code, ready to copy]

  [WHY — the reason: correctness, performance, convention, or safety]
```

**Severity prefixes:**
- `COMPILE ERROR:` — the program won't compile
- `RUNTIME ERROR:` — the program stops while it runs; the compiler usually catches these first, but some can only be found while the program runs
- `WARNING:` — the program compiles, but something looks wrong
- `SUGGESTION:` — an IDE hint, the lowest urgency

**A message missing any of the three parts is incomplete.** A "WHAT" without a "WHAT TO DO INSTEAD" leaves the user stuck. A "WHAT" with a fix but no "WHY" teaches the user to apply fixes without understanding them, which is exactly what the teaching mission is meant to prevent.

---

## The Jargon Ban-List

These words must NEVER appear in messages the user reads. They need computer-science background that Yinz's readers may not have. Use the plain-English replacement instead.

| Jargon (banned) | Plain English (use this) |
|-----------------|---------------------------|
| propagate, propagation | "the error cascades to the caller", "an unhandled error cascades up through the call stack". **Exception:** "auto-propagation" / "auto-propagate" stays as Yinz's name for the feature, but the FIRST use in any user-facing doc must explain it in plain English ("the error cascades to the caller automatically"). |
| narrow, narrowing | "the compiler now treats X as a [type]", "from here on, X is [type]" |
| discriminator | "the tag that says which kind it is", "the label" |
| infer, inference | "figure out automatically", "guess from context", "work out from the value" |
| polymorphic | "works with any type", "is generic" |
| monomorphize, monomorphic | "make a separate version for each type used", "specialized" |
| covariant, contravariant | "fits anywhere a parent type fits" / "works in reverse" |
| lifetime | "how long this value stays around", "how long this exists" |
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
| residual | "what's left over" |
| associated type | (avoid this concept until we have a user-facing name for it) |
| implementation (generic CS sense) | Describe what it does; avoid "implementation detail" |
| referentially transparent | Avoid — describe the behavior directly |
| immutable | "can't be changed", "fixed" |
| mutable | "can be changed" |
| AST, abstract syntax tree | "the structure of your code", "the parsed code" |
| invariant (math sense) | "always true", "must hold" |
| precondition / postcondition | "must be true before", "must be true after" |
| invariant violation | "this assumption was broken" |

**When in doubt, use the plain-English form even if it is a little longer.** A junior developer who has to look up "monomorphize" will close the compiler output and ask someone else. That is a failure.

**Math words that help teaching are NOT jargon.** `remainder`, `quotient` and `divisor` are easier for a newcomer than the symbols `%` and `/`. Only words from inside compilers (`monomorphize`, `propagate`) belong on this list. If a word helps a non-programmer understand what an operation does, it is welcome in a message.

---

## What's NOT Banned (Yinz-Specific Terms)

These are Yinz's own names — use them freely:

- `errors`, `errors function` — the language's name for the failure system
- `maybe<T>`, `maybe`, `none` — values that might not exist
- `follows`, "follows the X contract" — Yinz's word for what other languages call an interface or trait
- `share` / `lend` / `give` / `copy` / `.freeze` — ownership words
- `options` — Yinz's word for an enum
- `type`, `field`, `method` — basic words that everyone understands
- `number`, `float`, `int`, `number<N>` — the number type names
- `array`, `fixed`, `map` — the collection type names
- `wait`, `background` — the keywords for running things at the same time
- `setup` / `teardown` — testing keywords
- `code point`, `byte`, `grapheme` — text terms (kept because the alternatives are worse)

If a word is the official name of a Yinz feature, use it. The ban-list covers programmer jargon, not Yinz's own vocabulary.

---

## Banned Keywords in Your Own Code

This is different from the jargon ban-list above. The jargon ban-list covers WORDS in the messages the compiler prints (a test checks every message). Banned keywords cover words in YOUR source code: if you write one, the compiler stops with a three-part teaching error that points you to the Yinz keyword.

| If you write | Write this in Yinz |
|--------------|--------------------|
| `type Foo { ... }` | `shape Foo { ... }` |
| `struct Foo { ... }` | `shape Foo { ... }` |
| `class Foo { ... }` | `shape Foo { ... }` |
| `interface Foo { ... }` | `shape Foo { ... }` (or `follows` for contracts) |
| `enum Status { ... }` | `options Status { ... }` |
| `fn name() { ... }` | `function name() { ... }` |

`type` is not on the jargon ban-list, because it appears in many messages in its everyday sense ("return type", "this type"). It is only refused when you use it to declare something in your own code.

The teaching error looks like this:

```
type Foo { name: string }
^^^^
COMPILE ERROR: Yinz uses `shape` to declare a group of fields, not `type`.

  Replace `type Foo { ... }` with `shape Foo { ... }`.

  `type` is also the everyday word for "what kind of thing this is."
  `shape` is clear: a shape is the structure of your data. (Golden Rule 4:
  self-documenting syntax.)
```

---

## Tone Guide

- **Direct, not talking down.** "You can't add to a fixed array" is fine. "Oh no! It looks like you tried to..." is too cute.
- **Don't blame.** Say "the compiler now treats X as Y", not "you tried to do Y."
- **Suggest something specific.** "Use `array<T>` instead", with the actual replacement code. Not "consider an alternative."
- **Use the user's own names.** "`scores` is a `map<string, number>`", not "this variable is a map." Point at what they wrote.
- **No exclamation marks.** Errors are not exciting.
- **No emoji.** Plain text only.

---

## A Message Step by Step

```
[SEVERITY]: [WHAT — plain English, one sentence]

  [WHAT TO DO INSTEAD — corrected code, with the change visible]

  Why: [WHY — speed / correctness / convention / safety]

[Optional link: "See: docs/reference/REF-x.md#section"]
```

**Example — a good compile error:**

```
COMPILE ERROR: 'Player' is imported from two places.

  Rename one with `as`:
    import { Player } from `models/game`
    import { Player as LegacyPlayer } from `external/legacy`

  Why: If the same name came from two imports, code that used 'Player'
       would silently pick the last one imported. Rearranging imports
       could change which 'Player' your code refers to. Forcing a rename
       makes the choice visible.
```

**Example — a good runtime error:**

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

**Example — a good suggestion:**

```
SUGGESTION: All keys in this map are written out in your code.

  Consider a shape instead — it gives you direct field access:
    shape Scores { alice: number, bob: number, charlie: number }
    let scores: Scores = { alice: 90, bob: 85, charlie: 78 }

  Why: Reading a shape field is one step (about 1 processor
       instruction). Reading a map key means searching for it (about
       10-50 instructions). For keys that never change, a shape is
       about 10x faster AND gives you dot access with autocomplete.
```

---

## Reporting Many Errors at Once

The compiler reports ALL the errors it finds in one run, not just the first. Two reasons:

1. **Faster fixing.** A user who sees 5 errors at once can fix them in one sitting, instead of fixing one, recompiling, seeing the next, and repeating five times.
2. **Seeing the chain.** If error 3 was caused by error 1, showing both together lets the user see why.

**Limit:** if a file has more than 50 errors, the compiler stops after 50 and prints `... and N more errors. Fix the first batch and try again.` This stops a flood of thousands of errors when something basic is wrong (such as a missing bracket confusing everything after it).

Type checking only runs on code that parsed cleanly. There is no point checking the types of code the compiler couldn't read.

---

## Links to the Docs

Every error that has a matching docs section ends with a "See:" line:

```
COMPILE ERROR: Direct index access is not allowed.

  Use .get() — it returns maybe<T> and handles out-of-bounds safely:
    let item = items.get(5)

  Why: ...

See: docs/reference/REF-collections.md#safe-access
```

In the IDE these are clickable and jump straight to the doc. In the terminal they print as plain text.

---

## Checklist for Reviewing an Error Message

When you review an error message, check that it:

- [ ] Has a clear WHAT, WHAT-INSTEAD and WHY (the three required parts)
- [ ] Uses no word from the ban-list
- [ ] Uses the user's own variable, type and function names where it can
- [ ] Has at least one concrete code suggestion
- [ ] Sounds direct, not cute or blaming
- [ ] Has no exclamation marks and no emoji
- [ ] Links to the matching docs section, if there is one

A message that fails ANY of these gets rewritten. This is a plain check, not a judgment call.

---

## Adding a New Error Message

Every new error message must:
1. Use the three-part format
2. Pass the jargon ban-list
3. Be shown in the docs page for its feature
4. Pass the checklist above

If a reviewer can't tell what a message is teaching, the message isn't doing its job. Rewrite it.
