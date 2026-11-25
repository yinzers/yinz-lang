// Generics typeck (M5 P3a, M9): which generic programs the checker accepts or rejects, what it
// records in the monomorphization table that codegen instantiates from, and how type
// substitution treats a union. Three harnesses, so three tests: one table of accept/reject cases
// (clean, or exactly N diagnostics), one table of table-entry cases (the exact set of
// instantiations with their concrete parameter and return types), and the substitution function.

use std::collections::HashMap;
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::generics::apply_substitution;
use ynz_typeck::{check_query, CheckOutput, Type};

fn check(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, source_file)).clone()
}

fn assert_clean(source: &str) {
    let output = check(source);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "Expected no diagnostics, got: {:#?}",
        output.diagnostics
    );
}

fn assert_diagnostic_count(source: &str, expected: usize) {
    let output = check(source);
    assert_eq!(
        output.diagnostics.len(),
        expected,
        "Expected {expected} diagnostic(s), got: {:#?}",
        output.diagnostics
    );
}

/// One expected monomorphization entry: function, type arguments, concrete parameter and return
/// types.
type ExpectedEntry = (&'static str, Vec<Type>, Vec<Type>, Type);

fn mono(
    function: &'static str,
    type_arguments: Vec<Type>,
    parameter_types: Vec<Type>,
    return_type: Type,
) -> ExpectedEntry {
    (function, type_arguments, parameter_types, return_type)
}

/// The program must be clean and the table must hold exactly `expected`, in any order.
fn assert_monomorphization_entries(source: &str, mut expected: Vec<ExpectedEntry>) {
    let output = check(source);
    assert_eq!(
        output.diagnostics.len(),
        0,
        "Expected no diagnostics, got: {:#?}",
        output.diagnostics
    );
    let mut found: Vec<String> = output
        .mono_table
        .entries
        .iter()
        .map(|(key, signature)| {
            format!(
                "{:?}",
                (
                    key.fn_name.as_str(),
                    &key.type_args,
                    &signature.param_types,
                    &signature.ret_type
                )
            )
        })
        .collect();
    found.sort();
    expected.sort_by_key(|entry| format!("{entry:?}"));
    let mut wanted: Vec<String> = expected.iter().map(|entry| format!("{entry:?}")).collect();
    wanted.sort();
    assert_eq!(found, wanted, "monomorphization table differs");
}

macro_rules! generic_cases {
    ($($case:ident: $source:expr => $expected:tt;)+) => {
        $(
            // bug: typeck accepts a generic call with the wrong number of arguments, a type argument that does not follow its constraint, a const given to a generic call, an unknown field on a generic shape or an unknown/ill-formed type annotation (no diagnostic), or refuses a valid generic declaration or call | tier: High
            #[test]
            fn $case() {
                generic_case!($source, $expected);
            }
        )+
    };
}

macro_rules! generic_case {
    ($source:expr, clean) => {
        assert_clean($source)
    };
    ($source:expr, $diagnostics:literal) => {
        assert_diagnostic_count($source, $diagnostics)
    };
}

generic_cases! {
    a_function_with_one_type_parameter_is_clean:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing { }
"#
        => clean;
    a_function_with_two_type_parameters_is_clean:
        r#"
function pair<A, B>(give first: A, give second: B) -> A { return first }
function entrypoint() -> Nothing { }
"#
        => clean;
    a_generic_call_with_too_many_arguments_is_one_diagnostic:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing { let n = identity(5, 6) }
"#
        => 1;
    a_generic_call_with_no_argument_to_infer_from_is_one_diagnostic:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing { let n = identity() }
"#
        => 1;
    a_type_argument_that_follows_the_constraint_is_clean:
        r#"
shape Comparable {
  compare(share self, share other: Self) -> Int
}
shape Player follows Comparable {
  name: String
  score: Int
}
function compare(share self: Player, share other: Player) -> Int {
  return self.score
}
function sort<T follows Comparable>(share item: T) -> T { return item }
function entrypoint() -> Nothing {
  const p: Player = { name: `Alice`, score: 100 }
  let q = sort(p)
}
"#
        => clean;
    a_type_argument_that_does_not_follow_the_constraint_is_one_diagnostic:
        r#"
shape Comparable {
  compare(share self, share other: Self) -> Int
}
shape Player {
  name: String
}
function sort<T follows Comparable>(share item: T) -> T { return item }
function entrypoint() -> Nothing {
  const p: Player = { name: `Alice` }
  let q = sort(p)
}
"#
        => 1;
    a_generic_shape_declaration_is_clean:
        r#"
shape Pair<A, B> { first: A  second: B }
function entrypoint() -> Nothing { }
"#
        => clean;
    an_unknown_field_on_a_generic_shape_is_one_diagnostic:
        r#"
shape Pair<A, B> { first: A  second: B }
function entrypoint() -> Nothing {
  let p: Pair<Int, String> = { first: 5, second: `hi` }
  let x = p.third
}
"#
        => 1;
    giving_a_const_to_a_generic_call_is_one_diagnostic:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing {
  const x = 5
  let y = identity(x)
}
"#
        => 1;
    an_unknown_generic_name_in_a_let_annotation_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
  let slot: Nope<Int> = 4096
}
"#
        => 1;
    type_arguments_on_a_plain_shape_in_a_parameter_are_one_diagnostic:
        r#"
shape Big {
  label: String
}

function show(share value: Big<Int>) -> Nothing { }

function entrypoint() -> Nothing { }
"#
        => 1;
    a_map_missing_its_value_type_in_a_let_annotation_is_one_diagnostic:
        r#"
function entrypoint() -> Nothing {
  let scores: Map<Int> = {}
}
"#
        => 1;
}

