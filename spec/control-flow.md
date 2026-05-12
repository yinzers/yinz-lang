# Control Flow

Two forms of `if`. Loops. Early returns. That's it.

---

## Simple `if` — conditions and early returns

```
if (player.health <= 0) {
  return
}

if (player.banned) {
  return
}
```

Parentheses around the condition are required. Curly braces always required.

There is no `else` block. Instead, use early returns to handle one case and fall through to the other:

```
// Instead of if/else — early return pattern
function describe(share player: Player) -> string {
  if (player.health <= 0) {
    return "dead"
  }
  return "alive"
}
```

When you can't return (not in a function, or you need to continue after both cases), use pre-assignment:

```
// Pre-assignment pattern
let message = "in danger"
if (player.health > 50) {
  message = "healthy"
}
print(message)
```

Two patterns, zero `else` blocks. This keeps code flat and readable.

---

## Early returns — the main way to handle branches

Return early to handle edge cases first. The happy path stays at the left margin with no nesting:

```
function processPlayer(lend player: Player) -> nothing {
  if (player.health <= 0) {
    return
  }

  if (player.banned) {
    return
  }

  player.score = player.score + 10
}
```

---

## Boolean operators

```
if (player.active && player.health > 0) {
  // both must be true
}

if (player.banned || player.suspended) {
  // either can be true
}

if (!player.active) {
  // negation
}

// Combined
if (active && (isAdmin || isModerator) && !banned) {
  grantFullAccess()
}
```

Standard symbols — the same as JavaScript and TypeScript.

| Symbol | Meaning |
|--------|---------|
| `&&` | boolean AND |
| `\|\|` | boolean OR |
| `!` | boolean NOT |

Note: `|` (single pipe) is for union types (`type Shape = Circle | Square`), not boolean OR. `||` (double pipe) is boolean OR in expressions.

---

## Multi-case `if` — replaces switch and match

When you need to match a value against several cases, use multi-case `if` with `=>` arrows inside the block. No `switch`. No `match`. Same keyword, different form.

The compiler tells them apart: `=>` inside the block = multi-case matching.

**Matching on `options` values:**

```
if (status) {
  active => print("online")
  inactive => print("offline")
  banned => print("banned")
}
```

**Matching on union types — `is` narrows the type automatically:**

```
if (shape) {
  is Circle => return math.PI * shape.radius * shape.radius
  is Square => return shape.side * shape.side
  is Triangle => return (shape.base * shape.height) / 2
}
```

After `is Circle`, the compiler knows `shape` is a `Circle`. Access `.radius` directly — no cast, no `.value`.

**Matching on values — use `else =>` as catch-all:**

```
if (statusCode) {
  200 => print("ok")
  404 => print("not found")
  500 => print("server error")
  else => print(`unexpected: ${statusCode}`)
}
```

`else =>` is only valid inside multi-case `if` blocks. It is never a standalone `else { }` block.

**Multiline cases — wrap in curly braces:**

```
if (shape) {
  is Circle => {
    let radius = shape.radius
    let area = math.PI * radius * radius
    return area
  }
  is Square => {
    return shape.side * shape.side
  }
  is Triangle => {
    return (shape.base * shape.height) / 2
  }
}
```

---

## Exhaustiveness — compiler enforced for options and unions

For `options` types and union types, the compiler verifies every case is handled:

```
if (shape) {
  is Circle => return circleArea(shape)
  is Square => return squareArea(shape)
  // COMPILE ERROR: not all Shape variants handled. Missing: Triangle.
  // Add a Triangle case or add an else => catch-all.
}
```

For value matching (numbers, strings), exhaustiveness isn't enforced — use `else =>` as a catch-all.

---

## for — loop over a collection

```
for (player in players) {
  print(player.name)
}
```

`player` is a new variable scoped to the loop. Its type is inferred from the collection.

---

## while — loop until a condition is false

```
let health = 100
while (health > 0) {
  health = health - 10
}
```
