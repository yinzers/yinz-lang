//! THE authoritative answer to two questions about an `ErrorsCapable` value's dot-property
//! surface: **which four names are gated behind a `.failed()` check**, and **which of them
//! codegen actually knows how to build a value for**.
//!
//! Before this module the two questions had two independent, hand-written answers: typeck's
//! `EC_FIELDS_REQUIRE_FAILED_CHECK` constant (all four names) and codegen's raw
//! `field_name == "message"` string check with an `Err(...)` fallback for everything else
//! (`crates/ynz-codegen/src/emit.rs`'s `Type::ErrorsCapable` field arm). Nothing bound them, so
//! a name admitted by the first list and refused by the second reached a real user as
//! `"This is a compiler bug"` — a lie about a user program (v0.3 concurrency hardening Phase 3,
//! FRAGO 002 singleton S1). `.claude/rules/authoritative-derivation.md` names this exact class:
//! two lists answering related questions, kept in sync only by a comment.
//!
//! [`ec_field_lowering`] is now the one place either question is answered. Both call sites
//! (`Expr::MethodCall`'s parenthesized dispatch and `Expr::FieldAccess`'s dot-postfix dispatch,
//! per `EC_MEMBER_NAMES`'s doc comment in `check.rs`) consult the same table, and codegen's
//! field arm matches [`EcFieldLowering`] exhaustively — no `_` arm — so a fifth field added to
//! [`EcField`] without a matching codegen arm is a BUILD failure, never a message a user reads.
//!
//! ## The ruling this table encodes
//!
//! `.message` is lowered: codegen can build the string. `.suggestions`, `.trace`, and
//! `.source` are NOT lowered — the runtime's `suggestions_ptr`/`_len` fields are permanently
//! null (nothing in the compiler ever populates them), and building `array<Frame>` /
//! `SourceLoc` values from the runtime's already-captured trace data
//! (`ynz_error_trace_len`/`ynz_error_trace_frame`) needs its own shape-construction machinery
//! that does not exist yet. That is a real, separate, feature-shaped piece of work — not
//! something a producer-alignment fix should grow to cover — so those three fields are refused
//! at COMPILE TIME with real teaching text ([`EcFieldLowering::Refused`]), never admitted only
//! to ICE deep in codegen. `.claude/rules/no-duct-tape.md`: a loud, actionable, honest refusal
//! is a legitimate answer; an internal-error string reached by a correct user program is not.

/// One of the four `ErrorsCapable` dot-properties gated behind a `.failed()` check
/// (`REF-errors.md:171-175`). `.failed()` and `.or(default)` are methods (parens), not
/// properties, and are not part of this table — see `EC_MEMBER_NAMES` in `check.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EcField {
    Message,
    Suggestions,
    Trace,
    Source,
}

impl EcField {
    /// Parse a dot-property name into its `EcField`, or `None` when `name` is not one of the
    /// four gated fields at all (an ordinary shape field, `MapEntry.key`, etc. — the caller
    /// falls through to its normal field-lookup path). This function IS the admission list —
    /// there is no separate array to keep in sync with it.
    pub fn from_field_name(name: &str) -> Option<EcField> {
        match name {
            "message" => Some(EcField::Message),
            "suggestions" => Some(EcField::Suggestions),
            "trace" => Some(EcField::Trace),
            "source" => Some(EcField::Source),
            _ => None,
        }
    }

    /// The field name as written in source — for diagnostic slots.
    pub fn name(self) -> &'static str {
        match self {
            EcField::Message => "message",
            EcField::Suggestions => "suggestions",
            EcField::Trace => "trace",
            EcField::Source => "source",
        }
    }
}

/// Whether codegen has a real lowering for this field, or must refuse it. Exhaustive over
/// [`EcField`] via [`ec_field_lowering`], which has no `_` arm, so a new `EcField` variant
/// fails to compile until someone says whether codegen can build it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EcFieldLowering {
    /// Codegen builds a real value for this field.
    Lowered,
    /// No codegen exists for this field yet. Refused at compile time with the shared
    /// `EcFieldNotYetAvailable` teaching text — never admitted through to codegen.
    Refused,
}

/// THE lowering-status table both typeck's admission gate and codegen's field arm consume.
/// See this module's header for why `suggestions`/`trace`/`source` are `Refused` today.
pub fn ec_field_lowering(field: EcField) -> EcFieldLowering {
    match field {
        EcField::Message => EcFieldLowering::Lowered,
        EcField::Suggestions | EcField::Trace | EcField::Source => EcFieldLowering::Refused,
    }
}
