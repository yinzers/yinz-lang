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

## 2026-07-03 — Deferral: all 5 inlay-hint-pass Stmt walkers skip assignment-lvalue recursion (non-blocking — deferred by 2026-07-02-v0-3-m4-channels-arc-release#5 at the post-review fix round)
Idempotency-Key: 2026-07-02-v0-3-m4-channels-arc-release#5: crates-ynz-typeck-src-inlay-hint-passes-rs-1554

- **WHAT** — All 5 inlay-hint-pass Stmt walkers in
  `crates/ynz-typeck/src/inlay_hint_passes.rs` (the `channel_capacity` walker at :1554 and its 4
  siblings) recurse into `FieldAssign.value` / `IndexAssign.index`+`value` but NOT into
  `FieldAssign.target` / `IndexAssign.receiver` — a hint-relevant construction nested inside an
  assignment LVALUE is silently missed by EVERY hint domain, not just the new channel_capacity
  one (code-reviewer, P5 fix round; confirmed against the live tree — the new walker exactly
  matches all 4 pre-existing siblings, so this diff introduced no regression).
- **WHY** — Fixing it correctly needs ONE shared-visitor refactor across all 5 passes
  (reusability.md: same plumbing in 5 places); a partial fix in only the newest walker would
  fork the walkers' semantics and create exactly the parallel-derivation inconsistency
  authoritative-derivation.md bans. That refactor is real scope beyond a single phase's fix
  round, and leaving it matches existing behavior exactly (zero regression).
- **COST** — One shared-visitor extraction touching 5 walker families in
  `inlay_hint_passes.rs`, roughly a half-day including tests.
- **TRIGGER** — A second independent report of a hint failing to fire inside an assignment
  lvalue, OR whenever the next hint-pass domain is added (the natural refactor point — do the
  extraction BEFORE adding walker #6).

## 2026-07-04 — Deferral: param-shadowed-by-let produces two LayoutDecisions.arrays rows sharing one array_name (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#4 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4: crates-ynz-typeck-src-soa-rs-294
Filed-by-session: phase4-deferral-executor-2026-07-03-m5

- **WHAT** — `crates/ynz-typeck/src/soa.rs:294`: a param `arr: array<Shape>` shadowed by a body
  `let arr = [...]` produces TWO `LayoutDecisions.arrays` rows sharing the same `array_name` (one
  param-declined row, one possibly-Admitted let row).
- **WHY** — Inert for Phase 4 because codegen consumes only `layout.padded_shapes`, never
  `layout.arrays`, so the duplicate-keyed row has zero observable effect today; fixing it now
  would be scope creep into Phase 5's own consumption design, which hasn't decided how it
  keys/dedupes `arrays` rows yet.
- **COST** — Small, bounded to Phase 5: either dedupe by `(array_name, decl_span)` instead of
  `array_name` alone, or accept the duplicate and have Phase 5's codegen key its lookup the same
  way. Roughly a half-day of Phase-5-scoped work once the consumption design exists to react to.
- **TRIGGER** — Phase 5 (SoA codegen on the by-value substrate) begins consuming
  `LayoutDecisions.arrays` for real; its own design must decide the keying/dedup story, at which
  point this pre-existing duplicate-row shape becomes load-bearing and must be resolved.

## 2026-07-04 — Deferral: Pass 2 Match-arm scan skips arm.pattern's Value(Expr) variant (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#4 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#4: crates-ynz-typeck-src-soa-rs-535
Filed-by-session: phase4-deferral-executor-2026-07-03-m5

- **WHAT** — `crates/ynz-typeck/src/soa.rs:535`: Pass 2's `Match` arm scan covers the scrutinee,
  arm bodies, and the else-arm, but NOT `arm.pattern`'s `Value(Expr)` variant — a tracked array
  used only inside a match-pattern value expression would miss an escape classification.
- **WHY** — Contrived and currently unreachable in any of this milestone's fixtures (requires an
  array both hot-looped elsewhere AND used as a literal match-pattern value, a combination no real
  Yinz code in this repo exercises); fixing it now with no reproducing fixture would be
  speculative hardening, not a confirmed bug fix.
- **COST** — Small — add one more expr-scan call on `arm.pattern`'s `Value` variant inside the
  existing `Match` handling in `scan_stmt`/`scan_expr`; under an hour once a concrete repro
  exists.
