# Types

A `type` defines the shape of your data — what fields it has and what types those fields hold.

---

## Defining a type

```
type Player {
  name: string
  health: number
  score: number
}
```

Fields are laid out flat and contiguous in memory. Fast to access, CPU-cache friendly.

---

## Creating an instance

```
let player: Player = {
  name: "Patrick",
  health: 100,
  score: 0
}
```

Every field must be provided. Forgetting one is a compile error:

```
let player: Player = {
  name: "Patrick",
  health: 100
}
// COMPILE ERROR: Missing field 'score' (required by Player).
```

---

## Methods

Types can have functions that operate on them:

```
type Player {
  name: string
  health: number

  function takeDamage(lend self, amount: number) -> nothing {
    self.health = self.health - amount
  }

  function isAlive(share self) -> bool {
    return self.health > 0
  }
}

player.takeDamage(25)    // lend self — this method modifies player
player.isAlive()         // share self — this method only reads
```

`self` works like any other parameter. `lend self` = the method modifies the value. `share self` = read-only.

---

## Extending types — single inheritance

One type can extend another, gaining all its fields and methods:

```
type Entity {
  name: string
  health: number

  function takeDamage(lend self, amount: number) -> nothing {
    self.health = self.health - amount
  }
}

type Warrior extends Entity {
  weapon: string
  armor: number
}

let warrior: Warrior = {
  name: "Patrick",
  health: 100,
  weapon: "sword",
  armor: 15
}

warrior.takeDamage(20)    // inherited from Entity
```

Single inheritance only. A type can `extends` one other type, that's it.

---

## Overriding methods

To replace a method from a parent type, use `override`:

```
type Warrior extends Entity {
  weapon: string
  armor: number

  override function takeDamage(lend self, amount: number) -> nothing {
    self.health = self.health - (amount - self.armor)    // armor absorbs damage
  }
}
```

Forgetting `override` when the method already exists in the parent is a compile error:

```
type Warrior extends Entity {
  function takeDamage(lend self, amount: number) -> nothing { ... }
  // COMPILE ERROR: takeDamage() already exists in Entity.
  // Use "override" if you intend to replace it.
}
```

Using `override` on a method that doesn't exist in the parent is also an error:

```
type Warrior extends Entity {
  override function fly(share self) -> nothing { ... }
  // COMPILE ERROR: override used but fly() does not exist in Entity. Remove "override".
}
```

---

## base types — not directly instantiable

Mark a type `base` if it's only meant to be extended, never created directly:

```
base type Entity {
  name: string
  health: number
}

let e: Entity = { name: "test", health: 50 }    // COMPILE ERROR: Entity is a base type
let w: Warrior = { name: "test", health: 50, weapon: "axe", armor: 10 }   // fine
```

---

## follows — behavior contracts

If two unrelated types need to be used interchangeably, define a contract type and use `follows`:

```
type Damageable {
  health: number
  function takeDamage(lend self, amount: number) -> nothing
}

type Player follows Damageable {
  name: string
  health: number

  function takeDamage(lend self, amount: number) -> nothing {
    self.health = self.health - amount
  }
}

type Building follows Damageable {
  address: string
  health: number

  function takeDamage(lend self, amount: number) -> nothing {
    self.health = self.health - (amount / 2)    // buildings take half damage
  }
}

function dealDamage(lend target: Damageable, amount: number) -> nothing {
  target.takeDamage(amount)
}

dealDamage(player.lend, 50)      // Player follows Damageable — works
dealDamage(building.lend, 30)    // Building follows Damageable — works
```

`follows` is optional — Yinz uses structural typing (if the shape matches, it works). But declaring `follows` catches mismatches at definition time rather than when you first try to use the type, and makes the relationship visible to anyone reading the code.

A type can follow multiple contracts — `extends` comes first, then `follows` with a comma-separated list:

```
type Warrior extends Entity follows Damageable, Attackable, Renderable {
  ...
}
```

---

## Structural typing — shape matching

You don't have to name the type when returning a literal. The shape just has to match:

```
type DivResult {
  quotient: number
  remainder: number
}

function divmod(a: number, b: number) -> DivResult {
  return { quotient: a / b, remainder: a % b }    // shape matches DivResult — valid
}
```

The compiler checks that the fields exist with the right types. No need to write `new DivResult(...)`.

---

## Hidden fields — invisible outside the type

Mark a field `hidden` to make it completely invisible to code outside the type's own methods. Hidden fields require a default value:

```
type Player {
  name: string
  health: number
  hidden damageMultiplier: number = 1.0
  hidden internalCache: map[string, number] = {}

  function takeDamage(lend self, amount: number) -> nothing {
    let actual = amount * self.damageMultiplier    // accessible inside Player's methods
    self.health = self.health - actual
  }
}
```

The caller only provides visible fields when creating the type:

```
let player: Player = { name: "Alice", health: 100 }
// damageMultiplier starts at 1.0, internalCache starts empty
// both defaults are visible in the type definition above
```

Accessing a hidden field from outside the type is a compile error:

```
print(player.damageMultiplier)
// COMPILE ERROR: damageMultiplier is hidden — not accessible outside Player.
```

`hidden` is about visibility, not mutability. Hidden fields can be both read and modified by the type's own methods. They simply don't exist to outside code.

---

## Type aliases

Create a new name for an existing type. Zero runtime cost — the alias is erased at compile time.

```
type UserId = string
type Timestamp = number
type PlayerList = array[Player]
type Coordinates = { x: number, y: number }
```

The alias and the original type are fully interchangeable:

```
type UserId = string

function fetchUser(id: UserId) -> maybe User errors { ... }

let id: UserId = "abc123"    // fine
let id: string = "abc123"    // also fine — same type
fetchUser(id)                 // works either way
```

Aliases are for self-documentation — they make signatures tell a story:

```
// Without alias — what does this string represent?
function fetchUser(id: string) -> maybe User errors

// With alias — immediately clear
function fetchUser(id: UserId) -> maybe User errors
```
