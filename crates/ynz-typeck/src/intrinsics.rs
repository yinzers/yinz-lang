use crate::types::Type;

/// A free-standing function signature (not method dispatch).
#[derive(Clone, Debug)]
pub struct FreeFnSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

/// The table of primitive intrinsics available in M2.
///
/// Two categories:
///   1. Free-standing calls — `print` (polymorphic over all primitive types).
///   2. Method calls — `.toNumber()`, `.toFloat()`, `.toString()` on primitive types.
///
/// Single source of truth: no scattered hardcoded method-name lists exist
/// anywhere else in the type checker.
pub struct PrimitiveIntrinsicTable {
    /// Types that `print` accepts as its single argument.
    print_types: Vec<Type>,
    /// Method dispatch: `(receiver_type, method_name)` → return type.
    methods: Vec<(Type, &'static str, Type)>,
    /// Test-only free-standing functions added via `with_test_intrinsic`.
    #[cfg(test)]
    test_fns: Vec<(&'static str, FreeFnSig)>,
}

impl PrimitiveIntrinsicTable {
    pub fn m2() -> Self {
        Self {
            print_types: vec![
                Type::String,
                Type::Int,
                Type::Float,
                Type::Number { precision: 34 },
                Type::Bool,
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
