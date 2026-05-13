use crate::types::Type;

/// A free-standing function signature (not method dispatch).
#[derive(Clone, Debug)]
pub struct FreeFnSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

/// The table of primitive intrinsics available in M3.
///
/// Three categories:
///   1. Free-standing polymorphic calls — `print` (accepts all primitive types).
///   2. Fixed-signature free-standing calls — `range` (checked via `free_fns`).
///   3. Method calls — `.toNumber()`, `.toFloat()`, `.toString()` on primitive types.
///
/// Single source of truth: no scattered hardcoded function-name lists exist
/// anywhere else in the type checker.
pub struct PrimitiveIntrinsicTable {
    /// Types that `print` accepts as its single argument.
    print_types: Vec<Type>,
    /// Fixed-signature free-standing functions: (name, sig).
    /// Multiple entries with the same name represent overloads (e.g. `range` with 1 or 2 args).
    pub free_fns: Vec<(&'static str, FreeFnSig)>,
    /// Method dispatch: `(receiver_type, method_name)` → return type.
    methods: Vec<(Type, &'static str, Type)>,
    /// Test-only free-standing functions added via `with_test_intrinsic`.
    #[cfg(test)]
    test_fns: Vec<(&'static str, FreeFnSig)>,
}

impl PrimitiveIntrinsicTable {
    pub fn m2() -> Self {
        Self::m3()
    }

    pub fn m3() -> Self {
        let range_ret = || Type::Range { element: Box::new(Type::Int), end_inclusive: false };
        Self {
            print_types: vec![
                Type::String,
                Type::Int,
                Type::Float,
                Type::Number { precision: 34 },
                Type::Bool,
            ],
            free_fns: vec![
                // range(end) — 0..end exclusive
                ("range", FreeFnSig { params: vec![Type::Int], ret: range_ret() }),
                // range(start, end) — start..end exclusive
                ("range", FreeFnSig { params: vec![Type::Int, Type::Int], ret: range_ret() }),
            ],
            methods: vec![
                // int conversions
                (Type::Int, "toNumber", Type::Number { precision: 34 }),
                (Type::Int, "toFloat", Type::Float),
                (Type::Int, "toString", Type::String),
                // number conversions
                (Type::Number { precision: 34 }, "toFloat", Type::Float),
                (Type::Number { precision: 34 }, "toString", Type::String),
                // float conversions
                (Type::Float, "toNumber", Type::Number { precision: 34 }),
                (Type::Float, "toString", Type::String),
                // bool conversion
                (Type::Bool, "toString", Type::String),
            ],
            #[cfg(test)]
            test_fns: Vec::new(),
        }
    }

    /// Look up a `range` overload by argument count.
    ///
    /// Returns the matching `FreeFnSig` if one exists, or `None` on arity mismatch.
    pub fn lookup_free_fn(&self, name: &str, arg_count: usize) -> Option<&FreeFnSig> {
        self.free_fns
            .iter()
            .find(|(n, sig)| *n == name && sig.params.len() == arg_count)
            .map(|(_, sig)| sig)
    }

    /// All free-function names (for Levenshtein suggestions on undefined-function errors).
    pub fn free_fn_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> =
            self.free_fns.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Whether `ty` is a type that `print` can accept.
    pub fn is_print_type(&self, ty: &Type) -> bool {
        self.print_types.contains(ty)
    }

    /// Look up a method call: `receiver_type.method_name()`.
    ///
    /// Returns the return type if found, `None` if the method doesn't exist on this type.
    pub fn lookup_method(&self, receiver: &Type, name: &str) -> Option<Type> {
        self.methods
            .iter()
            .find(|(r, n, _)| r == receiver && *n == name)
            .map(|(_, _, ret)| ret.clone())
    }

    /// All method names available on `ty`, used in "did you mean" suggestions.
    pub fn methods_for_type(&self, ty: &Type) -> Vec<&'static str> {
        self.methods
            .iter()
            .filter(|(r, _, _)| r == ty)
            .map(|(_, n, _)| *n)
            .collect()
    }

    /// Look up a test-only free-standing function.
    #[cfg(test)]
    pub fn lookup_test_fn(&self, name: &str) -> Option<&FreeFnSig> {
        self.test_fns
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, sig)| sig)
    }

    /// Add a test-only free-standing function to the table.
    ///
    /// Used to test type-mismatch paths without needing full language features.
    /// The production binary never includes test intrinsics.
    #[cfg(test)]
    pub fn with_test_intrinsic(
        mut self,
        name: &'static str,
        params: Vec<Type>,
        ret: Type,
    ) -> Self {
        self.test_fns.push((name, FreeFnSig { params, ret }));
        self
    }
}
