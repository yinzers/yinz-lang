# Bug Analysis Report

**Analyzed**: 2026-05-19
**Files Checked**: ~35 (crates/**/*.rs, focused on parser, typeck, codegen, runtime, driver)
**Scope**: Yinz compiler — Rust crates that lex, parse, type-check, lower to LLVM IR, and link binaries

---

## CRITICAL BUGS (Fix Immediately)

### Bug #1: `check_index_assign` skips the `const`-deep-immutability check

**File**: `crates/ynz-typeck/src/check.rs:3058-3128`
**Severity**: CRITICAL
**Category**: Logic Error / Yinz invariant violation

**Issue**: `check_index_assign` does NOT call `root_binding_name` to verify the receiver isn't a `const` binding. Compare to `check_field_assign` at line 2917-2929 which DOES check. Result: this code compiles cleanly even though it violates the locked const-deep-immutability invariant:

```ts
const nums: array<int> = [1, 2, 3]
nums[0] = 5                          // should error — const blocks index-write — but compiles
```

The other two const-mutation sites (`check_assign` at line 455, `check_field_assign` at line 2919) DO emit the diagnostic. Only the index-assign path is missing it.

**Why This Breaks Production**: violates a documented Yinz language invariant (`.claude/rules/plan-invariants.md` `### Safety`: "const bindings cannot have their fields mutated" extended to deep immutability). A compiled Yinz program would silently mutate a value the user declared immutable. This is the textbook bug class the M4 invariants section exists to prevent — and it shipped past it for index-assign.

**Reproduction**:
```ynz
function entrypoint() -> nothing {
  const xs: array<int> = [1, 2, 3]
  xs[0] = 99      // currently compiles; should be a compile error
}
```

---

### Bug #2: Loose `PartialEq` on salsa-tracked outputs causes stale incremental compilation

**File**: `crates/ynz-typeck/src/queries.rs:32-89`, `crates/ynz-typeck/src/exports.rs:35-44`
**Severity**: CRITICAL
**Category**: Salsa cache invalidation

**Issue**: The `PartialEq` impls for `SignatureOutput`, `SignatureTable`, `ShapeTable`, `GenericFnTable`, `GenericShapeTable`, `MonomorphizationTable`, and `ExportTable` compare ONLY map sizes and keys — never the values. Example (`queries.rs:50-54`):

```rust
impl PartialEq for SignatureTable {
    fn eq(&self, other: &Self) -> bool {
        self.fns.len() == other.fns.len() && self.fns.keys().all(|k| other.fns.contains_key(k))
    }
}
```

Salsa uses `PartialEq` on a query output to decide whether downstream queries need to re-run. With this implementation, any edit that changes a function's parameter types, return type, ownership modifiers, or body — without renaming the function — produces an output that `==` the old output, so salsa SKIPS re-running `check_query` / `codegen_query`. The downstream queries return stale results — wrong types, wrong IR, wrong binary.

**Reproduction sketch**:
1. Build a project with `function foo(x: int) -> int { x + 1 }`.
2. Edit `foo` body to `x + 2` (same signature).
3. Edit again, change return type to `int` from `string` (same name).
4. Subsequent `ynz build` may emit IR using the old signature because `SignatureTable::eq` returns true (same names, same length).

**Why This Breaks Production**: incremental builds silently produce wrong binaries. This is a worst-case compiler bug class — the user sees "build succeeded" but the program runs old (or worse, half-old, half-new) code. The `ExportTable::PartialEq` comment even acknowledges the issue ("deferred to v0.2 LSP incremental caching") — but with this `PartialEq` shipping in production today, the cache is already broken.

**Fixed Code**: derive real PartialEq on the inner types (`FunctionSig`, `ShapeDef`, `OptionsEntry`) or use a content-hash invalidation strategy. Make these salsa outputs Hash + PartialEq over the actual contract, not just names.

---

### Bug #3: `flatten_inherited_fields` has order-dependent results from HashMap iteration

**File**: `crates/ynz-typeck/src/shapes.rs:498-532`
**Severity**: CRITICAL
**Category**: Logic Error / Non-determinism

**Issue**: When flattening inheritance chains (e.g., `C extends B extends A`), the function iterates `table.shapes` in HashMap iteration order — which is non-deterministic in Rust. The comment at line 505-506 acknowledges:

```rust
// Collect parent fields (may themselves be inherited — already flattened if
// we process in topological order, but for simplicity just grab what's there).
```

If iteration visits C before B, then C grabs B's NOT-yet-flattened fields (which lack A's fields). C ends up missing A's fields.

**Why This Breaks Production**:
1. Same source compiles to different binaries across runs (HashMap is randomized per-process).
2. Field access on inherited grandparent fields silently produces "field not found" errors OR (worse) wrong offsets at codegen if some paths happened to win the race.

