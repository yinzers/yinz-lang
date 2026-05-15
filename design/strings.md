# Strings — Internal Encoding and Implementation

User spec: `spec/strings.md` (indexing API, methods).

This file locks the internal-encoding decision and SIMD implementation strategy for the built-in `string` type. The user-facing API (`.byteAt()`, `.get()` by code point, `.graphemeAt()`, `.count()`/`.byteCount()`/`.graphemeCount()`) is documented in `design/collections.md` "String Methods" section and `spec/strings.md`. This doc covers what's UNDERNEATH that API.

---

## Internal Encoding: UTF-8, Always

**Decision**: Yinz strings are UTF-8 internally. Always. No alternate internal encoding, no platform-default fallback, no UTF-16 mode for "Windows compatibility."

**Why**: Java picked UTF-16 in 1996. Compact Strings (Java 9, 2017) added a `byte[]` optimization for ASCII-only strings — but the public `char` API still returns 16-bit values, so the optimization had to be hidden behind a runtime branch. 21 years of 2× memory overhead for ASCII workloads, and the fix added complexity rather than removing it. Swift made the same mistake (NSString was UTF-16) and switched to UTF-8 in Swift 5 (March 2019). Rust shipped with UTF-8 from 1.0. Python 3.3+ uses the smallest encoding that fits the string content.

The 2026 consensus is unambiguous: UTF-8 is the right internal encoding. Yinz adopts it from day 1 and locks it.

**Cost**: code-point indexing (`s.get(n)`) is O(n) over UTF-8 storage — you have to scan from the start to find the n-th code point. Yinz's API design accepts this tradeoff: `.byteAt(n)` is O(1) for parsing protocols where bytes are what you actually want, `.graphemeAt(n)` is O(n) for human-character access. The default (`s.get(n)`) is O(n) but matches user intuition of "n-th character." Most code in practice iterates strings linearly (`for c in s`) which is O(n) total — the same as UTF-16 indexing would be in real terms.

**Memory**: ASCII strings take 1 byte per character. Java's UTF-16 takes 2. For string-heavy workloads (web servers, JSON processing) this is meaningful — Compact Strings benchmarks showed 5-10% throughput improvement from reduced GC pressure when strings dropped to 1 byte. Yinz starts there.

---

## File I/O Encoding Default: UTF-8

When the standard library reads a text file (v0.6 `file.read(path)`, etc.), the default encoding is **UTF-8 unconditionally**. Not `locale.getpreferredencoding()`. Not Windows ANSI codepage. UTF-8.

Python made the OPPOSITE choice in Python 3.0 (2008) — `open()` used the OS-preferred encoding, which is cp1252 on Windows. Result: code that worked on Linux/macOS silently broke on Windows when it hit non-ASCII data. PEP 686 to fix this was proposed in 2022 and is targeting Python 3.15 — 17+ years of cross-platform encoding bugs. Java made the same mistake until Java 18 (JEP 400, March 2022) finally defaulted JVM file I/O to UTF-8.

For users who genuinely need a non-UTF-8 encoding (legacy data files, specific protocols), the API takes an explicit parameter: `file.read(path, encoding: .latin1)`. The DEFAULT path is UTF-8 because that's the modern correct answer.

This decision belongs to the v0.6 file system milestone but is locked here NOW so the v0.6 implementer doesn't reach for `locale.getpreferredencoding()` out of habit.

---

## SIMD-Accelerated UTF-8 Validation and Traversal

**Implementation target**: built-in string operations that touch every byte (validation, decoding, search) use SIMD intrinsics where the target architecture supports them. Naive scalar UTF-8 validation runs at ~8 cycles/byte. SIMD validation (per Daniel Lemire's published technique, used by simdjson and simdutf) runs at ~0.7 cycles/byte — over 10× faster, often 80× faster on ASCII-heavy input.

This is a **design goal locked now**, not a v0.1 milestone scope commitment. `design/mvp-scope.md` v0.1 covers hello-world programs and basic computation — a polished SIMD UTF-8 runtime is not in v0.1's required scope. But the v0.1 string runtime SHOULD be written against this goal from the start so it doesn't need a rewrite when SIMD lands. The actual SIMD implementation can land in v0.1 (if there's appetite to do it alongside the basic compiler) or as a v0.2 polish item — but locking the target NOW prevents the v0.1 implementer from making layout choices that block SIMD later. The cost of NOT writing against this target compounds: every JSON parse, every HTTP header decode, every string search, every regex pre-pass touches every byte.

**Reference implementations to mirror or bind to**:
- `simdutf` (https://github.com/simdutf/simdutf) — BSD-licensed, mature, used by Node.js (which got a 364% benchmark improvement in UTF-8 decode after adoption)
- Daniel Lemire's "Validating UTF-8 strings using as little as 0.7 cycles per byte" (https://lemire.me/blog/2018/05/16/validating-utf-8-strings-using-as-little-as-0-7-cycles-per-byte/) — the foundational paper

**Fallback path**: when targeting an architecture without the relevant SIMD intrinsics (e.g., a stripped-down embedded build), the compiler emits the scalar implementation. Same correctness, slower.

---

## String Search

`.contains(substr)`, `.indexOf(substr)`, `.find(pattern)` use SIMD-accelerated search algorithms when the pattern is long enough to amortize the SIMD setup cost. For very short patterns (1-3 bytes), the naive byte scan is faster — the implementation picks the right algorithm per call site.

Specific algorithm choice (Boyer-Moore-Horspool vs SSE4.2 PCMPESTRI vs Two-Way) is an implementation detail not locked here. The PERFORMANCE TARGET is locked: `string.contains` should hit 1+ GB/s on patterns ≥ 16 bytes.

---

## What This Doc Does NOT Cover

- **The user-facing API** (methods like `.split()`, `.toUpperCase()`, etc.) — that's `spec/strings.md` and the v0.6+ stdlib expansion.
- **Indexing semantics** (code-point vs byte vs grapheme) — covered in `design/collections.md` "String Methods" section.
- **Locale-sensitive operations** (case conversion in Turkish locale, etc.) — open question for the stdlib design (see `design/stdlib/strings.md` when written). Brief preview: locale-sensitive operations MUST take an explicit locale parameter; no system-locale-default for security-critical comparisons.
- **Internationalization library** (Unicode normalization NFC/NFD/NFKC/NFKD, locale-aware sort, etc.) — likely v0.20+ stdlib expansion.

---

## Cross-References

- `design/collections.md` "String Methods" section (user-facing API)
- `spec/strings.md` (user spec for string operations)
- `design/golden-rules.md` Rule 4 (compiler does the hard work — SIMD selection)
- `design/golden-rules.md` Rule 10 (efficiency first — UTF-8 is the efficient default for ASCII)
- `lockin-cpu-bigo.md` Finding #16 (UTF-8 SIMD validation perf gap research)
- `lockin-type-and-memory.md` Finding #3 (Java UTF-16 21-year cost)
- `lockin-build-and-crossplat.md` Finding #8 (Python `open()` encoding mistake)
