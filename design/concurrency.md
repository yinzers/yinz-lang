# Concurrency — Design Decisions and Rationale

Full design document. Companion to `spec/concurrency.md` (user-facing).

---

## Core Design Decision — Auto-Parallelization via Dependency Graph

**Decision**: No async, no await, no promises, no goroutines, no manual thread management. The compiler automatically parallelizes independent operations by analyzing the dependency graph of your code.

**Why**: Every existing concurrency model requires explicit developer action for reads:
- async/await: developers must annotate calls and compose with `Promise.all()`
- goroutines: developers must create goroutines and channels manually
- threads: developers manage thread pools explicitly

Most developers don't bother parallelizing reads — they write sequential code and lose performance. Yinz makes parallelization the automatic default. Developers only act for the cases the compiler can't infer.

**How it works**:
1. Compiler builds a dependency graph from variable usage
2. Operations with no shared variable dependencies are scheduled concurrently
3. Operations that use the result of another operation automatically wait for it
4. Write ordering is inferred from ownership annotations (`lend` = write, `share` = read)

---

## Suspension vs. Ordering — What's Automatic and What `wait` Does (LOCKED 2026-06-05)

This is the authoritative statement of what `wait` means. Two different jobs were historically conflated — they are separate, and only the second is `wait`'s job.

**Suspension is automatic. You never write `wait` for it.** A call that can block (transitively reaches an I/O / may-block operation — in v0.3 that is `sleep`) is compiled into a suspension point by the compiler's whole-program may-block analysis (no function coloring — see `design/future/concurrency.md`). The function suspends and hands its thread back to the scheduler automatically. The IDE shows the inferred suspension as the muted `wait_points` hint. The user does **not** type `wait` to make a call suspend correctly — that shipped in v0.3-M2.

**Ordering is also mostly automatic.** The compiler orders operations it can prove are dependent:
- **Data dependency** — if B uses A's result, B waits for A. (No `wait` needed.)
- **Same-resource ownership** — two writes to the same `lend` target are sequenced. (No `wait` needed.)
- **Independent operations run concurrently** — reads, and writes to *different* resources, with no data dependency between them, auto-parallelize. This is the default and it is the maximal-performance choice.

```
// No data flows between these two calls — the compiler overlaps them automatically.
let user = fetchUser(a)
let orders = fetchOrders(b)
render(user, orders)     // waits for both; overlap is free, no user action required
```

**`wait` does exactly one thing the compiler cannot infer: it forces a causal order between operations that are otherwise independent.** Write `wait foo()` when `foo` must complete before the next statement runs even though they touch different resources and pass no value between them — a happens-before that lives in the outside world, not in the Yinz value graph.

```
// Different external services, no value flows between them →
// the compiler would otherwise run these concurrently. `wait` forces charge-before-email.
wait chargePayment(order)      // Stripe
sendConfirmationEmail(order)   // SendGrid — must not fire if the charge failed
```

**The observable difference between writing `wait` and not is ordering vs. overlap:**
- `wait foo()` then `bar()` → `foo` completes, then `bar` starts. Guaranteed order.
- `foo()` then `bar()` (independent) → `foo` and `bar` overlap; either may finish first.

If two operations are already dependent (data or same-resource), `wait` is redundant — they were ordered anyway. **`wait` only changes behavior for _independent_ operations.**

