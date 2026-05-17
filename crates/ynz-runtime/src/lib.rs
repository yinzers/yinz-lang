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
        // 48 bytes: 34 digits + sign + decimal point + exponent + null = comfortably enough
        static BUF: std::cell::RefCell<[u8; 48]> = const { std::cell::RefCell::new([0u8; 48]) };
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
/// e.g. `"int overflow in '+'"`.
///
/// # Safety
/// `op_name` must be a valid, null-terminated C string or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_panic_overflow(op_name: *const u8) -> ! {
    let msg = cstr_to_str(op_name);
    // Write the diagnostic to stderr before aborting.
    // The WHAT/WHAT-INSTEAD/WHY three-part format is embedded here; it cannot
    // go through ariadne because the runtime has no source map at abort time.
    eprintln!(
        "RUNTIME ERROR: {msg}\n\n  \
         The value wrapped past the maximum (or minimum) for this type.\n\n  \
         Use .wrappingAdd() if wrap-around is intentional (available in M4).\n\n  \
         Why: Yinz panics on integer overflow by default to prevent silent data corruption."
    );
    std::process::abort();
}

/// Called by compiled code on division by zero.
///
/// # Safety
/// `op_name` must be a valid, null-terminated C string or null.
#[no_mangle]
pub unsafe extern "C" fn ynz_panic_div_by_zero(op_name: *const u8) -> ! {
    let msg = cstr_to_str(op_name);
    eprintln!(
        "RUNTIME ERROR: {msg}\n\n  \
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
        // 64-bit int: max 20 digits + sign + null = 22 bytes
        static BUF: std::cell::RefCell<[u8; 22]> = const { std::cell::RefCell::new([0u8; 22]) };
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


/// Compare two null-terminated UTF-8 strings for byte equality.
///
/// Returns 1 if identical, 0 otherwise. Used by codegen for multi-case `if`
/// on string scrutinees.
///
/// REPLACE-AT M7: swap for Unicode canonical equivalence — M3 programs do not
/// produce NFD strings, so byte-equality is correct for all current programs.
///
/// # Safety
///
/// Both `a` and `b` must be valid pointers to null-terminated C strings.
/// Dereferencing either before the null byte is undefined behavior if the
/// pointer is invalid or not null-terminated.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_eq(a: *const u8, b: *const u8) -> i32 {
    // SAFETY: caller guarantees both pointers are valid null-terminated C strings.
    let mut i = 0;
    loop {
        let ca = *a.add(i);
        let cb = *b.add(i);
        if ca != cb { return 0; }
        if ca == 0 { return 1; }
        i += 1;
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

/// Allocate `size` bytes. Aborts on OOM — Yinz programs cannot recover from OOM.
///
/// # Safety
///
/// The returned pointer is valid for `size` bytes and properly aligned.
/// The caller must free it with `ynz_free` using the same `size`.
#[no_mangle]
pub unsafe extern "C" fn ynz_alloc(size: usize) -> *mut u8 {
    let ptr = malloc(size) as *mut u8;
    if ptr.is_null() {
        std::process::abort();
    }
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
    free(ptr as *mut core::ffi::c_void);
}


// ── SipHash-2-4 (M5 P4b) ─────────────────────────────────────────────────────
//
// Reference: https://131002.net/siphash/siphash.pdf
// SipHash-2-4: 2 compression rounds, 4 finalization rounds.
// Per-process key is initialized from OS entropy on first call.

use std::sync::OnceLock;

static SIPHASH_KEY: OnceLock<[u8; 16]> = OnceLock::new();

/// Initialize the per-process SipHash key from OS entropy.
/// Must be called before any map operation. Idempotent.
#[no_mangle]
pub extern "C" fn ynz_siphash_init() {
    SIPHASH_KEY.get_or_init(|| {
        let mut key = [0u8; 16];
        #[cfg(target_os = "linux")]
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
    let k0 = u64::from_le_bytes(k[0..8].try_into().unwrap());
    let k1 = u64::from_le_bytes(k[8..16].try_into().unwrap());
    (k0, k1)
}

macro_rules! sipround {
    ($v0:expr, $v1:expr, $v2:expr, $v3:expr) => {
        $v0 = $v0.wrapping_add($v1); $v1 = $v1.rotate_left(13); $v1 ^= $v0;
        $v0 = $v0.rotate_left(32);
        $v2 = $v2.wrapping_add($v3); $v3 = $v3.rotate_left(16); $v3 ^= $v2;
        $v0 = $v0.wrapping_add($v3); $v3 = $v3.rotate_left(21); $v3 ^= $v0;
        $v2 = $v2.wrapping_add($v1); $v1 = $v1.rotate_left(17); $v1 ^= $v2;
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
        let m = u64::from_le_bytes(data[i*8..i*8+8].try_into().unwrap());
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
/// `ptr` must be a valid pointer to a null-terminated byte string.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ynz_siphash_str(ptr: *const u8) -> u64 {
    let mut len = 0;
    while *ptr.add(len) != 0 { len += 1; }
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
    let ctrl = malloc(capacity as usize) as *mut u8;
    let keys = malloc((capacity as usize) * 8) as *mut i64;
    let vals = malloc((capacity as usize) * 8) as *mut i64;
    let order_cap: i64 = 64;
    let order = malloc((order_cap as usize) * 8) as *mut i64;
    std::ptr::write_bytes(ctrl, CTRL_EMPTY, capacity as usize);
    *hdr = YnzMap { ctrl, keys, vals, insert_order: order, count: 0, capacity, order_cap };
    hdr
}

/// Allocate a new empty map with initial capacity 16.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ynz_map_new() -> *mut YnzMap {
    map_alloc(16)
}

unsafe fn find_slot(map: *const YnzMap, hash: u64, key: i64) -> Option<usize> {
    let cap = (*map).capacity as usize;
    let h2 = (hash & 0x7f) as u8;
    let start = (hash >> 7) as usize & (cap - 1);
    let mut idx = start;
    loop {
        let ctrl = *(*map).ctrl.add(idx);
        if ctrl == CTRL_EMPTY { return None; }
        if ctrl == h2 && *(*map).keys.add(idx) == key { return Some(idx); }
        idx = (idx + 1) & (cap - 1);
        if idx == start { return None; }
    }
}

unsafe fn find_insert_slot(map: *const YnzMap, hash: u64) -> usize {
    let cap = (*map).capacity as usize;
    let mut idx = (hash >> 7) as usize & (cap - 1);
    loop {
        let ctrl = *(*map).ctrl.add(idx);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED { return idx; }
        idx = (idx + 1) & (cap - 1);
    }
}

unsafe fn map_grow_int(map: *mut YnzMap) {
    let old_cap = (*map).capacity;
    let new_cap = old_cap * 2;
    let new_ctrl = malloc(new_cap as usize) as *mut u8;
    let new_keys = malloc((new_cap as usize) * 8) as *mut i64;
    let new_vals = malloc((new_cap as usize) * 8) as *mut i64;
    std::ptr::write_bytes(new_ctrl, CTRL_EMPTY, new_cap as usize);

    for i in 0..old_cap as usize {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED { continue; }
        let k = *(*map).keys.add(i);
        let v = *(*map).vals.add(i);
        let hash = ynz_siphash_i64(k);
        let h2 = (hash & 0x7f) as u8;
        let mut idx = (hash >> 7) as usize & (new_cap as usize - 1);
        while *new_ctrl.add(idx) != CTRL_EMPTY { idx = (idx + 1) & (new_cap as usize - 1); }
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

unsafe fn map_grow_str(map: *mut YnzMap) {
    let old_cap = (*map).capacity;
    let new_cap = old_cap * 2;
    let new_ctrl = malloc(new_cap as usize) as *mut u8;
    let new_keys = malloc((new_cap as usize) * 8) as *mut i64;
    let new_vals = malloc((new_cap as usize) * 8) as *mut i64;
    std::ptr::write_bytes(new_ctrl, CTRL_EMPTY, new_cap as usize);

    for i in 0..old_cap as usize {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED { continue; }
        let k = *(*map).keys.add(i);
        let v = *(*map).vals.add(i);
        let hash = ynz_siphash_str(k as *const u8);
        let h2 = (hash & 0x7f) as u8;
        let mut idx = (hash >> 7) as usize & (new_cap as usize - 1);
        while *new_ctrl.add(idx) != CTRL_EMPTY { idx = (idx + 1) & (new_cap as usize - 1); }
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
        let new_order = realloc((*map).insert_order as *mut core::ffi::c_void, (new_cap as usize) * 8) as *mut i64;
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
        if ca != cb { return false; }
        if ca == 0 { return true; }
        i += 1;
    }
}

/// Get a value by i64 key. Writes `[has_value, value]` into `out`.
///
/// # Safety
/// `map` and `out` must be valid non-null pointers.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
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
/// `map`, `key`, and `out` must be valid non-null pointers.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ynz_map_get_str(map: *const YnzMap, key: *const u8, out: *mut [i64; 2]) {
    let cap = (*map).capacity as usize;
    for i in 0..cap {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED { continue; }
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
/// `map` must be a valid non-null pointer returned by `ynz_map_new`.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
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
/// `map` and `key` must be valid non-null pointers.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ynz_map_set_str(map: *mut YnzMap, key: *const u8, value: i64) {
    if (*map).count * 4 >= (*map).capacity * 3 {
        map_grow_str(map);
    }
    let cap = (*map).capacity as usize;
    for i in 0..cap {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED { continue; }
        let stored = *(*map).keys.add(i) as *const u8;
        if cstr_eq_raw(stored, key) {
            *(*map).vals.add(i) = value;
            return;
        }
    }
    let hash = ynz_siphash_str(key);
    let h2 = (hash & 0x7f) as u8;
    let mut idx = (hash >> 7) as usize & (cap - 1);
    while *(*map).ctrl.add(idx) != CTRL_EMPTY && *(*map).ctrl.add(idx) != CTRL_DELETED {
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
/// `map` must be a valid non-null pointer.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ynz_map_count(map: *const YnzMap) -> i64 {
    (*map).count
}

/// Check if an i64 key exists. Returns 1 if found, 0 otherwise.
///
/// # Safety
/// `map` must be a valid non-null pointer.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
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
/// `map` and `out` must be valid non-null pointers.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
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
/// `map` and `out` must be valid non-null pointers.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ynz_map_iter_get_str(map: *const YnzMap, pos: i64, out: *mut [i64; 3]) {
    if pos < 0 || pos >= (*map).count {
        *out = [0, 0, 0];
        return;
    }
    let key_ptr = *(*map).insert_order.add(pos as usize);
    let cap = (*map).capacity as usize;
    for i in 0..cap {
        let ctrl = *(*map).ctrl.add(i);
        if ctrl == CTRL_EMPTY || ctrl == CTRL_DELETED { continue; }
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
/// `map` must be a valid non-null pointer returned by `ynz_map_new` and not yet freed.
#[no_mangle]
#[allow(clippy::missing_safety_doc)]
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

/// Allocate a new empty array with an initial capacity of 8 elements.
///
/// # Safety
/// Returns a heap pointer. Caller must free with `ynz_array_drop`.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_new() -> *mut YnzArray {
    let cap: i64 = 8;
    let data = malloc((cap as usize) * 8) as *mut u8;
    let hdr = malloc(std::mem::size_of::<YnzArray>()) as *mut YnzArray;
    (*hdr) = YnzArray { data, len: 0, cap };
    hdr
}

/// Push an i64-sized element (int, float bits, bool, or pointer cast to i64).
///
/// Doubles the capacity when full (amortized O(1) push).
///
/// # Safety
/// `arr` must be a valid pointer returned by `ynz_array_new`.
#[no_mangle]
pub unsafe extern "C" fn ynz_array_push(arr: *mut YnzArray, value: i64) {
    if (*arr).len == (*arr).cap {
        let new_cap = (*arr).cap * 2;
        let new_data = realloc((*arr).data as *mut core::ffi::c_void, (new_cap as usize) * 8) as *mut u8;
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
