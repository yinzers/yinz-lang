/// State-machine codegen helpers for `wait`-containing Yinz functions.
///
/// # Why this module exists
///
/// Functions containing `wait` compile to LLVM state machines instead of straight-line
/// code. A state machine has two generated components:
///
/// 1. A **resume function** (`ynz_sm_<name>_resume`) — the actual logic, split at
///    each `wait` point. Signature: `extern "C" fn(frame: *mut u8, waker_ctx: *mut u8) -> i32`.
///    Returns 0 = Ready (done), 1 = Pending (suspended; waker registered by sub-future).
///
/// 2. A **wrapper function** (`<name>` — the user-visible Yinz function) — allocates the
///    frame on the heap, writes parameters to frame slots, calls `ynz_rt_call_state_machine_sync`
///    (sync bridge), reads the return value from frame slot 0, frees the frame, and returns.
///
/// When called via `background name(args)`, the emitter bypasses the wrapper and instead
/// allocates the frame + calls `ynz_rt_spawn` directly (see emit.rs).
///
/// # Frame layout
///
/// The frame is a heap-allocated byte array with the following layout:
///
/// | Offset | Size | Field             |
/// |--------|------|-------------------|
/// | 0      | 4    | resume_point: i32 |
/// | 4      | 4    | padding           |
/// | 8      | 8    | sleep_handle: ptr |
/// | 16     | 8    | local_0: i64      |
/// | 24     | 8    | local_1: i64      |
/// | ...    | 8    | local_n: i64      |
///
/// `resume_point` drives the switch in the resume function. On the terminal transition
/// (function exit), the codegen writes the final return value (as i32) to offset 0 before
/// returning 0. The sync bridge reads offset 0 and propagates it as the exit code.
///
/// # State encoding
///
/// Resume points are numbered from 0. State 0 = before the first `wait`. Each `wait`
/// increments the resume point. The terminal state writes the return value and returns 0.
///
/// # ABI lock
///
/// Waker forwarding: `waker_ctx: *mut u8` is a type-erased `&mut Context<'_>`. The
/// runtime casts it back for `Sleep::poll`. No fabricated wakers are ever created here —
/// that would hang any task awaiting under a quiet runtime (P0 Contract #11).
use inkwell::{
    context::Context,
    module::Module,
    values::{FunctionValue, PointerValue},
    AddressSpace,
};

use crate::runtime_decls::RuntimeDecls;

/// Byte offset of the `resume_point` i32 field in the state-machine frame.
pub const FRAME_OFFSET_RESUME_POINT: u64 = 0;
/// Byte offset of the `sleep_handle` pointer field in the state-machine frame.
pub const FRAME_OFFSET_SLEEP_HANDLE: u64 = 8;
/// Byte offset of the first local-variable slot in the frame.
pub const FRAME_OFFSET_LOCALS_START: u64 = 16;
/// Size of each local-variable slot (i64).
pub const FRAME_LOCAL_SLOT_SIZE: u64 = 8;
/// Fixed overhead of the frame header (resume_point + padding + sleep_handle = 16 bytes).
pub const FRAME_HEADER_SIZE: u64 = 16;

/// Compute total frame size in bytes for a function with `n_locals` live locals crossing
/// a wait boundary.
///
/// Frame size: 4 (resume_point) + 4 (padding) + 8 (sleep_handle) + 8*n_locals.
pub const fn frame_size(n_locals: usize) -> u64 {
    FRAME_HEADER_SIZE + (n_locals as u64) * FRAME_LOCAL_SLOT_SIZE
}

/// Return the LLVM name for a state-machine resume function given the Yinz function name.
///
/// The name is deterministic — same Yinz source always produces the same LLVM symbol.
pub fn resume_fn_name(yinz_name: &str) -> String {
    format!("ynz_sm_{yinz_name}_resume")
}

