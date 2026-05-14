---
slug: design-lockdown-from-gemini-review
owner: patrick
status: active
files:
  - .claude/rules/**
  - .claude/graveyard.md
  - .claude/plans/active/v0-1-compiler.md
  - .claude/plans/active/m3-control-flow-fns.md
  - CLAUDE.md
  - design/**
  - design/compiler-errors.md
  - spec/**
  - crates/ynz-diagnostics/src/banned_jargon.rs
  - crates/ynz-diagnostics/tests/jargon_audit.rs
  - crates/ynz-diagnostics/tests/snapshots.rs
created: 2026-05-14
last_updated: 2026-05-14-r2
depends_on: v0-1-compiler
---

# Plan: Design Lockdown From Gemini Review

Created: 2026-05-14
Status: pending_approval (round 2 — addressing plan-reviewer fixes)

**Parent**: `.claude/plans/active/v0-1-compiler.md` (this is a cross-cutting design + docs workstream that affects every future milestone but doesn't add new compiler features by itself).

---

## Context & Why

**Goal.** Lock in every design decision made during the conversation that started with Gemini's code review pointing out Yinz had no explicit `const` deep-immutability invariant. The conversation expanded into a full design audit. Without this plan, those decisions will be lost in compaction and we'll re-relitigate them, drift on terminology, or worse — bake conflicting decisions into M4 and beyond.

**Why this exists.** Patrick said explicitly: "alot of this very tehicnial stuff i will forget maybe" and "all things we need to do (patching our code) need a plan and todo because we cannot forget this stuff." The plan file IS the institutional memory. Every decision below has Why+How-to-apply context so future-Claude can act on them without needing the full conversation context.

**Background — what triggered this.** Gemini asked whether Yinz enforces `const` like Rust enforces non-`mut`. Investigation found:
- Typeck already enforces no-reassignment for const bindings (`check_assign` rejects when `entry.is_const`)
- But NO design doc states the full invariant chain: const blocks `.lend`, `.give`, field mutation, and emits LLVM `readonly`
- The performance contract (`readonly` + `noalias` → LLVM aliasing optimizations → Rust-beating perf) is undocumented
- M4 (types + ownership) hasn't been planned yet, so this invariant was about to be missed

The conversation then expanded to cover: terminology (shape vs type), inference rules (everything inferred + muted IDE hints), `verified` vs `unchecked` naming, self-referential shapes, zero-cost framing, kernel mode, arenas, no-function-coloring async, panic safety without try/catch, HTTP framework supervision, auto-metadata in binaries, Rule 11 extension to IDE surfaces, and the plan-invariants enforcement mechanism.

**Constraints.**
- v0.1 compiler implementation MUST NOT be blocked by this plan — M3 is in-flight on a different chat slug and continues independently
- Shape keyword rename is cheap NOW (M4 hasn't started, lexer doesn't reserve `type` yet) and gets exponentially more expensive each milestone
- Docs are the only deliverable users see; spec quality matters more than internal compiler purity for this plan
- No new compiler features in scope — this is design lockdown + one keyword reservation + banned-jargon entry

**Success criteria.**
- Every decision from the conversation appears in a durable file (rules, design doc, graveyard, plan amendment)
- `const` deep-immutability invariant is stated in `design/ownership.md` AND embedded in v0-1-compiler.md forward-compat constraints AND in graveyard as the lesson learned
- M4 plan (when written) will mechanically inherit the invariants requirement via plan-invariants.md + plan-reviewer enforcement
- `shape` keyword is locked in for M4 — banned-jargon includes `type` with a teaching error pointing to `shape`
- Future-Claude reading the saved plan can reconstruct the full design intent without the conversation transcript

---

## Research Findings

**Current state of the codebase relevant to this plan:**

- **Lexer (`crates/ynz-parser/src/lexer.rs`)**: does NOT yet reserve `type` as a keyword. M4 was going to add it. This makes the rename free at the keyword level — we just reserve `shape` instead.
- **Typeck (`crates/ynz-typeck/src/check.rs`)**: already enforces const-no-reassign at `check_assign:264`. The `is_const` flag flows through `ScopeEntry`. **Field mutation isn't in the AST yet** (no `FieldAssign` variant in `Stmt`), so the deep-immutability piece is forward-looking design work for M4, not a current bug.
- **Banned-jargon (`crates/ynz-diagnostics/src/banned_jargon.rs`)**: already has the infrastructure to ban legacy terms with three-part teaching errors. Adding `type` as banned (pointing to `shape`) is a one-line addition + a jargon-audit test entry.
- **Docs scale**: `spec/types.md` has 42 instances of "type" (some are the keyword, some are the generic word "type"); 70 type-declaration patterns (`type Foo {}`) across spec+design. The rename needs careful regex — `type Foo {}` declarations rename to `shape Foo {}`, but "the type of x" generic prose stays as "type."
- **Graveyard (`/workspaces/ynz/.claude/graveyard.md`)**: already exists and is loaded by the global Bouncer (per global CLAUDE.md). Adding entries is mechanical enforcement at no extra hook cost.
- **Rules directory (`.claude/rules/`)**: has 4 files (`naming.md`, `spec-writing.md`, `language-design.md`, `docs-checklist.md`). Adding 3 new files follows established pattern.
- **Design index (`design/decisions.md`)**: exists as a TOC; needs entries for new design docs.
- **`design/future/` subdir**: does NOT exist yet. Need to create the directory and the index.

**Key technical insight**: because the lexer doesn't yet reserve `type`, this plan is overwhelmingly docs + rules. The one code-touch is adding `type` to banned-jargon (one line + test row). The keyword reservation happens organically when M4 plan adds the `shape` keyword to the lexer — no separate "rename" PR needed.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Decisions get lost in compaction before plan file is saved | Med (this conversation is long) | High (we re-relitigate or drift) | Phase 1 saves the plan FIRST so the institutional memory exists before anything else |
| Inconsistent vocabulary use after rename (some docs still say "type", some say "shape") | High during rename phase | Med (confusing for users reading docs) | Phase 6 grep-sweeps every doc; verification phase rechecks |
| Const deep-immutability invariant gets diluted/forgotten when M4 is planned | Med (M4 is the danger zone) | High (this is the entire reason for this plan) | Lock invariant in three places: design/ownership.md, v0-1-compiler.md forward-compat section, graveyard with diff-greppable pattern |
| Future-list docs sit unread until v0.2 and decisions drift | Low | Med | `design/future/index.md` is a single discoverable TOC; v0.2 milestone plan MUST link these as inputs |
| Graveyard entry regex patterns are too broad and cause false-positive Bouncer warnings | Low | Low | Patterns target specific anti-patterns (e.g., "type Player {}" not just "type") with test phrases in the entry |
| Rule 8 clarification gets re-interpreted later as "soften zero-cost" instead of "clarify what zero-cost means" | Med | High (could open door to design degradation) | Clarification block explicitly says "KEEP THE RULE AS-IS, this is clarification not relaxation" with the specific words to use and not use |
| Shape rename misses a doc, M4 implementation references the old keyword | Med | Med (M4 hits a compile error against banned-jargon) | Phase 7 verification sweep greps for `\btype\b` in non-prose contexts; banned-jargon catches anything that slips through at compile time |
| Inference rule gets interpreted as "make some things explicit" against the conversation's actual conclusion | Med | High (would undo the consistency win) | Rules file states explicitly: "infer EVERYTHING; if forcing explicit annotation is being considered, that's the inverse anti-pattern" with graveyard entry pointing back |

---

## Questions for Patrick

1. **`.claude/rules/inference.md` vs merging into existing files** — I'm proposing a standalone rules file for the inference rule since it's load-bearing. Alternative: merge into `.claude/rules/naming.md` or a unified `vocabulary.md`. **My recommendation: standalone, because it's about behavior (the IDE protocol) not just vocabulary.** Confirm?

2. **`design/future/index.md` content** — the conversation locked in a list of future-design docs (self-references, no-runtime-mode, arena, concurrency, panic-safety, supervisor, http-framework, packages). Want me to also include a "deferred until concrete need" section for things like Option B arenas, formal verification for NASA, etc., or keep the index strictly to designs we've committed to writing? **My recommendation: include both — main list for committed, "parking lot" subsection for ideas mentioned but not yet committed.**

(All open questions resolved as of round 3. See Locked Decisions below.)

## Locked Decisions (resolved in earlier rounds)

- **No stub M4 plan written now.** Rules file (`plan-invariants.md`) is the durable mechanism — /plan picks it up when M4 is scheduled. Pre-writing M4 plan is YAGNI. (Decision: option (b); plan operationalizes this in Phase 1 step 2 and Phase 5 step 1.)
- **Graveyard entries use tight, concrete diff-greppable patterns** with WHY-the-pattern-was-chosen field, so future-Claude can adjust on false-positive. (Operationalized in Phase 5 entry rewrites — round 2.)
- **Self-references use Approach A (relative pointers).** Locked round 3 (Patrick confirmed). Alternatives B (fix-up-on-move — complexity/corruption risk) and C (pin-in-place — forces heap, API constraints, Rust's solution we explicitly reject) considered and rejected. Phase 4b doc writes Approach A as the design with this rationale.

---

## Risk Assessment & Rollout Strategy

**Risk level: LOW**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | Compiler/docs work |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | New docs + one banned-jargon entry |
| Third-party integration | No | |
| Changes existing endpoints | No | No runtime code paths |

**Mitigations applied**: this is design + docs work with one banned-jargon line; the only "production" surface is the lexer's banned-jargon list which is already infrastructure with tests.

**Rollout plan**: not applicable in the production-rollout sense. Phases ship sequentially as PRs to main; verification phase confirms everything reads coherently before final PR closes. No feature flag.

---

## Phases

### Phase 1: Rules + Vocabulary Foundation
**PR scope**: Lock in the rules files and vocabulary that every subsequent phase will reference. This MUST come first so terminology and the inference rule are settled before docs cite them.
**Branch**: `docs/design-lockdown-p1-rules`
**Flag**: N/A
**Est. lines**: ~400 (3 new rule files, CLAUDE.md additions)
**Ships via**: `/pr`
**Objective**: Three new rules files exist, project CLAUDE.md references them, project naming.md is updated for the `shape` rename.
**Why this phase exists**: Every later phase cites "the inference rule" or "the vocabulary file" — those must exist before they can be cited.
**Current-state anchors**:
- `/workspaces/ynz/CLAUDE.md` — current rules table; phase adds three entries
- `/workspaces/ynz/.claude/rules/naming.md` — already has the renamed-concepts table; phase updates `type → shape` row and replaces user-facing examples
- `/workspaces/ynz/.claude/rules/` — directory exists, add 3 files
**Files (expected scope)**:
- NEW `.claude/rules/inference.md`
- NEW `.claude/rules/plan-invariants.md`
- NEW `.claude/rules/vocabulary.md`
- `.claude/rules/naming.md` (update for shape rename)
- `CLAUDE.md` (project root — add one-liners pointing to new rules)
**Deviation rule**: Executor MAY touch files not listed if the change serves the planned work (typo fix in adjacent docs, lint fix). Document each deviation in the PR. If a deviation is its own concern, STOP — split into separate PR.
**Steps**:
0. **Resolve banned-jargon collision FIRST** (load-bearing prerequisite). The Yinz compiler bans the words `infer` and `inference` in **user-facing compiler diagnostics** (`crates/ynz-diagnostics/src/banned_jargon.rs:14-15`, source-of-truth `design/compiler-errors.md`). This plan's rule file is named `inference.md` and design docs talk about "the compiler infers X" — these audiences are different (internal design language vs end-user error messages). Add an explicit dual-audience note in BOTH `inference.md` (top of file) AND in `crates/ynz-diagnostics/src/banned_jargon.rs` (comment at top of the const) stating: "Banned-jargon governs USER-FACING DIAGNOSTICS only. Design docs, rule files, and internal spec prose are allowed to use `infer`/`inference` because they describe how the compiler works to engineers, not how to fix code to end users. Do not 'fix' the divergence by renaming this rule file or stripping these words from banned-jargon — they intentionally diverge by audience." This step is Phase 1 work (touches the rule file) but the `banned_jargon.rs` comment edit happens in Phase 6 alongside the `type` ban addition.
1. Write `.claude/rules/inference.md`. Content: state the uniform inference rule (compiler infers → IDE muted hint → dev can opt to type explicitly → hover tooltip teaches WHY). List domains it applies to (types, ownership, wait, lifetimes, allocators, copy points). Specify muted-text styling rules (neutral gray for benign inference; red-tinted for cautionary like mutation). Explicit non-rule: compile errors use standard error styling separately, NOT muted hints. Include the inverse anti-pattern: "forcing explicit annotation at call sites that could be inferred." TOP OF FILE: dual-audience disclaimer per Step 0 — this rule file uses the words `infer`/`inference` deliberately; that's allowed in internal design docs but banned in user-facing diagnostics; cross-link to `design/compiler-errors.md`.
2. Write `.claude/rules/plan-invariants.md`. Content: every milestone plan from M4 onward must include `## Invariants This Milestone Must Preserve` with five sub-sections: Safety, Performance (including LLVM attribute emission), Teaching, Runtime Dependencies, Kernel-Mode Behavior. Each sub-section lists invariants as testable assertions (not vague aspirations). Plan-reviewer agent checks this section exists and is non-empty.
3. Write `.claude/rules/vocabulary.md`. Content: official Yinz user-facing terms — `shape` (declaration), value (instance), `map<K,V>` (separate concept from shape), `array<T>`, `fixed<T>`, `options`, `union`, `nothing`, `none`, `maybe`. Banned legacy terms with replacements. Examples of correct vs incorrect prose. This is the authoritative reference for all user-facing docs.
4. Update `.claude/rules/naming.md`: change `struct/class/interface → type` row to `struct/class/interface/type → shape`. Update `enum → options` is fine. Update Capital Letter examples that use `Type` to use `Shape` (or pick a non-keyword example to avoid the loop).
5. Update project CLAUDE.md: add three one-liner references under "Rules Files" table — inference.md (load on IDE/teaching topics), plan-invariants.md (load when writing/reviewing plans), vocabulary.md (load on any docs work). Confirm the table format matches existing entries.

**Acceptance criteria**:
- [ ] `.claude/rules/inference.md` exists and states the uniform inference rule with WHY, muted-hint styling, inverse anti-pattern
- [ ] `.claude/rules/plan-invariants.md` exists and specifies the five required sub-sections for milestone plans
- [ ] `.claude/rules/vocabulary.md` exists and lists every Yinz user-facing term + banned legacy term + correct-vs-incorrect prose examples
- [ ] `.claude/rules/naming.md` updated: `type` → `shape` everywhere in the renamed-concepts table
- [ ] Project CLAUDE.md table includes references to all three new rule files
- [ ] No occurrence of "smart defaults at call sites" or similar inference-only-for-some-things framing in any rule file

**Quality gate**:
- [ ] Three new rules files are written for the Claude/AI audience (not for end users) and follow existing rules-file tone
- [ ] Vocabulary file's "banned legacy terms" list catches every term we discussed (struct, class, interface, enum, void, null, undefined, Optional, fn, &T, &mut T, move, clone, Result, match, switch, T[], Array, HashMap, Map)
- [ ] No new rule contradicts an existing golden rule
- [ ] `shape` is treated as the keyword; "Player" or similar PascalCase identifiers are the example type names

**Verification**: `ls /workspaces/ynz/.claude/rules/` shows 7 files (existing 4 + new 3). `grep -l "shape" /workspaces/ynz/.claude/rules/*.md` shows new files reference it correctly. CLAUDE.md table renders with new entries.

---

### Phase 2: Golden Rules Update (Rules 8 & 11)
**PR scope**: Update the two golden rules touched by this conversation — Rule 11 to cover IDE teaching surfaces uniformly, Rule 8 to add the zero-cost clarification block.
**Branch**: `docs/design-lockdown-p2-golden-rules`
**Flag**: N/A
**Est. lines**: ~150
**Ships via**: `/pr`
**Objective**: `design/golden-rules.md` reflects the locked-in interpretation of both rules so future-Claude can't re-relitigate them.
**Why this phase exists**: The golden rules are sacrosanct per project CLAUDE.md. Any ambiguity in them propagates to every design decision. Locking in the clarifications is more important than the new design docs that follow.
**Current-state anchors**:
- `design/golden-rules.md` — file exists with all 13 rules; Rule 8 and Rule 11 need updates
- Project CLAUDE.md (current state shows Rule 11 mentions "compiler is a teacher" but doesn't cover IDE surfaces explicitly)
**Files (expected scope)**:
- `design/golden-rules.md`
- `CLAUDE.md` (project root — the inline rule statements at top need to match)
**Steps**:
1. Read current `design/golden-rules.md` to understand the WHY-block structure for each rule.
2. Update Rule 11 wording: extend "compiler is a teacher" to "every diagnostic AND IDE tooltip answers WHAT happened/is happening, WHAT to do instead (or how to make it explicit), and WHY." Add sub-rule: shared wording — one canonical explanation per concept, reused wherever it surfaces. Include the muted-hint case where the "fix" is just typing what's already inferred.
3. Update Rule 8: KEEP the current "Zero-cost abstractions" wording exactly. Add a clarification block (clearly labeled "**Clarification — do not interpret as relaxation:**") explaining: (a) zero-cost ABSTRACTIONS means the abstraction adds no cost beyond hand-written code, NOT "no features have any cost"; (b) features (Arc, async runtime) cost what they inherently cost; (c) compiler inference is at compile time → zero runtime cost from the inference itself; (d) any future tradeoff that costs MORE than hand-written code requires Patrick's explicit approval + documented decision in `design/decisions.md`.
4. Update project CLAUDE.md if it has inline statements of these rules (it has the 13-rule list at top — make sure the one-line summaries don't contradict the expanded versions in `design/golden-rules.md`).

**Acceptance criteria**:
- [ ] `design/golden-rules.md` Rule 11 explicitly covers IDE teaching surfaces (muted hints + tooltips) AND compiler diagnostics
- [ ] `design/golden-rules.md` Rule 8 wording is UNCHANGED at the headline ("Zero-cost abstractions. ...") and has a labeled clarification block underneath
- [ ] The clarification block explicitly forbids interpretation as "soften zero-cost"
- [ ] The clarification names Patrick as the approval authority for any tradeoff that costs more than hand-written code
- [ ] Project CLAUDE.md's one-line summaries of rules 8 and 11 don't contradict the expanded versions

**Quality gate**:
- [ ] Rule 11 update can be cited from any new design doc as "see Golden Rule 11" without ambiguity
- [ ] Rule 8 clarification reads as "here's what zero-cost MEANS" not "here's what zero-cost EXCEPTS"
- [ ] Tone matches existing golden-rules.md (terse, prescriptive)

**Verification**: `grep -A 5 "Rule 11" /workspaces/ynz/design/golden-rules.md` shows IDE coverage. `grep -A 15 "Rule 8\|Zero-cost" /workspaces/ynz/design/golden-rules.md` shows clarification block. Project CLAUDE.md one-liners unchanged or aligned.

---

### Phase 3: Existing Spec + Design Doc Updates
**PR scope**: Update the existing user-facing spec and design docs to reflect locked decisions — const deep-immutability, ownership inference rule, teaching-mission IDE reference. NO shape rename yet (that's Phase 6).
**Branch**: `docs/design-lockdown-p3-existing-docs`
**Flag**: N/A
**Est. lines**: ~250
**Ships via**: `/pr`
**Objective**: User-facing spec files and design docs reflect the conversation's locked decisions, without yet renaming `type` to `shape` (rename is its own phase for clean diff).
**Why this phase exists**: spec/variables.md and design/ownership.md were directly cited in the conversation as having gaps. Fix those gaps now in their own PR so reviewers can focus on substance, not rename diffs.
**Current-state anchors**:
- `spec/variables.md` — has the let/const section but no deep-immutability discussion
- `spec/ownership.md` — describes `.share`/`.lend`/`.give`/`.copy`/`.freeze` but with "smart defaults at call sites" inference framing that the conversation REJECTED in favor of "infer everything, IDE shows muted hints" uniformly
- `design/ownership.md` — has the "Smart Defaults at Call Sites" section that needs replacement
- `design/teaching-mission.md` — needs reference to IDE hint protocol as a core teaching surface
**Files (expected scope)**:
- `spec/variables.md` (add deep-immutability section + forward note about field mutation)
- `spec/ownership.md` (update to infer-everything + IDE hints framing; remove "smart defaults" wording)
- `design/ownership.md` (replace "Smart Defaults at Call Sites" section with "Uniform Inference + IDE Hints"; ADD new section connecting const to LLVM `readonly`/`noalias` and the performance contract)
- `design/teaching-mission.md` (add IDE surfaces as a core teaching pillar; cite golden rule 11 update)
**Steps**:
1. `spec/variables.md`: add a section under `## const — immutable variables` titled "What const blocks (full picture)". List: reassignment (already enforced), field mutation (when types land — points to design/ownership.md), mutable borrows (`.lend`/`.give` — when ownership lands). Add the one-line summary: "const means immutable in every direction — no reassignment, no field changes, no mutable borrows." Examples must still be HS-grad readable per `.claude/rules/spec-writing.md`.
2. `spec/ownership.md`: rewrite the "Smart defaults" section. New framing: "When you pass a value to a function, you usually don't write `.share` or `.lend` — the compiler infers it. Your IDE shows you what was inferred as muted text, so you can SEE what's happening." Show muted-text examples. Keep `.lend`/`.give`/`.copy`/`.freeze` as the explicit forms users CAN type if they choose. State the inverse anti-pattern: forcing explicit annotation is not the path. Cross-link to `.claude/rules/inference.md` for the rule itself.
3. `design/ownership.md`: delete the "Smart Defaults at Call Sites" sub-section verbatim. Replace with "Uniform Inference + IDE Hints" sub-section that (a) states the rule, (b) cites `.claude/rules/inference.md`, (c) gives the LLVM performance reasoning (const → readonly attribute → noalias → aliasing optimizations → Rust-beating perf in benchmarks). Add a NEW dedicated section: "`const` Deep Immutability — Safety + Performance Contract" with subsections: What const blocks (full list), Why the language enforces all of it (safety: aliasing rules; performance: LLVM attributes), LLVM emission contract (M4 must emit `readonly` on params from const bindings), Forward-compatibility note (field mutation enforcement lands with M4 type system).
4. `design/teaching-mission.md`: add a section "IDE as Teaching Surface" or similar. Content: the muted-hint protocol is a core teaching mechanism, not just convenience. Cite golden rule 11 (post-update). Cite `.claude/rules/inference.md`. State: every inferred semantic must be hoverable in the IDE with a three-part explanation matching compiler-diagnostic format. Tooltips re-use the same canonical wording.

**Acceptance criteria**:
- [ ] `spec/variables.md` const section explicitly mentions field mutation + mutable borrows as future enforcement targets
- [ ] `spec/ownership.md` no longer uses "smart defaults at call sites" framing; uses "inferred, IDE shows muted hints" instead
- [ ] `design/ownership.md` has a dedicated `const` Deep Immutability section with the LLVM performance contract spelled out
- [ ] `design/teaching-mission.md` references IDE surfaces as a core teaching pillar
- [ ] All four files internally cross-reference each other and the new rules files
- [ ] No occurrence of `type Player` or `type Foo` syntax in any updated file (Phase 6 will handle remaining; this phase keeps `type` where it appears, no NEW uses)

**Quality gate**:
- [ ] Spec files remain HS-grad readable per `.claude/rules/spec-writing.md` (short sentences, examples, no jargon)
- [ ] Design files remain compiler-engineer-focused
- [ ] Every claim in design/ownership.md about LLVM emission is paired with what M4 plan must do to honor it
- [ ] Cross-references resolve (i.e., the cited rules files actually exist from Phase 1)

**Verification**: `grep -n "field mutation\|deep immut\|readonly" /workspaces/ynz/design/ownership.md /workspaces/ynz/spec/variables.md` shows the new content. `grep -n "smart defaults" /workspaces/ynz/design/ownership.md /workspaces/ynz/spec/ownership.md` returns nothing (or only references explaining why we DON'T use that framing).

---

### Phase 4a: New Design Docs — v0.2-Critical (ide-hints + 4 v0.2 future docs)
**PR scope**: `design/ide-hints.md` (load-bearing for v0.2 LSP) plus the v0.2-target future docs (concurrency, panic-safety, supervisor) plus the future index.
**Branch**: `docs/design-lockdown-p4a-v0-2-design-docs`
**Flag**: N/A
**Est. lines**: ~750 (5 new files including the index)
**Ships via**: `/pr`
**Objective**: The IDE-hints protocol and v0.2-target designs are locked in. v0.2 planning has the inputs it needs to start cold.
**Why this phase exists**: These designs MUST be in writing now while context is fresh. Without them, v0.2 planning starts cold and we'll re-relitigate. Splitting from 4b keeps each PR under the 500-line guidance and review-friendly. `design/future/index.md` is the TOC making them discoverable.
**Current-state anchors**:
- `design/` directory exists; `design/future/` does NOT yet — create it in this phase
- `design/decisions.md` is the index; updated in this phase (NOT in Phase 5 as originally proposed — index update belongs in the same PR as the new files so the index isn't stale even briefly)
**Files (expected scope)**:
- NEW `design/ide-hints.md` (load-bearing — this IS the teaching protocol)
- NEW `design/future/index.md` (TOC + parking lot — initial entries for 4a docs; 4b docs added in Phase 4b)
- NEW `design/future/concurrency.md` (v0.2)
- NEW `design/future/panic-safety.md` (v0.2)
- NEW `design/future/supervisor.md` (v0.2)
- `design/decisions.md` (add index entries for the 5 new files above)
**Steps**:
1. Create `design/future/` directory (via touching `design/future/index.md`).
2. Write `design/ide-hints.md`: protocol spec — what gets hinted (types, ownership, wait, lifetimes, allocators, copy), how muted text mirrors what the dev would have typed (so click-to-make-explicit produces real syntax), styling rules (neutral gray for benign, red-tinted for cautionary), tooltip format (three-part WHAT/WHAT-INSTEAD/WHY matching compiler diagnostics). Quote the canonical example from the conversation: hovering muted `.share` on a const binding shows "this is .share because player is const; const can't grant write access; if you need mutation, declare with let; common scenarios..." Note that LSP work in v0.2 implements this; the protocol is locked in NOW so v0.2 doesn't have to redesign.
3. Write `design/future/index.md`: TOC table with one-line description per future doc + status (designed-locked / parking-lot) + target milestone column. Initial entries for 5 docs being created in this phase. Parking-lot section for ideas mentioned but not yet committed (formal verification for NASA, Option B arenas with lifecycle tracking, etc.). Phase 4b will append the v0.3+ docs to this TOC.
4. Write `design/future/concurrency.md`: no-function-coloring async. Compiler does whole-program may-block analysis (looks at call graph, sees I/O intrinsics). `wait` inserted at compile time, shown muted in IDE. No type-level async/sync split. FFI boundaries need explicit "may-block" annotation. Stackless state machines for codegen (low memory, fast spawn). Compiled Yinz packages embed may-block metadata. `background` spawns onto executor. Supervisor pattern at task boundary. v0.2 implementation milestone. Lock in the principles; v0.2 plan fleshes out scheduler/executor details. **Internal naming note**: the on-disk metadata field for "may-block analysis result" should NOT use the words `infer`/`inference` in its serialized form (audience: user-visible debug tooling) — propose `mayBlock` or `effectsAnalyzed`; design doc itself uses `infer`/`inference` freely (engineering audience).
5. Write `design/future/panic-safety.md`: errors (KNOWN failure modes) auto-propagate via `errors` keyword. Panics (UNKNOWN/unexpected) auto-isolate to the task. Main panic → process exit; orchestrator (Docker/systemd) restarts. Drop-on-scope-exit handles cleanup. NO try/catch ever. No mutex poisoning. Supervisor pattern (Erlang/BullMQ-style) at task boundary. Stdlib `task.onPanic` handler for observability. Explicit non-design: no `try { } recover { }` blocks (rejected; supervisor is the boundary). Cite the trading-app scenario for `panic("market data desync")` semantics. **Cross-reference**: Phase 5 will add a graveyard entry catching re-introduction of try/catch.
6. Write `design/future/supervisor.md`: stdlib supervisor helpers — `supervise.alwaysRestart(task)`, `.withBackoff(backoff)`, `.maxRestarts(n)`, `.onPanic(handler)`. API design + WHY (encapsulates the common pattern, IDE shows muted defaults). v0.2 implementation alongside `background`. Meta-rule: any stdlib API owning a long-running loop is supervised by default with explicit override available.
7. Update `design/decisions.md` index: add entries pointing to the 5 new files from this phase (`design/ide-hints.md`, `design/future/index.md`, `design/future/concurrency.md`, `design/future/panic-safety.md`, `design/future/supervisor.md`). Each entry: title + link + one-sentence description. Inserted in TOC ordering, not appended.

**Acceptance criteria**:
- [ ] 5 new files exist at the listed paths
- [ ] `design/future/index.md` is a TOC with one-line description per other future doc + status column + parking-lot section
- [ ] Each design/future/*.md states: what's deferred, why deferred, milestone target, what design decisions are LOCKED, what's still open
- [ ] `design/ide-hints.md` is detailed enough that v0.2 LSP planning can implement against it directly
- [ ] No future doc contains an "acceptable for now" or "we'll revisit" framing without an explicit cost+trigger (per `~/.claude/rules/no-duct-tape.md`)
- [ ] Every locked decision from the conversation that targets v0.2 appears in exactly one design doc (no duplication; cross-link instead)
- [ ] `design/decisions.md` index updated in THIS PR (not deferred to Phase 5) so the index is never stale

**Quality gate**:
- [ ] Each future doc is concise (under ~250 lines); detail belongs in the v0.2/v0.3 milestone plan when that work is scheduled
- [ ] No future doc tries to design what hasn't been decided yet — open questions are flagged explicitly
- [ ] Cross-references all resolve (each cited file exists from earlier phases or is one of the new ones)
- [ ] Future docs are compiler-engineer-focused, NOT HS-grad. Distinct from spec/ tone.
- [ ] `design/future/panic-safety.md` explicitly states the try/catch rejection AND notes the Phase 5 graveyard entry will mechanically enforce it

**Verification**: `ls /workspaces/ynz/design/future/` shows 4 files (index + concurrency + panic-safety + supervisor). `ls /workspaces/ynz/design/ide-hints.md` exists. `grep -l "design/future/" /workspaces/ynz/design/future/*.md /workspaces/ynz/design/ide-hints.md` shows cross-linking. `grep "design/future/concurrency\|design/future/panic-safety\|design/future/supervisor\|design/ide-hints\|design/future/index" /workspaces/ynz/design/decisions.md` returns 5 matches.

---

### Phase 4b: New Design Docs — v0.3+ Deferred (5 docs)
**PR scope**: The v0.3+ future docs (self-references, no-runtime-mode, arena, http-framework, packages). Smaller than 4a, ships after to keep PRs reviewable.
**Branch**: `docs/design-lockdown-p4b-v0-3-design-docs`
**Flag**: N/A
**Est. lines**: ~750 (5 new files)
**Ships via**: `/pr`
**Objective**: Every v0.3+ design decision from the conversation has its own doc. Future index updated to point at them.
**Why this phase exists**: These are deferred farther out than 4a docs. Sized small enough to ship as one reviewable PR. Splitting 4a/4b avoids the 1500-line-one-PR anti-pattern the plan otherwise would have hit.
**Current-state anchors**:
- `design/future/` directory now exists (created in Phase 4a)
- `design/future/index.md` exists (Phase 4a); this phase appends entries for 5 more docs
- `design/decisions.md` updated in Phase 4a; this phase appends entries for 5 more docs
**Files (expected scope)**:
- NEW `design/future/self-references.md` (v0.3+)
- NEW `design/future/no-runtime-mode.md` (v0.3 — kernel mode flag)
- NEW `design/future/arena.md` (v0.2 for A1/A2, v0.3+ for Option B)
- NEW `design/future/http-framework.md` (v0.3+)
- NEW `design/future/packages.md` (binary format — DESIGN now, IMPLEMENT v0.2)
- `design/future/index.md` (append entries)
- `design/decisions.md` (append entries)
**Steps**:
1. Write `design/future/self-references.md`: Approach A (relative pointers) as the LOCKED design (decision confirmed by Patrick 2026-05-14 round 3 of design-lockdown plan). Opt-in via `self-referential` modifier on shape declarations (compiler auto-detects when not specified). ~1 cycle overhead per access (single addition for offset arithmetic). Rust's `Pin` lock-in explained. v0.3+ implementation. IDE shows muted `self-referential` hint when the compiler infers it. **Status header**: "Decided — Approach A. Alternatives B (fix-up-on-move) and C (pin-in-place) considered and rejected. B: too much move-semantics complexity, memory corruption risk in fix-up code. C: forces heap allocation, API constraints (struct can't move) — what Rust does, and a major usability problem we're explicitly avoiding."
2. Write `design/future/no-runtime-mode.md`: `--kernel` (or `--bare`) flag spec. What it disables (heap-allocating types without custom allocator, `background` without custom scheduler, default panic handler). Plug-in runtime architecture (kernel devs provide allocator, panic handler, output device). Compile-error format for blocked features (three-part: WHAT not available, WHAT alternative — `fixed<T,N>` or custom allocator, WHY no OS). Chipset/NASA target rationale. Every M3+ feature MUST declare runtime dependencies and kernel-mode behavior. Plan-reviewer enforces.
3. Write `design/future/arena.md`: A1 (`arena scratch { }` named scope) + A2 (`arena { }` anonymous scope) ship in v0.2 as the default. Allocations inside the block use the arena; arena wiped at scope end in one operation. ~10-100x faster than malloc for scope-bounded workloads. Option B (explicit `Arena()` + `.reset()`) deferred to v0.3+ with risks documented (leak potential, requires lifecycle tracking comparable to borrow checker). Note: Yinz compiler itself should use arenas for parse/typeck/codegen internally (separate task in v0.1 M8 polish todo list).
4. Write `design/future/http-framework.md`: supervision-by-default. `http.listen()` returns a supervised server. Request handlers run in isolated `background` tasks. Accept loop is supervised — if it crashes, framework restarts it. Default 500 handler. Custom panic policy via `supervise:` config option. IDE shows muted supervision config. v0.3+ (after stdlib basics in v0.2). Reference the BullMQ analogy from the conversation.
5. Write `design/future/packages.md`: binary package format must reserve space for: may-block metadata per function, ownership signatures (which params are share/lend/give), allocator requirements, kernel-mode compatibility flags per item. **THIS IS BAKE-IN-NOW**: the format must be designed v0.1 even though most metadata won't be populated until v0.2. Retrofitting later is painful. Cross-reference to `design/future/concurrency.md`. **On-disk field names** for the may-block metadata must use `mayBlock` (not `inferred*` — banned-jargon collision).
6. Append entries to `design/future/index.md` for the 5 new docs.
7. Append entries to `design/decisions.md` for the 5 new docs.

**Acceptance criteria**:
- [ ] 5 new design/future/*.md files exist at the listed paths
- [ ] `design/future/self-references.md` locks Approach A as the design (with rejection rationale for B and C)
- [ ] `design/future/packages.md` explicitly marks binary-format reservation as a v0.1 obligation (DESIGN in v0.1, IMPLEMENT in v0.2)
- [ ] `design/future/index.md` now lists all 9 future docs (4 from 4a, 5 from 4b)
- [ ] `design/decisions.md` has entries for all 9 future docs + ide-hints.md
- [ ] No future doc contains "acceptable for now" framing without explicit cost+trigger
- [ ] Cross-references resolve

**Quality gate**:
- [ ] Same as 4a — concise, engineer-focused, no premature design
- [ ] Status headers on each doc clearly state DECIDED vs DECISION-NEEDED-BEFORE-IMPLEMENTATION
- [ ] On-disk-format-name vs design-doc-vocabulary collision flagged in `packages.md`

**Verification**: `ls /workspaces/ynz/design/future/` shows 9 files (1 index + 8 topic). `grep "design/future/self-references\|design/future/no-runtime\|design/future/arena\|design/future/http-framework\|design/future/packages" /workspaces/ynz/design/decisions.md` returns 5 matches.

---

### Phase 5: Graveyard Entries + Plan File Forward-Compat
**PR scope**: Mechanical enforcement layer — five graveyard entries with concrete diff-greppable Bouncer checks, plus forward-compat sections added to existing in-flight plans.
**Branch**: `docs/design-lockdown-p5-enforcement`
**Flag**: N/A
**Est. lines**: ~350
**Ships via**: `/pr`
**Objective**: Bouncer will mechanically warn future-Claude when commits drift toward documented anti-patterns. v0-1-compiler.md and m3-control-flow-fns.md carry the invariants forward.
**Why this phase exists**: Without graveyard + plan forward-compat, the decisions in earlier phases are inert documentation. The enforcement loop is what prevents drift. Per `~/.claude/CLAUDE.md`: "Graveyard is the load-bearing knowledge base." Per `/workspaces/ynz/.claude/graveyard.md` field rules: entries need 8 fields (Scope, Exemption, Last verified, Cause, Detection signature, Constraint, Bouncer checks, Severity); Bouncer checks must be diff-greppable.
**Current-state anchors**:
- `.claude/graveyard.md` exists, currently has no corpses, format documented at top of file
- `~/.claude/memory/graveyard.md` format spec — read this first to align field shape
- `.claude/plans/active/v0-1-compiler.md` — umbrella plan; needs Forward-Compatibility Constraints section
- `.claude/plans/active/m3-control-flow-fns.md` — current active milestone plan; needs retroactive Invariants section per plan-invariants.md rule
**Files (expected scope)**:
- `.claude/graveyard.md` (5 new entries)
- `.claude/plans/active/v0-1-compiler.md`
- `.claude/plans/active/m3-control-flow-fns.md`
**Steps**:
1. Read `/workspaces/ynz/.claude/graveyard.md` field spec at top AND `~/.claude/memory/graveyard.md` format spec to align the 8-field shape exactly.
2. Add five graveyard entries — each populates all 8 fields with concrete diff-greppable Bouncer checks:

   **Entry 1: const-deep-immutability invariant unstated in milestone plans.**
   - **Scope**: `.claude/plans/active/m[0-9]+-*.md` and `.claude/plans/done/m[0-9]+-*.md` for M4+
   - **Exemption**: pre-M4 plans (M1, M2, M3) — they predate the rule
   - **Last verified**: 2026-05-14
   - **Cause**: M3 plan stated "ownership system arrives in M4" without enumerating which bindings reject which call-site operations or which LLVM attributes get emitted; the Gemini review surfaced the gap before it shipped, but the gap WAS real
   - **Detection signature**: M4+ milestone plan file that mentions `ownership`, `const`, `let`, `lend`, `give`, or `share` in its body but does NOT contain a literal `## Invariants This Milestone Must Preserve` heading AND a `### Safety` sub-heading enumerating const semantics within that section
   - **Constraint**: every M4+ milestone plan must contain the literal heading `## Invariants This Milestone Must Preserve` with at minimum a `### Safety` sub-section that enumerates const-blocks-reassignment, const-blocks-`.lend`, const-blocks-`.give`, const-blocks-field-mutation, AND a `### Performance` sub-section naming the LLVM `readonly`/`noalias` attribute emission
   - **Bouncer checks** (diff-greppable, runnable as shell):
     - `git diff HEAD~1..HEAD --name-only -- '.claude/plans/active/m[0-9]*' '.claude/plans/done/m[0-9]*'` → for each plan-file diff: extract milestone number from filename; if number >= 4, grep file for `^## Invariants This Milestone Must Preserve$`; if missing → CRITICAL
     - For each M4+ plan file: also grep for `^### Safety$` AND that the section body within 50 lines contains all five strings: `cannot be reassigned`, `cannot be lent`, `cannot be given`, `field mutation`, `readonly` → if any missing → CRITICAL
   - **Severity**: CRITICAL (this is the spine of the language's safety+performance contract; if it's not stated, M4 will ship without enforcing it)
   - **Originating incident**: 2026-05-14 Gemini review surfaced this gap; full conversation in plan `design-lockdown-from-gemini-review` round 1

   **Entry 2: requiring explicit ownership annotation at call sites (inverse anti-pattern).**
   - **Scope**: `spec/*.md` and `design/*.md` (NOT design/future/* which are speculation only)
   - **Exemption**: function signature documentation — signatures explicitly declaring `share`/`lend`/`give` is CORRECT (e.g., "function rename(lend player: Player)"); the anti-pattern is requiring annotation at CALL sites
   - **Last verified**: 2026-05-14
   - **Cause**: conversation considered requiring explicit `.share`/`.lend` at every call site; rejected in favor of uniform inference + IDE muted hints; without this graveyard entry, future-Claude could revert the decision
   - **Detection signature**: in `spec/*.md` or `design/*.md` (excluding `design/future/`), the phrases "must annotate at the call site", "explicit ownership at every call", "call sites must declare share/lend/give", OR a code example showing `foo(player.share)` where the surrounding text describes this as required rather than optional/inferred
   - **Constraint**: spec and design docs must describe call-site ownership as INFERRED-WITH-MUTED-IDE-HINT; explicit typing is described as available-for-clarity but never required
   - **Bouncer checks** (diff-greppable, runnable as shell):
     - `git diff HEAD~1..HEAD --name-only -- 'spec/*.md' 'design/*.md' | grep -v 'design/future/'` → for each file diff: grep for the strings (case-insensitive): `must annotate at`, `required at call site`, `explicit annotation.*call site`, `must declare.*at every call`; if any match → WARNING
     - Co-occurrence sub-check: grep for `\.share\|\.lend\|\.give` in changed lines; for each match-line, check the surrounding 5 lines for `must` AND `call`; if both present AND `function.*signature\|at the definition\|in the signature` is NOT present in same 10-line window → WARNING
   - **Severity**: WARNING (re-relitigation risk, not a runtime safety issue)
   - **Originating incident**: 2026-05-14 design conversation; previous spec wording "smart defaults at call sites" was specifically called out as confusing and replaced

   **Entry 3: M4+ milestone plans without the full 5-subsection Invariants section.**
   - **Scope**: `.claude/plans/active/m[0-9]+-*.md` and `.claude/plans/done/m[0-9]+-*.md` for M4+ (separate from Entry 1 — Entry 1 focuses on the const invariants; Entry 3 enforces the 5-subsection structure required by `.claude/rules/plan-invariants.md`)
   - **Exemption**: pre-M4 plans (M3 has a retroactive partial section per Phase 5 step 4 below; M1/M2 are done and exempt)
   - **Last verified**: 2026-05-14
   - **Cause**: without mechanical enforcement, future plans will skip subsections; the const-deep-immutability gap that triggered this whole conversation IS an example of this in real life
   - **Detection signature**: M4+ milestone plan that has `## Invariants This Milestone Must Preserve` but is missing one or more of the required 5 subsections: `### Safety`, `### Performance`, `### Teaching`, `### Runtime Dependencies`, `### Kernel-Mode Behavior`
   - **Constraint**: every M4+ milestone plan has `## Invariants This Milestone Must Preserve` AND all 5 named subsections, each non-empty
   - **Bouncer checks**:
     - For each M4+ plan file in the diff: extract the Invariants section (lines between `^## Invariants This Milestone Must Preserve$` and the next `^## ` or EOF); within that block grep for each of `^### Safety$`, `^### Performance$`, `^### Teaching$`, `^### Runtime Dependencies$`, `^### Kernel-Mode Behavior$` — every missing one → WARNING
     - Also check each subsection has at least one non-blank, non-heading line of content within 10 lines → WARNING per empty section
   - **Severity**: WARNING (structural enforcement; CRITICAL invariants covered by Entry 1)
   - **Originating incident**: 2026-05-14, sibling of Entry 1

   **Entry 4: language or stdlib features added without runtime/kernel-mode declaration.**
   - **Scope**: `.claude/plans/active/*.md`, `.claude/plans/done/*.md` for plans dated 2026-05-15 or later (cutoff = the day after this plan ships)
   - **Exemption**: plans that are exclusively docs/rules updates (no `crates/` files in their `files:` front-matter)
   - **Last verified**: 2026-05-14
   - **Cause**: kernel-mode goal (NASA, chipset) requires every new feature to declare its runtime dependencies; without enforcement, features ship with hidden heap/scheduler/io dependencies that block kernel use later
   - **Detection signature**: a plan with `files:` matching `crates/**` in its front-matter, dated 2026-05-15+, whose body does NOT contain `### Runtime Dependencies` AND `### Kernel-Mode Behavior` sub-sections (these are part of the 5-subsection Invariants from Entry 3, but called out separately because they're the KERNEL-specific check)
   - **Constraint**: any plan adding language/stdlib features (touches `crates/`) must declare runtime dependencies and kernel-mode behavior
   - **Bouncer checks**:
     - For each plan-file diff dated 2026-05-15+ with `crates/` in front-matter `files:`: grep for `^### Runtime Dependencies$` AND `^### Kernel-Mode Behavior$`; missing either → WARNING
   - **Severity**: WARNING (structural; kernel-mode is v0.3+ so this is forward-compat hygiene)
   - **Originating incident**: 2026-05-14 conversation — Patrick stated chipset/NASA as targets; without this entry, M4-M8 ship without thinking about it

   **Entry 5: re-introducing try/catch or recover blocks after explicit rejection.**
   - **Scope**: `crates/ynz-parser/**`, `crates/ynz-ast/**`, `spec/*.md`, `design/*.md` (excluding `design/future/panic-safety.md` which DOCUMENTS the rejection)
   - **Exemption**: documentation that explicitly references the rejection (e.g., "we considered try/catch and rejected it — see design/future/panic-safety.md")
   - **Last verified**: 2026-05-14
   - **Cause**: conversation rejected try/catch in favor of errors-auto-propagate + supervisor pattern; in 6 months someone will say "Erlang has supervisors AND catch, why can't we" and try to add it; this entry catches the regression
   - **Detection signature**: any of:
     - `crates/ynz-parser/src/lexer.rs` or `tokens.rs` adding a `Try`, `Catch`, or `Recover` keyword/token
     - `crates/ynz-ast/src/nodes.rs` adding a `Try`, `Catch`, or `Recover` AST variant
     - `spec/*.md` or `design/*.md` (excluding panic-safety future doc) introducing `try {` or `catch (` or `recover (` syntax in a Yinz code block
   - **Constraint**: panic handling uses errors-keyword auto-propagation + supervisor pattern at task boundary; no try/catch syntax
   - **Bouncer checks**:
     - `git diff HEAD~1..HEAD -- 'crates/ynz-parser/src/lexer.rs' 'crates/ynz-parser/src/token.rs' 'crates/ynz-ast/src/nodes.rs'` → grep for `Try\|Catch\|Recover` in added lines (excluding comments); if found → CRITICAL
     - `git diff HEAD~1..HEAD -- 'spec/*.md' 'design/*.md' ':!design/future/panic-safety.md'` → use pathspec exclusion (NOT `grep -v` on diff output, which strips lines not files — would leave offending `+try {` lines visible after the file header is stripped). Grep added lines for code-block patterns: ` try \{`, ` catch \(`, ` recover \(`; if found → WARNING
   - **Severity**: CRITICAL for AST/lexer (would land in compiler); WARNING for docs (re-relitigation only)
   - **Originating incident**: 2026-05-14 — earlier in the conversation I (Claude) proposed try/recover blocks; Patrick caught it and reminded me we'd previously rejected them. Without this entry, that mistake recurs.

3. Add `## Forward-Compatibility Constraints` section to `.claude/plans/active/v0-1-compiler.md`. Content: (a) binary format must reserve space for may-block metadata, ownership signatures, mayBlock-analyzed flags, kernel-mode flags (cite `design/future/packages.md`; field name `mayBlock` not `inferred*` per banned-jargon collision); (b) every feature from M3 onward must declare its runtime dependencies and kernel-mode behavior in its milestone plan (cite `.claude/rules/plan-invariants.md`); (c) every feature touching ownership at M4+ must enumerate the const invariants in the plan's Invariants section AND emit corresponding LLVM attributes in codegen; (d) `shape` keyword is reserved for M4 type declarations (not `type`); banned-jargon entry for `type` lands in Phase 6.
4. Add a retroactive `## Invariants This Milestone Must Preserve` section to `.claude/plans/active/m3-control-flow-fns.md`. All 5 subsections required by the rule:
   - **Safety**: control flow doesn't introduce mutation of const bindings; loop variables are immutable per existing typeck enforcement at `crates/ynz-typeck/src/check.rs:253`
   - **Performance**: control flow lowering preserves codegen quality, no new function-call overhead vs hand-written branches
   - **Teaching**: control flow diagnostics follow three-part format (currently enforced by Diagnostic constructor)
   - **Runtime Dependencies**: none — control flow is pure language; `range` builtin is M3-temporary and stack-only
   - **Kernel-Mode Behavior**: works in --kernel mode trivially — all branching/looping is stack-based, no heap, no scheduler

**Acceptance criteria**:
- [ ] Five new entries in `.claude/graveyard.md`, each with all 8 fields populated (Scope, Exemption, Last verified, Cause, Detection signature, Constraint, Bouncer checks, Severity)
- [ ] Every Bouncer check is a runnable shell command or `grep` against a specific file pattern — NO mental-simulation patterns
- [ ] `v0-1-compiler.md` has "Forward-Compatibility Constraints" section listing the four locked constraints
- [ ] `m3-control-flow-fns.md` has "Invariants This Milestone Must Preserve" section with all 5 required sub-sections, each non-empty
- [ ] No Bouncer regex false-positives on the in-repo test (Phase 7 validates)
- [ ] Each entry references its originating incident (date + plan slug)

**Quality gate**:
- [ ] Every graveyard entry matches the 8-field format at top of `/workspaces/ynz/.claude/graveyard.md` (read first to verify exact field names)
- [ ] Detection signatures and Bouncer checks are SPECIFIC enough that a model with no prior context can run them and produce yes/no answers
- [ ] Plan-file additions don't break existing plan content (Markdown structure preserved)

**Verification**: `grep "## Forward-Compat" /workspaces/ynz/.claude/plans/active/v0-1-compiler.md` shows the new section. `grep "Invariants This Milestone Must Preserve" /workspaces/ynz/.claude/plans/active/m3-control-flow-fns.md` shows the retroactive section. `grep -c "^### " /workspaces/ynz/.claude/graveyard.md` shows the new entries' field-header counts add up. Phase 7 runs the actual Bouncer regex tests against fixture diffs.

---

### Phase 6: Shape Keyword Rename (Docs + Banned-Jargon)
**PR scope**: Rename `type` → `shape` everywhere in spec and design docs as the user-facing keyword for shape declarations. Add `type` to banned-jargon with three-part error pointing to `shape`.
**Branch**: `docs/design-lockdown-p6-shape-rename`
**Flag**: N/A
**Est. lines**: ~400 (mostly doc edits; one diagnostic + one test row)
**Ships via**: `/pr`
**Objective**: All documentation uses `shape Foo { }` for shape declarations. Lexer banned-jargon catches anyone (human or AI) accidentally writing `type Foo {}` with a teaching error.
**Why this phase exists**: This is the only "real" code change (banned-jargon line). Renames are mechanical but tedious — isolating to its own phase keeps the diff reviewable. Doing this AFTER Phase 3 (existing doc updates) means the doc updates from Phase 3 don't conflict with this rename.
**Current-state anchors**:
- `crates/ynz-diagnostics/src/banned_jargon.rs:9` — `BANNED_JARGON` const array
- `crates/ynz-diagnostics/tests/jargon_audit.rs` — jargon audit tests
- `spec/types.md` (42 instances of "type"), `spec/overview.md`, ~70 `type Foo {}` patterns across spec+design
**Files (expected scope)**:
- All `spec/*.md` files referencing `type Foo { }` syntax (notably `spec/types.md`, `spec/overview.md`, anything with type-declaration examples)
- All `design/*.md` files referencing `type Foo { }` declarations
- `crates/ynz-diagnostics/src/banned_jargon.rs` (add `type` with replacement guidance)
- `crates/ynz-diagnostics/tests/jargon_audit.rs` (add test row for the new banned word)
**Steps**:
0. **Update source-of-truth FIRST** (load-bearing prerequisite — `banned_jargon.rs:2` says "Source of truth: `design/compiler-errors.md`"): edit `design/compiler-errors.md` jargon table to add a row for `type` with replacement `shape` and the three-part teaching rationale. Without this step, Phase 6 ships with `banned_jargon.rs` ahead of its declared source of truth — the exact "redundant state duplicating existing invariants" anti-pattern.
1. Spec rename pass: grep for `type [A-Z][a-zA-Z]*` patterns (declaration sites) in spec/ — change to `shape Foo {}`. DO NOT change generic-prose uses of "type" (like "the type of x", "a type system", "primitive types"). Use careful per-file review, not blanket sed. Files likely affected: `spec/types.md`, `spec/overview.md`, `spec/variables.md`, `spec/options.md`, `spec/unions.md`, `spec/generics.md`, `spec/destructuring.md`, `spec/control-flow.md`, `spec/scope.md`, `spec/functions.md`. Other spec files may have examples referencing types.
2. **Adversarial check during spec rename**: `spec/types.md` contains the phrase "the type system", "a primitive type", "type inference" AND example declarations `type Foo {}`. After the rename: the PROSE uses ("the type system", "type inference") MUST remain unchanged; only DECLARATION sites change to `shape`. Do this as per-file manual review, not sed. If unsure about a specific instance, leave it as-is and document in PR description for review.
3. Design doc rename pass: same approach for `design/*.md`. Likely: `design/type-system.md` (keep filename — it's the "type system" of the language, a meta-concept which IS still the type system; contents reference `shape` declarations), `design/decisions.md`, `design/teaching-mission.md`, `design/golden-rules.md`, `design/naming.md`, etc.
4. Update `spec/overview.md` keyword list: ensure `shape` appears, `type` does not (as a declaration keyword — it's fine in prose).
5. Update `.claude/rules/naming.md`: confirm `type → shape` row is the authoritative entry (added in Phase 1; verify nothing in this phase contradicts).
6. Add `type` to `crates/ynz-diagnostics/src/banned_jargon.rs` `BANNED_JARGON` array. The const comment at top of file gets the dual-audience clarification added in Phase 1 Step 0 (banned-jargon governs user-facing diagnostics; design docs use `infer`/`inference` deliberately). Use the existing three-part error format. Suggested error:
   - WHAT: "Yinz uses `shape` to declare a data structure, not `type`."
   - WHAT-INSTEAD: "Replace `type Foo { ... }` with `shape Foo { ... }`."
   - WHY: "`type` is overloaded — it's also the everyday word for 'what kind of thing this is'. `shape` is unambiguous: a shape is the structure of your data. (This also matches Golden Rule 2: self-documenting syntax.)"
7. Add a test row to the sync-test in `crates/ynz-diagnostics/tests/snapshots.rs` (the sync-test referenced in `banned_jargon.rs:3`) AND/OR `crates/ynz-diagnostics/tests/jargon_audit.rs` (whichever owns the cross-check between `design/compiler-errors.md` jargon table and the Rust const). Test asserts: every word in `BANNED_JARGON` appears in the `design/compiler-errors.md` jargon table.
8. Run `cargo test --workspace` to confirm no breakage.
9. **Fallback if `cargo test` fails**: if the sync-test or any other test breaks because of the `type` addition, ROLL BACK the banned-jargon entry, fix the failing test or doc, then re-add the entry. Do NOT bypass the test — the sync-test is the integrity check we just relied on for the source-of-truth chain.

**Acceptance criteria**:
- [ ] `grep -rn "^type [A-Z]" spec/ design/` returns zero results (all declaration sites renamed)
- [ ] `grep -rn "type Player\|type Foo\|type User\|type Order" spec/ design/` returns zero results (no example types using old keyword)
- [ ] `spec/overview.md` keyword list includes `shape`, does not include `type` as a declaration keyword
- [ ] `crates/ynz-diagnostics/src/banned_jargon.rs` has `type` in BANNED_JARGON with three-part teaching error pointing to `shape`
- [ ] `cargo test --workspace` passes including the new banned-jargon test row
- [ ] No design or spec file's prose discussion of "the type system", "type inference", "type checking", etc. is broken (those are correctly still about the type system, which the language has)

**Quality gate**:
- [ ] No mechanical sed-style replacement that changes "the type of x" prose to "the shape of x" — those are different concepts
- [ ] Examples in spec/ remain HS-grad readable with the new keyword
- [ ] design/teaching-mission.md banned-jargon list reflects this addition
- [ ] No file references `shape` and a stale `type` declaration in the same example (consistency within each file)

**Verification**: `grep -c "shape " /workspaces/ynz/spec/types.md` is greater than zero. `grep -rn "type Player\|type Foo" /workspaces/ynz/{spec,design}/` returns no matches. `cargo test -p ynz-diagnostics` passes.

---

### Phase 7: Verification Sweep
**PR scope**: Verification phase per `/plan` skill Step 10. Confirms every prior phase landed correctly, sweeps for orphaned items, updates todos, closes the plan.
**Branch**: `docs/design-lockdown-p7-verification`
**Flag**: N/A
**Est. lines**: ~100 (mostly todos.md cleanup + final closing notes)
**Ships via**: `/pr`
**Objective**: Verify all 26 checklist items landed. Update `.claude/todos.md` if anything spilled forward. Mark this plan complete and move to `done/` after merge.
**Why this phase exists**: Per `/plan` Step 10: every plan's final phase is a verification sweep. This catches stragglers, missing cross-links, and any work that didn't fit cleanly into prior phases.
**Current-state anchors**:
- `.claude/todos.md` — current cross-workstream backlog; may need updates from this plan
- `.claude/state.md` — radar; updated by SessionStart hook automatically
**Files (expected scope)**:
- `.claude/todos.md` (add any v0.2/v0.3 follow-ups that emerged)
- `.claude/plans/active/design-lockdown-from-gemini-review.md` (this file; status → ready-for-done)
- Possibly small corrections to any file from prior phases that the sweep finds
**Steps**:
1. TODO sweep: grep entire repo for `TODO`, `FIXME`, `HACK`, `XXX`, `// will be`, `// eventually` — any new ones introduced by this plan's phases get moved to `.claude/todos.md` (none should have been introduced; this is a paranoia check).
2. Todos cross-check: review `.claude/todos.md`; add any follow-ups this plan generated:
   - "M8 polish: switch compiler internals to arena allocation (parse, typeck, codegen) — cite `design/future/arena.md`"
   - "M4 plan when drafted: must include Invariants section per `.claude/rules/plan-invariants.md` — specifically the const→readonly LLVM contract"
   - "v0.2 LSP work: implement IDE muted hints per `design/ide-hints.md`"
   - "v0.2 binary format: reserve metadata fields per `design/future/packages.md`"
   - Any others surfaced during phases 1-6
3. Cross-link verification: spot-check that every cross-reference from new docs resolves. `grep -rn "\.md)" /workspaces/ynz/design/future/ /workspaces/ynz/design/ide-hints.md` — every `[link](path)` must point to a real file.
4. Vocabulary consistency: `grep -c "shape\|type " /workspaces/ynz/spec/*.md /workspaces/ynz/design/*.md` and spot-check that "type" only appears in prose (the type system, type inference, etc.) and not in declaration positions.
5. **Real Bouncer-pattern verification — fixture diffs, not mental simulation**: for each of the 5 graveyard entries from Phase 5, build a small fixture file with content that SHOULD trigger the entry, then construct and run the exact grep/regex from the entry's Bouncer Checks field against that fixture. Verify each check produces the expected yes/no:
   - Entry 1 fixture: a `.claude/plans/active/m4-fake.md` with body mentioning "ownership system" but NO `## Invariants This Milestone Must Preserve` heading → Bouncer check should output CRITICAL warning
   - Entry 2 fixture: a `spec/fake.md` with text "the developer must annotate at the call site with .share" → check outputs WARNING
   - Entry 3 fixture: a `.claude/plans/active/m4-fake.md` with `## Invariants This Milestone Must Preserve` but only `### Safety` subsection (missing Performance, Teaching, Runtime Dependencies, Kernel-Mode Behavior) → check outputs 4 missing-subsection warnings
   - Entry 4 fixture: a `.claude/plans/active/m5-fake.md` with `files:` listing `crates/**` and no `### Runtime Dependencies`/`### Kernel-Mode Behavior` subsections → check outputs WARNING
   - Entry 5 fixture: a diff to `crates/ynz-ast/src/nodes.rs` adding `Try` variant → check outputs CRITICAL
   - Also build NEGATIVE fixtures (content that should NOT trigger): well-formed plans, signature documentation correctly using ".lend", design/future/panic-safety.md mentioning try/catch in the "rejected" context — verify checks output zero warnings
   - Document the fixture files and check results in this phase's PR description
   - Delete the fixture files after verification (they're for verification only, not committed)
6. **Bouncer integration verification**: trigger one Stop event (e.g., make a trivial edit and let the Stop hook fire) OR manually run the global Bouncer auditor against `git diff HEAD~1..HEAD` and verify the new graveyard entries' patterns are loaded and active. Check `~/.claude/.bouncer.log` for references to the new entries.
7. Mark this plan's status as "ready-for-done": update front-matter `status: done`, update `last_updated` date, add a final summary section listing what shipped per phase.

**Acceptance criteria**:
- [ ] No new orphaned TODOs in the codebase introduced by phases 1-6
- [ ] `.claude/todos.md` reflects every v0.2+ follow-up this plan generated
- [ ] All cross-references in new docs resolve to real files
- [ ] "type" appears in prose only, not in declaration positions, across all spec/design files
- [ ] Each of the 5 graveyard entries has been verified against a positive AND negative fixture diff — yes/no results documented in PR
- [ ] Bouncer integration verified — log shows new patterns active
- [ ] This plan file's `status:` front-matter is updated to mark completion
- [ ] Final summary section lists each phase's actual deliverables (PR numbers if available)

**Quality gate**:
- [ ] If anything didn't land cleanly in phases 1-6, this phase catches it and either fixes it (small) or moves it to a follow-up plan (large)
- [ ] Verification doesn't introduce new design decisions — it's a closing/cleanup phase
- [ ] Bouncer/Stop hook output is reviewed for any new warnings from the merged changes
- [ ] No graveyard regex is left as a "should work" — every one has been EXECUTED against a fixture

**Verification**: `cat /workspaces/ynz/.claude/plans/active/design-lockdown-from-gemini-review.md | head -20` shows `status: done`. `git log --oneline -10` shows the eight phase commits (4a and 4b each ship as PRs). After merge to main, move the plan file to `.claude/plans/done/` per project convention.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: each phase is one PR with one branch (named `docs/design-lockdown-pN[a|b]-*`). Phases are sized to be reviewable on their own; no "split later." Phase 4 was originally 1500 lines and got split into 4a/4b after plan-reviewer round 1 flagged the size.
- **Shadow main branches**: each phase merges to `main` before the next starts. No long-lived umbrella branch holding all 8 phases (1, 2, 3, 4a, 4b, 5, 6, 7).
- **Building the engine before shipping value**: every phase delivers durable artifacts (rules, docs, enforcement) that survive beyond this plan. Phase 1 alone — vocabulary + inference rule — would still be valuable even if the rest never shipped.
- **Hotfix that isn't**: this plan is not a hotfix. It's a design-lockdown. Marked as docs branches throughout, no production-fix framing.
- **Abandoned branches**: each phase is sized for one session. Phase 4a/4b are ~750 lines each spread across 5 distinct design files per phase — large in raw-line count but each file is self-contained and review-friendly. The 500-line code-PR guidance applies most strictly to code paths; spread-across-files docs work scales differently.
- **Flag graveyards**: no feature flags are introduced. The `--kernel` flag is DESIGNED in `design/future/no-runtime-mode.md` but not implemented in this plan (v0.3 work).

---

## Quality Checklist (verify at completion — Phase 7)

- [ ] Every conversation decision appears in exactly one durable location (rules, design doc, graveyard, or plan amendment)
- [ ] **META-invariant**: `const` deep-immutability statement appears in all three required places: `design/ownership.md`, `v0-1-compiler.md` forward-compat section, `.claude/graveyard.md` Entry 1
- [ ] `shape` is the keyword in all docs; banned-jargon catches `type` declarations with a teaching error
- [ ] `design/compiler-errors.md` jargon table updated WITH `type` BEFORE `banned_jargon.rs` was edited (source-of-truth chain preserved)
- [ ] Golden Rule 8 wording is UNCHANGED at the headline; clarification is a labeled addendum
- [ ] Golden Rule 11 covers IDE teaching surfaces explicitly (not just compiler errors)
- [ ] `.claude/rules/inference.md`, `plan-invariants.md`, `vocabulary.md` all exist and CLAUDE.md references them
- [ ] `.claude/rules/inference.md` and `crates/ynz-diagnostics/src/banned_jargon.rs` BOTH carry the dual-audience clarification (rule files use `infer`/`inference`; user-facing diagnostics don't)
- [ ] All 10 new files exist (5 from 4a + 5 from 4b under `design/ide-hints.md` and `design/future/*.md`)
- [ ] All five graveyard entries (including the try/catch-return one) have all 8 required fields populated AND concrete diff-greppable Bouncer checks AND have been verified against fixture diffs
- [ ] `m3-control-flow-fns.md` has retroactive Invariants section with all 5 sub-sections
- [ ] `design/decisions.md` indexes the new design docs (updated in Phases 4a and 4b, NOT Phase 5)
- [ ] `cargo test --workspace` passes after Phase 6 (banned-jargon change); sync-test passes
- [ ] No new TODOs introduced by this plan; any follow-ups are in `.claude/todos.md`
- [ ] Plan file `status:` field is `done` after Phase 7

---

## Execution Notes (live as phases ship)

### Phase 5 — partial execution, two items deferred

Phase 5 originally planned three actions: (a) add 5 graveyard entries, (b) add `## Forward-Compatibility Constraints` section to `.claude/plans/active/v0-1-compiler.md`, (c) add retroactive `## Invariants This Milestone Must Preserve` section to `.claude/plans/active/m3-control-flow-fns.md`.

At Phase 5 execution time (2026-05-14), the M3 chat had uncommitted modifications to `v0-1-compiler.md` (restructuring it into the umbrella-with-milestones format) and the file `m3-control-flow-fns.md` does not yet exist on `main` (it's on the `feat/m3-codegen` branch). Touching either file from this plan's chat would entangle Claude with the M3 chat's in-flight work and cause merge conflicts.

**Shipped in Phase 5**: 5 graveyard entries in `.claude/graveyard.md` (load-bearing — the enforcement that makes the prior phases real).

**Deferred to a follow-up commit/PR after the M3 chat commits**:
- Add `## Forward-Compatibility Constraints` section to `.claude/plans/active/v0-1-compiler.md`
- Add retroactive `## Invariants This Milestone Must Preserve` section to `.claude/plans/active/m3-control-flow-fns.md`

**Coordination**: this plan's owner (Patrick) tells the M3 chat to re-read its context after their work commits to main. Once committed, a small follow-up commit on a future phase (or a dedicated tiny PR) lands the deferred items. This avoids cross-chat merge conflicts.

Phase 7 verification sweep checks that these deferrals were resolved before closing the plan.

---

## Reviewer Disputes

### Round 1 — Plan-reviewer flagged 8 required fixes; resolutions:

1. **Banned-jargon collision with `inference.md` rule file** — RESOLVED by adding Phase 1 Step 0 (dual-audience clarification in both `inference.md` and `banned_jargon.rs` top comments). The collision is intentional and documented.
2. **Phase 6 source-of-truth chain (design/compiler-errors.md must be updated first)** — RESOLVED by adding Phase 6 Step 0 (update doc before code). Front-matter `files:` updated to include `design/compiler-errors.md`. Sync-test added to Phase 6 Step 7.
3. **Graveyard regex patterns too vague** — RESOLVED by rewriting all entries (now 5, including the new try/catch entry) with full 8-field format and concrete diff-greppable Bouncer checks. Each check is a runnable shell command.
4. **Phase 4 oversized at 1500 lines** — RESOLVED by splitting into Phase 4a (v0.2-target, 5 files / ~750 lines) and Phase 4b (v0.3+, 5 files / ~750 lines). Each PR now ships in spec-writing-friendly chunks.
5. **No graveyard entry catching try/catch return** — RESOLVED by adding Entry 5 to Phase 5 (try/catch/recover token/AST regression detection with CRITICAL severity for compiler changes, WARNING for docs).
6. **Phase 7 step 5 was rubber-stamp (mental simulation)** — RESOLVED by replacing with real fixture-based verification. Each graveyard entry now must pass against both positive and negative fixture diffs.
7. **Question #4 (M4 stub plan) self-answered** — RESOLVED by moving from "Questions for Patrick" to "Locked Decisions" section (option b confirmed: rules file is the durable mechanism).
8. **`spec/ownership.md` existence check** — RESOLVED: confirmed file exists via `ls /workspaces/ynz/spec/` (it's in the spec directory listing). Phase 3 Step 2 correctly treats it as an edit, not a new file.

### Non-blocking concerns also addressed in round 2:

- **"inferred-wait points" naming collision in packages.md** — addressed by specifying on-disk field name should be `mayBlock` (not `inferred*`); design doc itself uses inference terminology freely.
- **Self-references Approach A confirmation needed** — moved to Phase 4b status header ("decision-needed-before-implementation"). Also kept as Question 3 to Patrick.
- **`design/teaching-mission.md` location pinning** — Phase 3 Step 4 still says "add a section 'IDE as Teaching Surface' or similar" but executor instruction now is to pin placement after existing content discussing teaching surfaces (golden rule 11 cross-reference area).
- **design/decisions.md index timing** — moved from Phase 5 to Phase 4a/4b so the index is never stale, even briefly.
- **Phase 6 fallback for `cargo test` failure** — added explicit rollback step (don't bypass the test; fix the file or doc first).
- **`crates/ynz-diagnostics/tests/snapshots.rs`** — added to front-matter `files:` because that's where the sync-test lives.
