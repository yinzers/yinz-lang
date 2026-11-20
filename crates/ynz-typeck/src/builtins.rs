use ynz_diagnostics::{Diagnostic, DiagnosticBucket, SourceSpan};

use crate::types::{type_name, Type};

/// Dispatch a method call on `sensitive T`, returning the result type.
///
/// Implements the M8 propagation table: methods that return string from string
/// produce `sensitive string`; scalar extractions (bool, int) return plain types.
pub fn sensitive_method_return(
    method: &str,
    inner: &Type,
    method_span: &SourceSpan,
    diags: &mut DiagnosticBucket,
) -> Type {
    match method {
        // .reveal() — strips the sensitive modifier, returns the inner type.
        "reveal" => inner.clone(),

        // Boolean predicates — result is NOT sensitive (per spec: "length isn't secret").
        "contains" | "startsWith" | "endsWith" => Type::Bool,

        // Scalar extractions — NOT sensitive.
        "indexOf" => Type::Maybe {
            inner: Box::new(Type::Int),
        },
        "byteAt" | "get" => Type::Maybe {
            inner: Box::new(Type::Int),
        },
        "count" | "byteCount" | "graphemeCount" => Type::Int,

        // String-returning methods — preserve sensitivity.
        "toUpperCase" | "toLowerCase" | "trim" | "substring" | "replace" => Type::Sensitive {
            inner: Box::new(inner.clone()),
        },
        "graphemeAt" => Type::Maybe {
            inner: Box::new(Type::Sensitive {
                inner: Box::new(inner.clone()),
            }),
        },
        "split" => Type::BuiltinArray {
            elem: Box::new(Type::Sensitive {
                inner: Box::new(inner.clone()),
            }),
        },

        // Unknown method — emit an error.
        _ => {
            diags.push(Diagnostic::error(
                method_span.clone(),
                format!(
                    "`sensitive {}` does not have a method called `{method}`.",
                    type_name(inner)
                ),
                "Available: `.reveal()` (returns the raw value), plus all string methods.",
                "Sensitive values support the same methods as their inner type, plus `.reveal()`.",
            ));
            Type::Error
        }
    }
}

/// Look up the return type of a method call on `string`.
///
/// Returns `None` if the method does not exist on strings.
/// Multi-arg string methods (substring, split, replace) use arity checked
/// at the call site; this table only covers 0- and 1-arg dispatch.
pub fn string_method_return(method: &str) -> Option<Type> {
    match method {
        // Zero-arg transformations
        "toUpperCase" => Some(Type::String),
        "toLowerCase" => Some(Type::String),
        "trim" => Some(Type::String),
        "count" => Some(Type::Int),
        "byteCount" => Some(Type::Int),
        "graphemeCount" => Some(Type::Int),
        // One-arg predicates (arg type checked at call site)
        "contains" => Some(Type::Bool),
        "startsWith" => Some(Type::Bool),
        "endsWith" => Some(Type::Bool),
        // One-arg indexed access
        "get" | "graphemeAt" => Some(Type::Maybe {
            inner: Box::new(Type::String),
        }),
        "byteAt" => Some(Type::Maybe {
            inner: Box::new(Type::Int),
        }),
        "indexOf" => Some(Type::Maybe {
            inner: Box::new(Type::Int),
        }),
        // Multi-arg methods: substring(int, int), split(string), replace(string, string)
        "substring" => Some(Type::String),
        "split" => Some(Type::BuiltinArray {
            elem: Box::new(Type::String),
        }),
        "replace" => Some(Type::String),
        _ => None,
    }
}

// CARVE-OUT: string_method_return() is a local dispatch table used by the type checker.
// It is NOT an independent registry — string method names also live in registry/features.toml
// as [[primitive_intrinsic]] entries with receiver_type = "string".