**Reproduction sketch**:
```ynz
shape A { x: int }
shape B extends A { y: int }
shape C extends B { z: int }
// c.x access may or may not work depending on HashMap order
```

**Fix**: process shapes in topological order over the `extends` graph — children only flattened after their parent has been flattened.

---

### Bug #4: `load_project` still prefers `src/` despite recent "remove src/ convention" commit

**File**: `crates/ynz-driver/src/load.rs:164-172`
**Severity**: HIGH
**Category**: Logic Error / Inconsistency with commit intent

**Issue**: Doc comment says "no `src/` convention required" and recent commit `8440274` removed the requirement. The code still has:

```rust
let src_dir = root.join("src");
let walk_root = if src_dir.exists() { src_dir } else { root.to_path_buf() };
```

This means a project with BOTH a `src/` subdirectory AND `.ynz` files at the project root will only compile the files under `src/`. The project-root-level files are silently ignored. Also, `build.rs:67` still emits an error message hardcoded to `src/`: `"No `.ynz` source files found under `src/`."`

**Why This Breaks Production**: directly contradicts the documented spec (project-root-relative) that the commit was supposed to enforce. Silent file-skipping is among the worst compiler bug classes — users have no signal that files are being ignored.

---

### Bug #5: `resolve_imports` reads files from disk, bypassing the salsa source registry

**File**: `crates/ynz-typeck/src/resolve_import.rs:278-289`
**Severity**: HIGH
**Category**: Salsa cache invalidation / TOCTOU on file reads

**Issue**: When resolving an import, `load_export_table` does `std::fs::read_to_string(resolved_path)` directly, rather than looking up the file via `db.source_by_path`. The comment at lines 275-277 acknowledges this is incorrect ("v0.2 TODO: use the pre-registered SourceFile from the driver's project load via CompilerDb::source_by_path").

Two real bugs result:
1. **Salsa cache inconsistency**: file A's SourceFile may have updated text in salsa, but file B (which imports A) reads stale disk content during import resolution. Type-check uses two different versions of A simultaneously.
2. **TOCTOU**: between two queries in the same session, the file on disk may change. The driver and the import resolver see different contents.

**Why This Breaks Production**: in incremental builds (the entire point of using salsa), imports use one version of a module while the module's own queries use another. Result: type errors that don't exist, or invalid IR.

---

## HIGH SEVERITY BUGS

### Bug #6: `validate_underscores` misses leading underscore in hex/binary literals

**File**: `crates/ynz-parser/src/lexer.rs:1157-1172`, called from `lex_hex_int` (line 867) and `lex_binary_int` (line 936)
**Severity**: MEDIUM
**Category**: Logic Error

**Issue**: `validate_underscores` correctly catches double-underscores and trailing underscores, but not LEADING underscores. For hex/binary literals, the digit slice passed in starts immediately AFTER `0x`/`0b`, so a leading `_` (e.g., `0x_FF`) is a valid input to the validator that the validator silently accepts. The numeric parser then strips it and parses `FF` — producing a numeric literal that the spec presumably forbids.

```ynz
let bad = 0x_FF       // currently parses as 0xFF without error
let bad2 = 0b_1010    // same
```

Decimal literals (`1_000`) are not affected — the lexer enters `lex_decimal_number` only on a digit byte, so a leading `_` is impossible in that path.

---

### Bug #7: Memory leak / use-after-free on OOM in map runtime

**File**: `crates/ynz-runtime/src/lib.rs:405-423` (`map_alloc`), `534-545` (`order_push`)
**Severity**: HIGH
**Category**: Resource Leak / Null deref UB

**Issue**: Two related issues in the map runtime:

1. `map_alloc` (line 405) calls `malloc` 5 times with no null check. If any call returns null (OOM), `std::ptr::write_bytes(ctrl, CTRL_EMPTY, ...)` on line 412 writes through a null pointer — undefined behavior. Compare to `ynz_alloc` (line 250) which DOES check and abort.

2. `order_push` (line 537) does `realloc(...) as *mut i64; (*map).insert_order = new_order;` with no null check. If `realloc` returns null (OOM), the original buffer is still valid but `(*map).insert_order` is set to null, leaking the old buffer and breaking subsequent `*(*map).insert_order.add(i) = key` writes (UB).

**Why This Breaks Production**: a Yinz program that exhausts memory while inserting into a map gets a segfault or worse rather than the documented OOM-abort.

---

### Bug #8: `ynz_map_set_str` can infinite-loop on a full map

**File**: `crates/ynz-runtime/src/lib.rs:629-656`
**Severity**: HIGH
**Category**: Logic Error / Infinite Loop

**Issue**: After the growth check at line 630, line 648 has:

```rust
while *(*map).ctrl.add(idx) != CTRL_EMPTY && *(*map).ctrl.add(idx) != CTRL_DELETED {
    idx = (idx + 1) & (cap - 1);
}
```

