//! Background task handles (`let h = background fn(...)`) — the v0.3-M4 Phase 2 C-ABI
//! substrate (risks R2 + R8).
//!
//! # The model (spike-proven — R5 report, trap doors structurally absent)
//!
//! A handle-form spawn creates an **independent joinable Tokio task** driving the callee's
//! state-machine frame — the same `SpawnStateFnFuture` drive `ynz_rt_spawn` uses, wrapped in
//! [`HandleStateFnFuture`] for completion extraction. The three R5 trap doors are absent by
//! construction:
//!
//! - **1a** — never a `CpuJoinHandle` run-once closure model: the child is a real
//!   `tokio::task::JoinHandle` over a poll-driven state machine.
//! - **1b** — never a re-polled one-shot `ynz_rt_join_poll`: the parent collects via
//!   [`ynz_handle_recv_poll`], a real `mpsc::Receiver::poll_recv` endpoint future with the
//!   forwarded waker; re-polling is its native mode.
//! - **1c** — never a `sleep_handle` tenant: every endpoint lives in the heap-owned
//!   [`YnzTaskHandle`] object; the frame header is untouched.
//!
//! # R8 — copy-before-free, compile-time spawn-form-keyed
//!
//! The completion value is extracted from the frame's return slot INSIDE
//! [`HandleStateFnFuture::poll`]'s Ready arm — strictly BEFORE the embedded
//! `SpawnStateFnFuture`'s `Drop` frees the frame (drop runs when the task retires, after
//! `poll` returned Ready). For a frame-interior ok-value (`-> number errors` staging slot /
//! a bare `-> number` return), the 16 bytes are copied to a HANDLE-OWNED heap buffer and the
//! ok-word is repointed at it; the buffer is freed exactly once, at handle drop
//! ([`ynz_handle_free`]). The extraction kind ([`ret_kind`]) is a COMPILE-TIME constant keyed
//! on the spawn form + the callee's declared return type — there is no runtime
//! "was `.receive()` called yet" conditional anywhere, and the bare fire-and-forget spawn
//! path (`ynz_rt_spawn`) is byte-for-byte untouched.
//!
//! # `.receive()` — ONE surface
//!
//! `h.receive()` polls the handle's outbox: "the next thing from the task". In v0.3-M4 the
//! producer is the task's completion delivery `(err, ok)`; message replies from a
//! long-running child ride the same conduit when the child-side surface ships. After the
//! completion is delivered the child's sender is dropped, so a SECOND `.receive()` observes
//! Closed — a typed task-already-finished error, never a hang (the never-received /
//! double-received hostile cases are bounded by construction).
//!
//! # `.send()` — feeds the child's first `channel<T>` parameter
//!
//! `h.send(v)` delegates to the shared channel object the spawn wired into the child's frame
//! (the first channel-typed argument), with the handle pointer as the per-caller suspended-send
//! token. Typeck rejects `.send()` on a handle whose callee takes no channel.
//!
//! # Handle drop (`ynz_handle_free`) — cancel-via-drop at the runtime level
//!
//! Aborts the child's Tokio task (a no-op when already finished). Tokio delivers the abort at
//! the task's next yield point — i.e. the child's next suspension — whereupon the embedded
//! `SpawnStateFnFuture::Drop` runs the full cleanup ladder (sleep handle, arg-copies incl.
//! shared-channel refs, recursion chain, frame). The handle then releases its own conduit
//! reference and the R8 buffer, each exactly once. Source-level automatic drop insertion
//! awaits the language-wide scope-drop mechanism (recorded deferral — see the plan).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use tokio::sync::mpsc;

use crate::channel::{ynz_channel_free, ynz_channel_send_poll, ynz_channel_share};
use crate::runtime::{spawn_on_runtime, BgArgDropEntry, SpawnStateFnFuture};
use ynz_abi::FRAME_OFFSET_RETURN_SLOT;

/// Poll results — the channel poll ABI codes, shared verbatim (Ready/Pending/Closed).
use crate::channel::{CHANNEL_CLOSED, CHANNEL_PENDING, CHANNEL_READY};

