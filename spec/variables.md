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