There is no termination guard if every slot is occupied (no EMPTY or DELETED markers). The growth check at line 630 protects against this in the normal path (75% load factor forces growth), BUT if `map_grow_str` fails (e.g., OOM, sibling Bug #7) and silently returns without growing, the next probe hits 100%-occupied state and infinite-loops.

Same pattern in `find_insert_slot` (line 452-462): no full-map guard, only saved by the load-factor check.

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

### Bug #14: `lex_decimal_number` accepts `3.` as a valid number literal

**File**: `crates/ynz-parser/src/lexer.rs:996-1014`, `crates/ynz-runtime/src/lib.rs:1066-1071` (`is_valid_float_digits`)
**Severity**: LOW
**Category**: Logic Error / Spec inconsistency

**Issue**: `is_valid_float_digits` and the lexer accept `"3."` as a valid number. The lexer at line 991-994 only consumes the `.` when followed by a digit, so `3.toString()` lexes correctly. But the runtime parser `is_valid_float_digits` accepts the bare `"3."` — if a user passes `"3."` to `.toFloat()` at runtime, it succeeds because Rust's f64 parser accepts it. Spec inconsistency more than a real bug.

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

### Bug #17: `div_finite` exponent arithmetic in `div(0, finite)` may exceed encoding range

**File**: `crates/ynz-numerics/src/decimal128/ops.rs:86-88`
**Severity**: LOW
**Category**: Logic Error / Numeric edge case

**Issue**: When `av.is_zero()` and `bv` is finite-nonzero, the code returns:

```rust
return encode_finite(av.sign ^ bv.sign, av.exponent - bv.exponent, 0);
```

If `av.exponent = MIN_EXPONENT (-6176)` and `bv.exponent = MAX_EXPONENT (6111)`, then `av.exponent - bv.exponent = -12287`, which is out of range `[MIN_EXPONENT, MAX_EXPONENT]`. In debug builds, the assert at `encode_finite` line 142 panics; in release builds, the encoding silently produces garbage bits.

This is reachable only on input values at the encoding extremes — but extreme values are exactly where users expect arithmetic to fail GRACEFULLY (NaN or subnormal), not produce garbage.

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
- Logic: 6 (Bug #1, #3, #4, #6, #8, #11)
- Leaks: 2 (Observation #19, Bug #7 part 2)
- Isolation: 0
- Flow/Ordering: 1 (Bug #3)
- Salsa cache: 2 (Bug #2, #5)
- Edge case: 4 (Bug #9, #14, #15, #17)

## Prioritized Fix Order

### Must Fix Now (Production Risk)
1. **Bug #1** — const-deep-immutability bypass via index-assign. Documented invariant violation.
2. **Bug #2** — Salsa `PartialEq` cache bug. Silently wrong incremental builds.
3. **Bug #3** — HashMap-order-dependent inheritance flattening. Non-deterministic compilation.
4. **Bug #4** — `src/` preference contradicts spec. Silent file-skipping.
5. **Bug #5** — Import resolver bypasses salsa registry. Cross-file inconsistency.

### Should Fix Soon (User Impact)
6. **Bug #7** — Map allocator OOM handling.
7. **Bug #8** — Map set_str infinite loop risk.
8. **Bug #10** — entrypoint→main rename collision across files.
9. **Bug #6** — Leading-underscore numeric literal accepted.
10. **Bug #11** — Misleading import-failure diagnostic.

### Nice to Fix (Code Quality)
11. **Bug #9** — i64::MIN parse rejection.
12. **Bug #12** — Dead-code fallback in `resolve_module_path`.
13. **Bug #13** — Debug-formatted mangle names.
14. **Bug #14** — `3.` accepted as float.
15. **Bug #17** — div exponent overflow on extremes.

---

## What's Working Well

- The lexer's banned-jargon emission for `type`/`struct`/`class`/`interface`/`enum`/`abstract`/`async`/`await`/`promise`/`future`/`goroutine`/`pub`/`private`/`protected`/`public` is comprehensive and correctly redirects to Yinz vocabulary.
- The non-OOP invariant (methods are standalone functions, not inside shape bodies) appears enforced at the parser level.
- Diagnostics consistently follow the WHAT/WHAT-INSTEAD/WHY three-part teaching format from Golden Rule 11.
- Const-mutation checks (Bug #1 aside) exist at `check_assign`, `check_field_assign`, and call-site `give`/`lend` argument enforcement.
- The lexer recovers from errors and emits diagnostics rather than panicking — good fail-soft behavior.
- The unsafe ABI shim layer in `ynz-runtime` consistently documents safety preconditions.
- SipHash key initialization is gated on a `OnceLock` for thread safety.
- The Pratt parser precedence table matches `spec/operators.md` (verified manually).
