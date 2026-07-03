# P0 audit — exhaustive `ynz_array_*` call-site checklist (feeds Phase 2 exit criteria)

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 4
- **Method:** `grep -n "ynz_array_"` across the workspace + enclosing-fn attribution against the
  live worktree (fork of `1ac52fd`). Line numbers WILL drift once Phase 2 edits begin — re-anchor
  by enclosing function + op label, not raw line.
- **Usage:** Phase 2's exit criteria tick every box (each surface migrated to the elem_size-aware
  hard-cut ABI or explicitly dispositioned). The E7 grep gate then confirms zero old-signature
  symbols remain.

## A. Runtime ABI — `crates/ynz-runtime/src/lib.rs` (the entry points the hard-cut DELETES/replaces)

- [ ] `YnzArray` struct def (uniform-8-byte-slot header) — lib.rs:1100-1105
- [ ] `ynz_array_new()` — lib.rs:1112
- [ ] `ynz_array_push(arr, i64)` — lib.rs:1145
- [ ] `ynz_array_get(arr, idx, out: *mut [i64;2])` — lib.rs:1177
- [ ] `ynz_array_set(arr, idx, i64)` — lib.rs:1191
- [ ] `ynz_array_count(arr)` — lib.rs:1204
- [ ] `ynz_array_drop(arr)` — lib.rs:1213 (element-blind today — E8's leak class; S1 note: byval
      header layout-compat is accidental, ship a real elem_size-aware drop)
- [ ] `ynz_array_clone_primitive(src)` — lib.rs:1239 (background arg-copy for `array<primitive>`)
- [ ] `ynz_string_split(...) -> *mut YnzArray` — lib.rs:1802; INTERNAL constructor uses
      `ynz_array_new`/`ynz_array_push` at lib.rs:1809/1816/1821 (string arrays = pointer
      elements; must stay correct under the new ABI or keep a string-elem path)
- [ ] **Scheduler-internal consumer:** `crates/ynz-runtime/src/runtime.rs:634` —
      `crate::ynz_array_drop(heap_ptr)` in the background-arg free path (HeapArrayPrimitive
      descriptor kind; see runtime.rs:458-464 comment contract + emit.rs:14162/14178/14257
      descriptor emission)
- [ ] (tests) runtime unit tests construct arrays directly — lib.rs:2663-2691 (`#[cfg(test)]`,
      update alongside the ABI); PLUS `string_split_basic` — lib.rs:2603-2616, which calls
      `ynz_array_count`/`ynz_array_get`/`ynz_array_drop` directly on `ynz_string_split`'s result
      (`#[cfg(test)]`, same ABI-update obligation)

## B. LLVM decl surface — `crates/ynz-codegen/src/runtime_decls.rs`

- [ ] Struct fields `ynz_array_{new,push,get,set,count,drop,clone_primitive}` — runtime_decls.rs:51-61
- [ ] `declare_fn` signatures for all seven — runtime_decls.rs:367-397 (hard-cut: signatures gain
      `elem_size`/byte-pointer params; a missed site then fails to compile — E7 B1)

## C. Codegen call sites — `crates/ynz-codegen/src/emit.rs`

### Construction
- [ ] `Expr::ArrayLit` lowering: `ynz_array_new` @13056-13060 + per-element `ynz_array_push`
      @13136 (in `lower_expr`; the padded-shape const-global-fold skip precedent lives in this arm)

### Element read
- [ ] `Expr::IndexAccess` array branch: `ynz_array_get` @13233 (maybe-envelope result)
- [ ] `lower_array_method "get"`: `ynz_array_get` @16219
- [ ] `lower_array_method "first"`: `ynz_array_get` @16235
- [ ] `lower_array_method "last"`: `ynz_array_count` @16246 + `ynz_array_get` @16262
- [ ] `lower_array_method "contains"`: `ynz_array_count` @16288 + loop `ynz_array_get` @16343

### Element write / growth
- [ ] `Stmt::IndexAssign` array branch: `ynz_array_set` @11024 (in `lower_stmt`)
- [ ] `lower_array_method "set"`: `ynz_array_set` @16277
- [ ] `lower_array_method "add"`: `ynz_array_push` @16207

### Length
- [ ] `lower_array_method "count"`: `ynz_array_count` @16195-16199

### For-loop element reads (BOTH paths — the M3a twin-dispatch corpse lives here)
- [ ] Non-SM for-loop: `ynz_array_count` @11518 + `ynz_array_get` @11562 (in `lower_stmt_for`)
- [ ] SM (suspension) for-loop: `ynz_array_count` @5320 + `ynz_array_get` @5342 (in
      `lower_sm_for`; frame-region element copy contract per comments @5370/5387)

### Background / drop integration
- [ ] `prepare_bg_arg_for_ctx`: `ynz_array_clone_primitive` @13680
- [ ] `emit_bg_arg_frees`: `ynz_array_drop` @13776
- [ ] `lower_sm_background_spawn` free-descriptor emission (kind=1 → `ynz_array_drop`, size=0
      contract) @14162/14178/14257

### Debug/format surface
- [ ] Array debug representation (toString/print path): `ynz_array_count` @15233-15237 +
      `ynz_array_get` @15303-15310 (capped-at-20 debug repr)

## D. Out-of-scope references confirmed comment-only (no code to migrate)

- `crates/ynz-typeck/src/check.rs` — zero `ynz_array_*` references (map-iter comments only)
- `crates/ynz-driver/tests/integration.rs:863/1177/4644/5204` — comments in tests (the tests
  themselves are behavior gates Phase 2 must keep green, not call sites)

## Grep gate (E7 proof obligation, run at Phase 2 exit)

Zero matches for old-signature decls/symbols; zero raw `ynz_array_get/set/push` calls outside the
choke-point helper pair Phase 2 introduces.
