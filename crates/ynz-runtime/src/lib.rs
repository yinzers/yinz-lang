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
/// NFC normalization for Unicode canonical equivalence is targeted for M7 P4b
/// (string codegen). Current programs only produce NFC strings from source
/// literals, so byte-equality is correct. P4b will swap this for NFC comparison.
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
    let raw: [u8; 16] = std::slice::from_raw_parts(num_ptr, 16).try_into().unwrap_or([0u8; 16]);
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
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_int(ptr: *const u8, len: i64, out: *mut i64) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let result = parse_string_to_int(bytes);
    match result {
        Some(n) => { *out = 1; *out.add(1) = n; }
        None    => { *out = 0; *out.add(1) = 0; }
    }
}

/// ABI: `(ptr: *const u8, len: i64, out: *mut [i64; 2]) -> void`
/// out[0] = has_value, out[1] = f64 bits (bit-cast from the double).
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_float(ptr: *const u8, len: i64, out: *mut i64) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let result = parse_string_to_float(bytes);
    match result {
        Some(f) => { *out = 1; *out.add(1) = f64::to_bits(f) as i64; }
        None    => { *out = 0; *out.add(1) = 0; }
    }
}

/// ABI: `(ptr: *const u8, len: i64, out: *mut [i64; 3]) -> void`
/// out[0] = has_value, out[1..3] = 16-byte decimal128 on success.
#[no_mangle]
pub unsafe extern "C" fn ynz_string_to_number(ptr: *const u8, len: i64, out: *mut i64) {
    let bytes = std::slice::from_raw_parts(ptr, len as usize);
    let trimmed = trim_ascii_ws(bytes);
    // Use ynz-numerics parse (decimal128 string parser).
    // parse() is infallible but returns 0 on bad input; we pre-validate here.
    match std::str::from_utf8(trimmed) {
        Err(_) => { *out = 0; }
        Ok(s) => {
            if is_valid_number_str(s.as_bytes()) {
                match parse(s) {
                    Some(val) => {
                        let bytes_le = val.to_ne_bytes();
                        *out = 1;
                        let data = out.add(1) as *mut u8;
                        std::ptr::copy_nonoverlapping(bytes_le.as_ptr(), data, 16);
                    }
                    None => { *out = 0; }
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
#[no_mangle]
pub unsafe extern "C" fn ynz_string_from_static(ptr: *const u8, len: i64) -> *const u8 {
    let size = len as usize + 1; // +1 for null terminator
    let buf = malloc(size) as *mut u8;
    if buf.is_null() { return b"\0".as_ptr(); }
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
    if x.is_nan() { return None; }
    // i64::MAX (2^63-1) is not exactly representable in f64; nearest is 2^63.
    // Upper check: x must be < 2^63 (strictly less, i.e. fits after truncation).
    const I64_MAX_F64: f64 = 9.223372036854776e18_f64; // 2^63
    const I64_MIN_F64: f64 = -9.223372036854776e18_f64; // -2^63
    if x >= I64_MAX_F64 || x < I64_MIN_F64 { return None; }
    Some(x as i64) // truncate toward zero; in-range proven above
}

// ── Private helpers ──────────────────────────────────────────────────────────

fn trim_ascii_ws(bytes: &[u8]) -> &[u8] {
    let is_ws = |b: &u8| matches!(b, 0x20 | 0x09 | 0x0A | 0x0D);
    let start = bytes.iter().position(|b| !is_ws(b)).unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|b| !is_ws(b)).map(|i| i + 1).unwrap_or(0);
    if start >= end { &[] } else { &bytes[start..end] }
}

fn parse_sign(bytes: &[u8]) -> (bool, &[u8]) {
    match bytes.first() {
        Some(b'+') => (false, &bytes[1..]),
        Some(b'-') => (true,  &bytes[1..]),
        _          => (false, bytes),
    }
}

fn parse_string_to_int(bytes: &[u8]) -> Option<i64> {
    let trimmed = trim_ascii_ws(bytes);
    let (neg, digits) = parse_sign(trimmed);
    if digits.is_empty() { return None; }
    if !digits.iter().all(|b| b.is_ascii_digit()) { return None; }
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
    if digits_only.is_empty() { return None; }
    if !is_valid_float_digits(digits_only) { return None; }
    let s = std::str::from_utf8(trimmed).ok()?;
    let f: f64 = s.parse().ok()?; // Rust parser handles the sign
    if f.is_infinite() { return None; }
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
    while i < n && bytes[i].is_ascii_digit() { i += 1; }
    if i == 0 { return false; }
    // Decimal part
    if i < n && bytes[i] == b'.' {
        i += 1;
        while i < n && bytes[i].is_ascii_digit() { i += 1; }
    }
    // Exponent
    if i < n && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < n && (bytes[i] == b'+' || bytes[i] == b'-') { i += 1; }
        let start = i;
        while i < n && bytes[i].is_ascii_digit() { i += 1; }
        if i == start { return false; } // exponent with no digits
    }
    i == n
}

fn is_valid_number_str(bytes: &[u8]) -> bool {
    // Same rules as float — decimal128 parser uses same format
    is_valid_float_digits(bytes)
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
    if err.is_null() { std::process::abort(); }

    // Snapshot the frame stack.
    let (trace_ptr, trace_len) = FRAME_STACK.with(|stack| {
        let s = stack.borrow();
        let len = s.len();
        if len == 0 {
            return (std::ptr::null_mut::<YnzFrame>(), 0i64);
        }
        let frames_mem = malloc(len * std::mem::size_of::<YnzFrame>()) as *mut YnzFrame;
        if frames_mem.is_null() { std::process::abort(); }
        for (i, &(file, line, function)) in s.iter().enumerate() {
            *frames_mem.add(i) = YnzFrame { file, line, function };
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
    if err.is_null() { return; }
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
    if idx < 0 || idx >= (*err).trace_len { return std::ptr::null(); }
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
            let fn_name = if (*frame).function.is_null() { "<unknown>" }
                          else { cstr_to_str((*frame).function) };
            let file = if (*frame).file.is_null() { "<unknown>" }
                       else { cstr_to_str((*frame).file) };
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
    fn int_basic() { assert_eq!(parse_string_to_int(b"42"), Some(42)); }
    #[test]
    fn int_whitespace_sign() { assert_eq!(parse_string_to_int(b"  +42  "), Some(42)); }
    #[test]
    fn int_negative() { assert_eq!(parse_string_to_int(b"-42"), Some(-42)); }
    #[test]
    fn int_empty() { assert_eq!(parse_string_to_int(b""), None); }
    #[test]
    fn int_whitespace_only() { assert_eq!(parse_string_to_int(b"  "), None); }
    #[test]
    fn int_lone_sign() { assert_eq!(parse_string_to_int(b"+"), None); }
    #[test]
    fn int_hex_prefix() { assert_eq!(parse_string_to_int(b"0x1A"), None); }
    #[test]
    fn int_trailing_chars() { assert_eq!(parse_string_to_int(b"42 hello"), None); }
    #[test]
    fn int_fractional() { assert_eq!(parse_string_to_int(b"42.5"), None); }
    #[test]
    fn int_scientific() { assert_eq!(parse_string_to_int(b"1e3"), None); }
    #[test]
    fn int_tab_lf() { assert_eq!(parse_string_to_int(b"\t42\n"), Some(42)); }
    #[test]
    fn int_non_breaking_space() { assert_eq!(parse_string_to_int(b"\xC2\xA042"), None); } // UTF-8 U+00A0
    #[test]
    fn float_basic() { assert_eq!(parse_string_to_float(b"1.5"), Some(1.5)); }
    #[test]
    fn float_scientific() { assert_eq!(parse_string_to_float(b"1.5e2"), Some(150.0)); }
    #[test]
    fn float_negative() { assert_eq!(parse_string_to_float(b"  -1.5  "), Some(-1.5)); }
    #[test]
    fn float_bad() { assert_eq!(parse_string_to_float(b"abc"), None); }
    #[test]
    fn float_double_dot() { assert_eq!(parse_string_to_float(b"1.5.5"), None); }
    #[test]
    fn float_to_int_nan() { assert_eq!(float_to_int_ref(f64::NAN), None); }
    #[test]
    fn float_to_int_inf() { assert_eq!(float_to_int_ref(f64::INFINITY), None); }
    #[test]
    fn float_to_int_oor() { assert_eq!(float_to_int_ref(1e30), None); }
    #[test]
    fn float_to_int_truncate() { assert_eq!(float_to_int_ref(2.5), Some(2)); }
    #[test]
    fn float_to_int_neg_truncate() { assert_eq!(float_to_int_ref(-2.5), Some(-2)); }
    #[test]
    fn float_to_int_boundary_upper() { assert_eq!(float_to_int_ref(9.223372036854776e18), None); }
    #[test]
    fn float_to_int_boundary_lower() { assert_eq!(float_to_int_ref(-9.223372036854776e18), Some(i64::MIN)); }
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

            ynz_frame_push(b"test.ynz\0".as_ptr(), 10, b"myFn\0".as_ptr());
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
            ynz_frame_push(b"a.ynz\0".as_ptr(), 1, b"fn_a\0".as_ptr());
            ynz_frame_push(b"b.ynz\0".as_ptr(), 2, b"fn_b\0".as_ptr());

            let err = ynz_error_new(b"oops\0".as_ptr());
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
            let err = ynz_error_new(b"empty\0".as_ptr());
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
                ynz_frame_push(b"f.ynz\0".as_ptr(), i, b"deep\0".as_ptr());
            }
            let len = FRAME_STACK.with(|s| s.borrow().len());
            assert_eq!(len, FRAME_STACK_LIMIT);
            // Clean up.
            FRAME_STACK.with(|s| s.borrow_mut().clear());
        }
    }
}
