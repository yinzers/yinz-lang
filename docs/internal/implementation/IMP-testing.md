---
name: "IMP-testing"
description: "Design decisions for Yinz's built-in test runner: the 'test' keyword with string descriptions, standalone assertion functions, and file-scoped setup/teardown."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-01"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Testing — Design Decisions

No user-facing reference page yet: `ynz test` and the `test` keyword are not shipped. Their milestone (v0.13) is in [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md).

---

## Built-In, Not a Framework

Testing is part of the language and compiler. No `npm install`, no framework choice, no configuration files for the test runner.

**Why**: Framework fatigue is real. Every JavaScript project starts with "which test framework?" — Jest, Vitest, Mocha, Jasmine, tap. Built-in testing eliminates the choice, eliminates the config, and means `ynz test` always works in any project.

**Precedent**: Go's `go test`, Rust's `cargo test`, Zig's `zig test`. Compiled languages that build testing in get better adoption because starting a project with tests has zero friction.

**Status: designed, not built.** The `test` keyword is reserved in the parser, so test code parses; the runner (`ynz test`), `setup` / `teardown`, `group` and the assertions below are not built. Planned for v0.13 per [`docs/reference/REF-mvp-scope.md`](../../reference/REF-mvp-scope.md). Everything below this line is the design for that release.

**Test files** end in `.test.ynz` and live next to the code they test (`models/player.ynz` beside `models/player.test.ynz`). A test file imports what it tests with the usual backtick import path.

**The command line.** `ynz test` runs every test in the project; `ynz test players` runs the tests whose file path, group name or description contains `players`; `ynz test --watch` reruns when a file changes; `--serial` and `--parallel N` are described under "Parallelization" below.

---

## ``test `description` { }`` Keyword

Test functions use a dedicated `test` keyword with a backtick string description (Yinz has one string form), not a function named `testSomething` or a callback passed to a `describe()` function.

**The name is a string description**: readable English sentences produce readable output, and ``test `new player has full health` `` can be read by non-developers.

**`test` is its own keyword, not `function`**: test functions are discovered and run by the test runner, not called by other code. `test` reads like English: "test: new player has full health."

---

## Assertions as Functions, Not Methods

`assert(condition)`, `assertEqual(actual, expected)` — standalone functions, not chained methods. The designed set:

```ynz
assert(condition)                     // fails if condition is false
assertEqual(actual, expected)         // fails if not equal — shows the difference
assertNotEqual(actual, expected)      // fails if equal
assertGreaterThan(actual, expected)   // fails if actual <= expected
assertLessThan(actual, expected)      // fails if actual >= expected
assertContains(collection, item)      // fails if collection doesn't have item
assertFails(expression)               // fails if expression does NOT fail with an error
assertPanics(expression)              // fails if expression does NOT panic
```

`assertEqual(player.health, 100)` reads like an English instruction and follows the language's step-by-step-over-chaining rule (Golden Rule 9).

---

## Setup and Teardown — `setup` / `teardown` keywords

Per-test setup runs before each test; per-test teardown runs after each test. Per-file scope uses the `file` modifier — runs once per file regardless of test count.

```ynz
setup file {
  db.connect(`test-database`)
}

teardown file {
  db.disconnect()
}

setup {
  db.clearTables()                // runs before EACH test in this file
}

teardown {
  // runs after each test
}

test `creates a user` {
  // ...
}
```

**Scope hierarchy:** file → group → test.
- `setup file` runs once before any test in the file
- `setup` inside a `group` runs before each test in that group
- `setup` at file level (outside a group) runs before each ungrouped test AND before each test inside any group
- `teardown` mirrors `setup` semantics in reverse order

**Why `setup` / `teardown` over `beforeEach` / `afterEach`**: Plain English. Matches Golden Rule 5 (human-readable over jargon). A jr dev reads "setup the test environment" / "tear down the test environment" without needing prior context.

**Why parameterless in v0.13**: Most tests don't need test metadata (current test name, etc.) inside setup. Adding an optional `setup(testInfo) { ... }` form can come later if real use cases surface — it doesn't break the parameterless default.

---

## Groups — Optional, Single-Level, No Nesting

Tests can be grouped with ``group `name` { ... }`` blocks. Groups are single-level only — nesting is a compile error:

```ynz
group `outer` {
  group `inner` {        // COMPILE ERROR: Cannot nest groups.
    // ...               //                Use one level per file. Split into
  }                      //                multiple test files if needed.
}
```

```ynz
group `user creation` {
  setup {
    db.clearTables()
  }

  test `creates a user with valid input` { ... }
  test `rejects invalid email` { ... }
}

group `user queries` {
  setup {
    db.clearTables()
    db.seed(`users`, testFixtures)
  }

  test `finds user by id` { ... }
}

test `schema validation passes` {           // ungrouped, at file level
  // ...
}
```

