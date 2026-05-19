# Scope

Variables are block-scoped. File-level declarations are constants only.

---

## Block scoping

A variable lives only inside the block where it's declared. Once the block ends, the variable is gone:

```
function process() -> nothing {
  let x = 10                         // lives for the whole function

  if (condition) {
    let y = 20                       // lives only inside this if block
    print(x + y)                     // fine — x is accessible from the outer scope
  }

  print(y)                           // COMPILE ERROR: y is not defined here

  for (item in items) {
    let temp = transform(item)       // new variable each iteration
  }

  print(temp)                        // COMPILE ERROR: temp is not defined here
}
```

No variable hoisting. No surprises. See a `let`, know exactly where that variable lives.

---

## File-level constants

`const` declared outside any function is accessible by all functions in that file:

```
// player.ynz

const MAX_HEALTH = 100
const MIN_HEALTH = 0
const RESPAWN_TIME = duration.seconds(5)

function heal(lend player: Player) -> nothing {
  player.health = MAX_HEALTH          // accessible
}

function isDead(share player: Player) -> bool {
  return player.health <= MIN_HEALTH  // accessible
}
```

Mutable file-level variables are not allowed:

```
let globalCounter = 0     // COMPILE ERROR: file-level variables must be const.
const MAX_RETRIES = 3     // fine
```

---

## Const expressions — evaluated at compile time

File-level constants can use pure function calls as long as all inputs are constant. The compiler evaluates these before the program runs:

```
// Valid const expressions — compiler evaluates at compile time
const RESPAWN_TIME = duration.seconds(5)
const MAX_AREA = math.PI * 100 * 100
const GREETING = `Hello world`
const GRAVITY: float = 9.81
```

Calls that depend on runtime information are not valid:

```
const NOW = date.now()                  // COMPILE ERROR: date.now() depends on runtime
const CONFIG = file.read(`config.ynz`) // COMPILE ERROR: file.read() is I/O
```

The rule: can the compiler know this value before the program starts? Pure math, pure string operations, and pure stdlib functions with constant arguments — yes. Anything reading from the clock, file system, network, or OS — no.

---

## Sharing constants across files

Export a constant to share it across files:

```
// constants.ynz
export const MAX_HEALTH = 100
export const API_URL = `https://api.example.com`
export const GRAVITY: float = 9.81

// anywhere else in the project
import { MAX_HEALTH } from `constants`
```

No special global keyword. The module system handles it.