- **TRIGGER** — A real fixture (in this milestone's later phases, or any future SoA-adjacent work)
  is found to construct exactly this shape (a tracked array referenced only via a match-pattern
  value expression), OR Phase 8's suppression-enumeration mandate sweeping `examples/` + test
  fixtures surfaces a shape matching this pattern.

## 2026-07-04 — Deferral: fixed<T>.copy() remains a pre-existing alias no-op (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#5 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5: crates-ynz-codegen-src-emit-rs-17743
Filed-by-session: phase5-executor-2026-07-03-m5-closing

- **WHAT** — `crates/ynz-codegen/src/emit.rs:17743`: `fixed<T>.copy()` still lowers as an alias
  no-op — the same M4-era `lower_postfix_op` Copy-arm catch-all (`_ => Ok(recv_val)`) that
  FRAGO 014 fixed for `array<T>` (the new `Type::BuiltinArray` arm at :17705), but `fixed<T>`
  was explicitly outside that fix's scope, so a fixed-array receiver falls through to the
  alias-returning catch-all.
- **WHY** — FRAGO 014 was scoped narrowly to `array<T>` (Phase 5's own SoA/E9 domain);
  `fixed<T>` was never in Phase 5's scope and fixing it there would have been scope creep into
  an already-large (scale=large, 5-segment) phase.
- **COST** — Likely small: Phase 2 already found and rerouted the three `fixed<T>` element
  write choke points during its ABI migration, so extending `.copy()` there mirrors an
  already-established pattern — probably a similar shallow one-level-memcpy fix to what
  FRAGO 014 did for arrays.
- **TRIGGER** — The next phase/milestone that touches `fixed<T>` semantics, or a user-facing
  bug report of `fixed<T>.copy()` aliasing unexpectedly.

## 2026-07-04 — Deferral: SoA bounds-check predicate is a second (currently-equivalent) derivation of the runtime's bounds check (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#5 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#5: crates-ynz-codegen-src-emit-rs-2445
Filed-by-session: phase5-executor-2026-07-03-m5-closing

- **WHAT** — `crates/ynz-codegen/src/emit.rs:2445` (SoA gather, `idx u< cap`) and its sibling
  at the analogous scatter site (:2525): the SoA gather/scatter bounds predicate is logically
  equivalent to but textually separate from the runtime's own `idx < 0 || idx >= len` check
  (`crates/ynz-runtime/src/lib.rs:1239`, `ynz_array_get`) — linked today only by
  `soa_gather_into`'s doc comment, not a compile-time assertion or shared constant, per this
  milestone's `authoritative-derivation.md` house rule preferring one authoritative source
  over a documented-equivalent duplicate.
- **WHY** — Today the two predicates are provably equivalent (D3's admission invariant
  guarantees `len == cap` for every SoA-admitted array, and unsigned `u<` vs signed
  `< 0 || >=` are equivalent under the non-negative-length invariant) — fixing this now would
  be gold-plating a currently-zero-risk duplicate for a house-style preference, not a
  correctness necessity.
- **COST** — Small: a compile-time `debug_assert_eq!`-style link between the two predicates'
  logic, or extracting a shared helper/constant both codegen and the runtime check can point
  to.
- **TRIGGER** — Any future change to `len`/`cap`'s relationship (e.g. a future milestone
  allowing growable SoA arrays, breaking D3's `len == cap` invariant) — exactly the condition
  under which this duplicate would silently drift, per this milestone's own
  authoritative-derivation risk class (E3's whole reason for existing).

## 2026-07-04 — Deferral: construction-cost confound in the SoA calibration harness (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#6 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#6: crates-ynz-driver-benches-soa-calibration-rs-159
Filed-by-session: phase6-fixround1-executor-2026-07-04-m5

- **WHAT** — the Phase 6 calibration harness
  (`crates/ynz-driver/benches/soa_calibration.rs`) times the entire process (array
  construction + hot loop + print) with `TOTAL_VISITS` held fixed across N, so O(N)
  construction cost is a growing fraction of the measured signal as N increases, and SoA's
  segmented-scatter construction vs. AoS's contiguous-push construction differ in a way the
  harness doesn't isolate from the access-pattern delta it claims to measure; the `overhead`
  baseline (soa_calibration.rs:159, the reps=0 spawn-only group) is measured once at N=8 and
  subtracted as a flat scalar across all N.
- **WHY DEFERRED** — fixing this requires a harness redesign (per-N reps=0 baseline, or an
  isolated construction-only benchmark) — real work, and the current SIZE_THRESHOLD=64 ships
  unchanged regardless (no precision claim rests on this), so it doesn't block Phase 6's
  honest "no crossover" conclusion today.
- **COST** — ~0.5 session (redesign the baseline-subtraction methodology, re-run the sweep).
- **TRIGGER** — before this harness is used as the authoritative input for a REAL
  SIZE_THRESHOLD recalibration (FR#2's trigger — a dedicated perf environment becomes
  available, or a user-reported regression implicates the threshold).

## 2026-07-04 — Deferral: noise-floor regime mismatch + single-sweep-invocation rigor in the SoA calibration evidence (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#6 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#6: crates-ynz-driver-benches-soa-threshold-raw-2026-07-04-md-24
Filed-by-session: phase6-fixround1-executor-2026-07-04-m5

- **WHAT** — the ~15% noise floor cited as the credibility bar
  (`crates/ynz-driver/benches/soa-threshold-raw-2026-07-04.md:24`) was derived from S3's
  long-in-process-reps regime, then applied to the calibration sweep's short
  subprocess-spawn-timed "net" values (where subtracting two noisy quantities compounds
  variance rather than reducing it); the full 10-point x 2-mode calibration sweep was run
  exactly once, not repeated as an independent process invocation the way S3's own protocol
  was (which showed up to 50% cross-invocation drift on this shared host); reported values
  are bare medians with no confidence intervals.
- **WHY DEFERRED** — the shipped decision (SIZE_THRESHOLD=64, unchanged) is robust to this
  gap either way — no new precision was manufactured from the noisy data, and the honest
  hedge already exists in the plan text. Full statistical validation is a bigger lift than
  this boundary-review fix-loop should absorb.
- **COST** — ~0.5-1 session (re-run the full sweep 1-2 more times as independent process
  invocations, report criterion's confidence intervals, re-derive an in-regime noise floor).
- **TRIGGER** — before Phase 7/8 user-facing text (lint hover, CHANGELOG) asserts "no
  detectable benefit... direction uniform" as settled fact rather than a hedged observation —
  this is ALREADY gated by E14's existing "docs-consistency review before any Phase 7/8
  user-facing text ships" mitigation; this deferral just makes sure that gate specifically
  checks this point. Also: if a dedicated perf environment ever becomes available (FR#2's
  trigger).

## 2026-07-04 — Deferral: pre-existing clippy debt in integration-test binaries, outside CI's own scanned scope (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#7 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#7: crates-ynz-diagnostics-tests-jargon-audit-rs-7
- **WHAT** — 9 real `cargo clippy --tests -- -D warnings` violations exist across
  `crates/ynz-diagnostics/tests/jargon_audit.rs` (redundant single-component import; a
  `map_or(false, ...)` that should be `is_some_and(...)`; a `while let Some(...) = iter.next()`
  that should be a `for` loop; three implicit-saturating-subtraction sites; three dead/unused
  assignments to `site_count`), `crates/ynz-registry/tests/consistency.rs` (redundant
  single-component import), and `crates/ynz-runtime/tests/m2_runtime.rs` +
  `crates/ynz-runtime/src/lib.rs` (a duplicated `#[repr(C)]` attribute; a dead assignment). Verified
  via `git stash` against clean `HEAD` (`e94a2a3`, the Phase 6 boundary): the identical 9 warnings
  reproduce, confirming none of this milestone's Phase 6/7 work introduced them.
- **WHY DEFERRED** — this repo's own documented CI (`.github/workflows/ci.yml`) runs
  `cargo clippy --workspace -- -D warnings` with NO `--tests` flag, so it has never scanned these
  separate integration-test binaries under `tests/*.rs` — this debt sits entirely outside the
  project's own actual acceptance bar, and cleaning up pre-existing lint debt across three unrelated
  crates is out of M5's array-storage/SoA charter.
  discovered incidentally while boundary-reviewing Phase 7 (a broader `--tests`-scope gate run
  surfaced it), not caused by anything this milestone touched.
- **COST** — ~0.5 session (mechanical clippy auto-fixes for most of the 9; the two dead-assignment
  and duplicated-attribute findings need a one-line human judgment call each on intent).
- **TRIGGER** — the next milestone that touches any of `ynz-diagnostics`, `ynz-registry`, or
  `ynz-runtime`'s test suites, OR if this repo's CI convention is ever widened to include `--tests`
  in its clippy invocation (at which point this debt would newly redden CI and must be cleared
  first).

## 2026-07-04 — Deferral: codegen ICE — bare int literal into ANY `number`-typed slot crashes the compiler (non-blocking — deferred by 2026-07-03-v0-3-m5-auto-soa#7 at the phase boundary)
Idempotency-Key: 2026-07-03-v0-3-m5-auto-soa#7: crates-ynz-codegen-src-emit-rs-14101
Filed-by-session: plan-fixup-icedefer-2026-07-04-m5

> **ELEVATED priority.** This is a genuine user-facing compiler CRASH (panic/ICE, not a diagnostic)
> reachable via what may be the single most common beginner mistake in the language — a bare int
> literal where a `number`-typed slot is expected (`let x: number = 5`). "Non-blocking" in the
> heading means non-blocking for M5's phase boundary only; it is NOT routine cleanup debt.

- **WHAT** — Systemic codegen bug, broader than the shape-literal→array path it was first observed
  on: an int literal assigned into ANY `number`-typed slot panics the compiler. Root cause is a
  representation mismatch between the literal lowering and the store paths:
  - `crates/ynz-codegen/src/emit.rs:14101` — `Expr::IntLit` unconditionally lowers to a raw `i64`
    `IntValue`, with no branch on the expected type. Contrast `Expr::NumberLit` at
    `emit.rs:14103-14136`, which correctly builds an alloca + stores decimal128 bits, returning a
    pointer.
  - `crates/ynz-codegen/src/emit.rs:19674-19679` (`store_field`, fired by ANY struct literal or
    field assignment) AND the identical pattern in the plain `store()` function at
    `emit.rs:19552-19557` (fired for `let`/`const` bindings generally) — both unconditionally call
    `.into_pointer_value()` on the `Type::Number{precision<=34}` arm, assuming the value is always
    a pointer-to-i128.
  - `crates/ynz-typeck/src/check.rs:2162-2166` — typeck ADMITS the coercion (retypes `Expr::IntLit`
    to `Type::Number` at the type level, no AST rewrite), so the program type-checks cleanly and
    then codegen panics — inkwell's message is exactly "Found IntValue … expected the PointerValue
    variant".
  - Blast radius: `let x: number = 5` (no shape, no array) crashes; any struct literal or field
    assignment with an int literal into a `number` field crashes. Confirmed PRE-EXISTING and
    orthogonal to M5 (M5 Phase 2 only added Shape/Maybe arms to `store_field`; the `Type::Number`
    arm is untouched legacy code from before this milestone). Confirmed no existing fixture or
    example anywhere in the repo exercises a bare int literal into a `number`-typed slot — every
    existing usage uses a decimal literal (e.g. `1234567.89`) — which is exactly why it went
    undiscovered until Phase 7's doc-writing incidentally tried it. Independently confirmed at
    source level by a deviation-judge dispatch (not just executor narration).
- **WHY** — Out of M5's array-storage/SoA charter: the bug lives in untouched legacy numeric-literal
  codegen, unrelated to array/SoA representation; fixing a systemic pre-existing literal-lowering
  bug inside M5's Phase 7 (docs) fix round would be unreviewed scope creep into codegen at the
  milestone's boundary.
- **COST** — ~0.5-1 session. The fix is likely either (a) an expected-type-aware `Expr::IntLit`
  branch mirroring `NumberLit`'s alloca-and-store pattern at `emit.rs:14103-14136`, or (b) coercing
  the int literal to a number literal at typeck's retype point (`check.rs:2162-2166`) so codegen
  never sees a raw int for a `number` slot — either approach needs its own small design pass + the
  same E7-style call-site audit rigor this milestone used for arrays (every consumer of the lowered
  value must agree on the representation).
- **TRIGGER** — The next milestone touching numeric-literal codegen, OR **immediately** if a real
  user hits this crash, OR the next time someone adds a `.ynz` example/fixture using a bare int
  literal for a `number` field (which will immediately hit it — several pre-existing
  `REF-collections` examples, e.g. `Position { x: number }` built with `{ x: 0, y: 0 }` int
  literals, would already ICE if run today).

## 2026-07-04 — Session log: roadmap Capability Ledger fixup — two new unscoped rows + Patrick's triage policy applied to all six (standalone roadmap housekeeping, Patrick-requested)
Filed-by-session: roadmap-fixup-triage-2026-07-04

Standalone roadmap-only edit, explicitly requested by Patrick — NOT part of any plan's phases (M5
plan `2026-07-03-v0-3-m5-auto-soa` is complete and awaiting his completion approval; its plan.md /
audit.md are untouched). Changes, all in `roadmap.md`, applied identically to BOTH Capability
Ledger tables:

- **Promoted M5 plan Future Requirements #15 to a ledger row** (selective hot-field-only element
  materialization, FRAGO 020): Phase 5's SoA codegen computes `hot_fields` via `soa_candidate_query`
  but `soa_gather_into`/`array_elem_get_into` (`crates/ynz-codegen/src/emit.rs`) never consume it —
  every field gathered unconditionally. FR#15's own text stays in place in the M5 plan (cross-
  referenced, not deleted); the new row is the roadmap-level anchor. Status: unscoped → needs a
  milestone.
- **Added a NEW capability-discovery row: no LLVM optimization pass pipeline exists at all.**
  Grep-verified this session: `OptimizationLevel::None` is hardcoded at both TargetMachine creation
  sites (`crates/ynz-codegen/src/emit.rs:879`, `crates/ynz-codegen/src/state_machine.rs:755`) and
  those are the ONLY optimization-level configuration points in the entire codegen crate — a single,
  global, compiler-wide setting, NOT array/SoA-specific. Every emitted code path (arrays, shapes,
  the concurrency state-machine engine in `state_machine.rs`, channels, Arc ops) compiles with zero
  LLVM passes (no inlining, no DCE, no SROA, no mem2reg). Surfaced via M5 Phase 6's SoA calibration
  (which measured only the SoA-specific consequence); the finding itself is compiler-wide. Status:
  unscoped → needs a milestone; flagged HIGH STRATEGIC VALUE per Patrick's flagship-concurrency note.
- **Recorded Patrick's triage policy (2026-07-04)** as a preface blockquote above BOTH tables — "a
  REAL BUG/crash/leak/security-risk gets prioritized as the next fix; anything else (missing
  feature, perf-only gap, process tooling) is fine to defer until after the v1.0 release" — and
  classified ALL SIX unscoped rows inline in their Notes columns:
  - Int-literal-into-`number` ICE → BUG (crash on common valid code) → next-fix priority.
  - O0 stack-exhaustion SIGSEGV ceiling → BUG (crash on any big-enough hot loop, not SoA-specific)
    → next-fix priority.
  - Stale-runtime-archive footgun → BUG (silent miscompile in build/release tooling) → next-fix
    priority, lower urgency (precondition-gated, not everyday code).
  - Authoritative-derivation write-time hook → NOT a bug (process/tooling) → fine post-v1.0.
  - FR#15 hot_fields unused → NOT a bug (perf-only gap) → fine post-v1.0.
  - LLVM optimization pipeline → NOT technically a bug (missing capability, perf-only) → fine
    post-v1.0 per the rule's letter, BUT flagged in the row's own text as the single most
    strategically important item on the list (Rust-level-performance positioning, Golden Rules
    4/8/10, concurrency-as-flagship) — Patrick's call whether to treat it specially.

No code touched. Nothing committed. Session-id `roadmap-fixup-triage-2026-07-04` appended to the
roadmap's frontmatter chain in the same action as this entry.

## 2026-07-04 — Session log: row 441 (int-literal-into-`number` ICE) assigned its own hotfix plan-id (Gate-4 closing action, Patrick-requested)
Filed-by-session: gate4-signatures-2026-07-04

Standalone roadmap-only edit, one of three closing actions from Patrick's Gate-4 approval of the
v0.3-M6/M7/M8 sibling concurrency plans — NOT part of any of those three plans' own phases. Changes,
in `roadmap.md`, applied identically to BOTH Capability Ledger tables (lines ~441 and ~495):

- Row 441's Status cell changed from "unscoped → ELEVATED — needs its own small hotfix slot (Patrick
  to assign)" to "assigned → plan-id `2026-07-04-v0-3-hotfix-int-literal-number` (NEEDS-EXECUTION/stub
  status)". Row's Notes column appended with an **ASSIGNED 2026-07-04** clause naming the new plan-id
  and its stub/independent-sequencing status.
