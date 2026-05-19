# Round 3 — Fix-by-Fix Breakdown

After Round 2 lands, Round 3 is two batches. Both touch shared code (mainly `crates/ynz-typeck/src/check.rs`, `crates/ynz-diagnostics/**`, `crates/ynz-driver/**`), so **they run sequentially**, not in parallel.

- **Batch 6 — Typeck perf + diagnostics UX + banned-jargon (typeck side)** — ~15 fixes
- **Batch 7 — ICE + observability + banned-jargon (parser/runtime/codegen side)** — ~10 fixes

Delete this file when Round 3's commits land.

---

## Batch 6 — Typeck perf + diagnostics UX + banned-jargon (typeck side)

Touches `crates/ynz-typeck/src/{check,shapes}.rs` + `crates/ynz-diagnostics/**` + `crates/ynz-typeck/src/resolve_import.rs` + `examples/errors/m{N}_errors.ynz`.

### 6.1 — `DiagnosticBucket::push` O(n²) → O(1) (CRITICAL)

**What's broken**: every time the typeck adds an error, the diagnostic bucket counts the errors-so-far by walking the entire diagnostics vector. That's bounded by the 50-error cap, but the algorithm is wrong — and `has_errors()` (called from the driver after every phase) ALSO walks the vector linearly.

**Yinz analogy**: imagine if Yinz computed `myArray.count()` by walking the whole array every time you wanted to check the count. Obvious waste. The fix is what you'd reach for instinctively — keep a counter, update on push.

**File**: `crates/ynz-diagnostics/src/bucket.rs:29-38, 43-47`.

**Fix**: add `error_count: usize` field to `DiagnosticBucket`. Increment on push when severity is Error; decrement in `truncate` by counting errors in the truncated tail. `has_errors()` becomes `self.error_count > 0` — O(1).

**Risk**: low. Mechanical. Existing tests should pass unchanged.

### 6.2 — `levenshtein()` allocates O(m×n) matrix per call (HIGH)

**What's broken**: every undefined-name error in your code triggers a "did you mean?" search. That search calls `levenshtein` against every candidate name in scope. Each `levenshtein` call allocates a fresh 2-D matrix on the heap. 20 candidates in scope × 1 typo = 20 matrix allocations.

**Yinz analogy**: it's like building a fresh `array<int>` of size N×M for every word comparison — when a 2-row rolling window would suffice.

**File**: `crates/ynz-typeck/src/check.rs:3683-3700`.

**Fix**: rewrite using the standard two-row rolling DP (only `O(min(m,n))` space). Skip the `Vec<char>` allocations too — identifiers are ASCII; work on bytes directly. Add an early-exit when `|m-n| > threshold` to skip obviously-too-different candidates without doing any DP work.

**Risk**: low-medium. Algorithm change but easy to test (existing tests that exercise "did you mean" should still pass and produce identical suggestions).

### 6.3 — `detect_extends_cycles` uses `Vec::contains` for visited set (HIGH)

**What's broken**: cycle detection in `extends` chains walks each candidate's chain looking for repeats. The "visited" set is a `Vec<String>`, and `.contains(parent)` is O(k) per check.

**Yinz analogy**: using `array<string>` where you should use `set<string>` for membership tests. The neighbor function `has_cycle` (line 556) already uses `HashSet` — this function just didn't get updated when the pattern landed.

**File**: `crates/ynz-typeck/src/shapes.rs:474-495`.

**Fix**: swap `Vec<String>` for `HashSet<String>`. The chain-string for the diagnostic message can still be built as a `Vec<String>` separately if needed.

**Risk**: zero. One-line type change. Tests already cover.

### 6.4 — `render()` clones every source string per render call (HIGH)

**What's broken**: every time the compiler renders a diagnostic, it clones every project source file into an `ariadne::Source`, even files that have no diagnostics in this render pass. For a 100-file project, that's 100× clone + line-table-build, only one of which actually gets used.

