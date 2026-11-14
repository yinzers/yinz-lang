use std::collections::{BTreeMap, HashMap, HashSet};

use ynz_ast::nodes::OwnershipModifier;
use ynz_diagnostics::SourceSpan;

use crate::{shapes::FieldDef, types::Type};

// ── Substitution ─────────────────────────────────────────────────────────────

/// Maps type-parameter name → concrete resolved type.
pub type Substitution = HashMap<String, Type>;

/// Unify a parameter type (which may be `TypeParam`) against a concrete argument type.
///
/// On success, updates `subst` with any new bindings.
/// On failure, returns an error string describing the mismatch.
pub fn unify_param(param_ty: &Type, arg_ty: &Type, subst: &mut Substitution) -> Result<(), String> {
    match param_ty {
        Type::TypeParam { name } => match subst.get(name) {
            Some(existing) if existing == arg_ty => Ok(()),
            Some(existing) => Err(format!(
                "two different types for `{name}`: `{}` and `{}`",
                crate::types::type_name(existing),
                crate::types::type_name(arg_ty),
            )),
            None => {
                subst.insert(name.clone(), arg_ty.clone());
                Ok(())
            }
        },
        Type::Generic {
            name: pname,
            args: pargs,
        } => match arg_ty {
            Type::Generic {
                name: aname,
                args: aargs,
            } if pname == aname && pargs.len() == aargs.len() => {
                for (p, a) in pargs.iter().zip(aargs.iter()) {
                    unify_param(p, a, subst)?;
                }
                Ok(())
            }
            _ => Err(format!(
                "expected `{}`, got `{}`",
                crate::types::type_name(param_ty),
                crate::types::type_name(arg_ty),
            )),
        },
        _ => {
            if param_ty == arg_ty {
                Ok(())
            } else {
                Err(format!(
                    "expected `{}`, got `{}`",
                    crate::types::type_name(param_ty),
                    crate::types::type_name(arg_ty),
                ))
            }
        }
    }
}

/// Recursively apply `subst` to `ty`, replacing `TypeParam` leaves with concrete types.
pub fn apply_substitution(ty: &Type, subst: &Substitution) -> Type {
    let apply = |inner: &Type| Box::new(apply_substitution(inner, subst));
    match ty {
        Type::TypeParam { name } => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Generic { name, args } => Type::Generic {
            name: name.clone(),
            args: args.iter().map(|a| apply_substitution(a, subst)).collect(),
        },
        Type::BuiltinArray { elem } => Type::BuiltinArray { elem: apply(elem) },
        Type::BuiltinFixed { elem, size } => Type::BuiltinFixed {
            elem: apply(elem),
            size: *size,
        },
        Type::BuiltinChannel { elem } => Type::BuiltinChannel { elem: apply(elem) },
        Type::Maybe { inner } => Type::Maybe {
            inner: apply(inner),
        },
        Type::ErrorsCapable { inner } => Type::ErrorsCapable {
            inner: apply(inner),
        },
        Type::Sensitive { inner } => Type::Sensitive {
            inner: apply(inner),
        },
        Type::BuiltinMap { key, val } => Type::BuiltinMap {
            key: apply(key),
            val: apply(val),
        },
        Type::MapEntry { key, val } => Type::MapEntry {
            key: apply(key),
            val: apply(val),
        },
        Type::Union { variants } => {
            let substituted: Vec<Type> = variants
                .iter()
                .map(|variant| apply_substitution(variant, subst))
                .collect();
            flatten_union(substituted)
        }
        other => other.clone(),
    }
}

