// WHY: Three-part format is mandatory per Golden Rule 3. Building a diagnostic with an
// empty part must panic at the call site, so the empty text never reaches the user.

use ynz_diagnostics::{Diagnostic, SourceSpan};

/// One case per part of the three-part message: building a diagnostic with that part
/// empty panics at the call site, not at display time.
macro_rules! empty_part_cases {
    ($($case:ident: ($what:expr, $what_instead:expr, $why:expr) panics $message:literal;)+) => {$(
        // bug: a diagnostic with an empty what, what-instead or why can be built, leaving the user without the teaching text Golden Rule 3 requires | tier: High
        #[test]
        #[should_panic(expected = $message)]
        fn $case() {
            Diagnostic::error(SourceSpan::new("test.ynz", 0, 1), $what, $what_instead, $why);
        }
    )+};
}

empty_part_cases! {
    empty_what_panics: ("", "fix it", "because") panics "what must not be empty";
    empty_what_instead_panics: ("problem", "", "because") panics "what_instead must not be empty";
    empty_why_panics: ("problem", "fix it", "") panics "why must not be empty";
}
