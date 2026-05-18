// WHY: Three-part format is mandatory per Golden Rule 11. Snapshots catch silent
// regression to one-line error strings where the "What to do instead" or "Why"
// sections are dropped.

use std::collections::HashMap;

use insta::assert_snapshot;
use ynz_diagnostics::{render, Diagnostic, DiagnosticBucket, SourceSpan};

const SOURCE: &str = "function entrypoint() -> nothing {\n    print(42)\n}\n";
const FILE: &str = "test.ynz";

fn sources() -> HashMap<String, String> {
    [(FILE.to_string(), SOURCE.to_string())].into()
}

fn span(start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(FILE, start, end)
}


#[test]
fn single_error_with_span() {
    // WHY: verifies that a single error renders with all three message parts and
    // the caret points to the correct source position.
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::error(
        span(36, 38),
        "`print` was given a number, but it expects a string.",
        "Add quotes: print(\"42\")",
        "`print` always writes text. Numbers have to be turned into text first.",
    ));

    let output = render(&bucket, &sources(), false);
    assert_snapshot!("single_error_with_span", output);
}


#[test]
fn two_errors_same_file() {
    // WHY: verifies that multiple errors in one file are all rendered, not just
    // the first one. Regression guard for an early-bail bug in the render loop.
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


#[test]
fn error_and_warning() {
    // WHY: verifies that Error and Warning severity both render, and that errors
    // appear before warnings in the output regardless of insertion order.
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


#[test]
fn suggestion_only() {
    // WHY: Suggestion tier is reserved for v0.4 lint rules, but the tier must
    // exist and render from day 1 so later phases can use it. This test prevents
    // a "tier was never wired up" regression.
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::suggestion(
        span(0, 8),
        "All fields in this type are known at compile time.",
        "Consider using a `type` instead of a `map` for direct field access.",
        "Type field access is a single memory lookup. Map access requires a hash lookup.",
    ));

    let output = render(&bucket, &sources(), false);
    assert_snapshot!("suggestion_only", output);
}


#[test]
fn sixty_errors_capped_at_fifty() {
    // WHY: the 50-error cap prevents a broken file from flooding the terminal
    // with thousands of diagnostics. The footer tells the user how many were
    // hidden so they know there is more to fix.
    let mut bucket = DiagnosticBucket::new();
    for i in 0..60 {
        bucket.push(Diagnostic::error(
            span(0, 1),
            format!("Error number {i}."),
            "Fix it.",
            "Because it is wrong.",
        ));
    }

    assert!(
        !bucket.has_errors() || bucket.hidden_count() == 10,
        "Expected 10 hidden errors, got {}",
        bucket.hidden_count()
    );
    assert_eq!(bucket.hidden_count(), 10, "Should have 10 hidden errors");
    assert_eq!(bucket.len(), 50, "Bucket should hold exactly 50 errors");

    let output = render(&bucket, &sources(), false);
    assert!(
        output.contains("... and 10 more errors hidden"),
        "Expected footer line in output:\n{output}"
    );
    assert_snapshot!("sixty_errors_capped_at_fifty", output);
}


#[test]
#[should_panic(expected = "what must not be empty")]
fn empty_what_panics() {
    // WHY: empty "what" would leave the user with no description of the problem.
    // The constructor panic prevents this at the call site, not at display time.
    Diagnostic::error(span(0, 1), "", "fix it", "because");
}

#[test]
#[should_panic(expected = "what_instead must not be empty")]
fn empty_what_instead_panics() {
    // WHY: a diagnostic without a "what to do instead" leaves the user stuck.
    Diagnostic::error(span(0, 1), "problem", "", "because");
}

#[test]
#[should_panic(expected = "why must not be empty")]
fn empty_why_panics() {
    // WHY: a diagnostic without a "why" trains users to apply fixes without
    // understanding, which is exactly the failure mode the teaching mission rejects.
    Diagnostic::error(span(0, 1), "problem", "fix it", "");
}
