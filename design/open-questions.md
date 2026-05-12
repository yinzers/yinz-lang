# Open Questions

Design decisions that haven't been made yet. When one is resolved, move it to its dedicated design file and remove the entry here.

Resolved questions are NOT listed here — see `design/decisions.md` for the index of resolved topics.

Deferred features (decided not to ship in v0.1) are NOT listed here either — see `design/deferrals.md` for the ledger.

---

## Standard Library Scope

What's built in for v0.1 vs what gets deferred to v0.2+. Minimum expected for v0.1:
- File I/O
- HTTP client (server can be v0.2)
- JSON parsing
- String utilities (split, trim, pad, etc.)
- Math functions
- Basic date / time
- Random numbers

Full v0.1 scope TBD — see `design/stdlib/overview.md`. Things tagged v0.2+ are listed in `design/mvp-scope.md`.

---

## Test Setup and Teardown

No `beforeEach`/`afterEach` designed yet for `.test.ynz` files.

Options:
- `setup { }` and `teardown { }` blocks at the top of a test file
- A `group "name" { setup { } test "..." { } }` grouping block
- No built-in setup — tests call regular functions manually (simple, but verbose)
- `before { }` and `after { }` blocks per-test or per-file

---

## `assertFails` Semantics

`assertFails(expression)` is intended to verify that an expression errors. But the `errors` system uses auto-propagation. Open questions:
- Does `assertFails` work on calls to `errors` functions?
- Does it require the test to be in an `errors` context?
- What exactly triggers "fails" — a propagated error, a runtime panic, or both?
- How does it capture and inspect the error (message, type)?

---

## Test Grouping

No `describe` block shown in the test spec. Questions:
- Can tests be grouped with a `group "name" { ... }` block?
- Is a file the unit of grouping?
- Can filter (`ynz test players`) match group names as well as test description strings?

---

## Test Parallelization

Do tests run concurrently? The auto-parallelization system would try to run independent tests at the same time.

- Tests within a file — parallel or sequential?
- Tests across files — parallel?
- Should tests be guaranteed sequential by default (safer for tests with shared state)?
- Opt-in parallel: `test "description" parallel { ... }`?

---

## `process` Module — stdlib Design Needed

`process.exit(code)` is referenced in `spec/main.md` but the `process` module is undesigned. Open questions:
- `process.exit(code: int)` — exit with code
- `process.pid` — current process ID
- `process.env` — environment variable access (vs the standalone `env.get()` already designed)
- `process.args` — raw argument access (vs `cli.args()`)
- Signal handling beyond `setShutdownHandler()`
- Standard exit codes (0 = success, 1 = general failure — what else?)

---

## `running()` Built-In for Background Tasks

The linting spec references `while (running())` as the recommended pattern for graceful shutdown in long-running background tasks. `running()` presumably returns `false` when a shutdown signal is received.

Open questions:
- Is `running()` a built-in function, a keyword, or a standard library function?
- What signals set it to false? SIGTERM? SIGINT? All shutdown signals?
- Does it work outside background tasks? In `main()` loops?
- How does it interact with `setShutdownHandler()`?

---

## Built-In Linting Rules

The spec mentions duplicate/reusable code detection. What other rules should the compiler enforce?
- Dead code warnings
- Unused variables
- Function complexity limits (max lines, nesting depth)
- Naming conventions enforcement
- Unused `follows` declarations

---

## Compiler Error Format — Full Spec

The spec shows example compiler error messages. The exact format, structure, and tone of all compiler errors needs a full spec of its own before implementation.

Two concrete pieces of work, both needed:

**(A) Write the error-message style rule.** A dedicated file (likely `design/compiler-errors.md` or section in `design/linting.md`) that pins down:

- **No programmer jargon.** Words like "propagate," "narrow," "discriminator," "monomorphize," "infer," "polymorphic," "covariant" are banned from user-facing error messages. Use plain-English equivalents:
  - "propagate" → "let the error pass up to the caller" or "bubble up"
  - "narrow" → "the compiler now knows it's a [type]"
  - "discriminator" → "the tag that says which kind it is"
  - "infer" → "figure out automatically"
  - "polymorphic" → "works with any type"
- **Test a jr dev can read it.** Every error message should be readable by a developer who just graduated high school, knows JavaScript, and has never done systems programming. If a sentence requires a CS degree to parse, rewrite it.
- **Visual structure** (header, location, source snippet with arrows, suggestion, optional reference link)
- **Tone guide** — Cortana-friendly? Strictly factual? Encouraging? (Probably plain-English-helpful, not personality-driven.)
- **When to suggest fixes vs explain the rule** — most errors should suggest at least one concrete fix
- **Multi-error reporting strategy** (always show all, or stop after N?)

**(B) Audit existing spec and design docs for error-message examples and rewrite any jargon.** Every example compiler error message in `spec/**/*.md` and `design/**/*.md` files needs review:
- Sweep all error message examples
- Flag every instance of programmer jargon
- Rewrite in plain English following the rule from (A)
- Cross-reference: error messages that appear in multiple files should be consistent

This is the meta-rule: **the compiler's character as a teacher (Rule 11) is set by its error messages. If those use jargon, the language fails its own promise — regardless of how good the syntax design is.**
