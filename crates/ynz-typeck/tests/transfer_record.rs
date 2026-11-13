// v0.3-M9 C5a (plan #17): the persisted transfer record and the loop-carried give rejection.
//
// WHY (High tier: ownership / data integrity): spike S3 found six programs typeck accepts that
// print correctly today and become use-after-free under ANY scope-exit release pass, because the
// transfer record missed the transfer. They are one class: a heap value stored without a sink,
// plus multi-root alias. Each test below asserts typeck's recorded ownership facts directly
// (not a program's output): there is no release pass yet, so the miss is only observable here.
//
//   hole  program                         expected fact
//   t3    outer binding given in a loop   rejected: used again on the next pass (Consumed)
//   t6b   b.items = a; return b           a and b one class, escapes by return
//   g1    o.add(a); return o              a and o one class, escapes by return
//   g8    o[0] = a; return o              a and o one class, escapes by return
//   g7    acc = per (per-iteration local) acc and per one class, anchored at acc (outlives per)
//   t7    { first: a, second: c }         a, c and the literal's binding all one class
//
// Each fact test failed on the base commit (the record did not exist; the stub returned no
// classes); the rejection test failed because typeck accepted the program.
//
// WHY this many tests for one High-tier unit (the SOP says stop at about 8 and justify): the
// unit is the whole ownership record, and each test is one way the checker can lose a transfer
// at its own code site (a field store, a container store, an index store, a reassignment, a
// literal, a call-site store, a call-result receiver, a loop cell, a builtin result, a loop
// give). Reverting each producer in turn (the C5a fix commits, and a one-line mutation of each
// remaining site) failed only that site's tests. The case tables below are one NAMED test per
// case, so the runner parallelizes them and names the failing program; a case is in a table only
// when it takes a different checker path.

use std::sync::atomic::{AtomicUsize, Ordering};

use ynz_diagnostics::{Diagnostic, DiagnosticKind, Severity};
use ynz_parser::{CompilerDb, SourceFile};
use ynz_typeck::transfer_record::{Escape, FunctionTransfers, NoFounderReason, ReleaseAnchor};
use ynz_typeck::{check_query, CheckOutput};

fn run(source: &str) -> CheckOutput {
    let db = CompilerDb::default();
    // Every program needs an `entrypoint`; the cases under test are the other functions.
    let program = format!("{source}\nfunction entrypoint() -> nothing {{\n}}\n");
    let source_file = SourceFile::new(&db, "test.ynz".to_string(), program);
    (*check_query(&db, source_file)).clone()
}

/// Check `main_source` (as `main.ynz`) in a project that also holds `library_source` (as
/// `lib.ynz`, imported with ``import { .. } from `lib` ``). The files must exist on disk for the
/// import to resolve; each call gets its own directory so parallel tests never share one.
fn run_with_library(library_source: &str, main_source: &str) -> CheckOutput {
    static NEXT_PROJECT: AtomicUsize = AtomicUsize::new(0);
    let project_directory = std::env::temp_dir().join(format!(
        "ynz_transfer_record_{}_{}",
        std::process::id(),
        NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&project_directory).expect("create the project directory");
    std::fs::write(
        project_directory.join("yinz.toml"),
        "[project]\nname = \"test\"\n",
    )
    .expect("write yinz.toml");
    let library_path = project_directory.join("lib.ynz");
    let main_path = project_directory.join("main.ynz");
    std::fs::write(&library_path, library_source).expect("write lib.ynz");
    std::fs::write(&main_path, main_source).expect("write main.ynz");

    let mut db = CompilerDb::default();
    let library_file = SourceFile::new(
        &db,
        library_path.display().to_string(),
        library_source.to_string(),
    );
    let main_file = SourceFile::new(
        &db,
        main_path.display().to_string(),
        main_source.to_string(),
    );
    db.register_source(library_file);
    db.register_source(main_file);
    let output = (*check_query(&db, main_file)).clone();
    let _ = std::fs::remove_dir_all(&project_directory);
    output
}

fn error_diagnostics(output: &CheckOutput) -> Vec<&Diagnostic> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .collect()
}

