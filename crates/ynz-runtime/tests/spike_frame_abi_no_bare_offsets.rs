//! Completeness gate for the frame-ABI offset contract — both the spike/CPU-group layer
//! (discriminator word, CPU-handle slots) AND, as of v0.3-M3g Phase 1, the general SM
//! frame-header layer (resume_point@0, sleep_handle@8, return_slot@16).
//!
//! Both layers are written by `ynz-codegen` and read back by `ynz-runtime` across a binary-layout
//! seam. Every offset in that contract has a single home in `ynz-abi`
//! (`SPIKE_FRAME_DISCRIMINATOR_OFFSET`, `SPIKE_HANDLE_BASE_OFFSET`, `SPIKE_HANDLE_SLOT_BYTES`,
//! `FRAME_OFFSET_SLEEP_HANDLE`, `FRAME_OFFSET_RETURN_SLOT`, `FRAME_HEADER_SIZE`). A bare numeric
//! offset (`.add(4)`, `const_int(32, false)`, ...) reappearing outside `ynz-abi` is exactly the
//! drift this seam cannot tolerate: a future edit moves one side and the other keeps reading the
//! old byte, so cancellation frees a live-local-turned-garbage pointer (heap corruption).
//!
//! Two gates:
//!   - `no_bare_spike_frame_offset_literals` — unchanged from the M3d sub-slice-4d original.
//!     Simple exact-substring scan is safe here because the spike offset VALUES (4, 32, 40) are
//!     never used for anything else in the scanned files (this is ALSO now true of the general
//!     header's sleep_handle(8)/return_slot(16) values on the runtime side, post-Phase-1-sweep —
//!     see below — so `.add(8)`/`.add(16)` are folded into this same simple gate).
//!   - `no_bare_general_frame_header_offset_in_codegen_gep` — v0.3-M3g Phase 1 addition. The
//!     general header's offset VALUES (8, 16) are NOT unique on the codegen side: `const_int(8,
//!     false)` / `const_int(16, false)` also appear legitimately as a return-slot-INTERNAL field
//!     stride (offset within an already-offset return-slot pointer, unrelated to the frame's own
//!     header) and as unrelated byte-count arguments (an 8-byte spawn-ctx buffer, a 16-byte heap
//!     allocation). A plain substring scan would false-positive on those. This gate instead scans
//!     for the GEP-call SHAPE (`.build_gep(<i8-element-type-line>, <base>, &[const_int(N,
//!     false)], ...)`) and flags a bare 8/16 literal only when the GEP's base pointer is NOT one
//!     of the documented already-offset exceptions — see `find_bare_frame_header_offset_geps` doc
//!     below. The element-type line is EITHER `ctx.i8_type(),` (inline) OR `i8_ty,` (a
//!     `let i8_ty = ctx.i8_type();` / `let i8_ty = cg.ctx.i8_type();` bound local — the shape
//!     `emit.rs` actually uses at 5 of its 16 byte-offset-GEP call sites, confirmed by grep
//!     2026-07-01). Both are recognized; see `BYTE_ELEMENT_TYPE_LINES` below.
//!
//! Deliberately OUT of scope:
//!   - `const_int(8, false)` ctx-size / alloc-size arguments that are not GEP offsets at all
//!     (e.g. the 8-byte spawn ctx buffer, `sizeof(i64)`; a 16-byte `ynz_alloc` call for an i128
//!     heap box) — not offsets, so out of scope for both gates.
//!   - The return-slot's own internal 8-byte field stride (`store_return_value_errors` /
//!     `load_errors_pair` in `state_machine.rs`, GEP base `slot`/`slot8`) — this is NOT a
//!     frame-header offset; it is an offset WITHIN the already-offset 16-byte return slot, and is
//!     not part of the cross-crate ABI contract this gate exists to protect (the runtime never
//!     reads it independently — only `ynz-codegen` produces and consumes it, entirely within one
//!     crate). See `ALLOWED_SUB_OFFSET_BASES` below.
//!
//! Time: O(n) where n = total bytes of the four scanned source files. Space: O(n).

use std::path::Path;

/// One scanned source region: the file and the bare-offset patterns that are forbidden in it.
/// `forbidden` holds full substrings (not regexes) so the gate has no dependency and no false
/// positives from partial numeric matches (e.g. `.add(40)` is forbidden but `.add(400)` is not,
/// because we match the exact closing paren).
struct ScanTarget {
    relative_path: &'static str,
    forbidden: &'static [&'static str],
}

/// Spike-frame discriminator offset (4), handle-base offset (32), and handle-slot-1 offset (40),
/// plus (v0.3-M3g Phase 1) the general frame-header's sleep_handle(8) and return_slot(16) byte
/// offsets, as they appear in runtime pointer arithmetic. Safe as an exact-substring scan on the
/// RUNTIME side because — verified 2026-07-01, after Phase 1's sweep moved every header-offset
/// GEP in `runtime.rs`/`lib.rs` onto the named `ynz_abi::FRAME_OFFSET_*` constants — no
/// remaining `.add(8)` / `.add(16)` call in either file means anything OTHER than a frame-header
/// offset (every other `.add(N)` in these files uses a dynamic, non-literal offset expression).
const RUNTIME_FORBIDDEN: &[&str] = &[".add(4)", ".add(8)", ".add(16)", ".add(32)", ".add(40)"];
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