/// Look up the return type of a method call on `array<elem>`.
///
/// Returns `None` if the method does not exist on arrays.
pub fn array_method_return(method: &str, elem: &Type) -> Option<Type> {
    let elem = elem.clone();
    match method {
        "add" | "remove" | "removeFirst" | "removeLast" | "clear" => Some(Type::Nothing),
        "get" | "first" | "last" | "find" => Some(Type::Maybe {
            inner: Box::new(elem),
        }),
        "set" => Some(Type::Nothing),
        "count" => Some(Type::Int),
        "contains" => Some(Type::Bool),
        "sort" | "sortFast" | "sortStrict" | "filter" | "unique" | "limit" | "copy" => {
            Some(Type::BuiltinArray {
                elem: Box::new(elem),
            })
        }
        // Closure return type depends on the closure; return BuiltinArray<Error> as placeholder.
        "map" => Some(Type::BuiltinArray {
            elem: Box::new(Type::Error),
        }),
        "concat" | "append" | "prepend" => Some(Type::BuiltinArray {
            elem: Box::new(elem),
        }),
        "freeze" => Some(Type::BuiltinFixed {
            elem: Box::new(elem),
            size: crate::types::FixedLength::Unknown,
        }),
        _ => None,
    }
}

/// Check whether a method on `array<T>` mutates (i.e., needs `lend self`).
pub fn array_method_is_mutating(method: &str) -> bool {
    matches!(
        method,
        "add" | "remove" | "removeFirst" | "removeLast" | "set" | "clear"
    )
}

/// Look up the return type of a method call on `fixed<elem>`.
pub fn fixed_method_return(method: &str, elem: &Type) -> Option<Type> {
    let elem = elem.clone();
    match method {
        "get" | "first" | "last" | "find" => Some(Type::Maybe {
            inner: Box::new(elem),
        }),
        "set" => Some(Type::Nothing),
        "count" => Some(Type::Int),
        "contains" => Some(Type::Bool),
        "sort" | "sortFast" | "sortStrict" | "filter" | "unique" | "limit" | "copy" => {
            Some(Type::BuiltinFixed {
                elem: Box::new(elem),
                size: crate::types::FixedLength::Unknown,
            })
        }
        "append" | "prepend" | "concat" => Some(Type::BuiltinFixed {
            elem: Box::new(elem),
            size: crate::types::FixedLength::Unknown,
        }),
        "freeze" => Some(Type::BuiltinFixed {
            elem: Box::new(elem),
            size: crate::types::FixedLength::Unknown,
        }),
        _ => None,
    }
}

/// Check whether a method on `fixed<T>` mutates.
pub fn fixed_method_is_mutating(method: &str) -> bool {
    matches!(method, "set")
}

/// Look up the return type of a method call on `maybe<inner>`.
pub fn maybe_method_return(method: &str, inner: &Type) -> Option<Type> {
    match method {
        "exists" => Some(Type::Bool),
        "or" => Some(inner.clone()),
        _ => None,
    }
}

/// Look up the return type of a method call on `map<key, val>`.
///
/// Returns `None` if the method does not exist on maps.
pub fn map_method_return(method: &str, key: &Type, val: &Type) -> Option<Type> {
    match method {
        "get" | "find" => Some(Type::Maybe {
            inner: Box::new(val.clone()),
        }),
        "has" => Some(Type::Bool),
        "set" | "remove" | "clear" => Some(Type::Nothing),
        "count" => Some(Type::Int),
        "keys" => Some(Type::BuiltinArray {
            elem: Box::new(key.clone()),
        }),
        "values" => Some(Type::BuiltinArray {
            elem: Box::new(val.clone()),
        }),
        "entries" => Some(Type::BuiltinArray {
            elem: Box::new(Type::MapEntry {
                key: Box::new(key.clone()),
                val: Box::new(val.clone()),
            }),
        }),
        _ => None,
    }
}

/// Check whether a method on `map<K, V>` mutates (i.e., needs `lend self`).
pub fn map_method_is_mutating(method: &str) -> bool {
    matches!(method, "set" | "remove" | "clear")
}

