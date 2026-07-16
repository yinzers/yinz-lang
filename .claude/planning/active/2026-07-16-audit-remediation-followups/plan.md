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
