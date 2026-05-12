/// The types known to the M1 type checker.
///
/// Variant count is pinned by `m1_type_variant_count_locked` in tests.
/// M1 count: Nothing(1) + String(2) + Error(3)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    /// The `nothing` return type — functions that don't return a value.
    Nothing,
    /// The type of a string literal.
    String,
    /// Placeholder used when the type is unknown due to an earlier error.
    Error,
}
