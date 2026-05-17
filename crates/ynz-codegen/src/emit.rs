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
    types::{BasicMetadataTypeEnum, BasicTypeEnum},
    values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, GlobalValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use ynz_ast::nodes::{
    BinOpKind, Expr, FunctionDecl, Item, MatchPatternKind, OwnershipModifier, Stmt, UnaryOpKind,
};
use ynz_numerics; // parse(s: &str) -> Option<u128>
use ynz_typeck::{type_attached_const_type, ShapeTable, SignatureTable, Type, TypedModule};

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

/// Emit a relocatable object file for an M4 program.
pub fn emit_artifact(
    source_path: &str,
    typed_module: &TypedModule,
    shape_table: &ShapeTable,
    _sig_table: &SignatureTable,
    target_triple: Option<&str>,
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
            &triple, "generic", "",
            OptimizationLevel::None,
            RelocMode::Default,
            CodeModel::Default,
        )
        .ok_or_else(|| "LLVM: failed to create target machine".to_string())?;
    module.set_data_layout(&machine.get_target_data().get_data_layout());

    build_module(&context, &module, typed_module, shape_table)?;

    module.verify()
        .map_err(|e| format!("LLVM module verify failed: {}", e.to_string()))?;

    let ir_text = module.print_to_string().to_string();
    let obj_buf = machine
        .write_to_memory_buffer(&module, FileType::Object)
        .map_err(|e| format!("LLVM: failed to write object: {}", e.to_string()))?;
    let object_bytes = obj_buf.as_slice().to_vec();
    let hash = sha256(&object_bytes);

    Ok(CompiledArtifact { object_bytes, ir_text, sha256: hash })
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
}

fn build_module<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    typed: &'g TypedModule,
    shape_table: &'g ShapeTable,
) -> Result<(), String> {
    let rt = RuntimeDecls::declare(ctx, module);

    let zero_bits = ynz_numerics::parse("0").expect("decimal zero parse");
    let globals = ModuleGlobals {
        str_true:      build_string_global(ctx, module, "true",                       ".str.true"),
        str_false:     build_string_global(ctx, module, "false",                      ".str.false"),
        dec_zero:      build_decimal_global(ctx, module, zero_bits,                   ".dec.zero"),
        panic_int_add: build_string_global(ctx, module, "int overflow in '+'",        ".panic.iadd"),
        panic_int_sub: build_string_global(ctx, module, "int overflow in '-'",        ".panic.isub"),
        panic_int_mul: build_string_global(ctx, module, "int overflow in '*'",        ".panic.imul"),
        panic_int_div: build_string_global(ctx, module, "division by zero (int)",     ".panic.idiv"),
        panic_int_rem: build_string_global(ctx, module, "remainder by zero (int)",    ".panic.irem"),
        panic_dec_div: build_string_global(ctx, module, "division by zero (number)",  ".panic.ddiv"),
        panic_dec_rem: build_string_global(ctx, module, "remainder by zero (number)", ".panic.drem"),
    };

    // Pass 0 — emit LLVM struct types for all user-defined shapes.
    let shape_types = emit_shape_types(ctx, shape_table);

    // Pass 1 — forward-declare every function so vtables and bodies can reference them.
    for item in &typed.module.items {
        match item {
            Item::Function(f) => declare_function(ctx, module, f, shape_table)?,
            Item::ShapeDecl(_) => {}
        }
    }

    // Pass 1.5 — emit vtable globals for `dynamic Foo` dispatch.
    // Currently unused in lowering (dynamic call-site coercion lands after P5 catch-up).
    let _vtables = emit_vtable_globals(module, shape_table);

    // Pass 2 — emit bodies.
    for item in &typed.module.items {
        match item {
            Item::Function(f) => lower_function(ctx, module, &rt, &globals, typed, f, shape_table, &shape_types)?,
            Item::ShapeDecl(_) => {}
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
    f.params.iter().map(|p| {
        match &p.ty {
            ynz_ast::nodes::Type::Int   => ctx.i64_type().into(),
            ynz_ast::nodes::Type::Float => ctx.f64_type().into(),
            ynz_ast::nodes::Type::Bool  => ctx.bool_type().into(),
            // Named shape type or self — all shapes pass as ptr
            ynz_ast::nodes::Type::Named(n, _) if shape_table.contains(n) || n == "self" => ptr.into(),
            ynz_ast::nodes::Type::SelfType { .. } => ptr.into(),
            ynz_ast::nodes::Type::Dynamic { .. } => ptr.into(),
            // String, Number, and everything else: ptr
            _ => ptr.into(),
        }
    }).collect()
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
    let fn_ty = if f.name == "main" {
        ctx.i32_type().fn_type(&params, false)
    } else {
        match &f.return_type {
            ynz_ast::nodes::Type::Nothing => ctx.void_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Int     => ctx.i64_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Float   => ctx.f64_type().fn_type(&params, false),
            ynz_ast::nodes::Type::Bool    => ctx.bool_type().fn_type(&params, false),
            _                             => ctx.ptr_type(AddressSpace::default()).fn_type(&params, false),
        }
    };
    let fn_val = module.add_function(&f.name, fn_ty, None);

    // Emit LLVM ownership attributes on pointer-typed parameters.
    let readonly_kind = Attribute::get_named_enum_kind_id("readonly");
    let noalias_kind  = Attribute::get_named_enum_kind_id("noalias");

    for (i, param) in f.params.iter().enumerate() {
        if !is_ptr_param(&param.ty, shape_table) { continue; }
        let ownership = param.ownership.as_ref().unwrap_or(&OwnershipModifier::Share);
        match ownership {
            OwnershipModifier::Share => {
                fn_val.add_attribute(AttributeLoc::Param(i as u32), ctx.create_enum_attribute(readonly_kind, 0));
                fn_val.add_attribute(AttributeLoc::Param(i as u32), ctx.create_enum_attribute(noalias_kind, 0));
            }
            OwnershipModifier::Lend => {
                fn_val.add_attribute(AttributeLoc::Param(i as u32), ctx.create_enum_attribute(noalias_kind, 0));
            }
            OwnershipModifier::Give => {}
        }
    }

    Ok(())
}

/// True when the AST type will be passed as a pointer in LLVM (not a scalar value).
fn is_ptr_param(ty: &ynz_ast::nodes::Type, shape_table: &ShapeTable) -> bool {
    match ty {
        ynz_ast::nodes::Type::Int | ynz_ast::nodes::Type::Float | ynz_ast::nodes::Type::Bool => false,
        ynz_ast::nodes::Type::Named(n, _) if !shape_table.contains(n) && n != "self" && n != "string" => false,
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
}

impl<'ctx, 'g> Cg<'ctx, 'g> {
    fn i64(&self) -> inkwell::types::IntType<'ctx>   { self.ctx.i64_type() }
    fn i128(&self) -> inkwell::types::IntType<'ctx>  { self.ctx.i128_type() }
    fn i32(&self) -> inkwell::types::IntType<'ctx>   { self.ctx.i32_type() }
    fn i8(&self) -> inkwell::types::IntType<'ctx>    { self.ctx.i8_type() }
    fn f64(&self) -> inkwell::types::FloatType<'ctx> { self.ctx.f64_type() }
    fn bool(&self) -> inkwell::types::IntType<'ctx>  { self.ctx.bool_type() }
    fn ptr(&self) -> inkwell::types::PointerType<'ctx> { self.ctx.ptr_type(AddressSpace::default()) }

    fn expr_type<'a>(&'a self, expr: &Expr) -> &'a Type {
        let key = (expr.span().start, expr.span().end);
        self.typed.expr_types.get(&key).unwrap_or(&Type::Error)
    }

    fn llvm_type_for(&self, ty: &Type) -> Option<BasicTypeEnum<'ctx>> {
        match ty {
            Type::Int            => Some(self.i64().into()),
            Type::Float          => Some(self.f64().into()),
            Type::Bool           => Some(self.bool().into()),
            Type::Number { .. }  => Some(self.i128().into()),
            Type::String         => Some(self.ptr().into()),
            // Shape and dynamic values are always passed/stored as opaque pointers.
            Type::Shape { .. }   => Some(self.ptr().into()),
            Type::Dynamic { .. } => Some(self.ptr().into()),
            _                    => None,
        }
    }


    fn alloca(&self, ty: &Type, name: &str) -> Result<PointerValue<'ctx>, String> {
        let llvm_ty = self.llvm_type_for(ty)
            .ok_or_else(|| format!("cannot alloca for type {:?}", ty))?;
        self.builder.build_alloca(llvm_ty, name).map_err(|e| format!("{e}"))
    }

    fn append_block(&self, name: &str) -> BasicBlock<'ctx> {
        self.ctx.append_basic_block(self.current_fn, name)
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
) -> Result<(), String> {
    let fn_val = module.get_function(&f.name)
        .ok_or_else(|| format!("function `{}` was not forward-declared", f.name))?;

    let ret_ty = ast_type_to_typeck_type(&f.return_type, shape_table);
    let is_main = f.name == "main";
    let ret_is_nothing = matches!(ret_ty, Type::Nothing);

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
    };

    let entry = ctx.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);

    // Materialize each parameter as an alloca so the "every name = alloca" invariant holds.
    for (i, param) in f.params.iter().enumerate() {
        let llvm_param = fn_val.get_nth_param(i as u32)
            .ok_or_else(|| format!("missing LLVM param {} for `{}`", i, f.name))?;
        let param_ty = ast_type_to_typeck_type(&param.ty, shape_table);
        materialize_param(&mut cg, &param.name, llvm_param, &param_ty)?;
    }

    for stmt in &f.body.stmts {
        if is_block_terminated(&cg) { break; }
        lower_stmt(&mut cg, stmt)?;
    }

    // Implicit terminator if the current block has no terminator yet.
    //
    // - main: always ret i32 0 (C ABI entry point).
    // - nothing-returning functions: ret void (legitimate fall-off-the-end).
    // - non-nothing functions: unreachable — either the typeck confirmed all
    //   paths return (so this block is dead code after an exhaustive match) or
    //   it's a typeck bug. Either way, ret void would fail LLVM verify.
    if !is_block_terminated(&cg) {
        if is_main {
            cg.builder.build_return(Some(&ctx.i32_type().const_int(0, false)))
                .map_err(|e| format!("implicit main ret: {e}"))?;
        } else if ret_is_nothing {
            cg.builder.build_return(None)
                .map_err(|e| format!("implicit void ret: {e}"))?;
        } else {
            cg.builder.build_unreachable()
                .map_err(|e| format!("implicit unreachable: {e}"))?;
        }
    }
    Ok(())
}

