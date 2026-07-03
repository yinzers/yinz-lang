//! v0.3-M4 Phase 3 — auto-Arc runtime substrate.
//!
//! A minimal atomically-refcounted heap allocation for cross-thread SHARED READ-ONLY
//! state crossing a `background` boundary. Codegen wraps an Arc-eligible value (a shape
//! whose fields are all primitives or strings — the compile-time "Arc-able floor") in one
//! of these allocations at the spawn site; each spawned task's arg-drop ladder releases
//! its reference with [`ynz_arc_free`]; the last release frees the allocation.
//!
//! # Layout
//!
//! One single `ynz_alloc` allocation:
//!
//! ```text
//! [ strong: AtomicU64 — 8 bytes ][ data: `size` bytes ... ]
//! ^ base pointer                 ^ DATA pointer (what codegen holds and passes)
//! ```
//!
//! The C-ABI surface hands out the DATA pointer, never the base pointer, so the spawned
//! callee receives a plain struct pointer indistinguishable from any other shape pointer
//! (same ABI as the heap-copy path). All three entry points recover the header at
//! `data_ptr - 8`.
//!
//! Allocation goes through the COUNTED [`crate::ynz_alloc`] / [`crate::ynz_free`] pair
//! deliberately: the `YNZ_ALLOC_COUNTER_OUTPUT` alloc=free gates then genuinely prove the
//! Arc control block is freed (the plan's R3 exit criterion), instead of exempting it the
//! way runtime-internal `Box` allocations are exempt.
//!
//! # Atomic ordering — acquire-release, never seq-cst
//!
//! Per `docs/internal/implementation/IMP-no-function-coloring.md` "Atomic Ordering
//! Default": Yinz's shared-state primitives use acquire-release, with seq-cst available
//! only as a (deferred, v0.4+) opt-in. The refcount discipline here is the standard Arc
//! protocol built from exactly those orderings:
//!
//! - **Clone: `Relaxed` increment.** The cloning thread already owns a reference, so the
//!   data is already published to it; the increment needs no synchronization of its own.
//! - **Free: `Release` decrement, `Acquire` fence before deallocating.** Every releasing
//!   thread's writes happen-before the decrement (Release); the thread that observes the
//!   count hit zero acquires all of them (the fence) before freeing — no read of the data
//!   can be torn loose from a stale cache line, and the free itself is ordered after every
//!   other thread's last use.
//!
//! This is the same pairing `std::sync::Arc` uses; none of it is sequentially consistent.

use std::sync::atomic::{fence, AtomicU64, Ordering};

/// Byte size of the refcount header preceding the data bytes.
const ARC_HEADER_SIZE: usize = 8;

/// Recover the header pointer from a data pointer.
///
/// # Safety
///
/// `data_ptr` must have been returned by [`ynz_arc_new`] (or [`ynz_arc_clone`]) and not
/// yet released by the final [`ynz_arc_free`].
unsafe fn header<'a>(data_ptr: *mut u8) -> &'a AtomicU64 {
    &*(data_ptr.sub(ARC_HEADER_SIZE) as *const AtomicU64)
}

/// Allocate a refcounted block for `size` data bytes. Returns the DATA pointer
/// (header lives at `ptr - 8`); strong count starts at 1. The caller copies the value's
/// bytes into the returned pointer.
///
/// Data alignment: the data starts 8 bytes into a `malloc`-aligned allocation, so it is
/// 8-byte aligned — sufficient for the Arc-able floor (i64 / f64 / pointer fields only;
/// the compile-time floor excludes 16-byte `number` fields precisely so this holds).
///
/// # Safety
///
/// The returned pointer is valid for `size` bytes. Each reference (the initial one plus
/// every [`ynz_arc_clone`]) must be released exactly once via [`ynz_arc_free`] with the
/// same `size`.
#[no_mangle]
pub unsafe extern "C" fn ynz_arc_new(size: usize) -> *mut u8 {
    let base = crate::ynz_alloc(ARC_HEADER_SIZE + size);
    (base as *mut AtomicU64).write(AtomicU64::new(1));
    base.add(ARC_HEADER_SIZE)
}

