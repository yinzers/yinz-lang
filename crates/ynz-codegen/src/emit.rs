/// LLVM IR emission via inkwell.
///
/// # LLVM context lifetime
///
/// `inkwell::Context` is NOT `Send + Sync` and must NOT outlive this module.
/// `emit_artifact` creates a `Context`, builds the module, emits object bytes,
/// and drops the context before returning. Salsa never sees inkwell types.
///
/// # Decimal128 representation
///
/// `number` values are stored as `i128` on the stack (16 bytes = BID encoding).
/// `BasicValueEnum::PointerValue` carries decimal128 values through the lowering
/// pass; the runtime arithmetic functions receive `ptr` arguments.
use std::collections::HashMap;

use inkwell::{
    attributes::{Attribute, AttributeLoc},
    basic_block::BasicBlock,
    context::Context,
    module::Module,
    targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
    },
    types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, GlobalValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use ynz_ast::nodes::{
    BinOpKind, Expr, FunctionDecl, Item, MatchPatternKind, OwnershipModifier, Stmt, UnaryOpKind,
};
use ynz_numerics; // parse(s: &str) -> Option<u128>
use ynz_typeck::{
    type_attached_const_type, GenericFnTable, MonomorphizationTable, ShapeTable, SignatureTable,
    Type, TypedModule,
};

use crate::{
    artifact::{sha256, CompiledArtifact},
    runtime_decls::RuntimeDecls,
    shape_types::{emit_shape_types, ShapeLlvmTypes},
    vtable::emit_vtable_globals,
};

/// The file ID embedded in the LLVM module for deterministic IR and object output.
pub fn module_identifier(source_path: &str) -> String {
    format!("ynz-{source_path}")
}

/// Emit a relocatable object file for an M5 program.
pub fn emit_artifact(
    source_path: &str,
    typed_module: &TypedModule,
    shape_table: &ShapeTable,
    _sig_table: &SignatureTable,
    generic_fn_table: &GenericFnTable,
    mono_table: &MonomorphizationTable,
    target_triple: Option<&str>,
    imported_options: &std::collections::HashMap<String, ynz_typeck::options_table::OptionsEntry>,
) -> Result<CompiledArtifact, String> {
    Target::initialize_x86(&InitializationConfig::default());

    let context = Context::create();
    let module_id = module_identifier(source_path);
    let module = context.create_module(&module_id);

    let triple = match target_triple {
        Some(t) => TargetTriple::create(t),
        None => TargetMachine::get_default_triple(),
    };
    module.set_triple(&triple);

    let target = Target::from_triple(&triple)
        .map_err(|e| format!("LLVM: no target for triple {:?}: {e}", triple.as_str()))?;
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

    build_module(
        &context,
        &module,
        source_path,
        typed_module,
        shape_table,
        generic_fn_table,
        mono_table,
        imported_options,
    )?;

    module
        .verify()
        .map_err(|e| format!("LLVM module verify failed: {}", e.to_string()))?;

    let ir_text = module.print_to_string().to_string();
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

struct ModuleGlobals<'ctx> {
    str_true: GlobalValue<'ctx>,
    str_false: GlobalValue<'ctx>,
    dec_zero: GlobalValue<'ctx>,
    panic_int_add: GlobalValue<'ctx>,
    panic_int_sub: GlobalValue<'ctx>,
    panic_int_mul: GlobalValue<'ctx>,
    panic_int_div: GlobalValue<'ctx>,
    panic_int_rem: GlobalValue<'ctx>,
    panic_dec_div: GlobalValue<'ctx>,
    panic_dec_rem: GlobalValue<'ctx>,
    /// Source file path embedded as a C string for runtime panic messages.
    source_file: GlobalValue<'ctx>,
}

/// Emit LLVM IR for one Yinz source module.
///
/// # Flow (5 passes — order is mandatory)
///
/// | Pass | What | Requires from prior passes |
/// |------|------|---------------------------|
/// | 0 | Emit LLVM struct types for all user-defined shapes | nothing |
/// | 1 | Forward-declare every non-generic function | Pass 0 (shape types used in param/return types) |
/// | 1.5 | Forward-declare monomorphized generic functions | Pass 0 + Pass 1 (non-generic functions may be called from generic bodies) |
/// | 1.6 | Emit vtable globals for `dynamic Foo` dispatch | Pass 1 (vtable entries point to forward-declared function values) |
/// | 2 | Emit non-generic function bodies (and lower generic instances) | All prior passes (bodies call functions, construct shapes, dispatch via vtables) |
///
/// Generic functions are lowered during Pass 2 by iterating `mono_table` entries, not
/// by walking the AST's `Item::Function` list — by Pass 2 every generic call site has
/// already been collected into `mono_table` by the typeck pass.
fn build_module<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    source_path: &str,
    typed: &'g TypedModule,
    shape_table: &'g ShapeTable,
    generic_fn_table: &'g GenericFnTable,
    mono_table: &'g MonomorphizationTable,
    imported_options: &std::collections::HashMap<String, ynz_typeck::options_table::OptionsEntry>,
) -> Result<(), String> {
    let rt = RuntimeDecls::declare(ctx, module);

    // M6: collect options table for variant tag lookups during codegen.
    let mut options_diags = ynz_diagnostics::DiagnosticBucket::new();
    let mut options_table =
        ynz_typeck::options_table::collect_options(&typed.module, &mut options_diags);
    // Merge imported options so cross-file options types work in codegen
    // (e.g. `Timeframe.daily` where Timeframe is imported from another file).
    for (name, entry) in imported_options {
        options_table.options.entry(name.clone()).or_insert_with(|| entry.clone());
    }

    let zero_bits = ynz_numerics::parse("0").expect("decimal zero parse");
    let globals = ModuleGlobals {
        str_true: build_string_global(ctx, module, "true", ".str.true"),
        str_false: build_string_global(ctx, module, "false", ".str.false"),
        dec_zero: build_decimal_global(ctx, module, zero_bits, ".dec.zero"),
        panic_int_add: build_string_global(ctx, module, "int overflow in '+'", ".panic.iadd"),
        panic_int_sub: build_string_global(ctx, module, "int overflow in '-'", ".panic.isub"),
        panic_int_mul: build_string_global(ctx, module, "int overflow in '*'", ".panic.imul"),
        panic_int_div: build_string_global(ctx, module, "division by zero (int)", ".panic.idiv"),
        panic_int_rem: build_string_global(ctx, module, "remainder by zero (int)", ".panic.irem"),
        panic_dec_div: build_string_global(ctx, module, "division by zero (number)", ".panic.ddiv"),
        panic_dec_rem: build_string_global(
            ctx,
            module,
            "remainder by zero (number)",
            ".panic.drem",
        ),
        source_file: build_string_global(ctx, module, source_path, ".source.file"),
    };

    // Pass 0 — emit LLVM struct types for all user-defined shapes.
    let shape_types = emit_shape_types(ctx, shape_table);

    // Pass 1 — forward-declare every non-generic function so vtables and bodies can reference them.
    for item in &typed.module.items {
        match item {
            Item::Function(f) if f.generics.is_empty() => {
                declare_function(ctx, module, f, shape_table)?
            }
            Item::Function(_)
            | Item::ShapeDecl(_)
            | Item::OptionsDecl(_)
            | Item::ImportDecl(_)
            | Item::ConstDecl(_)
            | Item::ReExport(_) => {}
        }
    }

    // Pass 1.5 — forward-declare monomorphized generic functions.
    for (key, mono_sig) in &mono_table.entries {
        let mangled = mangle_mono_name(&key.fn_name, &key.type_args);
        let param_llvms: Vec<BasicMetadataTypeEnum<'ctx>> = mono_sig
            .param_types
            .iter()
            .map(|t| {
                llvm_type_for_ctx(ctx, t)
                    .map(BasicMetadataTypeEnum::from)
                    .unwrap_or_else(|| ctx.i64_type().into())
            })
            .collect();
        let fn_ty = match llvm_type_for_ctx(ctx, &mono_sig.ret_type) {
            Some(ret) => ret.fn_type(&param_llvms, false),
            None => ctx.void_type().fn_type(&param_llvms, false),
        };
        if module.get_function(&mangled).is_none() {
            module.add_function(&mangled, fn_ty, None);
        }
    }

    // Pass 1.6 — emit vtable globals for `dynamic Foo` dispatch.
    let _vtables = emit_vtable_globals(module, shape_table);

    // Pass 2 — emit non-generic function bodies.
    for item in &typed.module.items {
        match item {
            Item::Function(f) if f.generics.is_empty() => lower_function(
                ctx,
                module,
                &rt,
                &globals,
                typed,
                f,
                shape_table,
                &shape_types,
                mono_table,
                &options_table,
            )?,
            Item::Function(_)
            | Item::ShapeDecl(_)
            | Item::OptionsDecl(_)
            | Item::ImportDecl(_)
            | Item::ConstDecl(_)
            | Item::ReExport(_) => {}
        }
    }

    // Pass 2.5 — emit monomorphized generic function bodies.
    for (key, mono_sig) in &mono_table.entries {
        let Some(fn_decl) = typed.module.items.iter().find_map(|item| {
            if let Item::Function(f) = item {
                if f.name == key.fn_name && !f.generics.is_empty() {
                    return Some(f);
                }
            }
            None
        }) else {
            continue;
        };

        let type_subst: HashMap<String, Type> = fn_decl
            .generics
            .iter()
            .zip(key.type_args.iter())
            .map(|(gp, ty)| (gp.name.clone(), ty.clone()))
            .collect();

        let mangled = mangle_mono_name(&key.fn_name, &key.type_args);
        let Some(fn_val) = module.get_function(&mangled) else {
            continue;
        };

        lower_generic_function(
            ctx,
            module,
            &rt,
            &globals,
            typed,
            fn_decl,
            shape_table,
            &shape_types,
            fn_val,
            type_subst,
            mono_sig,
            mono_table,
            &options_table,
        )?;
    }

    let _ = generic_fn_table; // consumed via mono_table; kept in signature for future use
    Ok(())
}

/// Mangle a generic function name + type args into a unique LLVM symbol name.
fn mangle_mono_name(fn_name: &str, type_args: &[Type]) -> String {
    let mut name = fn_name.to_string();
    for ty in type_args {
        name.push('_');
        name.push_str(&mangle_type(ty));
    }
    name
}

fn mangle_type(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::Bool => "boolean".to_string(),
        Type::String => "string".to_string(),
        Type::Nothing => "nothing".to_string(),
        Type::Shape { name } => format!("shape_{name}"),
        Type::Dynamic { contract } => format!("dyn_{contract}"),
        Type::BuiltinArray { elem } => format!("array_{}", mangle_type(elem)),
        Type::BuiltinFixed { elem, size } => {
            format!("fixed_{}_{}", size.unwrap_or(0), mangle_type(elem))
        }
        Type::Maybe { inner } => format!("maybe_{}", mangle_type(inner)),
        Type::Generic { name, args } => {
            let arg_str = args.iter().map(mangle_type).collect::<Vec<_>>().join("_");
            format!("{name}_{arg_str}")
        }
        Type::TypeParam { name } => format!("tparam_{name}"),
        Type::Number { precision } => format!("number_{precision}"),
        Type::Range { element, end_inclusive } => {
            format!("range_{}{}", mangle_type(element), if *end_inclusive { "_inc" } else { "" })
        }
        Type::BuiltinMap { key, val } => {
            format!("map_{}_{}", mangle_type(key), mangle_type(val))
        }
        Type::MapEntry { key, val } => {
            format!("mapentry_{}_{}", mangle_type(key), mangle_type(val))
        }
        Type::Options { name } => format!("options_{name}"),
        Type::Union { variants } => {
            let parts: Vec<_> = variants.iter().map(mangle_type).collect();
            format!("union_{}", parts.join("_or_"))
        }
        Type::ErrorsCapable { inner } => format!("errors_{}", mangle_type(inner)),
        Type::Sensitive { inner } => format!("sensitive_{}", mangle_type(inner)),
        // Type::Error should never reach codegen — an earlier phase should have
        // caught all type errors and stopped compilation. Panic here so it's
        // visible immediately if it ever does (instead of silently emitting a
        // mangled name that causes a mysterious linker error).
        Type::Error => panic!("Type::Error reached mangle_type — compilation should have stopped at typeck"),
    }
}

/// Map a typeck `Type` to an LLVM basic type, given only a `Context`.
///
/// Used during Pass 1.5 (forward declarations) where `Cg` doesn't exist yet.
fn llvm_type_for_ctx<'ctx>(ctx: &'ctx Context, ty: &Type) -> Option<BasicTypeEnum<'ctx>> {
    match ty {
        Type::Int => Some(ctx.i64_type().into()),
        Type::Float => Some(ctx.f64_type().into()),
        Type::Bool => Some(ctx.bool_type().into()),
        // N ≤ 34: hardware decimal128 path (i128 pair).
        // N > 34: bignum path — pointer to heap-allocated decimal string.
        Type::Number { precision } if *precision <= 34 => Some(ctx.i128_type().into()),
        Type::Nothing => None,
        _ => Some(ctx.ptr_type(AddressSpace::default()).into()),
    }
}

/// Lower a single monomorphized instantiation of a generic function.
#[allow(clippy::too_many_arguments)]
fn lower_generic_function<'ctx>(
    ctx: &'ctx Context,
    module: &'_ Module<'ctx>,
    rt: &'_ RuntimeDecls<'ctx>,
    globals: &'_ ModuleGlobals<'ctx>,
    typed: &'_ TypedModule,
    f: &FunctionDecl,
    shape_table: &'_ ShapeTable,
    shape_types: &'_ ShapeLlvmTypes<'ctx>,
    fn_val: FunctionValue<'ctx>,
    type_subst: HashMap<String, Type>,
    mono_sig: &ynz_typeck::generics::MonoSignature,
    mono_table: &'_ MonomorphizationTable,
    options_table: &'_ ynz_typeck::options_table::OptionsTable,
) -> Result<(), String> {
    let ret_ty = mono_sig.ret_type.clone();
    let ret_is_nothing = matches!(ret_ty, Type::Nothing);

    let mut cg = Cg {
        ctx,
        module,
        builder: ctx.create_builder(),
        rt,
        globals,
        typed,
        current_fn: fn_val,
        is_main: false,
        _current_fn_ret_ty: ret_ty,
        locals: HashMap::new(),
        shape_table,
        shape_types,
        type_subst,
        mono_table,
        options_table,
        // Generic functions are not errors-capable in M7 (no `-> T errors` on generics yet).
        is_errors_capable: false,
        errors_capable_locals: std::collections::HashSet::new(),
    };

    let entry = ctx.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);

    for (i, (param, concrete_ty)) in f.params.iter().zip(mono_sig.param_types.iter()).enumerate() {
        let llvm_param = fn_val
            .get_nth_param(i as u32)
            .ok_or_else(|| format!("missing param {} for generic `{}`", i, f.name))?;
        materialize_param(&mut cg, &param.name, llvm_param, concrete_ty)?;
    }

    for stmt in &f.body.stmts {
        if is_block_terminated(&cg) {
            break;
        }
        lower_stmt(&mut cg, stmt)?;
    }

    if !is_block_terminated(&cg) {
        if ret_is_nothing {
            cg.builder.build_return(None).map_err(|e| format!("{e}"))?;
        } else {
            cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

/// Compute the LLVM parameter types for a function declaration.
///
/// Primitive scalars pass by value. Everything else (string, number, shape, dynamic)
/// passes as an opaque pointer — the callee accesses the actual data via loads/GEPs.
fn llvm_param_types<'ctx>(
    ctx: &'ctx Context,
    f: &FunctionDecl,
    shape_table: &ShapeTable,
) -> Vec<BasicMetadataTypeEnum<'ctx>> {
    let ptr = ctx.ptr_type(AddressSpace::default());
    f.params
        .iter()
        .map(|p| {
            match &p.ty {
                ynz_ast::nodes::Type::Int => ctx.i64_type().into(),
                ynz_ast::nodes::Type::Float => ctx.f64_type().into(),
                ynz_ast::nodes::Type::Bool => ctx.bool_type().into(),
                // Named shape type or self — all shapes pass as ptr
                ynz_ast::nodes::Type::Named(n, _) if shape_table.contains(n) || n == "self" => {
                    ptr.into()
                }
                ynz_ast::nodes::Type::SelfType { .. } => ptr.into(),
                ynz_ast::nodes::Type::Dynamic { .. } => ptr.into(),
                // String, Number, and everything else: ptr
                _ => ptr.into(),
            }
        })
        .collect()
}

/// Forward-declare a function in the LLVM module (signature only, no body).
///
/// Also attaches LLVM `readonly` and `noalias` attributes to pointer parameters
/// based on the declared ownership modifier:
/// - `share` / inferred (None) → `readonly` + `noalias`
/// - `lend` → `noalias` only
/// - `give` → no attributes (callee owns the data, may mutate)
fn declare_function<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    f: &FunctionDecl,
    shape_table: &ShapeTable,
) -> Result<(), String> {
    let params = llvm_param_types(ctx, f, shape_table);
    let fn_ty = if f.name == "entrypoint" {
        ctx.i32_type().fn_type(&params, false)
    } else if f.errors_capable {
        // M7 P4a: errors-capable functions return `{i64 error_ptr, i64 success_val}`.
        // field 0 = error pointer (0 = success, non-zero = *YnzError)
        // field 1 = success value as i64 (valid only when field 0 = 0)
        let result_ty = errors_result_type(ctx);
        result_ty.fn_type(&params, false)
    } else {
        match &f.return_type {
            ynz_ast::nodes::Type::Nothing => ctx.void_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Int => ctx.i64_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Float => ctx.f64_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Bool => ctx.bool_type().fn_type(&params, false),
            _ => ctx
                .ptr_type(AddressSpace::default())
                .fn_type(&params, false),
        }
    };
    // `entrypoint` is the Yinz name; the C ABI entry point must be `main` for the linker.
    let llvm_name = if f.name == "entrypoint" {
        "main"
    } else {
        &f.name
    };
    let fn_val = module.add_function(llvm_name, fn_ty, None);

    // Emit LLVM ownership attributes on pointer-typed parameters.
    let readonly_kind = Attribute::get_named_enum_kind_id("readonly");
    let noalias_kind = Attribute::get_named_enum_kind_id("noalias");

    for (i, param) in f.params.iter().enumerate() {
        if !is_ptr_param(&param.ty, shape_table) {
            continue;
        }
        let ownership = param
            .ownership
            .as_ref()
            .unwrap_or(&OwnershipModifier::Share);
        match ownership {
            OwnershipModifier::Share => {
                fn_val.add_attribute(
                    AttributeLoc::Param(i as u32),
                    ctx.create_enum_attribute(readonly_kind, 0),
                );
                fn_val.add_attribute(
                    AttributeLoc::Param(i as u32),
                    ctx.create_enum_attribute(noalias_kind, 0),
                );
            }
            OwnershipModifier::Lend => {
                fn_val.add_attribute(
                    AttributeLoc::Param(i as u32),
                    ctx.create_enum_attribute(noalias_kind, 0),
                );
            }
            OwnershipModifier::Give => {}
        }
    }

    Ok(())
}

/// LLVM struct type for errors-capable return values: `{i64, i64}`.
///
/// LLVM struct type for the `errors`-keyword return encoding: `{i64, i64}`.
///
/// # ABI contract
///
/// - `field 0` (i64): error pointer — `0` means success; non-zero is a `*YnzError`
///   heap pointer cast to i64.
/// - `field 1` (i64): success value bits — valid ONLY when `field 0 == 0`.
///   - Scalar types (int, bool, float): stored directly as i64.
///   - Pointer-typed success values (string, shape, array, …): the heap pointer is
///     cast to i64.  Callers must cast `field 1` back to the appropriate pointer
///     type before dereferencing.
fn errors_result_type(ctx: &Context) -> inkwell::types::StructType<'_> {
    ctx.struct_type(&[ctx.i64_type().into(), ctx.i64_type().into()], false)
}

/// True when the AST type will be passed as a pointer in LLVM (not a scalar value).
fn is_ptr_param(ty: &ynz_ast::nodes::Type, shape_table: &ShapeTable) -> bool {
    match ty {
        ynz_ast::nodes::Type::Int | ynz_ast::nodes::Type::Float | ynz_ast::nodes::Type::Bool => {
            false
        }
        ynz_ast::nodes::Type::Named(n, _)
            if !shape_table.contains(n) && n != "self" && n != "string" =>
        {
            false
        }
        _ => true,
    }
}

struct Cg<'ctx, 'g> {
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    builder: inkwell::builder::Builder<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    current_fn: FunctionValue<'ctx>,
    /// True when this function is `main` (affects return type and implicit ret).
    is_main: bool,
    /// Return type of the current function.
    _current_fn_ret_ty: Type,
    locals: HashMap<String, PointerValue<'ctx>>,
    // M4 additions:
    shape_table: &'g ShapeTable,
    shape_types: &'g ShapeLlvmTypes<'ctx>,
    // M5 P4a: type-param substitution for the current monomorphized instance.
    // Empty for non-generic functions.
    type_subst: HashMap<String, Type>,
    mono_table: &'g MonomorphizationTable,
    // M6: options table for variant tag lookup.
    options_table: &'g ynz_typeck::options_table::OptionsTable,
    // M7 P4a: true when the current function declared `-> T errors`.
    // Affects return-type wrapping and call-site auto-propagation.
    is_errors_capable: bool,
    // M7 P4a: set of local names that hold errors-capable results (pointer to {i64, i64}).
    // When one of these is first used in an errors-capable function, auto-propagation fires.
    errors_capable_locals: std::collections::HashSet<String>,
}

