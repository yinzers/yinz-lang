# Documentation & Clarity — Yinz Compiler

**Scope**: `crates/**/*.rs`
**Findings**: 23 (no severity counts — all standard/low except where noted)

---

## Naming / Magic Constants

| File:line | Current | Suggested | Why |
|---|---|---|---|
| `crates/ynz-numerics/src/decimal128/ops.rs:163` | `subtract: bool` | `negate_b: bool` | param toggles `b`'s sign flip, not the outer op |
| `crates/ynz-numerics/src/decimal128/ops.rs:249` | `coarse_is_a` | `large_is_a` | tracks which operand has the larger exponent |
| `crates/ynz-numerics/src/decimal_n/ops.rs:29-30` | `a_off`, `b_off` | `a_pad`, `b_pad` | leading-zero padding count, not offset |
| `crates/ynz-codegen/src/artifact.rs:73-86` | `a`,`b`,`c`,`d`,`e`,`f`,`g`,`h` (SHA-256 state) | `ha`..`hh` | FIPS uses single-letter; readability suffers in 30-line loop |
| `crates/ynz-runtime/src/lib.rs:410` | `order_cap: i64 = 64` | extract `INITIAL_ORDER_CAP` | rationale missing |
| `crates/ynz-runtime/src/lib.rs:429` | `map_alloc(16)` | `INITIAL_MAP_CAPACITY = 16` | tuning choice with 75% LF threshold |
| `crates/ynz-runtime/src/lib.rs:759` | `cap: i64 = 8` (`ynz_array_new`) | `INITIAL_ARRAY_CAPACITY = 8` | tuning choice |
| `crates/ynz-runtime/src/lib.rs:94` | `[u8; 48]` decimal-to-string buffer | `DECIMAL128_STRING_BUF_LEN = 48` | named constant + format breakdown comment |
| `crates/ynz-runtime/src/lib.rs:154` | `[u8; 22]` int-to-string buffer | `INT64_STRING_BUF_LEN = 22` | same |

---

## HIGH — Missing Safety contracts on unsafe FFI

- **Files**: `crates/ynz-runtime/src/lib.rs:568, 583, 605, 629, 664, 674, 688, 706, 732`
- **Functions**: `ynz_map_get`, `ynz_map_get_str`, `ynz_map_set`, `ynz_map_set_str`, `ynz_map_count`, `ynz_map_has`, `ynz_map_iter_get`, `ynz_map_iter_get_str`, `ynz_map_drop`
- **Gap**: All `#[no_mangle] pub unsafe extern "C"` + `#[allow(clippy::missing_safety_doc)]` — no `# Safety` section. `ynz_siphash_str` at L374 DOES document → pattern inconsistent.
- **Fix**: Add `/// # Safety\n/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet dropped.\n/// - `out` must point to a writable array of size N.`. Remove the `#[allow]` suppressions.

## HIGH — `BigNum::mul` / `div` missing complexity annotations

- **File**: `crates/ynz-numerics/src/decimal_n/ops.rs:183, 262`
- **Gap**: Schoolbook O(n × m) double loop; compare `decimal128/wide.rs:94` which states `Time: O(256)`.
- **Fix**: Add `/// Time: O(n × m); bounded by O(precision²).\n/// Space: O(n + m).` to both.

## HIGH — `map_grow_int`/`map_grow_str` side-effect contract undocumented

- **File**: `crates/ynz-runtime/src/lib.rs:464, 499`
- **Gap**: ×2 growth factor, 75% load factor, invalidates all slot pointers — all implicit. Public FFI (`ynz_map_set` calls these) so callers need the contract.

## MEDIUM — `build_module` 5-pass flow undocumented

- **File**: `crates/ynz-codegen/src/emit.rs:129`
- **Gap**: 150+ line function with pass 0 / 1 / 1.5 / 1.6 / 2 sequence; inline comments name passes but no doc explains ordering invariants. Adding a new item kind (e.g., trait objects) needs the docs to know where to slot in.

## MEDIUM — `sha256` missing spec citation + complexity

- **File**: `crates/ynz-codegen/src/artifact.rs:17`
- **Gap**: "hand-transcribed per FIPS 180-4" comment present but no complexity, no rationale for hand-roll vs `sha2` crate.

## MEDIUM — `Checker` struct missing field-group explanation

- **File**: `crates/ynz-typeck/src/check.rs:87`
- **Gap**: 16 fields in three functional groups (borrowed tables, mutable module state, flow-sensitive). Inline comments per field, no group header. `errors_success_narrowed` / `errors_consumed` reset per-function (L165-166); `maybe_non_none` / `union_narrowed` deliberately persist. Non-obvious; would mislead a new contributor.

