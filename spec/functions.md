# Functions

Functions are blocks of code that do one job and return a result.

---

## Defining a function

```
function add(a: number, b: number) -> number {
  return a + b
}
```

Every function:
- Starts with `function`
- Lists its parameters with their types
- Declares what it returns with `->`
- Always has a return type — no exceptions

---

## Return types are always required

The compiler enforces that every code path actually returns the declared type:

```
function getScore(share player: Player) -> number {
  return player.score
}
```

---

## nothing — functions that don't return a value

If a function does something but gives nothing back, say `-> nothing`:

```
function logMessage(share msg: string) -> nothing {
  print(msg)
}
```

`nothing` reads like English. The function runs and returns... nothing.

Using `return` inside a `nothing` function is fine for early exits:

```
function processPlayer(lend player: Player) -> nothing {
  if (player.health <= 0) {
    return    // early exit — done
  }
  player.health = player.health - 10
}
```

Returning a value from a `nothing` function is a compile error:

```
function logMessage(share msg: string) -> nothing {
  return msg
  // COMPILE ERROR: logMessage is declared to return nothing — it cannot return a value.
  //
  //   Either remove the return value:
  //     return
  //   Or change the function's return type:
  //     function logMessage(share msg: string) -> string { return msg }
  //
  //   Why: -> nothing means the function does its work and returns no value
  //        to the caller. Returning a value would contradict that promise.
}
```

---

## Parameters — share, lend, give

Every parameter says how the function uses the value it receives. See [Ownership](ownership.md) for the full picture.

```
function greet(share name: string) -> nothing      // just reading name
function rename(lend player: Player) -> nothing    // going to modify player
function consume(give data: Data) -> nothing       // taking ownership
```

If a function only reads, the caller doesn't need to annotate:

```
greet(playerName)    // compiler infers .share — no annotation needed
```

Only annotate when you're escalating beyond reading:

```
rename(player.lend)    // explicit — granting write access
consume(data.give)     // explicit — giving it away permanently
```

---

## Arrow functions — for callbacks only

Arrow syntax is for inline callbacks passed to functions. Don't use it for standalone named functions — use `function` for those.

**Simple form** — the compiler infers types from context:

```
let active = players.filter(p => p.health > 0)
let names = players.map(p => p.name)
```

**Multi-line** — curly braces with explicit return:

```
let processed = players.map(p => {
  let bonus = p.health * 2
  return p.name + ": " + bonus
})
```

**Typed form** — when the compiler needs explicit types (complex callbacks, HTTP route handlers, anywhere inference isn't enough):

```
server.get("/users", (request: Request) -> Response => {
  let users = loadUsers()
  return response.json(users)
})
```

The pattern is `(params: Types) -> ReturnType => { body }` — the same shape as a named function, just `=>` where the function name would be. The `-> ReturnType` is only required when the compiler can't infer it.

When to use which form:
1. `p => expr` — when types are obvious from context
2. `p => { ... }` — multi-line, types still inferred
3. `(p: T) -> R => { ... }` — when the compiler asks for explicit types

---

## No tuples — define a type instead

If you need to return multiple values, define a type. Named fields are always clearer than positional slots:

```
type DivResult {
  quotient: number
  remainder: number
}

function divmod(a: number, b: number) -> DivResult {
  return { quotient: a / b, remainder: a % b }
}
```
