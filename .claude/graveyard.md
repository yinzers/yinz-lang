---
name: Project Graveyard
description: Project-specific failure patterns. Auto-discovered by the global Bouncer Stop hook.
type: reference
---

# ynz Graveyard

Project-specific corpses. For format spec and cross-project corpses, see `~/.claude/memory/graveyard.md`.

Add entries via `/learn` when a project-specific mistake pattern is identified. Each entry needs Scope, Exemption, Last verified, Cause, Detection signature, Constraint, Bouncer checks, and Severity. Bouncer checks must be diff-greppable.

---

## Const Deep-Immutability Invariant Unstated in Milestone Plans — 2026-05-14

**Scope**: `.claude/plans/active/m[0-9]+-*.md` and `.claude/plans/done/m[0-9]+-*.md` for milestones M4 and later. Plans for M1, M2, M3 are exempt (predate the rule).
**Exemption**:
- Pre-M4 plan files (M1, M2, M3 — these predate the plan-invariants rule)
- Plans explicitly marked `## Invariants This Milestone Must Preserve` with all required subsections containing the const semantics
- Cross-cutting plans not tied to a specific milestone (e.g., the `design-lockdown-from-gemini-review` plan itself, which is what created this entry)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/plans/(active|done)/m[0-9]+-
ownership
const binding
borrow checker
```


**Cause**: The M3 plan stated "ownership system arrives in M4" without enumerating which call-site operations `const` blocks (`.lend`/`.give`), or which LLVM attributes M4 codegen must emit (`readonly`). The gap WAS real and would have shipped a less-safe + less-performant M4. Gemini's 2026-05-14 code review surfaced it before it shipped.

**Detection signature**: An M4-or-later milestone plan file mentions any of `ownership`, `const`, `let`, `.lend`, `.give`, `.share` in its body but does NOT contain a literal `## Invariants This Milestone Must Preserve` heading AND a `### Safety` subheading enumerating const semantics (cannot reassign, cannot be lent, cannot be given, field mutation blocked) AND a `### Performance` subheading naming LLVM `readonly` / `noalias` attribute emission.

**Constraint**: Every M4+ milestone plan that touches ownership, types, or the `const`/`let` distinction MUST include `## Invariants This Milestone Must Preserve` with `### Safety` enumerating the five paths const blocks (reassignment, `.lend`, `.give`, field mutation, mutable inference) AND `### Performance` naming the LLVM attribute contract (`readonly` on share params and params figured-out from const bindings; `noalias` where ownership rules prove non-aliasing). See `.claude/rules/plan-invariants.md` for the full required subsection list.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each plan-file diff matching `.claude/plans/{active,done}/m[0-9]+-*.md`: extract the milestone number from the filename. If number >= 4, grep the file body for `^## Invariants This Milestone Must Preserve$`. Missing → CRITICAL.
- [ ] For each M4+ plan file with the heading present: check the `### Safety` subsection (lines between `^### Safety$` and the next `^### ` or `^## `) contains ALL FIVE substrings: `cannot be reassigned`, `cannot be lent`, `cannot be given`, `field mutation`, AND (case-insensitive) one of `readonly`, `noalias`, `LLVM` somewhere in the Performance subsection. Missing any → CRITICAL.
- [ ] If the plan mentions `ownership system`, `borrow checker`, or `const binding` in its body but lacks a `## Invariants This Milestone Must Preserve` heading entirely → CRITICAL.

**Severity**: critical (this is the spine of the language's safety + performance contract; if it's not stated, M4 ships without enforcing it).

**Originating incident**: 2026-05-14 — Gemini code review during the `design-lockdown-from-gemini-review` plan's discovery phase asked "does Yinz enforce const like Rust enforces non-mut?" Investigation found const reassignment was enforced in M2 typeck (`crates/ynz-typeck/src/check.rs:264`) but no design doc stated the full invariant chain (block `.lend`/`.give`/field-mutation + emit LLVM `readonly`). The 2-hour conversation that resulted produced this plan.

---

## Requiring Explicit Ownership Annotation at Call Sites — 2026-05-14

