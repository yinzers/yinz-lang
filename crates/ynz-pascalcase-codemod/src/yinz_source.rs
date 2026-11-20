//! Rewrites one complete Yinz source text: every old lowercase built-in type name in a type
//! position becomes its PascalCase name. Positions come from the real parser
//! (`ynz_parser::find_type_positions`), never from text matching; the old-to-new pairs come
//! from `ynz_diagnostics::builtin_type_names`, the table the teaching error and the LSP
//! quick-fix read, so there is no second copy of them here.

use std::collections::BTreeSet;
use ynz_diagnostics::builtin_type_names::{
    old_builtin_type_names, pascal_case_name_for_old_builtin_type,
};
use ynz_diagnostics::DiagnosticKind;
use ynz_parser::{find_type_positions, lexer, Spanned, Token};

/// Old lowercase built-in types that are also written as a constructor call
/// (`channel<Int>(8)`, `channel()`). Every other callee (`range(0, 3)`, `pick<Int>(x)`) is a
/// function and is never renamed.
const CONSTRUCTED_IN_CALL_POSITION: &[&str] = &["channel"];

/// The parser reports the legacy square-bracket form `number[N]` as an error although its type
/// name is still a type position the tool must rename (`number[N]` -> `Number[N]`).
const TOLERATED_DIAGNOSTIC_PREFIX: &str = "Use angle brackets for";

pub fn new_name_for(old_name: &str) -> Option<&'static str> {
    pascal_case_name_for_old_builtin_type(old_name)
}

/// True when `text` contains the spelling of any old built-in name. A cheap pre-filter.
pub fn mentions_old_builtin_name(text: &str) -> bool {
    old_builtin_type_names().any(|old| text.contains(old))
}

/// One renamed token, in byte offsets of the text that was given to `rewrite_yinz_source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub start: usize,
    pub end: usize,
    pub new_name: &'static str,
}

#[derive(Debug, Default)]
pub struct YinzRewrite {
    /// Set only under `ParseDiagnostics::Tolerate`: the first parse problem the rewrite went past.
    pub tolerated_parse_problem: Option<String>,
    pub renames: Vec<Rename>,
    /// `int.max`-style reads left alone because the file also binds a variable of that name.
    pub ambiguous_constant_receivers: Vec<(usize, String)>,
}

/// What to do with a text that has parse diagnostics.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ParseDiagnostics {
    /// Leave the text untouched and report it (default; never guess).
    Reject,
    /// Rename the type positions the parser recorded anyway (`--lenient`, error fixtures only).
    Tolerate,
}

/// Why a text was not rewritten. The text is left untouched in every case.
#[derive(Debug)]
pub struct Unparseable {
    pub reason: String,
}

/// O(n) time and memory in the source length: lex, parse, then one pass over the recorded spans.
pub fn rewrite_yinz_source(
    file_label: &str,
    source: &str,
    on_diagnostics: ParseDiagnostics,
) -> Result<YinzRewrite, Unparseable> {
    let found = find_type_positions(file_label, source);

    let mut result = YinzRewrite::default();
    for diagnostic in found.diagnostics.iter() {
        // The teaching error for the old spelling is the input of this tool, not a failure.
        let is_expected_input = matches!(
            diagnostic.kind,
            Some(DiagnosticKind::OldBuiltinTypeName { .. })
        ) || diagnostic.what.starts_with(TOLERATED_DIAGNOSTIC_PREFIX);
        if is_expected_input {
            continue;
        }
        let line = line_of_offset(source, diagnostic.span.start);
        let problem = format!("line {line}: {}", diagnostic.what);
        match on_diagnostics {
            ParseDiagnostics::Reject => return Err(Unparseable { reason: problem }),
            ParseDiagnostics::Tolerate => {
                result.tolerated_parse_problem.get_or_insert(problem);
            }
        }
    }

    let mut seen_starts = BTreeSet::new();
    let mut rename_if_old_spelling = |start: usize, end: usize, only: Option<&[&str]>| {
        let Some(old_text) = source.get(start..end) else {
            return Err(Unparseable {
                reason: format!("span {start}..{end} is not on a char boundary"),
            });
        };
        if only.is_some_and(|names| !names.contains(&old_text)) {
            return Ok(());
        }
        if let Some(new_name) = new_name_for(old_text) {
            if seen_starts.insert(start) {
                result.renames.push(Rename {
                    start,
                    end,
                    new_name,
                });
            }
        }
        Ok(())
    };
    for span in &found.positions.type_names {
        rename_if_old_spelling(span.start, span.end, None)?;
    }
    for span in &found.positions.call_callees {
        rename_if_old_spelling(span.start, span.end, Some(CONSTRUCTED_IN_CALL_POSITION))?;
    }

    let names_bound_as_variables = names_bound_as_variables(&lex_tokens(file_label, source));
    let mut constant_receivers = Vec::new();
    for read in &found.positions.constant_read_receivers {
        // A type-attached constant read is `<type>.<constant>` where the registry knows the pair.
        let Some(new_name) = new_name_for(&read.receiver_name) else {
            continue;
        };
        if ynz_registry::type_attached_constant_lookup(new_name, &read.field_name).is_none() {
            continue;
        }
        if names_bound_as_variables.contains(&read.receiver_name) {
            result.ambiguous_constant_receivers.push((
                line_of_offset(source, read.receiver_span.start),
                read.receiver_name.clone(),
            ));
        } else {
            constant_receivers.push((read.receiver_span.start, read.receiver_span.end));
        }
    }
    for (start, end) in constant_receivers {
        rename_if_old_spelling(start, end, None)?;
    }

    result.renames.sort_by_key(|rename| rename.start);
    Ok(result)
}