fn errors(output: &CheckOutput) -> Vec<String> {
    error_diagnostics(output)
        .into_iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect()
}

/// The record of `function_name` in `output`, which must be free of errors.
fn record_of(output: &CheckOutput, function_name: &str) -> FunctionTransfers {
    let found = errors(output);
    assert!(found.is_empty(), "expected a clean program, got {found:#?}");
    output
        .typed_module
        .transfers
        .function(function_name)
        .unwrap_or_else(|| panic!("no transfer record for `{function_name}`"))
        .clone()
}

/// Run `source`, require zero errors, return the record of `function_name`.
fn transfers_of(source: &str, function_name: &str) -> FunctionTransfers {
    record_of(&run(source), function_name)
}

fn founder_name(transfers: &FunctionTransfers, member: &str) -> String {
    let class = transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"));
    match &class.anchor {
        ReleaseAnchor::Founder { codegen_name, .. } => codegen_name.clone(),
        other => panic!("expected a founder for `{member}`, got {other:?}"),
    }
}

fn escapes_of(transfers: &FunctionTransfers, member: &str) -> Vec<Escape> {
    transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"))
        .escapes
        .clone()
}

fn assert_accepted(source: &str) {
    let found = errors(&run(source));
    assert!(found.is_empty(), "expected accepted, got {found:#?}");
}

/// The program must be rejected with the use-after-give text.
fn assert_use_after_give(source: &str) {
    let found = errors(&run(source));
    assert!(
        found
            .iter()
            .any(|message| message.contains("was already given away")),
        "expected a use-after-give for:\n{source}\ngot {found:#?}"
    );
}

/// One named test per program: `cases!(check; name: source, "bug it catches (High)";)` expands to
/// `#[test] fn name() { check(&source) }` with the bug line as its doc.
macro_rules! cases {
    ($check:ident; $($case:ident: $source:expr, $bug:literal;)+) => {
        $(
            #[doc = $bug]
            #[test]
            fn $case() {
                $check(&$source);
            }
        )+
    };
}

const BAG: &str = "shape Bag {\n  items: array<int>\n}\n";
const PAIR: &str = "shape Pair {\n  first: array<int>\n  second: array<int>\n}\n";

// ── the five record holes ───────────────────────────────────────────────────

/// (High) `b.items = a; return b`: the field-set stores `a` into `b` without a sink, so `a`
/// is released at scope exit while the returned `b` holds it.
#[test]
fn field_set_joins_the_stored_value_into_the_receivers_class() {
    let source = format!(
        "{BAG}function make() -> Bag {{
  let a: array<int> = [1, 2, 3]
  let b: Bag = {{ items: [0] }}
  b.items = a
  return b
}}"
    );
    let transfers = transfers_of(&source, "make");
    assert!(transfers.same_class("a", "b"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "a"), vec![Escape::Returned]);
}

/// (High) `o.add(a); return o`: the container `.add` stores `a` without a sink.
#[test]
fn container_add_joins_the_stored_value_into_the_receivers_class() {
    let source = "function make() -> array<array<int>> {
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  o.add(a)
  return o
}";
    let transfers = transfers_of(source, "make");
    assert!(transfers.same_class("a", "o"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "a"), vec![Escape::Returned]);
}

/// (High) `o[0] = a; return o`: the index-set stores `a` without a sink.
#[test]
fn index_set_joins_the_stored_value_into_the_receivers_class() {
    let source = "function make() -> array<array<int>> {
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = [[0]]
  o[0] = a
  return o
}";
    let transfers = transfers_of(source, "make");
    assert!(transfers.same_class("a", "o"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "a"), vec![Escape::Returned]);
}

/// (High) `acc = per` inside a loop: `acc` outlives `per`, so a release at the back-edge of
/// `per` frees what `acc` still holds. The class must be one, anchored at `acc`.
#[test]
fn assign_from_a_loop_local_anchors_the_class_at_the_outliving_binding() {
    let source = "function f() -> int {
  let acc: array<int> = [0]
  let i: int = 0
  while (i < 2) {
    let per: array<int> = [i, i, i]
    acc = per
    i = i + 1
  }
  return acc.count()
}";
    let transfers = transfers_of(source, "f");
    assert!(transfers.same_class("acc", "per"), "{transfers:#?}");
    assert_eq!(founder_name(&transfers, "per"), "acc");
}

