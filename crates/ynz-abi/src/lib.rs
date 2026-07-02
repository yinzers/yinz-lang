//! Frame-ABI constants shared between the Yinz compiler and runtime.
//!
//! `ynz-codegen` writes the spike-frame discriminator and CPU-handle slots into a
//! state-machine frame; `ynz-runtime`'s `cleanup_spike_cpu_handles` reads them back on
//! drop. The two crates therefore agree on a binary layout contract. This crate is the
//! single home for that contract so neither side defines the values locally and drifts.
//!
//! It has **zero dependencies on purpose**: codegen needs only these constants, and
//! depending on `ynz-runtime` directly would drag tokio + simdutf8 + num_cpus into the
//! compiler's transitive tree (and into `--kernel`-mode builds). A constants-only crate
//! keeps the compiler's dependency graph free of the async runtime.

/// Byte offset of the `resume_point` i32 field within each (sub-)frame. Drives the switch
/// in the generated resume function. Offset 0 needs no GEP arithmetic on either side of the
/// seam (both `ynz-codegen` and `ynz-runtime` address it via the bare frame pointer), so this
/// constant exists purely as the documented single home for the offset — see the composed-frame
/// layout table in `ynz-codegen`'s `state_machine` module for the full per-frame layout.
pub const FRAME_OFFSET_RESUME_POINT: u64 = 0;

/// Byte offset of the `sleep_handle` pointer field within each (sub-)frame. Present regardless
/// of whether the fn directly calls `sleep`; zeroed when unused. `ynz-codegen` writes this slot
/// (`store_sleep_handle` / `load_sleep_handle` in `state_machine.rs`); `ynz-runtime`'s
/// `SpawnStateFnFuture::drop` reads it to free any in-flight `Sleep` box on cancellation. This
/// is the general-SM-frame counterpart to the spike-only offsets below — moved here (v0.3-M3g
/// Phase 1) after a prose-only "must stay in sync" comment on each side went unenforced by any
/// compile-time binding, the same drift class sub-slice 4d closed for the spike-frame offsets.
pub const FRAME_OFFSET_SLEEP_HANDLE: u64 = 8;

/// Byte offset of the 16-byte return slot within each (sub-)frame. The typed return value is
/// stored here on the terminal transition (function exit); the driver (`ynz-runtime`) and any
/// synthesized cross-module frame read the typed value from this slot.
pub const FRAME_OFFSET_RETURN_SLOT: u64 = 16;

/// Fixed per-frame header size: resume_point(4) + padding(4) + sleep_handle(8) + return_slot(16)
/// = 32 bytes. `ynz-codegen` sizes every composed frame's header region to this value;
/// `SPIKE_HANDLE_BASE_OFFSET` below is the byte offset immediately following it (bound by a
/// same-crate compile-time assertion in `ynz-codegen`'s `emit.rs`, since the two are still
/// independently-valued constants even though both now live in this one crate).
pub const FRAME_HEADER_SIZE: u64 = 32;

/// High-16-bit tag identifying a spike frame, ASCII "SP". The spike-frame discriminator
/// word packs `(SPIKE_FRAME_TAG << 16) | handle_count`: the high 16 bits carry this tag,
/// the low 16 bits carry the live CPU-handle count. A zeroed (non-spike) frame has a zero
/// tag and never matches. Codegen writes this exact tag; the runtime checks for it.
pub const SPIKE_FRAME_TAG: u32 = 0x5350;

/// Byte offset of the spike-frame discriminator word within a state-machine frame. The
/// discriminator is the single u32 written by codegen at spawn time and read by the runtime's
/// `cleanup_spike_cpu_handles` on drop; it packs `(SPIKE_FRAME_TAG << 16) | handle_count`. It
/// sits in the frame header (bytes 4-7), which normal SM frames leave zeroed, so the high-bits
/// tag never false-matches a non-spike frame. Codegen writes the word at this offset; the
/// runtime reads it at this offset — one shared constant keeps the two sides from drifting.
pub const SPIKE_FRAME_DISCRIMINATOR_OFFSET: usize = 4;

/// Byte offset of the first CPU join handle pointer in a spike frame. Handle slots are
/// contiguous from here. MUST equal `FRAME_HEADER_SIZE` above — the handle region begins
/// immediately after the frame header. Both constants now live in this one crate (v0.3-M3g
/// Phase 1 moved `FRAME_HEADER_SIZE` here alongside it), but stay independently-valued
/// literals rather than one deriving from the other, so the existing same-crate compile-time
/// assertion in `ynz-codegen` (`emit.rs`) still catches either one drifting without the other.
pub const SPIKE_HANDLE_BASE_OFFSET: usize = 32;

/// Bytes per CPU-handle slot (`*mut CpuJoinHandle`). Handle slot `i` lives at
/// `SPIKE_HANDLE_BASE_OFFSET + i * SPIKE_HANDLE_SLOT_BYTES`.
pub const SPIKE_HANDLE_SLOT_BYTES: usize = 8;
