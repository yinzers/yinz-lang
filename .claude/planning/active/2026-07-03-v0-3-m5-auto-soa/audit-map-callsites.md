# P0 audit — exhaustive `ynz_map_*` call-site checklist (feeds Phase 3 step 1 ENTRY criteria — E12)

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 4
- **Method:** `grep -n "ynz_map_"` across the workspace + enclosing-fn attribution against the
  live worktree (fork of `1ac52fd`). Re-anchor by enclosing function + op label once lines drift.
- **Usage:** Phase 3 step 1 ticks every box for the `map<K,Shape>` symmetric hard-cut (E12's
  audit precondition — the SAME missed-call-site class as E7 but on the map surface, which the
  array-only grep gate does NOT cover).

## A. Runtime ABI — `crates/ynz-runtime/src/lib.rs`

- [x] `YnzMap` struct def — lib.rs:594; `ynz_map_new()` — lib.rs:674 (values are uniform i64 *(seg-4 tick: elem_size ABI landed lib.rs:594-617; abort-on-<=0 new; fixtures m5_p3_mapshape_literal_str / m5_p3_mapshape_runtime_int prints 1-8 verified)*
      slots — the same pointer-element pattern as `YnzArray`, pre-existing miscompile class for
      shape values)
- [x] `ynz_map_get(map, i64 key, out: *mut [i64;2])` — lib.rs:907 *(seg-4 tick: flag-return memcpy contract; m5_p3_mapshape_runtime_int get(100)/D13 snapshot verified)*
- [x] `ynz_map_get_str(map, *const u8 key, out)` — lib.rs:923 *(seg-4 tick: m5_p3_mapshape_literal_str get(`a`)/get(`d`) verified)*
- [x] `ynz_map_set(map, i64 key, i64 value)` — lib.rs:946 *(seg-4 tick: src-ptr memcpy; m5_p3_mapshape_runtime_int set/re-set-over-key + m5_p2_byval_map_set_escape)*
- [x] `ynz_map_set_str(map, *const u8 key, i64 value)` — lib.rs:973 *(seg-4 tick: m5_p3_mapshape_literal_str set(`a`,pc) overwrite verified)*
- [x] `ynz_map_count(map)` — lib.rs:1017 *(seg-4 tick: unchanged ABI; both fixtures count pre/post-insert verified)*
- [x] `ynz_map_has(map, i64 key)` — lib.rs:1026 *(seg-4 tick: m5_p3_mapshape_runtime_int has(100)/has(999) verified)*
- [x] `ynz_map_iter_get(map, pos, out: *mut [i64;3])` — lib.rs:1040; NOTE internal delegation to
      `ynz_map_get` at lib.rs:1047 (single-choke-point precedent already half-present) *(seg-5 tick: cut ABI at lib.rs:1016-1031 — split key_out/val_out, flag-return, delegates to ynz_map_get; verified end-to-end by m5_p3_mapshape_runtime_int + m5_p3_mapshape_wait_iter (int-key iteration, both arms) 6/6 green)*
- [x] `ynz_map_iter_get_str(map, pos, out)` — lib.rs:1060 *(seg-5 tick: cut ABI at lib.rs:1049-1077 — pointer-identity key scan, elem_size memcpy; verified by m5_p3_mapshape_literal_str + m5_p3_mapshape_iter_escape + m5_p3_mapshape_wait_cross (str-key iteration, both arms) 6/6 green)*
- [x] `ynz_map_drop(map)` — lib.rs:1086 (element-blind — same E8 leak-class question as *(seg-4 tick: five counted frees; accounting pinned by seg-3 m3d +10 alloc-gap re-pins — maps never dropped locally by design)*
      `ynz_array_drop`; dispositioned by the P3 design)

## B. LLVM decl surface — `crates/ynz-codegen/src/runtime_decls.rs`

- [x] Struct fields `ynz_map_{new,get,get_str,set,set_str,count,has,iter_get,iter_get_str,drop}` *(seg-4 tick: retyped seg-3; 34/34 golden IR snapshots verified as decl churn)*
      — runtime_decls.rs:65-79
- [x] `declare_fn` signatures for all ten — runtime_decls.rs:402-439 *(seg-4 tick: same golden-snapshot verification)*

## C. Codegen call sites — `crates/ynz-codegen/src/emit.rs`

### Construction
- [x] `Expr::MapLit` lowering: `ynz_map_new` @13338-13342 + per-entry `ynz_map_set_str` @13353 / *(seg-4 tick: map_new(val_ty)+map_val_set; m5_p3_mapshape_literal_str construction + m5_p2_byval_map_lit_escape green)*
      `ynz_map_set` @13363
- [x] `Expr::StructLit`-with-map-annotation lowering (`{ key: value }` under a `map<...>` *(seg-4 tick: m5_p3_mapshape_runtime_int `{}` empty-lit + m5_p2_byval_struct_map_lit_escape green)*
      annotation): `ynz_map_new` @12984-12988 + `ynz_map_set_str` @12997

