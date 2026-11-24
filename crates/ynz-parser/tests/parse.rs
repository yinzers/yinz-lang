// The parser must accumulate errors, not bail on the first one, and must hand later
// stages an AST whose shape they can rely on. Every table below is a generating macro:
// one named test per case, so nextest runs the cases in parallel and names the failing one.
//
//   * `ast_cases!`        valid source parses with no diagnostic and has this AST shape
//   * `recovery_cases!`   invalid source yields this many diagnostics and a recovered AST
//   * `precedence_cases!` parity: each infix operator's binding power equals the spec table
//   * `snapshot_cases!`   whole programs against insta snapshots (own harness)

mod common;

use common::{AtLeast, ExpectedDiagnosticCount};
use insta::assert_debug_snapshot;
use ynz_ast::nodes::{
    BinOpKind, Expr, FunctionDecl, Item, MatchPatternKind, OptionsDecl, OwnershipModifier,
    PostfixOpKind, ReceiverKind, ShapeDecl, Stmt, StringPart, Type, UnaryOpKind,
};
use ynz_diagnostics::SourceSpan;
use ynz_parser::{parse_query, CompilerDb, ParseOutput, SourceFile, Token};

const FILE: &str = "test.ynz";

fn parse(source: &str) -> ParseOutput {
    let db = CompilerDb::default();
    let source_file = SourceFile::new(&db, FILE.to_string(), source.to_string());
    let output = parse_query(&db, source_file);
    (*output).clone()
}

fn wrap(inner: &str) -> String {
    format!("function entrypoint() -> Nothing {{ {inner} }}")
}

fn assert_no_diagnostics(output: &ParseOutput) {
    assert_eq!(
        output.diagnostics.len(),
        0,
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
}

fn first_function(output: &ParseOutput) -> &FunctionDecl {
    match &output.module.items[0] {
        Item::Function(function) => function,
        other => panic!("expected a function item, got {other:?}"),
    }
}

fn first_shape(output: &ParseOutput) -> &ShapeDecl {
    match &output.module.items[0] {
        Item::ShapeDecl(shape) => shape,
        other => panic!("expected a shape item, got {other:?}"),
    }
}

fn first_options(output: &ParseOutput) -> &OptionsDecl {
    match &output.module.items[0] {
        Item::OptionsDecl(options) => options,
        other => panic!("expected an options item, got {other:?}"),
    }
}

fn body_stmts(output: &ParseOutput) -> &[Stmt] {
    &first_function(output).body.stmts
}

/// The value of the first statement of the first function, which must be a `let`.
fn first_let_value(output: &ParseOutput) -> &Expr {
    match &body_stmts(output)[0] {
        Stmt::Let { value, .. } => value,
        other => panic!("expected a let statement, got {other:?}"),
    }
}

/// The type annotation of the first statement of the first function, a `let` with a type.
fn first_let_type(output: &ParseOutput) -> &Type {
    match &body_stmts(output)[0] {
        Stmt::Let { ty: Some(ty), .. } => ty,
        other => panic!("expected a let statement with a type annotation, got {other:?}"),
    }
}

/// The parts of the backtick string bound by the first `let` of the first function.
fn first_let_string_parts(output: &ParseOutput) -> &[StringPart] {
    match first_let_value(output) {
        Expr::InterpolatedString(parts, _) => parts,
        other => panic!("expected an interpolated string, got {other:?}"),
    }
}

fn is_literal_part(part: &StringPart, expected: &[u8]) -> bool {
    matches!(part, StringPart::Lit(bytes, _) if bytes == expected)
}

/// Asserts `type_` is `Fixed<Int, length>`, the length arriving as `Type::Length`.
fn assert_fixed_length(type_: &Type, length: u64) {
    match type_ {
        Type::Generic { name, args, .. } => {
            assert_eq!(name, "Fixed");
            assert_eq!(args.len(), 2, "{length}");
            assert!(matches!(args[0], Type::Int), "{length}");
            assert!(
                matches!(&args[1], Type::Length { value, .. } if *value == length),
                "{length}: {:?}",
                args[1]
            );
        }
        other => panic!("expected `Fixed` with a length, got {other:?}"),
    }
}

/// Binary operator at the top of the first `let` value, with its two operands.
fn first_let_binop(output: &ParseOutput) -> (&BinOpKind, &Expr, &Expr) {
    match first_let_value(output) {
        Expr::BinOp { op, lhs, rhs, .. } => (op, lhs.as_ref(), rhs.as_ref()),
        other => panic!("expected a binary operation, got {other:?}"),
    }
}

fn binop_kind(expr: &Expr) -> Option<&BinOpKind> {
    match expr {
        Expr::BinOp { op, .. } => Some(op),
        _ => None,
    }
}

/// One case per valid source: it parses with no diagnostic and `$assertions` holds.
macro_rules! ast_cases {
    ($($case:ident: $source:expr => |$output:ident| $assertions:block)+) => {$(
        // bug: parser builds the wrong AST node or shape for valid source, or reports a diagnostic on it | tier: High
        #[test]
        fn $case() {
            let $output = parse(&$source);
            assert_no_diagnostics(&$output);
            $assertions
        }
    )+};
}

/// One case per invalid source: the diagnostic count, an optional key phrase in a named
/// field (`what` or `what_instead`) of some diagnostic, and an optional extra check on the
/// recovered module.
macro_rules! recovery_cases {
    ($($case:ident: $source:expr => $count:expr
        $(, says $field:ident $phrase:expr)?
        $(, then |$output:ident| $checks:block)?;)+) => {$(
        // bug: parser misses, duplicates or cascades the diagnostic for invalid source, or loses the recovered AST | tier: High
        #[test]
        #[allow(unused_variables)] // `output` is read only by cases with a `then` check
        fn $case() {
            let output = parse(&$source);
            assert!(
                ExpectedDiagnosticCount::accepts(&$count, output.diagnostics.len()),
                "diagnostic count {} for {:?}: {:?}",
                output.diagnostics.len(),
                $source,
                output.diagnostics
            );
            $(
                assert!(
                    output.diagnostics.iter().any(|diagnostic| diagnostic.$field.contains($phrase)),
                    "no diagnostic {} contains {:?}: {:?}",
                    stringify!($field),
                    $phrase,
                    output.diagnostics
                );
            )?
            $(
                let $output = &output;
                $checks
            )?
        }
    )+};
}

/// One case per infix operator: the spec table (docs/reference/REF-operators.md) and the
/// parser's `infix_bp` agree on its precedence level. Level N maps to lbp = (13 - N) * 2.
macro_rules! precedence_cases {
    ($($case:ident: $operator:literal, level $level:literal, $token:expr;)+) => {$(
        // bug: parser binding power for the operator drifts from the precedence table in the language spec | tier: High
        #[test]
        fn $case() {
            let spec_level = spec_precedence_level($operator);
            assert_eq!(
                spec_level, $level,
                "operator `{}`: spec says level {spec_level} but the test table says level {}. \
                 Update one to match.",
                $operator, $level
            );
            let expected_lbp: u8 = (13 - $level) * 2;
            let (actual_lbp, _) = ynz_parser::parser::infix_bp(&$token)
                .unwrap_or_else(|| panic!("infix_bp returned None for `{}`", $operator));
            assert_eq!(
                actual_lbp, expected_lbp,
                "operator `{}`: expected lbp {expected_lbp} (spec level {}), got lbp {actual_lbp}",
                $operator, $level
            );
        }
    )+};
}

/// One case per realistic program: it parses clean and its whole AST matches the snapshot.
macro_rules! snapshot_cases {
    ($($case:ident: $snapshot_name:literal, $source:expr;)+) => {$(
        // bug: a parser change silently alters the AST that typeck and codegen read for a realistic program | tier: High
        #[test]
        fn $case() {
            let output = parse($source);
            assert_no_diagnostics(&output);
            assert_debug_snapshot!($snapshot_name, output.module);
        }
    )+};
}

/// Level the spec's "Operator precedence" code block gives `operator`.
fn spec_precedence_level(operator: &str) -> u8 {
    let spec_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap() // crates/
        .parent()
        .unwrap() // workspace root
        .join("docs/reference/REF-operators.md");
    let spec = std::fs::read_to_string(&spec_path)
        .unwrap_or_else(|_| panic!("Could not read {:?}", spec_path));
    let section_start = spec
        .find("## Operator precedence")
        .expect("REF-operators.md must have an '## Operator precedence' section");
    let after = &spec[section_start..];
    let block_start = after
        .find("```\n")
        .expect("Operator precedence section must have a ``` code block")
        + 4;
    let block_length = after[block_start..]
        .find("```")
        .expect("Code block must be closed");
    let block = &after[block_start..block_start + block_length];

    // Each line reads "N.  op1  op2  // comment".
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let without_comment = line.split("//").next().unwrap_or("").trim();
        let mut parts = without_comment.split_whitespace();
        let Some(level_text) = parts.next() else {
            continue;
        };
        let Ok(level) = level_text.trim_end_matches('.').parse::<u8>() else {
            continue;
        };
        if parts.any(|written| written == operator) {
            return level;
        }
    }
    panic!(
        "Operator `{operator}` is in the code's infix_bp table but missing from \
         REF-operators.md's precedence list. Add it to the spec."
    );
}