impl<'ctx, 'g> Cg<'ctx, 'g> {
    fn i64(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i64_type()
    }
    fn i128(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i128_type()
    }
    fn i32(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i32_type()
    }
    fn i8(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.i8_type()
    }
    fn f64(&self) -> inkwell::types::FloatType<'ctx> {
        self.ctx.f64_type()
    }
    fn bool(&self) -> inkwell::types::IntType<'ctx> {
        self.ctx.bool_type()
    }
    fn ptr(&self) -> inkwell::types::PointerType<'ctx> {
        self.ctx.ptr_type(AddressSpace::default())
    }

    /// Apply the current type-param substitution to `ty`, returning a concrete type.
    fn resolve_type(&self, ty: &Type) -> Type {
        match ty {
            Type::TypeParam { name } => self
                .type_subst
                .get(name)
                .cloned()
                .unwrap_or_else(|| ty.clone()),
            Type::Generic { name, args } => Type::Generic {
                name: name.clone(),
                args: args.iter().map(|a| self.resolve_type(a)).collect(),
            },
            Type::BuiltinArray { elem } => Type::BuiltinArray {
                elem: Box::new(self.resolve_type(elem)),
            },
            Type::BuiltinFixed { elem, size } => Type::BuiltinFixed {
                elem: Box::new(self.resolve_type(elem)),
                size: *size,
            },
            Type::Maybe { inner } => Type::Maybe {
                inner: Box::new(self.resolve_type(inner)),
            },
            other => other.clone(),
        }
    }

    /// Look up the typeck type for `expr` and apply the current type-param substitution.
    fn expr_type(&self, expr: &Expr) -> Type {
        let key = (expr.span().start, expr.span().end);
        let raw = self
            .typed
            .expr_types
            .get(&key)
            .cloned()
            .unwrap_or(Type::Error);
        self.resolve_type(&raw)
    }

    fn llvm_type_for(&self, ty: &Type) -> Option<BasicTypeEnum<'ctx>> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            Type::Int => Some(self.i64().into()),
            Type::Float => Some(self.f64().into()),
            Type::Bool => Some(self.bool().into()),
            // N ≤ 34: hardware decimal128 (i128). N > 34: bignum (ptr to decimal string).
            Type::Number { precision } if *precision <= 34 => Some(self.i128().into()),
            Type::Number { .. } => Some(self.ptr().into()),
            Type::String => Some(self.ptr().into()),
            // Shape and dynamic values are always passed/stored as opaque pointers.
            Type::Shape { .. } => Some(self.ptr().into()),
            Type::Dynamic { .. } => Some(self.ptr().into()),
            // Collections and maybe are all represented as opaque pointers (heap or alloca).
            Type::BuiltinArray { .. } => Some(self.ptr().into()),
            Type::BuiltinFixed { .. } => Some(self.ptr().into()),
            Type::Maybe { .. } => Some(self.ptr().into()),
            Type::MapEntry { .. } => Some(self.ptr().into()),
            Type::BuiltinMap { .. } => Some(self.ptr().into()),
            // M6: options values are i8 tags; union values are opaque pointers (heap tagged-struct).
            Type::Options { .. } => Some(self.ctx.i8_type().into()),
            Type::Union { .. } => Some(self.ptr().into()),
            // M7 P4c: Range values are {i64 start, i64 end} stored via a stack-alloca pointer.
            Type::Range { .. } => Some(self.ptr().into()),
            // M8 P4: sensitive values have the same ABI as their inner type (string ptr).
            Type::Sensitive { .. } => Some(self.ptr().into()),
            _ => None,
        }
    }

    /// LLVM struct type for `maybe<T>`: `{i64, i64}` where slot 0 = has_value, slot 1 = bits.
    fn maybe_type(&self) -> inkwell::types::StructType<'ctx> {
        self.ctx
            .struct_type(&[self.i64().into(), self.i64().into()], false)
    }

    fn alloca(&self, ty: &Type, name: &str) -> Result<PointerValue<'ctx>, String> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            // Collections and maybe: the variable slot holds an opaque pointer to the
            // actual data structure (heap YnzArray, stack [N x i64], or stack {i64,i64}).
            // Loading the slot returns the pointer; the caller works through that pointer.
            Type::BuiltinArray { .. }
            | Type::BuiltinFixed { .. }
            | Type::Maybe { .. }
            | Type::BuiltinMap { .. }
            | Type::MapEntry { .. }
            | Type::Union { .. } => self
                .builder
                .build_alloca(self.ptr(), name)
                .map_err(|e| format!("{e}")),
            // M7 P4a: ErrorsCapable stores a pointer to the {i64, i64} result struct alloca.
            Type::ErrorsCapable { .. } => self
                .builder
                .build_alloca(self.ptr(), name)
                .map_err(|e| format!("{e}")),
            // M7 P4c: Range stores a pointer to the {i64 start, i64 end} alloca.
            Type::Range { .. } => self
                .builder
                .build_alloca(self.ptr(), name)
                .map_err(|e| format!("{e}")),
            _ => {
                let llvm_ty = self
                    .llvm_type_for(&resolved)
                    .ok_or_else(|| format!("cannot alloca for type {:?}", resolved))?;
                self.builder
                    .build_alloca(llvm_ty, name)
                    .map_err(|e| format!("{e}"))
            }
        }
    }

    fn append_block(&self, name: &str) -> BasicBlock<'ctx> {
        self.ctx.append_basic_block(self.current_fn, name)
    }

    /// Build an alloca holding a `maybe<T>` with `has_value = 0`.
    fn build_maybe_none(&self) -> Result<PointerValue<'ctx>, String> {
        let slot = self
            .builder
            .build_alloca(self.maybe_type(), "maybe_none")
            .map_err(|e| format!("{e}"))?;
        self.builder
            .build_store(slot, self.maybe_type().const_zero())
            .map_err(|e| format!("{e}"))?;
        Ok(slot)
    }

    /// Build an alloca holding a `maybe<T>` with `has_value = 1` and `bits = value_i64`.
    #[allow(dead_code)]
    fn build_maybe_some(
        &self,
        value_i64: inkwell::values::IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
        let slot = self
            .builder
            .build_alloca(self.maybe_type(), "maybe_some")
            .map_err(|e| format!("{e}"))?;
        let one = self.i64().const_int(1, false);
        let has_gep = self
            .builder
            .build_struct_gep(self.maybe_type(), slot, 0, "has_gep")
            .map_err(|e| format!("{e}"))?;
        self.builder
            .build_store(has_gep, one)
            .map_err(|e| format!("{e}"))?;
        let val_gep = self
            .builder
            .build_struct_gep(self.maybe_type(), slot, 1, "val_gep")
            .map_err(|e| format!("{e}"))?;
        self.builder
            .build_store(val_gep, value_i64)
            .map_err(|e| format!("{e}"))?;
        Ok(slot)
    }

    /// Convert any BasicValueEnum to its i64-bit representation for uniform array storage.
    fn to_i64_bits(
        &self,
        val: BasicValueEnum<'ctx>,
        ty: &Type,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            Type::Int | Type::Bool => Ok(val.into_int_value()),
            Type::Float => self
                .builder
                .build_bit_cast(val.into_float_value(), self.i64(), "f_to_i")
                .map(|v| v.into_int_value())
                .map_err(|e| format!("{e}")),
            Type::String
            | Type::Number { .. }
            | Type::Shape { .. }
            | Type::Dynamic { .. }
            | Type::BuiltinArray { .. }
            | Type::BuiltinFixed { .. }
            | Type::Maybe { .. }
            | Type::BuiltinMap { .. }
            | Type::MapEntry { .. }
            | Type::Union { .. } => self
                .builder
                .build_ptr_to_int(val.into_pointer_value(), self.i64(), "ptr_to_i")
                .map_err(|e| format!("{e}")),
            _ => Err(format!("cannot convert {:?} to i64 bits", resolved)),
        }
    }

    /// Convert an i64-bit pattern back to the concrete type representation.
    fn i64_bits_to(
        &self,
        val: inkwell::values::IntValue<'ctx>,
        ty: &Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let resolved = self.resolve_type(ty);
        match &resolved {
            Type::Int => Ok(val.into()),
            Type::Bool => self
                .builder
                .build_int_truncate(val, self.bool(), "i_to_b")
                .map(|v| v.into())
                .map_err(|e| format!("{e}")),
            Type::Float => self
                .builder
                .build_bit_cast(val, self.f64(), "i_to_f")
                .map_err(|e| format!("{e}")),
            Type::String
            | Type::Number { .. }
            | Type::Shape { .. }
            | Type::Dynamic { .. }
            | Type::BuiltinArray { .. }
            | Type::BuiltinFixed { .. }
            | Type::Maybe { .. }
            | Type::BuiltinMap { .. }
            | Type::MapEntry { .. }
            | Type::Union { .. } => self
                .builder
                .build_int_to_ptr(val, self.ptr(), "i_to_ptr")
                .map(|v| v.into())
                .map_err(|e| format!("{e}")),
            _ => Err(format!("cannot convert i64 bits to {:?}", resolved)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_function<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    f: &FunctionDecl,
    shape_table: &'g ShapeTable,
    shape_types: &'g ShapeLlvmTypes<'ctx>,
    mono_table: &'g MonomorphizationTable,
    options_table: &'g ynz_typeck::options_table::OptionsTable,
) -> Result<(), String> {
    let llvm_name = if f.name == "entrypoint" {
        "main"
    } else {
        f.name.as_str()
    };
    let fn_val = module
        .get_function(llvm_name)
        .ok_or_else(|| format!("function `{}` was not forward-declared", f.name))?;

    let ret_ty = ast_type_to_typeck_type(&f.return_type, shape_table);
    let is_main = f.name == "entrypoint";
    let ret_is_nothing = matches!(ret_ty, Type::Nothing);
    let is_errors_capable = f.errors_capable;

    let mut cg = Cg {
        ctx,
        module,
        builder: ctx.create_builder(),
        rt,
        globals,
        typed,
        current_fn: fn_val,
        is_main,
        _current_fn_ret_ty: ret_ty,
        locals: HashMap::new(),
        shape_table,
        shape_types,
        type_subst: HashMap::new(),
        mono_table,
        options_table,
        is_errors_capable,
        errors_capable_locals: std::collections::HashSet::new(),
    };

    let entry = ctx.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);

    // Initialize SipHash key from OS entropy before any map operations.
    if is_main {
        cg.builder
            .build_call(cg.rt.ynz_siphash_init, &[], "siphash_init")
            .map_err(|e| format!("siphash_init: {e}"))?;
    }

    // M7 P4a: errors-capable functions push a frame on entry for the call trace.
    if is_errors_capable {
        let file_g = build_string_global(ctx, module, f.name.as_str(), ".ec.file");
        let fn_g = build_string_global(ctx, module, f.name.as_str(), ".ec.fn");
        cg.builder
            .build_call(
                cg.rt.ynz_frame_push,
                &[
                    file_g.as_pointer_value().into(),
                    cg.i64().const_int(0, false).into(),
                    fn_g.as_pointer_value().into(),
                ],
                "ec_push",
            )
            .map_err(|e| format!("frame_push: {e}"))?;
    }

    // Materialize each parameter as an alloca so the "every name = alloca" invariant holds.
    for (i, param) in f.params.iter().enumerate() {
        let llvm_param = fn_val
            .get_nth_param(i as u32)
            .ok_or_else(|| format!("missing LLVM param {} for `{}`", i, f.name))?;
        let param_ty = ast_type_to_typeck_type(&param.ty, shape_table);
        materialize_param(&mut cg, &param.name, llvm_param, &param_ty)?;
    }

    for stmt in &f.body.stmts {
        if is_block_terminated(&cg) {
            break;
        }
        lower_stmt(&mut cg, stmt)?;
    }

    // Implicit terminator if the current block has no terminator yet.
    //
    // - entrypoint: always ret i32 0 (C ABI entry point).
    // - nothing-returning functions: ret void (legitimate fall-off-the-end).
    // - errors-capable: implicit success return of zero value (typeck ensures all
    //   paths have explicit returns, so this block should be dead code; emit a
    //   safe zeroed success result instead of unreachable to help debugging).
    // - non-nothing functions: unreachable — typeck confirmed exhaustive paths.
    if !is_block_terminated(&cg) {
        if is_main {
            cg.builder
                .build_return(Some(&ctx.i32_type().const_int(0, false)))
                .map_err(|e| format!("implicit main ret: {e}"))?;
        } else if is_errors_capable {
            // Implicit success with zero-valued success field.
            cg.builder
                .build_call(cg.rt.ynz_frame_pop, &[], "ec_pop_implicit")
                .map_err(|e| format!("frame_pop: {e}"))?;
            let zero_result = errors_result_type(ctx).const_zero();
            cg.builder
                .build_return(Some(&zero_result))
                .map_err(|e| format!("implicit ec ret: {e}"))?;
        } else if ret_is_nothing {
            cg.builder
                .build_return(None)
                .map_err(|e| format!("implicit void ret: {e}"))?;
        } else {
            cg.builder
                .build_unreachable()
                .map_err(|e| format!("implicit unreachable: {e}"))?;
        }
    }
    Ok(())
}

/// Map an AST type annotation to the typeck `Type` for use in codegen decisions.
fn ast_type_to_typeck_type(ast_ty: &ynz_ast::nodes::Type, shape_table: &ShapeTable) -> Type {
    match ast_ty {
        ynz_ast::nodes::Type::Nothing => Type::Nothing,
        ynz_ast::nodes::Type::Int => Type::Int,
        ynz_ast::nodes::Type::Float => Type::Float,
        ynz_ast::nodes::Type::Bool => Type::Bool,
        ynz_ast::nodes::Type::Named(n, _) if n == "string" => Type::String,
        // M6: union type aliases.
        ynz_ast::nodes::Type::Named(n, _) if shape_table.union_aliases.contains_key(n) => {
            shape_table.union_aliases[n].clone()
        }
        ynz_ast::nodes::Type::Named(n, _) if shape_table.contains(n) => {
            Type::Shape { name: n.clone() }
        }
        ynz_ast::nodes::Type::Dynamic { contract, .. } => Type::Dynamic {
            contract: contract.clone(),
        },
        // `self` parameter and SelfType: the typeck stores the concrete Shape type in
        // expr_types; for parameter materialization we just need to know it's a ptr.
        ynz_ast::nodes::Type::Named(n, _) if n == "self" => Type::Shape {
            name: String::new(),
        },
        ynz_ast::nodes::Type::SelfType { .. } => Type::Shape {
            name: String::new(),
        },
        // M5 generic types — delegate to shape_table.resolve_ast_type for proper resolution.
        ynz_ast::nodes::Type::Generic { .. }
        | ynz_ast::nodes::Type::Maybe { .. }
        | ynz_ast::nodes::Type::TypeParam { .. } => shape_table.resolve_ast_type(ast_ty),
        // M6: union types in parameter position.
        ynz_ast::nodes::Type::Union { variants, .. } => {
            let resolved: Vec<Type> = variants
                .iter()
                .map(|v| ast_type_to_typeck_type(v, shape_table))
                .collect();
            if resolved.len() < 2 {
                Type::Error
            } else {
                Type::Union { variants: resolved }
            }
        }
        // M7 P1: `-> T errors` — defer to the inner type for codegen purposes.
        ynz_ast::nodes::Type::ErrorCapable { inner, .. } => {
            ast_type_to_typeck_type(inner, shape_table)
        }
        // M8 P4: `sensitive T` — preserve the Sensitive wrapper (ABI = ptr, same as string).
        ynz_ast::nodes::Type::Sensitive(inner) => {
            let inner_ty = ast_type_to_typeck_type(inner, shape_table);
            Type::Sensitive {
                inner: Box::new(inner_ty),
            }
        }
        // M8 P6: `number<N>` with explicit precision in parameter position.
        ynz_ast::nodes::Type::Number { precision } => Type::Number {
            precision: *precision,
        },
        _ => Type::Error,
    }
}

/// Materialize an LLVM function parameter as a named alloca.
///
/// Scalars (int, float, bool) are stored directly. Pointer params (string, number,
/// shape, dynamic) get a `ptr`-sized alloca that holds the incoming pointer, so the
/// "every local = alloca" invariant holds. Loading from that slot gives back the ptr.
fn materialize_param<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    name: &str,
    llvm_val: inkwell::values::BasicValueEnum<'ctx>,
    param_ty: &Type,
) -> Result<(), String> {
    match param_ty {
        // Pointer-typed params: incoming LLVM value is a ptr; store the ptr in a ptr-slot.
        Type::Shape { .. } | Type::Dynamic { .. }
        | Type::BuiltinArray { .. } | Type::BuiltinFixed { .. } | Type::Maybe { .. }
        | Type::BuiltinMap { .. } | Type::MapEntry { .. }
        // M6: union values are heap tagged-structs, passed/stored as opaque pointers.
        | Type::Union { .. } => {
            let slot = cg.builder.build_alloca(cg.ptr(), name)
                .map_err(|e| format!("param alloca {name}: {e}"))?;
            cg.builder.build_store(slot, llvm_val)
                .map_err(|e| format!("param store {name}: {e}"))?;
            cg.locals.insert(name.to_string(), slot);
        }
        _ => {
            let slot = cg.alloca(param_ty, name)?;
            store(cg, llvm_val, param_ty, slot)?;
            cg.locals.insert(name.to_string(), slot);
        }
    }
    Ok(())
}

/// True when the current basic block already has a terminator instruction.
///
/// Used to avoid emitting dead instructions after `ret`, `br`, or `unreachable`.
fn is_block_terminated(cg: &Cg) -> bool {
    cg.builder
        .get_insert_block()
        .map(|bb| bb.get_terminator().is_some())
        .unwrap_or(true)
}

fn lower_stmt<'ctx>(cg: &mut Cg<'ctx, '_>, stmt: &Stmt) -> Result<(), String> {
    match stmt {
        Stmt::Expr(expr) => {
            lower_expr(cg, expr)?;
        }

        Stmt::Let {
            name,
            ty: ann_ty,
            value,
            ..
        } => {
            let val_ty = cg.expr_type(value);
            let val = lower_expr(cg, value)?;

            // M6: when annotation is a union type and value is a concrete shape variant,
            // construct a tagged-struct { i64 tag, i64 data } on the stack.
            let union_constructed = 'union_ctor: {
                if let Some(ast_ann) = ann_ty.as_ref() {
                    let resolved_ann = ast_type_to_typeck_type(ast_ann, cg.shape_table);
                    if let Type::Union { ref variants } = resolved_ann {
                        if let Type::Shape {
                            name: ref shape_name,
                        } = val_ty
                        {
                            let tag = variants.iter().position(|v| {
                                if let Type::Shape { name } = v {
                                    name == shape_name
                                } else {
                                    false
                                }
                            });
                            if let Some(tag_idx) = tag {
                                let union_st = cg
                                    .ctx
                                    .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                                let union_slot = cg
                                    .builder
                                    .build_alloca(union_st, &format!("{name}_union"))
                                    .map_err(|e| format!("{e}"))?;
                                let tag_gep = cg
                                    .builder
                                    .build_struct_gep(union_st, union_slot, 0, "u_tag")
                                    .map_err(|e| format!("{e}"))?;
                                cg.builder
                                    .build_store(tag_gep, cg.i64().const_int(tag_idx as u64, false))
                                    .map_err(|e| format!("{e}"))?;
                                let data_gep = cg
                                    .builder
                                    .build_struct_gep(union_st, union_slot, 1, "u_data")
                                    .map_err(|e| format!("{e}"))?;
                                let ptr_as_i64 = cg
                                    .builder
                                    .build_ptr_to_int(val.into_pointer_value(), cg.i64(), "ptr2i")
                                    .map_err(|e| format!("{e}"))?;
                                cg.builder
                                    .build_store(data_gep, ptr_as_i64)
                                    .map_err(|e| format!("{e}"))?;
                                let outer_slot = cg
                                    .builder
                                    .build_alloca(cg.ptr(), name)
                                    .map_err(|e| format!("{e}"))?;
                                cg.builder
                                    .build_store(outer_slot, union_slot)
                                    .map_err(|e| format!("{e}"))?;
                                cg.locals.insert(name.clone(), outer_slot);
                                break 'union_ctor true;
                            }
                        }
                    }
                }
                false
            };
            if !union_constructed {
                let slot = cg.alloca(&val_ty, name)?;
                store(cg, val, &val_ty, slot)?;
                cg.locals.insert(name.clone(), slot);
                // M7 P4a: track bindings that hold errors-capable results.
                if matches!(val_ty, Type::ErrorsCapable { .. }) {
                    cg.errors_capable_locals.insert(name.clone());
                }
            }
        }

        Stmt::Assign { target, value, .. } => {
            let slot = *cg
                .locals
                .get(target.as_str())
                .ok_or_else(|| format!("undefined `{target}` in codegen"))?;
            let ty = cg.expr_type(value);
            let val = lower_expr(cg, value)?;
            store(cg, val, &ty, slot)?;
        }

        Stmt::If { cond, body, .. } => {
            lower_stmt_if(cg, cond, body)?;
        }

        Stmt::Match {
            scrutinee,
            arms,
            else_arm,
            ..
        } => {
            lower_stmt_match(cg, scrutinee, arms, else_arm.as_ref())?;
        }

        Stmt::While { cond, body, .. } => {
            lower_stmt_while(cg, cond, body)?;
        }

        Stmt::For {
            var, iter, body, ..
        } => {
            lower_stmt_for(cg, var, iter, body)?;
        }

        Stmt::Return { value, .. } => {
            lower_stmt_return(cg, value.as_ref())?;
        }

        Stmt::FieldAssign { target, value, .. } => {
            lower_stmt_field_assign(cg, target, value)?;
        }

        Stmt::IndexAssign {
            receiver,
            index,
            value,
            ..
        } => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;
            let elem_val = lower_expr(cg, value)?;
            let elem_ty = cg.expr_type(value);
            let bits = cg.to_i64_bits(elem_val, &elem_ty)?;

            if let Type::BuiltinMap { key, .. } = &recv_ty {
                let key_ty = key.as_ref().clone();
                let key_val = lower_expr(cg, index)?;
                if key_is_string(&key_ty) {
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set_str,
                            &[recv_val.into(), key_val.into(), bits.into()],
                            "mia_s",
                        )
                        .map_err(|e| format!("{e}"))?;
                } else {
                    let kt = cg.expr_type(index);
                    let kb = cg.to_i64_bits(key_val, &kt)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set,
                            &[recv_val.into(), kb.into(), bits.into()],
                            "mia",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
            } else {
                let idx_val = lower_expr(cg, index)?.into_int_value();
                match &recv_ty {
                    Type::BuiltinArray { .. } => {
                        cg.builder
                            .build_call(
                                cg.rt.ynz_array_set,
                                &[recv_val.into(), idx_val.into(), bits.into()],
                                "arr_set",
                            )
                            .map_err(|e| format!("array_set: {e}"))?;
                    }
                    Type::BuiltinFixed { size, .. } => {
                        let arr_ptr = recv_val.into_pointer_value();
                        let gep = unsafe {
                            cg.builder
                                .build_gep(cg.i64(), arr_ptr, &[idx_val], "fixed_set_elem")
                                .map_err(|e| format!("fixed_set gep: {e}"))?
                        };
                        cg.builder
                            .build_store(gep, bits)
                            .map_err(|e| format!("{e}"))?;
                        let _ = size;
                    }
                    _ => return Err(format!("IndexAssign on unsupported type: {:?}", recv_ty)),
                }
            }
        }
    }
    Ok(())
}

fn lower_stmt_if<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    cond: &Expr,
    body: &ynz_ast::nodes::Block,
) -> Result<(), String> {
    let cond_val = lower_expr(cg, cond)?.into_int_value();
    let then_bb = cg.append_block("if_then");
    let merge_bb = cg.append_block("if_merge");

    cg.builder
        .build_conditional_branch(cond_val, then_bb, merge_bb)
        .map_err(|e| format!("if branch: {e}"))?;

    cg.builder.position_at_end(then_bb);
    for stmt in &body.stmts {
        if is_block_terminated(cg) {
            break;
        }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        cg.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(merge_bb);
    Ok(())
}

fn lower_stmt_match<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    scrutinee: &Expr,
    arms: &[ynz_ast::nodes::MatchArm],
    else_arm: Option<&ynz_ast::nodes::Block>,
) -> Result<(), String> {
    let scrutinee_ty = cg.expr_type(scrutinee);
    let scrutinee_val = lower_expr(cg, scrutinee)?;

    let merge_bb = cg.append_block("match_merge");
    let final_fallthrough_bb = if else_arm.is_some() {
        cg.append_block("match_else")
    } else {
        merge_bb
    };

    for (i, arm) in arms.iter().enumerate() {
        let arm_body_bb = cg.append_block(&format!("match_arm{i}"));
        let next_check_bb = if i + 1 < arms.len() {
            cg.append_block(&format!("match_check{}", i + 1))
        } else {
            final_fallthrough_bb
        };

        let pat_cond = match &arm.pattern.kind {
            MatchPatternKind::Value(pat_expr) => {
                let pat_val = lower_expr(cg, pat_expr)?;
                match_cmp(cg, &scrutinee_ty, scrutinee_val, pat_val)?
            }
            // M6: options variant arm — compare i8 tag
            MatchPatternKind::OptionName(variant_name) => {
                if let Type::Options { name: opts_name } = &scrutinee_ty {
                    if let Some(entry) = cg.options_table.options.get(opts_name.as_str()) {
                        let tag = entry.variants.iter().position(|v| v == variant_name)
                            .ok_or_else(|| format!("codegen: unknown variant `{variant_name}` in options `{opts_name}`"))? as u64;
                        let tag_val = cg.ctx.i8_type().const_int(tag, false);
                        cg.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                scrutinee_val.into_int_value(),
                                tag_val,
                                "opt_arm_cmp",
                            )
                            .map_err(|e| format!("{e}"))?
                    } else {
                        return Err(format!("codegen: unknown options type `{opts_name}`"));
                    }
                } else {
                    return Err(format!(
                        "codegen: OptionName arm on non-options type {:?}",
                        scrutinee_ty
                    ));
                }
            }
            // M6: union type-narrowing arm — compare tag in { i64 tag, i64 data } struct
            MatchPatternKind::Is(type_path) => {
                if let Type::Union { variants } = &scrutinee_ty {
                    let tag = variants
                        .iter()
                        .position(|v| {
                            if let Type::Shape { name } = v {
                                name == &type_path.name
                            } else {
                                false
                            }
                        })
                        .ok_or_else(|| {
                            format!("codegen: union variant `{}` not found", type_path.name)
                        })? as u64;
                    let tag_const = cg.i64().const_int(tag, false);
                    // Union layout: { i64 tag, i64 data }. scrutinee_val is ptr-to-struct.
                    // (It was loaded from the slot by lower_expr, giving us the struct ptr directly.)
                    let union_st = cg
                        .ctx
                        .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                    let tag_gep = cg
                        .builder
                        .build_struct_gep(
                            union_st,
                            scrutinee_val.into_pointer_value(),
                            0,
                            "union_tag_gep",
                        )
                        .map_err(|e| format!("union tag gep: {e}"))?;
                    let tag_loaded = cg
                        .builder
                        .build_load(cg.i64(), tag_gep, "union_tag")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            tag_loaded.into_int_value(),
                            tag_const,
                            "union_arm_cmp",
                        )
                        .map_err(|e| format!("{e}"))?
                } else {
                    return Err(format!(
                        "codegen: Is arm on non-union type {:?}",
                        scrutinee_ty
                    ));
                }
            }
        };

        cg.builder
            .build_conditional_branch(pat_cond, arm_body_bb, next_check_bb)
            .map_err(|e| format!("match branch: {e}"))?;

        cg.builder.position_at_end(arm_body_bb);
        for stmt in &arm.body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(next_check_bb);
    }

    // Emit else body (current position is final_fallthrough_bb or merge_bb).
    if let Some(else_body) = else_arm {
        for stmt in &else_body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;
        }
        cg.builder.position_at_end(merge_bb);
    }
    // If no else_arm: current position is already merge_bb (final_fallthrough_bb == merge_bb).

    Ok(())
}

