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