macro_rules! monomorphization_cases {
    ($($case:ident: $source:expr => $expected:expr;)+) => {
        $(
            // bug: typeck records a wrong, missing, duplicated or non-concrete instantiation, so codegen emits a missing or wrong-typed monomorphic function (ABI mismatch) | tier: High
            #[test]
            fn $case() {
                assert_monomorphization_entries($source, $expected);
            }
        )+
    };
}

monomorphization_cases! {
    an_inferred_int_argument_records_identity_over_int:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing { let n = identity(5) }
"#
        => vec![mono("identity", vec![Type::Int], vec![Type::Int], Type::Int)];
    two_inferred_type_parameters_record_both_type_arguments:
        r#"
function pair<A, B>(give first: A, give second: B) -> A { return first }
function entrypoint() -> Nothing { let p = pair(5, `hello`) }
"#
        => vec![mono("pair", vec![Type::Int, Type::String], vec![Type::Int, Type::String], Type::Int)];
    two_different_instantiations_are_two_entries:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing {
  let n = identity(5)
  let s = identity(`hello`)
}
"#
        => vec![
            mono("identity", vec![Type::Int], vec![Type::Int], Type::Int),
            mono("identity", vec![Type::String], vec![Type::String], Type::String),
        ];
    the_same_instantiation_twice_is_one_entry:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing {
  let a = identity(5)
  let b = identity(10)
}
"#
        => vec![mono("identity", vec![Type::Int], vec![Type::Int], Type::Int)];
    an_explicit_type_argument_records_the_same_entry:
        r#"
function identity<T>(give value: T) -> T { return value }
function entrypoint() -> Nothing { let n = identity<Int>(5) }
"#
        => vec![mono("identity", vec![Type::Int], vec![Type::Int], Type::Int)];
    a_non_generic_call_records_no_entry:
        r#"
function add(a: Int, b: Int) -> Int { return a }
function entrypoint() -> Nothing { let x = add(1, 2) }
"#
        => vec![];
}

// bug: substituting a type parameter into a union that collapses to one variant gives a union instead of that variant, so a correct generic call is refused naming the same type on both sides | tier: High
#[test]
fn substituting_a_union_that_collapses_to_one_variant_gives_that_variant() {
    let tiny = Type::Shape {
        name: "Tiny".into(),
    };
    let tiny_or_type_parameter = Type::Union {
        variants: vec![Type::TypeParam { name: "T".into() }, tiny.clone()],
    };
    let substitution = HashMap::from([("T".to_string(), tiny.clone())]);
    assert_eq!(
        apply_substitution(&tiny_or_type_parameter, &substitution),
        tiny
    );
}
