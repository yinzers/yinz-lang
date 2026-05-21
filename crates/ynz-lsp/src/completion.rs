use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionItemTag, CompletionList, Documentation,
    InsertTextFormat, MarkupContent, MarkupKind, Position, Range, TextEdit,
};
use ynz_registry::{CompletionContext, CompletionKind, RegistryCompletionItem};
use ynz_typeck::shapes::ShapeTable;
use ynz_typeck::signatures::SignatureTable;
use ynz_typeck::types::type_name;
use ynz_parser::SourceFileRegistry as _;

use crate::{capabilities::PositionEncoding, position::LineTable, state::{import_path_for_file, ServerState}};

/// Detect the completion context from the source text and cursor byte offset.
pub fn detect_context<'a>(text: &str, cursor_offset: usize) -> CompletionContext<'a> {
    if cursor_offset == 0 || text.is_empty() {
        return CompletionContext::BareIdentifier;
    }

    let before = &text[..cursor_offset.min(text.len())];

    // Walk backwards past the cursor to find the character just before it
    let bytes = before.as_bytes();
    let last_char = bytes
        .iter()
        .rev()
        .find(|&&b| !b.is_ascii_whitespace())
        .copied();

    if last_char == Some(b'.') {
        // After-dot context: walk left of the '.' to find the receiver token
        // Numeric literal disambiguation: if the previous non-whitespace byte before
        // the '.' is an ASCII digit AND the char before THAT digit is NOT an
        // identifier-continue char, this is a numeric literal — return no context.
        let pos_of_dot = before
            .trim_end_matches(|c: char| c.is_ascii_whitespace())
            .len();
        if pos_of_dot == 0 {
            return CompletionContext::BareIdentifier;
        }
        let before_dot = &before[..pos_of_dot - 1]; // exclude the '.'
        let prev_non_ws = before_dot.trim_end_matches(|c: char| c.is_ascii_whitespace());

        if prev_non_ws.is_empty() {
            return CompletionContext::BareIdentifier;
        }

        // Check if this looks like a numeric literal (e.g. `5.`)
        let last_byte = prev_non_ws.as_bytes().last().copied().unwrap_or(0);
        if last_byte.is_ascii_digit() {
            let second_last = prev_non_ws
                .as_bytes()
                .get(prev_non_ws.len().wrapping_sub(2))
                .copied();
            let is_identifier_continue = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
            let preceded_by_ident = second_last.map(is_identifier_continue).unwrap_or(false);
            if !preceded_by_ident {
                // e.g. `5.` — numeric literal decimal point, not a method call
                return CompletionContext::BareIdentifier;
            }
        }

        // receiver_type is filled in by the caller (server.rs) via type_of_expression_at_offset;
        // detect_context itself is a pure text-analysis function with no db access.
        CompletionContext::AfterDot {
            receiver_type: None,
        }
    } else {
        CompletionContext::BareIdentifier
    }
}

/// Return the byte offset of the last character of the receiver identifier before `cursor_offset`.
///
/// If the character immediately before the cursor (skipping whitespace) is `.`, walks backwards
/// to find the identifier token and returns its end byte offset (exclusive) — which is the byte
/// to pass to `type_of_expression_at_offset` for receiver type narrowing.
///
/// Returns `None` if there is no `.` before the cursor, or if the text before the `.` is not
/// an identifier (e.g., a numeric literal).
pub fn receiver_end_offset(text: &str, cursor_offset: usize) -> Option<usize> {
    if cursor_offset == 0 || text.is_empty() {
        return None;
    }
    let before = &text[..cursor_offset.min(text.len())];
    let bytes = before.as_bytes();
    let last_non_ws = bytes.iter().rev().find(|&&b| !b.is_ascii_whitespace()).copied()?;
    if last_non_ws != b'.' {
        return None;
    }
    // Find the position of the dot
    let dot_pos = before.trim_end_matches(|c: char| c.is_ascii_whitespace()).len();
    if dot_pos == 0 {
        return None;
    }
    let before_dot = &before[..dot_pos - 1];
    let prev_non_ws = before_dot.trim_end_matches(|c: char| c.is_ascii_whitespace());
    if prev_non_ws.is_empty() {
        return None;
    }
    // Walk backwards to find the start of the identifier
    let end = prev_non_ws.len(); // byte offset of end of receiver in `before`
    let is_ident_continue = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let start = prev_non_ws
        .as_bytes()
        .iter()
        .rev()
        .position(|&b| !is_ident_continue(b))
        .map(|rpos| end - rpos)
        .unwrap_or(0);
    if start == end {
        return None; // no identifier characters found
    }
    // Return the offset of the LAST character of the identifier (end - 1 is inside the name)
    Some(end.saturating_sub(1))
}

