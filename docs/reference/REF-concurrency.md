---
name: "REF-concurrency"
description: "Yinz runs code concurrently for you. You write normal, sequential-looking code. The compiler analyzes what each operation depends on and runs independent operations at the same time."
tags:
  - "yinz-compiler"
created_at: "2026-05-12"
updated_at: "2026-07-03"
status: "active"
author: "patrick"
metadata:
  type: "reference"
---

# Concurrency

Yinz runs code concurrently for you. You write normal, sequential-looking code. The compiler analyzes what each operation depends on and runs independent operations at the same time.

Two keywords and one type. That's the entire developer API:
- `wait` — force something to complete before continuing
- `background` — run something separately, outside this function's lifetime
- `channel<T>` — send values between tasks safely

---

## How it works — the compiler does the thinking

The compiler builds a dependency graph from your code. Operations that don't depend on each other run at the same time. Operations that depend on something automatically wait for it.

```
function loadDashboard() -> Dashboard errors {
  let user = fetchCurrentUser()
  let feed = fetchFeed(user)
  let messages = fetchMessages(user)
  let stats = fetchStats(user)
  return buildDashboard(user, feed, messages, stats)
}
```

What the compiler sees:
- `fetchCurrentUser()` depends on nothing → runs immediately
- `fetchFeed`, `fetchMessages`, `fetchStats` all need `user` → all three wait for `user`, then run at the same time
- `buildDashboard` needs all four → waits for everything

You write 6 lines of normal code. You get optimal concurrent execution.

---

## The IDE shows you the execution plan

The IDE marks which lines run concurrently and which run in sequence. This is not optional — you should always be able to see what the compiler decided.

```
function loadDashboard() -> Dashboard errors {
  let user = fetchCurrentUser()          // ① sequential
  let feed = fetchFeed(user)             // ② ┐
  let messages = fetchMessages(user)     // ② ├ concurrent group
  let stats = fetchStats(user)           // ② ┘
  return buildDashboard(user, feed, messages, stats)  // ③ sequential
}
```

Hovering over any line shows: "Runs concurrently with lines X and Y. Waits for: user (line 2)."

---

## Reads vs writes — the compiler knows the difference

The compiler uses ownership to tell reads from writes. This is the same ownership system you already use — no new rules.

```
function getUser(share db: Database, id: string) -> User errors
//               ^^^^^ share = reading — safe to parallelize with other reads

function saveUser(lend db: Database, user: User) -> nothing errors
//                ^^^^ lend = writing — compiler sequences this correctly
```

`share` in a parameter = the function reads this resource. Multiple reads can run concurrently.

`lend` in a parameter = the function writes to this resource. Writes to the same resource run in order. Two functions that only `share` the same data can always parallelize.

---

## `wait` — when you need explicit ordering

**Suspension is automatic — you never type `wait` to make I/O suspend.** When a call can block (for example, a network fetch or `sleep`), the compiler detects this and suspends the function automatically. The IDE shows that suspension as a muted `wait` hint. You don't write `wait` for it.

**`wait` is only for ordering.** Use it when a call must complete before the next statement runs, but the compiler can't infer that — because the two operations touch different resources and no value passes between them.

Most code never needs `wait`. The compiler handles reads automatically and infers write ordering from ownership. Use `wait` when you need explicit ordering that the dependency graph can't figure out on its own.

**When to use `wait` — ordered side effects:**

```
function processOrder(order: Order) -> nothing errors {
  wait chargePayment(order)          // must complete before anything else
  sendConfirmationEmail(order)       // only runs after payment succeeds
  updateInventory(order)             // runs concurrently with email — fine
}
```

