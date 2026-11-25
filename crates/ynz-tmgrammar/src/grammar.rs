use serde_json::{json, Value};

/// Built-in type names, all PascalCase. `Nothing` is an ordinary type name, not a keyword
/// (card #573: every built-in type is PascalCase).
const BUILT_IN_TYPES: [&str; 12] = [
    "Int", "Float", "Number", "Boolean", "String", "Range", "Maybe", "Array", "Fixed", "Map",
    "Channel", "Nothing",
];

/// Build the TextMate grammar as a JSON value, sourced entirely from the SSOT registry.
///
/// Grammar structure:
/// - Keywords → `keyword.control.ynz` scope (normal highlighting)
/// - Banned declaration keywords → `invalid.deprecated` scope (strikethrough/warning visual)
/// - Deferred language features → `invalid.illegal` scope (illegal visual — not yet in Yinz)
/// - Literals (booleans, numbers, strings) → standard scopes
/// - Comments → standard `comment.*` scopes
pub fn build_grammar() -> Value {
    // Exclude literal-value keywords (true, false, none) — they're covered by
    // #literals with a more specific scope. Keywords are matched after literals
    // in the outer patterns array, so this is defense-in-depth.
    let literal_kws: std::collections::HashSet<&str> =
        ["true", "false", "none"].iter().copied().collect();
    let keywords: Vec<String> = ynz_registry::keywords()
        .filter(|e| !literal_kws.contains(e.name))
        .map(|e| e.name.to_string())
        .collect();
    let banned: Vec<String> = ynz_registry::banned_declaration_keywords()
        .map(|e| e.name.to_string())
        .collect();
    let deferred: Vec<String> = ynz_registry::deferred_language_features()
        .map(|e| e.name.to_string())
        .collect();

    let kw_pattern = build_word_boundary_regex(&keywords);
    let banned_pattern = build_word_boundary_regex(&banned);
    let deferred_pattern = build_word_boundary_regex(&deferred);
    let built_in_type_names: Vec<String> = BUILT_IN_TYPES
        .iter()
        .map(|type_name| type_name.to_string())
        .collect();
    let built_in_type_pattern = build_word_boundary_regex(&built_in_type_names);

    json!({
        "name": "Yinz",
        "scopeName": "source.ynz",
        "fileTypes": ["ynz"],
        "patterns": [
            { "include": "#comments" },
            { "include": "#strings" },
            { "include": "#literals" },
            { "include": "#banned-keywords" },
            { "include": "#deferred-features" },
            { "include": "#keywords" },
            { "include": "#built-in-types" }
        ],
        "repository": {
            "comments": {
                "patterns": [
                    {
                        "name": "comment.line.double-slash.ynz",
                        "match": "//.*$"
                    },
                    {
                        "name": "comment.line.doc.ynz",
                        "match": "///.*$"
                    },
                    {
                        "name": "comment.block.ynz",
                        "begin": "/\\*",
                        "end": "\\*/"
                    }
                ]
            },
            "strings": {
                "patterns": [
                    {
                        "name": "string.template.ynz",
                        "begin": "`",
                        "end": "`",
                        "patterns": [
                            {
                                "name": "variable.other.interpolation.ynz",
                                "begin": "\\$\\{",
                                "end": "\\}"
                            }
                        ]
                    }
                ]
            },
            "keywords": {
                "name": "keyword.control.ynz",
                "match": kw_pattern
            },
            "built-in-types": {
                "name": "support.type.builtin.ynz",
                "match": built_in_type_pattern
            },
            "banned-keywords": {
                "name": "invalid.deprecated",
                "match": banned_pattern
            },
            "deferred-features": {
                "name": "invalid.illegal",
                "match": deferred_pattern
            },
            "literals": {
                "patterns": [
                    {
                        "name": "constant.language.boolean.ynz",
                        "match": "\\b(true|false)\\b"
                    },
                    {
                        "name": "constant.language.null.ynz",
                        "match": "\\bnone\\b"
                    },
                    {
                        "name": "constant.numeric.float.ynz",
                        "match": "\\b[0-9][0-9_]*\\.[0-9][0-9_]*\\b"
                    },
                    {
                        "name": "constant.numeric.integer.ynz",
                        "match": "\\b[0-9][0-9_]*\\b"
                    }
                ]
            }
        }
    })
}

/// Build a word-boundary Oniguruma regex from a list of keywords.
/// Uses `\b(kw1|kw2|...)\b` — simple and no lookbehind / nested captures.
fn build_word_boundary_regex(words: &[String]) -> String {
    if words.is_empty() {
        return String::from("(?!x)x"); // matches nothing
    }
    // Sort by length descending so longer keywords match before shorter prefixes
    let mut sorted = words.to_vec();
    sorted.sort_by_key(|w| std::cmp::Reverse(w.len()));
    let joined = sorted.join("|");
    format!("\\b({joined})\\b")
}

#[cfg(test)]
mod tests {
    use super::*;

    // bug: a registry receiver type is missing from BUILT_IN_TYPES, so the editor does not highlight it | tier: Medium
    #[test]
    fn every_registry_receiver_type_is_highlighted() {
        let missing: Vec<String> = ynz_registry::primitive_intrinsics()
            .filter_map(|entry| entry.receiver_type)
            .filter(|receiver_type| !BUILT_IN_TYPES.contains(receiver_type))
            .map(|receiver_type| {
                format!("registry receiver_type {receiver_type} is missing from BUILT_IN_TYPES")
            })
            .collect();
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }
}
