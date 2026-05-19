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
            size: None,
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
                size: None,
            })
        }
        "append" | "prepend" | "concat" => Some(Type::BuiltinFixed {
            elem: Box::new(elem),
            size: None,
        }),
        "freeze" => Some(Type::BuiltinFixed {
            elem: Box::new(elem),
            size: None,
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