/// Map an AST type annotation to the typeck `Type` for use in codegen decisions.
fn ast_type_to_typeck_type(ast_ty: &ynz_ast::nodes::Type, shape_table: &ShapeTable) -> Type {
    match ast_ty {
        ynz_ast::nodes::Type::Nothing          => Type::Nothing,
        ynz_ast::nodes::Type::Int              => Type::Int,
        ynz_ast::nodes::Type::Float            => Type::Float,
        ynz_ast::nodes::Type::Bool             => Type::Bool,
        ynz_ast::nodes::Type::Number { .. }    => Type::Number { precision: 34 },
        ynz_ast::nodes::Type::Named(n, _) if n == "string" => Type::String,
        ynz_ast::nodes::Type::Named(n, _) if shape_table.contains(n) => Type::Shape { name: n.clone() },
        ynz_ast::nodes::Type::Dynamic { contract, .. } => Type::Dynamic { contract: contract.clone() },
        // `self` parameter and SelfType: the typeck stores the concrete Shape type in
        // expr_types; for parameter materialization we just need to know it's a ptr.
        ynz_ast::nodes::Type::Named(n, _) if n == "self" => Type::Shape { name: String::new() },
        ynz_ast::nodes::Type::SelfType { .. } => Type::Shape { name: String::new() },
        _                                      => Type::Error,
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
        // Shape/Dynamic: incoming param is a ptr; store the ptr in a ptr-slot.
        Type::Shape { .. } | Type::Dynamic { .. } => {
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
        Stmt::Expr(expr) => { lower_expr(cg, expr)?; }

        Stmt::Let { name, value, .. } => {
            let ty = cg.expr_type(value).clone();
            let val = lower_expr(cg, value)?;
            let slot = cg.alloca(&ty, name)?;
            store(cg, val, &ty, slot)?;
            cg.locals.insert(name.clone(), slot);
        }

        Stmt::Assign { target, value, .. } => {
            let slot = *cg.locals.get(target.as_str())
                .ok_or_else(|| format!("undefined `{target}` in codegen"))?;
            let ty = cg.expr_type(value).clone();
            let val = lower_expr(cg, value)?;
            store(cg, val, &ty, slot)?;
        }

        Stmt::If { cond, body, .. } => {
            lower_stmt_if(cg, cond, body)?;
        }

        Stmt::Match { scrutinee, arms, else_arm, .. } => {
            lower_stmt_match(cg, scrutinee, arms, else_arm.as_ref())?;
        }

        Stmt::While { cond, body, .. } => {
            lower_stmt_while(cg, cond, body)?;
        }

        Stmt::For { var, iter, body, .. } => {
            lower_stmt_for(cg, var, iter, body)?;
        }

        Stmt::Return { value, .. } => {
            lower_stmt_return(cg, value.as_ref())?;
        }

        Stmt::FieldAssign { target, value, .. } => {
            lower_stmt_field_assign(cg, target, value)?;
        }
    }
    Ok(())
}

fn lower_stmt_if<'ctx>(cg: &mut Cg<'ctx, '_>, cond: &Expr, body: &ynz_ast::nodes::Block) -> Result<(), String> {
    let cond_val = lower_expr(cg, cond)?.into_int_value();
    let then_bb = cg.append_block("if_then");
    let merge_bb = cg.append_block("if_merge");

    cg.builder.build_conditional_branch(cond_val, then_bb, merge_bb)
        .map_err(|e| format!("if branch: {e}"))?;

    cg.builder.position_at_end(then_bb);
    for stmt in &body.stmts {
        if is_block_terminated(cg) { break; }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        cg.builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{e}"))?;
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
    let scrutinee_ty = cg.expr_type(scrutinee).clone();
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
            // IsType/Variant: M6 deferral — parser emitted the diagnostic; typeck
            // would have rejected the program before reaching codegen.
            MatchPatternKind::IsType(_) | MatchPatternKind::Variant(_) => {
                return Err("codegen: M6 pattern kind reached codegen (should be rejected by typeck)".to_string());
            }
        };

        cg.builder.build_conditional_branch(pat_cond, arm_body_bb, next_check_bb)
            .map_err(|e| format!("match branch: {e}"))?;

        cg.builder.position_at_end(arm_body_bb);
        for stmt in &arm.body.stmts {
            if is_block_terminated(cg) { break; }
            lower_stmt(cg, stmt)?;
        }
        if !is_block_terminated(cg) {
            cg.builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{e}"))?;
        }

        cg.builder.position_at_end(next_check_bb);
    }

    // Emit else body (current position is final_fallthrough_bb or merge_bb).
    if let Some(else_body) = else_arm {
        for stmt in &else_body.stmts {
            if is_block_terminated(cg) { break; }
            lower_stmt(cg, stmt)?;
        }
        if !is_block_terminated(cg) {
            cg.builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{e}"))?;
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
        Type::Int | Type::Bool => cg.builder
            .build_int_compare(IntPredicate::EQ, scrutinee_val.into_int_value(), pattern_val.into_int_value(), "match_eq")
            .map_err(|e| format!("{e}")),
        Type::Float => cg.builder
            .build_float_compare(inkwell::FloatPredicate::OEQ, scrutinee_val.into_float_value(), pattern_val.into_float_value(), "fmatch")
            .map_err(|e| format!("{e}")),
        Type::String => {
            let call = cg.builder.build_call(
                cg.rt.string_eq,
                &[scrutinee_val.into(), pattern_val.into()],
                "str_eq",
            ).map_err(|e| format!("{e}"))?;
            let result = call.try_as_basic_value().basic().ok_or("string_eq void")?.into_int_value();
            cg.builder.build_int_compare(IntPredicate::NE, result, cg.i32().const_int(0, false), "str_eq_bool")
                .map_err(|e| format!("{e}"))
        }
        Type::Number { .. } => {
            let c = cg.builder.build_call(cg.rt.decimal_compare, &[scrutinee_val.into(), pattern_val.into()], "dcmp")
                .map_err(|e| format!("{e}"))?;
            let ci = c.try_as_basic_value().basic().ok_or("dcmp void")?.into_int_value();
            cg.builder.build_int_compare(IntPredicate::EQ, ci, cg.i32().const_int(0, false), "deq")
                .map_err(|e| format!("{e}"))
        }
        other => Err(format!("codegen: match on unsupported type {:?}", other)),
    }
}

