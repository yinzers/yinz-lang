# P0 audit — exhaustive `ynz_array_*` call-site checklist (feeds Phase 2 exit criteria)

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 4
- **Ticked:** phase2-executor-2026-07-03-m5-seg2 · 2026-07-03 · Phase 2 step 5 — every surface
  migrated to the elem_size-aware hard-cut ABI or explicitly dispositioned (disposition noted per
  line). Pre-cut line anchors retained for provenance; post-cut positions differ (re-anchor by
  enclosing function + op label per the original method note).
- **Method:** `grep -n "ynz_array_"` across the workspace + enclosing-fn attribution against the
  live worktree (fork of `1ac52fd`). Line numbers WILL drift once Phase 2 edits begin — re-anchor
  by enclosing function + op label, not raw line.
- **Usage:** Phase 2's exit criteria tick every box (each surface migrated to the elem_size-aware
  hard-cut ABI or explicitly dispositioned). The E7 grep gate then confirms zero old-signature
  symbols remain.

## A. Runtime ABI — `crates/ynz-runtime/src/lib.rs` (the entry points the hard-cut DELETES/replaces)

- [x] `YnzArray` struct def (uniform-8-byte-slot header) — lib.rs:1100-1105 — REPLACED: gains
      `elem_size: i64`; buffer = `cap * elem_size` bytes, elements inline by value
- [x] `ynz_array_new()` — lib.rs:1112 — HARD-CUT: `ynz_array_new(elem_size: i64)`; header + buffer
      route through counted `ynz_alloc` (FRAGO 005); aborts on `elem_size <= 0`
- [x] `ynz_array_push(arr, i64)` — lib.rs:1145 — HARD-CUT: `push(arr, src: *const u8)` memcpy;
      growth = counted alloc-new + copy + `ynz_free` old (NO raw realloc — counter-invisible)
- [x] `ynz_array_get(arr, idx, out: *mut [i64;2])` — lib.rs:1177 — HARD-CUT:
      `get(arr, idx, out: *mut u8) -> i64` has-flag as RETURN VALUE; OOB zeroes `out` (preserves
      the old `[0,0]` deterministic-bits contract)
- [x] `ynz_array_set(arr, idx, i64)` — lib.rs:1191 — HARD-CUT: `set(arr, idx, src: *const u8)`
      memcpy; OOB abort parity retained
- [x] `ynz_array_count(arr)` — lib.rs:1204 — UNCHANGED (signature + behavior, per D6)
- [x] `ynz_array_drop(arr)` — lib.rs:1213 — SIGNATURE UNCHANGED (D6, element-blind); internals now
      elem_size-aware sized frees through counted `ynz_free` (buffer = `cap * elem_size`, header =
      `size_of::<YnzArray>()`) — the accidental layout-compat is replaced by the real thing
- [x] `ynz_array_clone_primitive(src)` — lib.rs:1239 — SIGNATURE UNCHANGED; elem_size carried from
      src header; byte-copy `len * elem_size`; both allocations counted (`ynz_alloc`) so the
      task-exit drop stays in parity
- [x] `ynz_string_split(...) -> *mut YnzArray` — lib.rs:1802 — MIGRATED: `ynz_array_new(8)`
      (string = pointer cell, Option A); pushes stage the pointer bits in a local i64 and pass
      `*const u8`
- [x] **Scheduler-internal consumer:** `crates/ynz-runtime/src/runtime.rs:634` — UNCHANGED BY
      CONSTRUCTION: `ynz_array_drop` kept its signature; the BgArgDropEntry kind=1 arm and the
      descriptor size=0 contract are untouched
- [x] (tests) runtime unit tests — lib.rs:2663-2691 + `string_split_basic` lib.rs:2603-2616 —
      UPDATED to the new ABI; ADDED `array_multibyte_elements_stored_by_value` (16-byte elements:
      push/set/get round-trip, source-clobber independence, OOB zeroing, growth survival)

## B. LLVM decl surface — `crates/ynz-codegen/src/runtime_decls.rs`

- [x] Struct fields `ynz_array_{new,push,get,set,count,drop,clone_primitive}` —
      runtime_decls.rs:51-61 — comments updated to the by-value ABI + choke-point pointer
- [x] `declare_fn` signatures — runtime_decls.rs:367-397 — HARD-CUT: `new(i64)->ptr`,
      `push(ptr,ptr)->void`, `get(ptr,i64,ptr)->i64`, `set(ptr,i64,ptr)->void`;
      count/drop/clone unchanged. IR snapshot churn verified decl-only (step 4)

## C. Codegen call sites — `crates/ynz-codegen/src/emit.rs`

All element loads/stores now route through the `Cg::array_elem_*` choke-point section
(single `array_elem_size` derivation reading `shape_abi_sizes`, threaded into `Cg` as a
non-Option field at all 3 construction sites incl. a new `lower_generic_function` param).

