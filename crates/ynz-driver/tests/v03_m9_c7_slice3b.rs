//! v0.3-M9 C7 slice 3b: the pieces that finish C7 (`IMP-ownership.md` "Scope-exit release on the
//! plain path"): a nested shape moves out of a temporary, holders of an untraceable `maybe` /
//! union of a channel own a counted reference, a reassigned binding reads consistently, an
//! errors-capable local releases its old result at a reassignment, and writes through a place a
//! `let` views are guarded under ASan.
//!
//! Each case runs at both optimization levels. The counters say every allocation was freed (the
//! LEAK side) and AddressSanitizer with `free_fill_byte=0` says nothing was freed early or twice
//! (the SAFETY side). All are High tier: a wrong release is a use-after-free, a double free or a
//! leak, silent at exit 0.

mod support;

use support::{
    assert_asan_and_leaks_clean, assert_asan_clean, assert_heap_balanced, assert_strings_balanced,
    at_both_optimization_levels, run_balanced,
};

/// What a case checks the counters of.
#[derive(Clone, Copy)]
enum Balance {
    Heap,
    HeapAndStrings,
    Strings,
}

fn check(label: &str, program: &str, expected: &str, balance: Balance) {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced(label, program, expected, no_optimize);
        match balance {
            Balance::Heap => assert_heap_balanced(label, &counters, no_optimize),
            Balance::HeapAndStrings => {
                assert_heap_balanced(label, &counters, no_optimize);
                assert_strings_balanced(label, &counters, no_optimize);
            }
            Balance::Strings => assert_strings_balanced(label, &counters, no_optimize),
        }
        assert_asan_clean(label, program, expected, no_optimize);
    });
}

/// `case!(name, PROGRAM, "stdout", Balance::X, "the bug it catches (High)")`.
macro_rules! case {
    ($case:ident, $program:expr, $expected:literal, $balance:expr, $bug:literal) => {
        // bug: a nested shape take, channel hand-back, errors reassignment or write under a let view is leaked, freed early or freed twice in a built program, silently at exit 0 | tier: High
        #[doc = $bug]
        #[test]
        fn $case() {
            check(stringify!($case), $program, $expected, $balance);
        }
    };
}

// ── Item 1: a nested shape moves out of a temporary ─────────────────────────────────────────

const TAKE_NESTED_SHAPE: &str = "
shape Tag {
  name: String
  id: Int
}
shape Wrap {
  inner: Tag
  label: String
}
function make(index: Int) -> Wrap {
  let tag: Tag = { name: `t${index}`, id: index }
  return { inner: tag, label: `l${index}` }
}
function entrypoint() -> Nothing {
  let total: Int = 0
  let kept: Tag = { name: `k`, id: 7 }
  for (index in range(0, 100)) {
    let taken: Tag = make(index).inner
    kept = make(index + 1).inner
    total = total + taken.id + kept.id
  }
  print(total)
}
";

// bug: `let taken = make(i).inner` deep-copies the nested shape's strings where Rust moves it (200 calls build 2 strings and one cell each; the `let` and the reassignment must add no string and no cell) | tier: Medium
#[test]
fn a_nested_shape_taken_from_a_temporary_is_not_copied() {
    at_both_optimization_levels(|no_optimize| {
        let counters = run_balanced("take_nested", TAKE_NESTED_SHAPE, "10000\n", no_optimize);
        assert_eq!(
            (counters.alloc, counters.free),
            (200, 200),
            "no_optimize={no_optimize}: one cell per `make` call, none per take: {counters:?}"
        );
        assert_eq!(
            (counters.string_alloc, counters.string_free),
            (401, 401),
            "no_optimize={no_optimize}: two strings per `make` call and the literal `k`: {counters:?}"
        );
    });
}

