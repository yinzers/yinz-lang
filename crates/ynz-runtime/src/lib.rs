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
