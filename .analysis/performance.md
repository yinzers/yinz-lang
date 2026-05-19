# Performance Analysis — Yinz Compiler

**Scope**: `crates/**/*.rs` — lexer, parser, type checker, diagnostics, numerics, driver
**Issues**: 10 (Critical: 1, High: 4, Medium: 4, Low: 1)

---

## CRITICAL — `DiagnosticBucket::push` counts errors with full linear scan on every push

- **File**: `crates/ynz-diagnostics/src/bucket.rs:29-38`
- **Complexity**: O(n) per push → O(n²) total for n diagnostics
- **Issue**: Every error-severity push calls `.iter().filter(...).count()` over the whole Vec before deciding to cap. Fires from every error site in lexer/parser/typeck — including speculative pushes that later get truncated.
- **Fix**: Add `error_count: usize` field, increment on push. Cap check O(1). `truncate()` must update count; `has_errors()` becomes `self.error_count > 0`.
- **Gain**: O(n²) → O(1) per push. Fix covers Finding #6 (`has_errors`) automatically.

## HIGH — `levenshtein()` allocates full 2-D matrix per call

- **File**: `crates/ynz-typeck/src/check.rs:3683-3700`
- **Complexity**: Time O(m×n) per pair (expected); Space O(m×n) (avoidable)
- **Issue**: `vec![vec![0usize; n+1]; m+1]` plus two `Vec<char>` allocations for char conversion. Fires on every undefined-identifier error against every candidate name in scope. 20 candidates × matrix allocation per typo.
- **Fix**: Two-row rolling DP → O(min(m,n)) space. Work on bytes (identifiers are ASCII). Early-exit when `|m-n| > threshold`.
- **Gain**: O(m×n) → O(n) space; eliminates 2 allocs per candidate.

## HIGH — `detect_extends_cycles` uses `Vec::contains` in walk — O(n²)

- **File**: `crates/ynz-typeck/src/shapes.rs:474-495`
- **Complexity**: O(n²) where n = chain depth (`.contains` is O(k))
- **Issue**: `visited: Vec<String>` + `.contains(parent)` linear scan. Inconsistent with `has_cycle` (L556) which already uses `HashSet`.
- **Fix**: Swap to `HashSet<String>`. Build chain string separately for diagnostics if needed.
- **Gain**: O(n²) → O(n) per starting node.

## HIGH — `render()` clones every source string into `ariadne::Source` every render call

- **File**: `crates/ynz-diagnostics/src/render.rs:55-59`
- **Complexity**: O(total_source_bytes) per render
- **Issue**: `sources.iter().map(|(k, v)| (k.clone(), Source::from(v.clone())))` eagerly clones+parses all sources, even files without diagnostics. `Source::from` also builds the line-offset table.
- **Fix**: Lazy `fetch()` in `SourceCache` — only build `Source` on first access per file. Or store `&str` references (sources outlive render).
- **Gain**: Eliminates clone + line-table construction for every diagnostic-free file. 10-file project, 1 error → 9 files of wasted construction removed.

## MEDIUM — `has_errors()` O(n) scan

- **File**: `crates/ynz-diagnostics/src/bucket.rs:43-47`
- Covered by Critical fix above. Becomes `self.error_count > 0`.

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