const TAKE_A_NESTED_SHAPE_OF_A_NESTED_SHAPE: &str = "
shape Leaf {
  name: String
  numbers: Array<Int>
}
shape Branch {
  leaf: Leaf
  label: String
}
shape Trunk {
  branch: Branch
  note: String
}
function grow(index: Int) -> Trunk {
  let leaf: Leaf = { name: `leaf${index}`, numbers: [index, 1] }
  let branch: Branch = { leaf: leaf, label: `branch${index}` }
  return { branch: branch, note: `note${index}` }
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (index in range(0, 50)) {
    let branch: Branch = grow(index).branch
    let leaf: Leaf = grow(index).branch.leaf
    total = total + branch.leaf.numbers.count() + leaf.name.count() + branch.label.count()
  }
  print(total)
}
";

case!(
    a_nested_shape_two_levels_deep_taken_from_a_temporary_balances,
    TAKE_A_NESTED_SHAPE_OF_A_NESTED_SHAPE,
    "780\n",
    Balance::HeapAndStrings,
    "taking a nested shape (and one of its own nested shapes) out of a temporary frees the emptied cell twice, or leaves the temporary's release reading the taken cell (High)"
);

// ── Item 6: a reassigned binding owns every value it holds ──────────────────────────────────

const A_REASSIGNED_MAYBE_COPIES_AT_EVERY_EVENT: &str = r"
shape Tag {
  name: String
  id: Int
}
function entrypoint() -> Nothing {
  let tags: Array<Tag> = [{ name: `a${1}`, id: 1 }, { name: `b${2}`, id: 2 }]
  let names: Array<String> = [`aa-${1}`, `bb-${2}`]
  let total: Int = 0
  for (round in range(0, 50)) {
    let held: Maybe<Tag> = tags.get(0)
    let label: Maybe<String> = names.get(0)
    held = tags.get(1)
    label = names.get(1)
    total = total + held.or({ name: `z`, id: 0 }).id + label.or(`q`).count()
  }
  print(total)
}
";

case!(
    an_annotated_maybe_reassigned_from_an_element_releases_its_first_value,
    A_REASSIGNED_MAYBE_COPIES_AT_EVERY_EVENT,
    "300\n",
    Balance::HeapAndStrings,
    "an annotated `maybe` whose `let` copies and whose reassignment viewed an element lost the release of its first payload: one string per iteration (High)"
);

const A_REASSIGNED_ARRAY_COPIES_AT_EVERY_EVENT: &str = r"
shape Bucket {
  rows: Array<Int>
  id: Int
}
function entrypoint() -> Nothing {
  let bucket: Bucket = { rows: [10, 20, 30], id: 1 }
  let total: Int = 0
  for (round in range(0, 50)) {
    let rows: Array<Int> = [round]
    rows = bucket.rows
    total = total + rows.count()
  }
  print(total)
}
";

case!(
    an_array_reassigned_from_a_field_releases_its_first_value_and_its_copy,
    A_REASSIGNED_ARRAY_COPIES_AT_EVERY_EVENT,
    "150\n",
    Balance::Heap,
    "an array binding that is reassigned from a field it viewed lost its first value (leak) or freed the field's own array (High)"
);

// ── Item 6: an errors-capable local that is reassigned releases its old result ──────────────

const AN_ERRORS_CAPABLE_LOCAL_REASSIGNED_RELEASES_THE_OLD_RESULT: &str = r"
function load(index: Int) -> String errors {
  return `value-${index}`
}
function loadRows(index: Int) -> Array<Int> errors {
  return [index, index]
}
function entrypoint() -> Nothing {
  let total: Int = 0
  let result = load(0)
  let rows = loadRows(0)
  for (index in range(0, 50)) {
    result = load(index)
    rows = loadRows(index)
    if (result.failed()) {
      print(`bad`)
    }
    total = total + result.count()
    let taken: String = result
    total = total + taken.count()
    result = load(index + 1)
    total = total + result.or(`x`).count() + rows.or([]).count()
  }
  print(total)
}
";