**Yinz analogy**: imagine if every `print()` call read every other source file in your project just in case. Obvious waste.

**File**: `crates/ynz-diagnostics/src/render.rs:55-59`.

**Fix**: implement `ariadne::Cache::fetch` to build `Source` lazily on first access per file, instead of eagerly cloning at SourceCache construction. Files without diagnostics never get parsed.

**Risk**: low. ariadne's `Cache` trait is designed for this pattern.

### 6.5 — `Note: Why:` double label on every diagnostic (HIGH)

**What's broken**: every diagnostic renders as `│ Note: Why: ...`. Two stacked labels. Reads as bureaucratic noise.

**File**: `crates/ynz-diagnostics/src/render.rs:76`.

**Fix**: either drop the literal `Why: ` prefix (the `Note:` label from ariadne is enough) or replace ariadne's `Note:` label with `Why:` via custom config. The cleaner option is the latter — eliminates the `Note:` label entirely and produces just `Why: ...`.

**Risk**: medium. Visual change that will move every snapshot test. Accept `cargo insta` snapshots after confirming the new format is correct.

### 6.6 — `what_instead` used as caret label but reads as instructions (HIGH)

**What's broken**: the inline span label under the caret shows the `what_instead` text. For many diagnostics that text is action-instructions ("Change the annotation to `int`..."), which read poorly when the caret is pointing at the offending VALUE 20 characters away from where the change should happen.

**Yinz analogy**: imagine a Yinz error pointing at `xs[0]` with the inline label "Change `let` to `const` at line above" — the caret is on the wrong thing.

**Files**: `crates/ynz-diagnostics/src/render.rs:75` (the structural place) + audit of caller sites in `check.rs` (which diagnostics use prose-instruction `what_instead` vs terse code-fix `what_instead`).

