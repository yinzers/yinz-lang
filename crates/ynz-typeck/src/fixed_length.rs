//! `fixed<T, N>` in function signatures (v0.3-M9 C1h, card #433).
//!
//! Chief's decision: `fixed<T, N>` costs what Rust's `[T; N]` costs. The length is a compile-time
//! constant, never a runtime field and never a pointer-and-length pair. So:
//!
//! - a returned `fixed` comes back by value (its cells), and its length is part of its type;
//! - a parameter written `fixed<T>` (no length) is compiled once per length its callers pass;
//! - the ONE place that works out which lengths are needed is [`close_instances`]. Codegen reads
//!   its answer ([`FixedInstances`]); nothing else derives a length on its own
//!   (`.claude/rules/authoritative-derivation.md`).
//!
//! This module is also the ONE reader of a written `fixed<…>` argument list
//! ([`read_fixed_arguments`]); the three type resolvers (`shapes.rs`, `signatures.rs`,
//! `check.rs`) call it instead of each reading the arguments their own way.

use std::collections::{BTreeMap, BTreeSet};

use ynz_ast::nodes::Type as AstType;
use ynz_diagnostics::SourceSpan;

use crate::types::{FixedLength, Type};

// ── Reading a written `fixed<…>` ─────────────────────────────────────────────────────────────

/// The largest length `fixed<T, N>` accepts: codegen lays a list out as an LLVM array whose
/// length is a `u32`.
pub const MAX_FIXED_LENGTH: u64 = u32::MAX as u64;

/// What the type arguments of a written `fixed<…>` say.
#[derive(Debug)]
pub struct FixedArguments<'a> {
    /// The element type argument, when one is written.
    pub element: Option<&'a AstType>,
    /// The length argument.
    pub length: WrittenLength<'a>,
    /// Arguments past the element and the length (`fixed<int, 4, 5>`): not allowed.
    pub surplus: Vec<&'a AstType>,
}

/// The length of a written `fixed<…>`.
#[derive(Debug, PartialEq)]
pub enum WrittenLength<'a> {
    /// `fixed<T>`.
    Absent,
    /// `fixed<T, 4>`: a whole number, usable as a length.
    Number(usize),
    /// `fixed<T, 99999999999>`: a whole number too large for a length.
    TooLarge { value: u64, span: &'a SourceSpan },
    /// `fixed<T, N>`, `fixed<T, string>`: a second argument that is not a whole number.
    NotANumber(&'a AstType),
}

/// Read the argument list of a `fixed<…>`. O(arguments) time and space.
pub fn read_fixed_arguments(args: &[AstType]) -> FixedArguments<'_> {
    let mut written = args.iter();
    // `fixed<4>` has no element type: the number is reported through `element: None` by the
    // caller's missing-argument path, not read as an element.
    let element = match written.next() {
        Some(AstType::Length { .. }) | None => None,
        Some(element) => Some(element),
    };
    let length = match written.next() {
        None => WrittenLength::Absent,
        Some(AstType::Length { value, .. }) if *value <= MAX_FIXED_LENGTH => {
            WrittenLength::Number(*value as usize)
        }
        Some(AstType::Length { value, span }) => WrittenLength::TooLarge {
            value: *value,
            span,
        },
        Some(other) => WrittenLength::NotANumber(other),
    };
    FixedArguments {
        element,
        length,
        surplus: written.collect(),
    }
}

/// The type a written `fixed<…>` stands for, given its resolved element type. A length that is
/// not usable (`TooLarge`, `NotANumber`) leaves the length unknown: the checker has already
/// reported it, and no later pass runs on a program with an error.
pub fn fixed_type(element: Type, length: &WrittenLength<'_>) -> Type {
    let size = match length {
        WrittenLength::Number(count) => FixedLength::Known(*count),
        WrittenLength::Absent | WrittenLength::TooLarge { .. } | WrittenLength::NotANumber(_) => {
            FixedLength::Unknown
        }
    };
    Type::BuiltinFixed {
        elem: Box::new(element),
        size,
    }
}

