---
name: "REF-compiler-errors"
description: "The compiler's character as a teacher (Golden Rule 3, docs/reference/REF-teaching-mission.md) is set by its error messages. If they use jargon, the language fails its own promise — regardless of how good the syntax design is...."
tags:
  - "yinz-compiler"
created_at: "2026-05-14"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Compiler Error Messages — Style Spec

The compiler's error messages show whether it really acts as a teacher (Golden Rule 3, [REF-teaching-mission.md](REF-teaching-mission.md)). If they use jargon, the language breaks its own promise, however good the syntax is. This page lists what every error message must do.

---

## Required Format — Three Parts

Every compiler diagnostic (error, warning, suggestion) has three parts:

- **WHAT** — one sentence stating the issue in plain English. It is the headline of the message.
- **WHAT TO DO INSTEAD** — a fix you can copy. It is printed next to the code it points at.
- **WHY** — the reason: correctness, performance, convention, or safety. It is printed as a note under the code.

**Severity:**
- `Error` — the program won't compile
- `Warning` — the program compiles, but something looks wrong
- `SUGGESTION` — an IDE hint, the lowest urgency

A program that stops while it runs prints `RUNTIME ERROR:` instead. The compiler usually catches these problems first, but some can only be found while the program runs.

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
- `Maybe<T>`, `Maybe`, `none` — values that might not exist
- `follows`, "follows the X contract" — how a shape says it provides the functions a contract lists
- `share` / `lend` / `give` / `copy` / `.freeze` — ownership words
- `options` — a fixed set of named choices
- "type", "field", "function" — basic words that everyone understands
- `Number`, `Float`, `Int`, `Number<N>` — the number type names
- `Array`, `Fixed`, `Map` — the collection type names
- `wait`, `background` — the keywords for running things at the same time
- `code point`, `grapheme`, and byte counts (`byteCount`, `byteAt`) — text terms (kept because the alternatives are worse)

If a word is the official name of a Yinz feature, use it. The ban-list covers programmer jargon, not Yinz's own vocabulary.

---

## Banned Keywords in Your Own Code

This is different from the jargon ban-list above. The jargon ban-list covers WORDS in the messages the compiler prints (a test checks every message). Banned keywords cover words in YOUR source code: if you write one, the compiler prints a three-part teaching message that points you to the Yinz keyword.

| If you write | Write this in Yinz |
|--------------|--------------------|
| `type Foo { ... }` | `shape Foo { ... }` |
| `struct Foo { ... }` | `shape Foo { ... }` |
| `class Foo { ... }` | `shape Foo { ... }` |
| `interface Foo { ... }` | `shape Foo { ... }` (or `follows` for contracts) |
| `enum Status { ... }` | `options Status { ... }` |
| `abstract` | `base shape` |

"Type" is not on the jargon ban-list, because it appears in many messages in its everyday sense ("return type", "this type"). It is only refused when you use it to declare something in your own code.

The teaching message for `type Foo { name: String }` is a warning. Its three parts:

- **WHAT:** `` `type` is not a keyword in Yinz. ``
- **WHAT TO DO INSTEAD:** ``Use `shape` to declare a data type: `shape Player { name: String, health: Int }` ``
- **WHY:** why Yinz has one keyword for every data declaration.

---

## Type Names in Your Own Code

Every type name starts with a capital letter, built-in types included. Two mistakes get a teaching message.

**A lowercase built-in type name.** Writing `int`, `float`, `number`, `boolean`, `string`, `range`, `maybe`, `array`, `fixed`, `map`, `channel` or `nothing` where a type goes is an error. The editor offers a quick-fix titled ``Write `Int` instead of `int` `` (with the names you wrote) that rewrites the name:

```ynz
function entrypoint() -> Nothing {
  let health: int = 100
}
```

```
`int` is not a type name.
  Write `Int`.
  Why: Every type in Yinz starts with a capital letter, built-ins included.
```

