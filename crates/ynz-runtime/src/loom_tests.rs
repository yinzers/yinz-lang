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
//! | drop ladder kind-2 arm: purge THIS task's sends, THEN release its channel ref, concurrent with co-owners | `drop_ladder_*` | orphan / refcount imbalance / payload glued ≠ once; and — the ORDER, asserted by [`assert_purged_before_released`] in both models — the ladder's own entry still parked after its reference is gone (the two calls swapped) |
//! | P3-2 recv register-before-poll: no lost wakeup when a send lands mid-poll | `recv_register_before_poll_*` | a Pending receiver never woken while a value sits buffered |
//! | v0.3-M8 Phase 5 Auto-Arc: the codegen-emitted protocol (new at the first spawn, clone per task, transient released after the last spawn, task releases at retire) frees the block exactly once, never under a live task | `arc_group_*` | a task observing the block freed while it still holds a reference (drop one `ynz_arc_clone` — the revert-proof at introduction), or the block freed ≠ once |
//!
//! The revert-proofs were performed at introduction (M8 Phase 3 step 5 and its fix round 2,
//! recorded in the plan's audit) — the harness catches each reverted fix by its OWN assertion,
//! it does not merely run. The ladder-order row earned that at fix round 2: before it, a
//! swapped kind-2 arm passed the live-co-owner model clean and only killed the last-reference
//! model by use-after-free (a crash, not a finding — round-1 `test-quality` blocker).
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

