//! `find_type_positions` and the array-literal recovery regression.

use std::sync::mpsc;
use std::time::Duration;

use ynz_parser::find_type_positions;

fn texts<'a>(source: &'a str, spans: &[ynz_diagnostics::SourceSpan]) -> Vec<&'a str> {
    spans
        .iter()
        .map(|span| &source[span.start..span.end])
        .collect()
}

// `[{{ a: 1 }}]` (a `format!` escape pasted into Yinz) made the array-literal recovery
// loop re-parse the same `{` forever, growing diagnostics until the process died.
// The parse runs on a thread so a regression fails by timeout instead of hanging the suite.
// bug: array-literal recovery re-parses an element that consumed no token forever | tier: High
#[test]
fn array_literal_recovery_stops_when_an_element_consumes_no_token() {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let source = "function f() -> Int { let b = [{{ a: 1 }}] }";
        let found = find_type_positions("loop.ynz", source);
        let _ = sender.send(found.diagnostics.iter().count());
    });
    let diagnostic_count = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the parser must finish on `[{{ a: 1 }}]`");
    assert!(diagnostic_count > 0, "the malformed array is reported");
}

/// One case per source: the recorded type names are exactly the expected words, and,
/// where the case lists them, so are the call callees and constant-read receivers.
macro_rules! type_position_cases {
    ($($case:ident: $source:expr => type_names [$($type_name:expr),* $(,)?]
        $(, call_callees [$($callee:expr),* $(,)?])?
        $(, constant_read_receivers [$(($receiver:expr, $field:expr)),* $(,)?])?;)+) => {$(
        // bug: find_type_positions records a word the parser did not read as a type (a method, a variable named like a type, an abandoned speculative parse), so a rename tool rewrites an ordinary name | tier: Medium
        #[test]
        fn $case() {
            let source: &str = $source;
            let found = find_type_positions("t.ynz", source);
            assert_eq!(
                texts(source, &found.positions.type_names),
                [$($type_name),*]
            );
            $(
                assert_eq!(
                    texts(source, &found.positions.call_callees),
                    [$($callee),*]
                );
            )?
            $(
                let receivers: Vec<(&str, &str)> = found
                    .positions
                    .constant_read_receivers
                    .iter()
                    .map(|read| (read.receiver_name.as_str(), read.field_name.as_str()))
                    .collect();
                assert_eq!(receivers, [$(($receiver, $field)),*]);
            )?
        }
    )+};
}

type_position_cases! {
    only_words_the_parser_read_as_types_are_recorded: "\
shape Row { label: string, scores: map<string, int> }
function entrypoint() -> nothing {
  let string = `x`
  let wire: channel<int> = channel<int>(8)
  let rows = range(0, 3)
  let doubled = rows.map(string)
  print(int.max)
  if (wire is maybe) { }
}
" => type_names ["string", "map", "string", "int", "nothing", "channel", "int", "int", "maybe"],
    call_callees ["channel", "range", "print"],
    constant_read_receivers [("int", "max")];
    abandoned_generic_call_attempts_record_no_type_names:
        "function f(a: Int, b: Int, c: Int) -> Boolean { return a < b }"
        => type_names ["Int", "Int", "Int", "Boolean"];
}
