# Forensics / Observability Analysis — Yinz Compiler

**Scope**: Can a contributor reconstruct what decision the compiler made, on what inputs, with what outcome, from compiler output alone?

**Findings**: 11 (Critical: 2, High: 4, Medium: 3, Low: 2) — Batch 5b fixed: High bignum_binop, Medium SipHash try_into. Batch 7 fixed: Critical ICE (7.1), Critical --emit-ir (7.2), High .expect() (7.3+7.7), High runtime panic location (7.4), Low codegen span (7.6).

---

~~## CRITICAL — No ICE distinction: compiler panics look identical to user errors~~ **FIXED (Batch 7.1)**

`std::panic::set_hook` installed at top of `main()`. Prints box-bordered ICE banner, panic message, location, issue URL, and `RUST_BACKTRACE=1` instruction. Exits with `EXIT_INFRA_ERROR`.

~~## CRITICAL — LLVM IR generated on every build but inaccessible from CLI~~ **FIXED (Batch 7.2)**

`--emit-ir` flag added to `ynz build` and `ynz run`. Writes `<binary>.ll`. `BuildResult.ir_text` carries the IR for single-file builds.

~~## HIGH — `.expect()` calls in production paths panic without source span~~ **FIXED (Batch 7.3 + 7.7)**

- `render.rs:85` — replaced `.expect()` with graceful degradation (writes fallback line, continues rendering).
- `lexer.rs` — `// Invariant:` comments document why each `.expect()` is safe (load.rs UTF-8 validation guarantee).

~~## HIGH — Runtime overflow / div-zero panics lack source location~~ **FIXED (Batch 7.4)**

`ynz_panic_overflow` and `ynz_panic_div_by_zero` signatures extended to `(op_name, file, line, col)`. Codegen emits `source_file` global (source path C string) + span byte offset + 0 col at each arithmetic panic call site. Runtime messages now show `at <file>:<offset>:0`.

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

~~## LOW — `codegen_query` error diagnostic pinned to span 0,0~~ **FIXED (Batch 7.6)**

`codegen_query` already uses `Diagnostic::file_error` (added in Batch 6.8), which emits no misleading span.

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
