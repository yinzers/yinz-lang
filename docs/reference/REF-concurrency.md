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

Yinz runs code concurrently for you. *Concurrently* means several tasks make progress at the same time. A *task* is a piece of work that runs on its own. You write normal code that reads top to bottom. The compiler works out what each line needs and runs lines that don't need each other at the same time.

Two keywords and one type are the whole toolkit:
- `wait` — finish something before the next line runs
- `background` — start a task that keeps going after this function returns
- `Channel<T>` — pass values from one task to another safely

---

## How it works — the compiler does the thinking

The compiler maps which line needs which value. Lines that don't need each other run at the same time. A line that needs a value waits for it automatically.

```ynz
function fetchCurrentUser() -> Int {
  wait sleep(10)
  return 1
}

function fetchFeed(user: Int) -> Int {
  wait sleep(10)
  return user + 10
}

function fetchMessages(user: Int) -> Int {
  wait sleep(10)
  return user + 20
}

function fetchStats(user: Int) -> Int {
  wait sleep(10)
  return user + 30
}

function buildDashboard(user: Int, feed: Int, messages: Int, stats: Int) -> Int {
  return user + feed + messages + stats
}

function loadDashboard() -> Int {
  let user = fetchCurrentUser()
  let feed = fetchFeed(user)
  let messages = fetchMessages(user)
  let stats = fetchStats(user)
  return buildDashboard(user, feed, messages, stats)
}

function entrypoint() -> Nothing {
  let total = loadDashboard()
  print(total.toString())
}
```

What the compiler sees:
- `fetchCurrentUser()` depends on nothing → runs immediately
- `fetchFeed`, `fetchMessages`, `fetchStats` all need `user` → all three wait for `user`, then run at the same time
- `buildDashboard` needs all four → waits for everything

You write 6 lines of normal code. The compiler runs the three independent fetches at the same time.

---

## The IDE shows you the execution plan

The IDE adds a muted note after each line that runs at the same time as other lines. The note names the other lines, counted from the top of the file. For `loadDashboard` in the example above, the IDE shows:

```
function loadDashboard() -> Int {
  let user = fetchCurrentUser()
  let feed = fetchFeed(user)             // runs at the same time as line 28, line 29
  let messages = fetchMessages(user)     // runs at the same time as line 27, line 29
  let stats = fetchStats(user)           // runs at the same time as line 27, line 28
  return buildDashboard(user, feed, messages, stats)
}
```

A line with no note runs on its own, in order. Hover over a note to read why: the lines run together because no data flows between them. Writing `wait` before the second call forces them to run in order.

---

## Reads vs writes — the compiler knows the difference

The compiler uses the ownership words you already write (`share`, `lend`) to tell reads from writes. There are no new rules to learn.

```ynz
shape Database {
  rows: Int
}

function countRows(share db: Database) -> Int {
  // share = only reads — safe to run alongside other reads
  return db.rows
}

function addRow(lend db: Database) -> Nothing {
  // lend = may change it — the compiler runs this in the right order
  db.rows = db.rows + 1
}

function entrypoint() -> Nothing {
  let db: Database = { rows: 0 }
  addRow(db)
  let count = countRows(db)
  print(count.toString())
}
```

`share` on a parameter means the function only reads that value. Several readers can run at the same time.

`lend` on a parameter means the function may change that value. Changes to the same value run in order. Two functions that only `share` the same data can always run at the same time.

---

## `wait` — when you need explicit ordering

**Calling a function that waits is automatic — you never type `wait` before it.** When a function you call has to wait for the outside world, the compiler pauses your function until the call is done. The base pause itself is the one place you write `wait`: `wait sleep(10)` pauses for 10 milliseconds. A bare `sleep(10)` does not pause.

**`wait` is only for ordering.** Use it when one call must finish before the next line runs, but the compiler can't see that. That happens when the two calls touch different things and no value passes between them.

Most code never needs `wait`. The compiler orders reads and writes from the ownership words in your function signatures. Use `wait` when you need an order the compiler can't work out on its own.