/// The union a substituted union stands for: a variant that became a union itself
/// (`T | nothing` with `T = Big | Tiny`) contributes its variants in place, and a variant
/// that repeats an earlier one (`T | Tiny` with `T = Tiny`) is dropped, so the result is the
/// flat variant list a program writes by hand (`Big | Tiny | nothing`). A union left with one
/// variant is that variant. Without this a nested union is a different type from the flat
/// union holding the same variants, and a correct call is refused with a message that says the
/// parameter expects the very type the argument has (C1g, #424).
///
/// O(total variants squared) time, O(total variants) space.
fn flatten_union(substituted: Vec<Type>) -> Type {
    let mut flat: Vec<Type> = Vec::with_capacity(substituted.len());
    for variant in substituted {
        let contributed = match variant {
            Type::Union { variants } => variants,
            single => vec![single],
        };
        for contributed_variant in contributed {
            if !flat.contains(&contributed_variant) {
                flat.push(contributed_variant);
            }
        }
    }
    if flat.len() == 1 {
        flat.remove(0)
    } else {
        Type::Union { variants: flat }
    }
}

/// Does `ty` still mention a type parameter after substitution? True for a parameter the call
/// never bound (reported by the "cannot work out the type parameter" error), anywhere inside
/// the type. O(size of the type) time, O(depth) stack.
pub fn mentions_type_param(ty: &Type) -> bool {
    match ty {
        Type::TypeParam { .. } => true,
        Type::Generic { args, .. } => args.iter().any(mentions_type_param),
        Type::BuiltinArray { elem }
        | Type::BuiltinFixed { elem, .. }
        | Type::BuiltinChannel { elem } => mentions_type_param(elem),
        Type::Maybe { inner } | Type::ErrorsCapable { inner } => mentions_type_param(inner),
        Type::BuiltinMap { key, val } | Type::MapEntry { key, val } => {
            mentions_type_param(key) || mentions_type_param(val)
        }
        Type::Union { variants } => variants.iter().any(mentions_type_param),
        _ => false,
    }
}

// ── MonomorphizationTable ─────────────────────────────────────────────────────

/// Key into the MonomorphizationTable: the function name + the concrete type args.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MonoKey {
    pub fn_name: String,
    pub type_args: Vec<Type>,
}

/// The concrete signature produced when a generic function is instantiated.
#[derive(Clone, Debug, PartialEq)]
pub struct MonoSignature {
    pub param_types: Vec<Type>,
    pub ret_type: Type,
}

/// All generic instantiations observed during type-checking of a module.
///
/// Keyed by `(fn_name, concrete_type_args)`. Codegen (P4a) consumes this to
/// emit one LLVM function per unique instantiation.
///
/// `BTreeMap`, not `HashMap`: codegen's Pass 1.5/2.5 emission ORDER follows this
/// table's iteration order, which
/// therefore reaches the object bytes. A per-process-seeded `HashMap` here made
/// multi-file builds flap between orderings run-to-run (v0.3-M7 R10 — the
/// pirates-roster two-hash flap). Ordered iteration is load-bearing; do not switch back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonomorphizationTable {
    pub entries: BTreeMap<MonoKey, MonoSignature>,
}

impl MonomorphizationTable {
    pub fn record(&mut self, fn_name: String, type_args: Vec<Type>, sig: MonoSignature) {
        self.entries.insert(MonoKey { fn_name, type_args }, sig);
    }
}

/// The type arguments type checking chose for each generic call, keyed by the call's
/// `(span.start, span.end)`.
///
/// Codegen reads the instance a call runs from here and nowhere else. Re-deriving it from
/// the argument types is ambiguous: `pickA(tiny, big)` against `pickA<T>(first: T | Tiny,
/// second: T)` fits both `pickA<Big>` and `pickA<Big | Tiny>` when both exist, and only the
/// type checker knows which one it checked (C1g, #424).
///
/// A call checked twice with different type arguments is kept as a conflict, never as the
/// last or first choice, so codegen stops with an internal error instead of guessing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenericCallInstances {
    type_args_by_call: HashMap<(usize, usize), Vec<Type>>,
    conflicting_calls: HashSet<(usize, usize)>,
}

