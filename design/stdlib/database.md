# Database Standard Library — Early Thoughts

> **STATUS: NOT PLANNED — JUST THOUGHTS**
> This file captures early design ideas from a brainstorm session.
> Nothing here is approved, scoped, or scheduled for implementation.
> Do not implement any of this until it is formally planned and milestoned.
> Other chats: treat this as read-only background context, not a work order.

---

## Scope (rough idea)

A built-in `db` module supporting MySQL and Postgres to start. Additional drivers (SQLite, etc.) deferred to a later milestone.

---

## The Core Problem: GC Pressure from Conversion Layers

Traditional ORMs have multiple layers of allocation per query result:

1. Build the query object (heap allocation)
2. Receive raw wire bytes → convert to generic intermediate objects
3. "Restructure" — map intermediate objects onto model instances (second round of allocations)

On millions of rows this means millions of short-lived heap objects. Even if the SQL runs fast, the restructuring step creates massive GC pressure. Real-world example: a 320-million-row query where the SQL ran fine but restructuring took ~30 seconds.

Yinz has no GC. The goal is to also eliminate the intermediate allocation layers entirely.

---

## Fix 1: Direct Wire-to-Struct Deserialization

Since the compiler knows the target type at compile time, it generates a direct deserializer — one pass from wire bytes to typed Yinz struct. No intermediate map, no generic object, no two-step conversion.

The compiler generates the exact byte-reading code for a given struct shape once, at compile time. Runtime cost is a straight memory write per field.

---

## Fix 2: Binary Wire Protocol by Default

Both Postgres and MySQL support binary wire protocols (not just text). The text protocol sends everything as strings which must be parsed at runtime. The binary protocol sends typed bytes directly.

Binary mode by default eliminates all string-parsing overhead for numbers, booleans, dates, and other primitive types.

---

## Fix 3: Iterator Model by Default (Flat Memory)

`db.query()` returns an iterator — rows are processed as they arrive off the wire. At any moment, only a small fixed-size buffer of rows is in memory regardless of total result count.

`.collect()` exists as an explicit opt-in for cases where all rows are genuinely needed in memory simultaneously. It is not the default.

### Small Ring Buffer, Not Strict 1-at-a-Time

The iterator uses a small fixed-size ring buffer (exact size TBD — likely 64–512 rows). This keeps CPU cache lines warm while still bounding memory to a constant regardless of result size.

---

## Fix 4: Compile-Time Query Strings

Parameterized queries validated and constructed at compile time. No runtime query-AST allocation, no string-building overhead per call. The SQL becomes a compile-time constant handed to the driver.

---

## Parallelization

The iterator model composes naturally with Yinz's auto-parallelization:

- One thread streams rows off the wire into the ring buffer
- N worker threads pull from that buffer and process rows in parallel
- CPU count is the ceiling, not row count

**Caveat**: the bottleneck is usually the database and network, not the CPU. Parallel row processing helps most when per-row computation is expensive. For simple read-and-write-through workloads, wire speed is the ceiling regardless of CPU parallelism.

---

## INSERT Performance: The Bulk Seeding Problem

GC pressure on bulk inserts (seeding millions of rows from an external API) is just as bad as on SELECT. The problem: intermediate model-instance objects created per row during INSERT aren't freed until GC runs. With multiple worker threads inserting in parallel, dead objects pile up faster than GC can collect them, causing main-thread GC pauses that stall everything.

### What Yinz does instead

**Ownership frees memory immediately.** When a batch finishes, its memory is freed at that exact instruction. No GC backlog, no pauses, no main thread involvement. Next batch starts clean.

**No intermediate model layer.** The compiler generates a direct serializer for each type at compile time. Your struct goes straight to wire bytes — no model-instance wrapper.

**Result**: same batch sizes, same thread count — memory stays flat because each batch is deterministically freed the moment it's done.

### The Postgres COPY protocol (worth exploring)

Postgres has a `COPY` command for bulk inserts that bypasses row-by-row INSERT overhead entirely — streams raw data directly into a table at wire speed. A Yinz `db` module could use COPY automatically for batches above a certain size. Speculative — research at design time.

