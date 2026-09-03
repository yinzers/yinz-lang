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
//! (the first channel-typed argument), with the handle pointer + the handle's generation stamp
//! (`send_gen`, v0.3-M6 P3-1) as the per-caller suspended-send key. Typeck rejects `.send()`
//! on a handle whose callee takes no channel.
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
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use tokio::sync::mpsc;

use crate::channel::{
    channel_send_poll_guarded, lock_or_recover, next_caller_generation, purge_pending_sends,
    ynz_channel_free, ynz_channel_share,
};
use crate::runtime::{
    release_ladder_payload, spawn_on_runtime, BgArgDropEntry, SpawnStateFnFuture,
};
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
/// - `RET_KIND_VALUE_HEAP_PTR` (4) / `RET_KIND_EC_HEAP_PTR` (5): the `VALUE_WORD` /
///   `EC_WORD` twins for `-> array<T>` / `-> map<K, V>` (plain / `errors`). Extraction is
///   word-identical; the kind additionally RELEASES the delivered pointer from the child's
///   spawn-arg drop ladder (`release_ladder_payload`) — a child returning one of its own
///   heap-cloned arguments hands ownership to the parent, and the ladder must not free it.
pub use ynz_abi::{
    HANDLE_RET_KIND_EC_HEAP_PTR as RET_KIND_EC_HEAP_PTR,
    HANDLE_RET_KIND_EC_NUMBER as RET_KIND_EC_NUMBER, HANDLE_RET_KIND_EC_WORD as RET_KIND_EC_WORD,
    HANDLE_RET_KIND_VALUE_HEAP_PTR as RET_KIND_VALUE_HEAP_PTR,
    HANDLE_RET_KIND_VALUE_NUMBER as RET_KIND_VALUE_NUMBER,
    HANDLE_RET_KIND_VALUE_WORD as RET_KIND_VALUE_WORD,
};

/// State shared between the handle object (parent side) and the spawned child future:
/// the R8 handle-owned ok-value buffer + the receive-waiter registry (P2-7).
struct HandleShared {
    /// The 16-byte heap buffer holding a copied wide ok-value (`number` cases). `None` until
    /// the child completes with one; freed exactly once at handle drop via `Option::take`.
    ok_buf: Mutex<Option<Box<[u8; 16]>>>,
    /// Wakers of every task currently suspended on this handle's `.receive()` — the
    /// register-before-poll side registry (v0.3-M6 P2-7, Phase 4b), mirroring
    /// `YnzChannel::recv_waiters`. It lives on the SHARED state so the child future's
    /// completion delivery can wake a receiver whose mpsc single-slot registration was
    /// lost — the panic-then-Pending window: a panic inside [`ynz_handle_recv_poll`]'s
    /// body before `poll_recv` parks the waker returns Pending with an empty mpsc slot,
    /// and without this registry the completion's `try_send` wakes nobody, permanently.
    recv_waiters: Mutex<Vec<Waker>>,
}

impl HandleShared {
    /// Wake every recorded receive-waiter (the completion just landed). Mirrors
    /// `YnzChannel::wake_recv_waiters` — the one shared discipline, not a bespoke ordering.
    /// O(n) wakes where n = suspended receivers (typically 0 or 1).
    fn wake_recv_waiters(&self) {
        let mut waiters = lock_or_recover(&self.recv_waiters);
        for w in waiters.drain(..) {
            w.wake();
        }
    }

