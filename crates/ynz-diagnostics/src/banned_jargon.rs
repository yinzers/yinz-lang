/// Words that must never appear in user-facing compiler diagnostics.
///
/// Source of truth: `design/compiler-errors.md` — the jargon ban-list section.
/// If that file changes, update this list and the sync-test in tests/snapshots.rs.
///
/// Dual-audience: this list governs user-facing diagnostic prose. Design docs
/// and `.claude/rules/*.md` files deliberately use `infer`/`inference` etc. for
/// the engineer audience — see `.claude/rules/inference.md` "Dual-Audience
/// Disclaimer" for why the divergence is intentional.
///
/// Declaration-keyword bans (`type` -> `shape`, `enum` -> `options`) are lexer-
/// level, not in this list. The word "type" appears legitimately in many
/// diagnostic strings ("named type", "return type") so banning it here would
/// false-positive. See `design/compiler-errors.md` "Banned Declaration
/// Keywords" section for the lexer-level approach (M4 will reserve `shape`
/// and add `type` as a banned-keyword token, following the M3 pattern for `fn`).
///
/// Exception: "auto-propagation" / "auto-propagate" are Yinz's official feature
/// names and may appear once explained in plain English on first use. The raw
/// words "propagate" and "propagation" alone are still banned.
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
    // M7 additions — errors/iterables jargon (2026-05-18)
    "monad",
    "lift",
    "wrap",
    "unwrap",
    "Result",
    "Option",
    "Either",
    "exception",
    "try",
    "catch",
    "throw",
    "UTF-16",
];
