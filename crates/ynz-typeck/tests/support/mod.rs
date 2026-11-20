// Helpers shared by the typeck tests that check ownership programs (`transfer_record.rs`,
// `revive_paths.rs`): one place for running a program, reading its errors, the use-after-give
// assertions and the one-named-test-per-program macro, so the two files cannot drift apart.

// Each test crate compiles this module on its own and uses a different subset of the helpers, so
// a helper one of them leaves unused is not dead code.
#![allow(dead_code)]

use ynz_diagnostics::{Diagnostic, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{check_query, CheckOutput};

/// A callee that takes ownership of its argument (`give`), for programs that give a binding away.
pub const EAT: &str = "function eat(give rows: Array<Int>) -> Nothing {
  print(rows.count().toString())
}\n";

pub fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    // Every program needs an `entrypoint`; the cases under test are the other functions.
    let program = format!("{source}\nfunction entrypoint() -> Nothing {{\n}}\n");
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), program.clone());
    let output = (*check_query(&db, source_file)).clone();
    // Every program checked here must leave a whole transfer record (a hole is a checker bug).
    assert_eq!(
        output.typed_module.transfers.internal_errors(),
        Vec::<String>::new(),
        "the transfer record has holes for:\n{program}"
    );
    output
}

pub fn error_diagnostics(output: &CheckOutput) -> Vec<&Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .collect()
}

pub fn errors(output: &CheckOutput) -> Vec<String> {
    error_diagnostics(output)
        .into_iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect()
}

pub fn assert_accepted(source: &str) {
    let found = errors(&run(source));
    assert!(found.is_empty(), "expected accepted, got {found:#?}");
}

/// The program must be rejected with the use-after-give text.
pub fn assert_use_after_give(source: &str) {
    let found = errors(&run(source));
    assert!(
        found
            .iter()
            .any(|message| message.contains("was already given away")),
        "expected a use-after-give for:\n{source}\ngot {found:#?}"
    );
}

/// One named test per program: `cases!(check; name: source, "bug it catches (High)";)` expands to
/// `#[test] fn name() { check(&source) }` with the bug line as its doc.
#[allow(unused_macros)]
macro_rules! cases {
    ($check:ident; $($case:ident: $source:expr, $bug:literal;)+) => {
        $(
            #[doc = $bug]
            #[test]
            fn $case() {
                $check(&$source);
            }
        )+
    };
}

/// The program must be rejected with an error that contains `needle`.
pub fn assert_rejected_with(source: &str, needle: &str) {
    let found = errors(&run(source));
    assert!(
        found.iter().any(|message| message.contains(needle)),
        "expected an error containing {needle:?} for:\n{source}\ngot {found:#?}"
    );
}