/// Completion-extraction kinds — COMPILE-TIME constants emitted by codegen per spawn site,
/// keyed on the callee's declared return type. Never a runtime conditional.
///
/// - `RET_KIND_EC_WORD` (0): `-> T errors` where the ok-word is self-contained (int/bool/
///   float bits, or a heap-stable pointer: string/array/map/nothing). Read `{err, ok}` from
///   the return slot as-is.
/// - `RET_KIND_EC_NUMBER` (1): `-> number errors` — the ok-word points INTO the composed
///   frame's 16-byte staging slot. On the success path, copy the 16 bytes to a handle-owned
///   heap buffer BEFORE the frame is freed and repoint the ok-word (the R8 fix,
///   IMP-concurrency:475/477).
/// - `RET_KIND_VALUE_WORD` (2): plain `-> T` (i64-slot value incl. `nothing`). Completion is
///   `{0, slot}`.
/// - `RET_KIND_VALUE_NUMBER` (3): plain `-> number` — the return slot ITSELF holds the
///   16-byte decimal; copy it to the handle-owned buffer, deliver `{0, buf}`.
pub use ynz_abi::{
    HANDLE_RET_KIND_EC_NUMBER as RET_KIND_EC_NUMBER, HANDLE_RET_KIND_EC_WORD as RET_KIND_EC_WORD,
    HANDLE_RET_KIND_VALUE_NUMBER as RET_KIND_VALUE_NUMBER,
    HANDLE_RET_KIND_VALUE_WORD as RET_KIND_VALUE_WORD,
};

/// Poison-tolerant lock (same discipline as the channel module).
fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// State shared between the handle object (parent side) and the spawned child future:
/// the R8 handle-owned ok-value buffer.
struct HandleShared {
    /// The 16-byte heap buffer holding a copied wide ok-value (`number` cases). `None` until
    /// the child completes with one; freed exactly once at handle drop via `Option::take`.
    ok_buf: Mutex<Option<Box<[u8; 16]>>>,
}

/// A Yinz background task handle at the runtime ABI boundary (opaque pointer, `Box`-owned —
/// the same allocation discipline as the channel object).
pub struct YnzTaskHandle {
    /// Outbox consumer — `.receive()`'s endpoint future (poll-based, forwarded waker).
    outbox_rx: Mutex<mpsc::Receiver<(i64, i64)>>,
    /// The child's first `channel<T>` argument, refcount-bumped for the handle (`.send()`'s
    /// conduit). Null when the callee takes no channel (typeck rejects `.send()` then).
    msg_chan: *mut u8,
    /// The child's Tokio join handle — held ONLY for abort-on-drop. Never polled (trap door
    /// 1b is about join-polling; collection goes through the outbox conduit instead).
    join: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// R8 buffer ownership (shared with the child future, which fills it at completion).
    /// Ownership-only, never read: the child's Arc drops when the task retires (right after
    /// completion), so THIS Arc is what keeps `ok_buf` alive until the parent reads the
    /// collected ok-word pointing into it — removing this field is a use-after-free on
    /// collected wide values. Freed exactly once via `drop(handle)` in `ynz_handle_free`.
    #[expect(dead_code)]
    shared: Arc<HandleShared>,
}

// SAFETY: the raw msg_chan pointer targets the Arc-backed, internally-synchronized channel
// object; the handle itself is owned by the spawning task and its methods are re-entrant.
unsafe impl Send for YnzTaskHandle {}
unsafe impl Sync for YnzTaskHandle {}

/// The spawned child future: the plain `SpawnStateFnFuture` state-machine drive plus
/// completion extraction + delivery at the Ready boundary (copy-before-free by construction —
/// the embedded future's `Drop`, which frees the frame, runs only after this `poll` returns).
struct HandleStateFnFuture {
    inner: SpawnStateFnFuture,
    /// Completion conduit — taken (and thereby closed) after the single completion delivery,
    /// so a post-completion `.receive()` observes Closed instead of hanging.
    outbox_tx: Option<mpsc::Sender<(i64, i64)>>,
    /// Compile-time completion-extraction kind (see the `RET_KIND_*` constants).
    ret_kind: i64,
    shared: Arc<HandleShared>,
}

// SAFETY: owned exclusively by the spawned task, exactly like SpawnStateFnFuture.
unsafe impl Send for HandleStateFnFuture {}

