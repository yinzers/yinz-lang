# UX Analysis — Yinz Compiler

**Scope**: `crates/ynz-diagnostics/`, `crates/ynz-typeck/src/`, `crates/ynz-driver/src/`, `examples/errors/`, snapshot files
**Friction**: 12 findings (Critical: 1, High: 4, Medium: 6, Low: 1)
**Opportunities**: 9 (High: 3, Medium: 4, Low: 2)

---

## Friction

### CRITICAL — Snapshot test ships banned `type` declaration keyword in suggestion

- **File**: `crates/ynz-diagnostics/tests/snapshots.rs:88-92` + `__snapshots/suggestion_only.snap`
- **Text**: `"Consider using a `type` instead of a `map` for direct field access."`
- **Gap**: Tells user to write `type Foo { ... }` — which the lexer banned-jargon list rejects. Compiler contradicts itself. Jargon audit doesn't catch this because `type` is in the lexer banned-declaration-keyword list, not `BANNED_JARGON`.
- **Fix**: Replace `type` with `shape` in the snapshot fixture text.

### HIGH — Render output prefixes WHY with redundant `Note: Why:` label

- **File**: `crates/ynz-diagnostics/src/render.rs:76`
- **Gap**: Every diagnostic shows `│ Note: Why: ...`. Two stacked labels. Reads as bureaucratic noise.
- **Fix**: Drop the literal `Why: ` prefix or replace ariadne's `Note:` label with `Why:`.

### HIGH — `what_instead` used as caret label but often reads as instructions

- **File**: `crates/ynz-diagnostics/src/render.rs:75` + systematic across `check.rs`
- **Gap**: `what_instead` serves two purposes ariadne conflates: inline span annotation vs action instruction. Examples: `"Change the annotation to 'int', or use a different value."` underlines the value with prose about the annotation 20 chars away.
- **Fix**: For prose-instruction diagnostics, move the instruction into the note position; keep the caret label terse.

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

### MEDIUM — render footer (`... and N more` + issues URL) written without ariadne styling

- **File**: `crates/ynz-diagnostics/src/render.rs:88-101`
- **Gap**: Raw `writeln!` bypasses styling pipeline. Plain text below colored diagnostics; URL easy to miss.
- **Fix**: Format footer through a styled writer; use bold/underline for URL.

### MEDIUM — `next()` return-type error in `check.rs:822` mixes prose + signature in `what_instead`

- **File**: `crates/ynz-typeck/src/check.rs:822`
- **Gap**: `"Change \`function next(... ) -> {wrong}\` to return \`maybe<T>\` instead."` — user must mentally substitute. Not copy-pasteable.
- **Fix**: Emit the corrected signature in `what_instead`; explain in `why`.

### MEDIUM — Error gallery files have most triggers commented out

- **File**: `examples/errors/m7_errors.ynz:19-163`, `examples/errors/m8_errors.ynz:18-101`
- **Gap**: Per `plan-invariants.md` `### Demo & Error Gallery`, one run of the file should produce every diagnostic that milestone can emit. Currently most triggers are wrapped in `// Uncomment to trigger:` blocks. Review mechanism broken.
- **Fix**: Restructure gallery files so triggers fire by default — using separate small modules per error, or `expect`-style harness that asserts diagnostic counts.

### MEDIUM — `resolve_import.rs:147` circular-import error doesn't show the cycle chain + has dead var

- **File**: `crates/ynz-typeck/src/resolve_import.rs:147-149`
- **Gap**: `cycle_path` computed and dropped. Error says "module already in chain" without showing which path. Junior dev can't see their loop.
- **Fix**: Include the chain (`a → b → a`) in `what_instead`.

### MEDIUM — `build.rs:229` linker stderr stuffed into `why` field

- **File**: `crates/ynz-driver/src/build.rs:227-232`
- **Gap**: `Note: Why: ld: undefined symbol: ynz_print` — frames linker tool output as Yinz explanation.
- **Fix**: Use `why` for a teaching explanation ("the linker combines compiled code into a binary; this error means a function was declared but not implemented"); put raw stderr in a separate section.

### LOW — No success confirmation from `ynz build`

- **File**: `crates/ynz-driver/src/build.rs:385-401`, `crates/ynz-driver/src/run.rs:15-17`
- **Gap**: Successful build with warnings prints warnings; success with no warnings prints nothing. New user can't tell if it worked.
- **Fix**: Print `Build succeeded: <binary_path>` to stdout on success.

---

## Opportunities

### HIGH — No "did you mean?" for imports

- **File**: `crates/ynz-typeck/src/resolve_import.rs:211-229`
- **Gap**: Import-name typo shows the full sorted export list; `find_closest_name` is already wired for `check_call` / `resolve_ident` but not for imports.
- **Fix**: Apply `find_closest_name` against `exported_names` before formatting `what_instead`.

### HIGH — Single exit code (1) for all failure modes

- **File**: `crates/ynz-driver/src/main.rs:36-48`
- **Gap**: Compile error vs "linker not installed" vs I/O error → indistinguishable to CI/editor integration. `rustc`/`clang` use distinct codes.
- **Fix**: `EXIT_COMPILE_ERROR=1`, `EXIT_INFRA_ERROR=2`. Plumb through `BuildResult`.

### HIGH — No success-line output

- Covered above (Friction Low). Cross-listed because impact is high for new-user onboarding.

### MEDIUM — `.value` on unguarded `maybe<T>` lacks dedicated teaching diagnostic

- **File**: `crates/ynz-typeck/src/check.rs` field-access path
- **Gap**: Falls through to generic field-not-found. The most common novice mistake from nullable-types backgrounds deserves a specific "use `if (x.exists()) { let n = x.value }` with the user's actual variable name" diagnostic.

### MEDIUM — `ynz run` always deletes binary, no `--keep`

- **File**: `crates/ynz-driver/src/run.rs:27`
- **Gap**: Common workflow "build once, run many times with different inputs" forced through `ynz build` + manual invocation.
- **Fix**: `--keep` flag or make delete opt-in.

### MEDIUM — Mid-build SIGKILL leaves `.o` files in source tree

- Cross-references reliability.md Finding 4 + adversarial Finding 9.

### LOW — `ynz build --help` doesn't explain single-file vs project mode

- **File**: `crates/ynz-driver/src/main.rs:20-31`
- **Fix**: Expand `long_about` describing `yinz.toml` role and the two modes.

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
