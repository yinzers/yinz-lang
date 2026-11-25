// WHY: Inlay hint tests verify that each firing domain emits hints for the
// right constructs, stays silent where the source already says it or is
// already an error, and honours the viewport range. A wrong hint teaches
// the wrong thing silently: the user sees no error to diagnose.

use lsp_types::{InlayHint, InlayHintLabel, Position, Range};
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

fn string_labels(hints: &[InlayHint]) -> Vec<String> {
    hints
        .iter()
        .filter_map(|hint| match &hint.label {
            InlayHintLabel::String(label) => Some(label.clone()),
            _ => None,
        })
        .collect()
}

fn capacity_hint_count(hints: &[InlayHint]) -> usize {
    string_labels(hints)
        .iter()
        .filter(|label| label.as_str() == "64")
        .count()
}

const CHANNEL_DEFAULT_SRC: &str =
    "function entrypoint() -> Nothing {\n  let wire: Channel<Int> = Channel<Int>()\n}\n";

const AUTO_ARC_GROUP_SRC: &str = "\
shape Scene {
  name: String
  width: Int
}
function render(scene: Scene) -> Nothing {
  wait sleep(1)
  print(scene.name)
}
function run() -> Nothing {
  let scene: Scene = { name: `Three Rivers`, width: 6 }
  background render(scene)
  background render(scene)
  print(scene.width)
}
";

fn auto_arc_labels(hints: &[InlayHint]) -> Vec<String> {
    string_labels(hints)
        .into_iter()
        .filter(|label| label.contains("shared by reference count"))
        .collect()
}

macro_rules! inlay_hint_case {
    ($name:ident, $src:expr, $range:expr, $check:expr) => {
        // bug: a firing inlay-hint domain shows a hint on the wrong construct, misses one it should show, repeats what the source already says or piles onto a compile error, or ignores the viewport | tier: Medium
        #[test]
        fn $name() {
            let (state, uri) =
                state_single(concat!("/tmp/ynz_ih_", stringify!($name), ".ynz"), $src);
            let hints = inlay_hint_response(&state, &uri, $range);
            let check: fn(&[InlayHint]) = $check;
            check(&hints);
        }
    };
}

inlay_hint_case!(
    unannotated_let_gets_its_inferred_type_hint,
    "function entrypoint() -> Nothing {\n  let x = 42\n}\n",
    full_range(),
    |hints| {
        let labels = string_labels(hints);
        assert!(
            labels.iter().any(|label| label.contains(": Int")),
            "got {labels:?}"
        );
    }
);

inlay_hint_case!(
    annotated_let_gets_no_type_hint,
    "function entrypoint() -> Nothing {\n  let x: Int = 42\n}\n",
    full_range(),
    |hints| {
        let type_hints: Vec<_> = string_labels(hints)
            .into_iter()
            .filter(|label| label.starts_with(": "))
            .collect();
        assert!(type_hints.is_empty(), "got {type_hints:?}");
    }
);

inlay_hint_case!(
    hints_outside_the_viewport_are_not_returned,
    "function entrypoint() -> Nothing {\n  let a = 1\n  let b = 2\n  let c = 3\n}\n",
    Range {
        start: Position {
            line: 1,
            character: 0
        },
        end: Position {
            line: 2,
            character: 0
        },
    },
    |hints| {
        let lines: Vec<u32> = hints.iter().map(|hint| hint.position.line).collect();
        assert_eq!(
            lines,
            vec![1],
            "only the `let a` line is in view; got {lines:?}"
        );
    }
);

inlay_hint_case!(
    implicit_suspending_call_gets_a_wait_hint,
    "\
function doWork() -> Nothing {
  wait sleep(100)
}
function run() -> Nothing {
  doWork()
}
",
    full_range(),
    |hints| {
        let waits: Vec<_> = string_labels(hints)
            .into_iter()
            .filter(|label| label.trim_start().starts_with("wait"))
            .collect();
        assert!(!waits.is_empty(), "got {:?}", string_labels(hints));
    }
);

inlay_hint_case!(
    explicit_wait_gets_no_wait_hint,
    "\
function doWork() -> Nothing {
  wait sleep(100)
}
function run() -> Nothing {
  wait doWork()
}
",
    full_range(),
    |hints| {
        let waits: Vec<_> = string_labels(hints)
            .into_iter()
            .filter(|label| label.trim_start().starts_with("wait"))
            .collect();
        assert!(waits.is_empty(), "got {waits:?}");
    }
);

