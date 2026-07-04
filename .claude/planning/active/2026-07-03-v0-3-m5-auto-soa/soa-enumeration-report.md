# Phase 8 step 3 — SoA suppression enumeration report (roadmap.md:156 mandate)

- **Session:** phase8-executor-2026-07-04-m5-seg2 · 2026-07-04 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 8 step 3
- **Tree:** worktree `feat/v0-3-m5-auto-soa` @ `4832ac2` + Phase 8 working-tree edits (demo section in
  `examples/pirates-roster/entrypoint.ynz`). All runs in the worktree Docker `dev` container.
- **Mechanism:** every `.ynz` file under `examples/` (recursive) + `crates/ynz-driver/tests/fixtures/`
  (recursive) was fed through the AUTHORITATIVE `soa_candidate_query`
  (`crates/ynz-typeck/src/queries.rs:730`) — one fresh `CompilerDb` per file, env gates cleared
  (`YNZ_NO_AUTO_PARALLEL` / `YNZ_SOA_FORCE` unset), example-project files registered as full projects
  under absolute canonical paths (the import resolver walks the real filesystem for `yinz.toml`; a
  lone relative registration fails import resolution and yields zero candidates — verified live).
  No second derivation: verdicts below are the query's own rows, per
  [`authoritative-derivation.md`](../../rules/authoritative-derivation.md).
- **Coverage:** **599 files visited, 0 panics.** 44 files carry `array<Shape>` sites → **57
  machine-recorded verdict rows** (55 from the corpus walk + 1 project-dir fixture re-run with its
  sibling registered + 1 scratch probe, below). Every textual `array<[A-Z]…>` grep hit not in the
  table is accounted for in "Sites with no verdict row" below — nothing is silently absent.

## Headline assertions (the mandate's two named checks) — both hold

| Site | Verdict | Correct? |
|---|---|---|
| `examples/pirates-roster/entrypoint.ynz` `volley: array<Cannonball>` (128 elements, hot union {x, y}) | `Admitted { provable_len: 128, hot_fields: ["x", "y"] }` | ✅ — matches the demo's design (128 > 64 strict threshold, 2-field union, no escape/growth) and the `array-using-soa-layout` lint firing exactly once in the real build |
| `examples/pirates-roster/entrypoint.ynz` `crew: array<Pirate>` (66 elements, `recordHit` takes `lend self: Pirate`) | `Declined(LendSelfMethod { function: "recordHit" })` | ✅ — the E6 lend-self suppression filter firing in the demo itself, exactly as the mandate requires |

