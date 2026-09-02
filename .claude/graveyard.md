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

**Scope**: `.claude/planning/{active,paused,done}/<plan-id>/plan.md` where `<plan-id>` (the directory name) contains a milestone marker `m[0-9]+` for milestones M4 and later. Plans for M1, M2, M3 are exempt (predate the rule). (Migrated 2026-07-01 from the pre-migration `.claude/plans/(active|done)/m[0-9]+-*.md` flat-file scope — plans are now nested `<plan-id>/plan.md` directories, not files named after the milestone.)
**Exemption**:
- Pre-M4 plan files (M1, M2, M3 — these predate the plan-invariants rule)
- Plans explicitly marked `## Invariants This Milestone Must Preserve` with all required subsections containing the const semantics
- Cross-cutting plans not tied to a specific milestone (e.g., the `design-lockdown-from-gemini-review` plan itself, which is what created this entry)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/planning/(active|paused|done)/[0-9]{4}-[0-9]{2}-[0-9]{2}-.*m[0-9]+.*/plan\.md$
ownership
const binding
borrow checker
```


**Cause**: The M3 plan stated "ownership system arrives in M4" without enumerating which call-site operations `const` blocks (`.lend`/`.give`), or which LLVM attributes M4 codegen must emit (`readonly`). The gap WAS real and would have shipped a less-safe + less-performant M4. Gemini's 2026-05-14 code review surfaced it before it shipped.

**Detection signature**: An M4-or-later milestone plan file mentions any of `ownership`, `const`, `let`, `.lend`, `.give`, `.share` in its body but does NOT contain a literal `## Invariants This Milestone Must Preserve` heading AND a `### Safety` subheading enumerating const semantics (cannot reassign, cannot be lent, cannot be given, field mutation blocked) AND a `### Performance` subheading naming LLVM `readonly` / `noalias` attribute emission.

**Constraint**: Every M4+ milestone plan that touches ownership, types, or the `const`/`let` distinction MUST include `## Invariants This Milestone Must Preserve` with `### Safety` enumerating the five paths const blocks (reassignment, `.lend`, `.give`, field mutation, mutable inference) AND `### Performance` naming the LLVM attribute contract (`readonly` on share params and params figured-out from const bindings; `noalias` where ownership rules prove non-aliasing). See [`.claude/rules/plan-invariants.md`](rules/plan-invariants.md) for the full required subsection list.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each `plan.md` diff under `.claude/planning/{active,paused,done}/<plan-id>/`: extract the milestone number from the `<plan-id>` directory name (or the `legacy.milestone`/`legacy.slug` frontmatter field for migrated plans). If number >= 4, grep the file body for `^## Invariants This Milestone Must Preserve$`. Missing → CRITICAL.
- [ ] For each M4+ plan file with the heading present: check the `### Safety` subsection (lines between `^### Safety$` and the next `^### ` or `^## `) contains ALL FIVE substrings: `cannot be reassigned`, `cannot be lent`, `cannot be given`, `field mutation`, AND (case-insensitive) one of `readonly`, `noalias`, `LLVM` somewhere in the Performance subsection. Missing any → CRITICAL.
- [ ] If the plan mentions `ownership system`, `borrow checker`, or `const binding` in its body but lacks a `## Invariants This Milestone Must Preserve` heading entirely → CRITICAL.

**Severity**: critical (this is the spine of the language's safety + performance contract; if it's not stated, M4 ships without enforcing it).

**Originating incident**: 2026-05-14 — Gemini code review during the `design-lockdown-from-gemini-review` plan's discovery phase asked "does Yinz enforce const like Rust enforces non-mut?" Investigation found const reassignment was enforced in M2 typeck (`crates/ynz-typeck/src/check.rs:264`) but no design doc stated the full invariant chain (block `.lend`/`.give`/field-mutation + emit LLVM `readonly`). The 2-hour conversation that resulted produced this plan.

---

## Plan Contradicts a Governing Design Doc, Caught By No Review — 2026-05-31

