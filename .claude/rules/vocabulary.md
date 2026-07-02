# Yinz Vocabulary — Official Terms

This is the authoritative reference for Yinz user-facing terminology. **All user-facing docs (`spec/`), design docs (`design/`), compiler diagnostics, and Claude-chat output use these terms.** Never use legacy terms from other languages.

For internal-vs-user-facing audience distinctions (e.g., `infer`/`inference` allowed in design docs but banned in compiler errors), see [`.claude/rules/inference.md`](inference.md).

---

## Quick Reference

| Concept | Yinz Term | NOT |
|---|---|---|
| Declare a data structure | `shape` | type, struct, class, interface |
| An instance with a shape | value (or just "a Player") | object, instance, struct |
| Dynamic key-value collection | `map<K, V>` | dictionary, HashMap, hash, object |
| Growable list | `array<T>` | Vec, list, dynamic array |
| Stack-allocated fixed list | `fixed<T>` | static array, stack array, fixed-size array |
| Enum replacement | `options` | enum, enumeration |
| A-or-B type relationship | `union` (via `\|`) | sum type, variant type, `or` keyword (rejected — see Golden Rule 12 exception) |
| Optional/maybe value | `maybe T` | Optional, Option, nullable |
| Absent value | `none` | null, undefined, None, nil |
| No return value | `nothing` | void, unit, () |
| Function declaration | `function` | fn, func, def, method |
| The implementing type (in `follows`) | `Self` | self, this (capital S only for the type) |
| The instance (lowercase) | `self` | this, instance |
| Read-only borrow (signature only) | `share` keyword in signature; compiler-inferred at call sites | &T, shared ref, immutable borrow. NO body-level `.share()` syntax. |
| Mutable borrow (signature only) | `lend` keyword in signature; compiler-inferred at call sites | &mut T, mutable ref. NO body-level `.lend()` syntax. |
| Ownership transfer (signature only) | `give` keyword in signature; compiler-inferred at call sites | move, std::move. NO body-level `.give()` syntax. |
| Copy a value | `.copy()` (body operation, parens per dot-postfix rule) | clone, deep copy |
| Freeze to read-only | `.freeze()` (body operation, parens) | (no direct equivalent) |
| Error type / fallible | `errors` keyword | Result<T, E>, throws, exceptions |
| Type narrowing | `is` | typeof, instanceof, type guards |
| Async wait point | `wait` | await, async/await |
| Spawn task | `background` | async, go, spawn, thread |
| Block compiler safety | `verified { }` | unsafe { }, raw |

---

## Concept-Level Distinctions

### `shape` vs value

A `shape` is the DECLARATION of a data structure. A value is an instance of data with that structure.

```ynz
shape Player {                    // declaration — this is a shape (data only; no methods)
  name: string
  health: int
}

const p: Player = { name: "Patrick", health: 100 }   // creating a value (annotation-driven literal)
//    ^ "p" is "a Player value" or just "a Player"
```

When writing prose: "Players" or "a Player value" — never "a Player object" or "a Player instance" or "a Player struct."

Yinz is not object-oriented — see [`.claude/rules/non-oop.md`](non-oop.md). Methods are standalone functions, not bound to shape declarations. `value.method()` is parser-level sugar for `method(value)` (UFCS — Uniform Function Call Syntax).

### `shape` vs `map<K, V>`

`shape` and `map<K, V>` are different concepts:

| | `shape` | `map<K, V>` |
|---|---|---|
| Field names known? | At compile time | At runtime |
| Access pattern | Direct memory offset | Hash lookup |
| Example | `shape Player { name: string, health: int }` | `map<string, Player>` for a user directory |

JavaScript conflates these (an "object" is both `{ name: "x" }` (record) and `{ [key]: val }` (dictionary)). Yinz keeps them separate. **Never call a value a "map" unless it actually IS a `map<K, V>`.**

### `array<T>` vs `fixed<T>`

| | `array<T>` | `fixed<T>` |
|---|---|---|
| Storage | Heap (growable) | Stack (size locked at creation) |
| Default? | No | YES — prefer `fixed` when size is known |
| Use when | Size can grow at runtime | Size is known at compile time |

### `options` vs `union`

