/// State-machine codegen helpers for `wait`-containing Yinz functions.
///
/// # Why this module exists
///
/// Functions containing suspension points compile to LLVM state machines instead of
/// straight-line code. A state machine has two generated components:
///
/// 1. A **resume function** (`ynz_sm_<name>_resume`) — the actual logic, split at
///    each suspension point. Signature: `extern "C" fn(frame: *mut u8, waker_ctx: *mut u8) -> i32`.
///    Returns 0 = Ready (done), 1 = Pending (suspended; waker registered by sub-future).
///
/// 2. A **wrapper function** (`<name>` — the user-visible Yinz function) — allocates the
///    composed frame on the heap, writes parameters to frame slots, drives the state machine
///    to completion via `RUNTIME.block_on`, reads the typed return slot, frees the frame.
///
/// # Composed frame layout (P7 — inline poll-and-yield, no bridge)
///
/// Each suspending fn has a composed frame: its own header + return_slot + locals +
/// embedded sub-frames for each suspending callee at compile-time-fixed offsets.
/// The entire call tree shares ONE `ynz_alloc` per spawned task.
///
/// Per frame / sub-frame:
///
/// | Offset (relative to frame/sub-frame base) | Size | Field                              |
/// |-------------------------------------------|------|------------------------------------|
/// | 0                                          | 4    | resume_point: i32                  |
/// | 4                                          | 4    | padding                            |
/// | 8                                          | 8    | sleep_handle: ptr (null if no sleep)|
/// | 16                                         | 16   | return_slot: 16 bytes              |
/// | 32                                         | 8×N  | own locals (params in M2)          |
/// | 32+8N                                      | var  | embedded sub-frame of child SM 0   |
/// | …                                          | var  | embedded sub-frame of child SM 1   |
///
/// `resume_point` drives the switch in the resume function. On the terminal transition
/// (function exit), codegen stores the typed return value in the return_slot at offset 16,
/// then returns 0 (Ready). The parent / driver reads the typed value from that slot.
///
/// # Recursion edges
///
/// A recursive/cyclic call can't embed itself (infinite size). The recursion slot at a
/// fixed offset stores an opaque pointer to a heap-allocated child frame (`ynz_alloc`),
/// freed on Ready (normal) and in `SpawnStateFnFuture::Drop` on cancellation.
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

/// Byte offset of the `resume_point` i32 field within each (sub-)frame.
pub const FRAME_OFFSET_RESUME_POINT: u64 = 0;
/// Byte offset of the `sleep_handle` pointer field within each (sub-)frame.
/// Present regardless of whether the fn directly calls `sleep`; zeroed when unused.
pub const FRAME_OFFSET_SLEEP_HANDLE: u64 = 8;
/// Byte offset of the 16-byte return slot within each (sub-)frame.
///
/// The typed return value is stored here on terminal transition. Reading the value
/// here avoids the i32-truncation defect of the old frame[0] repurposing scheme.
pub const FRAME_OFFSET_RETURN_SLOT: u64 = 16;
/// Byte offset of the first local-variable slot within each (sub-)frame.
pub const FRAME_OFFSET_LOCALS_START: u64 = 32;
/// Size of each local-variable slot (i64 = 8 bytes).
pub const FRAME_LOCAL_SLOT_SIZE: u64 = 8;
/// Fixed per-frame header size: resume_point(4) + padding(4) + sleep_handle(8) + return_slot(16) = 32 bytes.
pub const FRAME_HEADER_SIZE: u64 = 32;

/// Compute the own-locals section size for a frame with `n_locals` local slots.
///
/// Does NOT include child sub-frames; callers add child_frame_sizes separately.
pub const fn own_locals_size(n_locals: usize) -> u64 {
    (n_locals as u64) * FRAME_LOCAL_SLOT_SIZE
}

/// Compute total frame size including own header + locals but NOT child sub-frames.
///
/// For composed frames, the total is FRAME_HEADER_SIZE + own_locals_size(n_locals) + sum(child sizes).
/// This helper covers the flat case; composed callers sum children separately.
pub const fn frame_size_flat(n_locals: usize) -> u64 {
    FRAME_HEADER_SIZE + own_locals_size(n_locals)
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

/// Load the `resume_point` i32 from offset 0 of a frame / sub-frame.
///
/// # Flow
///
/// GEP with byte offset 0 into the opaque frame pointer, then load i32.
pub fn load_resume_point<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    let rp = builder
        .build_load(ctx.i32_type(), frame_ptr, "rp")
        .map_err(|e| format!("load_resume_point: {e}"))?
        .into_int_value();
    Ok(rp)
}

