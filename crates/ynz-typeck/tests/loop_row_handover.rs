// v0.3-M9 C1q (card #524): which `for` loops hand their rows over, and which reads of the loop
// variable are the hand-over (`TypedModule::loop_row_handovers`, `IMP-ownership.md` "Loop rows").
//
// WHY (High tier: ownership and data integrity): a loop the record names makes codegen take the
// row and blank its cell. A loop named when the list IS read later, or a read named when the row
// is read again, leaves a row with two owners (a double free); a loop the record leaves out costs
// a copy where Rust moves. Each case asserts the record and the diagnostic, not the run: the
// counts and ASan runs are `v03_m9_c1q_loop_row_handover.rs` in `ynz-driver`.

mod support;

use support::{error_diagnostics, errors, run};

const ARRAY_ROWS: &str = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n";

/// The number of loops and of handed reads the program's record holds, and its errors.
fn record_of(source: &str) -> (usize, usize, Vec<String>) {
    let output = run(source);
    let handovers = &output.typed_module.loop_row_handovers;
    (
        handovers.loop_count(),
        handovers.store_count(),
        errors(&output),
    )
}

#[test]
fn a_list_not_read_again_hands_its_array_rows_over() {
    let (loops, stores, errors) = record_of(ARRAY_ROWS);
    assert_eq!((loops, stores, errors.len()), (1, 1, 0), "{errors:#?}");
}

#[test]
fn a_list_read_after_the_loop_keeps_the_copy_refusal() {
    let source = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  print(matrix.count().toString())
  return kept
}\n";
    let (loops, stores, found) = record_of(source);
    assert_eq!((loops, stores), (0, 0));
    assert!(
        found
            .iter()
            .any(|message| message.contains("one cell of `matrix`")),
        "{found:#?}"
    );
}

#[test]
fn a_list_read_in_the_body_keeps_the_copy_refusal() {
    let source = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    print(matrix.count().toString())
    kept.add(row)
  }
  return kept
}\n";
    let (loops, stores, found) = record_of(source);
    assert_eq!((loops, stores), (0, 0));
    assert!(
        found
            .iter()
            .any(|message| message.contains("one cell of `matrix`")),
        "{found:#?}"
    );
}

#[test]
fn a_list_read_after_an_enclosing_if_keeps_the_copy_refusal() {
    let source =
        "function keep(give matrix: Array<Array<Int>>, wanted: Boolean) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  if (wanted) {
    for (row in matrix) {
      kept.add(row)
    }
  }
  print(matrix.count().toString())
  return kept
}\n";
    let (loops, _, found) = record_of(source);
    assert_eq!(loops, 0);
    assert!(
        found
            .iter()
            .any(|message| message.contains("one cell of `matrix`")),
        "{found:#?}"
    );
}

#[test]
fn a_row_read_after_its_store_is_refused_as_any_stored_value() {
    let source = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
    print(row.count().toString())
  }
  return kept
}\n";
    let (_, _, found) = record_of(source);
    assert!(
        found
            .iter()
            .any(|message| message.contains("was stored in")),
        "{found:#?}"
    );
}

#[test]
fn a_borrowed_list_never_hands_its_rows_over() {
    let source = "function keep(share matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n";
    let (loops, _, found) = record_of(source);
    assert_eq!(loops, 0);
    assert!(!found.is_empty(), "a row of a `share` list needs `.copy()`");
}

#[test]
fn a_row_stored_in_a_loop_nested_in_the_body_is_refused_with_what_instead_and_why() {
    let source = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    for (pass in range(2)) {
      kept.add(row)
    }
  }
  return kept
}\n";
    let output = run(source);
    let diagnostics = error_diagnostics(&output);
    assert!(
        !diagnostics.is_empty(),
        "a row stored twice must be refused"
    );
    for diagnostic in diagnostics {
        let rendered = format!("{diagnostic:?}");
        assert!(
            !diagnostic.what_instead.is_empty() && !diagnostic.why.is_empty(),
            "a refusal needs WHAT / WHAT-INSTEAD / WHY: {rendered}"
        );
    }
}

#[test]
fn a_string_row_stored_at_its_last_read_is_handed_over() {
    let source = "function keep(give matrix: Array<String>) -> Array<String> {
  let kept: Array<String> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n";
    let (loops, stores, errors) = record_of(source);
    assert_eq!((loops, stores, errors.len()), (1, 1, 0), "{errors:#?}");
}

#[test]
fn a_string_row_read_again_after_its_store_is_not_handed_over() {
    let source = "function keep(give matrix: Array<String>) -> Array<String> {
  let kept: Array<String> = []
  for (row in matrix) {
    kept.add(row)
    print(row)
  }
  return kept
}\n";
    let (loops, stores, errors) = record_of(source);
    assert_eq!((loops, stores, errors.len()), (1, 0, 0), "{errors:#?}");
}

