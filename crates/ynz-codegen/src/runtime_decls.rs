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

    // String equality for multi-case match on strings: (ptr a, ptr b) → i32 (1=equal, 0=not)
    pub string_eq: FunctionValue<'ctx>,

    // Heap allocator (M4): (size: usize) → *mut u8
    pub ynz_alloc: FunctionValue<'ctx>,
    // Zero-initialized heap allocator for SM frames: (size: usize) → *mut u8
    // Zeros all bytes so recursion_slot and other pointer fields start null.
    pub ynz_alloc_zeroed: FunctionValue<'ctx>,
    // Heap deallocator (M4): (ptr: *mut u8, size: usize) → void
    pub ynz_free: FunctionValue<'ctx>,

    // Array runtime (M5 P4a) — all operate on the heap YnzArray header pointer.
    pub ynz_array_new: FunctionValue<'ctx>,
    pub ynz_array_push: FunctionValue<'ctx>,
    pub ynz_array_get: FunctionValue<'ctx>,
    pub ynz_array_set: FunctionValue<'ctx>,
    pub ynz_array_count: FunctionValue<'ctx>,
    pub ynz_array_drop: FunctionValue<'ctx>,
    // Deep-clone for primitive-element arrays (int/float/bool — no element indirection).
    // Returns a new independent YnzArray with its own data buffer. Must be freed with
    // ynz_array_drop. Used exclusively by background arg-copy for array<primitive> args
    // so the task's copy survives the spawner's stack frame.
    pub ynz_array_clone_primitive: FunctionValue<'ctx>,

    // Map runtime (M5 P4b) — SipHash-2-4 + Swiss Tables.
    pub ynz_siphash_init: FunctionValue<'ctx>,
    pub ynz_map_new: FunctionValue<'ctx>,
    // ynz_map_get(ptr map, i64 key, ptr out[2]) -> void
    pub ynz_map_get: FunctionValue<'ctx>,
    // ynz_map_get_str(ptr map, ptr key, ptr out[2]) -> void
    pub ynz_map_get_str: FunctionValue<'ctx>,
    // ynz_map_set(ptr map, i64 key, i64 value) -> void
    pub ynz_map_set: FunctionValue<'ctx>,
    // ynz_map_set_str(ptr map, ptr key, i64 value) -> void
    pub ynz_map_set_str: FunctionValue<'ctx>,
    pub ynz_map_count: FunctionValue<'ctx>,
    pub ynz_map_has: FunctionValue<'ctx>,
    // ynz_map_iter_get(ptr map, i64 pos, ptr out[3]) -> void
    pub ynz_map_iter_get: FunctionValue<'ctx>,
    pub ynz_map_iter_get_str: FunctionValue<'ctx>,
    pub ynz_map_drop: FunctionValue<'ctx>,

    // M6: string-to-numeric fallible conversions.
    // ABI: (ptr: *const u8, len: i64, out: *mut [i64; 2]) → void
    // out[0] = has_value (1 or 0), out[1] = value bits on success.
    pub ynz_string_to_int: FunctionValue<'ctx>,
    pub ynz_string_to_float: FunctionValue<'ctx>,
    // ynz_string_to_number: out[0] = has_value, out[1..3] = 16 bytes decimal128
    pub ynz_string_to_number: FunctionValue<'ctx>,

    // M6: create a heap-owned string copy from a static byte literal.
    // ABI: (ptr: *const u8, len: i64) → *const u8 (null-terminated)
    pub ynz_string_from_static: FunctionValue<'ctx>,

    // M7 P4a: errors runtime — frame stack + error allocation.
    // ynz_frame_push(file: ptr, line: i64, function: ptr) → void
    pub ynz_frame_push: FunctionValue<'ctx>,
    // ynz_frame_pop() → void
    pub ynz_frame_pop: FunctionValue<'ctx>,
    // ynz_error_new(message: ptr) → *mut YnzError  (ptr to heap-allocated error)
    pub ynz_error_new: FunctionValue<'ctx>,
    // ynz_error_drop(err: ptr) → void
    pub ynz_error_drop: FunctionValue<'ctx>,
    // ynz_error_message(err: ptr) → *const u8
    pub ynz_error_message: FunctionValue<'ctx>,
    // ynz_unhandled_error(err: ptr) → ! (noreturn: prints and exits 1)
    pub ynz_unhandled_error: FunctionValue<'ctx>,

    // M7 P4b: string runtime — methods and builder.
    // ynz_string_validate_utf8(ptr, len: usize) → i32 (1=valid, 0=invalid)
    pub ynz_string_validate_utf8: FunctionValue<'ctx>,
    // ynz_string_concat(a: ptr, b: ptr) → ptr  (heap-alloc'd null-terminated)
    pub ynz_string_concat: FunctionValue<'ctx>,
    // ynz_string_count(s: ptr) → i64  (Unicode code-point count)
    pub ynz_string_count: FunctionValue<'ctx>,
    // ynz_string_byte_count(s: ptr) → i64  (strlen)
    pub ynz_string_byte_count: FunctionValue<'ctx>,
    // ynz_string_codepoint_at(s: ptr, n: i64) → ptr  (null = OOB)
    pub ynz_string_codepoint_at: FunctionValue<'ctx>,
    // ynz_string_byte_at(s: ptr, n: i64) → i64  (-1 = OOB)
    pub ynz_string_byte_at: FunctionValue<'ctx>,
    // ynz_string_contains(s: ptr, substr: ptr) → i32
    pub ynz_string_contains: FunctionValue<'ctx>,
    // ynz_string_index_of(s: ptr, substr: ptr) → i64  (-1 = not found)
    pub ynz_string_index_of: FunctionValue<'ctx>,
    // ynz_string_starts_with(s: ptr, prefix: ptr) → i32
    pub ynz_string_starts_with: FunctionValue<'ctx>,
    // ynz_string_ends_with(s: ptr, suffix: ptr) → i32
    pub ynz_string_ends_with: FunctionValue<'ctx>,
    // ynz_string_to_upper(s: ptr) → ptr  (heap-alloc'd)
    pub ynz_string_to_upper: FunctionValue<'ctx>,
    // ynz_string_to_lower(s: ptr) → ptr  (heap-alloc'd)
    pub ynz_string_to_lower: FunctionValue<'ctx>,
    // ynz_string_substring(s: ptr, start: i64, end: i64) → ptr  (heap-alloc'd)
    pub ynz_string_substring: FunctionValue<'ctx>,
    // ynz_string_trim(s: ptr) → ptr  (heap-alloc'd)
    pub ynz_string_trim: FunctionValue<'ctx>,
    // ynz_string_grapheme_count(s: ptr) → i64
    pub ynz_string_grapheme_count: FunctionValue<'ctx>,
    // ynz_string_grapheme_at(s: ptr, n: i64) → ptr  (null = OOB)
    pub ynz_string_grapheme_at: FunctionValue<'ctx>,
    // ynz_string_split(s: ptr, sep: ptr) → *mut YnzArray
    pub ynz_string_split: FunctionValue<'ctx>,
    // ynz_string_replace(s: ptr, from: ptr, to: ptr) → ptr  (heap-alloc'd)
    pub ynz_string_replace: FunctionValue<'ctx>,
    // String interpolation builder.
    // ynz_string_builder_new() → *mut u8  (opaque handle)
    pub ynz_string_builder_new: FunctionValue<'ctx>,
    // ynz_string_builder_append(builder: *mut u8, s: ptr) → void
    pub ynz_string_builder_append: FunctionValue<'ctx>,
    // ynz_string_builder_finalize(builder: *mut u8) → *const u8  (null-terminated, builder consumed)
    pub ynz_string_builder_finalize: FunctionValue<'ctx>,
    // ynz_string_builder_drop(builder: *mut u8) → void
    pub ynz_string_builder_drop: FunctionValue<'ctx>,

    // M8 P4 / reveal-sensitive: (ptr: *const u8) → *const u8
    // Returns the raw pointer when YNZ_REVEAL_SENSITIVE=1, otherwise a static
    // "[REDACTED]" string. Checked once per process via OnceLock.
    pub ynz_sensitive_to_string: FunctionValue<'ctx>,

    // ── M8 P6: bignum arithmetic — `number<N>` for N > 34 ────────────────
    // Each function: (a: *const i8, b: *const i8, precision: i16) → *mut i8
    // Returns a heap-allocated C string (null-terminated decimal representation).
    pub ynz_bignum_add: FunctionValue<'ctx>,
    pub ynz_bignum_sub: FunctionValue<'ctx>,
    pub ynz_bignum_mul: FunctionValue<'ctx>,
    pub ynz_bignum_div: FunctionValue<'ctx>,

    // ── v0.3-M1: Tokio scheduler runtime ─────────────────────────────────
    // Called by generated `main` and `background` lowering.
    // ynz_rt_init() → void  — initialise Tokio multi-thread runtime at main entry
    pub ynz_rt_init: FunctionValue<'ctx>,
    // ynz_rt_spawn_blocking(fn_ptr: ptr, ctx_ptr: ptr, ctx_size: i64) → void
    // fn_ptr: extern "C" fn(*mut u8); ctx_ptr + ctx_size describe heap-copy of arg struct.
    pub ynz_rt_spawn_blocking: FunctionValue<'ctx>,
    // ynz_rt_check_preempt() → void  — v0.3-M1 stub (no-op); loop back-edge cooperative yield
    pub ynz_rt_check_preempt: FunctionValue<'ctx>,
    // ynz_rt_shutdown() → void  — drain runtime at main exit (shutdown_timeout 5s)
    pub ynz_rt_shutdown: FunctionValue<'ctx>,
    // ynz_thread_sleep_ms(ms: i64) → void  — blocking sleep; used by sleepBlocking() intrinsic
    pub ynz_thread_sleep_ms: FunctionValue<'ctx>,

    // ── v0.3-M2: async state-machine runtime ─────────────────────────────
    // Declarations only — call sites are emitted in Phase 2 codegen.
    // All params are primitive C types; no Tokio types cross the ABI boundary.
    //
    // resume_fn signature (fn_ptr_2arg): extern "C" fn(*mut u8, *mut u8) -> i32
    //   arg0 = frame_ptr (*mut u8 — state-machine heap frame)
    //   arg1 = waker_ctx (*mut u8 — type-erased &mut Context<'_>)
    //   returns: 0 = Ready, 1 = Pending
    //
    // ynz_rt_spawn(resume_fn, frame_ptr, frame_size, rec_slot_offset, arg_drop_ptr, arg_drop_count) → void
    //   Schedule a state-machine Future on the Tokio I/O worker pool.
    //   arg_drop_ptr: ptr to ynz_alloc'd BgArgDropEntry array (null when arg_drop_count=0)
    //   arg_drop_count: number of entries (0 = no heap arg-copies to free)
    pub ynz_rt_spawn: FunctionValue<'ctx>,
    // ynz_rt_async_sleep_create(ms: i64) → *mut u8
    //   Allocate a boxed tokio::time::Sleep future; returns opaque heap pointer.
    pub ynz_rt_async_sleep_create: FunctionValue<'ctx>,
    // ynz_rt_async_sleep_poll(handle_ptr: *mut u8, waker_ctx: *mut u8) → i32
    //   Poll the boxed Sleep future. Returns 0 (Ready, box freed) or 1 (Pending).
    pub ynz_rt_async_sleep_poll: FunctionValue<'ctx>,
    // ynz_rt_run_entrypoint(resume_fn, frame_ptr, frame_size) → i32
    //   Program-entry state-machine driver — the tokio::main-equivalent for Yinz.
    //   Called ONLY by the codegen-emitted main wrapper and non-entry wrappers;
    //   never reachable from inside a ynz_sm_*_resume function (those inline-poll-yield).
    //   Uses Handle::block_on on Tokio threads; RUNTIME.block_on outside Tokio.
    pub ynz_rt_run_entrypoint: FunctionValue<'ctx>,

    // ── v0.3-M3d: joinable CPU spawn + poll + drop ───────────────────────
    //
    // These three functions implement the poll-based CPU join protocol (production
    // runtime ABI). They are declared unconditionally so the LLVM module is always
    // valid; production call sites are emitted by the SM-promotion lowering phase.
    // Until then, only the YNZ_M3D_SPIKE=1 path in emit.rs emits calls against them.
    //
    // ynz_rt_spawn_blocking_joinable(fn_ptr, ctx_ptr, ctx_size) → *mut u8
    //   fn_ptr: extern "C" fn(*mut u8) -> YnzCpuResult  (trampoline calling the real fn)
    //   ctx_ptr: pointer to the ctx bytes to copy into the child's heap buffer
    //   ctx_size: number of bytes to copy (0 = pass null to fn_ptr)
    //   Returns: Box::into_raw(Box<CpuJoinHandle>) cast to *mut u8.
    //            Null when called before ynz_rt_init or after ynz_rt_shutdown.
    pub ynz_rt_spawn_blocking_joinable: FunctionValue<'ctx>,
    // ynz_rt_join_poll(handle_ptr, waker_ctx, result_out) → i32
    //   handle_ptr: the *mut u8 returned by ynz_rt_spawn_blocking_joinable
    //   waker_ctx:  the &mut Context<'_> forwarded from the enclosing SM poll
    //   result_out: *mut u8 pointing to 16 bytes for the YnzCpuResult ([i64; 2])
    //   Returns: 1 = Pending (waker registered), 0 = Ready (16 bytes written + handle freed).
    //            On Ready(panic): re-raises via resume_unwind (extern "C-unwind").
    pub ynz_rt_join_poll: FunctionValue<'ctx>,
    // ynz_rt_join_handle_free(handle_ptr) → void
    //   Drops the Box<CpuJoinHandle>, detaching the blocking task. Null-safe.
    //   Called by the frame drop shim when a parent SM is cancelled mid-join.
    pub ynz_rt_join_handle_free: FunctionValue<'ctx>,
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
                void.fn_type(&[ptr.into(), ptr.into(), i32.into(), i32.into()], false),
            ),
            panic_div_by_zero: declare_fn(
                module,
                "ynz_panic_div_by_zero",
                void.fn_type(&[ptr.into(), ptr.into(), i32.into(), i32.into()], false),
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
            string_eq: declare_fn(
                module,
                "ynz_string_eq",
                i32.fn_type(&[ptr.into(), ptr.into()], false),
            ),

            ynz_alloc: declare_fn(module, "ynz_alloc", ptr.fn_type(&[i64.into()], false)),
            ynz_alloc_zeroed: declare_fn(
                module,
                "ynz_alloc_zeroed",
                ptr.fn_type(&[i64.into()], false),
            ),
            ynz_free: declare_fn(
                module,
                "ynz_free",
                void.fn_type(&[ptr.into(), i64.into()], false),
            ),

            ynz_array_new: declare_fn(module, "ynz_array_new", ptr.fn_type(&[], false)),
            ynz_array_push: declare_fn(
                module,
                "ynz_array_push",
                void.fn_type(&[ptr.into(), i64.into()], false),
            ),
            // ynz_array_get: (ptr arr, i64 idx, ptr out) -> void
            ynz_array_get: declare_fn(
                module,
                "ynz_array_get",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            // ynz_array_set: (ptr arr, i64 idx, i64 value) -> void
            ynz_array_set: declare_fn(
                module,
                "ynz_array_set",
                void.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
            ),
            ynz_array_count: declare_fn(
                module,
                "ynz_array_count",
                i64.fn_type(&[ptr.into()], false),
            ),
            ynz_array_drop: declare_fn(
                module,
                "ynz_array_drop",
                void.fn_type(&[ptr.into()], false),
            ),
            ynz_array_clone_primitive: declare_fn(
                module,
                "ynz_array_clone_primitive",
                ptr.fn_type(&[ptr.into()], false),
            ),

            ynz_siphash_init: declare_fn(module, "ynz_siphash_init", void.fn_type(&[], false)),
            ynz_map_new: declare_fn(module, "ynz_map_new", ptr.fn_type(&[], false)),
            ynz_map_get: declare_fn(
                module,
                "ynz_map_get",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            ynz_map_get_str: declare_fn(
                module,
                "ynz_map_get_str",
                void.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            ynz_map_set: declare_fn(
                module,
                "ynz_map_set",
                void.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
            ),
            ynz_map_set_str: declare_fn(
                module,
                "ynz_map_set_str",
                void.fn_type(&[ptr.into(), ptr.into(), i64.into()], false),
            ),
            ynz_map_count: declare_fn(module, "ynz_map_count", i64.fn_type(&[ptr.into()], false)),
            ynz_map_has: declare_fn(
                module,
                "ynz_map_has",
                i64.fn_type(&[ptr.into(), i64.into()], false),
            ),
            ynz_map_iter_get: declare_fn(
                module,
                "ynz_map_iter_get",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            ynz_map_iter_get_str: declare_fn(
                module,
                "ynz_map_iter_get_str",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            ynz_map_drop: declare_fn(module, "ynz_map_drop", void.fn_type(&[ptr.into()], false)),

            // M6: string-to-numeric ABI: (ptr, i64 len, ptr out) -> void
            ynz_string_to_int: declare_fn(
                module,
                "ynz_string_to_int",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            ynz_string_to_float: declare_fn(
                module,
                "ynz_string_to_float",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            ynz_string_to_number: declare_fn(
                module,
                "ynz_string_to_number",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),

            // M6: (ptr, i64 len) -> ptr (null-terminated)
            ynz_string_from_static: declare_fn(
                module,
                "ynz_string_from_static",
                ptr.fn_type(&[ptr.into(), i64.into()], false),
            ),

            // M7 P4a: errors runtime.
            ynz_frame_push: declare_fn(
                module,
                "ynz_frame_push",
                void.fn_type(&[ptr.into(), i64.into(), ptr.into()], false),
            ),
            ynz_frame_pop: declare_fn(module, "ynz_frame_pop", void.fn_type(&[], false)),
            ynz_error_new: declare_fn(module, "ynz_error_new", ptr.fn_type(&[ptr.into()], false)),
            ynz_error_drop: declare_fn(
                module,
                "ynz_error_drop",
                void.fn_type(&[ptr.into()], false),
            ),
            ynz_error_message: declare_fn(
                module,
                "ynz_error_message",
                ptr.fn_type(&[ptr.into()], false),
            ),
            // ynz_unhandled_error is noreturn; LLVM void return + unreachable after call.
            ynz_unhandled_error: declare_fn(
                module,
                "ynz_unhandled_error",
                void.fn_type(&[ptr.into()], false),
            ),

            // M7 P4b: string runtime methods and builder.
            ynz_string_validate_utf8: declare_fn(
                module,
                "ynz_string_validate_utf8",
                i32.fn_type(&[ptr.into(), i64.into()], false),
            ),
            ynz_string_concat: declare_fn(
                module,
                "ynz_string_concat",
                ptr.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_count: declare_fn(
                module,
                "ynz_string_count",
                i64.fn_type(&[ptr.into()], false),
            ),
            ynz_string_byte_count: declare_fn(
                module,
                "ynz_string_byte_count",
                i64.fn_type(&[ptr.into()], false),
            ),
            ynz_string_codepoint_at: declare_fn(
                module,
                "ynz_string_codepoint_at",
                ptr.fn_type(&[ptr.into(), i64.into()], false),
            ),
            ynz_string_byte_at: declare_fn(
                module,
                "ynz_string_byte_at",
                i64.fn_type(&[ptr.into(), i64.into()], false),
            ),
            ynz_string_contains: declare_fn(
                module,
                "ynz_string_contains",
                i32.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_index_of: declare_fn(
                module,
                "ynz_string_index_of",
                i64.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_starts_with: declare_fn(
                module,
                "ynz_string_starts_with",
                i32.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_ends_with: declare_fn(
                module,
                "ynz_string_ends_with",
                i32.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_to_upper: declare_fn(
                module,
                "ynz_string_to_upper",
                ptr.fn_type(&[ptr.into()], false),
            ),
            ynz_string_to_lower: declare_fn(
                module,
                "ynz_string_to_lower",
                ptr.fn_type(&[ptr.into()], false),
            ),
            ynz_string_substring: declare_fn(
                module,
                "ynz_string_substring",
                ptr.fn_type(&[ptr.into(), i64.into(), i64.into()], false),
            ),
            ynz_string_trim: declare_fn(
                module,
                "ynz_string_trim",
                ptr.fn_type(&[ptr.into()], false),
            ),
            ynz_string_grapheme_count: declare_fn(
                module,
                "ynz_string_grapheme_count",
                i64.fn_type(&[ptr.into()], false),
            ),
            ynz_string_grapheme_at: declare_fn(
                module,
                "ynz_string_grapheme_at",
                ptr.fn_type(&[ptr.into(), i64.into()], false),
            ),
            ynz_string_split: declare_fn(
                module,
                "ynz_string_split",
                ptr.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_replace: declare_fn(
                module,
                "ynz_string_replace",
                ptr.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            ynz_string_builder_new: declare_fn(
                module,
                "ynz_string_builder_new",
                ptr.fn_type(&[], false),
            ),
            ynz_string_builder_append: declare_fn(
                module,
                "ynz_string_builder_append",
                void.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_string_builder_finalize: declare_fn(
                module,
                "ynz_string_builder_finalize",
                ptr.fn_type(&[ptr.into()], false),
            ),
            ynz_string_builder_drop: declare_fn(
                module,
                "ynz_string_builder_drop",
                void.fn_type(&[ptr.into()], false),
            ),
            ynz_sensitive_to_string: declare_fn(
                module,
                "ynz_sensitive_to_string",
                ptr.fn_type(&[ptr.into()], false),
            ),
            // M8 P6: bignum arithmetic — (ptr a, ptr b, i16 precision) → ptr
            ynz_bignum_add: declare_fn(
                module,
                "ynz_bignum_add",
                ptr.fn_type(&[ptr.into(), ptr.into(), i32.into()], false),
            ),
            ynz_bignum_sub: declare_fn(
                module,
                "ynz_bignum_sub",
                ptr.fn_type(&[ptr.into(), ptr.into(), i32.into()], false),
            ),
            ynz_bignum_mul: declare_fn(
                module,
                "ynz_bignum_mul",
                ptr.fn_type(&[ptr.into(), ptr.into(), i32.into()], false),
            ),
            ynz_bignum_div: declare_fn(
                module,
                "ynz_bignum_div",
                ptr.fn_type(&[ptr.into(), ptr.into(), i32.into()], false),
            ),

            // v0.3-M1: Tokio scheduler runtime
            ynz_rt_init: declare_fn(module, "ynz_rt_init", void.fn_type(&[], false)),
            ynz_rt_spawn_blocking: declare_fn(
                module,
                "ynz_rt_spawn_blocking",
                // fn_ptr: opaque function pointer (ptr), ctx_ptr: *mut u8 (ptr), ctx_size: i64
                void.fn_type(&[ptr.into(), ptr.into(), i64.into()], false),
            ),
            ynz_rt_check_preempt: declare_fn(
                module,
                "ynz_rt_check_preempt",
                void.fn_type(&[], false),
            ),
            ynz_rt_shutdown: declare_fn(module, "ynz_rt_shutdown", void.fn_type(&[], false)),
            ynz_thread_sleep_ms: declare_fn(
                module,
                "ynz_thread_sleep_ms",
                void.fn_type(&[i64.into()], false),
            ),

            // v0.3-M2: async state-machine runtime (declarations; call sites in Phase 2)
            //
            // fn_ptr_2arg: function pointer for the resume_fn ABI —
            //   extern "C" fn(frame_ptr: *mut u8, waker_ctx: *mut u8) -> i32
            // Represented in LLVM as a function pointer returning i32 with two ptr args.
            ynz_rt_spawn: declare_fn(
                module,
                "ynz_rt_spawn",
                // (resume_fn: fn(ptr,ptr)->i32, frame_ptr: ptr, frame_size: i64,
                //  recursion_slot_offset: i64, arg_drop_ptr: ptr, arg_drop_count: i64) -> void
                void.fn_type(
                    &[
                        ptr.into(),
                        ptr.into(),
                        i64.into(),
                        i64.into(),
                        ptr.into(),
                        i64.into(),
                    ],
                    false,
                ),
            ),
            ynz_rt_async_sleep_create: declare_fn(
                module,
                "ynz_rt_async_sleep_create",
                // (ms: i64) -> *mut u8  (opaque heap pointer to Pin<Box<Sleep>>)
                ptr.fn_type(&[i64.into()], false),
            ),
            ynz_rt_async_sleep_poll: declare_fn(
                module,
                "ynz_rt_async_sleep_poll",
                // (handle_ptr: *mut u8, waker_ctx: *mut u8) -> i32  (0=Ready, 1=Pending)
                i32.fn_type(&[ptr.into(), ptr.into()], false),
            ),
            ynz_rt_run_entrypoint: declare_fn(
                module,
                "ynz_rt_run_entrypoint",
                // (resume_fn: fn(ptr,ptr)->i32, frame_ptr: ptr, frame_size: i64) -> i32
                // Drives the state machine to completion; return value is a legacy i32
                // (wrapper reads typed value directly from frame[16] instead).
                i32.fn_type(&[ptr.into(), ptr.into(), i64.into()], false),
            ),

            // v0.3-M3d: joinable CPU spawn + poll + drop (production runtime ABI).
            // Declared unconditionally; production call sites land with SM-promotion lowering.
            ynz_rt_spawn_blocking_joinable: declare_fn(
                module,
                "ynz_rt_spawn_blocking_joinable",
                // (fn_ptr: ptr, ctx_ptr: ptr, ctx_size: i64) -> *mut u8
                ptr.fn_type(&[ptr.into(), ptr.into(), i64.into()], false),
            ),
            ynz_rt_join_poll: declare_fn(
                module,
                "ynz_rt_join_poll",
                // (handle_ptr: ptr, waker_ctx: ptr, result_out: ptr) -> i32
                i32.fn_type(&[ptr.into(), ptr.into(), ptr.into()], false),
            ),
            ynz_rt_join_handle_free: declare_fn(
                module,
                "ynz_rt_join_handle_free",
                // (handle_ptr: ptr) -> void
                void.fn_type(&[ptr.into()], false),
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
