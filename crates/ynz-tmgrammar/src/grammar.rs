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

    #[test]
    fn grammar_has_required_top_level_fields() {
        let g = build_grammar();
        assert!(g["name"].is_string(), "grammar must have a name");
        assert!(g["scopeName"].is_string(), "grammar must have a scopeName");
        assert!(g["patterns"].is_array(), "grammar must have patterns");
        assert!(g["repository"].is_object(), "grammar must have repository");
    }

    #[test]
    fn keyword_pattern_contains_function() {
        let g = build_grammar();
        let kw_pattern = g["repository"]["keywords"]["match"].as_str().unwrap_or("");
        assert!(
            kw_pattern.contains("function"),
            "keyword pattern must include 'function'"
        );
    }

    #[test]
    fn banned_pattern_contains_class() {
        let g = build_grammar();
        let banned_pattern = g["repository"]["banned-keywords"]["match"]
            .as_str()
            .unwrap_or("");
        assert!(
            banned_pattern.contains("class"),
            "banned pattern must include 'class'"
        );
    }

    #[test]
    fn deferred_pattern_contains_gpu() {
        let g = build_grammar();
        let deferred = g["repository"]["deferred-features"]["match"]
            .as_str()
            .unwrap_or("");
        assert!(
            deferred.contains("gpu"),
            "deferred pattern must include 'gpu'"
        );
    }

    #[test]
    fn built_in_types_highlighted_and_nothing_is_not_a_keyword() {
        let g = build_grammar();
        let type_pattern = g["repository"]["built-in-types"]["match"]
            .as_str()
            .unwrap_or("");
        for type_name in BUILT_IN_TYPES {
            assert!(
                type_pattern.contains(type_name),
                "built-in type {type_name} must be highlighted"
            );
        }
        let kw_pattern = g["repository"]["keywords"]["match"].as_str().unwrap_or("");
        assert!(
            !kw_pattern.to_lowercase().contains("nothing"),
            "`Nothing` is a type name, not a keyword"
        );
    }

    #[test]
    fn every_registry_receiver_type_is_highlighted() {
        for entry in ynz_registry::primitive_intrinsics() {
            if let Some(receiver_type) = entry.receiver_type {
                assert!(
                    BUILT_IN_TYPES.contains(&receiver_type),
                    "registry receiver_type {receiver_type} is missing from BUILT_IN_TYPES"
                );
            }
        }
    }

    #[test]
    fn regex_pattern_has_word_boundaries() {
        let g = build_grammar();
        let kw = g["repository"]["keywords"]["match"].as_str().unwrap_or("");
        assert!(
            kw.starts_with("\\b("),
            "keyword regex must start with word boundary \\b("
        );
        assert!(kw.ends_with(")\\b"), "keyword regex must end with )\\b");
    }
}
