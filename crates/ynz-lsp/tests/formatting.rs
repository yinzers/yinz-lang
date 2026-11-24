// WHY: Formatting tests verify that format-on-save is idempotent (no edit when
// already canonical), degrades gracefully on parse errors (empty edit + message),
// and that range formatting never panics on a rejected range.

use lsp_types::{notification::Notification as _, Position, Range};
use ynz_lsp::{
    capabilities::PositionEncoding,
    formatting::{formatting_response, range_formatting_response},
    state::ServerState,
};

const DIRTY_SOURCE: &str = "function entrypoint() -> Nothing {\n  let x   =  42\n}\n";
const BROKEN_SOURCE: &str = "function broken( -> Nothing {}\n"; // missing param closing paren

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn whole_file_range(end_line: u32) -> Range {
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: end_line,
            character: 0,
        },
    }
}

/// Method names of every notification the handler sent to the client.
fn notification_methods(
    receiver: &crossbeam_channel::Receiver<lsp_server::Message>,
) -> Vec<String> {
    receiver
        .try_iter()
        .filter_map(|message| match message {
            lsp_server::Message::Notification(notification) => Some(notification.method),
            _ => None,
        })
        .collect()
}

// bug: format-on-save returns edits for a canonical file (endless format loop), none for a dirty file, or no edits and no showMessage for a file with parse errors | tier: Medium
macro_rules! formatting_case {
    ($name:ident, $source:expr, $expected:ident) => {
        #[test]
        fn $name() {
            let (state, uri) =
                state_single(concat!("/tmp/ynz_fmt_", stringify!($name), ".ynz"), $source);
            let (sender, receiver) = crossbeam_channel::unbounded();
            let edits = formatting_response(&state, &uri, &sender);
            let methods = notification_methods(&receiver);
            match stringify!($expected) {
                "one_edit_that_formats_to_a_fixed_point" => {
                    assert_eq!(edits.len(), 1, "a dirty file gets one edit; got {edits:?}");
                    // A second pass over the formatted text must change nothing.
                    let (formatted_state, formatted_uri) = state_single(
                        concat!("/tmp/ynz_fmt_", stringify!($name), "_again.ynz"),
                        &edits[0].new_text,
                    );
                    let second_pass =
                        formatting_response(&formatted_state, &formatted_uri, &sender);
                    assert!(second_pass.is_empty(), "format must be idempotent");
                }
                "no_edit" => assert!(edits.is_empty(), "got {edits:?}"),
                "no_edit_and_a_message" => {
                    assert!(edits.is_empty(), "got {edits:?}");
                    assert!(
                        methods.contains(&lsp_types::notification::ShowMessage::METHOD.to_string()),
                        "parse-error path must emit window/showMessage; got {methods:?}"
                    );
                }
                other => panic!("unknown expectation {other}"),
            }
        }
    };
}

formatting_case!(
    dirty_file_gets_one_edit_that_formats_to_a_fixed_point,
    DIRTY_SOURCE,
    one_edit_that_formats_to_a_fixed_point
);
formatting_case!(
    canonical_file_gets_no_edit,
    "function entrypoint() -> Nothing {\n  let x = 42\n}\n",
    no_edit
);
formatting_case!(
    file_with_parse_error_gets_no_edit_and_a_message,
    BROKEN_SOURCE,
    no_edit_and_a_message
);

// bug: range formatting panics on an inverted range or returns edits for a canonical or unparseable file, or says nothing about a file it cannot format | tier: Medium
macro_rules! range_formatting_case {
    ($name:ident, $source:expr, $range:expr, $expect_message:expr) => {
        #[test]
        fn $name() {
            let (state, uri) =
                state_single(concat!("/tmp/ynz_fmt_", stringify!($name), ".ynz"), $source);
            let (sender, receiver) = crossbeam_channel::unbounded();
            let edits = range_formatting_response(&state, &uri, $range, &sender);
            assert!(edits.is_empty(), "got {edits:?}");
            let methods = notification_methods(&receiver);
            assert_eq!(
                methods.contains(&"window/showMessage".to_string()),
                $expect_message,
                "notifications: {methods:?}"
            );
        }
    };
}

range_formatting_case!(
    canonical_range_gets_no_edit,
    "function entrypoint() -> Nothing {\n  let x = 42\n}\n",
    whole_file_range(2),
    false
);
range_formatting_case!(
    range_in_file_with_parse_error_gets_no_edit_and_a_message,
    BROKEN_SOURCE,
    whole_file_range(1),
    true
);
// The source must be non-canonical: a canonical one short-circuits before the
// byte slice that panics on an inverted range.
range_formatting_case!(
    inverted_range_gets_no_edit_and_a_message,
    DIRTY_SOURCE,
    Range {
        start: Position {
            line: 2,
            character: 0
        },
        end: Position {
            line: 0,
            character: 0
        },
    },
    true
);

// bug: format-on-save takes over 50 ms and stalls the editor | tier: Medium
#[test]
fn nightly_formatting_response_under_50ms() {
    let (state, uri) = state_single(
        "/tmp/ynz_fmt_perf.ynz",
        "function entrypoint() -> Nothing {\n  let x = 42\n}\n",
    );
    let (sender, _receiver) = crossbeam_channel::unbounded();
    let start = std::time::Instant::now();
    let _ = formatting_response(&state, &uri, &sender);
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "formatting took {}ms, expected < 50ms",
        elapsed.as_millis()
    );
}
