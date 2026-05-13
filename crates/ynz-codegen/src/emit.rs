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
    basic_block::BasicBlock,
    context::Context,
    module::Module,
    targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
    },
    types::BasicTypeEnum,
    values::{BasicValueEnum, FunctionValue, GlobalValue, PointerValue},
    AddressSpace, IntPredicate, OptimizationLevel,
};
use ynz_ast::nodes::{BinOpKind, Expr, FunctionDecl, Item, Stmt, UnaryOpKind};
use ynz_numerics; // parse(s: &str) -> Option<u128>
use ynz_typeck::{Type, TypedModule};

use crate::{artifact::{sha256, CompiledArtifact}, runtime_decls::RuntimeDecls};

/// The file ID embedded in the LLVM module for deterministic IR and object output.
pub fn module_identifier(source_path: &str) -> String {
    format!("ynz-{source_path}")
}

/// Emit a relocatable object file for an M2 program.
pub fn emit_artifact(
    source_path: &str,
    typed_module: &TypedModule,
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

    build_module(&context, &module, typed_module)?;

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

    for item in &typed.module.items {
        match item {
            Item::Function(f) => lower_function(ctx, module, &rt, &globals, typed, f)?,
        }
    }
    Ok(())
}


struct Cg<'ctx, 'g> {
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    builder: inkwell::builder::Builder<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    current_fn: FunctionValue<'ctx>,
    locals: HashMap<String, PointerValue<'ctx>>,
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

fn lower_function<'ctx, 'g>(
    ctx: &'ctx Context,
    module: &'g Module<'ctx>,
    rt: &'g RuntimeDecls<'ctx>,
    globals: &'g ModuleGlobals<'ctx>,
    typed: &'g TypedModule,
    f: &FunctionDecl,
) -> Result<(), String> {
    let fn_ty = ctx.i32_type().fn_type(&[], false);
    let fn_val = module.add_function("main", fn_ty, None);

    let mut cg = Cg {
        ctx,
        module,
        builder: ctx.create_builder(),
        rt,
        globals,
        typed,
        current_fn: fn_val,
        locals: HashMap::new(),
    };

    let entry = ctx.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);

    for stmt in &f.body.stmts {
        lower_stmt(&mut cg, stmt)?;
    }

    cg.builder.build_return(Some(&ctx.i32_type().const_int(0, false)))
        .map_err(|e| format!("return: {e}"))?;
    Ok(())
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
        // M3 control flow — lowering implemented in Phase 4.
        Stmt::If { .. } | Stmt::Match { .. } | Stmt::While { .. } | Stmt::For { .. } | Stmt::Return { .. } => {
            return Err("M3 control-flow lowering not yet implemented (Phase 4)".to_string());
        }
    }
    Ok(())
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
            if let Expr::Ident(name, _) = &call.callee {
                if name == "print" && call.args.len() == 1 {
                    let ty = cg.expr_type(&call.args[0]).clone();
                    let val = lower_expr(cg, &call.args[0])?;
                    lower_print(cg, val, &ty)?;
                    return Ok(cg.i32().const_int(0, false).into());
                }
            }
            Err("codegen: unsupported call".to_string())
        }

        Expr::MethodCall { receiver, method, args, .. } => {
            let recv_ty = cg.expr_type(receiver).clone();
            for a in args { lower_expr(cg, a)?; }
            let recv_val = lower_expr(cg, receiver)?;
            lower_method_call(cg, recv_val, &recv_ty, method)
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

fn load<'ctx>(cg: &mut Cg<'ctx, '_>, slot: PointerValue<'ctx>, ty: &Type, name: &str) -> Result<BasicValueEnum<'ctx>, String> {
    match ty {
        Type::Number { .. } => {
            // Load i128 bits from slot, copy into fresh alloca, return ptr
            let bits = cg.builder.build_load(cg.i128(), slot, "dec_ld").map_err(|e| format!("{e}"))?;
            let tmp = cg.builder.build_alloca(cg.i128(), name).map_err(|e| format!("{e}"))?;
            cg.builder.build_store(tmp, bits).map_err(|e| format!("{e}"))?;
            Ok(tmp.into())
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
