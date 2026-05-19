# Forensics / Observability Analysis — Yinz Compiler

**Scope**: Can a contributor reconstruct what decision the compiler made, on what inputs, with what outcome, from compiler output alone?

**Findings**: 11 (Critical: 2, High: 4, Medium: 3, Low: 2) — Batch 5b fixed: High bignum_binop, Medium SipHash try_into

---

## CRITICAL — No ICE distinction: compiler panics look identical to user errors

- **File**: `crates/ynz-driver/src/build.rs:30-47` (entry); panics from `crates/ynz-diagnostics/src/render.rs:85`, `crates/ynz-codegen/src/emit.rs:150`, `crates/ynz-parser/src/lexer.rs:469`
- **Gap**: Compiler panics produce raw Rust panic strings to stderr with no "this is a compiler bug, please file an issue" framing. User-side errors route through the formatted diagnostic pipeline; panics bypass it entirely. `codegen_query` (`crates/ynz-codegen/src/queries.rs:57-62`) catches LLVM errors with bug-report framing, but no other stage does.
- **2 AM impact**: A bug report says "the compiler crashed." Maintainer cannot tell stage, input data, or path from the panic message alone. `RUST_BACKTRACE=1` is not the user's default.

## CRITICAL — LLVM IR generated on every build but inaccessible from CLI

- **File**: `crates/ynz-codegen/src/artifact.rs:10` (`ir_text` field), `crates/ynz-codegen/src/emit.rs:100`, `crates/ynz-driver/src/build.rs:96-172` (field never read by driver)
- **Gap**: `CompiledArtifact.ir_text` is populated every successful compile and silently discarded. No `--emit-ir` / `--print-ir` / `--dump-llvm` / `--verbose` CLI flag. Tests use it (`crates/ynz-codegen/tests/golden.rs` via `insta` snapshots) but production CLI cannot.
- **2 AM impact**: "Wrong output at runtime" → maintainer must recompile the compiler with a patch or run a test by hand to inspect IR. Every codegen investigation requires source instrumentation.

## HIGH — `.expect()` calls in production paths panic without source span

- **Files**:
  - `crates/ynz-diagnostics/src/render.rs:85,90,100,103` — `ariadne render failed`, footer writes, UTF-8 conversion
  - `crates/ynz-codegen/src/emit.rs:150` — `decimal zero parse` (currently infallible but unprotected)
  - `crates/ynz-driver/src/run.rs:19` — `success implies binary is set` (undocumented cross-struct invariant)
  - `crates/ynz-parser/src/lexer.rs:469,866,935,976,1003,1058,1073` — `digits are ASCII` / `identifier is UTF-8` (implicit invariant from `load.rs`)
- **Gap**: Each `.expect()` is a hidden panic path. No diagnostic bucket entry, no span context, no clue which source file was being processed.

## HIGH — Runtime overflow / div-zero panics lack source location

- **File**: `crates/ynz-runtime/src/lib.rs:115-127` (`ynz_panic_overflow`), `:134-143` (`ynz_panic_div_by_zero`)
- **Gap**: Operation name passed (e.g., `int overflow in '+'`), source file/line/column not. The design doc envisions `RUNTIME ERROR: integer overflow at line 12` (`design/compiler-errors.md:161`); reality is just the operator name.
- **2 AM impact**: "My program crashed with overflow." User can't bisect 15 addition sites without adding prints.

## HIGH — Salsa panic guard maintained only by code discipline

- **File**: `crates/ynz-typeck/src/resolve_import.rs:274` and `crates/ynz-typeck/src/queries.rs:116` (comments only)
- **Gap**: "Cannot change database mid-query" salsa panic — avoided by passing the same `db` everywhere, but no structural / type-system guarantee. New contributor adds a second `Database::new()` inside a query → opaque salsa crash.

~~## HIGH — `bignum_binop` silent zero-fallback on CString null-byte injection~~ **FIXED (Batch 5b)**

`CString::new(s)` failure now aborts with an INTERNAL ERROR message pointing to the issue tracker instead of silently returning `"0"`.

## MEDIUM — `RUST_BACKTRACE` documented nowhere user-facing

- **File**: `.github/workflows/ci.yml:11` only
- **Gap**: Feedback footer in `crates/ynz-diagnostics/src/render.rs:96-101` says "open an issue" but does not say "include `RUST_BACKTRACE=1` output."

## MEDIUM — No error codes / `--explain ERR_CODE`

- **File**: `crates/ynz-driver/src/main.rs:19-31`; `crates/ynz-diagnostics/src/diagnostic.rs` (no `code` field on `Diagnostic`)
- **Gap**: Cross-referencing bug-report text to source-of-truth requires grep across the diagnostics calls. No identifier ties a user-visible error to a specific diagnostic site.

~~## MEDIUM — SipHash `try_into().unwrap()` on infallible-but-undocumented slices~~ **FIXED (Batch 5b)**

Eliminated via direct array indexing (`[k[0], k[1], ...]`). No conversion step; no panic path.

## LOW — `decimal128/parse.rs:102` infallible `.unwrap()` with no safety comment

- **File**: `crates/ynz-numerics/src/decimal128/parse.rs:102`
- **Gap**: `tail.chars().next().unwrap().to_digit(10).unwrap()` — safe because `tail` is the >34-char remainder, but undocumented. A future tweak to `split_at` makes this a panic with no context.

## LOW — `codegen_query` error diagnostic pinned to span 0,0

- **File**: `crates/ynz-codegen/src/queries.rs:59`
- **Gap**: LLVM emit failure → diagnostic points at line 1 col 1 of source. Misleading caret; user wastes time looking at first line before realizing it's a backend error.

---

## Strengths (preserve)

- `Diagnostic::new()` panics on empty `what`/`what_instead`/`why` — structural enforcement of three-part format
- `codegen_query` catches LLVM errors with bug-report framing
- Linker stderr captured into diagnostic `why`
- `ynz_unhandled_error` runtime trace prints file+function+line for escaped errors
- `DiagnosticBucket` 50-error cap prevents console flooding
- `RUST_BACKTRACE=1` in CI
- `build.rs` uses `unwrap_or_else` on `canonicalize` to degrade gracefully

---

## Priorities

1. **ICE wrapper**: top-level `std::panic::set_hook` that prints "this is a compiler bug" + repro instructions + `RUST_BACKTRACE` hint
2. **`--emit-ir` flag**: trivial — `ir_text` already on `CompiledArtifact`
3. **`bignum_binop` silent-zero fallback** → at minimum log a diagnostic
4. **Source-location-aware runtime panics** (per `design/compiler-errors.md:161`)
5. **Audit `.expect()` calls** in render / runtime / lexer; convert to diagnostics or document the invariant
