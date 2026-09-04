/// The types known to the M7 type checker.
///
/// Variant count is pinned by `m4_type_variant_count_locked` in tests.
/// Current count: 22 (v0.3-M4 adds BuiltinChannel + BackgroundHandle; total 22 across M1–v0.3-M4)
///
/// `Ord` exists so `Type` can key ordered containers (`MonoKey` → `BTreeMap` in the
/// monomorphization table) — iteration order there reaches LLVM emission order, and an
/// unordered container made multi-file builds nondeterministic (v0.3-M7 R10). The derived
/// order is arbitrary but stable; nothing may attach semantic meaning to it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    BuiltinFixed {
        elem: Box<Type>,
        size: Option<usize>,
    },

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

    // ── v0.3-M4 ────────────────────────────────────────────────────────────────

    // test-ratchet: v0.3-M4 adds BuiltinChannel for bounded task-communication channels.
    /// Bounded task-communication channel: `channel<T>` in source.
    ///
    /// A `channel<T>` value is a heap-owned bounded `tokio::sync::mpsc` channel — an opaque
    /// pointer at the ABI (like `array`/`map`). Constructed with `channel<T>()` (default
    /// capacity 64) or `channel<T>(N)`; bounded by construction, no unbounded constructor
    /// (stdlib-design Rule 4). Phase 1 ships construction + typeck only; the suspending
    /// `.send()`/`.receive()` method surface is Phase 2 (they route through the state-machine
    /// suspension protocol, which is why they are gated to the phase whose two-task composed
    /// fixture can end-to-end-verify the suspend→resume codegen — FRAGO 004).
    BuiltinChannel { elem: Box<Type> },

    // test-ratchet: v0.3-M4 Phase 2 adds BackgroundHandle for `let h = background fn()`.
    /// A background task handle: the value of `let h = background fn(...)`.
    ///
    /// Inferred-only in v0.3 — there is no typeable source annotation for it (the binding's
    /// type comes from the spawn expression). An opaque pointer at the ABI (like `channel`).
    ///
    /// - `result` is the spawned function's SUCCESS type (the `T` of `-> T errors`, or the
    ///   plain return type). `h.receive()` returns `T errors` — "the next thing from the
    ///   task": a message reply from a long-running task, or the task's own completion value
    ///   as its final delivery (one surface, not two APIs).
    /// - `msg_elem` is the element type of the spawned function's FIRST `channel<T>`
    ///   parameter, when it has one — the conduit `h.send(v)` feeds. `None` when the spawned
    ///   function takes no channel (then `h.send()` is a compile error: the task has no way
    ///   to receive messages).
    BackgroundHandle {
        result: Box<Type>,
        msg_elem: Option<Box<Type>>,
    },

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

    // ── M8 ───────────────────────────────────────────────────────────────────

    // test-ratchet: M8 P4 adds Sensitive for the sensitive type modifier.
    /// A sensitive value: auto-redacts in `print()` and string interpolation.
    ///
    /// Only wraps `string` in v0.1. `.reveal()` strips the modifier.
    /// `.length` / `.count` / boolean methods return non-sensitive types per spec.
    Sensitive { inner: Box<Type> },
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
        Type::Bool => "boolean".into(),
        Type::Range { .. } => "range".into(),
        Type::Shape { name } if name.starts_with("__anon__") => {
            // Render canonical anon-shape names as `{ field: type, ... }` for diagnostics.
            // Format: `__anon__fieldname__typename__fieldname__typename...`
            // (each name__type pair joined by `__`, prefix `__anon__`)
            let body = &name["__anon__".len()..];
            let parts: Vec<&str> = body.split("__").collect();
            let mut fields: Vec<String> = Vec::new();
            let mut i = 0;
            while i + 1 < parts.len() {
                fields.push(format!("{}: {}", parts[i], parts[i + 1]));
                i += 2;
            }
            if fields.is_empty() {
                return name.clone();
            }
            format!("{{ {} }}", fields.join(", "))
        }
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
        Type::BuiltinChannel { elem } => format!("channel<{}>", type_name(elem)),
        // Inferred-only in v0.3 (not typeable source syntax); lowercase per the
        // built-in-types-are-lowercase convention (`array`, `map`, `channel`).
        Type::BackgroundHandle { result, .. } => {
            format!("background handle<{} errors>", type_name(result))
        }
        Type::Options { name } => name.clone(),
        Type::Union { variants } => variants
            .iter()
            .map(type_name)
            .collect::<Vec<_>>()
            .join(" | "),
        Type::ErrorsCapable { inner } => format!("{} errors", type_name(inner)),
        Type::Sensitive { inner } => format!("sensitive {}", type_name(inner)),
    }
}

