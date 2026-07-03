pub mod arc;
pub mod channel;
pub mod handle;
pub mod runtime;
pub use arc::{ynz_arc_clone, ynz_arc_free, ynz_arc_new};
pub use channel::{
    ynz_channel_create, ynz_channel_free, ynz_channel_recv_poll, ynz_channel_send_poll, YnzChannel,
};
pub use runtime::{
    ynz_rt_async_sleep_create, ynz_rt_async_sleep_poll, ynz_rt_check_preempt, ynz_rt_init,
    ynz_rt_join_handle_free, ynz_rt_join_poll, ynz_rt_run_entrypoint, ynz_rt_shutdown,
    ynz_rt_spawn, ynz_rt_spawn_blocking, ynz_rt_spawn_blocking_joinable, ynz_thread_sleep_ms,
    YnzCpuResult,
};

use unicode_normalization::UnicodeNormalization;
/// C-ABI runtime shims for Yinz-compiled binaries.
///
/// # Memory model (M2)
///
/// All `number` values at the ABI boundary are passed as `*const [u8; 16]` (in) or
/// `*mut [u8; 16]` (out).  The 16 bytes are the raw BID bit pattern in native-endian
/// byte order.  Callers (the LLVM codegen) use `alloca [16 x i8]` for stack-allocated
/// `number` locals and pass their addresses directly.
///
/// # String buffers
///
/// The `.toString()` conversion functions return a pointer to a **thread-local static
/// buffer**.  The buffer is valid until the next call to any `ynz_*_to_string` function
/// on the same thread.  This is safe for M2's single-threaded programs; it is NOT
/// safe for multi-threaded use.  A comment at each function marks this limitation.
use ynz_numerics::{abs, add, compare, div, format, mul, neg, parse, sub};

/// Raw decimal128 storage: 16 bytes = 128 bits, BID encoding.
type D128 = [u8; 16];

/// Convert a `D128` byte array (little-endian on LE hosts) to the internal u128.
#[inline]
fn load(p: *const D128) -> u128 {
    // SAFETY: caller guarantees the pointer is valid and aligned to 1 byte.
    u128::from_ne_bytes(unsafe { *p })
}

/// Store a u128 into a `D128` byte array.
#[inline]
fn store(p: *mut D128, v: u128) {
    // SAFETY: caller guarantees the pointer is valid and aligned to 1 byte.
    unsafe { *p = v.to_ne_bytes() }
}

#[no_mangle]
pub extern "C" fn ynz_decimal_add(a: *const D128, b: *const D128, out: *mut D128) {
    store(out, add(load(a), load(b)));
}

#[no_mangle]
pub extern "C" fn ynz_decimal_sub(a: *const D128, b: *const D128, out: *mut D128) {
    store(out, sub(load(a), load(b)));
}

#[no_mangle]
pub extern "C" fn ynz_decimal_mul(a: *const D128, b: *const D128, out: *mut D128) {
    store(out, mul(load(a), load(b)));
}

#[no_mangle]
pub extern "C" fn ynz_decimal_div(a: *const D128, b: *const D128, out: *mut D128) {
    store(out, div(load(a), load(b)));
}

/// Returns -1, 0, or 1 (or 2 for unordered/NaN).
#[no_mangle]
pub extern "C" fn ynz_decimal_compare(a: *const D128, b: *const D128) -> i32 {
    compare(load(a), load(b))
}

#[no_mangle]
pub extern "C" fn ynz_decimal_neg(a: *const D128, out: *mut D128) {
    store(out, neg(load(a)));
}

#[no_mangle]
pub extern "C" fn ynz_decimal_abs(a: *const D128, out: *mut D128) {
    store(out, abs(load(a)));
}

/// Construct a decimal128 from an i64 integer.
#[no_mangle]
pub extern "C" fn ynz_decimal_from_int(x: i64, out: *mut D128) {
    let s = if x < 0 {
        format!("-{}", x.unsigned_abs())
    } else {
        format!("{x}")
    };
    let bits = parse(&s).unwrap_or(ynz_numerics::QUIET_NAN);
    store(out, bits);
}

/// Convert a decimal128 to its string representation.
///
/// # Safety note (M2 limitation)
/// Returns a pointer into a thread-local static buffer.  Valid until the next
/// call to any `ynz_*_to_string` function on this thread.  NOT safe for
/// multi-threaded programs (see module doc).
#[no_mangle]
pub extern "C" fn ynz_decimal_to_string(a: *const D128) -> *const u8 {
    thread_local! {
        static BUF: std::cell::RefCell<[u8; DECIMAL128_STRING_BUF_LEN]> = const { std::cell::RefCell::new([0u8; DECIMAL128_STRING_BUF_LEN]) };
    }
    let s = format(load(a));
    let bytes = s.as_bytes();
    BUF.with(|buf| {
        let mut b = buf.borrow_mut();
        let len = bytes.len().min(b.len() - 1);
        b[..len].copy_from_slice(&bytes[..len]);
        b[len] = 0;
        b.as_ptr()
    })
}

/// Called by compiled code on integer overflow.
///
/// `op_name` is a static C string (null-terminated) describing the operation,
/// e.g. `"int overflow in '+'"`. `file` is the source file name; `line` and
/// `col` are 1-based source positions.
///
/// # Safety
/// `op_name` and `file` must be valid, null-terminated C strings or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_panic_overflow(
    op_name: *const u8,
    file: *const u8,
    line: u32,
    col: u32,
) -> ! {
    let msg = cstr_to_str(op_name);
    let src = cstr_to_str(file);
    // The WHAT/WHAT-INSTEAD/WHY three-part format is embedded here; it cannot
    // go through ariadne because the runtime has no source map at abort time.
    eprintln!(
        "RUNTIME ERROR: {msg} at {src}:{line}:{col}\n\n  \
         The value wrapped past the maximum (or minimum) for this type.\n\n  \
         Use .wrappingAdd() if wrap-around is intentional.\n\n  \
         Why: Yinz panics on integer overflow by default to prevent silent data corruption."
    );
    std::process::abort();
}

/// Called by compiled code on division by zero.
///
/// `op_name` is a static C string (null-terminated). `file`, `line`, `col`
/// are the source location of the division expression.
///
/// # Safety
/// `op_name` and `file` must be valid, null-terminated C strings or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_panic_div_by_zero(
    op_name: *const u8,
    file: *const u8,
    line: u32,
    col: u32,
) -> ! {
    let msg = cstr_to_str(op_name);
    let src = cstr_to_str(file);
    eprintln!(
        "RUNTIME ERROR: {msg} at {src}:{line}:{col}\n\n  \
         Check that the denominator is not zero before dividing:\n    \
         if (denominator != 0) {{ let result = numerator / denominator }}\n\n  \
         Why: Dividing by zero produces an undefined result. Yinz panics rather\n  \
         than silently producing garbage."
    );
    std::process::abort();
}

/// Convert an i64 to its decimal string representation.
///
/// Returns a pointer into a thread-local static buffer (same M2 limitation as
/// `ynz_decimal_to_string` above).
#[no_mangle]
pub extern "C" fn ynz_int_to_string(x: i64) -> *const u8 {
    thread_local! {
        static BUF: std::cell::RefCell<[u8; INT64_STRING_BUF_LEN]> = const { std::cell::RefCell::new([0u8; INT64_STRING_BUF_LEN]) };
    }
    let s = format!("{x}");
    let bytes = s.as_bytes();
    BUF.with(|buf| {
        let mut b = buf.borrow_mut();
        let len = bytes.len().min(b.len() - 1);
        b[..len].copy_from_slice(&bytes[..len]);
        b[len] = 0;
        b.as_ptr()
    })
}

/// Convert an f64 to its decimal string representation.
///
/// Returns a pointer into a thread-local static buffer (same M2 limitation).
#[no_mangle]
pub extern "C" fn ynz_float_to_string(x: f64) -> *const u8 {
    thread_local! {
        static BUF: std::cell::RefCell<[u8; 32]> = const { std::cell::RefCell::new([0u8; 32]) };
    }
    let s = format!("{x}");
    let bytes = s.as_bytes();
    BUF.with(|buf| {
        let mut b = buf.borrow_mut();
        let len = bytes.len().min(b.len() - 1);
        b[..len].copy_from_slice(&bytes[..len]);
        b[len] = 0;
        b.as_ptr()
    })
}

/// Compare two null-terminated UTF-8 strings for NFC canonical equivalence.
///
/// Returns 1 if NFC-normalized forms are equal, 0 otherwise. Used by codegen for
/// multi-case `if` on string scrutinees and `==` on strings.
///
/// Fast path: pointer equality or ASCII-only inputs (ASCII is always NFC). Slow path:
/// normalize both to NFC then compare — closes the M3 catch-up obligation for Unicode
/// canonical equivalence (e.g., precomposed `é` == decomposed `e` + combining accent).
///
/// # Safety
///
/// Both `a` and `b` must be valid pointers to null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_eq(a: *const u8, b: *const u8) -> i32 {
    if a == b {
        return 1;
    }
    let a_str = unsafe { std::ffi::CStr::from_ptr(a as *const i8) }
        .to_str()
        .unwrap_or("");
    let b_str = unsafe { std::ffi::CStr::from_ptr(b as *const i8) }
        .to_str()
        .unwrap_or("");
    // Fast path: ASCII-only strings are always NFC; byte comparison is sufficient.
    if a_str.is_ascii() && b_str.is_ascii() {
        return (a_str == b_str) as i32;
    }
    // Slow path: normalize both strings to NFC then compare.
    let a_nfc: String = a_str.nfc().collect();
    let b_nfc: String = b_str.nfc().collect();
    (a_nfc == b_nfc) as i32
}

/// Format a `sensitive string` value for output.
///
/// Checks `YNZ_REVEAL_SENSITIVE` once per process (via `OnceLock`). If the env var
/// is set to `"1"`, returns `raw_ptr` unchanged so the value prints normally.
/// Otherwise returns a pointer to the static `"[REDACTED]"` string.
///
/// # Safety
///
/// `raw_ptr` must be a valid pointer to a null-terminated C string, or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_sensitive_to_string(raw_ptr: *const u8) -> *const u8 {
    use std::sync::OnceLock;
    static REVEAL: OnceLock<bool> = OnceLock::new();
    let reveal =
        *REVEAL.get_or_init(|| std::env::var("YNZ_REVEAL_SENSITIVE").as_deref() == Ok("1"));
    if reveal {
        raw_ptr
    } else {
        c"[REDACTED]".as_ptr().cast()
    }
}

unsafe fn cstr_to_str<'a>(p: *const u8) -> &'a str {
    if p.is_null() {
        return "<unknown operation>";
    }
    let mut len = 0;
    while unsafe { *p.add(len) } != 0 {
        len += 1;
    }
    std::str::from_utf8(unsafe { std::slice::from_raw_parts(p, len) })
        .unwrap_or("<invalid utf-8 in op name>")
}

// ── Heap allocator shims (M4) ─────────────────────────────────────────────────
//
// Thin wrappers over libc malloc/free with a consistent ABI for the LLVM backend.
// `_size` in ynz_free is reserved for kernel-mode plug-in allocators (v0.3+)
// that need the size at deallocation time; libc free ignores it.

extern "C" {
    fn malloc(size: usize) -> *mut core::ffi::c_void;
    fn realloc(ptr: *mut core::ffi::c_void, new_size: usize) -> *mut core::ffi::c_void;
    fn free(ptr: *mut core::ffi::c_void);
}

// ── Test-instrumentation alloc counter ───────────────────────────────────────
//
// Counts ynz_alloc and ynz_free calls when the `YNZ_ALLOC_COUNTER` env var is set
// to any non-empty value at process start (`ynz_rt_init`).
//
// The env var is read ONCE at `ynz_rt_init` time and stored in `ALLOC_COUNTER_ENABLED`
// (a `static AtomicBool`). Per-alloc cost is a single relaxed atomic load — zero
// syscall/heap/lock overhead on the hot allocation path. Reading the env var on
// every allocation (lock + heap + UTF-8 validation) violated Golden Rule 8 (zero-cost).
//
// Used by Phase 7 composed-single-alloc proof: a 3-level synchronous SM call tree
// must allocate exactly ONE frame (the root), not one per call.
//
// CARVE-OUT: these are runtime internals with no user-facing Yinz surface. They
// are not registered in registry/features.toml per the feature-registry.md carve-out
// policy (compiler-internal bookkeeping, no IDE or error-message surface).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Set once at `ynz_rt_init` time from the `YNZ_ALLOC_COUNTER` env var.
/// Per-alloc gate is a relaxed atomic load (no syscall, no lock, no heap allocation).
// test-only: tests set `YNZ_ALLOC_COUNTER` before the process starts; ynz_rt_init
// reads it once here. Production runs that don't set the env var pay zero overhead.
static ALLOC_COUNTER_ENABLED: AtomicBool = AtomicBool::new(false);

/// Running count of `ynz_alloc` calls since last reset.
static YNZ_ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
/// Running count of `ynz_free` calls since last reset.
static YNZ_FREE_COUNT: AtomicU64 = AtomicU64::new(0);

/// Return true when alloc-counter instrumentation is enabled.
///
/// Reads a `static AtomicBool` set once at `ynz_rt_init`; cost is a single
/// relaxed atomic load. Call-site overhead is negligible.
#[inline]
fn alloc_counter_enabled() -> bool {
    ALLOC_COUNTER_ENABLED.load(Ordering::Relaxed)
}

/// Called by `ynz_rt_init` to latch the env-var check once per process.
pub(crate) fn init_alloc_counter_flag() {
    let enabled = std::env::var("YNZ_ALLOC_COUNTER")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    ALLOC_COUNTER_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Read the current alloc call count. Zero when counter not enabled.
#[no_mangle]
pub extern "C" fn ynz_alloc_count() -> u64 {
    YNZ_ALLOC_COUNT.load(Ordering::Relaxed)
}

/// Read the current free call count. Zero when counter not enabled.
#[no_mangle]
pub extern "C" fn ynz_free_count() -> u64 {
    YNZ_FREE_COUNT.load(Ordering::Relaxed)
}

/// Reset both counters to zero.
#[no_mangle]
pub extern "C" fn ynz_alloc_count_reset() {
    YNZ_ALLOC_COUNT.store(0, Ordering::Relaxed);
    YNZ_FREE_COUNT.store(0, Ordering::Relaxed);
}

/// Allocate `size` bytes. Aborts on OOM — Yinz programs cannot recover from OOM.
///
/// # Safety
///
/// The returned pointer is valid for `size` bytes and properly aligned.
/// The caller must free it with `ynz_free` using the same `size`.
#[no_mangle]
pub unsafe extern "C" fn ynz_alloc(size: usize) -> *mut u8 {
    if alloc_counter_enabled() {
        YNZ_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
    }
    let ptr = malloc(size) as *mut u8;
    if ptr.is_null() {
        std::process::abort();
    }
    ptr
}

/// Allocate `size` zeroed bytes. Used for state-machine frames where ALL fields must
/// be deterministically zero-initialized (in particular the recursion_slot pointer, which
/// `SpawnStateFnFuture::Drop` reads to walk the recursion chain — a non-null garbage value
/// from uninitialized memory would cause a use-after-free or double-free on cancellation).
///
/// # Safety
///
/// The returned pointer is valid for `size` zero-initialized bytes and properly aligned.
/// The caller must free it with `ynz_free` using the same `size`.
#[no_mangle]
pub unsafe extern "C" fn ynz_alloc_zeroed(size: usize) -> *mut u8 {
    if alloc_counter_enabled() {
        YNZ_ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
    }
    // malloc + memset is portable; calloc is equivalent but may not be exposed in all targets.
    let ptr = malloc(size) as *mut u8;
    if ptr.is_null() {
        std::process::abort();
    }
    // Zero all bytes so recursion_slot + any other pointer slots start null.
    std::ptr::write_bytes(ptr, 0, size);
    ptr
}

/// Free a heap allocation previously returned by `ynz_alloc`.
///
/// `_size` is unused in M4 (libc free doesn't need it) but is part of the
/// allocator ABI for kernel-mode plug-in support in v0.3+.
///
/// # Safety
///
/// `ptr` must have been returned by `ynz_alloc` and not yet freed.
/// Passing a null pointer is safe (no-op via libc free semantics).
#[no_mangle]
pub unsafe extern "C" fn ynz_free(ptr: *mut u8, _size: usize) {
    if alloc_counter_enabled() {
        YNZ_FREE_COUNT.fetch_add(1, Ordering::Relaxed);
    }
    free(ptr as *mut core::ffi::c_void);
}

// ── Capacity / buffer size constants ─────────────────────────────────────────
//
// Named here so the rationale is co-located with the values and tuning them
// updates all uses automatically.

/// Thread-local buffer length for `ynz_decimal_to_string`.
///
/// decimal128 max: 34 significant digits + sign + decimal point + exponent ("E+N") +
/// null terminator = ≤ 40 bytes. 48 rounds up to a cache-friendly size with margin.
const DECIMAL128_STRING_BUF_LEN: usize = 48;

/// Thread-local buffer length for `ynz_int_to_string`.
///
/// i64 max: 20 decimal digits + sign + null terminator = 22 bytes.
const INT64_STRING_BUF_LEN: usize = 22;

/// Initial insertion-order buffer capacity for a new map.
///
/// 64 entries: accommodates typical Yinz maps without a growth step; tuned
/// alongside `INITIAL_MAP_CAPACITY` so the order buffer only grows when the
/// hash table itself has already grown at least twice.
const INITIAL_ORDER_CAP: i64 = 64;

/// Initial hash-table capacity for a new map.
///
/// 16 slots: gives room for up to 12 entries before the 75% load-factor
/// threshold triggers the first ×2 growth. Power-of-two required by the
/// bitmask probe arithmetic `(hash >> 7) & (cap - 1)`.
const INITIAL_MAP_CAPACITY: i64 = 16;

/// Initial data-buffer capacity for a new array (in elements).
///
/// 8 elements × 8 bytes = 64 bytes = one typical cache line; avoids an
/// immediate realloc on the first push while keeping initial allocation small.
const INITIAL_ARRAY_CAPACITY: i64 = 8;

// ── SipHash-2-4 (M5 P4b) ─────────────────────────────────────────────────────
//
// Algorithm: SipHash-2-4 (Aumasson & Bernstein, 2012).
//   Reference: https://131002.net/siphash/siphash.pdf
//   2 compression rounds per 8-byte block, 4 finalization rounds.
//
// Key: 128-bit, seeded from /dev/urandom once per process via SIPHASH_KEY (OnceLock).
//   Per-process randomization provides hash-DoS protection: an attacker who can
//   observe map iteration order cannot predict insertion-hash collisions in a
//   subsequent request. Maps with user-controlled keys (Yinz user programs) use
//   this key. Compiler-internal maps (ExportTable etc.) may use a zero key via
//   get_or_init fallback — acceptable because their keys are not attacker-controlled.
//
// Magic constants (v0, v1, v2, v3 initialization XOR masks) are the standard
// SipHash IV values from the paper §2.1:
//   0x736f6d6570736575 = "somepseu"
//   0x646f72616e646f6d = "dorandom"
//   0x6c7967656e657261 = "lygenera"
//   0x7465646279746573 = "tedbytes"

use std::sync::OnceLock;

static SIPHASH_KEY: OnceLock<[u8; 16]> = OnceLock::new();

/// Initialize the per-process SipHash key from OS entropy.
/// Must be called before any map operation. Idempotent.
#[no_mangle]
pub extern "C" fn ynz_siphash_init() {
    SIPHASH_KEY.get_or_init(|| {
        let mut key = [0u8; 16];
        // Linux, macOS, and the BSDs all expose /dev/urandom. Using `unix` (not just
        // `linux`) means macOS gets a real entropy-seeded key instead of all-zeros —
        // and keeps `mut key` used on every CI target, so clippy stays clean cross-platform.
        #[cfg(unix)]
        {
            use std::fs::File;
            use std::io::Read;
            if let Ok(mut f) = File::open("/dev/urandom") {
                let _ = f.read_exact(&mut key);
            }
        }
        key
    });
}

fn siphash_key() -> (u64, u64) {
    let k = SIPHASH_KEY.get_or_init(|| [0u8; 16]);
    let k0 = u64::from_le_bytes([k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7]]);
    let k1 = u64::from_le_bytes([k[8], k[9], k[10], k[11], k[12], k[13], k[14], k[15]]);
    (k0, k1)
}