/// (High) `{ first: a, second: c }` returned through `p`: the binding joined only the first
/// root, so `c` was released while the returned `p` held it.
#[test]
fn multi_root_literal_joins_every_root() {
    let source = format!(
        "{PAIR}function make() -> Pair {{
  let a: array<int> = [1, 2, 3]
  let c: array<int> = [4, 5]
  let p: Pair = {{ first: a, second: c }}
  return p
}}"
    );
    let transfers = transfers_of(&source, "make");
    assert!(transfers.same_class("a", "c"), "{transfers:#?}");
    assert!(transfers.same_class("a", "p"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "c"), vec![Escape::Returned]);
}

// ── t3: a move inside a loop, rejected ──────────────────────────────────────

const EAT: &str = "function eat(give rows: array<int>) -> nothing {
  print(rows.count().toString())
}\n";

/// The program must be rejected with the canonical use-after-give text and the loop wording.
fn assert_loop_give_rejected(source: &str) {
    let found = error_diagnostics(&run(source))
        .into_iter()
        .map(|diagnostic| (diagnostic.kind.clone(), diagnostic.what.clone()))
        .collect::<Vec<_>>();
    assert!(
        found.iter().any(|(kind, what)| {
            *kind == Some(DiagnosticKind::Consumed)
                && what.contains("was already given away")
                && what.contains("previous pass")
        }),
        "expected the loop-carried give to be rejected for:\n{source}\ngot {found:#?}"
    );
}

/// The send twin: same rejection with the use-after-send template.
fn assert_loop_send_rejected(source: &str) {
    let found = error_diagnostics(&run(source))
        .into_iter()
        .map(|diagnostic| (diagnostic.kind.clone(), diagnostic.what.clone()))
        .collect::<Vec<_>>();
    assert!(
        found.iter().any(|(kind, what)| {
            *kind == Some(DiagnosticKind::ConsumedBySend)
                && what.contains("was sent into")
                && what.contains("previous pass")
        }),
        "expected the loop-carried send to be rejected for:\n{source}\ngot {found:#?}"
    );
}

cases! { assert_loop_give_rejected;
    give_in_a_while_loop_is_rejected: format!(
        "{EAT}function l(c: boolean) -> nothing {{
  let x: array<int> = [1, 2, 3]
  let i: int = 0
  while (i < 3) {{
    if (c) {{ return }}
    eat(x)
    i = i + 1
  }}
}}"
    ), "(High) t3, the exact spike probe: an outer binding given on one pass of a `while` is gone on the next; typeck accepted it, so a release pass would double-free";
    give_in_a_for_loop_is_rejected: format!(
        "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(x)
  }}
}}"
    ), "(High) t3 in a `for` loop: the second loop shape is checked by its own code path";
    give_of_an_alias_in_a_loop_is_rejected: format!(
        "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  let y: array<int> = x
  for (i in range(0, 3)) {{
    eat(y)
  }}
}}"
    ), "(High) t3 through an alias: the binding given (`y`) is declared outside the loop and its class-mate `x` is what the next pass would read";
    give_in_a_while_condition_is_rejected: format!(
        "function holds(give rows: array<int>) -> boolean {{
  return rows.count() > 0
}}
{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  while (holds(x)) {{
    print(`pass`)
  }}
}}"
    ), "(High) t3 through the condition: a `while` condition runs at the top of every pass, so a give made in it reaches the next pass; the loop was opened only after the condition was checked, so typeck accepted a double give";
    give_in_an_inner_loop_reaches_the_outer_pass: format!(
        "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (j in range(0, 2)) {{
    let y: array<int> = x
    for (i in range(0, 1)) {{
      eat(y)
      y = [0]
    }}
  }}
}}"
    ), "(High) t3 across nested loops: the inner loop revives `y` so it carries nothing, but the outer pass reads `x` again; the inner loop dropped the give, so `x`'s value was given once per outer pass (double free)";
}

