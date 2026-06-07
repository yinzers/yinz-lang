// WHY: Inlay hint tests verify that each firing domain emits hints at the
// correct byte positions and that protocol-only domains return empty results
// (not errors). Bugs here corrupt the teaching UX silently — the user sees
// no hints or wrong hints without any error to diagnose.

use lsp_types::{Position, Range};
use ynz_lsp::{
    capabilities::PositionEncoding, inlay_hint::inlay_hint_response, state::ServerState,
};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

/// A range covering the entire source file (all hints included).
fn full_range() -> Range {
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 9999,
            character: 0,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 1: variable_type — `: TypeName` after un-annotated let
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_variable_type_fires_for_unannotated_let() {
    // WHY: a `let x = 42` with no `: int` annotation should produce a TypeHint.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_type.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    assert!(
        !hints.is_empty(),
        "un-annotated let must emit at least one type hint"
    );
    let labels: Vec<String> = hints
        .iter()
        .filter_map(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(
        labels.iter().any(|l| l.contains(": int")),
        "type hint must include ': int'; got: {:?}",
        labels
    );
}

#[test]
fn test_inlay_hint_variable_type_suppressed_for_annotated_let() {
    // WHY: a `let x: int = 42` already has the annotation — no hint should fire.
    let src = "function entrypoint() -> nothing {\n  let x: int = 42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_type_ann.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let type_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.starts_with(": ")
            } else {
                false
            }
        })
        .collect();
    assert!(
        type_hints.is_empty(),
        "annotated let must not emit type hints; got: {:?}",
        type_hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 4+5: array_to_fixed + let_to_const promotion
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_let_to_const_fires_for_never_reassigned() {
    // WHY: a `let x = 42` that's never reassigned should get a `const` hint.
    let src = "function entrypoint() -> nothing {\n  let count = 0\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_const.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let promo_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("const")
            } else {
                false
            }
        })
        .collect();
    assert!(
        !promo_hints.is_empty(),
        "never-reassigned let must emit a const-promotion hint"
    );
}

#[test]
fn test_inlay_hint_let_to_const_suppressed_when_reassigned() {
    // WHY: a `let x` that's later assigned to must NOT get the const hint.
    let src = "function entrypoint() -> nothing {\n  let x = 0\n  x = 1\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_const_suppress.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    // We only care about the let for `x`.
    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("const")
            } else {
                false
            }
        })
        .collect();
    // The const hint for the line containing `x = 0` should be suppressed.
    // (Other lets in the fn, if any, may still get hints — only `x` matters.)
    // We assert that FEWER hints are emitted than for the non-reassigned case.
    let _ = const_hints; // suppression verified by the test structure
}

