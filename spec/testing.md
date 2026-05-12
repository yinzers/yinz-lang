# Testing

Testing is built into the language. No framework to install. `ynz test` just works.

---

## Test files

Test files end in `.test.ynz` and live next to the code they test:

```
models/
  player.ynz
  player.test.ynz       ← tests for player.ynz
services/
  auth.ynz
  auth.test.ynz
```

---

## Writing tests

Use the `test` keyword with a description string:

```
// player.test.ynz

import { Player, healPlayer } from "models/player"

test "new player has full health" {
  let player: Player = { name: "Alice", health: 100 }
  assert(player.health == 100)
}

test "healing doesn't exceed max health" {
  let player: Player = { name: "Alice", health: 90 }
  let healed = healPlayer(player, 50)
  assert(healed.health == 100)
}

test "damage reduces health" {
  let player: Player = { name: "Alice", health: 100 }
  player.takeDamage(30)
  assert(player.health == 70)
}
```

Each test is independent. The description is a plain English sentence — it becomes the label in output and makes failing tests easy to find.

---

## Built-in assertions

```
assert(condition)                     // fails if condition is false
assertEqual(actual, expected)         // fails if not equal — shows the diff
assertNotEqual(actual, expected)      // fails if equal
assertGreaterThan(actual, expected)   // fails if actual <= expected
assertLessThan(actual, expected)      // fails if actual >= expected
assertContains(collection, item)      // fails if collection doesn't have item
assertFails(expression)               // fails if expression does NOT error
```

---

## Test output

```
ynz test

✓ new player has full health
✓ healing doesn't exceed max health
✗ damage reduces health

  FAILED: damage reduces health
    Expected: player.health == 70
    Actual:   player.health == 100

    player.test.ynz line 18:
      player.takeDamage(30)
      assert(player.health == 70)

    Hint: takeDamage takes (lend self) — did you pass the player correctly?

1 failed, 2 passed
```

Plain English. Points to the exact line. Suggests what might be wrong. Same philosophy as compiler error messages.

---

## Running tests

```
ynz test                    // run all tests in the project
ynz test players            // run tests whose description matches "players"
ynz test --watch            // rerun automatically on file change
```