fn lower_stmt_while<'ctx>(cg: &mut Cg<'ctx, '_>, cond: &Expr, body: &ynz_ast::nodes::Block) -> Result<(), String> {
    let header_bb = cg.append_block("while_header");
    let body_bb   = cg.append_block("while_body");
    let exit_bb   = cg.append_block("while_exit");

    cg.builder.build_unconditional_branch(header_bb).map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(header_bb);
    let cond_val = lower_expr(cg, cond)?.into_int_value();
    cg.builder.build_conditional_branch(cond_val, body_bb, exit_bb).map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(body_bb);
    for stmt in &body.stmts {
        if is_block_terminated(cg) { break; }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        cg.builder.build_unconditional_branch(header_bb).map_err(|e| format!("{e}"))?;
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
    let (start_val, end_val) = extract_range_bounds(cg, iter)?;

    let counter_slot = cg.builder.build_alloca(cg.i64(), "for_ctr").map_err(|e| format!("{e}"))?;
    let end_slot     = cg.builder.build_alloca(cg.i64(), "for_end").map_err(|e| format!("{e}"))?;
    cg.builder.build_store(counter_slot, start_val).map_err(|e| format!("{e}"))?;
    cg.builder.build_store(end_slot, end_val).map_err(|e| format!("{e}"))?;

    // Loop variable alloca — loop var is typed as int.
    let var_slot = cg.builder.build_alloca(cg.i64(), var).map_err(|e| format!("{e}"))?;
    cg.locals.insert(var.to_string(), var_slot);

    let header_bb = cg.append_block("for_header");
    let body_bb   = cg.append_block("for_body");
    let exit_bb   = cg.append_block("for_exit");

    cg.builder.build_unconditional_branch(header_bb).map_err(|e| format!("{e}"))?;

    // Header: check counter < end.
    cg.builder.position_at_end(header_bb);
    let ctr = cg.builder.build_load(cg.i64(), counter_slot, "ctr").map_err(|e| format!("{e}"))?.into_int_value();
    let end = cg.builder.build_load(cg.i64(), end_slot, "end").map_err(|e| format!("{e}"))?.into_int_value();
    let in_range = cg.builder.build_int_compare(IntPredicate::SLT, ctr, end, "for_cond").map_err(|e| format!("{e}"))?;
    cg.builder.build_conditional_branch(in_range, body_bb, exit_bb).map_err(|e| format!("{e}"))?;

    // Body: bind loop var, emit stmts, increment, back-edge.
    cg.builder.position_at_end(body_bb);
    let ctr_bind = cg.builder.build_load(cg.i64(), counter_slot, "ctr_bind").map_err(|e| format!("{e}"))?;
    cg.builder.build_store(var_slot, ctr_bind).map_err(|e| format!("{e}"))?;

    for stmt in &body.stmts {
        if is_block_terminated(cg) { break; }
        lower_stmt(cg, stmt)?;
    }
    if !is_block_terminated(cg) {
        let ctr_cur = cg.builder.build_load(cg.i64(), counter_slot, "ctr_cur").map_err(|e| format!("{e}"))?.into_int_value();
        let one = cg.i64().const_int(1, false);
        let ctr_next = cg.builder.build_int_add(ctr_cur, one, "ctr_next").map_err(|e| format!("{e}"))?;
        cg.builder.build_store(counter_slot, ctr_next).map_err(|e| format!("{e}"))?;
        cg.builder.build_unconditional_branch(header_bb).map_err(|e| format!("{e}"))?;
    }

    cg.builder.position_at_end(exit_bb);
    cg.locals.remove(var);
    Ok(())
}

fn lower_stmt_return<'ctx>(cg: &mut Cg<'ctx, '_>, value: Option<&Expr>) -> Result<(), String> {
    if cg.is_main {
        cg.builder.build_return(Some(&cg.i32().const_int(0, false)))
            .map_err(|e| format!("main ret: {e}"))?;
        return Ok(());
    }
    match value {
        None => {
            cg.builder.build_return(None).map_err(|e| format!("void ret: {e}"))?;
        }
        Some(expr) => {
            let val = lower_expr(cg, expr)?;
            cg.builder.build_return(Some(&val)).map_err(|e| format!("ret: {e}"))?;
        }
    }
    Ok(())
}