cases! { assert_loop_send_rejected;
    send_in_a_for_loop_is_rejected: "function producer(lend wire: channel<array<int>>) -> nothing {
  let sent: array<int> = [10, 20, 30]
  for (i in range(0, 3)) {
    wire.send(sent)
  }
  wire.close()
}", "(High) t3 for `send`: the loop-carried check also covers a send, whose wording is its own arm; an accepted program sends one array twice and two receivers free it";
    send_in_an_inner_loop_reaches_the_outer_pass: "function producer(lend wire: channel<array<int>>) -> nothing {
  let sent: array<int> = [10, 20, 30]
  for (j in range(0, 2)) {
    let y: array<int> = sent
    for (i in range(0, 1)) {
      wire.send(y)
      y = [0]
    }
  }
  wire.close()
}", "(High) t3 for `send` across nested loops: the enclosing loop must check a send the inner loop does not carry";
}

cases! { assert_accepted;
    give_of_a_loop_local_is_accepted: format!(
        "{EAT}function l() -> nothing {{
  for (i in range(0, 3)) {{
    let x: array<int> = [1, 2, 3]
    eat(x)
  }}
}}"
    ), "(High) over-rejection: a give of a binding the loop itself declares carries nothing into the next pass";
    give_then_reassign_is_accepted: format!(
        "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(x)
    x = [4, 5]
  }}
}}"
    ), "(High) over-rejection: a reassignment before the back-edge revives the binding";
    give_on_a_path_that_returns_is_accepted: format!(
        "{EAT}function l(c: boolean) -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    if (c) {{
      eat(x)
      return
    }}
  }}
}}"
    ), "(High) over-rejection: a give on a path that leaves the loop never reaches the next pass";
    unread_class_mate_of_a_revived_binding_is_accepted: format!(
        "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  let snapshot: array<int> = x
  for (i in range(0, 3)) {{
    eat(x)
    x = [4, 5]
  }}
}}"
    ), "(High) over-rejection: a class-mate the loop never mentions stays consumed after the given binding is revived, but nothing in the next pass reads it";
    give_after_the_loop_is_accepted: format!(
        "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    print(x.count().toString())
  }}
  eat(x)
}}"
    ), "(High) over-rejection: a give after the loop is the ordinary single give";
    give_of_an_outer_loop_local_in_an_inner_loop_is_accepted: format!(
        "{EAT}function l() -> nothing {{
  for (j in range(0, 2)) {{
    let x: array<int> = [1, 2, 3]
    for (i in range(0, 1)) {{
      eat(x)
      x = [0]
    }}
  }}
}}"
    ), "(High) over-rejection: a give the inner loop hands up is of a binding the outer loop declares afresh each pass, so the outer loop carries nothing";
}

// ── the record's boundaries (no over-joining, anchors, escapes) ─────────────

/// (High) Values that are not heap values never join: `o.add(a[0])` stores an int, and
/// `let n = a.count()` is a number. Joining them would only delay releases.
#[test]
fn storing_a_scalar_joins_nothing() {
    let source = "function f() -> int {
  let a: array<int> = [1, 2, 3]
  let o: array<int> = []
  o.add(a[0])
  o.add(a.count())
  return o.count()
}";
    let transfers = transfers_of(source, "f");
    assert!(!transfers.same_class("a", "o"), "{transfers:#?}");
}

const EAT2: &str = "function eat_all(give rows: array<array<int>>) -> nothing {
  print(rows.count().toString())
}\n";

/// (High) A store joins; it does not consume: both names stay readable (the existing
/// `bg_arg_alias_container_add_red` fixture depends on it, and a false rejection breaks
/// programs that compile today).
#[test]
fn a_store_joins_without_consuming() {
    assert_accepted(
        "function f() -> int {
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  o.add(a)
  return a.count() + o.count()
}",
    );
}