macro_rules! sipround {
    ($v0:expr, $v1:expr, $v2:expr, $v3:expr) => {
        $v0 = $v0.wrapping_add($v1);
        $v1 = $v1.rotate_left(13);
        $v1 ^= $v0;
        $v0 = $v0.rotate_left(32);
        $v2 = $v2.wrapping_add($v3);
        $v3 = $v3.rotate_left(16);
        $v3 ^= $v2;
        $v0 = $v0.wrapping_add($v3);
        $v3 = $v3.rotate_left(21);
        $v3 ^= $v0;
        $v2 = $v2.wrapping_add($v1);
        $v1 = $v1.rotate_left(17);
        $v1 ^= $v2;
        $v2 = $v2.rotate_left(32);
    };
}

fn siphash24(data: &[u8]) -> u64 {
    let (k0, k1) = siphash_key();
    let mut v0 = k0 ^ 0x736f6d6570736575u64;
    let mut v1 = k1 ^ 0x646f72616e646f6du64;
    let mut v2 = k0 ^ 0x6c7967656e657261u64;
    let mut v3 = k1 ^ 0x7465646279746573u64;

    let len = data.len();
    let blocks = len / 8;
    for i in 0..blocks {
        let b = &data[i * 8..];
        let m = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
        v3 ^= m;
        sipround!(v0, v1, v2, v3);
        sipround!(v0, v1, v2, v3);
        v0 ^= m;
    }

    let rem = len % 8;
    let mut last = ((len as u64) << 56) & 0xff00000000000000u64;
    let base = blocks * 8;
    for i in (0..rem).rev() {
        last |= (data[base + i] as u64) << (i * 8);
    }
    v3 ^= last;
    sipround!(v0, v1, v2, v3);
    sipround!(v0, v1, v2, v3);
    v0 ^= last;

    v2 ^= 0xff;
    sipround!(v0, v1, v2, v3);
    sipround!(v0, v1, v2, v3);
    sipround!(v0, v1, v2, v3);
    sipround!(v0, v1, v2, v3);
    v0 ^ v1 ^ v2 ^ v3
}

/// Hash an i64 value (for int/bool/float keys).
#[no_mangle]
pub extern "C" fn ynz_siphash_i64(value: i64) -> u64 {
    siphash24(&value.to_le_bytes())
}

/// Hash a null-terminated string key.
///
/// # Safety
/// `ptr` must be a non-null pointer to a valid null-terminated byte string.
#[no_mangle]
pub unsafe extern "C" fn ynz_siphash_str(ptr: *const u8) -> u64 {
    let mut len = 0;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    siphash24(std::slice::from_raw_parts(ptr, len))
}

// ── Swiss Tables map runtime (M5 P4b) ────────────────────────────────────────
//
// Open-addressing hash map. Each slot has:
//   1 byte control: 0x80 = empty, 0xFE = deleted, low 7 bits of hash = present.
//   8 bytes key (stored as i64 — int/bool/float by value, string/ptr as i64 cast).
//   8 bytes value (stored as i64).
//
// Insertion order is tracked in a separate buffer for deterministic for-loop iteration.

const CTRL_EMPTY: u8 = 0x80;
const CTRL_DELETED: u8 = 0xFE;

#[repr(C)]
pub struct YnzMap {
    ctrl: *mut u8,
    keys: *mut i64,
    vals: *mut i64,
    insert_order: *mut i64,
    count: i64,
    capacity: i64,
    order_cap: i64,
}

unsafe fn map_alloc(capacity: i64) -> *mut YnzMap {
    let hdr = malloc(std::mem::size_of::<YnzMap>()) as *mut YnzMap;
    if hdr.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new map. \
                  The program tried to create a map but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let ctrl = malloc(capacity as usize) as *mut u8;
    if ctrl.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new map. \
                  The program tried to create a map but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let keys = malloc((capacity as usize) * 8) as *mut i64;
    if keys.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new map. \
                  The program tried to create a map but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let vals = malloc((capacity as usize) * 8) as *mut i64;
    if vals.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new map. \
                  The program tried to create a map but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let order_cap: i64 = INITIAL_ORDER_CAP;
    let order = malloc((order_cap as usize) * 8) as *mut i64;
    if order.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new map. \
                  The program tried to create a map but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    std::ptr::write_bytes(ctrl, CTRL_EMPTY, capacity as usize);
    *hdr = YnzMap {
        ctrl,
        keys,
        vals,
        insert_order: order,
        count: 0,
        capacity,
        order_cap,
    };
    hdr
}

/// Allocate a new empty map with initial capacity [`INITIAL_MAP_CAPACITY`].
///
/// # Safety
/// The returned pointer is valid and must be freed with [`ynz_map_drop`].
#[no_mangle]
pub unsafe extern "C" fn ynz_map_new() -> *mut YnzMap {
    map_alloc(INITIAL_MAP_CAPACITY)
}

unsafe fn find_slot(map: *const YnzMap, hash: u64, key: i64) -> Option<usize> {
    let cap = (*map).capacity as usize;
    let h2 = (hash & 0x7f) as u8;
    let start = (hash >> 7) as usize & (cap - 1);
    let mut idx = start;
    loop {
        let ctrl = *(*map).ctrl.add(idx);
        if ctrl == CTRL_EMPTY {
            return None;
        }
        if ctrl == h2 && *(*map).keys.add(idx) == key {
            return Some(idx);
        }
        idx = (idx + 1) & (cap - 1);
        if idx == start {
            return None;
        }
    }
}

unsafe fn find_insert_slot(map: *const YnzMap, hash: u64) -> usize {
    let cap = (*map).capacity as usize;
    let mut idx = (hash >> 7) as usize & (cap - 1);
    let mut probes = 0usize;
    loop {
        let ctrl = *(*map).ctrl.add(idx);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED {
            return idx;
        }
        probes += 1;
        if probes >= cap {
            eprintln!(
                "RUNTIME ERROR: Map full and unable to grow. \
                      This should be impossible — please file a compiler bug with \
                      the program that triggered it."
            );
            std::process::abort();
        }
        idx = (idx + 1) & (cap - 1);
    }
}

/// Grow an int-keyed map to ×2 capacity.
///
/// # Side effects
///
/// - Allocates three new arrays (ctrl, keys, vals) of `2 × capacity` slots each.
/// - Rehashes all live entries into the new arrays.
/// - Frees the old ctrl, keys, and vals arrays.
/// - Updates `map.ctrl`, `map.keys`, `map.vals`, and `map.capacity` in place.
///   Any previously obtained raw pointers into these arrays are INVALIDATED.
///   The `insert_order` / `order_cap` / `count` fields are NOT modified.
/// - Aborts the process on OOM (Yinz programs cannot recover from out-of-memory).
///
/// # Growth trigger
///
/// Called by `ynz_map_set` when `count * 4 >= capacity * 3` (75% load factor).
unsafe fn map_grow_int(map: *mut YnzMap) {
    let old_cap = (*map).capacity;
    let new_cap = old_cap * 2;
    let new_ctrl = malloc(new_cap as usize) as *mut u8;
    if new_ctrl.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while growing a map. \
                  The program tried to insert into a map but the system couldn't \
                  allocate more memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let new_keys = malloc((new_cap as usize) * 8) as *mut i64;
    if new_keys.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while growing a map. \
                  The program tried to insert into a map but the system couldn't \
                  allocate more memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let new_vals = malloc((new_cap as usize) * 8) as *mut i64;
    if new_vals.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while growing a map. \
                  The program tried to insert into a map but the system couldn't \
                  allocate more memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    std::ptr::write_bytes(new_ctrl, CTRL_EMPTY, new_cap as usize);

    for i in 0..old_cap as usize {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED {
            continue;
        }
        let k = *(*map).keys.add(i);
        let v = *(*map).vals.add(i);
        let hash = ynz_siphash_i64(k);
        let h2 = (hash & 0x7f) as u8;
        let mut idx = (hash >> 7) as usize & (new_cap as usize - 1);
        while *new_ctrl.add(idx) != CTRL_EMPTY {
            idx = (idx + 1) & (new_cap as usize - 1);
        }
        *new_ctrl.add(idx) = h2;
        *new_keys.add(idx) = k;
        *new_vals.add(idx) = v;
    }

    free((*map).ctrl as *mut core::ffi::c_void);
    free((*map).keys as *mut core::ffi::c_void);
    free((*map).vals as *mut core::ffi::c_void);
    (*map).ctrl = new_ctrl;
    (*map).keys = new_keys;
    (*map).vals = new_vals;
    (*map).capacity = new_cap;
}

/// Grow a string-keyed map to ×2 capacity.
///
/// Same side-effect contract as `map_grow_int` except rehashing uses
/// `ynz_siphash_str` (pointer-to-string keys) instead of `ynz_siphash_i64`.
/// String key POINTERS are preserved as-is (the map stores pointers, not copies).
unsafe fn map_grow_str(map: *mut YnzMap) {
    let old_cap = (*map).capacity;
    let new_cap = old_cap * 2;
    let new_ctrl = malloc(new_cap as usize) as *mut u8;
    if new_ctrl.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while growing a map. \
                  The program tried to insert into a map but the system couldn't \
                  allocate more memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let new_keys = malloc((new_cap as usize) * 8) as *mut i64;
    if new_keys.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while growing a map. \
                  The program tried to insert into a map but the system couldn't \
                  allocate more memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    let new_vals = malloc((new_cap as usize) * 8) as *mut i64;
    if new_vals.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while growing a map. \
                  The program tried to insert into a map but the system couldn't \
                  allocate more memory. Yinz aborts rather than continuing with \
                  an inconsistent map."
        );
        std::process::abort();
    }
    std::ptr::write_bytes(new_ctrl, CTRL_EMPTY, new_cap as usize);

    for i in 0..old_cap as usize {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED {
            continue;
        }
        let k = *(*map).keys.add(i);
        let v = *(*map).vals.add(i);
        let hash = ynz_siphash_str(k as *const u8);
        let h2 = (hash & 0x7f) as u8;
        let mut idx = (hash >> 7) as usize & (new_cap as usize - 1);
        while *new_ctrl.add(idx) != CTRL_EMPTY {
            idx = (idx + 1) & (new_cap as usize - 1);
        }
        *new_ctrl.add(idx) = h2;
        *new_keys.add(idx) = k;
        *new_vals.add(idx) = v;
    }

    free((*map).ctrl as *mut core::ffi::c_void);
    free((*map).keys as *mut core::ffi::c_void);
    free((*map).vals as *mut core::ffi::c_void);
    (*map).ctrl = new_ctrl;
    (*map).keys = new_keys;
    (*map).vals = new_vals;
    (*map).capacity = new_cap;
}

unsafe fn order_push(map: *mut YnzMap, key: i64) {
    if (*map).count >= (*map).order_cap {
        let new_cap = (*map).order_cap * 2;
        let new_order = realloc(
            (*map).insert_order as *mut core::ffi::c_void,
            (new_cap as usize) * 8,
        ) as *mut i64;
        if new_order.is_null() {
            eprintln!(
                "RUNTIME ERROR: Out of memory while growing a map's insertion-order tracking. \
                      The program tried to insert into a map but the system couldn't \
                      allocate more memory. Yinz aborts rather than continuing with \
                      an inconsistent map."
            );
            std::process::abort();
        }
        (*map).insert_order = new_order;
        (*map).order_cap = new_cap;
    }
    *(*map).insert_order.add((*map).count as usize) = key;
}

unsafe fn cstr_eq_raw(a: *const u8, b: *const u8) -> bool {
    let mut i = 0;
    loop {
        let ca = *a.add(i);
        let cb = *b.add(i);
        if ca != cb {
            return false;
        }
        if ca == 0 {
            return true;
        }
        i += 1;
    }
}

/// Get a value by i64 key. Writes `[has_value, value]` into `out`.
///
/// # Safety
/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// - `out` must point to a writable `[i64; 2]` array.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_get(map: *const YnzMap, key: i64, out: *mut [i64; 2]) {
    let hash = ynz_siphash_i64(key);
    match find_slot(map, hash, key) {
        Some(idx) => *out = [1, *(*map).vals.add(idx)],
        None => *out = [0, 0],
    }
}

/// Get a value by string key (key is a pointer to null-terminated bytes, passed as i64 cast).
/// Writes `[has_value, value]` into `out`.
///
/// # Safety
/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// - `key` must be a non-null pointer to a valid null-terminated byte string.
/// - `out` must point to a writable `[i64; 2]` array.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_get_str(map: *const YnzMap, key: *const u8, out: *mut [i64; 2]) {
    let cap = (*map).capacity as usize;
    for i in 0..cap {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED {
            continue;
        }
        let stored_ptr = *(*map).keys.add(i) as *const u8;
        if cstr_eq_raw(stored_ptr, key) {
            *out = [1, *(*map).vals.add(i)];
            return;
        }
    }
    *out = [0, 0];
}

/// Set a key-value pair with an i64 key.
///
/// # Safety
/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// - May grow the map (×2 capacity at 75% load factor), invalidating any previously
///   obtained slot pointers into the map's internal arrays.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_set(map: *mut YnzMap, key: i64, value: i64) {
    if (*map).count * 4 >= (*map).capacity * 3 {
        map_grow_int(map);
    }
    let hash = ynz_siphash_i64(key);
    if let Some(idx) = find_slot(map, hash, key) {
        *(*map).vals.add(idx) = value;
        return;
    }
    let h2 = (hash & 0x7f) as u8;
    let idx = find_insert_slot(map, hash);
    *(*map).ctrl.add(idx) = h2;
    *(*map).keys.add(idx) = key;
    *(*map).vals.add(idx) = value;
    order_push(map, key);
    (*map).count += 1;
}

/// Set a key-value pair with a string key (pointer to null-terminated bytes).
///
/// # Safety
/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// - `key` must be a non-null pointer to a valid null-terminated byte string that
///   remains valid for the lifetime of the map (the map stores the pointer, not a copy).
/// - May grow the map (×2 capacity at 75% load factor), invalidating any previously
///   obtained slot pointers into the map's internal arrays.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_set_str(map: *mut YnzMap, key: *const u8, value: i64) {
    if (*map).count * 4 >= (*map).capacity * 3 {
        map_grow_str(map);
    }
    let cap = (*map).capacity as usize;
    for i in 0..cap {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED {
            continue;
        }
        let stored = *(*map).keys.add(i) as *const u8;
        if cstr_eq_raw(stored, key) {
            *(*map).vals.add(i) = value;
            return;
        }
    }
    let hash = ynz_siphash_str(key);
    let h2 = (hash & 0x7f) as u8;
    let mut idx = (hash >> 7) as usize & (cap - 1);
    let mut probes = 0usize;
    while *(*map).ctrl.add(idx) != CTRL_EMPTY && *(*map).ctrl.add(idx) != CTRL_DELETED {
        probes += 1;
        if probes >= cap {
            eprintln!(
                "RUNTIME ERROR: Map full and unable to grow. \
                      This should be impossible — please file a compiler bug with \
                      the program that triggered it."
            );
            std::process::abort();
        }
        idx = (idx + 1) & (cap - 1);
    }
    *(*map).ctrl.add(idx) = h2;
    *(*map).keys.add(idx) = key as i64;
    *(*map).vals.add(idx) = value;
    order_push(map, key as i64);
    (*map).count += 1;
}

/// Return the number of key-value pairs.
///
/// # Safety
/// `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_count(map: *const YnzMap) -> i64 {
    (*map).count
}

/// Check if an i64 key exists. Returns 1 if found, 0 otherwise.
///
/// # Safety
/// `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_has(map: *const YnzMap, key: i64) -> i64 {
    let hash = ynz_siphash_i64(key);
    match find_slot(map, hash, key) {
        Some(_) => 1,
        None => 0,
    }
}

/// Get the entry at insertion-order position `pos`. Writes `[has, key, value]` into `out`.
///
/// # Safety
/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// - `out` must point to a writable `[i64; 3]` array.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_iter_get(map: *const YnzMap, pos: i64, out: *mut [i64; 3]) {
    if pos < 0 || pos >= (*map).count {
        *out = [0, 0, 0];
        return;
    }
    let key = *(*map).insert_order.add(pos as usize);
    let mut pair = [0i64; 2];
    ynz_map_get(map, key, &mut pair);
    *out = [1, key, pair[1]];
}

