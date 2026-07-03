---
name: "v0-3-concurrency-perf-audit"
plan-id: "2026-05-21-v0-3-concurrency-perf"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-05-21-v0-3-concurrency-perf

Append-only sidecar for the v0.3 concurrency/perf roadmap. Durable per-phase deferral records
(reviewer should-fix findings deferred at a milestone phase boundary, not fixed in-phase) land
here, attributed to the deferring plan+phase, each guarded by an `Idempotency-Key:` sentinel.

## 2026-07-03 — Deferral: R3 matrix header overclaims "full cross-product" coverage (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#3 at the phase boundary)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#3: crates-ynz-driver-tests-integration-rs-9773

- **WHAT** — The R3 boundary-exactness matrix's own header comment
  (`crates/ynz-driver/tests/integration.rs:9773`) claims to cover "the full cross-product
  {share,lend,give,copy,channel} × {concrete,generic,UFCS-method,non-ident,unresolvable} in BOTH
  directions," but the 11 shipped fixtures are a representative diagonal (each axis-value
  exercised at least once), not the full 25-cell product — notably missing a channel×generic cell
  that would specifically exercise the NEW generic-callee-resolution `.or_else` arm this phase
  added (code-reviewer, should-fix).
- **WHY** — The safety property is not at risk today (the modifier axis and the
  sig-vs-generic resolution axis are independently exercised elsewhere in the matrix), so this is
  a documentation-accuracy / future-maintainer-clarity issue, not a live gap — fixing it now is a
  nice-to-have, not urgent, and the phase's actual R3 safety gate is unaffected.
- **COST** — Cheap — one new fixture (`v0_3_m4_p3_cross_channel_generic.ynz`) + one new test
  function, OR simply soften the header comment's "full cross-product" wording to "representative
  cover of each modifier × each callee-kind."
- **TRIGGER** — The next time someone touches the generic-callee resolution path in `check.rs`'s
  boundary predicate, or the next full Phase-3-surface review.

## 2026-07-03 — Deferral: arc.rs hammer test never exercises the contended 1→0 last-release window (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#3 at the phase boundary)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#3: crates-ynz-runtime-src-arc-rs-147

- **WHAT** — The `concurrent_clone_free_hammer_keeps_the_count_exact` test
  (`crates/ynz-runtime/src/arc.rs:147`) spawns 8 threads racing clone/free against a shared
  allocation, but the main thread holds the original reference for the entire duration, so the
  refcount never actually hits 0 during the race — the test proves atomic counter correctness
  (guaranteed at any ordering) but never exercises the Release-decrement + Acquire-fence 1→0
  last-release window the acquire-release discipline specifically exists to protect
  (critical-path-integrity, should-fix).
- **WHY** — `arc.rs` currently has zero codegen callers (the emission itself is deferred per
  FRAGO 008 in `2026-07-02-v0-3-m4-channels-arc-release/audit.md`), so there is no live blast
  radius today — but this test gap must close BEFORE the emission ships, or the emission would go
  live on a substrate whose most safety-critical path (the actual free-on-last-release race) was
  never adversarially exercised.
- **COST** — Small — add a second concurrent test variant where N threads each own their own
  clone (no main-thread-held anchor) and all drop concurrently, so the final release is genuinely
  contended; ideally run under Miri or TSan to catch an ordering violation if one exists.
- **TRIGGER** — Whenever the auto-Arc codegen emission (FRAGO 008's deferral,
  registry `auto-arc-codegen-emission`) is picked back up — this test gap must close as a
  precondition of that work, not an afterthought.

## 2026-07-03 — Deferral: "stable across 5 repeated runs" claim is not CI-enforced (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#3 at the phase boundary)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#3: -claude-planning-active-2026-07-02-v0-3-m4-channels-arc-release-plan-md-963

- **WHAT** — The plan/audit record
  (`.claude/planning/active/2026-07-02-v0-3-m4-channels-arc-release/plan.md:963`; also cited at
  that plan's `audit.md:959`) claims the R3 cross-thread fixtures were "verified stable across 5
  repeated runs (no intermittent race)," but no such repeated-run loop exists anywhere in the
  committed test suite (grep-confirmed workspace-wide) — `cargo test` runs each fixture exactly
  once. The claim was an ephemeral manual observation during the executor's session, recorded as
  if it were durable, CI-enforced coverage (critical-path-integrity + test-quality, converged
  independently — should-fix).
- **WHY** — Currently low-risk in practice (the R3 fixtures cross via COPY/GIVE/channel — no
  shared mutable state to race on today), so the claim being unbacked doesn't currently hide a
  live bug — but the plan record overstates what the suite actually guards going forward, and
  once the auto-Arc emission ships (genuine shared refcounted state), an unenforced "no
  intermittent race" claim would be actively dangerous.
- **COST** — Small-to-moderate — either add a real `for _ in 0..5 { ... }` repeated-run wrapper
  around the affected R3 assertions in `integration.rs`, or scope the plan/audit record's
  language down to "observed once during manual execution, not CI-enforced" so it doesn't
  overstate coverage.
- **TRIGGER** — Same as the arc.rs hammer-test deferral above — before or alongside the auto-Arc
  codegen emission work, when genuine shared-refcount racing becomes a live path; or the next
  time any R3/auto-Arc fixture is touched.
