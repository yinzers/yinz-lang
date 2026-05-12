use crate::types::Type;

/// The signature of a built-in function.
#[derive(Clone, Debug)]
pub struct BuiltinSig {
    /// Parameter types in order.
    pub params: Vec<Type>,
    /// Return type.
    pub ret: Type,
}

/// The table of built-in functions available in M1.
///
/// `print` writes its argument followed by a newline (`\n`) to stdout.
/// This trailing-newline behaviour is the println-style semantics, locked in
/// Phase 5 because M1 codegen relies on libc `puts` which appends `\n`.
pub struct BuiltinTable {
    entries: Vec<(&'static str, BuiltinSig)>,
}

impl BuiltinTable {
    pub fn m1() -> Self {
        Self {
            entries: vec![(
                "print",
                BuiltinSig {
                    params: vec![Type::String],
                    ret: Type::Nothing,
                },
            )],
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&BuiltinSig> {
        self.entries
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, sig)| sig)
    }

    /// Add a test-only builtin. Only available in `#[cfg(test)]`.
    ///
    /// This helper exists so M1 type-mismatch paths can be tested without
    /// needing a real type like `int` or `float` in the language surface.
    /// The production binary does NOT include any test builtins.
    #[cfg(test)]
    pub fn with_test_builtin(mut self, name: &'static str, params: Vec<Type>, ret: Type) -> Self {
        self.entries.push((name, BuiltinSig { params, ret }));
        self
    }
}
