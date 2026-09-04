# Handoff — Phase 7 (Track 3: scope-drop cancellation) — STATUS: PARTIAL

Written 2026-09-04 by executor `m8-p7-20260904-a1`. Nothing committed; tree is `d1c4294` plus the
uncommitted files listed below. Resume-at: `phase-7/step-3` (the dispatch brief's token; the plan
numbers the Branch B sign-off gate as step 4 — same seam).

## Verdict: Branch B — RE-DEFER

The recon (steps 1–2) is DONE and the decision is MADE. The only cleanup dispatch in the tree,
`SpawnStateFnFuture::drop`'s kind arms in `crates/ynz-runtime/src/runtime.rs`, is a spawned TASK's
retirement ladder over that task's own frame. A handle binding's scope exit is the PARENT's event on
the PARENT's frame (block end / loop back-edge / `return` / the caller's `free_frame`), where no local
of any type is released today. Nothing to extend; a handle-only pass is the forked second mechanism
`IMP-no-function-coloring.md`'s parenthetical warns about. Full evidence (ten probes, IR read) is in
`audit.md` entry `m8-p7-20260904-a1`.

## What is done

- Plan `## Future Requirements / Revisit` #3 — the four-field re-deferral (WHAT / WHY with probe
  evidence / COST / TRIGGER) replaces the placeholder.
- `registry/features.toml` `background-handle-cancel-injection` — `substitute`, `why`, `triggers`,
  `ships_in` rewritten to the concrete finding (UPDATED, not retired).
- `docs/internal/implementation/IMP-no-function-coloring.md` "Task Cancellation" — current-state
  paragraph + the recon record + anchor; the "never silently killed mid-work" claim corrected
  (shutdown stops a still-running child at its next suspension when `entrypoint` returns).
- `docs/internal/implementation/IMP-concurrency.md` auto-close deferral — ruling that its drop-pass
  dependency is NOT satisfied by M8.
- Pin tests `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs` (2 tests, planned-RED inverse that
  flip when the trigger fires) over fixture
  `crates/ynz-driver/tests/fixtures/v0_3_m8_p7_handle_scope_exit_pin.ynz` — 2/2 green, exit 0.
- Plan Phase 7 STATUS block, session-id appended, Feature Registry Entries bullet's outcome recorded.

## What is left — the ONLY open item

**Patrick's sign-off on the re-deferral** (plan Phase 7 step 4's own gate: "surface for Patrick's
sign-off before closing this phase"). He reads FR #3 and the registry entry's rewritten fields. On
sign-off: record it in `audit.md` (the Phase 1/2 SIGN-OFF precedent), mark the Phase 7 STATUS block
closed, and delete this handoff. If he instead rules Branch A must ship, that is a FRAGO — the
evidence says it is the drop-story milestone's pass, not a contained M8 change.

Step 5 (full pre-existing suite) is the gate agent's lane, not the executor's.

## Do not

- Do not implement a handle-only scope-exit release (the rejected branch). Do not add a
  `BG_ARG_KIND_TASK_HANDLE` arm to the parent's ladder (weighed, rejected — retirement ≠ scope exit).
- Do not touch the `errors`-surface paths (`.message` / `.failed()`) — parked 32–34 ride their own
  hotfix branch.
- `target/probe-p7/` holds the throwaway probes (gitignored under `target/`); not to be committed.