/// Get the entry at insertion-order position `pos` for string-keyed maps.
/// Writes `[has, key_ptr_as_i64, value]` into `out`.
///
/// # Safety
/// - `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// - `out` must point to a writable `[i64; 3]` array.
/// - The `key_ptr_as_i64` written into `out[1]` is a pointer cast to i64; callers
///   must cast it back to `*const u8` before dereferencing.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_iter_get_str(map: *const YnzMap, pos: i64, out: *mut [i64; 3]) {
    if pos < 0 || pos >= (*map).count {
        *out = [0, 0, 0];
        return;
    }
    let key_ptr = *(*map).insert_order.add(pos as usize);
    let cap = (*map).capacity as usize;
    for i in 0..cap {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED {
            continue;
        }
        if *(*map).keys.add(i) == key_ptr {
            *out = [1, key_ptr, *(*map).vals.add(i)];
            return;
        }
    }
    *out = [0, 0, 0];
}

/// Free all memory associated with the map.
///
/// # Safety
/// `map` must be a non-null pointer returned by `ynz_map_new` and not yet freed.
/// After this call, `map` is a dangling pointer and must not be used.
#[no_mangle]
pub unsafe extern "C" fn ynz_map_drop(map: *mut YnzMap) {
    free((*map).ctrl as *mut core::ffi::c_void);
    free((*map).keys as *mut core::ffi::c_void);
    free((*map).vals as *mut core::ffi::c_void);
    free((*map).insert_order as *mut core::ffi::c_void);
    free(map as *mut core::ffi::c_void);
}

// ── Array runtime (M5 P4a) ────────────────────────────────────────────────────
//
// array<T> is a heap-allocated growable list. All elements are 8 bytes wide —
// int/float/bool stored as i64 bits; string/shape/pointer stored as i64-cast ptr.
// The header struct lives on the heap; the data buffer is a separate allocation.

#[repr(C)]
pub struct YnzArray {
    data: *mut u8,
    len: i64,
    cap: i64,
}

/// Allocate a new empty array with an initial capacity of [`INITIAL_ARRAY_CAPACITY`] elements.
///
/// # Safety
/// Returns a heap pointer. Caller must free with `ynz_array_drop`.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_new() -> *mut YnzArray {
    let cap: i64 = INITIAL_ARRAY_CAPACITY;
    let data = malloc((cap as usize) * 8) as *mut u8;
    if data.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new array. \
                  The program tried to create an array but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent state."
        );
        std::process::abort();
    }
    let hdr = malloc(std::mem::size_of::<YnzArray>()) as *mut YnzArray;
    if hdr.is_null() {
        eprintln!(
            "RUNTIME ERROR: Out of memory while allocating a new array. \
                  The program tried to create an array but the system couldn't \
                  allocate memory. Yinz aborts rather than continuing with \
                  an inconsistent state."
        );
        std::process::abort();
    }
    (*hdr) = YnzArray { data, len: 0, cap };
    hdr
}

/// Push an i64-sized element (int, float bits, bool, or pointer cast to i64).
///
/// Doubles the capacity when full (amortized O(1) push).
///
/// # Safety
/// `arr` must be a non-null pointer returned by `ynz_array_new` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_push(arr: *mut YnzArray, value: i64) {
    if (*arr).len == (*arr).cap {
        let new_cap = (*arr).cap * 2;
        let new_data = realloc(
            (*arr).data as *mut core::ffi::c_void,
            (new_cap as usize) * 8,
        ) as *mut u8;
        if new_data.is_null() {
            eprintln!(
                "RUNTIME ERROR: Out of memory while growing an array. \
                      The program tried to push to an array but the system couldn't \
                      allocate more memory. Yinz aborts rather than continuing with \
                      an inconsistent state."
            );
            std::process::abort();
        }
        (*arr).data = new_data;
        (*arr).cap = new_cap;
    }
    let slot = (*arr).data.add(((*arr).len as usize) * 8) as *mut i64;
    *slot = value;
    (*arr).len += 1;
}

/// Get element at `idx`. Writes `[1, value]` on success or `[0, 0]` on OOB.
///
/// Returns via an out-pointer so codegen can pick apart the result with GEPs
/// without needing aggregate return ABI conventions.
///
/// # Safety
/// `arr` and `out` must be valid non-null pointers.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_get(arr: *const YnzArray, idx: i64, out: *mut [i64; 2]) {
    if idx < 0 || idx >= (*arr).len {
        (*out) = [0, 0];
    } else {
        let slot = (*arr).data.add((idx as usize) * 8) as *const i64;
        (*out) = [1, *slot];
    }
}

/// Set element at `idx`. Aborts if out of bounds (contract: typeck rejects literal OOB).
///
/// # Safety
/// `arr` must be a valid non-null pointer. `idx` must be in [0, len).
#[no_mangle]
pub unsafe extern "C" fn ynz_array_set(arr: *mut YnzArray, idx: i64, value: i64) {
    if idx < 0 || idx >= (*arr).len {
        std::process::abort();
    }
    let slot = (*arr).data.add((idx as usize) * 8) as *mut i64;
    *slot = value;
}

/// Return the number of elements in the array.
///
/// # Safety
/// `arr` must be a valid non-null pointer returned by `ynz_array_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_count(arr: *const YnzArray) -> i64 {
    (*arr).len
}

/// Free the array's data buffer and header. Does not run element destructors.
///
/// # Safety
/// `arr` must be a valid non-null pointer returned by `ynz_array_new` and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_drop(arr: *mut YnzArray) {
    if !(*arr).data.is_null() {
        free((*arr).data as *mut core::ffi::c_void);
        (*arr).data = std::ptr::null_mut();
    }
    free(arr as *mut core::ffi::c_void);
}

/// Clone an array of primitive elements (int/float/bool — each element is an i64 bit
/// pattern) into a fresh independent heap allocation.
///
/// Each element is a raw i64; no element-level indirection. The cloned array has its
/// own data buffer and its own header — mutating either the original's elements OR
/// growing/shrinking the original after this call has NO effect on the clone.
///
/// Used when a `background` task receives an `array<int>` / `array<float>` / `array<bool>`
/// argument: the caller's array must remain independent of the task's array so that
/// post-spawn mutations by the caller are invisible to the task.
///
/// The caller (background task closure) must free the returned pointer with
/// `ynz_array_drop` when the task completes.
///
/// # Safety
/// `src` must be a valid non-null pointer returned by `ynz_array_new` and not yet dropped.
/// Returns null if `src` is null (caller should treat null as a bug, but this avoids UB).
#[no_mangle]
pub unsafe extern "C" fn ynz_array_clone_primitive(src: *mut YnzArray) -> *mut YnzArray {
    if src.is_null() {
        return std::ptr::null_mut();
    }
    let src_ref = &*src;
    let cap = if src_ref.cap > 0 { src_ref.cap } else { 1 };
    let new_data = malloc((cap as usize) * 8) as *mut u8;
    if new_data.is_null() {
        std::process::abort();
    }
    if src_ref.len > 0 && !src_ref.data.is_null() {
        std::ptr::copy_nonoverlapping(src_ref.data, new_data, (src_ref.len as usize) * 8);
    }
    let hdr = malloc(std::mem::size_of::<YnzArray>()) as *mut YnzArray;
    if hdr.is_null() {
        free(new_data as *mut core::ffi::c_void);
        std::process::abort();
    }
    (*hdr) = YnzArray {
        data: new_data,
        len: src_ref.len,
        cap,
    };
    hdr
}

// ── M4/M5: decimal128 → f64 conversion (needed by (Number).toFloat() and (Number).toInt()) ──

/// Convert a decimal128 value (stored as *const [u8;16]) to f64.
///
/// Used by the `.toFloat()` and `.toInt()` intrinsics on `number` values.
/// The conversion formats the decimal128 as a string then parses as f64 —
/// not bit-exact, but correct for any representable decimal128 value.
///
/// # Safety
/// `num_ptr` must be a valid pointer to 16 bytes of decimal128 data.
#[no_mangle]
pub unsafe extern "C" fn ynz_decimal_to_float(num_ptr: *const u8) -> f64 {
    let raw: [u8; 16] = std::slice::from_raw_parts(num_ptr, 16)
        .try_into()
        .unwrap_or([0u8; 16]);
    let bits = u128::from_ne_bytes(raw);
    // Use the ynz-numerics formatter to get a string, then parse as f64.
    let s = ynz_numerics::format(bits);
    s.parse::<f64>().unwrap_or(0.0)
}

// ── M6: string-to-numeric fallible conversions ────────────────────────────────
//
// All functions return a `{ has_value: i64, value: i64 }` pair via a pointer
// (the codegen stores the result into a stack-allocated maybe<T> struct).
// Locked parsing rules from design/narrowing.md:
//   a. Strip ASCII whitespace [0x20, 0x09, 0x0A, 0x0D] from both ends.
//   b. Accept optional leading '+' or '-'.
//   c. .toInt(): only [0-9]+ digits; no prefix, no decimal, no sci notation.
//   d. .toFloat()/.toNumber(): accepts decimal + scientific notation.
//   e. Any failure (empty, lone sign, non-digit chars) → none.

/// ABI: `(ptr: *const u8, len: i64, out: *mut [i64; 2]) -> void`
/// out[0] = has_value (1 or 0), out[1] = the i64 value on success.
///
/// # Safety
/// `ptr` must be valid for `len` bytes. `out` must point to a writable i64[2] array.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_int(ptr: *const u8, len: i64, out: *mut i64) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let result = parse_string_to_int(bytes);
    match result {
        Some(n) => {
            *out = 1;
            *out.add(1) = n;
        }
        None => {
            *out = 0;
            *out.add(1) = 0;
        }
    }
}

/// ABI: `(ptr: *const u8, len: i64, out: *mut [i64; 2]) -> void`
/// out[0] = has_value, out[1] = f64 bits (bit-cast from the double).
///
/// # Safety
/// `ptr` must be valid for `len` bytes. `out` must point to a writable i64[2] array.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_float(ptr: *const u8, len: i64, out: *mut i64) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let result = parse_string_to_float(bytes);
    match result {
        Some(f) => {
            *out = 1;
            *out.add(1) = f64::to_bits(f) as i64;
        }
        None => {
            *out = 0;
            *out.add(1) = 0;
        }
    }
}

/// ABI: `(ptr: *const u8, len: i64, out: *mut [i64; 3]) -> void`
/// out[0] = has_value, out[1..3] = 16-byte decimal128 on success.
///
/// # Safety
/// `ptr` must be valid for `len` bytes. `out` must point to a writable i64[3] array.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_number(ptr: *const u8, len: i64, out: *mut i64) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let trimmed = trim_ascii_ws(bytes);
    // Use ynz-numerics parse (decimal128 string parser).
    // parse() is infallible but returns 0 on bad input; we pre-validate here.
    match std::str::from_utf8(trimmed) {
        Err(_) => {
            *out = 0;
        }
        Ok(s) => {
            if is_valid_number_str(s.as_bytes()) {
                match parse(s) {
                    Some(val) => {
                        let bytes_le = val.to_ne_bytes();
                        *out = 1;
                        let data = out.add(1) as *mut u8;
                        std::ptr::copy_nonoverlapping(bytes_le.as_ptr(), data, 16);
                    }
                    None => {
                        *out = 0;
                    }
                }
            } else {
                *out = 0;
            }
        }
    }
}

/// ABI: `(ptr: *const u8, len: i64) -> *const u8` (null-terminated string, heap-alloc'd copy).
///
/// Used by options .toString() to produce a heap-owned Yinz string from a static byte literal.
/// Allocates and copies the bytes + null terminator. Caller is responsible for freeing
/// (though in practice, toString() results are printed then dropped in the same expression).
///
/// # Safety
/// `ptr` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_from_static(ptr: *const u8, len: i64) -> *const u8 {
    let size = len as usize + 1; // +1 for null terminator
    let buf = malloc(size) as *mut u8;
    if buf.is_null() {
        return c"".as_ptr().cast::<u8>();
    }
    std::ptr::copy_nonoverlapping(ptr, buf, len as usize);
    *buf.add(len as usize) = 0;
    buf as *const u8
}

// ── M6 helper: float → maybe<int> (locked codegen sequence from design/narrowing.md) ──
// This is codegen-side (LLVM IR) in P4; the Rust version is for unit tests only.
// The ACTUAL codegen emits the locked IR directly; this version validates the semantics.

/// Reference implementation of `(float).toInt()` semantics — used in Rust unit tests.
/// The actual codegen emits explicit LLVM IR (see P4 codegen, not this function).
pub fn float_to_int_ref(x: f64) -> Option<i64> {
    if x.is_nan() {
        return None;
    }
    // i64::MAX (2^63-1) is not exactly representable in f64; nearest is 2^63.
    // Upper check: x must be < 2^63 (strictly less, i.e. fits after truncation).
    const I64_MAX_F64: f64 = 9.223372036854776e18_f64; // 2^63
    const I64_MIN_F64: f64 = -9.223372036854776e18_f64; // -2^63
    if !(I64_MIN_F64..I64_MAX_F64).contains(&x) {
        return None;
    }
    Some(x as i64) // truncate toward zero; in-range proven above
}

// ── Private helpers ──────────────────────────────────────────────────────────

fn trim_ascii_ws(bytes: &[u8]) -> &[u8] {
    let is_ws = |b: &u8| matches!(b, 0x20 | 0x09 | 0x0A | 0x0D);
    let start = bytes.iter().position(|b| !is_ws(b)).unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !is_ws(b))
        .map(|i| i + 1)
        .unwrap_or(0);
    if start >= end {
        &[]
    } else {
        &bytes[start..end]
    }
}

fn parse_sign(bytes: &[u8]) -> (bool, &[u8]) {
    match bytes.first() {
        Some(b'+') => (false, &bytes[1..]),
        Some(b'-') => (true, &bytes[1..]),
        _ => (false, bytes),
    }
}

fn parse_string_to_int(bytes: &[u8]) -> Option<i64> {
    let trimmed = trim_ascii_ws(bytes);
    let (neg, digits) = parse_sign(trimmed);
    if digits.is_empty() {
        return None;
    }
    if !digits.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Parse decimal digits without any prefix.
    let mut acc: i64 = 0;
    for &d in digits {
        let digit = (d - b'0') as i64;
        acc = acc.checked_mul(10)?.checked_add(digit)?;
    }
    Some(if neg { acc.checked_neg()? } else { acc })
}

fn parse_string_to_float(bytes: &[u8]) -> Option<f64> {
    let trimmed = trim_ascii_ws(bytes);
    // Peel optional sign, validate the digit structure, then parse the full trimmed string.
    let (_, digits_only) = parse_sign(trimmed);
    if digits_only.is_empty() {
        return None;
    }
    if !is_valid_float_digits(digits_only) {
        return None;
    }
    let s = std::str::from_utf8(trimmed).ok()?;
    let f: f64 = s.parse().ok()?; // Rust parser handles the sign
    if f.is_infinite() {
        return None;
    }
    Some(f)
}

fn is_valid_float_digits(bytes: &[u8]) -> bool {
    // Accepts: [0-9]+ ([\.[0-9]+]? ([eE][+-]?[0-9]+)?
    // Rejects: 0x prefix, 0o prefix, 0b prefix, non-digit chars
    if bytes.starts_with(b"0x") || bytes.starts_with(b"0b") || bytes.starts_with(b"0o") {
        return false;
    }
    let mut i = 0;
    let n = bytes.len();
    // Integer part
    while i < n && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    // Decimal part
    if i < n && bytes[i] == b'.' {
        i += 1;
        while i < n && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    // Exponent
    if i < n && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < n && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < n && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return false;
        } // exponent with no digits
    }
    i == n
}

fn is_valid_number_str(bytes: &[u8]) -> bool {
    // Same rules as float — decimal128 parser uses same format
    is_valid_float_digits(bytes)
}

// ── M7 P4b: string runtime — UTF-8 validation, methods, builder ──────────────
//
// All string functions operate on null-terminated UTF-8 C strings (`*const u8`).
// Functions that return new strings allocate on the heap via `malloc`; callers are
// responsible for freeing the returned pointer (though in practice, short-lived string
// results are consumed by `puts` or another call and the pointer is dropped).
// The SSO 24-byte struct layout is a v0.2 optimization; M7 uses null-terminated UTF-8
// everywhere as the ABI wire format.

/// Validate that `len` bytes starting at `ptr` are valid UTF-8.
///
/// Uses SIMD-accelerated validation via the `simdutf8` crate on supported architectures,
/// with a scalar fallback on unsupported targets.
///
/// Returns 1 if valid UTF-8, 0 otherwise.
///
/// # Safety
/// `ptr` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_validate_utf8(ptr: *const u8, len: usize) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    (simdutf8::basic::from_utf8(bytes).is_ok()) as i32
}

/// Concatenate two null-terminated strings, returning a new heap-allocated null-terminated string.
///
/// # Safety
/// Both `a` and `b` must be valid pointers to null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_concat(a: *const u8, b: *const u8) -> *const u8 {
    let a_str = unsafe { std::ffi::CStr::from_ptr(a as *const i8) }.to_bytes();
    let b_str = unsafe { std::ffi::CStr::from_ptr(b as *const i8) }.to_bytes();
    let total = a_str.len() + b_str.len() + 1;
    let buf = malloc(total) as *mut u8;
    if buf.is_null() {
        return c"".as_ptr().cast::<u8>();
    }
    std::ptr::copy_nonoverlapping(a_str.as_ptr(), buf, a_str.len());
    std::ptr::copy_nonoverlapping(b_str.as_ptr(), buf.add(a_str.len()), b_str.len());
    *buf.add(a_str.len() + b_str.len()) = 0;
    buf as *const u8
}

/// Count Unicode code points in a null-terminated UTF-8 string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_count(s: *const u8) -> i64 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    s_str.chars().count() as i64
}

/// Return the byte length (strlen) of a null-terminated string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_byte_count(s: *const u8) -> i64 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    s_str.len() as i64
}

