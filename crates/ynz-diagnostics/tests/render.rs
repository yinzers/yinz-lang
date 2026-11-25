// WHY: the three-part format (what, what instead, why) is mandatory per Golden Rule 3, so
// `render` must show every part of every diagnostic it was given, at the right place.
//
// `SourceSpan` offsets are byte offsets; ariadne's default index type is char offsets.
// With the default, every multi-byte character before a span (an em dash or a box-drawing
// rule in a comment) pushed the rendered caret forward by the byte surplus — in the m8 error
// gallery every diagnostic landed 3–5 lines past its trigger, inside the NEXT function's
// `// WHY:` comment — and a span whose byte offset exceeded the file's char count was dropped
// by ariadne together with its WHAT-INSTEAD/WHY note. Both were one producer (v0.3-M8 Phase 4
// fix round 2). The byte-offset cases were RED against `Config::default()` and are GREEN with
// `IndexType::Byte` + the past-EOF clamp.
//
// One table, one named test per case so nextest runs them in parallel. Wording and the look
// of the rendering are not asserted: they are presentation.

use std::collections::HashMap;

use ynz_diagnostics::{render, Diagnostic, DiagnosticBucket, SourceSpan};

const FILE: &str = "gallery.ynz";

/// Two functions with comment lines between them that carry multi-byte characters (an em
/// dash, box-drawing rules, an arrow) — the shape of every `examples/primantis-orders`
/// gallery file.
const SOURCE: &str = "\
// ────── section one ──────
function first() -> Nothing {
  print(`one`)
}

// WHY: the second function — its trigger is `wire.send(rows)` → the read afterward.
// ────── section two ──────
function second() -> Nothing {
  let rows: Array<Int> = [1, 2, 3]
  wire.send(rows)
  print(rows.count())
}
";

const TRIGGER: &str = "rows.count()";

fn sources() -> HashMap<String, String> {
    [(FILE.to_string(), SOURCE.to_string())].into()
}

/// Byte span of the first occurrence of `needle` in `SOURCE`.
fn span_of(needle: &str) -> SourceSpan {
    let start = SOURCE.find(needle).expect("needle present");
    SourceSpan::new(FILE, start, start + needle.len())
}

fn line_of(needle: &str) -> usize {
    let start = SOURCE.find(needle).expect("needle present");
    SOURCE[..start].matches('\n').count() + 1
}

fn bucket_of(diagnostics: Vec<Diagnostic>) -> DiagnosticBucket {
    let mut bucket = DiagnosticBucket::new();
    for diagnostic in diagnostics {
        bucket.push(diagnostic);
    }
    bucket
}

fn trigger_error() -> Diagnostic {
    Diagnostic::error(
        span_of(TRIGGER),
        "`rows` was sent into `wire` and cannot be used here.",
        "Send a copy instead: `wire.send(rows.copy())`.",
        "A channel hands the value to whichever task receives it.",
    )
}

fn first_function_error() -> Diagnostic {
    Diagnostic::error(
        span_of("function first"),
        "`first` is declared twice.",
        "Rename one of them.",
        "Two functions cannot share a name.",
    )
}

fn warning() -> Diagnostic {
    Diagnostic::warning(
        span_of("function first"),
        "This function has no doc comment.",
        "Add `/// What this function does` above the `function` keyword.",
        "Doc comments are how `ynz doc` generates documentation.",
    )
}

fn two_errors() -> DiagnosticBucket {
    bucket_of(vec![trigger_error(), first_function_error()])
}

fn warning_pushed_before_error() -> DiagnosticBucket {
    bucket_of(vec![warning(), trigger_error()])
}

fn sixty_errors() -> DiagnosticBucket {
    bucket_of(
        (0..60)
            .map(|error_number| {
                Diagnostic::error(
                    span_of(TRIGGER),
                    format!("Error number {error_number}."),
                    "Fix it.",
                    "Because it is wrong.",
                )
            })
            .collect(),
    )
}

fn span_past_the_end() -> DiagnosticBucket {
    let len = SOURCE.len();
    bucket_of(vec![Diagnostic::error(
        SourceSpan::new(FILE, len + 3, len + 7),
        "`.close()` takes no arguments, but got 1.",
        "Call it bare: `wire.close()`.",
        "Closing a channel is a single act with nothing to configure.",
    )])
}