/// Convert a `RegistryCompletionItem` to an LSP `CompletionItem`.
pub fn to_lsp_completion_item(rci: RegistryCompletionItem) -> CompletionItem {
    let kind = match rci.kind {
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::PrimitiveMethod => CompletionItemKind::METHOD,
        CompletionKind::FreeFn => CompletionItemKind::FUNCTION,
        CompletionKind::TypeAttachedConstant => CompletionItemKind::CONSTANT,
        CompletionKind::DeferredFeature => CompletionItemKind::KEYWORD,
        CompletionKind::BannedKeyword => CompletionItemKind::KEYWORD,
    };

    let tags = if rci.deprecated {
        Some(vec![CompletionItemTag::DEPRECATED])
    } else {
        None
    };

    let documentation = rci.documentation.map(|d| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: d,
        })
    });

    // sort_text pads priority to 3 digits so lexicographic sort == numeric sort
    let sort_text = Some(format!("{:04}_{}", rci.sort_priority, rci.label));

    let snippet = keyword_snippet(&rci.label);
    let insert_text_format = snippet.as_ref().map(|_| InsertTextFormat::SNIPPET);

    CompletionItem {
        label: rci.label,
        kind: Some(kind),
        detail: rci.detail,
        documentation,
        deprecated: Some(rci.deprecated),
        tags,
        sort_text,
        insert_text: snippet,
        insert_text_format,
        ..Default::default()
    }
}

/// Build a call-site snippet for a user-defined function.
///
/// Parameters named `self` are skipped — in UFCS dot-call syntax the receiver is
/// already present before the `.`; inserting it as a tab stop would confuse rather
/// than help. All other parameters become `${N:paramName}` tab stops. When no
/// non-self parameters exist the snippet is `name($0)` (cursor inside the parens).
fn fn_snippet(name: &str, params: &[(String, ynz_typeck::types::Type)]) -> String {
    let non_self: Vec<&str> = params
        .iter()
        .map(|(pname, _)| pname.as_str())
        .filter(|pname| *pname != "self")
        .collect();

    if non_self.is_empty() {
        return format!("{name}($0)");
    }

    let tab_stops = non_self
        .iter()
        .enumerate()
        .map(|(i, pname)| format!("${{{n}:{pname}}}", n = i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{name}({tab_stops})")
}

/// Return a snippet string for keywords that benefit from template expansion,
/// or `None` for keywords where plain insertion is the right behaviour.
fn keyword_snippet(label: &str) -> Option<String> {
    let s = match label {
        "function" => "function ${1:name}(${2:params}) -> ${3:nothing} {\n\t$0\n}",
        "shape"    => "shape ${1:Name} {\n\t${2:field}: ${3:string}\n}",
        "options"  => "options ${1:Name} {\n\t${2:variant1},\n\t${3:variant2},\n}",
        "let"      => "let ${1:name} = $0",
        "const"    => "const ${1:name} = $0",
        "if"       => "if ($1) {\n\t$0\n}",
        "for"      => "for ($1 in $2) {\n\t$0\n}",
        "return"   => "return $0",
        "import"   => "import { $1 } from `$2`",
        _          => return None,
    };
    Some(s.to_string())
}

/// Sort priorities for user-defined symbols. Must be < registry keyword priority (100)
/// so user symbols appear first in the sorted completion list.
const USER_FN_SORT_PRIORITY: u16 = 0;
const USER_SHAPE_SORT_PRIORITY: u16 = 10;
/// Cross-file importable symbols rank between current-file symbols and keywords.
const CROSS_FILE_SORT_PRIORITY: u16 = 50;

/// Build user-defined function + shape completion items from typeck output.
/// These are inserted before all registry items (sort_priority 0-10).
pub fn user_symbol_items(
    sig_table: &SignatureTable,
    shape_table: &ShapeTable,
) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = Vec::new();

    for (name, sig) in &sig_table.fns {
        let param_str = sig
            .params
            .iter()
            .map(|(pname, ptype)| format!("{pname}: {}", type_name(ptype)))
            .collect::<Vec<_>>()
            .join(", ");
        let detail = format!("function {name}({param_str}) -> {}", type_name(&sig.ret));

        // Build a snippet with tab stops for each non-self parameter.
        // `self` is implicit in UFCS dot-call form; including it as a tab stop
        // would force the user to fill it in manually, which defeats the point.
        let snippet = fn_snippet(name, &sig.params);

        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(detail),
            sort_text: Some(format!("{:04}_{name}", USER_FN_SORT_PRIORITY)),
            insert_text: Some(snippet),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }

    for name in shape_table.shapes.keys().filter(|n| !n.starts_with("__anon__")) {
        items.push(CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some(format!("shape {name}")),
            sort_text: Some(format!("{:04}_{name}", USER_SHAPE_SORT_PRIORITY)),
            ..Default::default()
        });
    }

    items
}