ast_cases! {
    // The empty source is the only input with no item at all.
    empty_source_produces_empty_module: "" => |output| {
        assert_eq!(output.module.items.len(), 0);
    }
    // ── options, shapes, contracts ───────────────────────────────────────────
    options_decl_lists_its_variants: "options Status { active, inactive, banned }" => |output| {
        let options = first_options(&output);
        assert_eq!(options.name, "Status");
        let variant_names: Vec<&str> = options.variants.iter().map(|variant| variant.0.as_str()).collect();
        assert_eq!(variant_names, ["active", "inactive", "banned"]);
    }
    // An empty body must still parse (typeck rejects it with a teaching error).
    options_decl_with_empty_body_still_yields_the_item: "options Empty { }" => |output| {
        assert_eq!(output.module.items.len(), 1);
        let options = first_options(&output);
        assert_eq!(options.name, "Empty");
        assert_eq!(options.variants.len(), 0);
    }
    shape_declaration_lists_fields: "shape Player { name: String\n health: Int }" => |output| {
        let shape = first_shape(&output);
        assert_eq!(shape.name, "Player");
        assert_eq!(shape.fields.len(), 2);
        assert_eq!(shape.fields[0].name, "name");
        assert_eq!(shape.fields[1].name, "health");
        assert!(!shape.is_base);
        assert!(shape.extends.is_none());
        assert!(shape.follows.is_empty());
    }
    base_shape_sets_is_base: "base shape Entity { name: String }" => |output| {
        let shape = first_shape(&output);
        assert!(shape.is_base);
        assert_eq!(shape.name, "Entity");
    }
    shape_with_extends_and_follows: "shape Warrior extends Entity follows Damageable { weapon: String }" => |output| {
        let shape = first_shape(&output);
        assert!(shape.extends.as_ref().is_some_and(|(parent, _)| parent == "Entity"));
        assert_eq!(shape.follows.len(), 1);
        assert_eq!(shape.follows[0].0, "Damageable");
    }
    hidden_field_sets_is_hidden: "shape Player { name: String\n hidden cache: Int }" => |output| {
        let shape = first_shape(&output);
        assert!(!shape.fields[0].is_hidden);
        assert!(shape.fields[1].is_hidden);
    }
    // A bare method signature inside a shape declares a contract, not a field.
    contract_signature_in_shape: "shape Greetable { greet(share self) -> String }" => |output| {
        let shape = first_shape(&output);
        assert_eq!(shape.fields.len(), 0);
        assert_eq!(shape.contract_sigs.len(), 1);
        assert_eq!(shape.contract_sigs[0].name, "greet");
        assert!(matches!(shape.contract_sigs[0].receiver, Some(ReceiverKind::Share)));
    }
    generic_shape_with_two_params: "shape Pair<A, B> { first: A second: B }" => |output| {
        let shape = first_shape(&output);
        assert_eq!(shape.generics.len(), 2);
        assert_eq!(shape.generics[0].name, "A");
        assert_eq!(shape.generics[1].name, "B");
    }
    generic_shape_with_one_param: "shape Box<T> { value: T }" => |output| {
        let shape = first_shape(&output);
        assert_eq!(shape.generics.len(), 1);
        assert_eq!(shape.generics[0].name, "T");
    }
    non_generic_shape_has_empty_generics: "shape Player { name: Int }" => |output| {
        assert!(first_shape(&output).generics.is_empty());
    }

    // ── function declarations ────────────────────────────────────────────────
    function_with_parameters: "function add(x: Int, y: Int) -> Int { return x }" => |output| {
        let function = first_function(&output);
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].name, "x");
        assert_eq!(function.params[1].name, "y");
        assert!(matches!(function.params[0].ty, Type::Int));
        assert!(matches!(function.params[1].ty, Type::Int));
    }
    // Zero parameters must not require at least one.
    function_with_no_params: "function entrypoint() -> Nothing { }" => |output| {
        assert_eq!(first_function(&output).params.len(), 0);
    }
    trailing_comma_in_params_is_accepted: "function foo(x: Int,) -> Nothing { }" => |output| {
        assert_eq!(first_function(&output).params.len(), 1);
    }
    ownership_annotation_on_param: "function foo(share x: Int) -> Nothing { }" => |output| {
        let function = first_function(&output);
        assert_eq!(function.params.len(), 1);
        assert!(matches!(function.params[0].ownership, Some(OwnershipModifier::Share)));
        assert_eq!(function.params[0].name, "x");
    }
    two_functions_are_two_items: "function add(a: Int, b: Int) -> Int { return a }\nfunction entrypoint() -> Nothing { let x = add(1, 2) }" => |output| {
        assert_eq!(output.module.items.len(), 2);
    }
    generic_function_with_one_type_param: "function identity<T>(give value: T) -> T { return value }" => |output| {
        let function = first_function(&output);
        assert_eq!(function.generics.len(), 1);
        assert_eq!(function.generics[0].name, "T");
        assert!(function.generics[0].constraints.is_empty());
    }
    generic_function_with_two_type_params: "function pair<A, B>(give first: A, give second: B) -> Nothing { }" => |output| {
        let function = first_function(&output);
        assert_eq!(function.generics.len(), 2);
        assert_eq!(function.generics[0].name, "A");
        assert_eq!(function.generics[1].name, "B");
    }
    generic_function_with_constraint: "function sort<T follows Comparable>(share items: T) -> Nothing { }" => |output| {
        let function = first_function(&output);
        assert_eq!(function.generics.len(), 1);
        assert_eq!(function.generics[0].name, "T");
        assert_eq!(function.generics[0].constraints.len(), 1);
        assert_eq!(function.generics[0].constraints[0].0, "Comparable");
    }
    // The `,` must end T's constraint list and start U's entry.
    generic_function_with_one_constrained_param_then_one_plain: "function f<T follows Comparable, U>(share a: T, give b: U) -> Nothing { }" => |output| {
        let function = first_function(&output);
        assert_eq!(function.generics.len(), 2);
        assert_eq!(function.generics[0].name, "T");
        assert_eq!(function.generics[0].constraints.len(), 1);
        assert_eq!(function.generics[1].name, "U");
        assert!(function.generics[1].constraints.is_empty());
    }
    non_generic_function_has_empty_generics: "function add(a: Int, b: Int) -> Int { return a }" => |output| {
        assert!(first_function(&output).generics.is_empty());
    }

    // ── return types: `errors` makes a function fallible ─────────────────────
    errors_return_type_over_named_type: "function readFile() -> String errors { }" => |output| {
        let function = first_function(&output);
        assert!(function.errors_capable);
        assert!(matches!(function.return_type, Type::ErrorCapable { .. }), "{:?}", function.return_type);
    }
    errors_return_type_over_nothing: "function writeFile() -> Nothing errors { }" => |output| {
        let function = first_function(&output);
        assert!(function.errors_capable);
        assert!(
            matches!(function.return_type, Type::ErrorCapable { ref inner, .. } if matches!(**inner, Type::Nothing)),
            "{:?}",
            function.return_type
        );
    }
    errors_return_type_over_maybe: "function findUser() -> Maybe<String> errors { }" => |output| {
        let function = first_function(&output);
        assert!(function.errors_capable);
        match &function.return_type {
            Type::ErrorCapable { inner, .. } => assert!(matches!(**inner, Type::Maybe { .. }), "{inner:?}"),
            other => panic!("expected ErrorCapable, got {other:?}"),
        }
    }
    // Guards against double wrapping: the inner type is exactly what was written.
    errors_return_type_keeps_inner_type_unwrapped: "function compute() -> Int errors { }" => |output| {
        match &first_function(&output).return_type {
            Type::ErrorCapable { inner, .. } => assert!(matches!(**inner, Type::Int), "{inner:?}"),
            other => panic!("expected ErrorCapable, got {other:?}"),
        }
    }
    plain_return_type_is_not_errors_capable: "function getGreeting() -> String { }" => |output| {
        let function = first_function(&output);
        assert!(!function.errors_capable);
        assert!(matches!(&function.return_type, Type::Named(name, _) if name == "String"), "{:?}", function.return_type);
    }

    // ── doc comments attach to the item that follows ─────────────────────────
    doc_comment_attaches_to_function: "/// fetches a user.\nfunction entrypoint() -> Nothing {}" => |output| {
        assert_eq!(first_function(&output).doc, Some("fetches a user.".to_string()));
    }
    doc_comment_lines_join_with_newline: "/// line1\n/// line2\nfunction entrypoint() -> Nothing {}" => |output| {
        assert_eq!(first_function(&output).doc, Some("line1\nline2".to_string()));
    }
    doc_comment_before_blank_line_does_not_attach: "/// orphan\n\nfunction entrypoint() -> Nothing {}" => |output| {
        assert_eq!(first_function(&output).doc, None);
    }
    doc_comment_attaches_to_exported_const: "/// Maximum health.\nexport const MAX_HEALTH = 100" => |output| {
        match &output.module.items[0] {
            Item::ConstDecl(constant) => assert_eq!(constant.doc, Some("Maximum health.".to_string())),
            other => panic!("expected a const item, got {other:?}"),
        }
    }
    doc_comment_before_blank_line_does_not_attach_to_const: "/// orphan\n\nexport const MAX = 10" => |output| {
        match &output.module.items[0] {
            Item::ConstDecl(constant) => assert_eq!(constant.doc, None),
            other => panic!("expected a const item, got {other:?}"),
        }
    }

    // ── statements ───────────────────────────────────────────────────────────
    let_with_int_annotation: wrap("let x: Int = 1") => |output| {
        assert!(matches!(first_let_type(&output), Type::Int));
    }
    // A type mismatch is typeck's error, not a parse error.
    let_with_mismatched_annotation_still_parses: wrap("let x: Int = 1.5") => |output| {
        assert!(matches!(first_let_type(&output), Type::Int));
        assert!(matches!(first_let_value(&output), Expr::NumberLit(text, _) if text == "1.5"));
    }
    assignment_is_stmt_assign: wrap("let x = 0\nx = 5") => |output| {
        assert!(matches!(&body_stmts(&output)[1], Stmt::Assign { target, .. } if target == "x"));
    }
    field_assignment_is_stmt_field_assign: wrap("player.health = 50") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::FieldAssign { target, value: Expr::IntLit(50, _), .. } => {
                assert!(matches!(**target, Expr::FieldAccess { ref field, .. } if field == "health"));
            }
            other => panic!("expected FieldAssign, got {other:?}"),
        }
    }
    index_assign_with_int_key: wrap("arr[0] = 5") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::IndexAssign { index, value: Expr::IntLit(5, _), .. } => {
                assert!(matches!(**index, Expr::IntLit(0, _)));
            }
            other => panic!("expected IndexAssign, got {other:?}"),
        }
    }
    index_assign_with_string_key: wrap("scores[`alice`] = 90") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::IndexAssign { index, value: Expr::IntLit(90, _), .. } => {
                assert!(matches!(**index, Expr::InterpolatedString(_, _)));
            }
            other => panic!("expected IndexAssign, got {other:?}"),
        }
    }
    simple_if_is_stmt_if: wrap("if (x > 0) { print(x) }") => |output| {
        assert!(matches!(body_stmts(&output)[0], Stmt::If { .. }));
    }
    while_is_stmt_while: wrap("while (x > 0) { x = x - 1 }") => |output| {
        assert!(matches!(body_stmts(&output)[0], Stmt::While { .. }));
    }
    for_is_stmt_for_with_loop_variable: wrap("for (i in range(0, 5)) { print(i) }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::For { var, .. } => assert_eq!(var, "i"),
            other => panic!("expected For, got {other:?}"),
        }
    }
    return_with_value: "function foo() -> Int { return 42 }" => |output| {
        assert!(matches!(body_stmts(&output)[0], Stmt::Return { value: Some(_), .. }));
    }
    return_without_value: wrap("return") => |output| {
        assert!(matches!(body_stmts(&output)[0], Stmt::Return { value: None, .. }));
    }
    nested_if_inside_for_keeps_block_ends: wrap("for (i in range(0, 5)) {\n  if (i > 2) {\n    print(i)\n  }\n}") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::For { body, .. } => assert!(matches!(body.stmts[0], Stmt::If { .. })),
            other => panic!("expected For, got {other:?}"),
        }
    }

    // ── multi-case `if` becomes a match ──────────────────────────────────────
    multi_case_if_with_value_arms: wrap("if (x) { 1 => print(1) 2 => print(2) }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::Match { arms, else_arm: None, .. } => assert_eq!(arms.len(), 2),
            other => panic!("expected Match, got {other:?}"),
        }
    }
    multi_case_if_with_else_arm: wrap("if (x) { 1 => print(1) else => print(0) }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::Match { arms, else_arm: Some(_), .. } => assert_eq!(arms.len(), 1),
            other => panic!("expected Match with else_arm, got {other:?}"),
        }
    }
    multi_case_if_with_only_else_arm: wrap("if (x) { else => print(0) }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::Match { arms, else_arm: Some(_), .. } => assert_eq!(arms.len(), 0),
            other => panic!("expected Match with else_arm only, got {other:?}"),
        }
    }
    // Typeck and codegen key on `MatchPatternKind::Is` for union narrowing.
    multi_case_if_is_arm_is_an_is_pattern: wrap("if (value) { is Circle => print(1) }") => |output| {
        let found = body_stmts(&output).iter().any(|stmt| match stmt {
            Stmt::Match { arms, .. } => arms
                .iter()
                .any(|arm| matches!(&arm.pattern.kind, MatchPatternKind::Is(_))),
            _ => false,
        });
        assert!(found, "expected an Is arm in {:?}", output.module);
    }

    // ── expressions: precedence, associativity, call shapes ──────────────────
    mul_binds_tighter_than_add: wrap("let x = 1 + 2 * 3") => |output| {
        let (op, _, rhs) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::Add);
        assert_eq!(binop_kind(rhs), Some(&BinOpKind::Mul));
    }
    comparison_binds_tighter_than_and: wrap("let x = 1 < 2 && 3 > 4") => |output| {
        let (op, lhs, rhs) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::And);
        assert_eq!(binop_kind(lhs), Some(&BinOpKind::Lt));
        assert_eq!(binop_kind(rhs), Some(&BinOpKind::Gt));
    }
    subtraction_is_left_associative: wrap("let x = 1 - 2 - 3") => |output| {
        let (op, lhs, rhs) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::Sub);
        assert_eq!(binop_kind(lhs), Some(&BinOpKind::Sub));
        assert!(matches!(rhs, Expr::IntLit(3, _)));
    }
    or_binds_looser_than_and: wrap("let x = a || b && c") => |output| {
        let (op, _, rhs) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::Or);
        assert_eq!(binop_kind(rhs), Some(&BinOpKind::And));
    }
    parentheses_override_precedence: wrap("let x = (1 + 2) * 3") => |output| {
        let (op, lhs, _) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::Mul);
        assert_eq!(binop_kind(lhs), Some(&BinOpKind::Add));
    }
    // Typeck rejects `bool < int`; the parse result must still be the deterministic `(1 < 2) < 3`.
    chained_comparison_is_left_associative: wrap("let x = 1 < 2 < 3") => |output| {
        let (op, lhs, rhs) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::Lt);
        assert_eq!(binop_kind(lhs), Some(&BinOpKind::Lt));
        assert!(matches!(rhs, Expr::IntLit(3, _)));
    }
    unary_minus_is_unary_neg: wrap("let x = -5") => |output| {
        match first_let_value(&output) {
            Expr::UnaryOp { op, operand, .. } => {
                assert_eq!(*op, UnaryOpKind::Neg);
                assert!(matches!(operand.as_ref(), Expr::IntLit(5, _)));
            }
            other => panic!("expected UnaryOp(Neg), got {other:?}"),
        }
    }
    method_call_is_method_call_node: wrap("let s = count.toString()") => |output| {
        assert!(matches!(first_let_value(&output), Expr::MethodCall { method, .. } if method == "toString"));
    }
    // Arity is typeck's job, so the parser accepts a call with any number of arguments.
    call_with_three_arguments: wrap("print(1, 2, 3)") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::Expr(Expr::Call(call)) => assert_eq!(call.args.len(), 3),
            other => panic!("expected Expr(Call), got {other:?}"),
        }
    }
    // A dot without parens is a read: FieldAccess, never MethodCall.
    field_read_is_field_access: wrap("let x = player.health") => |output| {
        assert!(matches!(first_let_value(&output), Expr::FieldAccess { field, .. } if field == "health"));
    }
    copy_postfix_op: wrap("let b = a.copy()") => |output| {
        assert!(matches!(first_let_value(&output), Expr::PostfixOp { op: PostfixOpKind::Copy, .. }));
    }
    freeze_postfix_op: wrap("cfg.freeze()") => |output| {
        assert!(matches!(
            &body_stmts(&output)[0],
            Stmt::Expr(Expr::PostfixOp { op: PostfixOpKind::Freeze, .. })
        ));
    }
    self_value_in_function_body: "function greet(share self: String) -> Nothing { print(self) }" => |output| {
        match &body_stmts(&output)[0] {
            Stmt::Expr(Expr::Call(call)) => assert!(matches!(call.args[0], Expr::SelfValue { .. })),
            other => panic!("expected Expr(Call) with a SelfValue argument, got {other:?}"),
        }
    }
    anonymous_struct_literal: wrap("let p: String = { name: `Patrick` }") => |output| {
        match first_let_value(&output) {
            Expr::StructLit { fields, .. } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "name");
            }
            other => panic!("expected StructLit, got {other:?}"),
        }
    }
    none_literal_is_none_lit: wrap("let x = none") => |output| {
        assert!(matches!(first_let_value(&output), Expr::NoneLit { .. }));
    }
    // `none` must work in any expression position, not only as a binding value.
    none_inside_a_comparison: wrap("if (x == none) { }") => |_output| {}

    // ── indexing ─────────────────────────────────────────────────────────────
    index_access_with_int_key: wrap("let x = arr[0]") => |output| {
        match first_let_value(&output) {
            Expr::IndexAccess { index, .. } => assert!(matches!(**index, Expr::IntLit(0, _))),
            other => panic!("expected IndexAccess, got {other:?}"),
        }
    }
    index_access_with_string_key: wrap("let x = m[`alice`]") => |output| {
        match first_let_value(&output) {
            Expr::IndexAccess { index, .. } => assert!(matches!(**index, Expr::InterpolatedString(_, _))),
            other => panic!("expected IndexAccess, got {other:?}"),
        }
    }
    chained_index_access: wrap("let x = m[`alice`][0]") => |output| {
        match first_let_value(&output) {
            Expr::IndexAccess { receiver, .. } => assert!(matches!(**receiver, Expr::IndexAccess { .. })),
            other => panic!("expected IndexAccess, got {other:?}"),
        }
    }
    index_access_as_a_bare_statement: wrap("arr[0]") => |output| {
        assert!(matches!(body_stmts(&output)[0], Stmt::Expr(Expr::IndexAccess { .. })));
    }

    // ── generics in type position and at call sites ──────────────────────────
    array_type_annotation_is_generic: wrap("let arr: Array<Int> = arr") => |output| {
        match first_let_type(&output) {
            Type::Generic { name, args, .. } => {
                assert_eq!(name, "Array");
                assert_eq!(args.len(), 1);
                assert!(matches!(args[0], Type::Int));
            }
            other => panic!("expected a Generic type, got {other:?}"),
        }
    }
    // `Maybe` has its own variant (not Generic) so typeck can apply flow-sensitive `.value` rules.
    maybe_type_annotation_is_maybe: wrap("let x: Maybe<Int> = none") => |output| {
        match first_let_type(&output) {
            Type::Maybe { inner, .. } => assert!(matches!(**inner, Type::Int)),
            other => panic!("expected Maybe<Int>, got {other:?}"),
        }
        assert!(matches!(first_let_value(&output), Expr::NoneLit { .. }));
    }
    maybe_of_named_type: wrap("let x: Maybe<Player> = none") => |output| {
        match first_let_type(&output) {
            Type::Maybe { inner, .. } => assert!(matches!(**inner, Type::Named(ref name, _) if name == "Player")),
            other => panic!("expected Maybe<Player>, got {other:?}"),
        }
    }
    // Nested `<>` must not trip the generic depth cap.
    nested_generic_type: wrap("let x: Pair<Pair<Int, Int>, Int> = x") => |output| {
        match first_let_type(&output) {
            Type::Generic { name, args, .. } => {
                assert_eq!(name, "Pair");
                assert_eq!(args.len(), 2);
                assert!(matches!(&args[0], Type::Generic { name, .. } if name == "Pair"));
                assert!(matches!(args[1], Type::Int));
            }
            other => panic!("expected a nested Generic type, got {other:?}"),
        }
    }
    union_type_with_two_variants: wrap("let x: Circle | Square = { radius: 5.0 }") => |output| {
        match first_let_type(&output) {
            Type::Union { variants, .. } => assert_eq!(variants.len(), 2),
            other => panic!("expected a Union type, got {other:?}"),
        }
    }
    // Codegen relies on one flat variant list for tag assignment, not nested pairs.
    union_type_with_three_variants_is_flat: wrap("let x: Circle | Square | Triangle = { radius: 5.0 }") => |output| {
        match first_let_type(&output) {
            Type::Union { variants, .. } => assert_eq!(variants.len(), 3),
            other => panic!("expected a Union type, got {other:?}"),
        }
    }
    dynamic_type_annotation: wrap("let d: dynamic Foo = p") => |output| {
        assert!(matches!(first_let_type(&output), Type::Dynamic { contract, .. } if contract == "Foo"));
    }
    number_without_precision_defaults_to_34: wrap("let x: Number = 3.14") => |output| {
        assert!(matches!(first_let_type(&output), Type::Number { precision: 34 }));
    }
    number_with_explicit_precision_34: wrap("let x: Number<34> = 3.14") => |output| {
        assert!(matches!(first_let_type(&output), Type::Number { precision: 34 }));
    }
    // The precision deferral is gone: a non-default precision parses with no diagnostic.
    number_with_precision_128: wrap("let x: Number<128> = 0") => |_output| {}
    // `fixed<T, N>`: each written length reaches typeck as `Type::Length`, the u32 extremes included.
    fixed_length_zero: wrap("let x: Fixed<Int, 0> = x") => |output| {
        assert_fixed_length(first_let_type(&output), 0);
    }
    fixed_length_one: wrap("let x: Fixed<Int, 1> = x") => |output| {
        assert_fixed_length(first_let_type(&output), 1);
    }
    fixed_length_four: wrap("let x: Fixed<Int, 4> = x") => |output| {
        assert_fixed_length(first_let_type(&output), 4);
    }
    fixed_length_largest_u32: wrap("let x: Fixed<Int, 4294967295> = x") => |output| {
        assert_fixed_length(first_let_type(&output), u64::from(u32::MAX));
    }
    generic_call_with_one_type_arg: wrap("let x = identity<Int>(5)") => |output| {
        match first_let_value(&output) {
            Expr::Call(call) => {
                let type_args = call.type_args.as_ref().expect("type_args must be Some");
                assert_eq!(type_args.len(), 1);
                assert!(matches!(type_args[0], Type::Int));
            }
            other => panic!("expected Call with type_args, got {other:?}"),
        }
    }
    generic_call_with_two_type_args: wrap("let x = pair<Int, String>(1, `a`)") => |output| {
        match first_let_value(&output) {
            Expr::Call(call) => {
                let type_args = call.type_args.as_ref().expect("type_args must be Some");
                assert_eq!(type_args.len(), 2);
                assert!(matches!(type_args[0], Type::Int));
                assert!(matches!(type_args[1], Type::Named(ref name, _) if name == "String"));
            }
            other => panic!("expected Call with two type_args, got {other:?}"),
        }
    }
    // `None` means no explicit type arguments; `Some([])` would mean an empty explicit list.
    plain_call_has_no_type_args: wrap("let x = add(1, 2)") => |output| {
        match first_let_value(&output) {
            Expr::Call(call) => assert!(call.type_args.is_none()),
            other => panic!("expected Call, got {other:?}"),
        }
    }

    // ── `<` is a comparison unless a generic call follows ────────────────────
    less_than_stays_a_comparison: wrap("let x = a < b") => |output| {
        assert_eq!(binop_kind(first_let_value(&output)), Some(&BinOpKind::Lt));
    }
    comparison_chain_is_not_a_generic_call: wrap("let x = a < b > c") => |output| {
        let (op, lhs, _) = first_let_binop(&output);
        assert_eq!(op, &BinOpKind::Gt);
        assert_eq!(binop_kind(lhs), Some(&BinOpKind::Lt));
    }
    comparison_inside_index_is_not_a_generic: wrap("let x = arr[a < b]") => |output| {
        match first_let_value(&output) {
            Expr::IndexAccess { index, .. } => assert_eq!(binop_kind(index), Some(&BinOpKind::Lt)),
            other => panic!("expected IndexAccess with a Lt index, got {other:?}"),
        }
    }
    // `foo < Int > 5` has a type-looking middle but no `(` after `>`: a comparison, not a generic call.
    type_like_comparison_without_call_is_a_comparison: wrap("let x = foo < Int > 5") => |output| {
        assert_eq!(binop_kind(first_let_value(&output)), Some(&BinOpKind::Gt));
    }

    // ── backtick strings ─────────────────────────────────────────────────────
    backtick_plain_literal_is_one_literal_part: wrap("let s = `hello`") => |output| {
        let parts = first_let_string_parts(&output);
        assert_eq!(parts.len(), 1);
        assert!(is_literal_part(&parts[0], b"hello"));
    }
    // The lexer always closes an interpolation with a literal segment, so `${name}` is Lit + Expr + Lit.
    backtick_single_interpolation_is_three_parts: wrap("let s = `hello ${name}`") => |output| {
        let parts = first_let_string_parts(&output);
        assert_eq!(parts.len(), 3);
        assert!(is_literal_part(&parts[0], b"hello "));
        assert!(matches!(&parts[1], StringPart::Expr(_, _)));
        assert!(is_literal_part(&parts[2], b""));
    }
    backtick_two_interpolations_are_five_parts: wrap("let s = `${a} + ${b}`") => |output| {
        let parts = first_let_string_parts(&output);
        assert_eq!(parts.len(), 5);
        assert!(is_literal_part(&parts[0], b""));
        assert!(matches!(&parts[1], StringPart::Expr(_, _)));
        assert!(is_literal_part(&parts[2], b" + "));
        assert!(matches!(&parts[3], StringPart::Expr(_, _)));
        assert!(is_literal_part(&parts[4], b""));
    }
    // The interpolated slot holds the full expression AST (here an identifier), not a string.
    backtick_interpolated_slot_holds_an_expression: wrap("let s = `value=${x}`") => |output| {
        let parts = first_let_string_parts(&output);
        assert_eq!(parts.len(), 3);
        match &parts[1] {
            StringPart::Expr(inner, _) => assert!(matches!(**inner, Expr::Ident(ref name, _) if name == "x")),
            other => panic!("expected an Expr part, got {other:?}"),
        }
    }
    // Downstream stages assume at least one segment always exists.
    backtick_empty_string_is_one_empty_literal_part: wrap("let s = ``") => |output| {
        let parts = first_let_string_parts(&output);
        assert_eq!(parts.len(), 1);
        assert!(is_literal_part(&parts[0], b""));
    }

    // ── `for ((key, value) in map)` desugars to a synthetic entry ────────────
    for_destructure_loops_over_a_synthetic_entry: wrap("for ((k, v) in myMap) { }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::For { var, body, .. } => {
                assert_eq!(var, "__entry");
                assert!(body.stmts.len() >= 2, "body must hold the 2 injected let bindings");
                assert!(matches!(&body.stmts[0], Stmt::Let { name, .. } if name == "k"));
                assert!(matches!(&body.stmts[1], Stmt::Let { name, .. } if name == "v"));
            }
            other => panic!("expected For, got {other:?}"),
        }
    }
    // User statements come after the two injected lets, not instead of them.
    for_destructure_keeps_the_user_body: wrap("for ((k, v) in m) { print(k) }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::For { body, .. } => {
                assert_eq!(body.stmts.len(), 3, "2 injected lets + 1 user stmt");
                assert!(matches!(&body.stmts[2], Stmt::Expr(_)));
            }
            other => panic!("expected For, got {other:?}"),
        }
    }
    // The injected `let k` reads `__entry.key` as a FieldAccess so typeck can resolve it on MapEntry.
    for_destructure_key_binding_reads_the_key_field: wrap("for ((k, v) in m) { }") => |output| {
        match &body_stmts(&output)[0] {
            Stmt::For { body, .. } => match &body.stmts[0] {
                Stmt::Let { value: Expr::FieldAccess { field, .. }, .. } => assert_eq!(field, "key"),
                other => panic!("expected Let with FieldAccess, got {other:?}"),
            },
            other => panic!("expected For, got {other:?}"),
        }
    }
    // Card #399: typeck keys `expr_types` by span, so the `.key` access, the `.value` access, the
    // `__entry` receiver and the iterated map need four different spans (they have four types).
    for_destructure_synthetic_expressions_have_distinct_spans: wrap("for ((k, v) in m) { }") => |output| {
        let Stmt::For { iter, body, .. } = &body_stmts(&output)[0] else {
            panic!("expected For, got {:?}", body_stmts(&output)[0]);
        };
        let access_parts = |stmt: &Stmt| match stmt {
            Stmt::Let { value: Expr::FieldAccess { receiver, span, .. }, .. } => {
                (receiver.span().clone(), span.clone())
            }
            other => panic!("expected `let` of a field access, got {other:?}"),
        };
        let (key_receiver, key_access) = access_parts(&body.stmts[0]);
        let (value_receiver, value_access) = access_parts(&body.stmts[1]);
        let start_and_end = |span: &SourceSpan| (span.start, span.end);
        let spans = [
            start_and_end(iter.span()),
            start_and_end(&key_access),
            start_and_end(&value_access),
            start_and_end(&key_receiver),
        ];
        for (position, first) in spans.iter().enumerate() {
            for second in &spans[position + 1..] {
                assert_ne!(first, second, "synthetic span collision: {spans:?}");
            }
        }
        assert_eq!(
            start_and_end(&key_receiver),
            start_and_end(&value_receiver),
            "both receivers are the same MapEntry-typed `__entry`, so they may share one span"
        );
    }
    // The `((` look-ahead must not fire for a plain `(x`.
    for_single_binding_keeps_the_user_name: wrap("for (entry in myMap) { }") => |output| {
        assert!(matches!(&body_stmts(&output)[0], Stmt::For { var, .. } if var == "entry"));
    }
}