**When to use `wait` — ordered side effects:**

```ynz
function chargePayment(orderId: Int) -> Nothing {
  wait sleep(20)
  print(`charged order ${orderId.toString()}`)
}

function sendConfirmationEmail(orderId: Int) -> Nothing {
  print(`emailed order ${orderId.toString()}`)
}

function updateInventory(orderId: Int) -> Nothing {
  print(`updated stock for order ${orderId.toString()}`)
}

function processOrder(orderId: Int) -> Nothing {
  wait chargePayment(orderId)          // must complete before anything else
  sendConfirmationEmail(orderId)       // only runs after payment succeeds
  updateInventory(orderId)             // runs concurrently with email — fine
}

function entrypoint() -> Nothing {
  processOrder(1)
}
```

Each call passes no value to the next, so `wait chargePayment(orderId)` is how you say "finish this before moving on."

**When to use `wait` — one request at a time:**

```ynz
function fetchPage(page: Int) -> Int {
  wait sleep(10)
  return page * 100
}

function entrypoint() -> Nothing {
  let results: Array<Int> = []
  for (page in range(1, 4)) {
    let result = wait fetchPage(page)     // finish each fetch before starting the next
    results.add(result)
  }
  print(results.count().toString())
}
```

A loop already runs one pass at a time, so this loop would fetch the pages in order even without `wait`. Writing `wait` makes that intent clear, and it works anywhere you need one-at-a-time behavior.

**When you do NOT need `wait` — data dependencies:**

```ynz
function fetchUser(id: Int) -> Int {
  wait sleep(10)
  return id + 1000
}

function fetchPermissions(user: Int) -> Int {
  wait sleep(10)
  return user + 1
}

function buildProfile(id: Int) -> Int {
  let user = fetchUser(id)
  let permissions = fetchPermissions(user)       // depends on user — auto-waits
  let profile = user + permissions               // depends on both — auto-waits
  return profile
}

function entrypoint() -> Nothing {
  let profile = buildProfile(1)
  print(profile.toString())
}
```

`fetchPermissions(user)` passes `user` as an argument, so the compiler already knows it must wait. `wait` would be redundant here.

---

## Loops run one pass at a time

Separate statements that don't need each other run at the same time. The passes of a loop do not: each pass finishes before the next one starts.

---

## `background` — tasks that keep going after this function returns

`background` starts a task on its own. The current function doesn't wait for it, and the task can still be running after the function returns.

**Fire and forget — don't store the result:**

```ynz
function sendWelcomeEmail(userId: Int) -> Nothing {
  wait sleep(10)
  print(`welcome email sent to ${userId.toString()}`)
}

function logAnalytics(eventId: Int) -> Nothing {
  print(`logged event ${eventId.toString()}`)
}

function entrypoint() -> Nothing {
  background sendWelcomeEmail(7)
  background logAnalytics(3)
  // the function carries on right away — these tasks run on their own
  wait sleep(50)
}
```

Each task runs on its own. If it fails, it has to deal with the failure itself. The caller never waits and never sees the result. This suits small, non-critical tasks such as sending an email or logging.

**Long-running — keep the handle so you can talk to the task:**

```ynz
function gradeProspect(lend requests: Channel<Int>) -> Int errors {
  let command = requests.receive()
  return command.or(0)
}

function entrypoint() -> Nothing {
  let requests: Channel<Int> = Channel<Int>(4)
  let scout = background gradeProspect(requests)
}
```

If you store the result of `background`, you get a **handle**: a line you can use to talk to the running task with `.send()` and `.receive()`. If you don't store it, the task is fire-and-forget. It is the same keyword either way. Handles are built on channels, so read the next section first, then see "Talking to a background task" below.

---

## Channels — sending values between tasks

A channel carries values of one type from one task to another. You create one with `Channel<T>()`, where `T` is the type of value it carries:

```ynz
function entrypoint() -> Nothing {
  let scores: Channel<Int> = Channel<Int>()        // holds up to 64 values (the default)
  let names: Channel<String> = Channel<String>(8)  // holds up to 8 values
}
```

