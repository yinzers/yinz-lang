# Redundancy / DRY Analysis — Yinz Compiler

**Scope**: `crates/**/*.rs`
**Findings**: 6 patterns (~180 LOC consolidation opportunity)

---

## CRITICAL — Typeck test scaffolding duplicated across 7 files

- **Files**:
  - `crates/ynz-typeck/tests/check.rs:14-45` — `run`, `assert_clean`, `assert_errors`, `assert_warnings`
  - `crates/ynz-typeck/tests/errors_typeck.rs:17-61` — `run`, `assert_clean`, `assert_errors`, `assert_has_diagnostic`
  - `crates/ynz-typeck/tests/strings_typeck.rs:15-57` — `check`, `check_no_diags`, `check_diag_count`, `check_has_diag`
  - `crates/ynz-typeck/tests/generics_typeck.rs:15-41`
  - `crates/ynz-typeck/tests/iterables_typeck.rs:17-59`
  - `crates/ynz-typeck/tests/maps.rs:15-53`
  - `crates/ynz-typeck/tests/builtins.rs:15-53`
- **Pattern**: identical Salsa pipeline invocation (`CompilerDb::default()` → `SourceFile::new` → `check_query`) + structurally identical assertion helpers; names diverge (`run` vs `check`, `assert_clean` vs `check_no_diags`)
- **`check_has_diag` body verbatim across 4 files**:
  ```rust
  let found = out.diagnostics.iter().any(|d| {
      d.what.contains(fragment) || d.what_instead.contains(fragment) || d.why.contains(fragment)
  });
  ```
- **Fix**: `crates/ynz-typeck/tests/support.rs` (via `mod common;`) exporting `run_check`, `assert_no_diags`, `assert_error_count`, `assert_has_diag`. Move shared `const FILE` too.

## HIGH — Codegen `run_mN_codegen` pattern × 4

- **File**: `crates/ynz-codegen/tests/golden.rs:61-71, 191-200, 300-309, 374-387`
- **Pattern**: identical 6-line: create db, SourceFile, `codegen_query`, check diags, return artifact. Only the file-name and source constants differ.
- **Fix**: `fn run_codegen_for(file: &str, source: &str) -> Option<CompiledArtifact>` — call sites become one-liner wrappers.

~~## HIGH — `build_project` and `build_single_file` share warning-render block~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-driver/src/build.rs:151-163` and `:379-395`
- **Pattern**: filter non-errors → new `DiagnosticBucket` → push each warning → render
- **Fix**: `fn render_warnings(diags, sources) -> String`. Also: `build_failed` / `build_failed_diags` share `BuildResult` construction — can merge once.

~~## MEDIUM — "Unknown field" diagnostic block × 3 in check.rs~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-typeck/src/check.rs:2546-2563, 2621-2637, 2842-2864`
- **Pattern**: build `available` field list → `find_closest_name` → conditional "Did you mean" / "has these fields" what_instead → error diagnostic
- **Fix**: `fn emit_unknown_field_error(self, shape_name, field, available, span, context: FieldErrorContext)` — enum distinguishes access vs struct-literal wording.

~~## MEDIUM — `not_defined` diagnostic construction × 3~~ **FIXED (Batch 8)**

- **File**: `crates/ynz-typeck/src/check.rs:425-430, 1193-1207, 1274-1288`
- **Pattern**: candidates list (varies: scope only / scope+sigs / sigs+generics) → `find_closest_name` → "Did you mean" / fallback what_instead → "name is not defined" error
- **Fix**: `fn make_not_defined_diag(name, span, candidates, fallback) -> Diagnostic`.

## LOW — `Frame` / `SourceLoc` built-in shape field dispatch (2 instances, watch for 3rd)

- **File**: `crates/ynz-typeck/src/check.rs:2585-2601, 2603-2618`
- **Pattern**: match on field name → field type or unknown-field error
- **Below threshold (2x, need 3+)** — flag as watch-this. Extract when M7 P3a `ErrorValue` or similar adds the third copy. Table-driven `BuiltinShapeSpec { name, fields }` is the natural extraction.

---

## Already DRY (preserved)

- `Diagnostic::file_error/file_warning/file_suggestion` convenience constructors — added Batch 6.8; 21 call sites may migrate over time
- `Diagnostic::error/warning/suggestion` constructors — centralized; panic-on-empty enforces all three parts
- `find_closest_name` / `levenshtein` — single definition (`check.rs:3662-3700`), 6 call sites
- `type_name`, `PrimitiveIntrinsicTable`, `array_method_return` etc. — single source of truth
- `build_failed` vs `build_failed_diags` split is legitimate (different source-map assembly)

---

## Priority

1. **Phase 1**: typeck test scaffolding → shared `tests/support.rs` (zero-risk, saves drift across 7 files)
2. **Phase 2**: codegen `run_mN_codegen` helper + driver `render_warnings` helper
3. **Phase 3**: typeck `emit_unknown_field_error` + `make_not_defined_diag`
