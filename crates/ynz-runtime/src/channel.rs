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
//!   arm for frame tokens; `ynz_handle_free` for handle tokens), closing the orphan leak (P2-2),
//!   and frees each purged entry's heap payload through the channel's registered drop glue
//!   (FRAGO 028 — a removed entry is invisible to the channel's own `Drop`).
//!
//! # Multi-waiter receive wakeups
//!
//! `mpsc::Receiver::poll_recv` registers only the MOST RECENT waker. Two tasks suspended on
//! `receive()` on one shared channel would lose a wakeup (silent hang class). Every successful
//! send therefore wakes EVERY recorded receive-waiter (they re-poll; one wins, the rest
//! re-register) — sound because every producer in a Yinz program goes through this C-ABI.
//! Each receiver records itself BEFORE polling (v0.3-M6 P3-2), so a send can never land in an
//! unregistered gap between a receiver's poll and a late registration. Closure observed by one
//! receiver is NOT propagated to recorded co-waiters — presently unreachable in production (bare
//! channels never close; every close-simulation is `#[cfg(test)]`-only), left for the M8
//! channel-close-semantics design pass rather than fixed piecemeal here.
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

use crate::runtime::{release_ladder_payload, DriveIdentity};

/// Poll result: the operation completed (value accepted / value delivered).
pub(crate) const CHANNEL_READY: i32 = 0;
/// Poll result: the task must suspend (save resume_point, return Pending to the executor).
pub(crate) const CHANNEL_PENDING: i32 = 1;
/// Poll result: the channel is closed (send: receiver dropped; recv: drained and senders gone).
pub(crate) const CHANNEL_CLOSED: i32 = 2;

/// The in-flight `send()` endpoint future: owns a cloned `Sender` + the pending value, resolves
/// to `Ok(())` when the value is accepted or `Err(())` when the receiver has been dropped.
type PendingSend = Pin<Box<dyn Future<Output = Result<(), ()>> + Send>>;

/// A suspended send parked in [`YnzChannel::pending_sends`]: the boxed endpoint future plus a
/// mirror of the element value bits it captured (v0.3-M6 P2-4). The future owns the value and
/// never exposes it, so the bits are mirrored here at insert time — channel teardown
/// ([`YnzChannel`]'s `Drop`) needs them to run the element drop glue on payloads that never
/// reached the buffer.
struct PendingSendEntry {
    fut: PendingSend,
    /// The suspended send's element value as i64 bits (pointer bits for heap elements).
    value_bits: i64,
}

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
    /// The identity of the state-machine drive whose `poll` is currently running on THIS
    /// thread ([`DriveIdentity::NONE`] = none — a bare test call outside any drive): its
    /// generation, plus its spawn-arg drop ladder. Published by [`DriveGuard`] around every
    /// drive's resume-fn call — `SpawnStateFnFuture::poll` (spawned tasks) AND
    /// `SyncStateFnFuture::poll` (entrypoint / sync-wrapper drives, ladder-less) — so the
    /// extern-C `ynz_channel_send_poll` signature stays unchanged (no codegen change) while
    /// every frame token the drive mints — root, embedded-child, chain-child — carries the
    /// drive's generation, and a send of a ladder-owned payload can release it from the
    /// drive's ladder ([`crate::runtime::release_ladder_payload`]).
    static CURRENT_DRIVE: Cell<DriveIdentity> = const { Cell::new(DriveIdentity::NONE) };
}

/// RAII publisher for [`CURRENT_DRIVE`]: saves the previous value on entry, restores it on
/// drop (panic-safe, nesting-safe). Re-entered from the future's own fields at every poll, so
/// work-stealing across threads is safe by construction.
pub(crate) struct DriveGuard {
    prev: DriveIdentity,
}

impl DriveGuard {
    pub(crate) fn enter(drive: DriveIdentity) -> Self {
        let prev = CURRENT_DRIVE.with(|c| c.replace(drive));
        DriveGuard { prev }
    }
}

impl Drop for DriveGuard {
    fn drop(&mut self) {
        let prev = self.prev;
        CURRENT_DRIVE.with(|c| c.set(prev));
    }
}

/// The drive currently being polled on this thread ([`DriveIdentity::NONE`] = unstamped).
fn current_drive() -> DriveIdentity {
    CURRENT_DRIVE.with(|c| c.get())
}

