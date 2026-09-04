//! Loom model-checking harness for the runtime-owned channel synchronization state
//! (v0.3-M8 Phase 3). Compiled and run ONLY under:
//!
//! ```text
//! RUSTFLAGS='--cfg loom' cargo test -p ynz-runtime --release --lib -- loom_
//! ```
//!
//! (`LOOM_MAX_PREEMPTIONS=<n>` bounds the exploration; unset = exhaustive. Each test prints
//! the interleaving count and wall time it explored under `--nocapture`.)
//!
//! # What this covers, and how it has teeth
//!
//! Every model drives the REAL `channel.rs`/`runtime.rs`/`handle.rs` code — the extern-C
//! poll shims, the one keyed send core, the one shared purge helper, the real
//! `SpawnStateFnFuture` drop ladder — with `crate::sync`'s primitives swapped to loom's, so
//! loom schedules every interleaving of the three M6-fixed invariants:
//!
//! | Invariant (M6) | Model | Reverting the fix makes loom report… |
//! |---|---|---|
//! | P3-1 ABA: same reused token, new generation never matches a dead caller's entry | `aba_*` | the dead caller's value delivered |
//! | P2-2 orphan purge at BOTH cancellation paths (frame ladder + `ynz_handle_free`) | `orphan_purge_*` | a `pending_sends` entry outliving its owner |
//! | drop ladder kind-2 arm: purge THIS task's sends, then release its channel ref, concurrent with co-owners | `drop_ladder_*` | orphan / refcount imbalance / payload glued ≠ once |
//! | P3-2 recv register-before-poll: no lost wakeup when a send lands mid-poll | `recv_register_before_poll_*` | a Pending receiver never woken while a value sits buffered |
//!
//! The revert-proofs were performed at introduction (M8 Phase 3 step 5, recorded in the plan's
//! audit) — the harness catches each reverted fix, it does not merely run.
//!
//! # Boundary (named scoping decision, M8 Future Requirements)
//!
//! Tokio's own `mpsc`/semaphore internals stay std (see `sync.rs`): each `try_send` /
//! `poll_recv` / endpoint-future poll is one atomic step to loom. The harness proves the
//! runtime's OWN protocol around those calls is interleaving-safe; it does not (and cannot)
//! model-check tokio.
//!
//! # Observation counters
//!
//! Wake counts and the drop-glue log use std atomics / a std mutex: they are read only after
//! the model threads have been joined (or by the glue fn, which loom's scheduler already
//! serializes), so they never introduce an interleaving loom would need to explore.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc as StdArc;
use std::task::{Context, Wake, Waker};

use loom::thread;

use crate::channel::{
    channel_send_poll_guarded, pending_send_count, purge_pending_sends, strong_count,
    ynz_channel_create, ynz_channel_free, ynz_channel_recv_poll, ynz_channel_share, DriveGuard,
    CHANNEL_CLOSED, CHANNEL_PENDING, CHANNEL_READY,
};
use crate::handle::{ynz_handle_free, ynz_handle_send_poll, ynz_rt_spawn_handle};
use crate::runtime::{BgArgDropEntry, SpawnStateFnFuture};
use ynz_abi::{BG_ARG_KIND_SHARED_CHANNEL, HANDLE_RET_KIND_VALUE_WORD};

// ── model runner ────────────────────────────────────────────────────────────────────────

/// Run `f` under loom, count the interleavings explored, and refuse a vacuous (single
/// interleaving) run. The count and wall time are what the plan's tractability verdict cites.
fn model(name: &str, f: impl Fn(usize) + Sync + Send + 'static) {
    let iterations = StdArc::new(AtomicUsize::new(0));
    let it = iterations.clone();
    let start = std::time::Instant::now();
    loom::model(move || {
        let iteration = it.fetch_add(1, Ordering::Relaxed);
        f(iteration);
    });
    let n = iterations.load(Ordering::Relaxed);
    eprintln!(
        "[loom] {name}: {n} interleavings in {:?} (LOOM_MAX_PREEMPTIONS={})",
        start.elapsed(),
        std::env::var("LOOM_MAX_PREEMPTIONS").unwrap_or_else(|_| "unbounded".into())
    );
    assert!(
        n > 1,
        "{name}: vacuous run — loom explored a single interleaving"
    );
}

