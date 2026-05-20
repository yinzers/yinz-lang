# Stdlib Design Rules

Rules every Yinz stdlib module must follow. Distinct from `.claude/rules/language-design.md` (which covers language features) — this file covers stdlib API contracts.

Loaded when designing or reviewing any new stdlib module (v0.6 file system, v0.7 math, v0.8 cli/env/process, v0.9 json, v0.10 date/duration, v0.11 db (DuckDB + Postgres only), v0.12 log, v0.13 random, v0.14 testing, v0.15 regex, v0.16 http, etc.).

---

## Rule 1: Pure-Named Methods MUST Be Pure

**The rule**: any stdlib method whose name implies a pure read MUST be pure. No silent I/O. No silent allocation that observably changes state. No silent network calls. If a method needs side effects, the name must say so.

**What "pure-named" means**: methods that to a reasonable reader sound like they only inspect a value and return a result. Examples:
- `.equals(other)` — compares two values
- `.length()`, `.count()` — returns a size
- `.contains(item)` — membership test
- `.startsWith(prefix)` / `.endsWith(suffix)` — prefix/suffix test
- `.matches(pattern)` — pattern test
- `.toString()` — formats as string
- Property accessors (`.name`, `.age`, etc.) — field reads

**What "side effects" means in this rule**:
- Network I/O (DNS, HTTP, anything off-machine)
- File I/O (reads or writes to disk)
- Process spawning
- Lock acquisition that could block
- Allocation that's observable to other threads
- Time-dependent behavior (`time.now()` style calls)

### The case study: Java `URL.equals()`

`java.net.URL.equals()` does a DNS lookup. Marked "Will Not Fix" since 2009 (JDK-6810437). FindBugs found 29 hot-path call sites in Eclipse 3.2.1 alone where URLs were used as map keys or in sets — every comparison silently making network calls. The bug class is impossible to spot from the calling code; you have to know `URL.equals` is special.

This rule exists specifically to prevent any Yinz stdlib API from shipping with this characteristic.

### What to name a method that DOES need I/O

If a method genuinely needs a side effect to do its job, the name must communicate that:

- ❌ `url.equals(other)` doing DNS → ✅ `url.equalsResolved(other) errors` (the `errors` keyword surfaces the failure mode + tells the caller this can fail/block)
- ❌ `file.length()` doing disk I/O → ✅ `file.lengthOnDisk() errors`
- ❌ `set.contains(item)` doing remote lookup → ✅ `set.containsRemote(item) errors`

The pattern: **action verb or qualifier in the name** (`Resolved`, `OnDisk`, `Remote`, `Live`, `FromCache`, etc.) makes the side effect visible at every call site without requiring the reader to know the method is special.

### Compile-time enforcement (where possible)

For any stdlib method marked `errors`, the compiler already requires the caller to handle or propagate. That's good — it surfaces the failure mode. But "no I/O without `errors`" isn't currently a compiler rule — it's a stdlib design discipline.

When stdlib modules are reviewed before each version's release, this rule is checked against the API surface: any pure-named method that calls into I/O is rejected before merge.

### Corollary: Output-named operations must not silently do MORE than output

The inverse of the pure-named rule: an operation whose name implies simple output (write a string, end a line) must not silently perform additional I/O. C++'s `std::endl` combines newline + buffer flush in a name that sounds like it only ends a line. clang-tidy ships `performance-avoid-endl` specifically because this costs orders of magnitude in performance on buffered output.

In Yinz: `terminal.print()` writes without flushing. If you want to flush, you call `terminal.flush()`. The two operations have two names. No output method flushes silently.

---

## Rule 2: One API per Capability — No Parallel APIs

**The rule**: each stdlib capability ships with one canonical API. No `v2` alongside `v1`. No "old way" and "new way" coexisting permanently.

**Why this rule exists**: Java NIO vs IO (24+ years and counting), Python `os.path` vs `pathlib` (10+ years and counting), Go `sort.Sort` vs `sort.Slice` vs `slices.Sort` (3 generations all live), Node.js callbacks vs promises (8-year migration), Python `%`/`.format()`/f-strings (3 string formatting systems). Every one of these started as "we'll deprecate the old one once the new one matures" — and never did. The "old" API never goes away because too much code depends on it.

`design/versioning.md` already establishes the macro policy: pre-v1.0, breaking changes are fine; post-v1.0, strict major-version-bump compatibility. THIS rule operationalizes that for stdlib API design: never ship a "v2 alongside v1" — fix the v1 in place pre-1.0, or wait until you can do a major version bump.

### What to do when you want to redesign a stdlib API

- **Pre-v1.0**: just change it. Update the docs, update any depending code, ship the new shape. Yinz's pre-1.0 policy explicitly permits this.
- **Post-v1.0**: live with the old shape until a major version bump justifies the break. Don't ship a parallel "improved" API alongside.
- **Adding new capabilities to an existing API**: extension is fine (new methods, new optional parameters). What's banned is shipping a NEW NAME for the SAME concept.

