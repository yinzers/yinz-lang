---
name: "REF-modules"
description: "Every .ynz file is a module. Everything inside it is private by default. Add export to make something available to other files."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Modules

Every `.ynz` file is a module. Everything inside it is private by default. Add `export` to make something available to other files.

---

## Exporting

```ynz
// services/users.ynz

export shape User {
  name: String
  email: String
}

export function fetchUser(id: Int) -> User {
  return { name: `user${id.toString()}`, email: `user${id.toString()}@example.com` }
}

// Private — only this file can use this
function validateEmail(email: String) -> Boolean {
  return email.contains(`@`)
}
```

Two states: exported or private. Write `export` to share something, or nothing to keep it private.

---

## Built-in functions — no import needed

Functions such as `print` and `sleep` are always available. You don't import them:

```ynz
function entrypoint() -> Nothing {
  print(`hello`)
}
```

---

## Importing your own code

Name the items you need, in braces:

```ynz
import { fetchUser, User } from `services/users`

function entrypoint() -> Nothing {
  let user: User = fetchUser(7)
  print(user.name)
}
```

Yinz has no default exports: every import names the items it brings in.

---

## Import paths — always from the project root

No `./`. No `../`. Every import path is a backtick string, relative to your project root, without the `.ynz` suffix.

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

```ynz
// From anywhere in the project:
import { fetchUser, User } from `services/users`
import { Player } from `models/player`
import { formatDate } from `utils/helpers`

function entrypoint() -> Nothing {
  let user: User = fetchUser(7)
  let player: Player = { name: user.name }
  print(player.name)
  print(formatDate(3))
}
```

The file path is the import path. You don't set anything up.

An import path names one file. ``import { fetchUser } from `services` `` looks for `services.ynz`, not for a file inside a `services` folder, and reports:

```
Module "services" not found.
```

Write the full path to the file: `services/users`.

A relative path is an error:

```ynz
import { fetchUser } from `../services/users`
```

```
Import paths must be project-root relative, not relative paths.
```

Use the path from the project root: `services/users`.

---

## Renaming with `as` — fixing name clashes

Use `as` to give an import a new name in this file:

```ynz
// services/admins.ynz
export shape Admin {
  name: String
}

export function fetchAdmin(id: Int) -> Admin {
  return { name: `admin${id.toString()}` }
}
```

```ynz
// entrypoint.ynz
import { fetchUser as getUser, User } from `services/users`
import { fetchAdmin as getAdmin, Admin } from `services/admins`

function entrypoint() -> Nothing {
  let user: User = getUser(1)
  let admin: Admin = getAdmin(2)
  print(user.name)
  print(admin.name)
}
```

If two imports use the same name and you don't rename one, the compiler refuses to guess:

```ynz
import { Player } from `models/game`
import { Player } from `external/legacy`

function entrypoint() -> Nothing {
  print(`hello`)
}
```

```
Import name `Player` is already bound by a previous import.
```

Rename one of them:

```ynz
import { Player } from `models/game`
import { Player as legacyPlayer } from `external/legacy`

function entrypoint() -> Nothing {
  let current: Player = { name: `Ana` }
  let old: legacyPlayer = { nickname: `Ace` }
  print(current.name)
  print(old.nickname)
}
```

---

## Files that import each other

Two files that import each other are an error:

```ynz
// services/orders.ynz
import { loadCustomer } from `services/customers`

export function loadOrder(id: Int) -> Int {
  return loadCustomer(id)
}
```

```ynz
// services/customers.ynz
import { loadOrder } from `services/orders`

export function loadCustomer(id: Int) -> Int {
  return loadOrder(id)
}
```

The compiler reports:

```
Circular import: this module is part of a mutually-recursive import chain.
```

Move the shared shapes into a third file that both files import.

---

## What's not allowed

**No default exports:**

```ynz
export default function fetchUser() { }
```

```
Yinz does not have default exports.
```

Write `export function fetchUser() { }`.

**No `*` imports:**

```ynz
import * as users from `services/users`
```

```
Wildcard imports are not allowed in Yinz.
```

Write ``import { fetchUser } from `services/users` ``.

**No relative paths:** see "Import paths" above.

**No imports without a name:**

```ynz
import `some-module`
```

```
Expected a name or `{` after `import`.
```

Bind what you import: ``import { init } from `some-module` ``.

---

## Unused imports

Importing a name and never using it is a warning, not an error.

```ynz
// services/users.ynz
export function fetchUser(id: Int) -> String {
  return `user${id.toString()}`
}

export function createUser(id: Int) -> String {
  return `new${id.toString()}`
}
```

```ynz
// entrypoint.ynz
import { fetchUser, createUser } from `services/users`

function entrypoint() -> Nothing {
  print(fetchUser(1))
}
```

```
`createUser` is imported but never used.
```

This is a warning. The program still compiles. Remove the name or use it.

---

## IDE auto-import

The compiler keeps a list of every exported name in your project. When you type a name that isn't imported yet, the IDE finds it in that list and offers to add the import for you. Accept, and the import appears at the top of the file.
