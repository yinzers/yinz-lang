# v0-3-m3b-auto-parallelization Phase 1 Deviations — CLOSE-OUT ROUND (round 3), captured 2026-06-05

Scope: Patrick split full cross-module suspension codegen → milestone M3e. P1 now ships WORKING cases + LOUD-REJECT guards for the rest (the M3a→M3c pattern). Close-out round: fixed the buggy `composed_frame_simple` predicate (round-1 used `imported_fn_names` not `imported_suspending_names` → guard didn't fire → SIGILL), stripped debug prints, added loud-reject fixtures + the M3e deferral doc + registry entry.

D_count: 4 (judged — all mechanical/forced) + 1 SAFETY-FLOOR probe (the guard predicate, in-scope step 8 — judged for escape-completeness, not as a deviation).

## SAFETY-FLOOR PROBE (not a deviation — the core of step 8)
- **The conservative loud-reject guard (`composed_frame_simple` on FunctionSig).** Must reject EVERY cross-module suspending combo outside the proven-working set. Adversarial target (code-reviewer + dedicated judge): find a combo that ESCAPES the guard (marked simple=true) and still silently CRASHES — 4-module chain, shape RETURN cross-module, number/decimal128/float value-return cross-module, loop-var crossing, mixed transitive+shape, transitive depth ≥2. Over-rejection = OK (clean error); escape-then-crash = BLOCK (violates Patrick's no-silent-failure floor).

## Documented deviations (mechanical — forced)
### J-C hover.rs — `composed_frame_simple: true` added to 3 FunctionSig literals (struct exhaustiveness, forced by in-scope signatures.rs field).
### J-D completion.rs — same, 4 literals.
### J-E tmgrammar — `cargo run -p ynz-tmgrammar` regenerated `ynz.tmLanguage.json` (forced: registry `[[deferred_language_feature]]` add would otherwise fail the grammar-drift snapshot test).
### J-F design/future/index.md — one table row added for the new M3e deferral doc (SSOT index completeness).

(Per coordinator judgment + deep context budget: the 4 mechanical deviations are verified by plan-adherence + rules-compliance in-band rather than 4 separate judge spawns — same inert class J-C/J-D already PASSed in round 2. The high-value adversarial spawn is the guard-escape judge.)
