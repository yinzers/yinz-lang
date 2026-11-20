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
- `channel<T>` — pass values from one task to another safely

---

## How it works — the compiler does the thinking

The compiler maps which line needs which value. Lines that don't need each other run at the same time. A line that needs a value waits for it automatically.

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

You write 6 lines of normal code. The compiler runs the three independent fetches at the same time.

---

## The IDE shows you the execution plan

The IDE marks which lines run at the same time and which run one after another, so you can always see what the compiler decided.

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

The compiler uses the ownership words you already write (`share`, `lend`) to tell reads from writes. There are no new rules to learn.

```
function getUser(share db: Database, id: string) -> User errors
//               ^^^^^ share = only reads — safe to run alongside other reads

function saveUser(lend db: Database, user: User) -> nothing errors
//                ^^^^ lend = may change it — the compiler runs this in the right order
```

`share` on a parameter means the function only reads that value. Several readers can run at the same time.

`lend` on a parameter means the function may change that value. Changes to the same value run in order. Two functions that only `share` the same data can always run at the same time.

---

## `wait` — when you need explicit ordering

**Pausing is automatic — you never type `wait` to make a slow call pause.** When a call has to wait for the outside world (a network fetch, `sleep`, a file read), the compiler pauses your function by itself. The IDE shows that pause as a muted `wait` hint. You don't write it.

**`wait` is only for ordering.** Use it when one call must finish before the next line runs, but the compiler can't see that. That happens when the two calls touch different things and no value passes between them.

Most code never needs `wait`. The compiler orders reads and writes from the ownership words in your function signatures. Use `wait` when you need an order the compiler can't work out on its own.

**When to use `wait` — ordered side effects:**

```
function processOrder(order: Order) -> nothing errors {
  wait chargePayment(order)          // must complete before anything else
  sendConfirmationEmail(order)       // only runs after payment succeeds
  updateInventory(order)             // runs concurrently with email — fine
}
```

Without `wait`, the compiler would run all three at the same time, because none of them uses another's result. `wait chargePayment(order)` tells the compiler: "finish this before moving on."

**When to use `wait` — external API ordering:**

```
wait request.post(`api.stripe.com/charge`, chargeData)
request.post(`api.email.com/send`, receipt)
```

The compiler can't tell that these two HTTP calls are logically related. They're just two HTTP posts to different URLs. `wait` makes the intended ordering explicit.

**When to use `wait` — one request at a time:**

```
for (page in pageNumbers) {
  let result = wait fetchPage(page)     // finish each fetch before starting the next
  results.add(result)
}
```

A loop already runs one pass at a time, so this loop would fetch the pages in order even without `wait`. Writing `wait` makes that intent clear, and it works anywhere you need one-at-a-time behavior.

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

## Loops run one pass at a time

Separate statements that don't need each other run at the same time. The passes of a loop do not: each pass finishes before the next one starts.

---

## `background` — tasks that keep going after this function returns

`background` starts a task on its own. The current function doesn't wait for it, and the task can still be running after the function returns.

**Fire and forget — don't store the result:**

```
background sendWelcomeEmail(user)
background logAnalytics(event)
background retryPayment(payoutId)
// the function carries on right away — these tasks run on their own
```

Each task runs on its own. If it fails, it has to deal with the failure itself. The caller never waits and never sees the result. This suits small, non-critical tasks such as sending an email or logging.

**Long-running — keep the handle so you can talk to the task:**

```ynz
let scout = background gradeProspect(requests)
```

If you store the result of `background`, you get a **handle**: a line you can use to talk to the running task with `.send()` and `.receive()`. If you don't store it, the task is fire-and-forget. It is the same keyword either way. Handles are built on channels, so read the next section first, then see "Talking to a background task" below.

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

`relayScores` runs as a background task and puts three scores into the channel. `entrypoint` takes them out one at a time. If the channel is empty when you call `.receive()`, your task pauses until a value arrives. You never write locking or waiting code — the channel does it for you.

**`.receive()` gives you a `maybe<T>`, not a plain `T`.** A channel can be closed (see "Closing a channel" below). Once it is closed and empty, `.receive()` returns `none` instead of waiting forever. So you read a delivery the way you read any `maybe` value: `.or(fallback)` when you just want a default, or `.exists()` and `.value` when you need to know the stream has ended.

