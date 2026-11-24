// WHY: `Array` → `Fixed` and `let` → `const` are Replacement-category promotions:
// the compiler already chose the stricter form and the editor offers to write it
// into the source through a `refactor.rewrite` code action. A wrong edit span or an
// offer on a binding that does change breaks the user's program, so every case is
// driven through the JSON-RPC boundary and applies the edit.

mod harness;
use harness::{HarnessClient, InProcessHarness};
use serde_json::{json, Value};

const URI: &str = "file:///replacement_marker.ynz";

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

const PROMOTABLE_SRC: &str = concat!(
    "function consume(x: Int) -> Nothing {}\n",
    "function entrypoint() -> Nothing {\n",
    "  let scores: Array<Int> = [1, 2, 3]\n",
    "  let count = 5\n",
    "  let val = 42\n",
    "  consume(val)\n",
    "}\n",
);

const CHANGED_SRC: &str = concat!(
    "function entrypoint() -> Nothing {\n",
    "  let total = 0\n",
    "  total = 1\n",
    "  let items: Array<Int> = []\n",
    "  items.add(1)\n",
    "  for ((key, value) in table) {\n",
    "  }\n",
    "  for ((letter, other) in table) {\n",
    "  }\n",
    "}\n",
);

macro_rules! rewrite_action_case {
    ($name:ident, $src:expr, $anchor:expr, $needle:expr, $rewritten:expr) => {
        // bug: the promotion rewrite action is missing or edits the wrong span for a binding the compiler promoted, or is offered for one that changes (or for a loop variable) and breaks the program | tier: Medium
        #[test]
        fn $name() {
            let src: &str = $src;
            let client = open_document(src);
            let (line, column) = position_of(src, $anchor, $needle);
            let rewrites = rewrite_actions_at(&client, line, column);
            let expected_source: Option<String> = $rewritten;
            match expected_source {
                Some(expected_source) => {
                    assert_eq!(rewrites.len(), 1, "exactly one rewrite at that cursor");
                    assert_eq!(apply_edit(src, &rewrites[0]), expected_source);
                }
                None => assert!(rewrites.is_empty(), "no rewrite expected; got {rewrites:?}"),
            }
        }
    };
}

rewrite_action_case!(
    never_grown_array_is_rewritten_to_fixed,
    PROMOTABLE_SRC,
    "scores",
    "Array",
    Some(PROMOTABLE_SRC.replace("Array<Int>", "Fixed<Int>"))
);
rewrite_action_case!(
    never_reassigned_let_is_rewritten_to_const,
    PROMOTABLE_SRC,
    "let count",
    "let",
    Some(PROMOTABLE_SRC.replace("let count", "const count"))
);
// Shipped bug: any call receiving the binding counted as a mutation, so nearly every
// real `let` lost its rewrite. Only a `lend`/`give` parameter may block it.
rewrite_action_case!(
    let_passed_to_a_read_only_parameter_is_still_rewritten,
    PROMOTABLE_SRC,
    "let val",
    "let",
    Some(PROMOTABLE_SRC.replace("let val", "const val"))
);
rewrite_action_case!(
    reassigned_let_gets_no_rewrite,
    CHANGED_SRC,
    "let total",
    "let",
    None
);
rewrite_action_case!(
    array_that_grows_gets_no_rewrite_on_its_type,
    CHANGED_SRC,
    "items:",
    "Array",
    None
);
rewrite_action_case!(
    array_that_grows_gets_no_rewrite_on_its_let,
    CHANGED_SRC,
    "let items",
    "let",
    None
);
// Loop variables are bindings the parser synthesizes with no written `let`; a
// rewrite would turn the loop's own source text into `const`.
rewrite_action_case!(
    loop_variable_gets_no_rewrite,
    CHANGED_SRC,
    "for ((",
    "key",
    None
);
// `letter` starts with the characters `let` but is a variable name, not the keyword.
rewrite_action_case!(
    loop_variable_named_like_let_gets_no_rewrite,
    CHANGED_SRC,
    "for ((letter",
    "letter",
    None
);
