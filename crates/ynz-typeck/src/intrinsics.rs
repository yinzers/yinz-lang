use crate::types::Type;

// All data lives in registry/features.toml — edit that file to add or remove intrinsics.
// Schema reference: design/feature-registry.md #primitive_intrinsic

/// A free-standing function signature (not method dispatch).
#[derive(Clone, Debug)]
pub struct FreeFnSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

/// The table of primitive intrinsics.
///
/// Built at construction time from `ynz_registry::primitive_intrinsics()`.
/// Public API is identical to the pre-migration version so callers don't change.
pub struct PrimitiveIntrinsicTable {
    print_types: Vec<Type>,
    pub free_fns: Vec<(&'static str, FreeFnSig)>,
    methods: Vec<(Type, &'static str, Type)>,
    methods_1arg: Vec<(Type, &'static str, Type, Type)>,
    #[cfg(test)]
    test_fns: Vec<(&'static str, FreeFnSig)>,
}

/// Parse a registry `return_type` / `receiver_type` string into a `Type` enum value.
///
/// Covers every type string that appears in `registry/features.toml` `[[primitive_intrinsic]]` entries.
fn parse_type(s: &str) -> Type {
    match s {
        "int" => Type::Int,
        "float" => Type::Float,
        "number" => Type::Number { precision: 34 },
        "bool" => Type::Bool,
        "string" => Type::String,
        "nothing" => Type::Nothing,
        "range<int>" => Type::Range {
            element: Box::new(Type::Int),
            end_inclusive: false,
        },
        s if s.starts_with("maybe<") && s.ends_with('>') => Type::Maybe {
            inner: Box::new(parse_type(&s[6..s.len() - 1])),
        },
        s if s.starts_with("array<") && s.ends_with('>') => Type::BuiltinArray {
            elem: Box::new(parse_type(&s[6..s.len() - 1])),
        },
        other => {
            panic!("intrinsics: unknown type string in registry: {other:?} — update parse_type()")
        }
    }
}

/// Build a table from all registry entries whose `since` matches the predicate.
fn build_table(since_filter: impl Fn(&str) -> bool) -> PrimitiveIntrinsicTable {
    let mut print_types = Vec::new();
    let mut free_fns: Vec<(&'static str, FreeFnSig)> = Vec::new();
    let mut methods = Vec::new();
    let mut methods_1arg = Vec::new();

    for entry in ynz_registry::primitive_intrinsics() {
        if !since_filter(entry.since) {
            continue;
        }
        match entry.kind {
            "print_type" => {
                let ty = parse_type(entry.name);
                if !print_types.contains(&ty) {
                    print_types.push(ty);
                }
            }
            "free_fn" => {
                free_fns.push((
                    entry.name,
                    FreeFnSig {
                        params: entry.param_types.iter().map(|&s| parse_type(s)).collect(),
                        ret: parse_type(entry.return_type),
                    },
                ));
            }
            "method" => {
                let recv = parse_type(entry.receiver_type.unwrap_or_else(|| {
                    panic!(
                        "registry method entry '{}' missing receiver_type",
                        entry.name
                    )
                }));
                methods.push((recv, entry.name, parse_type(entry.return_type)));
            }
            "method_1arg" => {
                let recv = parse_type(entry.receiver_type.unwrap_or_else(|| {
                    panic!(
                        "registry method_1arg entry '{}' missing receiver_type",
                        entry.name
                    )
                }));
                assert_eq!(
                    entry.param_types.len(),
                    1,
                    "method_1arg '{}' must have exactly 1 param_type in registry",
                    entry.name
                );
                methods_1arg.push((
                    recv,
                    entry.name,
                    parse_type(entry.param_types[0]),
                    parse_type(entry.return_type),
                ));
            }
            other => panic!(
                "intrinsics: unknown kind {other:?} in registry for entry '{}'",
                entry.name
            ),
        }
    }

    PrimitiveIntrinsicTable {
        print_types,
        free_fns,
        methods,
        methods_1arg,
        #[cfg(test)]
        test_fns: Vec::new(),
    }
}

impl PrimitiveIntrinsicTable {
    /// Build the table with all entries up to and including M3 (and M4 catch-up).
    ///
    /// M6 additions (fallible conversions, string→numeric) are excluded.
    /// Callers that need the full current set should use `m6()`.
    pub fn m2() -> Self {
        Self::m3()
    }

    pub fn m3() -> Self {
        build_table(|since| since != "M6")
    }

    /// Build the full table with all intrinsic entries.
    pub fn m6() -> Self {
        build_table(|_| true)
    }

    /// Look up a `range` overload by argument count.
    pub fn lookup_free_fn(&self, name: &str, arg_count: usize) -> Option<&FreeFnSig> {
        self.free_fns
            .iter()
            .find(|(n, sig)| *n == name && sig.params.len() == arg_count)
            .map(|(_, sig)| sig)
    }

    /// All free-function names (for Levenshtein suggestions on undefined-function errors).
    pub fn free_fn_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.free_fns.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Whether `ty` is a type that `print` can accept.
    pub fn is_print_type(&self, ty: &Type) -> bool {
        self.print_types.contains(ty)
    }

    /// Look up a zero-arg method call: `receiver_type.method_name()`.
    pub fn lookup_method(&self, receiver: &Type, name: &str) -> Option<Type> {
        self.methods
            .iter()
            .find(|(r, n, _)| r == receiver && *n == name)
            .map(|(_, _, ret)| ret.clone())
    }

    /// Look up a one-arg method call: `receiver_type.method_name(arg)`.
    pub fn lookup_method_1arg(&self, receiver: &Type, name: &str) -> Option<(&Type, Type)> {
        self.methods_1arg
            .iter()
            .find(|(r, n, _, _)| r == receiver && *n == name)
            .map(|(_, _, arg_ty, ret_ty)| (arg_ty, ret_ty.clone()))
    }

    /// All method names available on `ty`, used in "did you mean" suggestions.
    pub fn methods_for_type(&self, ty: &Type) -> Vec<&'static str> {
        self.methods
            .iter()
            .filter(|(r, _, _)| r == ty)
            .map(|(_, n, _)| *n)
            .collect()
    }

    #[cfg(test)]
    pub fn lookup_test_fn(&self, name: &str) -> Option<&FreeFnSig> {
        self.test_fns
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, sig)| sig)
    }

    #[cfg(test)]
    pub fn with_test_intrinsic(mut self, name: &'static str, params: Vec<Type>, ret: Type) -> Self {
        self.test_fns.push((name, FreeFnSig { params, ret }));
        self
    }
}
