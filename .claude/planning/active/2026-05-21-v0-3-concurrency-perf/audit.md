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
