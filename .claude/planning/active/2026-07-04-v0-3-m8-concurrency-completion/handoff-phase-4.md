# Handoff — Phase 4, segment 1 (the RED seal)

**Dispatch:** `m8-p4-20260904-a1` · **Written:** 2026-09-04 · **Tree at write time:** `96ce515` + the
uncommitted RED set below · **Resume at:** `phase-4/step-2` (the conductor seals this segment as the
RED commit first; the next dispatch implements against it in the order under "Sequencing").

## What this segment did

Authored every RED fixture the phase's steps 3a/5 (and 3d's fixture class, and the SIGSEGV
precondition) call for, wired one named test per fixture, extended the m8 error gallery with a
trigger per new diagnostic, ran the set once in the dev container (foreground, exit observed), and
recorded each failure's reason. **No compiler/runtime/registry code was touched.** Nothing is
committed — the conductor seals.

Run: `docker compose run --rm dev cargo test -p ynz-driver --no-fail-fast --test v03_m8_channel_close --test error_galleries m8`
→ exit 101; **23 FAILED, 1 ok** in `v03_m8_channel_close`; `m8_gallery_fires_expected_diagnostics` FAILED.

## The RED fixtures and their observed failure (all under `crates/ynz-driver/tests/fixtures/`)

Test file: `crates/ynz-driver/tests/v03_m8_channel_close.rs` (one `#[test]` per fixture; helpers local
to the file on the `fr23_uaf_planned_red.rs` precedent — `ynz_run`, `ynz_run_with_alloc_counter`,
120s liveness watchdog).

| Fixture | Step | Observed RED (first diagnostic / signal) | Right reason? |
|---|---|---|---|
| `v0_3_m8_p4_map_copy_independent.ynz` | 3a | ran; stdout `copy b: 99` / `copy count: 3` (expected `20` / `2`) — the alias no-op observed the mutation | yes — the stub the step must change |
| `v0_3_m8_p4_close_drain_then_none.ynz` | 3 | `` `channel<int>` does not have a method called `close`. `` then `` `int` does not have a method called `exists`. `` | yes |
| `v0_3_m8_p4_close_double_idempotent.ynz` | 3 | unknown `close` (×3), `exists` on `int` | yes |
| `v0_3_m8_p4_send_after_close_refused.ynz` | 3/4 | unknown `close`; `exists` on `int` | yes |
| `v0_3_m8_p4_close_concurrent_send_linearized.ynz` | 5 | unknown `close`; `exists` on `int` (the `!=`/`==` on ints parsed fine) | yes |
| `v0_3_m8_p4_close_wakes_parked_receiver.ynz` | 3 | unknown `close`; `exists` on `int` | yes |
| `v0_3_m8_p4_close_inflight_send_lands.ynz` | 3 | unknown `close`; `exists` on `int` | yes |
| `v0_3_m8_p4_drop_without_close_unchanged.ynz` | 5 | `exists` on `int` only (no close in the file) | yes — RED purely on the retype |
| `v0_3_m8_p4_handle_maybe_return.ynz` | 3 (precondition) | `RUNTIME ERROR: the program was killed by signal 11 (SIGSEGV).`, exit 139 | yes |
| `v0_3_m8_p4_handle_maybe_errors_return.ynz` | 3 (precondition twin) | **GREEN today** (`7` / `done`) — a lock, not RED; recorded so nobody "fixes" it into red | n/a |
| `v0_3_m8_p4_chan_array_roundtrip.ynz` | 5 | unknown `close`; `exists` on `array<int>`; `` `array<int>` values do not have fields `` | yes |
| `v0_3_m8_p4_chan_array_send_after_close.ynz` | 4/5 | unknown `close`; `exists` on `array<int>` | yes |
| `v0_3_m8_p4_chan_array_drop_with_buffered.ynz` | 5 | unknown `close` | yes |
| `v0_3_m8_p4_chan_map_roundtrip.ynz` | 5 | unknown `close`; `exists` on `map`; `Cannot use .value to look up a map key` (the pre-retype `.value` arm on a map receiver) | yes |
| `v0_3_m8_p4_chan_map_send_after_close.ynz` | 4/5 | unknown `close`; `exists` on `map` | yes |
| `v0_3_m8_p4_chan_map_drop_with_buffered.ynz` | 5 | unknown `close` | yes |
| `v0_3_m8_p4_flow_give_through_call.ynz` | 5 | unknown `close`; `exists` on `array<int>` | yes |
| `v0_3_m8_p4_flow_two_hop_give.ynz` | 5 | unknown `close`; `exists` on `array<int>` | yes |
| `v0_3_m8_p4_flow_admitted_forms.ynz` | 5 | unknown `close`; `exists` on `array<int>` | yes |
| `v0_3_m8_p4_revive_on_reassign.ynz` | 3b/5 | `` `rows` was already given away and cannot be used here. `` (×2) — today's false error on a correct program | yes |
| `v0_3_m8_p4_number_chan_roundtrip.ynz` | 3d | `` `channel<number>` is not supported yet — this element type cannot cross a task boundary. `` (+ unknown `close`) | yes |
| `v0_3_m8_p4_number_chan_send_after_close.ynz` | 3d | same construction rejection (+ unknown `close`) | yes |
| `v0_3_m8_p4_number_chan_drop_with_buffered.ynz` | 3d | same | yes |
| `v0_3_m8_p4_number_chan_parked_send_drained_after_close.ynz` | 3d | same | yes |