/// Store a `resume_point` i32 to offset 0 of a frame / sub-frame.
///
/// # Side effects
///
/// Mutates the frame's `resume_point` field. The resume function will start at the
/// new state on its next invocation.
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

/// Return a GEP pointer to the 16-byte return slot at `FRAME_OFFSET_RETURN_SLOT` (offset 16).
///
/// # Side effects
///
/// None — produces a pointer to the slot without reading or writing it.
pub fn return_slot_ptr<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
) -> Result<PointerValue<'ctx>, String> {
    // SAFETY: FRAME_OFFSET_RETURN_SLOT=16 is within every valid frame (header is 32 bytes).
    let ptr = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                frame_ptr,
                &[ctx.i64_type().const_int(FRAME_OFFSET_RETURN_SLOT, false)],
                "ret_slot_ptr",
            )
            .map_err(|e| format!("return_slot_ptr gep: {e}"))?
    };
    Ok(ptr)
}

/// Store a typed i64 (int or bool) return value in the return slot at offset 16.
///
/// The i64 occupies the first 8 bytes of the 16-byte slot.
///
/// # Side effects
///
/// Writes 8 bytes at frame offset 16.
pub fn store_return_value_i64<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    value: inkwell::values::IntValue<'ctx>,
) -> Result<(), String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    // Widen to i64 if narrower (e.g., i1 bool or i32).
    let v_i64 = if value.get_type() != ctx.i64_type() {
        builder
            .build_int_z_extend(value, ctx.i64_type(), "ret_widen")
            .map_err(|e| format!("ret widen: {e}"))?
    } else {
        value
    };
    builder
        .build_store(slot, v_i64)
        .map_err(|e| format!("store_return_value_i64: {e}"))?;
    Ok(())
}

/// Store a typed pointer return value in the return slot at offset 16.
///
/// The pointer is stored as an i64 (ptr_to_int) in the first 8 bytes of the slot.
///
/// # Side effects
///
/// Writes 8 bytes at frame offset 16.
pub fn store_return_value_ptr<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<(), String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    let as_i64 = builder
        .build_ptr_to_int(value, ctx.i64_type(), "ret_ptr_int")
        .map_err(|e| format!("ret ptr_to_int: {e}"))?;
    builder
        .build_store(slot, as_i64)
        .map_err(|e| format!("store_return_value_ptr: {e}"))?;
    Ok(())
}

/// Store a `{i64, i64}` errors ABI result in the return slot at offset 16.
///
/// Layout: field0 (error_ptr i64) at slot+0, field1 (success_value i64) at slot+8.
/// This matches the `{i64, i64}` errors ABI used throughout M7/M8 codegen.
///
/// # Side effects
///
/// Writes 16 bytes at frame offset 16.
pub fn store_return_value_errors<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    error_ptr_i64: inkwell::values::IntValue<'ctx>,
    success_i64: inkwell::values::IntValue<'ctx>,
) -> Result<(), String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    // field0 = error_ptr at slot+0
    builder
        .build_store(slot, error_ptr_i64)
        .map_err(|e| format!("store_errors_field0: {e}"))?;
    // field1 = success at slot+8
    let slot8 = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                slot,
                &[ctx.i64_type().const_int(8, false)],
                "ret_slot8",
            )
            .map_err(|e| format!("ret_slot8 gep: {e}"))?
    };
    builder
        .build_store(slot8, success_i64)
        .map_err(|e| format!("store_errors_field1: {e}"))?;
    Ok(())
}

/// Store an f64 (float) return value in the return slot at offset 16.
///
/// The f64 is bitcast to i64 before storing so both the STORE and LOAD sides use the same
/// 8-byte integer representation. The return slot is 16 bytes; only the first 8 are used.
///
/// # Side effects
///
/// Writes 8 bytes at frame offset 16.
pub fn store_return_value_f64<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    value: inkwell::values::FloatValue<'ctx>,
) -> Result<(), String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    // Bitcast f64 → i64 so the slot stores the raw IEEE-754 bits. The matching load
    // does the inverse bitcast. Storing the f64 directly would mismatch the i64-typed
    // slot pointer and produce an LLVM type error.
    let as_i64 = builder
        .build_bit_cast(value, ctx.i64_type(), "ret_f_to_i")
        .map_err(|e| format!("store_return_value_f64 bitcast: {e}"))?
        .into_int_value();
    builder
        .build_store(slot, as_i64)
        .map_err(|e| format!("store_return_value_f64: {e}"))?;
    Ok(())
}

