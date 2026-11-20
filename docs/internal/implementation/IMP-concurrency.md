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

## Suspension vs. Ordering — What's Automatic and What `wait` Does (LOCKED)

This is the authoritative statement of what `wait` means. Two different jobs were historically conflated — they are separate, and only the second is `wait`'s job.

**Suspension is automatic. You never write `wait` for it.** A call that can block (transitively reaches an I/O / may-block operation — today that is `sleep`) is compiled into a suspension point by the compiler's whole-program may-block analysis (no function coloring — see [`docs/internal/implementation/IMP-no-function-coloring.md`](IMP-no-function-coloring.md)). The function suspends and hands its thread back to the scheduler automatically. The IDE shows the inferred suspension as the muted `wait_points` hint. The user does **not** type `wait` to make a call suspend correctly.

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

**Why this is the locked default (Model A)**: independent writes to different resources auto-parallelize (maximal throughput for write-heavy work), and the ordering responsibility for the residual causal cases is on the user via `wait`. Decided: preserving source order for *all* writes by default is rejected (it leaves throughput on the floor for intensive work). The accepted cost: independent side effects can race unless ordered, surfaced through the IDE concurrency hints. In Yinz, `wait` IS ordering.

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
wait request.post(`stripe.com/charge`, data)
request.post(`email.com/send`, receipt)
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
monitor.send(`get-status`)
let status = monitor.receive()
```

**One keyword for both patterns**: `background` is the only keyword; keeping the handle (`let monitor = background ...`) or not expresses the intent, so developers do not choose between two keywords up front.

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

Developers can always force an independent copy explicitly: `background process(data.copy())`. There is no body-level `.give` form (ownership words are signature-only — [`IMP-ownership.md`](IMP-ownership.md) read-first note; `PostfixOpKind` is `Copy | Freeze`, checked against the parser, `git log --grep=m8-p2`); the give direction is what the compiler already does for a value the caller never reads again.

The IDE warns on large copies so developers can restructure if needed.

| Scenario | Compiler does | Cost |
|---|---|---|
| Value unused after background | `.give` (move) | Zero |
| Value used after background | `.copy` (clone) | Copy cost |
| Small value (String, Int) | `.copy` | Trivial |
| Large value, unused after | `.give` | Zero |
| Large value, used after | `.copy` + IDE warning | Developer restructures or accepts |
| Developer writes `.copy()` at the spawn | `.copy()` — independent copy for that task | Copy cost |
| ≥ 2 spawns in one block read the same shape value, nothing writes it between them | one refcount-shared copy (auto-Arc — [`IMP-ownership.md`](IMP-ownership.md) "Auto-Arc — Sharing Topology") | One copy + an atomic bump per task |
| Callee declares `share`/`lend` on a non-channel parameter | COMPILE ERROR (`check.rs:~3470`) | — |

---

## Scope Exit and `background` Tasks — What Is Released and When a Task Stops

Every function, a `background` task's included, releases its heap locals at every exit edge by ownership: typeck lists each binding's fate per edge (`ReleasePlan`, `crates/ynz-typeck/src/release_plan.rs:1229-1240`; `Fate`, `:160-167`) and codegen emits the list (`emit_release_edge`, `crates/ynz-codegen/src/emit.rs:30830-30836`, and `emit_release_edge_with`, `:30841-30871`). The one rule for what is released there, the transfer rule at an exit edge, lives in [`IMP-ownership.md`](IMP-ownership.md) "The transfer rule at an exit edge"; this section states only what it means across a `background` boundary.

- **A value given, sent or stored is not released by the giver.** A `give` argument of a `background` call is owned by the task, and the task releases it at its own scope exits (or the spawn ladder does, at a position the worker does not release). A value sent on a channel belongs to the receiver from the `send`; the channel's element drop glue frees one nobody received (`channel_drop_glue`, `emit.rs:22239-22294`; the element kinds are `ChannelElemDrop`, `crates/ynz-typeck/src/types.rs:356-389`, chosen by `channel_elem_drop`, `types.rs:393-401`). A value given on only some paths is released by the paths that kept it (`Fate::ReleaseIfLive`, `release_plan.rs:160-167`; the take nulls the founder's slot, `MoveSite`, `release_plan.rs:200-209`).
- **A channel binding's reference is released at its scope exit** (`ReleaseStep::Channel` calls `ynz_channel_free`, `emit.rs:29565-29569`). The spawner's reference and each task's reference are counted holders of one channel; a release says a holder left, never which end it was ("Channel Close" below).
- **A `background` task handle leaving scope stops its task.** The handle local's release is `ynz_handle_free` (`ReleaseStep::Handle`, `emit.rs:29570-29574`, in `emit_release_body`, `emit.rs:29484-29583`; the runtime entry, `crates/ynz-runtime/src/handle.rs:511-548`), which aborts the task at its next suspension point: a task parked in `wait` does not resume, and what it would have done afterwards never runs. There is no detach and no join rule. A task that must finish keeps its handle live until then (`handle.receive()`). The frame, the argument copies and an unreceived result are each freed exactly once. [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Task Cancellation" owns the semantics, and `examples/pirates-roster/entrypoint.ynz` shows it running.
- **A fire-and-forget `background f()` has no handle**, so nothing leaves a scope: it runs to its end, or until `entrypoint` returns and the runtime stops every task still running at its next suspension point.
- **A CPU task cannot be stopped.** It has no suspension point, so a handle dropped over a running CPU task lets it finish and keeps its argument context until it ends ([`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Task Cancellation", the CPU-bound edge case).

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
function saveOrder(lend db: Database, order: Order) -> Nothing errors {
  db.insert(`orders`, order)           // sequential — same lend target
  db.insert(`auditLog`, order.log)     // sequential — same lend target
}
```

**Two connections = concurrent by default** (different `lend` targets):
```
function crossDbSync(lend db1: Database, lend db2: Database) -> Nothing errors {
  db1.insert(`users`, user)            // concurrent with db2 — different lend targets
  db2.insert(`usersBackup`, user)      // concurrent with db1
}
```

Use `wait` if ordering between connections matters:
```
wait db1.insert(`users`, user)
db2.insert(`usersBackup`, user)
```

**Connection pool**: pool-managed connections are handled by the pool — the compiler doesn't control which physical connection each call gets. Pooled calls to the same logical database may parallelize. Transactions handle ordering within a connection at the DB level; the compiler's auto-sequencing aligns naturally with transaction behavior.

Full database stdlib design (connection pooling, transactions, query builder, migrations) is tagged MVP2.

---

## Scope Boundaries — Deliberate Constraints and Deferrals

Scalars, shapes, strings and arrays can cross a `wait` (the `LocalCrossesWait` guard is lifted for them). The cases below stay deliberate compile errors.

### `ShadowsCrossingLocal` — same name re-declared around a suspension (deferred, clean compile error today)

A `let` binding that re-uses a name that already has a crossing-local frame slot is a compile error today. The guard is **safe-conservative**: it rejects any program where two bindings share a name around a suspension boundary, even when one of them might technically be unreachable from the other.

Two shapes are rejected:

**Shape A — nested shadow**: an outer `let x` before a `wait` AND a `let x` inside a nested block (if/while/for/match body) in the same function, where the outer `x` is read after the suspension resolving to the outer binding.

```
// ❌ — x crosses the wait AND is re-declared inside the if
function broken() -> Nothing {
  let x = 10
  wait sleep(5)
  if (someCondition) {
    let x = 20        // compile error — shadows crossing local x
    print(x.toString())
  }
  print(x.toString())
}

// ✅ — rename the inner binding
function fixed() -> Nothing {
  let x = 10
  wait sleep(5)
  if (someCondition) {
    let innerX = 20
    print(innerX.toString())
  }
  print(x.toString())
}
```

**Shape B — top-level redeclaration**: an outer `let x` before a `wait` AND a second `let x` at the TOP LEVEL of the function body after the suspension. The type checker rejects it as a language rule, even when all post-wait reads resolve to the redeclared binding. (Codegen now gives each binding its own slot, see "Per-binding slot identity" below, so the rule no longer protects against a shared slot.)

```
// ❌ — x=10 and x=99 both at top level, separated by a wait
function broken() -> Nothing {
  let x = 10
  wait sleep(5)
  let x = 99     // compile error — two top-level bindings share one frame slot
  print(x.toString())
}

// ✅ — use distinct names
function fixed() -> Nothing {
  let x = 10
  wait sleep(5)
  let xAfter = 99
  print(xAfter.toString())
}
```

The same two shapes apply to **parameters**: a parameter `p` occupies a frame slot at function entry.

- **Shape A (nested shadow)**: a `let p` inside any nested block (if/while/for/match body) is rejected in a suspending function. All nested param shadows in suspending functions stay rejected (`crates/ynz-typeck/src/check.rs:2838-2912`): the guard is unchanged language behaviour, and codegen no longer shares the parameter's slot with the inner binding (see below).
- **Shape B (top-level redeclaration)**: a `let p` at the TOP LEVEL of the function body shares the parameter's frame slot and is rejected regardless of whether the inner binding is read post-wait.

**Per-binding slot identity (the guards above stay as the language rule)**: slots, crossing allocas and `cg.locals` are still keyed by name, but codegen reads the module with a unique name per binding. Typeck's resolver records every declaration (`let`, `for` variable, desugared destructure alias, parameter) and which declaration each identifier use resolves to (`TypedModule::binding_names`, `BindingNames`, `crates/ynz-typeck/src/binding_names.rs:53-67`; naming rule `:16-22`); the first declaration of a name in a function keeps it, later ones become `name__N` (N unused anywhere in the function). `TypedModule::with_codegen_names` (`crates/ynz-typeck/src/check.rs:1023-1042`) applies the map to a clone through `BindingNames::apply` (`binding_names.rs:85-105`) — diagnostics, lints and the LSP keep the user's names — and fails hard when an identifier naming a local binding has no resolution entry. Any pass that looks a binding up by name after this point (e.g. a scope-exit release list) must use the unique name or the declaration key from that map. So the nested-shadow programs the guards do not examine (nested outer bindings, `for` variables, destructure aliases, non-crossing shadows) now get one slot per binding.

**Non-async functions**: a `let p` shadowing a parameter in a function that does NOT contain any `wait` is allowed — Yinz permits shadowing per [`docs/internal/implementation/IMP-linting.md`](IMP-linting.md) (`shadowed-variables` Tier-3 lint). The conservative guard only applies to suspending (async) functions where parameters are frame-slotted.

**Why the guard is still there**: slots, crossing allocas and `cg.locals` are keyed by name, and typeck now makes every binding's name unique for codegen, so the collisions the guard was written for cannot happen in generated code. The guard (`ShadowsCrossingLocal`) is kept unchanged because removing it is a language change, not a codegen fix. The user-level workaround is still: use distinct names.

**What it costs to lift** (small): delete the `ShadowsCrossingLocal` check and its parameter mirror in typeck and update the spec. Slot identity needs no further codegen change (per-binding names).

**Trigger**: user demand for re-using a name around a suspension in a program that cannot be restructured.

**Workaround** (always applies): rename any binding that re-uses a name already in use across a suspension boundary — two values with different semantics should have different names anyway (Golden Rule 4).

### `NestedShapeCrossing` — a shape with nested shape fields crossing a `wait` (deferred)

A `shape` whose fields are themselves `shape` types cannot cross a `wait` when those fields contain heap-allocated children (e.g. strings, arrays). The frame-embed codegen writes struct bytes directly into the composed frame; for nested shapes, this only copies the OUTER struct's bytes — any inner shape pointers that point into separately-allocated or stack regions become dangling after the suspension.

```
// ❌ (currently) — inner shape crosses a wait
shape Inner { value: Int }
shape Outer { child: Inner, score: Int }

function example() -> Nothing {
  let o: Outer = { child: { value: 42 }, score: 100 }
  wait sleep(5)         // compile error — Outer.child is a nested shape
  print(o.score.toString())
}

