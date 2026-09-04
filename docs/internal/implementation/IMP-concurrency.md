---
name: "IMP-concurrency"
description: "Full design document. Companion to docs/reference/REF-concurrency.md (user-facing)."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-16"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Concurrency — Design Decisions and Rationale

Full design document. Companion to [`docs/reference/REF-concurrency.md`](../../reference/REF-concurrency.md) (user-facing).

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

**Suspension is automatic. You never write `wait` for it.** A call that can block (transitively reaches an I/O / may-block operation — in v0.3 that is `sleep`) is compiled into a suspension point by the compiler's whole-program may-block analysis (no function coloring — see [`docs/internal/implementation/IMP-no-function-coloring.md`](IMP-no-function-coloring.md)). The function suspends and hands its thread back to the scheduler automatically. The IDE shows the inferred suspension as the muted `wait_points` hint. The user does **not** type `wait` to make a call suspend correctly — that shipped in v0.3-M2.

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

Developers can always force an independent copy explicitly: `background process(data.copy())`. There is no body-level `.give` form (ownership words are signature-only — [`IMP-ownership.md`](IMP-ownership.md) read-first note; `PostfixOpKind` is `Copy | Freeze`, checked against the parser in v0.3-M8 Phase 2, `git log --grep=m8-p2`); the give direction is what the compiler already does for a value the caller never reads again.

The IDE warns on large copies so developers can restructure if needed.

| Scenario | Compiler does | Cost |
|---|---|---|
| Value unused after background | `.give` (move) | Zero |
| Value used after background | `.copy` (clone) | Copy cost |
| Small value (string, int) | `.copy` | Trivial |
| Large value, unused after | `.give` | Zero |
| Large value, used after | `.copy` + IDE warning | Developer restructures or accepts |
| Developer writes `.copy()` at the spawn | `.copy()` — independent copy for that task | Copy cost |
| ≥ 2 spawns in one block read the same shape value, nothing writes it between them | one refcount-shared copy (auto-Arc — [`IMP-ownership.md`](IMP-ownership.md) "Auto-Arc — Sharing Topology"; Phase 5) | One copy + an atomic bump per task |
| Callee declares `share`/`lend` on a non-channel parameter | COMPILE ERROR (`check.rs:~3470`) | — |

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

**Non-async functions**: a `let p` shadowing a parameter in a function that does NOT contain any `wait` is allowed — Yinz permits shadowing per [`docs/internal/implementation/IMP-linting.md`](IMP-linting.md) (`shadowed-variables` Tier-3 lint). The conservative guard only applies to suspending (async) functions where parameters are frame-slotted.

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

### `ECWrapperResultCollection` — collecting the result of a `background`-spawned `-> T errors` task (SHIPPED v0.3-M4, with `background-handle-form`)

**SHIPPED v0.3-M4 (Phase 2)** — the deferral this section recorded is resolved: the copy-before-free fix landed WITH the `background-handle-form` feature, exactly per this section's own "landing WITH the `background-handle-form` feature" gating below. The paragraphs that follow preserve the M3b-era design record; the shipped mechanism is described under "How it was lifted."

The standalone EC wrapper (emitted for `background`-spawned suspending `-> T errors` functions) reconstructs the `{i64, i64}` EC struct from the frame's return slot and then calls `free_frame`. For `-> number errors`, the ok-word in that struct points into the composed frame's 16-byte staging slot — a region freed by `free_frame`. The returned struct's ok-pointer is therefore invalid after the wrapper returns.

This is **safe** because the only reachable caller of the wrapper is `background` (fire-and-forget), which discards the EC result entirely without dereferencing the ok-pointer. There is **no way to collect a `background` task's result in M3b**: the handle form `let h = background ecFn()` is rejected by typeck ("Capturing the output of background is not yet supported"), and the collection syntax (`.send`/`.receive`) ships separately with the `background-handle-form` feature. Until handle-collection lands, this copy-before-free path is **unreachable** — the deferral is vacuous in M3b and stays gated on background-handle collection.

A caller that **collects** the result — reads the ok-word and uses the pointed-at value — must copy it BEFORE `free_frame`. Implementing that read-before-free + copy requires the scheduler to know whether a spawned task's result is collected or discarded, and when the collection happens relative to the frame lifetime. That knowledge only exists once the `background` handle-collection form (`.send`/`.receive`) ships.

**Workaround**: use the inline-poll path — a suspending caller that calls another `-> T errors` suspending function composes the callee inline via the state-machine resume path. Only `background` hits the standalone wrapper, and there is no syntax to collect its `-> T errors` result until background-handle collection ships.

Note: the inline-poll path had its own same-callee reuse hole: when two `let` bindings called the same `-> number errors` function, both bindings' ok-words pointed into the same callee staging slot, so the second call clobbered the first binding's value before it was read. This hole is closed (v0.3-M3f) via copy-on-bind — each binding now receives its own per-binding stable storage for the wide ok-value before the next call can reuse the staging slot. The `background` wrapper's staging-slot deferral below is a separate concern: it applies only to the standalone EC wrapper emitted for `background`-spawned callees, not to the inline-poll path.

**How it was lifted** (SHIPPED v0.3-M4 Phase 2, landing WITH the `background-handle-form` feature): the completion value is extracted from the frame's return slot inside `HandleStateFnFuture::poll`'s Ready arm — strictly BEFORE the embedded future's `Drop` frees the frame. A frame-interior wide ok-value (the `-> number errors` staging slot) is copied to a handle-owned heap buffer and the ok-word repointed at it; the buffer is freed exactly once, at handle drop. One refinement over the paragraph above's prediction: the collected-vs-discarded distinction is keyed at COMPILE time on the spawn form (a handle spawn always extracts + copies; the bare fire-and-forget spawn path is byte-for-byte untouched) — there is deliberately NO runtime "was `.receive()` called yet" conditional anywhere. See `crates/ynz-runtime/src/handle.rs` ("R8 — copy-before-free, compile-time spawn-form-keyed").

**Trigger (satisfied v0.3-M4)**: collecting the completed value of a `background`-spawned suspending `-> T errors` function via its handle — shipped with the `background-handle-form` feature.

The registry deferral entry (`ec-wrapper-collect-on-completion`) was retired when this shipped — see the retirement note in [`registry/features.toml`](../../../registry/features.toml); the living design record is this section plus `crates/ynz-runtime/src/handle.rs`.

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

This is tracked in [`registry/features.toml`](../../../registry/features.toml) as `fixed-array-iter-with-wait`.

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

This is tracked in [`registry/features.toml`](../../../registry/features.toml) as `stored-range-with-wait`.

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

This is tracked in [`registry/features.toml`](../../../registry/features.toml) as `expression-iter-with-wait`.

---

### `ArrayShapeRuntimeFieldWithWait` — LIFTED (fixed by v0.3-M5 by-value storage)

**This guard no longer exists.** From v0.3-M3a through v0.3-M4, an `array<Shape>` crossing local whose literal contained a runtime-computed field value (e.g. `{ id: 1, qty: a }` where `a` is a variable) was loud-rejected at compile time: elements were stored as `i64` pointers to stack allocas in the constructing resume function's frame, which was freed on suspension — so after a `wait` the element pointers dangled (ASLR-varying stack garbage or a crash). Only all-literal elements were safe, via a module-global lowering (`try_build_shape_global`).

v0.3-M5's **by-value inline element storage** removed the root cause: the array heap buffer now stores the shape element bytes directly (variable slot size = element ABI size from `shape_abi_sizes`), with no per-element pointer indirection. The heap buffer is the stable owner of the element bytes and survives suspension like any other crossing pointer local, regardless of whether field values are literals or runtime-computed. The module-global lowering was deleted with the same cut — literal and runtime fields now take the identical path.

```ynz
// WORKS since v0.3-M5 (pre-M5: COMPILE ERROR ArrayShapeRuntimeFieldWithWait):
let a: int = 10
let items: array<Item> = [{ id: 1, qty: a }]   // runtime field: qty = a
wait sleep(5)
for (it in items) { print(it.qty.toString()) }  // prints the real value — no dangle
```

**What was removed in the lift** (v0.3-M5 Phase 3, one reviewed change):

- typeck Check 2d (the diagnostic push) and its helpers `find_array_shape_runtime_field_crossing`, `find_let_initializer_in_stmts`, `expr_is_compile_time_literal` in `crates/ynz-typeck/src/check.rs`
- the matching decline arm in the CPU-promotion probe (`suspension_guards_fire_for_fn`) — hosts with runtime-field `array<Shape>` crossings now promote
- the `array-shape-runtime-field-with-wait` deferral entry in [`registry/features.toml`](../../../registry/features.toml)
- the error-gallery trigger in `examples/primantis-orders/v0_3_m3a_errors.ynz`

