# Bug Analysis Report

**Analyzed**: 2026-05-19
**Files Checked**: ~35 (crates/**/*.rs, focused on parser, typeck, codegen, runtime, driver)
**Scope**: Yinz compiler — Rust crates that lex, parse, type-check, lower to LLVM IR, and link binaries

---

## CRITICAL BUGS (Fix Immediately)

### ~~Bug #1: `load_project` still prefers `src/`~~ FIXED (Batch 4b)

`load.rs` now walks from `root` directly with no `src/` fallback. Error message updated to "No `.ynz` source files found in the project root." All fixtures and `examples/basics/` restructured. 93/93 driver tests pass.

---

## HIGH SEVERITY BUGS

~~### Bug #6: `validate_underscores` misses leading underscore in hex/binary literals~~ **FIXED (Batch 5a)**

Leading-`_` check added to `lex_hex_int` and `lex_binary_int` before `validate_underscores`. `0x_FF` and `0b_1010` now emit a teaching diagnostic. Tests: `hardening_leading_underscore_hex_rejected`, `hardening_leading_underscore_binary_rejected`.

---

~~### Bug #7: Memory leak / use-after-free on OOM in map runtime~~ **FIXED (Batch 5b)**

`map_alloc`, `map_grow_int`, `map_grow_str`, and `order_push` now null-check every `malloc`/`realloc` return and abort with a teaching message on null.

---

~~### Bug #8: `ynz_map_set_str` can infinite-loop on a full map~~ **FIXED (Batch 5b)**

`find_insert_slot` and the `ynz_map_set_str` probe loop now track probe count and abort with a compiler-bug message if probe count reaches capacity.

---

### Bug #9: `parse_string_to_int` rejects `i64::MIN`

**File**: `crates/ynz-runtime/src/lib.rs:1014-1030`
**Severity**: MEDIUM
**Category**: Logic Error / Edge case

**Issue**: The function accumulates digits as positive i64 then negates at the end:

```rust
acc = acc.checked_mul(10)?.checked_add(digit)?;
...
Some(if neg { acc.checked_neg()? } else { acc })
```

The string `"-9223372036854775808"` (i64::MIN) requires `acc = 9223372036854775808` which overflows i64::MAX before negation. Result: `i64::MIN` is unrepresentable via `.toInt()` — the user gets `none` for a legitimately-representable value.

---

### Bug #10: `entrypoint` → `main` rename per file enables duplicate symbol on multi-file projects

**File**: `crates/ynz-codegen/src/emit.rs:460-465`, `815-819`
**Severity**: HIGH
**Category**: Logic Error / Linker error path

**Issue**: Every file's `entrypoint` function is renamed to LLVM symbol `main`. The driver compiles all `.ynz` files in a project. If two files both contain `function entrypoint() -> nothing` (e.g., a developer copy-pasted a starter file), both emit a `main` symbol — link fails with a confusing C linker error instead of a Yinz-level diagnostic.

Worse: if a single file has both `function entrypoint()` and `function main()`, the second produces a literal `main` C symbol, again colliding.

No deduplication check exists at signature collection or codegen — `collect_signatures` checks for in-file duplicates only.

---

### Bug #11: `find_project_root` discrepancy in `resolve_imports` diagnostic

**File**: `crates/ynz-typeck/src/resolve_import.rs:110-112`
**Severity**: MEDIUM
**Category**: Logic Error / Inconsistent diagnostic

**Issue**:

```rust
let has_project_root = find_project_root(
    std::path::Path::new(importer_path).parent().unwrap_or(std::path::Path::new("."))
).is_some();
```

If `importer_path` has no parent (e.g., the importer is `"main.ynz"` with no directory), the fallback `"."` may or may not have a `yinz.toml`. Meanwhile, `resolve_module_path` (line 51-72) starts with:

```rust
let importer_dir = importer.parent()?;   // returns None if no parent
```

