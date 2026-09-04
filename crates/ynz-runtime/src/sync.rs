//! Loom-swappable synchronization primitives (v0.3-M8 Phase 3) — the ONE place the runtime's
//! channel/handle modules import `Arc`/`Mutex`/`MutexGuard` from.
//!
//! # Production path is a pure re-export
//!
//! Without `--cfg loom` (every `cargo build`/`cargo test`/`cargo build --release` — the only
//! builds that ever produce `libynz_runtime.a`), this module is a `pub(crate) use` of the exact
//! `std::sync` items the code used before it existed: no wrapper type, no newtype, no trait —
//! so the generated code is byte-identical to the pre-shim crate (verified by an LLVM-IR diff
//! at introduction; the R3 no-op requirement in the M8 plan). A new contributor reading
//! `crate::sync::Mutex` should read it as `std::sync::Mutex` — because it IS.
//!
//! # Loom path
//!
//! Under `RUSTFLAGS='--cfg loom'` the same names resolve to `loom::sync::*`, whose API is a
//! drop-in subset of std's (`Mutex::{lock, try_lock, get_mut}`, `Arc::{into_raw, from_raw,
//! increment_strong_count, strong_count}`), and `channel::CURRENT_DRIVE` is declared through
//! `loom::thread_local!` (per-model-thread storage — a std thread-local would be shared across
//! every loom thread, since loom runs them all on one OS thread). Loom then owns every
//! preemption point in the runtime-owned synchronization state — `pending_sends`,
//! `recv_waiters`, the channel refcount, the published drive identity — and exhaustively
//! schedules the loom tests in `crate::loom_tests`.
//!
//! # Explicit boundary
//!
//! Tokio's own `mpsc`/scheduler internals are NOT swapped: tokio gates its loom paths on
//! `cfg(all(test, loom))`, which is never true for a dependency, so under our `--cfg loom`
//! tokio stays a std black box that loom treats as one atomic step per call. The harness
//! model-checks what ynz-runtime owns, never what tokio owns.
//!
//! Deliberately NOT swapped: `channel::CALLER_GENERATION` (a `static` monotonic mint counter —
//! loom atomics cannot live in a `static`, and the loom tests stamp generations explicitly, so
//! the counter's ordering is never part of a modeled interleaving) and `runtime.rs`'s Tokio
//! runtime lifecycle statics (`RUNTIME`, shutdown flags — the runtime is never started inside a
//! loom model).

#[cfg(not(loom))]
pub(crate) use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(loom)]
pub(crate) use loom::sync::{Arc, Mutex, MutexGuard};

// The one thread-local this swap touches (`channel::CURRENT_DRIVE`) is declared as two cfg
// variants at its own site rather than through a macro here: its production form uses the
// `const { .. }` initializer, which loom's `thread_local!` has no equivalent for, and a shim
// macro would have to special-case exactly that one declaration.
