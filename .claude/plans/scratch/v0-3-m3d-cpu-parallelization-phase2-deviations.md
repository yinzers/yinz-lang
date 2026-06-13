# v0-3-m3d-cpu-parallelization Phase 2 Deviations — round 4 (re-captured 2026-06-13T03:30)

D_count: 10

Tree-integrity snapshot (round 4, before re-gate fan-out):
- status_hash: ab4645dd83a52c564ead0c10cb33950c3ecba9f7
- diff_hash: e03aa4fb9995443e908d299d6f50186281088ad4
- per-path blob hashes:
  - crates/ynz-typeck/src/queries.rs aae9b44dbd08109ae9793ecc4fa55b5555249318 (CHANGED R3-fix — cpu_promotion_query Big-O + 3 for-wait decline fixtures + rename + error_whats helper)
  - crates/ynz-typeck/src/independence.rs 62f33322ad8c18c8c4449cc467fb3796bdf93f26 (CHANGED — 2 Big-O)
  - crates/ynz-typeck/src/lib.rs 7b83f1c3ee8c9a1b9e26eb7adabb3acb08596b36 (CHANGED — dropped compute_cpu_promotions re-export)
  - crates/ynz-typeck/src/check.rs eaadd78c19ea5522ee51acf0dff6823bf1c3d9a7 (unchanged)
  - crates/ynz-lsp/tests/completion.rs af89cf03e6802246ec5aa616c2aa7da8c25f2ee4 (unchanged)
  - crates/ynz-lsp/tests/hover.rs a095270130057ccf915f7a401126dbdade3ebdf6 (unchanged)
  - crates/ynz-typeck/tests/check.rs 6d43fafee893a12a651464f20b1fae0617337ee3 (unchanged)
  - crates/ynz-codegen/src/lib.rs 57f4e84be6d84354add3f5a01a6837aadc923072 (unchanged)
  - crates/ynz-typeck/src/resolve_import.rs 387e18b6f9445d1ff4e350820e3da5237069805c (unchanged)

Diff base (BASE): 23f4e81. HEAD = 724a765 (verbatim relocation commit). Diff form: `git diff 23f4e81`.

## Resolved spawn list (round 3)

### Deviation #1 (scope: completion.rs) — identity 41af42b993fc0520f37810c68d1bf9fd8787cc6e — CARRY PASS (blob unchanged)
### Deviation #2 (scope: hover.rs) — identity 90b5e2bab5242c5a4404eca0c1e22673977c9c73 — CARRY PASS (blob unchanged)
### Deviation #3 (scope: tests/check.rs) — identity e7946cdec9f5e23a7fe6d61a38873da5240ef4d2 — CARRY PASS (blob unchanged)
### Deviation #4 (approach: probe param-shadow predicate, check.rs) — identity f35246ed951f9ba110aa1fc42cf619dbc4cf2750 — CARRY PASS (check.rs blob unchanged eaadd78c)
### Deviation #5 (approach: per-candidate CPU-callee seed, queries.rs) — identity b008d70a82fb1aa3c53383ebe678ad70051c73fb — RE-FIRE (queries.rs blob changed; PASSed R2, re-confirm Big-O/visibility edits didn't disturb seed logic)
- diff hunks: crates/ynz-typeck/src/queries.rs:786-845, crates/ynz-typeck/src/queries.rs:933-960
### Deviation #6 (approach: kernel_mode=false query entry, queries.rs) — identity 55616887e442d0ccf1b34a4320b170f6bc9c5792 — RE-FIRE (queries.rs blob changed)
- diff hunks: crates/ynz-typeck/src/queries.rs:625-631, crates/ynz-typeck/src/queries.rs:698-702
### Deviation #7 (scope: typeck/lib.rs re-exports) — identity 86c548c492e1e7d07521531d51c13bc98819da76 — CARRY PASS (R3 judge #7 PASS; lib.rs blob unchanged since R3: 7b83f1c3)
- diff hunks: crates/ynz-typeck/src/lib.rs:77-79
### Deviation #8 (scope: codegen/lib.rs dereg) — identity 10ba7e3935ae7c5fd762fbf8669788cbbf31d6dd — CARRY PASS (blob unchanged 57f4e84)
### Deviation #9 (scope: resolve_import.rs propagation) — identity 8429db77cdb857301918c876e9575cadecd99885 — CARRY PASS (blob unchanged 387e18b)
### Deviation #10 (approach: for-wait decline fixtures assert `!promoted` + "zero NEW diagnostics", not `!has_errors`) — identity 542b08aec6c657cd48b072715d49d78a0ec66747 — RE-FIRE (BLOCKed R3; rationale corrected + 3 real fixtures added — verify guards isolated, contract honest)
- type: approach
- rationale: the three for-with-wait probe guards (StoredRangeWithWait / FixedArrayIterWithWait / ExpressionIterWithWait) ARE reachable (the earlier "structurally unreachable" claim was FALSE — corrected). The reachable shape is a host that `wait`s a NON-suspending callee inside a `for` loop: `has_explicit_waits == true` in the probe (`block_contains_wait` reads the host's own body), but the waited callee reaches no may-block call, so the host is NOT in `base_suspends` → it survives the candidacy gate (queries.rs:763) as a real CPU candidate and the for-wait guard fires in the probe, declining it non-vacuously. BUT the same explicit `wait` trips `check_function`'s PRE-EXISTING M2/M3 for-wait guards (check.rs:584-650), so the shape ERRORS at baseline regardless of M3d — it never "compiled pre-M3d". Step 7's contract targets shapes "that compiled pre-M3d"; that precondition does not hold for these already-erroring shapes. The honest, satisfiable contract is therefore `!promoted` (the meaningful M3d behavior) + "M3d introduced ZERO NEW diagnostics" — proven by asserting `error_whats` contains exactly the shape's OWN pre-existing for-wait guard message and none of the other two — NOT the impossible `!has_errors`. Each fixture isolates its guard (stored-range fixture asserts the stored-range message + absence of fixed/expr messages, etc.). A non-interference test (`wait_free_for_loop_does_not_block_promotion`) covers the complementary wait-free case (pair promotes).
- diff hunks: crates/ynz-typeck/src/queries.rs:1389-1410 (error_whats helper), crates/ynz-typeck/src/queries.rs:1615-1830 (renamed non-interference test + 3 decline fixtures + TICK const)