**A channel is the one value you can hand to a background task and keep using yourself.** Both sides share the same channel safely — that is the point of a channel. (Every other value must be given away or copied — see "Ownership with background tasks" below.)

### Every channel has a size limit

There is no unlimited channel. When a producer fills the channel faster than the receiver empties it, `send()` pauses the producer until a slot frees up.

In the example above the capacity is 1: `relayScores` sends the first score, then pauses on the second send until `entrypoint` receives the first. This slow-down is called **backpressure**: a fast sender is made to wait for a slow receiver.

**A paused producer is backpressure working correctly, not a freeze.** The producer isn't stuck. It is waiting for the receiver to catch up, and it continues the moment a slot opens. Without the limit, a fast producer would quietly fill up memory until the program crashed.

Need more room? Pass a bigger number: `channel<int>(10000)`. There is deliberately no "unlimited" option.

### The rules channels follow

**The value type is fixed when you create the channel.** A `channel<int>` carries `int` values only:

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

**A channel operation gets its own line.** `.send()` and `.receive()` can pause your task, so each one must be its own statement — `wire.send(5)` on its own line, or `let value = wire.receive()`. Putting one inside a bigger expression is a compile error that tells you to move it out into a named variable first.

**A channel can carry these values:** `int`, `float`, `boolean`, `string`, `number`, `array<T>`, and `map<K, V>`. Sending a `shape` value isn't supported yet — the compile error suggests sending its fields as separate values and building the shape again on the receiving side.

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
- `.send()` is refused with a channel-closed error. Because `send()` is declared `-> nothing errors`, you can check it with `.failed()` if you want to notice a late send:

```ynz
let late = wire.send(9)
if (late.failed()) {
  print(late.message)
}
// The channel is closed — close() was called, so this value cannot be delivered.
// Check .failed() on the send, or send everything before close().
```

- Calling `.close()` again does nothing — it is safe to close twice.
- A send that was already waiting for room when the channel closed still goes through. Close means no *new* sends. It does not throw away what was already on its way.
- A channel you never close keeps working as before: `.receive()` waits until a value arrives.

`.close()` never pauses your task and cannot fail, so it can sit anywhere in a function. It belongs to the channel, not to a handle. If you started a task with `let h = background worker(commands)`, you close `commands`, not `h`. For that reason the handle form needs the channel stored in a named variable:

```
let h = background doubler(makeWire())
// COMPILE ERROR: `doubler` gets its command channel from `makeWire()`, which is not a named
// binding — nothing outside the task can ever close that channel.
// Bind the channel first, then spawn: `let commands = makeWire()` and
// `let h = background doubler(commands, …)`. Call `commands.close()` when you are done sending.
```

### Sending an array or a map gives it away

An `int`, a `string`, or a `number` is copied into the channel — your variable keeps working after the send. An `array<T>` or a `map<K, V>` is different: the channel hands the *same* value to whichever task receives it, so `send()` gives it away and the compiler refuses any later use of that variable:

```ynz
let rows: array<int> = [1, 2, 3]
wire.send(rows)
print(rows.count())
// COMPILE ERROR: `rows` was sent into `wire` and cannot be used here — `send()` gave it away.
// If you still need `rows` after sending it, send a copy instead: `wire.send(rows.copy())`.
// If you only need it before the send, put this line above the `send()`.
```

Two more rules follow from the same idea:

- A parameter you send must be declared `give` on its function, so the caller's variable is given up too. Without the word, the compile error names the function and the one-word fix: `function producer(lend wire: channel<array<int>>, give rows: array<int>)`.
- You can only send something you own whole. A field (`bucket.rows`), an item (`rows[0]`), the loop variable of a `for`, or a value built from other named values still belongs to something else here — the compile error tells you to send `.copy()` of it instead.

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

**Sending TO the task:** `scout.send(21)` delivers into the **first `channel<T>` parameter** of the function you started. Here that's `requests`, so inside the task, `requests.receive()` reads what you sent. That's the whole rule: the task reads its messages from its first channel parameter. There is no hidden mailbox.

Because of that, the function you start must take a channel parameter to receive messages:

```
let counter = background slowCount()
counter.send(7)
// COMPILE ERROR: This task takes no channel — it has no way to receive messages.
// Add a `channel<T>` parameter to the task's function and pass a channel at the
// spawn: `let h = background worker(commands)` — `h.send(v)` then feeds that channel.
```

