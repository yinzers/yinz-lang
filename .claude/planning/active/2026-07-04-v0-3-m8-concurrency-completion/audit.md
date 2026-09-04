---
name: "v0-3-m8-concurrency-completion-audit"
plan-id: "2026-07-04-v0-3-m8-concurrency-completion"
metadata:
  type: "plan-audit"
---
# Audit trail — 2026-07-04-v0-3-m8-concurrency-completion

Append-only. *How the plan got here.* Read by the AAR, auditors, and the execution conductor's
Step-3a / Step-0 reconcile; never by executors (they read the current-truth plan.md slice).

## Session log

- `m8-p8-fix4-20260904` — 2026-09-04 — **Phase 8 correction round (verification + audit corrections + corpus widening): proof-of-fire claim corrected, citation qualifications verified, pool-reuse floor test corpus expanded. Nothing committed.**
  Read: the dispatch instruction, `mod.rs:~1244-1261` and the pool-reuse floor test, `take_or_make_array`'s doc comment, `audit.md` BLOCKER 1 and FRAGO 015 entries, `README.md:~75 and ~137`.
  **Proof-of-fire correction (BLOCKER 1, round-3 entry false claim).** Round-3's `m8-p8-fix3-20260904` entry (line ~25) claimed the panic-probe fires on "the FIRST seed (seed 0)" with the fixed code. Re-run confirms: first fire is **seed 176**, not seed 0. The pre-fix half (old code, 0 panics across 8,192 seeds) remains correct. Round-3 entry stands unedited per append-only; this record corrects the false claim.
  **Citation qualification verification.** All citations to "the v0.3-M8 plan's `audit.md`, FRAGO 015" verified present in `mod.rs` (lines 767, 868), `README.md` (lines 75, 137 — edited this round to qualify line 137). Grep across the entire repo (excluding `.claude/planning/active/2026-07-04-v0-3-m8-concurrency-completion/` and `.claude/corpses.md`) found zero bare `FRAGO 015` citations outside the M8 plan directory — all other plans' FRAGO 015 entries are their own unrelated audit records (M4/M5/M6/M7 and worktrees).
  **Future Requirements #11 forward pointer (edit 3).** `plan.md` FR#11(a) and (b) are already documented; neither citation needed a plan-internal forward pointer, as the FR itself IS the durable home. Verified by direct read: FR#11 exists, both defects fully recorded with all four fields (WHAT/WHY/COST/TRIGGER), no gaps.
  **Pool-reuse floor corpus widening (edit 4).** `mod.rs:~1250-1261` floor test runs `per_construct_floors_hold_over_a_fixed_corpus` at `n = 256` (fixed 0..256 corpus). The reuse floor measured 2/256 (vacuous). Widened the corpus to `n = 1024` (fixed 0..1024 seeds). Re-measured at base 0: **13/1024** (1.27%); base 10000: **17/1024** (1.66%); base 1000000: **17/1024** (1.66%). All three land well above zero and hold steady. Reuse floor (`>= 1`) holds green with 4x larger corpus; no adjustment to the probability in `take_or_make_array`/`take_or_make_map` needed.
  **Verification.** `cargo fmt --all --check` clean; `cargo clippy -p ynz-driver --all-targets -- -D warnings` clean; `cargo test -p ynz-driver --test cross_impl_consistency` 17 passed / 0 failed (pool-reuse floor test slower under 1024-corpus sweep, but all tests green). `git diff --stat`: 2 files, 3 insertions(+), 2 deletions(-).