/// Compare `scrutinee_val` against `pattern_val` for equality, returning `i1`.
fn match_cmp<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    scrutinee_ty: &Type,
    scrutinee_val: BasicValueEnum<'ctx>,
    pattern_val: BasicValueEnum<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    match scrutinee_ty {
        Type::Int | Type::Bool => cg
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                scrutinee_val.into_int_value(),
                pattern_val.into_int_value(),
                "match_eq",
            )
            .map_err(|e| format!("{e}")),
        Type::Float => cg
            .builder
            .build_float_compare(
                inkwell::FloatPredicate::OEQ,
                scrutinee_val.into_float_value(),
                pattern_val.into_float_value(),
                "fmatch",
            )
            .map_err(|e| format!("{e}")),
        Type::String => {
            let call = cg
                .builder
                .build_call(
                    cg.rt.string_eq,
                    &[scrutinee_val.into(), pattern_val.into()],
                    "str_eq",
                )
                .map_err(|e| format!("{e}"))?;
            let result = call
                .try_as_basic_value()
                .basic()
                .ok_or("string_eq void")?
                .into_int_value();
            cg.builder
                .build_int_compare(
                    IntPredicate::NE,
                    result,
                    cg.i32().const_int(0, false),
                    "str_eq_bool",
                )
                .map_err(|e| format!("{e}"))
        }
        Type::Number { .. } => {
            let c = cg
                .builder
                .build_call(
                    cg.rt.decimal_compare,
                    &[scrutinee_val.into(), pattern_val.into()],
                    "dcmp",
                )
                .map_err(|e| format!("{e}"))?;
            let ci = c
                .try_as_basic_value()
                .basic()
                .ok_or("dcmp void")?
                .into_int_value();
            cg.builder
                .build_int_compare(IntPredicate::EQ, ci, cg.i32().const_int(0, false), "deq")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!("codegen: match on unsupported type {:?}", other)),
    }
}

