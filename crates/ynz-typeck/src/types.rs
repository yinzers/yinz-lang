/// The types known to the M2 type checker.
///
/// Variant count is pinned by `m2_type_variant_count_locked` in tests.
/// Current count: 7
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {

    /// Functions that don't return a value.
    Nothing,
    /// String literal type (full Unicode strings land in M7).
    String,
    /// Placeholder when the type is unknown due to an earlier error.
    /// The type checker does not cascade errors through Error-typed expressions.
    Error,


    /// Signed 64-bit integer — the default inferred type for integer literals.
    Int,
    /// IEEE 754 binary64 floating-point.
    Float,
    /// IEEE 754 decimal128 with `precision` significant decimal digits.
    ///
    /// `precision: 34` is the default (plain `number`). Values > 34 are
    /// deferred to M8. The parser already emits the deferral diagnostic; typeck
    /// treats any `Number { precision != 34 }` as `Error`.
    Number { precision: u32 },
    /// Boolean: `true` or `false`.
    Bool,
}

/// Human-readable type name used in diagnostic messages.
///
/// Matches the Yinz keyword the user wrote, not internal implementation names.
pub fn type_name(t: &Type) -> &'static str {
    match t {
        Type::Nothing => "nothing",
        Type::String => "string",
        Type::Error => "unknown",
        Type::Int => "int",
        Type::Float => "float",
        Type::Number { .. } => "number",
        Type::Bool => "bool",
    }
}
