---
name: Yinz Stdlib Design Protocol
description: "Applies when designing or reviewing a Yinz stdlib module or API: purity, one API per capability, platform defaults, bounded queues, argument order, serialization, regex and SIMD."
trigger: relevance
---

- **A method named like a plain read stays pure.** `equals`, `length`, `contains`, `startsWith`, `matches`, `toString` and field accessors do no network or file I/O, process spawn, blocking lock, thread-visible allocation or clock read. Why: hidden I/O in a comparison cannot be spotted from the call site (a URL `equals` that does a DNS lookup hits every map key and set).
- **A method that needs a side effect says so in its name and is `errors`:** `equalsResolved`, `lengthOnDisk`, `containsRemote` (qualifiers like `Resolved`, `OnDisk`, `Remote`, `Live`). Why: the cost and failure mode show at every call site.
- **An output-named operation does no extra I/O.** `terminal.print()` writes without flushing; `terminal.flush()` flushes. Why: a name that implies simple output must not silently pay for a flush.
- **One API per capability, with no `v2` alongside `v1` and no new name for the same concept.** Before 1.0, change the API in place; after 1.0, keep it until a major version. New methods and optional parameters are fine, and genuinely separate concerns may split into separate modules. Why: an old API never goes away once code depends on it.
- **Defaults are identical on every platform.** Text is always UTF-8, number formatting is locale-invariant, and paths are logical with the separator internal. Time zone and locale (for case conversion) are always explicit parameters. Why: platform-varying defaults break code only on the other operating system.
- **Types that hold a collection of unknown future size are always bounded** (queues, channels, mailboxes, retry buffers). Pass a capacity or take the locked default of 64 (the `channel_capacity` domain in `registry/features.toml`). Offer no unbounded constructor or convenience; a huge buffer is a large explicit capacity. Why: an unbounded queue hides backpressure until the process runs out of memory.
- **Put the receiver first.** Dot calls do this by syntax. For a free function with two arguments of the same type, the order reads as English: `String.compare(a, b)`. Why: reversed signatures across one stdlib are a bug class.
- **Serialization uses compiler-generated code per `shape`, never runtime reflection.** Why: reflection costs speed that cannot be removed once the API ships.
- **Regex is a linear-time NFA engine (RE2-style).** No backtracking engine, backreferences, lookaround or possessive quantifiers; anything needing those is a parser's job. Why: a backtracking engine has exponential worst cases, so user-controlled input becomes a denial-of-service.
- **Operations that touch every byte use SIMD on x86_64 and ARM64:** UTF-8 validation, JSON parsing (target at least 1 GB/s on modern x86), base64, long-pattern string search and the regex literal pre-pass. Targets without the intrinsics, such as `--kernel` builds, get a scalar fallback. Why: SIMD is 10x or more faster on the same hardware, and every parse and search pays that cost.
