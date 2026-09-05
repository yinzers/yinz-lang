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

// ── v0.3-M4 Phase 2: background-handle completion-extraction kinds (R8) ──────
//
// `ynz-codegen` emits ONE of these per `let h = background f(...)` spawn site, keyed at
// COMPILE TIME on the callee's declared return type; `ynz-runtime`'s `HandleStateFnFuture`
// reads it to extract the completion value from the frame's return slot BEFORE the frame is
// freed (copy-before-free for frame-interior wide values — IMP-concurrency:475/477). Shared
// here so neither side defines the values locally and drifts (the R6 discipline applied to
// this seam).

/// `-> T errors` with a self-contained ok-word (int/bool/float bits or a heap-stable
/// pointer): read `{err, ok}` from the return slot as-is.
pub const HANDLE_RET_KIND_EC_WORD: i64 = 0;
/// `-> number errors`: the ok-word points INTO the frame's 16-byte staging slot — copy the
/// 16 bytes to the handle-owned buffer before the frame is freed; repoint the ok-word.
pub const HANDLE_RET_KIND_EC_NUMBER: i64 = 1;
/// Plain `-> T` (i64-slot value, incl. `nothing`): completion is `{0, slot}`.
pub const HANDLE_RET_KIND_VALUE_WORD: i64 = 2;
/// Plain `-> number`: the return slot itself holds the 16-byte decimal — copy it to the
/// handle-owned buffer; completion is `{0, buf}`.
pub const HANDLE_RET_KIND_VALUE_NUMBER: i64 = 3;
/// Plain `-> array<T>` / `-> map<K, V>`: the slot word is a heap-stable pointer the parent
/// takes ownership of through `h.receive()`. Extraction is identical to `VALUE_WORD`; the
/// distinct kind exists so the runtime can release the pointer from the child's spawn-arg
/// drop ladder (`BG_ARG_KIND_*` below) when the child returns one of its own heap-cloned
/// arguments — otherwise the ladder would free what the parent now holds.
pub const HANDLE_RET_KIND_VALUE_HEAP_PTR: i64 = 4;
/// `-> array<T> errors` / `-> map<K, V> errors`: the `EC_WORD` twin of `VALUE_HEAP_PTR` —
/// the ok-word is a heap-stable pointer released from the drop ladder on the ok path.
pub const HANDLE_RET_KIND_EC_HEAP_PTR: i64 = 5;
/// Plain `-> maybe<T>` (v0.3-M8 Phase 4): the return slot holds the envelope's `{flag, bits}`
/// PAIR inline (the resume fn stores the pair, never a pointer into its own dead stack) — a
/// 16-byte aggregate exactly like `VALUE_NUMBER`, so extraction copies the 16 bytes to the
/// handle-owned buffer before the frame is freed; completion is `{0, buf}`, and the parent
/// reads `buf` as its `maybe<T>` envelope. Before this kind existed the fallthrough classified
/// it `VALUE_WORD` and handed the parent the FLAG word as a pointer (SIGSEGV on `.value`).
/// (`-> maybe<T> errors` needs no twin: its ok-word is already a heap cell the resume fn
/// promotes at the return, `EC_WORD` reads it as-is.)
pub const HANDLE_RET_KIND_VALUE_MAYBE: i64 = 6;

// ── `background` spawn-argument drop-ladder descriptor kinds ─────────────────
//
// `ynz-codegen` heap-clones every heap-typed `background` argument so it survives the
// spawner's frame, and hands `ynz_rt_spawn` / `ynz_rt_spawn_handle` one descriptor per clone
// (`ynz-runtime`'s `BgArgDropEntry { byte_offset, kind, size }`). The runtime's task-retire
// drop ladder reads the pointer back out of the named frame slot and frees it by `kind`.
// Shared here so the two sides cannot drift on the wire values.

/// `ynz_alloc`'d cell (a shape copy, a decimal128 cell, a maybe envelope, a map-entry cell):
/// freed with `ynz_free(ptr, size)`.
pub const BG_ARG_KIND_HEAP_SHAPE: u64 = 0;
/// Array clone (`ynz_array_clone_primitive`): freed with `ynz_array_drop(ptr)`; `size` unused.
pub const BG_ARG_KIND_HEAP_ARRAY: u64 = 1;
/// The task's refcounted `channel<T>` reference (`ynz_channel_share` at the spawn site):
/// released with `ynz_channel_free(ptr)` after purging the task's suspended sends.
pub const BG_ARG_KIND_SHARED_CHANNEL: u64 = 2;
/// Ownership left the task while it ran — the payload was sent into a channel (whose buffer,
/// receiver, or teardown glue now owns it) or returned through a task handle. Written by the
/// runtime at that hand-off, never by codegen; the drop ladder skips the slot. Without this the
/// same pointer is owned twice (ladder + channel) and freed under the receiver's feet.
pub const BG_ARG_KIND_RELEASED: u64 = 3;
/// The task's counted reference to an Auto-Arc shared shape block (`ynz_arc_clone` at the
/// spawn site, v0.3-M8 Phase 5 — `IMP-ownership.md` "Auto-Arc — Sharing Topology Across
/// `background` Boundaries", topology (B)): released with `ynz_arc_free(ptr, size)`; the LAST
/// release frees the block. `size` is the shape's ABI byte size (the `ynz_arc_new` size).
pub const BG_ARG_KIND_ARC_SHAPE: u64 = 4;