**Fix** (LOCKED — Patrick chose option a 2026-05-19): restructure `render` so the caret label is a terse generated tag (auto-derived from the diagnostic's kind — "expected: int", "consumed", "borrowed", etc.), and move `what_instead` to the note position where prose instructions belong.

Side effect: ~100 snapshot tests regenerate. Plan: spot-check a few new snapshots to confirm the format is right, then `cargo insta accept` the rest. WHY this is the right long-term fix: the current setup forces every diagnostic author to write `what_instead` as a span-appropriate label AND an action-instruction simultaneously, and most pick one or the other and ship the wrong-context version. The restructure cleanly separates "what's at the caret" from "what you should do."

**Risk**: medium-high. Snapshot churn is large. Implementation needs care so the generated caret label is genuinely useful (not just "error here") — likely a small enum on `Diagnostic` for the kind, generating tags like `expected:T`, `unreachable`, `unused`, etc.

### 6.7 — Render footer (hidden-count + URL) written without ariadne styling (MEDIUM)

**File**: `crates/ynz-diagnostics/src/render.rs:88-101`.

**Fix**: format the footer through ariadne's styled writer so the URL gets bold/underline treatment. Currently `writeln!` raw bytes → flat text below colored diagnostics.

**Risk**: low. Polish.

### 6.8 — `Diagnostic::file_error()` convenience helper (MEDIUM)

**What's broken**: 21 call sites across `ynz-driver/`, `ynz-typeck/`, `ynz-codegen/` use the pattern `Diagnostic::error(SourceSpan::new(<path>, 0, 0), <what>, <what_instead>, <why>)` for "I don't have a real span, point at the file." Positional 4-arg API + magic `0, 0` numbers = error-prone.

**File**: `crates/ynz-diagnostics/src/diagnostic.rs` (add helper), then call sites stay flexible — no mandatory migration.

**Fix**: add `Diagnostic::file_error(path, what, what_instead, why)` convenience constructor that wraps the span construction. Existing call sites stay until they're touched naturally; no big-bang migration. The "0, 0" magic numbers stop being a code smell.

**Risk**: zero. Additive API.

### 6.9 — Error gallery restructure (auto-fire triggers) (MEDIUM)

**What's broken**: `examples/errors/m7_errors.ynz` and `m8_errors.ynz` have most of their compile-error triggers wrapped in `// Uncomment to trigger:` comments. Per `plan-invariants.md` `### Demo & Error Gallery`, one run of the file should produce every diagnostic for that milestone. Today it produces almost nothing.

**Files**: `examples/errors/m7_errors.ynz`, `examples/errors/m8_errors.ynz`.

**Fix** (LOCKED — Patrick chose option c 2026-05-19 for the long-term right answer): per-milestone test in `crates/ynz-driver/tests/` (probably a new file `error_galleries.rs` or extension of `integration.rs`) that:
1. Compiles each `examples/errors/m{N}_errors.ynz` via the driver
2. Asserts the gallery produces a known set of diagnostics
3. Each gallery's expected output catalogued in the test (diagnostic count + key text per error class)

This is option (c) because (a) alone (auto-fire triggers) doesn't VERIFY the gallery is current. If a diagnostic gets removed from the compiler, the gallery silently has a dead trigger and (a) won't catch it. The test makes the gallery a verified artifact.

Sub-fix: also restructure the gallery files themselves to auto-fire (option a), so the test actually has triggers to assert on. Both layers together — restructured files + verifying test — give the right long-term fix.

Per `plan-invariants.md` `### Demo & Error Gallery`, the gallery IS supposed to be a hands-on review surface that breaks when the implementation drifts. Option (c) makes that break automated.

**Risk**: medium. The verifying test is new infrastructure; need to design the expected-output format carefully so it's tolerant of formatting changes but catches semantic drift. Likely a JSON manifest per gallery (`m4_errors.expected.json` etc.) listing expected diagnostic codes/key-phrases.

### 6.10 — Keep `remainder`, remove it from the banned-jargon list (MEDIUM — reversed)

**Update 2026-05-19 (Patrick)**: `remainder` is NOT actually jargon. `%` (the symbol) is MORE opaque to newcomers than the mathematical term `remainder`. The audit's plan to ban `remainder` was wrong; the diagnostic at `check.rs:1551` is fine as-is.

**Fix**:

1. `crates/ynz-typeck/src/check.rs:1551` — light polish to teach the operator-to-term mapping. Suggested wording:
   ```
   WHAT: "The `%` (remainder) operator on `number` requires careful rounding semantics."
   WHAT INSTEAD: "Use `int` instead of `number` if you want exact integer remainders, or write your own rounding-aware helper."
   WHY: "On decimal `number`, `%` (remainder) depends on which rounding mode is in effect — IEEE 754-2008 §5.3.1 defines remainder as `a − (round(a/b) × b)`, and different rounding modes (half-even, truncation, etc.) produce different results for the same inputs. Yinz refuses `%` on `number` to avoid the silent precision-loss class."
   ```
   The `%` operator gets backtick-wrapped; the math term clarifies. Both available; users learn the mapping.

2. `design/compiler-errors.md` — REMOVE `remainder` from the banned-jargon list. Add a comment explaining why it was reconsidered: "Mathematical terms that aid teaching (`remainder`, `quotient`, `divisor`) are NOT jargon — they're more accessible than the operator symbols. Only terms that are programmer-internals jargon (`monomorphize`, `propagate`) belong on this list."

3. Do NOT add `remainder` to `crates/ynz-diagnostics/src/banned_jargon.rs` — leave the list as-is for this term.

**Risk**: zero. Polishes the diagnostic; cleans up the design doc.

### 6.11 — `alias` banned-jargon diagnostic rewrite (MEDIUM)

**What's broken**: `crates/ynz-typeck/src/resolve_import.rs:171,236` (and `parser.rs:382-383,487`) use "Expected an alias name after `as`" — references the syntax keyword `as alias`. Banned-jargon list (4c) skipped `alias` because of these sites.

**Fix**: rewrite to avoid the standalone word "alias" — use phrases like "the name you want to bind the import to" or "the local name after `as`". Then add `alias` to `banned_jargon.rs`.

The parser ones (4c.11 / parser.rs sites) will land in Batch 7 since they're parser-crate.

**Risk**: low.

### 6.12 — Consolidation #6 — DiagnosticBucket round-trip drops `hidden_count` (MEDIUM)

**What's broken**: salsa query outputs store `Vec<Diagnostic>` not `DiagnosticBucket`. Each query produces a bucket internally, then converts to Vec at the boundary. The `hidden_count` (errors past the 50-cap) is silently dropped in the Vec round-trip. A project with 80 errors total reports "0 hidden" because each query's bucket only sees its own 50.

**File**: `crates/ynz-diagnostics/src/bucket.rs` + salsa query output types.

**Fix**: pick option (a) `DiagnosticBucket: Clone + PartialEq` so queries store buckets directly, OR (b) wrap as `DiagnosticsWithHiddenCount { vec, hidden: usize }` carrying both. (a) is simpler if PartialEq is sensible on bucket.

**Risk**: medium. Touches salsa output types — already shipped real PartialEq in Round 3 (Batch 3), so adding the wrapper or bucket-PartialEq is incremental.

### 6.13 — `find_project_root` discrepancy in import diagnostic (MEDIUM)

**File**: `crates/ynz-typeck/src/resolve_import.rs:110-112` (`has_project_root` computation).

**What's broken**: `has_project_root` decides whether to show the "no yinz.toml" hint based on `importer_path.parent().unwrap_or(".")` while `resolve_module_path` uses `importer.parent()` (returns `None` if no parent). The two answers can diverge for files at root or with weird paths.

**Fix**: use one helper for both. After Batch 4b's `src/` cleanup, `find_project_root` is the unified helper. Make `has_project_root` use the same code path.

**Risk**: low.

### 6.14 — `options_table::tag_for` should return `u8` not `i8` (LOW)

**File**: `crates/ynz-typeck/src/options_table.rs:52`.

**What's broken**: tag values are 0-255 (validator caps at 256 variants). Returning `i8` means variants 128-255 produce negative tags. Today it works because no real `options` declaration hits 128+ variants — but it's a latent silent-overflow bug.

**Fix**: change `i8` → `u8` everywhere `tag_for` is used. Cascades through callers.

**Risk**: low. Mechanical.

### 6.15 — `ExportTable::eq` coarse-equality risk doc at field comparison (LOW)

**File**: `crates/ynz-typeck/src/exports.rs:35` (impl PartialEq).

Resolved by Batch 3 (the `PartialEq` derive replaced the coarse manual impl). Verify the entry is removed from the audit; the comment that lived above the old impl may need cleanup too.

**Risk**: zero.

---

## Batch 7 — ICE + observability + banned-jargon (parser/runtime/codegen side)

Sequential AFTER Batch 6 because both touch `crates/ynz-diagnostics/` and `crates/ynz-driver/`.

### 7.1 — Top-level panic hook (CRITICAL — ICE distinction)

**What's broken**: when the compiler itself panics (e.g., an `.expect()` deep in lexer/parser/typeck), the user sees a raw Rust panic message. They don't know whether it's their bug or ours. Maintainers reading bug reports get no source-stage, no backtrace, no repro info.

**Yinz analogy**: imagine if a runtime `errors`-capable function panicked and the user saw "called `Result::unwrap()` on `Err`" instead of a Yinz-shaped error. Confusing, useless.

**File**: `crates/ynz-driver/src/main.rs`.

**Fix**: install `std::panic::set_hook` at top of `main()`. When a panic fires:
1. Print a clean banner: "**This is a Yinz compiler bug, not your code.**"
2. Echo the panic message + location
3. Print the URL to file an issue + ask for `RUST_BACKTRACE=1` output
4. Exit with `EXIT_INFRA_ERROR` (= 2 per Batch 4b's exit-code constants)

Match the existing `codegen_query` error pattern at `crates/ynz-codegen/src/queries.rs:57-62` — that already has "this is a compiler bug" framing for LLVM emit failures.

**Risk**: medium. Every panic path becomes user-visible — test by intentionally triggering a panic and seeing the new output.

### 7.2 — `--emit-ir` flag (CRITICAL — observability)

**What's broken**: `CompiledArtifact.ir_text` is populated on every build but never read by the driver. Tests use it via insta snapshots; production CLI has no flag. "Wrong codegen output" investigations require recompiling the compiler with a one-off patch.

**File**: `crates/ynz-driver/src/main.rs` (add flag), `crates/ynz-driver/src/build.rs` (plumb to write the IR alongside the binary).

**Fix**: add `--emit-ir` to the `Build` clap variant. When set, write `<binary_path>.ll` alongside the binary (or to stdout if `--emit-ir=-`). The IR text is already in `result.artifact.ir_text` — just print it.

**Risk**: low. Trivial — the data is already populated.

### 7.3 — `ariadne` render `.expect()` panics on out-of-range span (HIGH)

**File**: `crates/ynz-diagnostics/src/render.rs:85`.

**What's broken**: `.expect("ariadne render failed")` panics if a diagnostic carries a span past EOF (a bug in a recovery path somewhere). The panic bypasses the diagnostic pipeline entirely.

**Fix**: replace `.expect()` with graceful degradation — write a fallback "diagnostic could not be rendered (compiler bug)" message and continue. With Batch 7.1's panic hook in place, this is a defense-in-depth fix.

**Risk**: low.

### 7.4 — Runtime overflow / div-zero panics lack source location (HIGH)

**Files**: `crates/ynz-runtime/src/lib.rs:115-127, 134-143` (the panic functions) + `crates/ynz-codegen/src/emit.rs` (where the panic calls are emitted).

**What's broken**: when an `int + int` overflows at runtime, the panic message says only "int overflow in '+'". User doesn't know WHERE in their source — could be any of dozens of additions.

**Yinz analogy**: imagine a Yinz function panicked and the message was "errors:something" with no file/line. You wouldn't know where to look.

**Fix**: cross-crate change. Codegen emits a panic-call with file+line+col as immediate i32/string args; runtime panic functions accept those args and include them in the message. The design doc `design/compiler-errors.md:161` envisions exactly this format: `"RUNTIME ERROR: integer overflow at <file>:<line>"`.

**Risk**: medium. Cross-crate cascade. Needs codegen changes + runtime ABI changes + tests.

### 7.5 — `bignum_binop` silent zero — already addressed in Batch 5b

Cross-reference. Was a Forensics finding too. Confirm removed from `.analysis/forensics.md`.

### 7.6 — `codegen_query` LLVM-error span pinned at (0,0) (LOW)

**File**: `crates/ynz-codegen/src/queries.rs:59`.

**What's broken**: when LLVM emit fails, the diagnostic points at byte 0 of the source. User sees "the error is on line 1" when actually the error is "the backend choked on something."

**Fix**: emit the diagnostic without a span — let ariadne render it as a file-level message rather than a misleading caret-at-line-1. Or use the new `Diagnostic::file_error` helper from Batch 6.

**Risk**: low.

### 7.7 — Parser/lexer `.expect()` calls — audit for span context (HIGH)

**Files**: `crates/ynz-parser/src/lexer.rs:469,866,935,976,1003,1058,1073` (the multiple "digits are ASCII" / "identifier is UTF-8" expects).

**What's broken**: these `.expect()` calls all rely on a cross-module invariant ("the driver validated UTF-8 at load time"). If anything creates a Lexer outside that path, the panic message is internal Rust developer text with no source context.

**Fix**: either document the invariant via `// SAFETY:` comments at each site (cheap), OR convert to graceful failure paths that route through diagnostics (more work). With Batch 7.1's panic hook in place, the cheap option is acceptable.

**Risk**: low.

### 7.8 — `lifetime` banned-jargon diagnostic rewrite (MEDIUM)

**File**: `crates/ynz-parser/src/lexer.rs:532,540` (`promise`/`future` banned-keyword diagnostics).

**What's broken**: the WHY text mentions "outside this function's lifetime" — uses banned `lifetime` term.

**Fix**: rephrase to avoid `lifetime`. "outside this function's scope" or "after this function returns" work. Then add `lifetime` to `banned_jargon.rs`.

**Risk**: low.

### 7.9 — `interface` banned-jargon diagnostic rewrite (MEDIUM)

**File**: `crates/ynz-parser/src/lexer.rs:613`.

**What's broken**: the diagnostic literally says "`interface` is not a keyword in Yinz." The word IS the diagnostic content.

**Fix**: rephrase the diagnostic so the banned word doesn't appear in plain text. Something like "Yinz doesn't have an `interface` keyword — use `shape` for data + `follows` for contracts." (Backtick-wrapped, treated as a code mention not a vocabulary leak.)

Actually re-reading the banned-jargon test logic: it checks for the bare word, not backtick-wrapped code references. Code references to forbidden TS/Java/Rust keywords ARE allowed in diagnostics (the user typed the word; we're rejecting it). The 4c agent may have skipped this entry unnecessarily.

**Action**: verify what `jargon_audit.rs` actually catches. If it's lenient on backtick-wrapped code references, no rewrite needed — just add `interface` to the list with a note about the backtick exception.

**Risk**: low.

### 7.10 — Parser `alias` banned-jargon rewrite (MEDIUM)

**File**: `crates/ynz-parser/src/parser.rs:382-383, 487`.

Cross-reference to Batch 6.11. The typeck side fixes resolve_import.rs; this side fixes the parser. Same rewording pattern.

**Risk**: low.

---

## Skipped / deferred from Round 3

- **NUL byte codegen-side handling**: Batch 5a.6 rejects `\0` at the lexer, so codegen no longer sees NUL bytes from source. The codegen side becomes defense-in-depth only — deferred indefinitely until `{ptr, len}` string overhaul.
- **Bug #10 entrypoint→main rename collision** (multi-file projects with multiple `function entrypoint()`): touches codegen, separate concern. Defer to Round 4 or its own batch.
- **`ynz_array_new` / `ynz_array_push` null-deref** (found by Batch 5b agent as out-of-scope): same pattern as the map fixes; small batch could ship alongside something else. Note for future.
- **`mangle_type` Debug-format ambiguity** (Bug #13): edge case, defer to Round 4.

---

## Decision items before dispatch

1. **6.5 / 6.6 — diagnostics visual changes**: pick (a) restructure caret-label vs (b) per-diagnostic audit for `what_instead` wording. (a) regenerates ~100 snapshots; (b) is surgical but slower. Recommend (b).

2. **6.9 error gallery restructure**: pick (a) inline triggers as top-level functions vs (c) test harness asserting diagnostic count. (a) is simpler. (c) is most robust. Recommend (a) for this batch; (c) as a follow-up if needed.

3. **7.4 runtime panic source location**: cross-crate cascade is the right design per `compiler-errors.md:161`, but it's bigger than the other items in this batch. Ship it in Batch 7 or split into its own batch?

Once you've read this and the Round 2 batches all land, I'll re-spec based on what surfaces and dispatch.

---

## Summary

| Batch | Critical fixes | High fixes | Notable items |
|---|---:|---:|---|
| 6 — Typeck perf + diagnostics UX | 1 (DiagnosticBucket O(n²)) | 4 (levenshtein, detect_extends_cycles, render Source clone, Note: Why:) | + 2 banned-jargon rewrites (typeck-side), error gallery restructure |
| 7 — ICE + observability + banned-jargon | 2 (panic hook, --emit-ir) | 2 (ariadne expect, runtime overflow source-loc) | + 3 banned-jargon rewrites (parser/lexer side) |

**Critical fixes remaining after Round 3**: probably 0-1 depending on what Round 2 surfaces.
