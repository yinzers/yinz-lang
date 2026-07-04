---
name: "v0-3-m5-auto-soa"
plan-id: "2026-07-03-v0-3-m5-auto-soa"
status: "active"
roadmap-id: "2026-05-21-v0-3-concurrency-perf"
session-id: ["plan-producer-2026-07-03-m5", "plan-conductor-2026-07-03-m5-approval", "plan-conductor-2026-07-03-m5-p0-gate-exception", "phase0-executor-2026-07-03-m5", "phase0-executor-2026-07-03-m5-seg2", "phase0-fix-executor-2026-07-03-m5", "phase1-executor-2026-07-03-m5", "phase2-executor-2026-07-03-m5", "phase2-executor-2026-07-03-m5-seg2", "phase2-fixloop-executor-2026-07-03-m5", "phase2-fixround2-executor-2026-07-03-m5", "phase2-fixround2-executor-2026-07-03-m5-seg2", "phase2-fixround3-executor-2026-07-03-m5", "phase2-fixround3-executor-2026-07-03-m5-seg2", "phase2-fixround4-executor-2026-07-03-m5", "phase3-executor-2026-07-03-m5", "phase3-executor-2026-07-03-m5-seg2", "phase3-executor-2026-07-03-m5-seg3", "phase3-executor-2026-07-03-m5-seg4", "phase3-executor-2026-07-03-m5-seg5", "phase3-executor-2026-07-03-m5-seg6", "phase3-executor-2026-07-03-m5-seg7", "phase3-executor-2026-07-03-m5-seg8", "phase3-executor-2026-07-03-m5-seg9", "phase3-executor-2026-07-03-m5-seg10", "phase4-executor-2026-07-03-m5", "phase4-executor-2026-07-03-m5-seg2", "phase4-executor-2026-07-03-m5-seg3", "phase4-executor-2026-07-03-m5-seg4", "plan-fixup-frago013-2026-07-03-m5", "phase4-executor-2026-07-03-m5-closing", "phase4-executor-2026-07-03-m5-closing2", "phase4-deferral-executor-2026-07-03-m5", "phase5-executor-2026-07-03-m5", "phase5-executor-2026-07-03-m5-seg2", "phase5-executor-2026-07-03-m5-seg3", "phase5-executor-2026-07-03-m5-seg4", "phase5-executor-2026-07-03-m5-seg5", "phase5-executor-2026-07-03-m5-closing"]
created_at: "2026-07-03"
updated_at: "2026-07-03"
metadata:
  type: "plan"
---

# PLAN: v0.3-M5 — Auto-SoA (Struct-of-Arrays) + Array-By-Value Element Storage

> **Status note (pre-approval convention).** This is a complete OPORD body under `status: "stub"` —
> the execution conductor flips `stub → active` at the approval gate (same file, same plan-id).
> **Execution gate: no phase starts until v0.3.0 ships** (M4 P5/P6 complete, tag cut). Plan now,
> execute later — recorded in ¶3.4 and Weather.
>
> **Fold decision (Patrick, 2026-07-03, at grilling).** The previously-unscheduled
> `v0-3-m3c-array-by-value` work
> ([`SCRATCH-future-array-by-value-element-storage.md`](../../../../docs/internal/scratchpad/SCRATCH-future-array-by-value-element-storage.md),
> Patrick-approved 2026-06-04, never planned) **FOLDS INTO M5**. One representation redesign owns
> both: by-value inline element storage ships first (Phases 2–3), then SoA rides the new contiguous
> storage as a layout variant (Phases 4–5). Phase 1 records the fold in the roadmap + scratch docs —
> that edit is risk E1's mitigation proof artifact.

## 1. Situation

### Terrain (landscape) — recon 2026-07-03, file:line verified against the live working tree

