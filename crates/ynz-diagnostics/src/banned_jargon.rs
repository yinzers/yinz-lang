/// Words that must never appear in user-facing compiler diagnostics.
///
/// Source of truth: `design/compiler-errors.md` — the jargon ban-list section.
/// If that file changes, update this list and the sync-test in tests/snapshots.rs.
///
/// Exception from the spec: "auto-propagation" and "auto-propagate" are Yinz's
/// official feature names and may appear once they are explained in plain English
/// on first use. The raw words "propagate" and "propagation" alone are still banned.
pub const BANNED_JARGON: &[&str] = &[
    "propagate",
    "propagation",
    "narrow",
    "narrowing",
    "discriminator",
    "infer",
    "inference",
    "polymorphic",
    "monomorphize",
    "monomorphic",
    "covariant",
    "contravariant",
    "deref",
    "dereference",
    "shadow",
    "shadowing",
    "coerce",
    "coercion",
    "fallible",
    "infallible",
    "first-class",
    "idiomatic",
    "arity",
    "variadic",
    "residual",
    "referentially transparent",
    "immutable",
    "mutable",
    "invariant violation",
    // Acronyms
    "ADT",
    "AST",
];
