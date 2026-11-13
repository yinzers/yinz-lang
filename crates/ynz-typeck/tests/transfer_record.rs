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

use ynz_diagnostics::Severity;
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

fn errors(output: &CheckOutput) -> Vec<String> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, Severity::Error))
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect()
}

/// Run `source`, require zero errors, return the record of `function_name`.
fn transfers_of(source: &str, function_name: &str) -> FunctionTransfers {
    let output = run(source);
    let found = errors(&output);
    assert!(found.is_empty(), "expected a clean program, got {found:#?}");
    output
        .typed_module
        .transfers
        .function(function_name)
        .unwrap_or_else(|| panic!("no transfer record for `{function_name}`"))
        .clone()
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

/// (High) t3: an outer binding given inside a loop is gone on the next pass. Each program is
/// rejected with the canonical use-after-give text and the loop wording.
#[test]
fn a_give_inside_a_loop_of_an_outer_binding_is_rejected() {
    let cases: [(&str, String); 3] = [
        (
            "while, the exact spike probe",
            format!(
                "{EAT}function l(c: boolean) -> nothing {{
  let x: array<int> = [1, 2, 3]
  let i: int = 0
  while (i < 3) {{
    if (c) {{ return }}
    eat(x)
    i = i + 1
  }}
}}"
            ),
        ),
        (
            "for loop",
            format!(
                "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(x)
  }}
}}"
            ),
        ),
        (
            "an alias of the outer binding is the one given",
            format!(
                "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  let y: array<int> = x
  for (i in range(0, 3)) {{
    eat(y)
  }}
}}"
            ),
        ),
    ];
    for (label, source) in cases {
        let found = errors(&run(&source));
        assert!(
            found
                .iter()
                .any(|e| e.contains("was already given away") && e.contains("previous pass")),
            "{label}: expected the loop-carried give to be rejected, got {found:#?}"
        );
    }
}

/// (High) The rejection must not catch the correct programs next to it: a give of a binding
/// the loop declares, a give followed by a reassignment, a give on a path that leaves the
/// loop, and a give after the loop.
#[test]
fn gives_that_do_not_carry_into_the_next_pass_are_accepted() {
    let cases: [(&str, String); 4] = [
        (
            "declared inside the loop",
            format!(
                "{EAT}function l() -> nothing {{
  for (i in range(0, 3)) {{
    let x: array<int> = [1, 2, 3]
    eat(x)
  }}
}}"
            ),
        ),
        (
            "reassigned before the next pass",
            format!(
                "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    eat(x)
    x = [4, 5]
  }}
}}"
            ),
        ),
        (
            "the path that gives then returns",
            format!(
                "{EAT}function l(c: boolean) -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    if (c) {{
      eat(x)
      return
    }}
  }}
}}"
            ),
        ),
        (
            "given after the loop",
            format!(
                "{EAT}function l() -> nothing {{
  let x: array<int> = [1, 2, 3]
  for (i in range(0, 3)) {{
    print(x.count().toString())
  }}
  eat(x)
}}"
            ),
        ),
    ];
    for (label, source) in cases {
        let found = errors(&run(&source));
        assert!(
            found.is_empty(),
            "{label}: expected accepted, got {found:#?}"
        );
    }
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

/// (High) A store joins; it does not consume: both names stay readable (the existing
/// `bg_arg_alias_container_add_red` fixture depends on it), and a later give of either name
/// consumes both.
#[test]
fn a_store_joins_without_consuming_and_a_give_consumes_the_class() {
    let reads_both = "function f() -> int {
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  o.add(a)
  return a.count() + o.count()
}";
    assert!(errors(&run(reads_both)).is_empty());

    let source = format!(
        "{EAT2}function f() -> int {{
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  o.add(a)
  eat_all(o)
  return a.count()
}}"
    );
    let found = errors(&run(&source));
    assert!(
        found.iter().any(|e| e.contains("was already given away")),
        "giving the container must consume the stored value's name: {found:#?}"
    );
}

