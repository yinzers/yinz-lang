// WHY: `array` → `fixed` and `let` → `const` are Replacement-category promotions:
// the compiler already chose the stricter form, and the editor shows that on the
// token the user wrote (semantic-token modifier `promotable`), explains it on hover,
// and rewrites it through a `refactor.rewrite` code action. Each case is driven
// through the JSON-RPC boundary, because the marker, hover, action and the absent
// end-of-line hint are four surfaces of ONE decision and a regression in any one
// (wrong token marked, edit span off by a byte, hover text from the plain keyword,
// stale inlay hint) leaves the others looking fine. Medium tier.

mod harness;
use harness::{HarnessClient, InProcessHarness};
use serde_json::{json, Value};

const URI: &str = "file:///replacement_marker.ynz";

/// `promotable` is index 0 of the server's modifier legend, so its bit is 1.
const PROMOTABLE_BIT: u64 = 1;

fn open_document(src: &str) -> HarnessClient {
    let client = InProcessHarness::new().start_server();
    client.initialize();
    client.did_open(URI, src);
    client
}

/// (line, character) of the first `needle` after the first `anchor` in `src`.
fn position_of(src: &str, anchor: &str, needle: &str) -> (usize, usize) {
    let anchor_at = src.find(anchor).expect("anchor in src");
    let needle_at = anchor_at + src[anchor_at..].find(needle).expect("needle after anchor");
    let line = src[..needle_at].matches('\n').count();
    let line_start = src[..needle_at]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    (line, needle_at - line_start)
}

/// Modifier bitset of the semantic token that starts at (`line`, `character`).
fn modifiers_at(client: &HarnessClient, line: usize, character: usize) -> Option<u64> {
    let tokens = client.request(
        10,
        "textDocument/semanticTokens/full",
        json!({ "textDocument": { "uri": URI } }),
    );
    let data: Vec<u64> = tokens["data"]
        .as_array()
        .expect("semantic tokens data")
        .iter()
        .map(|value| value.as_u64().expect("u32"))
        .collect();
    let (mut token_line, mut token_start) = (0usize, 0usize);
    for token in data.chunks(5) {
        token_line += token[0] as usize;
        token_start = if token[0] == 0 {
            token_start + token[1] as usize
        } else {
            token[1] as usize
        };
        if (token_line, token_start) == (line, character) {
            return Some(token[4]);
        }
    }
    None
}

fn is_marked(client: &HarnessClient, line: usize, character: usize) -> bool {
    let modifiers = modifiers_at(client, line, character)
        .unwrap_or_else(|| panic!("no semantic token at {line}:{character}"));
    modifiers & PROMOTABLE_BIT != 0
}

fn hover_text_at(client: &HarnessClient, line: usize, character: usize) -> String {
    let hover = client.request(
        11,
        "textDocument/hover",
        json!({
            "textDocument": { "uri": URI },
            "position": { "line": line, "character": character }
        }),
    );
    hover["contents"]["value"]
        .as_str()
        .unwrap_or_else(|| panic!("hover returned no markdown: {hover}"))
        .to_string()
}

/// `refactor.rewrite` code actions offered for a cursor at (`line`, `character`).
fn rewrite_actions_at(client: &HarnessClient, line: usize, character: usize) -> Vec<Value> {
    let cursor = json!({ "line": line, "character": character });
    let actions = client.request(
        12,
        "textDocument/codeAction",
        json!({
            "textDocument": { "uri": URI },
            "range": { "start": cursor, "end": cursor },
            "context": { "diagnostics": [] }
        }),
    );
    actions
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|action| action["kind"] == "refactor.rewrite")
        .collect()
}

/// Apply the single edit of `action` to `src` (single-byte characters only).
fn apply_edit(src: &str, action: &Value) -> String {
    let edits = action["edit"]["changes"][URI]
        .as_array()
        .expect("edits for the document");
    assert_eq!(edits.len(), 1, "one rewrite per action: {edits:?}");
    let offset_of = |position: &Value| -> usize {
        let line = position["line"].as_u64().unwrap() as usize;
        let character = position["character"].as_u64().unwrap() as usize;
        let line_start: usize = src.split_inclusive('\n').take(line).map(str::len).sum();
        line_start + character
    };
    let start = offset_of(&edits[0]["range"]["start"]);
    let end = offset_of(&edits[0]["range"]["end"]);
    format!(
        "{}{}{}",
        &src[..start],
        edits[0]["newText"].as_str().unwrap(),
        &src[end..]
    )
}

/// The Replacement domains render no inlay hint any more: none is an end-of-line
/// `//` comment and none carries either domain's hover tooltip.
fn assert_no_promotion_inlay_hint(client: &HarnessClient) {
    let hints = client.request(
        13,
        "textDocument/inlayHint",
        json!({
            "textDocument": { "uri": URI },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 999, "character": 0 } }
        }),
    );
    for hint in hints.as_array().cloned().unwrap_or_default() {
        let label = hint["label"].as_str().unwrap_or_default();
        let tooltip = hint["tooltip"]["value"].as_str().unwrap_or_default();
        assert!(
            !label.starts_with("//")
                && !tooltip.contains("ever grows it")
                && !tooltip.contains("ever reassigns it"),
            "promotion must not render as an inlay hint, got: {hint}"
        );
    }
}

