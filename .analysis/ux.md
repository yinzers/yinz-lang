# UX Analysis — Yinz Compiler

**Scope**: `crates/ynz-diagnostics/`, `crates/ynz-typeck/src/`, `crates/ynz-driver/src/`, `examples/errors/`, snapshot files
**Friction**: 9 findings remain (Critical: 1, High: 3, Medium: 5, Low: 0) — 3 fixed Batch 4b
**Opportunities**: 7 remain (High: 2, Medium: 4, Low: 1) — 2 fixed Batch 4b

---

## Friction

### CRITICAL — Snapshot test ships banned `type` declaration keyword in suggestion

- **File**: `crates/ynz-diagnostics/tests/snapshots.rs:88-92` + `__snapshots/suggestion_only.snap`
- **Text**: `"Consider using a `type` instead of a `map` for direct field access."`
- **Gap**: Tells user to write `type Foo { ... }` — which the lexer banned-jargon list rejects. Compiler contradicts itself. Jargon audit doesn't catch this because `type` is in the lexer banned-declaration-keyword list, not `BANNED_JARGON`.
- **Fix**: Replace `type` with `shape` in the snapshot fixture text.

~~### HIGH — Render output prefixes WHY with redundant `Note: Why:` label~~
**FIXED in Batch 6.5** — `"Why: "` prefix dropped; renders as `Note: <text>`.

~~### HIGH — `what_instead` used as caret label but often reads as instructions~~
**FIXED in Batch 6.6 (infrastructure)** — `DiagnosticKind` enum added; when `kind` is set, caret uses `kind.tag()` and `what_instead` moves to note. Per-diagnostic migration ongoing.

### HIGH — `check.rs:444` parameter-reassignment WHY references shipped M4 as future

- **File**: `crates/ynz-typeck/src/check.rs:444`
- **Text**: `"Yinz ownership modifiers that allow parameter mutation (`lend`) arrive in v0.1 milestone 4."`
- **Gap**: M4 shipped per CHANGELOG. WHY is factually stale + leaks internal milestone vocabulary.
- **Fix**: Rewrite without milestone reference; explain the actual constraint (signature must declare `lend`).

### HIGH — `check.rs:1282-1287` "not defined" function-call WHY is generic

- **File**: `crates/ynz-typeck/src/check.rs:1282-1287`
- **Text**: `"The compiler looks up every name you call. If a name doesn't exist, the program can't run."`
- **Gap**: Explains what compilers do; doesn't help the user fix the typo. Violates Golden Rule 11 (specific + contextual WHY).
- **Fix**: Mention "is it in another file? add `import { name } from './module'`"; specific to call-site.

~~### MEDIUM — render footer (`... and N more` + issues URL) written without ariadne styling~~
**FIXED in Batch 6.7** — URL wrapped with ANSI bold+underline when `colors=true`.

### MEDIUM — `next()` return-type error in `check.rs:822` mixes prose + signature in `what_instead`

- **File**: `crates/ynz-typeck/src/check.rs:822`
- **Gap**: `"Change \`function next(... ) -> {wrong}\` to return \`maybe<T>\` instead."` — user must mentally substitute. Not copy-pasteable.
- **Fix**: Emit the corrected signature in `what_instead`; explain in `why`.

~~### MEDIUM — Error gallery files have most triggers commented out~~
**FIXED in Batch 6.9** — m7/m8 gallery files restructured to auto-fire; `crates/ynz-driver/tests/error_galleries.rs` added to verify diagnostic counts + key phrases.

### MEDIUM — `resolve_import.rs:147` circular-import error doesn't show the cycle chain + has dead var

- **File**: `crates/ynz-typeck/src/resolve_import.rs:147-149`
- **Gap**: `cycle_path` computed and dropped. Error says "module already in chain" without showing which path. Junior dev can't see their loop.
- **Fix**: Include the chain (`a → b → a`) in `what_instead`.

### MEDIUM — `build.rs:229` linker stderr stuffed into `why` field

- **File**: `crates/ynz-driver/src/build.rs:227-232`
- **Gap**: `Note: Why: ld: undefined symbol: ynz_print` — frames linker tool output as Yinz explanation.
- **Fix**: Use `why` for a teaching explanation ("the linker combines compiled code into a binary; this error means a function was declared but not implemented"); put raw stderr in a separate section.

### ~~LOW — No success confirmation from `ynz build`~~ FIXED (Batch 4b)

`main.rs` now prints `Build succeeded: <binary_path>` to stdout on success.

---

## Opportunities

### HIGH — No "did you mean?" for imports

- **File**: `crates/ynz-typeck/src/resolve_import.rs:211-229`
- **Gap**: Import-name typo shows the full sorted export list; `find_closest_name` is already wired for `check_call` / `resolve_ident` but not for imports.
- **Fix**: Apply `find_closest_name` against `exported_names` before formatting `what_instead`.

### ~~HIGH — Single exit code (1) for all failure modes~~ FIXED (Batch 4b)

`EXIT_COMPILE_ERROR=1`, `EXIT_INFRA_ERROR=2` in `main.rs`. `FailureKind` plumbed through `BuildResult`.

### ~~HIGH — No success-line output~~ FIXED (Batch 4b)

See Friction section above.

### MEDIUM — `.value` on unguarded `maybe<T>` lacks dedicated teaching diagnostic

- **File**: `crates/ynz-typeck/src/check.rs` field-access path
- **Gap**: Falls through to generic field-not-found. The most common novice mistake from nullable-types backgrounds deserves a specific "use `if (x.exists()) { let n = x.value }` with the user's actual variable name" diagnostic.

### ~~MEDIUM — `ynz run` always deletes binary, no `--keep`~~ FIXED (Batch 4b)

`--keep` flag added to `ynz run` in `main.rs`/`run.rs`.

### ~~MEDIUM — Mid-build SIGKILL leaves `.o` files in source tree~~ FIXED (Batch 4b)

All intermediates now go into `tempfile::tempdir()` — source tree is never polluted.

### ~~LOW — `ynz build --help` doesn't explain single-file vs project mode~~ FIXED (Batch 4b)

Both `Build` and `Run` subcommand variants now have expanded doc comments describing modes, options, and exit codes.

### LOW — Jargon audit doesn't catch declaration-keyword leaks in backticks

- **File**: `crates/ynz-diagnostics/tests/jargon_audit.rs:34-38`
- **Gap**: `BANNED_JARGON` omits `type`/`struct`/`class`/`interface`/`enum` to allow legitimate prose use. Test can't catch the snapshot violation above.
- **Fix**: Secondary check that flags backtick-wrapped declaration keywords in `what` / `what_instead`.

---

## Strengths (preserve)

- `Diagnostic::new` panics on empty fields → three-part format structurally enforced
- `jargon_audit.rs` mechanically catches programmer-jargon at CI
- Levenshtein "did you mean" consistently applied in `resolve_ident` / `check_call`
- 50-error cap with footer count
- Context-specific WHY clauses for `const` mutation, `maybe<T>` access, `errors`-capable handling are exemplary teaching diagnostics
