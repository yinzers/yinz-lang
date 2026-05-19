# Cross-Crate Consolidation Report — Yinz Compiler Workspace

**Analyzed**: 2026-05-19
**Scope**: `crates/**/*.rs` + `crates/*/Cargo.toml`
**Crates Identified**: 8 — ynz-ast, ynz-codegen, ynz-diagnostics, ynz-driver, ynz-numerics, ynz-parser, ynz-runtime, ynz-typeck
**Consolidation Opportunities Found**: 9

---

## MEDIUM — Duplicate `find_project_root` in two crates

**Type**: Shared Logic
**Crates Involved**: `ynz-driver`, `ynz-typeck`
**Current State**: Two near-identical implementations of "walk up from a path looking for `yinz.toml`":
- `crates/ynz-driver/src/load.rs:48` — `pub fn find_project_root(start: &Path) -> Option<PathBuf>` (handles `start.is_file()` distinction)
- `crates/ynz-typeck/src/resolve_import.rs:74` — `fn find_project_root(start: &Path) -> Option<PathBuf>` (private, assumes start is a directory)

The two behave subtly differently — the driver's version normalizes file→parent first; typeck's caller passes `Path::new(importer_path).parent()` instead. Same intent, two code paths, no shared test.
**Proposed Consolidation**: Move `find_project_root` (and `resolve_module_path`) into a new `ynz-project` crate, OR co-locate as a public helper in `ynz-driver::load` re-exported. Both call sites take a `&Path` and answer "where's the project root?" — there's no good reason for divergence.
**Trigger Condition**: When the third call site lands (likely `ynz doc`, `ynz fmt`, or LSP — any of these walk-up project discoveries are in the v0.2 backlog), OR sooner if a bug-fix to one implementation forces a manual sync of the other.
**Effort**: Low — extract one ~10 line helper to a shared location; both crates already share `ynz-diagnostics` so the dependency graph already permits it.
**Benefits**: One source of truth for project-root discovery semantics. Eliminates the file-vs-dir handling asymmetry as a future foot-gun.
**Risks**: None — pure refactor with no behavior change once the asymmetry is resolved deliberately.

---

## MEDIUM — Repeated typeck test helper boilerplate across 6 test files

**Type**: Shared Logic (test helpers)
**Crates Involved**: `ynz-typeck` (internal: 6 test files in one crate, but the dev-dep pattern is workspace-relevant)
**Current State**: The exact same three-helper bundle (`run` / `check` + `assert_clean` / `check_no_diags` + `assert_errors` / `check_diag_count`) is copy-pasted across:
- `crates/ynz-typeck/tests/check.rs`
- `crates/ynz-typeck/tests/builtins.rs`
- `crates/ynz-typeck/tests/strings_typeck.rs`
- `crates/ynz-typeck/tests/maps.rs`
- `crates/ynz-typeck/tests/errors_typeck.rs`
- `crates/ynz-typeck/tests/generics_typeck.rs`
- `crates/ynz-typeck/tests/iterables_typeck.rs`

Each declares `const FILE: &str = "test.ynz";`, opens a `CompilerDb`, calls `check_query`, then provides a "must be clean" assertion and a "must have N diagnostics" assertion. Naming drifts (`run` vs `check`; `assert_clean` vs `check_no_diags`) but semantics are identical.

A similar but lighter pattern repeats in `crates/ynz-parser/tests/parse.rs` and `crates/ynz-codegen/tests/golden.rs`.
**Proposed Consolidation**: Add a `ynz-test-support` (or `ynz-test-utils`) dev-dep crate with:
- `pub fn check_source(source: &str) -> CheckOutput` (one-shot end-to-end typeck on `test.ynz`)
- `pub fn assert_clean(out: &CheckOutput)` / `pub fn assert_error_count(out: &CheckOutput, expected: usize)`
- `pub fn parse_source(source: &str) -> ParseOutput`
- `pub const TEST_FILE: &str = "test.ynz";`

All test-side crates list it as a `[dev-dependencies]` entry.
**Trigger Condition**: When the next test file lands (M9+ stdlib milestones will add per-module test files following the same template), OR when the naming drift (`run` vs `check`) bites someone on a refactor.
**Effort**: Low — new dev-only crate, ~50 lines of helpers, sed update each test file's `use`.
**Benefits**: Single canonical "drive a compile" entry point. New test files take 3 lines instead of 30. Eliminates the `run` vs `check` naming drift. Future changes to the CompilerDb construction pattern propagate from one place.
**Risks**: Dev-dep crates can feel like overhead in a small workspace. Mitigated by the fact this codebase already has 8 crates — one more is structurally fine.