/// True when a value of this type is passed by copy (no heap reference, no aliasing).
///
/// Trivially-copyable types — `int`, `float`, `boolean`, `number` — fit in a machine
/// register and are copied bit-for-bit when passed to a function. A copy cannot alias the
/// caller's value, so a callee that "writes" a copied scalar parameter never affects the
/// caller. Every other type (`shape`, `array`, `map`, `maybe`, union, `dynamic`, strings,
/// etc.) is a heap reference: passing it shares the underlying object, so a `lend`/`give`
/// write through it IS observable at the call site.
///
/// The auto-parallelization independence analysis uses this to decide whether a call
/// argument is a potential aliased write: only heap-typed, non-`share` arguments can be.
pub fn is_trivially_copyable(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::Bool)
        // N ≤ 34: decimal128, an i128 value — trivially copyable. N > 34: bignum, a pointer
        // to a heap decimal string (`llvm_type_for`, `ynz-codegen/src/emit.rs:2161-2162`) — a
        // bit-copy would alias the pointer, not copy the string. Deliberately not `Number { .. }`
        // (v0.3-M8 Phase 4 fix round 3, should-fix 5): bignum is unreachable from source today
        // (the parser defers non-34 precision), so this arm was never exercised, but codegen's
        // `copy_lowering_arm` must classify it the same way — `copy_parity_tests` in
        // `ynz-codegen/src/emit.rs` holds the two to that agreement.
        || matches!(ty, Type::Number { precision } if *precision <= 34)
}

// ── v0.3-M8 Phase 4: the ONE channel element-kind classification ─────────────
//
// `IMP-concurrency.md` "Which element types" / "Two mechanisms, one rule": the set of
// channel element kinds whose payload the runtime must free is defined ONCE, here, as an
// `Option<enum>` — typeck reads it for the transfer decision (`transfers_source`), codegen
// reads it for the drop-glue function (an exhaustive match whose arms are function values,
// never a nullable pointer), and `check_channel_construction`'s admitted set is DERIVED from
// it (`channel_elem_supported`). Adding an element kind is a new variant; the compiler then
// asks both questions of it — where the glue is, and whether the source binding is consumed
// — and neither can be satisfied with a null or a default (authoritative-derivation.md).

/// A channel element kind whose payload carries a runtime-owned heap allocation the
/// channel must free (at teardown, on a purged/refused send) — i.e. the element kinds for
/// which `ynz_channel_create` registers drop glue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelElemDrop {
    /// `array<T>`: the payload IS the sender's allocation (`ynz_array_drop`).
    Array,
    /// `map<K, V>`: the payload IS the sender's allocation (`ynz_map_drop`).
    Map,
    /// `number` (decimal128, precision ≤ 34): the payload is a fresh 16-byte cell the SEND
    /// mints (`number_to_heap_cell`) and the RECEIVE frees — never the sender's own storage
    /// (fr12, `IMP-concurrency.md`). Glue: `ynz_number_cell_free`.
    NumberCell,
}

impl ChannelElemDrop {
    /// Does sending a value of this kind transfer the SOURCE binding — i.e. is the payload
    /// the sender's own allocation, so the binding must be consumed at the send? `Array`/`Map`
    /// hand over their allocation; a `NumberCell` is minted at the send, so the sender's
    /// `number` binding stays its own 16 bytes and remains usable (copy-through, like `int`).
    /// Exhaustive so a new kind must answer both questions.
    pub fn transfers_source(self) -> bool {
        match self {
            ChannelElemDrop::Array | ChannelElemDrop::Map => true,
            ChannelElemDrop::NumberCell => false,
        }
    }
}

/// THE classification of a channel element type into its drop kind (`None` = no
/// runtime-owned payload: `int`/`float`/`bool` value bits, or `string`'s immortal bytes).
pub fn channel_elem_drop(elem: &Type) -> Option<ChannelElemDrop> {
    match elem {
        Type::BuiltinArray { .. } => Some(ChannelElemDrop::Array),
        Type::BuiltinMap { .. } => Some(ChannelElemDrop::Map),
        Type::Number { precision } if *precision <= 34 => Some(ChannelElemDrop::NumberCell),
        _ => None,
    }
}

