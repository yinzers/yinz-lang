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

`import * as X from "module"` is a compile error. Use module namespace import instead: `import X from "module"`.

**Why**: Wildcard imports obscure where symbols come from and import everything regardless of tree shaking. Module namespace imports are explicit, readable, and still allow using the whole module as a namespace.

**Note for JavaScript developers**: `import X from "..."` in Yinz is NOT a default import — it imports the entire module as a namespace object. The syntax looks identical to JS default imports but has different semantics.

---

## Project-Root-Relative Paths Only

`"services/users"` not `"../services/users"` or `"./services/users"`. No relative paths anywhere.

**Why**: Relative paths are fragile — moving a file breaks all its relative imports. Project-root paths are stable — the path is always the same regardless of which file imports it. No configuration, no mapping, no barrel files required.

---

## Circular References Just Work

Multi-pass compilation. Pass 1 collects all type signatures across all files before resolving any references. Circular imports between files are not a problem.

**Why this is safe**: Yinz files contain only declarations — types, functions, exports. No module-level initialization code runs when a file is imported. There's no ordering problem to solve. The compiler sees the whole project at once.

---

## Standard Library Auto-Imported

The standard library is always available with no import statement. `math.sqrt()` works anywhere without `import { sqrt } from "math"`.

**Why**: The compiler and IDE are built alongside the language — the stdlib is fully known at compile time. Forcing imports of always-available modules adds ceremony with zero benefit. A junior developer's first program should not require knowing which import to add before they can use `math.sqrt()`.

Tree shaking still works — the compiler traces which stdlib functions are actually called and strips everything else from the binary.

---

## Unused Imports Are Warnings

Unused imports produce a compile warning. The IDE auto-removes them on save.

**Why**: Unused imports add noise and slow compile times (more symbols to analyze). Making it a warning (not an error) means code still compiles during development when imports are temporarily unused.

---

## Import Aliases — `as` Renaming

Both named and namespace imports support `as` renaming, TypeScript-style:

```ynz
// Destructured import with rename
import { fetchUser as getUser } from "services/users"

// Namespace import with rename
import math as advancedMath from "math"

// Mixed within one import
import { Player as PlayerType, Score } from "models/game"
```

**Why**: Real codebases have naming collisions — a local `Player` type and a third-party `Player` type, a custom `math` namespace overlapping with stdlib. Aliasing is the only escape valve that doesn't require renaming the source. TypeScript's syntax is well-understood; reuse it rather than invent a Yinz-specific spelling.

---

## Duplicate Import Names — Compile Error, Force Aliasing

When two imports bring the same name (or namespace) into the same file, the compiler refuses to silently pick one:

```ynz
import { Player } from "models/game"
import { Player } from "external/legacy"
//
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

Same rule applies to namespace imports (`import request from "..."` collisions) and to local-vs-stdlib name collisions (a local module named `math` colliding with `stdlib/math`).

**Why**: Silent picking is a TypeScript / JavaScript footgun — last-import-wins semantics cause subtle bugs when refactors reorder imports. Forcing the user to disambiguate makes the code's intent explicit and refactor-safe. Same principle as `maybe`: if there's ambiguity, the compiler refuses to guess.

**Resolution implication for stdlib vs local**: there's no special precedence rule. If a project happens to define a local `math` module, the compiler errors at the import site and forces the user to alias. No silent shadowing of stdlib.