- Owning-milestone cell left as-is ("unscoped — owning milestone TBD…") — the new plan is a
  standalone hotfix under this roadmap, not itself a numbered milestone; the Status cell's plan-id is
  the authoritative assignment pointer, per the plan-format's by-id linking rule (never by path).
- New plan created at `2026-07-04-v0-3-hotfix-int-literal-number` (WARNO stub, `roadmap-id:
  "2026-05-21-v0-3-concurrency-perf"`) — see that plan's own `plan.md`/`audit.md` for its Mission,
  Situation, and open questions, drawn directly from this row's own text and the M5 plan's Future
  Requirements #7 deferral (also recorded above, "Deferral: codegen ICE — bare int literal into ANY
  `number`-typed slot crashes the compiler").

No code touched. Nothing committed. Session-id `gate4-signatures-2026-07-04` appended to the
roadmap's frontmatter chain in the same action as this entry.

## 2026-07-10 — Session log: two v0.3-M6 decimal128 defects (Future-Req #18/#19) surfaced to the Capability Ledger to survive M6 archival (DOC-ONLY cross-plan coordination)
Filed-by-session: m6-p1d-crossplan-coord-2026-07-10

Standalone roadmap-only edit, DOC-ONLY (no code, no compiler files) — a cross-plan coordination pass
recording two decimal128 defects that v0.3-M6 (`2026-07-04-v0-3-m6-concurrency-hotfix`) Phase 1d
surfaced as formal four-field Future-Reqs #18/#19 (FRAGO 019). They are NOT int-literal→`number`
class (that class is row 441's hotfix stub) — they are separate decimal128 ABI/hashing defects — and
they would go invisible when M6 archives to `done/`, so they get durable roadmap-level anchors here.
Changes, in `roadmap.md`, applied identically to BOTH Capability Ledger tables (after the
int-literal-into-`number` row in each):

- **decimal128 by-value RETURN ABI defect** — a synchronous `-> number` fn returning a valid decimal
  literal prints nondeterministic pointer garbage (stack-dangling decimal128 returned by value; a
  function-return ABI defect, orthogonal to FRAGO 009's concurrency-crossing charter; SILENT
  miscompile, exit 0). Cited M6 FRAGO 019 / plan Future-Req #18. Fix: return-slot ABI (sret / heap
  out-pointer), its own small design pass. Est/COST + TRIGGER transcribed from #18. Status: unscoped
  → needs a milestone. Triage: BUG.