// ✅ — flatten into primitive fields
shape FlatOuter { childValue: Int, score: Int }
```

**Why deferred**: the memcpy that stages the outer struct bytes doesn't recursively follow inner shape pointers. A correct implementation would either (a) walk the entire shape graph and embed all nested structs transitively in the frame, or (b) heap-allocate inner shapes and store pointers (with a drop guard). Both require non-trivial additions to the frame layout and drop subsystem. The flat-fields workaround always applies.

**What it costs to lift** (1–2 sessions): extend frame layout to recursively compute embedded nested-shape slot regions; add recursive memcpy at definition and reload sites; add a drop-on-cancel path for any heap-allocated inner shapes. Each of these is a well-contained change, but they need to be consistent across the frame-layout, codegen, and runtime layers.

**Trigger**: a user program that requires crossing a nested-shape local without a flatten workaround.

### `WideValueSuspendingReturn` — shape returns from suspending functions (deferred, clean compile error today)

A suspending function (one whose body contains a `wait`) cannot yet return a `Shape` or `Shape errors` value by value. These two return types require a variable-size EC/shape return-staging slot entangled with the pre-existing non-suspending shape-return base bug.

**`-> Number errors` is fully supported.** A 16-byte staging slot is reserved in the composed frame (after own-local slots, before child sub-frames) when the function returns `-> Number errors`. The resume function writes the i128 decimal to that slot, points the EC ok-word at it, and the staging slot is freed when the frame drops (alloc=1/free=1 — no leak). See the frame-layout comments in `crates/ynz-codegen/src/emit.rs` `build_frame_layouts`.

**Why each remaining variant fails without the guard**:

- **`-> Shape`** (non-crossing shape literal or call result): the old codegen staged the shape bytes at `FRAME_OFFSET_LOCALS_START` (frame offset 32). Offset 32 is where child sub-frames are embedded — writing there overwrites the sleep sub-frame's `resume_point` field, causing a `SIGSEGV` at the next `rt_async_sleep_poll` call.

- **`-> Shape errors`** (shape success value in an EC return): needs variable-size staging (shape size varies per declaration) and is also entangled with the pre-existing non-suspending `-> Shape` return base bug (shapes returned by value produce garbage for int fields even without suspension). Fixing the non-suspending bug first is the prerequisite.

**What IS supported**:

| Return type | Suspending function | Status |
|---|---|---|
| `-> Int`, `-> Boolean`, `-> Float` | yes | CLEAN — scalar, no staging needed |
| `-> Number` (plain) | yes | CLEAN — i128 stored directly in the 16-byte return slot |
| `-> Number errors` | yes | CLEAN — i128 in 16-byte frame staging slot; alloc=1/free=1 |
| `-> Int errors` | yes | CLEAN — `{err=0, ok=int_bits}` stored directly |
| `-> String`, `-> Array<T>`, `-> Map<K,V>` | yes | CLEAN — heap-stable pointer, `ptr_to_int` safe |
| `-> Shape` (crossing local) | N/A (frame-backed, not a return) | CLEAN — frame-embedded crossing locals work |

**Workarounds**:
- For `-> Shape`: return the shape's fields individually as primitives, or bind the shape to a crossing local and return a derived primitive.
- For `-> Shape errors`: return each field individually (e.g. `-> Int errors`), or compute a primitive result inside the function and return that.

**What it costs to lift Shape/Shape-errors** (~1 session): fix the non-suspending `-> Shape` return base bug first (shapes returned by value silently produce garbage for int fields), then add variable-size staging to the frame layout for the suspending case. The staging region size is the shape's ABI size (computed by the shape-size table). The return-path GEPs to the region after the resume function memcpys the shape bytes there.

**Trigger**: a suspending function (body contains `wait`) whose declared return type is `Shape` or `Shape errors`.

---

### `UnsupportedCrossingLocalType` — types that cannot yet cross a `wait` (deferred, clean compile error today)

Several types cannot yet cross a `wait` boundary. The type classifier is complete for the supported set; any unhandled type produces a clean WHAT/WHAT-INSTEAD/WHY compile error rather than a silent miscompile, UAF, or SIGSEGV.

**Why each variant fails without the guard**:

The frame-slot classifier in codegen handles Int, Boolean, Float, Number, String, Array, Map, Shape, and ErrorsCapable crossing locals. All others fall into buckets with safety problems:

- **`Maybe<T>` / `dynamic Contract`**: Internally represented as a pointer to a `{tag, payload}` struct alloca on the RESUME FUNCTION'S STACK. Flushing the alloca pointer to the frame slot and reloading it after the next resume stores and re-reads a dangling stack address. LLVM may detect this ("Instruction does not dominate all uses!") or silently corrupt the stack.

- **`union` (envelope union)**: a crossing union binding, used after the `wait` only as an argument to a suspending call, is held in a frame region of its own (the union slot, payload inline, [`IMP-unions.md`](IMP-unions.md) "Stored union slot"): a bind persists the value into the region and every resume rebuilds the view from it, so no stack pointer is flushed (`crossing_local_slot_count`, `crates/ynz-codegen/src/emit.rs:14814-14854`; the checker skips a union-annotated local whose only crossing is an argument, `crates/ynz-typeck/src/check.rs:2472-2480` and `:2626-2631`). Any other use of a crossing union is refused (`check.rs:2632-2708`).

- **`Fixed<T>` binding (crossing local)**: fixed arrays are stack-allocated allocas in the resume function's stack frame. When the resume function returns to the scheduler after a `wait`, that stack frame is freed. The crossing-local frame slot holds a dangling pointer — reading elements after resume is UB. (`Fixed<T>` as a for-loop iterator is caught separately by `FixedArrayIterWithWait`.) Registry entry: `fixed-crossing-local-with-wait`.

- **`Fixed<T, N>` in a function that waits or may run as a task**: such a function is built once, so a `fixed` parameter without a written length has no length for its body. It is refused where the body asks for the length (`FixedLengthMissingInTaskFunction`, `unsized_lengths_of_once_only_functions`, `crates/ynz-typeck/src/check.rs:1463-1517`; pinned by `a_function_built_once_is_refused_an_unsized_list_it_asks_the_length_of` in `crates/ynz-typeck/tests/fixed_length_signatures.rs`); a function that never asks for it is not refused (`a_function_built_once_that_never_asks_for_the_length_is_not_refused`, same file). A `Fixed<String>` (or any `fixed` whose items own heap) received from a task and kept across a later `wait` is a `cannot yet cross a `wait`` error, because its release would walk cells in a dead resume-function stack frame (`is_errors_of_fixed_with_heap_items`, `check.rs:12398-12408`; the guard, `check.rs:2568-2579`; `a_received_fixed_of_strings_cannot_cross_a_later_wait`, same test file). **Both are refused; supporting them is undecided.**

- **`range` binding (crossing local)**: a range binding (`let r = range(0,3)`) is stored on the resume function's stack as a pair of bounds (i64 lo, i64 hi). When the function suspends the stack frame is freed; the crossing-local frame slot holds a dangling pointer on the next resume. Iterating that dangling range produces zero iterations — silent wrong output. The wait-inside-body form is caught separately by `StoredRangeWithWait`; this guard catches the case where the wait is at the TOP LEVEL before the loop. Registry entry: `range-crossing-local-with-wait`.

- **`MapEntry<K,V>` (for-loop var over a map)**: the loop variable in `for (entry in m)` is a `{key, value}` struct pair. The current frame-slot mechanism assigns ONE i64 slot per crossing local — fine for scalar/pointer types, but only covers `entry.key` (field[0]). `entry.value` (field[1]) has no slot and reads garbage after resume. Registry entry: `map-entry-fields-after-wait`.

**What IS supported** (crossing locals that work correctly):

| Type | Status | Frame strategy |
|---|---|---|
| `Int`, `Boolean`, `Float` | CLEAN | scalar i64/i1/f64 slot |
| `Number` (decimal128) | CLEAN | 2 consecutive i64 slots (lo+hi) |
| `string`, `Array<T>`, `Map<K,V>` | CLEAN | heap-stable pointer stored as i64 |
| `Shape` (primitive fields only) | CLEAN | frame-embedded struct bytes, region rounded up to 16 bytes |
| `T errors` (ErrorsCapable) | CLEAN | 2-slot {err, ok} struct |

**Frame-embedded shape regions are 16-byte aligned at runtime.** The composed frame lays
crossing locals out at 8-byte slot granularity, but a shape's LLVM struct carries the
ABI alignment of its widest field — 16 the moment it has a `number` (i128) field — and
every access through the shape's region pointer (field reads, the whole-struct copy a
`background` spawn makes for its task, any callee the shape is passed to) is emitted at
that ABI alignment; those sites cannot downgrade the claim the way a bare i128 slot does
(`FRAME_I128_SLOT_ALIGN`), because a callee's `scene.scale` read cannot tell a frame
shape from a heap one. So the frame honors the claim: `shape_frame_region_ptr` (the ONE
producer of the region pointer, read by every consumer from the crossing local's ptr
alloca) rounds the address up to `FRAME_SHAPE_REGION_ALIGN` (16), and
`shape_frame_slots` reserves `FRAME_SHAPE_REGION_SLACK_SLOTS` (one 8-byte slot) per
embedded shape so the rounded region still fits. The rounding is dynamic rather than a
static layout rule because a child sub-frame is embedded at an arbitrary 8-multiple
offset inside its parent, and it is stable across resumes because a task's frame never
moves. Codegen refuses to lower a shape whose measured ABI alignment exceeds 16 (the
compile-time link between the constant and the TargetData truth). Locked by the
`bgarg-number` hotfix (`git log --grep=bgarg-number`): a shape with a `number` field copied for a `background` spawn from an 8-mod-16
frame region made optimized ISel emit `movaps` and SIGSEGV; `--no-optimize` hid it
because `-O0` lowers i128 ops alignment-indifferently.

**Blocked types and workarounds**:

| Type | Workaround |
|---|---|
| `Maybe<T>` / `dynamic Contract`, and a `union` used after the `wait` other than as an argument to a suspending call | Extract the inner value before `wait`; use the primitive after |
| `Fixed<T>` binding (crossing `wait`) | Redeclare as `Array<T>` — heap-allocated, pointer survives suspension |
| `range` binding (crossing `wait`) | Inline the range in the `for` header: `for (i in range(0,n))` |
| `MapEntry<K,V>` after `wait` | Read `entry.key`/`entry.value` before `wait`, bind to separate `let k`/`let v`, use those after |

**What it costs to lift each**:
- `maybe`/`dynamic`: per-type flush/reload strategies (store tag + payload fields to separate frame slots). ~half a session each. (`union` as a call argument already holds its slot in a frame region.)
- `Fixed<T>` crossing local: embed the fixed-array bytes directly in the composed heap frame (similar to Shape frame-embedding), using compile-time size to compute slot count. ~1 session.
- `range` crossing local: store lo/hi bounds as two consecutive i64 frame slots and reconstruct the range alloca on reload. ~half a session.
- `MapEntry<K,V>`: two consecutive frame slots (mirroring the ErrorsCapable 2-slot path). ~half a session.

**Trigger**: a local of any blocked type that is declared before a `wait` and read after it.

---

### `ECWrapperResultCollection` — collecting the result of a `background`-spawned `-> T errors` task (built, with `background-handle-form`)

The standalone EC wrapper (emitted for `background`-spawned suspending `-> T errors` functions) reconstructs the `{i64, i64}` EC struct from the frame's return slot and then calls `free_frame`. For `-> Number errors`, the ok-word points into the composed frame's 16-byte staging slot, which `free_frame` frees, so the wrapper's returned ok-pointer is invalid after it returns. This is safe for the fire-and-forget `background` statement, which discards the EC result without dereferencing the ok-pointer.

A caller that collects the result must copy it before `free_frame`. The handle form `let h = background ecFn()` does this. The inline-poll path (a suspending caller composing another `-> T errors` suspending function through the state-machine resume path) gives each `let` binding its own per-binding stable storage for a wide ok-value (copy-on-bind), so two bindings of the same `-> Number errors` callee never share its staging slot.

**How collection works**: the completion value is extracted from the frame's return slot inside `HandleStateFnFuture::poll`'s Ready arm — strictly BEFORE the embedded future's `Drop` frees the frame. A frame-interior wide ok-value (the `-> Number errors` staging slot) is copied to a handle-owned heap buffer and the ok-word repointed at it; the buffer is freed exactly once, at handle drop. One refinement over the paragraph above's prediction: the collected-vs-discarded distinction is keyed at COMPILE time on the spawn form (a handle spawn always extracts + copies; the bare fire-and-forget spawn path is byte-for-byte untouched) — there is deliberately NO runtime "was `.receive()` called yet" conditional anywhere. See `crates/ynz-runtime/src/handle.rs` ("R8 — copy-before-free, compile-time spawn-form-keyed").

The registry deferral entry `ec-wrapper-collect-on-completion` is retired (see the retirement note in [`registry/features.toml`](../../../registry/features.toml)); the design record is this section plus `crates/ynz-runtime/src/handle.rs`.

---

### `FixedArrayIterWithWait` — `Fixed<T>` array iterator in a suspending for-loop (deferred, clean compile error today)

A `for` loop that contains a `wait` cannot use a `Fixed<T>` array as its iterator.

```ynz
// COMPILE ERROR (FixedArrayIterWithWait):
let flags: Fixed<Boolean> = [true, false, true]
for (b in flags) { wait sleep(5) }

// WORKS: use Array<T> instead
let flags: Array<Boolean> = [true, false, true]
for (b in flags) { wait sleep(5) }
```

**Why**: `Fixed<T>` arrays are stack-allocated (`build_alloca([N x i64])`) in the current resume-function's stack frame. The crossing-local mechanism stores the array's pointer in the composed heap frame slot via `ptr_to_int`. When a `wait` suspends the function and the resume function returns to the Tokio scheduler, the resume function's stack frame is freed. On the next resume call, the pointer is reloaded from the heap frame slot via `int_to_ptr`, but it now points to freed stack memory. Reading the array elements after the first suspension produces undefined behavior — silently wrong values or memory corruption.

**Workaround**: use `Array<T>` instead. `Array<T>` is heap-allocated via `ynz_array_new` and its pointer remains valid after suspension.

**What it costs to lift** (~one session): two options: (a) heap-allocate `Fixed<T>` when used as a for-loop iterator in a suspending function (changes semantics — fixed arrays would no longer be unconditionally stack-allocated); (b) embed the fixed-array bytes directly in the composed heap frame (similar to Shape crossing-local embedding), using the array size at compile time to compute the slot count. Option (b) is architecturally cleaner but requires computing `size * sizeof(element)` slots and storing bytes in consecutive frame slots.

**Trigger**: a `for` loop whose body contains a `wait` and whose iterator resolves to `Type::BuiltinFixed` — either a variable annotated as `Fixed<T>` or an inline `[...]` literal in a `Fixed<T>` context.

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

### `Array<Shape>` with runtime-computed fields across `wait`

An `Array<Shape>` crossing local whose literal contains a runtime-computed field value (e.g. `{ id: 1, qty: a }` where `a` is a variable) is accepted. By-value inline element storage puts the shape element bytes directly in the array heap buffer (variable slot size = element ABI size from `shape_abi_sizes`), with no per-element pointer indirection, so the buffer survives suspension like any other crossing pointer local, whether field values are literals or runtime-computed.

```ynz
// WORKS:
let a: Int = 10
let items: Array<Item> = [{ id: 1, qty: a }]   // runtime field: qty = a
wait sleep(5)
for (it in items) { print(it.qty.toString()) }  // prints the real value — no dangle
```

**Acceptance coverage**: `crates/ynz-driver/tests/integration.rs` — `m5_p3_array_shape_runtime_field_crossing_runs` (the acceptance signal for this case), plus `m5_p3_array_shape_between_waits_runs`, `m5_p3_array_shape_nested_if_runs` and the literal-field boundary (`v03_m3a_p3_array_shape_literal_crossing_still_works`). Design record: [`docs/internal/implementation/IMP-collections.md` — "Array element storage — by-value inline"](IMP-collections.md#array-element-storage--by-value-inline-v03-m5).

---

## Permanent Positional Constraints on `wait`

Two restrictions on `wait` are **permanent design decisions**, not temporary limitations. Both are enforced at typeck and stay in force whatever happens to the `LocalCrossesWait` and `WaitInsideLoop` guards.

### `SubExprSuspendViolation` — suspending call in a sub-expression position

A suspending call nested inside a larger expression is a compile error:

```
// ❌ — inner() is inside a + expression
let x = 1 + inner()

