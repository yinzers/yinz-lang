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

/// The type a binding annotated with a union holds when the value is a `fixed` list: the union's
/// `fixed<T>` variant takes the VALUE's length. A list's length is a compile-time constant of its
/// value, never of an annotation that does not write one (`fixed<string> | nothing`), and the
/// release walks exactly that many cells, so a binding typed with `Unknown` could not be released
/// (`OmitReason::UnsizedFixed`, a leak). A later store of a list of another length is then refused
/// by `can_assign`, as for any sized `fixed` slot. Only a variant with the value's item type and no
/// length changes; any other type is returned as it is. O(variants).
pub fn union_binding_type_with_value_length(annotated: &Type, value: &Type) -> Type {
    let (
        Type::Union { variants },
        Type::BuiltinFixed {
            elem: value_elem,
            size,
        },
    ) = (annotated, value)
    else {
        return annotated.clone();
    };
    let sized = variants
        .iter()
        .map(|variant| match variant {
            Type::BuiltinFixed {
                elem,
                size: FixedLength::Unknown,
            } if elem == value_elem => Type::BuiltinFixed {
                elem: elem.clone(),
                size: size.clone(),
            },
            other => other.clone(),
        })
        .collect();
    Type::Union { variants: sized }
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

    fn calls_keyed_by_position(written: Vec<FixedCall>) -> FixedCalls {
        written
            .into_iter()
            .enumerate()
            .map(|(position, call)| ((position * 10, position * 10 + 5), call))
            .collect()
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

    /// One named test per case so the runner names the failing case.
    macro_rules! close_instances_case {
        ($test_name:ident, calls: $calls:expr, parameters: $parameters:expr, versions: $versions:expr) => {
            // bug: close_instances makes a wrong set of per-length versions, so a call reads the wrong list length or a dead function is compiled | tier: High
            #[test]
            fn $test_name() {
                let calls = calls_keyed_by_position($calls);
                let versions = close_instances(&calls, &parameters($parameters));
                let expected: BTreeSet<InstanceKey> = $versions.into_iter().collect();
                assert_eq!(versions, expected);
            }
        };
    }

    // Case bug: a function called with lists of two lengths got one version (the first), so the
    // second call read the wrong length.
    close_instances_case!(
        one_version_per_distinct_length_a_caller_passes,
        calls: vec![
            call("entrypoint", "total", vec![sized(2)]),
            call("entrypoint", "total", vec![sized(3)]),
            call("entrypoint", "total", vec![sized(3)]),
        ],
        parameters: &[("total", &["xs"])],
        versions: [version("total", &[Some(2)]), version("total", &[Some(3)])]
    );

    // Case bug: a call inside a function compiled per length must ask for the callee's version for
    // EACH length of its caller, and the placeholder must read the caller's parameter, not the
    // callee's (`twice(xs)` calls `pick(xs, xs)`).
    close_instances_case!(
        a_call_inside_a_specialised_function_follows_the_callers_length,
        calls: vec![
            call("entrypoint", "twice", vec![sized(3)]),
            call("entrypoint", "twice", vec![sized(4)]),
            call("twice", "pick", vec![of("xs"), of("xs")]),
        ],
        parameters: &[("twice", &["xs"]), ("pick", &["xs", "ys"])],
        versions: [
            version("twice", &[Some(3)]),
            version("twice", &[Some(4)]),
            version("pick", &[Some(3), Some(3)]),
            version("pick", &[Some(4), Some(4)]),
        ]
    );

    // Case bug: a chain of three specialised functions stopped one link early (the closure ran
    // one expansion round, which the two-link case above does not catch).
    close_instances_case!(
        versions_reach_the_end_of_a_chain,
        calls: vec![
            call("entrypoint", "a", vec![sized(7)]),
            call("a", "b", vec![of("xs")]),
            call("b", "c", vec![of("ys")]),
        ],
        parameters: &[("a", &["xs"]), ("b", &["ys"]), ("c", &["zs"])],
        versions: [
            version("a", &[Some(7)]),
            version("b", &[Some(7)]),
            version("c", &[Some(7)]),
        ]
    );

    // Case bug: recursion between specialised functions looped forever.
    close_instances_case!(
        recursion_adds_each_version_once,
        calls: vec![
            call("entrypoint", "ping", vec![sized(2)]),
            call("ping", "pong", vec![of("xs")]),
            call("pong", "ping", vec![of("ys")]),
        ],
        parameters: &[("ping", &["xs"]), ("pong", &["ys"])],
        versions: [version("ping", &[Some(2)]), version("pong", &[Some(2)])]
    );

    // Case bug: a specialised function nothing calls got a version (dead code with a made-up length).
    close_instances_case!(
        a_function_nothing_calls_has_no_version,
        calls: vec![],
        parameters: &[("total", &["xs"])],
        versions: Vec::<InstanceKey>::new()
    );

    // Case bug: a length the caller cannot say became a made-up number instead of unknown.
    close_instances_case!(
        a_length_nobody_knows_stays_unknown,
        calls: vec![call("entrypoint", "total", vec![FixedLength::Unknown])],
        parameters: &[("total", &["xs"])],
        versions: [version("total", &[None])]
    );

    // Case bug: a call inside a per-length function that only a dead caller reaches still
    // produced the callee's version.
    close_instances_case!(
        a_call_in_an_unreached_specialised_function_asks_for_nothing,
        calls: vec![call("dead", "pick", vec![of("xs")])],
        parameters: &[("dead", &["xs"]), ("pick", &["ys"])],
        versions: Vec::<InstanceKey>::new()
    );

    // bug: a callee's returned list loses its length because the placeholder is never replaced by the caller's length | tier: High
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

    /// One named test per case so the runner names the failing case. `$arguments` builds the
    /// written `fixed<…>` argument list from `length` (a `Length` argument) and `name` (`N`).
    macro_rules! fixed_arguments_case {
        ($test_name:ident, $arguments:expr, |$read:ident| $assertion:expr) => {
            // bug: a written fixed<…> argument list is read wrongly (length, element or surplus), so a bad length is accepted or a good one refused | tier: High
            #[test]
            fn $test_name() {
                let span = SourceSpan::new("t.ynz", 0, 1);
                let length = |value: u64| AstType::Length {
                    value,
                    span: span.clone(),
                };
                let name = AstType::Named("N".to_string(), span.clone());
                let arguments: Vec<AstType> = $arguments(&length, name);
                let $read = read_fixed_arguments(&arguments);
                $assertion
            }
        };
    }

    // Case bug: `fixed` with no arguments reports a length or element it never had.
    fixed_arguments_case!(
        no_arguments_has_no_length_and_no_element,
        |_length: &dyn Fn(u64) -> AstType, _name| Vec::new(),
        |read| {
            assert_eq!(read.length, WrittenLength::Absent);
            assert!(read.element.is_none());
        }
    );
    // Case bug: `fixed<int, 4>` loses its length.
    fixed_arguments_case!(
        element_and_length_read_as_a_number,
        |length: &dyn Fn(u64) -> AstType, _name| vec![AstType::Int, length(4)],
        |read| assert_eq!(read.length, WrittenLength::Number(4))
    );
    // Case bug: a zero length is treated as absent or refused here (the empty boundary).
    fixed_arguments_case!(
        zero_length_is_a_number,
        |length: &dyn Fn(u64) -> AstType, _name| vec![AstType::Int, length(0)],
        |read| assert_eq!(read.length, WrittenLength::Number(0))
    );
    // Case bug: a length one past the largest LLVM array length is accepted (off-by-one, max).
    fixed_arguments_case!(
        length_past_the_maximum_is_too_large,
        |length: &dyn Fn(u64) -> AstType, _name| vec![AstType::Int, length(MAX_FIXED_LENGTH + 1)],
        |read| assert!(matches!(read.length, WrittenLength::TooLarge { .. }))
    );
    // Case bug: the largest length is refused (off-by-one, max).
    fixed_arguments_case!(
        length_at_the_maximum_is_a_number,
        |length: &dyn Fn(u64) -> AstType, _name| vec![AstType::Int, length(MAX_FIXED_LENGTH)],
        |read| assert_eq!(
            read.length,
            WrittenLength::Number(MAX_FIXED_LENGTH as usize)
        )
    );
    // Case bug: a named second argument (`fixed<int, N>`) is read as a length.
    fixed_arguments_case!(
        named_length_is_not_a_number,
        |_length: &dyn Fn(u64) -> AstType, name| vec![AstType::Int, name],
        |read| assert!(matches!(read.length, WrittenLength::NotANumber(_)))
    );
    // Case bug: arguments past the length (`fixed<int, 2, 3>`) are silently dropped.
    fixed_arguments_case!(
        arguments_past_the_length_are_surplus,
        |length: &dyn Fn(u64) -> AstType, _name| vec![AstType::Int, length(2), length(3)],
        |read| assert_eq!(read.surplus.len(), 1)
    );
    // Case bug: `fixed<4>` reads the number as the element type.
    fixed_arguments_case!(
        a_lone_number_is_not_an_element,
        |length: &dyn Fn(u64) -> AstType, _name| vec![length(4)],
        |read| assert!(read.element.is_none())
    );

    // bug: a sized `fixed<int, 4>` or non-fixed parameter is marked as a per-call length parameter, so a call with another length is accepted or a needless version is compiled | tier: High
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