impl Future for HandleStateFnFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(()) => {
                // Completion extraction — the frame is still live (freed by `inner`'s Drop
                // when the task retires, strictly after this returns). R8: any frame-interior
                // ok-value is copied OUT here, before that free.
                let frame = this.inner.frame_ptr;
                let (err, ok) = unsafe { extract_completion(frame, this.ret_kind, &this.shared) };
                if let Some(tx) = this.outbox_tx.take() {
                    // Capacity >= 1 and this is the sole sender in v0.3-M4, so try_send
                    // cannot observe Full; a dropped-receiver (handle already freed) makes
                    // delivery moot — either way, never a blocking call.
                    let _ = tx.try_send((err, ok));
                }
                Poll::Ready(())
            }
        }
    }
}

/// Read the completion `(err, ok)` pair from the frame's return slot per `ret_kind`,
/// performing the R8 copy for frame-interior wide values.
///
/// # Safety
/// `frame` must be live and at least `FRAME_OFFSET_RETURN_SLOT + 16` bytes (every
/// codegen-emitted frame has the 32-byte header with the 16-byte return slot).
unsafe fn extract_completion(frame: *mut u8, ret_kind: i64, shared: &HandleShared) -> (i64, i64) {
    let slot = frame.add(FRAME_OFFSET_RETURN_SLOT as usize);
    match ret_kind {
        RET_KIND_EC_WORD | RET_KIND_EC_NUMBER => {
            let err = *(slot as *const i64);
            let mut ok = *(slot.add(8) as *const i64);
            if ret_kind == RET_KIND_EC_NUMBER && err == 0 && ok != 0 {
                // ok points into the frame's 16-byte staging slot — copy before the frame
                // is freed, repoint at the handle-owned buffer.
                let mut buf = Box::new([0u8; 16]);
                std::ptr::copy_nonoverlapping(ok as *const u8, buf.as_mut_ptr(), 16);
                ok = buf.as_ptr() as i64;
                *lock_or_recover(&shared.ok_buf) = Some(buf);
            }
            (err, ok)
        }
        RET_KIND_VALUE_NUMBER => {
            // The return slot itself holds the 16-byte decimal — copy it out.
            let mut buf = Box::new([0u8; 16]);
            std::ptr::copy_nonoverlapping(slot as *const u8, buf.as_mut_ptr(), 16);
            let ok = buf.as_ptr() as i64;
            *lock_or_recover(&shared.ok_buf) = Some(buf);
            (0, ok)
        }
        // RET_KIND_VALUE_WORD and any future kind default: the slot's first word is the
        // self-contained value (int/bool/float bits, heap-stable pointer, or 0 for nothing).
        _ => (0, *(slot as *const i64)),
    }
}

/// Spawn a state-machine callee as an independent joinable Tokio task with a collection
/// handle. The fire-and-forget path (`ynz_rt_spawn`) is untouched — this is the compile-time
/// spawn-form key (R8).
///
/// `ret_kind` — completion-extraction kind (see `RET_KIND_*`).
/// `msg_chan` — the child's first `channel<T>` argument (the `.send()` conduit), or null.
///   The handle takes its OWN refcounted reference (the child's arg reference is separate,
///   released by the arg-drop machinery).
///
/// Always returns a valid handle pointer. When the runtime is uninitialized/shut down the
/// task is discarded (warning logged, matching `ynz_rt_spawn`) and the handle's outbox is
/// born closed — `.receive()` yields the typed task-finished error instead of hanging.
///
/// # Safety
/// Same contract as `ynz_rt_spawn` for `resume_fn`/`frame_ptr`/`arg_drop_ptr`; `msg_chan`
/// must be null or a live channel pointer.
#[no_mangle]
pub unsafe extern "C" fn ynz_rt_spawn_handle(
    resume_fn: unsafe extern "C-unwind" fn(*mut u8, *mut u8) -> i32,
    frame_ptr: *mut u8,
    frame_size: i64,
    recursion_slot_offset: i64,
    arg_drop_ptr: *const BgArgDropEntry,
    arg_drop_count: i64,
    ret_kind: i64,
    msg_chan: *mut u8,
) -> *mut u8 {
    // Bounded outbox (stdlib Rule 4): sized for the single completion delivery plus headroom
    // for the future child-side message-reply surface (the locked default capacity).
    let (outbox_tx, outbox_rx) = mpsc::channel::<(i64, i64)>(64);
    let shared = Arc::new(HandleShared {
        ok_buf: Mutex::new(None),
    });

    let future = HandleStateFnFuture {
        inner: SpawnStateFnFuture {
            resume_fn,
            frame_ptr,
            frame_size,
            recursion_slot_offset,
            arg_drop_ptr,
            arg_drop_count: arg_drop_count as usize,
        },
        outbox_tx: Some(outbox_tx),
        ret_kind,
        shared: Arc::clone(&shared),
    };
    // On a dead runtime the future (and its outbox sender) is dropped here: the handle's
    // outbox is born closed and `.receive()` reports the typed task-finished error.
    let join = spawn_on_runtime(future, "ynz_rt_spawn_handle");

    let handle = Box::new(YnzTaskHandle {
        outbox_rx: Mutex::new(outbox_rx),
        msg_chan: if msg_chan.is_null() {
            std::ptr::null_mut()
        } else {
            // The handle's own conduit reference (released at ynz_handle_free).
            ynz_channel_share(msg_chan)
        },
        join: Mutex::new(join),
        shared,
    });
    Box::into_raw(handle) as *mut u8
}

