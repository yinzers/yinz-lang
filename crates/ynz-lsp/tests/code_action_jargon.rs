// WHY: These tests guard two related teaching-surface correctness properties:
//   (a) completion must NOT fire on every space — space in trigger_characters creates
//       popup noise on every word boundary, harming usability.
//   (b) banned-jargon diagnostics must carry a quick-fix lightbulb that replaces the
//       jargon word with its Yinz equivalent AND surfaces the WHY from the registry.
//       Without this, the diagnostic is a passive warning the user can't act on.

use ynz_lsp::{
    capabilities::{server_capabilities, PositionEncoding},
    code_action::code_action_response,
    position::LineTable,
    state::ServerState,
};

fn state_single(path: &str, src: &str) -> (ServerState, lsp_types::Url) {
    let mut state = ServerState::new(PositionEncoding::Utf8);
    let uri = lsp_types::Url::from_file_path(path).expect("valid path");
    state.open_document(uri.clone(), src.to_string());
    (state, uri)
}

fn full_range(src: &str) -> lsp_types::Range {
    let table = LineTable::new(src);
    let start = table.byte_offset_to_position(src, 0, PositionEncoding::Utf8);
    let end = table.byte_offset_to_position(src, src.len(), PositionEncoding::Utf8);
    lsp_types::Range { start, end }
}

// ─── (a) space trigger removed ───────────────────────────────────────────────

#[test]
fn completion_trigger_characters_does_not_include_space() {
    // WHY: space in trigger_characters fires the completion popup on every word
    // boundary — typing prose or any identifier triggers an unwanted dropdown.
    // Only `.` should trigger autocomplete; space must be absent.
    let caps = server_capabilities(PositionEncoding::Utf8);
    let triggers = caps
        .completion_provider
        .expect("completion_provider must be set")
        .trigger_characters
        .expect("trigger_characters must be set");
    assert!(
        !triggers.contains(&" ".to_string()),
        "trigger_characters must not contain space — found: {triggers:?}"
    );
    assert!(
        triggers.contains(&".".to_string()),
        "trigger_characters must still contain `.` for member access completion"
    );
}

// ─── (b) BannedJargon code action ────────────────────────────────────────────

#[test]
fn banned_jargon_identifier_produces_replacement_code_action() {
    // WHY: `infer` is a banned-jargon word (type-theory jargon — see registry
    // [[banned_jargon]] entries).  When a user writes it as an identifier, the
    // compiler emits a BannedJargon diagnostic.  This test verifies that a
    // quick-fix code action is produced, that it replaces the word using the
    // registry's replacement text, and that the action title surfaces the WHY
    // (the reason the word is banned) so the user learns from the lightbulb click
    // rather than just getting a word swap they don't understand.
    let src = "let infer = 5\n";
    let (state, uri) = state_single("/tmp/ynz_ca_jargon_infer.ynz", src);
    let range = full_range(src);
    let actions = code_action_response(&state, &uri, range);

    // At least one code action for the banned-jargon word.
    assert!(
        !actions.is_empty(),
        "expected a code action for banned-jargon identifier `infer`; got none. \
         DiagnosticKind::BannedJargon {{ term }} variant must be emitted and handled."
    );

    // Find the jargon quick-fix (there may also be other actions).
    let jargon_action = actions.iter().find_map(|a| {
        if let lsp_types::CodeActionOrCommand::CodeAction(ca) = a {
            // Jargon actions contain "jargon" or the replacement token in their title.
            if ca.title.contains("infer") {
                return Some(ca);
            }
        }
        None
    });

    let action = jargon_action.expect(
        "no code action title containing `infer` found among actions; \
         action title must reference the banned term",
    );

    // The action must carry a workspace edit that replaces `infer`.
    let changes = action
        .edit
        .as_ref()
        .and_then(|e| e.changes.as_ref())
        .expect("code action must have a workspace edit with changes");
    let edits = changes
        .get(&uri)
        .expect("edit changes must target the open file");
    assert!(
        !edits.is_empty(),
        "edit changes must contain at least one TextEdit"
    );

    // The replacement text must come from the registry (not hardcoded here).
    // The registry entry for `infer` says: replacement = "figure out automatically"
    let expected_replacement = ynz_registry::banned_jargon_lookup("infer")
        .expect("registry must have a `[[banned_jargon]]` entry for `infer`")
        .replacement;
    assert_eq!(
        edits[0].new_text, expected_replacement,
        "replacement text must be sourced from registry — no hardcoded duplicate"
    );

    // The title must surface the WHY from the registry's `reason` field so the
    // user learns why `infer` is banned, not just that it changed.
    let expected_reason = ynz_registry::banned_jargon_lookup("infer")
        .expect("registry must have a `[[banned_jargon]]` entry for `infer`")
        .reason;
    assert!(
        action.title.contains(expected_reason),
        "action title must contain the registry `reason` (WHY) for the ban: \
         expected to find {expected_reason:?} in title {:?}",
        action.title
    );
}