recovery_cases! {
    // ── missing or malformed pieces: one diagnostic, the module still has its item ──
    missing_rhs_recovers: "function entrypoint() -> Nothing { let x = 1 + }" => 1,
        then |output| { assert_eq!(output.module.items.len(), 1); };
    missing_let_identifier_recovers: "function entrypoint() -> Nothing { let : Int = 5 }" => AtLeast(1),
        then |output| { assert_eq!(output.module.items.len(), 1); };
    // `let x = }` must not eat the `}` that closes the function body.
    unexpected_rbrace_in_expression_recovers: "function entrypoint() -> Nothing { let x = }" => 1,
        then |output| { assert_eq!(output.module.items.len(), 1); };
    missing_arrow_recovers: "function entrypoint() nothing { }" => 2,
        says what "Missing return type";
    missing_closing_brace_is_reported: "function entrypoint() -> Nothing { print(\"hi\")" => 2,
        says what "Missing closing";
    trailing_garbage_after_function_keeps_the_function: "function entrypoint() -> Nothing { } extra }" => AtLeast(1),
        then |output| { assert_eq!(output.module.items.len(), 1); };
    if_without_parens_is_reported: "function entrypoint() -> Nothing { if x > 0 { print(x) } }" => AtLeast(1);
    duplicate_param_name_is_reported: "function foo(a: Int, a: Int) -> Int { return a }" => AtLeast(1),
        says what "Duplicate";

    // ── teaching diagnostics for syntax from other languages ─────────────────
    prefix_form_struct_literal_is_one_teaching_diagnostic:
        "function entrypoint() -> Nothing { let x = Player { name: `Patrick` } }" => 1,
        says what_instead "annotation";
    method_body_inside_shape_is_one_teaching_diagnostic:
        "shape Player { function greet() -> String { return `hi` } }" => 1,
        says what_instead "outside";
    // `number[N]` is the pre-generics form; it must redirect to `Number<N>`, not parse silently.
    number_bracket_syntax_redirects_to_angle_brackets:
        "function entrypoint() -> Nothing { let x: Number[128] = 0 }" => 1,
        says what "angle brackets";

    // ── generics and indexing ────────────────────────────────────────────────
    unclosed_generic_param_list_keeps_the_next_function: "function foo< { } function bar() -> Nothing { }" => AtLeast(1),
        then |output| {
            let function_names: Vec<&str> = output
                .module
                .items
                .iter()
                .filter_map(|item| match item {
                    Item::Function(function) => Some(function.name.as_str()),
                    _ => None,
                })
                .collect();
            assert!(function_names.contains(&"bar"), "bar() must parse despite foo<'s error");
        };
    missing_type_arg_close_bracket_is_reported: wrap("let x: Array<Int = 5") => AtLeast(1);
    index_missing_close_bracket_is_reported: wrap("let x = arr[0") => AtLeast(1);
    bare_maybe_without_type_arg_is_reported: wrap("let x: Maybe = none") => AtLeast(1), says what "Maybe";
    generic_type_nested_17_deep_is_reported:
        format!(
            "function entrypoint() -> Nothing {{ let x: {} = x }}",
            "A<".repeat(17) + "Int" + &">".repeat(17)
        ) => AtLeast(1);
    // A fixed length not closed by `,` or `>` is one diagnostic, not a silent parse or a cascade.
    fixed_length_followed_by_a_number_is_refused_once: wrap("let x: Fixed<Int, 3 4> = x") => AtLeast(1),
        then |output| {
            let refused = output.diagnostics.iter()
                .filter(|diagnostic| diagnostic.what.contains("after the length in a generic type argument list"))
                .count();
            assert_eq!(refused, 1, "{:#?}", output.diagnostics);
        };
    fixed_length_followed_by_a_type_is_refused_once: wrap("let x: Fixed<Int, 3 Int> = x") => AtLeast(1),
        then |output| {
            let refused = output.diagnostics.iter()
                .filter(|diagnostic| diagnostic.what.contains("after the length in a generic type argument list"))
                .count();
            assert_eq!(refused, 1, "{:#?}", output.diagnostics);
        };

    // ── `errors` is only a return-type modifier ──────────────────────────────
    errors_in_expression_position_is_rejected: wrap("let x = errors") => AtLeast(1);
    errors_after_a_let_annotation_is_rejected: wrap("let x: String errors = foo") => AtLeast(1);
    errors_as_a_bare_statement_is_rejected: wrap("errors") => AtLeast(1);
}