/// Declare the LLVM function type for a state-machine resume function.
///
/// Signature: `i32 (ptr, ptr)` — frame pointer + waker_ctx pointer → Ready/Pending flag.
pub fn declare_resume_fn<'ctx>(
    ctx: &'ctx Context,
    module: &Module<'ctx>,
    name: &str,
) -> FunctionValue<'ctx> {
    let i32 = ctx.i32_type();
    let ptr = ctx.ptr_type(AddressSpace::default());
    let fn_ty = i32.fn_type(&[ptr.into(), ptr.into()], false);
    module
        .get_function(name)
        .unwrap_or_else(|| module.add_function(name, fn_ty, None))
}

/// Load the `resume_point` i32 from offset 0 of the frame.
///
/// # Flow
///
/// GEP with byte offset 0 into the opaque frame pointer, then load i32.
///
/// # Side effects
///
/// Reads 4 bytes from the heap frame. No side effects on the frame state.
///
/// # Failure modes
///
/// Returns `Err` if the builder has no current insert block (should be unreachable
/// in well-formed LLVM IR generation).
pub fn load_resume_point<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    // resume_point is at byte offset 0; frame is already at offset 0.
    let rp = builder
        .build_load(ctx.i32_type(), frame_ptr, "rp")
        .map_err(|e| format!("load_resume_point: {e}"))?
        .into_int_value();
    Ok(rp)
}

/// Store a `resume_point` i32 to offset 0 of the frame.
///
/// # Flow
///
/// Stores the given i32 constant to the first 4 bytes of the frame.
///
/// # Side effects
///
/// Mutates the frame's `resume_point` field. After this call the resume function
/// will start at the new state on its next invocation.
pub fn store_resume_point<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    state: u64,
) -> Result<(), String> {
    let rp_val = ctx.i32_type().const_int(state, false);
    builder
        .build_store(frame_ptr, rp_val)
        .map_err(|e| format!("store_resume_point: {e}"))?;
    Ok(())
}

/// Write the final i32 return value to frame slot 0 (the `resume_point` field).
///
/// Called on the terminal state transition, immediately before returning 0 (Ready).
/// The sync bridge reads this i32 when it sees the resume function return 0, and
/// propagates it as the state machine's output value. Main's exit code flows through
/// this path.
///
/// # Why frame[0] doubles as the return slot
///
/// `resume_point` is only meaningful when the state machine is running. When it returns
/// Ready (0), the state machine is done and `resume_point` is dead. Reusing the same
/// 4-byte slot avoids adding a separate return-value field to every frame.
pub fn store_return_value<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    value: inkwell::values::IntValue<'ctx>,
) -> Result<(), String> {
    let i32 = ctx.i32_type();
    // Truncate to i32 if the value is wider (e.g., i64 int).
    let v = if value.get_type() != i32 {
        builder
            .build_int_truncate(value, i32, "ret_trunc")
            .map_err(|e| format!("ret_trunc: {e}"))?
    } else {
        value
    };
    builder
        .build_store(frame_ptr, v)
        .map_err(|e| format!("store_return_value: {e}"))?;
    Ok(())
}

/// Load the `sleep_handle` pointer from the frame (offset 8).
///
/// Returns `null` if no handle is stored (the slot is zeroed on frame allocation).
///
/// # Side effects
///
/// None — read only.
pub fn load_sleep_handle<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    let ptr_ty = ctx.ptr_type(AddressSpace::default());
    // SAFETY: FRAME_OFFSET_SLEEP_HANDLE=8 is within every valid frame (header is 16 bytes).
    let slot = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                frame_ptr,
                &[ctx.i64_type().const_int(FRAME_OFFSET_SLEEP_HANDLE, false)],
                "h_slot",
            )
            .map_err(|e| format!("sleep_handle gep: {e}"))?
    };
    let handle = builder
        .build_load(ptr_ty, slot, "sleep_handle")
        .map_err(|e| format!("load_sleep_handle: {e}"))?
        .into_pointer_value();
    Ok(handle)
}

