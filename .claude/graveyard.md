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