**Scope**: `.claude/planning/active/<plan-id>/plan.md` and `.claude/planning/paused/<plan-id>/plan.md` (execution plans — `legacy.type: execution` or `metadata.type: plan` with phases). Roadmaps + done plans exempt from the section requirement (done plans are historical; roadmaps don't have phases). (Migrated 2026-07-01 — see the const-immutability entry above for the same path-shape update.)
**Exemption**:
- Plans containing a `## Design-Doc Alignment` section that cites the governing `/design/` doc(s) and either confirms match OR enumerates each divergence as "design doc X says A; plan does B because <reason>" with sign-off.
- Plans where `/design/` genuinely has no relevant doc AND the `## Design-Doc Alignment` section states that explicitly.
- Pure bugfix/cleanup/refactor plans that implement no new design surface (still benefit from the section but it may legitimately say "no design doc governs a bugfix; restoring documented behavior").
**Last verified**: 2026-05-31
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/planning/(active|paused)/[0-9]{4}-[0-9]{2}-[0-9]{2}-.*/plan\.md$
block_on
bridge
defer.*to (M|v0\.)[0-9]
transitive
may-block
no.*coloring
```

**Cause**: The v0.3-M2 plan shipped a `block_on` sync bridge as its "no-coloring delivery mechanism." [`docs/internal/implementation/IMP-no-function-coloring.md`](../docs/internal/implementation/IMP-no-function-coloring.md) ("Concurrency — No Function Coloring") documents the actual model: whole-program TRANSITIVE may-block analysis up the call graph + auto-inserted `wait` at every suspension point + stackless state machines; FFI declares `may-block` (the only "can't infer → user" case). **There is no bridge in the design.** The bridge was invented to fill the gap created by cutting the M2/M3 milestone boundary at the wrong line (state machines in M2, may-block analysis deferred to M3 — but a correct state-machine layer REQUIRES the analysis). It both crashed at runtime (the HALT) and contradicted the documented design. Three rounds of adversarial plan-review + a P0 spike gate + five per-phase 4-agent review gates all PASSED it — because every review checked the plan against ITSELF (internal consistency, AC evidence, rule violations), never against the design doc it was violating.

**Detection signature**: (1) An execution plan file lacks a `## Design-Doc Alignment` section. OR (2) A plan defers a capability to a later milestone (`defer ... to M3`, `... is M3`, "deferred to v0.x") where that capability is named load-bearing for the CURRENT milestone in a `/design/` doc. OR (3) A plan introduces a runtime mechanism (`block_on`, sync bridge, thread-hold) for a behavior a `/design/` doc says is resolved at COMPILE time (inference / whole-program analysis).

**Constraint**: Every execution plan MUST include `## Design-Doc Alignment` citing the governing `/design/` doc(s) and confirming match or enumerating each divergence with sign-off (see [`.claude/rules/plan-invariants.md`](rules/plan-invariants.md) "Design-Doc Alignment"). Plan-review (Step 7) and per-phase review (Step 9a) MUST diff the plan/diff against the cited design docs, not only against the plan's internal consistency. A plan contradicting a design doc is a BLOCK with the citation regardless of internal consistency — surfaced as "design doc X says A; plan says B."

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each added/modified `.claude/planning/{active,paused}/<plan-id>/plan.md` that is an execution plan (`legacy.type: execution` or absent): grep the body for `^## Design-Doc Alignment$`. Missing → WARNING.
- [ ] If a plan body contains a deferral phrase (`defer.*to (M|v0\.)[0-9]`, `is (M[0-9]|v0\.[0-9])`, `deferred to`) for a capability, the `## Design-Doc Alignment` section MUST acknowledge whether that deferral is documented in the roadmap/mvp-scope or invented by the plan. Deferral phrase present + no Design-Doc Alignment acknowledgment → WARNING.
- [ ] If a plan introduces `block_on` / "sync bridge" / "hold the thread" / "blocking pool" language for a behavior, and [`docs/internal/implementation/IMP-no-function-coloring.md`](../docs/internal/implementation/IMP-no-function-coloring.md) (or any cited design doc) describes that behavior as compile-time-inferred → judgment BLOCK, cite "design doc says inferred-at-compile-time; plan introduces a runtime block".

**Severity**: critical (a plan that contradicts the governing design ships the WRONG language; this one cost a halted milestone + a full re-plan).

**Originating incident**: 2026-05-31 — during the v0.3-M2 re-spike + re-plan, Patrick asked "if `wait` is inferred at compile time, why is anything slower at runtime?" and "did we have this documented?" Reading [`docs/internal/implementation/IMP-no-function-coloring.md`](../docs/internal/implementation/IMP-no-function-coloring.md) (which the `/plan` research step had skipped — only the codebase was mapped) revealed the doc had the correct transitive-inference / no-bridge model all along. The bridge was a plan invention contradicting it. See [`.claude/planning/done/2026-05-30-v0-3-m2-wait-and-state-machines/plan.md`](planning/done/2026-05-30-v0-3-m2-wait-and-state-machines/plan.md) Findings Log "🔴 VERIFIED ROOT CAUSE."

---

## Requiring Explicit Ownership Annotation at Call Sites — 2026-05-14

**Scope**: `spec/*.md` and `design/*.md`, except `design/future/*` (parking-lot speculation only).
**Exemption**:
- Function signature documentation — signatures correctly declare `share`/`lend`/`give` at the parameter level. The anti-pattern is requiring annotation at the CALL site, NOT at the signature.
- Documentation explicitly describing the inverse-anti-pattern as wrong (cites this entry or [`.claude/rules/inference.md`](rules/inference.md)).
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

**Constraint**: Spec and design docs describe call-site ownership as INFERRED-WITH-MUTED-IDE-HINT (per [`.claude/rules/inference.md`](rules/inference.md)). Function SIGNATURES correctly require explicit `share`/`lend`/`give` declarations; CALL SITES infer. Explicit `.share`/`.lend` typing at call sites is documented as AVAILABLE for clarity, NEVER REQUIRED.

**Bouncer checks**:
- [ ] For each diff to `spec/*.md` or `design/*.md` (excluding `design/future/*`): grep added lines (case-insensitive) for `must annotate at`, `required at call site`, `must declare.*at every call`, `explicit annotation.*at the call site`. Any match → WARNING.
- [ ] Co-occurrence check: grep added lines for `\.share|\.lend|\.give` followed within 5 lines by `must` or `required`. If both present AND NO occurrence within the same 10-line window of `signature` or `at the definition` (which are the legitimate "explicit at signature" context) → WARNING.

**Severity**: warning (re-relitigation risk; not a runtime safety violation).

**Originating incident**: 2026-05-14 — during the design-lockdown conversation, Patrick and Claude initially considered requiring explicit `.share`/`.lend` at call sites for teaching visibility. Rejected in favor of uniform inference + IDE muted hints (which preserves teaching value without the syntactic burden). Documented in [`.claude/rules/inference.md`](rules/inference.md) "Inverse Anti-Pattern" section. The spec file [`docs/reference/REF-ownership.md`](../docs/reference/REF-ownership.md) previously had a "smart defaults at call sites" section that was rewritten to the inferred-with-hints framing in Phase 3 of the plan.

---

## M4+ Milestone Plans Missing the 5-Subsection Invariants Structure — 2026-05-14

**Scope**: `.claude/planning/{active,paused,done}/<plan-id>/plan.md` where `<plan-id>` contains a milestone marker `m[0-9]+`, for M4 and later. M1, M2, M3 exempt (predate the rule). (Migrated 2026-07-01 — same path-shape update as the const-immutability entry above.) This is a STRUCTURAL check (does the section exist? do all 5 subsections exist?), distinct from the const-deep-immutability entry above which checks specific CONTENT.
**Exemption**:
- Pre-M4 milestone plans
- Plans not associated with a specific milestone (cross-cutting plans, design plans, refactoring plans)
- Plans where the section structure is present but a subsection is legitimately empty because nothing applies (rare — must include "(none for this milestone)" or similar text to distinguish from forgotten)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/planning/(active|paused|done)/[0-9]{4}-[0-9]{2}-[0-9]{2}-.*m[0-9]+.*/plan\.md$
^## Invariants This Milestone Must Preserve$
```


**Cause**: Without mechanical enforcement of the 5-subsection structure, future plans will skip subsections. The const-deep-immutability gap that triggered this whole project IS an example of this kind of structural omission in real life — the M3 plan had no Invariants section at all.

**Detection signature**: An M4-or-later milestone plan file contains `## Invariants This Milestone Must Preserve` but is missing one or more of the required 5 subsections: `### Safety`, `### Performance`, `### Teaching`, `### Runtime Dependencies`, `### Kernel-Mode Behavior`. Also: a present subsection has no meaningful content within 10 lines of its header.

**Constraint**: Every M4+ milestone plan with `## Invariants This Milestone Must Preserve` MUST contain all 5 named subsections, each non-empty (at least one non-blank, non-heading line within 10 lines of the subsection heading). See [`.claude/rules/plan-invariants.md`](rules/plan-invariants.md) for what each subsection should contain.

**Bouncer checks**:
- [ ] For each M4+ plan file in the diff that contains `^## Invariants This Milestone Must Preserve$`: extract the section body (between that line and the next `^## ` or EOF). Within that body, grep for each of `^### Safety$`, `^### Performance$`, `^### Teaching$`, `^### Runtime Dependencies$`, `^### Kernel-Mode Behavior$`. Missing any → WARNING.
- [ ] For each subsection that IS present: verify at least one non-blank, non-heading line within 10 lines after the subsection header. Empty section → WARNING per empty.

**Severity**: warning (structural enforcement; CRITICAL safety+performance invariants covered by the const-deep-immutability entry).

**Originating incident**: 2026-05-14 — same originating incident as the const-deep-immutability entry. The 5-subsection structure was designed during plan recovery to mechanically enforce that every dimension (safety, performance, teaching, runtime, kernel-mode) is at minimum considered for every M4+ milestone.

---

## Language or Stdlib Features Without Runtime + Kernel-Mode Declaration — 2026-05-14

**Scope**: `.claude/planning/active/<plan-id>/plan.md` and `.claude/planning/done/<plan-id>/plan.md` for plans dated 2026-05-15 or later (one day after this entry lands). Includes both milestone plans AND cross-cutting plans. (Migrated 2026-07-01 — same path-shape update as the const-immutability entry above. `roadmap.md`/`capability-ledger.md` companion files never carry an Invariants section and are out of this entry's scope by construction — they're not `plan.md`.)
**Exemption**:
- Plans whose `files:` front-matter does NOT include `crates/**` (pure docs/rules plans don't add features)
- Plans dated before 2026-05-15 (rule applies forward from when it lands)
- Plans that are explicitly documentation-only (no language/stdlib feature added)
**Last verified**: 2026-05-14
**Category**: regex+judgment

**Pre-filter patterns**:
```
\.claude/planning/(active|done)/[0-9]{4}-[0-9]{2}-[0-9]{2}-.*/plan\.md$
crates/
^### Runtime Dependencies$
^### Kernel-Mode Behavior$
```


**Cause**: The `--kernel` mode design (see [`docs/internal/implementation/IMP-no-runtime-mode.md`](../docs/internal/implementation/IMP-no-runtime-mode.md)) for chipset/NASA/embedded targets requires every Yinz language and stdlib feature to declare its runtime dependencies (heap allocator? scheduler? OS I/O? none?) and kernel-mode behavior (compile error? works with user-provided primitive? always works?). Without enforcement, features ship with hidden heap dependencies and the v0.3 kernel-mode work hits a wall trying to retroactively analyze every feature.

**Detection signature**: A plan file dated 2026-05-15+ with `files:` matching `crates/**` (i.e., adds compiler or stdlib features) does NOT contain `### Runtime Dependencies` AND `### Kernel-Mode Behavior` subsections within its Invariants section. These are required parts of the 5-subsection structure but called out separately because they're the KERNEL-specific check.

**Constraint**: Any plan adding language or stdlib features must declare runtime dependencies and kernel-mode behavior in the `### Runtime Dependencies` and `### Kernel-Mode Behavior` subsections of `## Invariants This Milestone Must Preserve`. Each lists per-feature: what the feature depends on at runtime, and what happens in `--kernel` mode (compile error with which message? plug-in API? always works?). See [`.claude/rules/plan-invariants.md`](rules/plan-invariants.md) and [`docs/internal/implementation/IMP-no-runtime-mode.md`](../docs/internal/implementation/IMP-no-runtime-mode.md).

**Bouncer checks**:
- [ ] For each `plan.md` diff dated 2026-05-15+ (via `created_at`/`legacy.created`) with `legacy.files:` containing `crates/`: grep file body for `^### Runtime Dependencies$` AND `^### Kernel-Mode Behavior$`. Missing either → WARNING.

**Severity**: warning (forward-compat hygiene; kernel-mode is a v0.3+ target so consequences are forward-looking).

**Originating incident**: 2026-05-14 — Patrick stated during the design-lockdown conversation that chipset code, kernel modules, and NASA-grade embedded systems are target use cases. Without per-feature runtime-dependency declarations, the v0.3 `--kernel` implementation would have to retroactively analyze every M3-M8 feature for hidden allocator/scheduler dependencies. Forward-declaring is cheap; retroactive analysis is expensive.

---

## Re-Introducing Try/Catch / Recover Blocks After Rejection — 2026-05-14

**Scope**: `crates/ynz-parser/src/lexer.rs`, `crates/ynz-parser/src/token.rs`, `crates/ynz-parser/src/parser.rs`, `crates/ynz-ast/src/nodes.rs`, `spec/*.md`, `design/*.md`. Excludes [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../docs/internal/scratchpad/SCRATCH-future-panic-safety.md) (which DOCUMENTS the rejection rationale).
**Exemption**:
- [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../docs/internal/scratchpad/SCRATCH-future-panic-safety.md) — this file documents WHY try/catch was rejected; it must mention the syntax to do so
- Documentation that explicitly cites this entry or [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../docs/internal/scratchpad/SCRATCH-future-panic-safety.md) as the rejection rationale (e.g., "Yinz rejects try/catch — see X")
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
- In spec/design docs (excluding [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../docs/internal/scratchpad/SCRATCH-future-panic-safety.md)): added lines introducing `try {` or `catch (` or `recover (` syntax inside a Yinz code block as legal syntax (not as "this is rejected" demonstration).

**Constraint**: Yinz panic handling uses `errors` keyword for KNOWN failures (auto-propagate) + task-isolation via `background` + supervisor pattern at the task boundary for UNKNOWN panics. NO try/catch/recover syntax at the language level. See [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../docs/internal/scratchpad/SCRATCH-future-panic-safety.md) for the full rationale and rejected alternatives.

**Bouncer checks**:
- [ ] For diff entries touching `crates/ynz-parser/src/lexer.rs`, `crates/ynz-parser/src/token.rs`, `crates/ynz-parser/src/parser.rs`, or `crates/ynz-ast/src/nodes.rs`: grep added (non-comment) lines for `\bTry\b`, `\bCatch\b`, `\bRecover\b` (excluding lines containing `// banned`, `// rejected`, or matching banned-keyword test fixture patterns). Any match → CRITICAL.
- [ ] For diff entries touching `spec/*.md` or `design/*.md` (use pathspec exclusion `:!docs/internal/scratchpad/SCRATCH-future-panic-safety.md` to exclude the rationale file properly — NOT `grep -v` which only strips lines, leaving offending content visible): grep added lines for ` try \{`, ` catch \(`, ` recover \(` patterns. Any match → WARNING.

**Severity**: critical for compiler source changes (would land in the compiler if merged); warning for docs changes (re-relitigation risk only).

**Originating incident**: 2026-05-14 — earlier in the design-lockdown conversation, Claude proposed `try { } recover (e: Panic) { }` blocks for explicit panic recovery in scope. Patrick correctly identified this as try/catch under a different name and rejected it. The supervisor pattern at the task boundary handles every legitimate use case (per-request isolation in HTTP servers, per-job isolation in queue workers, per-order isolation in trading bots). Adding a second recovery mechanism would violate Yinz's "one concept = one keyword" principle and re-introduce all the problems Java/Python have with try/catch (catch-and-silently-continue, exception-as-flow-control, etc.). See [`docs/internal/scratchpad/SCRATCH-future-panic-safety.md`](../docs/internal/scratchpad/SCRATCH-future-panic-safety.md) for the full design rationale.

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
- Definitions with `// CARVE-OUT: <reason>` on the definition line (explicitly declared legitimate parallel registries per [`docs/internal/implementation/IMP-feature-registry.md`](../docs/internal/implementation/IMP-feature-registry.md) "Carve-Outs" section)
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

**Constraint**: All new user-facing feature inventories go in [`registry/features.toml`](../registry/features.toml) first. Code (Rust constant, adapter function) is derived from the registry, not the other way. See [`docs/internal/implementation/IMP-feature-registry.md`](../docs/internal/implementation/IMP-feature-registry.md) for the schema and [`.claude/rules/feature-registry.md`](rules/feature-registry.md) for the entry-type checklist.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each diff line adding `pub const.*&\[.*&str\].*=.*&\[` or `pub static.*&\[.*&str\].*=.*&\[` in `crates/ynz-diagnostics/src/`, `crates/ynz-typeck/src/`, or `crates/ynz-parser/src/`: check that within the 3 lines ABOVE the definition (in the same diff context) there is either `#[cfg(test)]` or `// CARVE-OUT:`. Missing either → WARNING: "New string-array registry detected without SSOT link — add to registry/features.toml or annotate // CARVE-OUT: <reason>."
- [ ] For each diff adding `pub const` or `pub static` matching the above pattern: additionally grep the diff for a corresponding `[[` TOML entry in [`registry/features.toml`](../registry/features.toml) within the same PR. Missing → WARNING: "Registry entry not found for new constant — was this added to registry/features.toml?"

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

---

## Parallel Per-Type Dispatch / Flat-Scan Re-Derivation in Suspension Codegen — 2026-06-04

**Scope**: `crates/ynz-codegen/src/emit.rs` (frame flush/reload for crossing locals + loop vars) and `crates/ynz-typeck/src/check.rs` (the crossing-analysis consumers — `wait`-position guards). The disease is "a second hand-rolled per-type/per-position dispatch that parallels the authoritative one and drifts from it."
**Exemption**:
- A new `flush_*`/`reload_*` that is a **thin delegating wrapper** around `flush_var_slot_to_frame` / `reload_params_from_frame` (e.g. `flush_for_loop_var` after the round-5 unification — it just forwards).
- Reads of the crossing set that go through the authoritative producers (`crossing_local_names` / `locals_crossing_wait` / the synthetic `collect_for_loop_synthetic_crossings_*` outputs) rather than re-deriving membership.
- A genuinely new value-type branch ADDED INSIDE the single `flush_var_slot_to_frame` dispatch (extending the one dispatch is correct; forking a second one is the bug).
**Last verified**: 2026-06-04
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-codegen/src/emit\.rs
crates/ynz-typeck/src/check\.rs
flush_var_slot_to_frame
flush_for_loop_var
reload_params_from_frame
declared.*before.*wait
crossing
to_i64_bits
```

**Cause**: M3a frame-backing was first written as TWO parallel hand-rolled per-type dispatches — the crossing-local flush AND a separate `flush_for_loop_var` — each `match`-ing on the value type to store it into the composed frame slot. They DRIFTED: `number` (decimal128, 2-slot i128) was handled in one but not the other; `shape`/`string`/`options` branches were missing or wrong in the second → values stored/reloaded as 8-byte i64 garbage or stack-dangling pointers, producing SILENT wrong output (`0.000`, stack garbage) across a suspension. Whack-a-mole: each round fixed one type in one dispatch; ~10 silent-miscompile rounds before the root fix (round 5) unified both into the single `flush_var_slot_to_frame`, with `reload_params_from_frame` mirroring the identical N-way dispatch (store/load symmetric by construction). **Second instance, same milestone**: the `ArrayShapeRuntimeFieldWithWait` guard added `is_let_declared_before_wait_in_stmts`, a FLAT statement-order scan that RE-DERIVED "is this let declared before the wait" instead of consuming the crossing-analysis set the compiler already computes. Deleting it → under-rejection; keeping it → over-rejection; resolved only by consuming the authoritative `crossing_names` set. Both are `no-duct-tape.md` #7 (parallel implementation that drifts) specialized to suspension codegen, where the drift is invisible (silent-wrong, no crash).

**Detection signature**: (1) A second function in `emit.rs` that `match`es on value type (`Type::Int|Bool|Float|Number|String|Shape|...`) and performs frame store/GEP (`build_store`, `build_gep`, `FRAME_OFFSET_LOCALS_START`, `to_i64_bits`) for a crossing local or loop var, living OUTSIDE `flush_var_slot_to_frame`/`reload_params_from_frame`. (2) A new helper that walks a statement list to decide whether a binding is "declared before a `wait`" / crosses a suspension, instead of reading `crossing_local_names`/`locals_crossing_wait`/the synthetic crossings. (3) A `flush`/`reload` change that edits ONE dispatch's type-match without the symmetric edit to its partner (store without reload, or vice versa).

**Constraint**: There is ONE frame-flush dispatch (`flush_var_slot_to_frame`) and ONE reload dispatch (`reload_params_from_frame`); every crossing-local AND loop-var flush routes through them, and any new value-type slot handling is a branch ADDED INSIDE that single dispatch (store + reload edited together). The set of "what crosses a suspension / is declared before a `wait`" is produced ONCE by the crossing analysis — consume `crossing_local_names`/`locals_crossing_wait`/the synthetic crossing collectors; never re-derive membership with a parallel flat statement scan. A suspension-codegen change that fails any acceptance check must be re-tested across the FULL per-type × per-position adversarial matrix on the live binary (silent-wrong has no crash to catch it).

**Bouncer checks** (each runnable as shell against a diff):
- [ ] Diff to `emit.rs` adds a function (not a thin wrapper over `flush_var_slot_to_frame`) that `match`es `Type::` variants AND calls `build_store`/`build_gep`/`to_i64_bits` with `FRAME_OFFSET_LOCALS_START` → WARNING (parallel flush dispatch — route through `flush_var_slot_to_frame`).
- [ ] Diff adds a helper whose name or body re-derives crossing/declared-before-wait membership (name matches `before.*wait|declared_before|is_let_.*wait`, body iterates stmts testing for `Wait`) → WARNING (re-derives the crossing set; consume `crossing_local_names`/`locals_crossing_wait`).
- [ ] Diff edits the type-match inside `flush_var_slot_to_frame` (add/remove/change a `Type::` arm) WITHOUT a corresponding edit to `reload_params_from_frame` in the same diff → WARNING (store/reload asymmetry → reload reads what store never wrote).

**Severity**: critical — silent-wrong codegen across a suspension is the worst failure class (no crash, no panic; the program runs and prints the wrong number). This exact disease cost ~10 silent-miscompile rounds in M3a P1+P3.

**Originating incident**: 2026-06-04, v0.3-M3a suspension codegen. Two parallel per-type flush dispatches drifted (decimal128/shape/string/options branches present in one, missing/wrong in the other) → `0.000`/stack-garbage across `wait` suspensions; ~10 whack-a-mole rounds. Root fix round 5: unify into `flush_var_slot_to_frame` + symmetric `reload_params_from_frame`; `flush_for_loop_var` became a thin wrapper. Second instance: `is_let_declared_before_wait_in_stmts` flat-scan re-derived the crossing set → under/over-rejection on the `ArrayShapeRuntimeFieldWithWait` guard; fixed by consuming the authoritative `crossing_names`. The cumulative Opus code-reviewer that finally certified the milestone called it "the unified flush killed the hydra." See [`.claude/planning/done/2026-06-01-v0-3-m3a-suspension-codegen/plan.md`](planning/done/2026-06-01-v0-3-m3a-suspension-codegen/plan.md) Phase 1/3 Findings Logs.

**Escalation verdict — 2026-07-03 (v0.3-M4 AAR):** this corpse and its sibling rule [`.claude/rules/authoritative-derivation.md`](rules/authoritative-derivation.md) recurred **4× within the single v0.3-M4 plan execution**, despite that rule being in-context and cited every time: the `SuspendSet` twin (P0), a latent `check.rs` twin (P2), a `DEFAULT_CHANNEL_CAPACITY` twin (P5 — a real BLOCKER), and a `lock_or_recover` twin (caught by the cumulative cross-phase gate). Run against [`~/.claude/rules/corpse-recurrence-escalation.md`](../../.claude/rules/corpse-recurrence-escalation.md)'s two-question test: (Q1) the sibling rule was genuinely in-context throughout — yes; (Q2) every one of the four was caught only downstream by a gate/reviewer, never by the actor's own in-context pre-flight self-check — yes. Both yes = **lever failure, not a wording gap**, and 4× within one plan clears the escalation floor (2× within one plan) decisively. Per that rule: this is recorded here (the auditable incident-history note) and routed via the AAR handoff to the roadmap's Capability Ledger as a **write-time / phase-boundary hook** candidate — a mechanical backstop that greps a new derivation of an already-authoritative question against the existing producers, sitting in front of the reactive auditor stage that already reads this corpse. It is explicitly **NOT** fixed by re-wording `authoritative-derivation.md` a further time (two wording passes already failed to hold the lever), and the hook is NOT authored here — `rule-author`'s charter stops at classifying and recording this call; `hook-author` designs and builds the backstop in its own session.

---

## Injected Resolver Dead via Memo-Cache Ordering — Test Green on Fallback Coincidence — 2026-06-06

**Scope**: `crates/ynz-codegen/src/emit.rs` (`build_frame_layouts_with_resolver` / `compute_frame_size` and any frame-size memo) + `crates/ynz-codegen/tests/*.rs` (tests asserting resolver/callback-driven layout values). The disease is broader than frame layout: ANY function that BOTH memoizes a recursive computation AND accepts an injected resolver/closure to supply values for some keys, where the resolver-seed runs AFTER the memo could have cached a fallback for those keys.
**Exemption**:
- The resolver SEEDS the memo BEFORE the recursive/memoizing pass runs (e.g. a pre-seed loop that `sizes.insert(name, resolver(name))` for all injected-key names, THEN `compute_frame_size` reads the seeded values on cache-hit). This is the correct ordering.
- The memoizer itself consults the resolver on a cache MISS (resolver is the miss-handler, not a post-hoc `or_insert_with`).
- A test that uses a resolver return DISTINCT from the fallback/default constant AND asserts the output tracks the resolver (vary-and-track) OR asserts a call-counter > 0.
**Last verified**: 2026-06-06
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-codegen/src/emit\.rs
crates/ynz-codegen/src/queries\.rs
crates/ynz-codegen/tests/.*\.rs$
or_insert_with
\.entry\(
resolver
callee_size_resolver
compute_frame_size
FRAME_HEADER_SIZE
```

**Cause**: M3e Phase 1 extracted `build_frame_layouts` into `build_frame_layouts_with_resolver(…, callee_size_resolver: &dyn Fn(&str) -> Option<u64>)` to supply imported-callee sizes cross-module. But the function ran the memoizing `compute_frame_size` loop FIRST: `compute_frame_size("doWork")` recursed into the imported callee `getValue`, didn't find it as a local fn, fell through to `FRAME_HEADER_SIZE` (32), and **cached `sizes["getValue"]=32`**. The later `sizes.entry("getValue").or_insert_with(|| resolver(name))` was then a NO-OP — the resolver was NEVER consulted. The bug was INVISIBLE because the happy-path fixture's `getValue` was a leaf with no crossing locals, so its real frame == `FRAME_HEADER_SIZE` == 32 == the fallback: the resolved value coincided with the bypass value. The unit test asserted `doWork.total_size == 64` (32+0+32) and passed — proving nothing, because a bypassed resolver produces the identical 64. For ANY imported callee with its own crossing-locals/children (frame > 32) this silently UNDER-SIZES the embedded sub-frame → SIGILL/corruption (the exact escape-#1 class M3e exists to fix). Caught only by the adversarial gate (code-reviewer varied the resolver's return Some(32)/Some(128)/None → output invariant at 64; a `Cell<bool>` probe → resolver never called). Fixed by moving the resolver-seed BEFORE `compute_frame_size` and rewriting the test with a callee whose real frame=40≠32 + an anti-bypass sentinel (resolver→56 → doWork=88).

**Detection signature**: (1) CODE — a fn takes an injected resolver/closure to supply values for some keys, populates a memo `HashMap` (`sizes`/cache) via a recursive `compute_*` pass, AND seeds the resolver via `entry(k).or_insert_with(resolver)` / `or_insert(resolver(k))` that runs AFTER the recursive pass could have cached a fallback for `k`. The seed-after-compute ordering makes the resolver dead for any key the memo already touched. (2) TEST — an assertion on a resolver/callback-driven value that EQUALS the fallback/default constant (e.g. asserting a frame size == `FRAME_HEADER_SIZE`, or == header+0+header), with NO varied-input assertion and NO call-counter — so a bypassed resolver passes identically.

**Constraint**: When a function both memoizes a recursive computation AND accepts an injected resolver for some keys, the resolver MUST seed the memo BEFORE the memoizing pass (or be the cache-miss handler) — never `or_insert_with(resolver)` after the memoizer could cache a fallback. Any test proving a resolver/callback is exercised MUST (a) use a resolver return value DISTINCT from the fallback/default so a bypass changes the asserted value, AND (b) vary the return and assert the output tracks it OR assert the resolver was actually called (counter > 0). A single-fixed-value assertion that coincides with the fallback proves nothing.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] Diff to `emit.rs`/`queries.rs` adds/edits a fn taking a `&dyn Fn`/closure resolver param that feeds `.entry(...).or_insert_with(...)` or `.or_insert(...)`: verify a resolver-seed loop (`insert(name, resolver(name))`) runs BEFORE the recursive memo pass (`compute_frame_size`/`compute_*`). If the `or_insert_with(resolver)` is the only consumption AND it runs after the recursive pass → WARNING (resolver likely dead — the memo poisons the key with a fallback first).
- [ ] Diff to `crates/ynz-codegen/tests/*.rs` adds/edits a test asserting a resolver/callback-driven layout value: verify the asserted value is DISTINCT from the fallback constant (`FRAME_HEADER_SIZE` / header-only) AND the test either varies the resolver return + asserts the output tracks it OR asserts a call-counter > 0. A test asserting a value equal to (or derivable solely from) the fallback, with no vary/counter → WARNING (passes on a fallback coincidence; resolver bypass undetectable).

**Severity**: critical — silent under-sizing of an embedded sub-frame → SIGILL/memory corruption, and the test goes green because the resolved value coincides with the fallback. Worst failure class (no crash in the happy-path fixture; detonates only on a callee with a non-trivial frame).

**Originating incident**: 2026-06-06, v0.3-M3e Phase 1. `build_frame_layouts_with_resolver`'s `compute_frame_size` loop cached the `FRAME_HEADER_SIZE` fallback for imported callees before the `or_insert_with(resolver)` seed ran → the cross-module resolver was dead code. The recursion unit test passed at `doWork.total_size==64` only because the leaf callee's real frame happened to equal the 32-byte fallback. code-reviewer's adversarial probe (vary the resolver return → output invariant; `Cell` counter → resolver never fired) caught it; acceptance-verifier's claim-trusting PASS missed it. Fixed: resolver-seed moved before `compute_frame_size`; test rewritten with a >32 callee frame + anti-bypass sentinel (`resolver→56 → 88`). See [`.claude/planning/done/2026-06-05-v0-3-m3e-cross-module-frame-serialization/plan.md`](planning/done/2026-06-05-v0-3-m3e-cross-module-frame-serialization/plan.md) Phase 1 Findings Log (round-2/round-3).

## Silent Envelope Narrowing — Gated-Path Decline Widens While Output Tests Stay Green — 2026-06-12

**Scope**: any conditional/gated codegen or optimization path with a correct sequential/fallback lowering — currently the M3d spike admission gates in `crates/ynz-codegen/src/emit.rs` (`spike_cpu_candidates`, `spike_cpu_group_result_names`, `spike_extract_cpu_group`) and any future auto-parallelization/auto-promotion admission predicate (M3d P1+ productionized passes, `prefer-fixed` promotion, auto-SoA). Sibling of the 2026-06-06 "Test Green on Fallback Coincidence" corpse — same disease family (correct fallback masks a dead feature), different mechanism (admission predicate widened a decline vs. injected resolver bypassed by memo ordering).
**Exemption**:
- A decline condition added together with evidence that every fixture/test claiming to exercise the gated path still ADMITS post-change (e.g. an IR-grep assertion for the runtime call, a fire-counter, a snapshot containing the gated instruction) — that is the correct shape of a gate fix.
- An intentional envelope narrowing that updates the plan/design's declared FIRE/DECLINE table in the same diff AND corrects every fixture header/test name that claimed gated-path coverage — honest narrowing is allowed; silent narrowing is not.
- Decline conditions in throwaway code explicitly scoped as decline-only fixtures (fixture headers that already say "DECLINE fixture").
**Last verified**: 2026-06-12
**Category**: regex+judgment

**Pre-filter patterns**:
```
spike_cpu_candidates
spike_extract_cpu_group
spike_cpu_group_result_names
stmt_contains_wait
stmt_contains_suspending_call
return None
ynz_rt_spawn_blocking_joinable
YNZ_M3D_SPIKE
```

**Cause**: a fix round added a decline predicate (`stmt_contains_wait` on post-pair statements) that was WIDER than the bug class it targeted (`wait sleep(0)` intrinsic waits embed no child sub-frame, posing zero aliasing risk, yet matched the predicate) — and because the decline path lowers to always-correct sequential code, every byte-identical output test stayed green while 11 of 17 fixtures silently stopped exercising the feature.
**Detection signature**: (1) CODE — a diff adds or widens a decline/early-return condition inside an admission-gate function (predicate functions feeding `return None`/`return false` in `spike_*`/`*_candidates`/`*_extract_*` or any future admission pass) with NO accompanying fire-assertion change. (2) TEST/FIXTURE — fixtures or tests whose headers/names claim gated-path behavior ("proves the spike…", "reload across suspension…") with verification that only compares outputs between modes — byte-identical is trivially true when both modes take the fallback. (3) PLAN — a phase shipping a gated path with no declared FIRE/DECLINE envelope table, so reviewers have no contract to diff admission changes against.

**Constraint**: any diff touching an admission-gate predicate MUST come with mechanism-fired evidence for every input that is supposed to stay admitted (IR grep for the gated runtime call ≥1, fire-counter, or gated-instruction snapshot), and any plan introducing a gated path MUST declare its admission envelope (which input shapes FIRE, which DECLINE) as a plan-time table — a fix that flips an input from FIRE to DECLINE is then a visible contract violation instead of a 7th-round forensic discovery. Output equality between gated and fallback modes is necessary but NEVER sufficient.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] Diff adds/edits a condition in an admission-gate function (`grep -E "spike_(cpu_candidates|extract_cpu_group|cpu_group_result_names)|fn .*_candidates|fn .*admission"` on changed hunks) AND introduces/widens a `return None`/decline branch: verify the same diff (or its cited evidence) shows a FIRES check for the still-admitted inputs — e.g. `grep -c "call.*ynz_rt_spawn_blocking_joinable" <fixture>.ll` ≥ 1 per admitted fixture. Decline-widening with output-only evidence → BLOCK.
- [ ] Diff touches fixture files whose header comments claim gated-path coverage (`grep -E "proves|exercises|reload|spike SM" crates/ynz-driver/tests/fixtures/v0_3_m3d_*.ynz`): verify each claimed-FIRE fixture is in the FIRES set post-diff, not silently moved to DECLINE. Header claims FIRE but IR shows 0 gated calls → BLOCK (lying fixture).
- [ ] Plan diff adds a phase introducing a gated/conditional codegen path: verify the phase declares a FIRE/DECLINE envelope table (which input shapes admit vs decline). Missing → plan-reviewer BLOCK at plan time.

**Severity**: critical — the feature under test silently stops existing while its entire test suite stays green; downstream phases inherit "proven" machinery with zero live coverage. Cost when it fired: rounds 6–8 of an 8-round gate saga (one full extra fix round + re-gate) to discover and undo a narrowing that a plan-time envelope table would have flagged instantly.

**Originating incident**: 2026-06-12, v0.3-M3d Phase 0 round 7. The ISSUE-B fix declined post-pair statements on `stmt_contains_wait || stmt_contains_suspending_call`; the `stmt_contains_wait` clause also caught intrinsic `wait sleep(0)`, flipping fixtures (g)/(h)/(i)/(j)/(n) from FIRE to DECLINE — IR-verified zero `ynz_rt_spawn_blocking_joinable` call sites — making the cross-suspension reload machinery (built and debugged across 5 prior fix rounds, deviations #9/#14) dead code while all 17 fixtures stayed byte-identical green. Caught only because the coordinator primed all four round-7 gate agents on the "does the spike still FIRE, not just match output" question; deviation-judge #15 traced it at IR level and named the one-clause fix (keep only `stmt_contains_suspending_call`, which already excludes sleep via `M2_MAY_BLOCK_INTRINSICS`). Fixed in round 8; FIRES restored 0→2 spawn calls per fixture. See [`.claude/planning/done/2026-06-11-v0-3-m3d-cpu-parallelization/plan.md`](planning/done/2026-06-11-v0-3-m3d-cpu-parallelization/plan.md) Phase 0 Findings Log (R7/R8 entries) — committed dcc1432.

---

## Diagnostic-Text Dedup Silently Blinds a Substring-Filter Test — 2026-07-02

**Scope**: any test asserting on a LITERAL SUBSTRING of compiler stdout/stderr — `.contains("<lit>")`, `.starts_with("<lit>")`, `.lines().filter(|l| l.contains("<lit>"))`, or a `count` of such filtered lines — in `crates/ynz-*/tests/*.rs` (notably `crates/ynz-driver/tests/error_galleries.rs`), when the SAME change reworded, unified, or dedup'd the diagnostic text that literal came from.
**Exemption**:
- The reworded diagnostic still contains the asserted literal verbatim (a grep of the current message source / registry confirms the substring survives the rewording).
- The test filters on a token guaranteed stable across wording changes — an error code, a banned keyword name the diagnostic is ABOUT (`"struct"`, `"class"`), a diagnostic-template id — rather than incidental prose.
- The assertion is a positive `assert!(x.contains(lit))` that would go RED (not silently green) if the substring vanished — the failure mode here is the vacuous-pass shape: a `filter(...).count()` that drops to 0, or a `!contains` / `is_empty` that becomes trivially true, when the substring disappears.
**Last verified**: 2026-07-02
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-.*/tests/.*\.rs$
crates/ynz-diagnostics/src/
registry/features\.toml$
\.contains\(
\.starts_with\(
\.filter\(
```

**Cause**: a diagnostic-wording change (rewording a message, unifying two near-identical messages into one, moving text into `registry/features.toml`) removes a substring that a test was filtering on. If the test's assertion is a *count* or a *negative* over the filtered result — `filter(|l| l.contains("<old>")).count() >= 0`, `warnings_only.is_empty()`, `!stderr.contains("<old>")` — the filter now matches nothing and the assertion passes VACUOUSLY. A real regression guard is gutted with zero test failures: the wording change goes green and nothing proves the diagnostic still fires.

**Detection signature**: (1) a diff edits diagnostic/message text in `crates/ynz-diagnostics/`, a `banned_jargon`/registry string, or any user-facing message literal; AND (2) a test elsewhere filters compiler output on a literal that is a fragment of the *old* text, where the surviving assertion is a count/negative/emptiness check rather than a positive presence assertion. The tell: after the diff, grep the test's filter-literal against the CURRENT emitted diagnostic — zero matches means the guard is now vacuous.

**Constraint**: any change that rewords, unifies, or relocates diagnostic text MUST, in the same diff, re-verify every test that filters compiler output on a fragment of that text — either the literal still appears verbatim, or the test is repointed at a wording-stable token (error code / keyword / template id). A substring-filter regression guard must be phrased as a POSITIVE presence assertion that goes red when the substring vanishes, never as a count/emptiness check that passes on zero matches.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] Diff edits a message literal in `crates/ynz-diagnostics/src/` or a diagnostic string in `registry/features.toml`: grep `crates/ynz-*/tests/*.rs` for `.contains(`/`.filter(`/`.starts_with(` calls whose literal is a fragment of the removed/changed text. Any match whose surrounding assertion is a `count`/`is_empty`/`!...contains` (not a positive `assert!(...contains(...))`) → WARNING: substring guard may now pass vacuously.
- [ ] For each such test: confirm the filter-literal still appears in the current diagnostic output for the fixture. Zero occurrences in current output + test still green → WARNING (dead regression guard).

**Severity**: warning (no runtime miscompile — but a silently-gutted diagnostic regression guard lets a real diagnostic regression ship undetected, which the teaching mission makes load-bearing).

**Originating incident**: 2026-07-02, v0.3-M3g AAR. After a diagnostic-text unification, a test filtering on an old substring began passing unconditionally instead of red — the regression guard was silently gutted. `crates/ynz-driver/tests/error_galleries.rs` already carries the exact hazard shape (`stderr.contains("type")` / `"struct"` / `"class"` key-phrase checks); those survive because they filter on wording-stable keyword names, which is the correct form this corpse mandates.

---

## Refactor-Extracted Helper Double-Invoked on One Branch — 2026-07-02

**Scope**: any Rust source under `crates/**/src/` — a diff that (a) extracts a shared helper out of a function whose body has an `if`/`else` (one or both arms called the now-extracted logic) AND (b) adds a hoisted call to that same helper before or after the `if`/`else`, without checking whether an arm still calls it internally.
**Exemption**:
- The hoisted call REPLACES the per-arm calls (the extraction removed them from both arms and centralized the single call) — verified by the diff deleting the in-arm invocations.
- The helper is genuinely idempotent AND its double-invocation is provably harmless (documented on the call site) — rare; prefer removing the duplicate call.
**Last verified**: 2026-07-02
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/.*/src/.*\.rs$
fn [[:alnum:]_]+\(
if .* \{
} else \{
```

**Cause**: extracting shared logic into a helper called from an `if`/`else`, then adding a hoisted call to that same helper before/after the branch, leaves one arm invoking the helper TWICE (once inside the arm, once via the hoisted call). Mechanical extraction bug — the extraction and the hoist are each locally correct; the double-invocation only exists in the composition of the two edits.

**Detection signature**: in one function, after the diff, the same helper name appears BOTH inside an `if`/`else` arm body AND at a hoisted position (before/after the same branch), with no evidence the in-arm call was removed. A branch that entered the arm now runs the helper twice.

**Constraint**: when extracting a helper out of branch arms and hoisting a call to it, the in-arm invocations MUST be removed as part of the same extraction (centralize to exactly one call), OR the hoisted call must be conditioned to fire only on the arm(s) that don't already call it. Never leave the helper reachable twice on any single control-flow path.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each function in the diff that gains a new call to a helper `H` at a hoisted position (outside/after an `if`/`else`): grep the same function body for another call to `H` inside an `if`/`else` arm. Both present in one function → WARNING: verify no single path invokes `H` twice.
- [ ] For a diff that extracts a new `fn H` and adds calls to it: if `H` is called from both a branch arm and a post-branch hoisted position in the same caller, flag for double-invocation review.

**Severity**: warning (usually a wasted/duplicated side effect or double-accumulation; escalates to critical if `H` is non-idempotent and mutates shared/durable state).

**Originating incident**: 2026-07-02, v0.3-M3g AAR. A shared helper extracted out of an `if`/`else` was then hoisted with an added call after the branch, without checking that one arm already invoked it internally — a mechanical, diff-greppable extraction bug caught in review.

---

## Concurrency-Stress Fixture's Claimed Pressure vs Actual Spawn Topology — 2026-07-02

**Scope**: concurrency/stress/exhaustion test fixtures and their harness assertions — `crates/ynz-driver/tests/**`, `crates/ynz-watch/tests/**`, and any `.ynz` fixture whose WHY-comment or test name asserts a concurrency level (e.g. "20 simultaneous", "N concurrent tasks", "saturates the pool").
**Exemption**:
- The fixture's real spawn topology is verified to produce the claimed number of TRULY-concurrent tasks (a spawn-count assertion, a fire-counter, or an IR/`background`-site count backs the comment).
- The comment describes total WORK ITEMS processed over time (not simultaneity) and says so explicitly — a serialized loop of 20 items is honestly "20 items," not "20 simultaneous."
**Last verified**: 2026-07-02
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-driver/tests/.*
crates/ynz-watch/tests/.*
simultaneous
concurrent
background
spawn
saturate
```

**Cause**: a stress fixture's WHY-comment or test name asserts a concurrency level ("20 simultaneous") measured by TOTAL worker/work-item count, not by the fixture's real spawn topology. A serialized loop, a bounded pool, or an accidental await-between-spawns can mean the fixture never actually reaches the claimed simultaneity — so the "stress" test exercises far less contention than its name promises, and a regression in the concurrent path slips through green.

**Detection signature**: a fixture/test whose comment or name claims a concurrency count that is NOT backed by a spawn-topology assertion — the number is derived from a total-item count or asserted in prose only, with no check that N tasks are actually in flight at once. Grep the fixture body: does the spawn/`background` topology (and any barrier/await placement) actually admit N concurrent tasks, or does it serialize?

**Constraint**: any fixture claiming a concurrency level MUST verify that claim against its real spawn topology, not its total worker/work-item count — back the claim with a spawn-count assertion, a max-in-flight counter, or an IR/`background`-site count. A concurrency claim in a comment or test name with no topology-level evidence is treated as unproven.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For a fixture/test in the diff whose comment or name contains a concurrency count (`[0-9]+ (simultaneous|concurrent|parallel|in.?flight)`): verify the harness has a spawn-count / max-in-flight assertion matching that number. Number claimed, no topology assertion → WARNING.
- [ ] For a fixture claiming N simultaneous but whose body spawns in a serialized loop (spawn followed by an immediate `wait`/join before the next spawn) → WARNING: topology serializes; claimed simultaneity is not reached.

**Severity**: warning (a weaker-than-advertised stress test gives false confidence in the concurrent path; not itself a miscompile).

**Originating incident**: 2026-07-02, v0.3-M3g AAR. A stress fixture's WHY-comment asserted a concurrency level that was verified against its total worker count rather than its real spawn topology — the fixture's actual simultaneity was lower than the comment claimed, caught in review.

---

## Authoritative Analysis Output Computed But Never Consumed Downstream — 2026-07-04

**Scope**: `crates/ynz-codegen/src/emit.rs` (and any codegen/lowering path meant to be steered by an analysis result) + `crates/ynz-typeck/src/` analysis queries that compute a precise per-element / per-field / per-candidate set (`soa_candidate_query`'s `hot_fields`, crossing/suspend sets, layout-decision outputs). The disease: an analysis pass computes a precise authoritative answer, and the designated downstream consumer ignores it — falling back to a coarser unconditional behavior — silently forfeiting the precision (and the perf/correctness benefit) the analysis existed to provide. **Mirror-case sibling** of [`.claude/rules/authoritative-derivation.md`](rules/authoritative-derivation.md): that rule bans RE-DERIVING an already-authoritative answer a second time; this corpse is the same family pointed the other way — computing the authoritative answer ONCE and then never consuming it.
**Exemption**:
- The consumer deliberately takes the full/coarse path for a NAMED, recorded reason (a choke-point-contract invariant every consumer must honor — e.g. `.copy()`/`soa_copy_to_aos`/background-arg passing needing full-fidelity elements) AND the deferral of the selective path is filed as a tracked Future Requirement (FRAGO 020's FR#15 is exactly this recorded shape — the gap is a corpse only when it is SILENT).
- The computed output has an OTHER real consumer (a lint, a hint, a test) even if this particular codegen path doesn't read it — confirm who else reads it before flagging.
- A pass that computes the set purely to DECIDE admission (a boolean gate), where the set itself was never meant to drive per-element codegen.
**Last verified**: 2026-07-04
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-codegen/src/emit\.rs
crates/ynz-typeck/src/.*\.rs$
hot_fields
soa_candidate_query
soa_gather_into
array_elem_get_into
```

**Cause**: v0.3-M5 Phase 4's `soa_candidate_query` computed `hot_fields` (exactly which fields a hot loop touches — the whole point of D5's ≤2-field-union admission criterion), but Phase 5's codegen (`soa_gather_into`/`array_elem_get_into` in `emit.rs`) never consumed it — it unconditionally gathers ALL declared fields ("design c: gather full element, let DSE/SROA drop unused fields"). Since the shipped compiler runs no LLVM optimization passes, nothing dropped the unused fields, so the precise `hot_fields` analysis produced no effect and was a plausible independent contributor to Phase 6's measured ~1.0x (no benefit) result.

**Detection signature**: an analysis query populates a precise per-element/per-field/per-candidate output (a struct field, a returned set), and the designated codegen/lowering consumer the analysis exists to steer never references it — instead performing a coarser unconditional behavior (gather-all, widest-layout, no-specialization). Distinct from ordinary dead code: the output is often read by SOME consumer (a test, a lint), so `dead_code` stays quiet, while the ONE consumer whose behavior the analysis was meant to change ignores it. The tell is a computed precision set with no reference in the function whose output it was designed to narrow.

**Constraint**: an analysis output computed to steer a downstream pass MUST be consumed by that pass, OR the coarse-path choice MUST be a named, recorded decision (a choke-point-contract invariant + a tracked Future Requirement naming the deferred selective path — the FRAGO 020 / FR#15 shape). A precise set computed and then silently ignored is the corpse. When you add or edit an analysis query that emits a per-element/per-field set, confirm the designated consumer references it, or record why it does not.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For a diff adding/editing an analysis query in `crates/ynz-typeck/src/` that populates a per-field/per-element set (e.g. `hot_fields`, a candidate field-set): grep the designated codegen consumer (`soa_gather_into`/`array_elem_get_into` in `emit.rs`, or the named consumer) for a reference to that field/set. Zero references AND no `// CARVE-OUT:`/tracked-FR note → WARNING (computed-but-unconsumed).
- [ ] For a diff to `emit.rs`'s gather/lower paths that keeps an unconditional gather-all / widest-layout behavior while an analysis set naming the precise subset exists: verify either the set is consumed or a tracked deferral (FR#) names the gap → WARNING if neither.

**Severity**: warning (silent perf forfeiture, not a miscompile — the output is correct, just coarser than the analysis proved necessary; promote to critical if a future consumer relies on the selective path for correctness rather than only perf).

**Originating incident**: 2026-07-04, v0.3-M5 Phase 6 boundary review (FRAGO 020). The performance reviewer found `hot_fields` computed by Phase 4 but ignored by Phase 5's full-element gather; the CODE choice was ruled JUSTIFIED (a true selective gather needs every full-fidelity consumer re-audited against a "cold fields may be garbage" invariant — out of Phase 5's charter), but the Future-Requirements ledger's SILENCE on the narrower fix was the real gap, filed as FR#15. See [`.claude/planning/active/2026-07-03-v0-3-m5-auto-soa/audit.md`](planning/active/2026-07-03-v0-3-m5-auto-soa/audit.md) FRAGO 020.

---

## Background-Spawn Callee Resolver Reads `sig_table` Only, Skips the `generic_fn_table` Fallback — 2026-07-19

**Scope**: `crates/ynz-typeck/src/check.rs` — any resolver/predicate function whose job is "resolve a callee name (or a nested call's callee) to its signature/return type," specifically the `background`-spawn admission family (`bg_arg_is_materialized_shape_temp`, `bg_arg_type_readonly`, `bg_call_return_type_readonly`, `bg_ufcs_return_type`) and any FUTURE resolver added to that family or elsewhere in the file that needs the same "what does calling this name return" answer. This is a project-scoped SUB-PATTERN of [`.claude/rules/authoritative-derivation.md`](rules/authoritative-derivation.md) — narrower than that rule's general "don't re-derive" principle: specifically, a resolver that consults only ONE half of a required PAIRED lookup (concrete `sig_table.fns` + generic `generic_fn_table.fns`), silently treating the unconsulted table as if it doesn't exist.

**Exemption**:
- A resolver that documents (in a comment on the function) an invariant that its input can never be a generic callee — e.g. it runs downstream of a check that already rejected generic callees — so `sig_table`-only lookup is provably complete for that call site.
- A resolver that DELEGATES to an already-two-table-aware resolver (calls `bg_call_return_type_readonly`, `bg_ufcs_return_type`, or the borrow-reject check's `resolved` lookup at line ~3408) rather than performing its own `.fns.get` — delegation is the correct shape, not a second derivation.
- A lookup whose purpose is NOT "resolve this callee's signature/return type" but something orthogonal (e.g. "does this exact name exist as a concrete signature at all," used for a diagnostic candidate list) — `sig_table`-only is correct there because the generic table is a genuinely different question.

**Last verified**: 2026-07-19
**Category**: regex+judgment

**Pre-filter patterns**:
```
crates/ynz-typeck/src/check\.rs$
fn bg_
self\.sig_table\.fns\.get
self\.generic_fn_table\.fns\.get
```

**Cause**: during the fr23 confirmed-live use-after-free saga (`background`-spawn argument handling, plan-id `2026-07-04-v0-3-m7-optimizer-pipeline`, FRAGO 016 through 025), the SAME sub-mistake shipped twice in a row, independent of the saga's other (enumeration-vs-default-deny) architecture problem:
- **FRAGO 018** (`executor-2026-07-18-completion-gate-round2-cleanup`): `bg_arg_type_readonly`'s nested-call arm resolved a nested-call argument's callee via `self.sig_table.fns.get(fname)` ONLY. It admitted the narrow case it was built for (a nested call whose callee is CONCRETE), but any nested call whose callee is itself GENERIC still fell through un-admitted — even though a SIBLING function in the same file (the outer `Expr::Call` arm, `crates/ynz-typeck/src/check.rs:3833` and `:3891`; the borrow-reject check's `resolved` lookup, `:3408`–`:3418`) already resolved exactly this kind of name via the two-table `.or_else` split.
- **FRAGO 019** (`executor-2026-07-18-completion-gate-round3-fr23-recursive`): confirmed the SAME shape recurred one call-nesting level deeper — `background identity(identity(makeCargo())).haul()` reproduced the identical fr23 UAF signature (`haul: 888888/222`, `haul: 0/0`, etc. — nondeterministic garbage / stomp-sentinel values across 3+ repeated runs at both optimizer tiers) because `bg_arg_type_readonly`'s nested-call arm STILL only checked `sig_table.fns.get`, with no `generic_fn_table` fallback, even after FRAGO 018's fix. The eventual close (`bg_call_return_type_readonly`, a single recursive resolver reusing the SAME `sig_table` → `generic_fn_table` two-table split already used by the outer `Expr::Call` arm and the borrow-reject check) is now the correct template — see `crates/ynz-typeck/src/check.rs:2052`–`:2068`.

Both instances are diff-visible, mechanical omissions: a resolver reads one table, a sibling in the same file already reads both, and nobody cross-checked the two against each other before shipping. The saga's final default-deny redesign (FRAGO 022/023) closed the OTHER flavor of this saga (unclassified/misclassified `Expr` shapes via a non-exhaustive match) but does **not** structurally prevent this two-table-pair flavor — a brand-new resolver checking only `sig_table` still compiles clean today. This corpse is the standing check against that residual, still-open gap.

**Detection signature**: a new or edited function in `crates/ynz-typeck/src/check.rs` calls `self.sig_table.fns.get(...)` to resolve a callee name to a signature/return type, and the SAME function body contains NO corresponding `self.generic_fn_table.fns.get(...)` call (directly, or via `.or_else(...)` on the `sig_table` lookup, or via delegation to a resolver that itself does the two-table split) — while at least one other function in the file already performs the two-table split for the same "resolve callee name → signature" job (the outer `Expr::Call` arm at `:3833`/`:3891`, the borrow-reject check's `resolved` at `:3408`–`:3418`, or `bg_call_return_type_readonly` at `:2052`–`:2068`). Grep for `sig_table.fns.get` or `.fns.get` calls inside a function whose body has no sibling `generic_fn_table.fns.get`/`.or_else` within the same function.

**Constraint**: any resolver in `check.rs` that must answer "what does calling this name return" (or an equivalent callee-signature question) consults BOTH `sig_table.fns` (concrete) and `generic_fn_table.fns` (generic), using the established `.or_else` fallback template already used at the borrow-reject check (`:3408`–`:3418`) and centralized in `bg_call_return_type_readonly` (`:2052`–`:2068`):
```rust
self.sig_table.fns.get(name)
    .map(|s| /* concrete answer */)
    .or_else(|| self.generic_fn_table.fns.get(name).map(|s| /* generic answer, substituted */))
```
A new resolver either follows this template directly or delegates to a function that already does — never a THIRD, narrower hand-roll that reads `sig_table` alone "because the generic case doesn't come up in the case I'm fixing today." That reasoning is exactly what produced FRAGO 018 (narrow to a concrete-callee bugfix) and let FRAGO 019 (a generic callee, one level deeper) slip through unnoticed.

**Bouncer checks** (each runnable as shell against a diff):
- [ ] For each diff to `crates/ynz-typeck/src/check.rs` adding or editing a function whose body contains `self.sig_table.fns.get(`: check whether the SAME function body also contains `self.generic_fn_table.fns.get(` (directly or via `.or_else`) OR calls `bg_call_return_type_readonly`/`bg_ufcs_return_type` (a resolver already known to do the two-table split). Neither present → WARNING: "resolver consults sig_table only — does a generic callee need the generic_fn_table fallback too? See the FRAGO 018/019 precedent."
- [ ] For each diff adding a NEW function whose name or doc-comment implies it resolves a callee's type/signature (name matches `bg_.*type|bg_.*return|resolve.*callee|resolve.*call`): verify it either reuses `bg_call_return_type_readonly`/`bg_ufcs_return_type` or independently implements the `sig_table` → `generic_fn_table` `.or_else` pair. Missing both → WARNING.

**Severity**: critical — this is the fr23 use-after-free class: a background-spawned argument's temporary gets freed while the parent frame still expects it live, producing nondeterministic garbage output (confirmed via 3-7 repeated live runs at both optimizer tiers in both FRAGO 018 and FRAGO 019). Not a lint nitpick — a silently un-admitted case reopens a confirmed-live memory-safety bug for that specific callee shape.

**Originating incident**: 2026-07-18, plan `2026-07-04-v0-3-m7-optimizer-pipeline` (`.claude/planning/active/2026-07-04-v0-3-m7-optimizer-pipeline/audit.md`, FRAGO 018 and FRAGO 019 — read alongside the `## AAR — 2026-07-19` section for full saga context). An 8-round memory-safety fix saga (FRAGO 016 through 025) on `background`-spawn argument admission hit the SAME sub-mistake twice: FRAGO 018 extended `bg_arg_type_readonly`'s nested-call arm to resolve a CONCRETE nested callee via `sig_table.fns.get` only, closing the confirmed-live repro `background identity(makeCargo()).haul()`; FRAGO 019, one round later, found the identical helper still fell through for a GENERIC nested callee (`background identity(identity(makeCargo())).haul()`) because the `generic_fn_table` fallback that the file's OTHER callee-resolution sites (the outer `Expr::Call` arm, the borrow-reject check) already had was never added to this one. FRAGO 019 fixed it for good by collapsing both hand-rolled derivations into one recursive, two-table-aware function (`bg_call_return_type_readonly`) — but the underlying human/agent tendency ("just add the fallback here, for this one case") is what this corpse exists to catch on the NEXT resolver, before a third occurrence needs a third fix round.
