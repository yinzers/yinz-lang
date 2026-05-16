# Non-OOP Model — Data Shapes + Standalone Functions + UFCS

> **Yinz is not object-oriented. Shapes hold data; methods are standalone functions; `value.method()` is parser-level sugar for `method(value)` (UFCS — Uniform Function Call Syntax). Both call forms are legal and equivalent.**

Loaded when designing or reviewing any feature touching shapes, methods, dispatch, inheritance, contracts, or polymorphism. Drift back into OOP patterns is the most common modeling mistake — check against this rule BEFORE writing the design.

---

## Why Yinz is not OOP

Yinz's positioning is "Rust-level performance, TypeScript-level readability, no garbage collector, approachable by junior developers."

- **Rust-level performance**: Rust is not OOP. Methods on types are sugar for standalone functions with receiver-typed first parameters. Yinz follows the same model — zero per-instance method storage; vtable lookup only when the user explicitly opts into `dynamic Foo`.
- **No garbage collector**: ownership tracking is simpler when methods aren't bound to instances. Each function call has clear ownership semantics declared at its signature.
- **Approachable by junior developers**: OOP introduces multiple parallel concepts (classes, instances, inheritance, polymorphism, virtual dispatch, abstract classes). Non-OOP collapses these into two: data shapes + functions.

The TypeScript-readability part still holds — modern TS often uses standalone functions over types (`function getName(user: User)` is more common than `class User { getName() }`). Yinz makes this the only pattern.

---

## What Goes Where

### Shapes hold data + (optionally) contract method-signatures ONLY

```yinz
shape Player {
  name: string
  health: int
}

// A contract — shape with method-signature declarations and no fields
shape Comparable {
  // Bare-signature form — NO `function` keyword, no body
  compare(share self, share other: Self) -> int
}

shape Warrior follows Comparable {
  name: string
  health: int
  // Warrior must provide a standalone `compare(Warrior, Warrior) -> int` function
}
```

### Functions live at file/module level

```yinz
function greet(share self: Player) -> string {
  return "Hello, " + self.name
}

function heal(lend self: Player, amount: int) -> nothing {
  self.health = self.health + amount
}

function compare(share a: Warrior, share b: Warrior) -> int {
  return a.health - b.health
}
```

### Methods are NOT inside shapes

```yinz
// ❌ COMPILE ERROR
shape Player {
  name: string
  function greet(share self) -> string {     // method inside shape body
    return "Hello, " + self.name
  }
}

// COMPILE ERROR: methods cannot be declared inside a shape body.
//   In Yinz, shapes hold data + contract signatures only.
//
//   Move greet to a standalone function:
//     shape Player { name: string, health: int }
//     function greet(share self: Player) -> string { return "Hello, " + self.name }
//
//   Why: Yinz is not object-oriented. Methods are standalone functions.
//   Call them with either dot-call syntax (player.greet()) or function-call
//   syntax (greet(player)) — both work via UFCS.
```

---

## Call Syntax — UFCS (both forms equivalent)

`value.method(args)` is sugar for `method(value, args)`. The compiler rewrites the dot form at parse time; both compile to the same machine code. Users pick per call site:

```yinz
const player: Player = { name: "Patrick", health: 100 }

player.heal(20)        // dot-call — natural when there's a clear receiver
heal(player, 20)       // function-call — also legal, same result

player.greet()         // dot-call
greet(player)          // function-call

// Utility functions with no obvious "receiver" — function-call usually reads better
max(50, player.health)
//   vs.
player.health.max(50)  // legal but reads awkwardly — neither value is "the receiver"
```

Style is a per-codebase choice; the language accepts both. Tier 3 lints in v0.4+ can enforce a team convention.

---

## How the IDE/Compiler Know What's Valid

The IDE and compiler share the same function-signature lookup. The rule:

- **Compiler**: at every call site, look up functions by name; type-check argument types against signatures
- **IDE**: when user types `value.`, scan functions in scope; filter to those whose FIRST parameter type matches `value`'s type; show matches as autocomplete

End-to-end example with three shapes:

```yinz
shape Player   { name: string, health: int }
shape Enemy    { name: string, health: int }
shape Building { address: string, hp: int }

function attack(lend attacker: Player, lend victim: Enemy, amount: int) -> nothing {
  victim.health = victim.health - amount
}

function fortify(lend self: Building, amount: int) -> nothing {
  self.hp = self.hp + amount
}

const player: Player     = { name: "Patrick", health: 100 }
const goblin: Enemy      = { name: "Goblin",  health: 50 }
const tower:  Building   = { address: "1 Main St", hp: 500 }
```

**Autocomplete behavior**:

