# Linting

The compiler catches problems at three levels. Errors block compilation. Warnings compile but flag something to fix. Suggestions are informational — shown in the IDE, not in the terminal by default.

The philosophy: catch real bugs, enforce code quality, don't police style. Every rule prevents an actual problem — nothing is enforced just because "that's how it's usually done."

**Note on the examples below:** This file is a catalog — each example is abbreviated to fit a one-or-two-line summary of the rule it shows. The actual compiler output includes the full three-part diagnostic (WHAT happened, WHAT to do instead, WHY) per `design/compiler-errors.md`. So when you see `ERROR: Cannot add to a fixed array` here, the real output also shows the suggested fix (`Use array<number> if it needs to grow`) and the reason behind the rule.

---

## Errors — code won't compile

These are things that would definitely break at runtime or violate a language guarantee:

**Ownership violations:**
```yinz
let data = loadData()
consume(data)            // compiler infers `give` from consume's signature
print(data)
// ERROR: data was transferred to consume() above (consume takes `give data: Data`).
//        It no longer exists here. Use .copy() if you need to keep it.
```

**Missing return paths:**
```
function getName(share player: Player) -> string {
  if (player.active) {
    return player.name
  }
  // ERROR: not all code paths return a string.
}
```

**Type mismatches:**
```
let count: number = "hello"
// ERROR: expected number, got string.
```

**Unhandled errors in a non-errors function:**
```
function load() -> Config {
  let raw = file.read("config.txt")
  // ERROR: file.read() can fail but load() is not marked errors.
  // Handle the error or add "errors" to this function.
}
```

**Unhandled maybe:**
```
let user: maybe User = findUser(id)
print(user.name)
// ERROR: user is maybe User — value might be none.
// Use user.or(default).name or check user.exists() first.
```

**Mutating a const:**
```
const name = "Patrick"
name = "Alice"
// ERROR: name is const and cannot be reassigned.
```

**Mixed types in a collection:**
```
let stuff = [42, "hello", true]
// ERROR: cannot mix types in a collection. All elements must be the same type.
```

**Adding to a fixed array:**
```
let nums: fixed<number> = [1, 2, 3]
nums.add(4)
// ERROR: Cannot add to a fixed array.
// fixed<number> is size-locked. Use array<number> if it needs to grow.
```

**Direct array indexing — use .get() instead:**
```
let item = items[5]
// ERROR: Direct index access is not allowed.
// Use items.get(5) for safe access — it returns maybe T and handles out-of-bounds.
```

All collection access uses `.get(index)` which returns `maybe T`. The compiler enforces that you handle the case where the index doesn't exist. No out-of-bounds crashes.

**Naming convention violations:**
```
function FetchUser() -> User { }
// ERROR: function names must be lowercase camelCase. Did you mean fetchUser()?

type player { }
// ERROR: type names must be PascalCase. Did you mean Player?
```

**Sharing with a background task:**
```yinz
background processData(data)
// ERROR: processData's signature says `share data: Data`, but background tasks
//        may outlive this function — sharing isn't safe across that boundary.
//        Change processData to take `give data: Data` (it will own the value)
//        OR call data.copy() to give it an independent copy.
```

---

## Warnings — compiles, but fix this

These are real problems that won't crash immediately but indicate something wrong:

**Unused variables:**
```
let temp = calculateSomething()
// WARNING: temp is declared but never used.
```

**Unused import:**
```
import { fetchUser, createUser } from "services/users"
// WARNING: createUser is imported but never used.
```

**Unused function parameter:**
```
function process(share user: User, share config: Config) -> nothing {
  print(user.name)
  // WARNING: config is never used in this function.
}
```

**Unused export:**
```
export function deleteUser(id: string) -> nothing errors { }
// WARNING: deleteUser is exported but never imported anywhere in the project.
```

**Unreachable code:**
```
function process() -> string {
  return "done"
  log("after return")
  // WARNING: unreachable code after return statement.
}
```

**Variable shadowing:**
```
let name = "Patrick"
if (condition) {
  let name = "Alice"
  // WARNING: name shadows the outer variable declared on line 1.
}
```

**Prefer const:**
```
let count = 42
// count is never reassigned
// WARNING: count is never mutated. Consider const count = 42.
```

**Empty error handling — silently swallowing an error:**
```
let data = fetchData(url)
if (data.failed()) {
  // WARNING: error caught but nothing is done with it.
  // Log it, return it, or handle it — don't silently ignore failures.
}
```

**Identical branches — both paths return the same value:**
```
if (active) {
  return defaultConfig
}
return defaultConfig
// WARNING: both code paths return the same value. The condition has no effect.
```

**Condition always true or false:**
```
const debug = false
if (debug) {
  // WARNING: condition is always false — this code never runs.
}
```

**Unnecessary `wait`:**
```
let user = fetchUser(id)
let perms = wait fetchPermissions(user)
// WARNING: wait is unnecessary here. fetchPermissions(user) depends on user
// and would auto-wait via the dependency graph. Remove wait for cleaner code.
```

**Recommended graceful-shutdown pattern (informational):**
```
while (true) {
  let job = jobQueue.next()
  job.process()
}
// SUGGESTION: This loop has no graceful shutdown path. For long-running
//             background tasks, prefer:
//   while (process.isRunning()) {
//     let job = jobQueue.next()
//     job.process()
//   }
//
// Why: process.isRunning() returns false on shutdown signals (SIGTERM,
//      SIGINT, SIGHUP), letting the loop exit cleanly before the process
//      terminates. Without it, the OS forcibly kills the process, which
//      can leave resources in a bad state.
```

---

## Suggestions — IDE only by default

These are informational. The IDE shows them. The terminal doesn't, unless you've enabled strict mode.

**Performance: consider fixed instead of array:**
```
let players: array<Player> = [p1, p2, p3]
// players never calls .add() or .remove()
// SUGGESTION: players is never modified after creation. Consider fixed<Player> for better performance.
```

**Performance: consider type instead of map:**
```
let stats: map<string, number> = { health: 100, attack: 50, defense: 30 }
// SUGGESTION: all keys are known at compile time. Consider defining a type for direct field access.
```

**Duplicate code:**
```
function calcBonus(share p: Player) -> number { return p.health * 2 }
function calcScore(share p: Player) -> number { return p.health * 2 }
// SUGGESTION: identical logic. Consider consolidating into one function.
```

**Large copy for background task:**
```
background processData(data)
print(data.count())
// SUGGESTION: data (~500MB) was copied for the background task.
// Move the .count() call above the background line to avoid the copy.
```

**Function complexity:**
```
function handleEverything(...) -> nothing {
  // SUGGESTION: cyclomatic complexity is high. Consider breaking this into smaller functions.
}
```

**Debug prints left in code:**
```
print("got here")
// SUGGESTION: debug print — remove before production?
```

---

## Configuring strictness

In `yinz.toml`:

```toml
[lint]
level = "balanced"               # "relaxed", "balanced", "strict"
treat_warnings_as_errors = false
```

**relaxed** — errors only. Nothing else. Good for exploratory code or very early prototypes.

**balanced** — errors + warnings in terminal, suggestions in IDE only. The default.

**strict** — all three levels in terminal, warnings treated as errors. Good for CI pipelines and mature codebases.