precedence_cases! {
    precedence_star: "*", level 3, Token::Star;
    precedence_slash: "/", level 3, Token::Slash;
    precedence_percent: "%", level 3, Token::Percent;
    precedence_plus: "+", level 4, Token::Plus;
    precedence_minus: "-", level 4, Token::Minus;
    precedence_shift_left: "<<", level 5, Token::LtLt;
    precedence_shift_right: ">>", level 5, Token::GtGt;
    precedence_less_than: "<", level 6, Token::Lt;
    precedence_greater_than: ">", level 6, Token::Gt;
    precedence_less_or_equal: "<=", level 6, Token::LtEq;
    precedence_greater_or_equal: ">=", level 6, Token::GtEq;
    precedence_equal: "==", level 7, Token::EqEq;
    precedence_not_equal: "!=", level 7, Token::NotEq;
    precedence_bit_and: "&", level 8, Token::Amp;
    precedence_bit_xor: "^", level 9, Token::Caret;
    precedence_bit_or: "|", level 10, Token::Pipe;
    precedence_logical_and: "&&", level 11, Token::AmpAmp;
    precedence_logical_or: "||", level 12, Token::PipePipe;
}

snapshot_cases! {
    operators_and_literals_program_snapshot: "m2_ast",
        r#"function entrypoint() -> Nothing {
  let price = 0.1 + 0.2
  let count: Int = 42
  let active = true
  print(price)
  print(count * count - 1)
  print(active && (count > 0))
}"#;
    control_flow_program_snapshot: "m3_ast",
        r#"function fib(n: Int) -> Int {
  if (n < 2) {
    return n
  }
  return fib(n - 1) + fib(n - 2)
}