fn lower_stmt_while<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    cond: &Expr,
    body: &ynz_ast::nodes::Block,
) -> Result<(), String> {
    let header_bb = cg.append_block("while_header");
    let body_bb = cg.append_block("while_body");
    let exit_bb = cg.append_block("while_exit");

    cg.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(header_bb);
    let cond_val = lower_expr(cg, cond)?.into_int_value();
    cg.builder
        .build_conditional_branch(cond_val, body_bb, exit_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(body_bb);
    for stmt in &body.stmts {
        if is_block_terminated(cg) {
            break;
        }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(exit_bb);
    Ok(())
}

fn lower_stmt_for<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    var: &str,
    iter: &Expr,
    body: &ynz_ast::nodes::Block,
) -> Result<(), String> {
    let iter_ty = cg.expr_type(iter);

    // M7 P4c: string iteration — `for (c in s)` where s: string.
    // Index-based: count code points, then loop 0..count calling ynz_string_codepoint_at.
    if matches!(iter_ty, Type::String) {
        let s_ptr = lower_expr(cg, iter)?.into_pointer_value();
        let cnt = cg
            .builder
            .build_call(cg.rt.ynz_string_count, &[s_ptr.into()], "si_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("string_count void")?
            .into_int_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "si_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("si_cond");
        let body_bb = cg.append_block("si_body");
        let after_bb = cg.append_block("si_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;
        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "si_i_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, cnt, "si_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let ch_ptr = cg
            .builder
            .build_call(
                cg.rt.ynz_string_codepoint_at,
                &[s_ptr.into(), i.into()],
                "si_cp",
            )
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("codepoint_at void")?;
        let var_slot = cg.alloca(&Type::String, var)?;
        cg.builder
            .build_store(var_slot, ch_ptr)
            .map_err(|e| format!("{e}"))?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "si_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.locals.remove(var);
        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // M7 P4c: user shape iteration — `for (x in obj)` where obj: Shape.
    // Calls the standalone `next(lend self: Shape) -> maybe<T>` function.
    if let Type::Shape { name: shape_name } = &iter_ty {
        let shape_name = shape_name.clone();
        let obj_ptr = lower_expr(cg, iter)?;

        let next_fn = cg.module.get_function("next").ok_or_else(|| {
            format!("codegen: shape `{shape_name}` follows Iterable<T> but `next` is not compiled")
        })?;

        let cond_bb = cg.append_block("uf_cond");
        let body_bb = cg.append_block("uf_body");
        let after_bb = cg.append_block("uf_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;
        cg.builder.position_at_end(cond_bb);

        // Call next(&obj) → maybe<T> (stored in a fresh alloca returned as ptr).
        let maybe_slot_ptr = cg
            .builder
            .build_call(next_fn, &[obj_ptr.into()], "uf_next")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("next() returned void")?
            .into_pointer_value();

        // Check has_value (slot 0).
        let tag_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), maybe_slot_ptr, 0, "uf_tag")
            .map_err(|e| format!("{e}"))?;
        let tag = cg
            .builder
            .build_load(cg.i64(), tag_gep, "uf_tag_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let has = cg
            .builder
            .build_int_compare(IntPredicate::NE, tag, cg.i64().const_zero(), "uf_has")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(has, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        // Extract the value (slot 1) and determine its type by looking at the typeck.
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), maybe_slot_ptr, 1, "uf_val")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "uf_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        // The element type is Int (as the most common case in our fixtures).
        // For P4c we emit the loop var as an Int slot; the compiler already typechecked the type.
        let var_slot = cg.alloca(&Type::Int, var)?;
        cg.builder
            .build_store(var_slot, bits)
            .map_err(|e| format!("{e}"))?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        cg.locals.remove(var);
        if !is_block_terminated(cg) {
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Array iteration: `for (x in arr)` where arr: array<T>.
    if let Type::BuiltinArray { elem } = &iter_ty {
        let elem = elem.as_ref().clone();
        let arr_ptr = lower_expr(cg, iter)?.into_pointer_value();
        let cnt = cg
            .builder
            .build_call(cg.rt.ynz_array_count, &[arr_ptr.into()], "for_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("array_count void")?
            .into_int_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "for_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("for_cond");
        let body_bb = cg.append_block("for_body");
        let after_bb = cg.append_block("for_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "i")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let cmp = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, cnt, "for_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(cmp, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let out = cg
            .builder
            .build_alloca(cg.maybe_type(), "for_get")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_call(
                cg.rt.ynz_array_get,
                &[arr_ptr.into(), i.into(), out.into()],
                "for_get_call",
            )
            .map_err(|e| format!("{e}"))?;
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), out, 1, "for_val")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "for_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let elem_val = cg.i64_bits_to(bits, &elem)?;

        let var_slot = cg.alloca(&elem, var)?;
        store(cg, elem_val, &elem, var_slot)?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "next_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.locals.remove(var);
        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Fixed array iteration: `for (x in arr)` where arr: fixed<T>.
    // Identical to BuiltinArray but uses compile-time size + direct GEP instead of runtime calls.
    if let Type::BuiltinFixed { elem, size } = &iter_ty {
        let elem = elem.as_ref().clone();
        let n = match size {
            Some(n) => *n as u64,
            None => return Err("codegen: cannot iterate fixed array with unknown size".to_string()),
        };
        let size_val = cg.i64().const_int(n, false);
        let arr_ptr = lower_expr(cg, iter)?.into_pointer_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "ff_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("ff_cond");
        let body_bb = cg.append_block("ff_body");
        let after_bb = cg.append_block("ff_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "ff_i_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, size_val, "ff_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let elem_gep = unsafe {
            cg.builder
                .build_gep(cg.i64(), arr_ptr, &[i], "ff_gep")
                .map_err(|e| format!("{e}"))?
        };
        let bits = cg
            .builder
            .build_load(cg.i64(), elem_gep, "ff_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let elem_val = cg.i64_bits_to(bits, &elem)?;
        let var_slot = cg.alloca(&elem, var)?;
        store(cg, elem_val, &elem, var_slot)?;
        cg.locals.insert(var.to_string(), var_slot);

        for stmt in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, stmt)?;
        }

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "ff_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.locals.remove(var);
        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Map iteration: `for (entry in m)` where m: map<K,V>.
    // Iterates by insertion order; loop var has type MapEntry {key_bits, val_bits}.
    if let Type::BuiltinMap { key, val } = &iter_ty {
        let key_ty = key.as_ref().clone();
        let _val_ty = val.as_ref().clone();
        let map_ptr = lower_expr(cg, iter)?.into_pointer_value();

        let cnt = cg
            .builder
            .build_call(cg.rt.ynz_map_count, &[map_ptr.into()], "mf_cnt")
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("map_count void")?
            .into_int_value();

        let i_slot = cg
            .builder
            .build_alloca(cg.i64(), "mf_i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(i_slot, cg.i64().const_zero())
            .map_err(|e| format!("{e}"))?;

        // Loop var slot: {i64 key_bits, i64 val_bits}
        let entry_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into()], false);
        let entry_slot = cg
            .builder
            .build_alloca(entry_ty, var)
            .map_err(|e| format!("{e}"))?;

        let cond_bb = cg.append_block("mfor_cond");
        let body_bb = cg.append_block("mfor_body");
        let after_bb = cg.append_block("mfor_after");

        cg.builder
            .build_unconditional_branch(cond_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(cond_bb);
        let i = cg
            .builder
            .build_load(cg.i64(), i_slot, "mf_i_v")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let lt = cg
            .builder
            .build_int_compare(IntPredicate::SLT, i, cnt, "mf_lt")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_conditional_branch(lt, body_bb, after_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(body_bb);
        let triple_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into(), cg.i64().into()], false);
        let triple_slot = cg
            .builder
            .build_alloca(triple_ty, "mf_triple")
            .map_err(|e| format!("{e}"))?;
        if key_is_string(&key_ty) {
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get_str,
                    &[map_ptr.into(), i.into(), triple_slot.into()],
                    "mf_iter_s",
                )
                .map_err(|e| format!("{e}"))?;
        } else {
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get,
                    &[map_ptr.into(), i.into(), triple_slot.into()],
                    "mf_iter",
                )
                .map_err(|e| format!("{e}"))?;
        }
        // triple = {has, key, val} — copy key[1] and val[2] into entry_slot
        let k_src = cg
            .builder
            .build_struct_gep(triple_ty, triple_slot, 1, "mf_ks")
            .map_err(|e| format!("{e}"))?;
        let v_src = cg
            .builder
            .build_struct_gep(triple_ty, triple_slot, 2, "mf_vs")
            .map_err(|e| format!("{e}"))?;
        let k_dst = cg
            .builder
            .build_struct_gep(entry_ty, entry_slot, 0, "mf_kd")
            .map_err(|e| format!("{e}"))?;
        let v_dst = cg
            .builder
            .build_struct_gep(entry_ty, entry_slot, 1, "mf_vd")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(
                k_dst,
                cg.builder
                    .build_load(cg.i64(), k_src, "mf_kv")
                    .map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(
                v_dst,
                cg.builder
                    .build_load(cg.i64(), v_src, "mf_vv")
                    .map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?;

        cg.locals.insert(var.to_string(), entry_slot);

        for s in &body.stmts {
            if is_block_terminated(cg) {
                break;
            }
            lower_stmt(cg, s)?;
        }

        cg.locals.remove(var);

        if !is_block_terminated(cg) {
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "mf_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(after_bb);
        return Ok(());
    }

    // Range iteration: `for (x in range(n))` — original inline-range path,
    // OR `for (x in r)` where r is a stored Range value (M7 P4c first-class range).
    let (start_val, end_val) =
        if matches!(iter_ty, Type::Range { .. }) && !matches!(iter, Expr::Call(_)) {
            // Stored range variable: load the {i64, i64} struct pointer and extract fields.
            let range_ptr_val = lower_expr(cg, iter)?;
            let range_struct_ptr = range_ptr_val.into_pointer_value();
            let range_ty = cg
                .ctx
                .struct_type(&[cg.i64().into(), cg.i64().into()], false);
            let s_gep = cg
                .builder
                .build_struct_gep(range_ty, range_struct_ptr, 0, "r_s")
                .map_err(|e| format!("{e}"))?;
            let e_gep = cg
                .builder
                .build_struct_gep(range_ty, range_struct_ptr, 1, "r_e")
                .map_err(|e| format!("{e}"))?;
            let s = cg
                .builder
                .build_load(cg.i64(), s_gep, "r_sv")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let e = cg
                .builder
                .build_load(cg.i64(), e_gep, "r_ev")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            (s, e)
        } else {
            extract_range_bounds(cg, iter)?
        };

    let counter_slot = cg
        .builder
        .build_alloca(cg.i64(), "for_ctr")
        .map_err(|e| format!("{e}"))?;
    let end_slot = cg
        .builder
        .build_alloca(cg.i64(), "for_end")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(counter_slot, start_val)
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(end_slot, end_val)
        .map_err(|e| format!("{e}"))?;

    // Loop variable alloca — loop var is typed as int.
    let var_slot = cg
        .builder
        .build_alloca(cg.i64(), var)
        .map_err(|e| format!("{e}"))?;
    cg.locals.insert(var.to_string(), var_slot);

    let header_bb = cg.append_block("for_header");
    let body_bb = cg.append_block("for_body");
    let exit_bb = cg.append_block("for_exit");

    cg.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| format!("{e}"))?;

    // Header: check counter < end.
    cg.builder.position_at_end(header_bb);
    let ctr = cg
        .builder
        .build_load(cg.i64(), counter_slot, "ctr")
        .map_err(|e| format!("{e}"))?
        .into_int_value();
    let end = cg
        .builder
        .build_load(cg.i64(), end_slot, "end")
        .map_err(|e| format!("{e}"))?
        .into_int_value();
    let in_range = cg
        .builder
        .build_int_compare(IntPredicate::SLT, ctr, end, "for_cond")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(in_range, body_bb, exit_bb)
        .map_err(|e| format!("{e}"))?;

    // Body: bind loop var, emit stmts, increment, back-edge.
    cg.builder.position_at_end(body_bb);
    let ctr_bind = cg
        .builder
        .build_load(cg.i64(), counter_slot, "ctr_bind")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(var_slot, ctr_bind)
        .map_err(|e| format!("{e}"))?;

    for stmt in &body.stmts {
        if is_block_terminated(cg) {
            break;
        }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        let ctr_cur = cg
            .builder
            .build_load(cg.i64(), counter_slot, "ctr_cur")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        let one = cg.i64().const_int(1, false);
        let ctr_next = cg
            .builder
            .build_int_add(ctr_cur, one, "ctr_next")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(counter_slot, ctr_next)
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_unconditional_branch(header_bb)
            .map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(exit_bb);
    cg.locals.remove(var);
    Ok(())
}

fn lower_stmt_return<'ctx>(cg: &mut Cg<'ctx, '_>, value: Option<&Expr>) -> Result<(), String> {
    if cg.is_main {
        cg.builder
            .build_return(Some(&cg.i32().const_int(0, false)))
            .map_err(|e| format!("main ret: {e}"))?;
        return Ok(());
    }
    if cg.is_errors_capable {
        // M7 P4a: errors-capable return wraps the success value in {0, success_bits}.
        // Always pop the frame before returning.
        cg.builder
            .build_call(cg.rt.ynz_frame_pop, &[], "ec_pop")
            .map_err(|e| format!("frame_pop on return: {e}"))?;
        match value {
            None => {
                // `-> nothing errors` function returning without a value.
                let zero_result = errors_result_type(cg.ctx).const_zero();
                cg.builder
                    .build_return(Some(&zero_result))
                    .map_err(|e| format!("ec void ret: {e}"))?;
            }
            Some(expr) => {
                let val = lower_expr(cg, expr)?;
                let val_ty = cg.expr_type(expr);
                let success_bits = cg
                    .to_i64_bits(val, &val_ty)
                    .unwrap_or_else(|_| cg.i64().const_int(0, false));
                let result_ty = errors_result_type(cg.ctx);
                let mut result = result_ty.const_zero();
                result = cg
                    .builder
                    .build_insert_value(result, cg.i64().const_int(0, false), 0, "ec_err0")
                    .map_err(|e| format!("ec insert err: {e}"))?
                    .into_struct_value();
                result = cg
                    .builder
                    .build_insert_value(result, success_bits, 1, "ec_val")
                    .map_err(|e| format!("ec insert val: {e}"))?
                    .into_struct_value();
                cg.builder
                    .build_return(Some(&result))
                    .map_err(|e| format!("ec success ret: {e}"))?;
            }
        }
        return Ok(());
    }
    match value {
        None => {
            cg.builder
                .build_return(None)
                .map_err(|e| format!("void ret: {e}"))?;
        }
        Some(expr) => {
            let val = lower_expr(cg, expr)?;
            cg.builder
                .build_return(Some(&val))
                .map_err(|e| format!("ret: {e}"))?;
        }
    }
    Ok(())
}

/// Extract the start and end `i64` values from a `range(end)` or `range(start, end)` call.
fn extract_range_bounds<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    iter: &Expr,
) -> Result<
    (
        inkwell::values::IntValue<'ctx>,
        inkwell::values::IntValue<'ctx>,
    ),
    String,
> {
    let Expr::Call(call) = iter else {
        return Err("for-loop iter is not a call expression".to_string());
    };
    let Expr::Ident(name, _) = &call.callee else {
        return Err("for-loop iter callee is not an identifier".to_string());
    };
    if name != "range" {
        return Err(format!("for-loop iter calls `{name}`, expected `range`"));
    }
    match call.args.len() {
        1 => {
            let end = lower_expr(cg, &call.args[0])?.into_int_value();
            Ok((cg.i64().const_int(0, false), end))
        }
        2 => {
            let start = lower_expr(cg, &call.args[0])?.into_int_value();
            let end = lower_expr(cg, &call.args[1])?.into_int_value();
            Ok((start, end))
        }
        n => Err(format!("range takes 1 or 2 args, got {n}")),
    }
}

fn lower_expr<'ctx>(cg: &mut Cg<'ctx, '_>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, String> {
    match expr {
        Expr::IntLit(n, _) => Ok(cg.i64().const_int(*n as u64, true).into()),

        Expr::NumberLit(s, _) => {
            let ty = cg.expr_type(expr);
            // M8 P6: bignum path for N > 34.
            if let Type::Number { precision } = &ty {
                if *precision > 34 {
                    let prec16 = (*precision).min(4096) as u16;
                    let bn = ynz_numerics::decimal_n::parse_bignum(s, prec16)
                        .unwrap_or_else(|| ynz_numerics::BigNum::zero(prec16));
                    let formatted = ynz_numerics::decimal_n::format_bignum(&bn);
                    let global = build_string_global(
                        cg.ctx,
                        cg.module,
                        &formatted,
                        &format!(".bignum.lit.{}", &s[..s.len().min(8)]),
                    );
                    return Ok(global.as_pointer_value().into());
                }
            }
            // Hardware decimal128 path (N ≤ 34).
            let bits: u128 =
                ynz_numerics::parse(s).ok_or_else(|| format!("bad decimal literal `{s}`"))?;
            let slot = cg
                .builder
                .build_alloca(cg.i128(), "dec_lit")
                .map_err(|e| format!("{e}"))?;
            let const_val = cg.i128().const_int_arbitrary_precision(&[
                (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
                (bits >> 64) as u64,
            ]);
            cg.builder
                .build_store(slot, const_val)
                .map_err(|e| format!("{e}"))?;
            Ok(slot.into())
        }

        Expr::BoolLit(b, _) => Ok(cg.bool().const_int(*b as u64, false).into()),

        Expr::StringLit(bytes, _) => {
            let mut null = bytes.clone();
            push_c_string_terminator(&mut null);
            let i8t = cg.i8();
            let arr_ty = i8t.array_type(null.len() as u32);
            let arr = i8t.const_array(
                &null
                    .iter()
                    .map(|&b| i8t.const_int(b as u64, false))
                    .collect::<Vec<_>>(),
            );
            let g = cg
                .module
                .add_global(arr_ty, Some(AddressSpace::default()), "str");
            g.set_initializer(&arr);
            g.set_constant(true);
            g.set_linkage(inkwell::module::Linkage::Private);
            g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
            Ok(g.as_pointer_value().into())
        }

        Expr::Ident(name, _) => {
            let slot = *cg
                .locals
                .get(name.as_str())
                .ok_or_else(|| format!("undefined `{name}` in codegen"))?;
            let ty = cg.expr_type(expr);

            // M7 P4a: handle errors-capable locals.
            //
            // A local in `errors_capable_locals` holds a pointer to {i64, i64} result struct.
            // The typeck's expr_type may be:
            //   - `ErrorsCapable { inner }` — still unhandled; load ptr for .or()/.failed()
            //   - inner type (String, int, etc.) — typeck narrowed it; extract success value
            //
            // Auto-propagation fires here when the caller IS errors-capable AND the type
            // is still ErrorsCapable (typeck hasn't narrowed it yet).
            if cg.errors_capable_locals.contains(name.as_str()) {
                if matches!(ty, Type::ErrorsCapable { .. }) && cg.is_errors_capable {
                    // Auto-propagation: load struct, check error, early-return or yield success.
                    let ec_ptr = cg
                        .builder
                        .build_load(cg.ptr(), slot, "ec_id_ptr")
                        .map_err(|e| format!("ec ident load: {e}"))?
                        .into_pointer_value();
                    let result_ty = errors_result_type(cg.ctx);
                    let result_struct = cg
                        .builder
                        .build_load(result_ty, ec_ptr, "ec_id_struct")
                        .map_err(|e| format!("ec ident struct load: {e}"))?
                        .into_struct_value();
                    cg.errors_capable_locals.remove(name.as_str());
                    let inner_ty = if let Type::ErrorsCapable { inner } = &ty {
                        *inner.clone()
                    } else {
                        ty.clone()
                    };
                    let success_val = lower_ec_auto_propagate(cg, result_struct, &inner_ty)?;
                    let new_slot = cg.alloca(&inner_ty, &format!("{name}_ec_inner"))?;
                    store(cg, success_val, &inner_ty, new_slot)?;
                    cg.locals.insert(name.to_string(), new_slot);
                    return Ok(success_val);
                } else if !matches!(ty, Type::ErrorsCapable { .. }) {
                    // Typeck narrowed the binding to the inner type (after a .failed() check).
                    // Extract the success value from the stored struct and update the slot.
                    let ec_ptr = cg
                        .builder
                        .build_load(cg.ptr(), slot, "ec_id_ptr")
                        .map_err(|e| format!("ec ident narrow load: {e}"))?
                        .into_pointer_value();
                    let result_ty = errors_result_type(cg.ctx);
                    let success_gep = cg
                        .builder
                        .build_struct_gep(result_ty, ec_ptr, 1, "ec_narrow_val")
                        .map_err(|e| format!("ec narrow gep: {e}"))?;
                    let bits = cg
                        .builder
                        .build_load(cg.i64(), success_gep, "ec_narrow_bits")
                        .map_err(|e| format!("ec narrow bits: {e}"))?
                        .into_int_value();
                    let success_val = cg.i64_bits_to(bits, &ty)?;
                    // Update the slot so future uses don't do this extraction again.
                    cg.errors_capable_locals.remove(name.as_str());
                    let new_slot = cg.alloca(&ty, &format!("{name}_ec_narrowed"))?;
                    store(cg, success_val, &ty, new_slot)?;
                    cg.locals.insert(name.to_string(), new_slot);
                    return Ok(success_val);
                }
                // If ErrorsCapable AND not errors-capable caller: fall through to load
                // which returns the ptr for .or()/.failed() method dispatch.
            }

            load(cg, slot, &ty, name)
        }

        Expr::BinOp { op, lhs, rhs, .. } => {
            let lhs_ty = cg.expr_type(lhs);
            let rhs_ty = cg.expr_type(rhs);
            lower_binop(cg, op, lhs, rhs, &lhs_ty, &rhs_ty)
        }

        Expr::UnaryOp { op, operand, .. } => {
            let ty = cg.expr_type(operand);
            let val = lower_expr(cg, operand)?;
            lower_unary(cg, op, val, &ty)
        }

        Expr::Call(call) => {
            let Expr::Ident(fn_name, _) = &call.callee else {
                return Err("codegen: call to non-identifier callee".to_string());
            };
            let fn_name = fn_name.clone();
            match fn_name.as_str() {
                "print" if call.args.len() == 1 => {
                    let ty = cg.expr_type(&call.args[0]);
                    let val = lower_expr(cg, &call.args[0])?;
                    lower_print(cg, val, &ty)?;
                    Ok(cg.i32().const_int(0, false).into())
                }
                // M8 P4: `sensitive(value)` — identity at the ABI level (string ptr passes through).
                // The type system already marked the return type as Sensitive; codegen just returns
                // the underlying value unchanged.
                "sensitive" if call.args.len() == 1 => {
                    let val = lower_expr(cg, &call.args[0])?;
                    Ok(val)
                }
                "range" => {
                    // M7 P4c: range() as a first-class value — produces a {i64 start, i64 end}
                    // alloca on the stack.  The pointer to that alloca is returned so the range
                    // can be stored in a variable binding and later used as a for-loop iter.
                    let (start_v, end_v) = match call.args.len() {
                        1 => {
                            let e = lower_expr(cg, &call.args[0])?.into_int_value();
                            (cg.i64().const_zero(), e)
                        }
                        2 => {
                            let s = lower_expr(cg, &call.args[0])?.into_int_value();
                            let e = lower_expr(cg, &call.args[1])?.into_int_value();
                            (s, e)
                        }
                        n => return Err(format!("range takes 1 or 2 args, got {n}")),
                    };
                    let range_ty = cg
                        .ctx
                        .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                    let slot = cg
                        .builder
                        .build_alloca(range_ty, "range_val")
                        .map_err(|e| format!("{e}"))?;
                    let s_ptr = cg
                        .builder
                        .build_struct_gep(range_ty, slot, 0, "rng_s")
                        .map_err(|e| format!("{e}"))?;
                    let e_ptr = cg
                        .builder
                        .build_struct_gep(range_ty, slot, 1, "rng_e")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(s_ptr, start_v)
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(e_ptr, end_v)
                        .map_err(|e| format!("{e}"))?;
                    Ok(slot.into())
                }
                name => {
                    // Prefer the direct name. If not found, find the correct monomorphized
                    // variant by matching argument types against MonomorphizationTable entries.
                    let effective_name = if cg.module.get_function(name).is_some() {
                        name.to_string()
                    } else {
                        // Infer concrete arg types from the call site to pick the right mono.
                        let arg_types: Vec<Type> =
                            call.args.iter().map(|a| cg.expr_type(a)).collect();
                        find_mono_name_by_args(cg.mono_table, name, &arg_types)
                            .unwrap_or_else(|| name.to_string())
                    };
                    let fn_val = cg.module.get_function(&effective_name).ok_or_else(|| {
                        format!("codegen: function `{effective_name}` not found in module")
                    })?;
                    let mut call_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
                    for arg in &call.args {
                        let val = lower_expr(cg, arg)?;
                        call_args.push(val.into());
                    }
                    let call_site = cg
                        .builder
                        .build_call(fn_val, &call_args, "call")
                        .map_err(|e| format!("call {effective_name}: {e}"))?;

                    // M7 P4a: if callee is errors-capable, handle the {i64, i64} result.
                    let callee_is_ec = is_errors_capable_fn(cg.typed, &effective_name);
                    if callee_is_ec {
                        let result_struct = call_site
                            .try_as_basic_value()
                            .basic()
                            .ok_or_else(|| {
                                format!("errors-capable call `{effective_name}` returned void")
                            })?
                            .into_struct_value();
                        return lower_errors_capable_call_result(
                            cg,
                            result_struct,
                            &effective_name,
                        );
                    }

                    match call_site.try_as_basic_value().basic() {
                        Some(val) => Ok(val),
                        None => Ok(cg.i32().const_int(0, false).into()),
                    }
                }
            }
        }

        Expr::MethodCall {
            receiver,
            method,
            args,
            ..
        } => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;
            match &recv_ty {
                Type::Shape { name } => {
                    let name = name.clone();
                    lower_ufcs_call(cg, recv_val, &name, method, args)
                }
                Type::Dynamic { .. } => {
                    // Dynamic dispatch via vtable — deferred post-P5.
                    Err("codegen: dynamic dispatch call sites not yet lowered in M4 P4".to_string())
                }
                Type::BuiltinArray { elem } => {
                    let elem = elem.as_ref().clone();
                    lower_array_method(cg, recv_val.into_pointer_value(), &elem, method, args)
                }
                Type::BuiltinFixed { elem, size } => {
                    let elem = elem.as_ref().clone();
                    let sz = *size;
                    lower_fixed_method(cg, recv_val.into_pointer_value(), &elem, sz, method, args)
                }
                Type::Maybe { inner } => {
                    let inner = inner.as_ref().clone();
                    lower_maybe_method(cg, recv_val.into_pointer_value(), &inner, method, args)
                }
                Type::BuiltinMap { key, val } => {
                    let key_ty = key.as_ref().clone();
                    let val_ty = val.as_ref().clone();
                    lower_map_method(
                        cg,
                        recv_val.into_pointer_value(),
                        &key_ty,
                        &val_ty,
                        method,
                        args,
                    )
                }
                // M6: options .toString()
                Type::Options { name: opts_name } => {
                    if method == "toString" {
                        lower_options_to_string(cg, recv_val, opts_name.as_str())
                    } else {
                        Err(format!(
                            "codegen: unknown method `{method}` on options type `{opts_name}`"
                        ))
                    }
                }
                // M7 P4a: ErrorsCapable — .failed() and .or(default).
                // The receiver is a pointer to a heap-allocated {i64 error_ptr, i64 success_val}
                // stored as a pointer in a local alloca.
                Type::ErrorsCapable { inner } => {
                    let inner = inner.as_ref().clone();
                    lower_errors_capable_method(cg, recv_val, &inner, method, args)
                }
                // M7 P4b: string methods — dispatched through the string runtime.
                Type::String => {
                    lower_string_method(cg, recv_val.into_pointer_value(), method, args)
                }
                // M8 P4: sensitive methods.
                // `.reveal()` → return the underlying string pointer (identity).
                // All other methods delegate to the string runtime (same ABI).
                Type::Sensitive { .. } => {
                    let ptr = recv_val.into_pointer_value();
                    if method == "reveal" {
                        // reveal() strips the sensitive wrapper — just return the pointer.
                        Ok(ptr.into())
                    } else {
                        // All string methods work on the underlying pointer.
                        lower_string_method(cg, ptr, method, args)
                    }
                }
                _ => {
                    // M4 P5: one-arg primitive intrinsics (wrapping/saturating arithmetic).
                    if args.len() == 1 && is_1arg_intrinsic(&recv_ty, method) {
                        let arg_val = lower_expr(cg, &args[0])?;
                        lower_method_call_1arg(cg, recv_val, &recv_ty, method, arg_val)
                    } else {
                        lower_method_call(cg, recv_val, &recv_ty, method)
                    }
                }
            }
        }

        Expr::FieldAccess {
            receiver, field, ..
        } => {
            // M4 P5: type-attached constants (int.max, number.epsilon, etc.)
            if let Expr::Ident(type_name_str, _) = receiver.as_ref() {
                if type_attached_const_type(type_name_str, field).is_some() {
                    return emit_type_const(cg, type_name_str, field);
                }
                // M6: options value access: `Status.active` → i8 tag constant.
                if let Some(entry) = cg.options_table.options.get(type_name_str.as_str()) {
                    if let Some(tag) = entry.variants.iter().position(|v| v == field) {
                        return Ok(cg.ctx.i8_type().const_int(tag as u64, false).into());
                    }
                }
            }
            lower_field_access(cg, receiver, field)
        }

        Expr::StructLit { fields, .. } => {
            // When typeck resolved this to a BuiltinMap, the user wrote `{ key: value }` syntax
            // with a map annotation — lower identifier names as string literal keys.
            if let Type::BuiltinMap { val, .. } = cg.expr_type(expr) {
                let map_ptr = cg
                    .builder
                    .build_call(cg.rt.ynz_map_new, &[], "map_new")
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_map_new returned void")?
                    .into_pointer_value();
                for f in fields {
                    let key_global = build_string_global(cg.ctx, cg.module, &f.name, "imap_key");
                    let key_ptr = key_global.as_pointer_value();
                    let val_val = lower_expr(cg, &f.value)?;
                    let val_bits = cg.to_i64_bits(val_val, &val)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set_str,
                            &[map_ptr.into(), key_ptr.into(), val_bits.into()],
                            "imap_set",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
                Ok(map_ptr.into())
            } else {
                lower_struct_lit(cg, expr, fields)
            }
        }

        Expr::PostfixOp { receiver, op, .. } => lower_postfix_op(cg, receiver, op),

        Expr::SelfValue { .. } => {
            let slot = *cg
                .locals
                .get("self")
                .ok_or("codegen: `self` used outside method scope")?;
            let ty = cg.expr_type(expr);
            load(cg, slot, &ty, "self")
        }

        Expr::NoneLit { .. } => {
            let slot = cg.build_maybe_none()?;
            Ok(slot.into())
        }

        Expr::ArrayLit { elements, .. } => {
            let arr_ty = cg.expr_type(expr);
            match &arr_ty {
                Type::BuiltinFixed { elem, size } => {
                    let n = size.unwrap_or(elements.len()) as u32;
                    let arr_t = cg.i64().array_type(n.max(1));
                    let slot = cg
                        .builder
                        .build_alloca(arr_t, "fixed_arr")
                        .map_err(|e| format!("{e}"))?;
                    for (i, elem_expr) in elements.iter().enumerate() {
                        let elem_val = lower_expr(cg, elem_expr)?;
                        let elem_ty2 = cg.expr_type(elem_expr);
                        let bits = cg.to_i64_bits(elem_val, &elem_ty2)?;
                        let idx = cg.i64().const_int(i as u64, false);
                        let gep = unsafe {
                            cg.builder
                                .build_gep(cg.i64(), slot, &[idx], "fixed_elem")
                                .map_err(|e| format!("fixed gep: {e}"))?
                        };
                        cg.builder
                            .build_store(gep, bits)
                            .map_err(|e| format!("{e}"))?;
                    }
                    let _ = elem;
                    Ok(slot.into())
                }
                _ => {
                    // BuiltinArray: heap-allocate via runtime.
                    let arr_ptr = cg
                        .builder
                        .build_call(cg.rt.ynz_array_new, &[], "arr_new")
                        .map_err(|e| format!("array_new: {e}"))?
                        .try_as_basic_value()
                        .basic()
                        .ok_or("ynz_array_new returned void")?
                        .into_pointer_value();
                    for elem_expr in elements {
                        let elem_ty2 = cg.expr_type(elem_expr);
                        let elem_val = lower_expr(cg, elem_expr)?;
                        let bits = cg.to_i64_bits(elem_val, &elem_ty2)?;
                        cg.builder
                            .build_call(
                                cg.rt.ynz_array_push,
                                &[arr_ptr.into(), bits.into()],
                                "arr_push",
                            )
                            .map_err(|e| format!("array_push: {e}"))?;
                    }
                    Ok(arr_ptr.into())
                }
            }
        }

        Expr::IndexAccess {
            receiver, index, ..
        } => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;

            // Map index access is handled separately — key may be a string (pointer).
            if let Type::BuiltinMap { key, .. } = &recv_ty {
                let key_ty = key.as_ref().clone();
                let idx_val = lower_expr(cg, index)?;
                let pair_ty = cg
                    .ctx
                    .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                let out = cg
                    .builder
                    .build_alloca(pair_ty, "mi_out")
                    .map_err(|e| format!("{e}"))?;
                if key_is_string(&key_ty) {
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_get_str,
                            &[recv_val.into(), idx_val.into(), out.into()],
                            "mi_gs",
                        )
                        .map_err(|e| format!("{e}"))?;
                } else {
                    let kt = cg.expr_type(index);
                    let key_bits = cg.to_i64_bits(idx_val, &kt)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_get,
                            &[recv_val.into(), key_bits.into(), out.into()],
                            "mi_g",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
                let maybe_slot = cg
                    .builder
                    .build_alloca(cg.maybe_type(), "mi_m")
                    .map_err(|e| format!("{e}"))?;
                let h0 = cg
                    .builder
                    .build_struct_gep(pair_ty, out, 0, "h0")
                    .map_err(|e| format!("{e}"))?;
                let v0 = cg
                    .builder
                    .build_struct_gep(pair_ty, out, 1, "v0")
                    .map_err(|e| format!("{e}"))?;
                let h1 = cg
                    .builder
                    .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "h1")
                    .map_err(|e| format!("{e}"))?;
                let v1 = cg
                    .builder
                    .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "v1")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_store(
                        h1,
                        cg.builder
                            .build_load(cg.i64(), h0, "hv")
                            .map_err(|e| format!("{e}"))?,
                    )
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_store(
                        v1,
                        cg.builder
                            .build_load(cg.i64(), v0, "vv")
                            .map_err(|e| format!("{e}"))?,
                    )
                    .map_err(|e| format!("{e}"))?;
                return Ok(maybe_slot.into());
            }

            let idx_val = lower_expr(cg, index)?;
            let idx = idx_val.into_int_value();

            match &recv_ty {
                Type::BuiltinArray { .. } => {
                    let out_slot = cg
                        .builder
                        .build_alloca(cg.maybe_type(), "arr_get_out")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_array_get,
                            &[recv_val.into(), idx.into(), out_slot.into()],
                            "arr_get",
                        )
                        .map_err(|e| format!("array_get: {e}"))?;
                    Ok(out_slot.into())
                }
                Type::BuiltinFixed { size, .. } => {
                    let n = size.unwrap_or(0) as u64;
                    let maybe_slot = cg
                        .builder
                        .build_alloca(cg.maybe_type(), "fixed_get")
                        .map_err(|e| format!("{e}"))?;

                    let in_bounds = if n > 0 {
                        let idx_ext = cg
                            .builder
                            .build_int_z_extend_or_bit_cast(idx, cg.i64(), "ie")
                            .map_err(|e| format!("{e}"))?;
                        cg.builder
                            .build_int_compare(
                                IntPredicate::ULT,
                                idx_ext,
                                cg.i64().const_int(n, false),
                                "in_bounds",
                            )
                            .map_err(|e| format!("{e}"))?
                    } else {
                        cg.bool().const_int(0, false)
                    };

                    let ok_bb = cg.append_block("fixed_ok");
                    let oob_bb = cg.append_block("fixed_oob");
                    let merge_bb = cg.append_block("fixed_merge");
                    cg.builder
                        .build_conditional_branch(in_bounds, ok_bb, oob_bb)
                        .map_err(|e| format!("{e}"))?;

                    // ok: load element and write maybe_some
                    cg.builder.position_at_end(ok_bb);
                    let arr_ptr = recv_val.into_pointer_value();
                    let gep = unsafe {
                        cg.builder
                            .build_gep(cg.i64(), arr_ptr, &[idx], "fixed_elem_ok")
                            .map_err(|e| format!("fixed gep ok: {e}"))?
                    };
                    let elem_bits = cg
                        .builder
                        .build_load(cg.i64(), gep, "elem_bits")
                        .map_err(|e| format!("{e}"))?
                        .into_int_value();
                    let has_ok = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_ok")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(has_ok, cg.i64().const_int(1, false))
                        .map_err(|e| format!("{e}"))?;
                    let val_ok = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_ok")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(val_ok, elem_bits)
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_unconditional_branch(merge_bb)
                        .map_err(|e| format!("{e}"))?;

                    // oob: write maybe_none
                    cg.builder.position_at_end(oob_bb);
                    let has_oob = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_oob")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(has_oob, cg.i64().const_zero())
                        .map_err(|e| format!("{e}"))?;
                    let val_oob = cg
                        .builder
                        .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_oob")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(val_oob, cg.i64().const_zero())
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_unconditional_branch(merge_bb)
                        .map_err(|e| format!("{e}"))?;

                    cg.builder.position_at_end(merge_bb);
                    Ok(maybe_slot.into())
                }
                _ => Err(format!("IndexAccess on unsupported type: {:?}", recv_ty)),
            }
        }

        Expr::MapLit { entries, .. } => {
            let map_ty = cg.expr_type(expr);
            let key_ty = match &map_ty {
                Type::BuiltinMap { key, .. } => key.as_ref().clone(),
                _ => return Err("MapLit with non-BuiltinMap type".to_string()),
            };

            let map_ptr = cg
                .builder
                .build_call(cg.rt.ynz_map_new, &[], "map_new")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_map_new returned void")?
                .into_pointer_value();

            for (key_expr, val_expr) in entries {
                let key_val = lower_expr(cg, key_expr)?;
                let val_val = lower_expr(cg, val_expr)?;
                let vt = cg.expr_type(val_expr);
                let val_bits = cg.to_i64_bits(val_val, &vt)?;
                if key_is_string(&key_ty) {
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set_str,
                            &[map_ptr.into(), key_val.into(), val_bits.into()],
                            "map_set_str",
                        )
                        .map_err(|e| format!("{e}"))?;
                } else {
                    let kt = cg.expr_type(key_expr);
                    let key_bits = cg.to_i64_bits(key_val, &kt)?;
                    cg.builder
                        .build_call(
                            cg.rt.ynz_map_set,
                            &[map_ptr.into(), key_bits.into(), val_bits.into()],
                            "map_set",
                        )
                        .map_err(|e| format!("{e}"))?;
                }
            }
            Ok(map_ptr.into())
        }

        Expr::Error(_) => Err("codegen: error node".to_string()),

        // M6: `x is Foo` — P4 implements; typeck would have rejected programs with unimplemented forms.
        Expr::Is { .. } => Err("codegen: Expr::Is not yet lowered (P4 work)".to_string()),

        // M7 P4b: interpolated string codegen.
        //
        // Pure-literal backtick strings (no `${}` parts): emit as a null-terminated
        // global byte array (same as the prior M7 P1 path).
        //
        // Strings with `${}` expressions: use the string builder API. Each part is
        // appended in sequence; the builder is finalized to a single heap string.
        // One allocation per interpolated string expression, regardless of part count.
        Expr::InterpolatedString(parts, _) => {
            let is_pure_lit = parts
                .iter()
                .all(|p| matches!(p, ynz_ast::nodes::StringPart::Lit(_, _)));
            if is_pure_lit {
                let mut bytes: Vec<u8> = parts
                    .iter()
                    .flat_map(|p| match p {
                        ynz_ast::nodes::StringPart::Lit(b, _) => b.clone(),
                        ynz_ast::nodes::StringPart::Expr(_, _) => unreachable!(),
                    })
                    .collect();
                push_c_string_terminator(&mut bytes);
                let i8t = cg.i8();
                let arr_ty = i8t.array_type(bytes.len() as u32);
                let arr = i8t.const_array(
                    &bytes
                        .iter()
                        .map(|&b| i8t.const_int(b as u64, false))
                        .collect::<Vec<_>>(),
                );
                let g = cg
                    .module
                    .add_global(arr_ty, Some(AddressSpace::default()), "bts");
                g.set_initializer(&arr);
                g.set_constant(true);
                g.set_linkage(inkwell::module::Linkage::Private);
                g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
                Ok(g.as_pointer_value().into())
            } else {
                // Interpolated string: build via ynz_string_builder_*.
                let builder = cg
                    .builder
                    .build_call(cg.rt.ynz_string_builder_new, &[], "istr_bld")
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_builder_new void")?
                    .into_pointer_value();

                for part in parts.iter() {
                    match part {
                        ynz_ast::nodes::StringPart::Lit(bytes, _) => {
                            // Emit the literal bytes as a global and append.
                            let mut b = bytes.clone();
                            push_c_string_terminator(&mut b);
                            let i8t = cg.i8();
                            let arr_ty = i8t.array_type(b.len() as u32);
                            let arr = i8t.const_array(
                                &b.iter()
                                    .map(|&x| i8t.const_int(x as u64, false))
                                    .collect::<Vec<_>>(),
                            );
                            let g = cg.module.add_global(
                                arr_ty,
                                Some(AddressSpace::default()),
                                "istr_lit",
                            );
                            g.set_initializer(&arr);
                            g.set_constant(true);
                            g.set_linkage(inkwell::module::Linkage::Private);
                            g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
                            cg.builder
                                .build_call(
                                    cg.rt.ynz_string_builder_append,
                                    &[builder.into(), g.as_pointer_value().into()],
                                    "",
                                )
                                .map_err(|e| format!("{e}"))?;
                        }
                        ynz_ast::nodes::StringPart::Expr(sub_expr, _) => {
                            // Evaluate the expression and convert to string via toString().
                            let val = lower_expr(cg, sub_expr)?;
                            let sub_ty = cg.expr_type(sub_expr);
                            // Produce a null-terminated string from the expression value.
                            let s_ptr = expr_to_cstring(cg, val, &sub_ty)?;
                            cg.builder
                                .build_call(
                                    cg.rt.ynz_string_builder_append,
                                    &[builder.into(), s_ptr.into()],
                                    "",
                                )
                                .map_err(|e| format!("{e}"))?;
                        }
                    }
                }

                let result = cg
                    .builder
                    .build_call(
                        cg.rt.ynz_string_builder_finalize,
                        &[builder.into()],
                        "istr_fin",
                    )
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_builder_finalize void")?;
                Ok(result)
            }
        }
        // M8 P5: wait/background — sequential semantics in M8 (both are identity at codegen).
        // `wait foo()` → lower foo() directly.
        // `background foo()` → lower foo(), discard return value, return i32(0).
        Expr::Wait(inner, _) => lower_expr(cg, inner),
        Expr::Background(inner, _) => {
            let _ = lower_expr(cg, inner)?; // run to completion, discard result
            Ok(cg.i32().const_int(0, false).into())
        }
    }
}

