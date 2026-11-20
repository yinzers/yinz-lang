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

> **Not available yet.** The `test` keyword is reserved, but the test runner (`ynz test`) is not built. This page describes the planned design.

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

Use the `test` keyword with a description:

```
// player.test.ynz

import { Player, healPlayer, takeDamage } from `models/player`

test `new player has full health` {
  let player: Player = { name: `Alice`, health: 100 }
  assert(player.health == 100)
}

test `healing doesn't exceed max health` {
  let player: Player = { name: `Alice`, health: 90 }
  let healed = healPlayer(player, 50)
  assert(healed.health == 100)
}

test `damage reduces health` {
  let player: Player = { name: `Alice`, health: 100 }
  takeDamage(player, 30)
  assert(player.health == 70)
}
```

Each test is independent. The description is a plain English sentence. It becomes the label in the output, so a failing test is easy to find.

---

## Setup and teardown

Use `setup` and `teardown` blocks to share preparation and cleanup between tests.

```
// Runs once before any test in this file
setup file {
  db.connect(`test-database`)
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

test `creates a user` {
  let user = createUser({ name: `Alice` })
  assert(user.id != ``)
}
```

There are two kinds:

- **`setup`** (no modifier) — runs before each test
- **`setup file`** — runs once, before all the tests in the file

`teardown` works the same way. Teardown runs in the reverse order of setup.

---

## Grouping tests

Use ``group `name` { ... }`` to organize related tests inside a file. Each group can have its own setup and teardown.

```
group `user creation` {
  setup {
    db.clearTables()
  }

  test `creates a user with valid input` {
    let user = createUser({ name: `Alice`, email: `alice@test.com` })
    assert(user.name == `Alice`)
  }

  test `rejects invalid email` {
    assertFails(createUser({ name: `Bob`, email: `not-an-email` }))
  }
}

group `user queries` {
  setup {
    db.clearTables()
    db.seed(`users`, testFixtures)
  }

  test `finds user by id` {
    let user = findUser(`user-1`)
    assert(user.exists())
  }
}

// Tests outside any group still use the file-level setup
test `schema validation passes` {
  // ...
}
```

**Groups can't be nested.** One level only. If you need more structure, split the tests into several files.

```
group `outer` {
  group `inner` {        // COMPILE ERROR: Cannot nest groups.
    // ...               //                Use one level per file. Split into
  }                      //                multiple test files if needed.
}
```

---

## Built-in assertions

An *assertion* is a check that fails the test when it is not true.

```
assert(condition)                     // fails if condition is false
assertEqual(actual, expected)         // fails if not equal — shows the difference
assertNotEqual(actual, expected)      // fails if equal
assertGreaterThan(actual, expected)   // fails if actual <= expected
assertLessThan(actual, expected)      // fails if actual >= expected
assertContains(collection, item)      // fails if collection doesn't have item
assertFails(expression)               // fails if expression does NOT fail with an error
assertPanics(expression)              // fails if expression does NOT panic
```

**`assertFails` and `assertPanics` are separate** because they catch different kinds of failure:

```
// assertFails — catches ONLY failures reported through errors
let error = assertFails(createUser({ email: `not-an-email` }))
assert(error.message.contains(`invalid email`))

// assertPanics — catches ONLY panics
let panic = assertPanics(averageScore(totalScore, 0))
assert(panic.message.contains(`division by zero`))
```

A *panic* is a crash that stops the running task when your code has a bug, such as dividing by zero. `assertFails` catches only errors, and `assertPanics` catches only panics. If a typo in your own test code causes a panic, it always reaches the test runner with the full list of calls that led to it, so you see exactly where the test broke.

Both `assertFails` and `assertPanics` return the failure they caught so you can look at it. It has `.message`, `.trace` (the list of calls that led to the failure), and `.source` (`{ file, line }`).

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
        let schema = loadSchema(`users`)
        assert(schema.valid() == true)

      Hint: loadSchema() returned a schema that failed validation —
            check the test fixture or the schema file itself.

Tests: 46 passed, 1 failed, 47 total
Time:  0.4s
```

The output uses plain English, points to the exact line, and suggests what might be wrong, like compiler error messages do.

---

## Running tests

```
ynz test                    // run all tests in the project
ynz test players            // run tests matching `players` (in file path, group name, or description)
ynz test --watch            // rerun automatically when a file changes
ynz test --serial           // run everything one at a time (to track down flaky tests)
ynz test --parallel N       // run at most N files at the same time (default: the number of CPU cores)
```

By default, **files run at the same time** as each other for speed, and **the tests inside one file run one after another**, because they often share setup and teardown. A *flaky* test passes sometimes and fails sometimes. If you suspect one, or a clash between tests running at the same time, run with `--serial`.