/// The element types `channel<T>()` admits — DERIVED from [`channel_elem_drop`] (glue-bearing
/// kinds) plus the value-bit scalars and `string`, never a third hand-maintained list.
/// `shape` elements and bignum `number` (precision > 34) stay rejected
/// (`channel-element-heap-upgrade` in the feature registry).
pub fn channel_elem_supported(elem: &Type) -> bool {
    matches!(elem, Type::Int | Type::Float | Type::Bool | Type::String)
        || channel_elem_drop(elem).is_some()
}

/// The user-facing names of the element types [`channel_elem_supported`] admits, in the
/// order the teaching text lists them ("Use one of: …"). Held to the predicate by
/// `channel_elem_supported_names_match_the_predicate` below — the prose list at the
/// construction check is rendered FROM this constant, never typed a second time.
pub const CHANNEL_ELEM_SUPPORTED_NAMES: &[&str] = &[
    "int",
    "float",
    "boolean",
    "string",
    "number",
    "array<T>",
    "map<K, V>",
];

/// Is `.copy()` on a value of this type a genuinely INDEPENDENT copy (a fresh allocation
/// nobody else reaches), so provenance may classify the result `Fresh`?
///
/// Held to parity with codegen's `copy_lowering_arm` (`ynz_codegen::emit`, the ONE
/// classification the `PostfixOpKind::Copy` lowering dispatches on) by
/// `copy_parity_tests::copy_is_independent_matches_the_copy_lowering_arm_for_every_type_variant`
/// over one sample of every `Type` variant: `true` here ⇔ the lowering yields an independent
/// value — `array` (`ynz_array_clone_primitive` / SoA gather), `map` (`ynz_map_clone`,
/// v0.3-M8 step 3a), an inline `shape` (memcpy into a fresh alloca), and the value-bit
/// primitives / immortal `string` bytes, which are already by-value. Every other type is
/// codegen's alias no-op (the FR#10 stub class — `maybe`, union, `fixed`, `dynamic`, …), so its
/// `.copy()` is `Unknown` and can never be transferred.
pub fn copy_is_independent(ty: &Type) -> bool {
    is_trivially_copyable(ty)
        || matches!(
            ty,
            Type::String | Type::Shape { .. } | Type::BuiltinArray { .. } | Type::BuiltinMap { .. }
        )
}

/// v0.3-M8 Phase 5 — THE Auto-Arc compile-time floor (`IMP-ownership.md` "The
/// beneficial-emission condition", item 4): can a value of this type be shared across a
/// `background` boundary as ONE refcounted block whose bytes every task reads?
///
/// A `shape` whose fields are all `int`/`float`/`bool`/`string` — the field kinds a byte copy
/// of the struct shares soundly: value bits, or (`string`) a pointer to immutable heap bytes
/// that outlive every frame. Excluded, each for a reason the block cannot express:
/// - `number` fields: 16-byte alignment; the block's data starts 8-aligned (`arc.rs`).
/// - `array`/`map`/`maybe`/union/`channel` fields: pointer cells whose ownership the block
///   does not count — sharing the outer bytes would alias the inner allocation between tasks.
/// - nested `shape` fields: stored as an opaque POINTER inside the struct
///   (`ynz-codegen/src/shape_types.rs::llvm_field_type`), so the same pointer-cell aliasing
///   applies — the design's original "inline shape" wording did not match the tree.
/// - every non-shape type: `array<int>` etc. are a header-plus-buffer pair, a different
///   sharing substrate (the registry's residual).
///
/// One predicate, consulted by the group admission in `check.rs` only; codegen reads the
/// recorded `BgOwnership::Arc` and never re-derives shareability (authoritative-derivation).
pub fn arc_shareable(ty: &Type, shapes: &crate::shapes::ShapeTable) -> bool {
    let Type::Shape { name } = ty else {
        return false;
    };
    shapes.get(name).is_some_and(|def| {
        def.fields
            .iter()
            .all(|f| matches!(f.ty, Type::Int | Type::Float | Type::Bool | Type::String))
    })
}

#[cfg(test)]
mod arc_shareable_tests {
    use super::*;
    use crate::shapes::{FieldDef, ShapeDef, ShapeTable};
    use ynz_diagnostics::SourceSpan;

    fn table_with(name: &str, fields: Vec<Type>) -> ShapeTable {
        let mut t = ShapeTable::empty();
        t.shapes.insert(
            name.to_string(),
            ShapeDef {
                name: name.to_string(),
                is_base: false,
                extends: None,
                follows: vec![],
                fields: fields
                    .into_iter()
                    .enumerate()
                    .map(|(i, ty)| FieldDef {
                        name: format!("f{i}"),
                        ty,
                        is_hidden: false,
                        is_inherited: false,
                        defined_at: SourceSpan::new("", 0, 0),
                    })
                    .collect(),
                contract_sigs: vec![],
                defined_at: SourceSpan::new("", 0, 0),
            },
        );
        t
    }