/// Return the Unicode code point at index `n` (0-based) as a new null-terminated string.
///
/// Returns null if `n` is out of bounds.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_codepoint_at(s: *const u8, n: i64) -> *const u8 {
    if n < 0 {
        return std::ptr::null();
    }
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    match s_str.chars().nth(n as usize) {
        None => std::ptr::null(),
        Some(ch) => {
            let mut buf = [0u8; 5];
            let encoded = ch.encode_utf8(&mut buf[..4]);
            let len = encoded.len();
            let heap = malloc(len + 1) as *mut u8;
            if heap.is_null() {
                return std::ptr::null();
            }
            std::ptr::copy_nonoverlapping(encoded.as_ptr(), heap, len);
            *heap.add(len) = 0;
            heap as *const u8
        }
    }
}

/// Return the byte value at index `n` (0-based), or -1 if out of bounds.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_byte_at(s: *const u8, n: i64) -> i64 {
    if n < 0 {
        return -1;
    }
    let s_bytes = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    match s_bytes.get(n as usize) {
        Some(&b) => b as i64,
        None => -1,
    }
}

/// Test whether `s` contains `substr` as a substring.
///
/// Uses SIMD-accelerated search via the `memchr` crate's `memmem::find`.
///
/// Returns 1 if found, 0 otherwise.
///
/// # Safety
/// Both pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_contains(s: *const u8, substr: *const u8) -> i32 {
    let haystack = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    let needle = unsafe { std::ffi::CStr::from_ptr(substr as *const i8) }.to_bytes();
    if needle.is_empty() {
        return 1;
    }
    (memchr::memmem::find(haystack, needle).is_some()) as i32
}

/// Return the byte offset of the first occurrence of `substr` in `s`, or -1 if not found.
///
/// # Safety
/// Both pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_index_of(s: *const u8, substr: *const u8) -> i64 {
    let haystack = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    let needle = unsafe { std::ffi::CStr::from_ptr(substr as *const i8) }.to_bytes();
    if needle.is_empty() {
        return 0;
    }
    match memchr::memmem::find(haystack, needle) {
        Some(offset) => offset as i64,
        None => -1,
    }
}

/// Test whether `s` starts with `prefix`.
///
/// Returns 1 if true, 0 otherwise.
///
/// # Safety
/// Both pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_starts_with(s: *const u8, prefix: *const u8) -> i32 {
    let s_bytes = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    let p_bytes = unsafe { std::ffi::CStr::from_ptr(prefix as *const i8) }.to_bytes();
    s_bytes.starts_with(p_bytes) as i32
}

/// Test whether `s` ends with `suffix`.
///
/// Returns 1 if true, 0 otherwise.
///
/// # Safety
/// Both pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_ends_with(s: *const u8, suffix: *const u8) -> i32 {
    let s_bytes = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    let sfx_bytes = unsafe { std::ffi::CStr::from_ptr(suffix as *const i8) }.to_bytes();
    s_bytes.ends_with(sfx_bytes) as i32
}

/// Convert a null-terminated UTF-8 string to uppercase using locale-invariant Unicode rules.
///
/// Returns a new heap-allocated null-terminated string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_upper(s: *const u8) -> *const u8 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    // unicase::UniCase provides locale-invariant case conversion; we use the standard
    // Unicode uppercase mapping directly since UniCase focuses on case folding.
    let upper: String = s_str.chars().flat_map(|c| c.to_uppercase()).collect();
    heap_string_from_str(&upper)
}

/// Convert a null-terminated UTF-8 string to lowercase using locale-invariant Unicode rules.
///
/// Returns a new heap-allocated null-terminated string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_lower(s: *const u8) -> *const u8 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    // Use locale-invariant Unicode lowercase mapping (avoids Turkish I problem).
    let lower: String = s_str.chars().flat_map(|c| c.to_lowercase()).collect();
    heap_string_from_str(&lower)
}

/// Return a substring of `s` from code-point index `start` (inclusive) to `end` (exclusive).
///
/// Clamps indices to valid range. Returns a new heap-allocated null-terminated string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_substring(s: *const u8, start: i64, end: i64) -> *const u8 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    let len = s_str.chars().count() as i64;
    let start = start.max(0).min(len) as usize;
    let end = end.max(0).min(len) as usize;
    let slice: String = s_str
        .chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();
    heap_string_from_str(&slice)
}

/// Trim leading and trailing ASCII whitespace from `s`.
///
/// Returns a new heap-allocated null-terminated string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_trim(s: *const u8) -> *const u8 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    heap_string_from_str(s_str.trim())
}

/// Count Unicode grapheme clusters in a null-terminated UTF-8 string.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_grapheme_count(s: *const u8) -> i64 {
    use unicode_segmentation::UnicodeSegmentation;
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    s_str.graphemes(true).count() as i64
}

/// Return the grapheme cluster at index `n` (0-based) as a new heap-allocated null-terminated string.
///
/// Returns null if `n` is out of bounds.
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_grapheme_at(s: *const u8, n: i64) -> *const u8 {
    use unicode_segmentation::UnicodeSegmentation;
    if n < 0 {
        return std::ptr::null();
    }
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    match s_str.graphemes(true).nth(n as usize) {
        None => std::ptr::null(),
        Some(g) => heap_string_from_str(g),
    }
}

/// Split `s` on every occurrence of `sep`, returning a heap-allocated `YnzArray` of string pointers.
///
/// Each string in the array is a heap-allocated null-terminated copy.
///
/// # Safety
/// Both pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_split(s: *const u8, sep: *const u8) -> *mut YnzArray {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    let sep_str = unsafe { std::ffi::CStr::from_ptr(sep as *const i8) }
        .to_str()
        .unwrap_or("");
    let arr = ynz_array_new();
    if sep_str.is_empty() {
        // Split into individual code-point strings.
        for ch in s_str.chars() {
            let mut buf = [0u8; 5];
            let enc = ch.encode_utf8(&mut buf[..4]);
            let ptr = heap_string_from_str(enc);
            ynz_array_push(arr, ptr as i64);
        }
    } else {
        for part in s_str.split(sep_str) {
            let ptr = heap_string_from_str(part);
            ynz_array_push(arr, ptr as i64);
        }
    }
    arr
}

/// Replace all occurrences of `from` in `s` with `to`.
///
/// Returns a new heap-allocated null-terminated string.
///
/// # Safety
/// All pointers must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_replace(
    s: *const u8,
    from: *const u8,
    to: *const u8,
) -> *const u8 {
    let s_str = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }
        .to_str()
        .unwrap_or("");
    let from_str = unsafe { std::ffi::CStr::from_ptr(from as *const i8) }
        .to_str()
        .unwrap_or("");
    let to_str = unsafe { std::ffi::CStr::from_ptr(to as *const i8) }
        .to_str()
        .unwrap_or("");
    if from_str.is_empty() {
        return ynz_string_from_static(s, ynz_string_byte_count(s));
    }
    let result = s_str.replace(from_str, to_str);
    heap_string_from_str(&result)
}

// ── String interpolation builder ──────────────────────────────────────────────
//
// The builder accumulates segments into a growable Vec<u8>, then finalizes into
// a null-terminated heap string. Represented as a `Box<Vec<u8>>` passed as `*mut u8`.

/// Create a new string builder. Returns an opaque `*mut u8` handle.
///
/// The caller must eventually call `ynz_string_builder_finalize` or
/// `ynz_string_builder_drop` to free the builder's memory.
#[no_mangle]
pub extern "C" fn ynz_string_builder_new() -> *mut u8 {
    Box::into_raw(Box::new(Vec::<u8>::new())) as *mut u8
}

/// Append a null-terminated string to the builder.
///
/// # Safety
/// `builder` must be a valid handle returned by `ynz_string_builder_new`.
/// `s` must be a valid pointer to a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_builder_append(builder: *mut u8, s: *const u8) {
    let vec = unsafe { &mut *(builder as *mut Vec<u8>) };
    let bytes = unsafe { std::ffi::CStr::from_ptr(s as *const i8) }.to_bytes();
    vec.extend_from_slice(bytes);
}

/// Finalize the builder into a new heap-allocated null-terminated string.
///
/// The builder handle is consumed (freed) by this call.
///
/// # Safety
/// `builder` must be a valid handle returned by `ynz_string_builder_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_builder_finalize(builder: *mut u8) -> *const u8 {
    let mut vec = unsafe { *Box::from_raw(builder as *mut Vec<u8>) };
    vec.push(0); // null-terminate
    let len = vec.len();
    let buf = malloc(len) as *mut u8;
    if buf.is_null() {
        return c"".as_ptr().cast::<u8>();
    }
    std::ptr::copy_nonoverlapping(vec.as_ptr(), buf, len);
    buf as *const u8
}

/// Free the builder without producing a string.
///
/// # Safety
/// `builder` must be a valid handle returned by `ynz_string_builder_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_builder_drop(builder: *mut u8) {
    drop(unsafe { Box::from_raw(builder as *mut Vec<u8>) });
}

/// Internal helper: allocate a heap null-terminated copy of a Rust `&str`.
fn heap_string_from_str(s: &str) -> *const u8 {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let buf = unsafe { malloc(len + 1) as *mut u8 };
    if buf.is_null() {
        return c"".as_ptr().cast::<u8>();
    }
    if len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
        }
    }
    unsafe {
        *buf.add(len) = 0;
    }
    buf as *const u8
}

// ── M7 P4a: errors runtime — YnzError, YnzFrame, frame stack ─────────────────
//
// Error struct carries a message and a snapshot of the call-chain at the moment
// ynz_error_new was called.  The frame stack is thread-local; each errors-capable
// function pushes a frame on entry and pops on exit (normal or early-return).
//
// ABI contract (locked M7 P4a):
//   - ynz_error_new: caller owns the message pointer (usually a static string
//     from IR). The runtime copies the frame stack snapshot but NOT the message
//     bytes — the message must outlive the error (static string literals do).
//   - ynz_error_drop: frees the frame snapshot. Does NOT free the message.
//   - ynz_frame_push / ynz_frame_pop: thread-local stack; capped at 1024.

/// C-ABI representation of a single call-chain frame.
#[repr(C)]
pub struct YnzFrame {
    pub file: *const u8,
    /// Source line, or -1 when not available.
    pub line: i64,
    pub function: *const u8,
}

/// C-ABI representation of a runtime error.
///
/// Layout fields 0–1 are for suggestions (not yet populated in M7 P4a —
/// null pointer + 0 length). Fields 2–3 are the trace snapshot.
#[repr(C)]
pub struct YnzError {
    /// Null-terminated UTF-8 message string. NOT owned by this struct.
    pub message: *const u8,
    /// Reserved: suggestion strings. Always null / 0 in M7 P4a.
    pub suggestions_ptr: *const *const u8,
    pub suggestions_len: i64,
    /// Heap-allocated copy of the frame stack at ynz_error_new time.
    pub trace_ptr: *mut YnzFrame,
    pub trace_len: i64,
    pub source_file: *const u8,
    /// -1 when no source location is available.
    pub source_line: i64,
}

const FRAME_STACK_LIMIT: usize = 1024;

thread_local! {
    static FRAME_STACK: std::cell::RefCell<Vec<(*const u8, i64, *const u8)>> =
        std::cell::RefCell::new(Vec::with_capacity(64));
}

/// Push a frame onto the thread-local call-chain stack.
///
/// `file` and `function` must be valid null-terminated C strings for the duration
/// of the call. In practice they are static string literals from the compiled IR.
///
/// Frames beyond the 1024-entry limit are silently dropped (truncation, not abort).
///
/// # Safety
/// `file` and `function` must be valid pointers to null-terminated byte strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_frame_push(file: *const u8, line: i64, function: *const u8) {
    FRAME_STACK.with(|stack| {
        let mut s = stack.borrow_mut();
        if s.len() < FRAME_STACK_LIMIT {
            s.push((file, line, function));
        }
    });
}

/// Pop the most recent frame from the thread-local call-chain stack.
///
/// No-op when the stack is already empty.
#[no_mangle]
pub extern "C" fn ynz_frame_pop() {
    FRAME_STACK.with(|stack| {
        stack.borrow_mut().pop();
    });
}

/// Allocate a new `YnzError` with the given message.
///
/// The current thread-local frame stack is snapshotted into the error's trace
/// buffer. The message pointer is stored as-is (not copied) — it MUST be a
/// static string literal (which all IR-generated error messages are).
///
/// # Safety
/// `message` must be a valid pointer to a null-terminated byte string.
#[no_mangle]
pub unsafe extern "C" fn ynz_error_new(message: *const u8) -> *mut YnzError {
    let err = malloc(std::mem::size_of::<YnzError>()) as *mut YnzError;
    if err.is_null() {
        std::process::abort();
    }

    // Snapshot the frame stack.
    let (trace_ptr, trace_len) = FRAME_STACK.with(|stack| {
        let s = stack.borrow();
        let len = s.len();
        if len == 0 {
            return (std::ptr::null_mut::<YnzFrame>(), 0i64);
        }
        let frames_mem = malloc(len * std::mem::size_of::<YnzFrame>()) as *mut YnzFrame;
        if frames_mem.is_null() {
            std::process::abort();
        }
        for (i, &(file, line, function)) in s.iter().enumerate() {
            *frames_mem.add(i) = YnzFrame {
                file,
                line,
                function,
            };
        }
        (frames_mem, len as i64)
    });

    *err = YnzError {
        message,
        suggestions_ptr: std::ptr::null(),
        suggestions_len: 0,
        trace_ptr,
        trace_len,
        source_file: std::ptr::null(),
        source_line: -1,
    };
    err
}

/// Free an error and its trace snapshot.
///
/// Does NOT free the `message` pointer (it is static; owned by the IR).
///
/// # Safety
/// `err` must be a valid non-null pointer returned by `ynz_error_new` and not
/// yet freed.
#[no_mangle]
pub unsafe extern "C" fn ynz_error_drop(err: *mut YnzError) {
    if err.is_null() {
        return;
    }
    if !(*err).trace_ptr.is_null() {
        free((*err).trace_ptr as *mut core::ffi::c_void);
    }
    free(err as *mut core::ffi::c_void);
}

/// Return the message pointer from an error.
///
/// # Safety
/// `err` must be a valid non-null pointer returned by `ynz_error_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_error_message(err: *const YnzError) -> *const u8 {
    (*err).message
}

/// Return the number of frames in the error's trace.
///
/// # Safety
/// `err` must be a valid non-null pointer returned by `ynz_error_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_error_trace_len(err: *const YnzError) -> i64 {
    (*err).trace_len
}

/// Return a pointer to the frame at `idx` in the error's trace.
///
/// Returns null if `idx` is out of range.
///
/// # Safety
/// `err` must be a valid non-null pointer returned by `ynz_error_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_error_trace_frame(err: *const YnzError, idx: i64) -> *const YnzFrame {
    if idx < 0 || idx >= (*err).trace_len {
        return std::ptr::null();
    }
    (*err).trace_ptr.add(idx as usize) as *const YnzFrame
}

/// Return the file pointer from a frame.
///
/// # Safety
/// `frame` must be a valid non-null pointer returned by `ynz_error_trace_frame`.
#[no_mangle]
pub unsafe extern "C" fn ynz_frame_file(frame: *const YnzFrame) -> *const u8 {
    (*frame).file
}

/// Return the source line from a frame (-1 when not available).
///
/// # Safety
/// `frame` must be a valid non-null pointer returned by `ynz_error_trace_frame`.
#[no_mangle]
pub unsafe extern "C" fn ynz_frame_line(frame: *const YnzFrame) -> i64 {
    (*frame).line
}

/// Return the function name pointer from a frame.
///
/// # Safety
/// `frame` must be a valid non-null pointer returned by `ynz_error_trace_frame`.
#[no_mangle]
pub unsafe extern "C" fn ynz_frame_function(frame: *const YnzFrame) -> *const u8 {
    (*frame).function
}

/// Called when an errors-capable error propagates out of `main()` without being
/// handled.  Prints the error message and trace to stderr then exits with code 1.
///
/// # Safety
/// `err` must be a valid non-null pointer returned by `ynz_error_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_unhandled_error(err: *const YnzError) -> ! {
    let msg = if (*err).message.is_null() {
        "<no message>"
    } else {
        cstr_to_str((*err).message)
    };
    eprintln!("RUNTIME ERROR: unhandled error: {msg}");
    if (*err).trace_len > 0 {
        eprintln!("  Call trace (most recent last):");
        for i in 0..(*err).trace_len {
            let frame = (*err).trace_ptr.add(i as usize);
            let fn_name = if (*frame).function.is_null() {
                "<unknown>"
            } else {
                cstr_to_str((*frame).function)
            };
            let file = if (*frame).file.is_null() {
                "<unknown>"
            } else {
                cstr_to_str((*frame).file)
            };
            let line = (*frame).line;
            if line >= 0 {
                eprintln!("    {fn_name} ({file}:{line})");
            } else {
                eprintln!("    {fn_name} ({file})");
            }
        }
    }
    std::process::exit(1);
}

// Unit tests for M6 string parsing semantics (locked test vectors from design/narrowing.md)
#[cfg(test)]
mod m6_string_parsing {
    use super::*;