### The escape valve: separate concerns get separate APIs

If two genuinely-different concerns share a name in another language but should be split, that's not "parallel APIs" — that's "fixing a mistaken naming decision." Example: Java's `java.io` covers BOTH stream-oriented blocking I/O AND text formatting. Yinz can split these into separate modules without violating this rule because they're separate concerns that shouldn't have shared a namespace.

---

## Rule 3: No Silent Configuration Defaults That Vary By Platform

**The rule**: stdlib defaults are the same on every platform. No `locale.getpreferredencoding()`-style platform-dependent defaults that change behavior between Linux/macOS/Windows.

**Why this rule exists**: Python 3 `open()` defaulted to the OS-preferred encoding (cp1252 on Windows, UTF-8 on Linux). Code worked on macOS, broke on Windows when it hit non-ASCII data. PEP 686 to fix this was proposed in 2022 and is targeting Python 3.15 — 17+ years of cross-platform encoding bugs. Java had the same problem; JEP 400 (Java 18, 2022) finally defaulted JVM file I/O to UTF-8.

If Yinz wants to support a non-default behavior (different encoding, different time zone, different locale), the API takes an EXPLICIT parameter. The default is the same everywhere.

This applies to:
- Text encoding (always UTF-8 — see `design/strings.md`)
- Time zone for date construction (always explicit — no "default JVM tz")
- Locale for case conversion (always explicit — no "system locale `tolower`" Turkish-bug class)
- Path separator (Yinz uses logical paths, separator is internal)
- Number formatting (always invariant locale unless explicit)

---

## Rule 4: Bounded by Default — Unbounded Requires Explicit Opt-In

**The rule**: stdlib types that hold collections of unknown future size (queues, channels, mailboxes, retry buffers) are bounded by construction. Unbounded behavior requires explicit opt-in with a name that surfaces the danger.

**Why this rule exists**: Erlang mailboxes (unbounded by design) → cascading node failures in production telecom systems for 30+ years (mitigated only in OTP 19 with `max_heap_size`, still reactive not preventive). Rust's `tokio::mpsc::unbounded_channel` exists as a permanent footgun. Node.js streams pre-streams2 had unbounded buffering by default. The pattern: unbounded queues hide backpressure, producer outpaces consumer, memory exhausts, OOM kill.

For Yinz's eventual channel/queue stdlib (v0.2 concurrency primitives or whenever it lands):
- `channel<T>(capacity: int)` — bounded, the only constructor.
- For "I really want unbounded": `channel<T>(capacity: int.max)` with a comment explaining why.
- No `unboundedChannel()`, no convenience constructor that hides the cost.

---

## Rule 5: Argument Order — Receiver First (already enforced by syntax)

Yinz's dot-method-first design (`value.method(args)`) puts the thing-being-operated-on first by syntax convention. This eliminates PHP's class of bug (`strpos($haystack, $needle)` vs `in_array($needle, $haystack)` — opposite conventions across the same stdlib).

For the rare case where a stdlib FREE function takes two arguments of the same type (e.g., a pure utility that doesn't bind to a value), the convention is **what you'd say in English first comes first**. `string.compare(a, b)` reads "compare a to b." Don't invent reverse-order signatures.

---

## Rule 6: Serialization Uses Compile-Time Codegen, Not Runtime Reflection

**The rule**: stdlib serialization (JSON, future formats) uses compiler-generated specialized code per `shape` declaration. No runtime reflection.

**Why this rule exists**: Go's `encoding/json` uses runtime reflection. Issue #5683 (filed 2013, closed FrozenDueToAge — Go's compatibility guarantee prevents the fix) tracks the perf gap. `easyjson` (codegen-based) is 4-5× faster per its own README; in some hot paths, reflection consumes ~50% of CPU time in Go web services. The decision to ship reflection-based serialization in stdlib is permanent in Go because the API can't change without breaking semver.

When Yinz designs the JSON module (v0.9), the marshal/unmarshal API uses compiler-generated specialized serializers per `shape`. The compiler emits a typed serializer at the time the `shape` is declared (or at first serialization use). Same rule applies to any future serialization formats (CSV v0.21, msgpack/cbor if added).

This rule should be cross-referenced into `design/stdlib/data.md` (when written) and the v0.9 milestone plan.

---

---

## Rule 7: Regex Engine Is Linear-Time NFA Only — No PCRE Backtracking

**The rule**: Yinz's stdlib regex (when designed in v0.15 per `design/mvp-scope.md`) MUST be a linear-time NFA-based engine (RE2-style). Backtracking engines (PCRE, Python `re`, Ruby `=~`, PHP) are explicitly rejected. No backreferences, no lookahead/lookbehind, no possessive quantifiers in stdlib regex.