/// (High) A store joins in the LIVE scope too: a later give of the container consumes the
/// stored value's name, else `eat_all(o)` frees the row `a` still names.
#[test]
fn a_give_of_the_container_consumes_the_stored_values_name() {
    assert_use_after_give(&format!(
        "{EAT2}function f() -> int {{
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  o.add(a)
  eat_all(o)
  return a.count()
}}"
    ));
}

// Escapes are recorded per class: a give, a send and a store into a temporary are facts the
// release pass reads (a `return` is asserted by the store tests above).

/// (High) A `give` of the container is recorded as an escape of every member (`a` here), on
/// the path that gives; a binding that is only returned fresh (`[0]`) adds none.
#[test]
fn a_give_is_recorded_as_an_escape_of_the_whole_class() {
    let source = format!(
        "{EAT2}function f(c: boolean) -> array<int> {{
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  o.add(a)
  if (c) {{
    eat_all(o)
  }}
  return [0]
}}"
    );
    let transfers = transfers_of(&source, "f");
    assert_eq!(
        escapes_of(&transfers, "a"),
        vec![Escape::Given {
            callee: "eat_all".to_string()
        }]
    );
}

/// (High) A `send` is recorded as `Sent` on the channel: a release pass that read no escape
/// would free the sent array at the producer's scope exit while the receiver reads it.
#[test]
fn a_send_is_recorded_as_an_escape_on_its_channel() {
    let source = "function producer(lend wire: channel<array<int>>) -> nothing {
  let sent: array<int> = [10, 20, 30]
  wire.send(sent)
  wire.close()
}";
    let transfers = transfers_of(source, "producer");
    assert_eq!(
        escapes_of(&transfers, "sent"),
        vec![Escape::Sent {
            channel: "wire".to_string()
        }]
    );
}

/// (High) A store into a place no binding names (`makeBag().items = a`) is recorded, not
/// dropped: the temporary holds `a`, so nothing here may release it as a plain local.
#[test]
fn a_store_into_a_temporary_is_recorded_as_an_escape() {
    let source = format!(
        "{BAG}function makeBag() -> Bag {{
  return {{ items: [0] }}
}}
function f() -> nothing {{
  let a: array<int> = [1, 2, 3]
  makeBag().items = a
}}"
    );
    let transfers = transfers_of(&source, "f");
    assert_eq!(
        escapes_of(&transfers, "a"),
        vec![Escape::StoredIntoTemporary]
    );
}

/// (High) A class with a parameter the caller keeps (`lend`, `share` or bare) is never released
/// here: the caller owns every object in it.
#[test]
fn a_borrowed_parameter_in_the_class_means_the_caller_owns_it() {
    let source = "function stash(lend bucket: array<array<int>>, rows: array<int>) -> nothing {
  bucket.add(rows)
}";
    let transfers = transfers_of(source, "stash");
    assert!(transfers.same_class("bucket", "rows"));
    let class = transfers.class_of("rows").expect("class");
    assert!(
        matches!(&class.anchor, ReleaseAnchor::CallerOwns { .. }),
        "{class:#?}"
    );
}

/// (High) The founder is the member whose scope exits last: a deeper owner never anchors a
/// class an outer owner is in.
#[test]
fn the_outer_owner_is_the_founder_of_a_class_with_a_deeper_owner() {
    let source = "function f() -> int {
  let outer: array<array<int>> = []
  if (true) {
    let inner: array<int> = [1]
    outer.add(inner)
  }
  return outer.count()
}";
    let transfers = transfers_of(source, "f");
    assert_eq!(founder_name(&transfers, "inner"), "outer");
}

/// (High) Members declared in one scope anchor at the earliest declared.
#[test]
fn members_of_one_scope_anchor_at_the_earliest_declared() {
    let source = "function f() -> int {
  let first: array<int> = [1]
  let second: array<int> = first
  return second.count()
}";
    let transfers = transfers_of(source, "f");
    assert_eq!(founder_name(&transfers, "second"), "first");
}