/// Extract the start and end `i64` values from a `range(end)` or `range(start, end)` call.
fn extract_range_bounds<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    iter: &Expr,
) -> Result<(inkwell::values::IntValue<'ctx>, inkwell::values::IntValue<'ctx>), String> {
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
            let end   = lower_expr(cg, &call.args[1])?.into_int_value();
            Ok((start, end))
        }
        n => Err(format!("range takes 1 or 2 args, got {n}")),
    }
}


fn lower_expr<'ctx>(cg: &mut Cg<'ctx, '_>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, String> {
    match expr {
        Expr::IntLit(n, _) => Ok(cg.i64().const_int(*n as u64, true).into()),

        Expr::NumberLit(s, _) => {
            let bits: u128 = ynz_numerics::parse(s)
                .ok_or_else(|| format!("bad decimal literal `{s}`"))?;
            let slot = cg.builder.build_alloca(cg.i128(), "dec_lit").map_err(|e| format!("{e}"))?;
            let const_val = cg.i128().const_int_arbitrary_precision(&[
                (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
                (bits >> 64) as u64,
            ]);
            cg.builder.build_store(slot, const_val).map_err(|e| format!("{e}"))?;
            Ok(slot.into())
        }

        Expr::BoolLit(b, _) => Ok(cg.bool().const_int(*b as u64, false).into()),

        Expr::StringLit(bytes, _) => {
            let mut null = bytes.clone();
            null.push(0);
            let i8t = cg.i8();
            let arr_ty = i8t.array_type(null.len() as u32);
            let arr = i8t.const_array(&null.iter().map(|&b| i8t.const_int(b as u64, false)).collect::<Vec<_>>());
            let g = cg.module.add_global(arr_ty, Some(AddressSpace::default()), "str");
            g.set_initializer(&arr);
            g.set_constant(true);
            g.set_linkage(inkwell::module::Linkage::Private);
            g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
            Ok(g.as_pointer_value().into())
        }

        Expr::Ident(name, _) => {
            let slot = *cg.locals.get(name.as_str())
                .ok_or_else(|| format!("undefined `{name}` in codegen"))?;
            let ty = cg.expr_type(expr).clone();
            load(cg, slot, &ty, name)
        }

        Expr::BinOp { op, lhs, rhs, .. } => {
            let lhs_ty = cg.expr_type(lhs).clone();
            let rhs_ty = cg.expr_type(rhs).clone();
            lower_binop(cg, op, lhs, rhs, &lhs_ty, &rhs_ty)
        }

        Expr::UnaryOp { op, operand, .. } => {
            let ty = cg.expr_type(operand).clone();
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
                    let ty = cg.expr_type(&call.args[0]).clone();
                    let val = lower_expr(cg, &call.args[0])?;
                    lower_print(cg, val, &ty)?;
                    Ok(cg.i32().const_int(0, false).into())
                }
                "range" => {
                    // range() only appears as the iter in Stmt::For, handled by extract_range_bounds.
                    // Reaching here means it appeared in expression position — a typeck bug.
                    Err("codegen: range() in expression position (should be caught by typeck)".to_string())
                }
                name => {
                    let fn_val = cg.module.get_function(name)
                        .ok_or_else(|| format!("codegen: function `{name}` not found in module"))?;
                    let mut args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
                    for arg in &call.args {
                        let val = lower_expr(cg, arg)?;
                        args.push(val.into());
                    }
                    let call_site = cg.builder.build_call(fn_val, &args, "call")
                        .map_err(|e| format!("call {name}: {e}"))?;
                    match call_site.try_as_basic_value().basic() {
                        Some(val) => Ok(val),
                        None => Ok(cg.i32().const_int(0, false).into()),
                    }
                }
            }
        }

        Expr::MethodCall { receiver, method, args, .. } => {
            let recv_ty = cg.expr_type(receiver).clone();
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

        Expr::FieldAccess { receiver, field, .. } => {
            // M4 P5: type-attached constants (int.max, number.epsilon, etc.)
            if let Expr::Ident(type_name, _) = receiver.as_ref() {
                if type_attached_const_type(type_name, field).is_some() {
                    return emit_type_const(cg, type_name, field);
                }
            }
            lower_field_access(cg, receiver, field)
        }

        Expr::StructLit { fields, .. } => {
            lower_struct_lit(cg, expr, fields)
        }

        Expr::PostfixOp { receiver, op, .. } => {
            lower_postfix_op(cg, receiver, op)
        }

        Expr::SelfValue { .. } => {
            let slot = *cg.locals.get("self")
                .ok_or("codegen: `self` used outside method scope")?;
            let ty = cg.expr_type(expr).clone();
            load(cg, slot, &ty, "self")
        }

        Expr::Error(_) => Err("codegen: error node".to_string()),
    }
}