**Receiving FROM the task:** `scout.receive()` gives you the next thing the task delivers. For a function like `gradeProspect` that returns a value, that's its final result. It comes with `errors`, because the task might have failed. Handle it like any other `errors` value (here, `graded.or(0)` falls back to 0).

One more rule: the handle form needs a function that can pause (one that uses `wait` or a channel, or calls something that does). For a plain calculation that never pauses, use fire-and-forget `background crunch()`. Handles for those are not supported yet, and the compile error says so.

---

## Ownership with background tasks

A background task can still be running after the current function returns. This changes how ownership works.

The compiler works out whether to give away or copy each argument, so you don't have to write `.give` or `.copy` yourself. The IDE shows its choice as a muted hint at the call.

**A function whose parameter is `share` cannot be started with `background`:**

```ynz
function processData(share data: Data) -> nothing { ... }

background processData(data)
// COMPILE ERROR: Cannot share with a background task.
// processData's signature is `share data: Data`, but background tasks
// may outlive the current function. A shared borrow would dangle.
// Use a function that takes `give data: Data` instead, or call .copy()
// on data before passing to a give-signature function.
```

A `share` parameter only works while the original owner still has the value. A background task might still be running after the current function returns and the value is gone. So the function you start with `background` must take its parameters as `give`, or the caller passes a `.copy()` (an independent value the task owns).

**The one exception: `channel<T>` parameters.** A channel is made to be held by two tasks at once, so passing a channel to a background task is always allowed, and you keep using your end afterward. Everything in the channels section above relies on this.

**When the parameter is `give`, the compiler picks give-away or copy for you:**

If you don't use the value after the `background` call, the compiler gives it away (ownership moves to the task):

```ynz
function processEvent(give event: WebhookEvent) -> nothing { ... }

function handleWebhook(event: WebhookEvent) -> Response errors {
  background processEvent(event)             // event not used after — it is given away
  return response.json({ status: `queued` })
}
```

If you do use the value after, you call `.copy()` to keep the original:

```ynz
function handleWebhook(event: WebhookEvent) -> Response errors {
  background processEvent(event.copy())      // explicit copy — the original event stays usable
  log(`Queued: ${event.id}`)
  return response.json({ status: `queued`, id: event.id })
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

`errors` works the same as everywhere else. Running things at the same time doesn't change the rules.

If one operation fails while others are still running:
- Operations already in progress run to the end, but their results are thrown away
- Operations not yet started are cancelled before they begin
- The function ends with the error

```
function loadDashboard() -> Dashboard errors {
  let user = fetchCurrentUser()
  let feed = fetchFeed(user)
  let messages = fetchMessages(user)    // if this fails while feed is still running:
  let stats = fetchStats(user)          // feed result is discarded, stats is cancelled
  return buildDashboard(user, feed, messages, stats)
}
```

Because `loadDashboard()` is marked `errors`, the failure goes up to the caller automatically. Same rules as everywhere else.

---

## Heavy number-crunching also runs at the same time

Everything above is about waiting for network calls and file reads — things that pause while
waiting for the outside world.  Yinz does the same for heavy calculations.

If you have several calculations that don't need each other, the compiler runs them
on different processor cores at the same time:

```ynz
function analyzeGame(game: Game) -> Stats {
    let hits    = crunchStat(game.hits)
    let rbi     = crunchStat(game.rbi)
    let average = crunchStat(game.average)
    return combineStats(hits, rbi, average)
}
```

`hits`, `rbi`, and `average` don't need each other, so the compiler runs all three
`crunchStat` calls at the same time — each on its own core.  `combineStats` runs only after
all three finish.

You don't write anything special.  No `background`, no `wait`.  Just normal code.

The compiler only does this when `crunchStat` does heavy work (a loop or
lots of math inside).  A small function that returns right away isn't worth it:
starting work on another core would take longer than just running it.

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
decided, so you know what is running at the same time.

---

## That's it

```
wait           // finish this before the next line
background     // start a task that keeps going after this function returns
channel<T>     // pass values between tasks safely
```

Two keywords and one type. Everything else is automatic. The compiler works out which lines
can run at the same time, spreads them over all available cores, keeps changes to the same
value in order, slows a fast sender down when the receiver falls behind, and cleans up
afterward. You write normal code.
