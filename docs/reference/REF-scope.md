---
name: "REF-scope"
description: "Variables are block-scoped. There are no file-level variables or constants; share a fixed value with a function."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Scope

Variables are block-scoped. A file holds declarations only (functions, shapes, imports and exports), so there are no file-level variables.

---

## Block scoping

A variable lives only inside the block where it's declared. Once the block ends, the variable is gone:

```
function process(condition: Boolean) -> Nothing {
  let x = 10                         // lives for the whole function

  if (condition) {
    let y = 20                       // lives only inside this if block
    print(x + y)                     // fine — x is accessible from the outer scope
  }

  print(y)
  // COMPILE ERROR: `y` is not defined.
  // not defined
  // Check the spelling, or declare it: `let y = ...` — `y` has no declaration in scope. Declare it with `let y = ...` before using it, or check whether it's defined in a file you haven't imported yet.
}

function entrypoint() -> Nothing {
  process(true)
}
```

A loop body is a block too. A variable declared inside it is new on each pass and is gone after the loop:

```
function process(items: Array<Int>) -> Nothing {
  for (item in items) {
    let nextItem = item + 1              // new variable each iteration
    print(nextItem)
  }

  print(nextItem)
  // COMPILE ERROR: `nextItem` is not defined.
  // not defined
  // Check the spelling, or declare it: `let nextItem = ...` — `nextItem` has no declaration in scope. Declare it with `let nextItem = ...` before using it, or check whether it's defined in a file you haven't imported yet.
}

function entrypoint() -> Nothing {
  let items: Array<Int> = [1, 2, 3]
  process(items)
}
```

No variable hoisting. No surprises. See a `let`, know exactly where that variable lives.

---

## No file-level variables

A `let` or `const` outside a function is a compile error:

```
const maxRetries = 3

function entrypoint() -> Nothing {
  print(maxRetries)
}
// COMPILE ERROR: Unexpected `const` at the top level.
// Top-level code must be inside a `function`, `shape`, `import`, or `export` declaration.
// Yinz programs are made of functions and shapes. Only declarations are allowed at the top level.
```

To share a fixed value across a whole file, write a function that returns it:

```
function maxHealth() -> Int {
  return 100
}

function minHealth() -> Int {
  return 0
}

shape Player {
  name: String
  health: Int
}

function heal(lend player: Player) -> Nothing {
  player.health = maxHealth()
}

function isDead(share player: Player) -> Boolean {
  return player.health <= minHealth()
}

function entrypoint() -> Nothing {
  let player: Player = { name: `Alice`, health: 0 }
  print(isDead(player))
  heal(player)
  print(isDead(player))
}
```

---

## Sharing a fixed value across files

Export the function, then import it. A project with two files, `constants.ynz` and `entrypoint.ynz`:

```
// constants.ynz
export function maxHealth() -> Int {
  return 100
}

export function apiUrl() -> String {
  return `https://api.example.com`
}
```

```
// entrypoint.ynz
import { maxHealth, apiUrl } from `constants`

function entrypoint() -> Nothing {
  print(maxHealth())
  print(apiUrl())
}
```

No special global keyword. The module system handles it. See [Modules](REF-modules.md).