/// Coerce a decimal128 (N≤34) operand to a bignum C-string when the other side is
/// bignum (N>34). Returns (lhs, rhs) with both as bignum string pointers when needed.
fn coerce_to_bignum_pair<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    lhs_ty: &Type,
    rhs_ty: &Type,
) -> Result<(BasicValueEnum<'ctx>, BasicValueEnum<'ctx>), String> {
    let lhs_big = matches!(lhs_ty, Type::Number { precision } if *precision > 34);
    let rhs_big = matches!(rhs_ty, Type::Number { precision } if *precision > 34);
    if !lhs_big && !rhs_big {
        return Ok((lhs, rhs));
    }
    let lhs2 = if matches!(lhs_ty, Type::Number { precision } if *precision <= 34) {
        let s = cg
            .builder
            .build_call(cg.rt.decimal_to_string, &[lhs.into()], "lhs_dec2str")
            .map_err(|e| format!("{e}"))?;
        s.try_as_basic_value()
            .basic()
            .ok_or_else(|| "decimal_to_string returned void".to_string())?
    } else {
        lhs
    };
    let rhs2 = if matches!(rhs_ty, Type::Number { precision } if *precision <= 34) {
        let s = cg
            .builder
            .build_call(cg.rt.decimal_to_string, &[rhs.into()], "rhs_dec2str")
            .map_err(|e| format!("{e}"))?;
        s.try_as_basic_value()
            .basic()
            .ok_or_else(|| "decimal_to_string returned void".to_string())?
    } else {
        rhs
    };
    Ok((lhs2, rhs2))
}

fn lower_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    op: &BinOpKind,
    lhs_e: &Expr,
    rhs_e: &Expr,
    lhs_ty: &Type,
    rhs_ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    use BinOpKind::*;
    if matches!(op, And | Or) {
        return lower_short_circuit(cg, matches!(op, And), lhs_e, rhs_e);
    }
    let lhs = lower_expr(cg, lhs_e)?;
    let rhs = lower_expr(cg, rhs_e)?;

    // M8 P6: when one operand is bignum (N>34) and the other is decimal128 (N≤34),
    // coerce the N≤34 side to a C string so both operands are bignum-compatible.
    let (lhs, rhs) = coerce_to_bignum_pair(cg, lhs, rhs, lhs_ty, rhs_ty)?;

    match (op, lhs_ty) {
        (Add, Type::Int) => int_arith_overflow(cg, lhs.into_int_value(), rhs.into_int_value(), op, lhs_e.span().start as u32),
        (Sub, Type::Int) => int_arith_overflow(cg, lhs.into_int_value(), rhs.into_int_value(), op, lhs_e.span().start as u32),
        (Mul, Type::Int) => int_arith_overflow(cg, lhs.into_int_value(), rhs.into_int_value(), op, lhs_e.span().start as u32),
        (Div, Type::Int) => int_divrem(cg, lhs.into_int_value(), rhs.into_int_value(), false, lhs_e.span().start as u32),
        (Rem, Type::Int) => int_divrem(cg, lhs.into_int_value(), rhs.into_int_value(), true, lhs_e.span().start as u32),

        (Add, Type::Float) => cg
            .builder
            .build_float_add(lhs.into_float_value(), rhs.into_float_value(), "fadd")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Sub, Type::Float) => cg
            .builder
            .build_float_sub(lhs.into_float_value(), rhs.into_float_value(), "fsub")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Mul, Type::Float) => cg
            .builder
            .build_float_mul(lhs.into_float_value(), rhs.into_float_value(), "fmul")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Div, Type::Float) => cg
            .builder
            .build_float_div(lhs.into_float_value(), rhs.into_float_value(), "fdiv")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Rem, Type::Float) => cg
            .builder
            .build_float_rem(lhs.into_float_value(), rhs.into_float_value(), "frem")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),

        (Add, Type::Number { precision }) if *precision <= 34 => decimal_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            cg.rt.decimal_add,
            "dadd",
        ),
        (Add, Type::Number { precision }) => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_add,
            "bnadd",
        ),
        (Sub, Type::Number { precision }) if *precision <= 34 => decimal_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            cg.rt.decimal_sub,
            "dsub",
        ),
        (Sub, Type::Number { precision }) => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_sub,
            "bnsub",
        ),
        (Mul, Type::Number { precision }) if *precision <= 34 => decimal_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            cg.rt.decimal_mul,
            "dmul",
        ),
        (Mul, Type::Number { precision }) => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_mul,
            "bnmul",
        ),
        (Div, Type::Number { precision }) if *precision > 34 => bignum_binop(
            cg,
            lhs.into_pointer_value(),
            rhs.into_pointer_value(),
            *precision,
            cg.rt.ynz_bignum_div,
            "bndiv",
        ),
        (Div, Type::Number { .. }) => {
            decimal_div(cg, lhs.into_pointer_value(), rhs.into_pointer_value())
        }
        (Rem, Type::Number { .. }) => {
            // typeck already rejected this; emit unreachable panic
            let file_ptr = cg.globals.source_file.as_pointer_value();
            let zero32 = cg.i32().const_int(0, false);
            cg.builder
                .build_call(
                    cg.rt.panic_div_by_zero,
                    &[cg.globals.panic_dec_rem.as_pointer_value().into(), file_ptr.into(), zero32.into(), zero32.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
            let out = cg
                .builder
                .build_alloca(cg.i128(), "rem_dead")
                .map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }

        (Lt, Type::Int) => icmp(cg, IntPredicate::SLT, lhs, rhs, "ilt"),
        (LtEq, Type::Int) => icmp(cg, IntPredicate::SLE, lhs, rhs, "ile"),
        (Gt, Type::Int) => icmp(cg, IntPredicate::SGT, lhs, rhs, "igt"),
        (GtEq, Type::Int) => icmp(cg, IntPredicate::SGE, lhs, rhs, "ige"),
        (EqEq, Type::Int) => icmp(cg, IntPredicate::EQ, lhs, rhs, "ieq"),
        (NotEq, Type::Int) => icmp(cg, IntPredicate::NE, lhs, rhs, "ine"),

        (Lt, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OLT, lhs, rhs, "flt"),
        (LtEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OLE, lhs, rhs, "fle"),
        (Gt, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OGT, lhs, rhs, "fgt"),
        (GtEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OGE, lhs, rhs, "fge"),
        (EqEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OEQ, lhs, rhs, "feq"),
        (NotEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::ONE, lhs, rhs, "fne"),

        (EqEq | NotEq | Lt | LtEq | Gt | GtEq, Type::Number { .. }) => {
            decimal_compare(cg, lhs.into_pointer_value(), rhs.into_pointer_value(), op)
        }

        (EqEq, Type::Bool) => icmp(cg, IntPredicate::EQ, lhs, rhs, "beq"),
        (NotEq, Type::Bool) => icmp(cg, IntPredicate::NE, lhs, rhs, "bne"),

        // M6: options equality — `icmp eq i8`
        (EqEq, Type::Options { .. }) => icmp(cg, IntPredicate::EQ, lhs, rhs, "oeq"),
        (NotEq, Type::Options { .. }) => icmp(cg, IntPredicate::NE, lhs, rhs, "one"),

        // M7 P4b: string equality/inequality via ynz_string_eq (NFC normalized).
        (EqEq, Type::String) => {
            let call = cg
                .builder
                .build_call(cg.rt.string_eq, &[lhs.into(), rhs.into()], "s_eq")
                .map_err(|e| format!("{e}"))?;
            let r = call
                .try_as_basic_value()
                .basic()
                .ok_or("string_eq void")?
                .into_int_value();
            // ynz_string_eq returns i32; convert to i1 for bool.
            cg.builder
                .build_int_compare(IntPredicate::NE, r, cg.i32().const_int(0, false), "s_eq_b")
                .map(|v| v.into())
                .map_err(|e| format!("{e}"))
        }
        (NotEq, Type::String) => {
            let call = cg
                .builder
                .build_call(cg.rt.string_eq, &[lhs.into(), rhs.into()], "s_ne")
                .map_err(|e| format!("{e}"))?;
            let r = call
                .try_as_basic_value()
                .basic()
                .ok_or("string_eq void")?
                .into_int_value();
            // Invert: EQ → 0 means not-equal.
            cg.builder
                .build_int_compare(IntPredicate::EQ, r, cg.i32().const_int(0, false), "s_ne_b")
                .map(|v| v.into())
                .map_err(|e| format!("{e}"))
        }

        (BitAnd, Type::Int) => cg
            .builder
            .build_and(lhs.into_int_value(), rhs.into_int_value(), "band")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (BitOr, Type::Int) => cg
            .builder
            .build_or(lhs.into_int_value(), rhs.into_int_value(), "bor")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (BitXor, Type::Int) => cg
            .builder
            .build_xor(lhs.into_int_value(), rhs.into_int_value(), "bxor")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Shl, Type::Int) => cg
            .builder
            .build_left_shift(lhs.into_int_value(), rhs.into_int_value(), "shl")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Shr, Type::Int) => cg
            .builder
            .build_right_shift(lhs.into_int_value(), rhs.into_int_value(), true, "shr")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),

        _ => Err(format!("codegen: unsupported binop {:?} {:?}", op, lhs_ty)),
    }
}

fn icmp<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    pred: IntPredicate,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    cg.builder
        .build_int_compare(pred, lhs.into_int_value(), rhs.into_int_value(), name)
        .map(|v| v.into())
        .map_err(|e| format!("{e}"))
}

fn fcmp<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    pred: inkwell::FloatPredicate,
    lhs: BasicValueEnum<'ctx>,
    rhs: BasicValueEnum<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    cg.builder
        .build_float_compare(pred, lhs.into_float_value(), rhs.into_float_value(), name)
        .map(|v| v.into())
        .map_err(|e| format!("{e}"))
}

fn int_arith_overflow<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
    op: &BinOpKind,
    span_start: u32,
) -> Result<BasicValueEnum<'ctx>, String> {
    let (intrinsic, msg_g) = match op {
        BinOpKind::Add => (cg.rt.sadd_overflow, cg.globals.panic_int_add),
        BinOpKind::Sub => (cg.rt.ssub_overflow, cg.globals.panic_int_sub),
        BinOpKind::Mul => (cg.rt.smul_overflow, cg.globals.panic_int_mul),
        _ => unreachable!(),
    };
    let call = cg
        .builder
        .build_call(intrinsic, &[lhs.into(), rhs.into()], "ov_res")
        .map_err(|e| format!("{e}"))?;
    let s = call
        .try_as_basic_value()
        .basic()
        .ok_or("overflow intrinsic void")?
        .into_struct_value();
    let sum = cg
        .builder
        .build_extract_value(s, 0, "sum")
        .map_err(|e| format!("{e}"))?
        .into_int_value();
    let ov = cg
        .builder
        .build_extract_value(s, 1, "ov")
        .map_err(|e| format!("{e}"))?
        .into_int_value();

    let ok_bb = cg.append_block("ov_ok");
    let panic_bb = cg.append_block("ov_panic");
    cg.builder
        .build_conditional_branch(ov, panic_bb, ok_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(panic_bb);
    let file_ptr = cg.globals.source_file.as_pointer_value();
    let offset_val = cg.i32().const_int(span_start as u64, false);
    let zero_col = cg.i32().const_int(0, false);
    cg.builder
        .build_call(
            cg.rt.panic_overflow,
            &[msg_g.as_pointer_value().into(), file_ptr.into(), offset_val.into(), zero_col.into()],
            "",
        )
        .map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ok_bb);
    Ok(sum.into())
}

fn int_divrem<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
    is_rem: bool,
    span_start: u32,
) -> Result<BasicValueEnum<'ctx>, String> {
    let zero = cg.i64().const_int(0, false);
    let is_z = cg
        .builder
        .build_int_compare(IntPredicate::EQ, rhs, zero, "div_zero")
        .map_err(|e| format!("{e}"))?;
    let ok_bb = cg.append_block("div_ok");
    let pbb = cg.append_block("div_panic");
    cg.builder
        .build_conditional_branch(is_z, pbb, ok_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(pbb);
    let msg = if is_rem {
        cg.globals.panic_int_rem
    } else {
        cg.globals.panic_int_div
    };
    let file_ptr = cg.globals.source_file.as_pointer_value();
    let offset_val = cg.i32().const_int(span_start as u64, false);
    let zero_col = cg.i32().const_int(0, false);
    cg.builder
        .build_call(
            cg.rt.panic_div_by_zero,
            &[msg.as_pointer_value().into(), file_ptr.into(), offset_val.into(), zero_col.into()],
            "",
        )
        .map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ok_bb);
    if is_rem {
        cg.builder
            .build_int_signed_rem(lhs, rhs, "srem")
            .map(|v| v.into())
            .map_err(|e| format!("{e}"))
    } else {
        cg.builder
            .build_int_signed_div(lhs, rhs, "sdiv")
            .map(|v| v.into())
            .map_err(|e| format!("{e}"))
    }
}

fn decimal_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    rt_fn: FunctionValue<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let out = cg
        .builder
        .build_alloca(cg.i128(), name)
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_call(rt_fn, &[lhs.into(), rhs.into(), out.into()], "")
        .map_err(|e| format!("{e}"))?;
    Ok(out.into())
}

/// M8 P6: bignum arithmetic call — (a: *i8, b: *i8, precision: i32) → *i8.
fn bignum_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    precision: u32,
    rt_fn: FunctionValue<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let prec_val = cg.ctx.i32_type().const_int(precision as u64, false);
    let result = cg
        .builder
        .build_call(rt_fn, &[lhs.into(), rhs.into(), prec_val.into()], name)
        .map_err(|e| format!("{e}"))?;
    Ok(result
        .try_as_basic_value()
        .basic()
        .ok_or("bignum op returned void")?)
}

fn decimal_div<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let zero = cg.globals.dec_zero.as_pointer_value();
    let cmp_call = cg
        .builder
        .build_call(
            cg.rt.decimal_compare,
            &[rhs.into(), zero.into()],
            "ddiv_cmp",
        )
        .map_err(|e| format!("{e}"))?;
    let cmp_i32 = cmp_call
        .try_as_basic_value()
        .basic()
        .ok_or("decimal_compare void")?
        .into_int_value();
    let is_z = cg
        .builder
        .build_int_compare(
            IntPredicate::EQ,
            cmp_i32,
            cg.i32().const_int(0, false),
            "ddiv_zero",
        )
        .map_err(|e| format!("{e}"))?;
    let ok_bb = cg.append_block("ddiv_ok");
    let pbb = cg.append_block("ddiv_panic");
    cg.builder
        .build_conditional_branch(is_z, pbb, ok_bb)
        .map_err(|e| format!("{e}"))?;
    cg.builder.position_at_end(pbb);
    let file_ptr = cg.globals.source_file.as_pointer_value();
    let zero32 = cg.i32().const_int(0, false);
    cg.builder
        .build_call(
            cg.rt.panic_div_by_zero,
            &[cg.globals.panic_dec_div.as_pointer_value().into(), file_ptr.into(), zero32.into(), zero32.into()],
            "",
        )
        .map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
    cg.builder.position_at_end(ok_bb);
    decimal_binop(cg, lhs, rhs, cg.rt.decimal_div, "ddiv")
}

fn decimal_compare<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    op: &BinOpKind,
) -> Result<BasicValueEnum<'ctx>, String> {
    let c = cg
        .builder
        .build_call(cg.rt.decimal_compare, &[lhs.into(), rhs.into()], "dcmp")
        .map_err(|e| format!("{e}"))?;
    let ci = c
        .try_as_basic_value()
        .basic()
        .ok_or("cmp void")?
        .into_int_value();
    let z = cg.i32().const_int(0, false);
    let pred = match op {
        BinOpKind::Lt => IntPredicate::SLT,
        BinOpKind::LtEq => IntPredicate::SLE,
        BinOpKind::Gt => IntPredicate::SGT,
        BinOpKind::GtEq => IntPredicate::SGE,
        BinOpKind::EqEq => IntPredicate::EQ,
        BinOpKind::NotEq => IntPredicate::NE,
        _ => unreachable!(),
    };
    cg.builder
        .build_int_compare(pred, ci, z, "dcmp_b")
        .map(|v| v.into())
        .map_err(|e| format!("{e}"))
}

fn lower_short_circuit<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    is_and: bool,
    lhs_e: &Expr,
    rhs_e: &Expr,
) -> Result<BasicValueEnum<'ctx>, String> {
    let bool_ty = cg.bool();
    let lhs = lower_expr(cg, lhs_e)?.into_int_value();
    let lhs_bb = cg.builder.get_insert_block().ok_or("no lhs block")?;

    let rhs_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_rhs" } else { "or_rhs" });
    let short_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_short" } else { "or_short" });
    let merge_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_merge" } else { "or_merge" });

    let _ = lhs_bb;
    if is_and {
        cg.builder.build_conditional_branch(lhs, rhs_bb, short_bb)
    } else {
        cg.builder.build_conditional_branch(lhs, short_bb, rhs_bb)
    }
    .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(rhs_bb);
    let rhs = lower_expr(cg, rhs_e)?.into_int_value();
    let rhs_bb_end = cg.builder.get_insert_block().ok_or("no rhs end block")?;
    cg.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(short_bb);
    cg.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(merge_bb);
    let phi = cg
        .builder
        .build_phi(bool_ty, if is_and { "and_r" } else { "or_r" })
        .map_err(|e| format!("{e}"))?;
    let short_val = bool_ty.const_int(if is_and { 0 } else { 1 }, false);
    phi.add_incoming(&[(&rhs, rhs_bb_end), (&short_val, short_bb)]);
    Ok(phi.as_basic_value())
}

fn lower_unary<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    op: &UnaryOpKind,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    match (op, ty) {
        (UnaryOpKind::Neg, Type::Int) => cg
            .builder
            .build_int_neg(val.into_int_value(), "neg")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (UnaryOpKind::Neg, Type::Float) => cg
            .builder
            .build_float_neg(val.into_float_value(), "fneg")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (UnaryOpKind::Neg, Type::Number { .. }) => {
            let zero = cg.globals.dec_zero.as_pointer_value();
            decimal_binop(
                cg,
                zero,
                val.into_pointer_value(),
                cg.rt.decimal_sub,
                "dec_neg",
            )
        }
        (UnaryOpKind::Not, Type::Bool) => cg
            .builder
            .build_xor(val.into_int_value(), cg.bool().const_int(1, false), "not")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (UnaryOpKind::BitNot, Type::Int) => cg
            .builder
            .build_not(val.into_int_value(), "bitnot")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        _ => Err(format!("codegen: unsupported unary {:?} {:?}", op, ty)),
    }
}

fn lower_print<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<(), String> {
    let p = to_c_string(cg, val, ty)?;
    cg.builder
        .build_call(cg.rt.puts, &[p.into()], "puts")
        .map_err(|e| format!("{e}"))?;
    Ok(())
}

/// Terminate a byte vector as a C string, aborting with an ICE message if any
/// embedded NUL is found.
///
/// The lexer rejects `\0` in string literals (Batch 5a.6), so this path should
/// be unreachable in v0.1. If reached, an earlier compiler phase silently
/// introduced a NUL — that is a compiler bug, not a user error.
fn push_c_string_terminator(bytes: &mut Vec<u8>) {
    if bytes.iter().any(|&b| b == 0) {
        eprintln!(
            "INTERNAL COMPILER ERROR: string literal contains an embedded NUL byte at codegen \
             time. The lexer should have rejected this. Please file an issue at \
             https://github.com/patrickrizzardi/ynz/issues with the source file."
        );
        std::process::abort();
    }
    bytes.push(0);
}