- **`map<number, V>` real-number-key silent breakage** — decimal128 keys hash/compare by pointer
  identity, so equal literal keys never match (`m.set(1.5,v)` then `m.get(1.5)` → `none`, exit 0).
  Cited M6 FRAGO 019 / plan Future-Req #19. Fix: value-based decimal128 hashing + equality (hash the
  16-byte payload, compare by value). Est/COST + TRIGGER transcribed from #19. Status: unscoped →
  needs a milestone. Triage: BUG.

Both rows mirror the existing unscoped-bug row format (Capability | Owning milestone | Status | Notes,
with **Capability discovery …** + COST + TRIGGER + **Triage** in Notes), and were added to BOTH the
"SSOT" table (~line 423) and the merged pre-migration table (~line 479) so the two duplicate ledgers
stay in lockstep — table 1 with the bold-capability styling, table 2 plain, matching each table's own
convention. RECORD-ONLY: transcribing already-decided M6 Future-Reqs into their durable roadmap home,
no adjudication. M6's own plan.md / audit.md were NOT touched (another executor's territory this turn).
No code touched. Nothing committed. Session-id `m6-p1d-crossplan-coord-2026-07-10` appended to the
roadmap's frontmatter chain in the same action as this entry.

## 2026-07-10 — Session log: int-literal-into-`number` ledger row reconciled to the v0.3-M6 store-site stopgap (DOC-ONLY, both tables)
Filed-by-session: m6-storesite-stopgap-ledger-reconcile-2026-07-10

DOC-ONLY single-sentence reconciliation, both Capability Ledger tables' int-literal-into-`number` row (historically row 441 + its ~497 duplicate): the stale "typeck ADMITS the coercion … then the compiler panics" phrasing was rewritten to record that v0.3-M6's store-site stopgap (M6 FRAGO 020, commit `46906d1`) makes typeck REJECT the bare int literal at `number` slots (including store sites) with a teaching error — the ICE is no longer reachable — while the row's core point stands unchanged: the actual int→`number` coercion remains unimplemented and stays assigned to plan-id `2026-07-04-v0-3-hotfix-int-literal-number`. Ownership/status columns untouched; rows #18/#19 untouched. No code touched. Nothing committed (conductor seals). Session-id `m6-storesite-stopgap-ledger-reconcile-2026-07-10` appended to the roadmap's frontmatter chain in the same action as this entry.

## 2026-07-11 — Ledger amendment: general union-narrowing payload-extraction defect class (v0.3-M6 Future-Req #24) lifted to the Capability Ledger to survive M6 archival (DOC-ONLY cross-plan coordination)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#24: union-narrowing-payload-extraction
Filed-by-session: m6-fr24-crossplan-lift-2026-07-11