**Why this rule exists**: backtracking regex engines have exponential worst-case complexity. A pattern like `(a+)+$` against a long string of `a`s followed by a non-matching character explores O(2^n) states. Cloudflare's July 2, 2019 global outage was caused by exactly this — a WAF regex with catastrophic backtracking exhausted CPU globally for 27 minutes (per Cloudflare's own post-mortem at https://blog.cloudflare.com/details-of-the-cloudflare-outage-on-july-2-2019/). Cloudflare subsequently switched their WAF from PCRE to RE2-inspired.

Python `re` and Ruby `=~` are still backtracking engines. Production services that pass user-controlled input to them are exposed to ReDoS attacks. Yinz refuses to ship that vulnerability class.

**For users who genuinely need backreferences (rare — usually a parsing job)**: those use cases should use a parser combinator library (when one ships in stdlib or as a third-party package), not regex. Regex is for pattern matching; full grammar parsing is a parser's job.

**Implementation**: when v0.15 designs the regex module, the locked target is RE2 or a Yinz-native NFA implementation matching RE2's semantics (linear-time guarantee, no backtracking-required features). Lock this as the v0.15 design's first principle.

This rule should be cross-referenced into a future `design/stdlib/regex.md` when written. Until then, it lives here to prevent the v0.15 designer from defaulting to PCRE-style features out of habit.

---

## Rule 8: SIMD-Accelerated Stdlib for Byte-Touching Operations

**The rule**: stdlib operations that touch every byte of input (UTF-8 validation, JSON parsing, regex pre-pass, base64 encode/decode, string search on long patterns) MUST use SIMD intrinsics on architectures that support them. Scalar fallback is provided for unsupported targets, but the SIMD path is the default on x86_64 and ARM64.

**Why this rule exists**: scalar UTF-8 validation runs at ~8 cycles/byte; SIMD-accelerated runs at ~0.7 cycles/byte (per Daniel Lemire's published technique used by simdutf/simdjson). Scalar JSON parsing runs at 50-200 MB/s; simdjson runs at 2-3 GB/s — a 20-40× gap on the same hardware. Node.js adopted simdutf and saw a 364% benchmark improvement on UTF-8 decode workloads.

The cost of scalar implementations compounds: every JSON parse, every HTTP header decode, every string search, every regex pre-pass touches every byte. Shipping scalar stdlib code in 2026 is leaving 10-40× performance on the table for a one-time engineering cost (writing or binding the SIMD implementation per architecture).

**Specific stdlib touchpoints**:
- `string.contains(substr)`, `string.indexOf(substr)`, `string.find(pattern)` — SIMD-accelerated for patterns ≥ 16 bytes
- UTF-8 validation on file read, network read, string construction from bytes — always SIMD where available (simdutf or equivalent)
- JSON parsing (v0.9 module) — adopt simdjson directly (BSD-licensed, mature) or write Yinz-native SIMD parser. Target: ≥ 1 GB/s on modern x86 with the default JSON workload.
- Base64 encode/decode (when shipped) — SIMD-accelerated
- Regex pre-pass (literal scanning before NFA simulation) — SIMD where the pattern includes literal substrings

**Reference implementations to bind to or mirror**:
- `simdutf` (https://github.com/simdutf/simdutf) — UTF-8 validation
- `simdjson` (https://github.com/simdjson/simdjson) — JSON parsing
- Daniel Lemire's "Validating UTF-8 strings using as little as 0.7 cycles per byte" (https://lemire.me/blog/2018/05/16/validating-utf-8-strings-using-as-little-as-0-7-cycles-per-byte/)

**Fallback path**: when targeting an architecture without the relevant SIMD intrinsics (some embedded targets, kernel-mode `--kernel` builds), the compiler emits the scalar implementation. Same correctness, slower. The SIMD path is the documented default for production builds.

This rule should be cross-referenced into the v0.9 (`json`), v0.15 (`regex`), and any future bytes-touching stdlib module designs. The decisions about specific algorithms can defer to those milestones; the COMMITMENT to SIMD where available is locked here.

---

## Cross-References

- `.claude/rules/language-design.md` (covers LANGUAGE features; this file covers STDLIB APIs)
- `.claude/rules/vocabulary.md` (Yinz terminology — uses correct terms in error messages)
- `design/golden-rules.md` Rule 11 (compiler is a teacher — applies to stdlib diagnostics too)
- `design/golden-rules.md` Rule 12 (human-readable over jargon — stdlib method names too)
- `design/versioning.md` (no-backwards-compat-pre-v1.0; this rule is the operational corollary)
- `design/strings.md` (Rule 3 — UTF-8 default cited there)
- `design/stdlib/data.md` (where Rule 6 — codegen serialization — lands when JSON v0.9 is designed; currently a stub)
- `lockin-stdlib-and-syntax.md` Findings #5 (Java URL.equals), #14 (Go encoding/json), #30 (Java NIO/IO duality)
- `lockin-build-and-crossplat.md` Finding #8 (Python encoding default)