**A shape named like a built-in type.** A `shape` or union alias called `Int`, `Float`, `Number`, `Boolean`, `String`, `Range`, `Maybe`, `Array`, `Fixed`, `Map`, `Channel`, `Nothing`, `MapEntry`, `Frame` or `SourceLoc` is an error:

```ynz
shape Map {
  name: String
}
```

```
`Map` is a reserved built-in type name.
  Give this shape another name, such as `MyMap` or one that says what it holds.
  Why: Yinz already has a built-in type called `Map`. A shape with the same name would make every `Map` in this file ambiguous between your shape and the built-in.
```

Functions, modules and variables keep lowercase names, and they may match a type name in lowercase: `range(1, 10)` is a function that returns a `Range`.

---

## Tone Guide

- **Direct, not talking down.** "You can't add to a fixed array" is fine. "Oh no! It looks like you tried to..." is too cute.
- **Don't blame.** Say "the compiler now treats X as Y", not "you tried to do Y."
- **Suggest something specific.** "Use `Array<T>` instead", with the actual replacement code. Not "consider an alternative."
- **Use the user's own names.** "`scores` is a `Map<String, Int>`", not "this variable is a map." Point at what they wrote.
- **No exclamation marks.** Errors are not exciting.
- **No emoji.** Plain text only.

---

## Messages Step by Step

**A good compile error** names the user's own variable, gives code that can be copied, and explains the reason for this code. For a duplicate import:

- **WHAT:** `` Import name `Player` is already bound by a previous import. ``
- **WHAT TO DO INSTEAD:** ``Use a different local name: `import { Player as otherName } from `...` ``
- **WHY:** Each imported name must be unique in the local scope.

**A good runtime error** says where it happened and how to fix it:

```
RUNTIME ERROR: <the operation> at src/main.ynz:12:5

  The value wrapped past the maximum (or minimum) for this type.

  Use .wrappingAdd() if wrap-around is intentional.

  Why: Yinz panics on integer overflow by default to prevent silent data corruption.
```

**A good suggestion** tells you what to write and why it helps. For `sleepBlocking` in a program with a scheduler:

- **WHAT:** `` `sleepBlocking` parks the current thread — it sits idle and runs nothing else until the time is up. ``
- **WHAT TO DO INSTEAD:** ``Write `wait sleep(100)` — your function pauses just the same, but the thread stays free to run other tasks.``
- **WHY:** `wait sleep` hands the thread back to the scheduler during the pause. `sleepBlocking` keeps the thread parked doing nothing.

---

## Reporting Many Errors at Once

The compiler reports ALL the errors it finds in one run, not just the first. Two reasons:

1. **Faster fixing.** A user who sees 5 errors at once can fix them in one sitting, instead of fixing one, recompiling, seeing the next, and repeating five times.
2. **Seeing the chain.** If error 3 was caused by error 1, showing both together lets the user see why.

**Limit:** the compiler prints the first 50 errors. After them it prints `... and N more errors hidden`. This stops a flood of thousands of errors when something basic is wrong (such as a missing bracket confusing everything after it). Warnings and suggestions are never counted against the limit.

Type checking only runs on code that parsed cleanly. There is no point checking the types of code the compiler couldn't read.

---

## Checklist for Reviewing an Error Message

When you review an error message, check that it:

- [ ] Has a clear WHAT, WHAT-INSTEAD and WHY (the three required parts)
- [ ] Uses no word from the ban-list
- [ ] Uses the user's own variable, type and function names where it can
- [ ] Has at least one concrete code suggestion
- [ ] Sounds direct, not cute or blaming
- [ ] Has no exclamation marks and no emoji

A message that fails ANY of these gets rewritten. This is a plain check, not a judgment call.

---

## Adding a New Error Message

Every new error message must:
1. Use the three-part format
2. Pass the jargon ban-list
3. Be shown in the docs page for its feature
4. Pass the checklist above

If a reviewer can't tell what a message is teaching, the message isn't doing its job. Rewrite it.