Without `wait`, the compiler would run all three concurrently (none use each other's return values). `wait chargePayment(order)` tells the compiler: "finish this before moving forward."

**When to use `wait` — external API ordering:**

```
wait request.post("api.stripe.com/charge", chargeData)
request.post("api.email.com/send", receipt)
```

The compiler can't tell that these two HTTP calls are logically related. They're just two HTTP posts to different URLs. `wait` makes the intended ordering explicit.

**When to use `wait` — rate limiting:**

```
for (page in pageNumbers) {
  let result = wait fetchPage(page)     // wait for each before starting the next
  results.add(result)
}
```

Without `wait`, the loop would fetch each page sequentially anyway (loops are sequential by default). With `wait`, you're being explicit — and it also works inside any other construct where you need one-at-a-time behavior.

**When you do NOT need `wait` — data dependencies:**

```
function buildProfile(id: string) -> Profile errors {
  let user = fetchUser(id)
  let permissions = fetchPermissions(user)       // depends on user — auto-waits
  let profile = buildProfile(user, permissions)  // depends on both — auto-waits
  return profile
}
```

`fetchPermissions(user)` passes `user` as an argument, so the compiler already knows it must wait. `wait` would be redundant here.

---

## Why loops don't auto-parallelize

Independent statements in a function auto-parallelize based on dependencies. Loop iterations do not.

Loop iterations could be 3 or 300,000. Parallelizing 300,000 network calls simultaneously would overwhelm any API, database, or server. The risk is too high for an unbounded default.

Independent statements in a function have a predictable count — typically 3 to 10 — and the compiler can see all of them at once. Loop iteration counts are unpredictable at compile time.

The rule: independent statements = auto-parallel. Loop iterations = sequential by default.

---

## `background` — tasks that outlive this function

`background` runs something outside the current function's lifetime. The current function doesn't wait for it.

**Fire and forget — don't store the result:**

```
background sendWelcomeEmail(user)
background logAnalytics(event)
background retryPayment(payoutId)
// function continues immediately — these run whenever
```

The task runs independently. If it fails, it handles the failure internally. The caller never waits and never sees the result. Good for MVP work and non-critical background operations.

**Long-running — store the handle to communicate:**

```ynz
let scout = background gradeProspect(requests)
```

If you store the result of `background`, you get a handle you can talk to via `.send()` and `.receive()`. If you don't store it, it's fire-and-forget. Same keyword, two patterns. Handles are built on channels, so read the next section first — then see "Talking to a background task" below.

---

## Channels — sending values between tasks

A channel carries values of one type from one task to another. You create one with `channel<T>()`, where `T` is the type of value it carries:

```ynz
let scores: channel<int> = channel<int>()        // holds up to 64 values (the default)
let names: channel<string> = channel<string>(8)  // holds up to 8 values
```

The number is the channel's **capacity** — how many values it can hold at once. Leave the parens empty and you get the default of 64. The IDE shows the default as a muted `64` hint inside the empty parens; click it to write `64` into your source.

One task puts values in with `.send(value)`. Another task takes them out with `.receive()`:

```ynz
function relayScores(lend wire: channel<int>) -> nothing {
  wire.send(2)
  wire.send(5)
  wire.send(3)
}

function entrypoint() -> nothing {
  let wire: channel<int> = channel<int>(1)
  background relayScores(wire)
  let first = wire.receive()
  let firstScore = first.or(0)
  let second = wire.receive()
  let secondScore = second.or(0)
  let third = wire.receive()
  let thirdScore = third.or(0)
  let total = firstScore + secondScore + thirdScore
  print(`runs this game: ${total.toString()}`)
}
```

`relayScores` runs as a background task and pushes three scores into the channel. `entrypoint` pulls them out one at a time. If the channel is empty when you call `.receive()`, your task pauses until a value arrives. You never write any locking or waiting code — the channel handles it.

**`.receive()` gives you a `maybe<T>`, not a bare `T`.** A channel can be closed (see "Closing a channel" below), and once it is closed and empty, `.receive()` returns `none` instead of waiting forever. So you read a delivery the way you read any `maybe` value: `.or(fallback)` when you know a value is there, or `.exists()` and `.value` when the end of the stream matters to you.

**A channel is the one value you can hand to a background task and keep using yourself.** Both sides hold the same channel safely — that is the entire point of a channel. (Every other value must be given away or copied — see "Ownership with background tasks" below.)

### Every channel is bounded — and that's a feature

There is no unlimited channel. When a producer fills the channel faster than the receiver drains it, `send()` pauses the producer until a slot frees up.

In the example above the capacity is 1: `relayScores` sends the first score, then pauses on the second send until `entrypoint` receives the first. This is called **backpressure**.

**A suspended producer is backpressure working correctly, not a deadlock.** The producer isn't stuck — it's waiting for the receiver to catch up, and it resumes the moment a slot opens. Without the bound, a fast producer would silently fill up memory until the program dies. The bound is what keeps that from ever happening.

Need more buffering? Pass a bigger number: `channel<int>(10000)`. There is deliberately no "unlimited" option.

### The rules channels follow

**The element type is fixed at construction.** A `channel<int>` carries `int` values only:

```
wire.send(`three`)
// COMPILE ERROR: This channel carries `int` values, but you're sending `string`.
// Send a `int` value, or create a `channel<string>` for this data.
```

**The capacity must be at least 1:**

```
let wire: channel<int> = channel<int>(0)
// COMPILE ERROR: A channel's capacity must be at least 1, but got 0.
// Use a positive capacity: `channel<int>(64)`. For very large buffering, pass a
// large explicit number — there is deliberately no unbounded channel.
```

**A channel operation gets its own line.** `.send()` and `.receive()` can pause your task, so each one must be its own statement — `wire.send(5)` on its own line, or `let value = wire.receive()`. Burying one inside a bigger expression is a compile error that tells you to pull it out into a named variable first.

**Channels carry these values in v0.3:** `int`, `float`, `boolean`, `string`, `number`, `array<T>`, and `map<K, V>`. Sending a `shape` value isn't supported yet — the compile error suggests sending its fields as separate values and rebuilding the shape on the receiving side.

### Closing a channel

A producer tells the other side "nothing more is coming" with `.close()`:

```ynz
function relayScores(lend wire: channel<int>) -> nothing {
  wire.send(2)
  wire.send(5)
  wire.send(3)
  wire.close()
}

function tallyScores(lend wire: channel<int>) -> int {
  let total = 0
  let stillOpen = true
  while (stillOpen) {
    let next = wire.receive()
    if (next.exists()) {
      total = total + next.value
    }
    stillOpen = next.exists()
  }
  return total
}

function entrypoint() -> nothing {
  let wire: channel<int> = channel<int>(4)
  background relayScores(wire)
  let total = tallyScores(wire)
  print(`total: ${total.toString()}`)
}
```

After `close()`:

- `.receive()` still delivers everything that was already sent, in order, and then returns `none`. That `none` is how `tallyScores` knows the game is over — no counting, no timers.
- `.send()` is refused with the channel-closed error on the usual `send() -> nothing errors` surface. Check it with `.failed()` if a late send is something you want to notice:

```ynz
let late = wire.send(9)
if (late.failed()) {
  print(late.message)
}
// The channel is closed - close() was called, so this value cannot be delivered.
// Check .failed() on the send, or send everything before close().
```

- Calling `.close()` again does nothing — it is safe to close twice.
- A send that was already waiting for room when the channel closed still goes through. Close means no *new* sends, not "throw away what was on its way."
- A channel you never close behaves exactly as before: `.receive()` waits until a value arrives.

`.close()` never pauses your task and cannot fail, so it can sit anywhere in a function. It belongs to the channel, not to a task handle — if you spawned a task with `let h = background worker(commands)`, you close `commands`, not `h`. For that reason the handle form needs its channel in a named binding:

```
let h = background doubler(makeWire())
// COMPILE ERROR: `doubler` gets its command channel from `makeWire()`, which is not a named
// binding — nothing outside the task can ever close that channel.
// Bind the channel first, then spawn: `let commands = makeWire()` and
// `let h = background doubler(commands, …)`. Call `commands.close()` when you are done sending.
```

### Sending an array or a map gives it away

An `int`, a `string`, or a `number` is copied into the channel — your binding keeps working after the send. An `array<T>` or a `map<K, V>` is different: the channel hands the *same* value to whichever task receives it, so `send()` gives it away and the compiler refuses any later read of that binding:

```ynz
let rows: array<int> = [1, 2, 3]
wire.send(rows)
print(rows.count())
// COMPILE ERROR: `rows` was sent into `wire` and cannot be used here — `send()` gave it away.
// If you still need `rows` after sending it, send a copy instead: `wire.send(rows.copy())`.
// If you only need it before the send, put this line above the `send()`.
```

Two more rules follow from the same idea:

- A parameter you send must be declared `give` on its function, so the caller's binding is given up too. Without the word, the compile error names the function and the one-word fix: `function producer(lend wire: channel<array<int>>, give rows: array<int>)`.
- You can only send something you hold whole. A field (`bucket.rows`), an item (`rows[0]`), the loop variable of a `for`, or a value built from other named values still belongs to someone here — the compile error tells you to send `.copy()` of it instead.

---

## Talking to a background task — handles

Storing the result of `background` gives you a **handle** — a two-way line to the running task:

```ynz
function gradeProspect(lend requests: channel<int>) -> int errors {
  let command = requests.receive()
  let jerseyNumber = command.or(0)
  return jerseyNumber * 2
}

function entrypoint() -> nothing {
  let requests: channel<int> = channel<int>(4)
  let scout = background gradeProspect(requests)
  scout.send(21)
  let graded = scout.receive()
  let grade = graded.or(0)
  print(`prospect grade: ${grade.toString()}`)
}
```

**Sending TO the task:** `scout.send(21)` delivers into the **first `channel<T>` parameter** of the spawned function. Here that's `requests`, so inside the task, `requests.receive()` reads what you sent. That's the whole convention: the task reads its messages from its first channel parameter — there is no hidden mailbox.

Because of that, the spawned function must take a channel parameter to receive messages:

```
let counter = background slowCount()
counter.send(7)
// COMPILE ERROR: This task takes no channel — it has no way to receive messages.
// Add a `channel<T>` parameter to the task's function and pass a channel at the
// spawn: `let h = background worker(commands)` — `h.send(v)` then feeds that channel.
```

**Receiving FROM the task:** `scout.receive()` gives you the next thing the task delivers. For a function like `gradeProspect` that returns a value, that's its completion value — typed with `errors`, because the task might have failed. Handle it like any other `errors` value (here, `graded.or(0)` falls back to 0).

One more rule: the handle form needs a function that can pause (one that uses `wait`, a channel, or calls something that does). For a pure number-cruncher that never pauses, use fire-and-forget `background crunch()` — handle support for those ships in a later milestone, and the compile error says exactly that.

---

## Ownership with background tasks

Background tasks might outlive the current function. This changes how ownership works.

The compiler infers whether to move or copy the argument — you don't have to write `.give` or `.copy` yourself. The IDE shows the inferred modifier as a muted hint at the call site.

**A `share`-signature function cannot be called via background:**

```ynz
function processData(share data: Data) -> nothing { ... }

background processData(data)
// COMPILE ERROR: Cannot share with a background task.
// processData's signature is `share data: Data`, but background tasks
// may outlive the current function. A shared borrow would dangle.
// Use a function that takes `give data: Data` instead, or call .copy()
// on data before passing to a give-signature function.
```

A shared borrow is only valid while its owner exists. A background task might still be running after the current function returns — so sharing would create a dangling reference. The function being called via `background` must take its parameters as `give`, OR the caller passes a `.copy()` (which creates an independent owned value).

**The one exception: `channel<T>` parameters.** A channel is built to be held by two tasks at once — that's its job — so passing a channel to a background task is always allowed, and you keep using your end afterward. Everything you saw in the channels section above relies on this.

**The compiler figures out which one to use** when the function signature is `give`:

If you don't use the value after the `background` call, the compiler infers `give` (transfers ownership to the task):

```ynz
function processEvent(give event: WebhookEvent) -> nothing { ... }

function handleWebhook(event: WebhookEvent) -> Response errors {
  background processEvent(event)             // event not used after — give inferred, ownership transfers
  return response.json({ status: "queued" })
}
```

If you do use the value after, you call `.copy()` to keep the original:

```ynz
function handleWebhook(event: WebhookEvent) -> Response errors {
  background processEvent(event.copy())      // explicit copy — original event stays usable
  log(`Queued: ${event.id}`)
  return response.json({ status: "queued", id: event.id })
}
```

**The IDE warns about large copies:**

```ynz
background processData(hugeDataset.copy())
const count = hugeDataset.count()
// IDE WARNING: hugeDataset (~500MB) was copied for the background task.
// Move hugeDataset.count() above the background call to avoid the copy.
```

---

## Error handling with concurrency

The `errors` system works exactly the same as everywhere else. Concurrency doesn't change the rules.

If a concurrent operation fails while others are still running:
- Operations already in progress run to completion — but their results are discarded
- Operations not yet started are cancelled before they begin
- The function exits with the error

```
function loadDashboard() -> Dashboard errors {
  let user = fetchCurrentUser()
  let feed = fetchFeed(user)
  let messages = fetchMessages(user)    // if this fails while feed is still running:
  let stats = fetchStats(user)          // feed result is discarded, stats is cancelled
  return buildDashboard(user, feed, messages, stats)
}
```

If `loadDashboard()` is marked `errors`, the error auto-propagates to the caller. Same rules as everywhere else.

---

## Heavy number-crunching also runs in parallel

Everything above is about waiting for network calls and disk reads — things that pause while
waiting for the outside world.  Yinz does the same thing for heavy CPU work.

If you have multiple calculations that don't depend on each other, the compiler runs them
on different cores at the same time:

```ynz
function analyzeGame(game: Game) -> Stats {
    let hits    = crunchStat(game.hits)
    let rbi     = crunchStat(game.rbi)
    let average = crunchStat(game.average)
    return combineStats(hits, rbi, average)
}
```

`hits`, `rbi`, and `average` don't depend on each other, so the compiler runs all three
`crunchStat` calls at the same time — each on its own core.  `combineStats` runs only after
all three finish.

You don't write anything special.  No `background`, no `wait`.  Just normal code.

The compiler only does this when `crunchStat` is actually doing heavy work (has a loop or
complex math inside).  Simple functions that return immediately aren't worth running in
parallel — the overhead of starting a new core would cost more than just running it directly.

Your editor shows what's happening in muted text:

```ynz
function analyzeGame(game: Game) -> Stats {
    let hits    = crunchStat(game.hits)    // runs at the same time as line 4, line 5 — separate core
    let rbi     = crunchStat(game.rbi)     // runs at the same time as line 3, line 5 — separate core
    let average = crunchStat(game.average) // runs at the same time as line 3, line 4 — separate core
    return combineStats(hits, rbi, average)
}
```

These annotations are read-only notes — nothing to change.  They show you what the compiler
decided so you understand what's actually running in parallel.

---

## That's it

```
wait           // complete this before continuing
background     // run this outside this function's lifetime
channel<T>     // send values between tasks safely
```

Two keywords and one type. Everything else is automatic. The compiler builds the dependency
graph, runs independent operations in parallel on all available cores, sequences writes to
the same resource, manages core pools, applies backpressure when a producer outruns a
receiver, and handles cleanup. You don't see any of it — you just write normal code.
