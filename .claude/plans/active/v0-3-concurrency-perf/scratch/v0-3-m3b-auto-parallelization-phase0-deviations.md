# v0-3-m3b-auto-parallelization Phase 0 Deviations — captured 2026-06-05

D_count: 3

## Scope Deviations (verbatim from executor report)

- **Scope Deviation #1** (file: `crates/ynz-registry/tests/lsp_adapter.rs`): touched outside declared scope. Rationale: `hover_wait_includes_what_what_instead_why` and `hover_background_includes_what_what_instead_why` asserted on old M2 hover text ("Suspends the calling function" / "separate thread"); updating the registry keyword hover fields to M3b semantics is the phase's explicit goal, so these tests were the correct counterpart — failing to update them would have left AC-green registry text with AC-red test assertions. Diff hunks: `crates/ynz-registry/tests/lsp_adapter.rs:122-172`.

- **Scope Deviation #2** (file: `crates/ynz-lsp/tests/hover.rs`): touched outside declared scope. Rationale: Same cause as Deviation #1 — `hover_wait_keyword_returns_m2_suspension_text` and `hover_background_keyword_returns_routing_distinction_text` in the LSP integration test suite asserted on old text that the registry update intentionally replaced; the `background` test used `mc.value.contains("I/O pool")` which does not match "I/O thread pool" (not a substring match). Leaving these tests red would block all subsequent phases. Diff hunks: `crates/ynz-lsp/tests/hover.rs:1-250` (approximate range covering the two updated test functions).

- **Scope Deviation #3** (file: `cspell.json`): touched outside declared scope. Rationale: "callees" appears in the new `background_routing` `description` field at `registry/features.toml:2077`; project memory `cspell-words-proactive` requires immediately adding flagged legitimate programming terms — deferring would leave a cspell failure in the repo. Diff hunks: `cspell.json:18-19`.

## Approach Deviations (verbatim from executor report)

None — implementation matched plan's named approaches.

## Resolved spawn list (orchestrator's parsed view)

### Deviation #1
- **type**: scope
- **rationale**: test-sync — `lsp_adapter.rs` hover assertions updated to M3b ordering-barrier / routing hover text (the explicit goal of P0 steps 4-5). Without it, AC-green registry text would have AC-red tests.
- **diff hunks**: crates/ynz-registry/tests/lsp_adapter.rs:122-172
- **judge identity hash**: 45fff33026fdbb1d416e0ec7578fb6b3abe39424

### Deviation #2
- **type**: scope
- **rationale**: test-sync — `crates/ynz-lsp/tests/hover.rs` LSP integration assertions updated; old text ("I/O pool" non-substring of "I/O thread pool") replaced. Leaving red blocks later phases.
- **diff hunks**: crates/ynz-lsp/tests/hover.rs:1-250
- **judge identity hash**: 069f9b01c300ce45dcdae13bc99712bd2d998ecd

### Deviation #3
- **type**: scope
- **rationale**: cspell word "callees" (appears in new `background_routing` description). Project memory `cspell-words-proactive` mandates immediate add.
- **diff hunks**: cspell.json:18-19
- **judge identity hash**: 45990061f93f34069c0aa157eff501b8e5e68a32