function entrypoint() -> Nothing {
  let result = fib(10)
  print(result)
}"#;
    shape_program_snapshot: "m4_ast",
        "shape Player {\n  name: String\n  health: Int\n  hidden cache: Int\n}\n\nfunction greet(share self: Player) -> String {\n  return self.name\n}\n\nfunction entrypoint() -> Nothing {\n  let p: Player = { name: `Patrick`, health: 100 }\n  p.health = 90\n  let h = p.health\n  let b = p.copy()\n}";
    generics_index_and_maybe_program_snapshot: "m5_ast_snapshot",
        r#"
shape Pair<A, B> {
  first: A
  second: B
}

function identity<T>(give value: T) -> T {
  return value
}

function sort<T follows Comparable>(share items: Array<T>) -> Nothing {
}

function entrypoint() -> Nothing {
  let x: Maybe<Int> = none
  let arr: Array<Int> = arr
  let first = arr[0]
  arr[0] = 99
  let y = identity<Int>(5)
  let a = arr[0]
  let cmp = a < 10
}
"#;
}

// bug: parser overflows the stack, or loses the function item, on deeply nested parentheses | tier: High
#[test]
fn expression_nesting_past_256_levels_is_refused_without_overflow() {
    // ~50k nested open parens deterministically crashed the compiler with a stack overflow
    // (SIGABRT). The 256-level cap must fire a diagnostic and return an Error node. The test
    // runs on a 16 MB thread stack only so debug-mode parse frames can REACH the cap; the
    // production cap (256) is what is under test.
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let depth = 300usize;
            let source = format!(
                "function entrypoint() -> Nothing {{ let x = {}1{} }}",
                "(".repeat(depth),
                ")".repeat(depth)
            );
            parse(&source)
        })
        .expect("thread spawn failed")
        .join()
        .expect("thread panicked");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.what.contains("256")),
        "A diagnostic mentioning the 256 limit must be present, got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.module.items.len(),
        1,
        "The function item must still appear in the AST"
    );
}
