# Modules

Every `.ynz` file is a module. Everything inside it is private by default. Add `export` to make something available to other files.

---

## Exporting

```
// services/users.ynz

export type User {
  name: string
  email: string
}

export function fetchUser(id: string) -> User errors {
  let response = http.get(`https://api.example.com/users/${id}`)
  return response.json()
}

export function createUser(data: UserInput) -> User errors {
  let response = http.post("https://api.example.com/users", { body: data })
  return response.json()
}

// Private — only this file can use this
function validateEmail(email: string) -> bool {
  return email.contains("@")
}
```

Two states: exported or private. No `pub`, no `pub(crate)`, no visibility modifiers. Just `export` or nothing.

---

## Standard Library — No Import Needed

The standard library is always available. You don't import it. Just use it:

```
let x = math.sqrt(16)
let content = file.read("data.txt")
let response = http.get("https://api.example.com")
let now = date.now()
```

The compiler knows the entire standard library. The IDE shows stdlib methods in autocomplete automatically. Tree shaking strips anything you don't actually call.

---

## Importing Your Own Code

### Style 1 — Pick specific items

```
import { fetchUser, User } from "services/users"

let user = fetchUser("abc123")
```

Use this when you need 1-3 things from a file. Most common style.

### Style 2 — Import the whole module as a name

```
import users from "services/users"

let user = users.fetchUser("abc123")
let newUser = users.createUser(data)
```

Use this when you use many things from the same file, or when you want the origin visible at every call site.

**Note for JavaScript developers**: In JavaScript, `import X from "..."` is a default import. In Yinz, there are no default exports — this syntax imports the entire module as a namespace object. Same syntax, different meaning. `users.fetchUser()` means "call `fetchUser` from the `users` module," not "call a method on a default export."

Both import styles tree-shake the same way. The compiler traces which functions you actually call and strips everything else.

---

## Import Paths — Always From Project Root

No `./`. No `../`. Every import path is relative to your project root.

```
my-project/
  yinz.toml
  main.ynz
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
import { fetchUser } from "services/users"
import { Player } from "models/player"
import { formatDate } from "utils/helpers"
```

```
// NOT allowed — relative paths are a compile error
import { fetchUser } from "../services/users"
// COMPILE ERROR: Use project-root paths. "services/users" not "../services/users".
```

File path = import path. No configuration, no mapping, no barrel files required.

### Installed packages — just the package name

```
import { serve } from "http-server"
import { parseCSV } from "csv-tools"
```

The compiler knows the difference between your project files and installed packages.

---

## Aliasing — Resolving Name Conflicts

When two imports have the same name, use `as` to rename one:

```
import { fetchUser as getUser } from "services/users"
import { fetchUser as getAdminUser } from "services/admins"

let user = getUser(id)
let admin = getAdminUser(id)
```

Aliasing also works on namespace imports:

```
import math as advancedMath from "math"
import math from "vendor/math-legacy"
// COMPILE ERROR — the second 'math' collides with the first.
//                 Rename one:
//
//     import math as advancedMath from "math"
//     import math from "vendor/math-legacy"     ← OK because the other was renamed
```

Module imports naturally avoid conflicts without aliasing — the module name itself is the discriminator:

```
import users from "services/users"
import admins from "services/admins"

let user = users.fetchUser(id)
let admin = admins.fetchUser(id)    // no conflict — the module name makes it clear
```

**If you forget to alias a collision, the compiler refuses to guess.** Last-import-wins (the JavaScript / TypeScript behavior) would silently change which value is used when imports are reordered — that's a bug factory. Yinz errors at the import site and forces you to pick:

```
import { Player } from "models/game"
import { Player } from "external/legacy"
// COMPILE ERROR: 'Player' is imported from two places.
//
//   Line 1: from "models/game"
//   Line 2: from "external/legacy"
//
//   These can't both use the name 'Player' in the same file. Rename one:
//
//     import { Player } from "models/game"
//     import { Player as LegacyPlayer } from "external/legacy"
```

The same rule applies if a local module collides with a standard library name (e.g., a local `math.ynz` file colliding with the built-in `math` module). There's no precedence — alias one or the other.

---

## Re-Exporting — Grouped Entry Points

You can collect exports from multiple files and re-export them from one place:

```
// services/index.ynz
export { fetchUser, createUser, User } from "services/users"
export { processOrder, Order } from "services/orders"
```

```
// Now other files can import from the folder:
import { fetchUser, processOrder } from "services"
```

When you import from `"services"`, the compiler looks for `services/index.ynz`. Re-exporting is optional — it's just a convenience for large projects.

---

## Circular References — Not a Problem

Files can import from each other. The compiler handles it automatically through multi-pass compilation.

```
// services/users.ynz
import { Order } from "services/orders"

export type User {
  name: string
  orders: array<Order>
}

// services/orders.ynz
import { User } from "services/users"

export type Order {
  total: number
  buyer: User
}
```

This works. No error. No workaround needed.

The compiler scans all files and collects all type signatures before resolving any references. By the time it tries to figure out what `Order` is, it already knows — because it read every file first. The order files are imported doesn't matter.

This is safe in Yinz because files only contain declarations (types, functions, exports). There's no code that runs when a file is imported. No initialization order to worry about.

---

## What's Not Allowed

**No default exports:**

```
export default function fetchUser() { }    // COMPILE ERROR: no default exports
export function fetchUser() { }            // correct — always named
```

**No wildcard imports:**

```
import * as users from "services/users"   // COMPILE ERROR: use module import instead
import users from "services/users"         // correct
```

**No relative paths:**

```
import { helper } from "./helpers"          // COMPILE ERROR: use project-root path
import { helper } from "utils/helpers"      // correct
```

**No side-effect imports:**

```
import "some-module"                        // COMPILE ERROR: imports must bind to something
import { init } from "some-module"         // correct — bind it, then call it
init()
```

---

## Unused Imports

```
import { fetchUser, createUser, deleteUser } from "services/users"

let user = fetchUser(id)    // createUser and deleteUser are never called

// COMPILE WARNING: createUser and deleteUser are imported but never used.
// Remove them or use them.
```

The IDE auto-removes unused imports on save.

---

## IDE Auto-Import

The compiler builds an index of every exported symbol across your project and all packages. When you type a name that isn't imported yet:

```
let user = fetchUser("abc123")    // fetchUser not imported yet
```

The IDE recognizes it, finds it in the index, and offers to add the import automatically. Accept and the import appears at the top of the file. Delete the usage and the IDE offers to remove the import.

Auto-import works because the module system is built for it: project-root-relative paths, explicit named exports, no ambiguity about where anything comes from.