/// (High) The honest remainder: a non-owner outlives an owner, so which value it holds at the
/// owner's scope exit is only known at run time. Recorded as "no static founder", not guessed.
#[test]
fn a_non_owner_that_outlives_the_owner_leaves_no_static_founder() {
    let source = format!(
        "{BUCKET}function make() -> Bucket {{
  let b: Bucket = {{ rows: [9] }}
  return b
}}
function f(c: boolean) -> int {{
  let cell: array<int> = make().rows
  if (c) {{
    let x: array<int> = [1]
    cell = x
  }}
  return cell.count()
}}"
    );
    let transfers = transfers_of(&source, "f");
    assert!(transfers.same_class("cell", "x"), "{transfers:#?}");
    let class = transfers.class_of("x").expect("class");
    assert_eq!(
        class.anchor,
        ReleaseAnchor::NoStaticFounder(NoFounderReason::OutlivedByNonOwner {
            holder: "cell".to_string()
        })
    );
}

const BUCKET: &str = "shape Bucket {\n  rows: array<int>\n}\n";

// ── a call that stores one argument into another ────────────────────────────

const STORING_CALLEES: &str = "function put(lend o: array<array<int>>, a: array<int>) -> nothing {
  o.add(a)
}
function relay(lend o: array<array<int>>, a: array<int>) -> nothing {
  put(o, a)
}
function peek(o: array<array<int>>, a: array<int>) -> int {
  return o.count() + a.count()
}\n";

/// The program `make` stores nothing itself: `call` is the only line that might tie `a` to `o`.
fn make_with_call(call: &str) -> String {
    format!(
        "{STORING_CALLEES}function make() -> array<array<int>> {{
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  {call}
  return o
}}"
    )
}

fn assert_a_and_o_joined(source: &str) {
    let transfers = transfers_of(source, "make");
    assert!(transfers.same_class("a", "o"), "{transfers:#?}");
}

fn assert_a_and_o_apart(source: &str) {
    let transfers = transfers_of(source, "make");
    assert!(!transfers.same_class("a", "o"), "{transfers:#?}");
}

cases! { assert_a_and_o_joined;
    call_stores_an_argument_directly: make_with_call("put(o, a)"),
        "(High) `put(o, a)` stores `a` into `o` INSIDE the callee with no sink at the call site: `a` is released at scope exit while the returned `o` holds it";
    call_stores_an_argument_through_a_relay: make_with_call("relay(o, a)"),
        "(High) the store happens in the callee's own call (`relay` -> `put`): a one-level solve misses it";
}

cases! { assert_a_and_o_apart;
    call_that_only_reads_joins_nothing: make_with_call("peek(o, a)"),
        "(High) a callee that only reads both arguments must tie nothing: a spurious join delays releases and rejects valid programs";
}

/// (High) `put(o, a)` where `put` is IMPORTED: its body is invisible, so the call could store
/// any argument into any other. Every heap argument joins; without it `a` is released while the
/// returned `o` may hold it. (The imported-callee branch of the call-store solve.)
#[test]
fn a_call_to_an_imported_function_joins_its_heap_arguments() {
    let library = "export function put(lend o: array<array<int>>, a: array<int>) -> nothing {
  o.add(a)
}\n";
    let main = "import { put } from `lib`
function make() -> array<array<int>> {
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  put(o, a)
  return o
}
function entrypoint() -> nothing {
}
";
    let transfers = record_of(&run_with_library(library, main), "make");
    assert!(transfers.same_class("a", "o"), "{transfers:#?}");
}

const EAT_ROWS: &str = "function eat(give o: array<array<int>>) -> nothing {
  print(o.count().toString())
}\n";

/// A program that stores through `call`, gives the container away, then reads `a`.
fn give_after_call(call: &str) -> String {
    format!(
        "{EAT_ROWS}{STORING_CALLEES}function descend(lend o: array<array<int>>, a: array<int>, depth: int) -> nothing {{
  if (depth > 0) {{
    descend(o, a, depth - 1)
  }}
  o.add(a)
}}
function f() -> nothing {{
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  {call}
  eat(o)
  print(a.count().toString())
}}"
    )
}

