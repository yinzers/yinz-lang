//! Rewrites one complete Yinz source text: every built-in type name in a type position
//! becomes PascalCase. Positions come from the real parser (`ynz_parser`), never from text matching.

use std::collections::BTreeSet;
use ynz_parser::lexer;
use ynz_parser::queries::parse_lexed;
use ynz_parser::{LexOutput, Token};

/// Old lowercase spelling -> new PascalCase spelling.
pub const RENAMES: &[(&str, &str)] = &[
    ("int", "Int"),
    ("float", "Float"),
    ("number", "Number"),
    ("bool", "Boolean"),
    ("boolean", "Boolean"),
    ("string", "String"),
    ("range", "Range"),
    ("maybe", "Maybe"),
    ("array", "Array"),
    ("fixed", "Fixed"),
    ("map", "Map"),
    ("channel", "Channel"),
    ("nothing", "Nothing"),
];

/// Names whose `Name.max` / `.min` / `.epsilon` reads are type-attached constants.
const CONSTANT_RECEIVERS: &[&str] = &["int", "float", "number"];

/// The lexer/parser reports the legacy `number[N]` form as an error, but its type name is
/// still a type position and the codemod must rename it (`number[N]` -> `Number[N]`).
const TOLERATED_DIAGNOSTIC_PREFIX: &str = "Use angle brackets for `number<N>`";

pub fn new_name_for(old_name: &str) -> Option<&'static str> {
    RENAMES
        .iter()
        .find(|(old, _)| *old == old_name)
        .map(|(_, new)| *new)
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
    let (tokens, lex_diagnostics) = lexer::lex(file_label, source);
    let lexed = LexOutput {
        tokens,
        diagnostics: lex_diagnostics,
    };
    let parsed = parse_lexed(file_label, &lexed);

    let mut result = YinzRewrite::default();
    for diagnostic in parsed.diagnostics.iter() {
        if diagnostic.what.starts_with(TOLERATED_DIAGNOSTIC_PREFIX) {
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

    for span in &parsed.type_name_spans {
        let Some(old_text) = source.get(span.start..span.end) else {
            return Err(Unparseable {
                reason: format!("span {}..{} is not on a char boundary", span.start, span.end),
            });
        };
        if let Some(new_name) = new_name_for(old_text) {
            if seen_starts.insert(span.start) {
                result.renames.push(Rename {
                    start: span.start,
                    end: span.end,
                    new_name,
                });
            }
        }
    }

    let names_bound_as_variables = names_bound_as_variables(&lexed.tokens);
    for span in &parsed.type_const_receiver_spans {
        let old_text = source.get(span.start..span.end).unwrap_or_default();
        if !CONSTANT_RECEIVERS.contains(&old_text) {
            continue;
        }
        if names_bound_as_variables.contains(old_text) {
            result
                .ambiguous_constant_receivers
                .push((line_of_offset(source, span.start), old_text.to_string()));
        } else if let Some(new_name) = new_name_for(old_text) {
            if seen_starts.insert(span.start) {
                result.renames.push(Rename {
                    start: span.start,
                    end: span.end,
                    new_name,
                });
            }
        }
    }

    result.renames.sort_by_key(|rename| rename.start);
    Ok(result)
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
fn names_bound_as_variables(tokens: &[ynz_parser::Spanned<Token>]) -> BTreeSet<String> {
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
            Token::Nothing => true,
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
