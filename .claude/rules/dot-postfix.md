---
name: Yinz Dot-Postfix Protocol
description: "Applies when designing dot-postfix syntax (`value.something`) or choosing whether a dot access takes parens."
trigger: relevance
---

**Access takes no parens; an action takes parens.** Why: parens tell the reader at a glance whether the dot reads data or does something.

| Form | Use for | Examples |
|---|---|---|
| `value.name` | field access, type-attached constant | `player.health`, `Int.max` |
| `value.name(args)` | call through dot sugar, or a transformation | `player.heal(20)`, `score.toString()`, `original.copy()` |

- **No parens means a pure read:** no side effect, no allocation, no transformation. Why: the reader can trust it without checking the definition.
- **Parens always mean an action.** `.copy()` makes a new owned value and `.freeze()` locks a binding, so both always take parens; a bare `.copy` reads as a field. Why: a transformation must never look like a read.
- **Module calls follow the same rule:** `request.get(url)` is a call on a module, so it takes parens. Why: one rule has no special cases.
- **Never give ownership modifiers a dot form.** `share`, `lend` and `give` appear in signatures only, and the compiler infers them at call sites. Why: they describe how a function takes a value, not an action on the value.
