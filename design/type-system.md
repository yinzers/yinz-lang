# Type System — Design Decisions

User spec: `spec/types.md`, `spec/maybe.md`, `spec/unions.md`, `spec/options.md`, `spec/generics.md`

---

## One Keyword: `shape`

`shape` is the only declaration keyword. No `interface`, `struct`, `class`, `type`.

**Why**: Three keywords for three overlapping concepts confuses junior developers. `shape` handles all cases: plain data shapes, shapes used as contracts (with bare-signature declarations only — no bodies), shapes that extend other shapes for data reuse. One concept, one word. The word `type` is banned at the lexer level (declaration-keyword diagnostic redirects to `shape`) because "type" is also the generic English word for a category — keeping it as a keyword would constantly conflict with that broader usage. Locked in design-lockdown (2026-05-14).

> Yinz is **NOT object-oriented** — see `.claude/rules/non-oop.md`. Shapes hold DATA + (optionally) CONTRACT METHOD-SIGNATURE DECLARATIONS only. Method implementations live as standalone `function` declarations at file/module level. `value.method()` is parser-level sugar for `method(value)` (UFCS — Uniform Function Call Syntax). The sections below describe the type system within this non-OOP model.

---

## Single Inheritance with `extends` (data-only)

Single inheritance only. `extends` reuses parent's DATA FIELDS — behavior comes from standalone functions, not from inherited methods (because there are no methods on shapes to inherit). No multiple inheritance.

**Why**: Multiple inheritance creates the diamond problem (ambiguous resolution with surprising behavior). Single inheritance is simpler to reason about and almost always sufficient. `extends` is for code reuse at the DATA level; for behavior polymorphism, write a standalone function for each shape and let the compiler pick by argument-type overloading. For shared behavior contracts, `follows` handles any number of contracts. Locked r10 (2026-05-16).

**Example**:
```ynz
shape Entity { name: string, health: int }
shape Warrior extends Entity { weapon: string, armor: int }

function greet(share self: Entity) -> string {
  return "Hello, I am " + self.name
}

function greet(share self: Warrior) -> string {
  return "Hello, I am " + self.name + " the warrior, wielding " + self.weapon
}

const w: Warrior = { name: "Aragorn", health: 100, weapon: "sword", armor: 50 }
w.greet()    // calls greet(Warrior) — more specific overload wins
```

---

## `base` for Non-Instantiable Shapes

`base shape Entity` instead of `abstract class Entity`.

**Why**: "Base shape" reads like English — "this is a base you build on." `abstract` requires knowing what abstraction means in object-oriented design. Yinz isn't OOP and avoids the term. Golden Rule 12.

`base` shapes cannot be instantiated via struct literal — attempting `let e: Entity = { ... }` on a `base shape Entity` is a compile error. They exist purely to be extended.

---

## `follows` for Contracts

`follows` instead of `implements`. Optional in structurally-typed code but recommended for catching contract mismatches at definition time.

**Why**: "Player follows Damageable" reads like a sentence. `implements` is CS jargon. Structural typing means `follows` isn't required for compatibility — but writing it catches missing-method errors at the shape-declaration site rather than at the first call site, and makes the relationship visible to readers.

**Contract verification**: when a shape declares `follows Foo`, the compiler verifies that standalone functions exist whose signatures match each of Foo's bare-signature method declarations. No method implementations live inside the contract OR the implementing shape — both sides declare signatures; standalone functions provide the behavior.

**Multiple allowed**: `shape Warrior extends Entity follows Damageable, Attackable` — single `extends`, any number of `follows`.

---

## Function Overloading by Argument Type (no `override` keyword)

There is no `override` keyword in Yinz. Method polymorphism is provided by **function overloading by argument type** — write multiple standalone functions with the same name but different first-parameter types; the compiler picks the most specific match at every call site.

**Why no `override`**: methods don't live inside shapes (`.claude/rules/non-oop.md`), so there's nothing to "override" — there's no parent-method-in-shape to redeclare. The OOP `override` keyword exists to disambiguate "I'm intentionally replacing the parent's method" from "I accidentally shadowed it" — Yinz doesn't have that ambiguity because methods are always standalone functions, and the compiler's overload-resolution rules are deterministic (most-specific-first-parameter-type wins).

