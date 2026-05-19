# Yinz Compiler Audit — 2026-05-19

Comprehensive code audit + doc-drift check across the entire workspace.

- **Analyzers run**: 13 (security, bugs, performance, reliability, adversarial, forensics, cleanup, redundancy, consolidation, consistency, documentation, ux, doc-drift)
- **Raw findings**: ~157 — combined here after dedup / conflict resolution
- **Cleanup + Consistency reports**: both empty — codebase is clean on those axes

Severity = execution order, not "whether to fix." Per Patrick's Rule 11, confirmed findings get fixed regardless of label.

---

## Severity Summary

| Severity | Count | Notes |
|---|---:|---|
| Critical | 7 | locked decisions violated / deterministic crashes / silent data corruption |
| High | 26 | shipped feature broken or misdocumented / large-impact UX / perf hotspots |
| Medium | 49 | drift, missed checks, minor UX |
| Low | 47 | polish, magic numbers, minor docs |
| Verified Correct | 9 | doc-drift positives — explicitly noted to prevent re-flagging |

---

## Dedup / Conflict Resolution Notes

- **Const-deep-immutability bypass** (Bug #1) — only flagged by bug analyzer; doc-drift confirms it as a documented M4 invariant.
- **Salsa cache invalidation gaps** — Bug #2 (PartialEq) **FIXED** + Bug #5 (import re-read) **FIXED** + Consolidation #6 (hidden_count loss) — Bug #2 and #5 resolved by Batch 3 (real PartialEq derive + `db.source_by_path` + memoized `module_signatures_query` calls). Consolidation #6 stays open (separate fix).
- **Concurrent build races** — Reliability #1/#3, Adversarial #9, Security #3 partial overlap → one grouped fix at `build.rs:294`.
- **Symlink loop** — Adversarial #4 (critical: infinite recursion) + Security #4 (medium: write primitive) → critical (infinite recursion is deterministic crash).
- **`DiagnosticBucket::push` O(n²)** — Performance Critical + Adversarial #12 (low). Performance assessment wins — even bounded by 50-cap, algorithm pattern is wrong AND `has_errors()` repeatedly pays the cost.
- **Stack overflow via deeply-nested expressions** — Adversarial #2 (critical) + Security #5 (medium) → critical (deterministic crash from a one-line `.ynz`).
- **NUL byte string truncation** — Adversarial #11 → critical (silent wrong-output on `print(x)`).
- **`src/` directory preference** — Bug #4 + Doc-drift #28 + Consolidation #1 (duplicate `find_project_root`) → **FIXED** (Batch 4b): `load.rs` now walks from project root directly; `examples/basics/` restructured to match spec; all affected fixtures updated.
- **`type` keyword leak** — UX critical + Doc-drift #36 + the snapshot fixture text → fix snapshot fixture AND `spec/collections.md`.
- **Error gallery commented-out triggers** — UX medium + Doc-drift #27 → single fix: restructure to auto-fire triggers (per `plan-invariants.md` `### Demo & Error Gallery`).
- **Performance / forensics overlap** — `has_errors()` O(n) is folded into the DiagnosticBucket Critical fix (`error_count` field eliminates both at once).
- **Priority hierarchy applied** — security wins over UX (no path-leak in error messages); correctness wins over perf (Bug #2 cache invalidation before perf items).

No genuine conflicts between analyzers.

---

## Phase 1 — CRITICAL (fix first; blocks downstream work)

### 1. Snapshot fixture + `spec/collections.md` recommend banned `type` keyword
- **Files**: `crates/ynz-diagnostics/tests/snapshots.rs:88-92` + `__snapshots/suggestion_only.snap`; `spec/collections.md:166, 248, 269`
- **Issue**: Compiler-shipped suggestion text says "Consider using a `type` instead of a `map`". User writes `type Foo { ... }` and gets a banned-keyword error from the lexer. Compiler contradicts itself.
- **Fix**: Replace `type` → `shape` in both the snapshot fixture and the spec page.

### 3. No ICE distinction — compiler panics look identical to user errors
- **File**: `crates/ynz-driver/src/main.rs` (top-level), panics across `crates/ynz-diagnostics/src/render.rs:85`, `crates/ynz-codegen/src/emit.rs:150`, `crates/ynz-parser/src/lexer.rs:469`
- **Issue**: Bug reports of "compiler crashed" arrive without source stage / input data / backtrace. Only `codegen_query` catches LLVM errors with bug-report framing.
- **Fix**: Top-level `std::panic::set_hook` printing "this is a compiler bug — please file at <URL> with `RUST_BACKTRACE=1` output." Use `EXIT_INFRA_ERROR = 101`.

### 4. LLVM IR generated every build but inaccessible from CLI
- **Files**: `crates/ynz-codegen/src/artifact.rs:10` (`ir_text` populated), `crates/ynz-driver/src/main.rs` (never read)
- **Issue**: Tests use `ir_text` via `insta` snapshots; production CLI has no `--emit-ir`/`--print-ir`/`--dump-llvm`. Every "wrong codegen" investigation requires recompiling the compiler.
- **Fix**: Add `--emit-ir` flag on `ynz build`. Trivial — field already populated.

### 5. Stack overflow via deeply-nested expressions (compiler DoS)
- **Files**: `crates/ynz-parser/src/parser.rs:2132` (`parse_expr`), `crates/ynz-typeck/src/check.rs` (`check_expr`/`infer_expr` family), `crates/ynz-codegen/src/emit.rs:2137` (`lower_expr`)
- **Issue**: Type recursion capped at 16; expression and statement recursion unbounded. `((((...((x))...))))` of ~50k depth deterministically crashes the compiler with SIGABRT.
- **Fix**: Add depth budget to `parse_expr` (256 limit), mirror in `check_expr` / `lower_expr`. Same teaching pattern as existing type-depth diagnostic.

### ~~6. Symlink loop in project tree → infinite recursion~~ FIXED (Batch 4b)
- **File**: `crates/ynz-driver/src/load.rs`
- `symlink_metadata()` instead of `is_dir()` prevents symlinks to dirs from being followed. Canonical-path `HashSet` detects hard-link cycles. Teaching diagnostic on detection.

### 7. NUL byte in string literal → silent runtime truncation
- **Files**: `crates/ynz-parser/src/lexer.rs:782-784` (`\0` escape accepted), `crates/ynz-codegen/src/emit.rs:2850-2851` (emits as C string)
- **Issue**: `` `hello\0world` `` compiles; `print(x)` outputs `hello`. Silent data loss on string round-trip.
- **Fix**: Either reject `\0` in string literals at the lexer, OR represent strings as `{ptr, len}` slices end-to-end (aligns with `design/strings.md`).

### 8. `DiagnosticBucket::push` does O(n) error count per push
- **File**: `crates/ynz-diagnostics/src/bucket.rs:29-38`
- **Issue**: `.iter().filter(...).count()` on every error push. Bounded by the 50-cap but pattern is wrong; `has_errors()` (L43-47) also pays O(n) per call.
- **Fix**: Add `error_count: usize` field; increment on push, decrement in `truncate`. `has_errors()` becomes `self.error_count > 0`. O(n²) → O(1).

---

## Phase 2 — HIGH (substantial impact; fix after Critical)

### Compiler logic / correctness
~~10. `load_project` prefers `src/` despite recent commit removing it~~ **FIXED (Batch 4b)**
11. Map runtime `malloc`/`realloc` paths skip null-check → SIGSEGV on OOM — `crates/ynz-runtime/src/lib.rs:405-423, 534-545`
13. `ynz_map_set_str` infinite-loop on full map if growth silently fails — `crates/ynz-runtime/src/lib.rs:629-656`
14. `entrypoint → main` rename collides on multi-file projects — `crates/ynz-codegen/src/emit.rs:460-465, 815-819`
15. `bignum_binop` silent fallback to `"0"` on CString null-byte → wrong math result with no log — `crates/ynz-runtime/src/lib.rs:2292`
16. Runtime overflow / div-zero panics lack source location (design says line/col, code says only op name) — `crates/ynz-runtime/src/lib.rs:115-143`
17. `ariadne` render `.expect()` panics on out-of-range span (no diagnostic; raw panic) — `crates/ynz-diagnostics/src/render.rs:85`

### Security (after path-traversal fix from Critical-adjacent set)
18. Path traversal via mid-path `..` in import — `crates/ynz-typeck/src/resolve_import.rs:59,69`
~~19. Predictable temp filename for runtime lib (CWE-377 on shared CI)~~ **FIXED (Batch 4b)** — uses `tempfile::NamedTempFile`
~~20. Output binary clobber + TOCTOU between write and execute in `ynz run`~~ **FIXED (Batch 4b)** — `ynz run` writes binary to per-invocation `tempdir()`
21. Extremely long identifier (10MB) → unbounded heap — `crates/ynz-parser/src/lexer.rs:486-639` (cap identifier length, e.g. 1024)

### Reliability
~~22. `.o` file stranded when rt_lib write or linker probe fails~~ **FIXED (Batch 4b)** — `CleanupGuard` drop impl removes `.o` on any early return
~~23. Concurrent `ynz build` races on same `obj_path`~~ **FIXED (Batch 4b)** — all intermediates go into per-invocation `tempdir()`

### Performance
24. `levenshtein` allocates full O(m×n) matrix per candidate — `crates/ynz-typeck/src/check.rs:3683-3700` — rolling DP + byte-level scan
26. `detect_extends_cycles` uses `Vec.contains` on visited set (already inconsistent with `has_cycle` HashSet) — `crates/ynz-typeck/src/shapes.rs:474-495`
27. `render()` eagerly clones every source string + builds line tables → O(total_source_bytes) per render — `crates/ynz-diagnostics/src/render.rs:55-59`

### Doc-drift (HIGH — spec misleads users today)
28. Spec-wide double-quoted strings across 11+ files (`"foo"` rejected by M7 lexer) — `spec/types.md`, `modules.md`, `errors.md`, `options.md`, `ownership.md`, `unions.md`, `control-flow.md`, `main.md`, `scope.md`, `maybe.md`, `sensitive.md`
29. `spec/destructuring.md` documents object destructuring that doesn't parse — only `for ((k,v) in m)` exists
30. `spec/sensitive.md` uses `.toUpper()` and `.length` (real names: `.toUpperCase()`, `.count()`)
31. `banned_jargon.rs` missing 7+ entries from `design/compiler-errors.md` — `crates/ynz-diagnostics/src/banned_jargon.rs:21-75` vs `design/compiler-errors.md:31-64` (`lifetime`, `alias`, `trait`, `interface`, `remainder`, `associated type`, `implementation`, `precondition`, `postcondition`)
32. `spec/modules.md` re-export described as shipped — syntax parses but cross-file calls are v0.2 stubs
33. `spec/modules.md:253` "side-effect imports" example uses double quotes — wouldn't trigger documented error
34. `spec/modules.md:139-146` "alias collision" example uses invalid `import math as advancedMath from "..."` syntax
35. `spec/collections.md` uses banned `type` keyword in body + code examples — `:166, 248, 269`

### Documentation (HIGH)
36. 9 unsafe FFI map functions missing `# Safety` contracts — `crates/ynz-runtime/src/lib.rs:568, 583, 605, 629, 664, 674, 688, 706, 732`
37. `map_grow_int` / `map_grow_str` side-effect contract undocumented — `crates/ynz-runtime/src/lib.rs:464, 499` (×2 growth, 75% LF, slot pointers invalidated)

### UX (HIGH)
39. `check.rs:444` parameter-reassignment WHY references shipped M4 as future
40. `check.rs:1282-1287` "not defined" WHY is generic ("the program can't run")
41. Render output prefixes WHY with `Note: Why:` double label — `crates/ynz-diagnostics/src/render.rs:76`
42. `what_instead` used as caret label conflicts with prose-instruction content — `crates/ynz-diagnostics/src/render.rs:75`
~~43. Single exit code 1 for all failure modes~~ **FIXED (Batch 4b)** — `EXIT_COMPILE_ERROR=1`, `EXIT_INFRA_ERROR=2`
~~44. No success output from `ynz build`~~ **FIXED (Batch 4b)** — prints `Build succeeded: <path>` to stdout
45. No "did you mean?" for imports (`find_closest_name` exists but unused in import path) — `crates/ynz-typeck/src/resolve_import.rs:211-229`

### Redundancy (HIGH)
46. Typeck test scaffolding duplicate across 7 files — `crates/ynz-typeck/tests/*.rs`
47. Codegen `run_mN_codegen` pattern × 4 — `crates/ynz-codegen/tests/golden.rs:61-71, 191-200, 300-309, 374-387`
48. `build_project` / `build_single_file` share warning-render block — `crates/ynz-driver/src/build.rs:151-163, 379-395`

---

## Phase 3 — MEDIUM (drift, missed checks, minor UX)

### Bugs / logic
- `validate_underscores` misses leading `_` in hex/binary literals (`0x_FF` accepted) — `crates/ynz-parser/src/lexer.rs:1157-1172`
- `parse_string_to_int` rejects `i64::MIN` (overflow before negate) — `crates/ynz-runtime/src/lib.rs:1014-1030`
- `find_project_root` discrepancy between driver + typeck implementations — `crates/ynz-typeck/src/resolve_import.rs:110-112`
- `mangle_type` ambiguous Debug-format catch-all — `crates/ynz-codegen/src/emit.rs:292-311`

### Reliability
- Partial/corrupt binary left after linker crash — `crates/ynz-driver/src/build.rs:333-373, 139-171`
- Project `.o` files orphan on SIGKILL — `crates/ynz-driver/src/build.rs:115`

### Security
- Symlink walker writes `.o` into symlink-targeted dirs — `crates/ynz-driver/src/load.rs:193-220`
- Unbounded interpolation depth stack in lexer — `crates/ynz-parser/src/lexer.rs:27,746`
- Diagnostic discloses canonicalized paths (info leak via #21) — `crates/ynz-typeck/src/resolve_import.rs:283`

### Adversarial
- Multi-byte Unicode error cascade — one diagnostic per byte instead of per codepoint
- yinz.toml `entry` path traversal (currently inert) — `crates/ynz-driver/src/load.rs`
- yinz.toml unreadable → silent fallback to defaults
- Import resolver re-reads same files O(n·m) times — `resolve_import.rs:278-279`

### Forensics
- `RUST_BACKTRACE` not documented user-facing — feedback footer should include it
- No error codes / `--explain` mechanism
- SipHash `try_into().unwrap()` on infallible-but-undocumented slices — `crates/ynz-runtime/src/lib.rs:302-303, 336`

### Performance
- Lexer allocates `String` per identifier (`String::from`) — significant on large files
- Lexer allocates `String` per numeric literal to strip underscores

### Redundancy / Consolidation
- "Unknown field" diagnostic × 3 in check.rs
- `not_defined` diagnostic × 3 in check.rs
- `collect_options` invoked redundantly at 2 remaining sites: `check.rs:52` and `emit.rs:141-143` (the `resolve_import.rs` site was fixed by Batch 3 via `module_signatures_query` memoization; `collect_shapes` and `collect_signatures` are now memoized at those sites too)
- Two `llvm_type_for` lookup functions in `emit.rs` (silent fall-through to ptr)
- `Diagnostic::file_error()` convenience missing (21 verbose call sites)

### Documentation
- `build_module` 5-pass flow undocumented — `crates/ynz-codegen/src/emit.rs:129`
- `sha256` missing spec citation + complexity — `crates/ynz-codegen/src/artifact.rs:17`
- `Checker` struct field-group explanation missing — `crates/ynz-typeck/src/check.rs:87`
- 3 crate-level docs missing (`ynz-parser`, `ynz-codegen`, `ynz-typeck`)
- SipHash zero-key choice unjustified in runtime — `crates/ynz-runtime/src/lib.rs:300-360`

### UX
- Render footer (hidden-count + URL) written without ariadne styling
- `next()` return-type error mixes prose + signature in `what_instead`
- Error gallery files have most triggers commented out (`m7_errors.ynz`, `m8_errors.ynz`) — violates `plan-invariants.md`
- Circular import error doesn't show the cycle chain — `crates/ynz-typeck/src/resolve_import.rs:147`
- Linker stderr stuffed into `why` field — `crates/ynz-driver/src/build.rs:227-232`
- `.value` on unguarded `maybe<T>` lacks dedicated teaching diagnostic
- `ynz run` always deletes binary, no `--keep` flag

### Doc-drift (MEDIUM)
- `spec/operators.md` "Overloading" section describes a v1.0-deferred feature without flagging
- `test` keyword not actually reserved in lexer — locked decision drift
- `spec/main.md` mentions `cli.args()`, `process.exit()` — both are v0.8
- `spec/types.md` hidden-field auto-default contradicts demo file
- `spec/doc-comments.md` "/// only on exported" — implementation attaches to all
- `background` no handle-form rejection diagnostic — `examples/errors/m8_errors.ynz:64-68` says it should exist
- `spec/sensitive.md` describes `--reveal-sensitive` flag that's not in driver
- `design/decisions.md` "type aliases removed" ambiguous — union aliases ARE supported
- ~~`examples/basics/yinz.toml` uses `src/entrypoint.ynz`~~ **FIXED (Batch 4b)**
- `spec/main.md` + `spec/config.md` mix `main()` / `entrypoint()` — Yinz term is `entrypoint`
- `spec/modules.md` "stdlib no import needed" examples reference unshipped modules
- ~~`examples/basics/src/entrypoint.ynz` shadows `nums` and `score`~~ **FIXED (Batch 4b)** — files moved to project root; path references updated

---

## Phase 4 — LOW (polish, minor issues)

(Selected; full list in individual analyzer reports)

- `codegen_query` LLVM error diagnostic pinned to span (0,0) — misleading caret
- `decimal128/parse.rs:102` infallible `.unwrap()` without safety comment
- `find_closest_name`/`levenshtein` should move to shared crate (`ynz-diagnostics::suggest`) — currently only in typeck
- Workspace cargo.toml: promote `simdutf8`/`unicode-normalization`/`memchr`/`unicase` to `[workspace.dependencies]`
- `build_single_file` should call existing `link_objects` helper (linker invocation duplicated)
- `Vec<Diagnostic>` ⇄ `DiagnosticBucket` round-trip silently drops `hidden_count`
- `options_table::tag_for` returns `i8` but variants reach 255 — switch to `u8`
- `errors_result_type` ABI contract for pointer types undocumented
- `ExportTable::eq` coarse-equality risk documented on impl, not at field comparison
- Float-to-string buffer can truncate — `crates/ynz-runtime/src/lib.rs:171-184`
- `lex_decimal_number` accepts `3.` — spec inconsistency
- `find_slot` early-return subtle but correct
- `ynz_decimal_to_float` silent fallback to 0.0 — `crates/ynz-runtime/src/lib.rs:851-859`
- ~~`link_objects` temp lib cleanup not in finally-pattern~~ **FIXED (Batch 4b)** — `NamedTempFile` drop handles cleanup
- Generic vs non-generic function name collision not caught — `crates/ynz-typeck/src/signatures.rs:80-91`
- 8 magic-number rename opportunities in `ynz-runtime`
- 6 documentation-naming improvements (sha-256 state vars, runtime capacity constants)
- `Lexer` struct three-mode state machine undocumented
- `parse_toml_string` parameter contract undocumented
- Spec `overview.md` says "12 Golden Rules" — should be 13
- `spec/iterables.md` under-documents `range(end)` one-arg form
- `spec/numeric-types.md` IDE-hint examples need "v0.2 (IDE)" tag
- `mvp-scope.md` "custom iterables v1.0" stale — shipped in M7
- `f32` deferred to v2 but no teaching diagnostic when user writes `let x: f32`
- `spec/sensitive.md` `env.get()` example references v0.8 module

---

## Cleanup / Consistency — 0 findings (preserve)

Both analyzers reported the codebase as exceptionally clean:
- No dead code / orphaned files / unused imports / commented-out blocks / `#[allow(dead_code)]`
- No code-level TODOs
- Diagnostics consistently follow WHAT/WHAT-INSTEAD/WHY format (mechanically enforced)
- `jargon_audit.rs` mechanically catches programmer-jargon at CI
- File/test/module naming consistent throughout

---

## Verified Correct (doc-drift positives)

These were checked and found accurate. Don't re-flag in future audits:

- M7 16-string method table matches spec exactly
- Type-attached constants (`int.max`, `int.min`, `number.epsilon`)
- Banned declaration keywords in lexer (all 19 documented; teaching diagnostics correct)
- `Frame` / `SourceLoc` compiler-synthesized shapes
- Union `|` syntax (locked; `or` not registered as keyword)
- `errors` keyword pipeline (parser + typeck + codegen + runtime)
- Backtick strings + interpolation + 6 escapes
- `for ((k, v) in m)` destructure
- M7 banned-jargon additions (monad, lift, wrap, unwrap, Result, Option, Either, exception, try, catch, throw, UTF-16)
- LLVM context lifetime in codegen (no leak — verified by golden test asserting `Send + Sync`)

---

## Parallel Fix Groups (when user approves Phase 6)

Group A (independent — different crates, different files):
- ynz-typeck files: Phase1 #1, #2; Phase2 #26; UX #39, #40, #46; redundancy #47; consolidation
- ynz-driver files: ~~Phase2 #10; Security #19, #20; UX #44~~ (FIXED Batch 4b); UX #41, #45 remain
- ynz-codegen files: Phase2 #14; doc
- ynz-runtime files: Phase2 #12, #13; doc
- ynz-diagnostics files: Phase1 #8; Phase2 #27; UX #42, #43
- ynz-parser files: Phase1 #5, #7; security #21; bug
- spec/ files: doc-drift #28-#35 (write changes only)

Group B (sequential — depends on A):
- LLVM IR flag (Phase1 #4) — after CLI exit-code rework (UX #44 — ~~exit codes FIXED Batch 4b~~; `--emit-ir` wiring remains)
- ~~`--keep` flag (UX medium)~~ **FIXED (Batch 4b)**
- Error-gallery restructure — after the underlying diagnostics fired

Group C (low-risk parallel — different concerns):
- Documentation High items: independent of fix work, can run in parallel as a `docs-writer` agent batch
- Magic-constant renames: mechanical, single haiku pass

---

## Discussion items (need a decision before fixing)

1. **`design/destructuring.md` deferral**: spec is shipped but feature isn't. Either implement object-destructuring (real scope) or mark the spec page "v0.2 deferred" + add to `open-questions.md`.
2. **Error-gallery convention**: should we restructure gallery files so triggers fire by default (auto-validation), or keep the "uncomment to trigger" model? The plan invariant requires the former.
3. **`spec/modules.md` cross-file calls**: syntax parses but calls are v0.2 stubs. Add per-section "v0.1 parse-only" caveats, or move whole section behind a "Status: v0.2" flag?
4. **`f32` deferral diagnostic**: worth a teaching diagnostic now or wait for v2?
5. **Object-destructuring + design/open-questions.md**: should `open-questions.md` track v0.2 deferrals or only undecided items? Currently mixed.

---

End of report.