// ── Length parameters of a function ──────────────────────────────────────────────────────────

/// Mark every unsized `fixed<T>` parameter of a function signature as `OfParam`: the function is
/// compiled once per length its callers pass. Only a parameter whose whole type is the `fixed`
/// is marked (`fixed<T, N>` is only allowed there). O(parameters) time.
pub fn mark_length_parameters(params: &mut [(String, Type)]) {
    for (name, ty) in params.iter_mut() {
        if let Type::BuiltinFixed { size, .. } = ty {
            if *size == FixedLength::Unknown {
                *size = FixedLength::OfParam {
                    parameter: name.clone(),
                };
            }
        }
    }
}

/// The parameters of a signature whose length each caller supplies, in declaration order.
/// O(parameters) time.
pub fn length_parameters(params: &[(String, Type)]) -> Vec<String> {
    params
        .iter()
        .filter_map(|(name, ty)| match ty {
            Type::BuiltinFixed {
                size: FixedLength::OfParam { parameter },
                ..
            } if parameter == name => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// Replace the length placeholders of a callee's return type with the lengths its caller
/// passed: `function same(give xs: fixed<int>) -> fixed<int> { return xs }` called with a list
/// of 3 returns a list of 3. `bindings[i]` is the length for `length_parameters[i]`. Only a
/// return type that IS a `fixed` can hold a placeholder. O(length parameters) time.
pub fn apply_length_bindings(
    ty: &Type,
    length_parameters: &[String],
    bindings: &[FixedLength],
) -> Type {
    match ty {
        Type::BuiltinFixed {
            elem,
            size: FixedLength::OfParam { parameter },
        } => {
            let size = length_parameters
                .iter()
                .position(|name| name == parameter)
                .and_then(|index| bindings.get(index))
                .cloned()
                .unwrap_or(FixedLength::Unknown);
            Type::BuiltinFixed {
                elem: elem.clone(),
                size,
            }
        }
        other => other.clone(),
    }
}

// ── The specialised versions a program needs ─────────────────────────────────────────────────

/// A call to a function that has length parameters: which lengths the caller passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedCall {
    /// The function the call is written in (`""` outside any function).
    pub caller: String,
    pub callee: String,
    /// One length per length parameter of `callee`, in declaration order. A length is known, the
    /// caller's own length parameter (a call inside a function that is itself compiled per
    /// length), or unknown.
    pub lengths: Vec<FixedLength>,
}

/// Every such call of a module, keyed by the call's `(start, end)` byte span.
pub type FixedCalls = BTreeMap<(usize, usize), FixedCall>;

/// One specialised version of a function: its length parameters' lengths in declaration order
/// (`None` where the caller cannot say: the version then behaves as an unsized `fixed` always
/// has, a list of unknown length).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceKey {
    pub function: String,
    pub lengths: Vec<Option<usize>>,
}

/// What the type checker decided about `fixed` lengths, for codegen to read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FixedInstances {
    /// Each call to a function with length parameters, by the call's span.
    pub calls: FixedCalls,
    /// The length parameters of every function that has any, in declaration order.
    pub length_parameters: BTreeMap<String, Vec<String>>,
    /// Every specialised version the program needs, in a stable order (the order reaches the
    /// emitted code).
    pub instances: BTreeSet<InstanceKey>,
}

impl FixedInstances {
    /// The version the call at `call_span` runs, when it is inside the version `caller_lengths`
    /// of its function. `None` when typeck recorded no call there. O(lengths) time.
    pub fn instance_of_call(
        &self,
        call_span: (usize, usize),
        caller_lengths: &[Option<usize>],
    ) -> Option<InstanceKey> {
        let call = self.calls.get(&call_span)?;
        let caller_parameters = self.length_parameters.get(&call.caller);
        Some(InstanceKey {
            function: call.callee.clone(),
            lengths: resolve_lengths(&call.lengths, caller_parameters, caller_lengths),
        })
    }