impl GenericCallInstances {
    /// Record the type arguments the call at `call_span` was checked with. O(type args) time.
    pub fn record(&mut self, call_span: (usize, usize), type_args: Vec<Type>) {
        match self.type_args_by_call.get(&call_span) {
            Some(recorded) if *recorded != type_args => {
                self.conflicting_calls.insert(call_span);
            }
            Some(_) => {}
            None => {
                self.type_args_by_call.insert(call_span, type_args);
            }
        }
    }

    /// The type arguments the call at `call_span` was checked with. An error when the type
    /// checker recorded none, or recorded two different sets. O(1) time.
    pub fn type_args_at(&self, call_span: (usize, usize)) -> Result<&[Type], String> {
        if self.conflicting_calls.contains(&call_span) {
            return Err(format!(
                "the call at bytes {}..{} was type-checked with two different sets of type \
                 arguments",
                call_span.0, call_span.1
            ));
        }
        self.type_args_by_call
            .get(&call_span)
            .map(Vec::as_slice)
            .ok_or_else(|| {
                format!(
                    "the type checker recorded no type arguments for the call at bytes {}..{}",
                    call_span.0, call_span.1
                )
            })
    }
}

// ── Generic function signatures ───────────────────────────────────────────────

/// The signature of a generic function, with types expressed using `TypeParam`.
///
/// Distinct from `FunctionSig` (which only holds non-generic functions) so the
/// two tables stay cleanly separated and the call-site dispatch is unambiguous.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericFnSig {
    /// Ordered list of type-parameter names (e.g. `["T"]` for `identity<T>`).
    pub type_params: Vec<String>,
    /// Constraint map: `type_param_name → [contract_names]`.
    ///
    /// `sort<T follows Comparable>` → `[("T", ["Comparable"])]`.
    pub constraints: Vec<(String, Vec<String>)>,
    /// Parameter list: `(name, type)` — types may contain `TypeParam`.
    pub params: Vec<(String, Type)>,
    /// Ownership modifiers, parallel to `params`.
    pub param_ownerships: Vec<Option<OwnershipModifier>>,
    /// Return type — may contain `TypeParam`.
    pub ret: Type,
    pub decl_span: SourceSpan,
}

/// All generic function signatures collected from a module.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenericFnTable {
    pub fns: HashMap<String, GenericFnSig>,
}

impl GenericFnTable {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn all_names(&self) -> Vec<&str> {
        self.fns.keys().map(String::as_str).collect()
    }
}

// ── Generic shape definitions ─────────────────────────────────────────────────

/// A generic shape: `shape Pair<A, B> { first: A, second: B }`.
///
/// Field types may be `TypeParam` — resolved per-instantiation via `field_type`.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericShapeDef {
    pub name: String,
    /// Ordered type-parameter names: `["A", "B"]` for `Pair<A, B>`.
    pub type_params: Vec<String>,
    /// All fields — types may be `TypeParam` referencing entries in `type_params`.
    pub fields: Vec<FieldDef>,
    pub follows: Vec<String>,
    pub defined_at: SourceSpan,
}

impl GenericShapeDef {
    /// Build a substitution from the ordered concrete type args.
    ///
    /// For `Pair<int, string>` with `type_params = ["A", "B"]`:
    /// returns `{"A": Int, "B": String}`.
    pub fn make_substitution(&self, type_args: &[Type]) -> Substitution {
        self.type_params
            .iter()
            .zip(type_args.iter())
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect()
    }

    /// Look up a field and apply the substitution, returning the concrete field type.
    pub fn field_type(&self, field_name: &str, subst: &Substitution) -> Option<Type> {
        self.fields
            .iter()
            .find(|f| f.name == field_name)
            .map(|f| apply_substitution(&f.ty, subst))
    }

    /// All field names — for "did you mean" suggestions.
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }
}

/// All generic shape definitions from a module.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenericShapeTable {
    pub shapes: HashMap<String, GenericShapeDef>,
}

impl GenericShapeTable {
    pub fn get(&self, name: &str) -> Option<&GenericShapeDef> {
        self.shapes.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.shapes.contains_key(name)
    }
}
