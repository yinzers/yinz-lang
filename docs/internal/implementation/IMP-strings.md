---
name: "IMP-strings"
description: "Locks Yinz's internal string encoding (UTF-8 always) and SIMD implementation strategy, plus UTF-8 defaults for file I/O, underlying the user-facing indexing API in docs/reference/REF-strings.md."
tags:
  - "yinz-compiler"
created_at: "2026-05-15"
updated_at: "2026-11-13"
status: "active"
author: "patrick"
metadata:
  type: "specification"
---

# Strings — Internal Encoding and Implementation

User spec: [`docs/reference/REF-strings.md`](../../reference/REF-strings.md) (indexing API, methods).

This file locks the internal encoding and the SIMD strategy for the built-in `String` type, and records how string memory is owned and released. The user-facing API (`.byteAt()`, `.get()` by code point, `.graphemeAt()`, `.count()`/`.byteCount()`/`.graphemeCount()`) is in [`docs/internal/implementation/IMP-collections.md`](IMP-collections.md) "String Methods" and [`docs/reference/REF-strings.md`](../../reference/REF-strings.md).

**Built today**: UTF-8 storage, SIMD validation and search, locale-invariant case operations, NFC-aware equality, and the ownership and release rules under "String Ownership and Release". **Designed, not built**: the 24-byte inline-or-heap `YnzString` layout (with the 23-byte inline threshold and `is_nfc_known` bit), the NFC-known propagation table, and `.toLowerCaseLocale(locale)` / `.toUpperCaseLocale(locale)`.

---

## Internal Encoding: UTF-8, Always

**Decision**: Yinz strings are UTF-8 internally, with no alternate encoding, no platform-default fallback and no UTF-16 mode.

**Why**: ASCII text takes 1 byte per character, and the encoding never needs a hidden second representation behind a runtime branch.

**Cost**: code-point indexing (`s.get(n)`) is O(n) because it scans from the start. `.byteAt(n)` is O(1) for protocol parsing, `.graphemeAt(n)` is O(n) for human-character access, and linear iteration (`for c in s`) is O(n) total.

---

## File I/O Encoding Default: UTF-8

When the standard library reads a text file (`file.read(path)` once the `file` module ships), the default encoding is UTF-8 unconditionally, never the locale or OS code page. A non-UTF-8 encoding is an explicit parameter: `file.read(path, encoding: .latin1)`.

**Why**: an OS-dependent default makes code that works on one platform fail silently on another when it meets non-ASCII data.

---

## SIMD-Accelerated UTF-8 Validation and Traversal

Built-in string operations that touch every byte (validation, decoding, search) use SIMD intrinsics where the target architecture supports them, via the crates in "SIMD Crate Selection". When the target has no suitable intrinsics, the compiler emits the scalar implementation with the same correctness.

---

## String Search

`ynz_string_contains` and `ynz_string_index_of` use `memchr::memmem::find` for every pattern length (`crates/ynz-runtime/src/lib.rs:2565-2589`). The performance target is `string.contains` at 1+ GB/s on patterns of 16 bytes or more. The algorithm is an implementation detail.

---

## Small String Optimization — 23-byte Inline Threshold (designed, not built)

**This is not a maximum string length.** Strings can be any length up to available memory (the length field is `Int`, 64-bit). The 23-byte threshold controls only where the bytes live, transparently to user code.

| String length | Storage | Access cost |
|---|---|---|
| ≤ 23 bytes | Inline in the 24-byte string value itself (stack or struct) | 1 cache line, no pointer chase |
| > 23 bytes | Heap-allocated; the string value holds a pointer + length | One pointer chase to the heap |

From the user's perspective, both behave identically — `.count()`, `.byteAt()`, `.contains()`, every string operation works the same way. The compiler picks the storage layout based on length; the user never has to think about it.

**The threshold is fixed, not implementation-defined.**

**Why 23 bytes**: it fits a 24-byte string struct (two registers) and covers identifiers, labels, short keys and short text. A fixed threshold keeps the string ABI stable; changing it later is an ABI break.

### Auto-promotion (codegen + muted hint)

Per [`.claude/rules/auto-promotion.md`](../../../.claude/rules/auto-promotion.md):
- **Codegen**: every string literal of 23 bytes or fewer is stored inline, with no heap allocation.
- **Muted IDE hint**: when the compiler proves a string variable only ever holds 23 bytes or fewer (a `shape` field initialized only from short literals, a `const` string), the IDE shows `// fits inline — no heap`.
- **No lint suggestion**: there is no source-level rewrite to suggest, because the user wrote a string and the compiler picked the storage.

---

## Locale-Invariant Case Operations Default