- `options Status { active, inactive, banned }` — replaces `enum`. A finite set of named constants.
- `union` is the concept; **`|` is the syntax**: `shape Result = Success | Failure`. Yinz keeps `|` from TypeScript (locked by Patrick 2026-05-14). The `or` keyword was considered but rejected because it's triple-overloaded (boolean operator + union syntax + prose word). See [`docs/reference/REF-golden-rules.md`](../../docs/reference/REF-golden-rules.md) Rule 12 expanded version for the full rationale.

### `maybe T`

`maybe T` is sugar for `T | none`. Use when "no value" is a normal possibility (a query that might not match; a parsed value that might fail).

For errors-that-the-caller-must-handle, use the `errors` keyword instead: `function readFile() -> string errors`.

---

## Banned Legacy Terms (Compile Error When Possible)

The Yinz compiler bans these legacy terms in user-facing diagnostics via `crates/ynz-diagnostics/src/banned_jargon.rs`. The replacement diagnostic uses three-part WHAT/WHAT-INSTEAD/WHY format.

| Banned word | Replacement (in user-facing text) |
|---|---|
| `type` (as declaration keyword) | `shape` |
| `struct` | `shape` |
| `class` | `shape` |
| `interface` | `shape` (or `follows` for contracts) |
| `enum` | `options` |
| `void` | `nothing` |
| `null` / `undefined` | `none` |
| `fn` | `function` |
| `infer` / `inference` | "figure out automatically", "the compiler can tell" |
| `Result` | `errors` keyword |
| `Option` / `Optional` | `maybe T` |

**M7 additions (2026-05-18):** `monad`, `lift`, `wrap` (bare `wrap` and `unwrap` — M5/M6 already banned `unwrap`); `Result`, `Option`, `Either`, `exception`, `try`, `catch`, `throw`, `UTF-16`. These must not appear in any user-facing diagnostic. They belong to the technical-programmer jargon world Yinz's users are NOT expected to know.

**Caveat for `type`**: the WORD "type" is fine in prose ("the type of x", "the type system", "type inference is a design concept"). The compile error only fires when `type` appears as a declaration keyword (`type Foo { ... }`) — that's what banned-jargon catches in compiler output.

---

## Correct vs Incorrect Prose Examples

❌ **Incorrect**:
> "Declare a `type` called Player. Player is a struct with two fields. Create a new instance of Player by calling its constructor."

✅ **Correct**:
> "Declare a `shape` called Player. Player has two fields (name and health). Create a Player value by writing `const p: Player = { name: ..., health: ... }` (annotation-driven literal — Yinz uses structural typing)."

❌ **Incorrect**:
> "The function returns Optional<User> or null."

✅ **Correct**:
> "The function returns `maybe User` — either a User or `none`."

❌ **Incorrect**:
> "Async functions must use the async/await keywords."

✅ **Correct**:
> "Functions that perform I/O can be marked with `wait` at the call site. The IDE shows the `wait` muted-text hint when the compiler infers it for you."

---

## Capital Letter Rule (Golden Rule 13)

Capital letter = type. Everything else = lowercase. This is universal:

- `Player`, `Warrior`, `Config`, `Request` — types (PascalCase)
- `player`, `score`, `health` — values (camelCase)
- `function`, `let`, `const`, `shape`, `options`, `wait` — keywords (lowercase)
- `file`, `request`, `date`, `math` — modules (lowercase)

When module and type share a base name, casing distinguishes them:
- `Date` = the type returned by `date.now()`
- `date` = the module
- `Duration` = the type
- `duration` = the module

---

## When You're Unsure

If a concept doesn't have an official term yet, **ask Patrick before inventing one.** Yinz vocabulary is curated, not crowdsourced. Adding terms without consultation creates the exact "every doc uses a different word" problem this file exists to prevent.

---

## Cross-References

- [`.claude/rules/naming.md`](naming.md) (capital-letter rule, module/type case distinctions)
- [`.claude/rules/inference.md`](inference.md) (dual-audience rule for `infer`/`inference` etc.)
- [`docs/reference/REF-compiler-errors.md`](../../docs/reference/REF-compiler-errors.md) (banned-jargon source-of-truth for user-facing diagnostics)
- `crates/ynz-diagnostics/src/banned_jargon.rs` (compile-time enforcement)
