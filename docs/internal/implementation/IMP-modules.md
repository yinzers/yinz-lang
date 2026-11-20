---
name: "IMP-modules"
description: "Design decisions for the Yinz module system — export/import keywords with private-by-default visibility, no default exports, and no wildcard imports."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Module System — Design Decisions

User spec: [`docs/reference/REF-modules.md`](../../reference/REF-modules.md)

---

## Two Keywords, Private by Default

`export` and `import`. Everything is private by default. No `pub`, no `pub(crate)`, no visibility modifiers.

**Why**: Two states, zero confusion. Private or exported. The most common state (private helper functions, internal types) requires no annotation. The intentional state (public API) requires one word.

---

## No Default Exports

Named exports only. Always `export function fetchUser()`, never `export default function fetchUser()`.

**Why**: Default exports in JavaScript created a two-tier import system (`import X` vs `import { X }`) that confuses developers constantly. One pattern — always named, always curly braces for destructured imports.

---

## No Wildcard Imports

`import * as X from `module`` is a compile error. Use a module namespace import instead: ``import X from `module` ``.

Wildcard imports hide where symbols come from. A namespace import keeps every use readable as `X.name`.

**Note for JavaScript developers**: `import X from ...` in Yinz is NOT a default import — it imports the entire module as a namespace object.

---

## Project-Root-Relative Paths Only

Write `` `services/users` ``, never `` `../services/users` `` or `` `./services/users` ``. No relative paths anywhere: the path reads the same from every file, so moving a file never breaks its imports.

---

## Circular References Just Work

Multi-pass compilation. Pass 1 collects all type signatures across all files before resolving any references. Circular imports between files are not a problem.

**Why this is safe**: Yinz files contain only declarations — types, functions, exports. No module-level initialization code runs when a file is imported. There's no ordering problem to solve. The compiler sees the whole project at once.

---

## Standard Library Auto-Imported

The standard library is always available with no import statement. `math.sqrt()` works anywhere without `import { sqrt } from `math``.

**Why**: The compiler and IDE are built alongside the language — the stdlib is fully known at compile time. Forcing imports of always-available modules adds ceremony with zero benefit. A junior developer's first program should not require knowing which import to add before they can use `math.sqrt()`.

Tree shaking still works — the compiler traces which stdlib functions are actually called and strips everything else from the binary.

---

## Unused Imports Are Warnings

Unused imports produce a compile warning. The IDE auto-removes them on save.

**Why**: Unused imports add noise and slow compile times (more symbols to analyze). Making it a warning (not an error) means code still compiles during development when imports are temporarily unused.

---

## Import Aliases — `as` Renaming

Both named and namespace imports support `as` renaming:

```ynz
// Destructured import with rename
import { fetchUser as getUser } from `services/users`

// Namespace import with rename
import math as advancedMath from `math`

// Mixed within one import
import { Player as PlayerType, Score } from `models/game`
```

Aliasing resolves naming collisions (a local `Player` and a third-party `Player`, a custom `math` namespace and stdlib) without renaming the source.

---

## Re-Exporting — One Place to Import From

A file can collect exports from several files and export them again, so a folder has one import point:

```ynz
// services/index.ynz
export { fetchUser, createUser, User } from `services/users`
export { processOrder, Order } from `services/orders`
```

```ynz
// Other files import from the folder:
import { fetchUser, processOrder } from `services`
```

Importing from `services` makes the compiler look for `services/index.ynz`. Re-exporting is optional.

**Status: partly built.** The compiler reads and checks the re-export syntax. Calling a re-exported function from another file does not work yet, so import from the file that declares the function until it does.

---

## Duplicate Import Names — Compile Error, Force Aliasing

When two imports bring the same name (or namespace) into the same file, the compiler refuses to pick one. The diagnostic text below is from `crates/ynz-typeck/src/resolve_import.rs`; I did not run it through `ynz build` in this lane (pending verification).

```ynz
import { Player } from `models/game`
import { Player } from `external/legacy`
// COMPILE ERROR: Import name `Player` is already bound by a previous import.
//   Use a different local name: `import { Player as otherName } from `...``
//   Each imported name must be unique in the local scope.
```

Same rule applies to namespace imports (`import request from ...` collisions) and to local-vs-stdlib name collisions (a local module named `math` colliding with `stdlib/math`).

The compiler never picks silently, so reordering imports cannot change which `Player` a file means.

**Resolution implication for stdlib vs local**: there's no special precedence rule. If a project happens to define a local `math` module, the compiler errors at the import site and forces the user to alias. No silent shadowing of stdlib.
