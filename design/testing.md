# Testing — Design Decisions

User spec: `spec/testing.md`

---

## Built-In, Not a Framework

Testing is part of the language and compiler. No `npm install`, no framework choice, no configuration files for the test runner.

**Why**: Framework fatigue is real. Every JavaScript project starts with "which test framework?" — Jest, Vitest, Mocha, Jasmine, tap. Built-in testing eliminates the choice, eliminates the config, and means `ynz test` always works in any project.

**Precedent**: Go's `go test`, Rust's `cargo test`, Zig's `zig test`. Compiled languages that build testing in get better adoption because starting a project with tests has zero friction.

Ships in v0.13 per `design/mvp-scope.md`. The `test` keyword is reserved in the parser from v0.1 so existing test code parses; the runner ships at v0.13.

---

## `test "description" { }` Keyword

Test functions use a dedicated `test` keyword with a string description, not a function named `test_something` or a callback passed to a `describe()` function.

**Why a string description over a function name**: Test names that are readable English sentences produce readable output. `test "new player has full health"` produces output that non-developers can read. `func TestNewPlayerHasFullHealth(t *testing.T)` is Go-style verbosity. `it("should have full health", () => ...)` is callback soup.

**Why not `function`**: Test functions have different semantics — they're discovered and run by the test runner, not called by other code. A distinct keyword makes this clear. `test` reads like English: "test: new player has full health."

---

## Assertions as Functions, Not Methods

`assert(condition)`, `assertEqual(actual, expected)` — standalone functions, not chained methods.

**Why**: Method chaining on assertions (`expect(value).toBe(100)`) is clever but harder to read than `assertEqual(player.health, 100)`. Standalone assertion functions read like English instructions. Consistent with the language's preference for step-by-step over chaining.

---

## Setup and Teardown — `setup` / `teardown` keywords

Per-test setup runs before each test; per-test teardown runs after each test. Per-file scope uses the `file` modifier — runs once per file regardless of test count.

```ynz
setup file {
  db.connect("test-database")
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

test "creates a user" {
  // ...
}
```

**Scope hierarchy:** file → group → test.
- `setup file` runs once before any test in the file
- `setup` inside a `group` runs before each test in that group
- `setup` at file level (outside a group) runs before each ungrouped test AND before each test inside any group
- `teardown` mirrors `setup` semantics in reverse order

**Why `setup` / `teardown` over `beforeEach` / `afterEach`**: Plain English. Matches Golden Rule 12 (human-readable over jargon). A jr dev reads "setup the test environment" / "tear down the test environment" without needing prior context.

**Why parameterless in v0.13**: Most tests don't need test metadata (current test name, etc.) inside setup. Adding an optional `setup(testInfo) { ... }` form can come later if real use cases surface — it doesn't break the parameterless default.

---

## Groups — Optional, Single-Level, No Nesting

Tests can be grouped with `group "name" { ... }` blocks. Groups are single-level only — nesting is a compile error.

```ynz
group "user creation" {
  setup {
    db.clearTables()
  }

  test "creates a user with valid input" { ... }
  test "rejects invalid email" { ... }
}

group "user queries" {
  setup {
    db.clearTables()
    db.seed("users", testFixtures)
  }

  test "finds user by id" { ... }
}

test "schema validation passes" {           // ungrouped, at file level
  // ...
}
```

**Why single-level**:
- **Rule 7 spirit** — Yinz rejects deep method chaining because nested code is hard to scan. Same applies to test structure. Three-level-nested describes in Mocha files are notorious.
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
let error = assertFails(createUser({ email: "not-an-email" }))
assert(error.message.contains("invalid email"))

// Catches ONLY panics. errors-system failures propagate normally.
let panic = assertPanics(processUnchecked(nullReference))
assert(panic.message.contains("null"))
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
test "should fail on bad input" {
  let user = createUser({ email: "bad" })   // createUser is errors
  // COMPILE ERROR: createUser() can fail but this test is not handling the error.
  //                Either wrap it in assertFails() to verify the failure:
  //                  let error = assertFails(createUser({ email: "bad" }))
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
- `sequential "resource-name"` declarations to serialize files sharing a resource

For v0.13, users are responsible for test isolation — the runner does NOT auto-sandbox files (no isolated DB schemas per file, etc.). The typical pattern is for `setup file` to scope its own state. Refinement features ship if real demand surfaces.

---

## Output Philosophy — Compiler-as-Teacher

Test failure output shows the exact line, the diff, and a hint about what might be wrong. Same philosophy as compiler error messages — every diagnostic follows the WHAT / WHAT-INSTEAD / WHY format per `design/teaching-mission.md`.

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
