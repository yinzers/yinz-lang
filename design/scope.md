# Scope — Design Decisions

User spec: `spec/scope.md`

---

## Block Scoping Only

No variable hoisting. Variables exist only in the block where they're declared.

**Why**: Variable hoisting (`var` in JavaScript) is a reliable source of confusion — a variable can be used before its declaration and still works, just with `undefined`. Block scoping (`let`/`const` in ES6, Rust, Go, Swift, C#) matches how developers actually reason about code. See a `let`, know exactly where the variable lives. No surprises.

---

## No Mutable File-Level Variables

File-level `let` is a compile error. Only `const` is allowed at file level.

**Why**: Mutable global state creates hidden dependencies between functions, makes execution order-dependent, and breaks concurrency (any two functions could race on a shared mutable variable). File-level `const` is safe — it never changes, so any number of functions can read it simultaneously with no issues. The ownership system prevents races inside function bodies; prohibiting mutable globals prevents races at the module level.

---

## Const Expressions — Compile-Time Evaluation

File-level constants can use pure function calls with constant arguments. The compiler evaluates them at compile time. Runtime-dependent calls are rejected.

**The rule**: Can the compiler know this value before the program starts?
- Pure math (`math.PI * 100 * 100`) — yes
- Pure stdlib functions with constant args (`duration.seconds(5)`) — yes
- String literals and operations — yes
- `date.now()` — no, depends on when the program runs
- `file.read("config.ynz")` — no, I/O at compile time would be wrong

**Why allow pure function calls at file level**: `const RESPAWN_TIME = 5` is less clear than `const RESPAWN_TIME = duration.seconds(5)`. The compiler can evaluate the latter, so there's no reason to restrict to raw literals. The rule is principled — not "only literals" but "only values the compiler can determine."

---

## Export for Cross-File Sharing

Constants are shared across files using the existing `export`/`import` system. No special global keyword, no global registry.

**Why**: Same mechanism as everything else in the module system. One concept, one way to do it.
