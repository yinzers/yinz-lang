use inkwell::{module::Module, types::FunctionType, values::FunctionValue, AddressSpace};

/// All extern "C" functions the generated code may call into `libynz_rt.a`.
///
/// Declared exactly once per LLVM module. Signatures match the buffer-ABI
/// locked in the M2 architectural-decisions block of the plan.
pub struct RuntimeDecls<'ctx> {
    // puts(ptr) → i32  (libc, not runtime)
    pub puts: FunctionValue<'ctx>,

    // Decimal arithmetic: (ptr lhs, ptr rhs, ptr out) → void
    pub decimal_add: FunctionValue<'ctx>,
    pub decimal_sub: FunctionValue<'ctx>,
    pub decimal_mul: FunctionValue<'ctx>,
    pub decimal_div: FunctionValue<'ctx>,

    // Decimal comparison: (ptr a, ptr b) → i32  (-1 / 0 / 1)
    pub decimal_compare: FunctionValue<'ctx>,

    // Decimal conversion from int: (i64 x, ptr out) → void
    pub decimal_from_int: FunctionValue<'ctx>,
    // Decimal conversion from float: (double x, ptr out) → void
    pub decimal_from_float: FunctionValue<'ctx>,

    // Formatting: (value, ptr buf, i64 buf_len) → i64 bytes_written
    pub decimal_to_string: FunctionValue<'ctx>,
    pub int_to_string: FunctionValue<'ctx>,
    pub float_to_string: FunctionValue<'ctx>,

    // Panic stubs — noreturn, write to stderr then abort
    pub panic_overflow: FunctionValue<'ctx>,
    pub panic_div_by_zero: FunctionValue<'ctx>,

    // LLVM overflow intrinsics: (i64, i64) → {i64, i1}
    pub sadd_overflow: FunctionValue<'ctx>,
    pub ssub_overflow: FunctionValue<'ctx>,
    pub smul_overflow: FunctionValue<'ctx>,
}

impl<'ctx> RuntimeDecls<'ctx> {
    pub fn declare(ctx: &'ctx inkwell::context::Context, module: &Module<'ctx>) -> Self {
        let void = ctx.void_type();
        let i1 = ctx.bool_type();
        let i32 = ctx.i32_type();
        let i64 = ctx.i64_type();
        let f64 = ctx.f64_type();
        let ptr = ctx.ptr_type(AddressSpace::default());
        // {i64, i1} struct for overflow intrinsics
        let i64_i1 = ctx.struct_type(&[i64.into(), i1.into()], false);

        Self {
            puts: declare_fn(module, "puts", i32.fn_type(&[ptr.into()], false)),

            decimal_add: declare_fn(
                module,
                "ynz_decimal_add",
                void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            decimal_sub: declare_fn(
                module,
                "ynz_decimal_sub",
                void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            decimal_mul: declare_fn(
                module,
                "ynz_decimal_mul",
                void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            decimal_div: declare_fn(
                module,
                "ynz_decimal_div",
                void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            decimal_compare: declare_fn(
                module,
                "ynz_decimal_compare",
                i32.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            decimal_from_int: declare_fn(
                module,
                "ynz_decimal_from_int",
                void.fn_type(&[i64.into(), ptr.into()], false),
            ),
            decimal_from_float: declare_fn(
                module,
                "ynz_decimal_from_float",
                void.fn_type(&[f64.into(), ptr.into()], false),
            ),
            // Format shims: value → ptr to null-terminated string in thread-local buffer.
            // Runtime uses thread-local static buffers, not caller-allocated buffers.
            decimal_to_string: declare_fn(
                module,
                "ynz_decimal_to_string",
                ptr.fn_type(&[ptr.into()], false),
            ),
            int_to_string: declare_fn(
                module,
                "ynz_int_to_string",
                ptr.fn_type(&[i64.into()], false),
            ),
            float_to_string: declare_fn(
                module,
                "ynz_float_to_string",
                ptr.fn_type(&[f64.into()], false),
            ),
            panic_overflow: declare_fn(
                module,
                "ynz_panic_overflow",
                void.fn_type(&[ptr.into()], false),
            ),
            panic_div_by_zero: declare_fn(
                module,
                "ynz_panic_div_by_zero",
                void.fn_type(&[ptr.into()], false),
            ),
            sadd_overflow: declare_fn(
                module,
                "llvm.sadd.with.overflow.i64",
                i64_i1.fn_type(&[i64.into(), i64.into()], false),
            ),
            ssub_overflow: declare_fn(
                module,
                "llvm.ssub.with.overflow.i64",
                i64_i1.fn_type(&[i64.into(), i64.into()], false),
            ),
            smul_overflow: declare_fn(
                module,
                "llvm.smul.with.overflow.i64",
                i64_i1.fn_type(&[i64.into(), i64.into()], false),
            ),
        }
    }
}

fn declare_fn<'ctx>(
    module: &Module<'ctx>,
    name: &str,
    ty: FunctionType<'ctx>,
) -> FunctionValue<'ctx> {
    module
        .get_function(name)
        .unwrap_or_else(|| module.add_function(name, ty, None))
}