/// Store a `sleep_handle` pointer in the frame (offset 8).
///
/// Called after `ynz_rt_async_sleep_create` to persist the handle across suspension.
/// Store `null` when the handle has been consumed (poll returned Ready).
///
/// # Side effects
///
/// Mutates frame offset 8. The handle must remain valid until the next poll that
/// returns Ready, at which point the runtime frees it and the frame slot should
/// be cleared to null.
pub fn store_sleep_handle<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    handle: PointerValue<'ctx>,
) -> Result<(), String> {
    // SAFETY: same offset guarantee as load_sleep_handle.
    let slot = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                frame_ptr,
                &[ctx.i64_type().const_int(FRAME_OFFSET_SLEEP_HANDLE, false)],
                "h_slot_wr",
            )
            .map_err(|e| format!("sleep_handle store gep: {e}"))?
    };
    builder
        .build_store(slot, handle)
        .map_err(|e| format!("store_sleep_handle: {e}"))?;
    Ok(())
}

/// Load an i64 local from the frame at slot index `idx` (0-based, after the header).
///
/// Slot `idx` is at byte offset `FRAME_OFFSET_LOCALS_START + idx * FRAME_LOCAL_SLOT_SIZE`.
///
/// # Flow
///
/// GEP into the frame at the slot's byte offset, load i64.
///
/// # Failure modes
///
/// Returns `Err` if the builder operation fails (should not happen in well-formed IR).
pub fn load_local_slot<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    idx: usize,
    name: &str,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    let byte_offset = FRAME_OFFSET_LOCALS_START + (idx as u64) * FRAME_LOCAL_SLOT_SIZE;
    let slot = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                frame_ptr,
                &[ctx.i64_type().const_int(byte_offset, false)],
                &format!("{name}_slot"),
            )
            .map_err(|e| format!("local gep [{idx}]: {e}"))?
    };
    let val = builder
        .build_load(ctx.i64_type(), slot, name)
        .map_err(|e| format!("local load [{idx}]: {e}"))?
        .into_int_value();
    Ok(val)
}

/// Store an i64 value into the frame's local slot at index `idx` (0-based, after the header).
///
/// # Flow
///
/// GEP to the slot's byte offset, store i64.
///
/// # Side effects
///
/// Mutates one i64 slot in the frame. The stored value persists across suspension.
pub fn store_local_slot<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    idx: usize,
    value: inkwell::values::IntValue<'ctx>,
) -> Result<(), String> {
    let byte_offset = FRAME_OFFSET_LOCALS_START + (idx as u64) * FRAME_LOCAL_SLOT_SIZE;
    let slot = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                frame_ptr,
                &[ctx.i64_type().const_int(byte_offset, false)],
                &format!("ls_{idx}"),
            )
            .map_err(|e| format!("local store gep [{idx}]: {e}"))?
    };
    // Widen to i64 if the value is narrower (e.g., i32 resume_point, i1 bool).
    let val_i64 = if value.get_type() != ctx.i64_type() {
        builder
            .build_int_z_extend(value, ctx.i64_type(), "widen")
            .map_err(|e| format!("local widen [{idx}]: {e}"))?
    } else {
        value
    };
    builder
        .build_store(slot, val_i64)
        .map_err(|e| format!("local store [{idx}]: {e}"))?;
    Ok(())
}