**Durable pin:** these two verdicts are now locked by
`crates/ynz-typeck/tests/soa_analysis.rs::pirates_roster_demo_volley_admits_and_crew_declines_lend_self`
(filters by binding name, so future demo growth doesn't break the pin). This also answers the
segment-1 UNVERIFIED question: single-file registration does NOT type `entrypoint.ynz` (imports fail
resolution — 27 errors, zero candidates); whole-project registration under absolute canonical paths
types it clean (7 residual diagnostics, ALL Warning/Suggestion severity — the intentional dead-code
teaching section, two `sleepBlocking` suggestions, unused-import warnings; zero errors).

## Full verdict table (all 57 machine-recorded rows)

`diags` = total diagnostics on that file under the harness (galleries/sweeps with intentional
diagnostics noted; every `diags>0` row below is a Warning/Suggestion-bearing sweep fixture that
still compiles).

| File | Binding | Shape | Verdict |
|---|---|---|---|
| `examples/pirates-roster/entrypoint.ynz` | `volley` | Cannonball | **Admitted { provable_len: 128, hot_fields: [x, y] }** |
| `examples/pirates-roster/entrypoint.ynz` | `crew` | Pirate | Declined(LendSelfMethod { recordHit }) |
| `fixtures/m5_p2_byval_array_maybe_elem_write_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_debug_repr.ynz` | `parts` | Part | Declined(Escapes { passed to `print()` (D4) }) |
| `fixtures/m5_p2_byval_field_assign_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_fixed_index_assign_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p2_byval_fixed_maybe_elem_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_fixed_set_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p2_byval_map_index_assign_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_map_lit_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_map_maybe_value_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_map_set_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_maybe_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_maybe_field_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_s1_spike.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_shape_escape_for.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_shape_escape_get.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_shape_escape_wait.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p2_byval_shape_literal.ynz` | `parts` | Part | Declined(Grown { .add() }) |
| `fixtures/m5_p2_byval_shape_runtime.ynz` | `parts` | Part | Declined(Grown { .add() }) |
| `fixtures/m5_p2_byval_struct_lit_field_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p2_byval_struct_map_lit_escape.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 3 }) |
| `fixtures/m5_p3_array_shape_between_waits_runs.ynz` | `items` | Item | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p3_array_shape_nested_if_runs.ynz` | `items` | Item | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p3_array_shape_runtime_field_runs.ynz` | `items` | Item | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p3_e8_parity_gate.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p3_sweep_bg_array_shape_copy.ynz` (diags=2) | `parts` (param) | Part | Declined(Escapes { function parameter (D4) }) |
| `fixtures/m5_p3_sweep_bg_array_shape_copy.ynz` | `parts` | Part | Declined(Escapes { passed to `processParts()` (D4) }) |
| `fixtures/m5_p3_sweep_bg_array_shape_give_wait.ynz` (diags=3) | `items` (param ×2) | Part | Declined(Escapes { function parameter (D4) }) ×2 |
| `fixtures/m5_p3_sweep_bg_array_shape_give_wait.ynz` | `given` | Part | Declined(Escapes { passed to `tallyGiven()` (D4) }) |
| `fixtures/m5_p3_sweep_bg_array_shape_give_wait.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p3_sweep_shape_eq_string_field.ynz` | `tags` | Tag | Declined(BelowSizeThreshold { len: 1 }) |
| `fixtures/m5_p4_soa_both_candidate.ynz` | `season` | Tally | **Admitted { provable_len: 66, hot_fields: [hits, outs] }** — the AUTHORITY (`layout_decisions_query`) then resolves it `Aos { CrossThreadPadded }` (D11 padding-wins; asserted in `soa_analysis.rs::both_candidate_authority_resolves_padding_wins`) |
| `fixtures/m5_p4_soa_escaping.ynz` | `pts` (param + binding) | Point | Declined(Escapes) ×2 — param (D4) + passed to `sumX()` (D4) |
| `fixtures/m5_p4_soa_growth.ynz` | `pts` | Point | Declined(Grown { .add() }) |
| `fixtures/m5_p4_soa_lendself.ynz` | `crew` | Pirate | Declined(LendSelfMethod { recordHit }) — the synthetic twin of the demo's crew case |
| `fixtures/m5_p4_soa_let_shadow.ynz` | `pts` | Point | Declined(Escapes { rebound by a later let }) |
| `fixtures/m5_p4_soa_no_field_access.ynz` | `pts` | Point | Declined(NoPerFieldLoopAccess) |
| `fixtures/m5_p4_soa_qualifying.ynz` | `pts` | Point | **Admitted { provable_len: 72, hot_fields: [x, y] }** |
| `fixtures/m5_p4_soa_reassigned.ynz` | `pts` | Point | Declined(Escapes { reassigned after initial binding }) |
| `fixtures/m5_p4_soa_reassigned.ynz` | `replacement` | Point | Declined(Escapes { stored into another binding (aliases the array) }) |
| `fixtures/m5_p4_soa_runtime_length.ynz` | `out` | Point | Declined(Escapes { returned from the function }) |
| `fixtures/m5_p4_soa_runtime_length.ynz` | `pts` | Point | Declined(LengthNotProvable) |
| `fixtures/m5_p4_soa_small_n.ynz` | `pts` | Point | Declined(BelowSizeThreshold { len: 4 }) |
| `fixtures/m5_p4_soa_threefield.ynz` | `rs` | Reading | Declined(FieldUnionTooWide { [a, b, c] }) |
| `fixtures/m5_p5_copy_aos_independent.ynz` | `a` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p5_copy_aos_independent.ynz` | `b` | Part | Declined(LengthNotProvable) |
| `fixtures/m5_p5_soa_copy_wait_bg.ynz` (diags=2) | `items` (param) | Part | Declined(Escapes { function parameter (D4) }) |
| `fixtures/m5_p5_soa_copy_wait_bg.ynz` | `parts` | Part | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/m5_p5_soa_copy_wait_bg.ynz` | `pts` | Point | **Admitted { provable_len: 72, hot_fields: [x, y] }** |
| `fixtures/m5_p5_soa_copy_wait_bg.ynz` | `snap` | Point | Declined(LengthNotProvable) |
| `fixtures/v0_3_m3a_p3_array_shape_literal_crossing_still_works.ynz` | `items` | Item | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/v0_3_m3a_p3_for_shape_loop_var.ynz` | `items` | Item | Declined(BelowSizeThreshold { len: 2 }) |
| `fixtures/v0_3_m3e_shape_loop_var_direct/entrypoint.ynz` | `entries` | Entry | Declined(BelowSizeThreshold { len: 2 }) — project-dir fixture; re-run with its sibling `io_ops.ynz` registered (diags=0) |

**Verdict-correctness assertion:** every row above was checked against its file's own design intent
(each fixture's `// WHY:` header names the decline class it exists to exercise; the `m5_p4_soa_*`
rows are additionally pinned verbatim by `crates/ynz-typeck/tests/soa_analysis.rs`). Four Admitted
rows exist in the whole corpus — the demo `volley`, the two synthetic qualifying fixtures, and
`season` (admitted at the walk, then correctly overridden to AoS by the padding authority). No row
contradicts its file's intent; no suppression filter failed to fire where its trigger exists.

