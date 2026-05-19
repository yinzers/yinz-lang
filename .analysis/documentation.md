# Documentation & Clarity — Yinz Compiler

**Scope**: `crates/**/*.rs`
**Findings**: 23 (no severity counts — all standard/low except where noted)

---

## Naming / Magic Constants **FIXED (Batch 8)**

All runtime magic constants extracted:
- `DECIMAL128_STRING_BUF_LEN = 48`
- `INT64_STRING_BUF_LEN = 22`
- `INITIAL_ORDER_CAP = 64`
- `INITIAL_MAP_CAPACITY = 16`
- `INITIAL_ARRAY_CAPACITY = 8`

SHA-256 FIPS 180-4 §6.2.2 citation added to `artifact.rs` doc comment.
Single-letter state variables `a`..`h` preserved (FIPS standard naming; doc comment now notes this explicitly).

---

~~## HIGH — Missing Safety contracts on unsafe FFI~~ **FIXED (Batch 8)**

- **Files**: `crates/ynz-runtime/src/lib.rs:568, 583, 605, 629, 664, 674, 688, 706, 732`
- **Functions**: `ynz_map_get`, `ynz_map_get_str`, `ynz_map_set`, `ynz_map_set_str`, `ynz_map_count`, `ynz_map_has`, `ynz_map_iter_get`, `ynz_map_iter_get_str`, `ynz_map_drop`
- **Gap**: All `#[no_mangle] pub unsafe extern "C"` + `#[allow(clippy::missing_safety_doc)]` — no `# Safety` section. `ynz_siphash_str` at L374 DOES document → pattern inconsistent.
- **Fix**: Add `/// # Safety\n/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet dropped.\n/// - `out` must point to a writable array of size N.`. Remove the `#[allow]` suppressions.

~~## HIGH — `map_grow_int`/`map_grow_str` side-effect contract undocumented~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-runtime/src/lib.rs:464, 499`
- **Gap**: ×2 growth factor, 75% load factor, invalidates all slot pointers — all implicit. Public FFI (`ynz_map_set` calls these) so callers need the contract.

~~## MEDIUM — `build_module` 5-pass flow undocumented~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-codegen/src/emit.rs:129`
- **Gap**: 150+ line function with pass 0 / 1 / 1.5 / 1.6 / 2 sequence; inline comments name passes but no doc explains ordering invariants. Adding a new item kind (e.g., trait objects) needs the docs to know where to slot in.

~~## MEDIUM — `sha256` missing spec citation + complexity~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-codegen/src/artifact.rs:17`
- **Gap**: "hand-transcribed per FIPS 180-4" comment present but no complexity, no rationale for hand-roll vs `sha2` crate.

~~## MEDIUM — `Checker` struct missing field-group explanation~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-typeck/src/check.rs:87`
- **Gap**: 16 fields in three functional groups (borrowed tables, mutable module state, flow-sensitive). Inline comments per field, no group header. `errors_success_narrowed` / `errors_consumed` reset per-function (L165-166); `maybe_non_none` / `union_narrowed` deliberately persist. Non-obvious; would mislead a new contributor.

~~## MEDIUM — Missing crate-level docs~~ **FIXED (Batch 8)**

- **Files**: `crates/ynz-parser/src/lib.rs:1`, `crates/ynz-codegen/src/lib.rs:1`, `crates/ynz-typeck/src/lib.rs:1`
- **Gap**: No `//!` describing entry points (salsa-tracked queries vs direct `lex`/`parse`/`emit_artifact`), pipeline phases, public types.

~~## MEDIUM — SipHash zero-key justification missing~~ **FIXED (Batch 5b)**

Algorithm citation, key-seeding rationale, IV constant provenance, and the zero-key-OK-for-internal-use distinction are all documented in the SipHash section header comment.

## LOW — `Lexer` struct three-mode state machine undocumented

- **File**: `crates/ynz-parser/src/lexer.rs:18`
- **Gap**: `interp_depth_stack: Vec<u32>` is a stack of *inner brace depths per open interpolation* — not a simple counter. Adding a new escape sequence requires understanding the normal/backtick/interpolation state machine.

~~## LOW — `parse_toml_string` parameter contract undocumented~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-driver/src/load.rs:117`
- **Gap**: Function strips the `=` itself; caller passes everything after the key.

## LOW — `options_table.rs::tag_for` returns `i8` but variants can reach 255

- **File**: `crates/ynz-typeck/src/options_table.rs:52`
- **Gap**: `i8::MAX = 127`; validator caps at 256. Variants 128-255 produce negative tag. Silent overflow if validator relaxed.
- **Fix**: Return `u8` (or `debug_assert!(i < 128)` at the cast).

~~## LOW — `errors_result_type` ABI contract for pointer types undocumented~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-codegen/src/emit.rs:504`
- **Gap**: Returns `{i64, i64}`. For pointer-typed success values, field 1 is pointer cast to i64. Caller must know the encoding. Currently unstated.

## LOW — `ExportTable::eq` coarse-equality risk only documented on the impl, not at the field comparison

- **File**: `crates/ynz-typeck/src/exports.rs:35`
- **Gap**: Name-count equality only — signature changes don't trigger Salsa downstream re-run. Deferred to v0.2 but the deferral comment lives on the impl, not the silently-incorrect comparison.

---

## Summary

- Naming / magic constant fixes: 6 (sha-256 state vars, runtime capacity constants)
- Missing function docs: 4 (parse_toml_string, Lexer struct, tag_for, encode_infinity/qnan fixed)
- Missing complexity analyses: 0 (BigNum mul/div/round_to_precision all annotated)
- Missing operational docs (flow/side-effects/safety): 3 (build_module passes, map_grow side effects, FFI safety contracts)
- Missing crate-level docs: 3 (parser, codegen, typeck lib.rs)
- Undocumented business logic / rationale: 2 (sha256 rationale, ExportTable coarse-eq risk) — SipHash zero-key fixed by Batch 5b

---

## Strengths (preserve)

- `crates/ynz-numerics/src/decimal128/bits.rs` — BID bit-layout diagram at file top
- `crates/ynz-numerics/src/decimal128/ops.rs:340-367` — double-rounding analysis
- `crates/ynz-numerics/src/decimal128/wide.rs::U256::div_rem` — complexity + planned-replacement comments
- `crates/ynz-diagnostics/src/diagnostic.rs::new` — three-part format enforced via `assert!`
- `crates/ynz-parser/src/parser.rs` — error-recovery doc + `infix_bp` precedence table with spec mapping
- `crates/ynz-typeck/src/shapes.rs` — `@design-decision` / `@rationale` / `@cost-to-fix` / `@trigger` deferral block per `no-duct-tape.md`