    #[test]
    fn primitive_and_string_fields_are_shareable() {
        let t = table_with(
            "Scene",
            vec![Type::String, Type::Int, Type::Float, Type::Bool],
        );
        assert!(arc_shareable(
            &Type::Shape {
                name: "Scene".into()
            },
            &t
        ));
    }

    #[test]
    fn every_excluded_field_kind_declines() {
        let excluded = [
            Type::Number { precision: 34 },
            Type::BuiltinArray {
                elem: Box::new(Type::Int),
            },
            Type::BuiltinMap {
                key: Box::new(Type::String),
                val: Box::new(Type::Int),
            },
            Type::Maybe {
                inner: Box::new(Type::Int),
            },
            Type::Shape {
                name: "Inner".into(),
            },
            Type::BuiltinChannel {
                elem: Box::new(Type::Int),
            },
        ];
        for field in excluded {
            let t = table_with("Outer", vec![Type::Int, field.clone()]);
            assert!(
                !arc_shareable(
                    &Type::Shape {
                        name: "Outer".into()
                    },
                    &t
                ),
                "a shape with a {field:?} field must not be Arc-shareable"
            );
        }
    }

    #[test]
    fn non_shape_types_and_unknown_shapes_decline() {
        let t = table_with("Scene", vec![Type::Int]);
        assert!(!arc_shareable(&Type::Int, &t));
        assert!(!arc_shareable(
            &Type::BuiltinArray {
                elem: Box::new(Type::Int)
            },
            &t
        ));
        assert!(!arc_shareable(
            &Type::Shape {
                name: "Missing".into()
            },
            &t
        ));
    }
}

#[cfg(test)]
mod channel_elem_tests {
    use super::*;
    use crate::type_variant_sampler::{all_type_variants, variant_tag};

    /// One sample per supported name, in the constant's order, plus — driven by the ONE
    /// authoritative variant sampler rather than a hand-picked `rejected` list (v0.3-M8 Phase 4
    /// fix round 3, should-fix 3) — every OTHER `Type` variant is asserted rejected:
    /// `supported ⊕ (all variants) == rejected`. Before this, `rejected` was hand-picked here
    /// while `channel_elem_drop` kept a `_ => None` wildcard, so a new variant could slip into
    /// "silently admitted" without either list noticing.
    #[test]
    fn channel_elem_supported_names_match_the_predicate() {
        let supported: Vec<(&str, Type)> = vec![
            ("int", Type::Int),
            ("float", Type::Float),
            ("boolean", Type::Bool),
            ("string", Type::String),
            ("number", Type::Number { precision: 34 }),
            (
                "array<T>",
                Type::BuiltinArray {
                    elem: Box::new(Type::Int),
                },
            ),
            (
                "map<K, V>",
                Type::BuiltinMap {
                    key: Box::new(Type::String),
                    val: Box::new(Type::Int),
                },
            ),
        ];
        let names: Vec<&str> = supported.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names, CHANNEL_ELEM_SUPPORTED_NAMES,
            "the prose list and the sampled predicate must name the same kinds in the same order"
        );
        for (name, ty) in &supported {
            assert!(
                channel_elem_supported(ty),
                "`{name}` is listed as supported but channel_elem_supported({ty:?}) is false"
            );
        }

        // Every variant NOT in the supported set, sampled once each from the one authoritative
        // sampler, must be rejected.
        let supported_tags: std::collections::BTreeSet<&str> =
            supported.iter().map(|(_, ty)| variant_tag(ty)).collect();
        for ty in all_type_variants() {
            let tag = variant_tag(&ty);
            if supported_tags.contains(tag) {
                continue;
            }
            assert!(
                !channel_elem_supported(&ty),
                "{tag} ({ty:?}) is admitted as a channel element but is not one of the \
                 CHANNEL_ELEM_SUPPORTED_NAMES tags — either add it there and to \
                 channel_elem_drop, or channel_elem_drop must keep refusing it"
            );
        }

        // Bignum `number` (precision > 34) is a within-variant edge case the whole-variant
        // sweep above cannot see (it samples one `Number { precision: 34 }`, which IS
        // supported) — the registry deferral (`channel-element-heap-upgrade`) still rejects it.
        assert!(
            !channel_elem_supported(&Type::Number { precision: 40 }),
            "bignum `number` (precision > 34) must stay rejected per the \
             channel-element-heap-upgrade deferral"
        );
    }
}