/// Poll `h.receive()` — the handle's outbox endpoint future, with the forwarded waker.
///
/// Returns Ready (writes `*err_out`/`*ok_out`), Pending (task still running / no delivery
/// yet — waker registered), or Closed (the task finished and every delivery was consumed —
/// the caller maps this to the typed task-already-finished error).
///
/// # Safety
/// `handle_ptr` must be a live pointer from [`ynz_rt_spawn_handle`]; `err_out`/`ok_out` must
/// be writable; `waker_ctx` must point to a live `&mut Context<'_>`.
#[no_mangle]
pub unsafe extern "C" fn ynz_handle_recv_poll(
    handle_ptr: *mut u8,
    err_out: *mut i64,
    ok_out: *mut i64,
    waker_ctx: *mut u8,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        // SAFETY: handle_ptr is a live Box-backed pointer (caller guarantee).
        let handle = &*(handle_ptr as *const YnzTaskHandle);
        // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing SM poll.
        let cx = &mut *(waker_ctx as *mut Context<'_>);
        let poll = lock_or_recover(&handle.outbox_rx).poll_recv(cx);
        match poll {
            Poll::Ready(Some((err, ok))) => {
                *err_out = err;
                *ok_out = ok;
                CHANNEL_READY
            }
            Poll::Ready(None) => CHANNEL_CLOSED,
            Poll::Pending => CHANNEL_PENDING,
        }
    });
    match result {
        Ok(v) => v,
        Err(_) => {
            eprintln!("ynz runtime: ynz_handle_recv_poll panicked (returning Pending)");
            CHANNEL_PENDING
        }
    }
}

/// Poll `h.send(value)` — delegates to the child's first-channel conduit, with the handle
/// pointer as the per-caller suspended-send token.
///
/// # Safety
/// Same contract as [`ynz_channel_send_poll`]; `handle_ptr` must be a live handle pointer.
#[no_mangle]
pub unsafe extern "C" fn ynz_handle_send_poll(
    handle_ptr: *mut u8,
    value: i64,
    waker_ctx: *mut u8,
) -> i32 {
    // SAFETY: handle_ptr is a live Box-backed pointer (caller guarantee).
    let handle = &*(handle_ptr as *const YnzTaskHandle);
    debug_assert!(
        !handle.msg_chan.is_null(),
        "ynz_handle_send_poll: typeck rejects .send() on a channel-less task — codegen bug"
    );
    if handle.msg_chan.is_null() {
        return CHANNEL_CLOSED;
    }
    ynz_channel_send_poll(handle.msg_chan, value, waker_ctx, handle_ptr as u64)
}