The number is the channel's **capacity** — how many values it can hold at once. Leave the parens empty and you get the default of 64. The IDE shows the default as a muted `64` hint inside the empty parens; click it to write `64` into your source.

One task puts values in with `.send(value)`. Another task takes them out with `.receive()`:

```ynz
function relayScores(lend wire: Channel<Int>) -> Nothing {
  wire.send(2)
  wire.send(5)
  wire.send(3)
}

function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>(1)
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

`relayScores` runs as a background task and puts three scores into the channel. `entrypoint` takes them out one at a time. If the channel is empty when you call `.receive()`, your task pauses until a value arrives. You never write locking or waiting code — the channel does it for you.

**`.receive()` gives you a `Maybe<T>`, not a plain `T`.** A channel can be closed (see "Closing a channel" below). Once it is closed and empty, `.receive()` returns `none` instead of waiting forever. So you read a delivery the way you read any `Maybe` value: `.or(fallback)` when you just want a default, or `.exists()` and `.value` when you need to know the stream has ended.

**A channel is the one value you can hand to a background task and keep using yourself.** Both sides share the same channel safely — that is the point of a channel. (Every other value must be given away or copied — see "Ownership with background tasks" below.)

### Every channel has a size limit

There is no unlimited channel. When a producer fills the channel faster than the receiver empties it, `send()` pauses the producer until a slot frees up.

In the example above the capacity is 1: `relayScores` sends the first score, then pauses on the second send until `entrypoint` receives the first. This slow-down is called **backpressure**: a fast sender is made to wait for a slow receiver.

**A paused producer is backpressure working correctly, not a freeze.** The producer isn't stuck. It is waiting for the receiver to catch up, and it continues the moment a slot opens. Without the limit, a fast producer would quietly fill up memory until the program crashed.

Need more room? Pass a bigger number: `Channel<Int>(10000)`. There is deliberately no "unlimited" option.

### The rules channels follow

**The value type is fixed when you create the channel.** A `Channel<Int>` carries `Int` values only:

```ynz
function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>()
  wire.send(`three`)
}
```

The compiler reports:

```
This channel carries `Int` values, but you're sending `String`.
```

Send an `Int` value, or create a `Channel<String>` for this data.

**The capacity must be at least 1:**

```ynz
function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>(0)
}
```

```
A channel's capacity must be at least 1, but got 0.
```

Use a positive capacity, such as `Channel<Int>(64)`. For very large buffering, pass a large number. There is no unbounded channel.

**A channel operation gets its own line.** `.send()` and `.receive()` can pause your task, so each one must be its own statement — `wire.send(5)` on its own line, or `let value = wire.receive()`. Putting one inside a bigger expression is a compile error that tells you to move it out into a named variable first.

**A channel can carry these values:** `Int`, `Float`, `Boolean`, `String`, `Number`, `Array<T>`, and `Map<K, V>`. Sending a `shape` value isn't supported yet — the compile error suggests sending its fields as separate values and building the shape again on the receiving side.

### Closing a channel

A producer tells the other side "nothing more is coming" with `.close()`:

```ynz
function relayScores(lend wire: Channel<Int>) -> Nothing {
  wire.send(2)
  wire.send(5)
  wire.send(3)
  wire.close()
}

