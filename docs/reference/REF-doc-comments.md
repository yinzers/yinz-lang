---
name: "REF-doc-comments"
description: "A // comment block immediately above a declaration — with no blank line between it and the declaration — is the doc comment for that item. It shows in IDE hover and in ynz doc generated output."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Doc Comments

A `//` comment block immediately above a declaration — with no blank line between it and the declaration — is the doc comment for that item. It shows in IDE hover and in `ynz doc` generated output.

Regular `//` comments anywhere else (inside function bodies, with a blank line above the declaration) are for readers of the source only. The IDE and doc generator ignore them.

---

## On functions

```ynz
// Fetches a user by their unique ID.
// Returns none if the user doesn't exist.
// Errors on database connection failure.
export function fetchUser(id: UserId) -> maybe User errors {
  // This note is inside the body — never shows in hover or docs
}
```

Hover over a call to `fetchUser` shows the three `//` lines as prose, then the function signature as a colored code block. No `@param` or `@returns` tags needed — the types tell you the structure, the comment tells you the behavior.

---

## On shapes

```ynz
// Represents a player in the game world.
export shape Player {
  name: string          // The player's display name.
  health: int           // Current HP, clamped 0–100. Never negative.
  position: Position    // World-space coordinates in meters.
}
```

Hover over `Player` at any use site shows the shape's fields in a colored code block with the doc comment above it.

---

## On options types

```ynz
// The category of a tradable asset on Alpaca.
export options AssetClass {
  us_equity
  us_option
  crypto: `Cryptocurrency`
  ipo:    `IPO Indication of Interest`
}
```

---

## On constants

```ynz
// Maximum health any player can have.
export const MAX_HEALTH = 100
```

---

## The blank-line rule

A blank line between the comment and the declaration breaks the association. The comment becomes free-floating source-only trivia.

```ynz
// This will NOT show as a doc comment — blank line breaks it.

export function greet(share self: Player) -> string {
```

```ynz
// This WILL show as a doc comment — immediately above with no gap.
export function greet(share self: Player) -> string {
```

---

## Inside function bodies

`//` inside a function body is always a private implementation note — never a doc comment, regardless of position.

```ynz
export function process(share data: array<int>) -> int {
  // This is only visible when reading this file
  const filtered = data.where(n => n > 0)
  return filtered.sum()
}
```

---

## Non-exported items

Doc comments on non-exported items still show in IDE hover within the same project. They are excluded from `ynz doc` generated output, since external users cannot reach those items.