fn to_c_string<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<PointerValue<'ctx>, String> {
    match ty {
        Type::String => Ok(val.into_pointer_value()),

        // M8 P4: sensitive string — delegate to the runtime, which checks
        // YNZ_REVEAL_SENSITIVE at first call and returns either the raw pointer
        // or a static "[REDACTED]" string accordingly.
        Type::Sensitive { .. } => {
            let call = cg
                .builder
                .build_call(
                    cg.rt.ynz_sensitive_to_string,
                    &[val.into()],
                    "sens_str",
                )
                .map_err(|e| format!("{e}"))?;
            let ptr = call
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_sensitive_to_string returned void")?
                .into_pointer_value();
            Ok(ptr)
        }

        Type::Bool => cg
            .builder
            .build_select(
                val.into_int_value(),
                cg.globals.str_true.as_pointer_value(),
                cg.globals.str_false.as_pointer_value(),
                "bstr",
            )
            .map(|v| v.into_pointer_value())
            .map_err(|e| format!("{e}")),

        // Runtime format shims return a ptr into a thread-local static buffer.
        Type::Int => {
            let c = cg
                .builder
                .build_call(cg.rt.int_to_string, &[val.into()], "int_str")
                .map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value()
                .basic()
                .ok_or("int_to_string returned void")?
                .into_pointer_value())
        }
        Type::Float => {
            let c = cg
                .builder
                .build_call(cg.rt.float_to_string, &[val.into()], "flt_str")
                .map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value()
                .basic()
                .ok_or("float_to_string returned void")?
                .into_pointer_value())
        }
        Type::Number { precision } if *precision <= 34 => {
            let c = cg
                .builder
                .build_call(cg.rt.decimal_to_string, &[val.into()], "dec_str")
                .map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value()
                .basic()
                .ok_or("decimal_to_string returned void")?
                .into_pointer_value())
        }
        // M8 P6: bignum (N > 34) is stored as a pointer to a decimal string — pass directly.
        Type::Number { .. } => Ok(val.into_pointer_value()),
        // Default debug representation for user-defined shapes: "ShapeName { field: val, ... }"
        // Visible fields only. Nested shapes are printed recursively.
        Type::Shape { name } => {
            let shape_ptr = val.into_pointer_value();

            // Collect visible field names + types before borrowing cg mutably.
            let (visible_fields, struct_ty) = {
                let shape_def = cg
                    .shape_table
                    .get(name)
                    .ok_or_else(|| format!("to_c_string: no shape `{name}`"))?;
                let visible: Vec<(String, usize, Type)> = shape_def
                    .fields
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| !f.is_hidden)
                    .map(|(idx, f)| (f.name.clone(), idx, f.ty.clone()))
                    .collect();
                let st = cg
                    .shape_types
                    .get(name)
                    .ok_or_else(|| format!("to_c_string: no LLVM type for `{name}`"))?;
                (visible, st)
            };

            let builder_val = cg
                .builder
                .build_call(cg.rt.ynz_string_builder_new, &[], "dbg_bld")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_new void")?
                .into_pointer_value();

            // Helper closure: append a static string literal to the builder.
            let append_static = |cg: &mut Cg<'ctx, '_>, bld: PointerValue<'ctx>, s: &str| {
                let g = build_string_global(cg.ctx, cg.module, s, "dbg_lit");
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[bld.into(), g.as_pointer_value().into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))
                    .map(|_| ())
            };

            // Anonymous inline shapes use synthesized `__anon_*` names — omit the name prefix.
            let open = if name.starts_with("__anon_") {
                "{ ".to_string()
            } else {
                format!("{name} {{ ")
            };
            append_static(cg, builder_val, &open)?;

            for (i, (field_name, field_idx, field_ty)) in visible_fields.iter().enumerate() {
                if i > 0 {
                    append_static(cg, builder_val, ", ")?;
                }
                append_static(cg, builder_val, &format!("{field_name}: "))?;

                let gep = cg
                    .builder
                    .build_struct_gep(struct_ty, shape_ptr, *field_idx as u32, field_name)
                    .map_err(|e| format!("GEP {field_name}: {e}"))?;
                // Fields use different LLVM types depending on the Yinz type:
                //   Number (decimal128) → i128 → pass as pointer
                //   Options            → i8   → zero-extend to i64, use as tag
                //   everything else    → i64  → i64_bits_to
                let field_val: BasicValueEnum = if matches!(field_ty, Type::Number { .. }) {
                    let i128t = cg.ctx.i128_type();
                    let raw = cg
                        .builder
                        .build_load(i128t, gep, "dbg_dec_raw")
                        .map_err(|e| format!("{e}"))?;
                    let slot = cg
                        .builder
                        .build_alloca(i128t, "dbg_dec_slot")
                        .map_err(|e| format!("{e}"))?;
                    cg.builder
                        .build_store(slot, raw)
                        .map_err(|e| format!("{e}"))?;
                    slot.into()
                } else if matches!(field_ty, Type::Options { .. }) {
                    // Options stored as i8 tag — load as i8, zero-extend to i64.
                    let raw = cg
                        .builder
                        .build_load(cg.ctx.i8_type(), gep, "dbg_opt_raw")
                        .map_err(|e| format!("{e}"))?
                        .into_int_value();
                    let extended = cg
                        .builder
                        .build_int_z_extend(raw, cg.i64(), "dbg_opt_ext")
                        .map_err(|e| format!("{e}"))?;
                    extended.into()
                } else {
                    let bits = cg
                        .builder
                        .build_load(cg.i64(), gep, "dbg_bits")
                        .map_err(|e| format!("{e}"))?
                        .into_int_value();
                    cg.i64_bits_to(bits, field_ty)?
                };
                let field_str = to_c_string(cg, field_val, field_ty)?;

                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[builder_val.into(), field_str.into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))?;
            }

            append_static(cg, builder_val, " }")?;

            let result = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_builder_finalize,
                    &[builder_val.into()],
                    "dbg_str",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_finalize void")?
                .into_pointer_value();

            Ok(result)
        }

        // Array debug representation: "[elem1, elem2, ..., elem20, ... and N more]"
        // Capped at 20 elements — arrays of 10k elements should not flood output.
        Type::BuiltinArray { elem } => {
            const PRINT_CAP: u64 = 20;
            let arr_ptr = val.into_pointer_value();
            let elem = elem.as_ref().clone();

            let builder_val = cg
                .builder
                .build_call(cg.rt.ynz_string_builder_new, &[], "arr_bld")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_new void")?
                .into_pointer_value();

            let append_static = |cg: &mut Cg<'ctx, '_>, bld: PointerValue<'ctx>, s: &str| {
                let g = build_string_global(cg.ctx, cg.module, s, "arr_lit");
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[bld.into(), g.as_pointer_value().into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))
                    .map(|_| ())
            };

            append_static(cg, builder_val, "[")?;

            let count = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr_ptr.into()], "arr_cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_array_count void")?
                .into_int_value();

            let cap_const = cg.i64().const_int(PRINT_CAP, false);
            let use_full = cg
                .builder
                .build_int_compare(IntPredicate::SLE, count, cap_const, "arr_use_full")
                .map_err(|e| format!("{e}"))?;
            let cap = cg
                .builder
                .build_select(use_full, count, cap_const, "arr_cap")
                .map_err(|e| format!("{e}"))?
                .into_int_value();

            // Loop 0..cap, appending each element.
            let i_slot = cg
                .builder
                .build_alloca(cg.i64(), "arr_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;

            let cond_bb = cg.append_block("arr_cond");
            let body_bb = cg.append_block("arr_body");
            let after_bb = cg.append_block("arr_after");

            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(cond_bb);

            let i = cg
                .builder
                .build_load(cg.i64(), i_slot, "arr_i_val")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let cmp = cg
                .builder
                .build_int_compare(IntPredicate::SLT, i, cap, "arr_lt")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(cmp, body_bb, after_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(body_bb);

            // Separator: ", " before all elements except the first.
            let is_first = cg
                .builder
                .build_int_compare(IntPredicate::EQ, i, cg.i64().const_zero(), "arr_first")
                .map_err(|e| format!("{e}"))?;
            let sep_bb = cg.append_block("arr_sep");
            let elem_bb = cg.append_block("arr_elem");
            cg.builder
                .build_conditional_branch(is_first, elem_bb, sep_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(sep_bb);
            append_static(cg, builder_val, ", ")?;
            cg.builder
                .build_unconditional_branch(elem_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(elem_bb);

            // Load element via ynz_array_get into a maybe_type() slot.
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "arr_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr_ptr.into(), i.into(), out.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            let val_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), out, 1, "arr_vgep")
                .map_err(|e| format!("{e}"))?;
            let bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "arr_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let elem_val = cg.i64_bits_to(bits, &elem)?;
            let elem_str = to_c_string(cg, elem_val, &elem)?;

            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), elem_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;

            // i += 1
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "arr_next_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(after_bb);

            // If count > cap, append "... and N more"
            let truncated_bb = cg.append_block("arr_trunc");
            let finish_bb = cg.append_block("arr_finish");
            let was_truncated = cg
                .builder
                .build_int_compare(IntPredicate::SGT, count, cap_const, "arr_trunc_cmp")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(was_truncated, truncated_bb, finish_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(truncated_bb);
            let remaining = cg
                .builder
                .build_int_sub(count, cap_const, "arr_rem")
                .map_err(|e| format!("{e}"))?;
            // Convert remaining count to string and build the "... and N more" suffix.
            let rem_str = cg
                .builder
                .build_call(cg.rt.int_to_string, &[remaining.into()], "arr_rem_str")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("int_to_string void")?
                .into_pointer_value();
            append_static(cg, builder_val, ", ... and ")?;
            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), rem_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            append_static(cg, builder_val, " more")?;
            cg.builder
                .build_unconditional_branch(finish_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(finish_bb);
            append_static(cg, builder_val, "]")?;

            let result = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_builder_finalize,
                    &[builder_val.into()],
                    "arr_str",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_builder_finalize void")?
                .into_pointer_value();

            Ok(result)
        }

        // Map debug representation: "{ key: val, key2: val2 }" using insertion order.
        Type::BuiltinMap {
            key,
            val: map_val_ty,
        } if key_is_string(key) => {
            let map_ptr = val.into_pointer_value();
            let val_ty = map_val_ty.as_ref().clone();

            let builder_val = cg
                .builder
                .build_call(cg.rt.ynz_string_builder_new, &[], "mdbg_bld")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("builder_new void")?
                .into_pointer_value();

            let append_static = |cg: &mut Cg<'ctx, '_>, bld: PointerValue<'ctx>, s: &str| {
                let g = build_string_global(cg.ctx, cg.module, s, "mdbg_lit");
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_builder_append,
                        &[bld.into(), g.as_pointer_value().into()],
                        "",
                    )
                    .map_err(|e| format!("{e}"))
                    .map(|_| ())
            };

            append_static(cg, builder_val, "{ ")?;

            let count = cg
                .builder
                .build_call(cg.rt.ynz_map_count, &[map_ptr.into()], "mdbg_cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("map_count void")?
                .into_int_value();

            let i_slot = cg
                .builder
                .build_alloca(cg.i64(), "mdbg_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;

            let cond_bb = cg.append_block("mdbg_cond");
            let body_bb = cg.append_block("mdbg_body");
            let after_bb = cg.append_block("mdbg_after");
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(cond_bb);
            let i = cg
                .builder
                .build_load(cg.i64(), i_slot, "mdbg_iv")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let lt = cg
                .builder
                .build_int_compare(IntPredicate::SLT, i, count, "mdbg_lt")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(lt, body_bb, after_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(body_bb);

            let is_first = cg
                .builder
                .build_int_compare(IntPredicate::EQ, i, cg.i64().const_zero(), "mdbg_first")
                .map_err(|e| format!("{e}"))?;
            let sep_bb = cg.append_block("mdbg_sep");
            let entry_bb = cg.append_block("mdbg_entry");
            cg.builder
                .build_conditional_branch(is_first, entry_bb, sep_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(sep_bb);
            append_static(cg, builder_val, ", ")?;
            cg.builder
                .build_unconditional_branch(entry_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(entry_bb);

            let triple_ty = cg
                .ctx
                .struct_type(&[cg.i64().into(), cg.i64().into(), cg.i64().into()], false);
            let triple_slot = cg
                .builder
                .build_alloca(triple_ty, "mdbg_triple")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_map_iter_get_str,
                    &[map_ptr.into(), i.into(), triple_slot.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;

            let key_gep = cg
                .builder
                .build_struct_gep(triple_ty, triple_slot, 1, "mdbg_kgep")
                .map_err(|e| format!("{e}"))?;
            let key_ptr = cg
                .builder
                .build_load(cg.i64(), key_gep, "mdbg_kbits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let key_str = cg
                .builder
                .build_int_to_ptr(key_ptr, cg.ptr(), "mdbg_kptr")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), key_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;
            append_static(cg, builder_val, ": ")?;

            let val_gep = cg
                .builder
                .build_struct_gep(triple_ty, triple_slot, 2, "mdbg_vgep")
                .map_err(|e| format!("{e}"))?;
            let val_bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "mdbg_vbits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let val_val = cg.i64_bits_to(val_bits, &val_ty)?;
            let val_str = to_c_string(cg, val_val, &val_ty)?;
            cg.builder
                .build_call(
                    cg.rt.ynz_string_builder_append,
                    &[builder_val.into(), val_str.into()],
                    "",
                )
                .map_err(|e| format!("{e}"))?;

            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "mdbg_ni")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(cond_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(after_bb);
            append_static(cg, builder_val, " }")?;

            let result = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_builder_finalize,
                    &[builder_val.into()],
                    "mdbg_str",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("finalize void")?
                .into_pointer_value();
            Ok(result)
        }
        Type::BuiltinMap { .. } => {
            let g = build_string_global(cg.ctx, cg.module, "{ <map> }", "map_ph");
            Ok(g.as_pointer_value())
        }

        // Union printing: for `T | nothing`, check null → "none", else print as T.
        // For other union shapes, show a placeholder — full union introspection is future work.
        Type::Union { variants } => {
            let non_nothing: Vec<&Type> = variants
                .iter()
                .filter(|v| !matches!(v, Type::Nothing))
                .collect();

            if non_nothing.len() == 1 {
                let inner = non_nothing[0].clone();
                let ptr = val.into_pointer_value();
                let is_null = cg
                    .builder
                    .build_is_null(ptr, "union_null")
                    .map_err(|e| format!("{e}"))?;

                let none_bb = cg.append_block("union_none");
                let some_bb = cg.append_block("union_some");
                let merge_bb = cg.append_block("union_merge");

                cg.builder
                    .build_conditional_branch(is_null, none_bb, some_bb)
                    .map_err(|e| format!("{e}"))?;

                cg.builder.position_at_end(none_bb);
                let none_str = build_string_global(cg.ctx, cg.module, "none", "union_none_str");
                cg.builder
                    .build_unconditional_branch(merge_bb)
                    .map_err(|e| format!("{e}"))?;

                cg.builder.position_at_end(some_bb);
                let some_str = to_c_string(cg, val, &inner)?;
                cg.builder
                    .build_unconditional_branch(merge_bb)
                    .map_err(|e| format!("{e}"))?;
                let some_bb_end = cg.builder.get_insert_block().unwrap();

                cg.builder.position_at_end(merge_bb);
                let phi = cg
                    .builder
                    .build_phi(cg.ptr(), "union_str")
                    .map_err(|e| format!("{e}"))?;
                phi.add_incoming(&[
                    (&none_str.as_pointer_value(), none_bb),
                    (&some_str, some_bb_end),
                ]);
                Ok(phi.as_basic_value().into_pointer_value())
            } else {
                let g = build_string_global(cg.ctx, cg.module, "<union>", "union_placeholder");
                Ok(g.as_pointer_value())
            }
        }

        // Options type — val is the i64-extended i8 tag; delegate to options toString.
        Type::Options { name } => {
            // Cast i64 back to i8 for lower_options_to_string which expects an i8.
            let tag_i64 = val.into_int_value();
            let tag_i8 = cg.builder
                .build_int_truncate(tag_i64, cg.ctx.i8_type(), "opt_tag_i8")
                .map_err(|e| format!("{e}"))?;
            lower_options_to_string(cg, tag_i8.into(), name)
                .map(|v| v.into_pointer_value())
        }

        _ => Err(format!("codegen: cannot convert {:?} to string", ty)),
    }
}

/// Convert any Yinz value to a null-terminated C string pointer for interpolation.
///
/// Delegates to `to_c_string` for the types it knows. For Options types, emits a call
/// to the options toString runtime helper. For all other types, defers to `to_c_string`.
fn expr_to_cstring<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
) -> Result<inkwell::values::PointerValue<'ctx>, String> {
    match ty {
        Type::Options { name } => {
            lower_options_to_string(cg, val, name).map(|v| v.into_pointer_value())
        }
        _ => to_c_string(cg, val, ty),
    }
}

/// UFCS dispatch: `receiver.method(args)` → call standalone function `method(receiver, args)`.
fn lower_ufcs_call<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv_val: BasicValueEnum<'ctx>,
    _shape_name: &str,
    method: &str,
    extra_args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    let fn_val = cg
        .module
        .get_function(method)
        .ok_or_else(|| format!("codegen UFCS: function `{}` not found in module", method))?;

    let mut args: Vec<BasicMetadataValueEnum<'ctx>> = vec![recv_val.into()];
    for arg in extra_args {
        let arg_val = lower_expr(cg, arg)?;
        // If the function param expects dynamic but arg is a concrete shape, coerce.
        args.push(arg_val.into());
    }

    let call = cg
        .builder
        .build_call(fn_val, &args, "ufcs")
        .map_err(|e| format!("UFCS call `{method}`: {e}"))?;
    match call.try_as_basic_value().basic() {
        Some(v) => Ok(v),
        None => Ok(cg.i32().const_int(0, false).into()),
    }
}

/// Field access: lower `receiver.field` to the field's value.
///
/// Handles `maybe<T>.value` (extract the inner value from the {i64,i64} slot) and
/// shape field GEP for user-defined shapes.
fn lower_field_access<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    field_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let recv_ty = cg.expr_type(receiver);

    // MapEntry field access: entry.key → field 0, entry.value → field 1.
    if let Type::MapEntry { key, val } = &recv_ty {
        let entry_ty = cg
            .ctx
            .struct_type(&[cg.i64().into(), cg.i64().into()], false);
        let entry_ptr = lower_expr(cg, receiver)?.into_pointer_value();
        let (field_idx, field_ty) = match field_name {
            "key" => (0u32, key.as_ref().clone()),
            "value" => (1u32, val.as_ref().clone()),
            f => return Err(format!("MapEntry has no field `{f}`")),
        };
        let gep = cg
            .builder
            .build_struct_gep(entry_ty, entry_ptr, field_idx, field_name)
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), gep, "me_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        return cg.i64_bits_to(bits, &field_ty);
    }

    // maybe<T>.value — extract value bits from the {i64,i64} alloca.
    if let Type::Maybe { inner } = &recv_ty {
        let inner = inner.as_ref().clone();
        let ptr = lower_expr(cg, receiver)?.into_pointer_value();
        let val_gep = cg
            .builder
            .build_struct_gep(cg.maybe_type(), ptr, 1, "mv_gep")
            .map_err(|e| format!("{e}"))?;
        let bits = cg
            .builder
            .build_load(cg.i64(), val_gep, "mv_bits")
            .map_err(|e| format!("{e}"))?
            .into_int_value();
        return cg.i64_bits_to(bits, &inner);
    }

    let (field_ptr, field_ty) = field_gep(cg, receiver, field_name)?;
    load(cg, field_ptr, &field_ty, field_name)
}

/// Get a GEP pointer to a field inside a shape value (for reads AND writes).
fn field_gep<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    field_name: &str,
) -> Result<(PointerValue<'ctx>, Type), String> {
    let recv_ty = cg.expr_type(receiver);
    let shape_name = match &recv_ty {
        Type::Shape { name } => name.clone(),
        _ => {
            return Err(format!(
                "field_gep: receiver is not a Shape, got {:?}",
                recv_ty
            ))
        }
    };

    let shape_def = cg
        .shape_table
        .get(&shape_name)
        .ok_or_else(|| format!("field_gep: shape `{}` not in table", shape_name))?;
    let field_idx = shape_def
        .fields
        .iter()
        .position(|f| f.name == field_name)
        .ok_or_else(|| {
            format!(
                "field_gep: field `{}` not found in shape `{}`",
                field_name, shape_name
            )
        })?;
    let field_ty = shape_def.fields[field_idx].ty.clone();

    let struct_ty = cg
        .shape_types
        .get(&shape_name)
        .ok_or_else(|| format!("field_gep: LLVM type for shape `{}` not found", shape_name))?;

    // Lower the receiver to get a pointer to the struct.
    let recv_ptr = lower_expr(cg, receiver)?;

    let gep = cg
        .builder
        .build_struct_gep(
            struct_ty,
            recv_ptr.into_pointer_value(),
            field_idx as u32,
            field_name,
        )
        .map_err(|e| format!("GEP field `{}`: {e}", field_name))?;

    Ok((gep, field_ty))
}

/// Lower a struct literal to a stack-allocated value; return pointer to it.
fn lower_struct_lit<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    expr: &Expr,
    fields: &[ynz_ast::nodes::StructLitField],
) -> Result<BasicValueEnum<'ctx>, String> {
    let ty = cg.expr_type(expr);
    let Type::Shape { name: shape_name } = &ty else {
        return Err(format!("struct literal with non-shape type {:?}", ty));
    };
    let shape_name = shape_name.clone();

    let struct_ty = cg
        .shape_types
        .get(&shape_name)
        .ok_or_else(|| format!("struct_lit: LLVM type for `{}` not found", shape_name))?;

    // Allocate the struct on the stack.
    let slot = cg
        .builder
        .build_alloca(struct_ty, &shape_name)
        .map_err(|e| format!("alloca {}: {e}", shape_name))?;

    // Zero-initialize — covers hidden fields that aren't provided.
    let zero = struct_ty.const_zero();
    cg.builder
        .build_store(slot, zero)
        .map_err(|e| format!("zero-init {}: {e}", shape_name))?;

    let shape_def = cg
        .shape_table
        .get(&shape_name)
        .ok_or_else(|| format!("struct_lit: shape `{}` not in table", shape_name))?
        .clone();

    // Evaluate and store each provided field.
    for lit_field in fields {
        let field_idx = shape_def
            .fields
            .iter()
            .position(|f| f.name == lit_field.name)
            .ok_or_else(|| format!("struct_lit: unknown field `{}`", lit_field.name))?;
        let field_ty = shape_def.fields[field_idx].ty.clone();

        let gep = cg
            .builder
            .build_struct_gep(struct_ty, slot, field_idx as u32, &lit_field.name)
            .map_err(|e| format!("struct_lit GEP `{}`: {e}", lit_field.name))?;

        let val = lower_expr(cg, &lit_field.value)?;
        store_field(cg, val, &field_ty, gep)?;
    }

    // Hidden-field defaults are not evaluated here — zero-init covers M4's
    // restricted defaults (constants and empty literals). String/shape hidden
    // fields with non-zero defaults require a setter. See todos.md.

    Ok(slot.into())
}

/// Field assignment helper: `target.field = value`.
fn lower_stmt_field_assign<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    target: &Expr,
    value: &Expr,
) -> Result<(), String> {
    // Expect target to be Expr::FieldAccess
    let Expr::FieldAccess {
        receiver, field, ..
    } = target
    else {
        return Err(format!(
            "codegen: FieldAssign target is not FieldAccess: {:?}",
            target
        ));
    };

    let (field_ptr, field_ty) = field_gep(cg, receiver, field)?;
    let val = lower_expr(cg, value)?;
    store_field(cg, val, &field_ty, field_ptr)
}

/// PostfixOp lowering: `.copy()` and `.freeze()`.
fn lower_postfix_op<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    op: &ynz_ast::nodes::PostfixOpKind,
) -> Result<BasicValueEnum<'ctx>, String> {
    use ynz_ast::nodes::PostfixOpKind;
    match op {
        PostfixOpKind::Freeze => {
            // `.freeze()` is a typeck-only operation; no codegen change.
            lower_expr(cg, receiver)
        }
        PostfixOpKind::Copy => {
            let recv_ty = cg.expr_type(receiver);
            let recv_val = lower_expr(cg, receiver)?;
            match &recv_ty {
                Type::Shape { name } => {
                    // Trivially-copyable shape: memcpy into a fresh alloca.
                    let name = name.clone();
                    let struct_ty = cg
                        .shape_types
                        .get(&name)
                        .ok_or_else(|| format!(".copy(): LLVM type for `{}` not found", name))?;
                    let new_slot = cg
                        .builder
                        .build_alloca(struct_ty, &format!("{}_copy", name))
                        .map_err(|e| format!(".copy alloca: {e}"))?;
                    // Load the struct value and store into the new slot.
                    let val = cg
                        .builder
                        .build_load(struct_ty, recv_val.into_pointer_value(), "copy_src")
                        .map_err(|e| format!(".copy load: {e}"))?;
                    cg.builder
                        .build_store(new_slot, val)
                        .map_err(|e| format!(".copy store: {e}"))?;
                    Ok(new_slot.into())
                }
                // For primitives, the value is already by-value — just return it.
                _ => Ok(recv_val),
            }
        }
    }
}

/// Emit a type-attached constant such as `int.max` or `number.epsilon`.
///
/// All data (value_type and value_literal) lives in registry/features.toml.
fn emit_type_const<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    type_name: &str,
    const_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let entry = ynz_registry::type_attached_constant_lookup(type_name, const_name)
        .ok_or_else(|| format!("no registry entry for {type_name}.{const_name}"))?;

    match entry.value_type {
        "int" => {
            let v: i64 = entry.value_literal.parse()
                .map_err(|e| format!("int literal parse error for {type_name}.{const_name}: {e}"))?;
            Ok(cg.i64().const_int(v as u64, v < 0).into())
        }
        "float" => {
            let v: f64 = entry.value_literal.parse()
                .map_err(|e| format!("float literal parse error for {type_name}.{const_name}: {e}"))?;
            Ok(cg.f64().const_float(v).into())
        }
        "number" => {
            let s = entry.value_literal;
            let bits = ynz_numerics::parse(s)
                .ok_or_else(|| format!("failed to parse decimal128 constant `{s}`"))?;
            let slot = cg
                .builder
                .build_alloca(cg.i128(), const_name)
                .map_err(|e| format!("{e}"))?;
            let val = cg.i128().const_int_arbitrary_precision(&[
                (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
                (bits >> 64) as u64,
            ]);
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
            Ok(slot.into())
        }
        other => Err(format!(
            "codegen: unknown value_type {other:?} for type-attached constant `{type_name}.{const_name}`"
        )),
    }
}

/// True for method calls that take one primitive argument (dispatched via `lower_method_call_1arg`).
fn is_1arg_intrinsic(recv_ty: &Type, method: &str) -> bool {
    matches!(
        (recv_ty, method),
        (Type::Int, "wrappingAdd")
            | (Type::Int, "wrappingSub")
            | (Type::Int, "wrappingMul")
            | (Type::Int, "saturatingAdd")
            | (Type::Int, "saturatingSub")
            | (Type::Int, "saturatingMul")
    )
}