/// Every `BG_ARG_KIND_*` value, for the runtime's per-kind alloc/free parity test that links
/// [`bg_arg_kind_is_releasable_payload`] to the drop ladder's free match: a kind added here
/// without a ladder arm (or the reverse) fails that test rather than leaking or double-freeing.
pub const ALL_BG_ARG_KINDS: &[u64] = &[
    BG_ARG_KIND_HEAP_SHAPE,
    BG_ARG_KIND_HEAP_ARRAY,
    BG_ARG_KIND_SHARED_CHANNEL,
    BG_ARG_KIND_RELEASED,
    BG_ARG_KIND_ARC_SHAPE,
];

/// Is a ladder slot of this kind a heap PAYLOAD the task can hand off (send into a channel,
/// return through its handle) — i.e. one `release_ladder_payload` may rewrite to
/// [`BG_ARG_KIND_RELEASED`]? Defined by INVERSION so a new heap kind is releasable by default
/// rather than silently skipped (v0.3-M8 Phase 4; the previous hand-listed
/// `HEAP_SHAPE`/`HEAP_ARRAY` filter would have missed a future `HEAP_MAP` and reopened the
/// spawn-arg use-after-free door). A shared-channel slot is the task's own refcount, never a
/// channel element (typeck rejects `channel<channel<T>>`); a released slot is terminal.
///
/// An Auto-Arc slot ([`BG_ARG_KIND_ARC_SHAPE`]) is NOT releasable — decided v0.3-M8 Phase 5
/// (the plan's packet item (h)), with this reasoning: the slot holds one COUNT on a block
/// other tasks also count, not a payload the task owns outright. Today no hand-off can ever
/// match it (shapes are not channel elements, and a shape is returned by value, so the data
/// pointer never leaves the task). If a future hand-off DID match, the two possible mistakes
/// are asymmetric: skipping the ladder's release leaks ONE count (the block is never freed —
/// a bounded leak the alloc/free parity gate reports); releasing it while a receiver still
/// reads the block is a use-after-free. `false` picks the leak side by construction, and the
/// runtime's per-kind parity test pins the answer.
pub const fn bg_arg_kind_is_releasable_payload(kind: u64) -> bool {
    kind != BG_ARG_KIND_SHARED_CHANNEL
        && kind != BG_ARG_KIND_RELEASED
        && kind != BG_ARG_KIND_ARC_SHAPE
}

#[cfg(test)]
mod bg_arg_kind_tests {
    use super::*;

    /// `ALL_BG_ARG_KINDS` is a hand list; this holds it to the crate's own `BG_ARG_KIND_*`
    /// constants by reading this source file, so a kind added above without being listed (or
    /// the reverse) fails here instead of slipping past the runtime's per-kind parity test.
    #[test]
    fn every_bg_arg_kind_const_is_in_all_bg_arg_kinds() {
        let src = include_str!("lib.rs");
        let mut declared: Vec<(String, u64)> = Vec::new();
        for line in src.lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub const BG_ARG_KIND_") else {
                continue;
            };
            let Some((name, value)) = rest.split_once(": u64 = ") else {
                continue;
            };
            let value: u64 = value
                .trim_end_matches(';')
                .trim()
                .parse()
                .unwrap_or_else(|_| panic!("BG_ARG_KIND_{name}: not an integer literal"));
            declared.push((name.to_string(), value));
        }
        assert!(
            !declared.is_empty(),
            "found no `pub const BG_ARG_KIND_*: u64 = N;` lines — the parser drifted from the source"
        );
        // v0.3-M8 Phase 4 fix round 3, should-fix 6: the parse above requires the ENTIRE
        // `pub const BG_ARG_KIND_NAME: u64 = N;` on one line — a multi-line declaration (the
        // value wrapped to its own line) would silently disappear from `declared` instead of
        // failing loudly. Count lines whose trimmed text STARTS WITH the declaration marker —
        // that survives a wrapped value (the marker itself is still line-initial) while never
        // matching this test's own doc comments or string literals (which start with `//` or
        // `"`, never `pub const`) — and hold it to the same count.
        let marker_count = src
            .lines()
            .filter(|l| l.trim_start().starts_with("pub const BG_ARG_KIND_"))
            .count();
        assert_eq!(
            marker_count,
            declared.len(),
            "found {marker_count} `pub const BG_ARG_KIND_` declaration markers but the \
             line-based parser only extracted {} — a declaration spans multiple lines (or some \
             other shape this parser does not handle) and silently dropped out of the parity \
             check",
            declared.len()
        );
        for (name, value) in &declared {
            assert!(
                ALL_BG_ARG_KINDS.contains(value),
                "BG_ARG_KIND_{name} = {value} is declared but missing from ALL_BG_ARG_KINDS"
            );
        }
        assert_eq!(
            ALL_BG_ARG_KINDS.len(),
            declared.len(),
            "ALL_BG_ARG_KINDS lists {} kinds but {} BG_ARG_KIND_* constants are declared",
            ALL_BG_ARG_KINDS.len(),
            declared.len()
        );
        let mut sorted = ALL_BG_ARG_KINDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ALL_BG_ARG_KINDS.len(),
            "duplicate kind value in ALL_BG_ARG_KINDS"
        );
    }
}
