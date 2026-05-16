# Concurrency

Yinz runs code concurrently for you. You write normal, sequential-looking code. The compiler analyzes what each operation depends on and runs independent operations at the same time.

Two keywords. That's the entire developer API:
- `wait` — force something to complete before continuing
- `background` — run something separately, outside this function's lifetime

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
wait http.post("api.stripe.com/charge", chargeData)
http.post("api.email.com/send", receipt)
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

```
let monitor = background watchHealth()
monitor.send("get-status")
let status = monitor.receive()
```

If you store the result of `background`, you get a handle you can communicate with via `.send()` and `.receive()`. If you don't store it, it's fire-and-forget.

---

## Ownership with background tasks

Background tasks might outlive the current function. This changes how ownership works.

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

## That's it

```
wait           // complete this before continuing
background     // run this outside this function's lifetime
```

Two keywords. Everything else is automatic. The compiler builds the dependency graph, runs independent operations in parallel, sequences writes to the same resource, manages thread pools, and handles cleanup. You don't see any of it — you just write normal code.