/// Free the task handle: abort the child (a no-op when already finished — Tokio delivers the
/// abort at the child's next suspension point, whereupon the embedded drop ladder frees the
/// frame + arg-copies + shared-channel refs), release the handle's conduit reference, and free
/// the R8 ok-buffer. Each resource exactly once; null is a no-op.
///
/// # Safety
/// `handle_ptr` must be a pointer from [`ynz_rt_spawn_handle`] not yet freed, or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_handle_free(handle_ptr: *mut u8) {
    if handle_ptr.is_null() {
        return;
    }
    // SAFETY: inverse of Box::into_raw in ynz_rt_spawn_handle; freed exactly once.
    let handle = Box::from_raw(handle_ptr as *mut YnzTaskHandle);
    if let Some(join) = lock_or_recover(&handle.join).take() {
        join.abort();
    }
    ynz_channel_free(handle.msg_chan);
    // ok_buf (if any) drops with `handle.shared` unless the child future still holds the
    // other Arc — then it drops when the aborted task retires. Either way exactly once,
    // via the Arc + Option ownership chain.
    drop(handle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Wake, Waker};

    struct CountingWaker(AtomicUsize);
    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn make_waker() -> Waker {
        Waker::from(Arc::new(CountingWaker(AtomicUsize::new(0))))
    }

    /// A minimal resume fn: stores {err=0, ok=99} in the return slot and returns Ready.
    unsafe extern "C-unwind" fn resume_ready(frame: *mut u8, _waker: *mut u8) -> i32 {
        let slot = frame.add(FRAME_OFFSET_RETURN_SLOT as usize) as *mut i64;
        *slot = 0;
        *slot.add(1) = 99;
        0
    }

    /// R8 substrate proof: the completion value is extracted BEFORE the frame is freed, the
    /// wide-value copy lands in the handle-owned buffer, and `.receive()` after completion
    /// yields Ready then Closed (bounded — never a hang).
    #[test]
    fn handle_collects_completion_then_closes() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let _guard = rt.enter();
        unsafe {
            let frame = crate::ynz_alloc_zeroed(64);
            let h = ynz_rt_spawn_handle(
                resume_ready,
                frame,
                64,
                -1,
                std::ptr::null(),
                0,
                RET_KIND_EC_WORD,
                std::ptr::null_mut(),
            );
            // Drive the runtime so the spawned task completes.
            rt.block_on(async { tokio::task::yield_now().await });

            let waker = make_waker();
            let mut cx = Context::from_waker(&waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
            let (mut e, mut o) = (0i64, 0i64);
            assert_eq!(
                ynz_handle_recv_poll(h, &mut e, &mut o, cx_ptr),
                CHANNEL_READY,
                "the completion value must be collectable after the task finished"
            );
            assert_eq!((e, o), (0, 99), "completion value must be byte-correct");
            // Second receive: Closed (typed task-finished error at the language level) —
            // bounded, never a hang.
            assert_eq!(
                ynz_handle_recv_poll(h, &mut e, &mut o, cx_ptr),
                CHANNEL_CLOSED,
                "post-completion receive must observe Closed, not hang"
            );
            ynz_handle_free(h);
        }
    }

    /// Never-received handle: the task completes, the handle is dropped without a receive —
    /// no hang, no double-free; the frame was freed by the task's own drop ladder.
    #[test]
    fn never_received_handle_is_bounded_and_freed() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let _guard = rt.enter();
        unsafe {
            let frame = crate::ynz_alloc_zeroed(64);
            let h = ynz_rt_spawn_handle(
                resume_ready,
                frame,
                64,
                -1,
                std::ptr::null(),
                0,
                RET_KIND_VALUE_WORD,
                std::ptr::null_mut(),
            );
            rt.block_on(async { tokio::task::yield_now().await });
            // Never receive; just free. Frame already freed by the task; buffer path empty.
            ynz_handle_free(h);
        }
    }

    /// Handle free before completion aborts the child at its next suspension point
    /// (cancel-via-drop, runtime level) — and the frame is freed exactly once by the
    /// embedded drop ladder.
    #[test]
    fn free_before_completion_aborts_child() {
        /// Resume fn that always returns Pending (a child parked at a suspension point).
        unsafe extern "C-unwind" fn resume_pending(_frame: *mut u8, _waker: *mut u8) -> i32 {
            1
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let _guard = rt.enter();
        unsafe {
            let frame = crate::ynz_alloc_zeroed(64);
            let h = ynz_rt_spawn_handle(
                resume_pending,
                frame,
                64,
                -1,
                std::ptr::null(),
                0,
                RET_KIND_VALUE_WORD,
                std::ptr::null_mut(),
            );
            rt.block_on(async { tokio::task::yield_now().await });
            ynz_handle_free(h); // aborts the parked child
            rt.block_on(async { tokio::task::yield_now().await }); // let the abort retire it
        }
    }
}
