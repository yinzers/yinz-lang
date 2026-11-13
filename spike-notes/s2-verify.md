# S2 independent verification (scratch, branch spike/v0-3-m9-s2-verify; base spike commit ebc6b26a7cc9)

Build: rustup stable + prebuilt LLVM 18.1.8 (LLVM_SYS_181_PREFIX=/tmp/llvm18 via env, `.cargo/config.toml` untouched),
stub libtinfo.so.5 with NCURSES_TINFO_5.0.19991023 version node, `RUSTFLAGS="-L /tmp/shim"`,
`cargo build -p ynz-runtime && cargo build -p ynz-driver`. Run: `LD_LIBRARY_PATH=/tmp/shim target/debug/ynz run <file>`.

## Claim 1: typeck accepts, output wrong — CONFIRMED (all 7)
| prog | actual | expected |
|---|---|---|
| p02 | inner, inner | inner, outer |
| p09 | amy, bob, bob | amy, bob, outer |
| p12 | b1 b2 b2 b1 b2 b2 | b1 b2 a1 b1 b2 a2 |
| p23 | inner, inner | inner, outer |
| p26 | 7, 0.000000000000000000000000000000000000000000000 | 7, 1.5 |
| p08 | amy, bob, bob | amy, bob, param |
| p28 | 1, 978766256 (garbage; spiker saw different garbage) | 1, 99 |
All exit 0, no diagnostics.

## Claim 2: p02 one slot — CONFIRMED
`YNZ_NO_OPTIMIZE=1 ynz build --emit-ir p02.ynz`: exactly one `%x_alloca = alloca ptr` (line 226); both `let x` stores
(`store ptr @bts` / `@bts.1`) target `%x_alloca`; both flushes `%ls_1` and `%ls_19` = `getelementptr i8, ptr %0, i64 40`.

## Claim 3: renamed variants correct — CONFIRMED
p02 inner/outer; p09 amy/bob/outer; p12 b1 b2 a1 b1 b2 a2; p23 inner/outer; p26 7/1.5.

## Claim 4: guard coverage — CONFIRMED, with two refinements
check.rs:1540-1625 (Check 3) iterates `crossings` and fires only if `outer_is_genuine_crossing_local` (needs a TOP-LEVEL
`let` before the first top-level suspension, then a post-wait read / top-level redecl / suspending loop|match);
`find_shadow_in_stmts` additionally needs a top-level outer `let`. Nested outers, `for` vars, non-crossing shadows: not examined.
Mirror at check.rs:12800 (`suspension_guards_fire_for_fn`) is identical. p04/p05/p07/p10/p18 rejected as the spiker said.
Refinements:
1. Param shadow Check 3b(a) (`param_has_nested_let_shadow`, :1628+, mirror :12830) fires for ANY nested `let <param>` in any
   suspending fn, crossing or not (spiker wrote "param crossing a wait"). It only looks at `let`, so a `for` var named like a
   param (p08) is the hole. A destructure alias is a desugared `let`, so it IS caught.
2. A pre-existing known gate exists the spike notes do not mention: `crates/ynz-driver/tests/d5_frame_slot_collision_planned_red.rs`
   (ignored, "name-keyed loop-var frame-slot collision", fixture v0_3_m7_d5_suspending_loop_var_slot_collision.ynz).

## Claim 5: cases the per-function `uniquify_bindings` pass does not (or may not) cover
Probes in spike/s2-verify/.
- q02 nested shape-destructure with the same alias (`for ({a as v}..) { for ({b as v}..)`): accepted, WRONG (d1 d2 d2 d1 d2 d2,
  want d1 d2 a1 d1 d2 a2); q02b hand-renamed (v2): correct. Same class as p12; pass fixes it iff it walks the parser-desugared `let v = __shape.f` (an ordinary Let).
- q05 errors-capable values (outer failed, inner ok, same name `r`): accepted, WRONG (99.9 / 99.9 / false, want 99.9 / 0.0 / true).
  q05b hand-renamed: correct. So errors bindings are a collision victim the spike table lacks; renaming fixes it.
- q06 `background work(tag)` with shadowed `tag` crossing: accepted, WRONG main prints `in in` (want `in out`). q06b hand-renamed: correct (out, in, in, out; background tasks print first).
- q01 nested shape-destructure (distinct aliases) and q03 sibling destructure over different shapes: correct. Parser names the
  loop var literally `__shape` (map form `__entry`) in every such loop, so the pass will see colliding `__shape` names; harmless today
  because the var is read only before the first wait. Pass must treat it consistently (rename or leave), not trip on it.
- q04 map-entry destructure `for ((k, v) in map)`: codegen fails on main even with no wait/no nesting
  ("field_gep: receiver is not a Shape, got String"). Pre-existing, independent of S2; found item.
- q07 `let { a } = p` destructuring does not parse on this base (no binding form to cover).
Risks to the Q3 conclusion (read, not run):
- Where the pass runs matters. Typeck results are span- or function-keyed (TypedModule), so a renamed clone keeps `expr_types`
  valid. But SoA decisions are keyed `(array_name, decl_span)` (soa.rs:245, emit.rs:13782-13791) from the ORIGINAL AST; a codegen-only
  rename of an `array<Shape>` binding makes the lookup miss and silently falls back to AoS (perf loss, not wrong output). Run the pass
  before typeck, or key SoA by span. Probe q08 (two sibling-scope `pts: array<P>` of 70 elements, one escaping) prints the correct 2415/4830/70 both same-name and renamed, so no observed SoA name bug today; the silent-AoS-fallback is read from code, not observed.
- Generated names `name__N` can collide with a user identifier of that exact name; the pass must pick a name absent from the function.
- Diagnostics/LSP: spiker's clone-only advice is right, but anything typeck emits after the rename (lints naming bindings, e.g.
  background-handle-not-waited) would print renamed names if the pass runs before the lint.
