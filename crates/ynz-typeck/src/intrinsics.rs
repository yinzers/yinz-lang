use crate::types::Type;

// All data lives in registry/features.toml — edit that file to add or remove intrinsics.
// Schema reference: design/feature-registry.md #primitive_intrinsic

/// A free-standing function signature (not method dispatch).
#[derive(Clone, Debug)]
pub struct FreeFnSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

/// Free-function intrinsic names whose calls may suspend (pause) the calling state machine.
///
/// v0.3-M2 has exactly two such intrinsics. v0.3-M3 will replace this set with a transitive
/// call-graph analysis pass — at that point `M2_MAY_BLOCK_INTRINSICS` and `is_may_block_callee`
/// can be deleted and replaced with the transitive predicate.
///
/// Legitimate deferral: a 2-element list does not justify a `FreeFnSig` `may_block` field +
/// registry schema migration. The cost of the deferral is concrete: every new may-block intrinsic
/// added before M3 must explicitly edit this const (caught at code review). That is cheaper than
/// schema migration for a list that M3 makes obsolete.
// CARVE-OUT: compiler-internal constant; M2 predicate only; M3 replaces via call-graph analysis.
pub const M2_MAY_BLOCK_INTRINSICS: &[&str] = &["sleepAsync", "__testFallibleAsync"];

/// Returns true when a call to `callee_name` may pause the calling state machine.
///
/// The M2 predicate has two signals:
/// 1. Callee is a known may-block intrinsic (in `M2_MAY_BLOCK_INTRINSICS`).
/// 2. Callee is a user-defined function whose body syntactically contains `wait`
///    (the `FunctionSig.contains_wait` flag — see `signatures.rs`).
///
/// NOT transitive: if `foo()` calls `bar()` and `bar()` calls `sleepAsync(100)` without
/// `wait`, then `bar.contains_wait == false` and `bar` is NOT flagged as may-block in M2.
/// M3 ships the transitive predicate.
pub fn is_may_block_callee(callee_name: &str, callee_contains_wait: bool) -> bool {
    M2_MAY_BLOCK_INTRINSICS.contains(&callee_name) || callee_contains_wait
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
    /// Internal-only intrinsics (e.g., `__testFallibleAsync`) — not surfaced in LSP
    /// completion, not registered in `registry/features.toml`, only callable via
    /// `lookup_free_fn_including_internal`. NOT `#[cfg(test)]` because production
    /// typeck must resolve these names in driver fixtures.
    internal_fns: Vec<(&'static str, FreeFnSig)>,
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
        // Generic collection types use bare base names in the registry so the LSP
        // completion system can match them via prefix stripping. Their type-checker
        // dispatch is handled by builtins.rs pattern matching, not by this scalar
        // intrinsic table — skip them here to avoid parse_type() panics.
        if matches!(
            entry.receiver_type,
            Some("array") | Some("fixed") | Some("maybe") | Some("map")
        ) {
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
        internal_fns: Vec::new(),
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

    /// Register v0.3-M2 internal intrinsics (`__testFallibleAsync`) on an existing table.
    ///
    /// Pure-additive builder: does NOT modify the existing `m1()`/`m2()`/`m3()`/`m6()` builders
    /// or any of their call sites. Attach after construction when internal intrinsics are needed:
    ///   `PrimitiveIntrinsicTable::m2().with_m2_internals()`
    ///
    /// Internal intrinsics are excluded from `free_fn_names()` so they never appear in LSP
    /// completion, and are excluded from `registry/features.toml` by design.
    pub fn with_m2_internals(mut self) -> Self {
        // `__testFallibleAsync(bool) -> int errors` — internal may-block intrinsic for M2
        // state-machine test fixtures. The `errors` return type maps to `Type::ErrorsCapable`.
        // NOT registered in registry/features.toml; NOT surfaced in LSP completion.
        self.internal_fns.push((
            "__testFallibleAsync",
            FreeFnSig {
                params: vec![Type::Bool],
                ret: Type::ErrorsCapable {
                    inner: Box::new(Type::Int),
                },
            },
        ));
        self
    }

    /// Look up a free function by name and argument count, including internal intrinsics.
    ///
    /// # USAGE GUARD
    /// Only call this from M2 state-machine test fixtures or callee dispatch for names
    /// registered in `M2_MAY_BLOCK_INTRINSICS`. Production user-code paths should call
    /// `lookup_free_fn` (which excludes `internal_fns`) — `internal_fns` are never
    /// visible in user source or LSP completion.
    #[doc(hidden)]
    pub fn lookup_free_fn_including_internal(
        &self,
        name: &str,
        arg_count: usize,
    ) -> Option<&FreeFnSig> {
        self.lookup_free_fn(name, arg_count).or_else(|| {
            self.internal_fns
                .iter()
                .find(|(n, sig)| *n == name && sig.params.len() == arg_count)
                .map(|(_, sig)| sig)
        })
    }

    /// Look up a free function by argument count (public registry entries only).
    pub fn lookup_free_fn(&self, name: &str, arg_count: usize) -> Option<&FreeFnSig> {
        self.free_fns
            .iter()
            .find(|(n, sig)| *n == name && sig.params.len() == arg_count)
            .map(|(_, sig)| sig)
    }

    /// All free-function names visible to users (for Levenshtein suggestions and LSP completion).
    ///
    /// Excludes `internal_fns` by design — `__testFallibleAsync` and similar internal
    /// intrinsics must not appear in autocomplete or "did you mean" suggestions.
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
