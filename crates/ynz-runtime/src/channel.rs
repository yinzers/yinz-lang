//! Bounded task-communication channels (`channel<T>`) — the deadlock-safe C-ABI substrate
//! (v0.3-M4 Phase 1 single-task model, extended in Phase 2 to the SHARED cross-task model).
//!
//! # Why this module exists — the R1 deadlock-safety core
//!
//! A `channel<T>` value is a bounded [`tokio::sync::mpsc`] channel. `send()` on a full channel
//! MUST suspend the calling task via the poll-yield state-machine protocol — it must NEVER make a
//! synchronous blocking call (`block_on`, `blocking_send`, `spawn_blocking`, `thread::sleep`,
//! `.park()`). A blocking call here is the exact M2 `block_on`-HALT corpse class.
//!
//! This module makes durable the design the P0 R5 spike proved through the real compiler
//! (`p0-spike/R5-composed-suspension-spike-report.md`): the channel endpoint futures live in the
//! runtime-owned channel object HERE, never the frame header's type-punned `sleep_handle` slot
//! (trap door 1c avoided by construction). Every suspension polls the in-flight endpoint future
//! with the FORWARDED waker from the enclosing state-machine's `poll` (the
//! `ynz_rt_async_sleep_poll` no-fabricated-waker discipline).
//!
//! # Phase 2 — shared cross-task ownership
//!
//! A channel handed to a `background` task is SHARED, not moved or copied: both tasks operate on
//! the SAME bounded buffer (that is the whole point of a channel). The object is therefore
//! refcounted (`Arc`) and internally synchronized:
//!
//! - [`ynz_channel_create`] returns an `Arc`-backed pointer (strong count 1).
//! - [`ynz_channel_share`] bumps the refcount at each spawn boundary (codegen emits it for every
//!   channel argument to `background`); the spawned task's reference is released by the
//!   arg-drop machinery (`BgArgDropEntry` kind 2 → [`ynz_channel_free`]).
//! - [`ynz_channel_free`] drops one reference; the buffer dies with the last one. Alloc=free by
//!   refcount balance; a null pointer is a safe no-op.
//!
//! Interior synchronization uses short `std::sync::Mutex` critical sections around NON-BLOCKING
//! polls only (`try_send`, one endpoint-future poll, `poll_recv`) — never held across a
//! suspension, never a blocking channel wait. This is the standard poll-safe pattern (Tokio's own
//! internals do the same); it is NOT in the R1 banned-call class (`block_on` / `blocking_send` /
//! `blocking_recv` / `spawn_blocking` / `thread::sleep` / `park`), which this module contains
//! zero of.
//!
//! # In-flight sends are keyed PER SUSPENDED CALLER — `(caller_token, caller_generation)`
//!
//! With two tasks sharing one channel, a single `pending_send` slot would be a silent-wrong
//! hazard: task B's re-poll could drive task A's suspended send and drop B's value. Each
//! suspended `send` is therefore keyed by an opaque `caller_token` (codegen passes the caller's
//! frame pointer for bare-channel sends; the handle pointer for `h.send()`) PLUS a
//! `caller_generation` salt (v0.3-M6 P3-1). The raw token alone is NOT collision-free across
//! time: a task/handle cancelled while suspended on `send` leaves its entry behind, and
//! allocator reuse of the freed address resurrects the dead caller's entry under a new caller's
//! identity (the ABA class — the new value silently discarded, the dead value delivered).
//! Two mitigations, both (Decision D2), from ONE shared scheme covering BOTH token producers
//! (frame-pointer conduit tokens AND handle-pointer tokens — authoritative-derivation.md):
//!
//! - **Generation-salted key.** One global monotonic counter ([`next_caller_generation`]) stamps
//!   every caller identity at birth: a spawned task's `SpawnStateFnFuture` carries `task_gen`
//!   (published to this module via a thread-local while its `poll` runs, so the extern-C send
//!   ABI stays unchanged and every token the task mints — root, embedded-child, chain-child —
//!   carries it), a task handle carries `send_gen` (passed explicitly by
//!   `ynz_handle_send_poll`), and every entrypoint sync drive (`SyncStateFnFuture`) carries its
//!   own `task_gen` published the same way — EVERY production caller identity is stamped
//!   nonzero, uniformly. Generation 0 is reserved for bare unstamped ABI calls (substrate
//!   tests polling `ynz_channel_send_poll` outside any state-machine drive); no production
//!   path mints it. A reused address can therefore never match a stale entry — the
//!   generations differ even inside the purge's race window.
//! - **Purge on cancellation.** ONE shared idempotent helper ([`purge_pending_sends`]) removes a
//!   dying identity's entries at BOTH cancellation paths (the drop ladder's kind-2 shared-channel
//!   arm for frame tokens; `ynz_handle_free` for handle tokens), closing the orphan leak (P2-2).
//!
//! # Multi-waiter receive wakeups
//!
//! `mpsc::Receiver::poll_recv` registers only the MOST RECENT waker. Two tasks suspended on
//! `receive()` on one shared channel would lose a wakeup (silent hang class). Every successful
//! send therefore wakes EVERY recorded receive-waiter (they re-poll; one wins, the rest
//! re-register) — sound because every producer in a Yinz program goes through this C-ABI.
//!
//! # The poll ABI (mirrors `ynz_rt_async_sleep_poll`)
//!
//! [`ynz_channel_send_poll`] / [`ynz_channel_recv_poll`] return an `i32`:
//!
//! - `0` = **Ready** — send: the value was accepted; recv: a value was written to `out`.
//! - `1` = **Pending** — the task must save `resume_point` and return Pending to the executor.
//! - `2` = **Closed** — send: the receiver was dropped (a typed Yinz channel-closed `errors`
//!   value, NEVER the raw Tokio `SendError` — Lock 8); recv: all senders dropped AND drained.

