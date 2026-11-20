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
export function fetchUser(id: UserId) -> Maybe<User> errors {
  // This note is a regular comment — never shows in hover
}
```

Hover over a call to `fetchUser` and the IDE shows the three `///` lines as text, then the function signature as highlighted code. You don't need `@param` or `@returns` tags: the signature already shows the types, and the comment says what the function does.

---

## On shapes

Put a `///` block above the shape, and above any field you want to document:

```ynz
/// Represents a player in the game world.
export shape Player {
  /// The player's display name.
  name: String
  /// Current health, from 0 to 100. Never negative.
  health: Int
  /// Where the player stands in the game world, in meters.
  position: Position
}
```

Hover over `Player` anywhere you use it and the IDE shows the shape's fields as highlighted code, with the doc comment above them.

---

## On options

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

export function greet(share self: Player) -> String {
```

```ynz
/// This WILL show as a doc comment — immediately above with no gap.
export function greet(share self: Player) -> String {
```

---

## Inside function bodies

Use `//` inside a function body. It is always a private note for people reading the file — never a doc comment.

```ynz
export function countActive(share players: Array<Player>) -> Int {
  // This is only visible when reading this file
  const active = players.filter(player => player.health > 0)
  return active.count()
}
```

---

## Non-exported items

A `///` block above a function, shape or options declaration works whether or not it is exported.
