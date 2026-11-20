---
name: "REF-scope"
description: "Variables are block-scoped. File-level declarations are constants only."
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

Variables are block-scoped. File-level declarations are constants only.

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

  print(y)                           // COMPILE ERROR: `y` is not defined.
}
```

A loop body is a block too. A variable declared inside it is new on each pass and is gone after the loop:

```
function process(items: Array<Int>) -> Nothing {
  for (item in items) {
    let temp = item + 1              // new variable each iteration
    print(temp)
  }

  print(temp)                        // COMPILE ERROR: `temp` is not defined.
}
```

No variable hoisting. No surprises. See a `let`, know exactly where that variable lives.

---

## File-level constants

`const` declared outside any function is accessible by all functions in that file:

```
// player.ynz

const maxHealth = 100
const minHealth = 0

shape Player {
  name: String
  health: Int
}

function heal(lend player: Player) -> Nothing {
  player.health = maxHealth           // accessible
}

function isDead(share player: Player) -> Boolean {
  return player.health <= minHealth   // accessible
}
```

Mutable file-level variables are not allowed. A `let` outside a function is a compile error, so write `const`:

```
const maxRetries = 3      // fine
```

---

## Sharing constants across files

Export a constant to share it across files:

```
// constants.ynz
export const maxHealth = 100
export const apiUrl = `https://api.example.com`
export const gravity: Float = 9.81

// anywhere else in the project
import { maxHealth } from `constants`
```

No special global keyword. The module system handles it.
