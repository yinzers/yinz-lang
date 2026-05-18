/// The types known to the M7 type checker.
///
/// Variant count is pinned by `m4_type_variant_count_locked` in tests.
/// Current count: 20 (M7 P3a adds ErrorsCapable; total 20 across M1–M7)
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
    /// The type produced by `range(...)` calls — an integer range value.
    ///
    /// First-class from M7 onward: can be stored in bindings, passed to functions,
    /// and returned. Iterating over a `Range` in a `for` loop yields integers.
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

    // ── M5 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M5 P3a adds TypeParam and Generic for the generics engine.

    /// A type parameter placeholder inside a generic function or shape body.
    ///
    /// `function identity<T>(give value: T) -> T` — the two `T` references in the
    /// signature produce `TypeParam { name: "T" }`. Resolved to a concrete type
    /// by the generics engine at each call site.
    TypeParam { name: String },

    /// A concrete generic instantiation: `Pair<int, string>`, `array<Player>`, etc.
    ///
    /// `name` is the type constructor (user-defined generic shape or built-in).
    /// `args` are the concrete type arguments, already resolved.
    ///
    /// P3a uses this for user-defined generic shapes only.
    /// P3b extends it to built-in collections (`array`, `fixed`, `map`, `maybe`).
    Generic { name: String, args: Vec<Type> },

    // test-ratchet: M5 P3b adds BuiltinArray, BuiltinFixed, Maybe for collection types.

    /// Heap-allocated growable list: `array<T>` in source.
    /// Element type is concrete at typeck time.
    BuiltinArray { elem: Box<Type> },

    /// Stack-allocated fixed-size list: `fixed<T>` in source.
    /// `size` is `Some(n)` when the literal element count is known at typeck time
    /// (for literal-OOB checking), `None` when only the annotation is present.
    BuiltinFixed { elem: Box<Type>, size: Option<usize> },

    /// Built-in optional type: `maybe<T>` in source.
    /// Distinct from `Type::Generic` because `.value` has special flow-sensitive
    /// enforcement rules that built-in generic shapes do not have.
    Maybe { inner: Box<Type> },

    // test-ratchet: M5 P3c adds BuiltinMap and MapEntry for map<K,V> support.

    /// Built-in hash map: `map<K, V>` in source.
    /// Uses Swiss Tables + SipHash-2-4 at runtime (M5 P4b codegen).
    BuiltinMap { key: Box<Type>, val: Box<Type> },

    /// Synthetic MapEntry<K, V> type produced during `for (entry in m)` iteration.
    /// Not user-constructable directly — only produced by map iteration.
    /// `entry.key: K`, `entry.value: V` field access is valid.
    MapEntry { key: Box<Type>, val: Box<Type> },

    // ── M6 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M6 adds Options and Union.

    /// A named options type: `options Status { active, inactive, banned }`.
    ///
    /// Values carry an `i8` tag; comparison is `icmp eq i8` at codegen time.
    /// `.toString()` returns the variant name as a string.
    Options { name: String },

    /// A union type: `Circle | Square | Triangle`.
    ///
    /// LLVM layout chosen per variant set at codegen time (pointer-niche or
    /// tagged-struct — see `design/unions.md`). Type narrowing via `is` arms
    /// and `if (x is Foo)` conditions.
    Union { variants: Vec<Type> },

    // ── M7 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M7 P3a adds ErrorsCapable for errors-keyword fallible types.

    /// Return type of a function marked `errors`: carries the success value on success,
    /// or an error value on failure. Flow-sensitive: the type narrows to `inner` after
    /// the caller checks `.failed()` or auto-propagation fires at first use.
    ErrorsCapable { inner: Box<Type> },
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
        Type::TypeParam { name } => name.clone(),
        Type::Generic { name, args } => {
            let arg_str = args.iter().map(type_name).collect::<Vec<_>>().join(", ");
            format!("{name}<{arg_str}>")
        }
        Type::BuiltinArray { elem } => format!("array<{}>", type_name(elem)),
        Type::BuiltinFixed { elem, .. } => format!("fixed<{}>", type_name(elem)),
        Type::Maybe { inner } => format!("maybe<{}>", type_name(inner)),
        Type::BuiltinMap { key, val } => format!("map<{}, {}>", type_name(key), type_name(val)),
        Type::MapEntry { key, val } => format!("MapEntry<{}, {}>", type_name(key), type_name(val)),
        Type::Options { name } => name.clone(),
        Type::Union { variants } => {
            variants.iter().map(type_name).collect::<Vec<_>>().join(" | ")
        }
        Type::ErrorsCapable { inner } => format!("{} errors", type_name(inner)),
    }
}