**Why single-level**:
- **Rule 9 spirit** — Yinz rejects deep method chaining because nested code is hard to scan. Same applies to test structure. Three-level-nested describes in Mocha files are notorious.
- **Setup/teardown scope is unambiguous** — file → group → test. Three clear layers. With nestable groups, "which setup runs?" becomes a question that requires reading the whole file.
- **Yinz isn't trying to be Jest** — JS test frameworks went nestable because their AST is flexible and the community wanted maximum structure. Yinz is opinionated. One level is enough for any properly-sized test file; if you need more structure, split into multiple files.

**Why allow groups at all (vs file-as-group only)**: Real test files often have 2-3 logical sections (creation, queries, deletion). Forcing one-group-per-file means artificial file proliferation or no logical grouping anywhere. Optional groups give the structure without forcing it.

**Filter behavior (`ynz test FILTER`):** Substring match against file paths, group names, AND test descriptions. All matching tests run; no count limit.

---

## `assertFails` and `assertPanics` — Separate Concerns

Two distinct assertions, each tight in what they catch:

```ynz
// Catches ONLY errors-system failures from the wrapped expression.
// Panics propagate normally.
let error = assertFails(createUser({ email: `not-an-email` }))
assert(error.message.contains(`invalid email`))

// Catches ONLY panics. errors-system failures propagate normally.
let panic = assertPanics(averageScore(totalScore, 0))
assert(panic.message.contains(`division by zero`))
```

**Why two functions instead of one:**

A single `assertFails` that caught BOTH errors and panics would silently capture test bugs (a panic from a typo in the test code) and count them as "yes it failed as expected." Test passes; production bug ships. **Bad.**

By separating: panics from test bugs ALWAYS propagate to the test runner with full stack trace. The dev sees exactly where the test broke. Debuggability stays high.

**Return shape (both):**
- `.message` — error or panic message
- `.trace` — call path
- `.source` — `{ file, line }` of origin

**Test functions do NOT need to be marked `errors`.** `assertFails` handles its own scope — it suppresses auto-propagation for the wrapped expression only. The test function itself remains a normal function.

**Compile error for unhandled `errors` calls in tests:**

```ynz
test `should fail on bad input` {
  let user = createUser({ email: `bad` })   // createUser is errors
  // COMPILE ERROR: createUser() can fail but this test is not handling the error.
  //                Either wrap it in assertFails() to verify the failure:
  //                  let error = assertFails(createUser({ email: `bad` }))
  //                Or handle the error explicitly with .failed() / .or().
}
```

**Typed error matching** (`assertFails(expr, ValidationError)`) deferred until Yinz has typed error variants. For now, message-based inspection covers what tests need.

---

## Parallelization — Files Parallel, Tests Sequential

By default:
- **Files run in parallel** — `users.test.ynz` and `orders.test.ynz` execute on separate threads simultaneously
- **Tests within a file run sequentially** — setup/teardown ordering depends on this

**Flags:**
- `ynz test --serial` forces all-serial execution (one test at a time, one file at a time) — debugging aid for flaky tests
- `ynz test --parallel N` caps parallel file count at N (default: number of CPU cores)

**Why files-parallel + tests-sequential is the default:**

- The file is the natural parallelism unit (per group scope decision above)
- Within-file parallelism would race on `setup`/`teardown` per-test ordering
- Files naturally have isolated state (each `setup file { db.connect(...) }` opens its own connection)
- Auto-parallelization (v0.3) doesn't apply directly — it analyzes pure-code dependencies, but tests touch external state the compiler can't see

**Deferred to v0.14+ (NOT v0.13):**
- `parallel file` declaration at top of file to enable within-file parallelism
- ``sequential `resource-name` `` declarations to serialize files sharing a resource

For v0.13, users are responsible for test isolation — the runner does NOT auto-sandbox files (no isolated DB schemas per file, etc.). The typical pattern is for `setup file` to scope its own state. Refinement features ship if real demand surfaces.

---

## Output Philosophy — Compiler-as-Teacher

Test failure output shows the exact line, the diff, and a hint about what might be wrong. Same philosophy as compiler error messages — every diagnostic follows the WHAT / WHAT-INSTEAD / WHY format per [`docs/reference/REF-teaching-mission.md`](../../reference/REF-teaching-mission.md).

**Why**: A test that fails with "AssertionError" and a stack trace is useless for a junior developer. A test that shows `Expected: 70, Actual: 100` and suggests "did you pass player correctly?" teaches at the moment of failure.

Output mirrors source structure (file → group → tests):

```
users.test.ynz
  user creation
    ✓ creates a user with valid input         (3ms)
    ✓ rejects invalid email                   (1ms)
  user queries
    ✓ finds user by id                        (2ms)
  ✓ schema validation passes                  (1ms)

Tests: 47 passed, 0 failed, 47 total
Time:  0.3s
```

A failing test prints the expected and actual values, the failing line and a hint:

```
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
```