use crate::arc::{arc_strong_count, ynz_arc_clone, ynz_arc_free, ynz_arc_new, ARC_BLOCK_FREES};
use crate::channel::{
    channel_send_poll_guarded, pending_send_contains, pending_send_count, purge_pending_sends,
    strong_count, ynz_channel_create, ynz_channel_free, ynz_channel_recv_poll, ynz_channel_share,
    DriveGuard, CHANNEL_CLOSED, CHANNEL_PENDING, CHANNEL_READY,
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

/// The key the task's parked send sits under: its frame pointer (the token the extern-C
/// shim mints) and its generation — exactly what the ladder's purge sweeps.
fn ladder_send_key(fut: &SpawnStateFnFuture) -> (u64, u64) {
    (fut.frame_ptr as u64, fut.drive_identity().generation)
}

/// The kind-2 arm's ORDER, observed as a state invariant from a co-owner that still holds
/// its own reference: if the ladder's reference is already gone (`strong_count` fell to
/// this co-owner's 1), the ladder's purge must already have removed the ladder's entry.
/// Loom schedules this probe at every point of the ladder's execution, so the window
/// between a release and a purge in the WRONG order (release first) is an explored state,
/// and it is reported by this assertion — not by whatever the dangling purge does to a
/// freed channel afterwards. Both loom-tracked reads (the `Arc` count, the `pending_sends`
/// lock) are what make the probe a preemption point loom explores.
///
/// Precondition: the calling thread holds one live reference itself (so `strong_count == 1`
/// can only mean the ladder released), and no other owner may release before it does.
unsafe fn assert_purged_before_released(chan: *mut u8, ladder_key: (u64, u64)) {
    if strong_count(chan) == 1 {
        assert!(
            !pending_send_contains(chan, ladder_key.0, ladder_key.1),
            "drop ladder kind-2 arm ORDER: the task's channel reference was released while \
             its own suspended send was still parked — the arm must purge BEFORE \
             ynz_channel_free (a last-reference release here would be a use-after-free in \
             the purge that follows)"
        );
    }
}

/// The task is cancelled (its future dropped — the REAL ladder runs: purge its sends, then
/// release its channel reference) while a co-owner drains and keeps using the channel. Under
/// every interleaving: no orphan, the task's payload freed exactly once, exactly one reference
/// released, the channel still fully usable by the survivor — and the ORDER: the survivor
/// never observes the ladder's reference gone while the ladder's send is still parked.
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
        let ladder_key = ladder_send_key(&fut);

        let f = Fut(fut);
        let c = P(chan);
        let t_cancel = thread::spawn(move || {
            let Fut(fut) = f;
            drop(fut); // the REAL ladder: purge task_gen's sends, then release the channel ref
        });
        let t_co = thread::spawn(move || {
            let (_w, waker) = make_waker();
            let got = recv(c.0, &waker);
            // The survivor (holding main's reference) probes the ladder's order mid-flight.
            assert_purged_before_released(c.0, ladder_key);
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
/// release; teardown's `Drop` walk must then find nothing left to glue), and the ORDER is
/// probed by the co-owner right before it lets go of its own reference — the last moment it
/// can still legally look.
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
        let ladder_key = ladder_send_key(&fut);

        let f = Fut(fut);
        let c = P(chan);
        let t_cancel = thread::spawn(move || {
            let Fut(fut) = f;
            drop(fut);
        });
        let t_co = thread::spawn(move || {
            let (_w, waker) = make_waker();
            let got = recv(c.0, &waker);
            assert_purged_before_released(c.0, ladder_key);
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

// ── v0.3-M8 Phase 4: channel close ───────────────────────────────────────────────────────
//
// Two models on the same substrate. Both drive the REAL `ynz_channel_close` /
// `channel_send_poll_guarded` / drop ladder; the payload tag keeps their glue counts apart.

const TAG_CLOSE_SEND: i64 = 6;
const TAG_REFUSE_CLOSED: i64 = 7;
const CLOSE_SEND_TOKEN: u64 = 0xC105E;
const CLOSE_SEND_GEN: u64 = 3;

/// Contract item 2 — `close()` vs `send()` are linearized at the SENDER-LOCK clone, not at
/// the `try_send`: a send that cloned the endpoint before `close()` took the `Option` is a
/// pre-close send and LANDS (its value is buffered, never glued); a send that reached the
/// lock after the take is refused and its payload is freed by `refuse_closed` exactly once.
/// Under every interleaving exactly one of the two happens — never both, never neither —
/// and the channel reports closed once drained. The model deliberately does NOT assert
/// "refused whenever `close()` returned first": that ordering is neither provided nor
/// needed (the clone is the linearization point).
#[test]
fn loom_close_vs_send_linearizes_at_the_sender_lock_clone() {
    model("close_vs_send_sender_lock", |iteration| unsafe {
        let v = payload(TAG_CLOSE_SEND, iteration, 7);
        let chan = make_glued_chan(2);
        let c = P(chan);
        let t_send = thread::spawn(move || {
            let (_w, waker) = make_waker();
            send_as(c.0, v, &waker, CLOSE_SEND_TOKEN, CLOSE_SEND_GEN)
        });
        let t_close = thread::spawn(move || crate::channel::ynz_channel_close(c.0));
        let code = t_send.join().unwrap();
        t_close.join().unwrap();

        let (_w, waker) = make_waker();
        match code {
            CHANNEL_READY => {
                // Pre-close send: the value landed, the channel owns it, nothing freed it.
                assert_eq!(
                    recv(chan, &waker),
                    (CHANNEL_READY, v),
                    "a send that cloned before the take must LAND"
                );
                assert_eq!(
                    glue_count(v),
                    0,
                    "a landed payload is never glued by the send"
                );
            }
            CHANNEL_CLOSED => {
                // Post-close send: refused, nothing buffered, payload freed exactly once.
                assert_eq!(
                    recv(chan, &waker).0,
                    CHANNEL_CLOSED,
                    "a refused send must leave nothing buffered"
                );
                assert_eq!(
                    glue_count(v),
                    1,
                    "refuse_closed must free the refused payload exactly once (P2-3)"
                );
            }
            other => panic!(
                "first-poll send returned {other} — a send never parks on a non-full channel"
            ),
        }
        // Either way the stream has ended.
        assert_eq!(
            recv(chan, &waker).0,
            CHANNEL_CLOSED,
            "after close + drain: none"
        );
        ynz_channel_free(chan);
    });
}

/// A spawned task's future with the ladder codegen emits for `background f(wire, cell)`: frame
/// slot 32 holds the task's shared channel reference (kind 2), slot 40 a 16-byte heap cell the
/// task owns (kind 0 / HEAP_SHAPE, size 16). Returns the future and its descriptor array.
unsafe fn plant_channel_and_cell_ladder(
    chan: *mut u8,
    cell_bits: i64,
) -> (SpawnStateFnFuture, *mut BgArgDropEntry) {
    unsafe extern "C-unwind" fn never_polled(_frame: *mut u8, _waker: *mut u8) -> i32 {
        1
    }
    const CHAN_SLOT: u64 = 32;
    const CELL_SLOT: u64 = 40;
    let frame_size = CELL_SLOT as usize + 8;
    let frame = crate::ynz_alloc_zeroed(frame_size);
    *(frame.add(CHAN_SLOT as usize) as *mut i64) = ynz_channel_share(chan) as i64;
    *(frame.add(CELL_SLOT as usize) as *mut i64) = cell_bits;
    let descs = crate::ynz_alloc(std::mem::size_of::<BgArgDropEntry>() * 2) as *mut BgArgDropEntry;
    descs.write(BgArgDropEntry {
        byte_offset: CHAN_SLOT,
        kind: BG_ARG_KIND_SHARED_CHANNEL,
        size: 0,
    });
    descs.add(1).write(BgArgDropEntry {
        byte_offset: CELL_SLOT,
        kind: ynz_abi::BG_ARG_KIND_HEAP_SHAPE,
        size: 16,
    });
    (
        SpawnStateFnFuture::new(never_polled, frame, frame_size as i64, -1, descs, 2),
        descs,
    )
}

/// The `refuse_closed` release+glue two-step under a concurrent `close()`: a TASK sends its
/// ladder-owned payload (published as the current drive, exactly as `SpawnStateFnFuture::
/// poll` publishes it) while another thread closes the channel. Under every interleaving the
/// payload has exactly ONE owner at every moment and is freed exactly once — landed: the
/// ladder's slot is RELEASED and teardown glue frees it; refused: the ladder's slot is
/// RELEASED and `refuse_closed` freed it — and the ladder (the task's drop) never touches it
/// on either path. A `refuse_closed` that glues WITHOUT releasing is the ladder+glue double
/// free (kind stays HEAP_SHAPE); one that releases WITHOUT gluing is P2-3's leak
/// (`glue_count == 0`).
#[test]
fn loom_refuse_closed_releases_the_ladder_slot_then_glues_exactly_once() {
    model("refuse_closed_release_then_glue", |iteration| unsafe {
        // The payload is a REAL counted 16-byte cell (the ladder's HEAP_SHAPE arm would
        // `ynz_free` it if it were not released — a double free the process would feel);
        // the channel's glue only LOGS, so "freed by the channel" is observable as a count
        // and the cell is reclaimed by the test at the end.
        let cell = crate::ynz_alloc(16);
        *(cell as *mut i64) = payload(TAG_REFUSE_CLOSED, iteration, 1);
        let bits = cell as i64;
        let chan = make_glued_chan(1);
        let (fut, descs) = plant_channel_and_cell_ladder(chan, bits);
        let c = P(chan);
        let f = Fut(fut);
        let t_send = thread::spawn(move || {
            let f = f;
            let (_w, waker) = make_waker();
            let _drive = DriveGuard::enter(f.0.drive_identity());
            let mut cx = Context::from_waker(&waker);
            let cx_ptr = &mut cx as *mut Context<'_> as *mut u8;
            let code = channel_send_poll_guarded(
                c.0,
                bits,
                cx_ptr,
                f.0.frame_ptr as u64,
                f.0.drive_identity().generation,
            );
            (code, f)
        });
        let t_close = thread::spawn(move || crate::channel::ynz_channel_close(c.0));
        let (code, f) = t_send.join().unwrap();
        t_close.join().unwrap();

        let released = (*descs.add(1)).kind == ynz_abi::BG_ARG_KIND_RELEASED;
        let (_w, waker) = make_waker();
        match code {
            CHANNEL_READY => {
                assert!(
                    released,
                    "a landed ladder payload must be RELEASED from the ladder"
                );
                assert_eq!(glue_count(bits), 0, "landed: nothing freed it yet");
                // Teardown below is the channel's free (it owns the buffered cell).
            }
            CHANNEL_CLOSED => {
                assert!(
                    released,
                    "refuse_closed must release the ladder slot BEFORE gluing — otherwise the \
                     task's drop ladder frees the payload a second time"
                );
                assert_eq!(
                    glue_count(bits),
                    1,
                    "refuse_closed must free the refused payload through the glue exactly once"
                );
                assert_eq!(recv(chan, &waker).0, CHANNEL_CLOSED);
            }
            other => panic!("first-poll send returned {other}"),
        }
        // The task retires: its ladder must skip the released cell (a HEAP_SHAPE arm here
        // would `ynz_free` the cell — with loom's std allocator that is a real double free).
        drop(f);
        // The survivor releases the last reference: teardown glue frees a landed payload.
        ynz_channel_free(chan);
        assert_eq!(
            glue_count(bits),
            1,
            "exactly one free across refuse_closed / teardown glue / the ladder"
        );
        crate::ynz_free(cell, 16);
    });
}

// ── v0.3-M8 Phase 5: Auto-Arc — the codegen-emitted protocol ────────────────────────────

/// The Arc models read the process-wide `ARC_BLOCK_FREES` observation counter, so they take
/// this lane for their whole run: libtest runs the models on parallel OS threads, and a
/// sibling model's frees would otherwise perturb the "no free yet" reading.
static ARC_MODEL_LANE: std::sync::Mutex<()> = std::sync::Mutex::new(());

const ARC_DATA: u64 = 0x5EED_0000_0000_5EED;
const ARC_SIZE: usize = 8;

/// Topology (B) exactly as `ynz-codegen`'s `prepare_bg_arg_for_ctx` + the spawn-site release
/// emit it, driving the REAL `arc.rs` code with its header atomic swapped to loom's: the caller
/// mints the block (`ynz_arc_new`, count 1), and for each of the N tasks takes the task's
/// reference (`ynz_arc_clone`) BEFORE spawning it; right after the last spawn the caller
/// releases its transient (`ynz_arc_free`); each task reads the shared bytes and then releases
/// its own reference (the drop ladder's `BG_ARG_KIND_ARC_SHAPE` arm / the closure-body free).
/// Loom schedules every interleaving of the two tasks' reads and releases against the
/// transient's release. Under all of them: no task ever observes the block freed while it holds
/// a reference (the count is what keeps it alive across the spawn gap), the bytes it reads are
/// intact, and the block is freed exactly ONCE — by whichever release is last.
///
/// Teeth (revert-proven at introduction): dropping the second task's `ynz_arc_clone` — the
/// exact imbalance a missed spawn-site clone would be — makes the transient release + task 1's
/// release hit zero while task 2 still holds the pointer, and task 2's "freed while I hold it"
/// assertion fires (an assertion, not a crash: the check runs BEFORE the read).
#[test]
fn loom_arc_group_clone_per_task_then_transient_release_frees_exactly_once() {
    let _lane = ARC_MODEL_LANE.lock().unwrap();
    model("arc_group_clone_per_task", |_iteration| unsafe {
        let frees_before = ARC_BLOCK_FREES.load(Ordering::Relaxed);
        let block = ynz_arc_new(ARC_SIZE);
        (block as *mut u64).write(ARC_DATA);
        assert_eq!(arc_strong_count(block), 1);
        let mut joins = Vec::new();
        for task in 0..2 {
            // Spawn-site order: the task's reference is taken BEFORE the spawn.
            let task_ref = P(ynz_arc_clone(block));
            joins.push(thread::spawn(move || {
                let freed = ARC_BLOCK_FREES.load(Ordering::Relaxed) - frees_before;
                assert_eq!(
                    freed, 0,
                    "task {task}: the shared block was freed while this task still holds a \
                     reference — a missing clone or an early release"
                );
                assert_eq!(
                    (task_ref.0 as *const u64).read(),
                    ARC_DATA,
                    "task {task}: torn read"
                );
                let freed = ARC_BLOCK_FREES.load(Ordering::Relaxed) - frees_before;
                assert_eq!(
                    freed, 0,
                    "task {task}: block freed before this task released"
                );
                ynz_arc_free(task_ref.0, ARC_SIZE); // the ladder's release at retire
            }));
        }
        // Right after the last spawn: the caller's transient goes away.
        ynz_arc_free(block, ARC_SIZE);
        for j in joins {
            j.join().expect("task thread panicked");
        }
        assert_eq!(
            ARC_BLOCK_FREES.load(Ordering::Relaxed) - frees_before,
            1,
            "the block must be freed exactly once, by the last of the three releases"
        );
    });
}
