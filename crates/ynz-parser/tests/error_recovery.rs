//! The parser must terminate on malformed input. A recovery loop that re-enters on a
//! token it never consumed hangs the whole process, so every case parses on its own
//! thread and fails by timeout instead of hanging CI.
//!
//! One generated table, one named test per source: the four reproducers of the
//! `parse_block` infinite loop and the two gallery files that reproducibly hung it
//! (a gallery case reads its file at run time; the harness is the same).

use std::sync::mpsc;
use std::time::Duration;

/// Generous for a debug build under load: the parses take milliseconds, so exceeding
/// this means a hang, not a slow machine.
const PARSE_DEADLINE: Duration = Duration::from_secs(5);

/// Parse `source` on a fresh thread and fail when it does not finish within
/// `PARSE_DEADLINE`. The thread of a hung parse is left running; the test process
/// ends right after the failed assertion.
fn assert_parse_terminates(file_name: String, source: String) {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let db = ynz_parser::CompilerDb::default();
        let source_file = ynz_parser::SourceFile::new(&db, file_name, source);
        let _ = ynz_parser::parse_query(&db, source_file);
        let _ = sender.send(());
    });
    receiver
        .recv_timeout(PARSE_DEADLINE)
        .expect("the parser hung: it did not finish within the deadline");
}

fn gallery_source(file_name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/primantis-orders")
        .join(file_name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|read_error| panic!("could not read {}: {read_error}", path.display()))
}

// WHY: Regression for the parse_block infinite-loop bug. Token::Function and
// Token::Options inside a function body caused parse_atom to return Expr::Error
// without consuming the token; parse_block re-entered the loop on the same token
// forever. Fixed by a forward-progress guarantee in parse_block's default arm.
macro_rules! terminating_parse_cases {
    ($($case:ident: $source:expr;)+) => {$(
        // bug: a parser recovery loop re-enters on a token it never consumed and hangs the process | tier: High
        #[test]
        fn $case() {
            assert_parse_terminates("test.ynz".to_string(), $source.to_string());
        }
    )+};
}

terminating_parse_cases! {
    function_declared_inside_function_body_terminates:
        "function outer() -> Nothing {\n  function inner() -> Nothing { }\n}";
    options_declared_inside_function_body_terminates:
        "function test() -> Nothing {\n  options Foo { a, b }\n}";
    function_declared_inside_nested_block_terminates:
        "function test() -> Nothing {\n  if (true) {\n    function nested() -> Nothing { }\n  }\n}";
    banned_async_keyword_before_inner_function_terminates:
        "function asyncExample() -> Nothing {\n  async function inner() -> Nothing { }\n}";
}

terminating_parse_cases! {
    gallery_m1_errors_terminates: gallery_source("m1_errors.ynz");
    gallery_v0_2_m1_errors_terminates: gallery_source("v0_2_m1_errors.ynz");
}