`.toLowerCase()` and `.toUpperCase()` are locale-invariant Unicode case conversion (`ynz_string_to_lower`/`ynz_string_to_upper` apply the standard Unicode mappings, `lib.rs:2624-2650`). The standard library has no "use the system locale" default for case operations, which extends [`.claude/rules/stdlib-design.md`](../../../.claude/rules/stdlib-design.md) Rule 3 (no silent platform-dependent defaults).

Locale-aware conversion is a separate, explicitly named pair, `.toLowerCaseLocale(locale)` and `.toUpperCaseLocale(locale)` (not built), so the locale dependency shows at every call site.

**Why**: in a Turkish locale `I` lowercases to `ı` and `i` uppercases to `İ`, so a case-insensitive security or routing check such as ``inputName.toLowerCase() == `admin` `` breaks silently for users in one locale. The locale-aware pair is for UI text, never for comparisons.

---

## 24-Byte String Layout (designed, not built)

**Locked.** Any change is an ABI break: a semver major bump and a recompile of all dependent binaries. No `YnzString` type or `crates/ynz-runtime/tests/string_layout.rs` exists yet.

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
- `` `hi` ``: inline, length 2, NFC-known. Bytes 0..1 = `h`, `i`; bytes 2..22 = zero-fill; byte 23 = `0x80 | 0x40 | 0x02 = 0xC2`.
- 30-char ASCII literal: heap. Bytes 0..7 = ptr; bytes 8..15 = `len = 30 | (1 << 1) = 32` (NFC-known); bytes 16..23 = `cap = 30`.

**Verification when built:** a layout test asserts `mem::size_of::<YnzString>() == 24` and `mem::align_of::<YnzString>() == 8`, and a bit-pattern test asserts the exact bytes of known inline and heap strings.

---

## String Ownership and Release

Current layout (the 24-byte layout above is not built): a `String` value is one `ptr` to NUL-terminated UTF-8 bytes. A `u64` header sits in front of the first byte (`STRING_HEADER_BYTES = 8`, `STATIC_STRING_HEADER = 0`, `crates/ynz-abi/src/lib.rs:204-216`):

| Header | Bytes are | Source |
|---|---|---|
| `STATIC_STRING_HEADER` (0) | static, never freed (a static string is the only kind never freed; every heap string is released by its owner) | literals, `true`/`false`, options display names, the `none`/placeholder debug text, error-message literals, `[REDACTED]` |
| any other value | heap, owned; the value is the byte capacity (length + NUL, never 0) | every string the runtime allocates: Interpolation, string methods, `Int`/`Float`/`Number` `.toString()`, bignum results |

Rules:

- Every string source that can reach Yinz code carries the header: codegen emits static strings as `{ i64 0, [N x i8] }` (`build_static_string`, `crates/ynz-codegen/src/emit.rs:35909-35942`), and `heap_string_buffer` (`crates/ynz-runtime/src/lib.rs:642-655`, the one string allocator, counted by `YNZ_ALLOC_COUNTER`) writes the capacity. Compiler-internal C strings (panic text, file names, debug-print fragments) are not `String` values and have no header.
- `ynz_string_release(ptr)` (`crates/ynz-runtime/src/lib.rs:657-689`) is the one release entry point: null and static bytes are no-ops; heap bytes are freed and counted. It accepts any string, so a caller never needs to know where one came from. Codegen calls it from the one release helper (`Cg::release_fn`, `emit.rs:29334-29345`; the string arm of `release_step`, `emit.rs:28990-29053`) at every scope exit that releases a string (`IMP-ownership.md` "The one release helper").
- A heap string has exactly one owner when it is released. A store guarantees that with the rule in "Storing a string" below.
- A conversion result is never shared: two `.toString()` results alive together are two strings.
- `print(n)` and `${n}` of an `Int`, `Float` or `Number` build no string: the text is read once and dropped, so codegen calls a runtime entry that formats and writes (`ynz_{int,float,decimal}_print`) or appends to the string builder (`ynz_{int,float,decimal}_append`; `crates/ynz-runtime/src/lib.rs:288-360`), with nothing to release. `Int` formats into a stack buffer owned by that call (never shared across calls or threads); `Float` prints from one and appends by formatting straight into the builder; `Number` keeps the one `String` the numerics crate returns (the only allocation left). Only `.toString()` returns an owned heap string. Test: `v03_m9_string_ownership` counts heap strings for each case.
- Cost: 8 header bytes per string, one 8-byte store per allocation, one load and compare per release, no atomics.

### Storing a string