use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

use tokio::sync::mpsc;

/// Poll result: the operation completed (value accepted / value delivered).
pub(crate) const CHANNEL_READY: i32 = 0;
/// Poll result: the task must suspend (save resume_point, return Pending to the executor).
pub(crate) const CHANNEL_PENDING: i32 = 1;
/// Poll result: the channel is closed (send: receiver dropped; recv: drained and senders gone).
pub(crate) const CHANNEL_CLOSED: i32 = 2;

/// The in-flight `send()` endpoint future: owns a cloned `Sender` + the pending value, resolves
/// to `Ok(())` when the value is accepted or `Err(())` when the receiver has been dropped.
type PendingSend = Pin<Box<dyn Future<Output = Result<(), ()>> + Send>>;

/// The ONE global monotonic caller-generation counter (v0.3-M6 P3-1) — the single salting
/// scheme every caller identity mints from (a spawned task's `task_gen`, a handle's
/// `send_gen`, a sync entrypoint drive's `task_gen`). Starts at 1: generation 0 is reserved
/// for bare unstamped ABI calls (substrate tests only — no production path mints it), which
/// [`purge_pending_sends`] never mass-purges.
static CALLER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Mint the next caller generation. Called once per caller-identity birth (task spawn /
/// handle spawn) — never per send, so a suspended send's key is stable across re-polls.
pub(crate) fn next_caller_generation() -> u64 {
    // Relaxed: only uniqueness/monotonicity of the returned value matters; the value is
    // published to other threads via the spawned future / handle object it is stored in.
    CALLER_GENERATION.fetch_add(1, Ordering::Relaxed)
}

thread_local! {
    /// The generation of the state-machine drive whose `poll` is currently running on THIS
    /// thread (0 = none — a bare test call outside any drive). Published by
    /// [`TaskGenGuard`] around every drive's resume-fn call — `SpawnStateFnFuture::poll`
    /// (spawned tasks) AND `SyncStateFnFuture::poll` (entrypoint / sync-wrapper drives) —
    /// so the extern-C `ynz_channel_send_poll` signature stays unchanged (no codegen
    /// change) while every frame token the drive mints — root, embedded-child,
    /// chain-child — carries the drive's generation.
    static CURRENT_TASK_GEN: Cell<u64> = const { Cell::new(0) };
}

/// RAII publisher for [`CURRENT_TASK_GEN`]: saves the previous value on entry, restores it on
/// drop (panic-safe, nesting-safe). Re-entered from the future's own field at every poll, so
/// work-stealing across threads is safe by construction.
pub(crate) struct TaskGenGuard {
    prev: u64,
}

impl TaskGenGuard {
    pub(crate) fn enter(task_generation: u64) -> Self {
        let prev = CURRENT_TASK_GEN.with(|c| c.replace(task_generation));
        TaskGenGuard { prev }
    }
}

impl Drop for TaskGenGuard {
    fn drop(&mut self) {
        let prev = self.prev;
        CURRENT_TASK_GEN.with(|c| c.set(prev));
    }
}

/// The generation of the task currently being polled on this thread (0 = unstamped).
fn current_task_generation() -> u64 {
    CURRENT_TASK_GEN.with(|c| c.get())
}

/// Extract a human-readable message from a caught-panic payload (the same `&str`/`String`
/// downcast `ynz_rt_async_sleep_poll` performs). Shared by both channel poll shims.
fn panic_payload_msg(e: &Box<dyn Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Poison-tolerant lock: a panic while holding the lock (already caught and reported by the
/// poll shims' `catch_unwind`) must not wedge every later channel op behind a poisoned mutex.
/// `pub(crate)` — the handle module shares this exact discipline (one definition, not two).
pub(crate) fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A Yinz `channel<T>` value at the runtime ABI boundary.
///
/// All element types cross the ABI as i64 bit patterns (int/bool/float bits, or a pointer cast
/// to i64 for string/array/map elements) — the same i64-slot convention `array`/`map` use.
///
/// Holds BOTH endpoints of one bounded mpsc channel. `Arc`-shared across tasks; every field is
/// either immutable-per-`&self` or guarded by a short non-blocking-critical-section `Mutex`.
pub struct YnzChannel {
    /// Cloneable multi-producer endpoint. In a `Mutex` so substrate tests can swap it to
    /// simulate all-senders-dropped closure; production paths only clone/try_send through it.
    sender: Mutex<mpsc::Sender<i64>>,
    /// The single-consumer endpoint. `poll_recv` needs `&mut` — guarded.
    receiver: Mutex<mpsc::Receiver<i64>>,
    /// In-flight suspended sends, keyed per suspended caller by
    /// `(caller_token, caller_generation)` (see module docs).
    ///
    /// An entry is created on the first send-on-full suspension of a caller, re-polled with the
    /// forwarded waker on each resume, and removed when it resolves. A cancelled caller's entry
    /// is purged at its cancellation path via [`purge_pending_sends`]; the generation salt keeps
    /// a reused token address from ever matching a stale entry in the interim.
    pending_sends: Mutex<HashMap<(u64, u64), PendingSend>>,
    /// Wakers of every task currently suspended on `receive()` (see module docs).
    recv_waiters: Mutex<Vec<Waker>>,
}

impl YnzChannel {
    /// Wake every recorded receive-waiter (a value just landed, or capacity changed).
    /// O(n) wakes where n = suspended receivers (typically 0 or 1).
    fn wake_recv_waiters(&self) {
        let mut waiters = lock_or_recover(&self.recv_waiters);
        for w in waiters.drain(..) {
            w.wake();
        }
    }

    /// Record `waker` as a receive-waiter (deduplicated via `will_wake`).
    fn record_recv_waiter(&self, waker: &Waker) {
        let mut waiters = lock_or_recover(&self.recv_waiters);
        if !waiters.iter().any(|w| w.will_wake(waker)) {
            waiters.push(waker.clone());
        }
    }
}

/// Construct a bounded channel with `capacity` slots. Returns an opaque `Arc`-backed pointer
/// (strong count 1).
///
/// Bounded by construction (stdlib-design.md Rule 4): there is no unbounded constructor. The
/// primary gate is typeck, which resolves the default (64) / explicit `channel<T>(N)` capacity
/// surface and rejects a non-positive literal capacity at compile time. This shim additionally
/// clamps `capacity` to at least 1 as a release-mode defensive floor because `mpsc::channel(0)`
/// panic-aborts; a `< 1` value reaching here is a typeck/codegen regression caught loudly in
/// debug builds by the `debug_assert!`.
///
/// # Side effects
/// Heap-allocates one `YnzChannel` (`Arc`). Each holder releases its reference with
/// [`ynz_channel_free`] exactly once (alloc=free by refcount balance).
#[no_mangle]
pub extern "C" fn ynz_channel_create(capacity: i64) -> *mut u8 {
    debug_assert!(
        capacity >= 1,
        "ynz_channel_create: capacity must be >= 1 (typeck rejects non-positive capacity); got {capacity}"
    );
    let cap = if capacity < 1 { 1 } else { capacity as usize };
    let (sender, receiver) = mpsc::channel::<i64>(cap);
    let chan = Arc::new(YnzChannel {
        sender: Mutex::new(sender),
        receiver: Mutex::new(receiver),
        pending_sends: Mutex::new(HashMap::new()),
        recv_waiters: Mutex::new(Vec::new()),
    });
    Arc::into_raw(chan) as *mut u8
}

/// Bump the channel's refcount — one new co-owner (a `background` task, or a task handle's
/// message conduit). Returns the same pointer for codegen convenience.
///
/// # Safety
/// `chan_ptr` must be a live pointer from [`ynz_channel_create`] (or a prior share), with at
/// least one strong reference outstanding for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_share(chan_ptr: *mut u8) -> *mut u8 {
    if chan_ptr.is_null() {
        return chan_ptr;
    }
    // SAFETY: caller guarantees a live Arc-backed pointer.
    Arc::increment_strong_count(chan_ptr as *const YnzChannel);
    chan_ptr
}

/// Poll a `send(value)` on the channel `chan_ptr`, forwarding the enclosing task's waker.
///
/// The extern-C thin mint over [`channel_send_poll_guarded`] — the ONE keyed send core.
/// `caller_token` keys this caller's suspended-send state (see module docs) — codegen passes the
/// caller's frame pointer (bare-channel send). The generation half of the key is read from the
/// thread-local the enclosing drive's `poll` published (`SpawnStateFnFuture` or the
/// entrypoint's `SyncStateFnFuture`; 0 = a bare unstamped test call), so this signature is
/// byte-identical to pre-M6 — no codegen change.
///
/// # Safety
/// - `chan_ptr` must be a live pointer from [`ynz_channel_create`]/[`ynz_channel_share`].
/// - `waker_ctx` must point to a live `&mut Context<'_>` for the duration of this call (the same
///   context passed into the enclosing state machine's `Future::poll`).
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_send_poll(
    chan_ptr: *mut u8,
    value: i64,
    waker_ctx: *mut u8,
    caller_token: u64,
) -> i32 {
    channel_send_poll_guarded(
        chan_ptr,
        value,
        waker_ctx,
        caller_token,
        current_task_generation(),
    )
}

/// The ONE keyed send-poll core (authoritative-derivation.md — both producers mint over this,
/// never two cores): [`ynz_channel_send_poll`] (generation from the poll thread-local) and
/// `ynz_handle_send_poll` (generation from the handle's own `send_gen` stamp).
///
/// # Flow
/// 1. If THIS caller has a send in flight (a prior call suspended on a full channel), re-poll
///    THAT future — `value` is ignored (the in-flight future already captured its value). The
///    key is `(caller_token, caller_generation)`: a reused token address under a NEW generation
///    can never match a dead caller's stale entry (the P3-1 ABA fix).
/// 2. Otherwise `try_send(value)`: on success wake receive-waiters and return [`CHANNEL_READY`];
///    on `Full` create the boxed endpoint future (`sender.clone().send(value)`), poll it once to
///    register the forwarded waker, and suspend; on `Closed` return [`CHANNEL_CLOSED`].
///
/// Never makes a synchronous blocking call — a full channel yields [`CHANNEL_PENDING`] and the
/// task suspends via the state machine. This is the R1 no-blocking-call guarantee in code.
///
/// # Failure modes
/// - Receiver dropped → [`CHANNEL_CLOSED`] (the caller maps this to a typed Yinz channel-closed
///   `errors` value — never the raw Tokio `SendError`, Lock 8). The unsent value is dropped by
///   ownership; never a silent success.
///
/// # Side effects
/// Time: O(1) + O(w) receive-waiter wakes + O(p) insert-time stale sweep where p = in-flight
/// suspended sends (typically 0 or 1)  Space: O(1); boxes one future on first suspension.
///
/// # Safety
/// Same contract as [`ynz_channel_send_poll`].
pub(crate) unsafe fn channel_send_poll_guarded(
    chan_ptr: *mut u8,
    value: i64,
    waker_ctx: *mut u8,
    caller_token: u64,
    caller_generation: u64,
) -> i32 {
    // Mirror ynz_rt_async_sleep_poll's panic discipline: a panic inside the poll is caught and
    // reported as CHANNEL_PENDING so the enclosing state-machine frame is not corrupted.
    let result = std::panic::catch_unwind(|| {
        // SAFETY: chan_ptr is a live Arc-backed pointer (caller guarantee); shared &.
        let chan = &*(chan_ptr as *const YnzChannel);
        // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing state-machine poll.
        let cx = &mut *(waker_ctx as *mut Context<'_>);
        let key = (caller_token, caller_generation);

        // Re-poll THIS caller's already-suspended send (never another caller's — the
        // token+generation keying is what makes the shared-channel model silent-wrong-proof).
        let mut pending = lock_or_recover(&chan.pending_sends);
        if let Some(fut) = pending.get_mut(&key) {
            return match fut.as_mut().poll(cx) {
                Poll::Pending => CHANNEL_PENDING,
                Poll::Ready(Ok(())) => {
                    pending.remove(&key);
                    drop(pending);
                    chan.wake_recv_waiters();
                    CHANNEL_READY
                }
                Poll::Ready(Err(())) => {
                    pending.remove(&key);
                    CHANNEL_CLOSED
                }
            };
        }
        drop(pending);

        // First attempt: non-blocking try_send. On a non-full channel this is the fast Ready
        // path (mirrors the sleep first-poll-Ready fast path — no suspension state needed).
        let sender = lock_or_recover(&chan.sender).clone();
        match sender.try_send(value) {
            Ok(()) => {
                chan.wake_recv_waiters();
                CHANNEL_READY
            }
            Err(mpsc::error::TrySendError::Closed(_)) => CHANNEL_CLOSED,
            Err(mpsc::error::TrySendError::Full(v)) => {
                // Backpressure: the channel is full. Create the endpoint future owning a cloned
                // sender + the value, poll it once to register the forwarded waker, and suspend
                // if it can't complete immediately.
                let fut_sender = sender.clone();
                let mut fut: PendingSend =
                    Box::pin(async move { fut_sender.send(v).await.map_err(|_| ()) });
                match fut.as_mut().poll(cx) {
                    Poll::Pending => {
                        let mut pending = lock_or_recover(&chan.pending_sends);
                        // Missed-path leak backstop: two LIVE caller identities can never
                        // share a token address, so any same-token / different-generation
                        // entry has a DEAD owner — sweep it here. Closes the P2-2 orphan for
                        // any cancellation path not wired to purge_pending_sends.
                        pending.retain(|k, _| k.0 != caller_token || k.1 == caller_generation);
                        pending.insert(key, fut);
                        CHANNEL_PENDING
                    }
                    Poll::Ready(Ok(())) => {
                        chan.wake_recv_waiters();
                        CHANNEL_READY
                    }
                    Poll::Ready(Err(())) => CHANNEL_CLOSED,
                }
            }
        }
    });
    match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "ynz runtime: channel send poll panicked (returning Pending): {}",
                panic_payload_msg(&e)
            );
            CHANNEL_PENDING
        }
    }
}

