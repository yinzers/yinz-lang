# Performance Analysis — Yinz Compiler

**Scope**: `crates/**/*.rs` — lexer, parser, type checker, diagnostics, numerics, driver
**Issues**: 10 (Critical: 1, High: 4, Medium: 4, Low: 1)

---

~~## CRITICAL — `DiagnosticBucket::push` counts errors with full linear scan on every push~~
**FIXED in Batch 6.1** — `error_count: usize` field added; O(1) push + `has_errors()`.

~~## HIGH — `levenshtein()` allocates full 2-D matrix per call~~
**FIXED in Batch 6.2** — two-row rolling DP, byte-level, early-exit on length diff.

~~## HIGH — `detect_extends_cycles` uses `Vec::contains` in walk — O(n²)~~
**FIXED in Batch 6.3** — swapped to `HashSet<String>`.

~~## HIGH — `render()` clones every source string into `ariadne::Source` every render call~~
**FIXED in Batch 6.4** — lazy `SourceCache` builds `Source` on first access per file.

~~## MEDIUM — `has_errors()` O(n) scan~~
**FIXED in Batch 6.1** — covered by `error_count` field.

## MEDIUM — Lexer allocates `String` per identifier token

- **File**: `crates/ynz-parser/src/lexer.rs:486-639`
- **Complexity**: O(identifier_length) allocation per identifier
- **Issue**: Every `Token::Identifier(text.to_string())` heaps a String. Banned keywords too. Parser later clones again into AST nodes. 1000-line file × ~400 identifiers = 400+400 allocs.
- **Fix (preferred)**: Store byte-span `(usize, usize)` into source; materialize strings only at AST build time. Consistent with existing `Token::IntLit(i64)` design.
- **Fix (alt)**: String interner (rustc-hash + bump arena).

## MEDIUM — Lexer allocates `String` per numeric literal for underscore stripping

- **File**: `crates/ynz-parser/src/lexer.rs:879, 948, 1075, 1078`
- **Issue**: `raw.chars().filter(|&c| c != '_').collect()` per hex/binary/decimal/float literal.
- **Fix**: Inline byte-by-byte radix accumulator that skips `_`. Zero-alloc.

---

## Already Performant (preserve)

- `decimal128` arithmetic — stack-only `u128`/`U256`, O(1) operations
- Lexer byte dispatch — `match byte` is O(1) table lookup
- ShapeTable / SignatureTable — `HashMap` O(1)
- Scope frame-stack lookup — appropriate for typical nesting (2-5 frames)
- Parser token indexing — `tokens[pos]`, O(1) `peek`/`advance`

---

## Priority Order

1. **DiagnosticBucket error_count field** — 5-line fix, fires on every error. Auto-fixes Medium #6.
2. **detect_extends_cycles HashSet swap** — trivial, already inconsistent with neighbor code.
3. **levenshtein rolling DP** — every typo error pays cost.
4. **render() lazy Source fetch** — every render pays full-source cost.
5. **Lexer identifier byte-span tokens** — larger refactor; defer to dedicated lexer pass.
6. **Numeric-literal underscore-strip** — minor.
