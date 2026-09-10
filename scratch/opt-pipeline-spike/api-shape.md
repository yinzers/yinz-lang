---
name: "opt-pipeline-spike-api-shape"
plan-id: "2026-07-04-v0-3-m7-optimizer-pipeline"
created_at: "2026-07-16"
status: "active"
metadata:
  type: "spike-artifact"
  phase: "0"
  verdict: "GREEN"
---

# v0.3-M7 Phase 0 spike — inkwell 0.9.0 PassBuilder API shape (GREEN)

**This file is the durable artifact of the Phase 0 feasibility spike** (per
plan-spike-discipline Facet 2). Phase 3 consumes this directly — do NOT re-discover the API
there. The sibling `src/main.rs` is the throwaway repro kept as reference; its build output
(`target/`) is gitignored and never committed.

## Verdict

**GREEN.** `inkwell` 0.9.0 (feature `llvm18-1-prefer-dynamic`, `llvm-sys` 181.3.0, LLVM 18)
exposes LLVM's new-PM PassBuilder surface, and a `"default<O2>"` pipeline demonstrably ran:
dead store eliminated, alloca promoted (mem2reg), return constant-folded, O2 function
attributes inferred. Not a no-op `Ok(())`.

## The exact API shape (verified against vendored source, 2026-07-16)

Entry point — `inkwell::module::Module::run_passes`
(vendored source: `inkwell-0.9.0/src/module.rs:1631`, gated `#[llvm_versions(13..)]`, so live
at `llvm18-1`):

```rust
pub fn run_passes(
    &self,
    passes: &str,                 // opt-style new-PM pipeline string
    machine: &TargetMachine,      // required — no machine-less overload
    options: PassBuilderOptions,  // consumed by value (LLVMPassBuilderOptionsRef wrapper)
) -> Result<(), LLVMString>
```

- **Pipeline string format**: identical to `opt -passes=` (new pass manager). Full pipelines:
  `"default<O0>"` … `"default<O3>"`, `"default<Os>"`, `"default<Oz>"`; individual passes
  comma-separated (e.g. `"mem2reg,dce"`). An invalid string returns `Err(LLVMString)` — it
  does not abort.
- **`PassBuilderOptions`**: `inkwell::passes::PassBuilderOptions::create()`
  (`inkwell-0.9.0/src/passes.rs:1196`). Setters available: `set_verify_each`,
  `set_debug_logging`, `set_loop_interleaving`, `set_loop_vectorization`,
  `set_loop_slp_vectorization` (SLP), `set_loop_unrolling`,
  `set_forget_all_scev_in_loop_unroll`, `set_licm_mssa_opt_cap`,
  `set_licm_mssa_no_acc_for_promotion_cap`, `set_call_graph_profile`,
  `set_merge_functions`. Defaults (bare `create()`) were sufficient for the spike.
- **`TargetMachine` is mandatory**: the module-level pipeline needs a machine. In the compiler,
  the authoritative constructor is `crate::state_machine::default_target_machine()`
  (`crates/ynz-codegen/src/emit.rs:886` threads it — per `authoritative-derivation.md`, Phase 3
  must reuse that machine, never construct a second one). The spike mirrored its shape
  (generic CPU, empty features, `RelocMode::Default`, `CodeModel::Default`).
- **Ordering**: run `run_passes` on the `Module` AFTER IR emission completes and BEFORE object
  emission (`TargetMachine::write_to_file` / `write_to_memory_buffer`). The
  `OptimizationLevel` passed to `create_target_machine` is the *codegen-stage* (back-end)
  level and is independent of this middle-end pipeline.
- Also exposed (not needed by the spike): `FunctionValue::run_passes`
  (`inkwell-0.9.0/src/values/fn_value.rs:543`) — same shape, per-function scope. The legacy
  `PassManager` still exists but is deprecated in-source ("Use `PassBuilderOptions` with
  `Module::run_passes` instead").

## The dead-store-elimination proof (captured run output, 2026-07-16, dev container)

IR before `default<O2>`:

```llvm
define i64 @spike() {
entry:
  %slot = alloca i64, align 8
  store i64 42, ptr %slot, align 4   ; dead store — overwritten before any load
  store i64 7, ptr %slot, align 4
  %v = load i64, ptr %slot, align 4
  ret i64 %v
}
```

IR after `module.run_passes("default<O2>", &machine, PassBuilderOptions::create())`:

```llvm
; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define noundef i64 @spike() local_unnamed_addr #0 {
entry:
  ret i64 7
}
```

Assertions passed: `store i64 42` absent, `alloca` absent, `ret i64 7` present.

## Repro

```bash
docker compose run --rm dev bash -c "cd scratch/opt-pipeline-spike && cargo run"
```