cases! { assert_use_after_give;
    give_after_a_direct_call_store_is_rejected: give_after_call("put(o, a)"),
        "(High) the record joined `a` and `o` after `put(o, a)` but the LIVE scope did not, so `eat(o); a.count()` was accepted and `eat` frees the row `a` still names";
    give_after_a_relayed_call_store_is_rejected: give_after_call("relay(o, a)"),
        "(High) the live merge must follow a store that travels through a relay";
    give_after_a_recursive_call_store_is_rejected: give_after_call("descend(o, a, 2)"),
        "(High) the live merge must follow a store made by a recursive callee (the solve repeats until no class changes)";
}

/// (High) The converse: a callee that only READS `from` into `into` ties nothing, so giving
/// `into` leaves `from` readable (no false rejection of a program that is correct).
#[test]
fn a_callee_that_only_reads_an_argument_leaves_it_readable_after_a_give() {
    assert_accepted(
        "function tally(lend into: array<int>, from: array<int>) -> nothing {
  into.add(from.count())
}
function eatInts(give rows: array<int>) -> nothing {
  print(rows.count().toString())
}
function g() -> nothing {
  let from: array<int> = [1, 2, 3]
  let into: array<int> = []
  tally(into, from)
  eatInts(into)
  print(from.count().toString())
}",
    );
}

// ── the remaining no-owner cases ────────────────────────────────────────────

fn assert_no_owner(transfers: &FunctionTransfers, member: &str) {
    let class = transfers
        .class_of(member)
        .unwrap_or_else(|| panic!("`{member}` is in no class"));
    assert_eq!(class.anchor, ReleaseAnchor::NoOwner, "{class:#?}");
}

/// (High) A class with no owner at all is recorded as `NoOwner`: the allocation, if any,
/// belongs to code this function cannot see, so nothing here may release it. Never a founder,
/// never a guess. A piece of a temporary (`make().rows`):
#[test]
fn a_piece_of_a_temporary_is_recorded_as_no_owner() {
    let source = format!(
        "{BUCKET}function make() -> Bucket {{
  let b: Bucket = {{ rows: [9] }}
  return b
}}
function f() -> int {{
  let cell: array<int> = make().rows
  return cell.count()
}}"
    );
    assert_no_owner(&transfers_of(&source, "f"), "cell");
}

/// (High) `NoOwner` for a loop cell of a temporary (`for (row in make())`).
#[test]
fn a_loop_cell_of_a_temporary_is_recorded_as_no_owner() {
    let source = "function make() -> array<array<int>> {
  let rows: array<array<int>> = [[1]]
  return rows
}
function f() -> int {
  let total: int = 0
  for (row in make()) {
    total = total + row.count()
  }
  return total
}";
    assert_no_owner(&transfers_of(source, "f"), "row");
}

/// (High) `NoOwner` for an IMPORTED function's result: its allocation belongs to the other
/// file, so this function may never release it.
#[test]
fn an_imported_functions_result_is_recorded_as_no_owner() {
    let library = "export function fresh() -> array<int> {
  return [1]
}\n";
    let main = "import { fresh } from `lib`
function g() -> int {
  let result: array<int> = fresh()
  return result.count()
}
function entrypoint() -> nothing {
}
";
    let transfers = record_of(&run_with_library(library, main), "g");
    assert_no_owner(&transfers, "result");
}

// ── inference that feeds the record ─────────────────────────────────────────

/// (High) A `background` spawn inside a loop of a binding declared OUTSIDE it: the next pass
/// spawns with the same binding, so the first spawn must copy, never give (a second give is a
/// second owner of one array).
#[test]
fn a_spawn_in_a_loop_of_an_outer_binding_copies_it() {
    use ynz_typeck::check::BgOwnership;
    let spawn_in = |declared_outside: bool| -> Vec<BgOwnership> {
        let (outside, inside) = if declared_outside {
            ("  let rows: array<int> = [1, 2, 3]\n", "")
        } else {
            ("", "    let rows: array<int> = [1, 2, 3]\n")
        };
        let source = format!(
            "function work(rows: array<int>) -> nothing {{
  print(rows.count().toString())
}}
function f() -> nothing {{
{outside}  for (i in range(0, 3)) {{
{inside}    background work(rows)
  }}
}}"
        );
        let output = run(&source);
        let found = errors(&output);
        assert!(found.is_empty(), "{found:#?}");
        output
            .typed_module
            .background_arg_inferred_ownership
            .values()
            .cloned()
            .collect()
    };
    assert_eq!(spawn_in(true), vec![BgOwnership::Copy]);
    assert_eq!(spawn_in(false), vec![BgOwnership::Give]);
}

