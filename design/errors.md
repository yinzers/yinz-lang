# Error Handling — Design Decisions

User spec: `spec/errors.md`

---

## `errors` Keyword — No try/catch, No Result Types

No try/catch. No Result types. No `?` operator. Functions that can fail declare `errors` after their return type. Auto-propagation in `errors` functions. Compile-time enforcement in non-errors functions.

**Why try/catch was rejected**: Separates error handling from the call site — you lose context. Error handling logic ends up far from the code that caused the error. Nested try/catch blocks are hard to read and easy to get wrong.

**Why Result types were rejected**: Require boilerplate at every level of the call stack. Every function that calls a fallible function must unwrap or propagate explicitly — the boilerplate multiplies through the codebase. Rust's `?` operator reduces this, but it's still a symbol with no plain-English meaning.

**Why `errors` works**: Keeps the contract at the function boundary with zero boilerplate inside `errors` functions. Auto-propagation handles the common case (let failures cascade to the caller). Explicit handling is required when the function doesn't auto-propagate, enforced at compile time.

**`maybe` vs `errors`**: Distinct concepts. `maybe T` = value might not exist (absence, not failure). `-> T errors` = function might fail (failure with a message and call trace). A function can have both: `-> maybe User errors` means "might fail AND might not find a user."

---

## Auto-Propagation: Flow-Sensitive Narrowing

Inside an `errors` function, calls to other `errors` functions return the error-capable form. The compiler decides whether to **auto-propagate** or **let the user handle the error** based on flow-sensitive analysis of how the variable is used.

**The rule:**
- If the user calls `.failed()`, `.message`, or other error-inspection methods on the variable BEFORE using the success value, auto-propagation is suppressed for that variable — the user has taken responsibility.
- If the user uses the success value (passes it to another function, reads a field, calls a function on it via UFCS) WITHOUT first checking the error state, the compiler inserts auto-propagation at that point and narrows the variable to its success type from there forward.

**Examples:**

```ynz
// Happy path — auto-propagation, error-capable type invisible
function loadConfig() -> Config errors {
  let raw = readFile("config.txt")           // raw briefly: string errors-capable
  let parsed = parseConfig(raw)              // ← first use of success value
                                             //   compiler auto-propagates raw here
                                             //   from this point: raw is plain string
  return parsed
}

// Explicit handling — user opts in by checking first
function loadConfigWithLog() -> Config errors {
  let raw = readFile("config.txt")
  if (raw.failed()) {                        // ← check first
    log.warn("config missing: " + raw.message)
    return Config.default()
  }
  return parseConfig(raw)                    // raw narrowed to string after the if
}
```

**Compile error when ordering is wrong:**

```ynz
function loadConfig() -> Config errors {
  let raw = readFile("config.txt")
  let parsed = parseConfig(raw)              // raw auto-propagated here
  if (raw.failed()) { ... }                  // ← compile error
}
// COMPILE ERROR: raw was auto-propagated on line 3 (parseConfig(raw)).
//                After auto-propagation, raw is narrowed to string and no
//                longer has .failed(). To handle the error before propagating,
//                move the .failed() check above the parseConfig() call.
```

**Why this design wins over alternatives:**

- **Pure Option A (eager auto-propagation, no `.failed()` inside errors functions):** Would break real systems code — you can't log-and-recover within an errors function. Forces every recoverable case to be a non-errors function, which pushes the complexity outward instead of containing it.
- **Pure Option B (lazy, variables retain error-capable type until manually unwrapped):** Forces users to think about "what's the type" at every fallible call. Heavier mental load. Goes against "happy path reads like the happy path."
- **This hybrid (lazy under the hood, eager in feel):** Pure Option B mechanics, taught to users as "auto-propagation happens by default, you can opt out by checking first." 99% of code reads as Option A; 1% gets Option B's flexibility.

**Implementation note:** This is flow-sensitive narrowing — the same machinery the type checker already uses for `maybe T` (after `if (item.exists())`, `item` narrows to `T`). Salsa handles flow-typing well. No new infrastructure needed.

**Same method set everywhere:** `.failed()`, `.message`, `.or(default)` work both inside and outside `errors` functions. One mental model, two contexts. The compiler picks behavior based on what the user wrote, not based on where they wrote it.

---

## Error Object Shape

When an error reaches a custom handler (`setErrorHandler`) or propagates to the top, it carries:

- `.message` — human-readable description (string)
- `.suggestions` — array of human-readable next steps (array<string>, may be empty)
- `.trace` — call path as structured data (`array<Frame>`)
- `.source` — a `SourceLoc` value (`{ file: string, line: maybe int }`) of the originating failure

`suggestions` is part of the base shape because the teaching-compiler philosophy applies at runtime too — not just compile time. An error that can tell you what to do next is better than one that just tells you what went wrong. Stdlib modules are expected to populate suggestions where the cause is known. User-defined errors may leave it empty.