/// Poll a `receive()` on the channel `chan_ptr`, forwarding the enclosing task's waker.
///
/// # Flow
/// `receiver.poll_recv(cx)`: on `Ready(Some(v))` write `v` to `out` and return
/// [`CHANNEL_READY`]; on `Ready(None)` (all senders dropped AND drained) return
/// [`CHANNEL_CLOSED`]; on `Pending` record the waker as a receive-waiter and return
/// [`CHANNEL_PENDING`] — the task suspends until a value arrives.
///
/// `poll_recv` is natively poll-based and re-entrant, so no in-flight future is stored.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — one `poll_recv`; writes `*out` only on the Ready path.
///
/// # Safety
/// - `chan_ptr` must be a live pointer from [`ynz_channel_create`]/[`ynz_channel_share`].
/// - `out` must point to a writable `i64` (written only on [`CHANNEL_READY`]).
/// - `waker_ctx` must point to a live `&mut Context<'_>` for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_recv_poll(
    chan_ptr: *mut u8,
    out: *mut i64,
    waker_ctx: *mut u8,
) -> i32 {
    // Mirror ynz_rt_async_sleep_poll's panic discipline (see ynz_channel_send_poll).
    let result = std::panic::catch_unwind(|| {
        // SAFETY: chan_ptr is a live Arc-backed pointer (caller guarantee); shared &.
        let chan = &*(chan_ptr as *const YnzChannel);
        // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing state-machine poll.
        let cx = &mut *(waker_ctx as *mut Context<'_>);
        let poll = lock_or_recover(&chan.receiver).poll_recv(cx);
        match poll {
            Poll::Ready(Some(v)) => {
                // SAFETY: out points to a writable i64 (caller guarantee).
                *out = v;
                // A slot just freed — wake any OTHER receive-waiters so a multi-consumer
                // arrangement re-polls (and, transitively, suspended senders progress via
                // their own per-future waker registrations).
                chan.wake_recv_waiters();
                CHANNEL_READY
            }
            Poll::Ready(None) => CHANNEL_CLOSED,
            Poll::Pending => {
                chan.record_recv_waiter(cx.waker());
                CHANNEL_PENDING
            }
        }
    });
    match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "ynz runtime: ynz_channel_recv_poll panicked (returning Pending): {}",
                panic_payload_msg(&e)
            );
            CHANNEL_PENDING
        }
    }
}