## Sites with no verdict row — each accounted for

Textual `array<[A-Z]…>` grep hits that produce no `soa_candidate_query` row, with the verified
reason:

| File | Why no row |
|---|---|
| `fixtures/v0_3_m3a_p3_audit_array_crossing.ynz`, `fixtures/v0_3_m3a_p3_audit_fixed_crossing_rejected.ynz` | Comments only — no `array<Shape>` code site. |
| `examples/primantis-orders/m5_errors.ynz` (line 21), `examples/primantis-orders/v0_3_m3a_errors.ynz` (lines 24, 191) | Comments only — no `array<Shape>` code site. Galleries are intentionally non-compiling; neither contains an array-of-shape binding. |
| `fixtures/m5_p3_sweep_union_readback_blocked_array.ynz` (`figs: array<Figure>`, `Figure = Circle \| Square`) | Element type is a UNION — resolves to a non-`Type::Shape` element, structurally outside the admission model by construction (`soa.rs` keys candidates on `Type::Shape` at src/soa.rs:407-408 and the Let arm's oracle lookup; a union has no unified per-element field set to segment). By design, not a miss. The fixture itself is a loud-fail pin (read-back ICEs; no binary is produced). |
| `fixtures/m5_p3_sweep_mapentry_array_escape.ynz` (`saved: array<MapEntry<int, Part>>`) | Element type is a generic builtin instantiation, not a plain `Type::Shape` — same structural filter as above. By design. |
| `fixtures/v0_3_m3a_p3_for_nested_shape_rejected.ynz` (`things: array<Nested> = []`) | Intentional CLEAN-REJECT fixture (nested-shape loop var crossing `wait` must not compile). The reject aborts typing before the binding reaches the expr-types oracle, so no verdict exists — and none should: layout is never applied to a non-compiling file. **Blind-spot hypothesis refuted by probe:** a COMPILING empty-literal `array<Point> = []` with a field-reading loop DOES record — `Declined(BelowSizeThreshold { len: 0 })` (scratch probe, this session) — so the absence is specific to the reject, not an empty-literal hole in the walk. |
| All other `examples/` projects (`incline-watcher`, `stadium-fleet`, `burgh-poem`, remaining `pirates-roster` modules) and remaining 550+ fixtures | Visited by the walk; zero `array<Shape>` sites (no rows, no grep hits). |

## Reproduction

One-shot harness (not committed — this table is the durable record; the demo pin is the durable
assertion): walk both roots for `.ynz`, register each file (plus, for `examples/`, every sibling
file of its containing `yinz.toml` project) into a fresh `CompilerDb` under absolute canonical
paths, run `soa_candidate_query`, dump rows. Wrapped in `catch_unwind` per file; 0 panics across
599 files.