    /// The length parameters of `function`, empty when it has none. O(log functions) time.
    pub fn parameters_of(&self, function: &str) -> &[String] {
        self.length_parameters
            .get(function)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// The lengths of one call as seen from inside the version `caller_lengths` of the function
/// whose length parameters are `caller_parameters`. THE one place a placeholder becomes a number.
/// O(lengths · caller parameters) time.
pub fn resolve_lengths(
    lengths: &[FixedLength],
    caller_parameters: Option<&Vec<String>>,
    caller_lengths: &[Option<usize>],
) -> Vec<Option<usize>> {
    lengths
        .iter()
        .map(|length| match length {
            FixedLength::Known(count) => Some(*count),
            FixedLength::Unknown => None,
            FixedLength::OfParam { parameter } => caller_parameters
                .and_then(|parameters| parameters.iter().position(|name| name == parameter))
                .and_then(|index| caller_lengths.get(index).copied().flatten()),
        })
        .collect()
}

/// The LLVM symbol of a specialised version: `name<3>`, `name<3,4>`, `name<?>` (unknown
/// length). `<`, `>`, `,` and `?` never occur in an identifier, so it never equals a user
/// function's name. O(lengths) time.
pub fn instance_symbol(function: &str, lengths: &[Option<usize>]) -> String {
    let written: Vec<String> = lengths
        .iter()
        .map(|length| match length {
            Some(count) => count.to_string(),
            None => "?".to_string(),
        })
        .collect();
    format!("{function}<{}>", written.join(","))
}

/// Which specialised versions the program needs: every call from a function that is not itself
/// compiled per length asks for one version, and every version of a function that is asks for
/// the versions its own calls need. A function with length parameters that nothing calls has no
/// versions.
///
/// O(calls + versions · calls per function) time, O(versions) space. Terminates because each
/// version is added once and the lengths only come from numbers written in the program.
pub fn close_instances(
    calls: &FixedCalls,
    length_parameters: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<InstanceKey> {
    let mut calls_by_caller: BTreeMap<&str, Vec<&FixedCall>> = BTreeMap::new();
    for call in calls.values() {
        calls_by_caller.entry(&call.caller).or_default().push(call);
    }
    let mut instances: BTreeSet<InstanceKey> = BTreeSet::new();
    let mut pending: Vec<InstanceKey> = Vec::new();
    let mut add = |key: InstanceKey, pending: &mut Vec<InstanceKey>| {
        if instances.insert(key.clone()) {
            pending.push(key);
        }
    };
    // Roots: calls written in a function that has no length parameters run in its one version.
    for call in calls.values() {
        if length_parameters.contains_key(&call.caller) {
            continue;
        }
        let key = InstanceKey {
            function: call.callee.clone(),
            lengths: resolve_lengths(&call.lengths, None, &[]),
        };
        add(key, &mut pending);
    }
    while let Some(version) = pending.pop() {
        let Some(version_calls) = calls_by_caller.get(version.function.as_str()) else {
            continue;
        };
        let parameters = length_parameters.get(&version.function);
        for call in version_calls {
            let key = InstanceKey {
                function: call.callee.clone(),
                lengths: resolve_lengths(&call.lengths, parameters, &version.lengths),
            };
            add(key, &mut pending);
        }
    }
    instances
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(count: usize) -> FixedLength {
        FixedLength::Known(count)
    }

    fn of(parameter: &str) -> FixedLength {
        FixedLength::OfParam {
            parameter: parameter.to_string(),
        }
    }

    fn call(caller: &str, callee: &str, lengths: Vec<FixedLength>) -> FixedCall {
        FixedCall {
            caller: caller.to_string(),
            callee: callee.to_string(),
            lengths,
        }
    }

    fn parameters(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        entries
            .iter()
            .map(|(function, names)| {
                (
                    function.to_string(),
                    names.iter().map(|name| name.to_string()).collect(),
                )
            })
            .collect()
    }

    fn version(function: &str, lengths: &[Option<usize>]) -> InstanceKey {
        InstanceKey {
            function: function.to_string(),
            lengths: lengths.to_vec(),
        }
    }

    // Bug: a function called with lists of two lengths got one version (the first), so the second
    // call read the wrong length (High: data integrity of a value across a call).
    #[test]
    fn one_version_per_distinct_length_a_caller_passes() {
        let mut calls = FixedCalls::new();
        calls.insert((0, 5), call("entrypoint", "total", vec![sized(2)]));
        calls.insert((10, 15), call("entrypoint", "total", vec![sized(3)]));
        calls.insert((20, 25), call("entrypoint", "total", vec![sized(3)]));
        let versions = close_instances(&calls, &parameters(&[("total", &["xs"])]));
        let expected: BTreeSet<InstanceKey> =
            [version("total", &[Some(2)]), version("total", &[Some(3)])]
                .into_iter()
                .collect();
        assert_eq!(versions, expected);
    }

    // Bug: a call inside a function compiled per length needs the callee's version for EACH
    // length of its caller, and the placeholder must read the caller's parameter, not the callee's
    // (`twice(xs)` calls `pick(xs, xs)`).
    #[test]
    fn a_call_inside_a_specialised_function_follows_the_callers_length() {
        let mut calls = FixedCalls::new();
        calls.insert((0, 5), call("entrypoint", "twice", vec![sized(3)]));
        calls.insert((10, 15), call("entrypoint", "twice", vec![sized(4)]));
        calls.insert((20, 25), call("twice", "pick", vec![of("xs"), of("xs")]));
        let versions = close_instances(
            &calls,
            &parameters(&[("twice", &["xs"]), ("pick", &["xs", "ys"])]),
        );
        let expected: BTreeSet<InstanceKey> = [
            version("twice", &[Some(3)]),
            version("twice", &[Some(4)]),
            version("pick", &[Some(3), Some(3)]),
            version("pick", &[Some(4), Some(4)]),
        ]
        .into_iter()
        .collect();
        assert_eq!(versions, expected);
    }

    // Bug: a chain of three specialised functions stopped one link early.
    #[test]
    fn versions_reach_the_end_of_a_chain() {
        let mut calls = FixedCalls::new();
        calls.insert((0, 1), call("entrypoint", "a", vec![sized(7)]));
        calls.insert((2, 3), call("a", "b", vec![of("xs")]));
        calls.insert((4, 5), call("b", "c", vec![of("ys")]));
        let versions = close_instances(
            &calls,
            &parameters(&[("a", &["xs"]), ("b", &["ys"]), ("c", &["zs"])]),
        );
        assert!(versions.contains(&version("c", &[Some(7)])));
        assert_eq!(versions.len(), 3);
    }

    // Bug: recursion between specialised functions looped forever.
    #[test]
    fn recursion_adds_each_version_once() {
        let mut calls = FixedCalls::new();
        calls.insert((0, 1), call("entrypoint", "ping", vec![sized(2)]));
        calls.insert((2, 3), call("ping", "pong", vec![of("xs")]));
        calls.insert((4, 5), call("pong", "ping", vec![of("ys")]));
        let versions =
            close_instances(&calls, &parameters(&[("ping", &["xs"]), ("pong", &["ys"])]));
        assert_eq!(versions.len(), 2);
    }

    // Bug: a specialised function nothing calls got a version (dead code with a made-up length).
    #[test]
    fn a_function_nothing_calls_has_no_version() {
        let calls = FixedCalls::new();
        let versions = close_instances(&calls, &parameters(&[("total", &["xs"])]));
        assert!(versions.is_empty());
    }

    // Bug: a length the caller cannot say became a made-up number instead of unknown.
    #[test]
    fn a_length_nobody_knows_stays_unknown() {
        let mut calls = FixedCalls::new();
        calls.insert(
            (0, 1),
            call("entrypoint", "total", vec![FixedLength::Unknown]),
        );
        let versions = close_instances(&calls, &parameters(&[("total", &["xs"])]));
        assert!(versions.contains(&version("total", &[None])));
    }

    // Bug: a call inside a function that is compiled per length, where nothing ever asks for a
    // version of that function (a function that only a dead caller reaches), still produced the
    // callee's version.
    #[test]
    fn a_call_in_an_unreached_specialised_function_asks_for_nothing() {
        let mut calls = FixedCalls::new();
        calls.insert((0, 1), call("dead", "pick", vec![of("xs")]));
        let versions =
            close_instances(&calls, &parameters(&[("dead", &["xs"]), ("pick", &["ys"])]));
        assert!(versions.is_empty());
    }

    #[test]
    fn symbols_do_not_collide_with_identifiers() {
        assert_eq!(instance_symbol("total", &[Some(3)]), "total<3>");
        assert_eq!(instance_symbol("pick", &[Some(3), None]), "pick<3,?>");
    }

    // Bug: a returned list lost its length because the callee's placeholder was never replaced.
    #[test]
    fn a_return_type_takes_the_length_the_caller_passed() {
        let returned = Type::BuiltinFixed {
            elem: Box::new(Type::Int),
            size: of("xs"),
        };
        let applied = apply_length_bindings(&returned, &["xs".to_string()], &[sized(3)]);
        assert_eq!(
            applied,
            Type::BuiltinFixed {
                elem: Box::new(Type::Int),
                size: sized(3)
            }
        );
    }

    // Reading `fixed<…>` (High: parser of untrusted input): every shape of argument list.
    #[test]
    fn reads_every_shape_of_argument_list() {
        let span = SourceSpan::new("t.ynz", 0, 1);
        let length = |value: u64| AstType::Length {
            value,
            span: span.clone(),
        };
        let name = AstType::Named("N".to_string(), span.clone());
        assert_eq!(read_fixed_arguments(&[]).length, WrittenLength::Absent);
        assert!(read_fixed_arguments(&[]).element.is_none());
        let sized_list = [AstType::Int, length(4)];
        assert_eq!(
            read_fixed_arguments(&sized_list).length,
            WrittenLength::Number(4)
        );
        let zero = [AstType::Int, length(0)];
        assert_eq!(read_fixed_arguments(&zero).length, WrittenLength::Number(0));
        let too_large = [AstType::Int, length(MAX_FIXED_LENGTH + 1)];
        assert!(matches!(
            read_fixed_arguments(&too_large).length,
            WrittenLength::TooLarge { .. }
        ));
        let at_the_limit = [AstType::Int, length(MAX_FIXED_LENGTH)];
        assert_eq!(
            read_fixed_arguments(&at_the_limit).length,
            WrittenLength::Number(MAX_FIXED_LENGTH as usize)
        );
        let by_name = [AstType::Int, name];
        assert!(matches!(
            read_fixed_arguments(&by_name).length,
            WrittenLength::NotANumber(_)
        ));
        let surplus = [AstType::Int, length(2), length(3)];
        assert_eq!(read_fixed_arguments(&surplus).surplus.len(), 1);
        let only_a_number = [length(4)];
        assert!(read_fixed_arguments(&only_a_number).element.is_none());
    }

    #[test]
    fn marks_only_unsized_fixed_parameters() {
        let mut params = vec![
            (
                "xs".to_string(),
                Type::BuiltinFixed {
                    elem: Box::new(Type::Int),
                    size: FixedLength::Unknown,
                },
            ),
            (
                "ys".to_string(),
                Type::BuiltinFixed {
                    elem: Box::new(Type::Int),
                    size: sized(4),
                },
            ),
            ("n".to_string(), Type::Int),
        ];
        mark_length_parameters(&mut params);
        assert_eq!(length_parameters(&params), vec!["xs".to_string()]);
    }
}
