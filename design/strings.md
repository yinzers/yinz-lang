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

When the standard library reads a text file (v0.5 `file.read(path)`, etc.), the default encoding is **UTF-8 unconditionally**. Not `locale.getpreferredencoding()`. Not Windows ANSI codepage. UTF-8.

Python made the OPPOSITE choice in Python 3.0 (2008) — `open()` used the OS-preferred encoding, which is cp1252 on Windows. Result: code that worked on Linux/macOS silently broke on Windows when it hit non-ASCII data. PEP 686 to fix this was proposed in 2022 and is targeting Python 3.15 — 17+ years of cross-platform encoding bugs. Java made the same mistake until Java 18 (JEP 400, March 2022) finally defaulted JVM file I/O to UTF-8.

For users who genuinely need a non-UTF-8 encoding (legacy data files, specific protocols), the API takes an explicit parameter: `file.read(path, encoding: .latin1)`. The DEFAULT path is UTF-8 because that's the modern correct answer.

This decision belongs to the v0.5 file system milestone but is locked here NOW so the v0.5 implementer doesn't reach for `locale.getpreferredencoding()` out of habit.

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

## Small String Optimization — 23-byte Inline Threshold

**This is NOT a maximum string length.** Yinz strings can be any length up to available memory (the type's length field is `int` / i64, so ~9.2 quintillion bytes — effectively unlimited). What the 23-byte threshold controls is **where the bytes physically live in memory**, transparently to user code.

| String length | Storage | Access cost |
|---|---|---|
| ≤ 23 bytes | Inline in the 24-byte string value itself (stack or struct) | 1 cache line, no pointer chase |
| > 23 bytes | Heap-allocated; the string value holds a pointer + length | One pointer chase to the heap |

From the user's perspective, both behave identically — `.count()`, `.byteAt()`, `.contains()`, every string operation works the same way. The compiler picks the storage layout based on length; the user never has to think about it.

**The threshold is fixed and documented — not implementation-defined.**

### Why 23 bytes, fixed

C++ `std::string` has Small String Optimization but the threshold is implementation-defined: 15 bytes on GCC libstdc++, 22 bytes on LLVM libc++. Code tuned for one threshold heap-allocates on the other (https://giodicanio.com/2023/04/26/cpp-small-string-optimization/). Worse, the threshold is part of the ABI — changing it later requires every dependent binary to be recompiled. No-SSO benchmarks show ~2× slowdown vs SSO for short-string-heavy workloads (29ms vs 14ms for 1M push_back ops on x64 i7 @3.40GHz, per https://sqlpey.com/c++/small-string-optimization-sso/).

23 bytes is chosen because:
- Fits the natural value-type size for a 24-byte string struct on 64-bit (one cache-line-eighth, fits in two registers)
- Covers the vast majority of real-world short strings: identifiers, labels, short keys, file extensions, short user-facing text
- Matches libc++ (22 bytes) and Rust's small string strategies — already industry-validated
- Locking now means future Yinz binaries have a stable string ABI

### Auto-promotion (codegen + muted hint)

Per `.claude/rules/auto-promotion.md`:
- **Codegen**: every short string literal (≤ 23 bytes) is stored inline. No heap allocation. Always applies.
- **Muted IDE hint**: when the compiler can statically prove a string variable will only ever hold values ≤ 23 bytes (e.g., a `shape` field initialized only from short literals, a const string), the IDE shows `// fits inline — no heap` confirming the perf decision.
- **Tier 3 lint suggestion**: not applicable — there's no source-level rewrite to suggest. The user wrote a string; the compiler picked the storage. No explicit form to "make explicit."

### Tradeoff

The 23-byte threshold is an ABI commitment. A future architecture with wider SIMD registers (e.g., AVX-512 routinely available, so a 64-byte inline could be defended) might benefit from a larger threshold — but that's an ABI break, not a swap. We accept the 23-byte ceiling as the right tradeoff for 2026 and forward-compatible with the next decade of typical workloads.

---

## Locale-Invariant Case Operations Default

`.toLowerCase()` and `.toUpperCase()` on Yinz strings are **locale-invariant** — Unicode case-folding algorithm, NOT locale-specific. There is NO "use the system locale" default for case operations in stdlib.

Locale-aware case conversion exists on a separate, explicitly-named pair of methods: `.toLowerCaseLocale(locale)` and `.toUpperCaseLocale(locale)`. The naming makes the distinction visible at every call site without requiring documentation lookup.

### Why locale-invariant default — Turkish-I

In Turkish locale (`tr_TR`), uppercase `I` lowercases to `ı` (dotless i) rather than `i`, and lowercase `i` uppercases to `İ` (dotted I) rather than `I`. ANY security check that normalizes case before comparison breaks silently in Turkish locale if it uses locale-sensitive case conversion.

Documented production failures:
- **OpenSSL 3.0**: cipher name matching broke in Turkish locale (https://developers.redhat.com/articles/2022/06/15/openssl-30-dealing-turkish-locale-bug)
- **Apache Spark**: SQL keyword parsing broke (SPARK-20156, https://issues.apache.org/jira/browse/SPARK-20156)
- **VS Code, .NET, PHP, JavaScript frameworks**: all had documented Turkish locale issues as of 2025
- **Phil Haack's analysis**: https://haacked.com/archive/2012/07/05/turkish-i-problem-and-why-you-should-care.aspx/

The pattern: a developer in en-US writes `inputName.toLowerCase() == "admin"` to allow case-insensitive role checks. Code works in dev. Customer in Turkey uses the system; their browser sends `İSTANBUL` for their location header; case-insensitive matching against `"istanbul"` fails because the locale-aware lowercase produces `i̇stanbul` (with combining mark). Authentication or data routing breaks in production for users in one locale only — extremely hard to reproduce, easy to attribute to "weird customer issue."

### Why an explicit locale-aware variant exists

Some legitimate i18n code DOES need locale-aware case conversion (displaying user-facing text in the user's locale). That use case is real but narrow — UI rendering, NOT security comparisons. The explicit `.toLowerCaseLocale(locale)` form serves that need and makes the locale dependency visible at the call site.

This extends `.claude/rules/stdlib-design.md` Rule 3 (no silent platform-dependent defaults) to string case operations. The pattern: defaults protect the security-sensitive case; the i18n display case is opt-in with a visible parameter.

### Cross-references

- `.claude/rules/stdlib-design.md` Rule 3 (no platform-default config)
- `lockin-build-and-crossplat.md` Finding #9 for the source data on Turkish-I production failures

---

---

## M7 SSO Layout — 24-Byte Struct (ABI Locked)

**Locked 2026-05-18 for M7.** This is ABI-locked — any change requires a semver major bump and a recompile of all dependent binaries.

A `YnzString` is exactly 24 bytes with 8-byte alignment. The tag byte lives at offset 23 and is the inline/heap discriminator.

| Byte offset | Inline form (byte 23 bit 7 = 1) | Heap form (byte 23 bit 7 = 0) |
|---|---|---|
| 0..7 (8 bytes) | data[0..7] | `ptr: *u8` (8-byte aligned) |
| 8..15 (8 bytes) | data[8..15] | `len: i64` |
| 16..22 (7 bytes) | data[16..22] | low 7 bytes of `cap: i64` |
| 23 (1 byte) | tag byte (see below) | high byte of `cap: i64`, top bit = 0 |

**Tag byte breakdown (inline form, offset 23):**
- Bit 7 (0x80): inline-discriminator flag — always 1 for inline.
- Bit 6 (0x40): `is_nfc_known` — 1 if string is known NFC-normalized.
- Bits 5..0 (0x1F mask): inline length, range 0..23.

**Heap form `is_nfc_known`:** stored in bit 1 of the `len` field. Lengths < 2^62 leave the top 2 bits free. Bit 0 reserved for future use. This avoids a separate header byte and keeps len arithmetic clean (mask before use).

**Cap budget (heap form):** cap top bit MUST be 0 to stay in 0x00..0x7F range distinguishable from inline tag (0x80..0xFF). Maximum heap capacity = 2^63-1 bytes ≈ 9.2 exabytes. Acceptable.

**Worked examples:**
- `"hi"`: inline, length 2, NFC-known. Bytes 0..1 = `h`, `i`; bytes 2..22 = zero-fill; byte 23 = `0x80 | 0x40 | 0x02 = 0xC2`.
- 30-char ASCII literal: heap. Bytes 0..7 = ptr; bytes 8..15 = `len = 30 | (1 << 1) = 32` (NFC-known); bytes 16..23 = `cap = 30`.

**Compile-time verification:** `crates/ynz-runtime/tests/string_layout.rs` asserts `mem::size_of::<YnzString>() == 24` AND `mem::align_of::<YnzString>() == 8`. Bit-pattern test constructs known inline + heap strings and asserts byte-by-byte expected patterns.

---

## M7 NFC-Known Propagation Table

For every string-producing operation, whether the result carries `is_nfc_known = true`:

| Operation | Result `is_nfc_known`? | Why |
|---|---|---|
| String literal (parser) | TRUE — compiler pre-normalizes at compile time | Lock the parser-side NFC pass |
| Backtick-interpolation result | TRUE only if every segment was NFC-known | One non-NFC segment poisons the result |
| `s1 + s2` | TRUE only if BOTH were NFC-known AND s2 doesn't start with a combining-class code point | `"e" + "́"` produces NFD even if both sides are individually NFC-normal. Conservative: force false when s2 first code point has combining class > 0. |
| `.substring(start, end)` | TRUE if source was NFC-known AND boundaries are code-point boundaries | Substring of NFC is NFC |
| `.trim()` | TRUE if source was NFC-known | Trim is byte-level on whitespace; preserves NFC |
| `.split(sep)` | TRUE for each piece if source was NFC-known | Split is byte-level; preserves NFC per piece |
| `.replace(old, new)` | TRUE only if source AND `new` were both NFC-known | Inserts `new` text |
| `.toUpperCase()` / `.toLowerCase()` | FALSE | Case folding produces NFD code points in many cases (Turkish-I, Greek, etc.) |
| `.toString()` on primitives | TRUE | ASCII only |
| `.toString()` on user shape | FALSE (conservative) | User-defined formatting may produce NFD |
| `string.fromBytes(bytes)` (v0.5+) | FALSE | Runtime byte input may not be normalized |
| Single code point from `s.get(n)` | FALSE | A single code point in isolation cannot be verified NFC without context |

**Fast path for `ynz_string_eq`:** if both strings have `is_nfc_known = true`, byte-compare directly. Slow path: normalize both via `unicode-normalization::nfc()` then byte-compare.

---

## M7 SIMD Crate Selection (Locked)

**`simdutf8` is the chosen UTF-8 validation crate.** Rust port of simdjson's UTF-8 validator. MIT/Apache-2.0 licensed. ~700 LOC, no transitive dependencies. Runtime CPU feature detection with scalar fallback baked in.

Pinned version: `simdutf8 = "=0.1.4"`.

Used for: literal validation at parse time, runtime byte-to-string construction, and as a building block for `.contains` long-pattern path (≥ 16 bytes).

**SIMD search for `.contains` / `.indexOf`** on patterns ≥ 16 bytes: uses `memchr` crate (SIMD-accelerated `memmem`-style scan). Pinned: `memchr = "=2.7.4"`. For patterns ≤ 15 bytes: scalar scan.

**NFC normalization crate:** `unicode-normalization = "=0.1.24"`. Used in `ynz_string_eq` slow path.

**Case-folding crate:** `unicase = "=2.7.0"`. Locale-invariant Unicode case-folding for `.toUpperCase()` / `.toLowerCase()`. `unicode-normalization` provides NFC/NFD only — NOT case-folding. `unicase` is small, well-maintained, MIT/Apache-2.0.

All four crates are pinned with `=` to prevent surprise upgrades during implementation.

---

## M7 SIMD Fallback CI Requirement

A second CI job is required alongside the normal x86_64 job. This job sets `RUSTFLAGS=-C target-feature=-sse4.1,-avx2` to disable SIMD intrinsics and exercise the scalar fallback path. Both jobs must pass. Cross-referenced in Risk table (SIMD portability row) and P4b acceptance criteria.

---

## What This Doc Does NOT Cover

- **The user-facing API** (methods like `.split()`, `.toUpperCase()`, etc.) — that's `spec/strings.md` and the v0.5+ stdlib expansion.
- **Indexing semantics** (code-point vs byte vs grapheme) — covered in `design/collections.md` "String Methods" section.
- **Locale-sensitive operations** (case conversion in Turkish locale, etc.) — open question for the stdlib design (see `design/stdlib/strings.md` when written). Brief preview: locale-sensitive operations MUST take an explicit locale parameter; no system-locale-default for security-critical comparisons.
- **Internationalization library** (Unicode normalization NFC/NFD/NFKC/NFKD, locale-aware sort, etc.) — likely v0.19+ stdlib expansion.

---

## Cross-References

- `design/collections.md` "String Methods" section (user-facing API)
- `spec/strings.md` (user spec for string operations)
- `design/golden-rules.md` Rule 4 (compiler does the hard work — SIMD selection)
- `design/golden-rules.md` Rule 10 (efficiency first — UTF-8 is the efficient default for ASCII)
- `lockin-cpu-bigo.md` Finding #16 (UTF-8 SIMD validation perf gap research)
- `lockin-type-and-memory.md` Finding #3 (Java UTF-16 21-year cost)
- `lockin-build-and-crossplat.md` Finding #8 (Python `open()` encoding mistake)
