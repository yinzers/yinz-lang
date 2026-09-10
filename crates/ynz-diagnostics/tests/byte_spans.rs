// WHY: `SourceSpan` offsets are byte offsets; ariadne's default index type is char offsets.
// With the default, every multi-byte character before a span (an em dash or a box-drawing
// rule in a comment) pushed the rendered caret forward by the byte surplus — in the m8 error
// gallery every diagnostic landed 3–5 lines past its trigger, inside the NEXT function's
// `// WHY:` comment — and a span whose byte offset exceeded the file's char count was dropped
// by ariadne together with its WHAT-INSTEAD/WHY note. Both were one producer (v0.3-M8 Phase 4
// fix round 2). These tests were RED against `Config::default()` and are GREEN with
// `IndexType::Byte` + the past-EOF clamp.

use std::collections::HashMap;

use ynz_diagnostics::{render, Diagnostic, DiagnosticBucket, SourceSpan};

const FILE: &str = "gallery.ynz";

/// Two functions with comment lines between them that carry multi-byte characters (an em
/// dash, box-drawing rules, an arrow) — the shape of every `examples/primantis-orders`
/// gallery file.
const SOURCE: &str = "\
// ────── section one ──────
function first() -> nothing {
  print(`one`)
}

// WHY: the second function — its trigger is `wire.send(rows)` → the read afterward.
// ────── section two ──────
function second() -> nothing {
  let rows: array<int> = [1, 2, 3]
  wire.send(rows)
  print(rows.count())
}
";

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

#[test]
fn rendered_line_is_the_trigger_line_when_multi_byte_text_precedes_the_span() {
    let trigger = "rows.count()";
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::error(
        span_of(trigger),
        "`rows` was sent into `wire` and cannot be used here.",
        "Send a copy instead: `wire.send(rows.copy())`.",
        "A channel hands the value to whichever task receives it.",
    ));

    let output = render(&bucket, &sources(), false);
    let expected_line = line_of(trigger);
    let expected_col = SOURCE
        .lines()
        .nth(expected_line - 1)
        .unwrap()
        .find(trigger)
        .unwrap()
        + 1;
    assert!(
        output.contains(&format!("{FILE}:{expected_line}:{expected_col}")),
        "header must name the trigger's line:col ({expected_line}:{expected_col}); got:\n{output}"
    );
    // The excerpt shows the trigger line itself, never the following comment.
    assert!(
        output.contains(&format!(" {expected_line} │   print(rows.count())")),
        "excerpt must show the trigger line; got:\n{output}"
    );
    assert!(
        !output.contains("// WHY:"),
        "excerpt must not drift into a comment line; got:\n{output}"
    );
}

#[test]
fn a_span_past_the_end_of_the_file_still_renders_its_teaching_block() {
    let len = SOURCE.len();
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::error(
        SourceSpan::new(FILE, len + 3, len + 7),
        "`.close()` takes no arguments, but got 1.",
        "Call it bare: `wire.close()`.",
        "Closing a channel is a single act with nothing to configure.",
    ));

    let output = render(&bucket, &sources(), false);
    assert!(
        output.contains("Call it bare: `wire.close()`."),
        "WHAT-INSTEAD must render for a past-EOF span; got:\n{output}"
    );
    assert!(
        output.contains("Closing a channel is a single act"),
        "WHY must render for a past-EOF span; got:\n{output}"
    );
    // Pinned to the file's last line, not to `?:?`.
    let last_line = SOURCE.trim_end_matches('\n').matches('\n').count() + 1;
    assert!(
        output.contains(&format!("{FILE}:{last_line}:")),
        "header must name the last line, not `?:?`; got:\n{output}"
    );
    assert!(!output.contains("?:?"), "got:\n{output}");
}

#[test]
fn a_span_that_runs_past_the_end_is_trimmed_not_dropped() {
    let len = SOURCE.len();
    let start = SOURCE.rfind("print(").unwrap();
    let mut bucket = DiagnosticBucket::new();
    bucket.push(Diagnostic::error(
        SourceSpan::new(FILE, start, len + 10),
        "Something ran off the end.",
        "Trim the tail.",
        "The excerpt still shows where it started.",
    ));

    let output = render(&bucket, &sources(), false);
    assert!(output.contains("Trim the tail."), "got:\n{output}");
    assert!(output.contains("The excerpt still shows"), "got:\n{output}");
    assert!(
        output.contains(&format!("{FILE}:{}:", line_of("print(rows"))),
        "got:\n{output}"
    );
}
