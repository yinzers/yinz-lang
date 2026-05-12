/// LLVM IR emission via inkwell.
///
/// # LLVM context lifetime
///
/// `inkwell::Context` is NOT `Send + Sync` and must NOT outlive this module.
/// The `emit_artifact` function creates a `Context`, builds the module, emits
/// object bytes, and drops the context before returning. Salsa never sees
/// inkwell types — only the owned bytes in `CompiledArtifact`.
use inkwell::{
    context::Context,
    module::Module,
    targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
    },
    OptimizationLevel,
};

use crate::artifact::{sha256, CompiledArtifact};

/// The file ID embedded in the LLVM module for deterministic IR and object output.
///
/// Including the source file name makes the IR easier to diff in regression tests.
pub fn module_identifier(source_path: &str) -> String {
    format!("ynz-m1-{source_path}")
}

/// Emit a relocatable object file for the M1 hello-world program.
///
/// `string_bytes` is the raw UTF-8 content of the string literal to pass to `puts`.
/// It must NOT include the null terminator — this function appends one.
///
/// # Errors
///
/// Returns a human-readable error string if LLVM fails at any step.
/// Callers should convert this into a Diagnostic before surfacing it to the user.
pub fn emit_artifact(
    source_path: &str,
    string_bytes: &[u8],
    target_triple: Option<&str>,
) -> Result<CompiledArtifact, String> {
    // Initialise only the X86 target for now (the Linux/macOS CI runner is x86_64).
    // This is expanded to all targets before v0.1 ships, but keeping it narrow
    // during M1 avoids the extra LLVM initialisation time.
    Target::initialize_x86(&InitializationConfig::default());

    let context = Context::create();
    let module_id = module_identifier(source_path);
    let module = context.create_module(&module_id);

    // Determine target triple — use the provided one or fall back to host.
    let triple = match target_triple {
        Some(t) => TargetTriple::create(t),
        None => TargetMachine::get_default_triple(),
    };
    module.set_triple(&triple);

    let target = Target::from_triple(&triple)
        .map_err(|e| format!("LLVM: no target for triple {:?}: {e}", triple.as_str()))?;

    // Deterministic target machine options: no PIC randomisation, fixed code model,
    // no debug info in M1 (debug info embeds paths and timestamps → breaks SHA-256 contract).
    let machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| "LLVM: failed to create target machine".to_string())?;

    module.set_data_layout(&machine.get_target_data().get_data_layout());

    // Build the IR.
    build_hello_module(&context, &module, string_bytes)?;

    // Verify the module (sanity-checks our own IR construction).
    module
        .verify()
        .map_err(|e| format!("LLVM module verify failed: {}", e.to_string()))?;

    // Emit IR text (informational — for snapshot regression detection).
    let ir_text = module.print_to_string().to_string();

    // Emit relocatable object file.
    let obj_buf = machine
        .write_to_memory_buffer(&module, FileType::Object)
        .map_err(|e| format!("LLVM: failed to write object: {}", e.to_string()))?;
    let object_bytes = obj_buf.as_slice().to_vec();

    let hash = sha256(&object_bytes);
    Ok(CompiledArtifact {
        object_bytes,
        ir_text,
        sha256: hash,
    })
}

/// Build the M1 module IR:
/// ```ir
/// @str_data = private unnamed_addr constant [N x i8] c"...\00"
/// declare i32 @puts(ptr)
/// define i32 @main() {
///   %p = getelementptr [N x i8], ptr @str_data, i32 0, i32 0
///   call i32 @puts(ptr %p)
///   ret i32 0
/// }
/// ```
fn build_hello_module<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    string_bytes: &[u8],
) -> Result<(), String> {
    let builder = ctx.create_builder();
    let i32_ty = ctx.i32_type();
    let i8_ty = ctx.i8_type();
    let ptr_ty = ctx.ptr_type(inkwell::AddressSpace::default());

    // Null-terminated string constant.
    let mut null_term = string_bytes.to_vec();
    null_term.push(0);

    let const_arr = i8_ty.const_array(
        &null_term
            .iter()
            .map(|&b| i8_ty.const_int(b as u64, false))
            .collect::<Vec<_>>(),
    );
    let arr_ty = i8_ty.array_type(null_term.len() as u32);
    let global = module.add_global(arr_ty, Some(inkwell::AddressSpace::default()), "str_data");
    global.set_initializer(&const_arr);
    global.set_constant(true);
    global.set_linkage(inkwell::module::Linkage::Private);
    global.set_unnamed_address(inkwell::values::UnnamedAddress::Global);

    // Declare `puts`.
    let puts_ty = i32_ty.fn_type(&[ptr_ty.into()], false);
    let puts_fn = module.add_function("puts", puts_ty, Some(inkwell::module::Linkage::External));

    // Define `main`.
    let main_ty = i32_ty.fn_type(&[], false);
    let main_fn = module.add_function("main", main_ty, None);
    let entry_block = ctx.append_basic_block(main_fn, "entry");
    builder.position_at_end(entry_block);

    // GEP to get a pointer to the first byte of the constant.
    let zero_i32 = i32_ty.const_int(0, false);
    // SAFETY: we control the global type and indices.
    let ptr = unsafe {
        builder.build_gep(
            arr_ty,
            global.as_pointer_value(),
            &[zero_i32, zero_i32],
            "str_ptr",
        )
    }
    .map_err(|e| format!("GEP build error: {e}"))?;

    builder
        .build_call(puts_fn, &[ptr.into()], "puts_call")
        .map_err(|e| format!("call build error: {e}"))?;

    builder
        .build_return(Some(&i32_ty.const_int(0, false)))
        .map_err(|e| format!("return build error: {e}"))?;

    Ok(())
}
