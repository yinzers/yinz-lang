# Module System — Design Decisions

User spec: `spec/modules.md`

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