One rule, applied at every place that keeps a string: **a store takes a string by move when typeck's record says nothing else can reach it, and otherwise by an owned copy.** The record is `TypedModule::string_moves` (`StringMoves`, `crates/ynz-typeck/src/string_ownership.rs:46-52`, filled by the one liveness walk `record_moves_and_reads`, `:96-118`); codegen reads it and decides nothing itself. The copy is `OwnedCopy::StringClone` (`owned_copy_plan`, `crates/ynz-typeck/src/owned_copy.rs:147-332`; the runtime entry `ynz_string_clone`, `crates/ynz-runtime/src/lib.rs:691-708`, a fresh counted heap string with the same bytes), made through the one `emit_owned_copy` seam (`crates/ynz-codegen/src/emit.rs:28103-28387`). Codegen reaches that seam through one function, `own_string_for_store` (`emit.rs:22446-22464`: a string remembered as movable is taken as it is, any other is copied); there is no second copy routine. A string missing from the record is copied, so a gap in the record costs an allocation, never a double free.

The stores that take the rule: a `let`/`const` binding and an assignment, a shape field, an array literal element, `add` and index assignment, a `fixed` element, a map key (stored by raw pointer) and a map value, a channel `send`, a `background` argument, a value entering a union slot (`let`, field, argument, `.or()` fallback), a `return`, and the payload of a `Maybe<String>` that is bound, stored or returned.

| An expression is | When | Store |
|---|---|---|
| Fresh | an interpolation with a `${}` part; `toUpperCase`, `toLowerCase`, `trim`, `substring`, `replace`; `Int`, `Float` and `Number` `.toString()`; `.copy()`; a call to a Yinz function | move |
| The last use of its binding | a `let`/`const` string read once, with no mention of the binding on any path after it | move |
| Borrowed | a literal or pure-text backtick string, `boolean.toString()` (static bytes); an element, field, map value or parameter read; an `.or()` result; a loop cell; a read of a binding that is used again | owned copy |