Standalone roadmap-only edit, DOC-ONLY (no code, no compiler files) — a cross-plan coordination pass
lifting the four-field deferral that v0.3-M6 (`2026-07-04-v0-3-m6-concurrency-hotfix`) homed as
Future-Req #24 at its Phase 3c polish round (per the deviation-judge should-fix, both surfaces probed
live 2026-07-10 during the FRAGO 026 round). It currently lives ONLY in that plan's Future
Requirements and would go invisible when M6 archives to `done/`, so it gets a durable roadmap-level
anchor here: this payload entry plus a pointer row in BOTH Capability Ledger tables (after the two
M6 decimal128 rows in each — table 1 with the bold-capability styling, table 2 plain, matching each
table's own convention), status **unscoped → needs a milestone**.

> **Orthogonality callout — do NOT mis-triage as spawn/concurrency cleanup.** Both surfaces below
> reproduce with NO `background`/spawn anywhere: pre-existing GENERAL union-narrowing
> memory-safety/correctness defects, NOT concurrency defects, out of M6's concurrency-race/leak/
> honesty charter (same non-absorption shape as M6 Future-Reqs #18/#19/#20). The
> concurrency-reachable face of the same root (the narrowed `background` receiver) is already
> fail-closed rejected by M6's FRAGO 026 teaching error and stays with M6 FR #21.

Four fields, carried faithfully from M6 plan FR #24:

- **WHAT (two surfaces, one root)** — narrowing a union to a shape variant does NOT extract the
  variant payload; the narrowed value is still the 16-byte `{tag,data}` union envelope.
  (a) direct field access on a narrowed union binding is SILENTLY WRONG — `if (fig is Circle) {
  print(fig.radius) }` prints `0` for `5.0`, exit 0: the field read is lowered against the union's
  `{tag,data}` storage, not the variant's payload (a Golden Rule 5 silent-wrong correctness bug);
  (b) re-binding a narrowed union value to a shape-typed binding is a MEMORY-SAFETY bug (CWE-125,
  security-reproduced as a SIGSEGV) — `let inner: Circle = fig` inside the `is Circle` arm copies
  the 16-byte union envelope into a shape-sized binding, and a subsequent pointer-field read (or
  `background inner.haul()`) reads out of bounds; this is the union→shape assignment-lowering face
  of the same root, and it is why the FRAGO 026 teaching error's WHAT-INSTEAD explicitly warns
  AGAINST the re-bind.
- **WHY deferred (out of M6's charter)** — union-payload extraction is general typeck/codegen
  lowering work with no concurrency dimension; M6's charter is concurrency races/leaks/honesty.
  The concurrency-reachable face of the same root (the narrowed `background` receiver) is already
  fail-closed rejected (FRAGO 026), and the teaching text steers users away from the (b) re-bind —
  but (a) and the (b) re-bind themselves remain reachable in plain non-concurrent code today
  (silent-wrong / SIGSEGV, no guard); a cheap interim fail-closed rejection of the union→shape
  re-bind (mirroring FRAGO 026's precedent) is a CANDIDATE for whoever owns this, surfaced at the
  Phase 3c polish round, not self-decided there.
- **COST to fix later** — the same `union_to_heap_cell`-based payload-extraction machinery as M6
  FR #21 (`crates/ynz-codegen/src/emit.rs:3248` already does exactly this extraction for the
  let-bound union arg-escape case and is the reuse target) — ONE design pass closes FR #21 and both
  surfaces here: (a) needs narrowed field-access lowering to resolve the payload (not the
  envelope); (b) needs union→shape assignment lowering to extract the payload (or reject the
  re-bind) rather than envelope-copy.
- **TRIGGER** — the milestone that owns union-payload extraction (land together with M6 FR #21),
  OR a user hits the silent-wrong narrowed field read / the re-bind SIGSEGV in the wild.

RECORD-ONLY: transcribing an already-decided M6 Future-Req into its durable roadmap home, no
adjudication. The `Idempotency-Key:` line above is the re-run sentinel — a later M6 Phase-8
deferral lift that finds it present must NOT re-append this payload or duplicate the ledger rows.
M6's own plan.md FR #24 received a one-line lifted-to-roadmap cross-reference in the same action
(recorded in the M6 audit.md Session log); FR #24 was not otherwise restructured. No code touched.
Nothing committed (conductor seals). Session-id `m6-fr24-crossplan-lift-2026-07-11` appended to the
roadmap's frontmatter chain in the same action as this entry.

## 2026-07-11 — Deferral: wake_recv_waiters drain-then-wake hardening (non-blocking — deferred by 2026-07-04-v0-3-m6-concurrency-hotfix#4 at the phase boundary)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#4: crates-ynz-runtime-src-channel-rs-208

- **WHAT** — harden `wake_recv_waiters` (crates/ynz-runtime/src/channel.rs) to drain recorded wakers into a local Vec, drop the `recv_waiters` guard, THEN call `.wake()` on each — removing the call-while-locked pattern.
- **WHY** — currently safe only under the assumption that every forwarded waker is a Tokio task waker whose `wake()` never synchronously re-enters; a drain-then-wake-outside-lock removes that latent coupling entirely, but is not a live bug today (no reentrant waker is ever installed in this runtime) and is out of Phase 4's narrow P3-2 scope (register-before-poll only).
- **COST** — small: one extra short-lived Vec allocation per drain call (an already-uncommon, small-N path — typically 1-2 waiters).
- **TRIGGER** — if this runtime ever accepts a caller-supplied or otherwise non-Tokio waker implementation (breaking the "wake() never re-enters" assumption), or if Phase 6b's sanitizer lane (Miri/ThreadSanitizer) flags this call-under-lock pattern.

## 2026-07-11 — Deferral: shared RecvWaiterRegistry extraction (non-blocking — deferred by 2026-07-04-v0-3-m6-concurrency-hotfix#4b at the phase boundary)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#4b: crates-ynz-runtime-src-handle-rs-123

- **WHAT** — extract the byte-identical `record_recv_waiter`/`wake_recv_waiters` methods + `recv_waiters: Mutex<Vec<Waker>>` field, currently duplicated between `YnzChannel` (crates/ynz-runtime/src/channel.rs) and `HandleShared` (crates/ynz-runtime/src/handle.rs), into a single shared `RecvWaiterRegistry` type embedded by both.
- **WHY** — two concrete, identical consumers exist today (not a speculative future) — the reusability gate-2 trigger is met per code-reviewer's Phase 4b review. Not extracted in Phase 4b itself because doing so would require modifying `channel.rs`, which is Phase 4's already-sealed, reviewed, and committed code (commit 42cd38a) — reaching into another phase's sealed diff mid-hotfix is exactly the cross-phase scope creep this plan's discipline avoids (deviation-judge validated leaving it as-is this phase as correct restraint, not omission).
- **COST** — small, mechanical: define `struct RecvWaiterRegistry { inner: Mutex<Vec<Waker>> }` with `record()`/`wake_all()` methods (the existing bodies, unchanged logic), embed it as a field in both `YnzChannel` and `HandleShared` replacing their direct `Mutex<Vec<Waker>>` fields, update both structs' 4 call sites (2 per struct) to the new method names if they differ. No behavior change, no new tests needed (existing tests for both already cover the registry behavior) — under half a session.
- **TRIGGER** — the next phase or plan that touches either `channel.rs`'s or `handle.rs`'s waiter-registry code for any other reason (natural opportunity to consolidate in the same diff), OR if a third identical registry-shaped duplication appears anywhere in ynz-runtime (three instances of the same pattern is a much stronger reuse signal than two).

## 2026-07-11 — Deferral: extract shared glue-invocation loop (non-blocking — deferred by 2026-07-04-v0-3-m6-concurrency-hotfix#5 at the phase boundary)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#5: crates-ynz-runtime-src-channel-rs-657

- **WHAT** — extract the duplicated `if let Some(glue) = chan.drop_glue { for bits in <collected> { unsafe { glue(bits) }; } }` loop (currently present verbatim at both `purge_pending_sends` and the insert-time stale sweep in `channel_send_poll_guarded`, crates/ynz-runtime/src/channel.rs) into a single shared helper, e.g. `fn glue_each(glue: Option<unsafe extern "C" fn(i64)>, bits: impl IntoIterator<Item = i64>)`.
- **WHY** — reusability gate-2 is technically met (2 identical-shape consumers), but the duplication is a 4-line, trivially-correct loop with distinct per-site SAFETY comments explaining each site's own disjointness argument — extracting now, in an already-heavily-reviewed phase, would touch reviewed code for marginal benefit rather than fixing a real defect.
- **COST** — trivial: define one 3-line helper fn, replace both call sites with a single call each. Under 10 minutes.
- **TRIGGER** — a third glue-invocation site appears anywhere in ynz-runtime (three instances of the same pattern is a much stronger reuse signal than two), or the next phase/session that touches either of these two functions for any other reason (natural opportunity to fold in the extraction in the same diff).

## 2026-07-11 — Deferral: sibling type-walker in ynz-typeck (non-blocking — deferred by 2026-07-04-v0-3-m6-concurrency-hotfix#5b at the phase boundary)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#5b: crates-ynz-typeck-src-check-rs-7285

- **WHAT** — confirm whether `find_crossing_local_typeck_type_in_map`/`find_crossing_local_typeck_type_in_stmts` (crates/ynz-typeck/src/check.rs:7279-7285) is an intentional separate-lifecycle-stage traversal (typeck's own Check-2 pass, distinct from codegen's now-unified `find_let_typeck_type_in_stmts`) or an accidental second derivation of the same "type of this crossing local" question that authoritative-derivation.md would want unified/linked.
- **WHY** — out of Phase 5b's committed scope (the P1-2 audit finding and this phase's CCIR-1 recon named only the emit.rs pair); expanding scope mid-phase to investigate a different crate's traversal was not warranted for zero added correctness gain to THIS phase's exit criteria.
- **COST** — a recon-sized investigation (read both traversals, confirm lifecycle-stage separation or file a follow-up unification phase) — well under one session.
- **TRIGGER** — the next time authoritative-derivation.md-class hardening work touches ynz-typeck, or a bug is found where these two "crossing local type" answers disagree.

## 2026-07-16 — Deferral: registry triggers-field schema drift (non-blocking — deferred by 2026-07-04-v0-3-m6-concurrency-hotfix#7 at the phase boundary)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#7: registry-features-toml-1330

- **WHAT** — the `triggers` field on several `[[deferred_language_feature]]`/`[[deferred_tooling_feature]]`
  registry entries (both pre-existing: auto-arc-codegen-emission, background-handle-cancel-injection,
  seq-cst-ordering-opt-in; and new from M6 Phase 7: background.cpuBound, cooperative-preemption-back-edge-yield)
  describes a runtime/measurement event rather than user-typeable trigger code, diverging from the
  documented schema in IMP-feature-registry.md:202.
- **WHY** — reconciling this is a cross-cutting registry-schema question spanning entries this phase
  didn't author; fixing it here would expand Phase 7's docs/registry-only charter into a schema audit
  of the whole registry file, which is out of scope for a single phase.
- **COST** — a dedicated small pass: either (a) relax IMP-feature-registry.md's documented schema to
  explicitly allow a measurement-event trigger for deferred features gated on real-world evidence
  rather than user syntax, or (b) rewrite the drifted entries' triggers fields to be genuinely
  user-typeable where that's actually possible. Estimate: <1 session.
- **TRIGGER** — the next time someone designs a new deferred_* registry entry and has to decide which
  triggers convention to follow, or a registry/schema-focused milestone/plan picks up general registry
  hygiene.

## 2026-07-16 — Ledger amendment: v0.3-M6 Phase 8 durable-home deferral lift (FRAGO 012, amended by FRAGO 015) — every surviving M6 Future-Requirements deferral lifted to the roadmap's durable store

This is the Phase-8-mandated lift of every surviving deferral from
`2026-07-04-v0-3-m6-concurrency-hotfix`'s own Future Requirements section into this roadmap's durable
store, per FRAGO 012 (amended by FRAGO 015) — a REVIEWED deliverable, not a promise. Items already
lifted by earlier sessions (Future-Reqs #7 registry triggers-field schema drift, and #24
union-narrowing payload extraction) are confirmed present above (Idempotency-Keys
`2026-07-04-v0-3-m6-concurrency-hotfix#7: registry-features-toml-1330` and
`2026-07-04-v0-3-m6-concurrency-hotfix#24: union-narrowing-payload-extraction`) and are NOT
re-duplicated here. Items already resolved in-plan (Future-Reqs #2 P1-2 twin type-walkers, #3 P2-5
recursion-chain leak — both fixed, no longer deferrals) and Future-Req #8 (Capability Ledger section
duplication — a documented, out-of-charter doc-hygiene note, not a bug/deferral requiring a durable
four-field lift) are correctly NOT lifted. The ten entries below cover every remaining item: #1, #4,
#5, #6, #9 (roadmap-ledger row 441)+#14 (paired), #10, #11, #12, #13+#17 (paired), #15+#16 (paired).
Each carries its own Idempotency-Key sentinel; a pointer row for each lands in BOTH Capability Ledger
tables (added in the same action as this entry). RECORD-ONLY — transcribing already-decided M6
Future-Reqs into their durable roadmap home, no adjudication.

### Deferral: P2-3 — closed-send drop-glue leak (owner-tagged M8)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr1: p2-3-closed-send-drop-glue

- **WHAT** — `crates/ynz-codegen/src/emit.rs:~11833-11960`'s closed1/closed2 blocks drop no
  `value_bits` — a channel-close drop-glue leak.
- **WHY** — structurally unreachable in production until channel-close semantics ship (a bare
  channel never closes today, per M6's own Terrain finding P2-1).
- **COST** — small once channel-close semantics land — reuses M6 Phase 5's drop-glue fn-ptr
  mechanism directly.
- **TRIGGER** — channel-close semantics ship (see the channel-close entry below — both land
  together in v0.3-M8, per that milestone's own scope).

### Deferral: bare-channel end-of-stream / channel-close semantics (owner-tagged M8)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr4: channel-close-end-of-stream

- **WHAT** — the bare-channel end-of-stream/close design gap (M6's own Terrain P2-1): the footgun is
  documented loudly (M6 Phase 7), but the FEATURE (what `.close()` looks like; does last-sender-drop
  auto-close) is not designed. A sibling facet independently surfaced during M6 Phase 4: a receiver
  observing `Ready(None)` wakes no recorded co-waiter whose mpsc single-slot registration was
  clobbered — confirmed presently LATENT (every close-simulation in `channel.rs` is
  `#[cfg(test)]`-only; no production path closes a channel while a receiver survives).
- **WHY** — a real design question (what `.close()` looks like; last-sender-drop auto-close
  semantics given the channel object itself holds a Sender) — out of M6's hotfix scope by the
  brief's explicit disposition. A first-pass piecemeal fix for the wake-propagation facet was
  REVERTED per deviation-judge review as landing inside this entry's deferred territory.
- **COST** — unknown — needs its own design pass before a cost estimate is honest.
- **TRIGGER** — a real user/workload needs bounded-lifetime channel consumption, or before any
  production-representative concurrency use case ships. Both land together in v0.3-M8 per that
  milestone's own scope (channel-close/end-of-stream semantics, design-first).

### Deferral: cooperative preemption — real back-edge yield (owner-tagged M7)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr5: cooperative-preemption-back-edge-yield

- **WHAT** — registry entry `cooperative-preemption-back-edge-yield`: today's `ynz_rt_check_preempt()`
  back-edge calls (`emit.rs:12356-12365`) invoke a documented no-op stub (`runtime.rs:281-299`,
  no-op body at :296-299) rather than a real yield.
- **WHY** — a 1190% O0 call-site cost was measured (M5 spike) with no offsetting benefit until the
  optimizer pipeline exists to bring that cost down.
- **COST** — implementation-sized, folded into M7's own scope.
- **TRIGGER** — M7's optimizer pipeline lands and the cost is re-measured under real LLVM passes.

### Deferral: `background.cpuBound` explicit override (unscoped)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr6: background-cpubound-override

- **WHAT** — an explicit override letting a user force CPU-bound routing for a `background` call,
  registry-tracked per `IMP-no-function-coloring.md:247`'s spec-but-unimplemented state.
- **WHY** — no real workload has yet demonstrated the auto-inference gets CPU-bound routing wrong;
  building an unused override is speculative.
- **COST** — small (naming + one typeck/codegen surface once a real need is named).
- **TRIGGER** — a real workload where auto-inference misroutes a CPU-bound task, causing measurable
  starvation.

### Deferral: int→`number` coercion — store-site + call-argument-site (paired, roadmap-ledger row 441, flagged for Patrick's Gate-4 home call)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr9-fr14: int-literal-number-coercion-store-and-call-site

- **WHAT** — two facets of ONE missing int→`number` coercion mechanism, both rooted in
  `Expr::IntLit` lowering to a raw `i64` while `Type::Number` slots expect a decimal128 pointer: (a)
  the STORE-site facet (roadmap-ledger row 441, M6 Future-Req #9) — `let x: number = 5` and the
  structurally identical `hidden f: number = 5` shape-field default both formerly ICE'd
  (`Found IntValue(i64 5) but expected PointerValue variant` at `store`/`store_field`, e.g.
  `emit.rs:20351`/`emit.rs:20436`) because typeck admitted the coercion while codegen could not
  represent it; (b) the CALL-ARGUMENT-site facet (M6 Future-Req #14, FRAGO 016) — a bare int literal
  passed as a call argument to a `number`-typed parameter (`f(5)` where `f(n: number)`) type-checked
  then ICE'd at codegen (`emit.rs:14514` raw-i64 lowering vs. the no-coercion call-argument loop at
  `emit.rs:14986-14990`, LLVM verifier reject) — reachable via plain synchronous calls, UFCS dot-calls
  (`p.f(5)`), and generic-fn concrete `number` params. As of the v0.3-M6 store-site stopgap (FRAGO 020)
  and the FRAGO 018/019 call-argument-class guard, BOTH facets — plus every other IntLit/`-IntLit` →
  `number` slot (collection-element args, struct/array/fixed/map literals, index/field assignment,
  `return` including `errors`-wrapped returns, match-arm patterns) — now REJECT uniformly via the one
  `reject_int_literal_number_slot`/`reject_int_literal_number_arg` teaching-error gate instead of
  ICE'ing; the ICE exposure is closed, but the int→`number` COERCION itself (which would ACCEPT the
  literal instead of rejecting it) remains unimplemented.
- **WHY** — NOT a concurrency-audit finding — a pre-existing literal-lowering bug orthogonal to M6's
  confirmed concurrency-race/leak/honesty charter (mixing an unrelated ICE/coercion fix into a hotfix
  milestone widens its blast radius for no charter-aligned benefit); M7's own Future Requirements #3
  independently reached the same non-absorption verdict. Both facets are the SAME one-mechanism gap
  (authoritative-derivation: fix once, thread at both the store site and the call-argument site) and
  are recorded together rather than as two independent deferrals. Neither M6 nor M7 claims it, so it
  is named here rather than left to fall through the gap between the two — Patrick explicitly declined
  to have it silently auto-assigned to either milestone.
- **COST** — unchanged from the roadmap ledger's own estimate — ~0.5-1 session (expected-type-aware
  `Expr::IntLit` lowering, or typeck-level int→number coercion covering BOTH the store site and the
  call-argument site; its own small design + call-site audit).
- **TRIGGER** — Gate-4 conversation — Patrick assigns roadmap-ledger row 441 (and this paired
  call-argument facet) a home, rather than either being auto-claimed by M6 or M7; or a real user
  hitting the ICE on valid-looking code before that conversation happens. The fix is routed to the
  existing stub plan `2026-07-04-v0-3-hotfix-int-literal-number`, expanded to cover BOTH facets under
  ONE coercion mechanism — that cross-plan expansion is a conductor→human coordination item, not
  applied to the stub plan by this lift.

### Deferral: dynamic-dispatch × suspension predicate blindness (unscoped, grouped alongside row 441)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr10: dynamic-dispatch-suspension-predicate-blindness

- **WHAT** — `check.rs`'s `check_follows_contracts` never reads `suspends`; the four suspension
  predicates (`may_block.rs` call-graph, `cpu_admission.rs`, `emit.rs` `collect_callees_in_expr` +
  `is_direct_suspending_call`) are all MethodCall-blind for the vtable-resolved `dynamic Contract`
  form — the same shape as the UFCS gap M6 Phase 1 fixed for the static-dispatch form.
- **WHY** — every `dynamic Contract` call site hard-errors at codegen today
  (`emit.rs:14622-14625`, "not yet lowered in M4 P4") — zero live exposure (a loud compile error,
  never a silent mis-suspension), so no reachable test can exercise a fix; coding the predicate
  threading now is speculative work against dead code.
- **COST** — small — reuses M6 Phase 1's shared authoritative-resolution threading directly; should
  land in the SAME future phase that lowers `dynamic Contract` codegen, not as a separate follow-on.
- **TRIGGER** — `dynamic Contract` call-site codegen lowering ships (the remaining M4 P4 work —
  owning milestone TBD, flagged to Patrick at Gate-4 rather than left "someday").

### Deferral: pre-existing backend ICE on `fixed<T>` PARAM iteration (unscoped, grouped alongside row 441)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr11: fixed-t-param-iteration-ice

- **WHAT** — codegen ICEs ("cannot iterate fixed array with unknown size") when a function body
  iterates a `fixed<T>` received as a PARAMETER (size not statically known at the callee) —
  surfaced by M6 Phase 1b segment 1 while constructing the `fixed<T>` escape fixture.
- **WHY** — a different bug class entirely from the UAF/crossing class Phase 1b closed — a backend
  lowering ICE, orthogonal to M6's concurrency charter; a LOUD compile-time crash, never a silent
  miscompile.
- **COST** — ~0.5-1 session (thread the fixed-array size through the param ABI, or reject
  fixed-param iteration with a teaching error — needs its own small design pass).
- **TRIGGER** — the next milestone touching `fixed<T>` codegen/ABI, or a real user hitting the ICE
  on valid-looking code.

### Deferral: conduit-send decimal128 marshalling (unscoped)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr12: conduit-send-decimal128-marshalling

- **WHAT** — `emit.rs:11809`'s `ptr_to_int` of a stack temp sent as a raw i64 into `mpsc<i64>`; a
  receiver on another frame would reconstruct a pointer into the sender's dead resume-fn stack —
  the same decimal128-across-a-concurrency-boundary UAF shape M6 Phase 1d fixed for the
  background-spawn/cpu-member arg boundary, but at the channel-conduit boundary instead.
  VERIFIED-SAFE-BY-GATE today: `channel<number>` is compile-gated by typeck
  (`check.rs:3417-3451`) with a teaching error naming this exact UAF class, so this path is
  unreachable from any current syntax.
- **WHY** — fixing unreachable code now is speculative work against dead code.
- **COST** — small — M6 Phase 1d selected Option 2 (D8: eager i128 heap-copy), so this directly
  reuses the shipped `number_to_heap_cell` codegen helper for the value copy; it still needs its OWN
  send/recv conduit-marshalling design pass to remove the `check.rs:3417-3451` gate — Option 2 does
  NOT unlock `channel<number>` on its own (the conduit surface is a separate marshalling problem
  from the spawn-arg boundary).
- **TRIGGER** — `channel<number>`'s heap-copy machinery ships (removing the existing compile gate),
  or a real workload needs `channel<number>` to work rather than be rejected.

### Deferral: never-drop-locals class — per-iteration maybe/union heap-cell loop leak + trampoline staged decimal128 arg-cell leak (unscoped → needs the drop-story milestone)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr13-fr17: never-drop-locals-heap-cell-and-trampoline-leaks

- **WHAT** — two siblings in the same never-drop-locals class (M5 Future-Req #6): (a) a crossing
  maybe/union binding re-bound each loop iteration orphans its promoted heap cell(s) (1-2
  cells/iter), held to process exit, never freed — confirmed by exact-gap Paper-Trace proof
  (`v0_3_m6_heap_cell_loop_parity.ynz`, alloc=12/free=1, gap=11 = 5×1 maybe envelope + 3×2 union
  envelope+payload, predicted before first run, stable 4/4); (b) the cpu-member spawn site
  heap-allocates a 16-byte decimal128 arg cell and the trampoline frees it after result packing
  (one alloc/one free), but a blocking-pool task queued but dropped UN-RUN at runtime shutdown never
  executes its trampoline, so its one balancing free never runs — the staged cell leaks, held to
  process exit only (never a UAF, never a double-free).
- **WHY** — freeing either needs the ownership drop story, out of this hotfix's charter — maybe/union
  heap cells and staged decimal128 arg cells both join the existing never-drop-locals regime
  alongside string/array/map crossing locals.
- **COST** — the drop-story milestone (1-2 sessions) + updating the two parity pins for (a); small
  once the drop-story milestone lands for (b) — register the staged cell with the same drop
  mechanism and update the free sites (the trampoline free + the shutdown drop path must be
  exactly-once between them).
- **TRIGGER** — the drop story lands, OR a real unbounded-suspension-loop-over-maybe/union workload
  (a) / a real long-lived workload measurably accumulating un-run dropped blocking-pool tasks at
  shutdown (b). Pinned loud in-suite via
  `v03_m6_p1c_heap_cell_loop_parity_pins_documented_per_iteration_leak` for (a).

### Deferral: Phase 1d polish minors — twin-scan consolidation + named decimal128 cell-size const (unscoped, trivial)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr15-fr16: phase1d-polish-minors

- **WHAT** — two code-reviewer polish minors, both explicitly "not debt": (a)
  `callee_takes_bare_number`/`callee_returns_bare_number` (`emit.rs:18922`/`:18885`-region) — the
  first-param predicate copies the return-type predicate's local-items + imported-fns scan plumbing
  verbatim; a shared scan helper would consolidate; (b) the decimal128 heap-cell size `16` is a bare
  literal at the alloc site (`emit.rs:3459` `number_to_heap_cell`) and both free sites (`:9717`
  trampoline `spike_num_free`, `:15798` `BgArgFreeKind::HeapShape { byte_size: 16 }`) with no
  compile-time link.
- **WHY** — pure polish — both predicates/sites are already correct and each a single authoritative
  consumer-shared source already (no drift risk named by the reviewer); consolidating mid-hotfix
  buys no behavior.
- **COST** — trivial: (a) ~30 min, one extraction + two call-site updates; (b) ~15 min, one const +
  three substitutions.
- **TRIGGER** — (a) the next milestone that touches either predicate or adds a third bare-number
  callee probe; (b) the next milestone that touches the decimal128 boundary machinery (e.g. the
  conduit-marshalling deferral above, which would add a fourth site).

No code touched. Nothing committed. Session-id `executor-2026-07-16-m6-phase8` appended to the
roadmap's frontmatter chain in the same action as this entry.

## 2026-07-16 — Fix-loop closure: the fr9-fr14 four-field record was missing from the Phase-8 lift above

A `graveyard-auditor` fix-loop finding confirmed that five separate places in the Phase-8 lift diff
(this file's summary sentence above; both Capability Ledger tables' row 441; the M6 plan's own Future
Requirements #9/#14 lift-notes; the M6 plan's own Phase 8 completion bullet) all CITED
Idempotency-Key `2026-07-04-v0-3-m6-concurrency-hotfix#8-fr9-fr14:
int-literal-number-coercion-store-and-call-site` as if the full four-field WHAT/WHY/COST/TRIGGER
record already existed at that key in this file — it did not; only the citation existed, never the
record. Confirmed absent by a whole-file grep for the `Idempotency-Key:` sentinel before writing
(idempotency check, per the surrounding Phase-8 lift's own convention).

The actual missing entry ("Deferral: int→`number` coercion — store-site + call-argument-site
(paired, roadmap-ledger row 441, flagged for Patrick's Gate-4 home call)") has now been written above,
between the `#8-fr6` and `#8-fr10` entries — matching the position implied by the summary sentence's
own enumeration order ("#1, #4, #5, #6, #9+#14, #10, #11, #12, #13+#17, #15+#16") and the style/shape
of the sibling entries this same Phase-8 lift wrote. Content is drawn directly from the M6 plan's own
Future Requirements #9 and #14 text (`.claude/planning/active/2026-07-04-v0-3-m6-concurrency-hotfix/plan.md:3491-3527,3626-3698`)
and cross-checked against the stub plan
[`2026-07-04-v0-3-hotfix-int-literal-number/plan.md`](../2026-07-04-v0-3-hotfix-int-literal-number/plan.md),
which carries no divergent content. Both Capability Ledger tables (roadmap.md rows 441/506) already
cited this key accurately and needed no wording change — they now resolve to a real record instead of
a dangling citation. Gap closed; no code touched; nothing committed. Session-id
`executor-2026-07-16-m6-phase8-fixloop-fr9fr14` appended to the roadmap's frontmatter chain in the
same action as this entry.

## 2026-07-16 — Ledger amendment: v0.3-M6 Future-Requirements #20/#21/#22/#23 lifted to the roadmap's durable store (fix-loop closure — a real gap the original Phase-8 lift missed entirely)

A five-reviewer fix-loop round over the M6 Phase 8 diff (code-reviewer as the primary finder) found
that four M6 Future-Requirements items — #20, #21, #22, #23 — were never lifted to this roadmap's
durable store despite the Phase 8 completion note's claim that "every surviving Future-Requirements
deferral" had been. Confirmed absent by a whole-file grep for each candidate `Idempotency-Key:`
sentinel before writing (idempotency check, per this file's own established convention). All four
lifted now, each as its own four-field payload entry, plus an owner-tagged (or explicitly
Patrick-flagged) pointer row added to BOTH Capability Ledger tables in `roadmap.md`.

### Deferral: general UFCS arg-validation gap + `array.remove` no codegen lowering (grouped alongside row 441)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr20: ufcs-arg-validation-gap-and-array-remove-lowering

- **WHAT** — two orthogonal sub-items: (a) the general UFCS/collection arg-validation surface does
  NOT validate the `number`→`int` direction — `a.concat([5])` and `pick(5, price)` pass typeck
  without the int→number gate's scrutiny in the reverse direction; (b) `array.remove` has NO
  codegen lowering arm for ANY element type — unimplemented at the backend, not merely for
  `number`.
- **WHY** — both are pre-existing gaps orthogonal to M6's concurrency charter and to the
  int-literal→`number` gate this milestone completed (which covers the int-literal→`number`
  direction, not the reverse and not the `array.remove` lowering); neither is a concurrency
  defect.
- **COST** — (a) extend the arg-validation surface to the reverse direction — small, folds into
  roadmap-ledger row 441's coercion + a validation pass; (b) implement the `array.remove` codegen
  lowering arm — its own small backend pass.
- **TRIGGER** — a real user hits either gap, or the milestone that owns collection-method codegen /
  the int↔number coercion class picks them up alongside row 441.

### Deferral: narrowed-union background receiver — the durable fix (grouped alongside the already-lifted #24 union-narrowing row)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr21: narrowed-union-background-receiver-durable-fix

- **WHAT** — make a union binding narrowed to a shape variant WORK as a `background` receiver
  (both spawn forms): extract the variant's payload across the spawn boundary, reusing the
  existing `union_to_heap_cell` envelope+tag-resolved deep-copy (`crates/ynz-codegen/src/emit.rs:3248`,
  consumer `:13069`) that already does exactly this for the let-bound union arg-escape case. The
  memory-safety exposure itself is already CLOSED — a confirmed reachable OOB read (CWE-125, 48+
  bytes past the 16-byte `{tag,data}` storage, IR-reproduced) is rejected fail-closed today with a
  deterministic teaching error (FRAGO 026). ALSO covers a concurrency-adjacent sibling in the SAME
  extraction family: a Call-form `background work(fig)` with a give-transferred UNION arg compiles
  and runs but the task's tag-match produces NO output (expected `circle`). This entry does NOT
  duplicate #24 above — #24 covers the two NON-concurrency general union-narrowing siblings
  (narrowed direct field access silent-wrong; union→shape re-bind OOB/SIGSEGV); this entry is the
  concurrency-adjacent `background`-receiver defect, and the two are grouped because ONE design
  pass (the same `union_to_heap_cell`-based extraction machinery) closes both.
- **WHY** — the interim fail-closed rejection already removes the memory-safety exposure; the
  extraction itself is spawn-path wiring + tests beyond the blocker-round's charter.
- **COST** — small-to-medium (<1 phase) — wire `union_to_heap_cell` into the background-arg
  heap-upgrade path for the narrowed-receiver (and union-arg) cases + RED→GREEN fixtures for both
  spawn forms + the give-transferred union-arg sibling (the same machinery closes #24's two
  general surfaces too).
- **TRIGGER** — the milestone that owns union-payload extraction (same family as the
  non-plain-ident receiver row below), or a user hits the teaching error and needs the working
  form.

### Deferral: call-only large-copy Tier-3 warning — UFCS-receiver teaching parity (trivial, teaching-only)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr22: call-only-large-copy-tier3-warning-parity

- **WHAT** — the background large-copy lint (`check.rs`, `BACKGROUND_LARGE_COPY_BYTES` loop) fires
  only for `Expr::Call` args; a UFCS receiver >64 bytes gets no give-vs-copy teaching warning.
- **WHY** — teaching-parity only, zero correctness/memory-safety impact — and the phase already
  built the spawn-target normalization (`background_spawn_call_form`) the extension would reuse.
- **COST** — small (<1 session).
- **TRIGGER** — whichever future phase next touches background-spawn UFCS diagnostics.

### Deferral: non-plain-ident shape receivers/args in background-spawn position — BOTH spawn forms (flagged for Patrick's own MILESTONE-seal human call, NOT auto-assigned)
Idempotency-Key: 2026-07-04-v0-3-m6-concurrency-hotfix#8-fr23: non-plain-ident-background-spawn-receivers

- **WHAT** — heap-upgrade non-plain-ident shape receivers/args in background-spawn position, both
  forms — `background fleet.flagship.haul()` / `ships[0].haul()`, and equally the Call-form
  `background haul(fleet.flagship)` — today ride membership-less as raw pointers (`is_heap_arg`,
  `emit.rs:~15909`, gates on `Expr::Ident`/explicit `.copy()`, dropping any field-access/index
  expr to no-heap-upgrade). Pre-existing, shared by both spawn forms, NOT introduced or widened by
  M6 Phase 3c.
- **WHY** — needs new field-projection give/copy machinery beyond Phase 3c's
  give-transferred-plain-ident-receiver charter; building it now expands the charter for an
  unconfirmed-live exposure (`security` could NOT reproduce a live UAF for the simple field-access
  case — the base local's storage survived — and `critical-path` couldn't confirm the full blast
  radius; a latent asymmetry, not a confirmed-live blocker).
- **COST** — a dedicated fix (new codegen give/copy machinery for field/index/return-materialized
  receivers), ~1 phase.
- **TRIGGER** — a live UAF is reproduced for a non-plain-ident receiver, OR the milestone-seal
  review (per the deviation-judge, route like the R13/R14 signed-risk overrides if confirmed
  live). **This item is deliberately NOT given a default owning milestone by this lift — its
  Capability Ledger pointer row (both tables) explicitly flags it for Patrick's own
  milestone-seal decision, mirroring how roadmap-ledger row 441 / `#8-fr9-fr14` is flagged for
  Patrick's Gate-4 home call, rather than being silently auto-assigned.**

RECORD-ONLY: transcribing already-decided M6 Future-Reqs into their durable roadmap home, no
adjudication of BUG-vs-NOT-a-bug triage (that stays Patrick's own call — the four new Capability
Ledger rows all carry "**Triage: NEEDS PATRICK'S CALL — not yet triaged.**" rather than a
fabricated verdict). Combined with the ten entries already lifted in the "v0.3-M6 Phase 8
durable-home deferral lift" session above, this brings the total to **14 named surviving
deferrals lifted as fourteen four-field payload entries** — the M6 plan's own `plan.md` and
`audit.md` were corrected in the same round to state this number consistently (they previously
misstated it as "11 … ten"). No code touched. Nothing committed. Session-id
`executor-2026-07-16-m6-phase8-fixloop-fr20-23` appended to the roadmap's frontmatter chain in the
same action as this entry.
