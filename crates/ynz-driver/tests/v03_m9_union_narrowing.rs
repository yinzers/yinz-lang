//! v0.3-M9 C1: `is Variant =>` union narrowing reads the variant PAYLOAD, not the
//! `{i64 tag, i64 data}` envelope. Tier: High (data integrity — a wrong read is silent
//! garbage or an out-of-bounds read).
//!
//! RED-first: all three fixtures fail on the pre-fix tree. Measured: with
//! `narrow_union_local_to_variant` disabled, all ten tests below are red; with only the
//! post-resume re-derivation (`renarrow_union_locals`) reverted, the four `sm_host` tests are
//! red and the other six stay green.
//!   - `field_read`: the arm typed `fig` as the variant but codegen left the binding on the
//!     16-byte envelope, so `fig.radius` returned the tag word (a zero value, not `5.0`).
//!   - `payload_copy`: `let inner: Circle = fig` memcpy'd `sizeof(Circle)` bytes out of the
//!     16-byte envelope (AddressSanitizer: stack-buffer-overflow READ in memcpy) and the
//!     string field then dereferenced garbage (SIGSEGV in the optimized build).
//!   - `sm_host`: in a suspending function (`lower_sm_match`, a second `is`-arm lowering site)
//!     the resume reloaded the envelope into the narrowed payload slot, so a read after a
//!     `wait` inside the arm saw the envelope again.
//!
//! The `*_asan_*` tests run the `payload_copy` and `sm_host` fixtures under AddressSanitizer
//! through the shared harness (`support::assert_asan_clean_and_prints`, which needs `clang`
//! with the ASan runtime, as the C2 safety fixtures do): the card's criterion is "clean under
//! ASan, failed before the fix", and the output assertions alone would not name an
//! out-of-bounds read that happens to return plausible bytes. Each fixture runs at the default
//! optimization level and under `--no-optimize` — one named test per case, built with
//! `ynz build` into its own temp directory and run as its own process under a liveness
//! watchdog, so the runner parallelizes them and names the failing case.
//!
//! Why 10 tests where the SOP says "about 8" per High-tier unit: three distinct lowering
//! paths (plain match arm, copy out of the arm, suspending-function arm), each at two
//! optimization levels; the plain and ASan variants of one fixture differ only in the
//! sanitizer verdict, kept apart so a failure names whether the VALUE or the MEMORY ACCESS is
//! wrong.

mod support;

use support::{
    assert_asan_clean_and_prints as assert_asan_clean, assert_fixture_prints as assert_prints,
};

/// The check every case here shares: the narrowed arm reads the variant payload.
fn assert_fixture_prints(name: &str, no_optimize: bool, expected_stdout: &str) {
    assert_prints(
        name,
        no_optimize,
        expected_stdout,
        "the narrowed arm must read the variant payload",
    );
}

const FIELD_READ: &str = "v0_3_m9_union_narrow_field_read.ynz";
const PAYLOAD_COPY: &str = "v0_3_m9_union_narrow_payload_copy.ynz";

#[test]
fn v03_m9_union_narrow_field_read_optimized() {
    // WHY: `fig.radius` in `is Circle =>` returned the envelope tag word instead of 5.0.
    assert_fixture_prints(FIELD_READ, false, "5.0\n3.0\n");
}

#[test]
fn v03_m9_union_narrow_field_read_no_optimize() {
    // WHY: same defect at -O0, where no optimizer can mask the envelope read.
    assert_fixture_prints(FIELD_READ, true, "5.0\n3.0\n");
}

#[test]
fn v03_m9_union_narrow_payload_copy_optimized() {
    // WHY: `let inner: Circle = fig` read sizeof(Circle) bytes out of a 16-byte envelope
    // (OOB read; SIGSEGV on the string field in the optimized build).
    assert_fixture_prints(PAYLOAD_COPY, false, "round\n5.0\n78.5\nt\n");
}

#[test]
fn v03_m9_union_narrow_payload_copy_no_optimize() {
    // WHY: same OOB read at -O0 (the ASan red).
    assert_fixture_prints(PAYLOAD_COPY, true, "round\n5.0\n78.5\nt\n");
}

#[test]
fn v03_m9_union_narrow_payload_copy_asan_optimized() {
    // WHY: C1 criterion 2 — `let inner: Circle = fig` read sizeof(Circle) bytes out of the
    // 16-byte envelope (ASan: READ in memcpy); the copy must be memory-safe, not merely
    // plausible-looking.
    assert_asan_clean(PAYLOAD_COPY, false, "round\n5.0\n78.5\nt\n");
}

#[test]
fn v03_m9_union_narrow_payload_copy_asan_no_optimize() {
    // WHY: same out-of-bounds copy at -O0, where nothing optimizes the read away.
    assert_asan_clean(PAYLOAD_COPY, true, "round\n5.0\n78.5\nt\n");
}

const SM_HOST: &str = "v0_3_m9_union_narrow_sm_host.ynz";
const SM_HOST_STDOUT: &str =
    "round\n5.0\nround\nround\n5.0\nround\nresumed\n5.0\nround\nround\nend\n";

#[test]
fn v03_m9_union_narrow_sm_host_optimized() {
    // WHY: `lower_sm_match` is a second, separate `is`-arm lowering site; fixing only the
    // plain-match site would leave narrowing in any suspending function reading the envelope.
    // The post-`wait` reads catch a resume that reloads the envelope into the payload slot.
    assert_fixture_prints(SM_HOST, false, SM_HOST_STDOUT);
}

#[test]
fn v03_m9_union_narrow_sm_host_no_optimize() {
    // WHY: same SM-site defect at -O0.
    assert_fixture_prints(SM_HOST, true, SM_HOST_STDOUT);
}

#[test]
fn v03_m9_union_narrow_sm_host_asan_optimized() {
    // WHY: after a `wait` inside the arm, `let after: Circle = fig` copied sizeof(Circle) bytes
    // out of the 16-byte envelope the resume reload had written into the payload slot.
    assert_asan_clean(SM_HOST, false, SM_HOST_STDOUT);
}

#[test]
fn v03_m9_union_narrow_sm_host_asan_no_optimize() {
    // WHY: same post-resume envelope read at -O0.
    assert_asan_clean(SM_HOST, true, SM_HOST_STDOUT);
}

// The narrowed binding passed or returned where the UNION is expected (union parameter of a
// plain, `wait`ed, UFCS or generic call; union return). Two more tests past the ten above
// because this is a separate lowering decision (envelope vs payload per use), and the ASan
// verdict is the criterion: the pre-fix output was wrong only some of the time, the read past
// the 1-byte variant every time.
const UNION_SLOT: &str = "v0_3_m9_union_narrow_union_slot.ynz";
const UNION_SLOT_STDOUT: &str = "tiny\ntrue\ntiny\ntiny\ntiny\ntiny\ntrue\nend\n";

#[test]
fn v03_m9_union_narrow_union_slot_asan_optimized() {
    // WHY: the callee read a 16-byte envelope out of the 1-byte `Tiny` payload (heap/stack
    // buffer overflow READ, wrong variant `big`). Tier: High (memory safety).
    assert_asan_clean(UNION_SLOT, false, UNION_SLOT_STDOUT);
}

#[test]
fn v03_m9_union_narrow_union_slot_asan_no_optimize() {
    // WHY: same out-of-bounds envelope read at -O0. Tier: High (memory safety).
    assert_asan_clean(UNION_SLOT, true, UNION_SLOT_STDOUT);
}
