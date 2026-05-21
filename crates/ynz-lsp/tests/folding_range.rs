// WHY: Folding range tests verify that the LSP handler produces correct
// collapsible regions for functions, shapes, and options declarations.
// Bugs here silently break code folding in the editor — the user loses
// the ability to collapse large declarations, with no error to diagnose.

use lsp_types::FoldingRangeKind;
use ynz_lsp::{capabilities::PositionEncoding, folding_range::folding_range_response, state::ServerState};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

#[test]
fn function_body_fold_spans_multiple_lines() {
    // WHY: A multi-line function body must produce a FoldingRange covering at
    // least lines 0-2.  If no range is returned the editor cannot fold the
    // function.  If the range is zero-height clients handle it inconsistently.
    let src = "function entrypoint() -> nothing {\n  let x = 42\n}\n";
    let (state, uri) = state_single("/tmp/fr_test_fn.ynz", src);
    let ranges = folding_range_response(&state, &uri);
    assert_eq!(ranges.len(), 1, "one function → one fold; got: {ranges:?}");
    let fr = &ranges[0];
    assert_eq!(fr.kind, Some(FoldingRangeKind::Region));
    // The body block must span at least two lines.
    assert!(
        fr.start_line < fr.end_line,
        "fold must cover multiple lines; start={} end={}",
        fr.start_line,
        fr.end_line,
    );
    // The closing `}` is on line 2 (0-indexed).
    assert_eq!(fr.end_line, 2, "closing brace must be on line 2");
}

#[test]
fn shape_decl_produces_fold() {
    // WHY: A multi-line shape declaration is a common fold candidate — users
    // collapse shapes to see calling code more clearly.  Missing this fold
    // leaves a visible gap in editor UX.
    let src = "shape Player {\n  name: string\n  health: int\n}\n";
    let (state, uri) = state_single("/tmp/fr_test_shape.ynz", src);
    let ranges = folding_range_response(&state, &uri);
    assert_eq!(ranges.len(), 1, "one shape → one fold; got: {ranges:?}");
    assert!(
        ranges[0].start_line < ranges[0].end_line,
        "shape fold must span multiple lines"
    );
}

#[test]
fn options_decl_produces_fold() {
    // WHY: Options declarations follow the same multi-line fold pattern as
    // shapes. Skipping OptionsDecl items silently removes folding for all
    // options types without any diagnostic or compile error.
    let src = "options Status {\n  active\n  inactive\n  banned\n}\n";
    let (state, uri) = state_single("/tmp/fr_test_options.ynz", src);
    let ranges = folding_range_response(&state, &uri);
    assert_eq!(ranges.len(), 1, "one options → one fold; got: {ranges:?}");
    assert!(
        ranges[0].start_line < ranges[0].end_line,
        "options fold must span multiple lines"
    );
}

#[test]
fn empty_file_returns_no_folds() {
    // WHY: The empty-file edge case must not panic.  Off-by-one bugs in line
    // table conversion code frequently surface here — an empty file has no
    // bytes and should produce zero folds cleanly.
    let src = "";
    let (state, uri) = state_single("/tmp/fr_test_empty.ynz", src);
    let ranges = folding_range_response(&state, &uri);
    assert!(ranges.is_empty(), "empty file → no folds; got: {ranges:?}");
}

#[test]
fn multiple_declarations_produce_multiple_folds() {
    // WHY: A real file has many declarations. The handler must produce one fold
    // per foldable item, not just the first one it encounters.  A short-circuit
    // bug would leave all but the first declaration un-foldable.
    let src = concat!(
        "shape Player {\n  name: string\n  health: int\n}\n",
        "options Status {\n  active\n  inactive\n}\n",
        "function greet(p: Player) -> nothing {\n  print(`hi`)\n}\n",
    );
    let (state, uri) = state_single("/tmp/fr_test_multi.ynz", src);
    let ranges = folding_range_response(&state, &uri);
    assert_eq!(
        ranges.len(),
        3,
        "one shape + one options + one function = 3 folds; got: {ranges:?}"
    );
    // All folds must use the Region kind.
    for fr in &ranges {
        assert_eq!(fr.kind, Some(FoldingRangeKind::Region), "all folds use Region kind");
    }
    // Ranges must be non-overlapping and in source order.
    for window in ranges.windows(2) {
        assert!(
            window[0].end_line <= window[1].start_line,
            "folds must be non-overlapping and in source order; {:?}",
            ranges
        );
    }
}
