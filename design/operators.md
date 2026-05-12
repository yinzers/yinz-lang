# Operators — Design Decisions

User spec: `spec/operators.md`

---

## Operator Overloading via `follows` Contracts

Operators are syntactic sugar for methods on standard library contracts. No magic method names, no special dispatch rules — the same `follows` system used everywhere else.

**Why not magic methods**: Languages like Python use `__add__`, `__eq__`, `__str__`. These are invisible to autocomplete, require knowing the naming convention, and feel like magic. The `follows` approach is fully transparent — type `.add` and see the method. The operator is sugar for a regular method.

**Why stdlib-defined contracts vs built-in operator keywords**: User types follow stdlib contracts (`Addable`, `Equatable`). This means the operator system is extensible without compiler changes — new operator contracts can be added to the stdlib like any other type. The compiler maps `+` → `add()` at the syntax level; everything else is regular type system.

---

## `Self` Keyword

`Self` (capital S) is a reserved type keyword meaning "the type that follows this contract."

**Why capital S**: Follows the naming convention — capital letter = type. `self` (lowercase) = the instance. `Self` (uppercase) = the type of the instance. Consistent, no new rule needed.

**How it resolves**: When `Vector2D follows Addable`, the compiler substitutes `Vector2D` for `Self` in all method signatures. `function add(share self, share other: Self) -> Self` becomes `function add(share self, share other: Vector2D) -> Vector2D`.

---

## `print()` Always Works — No `Printable` Required

Every type is printable. `print()` never errors.

**Default representation**: Type name + visible fields. Hidden fields excluded.

**Why a default**: Requiring `follows Printable` for `print()` to work would make debugging painful — you'd need to implement `toString()` before you could inspect any custom type. Default representation gives you something useful immediately. `Printable` is for when you want to customize the format, not for when you want `print()` to work at all.

**Hidden fields excluded from default**: Hidden fields are invisible to outside code. Printing them in the default representation would expose implementation details that the type author intentionally hid. Custom `toString()` can include hidden fields if the author chooses.

---

## Boolean Operators: `&&`, `||`, `!`

Symbol-based boolean operators — same as JavaScript, TypeScript, C, Java, Go, Rust.

**Why English words**: Golden Rule 12. `&&` and `||` are C-style operators that require knowing the convention. `and`, `or`, `not` are immediately readable by anyone who speaks English.

**`or` disambiguation by context**: `or` serves three roles:
1. Union type: `type Shape = Circle | Square` (type position)
2. Boolean OR: `if (admin or mod)` (expression position)
3. Method: `name.or("default")` (method call)

The parser distinguishes these by position. This is the same approach Python takes — `or` is both a boolean operator and used in expressions with no ambiguity because the parser knows where it is.

---

## Bitwise Operators — Symbols

Bitwise operators use symbols (`&`, `|`, `^`, `~`, `<<`, `>>`) while boolean operators use double-symbols (`&&`, `||`, `!`). This creates a clean, zero-overlap split.

**Why symbols for bitwise**: Bitwise symbols are universal across C, C++, Java, JavaScript, Rust, Go — every language that supports bitwise uses these symbols. Developers who need bitwise operations (systems programmers, game developers) already know them. Renaming to English words would create unnecessary friction for experienced developers without helping junior developers (who won't use bitwise at all).

**Why words for boolean**: Boolean logic is used constantly by every developer. Making it English (`and`, `or`, `not`) makes conditions read like sentences. Bitwise is niche; boolean is universal.

**The split is intuitive**: `if (active and verified)` is a condition check. `FLAG_A | FLAG_B` is bit manipulation. Any developer can tell these apart in context.

---

## No `===` Triple Equals

`==` is always type-safe. Comparing incompatible types is a compile error.

**Why**: `===` exists in JavaScript to compensate for `==` doing implicit type coercion (`"5" == 5` is `true`). Yinz has no implicit coercion. The type system prevents comparing a `string` to a `number` at compile time. `===` solves a problem that doesn't exist in a statically-typed language.