/// (High) A builtin method's result can be an ARGUMENT, not only a piece of the receiver:
/// `cache.first().or(fallback)` returns `fallback` when the cache is empty, `a.concat(b)` holds
/// the elements of both. Returned, `fallback` / `b` must escape with it.
#[test]
fn a_builtin_methods_result_reaches_its_heap_arguments() {
    let or_source = "function pick(cache: array<array<int>>) -> array<int> {
  let fallback: array<int> = [1]
  let found: array<int> = cache.first().or(fallback)
  return found
}";
    let transfers = transfers_of(or_source, "pick");
    assert!(transfers.same_class("found", "fallback"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "fallback"), vec![Escape::Returned]);

    let concat_source = "function both() -> array<array<int>> {
  let a: array<array<int>> = [[1]]
  let b: array<array<int>> = [[2]]
  let joined: array<array<int>> = a.concat(b)
  return joined
}";
    let transfers = transfers_of(concat_source, "both");
    assert!(transfers.same_class("a", "b"), "{transfers:#?}");
    assert_eq!(escapes_of(&transfers, "b"), vec![Escape::Returned]);
}

// ── receivers that are not a name chain (fixer round 1) ─────────────────────

const FIRST_BAG: &str = "shape Bag {
  items: array<int>
}
function firstBag(share bags: array<Bag>) -> Bag {
  let fallback: Bag = { items: [] }
  return bags[0].or(fallback)
}
";

/// (High) Bug caught: `firstBag(bags).items` is a piece of `bags` (the callee returns one of its
/// elements), but the field access rooted only in NAME chains and recorded no root. Stored into
/// the returned `o`, `n` then freed `bags[0].items` at `bags`'s scope exit while `o` still held it.
#[test]
fn a_field_of_a_call_result_is_a_piece_of_the_calls_arguments() {
    let source = format!(
        "{FIRST_BAG}function f() -> array<array<int>> {{
  let bags: array<Bag> = [{{ items: [1] }}]
  let o: array<array<int>> = []
  let n: array<int> = firstBag(bags).items
  o.add(n)
  return o
}}"
    );
    let transfers = transfers_of(&source, "f");
    assert!(transfers.same_class("bags", "o"), "{transfers:#?}");
    assert!(transfers.same_class("bags", "n"), "{transfers:#?}");
}

/// (High) Bug caught: a store whose PLACE is a call result (`firstBag(bags).items = a`) was
/// recorded as a store into a temporary, so `a` and the `bags` that now hold it stayed apart.
#[test]
fn a_store_into_a_piece_of_a_call_result_joins_the_arguments() {
    let source = format!(
        "{FIRST_BAG}function f() -> array<int> {{
  let bags: array<Bag> = [{{ items: [1] }}]
  let a: array<int> = [5]
  firstBag(bags).items = a
  return a
}}"
    );
    let transfers = transfers_of(&source, "f");
    assert!(transfers.same_class("bags", "a"), "{transfers:#?}");
}

/// (High) Bug caught: `for (row in rows.concat(extra))` joined the loop cell to nothing (the
/// iterated expression is a method call, not a name), so `out.add(row)` left `rows` and `extra`
/// free to release the rows `out` holds.
#[test]
fn a_loop_cell_joins_everything_the_iterated_value_reaches() {
    let source = "function f() -> array<array<int>> {
  let rows: array<array<int>> = [[1]]
  let extra: array<array<int>> = [[2]]
  let out: array<array<int>> = []
  for (row in rows.concat(extra)) {
    out.add(row)
  }
  return out
}";
    let transfers = transfers_of(source, "f");
    for member in ["rows", "extra", "out", "row"] {
        assert!(
            transfers.same_class("rows", member),
            "{member}: {transfers:#?}"
        );
    }
}