**Gallery:** `examples/primantis-orders/m8_errors.ynz` gained the "v0.3-M8 Phase 4" section (25 trigger
functions + 4 shapes/helpers, each with `// WHY:`); `crates/ynz-driver/tests/error_galleries.rs`'s
`m8_gallery_fires_expected_diagnostics` now asserts 26–36 errors and 13 key phrases. Observed today:
**15 errors** (the file parses cleanly — no `Unexpected` lines — so typeck ran over all of it), failing
on the count and on every new phrase. Worth reading in that output: the seven 2026-09-03 probe holes
are confirmed live on this tree (`m8_ufcs_non_receiver_give`, the three alias forms, `m8_consumed_by_send_*`
produce NO error today), and the old share-refusal text (`is declared share (read-only); m8_eat needs
to take ownership`) fires for `m8_param_needs_give_share` — that is the string `ParamNeedsGive` retires.

## The SIGSEGV precondition — root cause (probed, not yet fixed)

Reproduced with a `-> maybe<int>` function (`wait sleep(10)`, `return arr.get(0)`) spawned on the handle
form and consumed via `h.receive()` inside an `errors` function: signal 11 every run. The same body
consumed via `wait pick()` prints `7`; the `-> maybe<int> errors` twin on the handle form prints `7`.

Producer: `crates/ynz-codegen/src/emit.rs` `lower_let_background_handle` — its `ret_kind` match has no
arm for `maybe<T>` (nor `union`/`dynamic`), so a plain `-> maybe<T>` falls to
`HANDLE_RET_KIND_VALUE_WORD` ("an i64-slot value"). A `maybe<T>` is a `{tag, payload}` aggregate
whose storage is the task's frame/stack; the parent receives its address after the task retired. The
`number` case already has the right shape of fix (`HANDLE_RET_KIND_VALUE_NUMBER` copies the 16 bytes
into a handle-owned buffer before the frame is freed — `ynz-abi/src/lib.rs:84–88`,
`handle.rs::extract_completion`). Fix at the producer: a maybe-aware completion kind (or widen the
NUMBER-style copy to every non-word aggregate return, sized from the type), classified in the ONE
`ret_kind` match, mirrored in `extract_completion`, with the `_ => VALUE_WORD` fallthrough no longer
silently admitting an aggregate. The errors-twin's "works today" is the ok-word surviving by accident
— confirm it goes through the same copy so it stays correct by construction, not by luck.

## Sequencing for the implementation segment (per the dispatch brief)

3a `ynz_map_clone` + `Type::BuiltinMap` copy arm + registry `copy` on `map` (greens
`map_copy_independent`) → 3d `ChannelElemDrop { Array, Map, NumberCell }` + `transfers_source()` +
`number_to_heap_cell` send / free-at-receive + `ynz_number_cell_free` + `elem_supported` derived from
it (greens the four `number_chan_*`) → 3b the transfer rule (hoist fixpoint; `provenance()`; binding
events; `check_transfer`; three diagnostics; `consumed: Option<ConsumedBy>`; `ContractSigDef`
modifiers; `root_binding_name` twin collapse, parked 27; `Consumed` template parity test, parked 7/8)
→ 2/3 `.close()` at the three typeck sites + runtime `ynz_channel_close` + `receive()` → `maybe<T>`
with `alloca_in_entry_llvm` (4b) + the 19-site/13-fixture rewrite + demo/gallery/spec + the SIGSEGV
fix → 3c `HandleChannelArgNeedsBinding` → 4 `refuse_closed` + `bg_arg_kind_is_releasable_payload` +
`ALL_BG_ARG_KINDS` + parity test + flip of the runtime doc comment and case (c) → loom models
(close-vs-send at the sender-lock clone; `refuse_closed` release+glue) in `loom_tests.rs` → 6/7 docs +
registry → 8 the named suite. Stop at the plan's CHECKPOINT (after 4b) if the segment runs long.

## Things the next segment must know (found while authoring; none silently absorbed)