**Scope**: `spec/*.md` and `design/*.md`, except `design/future/*` (parking-lot speculation only).
**Exemption**:
- Function signature documentation — signatures correctly declare `share`/`lend`/`give` at the parameter level. The anti-pattern is requiring annotation at the CALL site, NOT at the signature.
- Documentation explicitly describing the inverse-anti-pattern as wrong (cites this entry or `.claude/rules/inference.md`).
- Examples showing `foo(player.share)` as one of several legal forms, not as a requirement.
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
^spec/.*\.md$
^design/.*\.md$
must annotate at
required at call site
explicit annotation
at every call
\.share
\.lend
\.give
```


**Cause**: The design-lockdown conversation considered requiring explicit `.share`/`.lend` at every call site as an alternative to the uniform-inference + muted-IDE-hint approach. It was rejected because forcing explicit annotation degrades into syntactic noise developers learn to ignore (the Rust `&` pattern). Without this graveyard entry, future-Claude reading the spec could re-introduce the "must annotate at call site" framing, undoing the design.

**Detection signature**: A spec or design doc (excluding `design/future/*`) introduces language requiring ownership annotation at CALL sites. Phrases that signal the anti-pattern: "must annotate at the call site", "explicit ownership at every call", "call sites must declare share/lend/give", "required at call site", or code examples showing `foo(player.share)` with surrounding text stating this is REQUIRED (rather than optional/click-to-make-explicit-from-the-muted-hint).

**Constraint**: Spec and design docs describe call-site ownership as INFERRED-WITH-MUTED-IDE-HINT (per `.claude/rules/inference.md`). Function SIGNATURES correctly require explicit `share`/`lend`/`give` declarations; CALL SITES infer. Explicit `.share`/`.lend` typing at call sites is documented as AVAILABLE for clarity, NEVER REQUIRED.

**Bouncer checks**:
- [ ] For each diff to `spec/*.md` or `design/*.md` (excluding `design/future/*`): grep added lines (case-insensitive) for `must annotate at`, `required at call site`, `must declare.*at every call`, `explicit annotation.*at the call site`. Any match → WARNING.
- [ ] Co-occurrence check: grep added lines for `\.share|\.lend|\.give` followed within 5 lines by `must` or `required`. If both present AND NO occurrence within the same 10-line window of `signature` or `at the definition` (which are the legitimate "explicit at signature" context) → WARNING.

**Severity**: warning (re-relitigation risk; not a runtime safety violation).

**Originating incident**: 2026-05-14 — during the design-lockdown conversation, Patrick and Claude initially considered requiring explicit `.share`/`.lend` at call sites for teaching visibility. Rejected in favor of uniform inference + IDE muted hints (which preserves teaching value without the syntactic burden). Documented in `.claude/rules/inference.md` "Inverse Anti-Pattern" section. The spec file `spec/ownership.md` previously had a "smart defaults at call sites" section that was rewritten to the inferred-with-hints framing in Phase 3 of the plan.

---

## M4+ Milestone Plans Missing the 5-Subsection Invariants Structure — 2026-05-14

**Scope**: `.claude/plans/active/m[0-9]+-*.md` and `.claude/plans/done/m[0-9]+-*.md` for M4 and later. M1, M2, M3 exempt (predate the rule). This is a STRUCTURAL check (does the section exist? do all 5 subsections exist?), distinct from the const-deep-immutability entry above which checks specific CONTENT.
**Exemption**:
- Pre-M4 milestone plans
- Plans not associated with a specific milestone (cross-cutting plans, design plans, refactoring plans)
- Plans where the section structure is present but a subsection is legitimately empty because nothing applies (rare — must include "(none for this milestone)" or similar text to distinguish from forgotten)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/plans/(active|done)/m[0-9]+-
^## Invariants This Milestone Must Preserve$
```


**Cause**: Without mechanical enforcement of the 5-subsection structure, future plans will skip subsections. The const-deep-immutability gap that triggered this whole project IS an example of this kind of structural omission in real life — the M3 plan had no Invariants section at all.

**Detection signature**: An M4-or-later milestone plan file contains `## Invariants This Milestone Must Preserve` but is missing one or more of the required 5 subsections: `### Safety`, `### Performance`, `### Teaching`, `### Runtime Dependencies`, `### Kernel-Mode Behavior`. Also: a present subsection has no meaningful content within 10 lines of its header.

**Constraint**: Every M4+ milestone plan with `## Invariants This Milestone Must Preserve` MUST contain all 5 named subsections, each non-empty (at least one non-blank, non-heading line within 10 lines of the subsection heading). See `.claude/rules/plan-invariants.md` for what each subsection should contain.

**Bouncer checks**:
- [ ] For each M4+ plan file in the diff that contains `^## Invariants This Milestone Must Preserve$`: extract the section body (between that line and the next `^## ` or EOF). Within that body, grep for each of `^### Safety$`, `^### Performance$`, `^### Teaching$`, `^### Runtime Dependencies$`, `^### Kernel-Mode Behavior$`. Missing any → WARNING.
- [ ] For each subsection that IS present: verify at least one non-blank, non-heading line within 10 lines after the subsection header. Empty section → WARNING per empty.

**Severity**: warning (structural enforcement; CRITICAL safety+performance invariants covered by the const-deep-immutability entry).

**Originating incident**: 2026-05-14 — same originating incident as the const-deep-immutability entry. The 5-subsection structure was designed during plan recovery to mechanically enforce that every dimension (safety, performance, teaching, runtime, kernel-mode) is at minimum considered for every M4+ milestone.

---

## Language or Stdlib Features Without Runtime + Kernel-Mode Declaration — 2026-05-14

**Scope**: `.claude/plans/active/*.md` and `.claude/plans/done/*.md` for plans dated 2026-05-15 or later (one day after this entry lands). Includes both milestone plans AND cross-cutting plans.
**Exemption**:
- Plans whose `files:` front-matter does NOT include `crates/**` (pure docs/rules plans don't add features)
- Plans dated before 2026-05-15 (rule applies forward from when it lands)
- Plans that are explicitly documentation-only (no language/stdlib feature added)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/plans/(active|done)/.*\.md$
crates/
^### Runtime Dependencies$
^### Kernel-Mode Behavior$
```


**Cause**: The `--kernel` mode design (see `design/future/no-runtime-mode.md`) for chipset/NASA/embedded targets requires every Yinz language and stdlib feature to declare its runtime dependencies (heap allocator? scheduler? OS I/O? none?) and kernel-mode behavior (compile error? works with user-provided primitive? always works?). Without enforcement, features ship with hidden heap dependencies and the v0.3 kernel-mode work hits a wall trying to retroactively analyze every feature.

**Detection signature**: A plan file dated 2026-05-15+ with `files:` matching `crates/**` (i.e., adds compiler or stdlib features) does NOT contain `### Runtime Dependencies` AND `### Kernel-Mode Behavior` subsections within its Invariants section. These are required parts of the 5-subsection structure but called out separately because they're the KERNEL-specific check.

**Constraint**: Any plan adding language or stdlib features must declare runtime dependencies and kernel-mode behavior in the `### Runtime Dependencies` and `### Kernel-Mode Behavior` subsections of `## Invariants This Milestone Must Preserve`. Each lists per-feature: what the feature depends on at runtime, and what happens in `--kernel` mode (compile error with which message? plug-in API? always works?). See `.claude/rules/plan-invariants.md` and `design/future/no-runtime-mode.md`.

**Bouncer checks**:
- [ ] For each plan-file diff dated 2026-05-15+ with `files:` containing `crates/`: grep file body for `^### Runtime Dependencies$` AND `^### Kernel-Mode Behavior$`. Missing either → WARNING.

**Severity**: warning (forward-compat hygiene; kernel-mode is a v0.3+ target so consequences are forward-looking).

**Originating incident**: 2026-05-14 — Patrick stated during the design-lockdown conversation that chipset code, kernel modules, and NASA-grade embedded systems are target use cases. Without per-feature runtime-dependency declarations, the v0.3 `--kernel` implementation would have to retroactively analyze every M3-M8 feature for hidden allocator/scheduler dependencies. Forward-declaring is cheap; retroactive analysis is expensive.

---

## Re-Introducing Try/Catch / Recover Blocks After Rejection — 2026-05-14

**Scope**: `crates/ynz-parser/src/lexer.rs`, `crates/ynz-parser/src/token.rs`, `crates/ynz-parser/src/parser.rs`, `crates/ynz-ast/src/nodes.rs`, `spec/*.md`, `design/*.md`. Excludes `design/future/panic-safety.md` (which DOCUMENTS the rejection rationale).
**Exemption**:
- `design/future/panic-safety.md` — this file documents WHY try/catch was rejected; it must mention the syntax to do so
- Documentation that explicitly cites this entry or `design/future/panic-safety.md` as the rejection rationale (e.g., "Yinz rejects try/catch — see X")
- Test fixtures specifically testing that try/catch produces a parse error (banned-keyword diagnostic test)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-parser/src/(lexer|token|parser)\.rs$
crates/ynz-ast/src/nodes\.rs$
^spec/.*\.md$
^design/.*\.md$
(^|[^[:alnum:]_])(Try|Catch|Recover)([^[:alnum:]_]|$)
try \{
catch \(
recover \(
```


**Cause**: The design-lockdown conversation rejected try/catch in favor of `errors`-keyword auto-propagation (for KNOWN failures) + supervisor pattern at the task boundary (for UNKNOWN panics). At one point during the conversation, Claude proposed `try { } recover { }` syntax as a "different" panic-handling mechanism. Patrick caught this as try/catch under a different name. Without this graveyard entry, future-Claude in a 6-month-later session will re-relitigate ("but Erlang has supervisors AND try/catch, why can't we") and re-introduce the rejected syntax.

**Detection signature**:
- In compiler source (`crates/ynz-parser/**`, `crates/ynz-ast/**`): added lines introducing a `Try`, `Catch`, or `Recover` token/keyword/AST variant (excluding lines that are clearly comments or banned-keyword test fixtures).
- In spec/design docs (excluding `design/future/panic-safety.md`): added lines introducing `try {` or `catch (` or `recover (` syntax inside a Yinz code block as legal syntax (not as "this is rejected" demonstration).

**Constraint**: Yinz panic handling uses `errors` keyword for KNOWN failures (auto-propagate) + task-isolation via `background` + supervisor pattern at the task boundary for UNKNOWN panics. NO try/catch/recover syntax at the language level. See `design/future/panic-safety.md` for the full rationale and rejected alternatives.

**Bouncer checks**:
- [ ] For diff entries touching `crates/ynz-parser/src/lexer.rs`, `crates/ynz-parser/src/token.rs`, `crates/ynz-parser/src/parser.rs`, or `crates/ynz-ast/src/nodes.rs`: grep added (non-comment) lines for `\bTry\b`, `\bCatch\b`, `\bRecover\b` (excluding lines containing `// banned`, `// rejected`, or matching banned-keyword test fixture patterns). Any match → CRITICAL.
- [ ] For diff entries touching `spec/*.md` or `design/*.md` (use pathspec exclusion `:!design/future/panic-safety.md` to exclude the rationale file properly — NOT `grep -v` which only strips lines, leaving offending content visible): grep added lines for ` try \{`, ` catch \(`, ` recover \(` patterns. Any match → WARNING.

**Severity**: critical for compiler source changes (would land in the compiler if merged); warning for docs changes (re-relitigation risk only).

**Originating incident**: 2026-05-14 — earlier in the design-lockdown conversation, Claude proposed `try { } recover (e: Panic) { }` blocks for explicit panic recovery in scope. Patrick correctly identified this as try/catch under a different name and rejected it. The supervisor pattern at the task boundary handles every legitimate use case (per-request isolation in HTTP servers, per-job isolation in queue workers, per-order isolation in trading bots). Adding a second recovery mechanism would violate Yinz's "one concept = one keyword" principle and re-introduce all the problems Java/Python have with try/catch (catch-and-silently-continue, exception-as-flow-control, etc.). See `design/future/panic-safety.md` for the full design rationale.

---

## Generic Lexer Error for Common Migrant-Language Characters — 2026-05-18

**Scope**: `crates/ynz-parser/src/lexer.rs` — the `lex_one` match and `emit_unknown_byte` fallthrough.

**Exemption**: Truly obscure characters with no established use in major languages (e.g., `\x01`, `\x7f`) may use the generic fallthrough. Any character that IS syntactically meaningful in JS, Python, Rust, Go, TypeScript, PHP, Swift, or Kotlin needs a dedicated handler.
**Last verified**: 2026-05-18
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-parser/src/lexer\.rs$
emit_unknown_byte
not valid here
Remove or replace
```

**Cause**: New characters added to the lexer's fallthrough `emit_unknown_byte` path produce a generic "The character X is not valid here / Remove or replace this character" message with a wrong WHY ("Yinz source files may only contain ASCII text" — single quote IS ASCII). This violates Golden Rule 11 (WHY must be specific and contextual) and can cause cascade errors when the character normally delimits a multi-token construct (e.g., `'hello world'` → 5 cascade errors before the fix; `# comment` → cascade into keyword parse errors before the fix).

**Known fixed**: `"` (double quote), `'` (single quote), `#` (Python/shell comment), `;` (JS/Rust/C semicolon), `$` (PHP/shell variable prefix), `?` (Swift/Kotlin nullable suffix). Each has a dedicated handler and a regression test.

**Detection signature**: A diff adding a `b'X' => { ... emit_unknown_byte ... }` arm in `lex_one` where the message text contains "not valid here" or "Remove or replace this character" — i.e., the generic text leaks through instead of a dedicated teaching message.

**Constraint**: Any new character that is syntactically meaningful in a major language (see Exemption above) MUST get a dedicated handler in `lex_one` that:
1. Consumes the full construct if it can span multiple tokens (e.g., `#` comment → consume to EOL; `'...'` → consume to closing quote)
2. Emits a WHAT that names the character's role in the source language ("Semicolons are not used in Yinz" not "The character `;` is not valid here")
3. Emits a WHAT-INSTEAD with a concrete Yinz equivalent (not "Remove or replace")
4. Has a regression test in `crates/ynz-parser/tests/lex.rs` asserting non-generic message content

**Bouncer checks**:
- [ ] For diff lines in `crates/ynz-parser/src/lexer.rs` adding `emit_unknown_byte`: grep surrounding context for "not valid here" or "Remove or replace". Match → WARNING.

**Severity**: warning (bad UX, not a correctness bug — but the teaching mission makes this load-bearing).

**Originating incident**: 2026-05-18 — user tested `ynz run` with `print('hello world')` and got 5 cryptic cascade errors. Audit by Explore agent identified `#`, `;`, `$`, `?` as additional gaps. All five fixed in one session.

---

## Parser Infinite Loop — Zero-Advance Recovery in Bounded Loop — 2026-05-18

**Scope**: Any parser loop in `crates/ynz-parser/src/parser.rs` — specifically all `loop { ... }` constructs that contain error-recovery paths.
**Last verified**: 2026-05-18
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-parser/src/parser\.rs$
loop \{
macro_rules!
```

**Cause**: Using a macro with a `continue` statement inside `loop { macro!(); break; }`. In Rust, `continue` inside a macro targets the **innermost** enclosing loop at the expansion site — the inner `loop { ... break; }`, not the outer field/element-parsing loop. When the macro's recovery path advances to a boundary token (e.g. `Eof`, `RBrace`) and then `continue`s, it re-enters the inner loop, calls the macro again, sees the same boundary token, skips the recovery loop (no advance), and `continue`s again. Infinite loop.

**The invariant**: every parser loop must **advance at least one token OR exit (break/return) on every iteration**. A zero-advance path that does not `break` or `return` is always a hang.

**Detection**: any `loop { some_macro_with_continue!(); break; }` pattern in parser code is suspect. The correct alternative is a real method (`fn parse_one_thing(&mut self) -> Option<T>`) that returns `None` on failure (already recovered) instead of `continue`-ing.

**Root incident**: `parse_struct_lit_fields` used `loop { parse_one_field!(); break; }` in three places. The macro contained two `continue` paths. On malformed input (e.g. `` {`name:`a`} `` producing an unterminated backtick string + weird token sequence), the compiler hung indefinitely. Fixed by extracting `parse_struct_lit_one_field(&mut self) -> Option<StructLitField>` and replacing all `loop { macro!(); break; }` with `if let Some(f) = self.parse_struct_lit_one_field() { fields.push(f); }`.

**Bouncer checks**:
- [ ] For diffs touching `crates/ynz-parser/src/parser.rs`: grep added lines for `loop {` followed within 3 lines by a macro invocation ending in `!()` and `break;`. Match → WARNING: verify the macro contains no `continue` or that all `continue` paths advance at least one token first.
- [ ] For any new parser macro (`macro_rules!`) containing `continue`: flag as high risk unless the macro is ONLY used in the outermost loop context where `continue` is intended.

**Severity**: critical — parser hang means the compiler never exits, which users experience as a freeze with no diagnostic output. This is the worst possible failure mode: silent, unrecoverable, no error message.

---

## Scattered Registry Without SSOT Link — 2026-05-19

**Scope**: `crates/ynz-diagnostics/src/`, `crates/ynz-typeck/src/`, `crates/ynz-parser/src/` — new `pub const` or `pub static` string-array definitions that represent feature inventories (keyword lists, jargon lists, method-name lists, etc.).

**Exemption**:
- Definitions marked `#[cfg(test)]` (test-only intrinsics — registry is for production surface only)
- Definitions with `// CARVE-OUT: <reason>` on the definition line (explicitly declared legitimate parallel registries per `design/feature-registry.md` "Carve-Outs" section)
- Definitions that are lookup tables OVER the registry's generated output (e.g., a perf-critical wrapper that caches registry data)

**Last verified**: 2026-05-19 — pattern tested against current codebase. Matches: `crates/ynz-diagnostics/src/banned_jargon.rs:21` (migrating in Phase 2) and `crates/ynz-typeck/src/builtins.rs:101` (migrating in Phase 3, undiscovered in original research). Zero false positives in other files in those crates.
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-(diagnostics|typeck|parser)/src/
pub (const|static).*&\[
&\[&str\]
```


**Cause**: v0.1.0 shipped with feature inventories scattered across 7+ locations. Adding `int.max` in M4 P5 touched five locations that nothing enforced to stay in sync. The v0.2 LSP (M2) needs these inventories at IDE-keystroke latency — reading from 7 separate Rust files is not tenable. v0.2-M1 builds the SSOT registry to fix the class. This entry prevents new scattered registries from appearing post-M1.

**Detection signature**: A new `pub const NAME: &[&str] = &[...]` or `pub static NAME: &[&str] = &[...]` definition in `crates/ynz-{diagnostics,typeck,parser}/src/` that:
- Does NOT have `#[cfg(test)]` on the preceding line or the definition line itself
- Does NOT have a `// CARVE-OUT:` comment within 3 lines above the definition

**Constraint**: All new user-facing feature inventories go in `registry/features.toml` first. Code (Rust constant, adapter function) is derived from the registry, not the other way. See `design/feature-registry.md` for the schema and `.claude/rules/feature-registry.md` for the entry-type checklist.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each diff line adding `pub const.*&\[.*&str\].*=.*&\[` or `pub static.*&\[.*&str\].*=.*&\[` in `crates/ynz-diagnostics/src/`, `crates/ynz-typeck/src/`, or `crates/ynz-parser/src/`: check that within the 3 lines ABOVE the definition (in the same diff context) there is either `#[cfg(test)]` or `// CARVE-OUT:`. Missing either → WARNING: "New string-array registry detected without SSOT link — add to registry/features.toml or annotate // CARVE-OUT: <reason>."
- [ ] For each diff adding `pub const` or `pub static` matching the above pattern: additionally grep the diff for a corresponding `[[` TOML entry in `registry/features.toml` within the same PR. Missing → WARNING: "Registry entry not found for new constant — was this added to registry/features.toml?"

**Severity**: warning (the pre-M1 code is being migrated; the Bouncer prevents NEW drift from being introduced post-M1).

**Originating incident**: 2026-05-19 — v0.2-M1 planning revealed that `int.max` (M4 P5) touched 5 separate registry locations with no enforced sync. The Explore agent research found 7 scattered registries; manual grep found an 8th (`STRING_METHODS` in `builtins.rs:101`) missed in the initial research scan. v0.2-M1 builds the fix; this entry makes the fix self-defending.

---

## M8 Modules Shipped Untested — Three Infrastructure Bugs — 2026-05-18/19

**Scope**: `crates/ynz-driver/src/` — any build/load infrastructure change that touches multi-file project loading or path resolution.
**Last verified**: 2026-05-19
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-driver/src/(load|build)\.rs$
yinz\.toml
find_project_root
load_project
ImportDecl
```

**Cause**: M8 Phase 2 (modules) shipped import/export grammar + multi-file driver wiring but wrote zero integration tests for the project-load path. Three infrastructure bugs shipped:

1. **`src/` directory hard-requirement** (`load.rs`): `load_project()` errored if `root/src/` didn't exist, breaking any project without that exact layout. Fixed by falling back to walking the project root when `src/` is absent.

2. **Empty project root from relative path** (`build.rs`): `find_project_root()` walked up relative paths and returned `""` when `yinz.toml` was in the current working directory. `build_project("")` then called `read_dir("")` → "No such file or directory". Fixed by canonicalizing the source path to absolute before root detection.

3. **Typeck cross-file resolution entirely unimplemented** (separate graveyard note): `Item::ImportDecl(_) => {}` meant imports compiled but were silently ignored in all type checking and codegen.

**Detection**: all three bugs would have been caught by one integration test: a two-file project where file B imports a type from file A and the test asserts `ynz run B` produces the expected output. This test was listed in the M8 P2 plan but never written.

**Constraint**: Any compiler phase that ships multi-file or cross-module functionality MUST include an integration test that:
1. Creates a temporary two-file project on disk (with `yinz.toml` at the root)
2. Has one file export a type and a second file import and use it
3. Runs `ynz run` on the entry file
4. Asserts the binary produces the expected stdout

**Bouncer checks**:
- [ ] For diffs touching `crates/ynz-driver/src/load.rs` or `crates/ynz-driver/src/build.rs`: check that `crates/ynz-driver/tests/integration.rs` has at least one test with `yinz.toml` creation + two-file import. No such test → WARNING.
- [ ] For plan files claiming "multi-file" or "module" or "import/export": verify `### Demo & Error Gallery` includes a two-file cross-import demo in `examples/`. Missing → WARNING.

**Severity**: critical — infrastructure bugs that produce non-obvious error messages ("No src/ directory", empty path crashes) with no clear path to the fix without reading source code.

**Originating incident**: M8 P2 shipped 2026-05-18. All three bugs discovered 2026-05-19 during first real-world use (trading-v4 project). Total fix time: ~2 hours.

---

## ynz watch Hardcoded `cc` Linker — 2026-05-20

**Scope**: `crates/ynz-watch/src/rebuild.rs` — any code that spawns the linker from the watch rebuild path.
**Last verified**: 2026-05-20
**Category**: regex

*(Direct-fire: the literal `Command::new("cc")` pattern in ynz-watch is unambiguously the bug. Other hardcoded linker names matched here would also be wrong. No legit reason to do this — `find_linker` exists. The bar for `regex` is high; this earns it because the pattern is a literal anti-pattern with no exemption.)*

**Pre-filter patterns**:
```
crates/ynz-watch/.*\.rs$
Command::new\("cc"\)
Command::new\("gcc"\)
Command::new\("g\+\+"\)
Command::new\("ld"\)
```

**Cause**: `write_binary()` in `rebuild.rs` hardcoded `Command::new("cc")`. The existing `ynz build` path (`crates/ynz-driver/src/build.rs`) already had a `find_linker()` probe that tries `clang-18`, `clang`, `cc`, `gcc`, `g++` in order. Watch was written independently and didn't reuse it. Devcontainers with LLVM/clang-18 installed but not `cc` (no build-essential) hit `No such file or directory` on every non-check rebuild.

**Detection**: `ynz watch` produces `could not invoke linker \`cc\`: No such file or directory` on a container that has `clang-18` but not `cc`.

**Constraint**: Any new code path that invokes the system linker MUST use `find_linker()` from `crates/ynz-driver/src/build.rs` (or an equivalent probe). Never hardcode `"cc"`. The probe order is `["clang-18", "clang", "cc", "gcc", "g++"]` — `clang-18` is first because it ships with LLVM 18 which is already required to run ynz.

**Bouncer checks**:
- [ ] For diffs touching any file under `crates/ynz-watch/` that adds `Command::new`: check that it does NOT contain the literal `Command::new("cc")`. Match → BLOCK.
- [ ] For diffs that add a new linker invocation anywhere: verify `find_linker` or equivalent probe is used. Hardcoded linker names (`"cc"`, `"gcc"`, `"g++"`, `"ld"`) → WARNING.

**Severity**: high — `ynz watch` (default mode, not `--check`) silently fails to spawn the compiled program on every rebuild in any container that has LLVM installed but not build-essential.

**Originating incident**: Discovered 2026-05-20 during first real-world `ynz watch` use on trading-v4 devcontainer.