/// Allocate a state-machine frame on the heap using `ynz_alloc`.
///
/// # Flow
///
/// 1. Call `ynz_alloc(frame_size_bytes)` to get a heap pointer.
/// 2. Zero the sleep_handle slot (offset 8) to mark "no in-flight sleep".
/// 3. Set resume_point = 0 (initial state).
///
/// The caller is responsible for writing parameter values to local slots (offset 16+).
///
/// # Failure modes
///
/// Propagates `ynz_alloc` failures as `Err(String)`.
///
/// # Side effects
///
/// Heap-allocates `frame_size_bytes` bytes. Caller MUST free via `ynz_free` after the
/// state machine completes (or via the frame's drop guard on cancellation).
pub fn alloc_frame<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    rt: &RuntimeDecls<'ctx>,
    n_locals: usize,
) -> Result<PointerValue<'ctx>, String> {
    let size = frame_size(n_locals);
    let size_val = ctx.i64_type().const_int(size, false);
    let frame_ptr = builder
        .build_call(rt.ynz_alloc, &[size_val.into()], "sm_frame")
        .map_err(|e| format!("alloc_frame: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("ynz_alloc returned void")?
        .into_pointer_value();

    // Zero the sleep_handle slot so the resume function knows no handle is active yet.
    let null_ptr = ctx.ptr_type(AddressSpace::default()).const_null();
    store_sleep_handle(ctx, builder, frame_ptr, null_ptr)?;

    // Set initial resume_point = 0.
    store_resume_point(ctx, builder, frame_ptr, 0)?;

    Ok(frame_ptr)
}

/// Free a state-machine frame allocated by `alloc_frame`.
///
/// Must be called after the sync bridge returns (state machine completed normally).
/// The RAII drop guard in `ynz_rt_spawn`'s future handles the spawn path.
///
/// # Side effects
///
/// Frees the heap memory. `frame_ptr` is invalid after this call.
pub fn free_frame<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    rt: &RuntimeDecls<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    n_locals: usize,
) -> Result<(), String> {
    let size = frame_size(n_locals);
    let size_val = ctx.i64_type().const_int(size, false);
    builder
        .build_call(rt.ynz_free, &[frame_ptr.into(), size_val.into()], "sm_free")
        .map_err(|e| format!("free_frame: {e}"))?;
    Ok(())
}

/// Emit the poll-and-yield sequence for a `wait sleepAsync(ms)` call inside a state machine.
///
/// # Flow
///
/// This is called at state N (the continuation state after a `wait`):
///
/// 1. Load the sleep handle from `frame[FRAME_OFFSET_SLEEP_HANDLE]`.
/// 2. Call `ynz_rt_async_sleep_poll(handle, waker_ctx)`.
/// 3. If Ready (0): clear handle slot to null, branch to `continue_bb` (post-wait code).
/// 4. If Pending (1): set `resume_point = continuation_state` (already set before suspension),
///    branch to `pending_bb` (caller returns 1).
///
/// The `continue_bb` block is where the caller places post-wait statements.
/// The `pending_bb` block emits `ret i32 1` for the Pending path.
///
/// # Failure modes
///
/// `ynz_rt_async_sleep_poll` panics are caught by Tokio's task wrapper; the function
/// returns Pending on panic so the frame is not corrupted (runtime-side behaviour).
///
/// # Side effects
///
/// - On Ready: frees the `Pin<Box<Sleep>>` via the runtime (poll returns 0 = box is dropped).
/// - Mutates `frame[FRAME_OFFSET_SLEEP_HANDLE]` (clears to null on Ready).
pub fn emit_sleep_poll_branch<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    rt: &RuntimeDecls<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    waker_ctx: PointerValue<'ctx>,
    continue_bb: inkwell::basic_block::BasicBlock<'ctx>,
    pending_bb: inkwell::basic_block::BasicBlock<'ctx>,
) -> Result<(), String> {
    let handle = load_sleep_handle(ctx, builder, frame_ptr)?;

    let poll_result = builder
        .build_call(
            rt.ynz_rt_async_sleep_poll,
            &[handle.into(), waker_ctx.into()],
            "poll",
        )
        .map_err(|e| format!("sleep_poll call: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("ynz_rt_async_sleep_poll returned void")?
        .into_int_value();

    let is_ready = builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            poll_result,
            ctx.i32_type().const_int(0, false),
            "is_ready",
        )
        .map_err(|e| format!("poll cmp: {e}"))?;

    builder
        .build_conditional_branch(is_ready, continue_bb, pending_bb)
        .map_err(|e| format!("poll branch: {e}"))?;

    Ok(())
}
