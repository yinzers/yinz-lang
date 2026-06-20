//! Completeness gate for the spike/CPU-group frame-ABI offset contract.
//!
//! The spike-frame layout (discriminator word, CPU-handle slots) is written by `ynz-codegen`
//! and read back by `ynz-runtime` across a binary-layout seam. Every offset in that contract
//! has a single home in `ynz-abi` (`SPIKE_FRAME_DISCRIMINATOR_OFFSET`, `SPIKE_HANDLE_BASE_OFFSET`,
//! `SPIKE_HANDLE_SLOT_BYTES`). A bare numeric offset (`.add(4)`, `const_int(32, false)`, ...) in
//! the spike paths is exactly the drift this seam cannot tolerate: a future edit moves one side
//! and the other keeps reading the old byte, so cancellation frees a live-local-turned-garbage
//! pointer (heap corruption).
//!
//! This gate scans the spike-bearing source of BOTH crates and fails if any bare spike-frame
//! offset literal (4 / 32 / 40 as a frame GEP or `.add()`) reappears outside `ynz-abi`. It is
//! the structural backstop that stops the offset-drift bug class from re-seeding one member at a
//! time: a newly introduced bare literal turns red here at `cargo test`, before review.
//!
//! Deliberately OUT of scope (a separate, finding-free layer):
//!   - The general SM-frame-header offsets (resume_point@0, sleep_handle@8, return_slot@16) — a
//!     pre-existing layout that has never drifted and is not part of the spike-frame ABI class.
//!     Only the spike values {disc@4, handle-base@32, handle-slot-1@40} are guarded here.
//!   - `const_int(8, false)` ctx-size arguments — the 8-byte spawn ctx buffer is `sizeof(i64)`,
//!     coincidentally 8 but semantically unrelated to the handle-slot stride; not an offset.
//!
//! Time: O(n) where n = total bytes of the three scanned source files. Space: O(n).

use std::path::Path;

/// One scanned source region: the file and the bare-offset patterns that are forbidden in it.
/// `forbidden` holds full substrings (not regexes) so the gate has no dependency and no false
/// positives from partial numeric matches (e.g. `.add(40)` is forbidden but `.add(400)` is not,
/// because we match the exact closing paren).
struct ScanTarget {
    relative_path: &'static str,
    forbidden: &'static [&'static str],
}

/// Spike-frame discriminator offset (4), handle-base offset (32), and handle-slot-1 offset (40)
/// as they appear in codegen LLVM GEP constants and runtime pointer arithmetic. These are the
/// complete set of spike-frame offset literals; each must be the named `ynz-abi` constant.
const RUNTIME_FORBIDDEN: &[&str] = &[".add(4)", ".add(32)", ".add(40)"];
const CODEGEN_FORBIDDEN: &[&str] = &[
    "const_int(4, false)",
    "const_int(32, false)",
    "const_int(40, false)",
];

const SCAN_TARGETS: &[ScanTarget] = &[
    ScanTarget {
        relative_path: "src/runtime.rs",
        forbidden: RUNTIME_FORBIDDEN,
    },
    ScanTarget {
        relative_path: "src/lib.rs",
        forbidden: RUNTIME_FORBIDDEN,
    },
    ScanTarget {
        // `ynz-codegen` is a sibling crate in the same workspace; the gate guards the codegen
        // side of the seam from the runtime crate because the contract is owned jointly.
        relative_path: "../ynz-codegen/src/emit.rs",
        forbidden: CODEGEN_FORBIDDEN,
    },
];

// WHY: the spike-frame offset contract has exactly one home (ynz-abi). A bare 4/32/40 frame
//      offset re-appearing in either crate's spike paths is the offset-drift bug class that
//      BLOCKed three review rounds, one member per round. This gate makes the next bare literal
//      a build-time failure instead of a fourth review finding. If it fails: replace the literal
//      with `ynz_abi::SPIKE_FRAME_DISCRIMINATOR_OFFSET` / `SPIKE_HANDLE_BASE_OFFSET` /
//      `SPIKE_HANDLE_BASE_OFFSET + SPIKE_HANDLE_SLOT_BYTES` — do NOT relax this list.
/// Time: O(n * p)  Space: O(v)  where n = total bytes of all scanned source files, p = patterns per target class (constant = 3), v = violation count found.
#[test]
fn no_bare_spike_frame_offset_literals() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let mut violations: Vec<String> = Vec::new();

    for target in SCAN_TARGETS {
        let path = Path::new(crate_dir).join(target.relative_path);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("completeness gate cannot read {}: {e}", path.display()));

        for (line_no, line) in source.lines().enumerate() {
            for pattern in target.forbidden {
                if line.contains(pattern) {
                    violations.push(format!(
                        "{}:{} bare spike-frame offset `{}` — use the ynz-abi constant instead\n    {}",
                        target.relative_path,
                        line_no + 1,
                        pattern,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "bare spike-frame offset literal(s) found outside ynz-abi — the offset-drift bug class \
         has re-seeded. Replace each with the named ynz-abi constant:\n{}",
        violations.join("\n")
    );
}
