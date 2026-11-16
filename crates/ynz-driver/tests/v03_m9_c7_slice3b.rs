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
    assert_asan_clean, assert_heap_balanced, assert_strings_balanced, at_both_optimization_levels,
    run_balanced,
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
  name: string
  id: int
}
shape Wrap {
  inner: Tag
  label: string
}
function make(index: int) -> Wrap {
  let tag: Tag = { name: `t${index}`, id: index }
  return { inner: tag, label: `l${index}` }
}
function entrypoint() -> nothing {
  let total: int = 0
  let kept: Tag = { name: `k`, id: 7 }
  for (index in range(0, 100)) {
    let taken: Tag = make(index).inner
    kept = make(index + 1).inner
    total = total + taken.id + kept.id
  }
  print(total)
}
";

#[test]
fn a_nested_shape_taken_from_a_temporary_is_not_copied_and_balances() {
    // Bug (cost, Golden Rule 8; High: a wrong take is a double free): `let taken = make(i).inner`
    // deep-copies the nested shape's strings where Rust moves it. 200 calls build 2 strings and one
    // cell each; the `let` and the reassignment must add no string and no cell.
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
        assert_asan_clean("take_nested", TAKE_NESTED_SHAPE, "10000\n", no_optimize);
    });
}

const TAKE_A_NESTED_SHAPE_OF_A_NESTED_SHAPE: &str = "
shape Leaf {
  name: string
  numbers: array<int>
}
shape Branch {
  leaf: Leaf
  label: string
}
shape Trunk {
  branch: Branch
  note: string
}
function grow(index: int) -> Trunk {
  let leaf: Leaf = { name: `leaf${index}`, numbers: [index, 1] }
  let branch: Branch = { leaf: leaf, label: `branch${index}` }
  return { branch: branch, note: `note${index}` }
}
function entrypoint() -> nothing {
  let total: int = 0
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
  name: string
  id: int
}
function entrypoint() -> nothing {
  let tags: array<Tag> = [{ name: `a${1}`, id: 1 }, { name: `b${2}`, id: 2 }]
  let names: array<string> = [`aa-${1}`, `bb-${2}`]
  let total: int = 0
  for (round in range(0, 50)) {
    let held: maybe<Tag> = tags.get(0)
    let label: maybe<string> = names.get(0)
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
  rows: array<int>
  id: int
}
function entrypoint() -> nothing {
  let bucket: Bucket = { rows: [10, 20, 30], id: 1 }
  let total: int = 0
  for (round in range(0, 50)) {
    let rows: array<int> = [round]
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
function load(index: int) -> string errors {
  return `value-${index}`
}
function loadRows(index: int) -> array<int> errors {
  return [index, index]
}
function entrypoint() -> nothing {
  let total: int = 0
  let result = load(0)
  let rows = loadRows(0)
  for (index in range(0, 50)) {
    result = load(index)
    rows = loadRows(index)
    if (result.failed()) {
      print(`bad`)
    }
    total = total + result.count()
    let taken: string = result
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

/// Run `program` under ASan with leak detection on. A channel is a counted `Arc` the runtime does
/// not instrument, so a reference nobody gives back is a leak this run reports, and a reference
/// given back twice is a free this run reports.
fn assert_channels_balanced(label: &str, program: &str, expected: &str, no_optimize: bool) {
    let built = support::build_source(program, label, no_optimize);
    let sanitized = support::build_asan(&built);
    let output = std::process::Command::new(&sanitized)
        .env("ASAN_OPTIONS", "detect_leaks=1:free_fill_byte=0")
        .output()
        .expect("run the sanitized binary");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && !stderr.contains("Sanitizer"),
        "{label} (no_optimize={no_optimize}): ASan or LeakSanitizer reported:\n{stderr}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        expected,
        "{label} (no_optimize={no_optimize})"
    );
}

const CHANNELS_HANDED_BACK_AND_RELAYED: &str = "
function firstChan(share chans: array<channel<int>>) -> maybe<channel<int>> {
  return chans.first()
}
function relay(share chans: array<channel<int>>) -> maybe<channel<int>> {
  let held = firstChan(chans)
  return held
}
function passOn(share ch: channel<int>) -> channel<int> {
  return ch
}
function relayChannel(share ch: channel<int>) -> channel<int> {
  let held = passOn(ch)
  return held
}
function entrypoint() -> nothing {
  let total: int = 0
  for (round in range(0, 20)) {
    let chans: array<channel<int>> = [channel<int>()]
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

#[test]
fn a_returned_channel_or_maybe_channel_is_one_counted_reference_given_back_once() {
    // Bug (High): a holder of an untraceable `maybe<channel>` result leaked (no founder), and an
    // owned local the callee returned left its reference orphaned (the caller shared again).
    // Every reference now has one owner: the callee's return shares what it does not move, the
    // caller moves the result, and each holder gives its reference back at its scope exit.
    at_both_optimization_levels(|no_optimize| {
        assert_channels_balanced(
            "channels_handed_back",
            CHANNELS_HANDED_BACK_AND_RELAYED,
            "20\n",
            no_optimize,
        );
    });
}

/// An imported function hands back a `maybe` of its argument's channel and the argument's owner dies
/// before the holder (the `NoStaticFounder` reproducer of `transfer_record.rs`): the holder owns a
/// reference of its own, so the channel is neither leaked nor freed early.
#[test]
fn a_maybe_channel_an_imported_function_returns_outlives_its_arguments_owner() {
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

// ── Item 5: a `MapEntry` spawned to a task keeps its own decimal key ────────────────────────

const A_MAP_ENTRY_WITH_A_DECIMAL_KEY_IS_SPAWNED: &str = "
function work(e: MapEntry<number<10>, int>) -> nothing {
  print(e.key)
}
function entrypoint() -> nothing {
  let table: map<number<10>, int> = { 1.5: 1, 2.5: 2, 3.5: 3 }
  for (entry in table) {
    background work(entry)
  }
  wait sleep(200)
}
";

#[test]
fn a_map_entry_spawned_to_a_task_keeps_its_own_decimal_key() {
    // Bug (High: a task reads a buffer the next iteration overwrote, or freed memory): the stable
    // copy of a spawned `MapEntry` copied the value half and left a decimal KEY as the address of the
    // loop's per-site key buffer. The three tasks printed `3.5` and then garbage. The tasks run
    // concurrently, so the lines are compared as a set.
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
        "shape Holder {{\n  entry: MapEntry<number<10>, int>\n}}\n\
         function entrypoint() -> nothing {{\n  let table: map<number<10>, int> = {{ 1.5: 1 }}\n  \
         let list: array<MapEntry<number<10>, int>> = []\n  let wire: channel<MapEntry<number<10>, int>> = channel<MapEntry<number<10>, int>>()\n  \
         for (entry in table) {{\n    {body}\n  }}\n}}\n"
    )
}

#[test]
fn typeck_refuses_every_way_a_map_entry_outlives_its_iteration() {
    // Evidence for the C7 item 5 claim (High: an entry points at the loop's per-site staging, whose
    // decimal key and value buffers the next iteration overwrites). Each escape is refused at
    // compile time; the one accepted use that keeps an entry beyond the iteration is a `background`
    // argument, which `value_to_stable_bits` re-homes (the case above).
    let refused_by_a_store = [
        ("let", "let kept = entry"),
        ("list add", "list.add(entry)"),
        ("shape literal", "let holder: Holder = { entry: entry }"),
        ("array literal", "let items = [entry]"),
    ];
    for (label, body) in refused_by_a_store {
        let (built, text) = build_result(&entry_escape(body), label);
        assert!(!built, "{label}: `{body}` must not compile:\n{text}");
        assert!(
            text.contains("gives `entry` away"),
            "{label}: refused for another reason:\n{text}"
        );
    }
    let (built, text) = build_result(&entry_escape("wire.send(entry)"), "send");
    assert!(!built, "a send of an entry must not compile:\n{text}");
    assert!(text.contains("is not supported yet"), "send: {text}");
    let (built, text) = build_result(&entry_escape("let copied = entry.copy()"), "copy");
    assert!(!built, "a copy of an entry must not compile:\n{text}");
    assert!(text.contains("cannot make a separate `MapEntry"), "copy: {text}");
}

// ── Item 3: a write to a place a `let` would view makes the `let` a copy ────────────────────

const A_RECURSIVE_UNION_BOX_REPLACED_UNDER_A_VIEW: &str = "
shape End {
  flag: boolean
}
shape Node {
  value: int
  label: string
  next: Node | End
}
function describe(share link: Node | End) -> int {
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
function entrypoint() -> nothing {
  let total: int = 0
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
  name: string
  id: int
}
shape Holder {
  inner: Tag
  label: string
}
function entrypoint() -> nothing {
  let total: int = 0
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