// ── wakers / glue / channel helpers ─────────────────────────────────────────────────────

struct CountingWaker(AtomicUsize);
impl Wake for CountingWaker {
    fn wake(self: StdArc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &StdArc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn make_waker() -> (StdArc<CountingWaker>, Waker) {
    let arc = StdArc::new(CountingWaker(AtomicUsize::new(0)));
    let waker = Waker::from(arc.clone());
    (arc, waker)
}

/// Every payload the channel's registered drop glue ever freed, process-wide. The glue is an
/// `extern "C" fn(i64)` with no closure context, so the log is a static — and libtest runs
/// the models on parallel OS threads, so it is shared by every test and every iteration.
/// It is therefore never cleared and never read as a whole: each model tags its payloads
/// with its own iteration index ([`payload`]) and counts exact matches ([`glue_count`]), so
/// no test's state is ever visible to another's assertion.
static GLUE_LOG: std::sync::Mutex<Vec<i64>> = std::sync::Mutex::new(Vec::new());

unsafe extern "C" fn logging_glue(bits: i64) {
    GLUE_LOG.lock().unwrap().push(bits);
}

/// A payload value unique to (this test's `tag`, this `iteration`, the base `value`).
fn payload(tag: i64, iteration: usize, value: i64) -> i64 {
    (tag << 48) | ((iteration as i64) << 16) | value
}

/// How many times the glue freed exactly `bits`.
fn glue_count(bits: i64) -> usize {
    GLUE_LOG
        .lock()
        .unwrap()
        .iter()
        .filter(|b| **b == bits)
        .count()
}

/// A capacity-`cap` channel with the logging glue (so parked-payload frees are observable).
unsafe fn make_glued_chan(cap: i64) -> *mut u8 {
    ynz_channel_create(cap, logging_glue as *mut u8)
}

/// One send poll through the ONE keyed core, with an explicit (token, generation) identity.
unsafe fn send_as(chan: *mut u8, value: i64, waker: &Waker, token: u64, gen: u64) -> i32 {
    let mut cx = Context::from_waker(waker);
    let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
    channel_send_poll_guarded(chan, value, cx_ptr, token, gen)
}

unsafe fn recv(chan: *mut u8, waker: &Waker) -> (i32, i64) {
    let mut out: i64 = 0;
    let mut cx = Context::from_waker(waker);
    let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
    (ynz_channel_recv_poll(chan, &mut out, cx_ptr), out)
}

/// Raw pointers are not `Send`; loom threads need to carry them.
#[derive(Clone, Copy)]
struct P(*mut u8);
unsafe impl Send for P {}

const FILLER: i64 = 42;
const FILLER_TOKEN: u64 = 0xF1;
const FILLER_GEN: u64 = 1;
/// The reused token address the dead and the new caller share.
const T: u64 = 0xDEAD;
const DEAD_GEN: u64 = 5;
const NEW_GEN: u64 = 9;
/// Base payload values; every model tags them per iteration via [`payload`].
const DEAD_VALUE: i64 = 111;
const NEW_VALUE: i64 = 222;
/// Per-test payload tags (the high bits of every payload a model mints).
const TAG_ABA: i64 = 1;
const TAG_ORPHAN_FRAME: i64 = 2;
const TAG_ORPHAN_HANDLE: i64 = 3;
const TAG_LADDER_LIVE: i64 = 4;
const TAG_LADDER_LAST: i64 = 5;

// ── P3-1: ABA ───────────────────────────────────────────────────────────────────────────

/// Caller A (gen 5) is parked at token T on a full channel and dies. Concurrently: A's
/// cancellation purge, a NEW caller B (gen 9) at the SAME reused token sending its own value,
/// and the consumer draining the filler. Under every interleaving B's value must be what B's
/// send delivers, the dead value must never resurface, no entry may survive, and the dead
/// payload must be glue-freed exactly once (by the purge OR by B's insert-time sweep —
/// whichever wins the race — never twice, never zero).
///
/// Both token producers (frame conduit tokens and handle tokens) mint over this one keyed
/// core, so this model covers the key scheme for both.
#[test]
fn loom_aba_same_token_new_generation_delivers_new_value() {
    model("aba_same_token_new_generation", |iteration| unsafe {
        let dead = payload(TAG_ABA, iteration, DEAD_VALUE);
        let new_value = payload(TAG_ABA, iteration, NEW_VALUE);
        let (_w, waker) = make_waker();
        let chan = make_glued_chan(1);
        assert_eq!(
            send_as(chan, FILLER, &waker, FILLER_TOKEN, FILLER_GEN),
            CHANNEL_READY
        );
        // A parks (channel full) and then "dies" — its purge runs on its own thread below.
        assert_eq!(send_as(chan, dead, &waker, T, DEAD_GEN), CHANNEL_PENDING);

        let c = P(chan);
        let t_purge = thread::spawn(move || purge_pending_sends(c.0, DEAD_GEN));
        let t_new = thread::spawn(move || {
            let (_w, waker) = make_waker();
            send_as(c.0, new_value, &waker, T, NEW_GEN)
        });
        let t_drain = thread::spawn(move || {
            let (_w, waker) = make_waker();
            recv(c.0, &waker)
        });
        t_purge.join().unwrap();
        let first = t_new.join().unwrap();
        assert_eq!(t_drain.join().unwrap(), (CHANNEL_READY, FILLER));

        // B re-polls its own send once the slot is free (value 0, as the resumed poll passes).
        if first == CHANNEL_PENDING {
            assert_eq!(
                send_as(chan, 0, &waker, T, NEW_GEN),
                CHANNEL_READY,
                "B's parked send must resolve once the slot is free"
            );
        } else {
            assert_eq!(first, CHANNEL_READY);
        }
        assert_eq!(
            recv(chan, &waker),
            (CHANNEL_READY, new_value),
            "ABA: the NEW caller's value must deliver; the dead caller's value resurfaced"
        );
        assert_eq!(pending_send_count(chan), 0, "orphan: an entry survived");
        assert_eq!(
            glue_count(dead),
            1,
            "the dead caller's parked payload must be glue-freed exactly once"
        );
        assert_eq!(
            glue_count(new_value),
            0,
            "the delivered value was never glued"
        );
        ynz_channel_free(chan);
    });
}

// ── P2-2: orphan purge at both cancellation paths ───────────────────────────────────────

/// Frame-token producer: a task parked on `send` is cancelled (its purge — exactly the drop
/// ladder's call) while the consumer drains. The dead send must never land, its entry must
/// not outlive it, and its payload is freed exactly once.
#[test]
fn loom_orphan_purge_on_frame_cancellation() {
    model("orphan_purge_frame", |iteration| unsafe {
        let dead = payload(TAG_ORPHAN_FRAME, iteration, DEAD_VALUE);
        let (_w, waker) = make_waker();
        let chan = make_glued_chan(1);
        assert_eq!(
            send_as(chan, FILLER, &waker, FILLER_TOKEN, FILLER_GEN),
            CHANNEL_READY
        );
        assert_eq!(send_as(chan, dead, &waker, T, DEAD_GEN), CHANNEL_PENDING);

        let c = P(chan);
        let t_purge = thread::spawn(move || purge_pending_sends(c.0, DEAD_GEN));
        let t_drain = thread::spawn(move || {
            let (_w, waker) = make_waker();
            recv(c.0, &waker)
        });
        t_purge.join().unwrap();
        assert_eq!(t_drain.join().unwrap(), (CHANNEL_READY, FILLER));

        assert_eq!(
            pending_send_count(chan),
            0,
            "orphan: the cancelled caller's entry survived its purge"
        );
        assert_eq!(
            glue_count(dead),
            1,
            "the purged payload is freed exactly once"
        );
        let (code, _) = recv(chan, &waker);
        assert_eq!(
            code, CHANNEL_PENDING,
            "a purged (never re-polled) send must never deliver"
        );
        ynz_channel_free(chan);
    });
}

/// Handle-token producer: `h.send()` parks under `(handle_ptr, send_gen)`; `ynz_handle_free`
/// (the real second cancellation path — purge, then release the conduit ref) races the
/// consumer's drain. Same invariant as the frame path, through the real handle ABI.
#[test]
fn loom_orphan_purge_on_handle_free() {
    unsafe extern "C-unwind" fn never_polled(_frame: *mut u8, _waker: *mut u8) -> i32 {
        1
    }
    model("orphan_purge_handle", |iteration| unsafe {
        let dead = payload(TAG_ORPHAN_HANDLE, iteration, DEAD_VALUE);
        let (_w, waker) = make_waker();
        let chan = make_glued_chan(1);
        assert_eq!(
            send_as(chan, FILLER, &waker, FILLER_TOKEN, FILLER_GEN),
            CHANNEL_READY
        );
        // No runtime inside a loom model: the child is discarded at spawn (its frame freed by
        // its own ladder), but the handle is real — msg_chan shared, send_gen stamped.
        let frame = crate::ynz_alloc_zeroed(64);
        let h = ynz_rt_spawn_handle(
            never_polled,
            frame,
            64,
            -1,
            std::ptr::null(),
            0,
            HANDLE_RET_KIND_VALUE_WORD,
            chan,
        );
        assert_eq!(
            strong_count(chan),
            2,
            "the handle holds its conduit reference"
        );
        let mut cx = Context::from_waker(&waker);
        let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
        assert_eq!(
            ynz_handle_send_poll(h, dead, cx_ptr),
            CHANNEL_PENDING,
            "h.send() on a full conduit parks"
        );
        assert_eq!(pending_send_count(chan), 1);

        let hp = P(h);
        let c = P(chan);
        let t_free = thread::spawn(move || ynz_handle_free(hp.0));
        let t_drain = thread::spawn(move || {
            let (_w, waker) = make_waker();
            recv(c.0, &waker)
        });
        t_free.join().unwrap();
        assert_eq!(t_drain.join().unwrap(), (CHANNEL_READY, FILLER));

        assert_eq!(
            pending_send_count(chan),
            0,
            "orphan: the freed handle's h.send() entry survived ynz_handle_free"
        );
        assert_eq!(
            strong_count(chan),
            1,
            "the handle released exactly its one reference"
        );
        assert_eq!(
            glue_count(dead),
            1,
            "the purged payload is freed exactly once"
        );
        let (code, _) = recv(chan, &waker);
        assert_eq!(code, CHANNEL_PENDING);
        ynz_channel_free(chan);
    });
}

// ── drop ladder: the kind-2 SHARED_CHANNEL arm under concurrent co-ownership ────────────

/// A spawned task's future with the exact ladder codegen emits for a `background f(wire)`:
/// frame slot 32 holds the task's shared channel reference, one kind-2 descriptor names it.
unsafe fn plant_shared_channel_ladder(chan: *mut u8) -> SpawnStateFnFuture {
    unsafe extern "C-unwind" fn never_polled(_frame: *mut u8, _waker: *mut u8) -> i32 {
        1
    }
    const SLOT: u64 = 32;
    let frame_size = SLOT as usize + 8;
    let frame = crate::ynz_alloc_zeroed(frame_size);
    *(frame.add(SLOT as usize) as *mut i64) = ynz_channel_share(chan) as i64;
    let descs = crate::ynz_alloc(std::mem::size_of::<BgArgDropEntry>()) as *mut BgArgDropEntry;
    descs.write(BgArgDropEntry {
        byte_offset: SLOT,
        kind: BG_ARG_KIND_SHARED_CHANNEL,
        size: 0,
    });
    SpawnStateFnFuture::new(never_polled, frame, frame_size as i64, -1, descs, 1)
}

/// Park the task's own send through the extern-C shim (token = its frame pointer, generation
/// = its `task_gen`, published exactly the way `SpawnStateFnFuture::poll` publishes it).
unsafe fn park_task_send(fut: &SpawnStateFnFuture, chan: *mut u8, value: i64) {
    let (_w, waker) = make_waker();
    let _drive = DriveGuard::enter(fut.drive_identity());
    let mut cx = Context::from_waker(&waker);
    let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
    assert_eq!(
        crate::channel::ynz_channel_send_poll(chan, value, cx_ptr, fut.frame_ptr as u64),
        CHANNEL_PENDING,
        "the task's send parks on the full channel"
    );
}

/// A wrapper so the future (which owns raw pointers) can cross into a loom thread.
struct Fut(SpawnStateFnFuture);
unsafe impl Send for Fut {}

/// The task is cancelled (its future dropped — the REAL ladder runs: purge its sends, then
/// release its channel reference) while a co-owner drains and keeps using the channel. Under
/// every interleaving: no orphan, the task's payload freed exactly once, exactly one reference
/// released, and the channel still fully usable by the survivor.
#[test]
fn loom_drop_ladder_kind2_arm_purges_before_release_with_live_co_owner() {
    model("drop_ladder_live_co_owner", |iteration| unsafe {
        let dead = payload(TAG_LADDER_LIVE, iteration, DEAD_VALUE);
        let (_w, waker) = make_waker();
        let chan = make_glued_chan(1);
        assert_eq!(
            send_as(chan, FILLER, &waker, FILLER_TOKEN, FILLER_GEN),
            CHANNEL_READY
        );
        let fut = plant_shared_channel_ladder(chan);
        assert_eq!(strong_count(chan), 2);
        park_task_send(&fut, chan, dead);
        assert_eq!(pending_send_count(chan), 1);

        let f = Fut(fut);
        let c = P(chan);
        let t_cancel = thread::spawn(move || {
            let Fut(fut) = f;
            drop(fut); // the REAL ladder: purge task_gen's sends, then release the channel ref
        });
        let t_co = thread::spawn(move || {
            let (_w, waker) = make_waker();
            let got = recv(c.0, &waker);
            // The survivor keeps sending on the same channel while the ladder may be running.
            let sent = send_as(c.0, 7, &waker, 0x77, 3);
            (got, sent)
        });
        t_cancel.join().unwrap();
        let (got, sent) = t_co.join().unwrap();
        assert_eq!(got, (CHANNEL_READY, FILLER));

        assert_eq!(
            pending_send_count(chan),
            usize::from(sent == CHANNEL_PENDING),
            "only the survivor's own (possibly parked) send may remain — never the dead task's"
        );
        assert_eq!(
            strong_count(chan),
            1,
            "the ladder released exactly one reference"
        );
        assert_eq!(
            glue_count(dead),
            1,
            "the dead task's payload freed exactly once"
        );
        // The survivor's send completes and delivers — the channel is intact.
        if sent == CHANNEL_PENDING {
            assert_eq!(send_as(chan, 0, &waker, 0x77, 3), CHANNEL_READY);
        }
        assert_eq!(recv(chan, &waker), (CHANNEL_READY, 7));
        ynz_channel_free(chan);
    });
}

/// Same cancellation, but the co-owner releases ITS reference concurrently — so whichever
/// side runs last performs the channel's last-reference teardown. The dead payload must be
/// freed exactly once regardless of who tears down (the purge glues it before the ladder's
/// release; teardown's `Drop` walk must then find nothing left to glue).
#[test]
fn loom_drop_ladder_kind2_arm_when_ladder_may_hold_last_reference() {
    model("drop_ladder_last_reference", |iteration| unsafe {
        let dead = payload(TAG_LADDER_LAST, iteration, DEAD_VALUE);
        let (_w, waker) = make_waker();
        let chan = make_glued_chan(1);
        assert_eq!(
            send_as(chan, FILLER, &waker, FILLER_TOKEN, FILLER_GEN),
            CHANNEL_READY
        );
        let fut = plant_shared_channel_ladder(chan);
        park_task_send(&fut, chan, dead);

        let f = Fut(fut);
        let c = P(chan);
        let t_cancel = thread::spawn(move || {
            let Fut(fut) = f;
            drop(fut);
        });
        let t_co = thread::spawn(move || {
            let (_w, waker) = make_waker();
            let got = recv(c.0, &waker);
            ynz_channel_free(c.0); // the co-owner's (main's) reference — maybe the last
            got
        });
        t_cancel.join().unwrap();
        assert_eq!(t_co.join().unwrap(), (CHANNEL_READY, FILLER));
        assert_eq!(
            glue_count(dead),
            1,
            "the dead task's payload must be freed exactly once whichever owner tears down"
        );
    });
}

// ── P3-2: recv register-before-poll ─────────────────────────────────────────────────────

/// Two consumers race one send. `poll_recv` parks a waker in mpsc's SINGLE slot, so the
/// second consumer's poll clobbers the first's — the send's drain of `recv_waiters` is the
/// only thing that reaches a clobbered receiver, and it can only reach a waker that was
/// recorded BEFORE its `poll_recv` returned Pending. The property: if the value is still
/// buffered after all three threads finish (nobody consumed it), every consumer that ended
/// Pending has been woken — a Pending, un-woken receiver next to a buffered value is the hang.
#[test]
fn loom_recv_register_before_poll_no_lost_wakeup() {
    model("recv_register_before_poll", |_iteration| unsafe {
        let chan = ynz_channel_create(2, std::ptr::null_mut());
        let c = P(chan);
        let (arc_a, waker_a) = make_waker();
        let (arc_c, waker_c) = make_waker();
        let t_a = thread::spawn(move || recv(c.0, &waker_a).0);
        let t_c = thread::spawn(move || recv(c.0, &waker_c).0);
        let t_s = thread::spawn(move || {
            let (_w, waker) = make_waker();
            send_as(c.0, 5, &waker, 0x5E, 2)
        });
        let code_a = t_a.join().unwrap();
        let code_c = t_c.join().unwrap();
        assert_eq!(t_s.join().unwrap(), CHANNEL_READY);

        // Snapshot the wake counts BEFORE probing: the probe's own `Ready(Some)` exit drains
        // `recv_waiters` and would wake a receiver the race had already lost (confirmed at
        // introduction — probing first laundered the reverted ordering into a pass).
        let wakes_a = arc_a.0.load(Ordering::SeqCst);
        let wakes_c = arc_c.0.load(Ordering::SeqCst);
        let (_w, probe) = make_waker();
        let (still_buffered, v) = recv(chan, &probe);
        if still_buffered == CHANNEL_READY {
            assert_eq!(v, 5);
            for (name, code, wakes) in [("A", code_a, wakes_a), ("C", code_c, wakes_c)] {
                if code == CHANNEL_PENDING {
                    assert!(
                        wakes >= 1,
                        "lost wakeup: consumer {name} is Pending with a value buffered and \
                         was never woken (P3-2 register-before-poll)"
                    );
                }
            }
        } else {
            assert_eq!(still_buffered, CHANNEL_PENDING);
        }
        assert_ne!(code_a, CHANNEL_CLOSED);
        assert_ne!(code_c, CHANNEL_CLOSED);
        ynz_channel_free(chan);
    });
}