/// Load the f64 (float) return value from the return slot at offset 16.
///
/// Loads the 8-byte i64 and bitcasts back to f64 — the inverse of `store_return_value_f64`.
///
/// # Side effects
///
/// None — read only.
pub fn load_return_value_f64<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    name: &str,
) -> Result<inkwell::values::FloatValue<'ctx>, String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    let as_i64 = builder
        .build_load(ctx.i64_type(), slot, &format!("{name}_i64"))
        .map_err(|e| format!("load_return_value_f64 load: {e}"))?
        .into_int_value();
    let f_val = builder
        .build_bit_cast(as_i64, ctx.f64_type(), name)
        .map_err(|e| format!("load_return_value_f64 bitcast: {e}"))?
        .into_float_value();
    Ok(f_val)
}

/// Store a decimal128 (i128) return value in the 16-byte return slot at offset 16.
///
/// The i128 exactly fills the 16-byte slot. It is stored as a single i128 load rather
/// than as two i64 halves so that the return slot doubles as a stable heap-resident
/// value the wrapper can load directly — no intermediate alloca required.
///
/// # Side effects
///
/// Writes 16 bytes at frame offset 16.
pub fn store_return_value_i128<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    value: inkwell::values::IntValue<'ctx>,
) -> Result<(), String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    builder
        .build_store(slot, value)
        .map_err(|e| format!("store_return_value_i128: {e}"))?;
    Ok(())
}

/// Load the decimal128 (i128) return value from the 16-byte return slot at offset 16.
///
/// The inverse of `store_return_value_i128`. The slot is 16 bytes = exactly the size
/// of an i128, so this is a direct aligned load — no GEP arithmetic needed.
///
/// # Side effects
///
/// None — read only.
pub fn load_return_value_i128<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    name: &str,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    let v = builder
        .build_load(ctx.i128_type(), slot, name)
        .map_err(|e| format!("load_return_value_i128: {e}"))?
        .into_int_value();
    Ok(v)
}

/// Load the i64 return value from the return slot at offset 16.
///
/// Used by the parent frame or top-level driver after a child returns Ready.
///
/// # Side effects
///
/// None — read only.
pub fn load_return_value_i64<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    name: &str,
) -> Result<inkwell::values::IntValue<'ctx>, String> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    let v = builder
        .build_load(ctx.i64_type(), slot, name)
        .map_err(|e| format!("load_return_value_i64: {e}"))?
        .into_int_value();
    Ok(v)
}

/// Load the `{i64, i64}` errors ABI result from the return slot at offset 16.
///
/// Returns `(error_ptr_i64, success_i64)` — field0 and field1.
///
/// # Side effects
///
/// None — read only.
pub fn load_return_value_errors<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
) -> Result<
    (
        inkwell::values::IntValue<'ctx>,
        inkwell::values::IntValue<'ctx>,
    ),
    String,
> {
    let slot = return_slot_ptr(ctx, builder, frame_ptr)?;
    let error_ptr = builder
        .build_load(ctx.i64_type(), slot, "ret_err")
        .map_err(|e| format!("load_errors_field0: {e}"))?
        .into_int_value();
    let slot8 = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                slot,
                &[ctx.i64_type().const_int(8, false)],
                "ret_slot8_r",
            )
            .map_err(|e| format!("ret_slot8_r gep: {e}"))?
    };
    let success = builder
        .build_load(ctx.i64_type(), slot8, "ret_ok")
        .map_err(|e| format!("load_errors_field1: {e}"))?
        .into_int_value();
    Ok((error_ptr, success))
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
    // SAFETY: FRAME_OFFSET_SLEEP_HANDLE=8 is within every valid frame (header is 32 bytes).
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
/// Mutates frame offset 8. The handle must remain valid until the next poll returns Ready.
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

/// Return a GEP pointer to the 16-byte `number errors` staging slot at `staging_byte_offset`.
///
/// The staging slot stores the raw i128 decimal128 value between the point where the resume
/// function writes the success value and the point where the wrapper reads the EC ok-pointer.
/// The slot lives inside the composed frame (after own-local slots, before child sub-frames)
/// so it is freed automatically when the frame drops — no extra `ynz_free` needed.
///
/// `staging_byte_offset` is the absolute byte offset within the composed frame, computed by
/// `build_frame_layouts` and stored in `FrameLayout::number_errors_staging_offset`.
///
/// # Side effects
///
/// None — produces a pointer without reading or writing it.
pub fn number_errors_staging_ptr<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    staging_byte_offset: u64,
) -> Result<PointerValue<'ctx>, String> {
    // SAFETY: staging_byte_offset is within the composed frame (build_frame_layouts guarantees it).
    let ptr = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                frame_ptr,
                &[ctx.i64_type().const_int(staging_byte_offset, false)],
                "num_err_staging_ptr",
            )
            .map_err(|e| format!("number_errors_staging_ptr gep: {e}"))?
    };
    Ok(ptr)
}