**Example**:
```ynz
shape Entity { name: string }
shape Warrior extends Entity { weapon: string }

function attack(share self: Entity) -> string { return self.name + " attacks!" }
function attack(share self: Warrior) -> string { return self.name + " swings " + self.weapon + "!" }

const e: Entity = { name: "Orc" }
const w: Warrior = { name: "Aragorn", weapon: "Andúril" }

e.attack()    // calls attack(Entity) — "Orc attacks!"
w.attack()    // calls attack(Warrior) — "Aragorn swings Andúril!" (more specific match)
```

**Diagnostic for shadow-without-overload**: if you write a function with the same name as one in a parent's overload set but with a less-specific signature, the compiler accepts it (it's still callable for the parent type's values); shadowing-detection is not needed because OOP's "did I mean to override?" problem doesn't apply when there's no method-on-instance binding.

Locked r10 (2026-05-16). Replaces the previously-locked `override` keyword which is now removed entirely.

---

## Structural Typing

Shape matching like TypeScript. If the fields match the type, the value is valid — no explicit type constructor required.

**Why**: Reduces ceremony. Object literals that match a type's shape are accepted without explicit type-name syntax. `return { quotient: a / b, remainder: a % b }` works when the return type is `DivResult` — no `return DivResult { ... }` needed.

---

## Static Dispatch Default for `follows` Constraints

When a function is generic over a `follows` constraint AND the concrete type at the call site is known to the compiler, the compiler **generates a specialized version per concrete type** and pastes the called methods directly into the loop (no function-call overhead, no runtime lookup). This is "static dispatch" — the dispatch decision is made at compile time. Dynamic dispatch (runtime lookup, no inlining) is the explicit opt-in for the case where the concrete type cannot be known until runtime.

Locked: static dispatch is the default. Dynamic dispatch requires explicit syntax.

> **Internal terminology note**: this technique is called *monomorphization* in compiler literature ("one shape per concrete type"). That word is BANNED from user-facing diagnostics, IDE hints, and spec docs per Golden Rule 12 — even the design doc you're reading should prefer "specialized version per type" so a jr dev contributing to Yinz isn't blocked by jargon. The internal term is documented here once and referenced in contributor-only compiler-internals docs (`crates/ynz-codegen/`); user-facing surfaces use the plain phrase.

### Concrete example

```ynz
// Define a contract — bare-signature declarations only (no `function` keyword, no body)
shape Comparable {
  compare(share self, share other: Self) -> int
}

// Two shapes that follow the contract — data fields only
shape Player follows Comparable {
  name: string
  health: int
}

shape Item follows Comparable {
  name: string
  weight: int
}

// Standalone functions provide the implementations.
// Compiler verifies these match Comparable's signature when checking `follows`.
function compare(share self: Player, share other: Player) -> int {
  return self.health - other.health
}

function compare(share self: Item, share other: Item) -> int {
  return self.weight - other.weight
}

// A generic function — works on anything that follows Comparable
function findMax<T follows Comparable>(share items: array<T>) -> maybe T {
  // ... walk the array, keep the largest per compare()
}

// CASE A — concrete type known → STATIC DISPATCH (auto-picked, fast)
let players: array<Player> = [...]
let best = findMax(players)
// Compiler generates a specialized findMax just for Player; .compare() is pasted inline.
// Cost: ~1 CPU instruction per .compare()
// IDE muted hint: // static dispatch (T = Player)

// CASE B — heterogeneous collection → DYNAMIC DISPATCH (user opt-in via `dynamic`)
let mixedThings: array<dynamic Comparable> = [player1, item1, player2]
let maxThing = findMax(mixedThings)
// Compiler can't generate a specialized version — array holds different concrete types.
// Each .compare() = runtime lookup ("which compare() do I call for THIS element?") + indirect function call.
// Cost: ~3 CPU instructions + likely cache miss per .compare()
```

### Why this matters

