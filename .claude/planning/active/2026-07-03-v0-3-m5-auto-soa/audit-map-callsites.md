# P0 audit — exhaustive `ynz_map_*` call-site checklist (feeds Phase 3 step 1 ENTRY criteria — E12)

- **Session:** phase0-executor-2026-07-03-m5-seg2 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 4
- **Method:** `grep -n "ynz_map_"` across the workspace + enclosing-fn attribution against the
  live worktree (fork of `1ac52fd`). Re-anchor by enclosing function + op label once lines drift.
- **Usage:** Phase 3 step 1 ticks every box for the `map<K,Shape>` symmetric hard-cut (E12's
  audit precondition — the SAME missed-call-site class as E7 but on the map surface, which the
  array-only grep gate does NOT cover).

## A. Runtime ABI — `crates/ynz-runtime/src/lib.rs`

- [ ] `YnzMap` struct def — lib.rs:594; `ynz_map_new()` — lib.rs:674 (values are uniform i64
      slots — the same pointer-element pattern as `YnzArray`, pre-existing miscompile class for
      shape values)
- [ ] `ynz_map_get(map, i64 key, out: *mut [i64;2])` — lib.rs:907
- [ ] `ynz_map_get_str(map, *const u8 key, out)` — lib.rs:923
- [ ] `ynz_map_set(map, i64 key, i64 value)` — lib.rs:946
- [ ] `ynz_map_set_str(map, *const u8 key, i64 value)` — lib.rs:973
- [ ] `ynz_map_count(map)` — lib.rs:1017
- [ ] `ynz_map_has(map, i64 key)` — lib.rs:1026
- [ ] `ynz_map_iter_get(map, pos, out: *mut [i64;3])` — lib.rs:1040; NOTE internal delegation to
      `ynz_map_get` at lib.rs:1047 (single-choke-point precedent already half-present)
- [ ] `ynz_map_iter_get_str(map, pos, out)` — lib.rs:1060
- [ ] `ynz_map_drop(map)` — lib.rs:1086 (element-blind — same E8 leak-class question as
      `ynz_array_drop`; dispositioned by the P3 design)

## B. LLVM decl surface — `crates/ynz-codegen/src/runtime_decls.rs`

- [ ] Struct fields `ynz_map_{new,get,get_str,set,set_str,count,has,iter_get,iter_get_str,drop}`
      — runtime_decls.rs:65-79
- [ ] `declare_fn` signatures for all ten — runtime_decls.rs:402-439

## C. Codegen call sites — `crates/ynz-codegen/src/emit.rs`

### Construction
- [ ] `Expr::MapLit` lowering: `ynz_map_new` @13338-13342 + per-entry `ynz_map_set_str` @13353 /
      `ynz_map_set` @13363
- [ ] `Expr::StructLit`-with-map-annotation lowering (`{ key: value }` under a `map<...>`
      annotation): `ynz_map_new` @12984-12988 + `ynz_map_set_str` @12997

### Element read
- [ ] `Expr::IndexAccess` map branch: `ynz_map_get_str` @13167 / `ynz_map_get` @13177
      (string-key vs int-key split; maybe-envelope result)
- [ ] `lower_map_method "get"`: `ynz_map_get_str` @17017 / `ynz_map_get` @17027
- [ ] `lower_map_method "has"`: `ynz_map_get_str` @16977 (string-key path) / `ynz_map_has`
      @16993 (int-key path)

### Element write
- [ ] `Stmt::IndexAssign` map branch: `ynz_map_set_str` @11002 / `ynz_map_set` @11012 (in
      `lower_stmt`)
- [ ] `lower_map_method "set"`: `ynz_map_set_str` @17079 / `ynz_map_set` @17089

### Length
- [ ] `lower_map_method "count"`: `ynz_map_count` @16958

### For-loop entry iteration (BOTH paths — same twin-dispatch hazard as arrays)
- [ ] Non-SM map for-loop: `ynz_map_count` @11700 + `ynz_map_iter_get_str` @11757 /
      `ynz_map_iter_get` @11765 (in `lower_stmt_for`)
- [ ] SM (suspension) map for-loop: `ynz_map_count` @5580 + `ynz_map_iter_get_str` @5613 /
      `ynz_map_iter_get` @5621 (in `lower_sm_for`)

### Debug/format surface
- [ ] Map debug representation (toString/print path): `ynz_map_count` @15439 +
      `ynz_map_iter_get_str` @15501 (insertion-order repr)

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
