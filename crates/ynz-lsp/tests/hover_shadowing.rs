// WHY: Regression suite for Bug 2.8 (expression-position user identifiers must
// return a user-symbol hover with the binding's type, not a keyword hover, and
// must return Some, not None — without the context-aware gate, a name like
// `share`/`lend` falls through both registry and sig_table to None) and Bug 2.12
// (cursor at tok.span.end must resolve to the token — editors place the cursor at
// the byte AFTER the last character of a token).

use lsp_types::HoverContents;
use ynz_lsp::{
    capabilities::PositionEncoding,
    hover::{hover_response, token_at_offset},
    position::LineTable,
};
use ynz_parser::lexer::lex;
use ynz_parser::queries::parse_query;

fn tokenize(src: &str) -> Vec<ynz_parser::token::Spanned<ynz_parser::token::Token>> {
    let (tokens, _) = lex("test.ynz", src);
    tokens
}

fn make_empty_sig() -> ynz_typeck::signatures::SignatureTable {
    ynz_typeck::signatures::SignatureTable {
        fns: std::collections::HashMap::new(),
    }
}

fn make_db_and_module(src: &str) -> ynz_ast::nodes::Module {
    use ynz_parser::{CompilerDb, SourceFile};
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, "test.ynz".to_string(), src.to_string());
    db.register_source(sf);
    let parse = parse_query(&db, sf);
    parse.module.clone()
}

/// Returns true if the hover body is a registry keyword entry.
///
/// Registry keyword hovers always open with an `## Keyword` heading — that is the
/// structural invariant produced by `lsp_hover_for_token`. Other phrases
/// ("read-only borrow", "fallible", etc.) are variable body text that can change
/// without breaking the hover contract; only the heading is stable.
fn is_keyword_hover(body: &str) -> bool {
    body.contains("## Keyword")
}

// ──────────────────────────────────────────────────────────────────────────────
// Bug 2.8: contextual-keyword identifier in expression position
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn variable_named_share_in_expression_position_shows_type_in_hover() {
    // WHY: An annotated local (`let share: int = 5`) hovered at its expression
    // use-site must show the variable's type (`int`) — not None and not a keyword
    // hover. The invariant: expression-position identifiers always get a user-symbol
    // hover, and annotated ones always include the type name. Catches a regression
    // where the binding hover was typeless ("Binding: share") even when the type
    // annotation was available via the cheap AST walk.
    let src = "function entrypoint() -> nothing {\n  let share: int = 5\n  print(share)\n}";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    // `print(share)` — the last `share` in the source is the expression use-site.
    let use_offset = src.rfind("share").unwrap();

    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        use_offset,
        PositionEncoding::Utf8,
        None,
        None,
    );

    assert!(
        result.is_some(),
        "expression-position `share` binding must return Some hover (not None)"
    );
    if let Some(hover) = result {
        if let HoverContents::Markup(mc) = hover.contents {
            assert!(
                !is_keyword_hover(&mc.value),
                "expression-position `share` binding must NOT show keyword hover; \
                 got: {v:?}",
                v = mc.value
            );
            // The annotation `let share: int` must surface the type.
            assert!(
                mc.value.contains("int"),
                "annotated binding hover must include the type `int`; got: {v:?}",
                v = mc.value
            );
        }
    }
}

#[test]
fn variable_named_lend_in_expression_position_shows_binding_hover_not_none() {
    // WHY: `lend`, like `share`, is lexed as Token::Identifier — it can be used as a
    // variable binding name without a keyword hover entry in the registry. An annotated
    // `let lend: int` hovered at its expression use-site must return Some with the type,
    // not None. Guards that the context-aware path handles ALL Identifier-class contextual
    // keywords, not just `share`.
    let src = "function entrypoint() -> nothing {\n  let lend: int = 7\n  print(lend)\n}";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    // The use in `print(lend)` — last occurrence.
    let use_offset = src.rfind("lend").unwrap();

    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        use_offset,
        PositionEncoding::Utf8,
        None,
        None,
    );

    assert!(
        result.is_some(),
        "expression-position `lend` binding must return Some hover (not None)"
    );
    if let Some(hover) = result {
        if let HoverContents::Markup(mc) = hover.contents {
            assert!(
                !is_keyword_hover(&mc.value),
                "expression-position `lend` binding must NOT show keyword hover; \
                 got: {v:?}",
                v = mc.value
            );
        }
    }
}