Go ships with always-dynamic interface dispatch — even when only one concrete type ever satisfies an interface at a given call site, Go can't skip the runtime lookup without profile-guided optimization. Polar Signals' benchmark shows ~3× overhead in tight loops: 958 µs (interface dispatch) vs 320 µs (concrete type dispatch) per 1024 iterations (https://www.polarsignals.com/blog/posts/2023/11/24/go-interface-devirtualization-and-pgo). Yinz inherits Rust's correct model — generate a specialized version when the type is known, runtime lookup only when the user explicitly opts in via `dynamic`.

### When you actually need `dynamic`

Rare in practice. Real cases:
- **Heterogeneous collections** — UI tree where leaves are buttons, sliders, text boxes (all `Drawable`), stored in `array<dynamic Drawable>` for rendering
- **Plugin architectures** — types loaded from packages at runtime
- **Stored callback collections** — arrays of different function-shaped values

For 95% of code, the user writes generic functions and the compiler picks static dispatch automatically. `dynamic` is only needed when the heterogeneity is the actual feature.

### Teaching surfaces — both directions get IDE hints

Per Golden Rule 11 (compiler is teacher) and `.claude/rules/auto-promotion.md`, both dispatch cases get visible teaching at the call site, plus a lint when the user opted into dynamic unnecessarily.

#### Static dispatch — neutral muted hint

```ynz
let best = findMax(players)        // muted: // static dispatch (T = Player) — .compare() inlined, ~1 cycle
```

Hover tooltip:
- **WHAT**: A specialized version of `findMax` was generated for `Player`. The `.compare()` call is pasted directly into the loop body — no function-call jump.
- **WHAT INSTEAD**: To explicitly force dynamic dispatch (rare — only useful for benchmarking), wrap the input as `array<dynamic Comparable>`.
- **WHY**: The compiler knew the concrete type at this call site. Static dispatch is ~3× faster than runtime lookup in tight loops.

#### Dynamic dispatch — cautionary muted hint (red-tinted per `.claude/rules/inference.md`)

```ynz
let maxThing = findMax(mixedThings)   // muted (red-tinted): // dynamic dispatch — runtime lookup per .compare(), ~3× cost
```

Hover tooltip:
- **WHAT**: This call uses runtime lookup because `mixedThings` is `array<dynamic Comparable>` — the concrete type at each iteration isn't known at compile time.
- **WHAT INSTEAD**: If all elements are actually the same concrete type, switch to `array<Player>` for static dispatch (~3× faster).
- **WHY**: You wrote `dynamic Comparable` to allow heterogeneous storage. The cost is one runtime lookup + indirect call per `.compare()`.

#### Tier 3 lint when dynamic could have been static

```ynz
let things: array<dynamic Comparable> = []   // yellow squiggle from `prefer-static-dispatch-when-monotype`
things.add(player1)
things.add(player2)
// All elements are Player. The dynamic wrapper is unnecessary.
```

The lint fires when the compiler can prove the `dynamic` collection only ever holds one concrete type. Suggested fix: change to `array<Player>` for the perf win.

#### Compile errors when contract isn't followed (Rule 11 format)

```
COMPILE ERROR: Item is not a Player.
  Player and Item are different types — array<Player> only holds Players.

  To store a mix of Comparable shapes, declare:
    let things: array<dynamic Comparable> = []

  This adds runtime dispatch cost (~3× per .compare() call) but allows mixed types.
```

```
COMPILE ERROR: Foo does not follow Comparable.
  findMax requires elements to follow the Comparable contract.

  To make Foo work with findMax:

    1. Declare Foo follows Comparable:
       shape Foo follows Comparable {
         name: string
       }

    2. Provide a standalone compare function matching Comparable's signature:
       function compare(share self: Foo, share other: Foo) -> int {
         // ... return negative, zero, or positive
       }

  Yinz is not object-oriented — contract implementations are standalone
  functions, not methods inside the shape. See spec/operators.md.
```

All four diagnostics (two muted hints + one lint + two compile errors) follow WHAT/WHAT-INSTEAD/WHY. None use jargon (`monomorphization`, `vtable`, `devirtualization` are internal compiler terms — user-facing diagnostics say "specialized version per type" and "runtime lookup").

### Why this matters

Go shipped with interface dispatch always being dynamic. Polar Signals' production benchmark (https://www.polarsignals.com/blog/posts/2023/11/24/go-interface-devirtualization-and-pgo) shows ~3× overhead in tight loops: ~958 µs (interface dispatch) vs ~320 µs (concrete type dispatch). Even when only one concrete type ever satisfies the interface at a given call site, Go can't skip the runtime lookup without profile-guided optimization. Yinz inherits Rust's correct model — `follows` is the contract, generating a specialized version per concrete type is the codegen — so this overhead doesn't exist by default.

### Auto-promotion (per `.claude/rules/auto-promotion.md`)

This pattern qualifies as auto-promotion (codegen surface only):
- **Codegen**: when the compiler proves the concrete type at a call site, it emits the static-dispatch version. Always applies when proof is possible.
- **Muted IDE hint**: optional — the IDE can show `// static dispatch (T = Player)` on the call to confirm what was chosen.
- **Tier 3 lint suggestion**: not applicable — there's no source-level rewrite that improves things; the user wrote idiomatic generic code and the compiler handled it.

### Dynamic dispatch — when and how

Dynamic dispatch is needed when the concrete type genuinely isn't known until runtime — e.g., a heterogeneous collection of values that all satisfy a contract but are different types. Surface syntax for the opt-in is TBD at M4 implementation. Candidate: `dynamic Drawable` (matches Yinz's plain-English vocabulary; explicitly NOT `dyn Drawable` — `dyn` is Rust jargon and violates Golden Rule 12).

