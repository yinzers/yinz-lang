# Yinz Vocabulary — Official Terms

This is the authoritative reference for Yinz user-facing terminology. **All user-facing docs (`spec/`), design docs (`design/`), compiler diagnostics, and Claude-chat output use these terms.** Never use legacy terms from other languages.

For internal-vs-user-facing audience distinctions (e.g., `infer`/`inference` allowed in design docs but banned in compiler errors), see `.claude/rules/inference.md`.

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
| A-or-B type relationship | `union` (via `or`) | sum type, variant type |
| Optional/maybe value | `maybe T` | Optional, Option, nullable |
| Absent value | `none` | null, undefined, None, nil |
| No return value | `nothing` | void, unit, () |
| Function declaration | `function` | fn, func, def, method |
| The implementing type (in `follows`) | `Self` | self, this (capital S only for the type) |
| The instance (lowercase) | `self` | this, instance |
| Read-only borrow | `.share` | &T, shared ref, immutable borrow |
| Mutable borrow | `.lend` | &mut T, mutable ref |
| Ownership transfer | `.give` | move, std::move |
| Copy a value | `.copy` | clone, deep copy |
| Freeze to read-only | `.freeze` | (no direct equivalent) |
| Error type / fallible | `errors` keyword | Result<T, E>, throws, exceptions |
| Type narrowing | `is` | typeof, instanceof, type guards |
| Async wait point | `wait` | await, async/await |
| Spawn task | `background` | async, go, spawn, thread |
| Block compiler safety | `verified { }` | unsafe { }, raw |

---

## Concept-Level Distinctions

### `shape` vs value

A `shape` is the DECLARATION of a structure. A value is an INSTANCE with that structure.

```yinz
shape Player {                    // declaration — this is a shape
  name: string
  health: int
}

let p = Player { name: "Patrick", health: 100 }   // creating a value
//  ^ "p" is "a Player value" or just "a Player"
```

When writing prose: "Players" or "a Player value" — never "a Player object" or "a Player instance" or "a Player struct."

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
- `union` is the relationship: replaces variant types / sum types. **Open contradiction**: `.claude/rules/naming.md` says use `or` (`shape Result = Success or Failure`); CLAUDE.md Golden Rule 12 says use `|` because `or` was triple-overloaded. Phase 2 (Golden Rules update) must resolve this — pending Patrick's call.

### `maybe T`

`maybe T` is sugar for `T or none`. Use when "no value" is a normal possibility (a query that might not match; a parsed value that might fail).

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

**Caveat for `type`**: the WORD "type" is fine in prose ("the type of x", "the type system", "type inference is a design concept"). The compile error only fires when `type` appears as a declaration keyword (`type Foo { ... }`) — that's what banned-jargon catches in compiler output.

---

## Correct vs Incorrect Prose Examples

❌ **Incorrect**:
> "Declare a `type` called Player. Player is a struct with two fields. Create a new instance of Player by calling its constructor."

✅ **Correct**:
> "Declare a `shape` called Player. Player has two fields (name and health). Create a Player value by writing `Player { name: ..., health: ... }`."

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
- `file`, `http`, `date`, `math` — modules (lowercase)

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

- `.claude/rules/naming.md` (capital-letter rule, module/type case distinctions)
- `.claude/rules/inference.md` (dual-audience rule for `infer`/`inference` etc.)
- `design/compiler-errors.md` (banned-jargon source-of-truth for user-facing diagnostics)
- `crates/ynz-diagnostics/src/banned_jargon.rs` (compile-time enforcement)