// ✅ — give it its own line
let result = inner()
let x = 1 + result
```

**Rationale**: Step-by-step style — one operation per line with a named variable — is Yinz's deliberate design (Golden Rule 9). Keeping each suspending call on its own statement also enables the auto-parallelization of independent statements: two `let a = wait fa()` / `let b = wait fb()` lines can be analyzed as independent and parallelized automatically. Expression-position suspension would require a different, more complex codegen path that buys nothing over the step-by-step style.

This guard is not a codegen limitation. It is a style constraint enforced at the language level.

### `MutualSuspensionCycle` — mutually-recursive suspending functions

Two or more different functions that call each other AND all suspend is a compile error:

```
// ❌ — ping and pong mutually call each other and both suspend
function ping(n: Int) -> Nothing { wait sleep(10); pong(n - 1) }
function pong(n: Int) -> Nothing { wait sleep(10); ping(n - 1) }

// ✅ — self-recursion works correctly
function countdown(n: Int) -> Nothing {
  if (n > 0) { wait sleep(100); countdown(n - 1) }
}

// ✅ — restructure: one function delegates to the other without suspending
function step(n: Int) -> Nothing { wait sleep(10) }
function loop(n: Int) -> Nothing { if (n > 0) { step(n); loop(n - 1) } }
```

**Rationale**: Self-recursive suspending functions work correctly — a function calling itself is always self-contained (the recursive frame is a heap-boxed child of the same function, and the drop guard walks the chain). Mutually-recursive suspending cycles require per-frame size metadata to safely cancel mid-wait, and the cases that arise in practice can always be restructured into non-mutual forms. The guard is permanent because the restructured form is always cleaner and the mutual-recursion case is rare.

---

## CPU Statement Parallelization

### What This Is

The I/O-overlap pass auto-parallelizes independent suspending (I/O) calls by interleaving them in the
state-machine's async event loop.  The CPU-parallelization pass extends that to **pure CPU-bound** calls: when two
or more independent, CPU-heavy statements appear in the same straight-line block, the
compiler schedules them on separate hardware cores concurrently, then collects results
before the next dependent statement runs.

The user writes exactly what they'd write without the pass:

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
   it or calls a suspending function), its statements are scheduled by the I/O-overlap
   path, not this pure-CPU path — **unless** the state machine's top-level body itself admits
   a *mixed* CPU+I/O group, in which case poll-path fusion (below) spawns the CPU members and
   inline-polls the I/O members through one shared continuation.  A suspending host with no
   qualifying top-level mixed group still runs its statements sequentially through the
   ordinary I/O-overlap path.

2. **Two or more independent groups exist** — the independence analysis (the same analysis the
   I/O-overlap pass uses) finds a partition where at least one group holds ≥2 statements that share no data
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

- **The function calls a suspending helper** — it will be a state machine; the I/O-overlap pass handles it,
  unless the state machine's top level admits a mixed CPU+I/O group, in which case poll-path fusion fuses
  it (see below) — this pure-CPU path itself still never fires on a suspending host either way.
- **All groups are singletons** — nothing to parallelize in parallel with.
- **No member does real work** — per rule 3 above, cheap helpers are not worth spawning.
- **Auto-parallelization is disabled** — the `--no-auto-parallel` build flag (and `kernel` mode,
  which has no scheduler to spawn onto) turns promotion off entirely.  The flag is currently
  carried internally via the `YNZ_NO_AUTO_PARALLEL` env var read by both the promotion query and
  codegen; threading it as an explicit salsa input (so `ynz watch`/LSP codegen honor it without an
  env var) is a deferred mechanism.  It is the program-wide override
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

The CPU-parallelization pass uses a conservative worth-it proxy rather than runtime profiling:

- The function's call graph is inspected for loops (`Stmt::For`, `Stmt::While`) and for
  recursion (self-recursion `f → f`, or a mutual cycle of size ≥ 2 in the call graph).
- If either is found, the callee is marked `does_real_work = true` in the signature table.
- Only `does_real_work` callees appear in CPU parallel groups.

This proxy avoids the need for PGO or runtime cost models at the expense of potentially
missing some genuinely-expensive functions that have neither loops nor recursion.
That is acceptable: skipping a promotion is always correct; a false-positive promotion adds
spawn overhead to a cheap function (soundness issue, not a miscompile — but wasteful).

Future: PGO-based cost models (v0.6+) will replace the proxy with profile-guided thresholds.

### Same-Callee CPU Calls

The same-callee restriction does **not** apply to CPU members.  Two data-independent calls to the
SAME CPU callee — `crunchStat(a)` + `crunchStat(b)`, or `fib(40)` + `fib(41)` — DO run in
parallel: each spawn gets a per-invocation ctx, and the handle/result slots are keyed by
`(group, member-index)` rather than by callee name, so the two invocations never alias each
other's frame slots.  The independence analysis (`partition_groups_classified`) gates the
same-callee branch on `MemberClass::Suspending` only, so CPU members skip it entirely.

The restriction REMAINS for suspending (I/O) members: two calls to the same suspending function
still collapse to sequential, because the I/O sub-frame is keyed by callee name (one slot per
unique callee).  See "Design Divergences" below for the I/O cost and reversal path.

---

## Mixed CPU+I/O Overlap — Poll-Path Fusion

### What This Is

The I/O-overlap pass overlaps independent I/O (suspending) calls; the CPU-parallelization pass
overlaps independent pure-CPU calls. Poll-path fusion lets the two mix: a function containing
BOTH one CPU-heavy call and one suspending call runs them concurrently when nothing depends on
the other's result, as Model A requires ("independent operations run concurrently — this is the
default and the maximal-performance choice," see "Suspension vs. Ordering" above). Model A has
no member-count or work-class cap, and the typeck partition machinery
(`partition_groups_classified` / `MemberClass::{Suspending, Cpu}`) describes both classes
sharing one continuation.

A single top-level group can mix CPU members (spawned onto the blocking
pool, joined via `ynz_rt_join_poll`) and I/O members (inline-polled via an embedded child
sub-frame) under ONE shared continuation. Every resume of that continuation re-drives every live
CPU handle AND every pending I/O sub-frame in one pass — never a blocking join anywhere (no
`block_on` bridge; see
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
time is `max(cost)`, not `sum(cost)`. The user writes nothing special for the overlap.

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
   pure-CPU or pure-I/O path, unchanged.
4. **No two `Suspending` members share the same callee name** — the
   same-callee-I/O restriction (above) still applies within a fused group; `Cpu` members are
   unaffected ("Same-Callee CPU Calls" above).
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
   work, not a correctness requirement.

Sequential fallback is always correct: a function that fails any rule above lowers exactly as it
did without fusion (unchanged pure-CPU or pure-I/O path, or fully sequential).

### Decline Rules (what stays sequential)

- **A NESTED mixed group** (inside `if`/`while`/`for`/`match`) — the existing pure-CPU nested
  machinery's frame-slot reasoning does not yet extend to a nested fused group.
- **A parameter-bearing host** — rule 7 above.
- **Two members of the same suspending callee** — rule 4 above (the
  same-callee-I/O restriction, unchanged by fusion).
- **A loop-body group, or a function with a second (multi-group ≥2) parallelizable run** — the
  same decline shapes as the pure-CPU path, unchanged; a mixed group is not a new escape hatch for either.
- **A wide-EC-return member** (`Shape errors` / `number errors` / any return class the
  `YnzCpuResult` ABI cannot carry) — never admitted as a CPU member of a fused group, same as the
  pure-CPU path's own `cpu_result_abi_supports` gate.
- **A non-scalar argument** (anything other than a bare `IntLit`/`Ident`) — rule 2 above.

Each of these is a scoped, permanent-for-now decline with its own trigger and cost (multi-group ≥2,
loop-body groups, and same-callee I/O overlap are the pure-CPU and I/O-overlap declines this section already
cross-references above; nested fused groups and the parameter-bearing bar are fused-specific). Each of these shapes runs
fully sequential without fusion too — fusion only ADDS a fused path for shapes that were
never fusable otherwise; it never narrows what already fires.

---

## Channel Close — End-of-Stream Semantics (the ownership half lives in [`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value")