function tallyScores(lend wire: Channel<Int>) -> Int {
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

function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>(4)
  background relayScores(wire)
  let total = tallyScores(wire)
  print(`total: ${total.toString()}`)
}
```

After `close()`:

- `.receive()` still delivers everything that was already sent, in order, and then returns `none`. That `none` is how `tallyScores` knows the game is over — no counting, no timers.
- `.send()` is refused with a channel-closed error. Because `send()` is declared `-> Nothing errors`, you can check it with `.failed()` if you want to notice a late send:

```ynz
function entrypoint() -> Nothing {
  let wire: Channel<Int> = Channel<Int>(4)
  wire.close()
  let late = wire.send(9)
  if (late.failed()) {
    print(late.message)
  }
}
```

The program prints:

```
The channel is closed — close() was called, so this value cannot be delivered. Check .failed() on the send, or send everything before close().
```

- Calling `.close()` again does nothing — it is safe to close twice.
- A send that was already waiting for room when the channel closed still goes through. Close means no *new* sends. It does not throw away what was already on its way.
- A channel you never close keeps working as before: `.receive()` waits until a value arrives.

`.close()` never pauses your task and cannot fail, so it can sit anywhere in a function. It belongs to the channel, not to a handle. If you started a task with `let scout = background worker(commands)`, you close `commands`, not `scout`. For that reason the handle form needs the channel stored in a named variable:

```ynz
function makeWire() -> Channel<Int> {
  let wire: Channel<Int> = Channel<Int>(4)
  return wire
}

function doubler(lend commands: Channel<Int>) -> Int errors {
  let command = commands.receive()
  let value = command.or(0)
  return value * 2
}

function entrypoint() -> Nothing {
  let h = background doubler(makeWire())
}
```

```
`doubler` gets its command channel from `makeWire()`, which is not a named binding — nothing outside the task can ever close that channel.
```

Bind the channel first (`let commands = makeWire()`), then spawn with `background doubler(commands)`. Call `commands.close()` when you are done sending.

### Sending an array or a map gives it away

An `Int`, a `String`, or a `Number` is copied into the channel — your variable keeps working after the send. An `Array<T>` or a `Map<K, V>` is different: the channel hands the *same* value to whichever task receives it, so `send()` gives it away and the compiler refuses any later use of that variable:

```ynz
function entrypoint() -> Nothing {
  let wire: Channel<Array<Int>> = Channel<Array<Int>>(4)
  let rows: Array<Int> = [1, 2, 3]
  wire.send(rows)
  print(rows.count().toString())
}
```

```
`rows` was sent into `wire` and cannot be used here — `send()` gave it away.
```

If you still need `rows` after sending it, send a copy instead: `wire.send(rows.copy())`. If you only need it before the send, put that line above the `send()`.

Two more rules follow from the same idea:

- A parameter you send must be declared `give` on its function, so the caller's variable is given up too. Without the word, the compile error names the function and the one-word fix: `function producer(lend wire: Channel<Array<Int>>, give rows: Array<Int>)`.
- You can only send something you own whole. A field (`bucket.rows`), an item (`rows[0]`), the loop variable of a `for`, or a value built from other named values still belongs to something else here — the compile error tells you to send `.copy()` of it instead.

---

## Talking to a background task — handles

Storing the result of `background` gives you a **handle** — a two-way line to the running task:

```ynz
function gradeProspect(lend requests: Channel<Int>) -> Int errors {
  let command = requests.receive()
  let jerseyNumber = command.or(0)
  return jerseyNumber * 2
}

function entrypoint() -> Nothing {
  let requests: Channel<Int> = Channel<Int>(4)
  let scout = background gradeProspect(requests)
  scout.send(21)
  let graded = scout.receive()
  let grade = graded.or(0)
  print(`prospect grade: ${grade.toString()}`)
}
```

**Sending TO the task:** `scout.send(21)` delivers into the **first `Channel<T>` parameter** of the function you started. Here that's `requests`, so inside the task, `requests.receive()` reads what you sent. That's the whole rule: the task reads its messages from its first channel parameter. There is no hidden mailbox.

Because of that, the function you start must take a channel parameter to receive messages:

```ynz
function slowCount() -> Int errors {
  wait sleep(10)
  return 3
}

