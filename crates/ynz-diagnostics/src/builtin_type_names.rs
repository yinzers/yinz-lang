//! The built-in type names, in one place.
//!
//! Every type in Yinz starts with a capital letter, built-ins included. The old lowercase
//! spellings are not aliases: they get a teaching error whose quick-fix writes the new name.
//! The parser's error, the LSP quick-fix and the reserved-name check all read THIS table, so
//! the old-to-new pairs are never typed a second time.

use crate::{Diagnostic, DiagnosticKind, SourceSpan};

/// Old lowercase spelling, then the PascalCase name that replaces it. `bool` was never a Yinz
/// spelling but is a common guess, so it teaches the same `Boolean`.
const OLD_TO_NEW_BUILTIN_TYPE_NAMES: &[(&str, &str)] = &[
    ("int", "Int"),
    ("float", "Float"),
    ("number", "Number"),
    ("boolean", "Boolean"),
    ("bool", "Boolean"),
    ("string", "String"),
    ("range", "Range"),
    ("maybe", "Maybe"),
    ("array", "Array"),
    ("fixed", "Fixed"),
    ("map", "Map"),
    ("channel", "Channel"),
    ("nothing", "Nothing"),
];

/// Every PascalCase built-in type name a user shape may not take. `MapEntry`, `Frame` and
/// `SourceLoc` are built-in too, so a shape of that name would hide them the same way.
const RESERVED_BUILTIN_TYPE_NAMES: &[&str] = &[
    "Int",
    "Float",
    "Number",
    "Boolean",
    "String",
    "Range",
    "Maybe",
    "Array",
    "Fixed",
    "Map",
    "Channel",
    "Nothing",
    "MapEntry",
    "Frame",
    "SourceLoc",
];

/// The PascalCase name for an old lowercase built-in spelling (`"int"` gives `"Int"`), or
/// `None` when `name` is not an old built-in spelling. O(1) (the table has 13 rows).
pub fn pascal_case_name_for_old_builtin_type(name: &str) -> Option<&'static str> {
    OLD_TO_NEW_BUILTIN_TYPE_NAMES
        .iter()
        .find(|(old, _)| *old == name)
        .map(|(_, new)| *new)
}

/// Every old lowercase built-in spelling in the table, in table order. For tools that scan
/// source text for old spellings (the PascalCase codemod's cheap pre-filter). O(1) to create.
pub fn old_builtin_type_names() -> impl Iterator<Item = &'static str> {
    OLD_TO_NEW_BUILTIN_TYPE_NAMES.iter().map(|(old, _)| *old)
}

/// True when `name` is a built-in type name a user shape may not take (`Map`, `String`, ...).
/// O(1).
pub fn is_reserved_builtin_type_name(name: &str) -> bool {
    RESERVED_BUILTIN_TYPE_NAMES.contains(&name)
}

/// The teaching error for an old lowercase built-in spelling written where a type goes.
/// `None` when `old_name` is not an old built-in spelling. Carries
/// [`DiagnosticKind::OldBuiltinTypeName`] so the LSP can offer the quick-fix.
pub fn old_builtin_type_name_diagnostic(span: SourceSpan, old_name: &str) -> Option<Diagnostic> {
    let new_name = pascal_case_name_for_old_builtin_type(old_name)?;
    Some(
        Diagnostic::error(
            span,
            format!("`{old_name}` is not a type name."),
            format!("Write `{new_name}`."),
            "Every type in Yinz starts with a capital letter, built-ins included.",
        )
        .with_kind(DiagnosticKind::OldBuiltinTypeName {
            old_name: old_name.to_string(),
        }),
    )
}

/// The "reserved built-in type name" error for a user shape declared with a built-in's name.
/// The WHY names the shape and what it would hide, so it reads as about this code.
pub fn reserved_builtin_type_name_diagnostic(span: SourceSpan, shape_name: &str) -> Diagnostic {
    Diagnostic::error(
        span,
        format!("`{shape_name}` is a reserved built-in type name."),
        format!("Give this shape another name, such as `My{shape_name}` or one that says what it holds."),
        format!(
            "Yinz already has a built-in type called `{shape_name}`. A shape with the same name \
             would make every `{shape_name}` in this file ambiguous between your shape and the \
             built-in."
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Bug it catches: the old-to-new pairs drifting from the reserved set, so a name could be
    // taught as the replacement yet still be declarable by a user shape.
    #[test]
    fn every_replacement_name_is_reserved() {
        for (old, new) in OLD_TO_NEW_BUILTIN_TYPE_NAMES {
            assert!(is_reserved_builtin_type_name(new), "`{new}` (for `{old}`)");
            assert!(!is_reserved_builtin_type_name(old));
        }
    }

    // Bug it catches: the teaching text drifting from the wording Chief approved.
    #[test]
    fn the_teaching_error_has_the_approved_text() {
        let diag = old_builtin_type_name_diagnostic(SourceSpan::new("a.ynz", 0, 3), "int")
            .expect("`int` is an old built-in spelling");
        assert_eq!(diag.what, "`int` is not a type name.");
        assert_eq!(diag.what_instead, "Write `Int`.");
        assert_eq!(
            diag.why,
            "Every type in Yinz starts with a capital letter, built-ins included."
        );
        assert!(old_builtin_type_name_diagnostic(SourceSpan::new("a.ynz", 0, 3), "Int").is_none());
    }
}