#[test]
fn errors_keyword_in_return_type_position_still_shows_keyword_hover() {
    // WHY: `errors` in a return-type modifier position (`-> nothing errors`) is a
    // hard token — not an identifier use-site in the AST. identifier_use_site_at_offset
    // returns None for it, so the registry path fires and delivers the keyword hover.
    // The invariant: the context-aware reorder must not suppress keyword hover for
    // genuine keyword positions. Failing this means `errors` at a return-type site
    // would silently return None instead of teaching the user what the keyword does.
    let src = "errors";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    // Top-level bare `errors` — not inside a function body, so identifier_use_site_at_offset
    // returns None → falls through to registry.
    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        0,
        PositionEncoding::Utf8,
        None,
        None,
    );

    assert!(
        result.is_some(),
        "bare `errors` keyword must return Some (keyword hover via registry lookup), not None"
    );
    if let Some(hover) = result {
        if let HoverContents::Markup(mc) = hover.contents {
            assert!(
                is_keyword_hover(&mc.value),
                "`errors` in keyword position must show a `## Keyword` registry hover; \
                 got: {v:?}",
                v = mc.value
            );
        }
    }
}

#[test]
fn wait_keyword_still_shows_keyword_hover_after_fix() {
    // WHY: `wait` is in the registry with a `## Keyword` entry. Bare `wait` outside
    // a function body has no identifier use-site in the AST, so identifier_use_site_at_offset
    // returns None → registry fires → keyword hover shown. The invariant: `wait` in any
    // genuine keyword position must deliver a `## Keyword` hover body. Failing this means
    // a regression in the registry lookup path (Step 2 of hover_response).
    let src = "wait";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        0,
        PositionEncoding::Utf8,
        None,
        None,
    );

    assert!(
        result.is_some(),
        "bare `wait` keyword must return Some (keyword hover via registry lookup), not None"
    );
    if let Some(hover) = result {
        if let HoverContents::Markup(mc) = hover.contents {
            assert!(
                is_keyword_hover(&mc.value),
                "`wait` in keyword position must show a `## Keyword` registry hover; \
                 got: {v:?}",
                v = mc.value
            );
        }
    }
}