**Status**: built (`git log --grep=m8-p1` / `git log --grep=m8-p2` for the design rounds; `crates/ynz-runtime/src/channel.rs` `ynz_channel_close` / `refuse_closed`, typeck's `close` arm and `Maybe<T>` receive, codegen's `conduit_recv_env` envelope; fixture class `crates/ynz-driver/tests/v03_m8_channel_close.rs`). Two facts to know before the subsections: the runtime's pointer-identity release protocol on the spawn-argument drop ladder is the substrate ("Two mechanisms, one rule" below states how it and typeck's consume divide the ownership question, and why the closed-send free lives in the runtime); and the authoritative statement of *who else holds this value* (provenance, binding events, alias classes, the sink list, whole-chain reporting, `dynamic Contract` coverage) lives in [`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value". The subsections below keep the channel-specific facts (which element kinds transfer, the runtime release protocol, the closed-path free) and cite that section for the rule.

### The decision in one paragraph

`Channel<T>` gains one non-suspending action, **`.close()`**: "no more values will be sent." After `close()`, every `send()` — from any task — returns the typed channel-closed error (the channel-closed arm of the `send() -> Nothing errors` surface, which `close()` makes reachable), and `receive()` keeps delivering whatever is still buffered, then returns `none` once the buffer is empty. A second `close()` is a safe no-op. Two type-level changes ride with it. **`receive()` on a bare channel changes type from `T` to `Maybe<T>`**, so the end of the stream is a normal value the consumer checks — never a hang, and never an error, because running out is not failing. **`send()` gives its payload**: for an owned-heap element type (`Array<T>`, `Map<K, V>`) the sent binding is consumed at the send, which is what makes freeing a refused payload on the closed path sound and closes a pre-existing cross-task aliasing hole — and, so that the consume reaches the value's real owner, **a parameter that is sent must be declared `give` on its function, and only a named binding (or a fresh `.copy()`/literal) may be sent**. Auto-close when the last producer goes away is NOT part of this design — it is a named deferral below, with the missing substrates that gate it.

### Why closure is structurally unreachable today (the mechanism the design changes)

`YnzChannel` (`crates/ynz-runtime/src/channel.rs`) is one `Arc`-shared object that owns BOTH endpoints of one Tokio bounded mpsc channel: `sender: Mutex<mpsc::Sender<i64>>` and `receiver: Mutex<mpsc::Receiver<i64>>`. Tokio's two closure signals are "every `Sender` dropped AND buffer drained" (`poll_recv → Ready(None)`) and "the `Receiver` dropped" (`try_send → Closed`). Because the object holds one of each for its whole life, and the object only dies when the LAST `Arc` reference is released, neither signal can fire while any holder can still call anything. The substrate tests reach closure only by `std::mem::replace`-ing an endpoint under `#[cfg(test)]`.

Two further facts bound what any fix can be:

- **Holders have no role.** Every holder — the spawning function and each `background` task it passed the channel to — holds the SAME `Arc<YnzChannel>` (`ynz_channel_share` per spawn argument; `ynz_channel_free` from the drop ladder at task retirement). The strong count counts holders, not producers vs. consumers. The language surface has no role either: a `Channel<T>` binding is both ends.
- **The spawner's own reference is released at its scope exit, with no role.** The scope-exit release pass releases every heap local, a channel binding included (`ynz_channel_free` through `Cg::emit_release_edge`, `crates/ynz-codegen/src/emit.rs:30830-30836` and `:29565-29569`; [`IMP-ownership.md`](IMP-ownership.md) "Scope-exit release on the plain path"); the drop ladder frees each spawned task's reference. The count still counts holders, not producers, so "all references except the consumer's are gone" is not an event a consumer can observe: a release says a holder left, never which end it was.

### Mechanism: explicit `.close()`

The sender endpoint becomes `Mutex<Option<mpsc::Sender<i64>>>`; `close()` is `.take()` plus a wake of every recorded receive-waiter. The handle path already does exactly this: `HandleStateFnFuture.outbox_tx: Option<mpsc::Sender<..>>` is `.take()`n after completion delivery so a post-completion `h.receive()` observes Closed "instead of hanging" (`handle.rs`). Runtime: one field shape change + one new C-ABI entry. Typeck: one new non-suspending conduit method. Codegen: a thin call, and the two closed arms go live.

Decided: auto-close on last-producer drop is deferred (four fields below); it needs per-holder role classification and a producer-vs-holder refcount split, neither of which exists (the scope-exit release pass releases the spawner's reference, but it cannot tell a producer's release from a consumer's). Explicit `.close()` is built and auto-close can be added on top.

### Name: `.close()`

The name is checked against [`vocabulary.md`](../../../.claude/rules/vocabulary.md), [`dot-postfix.md`](../../../.claude/rules/dot-postfix.md), and Golden Rule 5:

- **`.close()`.** An 18-year-old JS developer reads `wire.close()` as "close the wire, nothing more is coming" with no lookup. It takes parens because it is an ACTION (dot-postfix rule), and it is not banned jargon. The runtime, codegen and every existing error string already call this state **closed** (`CHANNEL_CLOSED`, "The channel is closed - the receiving side is gone…"), so the action and the error text use the same word. A future `file.close()` will mean the same thing.

### The contract (what the tests lock)

1. **`ch.close()`** — takes no arguments, returns `Nothing`, **never suspends**, never fails. It is a plain action with no `errors` on it: closing a channel cannot fail, and there is nothing for a caller to handle.
2. **Send after close** → the typed channel-closed error, at runtime, on the existing `send() -> Nothing errors` surface — from any task, including one whose `send()` happens to race the `close()`. **The order is decided by ONE critical section, and it is the sender-lock section, not the `try_send`.** `channel_send_poll_guarded`'s first attempt takes the sender lock, clones the shared `Sender`, and releases the lock BEFORE calling `try_send` (`channel.rs:445–446`). The `None`-check and the clone are that one critical section: a `send()` that reaches it after `close()` took the `Option` sees `None` and is refused; a `send()` that reached it first holds a transient clone, and a `close()` that runs between its unlock and its `try_send` does NOT stop it — the clone keeps the endpoint alive and the value lands. That is linearizable as send-before-close (the send's linearization point is the clone under the lock), so the contract holds. The channel-close fixture and the loom model encode exactly this: **a send already holding a clone is a pre-close send.** A test asserting "any `try_send` that executes after `close()` returns must be refused" is asking for an ordering the implementation does not provide and does not need to. **No compile-time diagnostic** — see "Send-after-close is a runtime error" below.
3. **Receive after close** → drains the buffer first (every value sent before `close()` is delivered, in order), then returns `none`. Never a hang, never a panic, never an abort — the `ChanRecv` closed arm that currently aborts via `ynz_unhandled_error` ("structurally unreachable") becomes a normal `none` return through the same `Maybe<T>` construction every other `maybe` value uses.
4. **In-flight sends issued BEFORE close complete normally.** A producer suspended on a full channel holds its own cloned `Sender` inside its boxed endpoint future; `close()` only takes the object's shared endpoint. That suspended value still lands once the consumer drains a slot — "close" means no NEW sends, not "discard what was already on its way." When the consumer drains, the future resolves, its clone drops, and the channel then reports closed. (A cancelled producer's clone is dropped by `purge_pending_sends`, so a dead suspended send cannot hold the channel open.)
5. **Idempotent.** A second, third, Nth `close()` is a safe no-op: `Option::take()` on `None` does nothing; no error, no panic, no log line. This is a contract, not an implementation accident — a test locks it.
6. **Close wakes every recorded receive-waiter.** A task parked on `receive()` when `close()` runs must re-poll and observe either a remaining buffered value or `none` — never stay parked. `close()` therefore drains `recv_waiters` exactly the way a successful send does. This also settles the "closure observed by one receiver is not propagated to co-waiters" facet `channel.rs` left for this design pass: after `close()`, every waiter is woken once by `close()` itself, and any receiver that registers afterward polls immediately and sees the closed state; the `Ready(None)` exit additionally drains the waiter list (harmless if empty), so no registration can outlive closure unwoken.
7. **Drop-without-close is unchanged.** A channel that is never closed behaves byte-for-byte as it does today (the whole channel fixture suite that does not call `close()` is the regression gate). Closing is opt-in; it adds an end-of-stream signal, it does not change the lifetime model.
8. **Element-type agnostic.** Closed-ness is state on the object, not on the payload; `int`, `string`, `Array<T>`, `Map<K, V>` channels close identically, and any element type added later (see "`Channel<Number>` decimal128 marshalling" below) inherits it without a per-type arm.
9. **`send()` gives its payload for owned-heap element types.** On a `Channel<Array<T>>` or `Channel<Map<K, V>>`, `send()` is a transfer sink: the payload goes through the ONE `check_transfer` decision in [`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value" — a fresh value is admitted; a whole owned binding is consumed (with its alias class); a parameter must be declared `give` on its function (`ParamNeedsGive`, the word travelling up the whole chain, every frame reported in one compile); anything someone else still reaches — a field, an index, a loop cell, a literal built from named values, a call that returns a piece of its argument — is refused (`TransferNeedsCopy`). `int`/`float`/`bool`/`string` channels are unchanged, and `number` (see "`Channel<Number>` decimal128 marshalling" below) is copy-through. Channel-specific consequences in "`send()` gives its payload" below.

### `receive()` on a bare channel becomes `Maybe<T>`

This is the one breaking change, and it is deliberate. Today `let v = ch.receive()` yields a plain `T`, which is only sound because closure cannot happen — the closed arm aborts the process. Once closure is real, the consumer needs a value that says "the stream ended." That value is **`none`**: `ch.receive()` returns `Maybe<T>`, consumed with the narrowing the language already ships — `.exists()` / `.value` (`check.rs:2555–2572`; `m5_p2_byval_bg_maybe_arg_escape.ynz:15–17` is the shipped form) or `.or(fallback)`.

**Why `Maybe<T>`, not `T errors`.** [`vocabulary.md`](../../../.claude/rules/vocabulary.md) reserves `Maybe<T>` for "when 'no value' is a normal possibility" and `errors` for failures the caller must handle. End-of-stream is the normal outcome of every correct consumer — the `tallyScores` loop below ends every correct run by observing it exactly once. Typing that as an error would make the normal exit a failure, and the tree shows precisely how that goes wrong:

- **Auto-propagation would turn the end of the stream into the task's own failure.** Inside a `-> T errors` function, `resolve_ident` narrows a bound `T errors` value to its success type and marks it for early-return-on-failure (`check.rs:3647–3653`). A consumer written `function drain(lend wire: Channel<Int>) -> Int errors { let v = wire.receive() … }` would, on the closed signal, return "channel closed" as the TASK's error — and both channel-consuming task functions that ship today are exactly that shape (`v0_3_m4_handle_send_roundtrip.ynz:4`, `examples/pirates-roster/entrypoint.ynz:1082`). With `Maybe<T>` there is nothing to propagate; the consumer decides what the end means.
- **An allocation per end-of-stream, on a path that is not a failure.** The closed arm builds its error value through `ynz_error_new` (`emit.rs:12802–12813`) — a heap allocation plus a message string every time a loop ends normally. `none` is a bit pattern.
- **The "one `.receive()` convention" argument does not hold.** A handle's `receive()` is `T errors` because its error arm carries the task's OWN failure (`check.rs:4181–4186` — "the error arm is the task's own error, or task-already-finished"). A bare channel has no failure to carry; it has absence. [`stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 2 bans two NAMES for one capability; two receiver types each returning what they actually have is not that.

**The handle's `receive()` stays `T errors` — do not "unify" the two.** `h.receive()` delivers a message reply or the task's completion value, and a task can FAIL; that failure has to travel somewhere, and the handle's error arm is where it goes. `ch.receive()` has no task behind it: a channel ends, it does not fail. The two share a name because they share a verb (take the next thing out) and differ in return type because a task can go wrong while a channel can only run out. A future reader proposing "one `.receive()` type" is proposing either to lose the task's error (handle → `Maybe<T>`) or to manufacture a failure that never happened (channel → `T errors`); both are worse than two honest types.

**The end-of-stream consumer loop, in real syntax.** There is no `break` keyword and no standalone `else` block (`REF-control-flow.md` — only `else =>` inside a multi-case `if`), so the loop runs on a flag that is re-read from the delivery — this is the form the spec and demo teach:

```ynz
function tallyScores(lend wire: Channel<Int>) -> Int {
  let total = 0
  let stillOpen = true
  while (stillOpen) {
    let next = wire.receive()          // Maybe<Int> — none means the stream ended
    if (next.exists()) {
      total = total + next.value
    }
    stillOpen = next.exists()
  }
  return total
}

function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>(4)
  background relayScores(wire)         // sends, then calls wire.close()
  let total = tallyScores(wire)
  print(`total: ${total.toString()}`)
}
```

**Where bare-channel `receive()` appears.** 13 fixture files under `crates/ynz-driver/tests/fixtures/` read a bare channel: `v0_3_m4_channel_composed_bare.ynz`, `v0_3_m3e_alias_local_name_collision/entrypoint.ynz`, `v0_3_m4_channel_composed.ynz`, `v0_3_m4_r5xr8_suspend_then_collect.ynz`, `v0_3_m4_channel_smoke.ynz`, `v0_3_m4_channel_cpu_admission_declines.ynz`, `v0_3_m4_channel_field_annotated.ynz`, `v0_3_m4_handle_send_roundtrip.ynz`, `m5_p4_soa_both_candidate.ynz`, `v0_3_m4_p4_padding_gate.ynz`, `v0_3_m4_p3_cross_channel_exempt.ynz`, `v0_3_m4_channel_xmod_return_annotated/entrypoint.ynz` and `v0_3_m4_channel_xmod_return/entrypoint.ynz`. Outside the fixtures: `examples/primantis-orders/v0_3_m4_errors.ynz`, `examples/pirates-roster/entrypoint.ynz`, and the handle-section example in [`REF-concurrency.md`](../../reference/REF-concurrency.md), whose child reads its command channel bare. Every bare site uses a narrowed form, never the value as `T`; handle-side `h.receive()` sites keep their type. `v0_3_m4_errors.ynz` keeps its sub-expression-position error-gallery trigger.

### `send()` gives its payload — the closed-arm free is only sound if the sender no longer holds the value

**Why the sender must not keep the payload.** A send lowers the payload as one 64-bit slot with no copy and no reference bump, so on a `Channel<Array<Int>>` the sender's `rows` and the buffered element are one allocation. Two things follow: (1) the sending task and the receiving task would alias one mutable array with no synchronization, the cross-task aliasing the ownership model exists to forbid; (2) `YnzChannel::drop` runs the registered element glue on every residual buffered element (`channel_drop_glue`, `crates/ynz-codegen/src/emit.rs:22239-22294`), and the closed arms free a refused payload through the same glue, so the sender's binding would point at freed memory. Typeck therefore routes both `send` forms through the one transfer decision (`check_conduit_method_call`, `crates/ynz-typeck/src/check.rs:6787-7143`; the `TransferSink::Send` call sites are at `:7019` and `:7130`), and a sent binding is consumed.

**The rule.** `send()` transfers ownership of its payload. In `check_conduit_method_call`'s `send` arms (`check.rs:7019`, `:7130`), when `channel_elem_drop(elem)` is `Some(kind)` with `kind.transfers_source()` (the **owned-heap element** kinds, "Which element types" below), the arm calls the ONE transfer decision, `check_transfer(payload, Sink::Send { channel })` (`check.rs:7682-7787`), and inspects no syntax of its own. What that decision does — `Provenance::Fresh` admitted; `Whole(name)` consumed with its alias class, or refused when the binding is `const` (the WHAT-INSTEAD comes from the sink: `Send a copy instead: {channel}.send({name}.copy()).`, `check.rs:7694-7722`), a non-`give` parameter (`ParamNeedsGive`, `emit_param_needs_give`, `check.rs:7797-7825`), or a binding whose origin is a piece of something else (`TransferNeedsCopy`, `emit_transfer_needs_copy`, `check.rs:7827-7848`); `Reaches`/`Unknown` refused (`TransferNeedsCopy`) — is specified once in [`IMP-ownership.md`](IMP-ownership.md) "The transfer decision — ONE emit site" and not restated here. A list of three forms and a refusal would admit forms that alias (a literal built from named values; a `let` bound to a call that returns a piece of its argument — both confirmed by probe) and refused forms that are fresh (`Array<Int>()`); provenance gets both right by construction, and a form nobody listed is a compile error in one function rather than a hole.

After the send, any read of a consumed binding — or of any name in its alias class — reaches the consumed-read site (`report_consumed_read`, `check.rs:6425-6430`, called from `resolve_ident`, `check.rs:6437`), the ONE place a use-after-give is reported, which selects `ConsumedBySend` by cause.

**Ownership must flow through the call — the hole the source-level claim had, and the guard that closes it.** Claiming "the aliasing hole closes by construction at the SOURCE level" is false. This program compiles, prints `3` today, and is a deterministic use-after-free with no `background` anywhere:

```ynz
function producer(wire: Channel<Array<Int>>, rows: Array<Int>) -> Nothing {
  wire.close()
  let sent = wire.send(rows)     // consumes producer's OWN binding `rows` — and nothing else
}
function entrypoint() -> Nothing {
  let wire: Channel<Array<Int>> = Channel<Array<Int>>(4)
  let rows: Array<Int> = [1, 2, 3]
  wait producer(wire, rows)      // entrypoint's `rows` is NOT consumed: the parameter has no `give`
  print(rows.count())            // the send core freed the payload on the closed path — this reads freed memory
}
```

Ownership flows through every call at one place: `check_call_transfers` (`check.rs:7856-7897`) sends each declared-`give` position (and each position the effective-ownership fixpoint finds consumed) to the one transfer decision, `check_transfer` (`check.rs:7682-7787`), which consumes the CALLER's binding. A parameter the callee does not declare `give` cannot be given away: `send()` hands a payload to a channel that WILL free it (teardown glue, the closed-send free, the receiver), so every un-`give`n parameter between the owner and the send is refused. Two guards, both threading the one existing give path rather than a second analysis ([`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)):

1. **A parameter binding sent on an owned-heap channel must be declared `give` on its enclosing function** (`ParamNeedsGive`; text `registry/features.toml:1665-1669`). With `give` declared, the one machinery does the rest: every call site — `wait producer(wire, rows)`, a plain call, a UFCS dot-call (receiver AND non-receiver positions), a generic call, a `dynamic Contract` call — reaches the one transfer decision at every `give` position (`check_call_transfers`, `check.rs:7856-7897`) and consumes the caller's binding; every `background` spawn site reaches it too, because the spawned call is checked as an ordinary call. The statement-form liveness inference still runs and records Give or Copy for codegen; for a declared-`give` parameter the binding is already consumed when it runs, so a program in which the spawner reads the binding afterward does not compile (the consumed-read site fires) and a program in which it does not read it records Give — the two never disagree on a program that builds.
2. **A payload someone else still reaches is refused** (`TransferNeedsCopy`) with the `.copy()` WHAT-INSTEAD — decided by provenance, never by the shape of the argument.

**How far the `give` obligation transits — all the way, and every frame in ONE compile.** Every function between the value's owner and the send declares `give` on that parameter. A relays to B relays to C, and C sends: C's parameter must be `give` (guard 1); B passes its own parameter to C's `give` parameter, so B's parameter must be `give` too — otherwise B's caller keeps a live binding for exactly the reason the program above does; and so on up to the frame that owns the value as a local. Enforced at ONE site — the transfer decision every `give` position calls (`check_transfer`, `check.rs:7682-7787`): a bare, `share` or `lend` parameter passed to a `give` parameter is `ParamNeedsGive` (`non_give_param_in_class`, `check.rs:7655-7669`). "Sent into a channel" and "given to a `give` parameter" are one rule with one diagnostic and a `{act}` slot, not two rules kept in sync by hand. **The whole chain is reported in one compile**: the `effective_ownership` fixpoint's new `consumed[fn][i]` fact is true for B's position the moment B's body sends it, `give` or not, so A's call `B(x)` is a give position in the same build and A's own missing `give` is reported beside B's. Typeck has a callee-before-caller ordering, which is why one compile reports every frame: the fixpoint IS that ordering: it runs before the body check (`crates/ynz-typeck/src/queries.rs:514` calls `effective_ownership::analyze`, `:518` calls `check`; see [`IMP-ownership.md`](IMP-ownership.md) "Where the authority lives"). Fixture corpus: `give`-parameter functions are called from `entrypoint` with locals (`v0_3_m4_p3_cross_give.ynz`, `v0_3_m3b_p4_give_arg_sequenced.ynz`, `m4_player.ynz`, `pirates-roster/entrypoint.ynz:350,1108`); the only relay-through-bare-parameter instance is `examples/primantis-orders/m6_errors.ynz:119` (`background fig.haulCircle()` passes bare parameter `fig` as the receiver of a `give self`), which fires `ParamNeedsGive` beside its original diagnostic, and its `// WHY:` comment says so. A change to the corpus is checked by compiling it, not by inspection.

**Decided: inferring `give` for a bare parameter the body gives away is rejected.** `give` is written in the signature so the transfer is visible at the definition ([`inference.md`](../../../.claude/rules/inference.md); Golden Rule 2). The fixpoint's `consumed` fact may make an ERROR appear one frame earlier; it never makes a program ACCEPTED whose signatures do not say `give`. `share` and `lend` parameters are refused by the same rule: they state that the caller keeps the value, so giving it away contradicts the declaration.

**What this closes in the aliased-container door.** The door has a channel instance, `background producer(wire, table)` + `wire.send(table)` on a `Channel<Map<..>>`, and a container instance. For the channel instance, a `map` bg arg is passed ALIASED (un-cloned) by `prepare_bg_arg_for_ctx`, so the task would send the PARENT's allocation while the parent's binding stayed live. Guard 1 closes that instance: the send requires `give table` on `producer`, and `give` consumes the parent's `table` at the spawn (through `check_call_transfers`, as above), so the parent can no longer read the aliased map; the channel or its receiver is the allocation's only holder, the ladder has no descriptor for an un-cloned arg (nothing to double-free), and the parent's own scope-exit release skips the consumed binding (`Fate::Omitted(OmitReason::Moved)`, `crates/ynz-typeck/src/release_plan.rs:140-167`). The pass-a-`give`-map-and-send shape is therefore asserted CORRECT by the `Channel<Map<K, V>>` fixture, not RED-pinned. The container instance — `bucket.add(rows)` storing a ladder-owned clone into an aliased container — is closed separately by the shared owned-copy routine (see below). The alias cases — by `let` (`let other = rows; wire.send(rows); other.count()`), by reassignment (`rows = other; wire.send(rows); other.count()`) and by a shadowing `let` (probes: all three compile and print the un-consumed alias's count) — are CLOSED by the binding-event rule and alias classes in [`IMP-ownership.md`](IMP-ownership.md) "Binding events, origin and alias classes": every binding event puts the name in the class of the value it now denotes (a reassignment moves it out of its old class first), and consuming any member consumes all. **The consumed state is one field, one enum**: a scope entry holds `consumed: Option<ConsumedBy>` with `ConsumedBy::{Given, Sent, Stored}` (`crates/ynz-typeck/src/scope.rs:8-45`; the cause fills `{channel}`, `{callee}` and `{place}`); the entry's origin and alias class are (re)computed at every binding event ([`IMP-ownership.md`](IMP-ownership.md) "Binding events, origin and alias classes"); `Scope::consume(name, cause)` consumes the whole class (`scope.rs:412-444`); and the consumed-read site selects the `Consumed`, `ConsumedBySend` or `ConsumedByStore` template by the cause (`report_consumed_read`, `check.rs:6425-6430`). The fix for a sender that still needs the value is `wire.send(rows.copy())`.