---

## How Byte Translation Works

The Postgres and MySQL wire protocols are public specifications defining exactly how every type is encoded:

- `number` → 8-byte IEEE 754 float
- `string` → 4-byte length prefix + UTF-8 bytes
- `Date` → 8-byte microseconds since epoch
- `boolean` → 1 byte

The Yinz compiler authors implement these translations once per supported database. Wire protocols don't change, so these translations never change. Given a user-defined type, the compiler generates a serializer and deserializer for that exact shape — one pass, no runtime lookups, no branching.

---

## CRUD Convenience Layer

The goal is ergonomic CRUD with none of the allocation overhead. These are not in conflict — the convenience layer is a compiler problem, not a runtime problem.

```
const quote: Quote = { symbol: "AAPL", price: 189.50, timestamp: date.now() }
quote.save()
quote.destroy()

let found = db.find(Quote, { symbol: "AAPL" })
```

The compiler generates `save()` and `destroy()` specifically for `Quote` at compile time — they compile to direct wire bytes for that type's INSERT/DELETE. No generic model instance layer underneath.

**Open question**: change tracking (knowing which fields changed before `.save()`). Sequelize tracks this via dirty-field state on the model instance. Yinz's ownership model may offer a cleaner approach. Defer to design time.

---

## Query Expressiveness

### Known gaps to close (things Sequelize can't do cleanly)

- `NOT EXISTS` / `EXISTS` — no clean structured representation in Sequelize, forces raw SQL
- Window functions — not supported
- CTEs (`WITH` clauses) — not supported
- Lateral joins — not supported
- Conditional aggregates — awkward
- Subqueries in `WHERE` / `FROM` — limited support

The goal is to push the structured layer far enough that raw SQL is rarely needed. If it's a standard SQL pattern that appears in real production queries, the structured layer should cover it.

### Two-layer API

**Structured layer** — type-safe, compiler-validated. Should cover at minimum:

- Filters, joins, group by, order by
- Aggregates: `count`, `sum`, `avg`, `min`, `max`
- Subqueries in `WHERE` and `FROM`
- `NOT EXISTS` / `EXISTS`
- `HAVING`, `DISTINCT`, `LIMIT`, `OFFSET`
- Basic window functions

**Raw escape hatch** — for what the structured layer can't express. User writes SQL directly but still gets typed results back. Type safety on the output is never sacrificed.

### API shape

No dot-chain notation. The Sequelize style (structured object/options) is a good starting point — worth evaluating whether we can improve on it, but don't start from scratch for the sake of it. Autocomplete works without dot notation because the stdlib defines the known types.

### Compiler as query advisor (Rule 11)

The compiler knows the schema — types, indexes, constraints — because those are defined in the same Yinz models used for migrations. It should use this to:

- Warn when a query pattern would do a full table scan on an unindexed column
- Suggest `NOT EXISTS` over `LEFT JOIN ... WHERE NULL` when the optimizer would prefer it
- Flag missing indexes for common query patterns it sees in the codebase
- Suggest query rewrites with reasoning attached (not just "this is faster" but "this avoids a seq scan on `quotes.symbol` — add an index or rewrite as NOT EXISTS")

This is the teaching-compiler principle applied to database access. The compiler mentors the developer toward efficient queries the same way it mentors them toward correct code.

---

## Security

### SQL injection is structurally impossible in the structured layer

When using the structured query API, the user never writes SQL. The compiler generates it. There is no surface for injection — parameters are always passed separately as typed values, never interpolated into a string. This is the goal: security by construction, not by convention.

### The raw escape hatch is the only real injection surface

When a user drops to `db.raw()`, they're writing SQL directly. This is where injection becomes possible. The compiler should:

- Refuse to compile `db.raw()` calls where a variable is directly interpolated into the SQL string
- Require parameters to be passed as separate typed arguments, never string-concatenated in
- Warn loudly at the call site when it detects unsafe patterns