/// Build completion items for symbols exported by other files in the project.
///
/// Each item carries `additionalTextEdits` that inserts the import statement when
/// the user selects the item. Only fires in BareIdentifier context (not after-dot).
/// Symbols already visible in `sig_table`/`shape_table` are skipped to avoid
/// offering items that don't need an import.
pub fn cross_file_completion_items(
    state: &ServerState,
    uri: &lsp_types::Url,
    text: &str,
    table: &LineTable,
    sig_table: Option<&SignatureTable>,
    shape_table: Option<&ShapeTable>,
) -> Vec<CompletionItem> {
    let Some(project_root) = state.project_root.as_deref() else {
        return vec![];
    };

    let current_path = crate::state::uri_to_path(uri);
    let mut all_paths = state.db.all_source_paths();
    all_paths.sort();

    // Pre-compute the import insertion position once for the whole list.
    let insert_pos = import_insert_position(state, uri, text, table);

    let mut items: Vec<CompletionItem> = Vec::new();

    for path in &all_paths {
        if path.as_str() == current_path {
            continue;
        }
        let Some(sf) = state.db.source_by_path(path) else {
            continue;
        };
        let Some(import_path) = import_path_for_file(path, project_root) else {
            continue;
        };

        let exports = ynz_typeck::exports_query(&state.db, sf);

        for name in exports.shapes.keys().filter(|n| !n.starts_with("__anon__")) {
            if shape_table.map(|t| t.shapes.contains_key(name)).unwrap_or(false) {
                continue; // already visible
            }
            items.push(import_completion_item(
                name, "shape", CompletionItemKind::CLASS,
                &import_path, insert_pos, state.encoding, text, table,
            ));
        }

        for name in exports.functions.keys() {
            if sig_table.map(|t| t.fns.contains_key(name)).unwrap_or(false) {
                continue;
            }
            items.push(import_completion_item(
                name, "function", CompletionItemKind::FUNCTION,
                &import_path, insert_pos, state.encoding, text, table,
            ));
        }

        for name in exports.options.keys() {
            if shape_table.map(|t| t.options_names.contains(name)).unwrap_or(false) {
                continue;
            }
            items.push(import_completion_item(
                name, "options", CompletionItemKind::ENUM,
                &import_path, insert_pos, state.encoding, text, table,
            ));
        }
    }

    items
}

fn import_completion_item(
    name: &str,
    kind_label: &str,
    kind: CompletionItemKind,
    import_path: &str,
    insert_pos: Position,
    encoding: PositionEncoding,
    text: &str,
    table: &LineTable,
) -> CompletionItem {
    let _ = (encoding, text, table); // consumed via insert_pos which is pre-computed
    let import_text = format!("import {{ {name} }} from `{import_path}`\n");
    let edit = TextEdit {
        range: Range { start: insert_pos, end: insert_pos },
        new_text: import_text,
    };
    CompletionItem {
        label: name.to_string(),
        kind: Some(kind),
        detail: Some(format!("{kind_label} — from `{import_path}`")),
        sort_text: Some(format!("{:04}_{name}", CROSS_FILE_SORT_PRIORITY)),
        additional_text_edits: Some(vec![edit]),
        ..Default::default()
    }
}

/// Compute the LSP Position at which to insert a new import statement.
fn import_insert_position(
    state: &ServerState,
    uri: &lsp_types::Url,
    text: &str,
    table: &LineTable,
) -> Position {
    let byte = crate::code_action::import_insert_byte(state, uri);
    table.byte_offset_to_position(text, byte, state.encoding)
}

/// Build the LSP `CompletionList` for the given cursor position in `text`.
///
/// Merges user-defined symbols (from typeck) before registry items.
///
/// `receiver_type_name` is the resolved type name of the expression before a `.` trigger
/// (e.g. `"int"` for `let score: int = 5; score.`). When `Some`, completion is narrowed to
/// methods of that primitive type. When `None`, all primitive methods are returned as
/// best-effort candidates. Resolved by the server via `type_of_expression_at_offset`.
pub fn completion_list(
    text: &str,
    table: &LineTable,
    position: Position,
    encoding: PositionEncoding,
    sig_table: Option<&SignatureTable>,
    shape_table: Option<&ShapeTable>,
    receiver_type_name: Option<&str>,
) -> Option<CompletionList> {
    let cursor_offset = table.position_to_byte_offset(text, position, encoding)?;
    let mut context = detect_context(text, cursor_offset);

    // Apply pre-resolved receiver type from the server's typeck lookup
    if let (CompletionContext::AfterDot { receiver_type }, Some(rtype)) =
        (&mut context, receiver_type_name)
    {
        *receiver_type = Some(rtype);
    }

    let mut items: Vec<CompletionItem> = Vec::new();

    // User-defined symbols come first (sort_priority 0-10) for BareIdentifier context
    if matches!(context, CompletionContext::BareIdentifier) {
        if let (Some(sig), Some(shapes)) = (sig_table, shape_table) {
            items.extend(user_symbol_items(sig, shapes));
        }
    }

    // Registry items (keywords, intrinsics, deferred features, etc.)
    let registry_items = ynz_registry::lsp_completion_items(&context);
    items.extend(registry_items.into_iter().map(to_lsp_completion_item));

    Some(CompletionList {
        is_incomplete: false,
        items,
    })
}
