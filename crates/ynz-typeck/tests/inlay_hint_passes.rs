// WHY: tests for the salsa-tracked inlay-hint passes other than `let_to_const_promotion_hints`
// (that unit lives in `inlay_hint_maybe_mutated.rs`): whether `array_to_fixed_promotion_hints`,
// `variable_type_hints` and `copy_point_hints` fire for the right bindings and call arguments.
// Hint text wording and hint position are not asserted (Low tier); a position is used only to
// say WHICH expression the hint belongs to.

use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::{
    array_to_fixed_promotion_hints, copy_point_hints, variable_type_hints, PromotionKind,
};

fn single_file(src: &str) -> (CompilerDb, SourceFile) {
    let path = format!("/tmp/ynz_ih_pass_{}.ynz", src.len());
    let mut db = CompilerDb::default();
    let sf = SourceFile::new(&db, path, src.to_string());
    db.register_source(sf);
    (db, sf)
}

fn array_to_fixed_hint_count(src: &str) -> usize {
    let (db, sf) = single_file(src);
    array_to_fixed_promotion_hints(&db, sf)
        .iter()
        .filter(|h| matches!(h.kind, PromotionKind::ArrayToFixed))
        .count()
}

fn assert_array_to_fixed_fires(src: &str) {
    assert!(
        array_to_fixed_hint_count(src) >= 1,
        "expected an ArrayToFixed hint for:\n{src}"
    );
}

fn assert_array_to_fixed_suppressed(src: &str) {
    assert_eq!(
        array_to_fixed_hint_count(src),
        0,
        "expected no ArrayToFixed hint for:\n{src}"
    );
}

macro_rules! array_to_fixed_cases {
    ($($case:ident: $check:ident($src:expr), $bug:literal;)+) => {
        $(
            // bug: the array→fixed hint fires on an array that is grown (or is suppressed on one that never is), so the IDE suggests a fixed length the program then violates | tier: Medium
            #[doc = $bug]
            #[test]
            fn $case() {
                $check($src);
            }
        )+
    };
}

array_to_fixed_cases! {
    never_grown_annotated_array_fires:
        assert_array_to_fixed_fires("function entrypoint() -> Nothing {\n  let nums: Array<Int> = [1, 2, 3]\n}\n"),
        "an annotated array that is never grown gets no array→fixed hint";

    array_grown_in_else_arm_is_suppressed:
        assert_array_to_fixed_suppressed(
            r#"
function entrypoint() -> Nothing {
  let nums: Array<Int> = [1, 2, 3]
  if (nums) {
    1 => {}
    else => {
      nums.add(4)
    }
  }
}
"#),
        "the `else =>` arm of a match is skipped by the walker, so `.add()` there is invisible (Bug 2.6)";

    array_not_grown_in_any_arm_still_fires:
        assert_array_to_fixed_fires(
            r#"
function entrypoint() -> Nothing {
  let nums: Array<Int> = [1, 2, 3]
  if (nums) {
    1 => {}
    else => {
      let x = 5
    }
  }
}
"#),
        "visiting the `else =>` arm over-suppresses the hint when the arm does not grow the array";

    wait_wrapped_lend_call_on_an_array_is_suppressed:
        assert_array_to_fixed_suppressed(
            r#"
function growIt(lend arr: Int) -> Nothing { }

function entrypoint() -> Nothing {
  let nums: Array<Int> = [1, 2, 3]
  wait growIt(nums)
}
"#),
        "the walker does not recurse into `wait`, so a lend call that may grow the array behind it is invisible";
}

macro_rules! type_hint_cases {
    ($($case:ident: $body:block)+) => {
        $(
            // bug: `variable_type_hints` skips an unannotated `let`, or hints one that already carries its annotation | tier: Medium
            #[test]
            fn $case() $body
        )+
    };
}

type_hint_cases! {
    type_hint_fires_for_unannotated_let: {
        let (db, sf) = single_file("function entrypoint() -> Nothing {\n  let x = 42\n}\n");
        let hints = variable_type_hints(&db, sf);
        assert!(
            hints.iter().any(|h| h.type_text == "Int"),
            "unannotated `let x = 42` must get an `Int` type hint; got: {:?}",
            hints.iter().map(|h| &h.type_text).collect::<Vec<_>>()
        );
    }

    type_hint_is_suppressed_for_annotated_let: {
        let (db, sf) = single_file("function entrypoint() -> Nothing {\n  let x: Int = 42\n}\n");
        let hints = variable_type_hints(&db, sf);
        assert!(
            hints.is_empty(),
            "annotated let must produce zero type hints; got: {hints:?}"
        );
    }
}

macro_rules! copy_point_cases {
    ($($case:ident: $body:block)+) => {
        $(
            // bug: `copy_point_hints` misses a trivially-copyable argument (nested in an inner call), hints it twice, or hints a heap-allocated `String` argument | tier: Medium
            #[test]
            fn $case() $body
        )+
    };
}

copy_point_cases! {
    // Bug 2.14: the walker only inspected the top-level arguments of the outermost call.
    nested_call_arg_gets_copy_hint: {
        let src = r#"
function inner(x: Int) -> Nothing { }
function outer(result: Int) -> Nothing { }

function entrypoint() -> Nothing {
    let n: Int = 5
    outer(inner(n))
}
"#;
        let (db, sf) = single_file(src);
        let hints = copy_point_hints(&db, sf);
        let end_of_n = src.find("inner(n)").unwrap() + "inner(n".len();
        assert!(
            hints.iter().any(|h| h.position == end_of_n),
            "expected a copy hint at `n` inside `inner(n)` (byte {end_of_n}), got: {hints:?}"
        );
    }

    top_level_copyable_arg_gets_exactly_one_copy_hint: {
        let src = r#"
function consume(x: Int) -> Nothing { }

function entrypoint() -> Nothing {
    let n: Int = 42
    consume(n)
}
"#;
        let (db, sf) = single_file(src);
        let hints = copy_point_hints(&db, sf);
        let end_of_n = src.find("consume(n)").unwrap() + "consume(n".len();
        let hints_at_n = hints.iter().filter(|h| h.position == end_of_n).count();
        assert_eq!(hints_at_n, 1, "got: {hints:?}");
    }

    string_arg_gets_no_copy_hint: {
        let src = "function greet(s: String) -> String { return s }\n\
                   function entrypoint() -> Nothing {\n  let r = greet(`hi`)\n}\n";
        let (db, sf) = single_file(src);
        let hints = copy_point_hints(&db, sf);
        assert!(
            hints.is_empty(),
            "a String argument must not get a copy hint; got: {hints:?}"
        );
    }
}