case!(
    an_errors_capable_local_reassigned_releases_the_old_result,
    AN_ERRORS_CAPABLE_LOCAL_REASSIGNED_RELEASES_THE_OLD_RESULT,
    "1271\n",
    Balance::HeapAndStrings,
    "an errors-capable local that is reassigned (outside its loop, after a narrowing read, after its string was taken) leaks the old ok payload or frees a taken one twice (High)"
);

// ── Item 2: a function hands back an owned channel reference ────────────────────────────────

const CHANNELS_HANDED_BACK_AND_RELAYED: &str = "
function firstChan(share chans: Array<Channel<Int>>) -> Maybe<Channel<Int>> {
  return chans.first()
}
function relay(share chans: Array<Channel<Int>>) -> Maybe<Channel<Int>> {
  let held = firstChan(chans)
  return held
}
function passOn(share ch: Channel<Int>) -> Channel<Int> {
  return ch
}
function relayChannel(share ch: Channel<Int>) -> Channel<Int> {
  let held = passOn(ch)
  return held
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 20)) {
    let chans: Array<Channel<Int>> = [Channel<Int>()]
    let got = relay(chans)
    for (item in chans) {
      let same = relayChannel(item)
      let again = passOn(same)
    }
    firstChan(chans)
    total = total + 1
  }
  print(total)
}
";

// ── Item 5: a `MapEntry` spawned to a task keeps its own decimal key ────────────────────────

const A_MAP_ENTRY_WITH_A_DECIMAL_KEY_IS_SPAWNED: &str = "
function work(e: MapEntry<Number<10>, Int>) -> Nothing {
  print(e.key)
}
function entrypoint() -> Nothing {
  let table: Map<Number<10>, Int> = { 1.5: 1, 2.5: 2, 3.5: 3 }
  for (entry in table) {
    background work(entry)
  }
  wait sleep(200)
}
";