/// Lower a one-arg primitive intrinsic method call.
fn lower_method_call_1arg<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    recv_ty: &Type,
    method: &str,
    arg: BasicValueEnum<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let a = recv.into_int_value();
    let b = arg.into_int_value();
    match (recv_ty, method) {
        // Wrapping: plain LLVM add/sub/mul — two's complement overflow is the default.
        (Type::Int, "wrappingAdd") => cg
            .builder
            .build_int_add(a, b, "wadd")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Type::Int, "wrappingSub") => cg
            .builder
            .build_int_sub(a, b, "wsub")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Type::Int, "wrappingMul") => cg
            .builder
            .build_int_mul(a, b, "wmul")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        // Saturating: LLVM `sadd.sat` / `ssub.sat` / `smul.fix.sat` (scale=0) intrinsics.
        (Type::Int, "saturatingAdd") => {
            let f = declare_sat_intrinsic(
                cg.module,
                cg.ctx,
                "llvm.sadd.sat.i64",
                cg.i64().fn_type(&[cg.i64().into(), cg.i64().into()], false),
            );
            let r = cg
                .builder
                .build_call(f, &[a.into(), b.into()], "sadd_sat")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("sadd.sat void")?)
        }
        (Type::Int, "saturatingSub") => {
            let f = declare_sat_intrinsic(
                cg.module,
                cg.ctx,
                "llvm.ssub.sat.i64",
                cg.i64().fn_type(&[cg.i64().into(), cg.i64().into()], false),
            );
            let r = cg
                .builder
                .build_call(f, &[a.into(), b.into()], "ssub_sat")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("ssub.sat void")?)
        }
        (Type::Int, "saturatingMul") => {
            // `llvm.smul.fix.sat.i64(a, b, scale=0)` gives saturating signed integer mul.
            let fn_ty = cg
                .i64()
                .fn_type(&[cg.i64().into(), cg.i64().into(), cg.i32().into()], false);
            let f = declare_sat_intrinsic(cg.module, cg.ctx, "llvm.smul.fix.sat.i64", fn_ty);
            let scale = cg.i32().const_int(0, false);
            let r = cg
                .builder
                .build_call(f, &[a.into(), b.into(), scale.into()], "smul_sat")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("smul.fix.sat void")?)
        }
        _ => Err(format!(
            "codegen: no 1-arg intrinsic for {recv_ty:?}.{method}"
        )),
    }
}

fn declare_sat_intrinsic<'ctx>(
    module: &inkwell::module::Module<'ctx>,
    ctx: &'ctx inkwell::context::Context,
    name: &str,
    fn_ty: inkwell::types::FunctionType<'ctx>,
) -> inkwell::values::FunctionValue<'ctx> {
    let _ = ctx; // ctx not needed — type already constructed
    module
        .get_function(name)
        .unwrap_or_else(|| module.add_function(name, fn_ty, None))
}

/// Find the mangled name for a generic function call by matching the base name in the mono table.
///
/// When there are multiple instantiations of the same generic (e.g. `identity<int>` and
/// `identity<string>`), this picks the first match. For P4a the typical case is a single
/// instantiation per call site — a precise match by argument types is a P4b refinement.
fn find_mono_name_by_args(
    mono_table: &MonomorphizationTable,
    fn_name: &str,
    arg_types: &[Type],
) -> Option<String> {
    // Try exact match on param types first.
    for (key, sig) in &mono_table.entries {
        if key.fn_name != fn_name {
            continue;
        }
        if sig.param_types.len() == arg_types.len()
            && sig.param_types.iter().zip(arg_types).all(|(a, b)| a == b)
        {
            return Some(mangle_mono_name(&key.fn_name, &key.type_args));
        }
    }
    // Fall back: first entry with matching name (single-instantiation case).
    mono_table
        .entries
        .keys()
        .find(|k| k.fn_name == fn_name)
        .map(|k| mangle_mono_name(&k.fn_name, &k.type_args))
}

// ── BuiltinArray method dispatch ─────────────────────────────────────────────

fn lower_array_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    arr: PointerValue<'ctx>,
    elem: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        "count" => {
            let n = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr.into()], "arr_count")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_array_count returned void")?;
            Ok(n)
        }
        "add" => {
            let val = lower_expr(cg, &args[0])?;
            let elem_ty = cg.expr_type(&args[0]);
            let bits = cg.to_i64_bits(val, &elem_ty)?;
            cg.builder
                .build_call(cg.rt.ynz_array_push, &[arr.into(), bits.into()], "arr_add")
                .map_err(|e| format!("{e}"))?;
            Ok(cg.i64().const_zero().into())
        }
        "get" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "get_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), idx.into(), out.into()],
                    "arr_get_m",
                )
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(out.into())
        }
        "first" => {
            let idx = cg.i64().const_zero();
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "first_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), idx.into(), out.into()],
                    "arr_first",
                )
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(out.into())
        }
        "last" => {
            let cnt = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr.into()], "cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("count void")?
                .into_int_value();
            let idx = cg
                .builder
                .build_int_sub(cnt, cg.i64().const_int(1, false), "last_idx")
                .map_err(|e| format!("{e}"))?;
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "last_out")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), idx.into(), out.into()],
                    "arr_last",
                )
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(out.into())
        }
        "set" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let val = lower_expr(cg, &args[1])?;
            let elem_ty = cg.expr_type(&args[1]);
            let bits = cg.to_i64_bits(val, &elem_ty)?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_set,
                    &[arr.into(), idx.into(), bits.into()],
                    "arr_set_m",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(cg.i64().const_zero().into())
        }
        "contains" => {
            // Linear scan: bool result.
            let cnt = cg
                .builder
                .build_call(cg.rt.ynz_array_count, &[arr.into()], "cnt")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("count void")?
                .into_int_value();
            let target_val = lower_expr(cg, &args[0])?;
            let target_ty = cg.expr_type(&args[0]);
            let target_bits = cg.to_i64_bits(target_val, &target_ty)?;

            let result_slot = cg
                .builder
                .build_alloca(cg.bool(), "contains_res")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(result_slot, cg.bool().const_int(0, false))
                .map_err(|e| format!("{e}"))?;
            let i_slot = cg
                .builder
                .build_alloca(cg.i64(), "ci")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;

            let loop_bb = cg.append_block("contains_loop");
            let body_bb = cg.append_block("contains_body");
            let found_bb = cg.append_block("c_found");
            let cont_bb = cg.append_block("c_cont");
            let exit_bb = cg.append_block("contains_exit");

            cg.builder
                .build_unconditional_branch(loop_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(loop_bb);
            let i = cg
                .builder
                .build_load(cg.i64(), i_slot, "i")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let cmp = cg
                .builder
                .build_int_compare(IntPredicate::SLT, i, cnt, "lt")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(cmp, body_bb, exit_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(body_bb);
            let out = cg
                .builder
                .build_alloca(cg.maybe_type(), "c_get")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(
                    cg.rt.ynz_array_get,
                    &[arr.into(), i.into(), out.into()],
                    "c_get_call",
                )
                .map_err(|e| format!("{e}"))?;
            let val_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), out, 1, "c_val")
                .map_err(|e| format!("{e}"))?;
            let elem_bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "c_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let eq = cg
                .builder
                .build_int_compare(IntPredicate::EQ, elem_bits, target_bits, "eq")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_conditional_branch(eq, found_bb, cont_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(found_bb);
            cg.builder
                .build_store(result_slot, cg.bool().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(exit_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(cont_bb);
            let next_i = cg
                .builder
                .build_int_add(i, cg.i64().const_int(1, false), "next_i")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(i_slot, next_i)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(loop_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(exit_bb);
            let res = cg
                .builder
                .build_load(cg.bool(), result_slot, "res")
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(res)
        }
        other => Err(format!("array method `{other}` not yet lowered in P4a")),
    }
}

// ── BuiltinFixed method dispatch ──────────────────────────────────────────────

fn lower_fixed_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    arr: PointerValue<'ctx>,
    elem: &Type,
    size: Option<usize>,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        "count" => {
            let n = size.unwrap_or(0) as u64;
            Ok(cg.i64().const_int(n, false).into())
        }
        "get" | "first" | "last" => {
            let n = size.unwrap_or(0) as u64;
            let idx = if method == "first" {
                cg.i64().const_zero()
            } else if method == "last" && n > 0 {
                cg.i64().const_int(n - 1, false)
            } else {
                lower_expr(cg, &args[0])?.into_int_value()
            };
            let maybe_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "fg")
                .map_err(|e| format!("{e}"))?;
            let in_bounds = if n > 0 {
                let idx_ext = cg
                    .builder
                    .build_int_z_extend_or_bit_cast(idx, cg.i64(), "ie")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_int_compare(
                        IntPredicate::ULT,
                        idx_ext,
                        cg.i64().const_int(n, false),
                        "ib",
                    )
                    .map_err(|e| format!("{e}"))?
            } else {
                cg.bool().const_int(0, false)
            };
            let ok_bb = cg.append_block("fg_ok");
            let oob_bb = cg.append_block("fg_oob");
            let merge_bb = cg.append_block("fg_merge");
            cg.builder
                .build_conditional_branch(in_bounds, ok_bb, oob_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(ok_bb);
            let gep = unsafe {
                cg.builder
                    .build_gep(cg.i64(), arr, &[idx], "fg_elem")
                    .map_err(|e| format!("{e}"))?
            };
            let bits = cg
                .builder
                .build_load(cg.i64(), gep, "bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let has0 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_ok")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(has0, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let val0 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_ok")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val0, bits)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(oob_bb);
            let has1 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "has_oob")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(has1, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            let val1 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "val_oob")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val1, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(merge_bb)
                .map_err(|e| format!("{e}"))?;

            cg.builder.position_at_end(merge_bb);
            let _ = elem;
            Ok(maybe_slot.into())
        }
        "set" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let val = lower_expr(cg, &args[1])?;
            let vty = cg.expr_type(&args[1]);
            let bits = cg.to_i64_bits(val, &vty)?;
            let gep = unsafe {
                cg.builder
                    .build_gep(cg.i64(), arr, &[idx], "fs_elem")
                    .map_err(|e| format!("{e}"))?
            };
            cg.builder
                .build_store(gep, bits)
                .map_err(|e| format!("{e}"))?;
            let _ = elem;
            Ok(cg.i64().const_zero().into())
        }
        other => Err(format!("fixed method `{other}` not yet lowered in P4a")),
    }
}

// ── Maybe method dispatch ─────────────────────────────────────────────────────

fn lower_maybe_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    maybe_ptr: PointerValue<'ctx>,
    inner: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        "exists" => {
            let has_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_ptr, 0, "has")
                .map_err(|e| format!("{e}"))?;
            let has = cg
                .builder
                .build_load(cg.i64(), has_gep, "has_val")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let b = cg
                .builder
                .build_int_truncate(has, cg.bool(), "exists_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "or" => {
            let has_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_ptr, 0, "or_has")
                .map_err(|e| format!("{e}"))?;
            let has = cg
                .builder
                .build_load(cg.i64(), has_gep, "or_hv")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let cond = cg
                .builder
                .build_int_compare(IntPredicate::NE, has, cg.i64().const_zero(), "or_cond")
                .map_err(|e| format!("{e}"))?;

            let val_gep = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_ptr, 1, "or_val_gep")
                .map_err(|e| format!("{e}"))?;
            let bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "or_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let inner_val = cg.i64_bits_to(bits, inner)?;
            let default_val = lower_expr(cg, &args[0])?;

            cg.builder
                .build_select(cond, inner_val, default_val, "or_res")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!("maybe method `{other}` not yet lowered in P4a")),
    }
}

// ── M7 P4a: errors-capable helpers ───────────────────────────────────────────

/// True when the named function in `typed_module` has `errors_capable = true`.
fn is_errors_capable_fn(typed: &TypedModule, fn_name: &str) -> bool {
    typed.module.items.iter().any(|item| {
        if let ynz_ast::nodes::Item::Function(f) = item {
            f.name == fn_name && f.errors_capable
        } else {
            false
        }
    })
}

/// Emit auto-propagation for an errors-capable result struct.
///
/// Emits IR that:
/// 1. Extracts the error pointer from `result_struct`.
/// 2. If non-null → pops the current frame, returns the error wrapped in
///    `{error_ptr, 0}` (early-exit path).
/// 3. If null → falls through to a "success" block and returns the success
///    value converted to the given inner type.
///
/// Must only be called when `cg.is_errors_capable = true`.
fn lower_ec_auto_propagate<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    result_struct: inkwell::values::StructValue<'ctx>,
    inner_ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_ty = errors_result_type(cg.ctx);

    let err_ptr_i64 = cg
        .builder
        .build_extract_value(result_struct, 0, "ap_err")
        .map_err(|e| format!("ap extract err: {e}"))?
        .into_int_value();
    let is_err = cg
        .builder
        .build_int_compare(
            IntPredicate::NE,
            err_ptr_i64,
            cg.i64().const_int(0, false),
            "ap_is_err",
        )
        .map_err(|e| format!("ap icmp: {e}"))?;

    let propagate_bb = cg.append_block("ap_propagate");
    let success_bb = cg.append_block("ap_success");
    cg.builder
        .build_conditional_branch(is_err, propagate_bb, success_bb)
        .map_err(|e| format!("ap branch: {e}"))?;

    // Propagation block: pop frame, wrap error, early-return.
    cg.builder.position_at_end(propagate_bb);
    cg.builder
        .build_call(cg.rt.ynz_frame_pop, &[], "ap_pop")
        .map_err(|e| format!("ap pop: {e}"))?;
    let mut prop_result = result_ty.const_zero();
    prop_result = cg
        .builder
        .build_insert_value(prop_result, err_ptr_i64, 0, "prop_err")
        .map_err(|e| format!("prop ins err: {e}"))?
        .into_struct_value();
    prop_result = cg
        .builder
        .build_insert_value(prop_result, cg.i64().const_int(0, false), 1, "prop_val")
        .map_err(|e| format!("prop ins val: {e}"))?
        .into_struct_value();
    cg.builder
        .build_return(Some(&prop_result))
        .map_err(|e| format!("ap early return: {e}"))?;

    // Success block: extract the success bits and convert to the inner type.
    cg.builder.position_at_end(success_bb);
    let success_bits = cg
        .builder
        .build_extract_value(result_struct, 1, "ap_val")
        .map_err(|e| format!("ap extract val: {e}"))?
        .into_int_value();
    cg.i64_bits_to(success_bits, inner_ty)
}

/// Handle the `{i64 error_ptr, i64 success_val}` struct returned from an
/// errors-capable function call.
///
/// Stores the result struct in a stack alloca.
///
/// - **In an errors-capable caller**: also emits an auto-propagation check
///   immediately. If error: pop frame + early-return. Returns a flag in
///   `errors_capable_locals` so the subsequent `Ident` use site knows it needs
///   to load the success value from the struct.
/// - **Outside an errors-capable caller**: just stores the struct; the caller
///   handles it via `.or()` / `.failed()`.
fn lower_errors_capable_call_result<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    result_struct: inkwell::values::StructValue<'ctx>,
    _callee_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_ty = errors_result_type(cg.ctx);
    let slot = cg
        .builder
        .build_alloca(result_ty, "ec_result")
        .map_err(|e| format!("ec_result alloca: {e}"))?;
    cg.builder
        .build_store(slot, result_struct)
        .map_err(|e| format!("ec_result store: {e}"))?;
    Ok(slot.into())
}

/// Dispatch `.failed()` and `.or(default)` on an `ErrorsCapable` value.
///
/// The receiver is a pointer to a stack-allocated `{i64 error_ptr, i64 success_val}`.
fn lower_errors_capable_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv_val: BasicValueEnum<'ctx>,
    inner: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_ty = errors_result_type(cg.ctx);
    let recv_ptr = recv_val.into_pointer_value();

    match method {
        "failed" => {
            // .failed() → bool: true when error_ptr != 0.
            let err_gep = cg
                .builder
                .build_struct_gep(result_ty, recv_ptr, 0, "ec_err_gep")
                .map_err(|e| format!("{e}"))?;
            let err_ptr = cg
                .builder
                .build_load(cg.i64(), err_gep, "ec_err_ptr")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let is_failed = cg
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    err_ptr,
                    cg.i64().const_int(0, false),
                    "ec_failed",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(is_failed.into())
        }
        "or" => {
            // .or(default) → inner_type: error_ptr == 0 ? success_val : default.
            let err_gep = cg
                .builder
                .build_struct_gep(result_ty, recv_ptr, 0, "ec_or_err_gep")
                .map_err(|e| format!("{e}"))?;
            let err_ptr = cg
                .builder
                .build_load(cg.i64(), err_gep, "ec_or_err")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let is_ok = cg
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    err_ptr,
                    cg.i64().const_int(0, false),
                    "ec_is_ok",
                )
                .map_err(|e| format!("{e}"))?;

            let val_gep = cg
                .builder
                .build_struct_gep(result_ty, recv_ptr, 1, "ec_or_val_gep")
                .map_err(|e| format!("{e}"))?;
            let bits = cg
                .builder
                .build_load(cg.i64(), val_gep, "ec_or_bits")
                .map_err(|e| format!("{e}"))?
                .into_int_value();
            let success_val = cg.i64_bits_to(bits, inner)?;
            let default_val = lower_expr(cg, &args[0])?;

            cg.builder
                .build_select(is_ok, success_val, default_val, "ec_or_res")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!(
            "errors-capable method `{other}` not yet implemented in M7 P4a"
        )),
    }
}

// ── Helper: test if a map key type is String ──────────────────────────────────

fn key_is_string(key_ty: &Type) -> bool {
    matches!(key_ty, Type::String)
}

// ── Map method dispatch (M5 P4b) ──────────────────────────────────────────────

fn lower_map_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    map: PointerValue<'ctx>,
    key_ty: &Type,
    val_ty: &Type,
    method: &str,
    args: &[Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    let is_str = key_is_string(key_ty);
    match method {
        "count" => {
            let n = cg
                .builder
                .build_call(cg.rt.ynz_map_count, &[map.into()], "mc")
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("map_count void")?;
            Ok(n)
        }
        "has" => {
            let key_val = lower_expr(cg, &args[0])?;
            let n = if is_str {
                let pair_ty = cg
                    .ctx
                    .struct_type(&[cg.i64().into(), cg.i64().into()], false);
                let out = cg
                    .builder
                    .build_alloca(pair_ty, "has_out")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_get_str,
                        &[map.into(), key_val.into(), out.into()],
                        "hg",
                    )
                    .map_err(|e| format!("{e}"))?;
                let has_gep = cg
                    .builder
                    .build_struct_gep(pair_ty, out, 0, "has0")
                    .map_err(|e| format!("{e}"))?;
                cg.builder
                    .build_load(cg.i64(), has_gep, "has_v")
                    .map_err(|e| format!("{e}"))?
            } else {
                let kt = cg.expr_type(&args[0]);
                let key_bits = cg.to_i64_bits(key_val, &kt)?;
                cg.builder
                    .build_call(cg.rt.ynz_map_has, &[map.into(), key_bits.into()], "mhas")
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("map_has void")?
            };
            let b = cg
                .builder
                .build_int_truncate(n.into_int_value(), cg.bool(), "has_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "get" => {
            let key_val = lower_expr(cg, &args[0])?;
            let pair_ty = cg
                .ctx
                .struct_type(&[cg.i64().into(), cg.i64().into()], false);
            let out = cg
                .builder
                .build_alloca(pair_ty, "mget_out")
                .map_err(|e| format!("{e}"))?;
            if is_str {
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_get_str,
                        &[map.into(), key_val.into(), out.into()],
                        "mg_s",
                    )
                    .map_err(|e| format!("{e}"))?;
            } else {
                let kt = cg.expr_type(&args[0]);
                let key_bits = cg.to_i64_bits(key_val, &kt)?;
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_get,
                        &[map.into(), key_bits.into(), out.into()],
                        "mg",
                    )
                    .map_err(|e| format!("{e}"))?;
            }
            // Copy pair {i64 has, i64 bits} into a maybe<V> slot — identical layout.
            let maybe_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "mg_maybe")
                .map_err(|e| format!("{e}"))?;
            let has_src = cg
                .builder
                .build_struct_gep(pair_ty, out, 0, "hs")
                .map_err(|e| format!("{e}"))?;
            let val_src = cg
                .builder
                .build_struct_gep(pair_ty, out, 1, "vs")
                .map_err(|e| format!("{e}"))?;
            let has_v = cg
                .builder
                .build_load(cg.i64(), has_src, "hv")
                .map_err(|e| format!("{e}"))?;
            let val_v = cg
                .builder
                .build_load(cg.i64(), val_src, "vv")
                .map_err(|e| format!("{e}"))?;
            let has_dst = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 0, "hd")
                .map_err(|e| format!("{e}"))?;
            let val_dst = cg
                .builder
                .build_struct_gep(cg.maybe_type(), maybe_slot, 1, "vd")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(has_dst, has_v)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val_dst, val_v)
                .map_err(|e| format!("{e}"))?;
            let _ = val_ty;
            Ok(maybe_slot.into())
        }
        "set" => {
            let key_val = lower_expr(cg, &args[0])?;
            let val_val = lower_expr(cg, &args[1])?;
            let vt = cg.expr_type(&args[1]);
            let val_bits = cg.to_i64_bits(val_val, &vt)?;
            if is_str {
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_set_str,
                        &[map.into(), key_val.into(), val_bits.into()],
                        "ms_s",
                    )
                    .map_err(|e| format!("{e}"))?;
            } else {
                let kt = cg.expr_type(&args[0]);
                let key_bits = cg.to_i64_bits(key_val, &kt)?;
                cg.builder
                    .build_call(
                        cg.rt.ynz_map_set,
                        &[map.into(), key_bits.into(), val_bits.into()],
                        "ms",
                    )
                    .map_err(|e| format!("{e}"))?;
            }
            Ok(cg.i64().const_zero().into())
        }
        other => Err(format!("map method `{other}` not yet lowered in P4b")),
    }
}

// ── M6: options + fallible-conversion codegen helpers ────────────────────────

/// Lower `options_value.toString()` → call `ynz_string_from_static` with the variant name.
///
/// The options_table maps variant tags to names; we use a runtime switch to call
/// the right string. For simplicity, we build an LLVM switch with one case per variant.
fn lower_options_to_string<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    opts_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let entry = cg
        .options_table
        .options
        .get(opts_name)
        .ok_or_else(|| format!("codegen: unknown options type `{opts_name}`"))?
        .clone();

    // Allocate a pointer slot to hold the result string pointer.
    let result_slot = cg
        .builder
        .build_alloca(cg.ptr(), "opts_str_slot")
        .map_err(|e| format!("{e}"))?;
    // Default: empty string
    let empty_g = build_string_global(cg.ctx, cg.module, "", ".opts.str.empty");
    cg.builder
        .build_store(result_slot, empty_g.as_pointer_value())
        .map_err(|e| format!("{e}"))?;

    let merge_bb = cg.append_block("opts_str_merge");
    let tag_val = recv.into_int_value();

    // For each variant, emit a conditional branch: if tag == i, store variant-name string.
    let mut current_bb = cg
        .builder
        .get_insert_block()
        .ok_or("opts_to_string: no insert block")?;

    for (i, variant) in entry.variants.iter().enumerate() {
        // Use the display string if provided, otherwise fall back to the variant name.
        let display = entry.display_strings.get(i)
            .and_then(|d| d.as_deref())
            .unwrap_or(variant.as_str());
        let tag_const = cg.ctx.i8_type().const_int(i as u64, false);
        let is_this_variant = cg
            .builder
            .build_int_compare(inkwell::IntPredicate::EQ, tag_val, tag_const, "ots_cmp")
            .map_err(|e| format!("{e}"))?;

        let arm_bb = cg.append_block(&format!("opts_str_{i}"));
        let next_bb = if i + 1 < entry.variants.len() {
            cg.append_block(&format!("opts_str_check{}", i + 1))
        } else {
            merge_bb
        };

        cg.builder
            .build_conditional_branch(is_this_variant, arm_bb, next_bb)
            .map_err(|e| format!("{e}"))?;

        cg.builder.position_at_end(arm_bb);
        let g = build_string_global(
            cg.ctx,
            cg.module,
            display,
            &format!(".opts.{opts_name}.{variant}"),
        );
        let len = cg.i64().const_int(display.len() as u64, false);
        let ptr = cg
            .builder
            .build_call(
                cg.rt.ynz_string_from_static,
                &[g.as_pointer_value().into(), len.into()],
                "opt_str",
            )
            .map_err(|e| format!("{e}"))?
            .try_as_basic_value()
            .basic()
            .ok_or("ynz_string_from_static returned void")?;
        cg.builder
            .build_store(result_slot, ptr)
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| format!("{e}"))?;

        // Only position at next_bb if it's different from merge_bb.
        // If next_bb IS merge_bb (last iteration), don't reposition — the builder
        // is already in a terminated arm body and will fall into the next part.
        if next_bb != merge_bb {
            cg.builder.position_at_end(next_bb);
        }
        current_bb = next_bb;
    }
    let _ = current_bb;

    // Ensure the current block (the last check/default block, or merge_bb itself)
    // branches to merge_bb if not already terminated.
    if !is_block_terminated(cg) {
        cg.builder
            .build_unconditional_branch(merge_bb)
            .map_err(|e| format!("{e}"))?;
    }
    cg.builder.position_at_end(merge_bb);

    let result = cg
        .builder
        .build_load(cg.ptr(), result_slot, "opts_str")
        .map_err(|e| format!("{e}"))?;
    Ok(result)
}

