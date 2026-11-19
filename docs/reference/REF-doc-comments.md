---
name: "REF-doc-comments"
description: "A /// comment block immediately above a declaration — with no blank line between it and the declaration — is the doc comment for that item. It shows in IDE hover."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-19"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Doc Comments

A `///` comment block immediately above a declaration — with no blank line between it and the declaration — is the doc comment for that item. It shows in IDE hover.

Regular `//` comments are for readers of the source only. The IDE ignores them.

---

## On functions

```ynz
/// Fetches a user by their unique ID.
/// Returns none if the user doesn't exist.
/// Errors on database connection failure.
export function fetchUser(id: UserId) -> maybe<User> errors {
  // This note is a regular comment — never shows in hover
}
```

Hover over a call to `fetchUser` shows the three `///` lines as prose, then the function signature as a colored code block. No `@param` or `@returns` tags needed — the types tell you the structure, the comment tells you the behavior.

---

## On shapes

Put a `///` block above the shape, and above any field you want to document:

```ynz
/// Represents a player in the game world.
export shape Player {
  /// The player's display name.
  name: string
  /// Current HP, clamped 0–100. Never negative.
  health: int
  /// World-space coordinates in meters.
  position: Position
}
```

Hover over `Player` at any use site shows the shape's fields in a colored code block with the doc comment above it.

---

## On options types

```ynz
/// The category of a tradable asset on Alpaca.
export options AssetClass {
  usEquity
  usOption
  crypto: `Cryptocurrency`
  ipo:    `IPO Indication of Interest`
}
```

---

## On constants

```ynz
/// Maximum health any player can have.
export const maxHealth = 100
```

---

## The blank-line rule

A blank line between the comment and the declaration breaks the association. The comment is then ignored.

```ynz
/// This will NOT show as a doc comment — blank line breaks it.

export function greet(share self: Player) -> string {
```

```ynz
/// This WILL show as a doc comment — immediately above with no gap.
export function greet(share self: Player) -> string {
```

---

## Inside function bodies

Use `//` inside a function body. It is always a private implementation note — never a doc comment.

```ynz
export function countActive(share players: array<Player>) -> int {
  // This is only visible when reading this file
  const active = players.filter(player => player.health > 0)
  return active.count()
}
```

---

## Non-exported items

A `///` block above a function, shape or options declaration works whether or not it is exported.
