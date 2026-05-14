# Type System — Design Decisions

User spec: `spec/types.md`, `spec/maybe.md`, `spec/unions.md`, `spec/options.md`, `spec/generics.md`

---

## One Keyword: `type`

`type` is the only keyword for defining shapes. No `interface`, `struct`, `class`.

**Why**: Three keywords for three overlapping concepts confuses junior developers. `type` handles all cases: plain data shapes, shapes with methods, shapes used as contracts, shapes that extend other shapes. One concept, one word.

---

## Single Inheritance with `extends`

Single inheritance only. No multiple inheritance.

**Why**: Multiple inheritance creates the diamond problem — ambiguous method resolution with surprising behavior. Single inheritance is simpler to reason about and almost always sufficient. For sharing behavior across unrelated types, `follows` handles multiple contracts (no limit on how many a type can follow).

---

## `base` for Non-Instantiable Shapes

`base shape Entity` instead of `abstract class Entity`.

**Why**: "Base shape" reads like English — "this is a base you build on." `abstract` requires knowing what abstraction means in OOP. Golden Rule 12.

---

## `follows` for Contracts

`follows` instead of `implements`. Optional in structurally-typed code but recommended.

**Why**: "Player follows Damageable" reads like a sentence. `implements` is CS jargon. Structural typing means `follows` isn't required for compatibility — but it catches contract mismatches at definition time rather than usage site, and makes intent explicit.

**Multiple allowed**: `type Warrior extends Entity follows Damageable, Attackable` — single `extends`, any number of `follows`.

---

## `override` Keyword Required

Overriding a parent method requires the `override` keyword. Compiler errors in both directions — missing `override` when parent has the method, and using `override` when parent doesn't.

**Why**: Accidental method shadowing is a silent bug. The two-direction check prevents both "I didn't mean to override this" and "I typed the method name wrong and thought I was overriding."

---

## Structural Typing

Shape matching like TypeScript. If the fields match the type, the value is valid — no explicit type constructor required.

**Why**: Reduces ceremony. Object literals that match a type's shape are accepted without explicit type-name syntax. `return { quotient: a / b, remainder: a % b }` works when the return type is `DivResult` — no `return DivResult { ... }` needed.

---

## Generics — `name<T>` Syntax

`type Box<T> { value: T }` — angle bracket generics, same pattern as built-in collections.

**Why**: Consistent with `array<Player>`, `map<string, number>`, `fixed<string>`. One `name<type>` pattern covers both built-in and user-defined generic types. No special cases.

---

## Union Types with `|`

`type Shape = Circle | Square | Triangle` with `is` for type checking and narrowing.

**Why `|` over `or`**: Consistency — all operators are symbols in Yinz. JS/TS developers already know `|` for union types. `or` was triple-overloaded (union types, boolean OR, `.or()` method) — switching union types to `|` eliminates the overload entirely. `is` matches plain English — "if shape is Circle."

**`is` in union context = exact type**: `Admin` does NOT match `is User` in a union even if `Admin extends User`. Outside unions, normal subtype rules apply. This makes union discrimination predictable — each variant is always distinct.

---

## No Null — `maybe` Types

No `null`. No `undefined`. Absence is expressed as `none` and tracked by the type system with `maybe T`.

**Why**: Null references are the "billion dollar mistake" — entire categories of runtime errors exist only because null can masquerade as any type. Making absence explicit in the type system moves null errors to compile time (Golden Rule 5).

**`maybe T` = `T | none`**: Interchangeable syntax. `maybe string` is sugar for `string | none`. Both valid everywhere.

---

## Hidden Fields — `hidden` Keyword

`hidden` fields are completely invisible outside the type's own methods. They require a default value:

```
shape Player {
  name: string
  hidden damageMultiplier: number = 1.0
  hidden internalCache: map<string, number> = {}
}
```

**Why `hidden` over `private`**: Golden Rule 12. "Hidden field" reads like English. "Private" requires knowing OOP access modifier terminology. A developer who has never seen Yinz understands what `hidden damageMultiplier` means.

**Why require defaults**: Without a default, the caller would need to provide the hidden field during construction — which would require knowing the field name, defeating the purpose of hiding it. Requiring defaults makes the initial state explicit and visible in the type definition (Golden Rule 2 — self-documenting). The caller only provides visible fields.

**Hidden vs `share`/`lend`**: These are different concepts. `share`/`lend` control mutability in a given context. `hidden` controls visibility — hidden fields simply do not exist to code outside the type. A hidden field can be both read and written by the type's methods.

---

## Type Aliases

`type Name = ExistingType` creates a documentation alias. Zero runtime cost — erased at compile time. The alias and original type are fully interchangeable.

**Why**: Self-documenting function signatures. `fetchUser(id: UserId)` tells a clearer story than `fetchUser(id: string)`. The compiler enforces nothing extra — `UserId` IS `string` — but the name communicates intent to human readers.

**Not nominal typing**: Aliases don't create new types. A function expecting `UserId` accepts a `string`. This is consistent with structural typing throughout the language.

---

## `options` Keyword

`options Status { active, inactive, banned }` instead of `enum`.

**Why**: "Options" is a plain English word non-programmers understand. A non-programmer reading `options Status` knows what it means. They don't know what an enum is. Golden Rule 12.

**Built-in options**: `SortOrder { asc, desc }`, `Comparison { equal, greater, less }`.

**Shorthand**: Context-aware shorthand allowed when the expected type is known at the call site.