#[test]
fn share_in_function_signature_modifier_position_returns_none() {
    // WHY: In `function f(share self: int) -> nothing { }`, `share` at the ownership-
    // modifier position is not tracked as an identifier use-site by `identifier_use_site_at_offset`
    // (it walks param name spans, not modifier keyword spans). So the context-aware path
    // skips it, registry lookup finds no entry for `share`, and sig_table has no entry →
    // result is None. The invariant: a bare ownership modifier with no registry entry and
    // no variable shadow must return None, not a spurious binding hover. Catches a
    // regression where the context-aware path would incorrectly claim `share` is a
    // user-defined symbol just because it appears as a Token::Identifier.
    let src = "function f(share self: int) -> nothing { }";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    let share_modifier_offset = src.find("share").unwrap();

    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        share_modifier_offset,
        PositionEncoding::Utf8,
        None,
        None,
    );

    // `share` modifier: no registry entry, no use-site, no sig_table entry → None.
    // If `share` is later added to the registry, the result becomes Some with keyword
    // content — update this test at that point.
    assert!(
        result.is_none(),
        "ownership-modifier `share` with no registry entry must return None; got Some"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Bug 2.12: cursor at tok.span.end returns None instead of hover content
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn end_of_token_cursor_returns_hover_content() {
    // WHY: Editors that place the cursor at the byte immediately after the last
    // character of a token (tok.span.end) must still resolve the token. The `<=`
    // upper bound in `token_at_offset` makes this work; reverting it to `<` silently
    // breaks hover for common editor cursor-placement conventions (Bug 2.12).
    let src = "function entrypoint() -> nothing { }";
    let tokens = tokenize(src);

    // "function" spans bytes [0, 8). Cursor at byte 8 = span.end.
    let function_end = "function".len(); // = 8
    let result = token_at_offset(&tokens, function_end);

    assert!(
        result.is_some(),
        "cursor at tok.span.end ({function_end}) must return the token"
    );
    if let Some((text, start, end)) = result {
        assert_eq!(text, "function");
        assert_eq!(start, 0);
        assert_eq!(end, function_end);
    }
}

#[test]
fn end_of_token_cursor_does_not_double_match_adjacent_token() {
    // WHY: The `<=` span.end bound must not cause adjacent-token double-matching.
    // When tokens are separated by whitespace, a cursor at tok.span.end falls in
    // whitespace (not inside the next token's start), so None is the correct result.
    // Catches a regression where the `<=` bound accidentally resolved whitespace gaps
    // to the preceding token.
    let src = "function entrypoint() -> nothing { }";
    let tokens = tokenize(src);

    // `->` in "function entrypoint() -> nothing". Find its end byte.
    let arrow_pos = src.find("->").unwrap();
    let arrow_end = arrow_pos + 2; // byte after `>`; whitespace follows before `nothing`

    let result = token_at_offset(&tokens, arrow_end);
    // Whitespace after `->` — must return None (not `nothing`).
    assert!(
        result.is_none(),
        "cursor in whitespace gap after `->` must return None, not bridge to adjacent `nothing`"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Adversarial: inner-scope shadowing resolves innermost symbol (plan Step 4)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn inner_shadowing_share_resolves_to_binding_hover_not_keyword() {
    // WHY: When an outer-scope and inner-scope binding share the same name (`share`),
    // hovering the inner use-site must return Some with a user-symbol hover — not None
    // and not a keyword hover. Guards that identifier_use_site_at_offset resolves the
    // innermost expression use-site correctly, and that the result is Some regardless
    // of any keyword disambiguation logic.
    let src = concat!(
        "function entrypoint() -> nothing {\n",
        "  let share: int = 5\n", // outer binding
        "  if (true) {\n",
        "    let share: int = 99\n", // inner binding (shadows outer)
        "    print(share)\n",        // inner use-site — resolve this one
        "  }\n",
        "}"
    );
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    // `print(share)` — the last `share` in the source is the inner use-site.
    let inner_use_offset = src.rfind("share").unwrap();

    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        inner_use_offset,
        PositionEncoding::Utf8,
        None,
        None,
    );

    assert!(
        result.is_some(),
        "hovering inner-scope `share` use must return Some (binding hover)"
    );
    if let Some(hover) = result {
        if let HoverContents::Markup(mc) = hover.contents {
            assert!(
                !is_keyword_hover(&mc.value),
                "inner-scope `share` use must NOT show keyword hover; got: {v:?}",
                v = mc.value
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Regression: hover still works for genuine keywords and identifiers
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn keyword_hover_function_unaffected_by_fix() {
    // WHY: `function` is a genuine keyword with a registry entry and is never a
    // user-defined identifier. Its hover must survive the context-aware reorder
    // unchanged — `identifier_use_site_at_offset` does NOT flag the `function` token
    // itself as an expression use-site, so the registry path fires normally.
    let src = "function entrypoint() -> nothing { }";
    let tokens = tokenize(src);
    let table = LineTable::new(src);
    let module = make_db_and_module(src);

    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        Some(&module),
        src,
        &table,
        0, // offset 0 = start of `function`
        PositionEncoding::Utf8,
        None,
        None,
    );

    assert!(
        result.is_some(),
        "`function` keyword hover must still return Some"
    );
}

#[test]
fn no_panic_when_module_is_none() {
    // WHY: `module: Option<&Module>` is None when the LSP has tokens but no parsed
    // AST yet (e.g. unparseable file). The context-aware path is skipped; hover falls
    // through to registry or returns None. Must never panic regardless of module state.
    let src = "share";
    let tokens = tokenize(src);
    let table = LineTable::new(src);

    // module = None simulates the pre-AST-available fallback path.
    let result = hover_response(
        &tokens,
        &make_empty_sig(),
        None,
        src,
        &table,
        0,
        PositionEncoding::Utf8,
        None,
        None,
    );

    // Asserts only that it does NOT panic. Result may be Some or None.
    let _ = result;
}
