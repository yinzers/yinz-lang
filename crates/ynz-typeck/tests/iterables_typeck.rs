// Iterable protocol typeck (M7 P3c): which `for` iterables, first-class ranges and built-in shape
// fields (`Frame`, `SourceLoc`) the checker accepts or rejects. One table of named cases: a case
// is either clean (no diagnostic) or exactly N errors.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::check_query;

fn check(source: &str) -> ynz_typeck::CheckOutput {
    let db = CompilerDb::default();
    let sf = SourceFile::new(&db, "test.ynz".to_string(), source.to_string());
    (*check_query(&db, sf)).clone()
}

fn assert_clean(source: &str) {
    let out = check(source);
    assert_eq!(
        out.diagnostics.len(),
        0,
        "Expected no diagnostics, got: {:#?}",
        out.diagnostics
    );
}

fn assert_error_count(source: &str, expected: usize) {
    let out = check(source);
    let error_count = out
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, ynz_diagnostics::Severity::Error))
        .count();
    assert_eq!(
        error_count, expected,
        "Expected {expected} error(s), got: {:#?}",
        out.diagnostics
    );
}

macro_rules! iterable_cases {
    ($($case:ident: $source:expr => $expected:tt;)+) => {
        $(
            // bug: typeck refuses a valid iterable/Range/Frame/SourceLoc program, or accepts an invalid one (a shape with no usable next(), an unknown field, a three-argument range) that codegen cannot lower | tier: High
            #[test]
            fn $case() {
                iterable_case!($source, $expected);
            }
        )+
    };
}

macro_rules! iterable_case {
    ($source:expr, clean) => {
        assert_clean($source)
    };
    ($source:expr, $errors:literal) => {
        assert_error_count($source, $errors)
    };
}

iterable_cases! {
    a_range_stored_in_a_let_is_clean:
        r#"function entrypoint() -> Nothing {
    let r = range(0, 10)
}"#
        => clean;
    a_range_passed_to_a_function_is_clean:
        r#"function useRange(give r: Range) -> Nothing {
}

function entrypoint() -> Nothing {
    useRange(range(0, 5))
}"#
        => clean;
    a_range_with_three_arguments_is_one_error:
        r#"function entrypoint() -> Nothing {
    for (i in range(0, 5, 1)) { print(i) }
}"#
        => 1;
    a_for_loop_over_a_string_is_clean:
        r#"function entrypoint() -> Nothing {
    let word = `café`
    for (c in word) {
        print(c)
    }
}"#
        => clean;
    a_for_loop_over_an_array_is_clean:
        r#"function entrypoint() -> Nothing {
    let nums: Array<Int> = [1, 2, 3]
    for (n in nums) {
        print(n)
    }
}"#
        => clean;
    a_for_loop_over_a_fixed_list_is_clean:
        r#"function entrypoint() -> Nothing {
    let nums: Fixed<Int> = [1, 2, 3]
    for (n in nums) {
        print(n)
    }
}"#
        => clean;
    a_for_loop_over_a_map_is_clean:
        r#"function entrypoint() -> Nothing {
    let m: Map<String, Int> = { `a`: 1, `b`: 2 }
    for (entry in m) {
        print(entry.key)
        print(entry.value)
    }
}"#
        => clean;
    a_for_loop_over_a_range_is_clean:
        r#"function entrypoint() -> Nothing {
    for (i in range(0, 10)) {
        print(i)
    }
}"#
        => clean;
    a_for_loop_over_a_shape_with_next_is_clean:
        r#"shape Counter {
    current: Int
    limit: Int
}

function next(lend self: Counter) -> Maybe<Int> {
    return none
}

function entrypoint() -> Nothing {
    let c: Counter = { current: 0, limit: 3 }
    for (n in c) {
        print(n)
    }
}"#
        => clean;
    a_for_loop_over_a_shape_without_next_is_one_error:
        r#"shape Player {
    name: String
}

function entrypoint() -> Nothing {
    let p: Player = { name: `Patrick` }
    for (x in p) {
        print(x)
    }
}"#
        => 1;
    a_for_loop_over_a_shape_whose_next_does_not_return_a_maybe_is_one_error:
        r#"shape BadIter {
    pos: Int
}

function next(lend self: BadIter) -> Int {
    return self.pos
}

function entrypoint() -> Nothing {
    let b: BadIter = { pos: 0 }
    for (x in b) {
        print(x)
    }
}"#
        => 1;
    a_frame_file_field_is_a_string:
        r#"function getFile(give frame: Frame) -> String {
    return frame.file
}

function entrypoint() -> Nothing {
}"#
        => clean;
    a_frame_line_field_is_a_maybe_int:
        r#"function getLine(give frame: Frame) -> Maybe<Int> {
    return frame.line
}

function entrypoint() -> Nothing {
}"#
        => clean;
    a_source_loc_file_field_is_a_string:
        r#"function getSrcFile(give loc: SourceLoc) -> String {
    return loc.file
}

function entrypoint() -> Nothing {
}"#
        => clean;
    a_source_loc_line_field_is_a_maybe_int:
        r#"function getSrcLine(give loc: SourceLoc) -> Maybe<Int> {
    return loc.line
}

function entrypoint() -> Nothing {
}"#
        => clean;
    an_unknown_frame_field_is_one_error:
        r#"function getBad(give frame: Frame) -> String {
    return frame.notAField
}

function entrypoint() -> Nothing {
}"#
        => 1;
    an_unknown_source_loc_field_is_one_error:
        r#"function getBad(give loc: SourceLoc) -> String {
    return loc.notAField
}

function entrypoint() -> Nothing {
}"#
        => 1;
}
