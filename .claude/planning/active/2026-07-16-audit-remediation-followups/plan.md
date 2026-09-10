---
name: "audit-remediation-followups"
plan-id: "2026-07-16-audit-remediation-followups"
status: "stub"
roadmap-id: null
session-id: []
created_at: "2026-07-16"
updated_at: "2026-07-16"
metadata:
  type: "plan"
---

# Deferral stub — non-blocking findings from 2026-07-16-audit-remediation-two-lane

## 2026-07-16 — Deferral: cache resolved string-map slot to avoid per-iteration SipHash + reprobe (non-blocking — deferred by 2026-07-16-audit-remediation-two-lane#A1 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A1: crates-ynz-runtime-src-lib-rs-1121

- WHAT: cache the resolved slot index (or 64-bit hash) alongside the key in `insert_order` so
  `ynz_map_iter_get_str` stops recomputing SipHash + reprobing per iteration position
  (performance-reviewer should-fix; hot via codegen `map_iter_get_into` in for-loops).
- WHY: constant-factor perf polish, not correctness — total iteration stays O(n); layout change to
  `insert_order` touches the map ABI and deserves its own design pass, not a fix-round rider.
- COST: ~1 session (`insert_order` layout + growth-path update + tests).
- TRIGGER: string-keyed map iteration shows up in a real hot loop / profiling, or the next
  runtime-map milestone touches `insert_order` anyway.

## 2026-07-16 — Deferral: harden string-map hash-probe wall-clock ratio test against CI noise (non-blocking — deferred by 2026-07-16-audit-remediation-two-lane#A1 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A1: crates-ynz-runtime-tests-map-str-hash-probe-rs-150

- WHAT: harden the wall-clock ratio test against shared-CI noise (op-count-based assertion, or
  `#[ignore]`-by-default perf suite) per test-quality should-fix.
- WHY: current best-of-3 + 51× margin makes flake risk low but nonzero; converting to op-count
  assertions is a deliberate test-design change that would need probe instrumentation.
- COST: <1 session.
- TRIGGER: the test flakes red once in CI without a real regression.

## 2026-07-16 — Deferral: M3b wall-clock-race flaky test (non-blocking — deferred by 2026-07-16-audit-remediation-two-lane#A2 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A2: crates-ynz-driver-tests-integration-rs-8231

- WHAT: v03_m3b_p4_model_a_intended_reorder_parallel_output (crates/ynz-driver/tests/integration.rs:8231) asserts 50ms-vs-100ms wall-clock ordering and flips under parallel-run load; redesign to synchronization-based or sufficiently-separated durations.
- WHY: the current shape proves real concurrent overlap, which a synchronization-only rewrite might lose — a real test-design tradeoff owned by M3b-era work, not Lane A's bug-remediation scope.
- COST: small — rework one fixture + two assertions (<1 session).
- TRIGGER: next CI flake of this test, or the next plan touching M3b auto-parallelization tests.

## 2026-07-16 — Deferral: contention-sensitive stdout-race test class (non-blocking — deferred by 2026-07-16-audit-remediation-two-lane#A3 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A3: crates-ynz-driver-tests-integration-rs-7346

- WHAT: v03_m3g_background_fused_group_detach (integration.rs:7346) + v0_3_m4_p3_cross_copy_safe_and_byte_identical + v0_3_m4_p3_cross_give_generic_not_over_rejected all assert exact stdout while detached/parallel tasks can interleave under CPU contention (reproduced 18/20 under forced load; pass in isolation); redesign the class (sync-based output capture or per-stream separation).
- WHY: the racy shape is what proves real concurrent overlap — a redesign needs care not to lose the property under test; owned by M3b/M3g-era work, not Lane A remediation.
- COST: ~1 session for the class.
- TRIGGER: next CI flake of any of the three, or the next plan touching auto-parallelization tests.

## 2026-07-16 — Deferral: LSP caught-panic state-consistency + message redaction (non-blocking — deferred by 2026-07-16-audit-remediation-two-lane#A3 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A3: crates-ynz-lsp-src-server-rs-150

- WHAT: (a) a caught mid-loop panic in multi-file ops (did_rename_files) can leave db/open_documents/disk partially patched for the session (pre-F3: process crash + client restart self-healed); (b) caught-panic messages round-trip unredacted into JSON-RPC error responses.
- WHY: single-tenant LSP (developer's own editor+files); restart recovers; a transactional rollback for multi-file ops is a real design pass, not a fix-round rider.
- COST: ~1 session (op-scoped rollback or re-index-on-caught-panic; message scrub).
- TRIGGER: first field report of post-panic LSP inconsistency, or the LSP gaining multi-client/remote surface.

## 2026-07-16 — Deferral: registry self-referential-shape substitute teaches bare maybe T (non-blocking — deferred by 2026-07-16-audit-remediation-two-lane#A4 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A4: registry-features-toml-1300

- WHAT: `registry/features.toml` ~line 1300, `[[deferred_language_feature]] self-referential-shape`'s
  `substitute` field reads bare `` `maybe T` field `` — invalid syntax (parser requires `maybe<T>`);
  same confirmed bug class as the A4 `maybe<T>` sweep, but this field was outside A4's drawn correction
  surface (docs/ spec files + the jargon entries the reviewers named).
- WHY: surfaced only at round-2 review after the fix round closed; fixing it then would have re-opened
  a third review round for a one-line change (review-round cost exceeds the defect's user impact — it
  is a deferred-feature record's substitute text, lower-traffic than the banned_jargon diagnostics
  already fixed).
- COST: one-line edit + `cargo test -p ynz-registry -p ynz-diagnostics` re-run (<5 min).
- TRIGGER: the next plan/phase that touches `registry/features.toml`, or the followups plan's execution,
  whichever first.

## 2026-07-16 — Deferral: docs/README type-system link points at file not section anchor (minor — deferred by 2026-07-16-audit-remediation-two-lane#A4 at the phase boundary)

Idempotency-Key: 2026-07-16-audit-remediation-two-lane#A4: docs-readme-md-32

- WHAT: `docs/README.md:32`'s new `IMP-type-system.md` inline link points at the whole file rather than
  the `#no-override-keyword-function-overloading-by-argument-type-is-not-implemented-v01` anchor the
  prose describes. Link resolves; precision nit only.
- WHY: cosmetic; not worth a review round on its own.
- COST: one-line edit (<1 min).
- TRIGGER: next edit to `docs/README.md` or the followups plan's execution.
