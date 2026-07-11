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