fn lower_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    op: &BinOpKind,
    lhs_e: &Expr,
    rhs_e: &Expr,
    lhs_ty: &Type,
    _rhs_ty: &Type,
) -> Result<BasicValueEnum<'ctx>, String> {
    use BinOpKind::*;
    if matches!(op, And | Or) {
        return lower_short_circuit(cg, matches!(op, And), lhs_e, rhs_e);
    }
    let lhs = lower_expr(cg, lhs_e)?;
    let rhs = lower_expr(cg, rhs_e)?;

    match (op, lhs_ty) {
        (Add, Type::Int) => int_arith_overflow(cg, lhs.into_int_value(), rhs.into_int_value(), op),
        (Sub, Type::Int) => int_arith_overflow(cg, lhs.into_int_value(), rhs.into_int_value(), op),
        (Mul, Type::Int) => int_arith_overflow(cg, lhs.into_int_value(), rhs.into_int_value(), op),
        (Div, Type::Int) => int_divrem(cg, lhs.into_int_value(), rhs.into_int_value(), false),
        (Rem, Type::Int) => int_divrem(cg, lhs.into_int_value(), rhs.into_int_value(), true),

        (Add, Type::Float) => cg.builder.build_float_add(lhs.into_float_value(), rhs.into_float_value(), "fadd").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Sub, Type::Float) => cg.builder.build_float_sub(lhs.into_float_value(), rhs.into_float_value(), "fsub").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Mul, Type::Float) => cg.builder.build_float_mul(lhs.into_float_value(), rhs.into_float_value(), "fmul").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Div, Type::Float) => cg.builder.build_float_div(lhs.into_float_value(), rhs.into_float_value(), "fdiv").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Rem, Type::Float) => cg.builder.build_float_rem(lhs.into_float_value(), rhs.into_float_value(), "frem").map(|v| v.into()).map_err(|e| format!("{e}")),

        (Add, Type::Number { .. }) => decimal_binop(cg, lhs.into_pointer_value(), rhs.into_pointer_value(), cg.rt.decimal_add, "dadd"),
        (Sub, Type::Number { .. }) => decimal_binop(cg, lhs.into_pointer_value(), rhs.into_pointer_value(), cg.rt.decimal_sub, "dsub"),
        (Mul, Type::Number { .. }) => decimal_binop(cg, lhs.into_pointer_value(), rhs.into_pointer_value(), cg.rt.decimal_mul, "dmul"),
        (Div, Type::Number { .. }) => decimal_div(cg, lhs.into_pointer_value(), rhs.into_pointer_value()),
        (Rem, Type::Number { .. }) => {
            // typeck already rejected this; emit unreachable panic
            cg.builder.build_call(cg.rt.panic_div_by_zero, &[cg.globals.panic_dec_rem.as_pointer_value().into()], "").map_err(|e| format!("{e}"))?;
            cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;
            let out = cg.builder.build_alloca(cg.i128(), "rem_dead").map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }

        (Lt,   Type::Int) => icmp(cg, IntPredicate::SLT, lhs, rhs, "ilt"),
        (LtEq, Type::Int) => icmp(cg, IntPredicate::SLE, lhs, rhs, "ile"),
        (Gt,   Type::Int) => icmp(cg, IntPredicate::SGT, lhs, rhs, "igt"),
        (GtEq, Type::Int) => icmp(cg, IntPredicate::SGE, lhs, rhs, "ige"),
        (EqEq, Type::Int) => icmp(cg, IntPredicate::EQ,  lhs, rhs, "ieq"),
        (NotEq,Type::Int) => icmp(cg, IntPredicate::NE,  lhs, rhs, "ine"),

        (Lt,   Type::Float) => fcmp(cg, inkwell::FloatPredicate::OLT, lhs, rhs, "flt"),
        (LtEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OLE, lhs, rhs, "fle"),
        (Gt,   Type::Float) => fcmp(cg, inkwell::FloatPredicate::OGT, lhs, rhs, "fgt"),
        (GtEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OGE, lhs, rhs, "fge"),
        (EqEq, Type::Float) => fcmp(cg, inkwell::FloatPredicate::OEQ, lhs, rhs, "feq"),
        (NotEq,Type::Float) => fcmp(cg, inkwell::FloatPredicate::ONE, lhs, rhs, "fne"),

        (EqEq|NotEq|Lt|LtEq|Gt|GtEq, Type::Number { .. }) =>
            decimal_compare(cg, lhs.into_pointer_value(), rhs.into_pointer_value(), op),

        (EqEq, Type::Bool) => icmp(cg, IntPredicate::EQ, lhs, rhs, "beq"),
        (NotEq,Type::Bool) => icmp(cg, IntPredicate::NE, lhs, rhs, "bne"),

        (BitAnd, Type::Int) => cg.builder.build_and(lhs.into_int_value(),  rhs.into_int_value(), "band").map(|v| v.into()).map_err(|e| format!("{e}")),
        (BitOr,  Type::Int) => cg.builder.build_or( lhs.into_int_value(),  rhs.into_int_value(), "bor" ).map(|v| v.into()).map_err(|e| format!("{e}")),
        (BitXor, Type::Int) => cg.builder.build_xor(lhs.into_int_value(),  rhs.into_int_value(), "bxor").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Shl,    Type::Int) => cg.builder.build_left_shift(lhs.into_int_value(), rhs.into_int_value(), "shl").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Shr,    Type::Int) => cg.builder.build_right_shift(lhs.into_int_value(), rhs.into_int_value(), true, "shr").map(|v| v.into()).map_err(|e| format!("{e}")),

        _ => Err(format!("codegen: unsupported binop {:?} {:?}", op, lhs_ty)),
    }
}

fn icmp<'ctx>(cg: &mut Cg<'ctx, '_>, pred: IntPredicate, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>, name: &str) -> Result<BasicValueEnum<'ctx>, String> {
    cg.builder.build_int_compare(pred, lhs.into_int_value(), rhs.into_int_value(), name).map(|v| v.into()).map_err(|e| format!("{e}"))
}

fn fcmp<'ctx>(cg: &mut Cg<'ctx, '_>, pred: inkwell::FloatPredicate, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>, name: &str) -> Result<BasicValueEnum<'ctx>, String> {
    cg.builder.build_float_compare(pred, lhs.into_float_value(), rhs.into_float_value(), name).map(|v| v.into()).map_err(|e| format!("{e}"))
}

fn int_arith_overflow<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
    op: &BinOpKind,
) -> Result<BasicValueEnum<'ctx>, String> {
    let (intrinsic, msg_g) = match op {
        BinOpKind::Add => (cg.rt.sadd_overflow, cg.globals.panic_int_add),
        BinOpKind::Sub => (cg.rt.ssub_overflow, cg.globals.panic_int_sub),
        BinOpKind::Mul => (cg.rt.smul_overflow, cg.globals.panic_int_mul),
        _ => unreachable!(),
    };
    let call = cg.builder.build_call(intrinsic, &[lhs.into(), rhs.into()], "ov_res").map_err(|e| format!("{e}"))?;
    let s = call.try_as_basic_value().basic().ok_or("overflow intrinsic void")?.into_struct_value();
    let sum = cg.builder.build_extract_value(s, 0, "sum").map_err(|e| format!("{e}"))?.into_int_value();
    let ov  = cg.builder.build_extract_value(s, 1, "ov").map_err(|e| format!("{e}"))?.into_int_value();

    let ok_bb    = cg.append_block("ov_ok");
    let panic_bb = cg.append_block("ov_panic");
    cg.builder.build_conditional_branch(ov, panic_bb, ok_bb).map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(panic_bb);
    cg.builder.build_call(cg.rt.panic_overflow, &[msg_g.as_pointer_value().into()], "").map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ok_bb);
    Ok(sum.into())
}

fn int_divrem<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: inkwell::values::IntValue<'ctx>,
    rhs: inkwell::values::IntValue<'ctx>,
    is_rem: bool,
) -> Result<BasicValueEnum<'ctx>, String> {
    let zero = cg.i64().const_int(0, false);
    let is_z = cg.builder.build_int_compare(IntPredicate::EQ, rhs, zero, "div_zero").map_err(|e| format!("{e}"))?;
    let ok_bb = cg.append_block("div_ok");
    let pbb   = cg.append_block("div_panic");
    cg.builder.build_conditional_branch(is_z, pbb, ok_bb).map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(pbb);
    let msg = if is_rem { cg.globals.panic_int_rem } else { cg.globals.panic_int_div };
    cg.builder.build_call(cg.rt.panic_div_by_zero, &[msg.as_pointer_value().into()], "").map_err(|e| format!("{e}"))?;
    cg.builder.build_unreachable().map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(ok_bb);
    if is_rem {
        cg.builder.build_int_signed_rem(lhs, rhs, "srem").map(|v| v.into()).map_err(|e| format!("{e}"))
    } else {
        cg.builder.build_int_signed_div(lhs, rhs, "sdiv").map(|v| v.into()).map_err(|e| format!("{e}"))
    }
}

fn decimal_binop<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
    rt_fn: FunctionValue<'ctx>,
    name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let out = cg.builder.build_alloca(cg.i128(), name).map_err(|e| format!("{e}"))?;
    cg.builder.build_call(rt_fn, &[lhs.into(), rhs.into(), out.into()], "").map_err(|e| format!("{e}"))?;
    Ok(out.into())
}

