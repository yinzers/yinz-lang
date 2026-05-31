---
slug: v0-2-1-m10-teaching-surface-bugfix
type: execution
roadmap: v0-2-1-lsp-gap-closure
owner: Patrick Rizzardi
status: active
created: 2026-05-30
last_updated: 2026-05-30 (Phase 2 complete)
files:
  - crates/ynz-typeck/src/check.rs
  - crates/ynz-typeck/src/inlay_hint_passes.rs
  - crates/ynz-typeck/tests/**
  - crates/ynz-lsp/src/hover.rs
  - crates/ynz-lsp/src/inlay_hint.rs
  - crates/ynz-lsp/src/code_action.rs
  - crates/ynz-lsp/src/capabilities.rs
  - crates/ynz-lsp/tests/**
  - crates/ynz-registry/src/lib.rs
  - registry/features.toml
  - tooling/vscode-ynz/package.json
  - examples/pirates-roster/entrypoint.ynz
---

# Plan: v0.2.1 M10 — Teaching-Surface Correctness (Bug Hunt Cluster)
Created: 2026-05-30
Status: EXECUTING (whole-plan unattended, isolated worktree) — PAUSED after Phase 6 at Patrick's request (context full)

> **RESUME POINTER (2026-05-30) — read this first:**
> Executing via `/execute-plan` (whole-plan, all-PASS-to-commit) in worktree `.claude/worktrees/v0-2-1-m10-teaching-surface-bugfix`, branch `v0.2.1-m10-teaching-surface-bugfix` off `main` (main untouched @ d509770; seed bd07e69).
> **DONE (6/12), each 4-reviewer-gated + committed:** P0 `641896a` (unused-import FPs, Bug 1+2.1–2.5) · P1 `0d5c985` (else_arm, 2.6) · P2 `c9da88f` (nested-assign+MethodCall root, 2.7) · P3 `572a6d8` (ownership-aware mutation+literals, 2.9+2.10, intrinsic-aware) · P4 `e094df8` (inlay positioning + Yinz-gold #ffd23f) · P5 `586db65` (array→fixed click-edit, 2.13, + a Phase-3 verification-gap test fix) · P6 `aea528c` (hover context-aware + end-of-token, 2.8+2.12).
> **REMAINING (5): P7** (space-trigger removal + BannedJargon quick-fix carrying the WHY) · **P8** (booleanean typo + 2 `infers` jargon) · **P9** (ownership-hint generic/UFCS, 2.11) · **P10** (copy-hint recursion, 2.14) · **P11** (pirates-roster demo extension + final cumulative sweep + end-of-plan BEFORE/AFTER report for Patrick).
> **To resume:** `/execute-plan` on this plan in the worktree. Per-phase loop = plan-executor → 4 reviewers (code/rules/adherence/acceptance, all PASS to commit) → coordinator writes evidence+gates+SHA. RUN `cargo test -p ynz-typeck` AND `-p ynz-lsp` (a Phase-3 gap was a typeck change breaking an untested ynz-lsp test). Executors must NOT run `cargo fmt` (worktree rustfmt baseline mismatched → whole-crate churn). KNOWN NON-ISSUES: 5 ynz-driver integration snapshot tests fail on a worktree absolute-path artifact (NOT regressions); ynz-lsp has 5 pre-existing clippy errors (v0.3.0-m1 condition, NOT M10) → bar is "no-NEW", tracked for a separate cleanup.
> **TRACKED follow-up nits (carry into the end-of-plan report, NOT blockers):** (1) Phase 2 — `nested_method_call_…` test hardcodes byte offset 268 (brittle if fixture edited); (2) Phase 4 — multi-line `let` initializer anchors the promotion hint at end-of-first-line; (3) Phase 6 — Bug 2.12 EOF-flush edge (last token vs EOF, no trailing newline → None); (4) intrinsic-fns-not-in-sigtable memory (P3 conservative fallback for imported fns).
> **M9 (separate roadmap plan, NOT M10):** v0.2.1 ships as its OWN `0.2.1` tag FIRST, THEN merges into the v0.3 line.
>
> --- original decision lock (2026-05-30, all honored during execution) ---
> plan-reviewer PASS (Tier A, zero Required Fixes); four locked decisions:
> 1. **P9/P10 (Bugs 2.11/2.14): KEEP in M10.** No split.
> 2. **Phase 6 hover: context-aware fix ONLY (no fallback).** The narrow fallback is duct tape — it relocates the wrong-hover hole rather than closing it (violates GR11 teaching mission + no-duct-tape "no caller violates this today"). Use the EXISTING `type_of_expression_at_offset` (`type_at_offset.rs:44`) / `identifier_use_site_at_offset` (`ast_offset.rs:19`) infra. If that infra genuinely can't deliver it (it can — verified present), STOP and raise as a blocker; do NOT silently downgrade.
> 3. **Phase 7: YES add `DiagnosticKind::BannedJargon { term }`.** Plus the quick-fix MUST carry the WHY (the lesson), not just the replacement word — sourced from the registry `[[banned_jargon]]` `why` field. (`enum` already teaches+fixes via the BannedKeyword path; Phase 7 brings parity to the jargon class.)
> 4. **Dispatch: ISOLATED GIT WORKTREE** (separate), per the roadmap-locked model — main=0.3.0-m1 with v0-3-m2 active, so M10's v0.2.1 track stays out of the main checkout.
>
> Baseline at plan time: 1434 tests green on main. Ready for `/execute-plan` in a worktree whenever Patrick says go.

## Context & Why

**Goal**: Fix every bug found by the 2026-05-21 four-agent teaching-surface audit so that Yinz's compiler-as-teacher surfaces (unused-import diagnostics, inlay hints, hover, completion, banned-jargon quick-fixes) stop being silently wrong. This is the first milestone of the `v0-2-1-lsp-gap-closure` roadmap to dispatch — Patrick's locked call (roadmap Q12): "bug fixes ship to users in days, not weeks."

**Why**: A teaching language whose teaching surface lies is worse than one with no hints. Every confirmed bug here is a Tier 1/2 teaching regression — the user sees a warning on valid code (and learns to ignore *all* warnings), or sees a "this is effectively const" hint on a binding that IS mutated (and learns the hint can't be trusted). The dominant case is brutal: Bug 2.9 means almost every `let` passed to *any* call (including `print(x)`) gets its `let → const` hint suppressed — so the headline auto-promotion teaching surface almost never fires in real code.

**Background (current state)**:
- v0.2.0 shipped the LSP foundation (go-to-def, find-refs, rename, format, inlay hints, 9 code-action types).
- A user reported the `Timeframe` unused-import false positive (Bug 1). The four-agent audit (`.analysis/bugs.md`, `.analysis/teaching-content-audit.md`, `.analysis/lsp-ux-friction.md`) generalized it: Bug 1 shares a root cause with five more unused-import sites, plus 8 other independent teaching-surface bugs.
- The codebase has moved since the audit — `v0.3.0-m1` shipped (Cargo `version = "0.3.0-m1"`). All audit line numbers drifted +150–900 lines and were re-verified against current source on 2026-05-30 (see anchors below). **Current baseline: 1434 tests green, 0 failures.**

**Constraints**:
- No language changes, no syntax changes, no compiler ABI changes — purely additive/corrective on the typeck + LSP side.
- v0.2.1 is its own track, branched off `main` (which sits at `0.3.0-m1`), shipped first as a `0.2.1` tag, then merged into the v0.3 line later (M9 owns that). M10's diffs are additive and merge cleanly into v0.3.
- Every fix must keep the WHAT/WHAT-INSTEAD/WHY diagnostic format and Yinz vocabulary (no banned jargon — `infer`/`inferred` is itself banned in user-facing text, which is why P8 exists).

**Success criteria**: Every bug below has a regression test that FAILS on the pre-fix commit and PASSES after the fix. The `examples/pirates-roster/entrypoint.ynz` demo exercises all six previously-false-positive import patterns and produces ZERO spurious unused-import warnings. `cargo test --workspace` stays green (≥1434 + the new regression tests). `cargo clippy --workspace -- -D warnings` clean.

---

## Scope Note — This Plan Is a Superset of the Roadmap's M10 P0–P8

The roadmap's M10 rough-scope lists phases P0–P8, mapping to audit Bugs 1, 2.1–2.9, 2.12, 2.13 + the typo/jargon fixes. But M10's own value statement claims **"every bug found by the audit gets fixed."** Three confirmed real bugs are silently absent from the P0–P8 list:

- **Bug 2.10** — `collect_maybe_mutated_expr` doesn't recurse into `StructLit`/`ArrayLit`/`MapLit`/`PostfixOp` (mutations inside literals missed). *(Verified narrower than the audit stated — `MethodCall` IS already handled.)*
- **Bug 2.11** — `ownership_call_site_hints` doesn't handle generic functions or UFCS method-call form (`player.heal(20)` gets no ownership hint; `heal(player, 20)` does).
- **Bug 2.14** — `collect_copy_hints_expr` doesn't recurse into call args (nested `outer(inner(x))` misses `x`'s copy hint).

Per CLAUDE.md Rule 11 (all confirmed findings get fixed; priority labels are ordering hints, not gates) and `no-duct-tape.md`, leaving them out would be an undocumented deferral that contradicts the milestone's stated promise. **Decision baked into this plan**: Bug 2.10 folds into Phase 3 (same function being edited); Bugs 2.11 and 2.14 become Phases 9 and 10. See the Questions section — Patrick can split P9/P10 into a v0.2.1 follow-up if he wants M10 to match the roadmap's P0–P8 letter exactly, but the default is "fix them now."

---

## Research Findings (verified against current source 2026-05-30)

All anchors below are CURRENT line numbers, re-verified post-`v0.3.0-m1`. Every cited bug pattern still exists — no no-op phases.

**Unused-import `referenced_names.insert` gaps (Bug 1 + 2.1–2.5), all in `crates/ynz-typeck/src/check.rs`:**
- `check_options_value` — **3736** (was 3574). Returns `Type::Options` with no insert.
- `check_is_arm_pattern` — **3634** (was 3472); `check_is_expr` — **3684** (was 3522). Both validate `type_path.name`, no insert.
- `check_follows_contracts` — **2664** (was 2502).
- `check_module` match arms — **222**; `Item::ShapeDecl(_) => {}` at **230**; `ConstDecl` now folded into combined arm `Item::ImportDecl(_) | Item::ConstDecl(_) | Item::ReExport(_) => {}` at **236** (shape change — edit the combined arm).
- `AstType::Dynamic` — **2456** (was 2294); `if self.shape_table.contains(contract)` branch, no insert.
- `AstType::Generic` user-defined fallthrough — **2541–2549** (was 2379); `if self.generic_shape_table.contains(name)` branch, no insert.
- **Correct-pattern reference examples**: `AstType::Named` options insert at **2403**, shape insert at **2407**; free-fn name insert at **1485**; UFCS method name insert at **2216**.

**Inlay-hint walkers, all in `crates/ynz-typeck/src/inlay_hint_passes.rs` (533 lines):**
- Six `Stmt::Match` arms drop `else_arm` via `..`: lines **142** (`collect_maybe_mutated_stmt`), **250** (`collect_type_hints_block`), **311** (`collect_ownership_hints_block`), **402** (`collect_copy_hints_block`), **474** (`collect_array_hints_block`), **525** (`collect_const_hints_block`). `Stmt::Match.else_arm: Option<Block>` confirmed at `crates/ynz-ast/src/nodes.rs:246`.
- Nested assign root-binding: **119–134**; `Stmt::FieldAssign` (121–123) + `Stmt::IndexAssign` (129) only handle one-level `Expr::Ident` receiver.
- `collect_maybe_mutated_expr` — **160–198**; `Expr::Call` arm (164–172) marks every arg ident mutated, no callee-ownership check. Function signature is `fn collect_maybe_mutated_expr(expr: &Expr, out: &mut HashSet<String>)` — **no `sig_table` param** (must thread one in). `MethodCall` IS handled (line 173); the genuine coverage gap (Bug 2.10) is `StructLit`/`ArrayLit`/`MapLit`/`PostfixOp` falling through `_ => {}` at 196.
- `collect_ownership_hints_expr` — **330–364**; line **339** `sig_table.fns.get(name).or_else(|| imported.get(name))` never queries `generic_fn_table`; the guard at 336 matches only `Expr::Call`, not `Expr::MethodCall`.
- `collect_copy_hints_expr` — **416–433**; inspects top-level call args, no recursion into them.
- Promotion hints: `PromotionHint` struct at **73–80** (fields `position: usize`, `kind: PromotionKind`, `label: String`). `array_to_fixed_promotion_hints` sets `position: span.start` at **464**; `let_to_const_promotion_hints` at **515**.

**LSP, `crates/ynz-lsp/src`:**
- `hover.rs:112–121` — registry `lsp_hover_for_token` tried before user-symbol fallback (which is at 124).
- `hover.rs:23` — `byte_offset >= tok.span.start && byte_offset < tok.span.end` (strict `<` upper bound).
- `capabilities.rs:38` — `trigger_characters: Some(vec![".".to_string(), " ".to_string()])` (space present).
- `code_action.rs:57–76` — arms: `BannedKeyword`, `NotDefined`, `UnusedImport`, `_ => None`. **No `BannedJargon` arm.**
- `inlay_hint.rs` — `let_to_const_edit` helper at **120**; **no `array_to_fixed_edit` helper exists**. `make_hint` (76) vs `make_hint_with_edit` (85) distinction exists. `array_to_fixed` uses plain `make_hint` (no edit) at **221–226**; `let_to_const` uses `make_hint_with_edit` at **232+**.

**Registry:** `lsp_code_action_replacement_for(diagnostic_kind: &str, token: &str) -> Option<&'static str>` is at **`crates/ynz-registry/src/lib.rs:180`** (NOT `lsp_adapter.rs` as the audit said). Only matches `"BannedKeyword"` via `SIMPLE_KEYWORD_REPLACEMENTS` (185–188); does not search `[[banned_jargon]]`.

**Typo/jargon, `check.rs`:** `booleanean` at **1577**; `"infers"` at **1997** and **2010** (only 2 sites, not the audit's 3).

**Error gallery:** `examples/primantis-orders/` latest is `v0_3_m1_errors.ynz`. M10 adds NO new compile-error class (it fixes false-positives), so it adds NO new gallery trigger — see the `### Demo & Error Gallery` invariant for how that obligation is satisfied instead.

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Bug 2.9 fix (Phase 3) makes `let → const` hints suddenly appear on many bindings users assumed never triggered | High | Low | This is the CORRECT behavior; the old over-suppression was the bug. Document in the v0.2.1 CHANGELOG. M9's per-domain inlay toggles let users mute the domain (defaults stay on). |
| Phase 6 hover reorder breaks keyword hover for genuine keyword uses (`share self` in a signature now shows nothing / wrong content) | Low | Medium | Phase 6 is LOCKED to the context-aware fix using the existing `type_of_expression_at_offset` / `identifier_use_site_at_offset` infra (verified present). Value-position expression → user symbol; non-expression (signature modifier) → keyword hover. Required tests: hovering `share` in a signature modifier STILL returns the keyword hover; `let share = 5; share + 1` returns the variable type. No fallback heuristic is permitted (it relocates the bug). |
| Phase 3 `sig_table` threading touches many walker signatures; risk of incomplete threading leaving some call sites unchecked | Medium | Medium | Phase 3 acceptance requires the threaded param reach `collect_maybe_mutated`, `collect_maybe_mutated_stmt`, AND `collect_maybe_mutated_expr`; a test asserts `foo(lend x)` suppresses while `print(x)` fires. Generic-fn + UFCS resolution covered by explicit test cases. |
| Phase 7 `BannedJargon` quick-fix assumes a `DiagnosticKind::BannedJargon { term }` variant exists; it may not | Medium | Medium | Phase 7 Step 1 confirms what `DiagnosticKind` variant the banned-jargon diagnostic uses. If no `term`-carrying variant exists, Phase 7 adds one (or extends the existing one) before wiring the code action — see Questions. |
| Phase 4 end-of-statement positioning helper needs source text; `inlay_hint_passes` is a salsa query | Low | Low | The pass already has `sf.text(db)`; the helper takes `(text, stmt_span)`. Verified the pass keys on `SourceFile`. No new salsa input. |
| Phase 0 unified PR (6 sites + 2 walks) rejected by reviewer as "too many changes" | Low | Medium | Shared root cause + shared regression-test file; splitting creates 6 near-duplicate PRs. If the reviewer insists, fall back to 3 PRs grouped by AST-position-family (variant/pattern access; declaration walks; type-position annotations). |
| Array→fixed click-edit (Phase 5) produces invalid source if `fixed<T>` needs an explicit size | Low | Medium | Roadmap locked the approach as a keyword token-swap (`array` → `fixed`), size inferred from the literal — consistent with `design/collections.md` auto-promotion. Phase 5 verification compiles the post-edit source to prove it's valid. |

---

## Questions — ALL RESOLVED 2026-05-30 (kept for audit trail)

1. **Superset scope (P9 + P10)** — **RESOLVED: KEEP in M10.** Bugs 2.11 (ownership-hint generic/UFCS) and 2.14 (copy-hint recursion) stay as Phases 9–10. Bug 2.10 stays folded into Phase 3. No split to a follow-up.
2. **Phase 6 hover-reorder depth** — **RESOLVED: context-aware fix ONLY, no fallback.** The narrow fallback was rejected as duct tape — it relocates the wrong-hover hole (a file with both a `share` variable and a `share` signature modifier would hover wrong in the opposite direction) rather than closing it, violating GR11 + no-duct-tape. The existing `type_of_expression_at_offset` (`type_at_offset.rs:44`) and `identifier_use_site_at_offset` (`ast_offset.rs:19`) provide everything needed. If that infra genuinely can't deliver the context-aware fix, STOP and raise it as a blocker — do not silently ship the fallback.
3. **Phase 7 `DiagnosticKind` variant** — **RESOLVED: YES, add `BannedJargon { term }`.** Typeck-internal enum change, no ABI/user impact. ADDITIONAL requirement from Patrick: the quick-fix must carry the WHY (the lesson), not just the replacement word — source it from the registry `[[banned_jargon]]` `why` field.

---

## Risk Assessment & Rollout Strategy

**Risk level: LOW**

| Criteria | Applies? | Notes |
|---|---|---|
| Touches payments/billing | No | |
| Touches auth/permissions | No | |
| Raw SQL / literals | No | |
| Modifies existing data | No | Compile-time + LSP-time only; no persisted state |
| Third-party integration | No | LSP is in-house |
| Changes existing endpoints/behavior | Yes | Diagnostic emission (fewer false-positives), inlay-hint frequency (Bug 2.9 fires more), hover content, completion trigger. All corrections of wrong behavior. |
| New feature, no equivalent | No | Pure bug fixes |

**Mitigations applied:**
- Comprehensive regression test per bug (test-first per phase) → catches regressions before merge → LOW.
- Backward compatible: no source-language change; existing v0.1/v0.2 `.ynz` files behave identically except for the corrected (fewer) diagnostics and corrected hints.

**Rollout plan:** N/A in the gradual-ramp sense — this is developer tooling shipped in the VSIX, not a production service with traffic to ramp. Ships with the v0.2.1 release tag (M9). The only "rollout" is users installing the new VSIX. Behavior changes (Bug 2.9 hint frequency) are called out in the v0.2.1 CHANGELOG.

---

## Invariants This Milestone Must Preserve

### Safety
- Unused-import diagnostics still fire on genuinely-unused imports after the fix (no false NEGATIVE introduced) — a regression test imports a symbol and never uses it, asserts the warning STILL appears.
- The six new `referenced_names` inserts (Phase 0) only mark a name referenced when the name actually appears in that AST position — they do not blanket-suppress.
- `let → const` and `array → fixed` inlay hints fire only when the binding is provably never reassigned/mutated/grown after the Phase 1/2/3 walker fixes (no hint on a mutated binding — the core correctness property).
- No change to type-checking verdicts: a program that compiled before compiles after; a program that errored before errors after (except spurious unused-import warnings, which are warnings, not errors).

### Performance
- No new salsa query is introduced; all fixes are inside existing queries (`check_query`, the inlay-hint passes). Salsa per-file memoization unchanged.
- Phase 3 replaces an unconditional `out.insert(name)` per call arg with a `sig_table` HashMap lookup + conditional insert — O(1) per arg, no asymptotic change; lookups hit the already-built `sig_table`.
- Phase 0 adds at most one `HashSet::insert` per shape-decl/const-decl/type-position visited during `check_module` — bounded by AST size, already walked.
- **Auto-promotion analysis**: this milestone IS the correctness layer of two existing auto-promotions (`array<T>→fixed<T>` via `prefer-fixed-when-immutable`; `let→const` via `mutable-when-const-suffices`). No NEW auto-promotion candidate is introduced. The codegen auto-promotion itself is unchanged — only the IDE teaching surface (muted hint + click-to-make-explicit) is being corrected. Phase 5 completes the `array→fixed` click-to-make-explicit surface so it matches `let→const` per `.claude/rules/inference.md` "Two Surfaces" (Replacement category). No lint-rule names change (the M4 lint tier-3 milestone owns `prefer-fixed-when-immutable` / `mutable-when-const-suffices`; M10 only fixes the hint analyses they share).

### Teaching
- Every diagnostic and hover touched keeps the WHAT/WHAT-INSTEAD/WHY three-part format.
- Phase 7 adds a `BannedJargon` quick-fix lightbulb so users clicking it convert `enum`→`options` etc. — turning a passive warning into an actionable teaching moment.
- Phase 8 removes banned jargon (`infers`) from user-facing diagnostic WHY strings (per `.claude/rules/vocabulary.md` — `infer`/`inferred` banned in user-facing text) and fixes the `booleanean` typo.
- No new banned-jargon word is introduced; `tests/jargon_audit.rs` (or equivalent) stays green. Phase 8 adds the `booleanean`/`infers` strings to whatever audit guards them so they can't regress.

### Runtime Dependencies
- None. Every change is compile-time (typeck) or editor-time (LSP). No heap allocator, scheduler, or OS I/O dependency added. The LSP already does filesystem reads for cross-file resolution; M10 adds no new I/O.

### Kernel-Mode Behavior
- N/A — these code paths (typeck diagnostics, LSP hint/hover/completion handlers) do not execute at program runtime and emit no codegen. They always work regardless of `--kernel` mode because they never run in the compiled binary. No `--kernel` compile-error path is added or changed.

### Demo & Error Gallery
- **`examples/pirates-roster/entrypoint.ynz`**: extend with a section that exercises ALL SIX previously-false-positive import patterns in realistic context — an imported `options` accessed via variant (`Timeframe.fiveMinute`), an imported union variant used in `is`-narrowing, an imported contract used via `follows`, an imported parent shape used via `extends`, an imported type used as a shape field annotation and in a module-level `const`, an imported contract used via `dynamic`, and an imported generic shape used in `Container<T>` position. The acceptance bar is **zero spurious unused-import warnings** when this file compiles. Captured as an `insta` stdout/stderr snapshot.
- **Error gallery**: M10 adds NO new compile-error class — it removes false-positive *warnings* and fixes hints. Per the `### Demo & Error Gallery` invariant's deferred-handling clause, there is nothing to add to `examples/primantis-orders/v0_3_m1_errors.ynz` (no new diagnostic class to trigger). Instead, the regression coverage lives as (a) the `pirates-roster` "should NOT warn" snapshot above, and (b) per-bug regression tests in `crates/ynz-typeck/tests/` and `crates/ynz-lsp/tests/`. This is a stated, justified exception — recorded here so reviewers know the gallery was considered, not forgotten.

### Feature Registry Entries
- **No new registry entries.** M10 adds zero keywords, zero banned-jargon words, zero primitive intrinsics, zero type-attached constants, zero deferred features, zero diagnostic templates, zero muted-hint domains.
- The only registry-adjacent change is Phase 7: `lsp_code_action_replacement_for` gains a `"BannedJargon"` arm that READS existing `[[banned_jargon]]` entries (already in `registry/features.toml`) to source the replacement text. No schema change, no new entry — read-only consumption of an existing catalog.
- Phase 8 may edit the *description text* of up to 3 existing `[[muted_hint_domain]]` entries to remove banned `infer`/`inferred` wording — that's a content edit to existing entries, not a new entry. Listed explicitly so the `### Feature Registry Entries` audit sees it was considered.

---

## Phase Execution Protocol

This is a **bug-fix milestone**, so per `~/.claude/rules/verification.md` and the plan skill's bug-fix rule, **every phase is test-first**: write the regression test that reproduces the bug FIRST, run it, confirm it FAILS with the observed wrong behavior (Paper-Trace the residual in the commit/PR notes), THEN apply the fix, THEN confirm the test PASSES. You cannot weaken a test you wrote 30 seconds ago to reproduce a bug.

Per `feedback_all_phases_then_review` memory: the executing agent runs ALL phases without pausing for a per-phase "start next phase?" commit gate, BUT runs the four-reviewer pass (code-reviewer + rules-compliance-reviewer + plan-adherence-verifier + acceptance-verifier) after EACH phase before moving on. Patrick reviews the full milestone diff at the end.

**Each phase's Exit Sequence (run these as actions, not a checklist):**
1. **Persist plan state** — tick the phase's Acceptance Criteria checkboxes for criteria the diff actually met, fill each `Evidence:` sub-bullet with concrete content (test name, file:line, command output), tick Quality Gate items verified, bump `last_updated:`. After the reviewer pass, tick the Phase Review Gates with verdict + ISO timestamp and record the commit SHA.
2. **Run `cargo test --workspace` + `cargo clippy --workspace -- -D warnings` + `cargo fmt --all --check`** — all green before review.
3. **Invoke the four reviewers** (code-reviewer, rules-compliance-reviewer, plan-adherence-verifier, acceptance-verifier) against the phase diff. Each reviewer prompt MUST remind the agent of `~/.claude/rules/comments.md` + Golden Rule 11 WHY-quality + Yinz vocabulary (per `agent-dispatch-rule-reminders` memory).
4. **Handle verdicts** — BLOCK → address Required Fixes, re-invoke (max 3 rounds; non-concession evidence rules apply). PASS → commit the phase on the M10 branch and continue to the next phase.

Milestone ships via `/pr` (project skill) when all phases are done and the final cumulative reviewer sweep passes. v0.2.1 release/tag is M9's job, not M10's.

---

## Phases

### Phase 0: Unused-import false-positive bug class (Bug 1 + 2.1–2.5)
**PR scope**: Six missing `referenced_names.insert` sites + two `check_module` walks (`Item::ShapeDecl`, `Item::ConstDecl`) so imports used only via options-variant access, `is`-narrowing, `extends`/`follows`, shape field types, module-`const`, `dynamic`, or generic position stop being flagged "imported but never used."
**Branch**: `fix/m10-unused-import-false-positives`
**Flag**: N/A
**Est. lines**: ~60 (six 1-line inserts + two walk loops + one regression-test file)
**Ships via**: commit on M10 branch (single logical PR — shared root cause)
**Objective**: A symbol imported and used through ANY of the six AST positions is recorded in `referenced_names`, so `check_query`'s unused-import pass doesn't warn on it.
**Why this phase exists**: This is the user-reported bug (`Timeframe`) plus its five siblings. Ships first — highest user pain, lowest risk.
**Current-state anchors**:
- `crates/ynz-typeck/src/check.rs:3736` — `check_options_value`, no insert (Bug 1).
- `crates/ynz-typeck/src/check.rs:3634` — `check_is_arm_pattern`; `:3684` — `check_is_expr` (Bug 2.1).
- `crates/ynz-typeck/src/check.rs:2664` — `check_follows_contracts` (Bug 2.2).
- `crates/ynz-typeck/src/check.rs:230` — `Item::ShapeDecl(_) => {}`; `:236` — combined `ConstDecl` arm (Bug 2.2/2.3).
- `crates/ynz-typeck/src/check.rs:2456` — `AstType::Dynamic` branch (Bug 2.4).
- `crates/ynz-typeck/src/check.rs:2541` — `AstType::Generic` user-defined branch (Bug 2.5).
- Correct-pattern examples to mirror: `check.rs:2403/2407` (Named insert), `:1485` (free-fn), `:2216` (UFCS).
**Files (expected scope)**: `crates/ynz-typeck/src/check.rs`, new `crates/ynz-typeck/tests/unused_import_false_positives.rs`.
**Deviation rule**: standard — document deviations in the PR notes; unrelated concerns split out.
**Steps**:
1. Write `crates/ynz-typeck/tests/unused_import_false_positives.rs` with six tests, one per pattern (options-variant access, `is`-narrowing, `follows`/`extends`, shape-field-type, module-`const`, `dynamic`, generic). Each imports a symbol, uses it ONLY via the target pattern, asserts NO `UnusedImport` diagnostic for that name. Run — confirm the relevant tests FAIL today.
2. Add a seventh "control" test: import a symbol, never use it, assert the `UnusedImport` warning STILL fires (guards against over-suppression / false-negative).
3. `check_options_value` (3736): `self.referenced_names.insert(type_name.to_string());` before the variant check.
4. `check_is_arm_pattern` (3634) + `check_is_expr` (3684): `self.referenced_names.insert(type_path.name.clone());` after the empty-name guard.
5. `AstType::Dynamic` (2456): insert `contract.clone()` inside the `shape_table.contains` branch. `AstType::Generic` (2541): insert `name.clone()` inside the `generic_shape_table.contains` branch.
6. In `check_module`: replace `Item::ShapeDecl(_) => {}` (230) with an arm that, for the shape decl, inserts the `extends` parent name, each `follows` contract name, and walks each field's type annotation via `self.ast_type_to_type(&field.ty)` (discard the returned `Type` — `shapes.rs` already validated; we call it for the `referenced_names` side effect). Pull `ConstDecl` out of the combined arm (236) into its own arm that walks the const's declared type + initializer expression for referenced names. (Verify whether `check_follows_contracts` at 2664 is the better single chokepoint for the `follows`/`extends` inserts — if it already iterates contract names at the equivalent of the audit's `:2509`, do the inserts there instead and leave `check_module`'s ShapeDecl arm to only the field-type + extends walk. Pick whichever avoids double-walking; document the choice.)
7. **Adversarial test (plan-review)**: import a symbol used via TWO patterns at once (e.g. `extends Parent` AND a field-type annotation of the same imported type), plus a SECOND genuinely-unused import in the same file — assert the dual-used one is not flagged AND the unused one still IS. Pins that double-insert doesn't mask a sibling unused import.
**Acceptance criteria**:
- [x] All six pattern tests pass; the import is not flagged unused.
  - Evidence: `tests/unused_import_false_positives.rs` — `options_variant_access_…`:103, `is_narrowing_…`:125, `follows_contract_…`:155, `shape_field_type_…`:185, `module_const_type_annotation_…`:209, `dynamic_contract_…`:230, `concrete_shape_as_field_type_…`:259 (7 tests covering the 6 patterns). `cargo test -p ynz-typeck --test unused_import_false_positives` → 12 passed; 0 failed. Fixes at check.rs check_options_value, check_is_arm_pattern/check_is_expr, AstType::Dynamic, AstType::Generic, ShapeDecl/ConstDecl arms.
- [x] The control test passes: a genuinely-unused import STILL warns.
  - Evidence: `tests/unused_import_false_positives.rs:290` `genuinely_unused_import_still_warns` — imports `Status`, never uses it, asserts `assert_has_unused_import_warning`. Passes (in the 12/12 run). Proves no over-suppression.
- [x] `Timeframe.fiveMinute`-style options-variant access (the exact user-reported repro) produces no warning.
  - Evidence: `tests/unused_import_false_positives.rs:103–117` `options_variant_access_does_not_warn_unused_import` — exact `export options Timeframe {...}` + `Timeframe.fiveMinute.toString()` repro; asserts no `UnusedImport` for `Timeframe`. Passes. Fix: `referenced_names.insert(type_name)` in `check_options_value`.
- [x] No existing test regresses (`cargo test --workspace` ≥ baseline + new tests).
  - Evidence: `cargo test -p ynz-typeck` → 447 passed, 0 failed (baseline ~435 + 12 new). The 5 `ynz-driver` integration snapshot failures are a worktree absolute-path artifact (proven identical on seed bd07e69 with Phase 0 reverted), NOT a regression. Round-2 added 3 tests covering the verdict-change fix (`module_const_undefined_type_emits_no_new_diagnostic_vs_head`) + the generic mixed-field hole (`generic_shape_concrete_field_import_tracked_no_type_param_diagnostic`).
**Quality gate**:
- [ ] Inserts mirror the existing correct-pattern sites (2403/2407/1485/2216) — same idiom, not a new one.
- [ ] No double-insert path that could mask a real unused import.
- [ ] Arrow-fn / type discipline per coding-style.md (no `as any` equivalent; no `.unwrap()` added on fallible lookups without a guard).
**Verification**: `cargo test -p ynz-typeck unused_import_false_positives` all green; `cargo test --workspace` green.

**Phase Review Gates** (3 review rounds: r1 BLOCK→r2 fix→r3 trivial Big-O fix):
- [x] code-reviewer: PASS 2026-05-30 (r2 confirmed const-arm fix + generic hole closed; r3 confirmed TypeParam tidy)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (r3 — Big-O annotations added; r1/r2 otherwise clean)
- [x] plan-adherence-verifier: PASS 2026-05-30 (r2 — all 7 steps + round-2 fix in scope; r3 doc/tidy non-functional, PASS carried)
- [x] acceptance-verifier: PASS 2026-05-30 (r2 — 4/4 ACs MET + both round-2 findings covered; r3 non-functional, PASS carried)
- [x] Committed: 641896a

**Findings Log**:
- 2026-05-30 — code-reviewer round 1: BLOCK. New `Item::ConstDecl` arm uses diagnostic-emitting `self.infer_expr(&c.value, None)` + `ast_type_to_type(ty)` → `const X: NonexistentType = 5` now emits NotDefined where HEAD emitted 0. Type-checking verdict change; violates the locked Safety invariant ("no change to type-checking verdicts"). Out-of-scope scope creep smuggled via a side effect.
- 2026-05-30 — coordinator round 1: generic-shape field-walk skip (`if s.generics.is_empty()`, ShapeDecl arm) is overbroad. `shape Box<T> { meta: ImportedType }` skips the entire field loop → a concrete imported field type inside a generic shape is still falsely flagged unused (the exact Phase-0 bug class, narrowed). Code-reviewer confirmed walking bare `T` emits a spurious diagnostic (true) but did not test the mixed concrete-field case.
- 2026-05-30 — ROUND 2 consolidated fix dispatched: introduce a non-diagnostic-emitting `referenced_names` collection walk over AstType (skip bare TypeParams) + const initializer expr; use it for (a) const type + initializer [kills the verdict change — restores HEAD's zero-diagnostic behavior for consts], (b) generic-shape fields skipping TypeParams [kills the mixed-case hole]. Add regression tests: (i) module-level const with valid type/initializer emits NO new diagnostic vs baseline AND a const referencing an imported type tracks it; (ii) imported concrete type used only as a field inside a generic shape is not flagged unused; (iii) a const with a genuinely-undefined type still behaves as HEAD did (no NEW diagnostic introduced by this phase).
- 2026-05-30 — round-2 landed: const arm + generic-shape fields now use non-emitting `collect_referenced_names_in_ast_type`/`_in_expr`; `const X: NonexistentType = 5` → 0 diagnostics (verdict change gone); generic mixed-field case tracks the concrete import with no spurious TypeParam diagnostic. 447 ynz-typeck tests green, 12/12 unused-import suite, clippy clean.
- 2026-05-30 — coordinator cleanup: `cargo fmt -p ynz-typeck` (round-2 executor) had reflowed 10 unrelated crate files against the worktree's mismatched rustfmt baseline (90-file mismatch; project CI does not gate on fmt). Reverted all 10 to HEAD + deleted insta `.new` artifacts. Phase 0 diff is now cleanly check.rs + the new test file. check.rs retains some local-rustfmt reflow (kept — legit edit target, no CI fmt gate); reviewers told to judge logic not formatting.
- 2026-05-30 — all 4 reviewers re-dispatched on the cleaned round-2 diff; verdicts pending.
- (PASS verdicts logged above are round-1; the round-2 re-run gates the commit.)

---

### Phase 1: `Stmt::Match.else_arm` blindspot (Bug 2.6)
**PR scope**: All six `Stmt::Match` walkers in `inlay_hint_passes.rs` visit `else_arm` so a binding mutated inside an `else =>` catch-all is correctly tracked.
**Branch**: `fix/m10-else-arm-blindspot`
**Est. lines**: ~18 (one `if let Some(eb) = else_arm` per walker × 6)
**Objective**: No inlay hint (`let→const`, `array→fixed`, type, ownership, copy) fires based on analysis that ignored an `else =>` arm.
**Why this phase exists**: `let count = 0; if (...) { ... else => count = 99 }` currently shows "effectively const — never reassigned" on a binding that IS reassigned — the canonical "hint fires on a mutated binding" bug.
**Current-state anchors**: `crates/ynz-typeck/src/inlay_hint_passes.rs` lines **142, 250, 311, 402, 474, 525** (each `Stmt::Match { ..arms.., .. }` drops `else_arm`); `crates/ynz-ast/src/nodes.rs:246` (`else_arm: Option<Block>`).
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-typeck/tests/inlay_hint_else_arm.rs` (new).
**Steps**:
1. Write a test: `let` binding mutated only inside an `else =>` arm → assert NO `let→const` hint. Confirm it FAILS today.
2. For each of the six `Stmt::Match` arms, bind `else_arm` in the pattern and recurse into it with the SAME walker that arm uses for `arms` (e.g. `collect_maybe_mutated` for 142, `collect_const_hints_block` for 525, etc.).
3. Add a companion test for the `array→fixed` hint with an `else =>`-arm `.add()` call to prove the array walker (474) is also fixed.
**Acceptance criteria**:
- [x] `let` mutated in `else =>` arm → no `let→const` hint.
  - Evidence: `tests/inlay_hint_else_arm.rs:25` `let_mutated_only_in_else_arm_does_not_get_let_to_const_hint` — `count = 99` only in `else =>`; asserts zero LetToConst hints. code-reviewer verified it FAILS on pre-fix HEAD (false hint fired), PASSES post-fix. Fix: `inlay_hint_passes.rs:142` binds `else_arm` + `collect_maybe_mutated(eb, out)`.
- [x] array grown via `.add()` in `else =>` arm → no `array→fixed` hint.
  - Evidence: `tests/inlay_hint_else_arm.rs:86` `array_grown_in_else_arm_does_not_get_array_to_fixed_hint` — `nums.add(4)` only in `else =>`; asserts zero ArrayToFixed hints. Fail-before/pass-after confirmed by code-reviewer. Fix: `inlay_hint_passes.rs:486` (array walker) + the :142 mutation collector.
- [x] All six walkers visit `else_arm` (grep confirms no `Stmt::Match` arm in the file still drops it).
  - Evidence: `grep -n 'Stmt::Match' inlay_hint_passes.rs` → 6 hits (142, 253, 317, 411, 486, 540), each binding `else_arm` and recursing with its OWN walker (verified by code-reviewer + plan-adherence: no cross-wiring). Plus 2 over-suppression guard tests (not-mutated/not-grown still get hints).
**Quality gate**:
- [x] Each walker uses its own correct recursion fn (not a copy-paste of the wrong one). — verified per-arm by code-reviewer + plan-adherence.
- [x] No change to the `arms` iteration behavior. — diff only adds `else_arm` handling; `arms` loops untouched.
**Verification**: `cargo test -p ynz-typeck inlay_hint_else_arm` green (4/4); `cargo test -p ynz-typeck` 451 green; clippy clean; grep confirms every arm binds `else_arm`.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-30 (reverted fix to confirm fail-before/pass-after; six walkers correctly wired)
- [x] rules-compliance-reviewer: PASS 2026-05-30
- [x] plan-adherence-verifier: PASS 2026-05-30 (6/6 walkers + both tests + guards, zero creep)
- [x] acceptance-verifier: PASS 2026-05-30 (3/3 ACs MET, anti-tautology confirmed)
- [x] Committed: 0d5c985

**Findings Log**:
_(empty until a reviewer returns BLOCK)_

---

### Phase 2: Nested `FieldAssign`/`IndexAssign` root-binding tracking (Bug 2.7)
**PR scope**: `player.address.street = "x"` and `arr[i][j] = v` mark their root binding (`player`, `arr`) as mutated.
**Branch**: `fix/m10-nested-assign-root-binding`
**Est. lines**: ~20 (a `root_ident` walker + two call sites)
**Objective**: The mutation collector follows chained `FieldAccess`/`IndexAccess` to the root identifier instead of stopping at one level.
**Why this phase exists**: A binding mutated through a nested field/index path currently gets a wrong `let→const` hint because only single-level `Expr::Ident` receivers are recorded.
**Current-state anchors**: `crates/ynz-typeck/src/inlay_hint_passes.rs:119–134` (`Stmt::FieldAssign` 121–123, `Stmt::IndexAssign` 129 — one-level only).
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-typeck/tests/inlay_hint_nested_assign.rs` (new).
**Steps**:
1. Write a test: `let player = makePlayer(); player.address.street = "x"` → assert NO `let→const` hint on `player`. Confirm FAIL today.
2. Add a free `root_ident(e: &Expr) -> Option<&str>` helper that loops through `Expr::FieldAccess { receiver, .. } | Expr::IndexAccess { receiver, .. }` until it hits `Expr::Ident` (or returns `None`).
3. Use it for `FieldAssign.target` and `IndexAssign.receiver`; insert the returned root name.
**Acceptance criteria**:
- [x] `player.address.street = "x"` marks `player` mutated → no hint.
  - Evidence: `tests/inlay_hint_nested_assign.rs:27` `nested_field_assign_marks_root_binding_mutated_no_let_to_const_hint` — FAILED before fix (1 spurious hint at position 38 on `player`), PASSES after fix. Fix: `root_ident` in `inlay_hint_passes.rs:139` follows the two-level FieldAccess chain to `player`.
- [x] `arr[i][j] = v` marks `arr` mutated → no hint.
  - Evidence: `tests/inlay_hint_nested_assign.rs:57` `nested_index_assign_marks_root_binding_mutated_no_let_to_const_hint` — FAILED before fix (3 hints: arr + i + j; expected 2), PASSES after fix (2 hints: i + j only). Fix: `root_ident` in `inlay_hint_passes.rs:146` follows the nested IndexAccess chain to `arr`.
- [x] Single-level case (`player.health = 5`) still works.
  - Evidence: `tests/inlay_hint_nested_assign.rs:95` `single_level_field_assign_still_marks_binding_mutated` — PASSED before and after fix (regression guard). No over-suppression from refactor.
- [x] (round-2, code-reviewer finding) Chained method-call receiver marks root binding mutated.
  - Evidence: `tests/inlay_hint_nested_assign.rs:131` `nested_method_call_receiver_marks_root_binding_mutated` — `player.address.heal(5)` (heal = `lend self`); asserts no `let→const` hint at the `let player` position. acceptance-verifier confirmed pre-fix would emit a hint at byte 268 (not a tautology); position filter is tight (hint position = `let` span.start). Fix: MethodCall arm in `collect_maybe_mutated_expr` (`inlay_hint_passes.rs:196`) now uses `root_ident` — all 3 mutation sites uniform.
**Quality gate**:
- [x] `root_ident` handles `None` (non-ident root) without panicking. — Returns `None` for any non-Ident/FieldAccess/IndexAccess root; callers use `if let Some(name)`, no `.unwrap()`.
- [x] No `.unwrap()` on the `Option`. — Both call sites use `if let Some(name) = root_ident(...)`.
**Verification**: `cargo test -p ynz-typeck --test inlay_hint_nested_assign` green (4/4); `cargo test -p ynz-typeck` 455 green, 0 failures; clippy clean.

**Phase Review Gates** (r1 BLOCK on MethodCall sibling-bug → r2 one-line fix):
- [x] code-reviewer: PASS 2026-05-30 (r2 — reverted fix to confirm new test has teeth at byte 268; all 3 mutation sites uniform via root_ident)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (r1 — root_ident has Big-O; no violations. r2 one-line helper swap adds no rule surface — PASS carried)
- [x] plan-adherence-verifier: PASS 2026-05-30 (r1 — 3 steps + 2 call sites + helper, no creep. r2 closes a same-class sibling site, no scope change — PASS carried)
- [x] acceptance-verifier: PASS 2026-05-30 (r2 — 4/4 ACs MET, position filter proven non-tautological, 455 tests 0 failures)
- [x] Committed: c9da88f

**Findings Log**:
- 2026-05-30 — code-reviewer round 1: BLOCK. The `Expr::MethodCall` arm in `collect_maybe_mutated_expr` (inlay_hint_passes.rs ~193) uses the SAME single-level `if let Expr::Ident(name,_) = receiver.as_ref()` pattern Phase 2 exists to kill — so a mutating method call through a chained receiver (`player.address.heal(5)`) never marks the root `player` mutated → false `let→const` hint on a third sibling path. Reviewer proved it with a probe. Same bug class as Bug 2.7; `root_ident` already exists. (rules/adherence/acceptance all PASS round 1.)
- 2026-05-30 — round-2 fix dispatched: swap the MethodCall receiver check to `root_ident(receiver.as_ref())` + add a fail-before/pass-after regression test `nested_method_call_receiver_marks_root_binding_mutated`. Fixing now (not deferring) per no-duct-tape — same bug class, one-line fix with the helper present.
- 2026-05-30 — round-2 landed + code-reviewer PASS: MethodCall arm now uses `root_ident` (all 3 mutation sites uniform). Reviewer reverted the fix and confirmed the new test catches the `player` hint at byte 268 — assertion has teeth, not a tautology. CORRECTION: real ynz-typeck count is **455** (454 + 1 new test), 0 failures — the executor's "471" was a miscount artifact.
- 2026-05-30 — TRACKED NON-BLOCKING NIT (code-reviewer concern): `nested_method_call_receiver_marks_root_binding_mutated` hardcodes byte offset 268 for `let player` (documented with a provenance comment). Brittle — a future edit to the fixture string would shift the offset and the position filter could false-pass. TRIGGER: anyone editing the nested_assign fixture strings should replace the byte-268 filter with a position-by-name lookup (find the hint whose source slice starts with `let player`). Not blocking: the test is correct today, has teeth, and is guarded by a provenance comment.

---

### Phase 3: `collect_maybe_mutated` over-suppression + literal coverage (Bug 2.9 + Bug 2.10)
**PR scope**: Mark a call argument as mutated ONLY when the callee's parameter is `lend`/`give` (not `share`); and recurse into `StructLit`/`ArrayLit`/`MapLit`/`PostfixOp` so mutations inside literals are seen. Recovers the `let→const` hint on the dominant real-code case (`let x = 5; print(x)`).
**Branch**: `fix/m10-maybe-mutated-ownership-aware`
**Est. lines**: ~70 (thread `sig_table` through 3 fns + ownership check + 4 new match arms + tests)
**Objective**: `print(count)` no longer suppresses the `let→const` hint; `foo(lend x)` still does; mutations inside literal arguments are tracked.
**Why this phase exists**: This is the highest-impact teaching fix — almost every `let` is passed to *some* call, so the over-suppression means the headline auto-promotion hint almost never fires. Bug 2.10 (literal coverage) folds in because it's the same function (`collect_maybe_mutated_expr`).
**Current-state anchors**: `crates/ynz-typeck/src/inlay_hint_passes.rs:160–198` (`collect_maybe_mutated_expr`; `Expr::Call` arm 164–172 unconditional insert; fn has NO `sig_table` param; `_ => {}` leaf at 196 swallows `StructLit`/`ArrayLit`/`MapLit`/`PostfixOp`; `MethodCall` already at 173). Ownership-modifier lookup precedent: `collect_ownership_hints_expr` (330–364).
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-typeck/tests/inlay_hint_maybe_mutated.rs` (new).
**Steps**:
1. Write tests: (a) `let count = 5; print(count)` → `let→const` hint FIRES; (b) `let x = mk(); consume(x)` where `consume(lend T)` → hint SUPPRESSED; (c) `let nums = [a, b.mutate()]` → `b`'s mutation tracked. Confirm (a) and (c) FAIL today, (b) passes for the wrong reason (everything suppressed).
2. Thread `sig_table: &SigTable` (and `imported`, `generic_fn_table` as needed) through `collect_maybe_mutated`, `collect_maybe_mutated_stmt`, `collect_maybe_mutated_expr`.
3. In the `Expr::Call` arm: resolve the callee sig (`sig_table.fns.get(name).or_else(|| imported.get(name))`, with `generic_fn_table` fallback); for each arg ident, insert ONLY when the matched parameter's ownership modifier is `lend` or `give`. `share` (and the `const`-binding implicit default) → do not insert. If the callee can't be resolved, fall back to the conservative "mark mutated" behavior (so an unknown callee never produces a wrong "this is const" hint) — document this fallback.
4. Add UFCS handling: for `Expr::MethodCall`, resolve the method the same way typeck does (the UFCS path near `check.rs:2216`) and apply the same per-parameter ownership check to the receiver + args.
5. Add explicit `Expr::StructLit`/`ArrayLit`/`MapLit`/`PostfixOp` arms that recurse into their sub-expressions (Bug 2.10).
6. **Adversarial test (plan-review)**: a `let` binding passed to a `share` param AND separately reassigned (`let x = 5; print(x); x = 9`) → assert NO `let→const` hint, because the genuine reassignment must still win. Proves the ownership-aware fix didn't accidentally drop real mutation tracking.

**Behavior-change note for CHANGELOG**: when a callee can't be resolved (e.g. an imported function whose signature isn't in scope), Phase 3 conservatively marks the arg mutated — so imported-function args get no `let→const` hint. Named tradeoff (never a wrong "const" hint over a possibly-missing one); call it out in the v0.2.1 CHANGELOG alongside the Bug 2.9 "hints now fire more often" note so neither is mistaken for a regression.
**Acceptance criteria**:
- [x] `print(count)` → `let→const` hint fires.
  - Evidence: `tests/inlay_hint_maybe_mutated.rs:336` `print_call_does_not_suppress_let_to_const_hint` — LITERAL `let count = 5; print(count)`, asserts hint FIRES. (round 1 substituted a user-fn for print and MASKED this; round 2 added `builtin_free_fn_is_readonly` + the real test.) code-reviewer mutation-verified: forcing the helper to `false` fails exactly this test. Fix: `inlay_hint_passes.rs` builtin/intrinsic ownership resolution (print/range/sleepMs/sensitive → share; range/sleepMs from `free_fn_names()` SSOT).
- [x] `consume(x)` with `consume(lend T)` → hint suppressed.
  - Evidence: `tests/inlay_hint_maybe_mutated.rs` `lend_param_call_suppresses_let_to_const_hint` + `give_param_call_…`; `mixed_params_only_share_gets_hint` (assert count==1) is the teeth — proves share-fires-while-lend-suppresses per binding.
- [x] Mutation inside an array/struct literal arg is tracked.
  - Evidence: `mutation_inside_array_literal_arg_is_tracked`, `mutation_nested_in_struct_literal_is_tracked`, `mutation_call_inside_struct_literal_field_is_tracked` (count==2, sharp). Fix: new `Expr::StructLit/ArrayLit/MapLit/PostfixOp` arms recurse into sub-expressions (Bug 2.10).
- [x] Unresolvable callee → conservative suppress (no wrong const hint).
  - Evidence: `unresolvable_callee_conservatively_suppresses_hint` (`unknownFn(x)`); `None => true` fallback in both Call + MethodCall arms, documented with the no-duct-tape tradeoff/cost/trigger docstring.
- [x] (round-2 flagship) Intrinsic method receiver keeps its hint.
  - Evidence: `intrinsic_method_call_does_not_suppress_receiver_hint` — `let score = 5; const s = score.toString()` → `score` keeps hint (assert count==1, teeth via `const s`). Fix: `primitive_intrinsic_method_is_readonly` consults `intrinsics.rs::all_scalar_intrinsic_method_names()`.
**Quality gate**:
- [x] `sig_table` threaded to all three fns (no call path left unchecked). — plan-adherence verified every recursive call site + both entry points.
- [x] `share` parameters never mark args mutated (definitional). — `mixed_params_only_share_gets_hint` proves it.
- [x] No panic on generic/UFCS callee lookup miss. — code-reviewer: no `.unwrap()`/`.expect()` in the file; `None`→conservative branch.
**Verification**: `cargo test -p ynz-typeck --test inlay_hint_maybe_mutated` 13/13 green; `cargo test -p ynz-typeck` 468 green, 0 failures; clippy clean.

**Phase Review Gates** (r1: code-reviewer BLOCK [flagship print broken] + rules BLOCK [6 comments] → r2 fixed both):
- [x] code-reviewer: PASS 2026-05-30 (r2 — mutation-verified the 2 flagship tests have teeth; print(count) + score.toString() now fire)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (r2 — fallback docstring has WHAT/WHY/COST/TRIGGER; zero changelog framing; new helpers carry Big-O)
- [x] plan-adherence-verifier: PASS 2026-05-30 (r2 — 6/6 steps; intrinsics.rs 3rd-file addition documented as correct SSOT-consult, not creep)
- [x] acceptance-verifier: PASS 2026-05-30 (r2 — all 5 ACs MET; flagship now a real print(count) test failing on r1 code)
- [x] Committed: 572a6d8

**Findings Log (continued — round-2 closure):**
- 2026-05-30 — round-2 PASS all 4: flagship genuinely fixed (`print(count)`/`score.toString()` fire via builtin+intrinsic ownership resolution, SSOT-sourced); 6 comments rewritten durable; intrinsics.rs gained `all_scalar_intrinsic_method_names()` (minimal SSOT helper). 13 tests (was 11, +2 flagship), 468 ynz-typeck green.
- 2026-05-30 — TRACKED NON-BLOCKING NIT (code-reviewer): `builtin_free_fn_is_readonly` hardcodes `matches!(name, "print" | "sensitive")` (the check.rs-special-cased free-fns NOT in the registry `free_fn_names()` SSOT; range/sleepMs come from SSOT). Verified complete today {print,range,sleepMs,sensitive}. TRIGGER: if a future builtin free-fn is added to check.rs dispatch but NOT the registry, this helper silently misses it (suppresses a legit hint) — add it here OR (better) migrate builtin ownership into the registry. Documented with a narrowing-trigger comment in-code.

**Findings Log**:
- 2026-05-30 — round 1: rules-compliance PASS-able-but-BLOCK on comment quality; plan-adherence PASS (6/6 steps); acceptance-verifier PASS (4/4 ACs by the tests as written). code-reviewer BLOCK + rules-compliance BLOCK.
- 2026-05-30 — **code-reviewer BLOCK (the important one)**: flagship AC#1 (`print(count)` → hint fires) does NOT actually hold. `print` / primitive intrinsics (`.toString()` etc.) aren't in sig_table/imported/generic_fn_table → conservative fallback marks the arg mutated → hint suppressed. Proven: `let count = 5; print(count)` → 0 hints. The executor's test substituted a user-defined `share` fn for `print`, so the green suite MASKED the headline objective being broken. Per no-duct-tape, "intrinsics fall to conservative-suppress, same as pre-fix" is wrong framing — pre-fix everything was suppressed (invisible); post-fix print/.toString are the conspicuous remaining failures of the feature Bug 2.9 exists to fix. (See `intrinsic-fns-not-in-sigtable` memory.)
- 2026-05-30 — rules-compliance BLOCK: 6 comment-quality issues — (1) conservative-fallback docstring states symptom not the full tradeoff/cost/trigger (no-duct-tape documented-deferral shape); (2-6) test WHY comments use changelog framing ("Before the fix…", "Without the StructLit recursion arm…") + a plan reference ("adversarial case from Step 6"). comments.md Hard Rule 2.
- 2026-05-30 — ROUND-2 fix dispatched: (a) make the walker intrinsic/builtin-aware — print/range/sleepMs → share args; primitive intrinsics → share self — via a shared "resolve callee ownership incl. builtins" path consulting the existing intrinsic table (NOT a hardcoded duplicate list); only genuinely-unknown USER callees hit the conservative fallback (keep the `unknownFn(x)` suppress test — that's the legit fallback). Add fail-before/pass-after tests for the LITERAL `print(count)` repro + `let s = score.toString()` keeping `score`'s hint. (b) rewrite the 6 comments to durable WHY (current contract, no "before the fix"/plan refs); fill the fallback docstring's tradeoff/cost/trigger.

---

### Phase 4: Inlay-hint positioning fix + Yinz-yellow color contribution
**PR scope**: Replacement-category promotion hints (`array→fixed`, `let→const`) render at end-of-statement (before any trailing user comment) instead of at the `let` keyword; the VSCode extension sets the `[ynz]`-scoped inlay-hint color to Pittsburgh gold `#ffd23f`.
**Branch**: `fix/m10-inlay-position-and-color`
**Est. lines**: ~40 (helper + two position swaps + one package.json block + tests)
**Objective**: The promotion decorations sit at the natural read position (end of line) and are visually distinct in `.ynz` files.
**Why this phase exists**: `position: span.start` puts the hint on the `let` keyword, mid-statement — wrong place for a Replacement-category annotation per `.claude/rules/inference.md`.
**Current-state anchors**: `crates/ynz-typeck/src/inlay_hint_passes.rs:464` (`array_to_fixed` position) and `:515` (`let_to_const` position), both `position: span.start`. `PromotionHint` struct at 73–80. `tooling/vscode-ynz/package.json` (no `contributes.configurationDefaults` for inlay color yet).
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `tooling/vscode-ynz/package.json`, `crates/ynz-typeck/tests/inlay_hint_position.rs` (new).
**Steps**:
1. Write a test asserting the promotion hint's byte position equals end-of-statement (or the `//` position when a trailing comment exists), not `span.start`. Confirm FAIL today.
2. Add helper `end_of_let_statement_or_before_comment(text: &str, stmt_span: SourceSpan) -> usize` in `inlay_hint_passes.rs` — scans from `stmt_span.end` (or the line) for a `//` not inside a string literal, returns the `//` byte offset or end-of-line. The pass has `sf.text(db)` available.
3. Set `PromotionHint.position` (464 and 515) to the helper's result.
4. In `tooling/vscode-ynz/package.json`, add `contributes.configurationDefaults`: `"[ynz]": { "editor.inlayHints.foreground": "#ffd23f" }`. (Hex locked by roadmap Q11 — Pittsburgh gold, `--color-gold` from `website/app/assets/css/tailwind.css:22`.)
5. **Adversarial test (plan-review)**: a statement with `//` INSIDE a string literal before any real trailing comment, e.g. `let url: array<int> = parse("http://x")  // real comment` → assert the hint positions before the REAL trailing comment, not the `//` inside `"http://x"`. Directly exercises the string-literal-`//` quality gate.
**Acceptance criteria**:
- [x] Promotion hint positions at end-of-statement, not `span.start`.
  - Evidence: `tests/inlay_hint_position.rs:36` `let_to_const_hint_position_is_end_of_statement_not_span_start` + `:155` `array_to_fixed_…`. acceptance-verifier confirmed anti-tautology by arithmetic: pre-fix `span.start`=byte 37 fails `>= stmt_end`(50); new code passes. code-reviewer reverted to span.start → 3/4 fail. Fix: new `hint_position_end_of_stmt_or_before_comment` helper, both promotion passes updated.
- [x] Trailing-comment case positions before the `//`.
  - Evidence: `tests/inlay_hint_position.rs:77` `let_to_const_hint_positions_before_trailing_comment` asserts `position <= comment_pos`. (Note: `array→fixed` shares the same helper but has no dedicated trailing-comment test — plan-adherence flagged; behavioral correctness shared.)
- [x] `package.json` declares the `[ynz]`-scoped inlay color `#ffd23f`; extension still loads (`package.json` valid JSON).
  - Evidence: `tooling/vscode-ynz/package.json` `contributes.configurationDefaults["[ynz]"]["editor.inlayHints.foreground"] = "#ffd23f"`; `node -e "JSON.parse(...)"` → valid JSON (verified by acceptance-verifier). Language-scoped, not global.
- [x] (plan-review adversarial) string-literal `//` not treated as a comment.
  - Evidence: `tests/inlay_hint_position.rs:101` `let_to_const_hint_does_not_treat_url_slash_slash_as_comment` — `` let msg = `http://x`  // real comment ``; dual assert `position >= in_string_slash+2` AND `<= real_comment_pos`. Anti-tautology proven arithmetically (pre-fix byte 37 fails both). State machine tracks backtick + dquote + escapes; code-reviewer stress-tested escaped-quote/multibyte-UTF-8/no-comment edges.
**Quality gate**:
- [x] Helper does not treat `//` inside a string literal as a comment. — string-state machine (backtick/dquote + escape), adversarial test + code-reviewer harness confirm.
- [x] Color contribution is language-scoped (`[ynz]`), not global. — verified by acceptance-verifier + plan-adherence.
**Verification**: `cargo test -p ynz-typeck --test inlay_hint_position` 4/4 green; `cargo test -p ynz-typeck` 472 green, 0 failures; clippy clean (ynz-typeck); `node -e "JSON.parse(...)"` valid.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-30 (state machine stress-tested all edges; fail-before verified by revert; UTF-8-safe, no unwraps)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (helper has Big-O; no banned content; color needs no comment)
- [x] plan-adherence-verifier: PASS 2026-05-30 (all 5 steps; helper name/sig differ but functionally equivalent + pre-disclosed; #ffd23f exact; zero creep)
- [x] acceptance-verifier: PASS 2026-05-30 (4/4 ACs, anti-tautology proven arithmetically)
- [x] Committed: e094df8

> **CROSS-PHASE NOTE (ynz-lsp clippy)**: `ynz-lsp` carries 5 pre-existing clippy `-D warnings` errors on the seed/main commit (deprecated `root_uri`, `let...else`→`?`, two too-many-arguments, `sort_by_key`) — a v0.3.0-m1 condition, NOT introduced by M10 and out of M10's bug scope. Phases 5/6/7 touch ynz-lsp; their clippy bar is "**no NEW warnings beyond the pre-existing 5**", not "clippy -p ynz-lsp clean". Track for a separate v0.2.1 ynz-lsp cleanup (or fold into M9 polish). Listed in the end-of-plan report as a known pre-existing.

**Findings Log**:
- 2026-05-30 — all 4 reviewers PASS round 1 (no fix loop). Two NON-BLOCKING concerns addressed in-place before commit (comment-only, no re-review needed): (a) fixed the adversarial test's WHY comment which narrated a hypothetical `array<int>=[1,2,3]` source that didn't match the actual `` `http://x` `` backtick-string test; (b) added a single-line-constraint doc comment to the helper.
- 2026-05-30 — TRACKED NON-BLOCKING NIT (code-reviewer): multi-line `let` initializers (e.g. a multi-line array literal) get the Replacement decoration anchored at end of the FIRST line, not the true end-of-statement, because the helper scans only `stmt_start`'s line. Narrow (multi-line promotable bindings are rare), no crash, single-line is correct. Documented in-code as a single-line assumption with rationale (span.end points past trivia, so reliable across-line statement-end detection is non-trivial). TRIGGER: sort the span.end-trivia handling, or a user reports the multi-line mis-anchor.

---

### Phase 5: `array_to_fixed_promotion` click-to-make-explicit (Bug 2.13)
**PR scope**: The `array→fixed` inlay hint attaches a `TextEdit` that swaps the `array` keyword for `fixed` in the type annotation, matching the `let→const` hint's click-to-make-explicit behavior.
**Branch**: `fix/m10-array-to-fixed-edit`
**Est. lines**: ~50 (new `array_to_fixed_edit` helper + `PromotionHint` field + `make_hint`→`make_hint_with_edit` switch + test)
**Objective**: Clicking the `array→fixed` decoration rewrites the source, per `.claude/rules/inference.md` "Two Surfaces for the Same Decision" (Replacement category — both auto-promotions get click-to-make-explicit).
**Why this phase exists**: `let→const` is clickable; `array→fixed` is a dead decoration. Inconsistent teaching surface.
**Current-state anchors**: `crates/ynz-lsp/src/inlay_hint.rs:221–226` (`array_to_fixed` uses plain `make_hint`, no edit); `:120` (`let_to_const_edit` helper to mirror); `:232+` (`let_to_const` uses `make_hint_with_edit`). `PromotionHint` at `inlay_hint_passes.rs:73–80` (carries no type-annotation span yet).
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs` (extend `PromotionHint`), `crates/ynz-lsp/src/inlay_hint.rs` (new helper + switch), `crates/ynz-lsp/tests/inlay_hint_array_to_fixed_edit.rs` (new).
**Steps**:
1. Write a test: `let nums: array<int> = [1,2,3]` (never grown) → the `array→fixed` hint carries a `TextEdit` that replaces `array` with `fixed` in the annotation, and the post-edit source `let nums: fixed<int> = [1,2,3]` type-checks. Confirm FAIL today (no edit attached).
2. Extend `PromotionHint` with the byte range of the `array` keyword in the type annotation (e.g. `type_keyword_span: Option<SourceSpan>`), populated by `array_to_fixed_promotion_hints`.
3. Add `array_to_fixed_edit(...)` in `inlay_hint.rs` (mirror `let_to_const_edit` at 120) producing a `TextEdit` over the `array` keyword range → `"fixed"`.
4. Switch the `array_to_fixed` push (221) from `make_hint` to `make_hint_with_edit` with that edit.
**Acceptance criteria**:
- [x] `array→fixed` hint carries a `TextEdit`.
  - Evidence: `tests/inlay_hint_array_to_fixed_edit.rs:38` `array_to_fixed_hint_carries_text_edit`. Anti-tautology: pre-fix used `make_hint` (→`text_edits: None`); switch to `make_hint_with_edit` + `PromotionHint.type_keyword_span` (inlay_hint_passes.rs:145) + `array_to_fixed_edit` helper (inlay_hint.rs:143).
- [x] Applying the edit yields valid, type-checking source (`fixed<int>` size inferred from literal).
  - Evidence: `tests/inlay_hint_array_to_fixed_edit.rs:80` `array_to_fixed_edit_replaces_array_keyword_with_fixed` — applies the edit, asserts `fixed<int>` present / `array<int>` absent / edit width == 5 (`"array".len()`). `name_span` covers only the keyword (verified by code-reviewer against `nodes.rs:712`). `fixed<int>` w/o explicit size is valid Yinz (size inferred).
- [x] `let→const` edit unchanged (no regression).
  - Evidence: `tests/inlay_hint_array_to_fixed_edit.rs:141` `let_to_const_edit_unchanged_no_regression` — asserts the const hint still carries an edit with `new_text == "const"`. let→const code path untouched in the diff.
**Quality gate**:
- [x] Edit range covers exactly the `array` keyword, not the `<int>` args. — edit-width==5 assertion + `Type::Generic.name_span` semantics (code-reviewer verified).
- [x] No edit emitted when the annotation can't be located (graceful, no panic). — `type_keyword_span: Option`, `.map()` no-op path; no `.unwrap()`. (Note: None is currently unreachable for array→fixed — annotated arrays always carry name_span — so the Option is documented as defensive for future inferred-array promotion.)
**Verification**: `cargo test -p ynz-lsp --test inlay_hint_array_to_fixed_edit` 4/4 green; `cargo test -p ynz-lsp` 204 green, 0 failures; `cargo test -p ynz-typeck` green; clippy: ynz-typeck clean, ynz-lsp no-new (5 pre-existing).

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-30 (verified the test-flip is legitimate — bare `int` param non-mutating; edit token-swaps exactly)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (test-ratchet + WHY on the flipped assertion satisfy immutable-test discipline; Big-O present)
- [x] plan-adherence-verifier: PASS 2026-05-30 (4 steps; the ynz-lsp stale-test fix is in the plan's `files:` scope, documented via test-ratchet — not creep)
- [x] acceptance-verifier: PASS 2026-05-30 (3/3 ACs; renamed/flipped test passes; 204 ynz-lsp 0 failures)
- [x] Committed: 586db65

**Findings Log**:
- 2026-05-30 — PHASE-3 VERIFICATION GAP CAUGHT HERE: an ynz-lsp test (`test_inlay_hint_const_hint_suppressed_when_passed_to_function`) had been FAILING since Phase 3 (572a6d8) because Phase 3's verification ran only `cargo test -p ynz-typeck`, never `-p ynz-lsp`. The test encoded the Bug 2.9 over-suppression as expected behavior; Phase 3 correctly fixed the behavior, breaking the stale test. Fixed here: renamed to `..._fires_when_passed_to_readonly_param`, assertion flipped to corrected behavior, `// test-ratchet:` + rewritten WHY. LESSON: remaining phases + final sweep run `-p ynz-lsp` (and full workspace) too, not just `-p ynz-typeck`.
- 2026-05-30 — all 4 reviewers PASS round 1. Three non-blocking code-reviewer concerns addressed in-place before commit (comment/test-label only, no logic, no re-review): (a) renamed the mislabeled test-theater `array_to_fixed_hint_absent_when_array_has_no_type_annotation` → `every_array_to_fixed_hint_in_file_carries_an_edit` with honest WHY (it never exercised the no-annotation path; the source had two annotated bindings); (b) softened a test WHY that overclaimed "type-checks" (it does textual assertions); (c) tightened the `type_keyword_span` field doc to note None is currently unreachable/defensive.
_(empty until a reviewer returns BLOCK)_

---

### Phase 6: Hover fixes (Bug 2.8 + Bug 2.12)
**PR scope**: (a) User-defined symbols win over registry keyword hover for contextual identifiers (`share`/`lend`/`give`/`errors`/`wait`/`is`/`background`) used as ordinary identifiers; (b) hover works at the end-of-token cursor position.
**Branch**: `fix/m10-hover-shadowing-and-eot`
**Est. lines**: ~40 (lookup reorder with context guard + bound change + tests)
**Objective**: Hovering `share` in `let share = 5; share + 1` shows the variable's type; hovering `share` in `function f(share self: Player)` still shows the keyword hover; hovering at the byte-after-last-char of a token returns content.
**Why this phase exists**: A user-defined variable named like a contextual keyword currently always gets the keyword hover (wrong symbol). And many editors place the cursor at `tok.span.end`, which the strict `<` check excludes.
**Current-state anchors**: `crates/ynz-lsp/src/hover.rs:112–121` (registry tried before user-symbol fallback at 124); `:23` (`byte_offset < tok.span.end` strict). Contextual identifiers confirmed non-token via `parser.rs:1450–1458`.
**Files (expected scope)**: `crates/ynz-lsp/src/hover.rs`, `crates/ynz-lsp/tests/hover_shadowing.rs` (new).
**Steps**:
1. Write tests: (a) `let share = 5; share + 1` hover on second `share` → variable type, not keyword; (b) `function f(share self: Player)` hover on `share` → keyword hover STILL shown; (c) cursor at `tok.span.end` → hover returns content. Confirm (a) and (c) FAIL today.
2. **Context-aware reorder (LOCKED — no fallback).** In `hover_response`, before calling `lsp_hover_for_token`, ask whether the cursor is on a value-position expression via the EXISTING `type_of_expression_at_offset` (`crates/ynz-typeck/src/type_at_offset.rs:44`) — and/or `identifier_use_site_at_offset` (`ast_offset.rs:19`) for definition/use sites. If it resolves to a typed expression / user-defined symbol, show the user-symbol hover (its type). Only when the offset does NOT resolve to a value-position expression (i.e. it's an ownership-modifier token in a function signature) does it fall through to the registry keyword hover. This naturally disambiguates `share + 1` (expression → variable) from `function f(share self)` (modifier → keyword) with no residual hole. Do NOT ship a "same-named binding exists → user wins" heuristic — that relocates the bug. If the offset infra unexpectedly can't deliver this, STOP and raise a blocker.
3. Change the cursor-in-token bound at `:23` from `< tok.span.end` to `<= tok.span.end`, OR add previous-token fallback when offset == end and the next token isn't an identifier. Pick the one that doesn't double-match adjacent tokens; add a test proving no double-match.
4. **Adversarial test (plan-review)**: hover on an inner shadowing `share` where an outer-scope binding of the same name exists — assert the INNERMOST symbol resolves. Guards the scope-lookup precedence the reorder depends on.
**Acceptance criteria** (RESTATED 2026-05-30 — Bug 2.8's literal "keyword hover shadows a same-named variable" symptom is NON-REPRODUCIBLE in current Yinz: no token is both registry-hoverable AND usable as a binding name [`share`/`lend`/`give` = bindable but no keyword hover; `errors`/`wait`/`is`/`background` = hoverable but hard tokens, `let errors = 5` fails to parse]. The original ACs assumed `share` had a keyword hover to shadow; it doesn't. Restated to what Phase 6 genuinely delivers — context-aware reorder is sound + future-proof for any dual-use token):
- [x] An annotated local identifier in expression position hovers to its TYPE (not a keyword hover, not typeless).
  - Evidence: `tests/hover_shadowing.rs:51` `variable_named_share_in_expression_position_shows_type_in_hover` — `let share: int = 5; print(share)` → hover body contains `int` (asserts `mc.value.contains("int")`, fails on round-1 typeless code). Mechanism: `type_of_name_at_offset` (new in `type_at_offset.rs`) resolves the declared type; `binding_hover` renders `` `share`: `int` ``. acceptance-verifier confirmed anti-tautological.
- [x] `errors`/`wait` keyword hover STILL fires in genuine keyword positions after the context-aware reorder (the real preservation invariant — replaces the vacuous original `share`-modifier AC since `share` has no hover).
  - Evidence: `tests/hover_shadowing.rs:138` `errors_keyword_in_return_type_position_still_shows_keyword_hover` + `:178` `wait_keyword_still_shows_keyword_hover_after_fix` — both assert `is_keyword_hover` (body contains `## Keyword`). These have teeth (errors/wait genuinely have registry hovers). Plus `share_in_function_signature_modifier_position_returns_none` honestly captures that a bare modifier with no registry entry returns None.
- [x] End-of-token cursor → hover content returned, no adjacent-token double-match (Bug 2.12 — genuine fail-before/pass-after).
  - Evidence: `tests/hover_shadowing.rs:255` `end_of_token_cursor_returns_hover_content` (cursor at byte 8 = span.end of `function` → resolves; fails on pre-fix strict `<`) + `:279` `end_of_token_cursor_does_not_double_match_adjacent_token` (whitespace gap → None). Fix: `hover.rs` `<`→`<=` bound.
**Quality gate**:
- [x] Keyword hover still works for genuine keyword positions (`wait`, `errors` tested with `## Keyword` body assertions; `function` regression-guarded). — `share` is NOT a registry keyword so it's excluded from this (documented above).
- [x] No panic when no symbol resolves. — `no_panic_when_module_is_none` test; Option threaded, no unguarded `.unwrap()` in production.
**Verification**: `cargo test -p ynz-lsp --test hover_shadowing` 10/10 green; `cargo test -p ynz-lsp` 0 failures; `cargo test -p ynz-typeck` green; clippy clean (ynz-typeck) / no-new (ynz-lsp).

**Phase Review Gates** (r1: code-reviewer feature-dressed-as-bugfix concern + rules BLOCK + acceptance 2-WEAK → r2: type delivery + honest reframe + comment fixes → r3: final comment cleanups):
- [x] code-reviewer: PASS 2026-05-30 (r2 — type now shown, dead fn buried; ruled `type_of_name_at_offset` the correct primitive)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (r3 — Big-O on hover_response; all changelog framing removed; // WHY: headers kept per testing.md)
- [x] plan-adherence-verifier: PASS 2026-05-30 (r2 — LOCKED context-aware approach held; type-delivery divergence documented + correct; type_at_offset.rs justified SSOT)
- [x] acceptance-verifier: PASS 2026-05-30 (r2 — both round-1 WEAK holes closed: type asserted, preservation tested on hoverable keywords)
- [x] Committed: aea528c

**Findings Log**:
- 2026-05-30 — round 1: plan-adherence PASS (LOCKED context-aware approach confirmed — `identifier_use_site_at_offset` is the sole gate, rejected heuristic absent). rules-compliance BLOCK + acceptance-verifier BLOCK.
- 2026-05-30 — **KEY FINDING: Bug 2.8's literal symptom is NON-REPRODUCIBLE in current Yinz.** No token is both (a) registry-hoverable AND (b) usable as a binding name: `share`/`lend`/`give` are `Token::Identifier` (bindable) but have NO `[[keyword]]` registry hover; `errors`/`wait`/`is`/`background` HAVE hovers but are HARD TOKENS — `let errors = 5` → "Expected a variable name after `let`" (verified via `ynz build`). So "keyword hover shadows a same-named variable" can't happen today. The audit's Bug 2.8 assumed `share` had a keyword hover to shadow; it doesn't. acceptance-verifier independently reached the same conclusion (AC#1 proves None→binding-hover, not keyword→user; AC#2 `share`-modifier asserts None→None, vacuous).
- 2026-05-30 — WHAT PHASE 6 ACTUALLY DELIVERS (all genuine, keep): (a) **Bug 2.12 end-of-token cursor `<`→`<=`** — real fail-before/pass-after, MET; (b) binding-hover for local identifiers in expression position (`let share=5; share+1` → was None, now `Binding: share`) — a real UX add; (c) defensive context-aware reorder (user-symbol wins over registry if a dual-use token ever exists — future-proof) + verified `errors`/`wait` keyword hovers still fire in keyword positions. NOT duct tape — it's correcting a planning assumption (audit's false premise) with evidence + keeping the valuable adjacent work.
- 2026-05-30 — rules-compliance BLOCK: (1-2) missing canonical `Time: O() Space: O()` on `user_symbol_hover` + `binding_hover` (verbal complexity present, not canonical format) — LEGIT, fix. (4-6) test comments use "Pre-fix:/Post-fix:" changelog framing — LEGIT, reword to durable invariants. **(#3 REJECTED by coordinator): the reviewer flagged the `// WHY:` test comments as durability violations — that's WRONG; `.claude/rules/testing.md` MANDATES `// WHY:` on every test ("Every test gets a one-line // WHY: comment stating the invariant it protects"). The reviewer conflated mandated test-WHY with banned changelog comments. WHY headers stay; only their version-relative CONTENT gets reworded.** (#7 reviewer's own "not a hard violation".)
- 2026-05-30 — ROUND-2 fix dispatched: (A) coordinator reframes Phase 6 ACs honestly (Bug 2.8 keyword-shadow non-reproducible; ACs restated to what's delivered: binding-hover-for-locals + Bug-2.12 + errors/wait keyword preservation). (B) executor strengthens tests + (C) comment fixes.
- 2026-05-30 — ROUND-2 LANDED (self-tests green) but **NOT YET RE-REVIEWED OR COMMITTED — PAUSED HERE at Patrick's request (context full)**. Executor result: AC#1 now shows the TYPE (`` `share`: `int` ``, test asserts body contains "int"); AC#2 errors/wait keyword-hover tests assert `## Keyword` body (not just is_some); share-modifier test renamed to `..._returns_none` (honest, no false "keyword preserved" claim); Big-O canonical on `user_symbol_hover`/`binding_hover`/`type_of_name_at_offset`; "Pre-fix/Post-fix" framing removed, 11 `// WHY:` headers intact; dead OR-branches in `is_keyword_hover` test helper cleaned. ynz-lsp + ynz-typeck 0 failures; clippy clean (typeck) / no-new (lsp, 5 pre-existing).
- **RESUME CHECKLIST for Phase 6 (do this first when work resumes):**
  1. Phase 6 round-2 diff is UNCOMMITTED in the worktree (`crates/ynz-typeck/src/type_at_offset.rs` + `crates/ynz-lsp/src/hover.rs` + `crates/ynz-lsp/tests/hover_shadowing.rs`). Base = 586db65 (Phase 5 commit).
  2. Re-run all 4 reviewers on the round-2 diff. ROUND-2-SPECIFIC REVIEW ITEMS: (a) executor used a NEW `type_of_name_at_offset` (name-based, in type_at_offset.rs) instead of the plan's named `type_of_expression_at_offset` — judge whether that's an acceptable equivalent or should use the named fn; (b) executor left a DEAD unused public fn `type_of_expression_in_module` in type_at_offset.rs (added then superseded) — should be removed unless a caller exists; (c) `type_at_offset.rs` is a 3rd file beyond Phase 6's expected scope (hover.rs + test) — judge as justified-SSOT (name-based type lookup belongs in the type-resolution module) vs creep.
  3. Coordinator must REFRAME the Phase 6 AC text in the plan (still says "variable type" / "keyword hover preserved") to match reality before ticking — Bug 2.8 keyword-shadow is non-reproducible; ACs are now: (1) annotated local hover shows its type; (2) errors/wait keyword hover still fires in keyword position; (3) Bug 2.12 end-of-token; (4) share-modifier returns None (no spurious binding hover).
  4. TRACKED follow-up nits (do NOT block P6; note in end-of-plan report): (i) Bug 2.12 EOF-flush edge — last token against EOF with no trailing newline (`"myvar"`@5) returns None (narrow, non-silent); (ii) remove dead `type_of_expression_in_module` if still unused.
  5. Then tick gates + AC evidence + commit P6, write SHA, continue to Phase 7.

---

### Phase 7: Space-trigger removal + `BannedJargon` quick-fix
**PR scope**: Stop the completion popup from opening on every space; add a quick-fix lightbulb that converts banned jargon (`enum`→`options`, etc.) to the Yinz term.
**Branch**: `fix/m10-space-trigger-and-jargon-quickfix`
**Est. lines**: ~40 (one-line trigger removal + code-action arm + registry lookup arm + tests)
**Objective**: Typing prose/space doesn't spam completion; a banned-jargon diagnostic offers a one-click fix sourced from the registry's `[[banned_jargon]]` replacement.
**Why this phase exists**: Space in `trigger_characters` opens completion on every word boundary (noise). And banned-jargon diagnostics are passive — no actionable fix.
**Current-state anchors**: `crates/ynz-lsp/src/capabilities.rs:38` (`" "` in `trigger_characters`); `crates/ynz-lsp/src/code_action.rs:57–76` (arms: `BannedKeyword`, `NotDefined`, `UnusedImport`, `_ => None` — no `BannedJargon`); `crates/ynz-registry/src/lib.rs:180` (`lsp_code_action_replacement_for` matches only `"BannedKeyword"` via `SIMPLE_KEYWORD_REPLACEMENTS`).
**Files (expected scope)**: `crates/ynz-lsp/src/capabilities.rs`, `crates/ynz-lsp/src/code_action.rs`, `crates/ynz-registry/src/lib.rs`, possibly `crates/ynz-diagnostics/src/*` (if `DiagnosticKind::BannedJargon { term }` needs adding), `crates/ynz-lsp/tests/code_action_jargon.rs` (new).
**Steps**:
1. **Confirm** what `DiagnosticKind` variant the banned-jargon diagnostic uses and whether it carries the offending term. If no `term`-carrying variant exists, add/extend one (typeck-internal enum — no ABI impact; see Question 3).
2. Write tests: (a) completion trigger list no longer contains `" "`; (b) a `enum` banned-jargon diagnostic yields a code action that replaces `enum`→`options`. Confirm both FAIL today.
3. Remove `" ".to_string()` from `capabilities.rs:38` `trigger_characters` (keep `"."`).
4. Add a `DiagnosticKind::BannedJargon { term }` arm to `code_action_response` (code_action.rs) calling `lsp_code_action_replacement_for("BannedJargon", term)`.
5. Add a `"BannedJargon"` arm to `lsp_code_action_replacement_for` (lib.rs:180) that searches `[[banned_jargon]]` registry entries for `term` and returns its replacement.
6. **The lesson (Patrick, 2026-05-30)**: the code action must carry the WHY, not just swap the word. Set the code action's `title`/`description` to include the registry `[[banned_jargon]]` `why` text (e.g. for `enum`: "use `options` — Yinz uses human-readable words a non-programmer can guess"). A user clicking the lightbulb learns WHY the word is banned, not just that it changed. (Note: `enum` already teaches+fixes via the existing `BannedKeyword` path — `SIMPLE_KEYWORD_REPLACEMENTS` `("enum","options")` at `lib.rs:158`; Phase 7 brings the SAME teach-and-fix parity to the jargon class that currently lacks the quick-fix.)
**Acceptance criteria**:
- [x] `trigger_characters` no longer includes space; `.`-triggered completion still works.
  - Evidence: `capabilities.rs:38` one-line deletion (`vec![".".to_string(), " ".to_string()]` → `vec![".".to_string()]`); test `completion_trigger_characters_does_not_include_space` (`code_action_jargon.rs:36–57`) asserts space ABSENT and `.` PRESENT in the same test; 2/2 green via `cargo test -p ynz-lsp --test code_action_jargon`.
- [x] Banned-jargon diagnostic produces a working replacement code action.
  - Evidence: `lexer.rs` `emit_banned_jargon_identifier` emits `DiagnosticKind::BannedJargon { term }` (variant added `diagnostic.rs:43`); `code_action.rs:63–65` BannedJargon arm → `build_banned_jargon_action`; test `banned_jargon_identifier_produces_replacement_code_action` (`code_action_jargon.rs:60–121`) drives lexer→typeck→code_action end-to-end. code-reviewer confirmed NON-TAUTOLOGICAL: neutralized the arm (`=> None`), test failed with "expected a code action…got none", then restored.
- [x] Replacement text is sourced from the registry `[[banned_jargon]]` entry (no hardcoded duplicate).
  - Evidence: `ynz-registry/src/lib.rs:189` `"BannedJargon" => banned_jargon_lookup(token).map(|e| e.replacement)`; `BANNED_JARGON` baked from `registry/features.toml` via `build.rs` (no LSP-layer constant). Test asserts `edits[0].new_text == ynz_registry::banned_jargon_lookup("infer").unwrap().replacement` — RHS is a live registry read, not a literal, so hardcoded drift fails the test.
- [x] The code action surfaces the WHY (lesson) from the registry `why` field, not just the replacement word.
  - Evidence: `ynz-registry/src/lib.rs:196–205` `lsp_code_action_label_for_jargon` formats title `"Replace \`{term}\` with \`{replacement}\` — {reason}"` from `entry.reason`; test asserts `action.title.contains(expected_reason)` where `expected_reason = banned_jargon_lookup("infer").unwrap().reason` (live read). NOTE: plan text says "registry `why` field"; the actual schema field is `reason` — implementation correctly uses `reason` (all 4 reviewers confirmed; tracked for the end-of-plan report).
**Quality gate**:
- [x] No new registry ENTRY added — only a read of existing `[[banned_jargon]]`. (rules-compliance + acceptance-verifier confirmed `registry/features.toml` not in diff; registry change is two READ functions consuming existing entries.)
- [x] Code-action arm follows the existing `BannedKeyword`/`UnusedImport` arm idiom. (code-reviewer: `build_banned_jargon_action` mirrors `build_banned_keyword_action` byte-for-byte; same range-computation, `TextEdit`/`WorkspaceEdit`/`is_preferred` shape.)
**Verification**: `cargo test -p ynz-lsp code_action_jargon` green; manual: open a file with `enum`, confirm the lightbulb fixes it.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-30 (r1 — neutralized the BannedJargon arm to prove the test bites; replacement+reason both registry-sourced, no panic on fallible lookup. 3 non-blocking concerns: thin registry `reason` text [content follow-up], plan `why`/`reason` wording, cosmetic lexer `other` vs `text`.)
- [x] rules-compliance-reviewer: PASS 2026-05-30 (r1 — zero violations; durable-WHY comments, registry SSOT respected, no banned jargon in user-facing strings, no test weakening.)
- [x] plan-adherence-verifier: PASS 2026-05-30 (r1 — all 6 steps landed; lexer.rs deviation ruled load-bearing + architecturally correct [mirrors emit_banned_declaration_keyword] + documented; `infer` test trigger is the correct path [`enum` routes through BannedKeyword]; zero creep.)
- [x] acceptance-verifier: PASS 2026-05-30 (r1 — 4/4 ACs MET; tests derive expected values from live registry lookups so hardcoded drift would fail.)
- [x] Committed: 5e20fea

**Findings Log**:
_(no BLOCK rounds — all 4 reviewers PASS on round 1. Non-blocking concerns recorded in the code-reviewer gate above; the registry `reason`-text enrichment is a follow-up, not a Phase 7 fix.)_

---

### Phase 8: Typo + jargon fix-ups (`booleanean`, `infers`)
**PR scope**: Fix the `booleanean` typo and replace banned `infers` jargon with "figures out" in user-facing diagnostic strings; add a guard so they can't regress.
**Branch**: `fix/m10-typo-and-jargon-cleanup`
**Est. lines**: ~15 (3 string edits + audit-guard extension)
**Objective**: No user-facing diagnostic contains `booleanean` or `infers`.
**Why this phase exists**: `booleanean` is a visible typo in the `print` diagnostic; `infers`/`inferred` are banned in user-facing text per `.claude/rules/vocabulary.md`.
**Current-state anchors**: `check.rs:1577` (`booleanean`); `check.rs:1997` and `:2010` (`"infers"` — 2 sites, not 3). Banned-jargon enforcement lives in `crates/ynz-diagnostics/src/banned_jargon.rs`.
**Files (expected scope)**: `crates/ynz-typeck/src/check.rs`, `registry/features.toml` (if any `[[muted_hint_domain]]` description uses `infer`/`inferred`), the jargon-audit test.
**Steps**:
1. Write/extend a test that scans the two diagnostic strings (or runs the jargon audit over typeck diagnostics) asserting no `booleanean` and no `infers`. Confirm FAIL today.
2. `check.rs:1577`: `booleanean` → `boolean`.
3. `check.rs:1997`, `:2010`: `"... Yinz infers type parameters ..."` → `"... Yinz figures out type parameters ..."` (keep WHY meaning, drop banned word).
4. Grep `registry/features.toml` `[[muted_hint_domain]]` description fields for `infer`/`inferred`; replace with "figures out" / "the compiler picks" per vocabulary.md. (Audit suggested up to 3 such fields — fix whatever actually exists.)
5. Ensure `banned_jargon.rs` (or the doc-grep audit) covers these strings so they can't regress.
**Acceptance criteria**:
- [x] No `booleanean` anywhere in user-facing diagnostics.
  - Evidence: `check.rs:1636` `booleanean`→`boolean` in the `print` `Diagnostic::error` string; live `grep booleanean crates/ynz-typeck/src/check.rs` empty. Guard test `no_typo_booleanean_or_verb_infers_in_diagnostic_strings` (`jargon_audit.rs`) source-scans all `crates/**` Diagnostic call sites; code-reviewer mutation-verified it FAILS on reintroduction.
- [x] No `infers`/`inferred` in the two `check.rs` strings or `[[muted_hint_domain]]` descriptions.
  - Evidence: `check.rs:2059` + `:2072` `"Yinz infers type parameters…"`→`"Yinz figures out…"` (WHY preserved); all 9 `[[muted_hint_domain]]` descriptions scrubbed (5 had banned wording: variable_type/function_param_type/ownership_call_site/lifetimes/allocators). SAME-CLASS ADDITION (Rule 11): `ynz-registry/src/lib.rs:134` `_=>` hover-WHY fallback `"…inferred…"`→`"…figured this out…"`. acceptance-verifier confirmed the 2 remaining `inferred` in `features.toml:1196,1214` are `[[deferred_tooling_feature]]` `why` fields — internal/non-user-facing, correctly left (no over-ban).
- [x] Jargon audit guards both so a future reintroduction fails CI/tests.
  - Evidence: 4 new tests in `jargon_audit.rs` — diagnostic-string scan, `lsp_inlay_hint_hover_for` source-scan (guards the unreachable `_=>` arm a runtime test can't reach), runtime hover-output sweep over all 9 domains, muted_hint_domain description scan. code-reviewer ran 4 mutation checks: every guard goes RED on reintroduction, GREEN on restore. Source-scan empirically failed pre-fix (`"WHY string in lsp_inlay_hint_hover_for contains banned jargon"`) — non-tautological.
**Quality gate**:
- [x] Replacement preserves the diagnostic's WHAT/WHAT-INSTEAD/WHY meaning. (code-reviewer + plan-adherence: WHY meaning intact on all 5 reworded strings — e.g. typeck "no arguments, specify explicitly" guidance retained.)
- [x] `infer`/`inference` still allowed in design-doc/internal contexts (do not over-ban — vocabulary.md dual-audience rule). (all 4 reviewers confirmed only user-facing strings touched; zero internal identifiers/comments/doc-comments reworded; the audit test's own banned-word literals are correct detection patterns, not violations.)
**Verification**: `cargo test --workspace` green; `grep -rn 'booleanean\|infers' crates/ynz-typeck/src/check.rs` returns nothing.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-31 (mutation-verified all 4 guard tests bite, incl. the unreachable `_=>` arm; dual-audience boundary respected. 1 non-blocking: `jargon_audit.rs:449` "historically contained" comment — ruled load-bearing escape-valve, cross-flagged to rules-compliance.)
- [x] rules-compliance-reviewer: PASS 2026-05-31 (zero violations; dual-audience applied correctly — test code naming banned words is legit detection data; registry edits content-only on existing entries; no ratcheting. Did not flag the :449 comment.)
- [x] plan-adherence-verifier: PASS 2026-05-31 (all 5 steps MET; lib.rs deviation front-matter-blessed [line 18] + Rule-11 same-class; 5-vs-3 toml edits all genuine; no Phase 9/10 creep.)
- [x] acceptance-verifier: PASS 2026-05-31 (3/3 ACs MET; guards scan real disk artifacts, non-tautological with confirmed pre-fix failure; 2 remaining features.toml `inferred` correctly exempt [deferred_tooling_feature why-fields].)
- [x] Committed: 4a016fd

**Findings Log**:
_(no BLOCK rounds — all 4 reviewers PASS on round 1. Scope expanded mid-phase to fold in a Rule-11 same-class fix [`ynz-registry/src/lib.rs:134` user-facing `inferred` in the inlay-hint hover WHY fallback] + its audit guard. Non-blocking concern: `jargon_audit.rs:449` "historically contained `inferred`" comment is borderline changelog phrasing per comments.md — both code-reviewer and rules-compliance cleared it as load-bearing test-rationale; left as-is, trivial follow-up if Patrick wants it tightened.)_

---

### Phase 9: Ownership-hint generic + UFCS coverage (Bug 2.11) — superset of roadmap P0–P8
**PR scope**: `ownership_call_site_hints` resolves generic-function calls (via `generic_fn_table`) and UFCS method-call form (`player.heal(20)`), so ownership muted hints are consistent across both call syntaxes.
**Branch**: `fix/m10-ownership-hint-generic-ufcs`
**Est. lines**: ~45 (generic fallback + `MethodCall` arm + tests)
**Objective**: A user sees the same ownership hint whether they write `heal(player, 20)` or `player.heal(20)`, and for generic functions.
**Why this phase exists**: Bug 2.11 — the muted-hint protocol is supposed to be informative across ALL call sites, but generic + UFCS calls currently get no ownership hint. Included per Rule 11 (confirmed finding gets fixed); see Question 1.
**Current-state anchors**: `crates/ynz-typeck/src/inlay_hint_passes.rs:330–364`; line 339 `sig_table.fns.get(name).or_else(|| imported.get(name))` (no `generic_fn_table`); guard at 336 matches only `Expr::Call`.
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-typeck/tests/inlay_hint_ownership_ufcs.rs` (new).
**Steps**:
1. Write tests: (a) UFCS `player.heal(20)` where `heal(lend Player, int)` → ownership hint on `player`; (b) generic-fn call gets an ownership hint. Confirm FAIL today.
2. Add `generic_fn_table` fallback to the sig lookup at 339.
3. Add an `Expr::MethodCall` branch that resolves the method via the same UFCS lookup typeck uses (near `check.rs:2216`) and emits hints for the receiver + args.
**Acceptance criteria**:
- [x] UFCS call gets the same ownership hint as the equivalent free-fn call.
  - Evidence: `inlay_hint_ownership_ufcs.rs` — `ufcs_method_call_emits_lend_hint_on_receiver` (`player.heal(20)`, sig `heal(lend self, int)` → `lend` hint on receiver) + baseline `free_fn_call_emits_lend_hint` (`heal(player, 20)` → same `lend`). Production: new `Expr::MethodCall` arm maps receiver→param 0 via the SAME `resolve_param_ownerships` helper as `Expr::Call`. acceptance-verifier confirmed `share`/`lend` cross-discrimination (not hardcoded). POSITION PARITY now asserted (added post-review): both tests assert the hint lands at the source-derived end-of-`player` offset (`src.find(...).unwrap()+"player".len()`), so the "same position relative to the argument" claim is tested, not just modifier.
- [x] Generic-fn call gets an ownership hint.
  - Evidence: `generic_fn_call_emits_ownership_hint` (`identity<T>(share x)` → `share`) + `generic_fn_lend_param_emits_lend_hint` (`swap<T>(lend a, lend b)` → `assert_eq!(lend_hints.len(), 2)` — exact count). Production: `resolve_param_ownerships` adds the `generic_fn_table` third tier. code-reviewer mutation-verified: neutralizing the `generic_fn_table` line fails both generic tests.
**Quality gate**:
- [x] UFCS resolution mirrors typeck's lookup (no parallel/divergent logic per no-duct-tape #7). (plan-adherence + code-reviewer: `resolve_param_ownerships` chain `sig_table.fns→imported→generic_fn_table` is byte-for-byte Phase 3's `collect_maybe_mutated_expr` chain [commit 572a6d8]; helper-extraction factors the shared path rather than forking it.)
- [x] No panic on unresolved generic/UFCS callee. (test `unresolved_method_call_yields_no_hint_and_no_panic`; both arms route unresolvable callees to recurse-without-hint; zero `.unwrap()`/`.expect()` in the changed region; intrinsics like `print` not in any table → no hint, not a wrong hint.)
**Verification**: `cargo test -p ynz-typeck inlay_hint_ownership_ufcs` green.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-31 (r1 — mutation-verified both call arms + generic line; lookup mirrors Phase 3; graceful unresolved path; size justified as threading overhead. 2 non-blocking: header overstated position coverage [CLOSED post-review by adding source-derived position-parity asserts]; `Stmt::Return` ownership-hint gap [tracked follow-up].)
- [x] rules-compliance-reviewer: PASS 2026-05-31 (r1 — durable docstring rewrite, no parallel logic, ownership hints informational, Yinz vocab, no test weakening, no graveyard corpses.)
- [x] plan-adherence-verifier: PASS 2026-05-31 (r1 — all 3 steps MET; 2-file scope clean; line overage is pure generic_fn_table threading [minimum diff], not creep; resolve_param_ownerships chain matches Phase 3; no Phase 10 bleed.)
- [x] acceptance-verifier: PASS 2026-05-31 (r1 — both ACs MET; tests drive Expr::MethodCall + generic syntax, assert exact modifier + exact count, paired with free-fn baseline.)
- [x] Committed: e2b3827

**Findings Log**:
_(no BLOCK rounds — all 4 reviewers PASS round 1. Post-review test-strengthening: added source-derived position-parity assertions to close code-reviewer concern #1 [header comment overstated that position was tested]; verified non-tautological + 7/7 green._
_**TRACKED FOLLOW-UP (out of M10 scope — Rule 11 / deferrals-must-be-tracked)**: `collect_ownership_hints_block` has no `Stmt::Return { value: Some(e) }` arm (`_ => {}` swallows it), so `return heal(player, 20)` emits no ownership hint — inconsistent with the sibling `collect_maybe_mutated_stmt` which DOES handle `Stmt::Return`. Pre-existing before Phase 9 (the block walker had only `_ => {}`); NOT one of the 14 cataloged audit bugs; NOT Bug 2.11 (call-form parity). One-line add when picked up. Recorded here + surfaced in the end-of-plan before/after report so it isn't lost. Not fixed in M10.)_

---

### Phase 10: Copy-hint recursion into nested calls (Bug 2.14) — superset of roadmap P0–P8
**PR scope**: `collect_copy_hints_expr` recurses into call arguments so nested calls like `outer(inner(x))` emit a copy hint for `x`.
**Branch**: `fix/m10-copy-hint-recursion`
**Est. lines**: ~25 (recursion + test)
**Objective**: Trivially-copyable values get their copy hint at every call depth, not just the outermost.
**Why this phase exists**: Bug 2.14 — `collect_ownership_hints_expr` recurses into args; `collect_copy_hints_expr` doesn't, so the two teaching surfaces are inconsistent at nested calls. Included per Rule 11; see Question 1.
**Current-state anchors**: `crates/ynz-typeck/src/inlay_hint_passes.rs:416–433` (inspects top-level args, no recursion). Precedent: `collect_ownership_hints_expr` recurses via `collect_ownership_hints_expr(arg, ...)`.
**Files (expected scope)**: `crates/ynz-typeck/src/inlay_hint_passes.rs`, `crates/ynz-typeck/tests/inlay_hint_copy_recursion.rs` (new).
**Steps**:
1. Write a test: `outer(inner(n))` with `n: int` → `n` gets a copy hint. Confirm FAIL today.
2. After inspecting each top-level arg, recurse `collect_copy_hints_expr(arg, ...)` (mirror the ownership-hints recursion).
**Acceptance criteria**:
- [x] Nested-call arg gets a copy hint.
  - Evidence: `inlay_hint_copy_recursion.rs` — `nested_call_arg_gets_copy_hint`: `outer(inner(n))`, `n: int` → asserts a `CopyHint{size_text:"8 bytes"}` at the end-of-`n` byte offset (presence + content). Production: 1 functional line `collect_copy_hints_expr(arg, expr_types, out)` added after the per-arg type-check in the `Expr::Call` loop. code-reviewer mutation-verified: neutralizing the recursion line → test fails with `hints: []` (pre-fix state); restored → passes. Non-tautological.
- [x] No duplicate hint at the outer level.
  - Evidence: `top_level_copyable_arg_gets_exactly_one_copy_hint` (`consume(n)`, `n: int`) asserts `assert_eq!(hints_at_n.len(), 1)` — EXACT count (not `>=1`), the sharp predicate that catches a double-emit. Mechanism (verified by code-reviewer + plan-adherence): recursing into a plain `Expr::Ident` arg hits the `if let Expr::Call` guard which doesn't match → emits nothing; the single hint comes from the top-level type-check only. acceptance-verifier confirmed the `==1` sharpness.
**Quality gate**:
- [x] Recursion mirrors `collect_ownership_hints_expr` (consistent walker shape). (all reviewers: same check-then-recurse-inside-arg-loop shape; copy-hint signature omits sig_table since copy hints don't resolve ownership, but the walker structure is identical — no forked approach per no-duct-tape #7.)
- [x] `Type::Error`/`Type::Nothing` still filtered (no copy hint on non-copyable). (recursion added AFTER the `is_trivially_copyable` guard [Int|Float|Bool|Number only]; non-copyable types filtered at every depth; confirmed by still-green `test_copy_point_does_not_fire_for_string_arg`.)
**Verification**: `cargo test -p ynz-typeck inlay_hint_copy_recursion` green.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-31 (r1 — mutation-verified [neutralized recursion → AC1 dies with `hints:[]`, AC2 stays green proving independence]; 1-line surgical fix mirrors ownership walker; type filter preserved at depth. 2 non-blocking: MethodCall/UFCS copy-hint gap + block-walker stmt gap — both pre-existing, tracked follow-ups, reviewer insisted they be written down not left in chat.)
- [x] rules-compliance-reviewer: PASS 2026-05-31 (r1 — durable comments, mirrors walker [no parallel logic], no test weakening, no violations.)
- [x] plan-adherence-verifier: PASS 2026-05-31 (r1 — both steps MET; 2-file scope clean; recursion follows ownership-walker precedent; is_trivially_copyable filter preserved; out-of-scope gaps correctly absent [not skipped steps].)
- [x] acceptance-verifier: PASS 2026-05-31 (r1 — both ACs MET; AC1 presence+content, AC2 exact `==1` count.)
- [x] Committed: ad3f4d3

**Findings Log**:
_(no BLOCK rounds — all 4 reviewers PASS round 1, no documented deviations.)_
_**TRACKED FOLLOW-UPS (out of M10 scope — Rule 11 / deferrals-must-be-tracked; recorded in `.claude/todos.md` per code-reviewer's insistence they not live only in chat)**: two adjacent inlay-hint-walker completeness gaps surfaced during Phase 10, both PRE-EXISTING and NOT among the 14 cataloged audit bugs:_
_  1. `collect_copy_hints_expr` only handles `Expr::Call`, not `Expr::MethodCall` (UFCS) — `player.greet(n)` gives `n` no copy hint. This is the EXACT class Phase 9 fixed for the OWNERSHIP walker (Bug 2.11); the copy walker now lags — a sibling-walker asymmetry (no-duct-tape #7 smell)._
_  2. `collect_copy_hints_block` walks only `Stmt::Expr`+`Stmt::Let`, skipping `Stmt::Assign`/`FieldAssign`/`IndexAssign`/`Return` value exprs (the ownership block walker covers more). Joins the Phase 9 `Stmt::Return` ownership-walker gap — together these three form a coherent "inlay-hint walker completeness" follow-up workstream for a future v0.2.1 milestone._

---

### Phase 11: Demo extension + cumulative verification sweep
**PR scope**: Extend `examples/pirates-roster/entrypoint.ynz` to exercise all six previously-false-positive import patterns (zero spurious warnings), then run the Step-10 verification sweep over the whole milestone.
**Branch**: `fix/m10-demo-and-verification`
**Est. lines**: ~40 (demo additions + snapshot)
**Objective**: Hands-on demo proof that the unused-import fixes hold in a realistic project, plus the cumulative end-of-plan review.
**Why this phase exists**: The `### Demo & Error Gallery` invariant requires the demo extension; the plan skill requires a final verification sweep.
**Current-state anchors**: `examples/pirates-roster/entrypoint.ynz` (single-entry layout per `examples/README.md`).
**Files (expected scope)**: `examples/pirates-roster/entrypoint.ynz` (+ any imported service/util files needed for the patterns), the project's `insta` snapshot for the demo build.
**Steps**:
1. Add a Pittsburgh-themed section to `pirates-roster` that imports symbols used ONLY via: options-variant access, `is`-narrowing, `follows`, `extends`, shape-field-type annotation, module-`const`, `dynamic`, and generic position.
2. Build the demo (`./target/debug/ynz build examples/pirates-roster/entrypoint.ynz`); assert ZERO unused-import warnings. Capture `insta` stdout/stderr snapshot.
3. Step-10 sweep: TODO grep (no `TODO`/`FIXME`/`Phase N` left in touched code); confirm `.claude/todos.md` LSP bug items reflect what shipped; cumulative `git diff <m10-base>..HEAD` four-reviewer pass; verify Quality Checklist below.
**Acceptance criteria**:
- [x] `pirates-roster` exercises all six patterns; build emits zero spurious unused-import warnings.
  - Evidence: `examples/pirates-roster/entrypoint.ynz` exercises 7 patterns cross-file — options-variant (`ScheduleDay.home` :489), is-narrowing (`is StripeDistrictEvent` :513), dynamic (`runAnnouncement(a: dynamic Announceable)` :535), shape-field-type (`day: ScheduleDay` :543), module-const (`const OPENING_DAY_SLOT: ScheduleDay` :551), generic (`category: StatCategory` in `StatBook<T>` :560), union-alias-RHS (`shape RiverEvent = SouthSideEvent | LocalVenueEvent` :526). CLEAN-REBUILD VERIFIED (coordinator forced `touch main.rs && cargo build`; acceptance-verifier re-ran on the clean binary): `ynz build` emits ZERO unused-import warning for any type-position symbol — only the 4 pre-existing genuine unused-FUNCTION-import warnings + 1 pre-existing dead-code remain. follows/extends are an EVIDENCED same-file-only compile constraint (`shapes.rs:393/406`, `all_names` file-local) covered by unit tests (`follows_contract_does_not_warn_unused_import`, `extends_parent_does_not_warn_unused_import`) — demo + unit tests together cover every enumerated pattern.
- [x] `insta` snapshot committed and green.
  - Evidence: `crates/ynz-driver/tests/snapshots/error_galleries__pirates_roster_demo_warning_lines.snap` + test `pirates_roster_demo_builds_with_zero_m10_pattern_warnings` — green (7/7, no `.snap.new`), pins the 5-warning set with all M10 pattern symbols ABSENT; path-stripped for worktree-stability (deviation-judge #3 JUSTIFIED). Committed in the Phase 11 commit below (the `.snap` was untracked at review time — all phase work is uncommitted until the coordinator's commit step; staged + committed here, satisfying the "committed" conjunct the acceptance-verifier correctly flagged as pending).
- [x] No orphaned TODO/FIXME in M10-touched code.
  - Evidence: two grep passes (diff-scoped + direct) over all 7 Phase 11 files → zero `TODO`/`FIXME`. "Phase 0"/"Bug 2.x" in comments are durable context names, not work-items.
**Quality gate**:
- [x] Demo section uses real Yinz operations only (no invented APIs per dot-postfix.md). (code-reviewer + rules-compliance + plan-adherence all confirmed real Yinz; demo builds clean.)
- [x] Folder/section naming Pittsburgh-themed per examples-structure.md. ("Three Rivers schedule" section; ScheduleDay/StripeDistrictEvent/SouthSideEvent/RiverEvent — Pittsburgh-themed; single-entry layout preserved.)
**Verification**: `cargo test --workspace` green (≥1434 + all new tests); `cargo clippy --workspace -- -D warnings` clean; `cargo fmt --all --check` clean; demo build snapshot green.

**Phase Review Gates**:
- [x] code-reviewer: PASS 2026-05-31 (r1 — reverted the 8-line union-alias fix to prove tests bite; demo builds clean, ZERO type-position spurious warnings; limitation comment audited line-by-line vs shapes.rs, accurate; snapshot non-vacuous + worktree-stable.)
- [x] rules-compliance-reviewer: PASS 2026-05-31 (r1 — zero violations; durable comments, real Yinz, Pittsburgh-themed, non-OOP, no test weakening, follows/extends limitation legit per no-duct-tape.)
- [x] plan-adherence-verifier: PASS 2026-05-31 (r1 — both steps MET, scope respected; 3 deviations documented with clean rationales [no banned phrases]; pattern coverage = demo(7) ∪ unit-tests(all) complete.)
- [x] acceptance-verifier: PASS 2026-05-31 (r2 on CLEAN rebuild — AC1 MET [zero spurious warnings], AC3 MET; AC2 MET on commit. r1 BLOCK was a STALE-BINARY/build-race false negative [7 agents built target/ concurrently]; coordinator clean-rebuild + code-reviewer revert-test + plan-adherence all refuted it; r2 solo re-run confirmed clean.)
- [x] Deviation-judge #1 (union-alias fix folded into Phase 11): JUSTIFIED 2026-05-31 (same Phase-0 bug class, 7th type position, same idiom, files in front-matter, unit-test-proven, surfaced by the Demo invariant — deferring would ship a teaching lie).
- [x] Deviation-judge #2 (follows/extends absent from demo): JUSTIFIED 2026-05-31 (independently verified shapes.rs:393/406 makes cross-file follows/extends a compile error — demo physically can't show it; unit-test substitution correct; accurate comment; tracked todos line 12).
- [x] Deviation-judge #3 (path-stripped warning-lines snapshot): JUSTIFIED 2026-05-31 (strictly stronger than full-stderr here — full-stderr embeds worktree-absolute paths and fails everywhere, as the 5 pre-existing integration .snap.new prove; + 4-symbol assert! belt-and-suspenders; matches error_galleries prior art).
- [x] Committed: <commit SHA>

**Findings Log**:
_(no executor BLOCK rounds. Scope EXPANDED mid-phase to fold in a Rule-11 same-class fix — the union-alias-RHS unused-import gap [`check.rs` alias_ty walk + 2 unit tests], discovered during the Demo invariant's mandatory build validation; deviation-judge #1 ruled JUSTIFIED. Two earlier executor MISDIAGNOSES were caught + corrected by coordinator verification: (a) a "separate driver unused-import tracker" claim [FALSE — single source `queries.rs:175`, driver calls `check_query`]; (b) a "union-alias can't build cross-file in the driver" claim [FALSE — it builds clean; the false demo comment was deleted and the union pattern restored]. The follows/extends cross-file limitation [shapes.rs:393/406] is REAL [verified] and accurately documented. acceptance-verifier r1 BLOCK was a stale-binary false negative from concurrent-agent build races — resolved by a solo clean-rebuild re-run [r2 PASS]. Three out-of-M10-scope walker-completeness follow-ups [Phase 9 return-stmt ownership, Phase 10 copy-hint UFCS + copy-hint block-stmt] are tracked in todos.md.)_

---

## End-of-Plan Deliverable — Before/After Report (Patrick, 2026-05-30)

After all phases pass the final cumulative review, produce a **before/after report** for Patrick covering every bug fixed. For each of the 14 bugs (Bug 1, 2.1–2.14), state:
- **What was broken** — the bug in one line.
- **Before (what Patrick saw in the editor/extension)** — the concrete wrong behavior visible in VSCode (e.g. "spurious `Timeframe` imported-but-never-used warning on valid code", "hovering `share` variable showed the keyword doc", "`let x = 5; print(x)` showed NO `let→const` hint", "completion popup opened on every space").
- **After (what Patrick should now see)** — the corrected behavior (e.g. "no warning", "variable type shown", "`let→const` hint now fires", "popup only on `.`"), and whether it's a **visible behavior change** (some fixes change what shows up — e.g. `let→const` hints now appear on far more bindings; the `array→fixed` decoration is now clickable; the inlay-hint color is now Pittsburgh gold) vs an **invisible correctness fix**.
- **How to see it** — the file/scenario in `examples/pirates-roster/` or a minimal repro to eyeball it after installing the rebuilt VSIX.

Group the report by "you'll notice this changed" vs "quietly correct now." Call out the two CHANGELOG-worthy behavior shifts explicitly (Bug 2.9 → many more `let→const` hints; Phase 4 → gold inlay color + end-of-line positioning). This is the artifact Patrick reviews to validate the milestone hands-on.

---

## Anti-Pattern Callouts

- **Splitting into commits instead of PRs**: Each phase is a self-contained bug fix sized as its own PR/commit on the M10 branch; Phase 0 is deliberately one PR (6 sites + shared root cause + shared test file), not six. Size is per-PR per branching.md.
- **Shadow main branches**: M10 branches off `main` (at `0.3.0-m1`); no long-lived parallel main. The v0.2.1 track is a real release branch M9 merges into v0.3 — documented in the roadmap's amended release model, not an orphan.
- **Building the engine before shipping value**: Every phase ships a user-visible fix on its own. M10 is dispatched FIRST precisely because it delivers value (fewer false positives, working hints) in days — no infrastructure-before-value.
- **Hotfix that isn't**: These are real fixes with regression tests that fail-then-pass; no band-aids. The test-first protocol guarantees each fix is proven against a reproducing test.
- **Abandoned branches**: M10 is a single milestone branch; phases commit to it sequentially and it merges via `/pr` when done. No per-phase branch is left dangling.
- **Flag graveyards**: No feature flags — these are correctness fixes, not flag-gated features. Nothing to clean up.

## Quality Checklist (verify at completion)
- [ ] All inputs validated — N/A (no new user input surface; typeck/LSP internal)
- [ ] Auth/authz enforced — N/A (dev tooling)
- [ ] Error handling: diagnostics keep WHAT/WHAT-INSTEAD/WHY; no panics added on lookup misses
- [ ] No SQL injection / XSS / path traversal / secret exposure — N/A
- [ ] Performance: no new salsa query; O(1) per-arg lookups; no N+1
- [ ] Tests: every bug has a fail-then-pass regression test (happy + the wrong-behavior case)
- [ ] Existing tests still pass (≥1434 baseline preserved)
- [ ] Types complete (no `as any`-equivalent, no unguarded `.unwrap()` added)
- [ ] Follows existing codebase patterns (inserts mirror 2403/2407/1485/2216; walkers mirror ownership-hints recursion)
- [ ] Every phase received the four-reviewer PASS before committing
- [ ] Final cumulative reviewer sweep passed (Phase 11)
- [ ] Plan-file acceptance-criteria checkboxes accurate across all phases
- [ ] `examples/pirates-roster/entrypoint.ynz` extended + snapshot green (Demo invariant)