/// `ynz build` of `program`: its exit status and the text it printed.
fn build_result(program: &str, label: &str) -> (bool, String) {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("program.ynz");
    std::fs::write(&source, program).expect("write the program");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ynz"))
        .arg("build")
        .arg(&source)
        .env("CLICOLOR", "0")
        .output()
        .unwrap_or_else(|err| panic!("{label}: spawn `ynz build`: {err}"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

/// A `for (entry in table)` body that keeps `entry` somewhere that outlives the iteration.
fn entry_escape(body: &str) -> String {
    format!(
        "shape Holder {{\n  entry: MapEntry<Number<10>, Int>\n}}\n\
         function entrypoint() -> Nothing {{\n  let table: Map<Number<10>, Int> = {{ 1.5: 1 }}\n  \
         let list: Array<MapEntry<Number<10>, Int>> = []\n  let wire: Channel<MapEntry<Number<10>, Int>> = Channel<MapEntry<Number<10>, Int>>()\n  \
         for (entry in table) {{\n    {body}\n  }}\n}}\n"
    )
}

/// A map entry points at the loop's per-site staging, whose decimal key and value buffers the next
/// iteration overwrites, so each way to keep it beyond the iteration is refused at compile time; the
/// one accepted use is a `background` argument, which `value_to_stable_bits` re-homes (the spawn case).
fn assert_entry_escape_refused(body: &str, phrase: &str) {
    let (built, text) = build_result(&entry_escape(body), body);
    assert!(!built, "`{body}` must not compile:\n{text}");
    assert!(
        text.contains(phrase),
        "`{body}` refused for another reason:\n{text}"
    );
}

// ── Item 3: a write to a place a `let` would view makes the `let` a copy ────────────────────

const A_RECURSIVE_UNION_BOX_REPLACED_UNDER_A_VIEW: &str = "
shape End {
  flag: Boolean
}
shape Node {
  value: Int
  label: String
  next: Node | End
}
function describe(share link: Node | End) -> Int {
  if (link) {
    is Node => {
      return link.value + link.label.count()
    }
    is End => {
      return 0
    }
  }
  return 0
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 20)) {
    let stop: End = { flag: true }
    let tail: End = { flag: false }
    let after: End = { flag: true }
    let last: Node = { value: 3, label: `last${round}`, next: stop }
    let head: Node = { value: 1, label: `head${round}`, next: last }
    let view = head.next
    let second: Node = { value: 9, label: `second${round}`, next: tail }
    let wrapped: Node | End = second
    head.next = wrapped
    total = total + describe(view)
    let again = head.next
    let fresh: Node = { value: 5, label: `fresh${round}`, next: after }
    head = fresh
    total = total + describe(again)
  }
  print(total)
}
";

case!(
    a_recursive_union_box_replaced_while_a_let_would_view_it_is_not_freed_under_the_reader,
    A_RECURSIVE_UNION_BOX_REPLACED_UNDER_A_VIEW,
    "500\n",
    Balance::Strings,
    "a `let` that views a recursive-union field reads the box a later field assignment or root reassignment released (High)"
);

const A_NESTED_CELL_REPLACED_OR_RELEASED_UNDER_A_VIEW: &str = "
shape Tag {
  name: String
  id: Int
}
shape Holder {
  inner: Tag
  label: String
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (round in range(0, 20)) {
    let first: Tag = { name: `n${round}`, id: round }
    let holder: Holder = { inner: first, label: `w${round}` }
    let view = holder.inner
    let replaced: Tag = { name: `replaced${round}`, id: 100 }
    holder.inner = replaced
    total = total + view.name.count() + view.id
    let second = holder.inner
    let again: Tag = { name: `again${round}`, id: 7 }
    let other: Holder = { inner: again, label: `x` }
    holder = other
    total = total + second.name.count() + second.id
  }
  print(total)
}
";

case!(
    a_nested_cell_replaced_or_released_while_a_let_would_view_it_is_not_read_after_free,
    A_NESTED_CELL_REPLACED_OR_RELEASED_UNDER_A_VIEW,
    "2430\n",
    Balance::HeapAndStrings,
    "a `let` that views a nested shape field reads the cell a field assignment or a root reassignment released (High)"
);

// ── Item 4: a union returned from a local is an owned value ─────────────────────────────────

const A_UNION_RETURNED_FROM_A_LOCAL: &str = "
shape Circle {
  label: String
}
shape Square {
  side: Int
}
shape Figure = Circle | Square
function make(index: Int) -> Figure {
  let circle: Circle = { label: `c${index}` }
  let figure: Figure = circle
  return figure
}
function relay(index: Int) -> Figure {
  let inner = make(index)
  return inner
}
function pick(share figure: Figure) -> Figure {
  return figure
}
function area(share figure: Figure) -> Int {
  if (figure) {
    is Circle => {
      return figure.label.count()
    }
    is Square => {
      return figure.side
    }
  }
  return 0
}
function entrypoint() -> Nothing {
  let total: Int = 0
  for (index in range(0, 50)) {
    let first = make(index)
    let second = relay(index)
    let third = pick(first)
    total = total + area(first) + area(second) + area(third) + area(make(index))
  }
  print(total)
}
";

case!(
    a_union_returned_from_a_local_or_a_parameter_is_owned_by_its_caller_once,
    A_UNION_RETURNED_FROM_A_LOCAL,
    "560\n",
    Balance::Strings,
    "a union a function returns (a moved local, a relayed result, a `share` parameter) is copied twice, or its payload strings are freed by the callee and again by the caller, or the unbound result leaks (High)"
);

/// One named test per hand-written program of the `slice 3b` unit (the `case!` table runs inline sources; these build fixture directories, spawn tasks or assert compile refusals).
macro_rules! slice3b_program_high_case {
    ($($case_name:ident => $case_body:block)+) => { $(
        // bug: a nested shape take, channel hand-back, map entry spawn or entry escape in a built program is leaked, freed early or freed twice, or typeck accepts a map entry that outlives its iteration, silently at exit 0 | tier: High
        #[test]
        fn $case_name() $case_body
    )+ };
}

slice3b_program_high_case! {
    // case: Bug (High): a wrong take of `let taken = make(i).inner` is a double free or a leak.
    a_nested_shape_taken_from_a_temporary_is_taken_and_freed_once => {
        at_both_optimization_levels(|no_optimize| {
            let counters = run_balanced("take_nested", TAKE_NESTED_SHAPE, "10000\n", no_optimize);
            assert_heap_balanced("take_nested", &counters, no_optimize);
            assert_strings_balanced("take_nested", &counters, no_optimize);
            assert_asan_clean("take_nested", TAKE_NESTED_SHAPE, "10000\n", no_optimize);
        });
    }
    // case: Bug (High): a holder of an untraceable `maybe<channel>` result leaked (no founder), and an owned local the callee returned left its reference orphaned (the caller shared again). Every reference now has one owner: the callee's return shares what it does not move, the caller moves the result, and each holder gives its reference back at its scope exit.
    a_returned_channel_or_maybe_channel_is_one_counted_reference_given_back_once => {
        at_both_optimization_levels(|no_optimize| {
            assert_asan_and_leaks_clean(
                "channels_handed_back",
                CHANNELS_HANDED_BACK_AND_RELAYED,
                "20\n",
                no_optimize,
            );
        });
    }
    // case: An imported function hands back a `maybe` of its argument's channel and the argument's owner dies before the holder (the `NoStaticFounder` reproducer of `transfer_record.rs`): the holder owns a reference of its own, so the channel is neither leaked nor freed early.
    a_maybe_channel_an_imported_function_returns_outlives_its_arguments_owner => {
        at_both_optimization_levels(|no_optimize| {
            let built = support::build("v0_3_m9_c7_imported_maybe_channel", no_optimize);
            let sanitized = support::build_asan(&built);
            let output = std::process::Command::new(&sanitized)
                .env("ASAN_OPTIONS", "detect_leaks=1:free_fill_byte=0")
                .output()
                .expect("run the sanitized binary");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success() && !stderr.contains("Sanitizer"),
                "no_optimize={no_optimize}: ASan or LeakSanitizer reported:\n{stderr}"
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n1\n");
        });
    }
    // case: Bug (High: a task reads a buffer the next iteration overwrote, or freed memory): the stable copy of a spawned `MapEntry` copied the value half and left a decimal KEY as the address of the loop's per-site key buffer. The three tasks printed `3.5` and then garbage. The tasks run concurrently, so the lines are compared as a set.
    a_map_entry_spawned_to_a_task_keeps_its_own_decimal_key => {
        at_both_optimization_levels(|no_optimize| {
            let built = support::build_source(
                A_MAP_ENTRY_WITH_A_DECIMAL_KEY_IS_SPAWNED,
                "map_entry_decimal_key",
                no_optimize,
            );
            let output = std::process::Command::new(&built.binary)
                .output()
                .expect("run the program");
            let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_string)
                .collect();
            lines.sort();
            assert_eq!(
                lines,
                vec!["1.5", "2.5", "3.5"],
                "no_optimize={no_optimize}: stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        });
    }
    a_map_entry_bound_by_let_outlives_its_iteration_is_refused => {
        assert_entry_escape_refused("let kept = entry", "gives `entry` away");
    }
    a_map_entry_added_to_a_list_is_refused => {
        assert_entry_escape_refused("list.add(entry)", "gives `entry` away");
    }
    a_map_entry_in_a_shape_literal_is_refused => {
        assert_entry_escape_refused("let holder: Holder = { entry: entry }", "gives `entry` away");
    }
    a_map_entry_in_an_array_literal_is_refused => {
        assert_entry_escape_refused("let items = [entry]", "gives `entry` away");
    }
    a_map_entry_sent_on_a_channel_is_refused => {
        assert_entry_escape_refused("wire.send(entry)", "is not supported yet");
    }
    a_map_entry_copied_is_refused => {
        assert_entry_escape_refused("let copied = entry.copy()", "cannot make a separate `MapEntry");
    }
}