inlay_hint_case!(
    background_call_of_a_suspending_function_is_routed_to_the_io_pool,
    "\
function doWork() -> Nothing {
  wait sleep(100)
}
function run() -> Nothing {
  background doWork()
}
",
    full_range(),
    |hints| {
        let labels = string_labels(hints);
        assert!(
            labels.iter().any(|label| label.contains("I/O pool")),
            "got {labels:?}"
        );
    }
);

inlay_hint_case!(
    background_call_of_a_plain_function_is_routed_to_the_cpu_pool,
    "\
function helper() -> Nothing {
  print(`working`)
}
function run() -> Nothing {
  background helper()
}
",
    full_range(),
    |hints| {
        let labels = string_labels(hints);
        assert!(
            labels.iter().any(|label| label.contains("CPU pool")),
            "got {labels:?}"
        );
    }
);

// The shape below has 9 fields × 8 bytes = 72 bytes, over the 64-byte copy threshold.
inlay_hint_case!(
    background_copy_of_a_shape_over_64_bytes_gets_a_give_hint,
    r#"
shape LargeEvent {
    a: Int
    b: Int
    c: Int
    d: Int
    e: Int
    f: Int
    g: Int
    h: Int
    i: Int
}
function process(evt: LargeEvent) -> Nothing {
}
function entrypoint() -> Nothing {
    let event: LargeEvent = { a: 1, b: 2, c: 3, d: 4, e: 5, f: 6, g: 7, h: 8, i: 9 }
    background process(event.copy())
}
"#,
    full_range(),
    |hints| {
        let labels = string_labels(hints);
        assert!(
            labels.iter().any(|label| label.contains(".give")),
            "got {labels:?}"
        );
    }
);

inlay_hint_case!(
    background_copy_of_a_small_shape_gets_no_give_hint,
    r#"
shape SmallEvent { name: Int }
function process(evt: SmallEvent) -> Nothing {
}
function entrypoint() -> Nothing {
    let event: SmallEvent = { name: 1 }
    background process(event.copy())
}
"#,
    full_range(),
    |hints| {
        let labels = string_labels(hints);
        assert!(
            !labels.iter().any(|label| label.contains(".give")),
            "got {labels:?}"
        );
    }
);

// The handle form `let h = background f(v)` once recorded `Copy` unconditionally,
// so the hint said "copy" for a value the callee declares `give`.
inlay_hint_case!(
    background_handle_form_labels_a_given_argument_give_not_copy,
    "\
shape Scene {
  name: String
  width: Int
}
function render(give scene: Scene) -> Int {
  wait sleep(1)
  return scene.width
}
function run() -> Nothing {
  let scene: Scene = { name: `Three Rivers`, width: 6 }
  let h = background render(scene)
  let got = h.receive()
  print(got.or(0))
}
",
    full_range(),
    |hints| {
        let labels: Vec<String> = string_labels(hints)
            .into_iter()
            .map(|label| label.trim().to_string())
            .collect();
        assert!(labels.iter().any(|label| label == "give"), "got {labels:?}");
        assert!(
            !labels.iter().any(|label| label == "copy"),
            "got {labels:?}"
        );
    }
);

// The label asserts the LITERAL "64" on purpose: an independent oracle pinning the
// shipped default (see the parity test below).
inlay_hint_case!(
    default_capacity_channel_gets_a_64_hint_with_an_insert_edit_at_the_closing_paren,
    CHANNEL_DEFAULT_SRC,
    full_range(),
    |hints| {
        let capacity_hint = hints
            .iter()
            .find(|hint| matches!(&hint.label, InlayHintLabel::String(label) if label == "64"))
            .expect("a default-capacity channel must emit the plain `64` hint");
        // The closing `)` of the constructor call (line 1), not the type annotation.
        let constructor_line = CHANNEL_DEFAULT_SRC.lines().nth(1).unwrap();
        let closing_paren_column = constructor_line.rfind(')').unwrap() as u32;
        assert_eq!(
            capacity_hint.position,
            Position {
                line: 1,
                character: closing_paren_column
            }
        );
        // Clicking the hint inserts `64` there, producing `Channel<Int>(64)`.
        let edits = capacity_hint
            .text_edits
            .as_ref()
            .expect("click-to-make-explicit edit");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "64");
        assert_eq!(edits[0].range.start, capacity_hint.position);
        assert_eq!(
            edits[0].range.end, capacity_hint.position,
            "insert must be zero-width"
        );
    }
);

inlay_hint_case!(
    explicit_capacity_channel_gets_no_capacity_hint,
    "function entrypoint() -> Nothing {\n  let wire: Channel<Int> = Channel<Int>(8)\n}\n",
    full_range(),
    |hints| assert_eq!(capacity_hint_count(hints), 0)
);

