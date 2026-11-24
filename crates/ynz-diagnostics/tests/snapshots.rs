// WHY: Three-part format is mandatory per Golden Rule 3. The two snapshots show a rendered
// bucket whole, so a render that drops the "What to do instead" or "Why" section, or drops a
// diagnostic, fails here. The constructor tests keep an empty part from ever being built.
//
// Wording of individual messages and the look of the rendering are not asserted per message:
// they are presentation.

use std::collections::HashMap;

use insta::assert_snapshot;
use ynz_diagnostics::{render, Diagnostic, DiagnosticBucket, SourceSpan};

const SOURCE: &str = "function entrypoint() -> Nothing {\n    print(42)\n}\n";
const FILE: &str = "test.ynz";

fn sources() -> HashMap<String, String> {
    [(FILE.to_string(), SOURCE.to_string())].into()
}

fn span(start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(FILE, start, end)
}

// WHY: verifies that multiple errors in one file are all rendered, not just
// the first one. Regression guard for an early-bail bug in the render loop.
// bug: the render loop stops after the first diagnostic or drops its what-instead/why sections, so the user never sees the other errors | tier: Medium
#[test]
fn two_errors_same_file() {
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::error(
        span(36, 38),
        "`print` was given a number, but it expects a string.",
        "Add quotes: print(\"42\")",
        "`print` always writes text. Numbers have to be turned into text first.",
    ));
    bucket.push(Diagnostic::error(
        span(0, 8),
        "`function` keyword is misspelled here.",
        "Use `function` (all lowercase).",
        "Keywords in Yinz are always lowercase.",
    ));

    let output = render(&bucket, &sources(), false);
    assert_snapshot!("two_errors_same_file", output);
}

// WHY: verifies that Error and Warning severity both render, and that errors
// appear before warnings in the output regardless of insertion order.
// bug: a warning is dropped, or rendered before the errors the user must fix first | tier: Medium
#[test]
fn error_and_warning() {
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::warning(
        span(0, 8),
        "This function has no doc comment.",
        "Add `/// What this function does` above the `function` keyword.",
        "Doc comments are how `ynz doc` generates documentation.",
    ));
    bucket.push(Diagnostic::error(
        span(36, 38),
        "`print` was given a number, but it expects a string.",
        "Add quotes: print(\"42\")",
        "`print` always writes text. Numbers have to be turned into text first.",
    ));

    let output = render(&bucket, &sources(), false);
    assert_snapshot!("error_and_warning", output);
}

// WHY: the 50-error cap prevents a broken file from flooding the terminal
// with thousands of diagnostics. The footer tells the user how many were
// hidden so they know there is more to fix.
// bug: more than 50 diagnostics flood the terminal, or the hidden count is wrong or missing from the footer | tier: Medium
#[test]
fn sixty_errors_capped_at_fifty() {
    let mut bucket = DiagnosticBucket::new();
    for error_number in 0..60 {
        bucket.push(Diagnostic::error(
            span(0, 1),
            format!("Error number {error_number}."),
            "Fix it.",
            "Because it is wrong.",
        ));
    }

    assert_eq!(bucket.hidden_count(), 10, "Should have 10 hidden errors");
    assert_eq!(bucket.len(), 50, "Bucket should hold exactly 50 errors");

    let output = render(&bucket, &sources(), false);
    assert!(
        output.contains("... and 10 more errors hidden"),
        "Expected footer line in output:\n{output}"
    );
}

/// One case per part of the three-part message: building a diagnostic with that part
/// empty panics at the call site, not at display time.
macro_rules! empty_part_cases {
    ($($case:ident: ($what:expr, $what_instead:expr, $why:expr) panics $message:literal;)+) => {$(
        // bug: a diagnostic with an empty what, what-instead or why can be built, leaving the user without the teaching text Golden Rule 3 requires | tier: High
        #[test]
        #[should_panic(expected = $message)]
        fn $case() {
            Diagnostic::error(span(0, 1), $what, $what_instead, $why);
        }
    )+};
}

empty_part_cases! {
    empty_what_panics: ("", "fix it", "because") panics "what must not be empty";
    empty_what_instead_panics: ("problem", "", "because") panics "what_instead must not be empty";
    empty_why_panics: ("problem", "fix it", "") panics "why must not be empty";
}
