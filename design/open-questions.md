# Open Questions

Design decisions that haven't been made yet. When one is resolved, move it to `decisions.md`.

---

## Error Handling — Auto-Propagation Detail

In an `errors` function, what exactly happens when you call an `errors` sub-function?

- **Option A (eager)**: The compiler auto-propagates immediately. Variables are narrowed to their success types directly. You cannot manually check `.failed()` inside an `errors` function — there's nothing to check.
- **Option B (lazy)**: Variables retain the error-capable type. Auto-propagation fires if you use the value without checking. Explicit `.failed()` checks work inside `errors` functions (useful for logging before propagating).

The spec's "handling explicitly" section shows `return content` inside what appears to be an errors function — this only makes sense under Option B. Needs resolution before the type checker can be implemented.

---

## Generic Functions

Can functions have type parameters?

```
// Is this valid?
function identity[T](give value: T) -> T {
  return value
}
```

Generics for types are defined. Generic functions are not yet designed.

---

## Concurrency / Async Model

**RESOLVED** — see `design/concurrency.md` for full design and `spec/concurrency.md` for user-facing spec.

Summary of what was decided:
- Auto-parallelization via compiler dependency graph analysis — no keywords needed for most code
- `wait` keyword for explicit ordering of side effects
- `background` keyword for fire-and-forget and long-running tasks
- Loops sequential by default (unbounded iteration count risk)
- Ownership (`share`/`lend`) determines read vs write classification — no separate I/O tagging
- Best-effort cancellation on error — in-progress ops complete, results discarded
- `.share` is a compile error with `background` (task outlives current scope)
- Compiler auto-infers `.give` or `.copy` for background tasks based on post-call usage
- IDE execution plan visualization is mandatory

Still open (not blockers):
- `await all(task1, task2, ...)` exact semantics
- Batch processing utilities for parallel loop patterns
- Database concurrency details → tagged MVP2 (see design/concurrency.md)

---

## GPU Dispatch — Syntax Detail

Core decision made: `gpu` as a call-site keyword prefix (consistent with `wait` and `background`). Full vision in `design/gpu.md`.

Still open:
- Which types are GPU-compatible? Probably only `float`, `int`, fixed arrays and tensors. Strings and maps likely not.
- Fallback behavior when no GPU is available — silent CPU fallback, or compile-time configuration?
- How does ownership transfer to/from GPU actually work at the ABI level?

---

## FFI Type Mapping

How C types map to Yinz types is not designed. Specific open questions:
- `void*` — raw pointer type in Yinz?
- `char*` / `const char*` — maps to `string`? Or a raw byte array?
- Struct pointers — wrap in a Yinz type?
- Function pointers — maps to a closure type?
- Integer sizes — `int32_t` vs `int64_t` vs C's `int`?

---

## `Iterable` Contract with Errors

Some iterators do I/O (`file.lines()`, paginated API clients). The current contract `next(lend self) -> maybe T` has no error path. Options:

- **Option A**: `next(lend self) -> maybe T errors` — for loops over fallible iterables require `errors` context
- **Option B**: Fallible iterators swallow errors internally (return `none` on failure, log the error)
- **Option C**: Two contracts — `Iterable[T]` infallible and `FallibleIterable[T]` for I/O

This needs resolution before `file.lines()` and paginated iterables can be spec'd correctly.

---

## Module / Package System

Questions:
- How are files organized? How are types/functions exported and imported?
- `import` syntax (familiar from JS)?
- How are packages published and versioned?
- What belongs in the standard library vs external packages?

---

## Standard Library Scope

What's built in? Minimum expected:
- File I/O
- HTTP client / server
- JSON parsing
- String utilities (split, trim, pad, etc.)
- Math functions
- Date / time
- Random numbers

Full scope TBD.

---

## Numeric Types

The spec uses `number` everywhere. Real systems code needs:
- Integer sizes (`i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`)
- Float precision (`f32`, `f64`)
- Overflow behavior (panic? wrap? saturate?)

How much of this is exposed vs handled by the compiler?

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

## FFI (Foreign Function Interface)

Referenced in the linting spec as a suggestion-level warning (`ffi.doSomething(data)` — compiler cannot analyze ownership or concurrency). FFI is completely undesigned.

Open questions:
- How do you call C/C++/Rust libraries from Yinz?
- How does the ownership system interact with foreign functions?
- What safety guarantees (if any) exist for FFI calls?
- Is FFI even in scope for v1, or is it a later version feature?

---

## String Indexing

The no-direct-indexing rule applies to collections. What about strings?

In most languages, `name[3]` gets the character at position 3. If the rule "no direct indexing" is universal, then `name.get(3)` would return `maybe char`. But there's no `char` type in the spec yet.

Options:
- Strings support `.get(index)` returning `maybe string` (single-character string)
- A `char` type is added, `.get(index)` returns `maybe char`
- Strings have separate character-access methods (`.charAt(index)` etc.)
- The rule applies only to collection types, not strings

---

## Deprecation Marking

The linting spec references deprecated standard library usage as a suggestion. This implies there's a way to mark something as deprecated. Undesigned.

If Yinz follows the no-backwards-compatibility-pre-release policy, deprecation only matters post-release. Tag for post-v1 design.

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