inlay_hint_case!(
    channel_without_an_element_type_gets_no_capacity_hint,
    "function entrypoint() -> Nothing {\n  let wire = Channel()\n}\n",
    full_range(),
    |hints| assert_eq!(capacity_hint_count(hints), 0)
);

inlay_hint_case!(
    channel_with_two_type_arguments_gets_no_capacity_hint,
    "function entrypoint() -> Nothing {\n  let wire = Channel<Int, String>()\n}\n",
    full_range(),
    |hints| assert_eq!(capacity_hint_count(hints), 0)
);

inlay_hint_case!(
    each_member_of_a_two_spawn_group_gets_an_auto_arc_hint_naming_the_task_count,
    AUTO_ARC_GROUP_SRC,
    full_range(),
    |hints| {
        let labels = auto_arc_labels(hints);
        assert_eq!(labels.len(), 2, "got {:?}", string_labels(hints));
        assert!(
            labels.iter().all(|label| label.contains("with 2 tasks")),
            "got {labels:?}"
        );
    }
);

// One compiler decision, one annotation: the ownership walker yields to auto_arc.
inlay_hint_case!(
    auto_arc_group_member_arguments_get_no_copy_hint,
    AUTO_ARC_GROUP_SRC,
    full_range(),
    |hints| {
        let copies = string_labels(hints)
            .into_iter()
            .filter(|label| label.trim() == "copy")
            .count();
        assert_eq!(copies, 0);
    }
);

inlay_hint_case!(
    a_single_background_spawn_gets_no_auto_arc_hint,
    "\
shape Scene {
  name: String
  width: Int
}
function render(scene: Scene) -> Nothing {
  wait sleep(1)
  print(scene.name)
}
function run() -> Nothing {
  let scene: Scene = { name: `Three Rivers`, width: 6 }
  background render(scene)
  print(scene.width)
}
",
    full_range(),
    |hints| assert!(
        auto_arc_labels(hints).is_empty(),
        "got {:?}",
        string_labels(hints)
    )
);

// ─────────────────────────────────────────────────────────────────────────────
// Parity: registry hover prose vs. the values and wording it must agree with
// ─────────────────────────────────────────────────────────────────────────────

// bug: the channel_capacity hover teaches a capacity different from the authoritative default | tier: Low
#[test]
fn channel_capacity_registry_hover_states_the_authoritative_default() {
    // The registry hover text is TOML data and cannot read
    // `ynz_typeck::DEFAULT_CHANNEL_CAPACITY`, so this parity test is the link
    // authoritative-derivation.md requires between the two homes.
    let capacity = ynz_typeck::DEFAULT_CHANNEL_CAPACITY.to_string();
    let hover = ynz_registry::lsp_inlay_hint_hover_for("channel_capacity")
        .expect("channel_capacity domain must have registry hover text");
    assert!(hover.contains(&capacity), "hover was: {hover}");
}

macro_rules! ownership_hover_fragment_case {
    ($name:ident, $fragment:expr) => {
        // bug: the ownership_call_site hover stops saying what the ConsumedByStore diagnostic says (one wording per concept), or loses the `.copy()` fix | tier: Low
        #[test]
        fn $name() {
            let hover = ynz_registry::lsp_inlay_hint_hover_for("ownership_call_site")
                .expect("ownership_call_site domain must have registry hover text");
            let diagnostic = ynz_registry::diagnostic_template_lookup("ConsumedByStore")
                .expect("ConsumedByStore template present");
            assert!(
                diagnostic
                    .what_template
                    .contains("storing a value hands it over"),
                "the diagnostic's wording moved; update the hover with it"
            );
            assert!(hover.contains($fragment), "hover was: {hover}");
        }
    };
}

ownership_hover_fragment_case!(ownership_hover_names_the_handed_to_hint, "handed to");
ownership_hover_fragment_case!(
    ownership_hover_uses_the_diagnostics_store_wording,
    "storing a value hands it over"
);
ownership_hover_fragment_case!(ownership_hover_names_the_copy_fix, ".copy()");
ownership_hover_fragment_case!(ownership_hover_names_share, "share");
ownership_hover_fragment_case!(ownership_hover_names_give, "give");

// ─────────────────────────────────────────────────────────────────────────────
// Nightly: wall-clock budget
// ─────────────────────────────────────────────────────────────────────────────

// bug: inlay hints take over 30 ms for a viewport and stall the editor on every keystroke | tier: Medium
#[test]
fn nightly_inlay_hint_response_under_30ms_for_viewport() {
    let src = "function entrypoint() -> Nothing {\n  let x = 42\n  let y = 100\n}\n";
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