/// Lower `(float).toInt()` → `maybe<int>` with NaN + range checks.
/// Locked codegen sequence per design/narrowing.md P4 step 9.
fn lower_float_to_int<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    x: inkwell::values::FloatValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let result_slot = cg
        .builder
        .build_alloca(cg.maybe_type(), "f2i_slot")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(result_slot, cg.maybe_type().const_zero())
        .map_err(|e| format!("{e}"))?;

    let nan_bb = cg.append_block("f2i_nan");
    let range_bb = cg.append_block("f2i_range");
    let low_bb = cg.append_block("f2i_low");
    let ok_bb = cg.append_block("f2i_ok");
    let ret_bb = cg.append_block("f2i_ret");

    // NaN check: `x != x` (unordered comparison)
    let is_nan = cg
        .builder
        .build_float_compare(inkwell::FloatPredicate::UNO, x, x, "is_nan")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(is_nan, nan_bb, range_bb)
        .map_err(|e| format!("{e}"))?;

    // NaN → return none (slot is already zeroed)
    cg.builder.position_at_end(nan_bb);
    cg.builder
        .build_unconditional_branch(ret_bb)
        .map_err(|e| format!("{e}"))?;

    // Upper range check: x >= 2^63 (9.223372036854776e18)
    cg.builder.position_at_end(range_bb);
    let i64_max_f64 = cg.f64().const_float(9.223372036854776e18_f64);
    let too_big = cg
        .builder
        .build_float_compare(inkwell::FloatPredicate::OGE, x, i64_max_f64, "too_big")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(too_big, nan_bb, low_bb)
        .map_err(|e| format!("{e}"))?;

    // Lower range check: x < -2^63
    cg.builder.position_at_end(low_bb);
    let i64_min_f64 = cg.f64().const_float(-9.223372036854776e18_f64);
    let too_small = cg
        .builder
        .build_float_compare(inkwell::FloatPredicate::OLT, x, i64_min_f64, "too_small")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_conditional_branch(too_small, nan_bb, ok_bb)
        .map_err(|e| format!("{e}"))?;

    // Convert: fptosi (safe — in-range proven)
    cg.builder.position_at_end(ok_bb);
    let int_val = cg
        .builder
        .build_float_to_signed_int(x, cg.i64(), "fptosi")
        .map_err(|e| format!("{e}"))?;
    let has_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), result_slot, 0, "has_gep")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(has_gep, cg.i64().const_int(1, false))
        .map_err(|e| format!("{e}"))?;
    let val_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), result_slot, 1, "val_gep")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(val_gep, int_val)
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_unconditional_branch(ret_bb)
        .map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ret_bb);
    Ok(result_slot.into())
}

/// Lower `(number).toInt()` → `maybe<int>`.
fn lower_number_to_int<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    num_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    // Use ynz_decimal_to_float then float_to_int.
    let f64t = cg.f64();
    let prt = cg.ptr();
    let fn_ty = f64t.fn_type(&[prt.into()], false);
    let f = cg
        .module
        .get_function("ynz_decimal_to_float")
        .unwrap_or_else(|| cg.module.add_function("ynz_decimal_to_float", fn_ty, None));
    let f_val = cg
        .builder
        .build_call(f, &[num_ptr.into()], "d2f")
        .map_err(|e| format!("{e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("decimal_to_float void")?
        .into_float_value();
    lower_float_to_int(cg, f_val)
}

/// Lower `(string).toInt()` → `maybe<int>` via `ynz_string_to_int` runtime.
fn lower_string_to_int<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    lower_string_to_maybe_primitive(cg, str_ptr, "ynz_string_to_int", false)
}

/// Lower `(string).toFloat()` → `maybe<float>` via `ynz_string_to_float` runtime.
fn lower_string_to_float<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    lower_string_to_maybe_primitive(cg, str_ptr, "ynz_string_to_float", false)
}

/// Lower `(string).toNumber()` → `maybe<number>` via `ynz_string_to_number` runtime.
fn lower_string_to_number<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    lower_string_to_maybe_primitive(cg, str_ptr, "ynz_string_to_number", true)
}

/// Shared implementation: call a `(ptr, len, out) -> void` runtime function;
/// the `out` buffer is `[i64; 2]` (has_value + value) or `[i64; 3]` for number.
/// Returns a pointer to a `maybe<T>` struct.
fn lower_string_to_maybe_primitive<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    str_ptr: inkwell::values::PointerValue<'ctx>,
    rt_fn_name: &str,
    is_number: bool,
) -> Result<BasicValueEnum<'ctx>, String> {
    // Allocate output buffer: [3 x i64] (enough for number, fine for int/float).
    let arr_ty = cg.i64().array_type(3);
    let out = cg
        .builder
        .build_alloca(arr_ty, "str_conv_out")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(out, arr_ty.const_zero())
        .map_err(|e| format!("{e}"))?;

    // Get string pointer and length from the Yinz string (null-terminated).
    // The runtime functions take (ptr, len) where len is the byte count.
    // For null-terminated strings, compute the length with strlen.
    let strlen_ty = cg.i64().fn_type(&[cg.ptr().into()], false);
    let strlen = cg
        .module
        .get_function("strlen")
        .unwrap_or_else(|| cg.module.add_function("strlen", strlen_ty, None));
    let len_val = cg
        .builder
        .build_call(strlen, &[str_ptr.into()], "str_len")
        .map_err(|e| format!("{e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("strlen void")?;

    // Look up or declare the runtime function.
    let fn_val = match rt_fn_name {
        "ynz_string_to_int" => cg.rt.ynz_string_to_int,
        "ynz_string_to_float" => cg.rt.ynz_string_to_float,
        "ynz_string_to_number" => cg.rt.ynz_string_to_number,
        _ => return Err(format!("unknown str conv fn: {rt_fn_name}")),
    };

    cg.builder
        .build_call(
            fn_val,
            &[str_ptr.into(), len_val.into(), out.into()],
            "str_conv",
        )
        .map_err(|e| format!("{e}"))?;

    // Build a `maybe<T>` struct from the output buffer.
    let slot = cg
        .builder
        .build_alloca(cg.maybe_type(), "str_conv_maybe")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(slot, cg.maybe_type().const_zero())
        .map_err(|e| format!("{e}"))?;

    let has_gep = unsafe {
        cg.builder
            .build_gep(cg.i64(), out, &[cg.i64().const_int(0, false)], "has_v")
    }
    .map_err(|e| format!("{e}"))?;
    let has_val = cg
        .builder
        .build_load(cg.i64(), has_gep, "has_val")
        .map_err(|e| format!("{e}"))?;

    // Store has_value
    let has_out_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), slot, 0, "has_out")
        .map_err(|e| format!("{e}"))?;
    cg.builder
        .build_store(has_out_gep, has_val)
        .map_err(|e| format!("{e}"))?;

    // Store value bits (index 1 for int/float, or for number we store the pointer-as-bits)
    let val_gep_src = unsafe {
        cg.builder
            .build_gep(cg.i64(), out, &[cg.i64().const_int(1, false)], "val_v")
    }
    .map_err(|e| format!("{e}"))?;

    let val_out_gep = cg
        .builder
        .build_struct_gep(cg.maybe_type(), slot, 1, "val_out")
        .map_err(|e| format!("{e}"))?;

    if is_number {
        // For number, store the pointer to the out buffer's number data as bits.
        let ptr_bits = cg
            .builder
            .build_ptr_to_int(val_gep_src, cg.i64(), "ptr2i")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(val_out_gep, ptr_bits)
            .map_err(|e| format!("{e}"))?;
    } else {
        let val = cg
            .builder
            .build_load(cg.i64(), val_gep_src, "val")
            .map_err(|e| format!("{e}"))?;
        cg.builder
            .build_store(val_out_gep, val)
            .map_err(|e| format!("{e}"))?;
    }

    Ok(slot.into())
}

// ── String method dispatch (M7 P4b) ──────────────────────────────────────────

fn lower_string_method<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    s_ptr: inkwell::values::PointerValue<'ctx>,
    method: &str,
    args: &[ynz_ast::nodes::Expr],
) -> Result<BasicValueEnum<'ctx>, String> {
    match method {
        // Zero-arg methods returning a string.
        "toUpperCase" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_to_upper, &[s_ptr.into()], "s_upper")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_to_upper void")?)
        }
        "toLowerCase" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_to_lower, &[s_ptr.into()], "s_lower")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_to_lower void")?)
        }
        "trim" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_trim, &[s_ptr.into()], "s_trim")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_trim void")?)
        }
        // Zero-arg methods returning int.
        "count" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_count, &[s_ptr.into()], "s_count")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_count void")?)
        }
        "byteCount" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_byte_count, &[s_ptr.into()], "s_bcount")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_byte_count void")?)
        }
        "graphemeCount" => {
            let r = cg
                .builder
                .build_call(cg.rt.ynz_string_grapheme_count, &[s_ptr.into()], "s_gcount")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_grapheme_count void")?)
        }
        // One-arg predicate methods returning bool.
        "contains" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_contains,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_contains",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_contains void")?
                .into_int_value();
            // Convert i32 → i1 (bool).
            let b = cg
                .builder
                .build_int_truncate(r, cg.bool(), "s_contains_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "startsWith" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_starts_with,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_sw",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_starts_with void")?
                .into_int_value();
            let b = cg
                .builder
                .build_int_truncate(r, cg.bool(), "s_sw_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        "endsWith" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_ends_with,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_ew",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_ends_with void")?
                .into_int_value();
            let b = cg
                .builder
                .build_int_truncate(r, cg.bool(), "s_ew_b")
                .map_err(|e| format!("{e}"))?;
            Ok(b.into())
        }
        // One-arg indexed access returning maybe<string>.
        "get" | "graphemeAt" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let raw_ptr = if method == "graphemeAt" {
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_grapheme_at,
                        &[s_ptr.into(), idx.into()],
                        "s_gat",
                    )
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_grapheme_at void")?
                    .into_pointer_value()
            } else {
                cg.builder
                    .build_call(
                        cg.rt.ynz_string_codepoint_at,
                        &[s_ptr.into(), idx.into()],
                        "s_cpat",
                    )
                    .map_err(|e| format!("{e}"))?
                    .try_as_basic_value()
                    .basic()
                    .ok_or("ynz_string_codepoint_at void")?
                    .into_pointer_value()
            };
            // Null pointer → none (tag=0); non-null → some (tag=1, value=ptr as i64).
            let result_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "s_get_res")
                .map_err(|e| format!("{e}"))?;
            let is_null = cg
                .builder
                .build_is_null(raw_ptr, "is_null")
                .map_err(|e| format!("{e}"))?;
            let some_bb = cg.append_block("s_get_some");
            let none_bb = cg.append_block("s_get_none");
            let done_bb = cg.append_block("s_get_done");
            cg.builder
                .build_conditional_branch(is_null, none_bb, some_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(none_bb);
            let tag_ptr = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "s_tag_n")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tag_ptr, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(some_bb);
            let tag_ptr2 = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "s_tag_s")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tag_ptr2, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let val_ptr = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 1, "s_val")
                .map_err(|e| format!("{e}"))?;
            let ptr_as_i64 = cg
                .builder
                .build_ptr_to_int(raw_ptr, cg.i64(), "ptr_i64")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(val_ptr, ptr_as_i64)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(done_bb);
            Ok(result_slot.into())
        }
        // One-arg: byteAt returns maybe<int>.
        "byteAt" => {
            let idx = lower_expr(cg, &args[0])?.into_int_value();
            let raw = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_byte_at,
                    &[s_ptr.into(), idx.into()],
                    "s_bat",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_byte_at void")?
                .into_int_value();
            // -1 means OOB → none; else → some(value).
            let result_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "s_bat_res")
                .map_err(|e| format!("{e}"))?;
            let minus_one = cg.i64().const_int(u64::MAX, true); // -1 as i64
            let is_oob = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, raw, minus_one, "bat_oob")
                .map_err(|e| format!("{e}"))?;
            let some_bb = cg.append_block("bat_some");
            let none_bb = cg.append_block("bat_none");
            let done_bb = cg.append_block("bat_done");
            cg.builder
                .build_conditional_branch(is_oob, none_bb, some_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(none_bb);
            let tp_n = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "bat_tn")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_n, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(some_bb);
            let tp_s = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "bat_ts")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_s, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let vp = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 1, "bat_v")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(vp, raw)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(done_bb);
            Ok(result_slot.into())
        }
        // One-arg: indexOf returns maybe<int>.
        "indexOf" => {
            let arg_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let raw = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_index_of,
                    &[s_ptr.into(), arg_ptr.into()],
                    "s_iof",
                )
                .map_err(|e| format!("{e}"))?
                .try_as_basic_value()
                .basic()
                .ok_or("ynz_string_index_of void")?
                .into_int_value();
            let result_slot = cg
                .builder
                .build_alloca(cg.maybe_type(), "s_iof_res")
                .map_err(|e| format!("{e}"))?;
            let minus_one = cg.i64().const_int(u64::MAX, true);
            let is_missing = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::EQ, raw, minus_one, "iof_miss")
                .map_err(|e| format!("{e}"))?;
            let some_bb = cg.append_block("iof_some");
            let none_bb = cg.append_block("iof_none");
            let done_bb = cg.append_block("iof_done");
            cg.builder
                .build_conditional_branch(is_missing, none_bb, some_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(none_bb);
            let tp_n = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "iof_tn")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_n, cg.i64().const_zero())
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(some_bb);
            let tp_s = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 0, "iof_ts")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tp_s, cg.i64().const_int(1, false))
                .map_err(|e| format!("{e}"))?;
            let vp = cg
                .builder
                .build_struct_gep(cg.maybe_type(), result_slot, 1, "iof_v")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(vp, raw)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_unconditional_branch(done_bb)
                .map_err(|e| format!("{e}"))?;
            cg.builder.position_at_end(done_bb);
            Ok(result_slot.into())
        }
        // Two-arg: substring(start: int, end: int) → string.
        "substring" => {
            let start = lower_expr(cg, &args[0])?.into_int_value();
            let end = lower_expr(cg, &args[1])?.into_int_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_substring,
                    &[s_ptr.into(), start.into(), end.into()],
                    "s_sub",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_substring void")?)
        }
        // One-arg: split(sep: string) → array<string>.
        "split" => {
            let sep_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_split,
                    &[s_ptr.into(), sep_ptr.into()],
                    "s_split",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_split void")?)
        }
        // Two-arg: replace(from: string, to: string) → string.
        "replace" => {
            let from_ptr = lower_expr(cg, &args[0])?.into_pointer_value();
            let to_ptr = lower_expr(cg, &args[1])?.into_pointer_value();
            let r = cg
                .builder
                .build_call(
                    cg.rt.ynz_string_replace,
                    &[s_ptr.into(), from_ptr.into(), to_ptr.into()],
                    "s_rep",
                )
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("ynz_string_replace void")?)
        }
        // toString() on string is identity.
        "toString" => Ok(s_ptr.into()),
        // Fallible conversions — forwarded to lower_method_call.
        "toInt" => lower_string_to_int(cg, s_ptr),
        "toFloat" => lower_string_to_float(cg, s_ptr),
        "toNumber" => lower_string_to_number(cg, s_ptr),
        other => Err(format!("codegen: unknown string method `{other}`")),
    }
}

fn lower_method_call<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    recv_ty: &Type,
    method: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    match (recv_ty, method) {
        (Type::Int, "toNumber") => {
            let out = cg
                .builder
                .build_alloca(cg.i128(), "i2n")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(cg.rt.decimal_from_int, &[recv.into(), out.into()], "")
                .map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }
        (Type::Int, "toFloat") => cg
            .builder
            .build_signed_int_to_float(recv.into_int_value(), cg.f64(), "i2f")
            .map(|v| v.into())
            .map_err(|e| format!("{e}")),
        (Type::Int, "toString") => {
            to_c_string(cg, recv, &Type::Int).map(|p: PointerValue<'ctx>| p.into())
        }
        (Type::Float, "toNumber") => {
            let out = cg
                .builder
                .build_alloca(cg.i128(), "f2n")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_call(cg.rt.decimal_from_float, &[recv.into(), out.into()], "")
                .map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }
        (Type::Float, "toString") => to_c_string(cg, recv, &Type::Float).map(|p| p.into()),
        (Type::Number { .. }, "toFloat") => {
            let f64t = cg.f64();
            let prt = cg.ptr();
            let fn_ty = f64t.fn_type(&[prt.into()], false);
            let f = cg
                .module
                .get_function("ynz_decimal_to_float")
                .unwrap_or_else(|| cg.module.add_function("ynz_decimal_to_float", fn_ty, None));
            let r = cg
                .builder
                .build_call(f, &[recv.into()], "d2f")
                .map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value()
                .basic()
                .ok_or("decimal_to_float void")?)
        }
        (Type::Number { .. }, "toString") => {
            to_c_string(cg, recv, &Type::Number { precision: 34 }).map(|p| p.into())
        }
        (Type::Bool, "toString") => to_c_string(cg, recv, &Type::Bool).map(|p| p.into()),

        // M6: fallible conversions — return maybe<T>
        (Type::Int, "toInt") => {
            // (int).toInt() is identity — return the int value directly (infallible).
            Ok(recv)
        }
        (Type::Float, "toInt") => lower_float_to_int(cg, recv.into_float_value()),
        (Type::Number { .. }, "toInt") => lower_number_to_int(cg, recv.into_pointer_value()),
        (Type::String, "toInt") => lower_string_to_int(cg, recv.into_pointer_value()),
        (Type::String, "toFloat") => lower_string_to_float(cg, recv.into_pointer_value()),
        (Type::String, "toNumber") => lower_string_to_number(cg, recv.into_pointer_value()),

        _ => Err(format!(
            "codegen: unknown method `{method}` on {:?}",
            recv_ty
        )),
    }
}

fn store<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
    slot: PointerValue<'ctx>,
) -> Result<(), String> {
    match ty {
        Type::Number { precision } if *precision <= 34 => {
            // Hardware decimal128: val is a ptr to i128; load the bits then store into slot.
            let bits = cg
                .builder
                .build_load(cg.i128(), val.into_pointer_value(), "dec_bits")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(slot, bits)
                .map_err(|e| format!("{e}"))?;
        }
        // M8 P6: bignum (N > 34) — val is a ptr to the string; store the pointer.
        Type::Number { .. } => {
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
        }
        // BuiltinArray is a pointer to the heap YnzArray; store the pointer.
        // BuiltinFixed is already stored as an alloca pointer; store the pointer.
        // Maybe is an alloca pointer to {i64,i64}; store the pointer.
        // ErrorsCapable is an alloca pointer to {i64,i64}; store the pointer.
        // Range is a pointer to the {i64,i64} range alloca; store the pointer.
        Type::BuiltinArray { .. }
        | Type::BuiltinFixed { .. }
        | Type::Maybe { .. }
        | Type::BuiltinMap { .. }
        | Type::MapEntry { .. }
        | Type::ErrorsCapable { .. }
        | Type::Range { .. } => {
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
        }
        _ => {
            cg.builder
                .build_store(slot, val)
                .map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

/// Store a value into a STRUCT FIELD pointer.
///
/// Shape fields use `llvm_field_type` layout (number = i128 inline; everything else
/// by value or by pointer). This differs from the variable-slot layout where number
/// is stored as i128 in the slot but "value" is a ptr-to-i128.
fn store_field<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    val: BasicValueEnum<'ctx>,
    ty: &Type,
    field_ptr: PointerValue<'ctx>,
) -> Result<(), String> {
    match ty {
        Type::Number { precision } if *precision <= 34 => {
            // N ≤ 34: field stores i128 bits inline; val is ptr-to-i128.
            let bits = cg
                .builder
                .build_load(cg.i128(), val.into_pointer_value(), "dec_field_bits")
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(field_ptr, bits)
                .map_err(|e| format!("{e}"))?;
        }
        Type::Number { .. } => {
            // N > 34: field stores a pointer to the decimal string; val is already a ptr.
            cg.builder
                .build_store(field_ptr, val)
                .map_err(|e| format!("{e}"))?;
        }
        _ => {
            cg.builder
                .build_store(field_ptr, val)
                .map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

fn load<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    slot: PointerValue<'ctx>,
    ty: &Type,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    match ty {
        Type::Number { precision } if *precision <= 34 => {
            // Hardware decimal128: load i128 bits from slot, copy into fresh alloca, return ptr.
            let bits = cg
                .builder
                .build_load(cg.i128(), slot, "dec_ld")
                .map_err(|e| format!("{e}"))?;
            let tmp = cg
                .builder
                .build_alloca(cg.i128(), name)
                .map_err(|e| format!("{e}"))?;
            cg.builder
                .build_store(tmp, bits)
                .map_err(|e| format!("{e}"))?;
            Ok(tmp.into())
        }
        // M8 P6: bignum (N > 34) — slot stores a pointer to the decimal string.
        Type::Number { .. } => cg
            .builder
            .build_load(cg.ptr(), slot, name)
            .map_err(|e| format!("{e}")),
        // Shapes and dynamic values: the slot holds a pointer to the struct data.
        // Load that pointer and return it — the caller gets a ptr to the struct.
        Type::Shape { .. } | Type::Dynamic { .. } => {
            let lt = cg.ptr();
            cg.builder
                .build_load(lt, slot, name)
                .map_err(|e| format!("{e}"))
        }
        // BuiltinArray: slot stores the *mut YnzArray pointer — load and return it.
        // BuiltinFixed: slot stores a pointer to the [N x i64] alloca — load and return.
        // Maybe: slot stores a pointer to the {i64,i64} alloca — load and return.
        // Union: slot stores a pointer to the tagged-struct alloca — load and return.
        // ErrorsCapable: slot stores a pointer to the {i64,i64} result alloca — load and return.
        // Range: slot stores a pointer to the {i64 start, i64 end} range alloca — load and return.
        Type::BuiltinArray { .. }
        | Type::BuiltinFixed { .. }
        | Type::Maybe { .. }
        | Type::BuiltinMap { .. }
        | Type::MapEntry { .. }
        | Type::Union { .. }
        | Type::ErrorsCapable { .. }
        | Type::Range { .. } => cg
            .builder
            .build_load(cg.ptr(), slot, name)
            .map_err(|e| format!("{e}")),
        ty => {
            let lt = cg
                .llvm_type_for(ty)
                .ok_or_else(|| format!("load: unknown type {:?}", ty))?;
            cg.builder
                .build_load(lt, slot, name)
                .map_err(|e| format!("{e}"))
        }
    }
}

fn build_string_global<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    s: &str,
    name: &str,
) -> GlobalValue<'ctx> {
    let i8t = ctx.i8_type();
    let mut bytes: Vec<u8> = s.bytes().collect();
    bytes.push(0);
    let arr_ty = i8t.array_type(bytes.len() as u32);
    let arr = i8t.const_array(
        &bytes
            .iter()
            .map(|&b| i8t.const_int(b as u64, false))
            .collect::<Vec<_>>(),
    );
    let g = module.add_global(arr_ty, Some(AddressSpace::default()), name);
    g.set_initializer(&arr);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Private);
    g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
    g
}

fn build_decimal_global<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    bits: u128,
    name: &str,
) -> GlobalValue<'ctx> {
    let i128t = ctx.i128_type();
    let val = i128t.const_int_arbitrary_precision(&[
        (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
        (bits >> 64) as u64,
    ]);
    let g = module.add_global(i128t, Some(AddressSpace::default()), name);
    g.set_initializer(&val);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Private);
    g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
    g
}
