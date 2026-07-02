---
name: "REF-testing"
description: "Testing is built into the language. No framework to install. ynz test just works."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

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

## Setup and teardown

Use `setup` and `teardown` blocks to share initialization between tests.

```
// Runs once before any test in this file
setup file {
  db.connect("test-database")
}

// Runs once after all tests in this file
teardown file {
  db.disconnect()
}

// Runs before each test in this file
setup {
  db.clearTables()
}

// Runs after each test in this file
teardown {
  // any per-test cleanup
}

test "creates a user" {
  let user = createUser({ name: "Alice" })
  assert(user.id != "")
}
```

Two scopes:

- **`setup`** (no modifier) — runs before each test (per-test)
- **`setup file`** — runs once before all tests (per-file)

Same applies to `teardown`. Teardown runs in reverse order of setup.

---

## Grouping tests

Use `group "name" { ... }` to organize related tests inside a file. Each group can have its own setup/teardown.

```
group "user creation" {
  setup {
    db.clearTables()
  }

  test "creates a user with valid input" {
    let user = createUser({ name: "Alice", email: "alice@test.com" })
    assert(user.name == "Alice")
  }

  test "rejects invalid email" {
    assertFails(createUser({ name: "Bob", email: "not-an-email" }))
  }
}

group "user queries" {
  setup {
    db.clearTables()
    db.seed("users", testFixtures)
  }

  test "finds user by id" {
    let user = findUser("user-1")
    assert(user.exists())
  }
}

// Tests outside any group still use the file-level setup
test "schema validation passes" {
  // ...
}
```

**Groups can't be nested.** One level only. If you need more structure, split into multiple files.

```
group "outer" {
  group "inner" {        // COMPILE ERROR: Cannot nest groups.
    // ...               //                Use one level per file. Split into
  }                      //                multiple test files if needed.
}
```

---

## Built-in assertions

```
assert(condition)                     // fails if condition is false
assertEqual(actual, expected)         // fails if not equal — shows the diff
assertNotEqual(actual, expected)      // fails if equal
assertGreaterThan(actual, expected)   // fails if actual <= expected
assertLessThan(actual, expected)      // fails if actual >= expected
assertContains(collection, item)      // fails if collection doesn't have item
assertFails(expression)               // fails if expression does NOT trigger an error
assertPanics(expression)              // fails if expression does NOT panic
```

**`assertFails` and `assertPanics` are separate** because they catch different kinds of failure:

```
// assertFails — catches ONLY errors-system failures
let error = assertFails(createUser({ email: "not-an-email" }))
assert(error.message.contains("invalid email"))

// assertPanics — catches ONLY panics
let panic = assertPanics(processUnchecked(badInput))
assert(panic.message.contains("null"))
```

Why the separation: if a single `assertFails` caught both errors AND panics, a typo in your test code that causes a panic would be silently counted as "yes it failed as expected" — and the test would pass for the wrong reason. With separate functions, panics from test bugs always cascade up to the test runner with full stack trace. You see exactly where the test broke.

Both `assertFails` and `assertPanics` return the captured failure for inspection. The returned value has `.message`, `.trace`, and `.source` (`{ file, line }`).

---

## Test output

```
ynz test

users.test.ynz
  user creation
    ✓ creates a user with valid input         (3ms)
    ✓ rejects invalid email                   (1ms)
  user queries
    ✓ finds user by id                        (2ms)
  ✗ schema validation passes                  (1ms)

    FAILED: schema validation passes

      Expected: schema.valid() == true
      Actual:   schema.valid() == false

      users.test.ynz line 42:
        let schema = loadSchema("users")
        assert(schema.valid() == true)

      Hint: loadSchema() returned a schema that failed validation —
            check the test fixture or the schema file itself.

Tests: 46 passed, 1 failed, 47 total
Time:  0.4s
```

Plain English. Points to the exact line. Suggests what might be wrong. Same philosophy as compiler error messages.

---

## Running tests

```
ynz test                    // run all tests in the project
ynz test players            // run tests matching "players" (in file path, group name, or description)
ynz test --watch            // rerun automatically on file change
ynz test --serial           // force all-serial execution (debug flaky tests)
ynz test --parallel N       // cap parallel files at N (default: CPU count)
```

By default, **files run in parallel** with each other for speed, and **tests within a file run sequentially** because they often share setup/teardown ordering. If you want to confirm a test is flaky or a race condition is involved, run with `--serial` to force sequential execution.