fn span_running_past_the_end() -> DiagnosticBucket {
    let start = SOURCE.rfind("print(").unwrap();
    bucket_of(vec![Diagnostic::error(
        SourceSpan::new(FILE, start, SOURCE.len() + 10),
        "Something ran off the end.",
        "Trim the tail.",
        "The excerpt still shows where it started.",
    )])
}

/// The `file:line:col` header the renderer must print for `TRIGGER`.
fn trigger_header() -> String {
    let line = line_of(TRIGGER);
    let column = SOURCE.lines().nth(line - 1).unwrap().find(TRIGGER).unwrap() + 1;
    format!("{FILE}:{line}:{column}")
}

/// One case per rendering contract: the bucket renders every `shows` text, none of the
/// `hides` texts, and the `in_order` texts in that order.
macro_rules! render_cases {
    ($($case:ident: $bucket:expr => shows [$($shown:expr),+ $(,)?]
        $(, hides [$($hidden:expr),+ $(,)?])?
        $(, in_order [$first:expr, $second:expr])?;)+) => {$(
        // bug: render drops a diagnostic or one of its three parts, mis-orders warnings before errors, floods past the 50-error cap, or (spans are byte offsets) puts the caret on the wrong line or drops a span past the end of the file | tier: Medium
        #[test]
        fn $case() {
            let output = render(&$bucket, &sources(), false);
            $(
                let shown: String = ($shown).to_string();
                assert!(output.contains(&shown), "missing {shown:?} in:\n{output}");
            )+
            $($(
                let hidden: String = ($hidden).to_string();
                assert!(!output.contains(&hidden), "unexpected {hidden:?} in:\n{output}");
            )+)?
            $(
                let first: String = ($first).to_string();
                let second: String = ($second).to_string();
                let first_position = output.find(&first).expect("first text rendered");
                let second_position = output.find(&second).expect("second text rendered");
                assert!(
                    first_position < second_position,
                    "{first:?} must render before {second:?} in:\n{output}"
                );
            )?
        }
    )+};
}

render_cases! {
    every_error_renders_all_three_parts: two_errors() => shows [
        "`rows` was sent into `wire` and cannot be used here.",
        "Send a copy instead: `wire.send(rows.copy())`.",
        "A channel hands the value to whichever task receives it.",
        "`first` is declared twice.",
        "Rename one of them.",
        "Two functions cannot share a name.",
    ];
    // Insertion order is warning, error; the user must see the error first.
    errors_render_before_warnings: warning_pushed_before_error() => shows [
        "This function has no doc comment.",
        "Add `/// What this function does` above the `function` keyword.",
    ], in_order ["`rows` was sent into `wire`", "This function has no doc comment."];
    // The cap keeps a broken file from flooding the terminal; the footer says how many hid.
    errors_past_fifty_are_hidden_and_counted: sixty_errors() => shows [
        "Error number 49.",
        "... and 10 more errors hidden",
    ], hides ["Error number 50."];
    // Byte-offset bug: multi-byte comments before the span must not push the caret down.
    caret_is_on_the_trigger_line_after_multi_byte_text: bucket_of(vec![trigger_error()]) => shows [
        trigger_header(),
        format!(" {} │   print(rows.count())", line_of(TRIGGER)),
    ], hides ["// WHY:"];
    // A span starting past the file's end must still show its teaching block, pinned to the last line.
    span_past_the_end_of_the_file_still_renders_its_teaching_block: span_past_the_end() => shows [
        "Call it bare: `wire.close()`.",
        "Closing a channel is a single act",
        format!("{FILE}:{}:", SOURCE.trim_end_matches('\n').matches('\n').count() + 1),
    ], hides ["?:?"];
    // A span that starts inside the file but ends past it is trimmed, not dropped.
    span_running_past_the_end_is_trimmed_not_dropped: span_running_past_the_end() => shows [
        "Trim the tail.",
        "The excerpt still shows",
        format!("{FILE}:{}:", line_of("print(rows")),
    ];
}