/// THE table of built-in methods whose RESULT is a fresh value nobody else reaches —
/// consumed by `effective_ownership::provenance` for a heap-typed builtin method result
/// (v0.3-M8 Phase 4, `IMP-ownership.md` "Classification"). The default for a heap-typed
/// builtin result is `Reaches([receiver root])` (`.get`/`.first`/`.last` hand out a cell of
/// the receiver); a method joins this set one at a time WITH EVIDENCE that its codegen
/// returns a fresh allocation, never by default (parked item 18 records the conservative
/// omission of `.sort`/`.filter`/`.map`/`.concat` until each is checked). Value-typed
/// results (`count`, `toString`, `exists`) never consult this table — a value is fresh bits.
///
/// - `receive` — a channel/handle delivery: the receiver's `maybe<T>` is the sole reference
///   (`IMP-concurrency.md` "What this makes sound").
/// - `split` — `ynz_string_split` builds a new array of new strings; no piece of the receiver is
///   shared. Audited with it: of the built-in methods codegen lowers, `split` is the only other
///   one that returns a fresh heap value (the string methods return strings, which `string_ownership`
///   judges; `get`/`first`/`last`/`or`/`reveal` hand out a cell or the receiver).
///
/// (`keys`/`values`/`entries` are typed as fresh arrays but have no codegen lowering yet —
/// "not yet lowered in P4b" — so they carry no evidence and stay out until they do.)
pub fn builtin_method_returns_fresh(method: &str) -> bool {
    matches!(method, "receive" | "split")
}

/// True when the built-in `method` on a `receiver` of this type returns a `maybe<string>` whose string
/// payload is a fresh allocation nobody else holds: `string.get(index)` and `string.graphemeAt(index)`
/// build a new string for the character (`ynz_string_codepoint_at`, `ynz_string_grapheme_at`), unlike
/// `array.get` and `map.get`, whose payload is a cell of the receiver. Such a `maybe` is held the way a
/// `maybe` a Yinz function returned is: the binding takes the payload, and a `maybe` nothing binds frees it
/// (`temporaries`).
///
/// O(1) time.
pub fn builtin_method_returns_fresh_maybe_string(receiver: &Type, method: &str) -> bool {
    let on_a_string = match receiver {
        Type::String => true,
        Type::Sensitive { inner } => matches!(inner.as_ref(), Type::String),
        _ => false,
    };
    on_a_string && matches!(method, "get" | "graphemeAt")
}

/// The typed argument positions of the built-in methods and the type each must have: the
/// collection's element / key / value, `maybe`'s and an `errors` result's inner type, an `int`
/// index, and the `string` methods with two arguments (`substring(start, end)`,
/// `replace(from, to)`; the registry lists them with one). A `sensitive T` has the positions of
/// `T`. Closure positions (`sort`, `filter`, `map`, `find` on arrays), whole-collection
/// positions (`concat`, `append`, `prepend`) and `limit(n)` carry no entry; none of them is
/// lowered yet, so a wrong argument there stops at codegen instead of running.
///
/// The ONE authoritative position table (authoritative-derivation): the `Expr::MethodCall` arm
/// infers each argument with its position's type as the hint, and `check_method_call` runs the
/// int-literal→`number` gate and the plain call form's argument check (`check_argument_type`)
/// on every position listed here. Codegen trusts these types (it reads a `Big` slot from the
/// argument's pointer), so a typed position missing here compiles a wrong argument and misreads
/// memory (C1g rework r3, #424): when a built-in method gains a typed argument, extend THIS
/// table, never a per-arm twin.
///
/// O(1): a match on two short strings and one recursion through `sensitive`.
pub fn builtin_method_argument_slots(receiver: &Type, method: &str) -> Vec<(usize, Type)> {
    match receiver {
        Type::BuiltinArray { elem } => match method {
            "add" | "contains" | "remove" => vec![(0, elem.as_ref().clone())],
            "get" => vec![(0, Type::Int)],
            "set" => vec![(0, Type::Int), (1, elem.as_ref().clone())],
            _ => Vec::new(),
        },
        Type::BuiltinFixed { elem, .. } => match method {
            "contains" => vec![(0, elem.as_ref().clone())],
            "get" => vec![(0, Type::Int)],
            "set" => vec![(0, Type::Int), (1, elem.as_ref().clone())],
            _ => Vec::new(),
        },
        Type::Maybe { inner } | Type::ErrorsCapable { inner } => match method {
            "or" => vec![(0, inner.as_ref().clone())],
            _ => Vec::new(),
        },
        Type::BuiltinMap { key, val } => match method {
            "set" => vec![(0, key.as_ref().clone()), (1, val.as_ref().clone())],
            "get" | "has" | "remove" | "find" => vec![(0, key.as_ref().clone())],
            _ => Vec::new(),
        },
        Type::String => match method {
            "substring" => vec![(0, Type::Int), (1, Type::Int)],
            "replace" => vec![(0, Type::String), (1, Type::String)],
            _ => Vec::new(),
        },
        Type::Sensitive { inner } => builtin_method_argument_slots(inner, method),
        _ => Vec::new(),
    }
}