- **`YnzArray` is a uniform-8-byte-slot pointer array** — `{data: *mut u8, len: i64, cap: i64}`
  (`crates/ynz-runtime/src/lib.rs:1100-1105`, design comment 1094-1098; `ynz_array_get/set/push` at
  lib.rs:1112-1219). Shape elements are stored as *pointers*: module global via
  `try_build_shape_global` (`crates/ynz-codegen/src/emit.rs:18126`) for literal-field shapes, or
  stack-alloca/heap ptr→int for runtime-field shapes. **No contiguous field storage exists** — both
  SoA and by-value storage need a new representation + ABI (`elem_size` param, ~20+ `emit.rs` call
  sites per the scratch doc's blast-radius analysis).
- **`Expr::ArrayLit` lowering:** `emit.rs:13025-13143`; padded shapes already skip the const-global
  fold at `emit.rs:13104-13107` — existing precedent for layout-transform-aware codegen branches.
  **RETIRED (FRAGO 013):** the `emit.rs:13104-13107` const-global-fold cite is obsolete — Phases
  2-3's by-value cut removed that fold path; do not re-anchor to it.
- **Shape field layout:** `crates/ynz-codegen/src/shape_types.rs:57` (`emit_shape_types`), `:90`
  (`pad_field_to_cache_line`).
- **M4 P4 substrate (uncommitted at plan time, complete in the working tree):**
  `crates/ynz-typeck/src/false_sharing.rs` — `finalize_false_sharing` (lines 103-170) partitions
  padded-vs-declined-with-lint, gated at entry by `kernel_mode || no_auto_parallel_env() || empty`
  (line 117). This is THE structural template for M5's SoA candidate/declined partition. Consumers
  thread one result (`TypedModule::cross_thread_padded_shapes`) — **THREE real reads, re-verified
  and re-threaded to the `LayoutDecisions` authority during Phase 4 (FRAGO 013):** `emit.rs:1057`
  (now `emit.rs:1064` post-rethread), the deep `Cg`-level struct-lit alignment read (missed by the
  original recon; found at `emit.rs:16848`, now `emit.rs:16872` post-rethread), and
  `codegen/queries.rs:208-212` (the `frame_layouts_query` sizing pass) — the
  [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) pattern SoA must mirror.
- **Lint mechanism ready as-is:** `crates/ynz-typeck/src/lints.rs` (`lint_diagnostic`, 47 lines) +
  [`registry/features.toml`](../../../../registry/features.toml):2272-2313 (`[[lint_rule]]` schema).
  Adding `array-using-soa-layout` = one TOML entry + one firing site, zero mechanism edits (M4 plan
  lines 1092-1095 confirm M5-generality was a locked M4 design goal).
- **`--no-auto-parallel` is a single predicate** — `no_auto_parallel_env()`
  (`crates/ynz-typeck/src/queries.rs:597-598`, env `YNZ_NO_AUTO_PARALLEL`, set by driver
  `main.rs:219-225`). It already gates one layout transform (M4 padding). SoA is gate #2, identical
  idiom — thread the one predicate, never a second derivation.
- **Salsa query precedents** for the new `soa_candidate_query`: `frame_layouts_query`
  (`codegen/queries.rs:81`), `codegen_query` (`:299`).
- **NO per-field access-counting analysis exists.** `independence.rs` (statement-level) and
  `effective_ownership.rs:111-565` (whole-value Reads/Writes/Unknown) are the nearest analogues;
  per-field-in-loop-body counting is net-new — the novel core (risk E5, gated by the P0 spike).
- **`lend self` method discovery:** standalone functions with `lend self: ShapeName` first param
  (UFCS, [`non-oop.md`](../../../rules/non-oop.md)); scan mirrors `finalize_false_sharing`'s
  `module.items.iter().find_map` (false_sharing.rs:131-134). Live example:
  `examples/pirates-roster/entrypoint.ynz:340-342` (`recordHit(lend self: Pirate, ...)`).
- **NO benchmark harness anywhere** (zero `criterion`/`[[bench]]` in any Cargo.toml). CI = shared GH
  `ubuntu-latest` (`.github/workflows/ci.yml`, Linux-only, golden snapshots x86_64-linux). Local dev
  = Docker `dev` service (docker-compose.yml, bind mount, no resource pinning). SIZE_THRESHOLD
  calibration has NO stable perf environment — the harness must be built and the noise question
  answered honestly (risk E2).
- **NO debug-info/DWARF code anywhere** (grep-confirmed) — grounds the DAP deferral (E4).
- **NO qualifying SoA-candidate fixture exists today:** pirates-roster has no large `array<Shape>`
  hot loop; the only multi-field shape with a lend-self method is `Pirate`
  (entrypoint.ynz:331-342). Phase 8 AUTHORS the first SoA-shaped example from scratch, and the
  suppression-filter enumeration mandate (roadmap risk row, roadmap.md:156) needs Phase 4's authored
  fixtures before it can be exercised.
- **FFI absent (v2+, registry entry exists)** — the no-FFI-export suppression criterion is vacuously
  true today; kept as a documented vacuous check (recorded decision D7).
- **Interim guard to lift:** `ArrayShapeRuntimeFieldWithWait`
  ([`IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md):588-598) —
  the M3a loud-reject that masks the stack-dangling miscompile this plan's by-value phases fix for
  real.

### Weather (external constraints)

- **Execution gated on the v0.3.0 release** — no phase starts until M4 P5/P6 complete and the
  `v0.3.0` tag is cut. No other deadline; budget phases against the work, not a date.
- **Solo project, pre-v1.0** — breaking ABI latitude per ADR-versioning; no external users
  (civil considerations: N/A — compiler-internal).
- **Recon-vs-execution drift is a standing hazard:** ¶1 file:line cites were verified against a
  working tree that includes *uncommitted* M4 P4 code. By execution time v0.3.0 has shipped and
  lines will have moved. Every phase re-verifies its cites at dispatch (CCIR-1, ¶3.4).
- **Benchmark environment is uncontrolled shared hardware** (Docker on WSL2 host / shared CI
  runners). E2's honesty posture: record variance, never claim precision the environment can't
  support.
- **Ships as a v0.3.x patch-line tag** post-v0.3.0 (Patrick, per roadmap §M5 "Ships via").

### Friendly forces

- **Higher intent:** roadmap
  [`2026-05-21-v0-3-concurrency-perf`](../2026-05-21-v0-3-concurrency-perf/roadmap.md) §Milestone 5
  (roadmap.md:341-356) + the Auto-SoA decision criteria (roadmap.md:109). M5 is the last capability
  of the original v0.3 vision.
- **M4 (`2026-07-02-v0-3-m4-channels-arc-release`)** ships the `[[lint_rule]]` mechanism, the
  false-sharing padding transform (the other layout transform), and the v0.3.0 tag M5 waits on.
- **M3a's interim guard** (`ArrayShapeRuntimeFieldWithWait`) is the loud-reject this milestone
  retires; M3d/M3g's adversarial-gate pattern (`*_declines.ynz`, DECLINE→FIRE flips, alloc=free via
  `YNZ_ALLOC_COUNTER_OUTPUT`) is the testing house style Phases 2–5 reuse.

### Assumptions

| # | Assumption | Status |
|---|---|---|
| A1 | M4 P4 substrate (`false_sharing.rs`, `lints.rs`, `[[lint_rule]]` schema) ships in v0.3.0 essentially as recon'd | **unverified** — uncommitted at plan time; re-verify at Phase 4 dispatch (CCIR-1) |
| A2 | `[[lint_rule]]` mechanism generality: one TOML entry + one firing site, zero mechanism edits | verified (features.toml:2272-2313; M4 plan 1092-1095) |
| A3 | `no_auto_parallel_env()` is the single sequential-mode predicate | verified (queries.rs:597-598; driver main.rs:219-225) |
| A4 | Zero DWARF/debug-info substrate exists | verified (grep, 2026-07-03) |
| A5 | No serializer exists until v0.8 (serialization risk is vacuous today) | verified (recon; stdlib-design Rule 6 is design-time-only) |
| A6 | Docker `dev` service can build + run compiler output for benchmarking | verified (docker-compose.yml; house workflow) |
| A7 | v0.3.0 released before any M5 phase executes | **unverified** — future event; enforced as the ¶3.4 execution gate, not assumed |

### Risk Assessment

Scored via the global `REF-risk-engine.md` (the frozen plan-system risk engine; 4×5 fixed lookup;
default code-domain anchor sheet — no project `risk-anchors.md` override exists).
Every mitigation names its bucket and proof obligation; no proof → step 0. No Floor B class fires
(no money/PII/security/irreversible-op — pre-1.0 compiler, everything git-reversible). **No HIGH
residual remains; no override block is required.** All MEDIUMs are recorded here and parked with
triggers in Future Requirements.

| Risk | Prob | Sev | Initial | Mitigations (bucket) | Residual | Gate |
|------|------|-----|---------|----------------------|----------|------|
| **E1 — twin-substrate collision** (m3c-array-by-value vs M5 building two parallel array representations that drift) — *Phases 1–3* | B | II | HIGH | Fold-in decision: ONE representation owns both; the second substrate cannot exist (**B1 eliminate**, prob −2; proof: §3.1 representation architecture + Phase 1 roadmap/scratch-doc diffs) | **MEDIUM** (D×II) | recorded |
| **E2 — SIZE_THRESHOLD false confidence** (no reliable bench env; a constant shipped with unearned precision) — *Phases 0, 6* | B | III | MEDIUM | Controlled local-Docker harness + recorded variance + documented revisit trigger (**B2 engineered guard**, prob −1; proof: committed variance/provenance record shipped WITH the constant, Phase 6 step 3) | **MEDIUM** (C×III) | recorded |
| **E3 — SoA × padding layout collision** (two layout transforms on one shape; the twin-derivation class that shipped silent miscompiles 4× — M3a/M3d/M3e/M3g per [`authoritative-derivation.md`](../../../rules/authoritative-derivation.md)) — *Phases 4–5* | A | II | EX-HIGH | (1) ONE authoritative layout-decision source both transforms read — a single query/struct resolving padding+SoA per shape, consumed by every codegen path, never re-derived (**B1 eliminate**, prob −2; proof: Phase 4 `layout_decisions` artifact + consumer threading, grep gate: no second derivation). (2) RED both-candidate fixture (shape simultaneously cross-thread-padded AND SoA-candidate) with byte-layout assertions gating the build (**B2 adversarial/RED test**, prob −1; proof: committed fixture, Phase 4 step 3) | **MEDIUM** (D×II) | recorded |
| **E4 — DAP debugger view under SoA layout** — *deferred* | D | IV | LOW | Deferred via `[[deferred_tooling_feature]]` — four-field deferral Patrick-signed 2026-07-03 (zero DWARF substrate exists, A4; a bespoke non-DWARF hack is duct tape). Parked in Future Requirements #1 | **LOW** | pass |
| **E5 — novel per-field analysis infeasible/mis-scoped** (nothing like it exists in the compiler) — *Phases 0, 4* | B | III | MEDIUM | P0 spike S2 with hard accept/reject exit gate, validated against the REAL compiler on real `.ynz` fixtures — never a hand-written model (the M2-HALT lesson) (**B2 adversarial gate**, prob −1; proof: S2 verdict + persisted spike fixtures) | **MEDIUM** (C×III) | recorded |
| **E6 — suppression filter wrong** (a `lend self` shape slips through → SoA splits what the method expects contiguous → silent miscompile; roadmap risk row roadmap.md:156) — *Phases 4, 5, 8* | C | II | HIGH | Escape-decline admission (D4) + shape-level lend-self filter + authored adversarial suppression fixtures + dual-mode byte-identical oracle, all build-blocking (**B2 adversarial/RED tests**, prob −1 — one pattern, no self-stacking; proof: Phase 4 fixture set + Phase 8 enumeration record) | **MEDIUM** (D×II) | recorded |
| **E7 — missed `elem_size` call site → silent miscompile** (the M3a-P1/P3 whack-a-mole class; direct prior evidence) — *Phases 2–3* | A | II | EX-HIGH | (1) Hard-cut ABI: old uniform-slot entry points DELETED, no parallel old path — a missed site cannot compile (Rust signature mismatch / LLVM IR verifier), plus one elem-size choke-point helper pair all sites route through (**B1 eliminate**, prob −2; proof: grep gate — zero old-signature symbols, zero raw `ynz_array_get/set/push` calls outside the helpers). (2) Per-type × per-operation × suspension adversarial fixture matrix gating the build (**B2**, prob −1; proof: committed matrix, Phase 2 step 1 / Phase 3 step 5) | **MEDIUM** (D×II) | recorded |
| **E8 — inline-element leak class** (shape with heap-owned fields stored by value; `ynz_array_drop` is element-blind — scratch doc risk 3) — *Phase 3* | C | III | MEDIUM | alloc=free parity gate via `YNZ_ALLOC_COUNTER_OUTPUT` vs the pre-migration baseline captured in P0, **with the FRAGO 005 counted-entry-point requirement**: P2's new buffer allocations MUST route through counted entry points (Phase 2 step 2) and P3's gate first proves buffer visibility — non-zero allocs vs the P0 baseline's alloc=0 blindness — before gating on parity, so the gate cannot pass vacuously (**B2 engineered guard**, prob −1; proof: committed parity test + baseline file + visibility entry criterion, Phase 3 step 4). Contingent fallback if parity REDs: loud-reject per D6 | **LOW** (D×III, earned per FRAGO 005) | pass |
| **E9 — SoA array × suspension/background/copy** (layout variant crossing `wait`, `background`, `.copy`) — *Phase 5* | B | II | HIGH | (1) The heap-owned by-value buffer substrate eliminates the stack-dangling class for SoA identically to AoS — SoA rides the same stable storage (**B1 eliminate**, prob −2; proof: Phase 3 crossing-wait fixture green on the shared substrate). (2) SoA×{`wait`, `background`, `.copy`}×dual-mode adversarial fixtures (**B2**, prob −1; proof: Phase 5 step 4 matrix) | **LOW** (E×II) | pass |
| **E10 — serialization forward-compat gap** (v0.8's compile-time serializer can't reconstruct unified values from SoA layout) — *Phases 4, 7* | C | III | MEDIUM | Layout metadata exposed as a real, tested struct (`LayoutDecision` — consumed by the Tier 3 lint hover, so it exists as exercised code, not prose) + forward-compat design note in IMP-collections (**B2**, prob −1; proof: lint reads the metadata; IMP-collections note, Phase 7 step 5). Reframe per Patrick 2026-07-03: no roundtrip test now — vacuous until v0.8 (A5) | **LOW** (D×III) | pass |
| **E11 — compile-time cost of the analysis passes >10%** (roadmap standing risk, roadmap.md:160) — *Phase 8* | C | III | MEDIUM | Release-gating profile step: wall-clock of the release-profile compiler binary running `ynz build` on pirates-roster, like-for-like vs the P0 baseline per `baselines-p0.md`'s documented methodology (FRAGO 006 — `ynz build --release` is not a CLI flag, main.rs:94-95), <10% or STOP (**B2 engineered gate**, prob −1; proof: recorded numbers, Phase 8 step 4) | **LOW** (D×III) | pass |
| **E12 — `map<K,Shape>` symmetric missed-call-site silent-miscompile** (scratch doc risk 4 — pre-existing base bug, miscompiles with OR without suspension; the SAME missed-call-site class as E7 but on `ynz_map_*`, which E7's array-only scope + grep gate do NOT cover) — *Phases 0, 3* | A | II | EX-HIGH | (1) P0 exhaustive `ynz_map_*` call-site audit + hard-cut/single-choke-point ABI same as arrays — old uniform-slot map entry points DELETED, all map element loads/stores route through one choke point; a missed site cannot compile (**B1 eliminate**, prob −2; proof: grep gate — zero old-signature `ynz_map_*` decls, zero raw `ynz_map_*` calls outside the helpers). (2) RED `map<K,Shape>` matrix fixture gating the build (**B2 adversarial/RED test**, prob −1; proof: committed matrix, Phase 3 step 1) | **MEDIUM** (D×II) | recorded |

## Design-Doc Alignment

Governing docs read at plan time; every divergence enumerated as "doc says A; plan does B because C."

**Cited governing docs:**
[`SCRATCH-future-auto-soa.md`](../../../../docs/internal/scratchpad/SCRATCH-future-auto-soa.md) ·
[`SCRATCH-future-array-by-value-element-storage.md`](../../../../docs/internal/scratchpad/SCRATCH-future-array-by-value-element-storage.md) ·
[`IMP-collections.md`](../../../../docs/internal/implementation/IMP-collections.md) (auto-promotion + array representation) ·
[`IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md) (`ArrayShapeRuntimeFieldWithWait` interim guard, :588-598; padding) ·
[`auto-promotion.md`](../../../rules/auto-promotion.md) ·
[`authoritative-derivation.md`](../../../rules/authoritative-derivation.md) ·
[roadmap §M5 + Architectural Decisions](../2026-05-21-v0-3-concurrency-perf/roadmap.md) (lines 109, 127, 341-356).

**Divergences (each Patrick-decided or recorded per decision-philosophy):**

1. **Scratch doc says** array-by-value is "its own `/plan` (`v0-3-m3c-array-by-value`)"; **this plan
   folds it into M5** — Patrick-approved 2026-07-03. One representation redesign owns both (E1
   mitigation); Phase 1 updates the scratch doc + roadmap to record the fold.
2. **Roadmap risk row (roadmap.md:159) says** "Assert: serialize + deserialize a SoA-laid-out
   `array<Player>` → roundtrip"; **this plan carries a forward-compat design note instead** — Patrick
   2026-07-03. Recon-verified: no serializer exists until v0.8 (A5); the roundtrip is untestable
   vacuity today. The SoA representation MUST expose layout metadata sufficient for v0.8's
   compile-time serializer codegen to reconstruct unified values (E10; Phase 7 step 5).
3. **Roadmap (roadmap.md:127, :353) says** DAP integration is "best-effort... defer if >3 phases";
   **this plan defers it outright** via `[[deferred_tooling_feature]]` — Patrick-signed 2026-07-03
   four-field deferral (Future Requirements #1). Zero DWARF substrate exists (A4); even one phase of
   DAP would be a bespoke non-DWARF hack (duct tape) or would hold a perf milestone hostage to
   building DWARF from scratch. Phase 1 updates the stale roadmap text.
4. **SCRATCH-future-auto-soa says** "codegen-only change" (pre-correction framing); **already
   corrected** in the roadmap 2026-07-02 — SoA needs the new array representation. This plan builds
   it (Phases 2–3 first). No new sign-off needed; recorded for completeness.
5. **[`auto-promotion.md`](../../../rules/auto-promotion.md) locks** SoA as the canonical
   codegen-only/no-typeable-form example: **Tier 3 lint YES, muted hint NO.** This plan conforms
   exactly — no muted-hint domain is added, and Phase 7 must NOT over-correct by adding one.
6. **SCRATCH-future-auto-soa open question "cross-function analysis"** — resolved conservatively for
   M5: candidate arrays are intra-function only; any escape (argument, return, store into another
   value, module boundary) DECLINES the array (recorded decision D4). Cross-function propagation is
   Future Requirements #3.
7. **Milestone-boundary assumptions:** M5 depends on M4's `[[lint_rule]]` mechanism (roadmap.md:345,
   confirmed shared infra) and on the v0.3.0 release preceding execution (roadmap §M5 trigger). Both
   deferrals/dependencies are documented in the roadmap, not invented here. The fold-in *adds* the
   previously-unscheduled by-value capability to M5 — Phase 1 gives it a Capability Ledger row so
   the ledger stays the scope-bleed SSOT.
8. **Behavior claims about untouched adjacent code, recon-cited per plan-invariants §Design-Doc
   Alignment (4):** padding consumers thread one result (recon cited emit.rs:1051 +
   codegen/queries.rs:204; Phase 4's A1/CCIR-1 re-verify found THREE real consumers — see the
   Terrain bullet + Phase 4 step 2, corrected per FRAGO 013); `no_auto_parallel_env()` single
   predicate (queries.rs:597-598); padded shapes skip the const-global fold (emit.rs:13104-13107 —
   RETIRED per FRAGO 013, obsoleted by Phases 2-3's by-value cut); guard text at
   IMP-concurrency.md:588-598. All carry the
   A1 re-verify caveat (M4 P4 was uncommitted at recon time).

### Recorded Decisions (durable calls made at plan time, per decision-philosophy — reasons on the record)

- **D1 — Kernel mode: SoA is DISABLED in `--kernel` mode**, mirroring M4 padding's gate idiom
  (false_sharing.rs:117). Reason: both are layout transforms with the same testing story
  (`--no-auto-parallel` dual-mode oracle); kernel mode has zero real programs today (the
  `KernelModeRejectsWait` template is wired in 0 code sites); divergent kernel×layout states would
  multiply the fixture matrix for no user. Revisit trigger: Future Requirements #7.
- **D2 — SoA storage = ONE allocation, segmented per field** (per-field segment offsets computed at
  compile time from cap × field sizes). Reason: Golden Rule 8 / one-allocation-per-buffer (the same
  GR8 argument that rejected per-element heap in the scratch doc's Option B); N separate field
  allocations would multiply the alloc=free accounting and fragment cache lines.
- **D3 — SoA admission requires compile-time-provable length > SIZE_THRESHOLD and NO growth ops**
  (`.add()`/push anywhere on the binding → decline). Reason: growth under segmented layout forces a
  per-segment re-layout on every realloc — real cost, zero proven demand; the provable-length rule
  keeps the "array length > threshold at the analysis-time proof point" criterion (roadmap.md:109)
  honest instead of guessing runtime sizes. Aligns with the `array<T>`→`fixed<T>` precedent: the
  proven-never-grown array is exactly the auto-promotion sweet spot. Triggers: Future Requirements
  #4, #5.
- **D4 — Escape-decline admission:** an array passed as an argument, returned, stored into another
  value, or crossing a module boundary is DECLINED (the analysis cannot see all accesses). Reason:
  M3d's decline-safely-to-baseline discipline; a partial-visibility SoA decision is the silent-wrong
  class. The shape-level lend-self filter (roadmap mandate) is retained as defense-in-depth on top —
  it becomes load-bearing when cross-function propagation (FR #3) lands.
- **D5 — ≤2-fields criterion is the UNION of fields accessed across all loops over the array** in
  the owning function; ≥3 in the union → decline. Reason: resolves the scratch doc's "mixed-access
  loops" open question conservatively; per-loop scoring with conflicting layouts is unimplementable
  (one array has one layout). Revisit: Future Requirements #9.
- **D6 — Element-drop parity, not element-aware drop:** by-value storage keeps `ynz_array_drop`
  element-blind, matching today's semantics; the P0-baselined `YNZ_ALLOC_COUNTER_OUTPUT` parity gate
  (E8) proves no NEW leak class vs the current pointer representation. If parity REDs, fall back to
  the scratch doc's loud-reject option (`array<Shape-with-heap-fields>` typeck error) via FRAGO.
  Reason: current drop is already element-blind for pointer elements; by-value is
  memory-equivalent-or-better, and inventing element-aware drop now is YAGNI until the ownership
  model's drop story (FR #6) demands it.
- **D7 — FFI suppression check kept as a documented vacuous check** (one line in the admission
  criteria naming it vacuously-true, A5-style). Reason: cheaper than deleting + re-inventing at v2+;
  the criterion is roadmap-mandated (roadmap.md:109 criterion 3).
- **D8 — Benchmark force-mode is an internal test-only env var** (`YNZ_SOA_FORCE`, harness-only,
  mirrors the `YNZ_NO_AUTO_PARALLEL` idiom) — never user-facing syntax. Reason: the scratch doc
  locks "no source-level opt-in/opt-out in v0.3"; the harness needs A/B forcing; an env var is the
  established non-syntax escape hatch.
- **D9 — Lint rule name `array-using-soa-layout` is kept as roadmap-locked** (roadmap.md:121)
  even though "soa" is an acronym of banned jargon — the *identifier* is registry-internal
  convention; all user-facing hover TEXT must be jargon-free ("stored as separate per-field arrays",
  never "struct"/"Struct-of-Arrays"). **Behavior claim, UNVERIFIED at plan time:** this decision
  assumes `jargon_audit.rs` gates user-facing diagnostic/hover TEXT but does NOT reject
  registry-internal lint-rule identifiers (so the roadmap-locked `array-using-soa-layout` name
  passes while jargon in hover text is caught). No recon cite exists for that scoping — **Phase 7
  step 1 MUST re-verify** what `jargon_audit.rs` actually scans before relying on it; if it also
  audits registry identifiers, surface it (the roadmap-locked name would then need Patrick's call —
  rename vs. audit carve-out), do not silently work around it.
- **D11 — Layout precedence: padding wins, SoA declines for cross-thread-padded shapes.** When a
  shape is simultaneously cross-thread-padded (M4) AND an SoA candidate, the ONE layout authority
  (Phase 4) resolves it to padding-only; the shape's arrays are never SoA'd. Reason: correctness
  under false sharing outranks cache-locality perf, and a cross-thread array is outside SoA's
  provable-single-thread hot-loop model anyway (D4's escape-decline would independently reject it).
  This is the human-visible resolution of E3's collision; the byte-layout-asserting both-candidate
  fixture (Phase 4 step 3) proves it. Revisit: FR #11 (any dual-mode divergence re-opens it).
- **D10 — Executor model dispatch: Fable 5** (`model: fable`) for all phase executors at
  `/execute-plan` time — Patrick 2026-07-03, an explicit availability-override of the frozen
  binding's excluded-models list (Fable returned). Reviewer fleet stays per the frozen
  model-selection binding. Recorded in ¶4/¶5; Model tags below still carry the honest
  `(task-type, quality-bar, scale)` classification for the record and for fallback.
- **D12 — `contains` on `array<Shape>` = field-wise VALUE equality** (ratified via FRAGO 008,
  P2-boundary). By-value storage has no pointer-identity substrate, so the migration FORCED a
  semantics pick; value equality is the golden-rules-consistent one (GR2 —
  `roster.contains(pirate)` reads as content membership; matches every primitive cell; matches
  the non-OOP shapes-are-data model). Implemented as per-field GEP compares (`shape_value_eq`,
  never a padded-bytes memcmp); locked by the two RED shape contract cells. **Pointer-typed
  fields (string, nested shape) compare by identity for now — deferred to Phase 3 step 5's
  E8-class review**, which must either ratify identity or extend to deep value equality.
- **D13 — Field-assign = COPY-ON-PERSIST snapshot semantics** (ratified via FRAGO 011, P2 fix
  rounds 2–3). Storing a shape (or maybe) value into a persist surface — shape field, map value,
  array element, background spawn descriptor — SNAPSHOTS the value's bytes at the assignment
  (counted heap cell / runtime memcpy), where the pre-M5 pointer representation ALIASED (a later
  mutation of the source stayed visible through the stored reference). A user-visible semantics
  call, FORCED by the by-value representation (there is no stable pointer to alias), and
  consistent with D12's value-semantics direction (shapes are data, not identities). Docs home:
  Phase 7 step 5 alongside D12 — MUST carry the teaching note that TypeScript developers expect
  aliasing here (objects are references in TS/JS), so the snapshot-at-assign behavior needs an
  explicit HS-grad-readable spec callout, not a silent divergence.

## 2. Mission

After v0.3.0 ships, the M5 execution delivers **by-value inline element storage for `array<Shape>`
(new elem_size-aware YnzArray ABI, permanently fixing the array-element stack-dangling miscompile
class the M3a guard only masks) and, riding that contiguous storage, automatic Struct-of-Arrays
layout for large `array<Shape>` hot loops** — zero syntax change, byte-identical program output in
both scheduling modes — **because** Yinz's efficiency-first positioning (Golden Rules 4/8/10)
promises the 10-40× cache-locality win to naive sequential-looking code, and one representation must
own both changes so the twin-substrate drift class can never ship.

## 3. Execution

### 3.1 Intent & End State

**Purpose.** Two structurally-coupled outcomes, in dependency order: (1) `array<Shape>` elements
become **owned, by-value, inline** in the heap buffer — the array survives suspension by
construction, the `ArrayShapeRuntimeFieldWithWait` guard and `try_build_shape_global` special-cases
are deleted, and `map<K,Shape>` gets the symmetric fix; (2) on that contiguous storage, the compiler
**automatically picks SoA layout** for provably-safe large hot-loop arrays, with the full teaching
surface (Tier 3 lint, registry, hover, VSCode) shipping in the same milestone per roadmap
constraint 71.

**Representation architecture (the E1 anchor).** There is exactly ONE array representation: the
elem_size-aware by-value `YnzArray`. SoA is a **layout variant** of that representation — same
header, same ownership, same drop path, segment addressing computed by codegen from the ONE
authoritative `layout_decisions` source (E3). No parallel array runtime, no second layout
derivation, anywhere. An executor who finds itself writing a second representation or re-deriving a
layout answer must STOP and surface it (CCIR-2).

**Key outcomes (definition of done):**

1. A runtime-field `array<Shape>` crossing a `wait` compiles and prints correct values (the scratch
   doc's acceptance signal); the interim guard + its registry deferral are gone; old galleries
   updated.
2. Every fixture in `examples/` + `crates/ynz-codegen/tests/` + `crates/ynz-driver/tests/` is
   **byte-identical** across default and `--no-auto-parallel` modes (which disables SoA — gate #2 on
   the one predicate).
3. A qualifying large-array hot loop in `pirates-roster` gets SoA automatically, the
   `array-using-soa-layout` Tier 3 lint fires on its declaration with jargon-free
   WHAT/WHAT-INSTEAD/WHY hover, and the measured hot-loop improvement is recorded with benchmark
   evidence.
4. SIZE_THRESHOLD ships with committed provenance: workload, machine, variance, date, and a revisit
   trigger — never a bare constant (E2's honesty posture: the hazard is false confidence, not the
   number).
5. A shape that is simultaneously cross-thread-padded AND an SoA candidate resolves through the one
   layout authority (padding wins, SoA declines — recorded decision D11), with a byte-layout-
   asserting fixture proving it.
6. The roadmap + scratch docs record the fold (E1 proof); IMP-collections carries the graduated
   design content including the serialization forward-compat note; a v0.3.x tag ships it.

**Disciplined initiative.** When steps and reality diverge: correctness of program output
(byte-identical dual-mode) outranks every perf claim; declining an array to AoS is ALWAYS safe —
when in doubt, decline and record why (the M3d discipline). Never invent a second derivation of any
layout/analysis answer to unblock yourself — thread the authoritative one or surface the blocker.
A spike verdict of RED is a full STOP, not a "note and proceed."

### 3.2 Concept

Nine phases, three regimes. **Gate first** (P0 spikes prove the two novel mechanisms + capture
baselines; P1 records the fold). **By-value substrate** (P2 the hard-cut ABI migration; P3 guard
lift + symmetric surfaces) — must be fully green before any SoA work. **SoA on top** (P4 analysis +
the one layout authority; P5 codegen; P6 threshold calibration; P7 teaching/docs; P8
demo/enumeration/release). Handoffs: each phase ends green-tree with its fixtures committed; fat
phases (P2, P5) checkpoint per the marks below.

### 3.3 Phases

#### Phase 0 — Feasibility spikes + call-site audit + baselines (HARD GATES)

> **STATUS: COMPLETE (2026-07-03).** S1 **GREEN** (`spike-notes/s1_verdict.md`, segment 1,
> session `phase0-executor-2026-07-03-m5`) · S2 **GREEN** (`spike-notes/s2_verdict.md` + 4
> fixtures, segment 2, session `phase0-executor-2026-07-03-m5-seg2`) · S3 variance note
> `spike-notes/s3_bench_noise.md` (+ rerunnable `s3_bench.rs`) · audits
> `audit-array-callsites.md` (P2) + `audit-map-callsites.md` (P3/E12) · baselines
> `baselines-p0.md` (E8 alloc=free — **with a counter-scope caveat P2/P3 must address**;
> E11 ≈210 ms mean). All spike scaffolding torn down; source tree pristine.

- **Task + purpose:** prove the two net-new load-bearing mechanisms (by-value ABI; per-field
  analysis) on throwaway spikes with explicit STOP conditions before anything durable is built
  (E5, E7 de-risking); capture the pre-M5 baselines later phases diff against.
- **Steps**
  1. **S1 — by-value ABI spike:** throwaway branch; minimal elem_size-aware `YnzArray`
     (`new(elem_size)` / `push(*const u8)` / `get(idx, out) -> has-flag` / `set(idx, *const u8)`) +
     one hand-lowered fixture through the REAL compiler (`docker compose run --rm dev cargo build -p
     ynz-driver` then `./target/debug/ynz run …`): a 3-element `array<Shape{int,float}>` with
     **runtime** field values — construct, `set`, `get`/index, correct stdout. Verdict S1
     GREEN/RED. **STOP: RED → return BLOCKED to the conductor for representation re-design; P2 does
     not start.**

     **CHECKPOINT** — S1 verdict recorded; scaffolding torn down EXCEPT the fixture `.ynz` + verdict
     note (persisted into this plan dir — P2 step 1 consumes them; spike-teardown evidence rule).
  2. **S2 — per-field access analysis spike:** prototype per-field-in-loop-body counting over the
     typed AST (analogues: `independence.rs`, `effective_ownership.rs:111-565`), run against ≥4
     real `.ynz` fixtures through the real pipeline — qualifying 2-field hot loop; 3-field loop;
     escaping array; runtime-length array. Never a hand-written model (M2-HALT lesson). Verdict S2.
     **STOP: RED → BLOCKED (the milestone's novel core is infeasible as scoped).**

     **CHECKPOINT** — S2 verdict + fixtures persisted (P4 step 1 consumes them).
  3. **S3 — bench noise probe:** raw Rust microbench (AoS scan vs contiguous scan), ≥10 repetitions
     inside the Docker `dev` container; record run-to-run variance to a committed note in this plan
     dir. This sets E2's credibility bar for Phase 6 (a threshold delta smaller than the noise floor
     is not evidence).
  4. **Exhaustive call-site audit (E7 + E12 precondition):** enumerate EVERY `ynz_array_*`
     construction/read/write surface — `emit.rs` (ArrayLit :13025-13143, for-loop SM + non-SM paths,
     `lower_array_method` get/first/last/contains/add/set, `IndexAccess`), `runtime_decls.rs`,
     `lib.rs` (`ynz_string_split`) — into a committed checklist P2's exit criteria tick off. **In the
     SAME pass, enumerate every `ynz_map_*` construction/read/write surface** (map literal lowering,
     `lower_map_method`, index/get/set/contains, `runtime_decls.rs` map decls) into a SEPARATE
     committed checklist that **Phase 3 step 1's entry criteria** tick off — E12's audit precondition,
     symmetric to the array audit but feeding the map fix, not P2.
  5. **Baselines:** capture + commit (a) `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free numbers for the array
     fixture suite (E8 baseline), (b) `ynz build --release` wall-clock on pirates-roster (E11
     baseline). Persist to files, not chat (evidence-durability).
- **Exit criteria:** S1 + S2 GREEN verdicts recorded; S3 variance note, BOTH audit checklists
  (`ynz_array_*` for P2, `ynz_map_*` for P3), and both baselines committed. Any RED → plan STOPPED
  at conductor.
- **Reviewer fan-out:** adversarial gate-checker on the spike verdicts (are S1/S2 actually proven
  through the real compiler, not narrated?); design-doc-alignment reviewer on the S1 ABI shape vs
  the scratch doc's Option A.
- **Model tag:** `(coding, high, medium)`

#### Phase 1 — Record the fold (roadmap + scratch cross-references) — E1 proof artifact

> **STATUS: COMPLETE (2026-07-03, session `phase1-executor-2026-07-03-m5`).** All three steps done +
> the FRAGO 006-addendum straggler (¶1 E11 mitigation cell) applied. **Deviation surfaced (not
> self-classified):** the worktree's committed roadmap.md predates the sibling M4 session's
> UNCOMMITTED 2026-07-02 M4/M5-split edits in the main repo — the plan's cited anchors (§Milestone 5
> at roadmap.md:341-356, ledger rows, :109/:127 bullets) did not exist in this checkout. Resolution:
> the split-affected regions this phase edits were imported VERBATIM from main's working copy first,
> then the Phase-1 amendments applied on top; untouched regions stay at the fork-commit base so the
> later merge auto-resolves them. Full import/amendment inventory in the phase return + audit.md.

- **Task + purpose:** make the fold-in decision durable in the SSOT docs so no future session
  re-derives a standalone `v0-3-m3c-array-by-value` plan or a second representation.
- **Steps**
  1. [`roadmap.md`](../2026-05-21-v0-3-concurrency-perf/roadmap.md): §Milestone 5 scope text (fold
     array-by-value in; serialization reframe per Divergence 2; DAP outright-deferral per
     Divergence 3 — also fix the stale "best-effort >3 phases" text at roadmap.md:127); the
     Architectural-Decisions Auto-SoA bullet (roadmap.md:109) gets a pointer to this plan-id;
     **roadmap.md:121** (the Feature-Registry-Entries mandate) still assigns `array-using-soa-layout`
     to M4 ("M4 MUST … add Tier 3 lint rules `array-using-soa-layout` and …") — REASSIGN the SoA
     lint to M5 (features.toml confirms M4 shipped only `cross-thread-fields-not-padded` +
     `prefer-yielding-sleep`); **BOTH Capability Ledger tables** (roadmap.md:364-377 and :409-421): add a row "Array by-value
     element storage + `map<K,Shape>` symmetric fix — owning milestone M5" (previously unscheduled)
     and amend the M5 row's notes with the fold.
  2. [`SCRATCH-future-array-by-value-element-storage.md`](../../../../docs/internal/scratchpad/SCRATCH-future-array-by-value-element-storage.md):
     status → "FOLDED INTO v0.3-M5 (Patrick 2026-07-03)", pointer to this plan-id.
  3. [`SCRATCH-future-auto-soa.md`](../../../../docs/internal/scratchpad/SCRATCH-future-auto-soa.md):
     owning-plan pointer note (full graduation/trim happens in Phase 7 step 5).
- **Exit criteria:** greppable cross-refs land; no doc still claims a standalone m3c plan is pending
  or that DAP is in-milestone; `_index.md` regenerated by the lifecycle hook (never hand-edited).
- **Reviewer fan-out:** docs-consistency reviewer (diff every edited claim against this plan's
  Design-Doc Alignment list).
- **Model tag:** `(general/mechanical, floor, small)`

#### Phase 2 — By-value element storage: hard-cut ABI + full codegen migration

> **STATUS: COMPLETE (2026-07-03).** Segment 1 (session `phase2-executor-2026-07-03-m5`): step 0
> (FRAGO 007 plan edit) + step 1 (11-fixture RED matrix, 7 green / 4 RED on the contract cells) —
> checkpointed PARTIAL at the step-1→step-2 boundary. Segment 2 (session
> `phase2-executor-2026-07-03-m5-seg2`): steps 2–5 — the atomic cut landed (elem_size-aware
> `YnzArray`, counted alloc + counted growth per FRAGO 005, hard-cut signatures, the
> `Cg::array_elem_*` choke-point section reading the ONE `shape_abi_sizes` source,
> `try_build_shape_global` + the SM shape-embed special-case deleted, shape `contains` =
> field-wise value equality); matrix 11/11 green (all 4 RED contract cells flipped); workspace +
> full suite green (481/481 integration); E7 grep gates PASS; `audit-array-callsites.md` 100%
> ticked with per-line dispositions; snapshot churn verified decl-signature-only; dual-mode
> oracle clean (340 byte-identical, 0 real divergences — the 2 flagged fixtures are the
> documented Model-A intended-reorder exclusion class, both array-free). **E8 first-look:** the
> counter now SEES buffers (P0's alloc=0 blindness → non-zero on every array fixture, +2 counted
> allocs per array); clone→drop pairs balance exactly; the residual alloc-without-free is the
> PRE-EXISTING never-drop-local-arrays design made visible — surfaced as a deviation, Phase 3
> step 4's parity gate owns the verdict (see audit.md + the segment-2 return).
> **Boundary fix-loop (session `phase2-fixloop-executor-2026-07-03-m5`):** code-reviewer BLOCKER
> fixed — get-side out-buffer aliasing (escaping elements read the LAST iteration's bytes;
> RED-proven differentially) via the binding-point ownership funnel (emit.rs `store_binding` +
> `shape_bytes_to_owned` / `maybe_to_owned` / `shape_bytes_into_embed_slot`; S1 entry-block
> staging preserved); probing found + fixed a second instance (assign to a frame-embedded
> crossing shape local clobbered its frame wiring — 0/0 for 2/20). 5 RED→GREEN escape tripwire
> cells + debug-repr fixture added (suite 481→487); FRAGO 008/009 plan-body edits applied (D12,
> Phase 7 step 5 docs home, Phase 3 steps 3–5 re-specifications); dual-mode oracle re-run clean
> post-fix — durable tallies in `p2-dualmode-report.md` (349 identical / 84 skip / 0 real
> divergences; 2 documented intended-divergence exclusions, both array-free; 2 timing-nondet).
> **Fix round 2 COMPLETE (sessions `phase2-fixround2-executor-2026-07-03-m5` seg 1 +
> `…-seg2`):** both code-reviewer BLOCKERS fixed at the PERSIST boundary via counted heap cells —
> `store_field` Shape/Maybe arms (field-assign + struct-lit fields + hidden defaults) +
> `map_value_to_stable_bits` choke point at ALL FOUR map insert sites; 8 tripwires RED-proven
> (3/30 unfixed) → GREEN (suite 487→495, all green); clippy/fmt clean; E7 grep gates re-pass
> (raw `ynz_array_*` only in the choke-point section; map values marshal ONLY through
> `map_value_to_stable_bits`; `try_build_shape_global` absent); FRAGO 010 paperwork ×3 landed
> (P3 step 5 item (d) size-derivation twin; `shape_frame_slots` doc corrected; stale fixture WHY
> fixed); ownership-contract comment now enumerates the full persist boundary; dual-mode oracle
> re-run clean post-round-2 (445 fixtures: 357 identical / 84 skip / 0 real divergences — same
> 2 documented DIFFs + 2 timing-nondet; `p2-dualmode-report.md` updated). Stray out-of-scope fix
> (cheap-gates tier per the conductor's review-economy note): round-2's nested-generic fixture
> exposed `ynz-fmt` emitting un-reparseable `maybe<Part>>` (lexer has no `>>` split) — fixed via
> `close_generic` in `crates/ynz-fmt/src/walker.rs` + locked by
> `nested_generic_keeps_lexer_required_space`. Deviations D-r2-1/2/3 (segment 1) + D-r2-4 (fmt
> stray) surfaced for the deviation-judge; snapshot churn expected by the dispatch did NOT
> materialize (no snapshot-covered fixture exercises the new heap-cell path; exact-count alloc
> asserts unchanged and green — the assertion family itself is the proof).
> **Fix round 3 COMPLETE (sessions `phase2-fixround3-executor-2026-07-03-m5` seg 1 +
> `…-seg2`):** both code-reviewer BLOCKERS fixed — (B1) `array<maybe<T>>` element writes:
> `map_value_to_stable_bits` GENERALIZED to `value_to_stable_bits` (no per-surface twin) and
> `array_elem_src_ptr`'s non-shape path routed through it (`zext_bits64` extracted;
> `array_elem_bits64` re-scoped compare-only); (B2) bg spawn maybe args: `prepare_bg_arg_for_ctx`
> Maybe arm + `BgArgFreeKind::HeapMaybeEnv` (SM descriptor rides wire kind 0, runtime unchanged).
> Both RED-proven byte-exact → GREEN (suite 495→497, all green; workspace exit 0; 0 pending
> snapshots — no churn, round-2 precedent held); clippy `-D warnings` + fmt `--check` clean; grep
> gates re-pass (raw `ynz_array_{new,push,get,set}` only in the choke-point section; ALL
> maybe/shape persist marshalling through the ONE `value_to_stable_bits` at array writes + all 4
> map sites; spawn frames heap-upgraded pre-marshal; `try_build_shape_global` present only in
> historical comments). Dual-mode oracle post-round-3: 447 fixtures, 359 identical / 84 skip /
> 2 documented DIFFs / 2 timing NONDETs / 0 anomalies / **0 real divergences**
> (`p2-dualmode-report.md` regenerated). FRAGO 011 paperwork ×3 landed (P3 step 4 third
> accounting category; D13 recorded; P7 step 5 docs home extended); ownership-contract comment
> corrected (array-element writes = persist surface; sync-only aliasing args vs spawn
> descriptors); round-3 residuals routed to P3 step 5 items (e)/(f)/(g). Should-fix 2 (Union
> arm) NOT shipped — BLOCKED-class, KNOWN HOLE documented in `value_to_stable_bits`, carried as
> a deviation. NEW deviation surfaced seg-2: probe-confirmed `fixed<Shape>` element-write
> aliasing (same escape class, fixed<T> uniform-slot surface — see audit.md seg-2 log).
> **Fix round 4 COMPLETE (session `phase2-fixround4-executor-2026-07-03-m5`):** the round-3
> closer's surfaced fixed<T> deviation fixed as the SAME persist class — exhaustive i64-GEP
> census found exactly THREE fixed-slot write sites (IndexAssign, `.set()`, literal fill; all
> other fixed GEPs read-side), all three rerouted through the ONE `value_to_stable_bits` choke
> point (no fixed-specific twin; `fixed<maybe<T>>` admission probe-verified — same three sites,
> covered). 4 tripwires RED-proven byte-exact (2/20 aliasing, 3/30 maybe, 9/90 literal-fill
> mutation-bleed) → GREEN; suite 497→501 all green, 0 snapshot churn; clippy/fmt clean; grep
> gate PASS (zero bare `to_i64_bits` at any fixed-element write site); dual-mode oracle
> spot-run over the 4 new fixtures byte-identical (full re-run declined on the record — changed
> arms unreachable from every pre-existing fixture); ownership-contract +
> `value_to_stable_bits` docs extended with the fixed persist surface. NEW enumeration finding
> (surfaced, not chased): the union KNOWN HOLE extends to `fixed<UnionAlias>` (admission
> probe-verified via union-typed binding fill) — KNOWN HOLE doc extended, deviation carried.
> Fixed-crossing-wait verified typeck-guarded (no live door).

- **Task + purpose:** replace the uniform-8-byte-slot pointer storage with elem_size-aware by-value
  inline storage — the scratch doc's Option A — as ONE atomic ABI cut (E7 mitigation 1: no parallel
  old path may survive).
- **Steps**
  1. **RED fixtures first:** adopt the S1 spike fixtures; author the per-type × per-operation matrix
     (int/float/bool/string/shape × literal/runtime fields × get/set/push/index/first/last/contains
     × non-suspending) with expected-output assertions, marked expected-fail until step 3.
  2. **The atomic cut** *(heavy)*: `YnzArray` gains `elem_size: i64` (alloc `cap * elem_size`);
     `ynz_array_new(elem_size)` / `push(*const u8)` (memcpy) / `get(idx, out: *mut u8)` →
     separate has-flag (the `{i64,i64}` maybe-convention cannot carry variable width) /
     `set(idx, *const u8)`; `count`/`drop` unchanged (D6). **DELETE the old entry points** — no
     parallel ABI. **Counted allocation (FRAGO 005):** the new elem_size-aware buffer allocation
     path MUST route through counted entry points (`ynz_alloc`/`ynz_free`, or an explicit counter
     extension covering buffer mallocs) — the P0 baseline proved `YNZ_ALLOC_COUNTER_OUTPUT`
     instruments `ynz_alloc`/`ynz_free` ONLY and cannot see raw-`malloc` buffers (alloc=0 across
     the array suite, `baselines-p0.md`), so E8's parity accounting must be able to see element
     buffers or the Phase 3 gate is vacuous. Update `runtime_decls.rs`; update `ynz_string_split`. In `emit.rs`: introduce the
     single elem-size choke-point helper pair (compute via `TargetData::get_abi_size` in ONE place;
     all element loads/stores route through it — authoritative-derivation), then migrate every
     audited call site until the workspace compiles. **DELETE `try_build_shape_global`
     (emit.rs:18126) and the SM for-loop shape-embed special-case** — the buffer is now the stable
     owner; both are dead by design.
  3. Flip the step-1 matrix green; run the alloc-counter against the P0 baseline (E8 first look —
     full parity gate is Phase 3).

     **CHECKPOINT** — workspace green; matrix green; old symbols gone (grep gate: zero
     old-signature `ynz_array_*` decls; zero raw runtime-array calls outside the choke-point
     helpers; `try_build_shape_global` absent).
  4. Snapshot refresh (mechanical churn: `insta` IR/golden snapshots showing `@ynz_array_*`
     signatures + object-file SHAs) — review each diff is signature/SHA churn, not semantic drift.
  5. Tick off the P0 audit checklist line-by-line; run the cross-impl dual-mode oracle over the full
     fixture suite.
- **Exit criteria:** workspace + full test suite green; the grep gates above pass; audit checklist
  100% ticked; dual-mode byte-identical.
- **Reviewer fan-out:** code-reviewer (the migration diff); adversarial gate-checker (matrix
  coverage vs the audit checklist — every audited site has a fixture); design-doc-alignment
  reviewer (GR8: one allocation per buffer, no per-element heap).
- **Model tag:** `(coding, high, large)` — scale=large; checkpoint marks mandatory.

#### Phase 3 — By-value completion: guard lift + symmetric surfaces

> **STATUS: COMPLETE (2026-07-03).** All 5 steps done across 10 segments (sessions
> `phase3-executor-2026-07-03-m5` … `…-seg10`); sealed at the segment-10 boundary commit
> (`Plan-Phase: 2026-07-03-v0-3-m5-auto-soa#3`). **Full-phase summary:** Step 1 — the
> `map<K,Shape>` by-value ABI cut (E12): elem_size-aware `YnzMap` runtime, counted 5-buffer
> accounting, single elem-size choke point, all 8 audited call-site groups hard-cut, grep gate
> passing, audit-map-callsites.md fully ticked — and the RED matrix CAUGHT a real loop-arm
> `entry.value` miscompile (indirection-level mismatch on the MapEntry local-slot contract),
> fixed via the canonical `materialize_param` pattern in both loop arms; matrix 7/7 incl. the
> new debug-repr lock. Step 2 — `ArrayShapeRuntimeFieldWithWait` guard LIFTED: typeck Check 2d
> + helpers deleted, promotion-probe decline arm removed (decline test inverted and passing),
> registry deferral retired, IMP-concurrency.md rewritten interim-guard→LIFTED, gallery
> trigger removed. Step 3 — crossing-wait acceptance ×3 GREEN (the 3 former guard-rejection
> fixtures repurposed as `m5_p3_array_shape_*_runs`, exact stdout 30 — E9's B1 proof). Step 4
> — E8 parity gate GREEN: FRAGO 005 visibility proven fail-loud (P0-blind fixtures now 2/0 and
> 5/0), gate fixture pins EXACT alloc=11/free=0 (Paper-Trace predicted before first run);
> verdict recorded per FRAGO 009/011 — no drop insertion (YAGNI until FR #6), no D6 fallback,
> `alloc == free + gap` encoding ratified durable; map re-set-over-key leak structurally GONE
> (values inline). Step 5 — ownership sweep: FRAGO-010 size-derivation twin UNIFIED ×3 sites
> onto `shape_abi_sizes` (zero shape-size `struct_ty.size_of()` remain); bg×array<Shape>
> alias bug FIXED (spawn-site inline-elem clone, RED 119 → GREEN 30); TWO MapEntry-escape
> silent-wrongs found by probes and FIXED at the banked choke points (`value_to_stable_bits`
> MapEntry arm + `prepare_bg_arg_for_ctx` unconditional pre-gate; RED 2/20 → 1/10 and
> 20/20 → 10/20); D12 RATIFIED (pointer-identity equality for pointer-typed shape fields,
> final for M5, pinned by the discriminating runtime-string fixture) + (e) contains-on-maybe
> ratified alongside; union persist KNOWN-HOLE documented to the probe-verified truth (write
> side constructible, read-back ICEs loud, plan-text "not constructible" parenthetical
> falsified and surfaced — the loud-reject gate stays FRAGO-grade, not self-built) + loud-fail
> pins landed both sides.
> Segment 10 (session `phase3-executor-2026-07-03-m5-seg10`): the 7 step-5 sweep/pin fixtures
> + integration tests authored per the seg-9 banked designs (`m5_p3_sweep_*`), all green first
> run — incl. the SM-main give/copy/wait cell (`caller: 119 / given: 30 / copied: 30`); full
> workspace suite 2246 passed / 0 failed (reconciles: seg-6's 2236 + 3 step-4 gate tests + 7
> sweep tests); dual-mode oracle re-run over ALL 466 fixtures per p2-dualmode-report.md
> methodology — 379 identical / 83 skip / 2 documented DIFFs / 2 timing NONDETs / 0 anomalies
> / **0 real divergences**, reconciliation vs the post-round-3 run exact (report updated);
> handoff-phase-3.md deleted; phase sealed.
> Earlier: segment-9 status preserved below.
> Segment 9 (session `phase3-executor-2026-07-03-m5-seg9`): **step 5 PARTIAL — ALL remaining
> CODE + DOC work LANDED; fixtures/tests + seal remain.** MapEntry escape fix landed at both
> banked choke points: `value_to_stable_bits` `Type::MapEntry` arm (16-byte entry-struct
> clone into a counted heap cell + deep-copy of the shape/maybe VALUE half; the fixed-envelope
> size register, NOT a shape-size twin) and `prepare_bg_arg_for_ctx`'s UNCONDITIONAL MapEntry
> pre-gate (before give/copy inference; `HeapShape{16}` free; value sub-cell deferred per
> FRAGO 011). Paper-Trace RED→GREEN: bg-arg probe 2/20 → 1/10; array<MapEntry> probe
> 20/20 → 10/20, zero residual. Doc ratifications landed: D12 on `shape_value_eq`'s header,
> (e) on the contains arm, union KNOWN-HOLE refreshed to the probe-verified truth (write side
> constructible, read-back ICEs loud, gate stays FRAGO-grade — surfaced, not built). D12 pin
> probe observed `true/true/false` (3rd cell: runtime-interpolated same-text string proves
> pointer identity; 2nd cell true via LLVM literal merging — artifact, noted for the WHY).
> Build/fmt/clippy green; goldens 34/34 zero refreshes; m5_p* 44/44, v03_m3d 72/72,
> v03_m3b 87/87. Remaining (seg 10): 7 sweep/pin fixtures + integration tests (designs +
> observed pins final in the handoff — zero probing left), full suite + dual-mode oracle,
> then SEAL. Work rides uncommitted (FRAGO 004).
> Earlier: segment-8 status preserved below.
> Segment 8 (session `phase3-executor-2026-07-03-m5-seg8`): **step 5 PARTIAL — item (c)
> and the bg×array<Shape> alias fix LANDED and verified; all sweep probes run; D12
> ratified on the record.** (c): the FRAGO 010 size-derivation twin is UNIFIED at all
> three sites (`bind_sm_result_and_flush` shape arm, SM shape-embed Let arm,
> `prepare_bg_arg_for_ctx` Shape arm — incl. replacing the free-side fallback-to-0 with
> the real `shape_abi_sizes` size) onto `shape_abi_size_const`; zero shape-size
> `struct_ty.size_of()` sites remain; stale twin-flag comments rewritten; golden
> snapshots 34/34 with ZERO refreshes (predicted). bg alias fix: `prepare_bg_arg_for_ctx`
> now clones Shape-element arrays (inline bytes since the P3 cut) via the elem_size-aware
> `ynz_array_clone_primitive`; probe Paper-Trace RED 119 → GREEN 30. Probes found TWO
> live MapEntry-escape silent-wrongs (bg-arg + array<MapEntry>, fix design banked in the
> handoff) and FALSIFIED the step-5(e) "union persist not constructible" parenthetical
> (map<int,union> set + array<union> literal compile AND run; read-back ICEs loudly —
> receipts + FRAGO-candidate routing in the handoff; the loud-reject gate is surfaced,
> NOT self-built). D12 RATIFIED: pointer-identity for pointer-typed fields, final for M5
> (reasoning on record — FRAGO-candidate); (e) ratified alongside. NEW note-only finding:
> plain `.copy()` on arrays aliases (pre-existing P3c deferral, carried to reviewers).
> Remaining in step 5 (seg 9+): MapEntry escape fix + tripwires, sweep/pin fixtures
> (designs banked), (a)/(e) doc-comment ratification notes, union KNOWN-HOLE doc refresh
> + loud-fail pins, then full suite + dual-mode oracle + SEAL. Build/fmt/clippy green;
> m5_p3 13/13, v03_m3d 72/72, m3b bg 2/2. Work rides uncommitted (FRAGO 004).
> Earlier: segment-7 status preserved below.
> Segment 7 (session `phase3-executor-2026-07-03-m5-seg7`): **step 4 COMPLETE — the E8
> parity gate is GREEN.** FRAGO 005 entry criterion PROVEN, never vacuous: 2 fail-loud
> visibility tests (`m5_p3_e8_gate_visibility_{arrays,maps}`) pin the exact fixtures the
> P0 baseline recorded as alloc=0-blind — now alloc=2/free=0 (m5_array.ynz) and
> alloc=5/free=0 (m5_p3_mapshape_runtime_int.ynz). Gate proper: NEW fixture
> `m5_p3_e8_parity_gate.ynz` + test pins EXACT alloc=11/free=0 (Paper-Trace predicted
> BEFORE first run, observed matched with zero residual: 2 array buffers + 5 map buffers
> + 4 FRAGO-011 persist cells; 40-iteration read loop allocates ZERO — the per-element/
> per-iteration regression pin). FRAGO 009 semantics honored: parity GREEN ("no NEW leak
> class"). **Step-4 verdict recorded (FRAGO 009/011 assigned it here): no drop insertion
> (YAGNI until FR #6), D6 loud-reject fallback NOT taken, the interim `alloc == free +
> gap` helper encoding RATIFIED as durable accounting until FR #6's drop story** (helper
> doc updated in integration.rs). Map-side finding: the re-set-over-key cell leak
> FRAGO 011 assigned to this verdict is structurally GONE — the P3 cut stores map shape
> values INLINE (0 cells, overwrite-in-place; gate fixture proves 3 sets incl. overwrite
> = 0 cells); the deliberate accounted class remains only for shape-FIELD re-assign +
> maybe persists (exact-pinned +4 in the gate). Seg-5 fix verified alloc-pattern-neutral:
> all 72 v03_m3d tests green incl. the 7 gap-pinned (+4 array / +10 map) — unchanged.
> Housekeeping: prior segments' tree was NOT rustfmt-clean (emit.rs, integration.rs,
> lib.rs — pre-existing, none in seg-7's diff regions); `cargo fmt --all` applied
> (mechanical, no behavior change), fmt --check now exit 0, build + gate tests re-green.
> Step 5 NOT started. Work rides uncommitted (FRAGO 004).
> Earlier: segment-6 status preserved below.
> Segment 6 (session `phase3-executor-2026-07-03-m5-seg6`): **steps 2 AND 3 COMPLETE.**
> Step 2 (the guard lift, planner's CHECKPOINT mark): typeck Check 2d + its three helpers
> (`find_array_shape_runtime_field_crossing`, `find_let_initializer_in_stmts` — verified
> single-consumer before deletion, `expr_is_compile_time_literal`) DELETED from check.rs;
> promotion-probe decline arm removed (queries.rs decline test INVERTED to
> `…_host_promotes_and_compiles_clean`, passes — host now genuinely promotes);
> `array-shape-runtime-field-with-wait` registry deferral RETIRED (+ design_future_sync.rs
> SKIP entry per the M3e retire precedent; tmLanguage.json regenerated per its own sync
> test's instruction, 1-line diff); IMP-concurrency.md §ArrayShapeRuntimeFieldWithWait
> rewritten interim-guard → LIFTED; m3a gallery trigger removed (header notes the lift).
> Step 3: the 3 old guard-rejection fixtures REPURPOSED (git mv) as the crossing-wait
> acceptance matrix — `m5_p3_array_shape_{runtime_field,between_waits,nested_if}_runs`,
> each asserting exact stdout "30" (crossing local + loop var across wait; the scratch
> doc's named acceptance signal + E9 B1 proof) — ALL GREEN; array constructibility proven
> empirically (map's maybe-after-wait typeck finding does NOT transfer — these cells never
> read a maybe). Maybe-crossing obligations (get-maybe payload-across-resume + SM spawn-arg
> maybe cell) NOTED as note-only per plan text, recorded in handoff §Carry-forwards.
> Recon drift surfaced: error_galleries.rs has NO v0_3_m3a gallery test to update (only
> m4-m8/v0_3_m1/m3b/m4 wired) — "update counts/phrases" was a no-op against reality.
> Full workspace suite 2236 passed / 0 failed. Steps 4-5 not started. Work rides
> uncommitted (FRAGO 004). Earlier: segment-5 status preserved below.
> Segment 5 (session `phase3-executor-2026-07-03-m5-seg5`): **step 1 COMPLETE.** The loop-arm
> `entry.value` miscompile is FIXED — root cause was an indirection-level mismatch on the
> MapEntry local-slot contract (both loop arms registered the {i64,i64} entry struct alloca
> directly as the local; `load`/`store`/`materialize_param` all expect the slot to HOLD a
> POINTER to it, so `load` reinterpreted key_bits as the struct pointer). Fix threads the
> canonical `materialize_param` pattern in both arms (emit.rs `mf_*` + `sm_mf_*`) — no new
> derivation, no runtime change. RED matrix released: 6/6 green + a NEW 7th test
> (`m5_p3_map_embed_repr`) locking the debug-repr walker (audit site 7 — verified reachable
> via shape-embedded map fields, scalar + shape-valued cells; previously fixture-less). Map
> grep gate re-verified passing; audit-map-callsites.md fully ticked (all rows). Full
> workspace suite 2235 passed / 0 failed (integration 508/508); one single-run load-flake in
> `v03_m3e_alias_local_name_collision` (concurrency-diagnostic, unreachable from the fix,
> passes isolated + both re-runs) surfaced for the boundary reviewers. Checkpointed EARLY at
> the step-1→step-2 boundary on a fully green tree (context budget; planner's mark sits after
> step 2). Steps 2-5 not started. Work rides uncommitted (FRAGO 004).
> Earlier: segment-4 status preserved below.
> Segment 4 (session `phase3-executor-2026-07-03-m5-seg4`): §H.5 RED matrix authored + wired
> (6 fixtures + 6 integration tests) — and it CAUGHT A REAL MISCOMPILE in the landed cut: both
> map for-loop arms (`mf_*`/`sm_mf_*`) read `entry.value` wrong (uninit-stack garbage for
> scalar values, SIGSEGV for shapes; every scalar map op — count/get/has/set/index, both key
> types, D13 snapshot, re-set-over-key — proven correct end-to-end on the cut tree). Matrix
> state: 1 green / 5 RED — the plan-prescribed RED lock gating the fix (step 1's own "RED
> matrix that gates the build"); the 501 pre-existing integration tests stay green. §H.6(b)
> checklist ticked for all scalar rows with fixture cites; the 3 iteration rows deliberately
> open with the bug dossier (handoff "THE BUG"). Deviations surfaced for the deviation-judge:
> (a) the spec'd post-resume `.get()` matrix cell is NOT constructible — typeck Check 2b's
> coarse post-wait crossing over-approximation rejects ANY maybe consumed after a wait
> (pre-existing, cut-independent; cell restructured with coverage preserved); (b) the driver
> EMBEDS libynz_runtime.a at driver-build time — a stale .a produces ABI-skew failures that
> mimic miscompiles (build-order landmine documented in the handoff); (c) pre-cut per-cell
> record declared unreliable due to (b) — the binding record is the cut-tree one. Earlier
> segments: 1-2 (sessions `phase3-executor-2026-07-03-m5`, `…-seg2`):
> recon + implementation-grain cut design, zero tree changes. Segment 3 (session
> `phase3-executor-2026-07-03-m5-seg3`): **the step-1 map ABI cut LANDED** — runtime rewritten
> (elem_size-aware `YnzMap`, counted 5-buffer accounting, flag-return get/iter, src-ptr set),
> runtime_decls retyped, emit.rs map choke-point section added (reuses the array elem-size
> helpers — no twin), all 8 call-site groups hard-cut migrated, grep gate H.6(a) passing,
> 13 golden IR snapshots verified as decl-signature churn + refreshed (34/34), 3 M3D map
> alloc-parity tests re-pinned to the deliberate +10 gap (2 maps × 5 now-counted
> never-dropped buffers, handoff §H.6(c)) — workspace build + full suite GREEN (501/501
> integration after re-pin). Remaining in step 1 (seg 5+): fix the loop-arm `entry.value`
> miscompile → 6/6 matrix green → full suite → tick the 3 iteration checklist rows. Steps 2-5
> not started. Work rides uncommitted (FRAGO 004 — no commit before full-phase completion).

- **Task + purpose:** finish the by-value substrate — lift the interim guard the fix retires, close
  the symmetric `map<K,Shape>` bug, and prove the leak-parity + suspension story (E7/E8, and E9's
  B1 precondition).
- **Steps**
  1. **`map<K,Shape>` symmetric by-value fix (E12)** *(heavy — pre-existing base bug, miscompiles
     with OR without suspension per the scratch doc)*. **Entry criterion:** the P0 `ynz_map_*` audit
     checklist (Phase 0 step 4) is complete — code no map site before it is enumerated. Apply the
     same hard-cut/single-choke-point discipline as arrays: DELETE the old uniform-slot map entry
     points, route all map element loads/stores through one elem-size choke point, and author a RED
     `map<K,Shape>` matrix fixture (literal/runtime shape values × get/set/contains/index ×
     non-suspending AND crossing-`wait`) that gates the build. **Grep gate:** zero old-signature
     `ynz_map_*` decls; zero raw `ynz_map_*` calls outside the choke-point helpers; every audited map
     site ticked.
  2. **Lift `ArrayShapeRuntimeFieldWithWait`:** remove the typeck guard; retire the registry
     deferral entry (`array-shape-runtime-field-with-wait`); rewrite
     [`IMP-concurrency.md`](../../../../docs/internal/implementation/IMP-concurrency.md):588-598
     from "interim guard" to "fixed by v0.3-M5 by-value storage"; remove/repurpose the old error
     gallery triggers for this diagnostic + update `error_galleries.rs` counts/phrases.

     **CHECKPOINT** — guard gone, galleries consistent, tree green.
  3. **Crossing-wait acceptance fixture:** runtime-field `array<Shape>` as crossing local / loop var
     across a `wait` prints correct values (the scratch doc's named acceptance signal; E9's B1
     proof). **Get-side-escape cells across suspension (P2 fix-loop carry):** the P2 boundary
     locked the escape-the-iteration ownership contract with `m5_p2_byval_*escape*` cells,
     including the constructible frame-embed half (`m5_p2_byval_shape_escape_wait`); the
     `maybe<Shape>`-crossing-`wait` sibling is NOT constructible today — typeck's
     `UnsupportedCrossingLocalType` rejects ANY maybe crossing a suspension (a broader guard than
     the array-specific one this step lifts) — so whichever step/milestone lands maybe-crossing
     frame support MUST add the get-maybe payload-across-resume cell in the same change (the
     payload's owned region is stack storage that dies at suspension; the frame story must carry
     the payload bytes, not the pointer) AND the SM-path spawn-arg maybe cell (P2 fix round 3's
     `HeapMaybeEnv` descriptor arm — see step 5 item (g); non-constructible under today's guard
     for the same reason).
  4. **Alloc=free parity gate (E8):** **Entry criterion (FRAGO 005):** first verify the counter
     demonstrably observes array/map buffer alloc/free — non-zero alloc counts on the array suite,
     vs the P0 baseline's recorded alloc=0 blindness (`baselines-p0.md`). A counter that cannot
     see buffers means the gate FAILS LOUD here — it never passes vacuously. Only once that
     visibility is proven, gate on parity. **Parity SEMANTICS (re-specified per FRAGO 009):** the
     gate targets E8's ACTUAL target — **"no NEW leak class vs the pointer representation"**:
     per-element and per-iteration allocation regressions and clone→drop imbalance must be ZERO
     (D6's own framing). That is DISTINCT from the PRE-EXISTING never-drop-local-arrays design
     that FRAGO 005's counted visibility merely made visible (+2 counted allocs per array — header
     + buffer — held until process exit): literal suite-wide `alloc == free` is impossible without
     a drop story that does not exist, and this gate does not demand it. **THIRD accounting
     category (FRAGO 011):** the counted PERSIST cells minted by the P2 fix rounds 2–3
     (`store_field` shape/maybe cells, `value_to_stable_bits` map-insert + array-maybe-element
     cells, `prepare_bg_arg_for_ctx` spawn-descriptor cells) are
     **accounted-and-deferred-to-drop-story** — deliberate alloc-without-free by design (eager
     free dangles shallow-copy siblings; `ynz_map_drop` never freed value cells pre-M5 either,
     lib.rs:1086-1092). They are NOT a parity RED under this gate's "no NEW leak class"
     semantics; the re-set-over-a-key leak (+1 never-freed counted cell per overwrite vs the
     pointer repr) is ASSIGNED to this step's / step 5's drop-story verdict, and the persist gap
     is pinned EXACT-COUNT (same mutation-proof-teeth discipline as the interim array-gap helper
     below) so any drift from the deliberate class fails loud. **The P2 test helper's
     `alloc == free + 2×arrays` encoding (`m3d_assert_fires_byte_identical_alloc_gap`,
     integration.rs) is RATIFIED as INTERIM** — pinning the exact gap keeps mutation-proof teeth —
     pending this step's verdict; the drop-insertion-vs-D6-fallback verdict stays THIS step's to
     make. Parity (in the re-specified sense) REDs → execute D6's fallback (loud-reject heap-field
     shapes + contingent `[[diagnostic_template]]`) via FRAGO, never a silent leak.
  5. **Suspension/ownership adversarial sweep:** arrays × `wait` × `background` (give/copy) ×
     `.copy()` on the by-value substrate — the AoS half of E9's matrix — dual-mode byte-identical.
     **E8-class review items (FRAGO 008 carry + P2 fix-loop observations):** (a) D12's
     pointer-typed-fields-by-identity question — `shape_value_eq` compares string/nested-shape
     fields by pointer identity; ratify identity or extend to deep value equality (heap-field
     shapes are the scratch doc's Risk-3 class this sweep owns); (b) shape values stored into
     shape FIELDS (`s.part = elem`) still store pointers — the get-side escape-copy discipline
     (emit.rs `store_binding`) covers variable bindings only *(STATUS, P2 fix round 2: the
     mechanism half is FIXED — `store_field` now clones shape/maybe bytes into counted heap
     cells, tripwired `m5_p2_byval_*field*escape`; what remains for this sweep is the ownership
     half — the deliberate alloc-without-free-per-persist class, incl. re-set-over-a-map-key
     leaking the old cell — reviewed under step 4's parity semantics, deviation D-r2-2 pending
     judgment)*; (c) map iteration `MapEntry` bindings
     alias a per-site entry slot (same reuse class store_binding fixed for arrays) — cover it in
     step 1's map matrix escape cells; (d) **size-derivation twin (FRAGO 010)** — the SM Let
     embed memcpy (and the `bind_sm_result_and_flush` flush) size shape copies from
     `struct_ty.size_of()` (emit.rs, SM shape-embed Let arm) while `shape_frame_slots` and the
     array/map elem-size choke points read the precomputed `shape_abi_sizes`
     (`TargetData::get_abi_size`) — two derivations of the same size question with no
     compile-time link; unify onto `shape_abi_sizes` or add a compile-time parity link per
     authoritative-derivation §3; (e) **union persist marshalling — the KNOWN HOLE in
     `value_to_stable_bits` (P2 fix round 3)**: union repr is NON-uniform (a `{i64 tag, i64
     data}` tagged struct from the annotated-Let ctor, but a NULL pointer for the `T | nothing`
     none case), so no blind clone is correct — a 16-byte clone segfaults on the null repr and
     still persists the interior stack payload pointer; probe-verified reachable-but-loud-blocked
     today (write-side persistence through `map<K,Union>.set` and `array<Union>` literals DOES
     compile and run — raw-pointer persist through the marshalling choke points — and only the
     read-back path ICEs, loud, not silently wrong; corrected per FRAGO 012), documented in the
     helper's doc (see `value_to_stable_bits`'s KNOWN-HOLE doc comment in
     `crates/ynz-codegen/src/emit.rs` and the two loud-fail pins
     `m5_p3_sweep_union_readback_blocked_array`/`_map` in
     `crates/ynz-driver/tests/integration.rs`) —
     CLOSE it when union narrowing improves enough to need a real diagnostic instead of the
     documented loud ICE (D6's full loud-reject remains unbuilt by design, per FRAGO 012); (f)
     **contains-on-maybe raw envelope-pointer compare (pre-existing, NON-persist)**: the
     contains loop marshals the maybe target's raw envelope bits via the compare-only
     `array_elem_bits64` — consumed in place, no escape, but bit-identity compare on a maybe
     envelope was semantically questionable pre-M5 too; ratify or fix alongside item (a)'s
     equality verdict; (g) **spawn-arg maybe coverage boundary (P2 fix round 3)**: step 3's
     maybe-crossing tripwire obligation ALSO names the SPAWN-ARG maybe case — the
     `BgArgFreeKind::HeapMaybeEnv` SM-descriptor arm is protocol-identical to the
     HeapShape-tested wire kind 0 but has NO direct fixture (a maybe param crossing the callee's
     own `wait` is rejected by `UnsupportedCrossingLocalType` today; a read-before-wait variant
     is racy-RED, a weak tripwire); whichever change lands maybe-crossing frame support MUST add
     the SM-path spawn-arg maybe cell alongside step 3's get-maybe payload-across-resume cell.
- **Exit criteria:** `map<K,Shape>` fix green with its RED matrix + map grep gate passing (E12);
  guard + registry deferral gone; IMP-concurrency updated; parity green; crossing-wait fixture
  green; sweep green.
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (the map matrix vs the P0 map audit
  checklist — every audited `ynz_map_*` site has a fixture — plus the sweep + parity evidence).
- **Model tag:** `(coding, high, large)` — scale=large (map ABI cut + array suspension sweep);
  checkpoint marks mandatory.

#### Phase 4 — SoA candidate analysis + the ONE authoritative layout-decision source

> **STATUS: COMPLETE (2026-07-03).** Steps 1–2 landed across segments 1–3 (query + authority +
> padding re-threading, byte-identical — see audit.md). Segment 4 (session
> `phase4-executor-2026-07-03-m5-seg4`) landed steps 3–5: the 8-fixture `m5_p4_soa_*`
> decline/admission set (qualifying / 3-field union / escaping / growth-op / lend-self /
> runtime-length / small-N / both-candidate); the E3 byte-layout integration test
> (`m5_p4_soa_both_candidate_padding_wins_byte_layout` — padded `{ i64, [56 x i8] }` IR intact
> under the SoA-candidate collision + exact dual-mode-identical stdout); and the exit-criterion
> analysis suite `crates/ynz-typeck/tests/soa_analysis.rs` (12 tests: exact verdict/reason
> payloads for all 8 fixtures; authority cells — qualifying → `Soa { segments: [x@0, y@1] }`,
> the E10 forward-compat surface, and both-candidate → `Aos { declined: CrossThreadPadded }` +
> `padded_shapes ∋ "Tally"` per D11; entry gates — `YNZ_NO_AUTO_PARALLEL=1` fresh-db → empty
> candidate list, kernel mode via the pure `soa::analyze(_, _, true, false)` core → empty).
> All 8 fixtures verified through the real compiler binary with exact tabled stdout. Receipts:
> full workspace suite **2264 passed / 0 failed** (= the 2251/0 segment-3 baseline + exactly
> the 13 new tests); clippy `-D warnings` clean; fmt `--check` clean; zero snapshot churn.
> Grep gates: ZERO codegen reads of `LayoutDecisions::arrays` (sole textual hit = the
> emit.rs:854 param doc stating this very criterion); `env::var("YNZ_NO_AUTO_PARALLEL")`
> parsed only at queries.rs:598; zero `cross_thread_padded_shapes` reads in ynz-codegen; zero
> ABI-size derivation in typeck soa.rs (`FieldSegment` carries order/names only, CCIR-2).
> handoff-phase-4.md deleted at phase close.
>
> **Closing round (session `phase4-executor-2026-07-03-m5-closing`)** — post-boundary-review
> should-fix pair, both landed: (1) reassign-staleness — `Stmt::Assign` on a tracked binding now
> conservatively declines it (`Escapes { how: "reassigned after initial binding" }`, soa.rs Assign
> arm; the stale-`provable_len` E3/E7 risk closed before Phase 5 reads admissions); fixture
> `m5_p4_soa_reassigned.ynz` + exact-reason test. (2) `NoPerFieldLoopAccess` coverage — the one
> live decline branch the step-4 fixture table never named; fixture
> `m5_p4_soa_no_field_access.ynz` + exact-reason test. Fixture set is now 10, analysis suite 14
> tests. Receipts: workspace **2266/0** (= 2264 + exactly the 2 new tests); all 12 pre-existing
> exact-verdict tests unchanged-green (the 8 original fixtures' verdicts UNCHANGED); clippy/fmt
> clean; zero `*.snap.new`; all grep gates re-held (env parse only queries.rs:598, zero codegen
> `cross_thread_padded_shapes` reads, `.arrays` sole hit = emit.rs:854 doc, no ABI-size derivation
> in soa.rs). Both fixtures run clean through the real binary (stdout `10\n20` and `66`).
>
> **Closing round 2 (session `phase4-executor-2026-07-03-m5-closing2`)** — the re-`let`-shadow
> sibling of closing-round finding 1, surfaced by code-reviewer's re-check: Pass 1
> (`collect_bindings_block`) kept the FIRST record on a same-name re-`let` (legal Yinz shadowing in
> non-suspending functions — `check_let` overwrites the scope entry, no duplicate error), so a
> 72-element qualifying binding shadowed by a 4-element re-`let` stayed
> `Admitted { provable_len: 72 }` — the same E3/E7 stale-provable_len class, in the Pass-1 path.
> Fix mirrors the Assign treatment: a `Stmt::Let` whose name is already tracked declines the record
> (`Escapes { how: "rebound by a later let" }`), firing regardless of the shadow's own type. Fixture
> `m5_p4_soa_let_shadow.ynz` (larger-then-smaller — the miscompile-risk direction) + exact-reason
> test. An exhaustive rebinding-construct sweep (all `Stmt`/binding forms in `ynz-ast/src/nodes.rs`)
> found NO third gap: Let-first/Let-shadow/Assign now all handled; FieldAssign/IndexAssign mutate
> elements, never rebind (check.rs:5755/5920 — `.set` sugar, scope entry never replaced); for-loop
> vars (incl. destructure/map-destructure) are scoped per-element bindings whose worst case is a
> conservative spurious escape, never a stale Admitted; array params are unconditionally declined
> rows (never Admitted → no staleness path); Match patterns (Value/Is/OptionName) bind nothing;
> `background` captures args without rebinding. Fixture set now 11, analysis suite 15 tests.
> Receipts: workspace **2267/0** (= 2266 + exactly the 1 new test); all 14 prior exact-verdict
> tests unchanged-green (10 prior fixtures' verdicts UNCHANGED); clippy/fmt clean; zero
> `*.snap.new`; grep gates re-held (env parse only queries.rs:598, one padded-set thread,
> zero ABI-size derivation in soa.rs).

- **Task + purpose:** productionize the S2 spike into `soa_candidate_query` and build the single
  layout authority BOTH transforms (padding, SoA) resolve through — E3's B1 mitigation lands HERE,
  before any SoA codegen exists. Typeck-first with zero codegen consumers (the M4 P4 pattern).
- **Steps**
  1. **`soa_candidate_query(db, source) -> Vec<SoaCandidate>`** (salsa precedents:
     `frame_layouts_query`, `codegen_query`). Admission criteria, each with a decline-reason enum:
     compile-time-provable length > SIZE_THRESHOLD (const 64, provenance pending Phase 6); ≤2-field
     union across all loops (D5); no growth ops (D3); no escape (D4); shape-level lend-self filter
     (mirror `finalize_false_sharing`'s scan, false_sharing.rs:131-134); FFI check documented
     vacuous (D7). Entry-gated by `kernel_mode || no_auto_parallel_env()` — the SAME predicate,
     threaded, never re-read via a second path (D1; A3).
  2. **The layout authority:** one `layout_decisions` artifact (struct/query) resolving padding ×
     SoA per shape+array. **Precedence per recorded decision D11: padding wins — a
     cross-thread-padded shape's arrays are NEVER SoA'd** (correctness under false sharing beats
     cache locality, and a cross-thread array is outside SoA's provable-single-thread hot-loop model
     anyway). Re-thread
     M4's existing consumers — all THREE real sites, verified and corrected during Phase 4
     execution (FRAGO 013; the A1/CCIR-1 re-verify found a third consumer the original recon
     missed): `emit.rs:1057` (now `emit.rs:1064` post-rethread), the deep `Cg`-level struct-lit
     alignment read at `emit.rs:16848` (now `emit.rs:16872` post-rethread), and
     `codegen/queries.rs:208-212` (the `frame_layouts_query` sizing pass) — to
     read THIS source, so exactly one artifact answers every layout question.

     **CHECKPOINT** — authority landed, padding consumers re-threaded, tree green, no behavior
     change (padding output byte-identical to pre-phase).
  3. **Both-candidate RED fixture (E3 mitigation 2):** a shape simultaneously cross-thread-padded
     AND SoA-candidate; byte-layout assertions prove padding intact + SoA declined with the right
     reason (precedent: `crates/ynz-driver/tests/fixtures/v0_3_m4_p4_padding_gate.ynz`).
  4. **Suppression/decline fixture set (E6):** qualifying; 3-field union; escaping; growth-op;
     lend-self shape (a `Pirate.recordHit`-style case); runtime-length; small-N. Each asserts the
     specific decline reason. These are the enumeration mandate's exercise inputs (Phase 8 step 3).
  5. Expose the decline-reason + layout metadata (`LayoutDecision` carries field segment info — the
     E10 forward-compat surface the Phase 7 lint hover consumes).
- **Exit criteria:** query + authority green with ZERO codegen consumers of SoA decisions yet; all
  fixtures green at analysis level; grep gate: no second derivation of any admission predicate.
- **Reviewer fan-out:** code-reviewer; design-doc-alignment reviewer (criteria vs roadmap.md:109 +
  this plan's D3–D7); adversarial gate-checker (fixture set vs criteria — one fixture per criterion
  minimum).
- **Model tag:** `(coding, high, medium)`

#### Phase 5 — SoA codegen on the by-value substrate

> **STATUS: COMPLETE (2026-07-04).** All 5 steps done across 5 segments (sessions
> `phase5-executor-2026-07-03-m5` through `…-seg5`; the closing segment-5 paragraph below
> carries the phase-exit receipts; per-segment detail in the paragraphs below + audit.md).
>
> Segment 1 (session
> `phase5-executor-2026-07-03-m5`) returned PARTIAL at `phase-5/step-1`: full phase
> orientation + implementation design paid and recorded in `handoff-phase-5.md`
> (verification receipts, gather/scatter design, consumer-site inventory, E6 total-
> coverage argument), plus step 1's runtime primitive landed green —
> `ynz_array_new_sized(elem_size, cap)` in `crates/ynz-runtime/src/lib.rs` (exact-cap,
> len==cap, counted allocs, same header/drop per D2/D6; unit test
> `array_new_sized_len_cap_set_drop` PASS) + its declaration in
> `crates/ynz-codegen/src/runtime_decls.rs`. Receipts: build clean, clippy `-D warnings`
> clean (ynz-runtime + ynz-codegen), fmt clean. DEVIATION SURFACED (undecided, for
> deviation-judge): array `.copy()` today lowers as an ALIAS no-op for all arrays
> (emit.rs `lower_postfix_op` `_ => Ok(recv_val)` catch-all) — the step-4 E9 matrix
> assumes deep-copy; proposed resolution (not applied) is deep-copy in BOTH modes;
> steps 1–3 are unaffected. Steps 1 (remainder) through 5 remain open.
>
> Segment 2 (session `phase5-executor-2026-07-03-m5-seg2`) returned PARTIAL at
> `phase-5/step-1` (sub-marker `phase-5/step-1-seg2-frago014-and-belt-gate-landed`):
> (a) FRAGO 014's ratified plan-text correction APPLIED to step 4 above (`.copy()`
> deep-copy in both modes, closing the alias-no-op — the segment-1 deviation is now
> resolved by that FRAGO); (b) step 3's belt assert LANDED green —
> `crates/ynz-codegen/src/emit.rs` `build_module` now refuses to lower when any
> `LayoutKind::Soa` decision exists under `no_auto_parallel` (belt on the ONE Phase 4
> entry gate, threaded predicate, no second derivation). Receipts: build/clippy
> `-D warnings`/fmt clean; `cargo test -p ynz-driver --test cross_impl_consistency`
> 2 passed (assert exercised across the whole corpus under `YNZ_NO_AUTO_PARALLEL=1`,
> never fired). (c) NEW step-4 hazard recorded in `handoff-phase-5.md`: `.copy()` in
> background-arg position double-copies + leaks the intermediate once FRAGO 014's
> deep-copy lands (spawn path already clones array args) — E8 parity + golden risk,
> for the step-4 executor to verify, not self-decided. (d) Implementation design
> hardened in the handoff (choke-param `Option<&SoaArrayInfo>` threading for
> compiler-forced E6 totality; loop-var-only masking, verified against soa.rs's
> nested-shadow decline; `.copy()` lowering plan under FRAGO 014). Steps 1
> (remainder: construction lowering), 2, 3 (oracle-divergence half), 4, 5 remain
> open — resume from the handoff's settled design.
>
> Segment 3 (session `phase5-executor-2026-07-03-m5-seg3`) returned PARTIAL at
> `phase-5/step-2-construction-access-green-checkpoint-evidence-persisted`: steps 1, 2
> AND 3 are now COMPLETE. (a) Step 1 construction lowering LANDED — `Stmt::Let`
> interception keyed by (array_name, decl_span) against `cg.layout.arrays`
> (`lower_soa_construction` in emit.rs: ONE `ynz_array_new_sized(elem_size, cap)`
> buffer, compile-time segment offsets per D2, scatter per element; same header/drop
> per D6). (b) Step 2 access lowering LANDED — `SoaArrayInfo`/`SoaSegment` + a new
> `soa: Option<&SoaArrayInfo>` param on ALL FOUR choke helpers (get_into/get_maybe/
> set/push — compiler-forced E6 totality; push = hard Err on SoA), gather/scatter
> helpers inside the E7 choke section (gather = full-element into the same out
> buffer, OOB memset parity; scatter OOB = raw runtime abort parity), `shape_field_abi`
> threaded from the ONE TargetData pass in build_module, loop-var-only masking in both
> array for-in lowerings, SM shape-embed gather honors the frame-region out-pointer
> contract. (c) **Planner CHECKPOINT satisfied**: release-mode (`opt-18 -O2`) IR of
> `m5_p4_soa_qualifying.ynz` shows SROA eliminated the gather out-buffer and the hot
> loop's surviving loads are exactly the two used-field segment loads, contiguous
> (x at `data+8i`, y at constant `+576`, stride 8) — evidence persisted to
> `p5-ir-evidence.md` (sibling file, committed) for Phase 6/8. (d) Step 3's remaining
> half CLOSED: with SoA codegen live, `cross_impl_consistency` passed over the whole
> corpus (2/2) and the qualifying fixture runs byte-identical across modes (cmp clean,
> both exit 0) — the dual-mode oracle now genuinely exercises AoS-vs-SoA divergence.
> Receipts: build + clippy `-D warnings` + fmt clean (ynz-codegen); SoA FIRED (438
> soa-named instructions in the fixture's IR — not a silent decline); E7 grep gate
> clean (all raw `rt.ynz_array_{new,push,get,set}` refs in-section); E3 grep gate
> clean (zero `soa_candidate|hot_fields|whole_value_uses` reads in ynz-codegen).
> Steps 4 (E9 matrix + FRAGO 014 `.copy()` fix + the recorded bg-arg double-copy
> hazard) and 5 (full-suite cross-impl run) remain open — see `handoff-phase-5.md`.
>
> Segment 4 (session `phase5-executor-2026-07-03-m5-seg4`) returned PARTIAL at
> `phase-5/step-4` (sub-marker `phase-5/step-4-copy-fixed-bgarg-hazard-resolved`):
> steps 4a + 4b LANDED green. (a) FRAGO 014 `.copy()` fix — `lower_postfix_op`'s
> Copy arm gained a `Type::BuiltinArray` arm: AoS receiver → `ynz_array_clone_primitive`
> (elem_size-aware byte deep copy = one-level semantics; pointer cells alias per
> D12/D13); SoA receiver → new choke-section helper `soa_copy_to_aos` (runtime
> gather loop into a fresh AoS buffer via `ynz_array_new_sized` + the existing
> `soa_gather_into`/`array_elem_set(soa=None)` choke points — the copy is AoS
> because the copy's binding is authority-declined (`provable_len` None), so all
> its reads lower AoS; a segmented copy would be misread). (b) Bg-arg double-copy
> hazard RESOLVED at the choke point: `prepare_bg_arg_for_ctx`'s BuiltinArray arm
> now transfers ownership of an explicit spawn-site `.copy()` arg to the task
> (`BgArgFreeKind::HeapArrayPrimitive` → task drop ladder frees it) instead of
> re-cloning. VERDICT: genuine LEAK, not harmless waste — alloc-counter receipts on
> `m5_p3_sweep_bg_array_shape_give_wait.ynz`: baseline (alias `.copy()`) alloc=9
> free=5 gap=4 (= 2 never-drop local arrays × 2, FRAGO 009 accounting); with 4a
> alone alloc=11 free=5 gap=6 (the `.copy()` intermediate leaks — E8 clone→drop
> imbalance, the zero-tolerance class); with 4a+4b alloc=9 free=5 gap=4 — restored
> exactly, output `caller: 119 / given: 30 / copied: 30` correct. Receipts: build +
> clippy `-D warnings` + fmt clean; targeted tests green (m5_p3_sweep_bg_array_shape_*
> 2/2, m5_p3_e8_* 3/3 incl. the exact-count parity pin, v03_m3b_p2_explicit_copy_honored);
> E7 gate clean (rt.ynz_array_{new,push,get,set} refs all in-section; the new
> `ynz_array_new_sized` call sits in-section too); E3 gate 0 hits. Steps 4c (E9
> SoA×wait×background×.copy()×dual-mode matrix fixtures + AoS `.copy()` independence
> lock fixture + both-candidate no-SoA-fired IR assert) and 5 (full-suite cross-impl
> run) remain — see `handoff-phase-5.md` for the settled fixture designs.
>
> **Segment 5 — CLOSING (session `phase5-executor-2026-07-03-m5-seg5`, 2026-07-04):
> steps 4c + 5 COMPLETE; phase closed.**
> (a) **SM-path open item DEFINITIVELY ANSWERED — shared interception, no gap.** Every
> route in `lower_sm_block` lowers a non-suspending `Stmt::Let` via `lower_stmt`
> (emit.rs no-auto-parallel arm :5621, spike pre/post :5673/:5732, fused pre/post
> :5783/:5828, default-partition Singleton :5873), and `lower_stmt`'s Let arm carries
> the ONE SoA construction interception (:12408–:12442); the SM-specific `Stmt::Let`
> arms (:6387/:6416/:6450/:6476) match only wait/suspending-call initializers — an
> `ArrayLit` can never route there. Proven empirically: a wait-only variant of the
> qualifying fixture emits 1156 soa-named IR lines (SoA fires in an SM main), and the
> shipped matrix fixture's IR assert passes.
> (b) **Step 4c E9 matrix landed** (3 new tests + 1 extension, integration.rs):
> `m5_p5_copy_aos_independent` (FRAGO 014 independence lock — mutate-after-copy on
> shape-elem + int-elem AoS arrays, copy keeps pre-mutation sums, dual-mode exact
> stdout `a: 119/b: 30/nums: 105/copy: 6`); `m5_p5_soa_copy_wait_bg_matrix`
> (fixture `m5_p5_soa_copy_wait_bg.ynz`: 72-elem SoA Point array × wait-crossing SM
> main × `.copy()` snapshot (SoA→AoS, pre-mutation 7884) × post-wait IndexAssign
> scatter read through segments (2727/5454) × `background` with spawn-site `.copy()`
> on the SM-descriptor path (bg 30 vs caller 119), dual-mode byte-identical + IR
> asserts: default-mode IR CONTAINS soa_new/soa_ctor, `--no-auto-parallel` IR contains
> ZERO); `m5_p5_bg_copy_alloc_gap_pin` (alloc−free == 4 on the give_wait fixture —
> locks the seg-4 4b ownership-transfer fix; gap 6 = the re-clone leak signature);
> both-candidate test extended with the end-to-end NEGATIVE half (default-mode IR has
> ZERO soa instructions — padding wins through codegen, not just analysis, D11).
> **D11 design constraint discovered live** (paper-traced, not a bug): passing any
> Point value — even a spawn-site `.copy()` — across `background` makes the shape
> cross-thread PADDED (M4 padding is shape-level) → `Aos { declined:
> CrossThreadPadded }`; admission stays Admitted{72,[x,y]} but the authority resolves
> padding-wins. So "SoA × background on the SAME shape" is structurally D11-excluded;
> the matrix fixture carries the bg cell on a second shape (Part) and documents the
> constraint in its header (the class is already pinned at analysis level in
> soa_analysis.rs's both-candidate suite).
> (c) **Step 5 full-suite receipts:** full `cargo test -p ynz-driver` COMPLETED
> (exit 0; integration 522/0, m2 SM 31/0, cross_impl_consistency both corpus sweeps
> green — the whole driver-fixtures + examples-entrypoints corpus byte-identical
> across default and `--no-auto-parallel` modes, ≥30 files compared; ynz-codegen's
> tests consume driver fixtures — 3 object-byte SHA goldens — no separate corpus);
> full workspace `cargo test --workspace --no-fail-fast` **2271 passed / 0 failed**;
> clippy `--workspace -- -D warnings` clean; `cargo fmt --all -- --check` clean.
> E3 gate: 0 `soa_candidate|hot_fields|whole_value_uses` hits in ynz-codegen. E7
> gate: all `rt.ynz_array_{new,push,get,set}` refs at 2544/2692/2752/2782/2872 —
> inside the choke section (2235–3403); `ynz_array_new_sized` (not a gated symbol)
> has exactly 2 call sites: soa_copy_to_aos in-section :2587 + the ratified
> construction interception :12381. **Snapshot churn: 13 pre-existing IR snapshots
> refreshed, investigated BEFORE accepting** — every delta is exactly the one added
> `declare ptr @ynz_array_new_sized(i64, i64)` line (seg 1's plan-ratified runtime
> primitive declares in every module; zero instruction-level changes; object-byte
> SHA goldens unaffected). Zero `*.snap.new` remaining.
> handoff-phase-5.md deleted at phase close.
>
> **Closing round (session `phase5-executor-2026-07-03-m5-closing`)** — post-boundary-review
> (7-dispatch fan-out, ZERO blockers): one should-fix landed + two durable deferrals filed +
> FR #12 recorded. (1) Should-fix (code-reviewer, correctness): `soa_gather_into`'s HIT path
> stored fields via struct_gep but never zeroed `out` first, leaving inter-field/tail struct
> padding nondeterministic — AoS `ynz_array_get` copies the FULL `elem_size` bytes (runtime
> lib.rs:1244), so a raw-byte consumer (memcmp, byte-hash) could see SoA-vs-AoS padding-byte
> divergence despite identical field values. Fix: `build_memset(out, …, elem_size)` at the top
> of the hit path (emit.rs :2459-2466, matching the OOB path's existing memset convention) —
> padding bytes now deterministically zero; doc comment records the WHY. Receipts: build +
> clippy `-D warnings` + fmt clean; SoA subset green (m5_p5_* 3/3, both-candidate 1/1 — exact
> pre-fix stdout pins hold = byte-identical behavior); `cross_impl_consistency` 2/2 whole-corpus
> dual-mode sweep; full workspace suite green (2271-test baseline, zero new tests — padding-only
> fix); E3 gate 0 hits; E7 gate all gated refs in-section (2556-2884 ∈ 2235-3415, refs shifted
> +12 by the insert); zero `*.snap.new`. (2) Deferrals filed in the ROADMAP's audit.md (keys
> grep-confirmed absent first): `…#5: crates-ynz-codegen-src-emit-rs-17743` (`fixed<T>.copy()`
> alias no-op — outside FRAGO 014's array scope) and `…#5: crates-ynz-codegen-src-emit-rs-2445`
> (SoA bounds predicate = documented-equivalent second derivation of the runtime check; inert
> under D3's len==cap). (3) FR #12 added below (shape-level padding granularity forfeits SoA
> for all of a shape's arrays — the segment-5 D11 live discovery, working-as-designed).

- **Task + purpose:** emit the SoA layout variant for admitted arrays — the miscompile-risk core
  (E3/E6/E9) — consuming ONLY the Phase 4 authority.
- **Steps**
  1. **Construction lowering** *(heavy)*: admitted `ArrayLit` constructions allocate ONE segmented
     buffer (D2: per-field segment offsets from cap × field ABI sizes, computed compile-time);
     scatter element fields into segments. Same `YnzArray` header, same drop path.
  2. **Access lowering** *(heavy)*: every same-function access (index/get/set/first/last/contains/
     count, hot-loop and cold) re-lowered via segment addressing; cold-field fan-out per the scratch
     doc's model.

     **CHECKPOINT** — construction + access green on the qualifying fixture; release-mode IR
     inspected: the hot loop's field loads are contiguous (vectorizer-friendly) — evidence persisted
     for Phase 6/8.
  3. **Gates:** SoA decisions already cannot exist under `--no-auto-parallel`/kernel (Phase 4 entry
     gate); add a codegen assert that no `SoaCandidate` arrives when the predicate is set (belt
     verifying the one gate, not a second derivation). Dual-mode oracle now exercises AoS-vs-SoA
     divergence detection for real.
  4. **E9 SoA matrix:** first, fix array `.copy()` to perform a genuine deep copy — shallow/
     one-level, mirroring the existing `Type::Shape` arm's already-shallow memcpy semantics (nested
     pointer fields like string or nested shape still alias, consistent with D12/D13) — in BOTH
     layout modes (AoS and SoA), closing the pre-existing alias-no-op bug (`emit.rs`
     `lower_postfix_op` Copy arm's `_ => Ok(recv_val)` catch-all — an M4-era stub, not introduced by
     Phase 5) rather than mirroring it into the new SoA path (FRAGO 014). Then run the SoA ×
     `wait` × `background` × `.copy()` × dual-mode matrix against the corrected `.copy()` behavior;
     plus the Phase 4 both-candidate fixture now asserted end-to-end through codegen.
  5. **Full-suite cross-impl run:** every `examples/` + codegen-tests + driver-tests fixture
     byte-identical across modes.
- **Exit criteria:** all matrices green; dual-mode byte-identical suite-wide; grep gate: layout
  answers read ONLY from `layout_decisions` (no re-derivation in emit paths).
- **Reviewer fan-out:** code-reviewer; adversarial gate-checker (E9 matrix + both-candidate
  end-to-end); design-doc-alignment reviewer (zero-cost claims vs GR8; scratch-doc transform model).
- **Model tag:** `(coding, high, large)` — scale=large; checkpoint marks mandatory.

#### Phase 6 — Benchmark harness + SIZE_THRESHOLD calibration (E2)

- **Task + purpose:** build the repo's first benchmark harness and replace the guessed 64 with an
  evidenced constant — or honestly re-confirm 64 — with variance recorded and a revisit trigger.
- **Steps**
  1. **Harness:** dev-only (`criterion` dev-dependency — MIT/Apache-2.0, never shipped in
     `libynz_rt.a`) driving compiled `.ynz` workloads: the roadmap-named physics-update loop over
     `array<Player>` with `x`/`y` access (roadmap.md:109), at N ∈ {8…4096}, SoA forced on/off via
     `YNZ_SOA_FORCE` (D8, harness-only).
  2. **Runs:** ≥10 repetitions per point inside the Docker `dev` container, per the S3 protocol.

     **CHECKPOINT** — harness green + raw numbers committed.
  3. **Calibrate:** find the crossover N; keep 64 if the crossover is within the S3 noise floor of
     it, else update. Commit the provenance record WITH the constant: workload, machine/container,
     date, variance, the honesty note that this is uncontrolled shared hardware, and the revisit
     trigger (Future Requirements #2).
  4. Wire the final constant into `soa_candidate_query` with a comment citing the provenance file.
  5. Record the measured hot-loop improvement at demo-scale N for the lint hover + CHANGELOG. **If
     the measured win is an order of magnitude below the 10-40× claim, STOP and investigate before
     any user-facing text claims it** (paper-trace the numbers, per verification).
- **Exit criteria:** constant + provenance + variance committed together; improvement number
  recorded with its evidence.
- **Reviewer fan-out:** code-reviewer (harness); adversarial gate-checker (does the provenance
  actually support the constant, or is it noise-level hand-waving?).
- **Model tag:** `(coding, standard, medium)`

#### Phase 7 — Teaching surface + registry + docs graduation

- **Task + purpose:** the full same-milestone teaching surface (roadmap constraint 71) + graduating
  the design content out of the scratchpad.
- **Steps**
  1. **Registry:** `[[lint_rule]] array-using-soa-layout` (name roadmap-locked, D9) on M4's
     mechanism — one TOML entry (A2) with jargon-free WHAT/WHAT-INSTEAD/WHY hover adapted from the
     scratch doc (lines 155-158): user-facing text says "stored as separate per-field arrays", never
     "struct"/"Struct-of-Arrays"; the WHY cites the measured Phase 6 number, not a generic claim.
     **Re-verify `jargon_audit.rs`'s scope first (D9's unverified behavior claim):** confirm it
     gates diagnostic/hover TEXT and does NOT reject the registry-internal `array-using-soa-layout`
     identifier; if it also audits identifiers, surface it for Patrick's rename-vs-carve-out call
     rather than working around it. Plus `[[deferred_tooling_feature]] dap-soa-unified-view` (four
     fields per Future Requirements #1, Patrick-signed 2026-07-03).
  2. **Firing site:** one `lint_diagnostic` emission in typeck when a `LayoutDecision` applies SoA —
     fires on the array declaration.
  3. **Explicitly NOT added:** no muted-hint domain (Divergence 5 — no typeable form; do not
     over-correct). One-line note in the registry entry's comment.
  4. **LSP + VSCode:** verify the lint flows through M4's diagnostics path (A1 re-check); VSCode
     extension version bump + at least one screenshot of the SoA lint hover (roadmap constraint 71
     item 5).
  5. **Docs graduation:**
     [`IMP-collections.md`](../../../../docs/internal/implementation/IMP-collections.md) gains
     "Array element storage — by-value inline (v0.3-M5)" and "Auto-SoA layout" sections (decisions,
     rejected alternatives, the E10 serialization forward-compat layout-metadata note); trim both
     SCRATCH docs to pointers per docs-checklist; update `docs/README.md` index rows.
     **Shape-contains docs home (FRAGO 008) + persist-semantics docs home (FRAGO 011):** the
     IMP-collections by-value section OWNS decision D12 (`contains` on `array<Shape>` = field-wise
     value equality) including the rejected alternatives (pointer identity — no by-value analogue;
     padded memcmp — compares garbage pad bytes), AND decision D13 (field-assign = copy-on-persist
     snapshot semantics across all persist surfaces: shape fields, map values, array elements,
     spawn descriptors) including the TS-aliasing teaching note (TS/JS objects are references and
     alias on assignment; Yinz by-value shapes SNAPSHOT at the assignment — spell this out in
     HS-grad wording, it is the single most surprising divergence for the target audience).
     RECONCILE the user-facing REF home: `REF-collections.md:152` documents only the
     predicate-form `.contains(fn)` — the value-form shape-contains semantics is a pre-existing
     spec/impl divergence that needs a named REF owner there (HS-grad wording per spec-writing.md).
- **Exit criteria:** registry round-trips (`schema_smoke`); `jargon_audit.rs` green; VSCode artifact
  builds; docs land per [`docs-checklist.md`](../../../rules/docs-checklist.md).
- **Reviewer fan-out:** docs-consistency reviewer (hover text vs Golden Rule 11/12 + vocabulary);
  code-reviewer (registry + firing site).
- **Model tag:** `(coding, standard, medium)`

#### Phase 8 — Demo, gallery, enumeration mandate, cost profile, release

- **Task + purpose:** the human-eyes-on layer + the release. Authors the FIRST SoA-shaped example in
  the repo (¶1: none exists).
- **Steps**
  1. **Demo:** extend `examples/pirates-roster/entrypoint.ynz` with the first large-array hot-loop
     section — a Pittsburgh-themed physics-ish workload (e.g. cannonball volley trajectories:
     `array<Cannonball{x,y,…}>`, N > SIZE_THRESHOLD, hot loop updating `x`/`y`, cold field access
     after — real work in context, not a print stub), with inline comments pointing at the lint per
     roadmap constraint 71 item 6. Regenerate `expected_stdout.txt` via its script; byte-exact
     golden convention (NOT `insta`) per plan-invariants.
  2. **Error gallery:** `examples/primantis-orders/v0_3_m5_errors.ynz` carrying any new error class
     this milestone shipped (contingent D6 reject if Phase 3 armed it); verify no stale
     `ArrayShapeRuntimeFieldWithWait` triggers remain anywhere (Phase 3 step 2 did the lift —
     re-verify); `error_galleries.rs` counts/phrases updated. If M5 shipped zero new error classes,
     record that explicitly in the gallery README line instead of a vacuous file.
  3. **Suppression enumeration mandate (roadmap.md:156):** enumerate ALL shapes across `examples/`
     + test fixtures; record each array-of-shape site's SoA verdict + decline reason in a committed
     enumeration report; assert every verdict is correct (the `Pirate` lend-self case must show the
     filter firing).

     **CHECKPOINT** — demo + gallery + enumeration green and committed.
  4. **Compile-time cost gate (E11):** wall-clock of the release-profile compiler binary running
     `ynz build` on pirates-roster, like-for-like vs the P0 baseline's documented methodology
     (`baselines-p0.md`; `ynz build --release` does not exist as a CLI flag — main.rs:94-95,
     FRAGO 006); <10% or STOP and optimize before release.
  5. Full cross-impl dual-mode suite, final run.
  6. **Release:** version bump to the v0.3.x patch-line slot current at execution time; CHANGELOG
     from merged PRs since the last tag; `/release` for the tag.
- **Exit criteria:** every Invariants subsection below verified green; tag cut.
- **Reviewer fan-out:** code-reviewer; docs-consistency reviewer (demo comments, CHANGELOG);
  adversarial gate-checker (enumeration report completeness).
- **Model tag:** `(coding, standard, medium)`

### 3.4 Coordinating Instructions

- **EXECUTION GATE (hard):** no phase dispatches until the `v0.3.0` tag exists (A7). The conductor
  checks this before Phase 0.
- **GATE WAIVED for this run (FRAGOs 002–004, 2026-07-03 — see `audit.md` for the full record):**
  execution proceeds ahead of the v0.3.0 tag under Patrick's recorded live waiver, unattended P0→P8,
  isolated in git worktree `../ynz-m5-worktree` (branch `feat/v0-3-m5-auto-soa`). The technical
  dependency A7 protected — M4 Phase 4's `[[lint_rule]]` + false-sharing substrate — is verified
  present at this branch's fork commit `1ac52fd`. Phase 4 carries a pre-dispatch M4-sync step (poll
  main every ~10 min for M4-completion signals, merge into this branch, re-verify A1 cites) before it
  dispatches; FRAGO 004 supersedes the narrower FRAGO 002/003 scoped exceptions below.
- **Scoped exception — Phase 0 + Phase 1 only (FRAGO 002, 2026-07-03; superseded by FRAGO 004 above):** Phase 0 and Phase 1 were
  dispatched ahead of the v0.3.0 tag under a conductor-logged, Patrick-authorized exception, isolated
  in git worktree `../ynz-m5-worktree` — both phases operate exclusively on array/map storage code
  and SSOT docs and touch none of M4's still-unshipped v0.3.0 substrate (A1/E1 are not exercised).
  **Phase 2 onward remains fully hard-gated on the real v0.3.0 tag, unchanged.** See FRAGO 002 in
  `audit.md` for the full reasoning.
- **CCIR — the executor must surface immediately, mid-flight:**
  1. **Recon drift:** any ¶1 file:line cite that no longer matches reality at phase start (recon ran
     against uncommitted M4 P4; v0.3.0 will have moved lines). Every phase re-verifies its file:line
     cites against **THE WORKTREE'S OWN state** at dispatch — never main's working tree, which is a
     different, moving document (FRAGO 007). An anchor that resolves only in main's uncommitted copy
     is a BLOCKED-class mismatch to surface, never to self-remediate; any other load-bearing mismatch
     → surface before building on it.
  2. **Any second derivation:** finding yourself re-computing a layout/admission/suspend answer a
     query already owns → STOP, thread the authoritative source or surface the blocker
     ([`authoritative-derivation.md`](../../../rules/authoritative-derivation.md)).
  3. **Dual-mode divergence:** ANY fixture byte-differing across modes is a build-stopping bug,
     never a "flaky test."
  4. **Spike/gate REDs** (P0 S1/S2, Phase 6 step 5 order-of-magnitude check, Phase 8 cost gate):
     full STOP + surface, never note-and-proceed.
  5. **Design-doc contradiction:** any "design doc X says A; the plan says B" discovered mid-phase —
     surface explicitly, the doc wins pending Patrick (project CLAUDE.md standing rule).
- **Sequencing (load-bearing):** P0 → P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8, strictly. By-value
  (P2/P3) fully green before any SoA phase; the layout authority (P4) before any SoA codegen (P5);
  calibration (P6) after SoA exists to measure.
- **Reviewer obligation (plan-invariants Step 7/9a):** every phase reviewer diffs the diff against
  the CITED DESIGN DOCS, not only the plan's own text — the M2-HALT lesson.
- **Verify-before-complete:** each phase's exit criteria are checked by running the named commands/
  tests, not by narration; numeric claims (parity, cost, crossover) get a paper-trace in the phase
  record.

## Invariants This Milestone Must Preserve

### Safety

- A runtime-field `array<Shape>` crossing a `wait` produces correct output (fixture-asserted) — the
  stack-dangling class is eliminated by ownership of element bytes, not masked by a guard.
- `map<K,Shape>` values are stored by value and produce correct output with and without suspension
  (E12 — the symmetric pre-existing base bug is fixed, not just arrays).
- No new silent-miscompile class: the array per-type × per-operation × suspension matrix (P2/P3),
  the `map<K,Shape>` matrix (P3, E12), and the SoA matrix (P5) gate the build; every audited
  `ynz_array_*` AND `ynz_map_*` call site has a fixture.
- Ownership semantics unchanged: `share`/`lend`/`give` rules, use-after-give, const rules — zero
  diffs in ownership diagnostics across the milestone (existing suite green throughout).
- SoA never changes observable program output: byte-identical stdout/stderr/exit-code across default
  and `--no-auto-parallel` modes for EVERY fixture (the cross-impl harness).
- A cross-thread-padded shape is never SoA'd (byte-layout-asserted both-candidate fixture).

### Performance

- Hot-loop improvement on the calibrated workload measured and recorded with provenance (target
  10-40× per the design doc; the SHIPPED claim is the measured number, Phase 6 step 5).
- By-value storage: ONE allocation per array buffer (GR8) — no per-element heap; SoA: ONE segmented
  allocation (D2). Alloc-count fixtures assert both.
- Compile-time cost of the new analysis passes: <10% wall-clock of the release-profile compiler
  binary running `ynz build` on pirates-roster, like-for-like vs the P0 baseline per
  `baselines-p0.md`'s documented methodology (release-gating, E11; FRAGO 006 — `ynz build
  --release` is not a CLI flag, main.rs:94-95).
- **Auto-promotion analysis (mandatory per [`auto-promotion.md`](../../../rules/auto-promotion.md)):**
  auto-SoA IS the auto-promotion — the canonical codegen-only case. Stricter/faster form: SoA
  layout; compiler proves fit via `soa_candidate_query` (D3–D5, D7). Surfaces: **codegen
  auto-promotion YES** (measurable runtime benefit); **muted hint NO** (no typeable explicit form —
  the locked exception, Divergence 5); **Tier 3 lint YES** (`array-using-soa-layout`). Muted-hint
  inline render: N/A by design. Hover tooltip: WHAT/WHAT-INSTEAD/WHY per Phase 7 step 1 (jargon-free,
  cites the measured win). Teaching error on failed proof: N/A — there is no user-typeable SoA form
  to fail; declines are silent-correct AoS (decline reasons are internal/test-visible).
  **Override-direction analysis:** force-the-auto-pick — no real use case (the pick is invisible;
  forcing it buys nothing) → deliberate omission; force-the-OTHER-pick (force AoS) — no v0.3 use
  case (FFI is v2+ and handled at the boundary per the scratch doc; testing uses D8's internal env
  var) → deliberate omission, revisit at FFI (FR #8). Neither direction gets user syntax; the
  scratch doc's "compiler picks, lint explains" model is the locked default. The by-value storage
  change itself has no auto-promotion candidate (it is the one representation, not a stricter
  variant) — stated so reviewers know it was considered.

### Teaching

- `array-using-soa-layout` hover follows WHAT/WHAT-INSTEAD/WHY; user-facing text is jargon-free
  (never "struct" / "Struct-of-Arrays" — "separate per-field arrays"; D9), gated by
  `jargon_audit.rs`.
- The WHY is contextual (names the hot loop's lines + fields + the measured win), not generic — per
  Golden Rule 11.
- The lifted guard's old diagnostic disappears cleanly: no stale gallery triggers, no orphaned
  registry deferral, IMP-concurrency rewritten (Phase 3 step 2).
- If D6's contingent reject arms: its diagnostic follows WHAT/WHAT-INSTEAD/WHY with a
  `[[diagnostic_template]]` entry.

### Runtime Dependencies

- By-value arrays: malloc (one buffer allocation — same dependency as today's `YnzArray`, different
  layout). No new runtime dependency.
- SoA arrays: same single malloc (D2 segmented buffer). No scheduler/Tokio dependency — layout is
  static, decided at compile time.
- Benchmark harness: `criterion` as a **dev-dependency only** (MIT/Apache-2.0) — never linked into
  `libynz_rt.a` or shipped binaries.

### Kernel-Mode Behavior

- SoA transform: **disabled in `--kernel` mode** (D1 — analysis entry gate mirrors M4 padding's
  `kernel_mode` gate; recorded decision with revisit trigger FR #7). No new kernel-mode error: the
  transform silently doesn't apply, identical to `--no-auto-parallel`.
- By-value array storage: applies identically in kernel mode — it is the ONE representation, and
  kernel-mode array allocation already requires the user-provided allocator story
  ([`IMP-no-runtime-mode.md`](../../../../docs/internal/implementation/IMP-no-runtime-mode.md));
  the elem_size change alters layout, not the allocator dependency.

### Demo & Error Gallery

- `examples/pirates-roster/entrypoint.ynz`: new large-array hot-loop section (the repo's FIRST
  SoA-qualifying example — authored from scratch, real work in context), byte-exact against the
  regenerated `expected_stdout.txt` golden in `crates/ynz-driver/tests/integration.rs` (never
  `insta` for these two files).
- `examples/primantis-orders/v0_3_m5_errors.ynz`: triggers for any new compile-error class (D6
  contingent); explicit recorded note if zero new classes ship. Old galleries scrubbed of the lifted
  guard's triggers; `error_galleries.rs` counts/phrases updated.
- The suppression enumeration report (Phase 8 step 3) is committed — Patrick's hands-on review
  surface for the filter logic.

### Feature Registry Entries

- **New `[[lint_rule]]`:** `array-using-soa-layout` (rides M4's mechanism; name roadmap-locked).
- **New `[[deferred_tooling_feature]]`:** `dap-soa-unified-view` (four-field, Patrick-signed
  2026-07-03; trigger: DWARF emission ships or a debugging-tooling milestone is scheduled).
- **Retired:** the `array-shape-runtime-field-with-wait` interim deferral entry (guard lifted,
  Phase 3).
- **Contingent `[[diagnostic_template]]`:** heap-field-shape array reject — ONLY if Phase 3's parity
  gate REDs and D6's fallback arms (via FRAGO).
- **Explicitly none:** no new keywords, banned_declaration_keywords, banned_jargon,
  primitive_intrinsics, type_attached_constants, deferred_language_features, and **no new
  muted_hint_domain** (SoA has no typeable form — Divergence 5; deliberate, not forgotten).

## Cross-Cutting Factor Sweep (mandatory factors — addressed or N/A with why)

| Factor | Disposition |
|---|---|
| Security | N/A — compiler-internal layout transform; no new parse surface for untrusted input; no authn/authz/secrets touched. |
| Perf / BigO (mem + cpu) | Core of the milestone — addressed in depth (Performance invariants; E2/E11 rows; linear-scaling analysis pass per the design doc, profiled at P8). |
| Accessibility | N/A — only UI is VSCode hover text, plain text through existing LSP surfaces. |
| PII / privacy | N/A — no user data anywhere in a compiler milestone. |
| Compliance | Addressed minimally — new dev-dep `criterion` is MIT/Apache-2.0, dev-only, never shipped (Sustainment); nothing else changes licensing posture. |
| SEO | N/A — no web surface. |
| Docs | Addressed — Phase 7 docs graduation (IMP-collections, scratch trims, README index), hover text, CHANGELOG, enumeration report. |
| Reusability / DRY | Addressed — ONE array representation (E1), ONE layout authority (E3), ONE elem-size choke point (E7), lint mechanism REUSED from M4 (A2), no parallel ABIs. |
| Type-safety | Addressed — hard-cut ABI makes missed call sites compile errors, not silent drift (E7 B1); typed `SoaCandidate`/`LayoutDecision` artifacts with decline-reason enums. |
| Idempotency | Addressed lightly — compiler passes are pure functions of source (re-run safe by construction); golden-regen script and release steps are re-runnable; no state mutation surfaces. |
| Error-handling | Addressed — `ynz_array_get` out-buffer + has-flag convention replaces the maybe-convention it outgrows; contingent D6 reject follows the diagnostic format; gallery coverage. |
| Observability / logging | Addressed via the teaching surface — the Tier 3 lint IS the observability of the transform decision; decline reasons are test-visible. No runtime telemetry needed: layout is static, nothing to observe at run time. |
| Race / TOCTOU | Addressed — E9 matrix (SoA × wait/background/copy); padding-wins precedence keeps SoA out of cross-thread shapes entirely; atomic-ordering defaults untouched. |
| Resource-cleanup | Addressed — E8 alloc=free parity gate vs baseline; D6 drop-parity decision with a loud-reject fallback; one-allocation designs keep the accounting trivial. |

## 4. Sustainment

- **Build/test env:** Docker `dev` service (docker-compose.yml) for ALL cargo work —
  `docker compose run --rm dev cargo build --workspace` / `cargo test --workspace` /
  `cargo clippy --workspace -- -D warnings`; compiler runs via
  `docker compose exec -T dev ./target/debug/ynz run <fixture>`.
- **Benchmarking:** same container; `criterion` dev-dependency (MIT/Apache-2.0, dev-only); ≥10
  reps; variance recorded per the S3 protocol; no host-native cargo.
- **Test utilities:** `YNZ_NO_AUTO_PARALLEL` (existing), `YNZ_ALLOC_COUNTER_OUTPUT` (existing),
  `YNZ_SOA_FORCE` (new, harness-only, D8).
- **Fixtures/goldens:** byte-exact demo golden via
  `examples/pirates-roster/expected_stdout.txt.regenerate.sh`; `insta` for IR snapshots elsewhere.
- **Executor model (Patrick 2026-07-03, D10):** phase executors dispatch on **Fable 5**
  (`model: fable`) at `/execute-plan` time — explicit availability-override of the frozen binding.
  Reviewer fleet per the frozen model-selection binding, unchanged. Phase Model tags remain the
  classification record/fallback.

## 5. Command & Signal

- **Ownership:** one executor per phase, dispatched by the execution conductor per phase slice;
  Patrick owns the approval gate (`stub → active` flip), all STOP/BLOCKED resolutions, and the
  release sign-off.
- **Succession:** resume from `plan-id: 2026-07-03-v0-3-m5-auto-soa` + the session-id chain +
  checkbox/step state; fat-phase handoffs (P2, P5) via `handoff-phase-<N>.md` in this directory per
  the plan-format convention.
- **Audit trail:** [`audit.md`](./audit.md) (append-only session log + FRAGO log), same directory.

## Future Requirements / Revisit

1. **DAP unified-view over SoA storage** — *what:* debugger shows a unified `Player` view when
   inspecting SoA-laid-out arrays; *why deferred:* zero debug-info/DWARF substrate exists
   (grep-confirmed A4); a bespoke non-DWARF hack is duct tape; DWARF-from-scratch would hold a perf
   milestone hostage to an unrelated subsystem; *cost:* its own multi-session milestone (DWARF
   emission pass + DAP adapter); *trigger:* DWARF emission ships or a debugging-tooling milestone is
   scheduled. **Patrick-signed 2026-07-03.** Registry: `[[deferred_tooling_feature]]
   dap-soa-unified-view` (Phase 7).
2. **SIZE_THRESHOLD provenance (E2, recorded MEDIUM)** — the constant's evidence comes from
   uncontrolled shared hardware; *trigger:* a dedicated perf environment becomes available, OR a
   user-reported perf regression implicates the threshold → re-run the Phase 6 harness and re-derive.
3. **Cross-function SoA propagation (D4 conservatism)** — arrays passed as `share array<Shape>` args
   are declined; *cost:* interprocedural field-access summary analysis; *trigger:* a real workload
   demonstrates the intra-function scope leaves the win on the table.
4. **SoA for grown/pushed arrays (D3)** — *cost:* per-segment re-layout on realloc; *trigger:*
   demand + the growth-re-layout design.
5. **Runtime-length candidates (D3)** — non-provable N declines today; *trigger:* PGO or
   runtime-adaptive layout ever scheduled (roadmap lists PGO as a possible later addition).
6. **Element-aware `ynz_array_drop` (D6)** — element-blind parity holds today; *trigger:* the
   ownership model adds guaranteed heap-owned field freeing (a v0.4+ drop-semantics milestone), OR
   Phase 3's parity gate REDs (immediate FRAGO path).
7. **Kernel-mode SoA (D1)** — disabled; *trigger:* `--kernel` emits real programs and a kernel
   workload wants the layout win.
8. **FFI suppression check becomes real (D7)** — vacuous today; *trigger:* FFI lands (v2+); also
   the force-AoS override question re-opens then (Performance invariants, override analysis).
9. **Mixed-access heuristic (D5)** — ≤2-field union is conservative; *trigger:* workload evidence
   that 3-field loops or per-loop layouts matter.
10. **Retro-validate M3d's cost-threshold constant** with the Phase 6 harness (roadmap.md:350
    suggestion) — *trigger:* post-M5 housekeeping once the harness exists; non-blocking.
11. **E1/E3/E5/E6/E7 recorded MEDIUMs** — standing residuals carried by this plan's gates; *trigger:*
    any dual-mode divergence, second-derivation sighting, or guard regression re-opens the row at
    FRAGO time (re-score per the engine).
12. **Shape-level padding granularity forfeits SoA for ALL of a shape's arrays** — M4's false-sharing
    padding is shape-level (not binding/array-level), so a single spawn-site `.copy()` on ANY array of
    a shape pads the shape and — per D11's padding-wins precedence — forfeits SoA for ALL of that
    shape's arrays (the Phase 5 segment-5 live discovery: even a `.copy()`'d bg-arg pads the
    ORIGINAL's shape). Working-as-designed today, confirmed by Phase 5's boundary review; *cost:*
    binding/array-level padding granularity in the M4 padding analysis + the D11 authority resolution;
    *trigger:* a real workload where this shape-level conservatism measurably costs performance — a
    shape with multiple large hot-loop arrays of which only one crosses a `background` boundary
    (Phase 6's benchmark harness may surface this).