    /// Record `waker` as a receive-waiter (deduplicated via `will_wake`). Mirrors
    /// `YnzChannel::record_recv_waiter`.
    fn record_recv_waiter(&self, waker: &Waker) {
        let mut waiters = lock_or_recover(&self.recv_waiters);
        if !waiters.iter().any(|w| w.will_wake(waker)) {
            waiters.push(waker.clone());
        }
    }
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
    /// This handle's caller-generation stamp (v0.3-M6 P3-1), minted at spawn from the ONE
    /// global counter (`channel::next_caller_generation`) — the generation half of the
    /// `(handle_ptr, send_gen)` key `h.send()`'s suspended sends are parked under, and the
    /// generation `ynz_handle_free` purges. A DISTINCT identity from the child task's
    /// `task_gen`: the handle and the child die at different moments.
    send_gen: u64,
    /// Shared state with the child future: the receive-waiter registry (read on every
    /// [`ynz_handle_recv_poll`] — P2-7 register-before-poll) and R8 buffer ownership (the
    /// child's Arc drops when the task retires, so THIS Arc is what keeps `ok_buf` alive
    /// until the parent reads the collected ok-word pointing into it — removing this field
    /// is a use-after-free on collected wide values). Freed exactly once via `drop(handle)`
    /// in `ynz_handle_free`.
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
                // A delivered heap pointer now belongs to the parent (`h.receive()`). If it is
                // one of THIS child's heap-cloned spawn arguments, the child's drop ladder —
                // which runs when `inner` drops, right after this returns — must not free it.
                // Gated on the compile-time kind so an `int` completion is never compared.
                let delivers_heap_ptr = match this.ret_kind {
                    RET_KIND_VALUE_HEAP_PTR => true,
                    RET_KIND_EC_HEAP_PTR => err == 0,
                    _ => false,
                };
                if delivers_heap_ptr && ok != 0 {
                    // SAFETY: `inner` is this future's own drive; its frame and ladder are
                    // live (freed only by `inner`'s Drop) and exclusively ours during poll.
                    unsafe { release_ladder_payload(&this.inner.drive_identity(), ok) };
                }
                if let Some(tx) = this.outbox_tx.take() {
                    // Capacity >= 1 and this is the sole sender in v0.3-M4, so try_send
                    // cannot observe Full; a dropped-receiver (handle already freed) makes
                    // delivery moot — either way, never a blocking call.
                    let _ = tx.try_send((err, ok));
                }
                // Completion delivered — drain the receive-waiter registry (P2-7), the
                // producer-side half of register-before-poll: mpsc's native wake reaches
                // only its single slot registrant, which a panic inside the recv poll's
                // body can leave unregistered; this drain-all is what closes that hang.
                // Mirrors the channel send path's wake_recv_waiters-after-enqueue.
                this.shared.wake_recv_waiters();
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
        RET_KIND_EC_WORD | RET_KIND_EC_NUMBER | RET_KIND_EC_HEAP_PTR => {
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
        // RET_KIND_VALUE_WORD / RET_KIND_VALUE_HEAP_PTR and any future kind default: the
        // slot's first word is the self-contained value (int/bool/float bits, heap-stable
        // pointer, or 0 for nothing).
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
        recv_waiters: Mutex::new(Vec::new()),
    });

    let future = HandleStateFnFuture {
        inner: SpawnStateFnFuture {
            resume_fn,
            frame_ptr,
            frame_size,
            recursion_slot_offset,
            // Exclusively owned by this call (safety contract): kinds are rewritten as
            // payload ownership leaves the child (channel send / handle return).
            arg_drop_ptr: arg_drop_ptr as *mut BgArgDropEntry,
            arg_drop_count: arg_drop_count as usize,
            // The CHILD TASK's own caller identity (frame tokens; purged by its drop ladder).
            task_gen: next_caller_generation(),
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
        // The HANDLE's own caller identity (h.send() tokens; purged by ynz_handle_free) —
        // same ONE counter, distinct stamp from the child's task_gen above.
        send_gen: next_caller_generation(),
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
/// Register-before-poll ordering (v0.3-M6 P2-7, Phase 4b — the same discipline as
/// `ynz_channel_recv_poll`'s P3-2 fix): the waker is recorded in the handle's
/// `recv_waiters` registry BEFORE `poll_recv` runs, so a panic ANYWHERE in the body below
/// the record — surfaced as Pending through the catch_unwind — leaves the task wakeable:
/// the child's completion delivery drains the registry (`HandleStateFnFuture::poll`'s
/// Ready arm) even when the panic left mpsc's single slot unregistered. With the old
/// poll-first ordering, a pre-registration panic returned Pending with NO registered waker
/// anywhere, and the task never woke — a permanent hang. The `Ready(Some)` exit drains the
/// registration (a self-wake is a harmless spurious re-poll); the `Ready(None)`/Closed
/// exit returns a terminal answer, so its register-first entry is left recorded (dedup'd
/// per task via `will_wake`; freed with the handle) and wakes nobody — mirroring the
/// channel path's closed-exit convention.
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
        // Register BEFORE polling — the P2-7 panic-then-Pending fix; rationale in the fn
        // doc above. Locks are strictly sequential, never nested: recv_waiters (released
        // inside record) → outbox_rx (held only across the one non-blocking poll_recv, as
        // a statement temporary) → recv_waiters (Ready(Some) drain only, released).
        handle.shared.record_recv_waiter(cx.waker());
        let poll = lock_or_recover(&handle.outbox_rx).poll_recv(cx);
        match poll {
            Poll::Ready(Some((err, ok))) => {
                *err_out = err;
                *ok_out = ok;
                // Drain this call's own register-first entry (and wake any co-waiter —
                // a self-wake is a harmless spurious re-poll), mirroring the channel path.
                handle.shared.wake_recv_waiters();
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

/// Poll `h.send(value)` — delegates to the child's first-channel conduit, keyed by the handle
/// pointer + the handle's own generation stamp (`send_gen`), through the ONE keyed send core
/// (`channel_send_poll_guarded` — never a second core, per authoritative-derivation.md).
///
/// The generation is passed EXPLICITLY from the handle — never read from the poll thread-local,
/// which would be the SENDING task's generation; the ABA key needs the HANDLE's own birth stamp
/// (the identity `ynz_handle_free` purges).
///
/// # Safety
/// Same contract as [`crate::channel::ynz_channel_send_poll`]; `handle_ptr` must be a live
/// handle pointer.
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
    channel_send_poll_guarded(
        handle.msg_chan,
        value,
        waker_ctx,
        handle_ptr as u64,
        handle.send_gen,
    )
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
    // v0.3-M6 P2-2 (second producer): purge this handle's suspended h.send() entries BEFORE
    // releasing the conduit reference — the handle-keyed orphan is the same leak + ABA
    // precondition as the frame path, purged through the SAME shared helper. Idempotent:
    // no in-flight send (or a double-cancel) is a safe no-op.
    purge_pending_sends(handle.msg_chan, handle.send_gen);
    ynz_channel_free(handle.msg_chan);
    // ok_buf (if any) drops with `handle.shared` unless the child future still holds the
    // other Arc — then it drops when the aborted task retires. Either way exactly once,
    // via the Arc + Option ownership chain.
    drop(handle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

    /// v0.3-M6 P3-1 (handle producer, DETERMINISTIC ABA proof): the REAL
    /// `ynz_handle_send_poll` mint — keyed `(handle_ptr, send_gen)` through the one shared
    /// core — never collides across generations at the SAME handle address. Mirrors
    /// `channel::tests::same_token_different_generation_never_collides_and_stale_is_swept`
    /// at the handle seam: the purge is WITHHELD (the residual window between cancellation
    /// and purge completing), and address reuse is simulated deterministically by
    /// restamping the handle's `send_gen` in place with a fresh mint from the ONE counter —
    /// exactly the stamp a NEW handle born at the reused address would carry. A broken salt
    /// (key ignoring the generation) fails this test: the second send would re-poll the
    /// dead generation's stale entry and deliver its 111 instead of the live 222.
    #[test]
    fn handle_send_same_address_different_generation_never_collides() {
        /// Resume fn that always returns Pending (a child parked at a suspension point).
        unsafe extern "C-unwind" fn resume_pending(_frame: *mut u8, _waker: *mut u8) -> i32 {
            1
        }
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt");
        let _guard = rt.enter();
        let waker = make_waker();
        let mut cx = Context::from_waker(&waker);
        let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
        unsafe {
            let chan = crate::channel::ynz_channel_create(1, std::ptr::null_mut());
            // Fill the single slot via the bare ABI (Ready path — no pending entry).
            assert_eq!(
                crate::channel::ynz_channel_send_poll(chan, 42, cx_ptr, 0xF),
                CHANNEL_READY
            );

            // "Handle A": h.send(111) suspends on the full conduit under A's send_gen.
            let frame = crate::ynz_alloc_zeroed(64);
            let h = ynz_rt_spawn_handle(
                resume_pending,
                frame,
                64,
                -1,
                std::ptr::null(),
                0,
                RET_KIND_VALUE_WORD,
                chan,
            );
            assert_eq!(ynz_handle_send_poll(h, 111, cx_ptr), CHANNEL_PENDING);
            assert_eq!(crate::channel::pending_send_count(chan), 1);

            // Handle A "dies" WITHOUT its purge running (the purge race window), and
            // "handle B" is born at the SAME reused address: same token, fresh generation
            // from the one counter — restamped in place to make the reuse deterministic.
            let gen_a = (*(h as *mut YnzTaskHandle)).send_gen;
            let gen_b = next_caller_generation();
            assert_ne!(gen_a, gen_b, "every birth mints a distinct generation");
            (*(h as *mut YnzTaskHandle)).send_gen = gen_b;

            // B's send must NOT match A's stale entry (key differs by generation); the
            // insert-time sweep removes the dead same-token/different-generation entry,
            // so the count stays 1 — the stale 111 future (and its value) is dropped.
            assert_eq!(ynz_handle_send_poll(h, 222, cx_ptr), CHANNEL_PENDING);
            assert_eq!(
                crate::channel::pending_send_count(chan),
                1,
                "the dead generation's same-token entry must be swept on insert \
                 (missed-path leak backstop)"
            );

            // Drain the prefill; re-poll B: ITS value must deliver — never the dead
            // generation's stale 111.
            let mut out = 0i64;
            assert_eq!(
                crate::channel::ynz_channel_recv_poll(chan, &mut out, cx_ptr),
                CHANNEL_READY
            );
            assert_eq!(out, 42);
            assert_eq!(ynz_handle_send_poll(h, 222, cx_ptr), CHANNEL_READY);
            assert_eq!(
                crate::channel::ynz_channel_recv_poll(chan, &mut out, cx_ptr),
                CHANNEL_READY
            );
            assert_eq!(
                out, 222,
                "the NEW generation's value must deliver through the handle path; the \
                 dead generation's stale suspended send must never resurface (handle ABA)"
            );
            assert_eq!(crate::channel::pending_send_count(chan), 0);

            ynz_handle_free(h); // purges gen_b (already resolved — safe no-op), aborts child
            rt.block_on(async { tokio::task::yield_now().await }); // retire the aborted child
            crate::channel::ynz_channel_free(chan);
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

    /// v0.3-M6 P2-7 (Phase 4b) probe — mirrors
    /// `channel::tests::recv_poll_registers_waiter_before_polling`'s `OrderProbe` at the
    /// handle seam. `ynz_handle_recv_poll` clones the waker at exactly two sites:
    /// `record_recv_waiter`'s push (recv_waiters mutex HELD → `try_lock` fails) and
    /// `poll_recv`'s mpsc single-slot registration (recv_waiters free → `try_lock`
    /// succeeds). At the mpsc-site clone, the fix's invariant is that the waker is
    /// ALREADY recorded. With `panic_at_mpsc_clone` armed, the clone hook panics AT the
    /// mpsc slot registration — a real panic inside the poll body, in the exact
    /// pre-slot-registration window P2-7 names (the slot ends the call empty).
    struct HandleOrderProbe {
        shared: *const HandleShared,
        /// At the mpsc-slot registration, was the waker already in `recv_waiters`?
        registered_before_poll: AtomicBool,
        /// Vacuity guard: the probe actually observed the mpsc-slot clone.
        mpsc_clone_seen: AtomicBool,
        /// Armed ⇒ the mpsc-slot clone panics (the panic-then-Pending repro).
        panic_at_mpsc_clone: AtomicBool,
        wakes: AtomicUsize,
    }

    const HANDLE_ORDER_PROBE_VTABLE: std::task::RawWakerVTable = std::task::RawWakerVTable::new(
        handle_order_probe_clone,
        handle_order_probe_wake,
        handle_order_probe_wake_by_ref,
        handle_order_probe_drop,
    );

    unsafe fn handle_order_probe_clone(data: *const ()) -> std::task::RawWaker {
        let st = &*(data as *const HandleOrderProbe);
        let shared = &*st.shared;
        // recv_waiters held ⇒ this is record_recv_waiter's own push-clone: skip.
        // recv_waiters free ⇒ this is poll_recv's mpsc slot-registration clone: inspect.
        if let Ok(waiters) = shared.recv_waiters.try_lock() {
            st.mpsc_clone_seen.store(true, Ordering::SeqCst);
            st.registered_before_poll
                .store(!waiters.is_empty(), Ordering::SeqCst);
            drop(waiters);
            if st.panic_at_mpsc_clone.load(Ordering::SeqCst) {
                panic!("injected: panic before the mpsc slot registration completes (P2-7)");
            }
        }
        std::task::RawWaker::new(data, &HANDLE_ORDER_PROBE_VTABLE)
    }
    unsafe fn handle_order_probe_wake(data: *const ()) {
        (*(data as *const HandleOrderProbe))
            .wakes
            .fetch_add(1, Ordering::SeqCst);
    }
    unsafe fn handle_order_probe_wake_by_ref(data: *const ()) {
        (*(data as *const HandleOrderProbe))
            .wakes
            .fetch_add(1, Ordering::SeqCst);
    }
    unsafe fn handle_order_probe_drop(_data: *const ()) {}

    /// v0.3-M6 P2-7 (Phase 4b) RED→GREEN, ordering half: the caller's waker must be
    /// recorded in the handle's `recv_waiters` BEFORE `poll_recv` runs — same invariant
    /// as `channel::tests::recv_poll_registers_waiter_before_polling`, at the handle
    /// seam. Register-first is what makes a panic anywhere after the record safe: the
    /// catch_unwind's Pending then rides on an already-recorded waker.
    #[test]
    fn handle_recv_poll_registers_waiter_before_polling() {
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
            // The child is scheduled but NOT yet driven — the outbox is empty, so the
            // parent's first poll takes the Pending path (the registration under test).
            let probe = HandleOrderProbe {
                shared: Arc::as_ptr(&(*(h as *const YnzTaskHandle)).shared),
                registered_before_poll: AtomicBool::new(false),
                mpsc_clone_seen: AtomicBool::new(false),
                panic_at_mpsc_clone: AtomicBool::new(false),
                wakes: AtomicUsize::new(0),
            };
            let probe_waker = Waker::from_raw(std::task::RawWaker::new(
                &probe as *const HandleOrderProbe as *const (),
                &HANDLE_ORDER_PROBE_VTABLE,
            ));
            let mut cx = Context::from_waker(&probe_waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
            let (mut e, mut o) = (0i64, 0i64);
            assert_eq!(
                ynz_handle_recv_poll(h, &mut e, &mut o, cx_ptr),
                CHANNEL_PENDING
            );
            assert!(
                probe.mpsc_clone_seen.load(Ordering::SeqCst),
                "probe never observed the mpsc slot registration — vacuous run"
            );
            assert!(
                probe.registered_before_poll.load(Ordering::SeqCst),
                "the waker must be recorded as a receive-waiter BEFORE poll_recv runs; \
                 poll-then-nothing leaves the panic path returning Pending with an \
                 unregistered waker and the task hangs permanently (P2-7)"
            );

            // Semantic follow-through: the child's completion must wake the recorded waiter.
            rt.block_on(async { tokio::task::yield_now().await });
            assert!(
                probe.wakes.load(Ordering::SeqCst) > 0,
                "completion delivery must wake the suspended receiver"
            );
            assert_eq!(
                ynz_handle_recv_poll(h, &mut e, &mut o, cx_ptr),
                CHANNEL_READY
            );
            assert_eq!((e, o), (0, 99));
            ynz_handle_free(h);
        }
    }

    /// v0.3-M6 P2-7 (Phase 4b) RED→GREEN, behavioral half — the literal
    /// panic-then-Pending hang: a panic fires inside `ynz_handle_recv_poll`'s body AT the
    /// mpsc slot registration (so the slot ends the call EMPTY — the panic-fires-before-
    /// waker-registration window), the catch_unwind returns Pending, and the child then
    /// completes. Pre-fix the completion's `try_send` had no registered waker to wake and
    /// no side registry existed → the parent was never woken (the hang, observed here as
    /// wakes == 0). Post-fix the register-first record is already in `recv_waiters` when
    /// the panic fires, and the completion delivery's drain-all wakes it; the re-poll
    /// then collects the completion value — through the panic-poisoned (and
    /// `lock_or_recover`-recovered) outbox mutex.
    #[test]
    fn completion_wakes_receiver_after_panic_before_slot_registration() {
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
            let probe = HandleOrderProbe {
                shared: Arc::as_ptr(&(*(h as *const YnzTaskHandle)).shared),
                registered_before_poll: AtomicBool::new(false),
                mpsc_clone_seen: AtomicBool::new(false),
                panic_at_mpsc_clone: AtomicBool::new(true),
                wakes: AtomicUsize::new(0),
            };
            let probe_waker = Waker::from_raw(std::task::RawWaker::new(
                &probe as *const HandleOrderProbe as *const (),
                &HANDLE_ORDER_PROBE_VTABLE,
            ));
            let mut cx = Context::from_waker(&probe_waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
            let (mut e, mut o) = (0i64, 0i64);
            // The child hasn't run yet (current_thread runtime, nothing driven): the
            // outbox is empty, poll_recv takes the Pending path, and the armed probe
            // panics at the slot registration → the panic path returns Pending.
            assert_eq!(
                ynz_handle_recv_poll(h, &mut e, &mut o, cx_ptr),
                CHANNEL_PENDING,
                "the panic path must surface as Pending (never a crash across the C ABI)"
            );
            assert!(
                probe.mpsc_clone_seen.load(Ordering::SeqCst),
                "probe never reached the mpsc slot registration — vacuous run"
            );

            // The child now completes and delivers. Pre-fix: nobody to wake → hang.
            rt.block_on(async { tokio::task::yield_now().await });
            assert!(
                probe.wakes.load(Ordering::SeqCst) > 0,
                "completion delivery must wake a receiver whose poll panicked before the \
                 mpsc slot registration — wakes == 0 IS the P2-7 permanent hang"
            );

            // The woken task re-polls (fresh waker, as after any spurious wake) and must
            // collect the completion — also proves the panic-poisoned outbox mutex recovers.
            let recheck_waker = make_waker();
            let mut recheck_cx = Context::from_waker(&recheck_waker);
            let recheck_ptr = &mut recheck_cx as *mut Context<'_> as *mut u8;
            assert_eq!(
                ynz_handle_recv_poll(h, &mut e, &mut o, recheck_ptr),
                CHANNEL_READY
            );
            assert_eq!((e, o), (0, 99));
            ynz_handle_free(h);
        }
    }
}
