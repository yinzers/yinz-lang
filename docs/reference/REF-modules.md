---
name: "REF-modules"
description: "Every .ynz file is a module. Everything inside it is private by default. Add export to make something available to other files."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Modules

Every `.ynz` file is a module. Everything inside it is private by default. Add `export` to make something available to other files.

---

## Exporting

```
// services/users.ynz

export shape User {
  name: String
  email: String
}

export function fetchUser(id: String) -> User errors {
  let response = request.get(`https://api.example.com/users/${id}`)
  return response.json()
}

export function createUser(data: UserInput) -> User errors {
  let response = request.post(`https://api.example.com/users`, { body: data })
  return response.json()
}

// Private — only this file can use this
function validateEmail(email: String) -> Boolean {
  return email.contains(`@`)
}
```

Two states: exported or private. Write `export` to share something, or nothing to keep it private.

---

## Standard Library — No Import Needed

The standard library is always available. You don't import it. Just use it:

```ynz
let squareRoot = math.sqrt(16)
let content = file.read(`data.txt`)
let response = request.get(`https://api.example.com`)
let now = date.now()
```

The compiler knows the entire standard library, and the IDE lists its functions in autocomplete. Your compiled program leaves out anything you don't actually call.

---

## Importing Your Own Code

### Style 1 — Pick specific items

```
import { fetchUser, User } from `services/users`

let user = fetchUser(`abc123`)
```

Use this when you need 1-3 things from a file. Most common style.

### Style 2 — Import the whole module as a name

```
import users from `services/users`

let user = users.fetchUser(`abc123`)
let newUser = users.createUser(data)
```

Use this when you use many things from the same file, or when you want the origin visible at every call site.

This form imports the whole module under one name. `users.fetchUser()` means "call `fetchUser` from the `users` module." Yinz has no default exports.

Both import styles work the same way underneath. The compiler checks which functions you actually call and leaves out the rest.

---

## Import Paths — Always From Project Root

No `./`. No `../`. Every import path is relative to your project root.

```
my-project/
  yinz.toml
  entrypoint.ynz
  services/
    users.ynz
    orders.ynz
  models/
    player.ynz
  utils/
    helpers.ynz
```

```
// From anywhere in the project:
import { fetchUser } from `services/users`
import { Player } from `models/player`
import { formatDate } from `utils/helpers`
```

```
// NOT allowed — relative paths are a compile error
import { fetchUser } from `../services/users`
// COMPILE ERROR: Use project-root paths. `services/users` not `../services/users`.
```

The file path is the import path. You don't set anything up, and you don't need a file that gathers other files' exports.

### Installed packages — just the package name

```
import { parseMarkdown } from `markdown-parser`
import { parseCSV } from `csv-tools`
```

The compiler knows the difference between your project files and installed packages.

---

## Renaming with `as` — Fixing Name Clashes

When two imports have the same name, use `as` to give one a new name:

```
import { fetchUser as getUser } from `services/users`
import { fetchUser as getAdminUser } from `services/admins`

let user = getUser(id)
let admin = getAdminUser(id)
```

`as` also works when you import a whole module:

```
import math as advancedMath from `math`
import math from `vendor/math-legacy`
// COMPILE ERROR — the second 'math' collides with the first.
//                 Rename one:
//
//     import math as advancedMath from `math`
//     import math from `vendor/math-legacy`     ← OK because the other was renamed
```

Whole-module imports rarely clash, because the module name tells the functions apart:

```
import users from `services/users`
import admins from `services/admins`

let user = users.fetchUser(id)
let admin = admins.fetchUser(id)    // no conflict — the module name makes it clear
```

**If two imports clash and you don't rename one, the compiler refuses to guess.** It reports an error at the import and asks you to choose:

```
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
// Use a different local name: `import { Player as otherName } from "..."`
```

The same rule applies if one of your own modules has the same name as a standard library module (for example, a `math.ynz` file next to the built-in `math` module). Neither one wins automatically — rename one with `as`.

---

## Re-Exporting — One Place to Import From

You can collect exports from several files and export them again from one file:

```
// services/index.ynz
export { fetchUser, createUser, User } from `services/users`
export { processOrder, Order } from `services/orders`
```

```
// Now other files can import from the folder:
import { fetchUser, processOrder } from `services`
```

When you import from `services`, the compiler looks for `services/index.ynz`. Re-exporting is optional. It is only a convenience for large projects.

> **Partly built.** The re-export syntax is read and checked by the compiler. Calling a re-exported function from another file does not work yet.

---

## Files That Import Each Other

Two files can import from each other. The compiler handles it automatically.

```
// services/users.ynz
import { Order } from `services/orders`

export shape User {
  name: String
  orders: Array<Order>
}

// services/orders.ynz
import { User } from `services/users`

export shape Order {
  total: Number
  buyer: User
}
```

This works. No error. No workaround needed.

The compiler reads every file and collects the names and types they declare before it checks any of them. When it needs to know what `Order` is, it already does. The order of the imports doesn't matter.

This is safe in Yinz because a file only declares things (shapes, functions, exports). No code runs when a file is imported, so there is no order to get wrong.

---

## What's Not Allowed

**No default exports:**

```
export default function fetchUser() { }    // COMPILE ERROR: no default exports
export function fetchUser() { }            // correct — always named
```

**No `*` imports:**

```
import * as users from `services/users`   // COMPILE ERROR: use module import instead
import users from `services/users`         // correct
```

**No relative paths:**

```
import { helper } from `./helpers`          // COMPILE ERROR: use project-root path
import { helper } from `utils/helpers`      // correct
```

**No side-effect imports:**

```
import `some-module`                        // COMPILE ERROR: imports must bind to something
import { init } from `some-module`         // correct — bind it, then call it
init()
```

---

## Unused Imports

```
import { fetchUser, createUser, deleteUser } from `services/users`

let user = fetchUser(id)    // createUser and deleteUser are never called

// COMPILE WARNING: createUser and deleteUser are imported but never used.
// Remove them or use them.
```

The IDE auto-removes unused imports on save.

---

## IDE Auto-Import

The compiler keeps a list of every exported name in your project and in your installed packages. When you type a name that isn't imported yet:

```
let user = fetchUser(`abc123`)    // fetchUser not imported yet
```

The IDE finds the name in that list and offers to add the import for you. Accept, and the import appears at the top of the file. Delete the last use of the name, and the IDE offers to remove the import.

This works because paths always start at the project root, exports have names, and there is never any doubt about where a name comes from.
