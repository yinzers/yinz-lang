# S1 verdict — by-value ABI spike: **GREEN**

- **Session:** phase0-executor-2026-07-03-m5 · 2026-07-03 · plan `2026-07-03-v0-3-m5-auto-soa` Phase 0 step 1
- **Fixture:** `spike-notes/s1_byval_fixture.ynz` (this dir) — 3-element `array<Part{qty:int, price:float}>`,
  ALL field values runtime-computed; exercises construct (ArrayLit), set (`parts[1] = {…}` IndexAssign),
  get (for-loop element reads), stdout of both int and float fields.
- **Proof protocol (real compiler, differential, never narrated):** built `ynz-driver` in the Docker
  `dev` container; ran the fixture through `./target/debug/ynz run` twice on the same binary —
  gate OFF (pointer ABI) and `YNZ_SPIKE_BYVAL=1` (by-value prototype ABI).

## Result

| Run | Exit | stdout |
|---|---|---|
| gate OFF (pointer ABI) | 0 | `11 33 100 700 13 39 124` (one per line) |
| gate ON (by-value ABI) | 0 | **byte-identical** (`diff` clean) |

The expected values are hand-derivable from the fixture (qty 11/100/13; price = qty·3 → 33/39, set
element = 100/700; total 124), so the golden is independently checkable, and the `parts[1]` set →
read-back (`100`, `700`) proves the set+get round-trip through the memcpy ABI.

**Gate-on genuinely used the new ABI:** an intermediate build with a stale runtime archive failed to
LINK with `undefined reference to ynz_array_byval_{new,push,set,get,count}` at the fixture's call
sites — direct evidence the env-gated codegen emits the by-value calls (they are not dead-gated).
After rebuilding the runtime staticlib the same fixture ran green.

## Prototype ABI proven

`#[repr(C)] YnzArrayByVal { data: *mut u8, len: i64, cap: i64, elem_size: i64 }` with
`new(elem_size) / push(*const u8) [memcpy] / get(idx, out) -> has-flag [memcpy out] /
set(idx, *const u8) [memcpy, aborts on OOB — parity with ynz_array_set] / count()`.
Elements are elem_size-byte inline values; element source pointers (stack allocas from
`lower_struct_lit`) are dead the instant push/set returns — the stack-dangling class the M3a
guard masks cannot exist on this ABI.

## Load-bearing discoveries for Phase 2 (the real migration)

1. **elem_size has ONE authoritative source:** `shape_abi_sizes` (LLVM-TargetData-accurate,
   `crates/ynz-codegen/src/queries.rs:184`). It was NOT reachable from `Cg` — the spike threaded it as
   a `Cg` field (`spike_shape_abi_sizes: Option<&HashMap<String,u64>>`, `None` only on the generic
   path) set at all 3 `Cg` construction sites (emit.rs ~1403 generic / ~2182 non-SM / ~2670 SM-resume).
   P2 must thread it the same way (non-Option, all paths) — never re-derive per
   `authoritative-derivation.md`.
2. **Build-order trap:** `ynz-driver` does NOT cargo-depend on `ynz-runtime`; it `include_bytes!`-embeds
   `target/<profile>/libynz_runtime.a` located by `crates/ynz-driver/build.rs` (rerun-if-changed on the
   .a). `cargo build -p ynz-driver` alone does NOT rebuild the runtime → stale-archive missing-symbol
   linker failures at fixture-link time. Correct sequence: `cargo build -p ynz-runtime && cargo build
   -p ynz-driver`. (Observed live: .a at 07:19 with 0 byval symbols vs driver binary at 07:44.)
3. **Shape variable slots are pointers** (`llvm_type_for_ctx` `_` arm → ptr, emit.rs:1370-1380), so a
   by-value get can hand back a pointer to a stack out-buffer and every downstream field GEP works
   unchanged. The spike reused one ENTRY-BLOCK out-buffer per loop (`alloca_in_entry_llvm`) — P2 must
   do the same; a body-block alloca would grow the stack per iteration.
4. **Alloc parity:** byval array = 2 mallocs (header + data), same as `YnzArray` — no per-element heap
   allocations (vs the pointer ABI's per-element shape allocas/globals). E8's alloc=free story improves
   or holds; nothing regresses by construction.
5. **`ynz_array_drop` is layout-compatible** with the byval header (data ptr at offset 0, then len/cap)
   — an accidental property, NOT a contract. P2's hard-cut must ship `ynz_array_byval_drop` (or the
   unified elem_size-aware drop) rather than lean on this.

## What S1 deliberately did NOT prove (P2 scope, not spike scope)

SM/`wait`-crossing paths; `lower_array_method` surfaces (`get/first/last/contains/add/set`);
`IndexAccess` maybe-envelope reads; `map<K,Shape>` symmetry (E12, P3); drop/free integration;
shapes with pointer-typed fields (string fields inside by-value elements stay pointers — semantics
to be decided by P2/scratch-doc Option A); `.copy()`/background-arg interplay.

## Pre-existing base bug surfaced (NOT S1's, NOT fixed — routed to conductor)

`let x: float = 1.5` (decimal literal under a float annotation) miscompiles to ~0.0 on the untouched
tree: typeck types the literal `Float` (`crates/ynz-typeck/src/check.rs:2198-2199`) but codegen's
`Expr::NumberLit` arm unconditionally lowers a decimal128 i128 alloca
(`crates/ynz-codegen/src/emit.rs:12370-12402` pre-spike numbering) with no f64 path. Verified by
probe: `n.toFloat()` prints `3` correctly while `let lit: float = 1.5` compares as ~0 and prints
`0.0000…`. The bug is memorialized in the committed demo golden
(`examples/pirates-roster/expected_stdout.txt:7` is the zero string). The fixture therefore seeds
floats via `.toFloat()` (the test suite's own established idiom — e.g.
`v0_3_m3d_return_class_float.ynz`), which keeps the float-through-byval proof fully valid.

## Teardown

All spike scaffolding (runtime block in `crates/ynz-runtime/src/lib.rs`, decls in
`crates/ynz-codegen/src/runtime_decls.rs`, the 3 gated lowering sites + `Cg` field in
`crates/ynz-codegen/src/emit.rs`) reverted after this verdict was recorded; tree rebuilt and the
fixture re-run green on the pristine pointer ABI (see handoff build+test status). Only this note and
the fixture persist, per the spike-teardown evidence rule. P2 step 1 consumes both.