fn decimal_div<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    lhs: PointerValue<'ctx>,
    rhs: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, String> {
    let zero = cg.globals.dec_zero.as_pointer_value();
    let cmp_call = cg.builder.build_call(cg.rt.decimal_compare, &[rhs.into(), zero.into()], "ddiv_cmp").map_err(|e| format!("{e}"))?;
    let cmp_i32 = cmp_call.try_as_basic_value().basic().ok_or("decimal_compare void")?.into_int_value();
    let is_z = cg.builder.build_int_compare(IntPredicate::EQ, cmp_i32, cg.i32().const_int(0, false), "ddiv_zero").map_err(|e| format!("{e}"))?;
    let ok_bb = cg.append_block("ddiv_ok");
    let pbb   = cg.append_block("ddiv_panic");
    cg.builder.build_conditional_branch(is_z, pbb, ok_bb).map_err(|e| format!("{e}"))?;
    cg.builder.position_at_end(pbb);
    cg.builder.build_call(cg.rt.panic_div_by_zero, &[cg.globals.panic_dec_div.as_pointer_value().into()], "").map_err(|e| format!("{e}"))?;
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
    let c = cg.builder.build_call(cg.rt.decimal_compare, &[lhs.into(), rhs.into()], "dcmp").map_err(|e| format!("{e}"))?;
    let ci = c.try_as_basic_value().basic().ok_or("cmp void")?.into_int_value();
    let z = cg.i32().const_int(0, false);
    let pred = match op {
        BinOpKind::Lt    => IntPredicate::SLT,
        BinOpKind::LtEq  => IntPredicate::SLE,
        BinOpKind::Gt    => IntPredicate::SGT,
        BinOpKind::GtEq  => IntPredicate::SGE,
        BinOpKind::EqEq  => IntPredicate::EQ,
        BinOpKind::NotEq => IntPredicate::NE,
        _ => unreachable!(),
    };
    cg.builder.build_int_compare(pred, ci, z, "dcmp_b").map(|v| v.into()).map_err(|e| format!("{e}"))
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

    let rhs_bb:   BasicBlock<'ctx> = cg.append_block(if is_and { "and_rhs"   } else { "or_rhs"   });
    let short_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_short" } else { "or_short" });
    let merge_bb: BasicBlock<'ctx> = cg.append_block(if is_and { "and_merge" } else { "or_merge" });

    let _ = lhs_bb;
    if is_and {
        cg.builder.build_conditional_branch(lhs, rhs_bb, short_bb)
    } else {
        cg.builder.build_conditional_branch(lhs, short_bb, rhs_bb)
    }.map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(rhs_bb);
    let rhs = lower_expr(cg, rhs_e)?.into_int_value();
    let rhs_bb_end = cg.builder.get_insert_block().ok_or("no rhs end block")?;
    cg.builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(short_bb);
    cg.builder.build_unconditional_branch(merge_bb).map_err(|e| format!("{e}"))?;

    cg.builder.position_at_end(merge_bb);
    let phi = cg.builder.build_phi(bool_ty, if is_and { "and_r" } else { "or_r" }).map_err(|e| format!("{e}"))?;
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
        (UnaryOpKind::Neg, Type::Int)   => cg.builder.build_int_neg(val.into_int_value(), "neg").map(|v| v.into()).map_err(|e| format!("{e}")),
        (UnaryOpKind::Neg, Type::Float) => cg.builder.build_float_neg(val.into_float_value(), "fneg").map(|v| v.into()).map_err(|e| format!("{e}")),
        (UnaryOpKind::Neg, Type::Number { .. }) => {
            let zero = cg.globals.dec_zero.as_pointer_value();
            decimal_binop(cg, zero, val.into_pointer_value(), cg.rt.decimal_sub, "dec_neg")
        }
        (UnaryOpKind::Not, Type::Bool)  => {
            cg.builder.build_xor(val.into_int_value(), cg.bool().const_int(1, false), "not").map(|v| v.into()).map_err(|e| format!("{e}"))
        }
        (UnaryOpKind::BitNot, Type::Int) => cg.builder.build_not(val.into_int_value(), "bitnot").map(|v| v.into()).map_err(|e| format!("{e}")),
        _ => Err(format!("codegen: unsupported unary {:?} {:?}", op, ty)),
    }
}


fn lower_print<'ctx>(cg: &mut Cg<'ctx, '_>, val: BasicValueEnum<'ctx>, ty: &Type) -> Result<(), String> {
    let p = to_c_string(cg, val, ty)?;
    cg.builder.build_call(cg.rt.puts, &[p.into()], "puts").map_err(|e| format!("{e}"))?;
    Ok(())
}

fn to_c_string<'ctx>(cg: &mut Cg<'ctx, '_>, val: BasicValueEnum<'ctx>, ty: &Type) -> Result<PointerValue<'ctx>, String> {
    match ty {
        Type::String => Ok(val.into_pointer_value()),

        Type::Bool => cg.builder
            .build_select(val.into_int_value(), cg.globals.str_true.as_pointer_value(), cg.globals.str_false.as_pointer_value(), "bstr")
            .map(|v| v.into_pointer_value())
            .map_err(|e| format!("{e}")),

        // Runtime format shims return a ptr into a thread-local static buffer.
        Type::Int => {
            let c = cg.builder.build_call(cg.rt.int_to_string, &[val.into()], "int_str").map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value().basic().ok_or("int_to_string returned void")?.into_pointer_value())
        }
        Type::Float => {
            let c = cg.builder.build_call(cg.rt.float_to_string, &[val.into()], "flt_str").map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value().basic().ok_or("float_to_string returned void")?.into_pointer_value())
        }
        Type::Number { .. } => {
            let c = cg.builder.build_call(cg.rt.decimal_to_string, &[val.into()], "dec_str").map_err(|e| format!("{e}"))?;
            Ok(c.try_as_basic_value().basic().ok_or("decimal_to_string returned void")?.into_pointer_value())
        }
        _ => Err(format!("codegen: cannot convert {:?} to string", ty)),
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
    let fn_val = cg.module.get_function(method)
        .ok_or_else(|| format!("codegen UFCS: function `{}` not found in module", method))?;

    let mut args: Vec<BasicMetadataValueEnum<'ctx>> = vec![recv_val.into()];
    for arg in extra_args {
        let arg_val = lower_expr(cg, arg)?;
        // If the function param expects dynamic but arg is a concrete shape, coerce.
        args.push(arg_val.into());
    }

    let call = cg.builder.build_call(fn_val, &args, "ufcs")
        .map_err(|e| format!("UFCS call `{method}`: {e}"))?;
    match call.try_as_basic_value().basic() {
        Some(v) => Ok(v),
        None    => Ok(cg.i32().const_int(0, false).into()),
    }
}

/// Dynamic dispatch: `d.method(args)` via vtable when receiver is `dynamic Foo`.
/// Field access: lower `receiver.field` to the field's value.
fn lower_field_access<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    field_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    let (field_ptr, field_ty) = field_gep(cg, receiver, field_name)?;
    load(cg, field_ptr, &field_ty, field_name)
}

