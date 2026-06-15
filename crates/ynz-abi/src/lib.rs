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
/// contiguous from here. MUST equal codegen's `FRAME_HEADER_SIZE` — the handle region
/// begins immediately after the frame header. The cross-crate compile-time assertion
/// binding the two lives in `ynz-codegen` (`emit.rs`).
pub const SPIKE_HANDLE_BASE_OFFSET: usize = 32;

/// Bytes per CPU-handle slot (`*mut CpuJoinHandle`). Handle slot `i` lives at
/// `SPIKE_HANDLE_BASE_OFFSET + i * SPIKE_HANDLE_SLOT_BYTES`.
pub const SPIKE_HANDLE_SLOT_BYTES: usize = 8;
