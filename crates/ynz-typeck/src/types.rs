/// The types known to the M4 type checker.
///
/// Variant count is pinned by `m4_type_variant_count_locked` in tests.
/// Current count: 9
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
    /// Internal type produced by `range(...)` calls.
    ///
    /// Only valid in the iterable position of a `for` loop. Using it in any other
    /// position (let binding, function argument, return type) is a compile error
    /// pointing to M7, where the full `Iterable[T]` protocol replaces this type.
    ///
    /// REPLACE-AT M7: remove and replace with Iterable[T] protocol dispatch.
    Range {
        /// Always `Int` in M3.
        element: Box<Type>,
        /// Always `false` in M3 (range end is exclusive).
        end_inclusive: bool,
    },

    // ── M4 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M4 adds Shape and Dynamic for user-defined types.

    /// A user-defined shape type, identified by name.
    ///
    /// Field layout and method resolution use the `ShapeTable`.
    Shape { name: String },

    /// Runtime-dispatch type: `dynamic Foo` where Foo is a contract shape.
    ///
    /// Values of this type are fat pointers `{ data_ptr, vtable_ptr }`.
    /// Method dispatch costs ~3× a static call — opt-in only.
    Dynamic { contract: String },
}

/// Human-readable type name for diagnostic messages.
///
/// Matches the Yinz keyword the user wrote, not internal implementation names.
/// Returns `String` (not `&'static str`) to handle dynamic shape names.
pub fn type_name(t: &Type) -> String {
    match t {
        Type::Nothing => "nothing".into(),
        Type::String => "string".into(),
        Type::Error => "unknown".into(),
        Type::Int => "int".into(),
        Type::Float => "float".into(),
        Type::Number { .. } => "number".into(),
        Type::Bool => "bool".into(),
        Type::Range { .. } => "range".into(),
        Type::Shape { name } => name.clone(),
        Type::Dynamic { contract } => format!("dynamic {contract}"),
    }
}
