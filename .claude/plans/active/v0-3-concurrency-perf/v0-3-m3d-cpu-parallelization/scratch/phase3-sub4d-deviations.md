# v0-3-m3d-cpu-parallelization Phase 3 sub-slice 4d Deviations — ROUND 4 (domain-enumeration drain) — 2026-06-15

D_count (round 4): minimal re-gate — see resolved spawn list

> R3 gate: BLOCKed (disc-offset binding [code-reviewer] + bare add(4) in old tests [rules]) — both were the
> NEXT members of a recurring class the no-progress hash couldn't see. Coordinator detected the
> recurring-bug-class (detector ii, Patrick-flagged) and switched to DOMAIN ENUMERATION: round 4 drains the
> ENTIRE spike frame-ABI constant set in one pass + adds a source-scan completeness gate (round-5 stopper).
> Live drain verified: ZERO bare spike-offset literals remain (coordinator re-grep). Corpus 2043/0.

## Round-4 Approach Deviations (verbatim from executor report)

- **Deviation #1** (handle 0/1 offsets — option choice): task offered (a) derive-at-use-site OR (b) const-assert, "pick lower-risk + justify". Chose (b) const-assert bindings. Rationale: "the two named constants are referenced by NAME in three comments (emit.rs:2399/6855/6886, zero-param-host decline reasoning) and the use site (emit.rs:7793) builds a fallback array entangled with the out-of-class result offsets [48,64]; deriving would orphan the comment refs + risk the result-offset fallback. const-assert fully closes drift with zero use-site risk". Diff hunks: `crates/ynz-codegen/src/emit.rs:6718-6733`.
- **Deviation #2** (member #3 bare-8 — verified NOT in class, NOT swapped): task hypothesized emit.rs:8057 `const_int(8)` was the bare stride; executor verified it is `ctx_size=8` (sizeof(i64) spawn-ctx buffer for ynz_rt_spawn_blocking_joinable), coincidentally 8, NOT the handle stride → left bare per the task's "flag don't mis-swap" instruction. Rationale: "swapping a ctx-buffer-size arg for SPIKE_HANDLE_SLOT_BYTES conflates two distinct semantics that are coincidentally both 8". Diff hunks: none (no change). [Coordinator PROBE E confirmed this classification independently.]
- **Deviation #3** (disc const naming): renamed all consumers to the ynz-abi name `SPIKE_FRAME_DISCRIMINATOR_OFFSET` rather than keeping a `pub(crate)` re-export of the old `FRAME_SPIKE_DISCRIMINATOR_OFFSET`. Rationale: "a pub(crate) re-export becomes a dead/clippy-unused import once lib.rs imports straight from ynz_abi; one canonical name across both crates is cleaner; mechanical 5-site rename". Diff hunks: `crates/ynz-runtime/src/runtime.rs:69-74, crates/ynz-runtime/src/lib.rs:2804-2807,3560,3673,3732,3793`.

## Resolved spawn list (round 4)

### Judge B (re-fire — tracks frame-ABI const-assert completeness) — IS THE CLASS FULLY CLOSED?
- type: approach (the whole domain-drain)
- rationale: round 4 added `SPIKE_FRAME_DISCRIMINATOR_OFFSET` to ynz-abi (both crates consume), const-asserts binding SPIKE_HANDLE_0/1_OFFSET to base/base+stride, swapped the disc GEP + 6 test literals to named consts, + a source-scan completeness gate. Confirm: (1) the disc-offset BLOCK (R3) is resolved — codegen + runtime read ONE canonical const; (2) the handle 0/1 asserts are load-bearing (not tautological); (3) the completeness gate is REAL + mutation-non-vacuous + correctly scoped (catches any new in-class bare literal; does NOT false-flag the out-of-class general-header offsets 0/8/16 or the ctx-size 8); (4) the bare-8 left-as-ctx-size is genuinely out-of-class. Does any in-class member remain unbound?
- diff hunks: crates/ynz-abi/src/lib.rs:25, crates/ynz-codegen/src/emit.rs:6718-6733 + disc GEP, crates/ynz-runtime/src/{runtime.rs,lib.rs}, crates/ynz-runtime/tests/spike_frame_abi_no_bare_offsets.rs
- judge identity: approach-frame-abi-domain-fully-closed-r4
- carry status: re-fire (was R3 BLOCK as disc-offset; now the whole-domain-closure judge)

### CARRIED:
- judge-A ynz-abi-extraction (R2 PASS): adding one more const to ynz-abi doesn't change its zero-dep/no-cycle/no-tokio properties. CARRY.
- judge-C NB1-checker (R2/R3 PASS): nounwind checker untouched this round. CARRY.
- codegen-ABI-no-op, lib.rs-colocation, m2_runtime, todos (R1 PASS): untouched. CARRY.

## Reviewer re-gate (round 4)
- code-reviewer: RE-FIRE (R3 BLOCK on disc offset — confirm resolved; review the const-asserts + completeness-gate quality + the bare-8 classification).
- rules-compliance: RE-FIRE (R3 BLOCK on bare literals — confirm all 6 gone + the new test file / ynz-abi const / asserts are rule-clean).
- acceptance-verifier: RE-FIRE (confirm 2043/0 + completeness gate passes + cancellation/nounwind tests green). [R3 acceptance run is STALE — round 4 changed the tree; this fresh one supersedes it.]
- plan-adherence: RE-FIRE delta-focus (confirm the drain is the in-class prescribed remedy, 4e gates + CPU_GROUP_MEMBER_COUNT byte-intact, no over-reach into the general-SM-header layer).
- design-compliance: CARRY R2 PASS (constants-relocation + asserts + a test touch no concurrency/coloring/kernel surface; the new ynz-abi const is kernel-safe).