**Idiomatic note**: prefer a data dependency over `wait` when one exists. `let receipt = chargePayment(order)` then `sendConfirmationEmail(receipt)` orders the two via the threaded `receipt` and needs no `wait`. Reserve `wait` for the genuinely-no-value-to-thread, different-resource causal-ordering case (and for FFI, where the compiler can't see effects at all).

**Why this is the locked default (Model A)**: independent writes to different resources auto-parallelize (maximal throughput for write-heavy work), and the ordering responsibility for the residual causal cases is on the user via `wait`. The rejected alternative — preserving source order for *all* writes by default — leaves real throughput on the floor for intensive work, and the read-parallelism win survives either way. The accepted cost: independent side effects can race unless ordered, surfaced through the IDE concurrency hints. A superseded earlier proposal treated `wait` as non-ordering ("just I need the result here"); that is **dead** — in Yinz, `wait` IS ordering.

---

## Reads vs Writes — Ownership Does Double Duty

**Decision**: Use the existing `share`/`lend` ownership system to classify reads vs writes. No new annotations needed.

`share` in a function signature = function reads the resource. Multiple readers can run concurrently.

`lend` in a function signature = function writes to the resource. The compiler sequences writes to the same resource.

**Why this works**:

The ownership system already forces callers to declare intent at the call site. The compiler gets read/write classification for free — no special I/O tagging, no new concepts. The same rule that prevents data races also determines scheduling.

**Standard library tagging**:

Built-in stdlib functions are tagged internally:
- `file.write()`, `request.post()`, `request.put()`, `request.delete()` → writes
- `file.read()`, `request.get()` → reads

Multiple stdlib writes to the same resource auto-sequence. Writes to different resources can parallelize.

**User-defined functions**:

The compiler traces through user functions to determine if they contain writes. A function that calls `lend` on a resource is classified as a write for scheduling purposes.

---

## `wait` Keyword — Explicit Ordering

**Decision**: `wait` forces a function call to complete before execution continues. It is the user's tool for the one thing the compiler can't infer — a causal order between operations that are otherwise *independent* (different resources, no value threaded between them). See the authoritative "Suspension vs. Ordering" section above for the full model; `wait` is never needed for suspension (automatic) or for already-dependent operations (auto-ordered).

**When `wait` is necessary**:

Side effects that must be ordered but don't use each other's return values *and touch different resources* (so the compiler would otherwise overlap them):
```
wait chargePayment(order)      // Stripe — must complete before email
sendConfirmationEmail(order)   // SendGrid (different resource) — only starts after payment
```

External API calls where the compiler can't see the relationship:
```
wait request.post("stripe.com/charge", data)
request.post("email.com/send", receipt)
```

**When `wait` is NOT necessary**:

Data dependencies — if a function takes a value as an argument, the compiler already knows it must wait for that value. Explicit `wait` would be redundant.

**Honest estimate of `wait` usage**:

- **Reads → auto-parallelized. Zero `wait` needed.**
- **Writes to known stdlib resources → compiler auto-sequences. Zero `wait` needed.**
- **External API calls where order matters → `wait` needed.**
- **Fire-and-forget side effects in specific order → `wait` needed.**

The last two categories are real and common in backend code. The claim "most code never needs `wait`" applies to data-fetching code. Code with ordered I/O side effects will use `wait` regularly. This is intentional and acceptable — `wait` in Yinz replaces `await` in traditional models, with the advantage that reads NEVER need `wait`.

**Comparison to async/await**:

| Scenario | async/await | Yinz |
|---|---|---|
| Independent reads | Developer adds async/await | Automatic — zero effort |
| Ordered writes | Developer adds await | Developer adds wait — same effort |
| Write inference | Not possible | Compiler auto-sequences known writes |
| Fire and forget | Explicit handling required | `background doThing()` |
| Thread safety | Developer manages | Ownership prevents races at compile time |

Traditional models require explicit work for both reads and writes. Yinz only requires explicit work for ordered external side effects. Reads are always free.

---

## Loop Iterations — Sequential by Default

**Decision**: Loop iterations run one at a time, in order. This is the one exception to auto-parallelization.

**Why**:

Iteration counts are often unknown at compile time and potentially unbounded (3, 300, 300,000). Parallelizing thousands of iterations could overwhelm APIs, exhaust connection pools, or consume unbounded memory. The risk profile of unbounded concurrency is too high to default to parallel.

Independent statements in a function have a predictable, bounded count — typically 3 to 10. The compiler can see all of them statically. Loop iterations cannot be bounded statically.

The rule: bounded parallelism (independent statements) = auto-parallel. Unbounded parallelism (loop iterations) = sequential by default.

If a developer needs parallel loop processing, they'll use batch utilities from the standard library — an intentional advanced-use-case, not an accident.

---

## `background` — Two Patterns, One Keyword

**Decision**: `background` handles both fire-and-forget and long-running tasks. The developer's usage (store the handle or not) determines the pattern.

**Fire and forget** — don't store the handle:
```
background retryPayment(payoutId)
```

**Long-running** — store the handle, communicate via `.send()`/`.receive()`:
```
let monitor = background watchHealth()
monitor.send("get-status")
let status = monitor.receive()
```

**Why no separate keywords**: Two keywords for two patterns would require developers to decide upfront. The handle-or-not distinction is natural and already expresses the intent.

**MVP use case**: Fire-and-forget background tasks are the pragmatic MVP pattern. Production systems would eventually use a job queue (BullMQ, Sidekiq, etc.) with durability, retry logic, and observability. `background` bridges the gap for early-stage work.

---

## Ownership with Background Tasks — Why `.share` Fails

**Decision**: `.share` is a compile error for background tasks. Only `.give` (move) or `.copy` are valid.

**Why**:

A shared reference (`share`) is a borrow — it's only valid while the owner's scope is alive. A background task might still be running after the current function returns. If the current function's variables go out of scope while the background task holds a reference to them, the reference dangles. Yinz's ownership system disallows this at compile time.

**Why `.give` and `.copy` work**:

- `.give` transfers ownership to the background task. The original owner no longer exists — nothing can dangle.
- `.copy` creates an independent copy. The background task owns its copy, the caller owns the original. No shared reference.

**Compiler inference for ownership**:

The compiler auto-selects `.give` or `.copy` based on usage after the `background` call:

- Value NOT used after → compiler moves (`.give`). Zero copy overhead.
- Value IS used after → compiler copies (`.copy`). Caller keeps the original.

Developers can always override explicitly: `background process(data.give)` or `background process(data.copy)`.

The IDE warns on large copies so developers can restructure if needed.

| Scenario | Compiler does | Cost |
|---|---|---|
| Value unused after background | `.give` (move) | Zero |
| Value used after background | `.copy` (clone) | Copy cost |
| Small value (string, int) | `.copy` | Trivial |
| Large value, unused after | `.give` | Zero |
| Large value, used after | `.copy` + IDE warning | Developer restructures or accepts |
| Developer specifies `.give` | `.give` | Zero |
| Developer specifies `.share` | COMPILE ERROR | — |

---

## Error Cancellation — Best-Effort with Result Discard

**Decision**: When an error occurs during concurrent operations:
1. Operations not yet started → cancelled, never run
2. Operations already in progress → run to completion, results discarded
3. Resources cleaned up via ownership (handles going out of scope trigger destructors)
4. Function exits with the error

**Why best-effort, not hard-cancel**:

True hard-cancellation of in-flight I/O is platform-dependent and unreliable:
- HTTP requests: no universal cancel mechanism; socket interrupts work but leave the server unaware
- File reads: OS-level cancel is platform-specific
- DB queries: mid-query cancellation may leave locks or partial writes

Best-effort discard is pragmatic and honest: in-progress work finishes, results are never used. Resources are freed through ownership — when pending handles go out of scope, their destructors close sockets, release file handles, etc.

---

## IDE Execution Plan — Non-Negotiable

**Decision**: The IDE must visually show what the compiler is parallelizing. This is not an optional enhancement.

**Why**: Auto-parallelization is invisible magic without visibility. Developers can't debug what they can't see. The IDE makes compiler decisions visible without requiring the developer to express them in code.

What the IDE shows:
- Which operations run concurrently (grouped with indicators)
- Which operations are sequential (dependency-ordered)
- Where `wait` forces explicit ordering
- Where the compiler inferred write sequencing
- Hover: "Runs concurrently with lines X and Y. Waits for: user (line 2)."

---

## Database Writes — MVP2

**Status**: Databases are not part of the initial standard library. The concurrency rules for when we add them:

**Same connection = sequential** (compiler sees same `lend` target):
```
function saveOrder(lend db: Database, order: Order) -> nothing errors {
  db.insert("orders", order)           // sequential — same lend target
  db.insert("audit_log", order.log)    // sequential — same lend target
}
```

**Two connections = concurrent by default** (different `lend` targets):
```
function crossDbSync(lend db1: Database, lend db2: Database) -> nothing errors {
  db1.insert("users", user)            // concurrent with db2 — different lend targets
  db2.insert("users_backup", user)     // concurrent with db1
}
```

Use `wait` if ordering between connections matters:
```
wait db1.insert("users", user)
db2.insert("users_backup", user)
```

**Connection pool**: pool-managed connections are handled by the pool — the compiler doesn't control which physical connection each call gets. Pooled calls to the same logical database may parallelize. Transactions handle ordering within a connection at the DB level; the compiler's auto-sequencing aligns naturally with transaction behavior.

Full database stdlib design (connection pooling, transactions, query builder, migrations) is tagged MVP2.

---

## M3a Scope Boundaries — Deliberate Constraints and Deferrals

M3a lifted the `LocalCrossesWait` guard (scalars, shapes, strings, arrays can now cross a `wait`). Two cases were deliberately left as compile errors; one is deferred to a later milestone.

### `ShadowsCrossingLocal` — same name re-declared around a suspension (deferred, clean compile error today)

A `let` binding that re-uses a name that already has a crossing-local frame slot is a compile error today. The guard is **safe-conservative**: it rejects any program where two bindings share a name around a suspension boundary, even when one of them might technically be unreachable from the other.

Two shapes are rejected:

**Shape A — nested shadow**: an outer `let x` before a `wait` AND a `let x` inside a nested block (if/while/for/match body) in the same function, where the outer `x` is read after the suspension resolving to the outer binding.

```
// ❌ — x crosses the wait AND is re-declared inside the if
function broken() -> nothing {
  let x = 10
  wait sleep(5)
  if (someCondition) {
    let x = 20        // compile error — shadows crossing local x
    print(x.toString())
  }
  print(x.toString())
}

// ✅ — rename the inner binding
function fixed() -> nothing {
  let x = 10
  wait sleep(5)
  if (someCondition) {
    let innerX = 20
    print(innerX.toString())
  }
  print(x.toString())
}
```

**Shape B — top-level redeclaration**: an outer `let x` before a `wait` AND a second `let x` at the TOP LEVEL of the function body after the suspension. Even when all post-wait reads resolve to the redeclared binding (not the outer one), both top-level `let x` bindings share the same name-keyed frame slot — the second write clobbers the first, producing a silent wrong answer at runtime.

```
// ❌ — x=10 and x=99 both at top level, separated by a wait
function broken() -> nothing {
  let x = 10
  wait sleep(5)
  let x = 99     // compile error — two top-level bindings share one frame slot
  print(x.toString())
}

// ✅ — use distinct names
function fixed() -> nothing {
  let x = 10
  wait sleep(5)
  let xAfter = 99
  print(xAfter.toString())
}
```

The same two shapes apply to **parameters**: a parameter `p` occupies a frame slot at function entry.

- **Shape A (nested shadow)**: a `let p` inside any nested block (if/while/for/match body) is rejected in a suspending function. The frame-slot system keys every crossing local and parameter by NAME — a nested `let p` shares the parameter's name-keyed slot. Every continuation state's `reload_params_from_frame` overwrites `cg.locals[p]` with the slot pointer, which means even a non-crossing inner shadow would install the wrong alloca across the next suspension. All nested param shadows in suspending functions are therefore conservatively rejected until per-binding-ID slot allocation ships (M3c).
- **Shape B (top-level redeclaration)**: a `let p` at the TOP LEVEL of the function body shares the parameter's frame slot and is rejected regardless of whether the inner binding is read post-wait.

**Non-async functions**: a `let p` shadowing a parameter in a function that does NOT contain any `wait` is allowed — Yinz permits shadowing per `design/linting.md` (`shadowed-variables` Tier-3 lint). The conservative guard only applies to suspending (async) functions where parameters are frame-slotted.

**Why the guard is conservative (not precise)**: the frame-slot system maps each crossing local to a slot by NAME — one slot per unique name across the entire function body. A precise implementation would assign each `let` declaration a unique binding ID (keyed by source span or a monotonic counter), then allocate one slot per binding ID. The conservative guard rejects all same-name cases because it cannot distinguish two bindings that would slot correctly from two bindings that would collide. The workaround is always: use distinct names.

**What it costs to lift** (1–2 sessions): assign each `let` declaration a unique binding ID; key crossing-local frame slots by binding ID rather than name; propagate ID-keyed resolution through the flush/reload and typeck layers so the compiler can distinguish "same name, different slot" at every read and write site.

**Trigger**: user demand for re-using a name around a suspension in a program that cannot be restructured, OR when per-binding slot identity is added to the crossing analysis and codegen.

**Workaround** (always applies): rename any binding that re-uses a name already in use across a suspension boundary — two values with different semantics should have different names anyway (Golden Rule 2).

### `NestedShapeCrossing` — a shape with nested shape fields crossing a `wait` (deferred)

A `shape` whose fields are themselves `shape` types cannot cross a `wait` when those fields contain heap-allocated children (e.g. strings, arrays). The frame-embed codegen writes struct bytes directly into the composed frame; for nested shapes, this only copies the OUTER struct's bytes — any inner shape pointers that point into separately-allocated or stack regions become dangling after the suspension.

```
// ❌ (currently) — inner shape crosses a wait
shape Inner { value: int }
shape Outer { child: Inner, score: int }

function example() -> nothing {
  let o: Outer = { child: { value: 42 }, score: 100 }
  wait sleep(5)         // compile error — Outer.child is a nested shape
  print(o.score.toString())
}

// ✅ — flatten into primitive fields
shape FlatOuter { childValue: int, score: int }
```

**Why deferred**: the memcpy that stages the outer struct bytes doesn't recursively follow inner shape pointers. A correct implementation would either (a) walk the entire shape graph and embed all nested structs transitively in the frame, or (b) heap-allocate inner shapes and store pointers (with a drop guard). Both require non-trivial additions to the frame layout and drop subsystem. The flat-fields workaround always applies.

**What it costs to lift** (1–2 sessions): extend frame layout to recursively compute embedded nested-shape slot regions; add recursive memcpy at definition and reload sites; add a drop-on-cancel path for any heap-allocated inner shapes. Each of these is a well-contained change, but they need to be consistent across the frame-layout, codegen, and runtime layers.

**Trigger**: a user program that requires crossing a nested-shape local without a flatten workaround.

### `WideValueSuspendingReturn` — shape returns from suspending functions (deferred, clean compile error today)

A suspending function (one whose body contains a `wait`) cannot yet return a `Shape` or `Shape errors` value by value. These two return types require a variable-size EC/shape return-staging slot entangled with the pre-existing non-suspending shape-return base bug.

**`-> number errors` is fully supported** as of M3a Phase 1. A 16-byte staging slot is reserved in the composed frame (after own-local slots, before child sub-frames) when the function returns `-> number errors`. The resume function writes the i128 decimal to that slot, points the EC ok-word at it, and the staging slot is freed when the frame drops (alloc=1/free=1 — no leak). See the frame-layout comments in `crates/ynz-codegen/src/emit.rs` `build_frame_layouts`.

**Why each remaining variant fails without the guard**:

- **`-> Shape`** (non-crossing shape literal or call result): the old codegen staged the shape bytes at `FRAME_OFFSET_LOCALS_START` (frame offset 32). Offset 32 is where child sub-frames are embedded — writing there overwrites the sleep sub-frame's `resume_point` field, causing a `SIGSEGV` at the next `rt_async_sleep_poll` call.

- **`-> Shape errors`** (shape success value in an EC return): needs variable-size staging (shape size varies per declaration) and is also entangled with the pre-existing non-suspending `-> Shape` return base bug (shapes returned by value produce garbage for int fields even without suspension). Fixing the non-suspending bug first is the prerequisite.

**What IS supported** (verified clean in Phase 1):

| Return type | Suspending function | Status |
|---|---|---|
| `-> int`, `-> bool`, `-> float` | yes | CLEAN — scalar, no staging needed |
| `-> number` (plain) | yes | CLEAN — i128 stored directly in the 16-byte return slot |
| `-> number errors` | yes | CLEAN — i128 in 16-byte frame staging slot; alloc=1/free=1 |
| `-> int errors` | yes | CLEAN — `{err=0, ok=int_bits}` stored directly |
| `-> string`, `-> array<T>`, `-> map<K,V>` | yes | CLEAN — heap-stable pointer, `ptr_to_int` safe |
| `-> Shape` (crossing local) | N/A (frame-backed, not a return) | CLEAN — frame-embedded crossing locals work |

**Workarounds**:
- For `-> Shape`: return the shape's fields individually as primitives, or bind the shape to a crossing local and return a derived primitive.
- For `-> Shape errors`: return each field individually (e.g. `-> int errors`), or compute a primitive result inside the function and return that.

**What it costs to lift Shape/Shape-errors** (~1 session): fix the non-suspending `-> Shape` return base bug first (shapes returned by value silently produce garbage for int fields), then add variable-size staging to the frame layout for the suspending case. The staging region size is the shape's ABI size (computed by the shape-size table). The return-path GEPs to the region after the resume function memcpys the shape bytes there.

**Trigger**: a suspending function (body contains `wait`) whose declared return type is `Shape` or `Shape errors`.

---

### `UnsupportedCrossingLocalType` — types that cannot yet cross a `wait` (deferred, clean compile error today)

Several types cannot yet cross a `wait` boundary. The type classifier is complete for the supported set; any unhandled type produces a clean WHAT/WHAT-INSTEAD/WHY compile error rather than a silent miscompile, UAF, or SIGSEGV.

**Why each variant fails without the guard**:

The frame-slot classifier in codegen handles int, bool, float, number, string, array, map, Shape, and ErrorsCapable crossing locals. All others fall into buckets with safety problems:

- **`union` / `maybe<T>` / `dynamic Contract`**: internally represented as a pointer to a `{tag, payload}` struct alloca on the RESUME FUNCTION'S STACK. Flushing the alloca pointer to the frame slot and reloading it after the next resume stores and re-reads a dangling stack address. LLVM may detect this ("Instruction does not dominate all uses!") or silently corrupt the stack.

- **`fixed<T>` binding (crossing local)**: fixed arrays are stack-allocated allocas in the resume function's stack frame. When the resume function returns to the scheduler after a `wait`, that stack frame is freed. The crossing-local frame slot holds a dangling pointer — reading elements after resume is UB. (`fixed<T>` as a for-loop iterator is caught separately by `FixedArrayIterWithWait`.) Registry entry: `fixed-crossing-local-with-wait`.

- **`range` binding (crossing local)**: a range binding (`let r = range(0,3)`) is stored on the resume function's stack as a pair of bounds (i64 lo, i64 hi). When the function suspends the stack frame is freed; the crossing-local frame slot holds a dangling pointer on the next resume. Iterating that dangling range produces zero iterations — silent wrong output. The wait-inside-body form is caught separately by `StoredRangeWithWait`; this guard catches the case where the wait is at the TOP LEVEL before the loop. Registry entry: `range-crossing-local-with-wait`.

- **`MapEntry<K,V>` (for-loop var over a map)**: the loop variable in `for (entry in m)` is a `{key, value}` struct pair. The current frame-slot mechanism assigns ONE i64 slot per crossing local — fine for scalar/pointer types, but only covers `entry.key` (field[0]). `entry.value` (field[1]) has no slot and reads garbage after resume. Registry entry: `map-entry-fields-after-wait`.

**What IS supported** (crossing locals that work correctly):

| Type | Status | Frame strategy |
|---|---|---|
| `int`, `bool`, `float` | CLEAN | scalar i64/i1/f64 slot |
| `number` (decimal128) | CLEAN | 2 consecutive i64 slots (lo+hi) |
| `string`, `array<T>`, `map<K,V>` | CLEAN | heap-stable pointer stored as i64 |
| `Shape` (primitive fields only) | CLEAN | frame-embedded struct bytes |
| `T errors` (ErrorsCapable) | CLEAN | 2-slot {err, ok} struct |

**Blocked types and workarounds**:

| Type | Workaround |
|---|---|
| `union` / `maybe<T>` / `dynamic Contract` | Extract the inner value before `wait`; use the primitive after |
| `fixed<T>` binding (crossing `wait`) | Redeclare as `array<T>` — heap-allocated, pointer survives suspension |
| `range` binding (crossing `wait`) | Inline the range in the `for` header: `for (i in range(0,n))` |
| `MapEntry<K,V>` after `wait` | Read `entry.key`/`entry.value` before `wait`, bind to separate `let k`/`let v`, use those after |

**What it costs to lift each**:
- `union`/`maybe`/`dynamic`: per-type flush/reload strategies (store tag + payload fields to separate frame slots). ~half a session each.
- `fixed<T>` crossing local: embed the fixed-array bytes directly in the composed heap frame (similar to Shape frame-embedding), using compile-time size to compute slot count. ~1 session.
- `range` crossing local: store lo/hi bounds as two consecutive i64 frame slots and reconstruct the range alloca on reload. ~half a session.
- `MapEntry<K,V>`: two consecutive frame slots (mirroring the ErrorsCapable 2-slot path). ~half a session.

**Trigger**: a local of any blocked type that is declared before a `wait` and read after it.

---

### `ECWrapperResultCollection` — collecting the result of a `background`-spawned `-> T errors` task (deferred to M3b)

The standalone EC wrapper (emitted for `background`-spawned suspending `-> T errors` functions) reconstructs the `{i64, i64}` EC struct from the frame's return slot and then calls `free_frame`. For `-> number errors`, the ok-word in that struct points into the composed frame's 16-byte staging slot — a region freed by `free_frame`. The returned struct's ok-pointer is therefore invalid after the wrapper returns.

This is **safe in M3a** because the only reachable caller of the wrapper is `background` (fire-and-forget), which discards the EC result entirely without dereferencing the ok-pointer.

A caller that **collects** the result — reads the ok-word and uses the pointed-at value — must copy it BEFORE `free_frame`. Implementing that read-before-free + copy requires the scheduler to know whether a spawned task's result is collected or discarded, and when the collection happens relative to the frame lifetime. That is M3b background result-collection machinery.

**Workaround**: use the inline-poll path — a suspending caller that calls another `-> T errors` suspending function composes the callee inline via the state-machine resume path, and the inline path is correct and complete. Only `background` hits the standalone wrapper; avoid collecting `background` handle return values for `-> T errors` spawns until M3b.

**What it costs to lift** (~half a session inside M3b): when the scheduler runs the wrapper function to completion, if the spawned task's result handle is collected, read the EC struct before freeing the frame, copy the ok-value to a heap buffer, update the ok-word to point to the heap buffer, then free the frame. The copy is conditional on whether the handle is collected — discarded handles skip it.

**Trigger**: storing or using the return value of a `background`-spawned suspending function whose declared return type is `-> T errors`.

This is tracked in `registry/features.toml` as `ec-wrapper-collect-on-completion`.

---

### `FixedArrayIterWithWait` — `fixed<T>` array iterator in a suspending for-loop (deferred, clean compile error today)

A `for` loop that contains a `wait` cannot use a `fixed<T>` array as its iterator.

```ynz
// COMPILE ERROR (FixedArrayIterWithWait):
let flags: fixed<boolean> = [true, false, true]
for (b in flags) { wait sleep(5) }

// WORKS: use array<T> instead
let flags: array<boolean> = [true, false, true]
for (b in flags) { wait sleep(5) }
```

**Why**: `fixed<T>` arrays are stack-allocated (`build_alloca([N x i64])`) in the current resume-function's stack frame. The crossing-local mechanism stores the array's pointer in the composed heap frame slot via `ptr_to_int`. When a `wait` suspends the function and the resume function returns to the Tokio scheduler, the resume function's stack frame is freed. On the next resume call, the pointer is reloaded from the heap frame slot via `int_to_ptr`, but it now points to freed stack memory. Reading the array elements after the first suspension produces undefined behavior — silently wrong values or memory corruption.

**Workaround**: use `array<T>` instead. `array<T>` is heap-allocated via `ynz_array_new` and its pointer remains valid after suspension.

**What it costs to lift** (~one session): two options: (a) heap-allocate `fixed<T>` when used as a for-loop iterator in a suspending function (changes semantics — fixed arrays would no longer be unconditionally stack-allocated); (b) embed the fixed-array bytes directly in the composed heap frame (similar to Shape crossing-local embedding), using the array size at compile time to compute the slot count. Option (b) is architecturally cleaner but requires computing `size * sizeof(element)` slots and storing bytes in consecutive frame slots.

**Trigger**: a `for` loop whose body contains a `wait` and whose iterator resolves to `Type::BuiltinFixed` — either a variable annotated as `fixed<T>` or an inline `[...]` literal in a `fixed<T>` context.

This is tracked in `registry/features.toml` as `fixed-array-iter-with-wait`.

---

### `StoredRangeWithWait` — stored range variable as a suspending for-loop iterator (deferred, clean compile error today)

A `for` loop that contains a `wait` cannot yet use a stored range variable as its iterator.

```ynz
// COMPILE ERROR (StoredRangeWithWait):
let r = range(0, 3)
for (i in r) { wait sleep(5) }

// WORKS: inline the range in the loop header
for (i in range(0, 3)) { wait sleep(5) }
```

**Why**: the state-machine codegen for `for (i in range(...))` calls `extract_range_bounds(iter)`, which requires the iterator to be a literal `range(...)` call expression. For a stored range variable (`let r = range(...); for (i in r)`), the bounds must be recovered from the range's frame-backed alloca. That recovery path is not yet implemented; without the guard the codegen would ICE on "for-loop iter is not a call expression".

**Workaround**: inline the range directly: `for (i in range(0, n)) { ... }`. If `n` is a local that crosses a `wait`, it is already frame-backed and the inline form works without any changes.

**What it costs to lift** (~one session): in the SM range arm of `lower_sm_for`, detect when `iter` is an `Expr::Ident` with a `Type::Range` type, load the start and end from the range struct alloca in the frame (using `build_struct_gep` on the `{i64, i64}` range alloca), and use those values as the loop bounds. The rest of the range-loop SM codegen applies unchanged.

**Trigger**: a `for` loop whose body contains a `wait` and whose iterator is a variable with `Type::Range` — e.g. `let r = range(0,3); for (i in r) { wait sleep(1) }`.

This is tracked in `registry/features.toml` as `stored-range-with-wait`.

---

### `ExpressionIterWithWait` — call-expression iterator in a suspending for-loop (deferred, clean compile error today)

A `for` loop that contains a `wait` cannot yet use a call expression as its iterator.

```ynz
// COMPILE ERROR (ExpressionIterWithWait):
for (x in makeArray()) { wait sleep(5) }

// WORKS: bind the collection first
let items = makeArray()
for (x in items) { wait sleep(5) }
```

**Why**: the state-machine codegen calls `lower_expr(iter)` at the loop header to obtain the array pointer and count. For a plain identifier iter, this loads from the frame-backed crossing-local alloca — stable across resumes. For a call-expression iter, this RE-INVOKES the function on every loop header visit (once for the count check, once for the element load per body_bb entry), meaning N+1 calls instead of 1. For expressions with side effects (heap allocation, network I/O), this breaks the one-alloc-per-task invariant and produces wrong behavior.

**Workaround**: bind the collection to a local before the loop. The bound variable becomes a frame-backed crossing local whose pointer survives suspension and is reloaded at each resume.

**What it costs to lift** (~one session): before the loop header, evaluate `lower_expr(iter)` once to obtain the collection pointer, store it in a synthetic frame slot (similar to `__ynz_for_idx_N`), and reload from that slot at the header on each resume instead of re-evaluating the expression. Both the pointer and the count are stable after the first evaluation, so only one frame slot (for the pointer) is needed.

**Trigger**: a `for` loop whose body contains a `wait` and whose iterator is a call expression — e.g. `for (x in makeArray()) { wait sleep(5) }`.

This is tracked in `registry/features.toml` as `expression-iter-with-wait`.

---

### `ArrayShapeRuntimeFieldWithWait` — `array<Shape>` with runtime field values crossing a `wait` (deferred, clean compile error today)

An `array<Shape>` local declared before a `wait` and read after it is permitted in general — the array's heap buffer survives suspension correctly (the YnzArray pointer is stored via `ptr_to_int` in the frame and reloaded on resume). **However**, if the array literal contains elements whose field values are runtime-computed (e.g. `{ id: 1, qty: a }` where `a` is a variable), those element structs are stored as pointers to stack allocas in the constructing resume function's frame. That stack frame is freed when the function suspends; on the next resume the element pointers are dangling, producing undefined behavior (ASLR-varying stack garbage or a crash).

Elements whose fields are ALL compile-time `IntLit` or `BoolLit` values work correctly — codegen emits LLVM module-level globals for those structs (stable, eternal addresses). The guard fires only when at least one element has a runtime-computed field value.

```ynz
// COMPILE ERROR (ArrayShapeRuntimeFieldWithWait):
let a: int = 10
let items: array<Item> = [{ id: 1, qty: a }]   // runtime field: qty = a
wait sleep(5)
for (it in items) { print(it.qty.toString()) }

// WORKS: all-literal fields (module-level globals)
let items2: array<Item> = [{ id: 1, qty: 10 }, { id: 2, qty: 20 }]
wait sleep(5)
for (it in items2) { total = total + it.qty }   // total = 30

// WORKS: move the array into a helper function that does not use wait
// (no suspension in buildItems — the runtime-field construction is safe there)
function buildItems(qty: int) -> array<Item> {
    return [{ id: 1, qty: qty }]
}
let a: int = 10
wait sleep(5)
let items3: array<Item> = buildItems(a)
for (it in items3) { print(it.qty.toString()) }
```

**Root cause**: `YnzArray` stores each Shape element as an `i64` pointer to the struct's bytes. The codegen path for runtime-field shapes emits a stack alloca in the current resume function — which is freed on suspension. The permanent fix is **by-value inline element storage**: the array heap buffer stores the shape bytes directly (variable slot size = `sizeof(elem)`), so no pointer indirection is needed and suspension is safe regardless of field values.

**What it costs to lift**: 2–3 sessions — a breaking ABI change to `YnzArray` (add `elem_size`, update `ynz_array_new`/`ynz_array_push`/`ynz_array_get`/`ynz_array_set`, and all codegen call sites). See `design/future/array-by-value-element-storage.md` for the full design.

**Guard scope (conservative)**: the interim guard fires on the full `crossing_names` set from the crossing analysis, which conservatively includes some after-last-wait constructions (e.g. `let items = [...]` declared after a `wait` but iterated by a for-loop, which the crossing analysis tracks as an in-scope reference). The guard intentionally over-rejects these — loud over silent — because distinguishing them precisely would require a more complex pre-suspension-only scan. The `m3c-array-by-value` milestone lifts the guard entirely by making runtime-field elements safe across any suspension.

**Workaround**: use only plain literal field values (`[{ id: 1, qty: 10 }]`), or move the array construction and all its uses into a separate helper function that does not contain any `wait`.

**Trigger**: an `array<Shape>` crossing local whose initializer `ArrayLit` contains at least one `StructLit` element with a non-literal field value. The `ArrayShapeRuntimeFieldWithWait` guard in `crates/ynz-typeck/src/check.rs` (Check 2d) rejects these at compile time.

This is tracked in `registry/features.toml` as `array-shape-runtime-field-with-wait`.

---

## Permanent Positional Constraints on `wait`

Two restrictions on `wait` are **permanent design decisions** — not temporary M2 limitations. Both are enforced at typeck and will remain even after M3a lifts the `LocalCrossesWait` and `WaitInsideLoop` guards.

### `SubExprSuspendViolation` — suspending call in a sub-expression position

A suspending call nested inside a larger expression is a compile error:

```
// ❌ — inner() is inside a + expression
let x = 1 + inner()

// ✅ — give it its own line
let result = inner()
let x = 1 + result
```

**Rationale**: Step-by-step style — one operation per line with a named variable — is Yinz's deliberate design (Golden Rule 7). Keeping each suspending call on its own statement also enables M3b's auto-parallelization of independent statements: two `let a = wait fa()` / `let b = wait fb()` lines can be analyzed as independent and parallelized automatically. Expression-position suspension would require a different, more complex codegen path that buys nothing over the step-by-step style.

This guard is not a codegen limitation. It is a style constraint enforced at the language level.

### `MutualSuspensionCycle` — mutually-recursive suspending functions

Two or more different functions that call each other AND all suspend is a compile error:

```
// ❌ — ping and pong mutually call each other and both suspend
function ping(n: int) -> nothing { wait sleep(10); pong(n - 1) }
function pong(n: int) -> nothing { wait sleep(10); ping(n - 1) }

// ✅ — self-recursion works correctly
function countdown(n: int) -> nothing {
  if (n > 0) { wait sleep(100); countdown(n - 1) }
}

// ✅ — restructure: one function delegates to the other without suspending
function step(n: int) -> nothing { wait sleep(10) }
function loop(n: int) -> nothing { if (n > 0) { step(n); loop(n - 1) } }
```

**Rationale**: Self-recursive suspending functions work correctly — a function calling itself is always self-contained (the recursive frame is a heap-boxed child of the same function, and the drop guard walks the chain). Mutually-recursive suspending cycles require per-frame size metadata to safely cancel mid-wait, and the cases that arise in practice can always be restructured into non-mutual forms. The guard is permanent because the restructured form is always cleaner and the mutual-recursion case is rare.

---

## Design Divergences (v0.3-M3b Auto-Parallelization Pass)

The following divergences from the full design doc are documented per `no-duct-tape.md` — each names the concrete cost and the reversal path.

### Same-callee concurrent calls run sequentially

**What the design says**: independent operations auto-parallelize regardless of which function they call.

**What the v0.3 pass does**: two calls to the same suspending function (e.g., `worker(1)` followed by `worker(2)`) are lowered sequentially, even when they are data-independent.

**Named cost**: the composed frame allocates one sub-frame slot per unique callee name (keyed on the function name string). Two concurrent invocations of the same callee would require two slots with a disambiguation scheme — that is a separate concern in `build_frame_layouts`. Any case where the user calls the same suspending helper twice in a row doesn't parallelize in v0.3; it runs sequentially, which is always correct.

**Reversal path**: extend `build_frame_layouts` to allocate N sub-frame slots per callee name when a function is called more than once in a straight-line block, keyed on (callee_name, invocation_index). The independence analysis and join codegen are already class-agnostic; only the frame-layout phase needs the extension. Tracked in `design/future/` for a future phase.

### Write-effect uses a TYPE-BASED conservative floor — mutable-heap arguments never parallelize

**What the design says** (`Reads vs Writes — Ownership Does Double Duty`): "the compiler traces through user functions to determine if they contain writes" — independent writes to *different* resources auto-parallelize.

**What the v0.3 pass does**: the independence analysis treats a call argument as a potential aliased write iff its parameter **type** is a mutable heap reference (`shape`/`array`/`map`/`maybe`/union/`dynamic`). It does **not** attempt to classify per-call-site whether the callee actually mutates the argument. Any suspending call carrying a mutable-heap argument is sequenced against every other statement in the block; only no-argument, primitive-argument, and immutable-`string`-argument suspending calls parallelize.

**Why a type-based floor and not a precise per-form classification**: a precise classification was built (a name-based AST fixpoint over the call graph) and **removed**. Across three adversarial gate rounds it missed five distinct write forms — a mutating method on a `share self` receiver, a mutating method on a parameter directly, mutation through a *field or index* of the parameter (`p.items.add(x)`), a builtin mutator shadowed by a same-named user function, and a local `let`-alias of the argument (`let twin = p; twin.add(x)`) — each a silent runtime miscompile (the analysis classified a real write as a read, so two aliased writes auto-parallelized and reordered observably). The root cause is structural: proving a heap argument is read-only requires receiver types **and** alias tracking — i.e. a full type+alias-aware borrow checker — which an AST-walking pass is not. The `share`-read-only premise the precise analysis leaned on is itself only partially enforced (the same five forms escape typeck's `share` check), so the auto-parallel soundness cannot rest on it. The type-based floor sidesteps the entire class: a mutable-heap argument is *unconditionally* a potential write, sound regardless of how the callee uses it.

This follows the golden rules directly: **Golden Rule 5 (compile-time soundness — never a silent runtime miscompile) outranks Golden Rule 10 (efficiency-first).** When the two conflict, soundness wins.

**Named cost**: two suspending calls that each take a mutable-heap argument do not overlap, even when both only READ their argument (e.g. two suspending reads of distinct shapes). This forfeits the read-only-mutable-heap-argument overlap. It is a perf miss, never a correctness miss — the floor only ever *adds* sequencing, never introduces a wrong parallel execution. The I/O-overlap headline is unaffected: no-argument, primitive-argument, and immutable-`string`-argument suspending calls (the realistic I/O-fan-out shapes) still parallelize.

**Reversal path**: a real type+alias-aware ownership analysis (M4 borrow-checker completion — receiver types threaded through the analysis + intra-procedural alias tracking) that can PROVE a heap argument is both read-only AND non-aliased re-enables narrowing the floor to permit proven-read mutable-heap arguments to overlap. Until that exists, the floor stands. (M3d CPU-parallel reuses this same type-based write-effect source — the floor transfers unchanged.)

### `share` read-only enforcement is best-effort (transitive teaching error)

**What ships**: typeck rejects mutation of an explicit `share` parameter — direct field/element assignment, mutating collection methods (`.add`/`.set`/...), `share self` receivers, passing a `share` argument to an explicit `lend`/`give` position, and the common transitive case (`fa(share x) { helper(x) }` where `helper` mutates) via the `effective_ownership` fixpoint.

**Known-incomplete (deferred to a real ownership analysis)**: the same five forms the auto-parallel floor sidesteps also escape the *transitive* `share` check — a mutation through a field/index of the `share` param via a bare callee, a builtin mutator shadowed by a same-named user function, and a local `let`-alias of the `share` param. These are **missed compile-errors, not miscompiles** — the program still runs soundly (the type-based floor sequentializes regardless), it simply doesn't receive the teaching diagnostic. The `share`-enforcement completeness is decoupled from auto-parallel soundness on purpose: soundness is the floor's job (complete); the `share` error is a teaching aid (best-effort). **Reversal path**: the same M4 type+alias-aware ownership analysis closes both at once.

---

## Runtime Implementation (Internal — Developer Never Sees This)

Thread pool sized to CPU cores. I/O operations use the OS event system (epoll/kqueue/IOCP). The compiler's dependency graph determines scheduling. Developers never configure or think about any of this.