    #[test]
    fn int_basic() {
        assert_eq!(parse_string_to_int(b"42"), Some(42));
    }
    #[test]
    fn int_whitespace_sign() {
        assert_eq!(parse_string_to_int(b"  +42  "), Some(42));
    }
    #[test]
    fn int_negative() {
        assert_eq!(parse_string_to_int(b"-42"), Some(-42));
    }
    #[test]
    fn int_empty() {
        assert_eq!(parse_string_to_int(b""), None);
    }
    #[test]
    fn int_whitespace_only() {
        assert_eq!(parse_string_to_int(b"  "), None);
    }
    #[test]
    fn int_lone_sign() {
        assert_eq!(parse_string_to_int(b"+"), None);
    }
    #[test]
    fn int_hex_prefix() {
        assert_eq!(parse_string_to_int(b"0x1A"), None);
    }
    #[test]
    fn int_trailing_chars() {
        assert_eq!(parse_string_to_int(b"42 hello"), None);
    }
    #[test]
    fn int_fractional() {
        assert_eq!(parse_string_to_int(b"42.5"), None);
    }
    #[test]
    fn int_scientific() {
        assert_eq!(parse_string_to_int(b"1e3"), None);
    }
    #[test]
    fn int_tab_lf() {
        assert_eq!(parse_string_to_int(b"\t42\n"), Some(42));
    }
    #[test]
    fn int_non_breaking_space() {
        assert_eq!(parse_string_to_int(b"\xC2\xA042"), None);
    } // UTF-8 U+00A0
    #[test]
    fn float_basic() {
        assert_eq!(parse_string_to_float(b"1.5"), Some(1.5));
    }
    #[test]
    fn float_scientific() {
        assert_eq!(parse_string_to_float(b"1.5e2"), Some(150.0));
    }
    #[test]
    fn float_negative() {
        assert_eq!(parse_string_to_float(b"  -1.5  "), Some(-1.5));
    }
    #[test]
    fn float_bad() {
        assert_eq!(parse_string_to_float(b"abc"), None);
    }
    #[test]
    fn float_double_dot() {
        assert_eq!(parse_string_to_float(b"1.5.5"), None);
    }
    #[test]
    fn float_to_int_nan() {
        assert_eq!(float_to_int_ref(f64::NAN), None);
    }
    #[test]
    fn float_to_int_inf() {
        assert_eq!(float_to_int_ref(f64::INFINITY), None);
    }
    #[test]
    fn float_to_int_oor() {
        assert_eq!(float_to_int_ref(1e30), None);
    }
    #[test]
    fn float_to_int_truncate() {
        assert_eq!(float_to_int_ref(2.5), Some(2));
    }
    #[test]
    fn float_to_int_neg_truncate() {
        assert_eq!(float_to_int_ref(-2.5), Some(-2));
    }
    #[test]
    fn float_to_int_boundary_upper() {
        assert_eq!(float_to_int_ref(9.223372036854776e18), None);
    }
    #[test]
    fn float_to_int_boundary_lower() {
        assert_eq!(float_to_int_ref(-9.223372036854776e18), Some(i64::MIN));
    }
}

// ── M7 P4a: errors runtime tests ──────────────────────────────────────────────
#[cfg(test)]
mod m7_errors_runtime {
    use super::*;

    #[test]
    fn frame_push_pop_round_trip() {
        // WHY: frame push and pop must be symmetric. A push with no matching pop
        // would leak a frame onto all subsequent error traces (silent trace corruption).
        unsafe {
            // Start clean — pop any frames left from other tests.
            FRAME_STACK.with(|s| s.borrow_mut().clear());

            ynz_frame_push(c"test.ynz".as_ptr().cast(), 10, c"myFn".as_ptr().cast());
            let len_after_push = FRAME_STACK.with(|s| s.borrow().len());
            assert_eq!(len_after_push, 1);

            ynz_frame_pop();
            let len_after_pop = FRAME_STACK.with(|s| s.borrow().len());
            assert_eq!(len_after_pop, 0);
        }
    }

    #[test]
    fn frame_pop_on_empty_is_noop() {
        // WHY: a pop on an empty stack must not panic or corrupt memory.
        // Early-return paths in errors functions always call pop, even when the
        // stack was cleared by a prior error path in the same call.
        FRAME_STACK.with(|s| s.borrow_mut().clear());
        ynz_frame_pop(); // must not panic
    }

    #[test]
    fn error_new_captures_message() {
        // WHY: ynz_error_new must store the message pointer exactly as given so
        // ynz_error_message returns the same address. If message is null-ed out
        // or replaced, runtime error messages print garbage.
        FRAME_STACK.with(|s| s.borrow_mut().clear());
        unsafe {
            let msg = b"something went wrong\0";
            let err = ynz_error_new(msg.as_ptr());
            assert!(!err.is_null());
            let retrieved = ynz_error_message(err);
            assert_eq!(retrieved, msg.as_ptr());
            ynz_error_drop(err);
        }
    }

    #[test]
    fn error_new_snapshots_frame_stack() {
        // WHY: the error's trace must capture the frame stack AT the moment
        // ynz_error_new is called, not later. Auto-propagation pops frames before
        // the caller sees the error, so the snapshot must be taken first.
        FRAME_STACK.with(|s| s.borrow_mut().clear());
        unsafe {
            ynz_frame_push(c"a.ynz".as_ptr().cast(), 1, c"fn_a".as_ptr().cast());
            ynz_frame_push(c"b.ynz".as_ptr().cast(), 2, c"fn_b".as_ptr().cast());

            let err = ynz_error_new(c"oops".as_ptr().cast());
            assert_eq!(ynz_error_trace_len(err), 2);

            let f0 = ynz_error_trace_frame(err, 0);
            assert!(!f0.is_null());
            assert_eq!(ynz_frame_line(f0), 1);

            let f1 = ynz_error_trace_frame(err, 1);
            assert!(!f1.is_null());
            assert_eq!(ynz_frame_line(f1), 2);

            // Out-of-range frame must return null.
            let f_oob = ynz_error_trace_frame(err, 5);
            assert!(f_oob.is_null());

            ynz_error_drop(err);
            // Clean up the stack.
            ynz_frame_pop();
            ynz_frame_pop();
        }
    }

    #[test]
    fn error_new_empty_stack_gives_zero_trace() {
        // WHY: when no frames are on the stack (e.g., a top-level errors call from
        // main with no frame push yet), the error must have trace_len = 0 and
        // trace_ptr = null. ynz_unhandled_error must handle this gracefully.
        FRAME_STACK.with(|s| s.borrow_mut().clear());
        unsafe {
            let err = ynz_error_new(c"empty".as_ptr().cast());
            assert_eq!(ynz_error_trace_len(err), 0);
            let f = ynz_error_trace_frame(err, 0);
            assert!(f.is_null());
            ynz_error_drop(err);
        }
    }

    #[test]
    fn frame_stack_caps_at_1024() {
        // WHY: unbounded frame stacks would exhaust memory on deeply recursive
        // programs. The 1024 cap must truncate silently, not abort.
        FRAME_STACK.with(|s| s.borrow_mut().clear());
        unsafe {
            for i in 0..2000i64 {
                ynz_frame_push(c"f.ynz".as_ptr().cast(), i, c"deep".as_ptr().cast());
            }
            let len = FRAME_STACK.with(|s| s.borrow().len());
            assert_eq!(len, FRAME_STACK_LIMIT);
            // Clean up.
            FRAME_STACK.with(|s| s.borrow_mut().clear());
        }
    }
}

// ── M7 P4b: string runtime tests ──────────────────────────────────────────────
#[cfg(test)]
mod m7_string_runtime {
    use super::*;