**Refusals at a send, a give or a store.** The same decision refuses a store the same way, with `{act}` reading "handed to `place`" (`TransferSink::Store`, `check.rs:58-76`, `render`, `check.rs:106-134`; `store_value`, `check.rs:6045-6066`; a container literal's parts, `hand_literal_parts_over`, `check.rs:6108-6132`): a store of a parameter without `give`, a piece, a loop cell or a `const` is refused with `ParamNeedsGive` / `TransferNeedsCopy`, and a read of a stored binding afterwards is `ConsumedByStore` (deny and teach; commit `664924b`). Two refusals stay by decision (Chief): `let held: T | nothing = first` with `first` a `share` parameter (`ParamNeedsGive`), and `[mkm()]` with `mkm()` returning a piece of a local array (`TransferNeedsCopy`). A generic call whose `T` is bound to a union that does not keep the return union's tags is refused (`GenericUnionBoundOutOfOrder`, commit `b891644`; `check_generic_fn_call`, `check.rs:8535-8806`, through `refuse_bound_union_out_of_slot_order`, `check.rs:8822-8894`, which asks `union_keeps_slot_tags`, `check.rs:12477-12489`). Refused until a decision (Chief) allows it: a `background` union passed to a generic parameter (`BackgroundUnionToGenericParameter`, `check_background_union_argument_for_generic_callee`, `check.rs:9587-9625`; text `registry/features.toml:1824-1828`).

**Which element types.** The bite is exactly the owned-heap element set — the types for which the channel registers drop glue at construction: `string`, `Array<T>`, `Map<K, V>` and `number` (a minted cell). `channel_drop_glue` has exactly those non-null arms (a string, array or map goes through the type's one release function; `emit.rs:22239-22294`), and typeck admits only `int`/`float`/`bool`/`string`/`number` (precision ≤ 34)/`array`/`map` as element types (`CHANNEL_ELEM_SUPPORTED_NAMES`, `crates/ynz-typeck/src/types.rs:415-423`; `channel_elem_supported`, `types.rs:407-409`). `int`, `float`, `bool` are value bits: the send copies them and the binding stays usable, so every existing `ch.send(v)` on a primitive channel is unchanged. A `string` is a counted value the channel owns from the send: a borrowed one is copied by the send and a movable one handed over (`own_string_for_store`, `emit.rs:22446-22464`), so a sent string binding keeps the rules of every string read (`ChannelElemDrop::String`, `types.rs:369-372`; `transfers_source` false, `types.rs:381-388`), and the channel releases the strings it still holds at teardown. The predicate "is this element type owned-heap?" must be ONE definition both crates read — typeck for the consume decision, codegen for the glue arm — per [`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md). **It is `Option<enum>`, not a bool and not a three-variant enum**, because codegen needs the drop function, not a yes/no, AND because the absence case must be unrepresentable inside the glue-bearing set: `pub enum ChannelElemDrop { Array, Map, NumberCell, String }` (`types.rs:356-373`) with `pub fn channel_elem_drop(elem: &Type) -> Option<ChannelElemDrop>` (`types.rs:391-401`) in `ynz_typeck::types`. **Two questions ride the one enum, and `Channel<Number>` is why they are two**: "does this element kind have glue?" (`is_some()`) and "does sending it transfer the SOURCE binding?" (`kind.transfers_source()` — `Array | Map => true`, `NumberCell | String => false`, an exhaustive match so a new kind must answer both). `array`/`map` payloads ARE the sender's allocation, so the send consumes the binding; a `number` payload is a fresh 16-byte cell the send mints ("`Channel<Number>` decimal128 marshalling" below), so the binding stays usable and only the cell needs glue. Typeck consumes on `is_some_and(transfers_source)`; `channel_drop_glue` (`emit.rs:22239-22294`) is `match channel_elem_drop(elem) { None => null, Some(kind) => <fn pointer> }` where the inner `match kind` is EXHAUSTIVE and its arms are typed as a function value (`String`, `Array` and `Map` → the type's release function, `Cg::release_fn`, `emit.rs:29334-29345`; `NumberCell → ynz_number_cell_free`) — never a nullable pointer. Decided: not a three-variant `{ None, Array, Map }` enum, because an exhaustive match only guarantees an ARM exists, not that the arm registers glue — `Number => cg.ptr().const_null()` compiles, and that is precisely the silent divergence the enum exists to forbid (a consumed binding whose payload nobody frees). With `Option`, every variant reachable from `Some(..)` must produce a real drop function or the arm does not type-check. Neither existing predicate fits: `is_trivially_copyable` (`types.rs:242`) classifies `string` wrong for this purpose, and `is_mutable_heap_type` (`independence.rs:751`, private) returns `false` for `number` — which is CORRECT for the transfer question (`number` does not join the give set; "`Channel<Number>` decimal128 marshalling" below) but says nothing about glue. `check_channel_construction` (`check.rs:7145-7205`) derives its admitted set from the same function (`channel_elem_supported` = value bits ∪ `channel_elem_drop(elem).is_some()`, `types.rs:407-409`) — never a third hand-maintained list. A future element kind joins by adding an enum variant, and the two exhaustive matches tell codegen where the glue is missing and typeck whether the source is consumed — neither can be satisfied with a null or a default.

**What this makes sound — and its precondition.** With the sender's binding consumed (and every name in its alias class), AND every parameter between the value's owner and the send declared `give` (so each caller's binding is consumed in turn), the buffered value has exactly one source-level holder at every moment: the owner until the send lands, then the channel, then the receiver. Teardown glue frees a value nobody else holds. The receiver's `Maybe<Array<T>>` is the sole reference. The claim holds for every payload the transfer decision admits — a task-built value, a heap-CLONED `background` argument (an `Array<Int|float|bool|shape>` — ladder-owned, and released from the ladder at the send by the shipped runtime protocol in "Two mechanisms, one rule" below), and an ALIASED `background` argument — since the shared owned-copy routine that class is exactly `Map<K, V>` arriving through a `give` parameter, and nothing else: `Array<pointer-elem>` is now copied items-and-all, union is refused at compile time, and the un-cloned fall-through arm no longer exists — **provided it arrives through a `give` parameter**, because the `give` consumes the parent's binding at the spawn and the parent can no longer reach the aliased allocation (see "What this closes in the aliased-container door" above). Without `give`, such a parameter cannot be sent at all (`ParamNeedsGive`); a loop cell of the parent's collection cannot be given to the task at all (`TransferNeedsCopy` at the spawn's `give` position), so the `for (row in matrix) { background producer(wire, row) }` door closes with the same rule. What stays outside the claim is what [`IMP-ownership.md`](IMP-ownership.md) "What this makes sound, and what stays outside" names. The non-channel container door (`bucket.add(rows)` into an aliased container) and the `.copy()` types are no longer on that list — both closed by the shared owned-copy routine; `bg_arg_alias_container_add_red.ynz` is now a green-world lock rather than a RED pin. The alias-by-`let` blind spot is no longer on that list. The `Channel<Map<K, V>>` fixture therefore covers BOTH shapes — a map built inside the sending task, and a map passed as a `give` bg arg and sent — as correct programs, plus the no-`give` variant as a gallery trigger.

**Four new compile-time diagnostics.** Use-after-send (`ConsumedBySend`); the two ownership-flow guards (`ParamNeedsGive` — which also replaces the existing share-parameter refusal at `check.rs:4611` — and `TransferNeedsCopy`, which fires at every transfer sink, not only at a send); and the handle-spawn channel-binding guard (`HandleChannelArgNeedsBinding`, from "The gap that decision leaves" below). The three transfer diagnostics' text is in [`IMP-ownership.md`](IMP-ownership.md) "Teaching text" — one home per string; `HandleChannelArgNeedsBinding`'s stays in this section's "Teaching text". All four are in the error gallery (`examples/primantis-orders/m8_errors.ynz`).

**Fixture coverage.** `crates/ynz-driver/tests/v03_m8_channel_close.rs` and the `v0_3_m8_p4_*` fixtures run one fixture per owned-heap element kind (array, map) covering: send-then-receive round-trip; send-then-read (the compile error, in the gallery); send-after-close (the freed payload, with alloc/free parity via `YNZ_ALLOC_COUNTER_OUTPUT`); drop-with-buffered-element; **the ownership-flow class** — the reviewer's `wait producer(wire, rows)` program above, once with `give rows` (the parent's later read is `ConsumedBySend`/`Consumed` at the call site — gallery) and once without (`ParamNeedsGive` — gallery); a two-hop relay (`A → B(give) → C(give)`, correct; and `B` without `give` — BOTH `B`'s and `A`'s errors in one build when `A` relays its own parameter); a `give`-map bg arg passed and sent (correct, alloc/free parity); **the seven live-hole probe programs as gallery triggers** (UFCS non-receiver give `bucket.stash(rows)`; field `eat(bucket.rows)`; loop cell `for (row in matrix) { wire.send(row) }`; alias-by-`let` `let other = rows`; alias-by-reassignment `rows = other; wire.send(rows)`; returns-a-piece `let rows = pick(bucket)`; nested literal `[a]`) — plus `dynamic Contract` give named separately as the non-exposing eighth probe (typeck rejects, codegen ICEs — zero runtime exposure, a compile-time-only trigger) — beside the admitted forms (owned ident, `.copy()`, a literal of fresh elements, a constructor call, a `returns_fresh` call result bound by `let`); and the `Channel<Number>` class below.

### Two mechanisms, one rule — the runtime release protocol and typeck's consume

**The runtime protocol (`861fd4d`).** A spawned task's drop ladder (`SpawnStateFnFuture::drop`, `runtime.rs`) frees the heap CLONES codegen made of its `background` arguments, by descriptor (`BgArgDropEntry { byte_offset, kind, size }`, kinds now shared through `ynz_abi::BG_ARG_KIND_*`). Nothing told the ladder when a clone LEFT the task, so a send of a cloned `rows` was freed by the ladder while the channel still held it — three reproduced use-after-frees (`ch.send(rows)`, `h.send(rows)`, `return rows` through a handle). The fix is a **pointer-identity release protocol**: `release_ladder_payload(drive, bits)` (`runtime.rs`) walks the current drive's descriptors and rewrites any `HEAP_SHAPE`/`HEAP_ARRAY` entry whose frame slot holds exactly `bits` to the terminal `BG_ARG_KIND_RELEASED`, which the ladder skips. The "current drive" is a thread-local `DriveIdentity` (generation + frame + descriptor array) published by `DriveGuard` around every resume-fn poll (`channel.rs`; the sync entrypoint drive publishes a ladder-less identity). It fires at three doors: `channel_send_poll_guarded`'s three accepting outcomes (`try_send` READY, the park to a pending entry, the re-poll READY) via `release_taken_value`, gated on `chan.drop_glue.is_some()` so a `Channel<Int>` never compares bits; and `HandleStateFnFuture::poll`'s completion extraction for the new `HANDLE_RET_KIND_{VALUE,EC}_HEAP_PTR` return kinds (`handle.rs`, `emit.rs` `lower_let_background_handle`). Both `ch.send` and `h.send` funnel through the one send core, so the channel link is made once. Also: the re-poll of a PARKED send that observes CLOSED now frees the orphaned payload through the channel's glue (the entry was its last owner). Scope: **spawn-argument clones only**; no typeck involvement.

**The ladder consults the hand-off event, not a typeck fact.** The shipped mechanism consults the hand-off EVENT at runtime, by pointer identity: the send core is the one place every hand-off passes through with the sender's identity in hand, so the ladder is never taught about typeck and typeck is never taught about `BgArgFreeKind` — one authoritative answer threaded to the second consumer. Decided: a codegen-side skip of the ladder free keyed on typeck's consumption fact is rejected (a compiled state-machine callee cannot know whether it was spawned or reached directly via `wait`). This design builds on the runtime protocol.

**The question: two surfaces, one derived answer?** [`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md) bans a second derivation of the same COMPUTED question. Here there is one RULE — *`send()` moves ownership of an owned-heap payload* — and two consumers of it that answer **different** questions:

| | Typeck consume | Runtime release protocol |
|---|---|---|
| **Question answered** | May the SOURCE PROGRAM read this binding after this send? | Does this TASK'S DROP LADDER still own this allocation at task retire? |
| **What it sees** | Every later read of the binding — a compile-time fact only typeck has | The ladder, whether THIS parameter was heap-cloned for THIS spawn, and the actual pointer bits — runtime facts only the runtime has |
| **What it cannot see** | Whether the parameter is ladder-owned (same function body, spawned or `wait`ed). (It DOES see aliases now — `let other = rows` joins one alias class, and a call that returns a piece of its argument is `Reaches` — and it sees a parameter's provenance because `ParamNeedsGive` forces the `give` word that makes every caller's consume happen through the same decision.) | The program's future reads |
| **Failure if absent** | Cross-task ALIASING: sender and receiver hold one mutable value with no synchronization (a race, not a crash) | DOUBLE OWNERSHIP of a clone: the ladder frees what the channel/parent holds (the reproduced UAF) |
| **Authoritative for** | Binding-level use-after-send (the `ConsumedBySend` diagnostic; the source-level "one holder" property) | Allocation-level ownership of spawn-arg clones (no double free, no leak, on every send outcome) |

Neither can answer the other's question, so **neither is redundant and both stay**: typeck's consume does not bypass or re-implement the release protocol, and there is no codegen-side ladder edit "because typeck knows." A future reader proposing to drop the runtime match "now that typeck consumes" must first answer (1)–(3) above; a reader proposing to drop the typeck consume "now that the runtime releases" is proposing to ship the cross-task aliasing race the ownership model exists to forbid.

**The compile-time link that keeps them from drifting.** Both are gated on the same set — *element kinds whose send moves ownership* — and that set is defined ONCE, by `channel_elem_drop` (the `Option<ChannelElemDrop>` above): typeck consumes iff `is_some()`; codegen registers glue from the exhaustive inner match on the same enum, whose arms are function values and cannot be null; the runtime releases and frees iff `chan.drop_glue.is_some()`, which is true exactly when codegen registered glue. Adding an element kind to one side without the other is a non-exhaustive match, not a silent divergence — and adding it with a null-glue arm is a type error, not a leak. The runtime half is additionally pinned by behavior: the owned-heap fixture class asserts, per element kind, BOTH the compile error on read-after-send AND exact alloc/free parity through task retire (`YNZ_ALLOC_COUNTER_OUTPUT`), so a typeck set that outgrows the glue set (a consumed binding whose payload nobody frees) shows as a parity gap, and a glue set that outgrows the typeck set (a released clone whose source binding is still readable) shows as a fixture that compiles when the gallery says it must not. The runtime crate's own gates (`send_of_ladder_owned_payload_releases_ladder_alloc_free_parity` and siblings, `channel.rs`) lock the protocol independent of codegen.

**The CLOSED-first-poll path.** `send()` gives its payload on EVERY outcome — typeck consumes before the outcome is knowable, so a refused payload has no source-level owner and is freed by the send path. The free lives in the RUNTIME, not in codegen: a codegen-side free of a ladder-owned clone on CLOSED, beside a runtime that did not release on CLOSED, would be a double free at task retire. `channel_send_poll_guarded` (`crates/ynz-runtime/src/channel.rs`) has THREE first-poll CLOSED arms — (i) `None` under the sender lock, (ii) `try_send → Closed`, (iii) `try_send → Full` then the freshly-boxed endpoint future's first poll returning `Ready(Err(()))` — and all three fall through ONE local `refuse_closed(chan, value_bits)` that, when `drop_glue.is_some()`, calls `release_taken_value()` and then `glue(value_bits)` and returns `CHANNEL_CLOSED`; no arm returns `CHANNEL_CLOSED` except through it, so a fourth closed outcome inherits the free or fails review for not calling it. The payload's fate is decided on ALL outcomes in that one function: READY → the channel owns it; PENDING → the pending entry owns it; CLOSED → freed there, nobody owns it. Codegen's `conduit_closed1`/`conduit_closed2` arms build the error value and free NOTHING. Test `closed_first_poll_send_frees_the_refused_payload_once_alloc_free_parity` plus the loom model `loom_refuse_closed_releases_the_ladder_slot_then_glues_exactly_once` gate it. Anchor: `git log --grep=m8-p4`.

### `.copy()` on `Map<K, V>`

The `ConsumedBySend` advice and the two refusal WHAT-INSTEADs say `{channel}.send({name}.copy())`, and the consume rule covers `Array<T>` **and** `Map<K, V>`, so `.copy()` is a real independent copy for every type the rule names: `Map<K, V>` is cloned through `MapClone` (`ynz_map_clone`, per the table in [`IMP-ownership.md`](IMP-ownership.md) "`.copy()` — produce a new owned value"), and a type with no independent copy is a compile error, never an alias. The independence lock is the fixture `v0_3_m8_p4_map_copy_independent.ynz` (copy, mutate the original with `.set()`, assert the copy kept the pre-mutation values), because for maps "it compiles" proves nothing.

The shared table `ynz_typeck::owned_copy::owned_copy_plan` is the one classifier for both the `.copy()` lowering and the `background`-argument path, so no type falls to an alias catch-all.

### Send-after-close is a runtime error, not a compile-time diagnostic

Decided: **no new compile-time diagnostic** for `send()` after `close()`: it is decidable at compile time only in the trivial same-binding, same-block case, and a check that fires there and stays silent on every real case would teach the wrong lesson. The runtime error is the mechanism; its text is the WHAT/WHAT-INSTEAD/WHY below. A Tier-3 lint for the straight-line same-binding case is a legitimate future teaching aid, not a deferral: nothing is owed until someone wants it.

**`.close()` on a task handle is deliberately not provided — and the one real gap that leaves is recorded below, not hidden.** The reason is not "the spawner already holds the binding" (that argument applies word-for-word to the shipped `h.send(v)`, which forwards to the SAME `channel_send_poll_guarded` on the first channel argument — `handle.rs:404–411` — and nobody calls `h.send` a second name for `commands.send`). The reason is what each act IS. `h.send(v)` exists so the spawner can address the child without naming the child's channel — it is a message TO the child. `close()` is not a message to anyone; it is a lifecycle act on the channel object, and the object the spawner constructed is what it closes: `commands.close()`. Routing that through the handle would make the child's lifetime and the channel's lifetime read as one thing when they are two — a task can finish with its channel open, and a channel can close with its task still running. Calling `.close()` on a handle hits the existing unknown-method diagnostic, whose available-methods list for a HANDLE stays `send(value), receive()` (the channel's list is `send(value), receive(), close()`; the two receivers have separate strings).

**The gap that decision leaves.** The handle's message channel is the callee's first `Channel<T>` PARAMETER (`msg_param_index`, `check.rs:4574-4580`), wired at the spawn from the first channel-typed ARGUMENT — and that argument need not be a binding unless the guard below refuses it. The spawn-arg ownership pre-record records `BgOwnership::Channel` only for `Expr::Ident` args; a non-ident channel expression falls to `bg_arg_is_provably_safe`, where `BuiltinChannel` is not in `type_provably_not_shape`'s set, so it is admitted as default-deny `Give`; codegen's `prepare_bg_arg_for_ctx` then shares it by TYPE regardless of expression form. Without the guard, `let h = background doubler(makeWire())` followed by `h.send(21)` and `h.receive()` would compile and print `42`. In that program there is NO spawner-side binding on which to call `.close()`; the only party that can close that channel is the child, through its own parameter — backwards for a command stream. This is a real omission, not a naming question. **Decision: no `h.close()`** — and the window that leaves is CLOSED by a cheap guard, because a deferral with live exposure takes the guard alongside it rather than leaving the window open until its trigger.

**The guard (one of the four channel-close compile-time diagnostics; a hard error, not a warning, `git log --grep=FRAGO-009`): `HandleChannelArgNeedsBinding`.** At the handle-form spawn (`check.rs:4581-4598`), the argument that binds the callee's first `Channel<T>` parameter (the parameter `msg_elem` is derived from, `check.rs:4574-4580`) must be an `Expr::Ident`. If it is any other expression — a call, a field access, anything call-materialized — the spawn is a compile ERROR, not a warning: a warning would leave the hang class live for exactly one spawn shape while the fix costs the user one `let` line, and Golden Rule 2 says wrong is caught at compile time. It is an error only on the HANDLE form (`let h = background …`), because that is the form where the spawner is a producer (`h.send`) with no way to end its own stream; the statement form with a call-materialized channel is a channel with a single holder — the child is its only producer and consumer, a degenerate single-task shape, not a cross-task stream — and is left alone. The guard is cheap because everything it needs is already in hand at that site: `call_form`'s argument list, the callee's parameter types, and the ident test the pre-record already performs. Teaching text is in "Teaching text" below. A side benefit: an ident argument is pre-recorded as `BgOwnership::Channel` (shared), whereas a non-ident channel argument falls to default-deny `Give` — so the guard also removes the one path where a channel argument's ownership record was the imprecise default.

The deferral itself, four fields, stands for the handle-side close: **WHAT** — a handle-side close (`h.close()`, forwarding to the same close entry on `msg_chan`). **WHY deferred** — with the guard above, every handle spawn's command channel has a spawner binding, so `commands.close()` is always available and `h.close()` would add only the two-lifetimes conflation for the common case; the call-materialized shape is taught nowhere (every spec and demo example binds the channel first, because the spawner normally also receives from it). **COST** — small: one non-suspending conduit method on the handle receiver forwarding to `ynz_channel_close(handle.msg_chan)`, plus its typeck arm and one fixture. **TRIGGER** — a real program for which binding the channel first is genuinely the wrong shape (none is known).

### What the runtime change looks like

- `sender: Mutex<Option<mpsc::Sender<i64>>>`; `ynz_channel_close(chan_ptr)` = lock, `take()`, unlock, `wake_recv_waiters()`. Null pointer is a no-op (matches `ynz_channel_free`/`purge_pending_sends`).
- `channel_send_poll_guarded`'s first-attempt path (`channel.rs`, the `lock_or_recover(&chan.sender).clone()` line): under the ONE sender lock, `None` → `CHANNEL_CLOSED` before any clone; `Some` → clone, unlock, then `try_send` exactly as now. The lock section is the send's linearization point (contract item 2): a send that already holds a clone when `close()` takes the `Option` is a pre-close send and lands. The re-poll path is untouched (an in-flight future owns its own clone). **On ALL THREE first-poll closed outcomes** — `None` under the lock; `try_send → Closed` (`channel.rs:503`); `Full` then the fresh future's first poll `Ready(Err)` (`channel.rs:554`) — through one `refuse_closed` fallthrough, when `chan.drop_glue.is_some()`: `release_taken_value()` then `glue(value)` — the payload's fate is decided here, see "Two mechanisms, one rule" and the closed-send free bullet below.
- **`release_ladder_payload`'s kind filter (`runtime.rs:933`) is linked to one predicate.** It releases only `HEAP_SHAPE`/`HEAP_ARRAY`, which is consistent today only because maps are never ladder-cloned; the day a map is ladder-cloned under a new `BG_ARG_KIND_HEAP_MAP`, that filter silently skips it and the use-after-free class returns through the same door the hotfix closed. The fix: ONE predicate in `ynz-abi`, beside the constants — `pub const fn bg_arg_kind_is_releasable_payload(kind: u64) -> bool`, defined by INVERSION (`kind != BG_ARG_KIND_SHARED_CHANNEL && kind != BG_ARG_KIND_RELEASED`), so a new heap kind is releasable by default rather than skipped by default — consumed by `release_ladder_payload`'s filter; plus `pub const ALL_BG_ARG_KINDS: &[u64]` and a `ynz-runtime` parity test that, for every kind in it, asserts `is_releasable_payload(kind)` iff the ladder's free match (`runtime.rs:1125`) frees a synthetic counted allocation of that kind (alloc/free parity per kind). The free match's defensive `_ => {}` no-op (`runtime.rs:1155`) gains `debug_assert!(!bg_arg_kind_is_releasable_payload(desc.kind))` so a releasable kind with no free arm fails the first debug run that retires a task carrying it, instead of leaking silently. Two consumers, one predicate, one parity test — never a second hand-listed kind set.
- `ynz_channel_recv_poll`'s `Ready(None)` exit: drain `recv_waiters` (item 6 above). The register-before-poll ordering is unchanged. The runtime's return code is unchanged too (`CHANNEL_CLOSED`); what changes is codegen's `ChanRecv` closed arms, which build a `none` `Maybe<T>` instead of calling `ynz_unhandled_error`, and its ready arms, which wrap the payload in the same `Maybe<T>` envelope every `.exists()`/`.value` site already reads — never a conduit-specific envelope twin. **The envelope's alloca goes in the function's ENTRY block, via `alloca_in_entry_llvm` (`emit.rs:~2270`), never at the insertion point.** The existing maybe builders (`build_maybe_some` is `#[allow(dead_code)]` with no live producer, `emit.rs:2382–2383`; its sibling likewise) call `build_alloca` wherever the builder currently sits, and the conduit's join block `conduit_post` (`emit.rs:12785`) sits INSIDE the `while` body of the canonical consumer loop — so an insertion-point alloca would grow the resume function's stack by 16 bytes on every iteration of every `receive()` loop. One slot per `receive()` site, hoisted once per function, stored per receive.
- The `Drop` impl and `purge_pending_sends` are unchanged — closure does not alter who frees what. The shipped release protocol (`release_ladder_payload`, `DriveGuard`, the handle-return release) is unchanged EXCEPT the CLOSED-first-poll extension in the closed-send free bullet below.
- **The closed-send free is sound, and lives in the RUNTIME, not in codegen's closed arms.** The two codegen closed arms for `send` (`conduit_closed1`/`conduit_closed2`) build the error and branch to `post` without freeing a heap-typed `value_bits`; they are the live send-after-close path and stay free-less. The rejected payload is freed inside `channel_send_poll_guarded`'s first-poll CLOSED outcomes — all THREE of them (`None` under the lock; `try_send → Closed`, `channel.rs:503`; `Full` then first-poll `Ready(Err)`, `channel.rs:554`), through the one `refuse_closed` fallthrough — using the SAME per-element drop glue the channel registered at construction (`chan.drop_glue`), immediately after `release_taken_value()` has told the sender's ladder to let go — the same two-step the shipped re-poll-CLOSED arm already performs for an orphaned parked payload. Any closed arm that does not go through the fallthrough leaks the refused payload. One drop path (the registered glue), one owner-decision site (the send core, on all three outcomes), never an ad hoc `ynz_array_drop` twin and never a codegen-side free the ladder does not know about ([`authoritative-derivation.md`](../../../.claude/rules/authoritative-derivation.md)). That free is sound ONLY because `send()` now gives its payload (contract item 9): the sender's binding is consumed at typeck, so the send core frees a value with no source-level holder. Decided: freeing it in codegen is rejected (the sender or the LADDER still holds it). The runtime tests `ladder_is_untouched_when_the_channel_does_not_take_ownership` (`channel.rs:2265`) and `closed_first_poll_send_frees_the_refused_payload_once_alloc_free_parity` (`channel.rs:2340`) gate it.
- **Typeck: three sites, not one.** `close` is NOT added to `CHANNEL_SUSPENDING_METHODS` (`suspension_source.rs`) — it never suspends, and adding it would make the may-block fixpoint over-approximate every function that only closes a channel into a state-machine function. It is the first non-suspending conduit method, unioned into the known-method set explicitly inside `check_conduit_method_call` (`crates/ynz-typeck/src/check.rs:7140-7154`), for channel receivers only. `known` (`check.rs:7151-7154`) is only the first of three sites: (a) the receiver-discipline guard (`check.rs:7206`, "needs the channel held in a named binding") and the derivable-conduit guard run after `known`, and both justify themselves with "this operation can suspend" — `close` cannot, so `check_conduit_method_call` returns `Nothing` for `close` (`check.rs:7190-7204`) before either guard, and `close` on a legitimately-typed receiver is never rejected with a WHY that is false for it; (b) the unknown-method WHAT-INSTEAD string is split per receiver type (`check.rs:7158-7176`): the channel's list reads `send(value), receive(), close()` (`check.rs:7160`) and the handle's stays `send(value), receive()` (`check.rs:7169`); (c) `.close()` takes no arguments (`check.rs:7191-7202`). The receiver must still be a `Channel<T>`-typed expression.
- **The loom substrate (`git log --grep=m8-p3`).** `crates/ynz-runtime/src/sync.rs` is the one site `channel.rs`/`handle.rs` import `Arc`/`Mutex`/`MutexGuard` from (a bare `std::sync` re-export in every production build — proven byte-equivalent by an LLVM-IR diff at introduction; loom's under `RUSTFLAGS='--cfg loom'`; `runtime.rs`'s Tokio-lifecycle `RUNTIME` static keeps its own `std::sync::Mutex` by name, a documented exemption in `sync.rs`), and `crates/ynz-runtime/src/loom_tests.rs` is the harness: six models driving the real shims/keyed core/purge helper/drop ladder, each with a revert-proven failure mode reported by the model's own assertion — including the kind-2 drop-ladder arm's purge-BEFORE-release order, which a co-owner probe asserts directly in both ladder models (a swapped arm would pass the live-co-owner model and only crash the last-reference one). Tokio's own mpsc stays a std black box under that cfg (tokio gates its loom paths on `all(test, loom)`); the loom-only `YnzChannel::mpsc_witness` makes each Tokio call one dependent atomic step so their orderings are exhaustively explored. The two channel-close interleavings this section names (`close()`-vs-`send()` under the sender lock, and `close()`-wakes-parked-receiver) are pure `ynz-runtime`-owned lock/waiter logic and are models in that file.

### Deferred: auto-close on last-producer drop (four fields)

- **WHAT**: the channel closing itself when the last binding that can `send()` goes out of scope, so a producer that simply returns ends the stream without writing `close()`.
- **WHY deferred**: it requires (1) a per-binding producer/consumer role analysis that does not exist and (2) splitting the runtime's single `Arc` holder count into producer vs. holder counts. The scope-exit release pass (every heap local, a channel binding and a task handle included, is released at its scope exit) cannot tell a producer's release from a consumer's. Shipping explicit `close()` first costs the user one line per producer and is correct today.
- **COST to add later**: medium — the role analysis (a `send`-reachability classification per channel binding, per task), the runtime producer count (one `AtomicUsize` decremented from the drop ladder for producer-classified holders), and a compatibility rule that explicit `close()` and auto-close compose (close-once semantics already make that trivial: whichever fires first closes, the other is the no-op).
- **TRIGGER**: a real workload where forgetting `close()` produced a hang in practice — the explicit form is the teaching-correct default until the compiler can prove the producer set.

### `Channel<Number>` decimal128 marshalling

Fixtures: `v0_3_m8_p4_number_chan_roundtrip.ynz`, `v0_3_m8_p4_number_chan_send_after_close.ynz`. The close design is element-type agnostic (contract item 8): closed-ness lives on the object, payloads cross as one opaque i64 slot (`channel.rs:204–214`, `mpsc::channel::<i64>`), and the only per-element hook is the drop glue registered at construction. `Channel<Number>` is the OPPOSITE axis — how a 16-byte value crosses a 64-bit slot — and its three decisions are these.

**Representation: a counted 16-byte heap cell minted at the send, freed at the receive.** `number` (precision ≤ 34, the hardware decimal128 `i128` — `emit.rs:1151`, `:1477`) lives on the sender's frame; its address dies with the frame, which is why the value cannot cross a channel by address. The send lowering copies the 16 bytes into a fresh `ynz_alloc(16)` cell through the ONE existing helper `number_to_heap_cell` (`emit.rs:7137` — the same mechanism the `background` argument path uses for a decimal128 arg) and passes the cell's pointer bits in the slot. The receive lowering loads the 16 bytes out of the cell into the receiver's own `i128` storage (a `sm_crossing_decimal128_set` two-slot frame local when it crosses a suspension, an alloca otherwise — the existing paths) and frees the cell immediately, before the `Maybe<Number>` envelope is built; the cell never outlives the receive statement. Drop glue: `ChannelElemDrop::NumberCell → ynz_number_cell_free` (a `ynz_free(ptr, 16)` behind a named C-ABI entry so the glue table's arms are all function values; it reuses the counted free the bg-arg ladder's `HeapShape { byte_size: 16 }` arm already performs for the same cell shape), so residual buffered cells are freed at teardown and a refused payload is freed by `refuse_closed` on every CLOSED first poll — the runtime's `release_taken_value()` finds no ladder slot holding these bits (the cell is new, never a ladder-owned clone) and is a harmless no-op. `h.send(number)` gets the same marshalling for free: the handle form funnels through the one send core, and the handle's `-> Number errors` return already travels as an `i128` pair (`emit.rs:202–206`), untouched.

Decided: the cell is minted at the send. Not the ladder-owned cell pointer (the ladder would free what the channel holds), not two i64 slots per element, and not an i128 slot for all channels (every channel would pay 16 bytes per element). Precision > 34 (bignum) is a heap/global string pointer that already survives the frame (the bg-arg path leaves it to the by-pointer default arm); it stays REJECTED at construction — `Channel<Number>` is decimal128 only, and the registry entry's `triggers` text says so.

**`number` does NOT join the give set — it stays copy-through.** `number` is a value type (`is_trivially_copyable`, `types.rs:242`); the cell the send mints is a fresh allocation nobody else holds, so the sender's binding remains its own 16 bytes and stays usable — exactly like `int`. This is why `ChannelElemDrop` carries `transfers_source()` ("Which element types" above): `NumberCell` has glue but does not consume; `Array`/`Map` have glue and do. Decided: `number` does not join the give set, because joining it would make `wire.send(price)` consume a value-typed binding for no memory-safety reason — a teaching error the user could not explain from the type. `ConsumedBySend`, `ParamNeedsGive` and `TransferNeedsCopy` never fire on a `number` channel; `wire.send(order.total)` (a field) is admitted, because there is nothing to own.

**Alloc/free consequence, and the gate that proves it.** One counted 16-byte `ynz_alloc` per successful or suspended send, one counted free per receive; residual cells freed by teardown glue; refused cells freed by `refuse_closed`. The `v0_3_m8_p4_number_chan_*` fixtures (`roundtrip`, `send_after_close`, `drop_with_buffered`, `parked_send_drained_after_close`) assert exact alloc/free parity (`YNZ_ALLOC_COUNTER_OUTPUT`) across send-then-receive, send-then-drop-with-buffered-element, send-after-close, and a parked send drained after close, plus value round-trip (`2.5` in, `2.5` out, through a suspension on the receiving side so the two-slot frame path is exercised). The typeck gate admits `Type::Number { precision <= 34 }` only, through `channel_elem_drop`, from which `elem_supported` is derived ("Which element types") — never through a hand-edited `elem_supported` list. **`shape` elements stay deferred**: a shape's inline copy shares any pointer-cell field (an `array` field) between sender and receiver, so admitting shapes is the depth-two transfer question the container-store deferral in [`IMP-ownership.md`](IMP-ownership.md) owns, not a marshalling question; the registry entry `channel-element-heap-upgrade` covers `shape` (and bignum `number`) only, and its `substitute`/`why`/`triggers` say `Number` is supported. The error gallery's rejected-construction trigger is `Channel<BatchNote>` (`examples/primantis-orders/v0_3_m4_errors.ynz:100`, a shape element).

### Registry entries this design needs

All three conduit methods are registered together in `registry/features.toml` (`:2507-2533`, the `Channel<T> methods` block), beside typeck's `CHANNEL_SUSPENDING_METHODS`:

- **`[[primitive_intrinsic]]`** `close`: `kind = "method"`, `receiver_type = "Channel"`, `return_type = "Nothing"`, `since = "v0.3-M8"`. `receive`: `kind = "method"`, `return_type = "Maybe<T>"`. `send`: `kind = "method_1arg"`, `param_types = ["T"]`, `return_type = "Nothing errors"`.
- **`send`'s give semantics are schema data.** `[[primitive_intrinsic]]` has one optional ownership field, `param_ownership` (a list aligned with `param_types`, each `share`, `lend` or `give`, validated in `crates/ynz-registry/build.rs:264-291`; absent = by-value/shared semantics). `send` sets `param_ownership = ["give"]`, so the LSP hover says "send gives its value" from the registry (`crates/ynz-registry/src/lsp_adapter.rs:253-256`) rather than from a typeck constant.
- **`[[deferred_language_feature]]`** `name = "channel-auto-close-on-last-producer"` carries the four fields above verbatim (`design_doc` = this file).
- **`[[deferred_language_feature]]`** `name = "background-handle-close"` carries the four fields from "The gap that decision leaves" above verbatim (`design_doc` = this file); its `substitute` field names the guard: bind the channel first and call `commands.close()`.
- **`[[primitive_intrinsic]]`** `copy` on `Map`: `kind = "method"`, `receiver_type = "Map"`, `param_types = []`, `return_type = "Map<K, V>"`, `since = "v0.3-M8"` — see "`.copy()` on `Map<K, V>`" above. Same shape as the `Array`/`Fixed` `copy` entries.
- **`[[diagnostic_template]]` entries.** `kind_name = "ConsumedBySend"` — the use-after-send compile diagnostic (text in [`IMP-ownership.md`](IMP-ownership.md) "Teaching text"; slots `{name}`/`{channel}`/`{sent}`/`{via}`); canonical and reusable (it fires for every owned-heap send followed by a read), which is exactly the template kind's criterion. `kind_name = "ParamNeedsGive"` — a parameter given away (sent into an owned-heap channel, or passed to a `give` position) without a `give` declaration; `{name}`/`{fn}`/`{modifier}`/`{act}`/`{type}`/`{copy_form}` varying; it replaces the former inline share-parameter refusal, so no gallery line or snapshot asserts the old share-refusal wording. `kind_name = "TransferNeedsCopy"`  — a payload at ANY transfer sink that someone else still reaches; `{act}`/`{expr}`/`{type}`/`{reason}`/`{fix}` varying. `kind_name = "HandleChannelArgNeedsBinding"` — the handle-spawn guard (text below); same criterion, `{callee}`/`{expr}` varying. `ConsumedByStore` (`registry/features.toml:1657`) is the fifth template of the family; the consumed-read site is the one consumer of `Consumed`, `ConsumedBySend` and `ConsumedByStore`, selected by consumption cause.
- **`Channel<Number>` entries.** `[[deferred_language_feature]] name = "channel-element-heap-upgrade"` covers `shape` elements and bignum `number` (precision > 34): `substitute` offers the field-wise workaround for `shape` and lists `Number` among the supported types, `why` states that decimal128 crosses as the send-minted cell, `triggers` names `Channel<Player>()` only. No `[[primitive_intrinsic]]` for it (the element type widens; `send`/`receive` are unchanged). `ynz_number_cell_free` is a C-ABI entry that is registry-invisible (compiler-internal glue, carve-out).
- **Still no `[[diagnostic_template]]` for the runtime messages.** The channel-closed text is a RUNTIME error-value message emitted per op in codegen (`closed_msg`), not a `DiagnosticKind` compile diagnostic; the template kind is for reusable compile-time diagnostics. Decided: no template for them.
- **No new keyword, banned word, or type constant.**

### Teaching text (Golden Rule 3)

Runtime error carried by the typed value on **`send()` after close** (the common cause of a closed send is a `close()`, so the text says so rather than describing a receiver as "gone"):

> **WHAT**: This channel is closed — `close()` was called on it, so no new values can go in.
> **WHAT INSTEAD**: Send everything before calling `close()`, or bind the result — `let sent = wire.send(value)` — and check `sent.failed()` to stop producing when it fails.
> **WHY**: `close()` is the signal that says "nothing more is coming" so the receiving side can stop waiting. A value sent after that signal would arrive after the receiver already decided the stream ended, so the send is refused rather than silently lost.

**`receive()` after close-and-drain has no message — it returns `none`.** There is no error text because there is no error: `none` is the same value every `Maybe<T>` in the language uses for "nothing here," and the consumer reads it with the `.exists()` / `.value` / `.or()` forms it already knows. The teaching happens in [`REF-concurrency.md`](../../reference/REF-concurrency.md) "Closing a channel" and in the demo's consumer loop, not in a runtime string.

The one-line runtime message on send-after-close (what the typed value's `.message` reads, and what an unhandled error prints) — kept short because the call trace already names the site:

- send: `The channel is closed — close() was called, so this value cannot be delivered. Check .failed() on the send, or send everything before close().`

**The three TRANSFER diagnostics — `ConsumedBySend`, `ParamNeedsGive`, `TransferNeedsCopy` — have ONE home: [`IMP-ownership.md`](IMP-ownership.md) "Teaching text (Golden Rule 3; four diagnostics, one emit site)".** They fire at every transfer sink (a channel send is one of them), so their text lives with the sink-neutral rule, not here. `ConsumedBySend` and `ParamNeedsGive` carry one slot each beyond the basic form (`{via}` for a class-mate read; the chain form of `{act}`), and `TransferNeedsCopy` has a `{reason}` slot that names exactly who still holds the value. The cannot-give-const refusal takes its WHAT-INSTEAD from the sink (`check_transfer`, `check.rs:7694-7722`), and the send sink supplies `Send a copy instead: {channel}.send({name}.copy()).` — the "declare it with `let`" advice is wrong for a send, where the caller keeping the value is the whole point. The share-cannot-be-given refusal is the `share` instance of `ParamNeedsGive`. `.copy()` is real for both element kinds the rule covers once `map`'s ships (above).

The channel-specific worked example the gallery renders (`ConsumedBySend` on a send followed by a read):

```ynz
let rows: Array<Int> = [1, 2, 3]
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

**New compile-time diagnostic — handle-spawn channel must be a binding** (`HandleChannelArgNeedsBinding`; fires at the handle-form spawn, `check.rs:4581-4598`, when the argument binding the callee's first `Channel<T>` parameter is not a plain identifier; `{callee}` is the spawned function, `{expr}` the offending argument's source text):

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

- unknown-method WHAT-INSTEAD list, on a CHANNEL receiver, becomes `Available methods: send(value), receive(), close().`, and its WHY gains one clause: `close()` marks the channel finished so a receiver knows when to stop. On a HANDLE receiver the list stays `send(value), receive().` (the two strings are separate: `check.rs:7160`, `:7169`).
- `.close()` with arguments: `` `.close()` takes no arguments, but got {n}. `` / `Call it bare: wire.close()`. / `close() only marks the channel finished — there is nothing to pass in.`

### Cross-references

- [`docs/reference/REF-concurrency.md`](../../reference/REF-concurrency.md) "Channels" and "Closing a channel" — the user-facing spec of what ships here.
- [`IMP-no-function-coloring.md`](IMP-no-function-coloring.md) "Channel/Queue Primitives" — the bounded-by-default half of the channel design; close is the end-of-stream half.
- `crates/ynz-runtime/src/handle.rs` `outbox_tx: Option<mpsc::Sender<..>>` — the shipped precedent this reuses.
- `crates/ynz-runtime/src/runtime.rs` `release_ladder_payload` / `DriveIdentity`; `crates/ynz-runtime/src/channel.rs` `DriveGuard` / `release_taken_value`; `crates/ynz-abi/src/lib.rs` `BG_ARG_KIND_RELEASED`, `HANDLE_RET_KIND_{VALUE,EC}_HEAP_PTR` — the shipped spawn-arg release protocol "Two mechanisms, one rule" builds on (`git log --grep=861fd4d` / `fix/bg-arg-channel-send-uaf`).
- The aliased-container door (`bucket.add(rows)`) — the FOURTH escape door. This design closes its CHANNEL instance (an aliased bg arg sent by its task — `ParamNeedsGive` forces the `give` that consumes the parent's binding); the container instance is closed by the shared owned-copy routine. See "Ownership must flow through the call — What this closes in the aliased-container door".
- History: `git log --grep=m8-p1` (close design), `git log --grep=m8-p4` (build). Invariant: channel-close semantics cannot regress.

### What is settled

In force, with nothing open: the close mechanism, the name, `receive()` → `Maybe<T>`, idempotency, wake-all, in-flight completion, drop-unchanged, the `refuse_closed` collapse, the closed-send free in the runtime, `Option<ChannelElemDrop>`, `HandleChannelArgNeedsBinding` as a hard compile ERROR, the four-field auto-close deferral (`git log --grep=m8-p1`); the transfer rule itself ([`IMP-ownership.md`](IMP-ownership.md) "Transfer — Who Else Holds This Value"), the Auto-Arc topology and beneficial-emission condition, and the `Channel<Number>` copy-through decision with the `transfers_source()` split of `ChannelElemDrop` (`git log --grep=m8-p2`).

---

## Design Divergences (Auto-Parallelization Pass)

Where the shipped auto-parallelization pass is narrower than the concurrency model, each limit names its concrete cost and reversal path. (The heading name is referenced from `crates/ynz-typeck/src/independence.rs` and the `v0_3_m3b_p4_*_sequentializes.ynz` fixtures.)

### Same-callee concurrent calls run sequentially (suspending/I/O members only)

**Scope**: applies to I/O-overlap parallelization only.  CPU parallelization has no such restriction (see "Same-Callee CPU Calls" above): two data-independent calls to the same CPU callee DO run in parallel, because CPU spawns are keyed by `(group, member-index)` with a per-invocation ctx rather than by callee name.

**The model**: independent operations auto-parallelize regardless of which function they call.

**What the pass does**: two calls to the same *suspending* function (e.g., `worker(1)` followed by `worker(2)`) are lowered sequentially, even when they are data-independent.  This does NOT apply to CPU calls: two data-independent `crunchStat(x)` + `crunchStat(y)` (or `fib(40)` + `fib(41)`) parallelize, since the CPU path uses per-invocation ctx + member-index slot keying.

**Named cost**: the I/O sub-frame allocates one slot per unique suspending callee name. Two concurrent invocations of the same *suspending* callee would require two slots with a disambiguation scheme — a separate concern in `build_frame_layouts`. Calling the same suspending helper twice in a row doesn't parallelize; it runs sequentially, which is always correct.  CPU members pay no such cost.

**Reversal path**: extend `build_frame_layouts` to allocate N sub-frame slots per suspending callee name when a suspending function is called more than once in a straight-line block, keyed on (callee_name, invocation_index) — mirroring the (group, member-index) keying the CPU path already uses. The independence analysis and join codegen are already class-agnostic; only the I/O frame-layout phase needs the extension.

### Write-effect uses a TYPE-BASED conservative floor — mutable-heap arguments never parallelize

**The model** (`Reads vs Writes — Ownership Does Double Duty`): "the compiler traces through user functions to determine if they contain writes" — independent writes to *different* resources auto-parallelize.

**What the pass does**: the independence analysis treats a call argument as a potential aliased write iff its parameter **type** is a mutable heap reference (`shape`/`array`/`map`/`maybe`/union/`dynamic`). It does **not** attempt to classify per-call-site whether the callee actually mutates the argument. Any suspending call carrying a mutable-heap argument is sequenced against every other statement in the block; only no-argument, primitive-argument, and immutable-`string`-argument suspending calls parallelize.

**Decided: a precise per-form write classification (a name-based AST fixpoint over the call graph) is rejected.** Proving a heap argument read-only needs receiver types and alias tracking, which an AST-walking pass is not; the type-based floor treats a mutable-heap argument as *unconditionally* a potential write, sound however the callee uses it, and the `share`-read-only premise a precise analysis would lean on is itself only partially enforced (see the `share` best-effort section below).

This follows the golden rules directly: **Golden Rule 2 (compile-time soundness — never a silent runtime miscompile) outranks Golden Rule 7 (efficiency-first).** When the two conflict, soundness wins.

**Named cost**: two suspending calls that each take a mutable-heap argument do not overlap, even when both only READ their argument (e.g. two suspending reads of distinct shapes). This forfeits the read-only-mutable-heap-argument overlap. It is a perf miss, never a correctness miss — the floor only ever *adds* sequencing, never introduces a wrong parallel execution. The I/O-overlap headline is unaffected: no-argument, primitive-argument, and immutable-`string`-argument suspending calls (the realistic I/O-fan-out shapes) still parallelize.

**Reversal path**: a real type+alias-aware ownership analysis (receiver types threaded through the analysis + intra-procedural alias tracking) that can PROVE a heap argument is both read-only AND non-aliased re-enables narrowing the floor to permit proven-read mutable-heap arguments to overlap. Until that exists, the floor stands. (The CPU-parallel pass reuses this same type-based write-effect source — the floor transfers unchanged.)

### `share` read-only enforcement is best-effort (transitive teaching error)

**What ships**: typeck rejects mutation of an explicit `share` parameter — direct field/element assignment, mutating collection methods (`.add`/`.set`/...), `share self` receivers, passing a `share` argument to an explicit `lend`/`give` position, and the common transitive case (`fa(share x) { helper(x) }` where `helper` mutates) via the `effective_ownership` fixpoint.

**Known-incomplete (deferred to a real ownership analysis)**: forms a name-based analysis cannot see also escape the *transitive* `share` check — a mutation through a field/index of the `share` param via a bare callee, a builtin mutator shadowed by a same-named user function, and a local `let`-alias of the `share` param. These are **missed compile-errors, not miscompiles** — the program still runs soundly (the type-based floor sequentializes regardless), it simply doesn't receive the teaching diagnostic. The `share`-enforcement completeness is decoupled from auto-parallel soundness on purpose: soundness is the floor's job (complete); the `share` error is a teaching aid (best-effort). **Reversal path**: the same type+alias-aware ownership analysis closes both at once.

---

## Runtime Implementation (Internal — Developer Never Sees This)

Thread pool sized to CPU cores. I/O operations use the OS event system (epoll/kqueue/IOCP). The compiler's dependency graph determines scheduling. Developers never configure or think about any of this.
