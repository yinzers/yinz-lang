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

### Style 1 — Pick specific items

```ynz
import { fetchUser, User } from `services/users`

function entrypoint() -> Nothing {
  let user: User = fetchUser(7)
  print(user.name)
}
```

Use this when you need a few things from a file. It is the most common style.

### Style 2 — Import the whole module as a name

```ynz
import users from `services/users`

function entrypoint() -> Nothing {
  let user = users.fetchUser(7)
  print(user.name)
}
```

Use this when you use many things from the same file, or when you want the origin visible at every call site.

This form imports the whole module under one name. `users.fetchUser()` means "call `fetchUser` from the `users` module." Yinz has no default exports.

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
import { fetchUser } from `services/users`
import { Player } from `models/player`
import { formatDate } from `utils/helpers`
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
Import path "../services/users" contains `..` or `.` segments.
```

Use the path from the project root: `services/users`.

---

## Renaming with `as` — fixing name clashes

When two imports have the same name, use `as` to give one a new name:

```ynz
import { fetchUser as getUser } from `services/users`
import { fetchUser as getAdminUser } from `services/admins`

function entrypoint() -> Nothing {
  print(getUser(1).name)
  print(getAdminUser(2).name)
}
```

Whole-module imports rarely clash, because the module name tells the functions apart:

```ynz
import users from `services/users`
import admins from `services/admins`

function entrypoint() -> Nothing {
  print(users.fetchUser(1).name)
  print(admins.fetchUser(2).name)
}
```

If two imports use the same name and you don't rename one, the compiler refuses to guess:

```ynz
import { Player } from `models/game`
import { Player } from `external/legacy`
```

```
Import name `Player` is already bound by a previous import.
```

Rename one of them: ``import { Player as otherName } from `external/legacy` ``.

---

## Files that import each other

Two files that import each other are an error:

```
Circular import detected: "services/orders" is already being imported in this chain.
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

Write ``import users from `services/users` ``.

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