1. **Design doc `IMP-concurrency.md` "receive() on a bare channel becomes maybe<T>" shows the
   consumer loop with a standalone `} else {` block; Yinz has no standalone `else`**
   (`REF-control-flow.md:34,153` — only `else =>` inside multi-case `if`). Every fixture here uses
   `stillOpen = next.exists()` after the `if`. The spec's "Closing a channel" subsection and the demo
   must use a real form; the IMP sketch should be corrected in the docs step (6/7).
2. **`IMP-ownership.md`'s provenance table lists "constructor call (`array<T>()`, `map<K, V>()`,
   `channel<T>()`)" as `Fresh`; `array<int>()` / `map<..>()` are not forms the parser accepts**
   (zero occurrences in fixtures/docs; `channel<T>()` is). `flow_admitted_forms` uses `[]` bound to a
   binding for the empty container. `provenance()` still needs the `channel<T>()` constructor row; the
   array/map constructor rows are dead unless the parser gains them — say so in the doc, don't invent
   the form.
3. **A pre-existing typeck over-approximation blocks some natural `maybe<T>` consumer shapes**:
   `collect_crossings_in_stmts` (`check.rs`, the `past_wait` non-suspending arm) pushes every `let`
   after a suspension into `declared` and then flags ANY later read of it as a crossing — even with no
   further suspension. Harmless for ints (a frame slot); for `maybe<T>` it is the hard
   `UnsupportedCrossingLocalType` rejection. Probe: `let r = h.receive(); let fallback: maybe<int> =
   none; let m = r.or(fallback)` is refused "cannot yet cross a wait" though nothing suspends after
   `fallback`. The retyped `let next = wire.receive()` inside a loop body is fine (a pending
   result-binding), and every fixture here is written around the over-approximation; but the demo/spec
   rewrite (19 sites) will meet it wherever a received `maybe<T>` is bound after a prior suspension
   and read later. Surface it in the report; fixing the analysis is out of this step's scope unless a
   rewrite cannot route around it.
4. **`r.or(none)` does not type-check** ("Cannot work out which type `none` should be here") even under
   a `let m: maybe<int> = …` annotation — the handle-form fixtures use `.exists()`/`.value` inside an
   `errors` function (auto-propagation narrows `h.receive()` to `maybe<int>`) instead.
5. **Alloc/free gap constants in `v03_m8_channel_close.rs` are derivations, not observations** (they
   cannot be observed until the fixtures compile): array = 2 counted allocs (header + data), map = 5
   (header + ctrl + keys + vals + insert_order; `.set` growth nets to zero), number cell = 1; channel
   objects and SM frames net to zero (the `bg_arg_channel_send_array` precedent: gap 4 = two arrays).
   Unknowns to confirm at GREEN: whether the bound `let sent = wire.send(..)` error value on the
   refused path is a counted alloc that is never freed (if so, the send-after-close gaps become 1, and
   that is a named allocation to record, not a silent adjustment); whether an empty map literal `{}`
   allocates zero-size buffers through counted `ynz_alloc` (assumed yes: 5). Any gap that differs
   from the derivation must be explained by a named allocation in the test's `gap_explained`.
6. **The drop-with-buffered fixtures observe "held by the never-torn-down channel", not the teardown
   glue** — the spawner's channel reference is never released (no scope-exit drop), so `YnzChannel::drop`
   is unreachable from source. The Drop-glue path stays the runtime crate's unit gate.
7. **`cross_impl_consistency.rs` sweeps every fixture**: at RED all new fixtures fail identically in
   both modes/runs (compile error or deterministic SIGSEGV), so the sweep stays consistent; after
   GREEN none of them should need an exclusion (no fixture name contains `background`/`concurrent`/
   `timing`; the concurrent-send fixture prints only interleaving-independent invariants).
8. **Gallery count bound 26–36 is an estimate**; pin it to the observed GREEN count ± the same slack
   the other galleries use, and add `m6_errors.ynz:112–115`'s second diagnostic + `// WHY:` (parked
   17) and `v0_3_m4_errors.ynz:98`'s `channel<number>` → `channel<Player>` swap (3d) in the same
   round. The retired share-refusal text (`is declared \`share\` (read-only); … needs to take
   ownership`) is asserted nowhere by phrase today (checked `error_galleries.rs`); `insta` snapshots
   under `crates/ynz-driver/tests/snapshots/` should be grepped for it before the retirement lands.
9. **Runtime unit tests for `refuse_closed` / `ynz_channel_close` are NOT authored RED** — they would
   not compile (the symbols don't exist), which takes the whole `ynz-runtime` test target down with
   them and hides Phase 3's loom suite. They land with the implementation, alongside the flip of test
   case (c) of `ladder_is_untouched_when_the_channel_does_not_take_ownership`.
