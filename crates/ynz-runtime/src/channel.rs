//! Bounded task-communication channels (`channel<T>`) — the deadlock-safe C-ABI substrate
//! (v0.3-M4 Phase 1, risk R1).
//!
//! # Why this module exists — the R1 deadlock-safety core
//!
//! A `channel<T>` value is a bounded [`tokio::sync::mpsc`] channel. `send()` on a full channel
//! MUST suspend the calling task via the poll-yield state-machine protocol — it must NEVER make a
//! synchronous blocking call (`block_on`, `blocking_send`, `spawn_blocking`, `thread::sleep`,
//! `.park()`). A blocking call here is the exact M2 `block_on`-HALT corpse class; the entire reason
//! Phase 1 exists is to prove send-on-full backpressure suspends-not-blocks.
//!
//! This module makes durable the design the P0 R5 spike proved through the real compiler
//! (`p0-spike/R5-composed-suspension-spike-report.md`): the channel endpoint futures live in the
//! runtime-owned channel object HERE, never the frame header's type-punned `sleep_handle` slot
//! (trap door 1c avoided by construction — the channel object is heap-owned and freed exactly once
//! via [`ynz_channel_free`]). Every suspension polls the in-flight endpoint future with the
//! FORWARDED waker from the enclosing state-machine's `poll` (the `ynz_rt_async_sleep_poll`
//! no-fabricated-waker discipline).
//!
//! # The poll ABI (mirrors `ynz_rt_async_sleep_poll`)
//!
//! [`ynz_channel_send_poll`] / [`ynz_channel_recv_poll`] return an `i32`:
//!
//! - `0` = **Ready** — send: the value was accepted; recv: a value was written to `out`.
//! - `1` = **Pending** — the task must save `resume_point` and return Pending to the executor. The
//!   endpoint future has registered the forwarded waker; the task is re-driven when capacity frees
//!   (send) or a value arrives (recv). No tight-loop polling, no thread block.
//! - `2` = **Closed** — send: the receiver was dropped (a typed Yinz channel-closed `errors` value,
//!   NEVER the raw Tokio `SendError` — Lock 8); recv: all senders dropped AND the buffer is drained.
//!
//! The two mpsc endpoints wake each other independently (tokio registers the receiver's waker on
//! empty and the sender's waker on full), so a producer suspended on send-on-full and a consumer
//! polling receive can never form a circular wait — the R5 composed-suspension proof.

use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::sync::mpsc;

/// Poll result: the operation completed (value accepted / value delivered).
const CHANNEL_READY: i32 = 0;
/// Poll result: the task must suspend (save resume_point, return Pending to the executor).
const CHANNEL_PENDING: i32 = 1;
/// Poll result: the channel is closed (send: receiver dropped; recv: drained and all senders gone).
const CHANNEL_CLOSED: i32 = 2;

/// The in-flight `send()` endpoint future: owns a cloned `Sender` + the pending value, resolves to
/// `Ok(())` when the value is accepted or `Err(())` when the receiver has been dropped (closed).
type PendingSend = Pin<Box<dyn Future<Output = Result<(), ()>> + Send>>;