/// The generation of the task currently being polled on this thread (0 = unstamped).
fn current_task_generation() -> u64 {
    current_drive().generation
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
    pending_sends: Mutex<HashMap<(u64, u64), PendingSendEntry>>,
    /// Wakers of every task currently suspended on `receive()` (see module docs).
    recv_waiters: Mutex<Vec<Waker>>,
    /// Per-element-type drop glue, registered ONCE at construction (v0.3-M6 P2-4) — the single
    /// authoritative element-drop choke point. Invoked by this type's `Drop` impl on each
    /// residual buffered element and suspended-send payload at last-ref teardown, AND on each
    /// `PendingSendEntry` removed at the two cancellation paths ([`purge_pending_sends`] and
    /// `channel_send_poll_guarded`'s insert-time stale-entry sweep — FRAGO 028: a removed
    /// entry's payload is unreachable to `Drop` and would leak otherwise). `None` for
    /// element types with no runtime-owned heap payload: int/float/bool (value bits) AND
    /// `string` (raw-malloc'd immortal bytes, invisible to the alloc counter — never freed).
    ///
    /// Stored as an `Option` of a fn pointer, NOT a raw `*mut u8` field: `YnzChannel` is
    /// `Arc`-shared cross-thread relying on AUTO `Send`/`Sync`, which a raw-pointer field
    /// would silently break (fn pointers are `Send + Sync`).
    drop_glue: Option<unsafe extern "C" fn(i64)>,
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

/// Channel teardown (v0.3-M6 P2-4): with the LAST reference gone, elements still sitting in
/// the bounded buffer — and suspended-send payloads that never reached it — would leak their
/// heap payload (the buffer is typeless i64 bits; tokio's own drop cannot free what the bits
/// point at). The glue registered at construction is the one authoritative per-element drop
/// path (authoritative-derivation.md — never a second ad hoc walk).
///
/// No double-free between the buffered drain and the pending-sends walk: the two touch
/// DISJOINT value sets by construction. tokio mpsc's `send` future enqueues its value in the
/// SAME poll that returns `Ready(Ok)`, and `channel_send_poll_guarded` removes the entry
/// inside that same call before returning — there is no state where a value is in the buffer
/// while its `pending_sends` entry survives. A residual entry's `value_bits` is therefore
/// never ALSO a buffered element; each payload sees the glue exactly once.
///
/// Time: O(b + p) where b = buffered elements, p = residual suspended sends  Space: O(1).
impl Drop for YnzChannel {
    fn drop(&mut self) {
        let Some(glue) = self.drop_glue else {
            return; // primitive/string element types: nothing per-element to drop
        };
        // `&mut self` is exclusive access: `get_mut` bypasses locking entirely, with the same
        // poison tolerance as `lock_or_recover` (which needs `&Mutex`, not owned access).
        let receiver = self.receiver.get_mut().unwrap_or_else(|e| e.into_inner());
        while let Ok(bits) = receiver.try_recv() {
            // SAFETY: glue was registered at construction for exactly this channel's element
            // type; each buffered element's bits pass through it exactly once (see above).
            unsafe { glue(bits) };
        }
        let pending = self
            .pending_sends
            .get_mut()
            .unwrap_or_else(|e| e.into_inner());
        for entry in pending.values() {
            // SAFETY: as above — a residual entry's payload never reached the buffer (the
            // disjointness argument in the impl doc), so this is its only drop.
            unsafe { glue(entry.value_bits) };
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
///
/// # Safety
/// `drop_glue` must be null (no per-element glue — primitive/string element types) or a valid
/// `unsafe extern "C" fn(i64)` that remains callable for the channel's entire lifetime
/// (codegen passes a module-level synthesized function; Rust tests pass a static fn). At
/// last-ref teardown the glue receives each residual element's i64 bits exactly once.
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_create(capacity: i64, drop_glue: *mut u8) -> *mut u8 {
    debug_assert!(
        capacity >= 1,
        "ynz_channel_create: capacity must be >= 1 (typeck rejects non-positive capacity); got {capacity}"
    );
    let cap = if capacity < 1 { 1 } else { capacity as usize };
    let (sender, receiver) = mpsc::channel::<i64>(cap);
    let glue: Option<unsafe extern "C" fn(i64)> = if drop_glue.is_null() {
        None
    } else {
        // SAFETY: caller guarantees non-null `drop_glue` is a valid extern "C" fn(i64) (the
        // fn-pointer form, not a raw field, is what preserves YnzChannel's auto Send/Sync).
        Some(std::mem::transmute::<*mut u8, unsafe extern "C" fn(i64)>(
            drop_glue,
        ))
    };
    let chan = Arc::new(YnzChannel {
        sender: Mutex::new(sender),
        receiver: Mutex::new(receiver),
        pending_sends: Mutex::new(HashMap::new()),
        recv_waiters: Mutex::new(Vec::new()),
        drop_glue: glue,
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
/// # Ownership hand-off (the spawn-arg use-after-free fix)
/// The moment the channel TAKES `value` — buffered by `try_send`, or captured by the parked
/// endpoint future on a full channel — it owns the payload: the receiver (or the channel's
/// teardown glue, or the pending-send purge) frees it. If the sending task was handed that
/// payload as a heap-cloned `background` argument, the task's drop ladder ALSO owned it and
/// freed it at task retire, under the receiver's feet. So on every accepting path this core
/// releases `value` from the current drive's ladder (`release_ladder_payload`) — gated on the
/// channel carrying a heap-pointer element type (`drop_glue.is_some()`; an `int` payload is
/// never compared). Both send producers (`ch.send`, `h.send`) funnel through here, so the
/// link is made exactly once. A [`CHANNEL_CLOSED`] result on a FIRST poll never releases: the
/// value was not taken, the sender still owns it. A [`CHANNEL_CLOSED`] result on the re-poll
/// of a PARKED send is different — the park already released the payload to the entry, so the
/// re-poll frees it through the channel's drop glue as the entry's last owner (see the
/// `Poll::Ready(Err(()))` arm below).
///
/// # Failure modes
/// - Receiver dropped → [`CHANNEL_CLOSED`] (the caller maps this to a typed Yinz channel-closed
///   `errors` value — never the raw Tokio `SendError`, Lock 8). The unsent value is dropped by
///   ownership; never a silent success.
///
/// # Side effects
/// Time: O(1) + O(w) receive-waiter wakes + O(p) insert-time stale sweep where p = in-flight
/// suspended sends (typically 0 or 1) + O(d) ladder release where d = the sending task's
/// heap-cloned argument count  Space: O(1); boxes one future on first suspension.
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
        // The channel now owns `value` (buffered or parked) — see "Ownership hand-off" above.
        // Runs on the SENDING task's poll thread, so `current_drive()` is the sender.
        let release_taken_value = || {
            if chan.drop_glue.is_some() {
                // SAFETY: the published drive is the future whose poll is running on this
                // thread; its ladder is exclusively its own for the duration of that poll.
                unsafe { release_ladder_payload(&current_drive(), value) };
            }
        };

        // Re-poll THIS caller's already-suspended send (never another caller's — the
        // token+generation keying is what makes the shared-channel model silent-wrong-proof).
        let mut pending = lock_or_recover(&chan.pending_sends);
        if let Some(entry) = pending.get_mut(&key) {
            return match entry.fut.as_mut().poll(cx) {
                Poll::Pending => CHANNEL_PENDING,
                Poll::Ready(Ok(())) => {
                    pending.remove(&key);
                    drop(pending);
                    chan.wake_recv_waiters();
                    CHANNEL_READY
                }
                Poll::Ready(Err(())) => {
                    // The receiver closed while this send was PARKED. The park already
                    // released the payload from the sender's ladder (the entry owned it), and
                    // the endpoint future discarded its `SendError(v)` — so the entry's
                    // `value_bits` mirror is the payload's LAST owner. Free it through the
                    // channel's glue here, exactly as the purge/teardown paths do for a parked
                    // entry, or nobody ever does (a leak, not a double free). Unreachable in
                    // production until channels can close (M8 Phase 4); guarded now because
                    // the park-time release above already assumes this path pays its debt.
                    let orphaned_bits = pending.remove(&key).map(|entry| entry.value_bits);
                    drop(pending);
                    // Glue OUTSIDE the pending_sends lock (never run an arbitrary extern fn
                    // under a channel-internal lock).
                    if let (Some(glue), Some(bits)) = (chan.drop_glue, orphaned_bits) {
                        // SAFETY: glue was registered at construction for exactly this
                        // channel's element type; the parked payload was never buffered and
                        // its entry is gone, so this is its only drop.
                        unsafe { glue(bits) };
                    }
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
                release_taken_value();
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
                        // Parked: the entry (and, on cancellation, the purge/teardown glue)
                        // owns the payload from here — release it from the sender's ladder
                        // now, not at the resumed poll (whose `value` argument is 0).
                        release_taken_value();
                        let mut pending = lock_or_recover(&chan.pending_sends);
                        // Missed-path leak backstop: two LIVE caller identities can never
                        // share a token address, so any same-token / different-generation
                        // entry has a DEAD owner — sweep it here. Closes the P2-2 orphan for
                        // any cancellation path not wired to purge_pending_sends. Each swept
                        // entry's heap payload is freed through the registered glue (FRAGO
                        // 028) — the entry is gone before `Drop` could see it, and a parked
                        // payload is never buffered, so this is its only drop.
                        let mut swept_bits: Vec<i64> = Vec::new();
                        pending.retain(|k, entry| {
                            let keep = k.0 != caller_token || k.1 == caller_generation;
                            if !keep {
                                swept_bits.push(entry.value_bits);
                            }
                            keep
                        });
                        // `v` is Copy (i64): the future captured its own copy above; this
                        // mirror is what channel teardown's drop glue reads (P2-4).
                        pending.insert(key, PendingSendEntry { fut, value_bits: v });
                        drop(pending);
                        // Glue outside the pending_sends lock (never run an arbitrary
                        // extern fn under a channel-internal lock).
                        if let Some(glue) = chan.drop_glue {
                            for bits in swept_bits {
                                // SAFETY: glue was registered at construction for exactly
                                // this channel's element type; the swept entry's payload
                                // sees it exactly once (see above).
                                unsafe { glue(bits) };
                            }
                        }
                        CHANNEL_PENDING
                    }
                    Poll::Ready(Ok(())) => {
                        release_taken_value();
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
/// Record the waker as a receive-waiter FIRST (v0.3-M6 P3-2 — see below), then
/// `receiver.poll_recv(cx)`: on `Ready(Some(v))` write `v` to `out` and return
/// [`CHANNEL_READY`]; on `Ready(None)` (all senders dropped AND drained) return
/// [`CHANNEL_CLOSED`]; on `Pending` return [`CHANNEL_PENDING`] — the task suspends until a
/// value arrives (its waker is already recorded). The `Ready(Some)` exit drains the
/// registration via [`YnzChannel::wake_recv_waiters`] (a self-wake is a harmless spurious
/// re-poll). The `Ready(None)` exit leaves the register-first entry recorded and wakes
/// nobody — closure is unreachable in production today (bare channels never close; every
/// close-simulation is `#[cfg(test)]`-only), and closed-channel wake propagation is an M8
/// channel-close-semantics design question, not fixed piecemeal here; the stale entry is
/// freed with the channel.
///
/// Register-before-poll ordering (v0.3-M6 P3-2): `poll_recv` parks the waker in mpsc's
/// SINGLE slot, where a later consumer's poll clobbers it. With the old poll-then-record
/// ordering, a send landing between an unregistered poll and the late record woke nobody —
/// if that was the channel's FINAL send, a permanent hang. Registering first closes the
/// window: a send's waiter drain is serialized against the record by the `recv_waiters`
/// mutex, so either the drain sees this waker (record first → woken), or the record ran
/// after the drain — in which case the send's enqueue happens-before the `poll_recv` below,
/// which then observes the value.
///
/// `poll_recv` is natively poll-based and re-entrant, so no in-flight future is stored.
///
/// # Side effects
/// Time: O(w) where w = suspended receivers, typically 0 or 1 (waiter-record dedup scan +
/// the `Ready(Some)` drain)  Space: O(1) — one `poll_recv`; writes `*out` only on the Ready
/// path.
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
        // Register BEFORE polling — the P3-2 lost-wakeup fix; ordering rationale in the fn
        // doc above. Each mutex is taken and released on its own (record releases
        // recv_waiters before the receiver lock below is taken — no nesting, no lock held
        // across the non-blocking poll).
        chan.record_recv_waiter(cx.waker());
        let poll = lock_or_recover(&chan.receiver).poll_recv(cx);
        match poll {
            Poll::Ready(Some(v)) => {
                // SAFETY: out points to a writable i64 (caller guarantee).
                *out = v;
                // A slot just freed — wake any OTHER receive-waiters so a multi-consumer
                // arrangement re-polls (and, transitively, suspended senders progress via
                // their own per-future waker registrations). Also drains this call's own
                // register-first entry.
                chan.wake_recv_waiters();
                CHANNEL_READY
            }
            Poll::Ready(None) => CHANNEL_CLOSED,
            Poll::Pending => CHANNEL_PENDING,
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
/// releases its boxed endpoint future + cloned sender + captured value bits (the leak fix).
/// Each purged entry's heap payload is freed through the channel's registered [`drop_glue`]
/// (v0.3-M6 FRAGO 028): a purged entry is removed from `pending_sends` here, so the channel's
/// own `Drop` can never see it — without the glue call the payload would leak forever, and
/// this path (unlike last-ref drop) is LIVE today, called on every real task cancellation.
/// No double-free with `YnzChannel::drop`: glue-at-purge removes the entry it frees, and the
/// `Drop` walk only glues entries still present. A parked payload is never ALSO buffered
/// (the disjointness argument on the `Drop` impl), so each payload sees the glue exactly once.
///
/// [`drop_glue`]: YnzChannel::drop_glue
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
    let mut purged_bits: Vec<i64> = Vec::new();
    lock_or_recover(&chan.pending_sends).retain(|k, entry| {
        let keep = k.1 != caller_generation;
        if !keep {
            purged_bits.push(entry.value_bits);
        }
        keep
    });
    // Glue OUTSIDE the pending_sends lock (the guard above is a temporary — dropped at the
    // end of that statement): the glue is an arbitrary extern fn and must never run under a
    // channel-internal lock.
    if let Some(glue) = chan.drop_glue {
        for bits in purged_bits {
            // SAFETY: glue was registered at construction for exactly this channel's element
            // type; a purged entry's payload never reached the buffer and its entry is gone,
            // so this is its only drop (FRAGO 028 — see the doc above).
            unsafe { glue(bits) };
        }
    }
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
    use crate::runtime::BgArgDropEntry;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::task::Wake;
    use ynz_abi::{BG_ARG_KIND_HEAP_ARRAY, BG_ARG_KIND_RELEASED};

    // ─────────────────────────────────────────────────────────────────────
    // Test isolation for the three `*_alloc_free_parity` gates below (search
    // `ALLOC_COUNTER_ENABLED.store(true` in this file).
    //
    // `YNZ_ALLOC_COUNT`/`YNZ_FREE_COUNT` (lib.rs) are process-global counters gated by
    // a STICKY `ALLOC_COUNTER_ENABLED` latch that, once flipped true by any test, never
    // resets for the rest of the process's life. Any OTHER test in this binary that
    // allocates — directly or transitively (array/map/shape/channel construction) —
    // increments the SAME global counters once the latch is live, so a mutex serializing
    // only these three parity tests against each other is not enough: it does nothing to
    // stop the ~90+ unrelated tests in the same `ynz-runtime --lib` binary from polluting
    // one of these three tests' before/after measurement window if it happens to run
    // concurrently under `cargo test`'s default thread-per-test parallelism.
    //
    // The isolation that actually holds — independent of which harness invokes the outer
    // suite (`cargo test`, `cargo nextest`, or anything else) — is genuine OS-process
    // isolation, via `run_isolated_or_return` below. Each parity test's `#[test]` fn, as
    // its first action, re-execs the CURRENT test binary (`std::env::current_exe()`)
    // filtered to just itself (`--exact <qualified_name>`) in a brand-new child process,
    // then returns without running its own body — the re-exec'd CHILD process (detected
    // via the `YNZ_ALLOC_PARITY_CHILD` env var) is the one that actually runs the
    // measured body. A fresh process has fresh `ALLOC_COUNTER_ENABLED`/
    // `YNZ_ALLOC_COUNT`/`YNZ_FREE_COUNT` statics that no other concurrently-running test,
    // in any harness, can ever share — nothing is left to serialize in-process, so no
    // mutex is needed here.
    //
    // This mirrors the SAME authoritative house-style pattern the M2/M5 subprocess parity
    // gates already use (spawn an isolated process, communicate the result back — see
    // `runtime.rs`'s `YNZ_ALLOC_COUNTER_OUTPUT` file-based readback and
    // `crates/ynz-driver/tests/m2_state_machine_integration.rs`'s `Command::new(...ynz...)`
    // fixture runs) rather than inventing a second, ad hoc isolation scheme
    // (authoritative-derivation.md). Those gates spawn the compiled `ynz` PRODUCT binary,
    // because their fixtures are real `.ynz` programs; these three tests exercise private
    // crate-internal runtime C-ABI functions with no product binary to spawn, so they
    // isolate by re-exec'ing THIS test binary filtered to themselves instead — same
    // "isolate via a real OS process, read the result back" idea, applied at the layer
    // these tests actually operate at.
    //
    // Communicating the result back: unlike the file-based `YNZ_ALLOC_COUNTER_OUTPUT`
    // readback, this helper reads the child's PROCESS EXIT STATUS (libtest already fails
    // the process with a non-zero exit code and a panic message on stderr/stdout when a
    // `#[test]` fn panics) — no new file-based protocol needed; the parent surfaces the
    // child's captured stdout/stderr verbatim on failure so the original assertion
    // message is never lost.
    ///
    /// Ensures the calling `#[test]` fn's alloc/free-counting body runs in a fully
    /// isolated child process rather than the shared, parallel `cargo test` process.
    ///
    /// Returns `true` when called from the PARENT (a child was just spawned and awaited —
    /// the caller MUST `return` immediately without running its own body) and `false` when
    /// called from the re-exec'd CHILD (the caller should proceed to run its real body,
    /// which now executes in an otherwise-empty process with no other test's alloc/free
    /// activity able to contend with it).
    ///
    /// `qualified_test_name` must be the EXACT libtest path shown by `cargo test -- --list`
    /// for the calling `#[test]` fn (e.g.
    /// `"channel::tests::channel_drop_glue_frees_buffered_heap_elements_alloc_free_parity"`)
    /// — `--exact` requires an exact match, so a drifted name (a renamed test whose call
    /// site wasn't updated) fails LOUD (the child matches zero tests, libtest still exits
    /// 0 with "0 passed", parent's non-vacuous alloc_delta assertion — which never ran —
    /// would be silently skipped) rather than silently passing. Guarded by the child-count
    /// assertion below specifically to catch that drift.
    ///
    /// **Not run under Miri** (see `#[cfg(not(miri))]` on each caller below): Miri does not
    /// support `Command::spawn`/`posix_spawn` at all — confirmed live, M6 Phase 6b —
    /// (`error: unsupported operation: can't call foreign function \`posix_spawnattr_init\`
    /// on OS \`linux\`` / "this means the program tried to do something Miri does not
    /// support; it does not indicate a bug in the program" — Miri's own diagnostic, not an
    /// inferred workaround), and even with `-Zmiri-disable-isolation` the re-exec'd "child"
    /// has no real interpreted binary to spawn in the first place (Miri never produces a
    /// natively-executable artifact matching the interpreted test — `current_exe()` names
    /// the `cargo-miri` driver process, not the test binary). The alloc/free-parity
    /// assertions these tests make are still exercised under the ordinary
    /// `cargo test`/`cargo nextest` CI lane; only the Miri UB/leak scan is skipped for this
    /// specific process-isolation mechanism, not the behavior it tests.
    fn run_isolated_or_return(qualified_test_name: &str) -> bool {
        if std::env::var_os("YNZ_ALLOC_PARITY_CHILD").is_some() {
            return false; // we ARE the re-exec'd child — run the real measured body
        }
        let exe = std::env::current_exe()
            .expect("run_isolated_or_return: std::env::current_exe() failed");
        let output = std::process::Command::new(exe)
            .arg(qualified_test_name)
            .arg("--exact")
            .arg("--nocapture")
            .env("YNZ_ALLOC_PARITY_CHILD", "1")
            .output()
            .unwrap_or_else(|e| {
                panic!("run_isolated_or_return: failed to spawn isolated child for {qualified_test_name}: {e}")
            });
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Catches the qualified_test_name drift case named in the doc comment above: a
        // filter matching zero tests still exits 0 with "0 passed; ... 99 filtered out".
        assert!(
            stdout.contains("1 passed") || stdout.contains("1 failed"),
            "run_isolated_or_return: filter {qualified_test_name:?} matched zero tests in \
             the child process (stale/drifted name?) — child stdout:\n{stdout}"
        );
        assert!(
            output.status.success(),
            "alloc/free parity test {qualified_test_name} failed in its isolated child \
             process (exit {:?}):\n--- child stdout ---\n{stdout}\n--- child stderr ---\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
        );
        true // we are the parent — caller must return without running its own body
    }

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

    /// Construct a glue-less channel through the real C-ABI (null drop glue — the
    /// primitive-element form; per-element glue is exercised by the P2-4 parity tests below).
    fn make_chan(capacity: i64) -> *mut u8 {
        // SAFETY: null glue is the documented no-glue form.
        unsafe { ynz_channel_create(capacity, std::ptr::null_mut()) }
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
        let chan = make_chan(1);
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
        let chan = make_chan(1);
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
        let chan = make_chan(4);
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
        let chan_ptr = make_chan(2);
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

    /// v0.3-M6 P3-2 RED→GREEN: the caller's waker must be recorded in `recv_waiters`
    /// BEFORE `poll_recv` runs. `poll_recv` parks the waker in mpsc's SINGLE slot, where a
    /// later consumer's poll clobbers it — so with poll-then-record ordering, a final send
    /// landing in the gap (after consumer A's unregistered poll, after consumer C clobbered
    /// the slot, before A's late record) wakes only C, and A hangs permanently.
    ///
    /// The window lives INSIDE one `ynz_channel_recv_poll` call, so the deterministic probe
    /// drives the REAL extern fn with a manual `RawWaker` whose clone hook observes the
    /// ordering directly: the fn clones the waker at exactly two sites —
    /// `record_recv_waiter`'s push (recv_waiters mutex HELD → `try_lock` fails) and
    /// `poll_recv`'s mpsc slot registration (recv_waiters free → `try_lock` succeeds). At
    /// the mpsc-site clone, the fix's invariant is that the waker is ALREADY recorded.
    struct OrderProbe {
        chan: *mut u8,
        /// At the mpsc-slot registration, was the waker already in `recv_waiters`?
        registered_before_poll: AtomicBool,
        /// Vacuity guard: the probe actually observed the mpsc-slot clone.
        mpsc_clone_seen: AtomicBool,
        wakes: AtomicUsize,
    }

    const ORDER_PROBE_VTABLE: std::task::RawWakerVTable = std::task::RawWakerVTable::new(
        order_probe_clone,
        order_probe_wake,
        order_probe_wake_by_ref,
        order_probe_drop,
    );

    unsafe fn order_probe_clone(data: *const ()) -> std::task::RawWaker {
        let st = &*(data as *const OrderProbe);
        let chan = &*(st.chan as *const YnzChannel);
        // recv_waiters held ⇒ this is record_recv_waiter's own push-clone: skip.
        // recv_waiters free ⇒ this is poll_recv's mpsc slot-registration clone: inspect.
        if let Ok(waiters) = chan.recv_waiters.try_lock() {
            st.mpsc_clone_seen.store(true, Ordering::SeqCst);
            st.registered_before_poll
                .store(!waiters.is_empty(), Ordering::SeqCst);
        }
        std::task::RawWaker::new(data, &ORDER_PROBE_VTABLE)
    }
    unsafe fn order_probe_wake(data: *const ()) {
        (*(data as *const OrderProbe))
            .wakes
            .fetch_add(1, Ordering::SeqCst);
    }
    unsafe fn order_probe_wake_by_ref(data: *const ()) {
        (*(data as *const OrderProbe))
            .wakes
            .fetch_add(1, Ordering::SeqCst);
    }
    unsafe fn order_probe_drop(_data: *const ()) {}

    #[test]
    fn recv_poll_registers_waiter_before_polling() {
        let chan = make_chan(1);
        let probe = OrderProbe {
            chan,
            registered_before_poll: AtomicBool::new(false),
            mpsc_clone_seen: AtomicBool::new(false),
            wakes: AtomicUsize::new(0),
        };
        unsafe {
            let probe_waker = Waker::from_raw(std::task::RawWaker::new(
                &probe as *const OrderProbe as *const (),
                &ORDER_PROBE_VTABLE,
            ));
            let mut out: i64 = 0;
            let mut cx = Context::from_waker(&probe_waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;

            // Consumer A suspends on the empty channel through the real ABI fn.
            assert_eq!(
                ynz_channel_recv_poll(chan, &mut out, cx_ptr),
                CHANNEL_PENDING
            );
            assert!(
                probe.mpsc_clone_seen.load(Ordering::SeqCst),
                "probe never observed the mpsc slot registration — vacuous run"
            );
            assert!(
                probe.registered_before_poll.load(Ordering::SeqCst),
                "the waker must be recorded as a receive-waiter BEFORE poll_recv runs; \
                 poll-then-record leaves a gap where a clobbered mpsc slot + the channel's \
                 FINAL send wake nobody and the receiver hangs permanently (P3-2)"
            );

            // Semantic follow-through: a send after the Pending must wake the recorded waiter.
            let (_arc, send_waker) = make_waker();
            assert_eq!(send(chan, 5, &send_waker), CHANNEL_READY);
            assert!(
                probe.wakes.load(Ordering::SeqCst) > 0,
                "a successful send must wake the suspended receiver"
            );
            assert_eq!(ynz_channel_recv_poll(chan, &mut out, cx_ptr), CHANNEL_READY);
            assert_eq!(out, 5);
            ynz_channel_free(chan);
        }
    }

    /// v0.3-M6 P3-2 — the literal 3-party scenario Phase 4 targets: consumer A suspends and
    /// its mpsc SINGLE-slot registration is clobbered by consumer C's later poll, then a LIVE
    /// send fires. mpsc's native wake reaches only the slot registrant (C); the send path's
    /// drain-all over `recv_waiters` must wake A too, or — if this was the channel's FINAL
    /// send — A hangs permanently. Asserts value delivery on A's re-poll, not just the wake
    /// (the delivered value is what the lost-wakeup race actually forfeits). Deterministic
    /// single-threaded construction per this module's manual-`Waker` precedent.
    #[test]
    fn live_send_after_slot_clobber_wakes_clobbered_receiver() {
        let (arc_a, waker_a) = make_waker();
        let (_arc_c, waker_c) = make_waker();
        let chan = make_chan(2);
        unsafe {
            // A suspends first (mpsc slot = A, recorded in recv_waiters).
            let (code, _) = recv(chan, &waker_a);
            assert_eq!(code, CHANNEL_PENDING);
            // C suspends next — C's poll clobbers the mpsc single-slot waker (slot = C).
            let (code, _) = recv(chan, &waker_c);
            assert_eq!(code, CHANNEL_PENDING);

            // A live send fires while A's slot registration is clobbered.
            let wakes_a_before = arc_a.0.load(Ordering::SeqCst);
            let (_arc_s, send_waker) = make_waker();
            assert_eq!(send(chan, 42, &send_waker), CHANNEL_READY);
            assert!(
                arc_a.0.load(Ordering::SeqCst) > wakes_a_before,
                "a successful send must wake EVERY recorded receive-waiter — A's mpsc \
                 single-slot registration was clobbered by C, so without the drain-all \
                 wake A misses the channel's final send and hangs permanently (P3-2)"
            );
            // A re-polls and receives the sent value — delivery, not just a wake.
            let (code, val) = recv(chan, &waker_a);
            assert_eq!(code, CHANNEL_READY);
            assert_eq!(val, 42);
            // C re-polls and suspends again — one waiter wins, the rest re-register.
            let (code, _) = recv(chan, &waker_c);
            assert_eq!(code, CHANNEL_PENDING);
            ynz_channel_free(chan);
        }
    }

    /// recv on a drained + all-senders-dropped channel returns Closed (no more values coming).
    #[test]
    fn recv_on_closed_drained_returns_closed() {
        let (_arc, waker) = make_waker();
        let chan_ptr = make_chan(2);
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
        let chan = make_chan(1);
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
        let chan = make_chan(1);
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

    // ── v0.3-M6 Phase 5 (P2-4): drop-glue alloc=free parity at the runtime C-ABI ──────────
    //
    // FRAGO 027: the channel's last-ref drop is E2E-unreachable today (no codegen path
    // releases the creator's reference), so the parity gate drives the C-ABI directly —
    // ynz_channel_create with real per-type glue → buffer heap elements → ynz_channel_free.
    //
    // Isolation for these three tests does not depend on which harness runs the suite
    // (`cargo test`, `cargo nextest`, or anything else) — see the test-isolation block
    // above `run_isolated_or_return`'s definition: each parity test re-execs itself into
    // its own OS process, so the `ALLOC_COUNTER_ENABLED`/`YNZ_ALLOC_COUNT`/
    // `YNZ_FREE_COUNT` globals are never shared with any other concurrently-running test
    // regardless of the runner.

    /// Element drop glue for `channel<array<T>>` — the exact call codegen's synthesized
    /// `ynz_chan_drop_glue_array_*` makes.
    unsafe extern "C" fn array_elem_glue(bits: i64) {
        crate::ynz_array_drop(bits as *mut crate::YnzArray);
    }

    /// Element drop glue for `channel<map<K,V>>` — the exact call codegen's synthesized
    /// `ynz_chan_drop_glue_map_*` makes.
    unsafe extern "C" fn map_elem_glue(bits: i64) {
        crate::ynz_map_drop(bits as *mut crate::YnzMap);
    }

    /// Shape-LIKE glue, hand-written: typeck rejects `shape` channel elements today, so
    /// codegen has no shape arm — but the runtime mechanism is element-type-agnostic; this
    /// proves it against a plain counted 16-byte cell (FRAGO 027's array/map/shape coverage).
    unsafe extern "C" fn shape_cell_glue(bits: i64) {
        crate::ynz_free(bits as *mut u8, 16);
    }

    /// Construct a channel through the real C-ABI with real per-element drop glue.
    fn make_chan_with_glue(capacity: i64, glue: unsafe extern "C" fn(i64)) -> *mut u8 {
        // SAFETY: `glue` is a static extern "C" fn — callable for the process lifetime.
        unsafe { ynz_channel_create(capacity, glue as *mut u8) }
    }

    /// P2-4 parity gate (buffered elements): heap elements still sitting in the buffer at
    /// last-ref drop must be freed through the registered glue — alloc=free, NON-vacuously
    /// (M5 FRAGO-005: a zero-alloc parity pass proves nothing).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn channel_drop_glue_frees_buffered_heap_elements_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::channel_drop_glue_frees_buffered_heap_elements_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            // One channel per glue kind, several elements each, buffered and NEVER
            // drained — the exact P2-4 leak shape.
            let chan_arr = make_chan_with_glue(4, array_elem_glue);
            for _ in 0..3 {
                let arr = crate::ynz_array_new(8); // 2 counted allocs (header + buffer)
                assert_eq!(send(chan_arr, arr as i64, &waker), CHANNEL_READY);
            }
            let chan_map = make_chan_with_glue(4, map_elem_glue);
            for _ in 0..2 {
                let map = crate::ynz_map_new(8); // 5 counted allocs
                assert_eq!(send(chan_map, map as i64, &waker), CHANNEL_READY);
            }
            let chan_shape = make_chan_with_glue(4, shape_cell_glue);
            for _ in 0..2 {
                let cell = crate::ynz_alloc(16); // 1 counted alloc
                assert_eq!(send(chan_shape, cell as i64, &waker), CHANNEL_READY);
            }

            // Last-ref drop of each channel: the Drop impl's buffered drain must free
            // every element through its registered glue.
            ynz_channel_free(chan_arr);
            ynz_channel_free(chan_map);
            ynz_channel_free(chan_shape);
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        // Non-vacuous: 3 arrays × 2 + 2 maps × 5 + 2 cells × 1 = 18 counted allocs MUST
        // have been exercised.
        assert!(
            alloc_delta >= 18,
            "vacuous parity run: expected >= 18 counted allocs, saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "channel drop glue must free EVERY buffered heap element (P2-4): \
             alloc_delta={alloc_delta} free_delta={free_delta}"
        );
    }

    /// P2-4 parity gate (residual suspended send): a channel dropped while a send is still
    /// parked in `pending_sends` with a heap payload must glue-free BOTH the buffered element
    /// AND the parked entry's mirrored payload — disjoint sets, each exactly once (the Drop
    /// impl's no-double-free invariant, asserted by exact parity: one double-free or one leak
    /// would each break alloc==free).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn channel_drop_glue_frees_residual_pending_send_payload_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::channel_drop_glue_frees_residual_pending_send_payload_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            // Capacity-1 array channel: the first heap element fills the buffer; the second
            // send suspends, parking its payload in pending_sends (never buffered).
            let chan = make_chan_with_glue(1, array_elem_glue);
            let buffered = crate::ynz_array_new(8); // 2 counted allocs
            assert_eq!(send(chan, buffered as i64, &waker), CHANNEL_READY);
            let parked = crate::ynz_array_new(8); // 2 counted allocs
            assert_eq!(send(chan, parked as i64, &waker), CHANNEL_PENDING);
            assert_eq!(
                pending_send_count(chan),
                1,
                "the second send must be parked"
            );

            // Last-ref drop with the send STILL suspended.
            ynz_channel_free(chan);
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        assert!(
            alloc_delta >= 4,
            "vacuous parity run: expected >= 4 counted allocs, saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "channel drop must free the buffered element AND the parked pending-send \
             payload, each exactly once (P2-4): alloc_delta={alloc_delta} \
             free_delta={free_delta}"
        );
    }

    /// FRAGO 028 parity gate (cancellation path — LIVE today, unlike last-ref drop): a task
    /// suspended on a full heap-typed channel whose identity is then cancelled must have its
    /// parked payload freed through the registered glue at BOTH removal sites —
    /// `purge_pending_sends` (the exact call the real drop ladder / `ynz_handle_free` makes)
    /// and the insert-time stale-same-token/different-generation sweep. Pre-fix, both sites
    /// removed the entry glue-less: the payload was gone from `pending_sends` before the
    /// channel's own `Drop` could ever see it — leaked forever. Exact alloc==free parity also
    /// asserts no double-free against the `Drop` walk (a glued entry is removed, so `Drop`
    /// never sees it twice).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn cancellation_purge_and_stale_sweep_glue_free_parked_payloads_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::cancellation_purge_and_stale_sweep_glue_free_parked_payloads_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            // Capacity-1 array channel with REAL glue: the first heap element fills the
            // buffer; every later send suspends, parking its payload in pending_sends.
            let chan = make_chan_with_glue(1, array_elem_glue);
            let mut cx = Context::from_waker(&waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
            const T: u64 = 0xC0FFEE;

            let buffered = crate::ynz_array_new(8); // 2 counted allocs
            assert_eq!(
                channel_send_poll_guarded(chan, buffered as i64, cx_ptr, T, 7),
                CHANNEL_READY
            );

            // Site 1 — the real cancellation purge. Gen-7 caller suspends with a heap
            // payload, then is cancelled (purge_pending_sends is EXACTLY what the drop
            // ladder's kind-2 arm and ynz_handle_free call).
            let parked_purged = crate::ynz_array_new(8); // 2 counted allocs
            assert_eq!(
                channel_send_poll_guarded(chan, parked_purged as i64, cx_ptr, T, 7),
                CHANNEL_PENDING
            );
            assert_eq!(pending_send_count(chan), 1, "the send must be parked");
            purge_pending_sends(chan, 7);
            assert_eq!(
                pending_send_count(chan),
                0,
                "purge must remove the cancelled identity's entry"
            );

            // Site 2 — the insert-time stale sweep. Gen-8 caller suspends at token T and
            // "dies" WITHOUT a purge (the missed-path window); gen-9 reusing the same token
            // suspends next, and the insert sweep removes the dead gen-8 entry — which must
            // glue-free its parked payload.
            let parked_swept = crate::ynz_array_new(8); // 2 counted allocs
            assert_eq!(
                channel_send_poll_guarded(chan, parked_swept as i64, cx_ptr, T, 8),
                CHANNEL_PENDING
            );
            let parked_residual = crate::ynz_array_new(8); // 2 counted allocs
            assert_eq!(
                channel_send_poll_guarded(chan, parked_residual as i64, cx_ptr, T, 9),
                CHANNEL_PENDING
            );
            assert_eq!(
                pending_send_count(chan),
                1,
                "the stale gen-8 entry must be swept on insert"
            );

            // Last-ref drop frees the buffered element + the residual gen-9 entry — exact
            // parity proves the purge/sweep glue calls freed ONLY their own payloads.
            ynz_channel_free(chan);
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        // Non-vacuous (M5 FRAGO-005): 4 arrays x 2 counted allocs each MUST be exercised.
        assert!(
            alloc_delta >= 8,
            "vacuous parity run: expected >= 8 counted allocs, saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "cancellation-path removal must glue-free each parked payload exactly once \
             (FRAGO 028): alloc_delta={alloc_delta} free_delta={free_delta}"
        );
    }

    // ── spawn-arg ownership hand-off: a sent ladder-owned payload leaves the ladder ──────
    //
    // A `background` task's heap-cloned array argument is owned by the task's drop ladder
    // (`BgArgDropEntry` kind HEAP_ARRAY → `ynz_array_drop` at task retire). When the task
    // sends that pointer into a channel, the channel owns it too — and pre-fix the ladder
    // freed it under the receiver (`got.count()` read a freed header). These tests plant the
    // exact ladder shape codegen emits (frame slot + descriptor), publish the task as the
    // current drive the way `SpawnStateFnFuture::poll` does, send through the real C-ABI,
    // and assert the ladder let go — by descriptor kind AND by exact alloc=free parity
    // (a double free or a leak each breaks parity).

    /// The ladder codegen would emit for `background f(wire, rows)`: a 48-byte frame (32-byte
    /// header + 2 param slots) with `payload_bits` in slot 1 (byte offset 40), and ONE
    /// HEAP_ARRAY descriptor naming that slot. Returns the descriptor pointer (readable while
    /// the future is alive — the future frees it on drop) and the future that owns both.
    unsafe fn plant_array_ladder(
        payload_bits: i64,
    ) -> (*mut BgArgDropEntry, crate::runtime::SpawnStateFnFuture) {
        plant_array_ladders(&[payload_bits])
    }

    /// N-descriptor form of [`plant_array_ladder`] — what codegen emits for
    /// `background f(wire, a, b, ...)`: one HEAP_ARRAY descriptor per heap-cloned array, in
    /// consecutive 8-byte frame slots starting at byte offset 40 (slot 0 at 32 is the channel).
    /// `payloads[i]` lands in slot `1 + i`; the returned descriptor pointer indexes the same way.
    unsafe fn plant_array_ladders(
        payloads: &[i64],
    ) -> (*mut BgArgDropEntry, crate::runtime::SpawnStateFnFuture) {
        const FIRST_PAYLOAD_SLOT_OFFSET: u64 = 40;
        let frame_size = FIRST_PAYLOAD_SLOT_OFFSET as usize + 8 * payloads.len();
        let frame = crate::ynz_alloc_zeroed(frame_size);
        let descs = crate::ynz_alloc(std::mem::size_of::<BgArgDropEntry>() * payloads.len())
            as *mut BgArgDropEntry;
        for (i, bits) in payloads.iter().enumerate() {
            let byte_offset = FIRST_PAYLOAD_SLOT_OFFSET + 8 * i as u64;
            *(frame.add(byte_offset as usize) as *mut i64) = *bits;
            descs.add(i).write(BgArgDropEntry {
                byte_offset,
                kind: BG_ARG_KIND_HEAP_ARRAY,
                size: 0,
            });
        }
        unsafe extern "C-unwind" fn never_polled(_frame: *mut u8, _waker: *mut u8) -> i32 {
            1
        }
        let fut = crate::runtime::SpawnStateFnFuture::new(
            never_polled,
            frame,
            frame_size as i64,
            -1,
            descs,
            payloads.len() as i64,
        );
        (descs, fut)
    }

    /// Multi-descriptor selectivity: a task holding TWO ladder-owned arrays sends ONE. The
    /// release walk must flip exactly the descriptor whose slot holds the sent bits — the other
    /// stays HEAP_ARRAY and is freed by the ladder at retire, the sent one by the channel. Every
    /// other test here plants a single descriptor, so this is the only place the walk faces a
    /// live non-matching sibling (releasing it too = a leak; releasing it INSTEAD = a UAF plus a
    /// leak; both break exact parity, and the kind assertions name which one happened).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn send_of_one_of_two_ladder_owned_payloads_releases_only_that_slot_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::send_of_one_of_two_ladder_owned_payloads_releases_only_that_slot_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            let rows = crate::ynz_array_new(8); // sent: 2 counted allocs
            let scratch = crate::ynz_array_new(8); // kept by the task: 2 counted allocs
            let (descs, fut) = plant_array_ladders(&[rows as i64, scratch as i64]);
            let chan = make_chan_with_glue(4, array_elem_glue);
            {
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(send(chan, rows as i64, &waker), CHANNEL_READY);
            }
            assert_eq!(
                (*descs).kind,
                BG_ARG_KIND_RELEASED,
                "the SENT array's descriptor must be released"
            );
            assert_eq!(
                (*descs.add(1)).kind,
                BG_ARG_KIND_HEAP_ARRAY,
                "the UN-SENT sibling's descriptor must be untouched — the ladder still owns it"
            );
            drop(fut); // ladder: skips `rows`, frees `scratch` + frame + descriptors
            ynz_channel_free(chan); // teardown glue: frees `rows` — its ONLY drop
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        assert!(
            alloc_delta >= 6,
            "vacuous parity run: expected >= 6 counted allocs (2 arrays + frame + descriptors), \
             saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "exactly one owner per payload: the channel frees the sent array, the ladder frees \
             the kept one; alloc_delta={alloc_delta} free_delta={free_delta}"
        );
    }

    /// RELEASED is terminal: releasing the same bits a second time (the same payload handed
    /// off twice — a repeat send, or a send followed by a handle return of the same pointer)
    /// matches nothing. `release_ladder_payload` only considers HEAP_SHAPE / HEAP_ARRAY
    /// descriptors, so an already-RELEASED slot can never be re-counted or re-flipped.
    #[test]
    fn release_ladder_payload_is_idempotent_released_is_terminal() {
        unsafe {
            let rows = crate::ynz_array_new(8);
            let (descs, fut) = plant_array_ladder(rows as i64);
            let drive = fut.drive_identity();
            assert_eq!(
                release_ladder_payload(&drive, rows as i64),
                1,
                "first hand-off releases the one matching slot"
            );
            assert_eq!((*descs).kind, BG_ARG_KIND_RELEASED);
            assert_eq!(
                release_ladder_payload(&drive, rows as i64),
                0,
                "a repeat hand-off of the same payload must match nothing — RELEASED is terminal"
            );
            assert_eq!((*descs).kind, BG_ARG_KIND_RELEASED);
            // Bits that match no slot never touch a descriptor either.
            assert_eq!(release_ladder_payload(&drive, 0x5EED), 0);
            // The ladder skips the released slot; the test frees `rows` as its owner now.
            drop(fut);
            crate::ynz_array_drop(rows);
        }
    }

    /// Parked → CLOSED: the send parks on a full channel (the park RELEASES the payload from the
    /// sender's ladder — the pending entry owns it from here), then the receiver drops, and the
    /// task's re-poll observes `Poll::Ready(Err(()))`. The endpoint future discarded its
    /// `SendError(v)`, and the ladder has already let go — so the entry's `value_bits` mirror is
    /// the payload's LAST owner and the re-poll must free it through the channel's glue. Pre-fix
    /// the entry was `remove`d with no glue call: nobody owned the payload (a leak the parity
    /// gate catches: alloc = free + 2). Unreachable in production until channels can close
    /// (M8 Phase 4) — guarded now because the park-time release already assumes this debt is paid.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn parked_send_closed_on_repoll_frees_the_orphaned_payload_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::parked_send_closed_on_repoll_frees_the_orphaned_payload_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            let chan_ptr = make_chan_with_glue(1, array_elem_glue);
            let filler = crate::ynz_array_new(8); // fills the single slot: 2 counted allocs
            assert_eq!(send(chan_ptr, filler as i64, &waker), CHANNEL_READY);
            let rows = crate::ynz_array_new(8); // the task's heap clone: 2 counted allocs
            let (descs, fut) = plant_array_ladder(rows as i64);
            {
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(
                    send(chan_ptr, rows as i64, &waker),
                    CHANNEL_PENDING,
                    "a send on a full channel must park, not block"
                );
            }
            assert_eq!(pending_send_count(chan_ptr), 1);
            assert_eq!(
                (*descs).kind,
                BG_ARG_KIND_RELEASED,
                "the park released the payload"
            );

            // The receiver goes away (what a closed channel will do once M8 ships close
            // semantics). Its buffered i64 words — `filler` — are dropped by Tokio WITHOUT
            // glue; the test frees `filler` by hand below so the parity assertion is about the
            // PARKED payload alone.
            let chan = &*(chan_ptr as *const YnzChannel);
            let (_dead_tx, dead_rx) = mpsc::channel::<i64>(1);
            let real_rx = std::mem::replace(&mut *lock_or_recover(&chan.receiver), dead_rx);
            drop(real_rx);
            crate::ynz_array_drop(filler);

            // The task's re-poll of ITS parked send (same token/generation; value 0, as the
            // resumed poll passes) observes the closed receiver.
            {
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(
                    send(chan_ptr, 0, &waker),
                    CHANNEL_CLOSED,
                    "the re-poll of a parked send on a closed channel reports CLOSED"
                );
            }
            assert_eq!(
                pending_send_count(chan_ptr),
                0,
                "the closed entry must be removed — and, being the payload's last owner, glued"
            );
            drop(fut); // ladder: skips `rows` (RELEASED at park), frees frame + descriptor
            ynz_channel_free(chan_ptr); // nothing buffered, nothing parked: glue runs for nobody
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        assert!(
            alloc_delta >= 6,
            "vacuous parity run: expected >= 6 counted allocs, saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "a parked payload whose receiver closed must be freed exactly once — by the \
             re-poll's glue call (a +2 gap = the pre-fix orphan leak); alloc_delta={alloc_delta} \
             free_delta={free_delta}"
        );
    }

    /// Ready path: `try_send` accepts the ladder-owned array → the descriptor flips to
    /// RELEASED, the ladder skips it at task retire, and the channel's teardown glue frees it
    /// exactly once. Pre-fix: ladder free + glue free = a double free (parity breaks or the
    /// process dies on the second `ynz_array_drop`).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn send_of_ladder_owned_payload_releases_ladder_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::send_of_ladder_owned_payload_releases_ladder_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            let rows = crate::ynz_array_new(8); // the task's heap clone: 2 counted allocs
            let (descs, fut) = plant_array_ladder(rows as i64); // frame + descriptor: 2 more
            let chan = make_chan_with_glue(4, array_elem_glue);
            {
                // What `SpawnStateFnFuture::poll` publishes around the resume-fn call.
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(send(chan, rows as i64, &waker), CHANNEL_READY);
            }
            assert_eq!(
                (*descs).kind,
                BG_ARG_KIND_RELEASED,
                "an accepted send of a ladder-owned payload must rewrite its descriptor to \
                 RELEASED — otherwise the ladder frees what the channel now owns"
            );
            drop(fut); // ladder: skips `rows`, frees frame + descriptor
            ynz_channel_free(chan); // last ref: teardown glue frees `rows` — its ONLY drop
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        assert!(
            alloc_delta >= 4,
            "vacuous parity run: expected >= 4 counted allocs (array 2 + frame + descriptor), \
             saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "a sent spawn-arg payload must be freed exactly once (by the channel, not the \
             ladder): alloc_delta={alloc_delta} free_delta={free_delta}"
        );
    }

    /// Parked path: the channel is full, so the send suspends and the parked entry owns the
    /// payload from that moment (purge/teardown glue frees it). The release must happen at
    /// park time — the resumed poll passes value 0 and could never match the slot.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "process re-exec isolation unsupported under Miri (posix_spawn); see \
                  run_isolated_or_return's doc comment — behavior is still covered by \
                  cargo test/nextest"
    )]
    fn parked_send_of_ladder_owned_payload_releases_ladder_alloc_free_parity() {
        if run_isolated_or_return(
            "channel::tests::parked_send_of_ladder_owned_payload_releases_ladder_alloc_free_parity",
        ) {
            return;
        }
        crate::ALLOC_COUNTER_ENABLED.store(true, Ordering::Relaxed);
        let (_arc, waker) = make_waker();
        let alloc_before = crate::ynz_alloc_count();
        let free_before = crate::ynz_free_count();
        unsafe {
            let chan = make_chan_with_glue(1, array_elem_glue);
            let filler = crate::ynz_array_new(8); // fills the single slot: 2 counted allocs
            assert_eq!(send(chan, filler as i64, &waker), CHANNEL_READY);
            let rows = crate::ynz_array_new(8); // the task's heap clone: 2 counted allocs
            let (descs, fut) = plant_array_ladder(rows as i64);
            {
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(
                    send(chan, rows as i64, &waker),
                    CHANNEL_PENDING,
                    "a send on a full channel must park, not block"
                );
            }
            assert_eq!(pending_send_count(chan), 1);
            assert_eq!(
                (*descs).kind,
                BG_ARG_KIND_RELEASED,
                "a PARKED send has handed the payload to the channel's pending entry — the \
                 ladder must release it at park time"
            );
            drop(fut); // ladder: skips `rows`
            ynz_channel_free(chan); // teardown glue: buffered `filler` + parked `rows`, once each
        }
        let alloc_delta = crate::ynz_alloc_count() - alloc_before;
        let free_delta = crate::ynz_free_count() - free_before;
        assert!(
            alloc_delta >= 6,
            "vacuous parity run: expected >= 6 counted allocs, saw {alloc_delta}"
        );
        assert_eq!(
            alloc_delta, free_delta,
            "buffered + parked spawn-arg payloads must each be freed exactly once: \
             alloc_delta={alloc_delta} free_delta={free_delta}"
        );
    }

    /// Non-release cases — the ladder keeps ownership and frees the payload itself:
    /// (a) a drive with no ladder (the sync entrypoint) sends the same bits — nothing to
    ///     release; (b) a glue-less (`channel<int>`) channel never compares bits at all, so an
    ///     `int` that happens to equal a live pointer cannot release anything; (c) a CLOSED send
    ///     never took the value, so the sender still owns it.
    #[test]
    fn ladder_is_untouched_when_the_channel_does_not_take_ownership() {
        let (_arc, waker) = make_waker();
        unsafe {
            // (a) ladder-less drive.
            let rows = crate::ynz_array_new(8);
            let (descs, fut) = plant_array_ladder(rows as i64);
            let chan = make_chan_with_glue(4, array_elem_glue);
            {
                let _drive = DriveGuard::enter(DriveIdentity::ladderless(7));
                assert_eq!(send(chan, rows as i64, &waker), CHANNEL_READY);
            }
            assert_eq!((*descs).kind, BG_ARG_KIND_HEAP_ARRAY);
            // Take the value back out so the channel does not also own it at teardown.
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, rows as i64));
            drop(fut); // the ladder frees `rows`
            ynz_channel_free(chan);

            // (b) glue-less channel: the payload is an int by type; bits are never compared.
            let rows = crate::ynz_array_new(8);
            let (descs, fut) = plant_array_ladder(rows as i64);
            let int_chan = make_chan(4);
            {
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(send(int_chan, rows as i64, &waker), CHANNEL_READY);
            }
            assert_eq!(
                (*descs).kind,
                BG_ARG_KIND_HEAP_ARRAY,
                "a channel without element drop glue carries no heap payload — it must never \
                 release a ladder slot on a coincidental bit match"
            );
            assert_eq!(recv(int_chan, &waker), (CHANNEL_READY, rows as i64));
            drop(fut);
            ynz_channel_free(int_chan);

            // (c) closed channel: the value was not taken.
            let rows = crate::ynz_array_new(8);
            let (descs, fut) = plant_array_ladder(rows as i64);
            let closed_ptr = make_chan_with_glue(4, array_elem_glue);
            let closed = &*(closed_ptr as *const YnzChannel);
            let (_dead_tx, dead_rx) = mpsc::channel::<i64>(1);
            let real_rx = std::mem::replace(&mut *lock_or_recover(&closed.receiver), dead_rx);
            drop(real_rx);
            {
                let _drive = DriveGuard::enter(fut.drive_identity());
                assert_eq!(send(closed_ptr, rows as i64, &waker), CHANNEL_CLOSED);
            }
            assert_eq!(
                (*descs).kind,
                BG_ARG_KIND_HEAP_ARRAY,
                "a CLOSED send never took the payload — the sender's ladder still owns it"
            );
            drop(fut); // the ladder frees `rows`
            ynz_channel_free(closed_ptr);
        }
    }

    /// Refcount balance: share bumps, free releases; the object survives until the LAST free.
    #[test]
    fn share_then_free_is_refcount_balanced() {
        let (_arc, waker) = make_waker();
        let chan = make_chan(2);
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