So `resolve_module_path` returns None entirely, while `has_project_root` computes a different answer (the current directory's project). The diagnostic emitted on import failure uses `has_project_root` to decide between two error messages — but the error MESSAGE doesn't match the actual resolution path that failed. User gets misleading "no yinz.toml" guidance when the real problem was the importer path had no parent.

---

### Bug #12: Dead-code import path resolution fallback

**File**: `crates/ynz-typeck/src/resolve_import.rs:64-71`
**Severity**: LOW
**Category**: Logic Error

**Issue**: The function's else-branch (line 67-71) reattempts `importer_dir.join(...)` — but when `project_root.is_none()`, the code at line 58 already set `base = importer_dir`, and the line-59 candidate equals line-69's `rel`. Already failed at line 61 (`candidate.exists()` false). Line 70 will produce the same `false` result. Dead code that confuses readers about the intent of the fallback.

---

## MEDIUM SEVERITY BUGS

### Bug #13: Codegen `mangle_type` produces ambiguous names for unhandled variants

**File**: `crates/ynz-codegen/src/emit.rs:292-311`
**Severity**: MEDIUM
**Category**: Logic Error / Potential symbol collision

**Issue**: The "catch-all" arm at line 307-309:

```rust
other => format!("{other:?}")
    .to_lowercase()
    .replace([' ', '{', '}', '"', ':'], "_"),
```

Uses Debug formatting of arbitrary Type variants. Two different Types whose Debug output differs only in characters that get replaced/lowercased could mangle to the same name. Example: `Type::Number { precision: 5 }` vs `Type::Number { precision: 10 }` — both contain `number` in debug output; the precision number appears but in different surrounding text — could in theory collide. Also: ordering of fields in Debug is not guaranteed stable across compiler versions. Cross-compilation could produce different symbol names for the same type.

---

~~### Bug #14: `lex_decimal_number` accepts `3.` as a valid number literal~~ **FIXED (Batch 5a)**

`lex_decimal_number` now checks for `.` followed by non-digit/non-alpha before the `has_dot` path. `3.` at EOF/operator emits a "Decimal point without fractional digits" diagnostic. `42.toString()` is unaffected (`.` followed by `t`). Test: `hardening_bare_dot_float_rejected`.

---

### Bug #15: `find_slot` may early-return None when map is mostly-full

**File**: `crates/ynz-runtime/src/lib.rs:432-450`
**Severity**: LOW
**Category**: Logic Error

**Issue**: `if idx == start { return None }` after wrapping is correct for a fully-full map, but assumes no DELETED entries between probes. If the search hits DELETED, it must continue, and the loop does (because the early-return condition is on `idx == start`, not on `idx == start AND not yet visited`). The wraparound check returns None even if the desired key would have hit `CTRL_EMPTY` had we kept going — but if `idx == start` we've already visited start, so wraparound means we've checked every slot. This is correct, just subtle. No bug; flagging for review.

---

### Bug #16: `ynz_decimal_to_float` silent fallback to 0.0

**File**: `crates/ynz-runtime/src/lib.rs:851-859`
**Severity**: LOW
**Category**: Error Propagation Gap

**Issue**: `s.parse::<f64>().unwrap_or(0.0)` silently maps parse failure to 0.0. Should be impossible (the decimal128 formatter produces valid f64-parseable output), but if a future formatter change produces an unparseable string (e.g., overflow → "Infinity"), `unwrap_or(0.0)` silently returns wrong data. A debug_assert at minimum would catch this in dev builds.

---

---

## LOWER SEVERITY / DESIGN OBSERVATIONS

### Observation #18: Float-to-string buffer can truncate

**File**: `crates/ynz-runtime/src/lib.rs:171-184`

`ynz_float_to_string` uses a 32-byte buffer. Rust's `format!("{x}")` for some f64 values can produce strings longer than 31 bytes (e.g., very small subnormals printed in scientific notation with many digits). The truncation code prevents overflow but silently produces a wrong-but-null-terminated string. No memory bug, but printed output is wrong.

### Observation #19: `link_objects` temp library cleanup not in a finally-like pattern

**File**: `crates/ynz-driver/src/build.rs:182-237`

If the linker invocation panics between `std::fs::write(&rt_lib_tmp, ...)` (line 183) and `std::fs::remove_file(&rt_lib_tmp)` (line 213), the temp file leaks. Wrap in a guard struct (Drop impl) to ensure cleanup. Same issue in `build_single_file`.

### Observation #20: Generic vs non-generic function name collision not caught

**File**: `crates/ynz-typeck/src/signatures.rs:80-91`, `collect_generic_signatures` at line 180-188

`collect_signatures` checks for duplicate names within its own table. `collect_generic_signatures` checks for duplicates within ITS table. But a non-generic function named `foo` AND a generic function named `foo<T>` would NOT be flagged as duplicates — they live in separate tables. At call-site dispatch, the typeck may pick one consistently, but two functions with the same name in source is a UX bug worth flagging.

---

## Summary by Category

- Null/Undefined: 2 (Bug #7, observation #18)
- Types: 1 (Bug #13)
- Async: 0
- Logic: 4 (Bug #4, #6, #8, #11) — Bug #1 and #3 fixed by Batch 3
- Leaks: 2 (Observation #19, Bug #7 part 2)
- Isolation: 0
- Flow/Ordering: 0 (Bug #3 fixed)
- Salsa cache: 0 — Bug #1 (coarse PartialEq) and Bug #5 (disk reads) **FIXED by Batch 3**
- Edge case: 4 (Bug #8, #13, #14, #16)

## Prioritized Fix Order

### Must Fix Now (Production Risk)
1. **Bug #1** — `src/` preference contradicts spec. Silent file-skipping.

### Should Fix Soon (User Impact)
~~2. **Bug #2** — Map allocator OOM handling.~~ **FIXED (Batch 5b)**
~~3. **Bug #3** — Map set_str infinite loop risk.~~ **FIXED (Batch 5b)**
4. **Bug #4** — entrypoint→main rename collision across files.
5. **Bug #6** — Leading-underscore numeric literal accepted.
6. **Bug #7** — Misleading import-failure diagnostic.

### Nice to Fix (Code Quality)
7. **Bug #8** — i64::MIN parse rejection.
8. **Bug #9** — Dead-code fallback in `resolve_module_path`.
9. **Bug #10** — Debug-formatted mangle names.
10. **Bug #11** — `3.` accepted as float.

---

## What's Working Well

- The lexer's banned-jargon emission for `type`/`struct`/`class`/`interface`/`enum`/`abstract`/`async`/`await`/`promise`/`future`/`goroutine`/`pub`/`private`/`protected`/`public` is comprehensive and correctly redirects to Yinz vocabulary.
- The non-OOP invariant (methods are standalone functions, not inside shape bodies) appears enforced at the parser level.
- Diagnostics consistently follow the WHAT/WHAT-INSTEAD/WHY three-part teaching format from Golden Rule 11.
- Const-mutation checks exist at `check_assign`, `check_field_assign`, `check_index_assign`, and call-site `give`/`lend` argument enforcement.
- The lexer recovers from errors and emits diagnostics rather than panicking — good fail-soft behavior.
- The unsafe ABI shim layer in `ynz-runtime` consistently documents safety preconditions.
- SipHash key initialization is gated on a `OnceLock` for thread safety.
- The Pratt parser precedence table matches `spec/operators.md` (verified manually).