/// Return a GEP pointer to the embedded child sub-frame at `child_byte_offset` within `parent_frame`.
///
/// The child's resume function receives this pointer as its `frame_ptr` argument.
/// The child's frame header is initialised by the parent before the first child-resume call.
///
/// # Side effects
///
/// None — produces a pointer without reading or writing.
pub fn child_frame_ptr<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    parent_frame: PointerValue<'ctx>,
    child_byte_offset: u64,
    name: &str,
) -> Result<PointerValue<'ctx>, String> {
    // SAFETY: child_byte_offset is within the parent frame (caller's FrameLayout guarantee).
    let ptr = unsafe {
        builder
            .build_gep(
                ctx.i8_type(),
                parent_frame,
                &[ctx.i64_type().const_int(child_byte_offset, false)],
                name,
            )
            .map_err(|e| format!("child_frame_ptr gep: {e}"))?
    };
    Ok(ptr)
}

/// Allocate a state-machine frame on the heap using `ynz_alloc`.
///
/// # Flow
///
/// 1. Call `ynz_alloc(total_frame_size)` to get a heap pointer.
/// 2. Zero the sleep_handle slot (offset 8) to mark "no in-flight sleep".
/// 3. Set resume_point = 0 (initial state).
///
/// The caller is responsible for writing parameter values to local slots (offset 32+).
/// Child sub-frame headers are initialised by the parent on first call to each child.
///
/// # Failure modes
///
/// Propagates `ynz_alloc` failures as `Err(String)`.
///
/// # Side effects
///
/// Heap-allocates `total_frame_size` bytes. Caller MUST free via `ynz_free` after the
/// state machine completes (or via the frame's drop guard on cancellation).
pub fn alloc_frame<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    rt: &RuntimeDecls<'ctx>,
    total_frame_size: u64,
) -> Result<PointerValue<'ctx>, String> {
    let size_val = ctx.i64_type().const_int(total_frame_size, false);
    // Use ynz_alloc_zeroed so ALL frame fields (including the recursion_slot pointer) start
    // as null. ynz_alloc gives uninitialized memory; a non-null garbage value in the
    // recursion_slot would cause SpawnStateFnFuture::Drop to chase the garbage pointer on
    // cancellation before the codegen writes the real recursion-child pointer there.
    let frame_ptr = builder
        .build_call(rt.ynz_alloc_zeroed, &[size_val.into()], "sm_frame")
        .map_err(|e| format!("alloc_frame: {e}"))?
        .try_as_basic_value()
        .basic()
        .ok_or("ynz_alloc_zeroed returned void")?
        .into_pointer_value();

    // sleep_handle and resume_point are already zero from ynz_alloc_zeroed.
    // Explicit stores are kept for clarity and SSA value correctness.
    let null_ptr = ctx.ptr_type(AddressSpace::default()).const_null();
    store_sleep_handle(ctx, builder, frame_ptr, null_ptr)?;

    // Set initial resume_point = 0.
    store_resume_point(ctx, builder, frame_ptr, 0)?;

    Ok(frame_ptr)
}

/// Free a state-machine frame allocated by `alloc_frame`.
///
/// Must be called after the top-level driver completes (state machine done normally).
/// The RAII drop guard in `SpawnStateFnFuture` handles the spawn path.
///
/// # Side effects
///
/// Frees the heap memory. `frame_ptr` is invalid after this call.
pub fn free_frame<'ctx>(
    ctx: &'ctx Context,
    builder: &inkwell::builder::Builder<'ctx>,
    rt: &RuntimeDecls<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    total_frame_size: u64,
) -> Result<(), String> {
    let size_val = ctx.i64_type().const_int(total_frame_size, false);
    builder
        .build_call(rt.ynz_free, &[frame_ptr.into(), size_val.into()], "sm_free")
        .map_err(|e| format!("free_frame: {e}"))?;
    Ok(())
}

/// Emit the poll-and-yield sequence for a `wait sleep(ms)` call inside a state machine.
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