### Construction
- [x] `Expr::ArrayLit` lowering: `ynz_array_new` @13056-13060 + per-element `ynz_array_push`
      @13136 — MIGRATED to `array_new` + `array_elem_push`; SM decimal-global fold KEPT (number
      elements stay pointer cells); SM SHAPE-global fold DELETED (buffer owns the bytes);
      explicit `BuiltinArray` arm + error arm replace the old `_` fallback

### Element read
- [x] `Expr::IndexAccess` array branch: `ynz_array_get` @13233 — MIGRATED to
      `array_elem_get_maybe` (envelope re-packed `{flag, bits}`; shape bits = out-buffer ptr)
- [x] `lower_array_method "get"` @16219 — MIGRATED to `array_elem_get_maybe`
- [x] `lower_array_method "first"` @16235 — MIGRATED to `array_elem_get_maybe`
- [x] `lower_array_method "last"` @16246/16262 — count call unchanged; get MIGRATED to
      `array_elem_get_maybe`
- [x] `lower_array_method "contains"` @16288/16343 — MIGRATED: non-shape = full-width-cell bit
      equality (target widened via `array_elem_bits64` — fixes the raw-i1 bool compare); shape =
      field-wise VALUE equality via `shape_value_eq` (recorded decision; locked by the 2 RED
      matrix cells; per-field GEP loads, never a padded-bytes memcmp)

### Element write / growth
- [x] `Stmt::IndexAssign` array branch: `ynz_array_set` @11024 — MIGRATED to `array_elem_set`;
      bits marshal moved INTO the map/fixed branches (map keeps uniform-slot ABI until P3/E12;
      fixed<T> keeps its stack [N x i64] layout — both explicitly out of the array cut)
- [x] `lower_array_method "set"` @16277 — MIGRATED to `array_elem_set`
- [x] `lower_array_method "add"` @16207 — MIGRATED to `array_elem_push`

### Length
- [x] `lower_array_method "count"` @16195-16199 — UNCHANGED (D6; not part of the elem-size ABI)

### For-loop element reads (BOTH paths — the M3a twin-dispatch corpse lives here)
- [x] Non-SM for-loop: count @11518 + get @11562 — get MIGRATED to entry-block out-buffer +
      `array_elem_get_into` + `array_elem_from_out` (loop var = per-iteration byte COPY)
- [x] SM (suspension) for-loop: count @5320 + get @5342 — get MIGRATED; the shape-embed
      special-case (manual size_of + memcpy from a heap element pointer, @5370-5403) DELETED —
      the by-value get writes element bytes DIRECTLY into the pre-wired frame region (the
      out-pointer IS the destination); non-embed path uses the standard out-buffer + flush

### Background / drop integration
- [x] `prepare_bg_arg_for_ctx`: `ynz_array_clone_primitive` @13680 — UNCHANGED BY CONSTRUCTION
      (signature-stable; clone reads elem_size from the src header)
- [x] `emit_bg_arg_frees`: `ynz_array_drop` @13776 — UNCHANGED BY CONSTRUCTION (signature-stable)
- [x] `lower_sm_background_spawn` free-descriptor emission (kind=1, size=0 contract)
      @14162/14178/14257 — UNCHANGED BY CONSTRUCTION (drop knows its own sizes from the header;
      the size=0 descriptor contract holds)

### Debug/format surface
- [x] Array debug representation: count @15233-15237 + get @15303-15310 — get MIGRATED to the
      choke point (out-buffer + `array_elem_from_out`); TESTED by `m5_p2_byval_debug_repr`
      (P2-boundary fix-loop: fixture `m5_p2_byval_debug_repr.ynz` prints per-type arrays incl.
      a shape array against a byte-exact golden — the site was ticked with no test exercising it)

### Deleted with the cut
- [x] `try_build_shape_global` @18126 — DELETED (fn + doc comment; tombstone NOTE left in place);
      stale mirror-claim comment on typeck's `expr_is_compile_time_literal` (check.rs:8094-8100)
      rewritten to point at the P3 guard-lift

## D. Out-of-scope references confirmed comment-only (no code to migrate)

- [x] `crates/ynz-typeck/src/check.rs` — zero `ynz_array_*` code references (comment-only;
      the `ArrayShapeRuntimeFieldWithWait` guard itself stays until P3 step 2's lift)
- [x] `crates/ynz-driver/tests/integration.rs` comments — tests kept green (481/481); the six
      alloc-count assertions written against the counter-blind world updated to the FRAGO 005
      counted reality (exact new constants, invariants preserved — see phase return)

## Grep gate (E7 proof obligation, run at Phase 2 exit)

**RUN 2026-07-03 (Phase 2 step 5): PASS.**
- Zero old-signature `ynz_array_*` decls/symbols (no no-arg `ynz_array_new()`, no
  `out: *mut [i64; 2]` on any array symbol — remaining `[i64;2]` hits are map/string surfaces,
  P3/E12 scope).
- Raw `rt.ynz_array_{new,push,get,set}` calls appear ONLY inside the `Cg::array_elem_*`
  choke-point section (emit.rs ~2179/2224/2241/2282 — `array_new`, `array_elem_push`,
  `array_elem_set`, `array_elem_get_into`).
- `fn try_build_shape_global` absent from the tree (comment tombstones only).