function entrypoint() -> Nothing {
  let counter = background slowCount()
  counter.send(7)
}
```

```
This task takes no channel — it has no way to receive messages.
```

Add a `Channel<T>` parameter to the task's function and pass a channel when you start it: `let h = background worker(commands)`. Then `h.send(value)` feeds that channel.

**Receiving FROM the task:** `scout.receive()` gives you the next thing the task delivers. For a function like `gradeProspect` that returns a value, that's its final result. It comes with `errors`, because the task might have failed. Handle it like any other `errors` value (here, `graded.or(0)` falls back to 0).

One more rule: the handle form needs a function that can pause (one that uses `wait` or a channel, or calls something that does). For a plain calculation that never pauses, use fire-and-forget `background crunch()`. Handles for those are not supported yet, and the compile error says so.

**When the handle goes out of scope, the task stops.** A handle belongs to the block it was declared in. When that block ends (or the function returns), the task stops at its next pause, and anything it would have done after that never runs:

```ynz
function worker() -> Int errors {
  wait sleep(20)
  print(`worker: after first sleep`)
  wait sleep(20)
  print(`worker: done`)
  return 42
}

function entrypoint() -> Nothing {
  if (true) {
    let h = background worker()
    print(`leaving the block`)
  }
  // `h` is gone, so `worker` was stopped. "worker: done" never prints.
  print(`block exited`)
  wait sleep(150)
  print(`finished`)
}
```

If you want the task to finish, keep its handle alive until you have what you need, for example by calling `h.receive()`. A fire-and-forget `background worker()` has no handle, so nothing stops it: it runs until it ends or until `entrypoint` returns.

---

## Ownership with background tasks

A background task can still be running after the current function returns. This changes how ownership works.

The compiler works out whether to give away or copy each argument, so you don't have to write `.give` or `.copy` yourself. The IDE shows its choice as a muted hint at the call.

**A function whose parameter is `share` cannot be started with `background`:**

```ynz
shape Data {
  size: Int
}

function processData(share data: Data) -> Nothing {
  print(data.size.toString())
}

function entrypoint() -> Nothing {
  let data: Data = { size: 10 }
  background processData(data)
}
```

```
Cannot use `background` with a function that borrows its arguments.
```

Change the parameter to `give`, or pass a copy: `background processData(data.copy())` to a function that takes `give`.

A `share` parameter only works while the original owner still has the value. A background task might still be running after the current function returns and the value is gone. So the function you start with `background` must take its parameters as `give`, or the caller passes a `.copy()` (an independent value the task owns).

**The one exception: `Channel<T>` parameters.** A channel is made to be held by two tasks at once, so passing a channel to a background task is always allowed, and you keep using your end afterward. Everything in the channels section above relies on this.

**A union cannot go to a generic parameter of a `background` function.** A task keeps its own copy of a union argument, and it can make that copy only when the function declares the union type. A generic parameter gives the task no union slot to copy into, so the compiler refuses it:

```ynz
shape Circle {
  radius: Int
}

shape Tag {
  label: String
}

function eat<T>(held: T) -> Nothing {
  print(`eaten`)
}

function entrypoint() -> Nothing {
  let tag: Tag = { label: `a` }
  let choice: Circle | Tag = tag
  background eat(choice)
}
```

```
Cannot give the union `Circle | Tag` to `eat`'s generic parameter `held` in a `background` call.
```

Declare `held` as `Circle | Tag` in `eat`'s signature instead of a type parameter, or put the union in a field of a shape and pass the shape.

**When the parameter is `give`, the compiler picks give-away or copy for you:**

If you don't use the value after the `background` call, the compiler gives it away (ownership moves to the task):

```ynz
shape WebhookEvent {
  id: Int
  kind: String
}

function processEvent(give event: WebhookEvent) -> Nothing {
  print(event.kind)
}

function handleWebhook(give event: WebhookEvent) -> Int {
  background processEvent(event)             // event not used after — it is given away
  return 202
}

function entrypoint() -> Nothing {
  let event: WebhookEvent = { id: 1, kind: `push` }
  let status = handleWebhook(event)
  print(status.toString())
  wait sleep(50)
}
```

If you do use the value after, you call `.copy()` to keep the original:

```ynz
shape WebhookEvent {
  id: Int
  kind: String
}

function processEvent(give event: WebhookEvent) -> Nothing {
  print(event.kind)
}

