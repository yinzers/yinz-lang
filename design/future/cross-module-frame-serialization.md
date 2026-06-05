# Cross-Module Frame Serialization (M3e)

**Status**: Deferred to v0.3-M3e. ALL cross-module suspending calls are rejected at compile
time with a clean diagnostic until M3e ships. The universal reject replaced a leaky
predictive guard (M3b Phase 1 close-out): five silent-crash escapes proved that typeck-side
frame analysis cannot safely predict which cross-module cases codegen can handle.

---

## WHAT

Full FrameLayout serialization across the export table. Instead of a scalar `u64`
approximation (`composed_frame_size`) for the saved-state size of an exported suspending
function, the compiler serializes the entire `FrameLayout` struct into the export table
and deserializes it at each import site.

The serialized layout includes:
- exact slot offsets for every crossing local (with LLVM-derived ABI sizes, not typeck approximations)
- the full sub-frame tree (each nested suspending callee's slot range)
- the EC staging-slot position when present
- the state-counter offset

With the full layout, the importing module's `build_frame_layouts` can embed the foreign
sub-frame at the correct offset instead of treating it as an opaque blob of `composed_frame_size` bytes.

---

## WHY

The typeck-side frame analysis (`is_composed_frame_simple`, now removed) reimplemented a
subset of `build_frame_layouts` (the codegen pass that lays out state-machine frames) using
only information from the exported module's AST and typeck types. It is a different,
shallower algorithm than what codegen actually does — and it disagreed with codegen five
times, each time producing a silent crash instead of a clean error:

1. **Re-export / multi-level transitive suspension**: Module B imports a suspending function
   `f` from A and re-exports its own `g` that calls `f`. Module C imports `g`. C's caller
   cannot embed B's sub-frame correctly because B's scalar size for `g` only accounts for
   B's view of A's frame — not the full tree that C's codegen needs.

2. **Shape-typed crossing locals**: A shape field's LLVM ABI size is computed by the backend
   (target-dependent struct layout, padding, alignment). The typeck approximation counts
   fields and assumes 8 bytes each. For shapes with mixed-width fields or nested shapes,
   the approximation under-sizes the slot, corrupting whatever lives at the real end of
   the slot in the calling frame.

3. **EC × transitive child sub-frames**: An errors-capable export that suspends transitively
   (via an inner function that calls `sleep`) has a 16-byte staging slot whose position
   interacts with the child sub-frame offsets. The scalar approach cannot reproduce the
   exact layout the calling module's `build_frame_layouts` expects.

4. **Number-type crossing locals in a cross-module caller**: A caller that has its own
   number-typed crossing locals AND calls an imported suspending function needs both frames
   composed correctly. The typeck analysis mis-sized the combined frame.

5. **Transitive × caller-frame combinations**: Any combination where the caller itself has
   a multi-slot frame AND the callee is a transitive suspender caused the approximation
   to produce the wrong total size.

All five classes produce silent memory corruption or crash without the universal loud-reject
guard added in M3b Phase 1 close-out. M3e removes the guard and replaces it with correct
serialization so ALL cross-module suspending combos work.

---

## COST (implementation cost at fix time)

Serializing `FrameLayout` requires:

1. **`FrameLayout` → wire format**: derive `serde::Serialize / Deserialize` (or a manual
   binary format) on the `FrameLayout` struct in `ynz-codegen`. The struct currently lives
   only in the codegen pass and is not shared with `ynz-typeck`.

2. **Move or expose `FrameLayout`**: `ynz-typeck` needs to read the serialized layout at
   import-resolution time. Either move the type to a shared crate (`ynz-types` or similar)
   or add a cross-crate accessor. This is the main structural change — it introduces a
   compile-time dependency from typeck on codegen types, which the current architecture
   deliberately avoids to keep the salsa query graph acyclic.

3. **Export-table wire format**: `FunctionSig` (in `ynz-typeck`) currently stores
   `composed_frame_size: u64`. M3e replaces that with `frame_layout: Option<FrameLayout>`.
   (`composed_frame_simple: bool` was removed in M3b Phase 1 close-out — no longer needed
   for the universal-reject guard.) The salsa-tracked `module_signatures_query` result will
   change shape, requiring a coordinated update across `resolve_import.rs`, `queries.rs`,
   `signatures.rs`, and any test that constructs `FunctionSig` literals.

4. **`build_frame_layouts` in the importing module**: currently seeds imported-callee slots
   with `sig.composed_frame_size`. M3e changes this to embed the deserialized layout at the
   exact offset, using the same slot-placement arithmetic as the exporting module's pass.

Estimated scope: one focused session. The structural crate-dependency question (point 2)
is the decision-point that may require a pre-design step.

---

## TRIGGER

The universal loud-reject guard added in M3b Phase 1 close-out (any imported suspending
function → compile error) is the direct trigger: any user who wants cross-module suspension
is blocked until M3e ships. When user reports of the rejection accumulate, or when a stdlib
module needs cross-module suspension in any form, M3e becomes load-bearing.

The universal reject is provably safe — it rejects everything so it cannot miss a case.
M3e replaces it with correct serialization so all cross-module suspending combos work.
