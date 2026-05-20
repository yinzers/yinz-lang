# Standard Library — String Module (M7)

Per-method documentation for the M7 string API. All methods are on the built-in `string` type — no import needed.

User spec: `spec/strings.md`

---

## Indexing and Access

### `.get(n)` / `s[n]`

```
.get(n: int) -> maybe string
```

Returns the code point at index `n` as a 1-character string. `none` if out of bounds. `O(n)` scan from start (UTF-8 storage). Bracket sugar `s[n]` desugars to `.get(n)`.

**Ownership:** `share self` — pure read, no allocation for the result (SSO inline for a 1-char result).

### `.byteAt(n)`

```
.byteAt(n: int) -> maybe int
```

Returns the raw UTF-8 byte at byte-offset `n` as an integer (0–255). `none` if out of bounds. `O(1)` direct memory access — no UTF-8 scanning.

**Ownership:** `share self`.

### `.graphemeAt(n)`

```
.graphemeAt(n: int) -> maybe string
```

Returns the grapheme cluster at grapheme-index `n` as a string (may be multiple code points). `none` if out of bounds. Uses `unicode-segmentation` crate for cluster boundaries.

**Ownership:** `share self`.

---

## Length

### `.count()`

```
.count() -> int
```

Number of Unicode code points. `O(n)` UTF-8 scan.

### `.byteCount()`

```
.byteCount() -> int
```

Number of UTF-8 bytes. `O(1)` for heap strings (stored in `len` field); `O(1)` for inline strings (tag byte encodes length).

### `.graphemeCount()`

```
.graphemeCount() -> int
```

Number of grapheme clusters. `O(n)` via `unicode-segmentation`.

---

## Search

### `.contains(substr)`

```
.contains(substr: string) -> boolean
```

Returns `true` if `substr` appears anywhere in `self`. For patterns ≥ 16 bytes, uses SIMD-accelerated `memchr::memmem`. For patterns ≤ 15 bytes, uses scalar scan. Pure read. No allocation. `stdlib-design.md` Rule 1: no I/O.

**Ownership:** `share self, share substr`.

### `.indexOf(substr)`

```
.indexOf(substr: string) -> maybe int
```

Returns the byte-offset of the first occurrence of `substr`, or `none`. Same SIMD/scalar dispatch as `.contains`.

**Ownership:** `share self, share substr`.

### `.startsWith(prefix)`

```
.startsWith(prefix: string) -> boolean
```

Returns `true` if `self` starts with `prefix`. Byte-level prefix check. `O(prefix.byteCount())`.

### `.endsWith(suffix)`

```
.endsWith(suffix: string) -> boolean
```

Returns `true` if `self` ends with `suffix`. Byte-level suffix check.

---

## Transformation

### `.toUpperCase()`

```
.toUpperCase() -> string
```

Locale-invariant Unicode case folding (uppercase). Result `is_nfc_known = false` (case folding may produce NFD). Uses `unicase` crate tables. Allocates a new string.

### `.toLowerCase()`

```
.toLowerCase() -> string
```

Locale-invariant Unicode case folding (lowercase). Result `is_nfc_known = false`. Uses `unicase` crate tables. Allocates a new string.

### `.substring(start, end)`

```
.substring(start: int, end: int) -> string
```

Returns a new string containing code points `[start, end)`. `start` and `end` are code-point indices. Bounds-clamped (no panic; clamped result may be empty). Allocates if result > 23 bytes; SSO inline otherwise. Result `is_nfc_known = true` if source was NFC-known AND boundaries are code-point boundaries.

### `.trim()`

```
.trim() -> string
```

Returns a new string with leading and trailing Unicode whitespace removed. Byte-level for ASCII whitespace (fast path); full Unicode for non-ASCII whitespace. Result `is_nfc_known = true` if source was NFC-known.

### `.split(separator)`

```
.split(separator: string) -> array<string>
```

Splits `self` on every occurrence of `separator`. Returns an `array<string>`. Empty separator is a compile error. Each piece: `is_nfc_known = true` if source was NFC-known.

### `.replace(old, new)`

```
.replace(old: string, new: string) -> string
```

Replaces all occurrences of `old` with `new`. Returns a new string. `is_nfc_known = true` only if source AND `new` were both NFC-known.

---

## String Iteration

### For-loop over string

```ynz
for c in "café" {
  // c is a 1-character string (one code point per step)
}
```

Uses `StringCodePointIter` wrapper shape. Default = code points. For grapheme iteration, use `.graphemes()` — deferred to v0.5+.

---

## Runtime Symbols Added in M7

| Symbol | Purpose |
|---|---|
| `ynz_string_concat(a, b) -> YnzString` | Concatenation; returns 24-byte struct |
| `ynz_string_eq(a, b) -> bool` | NFC-aware equality (replaces M1's byte-eq) |
| `ynz_string_codepoint_at(s, n) -> {tag, value}` | Code point access (UTF-8 walk) |
| `ynz_string_grapheme_at(s, n) -> YnzString` | Grapheme access (unicode-segmentation) |
| `ynz_string_contains(s, substr) -> bool` | SIMD search |
| `ynz_string_index_of(s, substr) -> {tag, value}` | SIMD search, maybe<int> |
| `ynz_string_to_upper(s) -> YnzString` | Case fold |
| `ynz_string_to_lower(s) -> YnzString` | Case fold |
| `ynz_string_builder_new() -> Builder` | Interpolation / multi-concat |
| `ynz_string_builder_append(b, s)` | Append to builder |
| `ynz_string_builder_finalize(b) -> YnzString` | Close builder, return string |

---

## Cross-References

- `spec/strings.md` — user-facing surface (audience: HS-grad JS developer)
- `design/strings.md` — SSO layout, NFC propagation table, SIMD crate selection
- `design/collections.md` — bracket sugar (`s[n]` → `.get(n)`), shared indexing API design
- `crates/ynz-runtime/src/lib.rs` — implementation of runtime symbols above