---

## MEDIUM — `collect_options` / `collect_shapes` / `collect_signatures` invoked redundantly across crates

**Type**: Shared Data Source (parallel access patterns to AST analysis)
**Crates Involved**: `ynz-typeck`, `ynz-codegen`
**Current State**: The signature/shape/options collection passes run from THREE different call sites for the same module:

1. `crates/ynz-typeck/src/queries.rs:123-132` (`module_signatures_query`) — the salsa-tracked happy path
2. `crates/ynz-typeck/src/check.rs:52` (`check`) — re-runs `collect_options` inside check, separate from the query that fed it
3. `crates/ynz-codegen/src/emit.rs:141-143` (`emit_artifact`) — re-runs `collect_options` AGAIN at codegen for variant tag lookups
4. `crates/ynz-typeck/src/resolve_import.rs:314-316` — re-runs all three passes for every imported file (uncached — there's a `dummy_diags` bucket that swallows diagnostics)

(1) is correct. (2), (3), (4) are recomputation. The result of (1) is already plumbed into `CheckOutput` and `CodegenOutput` via `sig_output`, but `collect_options` is called separately inside check.rs and emit.rs because the OptionsTable wasn't part of the SignatureOutput contract.
**Proposed Consolidation**: Promote `OptionsTable` into `SignatureOutput` (it's already in scope — `imported_options` is there). Have `check()` and `emit_artifact()` consume it from the query result instead of re-running `collect_options`. For (4), the cross-file resolution should call `module_signatures_query` on the imported `SourceFile` instead of running `collect_shapes` / `collect_options` / `collect_signatures` directly — that's exactly what salsa is designed for and `resolve_import.rs` already has the db handle.
**Trigger Condition**: When a project with N imported files starts compiling noticeably slower than `ynz` should be (the `dummy_diags` swallowing in `load_export_table` already signals "we know this is wrong"), OR when adding a new pass (e.g., M9 stdlib type registration) tempts a fourth call site.
**Effort**: Medium — needs `OptionsTable` PartialEq impl for salsa (it derives `Default` but the table isn't `PartialEq`-derivable through HashMap). The cross-file salsa call already has TODOs comments at `queries.rs:118` and `resolve_import.rs:275` flagging this same issue.
**Benefits**: Correct salsa caching of cross-file imports (currently uncached per the @design-decision comment at `shapes.rs:67`). Eliminates the redundant codegen-side options collection. Faster recompiles for multi-file projects.
**Risks**: Touches the central type-collection contract — requires the SignatureOutput to expand. Coordination with M9 stdlib work.

---

## MEDIUM — `Diagnostic` builder vs raw constructor — repetitive "file-level / no-span" diagnostics

**Type**: Shared Logic (diagnostic construction)
**Crates Involved**: `ynz-driver`, `ynz-typeck`, `ynz-codegen` (consumers); `ynz-diagnostics` (provider)
**Current State**: 21 call sites across 6 files use the pattern `Diagnostic::error(SourceSpan::new(<path>, 0, 0), <what>, <what_instead>, <why>)` for "I don't have a real span; point at the file." Heavily concentrated:
- `crates/ynz-driver/src/build.rs` — 12 occurrences (linker errors, IO errors, missing-source errors)
- `crates/ynz-driver/src/load.rs` — 4 occurrences (file-read errors, UTF-8 errors, yinz.toml errors)
- `crates/ynz-typeck/src/options_table.rs`, `check.rs`, `shapes.rs` — file-level diagnostics
- `crates/ynz-codegen/src/queries.rs` — 1 (LLVM backend failure)

The `Diagnostic::new/error/warning/suggestion` API takes 4-5 arguments positionally. There's no convenience for the common "file-level error with no span" case, and no `.with_path(path)` builder. A typo in the position-by-position arg order silently swaps `what` and `what_instead`.
**Proposed Consolidation**: Add `Diagnostic::file_error(path: &str, what, what_instead, why)` to `ynz-diagnostics` (and equivalents for `file_warning`, `file_suggestion`). Optionally add a fluent builder: `Diagnostic::build().error().at_file(path).what(...).what_instead(...).why(...).finish()`. The four-arg positional API stays for the case where a real span exists.
**Trigger Condition**: When the next linker / IO / driver-level diagnostic gets the arg order wrong in a code review, OR when a "Diagnostic: shape mismatch where we should point at the import statement but currently point at byte 0" bug ships.
**Effort**: Low — purely additive in `ynz-diagnostics`. No call site MUST migrate; existing constructors continue to work.
**Benefits**: Reduces 21 verbose call sites to 21 shorter ones. Eliminates the "0, 0" magic numbers as a code smell.
**Risks**: Adds API surface area. Mitigated by additive-only change.

---

## MEDIUM — Two LLVM-type-for-Yinz-type implementations in codegen

**Type**: Shared Logic (within one crate, but structurally parallel)
**Crates Involved**: `ynz-codegen` (internal: `emit.rs` has two)
**Current State**: `crates/ynz-codegen/src/emit.rs` declares two LLVM type lookup functions:
- `fn llvm_type_for_ctx<'ctx>(ctx, ty) -> Option<BasicTypeEnum<'ctx>>` (free function, line 316) — handles 5 Yinz types and falls through to `ptr` for everything else
- `fn llvm_type_for(&self, ty) -> Option<BasicTypeEnum<'ctx>>` (method on `Cg`, line 618) — handles 13+ Yinz types explicitly, calls `resolve_type` first

`llvm_type_for_ctx` is the "simple" version used during pass 1.5 (forward declaration of monomorphized generic functions) before a `Cg` exists. The method version is used inside lowering. They MUST stay in sync — any new Type variant (e.g., a future `BuiltinSet`) needs both updated, but the compiler won't catch a missed one because the free-function falls through to `ptr.into()`.
**Proposed Consolidation**: Replace the free function's body with `LlvmTypeMap::lookup(ctx, ty)` — a static lookup helper that doesn't need a `Cg`. Have the method version just call the same helper plus the `resolve_type` pre-step. The fall-through-to-ptr behavior in the free version becomes explicit in the helper.
**Trigger Condition**: When M9+ adds a new built-in collection type and the dev forgets to update the free function's match. (The fall-through means the bug is silent — generic monomorphization will use a `ptr` ABI for a type that should be unboxed.)
**Effort**: Low — pure refactor inside one file. No cross-crate change.
**Benefits**: Single source of truth for "what's this Yinz type's LLVM representation." The next Type variant gets caught by one match arm, not two.
**Risks**: Minimal — the two functions are already required to behave identically on the types they share.

---

## LOW — `Vec<Diagnostic>` ⇄ `DiagnosticBucket` round-tripping

**Type**: Shared Infrastructure (data representation)
**Crates Involved**: `ynz-parser`, `ynz-typeck`, `ynz-codegen`, `ynz-driver`
**Current State**: Salsa requires query outputs to be `Clone + PartialEq`. `DiagnosticBucket` is neither, so every query output stores `diagnostics: Vec<Diagnostic>`. But every PRODUCER inside the query first collects into a `DiagnosticBucket` (for the 50-error cap and the `truncate()` rollback feature used by the parser's contextual `<` disambiguation). The pattern:

```
let mut bucket = DiagnosticBucket::new();   // build phase
// ... bucket.push(...) ...
let diagnostics: Vec<Diagnostic> = bucket.into_iter().collect();  // commit phase
```

repeats in `parse_query`, `lex_query`, `module_signatures_query`, `check_query`, `codegen_query`. Driver's `build.rs` then converts BACK by `diags.push(d.clone())` into a fresh bucket for rendering. The `hidden_count` value (errors past the 50-cap) is silently dropped on every Vec round-trip because Vec<Diagnostic> has nowhere to store it.

This is the bug surface: a project with 50+ errors in lex+parse+typeck combined will under-report the hidden count because each query truncates its own bucket and the Vec round-trip loses the count.
**Proposed Consolidation**: Either (a) make `DiagnosticBucket: Clone + PartialEq` so queries can store it directly (PartialEq can compare diagnostic vectors structurally; Clone is cheap), or (b) add a `DiagnosticsWithHiddenCount` wrapper type that's the salsa output shape and carries both the Vec and the hidden count. The driver's rendering path then doesn't need to rebuild a bucket — it consumes the salsa output directly.
**Trigger Condition**: When a user reports "I have 80 errors but only see 50 — the count message says 0 hidden" (the current bug), OR when adding the LSP `publishDiagnostics` plumbing in v0.2 forces a clean diagnostic flow.
**Effort**: Low (option a — add derives) to Medium (option b — define new wrapper type and update query signatures).
**Benefits**: Correct `hidden_count` reporting across compilation phases. Eliminates the round-trip ceremony at every query.
**Risks**: PartialEq on DiagnosticBucket affects salsa cache invalidation — must verify it does the right thing for incremental rebuilds.

---

## LOW — Linker invocation and runtime-lib extraction duplicated between `build_single_file` and `build_project`

**Type**: Shared Logic
**Crates Involved**: `ynz-driver` (internal: one crate, two functions)
**Current State**: `crates/ynz-driver/src/build.rs:175-237` (`link_objects`) and `crates/ynz-driver/src/build.rs:309-373` (inside `build_single_file`) BOTH:
- Write `RUNTIME_LIB_BYTES` to a temp path
- Call `find_linker()` and emit the same "no linker found" diagnostic
- Invoke `Command::new(linker).arg(obj).arg(rt_lib_tmp).arg("-no-pie").arg("-o").arg(binary)`
- Emit the same "linker failed to start" / "linker failed" diagnostics
- Clean up the temp file

`build_project` refactored this into a helper (`link_objects`); `build_single_file` didn't follow suit and still inlines the whole sequence. Six diagnostic emit-sites are duplicated verbatim.
**Proposed Consolidation**: Have `build_single_file` call `link_objects(&[obj_path], ...)` with a single-object vec. The helper already exists.
**Trigger Condition**: When the linker handling changes (e.g., when codegen switches to PIC relocations per the `-no-pie` comment at line 341, OR when M9+ adds linker flags for stdlib libraries), the duplication forces a 2x edit.
**Effort**: Low — local refactor inside one file.
**Benefits**: One linker-invocation code path. Fix-once for linker-flag changes.
**Risks**: None — pure refactor with existing helper.

---

## LOW — `find_closest_name` / `levenshtein` is private-to-typeck but conceptually crate-shared

**Type**: Shared Logic
**Crates Involved**: `ynz-typeck` (current); future: `ynz-parser`, `ynz-codegen`, LSP crate
**Current State**: `crates/ynz-typeck/src/check.rs:3663` defines `pub fn find_closest_name` and `fn levenshtein`. Used only inside `check.rs`. The parser's "unexpected `{`" diagnostics could plausibly use it for keyword suggestions (e.g., `funtion` → `function`) but currently can't because it lives in typeck. The LSP layer (v0.2) will need it for completion ranking against in-scope identifiers.
**Proposed Consolidation**: Move `find_closest_name` + `levenshtein` to `ynz-diagnostics::suggest` (or a new tiny `ynz-text-utils` crate). Both typeck and parser can then call it. Keep the threshold heuristic (`len 0-2 → 0, 3-4 → 1, else 2`) co-located.
**Trigger Condition**: When the parser-level "did you mean a keyword?" diagnostic lands (high-impact UX win per Golden Rule 11; deferred since M3), OR when LSP work begins in v0.2.
**Effort**: Low — extract ~30 lines, update one `use` in check.rs.
**Benefits**: Levenshtein-based suggestions become available crate-wide. Parser can emit "did you mean `function`?" for typos.
**Risks**: None — pure refactor.

---

## LOW — Workspace `Cargo.toml` lists `unicode-segmentation` as a workspace dep but `ynz-runtime` pins concrete versions independently

**Type**: Shared Dependency
**Crates Involved**: `ynz-runtime` (uses `workspace = true`); workspace root (defines `unicode-segmentation = "1.13.2"`)
**Current State**: Workspace root at `Cargo.toml:21-29` declares `unicode-segmentation = "1.13.2"` as a workspace dep. `ynz-runtime/Cargo.toml` pins three other unicode crates with `=` exact-version pins NOT in the workspace deps:
- `simdutf8 = "=0.1.4"`
- `unicode-normalization = "=0.1.24"`
- `memchr = "=2.7.4"`
- `unicase = "=2.7.0"`

These are runtime-bridge deps for M7 string operations. They're only used by `ynz-runtime` today, so the omission is correct in YAGNI terms. BUT: the stdlib v0.6 file-system module will likely need `memchr` for line/byte scanning, and v0.14 regex will need `unicode-normalization`. When that happens, two crates will pin the same deps independently and they can drift.
**Proposed Consolidation**: Promote the four `=`-pinned crates to `[workspace.dependencies]` now, even though only one crate uses them today. Future stdlib crates inherit the lock.
**Trigger Condition**: When the second crate (likely the v0.6 file-system stdlib crate or v0.14 regex) needs one of these. Cheap to do preemptively.
**Effort**: Low — five lines moved from one Cargo.toml to another.
**Benefits**: Single version source for unicode/byte-scan deps across the workspace. Future crates inherit the `=`-exact-pin convention.
**Risks**: None.

---

## Crate Dependency Map

```
ynz-driver
  ├── ynz-codegen ──────────────┐
  ├── ynz-typeck                │
  ├── ynz-parser                │
  ├── ynz-ast                   │
  └── ynz-diagnostics           │
                                │
ynz-codegen ────────────────────┘
  ├── ynz-typeck
  ├── ynz-parser
  ├── ynz-ast
  ├── ynz-numerics  (also used directly by ynz-runtime)
  ├── ynz-diagnostics
  └── inkwell (LLVM)

ynz-typeck
  ├── ynz-parser
  ├── ynz-ast
  └── ynz-diagnostics

ynz-parser
  ├── ynz-ast
  └── ynz-diagnostics

ynz-ast
  └── ynz-diagnostics

ynz-runtime
  └── ynz-numerics  (only — no diagnostics; it's the C-ABI runtime)

ynz-numerics
  (no internal deps — IBM Hursley conformance corpus only)

ynz-diagnostics
  └── ariadne  (terminal rendering)
```

Shared-data flows:
- **SourceSpan** travels from parser → ast → typeck → codegen → diagnostics, threaded through Vec<Diagnostic> at each query boundary
- **Module/AST nodes** travel parser → typeck → codegen
- **SignatureTable / ShapeTable / OptionsTable** built in typeck, consumed in codegen (the `module_signatures_query` redundancy in opportunity #3 is here)
- **Diagnostic** flows from every crate INTO the driver, which rebuilds a bucket for rendering

---

## Priority

### Quick Wins (low effort, clear benefit)

1. **Linker dedup in `ynz-driver/src/build.rs`** — `build_single_file` calls existing `link_objects`. Pure refactor, no API change. (Opportunity #7)
2. **Extract `find_project_root` and `resolve_module_path`** to a shared helper. ~20 LOC moved. (Opportunity #1)
3. **Two LLVM type lookups → one in `ynz-codegen`**. Internal refactor, no cross-crate API change. (Opportunity #5)
4. **Promote runtime's unicode pins to `[workspace.dependencies]`**. 5 lines moved. (Opportunity #9)

### Strategic (medium effort, do when triggered)

5. **Eliminate redundant `collect_options` / `collect_shapes` / `collect_signatures` calls** — SignatureOutput grows to hold OptionsTable; cross-file resolution uses `module_signatures_query` instead of re-running passes outside salsa. Real perf win and fixes the @design-decision comment at `shapes.rs:67-75`. (Opportunity #3)
6. **`ynz-test-support` dev-dep crate** for shared typeck test helpers — pays off when the next milestone adds 3+ more test files. (Opportunity #2)
7. **`Diagnostic::file_error()` convenience** — additive, no migration required. (Opportunity #4)

### Future State (high effort, revisit later)

8. **Hidden-count preservation across query boundaries** — needs either `DiagnosticBucket: Clone + PartialEq` (verify salsa invalidation) or a new wrapper type threading through queries. Tied to v0.2 LSP work. (Opportunity #6)
9. **Move `find_closest_name`/`levenshtein` to a shared place** — only valuable when the parser starts emitting did-you-mean diagnostics or LSP completion ranking lands. (Opportunity #8)

---

## What's Already Well-Shared

- **`ynz-diagnostics` as a hub** — every crate that emits errors depends on it; the WHAT/WHAT-INSTEAD/WHY format is enforced in one place (`diagnostic.rs:39-72` panics on empty fields). This is exactly right.
- **`ynz-ast` as a pure data crate** — only depends on `ynz-diagnostics` for `SourceSpan`. AST nodes are constructed by parser and consumed by every downstream crate without redefinition.
- **Salsa db (`CompilerDb`) lives in `ynz-parser`** and is the single source for the query graph. Downstream crates (`ynz-typeck`, `ynz-codegen`) define their own queries that depend on parser's `parse_query` via `salsa::Database` trait object. No parallel-db drift.
- **`ynz-numerics` deliberately has zero runtime deps** per its Cargo.toml comment — correct for a decimal128 conformance crate that must stay reproducible against the IBM Hursley corpus.
- **`ynz-runtime` cleanly isolates the C-ABI surface** — only depends on `ynz-numerics`, no link to anything compiler-side. Correct boundary.
- **Workspace dep declarations for `salsa`, `inkwell`, `ariadne`, `insta`, `clap`** at root — prevents the cross-crate version drift that's the topic of Opportunity #9.
- **`SourceSpan` and `Spanned<T>` live in `ynz-diagnostics` and `ynz-parser` respectively** — span data structure has one definition, even though spans are constructed at many sites.