A function returns an owned string, so a caller binds a call result by move; inside the callee a returned local at its last use moves and anything else is copied at the `return`. A `share`, `lend` or bare string parameter is never movable: the caller owns it. A `give` string parameter is the exception: the call hands the string over (the checker refuses any later read of the caller's binding), so the callee owns and releases it and the caller's release plan omits that binding as moved (`Omitted(Moved)`, or `ReleaseIfLive` when only some paths gave it). Exactly one side releases. Only a string that founds its own class follows the give; a string joined to a shape's class (a `let` of a field read) owns a copy the give does not take, so its own scope releases it. See `IMP-ownership.md`, "Strings".

Containers copy their strings with themselves, all the way down. `.copy()` and a `background` copy of a shape (every heap field: `String`, nested shape, `Array`, `Map`, `Maybe`, envelope union, `fixed` cells), a `Map<String, V>` or `Map<K, string>` (keys and values), an `Array<String>`, a `Fixed<String>`, an `Array<Shape>` whose shape has `String` fields, and a `Maybe<String>` or `Maybe<Shape>` payload each give the copy strings of its own. The emitters are `deepen_shape_fields` (`emit.rs:5924-6002`), `emit_array_elem_deep_copy` (`emit.rs:28739-28852`), `emit_map_values_deep_copy` (`emit.rs:28555-28727`) and `emit_fixed_item_cells_copy` (`emit.rs:28451-28547`). An auto-Arc spawn group's block holds one such copy of the shape's strings for all its tasks, and the last `ynz_arc_free` releases it (`emit.rs:24601-24628`; `IMP-ownership.md` "What the block owns"). Equality follows: String fields (`shape_value_eq`, `emit.rs:7791-8108`, the string arm at `:8037-8048`) and `Array<String>.contains` (`lower_array_method`, `emit.rs:33875-34099`, the string arm at `:34025`) compare by content, since a stored copy has no pointer identity with its source.

### The scope-exit release pass

The pass exists. Typeck decides, once, which binding releases what on which exit edge (`ReleasePlan`, `crates/ynz-typeck/src/release_plan.rs:1229-1240`); codegen emits that list at each scope exit through `emit_release_edge` (`emit.rs:30830-30836`) and `emit_release_edge_with` (`emit.rs:30841-30871`) and derives no ownership fact of its own. Pin: `v03_m9_release_safety`.

- A string is never statically moved. `string_moves` says a read of a `let` binding may be taken by a store. The plan lists each such last-use read as a `MoveSite` with `MoveCause::LastUse` (`release_plan.rs:200-209`, `:1475-1491`, `:193-195`), and the binding's fate is `ReleaseIfLive`, else `Release`.
- Codegen decides at the store. `remember_movable_string` (`emit.rs:22392-22427`) remembers a movable string; the store seam (`own_string_for_store`) takes it and nulls the binding's slot, so the scope-exit release skips it. A copy leaves the binding owning its string, and the release frees it.
- A binding read at its last use by something that is not a store (a `print`, a call argument that is only read) still owns its string and is released.
- Every stored string is released by its owner once: the binding, field, element, map cell, envelope or channel slot that holds it.

Cost: a store clones only a borrowed string (a literal, a field or parameter read, an `.or()` result) and a `Maybe<String>` or `Maybe<Shape>` payload that is bound or stored; every other store moves.

Test: `v03_m9_c3b_string_store` counts heap strings for each store kind and runs each under AddressSanitizer at both optimization levels.

Under the 24-byte layout above, the rules carry over unchanged. What changes is where the "static" marker lives: inline strings (23 bytes or fewer) own nothing, so releasing one is a no-op; for the heap form the marker moves from the pre-header into the value (`len` bit 0, which this layout reserves, is the proposed "static, not owned" bit), the header word and the 8-byte allocation prefix go away, and `ynz_string_release` takes the 24-byte value.

---

## NFC-Known Propagation Table (designed, not built)

Today `ynz_string_eq` takes pointer equality, then a byte compare for two ASCII strings, then NFC-normalizes both (`lib.rs:372-390`). The table is what the `is_nfc_known` bit adds: for every string-producing operation, whether the result carries `is_nfc_known = true`:

| Operation | Result `is_nfc_known`? | Why |
|---|---|---|
| String literal (parser) | TRUE — compiler pre-normalizes at compile time | The parser runs an NFC pass |
| Backtick-interpolation result | TRUE only if every segment was NFC-known | One non-NFC segment poisons the result |
| `s1 + s2` | TRUE only if BOTH were NFC-known AND s2 doesn't start with a combining-class code point | `` `e` + `́` `` produces NFD even if both sides are individually NFC-normal. Conservative: force false when s2 first code point has combining class > 0. |
| `.substring(start, end)` | TRUE if source was NFC-known AND boundaries are code-point boundaries | Substring of NFC is NFC |
| `.trim()` | TRUE if source was NFC-known | Trim is byte-level on whitespace; preserves NFC |
| `.split(sep)` | TRUE for each piece if source was NFC-known | Split is byte-level; preserves NFC per piece |
| `.replace(old, new)` | TRUE only if source AND `new` were both NFC-known | Inserts `new` text |
| `.toUpperCase()` / `.toLowerCase()` | FALSE | Case folding produces NFD code points in many cases (Turkish-I, Greek, etc.) |
| `.toString()` on primitives | TRUE | ASCII only |
| `.toString()` on user shape | FALSE (conservative) | User-defined formatting may produce NFD |
| `String.fromBytes(bytes)` | FALSE | Runtime byte input may not be normalized |
| Single code point from `s.get(n)` | FALSE | A single code point in isolation cannot be verified NFC without context |

**Fast path for `ynz_string_eq`:** if both strings have `is_nfc_known = true`, byte-compare directly. Slow path: normalize both via `unicode-normalization::nfc()` then byte-compare.

---

## SIMD Crate Selection

All four crates are pinned with `=` in `crates/ynz-runtime/Cargo.toml:18-21`:

- **`simdutf8 = "=0.1.4"`**: UTF-8 validation with runtime CPU feature detection and a scalar fallback (used in `lib.rs:2471`).
- **`memchr = "=2.7.4"`**: SIMD `memmem` search for `.contains` and `.indexOf`.
- **`unicode-normalization = "=0.1.24"`**: NFC normalization in the `ynz_string_eq` slow path. It provides NFC/NFD only, not case conversion.
- **`unicase = "=2.7.0"`**: declared for locale-invariant case folding. `ynz_string_to_upper`/`ynz_string_to_lower` currently use the standard `char::to_uppercase`/`to_lowercase` mappings.

---

## SIMD Fallback CI Requirement

A second CI job, alongside the normal x86_64 job, sets `RUSTFLAGS=-C target-feature=-sse4.1,-avx2` to disable SIMD intrinsics and exercise the scalar fallback. Both jobs must pass. No workflow in the repository defines this job yet.

---

## What This Doc Does NOT Cover

- **The user-facing API** (`.split()`, `.toUpperCase()` and the rest): [`docs/reference/REF-strings.md`](../../reference/REF-strings.md).
- **Indexing semantics** (code point, byte, grapheme): [`docs/internal/implementation/IMP-collections.md`](IMP-collections.md) "String Methods".
- **Internationalization library** (NFC/NFD/NFKC/NFKD, locale-aware sort): not designed.

---

## Cross-References

- [`docs/internal/implementation/IMP-collections.md`](IMP-collections.md) "String Methods" section (user-facing API)
- [`docs/reference/REF-strings.md`](../../reference/REF-strings.md) (user spec for string operations)
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 8 (compiler does the hard work — SIMD selection)
- [`docs/reference/REF-golden-rules.md`](../../reference/REF-golden-rules.md) Rule 7 (efficiency first — UTF-8 is the efficient default for ASCII)