/// Take one additional reference to an Arc allocation. Returns the same data pointer.
///
/// # Safety
///
/// `data_ptr` must be a live pointer from [`ynz_arc_new`]/[`ynz_arc_clone`]; the caller
/// must already own at least one reference (this is what makes `Relaxed` sound).
#[no_mangle]
pub unsafe extern "C" fn ynz_arc_clone(data_ptr: *mut u8) -> *mut u8 {
    header(data_ptr).fetch_add(1, Ordering::Relaxed);
    data_ptr
}

/// Release one reference. When the last reference is released, frees the whole
/// allocation (header + data) through the counted `ynz_free`.
///
/// # Safety
///
/// `data_ptr` must be a live pointer from [`ynz_arc_new`]/[`ynz_arc_clone`]; each
/// reference is released at most once; `size` must match the `ynz_arc_new` size.
/// Null is a safe no-op (mirrors `ynz_free`'s null tolerance).
#[no_mangle]
pub unsafe extern "C" fn ynz_arc_free(data_ptr: *mut u8, size: usize) {
    if data_ptr.is_null() {
        return;
    }
    if header(data_ptr).fetch_sub(1, Ordering::Release) == 1 {
        // Last reference: acquire every other releasing thread's writes before freeing.
        fence(Ordering::Acquire);
        crate::ynz_free(data_ptr.sub(ARC_HEADER_SIZE), ARC_HEADER_SIZE + size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read the live strong count (test-only observation point).
    unsafe fn strong(data_ptr: *mut u8) -> u64 {
        header(data_ptr).load(Ordering::Acquire)
    }

    #[test]
    fn new_clone_free_lifecycle_counts_correctly() {
        unsafe {
            let p = ynz_arc_new(16);
            assert_eq!(strong(p), 1);
            // Data bytes are writable and stable across clones.
            (p as *mut u64).write(0xDEAD_BEEF);
            (p.add(8) as *mut u64).write(42);
            let q = ynz_arc_clone(p);
            assert_eq!(q, p, "clone returns the same data pointer");
            assert_eq!(strong(p), 2);
            ynz_arc_free(p, 16);
            assert_eq!(strong(q), 1, "one reference remains after first free");
            assert_eq!((q as *const u64).read(), 0xDEAD_BEEF);
            assert_eq!((q.add(8) as *const u64).read(), 42);
            ynz_arc_free(q, 16);
            // Freed — nothing left to observe; miri/asan would flag a double free here.
        }
    }

    #[test]
    fn free_of_null_is_a_safe_no_op() {
        unsafe { ynz_arc_free(std::ptr::null_mut(), 16) }
    }

    #[test]
    fn concurrent_clone_free_hammer_keeps_the_count_exact() {
        // 8 threads × 1000 clone+free pairs against one shared allocation while the main
        // thread holds the original reference. A lost update (wrong ordering / non-atomic
        // RMW) would leave the count != 1 at join; a premature zero would free the block
        // under the hammer and crash.
        unsafe {
            let p = ynz_arc_new(8);
            (p as *mut u64).write(7);
            let addr = p as usize;
            let mut joins = Vec::new();
            for _ in 0..8 {
                joins.push(std::thread::spawn(move || {
                    let p = addr as *mut u8;
                    for _ in 0..1000 {
                        let q = ynz_arc_clone(p);
                        assert_eq!((q as *const u64).read(), 7, "data stable under hammer");
                        ynz_arc_free(q, 8);
                    }
                }));
            }
            for j in joins {
                j.join().expect("hammer thread panicked");
            }
            assert_eq!(strong(p), 1, "exactly the original reference remains");
            ynz_arc_free(p, 8);
        }
    }
}
