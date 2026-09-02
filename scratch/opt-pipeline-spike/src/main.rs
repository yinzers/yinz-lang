// v0.3-M7 Phase 0 feasibility spike (throwaway — kept checked in as reference only, per
// plan-spike-discipline Facet 2; the durable artifact is api-shape.md alongside this file).
//
// Proves inkwell 0.9.0 (LLVM 18, llvm18-1-prefer-dynamic) exposes LLVM's new-PM PassBuilder
// surface: `Module::run_passes(passes, &TargetMachine, PassBuilderOptions)`.
//
// Proof shape: build a module with one function containing one alloca, one DEAD store
// (42, overwritten before any load), one live store (7), one load, ret. Run
// `"default<O2>"`. If the pipeline genuinely ran (not a no-op Ok return), mem2reg + DSE +
// constant folding collapse the body to `ret i64 7` — no alloca, no store survives.

use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use inkwell::OptimizationLevel;

fn main() {
    let context = Context::create();
    let module = context.create_module("spike");
    let builder = context.create_builder();

    let i64t = context.i64_type();
    let fn_type = i64t.fn_type(&[], false);
    let function = module.add_function("spike", fn_type, None);
    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);

    let slot = builder.build_alloca(i64t, "slot").expect("alloca");
    // Dead store: 42 is overwritten by 7 before any load ever reads it.
    builder
        .build_store(slot, i64t.const_int(42, false))
        .expect("dead store");
    builder
        .build_store(slot, i64t.const_int(7, false))
        .expect("live store");
    let v = builder.build_load(i64t, slot, "v").expect("load");
    builder.build_return(Some(&v)).expect("ret");

    let before = module.print_to_string().to_string();
    println!("=== IR BEFORE default<O2> ===\n{before}");

    // Mirror the project's real TargetMachine construction shape
    // (crates/ynz-codegen/src/emit.rs default-triple branch): generic CPU, empty features,
    // Default reloc/code model. OptimizationLevel here is the codegen-stage level; the
    // new-PM middle-end pipeline below is what this spike is proving.
    Target::initialize_native(&InitializationConfig::default()).expect("target init");
    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).expect("target from triple");
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .expect("target machine");

    module
        .run_passes("default<O2>", &machine, PassBuilderOptions::create())
        .expect("run_passes(default<O2>) failed");

    let after = module.print_to_string().to_string();
    println!("=== IR AFTER default<O2> ===\n{after}");

    assert!(
        before.contains("store i64 42"),
        "precondition broken: dead store missing from pre-opt IR"
    );
    assert!(
        !after.contains("store i64 42"),
        "dead store survived — pipeline was a no-op"
    );
    assert!(
        !after.contains("alloca"),
        "alloca survived — mem2reg did not run"
    );
    assert!(
        after.contains("ret i64 7"),
        "expected constant-folded `ret i64 7` in post-opt IR"
    );
    println!("GREEN: dead store eliminated, alloca promoted, ret constant-folded to 7.");
}