### Tradeoff

Generating a specialized version per concrete type produces more machine code than a single runtime-dispatched implementation. For very large programs with many concrete types satisfying the same contract, binary size grows proportionally. Mitigated by:
1. Monomorphization deduplication (per `design/compiler.md` Generics & Monomorphization section) — identical instantiations share LLVM IR
2. Opt-in dynamic dispatch (`dynamic ...` in stored-value contexts) recovers code size when that matters

The default favors perf; the opt-in covers binary-size-constrained cases.

---

## Generics — `name<T>` Syntax

`shape Box<T> { value: T }` — angle bracket generics, same pattern as built-in collections.

**Why**: Consistent with `array<Player>`, `map<string, number>`, `fixed<string>`. One `name<type>` pattern covers both built-in and user-defined generic types. No special cases.

---

## Union Types with `|`

`shape DrawableShape = Circle | Square | Triangle` with `is` for type checking and narrowing.

**Why `|` over `or`**: Consistency — all operators are symbols in Yinz. JS/TS developers already know `|` for union types. `or` was triple-overloaded (union types, boolean OR, `.or()` method) — switching union types to `|` eliminates the overload entirely. `is` matches plain English — "if shape is Circle."

**`is` in union context = exact type**: `Admin` does NOT match `is User` in a union even if `Admin extends User`. Outside unions, normal subtype rules apply. This makes union discrimination predictable — each variant is always distinct.

---

## No Null — `maybe` Types

No `null`. No `undefined`. Absence is expressed as `none` and tracked by the type system with `maybe T`.

**Why**: Null references are the "billion dollar mistake" — entire categories of runtime errors exist only because null can masquerade as any type. Making absence explicit in the type system moves null errors to compile time (Golden Rule 5).

**`maybe T` = `T | none`**: Interchangeable syntax. `maybe string` is sugar for `string | none`. Both valid everywhere.

---

## Hidden Fields — `hidden` Keyword

`hidden` fields are invisible to code OUTSIDE THE SAME FILE as the shape declaration. They require a default value:

```ynz
shape Player {
  name: string
  hidden damageMultiplier: number = 1.0
  hidden internalCache: map<string, number> = {}
}
```

**Why `hidden` over `private`**: Golden Rule 12. "Hidden field" reads like English. "Private" requires knowing OOP access modifier terminology. A developer who has never seen Yinz understands what `hidden damageMultiplier` means.

**Why require defaults**: Without a default, the caller would need to provide the hidden field during construction — which would require knowing the field name, defeating the purpose of hiding it. Requiring defaults makes the initial state explicit and visible in the type definition (Golden Rule 2 — self-documenting). The caller only provides visible fields.

**Visibility scope (non-OOP framing)**: Yinz isn't object-oriented — fields don't belong to instances; functions don't live inside shapes. `hidden` means "this field is only accessible to standalone functions declared in the same file as the shape." A `hidden` field can be read and written by functions in the file; it's invisible to imports and to functions in other files. Default values cover construction (the only way an outside caller could touch a hidden field would be at construction — defaults make that path go through the shape's file).

**Hidden vs ownership modifiers**: these are different concepts. `share`/`lend`/`give` are SIGNATURE-level ownership declarations (or compiler-inferred at call sites). `hidden` is a FIELD-level visibility modifier. They don't interact.

---

## `options` Keyword

`options Status { active, inactive, banned }` instead of `enum`.

**Why**: "Options" is a plain English word non-programmers understand. A non-programmer reading `options Status` knows what it means. They don't know what an enum is. Golden Rule 12.

**Built-in options**: `SortOrder { asc, desc }`, `Comparison { equal, greater, less }`.

**Shorthand**: Context-aware shorthand allowed when the expected type is known at the call site.