fn lex_tokens(file_label: &str, source: &str) -> Vec<Spanned<Token>> {
    lexer::lex(file_label, source).0
}

/// Applies renames (sorted, non-overlapping) to `source`. O(n) time and memory.
pub fn apply_renames(source: &str, renames: &[Rename]) -> String {
    let mut output = String::with_capacity(source.len() + renames.len() * 4);
    let mut copied_up_to = 0;
    for rename in renames {
        output.push_str(&source[copied_up_to..rename.start]);
        output.push_str(rename.new_name);
        copied_up_to = rename.end;
    }
    output.push_str(&source[copied_up_to..]);
    output
}

/// Names that the file declares as a variable, parameter, field, loop variable or function.
/// File-wide and conservative: an `int.max` read in a file that binds `int` anywhere is reported
/// as ambiguous instead of renamed.
fn names_bound_as_variables(tokens: &[Spanned<Token>]) -> BTreeSet<String> {
    let mut bound = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        let Token::Identifier(name) = &token.value else {
            continue;
        };
        let previous = index.checked_sub(1).map(|i| &tokens[i].value);
        let next = tokens.get(index + 1).map(|t| &t.value);
        let declared_after_keyword = matches!(
            previous,
            Some(Token::Let | Token::Const | Token::Function | Token::Shape)
        );
        let followed_by_binding_marker = matches!(next, Some(Token::Colon | Token::In));
        if declared_after_keyword || followed_by_binding_marker {
            bound.insert(name.clone());
        }
    }
    bound
}

pub fn line_of_offset(source: &str, offset: usize) -> usize {
    1 + source.as_bytes()[..offset.min(source.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
}

/// A lowercase built-in name that was NOT renamed, with its neighbours, for the spot-check report.
pub struct Leftover {
    pub line: usize,
    pub name: String,
    pub previous: String,
    pub next: String,
}

/// Lists every identifier token spelled like an old built-in type name that is not in `renames`.
pub fn leftovers_after(source: &str, renames: &[Rename]) -> Vec<Leftover> {
    let (tokens, _) = lexer::lex("leftovers", source);
    let renamed_starts: BTreeSet<usize> = renames.iter().map(|rename| rename.start).collect();
    let describe = |token: Option<&ynz_parser::Spanned<Token>>| match token {
        Some(spanned) => source
            .get(spanned.span.start..spanned.span.end)
            .unwrap_or("?")
            .to_string(),
        None => "<edge>".to_string(),
    };
    let mut leftovers = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let is_old_name = match &token.value {
            Token::Identifier(name) => new_name_for(name).is_some(),
            _ => false,
        };
        if is_old_name && !renamed_starts.contains(&token.span.start) {
            leftovers.push(Leftover {
                line: line_of_offset(source, token.span.start),
                name: describe(Some(token)),
                previous: describe(index.checked_sub(1).and_then(|i| tokens.get(i))),
                next: describe(tokens.get(index + 1)),
            });
        }
    }
    leftovers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewritten(source: &str) -> String {
        let rewrite = rewrite_yinz_source("t.ynz", source, ParseDiagnostics::Reject)
            .expect("test source parses");
        apply_renames(source, &rewrite.renames)
    }

    // Bug it catches: renaming a method, function or variable that shares a built-in's name.
    #[test]
    fn only_type_positions_are_renamed() {
        let source = "\
function entrypoint() -> nothing {
  let string = `x`
  let rows: array<int> = [1, 2]
  let doubled = rows.map(string)
  let span = range(0, 3)
  let wire: channel<int> = channel<int>(8)
  let found: maybe<int> = 1
  print(int.max)
}
";
        let expected = "\
function entrypoint() -> Nothing {
  let string = `x`
  let rows: Array<Int> = [1, 2]
  let doubled = rows.map(string)
  let span = range(0, 3)
  let wire: Channel<Int> = Channel<Int>(8)
  let found: Maybe<Int> = 1
  print(Int.max)
}
";
        assert_eq!(rewritten(source), expected);
    }

    // Bug it catches: a second run changing anything (the tool must be safe to re-run when main
    // moves under a branch).
    #[test]
    fn a_second_run_changes_nothing() {
        let once = rewritten("shape Row {\n  label: string\n  scores: map<string, int>\n}\n");
        assert_eq!(
            once,
            "shape Row {\n  label: String\n  scores: Map<String, Int>\n}\n"
        );
        let rewrite = rewrite_yinz_source("t.ynz", &once, ParseDiagnostics::Reject).unwrap();
        assert!(rewrite.renames.is_empty());
    }

    // Bug it catches: `int.max` renamed although `int` is a variable in the file.
    #[test]
    fn a_constant_read_on_a_variable_named_like_a_type_is_reported_not_renamed() {
        let source = "function f(int: Int) -> Int { return int.max }\n";
        let rewrite = rewrite_yinz_source("t.ynz", source, ParseDiagnostics::Reject).unwrap();
        assert_eq!(rewrite.ambiguous_constant_receivers.len(), 1);
        assert!(rewrite.renames.is_empty());
    }

    // Bug it catches: guessing at a file the parser rejects; it must stay untouched.
    #[test]
    fn a_file_with_a_parse_error_is_rejected_unless_lenient() {
        let source = "function f( -> int { }\n";
        assert!(rewrite_yinz_source("t.ynz", source, ParseDiagnostics::Reject).is_err());
    }
}
