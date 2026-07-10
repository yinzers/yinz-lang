---
name: "v0-3-hotfix-int-literal-number"
plan-id: "2026-07-04-v0-3-hotfix-int-literal-number"
status: "stub"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["gate4-signatures-2026-07-04", "m6-p1d-crossplan-coord-2026-07-10"]
created_at: "2026-07-04"
updated_at: "2026-07-10"
metadata:
  type: "plan"
---

# WARNO / stub: Fix codegen ICE — bare int literal into a `number`-typed slot crashes the compiler

- **Mission (best understanding):** Fix roadmap Capability Ledger row 441 (both duplicate tables,
  [`2026-05-21-v0-3-concurrency-perf/roadmap.md`](../2026-05-21-v0-3-concurrency-perf/roadmap.md) —
  the codegen ICE where `store`/`store_field`'s `Type::Number` arm
  (`crates/ynz-codegen/src/emit.rs:19552-19557`, `:19674-19679`) unconditionally assumes a
  decimal128-pointer representation (`.into_pointer_value()`), while `Expr::IntLit`
  (`crates/ynz-codegen/src/emit.rs:14101`) lowers to a raw `i64`. Typeck ADMITS the coercion
  (`crates/ynz-typeck/src/check.rs:2162-2166`), so a program like `let x: number = 5` type-checks
  cleanly and then the compiler panics at codegen time ("Found IntValue … expected the PointerValue
  variant").

- **Situation (what I know):**
  - **ELEVATED priority** — this is a real, user-facing compiler CRASH reachable via arguably the
    most common beginner mistake in the language (a bare int literal assigned into a `number`-typed
    slot — no shape or array construction needed to trigger it).
  - **Estimated cost: ~0.5–1 session**, per the roadmap ledger's own estimate (row 441).
  - **Fix-shape candidates (not yet decided between):**
    1. An expected-type-aware `Expr::IntLit` lowering branch, mirroring `NumberLit`'s existing
       alloca-and-store pattern (`crates/ynz-codegen/src/emit.rs:14103-14136`), OR
    2. A typeck-level int→number literal coercion that materializes the decimal128 representation
       at the retype point, before codegen ever sees a raw `i64` in a `number` slot.
    Either candidate needs its own small call-site audit (an E7-style sweep, per the roadmap ledger's
    own framing) before implementation — not yet done.
  - **SCOPE WIDENED — M6 Phase 1d discovery: this int-literal→`number` class has THREE sibling facets
    sharing ONE root** (a raw `i64` reaches a decimal128 `number` slot with no coercion). The stub was
    originally framed as the store-site codegen ICE only; v0.3-M6
    (`2026-07-04-v0-3-m6-concurrency-hotfix`) Phase 1d confirmed the class is broader:
    1. **store-site** — `let x: number = 5` (this stub's original target; M6 Future-Req #9) — ICEs.
    2. **declaration-site field default** — `hidden cache: number = 5` — ICEs at
       `crates/ynz-codegen/src/emit.rs:20347`/`:20351` (`Found IntValue "i64 5" but expected the
       PointerValue variant`), from field-default lowering at `emit.rs:18233` (bare `lower_expr`, no
       expected-type hint, storing a raw `i64` into a decimal128 slot).
    3. **call-site / arg positions** — `background f(5)`, collection-method args, etc. (M6 Future-Req
       #14) — currently REJECTED by M6's teaching gate, wants a real coercion rather than rejection.
    M6 Phase 1d shipped a REJECTION gate (clean teaching errors) for facet 3's arg / construction /
    statement slots, but facets 1 & 2 still ICE. **This stub's coercion fix should REPLACE rejection
    with actual int→number coercion across ALL THREE facets as ONE mechanism** — fix-shape candidate 2
    above (typeck-level coercion materializing the decimal128 representation at the retype point)
    naturally subsumes all three, and the call-site audit this stub already calls for should enumerate
    all three facets' slots. (WHICH fix-shape candidate to adopt stays this stub's own graduation-pass
    decision — this bullet records only the widened scope, not the fix choice.)
  - **Both v0.3-M6 (`2026-07-04-v0-3-m6-concurrency-hotfix`) and v0.3-M7
    (`2026-07-04-v0-3-m7-optimizer-pipeline`) explicitly declined to absorb this fix as out-of-charter**
    — see each plan's own Future Requirements section. It needs its own small hotfix slot.
  - Pre-existing legacy codegen — orthogonal to v0.3-M5's array/SoA charter (M5 Phase 2 only added
    Shape/Maybe arms to `store_field`; the `Type::Number` arm is untouched legacy code).
  - No fixture or example anywhere in the repo currently exercises the bare-int-literal-into-`number`
    pattern (every existing usage is a decimal literal, e.g. `1234567.89`) — several pre-existing
    `docs/reference/REF-collections.md`-adjacent examples would already ICE if run today with a bare
    int literal.
  - Full 4-field deferral text lives in the roadmap's own `audit.md` (Idempotency-Key
    `2026-07-03-v0-3-m5-auto-soa#7: crates-ynz-codegen-src-emit-rs-14101`) and in roadmap Capability
    Ledger row 441 (both duplicate tables).

- **Likely phases (rough):** TBD. Rough shape likely: (1) root-cause confirmation + decide between
  the two fix-shape candidates above (possibly its own small spike, given the two candidates trade
  off codegen-side vs. typeck-side complexity and each needs a call-site audit); (2) implement the
  chosen fix; (3) author the RED fixture(s) that would have caught this (bare int literal into a
  `number` variable, a `number` shape field, a `number` array element) — both a codegen unit test and
  a `primantis-orders/` gallery / `pirates-roster/` demo addition per
  [`.claude/rules/plan-invariants.md`](../../../rules/plan-invariants.md) `### Demo & Error Gallery`;
  (4) full-suite regression + roadmap reconciliation (retire or narrow row 441).

- **Earliest move / open questions:**
  - Which fix-shape candidate (codegen-side `IntLit` branch vs. typeck-side coercion materialization)
    is correct is NOT yet decided — this is the first real design question a graduation pass must
    resolve, likely via a short call-site audit of every `Type::Number` codegen arm and every
    int→number coercion site in typeck.
  - Sequencing is TBD — this hotfix is independent of M6/M7/M8 (no shared files, no ordering
    dependency either direction) and can run anytime; Patrick has not yet assigned when/where it
    slots relative to the three sibling concurrency plans.
  - Everything else (exact phase count, risk table, invariants, reviewer fan-out) is TBD pending
    graduation to a full OPORD — this stub intentionally stays mostly-TBD per the WARNO-stub
    convention ([`REF-plan-format.md`](../../../../../.claude/docs/reference/REF-plan-format.md) —
    same relative-link form the sibling M6/M7/M8 plans use; a systemic global-vs-project-link
    unreachability gap the M7 amendment pass already recorded as out of scope for these plans, not
    re-solved here).