const EAT2: &str = "function eat_all(give rows: array<array<int>>) -> nothing {
  print(rows.count().toString())
}\n";

/// (High) Escapes are recorded per class: a give and a return of members are both facts the
/// release pass reads.
#[test]
fn gives_and_returns_are_recorded_as_escapes() {
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

/// (High) A class with a parameter the caller keeps is never released here; a `give`
/// parameter is an owner.
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

/// (High) The founder is the member whose scope exits last; members declared in one scope
/// anchor at the earliest declared, and a deeper owner never anchors a class an outer owner
/// is in.
#[test]
fn the_founder_is_the_member_whose_scope_exits_last() {
    let outer_wins = "function f() -> int {
  let outer: array<array<int>> = []
  if (true) {
    let inner: array<int> = [1]
    outer.add(inner)
  }
  return outer.count()
}";
    let transfers = transfers_of(outer_wins, "f");
    assert_eq!(founder_name(&transfers, "inner"), "outer");

    let same_scope = "function f() -> int {
  let first: array<int> = [1]
  let second: array<int> = first
  return second.count()
}";
    let transfers = transfers_of(same_scope, "f");
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

/// (High) `put(o, a)` stores `a` into `o` INSIDE the callee, with no sink at the call site. The
/// caller's record must still join them, directly and through a relay; a callee that only reads
/// must join nothing (a spurious join would only delay releases).
#[test]
fn a_call_that_stores_one_argument_into_another_joins_them_in_the_caller() {
    let prelude = "function put(lend o: array<array<int>>, a: array<int>) -> nothing {
  o.add(a)
}
function relay(lend o: array<array<int>>, a: array<int>) -> nothing {
  put(o, a)
}
function peek(o: array<array<int>>, a: array<int>) -> int {
  return o.count() + a.count()
}\n";
    let cases: [(&str, &str, bool); 3] = [
        ("direct", "put(o, a)", true),
        ("through a relay (a callee's own call)", "relay(o, a)", true),
        ("a callee that only reads", "peek(o, a)", false),
    ];
    for (label, call, joined) in cases {
        let source = format!(
            "{prelude}function f() -> array<array<int>> {{
  let a: array<int> = [1, 2, 3]
  let o: array<array<int>> = []
  {call}
  return o
}}"
        );
        let transfers = transfers_of(&source, "f");
        assert_eq!(
            transfers.same_class("a", "o"),
            joined,
            "{label}: {transfers:#?}"
        );
    }
}

// ── the remaining no-static-founder cases ───────────────────────────────────

/// (High) Every class with no owner at all is recorded as `NoOwner`: the allocation, if any,
/// belongs to code this function cannot see (a piece of a temporary, an imported result, a
/// cell of a temporary), so nothing here may release it. Never a founder, never a guess.
#[test]
fn a_class_with_no_owner_is_recorded_as_no_owner() {
    let cases: [(&str, String, &str); 2] = [
        (
            "a piece of a temporary",
            format!(
                "{BUCKET}function make() -> Bucket {{
  let b: Bucket = {{ rows: [9] }}
  return b
}}
function f() -> int {{
  let cell: array<int> = make().rows
  return cell.count()
}}"
            ),
            "cell",
        ),
        (
            "a loop cell of a temporary",
            format!(
                "{BUCKET}function make() -> array<array<int>> {{
  let rows: array<array<int>> = [[1]]
  return rows
}}
function f() -> int {{
  let total: int = 0
  for (row in make()) {{
    total = total + row.count()
  }}
  return total
}}"
            ),
            "row",
        ),
    ];
    for (label, source, member) in cases {
        let transfers = transfers_of(&source, "f");
        let class = transfers
            .class_of(member)
            .unwrap_or_else(|| panic!("{label}: `{member}` is in no class"));
        assert_eq!(class.anchor, ReleaseAnchor::NoOwner, "{label}: {class:#?}");
    }
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
