use std::collections::HashMap;

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
    match ty {
        Type::TypeParam { name } => subst.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Generic { name, args } => Type::Generic {
            name: name.clone(),
            args: args.iter().map(|a| apply_substitution(a, subst)).collect(),
        },
        other => other.clone(),
    }
}

// ── MonomorphizationTable ─────────────────────────────────────────────────────

/// Key into the MonomorphizationTable: the function name + the concrete type args.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MonomorphizationTable {
    pub entries: HashMap<MonoKey, MonoSignature>,
}

impl MonomorphizationTable {
    pub fn record(&mut self, fn_name: String, type_args: Vec<Type>, sig: MonoSignature) {
        self.entries.insert(MonoKey { fn_name, type_args }, sig);
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