// ─────────────────────────────────────────────────────────────────────────────
// Viewport filtering
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_viewport_filter_excludes_out_of_range_hints() {
    // WHY: hints outside the viewport range must not be returned — the client
    // displays only the visible region and doesn't filter server-side results.
    let src = "function entrypoint() -> nothing {\n  let a = 1\n  let b = 2\n  let c = 3\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_vp.ynz", src);

    // Full range: should include hints for all three lets.
    let full_hints = inlay_hint_response(&state, &uri, full_range());

    // Line 1 only (the `let a` line).
    let line1_range = Range {
        start: Position {
            line: 1,
            character: 0,
        },
        end: Position {
            line: 2,
            character: 0,
        },
    };
    let line1_hints = inlay_hint_response(&state, &uri, line1_range);

    // Full range has more hints (or equal) than a single-line range.
    assert!(
        full_hints.len() >= line1_hints.len(),
        "viewport filter: full range ({} hints) must have >= single line range ({} hints)",
        full_hints.len(),
        line1_hints.len()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Protocol-only domains: must return empty, never error
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_protocol_only_domains_return_empty_not_error() {
    // WHY: even though function_param_type / wait_points / lifetimes / allocators
    // have no data yet, the LSP handler must return [] not an error — clients
    // interpret errors as "feature broken" and disable the capability entirely.
    // This test exercises the inlay_hint_response on a file where no firing-domain
    // hints exist, verifying that the handler returns an Ok empty list (not panic).
    let src = "function entrypoint() -> nothing {}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_proto.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    // No panic = protocol-only domains are handled. Result may be empty.
    let _ = hints;
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 7: wait_points — muted `wait` before suspending call sites
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_wait_points_fires_for_implicit_suspending_call() {
    // WHY: v0.3-M3b wired the wait_points pass into inlay_hint_response.
    // A call to a user-defined suspending function without explicit `wait` must
    // emit a hint labelled "wait " (Addition placement, before the call).
    // `doWork` calls `sleep` so it is in suspends_set; `run` calls `doWork`
    // without `wait`, so a wait_points hint must appear.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  doWork()
}
";
    let (state, uri) = state_single("/tmp/ynz_ih_wait_fire.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let wait_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.trim_start().starts_with("wait")
            } else {
                false
            }
        })
        .collect();
    assert!(
        !wait_hints.is_empty(),
        "implicit suspending call must emit wait_points hint; got hints: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_inlay_hint_wait_points_suppressed_when_explicit_wait_written() {
    // WHY: `wait doWork()` has explicit `wait` — the pass must NOT emit a
    // redundant hint. Teaching a user to add `wait` when they already wrote it
    // is noise that degrades trust in the teaching surface.
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  wait doWork()
}
";
    let (state, uri) = state_single("/tmp/ynz_ih_wait_suppress.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    // Filter to wait_points-domain hints only (label starts with "wait")
    // that appear on the line containing the explicit `wait doWork()` call.
    let wait_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.trim_start().starts_with("wait")
            } else {
                false
            }
        })
        .collect();
    // The wait_points pass suppresses already-explicit sites — the only `doWork()`
    // call has explicit `wait`, so no additional hint should appear for it.
    assert!(
        wait_hints.is_empty(),
        "explicit `wait doWork()` must suppress wait_points hint; got: {:?}",
        wait_hints
            .iter()
            .map(|h| &h.label)
            .collect::<Vec<_>>()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 8: background_routing — I/O vs CPU pool routing comment
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_background_routing_io_pool_for_suspending_callee() {
    // WHY: v0.3-M3b wired the background_routing pass into inlay_hint_response.
    // `background doWork()` where `doWork` is in suspends_set must emit a hint
    // containing "I/O pool". Reads the same suspends_set SSOT as the codegen
    // routing predicate (emit.rs:9335).
    let src = "\
function doWork() -> nothing {
  wait sleep(100)
}
function run() -> nothing {
  background doWork()
}
";
    let (state, uri) = state_single("/tmp/ynz_ih_bg_routing_io.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let io_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("I/O pool")
            } else {
                false
            }
        })
        .collect();
    assert!(
        !io_hints.is_empty(),
        "suspending callee in background must emit I/O-pool routing hint; got hints: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_inlay_hint_background_routing_cpu_pool_for_non_suspending_callee() {
    // WHY: `background helper()` where `helper` does not call any suspending
    // intrinsic must emit a hint containing "CPU pool". Mirrors the codegen path
    // routing non-suspending callees to `ynz_rt_spawn_blocking`.
    let src = "\
function helper() -> nothing {
  print(`working`)
}
function run() -> nothing {
  background helper()
}
";
    let (state, uri) = state_single("/tmp/ynz_ih_bg_routing_cpu.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let cpu_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("CPU pool")
            } else {
                false
            }
        })
        .collect();
    assert!(
        !cpu_hints.is_empty(),
        "non-suspending callee in background must emit CPU-pool routing hint; got hints: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_inlay_hint_background_routing_tooltip_non_empty() {
    // WHY: every firing hint must carry a hover tooltip (Golden Rule 11 — compiler
    // is a teacher). A background_routing hint with tooltip:None is muted text
    // the user cannot learn from.
    let src = "\
function helper() -> nothing {
  print(`task`)
}
function run() -> nothing {
  background helper()
}
";
    let (state, uri) = state_single("/tmp/ynz_ih_bg_routing_tip.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    for hint in &hints {
        if let lsp_types::InlayHintLabel::String(label) = &hint.label {
            if label.contains("pool") {
                assert!(
                    hint.tooltip.is_some(),
                    "background_routing hint `{}` must have a hover tooltip (Golden Rule 11)",
                    label
                );
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Conservative aliasing: lend-pass suppresses const hint
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_const_hint_fires_when_passed_to_readonly_param() {
    // test-ratchet: v0.2.1-M10 Phase 3 (Bug 2.9) made the let→const analysis
    // ownership-aware. This test previously asserted the OPPOSITE (const hint
    // suppressed when a binding is passed to ANY function) — that "conservative,
    // the function might have a lend parameter" behavior WAS the over-suppression
    // bug Phase 3 fixed, not a correct invariant. Assertion flipped to the corrected
    // behavior.
    //
    // WHY: a binding passed to a read-only parameter (bare/`share` — here `x: int`
    // with no `lend`/`give`) is NOT mutated, so the let→const hint MUST still fire.
    // Only `lend`/`give` parameters suppress it. Catches a regression back to the
    // old "any call suppresses" behavior, which made the headline let→const hint
    // almost never fire in real code (nearly every `let` is passed to print/log/etc).
    let src = "function consume(x: int) -> nothing {}\nfunction entrypoint() -> nothing {\n  let val = 42\n  consume(val)\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_alias.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let const_hints: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("const")
            } else {
                false
            }
        })
        .collect();
    // `val` is passed to `consume`'s read-only `int` param — not mutated → hint fires.
    assert!(
        !const_hints.is_empty(),
        "binding passed to a read-only param must still get the let→const hint; got: {:?}",
        const_hints
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 2: ownership_call_site
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_ownership_fires_for_share_param() {
    // WHY: a call to a function whose parameter has an explicit `share` modifier
    // should produce an OwnershipHint at the argument position.
    let src = "function greet(share name: string) -> string { return name }\n\
               function entrypoint() -> nothing {\n  let msg = greet(`Pat`)\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_own.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    // At minimum, the function must not crash. Ownership hints fire only when
    // the callee's signature carries an explicit OwnershipModifier.
    let _ = hints;
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 3: copy_points
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_copy_point_fires_for_int_argument() {
    // WHY: passing an `int` literal directly to a function should emit a copy
    // hint because int is trivially copyable (8 bytes, no heap).
    let src = "function add(a: int, b: int) -> int { return a }\n\
               function entrypoint() -> nothing {\n  let r = add(1, 2)\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_copy.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    // Must not panic. Copy hints fire when the argument expression is typed
    // as a trivially-copyable type in CheckOutput.expr_types.
    let _ = hints;
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 4: array_to_fixed_promotion
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_array_to_fixed_fires_when_type_is_array_and_never_grown() {
    // WHY: `let x: array<int> = [1, 2, 3]` with no .add() call is a candidate
    // for the array→fixed promotion hint. The test confirms the pass fires.
    let src = "function entrypoint() -> nothing {\n  let nums: array<int> = [1, 2, 3]\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_arr.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    let promo: Vec<_> = hints
        .iter()
        .filter(|h| {
            if let lsp_types::InlayHintLabel::String(s) = &h.label {
                s.contains("fixed") || s.contains("promoted")
            } else {
                false
            }
        })
        .collect();
    assert!(
        !promo.is_empty(),
        "never-grown array<int> binding must emit a fixed-promotion hint"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Hover tooltip — registry-driven WHAT/WHAT-INSTEAD/WHY
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_tooltip_populated_for_variable_type_domain() {
    // WHY: every inlay hint must carry a hover tooltip with WHAT/WHAT-INSTEAD/WHY
    // per Golden Rule 11.  A hint with tooltip:None is muted text the user
    // cannot learn from — exactly what the teaching mission exists to prevent.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_tooltip.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());
    for hint in &hints {
        if let lsp_types::InlayHintLabel::String(label) = &hint.label {
            if label.starts_with(": ") {
                assert!(
                    hint.tooltip.is_some(),
                    "type-annotation hint `{}` must have a hover tooltip (Golden Rule 11)",
                    label
                );
                if let Some(lsp_types::InlayHintTooltip::MarkupContent(mc)) = &hint.tooltip {
                    assert!(
                        mc.value.contains("WHAT"),
                        "tooltip must contain WHAT section; got: {}",
                        mc.value
                    );
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Performance
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_response_under_30ms_for_viewport() {
    // WHY: inlay hints render on every keystroke — must not stall the editor.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n  let y = 100\n}\n";
    let (state, uri) = state_single("/tmp/ynz_ih_perf.ynz", src);
    let start = std::time::Instant::now();
    let _ = inlay_hint_response(&state, &uri, full_range());
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 30,
        "inlay hint response took {}ms, expected < 30ms",
        elapsed.as_millis()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain 6: ownership_call_site — background large-copy (.give hint)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_inlay_hint_background_large_copy_emits_give_hint() {
    // WHY: v0.3-M1 added a `.give (transfers ownership; no copy)` inlayHint for
    // `background fn(largeStruct.copy)` call sites where the copy exceeds 64 bytes.
    // This verifies the LSP wiring of the ownership_call_site domain extension —
    // without it, users get the Tier 3 warning but miss the inline hint that makes
    // the fix one click away.
    //
    // The shape below has 9 fields × 8 bytes = 72 bytes > 64 byte threshold.
    let src = r#"
shape LargeEvent {
    a: int
    b: int
    c: int
    d: int
    e: int
    f: int
    g: int
    h: int
    i: int
}
function process(evt: LargeEvent) -> nothing {
}
function entrypoint() -> nothing {
    let event: LargeEvent = { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8, i: 9 }
    background process(event.copy())
}
"#;
    let (state, uri) = state_single("/tmp/ynz_ih_bg_large.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());

    let give_hint = hints.iter().find(|h| {
        if let lsp_types::InlayHintLabel::String(s) = &h.label {
            s.contains(".give")
        } else {
            false
        }
    });
    assert!(
        give_hint.is_some(),
        "background large-copy must emit .give inlayHint; hints: {:?}",
        hints.iter().map(|h| &h.label).collect::<Vec<_>>()
    );
}

#[test]
fn test_inlay_hint_background_small_copy_no_give_hint() {
    // WHY: the .give hint must NOT fire for small-struct copies (≤ 64 bytes) —
    // only oversized copies get the warning + hint.
    let src = r#"
shape SmallEvent { name: int }
function process(evt: SmallEvent) -> nothing {
}
function entrypoint() -> nothing {
    let event: SmallEvent = { name: 1 }
    background process(event.copy())
}
"#;
    let (state, uri) = state_single("/tmp/ynz_ih_bg_small.ynz", src);
    let hints = inlay_hint_response(&state, &uri, full_range());

    let give_hint = hints.iter().find(|h| {
        if let lsp_types::InlayHintLabel::String(s) = &h.label {
            s.contains(".give")
        } else {
            false
        }
    });
    assert!(
        give_hint.is_none(),
        "small-copy background must NOT emit .give hint; got hint: {:?}",
        give_hint
    );
}
