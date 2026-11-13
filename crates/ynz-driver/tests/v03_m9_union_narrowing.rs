//! v0.3-M9 C1: `is Variant =>` union narrowing reads the variant PAYLOAD, not the
//! `{i64 tag, i64 data}` envelope. Tier: High (data integrity — a wrong read is silent
//! garbage or an out-of-bounds read).
//!
//! RED-first: both fixtures fail on the pre-fix tree.
//!   - `field_read`: the arm typed `fig` as the variant but codegen left the binding on the
//!     16-byte envelope, so `fig.radius` returned the tag word (a zero value, not `5.0`).
//!   - `payload_copy`: `let inner: Circle = fig` memcpy'd `sizeof(Circle)` bytes out of the
//!     16-byte envelope (AddressSanitizer: stack-buffer-overflow READ in memcpy) and the
//!     string field then dereferenced garbage (SIGSEGV in the optimized build).
//!
//! The AddressSanitizer run itself is not in this suite (it needs the sanitizer runtime);
//! the output assertions here catch the same defect deterministically: the pre-fix reads
//! are never the fixture's values. Each fixture runs at the default optimization level and
//! under `YNZ_NO_OPTIMIZE=1` (what `ynz build --no-optimize` sets) — one named test per case
//! so the runner parallelizes them and names the failing case.

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Run `ynz run <fixture>` and return (stdout, stderr, exit code; -1 when signalled).
fn run_fixture(name: &str, no_optimize: bool) -> (String, String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ynz"));
    cmd.args(["run", fixture(name).to_str().unwrap()])
        .env("CLICOLOR", "0");
    if no_optimize {
        cmd.env("YNZ_NO_OPTIMIZE", "1");
    }
    let output = cmd.output().expect("failed to spawn ynz binary");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

fn assert_fixture_prints(name: &str, no_optimize: bool, expected_stdout: &str) {
    let (stdout, stderr, code) = run_fixture(name, no_optimize);
    assert_eq!(
        code, 0,
        "{name} (no_optimize={no_optimize}) must run clean; stderr:\n{stderr}"
    );
    assert_eq!(
        stdout, expected_stdout,
        "{name} (no_optimize={no_optimize}): the narrowed arm must read the variant payload; \
         stderr:\n{stderr}"
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

const SM_HOST: &str = "v0_3_m9_union_narrow_sm_host.ynz";

#[test]
fn v03_m9_union_narrow_sm_host_optimized() {
    // WHY: `lower_sm_match` is a second, separate `is`-arm lowering site; fixing only the
    // plain-match site would leave narrowing in any suspending function reading the envelope.
    assert_fixture_prints(SM_HOST, false, "round\n5.0\nround\nend\n");
}

#[test]
fn v03_m9_union_narrow_sm_host_no_optimize() {
    // WHY: same SM-site defect at -O0.
    assert_fixture_prints(SM_HOST, true, "round\n5.0\nround\nend\n");
}