/// Extract a human-readable message from a caught-panic payload (the same `&str`/`String` downcast
/// `ynz_rt_async_sleep_poll` performs). Shared by both channel poll shims so the panic-parity
/// handling lives in exactly one place.
fn panic_payload_msg(e: &Box<dyn Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// A Yinz `channel<T>` value at the runtime ABI boundary.
///
/// All element types cross the ABI as i64 bit patterns (int/bool/float bits, or a pointer cast to
/// i64 for shape/string elements) — the same i64-slot convention `array`/`map` use.
///
/// Holds BOTH endpoints of one bounded mpsc channel: `sender` (cloneable — multi-producer) and
/// `receiver` (single-consumer). The single combined object is the Phase-1 value model; Phase 2's
/// handle-form wires cloned senders/receivers into separate tasks.
pub struct YnzChannel {
    sender: mpsc::Sender<i64>,
    receiver: mpsc::Receiver<i64>,
    /// The in-flight `send()` endpoint future for a send that suspended on a full channel.
    ///
    /// This is the "endpoint future lives in the runtime object" discipline: the future owns a
    /// CLONED `Sender` plus the pending value (no self-reference into `self.sender`), is created
    /// lazily on the first send-on-full suspension, is re-polled with the forwarded waker on each
    /// resume, and is cleared to `None` once it resolves. Exactly one send may be in flight per
    /// channel object at a time (a single-owner channel suspends at one point at a time).
    pending_send: Option<PendingSend>,
}

/// Construct a bounded channel with `capacity` slots. Returns an opaque handle pointer.
///
/// Bounded by construction (stdlib-design.md Rule 4): there is no unbounded constructor. The primary
/// gate is typeck, which resolves the default (64) / explicit `channel<T>(N)` capacity surface and
/// rejects a non-positive literal capacity at compile time (`crates/ynz-typeck/src/check.rs`
/// channel-construction path). This shim additionally clamps `capacity` to at least 1 as a
/// release-mode defensive floor because `mpsc::channel(0)` panic-aborts — a codegen/typeck
/// regression that let a `< 1` capacity reach here is caught loudly in debug builds by the
/// `debug_assert!` below, while the clamp keeps release builds from aborting.
///
/// # Side effects
/// Heap-allocates one `YnzChannel` (`Box`). The caller owns the returned pointer and MUST free it
/// with [`ynz_channel_free`] exactly once (alloc=free-gated).
///
/// # Safety
/// The returned pointer is valid until passed to [`ynz_channel_free`]. Do not use after free.
#[no_mangle]
pub extern "C" fn ynz_channel_create(capacity: i64) -> *mut u8 {
    // Typeck rejects a non-positive capacity before codegen; a `< 1` value arriving here means a
    // typeck/codegen regression — trip loudly in debug while the release clamp prevents the
    // `mpsc::channel(0)` panic-abort.
    debug_assert!(
        capacity >= 1,
        "ynz_channel_create: capacity must be >= 1 (typeck rejects non-positive capacity); got {capacity}"
    );
    let cap = if capacity < 1 { 1 } else { capacity as usize };
    let (sender, receiver) = mpsc::channel::<i64>(cap);
    let chan = Box::new(YnzChannel {
        sender,
        receiver,
        pending_send: None,
    });
    Box::into_raw(chan) as *mut u8
}

/// Poll a `send(value)` on the channel `chan_ptr`, forwarding the enclosing task's waker.
///
/// # Flow
/// 1. If a send is already in flight (a prior call suspended on a full channel), re-poll THAT
///    future — `value` is ignored (the in-flight future already captured its value).
/// 2. Otherwise `try_send(value)`: on success return [`CHANNEL_READY`]; on `Full` create the
///    boxed endpoint future (`sender.clone().send(value)`) and poll it once to register the waker;
///    on `Closed` return [`CHANNEL_CLOSED`].
///
/// Never makes a synchronous blocking call — a full channel yields [`CHANNEL_PENDING`] and the task
/// suspends via the state machine. This is the R1 no-blocking-call guarantee in code.
///
/// # Failure modes
/// - Receiver dropped → [`CHANNEL_CLOSED`] (the caller maps this to a typed Yinz channel-closed
///   `errors` value — never the raw Tokio `SendError`, Lock 8). The unsent value is dropped by
///   ownership; never a silent success.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — one `try_send` or one endpoint-future poll; boxes one future on the
/// first suspension, frees it on resolution.
///
/// # Safety
/// - `chan_ptr` must be a non-null pointer from [`ynz_channel_create`], not yet freed.
/// - `waker_ctx` must point to a live `&mut Context<'_>` for the duration of this call (the same
///   context passed into the enclosing state machine's `Future::poll`).
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_send_poll(
    chan_ptr: *mut u8,
    value: i64,
    waker_ctx: *mut u8,
) -> i32 {
    // Mirror ynz_rt_async_sleep_poll's panic discipline: a panic inside the poll is caught and
    // reported as CHANNEL_PENDING so the enclosing state-machine frame is not corrupted (the task's
    // overall panic propagation is handled by the Tokio task wrapper around the state machine).
    let result = std::panic::catch_unwind(|| {
        // SAFETY: chan_ptr came from ynz_channel_create (Box::into_raw of Box<YnzChannel>); it is
        // valid, aligned, and exclusively borrowed for this call.
        let chan = &mut *(chan_ptr as *mut YnzChannel);
        // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing state-machine poll.
        let cx = &mut *(waker_ctx as *mut Context<'_>);

        // Re-poll an already-suspended send (do NOT create a second future / accept a second value).
        if let Some(fut) = chan.pending_send.as_mut() {
            return match fut.as_mut().poll(cx) {
                Poll::Pending => CHANNEL_PENDING,
                Poll::Ready(Ok(())) => {
                    chan.pending_send = None;
                    CHANNEL_READY
                }
                Poll::Ready(Err(())) => {
                    chan.pending_send = None;
                    CHANNEL_CLOSED
                }
            };
        }

        // First attempt: non-blocking try_send. On a non-full channel this is the fast Ready path
        // (mirrors the sleep first-poll-Ready fast path — no suspension state needed).
        match chan.sender.try_send(value) {
            Ok(()) => CHANNEL_READY,
            Err(mpsc::error::TrySendError::Closed(_)) => CHANNEL_CLOSED,
            Err(mpsc::error::TrySendError::Full(v)) => {
                // Backpressure: the channel is full. Create the endpoint future owning a cloned
                // sender + the value (no self-reference into chan.sender), poll it once to register
                // the forwarded waker, and suspend if it can't complete immediately.
                let sender = chan.sender.clone();
                let mut fut: PendingSend =
                    Box::pin(async move { sender.send(v).await.map_err(|_| ()) });
                let poll = fut.as_mut().poll(cx);
                match poll {
                    Poll::Pending => {
                        chan.pending_send = Some(fut);
                        CHANNEL_PENDING
                    }
                    Poll::Ready(Ok(())) => CHANNEL_READY,
                    Poll::Ready(Err(())) => CHANNEL_CLOSED,
                }
            }
        }
    });
    match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "ynz runtime: ynz_channel_send_poll panicked (returning Pending): {}",
                panic_payload_msg(&e)
            );
            CHANNEL_PENDING
        }
    }
}