/// How many arguments (the receiver not counted) the built-in methods of the collection, `maybe`,
/// string, `sensitive`, `errors`-capable and `options` types take, or `None` when this table does
/// not know the method: an unknown name (refused elsewhere, with its own text), `sort` (its
/// arguments are optional) and the methods the registry's `primitive_intrinsic` entries already
/// count (`int`/`float`/`number`/`bool` methods and the string methods with zero or one argument,
/// see `Checker::builtin_method_argument_count`, which asks this table first, then those entries).
/// `sort`, `filter`, `map` and `find` are left out on purpose: their closure arguments are not
/// settled (`sort` takes none, one or two) and the type-check tests call `filter()` with none.
///
/// The ONE argument-count table for built-in methods, beside `builtin_method_argument_slots` (which
/// says WHICH positions are typed and as what; this says HOW MANY positions there are).
/// `check_method_call`'s caller compares it with the call before any lowering, because codegen
/// indexes the argument list by position and a missing argument is a panic, not an error. A
/// method that gains an argument gains its number HERE, never a per-arm check.
///
/// O(1): a match on two short strings.
pub fn builtin_method_argument_count(receiver: &Type, method: &str) -> Option<usize> {
    match receiver {
        Type::BuiltinArray { .. } => match method {
            "removeFirst" | "removeLast" | "clear" | "first" | "last" | "count" | "unique"
            | "copy" | "freeze" => Some(0),
            "add" | "remove" | "get" | "contains" | "limit" | "concat" | "append" | "prepend" => {
                Some(1)
            }
            "set" => Some(2),
            _ => None,
        },
        Type::BuiltinFixed { .. } => match method {
            "first" | "last" | "count" | "unique" | "copy" | "freeze" => Some(0),
            "get" | "contains" | "limit" | "append" | "prepend" | "concat" => Some(1),
            "set" => Some(2),
            _ => None,
        },
        Type::Maybe { .. } => match method {
            "exists" => Some(0),
            "or" => Some(1),
            _ => None,
        },
        Type::BuiltinMap { .. } => match method {
            "clear" | "count" | "keys" | "values" | "entries" => Some(0),
            "get" | "has" | "remove" => Some(1),
            "set" => Some(2),
            _ => None,
        },
        // `substring(start, end)` and `replace(from, to)` take two; the registry lists them with
        // one parameter, so they are counted here and not by the registry.
        Type::String => match method {
            "substring" | "replace" => Some(2),
            _ => None,
        },
        // `sensitive T` has the methods of `T` plus `reveal()` (`sensitive_method_return`).
        Type::Sensitive { inner } => match method {
            "reveal" => Some(0),
            _ => builtin_method_argument_count(inner, method),
        },
        // The members of an `errors`-capable value that are calls: `failed()` and `or(fallback)`
        // (`check_errors_capable_method`; `message` and the other fields are not calls).
        Type::ErrorsCapable { .. } => match method {
            "failed" => Some(0),
            "or" => Some(1),
            _ => None,
        },
        Type::Options { .. } => match method {
            "toString" => Some(0),
            _ => None,
        },
        _ => None,
    }
}

/// THE table of built-in collection methods that STORE an argument into the receiver, and which
/// argument positions they store: `add(v)`, `set(i, v)`, `map.set(k, v)`. A heap value stored
/// this way is held by the receiver from then on, so the transfer record joins the two
/// (`transfer_record` module docs). Read-only methods (`contains`, `get`, `remove`) are absent.
/// A method that stores gains an entry here, never a per-site check.
pub fn collection_store_positions(receiver: &Type, method: &str) -> &'static [usize] {
    match (receiver, method) {
        (Type::BuiltinArray { .. }, "add") => &[0],
        (Type::BuiltinArray { .. }, "set") | (Type::BuiltinFixed { .. }, "set") => &[1],
        (Type::BuiltinMap { .. }, "set") => &[0, 1],
        _ => &[],
    }
}