```
// compiler blocks this
let sql = "SELECT * FROM quotes WHERE symbol = " + userInput
db.raw(sql)

// required pattern — parameter passed separately, never interpolated
db.raw("SELECT * FROM quotes WHERE symbol = ?", userInput)
```

### One parameterization mechanism, always safe

Sequelize has two ways to pass parameters — `replacements` (string interpolation under the hood, can be unsafe) and `bind` (true parameterized queries, always safe). Having two options means developers pick the wrong one.

Yinz has one way. Parameters are always passed as separate typed values and sent to the database as bound parameters via the wire protocol — never interpolated into the SQL string at any layer. There is no unsafe option to accidentally choose.

### TLS by default

Database connections should use TLS by default. Plaintext should require an explicit opt-in with a hard-to-miss config flag. Accidentally shipping a plaintext DB connection to production because TLS was the opt-in is a real failure mode.

### Least privilege (documentation / convention)

The db module can't enforce what DB user you connect as, but the docs and compiler warnings should steer toward least privilege — separate read and write credentials, no app-level connections as superuser. Worth calling out explicitly in the stdlib docs when those are written.

---

## Things Sequelize Misses (Yinz Should Do Better)

### N+1 detection

Sequelize never warns when you call `db.find()` inside a loop — the classic N+1 problem. The compiler sees the call site, knows you're in a loop, and can flag it with a suggestion to use a JOIN or batch fetch instead. This is especially valuable for junior devs who haven't learned what N+1 is yet.

### Cursor-based pagination as a first-class primitive

Offset pagination (`LIMIT x OFFSET y`) makes the database scan and discard all rows up to the offset. On large tables this gets slow fast. Cursor-based pagination (`WHERE id > last_seen_id`) doesn't have this problem — cost is constant regardless of page depth.

Yinz should have a first-class cursor pagination primitive. `LIMIT/OFFSET` can exist as an escape hatch but shouldn't be the default pattern the API steers you toward.

### Transaction scoping via ownership

Sequelize requires manually passing the transaction object into every call (`{ transaction: t }`). Forget it once and you silently execute outside the transaction — no error, no warning. Yinz's ownership model may be able to scope the transaction at the language level so forgetting is a compile error. Open question — needs design work.

### Schema drift detection

Sequelize does nothing to validate the DB schema matches the code. You find out at runtime when a query fails because a column was renamed or a table was dropped. Yinz should validate schema-vs-code alignment at compile time or startup and fail fast with a clear error. The compiler already knows the types — this is a natural extension.

### Default query timeout (required, not optional)

Sequelize has no default query timeout. A hung query holds a connection indefinitely. Yinz's db module should require a timeout to be configured — infinity should not be the silent default. If a user wants no timeout they should have to opt in explicitly.

### Deadlock auto-retry for transactions

Deadlocks under high concurrency are expected behavior, not errors. Sequelize surfaces them as exceptions and leaves retry logic to the user. Yinz's transaction primitive should retry automatically on deadlock with configurable backoff strategy.

---

## Connection Pool and Retry Configuration

These need to be first-class configurable options, not afterthoughts. Rough list of what should be configurable:

**Pool config**
- Min / max pool size
- Connection timeout (how long to wait for a connection from the pool)
- Idle timeout (how long before an unused connection is closed)
- Connection health check / keepalive interval

**Retry config**
- Max retry attempts on connection failure
- Backoff strategy (exponential recommended as default)
- Which errors are retryable vs fatal

**Query config**
- Default query timeout (required — no infinity default)
- Statement timeout (separate from query timeout, Postgres-specific)

**Transaction config**
- Deadlock retry attempts
- Deadlock retry backoff
- Isolation level

Exact API shape TBD at design time. These should all have sensible defaults so a basic setup is 2-3 lines, with full control available when needed.

---

## Rust Ecosystem Inspiration

### SQLx — compile-time query verification (via schema snapshot)

SQLx verifies SQL queries against the real schema at compile time. The practical problem: requiring a live DB connection during every build is painful for CI/CD.

Yinz approach: after each migration runs, the tooling generates a **schema snapshot file** committed to the repo. The compiler validates all queries against the snapshot — no live connection needed at build time. The snapshot is always accurate because it's regenerated automatically on every migration run.