function handleWebhook(give event: WebhookEvent) -> Int {
  background processEvent(event.copy())      // explicit copy — the original event stays usable
  print(`Queued: ${event.id.toString()}`)
  return 202
}

function entrypoint() -> Nothing {
  let event: WebhookEvent = { id: 1, kind: `push` }
  let status = handleWebhook(event)
  print(status.toString())
  wait sleep(50)
}
```

**The IDE warns about large copies.** When a `.copy()` handed to a `background` task is larger than 64 bytes and you could give the value away instead, the IDE shows a hint at the call suggesting `give`. Move the later use of the value above the `background` call to avoid the copy.

---

## Error handling with concurrency

`errors` works the same as everywhere else. Running things at the same time doesn't change the rules. A function marked `errors` can call other `errors` functions, and a failure goes up to its caller:

```ynz
function fetchCurrentUser() -> Int errors {
  return 1
}

function fetchFeed(user: Int) -> Int errors {
  wait sleep(10)
  return user + 10
}

function fetchMessages(user: Int) -> Int errors {
  wait sleep(10)
  return user + 20
}

function loadDashboard() -> Int errors {
  let user = fetchCurrentUser()
  let feed = fetchFeed(user)
  let messages = fetchMessages(user)
  return user + feed + messages
}

function entrypoint() -> Nothing {
  let total = loadDashboard()
  let safeTotal = total.or(0)
  print(safeTotal.toString())
}
```

`fetchFeed` and `fetchMessages` run at the same time. Because `loadDashboard()` is marked `errors`, a failure in either one goes up to its caller. `entrypoint` handles it with `.or(0)`. See [REF-errors.md](REF-errors.md).

---

## Heavy number-crunching also runs at the same time

Everything above is about waiting for network calls and file reads — things that pause while
waiting for the outside world.  Yinz does the same for heavy calculations.

If you have several calculations that don't need each other, the compiler runs them
on different processor cores at the same time:

```ynz
function crunchStat(seed: Int) -> Int {
  let total = 0
  for (step in range(0, 1000000)) {
    total = total + (step * seed) % 7
  }
  return total
}

function combineStats(hits: Int, rbi: Int, average: Int) -> Int {
  return hits + rbi + average
}

function analyzeGame(hitsSeed: Int, rbiSeed: Int, averageSeed: Int) -> Int {
  let hits    = crunchStat(hitsSeed)
  let rbi     = crunchStat(rbiSeed)
  let average = crunchStat(averageSeed)
  return combineStats(hits, rbi, average)
}

function entrypoint() -> Nothing {
  let result = analyzeGame(2, 3, 5)
  print(result.toString())
}
```

`hits`, `rbi`, and `average` don't need each other, so the compiler runs all three
`crunchStat` calls at the same time — each on its own core.  `combineStats` runs only after
all three finish.

You don't write anything special.  No `background`, no `wait`.  Just normal code.

The compiler only does this when `crunchStat` does heavy work (a loop or
lots of math inside).  A small function that returns right away isn't worth it:
starting work on another core would take longer than just running it.

Your editor shows what is happening in muted text. Line numbers count from the top of the example above:

```
function analyzeGame(hitsSeed: Int, rbiSeed: Int, averageSeed: Int) -> Int {
  let hits    = crunchStat(hitsSeed)       // runs at the same time as line 15, line 16 — separate core
  let rbi     = crunchStat(rbiSeed)        // runs at the same time as line 14, line 16 — separate core
  let average = crunchStat(averageSeed)    // runs at the same time as line 14, line 15 — separate core
  return combineStats(hits, rbi, average)
}
```

These annotations are read-only notes — nothing to change.  They show you what the compiler
decided, so you know what is running at the same time.

---

## That's it

```
wait           // finish this before the next line
background     // start a task that keeps going after this function returns
Channel<T>     // pass values between tasks safely
```

Two keywords and one type. Everything else is automatic. The compiler works out which lines
can run at the same time, spreads them over all available cores, keeps changes to the same
value in order, slows a fast sender down when the receiver falls behind, and cleans up
afterward. You write normal code.