/// Get a GEP pointer to a field inside a shape value (for reads AND writes).
fn field_gep<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    receiver: &Expr,
    field_name: &str,
) -> Result<(PointerValue<'ctx>, Type), String> {
    let recv_ty = cg.expr_type(receiver).clone();
    let shape_name = match &recv_ty {
        Type::Shape { name } => name.clone(),
        _ => return Err(format!("field_gep: receiver is not a Shape, got {:?}", recv_ty)),
    };

    let shape_def = cg.shape_table.get(&shape_name)
        .ok_or_else(|| format!("field_gep: shape `{}` not in table", shape_name))?;
    let field_idx = shape_def.fields.iter().position(|f| f.name == field_name)
        .ok_or_else(|| format!("field_gep: field `{}` not found in shape `{}`", field_name, shape_name))?;
    let field_ty = shape_def.fields[field_idx].ty.clone();

    let struct_ty = cg.shape_types.get(&shape_name)
        .ok_or_else(|| format!("field_gep: LLVM type for shape `{}` not found", shape_name))?;

    // Lower the receiver to get a pointer to the struct.
    let recv_ptr = lower_expr(cg, receiver)?;

    let gep = cg.builder.build_struct_gep(struct_ty, recv_ptr.into_pointer_value(), field_idx as u32, field_name)
        .map_err(|e| format!("GEP field `{}`: {e}", field_name))?;

    Ok((gep, field_ty))
}

/// Lower a struct literal to a stack-allocated value; return pointer to it.
fn lower_struct_lit<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    expr: &Expr,
    fields: &[ynz_ast::nodes::StructLitField],
) -> Result<BasicValueEnum<'ctx>, String> {
    let ty = cg.expr_type(expr).clone();
    let Type::Shape { name: shape_name } = &ty else {
        return Err(format!("struct literal with non-shape type {:?}", ty));
    };
    let shape_name = shape_name.clone();

    let struct_ty = cg.shape_types.get(&shape_name)
        .ok_or_else(|| format!("struct_lit: LLVM type for `{}` not found", shape_name))?;

    // Allocate the struct on the stack.
    let slot = cg.builder.build_alloca(struct_ty, &shape_name)
        .map_err(|e| format!("alloca {}: {e}", shape_name))?;

    // Zero-initialize — covers hidden fields that aren't provided.
    let zero = struct_ty.const_zero();
    cg.builder.build_store(slot, zero)
        .map_err(|e| format!("zero-init {}: {e}", shape_name))?;

    let shape_def = cg.shape_table.get(&shape_name)
        .ok_or_else(|| format!("struct_lit: shape `{}` not in table", shape_name))?
        .clone();

    // Evaluate and store each provided field.
    for lit_field in fields {
        let field_idx = shape_def.fields.iter().position(|f| f.name == lit_field.name)
            .ok_or_else(|| format!("struct_lit: unknown field `{}`", lit_field.name))?;
        let field_ty = shape_def.fields[field_idx].ty.clone();

        let gep = cg.builder.build_struct_gep(struct_ty, slot, field_idx as u32, &lit_field.name)
            .map_err(|e| format!("struct_lit GEP `{}`: {e}", lit_field.name))?;

        let val = lower_expr(cg, &lit_field.value)?;
        store_field(cg, val, &field_ty, gep)?;
    }

    // TODO (v0.2): evaluate hidden-field defaults and store them. For M4, zero
    // initialization above is sufficient because defaults are restricted to
    // constants and empty literals (zero is correct for int/float/bool; for
    // string/shape hidden fields, the user must call a setter before reading).

    Ok(slot.into())
}

/// Field assignment helper: `target.field = value`.
fn lower_stmt_field_assign<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    target: &Expr,
    value: &Expr,
) -> Result<(), String> {
    // Expect target to be Expr::FieldAccess
    let Expr::FieldAccess { receiver, field, .. } = target else {
        return Err(format!("codegen: FieldAssign target is not FieldAccess: {:?}", target));
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
            let recv_ty = cg.expr_type(receiver).clone();
            let recv_val = lower_expr(cg, receiver)?;
            match &recv_ty {
                Type::Shape { name } => {
                    // Trivially-copyable shape: memcpy into a fresh alloca.
                    let name = name.clone();
                    let struct_ty = cg.shape_types.get(&name)
                        .ok_or_else(|| format!(".copy(): LLVM type for `{}` not found", name))?;
                    let new_slot = cg.builder.build_alloca(struct_ty, &format!("{}_copy", name))
                        .map_err(|e| format!(".copy alloca: {e}"))?;
                    // Load the struct value and store into the new slot.
                    let val = cg.builder.build_load(struct_ty, recv_val.into_pointer_value(), "copy_src")
                        .map_err(|e| format!(".copy load: {e}"))?;
                    cg.builder.build_store(new_slot, val)
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
fn emit_type_const<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    type_name: &str,
    const_name: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    match (type_name, const_name) {
        ("int", "max") => Ok(cg.i64().const_int(i64::MAX as u64, false).into()),
        ("int", "min") => Ok(cg.i64().const_int(i64::MIN as u64, true).into()),
        ("float", "max") => Ok(cg.f64().const_float(f64::MAX).into()),
        ("float", "min") => Ok(cg.f64().const_float(f64::MIN).into()),
        ("float", "epsilon") => Ok(cg.f64().const_float(f64::EPSILON).into()),
        ("number", "max") | ("number", "min") | ("number", "epsilon") => {
            let s = match (type_name, const_name) {
                ("number", "max")     => "9.999999999999999999999999999999999e+6144",
                ("number", "min")     => "1e-6143",
                ("number", "epsilon") => "1e-33",
                _ => unreachable!(),
            };
            let bits = ynz_numerics::parse(s)
                .ok_or_else(|| format!("failed to parse decimal128 constant `{s}`"))?;
            let slot = cg.builder.build_alloca(cg.i128(), const_name).map_err(|e| format!("{e}"))?;
            let val = cg.i128().const_int_arbitrary_precision(&[
                (bits & 0xFFFF_FFFF_FFFF_FFFF) as u64,
                (bits >> 64) as u64,
            ]);
            cg.builder.build_store(slot, val).map_err(|e| format!("{e}"))?;
            Ok(slot.into())
        }
        _ => Err(format!("codegen: unknown type-attached constant `{type_name}.{const_name}`")),
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
        (Type::Int, "wrappingAdd") =>
            cg.builder.build_int_add(a, b, "wadd").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Type::Int, "wrappingSub") =>
            cg.builder.build_int_sub(a, b, "wsub").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Type::Int, "wrappingMul") =>
            cg.builder.build_int_mul(a, b, "wmul").map(|v| v.into()).map_err(|e| format!("{e}")),
        // Saturating: LLVM `sadd.sat` / `ssub.sat` / `smul.fix.sat` (scale=0) intrinsics.
        (Type::Int, "saturatingAdd") => {
            let f = declare_sat_intrinsic(cg.module, cg.ctx, "llvm.sadd.sat.i64",
                cg.i64().fn_type(&[cg.i64().into(), cg.i64().into()], false));
            let r = cg.builder.build_call(f, &[a.into(), b.into()], "sadd_sat").map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("sadd.sat void")?)
        }
        (Type::Int, "saturatingSub") => {
            let f = declare_sat_intrinsic(cg.module, cg.ctx, "llvm.ssub.sat.i64",
                cg.i64().fn_type(&[cg.i64().into(), cg.i64().into()], false));
            let r = cg.builder.build_call(f, &[a.into(), b.into()], "ssub_sat").map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("ssub.sat void")?)
        }
        (Type::Int, "saturatingMul") => {
            // `llvm.smul.fix.sat.i64(a, b, scale=0)` gives saturating signed integer mul.
            let fn_ty = cg.i64().fn_type(&[cg.i64().into(), cg.i64().into(), cg.i32().into()], false);
            let f = declare_sat_intrinsic(cg.module, cg.ctx, "llvm.smul.fix.sat.i64", fn_ty);
            let scale = cg.i32().const_int(0, false);
            let r = cg.builder.build_call(f, &[a.into(), b.into(), scale.into()], "smul_sat").map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("smul.fix.sat void")?)
        }
        _ => Err(format!("codegen: no 1-arg intrinsic for {recv_ty:?}.{method}")),
    }
}