    unsafe fn c(s: &'static [u8]) -> *const u8 {
        s.as_ptr()
    }
    unsafe fn str_from_ptr(p: *const u8) -> &'static str {
        std::ffi::CStr::from_ptr(p as *const i8).to_str().unwrap()
    }

    #[test]
    fn nfc_equality_ascii_fast_path() {
        // WHY: ASCII strings must go through the fast path (byte comparison).
        // If ASCII strings disagree despite being byte-equal, the fast path is broken.
        unsafe {
            assert_eq!(ynz_string_eq(c(b"hello\0"), c(b"hello\0")), 1);
            assert_eq!(ynz_string_eq(c(b"hello\0"), c(b"world\0")), 0);
        }
    }

    #[test]
    fn nfc_equality_pointer_identity() {
        // WHY: two pointers to the same address must be equal without normalizing.
        // Pointer identity fast path must fire before any normalization.
        unsafe {
            let s = b"hello\0";
            assert_eq!(ynz_string_eq(c(s), c(s)), 1);
        }
    }

    #[test]
    fn nfc_equality_precomposed_vs_decomposed() {
        // WHY: NFC normalization closes the canonical-equivalence obligation from M3.
        // Precomposed é (U+00E9) and decomposed e + combining accent (U+0065 U+0301)
        // must compare as equal. Without NFC normalization, byte comparison returns 0.
        unsafe {
            // U+00E9 = precomposed é (UTF-8 bytes: 0xC3 0xA9)
            let precomposed = b"\xC3\xA9\0";
            // U+0065 + U+0301 = e (0x65) + combining acute accent (UTF-8: 0xCC 0x81)
            let decomposed = b"\x65\xCC\x81\0";
            assert_eq!(
                ynz_string_eq(c(precomposed), c(decomposed)),
                1,
                "NFC: precomposed e-acute must equal decomposed e + combining accent"
            );
        }
    }

    #[test]
    fn string_concat_produces_null_terminated() {
        // WHY: concatenation must produce a null-terminated result that can be passed
        // to puts() or any other C string function without corruption.
        unsafe {
            let result = ynz_string_concat(c(b"Hello\0"), c(b" World\0"));
            assert!(!result.is_null());
            assert_eq!(str_from_ptr(result), "Hello World");
            free(result as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_concat_empty_strings() {
        // WHY: concatenating two empty strings must produce an empty null-terminated string.
        unsafe {
            let r = ynz_string_concat(c(b"\0"), c(b"\0"));
            assert!(!r.is_null());
            assert_eq!(str_from_ptr(r), "");
            free(r as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_count_handles_multibyte() {
        // WHY: code point count must count code points, not bytes.
        // "café" is 4 code points but 5 bytes (é = 2 UTF-8 bytes).
        unsafe {
            // café: c-a-f-é (4 code points, 5 bytes). é = UTF-8 bytes 0xC3 0xA9.
            let s = b"caf\xC3\xA9\0";
            assert_eq!(ynz_string_count(c(s)), 4);
        }
    }

    #[test]
    fn string_byte_count_vs_code_point_count() {
        // WHY: byteCount must return the byte count, not the code-point count.
        unsafe {
            let s = b"caf\xC3\xA9\0"; // 4 code points, 5 bytes (+ null terminator)
            assert_eq!(ynz_string_byte_count(c(s)), 5);
            assert_eq!(ynz_string_count(c(s)), 4);
        }
    }

    #[test]
    fn string_codepoint_at_ascii() {
        // WHY: codepoint_at on ASCII must return each character as a 1-byte string.
        unsafe {
            let s = b"hello\0";
            let ch = ynz_string_codepoint_at(c(s), 1); // 'e'
            assert!(!ch.is_null());
            assert_eq!(str_from_ptr(ch), "e");
            free(ch as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_codepoint_at_oob_returns_null() {
        // WHY: out-of-bounds index must return null, not garbage or a panic.
        unsafe {
            let s = b"hi\0";
            assert!(ynz_string_codepoint_at(c(s), 10).is_null());
            assert!(ynz_string_codepoint_at(c(s), -1).is_null());
        }
    }

    #[test]
    fn string_contains_basic() {
        // WHY: contains must correctly find a substring.
        unsafe {
            assert_eq!(ynz_string_contains(c(b"Hello World\0"), c(b"World\0")), 1);
            assert_eq!(ynz_string_contains(c(b"Hello World\0"), c(b"xyz\0")), 0);
            assert_eq!(ynz_string_contains(c(b"Hello\0"), c(b"\0")), 1); // empty string
        }
    }

    #[test]
    fn string_starts_with_ends_with() {
        // WHY: startsWith and endsWith must match the prefix/suffix exactly.
        unsafe {
            assert_eq!(
                ynz_string_starts_with(c(b"Hello World\0"), c(b"Hello\0")),
                1
            );
            assert_eq!(
                ynz_string_starts_with(c(b"Hello World\0"), c(b"World\0")),
                0
            );
            assert_eq!(ynz_string_ends_with(c(b"Hello World\0"), c(b"World\0")), 1);
            assert_eq!(ynz_string_ends_with(c(b"Hello World\0"), c(b"Hello\0")), 0);
        }
    }

    #[test]
    fn string_to_upper_lower() {
        // WHY: locale-invariant case conversion must not be affected by the system locale.
        // The Turkish-I problem (where 'i'.toUpperCase() = 'İ') must not occur.
        unsafe {
            let upper = ynz_string_to_upper(c(b"hello\0"));
            assert!(!upper.is_null());
            assert_eq!(str_from_ptr(upper), "HELLO");
            free(upper as *mut core::ffi::c_void);

            let lower = ynz_string_to_lower(c(b"HELLO\0"));
            assert!(!lower.is_null());
            assert_eq!(str_from_ptr(lower), "hello");
            free(lower as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_trim_removes_whitespace() {
        // WHY: trim must remove leading and trailing whitespace, not interior whitespace.
        unsafe {
            let trimmed = ynz_string_trim(c(b"  hello  \0"));
            assert!(!trimmed.is_null());
            assert_eq!(str_from_ptr(trimmed), "hello");
            free(trimmed as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_substring_basic() {
        // WHY: substring must slice by code-point index, not byte index.
        unsafe {
            let sub = ynz_string_substring(c(b"hello\0"), 1, 4); // "ell"
            assert!(!sub.is_null());
            assert_eq!(str_from_ptr(sub), "ell");
            free(sub as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_grapheme_count_combining() {
        // WHY: grapheme count must count grapheme clusters, not code points.
        // 'a' + combining acute accent (U+0301) = 1 grapheme cluster, 2 code points.
        unsafe {
            // 'a' = 0x61, U+0301 combining acute = UTF-8 0xCC 0x81
            let s = b"a\xCC\x81\0";
            assert_eq!(ynz_string_grapheme_count(c(s)), 1);
            assert_eq!(ynz_string_count(c(s)), 2); // 2 code points
        }
    }

    #[test]
    fn string_builder_round_trip() {
        // WHY: the builder must correctly assemble multiple segments into a single string.
        // Used by interpolated string codegen: builder_new, N×builder_append, builder_finalize.
        unsafe {
            let b = ynz_string_builder_new();
            ynz_string_builder_append(b, c(b"Hello\0"));
            ynz_string_builder_append(b, c(b", \0"));
            ynz_string_builder_append(b, c(b"World!\0"));
            let result = ynz_string_builder_finalize(b);
            assert!(!result.is_null());
            assert_eq!(str_from_ptr(result), "Hello, World!");
            free(result as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_builder_drop_cleans_up() {
        // WHY: builder_drop must not leak memory. Valgrind-clean programs depend on this.
        // We can't test for leaks directly, but we can assert no panic occurs.
        let b = ynz_string_builder_new();
        unsafe {
            ynz_string_builder_drop(b);
        }
    }

    #[test]
    fn string_split_basic() {
        // WHY: split must produce the correct number of segments for a known separator.
        unsafe {
            let parts = ynz_string_split(c(b"a,b,c\0"), c(b",\0"));
            assert!(!parts.is_null());
            assert_eq!(ynz_array_count(parts), 3);
            let mut out = [0i64; 2];
            ynz_array_get(parts, 0, &mut out);
            assert_eq!(out[0], 1);
            assert_eq!(str_from_ptr(out[1] as *const u8), "a");
            ynz_array_drop(parts);
        }
    }

    #[test]
    fn string_replace_basic() {
        // WHY: replace must substitute all occurrences of `from` with `to`.
        unsafe {
            let result = ynz_string_replace(c(b"hello world\0"), c(b"world\0"), c(b"Yinz\0"));
            assert!(!result.is_null());
            assert_eq!(str_from_ptr(result), "hello Yinz");
            free(result as *mut core::ffi::c_void);
        }
    }

    #[test]
    fn string_index_of_found_and_not_found() {
        // WHY: indexOf must return the byte offset on hit and -1 on miss.
        unsafe {
            assert_eq!(ynz_string_index_of(c(b"hello world\0"), c(b"world\0")), 6);
            assert_eq!(ynz_string_index_of(c(b"hello world\0"), c(b"xyz\0")), -1);
        }
    }

    #[test]
    fn string_validate_utf8_valid_and_invalid() {
        // WHY: UTF-8 validation must accept well-formed UTF-8 and reject ill-formed sequences.
        unsafe {
            // Valid UTF-8: ASCII
            assert_eq!(ynz_string_validate_utf8(b"hello".as_ptr(), 5), 1);
            // Valid UTF-8: 2-byte sequence é
            assert_eq!(ynz_string_validate_utf8(b"\xC3\xA9".as_ptr(), 2), 1);
            // Invalid: orphan continuation byte
            assert_eq!(ynz_string_validate_utf8(b"\x80".as_ptr(), 1), 0);
        }
    }
}

// ── M5 P4a: array runtime tests ───────────────────────────────────────────────
#[cfg(test)]
mod array_runtime {
    use super::*;

    #[test]
    fn array_new_push_get_count() {
        // WHY: guards that ynz_array_new initialises a valid empty array and
        // that ynz_array_push grows it correctly.  Any regression in the null-
        // check or capacity arithmetic trips these asserts.
        unsafe {
            let arr = ynz_array_new();
            assert!(!arr.is_null(), "ynz_array_new must return non-null");
            assert_eq!(ynz_array_count(arr), 0, "new array must have count 0");

            ynz_array_push(arr, 10);
            ynz_array_push(arr, 20);
            ynz_array_push(arr, 30);
            assert_eq!(ynz_array_count(arr), 3, "count must equal number of pushes");

            let mut out = [0i64; 2];
            ynz_array_get(arr, 0, &mut out);
            assert_eq!(out, [1, 10], "get(0) must return [1, 10]");
            ynz_array_get(arr, 2, &mut out);
            assert_eq!(out, [1, 30], "get(2) must return [1, 30]");
            ynz_array_get(arr, 3, &mut out);
            assert_eq!(out, [0, 0], "get(OOB) must return [0, 0]");

            ynz_array_drop(arr);
        }
    }

    #[test]
    fn array_push_beyond_initial_capacity_grows() {
        // WHY: INITIAL_ARRAY_CAPACITY = 8; pushing 16 elements forces one realloc.
        // Verifies the realloc null-check path doesn't break normal growth.
        unsafe {
            let arr = ynz_array_new();
            for i in 0..16i64 {
                ynz_array_push(arr, i * 100);
            }
            assert_eq!(
                ynz_array_count(arr),
                16,
                "must have 16 elements after growth"
            );
            let mut out = [0i64; 2];
            ynz_array_get(arr, 15, &mut out);
            assert_eq!(out, [1, 1500], "element 15 must be 15*100 = 1500");
            ynz_array_drop(arr);
        }
    }
}

// ── M8 P6: bignum runtime — `number<N>` for N > 34 ──────────────────────────
//
// Bignum values at the ABI boundary are null-terminated decimal strings
// (pointer to i8). The runtime allocates result strings with `malloc`;
// callers are responsible for eventual cleanup (deferred to v0.2 arena).

use ynz_numerics::decimal_n::{
    add as bignum_add_op, div as bignum_div_op, format_bignum, mul as bignum_mul_op, parse_bignum,
    sub as bignum_sub_op,
};
use ynz_numerics::BigNum;

/// Parse a decimal string and compute `a + b` at `precision` digits.
/// Returns a newly-allocated C string. Caller owns the memory.
///
/// # Safety
/// `a` and `b` must be valid null-terminated UTF-8 strings.
#[no_mangle]
pub unsafe extern "C" fn ynz_bignum_add(a: *const i8, b: *const i8, precision: u16) -> *mut i8 {
    bignum_binop(a, b, precision, bignum_add_op)
}

/// Subtract: `a - b` at `precision` digits.
///
/// # Safety
/// Same as `ynz_bignum_add`.
#[no_mangle]
pub unsafe extern "C" fn ynz_bignum_sub(a: *const i8, b: *const i8, precision: u16) -> *mut i8 {
    bignum_binop(a, b, precision, bignum_sub_op)
}

/// Multiply: `a * b` at `precision` digits.
///
/// # Safety
/// Same as `ynz_bignum_add`.
#[no_mangle]
pub unsafe extern "C" fn ynz_bignum_mul(a: *const i8, b: *const i8, precision: u16) -> *mut i8 {
    bignum_binop(a, b, precision, bignum_mul_op)
}

/// Divide: `a / b` at `precision` digits.
///
/// # Safety
/// Same as `ynz_bignum_add`.
#[no_mangle]
pub unsafe extern "C" fn ynz_bignum_div(a: *const i8, b: *const i8, precision: u16) -> *mut i8 {
    bignum_binop(a, b, precision, bignum_div_op)
}

/// Helper: parse, apply op, format, return as heap-allocated C string.
///
/// # Safety
/// `a` and `b` must be valid null-terminated UTF-8 strings.
unsafe fn bignum_binop(
    a: *const i8,
    b: *const i8,
    precision: u16,
    op: fn(&BigNum, &BigNum) -> BigNum,
) -> *mut i8 {
    let a_str = std::ffi::CStr::from_ptr(a).to_str().unwrap_or("0");
    let b_str = std::ffi::CStr::from_ptr(b).to_str().unwrap_or("0");
    let an = parse_bignum(a_str, precision).unwrap_or_else(|| BigNum::zero(precision));
    let bn_val = parse_bignum(b_str, precision).unwrap_or_else(|| BigNum::zero(precision));
    let result = op(&an, &bn_val);
    let s = format_bignum(&result);
    let cstr = std::ffi::CString::new(s).unwrap_or_else(|_| {
        eprintln!(
            "INTERNAL ERROR: bignum formatting produced an unexpected NUL byte. \
                  This is a compiler bug — please file an issue at \
                  https://github.com/yinz-lang/yinz/issues with the source file attached."
        );
        std::process::abort();
    });
    cstr.into_raw()
}

// The CPU-parallel join shim tests exercise the three production C-ABI shims in isolation —
// without any Yinz codegen involved — pinning the poll-based join ABI contract so codegen can
// emit calls against it without re-litigating the runtime semantics.
//
// All tests run through a real Tokio runtime (not mocked) to confirm:
//   1. value roundtrip per return class — int, float (bit-cast), pointer, decimal128
//      ([lo, hi] with an explicit 16-byte-aligned i128 load), and the `T errors` pair —
//      the result bytes ynz_rt_join_poll writes match what fn_ptr returned, no truncation
//   2. saturation — i64::MIN/i64::MAX survive the [i64;2] ABI AND ≥600 concurrent joins
//      complete without deadlock (poll-based join never parks a thread)
//   3. drop-detach — ynz_rt_join_handle_free frees the handle Box (proven by a drop-probe
//      counter, not just a "ran" witness) while the detached child still completes
//   4. panic re-raise — a panicking child propagates via resume_unwind to the parent
//   5. pre-init discard — spawn before ynz_rt_init returns null (the documented discard signal)
//
// The Context<'_> is obtained from inside a real Future::poll via `with_cx` — never a
// fabricated waker (the M2-HALT corpse) — matching the runtime contract that waker_ctx
// points to a &mut Context<'_> from the enclosing state machine's poll.
#[cfg(test)]
mod m3d_join_shims {
    use super::*;
    use runtime::{ynz_rt_join_handle_free, ynz_rt_join_poll};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    // The canonical spike-frame layout constants the runtime reads on drop. Tests plant the
    // same discriminator + handle slots codegen emits, so they import the shared constants
    // rather than hardcoding 0x5350 / frame offsets at each spawn site.
    use ynz_abi::{
        FRAME_HEADER_SIZE, SPIKE_FRAME_DISCRIMINATOR_OFFSET, SPIKE_FRAME_TAG,
        SPIKE_HANDLE_BASE_OFFSET, SPIKE_HANDLE_SLOT_BYTES,
    };

    // A trivially-copyable int result: [42, 0]
    extern "C" fn returns_42(_ctx: *mut u8) -> YnzCpuResult {
        YnzCpuResult([42, 0])
    }

    // Returns saturated values to confirm [i64::MIN, i64::MAX] survives the ABI.
    extern "C" fn returns_saturated(_ctx: *mut u8) -> YnzCpuResult {
        YnzCpuResult([i64::MIN, i64::MAX])
    }

    // Spawn a blocking task that panics and return the heap-boxed CpuJoinHandle pointer.
    //
    // Bypasses ynz_rt_spawn_blocking_joinable to avoid the extern "C" fn pointer boundary,
    // which in Rust 1.71+ turns panics into aborts (RFC 2945, Rust tracking issue #96300).
    // The test exercises ynz_rt_join_poll's Ready(Err(panic)) branch directly — the
    // mechanism under test is the resume_unwind call in join_poll, not the fn_ptr boundary.
    fn spawn_panicking_handle() -> *mut u8 {
        use crate::runtime::CpuJoinHandle;
        let join_handle = tokio::task::spawn_blocking(|| -> YnzCpuResult {
            panic!("intentional panic from CPU child");
        });
        // Box the handle to match the heap layout produced by ynz_rt_spawn_blocking_joinable.
        // CpuJoinHandle::new is pub(crate) precisely to enable this test path; the inner
        // field is private so external callers cannot directly .abort() or .poll() the handle.
        let boxed = Box::new(CpuJoinHandle::new(join_handle));
        Box::into_raw(boxed) as *mut u8
    }

    // Helper: yields the inner poll result from a closure that receives a real Context<'_>.
    //
    // This is the only correct way to get a real Context<'_> — fabricating a waker would
    // violate the waker-forwarding invariant and is exactly what the M2-HALT corpse did.
    async fn with_cx<F, R>(f: F) -> R
    where
        F: FnOnce(&mut std::task::Context<'_>) -> R,
    {
        // A one-shot future that captures the context and calls f.
        // Unpin: Option<F> and PhantomData are both Unpin when F: Unpin (closure is Unpin).
        struct CaptureCtx<F, R>(Option<F>, std::marker::PhantomData<R>);
        impl<F: Unpin, R> Unpin for CaptureCtx<F, R> {}
        impl<F, R> std::future::Future for CaptureCtx<F, R>
        where
            F: FnOnce(&mut std::task::Context<'_>) -> R,
        {
            type Output = R;
            fn poll(
                self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<R> {
                // SAFETY: CaptureCtx is Unpin (declared above); get_unchecked_mut is safe here.
                let inner = unsafe { self.get_unchecked_mut() };
                let f = inner.0.take().expect("polled twice");
                std::task::Poll::Ready(f(cx))
            }
        }
        CaptureCtx::<F, R>(Some(f), std::marker::PhantomData).await
    }

    #[tokio::test]
    async fn value_roundtrip_int() {
        // WHY: guards that ynz_rt_join_poll writes the correct 16-byte result to the
        // result_out slot and returns 0 (Ready) when the child has finished.
        unsafe {
            let handle_ptr = ynz_rt_spawn_blocking_joinable(returns_42, std::ptr::null_mut(), 0);
            assert!(
                !handle_ptr.is_null(),
                "spawn must return a non-null handle in a live Tokio context"
            );

            // We may need to poll more than once if the blocking task hasn't finished yet.
            let mut result_slot = [0i64; 2];
            let result_ptr = result_slot.as_mut_ptr() as *mut u8;

            // Give the blocking thread a moment; then poll.
            // Using a small sleep ensures the spawned task has time to complete before we
            // poll — making the test deterministic without a busy loop.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let status = with_cx(|cx| {
                let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr)
            })
            .await;

            assert_eq!(
                status, 0,
                "poll must return 0 (Ready) after the child has completed"
            );
            assert_eq!(
                result_slot,
                [42, 0],
                "result must be [42, 0] for returns_42"
            );
        }
    }

    #[tokio::test]
    async fn value_roundtrip_saturation() {
        // WHY: decimal128 paths write [lo_word, hi_word]; this test uses [i64::MIN, i64::MAX]
        // to confirm neither word is truncated or sign-extended incorrectly through the ABI.
        unsafe {
            let handle_ptr =
                ynz_rt_spawn_blocking_joinable(returns_saturated, std::ptr::null_mut(), 0);
            assert!(!handle_ptr.is_null());

            let mut result_slot = [0i64; 2];
            let result_ptr = result_slot.as_mut_ptr() as *mut u8;

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let status = with_cx(|cx| {
                let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr)
            })
            .await;

            assert_eq!(status, 0, "must be Ready");
            assert_eq!(
                result_slot,
                [i64::MIN, i64::MAX],
                "saturated values must survive the ABI unchanged"
            );
        }
    }

    // float result: the f64 is bit-cast (NOT truncated) into the low i64 word.
    extern "C" fn returns_float(_ctx: *mut u8) -> YnzCpuResult {
        YnzCpuResult([(std::f64::consts::PI).to_bits() as i64, 0])
    }

    // heap-pointer result (string/array/map class): an arbitrary pointer-shaped bit pattern
    // in the low word. The runtime treats it as opaque bits — it does not dereference it.
    extern "C" fn returns_ptr_bits(_ctx: *mut u8) -> YnzCpuResult {
        YnzCpuResult([0x0000_7FFF_DEAD_C0DE_u64 as i64, 0])
    }

    // number/decimal128 result: a 128-bit value split little-endian into [lo_word, hi_word].
    // Uses a value with non-trivial bits in BOTH words so a lo/hi swap or a dropped high word
    // would fail the reconstruction assertion.
    const DECIMAL128_TEST_VALUE: u128 = 0x1234_5678_9ABC_DEF0_0FED_CBA9_8765_4321_u128;
    extern "C" fn returns_decimal128(_ctx: *mut u8) -> YnzCpuResult {
        let lo = DECIMAL128_TEST_VALUE as u64;
        let hi = (DECIMAL128_TEST_VALUE >> 64) as u64;
        YnzCpuResult([lo as i64, hi as i64])
    }

    // `T errors` (error-channel pair) result: [err_tag, ok_bits]. err_tag != 0 marks the
    // error variant; the ok word carries whatever the codegen packs alongside.
    extern "C" fn returns_error_pair(_ctx: *mut u8) -> YnzCpuResult {
        YnzCpuResult([7, 0])
    }

    // Poll a freshly spawned handle to Ready and return the 16 bytes written to result_out.
    // 16-byte aligned result slot via #[repr(align(16))] so the decimal128/i128 read path is
    // exercised under the alignment the frame result slot guarantees.
    #[repr(C, align(16))]
    struct AlignedResultSlot([i64; 2]);

    // Time: O(n) where n = number of Pending polls before the blocking task completes
    // (one yield-to-executor cycle per Pending). Space: O(1) — a single 16-byte result slot.
    async fn spawn_join_collect(
        fn_ptr: extern "C" fn(*mut u8) -> YnzCpuResult,
    ) -> AlignedResultSlot {
        unsafe {
            let handle_ptr = ynz_rt_spawn_blocking_joinable(fn_ptr, std::ptr::null_mut(), 0);
            assert!(!handle_ptr.is_null(), "spawn must return a non-null handle");

            let mut slot = AlignedResultSlot([0i64; 2]);
            let result_ptr = slot.0.as_mut_ptr() as *mut u8;

            // Poll until Ready — yield to the executor between Pending cycles so the blocking
            // pool can make progress without a busy loop.
            loop {
                let status = with_cx(|cx| {
                    let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                    ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr)
                })
                .await;
                if status == 0 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            slot
        }
    }

    #[tokio::test]
    async fn value_roundtrip_float() {
        // WHY: the float return class bit-casts an f64 into the low i64 word. A truncation
        // or an int-cast (instead of a bit-cast) would corrupt the value; reconstructing the
        // f64 from the round-tripped bits proves the ABI preserves the exact bit pattern.
        let slot = spawn_join_collect(returns_float).await;
        let recovered = f64::from_bits(slot.0[0] as u64);
        assert_eq!(
            recovered,
            std::f64::consts::PI,
            "float must survive the [i64;2] ABI as an exact bit-cast"
        );
        assert_eq!(slot.0[1], 0, "high word must be zero for the float class");
    }

    #[tokio::test]
    async fn value_roundtrip_ptr() {
        // WHY: string/array/map returns pack a heap pointer into the low word. The runtime
        // moves the bits opaquely; this guards that no pointer bits are clobbered in transit.
        let slot = spawn_join_collect(returns_ptr_bits).await;
        assert_eq!(
            slot.0[0] as u64, 0x0000_7FFF_DEAD_C0DE_u64,
            "pointer-shaped low word must round-trip unchanged"
        );
        assert_eq!(slot.0[1], 0, "high word must be zero for the pointer class");
    }

    #[tokio::test]
    async fn value_roundtrip_decimal128_aligned() {
        // WHY: number/decimal128 returns use BOTH i64 words ([lo, hi]). A decimal128 is a
        // 128-bit value; reading the round-tripped result slot AS an i128 is only correct
        // when the slot is 16-byte aligned — a misaligned i128 load is a SIGBUS / silent-wrong
        // class on strict-alignment targets (the failure mode the plan calls out explicitly).
        // This test (a) confirms both words survive without a lo/hi swap, and (b) asserts the
        // result slot is 16-byte aligned and reads back as the exact i128 via an aligned load.
        let slot = spawn_join_collect(returns_decimal128).await;

        // (a) Both words survive independently.
        let lo = slot.0[0] as u64;
        let hi = slot.0[1] as u64;
        let reconstructed = ((hi as u128) << 64) | (lo as u128);
        assert_eq!(
            reconstructed, DECIMAL128_TEST_VALUE,
            "decimal128 must round-trip through [lo, hi] without word swap or truncation"
        );

        // (b) Explicit 16-byte alignment assertion on the result slot, then an aligned i128
        // load. AlignedResultSlot is #[repr(C, align(16))] so the slot base is 16-aligned —
        // the same alignment codegen guarantees for the frame result slot. Reading it as a
        // *const u128 must not fault and must equal the expected value.
        let slot_addr = slot.0.as_ptr() as usize;
        assert_eq!(
            slot_addr % 16,
            0,
            "decimal128 result slot must be 16-byte aligned (misaligned i128 load is SIGBUS/silent-wrong class)"
        );
        // SAFETY: slot_addr is 16-byte aligned (asserted above) and points to 16 valid bytes.
        let as_i128 = unsafe { (slot.0.as_ptr() as *const u128).read() };
        assert_eq!(
            as_i128, DECIMAL128_TEST_VALUE,
            "aligned i128 load of the result slot must equal the original decimal128 value"
        );
    }

    #[tokio::test]
    async fn value_roundtrip_error_pair() {
        // WHY: `T errors` returns pack [err_tag, ok_bits]. Both words are load-bearing — the
        // tag selects the variant and must not be dropped or merged with the ok word.
        let slot = spawn_join_collect(returns_error_pair).await;
        assert_eq!(
            slot.0,
            [7, 0],
            "error-channel pair must round-trip both words ([err_tag, ok_bits])"
        );
    }

    #[tokio::test]
    async fn ctx_copy_contents_reach_child() {
        // WHY: guards the ctx-copy path — child must see the same bytes that were in
        // the parent's ctx at spawn time. Uses a 16-byte ctx with a recognizable pattern.
        extern "C" fn reads_ctx(ctx: *mut u8) -> YnzCpuResult {
            // Read two i64 from the ctx and return them as the result.
            // SAFETY: caller passed 16 valid bytes.
            let a = unsafe { (ctx as *const i64).read() };
            let b = unsafe { (ctx as *const i64).add(1).read() };
            YnzCpuResult([a, b])
        }

        let mut ctx_data = [0u8; 16];
        {
            let ptr = ctx_data.as_mut_ptr() as *mut i64;
            unsafe {
                ptr.write(0x1234_5678);
                ptr.add(1).write(0xDEAD_BEEF_i64);
            }
        }

        unsafe {
            let handle_ptr = ynz_rt_spawn_blocking_joinable(
                reads_ctx,
                ctx_data.as_mut_ptr(),
                ctx_data.len() as i64,
            );
            assert!(!handle_ptr.is_null());

            let mut result_slot = [0i64; 2];
            let result_ptr = result_slot.as_mut_ptr() as *mut u8;

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let status = with_cx(|cx| {
                let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr)
            })
            .await;

            assert_eq!(status, 0, "must be Ready");
            assert_eq!(
                result_slot,
                [0x1234_5678, 0xDEAD_BEEF_i64],
                "ctx bytes must be faithfully copied and visible to the child"
            );
        }
    }

    #[tokio::test]
    async fn drop_detach_no_uaf() {
        // WHY: guards ynz_rt_join_handle_free — dropping a handle mid-flight must not
        // UAF or double-free. An Arc<AtomicBool> witness lets us confirm the child ran to
        // completion AND that the ctx-copy heap buffer reached the child intact despite
        // the parent having freed the join handle.
        //
        // Allocation ledger (the detach path):
        //   ALLOC 1: Arc<AtomicBool> (parent holds ref; clone into ctx = ref 2)
        //   ALLOC 2: ctx heap copy (ynz_rt_spawn_blocking_joinable copies 16 bytes)
        //   ALLOC 3: Box<CpuJoinHandle> (returned as handle_ptr)
        //   FREE  3: ynz_rt_join_handle_free drops Box<CpuJoinHandle> (detaches task)
        //   FREE  2: FrameDropGuard inside child closure frees the ctx heap copy on return
        //   FREE  1: Arc dropped by child (sets flag), outer Arc dropped at test end
        // Expected: no UAF (FREE 2 happens inside the child, after the child runs; the
        // parent's ynz_rt_join_handle_free dropped only the Box — not the ctx copy).
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        // Capture the Arc pointer as a raw ptr in ctx (16 bytes: pointer + sentinel).
        // The child reads it out, runs to completion, and sets the flag.
        let arc_ptr = Arc::into_raw(completed_clone) as u64;
        let mut ctx_data = [0u8; 16];
        unsafe {
            let p = ctx_data.as_mut_ptr() as *mut u64;
            p.write(arc_ptr);
            p.add(1).write(0xCAFE_BABE);
        }

        extern "C" fn sets_flag_and_returns(ctx: *mut u8) -> YnzCpuResult {
            // Read the Arc pointer back and signal completion.
            // SAFETY: ctx is valid for 16 bytes (caller guarantee).
            let arc_ptr = unsafe { (ctx as *const u64).read() };
            let arc: Arc<AtomicBool> = unsafe { Arc::from_raw(arc_ptr as *const AtomicBool) };
            arc.store(true, Ordering::Release);
            // Arc drops here — release ref from the child.
            YnzCpuResult([0, 0])
        }

        unsafe {
            let handle_ptr = ynz_rt_spawn_blocking_joinable(
                sets_flag_and_returns,
                ctx_data.as_mut_ptr(),
                ctx_data.len() as i64,
            );
            assert!(!handle_ptr.is_null());

            // Drop the handle immediately — parent is "cancelled mid-join".
            ynz_rt_join_handle_free(handle_ptr);
        }

        // Give the blocking task time to run and set the flag.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(
            completed.load(Ordering::Acquire),
            "child must have run to completion even after parent dropped the handle"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn handle_free_detaches_and_frees_box() {
        // WHY: the drop-detach contract has two halves the plan requires PROVEN separately:
        //   (1) the child task runs to completion after the parent frees the handle (detach), and
        //   (2) the handle's heap Box is actually FREED, not leaked.
        // An Arc<bool> "ran" witness only proves (1). Half (2) — the FREE — is proven here with a
        // per-handle drop-probe counter: CpuJoinHandle::Drop increments the counter exactly when
        // THIS Box is dropped. A leak would leave the counter at 0; a detach-only path would still
        // run the child but never increment. The counter is Arc-local so concurrent drops from
        // other parallel tests can never bleed into this assertion window. This is the allocation
        // ledger the plan asks for: free-count == 1 for the one handle allocated.
        use crate::runtime::CpuJoinHandle;
        use std::sync::atomic::AtomicUsize;

        let completed = Arc::new(AtomicBool::new(false));
        let completed_child = completed.clone();

        // Box-free probe: incremented exactly once, when this CpuJoinHandle's Box is dropped.
        let free_count = Arc::new(AtomicUsize::new(0));

        // Build the handle the same way ynz_rt_spawn_blocking_joinable does (Box<CpuJoinHandle>),
        // but with a drop probe injected so the FREE is observable. Going through CpuJoinHandle::new
        // keeps the construction identical to the production path.
        let join_handle = tokio::task::spawn_blocking(move || -> YnzCpuResult {
            completed_child.store(true, Ordering::Release);
            YnzCpuResult([0, 0])
        });
        let mut cpu_handle = CpuJoinHandle::new(join_handle);
        cpu_handle.set_drop_probe(Arc::clone(&free_count));
        let handle_ptr = Box::into_raw(Box::new(cpu_handle)) as *mut u8;

        // Parent cancels mid-join: free the handle before collecting any result.
        unsafe {
            ynz_rt_join_handle_free(handle_ptr);
        }

        // (2) The Box was freed exactly once — proven by the drop-probe counter, not by "ran".
        assert_eq!(
            free_count.load(Ordering::SeqCst),
            1,
            "ynz_rt_join_handle_free must drop (free) the handle Box exactly once — \
             a leak leaves this at 0"
        );

        // (1) The detached child still runs to completion (its ctx copy outlives the handle free).
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(
            completed.load(Ordering::Acquire),
            "detached child must run to completion after the handle was freed"
        );

        // Idempotence half of the contract: freeing a null handle is a no-op (never aborts).
        unsafe {
            ynz_rt_join_handle_free(std::ptr::null_mut());
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    // Time: O(n) where n = number of Pending polls before the blocking task completes  Space: O(1) — single result slot.
    async fn joinable_spawn_frees_ctx_copy_exactly_once() {
        // WHY: Step 5's ctx-free accounting requires that the FrameDropGuard ctx COPY made by
        // the REAL ynz_rt_spawn_blocking_joinable is freed exactly once (no leak) when the spawn
        // closure completes. Constructing a CpuJoinHandle directly (as handle_free_detaches_and_
        // frees_box does) makes no ctx copy, so it cannot prove this half. This test drives the
        // real FFI entry point with a NON-NULL ctx so the ctx copy actually happens, then asserts
        // the ctx heap buffer's FREE — not merely that the child "ran" — via a per-spawn drop
        // probe on the ctx FrameDropGuard. The process-global ynz_alloc/ynz_free counters can't
        // see this free: the ctx copy uses the Rust allocator and the lib unit-test binary never
        // calls ynz_rt_init. A leak would leave the probe at 0; a double-free would abort.
        use crate::runtime::arm_ctx_free_probe;
        use std::sync::atomic::AtomicUsize;

        // The child reads its ctx copy to prove the bytes survived the copy, then returns them.
        extern "C" fn echo_ctx_first_word(ctx: *mut u8) -> YnzCpuResult {
            assert!(!ctx.is_null(), "ctx copy must be non-null in the child");
            // SAFETY: ynz_rt_spawn_blocking_joinable copied ctx_size (8) bytes into this buffer.
            let word = unsafe { (ctx as *const i64).read_unaligned() };
            YnzCpuResult([word, 0])
        }

        let ctx_free_count = Arc::new(AtomicUsize::new(0));

        let mut ctx_bytes: [u8; 8] = 0xABCD_i64.to_ne_bytes();
        let ctx_ptr = ctx_bytes.as_mut_ptr();

        // Arm the probe on THIS thread, then call the real FFI fn on THIS thread — the probe is
        // read at guard construction (caller's thread) and baked into the guard before it moves
        // into the blocking-pool closure.
        arm_ctx_free_probe(Arc::clone(&ctx_free_count));
        let handle_ptr = unsafe { ynz_rt_spawn_blocking_joinable(echo_ctx_first_word, ctx_ptr, 8) };
        assert!(!handle_ptr.is_null(), "spawn must return a non-null handle");

        // Poll to Ready so the child closure runs to completion and its ctx FrameDropGuard drops.
        let mut slot = [0i64; 2];
        let result_ptr = slot.as_mut_ptr() as *mut u8;
        loop {
            let status = with_cx(|cx| {
                let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                unsafe { ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr) }
            })
            .await;
            if status == 0 {
                break;
            }
            tokio::task::yield_now().await;
        }

        assert_eq!(slot[0], 0xABCD, "child must observe the copied ctx bytes");
        assert_eq!(
            ctx_free_count.load(Ordering::SeqCst),
            1,
            "the ctx heap copy made by ynz_rt_spawn_blocking_joinable must be freed exactly once \
             — a leak leaves this at 0"
        );
    }

    #[test]
    fn spawn_before_init_returns_null() {
        // WHY: outside any Tokio context AND with the global RUNTIME uninitialised,
        // ynz_rt_spawn_blocking_joinable must DISCARD (return null) with a warning rather than
        // panic or abort — mirroring ynz_rt_spawn's pre-init/post-shutdown discard contract.
        // The doc names null as the pre-init/post-shutdown signal; this pins that the early-return
        // ladder actually produces null in that state.
        //
        // Determinism note: this is a plain #[test] (NOT #[tokio::test]), so Handle::try_current()
        // returns Err. In the lib crate's unit-test binary no test calls ynz_rt_init (init/shutdown
        // live only in the separate tests/*.rs integration binaries), so the process-global RUNTIME
        // OnceLock is never populated here — RUNTIME.get() is None and the spawn takes the
        // "called before ynz_rt_init" discard branch deterministically.
        extern "C" fn never_runs(_ctx: *mut u8) -> YnzCpuResult {
            YnzCpuResult([1, 0])
        }
        let handle_ptr =
            unsafe { ynz_rt_spawn_blocking_joinable(never_runs, std::ptr::null_mut(), 0) };
        assert!(
            handle_ptr.is_null(),
            "spawn before ynz_rt_init (no Tokio context) must return a null handle (discard), \
             not a live handle"
        );
    }

    #[tokio::test]
    async fn detach_child_runs_after_handle_free() {
        // WHY: guards that a spawned child runs to completion even when the parent calls
        // ynz_rt_join_handle_free before the child has started (cancellation path). Without
        // this guarantee, a cancelled parent would leave the ctx copy dangling — the child
        // reads from it after the parent's FrameDropGuard frees it (UAF).
        //
        // Ledger approach: an AtomicUsize counter is stored in ctx (8 bytes). The child
        // reads the pointer out of ctx, increments the counter to signal "child ran",
        // then drops. After the test, the counter must be 1 (child ran exactly once)
        // and the test process must not have crashed (no UAF on the ctx copy).
        // Proves: the child ran exactly once AND reached the return point without a UAF
        // crash. Does not measure allocator malloc/free balance directly — the UAF-free
        // assertion is the proof of interest here.
        let ran_counter = Arc::new(AtomicUsize::new(0));
        let counter_ptr = Arc::as_ptr(&ran_counter) as u64;

        let mut ctx_data = [0u8; 8];
        unsafe {
            (ctx_data.as_mut_ptr() as *mut u64).write(counter_ptr);
        }

        extern "C" fn increment_counter(ctx: *mut u8) -> YnzCpuResult {
            // SAFETY: ctx holds an 8-byte counter pointer; the Arc is still alive
            // (the outer test holds a ref; this does not take ownership).
            let ptr = unsafe { (ctx as *const u64).read() } as *const AtomicUsize;
            unsafe { (*ptr).fetch_add(1, Ordering::Release) };
            YnzCpuResult([0, 0])
        }

        unsafe {
            let handle_ptr =
                ynz_rt_spawn_blocking_joinable(increment_counter, ctx_data.as_mut_ptr(), 8);
            assert!(!handle_ptr.is_null(), "spawn must return non-null handle");

            // Free the handle immediately (simulate parent cancellation mid-join).
            // The ctx COPY inside FrameDropGuard is still owned by the child closure.
            ynz_rt_join_handle_free(handle_ptr);
        }

        // Give the child time to run.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert_eq!(
            ran_counter.load(Ordering::Acquire),
            1,
            "child must have run exactly once — ctx copy was intact after handle free"
        );
        // If there were a UAF, the child's read of the counter pointer from ctx would
        // crash (SIGSEGV) rather than reaching this assertion. The absence of a crash
        // confirms the ctx copy is valid for the child's full duration.
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn panic_reraises_in_parent() {
        // WHY: a panicking child must propagate via resume_unwind so the parent's panic
        // handler fires — matching the observable behavior of sequential execution.
        // Without this, a panicking child would be silently swallowed (M3d regression class).
        //
        // The catch is done inside with_cx so the context is a real Tokio waker.
        // catch_unwind wraps the poll call; resume_unwind on the Ready(Err(panic)) path
        // re-raises the original panic payload rather than a JoinError wrapper.
        unsafe {
            // Use spawn_panicking_handle() to bypass the extern "C" fn boundary abort.
            // The handle_ptr points to a Box<CpuJoinHandle> wrapping a JoinHandle whose
            // blocking task panicked — exactly what ynz_rt_join_poll's Ready(Err) branch
            // is designed to handle.
            let handle_ptr = spawn_panicking_handle();
            assert!(!handle_ptr.is_null());

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let mut result_slot = [0i64; 2];
            let result_ptr = result_slot.as_mut_ptr() as *mut u8;

            // catch_unwind must be INSIDE block_in_place: resume_unwind panics while on the
            // blocking OS thread; if catch_unwind were outside, the panic would try to cross
            // the block_in_place boundary (which aborts per tokio::task::block_in_place docs).
            let caught = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                // catch_unwind inside block_in_place captures the resume_unwind before it crosses
                // the block_in_place OS-thread boundary.
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    rt.block_on(with_cx(|cx| {
                        let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                        ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr)
                    }))
                }))
            });

            assert!(
                caught.is_err(),
                "poll must panic (via resume_unwind) when the child panicked"
            );
            let payload = caught.unwrap_err();
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("<non-string>");
            assert!(
                msg.contains("intentional panic"),
                "panic message must be the child's original message, got: {msg}"
            );
        }
    }

    /// Spawn ≥600 concurrent joinable tasks through the real blocking pool and poll them
    /// all to completion, verifying:
    ///   1. No deadlock: poll-based join never holds a thread, so pool saturation (default
    ///      512 threads) only queues excess tasks — the joiner is suspended, not parked.
    ///   2. All results are correct: each task returns its own sequence number × 2 packed
    ///      into the low i64 word of YnzCpuResult.
    ///   3. The blocking pool runs through the real `ynz_rt_spawn_blocking_joinable` ABI —
    ///      this is the runtime-level proof that Step 3(d)'s "≥600 spawned joins" claim
    ///      holds. (The Yinz-source fixture (d) only exercises 2 joins per entrypoint call;
    ///      this test provides the saturation guarantee that fixture alone cannot.)
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn saturation_600_joins() {
        // WHY: validates that ynz_rt_spawn_blocking_joinable + ynz_rt_join_poll can drive
        // ≥600 concurrent blocking tasks through the Tokio blocking pool without deadlock.
        // Each join is poll-based (no thread parked), so pool exhaustion only queues tasks.
        const JOIN_COUNT: usize = 640; // well above the 512-thread default pool size

        extern "C" fn double_seq(ctx: *mut u8) -> YnzCpuResult {
            // Read the sequence number from ctx (8 bytes) and return seq × 2.
            let seq = unsafe { (ctx as *const i64).read() };
            YnzCpuResult([seq * 2, 0])
        }

        let mut handles: Vec<*mut u8> = Vec::with_capacity(JOIN_COUNT);
        let mut ctx_data: Vec<[u8; 8]> = Vec::with_capacity(JOIN_COUNT);

        // Spawn all tasks before polling any of them — maximises pool pressure.
        unsafe {
            for i in 0..JOIN_COUNT {
                let mut buf = [0u8; 8];
                (buf.as_mut_ptr() as *mut i64).write(i as i64);
                ctx_data.push(buf);
                let handle =
                    ynz_rt_spawn_blocking_joinable(double_seq, ctx_data[i].as_mut_ptr(), 8);
                assert!(
                    !handle.is_null(),
                    "spawn {i}: expected non-null handle from live Tokio context"
                );
                handles.push(handle);
            }
        }

        // Allow time for most tasks to complete before polling to confirm no deadlock.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let mut completed = 0usize;
        for (i, handle_ptr) in handles.iter().copied().enumerate() {
            let mut result_slot = [0i64; 2];
            let result_ptr = result_slot.as_mut_ptr() as *mut u8;

            // Poll until Ready — most tasks will already be done; a handful still running
            // may need one Pending cycle before the waker fires.
            loop {
                let status = with_cx(|cx| unsafe {
                    let waker_ctx = cx as *mut std::task::Context<'_> as *mut u8;
                    ynz_rt_join_poll(handle_ptr, waker_ctx, result_ptr)
                })
                .await;
                if status == 0 {
                    break;
                }
                // Still pending — yield to the executor so the blocking pool can make progress.
                tokio::task::yield_now().await;
            }

            let expected = (i as i64) * 2;
            assert_eq!(
                result_slot[0], expected,
                "task {i}: expected {expected}, got {}",
                result_slot[0]
            );
            completed += 1;
        }

        assert_eq!(
            completed, JOIN_COUNT,
            "all {JOIN_COUNT} joins must complete without deadlock"
        );
    }

    /// Verify the spike-frame discriminator drop path:
    ///   1. A frame buffer with the packed spike discriminator (tag + handle count 2) and a
    ///      live CpuJoinHandle in slot 0 → handle is freed (child detaches, witness completes).
    ///   2. The second handle slot is null → no spurious free attempted.
    ///   3. A zeroed frame (no tag) → discriminator branch skipped, no free attempted.
    ///
    /// Tests `cleanup_spike_cpu_handles` directly rather than constructing a full
    /// SpawnStateFnFuture (which requires a live resume-fn and frame allocation).
    /// The function is the extracted core of SpawnStateFnFuture::drop's step 1.5.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn discriminator_drop_frees_spike_handles() {
        // WHY: SpawnStateFnFuture::drop's discriminator branch (step 1.5) had zero test
        // coverage before this test. A bug there would silently leak CpuJoinHandle boxes
        // on every parent cancellation mid-join — one per live handle per cancel, unbounded
        // over program lifetime. The test proves both the "magic present, free handles" path
        // and the "no magic, skip" path fire correctly.
        use crate::runtime::{cleanup_spike_cpu_handles, CpuJoinHandle, SpawnStateFnFuture};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        // --- Case 1: spike frame with magic + live handle in slot 0, null in slot 1 ---
        //
        // Exercises the full SpawnStateFnFuture::drop → cleanup_spike_cpu_handles delegation
        // path. The frame is allocated via ynz_alloc_zeroed (matches the allocator drop uses)
        // and a live CpuJoinHandle is planted in slot 0. Dropping the future triggers the
        // SpawnStateFnFuture::drop path that calls cleanup_spike_cpu_handles. Commenting out
        // that delegation line causes the probe count to stay 0 and this test to FAIL.
        let completed = Arc::new(AtomicBool::new(false));
        let completed_child = completed.clone();

        // Per-handle drop probe: incremented exactly when THIS CpuJoinHandle is dropped.
        // Arc-local — concurrent drops from other tests never race into this assertion window.
        let drop_count = Arc::new(AtomicUsize::new(0));

        // Spawn a real blocking task so CpuJoinHandle wraps a live JoinHandle.
        let join_handle = tokio::task::spawn_blocking(move || -> YnzCpuResult {
            completed_child.store(true, Ordering::Release);
            YnzCpuResult([1, 0])
        });
        let mut cpu_handle = CpuJoinHandle::new(join_handle);
        cpu_handle.set_drop_probe(Arc::clone(&drop_count));
        let handle_ptr = Box::into_raw(Box::new(cpu_handle)) as *mut u8;

        // Allocate via ynz_alloc_zeroed — same allocator SpawnStateFnFuture::drop uses to free.
        let frame_ptr = unsafe { ynz_alloc_zeroed(80) };
        assert!(!frame_ptr.is_null(), "frame alloc must succeed");

        unsafe {
            // Write the packed discriminator at offset 4: tag in the high 16 bits,
            // handle count = 2 in the low 16 bits (two slots to inspect — slot 0 live,
            // slot 1 null).
            (frame_ptr.add(SPIKE_FRAME_DISCRIMINATOR_OFFSET) as *mut u32)
                .write((SPIKE_FRAME_TAG << 16) | 2);
            // Plant the live handle pointer at slot 0 (offset 32). Slot 1 stays null.
            (frame_ptr.add(SPIKE_HANDLE_BASE_OFFSET) as *mut *mut u8).write(handle_ptr);
        }

        // Build a minimal no-op resume function for SpawnStateFnFuture::new.
        unsafe extern "C-unwind" fn noop_resume_c1(_frame: *mut u8, _waker: *mut u8) -> i32 {
            1 // Pending — never polled, so never called.
        }

        // Drop the future immediately: SpawnStateFnFuture::drop calls cleanup_spike_cpu_handles
        // which frees slot 0 → probe increments. If the delegation line is commented out,
        // the probe stays 0 and the assertion below fails.
        {
            let _future = SpawnStateFnFuture::new(
                noop_resume_c1,
                frame_ptr,
                80,
                std::ptr::null_mut::<u8>(),
                std::ptr::null::<runtime::BgArgDropEntry>(),
                0,
            );
        }

        assert_eq!(
            drop_count.load(Ordering::SeqCst),
            1,
            "SpawnStateFnFuture::drop must free exactly 1 CpuJoinHandle (slot 0) via cleanup delegation"
        );

        // Give the child task time to complete after the handle is dropped (detaches).
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(
            completed.load(Ordering::Acquire),
            "child must have run to completion after handle was dropped (detached)"
        );

        // --- Case 2: zeroed frame (no magic) → discriminator branch skipped ---
        //
        // Write a live handle pointer into the "handle slot 0" position of a NON-spike frame.
        // cleanup_spike_cpu_handles must NOT free it (discriminator is zero, not the spike tag).
        // We verify this by placing a sentinel value there and confirming it is untouched after
        // the call (no Box reconstruction / no drop of a fake pointer).
        let completed2 = Arc::new(AtomicBool::new(false));
        let completed2_child = completed2.clone();
        let join_handle2 = tokio::task::spawn_blocking(move || -> YnzCpuResult {
            completed2_child.store(true, Ordering::Release);
            YnzCpuResult([2, 0])
        });
        let cpu_handle2 = Box::new(CpuJoinHandle::new(join_handle2));
        let handle_ptr2 = Box::into_raw(cpu_handle2) as *mut u8;

        let mut frame2 = vec![0u8; 80];
        unsafe {
            // No magic at offset 4 — frame looks like a normal (non-spike) SM frame.
            // Write the handle pointer at offset 32 to confirm it is NOT freed.
            (frame2.as_mut_ptr().add(SPIKE_HANDLE_BASE_OFFSET) as *mut *mut u8).write(handle_ptr2);

            // Call the helper — discriminator is zero, so the if-branch must be skipped.
            cleanup_spike_cpu_handles(frame2.as_mut_ptr());

            // The handle pointer at offset 32 must still be non-null and valid.
            let still_there = *(frame2.as_ptr().add(SPIKE_HANDLE_BASE_OFFSET) as *const *mut u8);
            assert_eq!(
                still_there, handle_ptr2,
                "non-spike frame: handle pointer must be untouched by cleanup"
            );

            // Manually free the handle so the test does not leak it.
            ynz_rt_join_handle_free(handle_ptr2);
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(
            completed2.load(Ordering::Acquire),
            "non-spike child must still complete after manual free"
        );
    }

    /// Verify the count-driven cleanup frees EVERY live handle of an N=2 group, not just
    /// slot 0. A two-member CPU group cancelled mid-join has both handle slots live; the
    /// discriminator's encoded count (2) must drive the scan over both, freeing each box.
    ///
    /// This is the N=2 cancellation path the layout-driven cleanup must cover: a fixed
    /// "slot 0 only" scan would leak slot 1's handle on every cancellation of a full group.
    /// alloc=free is proven by the two per-handle drop probes both reaching 1.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cleanup_frees_both_handles_of_full_n2_group() {
        use crate::runtime::{cleanup_spike_cpu_handles, CpuJoinHandle};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        // Two independent drop probes — one per handle. Both must reach 1 (each box freed once).
        let drop0 = Arc::new(AtomicUsize::new(0));
        let drop1 = Arc::new(AtomicUsize::new(0));

        let plant = |probe: &Arc<AtomicUsize>| -> *mut u8 {
            let jh = tokio::task::spawn_blocking(|| -> YnzCpuResult { YnzCpuResult([7, 0]) });
            let mut h = CpuJoinHandle::new(jh);
            h.set_drop_probe(Arc::clone(probe));
            Box::into_raw(Box::new(h)) as *mut u8
        };
        let handle0 = plant(&drop0);
        let handle1 = plant(&drop1);

        // Allocate via ynz_alloc_zeroed so the layout matches a real spike frame. Both handle
        // slots (base and base+stride) hold a live handle; the discriminator encodes count = 2.
        let slot0 = SPIKE_HANDLE_BASE_OFFSET;
        let slot1 = SPIKE_HANDLE_BASE_OFFSET + SPIKE_HANDLE_SLOT_BYTES;
        let frame_ptr = unsafe { ynz_alloc_zeroed(80) };
        assert!(!frame_ptr.is_null(), "frame alloc must succeed");
        unsafe {
            (frame_ptr.add(SPIKE_FRAME_DISCRIMINATOR_OFFSET) as *mut u32)
                .write((SPIKE_FRAME_TAG << 16) | 2);
            (frame_ptr.add(slot0) as *mut *mut u8).write(handle0);
            (frame_ptr.add(slot1) as *mut *mut u8).write(handle1);
            cleanup_spike_cpu_handles(frame_ptr);
            // Both slots must be nulled after free (double-free guard).
            assert!(
                (*(frame_ptr.add(slot0) as *const *mut u8)).is_null(),
                "slot 0 must be nulled after free"
            );
            assert!(
                (*(frame_ptr.add(slot1) as *const *mut u8)).is_null(),
                "slot 1 must be nulled after free"
            );
            ynz_free(frame_ptr, 80);
        }

        assert_eq!(
            drop0.load(Ordering::SeqCst),
            1,
            "handle 0 (slot 0) must be freed exactly once"
        );
        assert_eq!(
            drop1.load(Ordering::SeqCst),
            1,
            "handle 1 (slot 1) must be freed exactly once — count-driven scan covers it"
        );
    }

    /// Verify the cleanup is correct-by-construction for N>2: a discriminator count of 3
    /// drives the scan over three contiguous handle slots (offsets 32/40/48), freeing the
    /// live ones and skipping a null. A live N>2 group IS now reachable end-to-end since
    /// v0.3-M3g Phase 4's N+M matrix (e.g. `v0_3_m3g_matrix_3cpu_2io.ynz` hosts 3 CPU-class
    /// fused-group members) — this test predates that fixture and stays as the fast, isolated,
    /// runtime-crate-level pin of the SAME count-driven mechanism, independent of any specific
    /// codegen-emitted frame shape.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cleanup_is_layout_driven_for_n_greater_than_two() {
        use crate::runtime::{cleanup_spike_cpu_handles, CpuJoinHandle};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let drop0 = Arc::new(AtomicUsize::new(0));
        let drop2 = Arc::new(AtomicUsize::new(0));
        let plant = |probe: &Arc<AtomicUsize>| -> *mut u8 {
            let jh = tokio::task::spawn_blocking(|| -> YnzCpuResult { YnzCpuResult([3, 0]) });
            let mut h = CpuJoinHandle::new(jh);
            h.set_drop_probe(Arc::clone(probe));
            Box::into_raw(Box::new(h)) as *mut u8
        };
        let handle0 = plant(&drop0);
        let handle2 = plant(&drop2);

        // Frame large enough for three handle slots (base, base+stride, base+2*stride) + slack.
        // Slot 1 stays null (e.g. a member whose handle was already consumed by a Ready poll).
        let slot0 = SPIKE_HANDLE_BASE_OFFSET;
        let slot2 = SPIKE_HANDLE_BASE_OFFSET + 2 * SPIKE_HANDLE_SLOT_BYTES;
        let frame_ptr = unsafe { ynz_alloc_zeroed(96) };
        assert!(!frame_ptr.is_null(), "frame alloc must succeed");
        unsafe {
            (frame_ptr.add(SPIKE_FRAME_DISCRIMINATOR_OFFSET) as *mut u32)
                .write((SPIKE_FRAME_TAG << 16) | 3);
            (frame_ptr.add(slot0) as *mut *mut u8).write(handle0);
            // slot 1 (base + stride) null
            (frame_ptr.add(slot2) as *mut *mut u8).write(handle2);
            cleanup_spike_cpu_handles(frame_ptr);
            assert!((*(frame_ptr.add(slot0) as *const *mut u8)).is_null());
            assert!((*(frame_ptr.add(slot2) as *const *mut u8)).is_null());
            ynz_free(frame_ptr, 96);
        }

        assert_eq!(drop0.load(Ordering::SeqCst), 1, "slot 0 handle freed once");
        assert_eq!(
            drop2.load(Ordering::SeqCst),
            1,
            "slot 2 handle freed once — third slot reached by the count-driven scan"
        );
    }

    /// v0.3-M3g Phase 4 (Resource cleanup): a FUSED (dual-kind) frame's discriminator count is
    /// the CPU-class member count ONLY (`emit_fused_group_spawn_poll`'s doc comment — I/O
    /// sub-frames carry no separate handle and free with the parent frame, unmodified M3b
    /// behavior). This test proves the count-driven scan is correctly bounded: with 2 live CPU
    /// handles and an ADJACENT byte region standing in for an embedded I/O child sub-frame
    /// (populated with a non-null, non-handle sentinel pattern immediately after the handle
    /// region), `cleanup_spike_cpu_handles` frees exactly the 2 handle slots and leaves the
    /// "child sub-frame" bytes byte-for-byte untouched — proving the scan cannot mistake
    /// adjacent embedded-child bytes for a third handle slot, which would be a double-free /
    /// wild-pointer-drop the instant those bytes happened to look like a live pointer.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cleanup_on_dual_kind_frame_leaves_io_subframe_region_untouched() {
        use crate::runtime::{cleanup_spike_cpu_handles, CpuJoinHandle};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let drop0 = Arc::new(AtomicUsize::new(0));
        let drop1 = Arc::new(AtomicUsize::new(0));
        let plant = |probe: &Arc<AtomicUsize>| -> *mut u8 {
            let jh = tokio::task::spawn_blocking(|| -> YnzCpuResult { YnzCpuResult([9, 0]) });
            let mut h = CpuJoinHandle::new(jh);
            h.set_drop_probe(Arc::clone(probe));
            Box::into_raw(Box::new(h)) as *mut u8
        };
        let handle0 = plant(&drop0);
        let handle1 = plant(&drop1);

        // 2 CPU handle slots (32..48), then an "embedded I/O child sub-frame" region (48..80)
        // sized like a real M3b child (FRAME_HEADER_SIZE bytes — single-homed in `ynz_abi`, never
        // hardcoded here) — filled with a sentinel byte pattern that is emphatically non-null
        // when read as a pointer (0xAB repeating), so an out-of-bounds scan would try to drop a
        // wild `*mut CpuJoinHandle` and crash/corrupt rather than silently succeed.
        let slot0 = SPIKE_HANDLE_BASE_OFFSET;
        let slot1 = SPIKE_HANDLE_BASE_OFFSET + SPIKE_HANDLE_SLOT_BYTES;
        let io_subframe_start = SPIKE_HANDLE_BASE_OFFSET + 2 * SPIKE_HANDLE_SLOT_BYTES;
        let io_subframe_len = FRAME_HEADER_SIZE as usize;
        let total_len = io_subframe_start + io_subframe_len;
        let frame_ptr = unsafe { ynz_alloc_zeroed(total_len) };
        assert!(!frame_ptr.is_null(), "frame alloc must succeed");
        unsafe {
            (frame_ptr.add(SPIKE_FRAME_DISCRIMINATOR_OFFSET) as *mut u32)
                .write((SPIKE_FRAME_TAG << 16) | 2);
            (frame_ptr.add(slot0) as *mut *mut u8).write(handle0);
            (frame_ptr.add(slot1) as *mut *mut u8).write(handle1);
            std::ptr::write_bytes(frame_ptr.add(io_subframe_start), 0xAB, io_subframe_len);

            cleanup_spike_cpu_handles(frame_ptr);

            assert!(
                (*(frame_ptr.add(slot0) as *const *mut u8)).is_null(),
                "CPU handle slot 0 must be freed and nulled"
            );
            assert!(
                (*(frame_ptr.add(slot1) as *const *mut u8)).is_null(),
                "CPU handle slot 1 must be freed and nulled"
            );
            let io_bytes =
                std::slice::from_raw_parts(frame_ptr.add(io_subframe_start), io_subframe_len);
            assert!(
                io_bytes.iter().all(|&b| b == 0xAB),
                "the embedded I/O child sub-frame region must be byte-for-byte untouched by the \
                 count-driven CPU-handle scan (count=2 must not read past the 2nd slot)"
            );
            ynz_free(frame_ptr, total_len);
        }

        assert_eq!(
            drop0.load(Ordering::SeqCst),
            1,
            "handle 0 freed exactly once"
        );
        assert_eq!(
            drop1.load(Ordering::SeqCst),
            1,
            "handle 1 freed exactly once"
        );
    }

    /// Verifies that dropping a `SpawnStateFnFuture` before the first poll (i.e., cancellation
    /// of a task that was never polled) correctly frees CPU join handles through the
    /// `cleanup_spike_cpu_handles` delegation in `SpawnStateFnFuture::drop`.
    ///
    /// This is the end-to-end path: task cancelled before first poll → Drop::drop →
    /// cleanup_spike_cpu_handles via the discriminator branch.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn spawn_state_fn_future_drop_before_first_poll_frees_handles() {
        // WHY: SpawnStateFnFuture::drop's step 1.5 (cleanup_spike_cpu_handles delegation)
        // is only useful if it fires on every cancellation path, including drop-before-poll.
        // This test creates a SpawnStateFnFuture with a spike-magic frame + live handle,
        // drops it immediately without polling, and verifies the handle's task completes
        // AND the CpuJoinHandle Drop counter increments (proving the Box was freed, not leaked —
        // a detach-only path would satisfy "task ran" without incrementing the counter).
        use crate::runtime::{CpuJoinHandle, SpawnStateFnFuture};
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        use std::sync::Arc;

        let completed = Arc::new(AtomicBool::new(false));
        let completed_child = completed.clone();

        // Per-handle drop probe: incremented exactly when THIS CpuJoinHandle is dropped.
        // Arc-local so concurrent drops in saturation_600_joins etc. can't race this window.
        let drop_count = Arc::new(AtomicUsize::new(0));

        // Spawn a real blocking task to create a live CpuJoinHandle.
        let join_handle = tokio::task::spawn_blocking(move || -> YnzCpuResult {
            completed_child.store(true, Ordering::Release);
            YnzCpuResult([42, 0])
        });
        let mut cpu_handle = CpuJoinHandle::new(join_handle);
        cpu_handle.set_drop_probe(Arc::clone(&drop_count));
        let handle_ptr = Box::into_raw(Box::new(cpu_handle)) as *mut u8;

        // Allocate via the same allocator SpawnStateFnFuture::drop uses to free — ynz_alloc_zeroed.
        // Using std::alloc here would be a mismatched-allocator crash when Drop calls ynz_free.
        let frame_ptr = unsafe { ynz_alloc_zeroed(80) };
        assert!(!frame_ptr.is_null(), "frame alloc must succeed");

        unsafe {
            // Write the packed discriminator at offset 4: tag in the high 16 bits,
            // handle count = 2 in the low 16 bits (slot 0 live, slot 1 null).
            (frame_ptr.add(SPIKE_FRAME_DISCRIMINATOR_OFFSET) as *mut u32)
                .write((SPIKE_FRAME_TAG << 16) | 2);
            // Plant the live handle pointer at slot 0 (offset 32).
            (frame_ptr.add(SPIKE_HANDLE_BASE_OFFSET) as *mut *mut u8).write(handle_ptr);
            // Slot 1 (offset 40) stays null.
        }

        // Build a minimal no-op resume function (never called, but SpawnStateFnFuture
        // requires a valid function pointer in the slot).
        unsafe extern "C-unwind" fn noop_resume(_frame: *mut u8, _waker: *mut u8) -> i32 {
            1 // Pending — would keep the task alive if polled; but we won't poll it.
        }

        // Create the future but NEVER poll it — drop fires cleanup immediately.
        {
            let _future = SpawnStateFnFuture::new(
                noop_resume,
                frame_ptr,
                80,
                std::ptr::null_mut::<u8>(), // rec_slot — none (-1 path in new())
                std::ptr::null::<runtime::BgArgDropEntry>(), // arg_drops — none
                0,                          // arg_drop_count
            );
            // Drop fires here: step 1.5 calls cleanup_spike_cpu_handles → frees handle at slot 0.
        }

        // drop_count is incremented exactly when the probe'd CpuJoinHandle is dropped.
        // If cleanup_spike_cpu_handles was skipped (commented out), the box leaks and
        // the drop impl never fires — count stays 0 and this assertion fails.
        assert_eq!(
            drop_count.load(Ordering::SeqCst),
            1,
            "SpawnStateFnFuture::drop must free exactly 1 CpuJoinHandle (slot 0) before-first-poll"
        );

        // After drop, frame_ptr was freed by SpawnStateFnFuture::drop (step 5). We must not
        // access frame_ptr here. Give the child task time to complete after detach.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(
            completed.load(Ordering::Acquire),
            "blocking task must complete after handle was freed on drop-before-poll"
        );
    }
}