- Type `player.` → IDE shows: `attack`, `heal`, `greet`, `compare`, `name`, `health` (functions whose first param is Player + Player's fields)
- Type `tower.`  → IDE shows: `fortify`, `address`, `hp` (functions whose first param is Building + Building's fields)
- `attack` is NOT shown in tower's autocomplete because `attack`'s first param is Player, not Building

**Call attempts**:

```yinz
player.attack(goblin, 10)        // ✅ compiles — attack(Player, Enemy, int) matches
attack(player, goblin, 10)        // ✅ compiles — same call, function-form
tower.attack(goblin, 10)          // ❌ compile error (see diagnostic below)
```

---

## Diagnostic Format — Dual-Style Teaching

When a UFCS-related call fails, the compiler diagnostic shows BOTH call styles in the WHAT-INSTEAD section. The first error a new user hits teaches both forms in one shot.

```
COMPILE ERROR: No function `attack` accepts (Building, Enemy, int).

  Available `attack` functions:
    attack(lend Player, lend Enemy, int) -> nothing
           ^^^^^^^^^^^ expected Player here; you passed a Building

  Functions you CAN call on a Building value:
    tower.fortify(50)               ← dot-call style
    fortify(tower, 50)              ← function-call style
    (both forms work — pick whichever reads better)

  Why both styles: Yinz lets you write either `value.fn()` or `fn(value)`
  for any function whose first parameter type matches the value. Pick the
  form that reads naturally for the situation.
```

**Compiler "did-you-mean" suggester searches both directions**:
1. Same name, mismatched signature → explain WHY each candidate fails
2. Any name, first-param-type matches receiver → show "things you CAN do with this value"

**IDE renders the same suggestion via hover/tooltip** on the underlined call site.

---

## How Inheritance Works (Data-Only)

`extends` is for data reuse only. Child shape inherits parent's fields. Behavior comes from standalone functions; the compiler picks the most specific overload at the call site.

```yinz
shape Entity {
  name: string
  health: int
}

shape Warrior extends Entity {
  weapon: string
  armor: int
}

function greet(share self: Entity) -> string {
  return "Hello, I am " + self.name
}

function greet(share self: Warrior) -> string {
  return "Hello, I am " + self.name + " the warrior, wielding " + self.weapon
}

const warrior: Warrior = { name: "Aragorn", health: 100, weapon: "sword", armor: 50 }
warrior.greet()    // calls greet(Warrior) — the more specific match
```

No `override` keyword exists. No virtual dispatch table. The compiler picks `greet(Warrior)` because Warrior is more specific than Entity.

---

## How Contracts Work (`follows`)

A shape `follows Contract` when standalone functions exist whose signatures match the contract's bare-signature declarations.

```yinz
shape Damageable {
  takeDamage(lend self, amount: int) -> nothing
}

shape Player follows Damageable {
  name: string
  health: int
}

function takeDamage(lend self: Player, amount: int) -> nothing {
  self.health = self.health - amount
}

// Player satisfies Damageable because takeDamage(lend Player, int) -> nothing exists.
// If the function were missing, `shape Player follows Damageable` would fail to compile.

// Static dispatch — concrete type known
player.takeDamage(10)

// Dynamic dispatch — heterogeneous collection
const targets: array<dynamic Damageable> = [...]
for (target in targets) {
  target.takeDamage(10)        // vtable lookup; ~3× cost of static call
}
```

The compiler emits one shared `takeDamage` function for Player. For `dynamic Damageable`, a per-(concrete-shape, contract) function-pointer table is emitted as a compile-time global.

---

## When You'd Reach for OOP — What to Use Instead

| OOP pattern | Yinz pattern |
|---|---|
| Class with methods | Shape (data) + standalone functions (behavior) |
| Constructor / factory method | Standalone function returning the shape: `function newPlayer(name: string) -> Player { ... }` |
| `class Child extends Parent { override greet() }` | `shape Child extends Parent { ...new fields... }` + standalone `function greet(share self: Child)` that overloads the parent's |
| Abstract class | `base shape` — data + contract signatures only; cannot be instantiated; must be extended |
| Interface / protocol | `shape Contract { ...bare-signature declarations... }` + `follows` on implementing shapes |
| Virtual method / runtime polymorphism | `dynamic Contract` — fat pointer + per-(shape, contract) function table |
| Private method | Don't export the function from the module (M8 feature) |
| Private field | `hidden field: T = default` — visible only to functions in the same file as the shape |
| Method chaining (`builder.set(x).set(y).build()`) | Step-by-step with named variables (Golden Rule 7) |

---

## Banned Anti-Patterns

The Bouncer + compile diagnostics catch these:

1. **Method declared inside shape body** — `shape X { function foo() {...} }` is a compile error
2. **`override` keyword used anywhere** — token doesn't exist; lexer emits banned-keyword diagnostic
3. **`class`, `struct`, `interface`, `enum`, `abstract` keywords** — already banned by lexer (declaration-keyword teaching diagnostic)
4. **Spec/design docs framing Yinz as "object-oriented"** — Bouncer warning; replace with non-OOP framing
5. **Per-instance function-typed fields used as method storage** — design-review warning; refactor to standalone function + UFCS
6. **`new`, `instanceof`, `this` keywords** — none of these exist in Yinz; reaching for them signals OOP drift

---

## Cross-References

- `design/golden-rules.md` Rule 1 (dot-first, satisfied by UFCS), Rule 4 (compiler does hard work), Rule 8 (zero-cost abstractions — UFCS is parse-time sugar), Rule 12 (human-readable over jargon)
- `.claude/rules/dot-postfix.md` (parens-for-actions / no-parens-for-access — applies to method-call syntax)
- `.claude/rules/inference.md` (call-site ownership inference for implicit `.share()` / `.lend()` / `.give()` at every call site)
- `.claude/rules/vocabulary.md` (Yinz user-facing terms — non-OOP framing in error messages)
- `design/type-system.md` (shapes, extends, follows, dynamic — the full type system spec)
- `design/ownership.md` (ownership modifiers — declared at signatures, inferred at call sites)
- `.claude/plans/active/m4-shapes-methods-ownership.md` (M4 — where the non-OOP model lands in the compiler)
