// Typeck accept/reject decisions over whole programs: types and operators, shapes, ownership and
// `give`, unions and options, inline shapes, channels and `background`, `wait` and suspension
// inference, kernel mode, errors-capable values. The programs are one table of NAMED cases,
// `check_cases!`: a case is clean (no error), rejected (at least one error) or exactly N errors,
// and wording of a diagnostic is not asserted here. Multi-file programs and kernel mode run
// through the same table (`run_project`, `run_kernel`, `run_kernel_project`); the re-check of a
// program after an edit is its own table, `recheck_cases!`.

use ynz_diagnostics::{Diagnostic, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_with_kernel_mode;
use ynz_typeck::{check_query, CheckOutput, Type};

const FILE: &str = "test.ynz";

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn run_kernel(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, FILE.to_string(), source.to_string());
    kernel_check(&db, source_file)
}

/// Drive typeck directly with kernel_mode=true, bypassing the salsa check_query (which always
/// runs with kernel mode off).
fn kernel_check(db: &CompilerDb, sf: SourceFile) -> CheckOutput {
    let parse = ynz_parser::parse_query(db, sf);
    let sig_output = ynz_typeck::queries::module_signatures_query(db, sf);

    let mut merged_sig_table = sig_output.sig_table.clone();
    for (name, sig) in &sig_output.imported_fns {
        merged_sig_table
            .fns
            .entry(name.clone())
            .or_insert_with(|| sig.clone());
    }

    let (typed, mono_table, check_diags) = check_with_kernel_mode(
        &parse.module,
        &merged_sig_table,
        &sig_output.shape_table,
        &sig_output.generic_fn_table,
        &sig_output.generic_shape_table,
        &ynz_typeck::intrinsics::PrimitiveIntrinsicTable::m6(),
        &sig_output.imported_options,
    );

    let mut all_diags = parse.diagnostics.clone();
    for d in sig_output.diagnostics.iter() {
        all_diags.push(d.clone());
    }
    for d in check_diags.into_iter() {
        all_diags.push(d);
    }

    CheckOutput {
        typed_module: typed,
        mono_table,
        diagnostics: all_diags,
        suspends_set: std::collections::HashSet::new(),
        does_real_work_set: std::collections::HashSet::new(),
        written_param_places: std::collections::HashMap::new(),
        effective_ownership: ynz_typeck::effective_ownership::EffectiveOwnershipReport::empty(),
    }
}

/// A project on disk holding `files` (with its `yinz.toml`, which import resolution needs) and a
/// database that knows them. Each project has its own temporary directory, so parallel tests never
/// share one, and the directory is removed even when the check panics.
struct Project {
    directory: tempfile::TempDir,
    db: CompilerDb,
    sources: Vec<(String, SourceFile)>,
}

impl Project {
    fn new(files: &[(&str, &str)]) -> Project {
        let directory = tempfile::tempdir().expect("create the project directory");
        std::fs::write(
            directory.path().join("yinz.toml"),
            "[project]\nname = \"test\"\n",
        )
        .expect("write yinz.toml");
        let mut db = CompilerDb::default();
        let mut sources = Vec::new();
        for (file_name, text) in files {
            let path = directory.path().join(file_name);
            std::fs::write(&path, text).expect("write a project file");
            let source_file = SourceFile::new(&db, path.display().to_string(), text.to_string());
            db.register_source(source_file);
            sources.push((file_name.to_string(), source_file));
        }
        Project {
            directory,
            db,
            sources,
        }
    }

    fn source(&self, file_name: &str) -> SourceFile {
        self.sources
            .iter()
            .find(|(name, _)| name == file_name)
            .unwrap_or_else(|| panic!("no project file `{file_name}`"))
            .1
    }

    fn check(&self, file_name: &str) -> CheckOutput {
        (*check_query(&self.db, self.source(file_name))).clone()
    }

    /// Replace the text of `file_name` on disk and in the database.
    fn edit(&mut self, file_name: &str, new_text: &str) {
        use salsa::Setter as _;
        std::fs::write(self.directory.path().join(file_name), new_text).expect("rewrite a file");
        let source_file = self.source(file_name);
        source_file.set_text(&mut self.db).to(new_text.to_string());
    }
}

/// Check `main_file` in a project holding `files`.
fn run_project(files: &[(&str, &str)], main_file: &str) -> CheckOutput {
    Project::new(files).check(main_file)
}

/// Check `main_file` in a project holding `files`, in kernel mode. The other files are checked
/// first so `main_file` sees which of their exports suspend.
fn run_kernel_project(files: &[(&str, &str)], main_file: &str) -> CheckOutput {
    let project = Project::new(files);
    for (file_name, _) in files {
        if *file_name != main_file {
            project.check(file_name);
        }
    }
    kernel_check(&project.db, project.source(main_file))
}

fn error_diagnostics(output: &CheckOutput) -> Vec<&Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .collect()
}

fn assert_no_errors(output: &CheckOutput) {
    let errors = error_diagnostics(output);
    assert!(
        errors.is_empty(),
        "Expected 0 errors, got {}: {:#?}",
        errors.len(),
        errors
    );
}

fn assert_has_errors(output: &CheckOutput) {
    assert!(
        !error_diagnostics(output).is_empty(),
        "Expected at least one error, got: {:#?}",
        output.diagnostics
    );
}

fn assert_error_count(output: &CheckOutput, expected_count: usize) {
    let errors = error_diagnostics(output);
    assert_eq!(
        errors.len(),
        expected_count,
        "Expected {expected_count} errors, got {}:\n{:#?}",
        errors.len(),
        errors
    );
}

/// The program is clean and typeck recorded `expected_type` for one of its expressions: type
/// inference ran and stored its results, it did not merely stay silent.
fn assert_infers(output: &CheckOutput, expected_type: Type) {
    assert_no_errors(output);
    assert!(
        output
            .typed_module
            .expr_types
            .values()
            .any(|recorded| *recorded == expected_type),
        "no expression was inferred as {expected_type:?}"
    );
}

macro_rules! check_expectation {
    ($output:expr, clean) => {
        assert_no_errors(&$output)
    };
    ($output:expr, rejected) => {
        assert_has_errors(&$output)
    };
    ($output:expr, (errors $count:literal)) => {
        assert_error_count(&$output, $count)
    };
    ($output:expr, (infers $inferred:expr)) => {
        assert_infers(&$output, $inferred)
    };
}

/// One named test per program: `check_cases!(name: run(source) => clean;)` (or `rejected`,
/// `(errors N)`, `(infers Type)`); the output may come from `run`, `run_kernel`, `run_project`
/// or `run_kernel_project`.
macro_rules! check_cases {
    ($($case:ident: $output:expr => $expected:tt;)+) => {
        $(
            // bug: typeck accepts a program it must reject, or rejects one it must accept (types, ownership, shapes, unions, options, channels, `background`, `wait` and suspension, kernel mode), so codegen lowers a wrong program or a correct one fails to compile | tier: High
            #[test]
            fn $case() {
                check_expectation!($output, $expected);
            }
        )+
    };
}