/// Release one reference to the channel object. The bounded buffer (and any orphaned suspended
/// sends) are dropped with the LAST reference. A null pointer is a no-op (safe under
/// cancellation-drop paths).
///
/// # Safety
/// `chan_ptr` must be a pointer from [`ynz_channel_create`]/[`ynz_channel_share`] whose
/// reference has not already been released, or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_free(chan_ptr: *mut u8) {
    if chan_ptr.is_null() {
        return;
    }
    // SAFETY: reconstructing the Arc is the inverse of Arc::into_raw / increment_strong_count.
    drop(Arc::from_raw(chan_ptr as *const YnzChannel));
}

/// Purge a dying caller identity's suspended sends from the channel (v0.3-M6 P2-2) — the ONE
/// shared purge helper both cancellation paths call (authoritative-derivation.md): the drop
/// ladder's kind-2 shared-channel arm (`SpawnStateFnFuture::drop`, frame tokens) and
/// `ynz_handle_free` (handle tokens). Rust-internal — codegen never calls this.
///
/// Purges by GENERATION, not by token: one call sweeps every token the dying identity ever
/// minted on this channel (root frame, embedded-child, chain-child), and each dropped entry
/// releases its boxed endpoint future + cloned sender + captured value (the leak fix).
///
/// **Idempotent by construction**: no matching entry (double-cancel, already-resolved,
/// already-purged) is a safe no-op — never a panic or UB. Null `chan_ptr` is a no-op (the
/// cancellation paths run on null channel slots too). `caller_generation == 0` is a no-op:
/// generation 0 is the reserved unstamped class (bare substrate-test calls — every
/// production identity is stamped nonzero at birth), so a 0 reaching here can only be an
/// unstamped/buggy caller and must never mass-purge the unstamped entries as one identity.
///
/// Time: O(p) where p = in-flight suspended sends (typically 0 or 1)  Space: O(1).
///
/// # Safety
/// `chan_ptr` must be null or a live pointer from [`ynz_channel_create`]/[`ynz_channel_share`]
/// whose reference has not yet been released (call this BEFORE `ynz_channel_free`).
pub(crate) unsafe fn purge_pending_sends(chan_ptr: *mut u8, caller_generation: u64) {
    if chan_ptr.is_null() || caller_generation == 0 {
        return;
    }
    // SAFETY: chan_ptr is a live Arc-backed pointer (caller guarantee); shared &.
    let chan = &*(chan_ptr as *const YnzChannel);
    lock_or_recover(&chan.pending_sends).retain(|k, _| k.1 != caller_generation);
}

