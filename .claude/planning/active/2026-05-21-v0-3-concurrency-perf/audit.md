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

## 2026-07-03 — Deferral: prefer-yielding-sleep lint name drops the -when-Y convention clause (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#4 at the phase boundary)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#4: registry-features-toml-2306

- **WHAT** — The `prefer-yielding-sleep` lint rule name (`registry/features.toml:2306`) drops the
  `-when-Y` clause of this project's `prefer-X-when-Y` naming convention (e.g.
  `prefer-fixed-when-immutable` per `auto-promotion.md`'s stated convention).
- **WHY** — Purely cosmetic naming drift; the lint's actual behavior and teaching text are
  correct — renaming it now would be a breaking change to the registry entry name for zero
  functional benefit mid-milestone (rules-compliance, minor).
- **COST** — Trivial — a rename (`prefer-yielding-sleep` → e.g.
  `prefer-yielding-sleep-when-non-kernel` or similar) plus updating every reference (registry
  entry, `lints.rs` call site, test assertions, gallery key-phrase).
- **TRIGGER** — The next time this lint rule's registry entry is touched for any other reason, or
  a broader lint-naming consistency pass.

## 2026-07-03 — Deferral: gate-test comment cross-references a nonexistent test file (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#4 at the phase boundary)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#4: crates-ynz-typeck-tests-false-sharing-no-auto-parallel-gate-rs-13

- **WHAT** — A test comment (`crates/ynz-typeck/tests/false_sharing_no_auto_parallel_gate.rs:13`)
  claims the end-to-end half of the `--no-auto-parallel` gating proof "lives in
  `crates/ynz-driver/tests/false_sharing_gating.rs`" — that file does not exist
  (verified against the live tree). The real end-to-end test is
  `v0_3_m4_p4_padding_gates_off_under_no_auto_parallel_with_identical_output` in
  `crates/ynz-driver/tests/integration.rs` (verified at :9930).
- **WHY** — A stale cross-reference comment, zero functional impact — but would send a future
  reader chasing a nonexistent file (test-quality, minor).
- **COST** — Trivial — one-line comment correction.
- **TRIGGER** — The next time this test file is touched for any other reason.

## 2026-07-03 — Deferral: prefer-yielding-sleep lint fires before the sleepBlocking arity guard (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#4 at the phase boundary)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#4: crates-ynz-typeck-src-check-rs-3602

- **WHAT** — The `prefer-yielding-sleep` lint fires unconditionally (in non-kernel mode) BEFORE
  the `call.args.len() != 1` arity guard in `check_sleep_blocking_call`
  (`crates/ynz-typeck/src/check.rs:3602` — fn verified at that exact line; the lint push at
  :3610 precedes the arity guard at :3630), so a malformed `sleepBlocking()` or
  `sleepBlocking(a, b)` call (already a hard error) also emits the lint suggestion alongside the
  arity error — cosmetic noise on already-erroring code, confirmed non-crashing (falls back to a
  literal `"ms"` string when no valid arg exists).
- **WHY** — A write-order oversight, not a forced tradeoff — nothing prevented ordering the
  checks correctly; purely a low-priority code-quality nit with zero user-facing correctness
  impact beyond slightly redundant diagnostic output on code that's already an error
  (deviation-judge, minor).
- **COST** — Trivial — move the lint-firing block after arity/type validation, or gate it on
  `call.args.len() == 1`.
- **TRIGGER** — The next time `check_sleep_blocking_call` is touched for any other reason.

## 2026-07-03 — Deferral: store_binding has no MapEntry deep-copy arm (suspected 4th escape surface) (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#3 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3: store-binding-mapentry-escape-gap

- **WHAT** — `store_binding` (`crates/ynz-codegen/src/emit.rs`, ~line 18907) has no `MapEntry`
  deep-copy arm — it falls through to a raw pointer store for that type. This is the same
  escape-bug class Phase 3 already fixed twice this phase (a `value_to_stable_bits` MapEntry arm
  for persist surfaces, and a `prepare_bg_arg_for_ctx` pre-gate for background-arg surfaces) but
  on a THIRD, un-probed surface: a function declared to return `MapEntry<K,V>`, or a
  `let e2 = entry` binding that escapes its scope without ever routing through
  `value_to_stable_bits`, could carry a dangling pointer into freed frame memory.
- **WHY** — code-reviewer flagged this as should-fix, not blocker, because it could not confirm
  the exact source-level repro is reachable within its review scope (no `Bash`/build access) —
  building the fix now, unconfirmed, risks either a wasted fix for an unreachable case or a fix
  without a locking regression test. Real cost: this is genuinely the 4th instance of the same
  bug class in this one migration (array-element escape, MapEntry bg-escape, MapEntry
  array-escape, and now this suspected 4th), so leaving it unconfirmed carries real
  technical-debt weight, not just theoretical risk.
- **COST** — 1 focused session — add a `MapEntry` arm to `store_binding` mirroring the
  `value_to_stable_bits` clone pattern (counted heap-cell clone of the `{i64,i64}` entry struct
  + deep value-half copy), plus author a repro fixture (a function returning `MapEntry<K,V>`, or
  a map-loop-var binding escaping via `let e2 = entry` without an intervening choke-point call)
  and a tripwire test proving the fix closes it.
- **TRIGGER** — (a) a future milestone's own adversarial sweep independently reproduces this
  escape class on the suspected third surface, OR (b) Phase 4/5's SoA work touches
  `store_binding` for an unrelated reason and the reviewer/executor at that point should
  re-check this gap while already in the function.

## 2026-07-03 — Deferral: no golden-IR-snapshot coverage on the new map choke-point call sites (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#3 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#3: map-choke-point-golden-ir-snapshot

- **WHAT** — No golden-IR-snapshot exists asserting the NEW map choke-point call sites'
  (`map_new`, `map_val_set`, `map_val_get_into`, `map_val_get_maybe`, `map_iter_get_into`,
  `map_count_val`, `map_has_int` in `crates/ynz-codegen/src/emit.rs`) generated LLVM IR shape in
  absolute terms — the 13 refreshed golden snapshots this phase cover regression-detection on
  OLD (pre-cut, non-map) call sites' decl-signature churn only.
- **WHY** — Two independent reviewers (code-reviewer, test-quality) confirmed this is a real gap
  but judged it low-priority defense-in-depth, not a current correctness concern —
  runtime-behavior testing (the `m5_p3_mapshape_*`/`m5_p3_sweep_*` fixture matrix, which caught
  both real miscompiles this phase found) is the appropriate primary verification tool for
  genuinely NEW codegen logic; a golden/insta snapshot is a drift-detector against an
  already-blessed baseline, not a correctness prover for new code. Building the snapshot now,
  while the map codegen may still shift in Phase 4/5's SoA work, risks freezing a soon-to-be-stale
  baseline.
- **COST** — <1 session — author one `map<int,Shape>` build+iterate fixture, generate its golden
  IR snapshot via the existing `insta` harness, review it by hand once for correctness, commit
  as the new baseline.
- **TRIGGER** — Once Phase 5 (SoA codegen) stabilizes the map/array codegen paths for good (SoA
  rides the same by-value substrate), add this snapshot then — building it before Phase 5 risks
  needing to regenerate it again once SoA lands.