check_cases! {
    a_const_binding_is_clean: run("function entrypoint() -> Nothing { const x = 42\nprint(x) }") => clean;
    an_int_to_string_conversion_is_a_string: run(r#"function entrypoint() -> Nothing {
  let n: Int = 42
  let s = n.toString()
  print(s)
}"#) => clean;
    an_int_to_float_conversion_is_a_float: run("function entrypoint() -> Nothing { let x: Int = 5\nlet f: Float = x.toFloat()\nprint(f) }") => clean;
    an_int_literal_infers_as_int: run("function entrypoint() -> Nothing { let x = 42\nprint(x.toString()) }") => (infers Type::Int);
    a_string_literal_print_infers_a_string: run(r#"function entrypoint() -> Nothing { print(`hello, yinz`) }"#) => (infers Type::String);
    an_int_literal_stored_in_a_number_binding_is_one_error: run("function entrypoint() -> Nothing { let x: Number = 42\nprint(x) }") => (errors 1);
    a_decimal_literal_stored_in_a_number_binding_is_clean: run("function entrypoint() -> Nothing { let x: Number = 42.0\nprint(x) }") => clean;
    an_int_literal_in_a_float_binding_is_clean: run("function entrypoint() -> Nothing { let x: Float = 42\nprint(x) }") => clean;
    a_decimal_literal_is_clean: run("function entrypoint() -> Nothing { let x = 3.14\nprint(x) }") => clean;
    a_decimal_literal_in_a_float_binding_is_clean: run("function entrypoint() -> Nothing { let x: Float = 1.0\nprint(x) }") => clean;
    a_decimal_literal_in_an_int_binding_is_one_error: run("function entrypoint() -> Nothing { let x: Int = 1.5 }") => (errors 1);
    int_arithmetic_is_clean: run("function entrypoint() -> Nothing { let x: Int = 2\nlet y: Int = 3\nlet z = x + y\nprint(z) }") => clean;
    float_arithmetic_is_clean: run("function entrypoint() -> Nothing { let x: Float = 2.0\nlet y: Float = 3.0\nlet z = x + y\nprint(z) }") => clean;
    number_arithmetic_is_clean: run("function entrypoint() -> Nothing { let x = 0.1\nlet y = 0.2\nlet z = x + y\nprint(z) }") => clean;
    an_int_plus_a_number_is_one_error: run("function entrypoint() -> Nothing { let a: Int = 1\nlet b: Number = 2.0\nlet c = a + b }") => (errors 1);
    an_int_plus_a_float_is_one_error: run("function entrypoint() -> Nothing { let a: Int = 1\nlet b: Float = 2.0\nlet c = a + b }") => (errors 1);
    a_number_plus_a_float_is_one_error: run("function entrypoint() -> Nothing { let a = 0.1\nlet b: Float = 0.2\nlet c = a + b }") => (errors 1);
    a_percent_on_numbers_is_one_error: run("function entrypoint() -> Nothing { let a = 0.1\nlet b = 0.2\nlet c = a % b }") => (errors 1);
    a_chained_comparison_is_one_error: run("function entrypoint() -> Nothing { let x = 1 < 2 < 3 }") => (errors 1);
    a_comparison_result_is_a_bool: run("function entrypoint() -> Nothing { let x: Int = 1\nlet y: Int = 2\nlet z = x < y\nprint(z) }") => clean;
    and_on_bools_is_clean: run("function entrypoint() -> Nothing { let a = true\nlet b = false\nlet c = a && b\nprint(c) }") => clean;
    and_on_an_int_is_one_error: run("function entrypoint() -> Nothing { let x = 42 && true }") => (errors 1);
    negating_an_int_is_clean: run("function entrypoint() -> Nothing { let x: Int = 5\nlet y = -x\nprint(y) }") => clean;
    not_on_a_bool_is_clean: run("function entrypoint() -> Nothing { let a = true\nlet b = !a\nprint(b) }") => clean;
    negating_a_bool_is_one_error: run("function entrypoint() -> Nothing { let x = -true }") => (errors 1);
    reassigning_a_const_is_one_error: run("function entrypoint() -> Nothing { const x = 1\nx = 2 }") => (errors 1);
    calling_an_undefined_function_is_one_error: run(r#"function entrypoint() -> Nothing { unknownIdent() }"#) => (errors 1);
    reading_an_undefined_variable_is_one_error: run("function entrypoint() -> Nothing { let count = 42\nlet x = conut }") => (errors 1);
    assigning_to_an_undefined_name_is_one_error: run("function entrypoint() -> Nothing { x = 42 }") => (errors 1);
    print_with_two_arguments_is_one_error: run(r#"function entrypoint() -> Nothing { print(1, 2) }"#) => (errors 1);
    printing_an_int_literal_is_clean: run(r#"function entrypoint() -> Nothing { print(42) }"#) => clean;
    printing_a_decimal_literal_is_clean: run(r#"function entrypoint() -> Nothing { print(3.14) }"#) => clean;
    printing_a_bool_is_clean: run(r#"function entrypoint() -> Nothing { print(true) }"#) => clean;
    printing_a_float_is_clean: run("function entrypoint() -> Nothing { let f: Float = 1.0\nprint(f) }") => clean;
    an_unknown_method_is_one_error: run("function entrypoint() -> Nothing { let x: Int = 1\nlet s = x.unknownMethod() }") => (errors 1);
    a_parse_error_body_reports_only_the_parse_error: run("function entrypoint() -> Nothing { $ }") => (errors 1);
    an_empty_file_is_rejected: run("") => rejected;
    a_module_without_an_entrypoint_is_one_error: run(r#"function helper() -> Nothing { }"#) => (errors 1);
    an_entrypoint_with_parameters_is_one_error: run(r#"function entrypoint(x: Int) -> Nothing { }"#) => (errors 1);
    an_entrypoint_returning_a_value_is_one_error: run(r#"function entrypoint() -> Int { return 0 }"#) => (errors 1);
    recursion_with_a_condition_and_a_typed_return_is_clean: run(r#"function fib(n: Int) -> Int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}
function entrypoint() -> Nothing {
  let result = fib(10)
  print(result)
}"#) => clean;
    mutual_recursion_is_clean: run(r#"function ping(n: Int) -> Int {
  if (n <= 0) {
    return 0
  }
  return pong(n - 1)
}
function pong(n: Int) -> Int {
  if (n <= 0) {
    return 0
  }
  return ping(n - 1)
}
function entrypoint() -> Nothing {
  print(ping(5))
}"#) => clean;
    a_while_loop_with_a_bool_condition_is_clean: run(r#"function entrypoint() -> Nothing {
  let x: Int = 5
  while (x > 0) {
    x = x - 1
  }
  print(x)
}"#) => clean;
    a_for_range_loop_is_clean: run(r#"function entrypoint() -> Nothing {
  for (i in range(0, 5)) {
    print(i)
  }
}"#) => clean;
    a_for_range_loop_with_one_argument_is_clean: run(r#"function entrypoint() -> Nothing { for (i in range(5)) { print(i) } }"#) => clean;
    a_multi_case_on_an_int_is_clean: run(r#"function entrypoint() -> Nothing {
  let x: Int = 2
  if (x) {
    1 => print(`one`)
    2 => print(`two`)
    else => print(`other`)
  }
}"#) => clean;
    a_multi_case_on_a_string_is_clean: run(r#"function entrypoint() -> Nothing {
  let s = `hello`
  if (s) {
    `hello` => print(`hi`)
    else => print(`bye`)
  }
}"#) => clean;
    a_bare_return_in_a_nothing_function_is_clean: run(r#"function entrypoint() -> Nothing { return }"#) => clean;
    nested_calls_pass_each_result_to_the_outer_call_cleanly: run(r#"function add(a: Int, b: Int) -> Int { return a + b }
function entrypoint() -> Nothing { print(add(add(1, 2), add(3, 4))) }"#) => clean;
    a_non_exhaustive_multi_case_in_a_nothing_function_is_clean: run(r#"function entrypoint() -> Nothing {
  let x: Int = 3
  if (x) {
    1 => print(`one`)
    2 => print(`two`)
  }
  print(`done`)
}"#) => clean;
    a_duplicate_function_name_is_one_error: run(r#"function foo() -> Nothing { }
function foo() -> Nothing { }
function entrypoint() -> Nothing { }"#) => (errors 1);
    assigning_to_a_parameter_is_one_error: run(r#"function foo(x: Int) -> Int { x = 5 return x }
function entrypoint() -> Nothing { print(foo(1)) }"#) => (errors 1);
    assigning_to_a_loop_variable_is_one_error: run(r#"function entrypoint() -> Nothing { for (i in range(0, 5)) { i = 10 } }"#) => (errors 1);
    returning_a_wrong_type_is_one_error: run(r#"function foo() -> Int { return `hi` }
function entrypoint() -> Nothing { print(foo()) }"#) => (errors 1);
    a_missing_return_is_one_error: run("function foo() -> Int { print(`no return`) }\nfunction entrypoint() -> Nothing { print(foo()) }") => (errors 1);
    a_bare_return_in_a_value_function_is_one_error: run(r#"function foo() -> Int { return }
function entrypoint() -> Nothing { print(foo()) }"#) => (errors 1);
    a_value_return_in_a_nothing_function_is_one_error: run(r#"function entrypoint() -> Nothing { return 1 }"#) => (errors 1);
    a_non_exhaustive_multi_case_in_a_value_function_is_one_error: run(r#"function foo(x: Int) -> Int {
  if (x) {
    1 => return 1
    2 => return 2
  }
}
function entrypoint() -> Nothing { print(foo(1)) }"#) => (errors 1);
    a_multi_case_with_else_where_every_arm_returns_is_clean: run(r#"function foo(x: Int) -> Int {
  if (x) {
    1 => return 1
    else => return 0
  }
}
function entrypoint() -> Nothing { print(foo(1)) }"#) => clean;
    an_int_if_condition_is_one_error: run(r#"function entrypoint() -> Nothing { if (42) { print(`hi`) } }"#) => (errors 1);
    an_int_while_condition_is_one_error: run(r#"function entrypoint() -> Nothing { let x: Int = 1 while (x) { x = x - 1 } }"#) => (errors 1);
    a_range_stored_in_a_binding_is_clean: run(r#"function entrypoint() -> Nothing { let r = range(0, 5) }"#) => clean;
    range_with_three_arguments_is_one_error: run(r#"function entrypoint() -> Nothing { for (i in range(0, 5, 1)) { print(i) } }"#) => (errors 1);
    range_with_a_string_argument_is_one_error: run(r#"function entrypoint() -> Nothing { for (i in range(`hi`)) { print(i) } }"#) => (errors 1);
    function_arg_type_mismatch_produces_diagnostic: run(r#"function foo(x: Int) -> Nothing { }
function entrypoint() -> Nothing { foo(`hi`) }"#) => (errors 1);
    function_arg_arity_mismatch_produces_diagnostic: run(r#"function foo(x: Int) -> Nothing { }
function entrypoint() -> Nothing { foo(1, 2) }"#) => (errors 1);
    a_while_true_loop_without_a_return_is_one_error: run(r#"function foo() -> Int { while (true) { } }
function entrypoint() -> Nothing { print(foo()) }"#) => (errors 1);
    an_empty_value_function_is_one_error: run(r#"function foo() -> Int { }
function entrypoint() -> Nothing { print(foo()) }"#) => (errors 1);
    a_shape_built_from_a_struct_literal_is_clean: run(r#"
shape Player {
  name: String
  health: Int
}

function entrypoint() -> Nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  print(p.health)
}
"#) => clean;
    a_struct_literal_missing_a_field_is_one_error: run(r#"
shape Player { name: String health: Int }
function entrypoint() -> Nothing { let p: Player = { name: `x` } }
"#) => (errors 1);
    a_struct_literal_with_a_wrong_field_type_is_one_error: run(r#"
shape Player { name: String }
function entrypoint() -> Nothing { let p: Player = { name: 42 } }
"#) => (errors 1);
    a_struct_literal_with_an_unknown_field_is_one_error: run(r#"
shape Player { name: String }
function entrypoint() -> Nothing { let p: Player = { name: `x`, age: 30 } }
"#) => (errors 1);
    a_field_access_has_the_field_type: run(r#"
shape Player { name: String health: Int }
function entrypoint() -> Nothing {
  let p: Player = { name: `x`, health: 100 }
  let doubled = p.health * 2
  print(doubled)
}
"#) => clean;
    an_unknown_field_access_is_one_error: run(r#"
shape Player { name: String }
function entrypoint() -> Nothing {
  let p: Player = { name: `x` }
  let x = p.age
}
"#) => (errors 1);
    a_field_assignment_of_the_right_type_is_clean: run(r#"
shape Player { name: String health: Int }
function entrypoint() -> Nothing {
  let p: Player = { name: `x`, health: 100 }
  p.health = 50
}
"#) => clean;
    a_field_assignment_of_the_wrong_type_is_one_error: run(r#"
shape Player { name: String health: Int }
function entrypoint() -> Nothing {
  let p: Player = { name: `x`, health: 100 }
  p.health = `fifty`
}
"#) => (errors 1);
    assigning_a_field_of_a_const_is_one_error: run(r#"
shape Player { name: String health: Int }
function entrypoint() -> Nothing {
  const p: Player = { name: `x`, health: 100 }
  p.health = 50
}
"#) => (errors 1);
    building_a_base_shape_is_one_error: run(r#"
base shape Entity { name: String }
function entrypoint() -> Nothing { let e: Entity = { name: `x` } }
"#) => (errors 1);
    reading_a_hidden_field_outside_its_shape_is_one_error: run(r#"
shape Player { name: String hidden cache: Int = 0 }
function entrypoint() -> Nothing {
  let p: Player = { name: `x` }
  let x = p.cache
}
"#) => (errors 1);
    a_self_parameter_resolves_to_its_shape_and_is_clean: run(r#"
shape Player { name: String health: Int }
function greet(share self: Player) -> String { return self.name }
function entrypoint() -> Nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  let g = greet(p)
  print(g)
}
"#) => clean;
    a_dot_call_on_a_shape_resolves_and_is_clean: run(r#"
shape Player { name: String }
function greet(share self: Player) -> String { return self.name }
function entrypoint() -> Nothing {
  let p: Player = { name: `Patrick` }
  let msg = p.greet()
  print(msg)
}
"#) => clean;
    a_duplicate_shape_name_is_one_error: run(r#"
shape Player { name: String }
shape Player { health: Int }
function entrypoint() -> Nothing { }
"#) => (errors 1);
    a_struct_literal_without_an_annotation_is_one_error: run(r#"
shape Player { name: String }
function entrypoint() -> Nothing { let p = { name: `x` } }
"#) => (errors 1);
    a_child_shape_reads_its_parent_fields_cleanly: run(r#"
shape Entity { name: String }
shape Player extends Entity { health: Int }
function entrypoint() -> Nothing {
  let p: Player = { name: `Patrick`, health: 100 }
  print(p.name)
  print(p.health)
}
"#) => clean;
    extending_an_unknown_parent_is_one_error: run(r#"
shape Player extends Ghost { health: Int }
function entrypoint() -> Nothing { }
"#) => (errors 1);
    a_satisfied_contract_is_clean: run(r#"
shape Damageable {
  takeDamage(lend self, amount: Int) -> Nothing
}
shape Player follows Damageable { health: Int }
function takeDamage(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health - amount
}
function entrypoint() -> Nothing {
  let p: Player = { health: 100 }
  takeDamage(p, 10)
}
"#) => clean;
    a_contract_with_a_missing_function_is_one_error: run(r#"
shape Damageable { takeDamage(lend self, amount: Int) -> Nothing }
shape Player follows Damageable { health: Int }
function entrypoint() -> Nothing { }
"#) => (errors 1);
    a_contract_function_with_a_wrong_return_type_is_one_error: run(r#"
shape Greetable { greet(share self) -> String }
shape Player follows Greetable { name: String }
function greet(share self: Player) -> Int { return 42 }
function entrypoint() -> Nothing { }
"#) => (errors 1);
    reading_a_given_binding_is_one_error: run(r#"
shape Player { name: String }
function consume(give p: Player) -> Nothing { }
function entrypoint() -> Nothing {
  let p: Player = { name: `Patrick` }
  consume(p)
  let x = p.name
}
"#) => (errors 1);
    giving_a_const_is_one_error: run(r#"
shape Player { name: String }
function consume(give p: Player) -> Nothing { }
function entrypoint() -> Nothing {
  const p: Player = { name: `Patrick` }
  consume(p)
}
"#) => (errors 1);
    lending_a_const_is_one_error: run(r#"
shape Player { health: Int }
function damage(lend self: Player, amount: Int) -> Nothing {
  self.health = self.health - amount
}
function entrypoint() -> Nothing {
  const p: Player = { health: 100 }
  damage(p, 10)
}
"#) => (errors 1);
    giving_a_binding_twice_is_one_error: run(r#"
shape Player { name: String }
function consume(give p: Player) -> Nothing { }
function entrypoint() -> Nothing {
  let p: Player = { name: `Patrick` }
  consume(p)
  consume(p)
}
"#) => (errors 1);
    sharing_a_const_is_clean: run(r#"
shape Player { name: String }
function greet(share p: Player) -> String { return p.name }
function entrypoint() -> Nothing {
  const p: Player = { name: `Patrick` }
  let g = greet(p)
  print(g)
}
"#) => clean;
    passing_a_const_to_an_unmarked_parameter_is_clean: run(r#"
shape Player { name: String }
function greet(p: Player) -> String { return p.name }
function entrypoint() -> Nothing {
  const p: Player = { name: `Patrick` }
  let g = greet(p)
  print(g)
}
"#) => clean;
    a_union_annotation_accepts_a_variant_and_is_clean: run(r#"
shape Circle { radius: Number }
shape Square { side: Number }
shape Shape = Circle | Square
function describe(s: Shape) -> Nothing {
  if (s) {
    is Circle => print(`circle`)
    is Square => print(`square`)
  }
}
function entrypoint() -> Nothing {
  let c: Circle = { radius: 5.0 }
  let s: Shape = c
  describe(s)
}
"#) => clean;
    an_exhaustive_union_multi_case_is_clean: run(r#"
shape Circle { radius: Number }
shape Square { side: Number }
shape Shape = Circle | Square
function classify(s: Shape) -> Nothing {
  if (s) {
    is Circle => print(`circle`)
    is Square => print(`square`)
  }
}
function entrypoint() -> Nothing { }
"#) => clean;
    a_non_exhaustive_union_multi_case_is_one_error: run(r#"
shape Circle { radius: Number }
shape Square { side: Number }
shape Triangle { width: Number
  height: Number }
shape Shape = Circle | Square | Triangle
function classify(s: Shape) -> Nothing {
  if (s) {
    is Circle => print(`circle`)
    is Square => print(`square`)
  }
}
function entrypoint() -> Nothing { }
"#) => (errors 1);
    a_union_multi_case_with_else_is_clean: run(r#"
shape Circle { radius: Number }
shape Square { side: Number }
shape Triangle { width: Number
  height: Number }
shape Shape = Circle | Square | Triangle
function classify(s: Shape) -> Nothing {
  if (s) {
    is Circle => print(`circle`)
    else => print(`other`)
  }
}
function entrypoint() -> Nothing { }
"#) => clean;
    an_is_check_on_a_non_union_is_one_error: run(r#"
function entrypoint() -> Nothing {
  let s: String = `hello`
  if (s is Int) {
    print(`wrong`)
  }
}
"#) => (errors 1);
    an_options_value_is_clean: run(r#"
options Status { active, inactive, banned }
function entrypoint() -> Nothing {
  let s: Status = Status.active
  print(s.toString())
}
"#) => clean;
    an_unknown_options_variant_is_one_error: run(r#"
options Status { active, inactive, banned }
function entrypoint() -> Nothing {
  let s: Status = Status.activ
}
"#) => (errors 1);
    an_empty_options_type_is_one_error: run(r#"
options Empty { }
function entrypoint() -> Nothing { }
"#) => (errors 1);
    a_single_variant_options_type_is_one_error: run(r#"
options Single { only }
function entrypoint() -> Nothing { }
"#) => (errors 1);
    an_exhaustive_options_multi_case_is_clean: run(r#"
options Status { active, inactive }
function entrypoint() -> Nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    inactive => print(`off`)
  }
}
"#) => clean;
    a_non_exhaustive_options_multi_case_is_one_error: run(r#"
options Status { active, inactive, banned }
function entrypoint() -> Nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    inactive => print(`off`)
  }
}
"#) => (errors 1);
    an_options_multi_case_with_else_is_clean: run(r#"
options Status { active, inactive, banned }
function entrypoint() -> Nothing {
  let s: Status = Status.active
  if (s) {
    active => print(`ok`)
    else => print(`other`)
  }
}
"#) => clean;
    comparing_two_values_of_one_options_type_is_clean: run(r#"
options Status { active, inactive }
function entrypoint() -> Nothing {
  let a: Status = Status.active
  let b: Status = Status.inactive
  let eq = a == b
  print(eq.toString())
}
"#) => clean;
    comparing_values_of_two_options_types_is_one_error: run(r#"
options Status { active, inactive }
options Visibility { visible, invisible }
function entrypoint() -> Nothing {
  let s: Status = Status.active
  let v: Visibility = Visibility.visible
  let eq = s == v
}
"#) => (errors 1);
    converting_a_bool_to_an_int_is_one_error: run(r#"
function entrypoint() -> Nothing {
  let x = true.toInt()
}
"#) => (errors 1);
    converting_an_int_to_an_int_is_clean: run(r#"
function entrypoint() -> Nothing {
  let x: Int = 42
  let y: Int = x.toInt()
  print(y.toString())
}
"#) => clean;
    converting_a_float_to_an_int_gives_a_maybe: run(r#"
function entrypoint() -> Nothing {
  let x: Float = 3.14
  let y: Maybe<Int> = x.toInt()
  print(y.or(0).toString())
}
"#) => clean;
    converting_a_string_to_an_int_gives_a_maybe: run(r#"
function entrypoint() -> Nothing {
  let s: String = `42`
  let x: Maybe<Int> = s.toInt()
  print(x.or(0).toString())
}
"#) => clean;
    a_grandchild_shape_reads_every_ancestor_field_cleanly: run(r#"
shape A { x: Int }
shape B extends A { y: Int }
shape C extends B { z: Int }

function entrypoint() -> Nothing {
  let c: C = { x: 1, y: 2, z: 3 }
  print(`${c.x}`)
  print(`${c.y}`)
  print(`${c.z}`)
}
"#) => clean;
    background_with_a_share_parameter_callee_is_one_error: run(r#"
function readData(share data: String) -> Nothing {
  print(data)
}

function entrypoint() -> Nothing {
  let s: String = `hello`
  background readData(s)
}
"#) => (errors 1);
    a_hidden_field_without_a_default_is_one_error: run(r#"
shape Foo {
    name: String
    hidden bar: Int
}
function entrypoint() -> Nothing {}
"#) => (errors 1);
    building_a_shape_with_a_defaulted_hidden_field_is_clean: run(r#"
shape Foo {
    name: String
    hidden bar: Int = 0
}

function entrypoint() -> Nothing {
    const x: Foo = { name: `hello` }
    print(x.name)
}
"#) => clean;
    a_background_handle_of_a_non_suspending_callee_is_one_error: run(r#"
function readData() -> Nothing { print(`hello`) }
function entrypoint() -> Nothing {
  let h = background readData()
}
"#) => (errors 1);
    a_background_handle_of_a_suspending_callee_is_clean: run(r#"
function worker() -> Int errors {
  wait sleep(1)
  return 42
}
function entrypoint() -> Nothing {
  let h = background worker()
  let r = h.receive()
  let v = r.or(0)
  print(v.toString())
}
"#) => clean;
    an_inline_shape_annotation_is_clean: run(r#"
function entrypoint() -> Nothing {
  let p: { a: Int, b: String } = { a: 1, b: `hi` }
  print(p.b)
}
"#) => clean;
    inline_shapes_with_the_same_fields_in_another_order_are_one_type: run(r#"
function takesAB(p: { a: Int, b: Int }) -> Nothing { print(`${p.a}`) }
function entrypoint() -> Nothing {
  let x: { b: Int, a: Int } = { a: 1, b: 2 }
  takesAB(x)
}
"#) => clean;
    an_inline_shape_with_an_unknown_field_is_one_error: run(r#"
function entrypoint() -> Nothing {
  let x: { a: Int } = { a: 1, b: 2 }
}
"#) => (errors 1);
    an_inline_shape_missing_a_field_is_one_error: run(r#"
function entrypoint() -> Nothing {
  let x: { a: Int, b: Int } = { a: 1 }
}
"#) => (errors 1);
    a_named_shape_is_not_an_inline_shape_of_the_same_fields: run(r#"
shape Foo { a: Int }
function entrypoint() -> Nothing {
  let x: Foo = { a: 1 }
  let y: { a: Int } = x
}
"#) => (errors 1);
    a_nested_inline_shape_is_clean: run(r#"
function entrypoint() -> Nothing {
  let p: { outer: { inner: Int } } = { outer: { inner: 42 } }
  print(`${p.outer.inner}`)
}
"#) => clean;
    an_inline_shape_in_a_fixed_list_destructures_cleanly: run(r#"
function entrypoint() -> Nothing {
  const intervals: Fixed<{ minutes: Int }> = [
    { minutes: 5 },
    { minutes: 15 },
    { minutes: 60 },
  ]
  for ({ minutes } in intervals) {
    print(`${minutes}`)
  }
}
"#) => clean;
    an_inline_shape_parameter_is_clean: run(r#"
function area(rect: { w: Int, h: Int }) -> Int {
  return rect.w * rect.h
}
function entrypoint() -> Nothing {
  let r: { w: Int, h: Int } = { w: 5, h: 3 }
  print(`${area(r)}`)
}
"#) => clean;
    a_hidden_field_in_an_inline_shape_is_rejected: run(r#"
function entrypoint() -> Nothing {
  let p: { a: Int, hidden b: Int } = { a: 1 }
}
"#) => rejected;
    an_unknown_type_in_an_inline_shape_is_one_error: run(r#"
function entrypoint() -> Nothing {
  const intervals: Fixed<{ minutes: Int, timeframe: Timeframe }> = []
}
"#) => (errors 1);
    an_unknown_type_inside_a_maybe_field_is_one_error: run(r#"
function entrypoint() -> Nothing {
  let x: { val: Maybe<GhostType> } = { val: none }
}
"#) => (errors 1);
    sleep_blocking_with_an_int_is_clean: run("function entrypoint() -> Nothing {\n  sleepBlocking(50)\n}") => clean;
    sleep_blocking_with_a_string_is_rejected: run("function entrypoint() -> Nothing {\n  sleepBlocking(`not an int`)\n}") => rejected;
    sleep_blocking_with_no_arguments_is_rejected: run("function entrypoint() -> Nothing {\n  sleepBlocking()\n}") => rejected;
    sleep_blocking_with_two_arguments_is_rejected: run("function entrypoint() -> Nothing {\n  sleepBlocking(50, 100)\n}") => rejected;
    background_with_a_lend_parameter_is_rejected: run("function mutate(lend x: Int) -> Nothing { }\n\
               function entrypoint() -> Nothing { let x: Int = 5\n background mutate(x) }") => rejected;
    wait_in_kernel_mode_is_rejected: run_kernel("function slow() -> Int { return 1 }\n\
               function entrypoint() -> Nothing { let x = wait slow() }") => rejected;
    a_channel_in_kernel_mode_is_rejected: run_kernel("function entrypoint() -> Nothing { let ch: Channel<Int> = Channel<Int>() }") => rejected;
    channels_with_default_and_explicit_capacity_are_clean: run("function entrypoint() -> Nothing {\n\
               let a: Channel<Int> = Channel<Int>()\n\
               let b: Channel<String> = Channel<String>(16)\n }") => clean;
    a_zero_channel_capacity_is_rejected: run("function entrypoint() -> Nothing { let c: Channel<Int> = Channel<Int>(0) }") => rejected;
    a_negated_channel_capacity_is_rejected: run("function entrypoint() -> Nothing { let c: Channel<Int> = Channel<Int>(-1) }") => rejected;
    a_channel_without_an_element_type_is_rejected: run("function entrypoint() -> Nothing { let c = Channel() }") => rejected;
    a_channel_with_a_string_capacity_is_rejected: run("function entrypoint() -> Nothing { let c: Channel<Int> = Channel<Int>(`nope`) }") => rejected;
    a_channel_read_from_a_shape_field_without_an_annotation_is_one_error: run("shape Box { wire: Channel<Int> }\n\
               function drain(b: Box) -> Nothing {\n\
                 let c = b.wire\n\
                 let first = c.receive()\n\
                 print(first)\n\
               }\n\
               function entrypoint() -> Nothing { }") => (errors 1);
    a_channel_read_from_a_maybe_value_without_an_annotation_is_one_error: run("function drain(chans: Array<Channel<Int>>) -> Nothing {\n\
                 let slot = chans[0]\n\
                 if (slot.exists()) {\n\
                   let c = slot.value\n\
                   let first = c.receive()\n\
                   print(first)\n\
                 }\n\
               }\n\
               function entrypoint() -> Nothing { }") => (errors 1);
    a_channel_loop_variable_receiver_is_one_error: run("function drain(chans: Array<Channel<Int>>) -> Nothing {\n\
                 for (c in chans) {\n\
                   let first = c.receive()\n\
                   print(first)\n\
                 }\n\
               }\n\
               function entrypoint() -> Nothing { }") => (errors 1);
    a_channel_read_from_a_shape_field_with_an_annotation_is_clean: run("shape Box { wire: Channel<Int> }\n\
         function drain(b: Box) -> Nothing {\n\
           let c: Channel<Int> = b.wire\n\
           let first = c.receive()\n\
           let value = first.or(0)\n\
           print(value)\n\
         }\n\
         function entrypoint() -> Nothing { }") => clean;
    an_alias_of_a_channel_binding_is_clean: run("function entrypoint() -> Nothing {\n\
           let a: Channel<Int> = Channel<Int>(4)\n\
           let b2 = a\n\
           let first = b2.receive()\n\
           let value = first.or(0)\n\
           print(value)\n\
         }") => clean;
    a_handle_read_from_a_maybe_value_is_one_error: run("function worker() -> Int {\n\
                 sleep(10)\n\
                 return 42\n\
               }\n\
               function entrypoint() -> Nothing {\n\
                 let h = background worker()\n\
                 let hs = [h]\n\
                 let slot = hs[0]\n\
                 if (slot.exists()) {\n\
                   let h2 = slot.value\n\
                   let v = h2.receive()\n\
                   let got = v.or(0)\n\
                   print(got)\n\
                 }\n\
               }") => (errors 1);
    background_in_kernel_mode_is_rejected: run_kernel("function process() -> Nothing { }\n\
               function entrypoint() -> Nothing { background process() }") => rejected;
    background_inside_a_for_loop_is_clean: run("function process(i: Int) -> Nothing { }\n\
               function entrypoint() -> Nothing {\n  for (i in range(0, 3)) {\n    background process(i)\n  }\n}") => clean;
    background_of_a_dot_call_with_lend_self_is_rejected: run("shape Counter { n: Int }\n\
               function increment(lend self: Counter) -> Nothing { self.n = self.n + 1 }\n\
               function entrypoint() -> Nothing {\n  let c: Counter = { n: 0 }\n  background c.increment()\n}") => rejected;
    reading_a_binding_given_to_background_is_rejected: run("function process(give x: Int) -> Nothing { }\n\
               function entrypoint() -> Nothing {\n  let x: Int = 5\n  background process(x)\n  print(x)\n}") => rejected;
    wait_inside_a_while_loop_is_clean: run(r#"
function entrypoint() -> Nothing {
  let i: Int = 0
  while (i < 3) {
    wait sleep(100)
    i = i + 1
  }
}
"#) => clean;
    wait_inside_a_for_loop_is_clean: run(r#"
function entrypoint() -> Nothing {
  for (i in range(0, 3)) {
    wait sleep(100)
  }
}
"#) => clean;
    a_local_read_after_a_wait_is_clean: run(r#"
function entrypoint() -> Nothing {
  let x: Int = 5
  wait sleep(30)
  print(x.toString())
}
"#) => clean;
    a_parameter_read_after_a_wait_is_clean: run(r#"
function pause(n: Int) -> Nothing {
  print(n.toString())
  wait sleep(100)
  print(n.toString())
}
function entrypoint() -> Nothing {
  background pause(1)
}
"#) => clean;
    wait_at_the_top_level_and_in_an_if_is_clean: run(r#"
function maybeWait(b: Boolean) -> Nothing {
  if (b) {
    wait sleep(50)
  }
  wait sleep(10)
}
function entrypoint() -> Nothing {
  background maybeWait(true)
  sleepBlocking(200)
}
"#) => clean;
    a_local_read_after_an_if_holding_a_wait_is_clean: run(r#"
function entrypoint() -> Nothing {
  let x: Int = 5
  if (x < 10) {
    wait sleep(30)
  }
  print(x.toString())
}
"#) => clean;
    a_local_read_inside_the_branch_after_its_wait_is_clean: run(r#"
function entrypoint() -> Nothing {
  let x: Int = 5
  if (x < 10) {
    wait sleep(30)
    print(x.toString())
  }
}
"#) => clean;
    a_branch_local_read_after_the_wait_in_its_branch_is_clean: run(r#"
function entrypoint() -> Nothing {
  if (true) {
    let y: Int = 7
    wait sleep(30)
    print(y.toString())
  }
}
"#) => clean;
    a_local_used_only_before_a_wait_is_clean: run(r#"
function entrypoint() -> Nothing {
  let x: Int = 5
  print(x.toString())
  wait sleep(10)
}
"#) => clean;
    a_local_declared_after_a_wait_is_clean: run(r#"
function entrypoint() -> Nothing {
  wait sleep(10)
  let z: Int = 99
  print(z.toString())
}
"#) => clean;
    a_result_read_after_a_later_suspension_is_clean: run(r#"
function sleeper() -> Int {
  wait sleep(10)
  return 5
}
function compute() -> Int {
  let slot = sleeper()
  let other = sleeper()
  return slot + other
}
function entrypoint() -> Nothing {
  background compute()
  sleepBlocking(200)
}
"#) => clean;
    a_result_unused_after_a_later_suspension_is_clean: run(r#"
function sleeper() -> Int {
  wait sleep(10)
  return 5
}
function compute(n: Int) -> Int {
  let slot = sleeper()
  let other = sleeper()
  return other
}
function entrypoint() -> Nothing {
  background compute(1)
  sleepBlocking(200)
}
"#) => clean;
    a_result_used_before_any_second_suspension_is_clean: run(r#"
function sleeper() -> Int {
  wait sleep(10)
  return 5
}
function compute(n: Int) -> Int {
  let a = sleeper()
  return a
}
function entrypoint() -> Nothing {
  background compute(1)
  sleepBlocking(200)
}
"#) => clean;
    wait_on_a_literal_is_rejected: run(r#"
function entrypoint() -> Nothing {
  wait 42
}
"#) => rejected;
    wait_of_a_wait_is_rejected: run(r#"
function entrypoint() -> Nothing {
  wait (wait sleep(10))
}
"#) => rejected;
    wait_on_sleep_is_clean: run(r#"
function entrypoint() -> Nothing {
  wait sleep(100)
}
"#) => clean;
    wait_on_a_non_suspending_call_is_clean: run(r#"
function entrypoint() -> Nothing {
  wait print(`hi`)
}
"#) => clean;
    sleep_without_wait_is_clean: run(r#"
function entrypoint() -> Nothing {
  sleep(100)
}
"#) => clean;
    a_suspending_call_without_wait_is_clean: run(r#"
function inner() -> Nothing {
  wait sleep(10)
}
function outer() -> Nothing {
  wait sleep(5)
  inner()
}
function entrypoint() -> Nothing {
  background outer()
}
"#) => clean;
    a_suspending_call_with_wait_is_clean: run(r#"
function inner() -> Nothing {
  wait sleep(10)
}
function outer() -> Nothing {
  wait sleep(5)
  wait inner()
}
function entrypoint() -> Nothing {
  background outer()
}
"#) => clean;
    background_of_a_suspending_call_is_clean: run(r#"
function inner() -> Nothing {
  wait sleep(10)
}
function outer() -> Nothing {
  wait sleep(5)
  background inner()
}
function entrypoint() -> Nothing {
  background outer()
}
"#) => clean;
    a_plain_function_calling_a_suspending_one_is_clean: run(r#"
function inner() -> Nothing {
  wait sleep(10)
}
function outer() -> Nothing {
  inner()
}
function entrypoint() -> Nothing {
  outer()
}
"#) => clean;
    sleep_in_kernel_mode_is_rejected: run_kernel(r#"
function entrypoint() -> Nothing {
  sleep(100)
}
"#) => rejected;
    a_non_suspending_call_inside_a_waited_call_is_clean: run(r#"
function addOne(n: Int) -> Int {
  return n + 1
}
function inner(n: Int) -> Nothing {
  sleepBlocking(n)
}
function entrypoint() -> Nothing {
  wait inner(addOne(5))
}
"#) => clean;
    a_suspending_call_inside_a_waited_call_is_rejected: run(r#"
function wrapper(n: Nothing) -> Nothing {
  sleepBlocking(1)
}
function entrypoint() -> Nothing {
  wait wrapper(sleep(100))
}
"#) => rejected;
    a_suspending_call_in_a_background_argument_is_rejected: run(r#"
function sm_bar() -> Nothing {
  wait sleep(10)
}
function foo(ignored: Nothing) -> Nothing {
  wait sleep(10)
}
function entrypoint() -> Nothing {
  wait sleep(1)
  background foo(sm_bar())
}
"#) => rejected;
    a_fallible_async_call_with_its_argument_is_clean: run(r#"
function entrypoint() -> Nothing {
  wait __testFallibleAsync(true)
}
"#) => clean;
    a_fallible_async_call_with_no_argument_is_rejected: run(r#"
function entrypoint() -> Nothing {
  wait __testFallibleAsync()
}
"#) => rejected;
    a_dot_call_through_a_dynamic_value_in_a_suspending_function_is_rejected: run(r#"
shape Worker {
    doWork(share self) -> Nothing
}

shape FastWorker follows Worker {
    speed: Int
}

function doWork(share self: FastWorker) -> Nothing {
    print(`fast`)
}

function runWorker(w: dynamic Worker) -> Nothing {
    sleep(1)
    w.doWork()
}

function entrypoint() -> Nothing {
    let fw: FastWorker = { speed: 10 }
    runWorker(fw)
}
"#) => rejected;
    a_dynamic_argument_in_a_suspending_function_is_rejected: run(r#"
shape Worker {
    doWork(share self) -> Nothing
}

shape FastWorker follows Worker {
    speed: Int
}

function doWork(share self: FastWorker) -> Nothing {
    print(`fast`)
}

function dispatch(w: dynamic Worker) -> Nothing {
    w.doWork()
}

function relay(w: dynamic Worker) -> Nothing {
    sleep(1)
    dispatch(w)
}

function entrypoint() -> Nothing {
    let fw: FastWorker = { speed: 10 }
    relay(fw)
}
"#) => rejected;
    dynamic_dispatch_in_a_non_suspending_function_is_clean: run(r#"
shape Worker {
    doWork(share self) -> Nothing
}

shape FastWorker follows Worker {
    speed: Int
}

function doWork(share self: FastWorker) -> Nothing {
    print(`fast`)
}

function relay(w: dynamic Worker) -> Nothing {
    w.doWork()
}

function entrypoint() -> Nothing {
    let fw: FastWorker = { speed: 10 }
    relay(fw)
}
"#) => clean;
    failed_and_or_on_an_errors_value_are_clean: run(r#"
function compute() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let x = compute()
  const failed = x.failed()
  const v = x.or(0)
  print(failed.toString())
  print(v.toString())
}
"#) => clean;
    or_on_a_shadowing_int_binding_is_one_error: run(r#"
function compute() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let x = compute()
  const outer_v = x.or(0)
  print(outer_v.toString())
  let x: Int = 7
  const inner_v = x.or(0)
  print(inner_v.toString())
}
"#) => (errors 1);
    or_on_an_int_inside_an_errors_function_is_one_error: run(r#"
function compute() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let n: Int = 5
  let _ = n.or(0)
  let x = compute()
  const v = x.or(0)
  print(v.toString())
}
"#) => (errors 1);
    or_on_an_int_in_a_plain_function_is_one_error: run(r#"
function entrypoint() -> Nothing {
  let n: Int = 5
  let _ = n.or(0)
}
"#) => (errors 1);
    message_inside_a_failed_check_is_clean: run(r#"
function compute() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let x = compute()
  if (x.failed()) {
    const msg = x.message
    print(msg)
  }
}
"#) => clean;
    or_after_a_failed_guard_is_clean: run(r#"
function compute() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  let x = compute()
  if (x.failed()) {
    print(`failed`)
  }
  const v = x.or(0)
  print(v.toString())
}
"#) => clean;
    or_on_a_const_errors_value_is_clean: run(r#"
function compute() -> Int errors {
  return 42
}

function entrypoint() -> Nothing {
  const x = compute()
  const v = x.or(0)
  print(v.toString())
}
"#) => clean;
}

// ── multi-file programs ─────────────────────────────────────────────────────

check_cases! {
    building_a_shape_with_a_hidden_field_from_another_file_is_rejected: run_project(
        &[
            ("player.ynz", "export shape Player { name: String hidden secret: Int = 0 }"),
            (
                "entrypoint.ynz",
                "import { Player } from `player`\n\
                 function entrypoint() -> Nothing {\n\
                     const p: Player = { name: `Alice`, secret: 42 }\n\
                 }",
            ),
        ],
        "entrypoint.ynz",
    ) => rejected;
    an_inline_shape_parameter_of_an_import_accepts_the_same_inline_shape: run_project(
        &[
            (
                "a.ynz",
                "export function takesAB(p: { a: Int, b: String }) -> Nothing { print(p.b) }",
            ),
            (
                "b.ynz",
                "import { takesAB } from `a`\n\
                 function entrypoint() -> Nothing { takesAB({ a: 1, b: `hi` }) }",
            ),
        ],
        "b.ynz",
    ) => clean;
    a_suspending_function_calling_a_non_suspending_import_is_clean: run_project(
        &[
            ("utils.ynz", "export function remoteOp() -> Nothing { print(`remote op`) }"),
            (
                "entrypoint.ynz",
                "import { remoteOp } from `utils`\n\
                 function entrypoint() -> Nothing { sleep(10)\n  remoteOp() }",
            ),
        ],
        "entrypoint.ynz",
    ) => clean;
    a_bare_suspending_import_call_in_kernel_mode_is_rejected: run_kernel_project(
        &[
            ("leaf_ops.ynz", "export function longJob() -> Nothing {\n  sleep(100)\n}\n"),
            (
                "entrypoint.ynz",
                "import { longJob } from `leaf_ops`\nfunction entrypoint() -> Nothing {\n  longJob()\n}\n",
            ),
        ],
        "entrypoint.ynz",
    ) => rejected;
    a_dot_call_to_a_suspending_import_in_kernel_mode_is_rejected: run_kernel_project(
        &[
            (
                "worker_ops.ynz",
                "export shape Worker { name: String }\nexport function longJob(share self: Worker) -> Nothing {\n  sleep(100)\n}\n",
            ),
            (
                "entrypoint.ynz",
                "import { Worker, longJob } from `worker_ops`\nfunction entrypoint() -> Nothing {\n  const w: Worker = { name: `bob` }\n  w.longJob()\n}\n",
            ),
        ],
        "entrypoint.ynz",
    ) => rejected;
}

// ── a function reaching `sleep` through a callee is a state machine ─────────

// bug: the may-block fixpoint does not propagate through a callee, so a function that reaches `sleep` is not marked suspending and compiles as a plain function | tier: High
#[test]
fn transitive_no_wait_compiles_clean_under_inference() {
    // bar calls sleep without wait, foo calls bar, and `wait foo()` is valid since foo suspends.
    let src = r#"
function bar() -> Nothing {
  sleep(100)
}
function foo() -> Nothing {
  bar()
}
function entrypoint() -> Nothing {
  wait foo()
}
"#;
    assert_no_errors(&run(src));
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, FILE.to_string(), src.to_string());
    let module = ynz_parser::parse_query(&db, sf).module.clone();
    let suspends = ynz_typeck::may_block_suspends_set(&module, &std::collections::HashSet::new());
    let unmarked: Vec<&str> = ["bar", "foo"]
        .into_iter()
        .filter(|name| !suspends.contains(*name))
        .collect();
    assert!(
        unmarked.is_empty(),
        "the fixpoint must mark bar (direct sleep caller) and foo (transitive via bar); unmarked: {unmarked:?}, set={suspends:?}"
    );
}

// ── the signature pass ──────────────────────────────────────────────────────

// bug: the signature pass drops a missing-entrypoint error or check_query omits the signature pass's diagnostics, so a module without an entrypoint compiles | tier: Medium
#[test]
fn module_signatures_query_is_separate_from_check_query() {
    let db = CompilerDb::default();
    let sf = SourceFile::new(
        &db,
        FILE.to_string(),
        "function helper() -> Nothing { }".to_string(),
    );
    let sig_out = ynz_typeck::module_signatures_query(&db, sf);
    assert_eq!(
        sig_out.diagnostics.len(),
        1,
        "Missing main must appear in signature output"
    );
    let check_out = check_query(&db, sf);
    assert!(
        !check_out.diagnostics.is_empty(),
        "Missing main must appear in check output"
    );
}

// ── checking again after an edit ────────────────────────────────────────────

/// Check each of `probes` in a project holding `files`, replace the text of `edit.0` with
/// `edit.1`, and check the probes again: the outputs before and after the edit.
fn recheck(
    files: &[(&str, &str)],
    edit: (&str, &str),
    probes: &[&str],
) -> (Vec<CheckOutput>, Vec<CheckOutput>) {
    let mut project = Project::new(files);
    let before = probes.iter().map(|probe| project.check(probe)).collect();
    project.edit(edit.0, edit.1);
    let after = probes.iter().map(|probe| project.check(probe)).collect();
    (before, after)
}

/// For each of `names`, whether it is in the suspends set of the output that checked it (one
/// output per name, in order).
fn suspends_flags(outputs: &[CheckOutput], names: &[&str]) -> Vec<bool> {
    outputs
        .iter()
        .zip(names)
        .map(|(output, name)| output.suspends_set.contains(*name))
        .collect()
}

/// One named test per edit: `recheck_cases!(name: files [..], edit (file, text), probes [..],
/// |before, after| { checks }, "bug it catches";)`.
macro_rules! recheck_cases {
    ($($case:ident: files $files:tt, edit $edit:tt, probes $probes:tt, |$before:ident, $after:ident| $checks:block, $bug:literal;)+) => {
        $(
            // bug: salsa serves a stale check result after an edit (an unchanged diagnostic list, a suspends set that does not follow an imported function), so a wrong program is accepted, a correct one refused, or a function compiled on the wrong state-machine path | tier: Medium
            #[doc = $bug]
            #[test]
            fn $case() {
                let ($before, $after) = recheck(&$files, $edit, &$probes);
                $checks
            }
        )+
    };
}

recheck_cases! {
    a_same_file_edit_is_checked_again: files [("test.ynz", "")],
        edit ("test.ynz", "function entrypoint() -> Nothing { print(`hi`) }"),
        probes ["test.ynz"], |before, after| {
        assert!(!before[0].diagnostics.is_empty(), "an empty file has diagnostics");
        assert!(
            after[0].diagnostics.is_empty(),
            "a valid program has no diagnostics, got {:#?}",
            after[0].diagnostics
        );
    }, "salsa serves the old diagnostics after a same-file edit, so a fixed program still shows its errors or a broken one shows none";
    a_changed_import_signature_is_checked_again: files [
        ("b.ynz", "export function foo(x: Int) -> Int { return x }"),
        (
            "a.ynz",
            "import { foo } from `b`\nfunction entrypoint() -> Nothing { let r = foo(42) }",
        ),
    ],
        edit ("b.ynz", "export function foo(x: String) -> Int { return 42 }"),
        probes ["a.ynz"], |before, after| {
        // Same function name, different signature: a coarse `PartialEq` on the tracked output
        // used to make salsa skip the re-check.
        assert_no_errors(&before[0]);
        assert_has_errors(&after[0]);
    }, "salsa skips the re-check of a caller after an imported function changes signature, so a call that no longer type-checks is accepted";
    an_import_that_starts_suspending_flips_its_caller_to_a_state_machine: files [
        ("a.ynz", "export function op() -> Nothing { print(`op`) }"),
        ("b.ynz", "import { op } from `a`\nfunction caller() -> Nothing { op() }"),
    ],
        edit ("a.ynz", "export function op() -> Nothing { sleep(10)\nprint(`op`) }"),
        probes ["b.ynz"], |before, after| {
        assert_eq!(suspends_flags(&before, &["caller"]), vec![false]);
        assert_eq!(suspends_flags(&after, &["caller"]), vec![true]);
    }, "salsa keeps a caller off the state-machine path after its import starts suspending, so the caller is compiled as a plain function around a suspension";
    an_import_that_stops_suspending_clears_its_caller_from_the_state_machines: files [
        ("a.ynz", "export function op() -> Nothing { sleep(10)\nprint(`op`) }"),
        ("b.ynz", "import { op } from `a`\nfunction caller() -> Nothing { op() }"),
    ],
        edit ("a.ynz", "export function op() -> Nothing { print(`op`) }"),
        probes ["b.ynz"], |before, after| {
        assert_eq!(suspends_flags(&before, &["caller"]), vec![true]);
        assert_eq!(suspends_flags(&after, &["caller"]), vec![false]);
    }, "salsa keeps a caller on the state-machine path after its import stops suspending, so it is still compiled with a frame it does not need";
    an_import_shared_by_two_files_flips_both_callers: files [
        ("a.ynz", "export function op() -> Nothing { print(`op`) }"),
        ("b.ynz", "import { op } from `a`\nfunction b_caller() -> Nothing { op() }"),
        ("c.ynz", "import { op } from `a`\nfunction c_caller() -> Nothing { op() }"),
    ],
        edit ("a.ynz", "export function op() -> Nothing { sleep(10)\nprint(`op`) }"),
        probes ["b.ynz", "c.ynz"], |before, after| {
        assert_eq!(suspends_flags(&before, &["b_caller", "c_caller"]), vec![false, false]);
        assert_eq!(suspends_flags(&after, &["b_caller", "c_caller"]), vec![true, true]);
    }, "salsa re-flips only one of two callers sharing a changed import, so the other caller is compiled on the wrong path";
}