### SQLC — schema-first, no separate model files

Migrations define the schema. The compiler derives types from the migration history automatically. There is no separate model file to write or maintain. You import the generated type — you never define it manually.

This eliminates an entire class of bugs: a developer runs a migration and forgets to update the model. In Yinz this is impossible because the model IS the migration output. One source of truth, zero drift.

### Diesel — queries that can't be valid don't compile

With schema-first types, the compiler knows the full schema at compile time. Querying a column that doesn't exist is a compile error. Type mismatch between a query result and a struct field is a compile error. The error messages follow Rule 11 — WHAT went wrong, WHAT to do instead, WHY — not Diesel's wall of unreadable trait bounds.

---

## Migrations

### Atomic migrations via transactional DDL

Each migration runs inside a single database transaction. If anything fails, the whole migration rolls back — the DB is left exactly as it was before the migration started. No partial state, no manual cleanup.

Postgres supports transactional DDL natively. MySQL does not for DDL statements — this is a known limitation worth documenting when MySQL support is designed.

### Pre-flight schema validation

Before running a migration, validate that the current DB state matches what the previous migrations expect. This catches anyone who manually altered the DB outside of migrations — added a column, dropped an index, renamed a table — before any migration logic runs.

Flow:
1. Assert current DB state matches expected state (from snapshot)
2. If mismatch → stop with a clear error, touch nothing
3. If match → run migration inside a transaction
4. On success → update schema snapshot

This prevents the "half-ran migration" failure mode entirely. You either never start (failed pre-flight) or fully complete (transactional DDL).

### Migration metadata

Sequelize tracks migrations by filename in a meta table. This is fragile — rename a file and Sequelize thinks it's a new migration. Yinz should track migrations by a content hash or a stable ID defined inside the migration itself, not the filename. Exact mechanism TBD at design time.

### Idempotent seeds

Seeds should be safe to run multiple times without creating duplicates or errors. The db module should provide primitives that make idempotent seeding the easy path — upsert-by-default for seed operations, not plain insert.

---

## Structured Runtime Errors

Sequelize throws untyped exceptions with string error messages that must be caught and parsed manually. Every db call wrapped in try/catch, string matching on `.message` to figure out what went wrong — fragile and exhausting.

Yinz uses the `errors` keyword to make database errors typed at the call site:

```
function findUser(id: number) -> User errors DatabaseError
```

`DatabaseError` is a first-class type in the stdlib:

```
shape DatabaseError {
    summary: string             // short human-readable: "Unique constraint violated on quotes.symbol"
    suggestions: array<string>  // ["Check for duplicates before inserting", "Use upsert instead"]
    code: string | null         // raw DB error code ("23505", "08006", etc.)
    query: string | null        // sanitized query that triggered the error
    detail: string | null       // full DB-level detail message
}
```

Postgres error codes are a public specification — every 5-character code is documented. The stdlib maps all of them to human-readable summaries and suggestions. Examples:

- `23505` (unique_violation) → "Unique constraint violated" + suggest upsert
- `23503` (foreign_key_violation) → "Referenced record does not exist" + suggest checking parent record first
- `08006` (connection_failure) → "Lost connection to database" + suggest checking pool config
- `40P01` (deadlock_detected) → "Transaction deadlock" + suggest retry or reorder operations

The caller gets a typed, structured error they can pattern match on — no string parsing, no try/catch, no guessing what went wrong.

**Compile-time vs runtime errors are different mechanisms but both structured.** Compile-time errors are the compiler's domain — location, message, suggestion, already formatted by the teaching-compiler. Runtime errors are the `DatabaseError` type — handled by the `errors` keyword, propagated up the call stack as a typed value. Same philosophy (WHAT/WHAT-INSTEAD/WHY), different layer.

---

## What This Does NOT Cover Yet

- Exact query builder API / syntax
- Migrations
- Transactions
- Connection pooling
- Schema inference / compile-time schema validation
- Additional drivers beyond MySQL and Postgres