---

## M7 Frame Shape (Locked)

```ynz
shape Frame {
  file: string
  line: maybe int
  function: string
}
```

**`line` is `maybe int`** — real frames carry a one-based positive int; the truncation-sentinel frame uses `none` to avoid a magic-number sentinel. ONE-BASED: matches compiler diagnostic line numbering exactly. Tools integrating with `.trace` must treat this as one-based; convert to zero-based at the tool boundary (e.g., LSP) rather than in the Frame value.

**Truncation sentinel**: when the frame stack overflows 1024 entries, an additional sentinel Frame is appended: `Frame { file: "<trace truncated at depth 1024>", line: none, function: "<...>" }`. The real frames before it are still present and useful.

---

## M7 SourceLoc Shape (Locked)

```ynz
shape SourceLoc {
  file: string
  line: maybe int
}
```

`SourceLoc` answers "where did this specific failure originate" — the leaf call site. `Frame` (with its additional `function` field) answers "who was in the call chain." They're distinct because `.source` is a single origin point; `.trace[0..N]` is the full path. `.source` doesn't carry a function name because that context is already available as `.trace[0].function` (the outermost frame's function).

---

## M7 Frame Stack Mechanism (Locked)

**Thread-local `array<Frame>` capped at 1024 frames.**

At the start of every `errors` function body: `ynz_frame_push(file, line, function_name)`.
Before every return path (success or early return): `ynz_frame_pop()`.
At `ynz_error_new()`: snapshot the current thread-local frame stack into the error's `.trace` array.

Auto-propagation re-uses the snapshot — no re-capture at each propagation site. The error carries the trace from where it was created.

**Overflow behavior:** the 1024th push records the last useful frame. The 1025th push and onward are dropped. When the error surfaces, an additional sentinel Frame with `line: none` is appended marking the truncation point. Silent truncation — the user still sees the top and bottom of the stack.

**Trace capture is compile-time emitted, NOT libunwind.** Each `errors` function gets a `static FRAME_INFO: FrameInfo = { file: __FILE__, line: __LINE__, function: __NAME__ }` global at the IR level; entry emits `ynz_frame_push(&FRAME_INFO)`. This keeps M7 hermetic from the system unwinder ABI.

**LLVM IR representation of Frame:** `{ file: *u8, line: { tag: i8, padding: 7 bytes, value: i64 }, function: *u8 }` = 32 bytes total (16-byte aligned). Uses M5's existing `maybe<int>` lowering `{ tag: i8, padding: 7 bytes, value: i64 }` — no new representation.

---

## M7 `.message` Access Guard (Safety Invariant)

`.message` (and `.suggestions`, `.trace`, `.source`) access requires the binding to be flow-narrowed to the "failed" state — i.e., inside the true-branch of an `if (x.failed())` check, or after an early-return on `!x.failed()`. Direct `err.message` without flow-narrowing is a compile error.

This uses M6's narrowing infrastructure extended with a new "errors-capable" narrowing fact (parallel to `maybe<T>`'s exists-fact). See `design/narrowing.md` for the added rows.

---

## Typed Stdlib Errors

Stdlib modules define first-class error types with domain-specific fields — not just string messages. Callers pattern-match on the error type to handle specific failure modes. No string parsing, no try/catch, no guessing.

Example from the db module:

```
shape DatabaseError {
    summary: string             // short human-readable description
    suggestions: array<string>  // what to do about it
    code: string | null         // raw driver error code (e.g. Postgres "23505")
    query: string | null        // sanitized query that failed
    detail: string | null       // full driver-level message
}
```

The principle: a caller receiving a `DatabaseError` can switch on `.code`, read `.summary`, display `.suggestions` — all without parsing a string. The error is data, not a message.

Every stdlib module that uses `errors` should define a typed error for its domain. The base error fields (`.message`, `.suggestions`, `.trace`, `.source`) are always present. Domain-specific fields are additive.

This is the runtime equivalent of the compiler's WHAT/WHAT-INSTEAD/WHY diagnostic format — applied to errors that happen while the program is running.

---

## Implementation Note (for M7)

The `errors` keyword, flow-sensitive auto-propagation, and cascades are scheduled for **M7** (`design/stdlib/database.md` is already written against this contract). When implementing M7:

- The base error shape above (`message`, `suggestions`, `trace`, `source`) must be the foundation the `errors` keyword surfaces to callers via `.message`, `.suggestions`, etc.
- Typed stdlib errors (like `DatabaseError`) are additive on top of the base — the `errors` keyword must support domain-specific error types, not just a single generic error shape.
- See `design/stdlib/database.md` → "Structured Runtime Errors" for the first concrete example of a typed stdlib error this design must support.