// WHY: the spike-frame offset contract has exactly one home (ynz-abi). A bare 4/8/16/32/40 frame
//      offset re-appearing in either crate's spike paths (or, for 8/16, the runtime's general
//      frame-header paths) is the offset-drift bug class that BLOCKed three review rounds, one
//      member per round, in M3d sub-slice 4d. This gate makes the next bare literal a build-time
//      failure instead of a fourth review finding. If it fails: replace the literal with
//      `ynz_abi::SPIKE_FRAME_DISCRIMINATOR_OFFSET` / `SPIKE_HANDLE_BASE_OFFSET` /
//      `SPIKE_HANDLE_BASE_OFFSET + SPIKE_HANDLE_SLOT_BYTES` / `FRAME_OFFSET_SLEEP_HANDLE` /
//      `FRAME_OFFSET_RETURN_SLOT` as appropriate — do NOT relax this list.
/// Time: O(n * p)  Space: O(v)  where n = total bytes of all scanned source files, p = patterns per target class (constant = 5), v = violation count found.
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

/// Base-pointer identifiers (trimmed, trailing comma included) that are KNOWN to be an
/// already-offset pointer rather than a frame's own raw base pointer. A bare 8/16 literal GEP
/// off one of these is the return-slot's internal field stride (see module doc), not a
/// frame-header offset — codegen-internal, single-crate, out of this gate's ABI-seam scope.
///
/// Any base pointer NOT in this list is presumed to be a frame's own raw base (`frame_ptr`,
/// `child_ptr`, `child_frame`, `parent_frame`, `self.frame_ptr`, ...) — a bare 8/16 GEP off any
/// of those IS the frame-header-offset drift class this gate exists to catch.
const ALLOWED_SUB_OFFSET_BASES: &[&str] = &["slot,", "slot8,"];

/// Bare frame-header-offset literals a byte-offset GEP must never use.
const FORBIDDEN_HEADER_OFFSET_LITERALS: &[&str] = &["const_int(8, false)", "const_int(16, false)"];

/// Element-type argument lines (trimmed, trailing comma included) that identify a `.build_gep`
/// call as a BYTE-offset GEP (i8 element type) rather than a typed-element GEP (e.g. `i64_ty,` /
/// `cg.i64(),` array indexing, which can't express a frame-header byte offset and is correctly
/// out of scope). Two shapes appear in `state_machine.rs` / `emit.rs` (verified by grep,
/// 2026-07-01, v0.3-M3g Phase 1 should-fix): the inline `ctx.i8_type(),` call, and `i8_ty,` — a
/// local bound via `let i8_ty = ctx.i8_type();` / `let i8_ty = cg.ctx.i8_type();` earlier in the
/// same function (5 of `emit.rs`'s 16 byte-offset-GEP call sites use the bound-local form; the
/// prior version of this gate matched ONLY the inline form and was blind to all 5 — a confirmed
/// false-negative coverage gap, not a hypothetical one). Grep confirms no third identifier is
/// ever bound to `ctx.i8_type()`/`cg.ctx.i8_type()` and then fed as a `.build_gep` element-type
/// argument in either file — if a future edit introduces one, this list needs a matching entry.
const BYTE_ELEMENT_TYPE_LINES: &[&str] = &["ctx.i8_type(),", "i8_ty,"];