- `m8-p8-fix3-20260904` — 2026-09-04 — **Phase 8 fix round 4 (the ceiling round): all 3 blockers
  and 5 should-fixes from the round-3 review closed. Nothing committed.**
  Read: this fix-round dispatch, `fuzz_grammar/mod.rs` and `README.md` end to end, `plan.md`'s
  Future Requirements #11, `audit.md`'s FRAGO log (confirmed 015 was free), `main` (`ec014d8`) via
  a throwaway `git worktree`.
  **BLOCKER 1 — the reuse-from-pool branch was dead code.** `stmt_channel_inline_kind` set
  `suspension_seen = true` BEFORE the composite's own Array/Map sends ran — the composite is the
  ONLY caller of `take_or_make_array`/`take_or_make_map`, so their `!suspension_seen` reuse check
  was already false by construction. Fixed by moving the assignment to AFTER the composite's sends
  and receives complete. **Proof, both directions, panic-probe + 8,192 seeds**: with the OLD
  (pre-round-4, `ccbaa6b`) code, replacing the reuse branch body with `panic!` and generating
  seeds `0..8192` — 0 panics (dead, confirmed). With the FIXED code, the identical probe panics on
  the FIRST seed (seed 0). Added `Program::fired_pool_reuse` (set at both reuse sites) and a
  raw-count floor (`>= 1`, not a percentage — the construct needs four preconditions to align in
  one statement, measured 2/256 over the fixed 0..256 corpus) in
  `per_construct_floors_hold_over_a_fixed_corpus`, so this cannot silently die again without a red
  test.
  **BLOCKER 2 — two doc comments asserted the opposite of what shipped.** `take_or_make_array`'s
  "Half the time, reuse..." and the "preserving the widening's actual value" claim were both
  false as shipped (BLOCKER 1). Rewrote both doc comments to describe the actual gate (`one_in(2)`
  reuse ONLY when `suspension_seen` is false and the pool is non-empty; unconditional fresh once
  it's true) and to record BLOCKER 1's history directly at the site.
  **BLOCKER 3 — FRAGO 015 did not exist; four durable pointers cited it.** Confirmed by direct
  read: this plan's `audit.md` FRAGO log ran 001–014, no 015. Minted the real
  `### FRAGO 015 — 2026-09-04 — The record MINTED THIS round...` entry (placed at the top of the
  FRAGO log, newest-first) with trigger, both defects' full repro, both defects confirmed
  PRE-EXISTING on `main`, the containing guards, and the disposition (routed via standing CCIR
  item 5 / risk R5 policy, not fixed — no fresh ruling needed). Fixed all four citations
  (`mod.rs` twice, `README.md` once, this file's own round-3 entry is corrected here rather than
  edited in place, append-only) to say "the v0.3-M8 plan's `audit.md`, FRAGO 015" — the trap
  named in the dispatch (a DIFFERENT plan, `2026-07-03-v0-3-m5-auto-soa/audit.md`, already has its
  own unrelated FRAGO 015) is now named explicitly in the new record so a future citation can't
  repeat it. **Self-correction on the round-3 entry above:** it claims "FRAGO 015 minted" — that
  claim was false at the time it was written; this record is the actual mint. The prior entry
  stands uncorrected in place per append-only discipline.
  **Sanity-checked one of the two `main`-vs-branch evidence items myself** (defect a, per the
  dispatch's instruction — not re-derived from the reviewer's numbers, independently reproduced):
  built `main` (`ec014d8`) in a throwaway `git worktree` (removed after), ran the exact minimal
  repro 3×: `misaligned pointer dereference: address must be a multiple of 0x8 but is 0xb` at
  `ynz_array_count` (`lib.rs:1340` on `main` — line-shift-only match to the branch's `:1411`),
  3/3 crashes; the safe-order variant (array declared after the `wait`) printed `1`, 3/3. Matches
  the dispatch's recorded evidence exactly.
  **Should-fixes.** (1) `plan.md` FR#11(b) rewritten: NOT `number`-specific (the `int` variant
  shows it too), NOT deterministic (~17-30% across runs), NOT a lost value (a heap-address/
  garbage read) — pointer moved from `number_to_heap_cell` to the blocked-send path
  (`send_count > capacity`). (2) FR#11(a) now records that the dominant symptom with the guard
  removed is silent wrong output (28/35 non-crashing `MODE-DIVERGENT`, exit 0) in the DEFAULT
  mode, not the SIGABRT. (3) This entry is the append-only correction of round-3's "FRAGO 015
  minted" claim (round-3's own entry is left as written, per append-only). (4) Backpressure
  coverage loss (guard for defect b makes every generated producer non-blocking, corpus-wide, with
  no prior non-coverage note) added to `fuzz_grammar/README.md`'s "What it deliberately does NOT
  cover" list, naming defect (b) and the restoration trigger. (5) DRY: collapsed the three
  drifted array-literal builders into `fresh_array_literal()` (the drift itself — `below(3)` in
  `build_feed_body`'s Array arm vs `below(4)` in the other two — is fixed at the producer, now one
  source drawing `below(4)`), the three map-literal builders into `fresh_map_literal()` (these
  three had NOT drifted from each other), and the three near-identical Number/Array/Map
  receive-print blocks into `emit_receive_prints()`.
  **Minors.** The falsified "without incident" parenthetical (`stmt_background_drain_loop`'s old
  comment, claiming `int`'s capacity/count draw was safe "without incident") removed as part of
  the should-fix-1 comment rewrite — the `int` repro directly disproves it. Shape-free programs no
  longer generatable (`declare_shapes`'s `1 + below(2)`): already documented in place as a
  considered choice (round 3's own comment at that site); no further edit needed.
  **Parked, not done — named per the dispatch's own escape valve ("if under an hour... otherwise
  record as a four-field parked entry"):** the `suspension_seen`-is-hand-maintained-at-5-sites
  lesson the dispatch itself named. **WHAT:** derive the suspension flag from `push()`'s own
  emitted line, or carry one `suspends: bool` per composite in a lookup table, instead of five
  hand-set call sites (`~502, 627(moved), 791, 818, 898` post-this-round). **WHY:** this is a
  syntactic-site enumeration — the exact detection signature `.claude/graveyard.md`'s twin-
  derivation entries warn about — and a future suspending composite that forgets its own
  assignment silently reopens defect (a)'s corpus blind spot; doing it right needs a design
  decision (derive-from-`push()` vs. a lookup table) this round's time budget (already at the
  three-blocker-plus-five-should-fix ceiling) does not have room to make well. **COST to fix
  later:** small — a single-session refactor once the derivation approach is picked; the five call
  sites are already each individually documented. **TRIGGER:** the next new suspending composite
  added to this generator, OR a `per_construct_floors_hold_over_a_fixed_corpus` regression that
  traces to a missed `suspension_seen` site.
  **Verification.** `cargo fmt --all --check` clean; `cargo clippy -p ynz-driver --all-targets --
  -D warnings` clean; `fuzz_grammar::generator_contract::*` 7 passed / 1 ignored (including the
  new floor); full `cargo test -p ynz-driver --test cross_impl_consistency --include-ignored`: 17
  passed / 0 failed, 176.2s; two independent 256-seed `generated_corpus_byte_identical_across_
  mode_matrix` runs (seed bases 0 and 31337000): both 256/256 compiled-and-ran / 0 findings
  (60.4s, 59.7s). **Guard-removal re-measurement (both restored via `cp` + sha256 before/after,
  never `git checkout --`):** `suspension_seen` guard removed entirely, `YNZ_FUZZ_PROGRAMS=256`
  seed base 0 — **35 findings**, exact match to FRAGO 015's recorded number, all `SIGABRT`/
  `MODE-DIVERGENT` at `ynz-runtime/src/lib.rs:1058`/`:1411`. Capacity floor removed
  (`stmt_background_drain_loop`'s `cap` reverted to a plain `1 + below(4)` draw, no `send_count`
  max), same corpus — **6 findings** (FRAGO 015 recorded 7; within the same run-to-run noise band
  the record itself documents for defect (b), ~17-30% non-deterministic — not re-run further since
  the dispatch asked to confirm the numbers still hold, not to chase an exact match on a
  documented-nondeterministic count). No `Cargo.toml`/`Cargo.lock` change. Registry entries
  touched: none. Nothing committed.

- `m8-p8-fix2-20260904` — 2026-09-04 — **Phase 8 fix round 3: the owned-heap-channel widening
  landed, the false-claim lock test deleted, per-construct corpus floors added — and the
  widening itself surfaced TWO genuine runtime defects (FRAGO 015 / Future Requirements #11),
  neither fixed inline, both contained at the generator level with narrow, evidence-backed
  guards. Nothing committed.**
  Read: this fix-round dispatch, `fuzz_grammar/mod.rs` and `README.md` end to end,
  `cross_impl_consistency.rs`'s oracle/mode-matrix machinery, `.github/workflows/ci.yml`'s fuzz
  job, `crates/ynz-runtime/src/lib.rs` (`ynz_map_count`/`ynz_array_count`, the panic sites the
  two new crashes landed in), `plan.md`'s Future Requirements section (numbering continuity),
  hand-written `v0_3_m8_p4_chan_array_roundtrip.ynz`/`chan_map_roundtrip.ynz`/
  `number_chan_roundtrip.ynz`/`flow_two_hop_give.ynz`/`flow_admitted_forms.ynz` (the send/receive
  idiom the widening's composites were modelled on).

  **Item 1 — the widening, and what it found.** The prior README claim ("owned-heap payloads
  would produce correctly-REJECTED programs") was false, exactly as the dispatch's own probe
  showed: a `let` built and sent immediately needs no `.copy()`/`give` plumbing since nothing
  reads it back. Widened `ElemKind::{Int, Array, Map, Number}` across three composites
  (`stmt_channel_inline_kind`, `build_feed_body`/`stmt_background_drain_loop`'s feeder, and —
  new this round, matching the dispatch's explicit ask — a reuse-from-pool path in
  `take_or_make_array`/`take_or_make_map` that POPS a picked binding out of `self.arrays`/
  `self.maps` so a later `stmt_array_op`/`stmt_map_op` draw cannot read it back). `number`
  exercises fr12's copy-through contract directly (the same binding sent 2-3 times, still
  readable afterward). Deleted the false lock test
  `no_program_sends_an_owned_heap_payload_into_a_channel`.

  **Two defects found BY doing the widening, not before.** A 256-seed local run (`YNZ_FUZZ_
  PROGRAMS=256`) immediately surfaced a `SIGABRT` (seed 3) that survived one round of narrowing.
  Bisection (full transcript of ~20 minimal `.ynz` repros run against `./target/debug/ynz run`
  directly, not committed — kept only as text in this entry and the session transcript) found
  TWO independent, precisely-characterized, deterministic runtime defects, both new to this
  widening's construct space:
  1. **Crossing-local heap-channel-send corruption.** An `array<int>`/`map<string,int>` LOCAL
     declared BEFORE any suspension point (`wait`, a `background`-handle `.receive()`, a channel
     `.receive()`) in the SAME function, later `.send()`-ed into a channel AFTER that suspension,
     reads back corrupted — `RUNTIME ERROR: killed by signal 6 (SIGABRT)`,
     `crates/ynz-runtime/src/lib.rs:1058` (`ynz_map_count`) or `:1411` (`ynz_array_count`), a
     null or misaligned pointer. Minimal repro:
     ```
     function fetch1(n: int) -> int { wait sleep(1); return n + 6 }
     function entrypoint() -> nothing {
       let rows12: array<int> = [0]
       let v13 = wait fetch1(5)
       let wire15: channel<array<int>> = channel<array<int>>(1)
       wire15.send(rows12)
       let got16 = wire15.receive()
       if (got16.exists()) { print(got16.value.count().toString()) }
     }
     ```
     Swapping the order (array declared AFTER the `wait`) is confirmed SAFE, both ways, run
     three times each. Confirmed general to array AND map; confirmed NOT reproduced by
     `channel<number>`, by an inline (non-`background`) channel's own close-then-drain, or by a
     `background`-handle spawn with no channel at all; confirmed NOT present with an all-`int`
     channel sequence in the same shape (ruling out "any two channels in one program," narrowing
     to the transfer-into-a-channel path specifically). First surfaced through a much larger
     program (seed 3's full ~110-line source, then seed 102's, both saved in the fix-round
     session transcript) and narrowed by direct deletion of unrelated statements, re-running
     `./target/debug/ynz run` after each cut, until the six-line repro above was the smallest
     form that still crashed.
  2. **Capacity-forced-blocking `number` send loses a value.** `channel<number>(1)` fed by a
     `background` producer sending the SAME `number` binding 3 times prints `17.2` (`8.6 * 2`)
     instead of `25.8` (`8.6 * 3`) under `YNZ_NO_OPTIMIZE=1 YNZ_NO_AUTO_PARALLEL=1` — but ONLY
     when `send_count > capacity` forces the producer to actually block on a full buffer: 2
     sends into capacity 1 matches both modes; 3 sends into capacity 4 (no blocking needed)
     matches both modes; 3 sends into capacity 1 diverges. Confirmed with a direct minimal repro
     run under both env-var combinations, default and `--no-optimize`/`--no-auto-parallel`,
     three capacity/count combinations each.
  Both are genuine runtime defects, not generator bugs, and per this plan's own CCIR item 5 /
  risk R5 ("route through the plan-amendment/FRAGO seam... never fix inline unless it is
  trivially the SAME class an earlier phase in THIS plan already fixed") neither is fixed here —
  neither matches any already-fixed class in this plan (checked against FR#8's alias-container
  door, FR#10's `.copy()` catch-all, and the P2-3/P2-1 channel-close work; none share this shape).
  **FRAGO 015 minted; Future Requirements #11 written with full WHAT/WHY/COST/TRIGGER for both,
  plus an open clustering question (root-cause.md: are these one producer or two? bisection did
  not settle it — left for whoever picks this up).**

  **Containment, at the generator level, narrow and evidence-backed (not a repeat of the
  false-claim exclusion this round started by deleting).** `Builder::suspension_seen: bool`, set
  true the moment ANY statement introduces a real suspension in `entrypoint`'s frame (`wait`, a
  handle receive, a channel receive, the drain loop's own receive, the two-spawn topology's two
  receives). `take_or_make_array`/`take_or_make_map` refuse to REUSE a pooled binding once it is
  true (a FRESH local, built and sent in the same composite with zero suspension in between, is
  unaffected and always fires). This is deliberately narrower than an earlier draft of this fix
  (a `channel_close_seen` flag that pinned an entire LATER composite to `ElemKind::Int` once any
  drain loop had fired) — that draft was tried FIRST, found the map/array-after-drain-loop case,
  but a SECOND 256-seed run at that point still found seed 102's crash (the true trigger:
  ANY suspension, not specifically a drain-loop's close+drain), which is what led to the deeper
  bisection above and the more precise `suspension_seen` gate that replaced it. For defect #2,
  `stmt_background_drain_loop`'s channel capacity is now `send_count.max(1 + rng.below(4))` —
  `FeedFn` gained a `send_count: usize` field so the consumer can read how many sends its own
  feeder will make. Both guards are documented in full at their exact code sites, not just here.

  **Verification the widening is otherwise clean.** Two independent 256-program local runs after
  both guards landed, seed bases 0 and 31337000: **256/256 compiled and ran, 0 findings, both
  runs** (60.0s and 59.9s). `owned_heap_channel` (a program actually RAN a non-`int` channel
  composite) measured 197/256 = 77.0% over the fixed 0..256 corpus — well above the floor below.
  Full `cargo test -p ynz-driver --test cross_impl_consistency` (all 16 tests, unbounded): **16
  passed / 0 failed / 1 ignored, 172.7s** — including the default 24-program fuzz lane and the
  BLOCKER-1 process-group-kill test from the prior fix round, both green together (an EARLIER
  full-suite run this session saw the kill test fail once under the same full-corpus-plus-sweep
  CPU contention the prior round's own comment already documents as expected; re-run alone it is
  green, and it is not touched by anything in this round).

  **Item 2 — per-construct corpus floors.** The four pre-existing anti-triviality asserts
  (distinctness, `background` substring, entrypoint presence, `body_stmts >= 10`) never checked
  WHICH menu arm fired — confirmed exactly as the dispatch described: deleting arms 12/13 from
  `emit_statement`'s `below(14)` draw left every one of them green. Added
  `per_construct_floors_hold_over_a_fixed_corpus`, keyed off NEW counters set directly at each
  composite's own fire site (`Program::fired_inline_close`/`fired_drain_loop`/
  `fired_two_spawn_arc`/`fired_map_decl`/`fired_array_intrinsic`/`fired_shape_field_read`/
  `fired_owned_heap_channel`) — deliberately NOT a source-text substring search, because a
  substring search is unsound here regardless of floor value: an unconditionally-emitted,
  UNCALLED `feedN` helper always contains `.close()` in its own body (every feeder closes,
  whether or not the entrypoint ever calls it — "an uncalled helper is itself coverage," the
  existing design's own stated rule), so a whole-source `.close()` count would read close to
  100% no matter what `stmt_channel_inline`'s own conditional close branch did. Measured over
  0..256 (post-widening, post-both-guards): inline-close 97/256 (37.9%), drain-loop 164/256
  (64.1%), two-spawn-Arc 165/256 (64.5%), map-decl 224/256 (87.5%), array-intrinsic 106/256
  (41.4%), shape-field-read 161/256 (62.9%), owned-heap-channel 197/256 (77.0%). Floors set per
  the dispatch's own guidance (20% for the three flagship constructs — inline-close, drain-loop,
  two-spawn-Arc — plus this round's OWN flagship, owned-heap-channel, treated the same way at
  25%; sensible values for the rest: map-decl 40%, array-intrinsic 15%, shape-field-read 25%).
  **Root-cause bullet the dispatch named:** `declare_shapes`' `below(3)` (0-2 shapes, 1/3 chance
  of zero) starved arms 9/13 for no compensating coverage value — fixed at the producer:
  `1 + below(2)` (always 1-2 shapes; the "how many" variety survives, the "none at all" case,
  which is not a distinct code path anywhere else in the grammar, does not). This is WHY
  two-spawn-Arc's measured share (64.5%) is meaningfully higher than the dispatch's own
  pre-widening baseline (39.6%) even before counting the widening's own effect.

  **Minors.** (a) `body_stmts` counted emitted LINES (including `}`), not statements — mean 43.9
  against a 10-19 draw, so `>= 10` was accidentally-always-true regardless of what the generator
  did. Replaced with `stmt_count: usize`, incremented once per the seed `let` and once per
  `emit_statement()` call (never per `push()`), so it is genuinely bound to `build_body`'s own
  guarantee (`1 + (9 + below(10))` = `10..=19`) — a real invariant a future edit to `target`'s
  range would actually break. (b) The `wait`-prefix lock's `[("fetch0", 0), ("fetch1", 0)]` was
  complete only by coincidence with `declare_helpers`' `1 + below(2)` — replaced with
  `Program::io_fn_names: Vec<String>`, read from `self.io_fns` at `finish()` time, so the check
  is now genuinely derived from what THIS seed actually declared, and the dead tuple `0` element
  is gone. (c) `.github/workflows/ci.yml`'s fuzz job uploaded only `fuzz.log` on failure while
  the README calls the KEPT scratch directory (full untruncated per-mode capture files) the
  durable replay artifact — added a second `actions/upload-artifact` step uploading
  `/tmp/ynz-fuzz-*` (the OS temp dir `tempfile::Builder`'s `prefix("ynz-fuzz-")` lands in on this
  runner, since the call has no `tempdir_in`), so CI now keeps what the README promises it keeps.

  **Mutation re-proof.** Backed up `cross_impl_consistency.rs` to the session scratchpad first
  (`cp`, never `git checkout --`); sha256 before mutating:
  `63d41a65a871da716482a24e4ba3ade1fcbb0605ee3ed4e6a878ec7e0fe4ee0d`. Planted a one-line,
  `no_optimize`-gated corruption in `run_ynz_mode_bounded_impl` (`out_bytes.push(b'X')` when
  `no_optimize` is true) — deliberately in the HARNESS's own capture path, not the compiler,
  mirroring the prior fix round's own BLOCKER-1 re-proof style (mutate the mechanism under test,
  not an unrelated stand-in). Ran `YNZ_FUZZ_PROGRAMS=32`: **RED, 64 findings** (every one of the
  32 programs' two `no_optimize` corners, `[parallel+no-optimize]` and
  `[sequential+no-optimize]`, both flagged `MODE-DIVERGENT`) — confirmed 176 occurrences of
  `channel<array`/`channel<map<`/`channel<number` substrings across the failure text, and
  specifically confirmed at least one flagged finding (`gen_...00001.ynz`, `[parallel+
  no-optimize]`) embeds a `channel<array<int>>` composite in its source, so the widened oracle
  demonstrably still classifies a heap-payload program's mode divergence correctly, not just an
  `int`-only one. Restored from the scratchpad copy via `cp`; sha256 after restore:
  `63d41a65a871da716482a24e4ba3ade1fcbb0605ee3ed4e6a878ec7e0fe4ee0d` — byte-identical to the
  pre-mutation copy. Re-ran the same command: **GREEN, 0 findings** (`test result: ok`).

  **Verification, full.** `cargo fmt --all --check` clean; `cargo clippy -p ynz-driver
  --all-targets -- -D warnings` clean; `fuzz_grammar::generator_contract::*` 7 passed / 1
  ignored; full `cargo test -p ynz-driver --test cross_impl_consistency` 16 passed / 0 failed /
  1 ignored, 172.7s; two independent 256-seed `generated_corpus_byte_identical_across_mode_
  matrix` runs (seed bases 0 and 31337000), both 256/256 compiled-and-ran / 0 findings; the
  mutation RED→GREEN re-proof above. No `Cargo.toml`/`Cargo.lock` change. Registry entries
  touched: none. Scratch repro files (~20 `.ynz` probes, `.scratch-repro/` in the working tree)
  were deleted before this entry was written — none were committed; the ones load-bearing for
  this record are reproduced verbatim above. Nothing committed.

- `m8-p8-fix1-20260904` — 2026-09-04 — **Phase 8 fix round: BLOCKER 1 (a hung generated program
  survived its own timeout) and BLOCKER 2 (a false claim in the durable record) both closed, four
  should-fixes done, nothing committed.**
  Read: this fix-round dispatch, `cross_impl_consistency.rs` end to end, `run.rs::run`,
  `fuzz_grammar/mod.rs` + `README.md`, `.github/workflows/ci.yml`'s fuzz-job comment,
  `Cargo.toml`/`Cargo.lock` for `nix`/MSRV.
  **BLOCKER 1.** `run_ynz_mode_bounded`'s `child.kill()` only reached the direct `ynz run`
  process; `ynz run` itself blocks on `Command::status()` for the COMPILED BINARY it built
  (`run.rs::run`) — a grandchild doing the actual hanging work, reparented (not ended) by a
  direct-child-only kill. Fixed at the producer: the spawned `ynz run` now gets
  `process_group(0)` (stable since Rust 1.64; workspace MSRV 1.80, verified in `Cargo.toml`) so
  its own pid equals its pgid and every process it spawns inherits that group; the timeout path
  now calls `killpg` via `nix` (already a normal, non-dev workspace dependency reachable from
  integration tests — confirmed by a clean `cargo build -p ynz-driver --tests` with no Cargo.toml
  change, so no new dependency was added). Refactored `run_ynz_mode_bounded` into a thin
  production wrapper over `run_ynz_mode_bounded_impl` plus a `#[cfg(test)]` twin
  (`run_ynz_mode_bounded_with_killed_pgid`) that also reports the killed pgid, per
  `authoritative-derivation.md` — the test verifies the SAME kill path production uses, not a
  re-derived copy. New test `bounded_run_kills_the_whole_tree::
  timed_out_program_leaves_no_descendant_process_running` spawns a generated-shaped infinite loop
  (`while (true) { x = x + 1 }`, no I/O, no channel), confirms the budget fires, then polls
  `/proc/<pid>/stat` for the killed pgid up to 3s (a liveness-poll window, not a fixed sleep —
  the first version used a single 300ms check and false-failed under full-suite CPU contention
  because a genuinely-dead process can sit briefly in `D` state before SIGKILL is honored;
  polling fixed it without weakening the assertion) for any member whose STATE is not `Z`
  (zombie exists-but-reaped-pending is not the bug; still-scheduled is). **RED→GREEN, both
  witnessed directly, not inferred**: reverted `kill_process_tree` in-place to the pre-fix
  direct-child-only `child.kill()` (backed up first to the scratchpad, restored via `cp` after —
  never `git checkout --`) and re-ran the new test: FAILED, `process group 173 still has a
  RUNNING member (Some((177, "R")))` — the grandchild binary caught mid-execution. Restored the
  fix from the scratchpad copy, re-ran: `ok`. Both runs captured in this session's transcript.
  **BLOCKER 2.** The `m8-p8-20260904-a1` audit entry (~line 62) and the sealing commit body both
  asserted parked item 41 was "a coverage gap named in the design note, not a silent pass" — it
  was NOT named there; `fuzz_grammar/README.md`'s non-coverage list enumerated four unrelated
  gaps and never mentioned spawn-site argument shapes. Fixed the ARTIFACT per the dispatch's
  instruction (the append-only audit entry stands uncorrected in place; this paragraph is the
  correction): added parked 41 to the README's "What it deliberately does NOT cover" list,
  stating the grammar restricts spawn-site arguments to idents/literals so
  `f(x, mutate(x))` (the shape parked 41 names) cannot be emitted and this harness does not
  exercise the question.
  **Should-fixes.** (1) `run_ynz_mode_bounded`'s capture switched from
  `read_to_string(...).unwrap_or_default()` (silently empties on non-UTF-8) to
  `fs::read` + `String::from_utf8_lossy`, matching `run_ynz_mode`'s sibling path — done. (2) The
  R5 plan-amendment/FRAGO routing-seam rule, previously stated only in a `ci.yml` comment, now
  also lives in `fuzz_grammar/README.md` (a dedicated paragraph beside the non-blocking-CI
  explanation) and in the sweep's failure `panic!` text — done. (3) `truncate()` now walks back
  to the nearest `is_char_boundary` before slicing instead of cutting on a raw byte index — done.
  (4) `ChanState`/`pending` (always 0, never incremented — a tautological assertion) deleted
  outright along with the `chans` field, all three push sites, and `assert_channels_balanced`
  and its call site; git history is the archive. The README's determinism argument that cited
  "the builder tracks pending sends... asserted at the end of generation" was ALSO stale after
  this — reworded to describe the real, still-true mechanism (every channel composite is
  self-balancing by construction; the corpus sweep's own liveness bound is the backstop) rather
  than leave a doc pointing at deleted code — done, root-caused rather than patched around.
  **Record obligations the dispatch named as previously missing from the durable trail:** the
  `m8-p8-20260904-a1` entry's CHECKPOINT-after-step-3 was passed through, not stopped at, and
  that decision (and its one-line reason — the checkpoint's condition was already met) IS
  recorded in that entry, immediately after the CHECKPOINT line; nothing further to add there.
  The cold-resume banner at the top of `plan.md` was NOT updated by this fix round — per the
  `m8-p7-fix1-20260904` precedent, updating that banner is the conductor's job at phase-boundary
  time, not a fix-round's. Both are noted here because this dispatch's instructions flagged them
  as absent from the record rather than as wrong decisions, and the record now has them.
  **Verification.** `cargo fmt --all --check` clean; `cargo clippy -p ynz-driver --all-targets --
  -D warnings` clean; `fuzz_grammar::generator_contract::*` 7 passed / 1 ignored; full
  `cargo test -p ynz-driver --test cross_impl_consistency` **16 passed / 1 ignored / 0 failed,
  173.1s** (the new process-group test included); a bounded local fuzz run
  (`YNZ_FUZZ_PROGRAMS=128`) reported `128 generated (128 distinct, 112 spawning background), 128
  compiled and ran to exit 0, 0 findings, 29.1s` — same summary-line shape as the sealed Phase 8
  run, zero findings. No `Cargo.toml`/`Cargo.lock` change (`nix` was already reachable). No
  registry entries touched. Nothing committed.

- `m8-p8-20260904-a1` — 2026-09-04 — **Phase 8 (Track 4b, structured fuzzing): spike GREEN,
  full grammar + oracle wiring + CI job + design note all landed; ZERO findings across 512
  generated programs; nothing committed.**
  Read: the Phase 8 block, `cross_impl_consistency.rs` end to end, `.github/workflows/ci.yml`'s
  Loom step, `test-parallelism.md`, `authoritative-derivation.md`, `corpses.md`, `parked.md`
  31/41, plus the M4/M5/M8 channel/handle/Arc/array/map fixtures the grammar is modelled on.
  **Step 1–2 — spike verdict GREEN, first attempt, no grammar narrowing needed.** 64 programs
  generated / 64 distinct / **64 compiled and ran to exit 0** (100%), 59 of them spawning
  `background`, 0 findings, 14.5s. The 100% rate is the design working as specified, not a
  weak grammar: the generator is type-valid *by construction* (it draws every operand from a
  typed environment it maintains), so a rejection would have been a GENERATOR bug — which is
  exactly how the sweep reports one. Anti-triviality is asserted mechanically, not asserted in
  prose: `distinct >= 90%` of the corpus in the sweep, `>=120/128` distinct + `>=128/256`
  concurrent + `>=10` body statements in the generator's own contract tests.
  **Steps 3–6.** Grammar covers the full composable subset the phase named — independent
  statements, `wait` calls, both `background` spawn forms (statement + handle), `channel<int>`
  construct/send/receive/close, shape declarations + field reads, `array<int>` and
  `map<string,int>` literals with their real intrinsics — plus two composites that matter to
  THIS milestone: the taught end-of-stream drain loop (Phase 4's `.close()` contract) and the
  two-spawn read-only-shape topology (Phase 5's Auto-Arc). The oracle is **extended, never
  re-derived**: the generator lives in `tests/fuzz_grammar/mod.rs` (a subdirectory module, not
  a second test target) so the sweep calls `cross_impl_consistency`'s own `parallel_sweep`,
  `outputs_match` and `output_order_is_scheduler_dependent` directly. A generated `background`
  program is therefore auto-classified as scheduler-order-dependent from its SOURCE, with no
  exclusion list to maintain — which is the whole reason that classifier had to stop reading
  file names.
  **CHECKPOINT (after step 3) — passed through, not stopped at.** The checkpoint's condition
  was met (grammar + oracle wiring complete) and the remaining work was ~20 minutes of CI YAML,
  one measured run, and a design note; segmenting there would have cost a handoff for no
  reduction in risk. Recorded here because the decision was the executor's, not the plan's.
  **Deadlock is designed out, not hoped for.** The builder tracks pending sends per channel and
  never emits a receive it cannot account for (asserted at the end of generation); helpers never
  print (so an auto-parallel reorder cannot move an output line); every suspending call in a body
  is `wait`-prefixed (an un-prefixed adjacent pair is the documented M3b Model-A intended
  reorder, which the oracle would flag as a divergence it is not); every channel is
  `channel<int>` (an owned-heap payload would need `.copy()`/`give` plumbing and would produce
  correctly-REJECTED programs). The last three are locked by generator contract tests, and a
  90s per-invocation liveness kill (`run_ynz_mode_bounded`) means a hang is reported as a
  finding rather than becoming one.
  **Step 4 — CI.** New `fuzz` job, `continue-on-error: true` with its promote-to-blocking
  trigger written beside the flag (30 consecutive findings-free, timeout-free runs). Bounded
  three ways: fixed corpus of 96, 90s per (program × mode) kill, `timeout-minutes: 30` on the
  job. Three-part vacuity guard under `set -euo pipefail` (zero tests matched, zero programs
  generated, or zero compiled all FAIL the step) — the Loom step's shape.
  **Step 5 — real local run: 2 × 256 programs (seed bases 770000 and 31337000), 1,024
  compile+link+execute invocations each, 60.6s and 59.0s, 256/256 distinct and 256/256 ran to
  exit 0 both times, ZERO findings.** Note that the four mode corners are four separate
  processes, so this sweep also catches run-to-run nondeterminism, not only mode divergence.
  **Step 5's contingency did not fire: NO genuine miscompile was found**, so nothing was routed
  through the FRAGO/R5 seam and nothing was fixed inline. Parked 41 (`background` argument
  evaluation order) was never tripped — the grammar passes only idents and literals at spawn
  sites, so no `f(x, mutate(x))` shape can be emitted; that is a coverage gap named in the
  design note, not a silent pass. Parked 31 (pre-existing `--all-targets` clippy debt) left
  alone; the touched test target is clippy-clean under `-D warnings`.
  **Step 6 — design note** at `crates/ynz-driver/tests/fuzz_grammar/README.md`: grammar
  coverage, the explicit non-coverage list with a WHY per item, the determinism argument, the
  mode matrix, the three budgets with their measured basis, and the replay path (seed →
  `print_generated_program`; a genuinely interesting case gets promoted out of the corpus into
  a named `tests/fixtures/` file, because a seed is a weak pin that any grammar change
  invalidates).
  Gates run: `cargo fmt --all --check` clean; `cargo clippy -p ynz-driver --tests -- -D
  warnings` clean (with NO `allow(dead_code)` blanket — the three unread `ChanState` fields, the
  unread `shape_vars` environment and a dead zeroing loop were deleted rather than muted, and
  `Program::seed` was made live by naming the saved file from the program's own recorded seed
  instead of a parallel local copy); full `cargo test -p ynz-driver --test
  cross_impl_consistency` **15 passed / 1 ignored / 0 failed, 176.4s** (both hand-written sweeps
  still green). The cleanup left the seed→program mapping byte-identical — the 64-program spike
  re-ran to the same three numbers afterwards. No `registry/features.toml`
  entry added, so the jargon-audit / ynz-registry lane (FRAGO 014) is out of scope this round.

- `m8-p7-fix1-20260904` — 2026-09-04 — **Phase 7 text fixes (teaching-surfaces + vocabulary rules applied): deferred feature registry entries rewritten for 18-year-old-JS-dev audience, file-path citations in IMP-concurrency.md replaced with function-name references, Future Requirements #3 paragraph appended re: cheap muted-hint guard, plan frontmatter session-id updated, no commits made.**

- `m8-p7-20260904-a1` — 2026-09-04 — **Phase 7 (Track 3, scope-drop cancellation): recon done,
  verdict Branch B — RE-DEFER; four-field re-deferral authored, registry entry updated, current
  state pinned in-suite; STATUS PARTIAL awaiting Patrick's sign-off; nothing committed.**
  Read: the Phase 7 block, the cold-resume banner's FRAGO 002 table (navigated to
  `SpawnStateFnFuture::drop` by function, not by the dangling `runtime.rs:591-693`),
  `IMP-no-function-coloring.md` "Task Cancellation", registry `background-handle-cancel-injection`
  / `channel-auto-close-on-last-producer` / `background-handle-close`, `IMP-concurrency.md`
  "Deferred: auto-close on last-producer drop", `handle.rs` (`ynz_rt_spawn_handle`,
  `ynz_handle_free`), `emit.rs` (`lower_let_background_handle`, `lower_sm_background_spawn`,
  `emit_bg_arg_frees`, the channel glue table at `channel_elem_drop`), `state_machine.rs::free_frame`,
  `runtime.rs::ynz_rt_shutdown`, `loom_tests.rs`'s model list, FR #7(2)/#8, `corpses.md` (3),
  `parked.md` 32–34, `authoritative-derivation.md`, `test-parallelism.md`, `teaching-surfaces.md`.
  **Step 1 — the recon question.** No generic scope-exit drop dispatch exists for ANY type.
  Codegen's only release emissions: `emit_bg_arg_frees` (a CPU-pool task's closure freeing ITS
  arg copies after the callee returns), the channel element-glue table (registered at
  construction, run by the runtime at teardown / `refuse_closed`), the spike trampoline's staged
  decimal128 cell, and `free_frame` (a suspending callee's frame BYTES, freed by its caller after
  Ready — no per-slot walk). The runtime's `SpawnStateFnFuture::drop` ladder is the CHILD task's
  retirement over the CHILD's frame descriptors. **Step 2 — does it extend to handles?** No, by
  frame and by time: a handle's scope exit is the PARENT's event on the PARENT's frame, and the
  parent has no ladder unless it is itself a task (and then it fires at retirement, not scope
  exit). **Probes** (throwaway, `target/probe-p7/`, `YNZ_ALLOC_COUNTER=1` +
  `YNZ_ALLOC_COUNTER_OUTPUT`, debug `ynz` at `d1c4294`; the `handle_alloc`/`handle_free` lines
  count `CpuJoinHandle`s, not `YnzTaskHandle` — the handle Box and the channel `Arc` are
  uncounted, so alloc==free cannot see a leaked handle; the IR read is the proof for that):
  P1 handle bound, never received, `entrypoint` returns at once → stdout `parent: returning` /
  `child: start` only; alloc=1 free=1 — `ynz_rt_shutdown` stopped the child at its next
  suspension and the ladder freed its frame. P1b statement-form control → identical, so that
  is shutdown, not the handle form. P2 handle bound in an `if` block, block exits, parent sleeps
  150ms → child prints all three lines including `child: done`; alloc=2 free=2 — **scope exit
  did not cancel**. P3 handle crosses a `wait` in the parent, never received → child completes;
  IR: `ynz_rt_spawn_handle` result stored to the parent's frame slot (`%h_ptr_i64` → `%ls_0`),
  **0 `ynz_handle_free` calls, 0 declarations**. P4 handle bound in a suspending helper called by
  `wait`, whose frame the caller's `free_frame` frees after Ready → `launch: returning`, then
  the child's remaining lines — the slot died with the frame bytes, the child kept running. P5
  statement-form spawn, parent sleeps → child completes (baseline). P6 non-suspending parent
  (`lower_let_background_handle`'s alloca branch) returns → child completes. P7 receive then
  scope exit → `42`, alloc=2 free=2, handle Box still never freed (no call site). P8 handle
  holding a command-channel reference, dropped without receive → alloc=3 free=3 (the channel is
  an `Arc`, uncounted; its handle-held reference leaks invisibly to the counter). P9 `for` loop
  re-binding `h` three times → three scope exits, all three children complete; alloc=4 free=4.
  P10 the parent is itself a `background` task (the only parent kind WITH a ladder) →
  `child: retiring` then `grandchild: done` — the ladder fired at retirement and never read the
  handle slot. **Verdict: Branch B.** A handle-only release would have to build the scope-edge
  enumeration the general pass needs verbatim (block end, loop back-edge, every `return` path,
  `errors` auto-propagation, both state-machine frame free paths) and run beside it — the second
  mechanism — and would make scope exit mean *stop the task* for handles while every other value
  at the same exit stays held. A `BG_ARG_KIND_TASK_HANDLE` arm in the parent's ladder was weighed
  and rejected (only task-parents have one; retirement ≠ scope exit; silent structured-cancellation
  semantics). **Deliverables.** Plan FR #3 replaced with the four fields; `background-handle-
  cancel-injection` `substitute`/`why`/`triggers`/`ships_in` rewritten (no milestone tags or
  internal paths); `IMP-no-function-coloring.md` "Task Cancellation" amended to current state +
  recon record; `IMP-concurrency.md` auto-close deferral carries the ruling (its drop-pass
  dependency NOT satisfied; `background-handle-close` never depended on it). Pin tests
  `crates/ynz-driver/tests/v03_m8_handle_scope_pin.rs` (2, planned-RED inverse) over fixture
  `v0_3_m8_p7_handle_scope_exit_pin.ynz` — 2/2 green, exit 0. **Design doc says A; tree does
  B:** the doc's "never silently killed mid-work" — the tree stops a still-running child at its
  next suspension when `entrypoint` returns (P1); corrected in the doc as current state, runtime
  behavior untouched. **Out-of-phase touch, flagged:** `jargon_audit`'s
  `no_banned_jargon_in_deferred_feature_user_facing_fields` was RED at `d1c4294` — Phase 5's
  `auto-arc-codegen-emission` `why` says "would alias the inner allocation" and `alias` has been
  banned since `4aef3b8`; one word changed to "share" (no semantic change) so the full-suite gate
  does not trip on it; 10/10 after. Lanes green: `ynz-diagnostics` `jargon_audit` 10/10,
  `ynz-registry` 70/70, `ynz-driver` `v03_m8_handle_scope_pin` 2/2 and `v03_m8_auto_arc` 12/12,
  `cargo fmt --check`, `cargo clippy -p ynz-driver --tests -D warnings`. No runtime or codegen
  change, so no `--release` rebuild. Resume-at `phase-7/step-3` per the dispatch brief (plan
  numbers Branch B as step 4).
- `m8-p5-fix2-20260904` — 2026-09-04 — **Phase 5 fix round 2 (`red:code-reviewer`): the R2-class
  blocker fixed at the producer, RED→GREEN; eight should-fix/minor items done; nothing committed.**
  Read: the Phase 5 STATUS block's round-1 grading, entry `m8-p5-20260904-a1` below,
  `IMP-ownership.md` "Auto-Arc", `corpses.md`, `authoritative-derivation.md`,
  `teaching-surfaces.md`, `vocabulary.md`. **The blocker.** `check.rs::admit_arc_group_for`
  classified the caller side over `&stmts[first + 1..last]` — strictly between the members — so a
  `lend` write inside a member's OWN argument list was invisible. Fix: the range is
  `&stmts[first..=last]` (the members themselves plus everything between); the walker's `Call`
  arm already returns `Writes` for a declared `lend`/`give` position and the whole-binding member
  positions classify `Reads` from the same report the task-side proof read — no second
  classifier. Fixture `m8_arc_write_in_member_arg_declines.ynz` (`background render(scene,
  results, bump(scene))` then `background render(scene, results, 0)`, `bump` declared `lend`):
  **RED** on the old range, reproduced by flipping the one line back and rebuilding — stdout
  `task saw 6` / `task saw 112` / `caller keeps Wagner 106x7` with 4 `ynz_arc_*` calls in the
  IR (task 2 read the block minted before `bump` ran); **GREEN** on the fix — `task saw 106` /
  `task saw 112` / `caller keeps Wagner 106x7`, 0 `ynz_arc_*` calls AND 0 declarations
  (`assert_declined_fixture`), alloc==free. `check.rs` restored from the scratchpad copy
  (sha256 `19e4240d…` identical before the flip and after). Note for the record: a write in a
  member's LATER argument (`render(scene, results, bump(scene))` as the SECOND member) does not
  change output either way, because both the Arc mint and the copy path snapshot argument 0
  before a later argument runs; the divergent case is the write inside the FIRST member's list
  (or any member before the last), which the fixture pins. Driver test
  `caller_side_write_inside_a_member_spawns_own_argument_declines`; the suite is 12/12 with every
  admitted group still admitted. **Should-fixes.** (1) `declared_writes_from_sigs` deleted; the
  one map `check_query` builds now rides on `EffectiveOwnershipReport.declared_writes` (set by
  `effective_ownership::analyze`, empty on `::empty()`), and `classify_binding_in_stmts` reads it
  from the report — one fewer parameter, no twin; an orphaned doc comment that had been sitting
  on the twin ("Returns true if stmt contains ANY read…", describing a function that no longer
  exists) went with it. (2) Registry: `auto_arc` `hover_what`/`hover_what_instead` and
  `auto-arc-codegen-emission` `substitute`/`why` no longer say "spawn"; `hover_why`'s "atomic
  bump" is "a tiny bookkeeping step"; `channel` added to the excluded-field list in both. (3)
  `IMP-ownership.md` Auto-Arc: every line-number cite replaced by a function name (the
  parked-21 rule; the Transfer section's four `check.rs`/`emit.rs`/`inlay_hint_passes.rs` line
  cites on the spawn-inference paragraph went the same way), "Code is cited by function name;
  line numbers drift." at the top, condition 3 rewritten to the `first..=last` range with the
  member-argument case, `channel` in the residual, parked 40 recorded as the SECOND reason
  `number` stays out, the `string` rationale softened to what the tree shows (the copy path's
  struct `memcpy` already shares the pointer; the runtime has no string release), and — the
  **audit correction**: entry `m8-p5-20260904-a1` says all four deviations were recorded in
  `IMP-ownership.md`; deviation (3), the M7-attribute premise, was NOT. It is now, one sentence
  beside the lazy-declaration fact ("Phase 5 declared them for the FIRST time … M7's own FRAGO
  001 recording that no `ynz_arc_*` symbol was declared or called from codegen"). (4)
  `inlay_hint_passes.rs` module doc: `auto_arc_hints` listed as a firing domain; the "no
  emission yet" paragraph rewritten. (5) `IMP-no-function-coloring.md` P2-6 disposition:
  CLOSED by Phase 5, topology cited, the entry narrowed not deferred; anchor `git log
  --grep=m8-p5` (resolves: `0f62869`, `561476a`). (6) `plan.md`: FR#5's placeholder replaced by
  the four fields (WHAT/WHY/COST/TRIGGER, citing the narrowed registry entry's `triggers`); the
  false M7-attribute premise annotated at Weather, Friendly forces and step 2 with the
  bracketed note, each tagged `(FRAGO 013 — Phase 5 round 2)`; prose untouched. (7) this entry.
  (8) minors: `shape_types.rs::llvm_field_type` carries the cross-reference to `arc_shareable`
  ("a new inline-stored field kind must opt in"); parked 16 pinned by
  `test_inlay_hint_background_handle_form_renders_the_inferred_give_label` — which found that
  `inlay_hint_passes.rs::collect_background_ownership_hints_block` never visited the handle form
  (`let h = background f(v)`), so the label parked 16 restored had no reader; the walker now
  matches both spawn forms (28/28); the parity test `check::suspends_parity_tests::every_
  suspending_fn_has_a_stmt_the_conservative_predicate_flags` sweeps every standalone driver
  fixture (≥100 files, ≥50 suspending functions, asserted non-vacuous) and asserts each
  fixpoint-suspending function has a statement `stmt_may_suspend_conservative` flags — it
  FAILED first on `v0_3_m2_background_subexpr_error.ynz` (`background add(inner(), 4)`: the
  predicate's `Expr::Background(..) => false` leaned on typeck's rejection of a suspending
  spawn argument), so the predicate now walks a spawn's ARGUMENTS (never the callee) and is a
  true over-approximation on every input; on an accepted program it returns exactly what it
  did. **Verification (exit codes seen):** `cargo fmt --all --check` clean; clippy `-D warnings`
  clean on `ynz-typeck`/`ynz-codegen` libs and the touched `v03_m8_auto_arc`/`inlay_hint` test
  targets (`ynz-typeck --tests` red only in untouched `tests/check.rs`/`tests/inlay_hint_passes.rs`
  — parked 31); `ynz-typeck --all-targets` green (99 lib incl. the parity test); `ynz-driver
  --test v03_m8_auto_arc` 12/12, `v03_m8_channel_close` 31/31, `integration` 530/530;
  `ynz-lsp --test inlay_hint` 28/28; `ynz-registry` green; `ynz-runtime --lib` 110/110; loom
  lane 9/9 exhaustive, non-vacuous; `ynz-driver` + `ynz-lsp` rebuilt `--release`. **Deviations
  from the dispatch:** none in substance; the parity minor was well under an hour so it landed
  rather than parking, and it moved one line of production code (the `Background` arm) to make
  the link hold on every input. **Found, out of scope:** an explicit `scene.copy()` at argument 0
  and the inferred copy both snapshot before a later argument runs, i.e. `f(scene, bump(scene))`
  hands the task the PRE-bump value while a plain (non-`background`) call would see the
  post-bump value through the pointer — a left-to-right-evaluation question for the
  language design, not an Arc defect; noted for the conductor, no fix attempted. **No commit; no
  grep token minted.**

- `m8-p5-fix1-20260904` — 2026-09-04 — **Fix round 1: two clippy dead-code issues from parallel tests. Item 1:** `arc_strong_count` / `ARC_BLOCK_FREES` cfg gates — `arc_strong_count` changed to `#[cfg(test)]` (only test callers); `ARC_BLOCK_FREES` unchanged (correctly gates only the test observer line, line 143). Item 2:** `v03_m8_auto_arc.rs` counter file race — `ynz_run_counted` now uses `tempfile::NamedTempFile` for per-call uniqueness; `read_to_string` and counter parse failures now loud (no masking on missing file). Tests run 5×, 11/11 each, exit codes collected.

- `m8-p5-20260904-a1` — 2026-09-04 — **Phase 5 executed in one segment: the Auto-Arc emission
  (topology (B)) is live for the full beneficial-emission condition; nothing committed.** Read:
  the Phase 5 block (FRAGO 010 rewrite), R2's RISK OVERRIDE, `IMP-ownership.md` "Auto-Arc",
  this file's Phase 2 SIGN-OFF + FRAGO 010, `arc.rs`, `effective_ownership.rs`, `emit.rs`'s
  bg-arg path + both spawn lowerings, `runtime.rs`'s ladder + release protocol, `ynz-abi`,
  the registry entries, `inlay_hint.rs`, parked 16, Phase 3's loom harness, `corpses.md`.
  **Step 1:** sign-off confirmed. **Step 2 spike GREEN** on `m8_arc_two_spawn_group.ynz`
  (kept as the first real fixture): RED = correct output with zero `ynz_arc_*` calls; after =
  1 new / 2 clone / 1 transient free in IR, alloc=6 free=6 vs one-spawn 4/4 (one counted block
  replaced two copies — +2, not +3), all Arc calls in `sm_s0` before the first suspension,
  `%arc_new` an SSA local never in a frame slot. **Step 3:** `admit_arc_group_for` +
  `record_spawn_arg_ownership` (the ONE recording function all three sites share — parked 16
  done) + `stmt_may_suspend_conservative`/`stmt_contains_return` boundaries + `arc_shareable`;
  codegen's `Arc` arm in `prepare_bg_arg_for_ctx`, `release_pending_arc_transients` after
  both spawn calls, `BgArgFreeKind::ArcShape`, lazy `arc_decls` (so the one-spawn IR is
  byte-identical: sha256 `4a812358596c3c01…` before and after — verified with `cmp`); the
  ladder's `BG_ARG_KIND_ARC_SHAPE` arm; **(h) = `false`** with the co-owner-staged parity
  case. **Step 4:** `auto_arc_hints` + LSP domain 11 + 3 tests. **Step 5:** the four-task
  hammer fixture (105000, alloc=free). **Step 6:** 11 driver tests; loom
  `loom_arc_group_clone_per_task_then_transient_release_frees_exactly_once` — 152
  interleavings exhaustive in 4 ms, driving the real `arc.rs` (its `AtomicU64`/`fence` now
  come from `crate::sync`; `ARC_HEADER_SIZE` rounds loom's wider atomic; production stays 8);
  **revert-proof:** dropping task 2's `ynz_arc_clone` → assertion "task 1: the shared block
  was freed while this task still holds a reference" (left 1, right 0), never a crash; tree
  restored from the scratchpad copy, sha256 `b1628457c5239502…` identical before/after.
  **Step 7:** `auto-arc-codegen-emission` narrowed to the residual, `auto_arc` hover updated.
  **Step 8 (affected lane, exit codes seen):** `ynz-driver --test v03_m8_auto_arc` 11/11,
  `--test v03_m8_channel_close` green, `integration::examples_basics_runs_end_to_end` green
  (golden regenerated; the eight nondeterministic pirate lines kept in committed order),
  `cross_impl_consistency` green with the ten new fixtures in the corpus, `ynz-lsp --test
  inlay_hint` 27/27 (+ hover, array_to_fixed), `ynz-abi` 1/1, `ynz-runtime --lib` green
  (arc ×3, per-kind parity with the Arc kind), `ynz-typeck --lib` green (arc_shareable ×3),
  `ynz-registry` green; `cargo fmt --all --check` clean; clippy `-D warnings` clean on every
  touched lib and on the touched test targets (`ynz-typeck`'s pre-existing test-file debt in
  untouched files stays — parked 31). `ynz-driver` + `ynz-lsp` rebuilt `--release`.
  **Design-says-A / implementation-has-B (all recorded in `IMP-ownership.md`):** (1) "inline
  `shape`" fields are shareable → the tree stores nested shape fields as pointers
  (`llvm_field_type`), so they are EXCLUDED; (2) no early-exit rule in the signed list → a
  reachable `return` between spawns is a group boundary (a leaked count otherwise); (3) the
  plan's "attributes M7's audit confirmed on `ynz_arc_*`" → M7's own R1 row records that NO
  `ynz_arc_*` symbol was declared or called from codegen; this phase declares them (lazily)
  for the first time; (4) the hover `{n}` → the registry hover is static markdown, so the
  count is in the label and the hover says "the tasks spawned here". **Out of scope, found:**
  a shape with a `number` field SIGSEGVs on ANY `background` spawn (one spawn, zero Arc
  involvement, copy path) — parked item 40 with the repro. **No commit; no grep token
  minted** (this entry is the durable record; the conductor's seal carries the dispatch id).

- `m8-p4-fix3-20260904` — 2026-09-04 — **Phase 4 fix round 3: the blocker closed, two
  producers, both fixed; seven should-fix/minor items done; nothing committed.** Read: the
  Phase 4 block's round-2 grading, `REF-errors.md:155-180` (the `.message` contract),
  `teaching-surfaces.md`, `corpses.md`. **The blocker — two producers, both named upstream.**
  `check.rs`'s `check_errors_capable_method`/`infer_field_access` typed `.message`/
  `.suggestions`/`.trace`/`.source` as their success type UNCONDITIONALLY — REF-errors.md:171-
  175 requires a `.failed()` check first, and nothing enforced it. Fix: a new flow-sensitive
  set, `errors_failed_true_branch: Vec<String>`, pushed by `check_stmt_if` right before
  checking an `if (x.failed())` body and popped right after (block-scoped, unlike the existing
  function-scoped `errors_consumed`/`errors_success_narrowed` auto-propagation bookkeeping,
  which answers a different question); a new shared gate,
  `check_errors_field_needs_failed_check`, consulted from BOTH the `Expr::MethodCall` dispatch
  (`check_errors_capable_method`, the parenthesized form) and `Expr::FieldAccess` dispatch
  (`infer_field_access`, the real dot-postfix form) so the two call forms cannot drift on the
  rule. A new registry `[[diagnostic_template]]` + `DiagnosticKind::MessageBeforeFailedCheck`
  renders it, `{name}` filled from `expr_source_text(receiver)` (works for both a bound
  identifier and an anonymous call result — `readFile().message` is always refused, since
  `extract_failed_binding` only recognizes bare-ident receivers, so no such read could ever
  have been legally checked). Codegen (defense): `emit.rs`'s `.message` arm called
  `ynz_error_message` unconditionally then `select`ed the empty string on a null error pointer
  — LLVM `select` evaluates BOTH operands eagerly, so the call ran on a null pointer even on
  the not-failed path and SIGABRT'd; replaced with a real `br i1 %ec_msg_failed` / `phi`, the
  call moved inside the conditional block. Since typeck now refuses every source program that
  reaches the not-failed path, that path is unreachable from source; verified instead at the
  IR level (`--emit-ir --no-optimize`, asserting no `select` and a real `br i1`/`phi` around
  the `ynz_error_message` call). **A genuinely separate pre-existing bug surfaced while wiring
  the typeck fix, named and left OUT OF SCOPE.** `resolve_ident`'s auto-propagation strips
  `ErrorsCapable` → inner on the FIRST use of a binding inside an `errors`-capable function —
  including the very `x.failed()` condition-check read itself. A compensating restore existed
  for the `Expr::MethodCall` dispatch path only (the `EC_METHODS` inline block); `infer_field_
  access` had none, so `.message` inside `if (x.failed()) {...}` in an ERRORS function ICE'd
  ("`string` values do not have fields") before this round — hoisted both restores into one
  shared `restore_ec_receiver_ty` helper, closing that half. But a SEPARATE, deeper bug
  remains, confirmed independent of every change this round makes (reproduced identically on
  the pre-round base commit `d0c46b3` via `git stash`): repeated `.failed()` checks on a
  binding INSIDE an errors-capable function both evaluate true regardless of the actual
  failure state (two `if` blocks, `raw.failed()` then `!raw.failed()`, each printing a
  distinct marker string) inside `function loadConfig() -> nothing errors` prints BOTH markers
  for an always-succeeding `raw`. Not exercised by any existing fixture — the two pre-existing
  `.failed()` fixtures
  (`v0_3_m3f_ec_failed_then_ok.ynz`, `m7_errors_failed_check.ynz`) both call `.failed()` only
  from a NON-errors `entrypoint()`. Flagged for a future FRAGO/fix round; not fixed here — out
  of this round's named scope and orthogonal to both producers above. **Corpus sweep** (task
  mandate): `.message`/`.suggestions`/`.trace`/`.source` across `crates/ynz-driver/tests/
  fixtures/*.ynz` and `examples/`: exactly one instance
  (`v0_3_m8_p4_errors_message_after_failed.ynz`), already inside `if (late.failed())`,
  legitimate, admitted unchanged (re-verified GREEN). A separate sweep of `ynz-typeck`'s own
  embedded-source Rust test corpus (missed by the fixture-only grep — caught only by running
  `cargo test -p ynz-typeck --all-targets`) found four LATENT BUGS:
  `ec_method_{message,suggestions,trace,source}_resolves_in_ec_fn` in
  `crates/ynz-typeck/tests/check.rs` asserted the UNCHECKED read compiled clean — literally the
  class this round closes. Corrected to read inside `if (x.failed()) {...}`, preserving each
  test's original EC_METHODS-restoration-coverage intent. **Should-fix / minor items 3–9, all
  done, none declined.** (3) parity-sampler duplication: built ONE exhaustive per-`Type`-
  variant sampler (`ynz_typeck::type_variant_sampler`, not `#[cfg(test)]` — `ynz-codegen`'s
  test binary consumes it across the crate boundary), consumed by BOTH `types.rs`'s
  `channel_elem_supported_names_match_the_predicate` (now asserts every non-supported variant
  is rejected, sampled once each, plus a targeted bignum-precision regression the whole-variant
  sweep can't see) and `emit.rs`'s `copy_parity_tests` (the duplicate `all_variants` + hand
  `TYPE_VARIANT_COUNT` deleted). (4) `IMP-ownership.md:277`'s stale sentence ("Typeck drops the
  modifiers at resolution… checks no ownership at all", dead cite `check.rs:5391`) rewritten to
  current state — `ContractSigDef` DOES carry `param_ownerships`/`receiver`
  (`shapes.rs`), the dispatch site is `check.rs:~6153-6209`, calling `check_call_transfers`
  (`check.rs:5320`) — verified by reading both sites, not inferred. (5) `copy_lowering_arm`
  classified every `Type::Number` as `ByValue`; bignum (precision > 34) lowers as a POINTER
  (`llvm_type_for`) — split the arm, bignum falls to `AliasNoOp`; `is_trivially_copyable`
  (`ynz-typeck`) split the same way so `copy_is_independent` agrees; a targeted
  `bignum_number_is_not_by_value` test added (unreachable from source today — the parser defers
  non-34 precision — pinned honest anyway). (6) `ALL_BG_ARG_KINDS`'s source parser required a
  single-line `pub const … = N;`; added a second, line-START-based marker count that survives a
  wrapped value and asserted it equals the line-parsed count (first attempt used a whole-file
  substring count and self-matched the new assertion strings' own quoted text — corrected to a
  per-line `starts_with` check, which cannot self-match a comment or a string literal). (7)
  `call_argument_text` skipped only `"…"` strings; now panics loudly on an un-skipped `'` (char
  literal/lifetime) or `//` (line comment) inside a `registry_diag(...)` argument list rather
  than silently mis-scoping — verified none of the real call sites trip it. (8)+(9) the
  `Consumed` template's comment (`registry/features.toml`): finished the truncated `{via}`
  sentence and trimmed the decision narrative (parked items 7/8, "move"-wording retirement) to
  current state + a `git log --grep=m8-p4` anchor per `decision-records.md`. **Verified** (exit
  0 each): fmt; clippy `-D warnings` on every touched lib/test target individually (ynz-
  diagnostics lib; ynz-typeck lib + `diagnostic_template_parity` test; ynz-codegen lib; ynz-abi
  lib; ynz-driver's three named test targets — the parked-31 debt in untouched files, incl.
  `jargon_audit.rs`, stays, confirmed still present and still out of `-D warnings`' blast radius
  when scoped correctly); `ynz-typeck --all-targets` (222 in `check.rs`, 95 lib, 13
  `diagnostic_template_parity`, the rest unchanged — all green after the four latent-bug
  fixture fixes); `ynz-codegen --lib` (17, 2 new); `ynz-abi` (1); `ynz-diagnostics --all-
  targets` (unchanged, `jargon_audit` passes as a TEST target — its clippy debt is a lint-only
  gap); `ynz-driver` `v03_m8_channel_close` 31 (2 new), `error_galleries` 10 (1 new phrase,
  count ceiling 36→37), `integration` 530; `ynz-runtime --lib` 110 + the loom lane 8 (both
  untouched by this diff, confirmed green); `ynz-driver --release` rebuilt. Demo golden
  unchanged (pirates-roster never reads `.message`).

- `m8-p4-fix2-20260904` — 2026-09-04 — **Phase 4 fix round 2: five blockers, three producers,
  all closed; eight should-fixes and four minors done; nothing committed.** Read: the Phase 4
  block's round-1 grading, the `m8-p4-20260904-a2` entry, `corpses.md`, root-cause /
  authoritative-derivation / teaching-surfaces / test-parallelism, `IMP-concurrency.md` "Channel
  Close…", `IMP-ownership.md` "Transfer…". **Producer A — root cause named: the renderer, not
  the emit sites.** `SourceSpan` is byte-indexed; `ariadne::Config::default()` is
  `IndexType::Char`. Measured on `m8_errors.ynz`: 164 surplus bytes over the file, +28 before
  line 19 (rendered 22:11 — exactly 28 chars past the trigger), +90 before line 92 (→ 95); the
  m6/m7 galleries carry 26/116 surplus bytes and were off too, so this was never a Phase 4
  regression. The past-EOF drop (A2) is the SAME producer: a byte offset past the char count
  makes `get_offset_line` return `None` and ariadne `continue`s past the label, taking the note
  with it. Fix: `.with_index_type(IndexType::Byte)` + `clamp_to_source` (a span past the end is
  pinned to the last byte so the teaching block always renders). RED: `byte_spans.rs` 3 tests
  FAILED against the old `render.rs` (stash/pop run) → GREEN 3 passed. Gallery: the reorder
  workaround and its comment removed; `m8CloseOnHandle` deliberately last; the gallery test now
  fails if any caret line lands inside a `// WHY:` comment and asserts the close-with-args
  WHAT-INSTEAD renders. **Producer B — path taken: the codegen arm.** `REF-errors.md` defines
  `.message` (":160 error description (only valid after `.failed()` check)"), so the spec's
  example is legitimate and the ICE was a missing arm. `lower_field_access` gained the
  `ErrorsCapable`/`"message"` arm reading `{i64 err_ptr, i64 ok}` exactly as `.failed()` does,
  `ynz_error_message` → the null-terminated bytes (a Yinz `string` at the ABI), `select`ed
  against the empty string when `err_ptr == 0`. Other EC fields error explicitly instead of
  falling to `field_gep`. RED: `field_gep: receiver is not a Shape, got ErrorsCapable` → GREEN
  prints the closed-send message. **Producer C1 — chosen shape: the pre-call snapshot, not a
  two-pass reclassification.** A two-pass "classify all, then consume all" cannot see the alias
  pair at all (both positions are admitted individually; the defect is that they share a class),
  so the ONE fact that distinguishes "consumed before this call (reported at inference)" from
  "consumed by this call (reported nowhere)" is the set of consumed classes at call entry:
  `Scope::consumed_classes()` taken once in `check_call_transfers` (and right before each send
  arm's single `check_transfer`), passed to `check_transfer`, which now renders the consumed
  read when the class is absent from the snapshot — never an early return. The rendering is
  ONE function, `consumed_read_diag`, with two callers (`resolve_ident`, `check_transfer`/
  `check_read_of_same_call_consumed`); `ConsumedBy::Given { callee, given }` mirrors `Sent`,
  and the `Consumed` template gained `{via}` (" — it shares its value with `rows`, which is what
  was given away") + `{given}` in WHAT-INSTEAD. Non-`give` positions (`share`/`lend`/bare) run
  the same read check so `mix(give a, share b)` is caught. RED: give/give printed `3 3`,
  give/share printed `3 3`, background compiled → GREEN all three refused with the `{via}`
  form; `eat2(rows, rows)` renders the empty-`via` form; a class consumed BEFORE a call is
  reported exactly once (typeck test). **C2:** `emit.rs` conduit send/receive now ask
  `channel_elem_drop(..) == Some(ChannelElemDrop::NumberCell)`; codegen swept — the other
  `Type::Number { precision <= 34 }` tests are ABI/i128-storage decisions, not channel-element
  ones (the `elem_ty2` test at the array-literal lowering is SM decimal-global staging). **Should-
  fixes:** (1) 30 gallery fns → camelCase, `error_galleries.rs` phrases updated; (2) the
  `IMP-concurrency.md` CLOSED-first-poll paragraph rewritten to shipped state, anchor `git log
  --grep=m8-p4` (resolves: `2be2244`, `6b8a34d`); (3) the registry `why` no longer carries the
  milestone tag or the internal path; (4) `REF-ownership.md:83` comment; (5) the oracle resolves
  `let` locals from an annotation, a literal, or a builtin whose registry `return_type` is the
  same scalar on every receiver (`count` → `int`), joining all bindings of a name (disagreement
  or a `for` var → `None`, a same-named parameter is one more candidate) — RED refused → GREEN
  `1\n3\nend`, alloc gap 4 observed; (6) `copy_lowering_arm` is THE classification the Copy
  lowering dispatches on (no `_` arm over `Type`), `copy_parity_tests` holds it to
  `copy_is_independent` over one sample per variant, a variant count pins the sample list —
  there was no prior test, so RED is "absent"; (7) tier 2 parses each `registry_diag(` call's
  balanced argument list for `DiagnosticKind::X` (string literals skipped); the
  `NotDefined | UnusedImport` exemption is gone; (8) `IMP-ownership.md:277` now states the
  receiver rule the code enforces; (9) n/a — A2 landed, no `parked.md` entry. **Minors:** tag
  "given away"; `—` in `closed_msg` (+ the two doc quotes); "Use one of" rendered from
  `CHANNEL_ELEM_SUPPORTED_NAMES` with `channel_elem_supported_names_match_the_predicate`;
  `every_bg_arg_kind_const_is_in_all_bg_arg_kinds` (source-parsing test in `ynz-abi`). **Dead
  code removed:** `collect_let_names` (its only caller was the old oracle). **Runs (exit 0
  observed):** `cargo fmt --all --check`; `ynz-typeck --all-targets`; `ynz-diagnostics
  --all-targets`; `ynz-abi`; `ynz-codegen --lib copy_parity`; `ynz-driver` `v03_m8_channel_close`
  (29), `error_galleries` (10), `integration` (530); `ynz-runtime --lib` (110); loom lane (8);
  clippy `-D warnings` on `ynz-typeck --lib --test diagnostic_template_parity`, `ynz-codegen`/
  `ynz-abi` `--lib --tests`, `ynz-diagnostics --lib --test byte_spans --test snapshots`,
  `ynz-driver --bins --test v03_m8_channel_close --test error_galleries`; `cargo build -p
  ynz-driver --release`. Clippy on `ynz-typeck`'s lib-test target and the older typeck test
  files, and `ynz-diagnostics/tests/jargon_audit.rs`, is red on parked-31 debt in files this
  round never touched (`independence.rs` `susp` ×5, `iterables_typeck.rs`, `builtins.rs`,
  `inlay_hint_passes.rs`, `generics_typeck.rs`, `check.rs` tests). Demo golden untouched.
  **Deviation:** none from the brief's steps; one judgment call — C1 was fixed with a snapshot
  rather than the brief's suggested two-pass reclassification, for the reason above.

- `m8-p4-fix1-20260904` — 2026-09-04 — **Green-check fix round: cargo fmt + clippy manual_contains.** All tests GREEN; fmt --check exit 0; clippy exit 101 with reported-pre-existing at consistency.rs:6.
- `m8-p4-20260904-a2` — 2026-09-04 — **Phase 4 segment 2: the implementation — all 24 RED fixtures
  flipped GREEN, steps 2–8 done, the phase is complete pending conductor review.** Read in full:
  `handoff-phase-4.md`, the Phase 4 block, the two signed designs (`IMP-concurrency.md` "Channel
  Close — End-of-Stream Semantics", `IMP-ownership.md` "Transfer — Who Else Holds This Value"),
  this file's FRAGO 002/005/006 and the Phase 2 SIGN-OFF, `parked.md` 5/7/8/13/15/17/27/30,
  `corpses.md`, the test-parallelism / authoritative-derivation / feature-registry /
  teaching-surfaces rules, Phase 3's `loom_tests.rs` + `sync.rs`. **Built, in the brief's order:**
  3a `ynz_map_clone` (five counted allocs, one-level copy) + the `Type::BuiltinMap` arm of
  `PostfixOpKind::Copy` + the registry `copy` on `map` + `REF-ownership.md`'s stale `.copy()`
  text → `m8_p4_map_copy_is_independent_of_the_original` RED→GREEN. 3d `ChannelElemDrop { Array,
  Map, NumberCell }` / `channel_elem_drop` / `transfers_source()` / `channel_elem_supported` /
  `copy_is_independent` in `ynz_typeck::types`; `ynz_number_cell_free`; the send mints a cell via
  `number_to_heap_cell`, the receive copies into a per-site entry-block i128 slot and frees the
  cell before the envelope; `check_channel_construction` derived from the enum; the registry
  deferral narrowed to shape + bignum; `v0_3_m4_errors.ynz` swapped to `channel<BatchNote>`.
  3b the transfer rule: `effective_ownership::analyze` hoisted above `check(...)` in `queries.rs`
  and given a type resolver; `Provenance` + `provenance()` (exhaustive `Expr` match, no wildcard);
  `consumed[fn][i]` (declared give ∨ passed whole to a consumed position ∨ sent whole with an
  owned-heap DECLARED type) and `returns_fresh[fn]` (`Freshness::MayAlias { param }` names the
  reached parameter) in the same fixpoint loop; `stmt_rebinds` → `Writes`;
  `classify_binding_in_stmts`; `scope.rs` `ConsumedBy { Given, Sent { channel, sent } }`,
  `Origin { Owned, Param(m), Cell(reason), Reaches(reason), Unknown }`, alias-class ids,
  `Scope::consume` class-wide, `visible_members_of`; `check.rs` `binding_event_origin` (the ONE
  rule `check_let` AND `check_assign` call — leave class, clear consumed, join), ONE
  `check_transfer` fed by `check_call_transfers` over the normalized `[receiver, args…]` list at
  the plain call, the generic call, the UFCS dot-call (receiver AND non-receiver arguments), the
  `dynamic Contract` dispatch (via `ContractSigDef.param_ownerships`/`receiver`) and both conduit
  `send` arms; the `background` liveness inference class-aware (`Give` iff `Owned`/`Param(give)`
  and no class member read after; `Copy` still RECORDED); the consumed-read site renders
  `Consumed`/`ConsumedBySend` FROM the registry (`registry_diag`); `ParamNeedsGive` retires the
  `:4611` share refusal and the `:4617` silent consume; `TransferNeedsCopy`'s `{reason}` from the
  binding event; the const refusal extracted with a sink-supplied WHAT-INSTEAD;
  `root_binding_name` collapsed onto the module's; `follows` ownership parity. 2/3/4b `close` at
  the three typeck sites (known set, the two guards gated to suspending methods, the
  unknown-method string split per receiver), bare `receive()` → `maybe<T>` built at
  `conduit_post` into ONE `alloca_in_entry_llvm` envelope per site (closed paths → `none`, no
  error value, no abort), `ynz_channel_close` (take under the sender lock, drop outside it, wake
  every waiter) + `Ready(None)` drains waiters + `sender: Mutex<Option<..>>`; the 19-site
  rewrite (11 runnable fixtures + the two typeck acceptance tests; the expected-error
  `xmod_return` fixture untouched), `pirates-roster` (three sites + a new `m8_demo` section,
  golden regenerated byte-exact), `REF-concurrency.md` ("Closing a channel", "Sending an array
  or a map gives it away", the retyped examples, `number` in the element list). The SIGSEGV
  producer: `HANDLE_RET_KIND_VALUE_MAYBE` (abi 6) — the resume fn already stores the `{flag,
  bits}` pair inline in the return slot; `extract_completion` copies the 16 bytes exactly as
  `VALUE_NUMBER` does; the `ret_kind` match lists its word types by name and refuses any other
  aggregate. 3c `HandleChannelArgNeedsBinding` at the handle-form spawn (position of the
  callee's first `channel<T>` parameter; hard error). 4 `refuse_closed` — ONE closure for the
  three first-poll CLOSED arms (`None` under the lock, `try_send → Closed`, `Full` then first
  poll `Ready(Err)`), lock dropped before it runs (parked 15); doc comment + case (c) flipped;
  new runtime gates `closed_first_poll_send_frees_the_refused_payload_once_alloc_free_parity`,
  `close_is_idempotent_drains_then_closed_and_wakes_parked_receiver`,
  `in_flight_send_parked_before_close_still_lands`; `bg_arg_kind_is_releasable_payload` +
  `ALL_BG_ARG_KINDS` in `ynz-abi`, `release_ladder_payload` reads the predicate, the ladder's
  `_ => {}` arm `debug_assert!`s it, `every_bg_arg_kind_is_releasable_iff_the_ladder_frees_it_
  alloc_free_parity`. Loom: `loom_close_vs_send_linearizes_at_the_sender_lock_clone` (57
  interleavings; a send holding a clone LANDS, a refused one is glued once, never "refused after
  close returned") and `loom_refuse_closed_releases_the_ladder_slot_then_glues_exactly_once` (57;
  a real counted cell under a HEAP_SHAPE ladder slot). **Revert-proofs** (each: mutate, run the
  model, restore, sha256 `d09b9c9e…42ac3` before == after): (1) `close()` draining the buffer
  through the glue ("close discards what was on its way") → the model's own "a send that cloned
  before the take must LAND" fires; (2) `refuse_closed` gluing without `release_taken_value()`
  → "refuse_closed must release the ladder slot BEFORE gluing" fires. `param_ownership`
  (schema field, `build.rs` validation of values and alignment — parked 5, hover line in
  `lsp_adapter.rs`), `[[primitive_intrinsic]]` `send`/`receive`/`close` on `channel`, the three
  `[[deferred_language_feature]]`s (`channel-auto-close-on-last-producer`,
  `background-handle-close`, `maybe-move-out`), the four `[[diagnostic_template]]`s + the
  `Consumed` reconciliation (parked 7/8; the "move" word retired) + the parity test
  (`crates/ynz-typeck/tests/diagnostic_template_parity.rs`: every template kind_name classified;
  variant-backed templates rendered from the registry or on a shrinking ratchet of the eleven
  PRE-EXISTING hand-written kinds — named, not silently absorbed; the five kinds this phase owns
  rendered end-to-end). Step 6: the M6 "Bare `channel<T>` never closes" divergence entry
  replaced by a pointer. **Deviations / "design says A; compiler has B":** (i) the two `number`
  fixtures were authored with `got.or(0)` — an int literal into a `number` slot, refused by a
  SHIPPED deliberate gate (int→number coercion deferred) — corrected to `.or(0.0)`; expectation
  unchanged. (ii) Four hotfix fixtures (`bg_arg_channel_send_array`, `…_never_drained`,
  `bg_arg_handle_send_array`, `bg_arg_two_arrays_send_one`, added at `861fd4d` AFTER the design's
  corpus scan) were relay-through-bare-parameter instances — exactly the `ParamNeedsGive` class
  — corrected to `give` (stdout and alloc-gap pins unchanged). (iii) The pre-existing
  maybe-cannot-cross-a-suspension limit (`UnsupportedCrossingLocalType`; handoff gap 3) shaped
  `bg_arg_handle_send_array`'s sink: a `maybe` bound from a receive cannot be read inside an `if`
  whose body suspends — the values are pulled out before the `report.send`. Design says the
  consumer reads `.exists()/.value` freely; the compiler has this limit; not hacked here, stands
  as a known gap. (iv) `follows` parity on the receiver: the parser folds a bare contract `self`
  and "no self" into `receiver: None`, so the receiver check enforces the safety half (a bare
  contract receiver is never a give position) and exact modifier parity on the remaining params
  — design says "bare matches bare" exactly; the parser cannot express that for the receiver.
  (v) The m8 gallery's trigger ORDER was changed twice: the diagnostic renderer drops the span
  block of the last diagnostic in the file (a pre-existing renderer quirk — its WHAT prints, its
  WHAT-INSTEAD does not), so the two triggers whose asserted phrase lives in the WHAT-INSTEAD were
  moved off the end. (vi) The demo's refused-send line was first printed by the background
  producer — a race with the consumer's prints under full-suite load (one flaky run observed) —
  moved to the spawner after the consumer loop ends; deterministic now. (vii) `keys`/`values`/
  `entries` were NOT added to `builtin_method_returns_fresh` — they have no codegen lowering
  ("not yet lowered in P4b"), so no evidence; only `receive` is in the set. (viii) The `Part 2`
  transitive share violation still fires beside `ParamNeedsGive` for `m8_param_needs_give_share`
  (a declared give position is `Writes`) — two diagnostics on one line, pre-existing behavior,
  counted in the gallery bound. Tests run (all exit 0 observed): `ynz-driver` `v03_m8_channel_close`
  (24), `error_galleries` (10), `integration` (530), `cross_impl_consistency` (7, 163s);
  `ynz-runtime --lib` (110); the loom lane (8); `ynz-typeck` (all targets); `ynz-registry`;
  `ynz-lsp`; `cargo clippy` on the touched crates' libs clean (two PRE-EXISTING test-target clippy
  failures noted: `ynz-registry/tests/consistency.rs:6` redundant import, `independence.rs`
  unused `susp` — not mine, not touched). Release: `cargo build --release` for `ynz-runtime`,
  `ynz-driver`, `ynz-lsp`. `handoff-phase-4.md` deleted (phase complete).

- `m8-p4-20260904-a1` — 2026-09-04 — **Phase 4 segment 1: the RED seal — 24 fixtures, one named
  test each, the m8 gallery's Phase 4 section, the SIGSEGV precondition root-caused; no compiler
  code.** Read in full: the Phase 4 block (steps 1–8, 3a–3d, 4b, CHECKPOINT); `IMP-concurrency.md`
  "Channel Close — End-of-Stream Semantics" (all subsections through "What is signed off");
  `IMP-ownership.md` "Transfer — Who Else Holds This Value"; this file's FRAGO 002/005/006/007,
  both SIGN-OFF records, the `conductor-2026-09-03-m8-execution` entry; `parked.md` 5/7/8/13/15/
  17/27/30; `corpses.md`; the test-parallelism, authoritative-derivation, feature-registry and
  teaching-surfaces rules. Code read: `check.rs` (`check_conduit_method_call`,
  `check_channel_construction`, the crossing analysis `collect_crossings_in_stmts` and the
  `UnsupportedCrossingLocalType` guard), `scope.rs`, `channel.rs` (`channel_send_poll_guarded`,
  case (c)), `emit.rs` (`channel_drop_glue`, `lower_let_background_handle`), `ynz-abi`
  `HANDLE_RET_KIND_*`, `lib.rs` (`ynz_array_clone_primitive`, `YnzMap`), the registry's
  `Consumed`/`copy`/`channel-element-heap-upgrade` entries, `integration.rs`'s alloc-counter and
  handoff-parity helpers, `error_galleries.rs`, `cross_impl_consistency.rs`'s exclusion lists.
  **Probes (12, run in the dev container from `target/probe/`, deleted):** the
  `h.receive()`-on-`-> maybe<T>` SIGSEGV reproduces deterministically for a plain `maybe<int>`
  return on the handle form consumed in an `errors` function (exit 139); the `maybe<int> errors`
  twin prints `7`; the `wait pick()` form prints `7`. Root cause named in `handoff-phase-4.md`
  (`lower_let_background_handle`'s `ret_kind` match has no `maybe<T>` arm → `VALUE_WORD`). Three
  other findings from the probes, all recorded in the handoff rather than absorbed: Yinz has no
  standalone `else` block (the signed design's consumer-loop sketch uses one — a "design doc says
  A; language has B" the docs step must correct); `r.or(none)` does not type-check even under an
  annotation; a pre-existing crossing-analysis over-approximation (a `let` after a suspension is
  `declared`, and any later read is a crossing even with no further suspension) hard-rejects some
  `maybe<T>` consumer shapes. **RED run** (`cargo test -p ynz-driver --no-fail-fast --test
  v03_m8_channel_close --test error_galleries m8`, exit 101): 23 FAILED / 1 ok in the new file;
  the gallery FAILED on count (15 observed vs 26–36) and on every new-diagnostic phrase. Each
  failure's first line is tabulated in the handoff; every one is the intended reason (unknown
  `close`, `.exists()` on the bare `T`, `channel<number>` rejected at construction, the false
  "already given away", the alias no-op's `copy b: 99`, signal 11). The gallery output also
  confirms the seven 2026-09-03 probe holes are live on this tree (no error today for the UFCS
  non-receiver give, the three alias forms, or read-after-send). Tests run: only the scope above.
  Deviations: none from the plan's text; the segment stops before step 2 by the dispatch's own
  RED-commit protocol. Handoff: `handoff-phase-4.md`, resume-at `phase-4/step-2`.

- `m8-p3-fix1-20260904` — 2026-09-04 — **Phase 3 fix round 2 (`red:test-quality`): the kind-2
  drop-ladder purge→release ORDER made an asserted property; four should-fixes answered.** Read:
  the Phase 3 block's round-1 grading, the `m8-p3-20260903-a1` entry (its revert-proof clause
  "ladder purge/free ORDER swapped dies with SIGSEGV … named for the sanitizer lane, not a loom
  finding" is SUPERSEDED by this round — the loom models now report the swap themselves), `loom_tests.rs`,
  `channel.rs` (`mpsc_witness`/`mpsc_step`/`purge_pending_sends`/the parked-arm sweep), the kind-2
  arm (`runtime.rs:1143-1154`), `test-parallelism.md`, `corpses.md`. **Producer named:** both models
  observed the purge's *effect* and the release's *effect* after the joins, never the *state between*
  them. Fix: `assert_purged_before_released(chan, ladder_key)` — a co-owner that still holds its own
  reference asserts `strong_count == 1 ⇒ !pending_send_contains(frame_ptr, task_gen)`; both reads are
  loom-tracked (`Arc` count, `pending_sends` mutex) so the probe is a preemption point loom places at
  every step of the ladder, and the release-before-purge window becomes an explored, asserted state.
  Wired into both kind-2 models (`pending_send_contains` added under `cfg(all(test, loom))`).
  **Revert-proof** (throwaway script in the session scratchpad; kind-2 arm's two calls swapped, tree
  restored, `git diff` sha256 `64edaba2…fb86` before = after; a second Miri pass `89e4b71c…4383`
  before = after): live-co-owner → panicked at `loom_tests.rs:415` with the ORDER message (round 1:
  passed clean, 12 interleavings); last-reference → same assertion (round 1: SIGSEGV); deterministic
  test → plain debug build SIGABRT from rustc's misaligned-pointer UB check inside the dangling
  purge; Miri → UB at `channel.rs:756` (`purge_pending_sends`) from `runtime.rs:1153`. **Deviation,
  stated:** the dispatch asked for all three to fail by assertion; the deterministic last-reference
  test cannot in any build — its swapped failure is a use-after-free inside `drop(fut)` before any
  assertion runs, and the only pre-corruption observation point (the element glue) is behind an
  `extern "C"` boundary that cannot unwind (every ABI shim on that path is `extern "C"`, so a
  `cfg(test)` hook could not panic either — none added). It is the sanitizer lane's finding, which
  is the lane the dispatch named for it. New test: `lib.rs::m6_pending_send_aba::
  ladder_holding_last_reference_purges_parked_send_before_channel_teardown` (main releases first;
  asserts glue sequence `[parked, filler]`, each once) — passes plain and under Miri (1 passed).
  **Should-fixes:** (1) `mpsc_step()` before both `retain`s that drop parked `Send` futures;
  interleavings orphan_frame 3→9, orphan_handle 3→9, ladder_last 9→27, ladder_live 12→57 (probe +
  witness together), aba 987→11,079, recv 42,563 (no Tokio-future drop on that path); lane 1.53s→1.71s.
  The count moved, so the witness measures something. (2) CI loom step: `shell: bash` +
  `set -euo pipefail`; Patrick's 2026-09-03 BLOCKING ruling recorded in the comment; no
  `continue-on-error`. (3) `IMP-concurrency.md` "one import site" scoped to `channel.rs`/`handle.rs`,
  `RUNTIME`'s `std::sync::Mutex` named as the `sync.rs` exemption; the revert-proven clause gains the
  ladder-order fix. (4) Bounded CI run NOT added — documented in the step comment: all six models
  complete exhaustively unbounded and a preemption bound only prunes that exploration, so
  `LOOM_MAX_PREEMPTIONS=2` would be a strict subset. **Production:** `runtime.rs` untouched; every
  `channel.rs` addition is `cfg(loom)` / `cfg(all(test, loom))`, so round 1's byte-identical proof
  stands without re-running. Clippy clean plain and `--cfg loom` (cargo exit 0 captured explicitly —
  an earlier attempt piped through `grep|tail` and reported the pipe's status, the exact CI defect
  of should-fix 2, redone); `cargo fmt --all --check` clean. No commit (conductor seals).

- `m8-p3-20260903-a1` — 2026-09-03 — **Phase 3 executed end to end (loom substrate), one segment,
  no handoff.** Spike GREEN on both STOP halves (4,518 interleavings / 153 ms unbounded; the
  reintroduced unsalted-key ABA reported by the model's own assertion). Production no-op proven by
  a pre/post single-CGU LLVM-IR diff: 0 lines after masking only `core::panic::Location` line/col
  bytes and `@alloc_*` content-hash names (raw diff = 78 lines, all Location constants); 0 instruction
  lines differ across the staticlib's disassembled `.text`. Landed: `crates/ynz-runtime/src/sync.rs`
  (`Arc`/`Mutex`/`MutexGuard` re-export shim, std in every non-loom build), `channel.rs`/`handle.rs`
  imports swapped, `CURRENT_DRIVE` cfg twins, loom-only `YnzChannel::mpsc_witness`, six models in
  `src/loom_tests.rs`, `[target.'cfg(loom)'.dependencies] loom = "=0.7.2"` + `check-cfg` lint
  declaration in `ynz-runtime/Cargo.toml`, a `Loom` CI step. Teeth: five reverts run in the working
  tree and restored (git-diff sha256 identical before/after each) — ABA, orphan (both producers),
  ladder purge removed, recv poll-then-record all reported by loom assertions; ladder purge/free
  ORDER swapped dies with SIGSEGV (a memory-safety class, named for the sanitizer lane, not a loom
  finding). Two harness bugs found by the teeth runs and fixed at the producer: a process-global
  glue log shared by libtest's parallel threads (per-iteration payload tagging) and a post-join probe
  that re-woke the lost receiver (wake counts snapshotted first). One hypothesis retracted in-session:
  the `mpsc_witness` was added believing DPOR could not see the recv race; the revert is caught
  without it — the witness stays for exhaustiveness over Tokio-call orderings with a corrected,
  measured rationale on `mpsc_step` (2,985 → 42,563 interleavings). Scoping: handle-side P2-7 is a
  panic-robustness property, not an interleaving, and is not a loom model. Throwaway spike file
  deleted. Deviations from the step text: step 4's `runtime.rs:591-693` citation navigated by function
  per FRAGO 002 (`SpawnStateFnFuture::drop`'s kind-2 arm); "dev-dependency / cargo feature" in
  Sustainment replaced by the target-cfg dependency (lib code under `cfg(loom)` needs the crate; a
  feature would be consumer-enableable) — plan text corrected. No commit (conductor seals).

- `m8-p2-signoff-fix1-20260903` — 2026-09-03 — **Phase 2 sign-off fix round (docs only, six edits).**
  Plan step 6 no longer attributes `false` to packet item (h) — obligation + design leaning only.
  Plan's Teaching subsection repoints the stale `ConsumedBySend`-lives-in-`IMP-concurrency.md`
  sentence to its one real home, `IMP-ownership.md`. `IMP-concurrency.md`'s `HandleChannelArgNeedsBinding`
  paragraph now says hard compile error (FRAGO 009 ruling 2), not "Patrick's call." `IMP-ownership.md`'s
  `maybe-move-out` bullet rewritten to the real six-field registry schema. Probe count aligned
  (eight probes, seven live-hole, `dynamic Contract` separate). Fixed prior session's self-report
  ("seven Invariants subsections" → "the four touched"). No compiler code touched.

- `m8-p2-signoff-20260903` — 2026-09-03 — **Phase 2 sign-off round (design only, no compiler code):
  Patrick's sign-off recorded in the design docs, parked 19–27 applied, and the owed downstream plan
  edits executed under FRAGO 003's standing gate.** Authority: `audit.md`'s SIGN-OFF record ("Patrick
  signed off Phase 2") — every edit traces to one of its twelve packet items or its top-level ruling;
  full enumeration in the new FRAGO 010 entry below. Read in full: the SIGN-OFF record, the Phase 2
  STATUS block's owed-edits list, `.claude/plans/parked.md` items 19–27, `IMP-ownership.md`
  "Transfer"/"Auto-Arc," `IMP-concurrency.md`'s channel-close section (including "Two mechanisms, one
  rule" and fr12), `.claude/rules/plan-invariants.md`, and `~/.claude/docs/reference/REF-plan-format.md`.
  **Docs:** both `IMP-*.md` sign-off markers converted to current-state text with a one-clause anchor
  (`decision-records.md` discipline — no narrative, no invented commit-grep pointer where none exists
  yet: `audit.md`'s SIGN-OFF record is the anchor, not a fictional `git log --grep=m8-p2-signoff`,
  since this round does not commit). The (g) four-field `maybe-move-out` deferral written in full,
  with its registry entry. All nine text-accuracy findings (parked 19–27) applied at their producer
  sites, each marked APPLIED in `parked.md`. Two roadmap rows (`roadmap.md`, both duplicate Capability
  Ledger tables) and one plan Future-Requirements row had their dangling
  `SCRATCH-audit-2026-07-11-memory-safety.md` citation corrected to the code-direct premise, without
  altering Patrick's own quoted triage words. **Plan:** Phase 4 step 3b rewritten to the signed
  transfer rule; a new Phase 4 step 3d authored for fr12; Phase 4 step 5 extended with the probe/alias/
  revive fixtures; Phase 5 steps 2–3 rewritten to topology (B)'s specifics, step 6 gained the
  `bg_arg_kind_is_releasable_payload(ARC)` ruling; the four Invariants subsections touched by the
  owed list corrected (Safety, Performance, Teaching, Feature Registry Entries); FR#10 tied to the
  `Unknown`-provenance classification; Phase 9 step 2's gallery list corrected; the FRAGO 008 "never
  enumerating `Expr::` variants" sentence amended per packet item (i); the "Downstream plan text … NOT
  edited" paragraph replaced with a pointer to FRAGO 010; the cold-resume banner and Phase 2's STATUS
  header/exit-criteria rewritten to reflect the closed sign-off (Phases 4/5 UNBLOCKED, Phase 3 the
  frontier). **Untraceable-to-a-ruling, not made:** none found. **Deviation from the dispatch's own
  wording:** every downstream-edit `(FRAGO 010, signed 2026-09-03)` inline tag cites `audit.md`'s
  SIGN-OFF record rather than a `git log --grep=m8-p2-signoff` pointer, because this round is
  instructed not to commit — a grep pointer minted now would be a dead pointer the moment it was
  written, the exact class parked item 26 exists to catch; the conductor's eventual commit message is
  expected to carry the `m8-p2-signoff` token, at which point the anchor could be widened, but that is
  not this round's call to make. Tests: none run (docs-only diff). No handoff file (round ran to
  completion in one segment).

- `m8-p2-fix1-20260903` — 2026-09-03 — **Phase 2 fix round 2 (design only, no compiler code):
  the `doc-auditor` BLOCKER defeated at its producer; six should-fixes and the minors answered; step
  6 (Patrick's sign-off) still OPEN.** Read: the Phase 2 STATUS block and round-1 grading, the
  `m8-p2-20260903-a1` entry, `corpses.md`, both design sections, `Stmt` (`nodes.rs:201–305`, ten
  variants, `For`'s two destructure fields, no closure form in `Expr`), the walker
  (`effective_ownership.rs:303–404` — `:379` classifies `Let`/`Assign` by value only, `:330–333`
  says reassignment "does not escalate"), the `Assign` arm (`check.rs:2436–2465` — param / loop-var /
  `const` refusals, then a type check; the consumed flag is never cleared), `Scope` (`scope.rs` —
  same-frame `insert` REPLACES; `consume` marks by name), the liveness inference (`check.rs:1443–1515`;
  `ident_read_in_stmt` `:8499` has the same by-value blind spot for `Assign`), `bg_inferred`'s
  consumers (only `inlay_hint_passes.rs:765` — codegen copies by type), `.freeze()` typing
  (`check.rs:6947` → `nothing`; `emit.rs:19297` lowers the receiver), the contract-signature loop
  (`parser.rs:3996–4012`, `_ => None`), `emit.rs:12657`, and `git log -S` for the fix2/fix3 text
  (`de631bf`) and the scratch-audit file (never in git). **Six throwaway probes** (`.probe-m8p2r2/`,
  `docker compose run --rm dev ./target/debug/ynz run`, deleted): (p1) `let rows = [1]; let other =
  [2, 3]; rows = other; wire.send(rows); other.count()` then `receive()` — compiles, prints `2 2`
  (the blocker's first program, live); (p2) `background render(scene, out); scene = other; background
  render(scene, out)` with `width` 1 and 7 — prints `8`: each spawn copies the CURRENT value today; the
  round-1 Arc group would have shared the block minted at spawn 1 and printed `2` (the blocker's
  second program, live); (p3) `eat(rows)` (`give`) then `rows = [4, 5]; rows.count()` — REFUSED
  "already given away": today's typeck never revives a reassigned consumed name — a correct program
  rejected, a pre-existing false error the binding-event rule fixes by construction; (p4) same-frame
  shadow `let rows = [1,2,3]; let rows = [4]` — legal, prints `1`; (p5) the shadow form of p1 — prints
  `2 2`; (p6) `for (row in matrix) { background producer(wire, row) }` — prints `3` then `matrix`'s
  count `2`: correct today, so the inferred-give path must not error on a `Cell` origin.
  **Decisions (pending sign-off):** origin and alias class are properties of a BINDING EVENT; the rule
  is exhaustive over `Stmt` (`Let` incl. shadowing, `Assign`, `For` incl. destructures, plus params;
  the other seven variants named as non-binding), keyed by entry not name; a re-bind LEAVES the old
  class (old members keep their state) and clears the consumed flag, then joins per the initializer
  table; `Stmt::Assign` calls the same binding-event function `Stmt::Let` does. Caller-side Arc:
  ONE `stmt_rebinds(stmt, name)` predicate in `effective_ownership.rs`; the walker returns `Writes`
  on a rebinding of the tracked name (the honest extension's second part, `:330–333` corrected); a
  top-level rebinding between spawns is a GROUP BOUNDARY (each segment judged on its own member
  count — a rebinding changes WHICH value is shared, not whether sharing is sound), a nested
  rebinding is `Writes` (path-dependent → decline). Inferred-give (sink 3) is the liveness
  inference, not `check_transfer`: `Give` iff origin `Owned`/`Param(give)` AND no class member read
  after; else `Copy`, silently. Contract modifiers optional; bare = never a give position; `follows`
  parity exact; the "REQUIRED on" line corrected. `.freeze()` → `Fresh` (typed `nothing`).
  `consumed[fn][i]`'s send case gated on the declared parameter type. Dangling scratch-audit cite
  replaced at three sites (two in `IMP-concurrency.md`, one in `IMP-ownership.md`) with the
  code-direct premise (`ynz_handle_free` declared `runtime_decls.rs:126`, zero emit sites) +
  `IMP-no-function-coloring.md` "Task Cancellation" + the roadmap's never-drop-locals row (the
  roadmap row itself still carries the dead cite — a plan artifact from Patrick's triage, not
  edited). `m8-p1-fix2/fix3` pointers → `de631bf`. Narrative trimmed (`IMP-concurrency.md` Status,
  `:207`, sign-off record → "What is signed off, and what awaits Phase 2"; `IMP-ownership.md`
  producer paragraph; `corpses.md:37–39`). **Plan:** STATUS block gained the round-2 paragraph and
  the owed-list grew (binding-event function at both `Let`/`Assign`, `stmt_rebinds`, group
  boundaries, exact parity, revive-on-reassign fixture, alias-by-assign/shadow gallery triggers,
  the Phase 2 block's own "never enumerating" sentence); downstream text unedited; session-id
  appended. Tests: none run (docs-only). No handoff file.

  **Sign-off packet for Patrick (revised; supersedes round 1's):**
  (1) Auto-Arc topology (B) — one shared copy, N task references, caller keeps its original + one
  transient released after the last spawn of the group.
  (2) Beneficial iff ≥2 spawns in one block share a whole binding, no suspension between, task-side
  `Reads`, caller-side `Reads` where a rebinding is `Writes`, `arc_shareable` type; "caller + 1
  task" is OUT; **a top-level rebinding of the binding between spawns is a group boundary, a nested
  one declines the group.**
  (3) The transfer rule in one paragraph — every value that leaves a frame for a sink that will
  free it passes one `check_transfer`; provenance says `Fresh`/`Whole`/`Reaches`/`Unknown` from ONE
  exhaustive `Expr` match; **origin and alias class are set at every binding event — `let`,
  shadowing `let`, reassignment, `for`, parameter — exhaustively over `Stmt`, a re-bind leaving its
  old class and reviving a consumed name**; a whole owned binding is consumed with its class; a
  parameter must say `give` (every frame of a chain reported in one build); anything still
  reachable elsewhere needs `.copy()`.
  (4) fr12 — send-minted 16-byte cell; `number` NOT in the give set.
  (5) Override directions — `.copy()` only (no `.give` exists); no force-the-auto-pick; `share`
  stays an error.
  (6) OPEN QUESTIONS — (a) confirm the container-store/literal-element sink deferral; (b) confirm
  whole-chain reporting in one compile, including consuming a caller's LOCAL at an
  effectively-consumed-but-undeclared position for reporting only; (c) confirm alias classes as a
  language change (`let other = rows; wire.send(rows)` — and now `rows = other; wire.send(rows)` —
  makes `other` unusable; programs that never transfer are unaffected); (d) `TransferNeedsCopy` as
  the registry name; (e) confirm `number` copy-through; (f) confirm `dynamic Contract` coverage in
  Phase 4 scope; (g) the relay-a-received-value cost (`other.send(got.value)` needs `.copy()` this
  milestone); (h) Phase 5's `bg_arg_kind_is_releasable_payload(ARC)` answer; **(i) NEW — the plan
  says "never enumerating `Expr::` variants"; the design does ONE exhaustive `Expr` match inside
  `effective_ownership.rs` (the corpse bans partial lists at call sites; a no-wildcard match in the
  owning module is closed by the compiler) — accept, and the plan sentence is amended at sign-off;
  (j) NEW — contract-signature ownership modifiers stay OPTIONAL (the parser's behavior); a bare
  contract position is never a give position; `follows` parity is exact (bare = bare) — the
  alternative is parser enforcement of the old "REQUIRED" line, owed to Phase 4; (k) NEW —
  reassignment REVIVES a consumed name (`eat(rows); rows = [4, 5]; rows.count()` becomes legal —
  today it is refused); (l) NEW — rebinding between spawns as a group boundary rather than a
  whole-group decline (boundary is more precise and reads the same predicate; decline is simpler
  to implement — one line — and forfeits sharing only in a rare shape).**

- `m8-p2-20260903-a1` — 2026-09-03 — **Phase 2 executed (design only, no compiler code): steps 1–5
  done, FRAGO 008's ownership absorption done, FRAGO 009's fr12 done; step 6 (Patrick's sign-off)
  OPEN.** Read in full: `effective_ownership.rs` (1,747 lines — lattice, fixpoint, `Reads` bottom,
  `classify_call_position`, `arg_is_binding`/`root_binding_name`/`place_path`, the aliasing-call
  check, all 26 tests), the plan's Phase 2 block + FRAGO 008/009, `audit.md` FRAGO 005/007 and the
  SIGN-OFF, `.claude/corpses.md`, parked items 3 and 12–18, the channel-close section of
  `IMP-concurrency.md`, `IMP-ownership.md`, `arc.rs`, the `auto_arc` / `auto-arc-*` /
  `channel-element-heap-upgrade` registry entries, `check.rs` at every cited site
  (`check_arg_ownership` and its three call sites, the UFCS path, the `for` binding, the spawn
  liveness pass, `bg_arg_is_provably_safe`, the handle pre-record, the `Expr::Background` arm, the
  `send` arm, the consumed-read site, the share/lend-across-`background` guard, the dynamic-dispatch
  site), `shapes.rs` `ContractSigDef`, `ynz-ast` `ContractSig`/`Param`, `queries.rs` ordering,
  `emit.rs` (`number_to_heap_cell`, the decimal128 bg-arg arm, `channel_drop_glue`, the
  `readonly`/`noalias` consumer of the report, `prepare_bg_arg_for_ctx`), `channel.rs` slot
  contract, `ynz-abi` kinds. **Seven throwaway probes** (created under `.probe-m8p2/`, run with
  `docker compose run --rm dev ./target/debug/ynz run`, deleted): (1) `bucket.stash(rows)` with
  `stash(give self: Bucket, give rows)` — compiles, prints `3` (caller keeps `rows`); (1b) the
  function-call form `stash(bucket, rows)` — correctly `already given away`; (2) `eat(bucket.rows)`
  into `give` — `3 3`; (3) `for (row in matrix) { wire.send(row) }` then `receive()` — `2 2`;
  (4) `dynamic Sink` call with a contract `give` param — typeck accepts, codegen ICEs `dynamic
  dispatch call sites not yet lowered in M4 P4` (parked 14 has NO runtime exposure today);
  (5) `let other = rows; eat(rows); other.count()` — `3 3`; (6) `let rows = pick(bucket)` where
  `pick` returns `b.rows`, then `eat(rows)` — `3 3` (Phase 1's "bind it first" advice admits an
  alias); (7) `let outer = [a]; eat(outer); a.count()` — `1 3` (parked 12 confirmed). **Two record
  corrections**: the fixpoint runs AFTER `check(...)` today (`queries.rs:~423` vs `:503`) — FRAGO
  008 and the corpse said "before"; corpse text corrected, FRAGO 008 left as written (append-only)
  and corrected here. And `IMP-concurrency.md:207`/the plan's Performance invariant name a
  spawn-site `.give` override that does not exist (`PostfixOpKind` is `Copy | Freeze`). **Decisions
  (all pending sign-off):** Auto-Arc topology (B) — one shared copy, N task references, caller keeps
  its original + one transient released after the last spawn; beneficial iff ≥2 spawns in one block
  share a whole binding with no suspension between, `Reads` both sides, `arc_shareable` type; the
  transfer rule as ONE `provenance()` + origin/alias classes + ONE `check_transfer` + a closed sink
  list + `consumed`/`returns_fresh` fixpoint facts; whole chain reported in one compile; `dynamic
  Contract` covered by construction; `SendPayloadNeedsCopy` → `TransferNeedsCopy`; container-store
  sinks deferred (four fields), literals of named heap values `Reaches`; fr12 as a send-minted
  16-byte cell with `number` copy-through and `ChannelElemDrop::transfers_source()`. **Docs:**
  `IMP-ownership.md` +2 sections; `IMP-concurrency.md` channel-close section re-pointed (rule,
  transit, alternative-weighed, element-types enum, fr12, registry list, teaching text → one home,
  sign-off record), `.give` override line fixed, inference table rows corrected;
  `IMP-no-function-coloring.md:58` pointer resolved; `corpses.md` ordering claim corrected. **Plan:**
  Phase 2 status block with the owed-downstream-edits list (Phase 4/5/Invariants/FR text NOT edited —
  no ruling to trace to); banner; session-id. **Parked:** 12/13/14/16/17/18 annotated with their
  Phase 2 disposition. **Sign-off packet for Patrick** (also in the executor return):
  (1) topology (B); (2) beneficial condition as above — confirm "caller + 1 task" is out;
  (3) the transfer rule in one paragraph — every value that leaves a frame for a sink that will free
  it passes one `check_transfer`; provenance says Fresh/Whole/Reaches/Unknown; a whole owned binding
  is consumed with its alias class, a parameter must say `give` (every frame of a chain reported
  in one build), anything still reachable elsewhere needs `.copy()`; (4) fr12 — cell marshalling,
  `number` NOT in the give set; (5) override directions — `.copy()` only (no `.give` exists),
  no force-the-auto-pick, `share` stays an error; (6) OPEN QUESTIONS — (a) confirm the
  container-store/literal-element sink deferral (the alternative: literal elements consume, Rust's
  zero-copy answer — but then `[a]` and `.add(a)` disagree until the drop story); (b) confirm
  reporting the whole chain in one compile, including consuming a caller's LOCAL at an
  effectively-consumed-but-undeclared position for reporting only; (c) confirm alias classes as a
  language change (`let other = rows; wire.send(rows)` now makes `other` unusable — programs that
  never transfer are unaffected); (d) `TransferNeedsCopy` as the registry name (or keep
  `SendPayloadNeedsCopy` — it was never shipped); (e) confirm `number` copy-through (parked 3
  assumed the opposite); (f) confirm `dynamic Contract` coverage in Phase 4 scope (three small
  sub-steps) rather than a deferral; (g) the relay-a-received-value cost (`other.send(got.value)`
  needs `.copy()` this milestone) — accept, or ask for a move-out-of-`maybe` form now;
  (h) Phase 5's `bg_arg_kind_is_releasable_payload(ARC)` answer. Tests: none run (no code touched).
  No handoff file (phase ran to its sign-off boundary in one segment).

- `conductor-2026-09-03-m8-execution` — 2026-09-03 — **The `/execute-plan` conductor session that ran
  Phase 0 and Phase 1 to sign-off.** Ends with the plan handed off mid-milestone to a fresh
  conductor; this entry is the session's own record, distinct from the executor entries below.
  - **Pre-flight.** No `.claude/rules/branching.md` existed; Patrick's answer captured and written
    there (`main` protected, plan work on `feat/<slug>`, close-out via PR, one live checkout per
    branch). Plan frontmatter gained `branch:` so a cold resume can find the ref.
  - **Phase 0** — PROCEED. Double merge gate satisfied. Produced FRAGO 001 (Phase 6 retired: M6 had
    already shipped P2-7 under its own FRAGO 010; confirmed revert-sensitively, not on the commit
    subject) and FRAGO 002 (two plan citations were DANGLING, pointing at unrelated code — the drop
    ladder among them, which Phases 4 and 7 both navigate by).
  - **Phase 1** — three fix rounds, three grading rounds, ~14 agent dispatches. Signed off narrowed.
    Produced FRAGOs 003–008 and the sign-off record above.
  - **An unplanned hotfix took priority mid-phase.** Phase 1's own review found a use-after-free live
    in released v0.3.3; Patrick ruled it a separate hotfix. PR #89 merged (three UAF instances closed
    across channel send / handle send / handle return, plus a parked→Closed leak, plus a flaky
    `ynz-watch` test that had been making every verification gate a coin flip), then merged back into
    this branch at `6143c1d` so the milestone stopped carrying the bug its own review had found.
  - **Bugs found that predate this milestone and are recorded rather than fixed:** `h.receive()` on a
    `-> maybe<T>` background task SIGSEGVs on consumption (a **Phase 4 precondition**, since Phase 1
    designs bare `receive()` to return `maybe<T>`); `map.copy()` compiles today and silently aliases
    through a codegen catch-all (parked #13, and the reason Patrick's `.copy()`-on-`map` ruling is
    load-bearing rather than cosmetic); the `Consumed` diagnostic template is dead data nothing
    attaches (parked #7); `bucket.add(rows)` on a bg-arg alias (Future Requirements #9, RED-pinned).
  - **The session's most reusable output is not about channels.** `.claude/corpses.md` was created
    with its first entry: three review rounds were spent re-deriving an ownership analysis by
    enumerating syntactic call sites while `effective_ownership.rs` — a whole-program fixpoint that
    already answers the question — sat unused one phase away, inside a plan whose own Phase 2 exists
    to reuse it. The cheap check that would have caught it in round one: grep
    `crates/ynz-typeck/` for an existing analysis before writing a new predicate about
    program-wide behavior.
  - **Conductor discipline notes for the successor.** Reviewer seats were derived per round from each
    round's own diff, never inherited; every dispatch carries a minted classification receipt (the
    gate caught one mis-addressed effort and was right). Green-check was skipped only on rounds with
    no compiler code, and that skip is recorded rather than silent. Two agents were stopped mid-run to
    avoid concurrent mutation of one checkout. Three separate agents ended their turn waiting on a
    backgrounded suite that dies with the turn — dispatch prompts should say "foreground everything,
    or report the rest as not observed."
  - **Left deliberately undone:** the branch is UNPUSHED, no PR opened for the milestone; a tagged
    release was explicitly skipped by Patrick (the UAF fix rides M8's eventual release), though a
    local `cargo build --workspace --release` is still what stops consumer projects mounting
    `target/release` from running the pre-fix binary.

- `m8-p1-fix3-20260903` — 2026-09-03 — Phase 1 fix round 3 (docs-only; no compiler code). Answered
  a reviewer BLOCKER: the give-at-send rule did not flow through an ordinary call. Read
  `check_arg_ownership` (`check.rs:4591–4648`), its three call sites (`:4799`, `:5115`, `:5445`),
  the ident-only gate (`:4798`), the `background` liveness path (`:1443–1515`), the `Expr::Background`
  arm (`:3283–3302`, confirming the spawned call reaches `check_user_fn_call` and thus the one give
  helper), the handle-form pre-record (`:2321–2345`), `ScopeEntry` (`scope.rs:9–34`), the ladder's
  free match and `release_ladder_payload` (`runtime.rs:926–943`, `:1125–1158`), and the send core's
  first-poll arms (`channel.rs:494–558`). Recorded Patrick's guard-A ruling in `IMP-concurrency.md`
  as the new "Ownership must flow through the call" subsection: `ParamNeedsGive` (a parameter given
  away must be declared `give`; threads the existing Give path at every call and spawn site; ALSO
  applied at `check_arg_ownership`'s Give arm so the `:4617` silent consume of a bare/`lend`
  parameter — the relay hole — becomes the error; the `:4611` share refusal retires into it) and
  `SendPayloadNeedsCopy` (admitted forms: ident, `.copy()`, literal). Transit decided: the whole
  chain declares `give`, one frame reported per compile. FR#9's channel-door instance recorded as
  CLOSED by the guard in both the design and the plan's FR#9 text; the container door stays.
  Should-fixes: `ChannelElemDrop` → `Option<{Array, Map}>`; THREE first-poll CLOSED arms
  (`channel.rs:503`, `:554`, new `None`) → one `refuse_closed` fallthrough; `release_ladder_payload`
  filter → one `ynz-abi` predicate + `ALL_BG_ARG_KINDS` + per-kind parity test; `map.copy()`
  independence fixture RED-before-clone as an obligation. Bookkeeping: parked 9 marked ABSORBED
  (round 2), parked 10 corrected to fixed-in-round-1, banner "stays parked" list now 5/7/8. The
  section header no longer says "DESIGN LOCKED"; an "Open at sign-off" list names fr12, the
  error-vs-warning call, and the `give` requirement as Patrick's three discrete decisions. FRAGO 007
  below records the ruling. fr12 left OPEN, untouched. Tests: none run (docs-only diff).

- `m8-p1-fix2-20260903` — 2026-09-03 — Phase 1 fix round 2 (docs-only; no compiler code). Read the
  hotfix `861fd4d` from the code (`channel.rs`, `handle.rs`, `runtime.rs`, `ynz-abi/lib.rs`, `emit.rs`),
  not the commit message, and reconciled it with the channel-close design in `IMP-concurrency.md`:
  new subsection "Two mechanisms, one rule" — typeck consume and the runtime pointer-identity
  release protocol BOTH stay (different questions: source readability vs. ladder ownership), linked
  by the `ChannelElemDrop` enum; **record correction**: FRAGO 004 ruling 2's "ladder consults
  consumption" is NOT what shipped (the runtime consults the hand-off event, not typeck — the commit
  names why the literal ruling was infeasible); **P2-3's closed-send free moves into the runtime's
  CLOSED-first-poll path** because a codegen-side free of a ladder-owned clone would double-free
  (this inverts a shipped doc comment + test case (c) of
  `ladder_is_untouched_when_the_channel_does_not_take_ownership`, called out for Phase 4). Scoped
  the "one owner at every moment" claim to task-OWNED payloads and named FR#9's aliased-bg-arg
  shape through the channel door as not assumed away. Rewrote the `ConsumedBySend` WHY (compile-time
  refusal, nothing "empty" at runtime). Recorded `.copy()`-on-`map` as Phase 4 step 3a with its
  registry entry, plus the finding that `map.copy()` compiles TODAY as an alias no-op through the
  codegen catch-all (new FR#10 for the remaining types). Absorbed parked 1/2/3/4/6; took parked 11
  as a compile ERROR (`HandleChannelArgNeedsBinding`) — the design now has TWO new compile-time
  diagnostics. fr12 left OPEN, untouched. Banner flipped from BLOCKED to UNBLOCKED, resume at Phase 1
  step 7. Tests: none run (docs-only diff).

- `plan-producer-2026-07-04-m8-concurrency-completion` — 2026-07-04 — Authored the complete OPORD from
  the assembled brief. Read the concurrency-release audit
  (`.claude/audits/2026-07-04-concurrency-release-audit.md`), the frozen plan-format/risk-engine/
  decision-philosophy references, `IMP-no-function-coloring.md`, `IMP-concurrency.md`, `IMP-ownership.md`
  (confirmed live: it genuinely contains zero cross-thread Arc-sharing-topology text), the relevant
  `registry/features.toml` entries (`auto-arc-codegen-emission`, `auto-arc-cautionary-tint`,
  `background-handle-cancel-injection`), and both sibling plans (`2026-07-04-v0-3-m6-concurrency-hotfix`,
  `2026-07-04-v0-3-m7-optimizer-pipeline`) to confirm zero scope overlap. Scored the risk table against
  the default code-domain anchor sheet (no project override — glob-confirmed). Set `status: "paused"`
  per the conductor-pre-approval convention M7 established, gated on a double merge-and-tag precondition
  (M6 AND M7 must both merge before Phase 0 begins) plus the orchestrator's Gate 4 read-through. No HIGH
  residual anywhere in the risk table — every hazard reuses an already-authoritative source
  (`effective_ownership` for Arc, M6's drop-glue choke point for channel-close) rather than inventing a
  new frame-layout-affecting transform, keeping this milestone's hazard surface narrower than M7's own
  R8. Ten phases (0–9): gate, two parallel-safe design phases (channel-close, Arc topology) each with a
  Patrick sign-off gate, a loom-substrate phase sequenced before both implementation phases per the
  brief's explicit instruction, the two implementation phases, a small P2-7 mechanical fix, a
  design-plus-contingent-implementation phase for scope-drop cancellation, a structured-fuzzing phase,
  and close-out (demo/gallery/registry/roadmap reconciliation/full-suite gate).

- `plan-producer-2026-07-04-m8-amend1` — 2026-07-04 — Amendment pass resolving a plan-review's full
  finding set (2 BLOCKERs, 3 SHOULD-FIXes) before this plan's Gate 4 read-through. Re-read
  `REF-plan-format.md`, `REF-risk-engine.md`, `.claude/rules/plan-invariants.md`, and both sibling M6/M7
  plans' Invariants sections (M7 lines ~734-855) live before amending, per this producer's own
  read-at-start discipline.
  - **BLOCKER 1 (missing Invariants section):** authored the full `## Invariants This Milestone Must
    Preserve` section (all 7 subsections — Safety, Performance, Teaching, Runtime Dependencies,
    Kernel-Mode Behavior, Demo & Error Gallery, Feature Registry Entries), inserted between `### 3.4
    Coordinating Instructions` and `## 4. Sustainment`, matching M6/M7 sibling shape. Notably: ran the
    full auto-promotion.md checklist against Auto-Arc (a genuine instance — codegen yes, muted hint yes
    via the already-registered `auto_arc` domain firing at Phase 5 step 4, Tier 3 lint NO with reasoning,
    override directions both analyzed — force-the-other-pick already covered by existing `.give`/`.copy`
    syntax per auto-promotion.md's own canonical example, force-the-auto-pick a deliberate no-override);
    confirmed channel-close has NO auto-promotion candidate (stated explicitly, not silently skipped);
    confirmed via direct `check.rs` grep that `--kernel` already rejects `wait`/`background`/`channel<T>`
    entirely (lines 3392-3398, 3047-3059), so this milestone's entire surface is unreachable from kernel
    mode — zero new kernel-mode consideration, stated explicitly rather than left silent.
  - **BLOCKER 2 (R2 mis-scored):** re-scored R2 honestly — initial Prob was C, should be **B** (Likely):
    this is net-new codegen in the four-milestone silent-miscompile hazard family (M3a/M3d/M3e/M3g), and
    reusing `effective_ownership::EffectiveOwnership::Reads` closes only the MISCLASSIFICATION mode, not
    the SEPARATE frame/spawn-boundary-layout interaction hazard Phase 5's own spike step (step 2)
    explicitly concedes as an open question. B×II = H initial; the B2 adversarial/RED-repro + spike-gate
    mitigation (prob −1) shifts B→C; re-lookup(C, II) = H, UNCHANGED (Critical severity does not clear
    High until probability reaches D — same rule M7's own R8 override already established). Did NOT
    stretch a second catalog mitigation dishonestly to force a MEDIUM landing (M7's R8 precedent explicitly
    rejects that move). Drafted a full unsigned RISK OVERRIDE block for R2 mirroring M7 R8's shape
    (risk/why-not-mitigable/blank Accepted-by+Date/trigger-to-revisit citing Phase 5 Step 2's own spike
    verdict as the evidence path toward a future re-score) — signature line intentionally blank; this
    producer never self-signs a HIGH residual. Updated the risk-table row, the "No HIGH residual" intro
    prose (now "One HIGH residual — R2"), the Floor-check paragraph, Phase 5's task+purpose/exit-criteria/
    reviewer-fan-out text, and CCIR item 6 all in the same pass so the plan is internally consistent (R1's
    scoring was reviewer-confirmed correct and left untouched — a runtime-state change, not a frame-layout
    one).
  - **SHOULD-FIX 1:** swept the whole plan for "Task Cancellation" / "IMP-concurrency" and fixed all six
    misattributions (¶1 Terrain heading, Design-Doc Alignment citation-depth-verification bullet, Design-
    Doc Alignment divergence #3, Phase 7 step 2, Phase 7 step 3 Branch A, Phase 7 reviewer-fan-out) — the
    Task Cancellation section genuinely lives at `IMP-no-function-coloring.md:281-298` (confirmed by direct
    read this session), never `IMP-concurrency.md`. Verified the Design-Doc Alignment "Cited governing
    docs" line (line ~249) was ALREADY correct (Task Cancellation already listed under
    `IMP-no-function-coloring.md` there) — left untouched.
  - **SHOULD-FIX 2:** fixed the ¶1 Friendly-forces roadmap link — was `../2026-05-21-v0-3-concurrency-
    perf/roadmap.md` (wrong depth from this plan's `paused/` location), now
    `../../active/2026-05-21-v0-3-concurrency-perf/roadmap.md`, matching M7's own sibling-plan link
    pattern and confirmed against the roadmap's actual on-disk location
    (`.claude/planning/active/2026-05-21-v0-3-concurrency-perf/roadmap.md`, glob-verified).
  - **SHOULD-FIX 3:** Phase 9 step 3 now states explicitly that the roadmap's existing combined
    "Concurrency completion... status: being authored" placeholder row (present in BOTH duplicate
    Capability Ledger tables, roadmap.md lines ~445 and ~499) is REPLACED BY the four granular rows this
    plan adds, in both tables, in the same lockstep edit — not left standing as a stale fifth row.
  - **Confirmed out of scope, not touched:** the global `~/.claude` spec-link-unreachability systemic gap
    (all three M6/M7/M8 siblings share it, not this plan's defect to fix); the directory split (already
    fixed on disk — confirmed `audit.md` sits beside `plan.md` in `paused/2026-07-04-v0-3-m8-concurrency-
    completion/` this session, no action needed).
  - Appended this session's id to the `plan.md` frontmatter `session-id` array in the same action as this
    entry (never minted separately).

- `gate4-signatures-2026-07-04` — 2026-07-04 — Signature event: Patrick signed R2's RISK OVERRIDE
  (Auto-Arc codegen-emission refcount/frame-layout hazard, ¶1 Risk Assessment) as part of Gate-4
  approval covering all three sibling concurrency plans (M6/M7/M8). Filled `Accepted by: Patrick
  (Gate-4 approval, conducted 2026-07-04)` and `Date: 2026-07-04` on the previously-blank signature
  lines; updated R2's Gate cell from `BLOCKED — unsigned RISK OVERRIDE below` to `H — override SIGNED
  (see block below)`; reconciled every other plan-text mention asserting R2's override was still
  unsigned (the pre-table intro sentence, the post-table paragraph preceding the override block,
  Phase 5's task+purpose sentence, and CCIR item 6) so the plan is internally consistent. Appended
  `session-id: "gate4-signatures-2026-07-04"` to the frontmatter chain (append-only — both prior
  session-ids preserved).

- `m8-p1-20260903-a1` — 2026-09-03 — **Phase 1 executed (design only, no compiler code): steps 1–6
  done, step 7 (Patrick sign-off) OPEN.** Read `channel.rs` end-to-end (endpoint-holding
  architecture, `pending_sends` keying, purge, `Drop`), `handle.rs`'s `outbox_tx: Option<Sender>` +
  `.take()` close precedent, the `emit.rs` conduit closed arms (`~12749-12961`, `closed_msg` text and
  the aborting `ChanRecv` arm), typeck's `check_conduit_method_call` + `CHANNEL_SUSPENDING_METHODS`,
  `IMP-concurrency.md`, `IMP-no-function-coloring.md` (silent on close — no contradiction with the
  plan), `REF-concurrency.md`/`REF-errors.md`/`REF-maybe.md`/`REF-control-flow.md` (no `break`
  keyword exists — the taught consumer loop is flag-driven), vocabulary/dot-postfix/teaching-surfaces
  rules, Golden Rule 12, and the registry schemas. Decisions: explicit `.close()` (chosen over
  `.done()`/`.finish()`/`.end()` — matches the word every existing error string and runtime constant
  already uses for the state); auto-close-on-last-producer DEFERRED with four fields (needs role
  analysis + the missing scope-exit drop pass + a producer/holder refcount split — a redesign, not an
  extension; CCIR-2 discovery routed as a deferral, not absorbed); bare `receive()` → `T errors`
  (one `.receive()` convention with the handle form; `maybe<T>` weighed/rejected; ~12 fixture sites +
  demo + gallery + spec change in Phase 4); send-after-close = runtime typed error, no compile
  diagnostic; `close()` wakes all recv-waiters (settles the co-waiter facet `channel.rs` left to M8);
  idempotent double-close; in-flight pre-close sends complete; P2-3 fix routed through the registered
  drop glue. Doc: new `IMP-concurrency.md` section "Channel Close — End-of-Stream Semantics"
  (promoted out of the Divergences format — six load-bearing parts); the M6 Divergence entry rewritten
  to a pointer that retires at Phase 4; one cross-ref line added to `IMP-no-function-coloring.md`.
  Teaching text (send-after-close, receive-after-drain, the two extended compile diagnostics) drafted
  in the section. Registry kinds recorded for Phase 4 (`[[primitive_intrinsic]]` incl. registering the
  un-registered `send`/`receive`; `[[deferred_language_feature]]`; no `[[diagnostic_template]]`).
  **fr12 surfaced as a scope question**: separable from close by construction; its marshalling design
  is not written by this phase. No tests run (no code touched). No handoff file (phase ran to its
  sign-off boundary in one segment).

- `m8-p1-fix1-20260903` — 2026-09-03 — **Phase 1 fix round (design only, no compiler code): two
  reviewer BLOCKERs ruled on by Patrick applied, six should-fix items addressed; step 7 (sign-off)
  still OPEN.** BLOCKER 1: the first draft's P2-3 closed-arm free was a use-after-free — typeck's
  `send` arm never consumes its argument (`check.rs:4105–4149`; the only `scope.consume` sites are
  `:1511` and `:4618`), codegen lowers the payload by bare `to_i64_bits` (`emit.rs:12641–12649`).
  Ruling applied: `send()` gives its payload for owned-heap element types (`array`/`map` — the exact
  set `channel_drop_glue` registers glue for, `emit.rs:15511–15515`; primitives and `string`
  unchanged), mirroring the spawn-arg give path; new compile diagnostic `ConsumedBySend` drafted in
  full three-part form; emitted from the ONE existing consumed-read site by cause. **Probe on the
  current tree confirmed the hole is live today**: a `channel<array<int>>` program sending `rows`
  then printing `rows.count()` compiles and runs. Found and recorded: no `channel<array<T>>`/`map`
  E2E fixture exists anywhere (only `channel_construct.ynz:14`, `channel<string>`, construction
  only). BLOCKER 2: `receive()` retyped to `maybe<T>` (vocabulary.md: `maybe` is for normal absence,
  `errors` for failure; auto-propagation at `check.rs:3647–3653` would have made end-of-stream the
  task's failure in both shipped channel-consuming task fns; `ynz_error_new` per normal loop exit at
  `emit.rs:12802–12813`); handle's `receive()` stays `T errors` with the reason written so nobody
  "unifies" them; `tallyScores` rewritten to `.exists()`/`.value`. Should-fix: (1) lock-ordering
  nuance — the sender-lock clone is the linearization point, a send holding a clone is a pre-close
  send (`channel.rs:445–446`); (2) `h.close()` argument replaced (message-to-child vs lifecycle act
  on the channel), and the non-ident-first-channel-arg question answered by code read
  (`check.rs:2321–2345` idents only; `bg_arg_is_provably_safe` admits a call as `Give`;
  `prepare_bg_arg_for_ctx` shares by type) AND by probe (`background doubler(makeWire())` +
  `h.send(21)` prints 42) — a real gap, recorded as the `background-handle-close` four-field
  deferral; (3) blast radius corrected to 19 sites / 13 fixture files, enumerated, plus
  `REF-concurrency.md:252` which the review's own list missed; (4) loom named as not-yet-a-dependency
  (no `loom` in any `Cargo.toml`), Phase 3's swap gates the model-check; (5) three typeck sites
  named for `close` (`known`, the unconditional receiver/derivable guards `:4011–4082`, the shared
  unknown-method string `:4003`); (6) the "extend THIS site" quote re-attributed to
  `check.rs:3988–3992`. fr12 left OPEN with one line marking it pending Patrick. Registry list
  updated (`param_ownership` schema field; 2 deferrals; 1 template; plus a pre-existing
  `Consumed`-template-vs-code wording drift found and assigned to Phase 4). Plan: Phase 1 status
  block rewritten with the seven Phase 4 obligations; Safety/Teaching/Feature-Registry invariants
  and Phase 9 step 2 updated. No tests run (no code touched; the three probe programs were
  throwaway — created, run in the dev container, deleted). No handoff file.

## FRAGO log

### FRAGO 015 — 2026-09-04 — The record MINTED THIS round: two genuine runtime defects the Phase 8 owned-heap widening surfaced, both PRE-EXISTING on `main`, routed not fixed

- **Trigger:** fix round 3 (`m8-p8-fix2-20260904`)'s own entry (above, this log's Session log)
  claimed **"FRAGO 015 minted"** for the two defects its widening surfaced. That claim was false —
  no `### FRAGO 015` record existed anywhere in this plan's `audit.md` (highest prior number was
  014) until this fix round (`m8-p8-fix3-20260904`) wrote it, per an adversarial review that
  caught the dangling citation (four sites: this file, `plan.md`'s Future Requirements #11,
  `fuzz_grammar/mod.rs` twice, `fuzz_grammar/README.md` once). **Note the trap:** "FRAGO 015"
  already denotes an unrelated item in a DIFFERENT plan's audit
  (`2026-07-03-v0-3-m5-auto-soa/audit.md`) — every citation to this record must name which plan's
  `audit.md` it means ("the v0.3-M8 plan's `audit.md`, FRAGO 015"), never "FRAGO 015" bare.
- **Finding — two independent, precisely-characterized, deterministic-or-not runtime defects**,
  both surfaced by widening the fuzz generator to owned-heap channel payloads (v0.3-M8 Phase 8
  fix round 3), neither a generator/grammar bug:
  1. **Crossing-local heap-channel-send corruption.** An `array<int>`/`map<string,int>` LOCAL
     declared BEFORE any suspension point (`wait`, a `background`-handle `.receive()`, a channel
     `.receive()`) in the SAME function, later `.send()`-ed into a channel AFTER that suspension,
     reads back corrupted — `RUNTIME ERROR: killed by signal 6 (SIGABRT)`, a null/misaligned
     pointer dereference inside `ynz_map_count`/`ynz_array_count`
     (`crates/ynz-runtime/src/lib.rs:1058`/`:1411`). Minimal repro:
     ```
     function fetch1(n: int) -> int { wait sleep(1); return n + 6 }
     function entrypoint() -> nothing {
       let rows12: array<int> = [0]           // declared BEFORE the wait — CRASHES
       let v13 = wait fetch1(5)
       let wire15: channel<array<int>> = channel<array<int>>(1)
       wire15.send(rows12)
       let got16 = wire15.receive()
       if (got16.exists()) { print(got16.value.count().toString()) }
     }
     ```
     Swapping the two `let` lines (array declared AFTER the `wait`) is confirmed SAFE. General to
     both owned-heap kinds; NOT reproduced by `channel<number>`, by reading the local WITHOUT
     sending it into a channel, or by an inline (non-`background`) channel's own close-then-drain
     in isolation.
  2. **Capacity-forced-blocking channel send reads back garbage.** A `background` producer forced
     to actually BLOCK on a full channel buffer (`send_count > capacity`) can read back a HEAP
     ADDRESS where a sent value belongs — an uninitialized-or-freed read. NOT `number`-specific
     (the `int` variant of the same shape shows it); NOT deterministic (~17-30% of runs); NOT an
     arithmetic shortfall. 2 sends into capacity 1, or 3 sends into capacity 4 (neither forces
     blocking), are unaffected.
- **PRE-EXISTING on `main` — both confirmed, not assumed (fix round 4's own re-verification,
  recorded here verbatim per the dispatch instruction, one of the two independently sanity-checked
  this round):**
  - **(a) reproduces on `main` (`ec014d8`)**: same panic, same address, same function —
    `misaligned pointer dereference: address must be a multiple of 0x8 but is 0xb` at
    `ynz_array_count` (branch `lib.rs:1411`, `main` `lib.rs:1340` — line shift only), 3/3 runs;
    the same program with the array declared AFTER the wait prints `1` on both. **Fires in the
    DEFAULT optimized mode** — the mode `target/release` consumers run.
  - **(b) also reproduces on `main`**, and is NOT what the round-3 record claimed: not
    `number`-specific (the `int` variant of the same shape shows it), not deterministic
    (~17–30% across runs), not an arithmetic shortfall — the bad runs print HEAP ADDRESSES
    (`139853207069028` etc.) where `258` belongs. On `main`, `channel<int>(1)` with 3 sends under
    `--no-optimize --no-auto-parallel`: 6 of 36 runs printed garbage, 30 printed `258`; the
    default optimized mode was 36/36 correct; `cap=4` (producer never blocks) was 36/36 correct.
- **Guards containing them (generator-level, not a fix):** `take_or_make_array`/
  `take_or_make_map`'s `suspension_seen` gate in `fuzz_grammar/mod.rs` (defect a); the
  `send_count.max(1 + below(4))` capacity floor in `stmt_background_drain_loop` (defect b — costs
  the corpus its blocked-send coverage entirely, recorded in `fuzz_grammar/README.md`'s
  non-coverage list). Neither guard narrows an entire element kind or construct.
- **Disposition: routed, not fixed.** Per this plan's own CCIR item 5 / risk R5 — a genuine
  finding routes through the plan-amendment/FRAGO seam, never a same-round inline patch — this is
  standing plan policy set at Gate-4 authoring, not a fresh ruling this fix round. Full
  WHAT/WHY/COST/TRIGGER for both defects lives in `plan.md`'s Future Requirements #11 (kept in
  sync with the corrections this fix round made to (a)'s dominant-symptom claim and (b)'s three
  wrong claims — see that section, not restated here). **Open clustering question, unresolved**:
  are (a) and (b) the same producer (a general "value crossing a suspension/blocking-send
  boundary" bug with two symptoms) or two independent ones? Bisection did not settle it — (a)
  needs no backpressure and no `--no-optimize`; (b) needs both. Whoever picks this up checks that
  first, per `root-cause.md`'s "cluster findings before fixing any."
- **Authority:** no new ruling needed — this record documents facts already true (the defects'
  existence, their pre-existence on `main`) and applies standing plan policy (R5 routing) that was
  already signed at Gate-4. The prior round's false "minted" claim is corrected by this record's
  existence, not by editing the append-only prior entry.

(the note below this pair predates execution; FRAGO 001/002 are the first real mid-execution
delta-records against this running plan)

### FRAGO 001 — 2026-09-03 — Phase 6 RETIRED as already-satisfied by M6

- **Trigger:** Phase 0's terrain re-verification found this plan's P2-7 premise stale.
- **Finding:** M6 did not defer P2-7. It un-deferred it under its own FRAGO 010 and shipped the fix
  as M6 Phase 4b, commit `b0cdbd3`, inside PR #82.
- **Confirmation (not self-graded):** the finding came from an executor and was NOT applied on that
  basis. A separate `code-reviewer-medium` confirmed it adversarially: `record_recv_waiter(cx.waker())`
  is the first statement inside the `catch_unwind` closure (`handle.rs:354`), before `poll_recv`
  (`:355`), closing the exact panic-before-registration window the audit reported; two tests lock it
  (`handle.rs:724`, `:798`) and were **proven revert-sensitive** — the reviewer swapped the ordering
  back to poll-first, both failed on the P2-7 hang assertion (`wakes == 0`), tree restored clean.
  M6's no-lock-across-blocking-poll invariant holds; handle-side and channel-side are a genuine
  structural mirror.
- **Authority:** Patrick, 2026-09-03 — "verify first, then retire." Conditional authority discharged
  by the confirmation above.
- **Applied:** Phase 6 block marked RETIRED with its original text preserved in a `<details>` fold;
  risk row R6 retired; ¶1 Terrain P2-7 bullet and Design-Doc Alignment §4 boundary claim annotated as
  superseded; Invariants → Safety P2-7 assertion annotated satisfied-by-M6. Phases NOT renumbered —
  nine phases (0–5, 7–9), every existing citation and future `Plan-Phase:` trailer stays valid.
- **Residuals carried forward:** two M6-inherited items (panic-payload log asymmetry; the duplicated
  `recv_waiters` trio) recorded as Future Requirements item 8 rather than absorbed.

### FRAGO 004 — 2026-09-03 — M8 PAUSED at Phase 1 step 7; a live use-after-free on `main` takes priority

- **Trigger:** Phase 1's round-2 `code-reviewer` seat, grading the repaired channel-close design,
  found the design's "the buffered value has exactly one owner at every moment" claim false — and in
  proving it, reproduced a **use-after-free that exists on `main` today** (v0.3.3, released).
- **The bug, root cause named:** typeck's consume is not the only ownership authority.
  `prepare_bg_arg_for_ctx` gives a spawned task a heap clone of any `array<int|float|bool|shape>`
  argument tagged `BgArgFreeKind::HeapArrayPrimitive` (`emit.rs:16888`, `:16918`), and the task's
  drop ladder frees it at retirement (`emit.rs:17028`, `:17516`). If the task sends that parameter
  into a channel, the SAME pointer is in the channel buffer AND still on the ladder's free list.
  Nothing connects `check_conduit_method_call`'s consume to codegen's ladder. **Reproduced on the
  current tree:** a `producer(wire: channel<array<int>>, rows: array<int>)` spawned as
  `background producer(wire, rows)` that sends `rows` printed `-4760032263271174595` for
  `got.count()` in the spawner.
- **Why this is one bug and not two.** The M8 Phase 1 design blocker and the shipped UAF share a
  single ancestor: codegen's drop ladder and typeck's ownership view are independent owners of the
  same pointer. Per `root-cause.md`'s cluster rule, they get ONE fix at the ancestor, not one patch
  each. Patrick's ruling ("ladder consults consumption") is therefore both the hotfix's mechanism and
  the substrate M8 Phase 4 builds on — Phase 4 inherits it rather than re-deriving it.
- **Patrick's rulings, 2026-09-03** (four, taken together at one gate):
  1. **Hotfix now, on its own branch, separate from M8.** The released compiler is mounted read-only
     by external consumer projects via `target/release`; a memory-safety bug there cannot wait for a
     ten-phase milestone.
  2. **Ladder consults consumption** — codegen skips the ladder free for a binding typeck already
     marked consumed by a send. Threads the one authoritative answer into the second consumer per
     `authoritative-derivation.md`, rather than teaching typeck about `BgArgFreeKind`.
  3. **FRAGO 003 ratified** (see below).
  4. **`.copy()` ships on `map<K,V>` before Phase 4**, so the use-after-send diagnostic's advice is
     executable for every type the consume rule covers.
- **M8 state at pause:** Phase 0 COMPLETE. Phase 1 steps 1–6 complete through two review rounds;
  **step 7 (Patrick's sign-off) is still OPEN and was never granted** — the four rulings above are
  decisions feeding the design, NOT the sign-off itself. Phase 4 remains hard-blocked. An outstanding
  Phase 1 fix round is owed before sign-off, carrying: this FRAGO's ownership resolution, the
  `ConsumedBySend` WHY rewrite (doc-auditor blocker — "is empty afterward" teaches a runtime-emptiness
  model when the real behavior is a compile error), the `map`-`.copy()` ruling, and the parked items
  in `.claude/plans/parked.md`.
- **Still unruled and owed to Patrick at sign-off:** fr12's disposition (ride this design pass, per
  his July triage, or become its own step).

### SIGN-OFF — 2026-09-03 — Patrick signed off Phase 1 (as narrowed by FRAGO 008). Step 7 CLOSED.

- **Authority:** Patrick, 2026-09-03, conditional on everything unresolved being genuinely re-homed
  to a later phase rather than dropped. That condition was met by parking items 12–18 in
  `.claude/plans/parked.md` and by Phase 2's plan block carrying the three failing programs.
- **What is signed:** the narrowed Phase 1 — explicit `.close()`; the name, decided against
  `vocabulary.md` and GR12; bare `receive()` → `maybe<T>` with the handle's `T errors` deliberately
  distinct; idempotent double-close; close wakes every recorded receive-waiter (closing the co-waiter
  gap `channel.rs` left for this milestone); in-flight pre-close sends complete; drop-without-close
  byte-identical; the `refuse_closed` collapse of all three first-poll CLOSED arms; P2-3's free
  living in the runtime rather than codegen's closed arms (FRAGO 006); `Option<ChannelElemDrop>`;
  `HandleChannelArgNeedsBinding`; and the four-field auto-close-on-last-producer deferral.
- **What is NOT signed, because it left Phase 1:** the ownership rule and its three diagnostics —
  Phase 2's, per FRAGO 008, and gated by Phase 2's own sign-off.

**THREE ITEMS REMAIN OPEN. Patrick's sign-off did not answer them, and they must NOT be read as
settled by it.** The next conductor asks him before the phase that needs each:

1. **fr12 — `channel<number>` decimal128 marshalling.** Patrick assigned it to this milestone in July
   so it would "ride the same design pass as channel close — one design head for both." Phase 1
   argued separability and left it undesigned; Phase 1 has now closed without it. **Conductor's
   recommendation, not a decision:** it should ride Phase 2, since the ownership pass moved there and
   marshalling is the other half of "what does sending a value actually do." Ask before Phase 2
   starts.
2. **`HandleChannelArgNeedsBinding`: hard error or warning?** Designed as a compile error (GR5; a
   warning leaves the hang class live; the fix costs one `let`). It rejects source that compiles and
   runs correctly today whenever the channel is never closed. Ask before Phase 4 implements it.
3. **Does FRAGO 003's ratification stand for every Phase 1 fix round, or renew per round?** Patrick
   ratified round 1's downstream plan edits; rounds 2 and 3 did the same class of thing without
   their own ratification. `plan-adherence` raised it rather than assuming either way. Ask at the
   next round that edits Gate-4-signed plan text.

### FRAGO 010 — 2026-09-03 — Phase 2 sign-off recorded in the design docs; owed downstream plan edits and parked 19–27 applied

- **Trigger:** Patrick's Phase 2 sign-off (the SIGN-OFF record immediately below this entry), at the
  conductor's gate after round 2 closed CLEAN. The sign-off's own text authorizes this round, under
  FRAGO 003's standing traceability gate, to (1) record the ruling in the design docs, (2) apply
  parked items 19–27 (text-accuracy corrections on the signed design), and (3) execute every
  downstream plan edit the Phase 2 STATUS block's "Downstream plan text … NOT edited … Owed" list
  named. Dispatch `m8-p2-signoff-20260903`.
- **Authority:** the SIGN-OFF record below — every edit this FRAGO records traces to one of its
  twelve ruled packet items (a)–(l), or to the sign-off's top-level rulings (topology (B), the
  transfer rule, fr12, the `.copy()`-only override direction).
- **Applied — design docs** (`docs/internal/implementation/IMP-ownership.md`,
  `docs/internal/implementation/IMP-concurrency.md`): every "AWAITING Patrick's sign-off" /
  "awaiting Phase 2's sign-off" marker on the "Transfer" and "Auto-Arc" sections, the channel-close
  section's header/status, and the fr12 subsection header, converted to current-state text with a
  one-clause anchor (`audit.md`'s SIGN-OFF record) — traces to the sign-off's own authority, not a
  single lettered item. The (g) four-field deferral for a consuming move-out-of-`maybe` form written
  in full (WHAT/WHY/COST/TRIGGER) into `IMP-ownership.md` "What this makes sound, and what stays
  outside," with a matching `[[deferred_language_feature]] name = "maybe-move-out"` entry in the
  section's registry list — traces to packet item (g). The "never enumerating `Expr::` variants"
  divergence note amended to the ruled wording — traces to packet item (i). The `dynamic Contract`
  section's "REQUIRED" narrative trimmed to current state plus anchor — traces to packet item (j).
  Parked items 19 (`bg_inferred`/`is_heap_arg` presence-not-variant), 20 (`BgOwnership::Channel`
  precedes the inferred-give rule), 21 (swapped `check.rs:1511`/`:4618` labels, third instance, now
  cited by function name), 22 (origin/alias "set once at creation" corrected to "recomputed at every
  binding event"), 23 (function-type-annotation form dropped — does not exist), 24 (see packet item
  (j) above), 25 (`For`-destructure origin/alias row split from the loop-variable row), 26 (dead
  `git log --grep=m8-p1` claim against `de631bf` dropped for a direct SHA cite), and 27 (six not
  seven `ScopeEntry` constructors; "eight probes, seven live" not "nine found live";
  `root_binding_name`'s pre-existing twin flagged; dated probe prose replaced with a grep pointer;
  the roadmap's two duplicate rows and the plan's FR item 7(2) both gained a citation correction for
  the uncommitted `SCRATCH-audit-2026-07-11-memory-safety.md`) — each applied and marked in
  `.claude/plans/parked.md`, traceable to the SIGN-OFF record's "It also applies parked items 19–27"
  clause.
- **Applied — plan.md, downstream edits owed by the Phase 2 STATUS block, each traced:**
  - Phase 4 step 3b rewritten in full to the signed transfer rule (hoisted fixpoint, `provenance()`,
    the binding-event function at both `Stmt::Let`/`Stmt::Assign`, `stmt_rebinds`, the two fixpoint
    facts, `TransferNeedsCopy`, `dynamic Contract` coverage, the shared call-form normalization) —
    traces to the sign-off's transfer-rule paragraph and packet items (b)/(c)/(d)/(f)/(k).
  - Phase 4 step 5 extended with the eight probes as gallery/fixture triggers, alias-by-assign and
    alias-by-shadow, and revive-on-reassign as a correct-program fixture — traces to packet items
    (c) and (k).
  - New Phase 4 step 3d authored for fr12 (`NumberCell` glue, `number_to_heap_cell` marshalling,
    `channel-element-heap-upgrade` narrowed, the `v0_3_m4_errors.ynz:98` retirement) — traces to the
    sign-off's fr12 paragraph and packet item (e).
  - Phase 5 steps 2–3 rewritten with topology (B)'s specifics (the caller-side transient, the group
    condition, `stmt_rebinds` group boundaries, `arc_shareable`, "caller + 1 task" OUT) and step 6
    gained the explicit `bg_arg_kind_is_releasable_payload(BG_ARG_KIND_ARC_SHAPE)` ruling — traces to
    the sign-off's topology paragraph and packet item (h).
  - Invariants → Safety gained the alias-classes/revive-on-rebind paragraph and the `number`
    copy-through bullet; → Teaching renamed the three transfer diagnostics to their signed names and
    pointed each at its `IMP-*.md` home; → Feature Registry Entries corrected the diagnostic list to
    `TransferNeedsCopy`, added `maybe-move-out` and the narrowed `channel-element-heap-upgrade` entry
    plus a `modify auto_arc hover` line; → Performance deleted the nonexistent spawn-site `.give`
    override claim in three places, replacing it with `.copy()`-only — traces to packet items
    (c)/(d)/(e)/(g) and the sign-off's override-direction ruling.
  - FR#10 gained a paragraph tying the FR#10 alias-no-op types to `provenance(expr).copy()`'s
    `Unknown` classification (`TransferNeedsCopy` refuses their transfer already); FR#9's text was
    already reconciled by Phase 1 fix round 3 and needed no further edit this round.
  - Phase 9 step 2 gallery-trigger list corrected to `ParamNeedsGive`/`TransferNeedsCopy` and
    attributed to "Phases 1 and 2's signed design."
  - `plan.md`'s Phase 2 block FRAGO 008 sentence ("never enumerating `Expr::` variants") amended to
    the ruled wording — traces to packet item (i).
  - The Phase 2 STATUS block's "Downstream plan text … NOT edited … Owed" paragraph, now executed,
    replaced with a one-line pointer to this FRAGO entry.
  - The cold-resume banner and Phase 2's own STATUS header and exit-criteria line updated to reflect
    the closed sign-off, Phases 4/5 UNBLOCKED, and Phase 3 as the frontier — traces to the sign-off's
    top-level authority, not a lettered packet item.
- **Not applied — untraceable to any ruling, listed rather than made:** none identified this round;
  every edit above traces to a named packet item or the sign-off's top-level text.

### SIGN-OFF — 2026-09-03 — Patrick signed off Phase 2. Step 6 CLOSED. Phase 5 UNBLOCKED; Phase 4 UNBLOCKED (both design gates now signed).

- **Authority:** Patrick, 2026-09-03, at the conductor's gate after round 2 closed CLEAN (0 blockers
  across `plan-adherence-medium` and a fresh `doc-auditor-medium`; rounds sealed at `6a416c0` and
  `1ac1ab1`). Signed on the packet as revised by `m8-p2-fix1-20260903`, with every default the
  conductor recommended.
- **What is signed:** Auto-Arc topology (B) — one shared heap copy, N task references, the caller
  keeps its original plus one transient reference released after the last spawn of the group;
  beneficial-emission = ≥2 spawn statements in one block passing the same whole binding, no
  suspension between, `Reads` on both sides, `arc_shareable` type — "caller + 1 task" is OUT; the
  transfer rule — ONE `provenance()` exhaustive over `Expr`, ONE `check_transfer`, a closed list of
  free-positions, binding events exhaustive over all ten `Stmt` variants, keyed by scope entry,
  rebinding leaves the old class and revives the name; fr12 — decimal128 as a send-minted 16-byte
  cell freed at receive, `number` copy-through (NOT in the give set), `ChannelElemDrop::NumberCell`
  + `transfers_source()`; the `.copy()`-only override direction (`.give` body syntax does not exist).
- **The twelve packet items, each ruled:** (a) container-store/literal-element sinks DEFERRED
  four-field, literal elements `Reaches`; (b) whole relay chain reported in ONE compile; (c) alias
  classes ACCEPTED as a language change — `let other = rows; wire.send(rows)` makes `other`
  unusable, the read is refused, the holder is named; (d) `TransferNeedsCopy` is the registry name;
  (e) `number` copy-through CONFIRMED (parked item 3's assumption withdrawn); (f) `dynamic Contract`
  covered by construction, in Phase 4 scope; (g) relay-a-received-value pays `.copy()` this
  milestone — **with a FOUR-FIELD deferral for a consuming move-out-of-`maybe` form** (WHAT: a
  consuming accessor on `maybe<T>`; WHY: one extra allocation per relay hop vs. a language-wide
  `maybe` semantics addition outside a concurrency milestone's charter; COST: a `maybe` design
  addendum, one provenance arm, one codegen path; TRIGGER: a measured relay workload or the general
  move/drop story landing) — Patrick asked which option leaned toward `no-duct-tape.md`; the answer
  recorded: neither is duct tape, and "accept" stays compliant only while all four fields are
  written, which the sign-off round must do; (h) `bg_arg_kind_is_releasable_payload(ARC)` is
  Phase 5's; (i) the plan's "never enumerating `Expr::` variants" sentence is AMENDED — one
  exhaustive, wildcard-free match in the owning module is the remedy for open per-site lists, not
  an instance of them; (j) contract-signature modifiers stay parser-OPTIONAL, bare = never a give
  position, no parser enforcement; (k) revive-on-rebind ACCEPTED (today's false error on
  `eat(rows); rows = [4, 5]; rows.count()` becomes legal); (l) top-level rebinding between spawns
  is an Arc-group boundary, nested is a decline.
- **What this ruling authorizes under FRAGO 003's standing gate:** the sign-off round may edit
  every downstream plan section the Phase 2 STATUS block lists as owed (Phase 4 steps 3b/5 + a new
  fr12 step, Phase 5 steps 2–3, the Invariants subsections, FR#9/#10, Phase 9, `plan.md:818`) —
  each edit traces to this record. It also applies parked items 19–27 (text-accuracy corrections on
  the signed design) so Phase 4 never reads a known-false claim.

### FRAGO 014 — 2026-09-04 — Gate-coverage defect (conductor): Phase 5 sealed with `jargon_audit` red; the rule that closes it — PENDING Patrick's ratification

- **Trigger:** Phase 7's `plan-adherence` seat tagged `frago-needed`. Phase 5's registry edit
  (`auto-arc-codegen-emission`, landed `0f62869`) used the banned word "alias"
  (`[[banned_jargon]]` since `4aef3b8`); `jargon_audit::no_banned_jargon_in_deferred_feature_user_
  facing_fields` was 9/10 at Phase 5's boundary `d1c4294`. Neither Phase 5 gate ran
  `ynz-diagnostics` — the conductor's affected lanes named the crates the diff touched and never
  the crate whose TEST reads the registry. The Phase 7 executor found it and fixed the word
  (`alias` → `share`, no semantic change); the gate-coverage gap is the conductor's.
- **The rule, adopted immediately for every remaining gate and proposed for the skill:** any diff
  that touches `registry/features.toml` puts `cargo test -p ynz-diagnostics --test jargon_audit`
  and `cargo test -p ynz-registry` in the affected lane, regardless of which crates the diff
  names — the registry's consumers are the lane, not just its editors. Phase 9's full-workspace
  gate catches anything the same class hid elsewhere.
- **Authority:** Patrick's ratification owed (asleep at the time); the rule is applied meanwhile as
  conductor discipline, since applying it can only widen a gate. Tagged `frago-needed` by the seat
  because a sealed phase boundary carried a red test the plan's own gating promised it wouldn't.

### FRAGO 012 — 2026-09-04 — M8 PAUSES after Phase 5 seals; a shape-with-`number` `background` SIGSEGV on `main` takes priority as its own hotfix

- **Trigger:** Phase 5's executor (`m8-p5-20260904-a1`), building the Arc fixture class, found that a
  shape carrying a `number` field SIGSEGVs on ANY `background` spawn — one spawn, the ordinary
  copy path, zero Arc involvement (parked 40, repro recorded). It is live in released v0.3.3 and
  reachable from the consumer projects that mount `target/release`.
- **Patrick's ruling, 2026-09-04:** hotfix NOW on its own branch (`fix/bg-arg-number-field`),
  the FRAGO 004 precedent — a memory-safety bug on a consumer-mounted release does not wait for
  the remaining phases. M8 pauses at the Phase 5 boundary; the hotfix branches from `main`, lands
  by PR, merges back into `feat/v0-3-m8-concurrency-completion` before Phase 7 dispatches. RED pin
  first. No tagged release unless Patrick says so (the UAF hotfix rode M8's eventual release).
- **Why its own branch and not `fix/errors-fields`:** different ancestor — the bg-arg heap-clone
  layout for shapes with a 16-byte `number` field vs the `errors`-value field surface. Two root
  causes do not share a branch.
- **Applied:** this record; parked 40 annotated ROUTED; the Phase 5 boundary commit body carries
  `FRAGO-012`.
- **Amended the same day — M8 does NOT pause.** The hotfix runs in its own linked worktree
  (`/home/redacted/development/ynz-lang-hotfix-bgarg`, branch `fix/bg-arg-number-field` off `main`
  at `ec014d8`, compose project `ynz-lang-hotfix-bgarg-be028ff7`, provisioned by
  `worktree-birth.sh --no-deps` — the host has no `cargo`; everything builds in Docker), in
  parallel with Phase 5's fix round in the main checkout. Two executors, two trees, no shared
  state — the `one live checkout per branch` rule holds because the branches differ. Hotfix
  dispatch `bgarg-number-20260904-a1`. **PR #90 opened 2026-09-04**
  (https://github.com/yinzers/yinz-lang/pull/90, `ee7158c`, both gates green, RED re-proven on a
  clean `ec014d8` checkout). Sequence from here: merge on GitHub (Patrick) → merge `main` into
  `feat/v0-3-m8-concurrency-completion` **before Phase 8 dispatches** — Phase 7 runs meanwhile,
  since a design phase on the drop ladder / handle lifecycle touches different regions of
  `emit.rs`/`runtime.rs` than the hotfix's frame-layout change and idling on a GitHub button is
  waste (conductor's sequencing call, not a ruling) — and parked 43's fixture rides that merge
  commit (the same shape as PR #89's merge-back at `6143c1d`). The worktree is removed after the
  merge-back.

### FRAGO 011 — 2026-09-04 — Phase 4 CLOSED BY CEILING; the `errors`-value field surface is re-homed to its own hotfix branch after M8

- **Trigger:** Phase 4's fix loop reached the three-round cap with two blockers open
  (`code-reviewer-high`, round 3): the new `.failed()` guard is name-keyed and admits a shadowed
  rebinding; `.trace`/`.suggestions`/`.source` inside a `.failed()` check ICE (codegen lowers
  `.message` only). Round 2's blocker (`.message` null-deref on the not-failed path) and parked 32
  (`.failed()` twice both true inside an errors-capable function) share the same ancestor.
- **The producer, named:** the `errors`-value field surface was never finished end-to-end — typeck
  typed the four fields unconditionally, codegen lowered none of them. Phase 4 met it only because
  a REF example read `.message`. Three rounds patched instances of that one producer; per
  `root-cause.md` a recurring class is a diagnosis failure, and per `execute-plan` a fix that opens
  a gap earns one round, not a reset. Neither open blocker is memory-unsafe: the guard hole prints
  `""` (codegen's `br`/`phi` defense holds), and the sibling-field ICE is loud and pre-dates M8.
- **Patrick's rulings, 2026-09-04:**
  1. **Close Phase 4 by ceiling.** Round 3 seals as-is; both blockers parked four-field (33, 34);
     Phase 4's exit criteria are met for its own charter (close live, typed closed error reachable,
     P2-3 through the one choke point, RED→GREEN class committed at `6b8a34d`, stale entry retired,
     registry entries added, named suite + loom green).
  2. **One hotfix branch, `fix/errors-fields`, for all three** (parked 32, 33, 34) — the FRAGO 004
     precedent — **after M8 closes** (milestone PR up first). Exposure meanwhile: a compile-time
     guard with one shadowing hole that yields `""`, and a loud ICE on a gap older than this
     milestone. Each defect gets a RED pin before its fix.
- **Applied:** this record; parked 32 annotated ROUTED, 33/34 written; the Phase 4 block's round-3
  bookkeeping; the boundary commit carries `FRAGO-011` and `m8-p4-fix3` so every pointer resolves.

### FRAGO 009 — 2026-09-03 — The three items Phase 1's sign-off left open are RULED; Phase 2 begins

- **Trigger:** the SIGN-OFF record above named three questions Patrick's signature did not answer.
  The successor conductor (`conductor-2026-09-03-m8-phase2`) asked all three before Phase 2's first
  dispatch, per that record's instruction.
- **Patrick's rulings, 2026-09-03:**
  1. **fr12 rides Phase 2.** `channel<number>` decimal128 marshalling is designed in Phase 2 alongside
     the ownership rule (parked item 3 already ties `number` to the `ChannelElemDrop` give-set
     question Phase 2 owns); Phase 4 implements. Not its own FRAGO'd step, not deferred out of M8.
  2. **`HandleChannelArgNeedsBinding` is a hard compile ERROR**, as designed. GR5; the hang class dies
     at compile time; the fix is one `let`; pre-1.0 with zero users, so rejecting never-closed
     source that compiles today costs nothing real. Phase 4 step 3c ships it as an error.
  3. **FRAGO 003's ratification is STANDING, traceability-gated.** A fix round may edit downstream
     plan text (including Gate-4-signed sections) when every edit traces to a ruling Patrick made
     in that round; `plan-adherence` verifies the trace as part of its ordinary grade. An edit that
     traces to no ruling halts to Patrick. Rounds 2 and 3 of Phase 1 are covered retroactively by
     the same rule (their edits were already reviewer-verified as traceable).
- **Also settled at the same gate:** Phase 2 stays in this checkout on
  `feat/v0-3-m8-concurrency-completion` (worktree gate re-answered for this session).
- **Applied:** the plan's cold-resume banner (the "owed" line becomes the answered line), a FRAGO 009
  block inside Phase 2's own plan block carrying the fr12 obligation, and the session-id chain.

### FRAGO 008 — 2026-09-03 — The ownership question MOVES from Phase 1 to Phase 2; Phase 1's fix loop closed by re-diagnosis, not by ceiling

- **Trigger:** three consecutive Phase 1 review rounds returned the SAME class of finding — an
  ownership rule that could not see every holder of a value. Round 1: `send` freeing a payload it
  never took. Round 2: consume not reaching the caller through an unannotated parameter. Round 3:
  three more blockers — UFCS non-receiver arguments never reaching `check_arg_ownership`
  (`check.rs:3063-3085`, `:5431-5479`), non-ident expressions satisfying a `give` parameter while the
  caller still holds the value (the ident-only gates at `:4798`, `:5114`, `:5444`), and a `for`-loop
  variable admitted as a fresh payload when it is a cell pointer the parent still owns
  (`check.rs:2792-2803`). Round 4 was already visible before it ran: `dynamic Contract` dispatch
  carries no ownership modifiers at all (`shapes.rs:24-29`).
- **`root-cause.md` governs:** a recurring finding class is a diagnosis failure, not a fix
  opportunity. The loop is closed by re-diagnosis rather than by patching the fourth instance.
- **The producer, named:** the design derived *"who else holds this value?"* by **enumerating
  syntactic argument shapes at call sites** rather than threading the one authoritative answer. A
  syntactic site list is unbounded and grows with the language, so each new expression form silently
  reopens the hole. Every round fixed its instance correctly and was blind to the next form.
- **What was sitting unused:** `crates/ynz-typeck/src/effective_ownership.rs` — a whole-program Kleene
  fixpoint over every parameter that converges under mutual recursion, runs before body checking
  (`queries.rs:484-512`), and **already classifies "passed to a declared `give` position"** as
  `Writes` (`:410-411`, `:676`, test at `:1391`). The design's stated reason for reporting one frame
  per compile — that typeck lacks a callee-before-caller ordering — is **factually false** against
  this module, so that decision rested on a wrong premise and is void.
- **The aggravating detail.** This plan's **Phase 2** exists specifically to reuse
  `effective_ownership` rather than re-derive it, under `authoritative-derivation.md`. Phase 1 spent
  three rounds re-deriving a weaker version of that module, inside the same milestone, against the
  same rule, with the rule's canonical example one phase away in the same document. Recorded as the
  first entry in `.claude/corpses.md`.
- **Patrick's ruling, 2026-09-03: merge the ownership design into Phase 2.**
  - **Phase 1 KEEPS** (and is otherwise ready for sign-off): the close mechanism (explicit
    `.close()`, `Option<mpsc::Sender>` + `.take()`); the name, decided against `vocabulary.md` and
    GR12; bare `receive()` returning `maybe<T>` with the handle's `T errors` deliberately distinct;
    the idempotency, wake-all, in-flight-completion and drop-unchanged contract points; the
    `refuse_closed` collapse of the three first-poll CLOSED arms; the ruling that P2-3's free lives in
    the runtime rather than codegen's closed arms (FRAGO 006); the `Option<ChannelElemDrop>` element
    classification; the `HandleChannelArgNeedsBinding` diagnostic (a close-ability question, not an
    ownership one); and the four-field auto-close deferral.
  - **Phase 2 ABSORBS** the whole *"who else holds this value"* question and every diagnostic that
    depends on it: `ConsumedBySend`, `ParamNeedsGive`, `SendPayloadNeedsCopy`, the transit rule, the
    admitted-payload-form set, and the `.copy()`-on-`map` obligation that exists to make that advice
    executable. Phase 2 answers them by threading `effective_ownership`, never by enumerating syntax.
  - **Phase 4 stays blocked on BOTH** Phase 1's and Phase 2's sign-offs, since the channel-close
    implementation now depends on an ownership rule Phase 2 owns.
- **Not a ceiling closure.** The three round-3 blockers are NOT parked as accepted residual — they are
  re-homed as Phase 2's design inputs, with the concrete failing programs preserved so Phase 2's
  answer must defeat all three or explain why each is out of scope.
- **Blast radius carried forward:** `examples/primantis-orders/m6_errors.ynz:112-115` passes a bare
  parameter `fig` as a receiver into `haulCircle(give self: Circle)` — a real instance of the silent
  consume at `check.rs:4617` that any `give`-tightening rule will convert into a second diagnostic on
  that line. The gallery's count assertion (`error_galleries.rs:100`, allowing 7–14) probably absorbs
  it, and the `// WHY:` comment needs updating. The design's "zero existing instances" claim is false
  until this one is accounted for.

### FRAGO 005 — 2026-09-03 — FRAGO 004's ruling 2 is misrecorded; what shipped is not what it says

- **Trigger:** Phase 1's fix round 2 read the merged hotfix code (`861fd4d`) rather than its commit
  message, and found the record does not match the mechanism.
- **The correction.** FRAGO 004 records Patrick's ruling 2 as *"ladder consults consumption — codegen
  skips the ladder free for a binding typeck already marked consumed by a send."* **What shipped
  consults no typeck fact at all.** `crates/ynz-typeck` is untouched by the hotfix. The shipped
  protocol matches the hand-off EVENT at runtime by pointer identity (`release_ladder_payload`,
  `DriveIdentity`, `BG_ARG_KIND_RELEASED`). The commit names why the literal ruling was infeasible in
  a patch: one compiled body cannot know whether it was spawned or `wait`ed, the frame header is full,
  and a name-based match misses aliases like `let a = rows; wire.send(a)`.
- **Patrick's intent was preserved; the mechanism differs.** The ruling's substance — the ladder must
  stop freeing what the task gave away — is exactly what shipped. This record exists so a later reader
  comparing FRAGO 004 against the code does not conclude the fix diverged from its own authorization.
- **Consequence for Phase 4, now settled in the design** (`IMP-concurrency.md`, Channel Close section):
  the two mechanisms are NOT twins and neither is removed. Typeck consumption answers *may the source
  read this binding again* (binding-level, authoritative for `ConsumedBySend`); the runtime release
  answers *does this task's ladder still own this allocation at retire* (allocation-level,
  authoritative for double-free and leak). The compile-time link that keeps them from drifting is the
  `ChannelElemDrop { None, Array, Map }` enum: typeck consumes iff `!= None`, `channel_drop_glue`
  matches it exhaustively, and the runtime releases iff the channel has glue — so adding an element
  kind on one side is a non-exhaustive match on the other.

### FRAGO 007 — 2026-09-03 — Ownership must flow through the call: a parameter that is given away must be declared `give`

- **Trigger:** Phase 1's third review round constructed a program that compiles under the round-2
  design and is a deterministic use-after-free through plain `wait`, no `background`:
  `function producer(wire: channel<array<int>>, rows: array<int>) { wire.close(); wire.send(rows) }`
  called as `wait producer(wire, rows)`, then `print(rows.count())` in the caller. The send consumed
  producer's PARAMETER; the caller's `rows` stayed live; the send core freed the payload on the
  closed path.
- **Root cause, named:** `check_arg_ownership` (`check.rs:4591–4648`) consumes the caller's binding
  only when the callee's parameter is declared `give` (`:4599`); unannotated and `share` parameters
  fall to `_ => {}` (`:4646`); and the helper runs only for plain-identifier arguments
  (`:4798–4800`). Ownership never flowed through an ordinary call. Harmless while nothing freed a
  value; load-bearing the moment a channel does.
- **Patrick's ruling — guard A, both halves:** (1) a parameter binding sent on an owned-heap channel
  must be declared `give` on its enclosing function — compile error `ParamNeedsGive` otherwise;
  with `give`, the EXISTING Give path consumes the caller's binding at every call site and every
  spawn site (the `Expr::Background` arm's `infer_expr(inner)` reaches `check_user_fn_call`), no
  second analysis. (2) A non-identifier owned-heap payload is refused (`SendPayloadNeedsCopy`) with
  the `.copy()` WHAT-INSTEAD; `.copy()` postfix and literals are admitted as fresh.
- **Consequences the ruling did not spell out, decided in the design and surfaced for the sign-off:**
  (a) the obligation transits the WHOLE chain — every relay frame declares `give` — enforced at the
  one site, `check_arg_ownership`'s Give arm, where `:4617` today silently consumes a bare/`lend`
  parameter passed to a `give` parameter; that becomes `ParamNeedsGive`, so a bare parameter can
  no longer be relayed into a give without the word (a language-ergonomics change to shipped
  behavior; the fixture corpus is compiled in Phase 4 to confirm zero existing instances); (b) the
  error names the immediate frame only, one frame per compile — the honest cost, with the WHY
  stating the word travels up; (c) the `:4611` share refusal is retired into the new template;
  (d) inferring `give` for bare parameters was weighed and rejected (callee-before-caller
  ordering/fixpoint; silent change to the caller's program from another body; signatures carry
  ownership per `inference.md`); (e) FR#9's channel-door instance is CLOSED by (1) — `background
  producer(wire, table)` with `give table` consumes the parent's `table` at the spawn — and the
  plan's FR#9 text now says so; the container door stays.
- **Authority:** Patrick, 2026-09-03 (ruling relayed by the conductor with the fix-round-3 brief).
  The transit rule (a) and retirement (c) are listed under the design's "Open at sign-off" for his
  explicit confirmation at step 7, because they widen the ruling beyond the send arm.

### FRAGO 006 — 2026-09-03 — P2-3's leak fix MOVES to the runtime; freeing in codegen's closed arms would double-free

- **Trigger:** same read. The shipped CLOSED-first-poll path documents "the sender still owns it" and
  a shipped test (`ladder_is_untouched_when_the_channel_does_not_take_ownership`, case (c)) asserts it.
  That is correct **today**, in a world where `send` does not consume.
- **The defect this catches before it is built.** Under Phase 1's design `send()` gives its payload on
  every outcome, so a payload refused by a closed channel has no source owner left. The plan's
  original P2-3 fix frees it in codegen's closed arms — and for a ladder-owned spawn-arg clone that is
  a **double free**: codegen's arm frees it, then the ladder frees it again at task retire. The plan's
  own Phase 4 step 4 would have built this.
- **Resolution recorded in the design:** P2-3's free moves into the runtime's CLOSED-first-poll path
  (`release_taken_value()` + glue, mirroring the shipped re-poll-CLOSED arm the hotfix added).
  Codegen's closed arms free nothing. Phase 4 deliberately flips that runtime doc comment and that
  test case, rather than tripping over them.
- **Authority:** none needed — this is a design correction inside Phase 1's own remit, surfaced before
  implementation and gated by Patrick's still-open step-7 sign-off. Recorded as a FRAGO because it
  changes what Phase 4 step 4 is instructed to do.

### FRAGO 003 — 2026-09-03 — Downstream plan edits and the `param_ownership` schema field, RATIFIED

- **Trigger:** Phase 1's round-2 `plan-adherence` seat tagged `frago-needed`.
- **Finding:** the round-1 fix executor, from inside a DESIGN phase, edited downstream plan sections
  — Phase 4 steps 3/3b/4/5, Phase 9 step 2, and three Invariants subsections (Safety, Teaching,
  Feature Registry Entries) — and added a new optional schema field, `param_ownership`, to
  `[[primitive_intrinsic]]` (there is no ownership field today; `features.toml:581-587`). The
  Feature Registry Entries subsection carries Patrick's Gate-4 signature of 2026-07-04, so the plan
  as approved is not the plan as it now stands. A registry SCHEMA change also has cross-crate blast
  radius into `crates/ynz-registry/build.rs` codegen.
- **What was NOT wrong:** the reviewer independently verified every edit traces to one of Patrick's
  two rulings or a directly-consequent obligation the design doc itself justifies. **No smuggled
  decisions.** It also re-derived the 19-site blast radius from scratch and matched, correctly
  excluding every handle-side `.receive()`. The gap was process, not content.
- **The counter-argument, recorded because it is sound:** leaving Phase 4 step 3 unedited would have
  left it instructing an implementer to wire `receive()` to `T errors`, contradicting the `maybe<T>`
  ruling made in the same round. A self-contradictory plan is worse than the overreach.
- **Authority:** Patrick, 2026-09-03 — ratify the edits, keep `param_ownership`, log the formal
  FRAGO, and re-sign the changed Feature Registry Entries subsection.
- **Applied:** this record. `param_ownership` stands as a ratified schema addition; its build-time
  validation is parked (`.claude/plans/parked.md`, item 5) as Phase-4 implementation work — without
  it a typo or a length misalignment ships silently as hover text.

### FRAGO 002 — 2026-09-03 — Two dangling citations corrected in the cold-resume banner

- **Trigger:** Phase 0's drift check distinguished offset-only drift from citations that now point at
  unrelated code entirely.
- **Finding:** `runtime.rs:591-693` ("the drop ladder" — the choke point **Phases 4 and 7 both wire
  through**) is now `ynz_rt_shutdown`; the real drop ladder is `runtime.rs:981-1050`. The kernel-mode
  gates cited as `check.rs:3392-3398`/`3047-3059` are now `~4316-4322` and `~3972-3980`.
- **Applied:** a correction table in the plan's cold-resume banner, where a resuming reader hits it
  before navigating by any citation. Original prose left unedited — the banner is the correction of
  record. Offset-only drift listed alongside it so a reader can tell the two classes apart.
- **Why this was not left to the phases that use it:** Phase 4 and Phase 7 both navigate to the drop
  ladder by that citation. A wrong anchor there routes an executor into shutdown code while it
  believes it is reading cleanup dispatch — precisely the kind of silent wrong-turn this plan's own
  `authoritative-derivation.md` discipline exists to prevent.

## Context-segment log

(none yet — this plan has not begun execution)

- `conductor-2026-09-03-m7-merge-and-precondition-clear` — 2026-09-03 — **Preconditions cleared; plan
  is now genuinely startable at Phase 0.** No plan content changed beyond the status block and
  frontmatter; execution has still NOT begun.
  - **Cleared the double merge-and-tag precondition.** M7 was complete and sealed but its branch had
    never merged: PR #87 sat open with a red sanitizer lane. Root-caused that red to a pre-existing
    TSan flake in `panic_reraises_in_parent` (a 50ms sleep then a single poll — the poll returned
    `Poll::Pending` under instrumentation, so `resume_unwind` never fired). Confirmed pre-existing,
    not an M7 regression: the identical failure occurred on `main` at the released v0.3.2 sha on
    2026-07-16 and passed on that same sha on 07-24/07-29/07-31. Fixed by polling to readiness with
    a liveness deadline (`12f397b`); a duplicated `check_preempt` benchmark hiding behind it was
    collapsed (`67b3148`). PR #87 merged at `f7eb2fa`; v0.3.3 cut and tagged.
  - **Corrected the status/frontmatter contradiction.** `status: "active"` had been set while the
    status note still declared the plan "deliberately held at `paused`" pending those merges. A cold
    resume this session cost roughly a dozen tool calls to resolve which was true. The note is now a
    COLD-RESUME ENTRY POINT block stating the entry phase, a precondition table with evidence, and
    the tree changes Phase 0's re-read should expect.
  - **Tree changes this session that Phase 0's re-verification will encounter** (all on `main` at
    `cf17de3`, PR #88): `ynz_fmt::format()` was cubic — `comment_merge::line_of` rescanned the source
    from byte 0 per call, 91.18s on the 1,352-line `pirates-roster/entrypoint.ynz`, live in the
    just-released v0.3.3 — now `partition_point` over precomputed newline offsets (88.86s → 0.06s).
    The corpus sweep is parallel, and BOTH corpus sweeps now derive scheduler-dependence from the
    fixture SOURCE rather than its filename (the old name-substring proxy had drifted: 91 fixtures
    use `background`, only 16 were named for it, leaving 75 asserting a byte-identical ordering the
    language never promised). `/tmp` is tmpfs+exec in the dev container. Full workspace suite
    51.8 min → 4.5 min, same coverage.
  - **Relevant to this milestone specifically:** any new `background`/channel fixture M8 adds is now
    auto-classified by the corpus sweeps with no exclusion-list entry to remember; and
    `.claude/rules/test-parallelism.md` must be loaded before adding any fixture-looping test here.
  - Appended this session's id to the frontmatter `session-id` chain in the same action as this entry.

- `conductor-2026-09-03-m8-execution` — 2026-09-03 — **Phase 0 EXECUTED. Verdict: PROCEED.**
  Dispatch `executor-medium` (cell: coding/low/mechanical), dispatch-id `m8-p0-20260903-a1`.
  Read-only gate; zero code changes, so zero reviewer seats derived and green-check skipped — a
  round with no gradeable decision earns zero seats per `reviewer-seats.md`'s own carve-out, applied
  mechanically by the conductor. No round-seal commit either: nothing to seal.
  - **Pre-flight:** no `.claude/rules/branching.md` existed. Patrick's answer recorded and written to
    that file: `main` is protected, plan work lives on `feat/<slug>`, close-out is a PR via `/pr`,
    never a direct merge. Plan frontmatter now carries `branch:
    feat/v0-3-m8-concurrency-completion` so a cold resume can find the ref. A second gate (the
    worktree-ask gate) was answered "stay on this branch, in this checkout" and granted for the
    session.
  - **Double merge-and-tag gate: SATISFIED.** `main` at `cf17de3` contains PR #82 (M6, merge
    `10df6d7`) and PR #87 (M7, merge `f7eb2fa`, tagged v0.3.3 at `1aee207`).
  - **CRITICAL FINDING — Phase 6's premise is false, not drifted. P2-7 appears already fixed by M6
    itself.** M6 un-deferred P2-7 under its own FRAGO 010 and shipped it as M6 Phase 4b, commit
    `b0cdbd3` ("fix(runtime): close ynz_handle_recv_poll panic-then-Pending hang (M6 P4b)",
    2026-07-11), inside PR #82; the register-before-poll fix reads as live at `handle.rs:339-378`.
    If confirmed, this plan's ¶1 Terrain ("NOT fixed by M6"), its Design-Doc Alignment §4 boundary
    claim, risk row R6, and all of Phase 6 target already-completed work. **Not auto-applied:** the
    finding is a single unconfirmed executor claim, and retiring a whole phase is a mission-scope
    call. Patrick directed "verify first, then retire" — an independent `code-reviewer-medium`
    confirmation was dispatched before any amendment lands. This entry records the finding as
    PENDING CONFIRMATION, not as fact.
  - **Citation drift recorded — two are DANGLING, not merely offset (they point at unrelated code
    today):**
    - `runtime.rs:591-693` "the drop ladder" → those lines are now `ynz_rt_shutdown`. The real drop
      ladder Phases 4/7 must wire through (the kind-2 `BgArgDropEntry` arm calling
      `channel::purge_pending_sends` + `ynz_channel_free`) is at `runtime.rs:981-1050`.
    - `check.rs:3392-3398` / `3047-3059` (kernel-mode gates, cited in this plan's Kernel-Mode
      Behavior subsection) → construction gate is now `~4316-4322`, method-call gate `~3972-3980`.
      Substance holds; the cited lines are unrelated code.
  - **Citation drift, substance intact (offsets only):** `channel.rs:109-123` → `200-225`;
    `channel.rs:536-539`/`557-560` → `962`/`1123`; `channel.rs:120` (`pending_sends`) → `213`, shape
    now `Mutex<HashMap<(u64,u64), PendingSendEntry>>`; `emit.rs:~11833-11960` → `~12776-12961`, with
    the P2-3 leak genuinely still unfixed and the "Structurally unreachable in v0.3-M4" comment still
    present verbatim at `emit.rs:12852`; `IMP-no-function-coloring.md:281-294` → `295-311`.
    `registry/features.toml:1229-1235` and `IMP-no-function-coloring.md:58` are exact, zero drift.
  - **Registry entries confirmed present and matching this plan's characterization:**
    `auto-arc-codegen-emission` (`features.toml:1230`), `auto-arc-cautionary-tint` (`:1351`),
    `background-handle-cancel-injection` (`:1174`).
  - **Non-absorption re-affirmed in BOTH duplicate Capability Ledger tables** (roadmap lines 446/516
    and 452/522): "Selective hot-field-only element materialization" and `background.cpuBound`
    (P4-2) both still un-absorbed, triggers unchanged.
  - **Assumption A4 confirmed:** `crates/ynz-typeck/src/effective_ownership.rs` exists;
    `EffectiveOwnership::{Reads, Unknown, Writes}` is real. Semantic-reuse correctness deliberately
    left to Phase 2, per that phase's own step 1.
