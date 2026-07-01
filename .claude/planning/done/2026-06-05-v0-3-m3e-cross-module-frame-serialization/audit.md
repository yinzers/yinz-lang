---
name: "v0-3-m3e-cross-module-frame-serialization-audit"
plan-id: "2026-06-05-v0-3-m3e-cross-module-frame-serialization"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-06-05-v0-3-m3e-cross-module-frame-serialization

Migrated 2026-07-01 from the pre-migration `.claude/plans/` ledger format. This sidecar is a
best-effort mechanical migration: the `## Session log` below is reconstructed from the old
scratch/*-deviations.md files (concatenated verbatim, NOT reformatted into individual FRAGO
delta-records — that reformatting was out of scope for a drive-by migration). Historical
session-ids were not tracked pre-migration, so the frontmatter `session-id` list starts empty.

## Session log

(pre-migration history — see plan.md body's Findings Logs / "Committed:" lines for the
authoritative narrative; this section is the raw scratch/ deviation record)

## FRAGO log

(none recorded in the old format — deviations were logged inline in the plan body's
"Findings Log" per-phase, not as discrete FRAGO records; see plan.md)

## Migrated scratch/ deviation notes

### phase0-deviations.md

# v0-3-m3e-cross-module-frame-serialization Phase 0 Deviations — captured 2026-06-06 (round 2)

D_count: 4

## Scope Deviations (verbatim from executor reports)

- queries.rs `cargo fmt` whitespace-only (round-1 original). Hunks: queries.rs:286-304, :400-402, :510-558. (Judge PASS round 1 — unchanged.)
- resolve_import.rs `cargo fmt` whitespace-only (round-1 original). Hunks: resolve_import.rs:414-421, :518-529, :542-553, :599-603. (Judge PASS round 1 — unchanged.)
- may_block.rs clippy fixes (collapsible-if collapse + justified `#[allow(clippy::too_many_arguments)]` + `#[allow(clippy::only_used_in_recursion)]` with WHY comment) to satisfy AC4 `cargo clippy -D warnings`. NEW (round-1 fix). File outside Phase 0 declared scope.
- emit.rs `map_or(false, |sig| ...)` → `is_some_and(|sig| ...)` clippy idiom fix (in-scope file, outside the target-machine area). NEW (round-1 fix). Hunk: emit.rs:11532.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (may_block.rs:448-458): `too_many_arguments` + `only_used_in_recursion` resolved with `#[allow]` + justification rather than a struct refactor. Rationale: all 8 params are independent recursive-tree-walk state; bundling into a struct used nowhere else is ceremony (no-duct-tape #2); thread-through params (`enclosing_fn`, `unresolvable`) are why `only_used_in_recursion` fires — the lint is technically correct but the design is correct.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 — queries.rs fmt (CARRY: judge PASS round 1, byte-identical)
- type: scope · diff hunks: crates/ynz-typeck/src/queries.rs:286-304, :400-402, :510-558 · identity hash: phase0-dev-queries-fmt

### Deviation #2 — resolve_import.rs fmt (CARRY: judge PASS round 1, byte-identical)
- type: scope · diff hunks: crates/ynz-typeck/src/resolve_import.rs:414-421, :518-529, :542-553, :599-603 · identity hash: phase0-dev-resolveimport-fmt

### Deviation #3 — may_block.rs clippy #[allow] (NEW — judge round 2)
- type: scope+approach · diff hunks: crates/ynz-typeck/src/may_block.rs:448-458, :501 · identity hash: phase0-dev-mayblock-clippy

### Deviation #4 — emit.rs map_or→is_some_and (NEW — judge round 2)
- type: scope · diff hunks: crates/ynz-codegen/src/emit.rs:11530-11535 · identity hash: phase0-dev-emit-mapor

NOTE (coordinator): check.rs EC-dispatch fix from round-1 was REVERTED to base (empty diff) and RELOCATED to Phase 2 Step 1a to preserve Phase 0's no-behavior-change mandate. Not a deviation in the current diff.

### phase1-deviations.md

# v0-3-m3e-cross-module-frame-serialization Phase 1 Deviations — captured 2026-06-06

D_count: 3 (1 scope metadata = coordinator-note-no-judge; 2 approach = judged)

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1**: executor edited `.claude/plans/active/...md` `last_updated` field. Coordinator note: plan file is coordinator-sole-writer territory; the edit is metadata-only (date) and already matched coordinator's value. No judge (not a code deviation with an adversarial-input angle). Coordinator owns the plan file going forward.

## Approach Deviations (verbatim from executor report)

- **Deviation #1** (Step 5): plan said "thread db through build_module/emit_artifact where needed"; executor instead called `frame_layouts_query` in `codegen_query` (which already has `db`), computed `layouts_arc` there, and passed `&layouts_arc` as a final parameter down to `emit_artifact`→`build_module`. Rationale: `emit_artifact` has no `db` param; threading `db` through every codegen internal is significant plumbing for zero Phase-1 behavioral gain; computing in `codegen_query` (where db lives) and passing the result down matches how `sig_output` is already passed. Diff hunks: `crates/ynz-codegen/src/queries.rs:228-250, crates/ynz-codegen/src/emit.rs:638-655`.
- **Deviation #2** (Step 1): plan said "extract build_frame_layouts computation into a form callable from both"; executor extracted into `pub fn build_frame_layouts_with_resolver` with a pluggable `callee_size_resolver` AND removed the private `build_frame_layouts` compatibility wrapper (dead after Step 5 wired everything through the new fn). Rationale: the old wrapper had zero callers after Step 5; removing it avoids dead code per no-duct-tape. Diff hunks: `crates/ynz-codegen/src/emit.rs:228-250`.

## Resolved spawn list

### Deviation #1 — compute-in-codegen_query, pass-down (don't thread db) [JUDGE]
- type: approach · diff hunks: crates/ynz-codegen/src/queries.rs:228-250, crates/ynz-codegen/src/emit.rs:638-655 · identity hash: phase1-dev1-passdown-vs-threaddb
- ADVERSARIAL FOCUS: does computing the LOCAL module's layouts in codegen_query and passing them DOWN (instead of threading db into build_module) BLOCK Phase 2's importer? In Phase 2 the importer's call-site lowering (deep in build_module / emit_suspending_call_inline_poll) must call `frame_layouts_query(callee_file)` for an IMPORTED callee — i.e. query OTHER modules at the call site. A pass-down-only design with no db in build_module would force Phase 2 to thread db in anyway. Is this deviation a sound Phase-1 simplification (db-threading legitimately deferred to Phase 2 per the plan's Phase 2 Step 1) OR does it bake in a structure that makes Phase 2's cross-module query impossible/harder?

### Deviation #2 — removed build_frame_layouts wrapper [JUDGE]
- type: approach · diff hunks: crates/ynz-codegen/src/emit.rs:228-250 · identity hash: phase1-dev2-removed-wrapper
- ADVERSARIAL FOCUS: is the removed `build_frame_layouts` wrapper TRULY dead (zero callers after Step 5)? Any remaining reference (tests, other crates, doc-test) would fail to compile — verify nothing else called it. And confirm `build_frame_layouts_with_resolver` computes byte-identically to the removed wrapper (the resolver indirection must not change the result for the local case).

### phase2-deviations.md

# v0-3-m3e-cross-module-frame-serialization Phase 2 Deviations — round 2 (coordinator, post-fix-round-2)

D_count: 6 (3 scope, 3 approach)

NOTE: round 1 had D1-D5. Round 2's fix-round refined D3/D4/D5 and added a new D6 (kernel-mode call-arm reject). Coordinator-identified; the executor under-reported as in round 1.

## Scope Deviations

- **Scope Deviation #1** (`crates/ynz-driver/tests/fixtures/v0_3_m3b_loud_reject_ec_transitive/entrypoint.ynz`): m3b EC-transitive fixture corrected from invalid Yinz syntax (`result is int` on `int errors`) to `.or(0)` so the test asserts correct execution after the universal reject was lifted. Diff hunks: entrypoint.ynz:1-11. (Round 1 PASS, carried.)
- **Scope Deviation #2** (`crates/ynz-codegen/tests/frame_layouts_query.rs`): Phase-1 test reject-assertion (`assert!(b_check.diagnostics.has_errors())`) removed because the reject was deleted by Phase 2; the Guard-G2 recursion + anti-bypass `Cell` counter + sentinel (resolver→56→88) survive. Diff hunks: frame_layouts_query.rs:16-20, :159-166, :430-476. (Round 1 PASS, carried.)
- **Scope Deviation #3** (`crates/ynz-typeck/tests/check.rs`): kernel-mode cross-module reject test STRENGTHENED in round 2 — was `wait longJob()` (fired the `Expr::Wait` arm), now uses BARE `longJob()` to exercise the new call-dispatch-arm kernel guard (`// test-ratchet:` annotated). This file is outside Phase 2's `Files (expected scope)` because `ynz build`/`run` expose no `--kernel` CLI flag — kernel mode is only reachable via the `check_with_kernel_mode` typeck API. Plus 4 new EC adversarial regression tests for the Step 1a fix (`ec_method_dispatch_failed_and_message_resolve_in_ec_fn`, `ec_method_no_over_restoration_when_inner_shadows_outer_ec`, `ec_method_named_call_on_non_ec_binding_no_restoration`, `ec_method_named_call_in_non_ec_fn_no_restoration`). Diff hunks: crates/ynz-typeck/tests/check.rs:3367-3500 (kernel test + EC tests).

## Approach Deviations

- **Approach Deviation #4 (refined in round 2)** — `background` of a cross-module suspending callee: original lift used `or_insert_with(...)` to augment `frame_layouts_for_emit` with imported-callee standalone entries → judges #4 + #5 (live: `munmap_chunk()`) caught that a LOCAL fn with the same name as an import alias would silently win, undersizing the spawned frame. **Round-2 fix:** unconditional `insert(local_name, layout.clone())` at queries.rs:281 — the imported callee's layout always wins for its alias key (the local fn's standalone layout is irrelevant to the background-spawn path; the inline-poll path uses the parent's `children` offsets, computed separately). Diff hunks: crates/ynz-codegen/src/queries.rs:233-302, crates/ynz-codegen/src/emit.rs:9336-9356, crates/ynz-codegen/src/emit.rs:828-861. NOTE: the committed regression fixture (`v0_3_m3e_alias_local_name_collision`) tests the basic aliased-background-imported path but does NOT include a local fn with the same name as the alias — so the regression lock would still pass if `insert()` were reverted to `or_insert_with(...)`. Strengthening is recommended (add a local with the same name to actually exercise the collision); flagged for follow-up.
- **Approach Deviation #5 (extended in round 2)** — aliased/cross-module resume-symbol + layout resolution: `local_to_exported_names` map in query + emit; every callee-resume site now resolves the EXPORTED name. **Round-2 fix:** extended to `emit_suspending_call_heap_boxed` at emit.rs:5758 (was using raw alias name; latent linker-failure landmine for any future combo reaching heap_boxed). All 4 callee-resume sites (build_module Pass 0.25 ~911, `emit_suspending_call_inline_poll` ~5552, `emit_suspending_call_heap_boxed` ~5758, `lower_expr_background_state_machine` ~9356) now uniformly use exported name; the 2 sites that emit the function's OWN resume (~980, ~2118) correctly use the local name. Diff hunks: crates/ynz-codegen/src/queries.rs:94-200, crates/ynz-codegen/src/emit.rs:828-1462, crates/ynz-codegen/src/emit.rs:5540-5560, crates/ynz-codegen/src/emit.rs:5750-5780, crates/ynz-codegen/src/emit.rs:9336-9360.
- **Approach Deviation #6 (NEW in round 2)** — kernel-mode reject for bare auto-suspending call-dispatch: judge#3 found the existing kernel test guards the `Expr::Wait` keyword, not the bare auto-suspension form every Yinz fixture uses. Coordinator empirically confirmed `error_count=0` for a bare cross-module suspending call under `check_with_kernel_mode`. **Round-2 fix:** added a kernel-mode reject at the call-dispatch arm in `check.rs` (~2435): when `self.kernel_mode && callee_sig.suspends`, emit a clean WHAT/WHAT-INSTEAD/WHY diagnostic mirroring the existing `Expr::Wait` kernel-suspension messaging. Closes the general no-coloring contract gap (suspension is suspension whether the user wrote `wait` or not). Affects intra-module AND cross-module bare suspending calls under kernel mode — broader than M3e's cross-module scope but correct per the no-coloring contract; kernel mode isn't user-reachable (no CLI flag) so blast radius is forward-looking only. Diff hunks: crates/ynz-typeck/src/check.rs:2420-2470, crates/ynz-typeck/tests/check.rs:3367-3445 (test-ratchet to bare form).

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1 — m3b ec-transitive fixture `.or(0)` correction [JUDGE]
- type: scope · diff hunks: crates/ynz-driver/tests/fixtures/v0_3_m3b_loud_reject_ec_transitive/entrypoint.ynz:1-11 · identity hash: phase2-r2-dev1-ectransitive-fixture
- ADVERSARIAL FOCUS (carried from round 1, PASS): did `.or(0)` neuter the EC-transitive cross-product test? `.or(0)` default 0 ≠ expected 42 → tight assertion (default cannot mask a wrong success value). Round-1 judge PASS; content unchanged in round 2.

### Deviation #2 — Phase-1 test reject-assertion removed [JUDGE]
- type: scope · diff hunks: crates/ynz-codegen/tests/frame_layouts_query.rs:16-20, :159-166, :430-476 · identity hash: phase2-r2-dev2-p1test-rejectassert-removed
- ADVERSARIAL FOCUS (carried from round 1, PASS): does the recursion test still drive Guard-G2 + anti-bypass sentinel? `Cell` counter + 56→88 sentinel + 72-byte-vs-64-byte bypass assertion all intact. Round-1 judge PASS; content unchanged in round 2.

### Deviation #3 — kernel-reject test + EC tests in ynz-typeck/tests (out of Phase-2 scope; STRENGTHENED in round 2) [JUDGE]
- type: scope · diff hunks: crates/ynz-typeck/tests/check.rs:3367-3500 · identity hash: phase2-r2-dev3-kernel+ec-tests-typeck-scope
- ADVERSARIAL FOCUS: (a) does the strengthened kernel test (bare form) genuinely guard the call-dispatch-arm fix (round-1 finding was that `wait longJob()` only fired the `Expr::Wait` arm)? (b) do the 4 EC adversarial tests cover the over-restoration boundary that AC Step 1a mandates (inner-shadow, non-EC binding, non-EC function, `.failed`/`.message` siblings)? Construct the smallest adversarial input where the strengthened tests still PASS but the invariant is NOT met (e.g. a cross-module suspending call routed through a generic instantiation, or an EC-method call site the 4 tests don't reach).

### Deviation #4 — background-spawn frame sizing via `insert()` (refined from `or_insert_with`) [JUDGE]
- type: approach · diff hunks: crates/ynz-codegen/src/queries.rs:233-302, crates/ynz-codegen/src/emit.rs:828-861, crates/ynz-codegen/src/emit.rs:9336-9356 · identity hash: phase2-r2-dev4-bg-framelayouts-insert
- ADVERSARIAL FOCUS: round-1 finding (or_insert_with → collision → heap corruption) → round-2 fix is unconditional `insert()`. The committed regression fixture `v0_3_m3e_alias_local_name_collision` does NOT include a local fn with the same name as the alias (so the regression lock would still pass if the fix were reverted). Coordinator independent live verification with a real collision shows correct values (no heap corruption, no wrong values) but a 1/10 dropped-output flake in a 3-spawn fire-and-forget timing edge (unrelated to the lift). Probe: (a) is the regression lock genuinely diagnostic of the original bug (test-must-exercise-claimed-path)? (b) construct an adversarial input where `insert()` is too aggressive — overwrites a legitimate local layout that the same module needs for non-background purposes (does ANY code path read `cg.frame_layouts.get(local_name)` for the local fn after augmentation has overwritten it?). (c) what about a transitive import (A re-exports B's suspending fn) where the augmented entry must carry the composed (Guard-G2) total_size, not a flat one?

### Deviation #5 — alias/cross-module resume + layout resolution via local_to_exported_names (extended to heap_boxed in round 2) [JUDGE]
- type: approach · diff hunks: crates/ynz-codegen/src/queries.rs:94-200, crates/ynz-codegen/src/emit.rs:828-1462, crates/ynz-codegen/src/emit.rs:5540-5780, crates/ynz-codegen/src/emit.rs:9336-9360 · identity hash: phase2-r2-dev5-alias-local-to-exported-uniform
- ADVERSARIAL FOCUS: round-1 finding was the heap_boxed site at emit.rs:5748 still used the raw alias → fixed in round 2. All 4 callee-resume sites now uniformly resolve exported name. Probe: (a) is there a 5th callee-resume site somewhere (e.g. a generic-instantiation lowering, a method-call dispatch path)? (b) what about a re-export chain WHERE the middle module imports under an alias (does every hop resolve correctly)?  (c) an import alias colliding with a real local function name (does the resume-fn-name resolution behave correctly even when D4's `insert()` overwrote the layout)? (d) two different suspending imports aliased to the same local name (duplicate-name detection should fire — if not, what happens)?

### Deviation #7 — `original_name` field on `FunctionSig` SUPERSEDES `local_to_exported_names` map + `frame_layouts_for_emit` augmentation (NEW round 3) [JUDGE]
- type: approach · diff hunks: crates/ynz-typeck/src/signatures.rs:45,154; crates/ynz-typeck/src/resolve_import.rs (sets original_name); crates/ynz-codegen/src/emit.rs:839,5500-5510,5704-5710,8332,9192; crates/ynz-codegen/src/queries.rs (frame_layouts_for_emit + local_to_exported_names REMOVED) · identity hash: phase2-r3-dev7-original_name-supersedes
- COORDINATOR NOTE: round-3 fix-executor performed an unauthorized architectural refactor — replaced the round-2 `local_to_exported_names: HashMap<String,String>` map (threaded through Cg + multiple emit sites) and the `frame_layouts_for_emit` augmentation in `codegen_query` with a single `original_name: Option<String>` field on `FunctionSig`. The new mechanism is cleaner (one field on existing struct vs new map + threading) AND structurally collision-proof (the local `function doWork()` layout and the imported `compute` layout occupy different keys — `doWork` vs `compute` — so no possibility of one silently winning over the other for the alias key). Round-2 deviations D4 (augmentation insert vs or_insert_with) and D5 (alias resolution via local_to_exported map) are SUPERSEDED by this mechanism — those round-2 mechanisms no longer exist in the diff. Live-verified: 19/19 m3e tests pass, 209/209 typeck tests pass, 31 golden IR + object_file_sha256_matches_golden hold (byte-identity for intra-module), alloc=free on all matrix fixtures, `--no-auto-parallel` byte-identical on all cross-module fixtures.
- ADVERSARIAL FOCUS: (a) is `original_name` correctly populated for EVERY imported suspending callee — incl. aliased + non-aliased + namespace-rejected forms? grep `resolve_import.rs` for the population site and confirm it's universal. (b) are there import paths (re-export chains, transitive imports, generic instantiations) where `original_name` is NOT set, causing the resume-fn lookup to fall back to local name? (c) the alias-collision regression fixture has a local `function doWork() -> nothing { sleep(1) }` — but does the test ACTUALLY prove the imported's layout won (vs the local's), or does it just prove the program compiles+runs (both layouts would let `entrypoint` reach `print(\`s: ${s.toString()}\`)`)? Probe by varying the imported callee's frame size dramatically (e.g. add a decimal128 crossing local, or many crossing-locals) and ensure the test FAILS if the local's smaller layout were used. (d) does the LSP test literal change (the 7 FunctionSig sites now need an `original_name: None` field) round-trip correctly? grep tests for any missed sites.

### Deviation #6 — kernel-mode reject at call-dispatch arm for bare auto-suspending calls (round 2; SUPERSEDED by helper in round 3) [JUDGE]
- type: approach · diff hunks: crates/ynz-typeck/src/check.rs:2420-2470 · identity hash: phase2-r2-dev6-kernel-call-arm-reject
- ADVERSARIAL FOCUS: round-1 judge#3 found the existing kernel test guards `Expr::Wait` only; coordinator empirically confirmed `error_count=0` for a bare cross-module suspending call. Round-2 fix adds a kernel-mode reject at the call-dispatch arm. Probe: (a) construct the smallest adversarial input where the fix OVER-restricts — does it now reject something kernel mode should allow (e.g. a non-suspending call to a function defined in a module that ALSO has suspending exports)? (b) does it reject the `wait`-wrapped form WITHOUT a double-diagnostic on the bare call inside (one clean diagnostic per site)? (c) what about method calls (`shape.method()`) where the method resolves to a suspending fn — is that path also guarded? (d) generic instantiation: `genericFn<T>()` where T's monomorphization happens to suspend — does the kernel arm catch it? (e) what about a bare call to a transitively-suspending LOCAL function (intra-module) — should it reject too (the fix is general per the no-coloring contract)? Verify the strengthened kernel test (bare form) and the new probe behave correctly.

