// v0.3-M9 C1q (card #524): which `for` loops hand their rows over, and which reads of the loop
// variable are the hand-over (`TypedModule::loop_row_handovers`, `IMP-ownership.md` "Loop rows").
//
// A loop the record names makes codegen take the row and blank its cell. A loop named when the
// list IS read later, or a read named when the row is read again, leaves a row with two owners (a
// double free); a loop the record leaves out costs a copy where Rust moves. Each case asserts the
// record and the diagnostic, not the run: the counts and ASan runs are
// `v03_m9_c1q_loop_row_handover.rs` in `ynz-driver`.

mod support;

use support::{errors, run};

const ARRAY_ROWS: &str = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n";

const COPY_REFUSAL: &str = "one cell of `matrix`";

/// What the record and the diagnostics must say for one program.
enum Expect {
    /// This many loops and handed reads, and no error.
    Record(usize, usize),
    /// Nothing handed over, and an error containing the phrase (the `.copy()` refusal).
    CopyRefusal(&'static str),
    /// An error containing the phrase, whatever the record holds.
    RefusedWith(&'static str),
    /// No loop handed over, and at least one error.
    RefusedNotHandedOver,
    /// At least one error (a row stored twice), whatever the record holds.
    Refused,
    /// No loop and no read handed over.
    NothingHandedOver,
}

fn assert_handover(source: &str, expected: Expect) {
    let output = run(source);
    let handovers = &output.typed_module.loop_row_handovers;
    let (loops, stores) = (handovers.loop_count(), handovers.store_count());
    let found = errors(&output);
    match expected {
        Expect::Record(expected_loops, expected_stores) => assert_eq!(
            (loops, stores, found.len()),
            (expected_loops, expected_stores, 0),
            "{source}\n{found:#?}"
        ),
        Expect::CopyRefusal(phrase) => {
            assert_eq!((loops, stores), (0, 0), "{source}");
            assert!(
                found.iter().any(|message| message.contains(phrase)),
                "expected the loop-cell refusal for:\n{source}\ngot {found:#?}"
            );
        }
        Expect::RefusedWith(phrase) => assert!(
            found.iter().any(|message| message.contains(phrase)),
            "{source}\n{found:#?}"
        ),
        Expect::RefusedNotHandedOver => {
            assert_eq!(loops, 0, "{source}");
            assert!(!found.is_empty(), "{source}");
        }
        Expect::Refused => assert!(
            !found.is_empty(),
            "a row stored twice must be refused\n{source}"
        ),
        Expect::NothingHandedOver => assert_eq!((loops, stores), (0, 0), "{source}"),
    }
}

macro_rules! handover_cases {
    ($($case:ident: $source:expr => $expected:expr;)+) => {
        $(
            // bug: typeck's loop-row hand-over record names a loop whose list or row is read again (a row with two owners: double free), or leaves out a loop it may name (a copy where Rust moves) | tier: High
            #[test]
            fn $case() {
                assert_handover($source, $expected);
            }
        )+
    };
}

handover_cases! {
    a_list_not_read_again_hands_its_array_rows_over: ARRAY_ROWS => Expect::Record(1, 1);
    a_list_read_after_the_loop_keeps_the_copy_refusal:
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  print(matrix.count().toString())
  return kept
}\n"
        => Expect::CopyRefusal(COPY_REFUSAL);
    a_list_read_in_the_body_keeps_the_copy_refusal:
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    print(matrix.count().toString())
    kept.add(row)
  }
  return kept
}\n"
        => Expect::CopyRefusal(COPY_REFUSAL);
    a_list_read_after_an_enclosing_if_keeps_the_copy_refusal:
        "function keep(give matrix: Array<Array<Int>>, wanted: Boolean) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  if (wanted) {
    for (row in matrix) {
      kept.add(row)
    }
  }
  print(matrix.count().toString())
  return kept
}\n"
        => Expect::CopyRefusal(COPY_REFUSAL);
    a_row_read_after_its_store_is_refused_as_any_stored_value:
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
    print(row.count().toString())
  }
  return kept
}\n"
        => Expect::RefusedWith("was stored in");
    a_borrowed_list_never_hands_its_rows_over:
        "function keep(share matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::RefusedNotHandedOver;
    a_row_stored_in_a_loop_nested_in_the_body_is_refused:
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    for (pass in range(2)) {
      kept.add(row)
    }
  }
  return kept
}\n"
        => Expect::Refused;
    a_string_row_stored_at_its_last_read_is_handed_over:
        "function keep(give matrix: Array<String>) -> Array<String> {
  let kept: Array<String> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::Record(1, 1);
    a_string_row_read_again_after_its_store_is_not_handed_over:
        "function keep(give matrix: Array<String>) -> Array<String> {
  let kept: Array<String> = []
  for (row in matrix) {
    kept.add(row)
    print(row)
  }
  return kept
}\n"
        => Expect::Record(1, 0);
    a_string_row_stored_in_a_nested_loop_is_not_handed_over:
        "function keep(give matrix: Array<String>) -> Array<String> {
  let kept: Array<String> = []
  for (row in matrix) {
    for (pass in range(2)) {
      kept.add(row)
    }
  }
  return kept
}\n"
        => Expect::Record(1, 0);
    rows_of_a_fixed_list_of_shapes_keep_the_copy_refusal:
        "shape Tag { label: String }
function keep(give matrix: Fixed<Tag>) -> Array<Tag> {
  let kept: Array<Tag> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::RefusedNotHandedOver;
    a_loop_over_a_list_of_numbers_hands_nothing_over:
        "function total(give numbers: Array<Int>) -> Array<Int> {
  let kept: Array<Int> = []
  for (number in numbers) {
    kept.add(number)
  }
  return kept
}\n"
        => Expect::Record(0, 0);
    a_list_reassigned_in_the_body_keeps_the_copy_refusal:
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
    matrix = [[9]]
  }
  return kept
}\n"
        => Expect::CopyRefusal(COPY_REFUSAL);
    a_const_list_never_hands_its_rows_over:
        "function keep() -> Array<Array<Int>> {
  const matrix: Array<Array<Int>> = [[1], [2]]
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::CopyRefusal("one cell of");
    a_list_in_a_function_that_suspends_never_hands_its_rows_over:
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    wait sleep(1)
    kept.add(row)
  }
  return kept
}\n"
        => Expect::CopyRefusal("one cell of");
    a_list_that_is_not_a_plain_binding_never_hands_its_rows_over:
        "shape Holder {
  rows: Array<Array<Int>>
}
function keep(give holder: Holder) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in holder.rows) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::CopyRefusal("one cell of");
    rows_of_a_fixed_list_of_unions_keep_the_copy_refusal:
        "shape Big { name: String }
shape Tiny { flag: Boolean }
shape Thing = Big | Tiny
function keep(give matrix: Fixed<Thing>) -> Array<Thing> {
  let kept: Array<Thing> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::RefusedNotHandedOver;
    a_map_entry_loop_hands_nothing_over:
        "function keep(give table: Map<String, Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for ((name, row) in table) {
    kept.add(row)
  }
  return kept
}\n"
        => Expect::NothingHandedOver;
}