### Element read
- [x] `Expr::IndexAccess` map branch: `ynz_map_get_str` @13167 / `ynz_map_get` @13177 *(seg-4 tick: ONE map_val_get_maybe; [`b`]/[`a`]/[200]/[100] cells verified in both fixtures)*
      (string-key vs int-key split; maybe-envelope result)
- [x] `lower_map_method "get"`: `ynz_map_get_str` @17017 / `ynz_map_get` @17027 *(seg-4 tick: map_val_get_maybe; get cells + m5_p3_mapshape_wait_get_escape GREEN across wait)*
- [x] `lower_map_method "has"`: `ynz_map_get_str` @16977 (string-key path) / `ynz_map_has` *(seg-4 tick: get-flag-truncate str path / map_has_int; has cells verified)*
      @16993 (int-key path)

### Element write
- [x] `Stmt::IndexAssign` map branch: `ynz_map_set_str` @11002 / `ynz_map_set` @11012 (in *(seg-4 tick: map_val_set; [`d`]=pd / [300]=pc inserts + m5_p2_byval_map_index_assign_escape green)*
      `lower_stmt`)
- [x] `lower_map_method "set"`: `ynz_map_set_str` @17079 / `ynz_map_set` @17089 *(seg-4 tick: map_val_set; set cells + m5_p2_byval_map_set_escape green)*

### Length
- [x] `lower_map_method "count"`: `ynz_map_count` @16958 *(seg-4 tick: map_count_val; count cells verified)*

### For-loop entry iteration (BOTH paths — same twin-dispatch hazard as arrays)

> **SEG-5: FIXED + TICKED.** Root cause (Paper-Traced in seg-5 return): an indirection-level
> mismatch on the MapEntry LOCAL SLOT contract — `load`/`store`/`materialize_param`
> (emit.rs:18876/:18709/:11659-11666) all treat a MapEntry slot as HOLDING A POINTER to the
> {i64,i64} entry struct, but both loop arms registered the struct ALLOCA ITSELF as the local,
> so `load`'s `build_load(ptr, slot)` reinterpreted field 0 (key_bits) as the struct pointer —
> `entry.value` then read 8 bytes past the key cstring (garbage-not-zero for scalars; deref'd
> → SIGSEGV for shapes). Neither the runtime nor the choke points were wrong (matches seg-4's
> eliminated-suspects list). Fix: both arms now register a pointer-indirect slot per the
> canonical `materialize_param` pattern. RED lock released: 6/6 m5_p3_mapshape green,
> workspace suite 2235 passed / 0 failed.
- [x] Non-SM map for-loop: `mf_*` arm, emit.rs ~:12697-12820 (in `lower_stmt_for`) — routes
      via map_count_val + map_iter_get_into choke points *(seg-5 tick: entry_var_slot
      pointer-indirect fix; m5_p3_mapshape_literal_str / _runtime_int / _iter_escape green —
      incl. the step-5(b) MapEntry-aliasing escape cell)*
- [x] SM (suspension) map for-loop: `sm_mf_*` arm, emit.rs ~:6564-6675 (in `lower_sm_for`) —
      same choke points *(seg-5 tick: same pointer-indirect fix, entry-hoisted allocas,
      per-body-bb re-store; m5_p3_mapshape_wait_iter (body wait) + _wait_cross (pre-loop
      wait, SM arm) green; Check 2c post-wait entry-read rejection preserved)*

### Debug/format surface
- [x] Map debug representation (toString/print path): `mdbg_*` walker, emit.rs ~:16300-16435 —
      routes via map_count_val + map_iter_get_into + array_elem_from_out (choke-point-clean;
      does NOT route through the MapEntry local slot, so it never shared the loop-arm bug)
      *(seg-5 tick: VERIFIED not retired — reachable via shape-embedded map fields; NEW fixture
      m5_p3_map_embed_repr.ynz + integration test lock it, covering scalar AND shape-valued
      (elem_size>8) cells: `Depot { label: strip, stock: { a: 5, b: 6 }, parts: { pa: Part
      { qty: 1, price: 10 } } }` — green)*

## D. Non-code references (contract notes, not call sites)

- `crates/ynz-typeck/src/check.rs:970/7508/7680/7931` — comments documenting that map-entry
  loop bindings are RE-CREATED from `ynz_map_iter_get` on each body-bb entry and have no
  crossing-local frame slot. **P3 must preserve or consciously revise this
  entry-recreation contract when the iter ABI changes** — it is load-bearing for
  suspension-crossing map loops.
- `crates/ynz-runtime/src/runtime.rs` — no `ynz_map_*` call sites (bg-arg free path is
  array/shape-only today; if P3 adds map bg-args, a new descriptor kind is needed — surface at
  P3 design time).

## Grep gate (E12 proof obligation, run at Phase 3 exit)

Zero old-signature `ynz_map_*` decls; zero raw `ynz_map_*` element load/store calls outside the
choke-point helpers Phase 3 introduces.