/// Poll a `receive()` on the channel `chan_ptr`, forwarding the enclosing task's waker.
///
/// # Flow
/// `receiver.poll_recv(cx)`: on `Ready(Some(v))` write `v` to `out` and return [`CHANNEL_READY`];
/// on `Ready(None)` (all senders dropped AND buffer drained) return [`CHANNEL_CLOSED`]; on
/// `Pending` (empty channel) return [`CHANNEL_PENDING`] — the task suspends until a value arrives.
///
/// `poll_recv` is natively poll-based and re-entrant, so no in-flight future is stored for receive.
///
/// # Side effects
/// Time: O(1)  Space: O(1) — one `poll_recv`; writes `*out` only on the Ready path.
///
/// # Safety
/// - `chan_ptr` must be a non-null pointer from [`ynz_channel_create`], not yet freed.
/// - `out` must point to a writable `i64` (the delivered value is written only on [`CHANNEL_READY`]).
/// - `waker_ctx` must point to a live `&mut Context<'_>` for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_recv_poll(
    chan_ptr: *mut u8,
    out: *mut i64,
    waker_ctx: *mut u8,
) -> i32 {
    // Mirror ynz_rt_async_sleep_poll's panic discipline (see ynz_channel_send_poll): catch a poll
    // panic and report CHANNEL_PENDING so the enclosing frame is not corrupted.
    let result = std::panic::catch_unwind(|| {
        // SAFETY: chan_ptr came from ynz_channel_create; valid, aligned, exclusively borrowed here.
        let chan = &mut *(chan_ptr as *mut YnzChannel);
        // SAFETY: waker_ctx was cast from &mut Context<'_> by the enclosing state-machine poll.
        let cx = &mut *(waker_ctx as *mut Context<'_>);
        match chan.receiver.poll_recv(cx) {
            Poll::Ready(Some(v)) => {
                // SAFETY: out points to a writable i64 (caller guarantee).
                *out = v;
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

/// Free the channel object created by [`ynz_channel_create`].
///
/// Reconstructs the `Box<YnzChannel>` and drops it — dropping both endpoints (closing the channel)
/// and any in-flight `pending_send` future. Freed exactly once; the alloc=free proof pairs this
/// with [`ynz_channel_create`]. A null pointer is a no-op (safe under cancellation-drop paths).
///
/// # Safety
/// `chan_ptr` must be a pointer from [`ynz_channel_create`] not yet freed, or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_channel_free(chan_ptr: *mut u8) {
    if chan_ptr.is_null() {
        return;
    }
    // SAFETY: reconstructing Box<YnzChannel> is the inverse of Box::into_raw in ynz_channel_create.
    drop(Box::from_raw(chan_ptr as *mut YnzChannel));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::{Wake, Waker};

    /// A minimal counting waker — proves the poll ABI works WITHOUT a Tokio runtime (mpsc's waker
    /// registration is runtime-agnostic) and lets a test assert wakeups happened.
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
    unsafe fn send(chan: *mut u8, value: i64, waker: &Waker) -> i32 {
        let mut cx = Context::from_waker(waker);
        let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
        ynz_channel_send_poll(chan, value, cx_ptr)
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

            // Second send finds the channel FULL. It must return Pending — NOT block the thread.
            // This returning at all is the proof it did not block.
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

    /// recv on an empty (but open) channel suspends; recv after a value arrives delivers it.
    #[test]
    fn recv_on_empty_suspends_then_delivers() {
        let (_arc, waker) = make_waker();
        let chan = ynz_channel_create(4);
        unsafe {
            // Empty channel → recv suspends (Pending), never blocks.
            let (code, _) = recv(chan, &waker);
            assert_eq!(
                code, CHANNEL_PENDING,
                "recv on empty must SUSPEND, never block"
            );
            // A value arrives (send is Ready — channel not full).
            assert_eq!(send(chan, 42, &waker), CHANNEL_READY);
            // Now recv delivers it.
            assert_eq!(recv(chan, &waker), (CHANNEL_READY, 42));
            ynz_channel_free(chan);
        }
    }

    /// send to a closed channel (receiver dropped) returns Closed — the typed-`errors` signal,
    /// never a silent success. Uses a raw mpsc split so the receiver can be dropped independently.
    #[test]
    fn send_to_closed_returns_closed() {
        let (_arc, waker) = make_waker();
        // Build a channel object, then drop its receiver to simulate a dropped consumer.
        let chan_ptr = ynz_channel_create(2);
        unsafe {
            // Drop the receiver by reconstructing the box, taking the receiver, and dropping it,
            // then re-leaking the (now receiver-closed) object so send sees a closed channel.
            let mut chan = Box::from_raw(chan_ptr as *mut YnzChannel);
            // Replace the receiver with a fresh detached one and drop the real one → close.
            let (_dead_tx, dead_rx) = mpsc::channel::<i64>(1);
            let real_rx = std::mem::replace(&mut chan.receiver, dead_rx);
            drop(real_rx); // original receiver dropped → chan.sender now sees Closed
            let reptr = Box::into_raw(chan) as *mut u8;

            assert_eq!(
                send(reptr, 99, &waker),
                CHANNEL_CLOSED,
                "send to a dropped-receiver channel must return Closed (typed errors), never Ready"
            );
            ynz_channel_free(reptr);
        }
    }

    /// recv on a drained + all-senders-dropped channel returns Closed (no more values coming).
    #[test]
    fn recv_on_closed_drained_returns_closed() {
        let (_arc, waker) = make_waker();
        let chan_ptr = ynz_channel_create(2);
        unsafe {
            // Drop the sender so the receiver observes closure once drained.
            let mut chan = Box::from_raw(chan_ptr as *mut YnzChannel);
            let (dead_tx, _dead_rx) = mpsc::channel::<i64>(1);
            let real_tx = std::mem::replace(&mut chan.sender, dead_tx);
            drop(real_tx); // all real senders gone
            let reptr = Box::into_raw(chan) as *mut u8;

            let (code, _) = recv(reptr, &waker);
            assert_eq!(
                code, CHANNEL_CLOSED,
                "recv on a closed + drained channel must return Closed"
            );
            ynz_channel_free(reptr);
        }
    }
}
