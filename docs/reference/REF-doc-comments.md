---
name: "REF-doc-comments"
description: "A /// comment block immediately above a declaration — with no blank line between it and the declaration — is the doc comment for that item. It shows in IDE hover."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-11-20"
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
/// Returns the player's health after a heal.
/// Health never goes above 100.
export function healedHealth(health: Int, amount: Int) -> Int {
  // This note is a regular comment — never shows in hover
  let total = health + amount
  if (total > 100) {
    return 100
  }
  return total
}
```

Hover over a call to `healedHealth` and the IDE shows the two `///` lines as text, then the function signature as highlighted code. You don't need `@param` or `@returns` tags: the signature already shows the types, and the comment says what the function does.

---

## On shapes

Put a `///` block above the shape, and above any field you want to document:

```ynz
/// A spot on the game map.
export shape Position {
  x: Int
  y: Int
}

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

export function greetLoud(name: String) -> String {
  return name.toUpperCase()
}
```

```ynz
/// This WILL show as a doc comment — immediately above with no gap.
export function greetQuiet(name: String) -> String {
  return name.toLowerCase()
}
```

---

## Inside function bodies

Use `//` inside a function body. It is always a private note for people reading the file — never a doc comment.

```ynz
shape Player {
  name: String
}

export function countPlayers(share players: Array<Player>) -> Int {
  // This is only visible when reading this file
  return players.count()
}
```

---

## Non-exported items

A `///` block above a function, shape or options declaration works whether or not it is exported.