fn declare_sat_intrinsic<'ctx>(
    module: &inkwell::module::Module<'ctx>,
    ctx: &'ctx inkwell::context::Context,
    name: &str,
    fn_ty: inkwell::types::FunctionType<'ctx>,
) -> inkwell::values::FunctionValue<'ctx> {
    let _ = ctx; // ctx not needed — type already constructed
    module.get_function(name).unwrap_or_else(|| module.add_function(name, fn_ty, None))
}

fn lower_method_call<'ctx>(
    cg: &mut Cg<'ctx, '_>,
    recv: BasicValueEnum<'ctx>,
    recv_ty: &Type,
    method: &str,
) -> Result<BasicValueEnum<'ctx>, String> {
    match (recv_ty, method) {
        (Type::Int, "toNumber") => {
            let out = cg.builder.build_alloca(cg.i128(), "i2n").map_err(|e| format!("{e}"))?;
            cg.builder.build_call(cg.rt.decimal_from_int, &[recv.into(), out.into()], "").map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }
        (Type::Int, "toFloat") => cg.builder.build_signed_int_to_float(recv.into_int_value(), cg.f64(), "i2f").map(|v| v.into()).map_err(|e| format!("{e}")),
        (Type::Int, "toString") => to_c_string(cg, recv, &Type::Int).map(|p: PointerValue<'ctx>| p.into()),
        (Type::Float, "toNumber") => {
            let out = cg.builder.build_alloca(cg.i128(), "f2n").map_err(|e| format!("{e}"))?;
            cg.builder.build_call(cg.rt.decimal_from_float, &[recv.into(), out.into()], "").map_err(|e| format!("{e}"))?;
            Ok(out.into())
        }
        (Type::Float, "toString") => to_c_string(cg, recv, &Type::Float).map(|p| p.into()),
        (Type::Number { .. }, "toFloat") => {
            let f64t = cg.f64();
            let prt = cg.ptr();
            let fn_ty = f64t.fn_type(&[prt.into()], false);
            let f = cg.module.get_function("ynz_decimal_to_float")
                .unwrap_or_else(|| cg.module.add_function("ynz_decimal_to_float", fn_ty, None));
            let r = cg.builder.build_call(f, &[recv.into()], "d2f").map_err(|e| format!("{e}"))?;
            Ok(r.try_as_basic_value().basic().ok_or("decimal_to_float void")?)
        }
        (Type::Number { .. }, "toString") => to_c_string(cg, recv, &Type::Number { precision: 34 }).map(|p| p.into()),
        (Type::Bool, "toString") => to_c_string(cg, recv, &Type::Bool).map(|p| p.into()),
        _ => Err(format!("codegen: unknown method `{method}` on {:?}", recv_ty)),
    }
}


fn store<'ctx>(cg: &mut Cg<'ctx, '_>, val: BasicValueEnum<'ctx>, ty: &Type, slot: PointerValue<'ctx>) -> Result<(), String> {
    match ty {
        Type::Number { .. } => {
            // val is a ptr to i128; load the bits then store into slot
            let bits = cg.builder.build_load(cg.i128(), val.into_pointer_value(), "dec_bits").map_err(|e| format!("{e}"))?;
            cg.builder.build_store(slot, bits).map_err(|e| format!("{e}"))?;
        }
        _ => { cg.builder.build_store(slot, val).map_err(|e| format!("{e}"))?; }
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
        Type::Number { .. } => {
            // Field stores i128 bits inline; val is ptr-to-i128.
            let bits = cg.builder.build_load(cg.i128(), val.into_pointer_value(), "dec_field_bits")
                .map_err(|e| format!("{e}"))?;
            cg.builder.build_store(field_ptr, bits).map_err(|e| format!("{e}"))?;
        }
        _ => {
            cg.builder.build_store(field_ptr, val).map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

fn load<'ctx>(cg: &mut Cg<'ctx, '_>, slot: PointerValue<'ctx>, ty: &Type, name: &str) -> Result<BasicValueEnum<'ctx>, String> {
    match ty {
        Type::Number { .. } => {
            // Load i128 bits from slot, copy into fresh alloca, return ptr
            let bits = cg.builder.build_load(cg.i128(), slot, "dec_ld").map_err(|e| format!("{e}"))?;
            let tmp = cg.builder.build_alloca(cg.i128(), name).map_err(|e| format!("{e}"))?;
            cg.builder.build_store(tmp, bits).map_err(|e| format!("{e}"))?;
            Ok(tmp.into())
        }
        // Shapes and dynamic values: the slot holds a pointer to the struct data.
        // Load that pointer and return it — the caller gets a ptr to the struct.
        Type::Shape { .. } | Type::Dynamic { .. } => {
            let lt = cg.ptr();
            cg.builder.build_load(lt, slot, name).map_err(|e| format!("{e}"))
        }
        ty => {
            let lt = cg.llvm_type_for(ty).ok_or_else(|| format!("load: unknown type {:?}", ty))?;
            cg.builder.build_load(lt, slot, name).map_err(|e| format!("{e}"))
        }
    }
}


fn build_string_global<'ctx>(ctx: &'ctx Context, module: &Module<'ctx>, s: &str, name: &str) -> GlobalValue<'ctx> {
    let i8t = ctx.i8_type();
    let mut bytes: Vec<u8> = s.bytes().collect();
    bytes.push(0);
    let arr_ty = i8t.array_type(bytes.len() as u32);
    let arr = i8t.const_array(&bytes.iter().map(|&b| i8t.const_int(b as u64, false)).collect::<Vec<_>>());
    let g = module.add_global(arr_ty, Some(AddressSpace::default()), name);
    g.set_initializer(&arr);
    g.set_constant(true);
    g.set_linkage(inkwell::module::Linkage::Private);
    g.set_unnamed_address(inkwell::values::UnnamedAddress::Global);
    g
}

fn build_decimal_global<'ctx>(ctx: &'ctx Context, module: &Module<'ctx>, bits: u128, name: &str) -> GlobalValue<'ctx> {
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
