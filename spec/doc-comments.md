# Doc Comments

Triple-slash `///` comments on exported items appear in generated documentation. Regular `//` comments are for readers of the code and are ignored by the doc generator.

---

## On functions

```
/// Fetches a user by their unique ID.
/// Returns none if the user doesn't exist.
/// Errors on database connection failure.
export function fetchUser(id: UserId) -> maybe User errors {
  // implementation notes here — not in docs
}
```

---

## On types and fields

```
/// Represents a player in the game world.
export shape Player {
  /// The player's display name.
  name: string
  /// Current health points, clamped to 0-100.
  health: number
  /// Position in the game world.
  position: Position
}
```

---

## On constants

```
/// Maximum health any player can have.
export const MAX_HEALTH = 100
```

---

## Rules

- `///` only works on exported items. Commenting a private function has no effect on generated docs.
- Multiple `///` lines become a multi-line doc entry.
- `///` goes immediately above the item. A blank line between the comment and the item breaks the association.
- No block doc syntax (`/** */`) — triple-slash only.

---

## Regular comments vs doc comments

```
// This explains something to readers of THIS file — never in generated docs
/// This documents the public API — appears in generated docs

export function process() -> nothing {
  // Private implementation notes — ignored by the doc generator
}
```
