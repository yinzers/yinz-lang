use crate::types::Type;

/// Look up the return type of a method call on `array<elem>`.
///
/// Returns `None` if the method does not exist on arrays.
pub fn array_method_return(method: &str, elem: &Type) -> Option<Type> {
    let elem = elem.clone();
    match method {
        "add" | "remove" | "removeFirst" | "removeLast" | "clear" => Some(Type::Nothing),
        "get" | "first" | "last" | "find" => Some(Type::Maybe { inner: Box::new(elem) }),
        "set" => Some(Type::Nothing),
        "count" => Some(Type::Int),
        "contains" => Some(Type::Bool),
        "sort" | "sortFast" | "sortStrict" | "filter" | "unique" | "limit" | "copy" => {
            Some(Type::BuiltinArray { elem: Box::new(elem) })
        }
        // Closure return type depends on the closure; return BuiltinArray<Error> as placeholder.
        "map" => Some(Type::BuiltinArray { elem: Box::new(Type::Error) }),
        "concat" | "append" | "prepend" => Some(Type::BuiltinArray { elem: Box::new(elem) }),
        "freeze" => Some(Type::BuiltinFixed { elem: Box::new(elem), size: None }),
        _ => None,
    }
}

/// Check whether a method on `array<T>` mutates (i.e., needs `lend self`).
pub fn array_method_is_mutating(method: &str) -> bool {
    matches!(method, "add" | "remove" | "removeFirst" | "removeLast" | "set" | "clear")
}

/// Look up the return type of a method call on `fixed<elem>`.
pub fn fixed_method_return(method: &str, elem: &Type) -> Option<Type> {
    let elem = elem.clone();
    match method {
        "get" | "first" | "last" | "find" => Some(Type::Maybe { inner: Box::new(elem) }),
        "set" => Some(Type::Nothing),
        "count" => Some(Type::Int),
        "contains" => Some(Type::Bool),
        "sort" | "sortFast" | "sortStrict" | "filter" | "unique" | "limit" | "copy" => {
            Some(Type::BuiltinFixed { elem: Box::new(elem), size: None })
        }
        "append" | "prepend" | "concat" => Some(Type::BuiltinFixed { elem: Box::new(elem), size: None }),
        "freeze" => Some(Type::BuiltinFixed { elem: Box::new(elem), size: None }),
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