/// Test-support: number of in-flight suspended sends currently parked on the channel
/// (any caller). The M6 ABA/orphan repro suite asserts purge-on-cancellation through this.
///
/// # Safety
/// `chan_ptr` must be a live pointer from [`ynz_channel_create`]/[`ynz_channel_share`].
#[cfg(test)]
pub(crate) unsafe fn pending_send_count(chan_ptr: *mut u8) -> usize {
    let chan = &*(chan_ptr as *const YnzChannel);
    lock_or_recover(&chan.pending_sends).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::Wake;

    /// A minimal counting waker — proves the poll ABI works WITHOUT a Tokio runtime (mpsc's
    /// waker registration is runtime-agnostic) and lets a test assert wakeups happened.
    struct CountingWaker(AtomicUsize);
    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn make_waker() -> (Arc<CountingWaker>, Waker) {
        let arc = Arc::new(CountingWaker(AtomicUsize::new(0)));
        let waker = Waker::from(arc.clone());
        (arc, waker)
    }

    /// Poll a send through the real C-ABI with a `*mut Context`.
    unsafe fn send_tok(chan: *mut u8, value: i64, waker: &Waker, token: u64) -> i32 {
        let mut cx = Context::from_waker(waker);
        let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
        ynz_channel_send_poll(chan, value, cx_ptr, token)
    }

    unsafe fn send(chan: *mut u8, value: i64, waker: &Waker) -> i32 {
        send_tok(chan, value, waker, 0xA)
    }

    unsafe fn recv(chan: *mut u8, waker: &Waker) -> (i32, i64) {
        let mut out: i64 = 0;
        let mut cx = Context::from_waker(waker);
        let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
        let code = ynz_channel_recv_poll(chan, &mut out, cx_ptr);
        (code, out)
    }

    /// THE R1 substrate proof: send-on-full returns Pending (never blocks the thread), and the
    /// suspended send resumes to Ready once the consumer drains a slot. No Tokio runtime, no
    /// blocking call anywhere — the whole cycle is manual polls.
    #[test]
    fn send_on_full_suspends_then_resumes_after_drain() {
        let (_arc, waker) = make_waker();
        // Capacity-1 channel forces backpressure on the second send.
        let chan = ynz_channel_create(1);
        unsafe {
            // First send fills the single slot — Ready immediately (fast path, no suspension).
            assert_eq!(
                send(chan, 10, &waker),
                CHANNEL_READY,
                "send#1 must be Ready"
            );

            // Second send finds the channel FULL. It must return Pending — NOT block the
            // thread. This returning at all is the proof it did not block.
            assert_eq!(
                send(chan, 20, &waker),
                CHANNEL_PENDING,
                "send#2 on a full channel must SUSPEND (Pending), never block the thread"
            );

            // Consumer drains the first value — frees a slot.
            assert_eq!(
                recv(chan, &waker),
                (CHANNEL_READY, 10),
                "recv#1 must deliver 10"
            );

            // Re-poll the suspended send. Capacity is now free → it resolves to Ready.
            assert_eq!(
                send(chan, 20, &waker),
                CHANNEL_READY,
                "the suspended send must RESUME to Ready once a slot freed (no deadlock)"
            );

            // Consumer receives the second value — the full composed handshake completed.
            assert_eq!(
                recv(chan, &waker),
                (CHANNEL_READY, 20),
                "recv#2 must deliver 20"
            );

            ynz_channel_free(chan);
        }
    }

    /// Phase 2 shared-channel proof: two DIFFERENT callers suspended on send-on-full keep
    /// their values separate (per-caller-token keying — the silent-wrong hazard the shared
    /// model introduces and this design eliminates).
    #[test]
    fn per_caller_token_keeps_suspended_sends_separate() {
        let (_arc, waker) = make_waker();
        let chan = ynz_channel_create(1);
        unsafe {
            assert_eq!(send_tok(chan, 1, &waker, 0xAA), CHANNEL_READY);
            // Caller A suspends with value 2; caller B suspends with value 3.
            assert_eq!(send_tok(chan, 2, &waker, 0xAA), CHANNEL_PENDING);
            assert_eq!(send_tok(chan, 3, &waker, 0xBB), CHANNEL_PENDING);
            // Drain one slot; re-poll BOTH callers. Exactly one wins the freed slot; the other
            // stays Pending — and delivered values must be each caller's OWN value.
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 1));
            let a = send_tok(chan, 2, &waker, 0xAA);
            let b = send_tok(chan, 3, &waker, 0xBB);
            assert!(
                (a == CHANNEL_READY) ^ (b == CHANNEL_READY),
                "exactly one suspended sender wins the freed slot (a={a}, b={b})"
            );
            let (code, v1) = recv(chan, &waker);
            assert_eq!(code, CHANNEL_READY);
            // Re-poll the loser; it now wins the newly freed slot.
            let a2 = send_tok(chan, 2, &waker, 0xAA);
            let b2 = send_tok(chan, 3, &waker, 0xBB);
            let _ = (a2, b2);
            let (code2, v2) = recv(chan, &waker);
            assert_eq!(code2, CHANNEL_READY);
            let mut got = [v1, v2];
            got.sort_unstable();
            assert_eq!(
                got,
                [2, 3],
                "each caller's OWN value must be delivered, never mixed"
            );
            ynz_channel_free(chan);
        }
    }

    /// recv on an empty (but open) channel suspends; recv after a value arrives delivers it.
    /// Also proves the send-side wake of recorded receive-waiters.
    #[test]
    fn recv_on_empty_suspends_then_delivers() {
        let (arc, waker) = make_waker();
        let chan = ynz_channel_create(4);
        unsafe {
            // Empty channel → recv suspends (Pending), never blocks.
            let (code, _) = recv(chan, &waker);
            assert_eq!(
                code, CHANNEL_PENDING,
                "recv on empty must SUSPEND, never block"
            );
            // A value arrives (send is Ready — channel not full) → the recorded
            // receive-waiter must be woken.
            let wakes_before = arc.0.load(Ordering::SeqCst);
            assert_eq!(send(chan, 42, &waker), CHANNEL_READY);
            assert!(
                arc.0.load(Ordering::SeqCst) > wakes_before,
                "a successful send must wake the recorded receive-waiter"
            );
            // Now recv delivers it.
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 42));
            ynz_channel_free(chan);
        }
    }

    /// send to a closed channel (receiver dropped) returns Closed — the typed-`errors` signal,
    /// never a silent success.
    #[test]
    fn send_to_closed_returns_closed() {
        let (_arc, waker) = make_waker();
        let chan_ptr = ynz_channel_create(2);
        unsafe {
            // Drop the real receiver (swap in a detached one) so the sender observes closure.
            let chan = &*(chan_ptr as *const YnzChannel);
            let (_dead_tx, dead_rx) = mpsc::channel::<i64>(1);
            let real_rx = std::mem::replace(&mut *lock_or_recover(&chan.receiver), dead_rx);
            drop(real_rx); // original receiver dropped → the sender now sees Closed

            assert_eq!(
                send(chan_ptr, 99, &waker),
                CHANNEL_CLOSED,
                "send to a dropped-receiver channel must return Closed (typed errors), never Ready"
            );
            ynz_channel_free(chan_ptr);
        }
    }

    /// recv on a drained + all-senders-dropped channel returns Closed (no more values coming).
    #[test]
    fn recv_on_closed_drained_returns_closed() {
        let (_arc, waker) = make_waker();
        let chan_ptr = ynz_channel_create(2);
        unsafe {
            // Drop the real sender so the receiver observes closure once drained.
            let chan = &*(chan_ptr as *const YnzChannel);
            let (dead_tx, _dead_rx) = mpsc::channel::<i64>(1);
            let real_tx = std::mem::replace(&mut *lock_or_recover(&chan.sender), dead_tx);
            drop(real_tx); // all real senders gone

            let (code, _) = recv(chan_ptr, &waker);
            assert_eq!(
                code, CHANNEL_CLOSED,
                "recv on a closed + drained channel must return Closed"
            );
            ynz_channel_free(chan_ptr);
        }
    }

    /// v0.3-M6 P2-2: the shared purge helper is idempotent by construction — double-purge,
    /// purge-on-empty, purge-null, and the reserved generation-0 class are all safe no-ops.
    #[test]
    fn purge_pending_sends_is_idempotent_and_gen0_is_reserved() {
        let (_arc, waker) = make_waker();
        let chan = ynz_channel_create(1);
        unsafe {
            // Purge on an EMPTY map: no-op, no panic.
            purge_pending_sends(chan, 7);
            // Null channel: no-op, no panic.
            purge_pending_sends(std::ptr::null_mut(), 7);

            // Park one gen-7 suspended send and one gen-0 (unstamped) suspended send.
            assert_eq!(send(chan, 1, &waker), CHANNEL_READY); // fill capacity-1
            let mut cx = Context::from_waker(&waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
            assert_eq!(
                channel_send_poll_guarded(chan, 2, cx_ptr, 0xA1, 7),
                CHANNEL_PENDING
            );
            assert_eq!(send_tok(chan, 3, &waker, 0xB2), CHANNEL_PENDING); // gen 0 path
            assert_eq!(pending_send_count(chan), 2);

            // Generation 0 is the reserved unstamped class — NEVER mass-purged.
            purge_pending_sends(chan, 0);
            assert_eq!(
                pending_send_count(chan),
                2,
                "gen-0 purge must be a no-op (the unstamped class is never mass-purged \
                 as one identity)"
            );

            // Purge gen 7: exactly the gen-7 entry goes; the gen-0 entry survives.
            purge_pending_sends(chan, 7);
            assert_eq!(pending_send_count(chan), 1);
            // Double-purge (repeated cancel): safe no-op, never a panic.
            purge_pending_sends(chan, 7);
            assert_eq!(pending_send_count(chan), 1);

            // The surviving gen-0 send still completes correctly after a drain.
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 1));
            assert_eq!(send_tok(chan, 3, &waker, 0xB2), CHANNEL_READY);
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 3));
            ynz_channel_free(chan);
        }
    }

    /// v0.3-M6 P3-1: the deterministic keyed-core ABA collision proof — the same token under
    /// a NEW generation never matches a dead caller's stale entry (even with NO purge run,
    /// i.e. inside the purge's own race window), the new caller's value is delivered, the
    /// stale value never resurfaces, and the stale entry is swept on insert (the missed-path
    /// leak backstop). This is the deterministic coverage the handle-path repro's best-effort
    /// address forcing falls back to.
    #[test]
    fn same_token_different_generation_never_collides_and_stale_is_swept() {
        let (_arc, waker) = make_waker();
        let chan = ynz_channel_create(1);
        unsafe {
            assert_eq!(send(chan, 42, &waker), CHANNEL_READY); // fill capacity-1
            let mut cx = Context::from_waker(&waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;

            // Dead caller (gen 5) suspends at token T, then "dies" WITHOUT a purge —
            // simulating the residual window between cancellation and purge completion.
            const T: u64 = 0xDEAD;
            assert_eq!(
                channel_send_poll_guarded(chan, 111, cx_ptr, T, 5),
                CHANNEL_PENDING
            );
            assert_eq!(pending_send_count(chan), 1);

            // New caller (gen 9) at the SAME (reused) token suspends. Its key differs by
            // generation, so it must NOT re-poll the stale entry — and the insert-time
            // sweep removes the dead gen-5 entry (same token, different generation ⇒ dead
            // owner), so the count stays 1, not 2.
            assert_eq!(
                channel_send_poll_guarded(chan, 222, cx_ptr, T, 9),
                CHANNEL_PENDING
            );
            assert_eq!(
                pending_send_count(chan),
                1,
                "the stale same-token entry must be swept on insert (leak backstop)"
            );

            // Drain; re-poll the new caller: ITS value (222) must deliver — pre-fix the
            // stale entry delivered the dead caller's 111 and silently discarded 222.
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 42));
            assert_eq!(
                channel_send_poll_guarded(chan, 222, cx_ptr, T, 9),
                CHANNEL_READY
            );
            assert_eq!(
                recv(chan, &waker),
                (CHANNEL_READY, 222),
                "the NEW generation's value must deliver; the dead generation's stale \
                 value must never resurface"
            );
            assert_eq!(pending_send_count(chan), 0);
            ynz_channel_free(chan);
        }
    }

    /// Refcount balance: share bumps, free releases; the object survives until the LAST free.
    #[test]
    fn share_then_free_is_refcount_balanced() {
        let (_arc, waker) = make_waker();
        let chan = ynz_channel_create(2);
        unsafe {
            let alias = ynz_channel_share(chan);
            assert_eq!(alias, chan, "share returns the same pointer");
            // Release one reference — the object must still be live and usable.
            ynz_channel_free(alias);
            assert_eq!(send(chan, 7, &waker), CHANNEL_READY);
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 7));
            // Last reference — the buffer dies here (Miri/ASAN would flag a double-free).
            ynz_channel_free(chan);
        }
    }
}
