# Testing — Design Decisions

User spec: `spec/testing.md`

---

## Built-In, Not a Framework

Testing is part of the language and compiler. No `npm install`, no framework choice, no configuration files for the test runner.

**Why**: Framework fatigue is real. Every JavaScript project starts with "which test framework?" — Jest, Vitest, Mocha, Jasmine, tap. Built-in testing eliminates the choice, eliminates the config, and means `ynz test` always works in any project.

**Precedent**: Go's `go test`, Rust's `cargo test`, Zig's `zig test`. Compiled languages that build testing in get better adoption because starting a project with tests has zero friction.

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

## Output Philosophy — Compiler-as-Teacher

Test failure output shows the exact line, the diff, and a hint about what might be wrong. Same philosophy as compiler error messages.

**Why**: A test that fails with "AssertionError" and a stack trace is useless for a junior developer. A test that shows `Expected: 70, Actual: 100` and suggests "did you pass player correctly?" teaches at the moment of failure.

---

## Open Questions

- **Test setup/teardown**: No `beforeEach`/`afterEach` designed yet. Should there be a `setup { }` and `teardown { }` block inside a test group? Or do tests just call regular functions manually? See `design/open-questions.md`.
- **`assertFails` semantics**: How does `assertFails(expression)` interact with the `errors` system? Does it catch auto-propagated errors? Does it require the expression to be in an `errors` context? See `design/open-questions.md`.
- **Test grouping**: Can tests be grouped? No `describe` block shown. Is a file the unit of grouping?
- **Test parallelization**: Do tests within a file run in parallel? Across files? Configurable?