#[test]
fn a_string_row_stored_in_a_nested_loop_is_not_handed_over() {
    let source = "function keep(give matrix: Array<String>) -> Array<String> {
  let kept: Array<String> = []
  for (row in matrix) {
    for (pass in range(2)) {
      kept.add(row)
    }
  }
  return kept
}\n";
    let (loops, stores, errors) = record_of(source);
    assert_eq!((loops, stores, errors.len()), (1, 0, 0), "{errors:#?}");
}

#[test]
fn rows_of_a_fixed_list_of_shapes_keep_the_copy_refusal() {
    let source = "shape Tag { label: String }
function keep(give matrix: Fixed<Tag>) -> Array<Tag> {
  let kept: Array<Tag> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n";
    let (loops, _, found) = record_of(source);
    assert_eq!(loops, 0);
    assert!(!found.is_empty(), "{found:#?}");
}

#[test]
fn a_loop_over_a_list_of_numbers_hands_nothing_over() {
    let source = "function total(give numbers: Array<Int>) -> Array<Int> {
  let kept: Array<Int> = []
  for (number in numbers) {
    kept.add(number)
  }
  return kept
}\n";
    let (loops, stores, errors) = record_of(source);
    assert_eq!((loops, stores, errors.len()), (0, 0, 0), "{errors:#?}");
}

#[test]
fn a_list_reassigned_in_the_body_keeps_the_copy_refusal() {
    // WHY (High): `matrix = [...]` in the body releases the list the loop is still walking; a row
    // taken from a cell of it is read through freed memory (the probe aborted at run time).
    let source = "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
    matrix = [[9]]
  }
  return kept
}\n";
    let (loops, stores, found) = record_of(source);
    assert_eq!((loops, stores), (0, 0));
    assert!(
        found
            .iter()
            .any(|message| message.contains("one cell of `matrix`")),
        "{found:#?}"
    );
}

/// The loop is refused with the `.copy()` text and nothing is recorded as handed over.
fn assert_keeps_the_copy_refusal(source: &str) {
    let (loops, stores, found) = record_of(source);
    assert_eq!((loops, stores), (0, 0), "{source}");
    assert!(
        found.iter().any(|message| message.contains("one cell of")),
        "expected the loop-cell refusal for:\n{source}\ngot {found:#?}"
    );
}

#[test]
fn a_const_list_never_hands_its_rows_over() {
    // WHY (High): a `const` list cannot lose a row; blanking its cell would mutate it.
    assert_keeps_the_copy_refusal(
        "function keep() -> Array<Array<Int>> {
  const matrix: Array<Array<Int>> = [[1], [2]]
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n",
    );
}

#[test]
fn a_list_in_a_function_that_suspends_never_hands_its_rows_over() {
    // WHY (High): a suspension inside the loop can resume after the list was released elsewhere;
    // the hand-over is only admitted for a function that never suspends.
    assert_keeps_the_copy_refusal(
        "function keep(give matrix: Array<Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in matrix) {
    wait sleep(1)
    kept.add(row)
  }
  return kept
}\n",
    );
}

#[test]
fn a_list_that_is_not_a_plain_binding_never_hands_its_rows_over() {
    // WHY (High): the loop can only prove "not read again" for a name; a field of a shape can be
    // reached through the shape.
    assert_keeps_the_copy_refusal(
        "shape Holder {
  rows: Array<Array<Int>>
}
function keep(give holder: Holder) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for (row in holder.rows) {
    kept.add(row)
  }
  return kept
}\n",
    );
}

#[test]
fn rows_of_a_fixed_list_of_unions_keep_the_copy_refusal() {
    // WHY (High): a `Fixed` list of unions is kept on the `.copy()` refusal (commit `980bdea`):
    // no hand-over is recorded.
    let source = "shape Big { name: String }
shape Tiny { flag: Boolean }
shape Thing = Big | Tiny
function keep(give matrix: Fixed<Thing>) -> Array<Thing> {
  let kept: Array<Thing> = []
  for (row in matrix) {
    kept.add(row)
  }
  return kept
}\n";
    let (loops, _, found) = record_of(source);
    assert_eq!(loops, 0);
    assert!(!found.is_empty(), "{found:#?}");
}

#[test]
fn a_map_entry_loop_hands_nothing_over() {
    // WHY (High): a map is not a list of cells; only list loops are handed over.
    let source = "function keep(give table: Map<String, Array<Int>>) -> Array<Array<Int>> {
  let kept: Array<Array<Int>> = []
  for ((name, row) in table) {
    kept.add(row)
  }
  return kept
}\n";
    let (loops, stores, _) = record_of(source);
    assert_eq!((loops, stores), (0, 0));
}