**Acceptance coverage**: `crates/ynz-driver/tests/integration.rs` — `m5_p3_array_shape_runtime_field_crossing_runs` (the scratch doc's named acceptance signal), plus the two former guard-hole positions repurposed as acceptance (`m5_p3_array_shape_between_waits_runs`, `m5_p3_array_shape_nested_if_runs`) and the literal-field regression boundary (`v03_m3a_p3_array_shape_literal_crossing_still_works`). Design record: [`docs/internal/implementation/IMP-collections.md` — "Array element storage — by-value inline (v0.3-M5)"](IMP-collections.md#array-element-storage--by-value-inline-v03-m5).

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

## CPU Statement Parallelization (M3d)

### What This Is

M3b auto-parallelizes independent suspending (I/O) calls by interleaving them in the
state-machine's async event loop.  M3d extends that to **pure CPU-bound** calls: when two
or more independent, CPU-heavy statements appear in the same straight-line block, the
compiler schedules them on separate hardware cores concurrently, then collects results
before the next dependent statement runs.

The user writes exactly what they'd write without M3d:

```ynz
function analyzeGame(game: Game) -> Stats {
    let hits    = crunchStat(game.hits)
    let rbi     = crunchStat(game.rbi)
    let average = crunchStat(game.average)
    return combineStats(hits, rbi, average)
}
```

The compiler sees that `hits`, `rbi`, and `average` are independent (no data flows between
the three `crunchStat` calls) and emits parallel execution automatically.  The user never
writes `background`, `wait`, or any synchronization primitive.

### Promotion Rules (what gets parallelized)

A straight-line block is **promoted** to CPU-parallel execution when ALL of the following
hold:

1. **The enclosing function is pure-CPU** — its call graph contains no may-block intrinsics
   and no suspending helpers.  If the function is already a state machine (has `wait` inside
   it or calls a suspending function), its statements are scheduled by the M3b I/O-overlap
   path, not this pure-CPU path — **unless** the state machine's top-level body itself admits
   a *mixed* CPU+I/O group, in which case M3g's fusion (below) spawns the CPU members and
   inline-polls the I/O members through one shared continuation.  A suspending host with no
   qualifying top-level mixed group still runs its statements sequentially through the
   ordinary M3b I/O-overlap path, exactly as before M3g.

2. **Two or more independent groups exist** — the independence analysis (same analysis M3b
   uses) finds a partition where at least one group holds ≥2 statements that share no data
   dependency.

3. **Every member of a CPU group does real work** — a statement is considered "CPU-heavy"
   when the callee is listed in the `does_real_work` set: its call graph reaches a loop
   (`while`/`for`) or recursion (self-recursion or a mutual cycle).  A cheap constant-return
   function does not qualify; bundling it would add blocking-pool spawn overhead without saving
   real compute time.

4. **Minimum two qualifying members per parallel group** — a singleton group cannot be
   parallelized with itself.

### Decline Rules (what does NOT get parallelized)

The compiler declines to promote when ANY of the following hold:

- **The function calls a suspending helper** — it will be a state machine; M3b handles it,
  unless the state machine's top level admits a mixed CPU+I/O group, in which case M3g fuses
  it (see below) — this pure-CPU path itself still never fires on a suspending host either way.
- **All groups are singletons** — nothing to parallelize in parallel with.
- **No member does real work** — per rule 3 above, cheap helpers are not worth spawning.
- **Auto-parallelization is disabled** — the `--no-auto-parallel` build flag (and `kernel` mode,
  which has no scheduler to spawn onto) turns promotion off entirely.  The flag is currently
  carried internally via the `YNZ_NO_AUTO_PARALLEL` env var read by both the promotion query and
  codegen; threading it as an explicit salsa input (so `ynz watch`/LSP codegen honor it without an
  env var) is a deferred mechanism tracked in [`.claude/todos.md`](../../../.claude/todos.md).  It is the program-wide override
  for the "force sequential" direction; per-site, an explicit `wait` between two statements forces
  them to run in order.

### Panic Re-Raise

If any spawned task panics, the panic is re-raised on the driving coroutine after all tasks
are joined.  The first panic wins; subsequent panics from other tasks are discarded.

The user sees a normal panic (same message, same backtrace from the panicking task) — no
special concurrency error type.

### Cancellation Detach Constraint

Spawned tasks cannot be cancelled once dispatched.  If the driving function returns early
(e.g., a return inside an if-arm that runs before the join), the tasks run to completion
and their results are discarded.  This matches Tokio `spawn_blocking` semantics: tasks are
detached from the caller's lifecycle.

Design rationale: cancellation tracking requires per-task abort handles and a drop-guard
protocol that would surface a new primitive to the user.  The detach behavior is always
safe (tasks finish, results are dropped); the only cost is wasted CPU on the early-exit
path.

### Worth-It Proxy

M3d uses a conservative worth-it proxy rather than runtime profiling:

- The function's call graph is inspected for loops (`Stmt::For`, `Stmt::While`) and for
  recursion (self-recursion `f → f`, or a mutual cycle of size ≥ 2 in the call graph).
- If either is found, the callee is marked `does_real_work = true` in the signature table.
- Only `does_real_work` callees appear in CPU parallel groups.

This proxy avoids the need for PGO or runtime cost models at the expense of potentially
missing some genuinely-expensive functions that have neither loops nor recursion.
That is acceptable: skipping a promotion is always correct; a false-positive promotion adds
spawn overhead to a cheap function (soundness issue, not a miscompile — but wasteful).

Future: PGO-based cost models (v0.6+) will replace the proxy with profile-guided thresholds.

### Same-Callee Amendment

M3d **lifts** the same-callee restriction for CPU members.  Two data-independent calls to the
SAME CPU callee — `crunchStat(a)` + `crunchStat(b)`, or `fib(40)` + `fib(41)` — DO run in
parallel: each spawn gets a per-invocation ctx, and the handle/result slots are keyed by
`(group, member-index)` rather than by callee name, so the two invocations never alias each
other's frame slots.  The independence analysis (`partition_groups_classified`) gates the
same-callee branch on `MemberClass::Suspending` only, so CPU members skip it entirely.

The restriction REMAINS for suspending (I/O) members: two calls to the same suspending function
still collapse to sequential, because the I/O sub-frame is keyed by callee name (one slot per
unique callee).  See the divergence entry below for the I/O cost and reversal path.

---

## Mixed CPU+I/O Overlap — Poll-Path Fusion (M3g)

### What This Is

M3b overlaps independent I/O (suspending) calls; M3d overlaps independent pure-CPU calls — but
through v0.3-M3f, the two never mixed: a function containing BOTH one CPU-heavy call and one
suspending call ran them in sequence, one after the other, even though nothing depended on the
other's result. This was a real gap against Model A ("independent operations run concurrently —
this is the default and the maximal-performance choice," see "Suspension vs. Ordering" above),
not a deliberate design boundary — Model A has no member-count or work-class cap, and the typeck
partition machinery (`partition_groups_classified` / `MemberClass::{Suspending, Cpu}`) was
authored from the start to describe both classes sharing one continuation.

M3g closes the gap: a single top-level group can now mix CPU members (spawned onto the blocking
pool, joined via `ynz_rt_join_poll`) and I/O members (inline-polled via an embedded child
sub-frame) under ONE shared continuation. Every resume of that continuation re-drives every live
CPU handle AND every pending I/O sub-frame in one pass — never a blocking join anywhere (the
M2-HALT `block_on`-bridge corpse stays dead; see
[`docs/internal/implementation/IMP-no-function-coloring.md`](IMP-no-function-coloring.md)).

```ynz
function analyzeGame(game: Game) -> Stats {
    let hits    = crunchStat(game.hits)     // CPU-heavy — spawned onto a separate core
    let history = fetchHistory(game.id)     // suspending — inline-polled on the event loop
    return combineStats(hits, history)
}
```

`hits` and `history` share no data dependency, so they overlap: `crunchStat` runs on a separate
core while `fetchHistory` waits on I/O, and `analyzeGame` resumes once both are ready — elapsed
time is `max(cost)`, not `sum(cost)`. The user writes nothing different from the pre-M3g form.

### Admission Rules (what gets fused — ALL must hold)

A top-level group is admitted for fusion (`admitted_fused_group`) when:

1. **A maximal adjacent run, at the TOP LEVEL of the function body only** — a nested (inside an
   `if`/`while`/`for`/`match`) mixed group declines; see Decline Rules below.
2. **Every member is a direct call taking exactly one `IntLit`/`Ident` argument** — mirrors the
   pure-CPU path's existing member restriction (the 8-byte spawn ctx cannot carry more). Applying
   the SAME scalar-only restriction to the I/O members too sidesteps re-deriving the full
   write-effect/alias soundness floor `crate::independence` provides for the general case: a
   scalar-only argument set has no possible aliased-write hazard between members, so independence
   holds by construction regardless of what each callee does internally.
3. **The run contains AT LEAST ONE `Cpu` member and AT LEAST ONE `Suspending` member** — a
   single-class run is not "fused"; it is handled entirely by the existing, separately-tested
   pure-CPU (M3d) or pure-I/O (M3b) path, unchanged.
4. **No two `Suspending` members share the same callee name** — the pre-existing M3b
   same-callee-I/O restriction (above) still applies within a fused group; `Cpu` members are
   unaffected (the Same-Callee Amendment already lifted it for them).
5. **No member's argument references an earlier member's bind name** — the same independence
   check `cpu_group_member_indices` already applies.
6. **No pre-group or post-group statement suspends, and no post-group statement assigns a
   member's bind name** — mirrors the pure-CPU path's existing post-group gates. In practice this
   means a fused-admitted function contains NO suspending call anywhere outside the fused range
   (the pre-/post-group scan is deep, not just top-level), so a fused-admitted function's own
   nested blocks are always suspension-free.
7. **The host takes no parameters** — a conservative bar for this first fused-group codegen
   consumer (the fused group's CPU handle/result reserve and its embedded I/O sub-frames both
   depend on the same `own_base` byte computation a parameter would push past). Narrowing this to
   mirror the pure-CPU path's more precise `param_read_after_join` gate is legitimate follow-on
   work, not a correctness requirement — see Future Requirements in the milestone plan.

Sequential fallback is always correct: a function that fails any rule above lowers exactly as it
did before M3g existed (unchanged pure-CPU or pure-I/O path, or fully sequential).

### Decline Rules (what stays sequential)

- **A NESTED mixed group** (inside `if`/`while`/`for`/`match`) — the existing pure-CPU nested
  machinery's frame-slot reasoning does not yet extend to a nested fused group.
- **A parameter-bearing host** — rule 7 above.
- **Two members of the same suspending callee** — rule 4 above (the pre-existing M3b
  same-callee-I/O restriction, unchanged by M3g).
- **A loop-body group, or a function with a second (multi-group ≥2) parallelizable run** — the
  same M3d decline shapes, unchanged; a mixed group is not a new escape hatch for either.
- **A wide-EC-return member** (`Shape errors` / `number errors` / any return class the
  `YnzCpuResult` ABI cannot carry) — never admitted as a CPU member of a fused group, same as the
  pure-CPU path's own `cpu_result_abi_supports` gate.
- **A non-scalar argument** (anything other than a bare `IntLit`/`Ident`) — rule 2 above.

Each of these is a scoped, permanent-for-now decline with its own trigger and cost, tracked in
the M3g plan's Future Requirements table (multi-group ≥2, loop-body groups, and same-callee I/O
overlap are the same pre-existing M3d/M3b entries this section already cross-references above;
the fused-specific narrowing items — nested fused groups, the parameter-bearing bar — are new
entries added by that plan). None of these is a regression: every one of these shapes declined to
fully sequential before M3g existed too — M3g only ever ADDS a fused path for shapes that were
never fusable before; it never narrows what already fired.

---

## Channel Close — End-of-Stream Semantics (v0.3-M8; Phase 1 SIGNED OFF 2026-09-03 as narrowed by FRAGO 008 — the ownership half moved to [`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value," signed off 2026-09-03 as Phase 2; ships Phase 4)

**Status**: designed in v0.3-M8 Phases 1 and 2, both signed off 2026-09-03 (`audit.md`'s two SIGN-OFF records; `git log --grep=m8-p1` / `git log --grep=m8-p2` for the design rounds); **SHIPPED in v0.3-M8 Phase 4** (`crates/ynz-runtime/src/channel.rs` `ynz_channel_close` / `refuse_closed`, typeck's `close` arm and `maybe<T>` receive, codegen's `conduit_recv_env` envelope; the fixture class is `crates/ynz-driver/tests/v03_m8_channel_close.rs`). This section is promoted out of that entry's one-paragraph format because the design has seven load-bearing parts (mechanism, name, receive typing, send ownership, idempotency, wake propagation, the deferred non-mechanism). Two facts a reader needs before the subsections: the shipped RUNTIME pointer-identity release protocol on the spawn-argument drop ladder (`861fd4d`, merged at `ec014d8`) is substrate this design builds on — "Two mechanisms, one rule" below states how it and Phase 4's typeck consume divide the ownership question, and the closed-send free lives in the runtime because of it; and the authoritative statement of *who else holds this value* — provenance, binding events and alias classes, the sink list, whole-chain reporting, `dynamic Contract` coverage — lives in [`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value" (v0.3-M8 Phase 2). The subsections below keep the CHANNEL-specific facts (which element kinds transfer, the runtime release protocol, the closed-path free) and cite that section for the rule rather than restating it. History — the drafts, the three Phase 1 review rounds and what each changed — is in git: `cd71f7f` carries the first draft and fix round 1 (also found via `git log --grep=m8-p1`); `de631bf` carries fix rounds 2–3 and the sign-off — cite the SHA directly, its message does not carry the `m8-p1` token (parked item 26); `git log --grep=m8-p2` for the Phase 2 re-derivation.

### The decision in one paragraph

`channel<T>` gains one non-suspending action, **`.close()`**: "no more values will be sent." After `close()`, every `send()` — from any task — returns the typed channel-closed error (the M4 Lock-8 arm that has been dead code since it shipped), and `receive()` keeps delivering whatever is still buffered, then returns `none` once the buffer is empty. A second `close()` is a safe no-op. Two type-level changes ride with it. **`receive()` on a bare channel changes type from `T` to `maybe<T>`**, so the end of the stream is a normal value the consumer checks — never a hang, and never an error, because running out is not failing. **`send()` gives its payload**: for an owned-heap element type (`array<T>`, `map<K, V>`) the sent binding is consumed at the send, which is what makes freeing a refused payload on the closed path sound and closes a pre-existing cross-task aliasing hole — and, so that the consume reaches the value's real owner, **a parameter that is sent must be declared `give` on its function, and only a named binding (or a fresh `.copy()`/literal) may be sent**. Auto-close when the last producer goes away is NOT part of this design — it is a named deferral below, with the missing substrates that gate it.

### Why closure is structurally unreachable today (the mechanism the design changes)

`YnzChannel` (`crates/ynz-runtime/src/channel.rs`) is one `Arc`-shared object that owns BOTH endpoints of one Tokio bounded mpsc channel: `sender: Mutex<mpsc::Sender<i64>>` and `receiver: Mutex<mpsc::Receiver<i64>>`. Tokio's two closure signals are "every `Sender` dropped AND buffer drained" (`poll_recv → Ready(None)`) and "the `Receiver` dropped" (`try_send → Closed`). Because the object holds one of each for its whole life, and the object only dies when the LAST `Arc` reference is released, neither signal can fire while any holder can still call anything. The substrate tests reach closure only by `std::mem::replace`-ing an endpoint under `#[cfg(test)]`.

Two further facts bound what any fix can be:

- **Holders have no role.** Every holder — the spawning function and each `background` task it passed the channel to — holds the SAME `Arc<YnzChannel>` (`ynz_channel_share` per spawn argument; `ynz_channel_free` from the drop ladder at task retirement). The strong count counts holders, not producers vs. consumers. The language surface has no role either: a `channel<T>` binding is both ends.
- **The spawner's own reference is never released at scope exit.** No scope-exit drop-insertion pass exists in the compiler — no local of any kind is released at scope end (`ynz_handle_free` is (declared in `runtime_decls.rs`'s `RuntimeDecls`, zero call sites in `emit.rs`); [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Task Cancellation" implementation note; the roadmap's never-drop-locals row); the drop ladder frees only the spawned tasks' references. So "all references except the consumer's are gone" is not an observable event today, even in principle.

### Mechanism candidates weighed

| Candidate | Feasibility | Verdict |
|---|---|---|
| **(a) Explicit `.close()`** — the sender endpoint becomes `Mutex<Option<mpsc::Sender<i64>>>`; `close()` is `.take()` plus a wake of every recorded receive-waiter | Contained. The handle path already does exactly this — `HandleStateFnFuture.outbox_tx: Option<mpsc::Sender<..>>` is `.take()`n after completion delivery so a post-completion `h.receive()` observes Closed "instead of hanging" (`handle.rs`). One convention, second instance. Runtime: one field shape change + one new C-ABI entry. Typeck: one new non-suspending conduit method. Codegen: a thin call, and the two closed arms go live. | **CHOSEN** |
| **(b) Auto-close on last-producer drop** — the channel closes itself when every logical producer's reference is gone (Rust/Tokio's own model) | Needs THREE things the tree does not have: (1) per-holder role classification (which holders are producers — a flow analysis over `send` reachability per binding, per task), (2) a scope-exit drop pass so the spawner's reference is actually released when its function ends, and (3) a runtime refcount split into producer-count vs. holder-count so the object can observe "producers = 0" while consumers still hold it. That is a producer-ref-counting redesign plus the language-wide drop pass, not an extension of the current `Arc` model — and in the default fan-in shape (spawner spawns N producers, then receives) the spawner's reference survives the whole loop, so (1) is load-bearing, not optional. | **DEFERRED** (four fields below) |
| **(c) Both** — explicit now, auto-close later | This is (a) shipped plus (b) recorded honestly. | **This is the shape of the decision** |

### Name: `.close()`

Candidates weighed against [`vocabulary.md`](../../../.claude/rules/vocabulary.md), [`dot-postfix.md`](../../../.claude/rules/dot-postfix.md), and Golden Rule 12:

- **`.close()` — chosen.** An 18-year-old JS developer reads `wire.close()` as "close the wire, nothing more is coming" with no lookup — it is the word files, sockets, and WebSockets already use for the same idea. It takes parens because it is an ACTION (dot-postfix rule). It is not banned jargon. It has no collision: no `file`/`io` module exists yet, and when one ships, `file.close()` will mean the same thing — same concept, same word, which is consistency, not collision. Decisively: the runtime, codegen, and every existing error string already call this state **closed** (`CHANNEL_CLOSED`, "The channel is closed - the receiving side is gone…"). Naming the action `.close()` makes the error text and the action the same word; any other name forces the user to learn two words for one state.
- **`.done()` — rejected.** Reads as a QUESTION or a state ("is it done?"), which under the dot-postfix rule wants no parens and returns a boolean; as an action it is ambiguous about who is done (the channel? the sender? the task?).
- **`.finish()` — rejected.** "Finish what?" — the receiving, the sending, the task. It also collides conceptually with a task's own completion (`h.receive()` already delivers "the task finished"), so a reader would guess it ends the task.
- **`.end()` — considered (Node's `writable.end()`), rejected.** A channel has two ends; `wire.end()` reads as a noun ("the end of the wire — which end?") before it reads as a verb.

### The contract (what Phase 4 implements, what tests lock)

1. **`ch.close()`** — takes no arguments, returns `nothing`, **never suspends**, never fails. It is a plain action with no `errors` on it: closing a channel cannot fail, and there is nothing for a caller to handle.
2. **Send after close** → the typed channel-closed error, at runtime, on the existing `send() -> nothing errors` surface (Lock 8) — from any task, including one whose `send()` happens to race the `close()`. **The order is decided by ONE critical section, and it is the sender-lock section, not the `try_send`.** `channel_send_poll_guarded`'s first attempt takes the sender lock, clones the shared `Sender`, and releases the lock BEFORE calling `try_send` (`channel.rs:445–446`). After this design, the `None`-check and the clone are that one critical section: a `send()` that reaches it after `close()` took the `Option` sees `None` and is refused; a `send()` that reached it first holds a transient clone, and a `close()` that runs between its unlock and its `try_send` does NOT stop it — the clone keeps the endpoint alive and the value lands. That is linearizable as send-before-close (the send's linearization point is the clone under the lock), so the contract holds. Phase 4's fixture and Phase 3's loom model must encode exactly this: **a send already holding a clone is a pre-close send.** A test asserting "any `try_send` that executes after `close()` returns must be refused" is asking for an ordering the implementation does not provide and does not need to. **No compile-time diagnostic** — see "Send-after-close is a runtime error" below.
3. **Receive after close** → drains the buffer first (every value sent before `close()` is delivered, in order), then returns `none`. Never a hang, never a panic, never an abort — the `ChanRecv` closed arm that currently aborts via `ynz_unhandled_error` ("structurally unreachable") becomes a normal `none` return through the same `maybe<T>` construction every other `maybe` value uses.
4. **In-flight sends issued BEFORE close complete normally.** A producer suspended on a full channel holds its own cloned `Sender` inside its boxed endpoint future; `close()` only takes the object's shared endpoint. That suspended value still lands once the consumer drains a slot — "close" means no NEW sends, not "discard what was already on its way." When the consumer drains, the future resolves, its clone drops, and the channel then reports closed. (A cancelled producer's clone is dropped by `purge_pending_sends` — the M6 purge path — so a dead suspended send cannot hold the channel open.)
5. **Idempotent.** A second, third, Nth `close()` is a safe no-op: `Option::take()` on `None` does nothing; no error, no panic, no log line. This is an invariant of the v0.3-M8 plan, not an implementation accident — a RED test locks it.
6. **Close wakes every recorded receive-waiter.** A task parked on `receive()` when `close()` runs must re-poll and observe either a remaining buffered value or `none` — never stay parked. `close()` therefore drains `recv_waiters` exactly the way a successful send does. This also settles the "closure observed by one receiver is not propagated to co-waiters" facet `channel.rs` left for this design pass: after `close()`, every waiter is woken once by `close()` itself, and any receiver that registers afterward polls immediately and sees the closed state; the `Ready(None)` exit additionally drains the waiter list (harmless if empty), so no registration can outlive closure unwoken.
7. **Drop-without-close is unchanged.** A channel that is never closed behaves byte-for-byte as it does today (the whole pre-existing M4/M6 fixture suite is the regression gate). Closing is opt-in; it adds an end-of-stream signal, it does not change the lifetime model.
8. **Element-type agnostic.** Closed-ness is state on the object, not on the payload; `int`, `string`, `array<T>`, `map<K, V>` channels close identically, and any element type added later (see the fr12 note) inherits it without a per-type arm.
9. **`send()` gives its payload for owned-heap element types.** On a `channel<array<T>>` or `channel<map<K, V>>`, `send()` is a transfer sink: the payload goes through the ONE `check_transfer` decision in [`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value" — a fresh value is admitted; a whole owned binding is consumed (with its alias class); a parameter must be declared `give` on its function (`ParamNeedsGive`, the word travelling up the whole chain, every frame reported in one compile); anything someone else still reaches — a field, an index, a loop cell, a literal built from named values, a call that returns a piece of its argument — is refused (`TransferNeedsCopy`). `int`/`float`/`bool`/`string` channels are unchanged, and `number` (fr12, below) is copy-through. Channel-specific consequences in "`send()` gives its payload" below.

### `receive()` on a bare channel becomes `maybe<T>`

This is the one breaking change, and it is deliberate. Today `let v = ch.receive()` yields a plain `T`, which is only sound because closure cannot happen — the closed arm aborts the process. Once closure is real, the consumer needs a value that says "the stream ended." That value is **`none`**: `ch.receive()` returns `maybe<T>`, consumed with the narrowing the language already ships — `.exists()` / `.value` (`check.rs:2555–2572`; `m5_p2_byval_bg_maybe_arg_escape.ynz:15–17` is the shipped form) or `.or(fallback)`.

**Why `maybe<T>`, not `T errors`.** [`vocabulary.md`](../../../.claude/rules/vocabulary.md) reserves `maybe<T>` for "when 'no value' is a normal possibility" and `errors` for failures the caller must handle. End-of-stream is the normal outcome of every correct consumer — the `tallyScores` loop below ends every correct run by observing it exactly once. Typing that as an error would make the normal exit a failure, and the tree shows precisely how that goes wrong:

- **Auto-propagation would turn the end of the stream into the task's own failure.** Inside a `-> T errors` function, `resolve_ident` narrows a bound `T errors` value to its success type and marks it for early-return-on-failure (`check.rs:3647–3653`). A consumer written `function drain(lend wire: channel<int>) -> int errors { let v = wire.receive() … }` would, on the closed signal, return "channel closed" as the TASK's error — and both channel-consuming task functions that ship today are exactly that shape (`v0_3_m4_handle_send_roundtrip.ynz:4`, `examples/pirates-roster/entrypoint.ynz:1082`). With `maybe<T>` there is nothing to propagate; the consumer decides what the end means.
- **An allocation per end-of-stream, on a path that is not a failure.** The closed arm builds its error value through `ynz_error_new` (`emit.rs:12802–12813`) — a heap allocation plus a message string every time a loop ends normally. `none` is a bit pattern.
- **The "one `.receive()` convention" argument does not hold.** A handle's `receive()` is `T errors` because its error arm carries the task's OWN failure (`check.rs:4181–4186` — "the error arm is the task's own error, or task-already-finished"). A bare channel has no failure to carry; it has absence. [`stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 2 bans two NAMES for one capability; two receiver types each returning what they actually have is not that.

**The handle's `receive()` stays `T errors` — do not "unify" the two.** `h.receive()` delivers a message reply or the task's completion value, and a task can FAIL; that failure has to travel somewhere, and the handle's error arm is where it goes. `ch.receive()` has no task behind it: a channel ends, it does not fail. The two share a name because they share a verb (take the next thing out) and differ in return type because a task can go wrong while a channel can only run out. A future reader proposing "one `.receive()` type" is proposing either to lose the task's error (handle → `maybe<T>`) or to manufacture a failure that never happened (channel → `T errors`); both are worse than two honest types.

**The end-of-stream consumer loop, in real syntax.** There is no `break` keyword and no standalone `else` block (`REF-control-flow.md` — only `else =>` inside a multi-case `if`), so the loop runs on a flag that is re-read from the delivery — this is the form the spec and demo teach (Phase 4 corrected an earlier sketch here that used a standalone `} else {`):

```ynz
function tallyScores(lend wire: channel<int>) -> int {
  let total = 0
  let stillOpen = true
  while (stillOpen) {
    let next = wire.receive()          // maybe<int> — none means the stream ended
    if (next.exists()) {
      total = total + next.value
    }
    stillOpen = next.exists()
  }
  return total
}

function entrypoint() -> nothing {
  let wire: channel<int> = channel<int>(4)
  background relayScores(wire)         // sends, then calls wire.close()
  let total = tallyScores(wire)
  print(`total: ${total.toString()}`)
}
```

**Blast radius (Phase 4 owns it; pre-1.0 permits it).** The retyping touches **19 bare-channel `receive()` sites across 13 fixture files** under `crates/ynz-driver/tests/fixtures/`: `v0_3_m4_channel_composed_bare.ynz:14–16` (3), `v0_3_m3e_alias_local_name_collision/entrypoint.ynz:27–29` (3), `v0_3_m4_channel_composed.ynz:27,29` (2), `v0_3_m4_r5xr8_suspend_then_collect.ynz:17–18` (2), and nine singletons — `v0_3_m4_channel_smoke.ynz:4`, `v0_3_m4_channel_cpu_admission_declines.ynz:21`, `v0_3_m4_channel_field_annotated.ynz:17`, `v0_3_m4_handle_send_roundtrip.ynz:5`, `m5_p4_soa_both_candidate.ynz:26`, `v0_3_m4_p4_padding_gate.ynz:25`, `v0_3_m4_p3_cross_channel_exempt.ynz:13`, `v0_3_m4_channel_xmod_return_annotated/entrypoint.ynz:14`, `v0_3_m4_channel_xmod_return/entrypoint.ynz:13`. Outside the fixtures: `examples/primantis-orders/v0_3_m4_errors.ynz:80,93`; `examples/pirates-roster/entrypoint.ynz:1083,1120–1122,1146`; `docs/reference/REF-concurrency.md:199–201` and `:252` (the handle-section example's child reads its command channel bare). Handle-side `h.receive()` sites are untouched — their type does not change. Each bare site changes from a `let x = ch.receive()` used as `T` to a narrowed form; no error-gallery line is retired (`v0_3_m4_errors.ynz:80` stays a sub-expression-position trigger).

### `send()` gives its payload — the closed-arm free is only sound if the sender no longer holds the value

**The hole.** Typeck's `send` arm (`check.rs:4105–4149`) only calls `infer_expr` on the argument; the checker's only two `scope.consume` sites are `check.rs:1511` (the `background` statement-form give-inference path) and `check.rs:4618` (`check_arg_ownership`, the user-fn/UFCS `give`-parameter path — cited by FUNCTION NAME here, not line, per parked item 21, the third time this pair was swapped), and neither is reachable from `check_conduit_method_call`. Codegen lowers the payload with a bare `to_i64_bits` cast (`emit.rs:12657`) — no copy, no reference bump. So on a `channel<array<int>>`, `let sent = wire.send(rows)` leaves `rows` live and readable in the sender while the SAME allocation sits in the buffer for the receiver. Confirmed on the current tree (2026-09-03): a program that sends `rows` and then prints `rows.count()` compiles and runs. That is two live defects today, before any close design: (1) the sending task and the receiving task alias one mutable array with no synchronization — the exact cross-task aliasing the ownership model exists to forbid; (2) `YnzChannel::drop` runs the registered element glue on every residual buffered element (`channel_drop_glue`, `emit.rs:15483–15541`), so a buffered-but-unreceived array is freed at teardown while the sender's binding still points at it. The first draft's P2-3 fix — freeing the refused payload on the closed arms (`emit.rs:12848–12876` / `12923–12947`) through that same glue — would have added a third instance: a use-after-free on every send-after-close.

**The rule (re-derived in Phase 2; the three-round shape list it replaces is preserved in git at `de631bf`).** `send()` transfers ownership of its payload. In `check_conduit_method_call`'s `send` arm (`check.rs:4105–4149`), when `channel_elem_drop(elem)` is `Some(kind)` with `kind.transfers_source()` (the **owned-heap element** kinds, "Which element types" below), the arm calls the ONE transfer decision, `check_transfer(payload, Sink::Send { channel })`, and inspects no syntax of its own. What that decision does — `Provenance::Fresh` admitted; `Whole(name)` consumed with its alias class, or refused when the binding is `const` (the `:4602` refusal, **extracted** into a helper with a sink-supplied WHAT-INSTEAD so the send arm can say `Send a copy instead: {channel}.send({name}.copy()).`), a non-`give` parameter (`ParamNeedsGive`, which also retires the `:4611` share-only refusal), or a binding whose origin is a piece of something else; `Reaches`/`Unknown` refused (`TransferNeedsCopy`) — is specified once in [`IMP-ownership.md`](IMP-ownership.md) "The transfer decision — ONE emit site" and not restated here. The earlier three-forms-and-a-refusal list admitted forms that alias (a literal built from named values; a `let` bound to a call that returns a piece of its argument — both confirmed by probe on 2026-09-03) and refused forms that are fresh (`array<int>()`); provenance gets both right by construction, and a form nobody listed is a compile error in one function rather than a hole.

After the send, any read of a consumed binding — or of any name in its alias class — reaches the existing consumed-read site (`check.rs:3622–3631`), the ONE place a use-after-give is reported today, which selects `ConsumedBySend` by cause.

**Ownership must flow through the call — the hole the source-level claim had, and the guard that closes it (Patrick's ruling, 2026-09-03, fix round 3).** The second draft claimed "the aliasing hole closes by construction at the SOURCE level." It did not. This program compiles under that draft, prints `3` today, and under the draft is a deterministic use-after-free with no `background` anywhere:

```ynz
function producer(wire: channel<array<int>>, rows: array<int>) -> nothing {
  wire.close()
  let sent = wire.send(rows)     // consumes producer's OWN binding `rows` — and nothing else
}
function entrypoint() -> nothing {
  let wire: channel<array<int>> = channel<array<int>>(4)
  let rows: array<int> = [1, 2, 3]
  wait producer(wire, rows)      // entrypoint's `rows` is NOT consumed: the parameter has no `give`
  print(rows.count())            // the send core freed the payload on the closed path — this reads freed memory
}
```

The producer of the defect is the call-site ownership path itself: `check_arg_ownership` (`check.rs:4591–4648`) runs only for a plain-identifier argument (`simple_ident_name`, `:4798–4800`) and consumes the CALLER's binding only when the CALLEE's parameter is declared `give` (`:4599`); an unannotated or `share` parameter falls to `_ => {}` (`:4646`). So today ownership does not flow through an ordinary call at all — a bare parameter is a silent alias of the caller's value. That was harmless while nothing ever freed a value (no scope-exit drop pass exists); the moment `send()` hands a payload to a channel that WILL free it (teardown glue, the closed-send free, the receiver), every un-`give`n parameter between the owner and the send is a live use-after-free door. Two guards, both threading the one existing give path rather than a second analysis ([`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)):

1. **A parameter binding sent on an owned-heap channel must be declared `give` on its enclosing function** (`ParamNeedsGive`). With `give` declared, the EXISTING machinery does the rest with no new code path: every call site — `wait producer(wire, rows)`, a plain call, a UFCS dot-call (receiver AND non-receiver positions, once the three call paths share one argument-list normalization — the round-3 finding that `bucket.stash(rows)` never reached `check_arg_ownership` is closed by that sharing, not by a fourth loop), a generic call, a `dynamic Contract` call — reaches the one transfer decision at every `give` position and consumes the caller's binding; every `background` spawn site reaches it too, because the `Expr::Background` arm (`check.rs:3283`) runs `infer_expr(inner)` on the spawned call (`:3302`) and that call is `check_user_fn_call`. The statement-form liveness inference (`:1443–1515`) still runs and records Give or Copy for codegen; for a declared-`give` parameter the binding is already consumed when it runs, so a program in which the spawner reads the binding afterward does not compile (the consumed-read site fires) and a program in which it does not read it records Give — the two never disagree on a program that builds.
2. **A payload someone else still reaches is refused** (`TransferNeedsCopy`) with the `.copy()` WHAT-INSTEAD — decided by provenance, never by the shape of the argument.

**How far the `give` obligation transits — all the way, and every frame in ONE compile.** Every function between the value's owner and the send declares `give` on that parameter. A relays to B relays to C, and C sends: C's parameter must be `give` (guard 1); B passes its own parameter to C's `give` parameter, so B's parameter must be `give` too — otherwise B's caller keeps a live binding for exactly the reason the program above does; and so on up to the frame that owns the value as a local. Enforced at ONE site — the transfer decision every `give` position calls — where the current `else if !entry.is_consumed { consume }` branch of `check_arg_ownership` (`:4617`) silently consumes a bare or `lend` parameter passed to a `give` parameter; that silent consume IS the relay hole, and it becomes `ParamNeedsGive`. "Sent into a channel" and "given to a `give` parameter" are one rule with one diagnostic and a `{act}` slot, not two rules kept in sync by hand. **The whole chain is reported in one compile**: the `effective_ownership` fixpoint's new `consumed[fn][i]` fact is true for B's position the moment B's body sends it, `give` or not, so A's call `B(x)` is a give position in the same build and A's own missing `give` is reported beside B's. The earlier draft reported one frame per compile on the premise that typeck lacks a callee-before-caller ordering; the fixpoint IS that ordering (it runs after the body check today only because nobody had hoisted it — Phase 4 reorders, see [`IMP-ownership.md`](IMP-ownership.md) "Where the authority lives"). Today's fixture corpus: `give`-parameter functions are called from `entrypoint` with locals (`v0_3_m4_p3_cross_give.ynz`, `v0_3_m3b_p4_give_arg_sequenced.ynz`, `m4_player.ynz`, `pirates-roster/entrypoint.ynz:350,1108`); Phase 4 confirms zero relay-through-bare-parameter instances by compiling the corpus, not by inspection — with one known instance already on the books, `examples/primantis-orders/m6_errors.ynz:112–115` (`background fig.haulCircle()` passes bare parameter `fig` as the receiver of a `give self`), which gains a second diagnostic on that line and a `// WHY:` update (parked item 17).

**The alternative weighed: infer `give` for a bare parameter the body gives away (Golden Rule 4's direction).** Still rejected — but on two reasons, not three. The first reason the earlier draft gave (typeck has no whole-program ordering to infer from) was false, and is withdrawn: the fixpoint computes exactly the fact an inference would need, and this design USES it — for reporting. What stands: (1) the inference would silently change the CALLER's program — its binding becomes unusable — from an edit inside another function's body, possibly another file; [`inference.md`](../../../.claude/rules/inference.md) puts ownership words in signatures precisely so the contract is visible at the definition, and treats ownership-transfer inference as cautionary-class, which would need the muted hint at every affected call site. (2) Golden Rule 5: `give` in the signature makes the transfer a compile-time fact every reader of the signature sees. Patrick ruled for the explicit word. The line this design draws: the fixpoint's `consumed` fact may make an ERROR appear one frame earlier; it never makes a program ACCEPTED whose signatures do not say `give`. `share` and `lend` parameters are refused by the same rule: they are explicit statements that the caller keeps the value, so giving it away contradicts the declaration.

**What this closes in Future Requirement #9.** FR#9 names `background producer(wire, table)` + `wire.send(table)` on a `channel<map<..>>` as "this door through the channel" — a `map` bg arg is passed ALIASED (un-cloned) by `prepare_bg_arg_for_ctx`, so the task would send the PARENT's allocation while the parent's binding stayed live. Guard 1 closes that instance: the send requires `give table` on `producer`, and `give` consumes the parent's `table` at the spawn (through `check_arg_ownership`, as above), so the parent can no longer read the aliased map; the channel or its receiver is the allocation's only holder, the ladder has no descriptor for an un-cloned arg (nothing to double-free), and the parent's own frame never frees it (no drop pass). The pass-a-`give`-map-and-send shape is therefore asserted CORRECT by Phase 4's `channel<map>` fixture, not RED-pinned. What guard 1 does NOT close is FR#9's non-channel door — `bucket.add(rows)` storing a ladder-owned clone into an aliased container — which stays RED-pinned under FR#9; the plan's FR#9 text is updated to say exactly this rather than leaving the channel door overstated. The alias blind spot earlier rounds recorded as "best-effort" — by `let` (`let other = rows; wire.send(rows); other.count()`), by reassignment (`rows = other; wire.send(rows); other.count()`) and by a shadowing `let` (probes 2026-09-03: all three compile and print the un-consumed alias's count) — is CLOSED by the binding-event rule and alias classes in [`IMP-ownership.md`](IMP-ownership.md) "Binding events, origin and alias classes": every binding event puts the name in the class of the value it now denotes (a reassignment moves it out of its old class first), and consuming any member consumes all. **The `ScopeEntry` change is a signature change, not a flag**: `is_consumed: bool` (`scope.rs:31`) becomes `consumed: Option<ConsumedBy>` with `enum ConsumedBy { Given { callee: String }, Sent { channel: String } }` (the cause fills `{channel}`/`{callee}`); the entry gains `origin: Origin` and an alias-class id, **(re)computed at every binding event, per [`IMP-ownership.md`](IMP-ownership.md) "Binding events, origin and alias classes" — not set once at creation** (a reassignment leaves the old class and joins the new one; parked item 22 corrected this paragraph, which had contradicted that section); `Scope::consume(name, cause)` (`scope.rs:85`) consumes the class; the six `ScopeEntry` constructors, all in `check.rs` (`:642`, `:1406`, `:1623`, `:1660`, `:1723`, `:2803` — parked item 27), set origin and class; the two `!entry.is_consumed` guards (`check.rs:1510`, `:4617`) become "not already consumed by ANY cause"; and the consumed-read site selects the `Consumed` or `ConsumedBySend` template by the cause. Phase 4 must not bolt an `Option<String>` beside the bool — one field, one enum. The fix for a sender that still needs the value is `wire.send(rows.copy())`.

**Which element types.** The bite is exactly the owned-heap element set — the types for which the channel registers drop glue at construction: `array<T>` and `map<K, V>`. `channel_drop_glue` has exactly those two non-null arms, and typeck admits only `int`/`float`/`bool`/`string`/`array`/`map` as element types. `int`, `float`, `bool` are value bits: the send copies them and the binding stays usable, so every existing `ch.send(v)` on a primitive channel is unchanged. `string` is immutable, immortal bytes with no glue: sharing the pointer is sound and nothing ever frees it, so a sent string binding also stays usable. The predicate "is this element type owned-heap?" must be ONE definition both crates read — typeck for the consume decision, codegen for the glue arm — per [`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md). **It is `Option<enum>`, not a bool and not a three-variant enum**, because codegen needs the drop function, not a yes/no, AND because the absence case must be unrepresentable inside the glue-bearing set: `pub enum ChannelElemDrop { Array, Map, NumberCell }` with `pub fn channel_elem_drop(elem: &Type) -> Option<ChannelElemDrop>` in `ynz_typeck::types`. **Two questions ride the one enum, and fr12 is why they are two**: "does this element kind have glue?" (`is_some()`) and "does sending it transfer the SOURCE binding?" (`kind.transfers_source()` — `Array | Map => true`, `NumberCell => false`, an exhaustive match so a new kind must answer both). `array`/`map` payloads ARE the sender's allocation, so the send consumes the binding; a `number` payload is a fresh 16-byte cell the send mints ("fr12" below), so the binding stays usable and only the cell needs glue. Typeck consumes on `is_some_and(transfers_source)`; `channel_drop_glue` (`emit.rs:15509`) replaces its two-arm `match &elem` with `match channel_elem_drop(elem) { None => null, Some(kind) => <fn pointer> }` where the inner `match kind` is EXHAUSTIVE and its arms are typed as a function value (`Array → ynz_array_drop`, `Map → ynz_map_drop`, `NumberCell → ynz_number_cell_free`) — never a nullable pointer. A three-variant `{ None, Array, Map }` was the second draft's shape and is rejected: an exhaustive match only guarantees an ARM exists, not that the arm registers glue — `Number => cg.ptr().const_null()` compiles, and that is precisely the silent divergence the enum exists to forbid (a consumed binding whose payload nobody frees). With `Option`, every variant reachable from `Some(..)` must produce a real drop function or the arm does not type-check. Neither existing predicate fits: `is_trivially_copyable` (`types.rs:242`) classifies `string` wrong for this purpose, and `is_mutable_heap_type` (`independence.rs:751`, private) returns `false` for `number` — which is CORRECT for the transfer question (parked item 3 assumed `number` would join the give set; the fr12 design says it does not) but says nothing about glue. `check_channel_construction`'s `elem_supported` list (`check.rs:4286–4297`) is then a THIRD listing of element kinds; Phase 4 derives it from the same function (`supported = value-bits ∪ string ∪ channel_elem_drop(elem).is_some()`) or pins it with a parity test over every `Type` variant — never a third hand-maintained list. A future element kind joins by adding an enum variant, and the two exhaustive matches tell codegen where the glue is missing and typeck whether the source is consumed — neither can be satisfied with a null or a default.

**What this makes sound — and its precondition.** With the sender's binding consumed (and every name in its alias class), AND every parameter between the value's owner and the send declared `give` (so each caller's binding is consumed in turn), the buffered value has exactly one source-level holder at every moment: the owner until the send lands, then the channel, then the receiver. Teardown glue frees a value nobody else holds. The receiver's `maybe<array<T>>` is the sole reference. The claim holds for every payload the transfer decision admits — a task-built value, a heap-CLONED `background` argument (an `array<int|float|bool|shape>` — ladder-owned, and released from the ladder at the send by the shipped runtime protocol in "Two mechanisms, one rule" below), and an ALIASED `background` argument (`map<K, V>`, `array<pointer-elem>`, union — `prepare_bg_arg_for_ctx`'s un-cloned fall-through) **provided it arrives through a `give` parameter**, because the `give` consumes the parent's binding at the spawn and the parent can no longer reach the aliased allocation (see "What this closes in Future Requirement #9" above). Without `give`, such a parameter cannot be sent at all (`ParamNeedsGive`); a loop cell of the parent's collection cannot be given to the task at all (`TransferNeedsCopy` at the spawn's `give` position), so the `for (row in matrix) { background producer(wire, row) }` door closes with the same rule. What stays outside the claim is exactly what [`IMP-ownership.md`](IMP-ownership.md) "What this makes sound, and what stays outside" names: FR#9's non-channel door (`bucket.add(rows)` into an aliased container — the deferred container-store sink class, RED-pinned by `bg_arg_alias_container_add_red.ynz`) and the FR#10 `.copy()` types (which provenance refuses to transfer at all). The alias-by-`let` blind spot is no longer on that list. Phase 4's `channel<map<K, V>>` fixture therefore covers BOTH shapes — a map built inside the sending task, and a map passed as a `give` bg arg and sent — as correct programs, plus the no-`give` variant as a gallery trigger.

**Four new compile-time diagnostics.** Use-after-send (`ConsumedBySend`); the two ownership-flow guards (`ParamNeedsGive` — which also replaces the existing share-parameter refusal at `check.rs:4611` — and `TransferNeedsCopy`, which replaces the Phase 1 draft's `SendPayloadNeedsCopy` because it fires at every transfer sink, not only at a send); and the handle-spawn channel-binding guard (`HandleChannelArgNeedsBinding`, from "The gap that decision leaves" below). The three transfer diagnostics' text is in [`IMP-ownership.md`](IMP-ownership.md) "Teaching text" — one home per string; `HandleChannelArgNeedsBinding`'s stays in this section's "Teaching text". All four join the v0.3-M8 plan's Invariants → Teaching list and Phase 9's error-gallery obligation.

**Fixture gap Phase 4 must close.** There is **no `channel<array<T>>` or `channel<map<K, V>>` end-to-end fixture anywhere in the tree** — the only non-primitive channel fixture is `v0_3_m4_channel_construct.ynz:14` (`channel<string>`, construction only). The give semantics, the closed-arm free, and the teardown-glue path for owned-heap elements have never been executed by a `.ynz` program. Phase 4 authors one RED→GREEN fixture per element kind (array, map) covering: send-then-receive round-trip; send-then-read (the compile error, in the gallery); send-after-close (the freed payload, with alloc/free parity via `YNZ_ALLOC_COUNTER_OUTPUT`); drop-with-buffered-element; **the ownership-flow class** — the reviewer's `wait producer(wire, rows)` program above, once with `give rows` (the parent's later read is `ConsumedBySend`/`Consumed` at the call site — gallery) and once without (`ParamNeedsGive` — gallery); a two-hop relay (`A → B(give) → C(give)`, correct; and `B` without `give` — BOTH `B`'s and `A`'s errors in one build when `A` relays its own parameter); a `give`-map bg arg passed and sent (correct, alloc/free parity); **the seven live-hole probe programs of 2026-09-03 as gallery triggers** (UFCS non-receiver give `bucket.stash(rows)`; field `eat(bucket.rows)`; loop cell `for (row in matrix) { wire.send(row) }`; alias-by-`let` `let other = rows`; alias-by-reassignment `rows = other; wire.send(rows)`; returns-a-piece `let rows = pick(bucket)`; nested literal `[a]`) — plus `dynamic Contract` give named separately as the non-exposing eighth probe (typeck rejects, codegen ICEs — zero runtime exposure, a compile-time-only trigger; `.claude/corpses.md`, first entry) — beside the admitted forms (owned ident, `.copy()`, a literal of fresh elements, a constructor call, a `returns_fresh` call result bound by `let`); and the fr12 `channel<number>` class below.

### Two mechanisms, one rule — the shipped runtime release protocol and Phase 4's typeck consume

**What shipped (2026-09-03, `861fd4d`, read from the code — not the commit message).** A spawned task's drop ladder (`SpawnStateFnFuture::drop`, `runtime.rs`) frees the heap CLONES codegen made of its `background` arguments, by descriptor (`BgArgDropEntry { byte_offset, kind, size }`, kinds now shared through `ynz_abi::BG_ARG_KIND_*`). Nothing told the ladder when a clone LEFT the task, so a send of a cloned `rows` was freed by the ladder while the channel still held it — three reproduced use-after-frees (`ch.send(rows)`, `h.send(rows)`, `return rows` through a handle). The fix is a **pointer-identity release protocol**: `release_ladder_payload(drive, bits)` (`runtime.rs`) walks the current drive's descriptors and rewrites any `HEAP_SHAPE`/`HEAP_ARRAY` entry whose frame slot holds exactly `bits` to the terminal `BG_ARG_KIND_RELEASED`, which the ladder skips. The "current drive" is a thread-local `DriveIdentity` (generation + frame + descriptor array) published by `DriveGuard` around every resume-fn poll (`channel.rs`; the sync entrypoint drive publishes a ladder-less identity). It fires at three doors: `channel_send_poll_guarded`'s three accepting outcomes (`try_send` READY, the park to a pending entry, the re-poll READY) via `release_taken_value`, gated on `chan.drop_glue.is_some()` so a `channel<int>` never compares bits; and `HandleStateFnFuture::poll`'s completion extraction for the new `HANDLE_RET_KIND_{VALUE,EC}_HEAP_PTR` return kinds (`handle.rs`, `emit.rs` `lower_let_background_handle`). Both `ch.send` and `h.send` funnel through the one send core, so the channel link is made once. Also shipped: the re-poll of a PARKED send that observes CLOSED now frees the orphaned payload through the channel's glue (the entry was its last owner). Scope: **spawn-argument clones only**; no typeck involvement.

**Record correction — say it plainly.** FRAGO 004's ruling 2 (`audit.md`) and the plan's cold-resume banner describe the hotfix mechanism as *"ladder consults consumption — codegen skips the ladder free for a binding typeck already marked consumed by a send."* **The shipped code does not do that.** It consults no typeck fact; it consults the hand-off EVENT, at runtime, by pointer identity. The commit records why the ruling's literal form was infeasible: (1) a compiled state-machine callee cannot know whether it was spawned (parameter ladder-owned) or reached directly via `wait` (not) — one body, two ownership situations; (2) the 32-byte frame header has no room for a per-slot flag; (3) a codegen ident-match would miss `let alias = rows; wire.send(alias)` and a nested callee sending its caller's parameter. The ruling's INTENT — one authoritative answer threaded to the second consumer, never a ladder taught about typeck or a typeck taught about `BgArgFreeKind` — is preserved: the send core is the one place every hand-off passes through with the sender's identity in hand. But the plan's banner and audit text must be corrected to what shipped; this design builds on the runtime protocol, not on a typeck→codegen link that does not exist.

**The question: two surfaces, one derived answer?** [`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md) bans a second derivation of the same COMPUTED question. Here there is one RULE — *`send()` moves ownership of an owned-heap payload* — and two consumers of it that answer **different** questions:

| | Typeck consume (Phase 4) | Runtime release protocol (shipped) |
|---|---|---|
| **Question answered** | May the SOURCE PROGRAM read this binding after this send? | Does this TASK'S DROP LADDER still own this allocation at task retire? |
| **What it sees** | Every later read of the binding — a compile-time fact only typeck has | The ladder, whether THIS parameter was heap-cloned for THIS spawn, and the actual pointer bits — runtime facts only the runtime has |
| **What it cannot see** | Whether the parameter is ladder-owned (same function body, spawned or `wait`ed). (It DOES see aliases now — `let other = rows` joins one alias class, and a call that returns a piece of its argument is `Reaches` — and it sees a parameter's provenance because `ParamNeedsGive` forces the `give` word that makes every caller's consume happen through the same decision.) | The program's future reads |
| **Failure if absent** | Cross-task ALIASING: sender and receiver hold one mutable value with no synchronization (a race, not a crash) | DOUBLE OWNERSHIP of a clone: the ladder frees what the channel/parent holds (the reproduced UAF) |
| **Authoritative for** | Binding-level use-after-send (the `ConsumedBySend` diagnostic; the source-level "one holder" property) | Allocation-level ownership of spawn-arg clones (no double free, no leak, on every send outcome) |

Neither can answer the other's question, so **neither is redundant and both stay**. Phase 4 does not remove, bypass, or re-implement the release protocol, and it does not add a codegen-side ladder edit "because typeck knows." A future reader proposing to drop the runtime match "now that typeck consumes" must first answer (1)–(3) above; a reader proposing to drop the typeck consume "now that the runtime releases" is proposing to ship the cross-task aliasing race the ownership model exists to forbid.

**The compile-time link that keeps them from drifting.** Both are gated on the same set — *element kinds whose send moves ownership* — and that set is defined ONCE, by `channel_elem_drop` (the `Option<ChannelElemDrop>` above): typeck consumes iff `is_some()`; codegen registers glue from the exhaustive inner match on the same enum, whose arms are function values and cannot be null; the runtime releases and frees iff `chan.drop_glue.is_some()`, which is true exactly when codegen registered glue. Adding an element kind to one side without the other is a non-exhaustive match, not a silent divergence — and adding it with a null-glue arm is a type error, not a leak. The runtime half is additionally pinned by behavior: the owned-heap fixture class asserts, per element kind, BOTH the compile error on read-after-send AND exact alloc/free parity through task retire (`YNZ_ALLOC_COUNTER_OUTPUT`), so a typeck set that outgrows the glue set (a consumed binding whose payload nobody frees) shows as a parity gap, and a glue set that outgrows the typeck set (a released clone whose source binding is still readable) shows as a fixture that compiles when the gallery says it must not. The runtime crate's own gates (`send_of_ladder_owned_payload_releases_ladder_alloc_free_parity` and siblings, `channel.rs`) lock the protocol independent of codegen.

**The CLOSED-first-poll path (SHIPPED v0.3-M8 Phase 4).** `send()` gives its payload on EVERY outcome — typeck consumes before the outcome is knowable, so a refused payload has no source-level owner and is freed by the send path. The free lives in the RUNTIME, not in codegen: a codegen-side free of a ladder-owned clone on CLOSED, beside a runtime that did not release on CLOSED, would be a double free at task retire. `channel_send_poll_guarded` (`crates/ynz-runtime/src/channel.rs`) has THREE first-poll CLOSED arms — (i) `None` under the sender lock, (ii) `try_send → Closed`, (iii) `try_send → Full` then the freshly-boxed endpoint future's first poll returning `Ready(Err(()))` — and all three fall through ONE local `refuse_closed(chan, value_bits)` that, when `drop_glue.is_some()`, calls `release_taken_value()` and then `glue(value_bits)` and returns `CHANNEL_CLOSED`; no arm returns `CHANNEL_CLOSED` except through it, so a fourth closed outcome inherits the free or fails review for not calling it. The payload's fate is decided on ALL outcomes in that one function: READY → the channel owns it; PENDING → the pending entry owns it; CLOSED → freed there, nobody owns it. Codegen's `conduit_closed1`/`conduit_closed2` arms build the error value and free NOTHING. The pre-close doc comment and test case (c) of `ladder_is_untouched_when_the_channel_does_not_take_ownership` ("a CLOSED result on a FIRST poll never releases") were flipped to this contract, and `closed_first_poll_send_frees_the_refused_payload_once_alloc_free_parity` plus the loom model `loom_refuse_closed_releases_the_ladder_slot_then_glues_exactly_once` gate it. Anchor: `git log --grep=m8-p4` (the Phase 4 RED seal and round 1).

### `.copy()` on `map<K, V>` ships in Phase 4 (Patrick's ruling, 2026-09-03)

The `ConsumedBySend` advice and the two refusal WHAT-INSTEADs say `{channel}.send({name}.copy())`, and the consume rule covers `array<T>` **and** `map<K, V>`. Patrick ruled that `.copy()` ships on `map<K, V>` so the advice is executable for every type the rule names — rather than narrowing the rule to arrays or making the diagnostic conditional on the receiver type.

**What `array`'s `.copy()` actually does today (the model `map` mirrors).** `.copy()` is a parser-level postfix op (`PostfixOpKind::Copy`, `parser.rs:3222`), not a method table entry. Typeck (`check_postfix_op`, `check.rs:6939`) admits ANY receiver and returns its type — the "P3c will enforce trivially-copyable" comment there never shipped. Codegen (`emit.rs:19301`) does: `Shape` → memcpy into a fresh alloca; `BuiltinArray` → a genuine **one-level deep copy** — `ynz_array_clone_primitive` for AoS (fresh header + buffer, `len × elem_size` bytes; pointer cells such as `string`/`maybe` elements copy as pointers, so nested data still aliases — the recorded D12/D13 stance), or `soa_copy_to_aos` for an SoA receiver; and **`_ => Ok(recv_val)` for everything else — the receiver's own pointer**. That catch-all is the FRAGO 014 alias-no-op stub that was closed for arrays and is still open for maps: **`table.copy()` on a `map` compiles today and returns `table` itself.** So the advice is not merely unexecutable — shipped as-is, `wire.send(table.copy())` would compile, send the ORIGINAL allocation, and silently reintroduce the aliasing hole the diagnostic exists to close. The independence lock `m5_p5_copy_aos_independent.ynz` exists precisely because "it compiles" proved nothing for arrays.

**What `map`'s `.copy()` is.** A one-level deep copy mirroring the array arm: a new runtime entry `ynz_map_clone(src) -> *mut YnzMap` (alongside `ynz_array_clone_primitive`, `lib.rs`) that allocates a fresh header and fresh `ctrl`/`keys`/`vals`/`insert_order` buffers through `ynz_alloc` (counted, so parity gates see it), byte-copies each (`vals` by `count × elem_size`), and preserves `count`/`capacity`/`order_cap`/`elem_size`; pointer-valued cells copy as pointers (one level, same as arrays). Codegen adds a `Type::BuiltinMap` arm calling it. Typeck's return type is already the receiver type. Registry: `[[primitive_intrinsic]] name = "copy", kind = "method", receiver_type = "map", param_types = [], return_type = "map<K, V>", since = "v0.3-M8"` — the same shape as the existing `array` (`features.toml:1987`) and `fixed` (`:2153`) entries. Fixture: an independence lock in the shape of `m5_p5_copy_aos_independent.ynz` — copy, mutate the original with `.set()`, assert the copy kept the pre-mutation values — because for maps, too, "compiles" proves nothing. **Ordering is an obligation, not a preference: the fixture is authored and committed RED — failing on today's alias no-op, the copy observing the mutation — BEFORE `ynz_map_clone` and the `BuiltinMap` arm land, and flips GREEN in the commit that lands them.** The RED run is the only evidence that the stub existed and that Phase 4 step 3a actually changed it; an independence fixture written after the clone ships proves nothing about what was there before. **The codegen catch-all does not survive for `map`**: the `BuiltinMap` arm is added AND the `_` arm is left for value-bit primitives only; the remaining types that still fall through it (`maybe`, union, `fixed`, `dynamic`) are the same stub class and are recorded as a Future Requirement in the v0.3-M8 plan (FR#10) rather than silently left. [`REF-ownership.md`](../../reference/REF-ownership.md)'s "`.copy()`" section (line 87: "only allowed when every field … is trivially copyable") predates array `.copy()` and is stale; Phase 4 updates it when it writes "Closing a channel".

### Send-after-close is a runtime error, not a compile-time diagnostic

Considered and decided: **no new compile-time diagnostic.** A `send()` after `close()` is only decidable at compile time in the trivial case (same binding, same straight-line block), and the real cases — the close in one task and the send in another, or the close behind a branch — need cross-task flow analysis the compiler cannot do soundly. A check that fires on the trivial case and is silent on every real one would teach the wrong lesson ("the compiler catches this"). The runtime error is the mechanism; its text is the WHAT/WHAT-INSTEAD/WHY below. A Tier-3 lint for the straight-line same-binding case is a legitimate future teaching aid, not a deferral — it has no correctness or cost dimension, and nothing is owed until someone wants it.

**`.close()` on a task handle is deliberately not provided — and the one real gap that leaves is recorded below, not hidden.** The reason is not "the spawner already holds the binding" (that argument applies word-for-word to the shipped `h.send(v)`, which forwards to the SAME `channel_send_poll_guarded` on the first channel argument — `handle.rs:404–411` — and nobody calls `h.send` a second name for `commands.send`). The reason is what each act IS. `h.send(v)` exists so the spawner can address the child without naming the child's channel — it is a message TO the child. `close()` is not a message to anyone; it is a lifecycle act on the channel object, and the object the spawner constructed is what it closes: `commands.close()`. Routing that through the handle would make the child's lifetime and the channel's lifetime read as one thing when they are two — a task can finish with its channel open, and a channel can close with its task still running. Calling `.close()` on a handle hits the existing unknown-method diagnostic, whose available-methods list for a HANDLE stays `send(value), receive()` (the channel's list gains `close()` — Phase 4 must split the string the two receivers share today, see below).

**The gap that decision leaves.** The handle's message channel is the callee's first `channel<T>` PARAMETER (`check.rs:2405–2412`), wired at the spawn from the first channel-typed ARGUMENT (`emit.rs:17431–17442`) — and that argument need not be a binding. The spawn-arg ownership pre-record at `check.rs:2321–2345` records `BgOwnership::Channel` only for `Expr::Ident` args; a non-ident channel expression falls to `bg_arg_is_provably_safe` (`check.rs:1972–2005`), where `BuiltinChannel` is not in `type_provably_not_shape`'s set (`check.rs:2020–2030`), so it is admitted as default-deny `Give`; codegen's `prepare_bg_arg_for_ctx` then shares it by TYPE regardless of expression form (`emit.rs:16737–16746`). Confirmed on the current tree (2026-09-03): `let h = background doubler(makeWire())` followed by `h.send(21)` and `h.receive()` compiles and prints `42`. In that program there is NO spawner-side binding on which to call `.close()`; the only party that can close that channel is the child, through its own parameter — backwards for a command stream. This is a real omission, not a naming question. **Decision: still no `h.close()` in this design** — but the window it leaves is CLOSED by a cheap guard taken now, per [`no-duct-tape.md`](../../../.claude/rules/no-duct-tape.md)'s rule that a deferral with live exposure takes the in-scope guard alongside it rather than leaving the window open until its trigger.

**The guard (Phase 4 ships it; one of the design's four new compile-time diagnostics — error-vs-warning is a hard compile error, ruled 2026-09-03, `git log --grep=FRAGO-009`): `HandleChannelArgNeedsBinding`.** At the handle-form spawn-argument pre-record (`check.rs:2321–2345`), the argument that binds the callee's first `channel<T>` parameter (the parameter `msg_elem` is derived from at `check.rs:2405–2412`) must be an `Expr::Ident`. If it is any other expression — a call, a field access, anything call-materialized — the spawn is a compile ERROR, not a warning: a warning would leave the hang class live for exactly one spawn shape while the fix costs the user one `let` line, and Golden Rule 5 says wrong is caught at compile time. It is an error only on the HANDLE form (`let h = background …`), because that is the form where the spawner is a producer (`h.send`) with no way to end its own stream; the statement form with a call-materialized channel is a channel with a single holder — the child is its only producer and consumer, a degenerate single-task shape, not a cross-task stream — and is left alone. The guard is cheap because everything it needs is already in hand at that site: `call_form`'s argument list, the callee's parameter types, and the ident test the pre-record already performs. Teaching text is in "Teaching text" below. A side benefit: an ident argument is pre-recorded as `BgOwnership::Channel` (shared), whereas a non-ident channel argument falls to default-deny `Give` — so the guard also removes the one path where a channel argument's ownership record was the imprecise default.

The deferral itself, four fields, stands for the handle-side close: **WHAT** — a handle-side close (`h.close()`, forwarding to the same close entry on `msg_chan`). **WHY deferred** — with the guard above, every handle spawn's command channel has a spawner binding, so `commands.close()` is always available and `h.close()` would add only the two-lifetimes conflation for the common case; the call-materialized shape is taught nowhere (every spec and demo example binds the channel first, because the spawner normally also receives from it). **COST** — small: one non-suspending conduit method on the handle receiver forwarding to `ynz_channel_close(handle.msg_chan)`, plus its typeck arm and one fixture. **TRIGGER** — a real program for which binding the channel first is genuinely the wrong shape (none is known), or Phase 9's demo work finding it needs `h.close()`.

### What the runtime change looks like (implementation guidance for Phase 4, not a second spec)

- `sender: Mutex<Option<mpsc::Sender<i64>>>`; `ynz_channel_close(chan_ptr)` = lock, `take()`, unlock, `wake_recv_waiters()`. Null pointer is a no-op (matches `ynz_channel_free`/`purge_pending_sends`).
- `channel_send_poll_guarded`'s first-attempt path (`channel.rs`, the `lock_or_recover(&chan.sender).clone()` line): under the ONE sender lock, `None` → `CHANNEL_CLOSED` before any clone; `Some` → clone, unlock, then `try_send` exactly as now. The lock section is the send's linearization point (contract item 2): a send that already holds a clone when `close()` takes the `Option` is a pre-close send and lands. The re-poll path is untouched (an in-flight future owns its own clone). **On ALL THREE first-poll closed outcomes** — `None` under the lock; `try_send → Closed` (`channel.rs:503`); `Full` then the fresh future's first poll `Ready(Err)` (`channel.rs:554`) — through one `refuse_closed` fallthrough, when `chan.drop_glue.is_some()`: `release_taken_value()` then `glue(value)` — the payload's fate is decided here, see "Two mechanisms, one rule" and the P2-3 bullet below.
- **`release_ladder_payload`'s kind filter (`runtime.rs:933`) is hand-maintained and unlinked — Phase 4 links it.** It releases only `HEAP_SHAPE`/`HEAP_ARRAY`, which is consistent today only because maps are never ladder-cloned; the day FR#9 clones a map under a new `BG_ARG_KIND_HEAP_MAP`, that filter silently skips it and the use-after-free class returns through the same door the hotfix closed. The fix: ONE predicate in `ynz-abi`, beside the constants — `pub const fn bg_arg_kind_is_releasable_payload(kind: u64) -> bool`, defined by INVERSION (`kind != BG_ARG_KIND_SHARED_CHANNEL && kind != BG_ARG_KIND_RELEASED`), so a new heap kind is releasable by default rather than skipped by default — consumed by `release_ladder_payload`'s filter; plus `pub const ALL_BG_ARG_KINDS: &[u64]` and a `ynz-runtime` parity test that, for every kind in it, asserts `is_releasable_payload(kind)` iff the ladder's free match (`runtime.rs:1125`) frees a synthetic counted allocation of that kind (alloc/free parity per kind). The free match's defensive `_ => {}` no-op (`runtime.rs:1155`) gains `debug_assert!(!bg_arg_kind_is_releasable_payload(desc.kind))` so a releasable kind with no free arm fails the first debug run that retires a task carrying it, instead of leaking silently. Two consumers, one predicate, one parity test — never a second hand-listed kind set.
- `ynz_channel_recv_poll`'s `Ready(None)` exit: drain `recv_waiters` (item 6 above). The register-before-poll ordering (M6 P3-2) is unchanged. The runtime's return code is unchanged too (`CHANNEL_CLOSED`); what changes is codegen's `ChanRecv` closed arms, which build a `none` `maybe<T>` instead of calling `ynz_unhandled_error`, and its ready arms, which wrap the payload in the same `maybe<T>` envelope every `.exists()`/`.value` site already reads — never a conduit-specific envelope twin. **The envelope's alloca goes in the function's ENTRY block, via `alloca_in_entry_llvm` (`emit.rs:~2270`), never at the insertion point.** The existing maybe builders (`build_maybe_some` is `#[allow(dead_code)]` with no live producer, `emit.rs:2382–2383`; its sibling likewise) call `build_alloca` wherever the builder currently sits, and the conduit's join block `conduit_post` (`emit.rs:12785`) sits INSIDE the `while` body of the canonical consumer loop — so an insertion-point alloca would grow the resume function's stack by 16 bytes on every iteration of every `receive()` loop. One slot per `receive()` site, hoisted once per function, stored per receive.
- The `Drop` impl and `purge_pending_sends` are unchanged — closure does not alter who frees what. The shipped release protocol (`release_ladder_payload`, `DriveGuard`, the handle-return release) is unchanged EXCEPT the CLOSED-first-poll extension in the P2-3 bullet below.
- **P2-3 rides here, and is now sound — in the RUNTIME, not in codegen's closed arms.** The two codegen closed arms for `send` (`emit.rs:12848–12876` / `12923–12947`, `conduit_closed1`/`conduit_closed2`) currently build the error and branch to `post` without freeing a heap-typed `value_bits`. They were unreachable; after this design they are the live send-after-close path — and they STAY free-less. The rejected payload is freed inside `channel_send_poll_guarded`'s first-poll CLOSED outcomes — all THREE of them (`None` under the lock; `try_send → Closed`, `channel.rs:503`; `Full` then first-poll `Ready(Err)`, `channel.rs:554`), through the one `refuse_closed` fallthrough — using the SAME per-element drop glue the channel registered at construction (`chan.drop_glue`), immediately after `release_taken_value()` has told the sender's ladder to let go — the same two-step the shipped re-poll-CLOSED arm already performs for an orphaned parked payload. Any closed arm that does not go through the fallthrough is P2-3's leak again. One drop path (the registered glue), one owner-decision site (the send core, on all three outcomes), never an ad hoc `ynz_array_drop` twin and never a codegen-side free the ladder does not know about ([`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)). That free is sound ONLY because `send()` now gives its payload (contract item 9): the sender's binding is consumed at typeck, so the send core frees a value with no source-level holder. The first draft freed it in codegen while the sender still held it — a use-after-free the review caught before any code was written; the second draft would have freed it in codegen while the LADDER still held it — a double free, caught by reading the hotfix. Phase 4 flips the shipped runtime doc comment ("a CLOSED result on a FIRST poll never releases") and test case (c) of `ladder_is_untouched_when_the_channel_does_not_take_ownership` to the new contract, and adds the CLOSED-first-poll alloc/free parity gate.
- **Typeck: three sites, not one.** `close` is NOT added to `CHANNEL_SUSPENDING_METHODS` (`suspension_source.rs`) — it never suspends, and adding it would make the may-block fixpoint over-approximate every function that only closes a channel into a state-machine function. It is the first non-suspending conduit method — the case anticipated by the known-method-set comment inside `check_conduit_method_call` (`crates/ynz-typeck/src/check.rs:3988–3992`: "If a future non-suspending conduit method ships … extend THIS site to union it in explicitly"). But `known` (`check.rs:3993–3998`) is only the first of three sites Phase 4 must touch: (a) the receiver-discipline guard (`check.rs:4011–4030`, "needs the channel held in a named binding") and the derivable-conduit guard (`check.rs:4032–4082`) run UNCONDITIONALLY after `known`, and both justify themselves with "this operation can suspend" — `close` cannot, so both must be gated to the suspending set, or `close` on a legitimately-typed receiver is rejected with a WHY that is false for it; (b) the unknown-method WHAT-INSTEAD string (`check.rs:4003`, `"Available methods: send(value), receive()."`) is SHARED between channel and handle receivers, while this design wants the channel's list to read `send(value), receive(), close()` and the handle's to stay `send(value), receive()` — Phase 4 splits it per receiver type. The receiver must still be a `channel<T>`-typed expression.
- **The loom substrate exists (M8 Phase 3, 2026-09-03 — `audit.md` entry `m8-p3-20260903-a1` in the M8 plan directory).** `crates/ynz-runtime/src/sync.rs` is the one site `channel.rs`/`handle.rs` import `Arc`/`Mutex`/`MutexGuard` from (a bare `std::sync` re-export in every production build — proven byte-equivalent by an LLVM-IR diff at introduction; loom's under `RUSTFLAGS='--cfg loom'`; `runtime.rs`'s Tokio-lifecycle `RUNTIME` static keeps its own `std::sync::Mutex` by name, a documented exemption in `sync.rs`), and `crates/ynz-runtime/src/loom_tests.rs` is the harness: six models driving the real shims/keyed core/purge helper/drop ladder, each with a revert-proven failure mode reported by the model's own assertion — including the kind-2 drop-ladder arm's purge-BEFORE-release order, which a co-owner probe asserts directly in both ladder models (fix round 2; before it, a swapped arm passed the live-co-owner model and only crashed the last-reference one). Tokio's own mpsc stays a std black box under that cfg (tokio gates its loom paths on `all(test, loom)`); the loom-only `YnzChannel::mpsc_witness` makes each Tokio call one dependent atomic step so their orderings are exhaustively explored. The two channel-close interleavings this section names (`close()`-vs-`send()` under the sender lock, and `close()`-wakes-parked-receiver) are pure `ynz-runtime`-owned lock/waiter logic and land as models in that file when Phase 4 implements them.

### Deferred: auto-close on last-producer drop (four fields)

- **WHAT**: the channel closing itself when the last binding that can `send()` goes out of scope, so a producer that simply returns ends the stream without writing `close()`.
- **WHY deferred**: it requires (1) a per-binding producer/consumer role analysis that does not exist, (2) the compiler-wide scope-exit drop-insertion pass that does not exist (no local is released at scope end — `ynz_handle_free` is (declared in `runtime_decls.rs`'s `RuntimeDecls`, zero call sites in `emit.rs`); the same gap v0.3-M8 Phase 7 / the roadmap's never-drop-locals row, fr13–fr17, turn on — **Phase 7 ruled 2026-09-04 (Branch B): the pass does not exist and the task-retirement ladder is not it, so this deferral's drop-pass dependency is NOT satisfied by M8; its trigger stands, and its remaining blockers are unchanged, (1) and (3) plus the pass itself**), and (3) splitting the runtime's single `Arc` holder count into producer vs. holder counts. Shipping explicit `close()` first costs the user one line per producer and is correct today; shipping auto-close first would be a refcount redesign with a dead-on-arrival trigger until (2) lands.
- **COST to add later**: medium — the role analysis (a `send`-reachability classification per channel binding, per task), the runtime producer count (one `AtomicUsize` decremented from the drop ladder for producer-classified holders), and a compatibility rule that explicit `close()` and auto-close compose (close-once semantics already make that trivial: whichever fires first closes, the other is the no-op).
- **TRIGGER**: the general scope-exit drop pass shipping (Track 3 / the drop-story milestone), AND a real workload where forgetting `close()` produced a hang in practice — the explicit form is the teaching-correct default until the compiler can prove the producer set.

### fr12 — `channel<number>` decimal128 marshalling (designed v0.3-M8 Phase 2 per FRAGO 009; Phase 4 implements; signed off 2026-09-03 with the rest of Phase 2 — `audit.md`'s SIGN-OFF record)

Patrick assigned fr12 (roadmap Capability Ledger, Idempotency-Key `…#8-fr12: conduit-send-decimal128-marshalling`) to v0.3-M8; FRAGO 009 ruled it rides Phase 2. The close design is element-type agnostic (contract item 8): closed-ness lives on the object, payloads cross as one opaque i64 slot (`channel.rs:204–214`, `mpsc::channel::<i64>`), and the only per-element hook is the drop glue registered at construction. fr12 is the OPPOSITE axis — how a 16-byte value crosses a 64-bit slot — and the three decisions FRAGO 009 asked for are these.

**Representation: a counted 16-byte heap cell minted at the send, freed at the receive.** `number` (precision ≤ 34, the hardware decimal128 `i128` — `emit.rs:1151`, `:1477`) lives on the sender's frame; its address dies with the frame, which is why `check_channel_construction` rejects the element type today (`check.rs:4286–4297`, registry `channel-element-heap-upgrade`). The send lowering copies the 16 bytes into a fresh `ynz_alloc(16)` cell through the ONE existing helper `number_to_heap_cell` (`emit.rs:3802` — the same mechanism the `background` argument path already uses for a decimal128 arg, `emit.rs:16780–16800`) and passes the cell's pointer bits in the slot. The receive lowering loads the 16 bytes out of the cell into the receiver's own `i128` storage (a `sm_crossing_decimal128_set` two-slot frame local when it crosses a suspension, an alloca otherwise — the existing paths) and frees the cell immediately, before the `maybe<number>` envelope is built; the cell never outlives the receive statement. Drop glue: `ChannelElemDrop::NumberCell → ynz_number_cell_free` (a `ynz_free(ptr, 16)` behind a named C-ABI entry so the glue table's arms are all function values; it reuses the counted free the bg-arg ladder's `HeapShape { byte_size: 16 }` arm already performs for the same cell shape), so residual buffered cells are freed at teardown and a refused payload is freed by `refuse_closed` on every CLOSED first poll — the runtime's `release_taken_value()` finds no ladder slot holding these bits (the cell is new, never a ladder-owned clone) and is a harmless no-op. `h.send(number)` gets the same marshalling for free: the handle form funnels through the one send core, and the handle's `-> number errors` return already travels as an `i128` pair (`emit.rs:202–206`), untouched.

**Weighed and rejected**: (a) sending the pointer of a ladder-owned decimal128 bg-arg cell directly to save one allocation — reintroduces the double-ownership question the hotfix's release protocol exists to answer, for a 16-byte win; (b) two i64 slots per element — breaks the one-slot buffer contract (`mpsc::channel<i64>`, capacity semantics, every `send`/`receive` ABI) for one element kind; (c) widening the slot to i128 for all channels — touches every element kind's ABI to serve one. Precision > 34 (bignum) is a heap/global string pointer that already survives the frame (the bg-arg path leaves it to the by-pointer default arm); it stays REJECTED at construction — fr12 is decimal128 only, and the registry entry's `triggers` text says so.

**`number` does NOT join the give set — it stays copy-through.** `number` is a value type (`is_trivially_copyable`, `types.rs:242`); the cell the send mints is a fresh allocation nobody else holds, so the sender's binding remains its own 16 bytes and stays usable — exactly like `int`. This is why `ChannelElemDrop` carries `transfers_source()` ("Which element types" above): `NumberCell` has glue but does not consume; `Array`/`Map` have glue and do. Parked item 3's assumption that `number` "must join the give set" was made before the marshalling was designed and is withdrawn: joining it would make `wire.send(price)` consume a value-typed binding for no memory-safety reason — a teaching error the user could not explain from the type. `ConsumedBySend`, `ParamNeedsGive` and `TransferNeedsCopy` never fire on a `number` channel; `wire.send(order.total)` (a field) is admitted, because there is nothing to own.

**Alloc/free consequence, and the gate that proves it.** One counted 16-byte `ynz_alloc` per successful or suspended send, one counted free per receive; residual cells freed by teardown glue; refused cells freed by `refuse_closed`. Phase 4's fr12 fixture asserts exact alloc/free parity (`YNZ_ALLOC_COUNTER_OUTPUT`) across send-then-receive, send-then-drop-with-buffered-element, send-after-close, and a parked send drained after close, plus value round-trip (`2.5` in, `2.5` out, through a suspension on the receiving side so the two-slot frame path is exercised). The typeck gate is lifted for `Type::Number { precision <= 34 }` only, by extending `channel_elem_drop`, from which `elem_supported` is derived ("Which element types") — never by editing the `elem_supported` list. **`shape` elements stay deferred**: a shape's inline copy shares any pointer-cell field (an `array` field) between sender and receiver, so admitting shapes is the depth-two transfer question the container-store deferral in [`IMP-ownership.md`](IMP-ownership.md) owns, not a marshalling question; the registry entry `channel-element-heap-upgrade` narrows to `shape` (and bignum `number`) with its `substitute`/`why`/`triggers` rewritten to say `number` shipped in v0.3-M8. The gallery line `examples/primantis-orders/v0_3_m4_errors.ynz:98` (`channel<number>` as a rejected construction) is retired and replaced by a `channel<Player>` trigger so the error class keeps a gallery instance.

### Registry entries this design needs (Phase 4 adds them; recorded here so the kind is not re-litigated)

- **`[[primitive_intrinsic]]`** with `kind = "method"`, `receiver_type = "channel"`, `name = "close"`, `return_type = "nothing"`, `since = "v0.3-M8"`. The existing `send`/`receive` conduit methods are NOT in the registry today (they live only as `CHANNEL_SUSPENDING_METHODS` in typeck) — a pre-existing SSOT gap the feature-registry rule says to close at the moment of discovery, so Phase 4 registers all three together and the typeck method set derives from the registry rather than the reverse. The two back-filled entries carry this design's types: `receive` → `kind = "method"`, `return_type = "maybe<T>"`; `send` → `kind = "method_1arg"`, `param_types = ["T"]`, `return_type = "nothing errors"`.
- **`send`'s give semantics need a home in the schema.** `[[primitive_intrinsic]]` has no ownership field today (`registry/features.toml:581–587`). Phase 4 adds one optional field, `param_ownership` (a list aligned with `param_types`; absent = today's by-value/shared semantics), and sets `param_ownership = ["give"]` on `send`, so the LSP hover says "send gives its value" from the registry rather than from a typeck constant. The alternative — a comment on the entry — was weighed and rejected: a comment is not data, and hover text derived from it would be a second source.
- **`[[deferred_language_feature]]`** `name = "channel-auto-close-on-last-producer"` carrying the four fields above verbatim (`design_doc` = this file).
- **`[[deferred_language_feature]]`** `name = "background-handle-close"` carrying the four fields from "The gap that decision leaves" above verbatim (`design_doc` = this file); its `substitute` field names the guard: bind the channel first and call `commands.close()`.
- **`[[primitive_intrinsic]]`** `name = "copy"`, `kind = "method"`, `receiver_type = "map"`, `param_types = []`, `return_type = "map<K, V>"`, `since = "v0.3-M8"` — Patrick's ruling, see "`.copy()` on `map<K, V>` ships in Phase 4" above. Same shape as the existing `array`/`fixed` `copy` entries.
- **FOUR `[[diagnostic_template]]` entries.** `kind_name = "ConsumedBySend"` — the use-after-send compile diagnostic (text in [`IMP-ownership.md`](IMP-ownership.md) "Teaching text"; slots `{name}`/`{channel}`/`{sent}`/`{via}`); canonical and reusable (it fires for every owned-heap send followed by a read), which is exactly the template kind's criterion. `kind_name = "ParamNeedsGive"` — a parameter given away (sent into an owned-heap channel, or passed to a `give` position) without a `give` declaration; `{name}`/`{fn}`/`{modifier}`/`{act}`/`{type}`/`{copy_form}` varying; it REPLACES the inline share-parameter refusal at `check.rs:4611–4616`, whose text is retired (Phase 4 updates whatever gallery line or snapshot asserts the old share-refusal wording — `v0_3_m4_errors.ynz` is the first place to look). `kind_name = "TransferNeedsCopy"` (Phase 2's name; the Phase 1 draft called it `SendPayloadNeedsCopy` and it was never registered) — a payload at ANY transfer sink that someone else still reaches; `{act}`/`{expr}`/`{type}`/`{reason}`/`{fix}` varying. `kind_name = "HandleChannelArgNeedsBinding"` — the handle-spawn guard (text below); same criterion, `{callee}`/`{expr}` varying.
- **fr12's entries.** `modify [[deferred_language_feature]] name = "channel-element-heap-upgrade"` — narrowed to `shape` elements and bignum `number` (precision > 34): `substitute` drops the field-wise workaround for `number`, `why` states decimal128 shipped in v0.3-M8 via the send-minted cell, `triggers` names `channel<Player>()` only. No new `[[primitive_intrinsic]]` (the element type widens; `send`/`receive` are unchanged). The runtime gains `ynz_number_cell_free` as a C-ABI entry — registry-invisible (compiler-internal glue, carve-out). While registering it, Phase 4 reconciles a pre-existing drift found on the same site: the existing `Consumed` template (`features.toml:1599–1603`, "cannot be used again" / "Use `.copy()` before giving a value away…") and the code that emits it (`check.rs:3624–3629`, "cannot be used here" / "Create a new value or use `.copy()`…") disagree in both WHAT and WHAT-INSTEAD; the emitting site becomes the one consumer of both templates, selected by consumption cause, so the registry is the source for both.
- **Still no `[[diagnostic_template]]` for the runtime messages.** The channel-closed text is a RUNTIME error-value message emitted per op in codegen (`closed_msg`), not a `DiagnosticKind` compile diagnostic; the template kind is for reusable compile-time diagnostics. Decided, not forgotten.
- **No new keyword, banned word, or type constant.**

### Teaching text (Golden Rule 11 — the strings Phase 4 ships)

Runtime error carried by the typed value on **`send()` after close** (replaces the current closed-send text, which describes a receiver being "gone" — after this design the common cause is a `close()`, and the text must say so):

> **WHAT**: This channel is closed — `close()` was called on it, so no new values can go in.
> **WHAT INSTEAD**: Send everything before calling `close()`, or bind the result — `let sent = wire.send(value)` — and check `sent.failed()` to stop producing when it fails.
> **WHY**: `close()` is the signal that says "nothing more is coming" so the receiving side can stop waiting. A value sent after that signal would arrive after the receiver already decided the stream ended, so the send is refused rather than silently lost.

**`receive()` after close-and-drain has no message — it returns `none`.** There is no error text because there is no error: `none` is the same value every `maybe<T>` in the language uses for "nothing here," and the consumer reads it with the `.exists()` / `.value` / `.or()` forms it already knows. The teaching happens in the spec's "Closing a channel" subsection (Phase 4) and in the demo's consumer loop, not in a runtime string.

The one-line runtime message on send-after-close (what the typed value's `.message` reads, and what an unhandled error prints) — kept short because the call trace already names the site:

- send: `The channel is closed — close() was called, so this value cannot be delivered. Check .failed() on the send, or send everything before close().`

**The three TRANSFER diagnostics — `ConsumedBySend`, `ParamNeedsGive`, `TransferNeedsCopy` — have ONE home: [`IMP-ownership.md`](IMP-ownership.md) "Teaching text (Golden Rule 11; three diagnostics, one emit site)".** They fire at every transfer sink (a channel send is one of them), so their text lives with the sink-neutral rule, not here. Phase 1 drafted them in this section (`de631bf` has that text); Phase 2 re-derived the rule, kept the `ConsumedBySend` and `ParamNeedsGive` wording with one slot each added (`{via}` for a class-mate read; the chain form of `{act}`), and replaced `SendPayloadNeedsCopy` — whose WHAT ("not a named binding, so the compiler cannot tell who else holds it") was the shape enumeration speaking — with `TransferNeedsCopy`, whose `{reason}` slot names exactly who still holds the value. The cannot-give-const refusal is **extracted** from `check_arg_ownership`'s inline `format!` (`check.rs:4602–4607`) into a helper with a sink-supplied WHAT-INSTEAD, and the send sink supplies `Send a copy instead: {channel}.send({name}.copy()).` — the existing "declare it with `let`" advice is wrong for a send, where the caller keeping the value is the whole point. The share-cannot-be-given refusal (`:4611–4616`) is NOT extracted: it is the `share` instance of `ParamNeedsGive` and is retired into that template. `.copy()` is real for both element kinds the rule covers once `map`'s ships (above).

The channel-specific worked example the gallery renders (`ConsumedBySend` on a send followed by a read):

```ynz
let rows: array<int> = [1, 2, 3]
let sent = wire.send(rows)
print(rows.count())
// COMPILE ERROR: `rows` was sent into `wire` and cannot be used here — `send()` gave it away.
//   If you still need `rows` after sending it, send a copy instead: `wire.send(rows.copy())`.
//   If you only need it before the send, put this line above the `send()`.
//   Why: a channel hands the value to whichever task receives it, and that task may already
//   be reading or changing it by the time this line runs. If both sides kept the same value,
//   two tasks could change it at once and neither would know. So send() gives `rows` away,
//   and the compiler refuses to build any line that reads it afterward. Nothing happens at
//   runtime — `rows` is not cleared or set to none; the program simply does not compile until
//   this read is placed above the send() or the send takes a copy.
```

With `give rows` declared on `producer` (the program under "Ownership must flow through the call"), `entrypoint`'s `print(rows.count())` is the existing use-after-give error at the consumed-read site — the parent's binding was consumed by `wait producer(wire, rows)` through the one transfer decision, exactly as if `rows` had been given to any other `give` parameter; without `give`, `producer` gets `ParamNeedsGive` on its send line.

**New compile-time diagnostic — handle-spawn channel must be a binding** (`HandleChannelArgNeedsBinding`; fires at the handle-form spawn pre-record `check.rs:2321–2345` when the argument binding the callee's first `channel<T>` parameter is not a plain identifier; `{callee}` is the spawned function, `{expr}` the offending argument's source text):

> **WHAT**: `{callee}` gets its command channel from `{expr}`, which is not a named binding — nothing outside the task can ever close that channel.
> **WHAT INSTEAD**: Bind the channel first, then spawn: `let commands = {expr}` and `let h = background {callee}(commands, …)`. Call `commands.close()` when you are done sending.
> **WHY**: A task handle can send into its channel (`h.send(value)`) but cannot close it — only the channel binding can. Without a binding, a task waiting on `receive()` can never be told the stream has ended, so it waits forever.

Worked example, for the gallery:

```ynz
let h = background doubler(makeWire())
// COMPILE ERROR: `doubler` gets its command channel from `makeWire()`, which is not a named
//   binding — nothing outside the task can ever close that channel.
//   Bind the channel first, then spawn: `let commands = makeWire()` and
//   `let h = background doubler(commands, …)`. Call `commands.close()` when you are done sending.
//   Why: a task handle can send into its channel (h.send(value)) but cannot close it — only the
//   channel binding can. Without a binding, a task waiting on receive() can never be told the
//   stream has ended, so it waits forever.
```

Existing compile-time diagnostics that `.close()` extends (no new diagnostic class):

- unknown-method WHAT-INSTEAD list, on a CHANNEL receiver, becomes `Available methods: send(value), receive(), close().`, and its WHY gains one clause: `close()` marks the channel finished so a receiver knows when to stop. On a HANDLE receiver the list stays `send(value), receive().` (the string is shared today at `check.rs:4003`; Phase 4 splits it).
- `.close()` with arguments: `` `.close()` takes no arguments, but got {n}. `` / `Call it bare: wire.close()`. / `close() only marks the channel finished — there is nothing to pass in.`

### Cross-references

- [`docs/reference/REF-concurrency.md`](../../reference/REF-concurrency.md) "Channels" — gains a "Closing a channel" subsection in Phase 4 (spec files show only what ships, so it lands with the code, not with this design).
- [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Channel/Queue Primitives" — the bounded-by-default half of the channel design; close is the end-of-stream half.
- `crates/ynz-runtime/src/handle.rs` `outbox_tx: Option<mpsc::Sender<..>>` — the shipped precedent this reuses.
- `crates/ynz-runtime/src/runtime.rs` `release_ladder_payload` / `DriveIdentity`; `crates/ynz-runtime/src/channel.rs` `DriveGuard` / `release_taken_value`; `crates/ynz-abi/src/lib.rs` `BG_ARG_KIND_RELEASED`, `HANDLE_RET_KIND_{VALUE,EC}_HEAP_PTR` — the shipped spawn-arg release protocol "Two mechanisms, one rule" builds on (`git log --grep=861fd4d` / `fix/bg-arg-channel-send-uaf`).
- v0.3-M8 plan Future Requirement #9 (`bucket.add(rows)`, the aliased-container door) — the FOURTH escape door. This design closes its CHANNEL instance (an aliased bg arg sent by its task — `ParamNeedsGive` forces the `give` that consumes the parent's binding) and does NOT close the container instance; see "Ownership must flow through the call — What this closes in Future Requirement #9".
- v0.3-M8 plan (`.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/plan.md`) Phases 1 and 4; Safety invariants "Channel-close semantics cannot regress…".

### What is signed off (Phases 1 and 2 both closed, 2026-09-03)

**Signed off (Phase 1, narrowed by FRAGO 008 — `git log --grep=m8-p1`):** the close mechanism, the name, `receive()` → `maybe<T>`, idempotency, wake-all, in-flight completion, drop-unchanged, the `refuse_closed` collapse, P2-3's free in the runtime, `Option<ChannelElemDrop>`, `HandleChannelArgNeedsBinding` as a hard compile ERROR, and the four-field auto-close deferral.

**Signed off (Phase 2, `audit.md`'s SIGN-OFF record — "Patrick signed off Phase 2"):** the transfer rule itself ([`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value"), the Auto-Arc topology and beneficial-emission condition, and fr12's `number`-stays-copy-through decision with the `transfers_source()` split of `ChannelElemDrop`. Nothing named in this section remains open — both design gates that block Phase 4 (and Phase 5) are closed; the twelve-item packet ruling and the confirmation list are the Phase 2 sign-off packet in the v0.3-M8 plan's `audit.md`, not here.

---

## Design Divergences (v0.3-M3b Auto-Parallelization Pass)

The following divergences from the full design doc are documented per `no-duct-tape.md` — each names the concrete cost and the reversal path.

### Same-callee concurrent calls run sequentially (suspending/I/O members only)

**Scope**: applies to M3b I/O-overlap parallelization only.  M3d CPU parallelization LIFTS this restriction (see the Same-Callee Amendment above): two data-independent calls to the same CPU callee DO run in parallel, because CPU spawns are keyed by `(group, member-index)` with a per-invocation ctx rather than by callee name.

**What the design says**: independent operations auto-parallelize regardless of which function they call.

**What the v0.3 pass does**: two calls to the same *suspending* function (e.g., `worker(1)` followed by `worker(2)`) are lowered sequentially, even when they are data-independent.  This does NOT apply to CPU calls: two data-independent `crunchStat(x)` + `crunchStat(y)` (or `fib(40)` + `fib(41)`) parallelize, since the CPU path uses per-invocation ctx + member-index slot keying.

**Named cost**: the I/O sub-frame allocates one slot per unique suspending callee name. Two concurrent invocations of the same *suspending* callee would require two slots with a disambiguation scheme — a separate concern in `build_frame_layouts`. Calling the same suspending helper twice in a row doesn't parallelize in v0.3; it runs sequentially, which is always correct.  CPU members pay no such cost.

**Reversal path**: extend `build_frame_layouts` to allocate N sub-frame slots per suspending callee name when a suspending function is called more than once in a straight-line block, keyed on (callee_name, invocation_index) — mirroring the (group, member-index) keying the CPU path already uses. The independence analysis and join codegen are already class-agnostic; only the I/O frame-layout phase needs the extension.

### Write-effect uses a TYPE-BASED conservative floor — mutable-heap arguments never parallelize

**What the design says** (`Reads vs Writes — Ownership Does Double Duty`): "the compiler traces through user functions to determine if they contain writes" — independent writes to *different* resources auto-parallelize.

**What the v0.3 pass does**: the independence analysis treats a call argument as a potential aliased write iff its parameter **type** is a mutable heap reference (`shape`/`array`/`map`/`maybe`/union/`dynamic`). It does **not** attempt to classify per-call-site whether the callee actually mutates the argument. Any suspending call carrying a mutable-heap argument is sequenced against every other statement in the block; only no-argument, primitive-argument, and immutable-`string`-argument suspending calls parallelize.

**Why a type-based floor and not a precise per-form classification**: a precise classification was built (a name-based AST fixpoint over the call graph) and **removed**. Across three adversarial gate rounds it missed five distinct write forms — a mutating method on a `share self` receiver, a mutating method on a parameter directly, mutation through a *field or index* of the parameter (`p.items.add(x)`), a builtin mutator shadowed by a same-named user function, and a local `let`-alias of the argument (`let twin = p; twin.add(x)`) — each a silent runtime miscompile (the analysis classified a real write as a read, so two aliased writes auto-parallelized and reordered observably). The root cause is structural: proving a heap argument is read-only requires receiver types **and** alias tracking — i.e. a full type+alias-aware borrow checker — which an AST-walking pass is not. The `share`-read-only premise the precise analysis leaned on is itself only partially enforced (the same five forms escape typeck's `share` check), so the auto-parallel soundness cannot rest on it. The type-based floor sidesteps the entire class: a mutable-heap argument is *unconditionally* a potential write, sound regardless of how the callee uses it.

This follows the golden rules directly: **Golden Rule 5 (compile-time soundness — never a silent runtime miscompile) outranks Golden Rule 10 (efficiency-first).** When the two conflict, soundness wins.

**Named cost**: two suspending calls that each take a mutable-heap argument do not overlap, even when both only READ their argument (e.g. two suspending reads of distinct shapes). This forfeits the read-only-mutable-heap-argument overlap. It is a perf miss, never a correctness miss — the floor only ever *adds* sequencing, never introduces a wrong parallel execution. The I/O-overlap headline is unaffected: no-argument, primitive-argument, and immutable-`string`-argument suspending calls (the realistic I/O-fan-out shapes) still parallelize.

**Reversal path**: a real type+alias-aware ownership analysis (M4 borrow-checker completion — receiver types threaded through the analysis + intra-procedural alias tracking) that can PROVE a heap argument is both read-only AND non-aliased re-enables narrowing the floor to permit proven-read mutable-heap arguments to overlap. Until that exists, the floor stands. (M3d CPU-parallel reuses this same type-based write-effect source — the floor transfers unchanged.)

### Bare `channel<T>` closes with `.close()` (SHIPPED v0.3-M8 Phase 4 — the M6 Phase 7 "never closes" entry is retired)

The v0.3-M4/M6 divergence recorded here — `YnzChannel` held both endpoints for its whole life, so a fan-in consumer's `receive()` parked forever after the producers finished — no longer describes the tree: v0.3-M8 Phase 4 shipped the explicit `.close()` action, bare-channel `receive()` typed `maybe<T>`, close-wakes-every-waiter, idempotent double-close, and the runtime `refuse_closed` free. The contract, the runtime shape, and the one named deferral (auto-close on last-producer drop) live in "Channel Close — End-of-Stream Semantics" above; the user-facing text is [`REF-concurrency.md`](../../reference/REF-concurrency.md) "Closing a channel". This pointer stays so a reader of the old entry lands on the current design rather than on two contradictory records.

### `share` read-only enforcement is best-effort (transitive teaching error)

**What ships**: typeck rejects mutation of an explicit `share` parameter — direct field/element assignment, mutating collection methods (`.add`/`.set`/...), `share self` receivers, passing a `share` argument to an explicit `lend`/`give` position, and the common transitive case (`fa(share x) { helper(x) }` where `helper` mutates) via the `effective_ownership` fixpoint.

**Known-incomplete (deferred to a real ownership analysis)**: the same five forms the auto-parallel floor sidesteps also escape the *transitive* `share` check — a mutation through a field/index of the `share` param via a bare callee, a builtin mutator shadowed by a same-named user function, and a local `let`-alias of the `share` param. These are **missed compile-errors, not miscompiles** — the program still runs soundly (the type-based floor sequentializes regardless), it simply doesn't receive the teaching diagnostic. The `share`-enforcement completeness is decoupled from auto-parallel soundness on purpose: soundness is the floor's job (complete); the `share` error is a teaching aid (best-effort). **Reversal path**: the same M4 type+alias-aware ownership analysis closes both at once.

---

## Runtime Implementation (Internal — Developer Never Sees This)

Thread pool sized to CPU cores. I/O operations use the OS event system (epoll/kqueue/IOCP). The compiler's dependency graph determines scheduling. Developers never configure or think about any of this.