/// Scan `source` for `.build_gep(...)` call blocks and return violation strings for any bare
/// `const_int(8, false)` / `const_int(16, false)` literal used as the byte offset off a frame's
/// own raw base pointer.
///
/// Every byte-offset `.build_gep(...)` call in `state_machine.rs` / `emit.rs` is formatted by
/// `cargo fmt` (verified 2026-07-01, v0.3-M3g Phase 1) in exactly this shape:
/// ```text
///     .build_gep(
///         ctx.i8_type(),      // or the bound-local form: i8_ty,
///         <base_ptr>,
///         &[<offset_expr>],
///         "<name>",
///     )
/// ```
/// so the element-type argument is always the line immediately after `.build_gep(`, and the
/// base-pointer argument is always the line after that. This function locates that shape (via
/// `.build_gep(` as the trimmed-line anchor, and membership in `BYTE_ELEMENT_TYPE_LINES` as the
/// byte-offset-GEP confirmation — a non-byte GEP, e.g. i64-element array indexing, is out of
/// scope since it can't express a frame-header byte offset), then checks the following few lines
/// for a forbidden bare literal. A hit is a violation UNLESS the base pointer is in
/// `ALLOWED_SUB_OFFSET_BASES`.
///
/// This is a text-shape heuristic, not a real parser — like `no_bare_spike_frame_offset_literals`
/// above, it trades completeness-under-arbitrary-reformatting for zero dependencies and
/// zero-false-positives against the actual code as written. If a future reformat collapses a
/// `build_gep` call onto fewer lines, or introduces a THIRD identifier bound to an i8 LLVM type
/// and used as a GEP element-type argument, this function stops seeing it — the fix is to update
/// the line-offset assumptions / `BYTE_ELEMENT_TYPE_LINES` here, not to accept the drift class
/// back into the ABI seam silently.
fn find_bare_frame_header_offset_geps(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut violations = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        if line.trim() != ".build_gep(" {
            continue;
        }
        let Some(elem_ty_line) = lines.get(i + 1) else {
            continue;
        };
        if !BYTE_ELEMENT_TYPE_LINES.contains(&elem_ty_line.trim()) {
            continue; // not a byte-offset GEP — cannot express a frame-header byte offset
        }
        let Some(base_line) = lines.get(i + 2) else {
            continue;
        };
        let base = base_line.trim();
        if ALLOWED_SUB_OFFSET_BASES.contains(&base) {
            continue; // documented already-offset exception (return-slot internal field stride)
        }

        // The offset argument (inside `&[...]`) lives within the next few lines of this GEP's
        // argument list — scan a small window past the base-pointer line.
        for (window_idx, offset_line) in lines.iter().enumerate().skip(i + 3).take(3) {
            for literal in FORBIDDEN_HEADER_OFFSET_LITERALS {
                if offset_line.contains(literal) {
                    violations.push(format!(
                        "line {}: bare frame-header offset `{literal}` on GEP base `{base}` — \
                         use ynz_abi::FRAME_OFFSET_SLEEP_HANDLE / FRAME_OFFSET_RETURN_SLOT \
                         instead\n    {}",
                        window_idx + 1,
                        offset_line.trim()
                    ));
                }
            }
        }
    }

    violations
}

// WHY: proves `find_bare_frame_header_offset_geps` has REAL detection power on the `i8_ty,`
//      bound-local element-type shape — not just a broadened-but-still-blind regex. Before this
//      v0.3-M3g Phase 1 should-fix, the gate matched ONLY the inline `ctx.i8_type(),` form and
//      was structurally unable to see a bare-offset GEP written with a bound local (the exact
//      shape 5 real sites in `emit.rs` use — `emit.rs:7082`, `:7457`, `:12533`, `:12553`,
//      `:12568`). This synthetic fixture reproduces that shape directly (mirroring
//      `emit.rs:7082`'s real GEP off `frame_ptr`) so the detection claim is pinned by a
//      permanent regression test, independent of whatever real code looks like tomorrow.
#[test]
fn find_bare_frame_header_offset_geps_catches_bound_local_i8_ty_shape() {
    let synthetic_source = r#"
fn emit_something(frame_ptr: PointerValue) {
    let i8_ty = ctx.i8_type();
    let sleep_ptr = ctx
        .builder
        .build_gep(
            i8_ty,
            frame_ptr,
            &[ctx.i64_type().const_int(8, false)],
            "sleep_handle_ptr",
        )
        .unwrap();
}
"#;

    let violations = find_bare_frame_header_offset_geps(synthetic_source);

    assert_eq!(
        violations.len(),
        1,
        "gate must catch a bare `const_int(8, false)` offset behind the bound-local `i8_ty,` \
         element-type shape (the shape the prior `ctx.i8_type(),`-only matcher was blind to); \
         got: {violations:?}"
    );
    assert!(
        violations[0].contains("const_int(8, false)") && violations[0].contains("frame_ptr"),
        "violation message must name the offending literal and the raw base pointer; got: {}",
        violations[0]
    );
}

// WHY: the general SM frame-header offsets (resume_point@0, sleep_handle@8, return_slot@16)
//      moved into `ynz-abi` in v0.3-M3g Phase 1 alongside the spike-frame constants — the SAME
//      drift class `no_bare_spike_frame_offset_literals` guards, just with offset values (8, 16)
//      that are NOT unique to the frame header on the codegen side, so a context-aware scan is
//      required instead of a plain substring ban (see module doc + the two known legitimate
//      non-header uses of the literal `8`/`16` text this test must NOT flag).
/// Time: O(n) where n = total bytes of the two scanned source files. Space: O(v) where v = violation count.
#[test]
fn no_bare_general_frame_header_offset_in_codegen_gep() {
    let codegen_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ynz-codegen/src");
    let mut violations: Vec<String> = Vec::new();

    for relative_path in ["state_machine.rs", "emit.rs"] {
        let path = codegen_dir.join(relative_path);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("completeness gate cannot read {}: {e}", path.display()));
        for v in find_bare_frame_header_offset_geps(&source) {
            violations.push(format!("../ynz-codegen/src/{relative_path}:{v}"));
        }
    }

    assert!(
        violations.is_empty(),
        "bare general-frame-header offset literal(s) found in a codegen frame GEP — the \
         offset-drift bug class has re-seeded outside ynz-abi. Replace each with \
         ynz_abi::FRAME_OFFSET_SLEEP_HANDLE / FRAME_OFFSET_RETURN_SLOT:\n{}",
        violations.join("\n")
    );
}
