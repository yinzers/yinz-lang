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

// Bug it catches: `[{{ a: 1 }}]` (a `format!` escape pasted into Yinz) made the array-literal
// recovery loop re-parse the same `{` forever, growing diagnostics until the process died.
// The parse runs on a thread so a regression fails by timeout instead of hanging the suite.
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

// Bug it catches: a rename tool treating `.map(...)`, `range(...)` or a variable named `string`
// as types; only the parser knows which words are types.
#[test]
fn only_words_the_parser_read_as_types_are_recorded() {
    let source = "\
shape Row { label: string, scores: map<string, int> }
function entrypoint() -> nothing {
  let string = `x`
  let wire: channel<int> = channel<int>(8)
  let rows = range(0, 3)
  let doubled = rows.map(string)
  print(int.max)
  if (wire is maybe) { }
}
";
    let found = find_type_positions("t.ynz", source);
    let type_names = texts(source, &found.positions.type_names);
    assert_eq!(
        type_names,
        ["string", "map", "string", "int", "nothing", "channel", "int", "int", "maybe"]
    );
    let callees = texts(source, &found.positions.call_callees);
    assert_eq!(callees, ["channel", "range", "print"]);
    let receivers: Vec<_> = found
        .positions
        .constant_read_receivers
        .iter()
        .map(|read| (read.receiver_name.as_str(), read.field_name.as_str()))
        .collect();
    assert_eq!(receivers, [("int", "max")]);
}

// Bug it catches: type names recorded by a speculative generic-call parse that was then
// abandoned (`a < b` is a comparison), which would rename an ordinary variable.
#[test]
fn abandoned_generic_call_attempts_record_no_type_names() {
    let source = "function f(a: Int, b: Int, c: Int) -> Boolean { return a < b }";
    let found = find_type_positions("t.ynz", source);
    assert_eq!(
        texts(source, &found.positions.type_names),
        ["Int", "Int", "Int", "Boolean"]
    );
}