## MEDIUM — `decimal_n/ops.rs::div` missing double-rounding analysis

- **File**: `crates/ynz-numerics/src/decimal_n/ops.rs:262` (vs `decimal128/ops.rs:340-367`)
- **Gap**: decimal128 has detailed double-rounding-avoidance commentary; bignum equivalent is one line.

## MEDIUM — Missing crate-level docs

- **Files**: `crates/ynz-parser/src/lib.rs:1`, `crates/ynz-codegen/src/lib.rs:1`, `crates/ynz-typeck/src/lib.rs:1`
- **Gap**: No `//!` describing entry points (salsa-tracked queries vs direct `lex`/`parse`/`emit_artifact`), pipeline phases, public types.

## MEDIUM — SipHash zero-key justification missing

- **File**: `crates/ynz-runtime/src/lib.rs:300-360`
- **Gap**: Standard IV constants used; no spec citation; no note that zero-key SipHash is OK for compiler-internal maps but NOT for user-facing maps with attacker-controlled keys. Future contributor might reuse this for user-facing hashing without realizing.

## MEDIUM — `round_to_precision` complexity undocumented

- **File**: `crates/ynz-numerics/src/decimal_n/bignum.rs:54`
- **Gap**: Called after every arithmetic op. Composition `op → round → normalize → add_one` reads as possibly O(n²) without annotation.

## LOW — `Lexer` struct three-mode state machine undocumented

- **File**: `crates/ynz-parser/src/lexer.rs:18`
- **Gap**: `interp_depth_stack: Vec<u32>` is a stack of *inner brace depths per open interpolation* — not a simple counter. Adding a new escape sequence requires understanding the normal/backtick/interpolation state machine.

## LOW — `parse_toml_string` parameter contract undocumented

- **File**: `crates/ynz-driver/src/load.rs:117`
- **Gap**: Function strips the `=` itself; caller passes everything after the key.

## LOW — `options_table.rs::tag_for` returns `i8` but variants can reach 255

- **File**: `crates/ynz-typeck/src/options_table.rs:52`
- **Gap**: `i8::MAX = 127`; validator caps at 256. Variants 128-255 produce negative tag. Silent overflow if validator relaxed.
- **Fix**: Return `u8` (or `debug_assert!(i < 128)` at the cast).

## LOW — `errors_result_type` ABI contract for pointer types undocumented

- **File**: `crates/ynz-codegen/src/emit.rs:504`
- **Gap**: Returns `{i64, i64}`. For pointer-typed success values, field 1 is pointer cast to i64. Caller must know the encoding. Currently unstated.

## LOW — `encode_infinity` / `encode_qnan` missing docs

- **File**: `crates/ynz-numerics/src/decimal128/bits.rs:166, 172`
- **Gap**: Public functions used by `ops.rs` and `parse.rs`; lack the IEEE 754 reference that `encode_finite` (L139) has.

## LOW — `ExportTable::eq` coarse-equality risk only documented on the impl, not at the field comparison

- **File**: `crates/ynz-typeck/src/exports.rs:35`
- **Gap**: Name-count equality only — signature changes don't trigger Salsa downstream re-run. Deferred to v0.2 but the deferral comment lives on the impl, not the silently-incorrect comparison.

---

## Summary

- Naming / magic constant fixes: 9
- Missing function docs: 5 (encode_infinity/qnan, parse_toml_string, Lexer struct, tag_for)
- Missing complexity analyses: 3 (BigNum mul/div/round_to_precision)
- Missing operational docs (flow/side-effects/safety): 3 (build_module passes, map_grow side effects, FFI safety contracts)
- Missing crate-level docs: 3 (parser, codegen, typeck lib.rs)
- Undocumented business logic / rationale: 3 (sha256 rationale, SipHash zero-key justification, ExportTable coarse-eq risk)

---

## Strengths (preserve)

- `crates/ynz-numerics/src/decimal128/bits.rs` — BID bit-layout diagram at file top
- `crates/ynz-numerics/src/decimal128/ops.rs:340-367` — double-rounding analysis
- `crates/ynz-numerics/src/decimal128/wide.rs::U256::div_rem` — complexity + planned-replacement comments
- `crates/ynz-diagnostics/src/diagnostic.rs::new` — three-part format enforced via `assert!`
- `crates/ynz-parser/src/parser.rs` — error-recovery doc + `infix_bp` precedence table with spec mapping
- `crates/ynz-typeck/src/shapes.rs` — `@design-decision` / `@rationale` / `@cost-to-fix` / `@trigger` deferral block per `no-duct-tape.md`
