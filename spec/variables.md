# Variables

Variables store values. Declare them with `let` or `const`.

---

## let — mutable variables

```
let name: string = "Patrick"
let health: number = 100
let score: number = 0
```

A `let` variable can be changed after it's created:

```
let health: number = 100
health = health - 25    // fine — health is now 75
```

---

## const — immutable variables

```
const maxPlayers: number = 10
const serverName: string = "prod-01"
```

A `const` cannot be changed after it's created:

```
const maxPlayers: number = 10
maxPlayers = 20
// COMPILE ERROR: maxPlayers is const and cannot be reassigned.
```

Use `const` by default. Reach for `let` only when you need to update the value.

### What const blocks — the full picture

`const` means "this value never changes, in any way." That covers:

- **Reassignment**: `maxPlayers = 20` — blocked (the example above)
- **Field mutation** (when types land): `player.health = 50` on a `const player` is blocked
- **Mutable borrows** (when ownership lands): you can't pass a `const` value to a function whose signature declares `lend` or `give` — the compiler refuses to grant the mutable access

```ynz
const player: Player = { name: "Patrick", health: 100 }
player.health = 50            // COMPILE ERROR: player is const — fields can't change.
healPlayer(player)            // COMPILE ERROR: player is const — healPlayer's signature is
                              //   `lend player: Player` which needs mutable access.
                              //   Declare with `let` if the function needs to modify it.
```

The compiler rejects all three at compile time. `const` is a complete promise — no exceptions.

**Why this matters**: when you mark a value `const`, the compiler can optimize more aggressively (it knows the value never changes, so it can keep it in a register, share it across threads safely, and skip certain checks). The full picture lives in [Ownership](ownership.md) — `const` and the ownership system work together to give you both safety AND performance.

---

## Type inference — skip the obvious type

Inside a function body, you don't have to write the type when it's obvious:

```
let x = 42              // compiler knows: int
let name = "Patrick"    // compiler knows: string
let active = true       // compiler knows: bool
```

The IDE still shows the inferred type on hover. You just don't have to write it.

Type annotations are required at function boundaries — parameters, return types, and type definitions. Inference only applies inside function bodies:

```
function add(a: number, b: number) -> number {
  let result = a + b    // inferred — fine
  return result
}
```

---

## Every variable must be initialized

You cannot declare a variable without giving it a value:

```
let name: string
// COMPILE ERROR: name has no value.
// Every variable must be initialized. Use maybe string if the value is optional.
```

If a value might not exist, use a `maybe` type — see [Maybe Types](maybe.md).