#[test]
fn array_to_fixed_marks_array_token_with_hover_and_rewrite_action() {
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let scores: array<int> = [1, 2, 3]\n",
        "}\n",
    );
    let client = open_document(src);
    let (line, array_column) = position_of(src, "scores", "array");
    let (_, scores_column) = position_of(src, "let", "scores");

    assert!(
        is_marked(&client, line, array_column),
        "the `array` token must carry the `promotable` modifier"
    );
    assert!(
        !is_marked(&client, line, scores_column),
        "the binding name is not marked"
    );

    let hover = hover_text_at(&client, line, array_column);
    assert!(
        hover.contains("**WHAT**")
            && hover.contains("fixed-size")
            && hover.contains("**WHAT INSTEAD**")
            && hover.contains("the \"Write 'fixed' instead of 'array'\" action")
            && hover.contains("**WHY**"),
        "hover on `array` must teach the array→fixed decision, got: {hover}"
    );

    let angle_bracket_hover = client.request(
        12,
        "textDocument/hover",
        json!({
            "textDocument": { "uri": URI },
            "position": { "line": line, "character": array_column + "array".len() },
        }),
    );
    assert!(
        angle_bracket_hover["contents"]["value"]
            .as_str()
            .is_none_or(|text| !text.contains("**WHAT INSTEAD**")),
        "hover on the `<` after `array` must not show the `array` promotion hover, got: {angle_bracket_hover}"
    );

    let rewrites = rewrite_actions_at(&client, line, array_column);
    assert_eq!(
        rewrites.len(),
        1,
        "exactly the `array` rewrite at that cursor"
    );
    assert_eq!(
        rewrites[0]["title"], "Write 'fixed' instead of 'array'",
        "the action title is the exact text the hover quotes"
    );
    assert_eq!(
        apply_edit(src, &rewrites[0]),
        src.replace("array<int>", "fixed<int>"),
        "the action rewrites `array` to `fixed` and leaves `<int>` and the initializer alone"
    );

    assert_no_promotion_inlay_hint(&client);
}

#[test]
fn let_to_const_marks_let_token_with_hover_and_rewrite_action() {
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let count = 5\n",
        "}\n",
    );
    let client = open_document(src);
    let (line, let_column) = position_of(src, "let", "let");
    let (_, count_column) = position_of(src, "let", "count");

    assert!(
        is_marked(&client, line, let_column),
        "the `let` keyword must carry the `promotable` modifier"
    );
    assert!(
        !is_marked(&client, line, count_column),
        "the binding name is not marked"
    );

    let hover = hover_text_at(&client, line, let_column);
    assert!(
        hover.contains("**WHAT**")
            && hover.contains("treats this `let` as a `const`")
            && hover.contains("**WHAT INSTEAD**")
            && hover.contains("the \"Write 'const' instead of 'let'\" action")
            && hover.contains("**WHY**"),
        "hover on a promoted `let` replaces the plain keyword hover, got: {hover}"
    );

    let rewrites = rewrite_actions_at(&client, line, let_column);
    assert_eq!(
        rewrites.len(),
        1,
        "exactly the `let` rewrite at that cursor"
    );
    assert_eq!(
        rewrites[0]["title"], "Write 'const' instead of 'let'",
        "the action title is the exact text the hover quotes"
    );
    assert_eq!(
        apply_edit(src, &rewrites[0]),
        src.replace("let count", "const count"),
        "the action rewrites exactly the `let` keyword to `const`"
    );

    assert_no_promotion_inlay_hint(&client);
}

#[test]
fn changed_bindings_and_loop_variables_get_no_marker_hover_or_action() {
    // WHY: the marker must come from the compiler's "never reassigned / never grown"
    // analysis. A marker on a binding that does change would teach a false promotion
    // and offer an edit that breaks the program.
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let total = 0\n",
        "  total = 1\n",
        "  let items: array<int> = []\n",
        "  items.add(1)\n",
        "  for ((key, value) in table) {\n",
        "  }\n",
        "  for ((letter, other) in table) {\n",
        "  }\n",
        "}\n",
    );
    let client = open_document(src);

    // The `for ((key, value) in ...)` loop variables are bindings the parser
    // synthesizes with no written `let`; marking them would rewrite the loop's own
    // source text into `const`. `letter` starts with the characters `let` but is a
    // variable name, not the keyword.
    for (anchor, needle) in [
        ("let total", "let"),
        ("let items", "let"),
        ("items:", "array"),
        ("for ((", "key"),
        ("for ((", "value"),
        ("for ((letter", "letter"),
    ] {
        let (line, column) = position_of(src, anchor, needle);
        assert!(
            !is_marked(&client, line, column),
            "`{needle}` after `{anchor}` must not be marked"
        );
        assert!(
            rewrite_actions_at(&client, line, column).is_empty(),
            "`{needle}` after `{anchor}` must offer no rewrite"
        );
    }

    let (line, let_column) = position_of(src, "let total", "let");
    let plain_hover = hover_text_at(&client, line, let_column);
    assert!(
        !plain_hover.contains("treats this `let` as a `const`"),
        "a reassigned `let` keeps the plain keyword hover, got: {plain_hover}"
    );
}

#[test]
fn let_passed_to_read_only_parameter_is_still_marked() {
    // WHY: shipped bug — the promotion analysis once treated ANY call that received
    // the binding as a mutation, so nearly every real `let` lost its marker. Only a
    // `lend`/`give` parameter may block it; a plain read-only parameter must not.
    let src = concat!(
        "function consume(x: Int) -> Nothing {}\n",
        "function entrypoint() -> nothing {\n",
        "  let val = 42\n",
        "  consume(val)\n",
        "}\n",
    );
    let client = open_document(src);
    let (line, let_column) = position_of(src, "let val", "let");
    assert!(
        is_marked(&client, line, let_column),
        "a binding passed to a read-only parameter is never changed, so `let` stays marked"
    );
}
